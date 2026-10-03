//! ClickHouse is the sole reporting store; PostgreSQL remains the transaction ledger.
pub mod business;
mod client;
pub mod reports;
pub mod worker;
pub use client::{query_as, ClickHouse};

pub mod scope;
