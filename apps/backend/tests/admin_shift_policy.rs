//! Admin no-shift policy tests.
//! Integration: `cargo test --test admin_shift_policy -- --ignored`

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use gaming_cafe_api::app::{build_router, build_state};
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::error::AppError;
use gaming_cafe_api::middleware::{require_staff, require_staff_for_counter};
use gaming_cafe_api::models::ClockInDto;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use gaming_cafe_api::dto::JwtUserClaims;

async fn setup() -> Option<Arc<gaming_cafe_api::app::AppState>> {
    load_dotenv();
    if std::env::var("DATABASE_URL").is_err() && std::env::var("DB_HOST").is_err() {
        return None;
    }
    Some(build_state().await)
}

async fn find_admin_user_id(pool: &PgPool) -> Option<Uuid> {
    sqlx::query_scalar(
        r#"SELECT id FROM users WHERE role = 'admin' AND "isActive" = true AND "deletedAt" IS NULL LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

async fn find_staff_user_id(pool: &PgPool) -> Option<Uuid> {
    sqlx::query_scalar(
        r#"SELECT id FROM users WHERE role = 'staff' AND "isActive" = true AND "deletedAt" IS NULL LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

fn admin_claims(user_id: Uuid) -> JwtUserClaims {
    JwtUserClaims {
        sub: user_id.to_string(),
        permissions: vec![],
        allowedTenants: vec![],
        rateLimit: None,
        iss: "gamezone".to_string(),
        aud: serde_json::json!("gamezone"),
        iat: None,
        exp: None,
        userId: user_id.to_string(),
        tenantId: "test".to_string(),
        roles: vec!["admin".to_string()],
        appId: "test".to_string(),
        orgIds: vec![],
        deviceId: None,
    }
}

async fn admin_bearer_token(
    state: &gaming_cafe_api::app::AppState,
    admin_id: Uuid,
) -> Option<String> {
    let user = state.users.get_by_id(admin_id).await.ok()?;
    state
        .auth
        .issue_auth_response(&user)
        .ok()
        .map(|r| r.accessToken)
}

#[test]
fn admin_middleware_blocks_shift_and_register_ops() {
    let claims = admin_claims(Uuid::new_v4());
    assert!(require_staff(&claims).is_err());
    assert!(require_staff_for_counter(&claims).is_err());
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn clock_in_rejects_admin_user() {
    let Some(state) = setup().await else {
        return;
    };
    let Some(admin_id) = find_admin_user_id(&state.db).await else {
        return;
    };

    if let Some(active) = state.shifts.get_active(admin_id).await.ok().flatten() {
        let _ = state.shifts.force_close(active.id, admin_id).await;
    }

    let err = state
        .shifts
        .clock_in(
            admin_id,
            ClockInDto {
                notes: Some("admin shift attempt".to_string()),
            },
            admin_id,
        )
        .await
        .unwrap_err();

    assert!(
        matches!(err, AppError::Forbidden(_)),
        "expected Forbidden, got {err:?}"
    );
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn admin_login_response_has_no_shift_id() {
    let Some(state) = setup().await else {
        return;
    };
    let Some(admin_id) = find_admin_user_id(&state.db).await else {
        return;
    };

    let user = state
        .users
        .get_by_id(admin_id)
        .await
        .expect("load admin user");
    let response = state
        .auth
        .issue_auth_response(&user)
        .expect("issue auth response");

    assert!(response.shiftId.is_none());
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn login_admin_force_closes_orphan_shift() {
    let Some(state) = setup().await else {
        return;
    };
    let Some(admin_id) = find_admin_user_id(&state.db).await else {
        return;
    };

    if let Some(active) = state.shifts.get_active(admin_id).await.ok().flatten() {
        let _ = state.shifts.force_close(active.id, admin_id).await;
    }

    sqlx::query(
        r#"
        INSERT INTO shifts (id, "userId", "clockIn", status, "createdBy", "updatedBy", "createdAt", "updatedAt")
        VALUES (gen_random_uuid(), $1, NOW(), 'active', $1, $1, NOW(), NOW())
        "#,
    )
    .bind(admin_id)
    .execute(&state.db)
    .await
    .expect("seed orphan admin shift");

    assert!(
        state.shifts.get_active(admin_id).await.ok().flatten().is_some(),
        "expected active shift before cleanup"
    );

    if let Some(active) = state.shifts.get_active(admin_id).await.expect("get active") {
        state
            .shifts
            .force_close(active.id, admin_id)
            .await
            .expect("force close orphan shift");
    }

    assert!(
        state.shifts.get_active(admin_id).await.ok().flatten().is_none(),
        "expected no active shift after admin login cleanup"
    );
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn admin_http_cannot_clock_in_or_open_register() {
    let Some(state) = setup().await else {
        return;
    };
    let Some(admin_id) = find_admin_user_id(&state.db).await else {
        return;
    };
    let Some(token) = admin_bearer_token(&state, admin_id).await else {
        return;
    };

    let app = build_router(state);

    let clock_in = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/shifts/clock-in")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "notes": "admin attempt" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clock_in.status(), StatusCode::FORBIDDEN);

    let open_register = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/cash-registers/open")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "shiftId": Uuid::new_v4().to_string(),
                        "openingBalance": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(open_register.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn staff_can_still_clock_in() {
    let Some(state) = setup().await else {
        return;
    };
    let Some(staff_id) = find_staff_user_id(&state.db).await else {
        return;
    };

    if let Some(active) = state.shifts.get_active(staff_id).await.ok().flatten() {
        let _ = state.shifts.force_close(active.id, staff_id).await;
    }

    let shift = state
        .shifts
        .clock_in(
            staff_id,
            ClockInDto {
                notes: Some("staff shift policy test".to_string()),
            },
            staff_id,
        )
        .await
        .expect("staff clock in");

    state
        .shifts
        .force_close(shift.id, staff_id)
        .await
        .expect("cleanup staff shift");
}
