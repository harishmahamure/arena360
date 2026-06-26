use sqlx::PgPool;
use uuid::Uuid;

use crate::models::PlayerPlanBalance;
use crate::realtime::OutboxService;

/// Publish `balance.updated` to device + user channels when the player has an open session.
pub async fn publish_balance_updated_for_player(
    pool: &PgPool,
    outbox: &OutboxService,
    player_id: Uuid,
    balance: &PlayerPlanBalance,
) {
    let open: Option<(Uuid, Uuid)> = sqlx::query_as(
        r#"
        SELECT s.id, s."deviceId"
        FROM usage_sessions s
        INNER JOIN player_plan_balances b ON b.id = s."balanceId" AND b."deletedAt" IS NULL
        WHERE b."playerId" = $1 AND s."endTime" IS NULL AND s."deletedAt" IS NULL
        ORDER BY s."startTime" DESC
        LIMIT 1
        "#,
    )
    .bind(player_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let (session_id, device_id) = match open {
        Some(pair) => pair,
        None => return,
    };

    publish_balance_updated_for_session(outbox, player_id, device_id, session_id, balance).await;
}

/// Publish `balance.updated` for a known open session (e.g. heartbeat).
pub async fn publish_balance_updated_for_session(
    outbox: &OutboxService,
    player_id: Uuid,
    device_id: Uuid,
    session_id: Uuid,
    balance: &PlayerPlanBalance,
) {
    let payload = serde_json::json!({
        "balanceId": balance.id.to_string(),
        "remainingMinutes": balance.remaining_minutes,
        "playerId": player_id.to_string(),
        "sessionId": session_id.to_string(),
    });

    let device_channel = format!("device:{device_id}");
    let user_channel = format!("user:{player_id}");
    let _ = outbox
        .publish(
            &device_channel,
            "balance.updated",
            payload.clone(),
            None,
            None,
            false,
        )
        .await;
    let _ = outbox
        .publish(
            &user_channel,
            "balance.updated",
            payload.clone(),
            None,
            Some(player_id),
            false,
        )
        .await;
    let _ = outbox
        .publish(
            "staff",
            "balance.updated",
            payload,
            None,
            None,
            false,
        )
        .await;
}
