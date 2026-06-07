//! Foreground guard — redirect focus away from desktop / Explorer / unrelated apps.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;

static GUARD_RUNNING: AtomicBool = AtomicBool::new(false);
static AUDIO_UI_YIELD: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
mod win {
    use super::{AUDIO_UI_YIELD, GUARD_RUNNING};
    use crate::process;
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::Duration;
    use tauri::AppHandle;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible,
        SetForegroundWindow,
    };

    fn hwnd_raw(hwnd: HWND) -> isize {
        hwnd.0 as isize
    }

    fn process_image_name(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buffer = [0u16; 260];
            let mut size = buffer.len() as u32;
            QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, &mut buffer, &mut size)
                .ok()
                .ok()?;
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            Some(String::from_utf16_lossy(&buffer[..len]))
        }
    }

    fn window_class(hwnd: HWND) -> String {
        let mut buffer = [0u16; 256];
        unsafe {
            let len = GetClassNameW(hwnd, &mut buffer);
            if len > 0 {
                return String::from_utf16_lossy(&buffer[..len as usize]);
            }
        }
        String::new()
    }

    fn is_explorer_shell_escape(hwnd: HWND, pid: u32) -> bool {
        let class = window_class(hwnd);
        if class == "Progman" || class == "WorkerW" || class == "Shell_TrayWnd" {
            return true;
        }
        if class == "CabinetWClass" {
            return true;
        }
        if let Some(path) = process_image_name(pid) {
            let lower = path.to_lowercase();
            if lower.ends_with("explorer.exe") {
                return class != "ApplicationFrameWindow";
            }
        }
        false
    }

    fn is_audio_settings_window(hwnd: HWND, pid: u32) -> bool {
        let class = window_class(hwnd);
        if class == "ApplicationFrameWindow" || class == "#32770" {
            if let Some(path) = process_image_name(pid) {
                let lower = path.to_lowercase();
                if lower.ends_with("systemsettings.exe")
                    || lower.ends_with("applicationframehost.exe")
                    || lower.ends_with("rundll32.exe")
                    || lower.contains("control.exe")
                {
                    return true;
                }
            }
        }
        false
    }

    fn is_allowed_foreground(app: &AppHandle, hwnd: HWND) -> bool {
        if hwnd.0.is_null() || !unsafe { IsWindowVisible(hwnd).as_bool() } {
            return true;
        }

        if process::is_kiosk_hwnd(app, hwnd_raw(hwnd)) {
            process::set_last_allowed_hwnd(hwnd_raw(hwnd));
            return true;
        }

        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }

        if process::is_pid_tracked(pid) {
            process::set_last_allowed_hwnd(hwnd_raw(hwnd));
            return true;
        }

        if AUDIO_UI_YIELD.load(Ordering::SeqCst) && is_audio_settings_window(hwnd, pid) {
            return true;
        }

        if is_explorer_shell_escape(hwnd, pid) {
            return false;
        }

        // Any other process while locked is denied.
        false
    }

    fn redirect_to_last_allowed(app: &AppHandle) {
        let target = process::last_allowed_foreground_hwnd(app);
        if let Some(hwnd) = target {
            unsafe {
                let hwnd = HWND(hwnd as *mut _);
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }

    pub fn start(app: AppHandle) {
        if GUARD_RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }

        thread::spawn(move || {
            while GUARD_RUNNING.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(500));
                if !crate::lockdown::is_locked() {
                    continue;
                }
                crate::lockdown::shell::hide_shell_chrome();
                unsafe {
                    let fg = GetForegroundWindow();
                    if !is_allowed_foreground(&app, fg) {
                        redirect_to_last_allowed(&app);
                    }
                }
            }
        });
    }

    pub fn stop() {
        GUARD_RUNNING.store(false, Ordering::SeqCst);
    }
}

#[cfg(target_os = "windows")]
pub fn start_foreground_guard(app: AppHandle) {
    win::start(app);
}

#[cfg(target_os = "windows")]
pub fn stop_foreground_guard() {
    win::stop();
}

#[cfg(not(target_os = "windows"))]
pub fn start_foreground_guard(_app: AppHandle) {}

#[cfg(not(target_os = "windows"))]
pub fn stop_foreground_guard() {}

pub fn set_audio_ui_yield(active: bool) {
    AUDIO_UI_YIELD.store(active, Ordering::SeqCst);
}
