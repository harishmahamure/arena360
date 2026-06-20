/// Pre-update housekeeping (ADR-0028). Clears legacy watchdog pause files so an
/// in-app NSIS update is not blocked by artifacts from older builds.
#[tauri::command]
pub fn prepare_for_update() -> Result<(), String> {
    #[cfg(windows)]
    {
        let pause_path = std::path::Path::new(r"C:\ProgramData\Arena360\watchdog.pause");
        if pause_path.exists() {
            std::fs::remove_file(pause_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
