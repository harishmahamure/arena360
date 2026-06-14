use super::{
    INSTANCE_MUTEX_NAME, SPAWN_DEBOUNCE_SECS, WATCHDOG_EXE_BASENAME, WATCHDOG_MUTEX_NAME,
};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{ProcessesToUpdate, ProcessRefreshKind, RefreshKind, System};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CreateMutexW, OpenMutexW, ReleaseMutex, SYNCHRONIZATION_ACCESS_RIGHTS,
};

static LAST_SPAWN_MS: AtomicU64 = AtomicU64::new(0);

pub struct InstanceMutexGuard {
    handle: HANDLE,
}

impl Drop for InstanceMutexGuard {
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

pub fn acquire_instance_mutex() -> Result<InstanceMutexGuard, String> {
    acquire_named_mutex(INSTANCE_MUTEX_NAME)
}

pub fn acquire_watchdog_mutex() -> Result<InstanceMutexGuard, String> {
    acquire_named_mutex(WATCHDOG_MUTEX_NAME)
}

fn acquire_named_mutex(name: &str) -> Result<InstanceMutexGuard, String> {
    unsafe {
        let wide_name = wide(name);
        let handle = CreateMutexW(None, true, PCWSTR(wide_name.as_ptr()))
            .map_err(|e| format!("CreateMutexW failed: {e}"))?;
        if GetLastError() == windows::Win32::Foundation::ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return Err(format!("mutex already held: {name}"));
        }
        Ok(InstanceMutexGuard { handle })
    }
}

pub fn is_instance_mutex_held() -> bool {
    unsafe {
        let wide_name = wide(INSTANCE_MUTEX_NAME);
        let Ok(handle) = OpenMutexW(SYNCHRONIZATION_ACCESS_RIGHTS(0x00100000), false, PCWSTR(wide_name.as_ptr()))
        else {
            return false;
        };
        let wait = windows::Win32::System::Threading::WaitForSingleObject(handle, 0);
        let _ = CloseHandle(handle);
        wait == WAIT_OBJECT_0
    }
}

pub fn is_kiosk_running_in_dir(install_dir: &Path, kiosk_exe: &Path) -> bool {
    if is_instance_mutex_held() {
        return true;
    }

    let canonical_kiosk = normalize_path(kiosk_exe);
    let canonical_dir = normalize_path(install_dir);

    let mut system = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );
    system.refresh_processes(ProcessesToUpdate::All, true);

    system.processes().values().any(|process| {
        process.exe().is_some_and(|exe| {
            let exe_path = normalize_path(exe);
            exe_path.starts_with(&canonical_dir)
                && exe_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| {
                        !name.starts_with(WATCHDOG_EXE_BASENAME)
                            && !name.eq_ignore_ascii_case("uninstall.exe")
                    })
                && (exe_path == canonical_kiosk || is_likely_kiosk_exe(&exe_path))
        })
    })
}

fn is_likely_kiosk_exe(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| {
            (name.contains("Arena360") || name.contains("Station Management"))
                && !name.starts_with(WATCHDOG_EXE_BASENAME)
        })
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

pub fn spawn_kiosk(kiosk_exe: &Path) -> Result<(), String> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64;
    let last = LAST_SPAWN_MS.load(Ordering::Relaxed);
    if now_ms.saturating_sub(last) < SPAWN_DEBOUNCE_SECS * 1000 {
        return Ok(());
    }

    let work_dir = kiosk_exe
        .parent()
        .ok_or_else(|| "kiosk exe has no parent".to_string())?;

    Command::new(kiosk_exe)
        .current_dir(work_dir)
        .spawn()
        .map_err(|e| format!("failed to spawn kiosk: {e}"))?;

    LAST_SPAWN_MS.store(now_ms, Ordering::Relaxed);
    Ok(())
}

pub fn watchdog_main_loop(watchdog_exe: PathBuf) -> Result<(), String> {
    let _watchdog_guard = acquire_watchdog_mutex()?;
    let kiosk_exe = super::resolve_kiosk_exe(&watchdog_exe)?;
    let install_dir = kiosk_exe
        .parent()
        .ok_or_else(|| "kiosk exe has no parent".to_string())?
        .to_path_buf();

    loop {
        let now = SystemTime::now();
        if super::is_paused(now).unwrap_or(false) {
            std::thread::sleep(Duration::from_secs(super::POLL_INTERVAL_SECS));
            continue;
        }
        if is_instance_mutex_held() {
            std::thread::sleep(Duration::from_secs(super::POLL_INTERVAL_SECS));
            continue;
        }
        if !is_kiosk_running_in_dir(&install_dir, &kiosk_exe) {
            let _ = spawn_kiosk(&kiosk_exe);
        }

        std::thread::sleep(Duration::from_secs(super::POLL_INTERVAL_SECS));
    }
}
