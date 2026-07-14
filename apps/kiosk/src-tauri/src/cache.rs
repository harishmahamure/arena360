//! On-device asset caching (DRAFT-0022).
//!
//! Game background videos are large; downloading them on every login is wasteful
//! and stutters on first paint. `cache_asset` downloads a remote URL once into the
//! app cache dir (keyed by a hash of the URL so changing the URL re-caches) and
//! returns the local file path. The webview renders it via `convertFileSrc`,
//! falling back to the remote URL when caching is unavailable.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn allowed_cache_url(url: &str) -> bool {
    let trimmed = url.trim().to_lowercase();
    trimmed.starts_with("https://cdn.arena360.cloud/")
        || trimmed.starts_with("http://localhost")
        || trimmed.starts_with("https://localhost")
        || trimmed.starts_with("http://127.0.0.1")
        || trimmed.starts_with("https://127.0.0.1")
}

fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("assets");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn cache_file_name(url: &str) -> String {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let digest = hasher.finish();
    let ext = url
        .rsplit('/')
        .next()
        .and_then(|seg| seg.rsplit('.').next())
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("bin");
    format!("{digest:016x}.{ext}")
}

/// Download `url` into the app cache (once) and return the absolute local path.
/// Returns the existing cached path immediately on a hit.
#[tauri::command]
pub async fn cache_asset(app: AppHandle, url: String) -> Result<String, String> {
    if !allowed_cache_url(&url) {
        return Err("url host is not allowed for caching".to_string());
    }
    let dir = cache_dir(&app)?;
    let target = dir.join(cache_file_name(&url));

    if target.exists() {
        return Ok(target.to_string_lossy().to_string());
    }

    let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("download failed: HTTP {}", response.status()));
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    fs::write(&target, &bytes).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().to_string())
}
