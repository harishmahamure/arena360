//! Append-only boot/runtime log and in-memory error ring for the kiosk WebView.
//!
//! Log file location:
//! - Windows: `%ProgramData%\\Arena360\\kiosk.log`
//! - macOS:   `~/Library/Logs/Arena360/kiosk.log`
//! - Linux:   `/tmp/Arena360/kiosk.log`

use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_RING: usize = 50;
const MAX_RECENT_LINES: usize = 100;

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

fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn write_line(path: &PathBuf, line: &str) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
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
