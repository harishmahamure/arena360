use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;

/// Canonical wire representation for an instant.
pub fn utc_timestamp(value: &DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

/// Reject timestamp-looking JSON strings unless they are RFC 3339 UTC values ending in `Z`.
pub fn validate_utc_timestamps(value: &Value) -> Result<(), String> {
    validate_value(value, "$")
}

fn validate_value(value: &Value, path: &str) -> Result<(), String> {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                validate_value(value, &format!("{path}.{key}"))?;
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_value(value, &format!("{path}[{index}]"))?;
            }
        }
        Value::String(value) if looks_like_timestamp(value) => {
            let parsed = DateTime::parse_from_rfc3339(value)
                .map_err(|_| format!("{path} is not an RFC 3339 timestamp"))?;
            if !value.ends_with('Z') || parsed.offset().local_minus_utc() != 0 {
                return Err(format!("{path} must be a UTC timestamp ending in Z"));
            }
        }
        _ => {}
    }
    Ok(())
}

fn looks_like_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 11
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && matches!(bytes.get(10), Some(b'T' | b't'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_timestamp_uses_z() {
        let timestamp = DateTime::parse_from_rfc3339("2026-10-06T03:37:12.123+05:30")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(utc_timestamp(&timestamp), "2026-10-05T22:07:12.123Z");
    }

    #[test]
    fn recursively_rejects_local_or_offset_timestamps() {
        assert!(validate_utc_timestamps(&json!({
            "createdAt": "2026-10-06T03:37:12Z",
            "calendarDate": "2026-10-06"
        }))
        .is_ok());
        assert!(validate_utc_timestamps(&json!({
            "createdAt": "2026-10-06T03:37:12+05:30"
        }))
        .is_err());
        assert!(validate_utc_timestamps(&json!({
            "createdAt": "2026-10-06T03:37:12"
        }))
        .is_err());
    }
}
