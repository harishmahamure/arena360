//! Launcher-mediated launch profiles for platform store games (ADR-0019).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::path::Path;
#[cfg(target_os = "windows")]
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchVia {
    pub executable_path: String,
    #[serde(
        default = "Vec::new",
        deserialize_with = "deserialize_arguments",
        serialize_with = "serialize_arguments"
    )]
    pub arguments: Vec<String>,
}

impl LaunchVia {
    pub fn new(executable_path: impl Into<String>, arguments: Vec<String>) -> Self {
        Self {
            executable_path: executable_path.into(),
            arguments,
        }
    }

    pub fn single_arg(executable_path: impl Into<String>, arg: impl Into<String>) -> Self {
        Self::new(executable_path, vec![arg.into()])
    }
}

/// Stats emitted after scan for operator debug (trusted games only).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProfileStats {
    pub resolved: u32,
    pub unresolved: u32,
}

#[derive(Debug, Clone)]
struct IndexedProfile {
    profile: LaunchVia,
    /// Normalized install root used for prefix matching.
    install_root: Option<String>,
}

fn deserialize_arguments<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Args {
        List(Vec<String>),
        One(String),
    }

    match Args::deserialize(deserializer)? {
        Args::List(list) => Ok(list),
        Args::One(s) => Ok(tokenize_arguments(&s)),
    }
}

fn serialize_arguments<S>(args: &[String], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(args.len()))?;
    for arg in args {
        seq.serialize_element(arg)?;
    }
    seq.end()
}

/// Tokenize a command-line string respecting double quotes (Windows-style).
pub fn tokenize_arguments(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in input.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    out.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn steam_applaunch_args(app_id: &str) -> Vec<String> {
    vec!["-applaunch".to_string(), app_id.to_string()]
}

fn path_matches_exe(normalized_exe: &str, indexed: &IndexedProfile) -> bool {
    let Some(root) = &indexed.install_root else {
        return false;
    };
    if normalized_exe == root {
        return true;
    }
    let prefix = format!("{root}/");
    normalized_exe.starts_with(&prefix)
}

fn insert_index(
    map: &mut HashMap<String, IndexedProfile>,
    exe: &Path,
    install_root: Option<&Path>,
    profile: LaunchVia,
) {
    let key = normalize_path(&exe.to_string_lossy());
    let root = install_root.map(|p| normalize_path(&p.to_string_lossy()));
    map.insert(
        key,
        IndexedProfile {
            profile,
            install_root: root,
        },
    );
}

#[derive(Debug, Clone, Default)]
pub struct LauncherPaths {
    pub steam: Option<String>,
    pub riot: Option<String>,
    pub epic: Option<String>,
    pub battlenet: Option<String>,
    pub ea_helper: Option<String>,
    pub ubisoft: Option<String>,
    pub gog: Option<String>,
    pub rockstar: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct PlatformIndexes {
    epic_by_exe: HashMap<String, IndexedProfile>,
    battlenet_by_exe: HashMap<String, IndexedProfile>,
    ea_by_exe: HashMap<String, IndexedProfile>,
    ubisoft_by_exe: HashMap<String, IndexedProfile>,
    gog_by_exe: HashMap<String, IndexedProfile>,
    rockstar_by_exe: HashMap<String, IndexedProfile>,
}

#[derive(Debug, Clone)]
pub struct LaunchContext {
    pub launchers: LauncherPaths,
    indexes: PlatformIndexes,
}

impl LaunchContext {
    pub fn new(launchers: LauncherPaths) -> Self {
        Self {
            launchers,
            indexes: PlatformIndexes::default(),
        }
    }

    #[cfg(target_os = "windows")]
    pub fn with_platform_indexes(mut self) -> Self {
        self.indexes = load_platform_indexes(&self.launchers);
        self
    }

    #[cfg(not(target_os = "windows"))]
    pub fn with_platform_indexes(self) -> Self {
        self
    }
}

/// Resolve a launcher-mediated profile for a game executable.
pub fn resolve_launch_profile(
    executable_path: &str,
    steam_app_id: Option<&str>,
    ctx: &LaunchContext,
) -> Option<LaunchVia> {
    let norm = normalize_path(executable_path);

    if let Some(app_id) = steam_app_id {
        if let Some(steam) = &ctx.launchers.steam {
            return Some(LaunchVia::new(
                steam.clone(),
                steam_applaunch_args(app_id),
            ));
        }
    }

    if let Some(profile) = lookup_by_exe(&ctx.indexes.epic_by_exe, &norm) {
        return Some(profile);
    }
    if let Some(profile) = lookup_by_exe(&ctx.indexes.battlenet_by_exe, &norm) {
        return Some(profile);
    }
    if let Some(profile) = lookup_by_exe(&ctx.indexes.ea_by_exe, &norm) {
        return Some(profile);
    }
    if let Some(profile) = lookup_by_exe(&ctx.indexes.ubisoft_by_exe, &norm) {
        return Some(profile);
    }
    if let Some(profile) = lookup_by_exe(&ctx.indexes.gog_by_exe, &norm) {
        return Some(profile);
    }
    if let Some(profile) = lookup_by_exe(&ctx.indexes.rockstar_by_exe, &norm) {
        return Some(profile);
    }

    resolve_riot_profile(&norm, &ctx.launchers.riot)
}

fn lookup_by_exe(map: &HashMap<String, IndexedProfile>, norm: &str) -> Option<LaunchVia> {
    if let Some(indexed) = map.get(norm) {
        return Some(indexed.profile.clone());
    }
    for indexed in map.values() {
        if path_matches_exe(norm, indexed) {
            return Some(indexed.profile.clone());
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn program_files_roots() -> Vec<PathBuf> {
    ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(|var| std::env::var(var).ok().map(PathBuf::from))
        .collect()
}

#[cfg(target_os = "windows")]
fn fixed_drive_roots() -> Vec<PathBuf> {
    ('C'..='Z')
        .map(|letter| PathBuf::from(format!("{letter}:\\")))
        .filter(|path| path.exists())
        .collect()
}

#[cfg(target_os = "windows")]
fn battlenet_launch_args(code: &str) -> Vec<String> {
    vec![format!("--exec=launch {code}")]
}

#[cfg(target_os = "windows")]
fn battlenet_product_code(folder: &str) -> Option<&'static str> {
    let lower = folder.to_lowercase();
    if lower == "overwatch" || lower.contains("overwatch") {
        return Some("Pro");
    }
    if lower.contains("diablo iv") {
        return Some("D4");
    }
    if lower.contains("diablo iii") || lower == "diablo iii" {
        return Some("D3");
    }
    if lower.contains("diablo ii") {
        return Some("OSI");
    }
    if lower.contains("starcraft ii") {
        return Some("S2");
    }
    if lower.contains("world of warcraft") {
        return Some("WoW");
    }
    if lower.contains("hearthstone") {
        return Some("WTCG");
    }
    if lower.contains("heroes of the storm") {
        return Some("Hero");
    }
    if lower.contains("modern warfare") || lower.contains("call of duty modern warfare") {
        return Some("ODIN");
    }
    if lower.contains("black ops cold war") {
        return Some("ZEUS");
    }
    if lower.contains("warcraft iii") {
        return Some("W3");
    }
    None
}

#[cfg(target_os = "windows")]
fn resolve_riot_profile(norm: &str, riot_exe: &Option<String>) -> Option<LaunchVia> {
    let riot = riot_exe.as_ref()?;
    if !norm.contains("/riot games/") {
        return None;
    }

    let (product, patchline) = if norm.ends_with("/valorant/live/valorant.exe") {
        ("valorant", "live")
    } else if norm.contains("/league of legends/")
        && (norm.ends_with("/leagueclient.exe") || norm.ends_with("/league of legends.exe"))
    {
        ("league_of_legends", "live")
    } else if norm.contains("/league of legends/") && norm.ends_with("/leagueclientux.exe") {
        ("league_of_legends", "live")
    } else if norm.contains("/teamfight tactics/") {
        ("league_of_legends", "live")
    } else if norm.contains("/legendsofruneterra/") || norm.contains("/lor/") {
        ("lor", "live")
    } else {
        return None;
    };

    Some(LaunchVia::new(
        riot.clone(),
        vec![
            format!("--launch-product={product}"),
            format!("--launch-patchline={patchline}"),
        ],
    ))
}

#[cfg(not(target_os = "windows"))]
fn resolve_riot_profile(_norm: &str, _riot_exe: &Option<String>) -> Option<LaunchVia> {
    None
}

#[cfg(target_os = "windows")]
pub fn steam_app_id_from_manifest(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_prefix("appmanifest_")?.strip_suffix(".acf")?;
    if stem.chars().all(|c| c.is_ascii_digit()) {
        Some(stem.to_string())
    } else {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub fn steam_app_id_from_manifest(_path: &Path) -> Option<String> {
    None
}

#[cfg(target_os = "windows")]
pub fn load_platform_indexes(launchers: &LauncherPaths) -> PlatformIndexes {
    PlatformIndexes {
        epic_by_exe: load_epic_indexes(launchers.epic.as_deref()),
        battlenet_by_exe: load_battlenet_indexes(launchers.battlenet.as_deref()),
        ea_by_exe: load_ea_indexes(launchers.ea_helper.as_deref()),
        ubisoft_by_exe: load_ubisoft_indexes(launchers.ubisoft.as_deref()),
        gog_by_exe: load_gog_indexes(launchers.gog.as_deref()),
        rockstar_by_exe: load_rockstar_indexes(launchers.rockstar.as_deref()),
    }
}

#[cfg(target_os = "windows")]
fn epic_launch_uri(app_name: &str) -> String {
    format!("com.epicgames.launcher://apps/{app_name}?action=launch&silent=true")
}

#[cfg(target_os = "windows")]
fn epic_app_name_from_item(value: &serde_json::Value) -> Option<String> {
    if let Some(name) = value.get("AppName").and_then(|v| v.as_str()) {
        if name.contains(':') || name.len() > 20 {
            return Some(name.to_string());
        }
    }
    let ns = value.get("CatalogNamespace").and_then(|v| v.as_str())?;
    let item = value.get("CatalogItemId").and_then(|v| v.as_str())?;
    let app = value
        .get("AppName")
        .and_then(|v| v.as_str())
        .unwrap_or(item);
    Some(format!("{ns}:{item}:{app}"))
}

#[cfg(target_os = "windows")]
fn load_epic_indexes(epic_exe: Option<&str>) -> HashMap<String, IndexedProfile> {
    use std::fs;

    let Some(epic_exe) = epic_exe else {
        return HashMap::new();
    };

    let mut out = HashMap::new();
    let program_data = std::env::var("ProgramData").unwrap_or_default();

    let manifests_dir = PathBuf::from(&program_data)
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");
    if manifests_dir.exists() {
        if let Ok(entries) = fs::read_dir(&manifests_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext != "item" {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let Some(install_location) = value.get("InstallLocation").and_then(|v| v.as_str())
                else {
                    continue;
                };
                let Some(app_name) = epic_app_name_from_item(&value) else {
                    continue;
                };
                let install_root = PathBuf::from(install_location);
                let launch_exe = value
                    .get("LaunchExecutable")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let game_exe = if launch_exe.is_empty() {
                    find_primary_exe(&install_root)
                } else {
                    let candidate = install_root.join(launch_exe.replace('/', "\\"));
                    if candidate.exists() {
                        Some(candidate)
                    } else {
                        find_primary_exe(&install_root)
                    }
                };
                let Some(game_exe) = game_exe else {
                    continue;
                };
                let profile = LaunchVia::single_arg(epic_exe, epic_launch_uri(&app_name));
                insert_index(&mut out, &game_exe, Some(&install_root), profile);
            }
        }
    }

    let dat = PathBuf::from(&program_data)
        .join("Epic")
        .join("UnrealEngineLauncher")
        .join("LauncherInstalled.dat");
    if dat.exists() {
        if let Ok(text) = fs::read_to_string(&dat) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                let entries = value
                    .get("InstallationList")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                for entry in entries {
                    let Some(install_location) = entry.get("InstallLocation").and_then(|v| v.as_str())
                    else {
                        continue;
                    };
                    let app_name = entry
                        .get("AppName")
                        .and_then(|v| v.as_str())
                        .or_else(|| entry.get("NamespaceId").and_then(|v| v.as_str()));
                    let Some(app_name) = app_name else {
                        continue;
                    };
                    let install_root = PathBuf::from(install_location);
                    let launch_exe = entry
                        .get("LaunchExecutable")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let game_exe = if launch_exe.is_empty() {
                        find_primary_exe(&install_root)
                    } else {
                        let candidate = install_root.join(launch_exe.replace('/', "\\"));
                        if candidate.exists() {
                            Some(candidate)
                        } else {
                            find_primary_exe(&install_root)
                        }
                    };
                    let Some(game_exe) = game_exe else {
                        continue;
                    };
                    let norm = normalize_path(&game_exe.to_string_lossy());
                    if out.contains_key(&norm) {
                        continue;
                    }
                    let profile = LaunchVia::single_arg(epic_exe, epic_launch_uri(app_name));
                    insert_index(&mut out, &game_exe, Some(&install_root), profile);
                }
            }
        }
    }

    out
}

#[cfg(target_os = "windows")]
fn load_battlenet_indexes(battlenet_exe: Option<&str>) -> HashMap<String, IndexedProfile> {
    use std::fs;

    let Some(battlenet_exe) = battlenet_exe else {
        return HashMap::new();
    };

    let mut out = HashMap::new();
    let mut search_dirs: Vec<PathBuf> = program_files_roots();
    for drive in fixed_drive_roots() {
        search_dirs.push(drive.join("Games"));
    }

    for base in search_dirs {
        if !base.exists() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let folder = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let Some(code) = battlenet_product_code(&folder) else {
                continue;
            };
            if let Some(exe) = find_primary_exe(&dir) {
                let profile = LaunchVia::new(battlenet_exe, battlenet_launch_args(code));
                insert_index(&mut out, &exe, Some(&dir), profile);
            }
        }
    }

    for base in program_files_roots() {
        let root = base.join("Battle.net");
        if !root.exists() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let folder = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let Some(code) = battlenet_product_code(&folder) else {
                continue;
            };
            if let Some(exe) = find_primary_exe(&dir) {
                let profile = LaunchVia::new(battlenet_exe, battlenet_launch_args(code));
                insert_index(&mut out, &exe, Some(&dir), profile);
            }
        }
    }

    out
}

#[cfg(target_os = "windows")]
fn ea_install_dir_from_metadata(game_dir: &Path) -> Option<PathBuf> {
    use std::fs;

    let installer_xml = game_dir.join("__Installer").join("installerdata.xml");
    if installer_xml.exists() {
        if let Ok(text) = fs::read_to_string(&installer_xml) {
            if let Some(dir) = extract_xml_element(&text, "installPath")
                .or_else(|| extract_xml_element(&text, "InstallPath"))
                .or_else(|| extract_xml_element(&text, "InstallLocation"))
            {
                let path = PathBuf::from(dir);
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    let title = game_dir.file_name()?.to_str()?;
    for base in program_files_roots() {
        for rel in [
            format!("EA Games\\{title}"),
            format!("Electronic Arts\\{title}"),
        ] {
            let candidate = base.join(&rel);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn extract_xml_element(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    let value = text[start..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(target_os = "windows")]
fn load_ea_indexes(ea_helper: Option<&str>) -> HashMap<String, IndexedProfile> {
    use std::fs;

    let Some(ea_helper) = ea_helper else {
        return HashMap::new();
    };

    let mut out = HashMap::new();
    let install_data = PathBuf::from(std::env::var("ProgramData").unwrap_or_default())
        .join("EA Desktop")
        .join("InstallData");
    if !install_data.exists() {
        return out;
    }

    let Ok(entries) = fs::read_dir(&install_data) else {
        return out;
    };

    for entry in entries.flatten() {
        let game_dir = entry.path();
        if !game_dir.is_dir() {
            continue;
        }
        let Ok(files) = fs::read_dir(&game_dir) else {
            continue;
        };
        let offer_id = files.flatten().find_map(|f| {
            let name = f.file_name().to_string_lossy().to_string();
            name.strip_prefix("base-").map(|id| id.to_string())
        });
        let Some(offer_id) = offer_id else {
            continue;
        };
        let Some(install_dir) = ea_install_dir_from_metadata(&game_dir) else {
            continue;
        };
        let Some(exe) = find_primary_exe(&install_dir) else {
            continue;
        };
        let uri = format!("origin2://game/launch/?offerIds={offer_id}");
        let profile = LaunchVia::single_arg(ea_helper, uri);
        insert_index(&mut out, &exe, Some(&install_dir), profile);
    }

    out
}

#[cfg(target_os = "windows")]
fn load_ubisoft_indexes(ubisoft_exe: Option<&str>) -> HashMap<String, IndexedProfile> {
    use std::fs;

    let Some(ubisoft_exe) = ubisoft_exe else {
        return HashMap::new();
    };

    let mut out = HashMap::new();
    let config_dirs = program_files_roots()
        .into_iter()
        .map(|base| {
            base.join("Ubisoft")
                .join("Ubisoft Game Launcher")
                .join("cache")
                .join("configuration")
        })
        .collect::<Vec<_>>();

    for config_dir in config_dirs {
        if !config_dir.exists() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&config_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let game_id = value
                .pointer("/game/gameId")
                .or_else(|| value.get("gameId"))
                .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                .map(|id| id.to_string());
            let install_dir = value
                .pointer("/game/installDir")
                .or_else(|| value.get("installDir"))
                .and_then(|v| v.as_str())
                .map(PathBuf::from);
            let (Some(game_id), Some(install_dir)) = (game_id, install_dir) else {
                continue;
            };
            if !install_dir.exists() {
                continue;
            }
            let Some(exe) = find_primary_exe(&install_dir) else {
                continue;
            };
            let uri = format!("ubisoftconnect://launch/{game_id}");
            let profile = LaunchVia::single_arg(ubisoft_exe, uri);
            insert_index(&mut out, &exe, Some(&install_dir), profile);
        }
    }

    out
}

#[cfg(target_os = "windows")]
fn gog_installed_entries(value: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(arr) = value.as_array() {
        return arr.clone();
    }
    if let Some(obj) = value.as_object() {
        return obj.values().cloned().collect();
    }
    Vec::new()
}

#[cfg(target_os = "windows")]
fn load_gog_indexes(gog_exe: Option<&str>) -> HashMap<String, IndexedProfile> {
    use std::fs;

    let Some(gog_exe) = gog_exe else {
        return HashMap::new();
    };

    let mut out = HashMap::new();
    let installed = PathBuf::from(std::env::var("ProgramData").unwrap_or_default())
        .join("GOG.com")
        .join("Galaxy")
        .join("installed.json");
    if !installed.exists() {
        return out;
    }
    let Ok(text) = fs::read_to_string(&installed) else {
        return out;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return out;
    };
    for entry in gog_installed_entries(&value) {
        let game_id = entry.get("gameId").and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| v.as_u64().map(|n| n.to_string()))
        });
        let install_path = entry
            .get("installPath")
            .or_else(|| entry.get("installationPath"))
            .and_then(|v| v.as_str());
        let (Some(game_id), Some(install_path)) = (game_id, install_path) else {
            continue;
        };
        let install_dir = PathBuf::from(install_path);
        let Some(exe) = find_primary_exe(&install_dir) else {
            continue;
        };
        let profile = LaunchVia::new(
            gog_exe,
            vec![
                "/command=launch".to_string(),
                format!("/gameId={game_id}"),
            ],
        );
        insert_index(&mut out, &exe, Some(&install_dir), profile);
    }

    out
}

#[cfg(target_os = "windows")]
fn load_rockstar_indexes(rockstar_exe: Option<&str>) -> HashMap<String, IndexedProfile> {
    use std::fs;

    let Some(rockstar_exe) = rockstar_exe else {
        return HashMap::new();
    };

    let mut out = HashMap::new();
    let search_roots: Vec<PathBuf> = program_files_roots()
        .into_iter()
        .map(|base| base.join("Rockstar Games"))
        .collect();

    for root in search_roots {
        if !root.exists() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.eq_ignore_ascii_case("Launcher") {
                continue;
            }
            let Some(exe) = find_primary_exe(&dir) else {
                continue;
            };
            let folder_arg = format!("Rockstar Games\\{name}");
            let profile = LaunchVia::new(
                rockstar_exe,
                vec![
                    "-launchTitleInFolder".to_string(),
                    folder_arg,
                ],
            );
            insert_index(&mut out, &exe, Some(&dir), profile);
        }
    }

    out
}

#[cfg(target_os = "windows")]
fn find_primary_exe(root: &Path) -> Option<PathBuf> {
    use std::collections::VecDeque;
    use std::fs;

    let mut best: Option<(u64, PathBuf)> = None;
    let mut queue = VecDeque::from([(root.to_path_buf(), 0usize)]);
    while let Some((dir, depth)) = queue.pop_front() {
        if depth > 5 {
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
            if path.extension().and_then(|e| e.to_str()) != Some("exe") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            if is_noise_exe_name(&name) {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let mut score = size;
            if name.contains("shipping") || name.contains("win64") {
                score += 1_000_000_000;
            }
            if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
                best = Some((score, path));
            }
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(target_os = "windows")]
fn is_noise_exe_name(name: &str) -> bool {
    [
        "unins", "uninstall", "setup", "install", "updater", "update", "crash", "reporter",
        "helper", "service", "redist", "vcredist", "dxsetup", "launcher",
    ]
    .iter()
    .any(|needle| name.contains(needle))
}

/// Count resolved vs unresolved trusted candidates after profile attachment.
pub fn scan_profile_stats(candidates: &[ScanProfileCandidate]) -> ScanProfileStats {
    let mut stats = ScanProfileStats::default();
    for c in candidates {
        if !c.trusted {
            continue;
        }
        if c.launch_via.is_some() {
            stats.resolved += 1;
        } else {
            stats.unresolved += 1;
        }
    }
    stats
}

/// Minimal view for stats (avoids scan module dependency).
#[derive(Debug, Clone)]
pub struct ScanProfileCandidate {
    pub trusted: bool,
    pub launch_via: Option<LaunchVia>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_via_round_trips_json_array() {
        let via = LaunchVia::new(
            "C:\\Steam\\steam.exe",
            vec!["-applaunch".to_string(), "730".to_string()],
        );
        let json = serde_json::to_string(&via).unwrap();
        let parsed: LaunchVia = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, via);
    }

    #[test]
    fn steam_applaunch_uses_two_argv_tokens() {
        assert_eq!(
            steam_applaunch_args("730"),
            vec!["-applaunch", "730"]
        );
    }

    #[test]
    fn tokenize_respects_quotes() {
        assert_eq!(
            tokenize_arguments(r#"--exec="launch D3""#),
            vec!["--exec=launch D3"]
        );
    }

    #[test]
    fn deserialize_arguments_accepts_string() {
        let raw = r#"{"executablePath":"C:\\steam.exe","arguments":"-applaunch 730"}"#;
        let via: LaunchVia = serde_json::from_str(raw).unwrap();
        assert_eq!(via.arguments, vec!["-applaunch", "730"]);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn battlenet_code_from_overwatch_folder() {
        assert_eq!(battlenet_product_code("Overwatch"), Some("Pro"));
        assert_eq!(battlenet_product_code("Diablo III"), Some("D3"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn epic_app_name_builds_catalog_triple() {
        let value: serde_json::Value = serde_json::json!({
            "CatalogNamespace": "ns",
            "CatalogItemId": "item",
            "AppName": "app"
        });
        assert_eq!(
            epic_app_name_from_item(&value).as_deref(),
            Some("ns:item:app")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn extract_xml_install_path() {
        let xml = r#"<installPath>C:\Games\FIFA</installPath>"#;
        assert_eq!(
            extract_xml_element(xml, "installPath").as_deref(),
            Some("C:\\Games\\FIFA")
        );
    }

    #[test]
    fn scan_profile_stats_counts() {
        let stats = scan_profile_stats(&[
            ScanProfileCandidate {
                trusted: true,
                launch_via: Some(LaunchVia::single_arg("a", "b")),
            },
            ScanProfileCandidate {
                trusted: true,
                launch_via: None,
            },
            ScanProfileCandidate {
                trusted: false,
                launch_via: None,
            },
        ]);
        assert_eq!(stats.resolved, 1);
        assert_eq!(stats.unresolved, 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn riot_valorant_profile() {
        let ctx = LaunchContext::new(LauncherPaths {
            riot: Some("C:\\Riot Games\\Riot Client\\RiotClientServices.exe".to_string()),
            ..Default::default()
        });
        let profile = resolve_launch_profile(
            "C:\\Riot Games\\VALORANT\\live\\VALORANT.exe",
            None,
            &ctx,
        )
        .unwrap();
        assert_eq!(
            profile.arguments,
            vec![
                "--launch-product=valorant".to_string(),
                "--launch-patchline=live".to_string(),
            ]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn steam_app_id_inline() {
        let ctx = LaunchContext::new(LauncherPaths {
            steam: Some("C:\\Steam\\steam.exe".to_string()),
            ..Default::default()
        });
        let profile =
            resolve_launch_profile("C:\\Steam\\steamapps\\common\\CS2\\cs2.exe", Some("730"), &ctx)
                .unwrap();
        assert_eq!(profile.arguments, vec!["-applaunch", "730"]);
    }
}
