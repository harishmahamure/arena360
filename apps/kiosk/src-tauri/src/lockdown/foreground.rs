//! Shell maintenance while locked — hide taskbar chrome and refresh tracked game HWNDs.
//!
//! Alt+Tab is handled by Windows. We do not redirect focus away from the user's
//! Alt+Tab selection (desktop, other apps, games, or kiosk).

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;

static GUARD_RUNNING: AtomicBool = AtomicBool::new(false);
static AUDIO_UI_YIELD: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
mod win {
    use super::GUARD_RUNNING;
    use crate::process;
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::Duration;
    use tauri::AppHandle;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible,
    };

    fn hwnd_raw(hwnd: HWND) -> isize {
        hwnd.0 as isize
    }

    /// Remember kiosk / tracked-game HWNDs when the user Alt+Tabs to them.
    pub fn capture_session_foreground(app: &AppHandle) {
        unsafe {
            let fg = GetForegroundWindow();
            if fg.0.is_null() || !IsWindowVisible(fg).as_bool() {
                return;
            }
            if process::is_kiosk_hwnd(app, hwnd_raw(fg)) {
                process::set_last_allowed_hwnd(hwnd_raw(fg));
                return;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(fg, Some(&mut pid));
            if process::is_pid_tracked(pid) {
                process::set_last_allowed_hwnd(hwnd_raw(fg));
            }
        }
    }

    pub fn start(_app: AppHandle) {
        if GUARD_RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }

        thread::spawn(move || {
            while GUARD_RUNNING.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(500));
                if !crate::lockdown::is_locked() {
                    continue;
                }
                if process::has_tracked_processes() {
                    process::refresh_tracked_state();
                }
                crate::lockdown::shell::hide_shell_chrome();
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

#[cfg(target_os = "windows")]
pub fn capture_allowed_foreground(app: &AppHandle) {
    win::capture_session_foreground(app);
}

#[cfg(not(target_os = "windows"))]
pub fn start_foreground_guard(_app: AppHandle) {}

#[cfg(not(target_os = "windows"))]
pub fn stop_foreground_guard() {}

#[cfg(not(target_os = "windows"))]
pub fn capture_allowed_foreground(_app: &AppHandle) {}

pub fn set_audio_ui_yield(active: bool) {
    AUDIO_UI_YIELD.store(active, Ordering::SeqCst);
}
