/// Pre-update housekeeping (ADR-0028, ADR-0048).
///
/// Closes tracked games, pauses the watchdog, and clears stale pause artifacts
/// before an in-app NSIS update.
use tauri::AppHandle;

#[tauri::command]
pub fn prepare_for_update(app: AppHandle) -> Result<(), String> {
    if let Err(e) = crate::process::close_tracked_apps(app.clone()) {
        crate::diagnostics::warn(format!("prepare_for_update close_tracked_apps: {e}"));
    }
    if let Err(e) = crate::process::kill_tracked_processes_blocking(app) {
        crate::diagnostics::warn(format!("prepare_for_update kill_tracked_processes: {e}"));
    }
    #[cfg(windows)]
    {
        if let Err(e) = watchdog_common::purge_expired_pause() {
            crate::diagnostics::warn(format!("prepare_for_update purge_expired_pause: {e}"));
        }
        crate::watchdog_ipc::set_pause_for_update()?;
    }
    Ok(())
}
