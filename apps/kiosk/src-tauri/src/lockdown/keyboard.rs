//! Low-level keyboard hook used during lockdown.
//!
//! Blocks global Windows shell shortcuts that can break out of the kiosk:
//! both Windows keys and any `Win`+key combo, Alt+Esc, Alt+F4, Alt+Space
//! (system menu), Ctrl+Esc (Start menu), Ctrl+Shift+Esc (Task Manager), and
//! the Menu/Apps (context-menu) key. Alt+Tab is intentionally allowed so
//! players can switch between approved launched apps. **Limitation:**
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
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_APPS, VK_CONTROL, VK_ESCAPE, VK_F4, VK_LWIN, VK_MENU, VK_RWIN,
        VK_SPACE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
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

    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && ENABLED.load(Ordering::SeqCst) {
            let is_keydown = wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize;
            if is_keydown {
                let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
                if should_block(kb.vkCode) {
                    return LRESULT(1);
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
