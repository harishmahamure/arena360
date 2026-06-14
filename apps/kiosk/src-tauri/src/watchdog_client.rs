//! Tauri-facing watchdog pause/mutex helpers (Windows).

use crate::watchdog::{
    clear_pause, write_pause, write_pause_secs, DEFAULT_SETUP_PAUSE_MINUTES,
    POWER_HANDOFF_PAUSE_SECS, UPDATE_PAUSE_MINUTES,
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

fn pause_for_update() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        write_pause(UPDATE_PAUSE_MINUTES, "update")
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

pub fn pause_for_power_action() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        write_pause_secs(POWER_HANDOFF_PAUSE_SECS, "power")
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

/// Pause watchdog before download/install so it does not respawn the kiosk while
/// NSIS replaces locked binaries.
#[tauri::command]
pub fn prepare_update_install() -> Result<(), String> {
    pause_for_update()
}

/// Refresh the update pause before relaunch (no-op if the installer already exited the process).
#[tauri::command]
pub fn prepare_update_relaunch() -> Result<(), String> {
    pause_for_update()
}
