/// Pre-update housekeeping (ADR-0028, ADR-0048).
///
/// Closes tracked games, pauses the watchdog, and clears stale pause artifacts
/// before an in-app NSIS update.
use tauri::AppHandle;

#[tauri::command]
pub fn prepare_for_update(app: AppHandle) -> Result<(), String> {
    let _ = crate::process::close_tracked_apps(app.clone());
    let _ = crate::process::kill_tracked_processes_blocking(app);
    #[cfg(windows)]
    {
        let _ = watchdog_common::purge_expired_pause();
        crate::watchdog_ipc::set_pause_for_update()?;
    }
    Ok(())
}
