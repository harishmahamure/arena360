//! AES-256-GCM authenticates both the compressed bytes and their full object key.
//! A nonce is generated once, then the encoded artifact is persisted for retries.
use crate::error::AppError;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
    Aes256Gcm, Nonce,
};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;
fn error() -> AppError {
    AppError::Internal("Backup encryption or authentication failed".into())
}

/// Streaming snapshot envelope. Every 64-KiB compressed chunk authenticates its
/// position, and an authenticated empty final chunk detects truncated streams.
pub fn encode_file(
    key: &[u8; 32],
    object_key: &str,
    source: impl Read,
    mut output: impl Write,
) -> Result<(), AppError> {
    use aes_gcm::aead::rand_core::RngCore;
    let mut prefix = [0u8; 8];
    OsRng.fill_bytes(&mut prefix);
    output
        .write_all(b"A360BKP2")
        .and_then(|_| output.write_all(&prefix))
        .map_err(|_| error())?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| error())?;
    let mut encoder = zstd::stream::read::Encoder::new(source, 3).map_err(|_| error())?;
    let mut buffer = [0u8; 65536];
    let mut index = 0u32;
    loop {
        let count = encoder.read(&mut buffer).map_err(|_| error())?;
        let mut nonce = [0u8; 12];
        nonce[..8].copy_from_slice(&prefix);
        nonce[8..].copy_from_slice(&index.to_be_bytes());
        let aad = format!("{object_key}/{index}/{}", count == 0);
        let chunk = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &buffer[..count],
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| error())?;
        output
            .write_all(&(chunk.len() as u32).to_be_bytes())
            .and_then(|_| output.write_all(&chunk))
            .map_err(|_| error())?;
        if count == 0 {
            return Ok(());
        }
        index = index.checked_add(1).ok_or_else(error)?;
    }
}
struct Decrypted<R> {
    input: R,
    cipher: Aes256Gcm,
    prefix: [u8; 8],
    object: String,
    index: u32,
    buffer: std::io::Cursor<Vec<u8>>,
    done: bool,
}
impl<R: Read> Read for Decrypted<R> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let count = self.buffer.read(output)?;
        if count > 0 || self.done {
            return Ok(count);
        }
        let invalid = || {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Snapshot chunk authentication failed",
            )
        };
        let mut length = [0u8; 4];
        self.input.read_exact(&mut length)?;
        let length = u32::from_be_bytes(length) as usize;
        if !(16..=65552).contains(&length) {
            return Err(invalid());
        }
        let mut bytes = vec![0; length];
        self.input.read_exact(&mut bytes)?;
        let mut nonce = [0u8; 12];
        nonce[..8].copy_from_slice(&self.prefix);
        nonce[8..].copy_from_slice(&self.index.to_be_bytes());
        let final_chunk = length == 16;
        let aad = format!("{}/{}/{}", self.object, self.index, final_chunk);
        let decoded = self
            .cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &bytes,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| invalid())?;
        if final_chunk {
            let mut extra = [0];
            if self.input.read(&mut extra)? != 0 {
                return Err(invalid());
            }
            self.done = true;
            return Ok(0);
        }
        self.index = self.index.checked_add(1).ok_or_else(invalid)?;
        self.buffer = std::io::Cursor::new(decoded);
        self.buffer.read(output)
    }
}
pub fn decode_file(
    key: &[u8; 32],
    object_key: &str,
    mut source: impl Read,
    mut output: impl Write,
    limit: u64,
) -> Result<u64, AppError> {
    let mut header = [0u8; 16];
    source.read_exact(&mut header).map_err(|_| error())?;
    if &header[..8] != b"A360BKP2" {
        return Err(error());
    }
    let decrypted = Decrypted {
        input: source,
        cipher: Aes256Gcm::new_from_slice(key).map_err(|_| error())?,
        prefix: header[8..].try_into().unwrap(),
        object: object_key.into(),
        index: 0,
        buffer: std::io::Cursor::new(vec![]),
        done: false,
    };
    let mut decoder = zstd::stream::read::Decoder::new(decrypted).map_err(|_| error())?;
    let size = std::io::copy(
        &mut decoder
            .by_ref()
            .take(limit.checked_add(1).ok_or_else(error)?),
        &mut output,
    )
    .map_err(|_| error())?;
    if size > limit {
        return Err(error());
    }
    // Drain the authenticated final envelope even when zstd has reached its own
    // end marker. An omitted final chunk must never look like a valid snapshot.
    let mut inner = decoder.finish();
    let mut trailing = [0u8; 1];
    if inner.read(&mut trailing).map_err(|_| error())? != 0 {
        return Err(error());
    }
    Ok(size)
}
pub fn encode(key: &[u8; 32], object_key: &str, source: &[u8]) -> Result<Vec<u8>, AppError> {
    let compressed = zstd::stream::encode_all(source, 3).map_err(|_| error())?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| error())?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: &compressed,
                aad: object_key.as_bytes(),
            },
        )
        .map_err(|_| error())?;
    let mut bytes = b"A360BKP1".to_vec();
    bytes.extend_from_slice(&nonce);
    bytes.extend(ciphertext);
    Ok(bytes)
}
pub fn decode(
    key: &[u8; 32],
    object_key: &str,
    bytes: &[u8],
    limit: usize,
) -> Result<Vec<u8>, AppError> {
    if bytes.len() < 36 || &bytes[..8] != b"A360BKP1" {
        return Err(error());
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| error())?;
    let compressed = cipher
        .decrypt(
            Nonce::from_slice(&bytes[8..20]),
            Payload {
                msg: &bytes[20..],
                aad: object_key.as_bytes(),
            },
        )
        .map_err(|_| error())?;
    let decoder = zstd::stream::read::Decoder::new(compressed.as_slice()).map_err(|_| error())?;
    let mut output = Vec::new();
    decoder
        .take(limit as u64 + 1)
        .read_to_end(&mut output)
        .map_err(|_| error())?;
    if output.len() > limit {
        return Err(error());
    }
    Ok(output)
}
/// Keys are provisioned in a separate, durable secret mount, never in tenant
/// snapshots or the PostgreSQL manifest. Missing keys fail closed. Deployment
/// must keep the mount recoverable independently of a cell's local NVMe.
#[derive(Clone)]
pub struct TenantKeys {
    root: PathBuf,
}
impl TenantKeys {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn path(&self, tenant: Uuid) -> PathBuf {
        self.root.join(format!("{tenant}.key"))
    }
    pub fn read(&self, tenant: Uuid) -> Result<[u8; 32], AppError> {
        let bytes = std::fs::read(self.path(tenant))
            .map_err(|_| AppError::Internal("Tenant backup key unavailable".into()))?;
        bytes.try_into().map_err(|_| error())
    }
    pub fn destroy(&self, tenant: Uuid) -> Result<(), AppError> {
        match std::fs::remove_file(self.path(tenant)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(error()),
        }
        std::fs::File::open(Path::new(&self.root))
            .and_then(|f| f.sync_all())
            .map_err(|_| error())
    }
}
