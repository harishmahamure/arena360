//! Authenticated encrypted Parquet objects, independent of live DuckDB handles.
use crate::{
    error::AppError,
    replication::{crypto, snapshot},
};
use futures::TryStreamExt;
use object_store::{path::Path as ObjectPath, ObjectStore, ObjectStoreExt};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Write, path::Path};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub kind: String,
    pub primary: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<Column>,
    pub table: String,
    pub key: String,
    pub rows: i64,
    pub bytes: u64,
    pub checksum_sha256: String,
    pub source_checksum: String,
    pub plaintext_checksum: String,
    pub plaintext_bytes: u64,
}
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("Historical object: {e}"))
}
pub async fn upload(
    store: &dyn ObjectStore,
    key: &[u8; 32],
    object_key: String,
    table: String,
    rows: i64,
    source_checksum: String,
    path: &Path,
) -> Result<Object, AppError> {
    let plain_bytes = std::fs::metadata(path).map_err(fail)?.len();
    let plaintext_checksum = snapshot::hash_file(path)?;
    let encoded = path.with_extension("encrypted");
    let mut output = File::create(&encoded).map_err(fail)?;
    crypto::encode_file(
        key,
        &object_key,
        File::open(path).map_err(fail)?,
        &mut output,
    )?;
    output.sync_all().map_err(fail)?;
    drop(output);
    let checksum_sha256 = snapshot::hash_file(&encoded)?;
    let bytes = std::fs::metadata(&encoded).map_err(fail)?.len();
    put_file(store, &ObjectPath::from(object_key.as_str()), &encoded).await?;
    let object = Object {
        columns: vec![],
        table,
        key: object_key,
        rows,
        bytes,
        checksum_sha256,
        source_checksum,
        plaintext_checksum,
        plaintext_bytes: plain_bytes,
    };
    let checked = path.with_extension("checked.parquet");
    download(store, key, &object, &checked).await?;
    std::fs::remove_file(checked).map_err(fail)?;
    Ok(object)
}
pub async fn download(
    store: &dyn ObjectStore,
    key: &[u8; 32],
    object: &Object,
    target: &Path,
) -> Result<(), AppError> {
    if object.rows < 0 || object.bytes == 0 || object.plaintext_bytes == 0 {
        return Err(fail("Invalid object dimensions"));
    }
    let encoded = target.with_extension("download");
    let result = async {
        let mut output = File::create(&encoded).map_err(fail)?;
        let mut remote = store
            .get(&ObjectPath::from(object.key.as_str()))
            .await
            .map_err(fail)?
            .into_stream();
        let mut count = 0u64;
        while let Some(chunk) = remote.try_next().await.map_err(fail)? {
            count = count
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| fail("Object size overflow"))?;
            if count > object.bytes {
                return Err(fail("Object exceeds declared size"));
            }
            output.write_all(&chunk).map_err(fail)?;
        }
        output.sync_all().map_err(fail)?;
        drop(output);
        if count != object.bytes || snapshot::hash_file(&encoded)? != object.checksum_sha256 {
            return Err(fail("Encrypted checksum mismatch"));
        }
        let mut decoded = File::create(target).map_err(fail)?;
        let size = crypto::decode_file(
            key,
            &object.key,
            File::open(&encoded).map_err(fail)?,
            &mut decoded,
            object.plaintext_bytes,
        )?;
        decoded.sync_all().map_err(fail)?;
        drop(decoded);
        if size != object.plaintext_bytes
            || snapshot::hash_file(target)? != object.plaintext_checksum
        {
            return Err(fail("Plaintext checksum mismatch"));
        }
        Ok(())
    }
    .await;
    let _ = std::fs::remove_file(encoded);
    if result.is_err() {
        let _ = std::fs::remove_file(target);
    }
    result
}

/// Generated object keys contain a fresh UUID or a fenced worker token. Large
/// uploads stream multipart data, so neither RAM nor the single-PUT limit bounds exports.
pub(crate) async fn put_file(
    store: &dyn ObjectStore,
    path: &ObjectPath,
    file: &Path,
) -> Result<(), AppError> {
    match store.head(path).await {
        Ok(_) => return Err(AppError::Conflict("Immutable object already exists".into())),
        Err(object_store::Error::NotFound { .. }) => {}
        Err(error) => return Err(fail(error)),
    }
    let size = std::fs::metadata(file).map_err(fail)?.len();
    if size <= 16 * 1024 * 1024 {
        let mapped = unsafe { memmap2::MmapOptions::new().map(&File::open(file).map_err(fail)?) }
            .map_err(fail)?;
        store
            .put_opts(
                path,
                object_store::PutPayload::from(bytes::Bytes::from_owner(mapped)),
                object_store::PutMode::Create.into(),
            )
            .await
            .map_err(|error| match error {
                object_store::Error::AlreadyExists { .. } => {
                    AppError::Conflict("Immutable object already exists".into())
                }
                other => fail(other),
            })?;
        return Ok(());
    }
    let mut upload = store.put_multipart(path).await.map_err(fail)?;
    let result = async {
        use tokio::io::AsyncReadExt;
        let mut source = tokio::fs::File::open(file).await.map_err(fail)?;
        let mut parts = 0;
        loop {
            let mut data = vec![0u8; 64 * 1024 * 1024];
            let mut used = 0;
            while used < data.len() {
                let n = source.read(&mut data[used..]).await.map_err(fail)?;
                if n == 0 {
                    break;
                }
                used += n;
            }
            if used == 0 {
                break;
            }
            parts += 1;
            if parts > 10000 {
                return Err(fail(
                    "Object exceeds multipart part limit; use a smaller export range",
                ));
            }
            data.truncate(used);
            upload.put_part(data.into()).await.map_err(fail)?;
        }
        upload.complete().await.map_err(fail)?;
        Ok::<_, AppError>(())
    }
    .await;
    if result.is_err() {
        let _ = upload.abort().await;
    }
    result
}
