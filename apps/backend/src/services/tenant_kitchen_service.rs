//! Tenant kitchen tickets share the checkout writer transaction and immutable sale snapshots.
use crate::{
    error::AppError,
    repositories::tenant_back_office::{event, now, write},
    tenancy::{format_sqlite_timestamp, TenantDb},
};
use serde_json::{json, Value};
use sqlx::SqliteConnection;
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone)]
pub struct TenantKitchenService {
    db: Arc<TenantDb>,
    venues: Option<Vec<Uuid>>,
}
fn decode(s: String) -> Result<Value, AppError> {
    serde_json::from_str(&s).map_err(|e| AppError::Internal(e.to_string()))
}
impl TenantKitchenService {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db, venues: None }
    }
    pub fn scoped(db: Arc<TenantDb>, venues: Vec<Uuid>) -> Self {
        Self { db, venues: Some(venues) }
    }
    pub(crate) async fn enqueue_on(
        c: &mut SqliteConnection,
        transaction: Uuid,
        actor: Option<Uuid>,
    ) -> Result<(), AppError> {
        let row: Option<(String, Option<String>, String)> = sqlx::query_as("SELECT u.username,t.notes,t.location_id FROM transactions t JOIN users u ON u.id=t.player_id WHERE t.id=? AND t.deleted_at IS NULL AND t.payment_status IN('completed','credit') AND NOT EXISTS(SELECT 1 FROM kitchen_tickets WHERE transaction_id=t.id)").bind(transaction.to_string()).fetch_optional(&mut *c).await?;
        let Some((customer, notes, venue)) = row else {
            return Ok(());
        };
        let lines: Vec<(String, Option<String>, String, i32, String, i32)> = sqlx::query_as("SELECT tp.id,tp.product_id,tp.product_name,tp.quantity,m.station,m.prep_minutes FROM transaction_products tp JOIN kitchen_menu_settings m ON m.product_id=tp.product_id AND m.enabled=1 WHERE tp.transaction_id=? ORDER BY tp.product_name,tp.id").bind(transaction.to_string()).fetch_all(&mut *c).await?;
        if lines.is_empty() {
            return Ok(());
        }
        let mut items = Vec::new();
        let mut prep = 0;
        for (line, product, name, quantity, station, minutes) in lines {
            prep = prep.max(minutes);
            let options: Vec<String> = sqlx::query_scalar("SELECT name FROM transaction_product_options WHERE transaction_product_id=? ORDER BY id").bind(line).fetch_all(&mut *c).await?;
            items.push(json!({"productId":product,"name":name,"quantity":quantity,"station":station,"options":options}));
        }
        let id = Uuid::now_v7();
        let at = chrono::Utc::now();
        let ts = format_sqlite_timestamp(&at).map_err(|e| AppError::Internal(e.to_string()))?;
        let due = format_sqlite_timestamp(&(at + chrono::Duration::minutes(i64::from(prep))))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sqlx::query("INSERT INTO kitchen_tickets(id,transaction_id,items,customer,notes,created_at,due_at,updated_at) VALUES(?,?,?,?,?,?,?,?)").bind(id.to_string()).bind(transaction.to_string()).bind(json!(items).to_string()).bind(customer).bind(notes).bind(&ts).bind(due).bind(&ts).execute(&mut *c).await?;
        sqlx::query("INSERT INTO kitchen_ticket_events(ticket_id,status,actor_id,created_at) VALUES(?,'queued',?,?)").bind(id.to_string()).bind(actor.map(|x| x.to_string())).bind(&ts).execute(&mut *c).await?;
        event(
            c,
            "kitchen_ticket",
            id,
            "kitchen.changed",
            Some(Uuid::parse_str(&venue).map_err(|e| AppError::Internal(e.to_string()))?),
            false,
            json!({"ticketId":id}),
        )
        .await
    }
    pub async fn menu(&self) -> Result<Value, AppError> {
        let rows: Vec<String> = sqlx::query_scalar("SELECT json_object('productId',p.id,'name',p.name,'enabled',json(CASE WHEN COALESCE(m.enabled,0)=1 THEN 'true' ELSE 'false' END),'station',COALESCE(m.station,'Kitchen'),'prepMinutes',COALESCE(m.prep_minutes,15),'revision',COALESCE(m.revision,0)) FROM products p LEFT JOIN kitchen_menu_settings m ON m.product_id=p.id WHERE p.deleted_at IS NULL ORDER BY p.name,p.id").fetch_all(&self.db.read_pool()?).await?;
        Ok(Value::Array(
            rows.into_iter().map(decode).collect::<Result<_, _>>()?,
        ))
    }
    pub async fn save_menu(
        &self,
        id: Uuid,
        enabled: bool,
        station: &str,
        prep: i32,
        expected: i32,
        actor: Uuid,
    ) -> Result<Value, AppError> {
        let station = station.trim().to_owned();
        if station.is_empty() || station.chars().count() > 60 || !(1..=240).contains(&prep) {
            return Err(AppError::BadRequest(
                "Provide a station (1–60 characters) and preparation time (1–240 minutes)".into(),
            ));
        }
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE id=? AND deleted_at IS NULL)").bind(id.to_string()).fetch_one(&mut *c).await?;
                    if !exists {
                        return Err(AppError::NotFound("Product not found".into()));
                    }
                    let revision: Option<i32> = sqlx::query_scalar("SELECT revision FROM kitchen_menu_settings WHERE product_id=?").bind(id.to_string()).fetch_optional(&mut *c).await?;
                    if revision.unwrap_or(0) != expected {
                        return Err(AppError::Conflict("Menu setting changed. Reload before saving.".into()));
                    }
                    let next = expected.checked_add(1).ok_or_else(|| AppError::Conflict("Menu revision exhausted".into()))?;
                    sqlx::query("INSERT INTO kitchen_menu_settings(product_id,enabled,station,prep_minutes,revision,updated_by,updated_at) VALUES(?,?,?,?,?,?,?) ON CONFLICT(product_id) DO UPDATE SET enabled=excluded.enabled,station=excluded.station,prep_minutes=excluded.prep_minutes,revision=excluded.revision,updated_by=excluded.updated_by,updated_at=excluded.updated_at").bind(id.to_string()).bind(enabled).bind(station).bind(prep).bind(next).bind(actor.to_string()).bind(now()?).execute(&mut *c).await?;
                    event(c, "kitchen_menu", id, "kitchen.changed", None, false, json!({"ticketId":id})).await?;
                    Ok(json!({"saved":true}))
                })
            }),
        )
        .await
    }
    pub async fn list(&self, history: bool) -> Result<Value, AppError> {
        let cutoff = format_sqlite_timestamp(&(chrono::Utc::now() - chrono::Duration::days(7)))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let rows: Vec<String> = sqlx::query_scalar("SELECT json_object('id',k.id,'transactionId',k.transaction_id,'status',k.status,'revision',k.revision,'items',json(k.items),'customer',k.customer,'notes',k.notes,'createdAt',k.created_at,'updatedAt',k.updated_at,'dueAt',k.due_at,'paymentStatus',t.payment_status,'events',json((SELECT COALESCE(json_group_array(json(x)),'[]') FROM(SELECT json_object('status',e.status,'actor',u.username,'reason',e.reason,'at',e.created_at) x FROM kitchen_ticket_events e LEFT JOIN users u ON u.id=e.actor_id WHERE e.ticket_id=k.id ORDER BY e.id)))) FROM kitchen_tickets k JOIN transactions t ON t.id=k.transaction_id WHERE CASE WHEN ? THEN k.status IN('served','cancelled') AND k.updated_at>? ELSE k.status IN('queued','preparing','ready') END AND (? IS NULL OR t.location_id IN (SELECT value FROM json_each(?))) ORDER BY CASE WHEN ? THEN k.created_at END DESC,k.created_at,k.id LIMIT 500").bind(history).bind(cutoff).bind(self.venues.as_ref().map(|v| json!(v).to_string())).bind(self.venues.as_ref().map(|v| json!(v).to_string())).bind(history).fetch_all(&self.db.read_pool()?).await?;
        let mut values = rows
            .into_iter()
            .map(decode)
            .collect::<Result<Vec<_>, _>>()?;
        values.sort_by(|a, b| {
            a["createdAt"]
                .as_str()
                .cmp(&b["createdAt"].as_str())
                .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
        });
        Ok(json!(values))
    }
    pub async fn advance(
        &self,
        id: Uuid,
        status: &str,
        expected: i32,
        reason: Option<&str>,
        actor: Uuid,
    ) -> Result<Value, AppError> {
        let status = status.to_owned();
        let reason = reason.unwrap_or("").trim().to_owned();
        if reason.len() > 500 || status == "cancelled" && reason.is_empty() {
            return Err(AppError::BadRequest(
                "Cancellation needs a reason of 1–500 characters".into(),
            ));
        }
        let venues = self.venues.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let row: Option<(String, i32, String, String)> = sqlx::query_as("SELECT k.status,k.revision,t.payment_status,t.location_id FROM kitchen_tickets k JOIN transactions t ON t.id=k.transaction_id WHERE k.id=?").bind(id.to_string()).fetch_optional(&mut *c).await?;
                    let (old, revision, payment, venue) = row.ok_or_else(|| AppError::NotFound("Kitchen ticket not found".into()))?;
                    if venues.as_ref().is_some_and(|allowed| !allowed.iter().any(|id| id.to_string() == venue)) {
                        return Err(AppError::Forbidden("Kitchen ticket is outside the permitted venues".into()));
                    }
                    if revision != expected {
                        return Err(AppError::Conflict("Ticket changed. Refresh and try again.".into()));
                    }
                    if !super::kitchen_service::valid_transition(&old, &status) {
                        return Err(AppError::BadRequest("This kitchen status transition is not allowed".into()));
                    }
                    if status != "cancelled" && !matches!(payment.as_str(), "completed" | "credit") {
                        return Err(AppError::Conflict("Payment is no longer valid. Review the sale and cancel this ticket.".into()));
                    }
                    let next = revision.checked_add(1).ok_or_else(|| AppError::Conflict("Ticket revision exhausted".into()))?;
                    let ts = now()?;
                    sqlx::query("UPDATE kitchen_tickets SET status=?,revision=?,updated_at=? WHERE id=?").bind(&status).bind(next).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
                    sqlx::query("INSERT INTO kitchen_ticket_events(ticket_id,status,actor_id,reason,created_at) VALUES(?,?,?,?,?)").bind(id.to_string()).bind(&status).bind(actor.to_string()).bind(if reason.is_empty() { None } else { Some(reason) }).bind(ts).execute(&mut *c).await?;
                    event(c, "kitchen_ticket", id, "kitchen.changed", Some(Uuid::parse_str(&venue).map_err(|e| AppError::Internal(e.to_string()))?), false, json!({"ticketId":id})).await?;
                    Ok(json!({"id":id,"status":status,"revision":next}))
                })
            }),
        )
        .await
    }
}
