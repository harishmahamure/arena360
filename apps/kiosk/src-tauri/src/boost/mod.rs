//! Pre-launch game boost (Cortex-style resource prep) for Windows kiosk sessions.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameBoostConfig {
    pub enabled: bool,
    pub aggressive: bool,
}

impl Default for GameBoostConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            aggressive: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct BoostSnapshot {
    previous_power_plan: Option<String>,
}

static CONFIG: Mutex<GameBoostConfig> = Mutex::new(GameBoostConfig {
    enabled: true,
    aggressive: false,
});

static SNAPSHOT: Mutex<Option<BoostSnapshot>> = Mutex::new(None);

pub fn set_config(config: GameBoostConfig) {
    if let Ok(mut guard) = CONFIG.lock() {
        *guard = config;
    }
}

pub fn get_config() -> GameBoostConfig {
    CONFIG.lock().map(|g| g.clone()).unwrap_or_default()
}

fn config_enabled() -> bool {
    CONFIG.lock().map(|g| g.enabled).unwrap_or(false)
}

#[cfg(windows)]
fn config_aggressive() -> bool {
    CONFIG.lock().map(|g| g.aggressive).unwrap_or(false)
}

/// Run before spawning a game process.
pub fn prepare_game_boost() {
    if !config_enabled() {
        return;
    }

    let snapshot = {
        #[cfg(windows)]
        {
            if config_aggressive() {
                terminate_background_apps();
            }
            BoostSnapshot {
                previous_power_plan: activate_high_performance_plan(),
            }
        }
        #[cfg(not(windows))]
        {
            BoostSnapshot::default()
        }
    };

    if let Ok(mut guard) = SNAPSHOT.lock() {
        *guard = Some(snapshot);
    }
}

/// Raise priority for the launched root PID (and best-effort on tree during play).
pub fn apply_game_priority(pid: u32) {
    if !config_enabled() {
        return;
    }
    #[cfg(windows)]
    set_process_high_priority(pid);
    #[cfg(not(windows))]
    let _ = pid;
}

/// Restore boost side-effects when all tracked games have ended.
pub fn restore_game_boost() {
    let snapshot = SNAPSHOT.lock().ok().and_then(|mut g| g.take());
    let Some(snapshot) = snapshot else {
        return;
    };
    #[cfg(windows)]
    {
        if let Some(plan) = snapshot.previous_power_plan {
            restore_power_plan(&plan);
        }
    }
    #[cfg(not(windows))]
    let _ = snapshot;
}

#[cfg(windows)]
const DENYLIST_EXE: &[&str] = &[
    "discord.exe",
    "spotify.exe",
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "opera.exe",
    "brave.exe",
];

#[cfg(windows)]
fn terminate_background_apps() {
    use sysinfo::{ProcessesToUpdate, System};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, TerminateProcess, PROCESS_TERMINATE,
    };

    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);

    for (pid, process) in system.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        if !DENYLIST_EXE.iter().any(|deny| name == *deny) {
            continue;
        }
        let pid_u32 = pid.as_u32();
        if pid_u32 <= 4 {
            continue;
        }
        unsafe {
            if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid_u32) {
                let _ = TerminateProcess(handle, 1);
                let _ = CloseHandle(handle);
            }
        }
    }
}

#[cfg(windows)]
fn set_process_high_priority(pid: u32) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, SetPriorityClass, HIGH_PRIORITY_CLASS, PROCESS_SET_INFORMATION,
    };

    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_SET_INFORMATION, false, pid) {
            let _ = SetPriorityClass(handle, HIGH_PRIORITY_CLASS);
            let _ = CloseHandle(handle);
        }
    }
}

#[cfg(windows)]
const HIGH_PERFORMANCE_GUID: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";

#[cfg(windows)]
fn current_power_plan() -> Option<String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = Command::new("powercfg")
        .args(["/getactivescheme"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace()
        .find(|part| part.contains('-'))
        .map(|s| s.to_string())
}

#[cfg(windows)]
fn activate_high_performance_plan() -> Option<String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let previous = current_power_plan();
    let _ = Command::new("powercfg")
        .args(["/setactive", HIGH_PERFORMANCE_GUID])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    previous
}

#[cfg(windows)]
fn restore_power_plan(guid: &str) {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = Command::new("powercfg")
        .args(["/setactive", guid])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

#[tauri::command]
pub fn set_game_boost_config(config: GameBoostConfig) -> Result<(), String> {
    set_config(config);
    Ok(())
}

#[tauri::command]
pub fn get_game_boost_config() -> Result<GameBoostConfig, String> {
    Ok(get_config())
}
