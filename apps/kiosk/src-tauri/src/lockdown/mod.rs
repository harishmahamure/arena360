mod foreground;
mod keyboard;
pub mod shell;

pub use foreground::set_audio_ui_yield;

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockdownState {
    Locked,
    SetupRelaxed,
}

impl LockdownState {
    fn as_str(self) -> &'static str {
        match self {
            LockdownState::Locked => "Locked",
            LockdownState::SetupRelaxed => "SetupRelaxed",
        }
    }
}

static STATE: Mutex<LockdownState> = Mutex::new(LockdownState::Locked);
/// Serializes whole transitions (state write + window mode + hook toggle) so
/// concurrent `set_lockdown_state` calls cannot interleave window operations.
static TRANSITION: Mutex<()> = Mutex::new(());
fn parse_state(raw: &str) -> Result<LockdownState, String> {
    match raw {
        "Locked" => Ok(LockdownState::Locked),
        "SetupRelaxed" => Ok(LockdownState::SetupRelaxed),
        other => Err(format!("Unknown lockdown state: {other}")),
    }
}

fn apply_window_mode(app: &AppHandle, state: LockdownState) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .or_else(|| app.webview_windows().values().next().cloned())
        .ok_or_else(|| "Main window not found".to_string())?;

    match state {
        LockdownState::Locked => {
            window.set_closable(false).map_err(|e| e.to_string())?;
            window.set_minimizable(false).map_err(|e| e.to_string())?;
            window.set_maximizable(false).map_err(|e| e.to_string())?;
            if crate::process::has_tracked_processes() {
                crate::process::apply_kiosk_game_mode_background(app);
            } else {
                crate::process::apply_kiosk_shell_lockdown(app);
            }
            shell::hide_shell_chrome();
            keyboard::install_hook();
            foreground::start_foreground_guard(app.clone());
        }
        LockdownState::SetupRelaxed => {
            foreground::stop_foreground_guard();
            keyboard::remove_hook();
            shell::restore_shell_chrome();
            window.set_closable(true).map_err(|e| e.to_string())?;
            window.set_minimizable(true).map_err(|e| e.to_string())?;
            window.set_maximizable(true).map_err(|e| e.to_string())?;
            window.set_always_on_top(false).map_err(|e| e.to_string())?;
            window.set_fullscreen(false).map_err(|e| e.to_string())?;
            window.set_decorations(true).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn set_lockdown_state(app: AppHandle, state: String) -> Result<(), String> {
    let state = parse_state(&state)?;
    let _txn = TRANSITION.lock().map_err(|e| e.to_string())?;
    {
        let mut guard = STATE.lock().map_err(|e| e.to_string())?;
        *guard = state;
    }
    apply_window_mode(&app, state)?;
    let _ = app.emit("lockdown-changed", state.as_str());
    Ok(())
}

#[tauri::command]
pub fn get_lockdown_state() -> Result<String, String> {
    let guard = STATE.lock().map_err(|e| e.to_string())?;
    Ok(guard.as_str().to_string())
}

pub fn is_locked() -> bool {
    STATE
        .lock()
        .map(|g| *g == LockdownState::Locked)
        .unwrap_or(true)
}

pub fn register_keyboard_app(app: AppHandle) {
    keyboard::set_app_handle(app);
}

pub fn init_locked_on_startup(app: &AppHandle) {
    let _txn = TRANSITION.lock().expect("transition lock");
    if let Ok(mut guard) = STATE.lock() {
        *guard = LockdownState::Locked;
    }
    let _ = apply_window_mode(app, LockdownState::Locked);
    let _ = app.emit("lockdown-changed", LockdownState::Locked.as_str());
}

pub fn on_app_exit() {
    foreground::stop_foreground_guard();
    shell::restore_shell_chrome();
}

#[cfg(test)]
mod tests {
    use super::{parse_state, LockdownState};

    #[test]
    fn parses_known_states() {
        assert_eq!(parse_state("Locked").unwrap(), LockdownState::Locked);
        assert_eq!(
            parse_state("SetupRelaxed").unwrap(),
            LockdownState::SetupRelaxed
        );
    }

    #[test]
    fn rejects_unknown_state() {
        assert!(parse_state("Wide-Open").is_err());
    }

    #[test]
    fn round_trips_through_as_str() {
        for state in [LockdownState::Locked, LockdownState::SetupRelaxed] {
            assert_eq!(parse_state(state.as_str()).unwrap(), state);
        }
    }
}
