mod audio;
mod cache;
mod fingerprint;
mod lockdown;
mod power;
mod process;
mod scan;
mod storage;

use lockdown::{init_locked_on_startup, is_locked, on_app_exit, register_keyboard_app};
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    // Auto-update manager (ADR-0028). Desktop-only; base tauri.conf.json carries an
    // empty stub so the plugin can initialize in dev/CI. Release builds merge
    // tauri.updater.conf.json (pubkey + GitHub Releases endpoint).
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());
    }

    builder
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if is_locked() {
                    api.prevent_close();
                }
            }
            if let tauri::WindowEvent::Focused(focused) = event {
                if *focused {
                    process::on_kiosk_focused(window.app_handle());
                    lockdown::set_audio_ui_yield(false);
                }
            }
            if let tauri::WindowEvent::Resized(..) = event {
                process::recover_minimized_kiosk(window.app_handle());
            }
        })
        .setup(|app| {
            register_keyboard_app(app.handle().clone());
            init_locked_on_startup(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            storage::get_tokens,
            storage::set_device_token,
            storage::set_player_token,
            storage::clear_player_token,
            storage::clear_all_tokens,
            audio::get_system_volume,
            audio::set_system_volume,
            audio::open_audio_settings,
            fingerprint::collect_fingerprint,
            scan::scan_installed_software,
            lockdown::set_lockdown_state,
            lockdown::get_lockdown_state,
            power::lock_workstation,
            power::restart_station,
            power::shutdown_station,
            process::launch_allowed,
            process::focus_kiosk,
            process::get_tracked_processes,
            process::kill_tracked_processes,
            process::clear_tracked_processes,
            cache::cache_asset,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let RunEvent::Exit = event {
                on_app_exit();
            }
        });
}
