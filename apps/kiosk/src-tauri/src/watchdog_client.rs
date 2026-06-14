//! Tauri-facing watchdog pause/mutex helpers (Windows).

use crate::watchdog::{
    clear_pause, write_pause, write_pause_secs, DEFAULT_SETUP_PAUSE_MINUTES,
    UPDATE_HANDOFF_PAUSE_SECS,
};

#[tauri::command]
pub fn set_watchdog_pause(minutes: u64, reason: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        write_pause(minutes.max(1), &reason)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (minutes, reason);
        Ok(())
    }
}

#[tauri::command]
pub fn clear_watchdog_pause() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        clear_pause()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

pub fn pause_for_setup() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        write_pause(DEFAULT_SETUP_PAUSE_MINUTES, "setup")
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

pub fn pause_for_update_handoff() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        write_pause_secs(UPDATE_HANDOFF_PAUSE_SECS, "update")
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

pub fn pause_for_power_action() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        write_pause(DEFAULT_SETUP_PAUSE_MINUTES, "power")
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

pub fn init_instance_mutex() {
    #[cfg(target_os = "windows")]
    {
        if let Ok(guard) = crate::watchdog::acquire_instance_mutex() {
            // Hold the kiosk single-instance mutex for the process lifetime.
            // Cannot store HANDLE in a Sync static; leaking the guard is intentional.
            std::mem::forget(guard);
        }
    }
}

pub fn pause_before_update_relaunch() -> Result<(), String> {
    pause_for_update_handoff()
}

#[tauri::command]
pub fn prepare_update_relaunch() -> Result<(), String> {
    pause_before_update_relaunch()
}
