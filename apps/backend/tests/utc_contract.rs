use std::fs;
use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use chrono::{DateTime, FixedOffset};
use gaming_cafe_api::dto::SuccessResponse;
use gaming_cafe_api::time::validate_utc_timestamps;
use serde_json::json;

#[test]
fn api_and_outbox_payload_timestamps_end_in_z() {
    let offset_time: DateTime<FixedOffset> =
        DateTime::parse_from_rfc3339("2026-10-06T09:07:00+05:30").unwrap();
    let response = SuccessResponse::new(
        StatusCode::OK,
        json!({
            "createdAt": offset_time.with_timezone(&chrono::Utc),
            "calendarDate": "2026-10-06"
        }),
    );
    let payload = serde_json::to_value(response).unwrap();
    validate_utc_timestamps(&payload).unwrap();
    assert!(payload["timestamp"].as_str().unwrap().ends_with('Z'));
    assert!(payload["data"]["createdAt"]
        .as_str()
        .unwrap()
        .ends_with('Z'));

    let invalid_outbox_payload = json!({
        "occurredAt": "2026-10-06T09:07:00+05:30"
    });
    assert!(validate_utc_timestamps(&invalid_outbox_payload).is_err());
}

#[test]
fn postgres_migrations_only_use_timezone_aware_timestamps() {
    let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    for path in files_with_extension(&migrations, "sql") {
        let sql = fs::read_to_string(&path).unwrap();
        for (line_number, line) in sql.lines().enumerate() {
            let statement = line.split("--").next().unwrap_or_default();
            let has_naive_timestamp = statement
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                .any(|token| {
                    matches!(
                        token.to_ascii_uppercase().as_str(),
                        "TIMESTAMP" | "DATETIME"
                    )
                });
            assert!(
                !has_naive_timestamp,
                "{}:{} uses a timezone-naive timestamp type",
                path.display(),
                line_number + 1
            );
        }
    }
}

#[test]
fn wire_timestamps_use_the_canonical_formatter() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for path in files_with_extension(&source, "rs") {
        if path.ends_with("time.rs") {
            continue;
        }
        let contents = fs::read_to_string(&path).unwrap();
        assert!(
            !contents.contains(".to_rfc3339"),
            "{} bypasses the canonical UTC formatter",
            path.display()
        );
    }
}

fn files_with_extension(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some(extension) {
                files.push(path);
            }
        }
    }
    files
}
