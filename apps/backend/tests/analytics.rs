use gaming_cafe_api::analytics::{
    query_as,
    worker::{project, Change},
    ClickHouse,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn projections_allowlist_fields_and_preserve_decimal_precision() {
    let id = Uuid::new_v4();
    let mut change = Change { schema_version: 1, source_table: "users".into(), row_id: id, version: 42,
        deleted: false, row_data: serde_json::from_str(&format!(r#"{{"id":"{id}","username":"test","role":"player","isActive":true,"creditLimit":999999999999999.1234,"password_hash":"secret","totpSecret":"secret"}}"#)).unwrap() };
    let row = project(&change).unwrap();
    assert!(row.get("password_hash").is_none());
    assert!(row.get("totpSecret").is_none());
    assert_eq!(row["creditLimit"].to_string(), "999999999999999.1234");
    assert_eq!(row["_version"], 42);
    change.deleted = true;
    assert_eq!(project(&change).unwrap()["_deleted"], 1);
    change.schema_version = 2;
    assert!(project(&change).is_err());
    change.schema_version = 1;
    change.row_id = Uuid::new_v4();
    assert!(project(&change).is_err());
    change.source_table = "untrusted; DROP TABLE users".into();
    assert!(project(&change).is_err());
}

#[tokio::test]
async fn unavailable_clickhouse_is_an_error_not_zero_or_postgres_fallback() {
    let ch = ClickHouse::new(
        "http://127.0.0.1:1".into(),
        "unused".into(),
        "unused".into(),
        "".into(),
    );
    let err = query_as::<(i64,)>("SELECT count() FROM users")
        .fetch_one(&ch)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("ANALYTICS_UNAVAILABLE"));
}
