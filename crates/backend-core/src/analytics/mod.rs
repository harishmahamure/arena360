//! Tenant reports and optional event-driven analytics compatibility.
pub mod business;
#[cfg_attr(not(feature = "duckdb-analytics"), path = "sqlite_reports.rs")]
pub mod reports;

pub mod outbox_retention;
pub mod publisher;

#[cfg(feature = "duckdb-analytics")]
pub mod tenant_db;

#[cfg(feature = "duckdb-analytics")]
pub mod consumer;
pub mod session_hours;

#[cfg(feature = "duckdb-analytics")]
pub mod rebuild;

#[cfg(feature = "duckdb-analytics")]
pub mod retention;

#[cfg(feature = "duckdb-analytics")]
pub mod registry;

pub mod calendar;
#[cfg_attr(not(feature = "duckdb-analytics"), path = "sqlite_reader.rs")]
pub mod report_reader;

/// Each reporting backend uses explicit SQL, while public DTOs stay shared.
#[macro_export]
macro_rules! report_sql {
    ($path:literal) => {{
        #[cfg(feature = "duckdb-analytics")]
        const SQL: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/analytics/queries/",
            $path
        ));
        #[cfg(not(feature = "duckdb-analytics"))]
        const SQL: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/analytics/queries/sqlite/",
            $path
        ));
        SQL
    }};
}
