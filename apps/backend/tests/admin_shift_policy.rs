//! Counter access follows assigned permissions, independent of legacy account kind.
use gaming_cafe_api::{
    access::MANAGED,
    dto::JwtUserClaims,
    middleware::{require_staff, require_staff_for_counter},
};
use uuid::Uuid;

fn claims(kind: &str, permissions: &[&str]) -> JwtUserClaims {
    let id = Uuid::new_v4().to_string();
    JwtUserClaims {
        sub: id.clone(),
        userId: id,
        roles: vec![kind.into()],
        permissions: permissions.iter().map(|p| (*p).into()).collect(),
        tenantId: gaming_cafe_api::models::DEFAULT_ORGANIZATION_ID.to_string(),
        orgIds: vec![],
        allowedTenants: vec![],
        rateLimit: None,
        iss: "gamezone".into(),
        aud: serde_json::json!("gamezone"),
        iat: None,
        exp: None,
        appId: "test".into(),
        deviceId: None,
    }
}

#[test]
fn either_panel_account_kind_can_operate_counter_with_explicit_grant() {
    for kind in ["admin", "staff"] {
        let user = claims(kind, &[MANAGED, "shifts:read", "shifts:write"]);
        assert!(require_staff(&user).is_ok());
        assert!(require_staff_for_counter(&user).is_ok());
    }
}

#[test]
fn managed_accounts_without_counter_grant_are_denied() {
    for kind in ["admin", "staff"] {
        let user = claims(kind, &[MANAGED, "kitchen:read"]);
        assert!(require_staff(&user).is_err());
        assert!(require_staff_for_counter(&user).is_err());
    }
}
