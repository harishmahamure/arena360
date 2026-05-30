//! Installed-software scan for the setup-mode allow-list editor
//! (ADR-0019 / ADR-0020, US-KREG / K3 `kiosk-software-scan-cmd`).
//!
//! Windows: probes a curated set of common gaming launchers / peripherals at
//! their typical install paths, then augments the list from the registry
//! uninstall keys (DisplayName + DisplayIcon). Off-Windows we return a small
//! dev fixture so the allow-list editor UI is exercisable on macOS.
//!
//! Progress is emitted on the `scan-progress` Tauri event as
//! `{ scanned, total }` so the UI can render a determinate bar. The whole scan
//! is filesystem/registry only and completes well under the 60 s budget.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCandidate {
    pub name: String,
    pub executable_path: String,
    pub source: String,
    pub present: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanProgress {
    scanned: usize,
    total: usize,
}

/// (display name, relative path under a base dir)
#[cfg(target_os = "windows")]
const KNOWN_APPS: &[(&str, &str)] = &[
    ("Steam", "Steam/steam.exe"),
    ("Epic Games Launcher", "Epic Games/Launcher/Portal/Binaries/Win64/EpicGamesLauncher.exe"),
    ("Riot Client", "Riot Games/Riot Client/RiotClientServices.exe"),
    ("Google Chrome", "Google/Chrome/Application/chrome.exe"),
    ("Logitech G HUB", "LGHUB/lghub.exe"),
    ("NVIDIA GeForce Experience", "NVIDIA Corporation/NVIDIA GeForce Experience/NVIDIA GeForce Experience.exe"),
    ("Discord", "Discord/Update.exe"),
];

#[tauri::command]
pub fn scan_installed_software(app: AppHandle) -> Result<Vec<ScanCandidate>, String> {
    #[cfg(target_os = "windows")]
    {
        scan_windows(&app)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let candidates = dev_fixture();
        let total = candidates.len();
        for (i, _) in candidates.iter().enumerate() {
            let _ = app.emit(
                "scan-progress",
                ScanProgress {
                    scanned: i + 1,
                    total,
                },
            );
        }
        Ok(candidates)
    }
}

#[cfg(target_os = "windows")]
fn scan_windows(app: &AppHandle) -> Result<Vec<ScanCandidate>, String> {
    use std::path::Path;

    let bases: Vec<String> = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .iter()
        .filter_map(|var| std::env::var(var).ok())
        .collect();

    let total = KNOWN_APPS.len() + 1;
    let mut out: Vec<ScanCandidate> = Vec::new();

    for (index, (name, rel)) in KNOWN_APPS.iter().enumerate() {
        let mut found: Option<String> = None;
        for base in &bases {
            let candidate = format!("{base}\\{}", rel.replace('/', "\\"));
            if Path::new(&candidate).exists() {
                found = Some(candidate);
                break;
            }
        }
        if let Some(path) = found {
            out.push(ScanCandidate {
                name: (*name).to_string(),
                executable_path: path,
                source: "known".to_string(),
                present: true,
            });
        }
        let _ = app.emit(
            "scan-progress",
            ScanProgress {
                scanned: index + 1,
                total,
            },
        );
    }

    for candidate in scan_registry() {
        if !out.iter().any(|c| c.name.eq_ignore_ascii_case(&candidate.name)) {
            out.push(candidate);
        }
    }
    let _ = app.emit(
        "scan-progress",
        ScanProgress {
            scanned: total,
            total,
        },
    );

    Ok(out)
}

#[cfg(target_os = "windows")]
fn scan_registry() -> Vec<ScanCandidate> {
    use std::process::Command;

    let script = r#"
        $paths = @(
          'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
          'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*'
        )
        Get-ItemProperty $paths -ErrorAction SilentlyContinue |
          Where-Object { $_.DisplayName -and $_.DisplayIcon } |
          ForEach-Object { "$($_.DisplayName)|$($_.DisplayIcon)" }
    "#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((name, icon)) = line.split_once('|') else {
            continue;
        };
        let exe = icon.split(',').next().unwrap_or("").trim().trim_matches('"');
        if exe.to_lowercase().ends_with(".exe") {
            out.push(ScanCandidate {
                name: name.trim().to_string(),
                executable_path: exe.to_string(),
                source: "registry".to_string(),
                present: std::path::Path::new(exe).exists(),
            });
        }
    }
    out
}

#[cfg(not(target_os = "windows"))]
fn dev_fixture() -> Vec<ScanCandidate> {
    vec![
        ScanCandidate {
            name: "Steam (dev)".to_string(),
            executable_path: "/Applications/Steam.app".to_string(),
            source: "known".to_string(),
            present: true,
        },
        ScanCandidate {
            name: "Google Chrome (dev)".to_string(),
            executable_path: "/Applications/Google Chrome.app".to_string(),
            source: "known".to_string(),
            present: true,
        },
        ScanCandidate {
            name: "Epic Games (dev)".to_string(),
            executable_path: "/Applications/Epic Games Launcher.app".to_string(),
            source: "registry".to_string(),
            present: false,
        },
    ]
}
