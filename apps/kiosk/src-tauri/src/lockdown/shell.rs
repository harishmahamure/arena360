//! Hide / restore Windows shell chrome (taskbar, Start menu) during lockdown.

#[cfg(target_os = "windows")]
mod win {
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use sysinfo::{Pid, ProcessesToUpdate, System};
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowW, GetWindowThreadProcessId, IsWindowVisible, ShowWindow, SW_HIDE,
        SW_SHOW,
    };
    use windows::core::{w, PCWSTR};

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

    /// Win10/11 Start flyout windows created before our keyboard hook swallows Win/Ctrl+Esc.
    fn known_start_menu_hwnds() -> Vec<HWND> {
        const START_MENU_CLASS: PCWSTR = w!("Windows.UI.Core.CoreWindow");
        const START_MENU_TITLES: &[PCWSTR] = &[
            w!("Start"),
            w!("Start Menu"),
            w!("Start Experience Host"),
        ];

        let mut hwnds = Vec::new();
        unsafe {
            for title in START_MENU_TITLES {
                if let Ok(hwnd) = FindWindowW(START_MENU_CLASS, *title) {
                    if !hwnd.0.is_null() {
                        hwnds.push(hwnd);
                    }
                }
            }
        }
        hwnds
    }

    fn process_exe_name(pid: u32) -> Option<String> {
        if pid == 0 {
            return None;
        }
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);
        system
            .process(Pid::from_u32(pid))
            .map(|p| p.name().to_string_lossy().to_lowercase())
    }

    static LAST_START_MENU_ENUM_MS: AtomicU64 = AtomicU64::new(0);
    const START_MENU_ENUM_INTERVAL_MS: u64 = 2000;

    fn maybe_enum_start_menu_hwnds() -> Vec<HWND> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let last = LAST_START_MENU_ENUM_MS.load(Ordering::Relaxed);
        if now.saturating_sub(last) < START_MENU_ENUM_INTERVAL_MS {
            return Vec::new();
        }
        LAST_START_MENU_ENUM_MS.store(now, Ordering::Relaxed);
        enum_start_menu_hwnds()
    }

    fn enum_start_menu_hwnds() -> Vec<HWND> {
        struct Search {
            hwnds: Vec<HWND>,
        }

        unsafe extern "system" fn enum_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let search = &mut *(lparam.0 as *mut Search);
            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let Some(name) = process_exe_name(pid) else {
                return BOOL(1);
            };
            if name == "startmenuexperiencehost.exe" {
                search.hwnds.push(hwnd);
            }
            BOOL(1)
        }

        let mut search = Search { hwnds: Vec::new() };
        unsafe {
            let _ = EnumWindows(
                Some(enum_windows),
                LPARAM(&mut search as *mut Search as isize),
            );
        }
        search.hwnds
    }

    fn hide_hwnds(hwnds: impl IntoIterator<Item = HWND>) {
        let mut seen = HashSet::new();
        for hwnd in hwnds {
            let key = hwnd.0 as usize;
            if seen.insert(key) {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
    }

    pub fn hide() {
        hide_hwnds(
            shell_tray_hwnds()
                .into_iter()
                .chain(known_start_menu_hwnds())
                .chain(maybe_enum_start_menu_hwnds()),
        );
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

#[cfg(test)]
mod tests {
    #[test]
    fn start_menu_process_names_are_lowercase() {
        for name in [
            "startmenuexperiencehost.exe",
            "StartMenuExperienceHost.exe",
        ] {
            assert_eq!(name.to_ascii_lowercase(), "startmenuexperiencehost.exe");
        }
    }
}
