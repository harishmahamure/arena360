use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const DEVICE_KEY: &str = "gaming-cafe.kiosk.device_token";
const PLAYER_KEY: &str = "gaming-cafe.kiosk.player_token";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenStore {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_token: Option<String>,
}

fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("tokens.json"))
}

fn read_store(app: &AppHandle) -> Result<TokenStore, String> {
    let path = store_path(app)?;
    if !path.exists() {
        return Ok(TokenStore::default());
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn write_store(app: &AppHandle, store: &TokenStore) -> Result<(), String> {
    let path = store_path(app)?;
    let raw = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_tokens(app: AppHandle) -> Result<TokenStore, String> {
    read_store(&app)
}

#[tauri::command]
pub fn set_device_token(app: AppHandle, token: String) -> Result<(), String> {
    let mut store = read_store(&app)?;
    store.device_token = Some(token);
    write_store(&app, &store)
}

#[tauri::command]
pub fn set_player_token(app: AppHandle, token: String) -> Result<(), String> {
    let mut store = read_store(&app)?;
    store.player_token = Some(token);
    write_store(&app, &store)
}

#[tauri::command]
pub fn clear_player_token(app: AppHandle) -> Result<(), String> {
    let mut store = read_store(&app)?;
    store.player_token = None;
    write_store(&app, &store)
}

#[tauri::command]
pub fn clear_all_tokens(app: AppHandle) -> Result<(), String> {
    write_store(&app, &TokenStore::default())
}

#[allow(dead_code)]
pub const DEVICE_STORAGE_KEY: &str = DEVICE_KEY;
#[allow(dead_code)]
pub const PLAYER_STORAGE_KEY: &str = PLAYER_KEY;
