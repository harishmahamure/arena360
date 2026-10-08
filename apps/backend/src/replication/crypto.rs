//! AES-256-GCM authenticates both the compressed bytes and their full object key.
//! A nonce is generated once, then the encoded artifact is persisted for retries.
use crate::error::AppError;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
    Aes256Gcm, Nonce,
};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;
fn error() -> AppError {
    AppError::Internal("Backup encryption or authentication failed".into())
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
