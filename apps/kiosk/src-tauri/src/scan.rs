//! Installed-software scan for the setup-mode allow-list editor
//! (ADR-0019 / ADR-0020, US-KREG / K3 `kiosk-software-scan-cmd`).
//!
//! Windows: probes common gaming launchers / utilities, reads store manifests
//! (Steam plus best-effort Epic/Battle.net/Riot folders), augments from the
//! registry uninstall keys, then performs a filtered fixed-drive scan for
//! standalone/unmanaged game executables. Off-Windows we return a small dev
//! fixture so the allow-list editor UI is exercisable on macOS.
//!
//! Progress is emitted on the `scan-progress` Tauri event as
//! `{ scanned, total }` so the UI can render a determinate bar. The whole scan
//! is filesystem/registry only and completes well under the 60 s budget.

use crate::launch_profile::{LaunchContext, LaunchVia, LauncherPaths};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "windows")]
use crate::launch_profile::{
    resolve_launch_profile, scan_profile_stats, steam_app_id_from_manifest, ScanProfileCandidate,
};
#[cfg(target_os = "windows")]
use std::path::{Path, PathBuf};

pub type ScanLaunchVia = LaunchVia;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCandidate {
    pub name: String,
    pub executable_path: String,
    pub source: String,
    pub present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch_via: Option<ScanLaunchVia>,
    /// `true` = scan attached a profile; `false` = trusted source but no profile found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch_profile_from_scan: Option<bool>,
    #[serde(skip)]
    pub steam_app_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanProgress {
    scanned: usize,
    total: usize,
}

/// A known app candidate under a Windows environment variable base directory.
#[cfg(target_os = "windows")]
struct KnownApp {
    name: &'static str,
    env: &'static str,
    rel: &'static str,
}

#[cfg(target_os = "windows")]
const KNOWN_APPS: &[KnownApp] = &[
    KnownApp {
        name: "Steam",
        env: "ProgramFiles(x86)",
        rel: "Steam/steam.exe",
    },
    KnownApp {
        name: "Steam",
        env: "ProgramFiles",
        rel: "Steam/steam.exe",
    },
    KnownApp {
        name: "Epic Games Launcher",
        env: "ProgramFiles(x86)",
        rel: "Epic Games/Launcher/Portal/Binaries/Win64/EpicGamesLauncher.exe",
    },
    KnownApp {
        name: "Epic Games Launcher",
        env: "ProgramFiles",
        rel: "Epic Games/Launcher/Portal/Binaries/Win64/EpicGamesLauncher.exe",
    },
    KnownApp {
        name: "Riot Client",
        env: "SystemDrive",
        rel: "Riot Games/Riot Client/RiotClientServices.exe",
    },
    KnownApp {
        name: "Riot Client",
        env: "ProgramFiles",
        rel: "Riot Games/Riot Client/RiotClientServices.exe",
    },
    KnownApp {
        name: "VALORANT",
        env: "SystemDrive",
        rel: "Riot Games/VALORANT/live/VALORANT.exe",
    },
    KnownApp {
        name: "League of Legends",
        env: "SystemDrive",
        rel: "Riot Games/League of Legends/LeagueClient.exe",
    },
    KnownApp {
        name: "EA app",
        env: "ProgramFiles",
        rel: "Electronic Arts/EA Desktop/EA Desktop/EALauncher.exe",
    },
    KnownApp {
        name: "Ubisoft Connect",
        env: "ProgramFiles(x86)",
        rel: "Ubisoft/Ubisoft Game Launcher/UbisoftConnect.exe",
    },
    KnownApp {
        name: "Battle.net",
        env: "ProgramFiles(x86)",
        rel: "Battle.net/Battle.net Launcher.exe",
    },
    KnownApp {
        name: "Rockstar Games Launcher",
        env: "ProgramFiles",
        rel: "Rockstar Games/Launcher/Launcher.exe",
    },
    KnownApp {
        name: "GOG Galaxy",
        env: "ProgramFiles(x86)",
        rel: "GOG Galaxy/GalaxyClient.exe",
    },
    KnownApp {
        name: "Google Chrome",
        env: "ProgramFiles",
        rel: "Google/Chrome/Application/chrome.exe",
    },
    KnownApp {
        name: "Google Chrome",
        env: "ProgramFiles(x86)",
        rel: "Google/Chrome/Application/chrome.exe",
    },
    KnownApp {
        name: "Microsoft Edge",
        env: "ProgramFiles(x86)",
        rel: "Microsoft/Edge/Application/msedge.exe",
    },
    KnownApp {
        name: "Logitech G HUB",
        env: "ProgramFiles",
        rel: "LGHUB/lghub.exe",
    },
    KnownApp {
        name: "Logitech Gaming Software",
        env: "ProgramFiles",
        rel: "Logitech Gaming Software/LCore.exe",
    },
    KnownApp {
        name: "NVIDIA Control Panel",
        env: "SystemRoot",
        rel: "System32/nvcplui.exe",
    },
    KnownApp {
        name: "NVIDIA GeForce Experience",
        env: "ProgramFiles",
        rel: "NVIDIA Corporation/NVIDIA GeForce Experience/NVIDIA GeForce Experience.exe",
    },
    KnownApp {
        name: "NVIDIA App",
        env: "ProgramFiles",
        rel: "NVIDIA Corporation/NVIDIA app/CEF/NVIDIA app.exe",
    },
    KnownApp {
        name: "Discord",
        env: "LOCALAPPDATA",
        rel: "Discord/Update.exe",
    },
    KnownApp {
        name: "Razer Synapse",
        env: "ProgramFiles(x86)",
        rel: "Razer/Synapse3/Service/Razer Synapse Service.exe",
    },
    KnownApp {
        name: "Corsair iCUE",
        env: "ProgramFiles",
        rel: "Corsair/CORSAIR iCUE 5 Software/iCUE.exe",
    },
    KnownApp {
        name: "SteelSeries GG",
        env: "ProgramFiles",
        rel: "SteelSeries/GG/SteelSeriesGG.exe",
    },
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
    let mut progress = ScanProgressCounter::new(app, KNOWN_APPS.len() + 4);
    let mut out: Vec<ScanCandidate> = Vec::new();

    for app in KNOWN_APPS {
        if let Some(path) = known_app_path(app) {
            push_unique(&mut out, candidate(app.name, path, "known", true));
        }
        progress.tick();
    }

    for candidate in scan_steam_libraries() {
        push_unique(&mut out, candidate);
    }
    progress.tick();

    for candidate in scan_store_folders() {
        push_unique(&mut out, candidate);
    }
    progress.tick();

    for candidate in scan_registry() {
        push_unique(&mut out, candidate);
    }
    progress.tick();

    for candidate in scan_fixed_drives() {
        push_unique(&mut out, candidate);
    }
    progress.finish();

    let launch_ctx = build_launch_context();
    for candidate in &mut out {
        if !should_attach_launch_profile(&candidate.source) {
            continue;
        }
        candidate.launch_via = resolve_launch_profile(
            &candidate.executable_path,
            candidate.steam_app_id.as_deref(),
            &launch_ctx,
        );
        candidate.launch_profile_from_scan = Some(candidate.launch_via.is_some());
    }

    let stats = scan_profile_stats(
        &out
            .iter()
            .map(|c| ScanProfileCandidate {
                trusted: should_attach_launch_profile(&c.source),
                launch_via: c.launch_via.clone(),
            })
            .collect::<Vec<_>>(),
    );
    let _ = app.emit("scan-profile-stats", stats);

    Ok(out)
}

#[cfg(target_os = "windows")]
struct ScanProgressCounter<'a> {
    app: &'a AppHandle,
    scanned: usize,
    total: usize,
}

#[cfg(target_os = "windows")]
impl<'a> ScanProgressCounter<'a> {
    fn new(app: &'a AppHandle, total: usize) -> Self {
        Self {
            app,
            scanned: 0,
            total,
        }
    }

    fn tick(&mut self) {
        self.scanned += 1;
        let _ = self.app.emit(
            "scan-progress",
            ScanProgress {
                scanned: self.scanned.min(self.total),
                total: self.total,
            },
        );
    }

    fn finish(&mut self) {
        self.scanned = self.total;
        let _ = self.app.emit(
            "scan-progress",
            ScanProgress {
                scanned: self.total,
                total: self.total,
            },
        );
    }
}

#[cfg(target_os = "windows")]
fn candidate(
    name: impl Into<String>,
    executable_path: impl Into<String>,
    source: &str,
    present: bool,
) -> ScanCandidate {
    ScanCandidate {
        name: name.into(),
        executable_path: executable_path.into(),
        source: source.to_string(),
        present,
        launch_via: None,
        launch_profile_from_scan: None,
        steam_app_id: None,
    }
}

fn should_attach_launch_profile(source: &str) -> bool {
    matches!(source, "known" | "steam" | "manifest")
}

#[cfg(target_os = "windows")]
fn build_launch_context() -> LaunchContext {
    let ea_helper = resolve_ea_helper_path();
    LaunchContext::new(LauncherPaths {
        steam: known_app_path_by_name("Steam"),
        riot: known_app_path_by_name("Riot Client"),
        epic: known_app_path_by_name("Epic Games Launcher"),
        battlenet: known_app_path_by_name("Battle.net").or_else(|| resolve_battlenet_exe()),
        ea_helper,
        ubisoft: known_app_path_by_name("Ubisoft Connect"),
        gog: known_app_path_by_name("GOG Galaxy"),
        rockstar: known_app_path_by_name("Rockstar Games Launcher"),
    })
    .with_platform_indexes()
}

#[cfg(not(target_os = "windows"))]
fn build_launch_context() -> LaunchContext {
    LaunchContext::new(LauncherPaths::default())
}

#[cfg(target_os = "windows")]
fn known_app_path_by_name(name: &str) -> Option<String> {
    KNOWN_APPS
        .iter()
        .find(|a| a.name == name)
        .and_then(known_app_path)
}

#[cfg(target_os = "windows")]
fn resolve_battlenet_exe() -> Option<String> {
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        let Ok(base) = std::env::var(var) else {
            continue;
        };
        for rel in ["Battle.net/Battle.net.exe", "Battle.net/Battle.net Launcher.exe"] {
            let path = format!("{base}\\{}", rel.replace('/', "\\"));
            if Path::new(&path).exists() {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn resolve_ea_helper_path() -> Option<String> {
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        let Ok(base) = std::env::var(var) else {
            continue;
        };
        let helper = format!(
            "{base}\\Electronic Arts\\EA Desktop\\EA Desktop\\EALaunchHelper.exe"
        );
        if Path::new(&helper).exists() {
            return Some(helper);
        }
    }
    known_app_path_by_name("EA app")
}

#[cfg(target_os = "windows")]
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

#[cfg(target_os = "windows")]
fn source_priority(source: &str) -> u8 {
    match source {
        "steam" => 3,
        "known" => 2,
        "manifest" => 1,
        _ => 0,
    }
}

#[cfg(target_os = "windows")]
fn push_unique(out: &mut Vec<ScanCandidate>, item: ScanCandidate) {
    let path = normalize_path(&item.executable_path);
    if let Some(idx) = out
        .iter()
        .position(|c| normalize_path(&c.executable_path) == path)
    {
        if source_priority(&item.source) > source_priority(&out[idx].source) {
            out[idx] = item;
        }
        return;
    }
    if let Some(idx) = out
        .iter()
        .position(|c| c.name.eq_ignore_ascii_case(&item.name))
    {
        if source_priority(&item.source) > source_priority(&out[idx].source) {
            out[idx] = item;
        }
        return;
    }
    out.push(item);
}

#[cfg(target_os = "windows")]
fn known_app_path(app: &KnownApp) -> Option<String> {
    use std::path::Path;

    let base = std::env::var(app.env).ok()?;
    let path = format!("{base}\\{}", app.rel.replace('/', "\\"));
    Path::new(&path).exists().then_some(path)
}

#[cfg(target_os = "windows")]
fn scan_steam_libraries() -> Vec<ScanCandidate> {
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;

    let mut libraries = Vec::new();
    for app in KNOWN_APPS.iter().filter(|a| a.name == "Steam") {
        if let Some(steam_exe) = known_app_path(app) {
            if let Some(root) = Path::new(&steam_exe).parent() {
                libraries.push(root.to_path_buf());
                libraries.extend(read_steam_libraryfolders(root));
            }
        }
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for library in libraries {
        let key = normalize_path(&library.to_string_lossy());
        if !seen.insert(key) {
            continue;
        }
        let steamapps = library.join("steamapps");
        let Ok(entries) = fs::read_dir(&steamapps) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !(file_name.starts_with("appmanifest_") && file_name.ends_with(".acf")) {
                continue;
            }
            if let Some(game) = read_steam_manifest(&path, &steamapps) {
                out.push(game);
            }
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn read_steam_libraryfolders(steam_root: &std::path::Path) -> Vec<std::path::PathBuf> {
    use std::fs;

    let mut out = Vec::new();
    let path = steam_root.join("steamapps").join("libraryfolders.vdf");
    let Ok(text) = fs::read_to_string(path) else {
        return out;
    };
    for line in text.lines() {
        if let Some(path) = extract_quoted_value(line, "path") {
            out.push(std::path::PathBuf::from(path.replace("\\\\", "\\")));
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn read_steam_manifest(
    path: &std::path::Path,
    steamapps: &std::path::Path,
) -> Option<ScanCandidate> {
    let text = std::fs::read_to_string(path).ok()?;
    let name = text
        .lines()
        .find_map(|line| extract_quoted_value(line, "name"))?;
    let installdir = text
        .lines()
        .find_map(|line| extract_quoted_value(line, "installdir"))
        .unwrap_or_else(|| name.clone());
    let install_root = steamapps.join("common").join(installdir);
    let exe = find_game_exe(&install_root, &name)?;
    let mut item = candidate(
        name,
        exe.to_string_lossy().to_string(),
        "steam",
        true,
    );
    item.steam_app_id = steam_app_id_from_manifest(path);
    Some(item)
}

#[cfg(target_os = "windows")]
fn extract_quoted_value(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim();
    let prefix = format!("\"{key}\"");
    if !trimmed.starts_with(&prefix) {
        return None;
    }
    let rest = trimmed[prefix.len()..].trim();
    if !rest.starts_with('"') {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(target_os = "windows")]
fn scan_store_folders() -> Vec<ScanCandidate> {
    let mut roots: Vec<(String, PathBuf)> = Vec::new();
    for drive in fixed_drive_roots() {
        roots.push(("Riot".to_string(), drive.join("Riot Games")));
        roots.push(("Epic".to_string(), drive.join("Epic Games")));
        roots.push(("Games".to_string(), drive.join("Games")));
        roots.push(("Xbox".to_string(), drive.join("XboxGames")));
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramData"] {
        if let Ok(base) = std::env::var(var) {
            let base = PathBuf::from(base);
            roots.push(("Battle.net".to_string(), base.join("Battle.net")));
            roots.push(("EA".to_string(), base.join("EA Games")));
            roots.push(("Ubisoft".to_string(), base.join("Ubisoft")));
            roots.push(("Rockstar".to_string(), base.join("Rockstar Games")));
            roots.push(("GOG".to_string(), base.join("GOG Galaxy").join("Games")));
        }
    }

    let mut out = Vec::new();
    for (source, root) in roots {
        if !root.exists() {
            continue;
        }
        for dir in first_level_dirs(&root) {
            let name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Game")
                .trim()
                .to_string();
            if let Some(exe) = find_game_exe(&dir, &name) {
                out.push(candidate(
                    name,
                    exe.to_string_lossy().to_string(),
                    "manifest",
                    true,
                ));
            } else if source == "Riot" {
                for nested in first_level_dirs(&dir) {
                    let name = nested
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Riot Game")
                        .trim()
                        .to_string();
                    if let Some(exe) = find_game_exe(&nested, &name) {
                        out.push(candidate(
                            name,
                            exe.to_string_lossy().to_string(),
                            "manifest",
                            true,
                        ));
                    }
                }
            }
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn first_level_dirs(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(root)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn scan_fixed_drives() -> Vec<ScanCandidate> {
    use std::collections::{HashSet, VecDeque};
    use std::fs;

    let mut out = Vec::new();
    let mut seen_dirs = HashSet::new();
    for root in fixed_drive_roots() {
        let mut queue = VecDeque::from([(root, 0usize)]);
        while let Some((dir, depth)) = queue.pop_front() {
            let dir_key = normalize_path(&dir.to_string_lossy());
            if !seen_dirs.insert(dir_key) || should_skip_dir(&dir) || depth > 9 {
                continue;
            }
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    queue.push_back((path, depth + 1));
                } else if is_exe(&path) && is_game_like_exe(&path) {
                    let name = display_name_from_exe(&path);
                    out.push(candidate(
                        name,
                        path.to_string_lossy().to_string(),
                        "drive-scan",
                        true,
                    ));
                }
            }
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn fixed_drive_roots() -> Vec<PathBuf> {
    ('A'..='Z')
        .map(|letter| PathBuf::from(format!("{letter}:\\")))
        .filter(|path| path.exists())
        .collect()
}

#[cfg(target_os = "windows")]
fn find_game_exe(root: &std::path::Path, display_name: &str) -> Option<std::path::PathBuf> {
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;

    let mut best: Option<(u64, PathBuf)> = None;
    let mut queue = VecDeque::from([(root.to_path_buf(), 0usize)]);
    while let Some((dir, depth)) = queue.pop_front() {
        if depth > 5 || should_skip_dir(&dir) {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                queue.push_back((path, depth + 1));
                continue;
            }
            if !is_exe(&path) || is_noise_exe(&path) {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let score = exe_score(&path, display_name, size);
            if score == 0 {
                continue;
            }
            if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best = Some((score, path));
            }
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(target_os = "windows")]
fn is_game_like_exe(path: &std::path::Path) -> bool {
    if is_noise_exe(path) || should_skip_dir(path.parent().unwrap_or(path)) {
        return false;
    }
    let text = normalize_path(&path.to_string_lossy());
    let in_game_root = [
        "/games/",
        "/steamapps/common/",
        "/steamlibrary/",
        "/riot games/",
        "/epic games/",
        "/battlenet/",
        "/battle.net/",
        "/xboxgames/",
        "/ea games/",
        "/ubisoft/",
        "/rockstar games/",
        "/gog galaxy/games/",
    ]
    .iter()
    .any(|needle| text.contains(needle));

    if in_game_root {
        return true;
    }

    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    size > 20 * 1024 * 1024 && has_nearby_game_marker(path)
}

#[cfg(target_os = "windows")]
fn exe_score(path: &std::path::Path, display_name: &str, size: u64) -> u64 {
    let file = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    let wanted = display_name
        .to_lowercase()
        .replace([' ', '-', '_', ':'], "");
    let file_compact = file.replace([' ', '-', '_', ':'], "");
    let mut score = 0;
    if wanted.contains(&file_compact) || file_compact.contains(&wanted) {
        score += 100;
    }
    if file.contains("shipping") || file.contains("win64") || file.contains("launcher") {
        score += 20;
    }
    if size > 10 * 1024 * 1024 {
        score += 10;
    }
    if has_nearby_game_marker(path) {
        score += 20;
    }
    score
}

#[cfg(target_os = "windows")]
fn is_exe(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn is_noise_exe(path: &std::path::Path) -> bool {
    let name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    [
        "unins",
        "uninstall",
        "setup",
        "install",
        "updater",
        "update",
        "crash",
        "reporter",
        "helper",
        "service",
        "redist",
        "vcredist",
        "dxsetup",
    ]
    .iter()
    .any(|needle| name.contains(needle))
}

#[cfg(target_os = "windows")]
fn should_skip_dir(path: &std::path::Path) -> bool {
    let text = normalize_path(&path.to_string_lossy());
    [
        "/windows",
        "/windows/system32",
        "/windows/syswow64",
        "/windows/winsxs",
        "/$recycle.bin",
        "/system volume information",
        "/programdata/microsoft",
        "/appdata/local/temp",
        "/appdata/local/microsoft/windows",
        "/node_modules",
        "/target/",
        "/dist/",
        "/.git/",
        "/cache/",
        "/logs/",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

#[cfg(target_os = "windows")]
fn has_nearby_game_marker(path: &std::path::Path) -> bool {
    let Some(dir) = path.parent() else {
        return false;
    };
    if dir.join("UnityPlayer.dll").exists()
        || dir.join("Engine").join("Binaries").exists()
        || dir.join("Binaries").join("Win64").exists()
    {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().take(80).any(|entry| {
        let p = entry.path();
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                matches!(
                    e.to_ascii_lowercase().as_str(),
                    "pak" | "ucas" | "utoc" | "dll"
                )
            })
            .unwrap_or(false)
    })
}

#[cfg(target_os = "windows")]
fn display_name_from_exe(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Game")
        .replace(['_', '-'], " ")
        .trim()
        .to_string()
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
        let exe = icon
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches('"');
        if exe.to_lowercase().ends_with(".exe") {
            out.push(ScanCandidate {
                name: name.trim().to_string(),
                executable_path: exe.to_string(),
                source: "registry".to_string(),
                present: std::path::Path::new(exe).exists(),
                launch_via: None,
                launch_profile_from_scan: None,
                steam_app_id: None,
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
            launch_via: None,
            launch_profile_from_scan: None,
            steam_app_id: None,
        },
        ScanCandidate {
            name: "Google Chrome (dev)".to_string(),
            executable_path: "/Applications/Google Chrome.app".to_string(),
            source: "known".to_string(),
            present: true,
            launch_via: None,
            launch_profile_from_scan: None,
            steam_app_id: None,
        },
        ScanCandidate {
            name: "Epic Games (dev)".to_string(),
            executable_path: "/Applications/Epic Games Launcher.app".to_string(),
            source: "registry".to_string(),
            present: false,
            launch_via: None,
            launch_profile_from_scan: None,
            steam_app_id: None,
        },
    ]
}
