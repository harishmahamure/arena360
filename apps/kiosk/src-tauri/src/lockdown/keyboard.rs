//! Low-level keyboard hook used during lockdown.
//!
//! Blocks Alt+Tab, the Windows keys, Alt+F4, and Ctrl+Shift+Esc. **Limitation:**
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
        GetAsyncKeyState, VK_LWIN, VK_MENU, VK_RWIN, VK_TAB,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
    };

    static HOOK: OnceLock<HHOOK> = OnceLock::new();
    static ENABLED: AtomicBool = AtomicBool::new(false);

    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && ENABLED.load(Ordering::SeqCst) {
            let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode;

            // Block Alt+Tab, Win keys, Ctrl+Esc (0x1B with ctrl), Alt+F4
            let alt_down = GetAsyncKeyState(VK_MENU.0 as i32) & 0x8000u16 as i16 != 0;
            let win_down = GetAsyncKeyState(VK_LWIN.0 as i32) & 0x8000u16 as i16 != 0
                || GetAsyncKeyState(VK_RWIN.0 as i32) & 0x8000u16 as i16 != 0;

            if vk == VK_TAB.0 as u32 && alt_down {
                return LRESULT(1);
            }
            if vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32 {
                return LRESULT(1);
            }
            if vk == 0x1B && alt_down {
                // Alt+F4
                return LRESULT(1);
            }
            if vk == 0x1B && win_down {
                // Win+ shortcuts that include escape menu
                return LRESULT(1);
            }
            if win_down {
                return LRESULT(1);
            }

            if wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize {
                // Block Ctrl+Shift+Esc (task manager) — vk 0x1B with ctrl+shift
                if vk == 0x1B {
                    let ctrl = GetAsyncKeyState(0x11) & 0x8000u16 as i16 != 0;
                    let shift = GetAsyncKeyState(0x10) & 0x8000u16 as i16 != 0;
                    if ctrl && shift {
                        return LRESULT(1);
                    }
                }
            }
        }

        CallNextHookEx(HOOK.get().copied().unwrap_or_default(), code, wparam, lparam)
    }

    pub fn install() {
        if ENABLED.swap(true, Ordering::SeqCst) {
            return;
        }
        unsafe {
            let module = GetModuleHandleW(None).unwrap_or_default();
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), module, 0)
                .expect("keyboard hook");
            let _ = HOOK.set(hook);
        }
    }

    pub fn remove() {
        ENABLED.store(false, Ordering::SeqCst);
        if let Some(hook) = HOOK.get() {
            unsafe {
                let _ = UnhookWindowsHookEx(*hook);
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
