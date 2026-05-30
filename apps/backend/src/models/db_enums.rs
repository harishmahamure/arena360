//! Postgres enum labels — keep aligned with `packages/contracts/src/enums.ts`.

// Device / plan scope (case-sensitive uppercase)
pub const DEVICE_TYPES: &[&str] = &["PC", "CONSOLE", "PS5", "PS4", "OTHER"];
pub const DEVICE_SUB_TYPES: &[&str] = &[
    "HIGH_END_PCS",
    "MID_RANGE_PCS",
    "PREMIUM_TV_CONSOLES",
    "STANDARD_TV_CONSOLES",
    "OTHER",
];
pub const DEFAULT_DEVICE_TYPE: &str = "OTHER";
pub const DEFAULT_DEVICE_SUB_TYPE: &str = "OTHER";

pub const DEVICE_STATUSES: &[&str] = &[
    "operational",
    "under_maintenance",
    "out_of_service",
    "in_use",
    "available",
];

pub const PLAN_TYPES_ADMIN: &[&str] = &["time_based", "weekend_special"];
pub const PLAN_TYPES_DB: &[&str] = &[
    "time_based",
    "session_based",
    "unlimited_daily",
    "hourly_rental",
    "monthly_subscription",
    "weekend_special",
];

pub const PRODUCT_CATEGORIES: &[&str] = &["beverage", "snack", "meal", "other"];
pub const DEFAULT_PRODUCT_CATEGORY: &str = "other";

pub const UNIT_TYPES: &[&str] = &[
    "piece",
    "box",
    "carton",
    "pack",
    "bottle",
    "can",
    "kilogram",
    "gram",
    "liter",
    "milliliter",
    "other",
];
pub const DEFAULT_UNIT_TYPE: &str = "other";

pub const TRANSACTION_TYPES: &[&str] = &["plan_purchase", "product_purchase"];
pub const PAYMENT_METHODS: &[&str] = &["cash", "online", "split_payment", "credit"];
pub const PAYMENT_STATUSES: &[&str] = &["pending", "completed", "failed", "refunded", "credit"];

pub fn normalize_uppercase_enum(value: &str, allowed: &[&str]) -> Option<String> {
    let upper = value.trim().to_uppercase();
    if allowed.iter().any(|label| *label == upper) {
        Some(upper)
    } else {
        None
    }
}

pub fn normalize_exact_enum(value: &str, allowed: &[&str]) -> Option<String> {
    let trimmed = value.trim();
    if allowed.iter().any(|label| *label == trimmed) {
        return Some(trimmed.to_string());
    }
    allowed
        .iter()
        .find(|label| label.eq_ignore_ascii_case(trimmed))
        .map(|label| (*label).to_string())
}

pub fn enum_error_message(field: &str, allowed: &[&str]) -> String {
    format!("{field} must be one of: {}", allowed.join(", "))
}

pub fn normalize_device_type(value: &str) -> Option<String> {
    normalize_uppercase_enum(value, DEVICE_TYPES)
}

pub fn normalize_device_sub_type(value: &str) -> Option<String> {
    normalize_uppercase_enum(value, DEVICE_SUB_TYPES)
}

pub fn device_type_error_message() -> String {
    enum_error_message("deviceType", DEVICE_TYPES)
}

pub fn device_sub_type_error_message() -> String {
    enum_error_message("deviceSubType", DEVICE_SUB_TYPES)
}

pub fn normalize_device_status(value: &str) -> Option<String> {
    normalize_exact_enum(value, DEVICE_STATUSES)
}

pub fn normalize_product_category(value: &str) -> Option<String> {
    normalize_exact_enum(value, PRODUCT_CATEGORIES)
}

pub fn normalize_unit_type(value: &str) -> Option<String> {
    normalize_exact_enum(value, UNIT_TYPES)
}

pub fn normalize_plan_type(value: &str) -> Option<String> {
    normalize_exact_enum(value, PLAN_TYPES_DB)
}

pub fn normalize_transaction_type(value: &str) -> Option<String> {
    normalize_exact_enum(value, TRANSACTION_TYPES)
}

pub fn normalize_payment_method(value: &str) -> Option<String> {
    normalize_exact_enum(value, PAYMENT_METHODS)
}

pub fn normalize_payment_status(value: &str) -> Option<String> {
    normalize_exact_enum(value, PAYMENT_STATUSES)
}
