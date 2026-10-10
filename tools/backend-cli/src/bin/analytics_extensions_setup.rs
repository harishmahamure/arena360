//! Provision the matching, signature-verified SQLite extension in a build image.
#[cfg(feature = "duckdb-analytics")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().skip(1).next().is_some() {
        return Err("Usage: DUCKDB_EXTENSION_DIR=DIR analytics_extensions_setup".into());
    }
    let path = std::env::var("DUCKDB_EXTENSION_DIR")?;
    std::fs::create_dir_all(&path)?;
    let connection = duckdb::Connection::open_in_memory_with_flags(
        duckdb::Config::default().threads(1)?.max_memory("128MB")?,
    )?;
    let unsigned: bool = connection.query_row(
        "SELECT current_setting('allow_unsigned_extensions')",
        [],
        |r| r.get(0),
    )?;
    if unsigned {
        return Err("Unsigned extensions must remain disabled".into());
    }
    connection.execute_batch(&format!("SET extension_directory='{}'; INSTALL sqlite FROM 'https://extensions.duckdb.org'; LOAD sqlite",path.replace('\'',"''")))?;
    println!("Matching signature-verified SQLite extension is ready.");
    Ok(())
}
#[cfg(not(feature = "duckdb-analytics"))]
fn main() {
    eprintln!("Build analytics_extensions_setup with duckdb-analytics or duckdb-bundled.");
    std::process::exit(2);
}
