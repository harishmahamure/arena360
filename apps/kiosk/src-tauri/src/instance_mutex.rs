//! Single-instance mutex for the kiosk process (Windows).
//!
//! A second launch (e.g. logon scheduled task while the app is already running)
//! must exit immediately so only one kiosk window exists.

#[cfg(windows)]
mod win {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex};

    pub const INSTANCE_MUTEX_NAME: &str = "Global\\Arena360KioskInstance";

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
        unsafe {
            let wide_name = wide(INSTANCE_MUTEX_NAME);
            let handle = CreateMutexW(None, true, PCWSTR(wide_name.as_ptr()))
                .map_err(|e| format!("CreateMutexW failed: {e}"))?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return Err("Another kiosk instance is already running".to_string());
            }
            Ok(InstanceMutexGuard { handle })
        }
    }
}

#[cfg(windows)]
pub use win::{acquire_instance_mutex, InstanceMutexGuard, INSTANCE_MUTEX_NAME};

/// Acquire the global single-instance mutex or exit the process.
///
/// Called before Tauri initializes so duplicate launches (scheduled task, manual
/// double-click) never create a second window.
pub fn ensure_single_instance() {
    #[cfg(windows)]
    {
        match acquire_instance_mutex() {
            Ok(guard) => {
                // Leak the guard for process lifetime; releasing would allow a second instance.
                std::mem::forget(guard);
            }
            Err(reason) => {
                crate::diagnostics::info(format!(
                    "duplicate kiosk instance detected, exiting: {reason}"
                ));
                // Exit 0 so logon scheduled tasks do not report failure when kiosk is already up.
                std::process::exit(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn mutex_name_is_global() {
        #[cfg(windows)]
        assert!(super::INSTANCE_MUTEX_NAME.starts_with("Global\\"));
    }
}
