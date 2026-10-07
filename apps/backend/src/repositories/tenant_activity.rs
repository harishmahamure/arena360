//! Activity snapshots commit with canonical business events, without creating inbox entries.
use crate::{error::AppError, models::activity_kind as kind};
use serde_json::{json, Value};
use sqlx::SqliteConnection;
use uuid::Uuid;

async fn insert(
    c: &mut SqliteConnection,
    kind: &str,
    title: String,
    summary: Option<String>,
    payload: Value,
    actor: Option<String>,
    entity: &str,
    id: Uuid,
    venue: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO activity_log(id,kind,title,summary,payload,actor_user_id,entity_type,entity_id,created_at,location_id) VALUES(?,?,?,?,?,?,?,?,?,?)")
        .bind(Uuid::now_v7().to_string()).bind(kind).bind(title).bind(summary).bind(payload.to_string()).bind(actor).bind(entity).bind(id.to_string()).bind(super::tenant_back_office::now()?).bind(venue.map(|v|v.to_string())).execute(c).await?;
    Ok(())
}
async fn display_name(c: &mut SqliteConnection, user: &str) -> Result<String, AppError> {
    let value: Option<String> = sqlx::query_scalar("SELECT CASE WHEN trim(COALESCE(first_name,'')||' '||COALESCE(last_name,''))='' THEN username ELSE trim(COALESCE(first_name,'')||' '||COALESCE(last_name,'')) END FROM users WHERE id=?")
        .bind(user).fetch_optional(c).await?;
    Ok(value.unwrap_or_else(|| "Unknown".into()))
}
fn text(value: &Value, key: &str) -> Option<String> {
    value[key].as_str().map(str::to_owned)
}

pub(crate) async fn record_canonical_on(
    c: &mut SqliteConnection,
    event: &str,
    entity: &str,
    id: Uuid,
    venue: Option<Uuid>,
    payload: &Value,
) -> Result<(), AppError> {
    match event {
        "transaction.created" => {
            let (player, actor, role, transaction_type, amount, method): (String, Option<String>, Option<String>, String, f64, String) = sqlx::query_as("SELECT t.player_id,t.created_by,u.role,t.transaction_type,t.amount/10000.0,t.payment_method FROM transactions t LEFT JOIN users u ON u.id=t.created_by WHERE t.id=?")
                .bind(id.to_string()).fetch_one(&mut *c).await?;
            if role.as_deref() != Some("staff") {
                return Ok(());
            }
            let actor =
                actor.ok_or_else(|| AppError::Internal("Staff sale has no actor".into()))?;
            let staff_name = display_name(c, &actor).await?;
            let customer_name = display_name(c, &player).await?;
            let payment_label = match method.as_str() {
                "cash" => "Cash",
                "online" => "Online",
                "split_payment" => "Split payment",
                "credit" => "Credit",
                _ => "Payment",
            };
            let (kind, label) = if transaction_type == "plan_purchase" {
                (kind::PLAN_SALE, "Plan sale")
            } else {
                (kind::TRANSACTION_SALE, "Product sale")
            };
            let snapshot = json!({"transactionId":id,"amount":amount,"paymentMethod":method,"paymentLabel":payment_label,"transactionType":transaction_type,"staffId":actor,"staffName":staff_name,"customerId":player,"customerName":customer_name});
            insert(
                c,
                kind,
                format!("{label} · ₹{amount:.2}"),
                Some(format!(
                    "Sold by {staff_name} to {customer_name} · {payment_label}"
                )),
                snapshot,
                Some(actor),
                entity,
                id,
                venue,
            )
            .await?;
        }
        "credit.settled" => {
            let (player,actor,amount,method,notes,location): (String,String,f64,String,Option<String>,String) = sqlx::query_as("SELECT cs.player_id,cs.settled_by,cs.amount/10000.0,cs.payment_method,cs.notes,s.location_id FROM credit_settlements cs JOIN shifts s ON s.id=cs.shift_id WHERE cs.id=?")
                .bind(id.to_string()).fetch_one(&mut *c).await?;
            let venue =
                Uuid::parse_str(&location).map_err(|e| AppError::Internal(e.to_string()))?;
            insert(
                c,
                kind::CREDIT_SETTLEMENT,
                format!("Credit settlement: ₹{amount:.2}"),
                notes,
                json!({"settlementId":id,"playerId":player,"amount":amount,"paymentMethod":method}),
                Some(actor),
                entity,
                id,
                Some(venue),
            )
            .await?;
        }
        "shift.started" | "shift.ended" => {
            let start = event == "shift.started";
            insert(
                c,
                if start {
                    kind::SHIFT_CLOCK_IN
                } else {
                    kind::SHIFT_CLOCK_OUT
                },
                if start {
                    "Shift started"
                } else {
                    "Shift ended"
                }
                .into(),
                text(payload, "notes"),
                json!({"shiftId":id}),
                text(payload, if start { "createdBy" } else { "updatedBy" }),
                entity,
                id,
                venue,
            )
            .await?;
        }
        "shift.handover" => {
            insert(
                c,
                kind::SHIFT_HANDOVER,
                "Shift handed over".into(),
                None,
                payload.clone(),
                text(payload, "fromStaffId"),
                entity,
                id,
                venue,
            )
            .await?;
        }
        "cash_register.opened" | "cash_register.closed" => {
            let open = event == "cash_register.opened";
            insert(
                c,
                if open {
                    kind::CASH_REGISTER_OPENED
                } else {
                    kind::CASH_REGISTER_CLOSED
                },
                if open {
                    "Cash register opened"
                } else {
                    "Cash register closed"
                }
                .into(),
                if open { None } else { text(payload, "notes") },
                json!({"registerId":id,"shiftId":payload["shiftId"]}),
                text(payload, if open { "openedBy" } else { "closedBy" }),
                entity,
                id,
                venue,
            )
            .await?;
        }
        "kiosk_order.fulfilled" | "kiosk_order.status_changed"
            if event == "kiosk_order.fulfilled" || payload["status"] == "cancelled" =>
        {
            let fulfilled = event == "kiosk_order.fulfilled";
            let device: Option<String> = sqlx::query_scalar("SELECT d.name FROM kiosk_orders ko LEFT JOIN devices d ON d.id=ko.device_id WHERE ko.id=?").bind(id.to_string()).fetch_optional(&mut *c).await?.flatten();
            let transaction = payload["transactionId"].clone();
            insert(
                c,
                if fulfilled {
                    kind::KIOSK_ORDER_FULFILLED
                } else {
                    kind::KIOSK_ORDER_CANCELLED
                },
                format!(
                    "Order {} — {}",
                    if fulfilled { "fulfilled" } else { "cancelled" },
                    device.as_deref().unwrap_or("PC")
                ),
                if fulfilled {
                    Some(format!(
                        "Sale recorded · tx {}",
                        transaction.as_str().unwrap_or_default()
                    ))
                } else {
                    None
                },
                if fulfilled {
                    json!({"orderId":id,"transactionId":transaction})
                } else {
                    json!({"orderId":id})
                },
                None,
                entity,
                id,
                venue,
            )
            .await?;
        }
        "device.status_changed" => {
            let (name, status): (String, String) =
                sqlx::query_as("SELECT name,status FROM devices WHERE id=?")
                    .bind(id.to_string())
                    .fetch_one(&mut *c)
                    .await?;
            insert(
                c,
                kind::DEVICE_STATUS_CHANGED,
                format!("Device status: {status}"),
                Some(format!("{name} is now {status}")),
                json!({"deviceId":id,"status":status}),
                None,
                entity,
                id,
                venue,
            )
            .await?;
        }
        "approval.requested" | "approval.decided" => {
            let requested = event == "approval.requested";
            let entity = payload["entity_type"].as_str().unwrap_or(entity);
            let amount = payload["amount"]
                .as_f64()
                .or_else(|| payload["deposit"]["amount"].as_f64())
                .unwrap_or(0.0);
            let label = match entity {
                "cash_deposit" => "Cash deposit",
                "expense" => "Expense",
                "stock_transfer_request" => "Stock transfer",
                "stock_waste_event" => "Stock waste",
                _ => "Approval",
            };
            let status = payload["status"].as_str().unwrap_or_default();
            let title = if requested {
                match entity {
                    "cash_deposit" | "expense" => format!("{label} approval: ₹{amount:.2}"),
                    _ => format!("{label} awaiting approval"),
                }
            } else {
                format!("{label} {status}")
            };
            let actor = if requested {
                text(payload, "requestedBy")
            } else {
                let query = match entity {
                    "cash_deposit" => "SELECT approved_by FROM cash_deposits WHERE id=?",
                    "expense" => "SELECT updated_by FROM expenses WHERE id=?",
                    "stock_transfer_request" => {
                        "SELECT approved_by FROM stock_transfer_requests WHERE id=?"
                    }
                    "stock_waste_event" => "SELECT approved_by FROM stock_waste_events WHERE id=?",
                    _ => return Ok(()),
                };
                sqlx::query_scalar::<_, Option<String>>(query)
                    .bind(id.to_string())
                    .fetch_one(&mut *c)
                    .await?
            };
            insert(
                c,
                if requested {
                    kind::APPROVAL_REQUESTED
                } else {
                    kind::APPROVAL_DECIDED
                },
                title,
                Some(if requested {
                    "Awaiting admin approval".into()
                } else {
                    format!("Status: {status}")
                }),
                payload.clone(),
                actor.clone(),
                entity,
                id,
                venue,
            )
            .await?;
            if requested {
                let extra = match entity {
                    "cash_deposit" => Some((
                        kind::CASH_DEPOSIT_INITIATED,
                        format!("Cash deposit initiated: ₹{amount:.2}"),
                        json!({"depositId":id,"amount":amount}),
                        "Pending admin approval",
                    )),
                    "stock_transfer_request" if actor.is_some() => Some((
                        kind::INVENTORY_TRANSFER_REQUESTED,
                        "Stock transfer submitted".into(),
                        payload.clone(),
                        "Awaiting admin approval",
                    )),
                    "stock_waste_event" if actor.is_some() => Some((
                        kind::INVENTORY_WASTE_RECORDED,
                        "Stock waste recorded".into(),
                        payload.clone(),
                        "Awaiting admin approval",
                    )),
                    _ => None,
                };
                if let Some((kind, title, payload, summary)) = extra {
                    insert(
                        c,
                        kind,
                        title,
                        Some(summary.into()),
                        payload,
                        actor,
                        entity,
                        id,
                        venue,
                    )
                    .await?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
