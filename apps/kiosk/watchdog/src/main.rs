//! Arena360 kiosk watchdog sidecar (ADR-0048).
//!
//! Polls every 2 s and relaunches the main kiosk when absent unless a valid pause file exists.

#[cfg(not(windows))]
fn main() {
    eprintln!("arena360-watchdog is Windows-only");
    std::process::exit(1);
}

#[cfg(windows)]
mod win {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::process::Command;
    use std::thread;
    use std::time::Duration;
    use watchdog_common::{
        is_pause_file_active, purge_expired_pause, resolve_kiosk_exe, WATCHDOG_MUTEX_NAME,
        WATCHDOG_POLL_MS,
    };
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex};

    struct WatchdogMutexGuard {
        handle: HANDLE,
    }

    impl Drop for WatchdogMutexGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = ReleaseMutex(self.handle);
                let _ = CloseHandle(self.handle);
            }
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    fn acquire_watchdog_mutex() -> Result<WatchdogMutexGuard, String> {
        unsafe {
            let wide_name = wide(WATCHDOG_MUTEX_NAME);
            let handle = CreateMutexW(None, true, PCWSTR(wide_name.as_ptr()))
                .map_err(|e| format!("CreateMutexW failed: {e}"))?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return Err("Another watchdog instance is already running".to_string());
            }
            Ok(WatchdogMutexGuard { handle })
        }
    }

    fn kiosk_instance_running() -> bool {
        unsafe {
            let wide_name = wide(watchdog_common::KIOSK_MUTEX_NAME);
            let handle = CreateMutexW(None, false, PCWSTR(wide_name.as_ptr()));
            match handle {
                Ok(h) => {
                    let already = GetLastError() == ERROR_ALREADY_EXISTS;
                    let _ = CloseHandle(h);
                    already
                }
                Err(_) => false,
            }
        }
    }

    fn spawn_kiosk(path: &Path) -> Result<(), String> {
        Command::new(path)
            .spawn()
            .map_err(|e| format!("failed to spawn {}: {e}", path.display()))?;
        Ok(())
    }

    pub fn run() -> Result<(), String> {
        let _guard = acquire_watchdog_mutex()?;
        let _ = purge_expired_pause();

        let watchdog_exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let Some(kiosk_exe) = resolve_kiosk_exe(&watchdog_exe) else {
            return Err(format!(
                "could not resolve kiosk executable near {}",
                watchdog_exe.display()
            ));
        };

        eprintln!(
            "arena360-watchdog started; monitoring {} (kiosk: {})",
            watchdog_exe.display(),
            kiosk_exe.display()
        );

        let mut consecutive_spawn_failures = 0u32;
        loop {
            if is_pause_file_active() {
                consecutive_spawn_failures = 0;
            } else if !kiosk_instance_running() {
                match spawn_kiosk(&kiosk_exe) {
                    Ok(()) => {
                        eprintln!("spawned kiosk: {}", kiosk_exe.display());
                        consecutive_spawn_failures = 0;
                    }
                    Err(err) => {
                        consecutive_spawn_failures = consecutive_spawn_failures.saturating_add(1);
                        eprintln!("spawn failed ({consecutive_spawn_failures}): {err}");
                    }
                }
            }

            let backoff = if consecutive_spawn_failures > 3 {
                WATCHDOG_POLL_MS.saturating_mul(consecutive_spawn_failures.min(10) as u64)
            } else {
                WATCHDOG_POLL_MS
            };
            thread::sleep(Duration::from_millis(backoff));
        }
    }
}

#[cfg(windows)]
fn main() {
    if let Err(err) = win::run() {
        eprintln!("arena360-watchdog exiting: {err}");
        std::process::exit(if err.contains("already running") { 0 } else { 1 });
    }
}
