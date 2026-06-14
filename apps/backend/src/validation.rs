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
