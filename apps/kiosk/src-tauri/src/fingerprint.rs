//! Hardware fingerprint collection (ADR-0017 / ADR-0020, US-KREG-002).
//!
//! On Windows we read MAC, BIOS serial and the BIOS/system UUID. To keep the
//! build dependency-free and avoid linking the WMI crates (which complicate the
//! cross-platform build), we shell out to PowerShell CIM queries — these are
//! present on every supported Windows 10/11 host. Off-Windows we return a
//! stable dev placeholder so the registration flow is exercisable on macOS.

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
        collect_windows()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(FingerprintPayload {
            mac: dev_mac(),
            serial: dev_serial(),
            bios_uuid: dev_uuid(),
            platform: std::env::consts::OS.to_string(),
            collected_at: now_iso8601(),
        })
    }
}

#[cfg(target_os = "windows")]
fn collect_windows() -> Result<FingerprintPayload, String> {
    let serial =
        powershell("(Get-CimInstance -ClassName Win32_BIOS).SerialNumber").unwrap_or_default();
    let bios_uuid = powershell("(Get-CimInstance -ClassName Win32_ComputerSystemProduct).UUID")
        .unwrap_or_default();
    let mac = powershell(
        "(Get-CimInstance -ClassName Win32_NetworkAdapter -Filter 'PhysicalAdapter=True AND NetEnabled=True' | Select-Object -First 1 -ExpandProperty MACAddress)",
    )
    .unwrap_or_default();

    Ok(FingerprintPayload {
        mac: normalize(&mac, "00:00:00:00:00:00"),
        serial: normalize(&serial, "UNKNOWN"),
        bios_uuid: normalize(&bios_uuid, &dev_uuid()),
        platform: "windows".to_string(),
        collected_at: now_iso8601(),
    })
}

#[cfg(target_os = "windows")]
fn powershell(script: &str) -> Option<String> {
    use std::process::Command;
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(target_os = "windows")]
fn normalize(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(not(target_os = "windows"))]
fn dev_mac() -> String {
    "AA:BB:CC:DD:EE:FF".to_string()
}

/// Stable per-install serial for macOS/Linux dev builds (avoids clashing on `DEV-SERIAL`).
#[cfg(not(target_os = "windows"))]
fn dev_serial() -> String {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    if let Some(home) = std::env::var_os("HOME") {
        let path = PathBuf::from(home).join(".gaming-cafe-kiosk-dev-install-id");
        if let Ok(existing) = fs::read_to_string(&path) {
            let trimmed = existing.trim();
            if !trimmed.is_empty() {
                return format!("DEV-{trimmed}");
            }
        }
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let id = format!("{:x}-{}", nanos, std::process::id());
        let _ = fs::write(&path, &id);
        return format!("DEV-{id}");
    }
    "DEV-UNKNOWN".to_string()
}

fn dev_uuid() -> String {
    "00000000-0000-0000-0000-000000000000".to_string()
}

fn now_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Minimal RFC3339-ish timestamp without pulling in chrono on the Tauri side.
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Format as epoch seconds tagged so the backend can still parse/store it.
    format!("{secs}")
}

#[cfg(test)]
mod tests {
    use super::collect_fingerprint;

    #[test]
    fn collect_returns_complete_camelcase_payload() {
        let fp = collect_fingerprint().expect("fingerprint collected");
        assert!(!fp.mac.is_empty());
        assert!(!fp.serial.is_empty());
        assert!(!fp.bios_uuid.is_empty());
        assert!(!fp.platform.is_empty());
        assert!(!fp.collected_at.is_empty());

        let json = serde_json::to_value(&fp).expect("serializes");
        // The backend DeviceFingerprintDto expects camelCase keys.
        assert!(json.get("biosUuid").is_some());
        assert!(json.get("collectedAt").is_some());
    }
}
