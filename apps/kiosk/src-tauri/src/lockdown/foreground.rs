//! React to Windows focus/window events, with a slow fallback for missed events.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;

static AUDIO_UI_YIELD: AtomicBool = AtomicBool::new(false);

#[cfg(any(windows, test))]
fn is_shell_desktop_class(class: &str) -> bool {
    matches!(
        class,
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

#[cfg(any(windows, test))]
fn should_scan_shell(locked: bool, tracked_app_foreground: bool) -> bool {
    locked && !tracked_app_foreground
}

// Bound and coalesce noisy window-show events before doing any window work.
#[cfg(any(windows, test))]
#[derive(Default)]
struct PendingChecks {
    foreground_changed: bool,
    shown_windows: std::collections::BTreeSet<isize>,
    overflowed: bool,
    wake_posted: bool,
}

#[cfg(any(windows, test))]
impl PendingChecks {
    const MAX_WINDOWS: usize = 128;

    fn show(&mut self, hwnd: isize) {
        if self.shown_windows.len() < Self::MAX_WINDOWS {
            self.shown_windows.insert(hwnd);
        } else if !self.shown_windows.contains(&hwnd) {
            self.overflowed = true;
        }
    }

    fn needs_wake(&mut self) -> bool {
        if self.wake_posted {
            return false;
        }
        self.wake_posted = true;
        true
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::{is_shell_desktop_class, should_scan_shell, PendingChecks};
    use crate::process;
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Mutex;
    use std::thread;
    use std::time::Duration;
    use tauri::AppHandle;
    use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetClassNameW, GetForegroundWindow, GetMessageW,
        GetWindowThreadProcessId, IsWindowVisible, KillTimer, PeekMessageW, PostThreadMessageW,
        SetTimer, TranslateMessage, EVENT_OBJECT_SHOW, EVENT_SYSTEM_FOREGROUND, MSG, OBJID_WINDOW,
        PM_NOREMOVE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_APP, WM_TIMER,
    };

    const FALLBACK_MS: u32 = 2_000;
    const WM_CHECK: u32 = WM_APP + 41;
    const WM_STOP: u32 = WM_APP + 42;
    static RUNNING: AtomicBool = AtomicBool::new(false);
    static GENERATION: AtomicU64 = AtomicU64::new(0);
    static THREAD: Mutex<Option<(u64, u32)>> = Mutex::new(None);
    // Stop waits for in-flight window work before setup restores the shell.
    static CHECK_LOCK: Mutex<()> = Mutex::new(());
    thread_local! { static PENDING: RefCell<PendingChecks> = RefCell::new(PendingChecks::default()); }

    fn current(generation: u64) -> bool {
        RUNNING.load(Ordering::SeqCst) && GENERATION.load(Ordering::SeqCst) == generation
    }

    struct Hooks(Vec<HWINEVENTHOOK>);

    impl Drop for Hooks {
        fn drop(&mut self) {
            // Windows requires removal on the thread that installed the hook.
            for hook in self.0.drain(..) {
                unsafe {
                    let _ = UnhookWinEvent(hook);
                }
            }
        }
    }

    unsafe extern "system" fn on_window_event(
        _hook: HWINEVENTHOOK,
        event: u32,
        hwnd: HWND,
        object: i32,
        child: i32,
        _thread: u32,
        _time: u32,
    ) {
        if event != EVENT_SYSTEM_FOREGROUND
            && (event != EVENT_OBJECT_SHOW
                || object != OBJID_WINDOW.0
                || child != 0
                || hwnd.0.is_null())
        {
            return;
        }
        // Out-of-context events arrive on our message thread. Only enqueue
        // here: Windows can reenter this callback during window operations.
        let wake = PENDING.with(|pending| {
            let mut pending = pending.borrow_mut();
            if event == EVENT_SYSTEM_FOREGROUND {
                pending.foreground_changed = true;
            } else {
                pending.show(hwnd.0 as isize);
            }
            pending.needs_wake()
        });
        if wake {
            let _ = PostThreadMessageW(GetCurrentThreadId(), WM_CHECK, WPARAM(0), LPARAM(0));
        }
    }

    fn install_hooks() -> Hooks {
        let mut hooks = Hooks(Vec::new());
        for (event, flags) in [
            (EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT),
            (
                EVENT_OBJECT_SHOW,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            ),
        ] {
            let hook = unsafe {
                SetWinEventHook(
                    event,
                    event,
                    HMODULE::default(),
                    Some(on_window_event),
                    0,
                    0,
                    flags,
                )
            };
            if hook.0.is_null() {
                crate::diagnostics::warn(format!(
                    "window event hook {event} unavailable; using fallback"
                ));
            } else {
                hooks.0.push(hook);
            }
        }
        hooks
    }

    fn check(app: &AppHandle, generation: u64, pending: PendingChecks, force_scan: bool) {
        let Ok(_check) = CHECK_LOCK.lock() else {
            return;
        };
        if !current(generation) || !crate::lockdown::is_locked() {
            return;
        }
        let foreground = unsafe { GetForegroundWindow() };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(foreground, Some(&mut pid));
        }
        let kiosk = process::is_kiosk_hwnd(app, foreground.0 as isize);
        let game = !kiosk && process::is_pid_tracked(pid);
        if (kiosk || game) && unsafe { IsWindowVisible(foreground).as_bool() } {
            process::set_last_allowed_hwnd(foreground.0 as isize);
        }

        // Inspect only newly shown windows during gameplay, so a taskbar or
        // restricted window can still be hidden without enumerating windows.
        for hwnd in pending.shown_windows {
            crate::lockdown::shell::hide_restricted_window(hwnd);
        }
        if !should_scan_shell(true, game) {
            return;
        }
        if force_scan || pending.foreground_changed || pending.overflowed {
            crate::lockdown::shell::hide_shell_chrome();
        }
        unsafe {
            let mut class = [0u16; 128];
            let len = GetClassNameW(foreground, &mut class);
            if len <= 0
                || !is_shell_desktop_class(&String::from_utf16_lossy(&class[..len as usize]))
            {
                return;
            }
        }
        // Recover desktop escapes only; application Alt+Tab never redirects.
        if let Some(hwnd) = process::last_allowed_foreground_hwnd(app) {
            process::bring_hwnd_to_foreground(hwnd);
        } else {
            let _ = process::focus_kiosk_window(app);
        }
    }

    fn dispatch_check(app: &AppHandle, generation: u64, pending: PendingChecks, force_scan: bool) {
        let handle = app.clone();
        // Tauri window methods may wait for the main thread. Running there
        // also lets stop serialize setup relaxation without deadlocking it.
        if let Err(error) =
            app.run_on_main_thread(move || check(&handle, generation, pending, force_scan))
        {
            crate::diagnostics::error(format!("window guard dispatch failed: {error}"));
        }
    }

    fn worker(app: AppHandle, generation: u64) {
        // Create the message queue before publishing its thread ID.
        let mut message = MSG::default();
        unsafe {
            let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        }
        let hooks = install_hooks();
        let timer = unsafe { SetTimer(None, 0, FALLBACK_MS, None) };
        if timer == 0 {
            drop(hooks);
            crate::diagnostics::warn("window guard timer unavailable; using sleeping fallback");
            while current(generation) {
                dispatch_check(&app, generation, PendingChecks::default(), true);
                thread::sleep(Duration::from_millis(FALLBACK_MS as u64));
            }
            return;
        }
        if let Ok(mut route) = THREAD.lock() {
            if current(generation) {
                *route = Some((generation, unsafe { GetCurrentThreadId() }));
            }
        }
        dispatch_check(&app, generation, PendingChecks::default(), true);
        while current(generation) {
            let result = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
            if result <= 0 {
                break;
            }
            if message.message == WM_STOP {
                break;
            }
            if message.message == WM_CHECK
                || (message.message == WM_TIMER && message.wParam.0 == timer)
            {
                let pending = PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut()));
                dispatch_check(
                    &app,
                    generation,
                    pending,
                    message.message == WM_TIMER || message.wParam.0 == 1,
                );
            } else {
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
        unsafe {
            let _ = KillTimer(None, timer);
        }
        drop(hooks);
        if let Ok(mut route) = THREAD.lock() {
            if route.is_some_and(|(owner, _)| owner == generation) {
                *route = None;
            }
        }
        if GENERATION.load(Ordering::SeqCst) == generation {
            RUNNING.store(false, Ordering::SeqCst);
        }
    }

    pub fn start(app: AppHandle) {
        if RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        if let Err(error) = thread::Builder::new()
            .name("kiosk-window-events".into())
            .spawn(move || worker(app, generation))
        {
            if GENERATION.load(Ordering::SeqCst) == generation {
                RUNNING.store(false, Ordering::SeqCst);
            }
            crate::diagnostics::error(format!("window guard start failed: {error}"));
        }
    }

    pub fn request_check() {
        if let Ok(route) = THREAD.lock() {
            if let Some((generation, thread)) = *route {
                if current(generation) {
                    unsafe {
                        let _ = PostThreadMessageW(thread, WM_CHECK, WPARAM(1), LPARAM(0));
                    }
                }
            }
        }
    }

    pub fn stop() {
        RUNNING.store(false, Ordering::SeqCst);
        GENERATION.fetch_add(1, Ordering::SeqCst);
        let _check = CHECK_LOCK.lock();
        if let Ok(mut route) = THREAD.lock() {
            if let Some((_, thread)) = route.take() {
                unsafe {
                    let _ = PostThreadMessageW(thread, WM_STOP, WPARAM(0), LPARAM(0));
                }
            }
        }
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
pub fn request_check() {
    win::request_check();
}
#[cfg(not(target_os = "windows"))]
pub fn start_foreground_guard(_app: AppHandle) {}
#[cfg(not(target_os = "windows"))]
pub fn stop_foreground_guard() {}

pub fn set_audio_ui_yield(active: bool) {
    AUDIO_UI_YIELD.store(active, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::{is_shell_desktop_class, should_scan_shell, PendingChecks};

    #[test]
    fn scans_resume_when_game_loses_focus_and_stop_in_setup() {
        assert!(!should_scan_shell(true, true)); // tracked game/launcher
        assert!(should_scan_shell(true, false)); // kiosk, desktop, or another app
        assert!(!should_scan_shell(false, false)); // staff setup
        assert!(!should_scan_shell(false, true));
    }

    #[test]
    fn event_bursts_are_bounded_coalesced_and_can_be_drained() {
        let mut pending = PendingChecks::default();
        assert!(pending.needs_wake());
        pending.foreground_changed = true;
        for hwnd in 1..=200 {
            pending.show(hwnd);
            assert!(!pending.needs_wake());
        }
        assert_eq!(pending.shown_windows.len(), PendingChecks::MAX_WINDOWS);
        assert!(pending.overflowed);
        let work = std::mem::take(&mut pending);
        assert!(work.foreground_changed);
        assert!(pending.shown_windows.is_empty());
        assert!(pending.needs_wake());
    }

    #[test]
    fn duplicate_window_events_do_not_overflow() {
        let mut pending = PendingChecks::default();
        for hwnd in 1..=PendingChecks::MAX_WINDOWS {
            pending.show(hwnd as isize);
        }
        pending.show(1);
        assert!(!pending.overflowed);
    }

    #[test]
    fn desktop_recovery_preserves_application_alt_tab_targets() {
        for class in [
            "Progman",
            "WorkerW",
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
        ] {
            assert!(is_shell_desktop_class(class));
        }
        for class in [
            "Chrome_WidgetWin_1",
            "Notepad",
            "UnrealWindow",
            "CabinetWClass",
            "#32770",
        ] {
            assert!(!is_shell_desktop_class(class));
        }
    }
}
