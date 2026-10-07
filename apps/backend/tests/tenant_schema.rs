//! Separate tenant files enforce resource ownership and require local membership.
mod support;
use gaming_cafe_api::repositories::{TenantDeviceRepository, TenantShiftRepository};
use serde_json::json;
#[tokio::test]
async fn resources_and_memberships_stay_in_their_organization() {
    let a = support::TenantFixture::new().await;
    let b = support::TenantFixture::new().await;
    let va = a.venue("main").await;
    let vb = b.venue("main").await;
    let da = TenantDeviceRepository::new(a.db.clone());
    let db = TenantDeviceRepository::new(b.db.clone());
    let device=da.create(&serde_json::from_value(json!({"name":"PC-01","locationId":va,"deviceType":"PC","deviceSubType":"HIGH_END_PCS"})).unwrap(),None).await.unwrap();
    db.create(&serde_json::from_value(json!({"name":"PC-01","locationId":vb,"deviceType":"PC","deviceSubType":"HIGH_END_PCS"})).unwrap(),None).await.unwrap();
    assert!(db.find_by_id(device.id).await.unwrap().is_none());
    assert!(da.create(&serde_json::from_value(json!({"name":"wrong","locationId":vb,"deviceType":"PC","deviceSubType":"HIGH_END_PCS"})).unwrap(),None).await.is_err());
    let user = a.staff(Some(va), vec!["shifts:write".into()]).await;
    let shifts = TenantShiftRepository::new(b.db.clone());
    assert!(shifts.create(user, vb, None, user).await.is_err());
    assert!(TenantShiftRepository::new(a.db.clone())
        .create(user, va, None, user)
        .await
        .is_ok());
    a.close().await;
    b.close().await;
}
