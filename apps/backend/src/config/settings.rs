/// Application settings loaded from environment variables.
pub struct Settings {
    pub database_url: String,
    pub database_listener_url: String,
    pub database_min_connections: u32,
    pub database_max_connections: u32,
    pub database_acquire_timeout_seconds: u64,
    pub database_idle_timeout_seconds: u64,
    pub database_max_lifetime_seconds: u64,
    pub redis_url: Option<String>,
    pub jwt_secret: String,
    pub jwt_access_expiration: String,
    pub jwt_player_expiration: String,
    pub jwt_device_expiration: String,
    pub bcrypt_salt_rounds: u32,
    pub port: u16,
    pub cafe_timezone: String,
    pub zeptomail_token: Option<String>,
    pub legacy_rest_enabled: bool,
    pub trusted_proxy_cidrs: Vec<ipnet::IpNet>,
    pub max_concurrent_requests: usize,
}

impl Settings {
    pub fn from_env() -> Self {
        let jwt_secret: String =
            std::env::var("JWT_SECRET").expect("JWT_SECRET must be set and >= 32 chars");
        let jwt_secret = jwt_secret.trim().to_string();
        if jwt_secret.len() < 32 {
            panic!("JWT_SECRET must be at least 32 characters");
        }

        let database_url = resolve_database_url();
        Self {
            database_listener_url: std::env::var("DATABASE_LISTENER_URL")
                .ok()
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| database_url.clone()),
            database_url,
            database_min_connections: env_parse("DATABASE_MIN_CONNECTIONS", 2),
            database_max_connections: std::env::var("DATABASE_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .expect("DATABASE_MAX_CONNECTIONS must be a number"),
            database_acquire_timeout_seconds: env_parse("DATABASE_ACQUIRE_TIMEOUT_SECONDS", 2),
            database_idle_timeout_seconds: env_parse("DATABASE_IDLE_TIMEOUT_SECONDS", 600),
            database_max_lifetime_seconds: env_parse("DATABASE_MAX_LIFETIME_SECONDS", 1800),
            redis_url: std::env::var("REDIS_URL").ok().filter(|v| !v.is_empty()),
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
            legacy_rest_enabled: env_bool("LEGACY_REST_ENABLED", false),
            trusted_proxy_cidrs: std::env::var("TRUSTED_PROXY_CIDRS")
                .unwrap_or_default()
                .split(',')
                .filter_map(|value| {
                    let value = value.trim();
                    (!value.is_empty()).then(|| {
                        value.parse().unwrap_or_else(|_| {
                            panic!("invalid CIDR in TRUSTED_PROXY_CIDRS: {value}")
                        })
                    })
                })
                .collect(),
            max_concurrent_requests: env_parse("MAX_CONCURRENT_REQUESTS", 256),
        }
    }

    pub fn is_production(&self) -> bool {
        is_production_env()
    }
}

fn env_parse<T>(name: &str, default: T) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    std::env::var(name)
        .map(|value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("{name} must be a number"))
        })
        .unwrap_or(default)
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
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
