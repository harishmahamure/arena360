//! S3-compatible object storage presigning (DRAFT-0022).
//!
//! The backend never proxies asset bytes. It issues short-lived SigV4 presigned
//! `PUT` URLs so the admin uploads directly to the bucket (S3 / MinIO), then
//! persists the returned public URL on the game record.

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::AppError;

type HmacSha256 = Hmac<Sha256>;

/// Object-storage configuration resolved from the environment. When unset the
/// presign endpoint returns a clear "not configured" error rather than panicking.
#[derive(Clone)]
pub struct StorageConfig {
    /// Base endpoint, e.g. `https://s3.eu-central-1.amazonaws.com` or
    /// `http://localhost:9000` for MinIO.
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    /// Public base used to build the stored URL. Defaults to `{endpoint}/{bucket}`.
    pub public_base_url: String,
    pub presign_expiry_secs: u64,
}

impl StorageConfig {
    pub fn from_env() -> Option<Self> {
        let endpoint = std::env::var("STORAGE_ENDPOINT").ok()?;
        let bucket = std::env::var("STORAGE_BUCKET").ok()?;
        let access_key = std::env::var("STORAGE_ACCESS_KEY").ok()?;
        let secret_key = std::env::var("STORAGE_SECRET_KEY").ok()?;
        let region = std::env::var("STORAGE_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let endpoint = endpoint.trim_end_matches('/').to_string();
        let public_base_url = std::env::var("STORAGE_PUBLIC_URL")
            .unwrap_or_else(|_| format!("{endpoint}/{bucket}"))
            .trim_end_matches('/')
            .to_string();
        let presign_expiry_secs = std::env::var("STORAGE_PRESIGN_EXPIRY_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(900);
        Some(Self {
            endpoint,
            region,
            bucket,
            access_key,
            secret_key,
            public_base_url,
            presign_expiry_secs,
        })
    }
}

/// Result of issuing a presigned upload.
pub struct PresignedUpload {
    pub upload_url: String,
    pub public_url: String,
    pub key: String,
}

#[derive(Clone)]
pub struct StorageService {
    config: Option<StorageConfig>,
}

impl StorageService {
    pub fn new(config: Option<StorageConfig>) -> Self {
        Self { config }
    }

    /// Build an object key namespaced under `games/<uuid>/<sanitized-filename>`.
    pub fn game_asset_key(filename: &str) -> String {
        let safe: String = filename
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let safe = if safe.is_empty() {
            "asset".to_string()
        } else {
            safe
        };
        format!("games/{}/{}", Uuid::new_v4(), safe)
    }

    /// Issue a presigned PUT URL for the given object key. `content_type` is
    /// accepted for API symmetry; only `host` is signed so the client may set any
    /// content type on upload.
    pub fn presign_put(
        &self,
        key: &str,
        _content_type: Option<&str>,
    ) -> Result<PresignedUpload, AppError> {
        let cfg = self
            .config
            .as_ref()
            .ok_or_else(|| AppError::Internal("Object storage is not configured".to_string()))?;

        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let datestamp = now.format("%Y%m%d").to_string();

        let host = host_from_endpoint(&cfg.endpoint);
        let canonical_uri = format!(
            "/{}/{}",
            uri_encode(&cfg.bucket, false),
            uri_encode(key, false)
        );
        let scope = format!("{datestamp}/{}/s3/aws4_request", cfg.region);
        let credential = format!("{}/{scope}", cfg.access_key);

        // Sorted canonical query string.
        let mut params: Vec<(String, String)> = vec![
            (
                "X-Amz-Algorithm".to_string(),
                "AWS4-HMAC-SHA256".to_string(),
            ),
            ("X-Amz-Credential".to_string(), credential),
            ("X-Amz-Date".to_string(), amz_date.clone()),
            (
                "X-Amz-Expires".to_string(),
                cfg.presign_expiry_secs.to_string(),
            ),
            ("X-Amz-SignedHeaders".to_string(), "host".to_string()),
        ];
        params.sort_by(|a, b| a.0.cmp(&b.0));
        let canonical_query = params
            .iter()
            .map(|(k, v)| format!("{}={}", uri_encode(k, true), uri_encode(v, true)))
            .collect::<Vec<_>>()
            .join("&");

        let canonical_headers = format!("host:{host}\n");
        let signed_headers = "host";
        let payload_hash = "UNSIGNED-PAYLOAD";

        let canonical_request = format!(
            "PUT\n{canonical_uri}\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );

        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
            hex::encode(sha256(canonical_request.as_bytes()))
        );

        let signing_key = signing_key(&cfg.secret_key, &datestamp, &cfg.region, "s3");
        let signature = hex::encode(hmac(&signing_key, string_to_sign.as_bytes()));

        let upload_url = format!(
            "{}{canonical_uri}?{canonical_query}&X-Amz-Signature={signature}",
            cfg.endpoint
        );
        let public_url = format!("{}/{}", cfg.public_base_url, key);

        Ok(PresignedUpload {
            upload_url,
            public_url,
            key: key.to_string(),
        })
    }
}

fn host_from_endpoint(endpoint: &str) -> String {
    endpoint
        .strip_prefix("https://")
        .or_else(|| endpoint.strip_prefix("http://"))
        .unwrap_or(endpoint)
        .trim_end_matches('/')
        .to_string()
}

fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn signing_key(secret: &str, datestamp: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac(format!("AWS4{secret}").as_bytes(), datestamp.as_bytes());
    let k_region = hmac(&k_date, region.as_bytes());
    let k_service = hmac(&k_region, service.as_bytes());
    hmac(&k_service, b"aws4_request")
}

/// RFC3986 percent-encoding. When `encode_slash` is false, `/` is preserved
/// (used for the canonical URI path).
fn uri_encode(input: &str, encode_slash: bool) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b'/' if !encode_slash => out.push('/'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_encode_preserves_unreserved() {
        assert_eq!(uri_encode("aB9-_.~", true), "aB9-_.~");
    }

    #[test]
    fn uri_encode_path_keeps_slash() {
        assert_eq!(uri_encode("games/x/y.png", false), "games/x/y.png");
        assert_eq!(uri_encode("games/x/y.png", true), "games%2Fx%2Fy.png");
    }

    #[test]
    fn presign_errors_when_unconfigured() {
        let svc = StorageService::new(None);
        assert!(svc.presign_put("games/a/b.png", None).is_err());
    }

    #[test]
    fn presign_builds_expected_shape() {
        let svc = StorageService::new(Some(StorageConfig {
            endpoint: "http://localhost:9000".to_string(),
            region: "us-east-1".to_string(),
            bucket: "assets".to_string(),
            access_key: "AKIA".to_string(),
            secret_key: "secret".to_string(),
            public_base_url: "http://localhost:9000/assets".to_string(),
            presign_expiry_secs: 900,
        }));
        let out = svc
            .presign_put("games/abc/cover.png", Some("image/png"))
            .unwrap();
        assert!(out
            .upload_url
            .starts_with("http://localhost:9000/assets/games/abc/cover.png?"));
        assert!(out.upload_url.contains("X-Amz-Signature="));
        assert!(out.upload_url.contains("X-Amz-Algorithm=AWS4-HMAC-SHA256"));
        assert_eq!(
            out.public_url,
            "http://localhost:9000/assets/games/abc/cover.png"
        );
    }

    #[test]
    fn game_asset_key_sanitizes() {
        let key = StorageService::game_asset_key("My Cover!.png");
        assert!(key.starts_with("games/"));
        assert!(key.ends_with("/My-Cover-.png"));
    }
}
