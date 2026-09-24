use chrono::NaiveTime;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::models::{SettingDefinition, SettingScope, SettingValueType};

const ISO_4217_CODES: &[&str] = &[
    "AED", "AUD", "BRL", "CAD", "CHF", "CNY", "CZK", "DKK", "EUR", "GBP", "HKD", "HUF", "IDR",
    "ILS", "INR", "JPY", "KRW", "MXN", "MYR", "NOK", "NZD", "PHP", "PLN", "RON", "RUB", "SAR",
    "SEK", "SGD", "THB", "TRY", "TWD", "USD", "VND", "ZAR",
];

fn definition(
    key: &str,
    category: &str,
    description: &str,
    value_type: SettingValueType,
    default_value: Value,
    location_override: bool,
    owner: &str,
) -> SettingDefinition {
    let mut allowed_scopes = vec![SettingScope::Organization];
    if location_override {
        allowed_scopes.push(SettingScope::Location);
    }
    let validation = match key {
        "pricing.tax_rate" => json!({"minimum": 0, "maximum": 100}),
        "pricing.default_per_minute_rate" => json!({"minimum": 0, "maximum": 100000}),
        "plans.default_validity_days" | "staff.allowance_period_days" => {
            json!({"minimum": 1, "maximum": 3650})
        }
        "plans.default_time_credits" => json!({"minimum": 1, "maximum": 525600}),
        "sessions.ending_warning_minutes" => json!({"minimum": 0, "maximum": 120}),
        "sessions.offline_grace_minutes" => json!({"minimum": 0, "maximum": 1440}),
        "notifications.retention_days" => json!({"minimum": 1, "maximum": 365}),
        "pricing.night_window_start" | "pricing.night_window_end" => {
            json!({"format": "HH:MM"})
        }
        "venue.timezone" => json!({"format": "iana-timezone"}),
        "pricing.currency" => json!({"format": "iso-4217"}),
        _ if value_type == SettingValueType::String => json!({"maxLength": 2000}),
        _ => json!({}),
    };
    SettingDefinition {
        key: key.to_string(),
        category: category.to_string(),
        description: description.to_string(),
        value_type,
        default_value,
        allowed_scopes,
        validation,
        sensitive: false,
        owner: owner.to_string(),
    }
}

pub fn catalog(default_timezone: &str) -> Vec<SettingDefinition> {
    vec![
        definition(
            "business.name",
            "business",
            "Venue display name",
            SettingValueType::String,
            json!("Gaming Cafe"),
            true,
            "venue",
        ),
        definition(
            "business.address",
            "business",
            "Venue address",
            SettingValueType::String,
            json!(""),
            true,
            "venue",
        ),
        definition(
            "business.phone",
            "business",
            "Venue phone number",
            SettingValueType::String,
            json!(""),
            true,
            "venue",
        ),
        definition(
            "business.email",
            "business",
            "Venue contact email",
            SettingValueType::String,
            json!(""),
            true,
            "venue",
        ),
        definition(
            "business.gst_number",
            "business",
            "GST registration number",
            SettingValueType::String,
            json!(""),
            true,
            "venue",
        ),
        definition(
            "business.logo_url",
            "business",
            "Venue logo URL",
            SettingValueType::String,
            json!(""),
            true,
            "venue",
        ),
        definition(
            "venue.timezone",
            "venue",
            "IANA timezone used for venue rules",
            SettingValueType::Timezone,
            json!(default_timezone),
            true,
            "venue",
        ),
        definition(
            "pricing.currency",
            "pricing",
            "ISO 4217 currency code",
            SettingValueType::Currency,
            json!("INR"),
            true,
            "pricing",
        ),
        definition(
            "pricing.tax_rate",
            "pricing",
            "Default tax percentage",
            SettingValueType::Number,
            json!(18.0),
            true,
            "pricing",
        ),
        definition(
            "pricing.default_per_minute_rate",
            "pricing",
            "Fallback per-minute rate",
            SettingValueType::Number,
            json!(2.0),
            true,
            "pricing",
        ),
        definition(
            "pricing.night_window_start",
            "pricing",
            "Night price start in local HH:MM",
            SettingValueType::String,
            json!("23:00"),
            true,
            "pricing",
        ),
        definition(
            "pricing.night_window_end",
            "pricing",
            "Night price end in local HH:MM",
            SettingValueType::String,
            json!("08:00"),
            true,
            "pricing",
        ),
        definition(
            "receipt.header_text",
            "receipt",
            "Receipt header text",
            SettingValueType::String,
            json!("Thank you for visiting!"),
            true,
            "pos",
        ),
        definition(
            "receipt.footer_text",
            "receipt",
            "Receipt footer text",
            SettingValueType::String,
            json!("Visit again soon!"),
            true,
            "pos",
        ),
        definition(
            "receipt.show_gst",
            "receipt",
            "Show GST details on receipts",
            SettingValueType::Boolean,
            json!(true),
            true,
            "pos",
        ),
        definition(
            "plans.default_validity_days",
            "plans",
            "Default plan validity",
            SettingValueType::Integer,
            json!(30),
            true,
            "plans",
        ),
        definition(
            "plans.default_time_credits",
            "plans",
            "Default plan time credits in minutes",
            SettingValueType::Integer,
            json!(60),
            true,
            "plans",
        ),
        definition(
            "staff.allowance_period_days",
            "staff",
            "Staff gaming allowance period",
            SettingValueType::Integer,
            json!(30),
            true,
            "staff",
        ),
        definition(
            "sessions.ending_warning_minutes",
            "sessions",
            "Session ending warning lead time",
            SettingValueType::Integer,
            json!(5),
            true,
            "sessions",
        ),
        definition(
            "sessions.offline_grace_minutes",
            "sessions",
            "Offline session grace period",
            SettingValueType::Integer,
            json!(15),
            true,
            "sessions",
        ),
        definition(
            "notifications.retention_days",
            "notifications",
            "Notification retention period",
            SettingValueType::Integer,
            json!(7),
            true,
            "notifications",
        ),
        definition(
            "inventory.default_warehouse_id",
            "inventory",
            "Default warehouse stock location",
            SettingValueType::Uuid,
            Value::Null,
            true,
            "inventory",
        ),
        definition(
            "inventory.default_store_id",
            "inventory",
            "Default store stock location",
            SettingValueType::Uuid,
            Value::Null,
            true,
            "inventory",
        ),
        definition(
            "pos.default_sale_location_id",
            "pos",
            "Default POS stock location",
            SettingValueType::Uuid,
            Value::Null,
            true,
            "pos",
        ),
    ]
}

pub fn find(default_timezone: &str, key: &str) -> Option<SettingDefinition> {
    catalog(default_timezone)
        .into_iter()
        .find(|item| item.key == key)
}

pub fn validate(
    default_timezone: &str,
    key: &str,
    value: &Value,
    location_scope: bool,
) -> Result<SettingDefinition, AppError> {
    let definition = find(default_timezone, key)
        .ok_or_else(|| AppError::BadRequest(format!("Unknown setting key '{key}'")))?;
    if location_scope && !definition.allowed_scopes.contains(&SettingScope::Location) {
        return Err(AppError::BadRequest(format!(
            "Setting '{key}' does not allow location overrides"
        )));
    }

    match definition.value_type {
        SettingValueType::String => {
            let text = value.as_str().ok_or_else(|| type_error(key, "string"))?;
            if text.len() > 2_000 {
                return Err(AppError::BadRequest(format!(
                    "Setting '{key}' exceeds 2000 characters"
                )));
            }
            if matches!(
                key,
                "pricing.night_window_start" | "pricing.night_window_end"
            ) && NaiveTime::parse_from_str(text, "%H:%M").is_err()
            {
                return Err(AppError::BadRequest(format!(
                    "Setting '{key}' must use HH:MM"
                )));
            }
        }
        SettingValueType::Number => {
            let number = value.as_f64().ok_or_else(|| type_error(key, "number"))?;
            if !number.is_finite() {
                return Err(AppError::BadRequest(format!(
                    "Setting '{key}' must be finite"
                )));
            }
            match key {
                "pricing.tax_rate" if !(0.0..=100.0).contains(&number) => {
                    return Err(AppError::BadRequest(
                        "pricing.tax_rate must be between 0 and 100".to_string(),
                    ));
                }
                "pricing.default_per_minute_rate" if !(0.0..=100_000.0).contains(&number) => {
                    return Err(AppError::BadRequest(
                        "pricing.default_per_minute_rate is outside the supported range"
                            .to_string(),
                    ));
                }
                _ => {}
            }
        }
        SettingValueType::Integer => {
            let number = value.as_i64().ok_or_else(|| type_error(key, "integer"))?;
            let valid = match key {
                "plans.default_validity_days" | "staff.allowance_period_days" => {
                    (1..=3_650).contains(&number)
                }
                "plans.default_time_credits" => (1..=525_600).contains(&number),
                "sessions.ending_warning_minutes" => (0..=120).contains(&number),
                "sessions.offline_grace_minutes" => (0..=1_440).contains(&number),
                "notifications.retention_days" => (1..=365).contains(&number),
                _ => true,
            };
            if !valid {
                return Err(AppError::BadRequest(format!(
                    "Setting '{key}' is outside the supported range"
                )));
            }
        }
        SettingValueType::Boolean => {
            if !value.is_boolean() {
                return Err(type_error(key, "boolean"));
            }
        }
        SettingValueType::Uuid => {
            if !value.is_null()
                && value
                    .as_str()
                    .and_then(|text| uuid::Uuid::parse_str(text).ok())
                    .is_none()
            {
                return Err(type_error(key, "UUID string or null"));
            }
        }
        SettingValueType::Timezone => {
            let timezone = value
                .as_str()
                .ok_or_else(|| type_error(key, "IANA timezone"))?;
            timezone.parse::<chrono_tz::Tz>().map_err(|_| {
                AppError::BadRequest(format!("Setting '{key}' is not a valid IANA timezone"))
            })?;
        }
        SettingValueType::Currency => {
            let currency = value
                .as_str()
                .ok_or_else(|| type_error(key, "ISO 4217 currency"))?;
            if !ISO_4217_CODES.contains(&currency) {
                return Err(AppError::BadRequest(format!(
                    "Setting '{key}' is not a supported ISO 4217 currency"
                )));
            }
        }
    }
    Ok(definition)
}

fn type_error(key: &str, expected: &str) -> AppError {
    AppError::BadRequest(format!("Setting '{key}' must be {expected}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_keys_and_invalid_timezones() {
        assert!(validate("Asia/Kolkata", "missing.key", &json!(true), false).is_err());
        assert!(validate(
            "Asia/Kolkata",
            "venue.timezone",
            &json!("Mars/Olympus"),
            false
        )
        .is_err());
    }

    #[test]
    fn validates_ranges_and_currency() {
        assert!(validate("Asia/Kolkata", "pricing.tax_rate", &json!(101), false).is_err());
        assert!(validate("Asia/Kolkata", "pricing.currency", &json!("INR"), false).is_ok());
        assert!(validate("Asia/Kolkata", "pricing.currency", &json!("XYZ"), false).is_err());
    }
}
