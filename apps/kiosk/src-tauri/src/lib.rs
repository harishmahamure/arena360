mod cache;
mod fingerprint;
mod lockdown;
mod process;
mod scan;
mod storage;

use lockdown::init_locked_on_startup;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            init_locked_on_startup(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            storage::get_tokens,
            storage::set_device_token,
            storage::set_player_token,
            storage::clear_player_token,
            storage::clear_all_tokens,
            fingerprint::collect_fingerprint,
            scan::scan_installed_software,
            lockdown::set_lockdown_state,
            lockdown::get_lockdown_state,
            process::launch_allowed,
            process::get_tracked_processes,
            process::kill_tracked_processes,
            process::clear_tracked_processes,
            cache::cache_asset,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
