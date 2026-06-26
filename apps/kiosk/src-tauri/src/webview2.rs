//! WebView2 Evergreen runtime detection and silent install (Windows only).
//!
//! Runs before Tauri initializes the WebView so a missing runtime can be
//! installed from a bundled bootstrapper or downloaded from Microsoft.

#[cfg(windows)]
mod win {
    use crate::diagnostics;
    use serde::Serialize;
    use std::ffi::OsStr;
    use std::fs;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
        KEY_READ, REG_VALUE_TYPE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    const WEBVIEW2_CLIENT_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    const BOOTSTRAPPER_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";
    const BOOTSTRAPPER_NAME: &str = "MicrosoftEdgeWebview2Setup.exe";
    const INSTALL_ATTEMPTED_ENV: &str = "ARENA360_WEBVIEW2_INSTALL_ATTEMPTED";
    const MANUAL_INSTALL_URL: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";

    const REG_PATH_LM_WOW64: &str =
        "SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    const REG_PATH_LM: &str =
        "SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    const REG_PATH_CU: &str =
        "SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

    const REG_CHECKS: &[(HKEY, &str)] = &[
        (HKEY_LOCAL_MACHINE, REG_PATH_LM_WOW64),
        (HKEY_LOCAL_MACHINE, REG_PATH_LM),
        (HKEY_CURRENT_USER, REG_PATH_CU),
    ];

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct WebView2Status {
        pub installed: bool,
        pub version: Option<String>,
    }

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    fn read_pv(hkey: HKEY, subkey: &str) -> Option<String> {
        unsafe {
            let mut opened = HKEY::default();
            let subkey_wide = wide(subkey);
            if RegOpenKeyExW(hkey, PCWSTR(subkey_wide.as_ptr()), 0, KEY_READ, &mut opened).is_err()
            {
                return None;
            }

            let value_name = wide("pv");
            let mut value_type = REG_VALUE_TYPE::default();
            let mut buffer = [0u16; 64];
            let mut buffer_size = (buffer.len() * 2) as u32;

            let result = RegQueryValueExW(
                opened,
                PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut value_type),
                Some(buffer.as_mut_ptr().cast()),
                Some(&mut buffer_size),
            );
            let _ = RegCloseKey(opened);

            if result.is_err() {
                return None;
            }

            let len = (buffer_size as usize / 2).saturating_sub(1);
            let version = String::from_utf16_lossy(&buffer[..len]);
            if version.trim().is_empty() {
                None
            } else {
                Some(version)
            }
        }
    }

    pub fn installed_version() -> Option<String> {
        for (hkey, path) in REG_CHECKS {
            if let Some(version) = read_pv(*hkey, path) {
                return Some(version);
            }
        }
        None
    }

    pub fn get_webview2_status() -> WebView2Status {
        let version = installed_version();
        WebView2Status {
            installed: version.is_some(),
            version,
        }
    }

    fn exe_dir() -> Option<PathBuf> {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    }

    fn bundled_bootstrapper_paths() -> Vec<PathBuf> {
        let Some(dir) = exe_dir() else {
            return Vec::new();
        };
        vec![
            dir.join("resources").join(BOOTSTRAPPER_NAME),
            dir.join("WebView2").join(BOOTSTRAPPER_NAME),
        ]
    }

    fn find_bundled_bootstrapper() -> Option<PathBuf> {
        bundled_bootstrapper_paths()
            .into_iter()
            .find(|p| p.is_file())
    }

    fn temp_bootstrapper_path() -> Result<PathBuf, String> {
        let base = std::env::temp_dir().join("Arena360");
        fs::create_dir_all(&base).map_err(|e| format!("create temp dir: {e}"))?;
        Ok(base.join(BOOTSTRAPPER_NAME))
    }

    fn download_bootstrapper(dest: &Path) -> Result<(), String> {
        diagnostics::info("webview2: downloading Evergreen bootstrapper from Microsoft");
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| format!("http client: {e}"))?;
        let bytes = client
            .get(BOOTSTRAPPER_URL)
            .send()
            .map_err(|e| format!("download request: {e}"))?
            .error_for_status()
            .map_err(|e| format!("download status: {e}"))?
            .bytes()
            .map_err(|e| format!("download body: {e}"))?;
        fs::write(dest, &bytes).map_err(|e| format!("write bootstrapper: {e}"))?;
        diagnostics::info(format!(
            "webview2: downloaded bootstrapper ({} bytes) to {}",
            bytes.len(),
            dest.display()
        ));
        Ok(())
    }

    fn resolve_bootstrapper() -> Result<PathBuf, String> {
        if let Some(path) = find_bundled_bootstrapper() {
            diagnostics::info(format!(
                "webview2: using bundled bootstrapper at {}",
                path.display()
            ));
            return Ok(path);
        }

        let dest = temp_bootstrapper_path()?;
        if dest.is_file() {
            diagnostics::info(format!(
                "webview2: using cached bootstrapper at {}",
                dest.display()
            ));
            return Ok(dest);
        }

        download_bootstrapper(&dest)?;
        Ok(dest)
    }

    fn run_installer(installer: &Path) -> Result<(), String> {
        diagnostics::info(format!(
            "webview2: running silent install: {}",
            installer.display()
        ));
        let output = Command::new(installer)
            .args(["/silent", "/install"])
            .output()
            .map_err(|e| format!("spawn installer: {e}"))?;

        diagnostics::info(format!(
            "webview2: installer exit code {}",
            output.status.code().unwrap_or(-1)
        ));

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!(
                "installer failed (code {:?}): stdout={stdout} stderr={stderr}",
                output.status.code()
            ));
        }
        Ok(())
    }

    fn relaunch_self() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
        diagnostics::info(format!("webview2: relaunching after install: {}", exe.display()));
        Command::new(&exe)
            .env(INSTALL_ATTEMPTED_ENV, "1")
            .spawn()
            .map_err(|e| format!("relaunch spawn: {e}"))?;
        Ok(())
    }

    pub fn ensure_runtime() -> Result<(), String> {
        if let Some(version) = installed_version() {
            diagnostics::info(format!("webview2: runtime present (version {version})"));
            return Ok(());
        }

        diagnostics::warn("webview2: runtime not detected in registry");

        if std::env::var(INSTALL_ATTEMPTED_ENV).is_ok() {
            return Err(
                "WebView2 runtime still missing after install attempt (restart may be required)"
                    .to_string(),
            );
        }

        let installer = resolve_bootstrapper()?;
        run_installer(&installer)?;

        if let Some(version) = installed_version() {
            diagnostics::info(format!("webview2: install ok (version {version})"));
            relaunch_self()?;
            std::process::exit(0);
        }

        Err("WebView2 install completed but runtime was not detected".to_string())
    }

    pub fn show_install_failure(message: &str) {
        let log_path = crate::diagnostics::log_path_display();
        let text = format!(
            "Arena360 requires the Microsoft Edge WebView2 runtime.\n\n\
             {message}\n\n\
             Log file:\n{log_path}\n\n\
             Manual install:\n{MANUAL_INSTALL_URL}"
        );
        let wide_text = wide(&text);
        let wide_title = wide("Arena360 — WebView2 Required");
        unsafe {
            let _ = MessageBoxW(
                HWND::default(),
                PCWSTR(wide_text.as_ptr()),
                PCWSTR(wide_title.as_ptr()),
                MB_OK | MB_ICONERROR,
            );
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn client_guid_is_evergreen() {
            assert!(WEBVIEW2_CLIENT_GUID.starts_with('{'));
            assert!(WEBVIEW2_CLIENT_GUID.contains("F3017226"));
        }
    }
}

#[cfg(windows)]
pub use win::{ensure_runtime, get_webview2_status, show_install_failure, WebView2Status};

#[cfg(not(windows))]
mod stub {
    use serde::Serialize;

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct WebView2Status {
        pub installed: bool,
        pub version: Option<String>,
    }

    pub fn ensure_runtime() -> Result<(), String> {
        Ok(())
    }

    pub fn show_install_failure(_message: &str) {}

    pub fn get_webview2_status() -> WebView2Status {
        WebView2Status {
            installed: true,
            version: None,
        }
    }
}

#[cfg(not(windows))]
pub use stub::{ensure_runtime, get_webview2_status, show_install_failure, WebView2Status};

#[tauri::command]
pub fn get_webview2_status_cmd() -> WebView2Status {
    get_webview2_status()
}
