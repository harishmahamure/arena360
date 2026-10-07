//! Tenant-local grants honor current roles, templates, module switches, and tenant identity.
mod support;
use gaming_cafe_api::{access::MANAGED, repositories::TenantSettingsRepository};
#[tokio::test]
async fn roles_are_scoped_templates_do_not_grant_and_modules_override() {
    let f = support::TenantFixture::new().await;
    let other = support::TenantFixture::new().await;
    let user = f
        .staff(None, vec!["finance:read".into(), "unknown:grant".into()])
        .await;
    let repo = TenantSettingsRepository::new(f.db.clone());
    assert_eq!(
        repo.effective_permissions(user).await.unwrap(),
        vec!["finance:read", MANAGED]
    );
    assert!(repo
        .ensure_access(other.db.tenant_id(), user, "finance:read")
        .await
        .is_err());
    assert!(TenantSettingsRepository::new(other.db.clone())
        .effective_permissions(user)
        .await
        .is_err());
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("INSERT INTO access_modules(module,enabled) VALUES('finance',0)")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert_eq!(
        repo.effective_permissions(user).await.unwrap(),
        vec![MANAGED]
    );
    assert!(repo
        .ensure_access(f.db.tenant_id(), user, "finance:read")
        .await
        .is_err());
    f.db.with_immediate_writer(move |c|Box::pin(async move {
        sqlx::query("UPDATE access_modules SET enabled=1 WHERE module='finance'").execute(&mut *c).await?;
        sqlx::query("UPDATE access_roles SET is_template=1 WHERE id IN(SELECT role_id FROM access_assignments WHERE user_id=?)").bind(user.to_string()).execute(c).await?; Ok(())
    })).await.unwrap();
    assert_eq!(
        repo.effective_permissions(user).await.unwrap(),
        vec![MANAGED]
    );
    assert!(repo
        .ensure_access(f.db.tenant_id(), user, "finance:read")
        .await
        .is_err());
    f.close().await;
    other.close().await;
}
