/// Pre-update housekeeping (ADR-0028).
///
/// Closes tracked games and blocks until session cleanup completes before an
/// in-app NSIS update.
use tauri::AppHandle;

#[tauri::command]
pub fn prepare_for_update(app: AppHandle) -> Result<(), String> {
    if let Err(e) = crate::process::close_tracked_apps(app.clone()) {
        crate::diagnostics::warn(format!("prepare_for_update close_tracked_apps: {e}"));
    }
    if let Err(e) = crate::process::kill_tracked_processes_blocking(app) {
        crate::diagnostics::warn(format!("prepare_for_update kill_tracked_processes: {e}"));
    }
    Ok(())
}
