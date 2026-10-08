//! Full operational rows for lossless archives and bounded SQLite backfills.
use crate::error::AppError;
use futures::TryStreamExt;
use sqlx::Row;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Archive rows: {e}"))
}
pub fn literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
pub fn identifier(s: &str) -> Result<String, AppError> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return Err(fail("Invalid column identifier"));
    }
    Ok(format!("\"{s}\""))
}
pub use super::objects::Column;
pub async fn columns(pool: &sqlx::SqlitePool, table: &str) -> Result<Vec<Column>, AppError> {
    let table = identifier(table)?;
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await?;
    let mut columns = vec![];
    for row in rows {
        let kind: String = row.get("type");
        if !matches!(kind.as_str(), "INTEGER" | "TEXT") {
            return Err(fail("Unsupported archive source column type"));
        }
        columns.push(Column {
            name: row.get("name"),
            kind,
            primary: row.get::<i64, _>("pk") > 0,
        });
    }
    if columns.is_empty()
        || columns.iter().filter(|c| c.primary).count() != 1
        || !columns.iter().any(|c| c.primary && c.name == "id")
    {
        return Err(fail("Archive source requires a scalar id key"));
    }
    Ok(columns)
}
pub fn row_select(table: &str, columns: &[Column], predicate: &str) -> Result<String, AppError> {
    let fields = columns
        .iter()
        .map(|c| Ok(format!("{},r.{}", literal(&c.name), identifier(&c.name)?)))
        .collect::<Result<Vec<_>, AppError>>()?
        .join(",");
    Ok(format!("SELECT CAST(r.id AS TEXT) AS row_id,json_object({fields}) AS payload FROM {} r WHERE {predicate} ORDER BY r.id",identifier(table)?))
}
pub fn row_hash(payload: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(payload.as_bytes()))
}
pub async fn parquet(
    pool: &sqlx::SqlitePool,
    table: &str,
    columns: &[Column],
    predicate: &str,
    root: &Path,
) -> Result<(PathBuf, i64, String), AppError> {
    let select = row_select(table, columns, predicate)?;
    let raw = root.join(format!("{table}.rows.ndjson"));
    let mut file = File::create(&raw).map_err(fail)?;
    let mut count = 0i64;
    let mut source = sqlx::query(&select).fetch(pool);
    while let Some(row) = source.try_next().await? {
        let payload: String = row.get("payload");
        let id: String = row.get("row_id");
        let value = serde_json::json!({"id":id,"payload":payload,"checksum":row_hash(&payload)});
        serde_json::to_writer(&mut file, &value).map_err(fail)?;
        file.write_all(b"\n").map_err(fail)?;
        count += 1;
    }
    file.sync_all().map_err(fail)?;
    drop(file);
    let checksum = crate::replication::snapshot::hash_file(&raw)?;
    let output = root.join(format!("{table}.parquet"));
    let path = output.clone();
    let temp = root.join("duck-temp");
    tokio::task::spawn_blocking(move||{
  let connection=duckdb::Connection::open_in_memory().map_err(fail)?;
  connection.execute_batch(&format!("SET threads=1;SET memory_limit='128MB';SET temp_directory={};CREATE TABLE archive_rows(id VARCHAR PRIMARY KEY,payload VARCHAR NOT NULL,checksum VARCHAR NOT NULL)",literal(temp.to_str().ok_or_else(||fail("Invalid staging path"))?))).map_err(fail)?;
  if count>0{connection.execute_batch(&format!("INSERT INTO archive_rows SELECT json_extract_string(json,'$.id'),json_extract_string(json,'$.payload'),json_extract_string(json,'$.checksum') FROM read_ndjson_objects({})",literal(raw.to_str().unwrap()))).map_err(fail)?;}
  connection.execute_batch(&format!("COPY (SELECT * FROM archive_rows ORDER BY id) TO {} (FORMAT PARQUET,COMPRESSION ZSTD)",literal(path.to_str().unwrap()))).map_err(fail)?;
  let checked:i64=connection.query_row(&format!("SELECT count(*) FROM read_parquet({})",literal(path.to_str().unwrap())),[],|r|r.get(0)).map_err(fail)?;if checked!=count{return Err(fail("Archive Parquet row total differs"));}Ok::<_,AppError>(())
 }).await.map_err(fail)??;
    Ok((output, count, checksum))
}
pub async fn batch(
    path: PathBuf,
    after: Option<String>,
    limit: usize,
) -> Result<Vec<(String, String, String)>, AppError> {
    tokio::task::spawn_blocking(move||{
  if !(1..=1000).contains(&limit){return Err(fail("Invalid archive batch size"));}
  let connection=duckdb::Connection::open_in_memory().map_err(fail)?;connection.execute_batch("SET threads=1;SET memory_limit='128MB'").map_err(fail)?;
  let mut query=connection.prepare(&format!("SELECT id,payload,checksum FROM read_parquet({}) WHERE id>? ORDER BY id LIMIT {limit}",literal(path.to_str().ok_or_else(||fail("Invalid archive path"))?))).map_err(fail)?;
  let rows=query.query_map([after.unwrap_or_default()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(fail)?.collect::<Result<Vec<_>,_>>().map_err(fail)?;Ok(rows)
 }).await.map_err(fail)?
}
pub async fn fingerprint(
    pool: &sqlx::SqlitePool,
    table: &str,
    columns: &[Column],
    predicate: &str,
) -> Result<(i64, String), AppError> {
    use sha2::{Digest, Sha256};
    let select = row_select(table, columns, predicate)?;
    let mut source = sqlx::query(&select).fetch(pool);
    let mut digest = Sha256::new();
    let mut count = 0;
    while let Some(row) = source.try_next().await? {
        let payload: String = row.get("payload");
        let id: String = row.get("row_id");
        let value = serde_json::json!({"id":id,"payload":payload,"checksum":row_hash(&payload)});
        digest.update(serde_json::to_vec(&value).map_err(fail)?);
        digest.update(b"\n");
        count += 1;
    }
    Ok((count, hex::encode(digest.finalize())))
}
