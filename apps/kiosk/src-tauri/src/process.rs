use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedProcess {
    pub pid: u32,
    pub executable_path: String,
}

#[derive(Debug, Clone)]
struct WatchEntry {
    executable_path: String,
    arguments: Option<String>,
    pid: u32,
}

static TRACKED: Mutex<Vec<WatchEntry>> = Mutex::new(Vec::new());
static WATCHDOG_RUNNING: Mutex<bool> = Mutex::new(false);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub pid: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KillResult {
    pub killed: u32,
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn is_allowed(path: &str, allow_list: &[String]) -> bool {
    if allow_list.is_empty() {
        return true;
    }
    let norm = normalize_path(path);
    allow_list
        .iter()
        .any(|entry| norm.ends_with(&normalize_path(entry)))
}

fn spawn_process(path: &str, args: Option<&str>) -> Result<u32, String> {
    let mut cmd = Command::new(path);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if let Some(a) = args {
        for part in a.split_whitespace() {
            cmd.arg(part);
        }
    }
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    Ok(child.id())
}

fn is_running(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command as StdCommand;
        let output = StdCommand::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}")])
            .output();
        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            return text.contains(&pid.to_string());
        }
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::process::Command as StdCommand;
        StdCommand::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn kill_pid(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).status();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
    }
}

fn start_watchdog_if_needed() {
    let mut running = WATCHDOG_RUNNING.lock().expect("watchdog lock");
    if *running {
        return;
    }
    *running = true;
    drop(running);

    thread::spawn(|| {
        loop {
            thread::sleep(Duration::from_secs(1));
            let mut entries = TRACKED.lock().expect("tracked lock").clone();
            if entries.is_empty() {
                continue;
            }
            for entry in &mut entries {
                if is_running(entry.pid) {
                    continue;
                }
                if let Ok(new_pid) = spawn_process(&entry.executable_path, entry.arguments.as_deref())
                {
                    entry.pid = new_pid;
                }
            }
            let mut tracked = TRACKED.lock().expect("tracked lock");
            *tracked = entries;
        }
    });
}

#[tauri::command]
pub fn launch_allowed(
    executable_path: String,
    allow_list: Vec<String>,
    arguments: Option<String>,
) -> Result<LaunchResult, String> {
    if !is_allowed(&executable_path, &allow_list) {
        return Err("Executable not in allow-list".to_string());
    }
    let pid = spawn_process(&executable_path, arguments.as_deref())?;
    TRACKED.lock().map_err(|e| e.to_string())?.push(WatchEntry {
        executable_path,
        arguments,
        pid,
    });
    start_watchdog_if_needed();
    Ok(LaunchResult { pid })
}

#[tauri::command]
pub fn get_tracked_processes() -> Result<Vec<TrackedProcess>, String> {
    let tracked = TRACKED.lock().map_err(|e| e.to_string())?;
    Ok(tracked
        .iter()
        .map(|e| TrackedProcess {
            pid: e.pid,
            executable_path: e.executable_path.clone(),
        })
        .collect())
}

#[tauri::command]
pub fn kill_tracked_processes(grace_seconds: Option<u32>) -> Result<KillResult, String> {
    let grace = grace_seconds.unwrap_or(0);
    if grace > 0 {
        thread::sleep(Duration::from_secs(grace as u64));
    }
    let pids: Vec<u32> = TRACKED
        .lock()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|e| e.pid)
        .collect();
    for pid in &pids {
        kill_pid(*pid);
    }
    TRACKED.lock().map_err(|e| e.to_string())?.clear();
    Ok(KillResult {
        killed: pids.len() as u32,
    })
}

#[tauri::command]
pub fn clear_tracked_processes() -> Result<(), String> {
    TRACKED.lock().map_err(|e| e.to_string())?.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{is_allowed, normalize_path};

    #[test]
    fn normalize_lowercases_and_unifies_separators() {
        assert_eq!(normalize_path("C:\\Games\\Steam.EXE"), "c:/games/steam.exe");
    }

    #[test]
    fn empty_allow_list_permits_everything() {
        assert!(is_allowed("C:/anything.exe", &[]));
    }

    #[test]
    fn allow_list_matches_by_suffix_case_insensitively() {
        let allow = vec!["C:\\Program Files\\Steam\\steam.exe".to_string()];
        assert!(is_allowed("c:/program files/steam/STEAM.exe", &allow));
    }

    #[test]
    fn allow_list_rejects_unlisted_executable() {
        let allow = vec!["C:\\Games\\steam.exe".to_string()];
        assert!(!is_allowed("C:\\Windows\\System32\\cmd.exe", &allow));
    }
}
