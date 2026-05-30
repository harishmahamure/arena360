#[cfg(target_os = "windows")]
mod win {
    use std::process::Command;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };

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

    pub fn open_settings() -> Result<(), String> {
        Command::new("cmd")
            .args(["/C", "start", "", "ms-settings:sound"])
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
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
pub fn open_audio_settings() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::open_settings()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}
