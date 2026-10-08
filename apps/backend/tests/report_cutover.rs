//! Reporting stays unavailable until the tenant analytics projection is ready; cached legacy data cannot bypass scope.
mod support;
use axum::http::StatusCode;
use chrono::Utc;
use gaming_cafe_api::{app::build_router, cache::CacheService, dto::JwtUserClaims};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::json;
use std::sync::Arc;
#[tokio::test]
async fn authorized_reports_are_unavailable_and_revoked_access_stays_forbidden() {
    let fixture = support::SessionFixture::new().await;
    let permissions = vec![
        "finance:read".into(),
        "stats:read".into(),
        "credit:read".into(),
        "expenses:read".into(),
        "inventory:read".into(),
    ];
    let user = fixture
        .tenant
        .staff(Some(fixture.venue), permissions.clone())
        .await;
    let mut state = fixture.app().await;
    let cache = Arc::new(support::MemoryCache::default());
    cache
        .set_value(
            "stats:dashboard:legacy",
            &json!({"revenue":{"total":999}}),
            std::time::Duration::from_secs(60),
        )
        .await
        .unwrap();
    Arc::get_mut(&mut state).unwrap().cache = cache;
    let tenant = fixture.tenant.db.tenant_id();
    let claims: JwtUserClaims = serde_json::from_value(json!({"sub":user,"userId":user,"tenantId":tenant,"roles":["staff"],"permissions":permissions,"allowedTenants":[tenant],"iss":"gamezone","aud":"gamezone","appId":"admin","orgIds":[tenant],"iat":Utc::now().timestamp(),"exp":Utc::now().timestamp()+3600})).unwrap();
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.settings.jwt_secret.as_bytes()),
    )
    .unwrap();
    let app = build_router(state.clone());
    let routes = [
        "/stats/dashboard",
        "/stats/business",
        "/stats/staff-dashboard",
        "/stats/revenue/by-payment-method",
        "/stats/usage",
        "/stats/finance/reconciliation",
        "/stats/finance/deposits",
        "/stats/finance/variance",
        "/stats/finance/report",
        "/credit/summary",
        "/expenses/summary",
        "/inventory/overview",
        "/inventory/receipts/summary",
        "/inventory/waste/summary",
    ];
    for route in routes {
        let path = format!("{route}?startDate=2026-10-01&endDate=2026-10-02");
        let (status, body) =
            support::request(app.clone(), "GET", &path, Some(&token), None, json!({})).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{route}: {body}");
        assert!(body.to_string().contains("ANALYTICS_UNAVAILABLE"), "{body}");
    }
    #[cfg(feature = "duckdb-analytics")]
    {
        use gaming_cafe_api::analytics::tenant_db::error;
        let analytics = state
            .analytics
            .get(fixture.tenant.db.clone())
            .await
            .unwrap();
        analytics
            .write(|tx| {
                tx.execute_batch(
                    "UPDATE _ingest_state SET status='READY',hot_window_start=DATE '2025-01-01'",
                )
                .map_err(error)?;
                Ok(())
            })
            .await
            .unwrap();
        for route in routes {
            let path = format!("{route}?startDate=2026-10-01&endDate=2026-10-02");
            let (status, body) =
                support::request(app.clone(), "GET", &path, Some(&token), None, json!({})).await;
            assert_eq!(status, StatusCode::OK, "{route}: {body}");
        }
    }
    fixture.tenant.db.with_immediate_writer(move |c| Box::pin(async move {
        sqlx::query("UPDATE access_roles SET permissions='[]' WHERE id IN(SELECT role_id FROM access_assignments WHERE user_id=?)").bind(user.to_string()).execute(c).await?;
        Ok(())
    })).await.unwrap();
    for route in routes {
        let path = format!("{route}?startDate=2026-10-01&endDate=2026-10-02");
        let (status, body) =
            support::request(app.clone(), "GET", &path, Some(&token), None, json!({})).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{route}: {body}");
    }
    fixture.close().await;
}
