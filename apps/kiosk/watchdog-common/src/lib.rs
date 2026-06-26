//! Pause-file IPC shared between the kiosk main process and `arena360-watchdog.exe`.
//!
//! Pause file: `%ProgramData%\Arena360\watchdog.pause` as JSON
//! `{ "expiresAt": "<ISO8601 UTC>", "reason": "<string>" }`.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const MARKER_DIR: &str = r"C:\ProgramData\Arena360";
pub const PAUSE_FILE_NAME: &str = "watchdog.pause";
pub const MARKER_FILE_NAME: &str = "registry-hardening.json";
pub const KIOSK_MUTEX_NAME: &str = "Global\\Arena360KioskInstance";
pub const WATCHDOG_MUTEX_NAME: &str = "Global\\Arena360WatchdogInstance";
pub const KIOSK_EXE_NAME: &str = "Arena360 Station Management.exe";
pub const WATCHDOG_EXE_NAME: &str = "arena360-watchdog.exe";
pub const WATCHDOG_POLL_MS: u64 = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PauseFile {
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
    pub reason: String,
}

pub fn marker_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("WATCHDOG_COMMON_TEST_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(MARKER_DIR)
}

pub fn pause_file_path() -> PathBuf {
    marker_dir().join(PAUSE_FILE_NAME)
}

pub fn ensure_marker_dir() -> Result<(), String> {
    fs::create_dir_all(marker_dir()).map_err(|e| e.to_string())
}

pub fn read_pause_file() -> Option<PauseFile> {
    let raw = fs::read_to_string(pause_file_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn parse_expires_at(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

pub fn is_pause_active(pause: &PauseFile) -> bool {
    parse_expires_at(&pause.expires_at)
        .map(|expires| expires > Utc::now())
        .unwrap_or(false)
}

pub fn is_pause_file_active() -> bool {
    read_pause_file()
        .map(|pause| is_pause_active(&pause))
        .unwrap_or(false)
}

/// Delete the pause file when expired. Returns `true` if a stale file was removed.
pub fn purge_expired_pause() -> Result<bool, String> {
    let path = pause_file_path();
    if !path.is_file() {
        return Ok(false);
    }
    let Some(pause) = read_pause_file() else {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
        return Ok(true);
    };
    if is_pause_active(&pause) {
        return Ok(false);
    }
    fs::remove_file(&path).map_err(|e| e.to_string())?;
    Ok(true)
}

pub fn write_pause_file(duration_secs: u64, reason: &str) -> Result<(), String> {
    ensure_marker_dir()?;
    let expires = Utc::now() + Duration::seconds(i64::try_from(duration_secs).unwrap_or(i64::MAX));
    let pause = PauseFile {
        expires_at: expires.to_rfc3339(),
        reason: reason.to_string(),
    };
    let json = serde_json::to_string_pretty(&pause).map_err(|e| e.to_string())?;
    fs::write(pause_file_path(), json).map_err(|e| e.to_string())
}

pub fn clear_pause_file() -> Result<(), String> {
    let path = pause_file_path();
    if path.is_file() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct InstallMarker {
    #[serde(rename = "installDir")]
    install_dir: Option<String>,
}

pub fn read_install_dir_from_marker() -> Option<PathBuf> {
    let marker_path = marker_dir().join(MARKER_FILE_NAME);
    let raw = fs::read_to_string(marker_path).ok()?;
    let marker: InstallMarker = serde_json::from_str(&raw).ok()?;
    marker.install_dir.map(PathBuf::from)
}

pub fn resolve_kiosk_exe(watchdog_exe: &Path) -> Option<PathBuf> {
    if let Some(install_dir) = read_install_dir_from_marker() {
        let preferred = install_dir.join(KIOSK_EXE_NAME);
        if preferred.is_file() {
            return Some(preferred);
        }
    }

    let sibling_dir = watchdog_exe.parent()?;
    let preferred = sibling_dir.join(KIOSK_EXE_NAME);
    if preferred.is_file() {
        return Some(preferred);
    }

    fs::read_dir(sibling_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.extension().is_some_and(|ext| ext == "exe")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        !name.starts_with("arena360-watchdog")
                            && !name.to_lowercase().contains("uninstall")
                    })
        })
}

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
    fn pause_round_trip_and_expiry() {
        with_temp_marker(|| {
            write_pause_file(3600, "maintenance").expect("write");
            let pause = read_pause_file().expect("read");
            assert_eq!(pause.reason, "maintenance");
            assert!(is_pause_active(&pause));
        });
    }

    #[test]
    fn purge_removes_expired_pause() {
        with_temp_marker(|| {
            ensure_marker_dir().unwrap();
            let expired = PauseFile {
                expires_at: (Utc::now() - Duration::seconds(60)).to_rfc3339(),
                reason: "stale".to_string(),
            };
            fs::write(
                pause_file_path(),
                serde_json::to_string(&expired).unwrap(),
            )
            .unwrap();
            assert!(purge_expired_pause().unwrap());
            assert!(!pause_file_path().is_file());
        });
    }
}
