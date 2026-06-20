use gaming_cafe_api::dto::JwtUserClaims;
use gaming_cafe_api::middleware::{require_staff, require_staff_for_counter};

fn admin_claims() -> JwtUserClaims {
    JwtUserClaims {
        sub: "admin-1".to_string(),
        permissions: vec![],
        allowedTenants: vec![],
        rateLimit: None,
        iss: "gamezone".to_string(),
        aud: serde_json::json!("gamezone"),
        iat: None,
        exp: None,
        userId: "00000000-0000-0000-0000-000000000001".to_string(),
        tenantId: "test".to_string(),
        roles: vec!["admin".to_string()],
        appId: "test".to_string(),
        orgIds: vec![],
        deviceId: None,
    }
}

fn staff_claims() -> JwtUserClaims {
    let mut claims = admin_claims();
    claims.sub = "staff-1".to_string();
    claims.userId = "00000000-0000-0000-0000-000000000002".to_string();
    claims.roles = vec!["staff".to_string()];
    claims
}

#[test]
fn require_staff_accepts_staff_role() {
    assert!(require_staff(&staff_claims()).is_ok());
}

#[test]
fn require_staff_rejects_admin_role() {
    let err = require_staff(&admin_claims()).unwrap_err();
    assert!(err.to_string().contains("Shifts are staff-only"));
}

#[test]
fn require_staff_for_counter_accepts_staff_role() {
    assert!(require_staff_for_counter(&staff_claims()).is_ok());
}

#[test]
fn require_staff_for_counter_rejects_admin_role() {
    let err = require_staff_for_counter(&admin_claims()).unwrap_err();
    assert!(err
        .to_string()
        .contains("Staff login required for counter operations"));
}

#[test]
fn admin_is_not_staff() {
    let claims = admin_claims();
    assert!(claims.is_admin());
    assert!(!claims.is_staff());
}

#[test]
fn staff_is_not_admin() {
    let claims = staff_claims();
    assert!(claims.is_staff());
    assert!(!claims.is_admin());
}
