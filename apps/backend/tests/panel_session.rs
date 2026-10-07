//! Live tenant session checks reject disabled, deleted, revoked and changed memberships.
mod support;
use gaming_cafe_api::{
    dto::JwtUserClaims, realtime::tenant_transport::current_claims,
    repositories::TenantSettingsRepository,
};
use serde_json::json;
#[tokio::test]
async fn rejects_disabled_deleted_role_changed_and_revoked_membership_sessions() {
    let f = support::TenantFixture::new().await;
    let user = f.staff(None, vec!["finance:read".into()]).await;
    let repo = TenantSettingsRepository::new(f.db.clone());
    let claims: JwtUserClaims = serde_json::from_value(json!({
        "sub":user,"userId":user,"roles":["staff"],"tenantId":f.db.tenant_id(),
        "orgIds":[f.db.tenant_id()],"permissions":repo.effective_permissions(user).await.unwrap(),
        "allowedTenants":[],"iss":"gamezone","aud":"gamezone","exp":chrono::Utc::now().timestamp()+60,"appId":"game-zone-admin"
    })).unwrap();
    assert!(current_claims(f.db.clone(), &claims).await.is_ok());
    for statement in [
        "UPDATE users SET is_active=0 WHERE id=?",
        "UPDATE users SET is_active=1,deleted_at='2026-10-07T00:00:00.000000Z' WHERE id=?",
        "UPDATE users SET deleted_at=NULL,role='admin' WHERE id=?",
    ] {
        f.db.with_immediate_writer(move |c| {
            Box::pin(async move {
                sqlx::query(statement)
                    .bind(user.to_string())
                    .execute(c)
                    .await?;
                Ok(())
            })
        })
        .await
        .unwrap();
        assert!(current_claims(f.db.clone(), &claims).await.is_err());
    }
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE users SET role='staff' WHERE id=?")
                .bind(user.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(current_claims(f.db.clone(), &claims).await.is_ok());
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("DELETE FROM access_assignments WHERE user_id=?")
                .bind(user.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let current = current_claims(f.db.clone(), &claims).await.unwrap();
    assert!(
        !current.permissions.contains(&"finance:read".into()),
        "revoked grants must not survive in issued JWTs"
    );
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE users SET is_active=0 WHERE id=?")
                .bind(user.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(current_claims(f.db.clone(), &claims).await.is_err());
    f.close().await;
}
