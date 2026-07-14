//! Append-only boot/runtime log and in-memory error ring for the kiosk WebView.
//!
//! Log file location:
//! - Windows: `%ProgramData%\\Arena360\\kiosk.log`
//! - macOS:   `~/Library/Logs/Arena360/kiosk.log`
//! - Linux:   `/tmp/Arena360/kiosk.log`
//!
//! When `kiosk.log` exceeds 5 MiB it is rotated to `kiosk.log.1` (single backup).

use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_RING: usize = 50;
const MAX_RECENT_LINES: usize = 100;
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

struct DiagnosticsState {
    log_path: PathBuf,
    ring: Vec<String>,
    recent_lines: Vec<String>,
}

static STATE: Mutex<Option<DiagnosticsState>> = Mutex::new(None);

fn log_directory() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("PROGRAMDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
            .join("Arena360")
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Library/Logs/Arena360")
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        PathBuf::from("/tmp/Arena360")
    }
}

/// UTC ISO-8601 timestamp (`YYYY-MM-DDTHH:MM:SSZ`).
fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_unix_utc(secs)
}

fn format_unix_utc(secs: u64) -> String {
    let days = secs / 86_400;
    let time = secs % 86_400;
    let hours = time / 3600;
    let minutes = (time % 3600) / 60;
    let seconds = time % 60;

    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }

    format!(
        "{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z"
    )
}

fn backup_log_path(path: &Path) -> PathBuf {
    path.with_file_name("kiosk.log.1")
}

fn maybe_rotate(path: &Path) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    if meta.len() < MAX_LOG_BYTES {
        return;
    }
    let backup = backup_log_path(path);
    let _ = fs::remove_file(&backup);
    let _ = fs::rename(path, &backup);
}

fn write_line(path: &Path, line: &str) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    maybe_rotate(path);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

fn with_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut DiagnosticsState) -> R,
{
    let mut guard = STATE.lock().expect("diagnostics lock");
    if guard.is_none() {
        let log_path = log_directory().join("kiosk.log");
        *guard = Some(DiagnosticsState {
            log_path,
            ring: Vec::new(),
            recent_lines: Vec::new(),
        });
    }
    f(guard.as_mut().expect("diagnostics initialized"))
}

pub fn init() {
    with_state(|state| {
        let line = format!(
            "[{}] [INFO] kiosk diagnostics initialized (v{})",
            timestamp(),
            env!("CARGO_PKG_VERSION")
        );
        write_line(&state.log_path, &line);
        state.push_recent(line);
    });
}

/// Writes panics to `kiosk.log` before delegating to the default hook.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());
        let line = format!(
            "[{}] [ERROR] panic: {payload} at {location}",
            timestamp()
        );
        let path = log_directory().join("kiosk.log");
        write_line(&path, &line);
        default_hook(info);
    }));
}

impl DiagnosticsState {
    fn push_recent(&mut self, line: String) {
        self.recent_lines.push(line);
        if self.recent_lines.len() > MAX_RECENT_LINES {
            let drain = self.recent_lines.len() - MAX_RECENT_LINES;
            self.recent_lines.drain(0..drain);
        }
    }

    fn append(&mut self, level: &str, message: &str) {
        let line = format!("[{}] [{level}] {message}", timestamp());
        write_line(&self.log_path, &line);
        self.push_recent(line);
        if level == "ERROR" {
            self.ring.push(message.to_string());
            if self.ring.len() > MAX_RING {
                let drain = self.ring.len() - MAX_RING;
                self.ring.drain(0..drain);
            }
        }
    }
}

pub fn log(level: &str, message: impl AsRef<str>) {
    with_state(|state| state.append(level, message.as_ref()));
}

pub fn info(message: impl AsRef<str>) {
    log("INFO", message);
}

pub fn warn(message: impl AsRef<str>) {
    log("WARN", message);
}

pub fn error(message: impl AsRef<str>) {
    log("ERROR", message);
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootDiagnostics {
    pub log_path: String,
    pub recent_lines: Vec<String>,
    pub errors: Vec<String>,
}

#[tauri::command]
pub fn get_boot_diagnostics() -> BootDiagnostics {
    with_state(|state| BootDiagnostics {
        log_path: state.log_path.display().to_string(),
        recent_lines: state.recent_lines.clone(),
        errors: state.ring.clone(),
    })
}

#[tauri::command]
pub fn append_kiosk_log(level: String, message: String) -> Result<(), String> {
    let normalized = level.to_ascii_uppercase();
    let allowed = ["INFO", "WARN", "ERROR", "DEBUG"];
    if !allowed.contains(&normalized.as_str()) {
        return Err(format!("Invalid log level: {level}"));
    }
    log(&normalized, message);
    Ok(())
}

pub fn log_path_display() -> String {
    with_state(|state| state.log_path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_unix_utc_epoch() {
        assert_eq!(format_unix_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn format_unix_utc_known_instant() {
        // 2024-01-15 12:44:05 UTC
        assert_eq!(format_unix_utc(1_705_322_645), "2024-01-15T12:44:05Z");
    }

    #[test]
    fn maybe_rotate_renames_when_over_limit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log_path = dir.path().join("kiosk.log");
        let data = vec![b'x'; (MAX_LOG_BYTES + 1) as usize];
        fs::write(&log_path, data).expect("write log");

        maybe_rotate(&log_path);

        let backup = backup_log_path(&log_path);
        assert!(backup.exists(), "backup should exist after rotation");
        assert!(!log_path.exists(), "active log should be renamed");
    }
}
