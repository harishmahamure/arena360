use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(target_os = "windows")]
const DETACHED_PROCESS: u32 = 0x0000_0008;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedProcess {
    pub pid: u32,
    pub executable_path: String,
}

#[derive(Debug, Clone)]
struct WatchEntry {
    executable_path: String,
    pid: u32,
    _window_handle: Option<isize>,
}

static TRACKED: Mutex<Vec<WatchEntry>> = Mutex::new(Vec::new());
static MONITOR_RUNNING: Mutex<bool> = Mutex::new(false);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub pid: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KillResult {
    pub killed: u32,
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn is_allowed(path: &str, allow_list: &[String]) -> bool {
    if allow_list.is_empty() {
        return true;
    }
    let norm = normalize_path(path);
    allow_list
        .iter()
        .any(|entry| norm.ends_with(&normalize_path(entry)))
}

fn spawn_process(path: &str, args: Option<&str>) -> Result<u32, String> {
    let mut cmd = Command::new(path);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    if let Some(parent) = Path::new(path).parent() {
        cmd.current_dir(parent);
    }
    if let Some(a) = args {
        for part in a.split_whitespace() {
            cmd.arg(part);
        }
    }
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    Ok(child.id())
}

fn is_running(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command as StdCommand;
        let output = StdCommand::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}")])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            return text.contains(&pid.to_string());
        }
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::process::Command as StdCommand;
        StdCommand::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn kill_pid(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
    }
}

#[cfg(target_os = "windows")]
fn fullscreen_process_window(pid: u32) -> Option<isize> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowLongPtrW, GetWindowThreadProcessId, IsWindowVisible,
        SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_STYLE, HWND_TOPMOST,
        SWP_FRAMECHANGED, SWP_SHOWWINDOW, SW_MAXIMIZE, SW_RESTORE, WS_CAPTION, WS_MAXIMIZEBOX,
        WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
    };

    struct Search {
        pid: u32,
        hwnd: HWND,
    }

    unsafe extern "system" fn enum_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &mut *(lparam.0 as *mut Search);
        let mut window_pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        if window_pid == search.pid && IsWindowVisible(hwnd).as_bool() {
            search.hwnd = hwnd;
            return BOOL(0);
        }
        BOOL(1)
    }

    unsafe fn make_borderless_fullscreen(hwnd: HWND) {
        let _ = ShowWindow(hwnd, SW_RESTORE);

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            let _ = ShowWindow(hwnd, SW_MAXIMIZE);
            return;
        }

        // Best-effort borderless fullscreen for normal Win32 windows. Some games
        // own their swapchain/window mode and may ignore or later override this.
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let borderless = style
            & !(WS_CAPTION.0
                | WS_THICKFRAME.0
                | WS_SYSMENU.0
                | WS_MINIMIZEBOX.0
                | WS_MAXIMIZEBOX.0);
        let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, borderless as isize);

        let rect = monitor_info.rcMonitor;
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        );
    }

    for _ in 0..24 {
        let mut search = Search {
            pid,
            hwnd: HWND(std::ptr::null_mut()),
        };
        unsafe {
            let _ = EnumWindows(
                Some(enum_windows),
                LPARAM(&mut search as *mut Search as isize),
            );
            if !search.hwnd.0.is_null() {
                make_borderless_fullscreen(search.hwnd);
                let _ = SetForegroundWindow(search.hwnd);
                return Some(search.hwnd.0 as isize);
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn fullscreen_process_window(_pid: u32) -> Option<isize> {
    None
}

fn main_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window("main")
        .or_else(|| app.webview_windows().values().next().cloned())
        .ok_or_else(|| "Main window not found".to_string())
}

#[cfg(windows)]
fn force_foreground(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetForegroundWindow, SetWindowPos, ShowWindow, HWND_TOPMOST, SW_RESTORE, SW_SHOW,
        SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    if let Ok(hwnd_raw) = window.hwnd() {
        unsafe {
            let hwnd = HWND(hwnd_raw.0 as *mut _);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(not(windows))]
fn force_foreground(_window: &tauri::WebviewWindow) {}

fn show_kiosk_foreground(app: &AppHandle) {
    if let Ok(window) = main_window(app) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_decorations(false);
        let _ = window.set_fullscreen(true);
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        force_foreground(&window);
    }
}

pub fn has_tracked_processes() -> bool {
    TRACKED
        .lock()
        .map(|tracked| !tracked.is_empty())
        .unwrap_or(false)
}

pub fn focus_kiosk_window(app: &AppHandle) -> Result<(), String> {
    show_kiosk_foreground(app);
    Ok(())
}

fn prepare_kiosk_for_game_mode(app: &AppHandle) -> Result<(), String> {
    let window = main_window(app)?;
    window.set_always_on_top(false).map_err(|e| e.to_string())?;
    // Minimize so Alt+Tab lists the kiosk separately from launched games and
    // `show_kiosk_foreground` can raise it above TOPMOST game windows.
    window.minimize().map_err(|e| e.to_string())
}

fn restore_kiosk_window(app: &AppHandle) {
    show_kiosk_foreground(app);
}

fn start_monitor_if_needed(app: AppHandle) {
    let mut running = MONITOR_RUNNING.lock().expect("monitor lock");
    if *running {
        return;
    }
    *running = true;
    drop(running);

    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(2));
        let is_empty = {
            let mut tracked = TRACKED.lock().expect("tracked lock");
            tracked.retain(|entry| is_running(entry.pid));
            tracked.is_empty()
        };
        if is_empty {
            restore_kiosk_window(&app);
            if let Ok(mut running) = MONITOR_RUNNING.lock() {
                *running = false;
            }
            break;
        }
    });
}

#[tauri::command]
pub fn focus_kiosk(app: AppHandle) -> Result<(), String> {
    focus_kiosk_window(&app)
}

#[tauri::command]
pub fn launch_allowed(
    app: AppHandle,
    executable_path: String,
    allow_list: Vec<String>,
    arguments: Option<String>,
) -> Result<LaunchResult, String> {
    if !is_allowed(&executable_path, &allow_list) {
        return Err("Executable not in allow-list".to_string());
    }
    prepare_kiosk_for_game_mode(&app)?;
    let pid = spawn_process(&executable_path, arguments.as_deref())?;
    let window_handle = fullscreen_process_window(pid);
    TRACKED.lock().map_err(|e| e.to_string())?.push(WatchEntry {
        executable_path,
        pid,
        _window_handle: window_handle,
    });
    start_monitor_if_needed(app);
    Ok(LaunchResult { pid })
}

#[tauri::command]
pub fn get_tracked_processes() -> Result<Vec<TrackedProcess>, String> {
    let tracked = TRACKED.lock().map_err(|e| e.to_string())?;
    Ok(tracked
        .iter()
        .map(|e| TrackedProcess {
            pid: e.pid,
            executable_path: e.executable_path.clone(),
        })
        .collect())
}

#[tauri::command]
pub fn kill_tracked_processes(
    app: AppHandle,
    grace_seconds: Option<u32>,
) -> Result<KillResult, String> {
    let grace = grace_seconds.unwrap_or(0);
    if grace > 0 {
        thread::sleep(Duration::from_secs(grace as u64));
    }
    let pids: Vec<u32> = TRACKED
        .lock()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|e| e.pid)
        .collect();
    for pid in &pids {
        kill_pid(*pid);
    }
    TRACKED.lock().map_err(|e| e.to_string())?.clear();
    if !pids.is_empty() {
        restore_kiosk_window(&app);
    }
    Ok(KillResult {
        killed: pids.len() as u32,
    })
}

#[tauri::command]
pub fn clear_tracked_processes() -> Result<(), String> {
    TRACKED.lock().map_err(|e| e.to_string())?.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{is_allowed, normalize_path};

    #[test]
    fn normalize_lowercases_and_unifies_separators() {
        assert_eq!(normalize_path("C:\\Games\\Steam.EXE"), "c:/games/steam.exe");
    }

    #[test]
    fn empty_allow_list_permits_everything() {
        assert!(is_allowed("C:/anything.exe", &[]));
    }

    #[test]
    fn allow_list_matches_by_suffix_case_insensitively() {
        let allow = vec!["C:\\Program Files\\Steam\\steam.exe".to_string()];
        assert!(is_allowed("c:/program files/steam/STEAM.exe", &allow));
    }

    #[test]
    fn allow_list_rejects_unlisted_executable() {
        let allow = vec!["C:\\Games\\steam.exe".to_string()];
        assert!(!is_allowed("C:\\Windows\\System32\\cmd.exe", &allow));
    }
}
