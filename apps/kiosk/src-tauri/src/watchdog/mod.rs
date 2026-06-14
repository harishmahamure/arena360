//! Shared watchdog logic for `arena360-watchdog.exe` and the kiosk app (pause file,
//! process detection, instance mutex). Windows-only.

#[cfg(windows)]
mod win;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const INSTANCE_MUTEX_NAME: &str = "Global\\Arena360KioskInstance";
pub const WATCHDOG_MUTEX_NAME: &str = "Global\\Arena360WatchdogInstance";
pub const WATCHDOG_EXE_BASENAME: &str = "arena360-watchdog";
pub const PAUSE_DIR: &str = "Arena360";
pub const PAUSE_FILENAME: &str = "watchdog.pause";

pub const DEFAULT_SETUP_PAUSE_MINUTES: u64 = 15;
pub const UPDATE_HANDOFF_PAUSE_SECS: u64 = 30;
pub const POLL_INTERVAL_SECS: u64 = 2;
pub const SPAWN_DEBOUNCE_SECS: u64 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PauseFile {
    pub until: String,
    #[serde(default)]
    pub reason: String,
}

pub fn pause_file_path() -> PathBuf {
    program_data_dir().join(PAUSE_DIR).join(PAUSE_FILENAME)
}

fn program_data_dir() -> PathBuf {
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
}

pub fn write_pause(minutes: u64, reason: &str) -> Result<(), String> {
    let until = SystemTime::now()
        .checked_add(Duration::from_secs(minutes.saturating_mul(60)))
        .ok_or_else(|| "pause duration overflow".to_string())?;
    write_pause_until(until, reason)
}

pub fn write_pause_secs(secs: u64, reason: &str) -> Result<(), String> {
    let until = SystemTime::now()
        .checked_add(Duration::from_secs(secs))
        .ok_or_else(|| "pause duration overflow".to_string())?;
    write_pause_until(until, reason)
}

pub fn write_pause_until(until: SystemTime, reason: &str) -> Result<(), String> {
    let path = pause_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let payload = PauseFile {
        until: format_rfc3339(until),
        reason: reason.to_string(),
    };
    let json = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

pub fn clear_pause() -> Result<(), String> {
    let path = pause_file_path();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn read_pause_file() -> Result<Option<PauseFile>, String> {
    let path = pause_file_path();
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let parsed = serde_json::from_str::<PauseFile>(&raw).map_err(|e| e.to_string())?;
    Ok(Some(parsed))
}

pub fn is_paused(now: SystemTime) -> Result<bool, String> {
    let Some(file) = read_pause_file()? else {
        return Ok(false);
    };
    let until = parse_rfc3339(&file.until)?;
    Ok(now < until)
}

pub fn resolve_kiosk_exe(watchdog_exe: &Path) -> Result<PathBuf, String> {
    let install_dir = watchdog_exe
        .parent()
        .ok_or_else(|| "watchdog exe has no parent directory".to_string())?;

    let watchdog_name = watchdog_exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(WATCHDOG_EXE_BASENAME);

    let mut candidates: Vec<PathBuf> = std::fs::read_dir(install_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| {
                        !name.eq_ignore_ascii_case(watchdog_name)
                            && !name.eq_ignore_ascii_case("uninstall.exe")
                    })
        })
        .collect();

    candidates.sort_by(|a, b| {
        let a_main = is_likely_main_kiosk(a);
        let b_main = is_likely_main_kiosk(b);
        b_main.cmp(&a_main).then_with(|| a.file_name().cmp(&b.file_name()))
    });

    candidates
        .into_iter()
        .next()
        .ok_or_else(|| format!("no kiosk exe found in {}", install_dir.display()))
}

fn is_likely_main_kiosk(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| {
            name.contains("Arena360") || name.contains("Station Management") || name == "kiosk.exe"
        })
}

pub fn format_rfc3339(time: SystemTime) -> String {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    // UTC formatting without chrono: good enough for watchdog pause TTL.
    timestamp_to_rfc3339(secs)
}

pub fn parse_rfc3339(raw: &str) -> Result<SystemTime, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty until timestamp".to_string());
    }
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Ok(UNIX_EPOCH + Duration::from_secs(secs));
    }
    parse_rfc3339_utc(trimmed)
}

fn timestamp_to_rfc3339(secs: u64) -> String {
    let days = secs / 86400;
    let rem = secs % 86400;
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let second = rem % 60;

    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn parse_rfc3339_utc(raw: &str) -> Result<SystemTime, String> {
    let raw = raw.strip_suffix('Z').unwrap_or(raw);
    let (date, time) = raw
        .split_once('T')
        .ok_or_else(|| format!("invalid RFC3339 timestamp: {raw}"))?;
    let (year, month, day) = parse_date(date)?;
    let (hour, minute, second) = parse_time(time)?;
    let days = days_from_civil(year, month, day)?;
    let secs = days * 86400 + hour * 3600 + minute * 60 + second;
    Ok(UNIX_EPOCH + Duration::from_secs(secs))
}

fn parse_date(raw: &str) -> Result<(i64, u64, u64), String> {
    let parts: Vec<_> = raw.split('-').collect();
    if parts.len() != 3 {
        return Err(format!("invalid date: {raw}"));
    }
    Ok((
        parts[0].parse().map_err(|_| format!("invalid year: {raw}"))?,
        parts[1].parse().map_err(|_| format!("invalid month: {raw}"))?,
        parts[2].parse().map_err(|_| format!("invalid day: {raw}"))?,
    ))
}

fn parse_time(raw: &str) -> Result<(u64, u64, u64), String> {
    let raw = raw.split('.').next().unwrap_or(raw);
    let parts: Vec<_> = raw.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("invalid time: {raw}"));
    }
    Ok((
        parts[0].parse().map_err(|_| format!("invalid hour: {raw}"))?,
        parts[1].parse().map_err(|_| format!("invalid minute: {raw}"))?,
        parts[2].parse().map_err(|_| format!("invalid second: {raw}"))?,
    ))
}

// Algorithms from http://howardhinnant.github.io/date_algorithms.html (public domain).
fn civil_from_days(days: i64) -> (u64, u64, u64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + (era * 400) as u64;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

fn days_from_civil(year: i64, month: u64, day: u64) -> Result<u64, String> {
    if !(1..=12).contains(&month) || day == 0 || day > 31 {
        return Err(format!("invalid civil date: {year}-{month}-{day}"));
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok((era * 146097 + doe as i64 - 719468) as u64)
}

#[cfg(windows)]
pub use win::{acquire_instance_mutex, watchdog_main_loop};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn rfc3339_round_trip() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let until = now + Duration::from_secs(900);
        let formatted = format_rfc3339(until);
        let parsed = parse_rfc3339(&formatted).unwrap();
        assert!(parsed > now);
    }

    #[test]
    fn rejects_malformed_pause_timestamp() {
        assert!(parse_rfc3339("not-a-date").is_err());
    }

    #[test]
    fn expired_pause_timestamp_is_in_the_past() {
        let past = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        assert!(parse_rfc3339(&format_rfc3339(past)).unwrap() < now);
    }
}
