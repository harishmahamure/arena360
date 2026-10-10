//! One upload contains a contiguous ordered set of durable WAL captures.
use super::wal::{self, Capture};
use crate::{error::AppError, tenancy::TenantDb};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
fn fail(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("WAL batch: {e}"))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Batch {
    pub capture: Capture,
    pub files: Vec<PathBuf>,
    pub raw: PathBuf,
}
pub fn descriptor(db: &TenantDb) -> PathBuf {
    db.path().parent().unwrap().join(format!(
        "replication/batch-{}.json",
        db.ownership_generation()
    ))
}
pub fn prepare(db: &TenantDb, captures: &[(PathBuf, Capture)]) -> Result<Option<Batch>, AppError> {
    let descriptor = descriptor(db);
    if descriptor.exists() {
        let batch: Batch =
            serde_json::from_slice(&std::fs::read(&descriptor).map_err(fail)?).map_err(fail)?;
        if batch.capture.ownership_generation != db.ownership_generation() {
            return Err(fail("Batch ownership changed"));
        }
        return Ok(Some(batch));
    }
    if captures.is_empty() {
        return Ok(None);
    }
    let mut bytes = b"A360WAL2".to_vec();
    let mut files = vec![];
    let mut last = None;
    for (path, capture) in captures.iter().take(4096) {
        if let Some(previous) = last {
            if previous + 1 != capture.capture_number {
                break;
            }
        }
        let source = std::fs::read(path).map_err(fail)?;
        if wal::checksum(&source) != capture.checksum {
            return Err(fail("Capture checksum mismatch"));
        }
        wal::validate(&source, capture.frames)?;
        if !files.is_empty() && bytes.len() + source.len() > 8 * 1024 * 1024 {
            break;
        }
        let metadata = serde_json::to_vec(capture).map_err(fail)?;
        bytes.extend_from_slice(&(metadata.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&metadata);
        bytes.extend_from_slice(&(source.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&source);
        files.push(path.clone());
        last = Some(capture.capture_number);
    }
    let mut capture = captures[0].1.clone();
    capture.version = 2;
    capture.last_capture_number = last;
    capture.last_captured_at = Some(captures[files.len() - 1].1.captured_at.clone());
    capture.checksum = wal::checksum(&bytes);
    let raw = descriptor
        .parent()
        .unwrap()
        .join(format!("batches/{}.raw", capture.checksum));
    wal::durable_create(&raw, &bytes)?;
    let batch = Batch {
        capture,
        files,
        raw,
    };
    wal::durable_create(&descriptor, &serde_json::to_vec(&batch).map_err(fail)?)?;
    Ok(Some(batch))
}
pub fn decode(bytes: &[u8]) -> Result<Vec<(Capture, Vec<u8>)>, AppError> {
    if !bytes.starts_with(b"A360WAL2") {
        return Err(fail("Unsupported WAL batch format"));
    }
    let mut at = 8usize;
    let mut records = vec![];
    let mut previous = None;
    let read = |at: &mut usize| -> Result<&[u8], AppError> {
        let end = at.checked_add(4).ok_or_else(|| fail("Length overflow"))?;
        let size = u32::from_be_bytes(
            bytes
                .get(*at..end)
                .ok_or_else(|| fail("Truncated batch length"))?
                .try_into()
                .unwrap(),
        ) as usize;
        *at = end;
        let end = at
            .checked_add(size)
            .ok_or_else(|| fail("Length overflow"))?;
        let value = bytes
            .get(*at..end)
            .ok_or_else(|| fail("Truncated batch record"))?;
        *at = end;
        Ok(value)
    };
    while at < bytes.len() {
        if records.len() >= 4096 {
            return Err(fail("Too many batch records"));
        }
        let capture: Capture = serde_json::from_slice(read(&mut at)?).map_err(fail)?;
        let source = read(&mut at)?;
        if capture.version != 1
            || capture.last_capture_number.is_some()
            || previous.is_some_and(|p: u64| p.checked_add(1) != Some(capture.capture_number))
        {
            return Err(fail("Noncontiguous or nested WAL batch"));
        }
        if wal::checksum(source) != capture.checksum {
            return Err(fail("Batch record checksum mismatch"));
        }
        wal::validate(source, capture.frames)?;
        previous = Some(capture.capture_number);
        records.push((capture, source.to_vec()));
    }
    if records.is_empty() {
        return Err(fail("Empty batch"));
    }
    Ok(records)
}
pub fn read(batch: &Batch) -> Result<Vec<u8>, AppError> {
    let bytes = std::fs::read(&batch.raw).map_err(fail)?;
    if wal::checksum(&bytes) != batch.capture.checksum {
        return Err(fail("Batch spool checksum mismatch"));
    }
    let records = decode(&bytes)?;
    if records.first().unwrap().0.capture_number != batch.capture.capture_number
        || records.last().unwrap().0.capture_number != batch.capture.range_end()
        || records
            .iter()
            .any(|(c, _)| c.ownership_generation != batch.capture.ownership_generation)
    {
        return Err(fail("Batch metadata range mismatch"));
    }
    Ok(bytes)
}
pub fn remove(db: &TenantDb, batch: &Batch, encoded: &Path) -> Result<(), AppError> {
    // Record local acknowledgement before removing source captures. A pinned
    // reader can keep an already uploaded WAL prefix on disk indefinitely.
    let ack = db
        .path()
        .parent()
        .unwrap()
        .join("replication/acknowledged-capture");
    let temp = ack.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    wal::durable_create(&temp, batch.capture.range_end().to_string().as_bytes())?;
    std::fs::rename(&temp, &ack).map_err(fail)?;
    std::fs::File::open(ack.parent().unwrap())
        .and_then(|f| f.sync_all())
        .map_err(fail)?;
    for source in &batch.files {
        for path in [source.clone(), source.with_extension("json")] {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(fail(e)),
            }
        }
    }
    for path in [encoded.to_owned(), batch.raw.clone(), descriptor(db)] {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(fail(e)),
        }
    }
    std::fs::File::open(ack.parent().unwrap())
        .and_then(|f| f.sync_all())
        .map_err(fail)?;
    Ok(())
}
