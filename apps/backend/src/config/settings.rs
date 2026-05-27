/// Application settings loaded from environment variables.
pub struct Settings {
    pub database_url: String,
    pub database_max_connections: u32,
    pub jwt_secret: String,
    pub jwt_access_expiration: String,
    pub bcrypt_salt_rounds: u32,
    pub port: u16,
    pub cafe_timezone: String,
    pub upload_max_size_bytes: u64,
    pub zeptomail_token: Option<String>,
    pub r2_account_id: Option<String>,
    pub r2_access_key: Option<String>,
    pub r2_secret_key: Option<String>,
    pub r2_endpoint: Option<String>,
    pub r2_bucket: Option<String>,
}

impl Settings {
    pub fn from_env() -> Self {
        let jwt_secret: String =
            std::env::var("JWT_SECRET").expect("JWT_SECRET must be set and >= 32 chars");
        let jwt_secret = jwt_secret.trim().to_string();
        if jwt_secret.len() < 32 {
            panic!("JWT_SECRET must be at least 32 characters");
        }

        let r2_account_id = env_first(&["R2_ACCOUNT_ID"]);
        let r2_access_key = env_first(&["R2_ACCESS_KEY", "R2_ACCESS_KEY_ID"]);
        let r2_secret_key = env_first(&["R2_SECRET_KEY", "R2_SECRET_ACCESS_KEY"]);
        let r2_bucket = env_first(&["R2_BUCKET", "R2_BUCKET_NAME"]);
        let r2_endpoint = env_first(&["R2_ENDPOINT"]).or_else(|| {
            r2_account_id
                .as_ref()
                .map(|id| format!("https://{id}.r2.cloudflarestorage.com"))
        });

        Self {
            database_url: resolve_database_url(),
            database_max_connections: std::env::var("DATABASE_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .expect("DATABASE_MAX_CONNECTIONS must be a number"),
            jwt_secret,
            jwt_access_expiration: std::env::var("JWT_ACCESS_EXPIRATION")
                .unwrap_or_else(|_| "15m".to_string()),
            bcrypt_salt_rounds: std::env::var("BCRYPT_SALT_ROUNDS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("PORT must be a number"),
            cafe_timezone: std::env::var("CAFE_TZ").unwrap_or_else(|_| "Asia/Kolkata".to_string()),
            upload_max_size_bytes: 25 * 1024 * 1024,
            zeptomail_token: std::env::var("ZEPTOMAIL_TOKEN").ok(),
            r2_account_id,
            r2_access_key,
            r2_secret_key,
            r2_endpoint,
            r2_bucket,
        }
    }

    pub fn is_production(&self) -> bool {
        is_production_env()
    }

    pub fn r2_configured(&self) -> bool {
        self.r2_access_key.is_some()
            && self.r2_secret_key.is_some()
            && self.r2_bucket.is_some()
            && self.r2_endpoint.is_some()
    }
}

fn env_first(keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
}

pub fn is_production_env() -> bool {
    ["NODE_ENV", "RUST_ENV", "ENVIRONMENT"]
        .into_iter()
        .any(|key| std::env::var(key).is_ok_and(|v| v == "production"))
}

fn resolve_database_url() -> String {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.is_empty() {
            return url;
        }
    }

    let host = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
    let username = std::env::var("DB_USERNAME").unwrap_or_else(|_| "postgres".to_string());
    let password = std::env::var("DB_PASSWORD").unwrap_or_else(|_| "postgres".to_string());
    let database = std::env::var("DB_DATABASE").unwrap_or_else(|_| "gamezone_dev".to_string());

    format!("postgres://{username}:{password}@{host}:{port}/{database}")
}
