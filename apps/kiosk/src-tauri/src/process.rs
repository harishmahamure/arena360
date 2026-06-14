use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};

#[cfg(windows)]
use sysinfo::{Pid, ProcessesToUpdate, System};

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
    root_pid: u32,
    descendant_pids: Vec<u32>,
    window_handle: Option<isize>,
}

static TRACKED: Mutex<Vec<WatchEntry>> = Mutex::new(Vec::new());
static MONITOR_RUNNING: Mutex<bool> = Mutex::new(false);
static LAST_ALLOWED_HWND: Mutex<Option<isize>> = Mutex::new(None);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub pid: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KillResult {
    pub killed: u32,
    pub restored: bool,
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

fn spawn_process(path: &str, args: Option<&[String]>) -> Result<u32, String> {
    let mut cmd = Command::new(path);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    if let Some(parent) = Path::new(path).parent() {
        cmd.current_dir(parent);
    }
    if let Some(parts) = args {
        for part in parts {
            cmd.arg(part);
        }
    }
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    Ok(child.id())
}

fn normalize_launch_arguments(arguments: Option<Vec<String>>) -> Option<Vec<String>> {
    let args = arguments?;
    if args.is_empty() {
        return None;
    }
    Some(args)
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

/// Collect root PID and all descendant PIDs (US-KPROC-001).
#[cfg(windows)]
fn process_tree_pids(root: u32, system: &System) -> Vec<u32> {
    let mut tree = vec![root];
    let mut seen = HashSet::from([root]);
    let mut queue = vec![root];
    while let Some(parent) = queue.pop() {
        for (pid, process) in system.processes() {
            if process.parent().map(|p| p.as_u32()) != Some(parent) {
                continue;
            }
            let child = pid.as_u32();
            if seen.insert(child) {
                tree.push(child);
                queue.push(child);
            }
        }
    }
    tree
}

#[cfg(not(windows))]
fn process_tree_pids(root: u32) -> Vec<u32> {
    vec![root]
}

#[cfg(windows)]
fn refresh_system_processes(system: &mut System) {
    system.refresh_processes(ProcessesToUpdate::All, true);
}

#[cfg(windows)]
fn entry_has_live_process(entry: &WatchEntry, system: &System) -> bool {
    let live = |pid: u32| system.process(Pid::from_u32(pid)).is_some();
    live(entry.root_pid) || entry.descendant_pids.iter().copied().any(live)
}

#[cfg(windows)]
fn refresh_watch_entry(entry: &mut WatchEntry, system: &System) {
    entry.descendant_pids = process_tree_pids(entry.root_pid, system)
        .into_iter()
        .filter(|pid| *pid != entry.root_pid)
        .collect();
}

#[cfg(not(windows))]
fn entry_has_live_process(entry: &WatchEntry) -> bool {
    is_running(entry.root_pid)
}

fn executable_basename(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_lowercase()
}

fn is_launcher_executable(path: &str) -> bool {
    let name = executable_basename(path);
    is_overlay_launcher_exe(&name) || is_launcher_exe_basename(&name, path)
}

fn is_launcher_exe_basename(name: &str, path: &str) -> bool {
    let norm = normalize_path(path);
    if name == "launcher.exe" && norm.contains("/rockstar games/launcher/") {
        return true;
    }
    matches!(
        name,
        "riotclientservices.exe"
            | "steam.exe"
            | "epicgameslauncher.exe"
            | "battle.net launcher.exe"
            | "battle.net.exe"
            | "ubisoftconnect.exe"
            | "ealauncher.exe"
            | "ealaunchhelper.exe"
            | "galaxyclient.exe"
    )
}

/// Launcher UI processes whose windows should not cover the kiosk shell.
pub fn is_overlay_launcher_exe(name: &str) -> bool {
    matches!(
        name,
        "riotclientux.exe"
            | "riotclientservices.exe"
            | "steamwebhelper.exe"
            | "steamservice.exe"
    )
}

const SESSION_KEEP_EXE: &[&str] = &[
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "svchost.exe",
    "dwm.exe",
    "explorer.exe",
    "fontdrvhost.exe",
    "sihost.exe",
    "taskhostw.exe",
    "runtimebroker.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "ctfmon.exe",
    "audiodg.exe",
    "securityhealthsystray.exe",
    "securityhealthservice.exe",
    "system",
    "registry",
    "smss.exe",
    "conhost.exe",
];

pub fn is_session_keep_process(name: &str, kiosk_exe_name: Option<&str>) -> bool {
    let lower = name.to_lowercase();
    if lower.is_empty() {
        return true;
    }
    if SESSION_KEEP_EXE.iter().any(|keep| *keep == lower) {
        return true;
    }
    if let Some(kiosk) = kiosk_exe_name {
        if lower == kiosk.to_lowercase() {
            return true;
        }
    }
    false
}

fn kill_pid(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
    }
}

pub fn main_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window("main")
        .or_else(|| app.webview_windows().values().next().cloned())
        .ok_or_else(|| "Main window not found".to_string())
}

pub fn kiosk_hwnd(app: &AppHandle) -> Option<isize> {
    #[cfg(windows)]
    {
        main_window(app)
            .ok()
            .and_then(|w| w.hwnd().ok())
            .map(|h| h.0 as isize)
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        None
    }
}

#[cfg(target_os = "windows")]
pub fn is_kiosk_hwnd(app: &AppHandle, hwnd: isize) -> bool {
    kiosk_hwnd(app) == Some(hwnd)
}

#[cfg(not(target_os = "windows"))]
pub fn is_kiosk_hwnd(_app: &AppHandle, _hwnd: isize) -> bool {
    false
}

pub fn set_last_allowed_hwnd(hwnd: isize) {
    if let Ok(mut guard) = LAST_ALLOWED_HWND.lock() {
        *guard = Some(hwnd);
    }
}

pub fn last_allowed_foreground_hwnd(app: &AppHandle) -> Option<isize> {
    if let Ok(guard) = LAST_ALLOWED_HWND.lock() {
        if let Some(hwnd) = *guard {
            #[cfg(windows)]
            {
                use windows::Win32::Foundation::HWND;
                use windows::Win32::UI::WindowsAndMessaging::IsWindow;

                let hwnd = HWND(hwnd as *mut _);
                if unsafe { IsWindow(hwnd).as_bool() } {
                    return Some(hwnd.0 as isize);
                }
            }
            #[cfg(not(windows))]
            {
                return Some(hwnd);
            }
        }
    }
    tracked_game_hwnd().or_else(|| kiosk_hwnd(app))
}

fn tracked_game_hwnd() -> Option<isize> {
    TRACKED
        .lock()
        .ok()
        .and_then(|tracked| {
            tracked
                .iter()
                .rev()
                .find_map(|entry| entry.window_handle)
        })
}

pub fn is_pid_tracked(pid: u32) -> bool {
    TRACKED
        .lock()
        .map(|tracked| {
            tracked.iter().any(|e| {
                e.root_pid == pid || e.descendant_pids.iter().any(|&p| p == pid)
            })
        })
        .unwrap_or(false)
}

fn all_tree_pids(entries: &[WatchEntry]) -> Vec<u32> {
    let mut pids = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        for pid in std::iter::once(entry.root_pid).chain(entry.descendant_pids.iter().copied()) {
            if seen.insert(pid) {
                pids.push(pid);
            }
        }
    }
    pids
}

#[cfg(windows)]
fn kill_process_trees(entries: &[WatchEntry]) {
    let mut system = System::new();
    refresh_system_processes(&mut system);
    for entry in entries {
        let tree = process_tree_pids(entry.root_pid, &system);
        for pid in tree.iter().rev() {
            kill_pid(*pid);
        }
    }
}

#[cfg(not(windows))]
fn kill_process_trees(entries: &[WatchEntry]) {
    for entry in entries {
        kill_pid(entry.root_pid);
    }
}

#[cfg(target_os = "windows")]
fn window_area(hwnd: windows::Win32::Foundation::HWND) -> i32 {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut rect = RECT::default();
    unsafe {
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return 0;
        }
    }
    (rect.right - rect.left).saturating_mul(rect.bottom - rect.top)
}

#[cfg(target_os = "windows")]
fn primary_monitor_area() -> i32 {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};
    use windows::Win32::Foundation::POINT;

    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return 1920 * 1080;
        }
        let rect = info.rcMonitor;
        (rect.right - rect.left).saturating_mul(rect.bottom - rect.top)
    }
}

#[cfg(not(target_os = "windows"))]
fn primary_monitor_area() -> i32 {
    1920 * 1080
}

#[cfg(target_os = "windows")]
fn process_exe_name(pid: u32) -> Option<String> {
    use sysinfo::{Pid, ProcessesToUpdate, System};

    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
        .process(Pid::from_u32(pid))
        .map(|p| p.name().to_string_lossy().to_lowercase())
}

#[cfg(not(target_os = "windows"))]
fn process_exe_name(_pid: u32) -> Option<String> {
    None
}

#[cfg(target_os = "windows")]
fn is_overlay_hwnd(hwnd: windows::Win32::Foundation::HWND, min_primary_area: i32) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetWindowLongPtrW, GetWindowTextW, GetWindowThreadProcessId, GWL_EXSTYLE,
        WS_EX_TOOLWINDOW,
    };

    unsafe {
        let area = window_area(hwnd);
        if area > 0 && area < min_primary_area / 4 {
            return true;
        }

        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return true;
        }

        let mut class_buf = [0u16; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buf);
        if class_len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]).to_lowercase();
            if class_name.contains("rclient") || class_name.contains("riot") {
                return true;
            }
        }

        let mut title_buf = [0u16; 256];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        if title_len > 0 {
            let title = String::from_utf16_lossy(&title_buf[..title_len as usize]).to_lowercase();
            if title.contains("riot client") {
                return true;
            }
        }

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if let Some(exe) = process_exe_name(pid) {
            if is_overlay_launcher_exe(&exe) {
                return true;
            }
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
fn is_overlay_hwnd(_hwnd: isize, _min_primary_area: i32) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn best_visible_hwnd_for_pids(pids: &[u32]) -> Option<isize> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    };

    if pids.is_empty() {
        return None;
    }

    let pid_set: HashSet<u32> = pids.iter().copied().collect();
    let min_primary_area = primary_monitor_area();

    struct Search {
        pid_set: HashSet<u32>,
        best_hwnd: HWND,
        best_area: i32,
        min_primary_area: i32,
    }

    unsafe extern "system" fn enum_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &mut *(lparam.0 as *mut Search);
        let mut window_pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        if !search.pid_set.contains(&window_pid) || !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        if is_overlay_hwnd(hwnd, search.min_primary_area) {
            return BOOL(1);
        }
        let area = window_area(hwnd);
        if area > search.best_area {
            search.best_area = area;
            search.best_hwnd = hwnd;
        }
        BOOL(1)
    }

    let mut search = Search {
        pid_set,
        best_hwnd: HWND(std::ptr::null_mut()),
        best_area: 0,
        min_primary_area,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows),
            LPARAM(&mut search as *mut Search as isize),
        );
    }
    if search.best_hwnd.0.is_null() {
        None
    } else {
        Some(search.best_hwnd.0 as isize)
    }
}

#[cfg(target_os = "windows")]
fn refresh_entry_window_handle(entry: &mut WatchEntry) {
    let mut pids: Vec<u32> = entry.descendant_pids.clone();
    pids.push(entry.root_pid);
    pids.sort_unstable();
    pids.dedup();
    // Prefer descendant game windows over the launcher root when both are visible.
    pids.reverse();
    if let Some(hwnd) = best_visible_hwnd_for_pids(&pids) {
        entry.window_handle = Some(hwnd);
    }
}

#[cfg(target_os = "windows")]
pub fn refresh_tracked_state() {
    let Ok(mut tracked) = TRACKED.lock() else {
        return;
    };
    let mut system = System::new();
    refresh_system_processes(&mut system);
    for entry in tracked.iter_mut() {
        refresh_watch_entry(entry, &system);
        refresh_entry_window_handle(entry);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn refresh_tracked_state() {}

#[cfg(target_os = "windows")]
pub fn bring_hwnd_to_foreground(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::AttachThreadInput;
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
        ShowWindow, SW_RESTORE,
    };

    unsafe {
        let hwnd = HWND(hwnd as *mut _);
        if hwnd.0.is_null() {
            return;
        }
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let target_thread = GetWindowThreadProcessId(hwnd, None);
        let attached = fg_thread != target_thread
            && AttachThreadInput(fg_thread, target_thread, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(fg_thread, target_thread, false);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn bring_hwnd_to_foreground(_hwnd: isize) {}

#[cfg(target_os = "windows")]
fn fullscreen_process_window(pid: u32) -> Option<isize> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_STYLE, HWND_NOTOPMOST,
        SWP_FRAMECHANGED, SWP_SHOWWINDOW, SW_MAXIMIZE, SW_RESTORE, WS_CAPTION, WS_MAXIMIZEBOX,
        WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
    };

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
        // Normal Z band — never TOPMOST so Alt+Tab respects user selection.
        let _ = SetWindowPos(
            hwnd,
            HWND_NOTOPMOST,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        );
    }

    for _ in 0..24 {
        if let Some(hwnd) = best_visible_hwnd_for_pids(&[pid]) {
            let hwnd = HWND(hwnd as *mut _);
            unsafe {
                make_borderless_fullscreen(hwnd);
            }
            set_last_allowed_hwnd(hwnd.0 as isize);
            return Some(hwnd.0 as isize);
        }
        thread::sleep(Duration::from_millis(250));
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn fullscreen_process_window(_pid: u32) -> Option<isize> {
    None
}

#[cfg(windows)]
fn set_foreground_window(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_RESTORE, SW_SHOW};

    if let Ok(hwnd_raw) = window.hwnd() {
        unsafe {
            let hwnd = HWND(hwnd_raw.0 as *mut _);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        bring_hwnd_to_foreground(hwnd_raw.0 as isize);
        set_last_allowed_hwnd(hwnd_raw.0 as isize);
    }
}

/// Lower the kiosk below launched game windows without hiding or minimizing it.
pub fn send_kiosk_behind_games(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, HWND_NOTOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        };

        if let Ok(hwnd_raw) = window.hwnd() {
            unsafe {
                let hwnd = HWND(hwnd_raw.0 as *mut _);
                let _ = SetWindowPos(
                    hwnd,
                    HWND_NOTOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = window;
    }
}

/// Fullscreen shell — idle/login or after all games closed (no TOPMOST).
pub fn apply_kiosk_shell_lockdown(app: &AppHandle) {
    if let Ok(window) = main_window(app) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_decorations(false);
        let _ = window.set_fullscreen(true);
        let _ = window.set_always_on_top(false);
        let _ = window.set_focus();
        #[cfg(windows)]
        set_foreground_window(&window);
        if let Some(hwnd) = kiosk_hwnd(app) {
            set_last_allowed_hwnd(hwnd);
        }
    }
}

/// User chose kiosk while games are tracked — foreground without TOPMOST.
pub fn apply_kiosk_session_foreground(app: &AppHandle) {
    if let Ok(window) = main_window(app) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_decorations(false);
        let _ = window.set_always_on_top(false);
        let _ = window.set_fullscreen(true);
        let _ = window.set_focus();
        #[cfg(windows)]
        {
            set_foreground_window(&window);
            suppress_tracked_overlays();
        }
        if let Some(hwnd) = kiosk_hwnd(app) {
            set_last_allowed_hwnd(hwnd);
        }
    }
}

/// After game launch — kiosk stays in Alt+Tab set but behind the game.
pub fn apply_kiosk_game_mode_background(app: &AppHandle) {
    if let Ok(window) = main_window(app) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_decorations(false);
        let _ = window.set_always_on_top(false);
        let _ = window.set_fullscreen(true);
        send_kiosk_behind_games(&window);
    }
}

fn show_kiosk_foreground(app: &AppHandle) {
    if has_tracked_processes() {
        apply_kiosk_session_foreground(app);
    } else {
        apply_kiosk_shell_lockdown(app);
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

/// Re-apply the correct window profile when the kiosk regains focus (e.g. after sound settings).
pub fn on_kiosk_focused(app: &AppHandle) {
    if has_tracked_processes() {
        apply_kiosk_session_foreground(app);
    } else if crate::lockdown::is_locked() {
        apply_kiosk_shell_lockdown(app);
    }
    if crate::lockdown::is_locked() {
        crate::lockdown::shell::hide_shell_chrome();
        #[cfg(windows)]
        if has_tracked_processes() {
            suppress_tracked_overlays();
        }
    }
}

/// Lower kiosk for external UI (sound settings) without minimizing.
pub fn yield_kiosk_for_external_ui(app: &AppHandle) -> Result<(), String> {
    let window = main_window(app)?;
    window
        .set_always_on_top(false)
        .map_err(|e| e.to_string())?;
    window.set_fullscreen(false).map_err(|e| e.to_string())?;
    send_kiosk_behind_games(&window);
    Ok(())
}

fn prepare_kiosk_for_game_mode(app: &AppHandle) -> Result<(), String> {
    apply_kiosk_game_mode_background(app);
    Ok(())
}

fn restore_kiosk_window(app: &AppHandle) {
    apply_kiosk_shell_lockdown(app);
}

#[cfg(windows)]
fn suppress_tracked_overlays() {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetWindowPos, ShowWindow,
        HWND_BOTTOM, SW_MINIMIZE, SWP_NOACTIVATE, SWP_NOSIZE,
    };

    let pids = all_tree_pids(
        TRACKED
            .lock()
            .map(|t| t.clone())
            .unwrap_or_default()
            .as_slice(),
    );
    if pids.is_empty() {
        return;
    }
    let pid_set: HashSet<u32> = pids.into_iter().collect();
    let min_primary_area = primary_monitor_area();

    struct Search {
        pid_set: HashSet<u32>,
        min_primary_area: i32,
    }

    unsafe extern "system" fn enum_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &*(lparam.0 as *const Search);
        let mut window_pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        if !search.pid_set.contains(&window_pid) || !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        if is_overlay_hwnd(hwnd, search.min_primary_area) {
            let _ = ShowWindow(hwnd, SW_MINIMIZE);
            let _ = SetWindowPos(
                hwnd,
                HWND_BOTTOM,
                0,
                0,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        BOOL(1)
    }

    let search = Search {
        pid_set,
        min_primary_area,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows),
            LPARAM(&search as *const Search as isize),
        );
    }
}

#[cfg(not(windows))]
fn suppress_tracked_overlays() {}

#[cfg(windows)]
fn terminate_user_session_apps() -> u32 {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    use windows::Win32::System::Environment::GetCurrentProcessId;
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;

    let kiosk_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));

    let mut session_id = 0u32;
    unsafe {
        let _ = ProcessIdToSessionId(GetCurrentProcessId(), Some(&mut session_id));
    }

    let mut killed = 0u32;
    for pass in 0..3 {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);
        let mut targets = Vec::new();
        for (pid, process) in system.processes() {
            let pid_u32 = pid.as_u32();
            if pid_u32 <= 4 {
                continue;
            }
            let mut proc_session = 0u32;
            unsafe {
                if ProcessIdToSessionId(pid_u32, Some(&mut proc_session)).is_err()
                    || proc_session != session_id
                {
                    continue;
                }
            }
            let name = process.name().to_string_lossy();
            if is_session_keep_process(&name, kiosk_exe.as_deref()) {
                continue;
            }
            targets.push(pid_u32);
        }
        if targets.is_empty() {
            break;
        }
        for pid in targets {
            kill_pid(pid);
            killed += 1;
        }
        if pass < 2 {
            thread::sleep(Duration::from_millis(500));
        }
    }
    killed
}

#[cfg(not(windows))]
fn terminate_user_session_apps() -> u32 {
    0
}

fn session_end_cleanup(app: &AppHandle) -> KillResult {
    let entries: Vec<WatchEntry> = TRACKED
        .lock()
        .map(|t| t.clone())
        .unwrap_or_default();
    let mut killed = all_tree_pids(&entries).len() as u32;
    if !entries.is_empty() {
        kill_process_trees(&entries);
    }
    killed += terminate_user_session_apps();
    if let Ok(mut tracked) = TRACKED.lock() {
        tracked.clear();
    }
    if let Ok(mut guard) = LAST_ALLOWED_HWND.lock() {
        *guard = None;
    }
    crate::boost::restore_game_boost();
    restore_kiosk_window(app);
    KillResult {
        killed,
        restored: true,
    }
}

pub fn recover_minimized_kiosk(app: &AppHandle) {
    if !crate::lockdown::is_locked() {
        return;
    }
    if let Ok(window) = main_window(app) {
        if window.is_minimized().unwrap_or(false) {
            if has_tracked_processes() {
                apply_kiosk_session_foreground(app);
            } else {
                apply_kiosk_shell_lockdown(app);
            }
        }
    }
}

fn start_monitor_if_needed(app: AppHandle) {
    let mut running = MONITOR_RUNNING.lock().expect("monitor lock");
    if *running {
        return;
    }
    *running = true;
    drop(running);

    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(1));
        let is_empty = {
            let mut tracked = TRACKED.lock().expect("tracked lock");
            #[cfg(windows)]
            {
                let mut system = System::new();
                refresh_system_processes(&mut system);
                for entry in tracked.iter_mut() {
                    refresh_watch_entry(entry, &system);
                    refresh_entry_window_handle(entry);
                }
                tracked.retain(|entry| entry_has_live_process(entry, &system));
            }
            #[cfg(not(windows))]
            {
                tracked.retain(|entry| entry_has_live_process(entry));
            }
            tracked.is_empty()
        };
        if is_empty {
            crate::boost::restore_game_boost();
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
    arguments: Option<Vec<String>>,
) -> Result<LaunchResult, String> {
    if !is_allowed(&executable_path, &allow_list) {
        return Err("Executable not in allow-list".to_string());
    }
    prepare_kiosk_for_game_mode(&app)?;
    crate::boost::prepare_game_boost();
    let pid = spawn_process(
        &executable_path,
        normalize_launch_arguments(arguments).as_deref(),
    )?;
    crate::boost::apply_game_priority(pid);
    let window_handle = if is_launcher_executable(&executable_path) {
        None
    } else {
        fullscreen_process_window(pid)
    };
    apply_kiosk_game_mode_background(&app);
    TRACKED.lock().map_err(|e| e.to_string())?.push(WatchEntry {
        executable_path,
        root_pid: pid,
        descendant_pids: Vec::new(),
        window_handle,
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
            pid: e.root_pid,
            executable_path: e.executable_path.clone(),
        })
        .collect())
}

#[tauri::command]
pub fn kill_tracked_processes(app: AppHandle) -> Result<KillResult, String> {
    Ok(session_end_cleanup(&app))
}

#[tauri::command]
pub fn clear_tracked_processes() -> Result<(), String> {
    TRACKED.lock().map_err(|e| e.to_string())?.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        all_tree_pids, has_tracked_processes, is_allowed, is_launcher_executable,
        is_overlay_launcher_exe, is_session_keep_process, normalize_path, WatchEntry,
    };

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

    #[test]
    fn has_tracked_processes_false_when_empty() {
        let _ = clear_tracked_for_test();
        assert!(!has_tracked_processes());
    }

    #[test]
    fn is_launcher_executable_detects_steam_and_riot() {
        assert!(is_launcher_executable(
            "C:\\Program Files (x86)\\Steam\\steam.exe"
        ));
        assert!(is_launcher_executable(
            "C:\\Riot Games\\Riot Client\\RiotClientServices.exe"
        ));
        assert!(is_launcher_executable(
            "C:\\Program Files\\Electronic Arts\\EA Desktop\\EA Desktop\\EALaunchHelper.exe"
        ));
        assert!(!is_launcher_executable(
            "C:\\Riot Games\\VALORANT\\live\\VALORANT.exe"
        ));
    }


    #[test]
    fn is_overlay_launcher_exe_detects_riot_ux() {
        assert!(is_overlay_launcher_exe("riotclientux.exe"));
        assert!(is_overlay_launcher_exe("steamwebhelper.exe"));
        assert!(!is_overlay_launcher_exe("valorant-win64-shipping.exe"));
    }

    #[test]
    fn is_session_keep_process_skips_system_and_kiosk() {
        assert!(is_session_keep_process("explorer.exe", Some("Arena360 Kiosk.exe")));
        assert!(is_session_keep_process(
            "Arena360 Kiosk.exe",
            Some("Arena360 Kiosk.exe")
        ));
        assert!(!is_session_keep_process("discord.exe", Some("Arena360 Kiosk.exe")));
    }

    #[test]
    fn all_tree_pids_dedupes_roots_and_descendants() {
        let entries = vec![
            WatchEntry {
                executable_path: "a.exe".to_string(),
                root_pid: 100,
                descendant_pids: vec![101, 102],
                window_handle: None,
            },
            WatchEntry {
                executable_path: "b.exe".to_string(),
                root_pid: 200,
                descendant_pids: vec![201],
                window_handle: None,
            },
        ];
        let pids = all_tree_pids(&entries);
        assert_eq!(pids.len(), 5);
        assert!(pids.contains(&100));
        assert!(pids.contains(&102));
        assert!(pids.contains(&201));
    }

    fn clear_tracked_for_test() {
        let _ = super::clear_tracked_processes();
    }
}
