//! Typed Redis cache key builders.
//!
//! All keys use colon-separated namespaces to support prefix invalidation.

pub mod ttl {
    use std::time::Duration;

    pub const LOOKUP: Duration = Duration::from_secs(24 * 60 * 60);
    pub const SESSION: Duration = Duration::from_secs(60 * 60);
    pub const SESSION_ENRICHED: Duration = Duration::from_secs(30 * 60);
    pub const AUTH: Duration = Duration::from_secs(12 * 60 * 60);
    pub const AGGREGATE: Duration = Duration::from_secs(2 * 60 * 60);
    pub const OTP_RATE_LIMIT: Duration = Duration::from_secs(60);
    pub const NOTIFICATIONS: Duration = Duration::from_secs(5 * 60);
}

pub const INVALIDATION_CHANNEL: &str = "cache:invalidate";

pub fn plan(id: &uuid::Uuid) -> String {
    format!("plans:{id}")
}

pub fn plans_active() -> &'static str {
    "plans:active"
}

pub fn plans_list(filter_hash: &str) -> String {
    format!("plans:list:{filter_hash}")
}

pub fn product(id: &uuid::Uuid) -> String {
    format!("products:{id}")
}

pub fn products_list(filter_hash: &str) -> String {
    format!("products:list:{filter_hash}")
}

pub fn units_all() -> &'static str {
    "units:all"
}

pub fn unit(id: &uuid::Uuid) -> String {
    format!("units:{id}")
}

pub fn config(key: &str) -> String {
    format!("configs:{key}")
}

pub fn configs_all() -> &'static str {
    "configs:all"
}

pub fn expense_categories_tree() -> &'static str {
    "expense_categories:tree"
}

pub fn expense_category(id: &uuid::Uuid) -> String {
    format!("expense_categories:{id}")
}

pub fn games_active() -> &'static str {
    "games:active"
}

pub fn game(id: &uuid::Uuid) -> String {
    format!("games:{id}")
}

pub fn session_device(device_id: &uuid::Uuid) -> String {
    format!("session:device:{device_id}")
}

pub fn session_enriched(id: &uuid::Uuid) -> String {
    format!("session:enriched:{id}")
}

pub fn balance_active(player_id: &uuid::Uuid, scope: &str) -> String {
    format!("balance:active:{player_id}:{scope}")
}

pub fn balance_raw(id: &uuid::Uuid) -> String {
    format!("balance:raw:{id}")
}

pub fn user_username(username: &str) -> String {
    format!("user:username:{}", username.to_lowercase())
}

pub fn user_id(id: &uuid::Uuid) -> String {
    format!("user:id:{id}")
}

pub fn users_list(filter_hash: &str) -> String {
    format!("users:list:{filter_hash}")
}

pub fn jwt_blacklist(jti: &str) -> String {
    format!("jwt:blacklist:{jti}")
}

pub fn otp_rate_limit(username: &str) -> String {
    format!("otp:ratelimit:{}", username.to_lowercase())
}

pub fn credit_outstanding(player_id: &uuid::Uuid) -> String {
    format!("credit:outstanding:{player_id}")
}

pub fn cash_register_totals(id: &uuid::Uuid) -> String {
    format!("cashregister:totals:{id}")
}

pub fn stock_level(location_id: &uuid::Uuid, product_id: &uuid::Uuid) -> String {
    format!("stock:level:{location_id}:{product_id}")
}

pub fn stats_dashboard(filter_hash: &str) -> String {
    format!("stats:v5:dashboard:{filter_hash}")
}

pub fn stats_staff(filter_hash: &str) -> String {
    format!("stats:v5:staff:{filter_hash}")
}

pub fn stats_revenue(filter_hash: &str) -> String {
    format!("stats:v5:revenue:{filter_hash}")
}

pub fn stats_usage(filter_hash: &str) -> String {
    format!("stats:v5:usage:{filter_hash}")
}

pub const STATS_PREFIX: &str = "stats:v5:";

pub const NOTIFICATIONS_PREFIX: &str = "notifications:";

pub const MAX_INBOX_NOTIFICATIONS: i64 = 10;

pub fn notifications_user_prefix(user_id: &uuid::Uuid) -> String {
    format!("notifications:{user_id}:")
}

pub fn notifications_inbox(user_id: &uuid::Uuid, filter_hash: &str) -> String {
    format!("notifications:{user_id}:inbox:{filter_hash}")
}

pub fn notifications_unread(user_id: &uuid::Uuid) -> String {
    format!("notifications:{user_id}:unread")
}

/// Stable hash for filter DTOs used in list cache keys.
pub fn filter_hash(value: &impl serde::Serialize) -> String {
    use sha2::{Digest, Sha256};
    let json = serde_json::to_string(value).unwrap_or_default();
    let digest = Sha256::digest(json.as_bytes());
    hex::encode(&digest[..8])
}
