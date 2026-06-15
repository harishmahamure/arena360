//! Single-instance mutex for the kiosk process (Windows).

#[cfg(windows)]
mod win {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
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
            if GetLastError() == windows::Win32::Foundation::ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return Err("Another kiosk instance is already running".to_string());
            }
            Ok(InstanceMutexGuard { handle })
        }
    }
}

#[cfg(windows)]
pub use win::{acquire_instance_mutex, InstanceMutexGuard, INSTANCE_MUTEX_NAME};

/// Hold the kiosk single-instance mutex for the process lifetime.
pub fn init_instance_mutex() {
    #[cfg(windows)]
    {
        if let Ok(guard) = acquire_instance_mutex() {
            // Cannot store HANDLE in a Sync static; leaking the guard is intentional.
            std::mem::forget(guard);
        }
    }
}
