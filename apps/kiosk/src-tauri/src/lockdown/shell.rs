//! Hide / restore Windows shell chrome (taskbar, Start menu) during lockdown.

#[cfg(any(windows, test))]
#[derive(Debug, PartialEq, Eq)]
enum WindowAction {
    Ignore,
    Hide,
    Close,
}

#[cfg(any(windows, test))]
fn window_action(class: &str, executable: Option<&str>) -> WindowAction {
    if matches!(class, "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
        return WindowAction::Hide;
    }
    match executable {
        Some(name) if name.eq_ignore_ascii_case("startmenuexperiencehost.exe") => {
            WindowAction::Hide
        }
        Some(name) if name.eq_ignore_ascii_case("taskmgr.exe") => WindowAction::Close,
        _ => WindowAction::Ignore,
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::{window_action, WindowAction};
    use std::collections::HashSet;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowW, GetAncestor, GetClassNameW, GetWindowThreadProcessId,
        IsWindowVisible, PostMessageW, ShowWindow, GA_ROOT, SW_HIDE, SW_SHOW, WM_CLOSE,
    };

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
        const START_MENU_TITLES: &[PCWSTR] =
            &[w!("Start"), w!("Start Menu"), w!("Start Experience Host")];

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

    fn action_for_window(
        hwnd: HWND,
        names: &mut std::collections::HashMap<u32, Option<String>>,
    ) -> WindowAction {
        unsafe {
            if hwnd.0.is_null()
                || !IsWindowVisible(hwnd).as_bool()
                || GetAncestor(hwnd, GA_ROOT) != hwnd
            {
                return WindowAction::Ignore;
            }
            let mut class = [0u16; 128];
            let len = GetClassNameW(hwnd, &mut class);
            let class = String::from_utf16_lossy(&class[..len.max(0) as usize]);
            let action = window_action(&class, None);
            if action != WindowAction::Ignore {
                return action;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            // Restrict top-level windows by identity even if staff accidentally
            // added a restricted executable to the launch allow-list.
            if pid == std::process::id() {
                return WindowAction::Ignore;
            }
            let name = names
                .entry(pid)
                .or_insert_with(|| crate::process::process_exe_name(pid));
            window_action(&class, name.as_deref())
        }
    }

    pub fn hide_window(raw: isize) {
        let hwnd = HWND(raw as *mut _);
        let action = action_for_window(hwnd, &mut std::collections::HashMap::new());
        if action != WindowAction::Ignore {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
                if action == WindowAction::Close {
                    let _ = PostMessageW(hwnd, WM_CLOSE, None, None);
                }
            }
        }
    }

    fn enum_start_menu_hwnds() -> Vec<HWND> {
        struct Search {
            hwnds: Vec<HWND>,
            names: std::collections::HashMap<u32, Option<String>>,
        }

        unsafe extern "system" fn enum_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let search = &mut *(lparam.0 as *mut Search);
            let action = action_for_window(hwnd, &mut search.names);
            if action != WindowAction::Ignore {
                search.hwnds.push(hwnd);
                if action == WindowAction::Close {
                    // Also covers an existing instance and stations whose IT
                    // policy prevents our per-user registry restriction.
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    let _ = PostMessageW(hwnd, WM_CLOSE, None, None);
                }
            }
            BOOL(1)
        }

        let mut search = Search {
            hwnds: Vec::new(),
            names: std::collections::HashMap::new(),
        };
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
                .chain(enum_start_menu_hwnds()),
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
pub fn hide_restricted_window(hwnd: isize) {
    win::hide_window(hwnd);
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
    use super::{window_action, WindowAction};
    #[test]
    fn restricts_shell_and_task_manager_without_hiding_game_or_other_app_windows() {
        for class in ["Shell_TrayWnd", "Shell_SecondaryTrayWnd"] {
            assert_eq!(window_action(class, None), WindowAction::Hide);
        }
        assert_eq!(
            window_action(
                "Windows.UI.Core.CoreWindow",
                Some("StartMenuExperienceHost.exe")
            ),
            WindowAction::Hide
        );
        assert_eq!(
            window_action("TaskManagerWindow", Some("Taskmgr.EXE")),
            WindowAction::Close
        );
        for name in [
            "valorant.exe",
            "steam.exe",
            "notepad.exe",
            "msedgewebview2.exe",
        ] {
            assert_eq!(
                window_action("ApplicationWindow", Some(name)),
                WindowAction::Ignore
            );
        }
        assert_eq!(
            window_action("ApplicationWindow", None),
            WindowAction::Ignore
        );
    }
}
