//! Process startup never connects to the retired operational PostgreSQL database.
mod support;
use axum::http::StatusCode;
use gaming_cafe_api::app::{build_router, build_state_with_settings};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use support::{request, SessionFixture};
#[tokio::test]
async fn startup_ignores_unreachable_operational_pool_and_reports_missing_role_storage() {
    let state = tokio::time::timeout(
        Duration::from_secs(2),
        build_state_with_settings(Arc::new(support::settings())),
    )
    .await
    .expect("startup must not attempt the operational PostgreSQL URL");
    let app = build_router(state);
    let (status, body) = request(app.clone(), "GET", "/health/live", None, None, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["status"], "ok");
    assert_eq!(body["data"]["db"], "down");
    assert_eq!(
        request(app, "GET", "/health/ready", None, None, json!({}))
            .await
            .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
#[tokio::test]
async fn cell_readiness_checks_local_storage_without_a_control_pool() {
    let f = SessionFixture::new().await;
    let state = f.app().await;
    assert!(state.control_db.is_none());
    let app = build_router(state);
    let (status, body) = request(app.clone(), "GET", "/health/ready", None, None, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["db"], "up");
    // Closed handles are omitted, allowing an idle cell to remain ready.
    f.tenant.db.close().await.unwrap();
    assert_eq!(
        request(app, "GET", "/health/ready", None, None, json!({}))
            .await
            .0,
        StatusCode::OK
    );
    f.close().await;
}

#[tokio::test]
async fn readiness_rejects_a_fenced_tenant() {
    let f = SessionFixture::new().await;
    let app = build_router(f.app().await);
    f.tenant.revoke();
    assert_eq!(request(app, "GET", "/health/ready", None, None, json!({})).await.0,
        StatusCode::INTERNAL_SERVER_ERROR);
    f.close().await;
}
#[tokio::test]
async fn readiness_does_not_keep_idle_tenants_open() {
    let f = support::TenantFixture::new_with_idle(Duration::from_millis(50)).await;
    let mut state = build_state_with_settings(Arc::new(support::settings())).await;
    Arc::get_mut(&mut state).unwrap().tenant_dbs = Some(f.manager.clone());
    let app = build_router(state);
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(request(app, "GET", "/health/ready", None, None, json!({})).await.0, StatusCode::OK);
    assert_eq!(f.manager.reap_idle().await.unwrap(), 1);
    f.close().await;
}
