//! Tenant-local DuckDB reports, fed by canonical SQLite outbox snapshots.
pub mod business;
pub mod reports;


pub mod publisher;

#[cfg(feature = "duckdb-analytics")]
pub mod tenant_db;

#[cfg(feature = "duckdb-analytics")]
pub mod consumer;
#[cfg(feature = "duckdb-analytics")]
pub mod session_hours;

#[cfg(feature = "duckdb-analytics")]
pub mod rebuild;

#[cfg(feature = "duckdb-analytics")]
pub mod retention;

#[cfg(feature = "duckdb-analytics")]
pub mod registry;

#[cfg_attr(not(feature = "duckdb-analytics"), path = "report_reader_disabled.rs")]
pub mod report_reader;
pub mod calendar;
