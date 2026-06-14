#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("arena360-watchdog is only supported on Windows");
        std::process::exit(1);
    }

    #[cfg(target_os = "windows")]
    {
        if let Err(err) = run() {
            eprintln!("arena360-watchdog error: {err}");
            std::process::exit(1);
        }
    }
}

#[cfg(target_os = "windows")]
fn run() -> Result<(), String> {
    let watchdog_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    kiosk_lib::watchdog_main_loop(watchdog_exe)
}
