use crate::{dto::JwtUserClaims};
use serde_json::Value;
pub mod routes;
pub mod scope;
pub const MANAGED: &str = "system:managed-access-v1";
pub fn catalog() -> Value {
    serde_json::from_str(include_str!("catalog.json")).expect("checked-in access catalog")
}
pub fn has(claims: &JwtUserClaims, permission: &str) -> bool {
    claims.permissions.iter().any(|p| p == permission)
}
pub fn managed(claims: &JwtUserClaims) -> bool {
    has(claims, MANAGED)
}
pub fn known(permission: &str) -> bool {
    static GRANTS: std::sync::OnceLock<std::collections::HashSet<String>> =
        std::sync::OnceLock::new();
    GRANTS
        .get_or_init(|| {
            catalog()
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|m| {
                    m["permissions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| p["key"].as_str().unwrap().to_string())
                })
                .collect()
        })
        .contains(permission)
}
