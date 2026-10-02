use crate::{error::AppError, realtime::OutboxService};
use serde_json::json;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub fn valid_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("queued", "preparing")
            | ("preparing", "ready")
            | ("ready", "served")
            | ("queued" | "preparing" | "ready", "cancelled")
    )
}

pub async fn publish(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<(), AppError> {
    for role in ["admin", "staff"] {
        OutboxService::publish_in_tx(
            tx,
            role,
            "kitchen.changed",
            json!({"ticketId": id}),
            Some(role),
            None,
            None,
            true,
        )
        .await?;
    }
    Ok(())
}

/// Called inside the sale transaction, after its line items exist. Unique transaction_id
/// prevents duplicate preparation when a payment is retried or credit is settled later.
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    transaction_id: Uuid,
    actor: Option<Uuid>,
) -> Result<(), AppError> {
    let id: Option<Uuid> = sqlx::query_scalar(r#"
        INSERT INTO kitchen_tickets (transaction_id, items, customer, notes, due_at)
        SELECT t.id, jsonb_agg(jsonb_build_object('productId',p.id,'name',p.name,
            'quantity',tp.quantity,'station',m.station) ORDER BY p.name), u.username, t.notes,
            now() + make_interval(mins => max(m.prep_minutes))
        FROM transactions t JOIN users u ON u.id=t."playerId"
        JOIN transaction_products tp ON tp."transactionId"=t.id
        JOIN products p ON p.id=tp."productId"
        JOIN kitchen_menu_settings m ON m.product_id=p.id AND m.enabled
        WHERE t.id=$1 AND t."deletedAt" IS NULL AND t."paymentStatus"::text IN ('completed','credit')
        GROUP BY t.id,u.username HAVING count(*) > 0
        ON CONFLICT (transaction_id) DO NOTHING RETURNING id
    "#).bind(transaction_id).fetch_optional(&mut **tx).await?;
    if let Some(id) = id {
        sqlx::query(
            "INSERT INTO kitchen_ticket_events(ticket_id,status,actor_id) VALUES($1,'queued',$2)",
        )
        .bind(id)
        .bind(actor)
        .execute(&mut **tx)
        .await?;
        publish(tx, id).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progression_and_terminal_states() {
        assert!(valid_transition("queued", "preparing"));
        assert!(valid_transition("preparing", "ready"));
        assert!(valid_transition("ready", "served"));
        for state in ["queued", "preparing", "ready"] {
            assert!(valid_transition(state, "cancelled"));
        }
        for state in ["served", "cancelled"] {
            for next in ["queued", "preparing", "ready", "served", "cancelled"] {
                assert!(!valid_transition(state, next));
            }
        }
        assert!(!valid_transition("queued", "served"));
        assert!(!valid_transition("ready", "preparing"));
    }
}
