/// Application settings loaded from environment variables.
pub struct Settings {
    pub database_url: String,
    pub database_max_connections: u32,
    pub jwt_secret: String,
    pub jwt_access_expiration: String,
    pub jwt_player_expiration: String,
    pub jwt_device_expiration: String,
    pub bcrypt_salt_rounds: u32,
    pub port: u16,
    pub cafe_timezone: String,
    pub zeptomail_token: Option<String>,
}

impl Settings {
    pub fn from_env() -> Self {
        let jwt_secret: String =
            std::env::var("JWT_SECRET").expect("JWT_SECRET must be set and >= 32 chars");
        let jwt_secret = jwt_secret.trim().to_string();
        if jwt_secret.len() < 32 {
            panic!("JWT_SECRET must be at least 32 characters");
        }

        Self {
            database_url: resolve_database_url(),
            database_max_connections: std::env::var("DATABASE_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .expect("DATABASE_MAX_CONNECTIONS must be a number"),
            jwt_secret,
            jwt_access_expiration: std::env::var("JWT_ACCESS_EXPIRATION")
                .unwrap_or_else(|_| "15m".to_string()),
            jwt_player_expiration: std::env::var("JWT_PLAYER_EXPIRATION")
                .unwrap_or_else(|_| "24h".to_string()),
            jwt_device_expiration: std::env::var("JWT_DEVICE_EXPIRATION")
                .unwrap_or_else(|_| "365d".to_string()),
            bcrypt_salt_rounds: std::env::var("BCRYPT_SALT_ROUNDS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("PORT must be a number"),
            cafe_timezone: std::env::var("CAFE_TZ").unwrap_or_else(|_| "Asia/Kolkata".to_string()),
            zeptomail_token: std::env::var("ZEPTOMAIL_TOKEN").ok(),
        }
    }

    pub fn is_production(&self) -> bool {
        is_production_env()
    }
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
