use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintPayload {
    pub mac: String,
    pub serial: String,
    pub bios_uuid: String,
    pub platform: String,
    pub collected_at: String,
}

#[tauri::command]
pub fn collect_fingerprint() -> Result<FingerprintPayload, String> {
    #[cfg(target_os = "windows")]
    {
        return collect_windows();
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(FingerprintPayload {
            mac: "00:00:00:00:00:00".to_string(),
            serial: "DEV-MAC".to_string(),
            bios_uuid: uuid_placeholder(),
            platform: std::env::consts::OS.to_string(),
            collected_at: chrono_now(),
        })
    }
}

#[cfg(target_os = "windows")]
fn collect_windows() -> Result<FingerprintPayload, String> {
    Ok(FingerprintPayload {
        mac: "00:00:00:00:00:00".to_string(),
        serial: "WIN-DEV".to_string(),
        bios_uuid: uuid_placeholder(),
        platform: "windows".to_string(),
        collected_at: chrono_now(),
    })
}

fn uuid_placeholder() -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        0u32, 0u16, 0u16, 0u16, 0u64
    )
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}
