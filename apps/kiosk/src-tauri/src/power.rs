#[cfg(target_os = "windows")]
mod win {
    use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
    use windows::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
        SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows::Win32::System::Shutdown::{
        ExitWindowsEx, LockWorkStation, EWX_FORCEIFHUNG, EWX_POWEROFF, EWX_REBOOT, EWX_SHUTDOWN,
        SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_APPLICATION, SHTDN_REASON_MINOR_OTHER,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct TokenHandle(HANDLE);

    impl Drop for TokenHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    fn enable_shutdown_privilege() -> Result<(), String> {
        unsafe {
            let mut token = HANDLE::default();
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
                &mut token,
            )
            .map_err(|e| e.to_string())?;
            let _token = TokenHandle(token);

            let mut luid = LUID::default();
            LookupPrivilegeValueW(None, SE_SHUTDOWN_NAME, &mut luid).map_err(|e| e.to_string())?;

            let privileges = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES {
                    Luid: luid,
                    Attributes: SE_PRIVILEGE_ENABLED,
                }],
            };

            AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None)
                .map_err(|e| e.to_string())
        }
    }

    pub fn lock_workstation() -> Result<(), String> {
        unsafe { LockWorkStation().map_err(|e| e.to_string()) }
    }

    pub fn restart() -> Result<(), String> {
        enable_shutdown_privilege()?;
        unsafe {
            ExitWindowsEx(
                EWX_REBOOT | EWX_FORCEIFHUNG,
                SHTDN_REASON_MAJOR_APPLICATION
                    | SHTDN_REASON_MINOR_OTHER
                    | SHTDN_REASON_FLAG_PLANNED,
            )
            .map_err(|e| e.to_string())
        }
    }

    pub fn shutdown() -> Result<(), String> {
        enable_shutdown_privilege()?;
        unsafe {
            ExitWindowsEx(
                EWX_SHUTDOWN | EWX_POWEROFF | EWX_FORCEIFHUNG,
                SHTDN_REASON_MAJOR_APPLICATION
                    | SHTDN_REASON_MINOR_OTHER
                    | SHTDN_REASON_FLAG_PLANNED,
            )
            .map_err(|e| e.to_string())
        }
    }
}

#[tauri::command]
pub fn lock_workstation() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::lock_workstation()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Lock workstation is only available on Windows".to_string())
    }
}

#[tauri::command]
pub fn restart_station() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::restart()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Restart is only available on Windows".to_string())
    }
}

#[tauri::command]
pub fn shutdown_station() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::shutdown()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Shutdown is only available on Windows".to_string())
    }
}
