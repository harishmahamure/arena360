//! Low-level keyboard hook used during lockdown.
//!
//! Blocks global Windows shell shortcuts that can break out of the kiosk:
//! both Windows keys and any `Win`+key combo, Alt+Esc, Alt+F4, Alt+Space
//! (system menu), Ctrl+Esc (Start menu), Ctrl+Shift+Esc (Task Manager), and
//! the Menu/Apps (context-menu) key. Alt+Tab is intentionally allowed so
//! players can switch between approved launched apps. **Ctrl+Shift+H** raises
//! the kiosk above launched games without closing them. **Ctrl+Shift+A** opens
//! administrator setup (emits `enter-setup` to the webview). **Limitation:**
//! Ctrl+Alt+Del (the Secure Attention Sequence) cannot be intercepted from user
//! mode by design — it always reaches the Windows Secure Desktop. Full CAD
//! suppression requires the `DisableLockWorkstation` /
//! `DisableTaskMgr` group-policy keys plus a kiosk/assigned-access account; this
//! is documented for deployment and is out of scope for the app binary. After a
//! user returns from the Secure Desktop we re-assert fullscreen on window focus
//! (see `apply_window_mode`).

#[cfg(target_os = "windows")]
mod win {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::OnceLock;
    use std::thread;
    use std::time::{Duration, Instant};
    use tauri::{AppHandle, Emitter};
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_A, VK_APPS, VK_CONTROL, VK_ESCAPE, VK_F4, VK_H, VK_LWIN, VK_MENU,
        VK_RWIN, VK_SHIFT, VK_SPACE, VK_TAB,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    /// Newtype so the hook handle can live in a `static OnceLock`. `HHOOK`
    /// wraps a raw `*mut c_void` (so it is `!Send + !Sync`), but the handle is
    /// an opaque value only ever installed/removed/used on the main UI thread,
    /// so sharing it is sound.
    struct HookHandle(HHOOK);
    unsafe impl Send for HookHandle {}
    unsafe impl Sync for HookHandle {}

    static HOOK: OnceLock<HookHandle> = OnceLock::new();
    static ENABLED: AtomicBool = AtomicBool::new(false);
    static ALT_REHIDE_RUNNING: AtomicBool = AtomicBool::new(false);
    static APP: OnceLock<AppHandle> = OnceLock::new();

    const ALT_REHIDE_INTERVAL_MS: u64 = 75;
    const ALT_REHIDE_MAX_MS: u64 = 3000;

    pub fn set_app_handle(app: AppHandle) {
        let _ = APP.set(app);
    }

    /// `true` if `vk` is currently held down (high bit of the async key state).
    unsafe fn is_down(vk: i32) -> bool {
        GetAsyncKeyState(vk) & 0x8000u16 as i16 != 0
    }

    /// Decide whether a key-down event is a global Windows shortcut that must be
    /// swallowed during lockdown. `Ctrl+Alt+Del` is intentionally absent — it is
    /// a Secure Attention Sequence the OS handles before any user-mode hook.
    unsafe fn should_block(vk: u32) -> bool {
        let alt = is_down(VK_MENU.0 as i32);
        let ctrl = is_down(VK_CONTROL.0 as i32);
        let win = is_down(VK_LWIN.0 as i32) || is_down(VK_RWIN.0 as i32);

        // Either Windows key, or any combo while a Windows key is held
        // (Win+R/E/D/L/Tab/number, etc.).
        if vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32 || win {
            return true;
        }
        // Context-menu (Apps) key.
        if vk == VK_APPS.0 as u32 {
            return true;
        }
        // Alt-based shell escapes. Alt+Tab is deliberately allowed so players
        // can switch between approved launched applications.
        if alt && (vk == VK_ESCAPE.0 as u32 || vk == VK_F4.0 as u32 || vk == VK_SPACE.0 as u32) {
            return true;
        }
        // Ctrl+Esc (Start menu) and Ctrl+Shift+Esc (Task Manager).
        if ctrl && vk == VK_ESCAPE.0 as u32 {
            return true;
        }
        false
    }

    /// Ctrl+Shift+H — return to the kiosk shell while launched apps keep running.
    unsafe fn is_focus_kiosk_combo(vk: u32) -> bool {
        vk == VK_H.0 as u32
            && is_down(VK_CONTROL.0 as i32)
            && is_down(VK_SHIFT.0 as i32)
    }

    /// Ctrl+Shift+A — open administrator setup (handled in the webview via event).
    unsafe fn is_enter_setup_combo(vk: u32) -> bool {
        vk == VK_A.0 as u32
            && is_down(VK_CONTROL.0 as i32)
            && is_down(VK_SHIFT.0 as i32)
    }

    /// Alt+Tab is allowed through, but Explorer often flashes the taskbar when the switcher opens.
    unsafe fn is_alt_tab(vk: u32) -> bool {
        vk == VK_TAB.0 as u32 && is_down(VK_MENU.0 as i32)
    }

    /// Re-hide the taskbar while Alt is held — Explorer re-shows it when the switcher opens.
    fn start_alt_rehide_burst() {
        if ALT_REHIDE_RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }
        thread::spawn(|| {
            let deadline = Instant::now() + Duration::from_millis(ALT_REHIDE_MAX_MS);
            loop {
                crate::lockdown::shell::hide_shell_chrome();
                let alt_held = unsafe { is_down(VK_MENU.0 as i32) };
                if !alt_held || Instant::now() >= deadline {
                    break;
                }
                thread::sleep(Duration::from_millis(ALT_REHIDE_INTERVAL_MS));
            }
            crate::lockdown::shell::hide_shell_chrome();
            ALT_REHIDE_RUNNING.store(false, Ordering::SeqCst);
        });
    }

    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && ENABLED.load(Ordering::SeqCst) {
            let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
            let is_keydown =
                wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize;
            let is_keyup = wparam.0 == WM_KEYUP as usize || wparam.0 == WM_SYSKEYUP as usize;

            if is_keyup && kb.vkCode == VK_MENU.0 as u32 {
                crate::lockdown::shell::hide_shell_chrome();
                if let Some(app) = APP.get() {
                    crate::lockdown::foreground::capture_allowed_foreground(app);
                }
            }

            if is_keydown {
                if is_focus_kiosk_combo(kb.vkCode) {
                    if let Some(app) = APP.get() {
                        let _ = crate::process::focus_kiosk_window(app);
                    }
                    return LRESULT(1);
                }
                if is_enter_setup_combo(kb.vkCode) {
                    if let Some(app) = APP.get() {
                        let _ = crate::process::focus_kiosk_window(app);
                        let _ = app.emit("enter-setup", ());
                    }
                    return LRESULT(1);
                }
                if should_block(kb.vkCode) {
                    return LRESULT(1);
                }
                if is_alt_tab(kb.vkCode) {
                    crate::lockdown::shell::hide_shell_chrome();
                    start_alt_rehide_burst();
                }
            }
        }

        CallNextHookEx(
            HOOK.get().map(|h| h.0).unwrap_or_default(),
            code,
            wparam,
            lparam,
        )
    }

    pub fn install() {
        if ENABLED.swap(true, Ordering::SeqCst) {
            return;
        }
        unsafe {
            let module = GetModuleHandleW(None).unwrap_or_default();
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), module, 0)
                .expect("keyboard hook");
            let _ = HOOK.set(HookHandle(hook));
        }
    }

    pub fn remove() {
        ENABLED.store(false, Ordering::SeqCst);
        if let Some(hook) = HOOK.get() {
            unsafe {
                let _ = UnhookWindowsHookEx(hook.0);
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub fn set_app_handle(app: tauri::AppHandle) {
    win::set_app_handle(app);
}

#[cfg(not(target_os = "windows"))]
pub fn set_app_handle(_app: tauri::AppHandle) {}

#[cfg(target_os = "windows")]
pub fn install_hook() {
    win::install();
}

#[cfg(target_os = "windows")]
pub fn remove_hook() {
    win::remove();
}

#[cfg(not(target_os = "windows"))]
pub fn install_hook() {}

#[cfg(not(target_os = "windows"))]
pub fn remove_hook() {}
