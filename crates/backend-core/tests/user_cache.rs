//! Tenant user reads bypass obsolete global credential caches.
mod support;
use gaming_cafe_api::{
    dto::JwtUserClaims,
    models::{UpdateUserDto, UserFilterDto},
    services::UserService,
};
use support::TenantFixture;

#[tokio::test]
async fn user_reads_are_tenant_local_and_never_expose_credentials() {
    let a = TenantFixture::new().await;
    let b = TenantFixture::new().await;
    let alice = a.player("same-name").await;
    let bob = b.player("same-name").await;
    let users = UserService::new();
    let first = users
        .list_tenant(a.db.clone(), UserFilterDto::default())
        .await
        .unwrap();
    let again = users
        .list_tenant(a.db.clone(), UserFilterDto::default())
        .await
        .unwrap();
    assert_eq!(first.total, 1);
    assert_eq!(first.data[0].id, alice);
    assert_eq!(again.data[0].id, alice);
    let other = users
        .list_tenant(b.db.clone(), UserFilterDto::default())
        .await
        .unwrap();
    assert_eq!(other.data[0].id, bob);
    assert!(users.get_by_id_tenant(a.db.clone(), bob).await.is_err());
    for user in [&first.data[0], &other.data[0]] {
        assert!(user.password_hash.is_none());
        assert!(user.totp_secret.is_none());
        assert!(user.session_otp.is_none());
    }
    a.close().await;
    b.close().await;
}

#[tokio::test]
async fn user_changes_and_password_rotation_are_visible_without_cached_credentials() {
    let fixture = TenantFixture::new().await;
    let id = fixture.player("before-name").await;
    let users = UserService::new();
    let auth = users
        .find_by_username_for_auth_tenant(fixture.db.clone(), "before-name")
        .await
        .unwrap()
        .unwrap();
    assert!(bcrypt::verify("initial-password", auth.password_hash.as_deref().unwrap()).unwrap());
    users
        .update_tenant(
            fixture.db.clone(),
            id,
            serde_json::from_value::<UpdateUserDto>(
                serde_json::json!({"username":"after-name","firstName":"New name"}),
            )
            .unwrap(),
            None,
        )
        .await
        .unwrap();
    assert!(users
        .find_by_username_for_auth_tenant(fixture.db.clone(), "before-name")
        .await
        .unwrap()
        .is_none());
    let claims: JwtUserClaims = serde_json::from_value(serde_json::json!({
        "sub":id,"userId":id,"tenantId":fixture.db.tenant_id(),"roles":["player"],
        "iss":"gamezone","aud":"gamezone","appId":"game-zone-kiosk","orgIds":[fixture.db.tenant_id()],
        "permissions":[],"allowedTenants":[fixture.db.tenant_id()]
    })).unwrap();
    users
        .change_password_tenant(fixture.db.clone(), id, "rotated-password", &claims)
        .await
        .unwrap();
    let auth = users
        .find_by_username_for_auth_tenant(fixture.db.clone(), "after-name")
        .await
        .unwrap()
        .unwrap();
    assert!(bcrypt::verify("rotated-password", auth.password_hash.as_deref().unwrap()).unwrap());
    assert!(!bcrypt::verify("initial-password", auth.password_hash.as_deref().unwrap()).unwrap());
    let public = users
        .get_by_id_tenant(fixture.db.clone(), id)
        .await
        .unwrap();
    assert_eq!(public.first_name.as_deref(), Some("New name"));
    assert!(public.password_hash.is_none());
    assert!(public.totp_secret.is_none());
    fixture.close().await;
}
