use gaming_cafe_api::models::{
    normalize_product_category, normalize_unit_type, PRODUCT_CATEGORIES, UNIT_TYPES,
};
use gaming_cafe_api::validation::{
    optional_product_category, optional_unit_type, require_product_category, require_unit_type,
};

#[test]
fn product_category_accepts_db_labels() {
    for label in PRODUCT_CATEGORIES {
        assert_eq!(normalize_product_category(label).as_deref(), Some(*label));
    }
}

#[test]
fn unit_type_rejects_dozen() {
    assert!(normalize_unit_type("dozen").is_none());
}

#[test]
fn unit_type_accepts_carton_and_can() {
    assert_eq!(normalize_unit_type("carton").as_deref(), Some("carton"));
    assert_eq!(normalize_unit_type("can").as_deref(), Some("can"));
}

#[test]
fn require_unit_type_normalizes_case() {
    let value = require_unit_type(Some("Piece".into())).unwrap();
    assert_eq!(value, "piece");
}

#[test]
fn optional_product_category_blank_is_none() {
    assert_eq!(optional_product_category(Some("  ".into())).unwrap(), None);
}
