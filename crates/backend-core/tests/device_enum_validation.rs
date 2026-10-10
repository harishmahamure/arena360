use gaming_cafe_api::models::{normalize_device_sub_type, normalize_device_type};
use gaming_cafe_api::validation::{
    optional_device_type, require_device_sub_type, require_device_type,
};

#[test]
fn normalizes_lowercase_device_type() {
    assert_eq!(normalize_device_type("pc").as_deref(), Some("PC"));
}

#[test]
fn rejects_invalid_device_subtype_label() {
    assert!(normalize_device_sub_type("gaming").is_none());
}

#[test]
fn require_device_type_rejects_empty() {
    assert!(require_device_type(None).is_err());
    assert!(require_device_type(Some("".into())).is_err());
}

#[test]
fn optional_device_type_accepts_pc_and_blank() {
    assert_eq!(
        optional_device_type(Some("pc".into())).unwrap().as_deref(),
        Some("PC")
    );
    assert_eq!(optional_device_type(None).unwrap(), None);
    assert_eq!(optional_device_type(Some("  ".into())).unwrap(), None);
}

#[test]
fn require_device_sub_type_accepts_mid_range_pcs() {
    assert_eq!(
        require_device_sub_type(Some("mid_range_pcs".into())).unwrap(),
        "MID_RANGE_PCS"
    );
}
