//! Run against an isolated, migrated database:
//! DATABASE_URL=... cargo test --test panel_session -- --ignored
use gaming_cafe_api::{dto::JwtUserClaims, middleware::auth::panel_session_active};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires an isolated migrated DATABASE_URL"]
async fn rejects_disabled_deleted_role_changed_and_revoked_membership_sessions() {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("DATABASE_URL").expect("isolated DATABASE_URL"))
        .await
        .unwrap();
    let id = Uuid::new_v4();
    sqlx::query(r#"INSERT INTO users (id, username, password_hash, role) VALUES ($1, $2, 'unused', 'staff')"#)
        .bind(id).bind(format!("session-test-{id}")).execute(&pool).await.unwrap();
    let organization: Uuid = sqlx::query_scalar(
        r#"SELECT "organizationId" FROM organization_memberships WHERE "userId" = $1 LIMIT 1"#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let claims = JwtUserClaims {
        sub: id.to_string(),
        userId: id.to_string(),
        roles: vec!["staff".into()],
        tenantId: organization.to_string(),
        orgIds: vec![organization.to_string()],
        permissions: gaming_cafe_api::access::effective(&pool, organization, id)
            .await
            .unwrap(),
        allowedTenants: vec![],
        rateLimit: None,
        iss: "gamezone".into(),
        aud: serde_json::json!("gamezone"),
        iat: None,
        exp: Some(chrono::Utc::now().timestamp() + 60),
        appId: "test".into(),
        deviceId: None,
        locationId: None,
    };
    assert!(panel_session_active(&pool, &claims).await.unwrap());
    for update in [
        r#"UPDATE users SET "isActive" = FALSE WHERE id = $1"#,
        r#"UPDATE users SET "isActive" = TRUE, "deletedAt" = NOW() WHERE id = $1"#,
        r#"UPDATE users SET "deletedAt" = NULL, role = 'player' WHERE id = $1"#,
    ] {
        sqlx::query(update).bind(id).execute(&pool).await.unwrap();
        assert!(!panel_session_active(&pool, &claims).await.unwrap());
    }
    sqlx::query("UPDATE users SET role = 'staff' WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(panel_session_active(&pool, &claims).await.unwrap());
    sqlx::query(r#"UPDATE organization_memberships SET "isActive" = FALSE WHERE "userId" = $1"#)
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(!panel_session_active(&pool, &claims).await.unwrap());
    sqlx::query("DELETE FROM access_assignments WHERE user_id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
}
