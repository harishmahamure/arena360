use crate::{
    app::AppState,
    dto::{ok, ApiResult, JwtUserClaims},
    error::AppError,
    middleware::{AdminOrStaff, AdminUser},
    services::kitchen_service,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

// The existing sales ledger belongs to the original venue and has no organization
// column. Fail closed for other tenants until their ledgers are independently scoped.
pub(crate) fn require_venue(claims: &JwtUserClaims) -> Result<(), AppError> {
    if claims.tenantId != crate::models::DEFAULT_ORGANIZATION_ID.to_string() {
        return Err(AppError::Forbidden(
            "This operation requires the original venue ledger".into(),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct TicketQuery {
    pub history: Option<bool>,
}

pub async fn list(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Query(query): Query<TicketQuery>,
) -> ApiResult<Value> {
    require_venue(&claims)?;
    let tickets: Value = sqlx::query_scalar(r#"
        SELECT coalesce(jsonb_agg(x ORDER BY x->>'createdAt'), '[]'::jsonb) FROM (
            SELECT jsonb_build_object('id',k.id,'transactionId',k.transaction_id,'status',k.status,
                'revision',k.revision,'items',k.items,'customer',k.customer,'notes',k.notes,
                'createdAt',k.created_at,'updatedAt',k.updated_at,'dueAt',k.due_at,
                'paymentStatus',t."paymentStatus"::text,
                'events',(SELECT coalesce(jsonb_agg(jsonb_build_object('status',e.status,'actor',u.username,
                    'reason',e.reason,'at',e.created_at) ORDER BY e.id),'[]'::jsonb)
                    FROM kitchen_ticket_events e LEFT JOIN users u ON u.id=e.actor_id WHERE e.ticket_id=k.id)) x
            FROM kitchen_tickets k JOIN transactions t ON t.id=k.transaction_id
            WHERE CASE WHEN $1 THEN k.status IN ('served','cancelled') AND k.updated_at > now()-interval '7 days'
                ELSE k.status IN ('queued','preparing','ready') END
            ORDER BY CASE WHEN $1 THEN k.created_at END DESC, k.created_at ASC LIMIT 500
        ) q
    "#).bind(query.history.unwrap_or(false)).fetch_one(&state.db).await?;
    ok(tickets)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdvanceTicket {
    pub status: String,
    pub expected_revision: i32,
    pub reason: Option<String>,
}

pub async fn advance(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<AdvanceTicket>,
) -> ApiResult<Value> {
    require_venue(&claims)?;
    let reason = dto.reason.as_deref().unwrap_or("").trim();
    if reason.len() > 500 || (dto.status == "cancelled" && reason.is_empty()) {
        return Err(AppError::BadRequest(
            "Cancellation needs a reason of 1–500 characters".into(),
        ));
    }
    let mut tx = state.db.begin().await?;
    let (status, revision): (String, i32) =
        sqlx::query_as("SELECT status,revision FROM kitchen_tickets WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("Kitchen ticket not found".into()))?;
    if revision != dto.expected_revision {
        return Err(AppError::Conflict(
            "Ticket changed. Refresh and try again.".into(),
        ));
    }
    if !kitchen_service::valid_transition(&status, &dto.status) {
        return Err(AppError::BadRequest(
            "This kitchen status transition is not allowed".into(),
        ));
    }
    if dto.status != "cancelled" {
        let payment: String=sqlx::query_scalar(r#"SELECT t."paymentStatus"::text FROM transactions t JOIN kitchen_tickets k ON k.transaction_id=t.id WHERE k.id=$1 FOR SHARE OF t"#)
            .bind(id).fetch_one(&mut *tx).await?;
        if !matches!(payment.as_str(), "completed" | "credit") {
            return Err(AppError::Conflict(
                "Payment is no longer valid. Review the sale and cancel this ticket.".into(),
            ));
        }
    }
    sqlx::query(
        "UPDATE kitchen_tickets SET status=$2, revision=revision+1, updated_at=now() WHERE id=$1",
    )
    .bind(id)
    .bind(&dto.status)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO kitchen_ticket_events(ticket_id,status,actor_id,reason) VALUES($1,$2,$3,$4)",
    )
    .bind(id)
    .bind(&dto.status)
    .bind(claims.user_id_uuid())
    .bind(if reason.is_empty() {
        None
    } else {
        Some(reason)
    })
    .execute(&mut *tx)
    .await?;
    let receipts = kitchen_service::publish(&mut tx, id).await?;
    tx.commit().await?;
    state.outbox.notify_all_committed(receipts);
    ok(serde_json::json!({"id":id,"status":dto.status,"revision":revision+1}))
}

pub async fn menu(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Value> {
    require_venue(&claims)?;
    let rows:Value=sqlx::query_scalar(r#"SELECT coalesce(jsonb_agg(jsonb_build_object('productId',p.id,'name',p.name,
        'enabled',coalesce(m.enabled,false),'station',coalesce(m.station,'Kitchen'),
        'prepMinutes',coalesce(m.prep_minutes,15),'revision',coalesce(m.revision,0)) ORDER BY p.name),'[]'::jsonb)
        FROM products p LEFT JOIN kitchen_menu_settings m ON m.product_id=p.id WHERE p."deletedAt" IS NULL"#)
        .fetch_one(&state.db).await?;
    ok(rows)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuSetting {
    pub enabled: bool,
    pub station: String,
    pub prep_minutes: i32,
    pub expected_revision: i32,
}

pub async fn save_menu(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<MenuSetting>,
) -> ApiResult<Value> {
    require_venue(&claims)?;
    let station = dto.station.trim();
    if station.is_empty() || station.chars().count() > 60 || !(1..=240).contains(&dto.prep_minutes)
    {
        return Err(AppError::BadRequest(
            "Provide a station (1–60 characters) and preparation time (1–240 minutes)".into(),
        ));
    }
    let mut tx = state.db.begin().await?;
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM products WHERE id=$1 AND "deletedAt" IS NULL)"#,
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        return Err(AppError::NotFound("Product not found".into()));
    }
    // Serialize even the first insert, so revision zero is a compare-and-set operation.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;
    let revision: Option<i32> =
        sqlx::query_scalar("SELECT revision FROM kitchen_menu_settings WHERE product_id=$1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    if revision.unwrap_or(0) != dto.expected_revision {
        return Err(AppError::Conflict(
            "Menu setting changed. Reload before saving.".into(),
        ));
    }
    sqlx::query("INSERT INTO kitchen_menu_settings(product_id,enabled,station,prep_minutes,updated_by) VALUES($1,$2,$3,$4,$5) ON CONFLICT(product_id) DO UPDATE SET enabled=$2,station=$3,prep_minutes=$4,updated_by=$5,updated_at=now(),revision=kitchen_menu_settings.revision+1")
        .bind(id).bind(dto.enabled).bind(station).bind(dto.prep_minutes).bind(claims.user_id_uuid()).execute(&mut *tx).await?;
    let receipts = kitchen_service::publish(&mut tx, id).await?;
    tx.commit().await?;
    state.outbox.notify_all_committed(receipts);
    ok(serde_json::json!({"saved":true}))
}
