//! Low-level keyboard hook used during lockdown.
//!
//! Blocks global Windows shell shortcuts that can break out of the kiosk:
//! both Windows keys and any `Win`+key combo, Alt+Esc, Alt+F4, Alt+Space
//! (system menu), Ctrl+Esc (Start menu), Ctrl+Shift+Esc (Task Manager), and
//! the Menu/Apps (context-menu) key. Alt+Tab is intentionally allowed so
//! players can switch between approved launched apps. Staff actions (setup,
//! lockout reset) use login-screen buttons only — no global Ctrl+Shift combos.
//! **Limitation:** Ctrl+Alt+Del (the Secure Attention Sequence) cannot be
//! intercepted from user mode by design — it always reaches the Windows Secure
//! Desktop. Full suppression requires Windows Keyboard Filter on a supported
//! Windows edition; the deployment script configures this OS-level restriction. After a
//! user returns from the Secure Desktop we re-assert fullscreen on window focus
//! (see `apply_window_mode`).

// Virtual-key values are platform independent so the policy can be tested on CI.
#[cfg(any(windows, test))]
fn blocks_shortcut(vk: u32, alt: bool, ctrl: bool, win: bool) -> bool {
    matches!(vk, 0x5B | 0x5C | 0x5D)
        || win
        || (alt && matches!(vk, 0x1B | 0x73 | 0x20))
        || (ctrl && vk == 0x1B)
        || (alt && ctrl && vk == 0x09)
        || matches!(vk, 0xB4..=0xB7)
}

#[cfg(any(windows, test))]
#[derive(Default)]
struct Modifiers {
    ctrl: u8,
    win: u8,
}

#[cfg(any(windows, test))]
impl Modifiers {
    fn update(&mut self, vk: u32, down: bool) {
        let (state, bit) = match vk {
            0x11 | 0xA2 => (&mut self.ctrl, 1),
            0xA3 => (&mut self.ctrl, 2),
            0x5B => (&mut self.win, 1),
            0x5C => (&mut self.win, 2),
            _ => return,
        };
        if down {
            *state |= bit;
        } else {
            *state &= !bit;
        }
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::{blocks_shortcut, Modifiers};
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Mutex};
    use std::thread;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW,
        GetWindowThreadProcessId, KillTimer, SetTimer, SetWindowsHookExW, TranslateMessage,
        UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, LLKHF_ALTDOWN, MSG, WH_KEYBOARD_LL,
        WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
    };

    static INSTALLED: Mutex<bool> = Mutex::new(false);
    static ENABLED: AtomicBool = AtomicBool::new(false);
    thread_local! { static MODIFIERS: RefCell<Modifiers> = RefCell::new(Modifiers::default()); }

    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            let up = matches!(wparam.0 as u32, WM_KEYUP | WM_SYSKEYUP);
            if down || up {
                let (ctrl, win) = MODIFIERS.with(|state| {
                    let mut state = state.borrow_mut();
                    state.update(kb.vkCode, down);
                    (state.ctrl != 0, state.win != 0)
                });
                if ENABLED.load(Ordering::SeqCst) {
                    // The event carries Alt state; GetAsyncKeyState is not
                    // updated yet when the low-level callback runs.
                    let alt = kb.flags.contains(LLKHF_ALTDOWN);
                    if (up && matches!(kb.vkCode, 0x5B | 0x5C))
                        || (down && blocks_shortcut(kb.vkCode, alt, ctrl, win))
                    {
                        return LRESULT(1);
                    }
                    if (down && kb.vkCode == 0x09 && alt)
                        || (up && matches!(kb.vkCode, 0x12 | 0xA4 | 0xA5))
                    {
                        // Queue one check; window events handle subsequent
                        // focus changes without an Alt+Tab polling burst.
                        crate::lockdown::foreground::request_check();
                    }
                }
            }
        }
        CallNextHookEx(HHOOK::default(), code, wparam, lparam)
    }

    pub fn remove() {
        ENABLED.store(false, Ordering::SeqCst);
    }

    pub fn install() -> Result<(), String> {
        let mut installed = INSTALLED.lock().map_err(|e| e.to_string())?;
        if !*installed {
            let (tx, rx) = mpsc::sync_channel(1);
            thread::Builder::new()
                .name("kiosk-keyboard".into())
                .spawn(move || unsafe {
                    // Cover modifiers already held when the kiosk first starts.
                    MODIFIERS.with(|state| {
                        let mut state = state.borrow_mut();
                        for vk in [0xA2, 0xA3, 0x5B, 0x5C] {
                            state.update(vk, GetAsyncKeyState(vk as i32) < 0);
                        }
                    });
                    let module = GetModuleHandleW(None).unwrap_or_default();
                    let mut hook =
                        match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), module, 0) {
                            Ok(hook) => hook,
                            Err(e) => {
                                let _ = tx.send(Err(format!("keyboard hook install failed: {e}")));
                                return;
                            }
                        };
                    let timer = SetTimer(None, 0, 1000, None);
                    if timer == 0 {
                        let _ = UnhookWindowsHookEx(hook);
                        let _ = tx.send(Err("keyboard hook recovery timer failed".into()));
                        return;
                    }
                    let _ = tx.send(Ok(()));
                    let mut message = MSG::default();
                    while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                        if message.message == WM_TIMER && message.wParam.0 == timer {
                            // Windows silently removes timed-out low-level hooks.
                            // Install the replacement before releasing the old one
                            // so periodic recovery introduces no unhooked interval.
                            match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), module, 0) {
                                Ok(replacement) => {
                                    let _ = UnhookWindowsHookEx(hook);
                                    hook = replacement;
                                    // Reconcile modifiers after missed releases or
                                    // desktop switches. Blocked Win presses are not
                                    // reflected in async state; their keydowns were
                                    // already swallowed, so Windows cannot use them.
                                    MODIFIERS.with(|state| {
                                        let mut state = state.borrow_mut();
                                        for vk in [0xA2, 0xA3, 0x5B, 0x5C] {
                                            state.update(vk, GetAsyncKeyState(vk as i32) < 0);
                                        }
                                    });
                                }
                                Err(e) => crate::diagnostics::error(format!(
                                    "keyboard hook recovery failed: {e}"
                                )),
                            }
                            continue;
                        }
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                    let _ = KillTimer(None, timer);
                    let _ = UnhookWindowsHookEx(hook);
                    if let Ok(mut installed) = INSTALLED.lock() {
                        *installed = false;
                    }
                })
                .map_err(|e| e.to_string())?;
            rx.recv().map_err(|e| e.to_string())??;
            *installed = true;
        }
        ENABLED.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub fn install_hook() -> Result<(), String> {
    win::install()
}
#[cfg(not(target_os = "windows"))]
pub fn install_hook() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn remove_hook() {
    win::remove();
}
#[cfg(not(target_os = "windows"))]
pub fn remove_hook() {}

#[cfg(test)]
mod tests {
    use super::{blocks_shortcut, Modifiers};

    #[test]
    fn blocks_shell_escapes_but_preserves_game_input() {
        for key in [0x5B, 0x5C, 0x5D] {
            assert!(blocks_shortcut(key, false, false, false));
        }
        for key in [0x1B, 0x73, 0x20] {
            assert!(blocks_shortcut(key, true, false, false));
        }
        for key in [0x52, 0x45, 0x44, 0x4C, 0x09] {
            assert!(blocks_shortcut(key, false, false, true));
        }
        assert!(blocks_shortcut(0x1B, false, true, false));
        assert!(!blocks_shortcut(0x09, true, false, false)); // Alt+Tab
        assert!(!blocks_shortcut(0x73, false, false, false)); // plain F4
        for key in [0x41, 0x4C, 0x54, 0x57, 0x09, 0x73] {
            assert!(!blocks_shortcut(key, false, true, false)); // browser/game Ctrl shortcuts
        }
        for key in [0x25, 0x0D] {
            assert!(!blocks_shortcut(key, true, false, false)); // Alt+Left, Alt+Enter
        }
        assert!(blocks_shortcut(0x09, true, true, false)); // Ctrl+Alt+Tab
        for key in 0xB4..=0xB7 {
            assert!(blocks_shortcut(key, false, false, false));
        }
    }

    #[test]
    fn releasing_one_modifier_does_not_clear_the_other_side() {
        let mut state = Modifiers::default();
        for (left, right) in [(0xA2, 0xA3), (0x5B, 0x5C)] {
            state.update(left, true);
            state.update(right, true);
            state.update(left, false);
            assert!(state.ctrl != 0 || state.win != 0);
            state.update(right, false);
            assert_eq!((state.ctrl, state.win), (0, 0));
        }
    }
}
