//! Verified object datasets and isolated historical workers.
#[cfg(feature = "duckdb-analytics")]
pub mod hot;
pub mod objects;

#[cfg(feature="duckdb-analytics")]
pub mod raw;

#[cfg(feature="duckdb-analytics")]
pub mod policy;

#[cfg(feature="duckdb-analytics")]
pub mod archive;

#[cfg(feature="duckdb-analytics")]
pub mod handoff;
