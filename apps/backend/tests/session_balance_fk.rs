//! Tenant SQLite rejects null or missing wallets atomically.
mod support;
use support::SessionFixture;

async fn rejects_balance(balance: Option<String>) {
    let f = SessionFixture::new().await;
    let player = f.player.to_string();
    let device = f.device.to_string();
    let venue = f.venue.to_string();
    let result = f.tenant.db.with_immediate_writer(move |c| Box::pin(async move {
        let at = gaming_cafe_api::time::format_sqlite_timestamp(&chrono::Utc::now()).unwrap();
        sqlx::query("INSERT INTO usage_sessions(id,player_id,balance_id,device_id,location_id,start_time,wallet_minutes_at_start,created_at,updated_at) VALUES(?,?,?,?,?,?,120,?,?)")
            .bind(uuid::Uuid::now_v7().to_string()).bind(player).bind(balance).bind(device).bind(venue).bind(&at).bind(&at).bind(&at).execute(c).await?; Ok(())
    })).await;
    assert!(result.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM usage_sessions")
            .fetch_one(&f.tenant.db.read_pool().unwrap())
            .await
            .unwrap(),
        0
    );
    // A failed insertion cannot leave the writer poisoned or block a valid start.
    assert_eq!(f.start(None).await.balance_id, f.balance);
    f.close().await;
}
#[tokio::test]
async fn active_sessions_require_non_null_balance_id() {
    rejects_balance(None).await;
}
#[tokio::test]
async fn missing_wallet_is_rejected_by_foreign_key() {
    rejects_balance(Some(uuid::Uuid::now_v7().to_string())).await;
}
