//! SQLite WAL capture. Caller holds the tenant writer mutex and BEGIN IMMEDIATE:
//! no writer or checkpoint can reset the WAL during the copy. The WAL-index's
//! committed frame boundary excludes valid-looking frames left by rollbacks.
//! Format reference: https://sqlite.org/walformat.html (Unix/Windows default VFS).
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

fn fail(message: impl Into<String>) -> AppError {
    AppError::Internal(message.into())
}
fn io(error: std::io::Error) -> AppError {
    fail(format!("Replication filesystem: {error}"))
}
fn be(data: &[u8]) -> u32 {
    u32::from_be_bytes(data[..4].try_into().unwrap())
}
fn native(data: &[u8]) -> u32 {
    u32::from_ne_bytes(data[..4].try_into().unwrap())
}

pub fn checksum(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Persist bytes before exposing a name, and persist the directory entry. Hard
/// linking implements create-only semantics even on platforms where rename replaces.
pub fn durable_create(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = path
        .parent()
        .ok_or_else(|| fail("Missing spool directory"))?;
    fs::create_dir_all(parent).map_err(io)?;
    let temporary = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(io)?;
        file.write_all(bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
        match fs::hard_link(&temporary, path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if fs::read(path).map_err(io)? != bytes {
                    return Err(fail("Immutable spool collision"));
                }
            }
            Err(e) => return Err(io(e)),
        }
        File::open(parent).map_err(io)?.sync_all().map_err(io)?;
        Ok(())
    })();
    let _ = fs::remove_file(temporary);
    result
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capture {
    pub version: u32,
    pub capture_number: u64,
    pub ownership_generation: i64,
    pub captured_at: String,
    pub salt: String,
    pub page_size: u32,
    pub frames: u32,
    pub checksum: String,
}

/// A capture contains the complete committed prefix of one WAL incarnation.
/// Keeping its header and checksum chain makes each capture independently
/// verifiable. Repeated prefixes after a reader-blocked checkpoint are safe;
/// manifest ordering determines which database page image is applied last.
pub fn capture(database: &Path, ownership_generation: i64) -> Result<Option<PathBuf>, AppError> {
    let wal = PathBuf::from(format!("{}-wal", database.display()));
    let shm = PathBuf::from(format!("{}-shm", database.display()));
    if !wal.exists() || fs::metadata(&wal).map_err(io)?.len() == 0 {
        return Ok(None);
    }
    let mut index = [0u8; 96];
    File::open(shm)
        .map_err(io)?
        .read_exact(&mut index)
        .map_err(io)?;
    if index[..48] != index[48..] || index[12] != 1 || native(&index[..4]) != 3_007_000 {
        return Err(fail("Unstable or unsupported WAL index"));
    }
    let frames = native(&index[16..20]);
    if frames == 0 {
        return Ok(None);
    }
    let mut file = File::open(wal).map_err(io)?;
    let mut header = [0u8; 32];
    file.read_exact(&mut header).map_err(io)?;
    let page_size = be(&header[8..12]);
    if !(512..=65536).contains(&page_size) || !page_size.is_power_of_two() {
        return Err(fail("Invalid WAL page size"));
    }
    if header[16..24] != index[32..40] {
        return Err(fail("WAL/index lineage mismatch"));
    }
    let length = 32u64 + (page_size as u64 + 24) * frames as u64;
    // Bounded batches: an excessively large WAL remains uncheckpointed and is
    // surfaced as a failure, never silently dropped or allocated without limit.
    if length > 256 * 1024 * 1024 {
        return Err(fail("WAL capture exceeds 256 MiB batch limit"));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    bytes.extend_from_slice(&header);
    file.take(length - 32).read_to_end(&mut bytes).map_err(io)?;
    validate(&bytes, frames)?;
    let last = bytes.len() - page_size as usize - 24;
    if be(&bytes[last + 4..last + 8]) != native(&index[20..24])
        || be(&bytes[last + 16..last + 20]) != native(&index[24..28])
        || be(&bytes[last + 20..last + 24]) != native(&index[28..32])
    {
        return Err(fail("Committed WAL boundary disagrees with index"));
    }
    let digest = checksum(&bytes);
    let directory = database
        .parent()
        .ok_or_else(|| fail("Missing tenant directory"))?
        .join("replication/spool");
    let path = directory.join(format!("{ownership_generation}-{digest}.wal"));
    let metadata_path = path.with_extension("json");
    // Metadata must survive before WAL is eligible for checkpoint. On a retry,
    // preserve its original capture time rather than claiming a fresh RPO.
    if !metadata_path.exists() {
        let sequence_path = directory.parent().unwrap().join("capture-sequence");
        let previous = match fs::read_to_string(&sequence_path) {
            Ok(value) => value
                .parse::<u64>()
                .map_err(|_| fail("Invalid capture sequence"))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
            Err(e) => return Err(io(e)),
        };
        let capture_number = previous
            .checked_add(1)
            .ok_or_else(|| fail("Capture sequence exhausted"))?;
        fs::create_dir_all(directory.parent().unwrap()).map_err(io)?;
        let temporary = sequence_path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        durable_create(&temporary, capture_number.to_string().as_bytes())?;
        fs::rename(&temporary, &sequence_path).map_err(io)?;
        File::open(directory.parent().unwrap())
            .map_err(io)?
            .sync_all()
            .map_err(io)?;
        let metadata = Capture {
            version: 1,
            capture_number,
            ownership_generation,
            captured_at: crate::time::format_sqlite_timestamp(&chrono::Utc::now())
                .map_err(|e| fail(e.to_string()))?,
            salt: hex::encode(&header[16..24]),
            page_size,
            frames,
            checksum: digest,
        };
        durable_create(
            &metadata_path,
            &serde_json::to_vec(&metadata).map_err(|e| fail(e.to_string()))?,
        )?;
    }
    durable_create(&path, &bytes)?;
    Ok(Some(path))
}

/// Validate every frame's SQLite rolling checksum, salt and final commit marker.
pub fn validate(bytes: &[u8], frames: u32) -> Result<(), AppError> {
    if bytes.len() < 32 || frames == 0 {
        return Err(fail("Empty/truncated WAL"));
    }
    let magic = be(bytes);
    let big = match magic {
        0x377f0682 => false,
        0x377f0683 => true,
        _ => return Err(fail("Invalid WAL magic")),
    };
    if be(&bytes[4..8]) != 3_007_000 {
        return Err(fail("Unsupported WAL version"));
    }
    let page = be(&bytes[8..12]) as usize;
    if !(512..=65536).contains(&page)
        || !page.is_power_of_two()
        || bytes.len() != 32 + (page + 24) * frames as usize
    {
        return Err(fail("Invalid WAL length/page size"));
    }
    let update = |data: &[u8], mut pair: (u32, u32)| {
        for words in data.chunks_exact(8) {
            let read = |w: &[u8]| {
                if big {
                    be(w)
                } else {
                    u32::from_le_bytes(w[..4].try_into().unwrap())
                }
            };
            pair.0 = pair.0.wrapping_add(read(words)).wrapping_add(pair.1);
            pair.1 = pair.1.wrapping_add(read(&words[4..])).wrapping_add(pair.0);
        }
        pair
    };
    let mut pair = update(&bytes[..24], (0, 0));
    if pair != (be(&bytes[24..28]), be(&bytes[28..32])) {
        return Err(fail("WAL header checksum mismatch"));
    }
    for frame in bytes[32..].chunks_exact(page + 24) {
        if be(frame) == 0 || frame[8..16] != bytes[16..24] {
            return Err(fail("WAL frame lineage/page mismatch"));
        }
        pair = update(&frame[..8], pair);
        pair = update(&frame[24..], pair);
        if pair != (be(&frame[16..20]), be(&frame[20..24])) {
            return Err(fail("WAL frame checksum mismatch"));
        }
    }
    let last = bytes.len() - page - 24;
    if be(&bytes[last + 4..last + 8]) == 0 {
        return Err(fail("WAL capture does not end at a committed transaction"));
    }
    Ok(())
}
