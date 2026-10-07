//! Legacy ClickHouse definitions retained for M7 parity. Operational writes use tenant SQLite.
pub mod business;
mod client;
pub mod reports;
pub mod worker;
pub use client::{query_as, ClickHouse};

pub mod scope;
