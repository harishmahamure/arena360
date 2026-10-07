//! Session login snapshots are immutable while the live wallet changes (ADR-0050).
mod support;
use support::SessionFixture;

#[tokio::test]
async fn session_snapshot_preserved_after_recharge() {
    let f = SessionFixture::new().await;
    let session = f.start(None).await;
    assert_eq!(session.wallet_minutes_at_start, Some(120));
    assert_eq!(session.source_plan_id_at_start, Some(f.plan));
    let updated = f.recharge().await;
    assert_eq!(updated.remaining_minutes, 240);
    let enriched = f
        .sessions
        .get_by_id_tenant(f.tenant.db.clone(), session.id, "UTC".into())
        .await
        .unwrap();
    assert_eq!(enriched.wallet_minutes_at_start, Some(120));
    assert_eq!(enriched.source_plan_id_at_start, Some(f.plan));
    assert_eq!(
        enriched.balance.as_ref().map(|b| b.remaining_minutes),
        Some(240)
    );
    f.sessions
        .end_tenant(
            f.tenant.db.clone(),
            session.id,
            gaming_cafe_api::models::EndSessionDto {
                reason: Some("force".into()),
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
    f.close().await;
}
