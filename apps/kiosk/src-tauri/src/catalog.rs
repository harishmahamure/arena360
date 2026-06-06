//! Read-only CDN gallery cache (ADR-0019).
//!
//! When the webview fetches the centrally hosted `gallery.json`, it may persist
//! a copy to `app_data_dir()/games/gallery_cache.json` for offline picker use.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const GALLERY_CACHE_FILE: &str = "gallery_cache.json";

fn games_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("games");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn read_json(path: &Path) -> Result<Option<Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| e.to_string())
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_gallery_cache(app: AppHandle) -> Result<Option<Value>, String> {
    let path = games_dir(&app)?.join(GALLERY_CACHE_FILE);
    read_json(&path)
}

#[tauri::command]
pub fn write_gallery_cache(app: AppHandle, items: Value) -> Result<(), String> {
    let path = games_dir(&app)?.join(GALLERY_CACHE_FILE);
    write_json(&path, &serde_json::json!({ "items": items }))
}
