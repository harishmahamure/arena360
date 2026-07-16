use crate::error::AppError;
use crate::models::{
    device_sub_type_error_message, device_type_error_message, enum_error_message,
    normalize_device_status, normalize_device_sub_type, normalize_device_type,
    normalize_payment_method, normalize_payment_status, normalize_plan_type,
    normalize_product_category, normalize_transaction_type, normalize_unit_type, DEVICE_STATUSES,
    PAYMENT_METHODS, PAYMENT_STATUSES, PLAN_TYPES_DB, PRODUCT_CATEGORIES, TRANSACTION_TYPES,
    UNIT_TYPES,
};

pub fn require_non_empty(value: Option<String>, message: &str) -> Result<String, AppError> {
    let trimmed = value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::BadRequest(message.to_string()))?;
    Ok(trimmed)
}

/// Trim optional strings; blank becomes `None`.
pub fn trim_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Trim password and other secrets (leading/trailing whitespace only).
pub fn trim_secret(value: &str) -> String {
    value.trim().to_string()
}

/// Collapse whitespace runs to underscores after trim.
pub fn normalize_username(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
}

fn username_has_invalid_chars(value: &str) -> bool {
    !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// Validate normalized username for register/update/login.
pub fn validate_username(value: &str) -> Result<String, AppError> {
    let normalized = normalize_username(value);
    if normalized.len() < 3 || normalized.len() > 50 {
        return Err(AppError::BadRequest(
            "Username must be between 3 and 50 characters".to_string(),
        ));
    }
    if normalized.contains(char::is_whitespace) || username_has_invalid_chars(&normalized) {
        return Err(AppError::BadRequest(
            "Username can only contain letters, numbers, dots, hyphens, and underscores"
                .to_string(),
        ));
    }
    Ok(normalized)
}

/// Digits-only phone after trim.
pub fn normalize_phone_digits(value: &str) -> String {
    value.chars().filter(|c| c.is_ascii_digit()).collect()
}

const PLAYSTATION_DEVICE_TYPES: &[&str] = &["PS5", "PS4"];

pub fn is_playstation_device_type(device_type: &str) -> bool {
    PLAYSTATION_DEVICE_TYPES.contains(&device_type)
}

pub fn require_playstation_device_type(value: Option<String>) -> Result<String, AppError> {
    let normalized = require_device_type(value)?;
    if is_playstation_device_type(&normalized) {
        Ok(normalized)
    } else {
        Err(AppError::forbidden_code("DEVICE_TYPE_NOT_ALLOWED"))
    }
}

pub fn require_device_type(value: Option<String>) -> Result<String, AppError> {
    let raw = require_non_empty(value, "deviceType is required")?;
    normalize_device_type(&raw).ok_or_else(|| AppError::BadRequest(device_type_error_message()))
}

pub fn require_device_sub_type(value: Option<String>) -> Result<String, AppError> {
    let raw = require_non_empty(value, "deviceSubType is required")?;
    normalize_device_sub_type(&raw)
        .ok_or_else(|| AppError::BadRequest(device_sub_type_error_message()))
}

pub fn optional_device_type(value: Option<String>) -> Result<Option<String>, AppError> {
    optional_enum(value, normalize_device_type, device_type_error_message)
}

pub fn optional_device_sub_type(value: Option<String>) -> Result<Option<String>, AppError> {
    optional_enum(
        value,
        normalize_device_sub_type,
        device_sub_type_error_message,
    )
}

pub fn optional_device_status(value: Option<String>) -> Result<Option<String>, AppError> {
    optional_enum(value, normalize_device_status, || {
        enum_error_message("status", DEVICE_STATUSES)
    })
}

pub fn optional_product_category(value: Option<String>) -> Result<Option<String>, AppError> {
    optional_enum(value, normalize_product_category, || {
        enum_error_message("category", PRODUCT_CATEGORIES)
    })
}

pub fn require_product_category(value: Option<String>) -> Result<String, AppError> {
    let raw = require_non_empty(value, "category is required")?;
    normalize_product_category(&raw)
        .ok_or_else(|| AppError::BadRequest(enum_error_message("category", PRODUCT_CATEGORIES)))
}

pub fn optional_unit_type(value: Option<String>) -> Result<Option<String>, AppError> {
    optional_enum(value, normalize_unit_type, || {
        enum_error_message("type", UNIT_TYPES)
    })
}

pub fn require_unit_type(value: Option<String>) -> Result<String, AppError> {
    let raw = require_non_empty(value, "type is required")?;
    normalize_unit_type(&raw)
        .ok_or_else(|| AppError::BadRequest(enum_error_message("type", UNIT_TYPES)))
}

pub fn optional_plan_type(value: Option<String>) -> Result<Option<String>, AppError> {
    optional_enum(value, normalize_plan_type, || {
        enum_error_message("planType", PLAN_TYPES_DB)
    })
}

pub fn require_plan_type(value: Option<String>) -> Result<String, AppError> {
    let raw = require_non_empty(value, "planType is required")?;
    normalize_plan_type(&raw)
        .ok_or_else(|| AppError::BadRequest(enum_error_message("planType", PLAN_TYPES_DB)))
}

pub fn optional_transaction_type(value: Option<String>) -> Result<Option<String>, AppError> {
    optional_enum(value, normalize_transaction_type, || {
        enum_error_message("transactionType", TRANSACTION_TYPES)
    })
}

pub fn require_transaction_type(value: Option<String>) -> Result<String, AppError> {
    let raw = require_non_empty(value, "transactionType is required")?;
    normalize_transaction_type(&raw).ok_or_else(|| {
        AppError::BadRequest(enum_error_message("transactionType", TRANSACTION_TYPES))
    })
}

pub fn optional_payment_method(value: Option<String>) -> Result<Option<String>, AppError> {
    optional_enum(value, normalize_payment_method, || {
        enum_error_message("paymentMethod", PAYMENT_METHODS)
    })
}

pub fn require_payment_method(value: Option<String>) -> Result<String, AppError> {
    let raw = require_non_empty(value, "paymentMethod is required")?;
    normalize_payment_method(&raw)
        .ok_or_else(|| AppError::BadRequest(enum_error_message("paymentMethod", PAYMENT_METHODS)))
}

pub fn optional_payment_status(value: Option<String>) -> Result<Option<String>, AppError> {
    optional_enum(value, normalize_payment_status, || {
        enum_error_message("paymentStatus", PAYMENT_STATUSES)
    })
}

pub fn require_payment_status(value: Option<String>) -> Result<String, AppError> {
    let raw = require_non_empty(value, "paymentStatus is required")?;
    normalize_payment_status(&raw)
        .ok_or_else(|| AppError::BadRequest(enum_error_message("paymentStatus", PAYMENT_STATUSES)))
}

/// Require UPI receipt last-4 when payment includes an online portion.
///
/// Required when `payment_method` is `online`, or `split_payment` with
/// `online_amount > 0`. Value must be exactly four ASCII digits.
pub fn validate_online_payment_ref_last4(
    payment_method: &str,
    online_amount: Option<f64>,
    ref_last4: Option<String>,
) -> Result<Option<String>, AppError> {
    let requires = payment_method == "online"
        || (payment_method == "split_payment" && online_amount.unwrap_or(0.0) > 0.0);

    let trimmed = ref_last4
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if !requires {
        return Ok(None);
    }

    let value = trimmed.ok_or_else(|| {
        AppError::BadRequest(
            "onlinePaymentRefLast4 is required for online payments (last 4 digits of UPI Transaction ID)"
                .to_string(),
        )
    })?;

    if value.len() != 4 || !value.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(
            "onlinePaymentRefLast4 must be exactly 4 digits".to_string(),
        ));
    }

    Ok(Some(value))
}

fn optional_enum(
    value: Option<String>,
    normalize: fn(&str) -> Option<String>,
    error_message: fn() -> String,
) -> Result<Option<String>, AppError> {
    match value {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => normalize(&s)
            .map(Some)
            .ok_or_else(|| AppError::BadRequest(error_message())),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_phone_digits, normalize_username, trim_secret, validate_online_payment_ref_last4,
        validate_username,
    };

    #[test]
    fn normalize_username_collapses_whitespace() {
        assert_eq!(normalize_username(" Pranshu  Jha "), "Pranshu_Jha");
        assert_eq!(normalize_username("Yuvraj "), "Yuvraj");
    }

    #[test]
    fn validate_username_rejects_spaces_after_normalize() {
        assert!(validate_username("ab").is_err());
        assert!(validate_username("valid_user").is_ok());
    }

    #[test]
    fn trim_secret_strips_edges() {
        assert_eq!(trim_secret("  secret  "), "secret");
    }

    #[test]
    fn normalize_phone_digits_strips_formatting() {
        assert_eq!(normalize_phone_digits("98 7654 3210"), "9876543210");
    }

    #[test]
    fn online_ref_required_for_online_method() {
        assert!(validate_online_payment_ref_last4("online", Some(100.0), None).is_err());
        assert_eq!(
            validate_online_payment_ref_last4("online", Some(100.0), Some("1234".into())).unwrap(),
            Some("1234".into())
        );
    }

    #[test]
    fn online_ref_required_for_split_with_online_amount() {
        assert!(
            validate_online_payment_ref_last4("split_payment", Some(50.0), None).is_err()
        );
        assert_eq!(
            validate_online_payment_ref_last4(
                "split_payment",
                Some(50.0),
                Some(" 9876 ".into())
            )
            .unwrap(),
            Some("9876".into())
        );
    }

    #[test]
    fn online_ref_not_required_for_cash_or_credit() {
        assert_eq!(
            validate_online_payment_ref_last4("cash", Some(0.0), Some("1234".into())).unwrap(),
            None
        );
        assert_eq!(
            validate_online_payment_ref_last4("credit", None, None).unwrap(),
            None
        );
        assert_eq!(
            validate_online_payment_ref_last4("split_payment", Some(0.0), None).unwrap(),
            None
        );
    }

    #[test]
    fn online_ref_rejects_non_digit_or_wrong_length() {
        assert!(
            validate_online_payment_ref_last4("online", Some(10.0), Some("12".into())).is_err()
        );
        assert!(
            validate_online_payment_ref_last4("online", Some(10.0), Some("12ab".into())).is_err()
        );
        assert!(
            validate_online_payment_ref_last4("online", Some(10.0), Some("12345".into())).is_err()
        );
    }
}
