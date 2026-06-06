use tauri::AppHandle;

#[cfg(target_os = "windows")]
mod win {
    use std::ffi::c_void;
    use tauri::{AppHandle, Manager};
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
    use windows::core::{w, PCWSTR};

    struct ComGuard;

    impl ComGuard {
        fn init() -> Result<Self, String> {
            unsafe {
                CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                    .ok()
                    .map_err(|e| e.to_string())?;
            }
            Ok(Self)
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }

    fn endpoint_volume() -> Result<IAudioEndpointVolume, String> {
        let _com = ComGuard::init()?;
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(|e| e.to_string())?;
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| e.to_string())?;
            device.Activate(CLSCTX_ALL, None).map_err(|e| e.to_string())
        }
    }

    pub fn get_volume() -> Result<u8, String> {
        let endpoint = endpoint_volume()?;
        let scalar = unsafe {
            endpoint
                .GetMasterVolumeLevelScalar()
                .map_err(|e| e.to_string())?
        };
        Ok((scalar.clamp(0.0, 1.0) * 100.0).round() as u8)
    }

    pub fn set_volume(volume: u8) -> Result<u8, String> {
        let endpoint = endpoint_volume()?;
        let volume = volume.min(100);
        unsafe {
            endpoint
                .SetMasterVolumeLevelScalar(volume as f32 / 100.0, std::ptr::null())
                .map_err(|e| e.to_string())?;
            endpoint
                .SetMute(volume == 0, std::ptr::null())
                .map_err(|e| e.to_string())?;
        }
        Ok(volume)
    }

    fn shell_execute(file: PCWSTR, params: PCWSTR) -> Result<(), String> {
        unsafe {
            let result = ShellExecuteW(
                None,
                w!("open"),
                file,
                params,
                None,
                SW_SHOW,
            );
            // Values <= 32 indicate failure (https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew)
            if result.0 <= 32_usize as *mut c_void {
                return Err(format!(
                    "Could not open sound settings (ShellExecute code {})",
                    result.0 as isize
                ));
            }
        }
        Ok(())
    }

    /// Lower the always-on-top kiosk so Windows sound UI is visible, then open it.
    fn yield_kiosk_to_external_ui(app: &AppHandle) -> Result<(), String> {
        let window = app
            .get_webview_window("main")
            .or_else(|| app.webview_windows().values().next().cloned())
            .ok_or_else(|| "Main window not found".to_string())?;
        window
            .set_always_on_top(false)
            .map_err(|e| e.to_string())?;
        window.set_fullscreen(false).map_err(|e| e.to_string())?;
        window.minimize().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn open_settings(app: &AppHandle) -> Result<(), String> {
        yield_kiosk_to_external_ui(app)?;

        if shell_execute(w!("ms-settings:sound"), PCWSTR::null()).is_ok() {
            return Ok(());
        }

        shell_execute(w!("control"), w!("mmsys.cpl,,2"))
    }
}

#[tauri::command]
pub fn get_system_volume() -> Result<u8, String> {
    #[cfg(target_os = "windows")]
    {
        win::get_volume()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("System volume is only available on Windows".to_string())
    }
}

#[tauri::command]
pub fn set_system_volume(volume: u8) -> Result<u8, String> {
    #[cfg(target_os = "windows")]
    {
        win::set_volume(volume)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(volume.min(100))
    }
}

#[tauri::command]
pub fn open_audio_settings(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::open_settings(&app)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("Sound settings are only available on Windows".to_string())
    }
}
