mod keyboard;

use std::sync::Mutex;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockdownState {
    Locked,
    SetupRelaxed,
}

static STATE: Mutex<LockdownState> = Mutex::new(LockdownState::Locked);

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
            window
                .set_fullscreen(true)
                .map_err(|e| e.to_string())?;
            window
                .set_always_on_top(true)
                .map_err(|e| e.to_string())?;
            window
                .set_decorations(false)
                .map_err(|e| e.to_string())?;
            keyboard::install_hook();
        }
        LockdownState::SetupRelaxed => {
            keyboard::remove_hook();
            window
                .set_always_on_top(false)
                .map_err(|e| e.to_string())?;
            window
                .set_fullscreen(false)
                .map_err(|e| e.to_string())?;
            window
                .set_decorations(true)
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn set_lockdown_state(app: AppHandle, state: String) -> Result<(), String> {
    let state = parse_state(&state)?;
    {
        let mut guard = STATE.lock().map_err(|e| e.to_string())?;
        *guard = state;
    }
    apply_window_mode(&app, state)
}

#[tauri::command]
pub fn get_lockdown_state() -> Result<String, String> {
    let guard = STATE.lock().map_err(|e| e.to_string())?;
    Ok(match *guard {
        LockdownState::Locked => "Locked".to_string(),
        LockdownState::SetupRelaxed => "SetupRelaxed".to_string(),
    })
}

pub fn init_locked_on_startup(app: &AppHandle) {
    if let Ok(mut guard) = STATE.lock() {
        *guard = LockdownState::Locked;
    }
    let _ = apply_window_mode(app, LockdownState::Locked);
}
