//! Hide / restore Windows shell chrome (taskbar) during lockdown.

#[cfg(target_os = "windows")]
mod win {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, ShowWindow, SW_HIDE, SW_SHOW};
    use windows::core::w;

    fn shell_tray_hwnds() -> Vec<HWND> {
        let mut hwnds = Vec::new();
        unsafe {
            if let Ok(primary) = FindWindowW(w!("Shell_TrayWnd"), None) {
                if !primary.0.is_null() {
                    hwnds.push(primary);
                }
            }
            if let Ok(secondary) = FindWindowW(w!("Shell_SecondaryTrayWnd"), None) {
                if !secondary.0.is_null() {
                    hwnds.push(secondary);
                }
            }
        }
        hwnds
    }

    pub fn hide() {
        for hwnd in shell_tray_hwnds() {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }

    pub fn restore() {
        for hwnd in shell_tray_hwnds() {
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub fn hide_shell_chrome() {
    win::hide();
}

#[cfg(target_os = "windows")]
pub fn restore_shell_chrome() {
    win::restore();
}

#[cfg(not(target_os = "windows"))]
pub fn hide_shell_chrome() {}

#[cfg(not(target_os = "windows"))]
pub fn restore_shell_chrome() {}
