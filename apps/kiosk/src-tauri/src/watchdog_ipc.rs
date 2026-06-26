//! Watchdog pause-file IPC for the kiosk main process (ADR-0048).

use watchdog_common::{
    clear_pause_file, purge_expired_pause, write_pause_file, WATCHDOG_EXE_NAME,
};

#[tauri::command]
pub fn set_watchdog_pause(duration_secs: u64, reason: String) -> Result<(), String> {
    write_pause_file(duration_secs, &reason)
}

#[tauri::command]
pub fn clear_watchdog_pause() -> Result<(), String> {
    clear_pause_file()
}

pub fn clear_pause_on_locked_startup() -> Result<(), String> {
    let _ = purge_expired_pause()?;
    clear_pause_file()
}

pub fn set_pause_for_maintenance() -> Result<(), String> {
    write_pause_file(900, "maintenance")
}

pub fn set_pause_for_update() -> Result<(), String> {
    write_pause_file(600, "update")
}

pub const WATCHDOG_EXE_FILENAME: &str = WATCHDOG_EXE_NAME;

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::{Mutex, OnceLock};

    static TEST_DIR: OnceLock<Mutex<Option<tempfile::TempDir>>> = OnceLock::new();

    fn with_temp_marker<F: FnOnce()>(f: F) {
        let lock = TEST_DIR.get_or_init(|| Mutex::new(None));
        let mut guard = lock.lock().unwrap();
        let temp = tempfile::tempdir().expect("tempdir");
        env::set_var("WATCHDOG_COMMON_TEST_DIR", temp.path());
        *guard = Some(temp);
        drop(guard);
        f();
        env::remove_var("WATCHDOG_COMMON_TEST_DIR");
        *lock.lock().unwrap() = None;
    }

    #[test]
    fn set_and_clear_pause_round_trip() {
        with_temp_marker(|| {
            set_watchdog_pause(60, "test".to_string()).unwrap();
            assert!(watchdog_common::is_pause_file_active());
            clear_watchdog_pause().unwrap();
            assert!(!watchdog_common::is_pause_file_active());
        });
    }
}
