use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;

/// Canonical wire representation for an instant.
pub fn utc_timestamp(value: &DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SqliteTimestampError {
    #[error("timestamp must use YYYY-MM-DDTHH:MM:SS.ffffffZ")]
    InvalidFormat,
    #[error("timestamp year is outside the four-digit SQLite storage range")]
    OutOfRange,
}

/// Formats an instant as fixed-width UTC text, truncating sub-microsecond precision.
pub fn format_sqlite_timestamp(value: &DateTime<Utc>) -> Result<String, SqliteTimestampError> {
    let formatted = value.to_rfc3339_opts(SecondsFormat::Micros, true);
    if formatted.len() != 27 {
        return Err(SqliteTimestampError::OutOfRange);
    }
    Ok(formatted)
}

/// Parses only the canonical fixed-width UTC timestamp accepted by tenant schemas.
pub fn parse_sqlite_timestamp(value: &str) -> Result<DateTime<Utc>, SqliteTimestampError> {
    if value.len() != 27 || !value.ends_with('Z') {
        return Err(SqliteTimestampError::InvalidFormat);
    }
    let parsed =
        DateTime::parse_from_rfc3339(value).map_err(|_| SqliteTimestampError::InvalidFormat)?;
    if parsed.offset().local_minus_utc() != 0 {
        return Err(SqliteTimestampError::InvalidFormat);
    }
    let parsed = parsed.with_timezone(&Utc);
    if format_sqlite_timestamp(&parsed).as_deref() != Ok(value) {
        return Err(SqliteTimestampError::InvalidFormat);
    }
    Ok(parsed)
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
    use proptest::prelude::*;
    use serde_json::json;

    proptest! {
        #[test]
        fn sqlite_timestamps_round_trip_at_microsecond_precision(
            seconds in -2_208_988_800_i64..=253_402_300_799_i64,
            micros in 0_u32..1_000_000_u32,
        ) {
            let timestamp = DateTime::<Utc>::from_timestamp(seconds, micros * 1_000).unwrap();
            let encoded = format_sqlite_timestamp(&timestamp).unwrap();
            prop_assert_eq!(encoded.len(), 27);
            prop_assert_eq!(parse_sqlite_timestamp(&encoded), Ok(timestamp));
        }
    }

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

    #[test]
    fn sqlite_timestamp_formatter_is_fixed_width() {
        let timestamp = DateTime::<Utc>::from_timestamp(1_760_000_000, 123_456_789).unwrap();
        let encoded = format_sqlite_timestamp(&timestamp).unwrap();
        assert_eq!(encoded, "2025-10-09T08:53:20.123456Z");
        assert_eq!(
            parse_sqlite_timestamp(&encoded)
                .unwrap()
                .timestamp_subsec_nanos(),
            123_456_000
        );
    }

    #[test]
    fn sqlite_timestamp_parser_rejects_noncanonical_values() {
        for value in [
            "2026-10-06T12:00:00Z",
            "2026-10-06T12:00:00.000000+00:00",
            "2026-10-06T12:00:00.000000+05:30",
            "2026-10-06t12:00:00.000000z",
            "2026-02-30T12:00:00.000000Z",
        ] {
            assert_eq!(
                parse_sqlite_timestamp(value),
                Err(SqliteTimestampError::InvalidFormat)
            );
        }
    }

    #[test]
    fn sqlite_timestamp_formatter_rejects_out_of_range_years() {
        let year_ten_thousand = DateTime::<Utc>::from_timestamp(253_402_300_800, 0).unwrap();
        assert_eq!(
            format_sqlite_timestamp(&year_ten_thousand),
            Err(SqliteTimestampError::OutOfRange)
        );
    }
}
