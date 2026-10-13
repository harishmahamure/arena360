//! Installed Windows lifecycle. Development builds never change login startup,
//! close desktop applications, or spawn a recovery process.

#[cfg(all(windows, not(debug_assertions)))]
mod win {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
        REG_OPTION_NON_VOLATILE, REG_SZ,
    };
    use windows::Win32::System::Threading::{
        CreateEventW, OpenEventW, OpenProcess, SetEvent, WaitForMultipleObjects,
        WaitForSingleObject, EVENT_MODIFY_STATE, INFINITE, PROCESS_SYNCHRONIZE,
        SYNCHRONIZATION_SYNCHRONIZE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_SHUTTINGDOWN};

    const WATCH_ARGUMENT: &str = "--arena360-watch";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    static GENERATION: AtomicU32 = AtomicU32::new(0);
    static RECOVERY: Mutex<Option<Handle>> = Mutex::new(None);

    struct Handle(HANDLE);
    // Opaque kernel handles can be signaled/closed from any thread.
    unsafe impl Send for Handle {}
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }

    fn register_autostart() -> Result<(), String> {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let command = wide(&format!("\"{}\"", executable.display()));
        // REG_SZ is UTF-16 including its trailing NUL.
        let bytes: Vec<u8> = command.iter().flat_map(|unit| unit.to_le_bytes()).collect();
        unsafe {
            let mut key = HKEY::default();
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
                0,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut key,
                None,
            )
            .ok()
            .map_err(|e| e.to_string())?;
            let result = RegSetValueExW(key, w!("Arena360 Kiosk"), 0, REG_SZ, Some(&bytes));
            let _ = RegCloseKey(key);
            result.ok().map_err(|e| e.to_string())
        }
    }

    pub fn start_recovery() -> Result<(), String> {
        let mut recovery = RECOVERY.lock().map_err(|e| e.to_string())?;
        if recovery.is_some() {
            return Ok(());
        }
        let pid = std::process::id();
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst);
        let name = format!("Local\\Arena360KioskRecovery-{pid}-{generation}");
        let event_name = wide(&name);
        let event = Handle(unsafe {
            CreateEventW(None, true, false, PCWSTR(event_name.as_ptr()))
                .map_err(|e| e.to_string())?
        });
        let ready_name = wide(&format!("{name}-ready"));
        let ready = Handle(unsafe {
            CreateEventW(None, true, false, PCWSTR(ready_name.as_ptr()))
                .map_err(|e| e.to_string())?
        });
        Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .args([WATCH_ARGUMENT, &pid.to_string(), &name])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| e.to_string())?;
        // Do not consider recovery active until the helper holds both handles.
        if unsafe { WaitForSingleObject(ready.0, 5000) } != WAIT_OBJECT_0 {
            unsafe {
                let _ = SetEvent(event.0);
            }
            return Err("recovery helper did not become ready".into());
        }
        *recovery = Some(event);
        Ok(())
    }

    pub fn stop_recovery() {
        if let Ok(mut recovery) = RECOVERY.lock() {
            if let Some(event) = recovery.take() {
                unsafe {
                    let _ = SetEvent(event.0);
                }
            }
        }
    }

    fn watch(pid: u32, name: &str) -> Result<(), String> {
        let event_name = wide(name);
        // Open the cancellation event first: if the app has intentionally exited
        // before this helper starts, the event is gone and we must not restart it.
        let stop = Handle(unsafe {
            OpenEventW(
                SYNCHRONIZATION_SYNCHRONIZE,
                false,
                PCWSTR(event_name.as_ptr()),
            )
            .map_err(|e| e.to_string())?
        });
        let parent = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }
            .map(Handle)
            .map_err(|e| e.to_string())?;
        let ready_name = wide(&format!("{name}-ready"));
        let ready = Handle(unsafe {
            OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(ready_name.as_ptr()))
                .map_err(|e| e.to_string())?
        });
        unsafe {
            SetEvent(ready.0).map_err(|e| e.to_string())?;
        }
        let result = unsafe { WaitForMultipleObjects(&[stop.0, parent.0], false, INFINITE) };
        if result == WAIT_OBJECT_0 {
            return Ok(());
        }
        if result.0 != WAIT_OBJECT_0.0 + 1 {
            return Err("recovery wait failed".into());
        }
        // Debounce recovery to give cancellation and Windows logoff precedence.
        if unsafe { WaitForSingleObject(stop.0, 3000) } == WAIT_OBJECT_0
            || unsafe { GetSystemMetrics(SM_SHUTTINGDOWN) } != 0
        {
            return Ok(());
        }
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        crate::diagnostics::warn(format!(
            "kiosk process {pid} exited unexpectedly; restarting"
        ));
        // Retry transient launch errors (e.g. antivirus scanning a new binary).
        for _ in 0..10 {
            if !executable.exists() {
                return Ok(());
            }
            if Command::new(&executable)
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .is_ok()
            {
                return Ok(());
            }
            if unsafe { WaitForSingleObject(stop.0, 3000) } == WAIT_OBJECT_0
                || unsafe { GetSystemMetrics(SM_SHUTTINGDOWN) } != 0
            {
                return Ok(());
            }
        }
        Err("could not restart kiosk".into())
    }

    pub fn run_helper_if_requested() -> bool {
        let mut args = std::env::args().skip(1);
        if args.next().as_deref() != Some(WATCH_ARGUMENT) {
            return false;
        }
        if let (Some(pid), Some(name)) = (args.next().and_then(|arg| arg.parse().ok()), args.next())
        {
            if let Err(e) = watch(pid, &name) {
                crate::diagnostics::warn(format!("kiosk recovery: {e}"));
            }
        }
        true
    }

    pub fn initialize() {
        if let Err(e) = register_autostart() {
            crate::diagnostics::error(format!("kiosk autostart registration failed: {e}"));
        }
        if let Err(e) = start_recovery() {
            crate::diagnostics::error(format!("kiosk recovery startup failed: {e}"));
        }
        // Finish closing existing desktop apps before the player UI becomes usable.
        // Windows services/background agents are outside this cleanup.
        crate::process::close_existing_desktop_apps();
    }
}

pub fn run_helper_if_requested() -> bool {
    #[cfg(all(windows, not(debug_assertions)))]
    {
        win::run_helper_if_requested()
    }
    #[cfg(not(all(windows, not(debug_assertions))))]
    {
        false
    }
}

pub fn initialize() {
    #[cfg(all(windows, not(debug_assertions)))]
    win::initialize();
}

pub fn stop_recovery() {
    #[cfg(all(windows, not(debug_assertions)))]
    win::stop_recovery();
}

#[tauri::command]
pub fn resume_kiosk_recovery() {
    #[cfg(all(windows, not(debug_assertions)))]
    if let Err(e) = win::start_recovery() {
        crate::diagnostics::error(format!("kiosk recovery resume failed: {e}"));
    }
}
