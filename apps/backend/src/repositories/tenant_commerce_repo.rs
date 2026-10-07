use std::collections::{BTreeMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    kiosk_order_status, CreateTransactionDto, CreditAccountFilterDto, CreditPlayerRow,
    CreditSettlement, CreditSettlementDetail, CreditSettlementFilterDto, CreditSettlementItemRow,
    CreditSettlementListRow, CreditSummary, KioskOrder, KioskOrderFilterDto, KioskOrderItem,
    KioskOrderWithItems, PlayerCreditDetail, PricingPolicy, PricingRule, PricingTarget,
    SetCreditLimitDto, SettleCreditDto, Transaction, TransactionFilterDto,
    TransactionProductResponse, TransactionResponse, TransactionRow, TransactionWithLineItems,
    UpdateTransactionDto,
};
use crate::services::PricingPolicyService;
use crate::tenancy::{
    decimal_to_scale4, format_sqlite_timestamp, scale4_to_decimal,
    write_outbox_event_on_connection, NewOutboxEvent, TenantDb,
};
use crate::validation::validate_online_payment_ref_last4;

type WriteOperation<T> = Box<
    dyn for<'connection> FnOnce(
            &'connection mut SqliteConnection,
        ) -> BoxFuture<'connection, Result<T, AppError>>
        + Send,
>;

async fn write<T: Send + 'static>(
    db: &TenantDb,
    operation: WriteOperation<T>,
) -> Result<T, AppError> {
    db.with_immediate_writer(operation).await
}

fn timestamp(value: &DateTime<Utc>) -> Result<String, AppError> {
    format_sqlite_timestamp(value)
        .map_err(|error| AppError::Internal(format!("format tenant timestamp: {error}")))
}

fn money(value: f64) -> Result<i64, AppError> {
    if !value.is_finite() {
        return Err(AppError::BadRequest("Money must be finite".into()));
    }
    let decimal = Decimal::from_str(&value.to_string())
        .map_err(|_| AppError::BadRequest("Invalid money value".into()))?;
    decimal_to_scale4(decimal)
        .map_err(|error| AppError::BadRequest(format!("Invalid money value: {error}")))
}

fn money_f64(value: i64) -> Result<f64, AppError> {
    scale4_to_decimal(value)
        .to_f64()
        .ok_or_else(|| AppError::Internal("Stored money is outside f64 range".into()))
}

fn parse_uuid(value: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value).map_err(|error| AppError::Internal(error.to_string()))
}

async fn event(
    connection: &mut SqliteConnection,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event_type: &str,
    location_id: Option<Uuid>,
    payload: Value,
) -> Result<(), AppError> {
    write_outbox_event_on_connection(
        connection,
        NewOutboxEvent {
            location_id,
            aggregate_type: aggregate_type.into(),
            aggregate_id,
            event_type: event_type.into(),
            schema_version: 1,
            deleted: false,
            payload,
        },
    )
    .await?;
    Ok(())
}

const TRANSACTION_SELECT: &str = r#"
 SELECT unhex(replace(id,'-','')) id,
        unhex(replace(player_id,'-','')) player_id,transaction_type,
        unhex(replace(plan_id,'-','')) plan_id,unhex(replace(shift_id,'-','')) shift_id,
        amount/10000.0 amount,paid_amount/10000.0 paid_amount,
        cash_amount/10000.0 cash_amount,online_amount/10000.0 online_amount,
        payment_method,payment_status,notes,online_payment_ref_last4,transaction_date,
        unhex(replace(created_by,'-','')) created_by,
        unhex(replace(updated_by,'-','')) updated_by,created_at,updated_at,deleted_at
 FROM transactions
"#;

const TRANSACTION_ROW_SELECT: &str = r#"
 SELECT unhex(replace(t.id,'-','')) id,
        unhex(replace(t.player_id,'-','')) player_id,t.transaction_type,
        unhex(replace(t.plan_id,'-','')) plan_id,unhex(replace(t.shift_id,'-','')) shift_id,
        t.amount/10000.0 amount,t.paid_amount/10000.0 paid_amount,
        t.cash_amount/10000.0 cash_amount,t.online_amount/10000.0 online_amount,
        t.payment_method,t.payment_status,t.notes,t.online_payment_ref_last4,t.transaction_date,
        unhex(replace(t.created_by,'-','')) created_by,
        unhex(replace(t.updated_by,'-','')) updated_by,t.created_at,t.updated_at,t.deleted_at,
        u.username player_username,u.first_name player_first_name,u.last_name player_last_name,
        p.name plan_name,p.plan_type,p.price/10000.0 plan_price
 FROM transactions t
 LEFT JOIN users u ON u.id=t.player_id AND u.deleted_at IS NULL
 LEFT JOIN plans p ON p.id=t.plan_id AND p.deleted_at IS NULL
"#;

#[derive(Clone)]
pub struct TenantTransactionRepository {
    db: Arc<TenantDb>,
    timezone: String,
}

impl TenantTransactionRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self {
            db,
            timezone: "UTC".into(),
        }
    }

    pub fn with_timezone(mut self, timezone: String) -> Self {
        self.timezone = timezone;
        self
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Transaction>, AppError> {
        Ok(sqlx::query_as(&format!(
            "{TRANSACTION_SELECT} WHERE id=? AND deleted_at IS NULL"
        ))
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }

    pub async fn get_with_items(&self, id: Uuid) -> Result<TransactionWithLineItems, AppError> {
        let transaction = self
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Transaction with ID {id} not found")))?;
        let line_items = self.list_line_items(id).await?;
        Ok(TransactionWithLineItems::from_parts(
            transaction,
            line_items,
        ))
    }

    pub async fn list_line_items(
        &self,
        transaction_id: Uuid,
    ) -> Result<Vec<TransactionProductResponse>, AppError> {
        #[derive(FromRow)]
        struct LineRow {
            id: Uuid,
            transaction_id: Uuid,
            product_id: Uuid,
            quantity: i32,
            unit_price: f64,
            product_name: String,
            product_sku: Option<String>,
            product_price: f64,
            option_names: String,
            created_at: DateTime<Utc>,
        }
        let rows: Vec<LineRow> = sqlx::query_as(
            r#"SELECT unhex(replace(tp.id,'-','')) id,
                      unhex(replace(tp.transaction_id,'-','')) transaction_id,
                      unhex(replace(tp.product_id,'-','')) product_id,tp.quantity,
                      tp.unit_price/10000.0 unit_price,tp.product_name,tp.product_sku,
                      tp.unit_price/10000.0 product_price,
                      COALESCE((SELECT json_group_array(name) FROM (
                        SELECT name FROM transaction_product_options o
                        WHERE o.transaction_product_id=tp.id ORDER BY o.id
                      )),'[]') option_names,tp.created_at
               FROM transaction_products tp WHERE tp.transaction_id=?
               ORDER BY tp.created_at,tp.id"#,
        )
        .bind(transaction_id.to_string())
        .fetch_all(&self.db.read_pool()?)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(TransactionProductResponse {
                    id: row.id,
                    transaction_id: row.transaction_id,
                    product_id: row.product_id,
                    quantity: row.quantity,
                    unit_price: row.unit_price,
                    product_name: row.product_name,
                    product_sku: row.product_sku,
                    product_price: row.product_price,
                    option_names: serde_json::from_str(&row.option_names).map_err(|error| {
                        AppError::Internal(format!("Invalid option snapshot JSON: {error}"))
                    })?,
                    created_at: row.created_at,
                })
            })
            .collect()
    }

    pub async fn list(
        &self,
        filters: &TransactionFilterDto,
    ) -> Result<PaginationResult<TransactionResponse>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let mut query = QueryBuilder::<Sqlite>::new(format!(
            "{TRANSACTION_ROW_SELECT} WHERE t.deleted_at IS NULL"
        ));
        transaction_filters(&mut query, filters)?;
        let sort = match filters.sort_by.as_deref() {
            Some("amount") => "t.amount",
            Some("paymentStatus") => "t.payment_status",
            Some("createdAt") => "t.created_at",
            _ => "t.transaction_date",
        };
        let direction = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        query
            .push(format!(
                " ORDER BY {sort} {direction},t.id {direction} LIMIT "
            ))
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1) * limit);
        let rows: Vec<TransactionRow> = query
            .build_query_as()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*) FROM transactions t WHERE t.deleted_at IS NULL",
        );
        transaction_filters(&mut count, filters)?;
        let total = count
            .build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(
            rows.into_iter()
                .map(TransactionRow::into_response)
                .collect(),
            total,
            page,
            limit,
        ))
    }

    pub async fn create(
        &self,
        dto: CreateTransactionDto,
        actor: Option<Uuid>,
    ) -> Result<Transaction, AppError> {
        match dto.transaction_type.as_str() {
            "product_purchase" => self.create_product_purchase(dto, actor).await,
            "plan_purchase" => self.create_plan_purchase(dto, actor).await,
            _ => Err(AppError::BadRequest("Invalid transaction type".into())),
        }
    }

    async fn create_plan_purchase(
        &self,
        dto: CreateTransactionDto,
        actor: Option<Uuid>,
    ) -> Result<Transaction, AppError> {
        let id = Uuid::now_v7();
        let at = dto.transaction_date.unwrap_or_else(Utc::now);
        let at_text = timestamp(&at)?;
        let plan_id = dto
            .plan_id
            .ok_or_else(|| AppError::BadRequest("planId is required".into()))?;
        let venue = dto
            .venue_location_id
            .ok_or_else(|| AppError::BadRequest("venueLocationId is required".into()))?;
        let timezone = self.timezone.clone();
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let plan: Option<(i64, Option<String>)> = sqlx::query_as(
                        "SELECT COALESCE(pl.price,p.price),p.device_type FROM plans p
                         LEFT JOIN plan_locations pl ON pl.plan_id=p.id AND pl.location_id=?
                         WHERE p.id=? AND p.is_active=1 AND p.deleted_at IS NULL
                           AND (p.availability_scope='ALL' OR pl.plan_id IS NOT NULL)",
                    )
                    .bind(venue.to_string())
                    .bind(plan_id.to_string())
                    .fetch_optional(&mut *connection)
                    .await?;
                    let (base_amount, device_type) = plan
                        .ok_or_else(|| AppError::NotFound(format!("Plan with ID {plan_id} not found")))?;
                    let policy = plan_pricing_context(connection, venue).await?;
                    let amount = money(PricingPolicyService::evaluate_plan_price(
                        money_f64(base_amount)?,
                        device_type.as_deref(),
                        &policy,
                        at,
                        &timezone,
                    )?)?;
                    if dto.amount.map(money).transpose()?.is_some_and(|quoted| quoted != amount) {
                        return Err(AppError::Conflict(
                            "Plan price changed. Refresh the plan and review the new total before paying."
                                .into(),
                        ));
                    }
                    let payment = validated_payment(&dto, amount)?;
                    validate_player_and_shift(connection, dto.player_id, dto.shift_id, venue).await?;
                    if payment.method == "credit" {
                        ensure_credit_headroom(connection, dto.player_id, amount).await?;
                    }
                    insert_transaction(
                        connection, id, &dto, venue, amount, &payment, &at_text, actor,
                    )
                    .await?;
                    if payment.status == "completed" || payment.method == "credit" {
                        super::tenant_venue_repo::grant_balance_on_connection(
                            connection,
                            dto.player_id,
                            plan_id,
                            id,
                            actor,
                            &at_text,
                        )
                        .await?;
                    }
                    event(
                        connection,
                        "transaction",
                        id,
                        "transaction.created",
                        Some(venue),
                        json!({"id":id,"playerId":dto.player_id,"planId":plan_id,
                               "amount":money_f64(amount)?,"paymentStatus":payment.status,
                               "updatedAt":at_text}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.required(id).await
    }

    async fn create_product_purchase(
        &self,
        mut dto: CreateTransactionDto,
        actor: Option<Uuid>,
    ) -> Result<Transaction, AppError> {
        let id = Uuid::now_v7();
        let at = dto.transaction_date.unwrap_or_else(Utc::now);
        let at_text = timestamp(&at)?;
        let venue = dto
            .venue_location_id
            .ok_or_else(|| AppError::BadRequest("venueLocationId is required".into()))?;
        let store = dto
            .sale_location_id
            .ok_or_else(|| AppError::BadRequest("saleLocationId is required".into()))?;
        let lines = dto
            .line_items
            .take()
            .filter(|items| !items.is_empty())
            .ok_or_else(|| AppError::BadRequest("lineItems must not be empty".into()))?;
        let timezone = self.timezone.clone();
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    validate_store(connection, store, venue).await?;
                    validate_player_and_shift(connection, dto.player_id, dto.shift_id, venue).await?;
                    let pricing = pricing_context(connection, venue, timezone).await?;
                    let kiosk_snapshots = if let Some(order_id) = dto.kiosk_order_id {
                        Some(
                            load_kiosk_conversion(
                                connection,
                                order_id,
                                dto.player_id,
                                venue,
                                &lines,
                            )
                            .await?,
                        )
                    } else {
                        None
                    };
                    let resolved = resolve_sale(
                        connection,
                        &lines,
                        venue,
                        store,
                        at,
                        &pricing,
                        kiosk_snapshots.as_deref(),
                    )
                    .await?;
                    let payment = validated_payment(&dto, resolved.total)?;
                    if payment.status == "pending" {
                        return Err(AppError::BadRequest(
                            "Product purchases must be completed or use credit".into(),
                        ));
                    }
                    if payment.method == "credit" {
                        ensure_credit_headroom(connection, dto.player_id, resolved.total).await?;
                    }
                    insert_transaction(
                        connection,
                        id,
                        &dto,
                        venue,
                        resolved.total,
                        &payment,
                        &at_text,
                        actor,
                    )
                    .await?;
                    insert_sale_lines(connection, id, &resolved.lines, &at_text).await?;
                    deduct_stock(
                        connection,
                        store,
                        id,
                        &resolved.stock,
                        actor,
                        &at_text,
                    )
                    .await?;
                    if let Some(order_id) = dto.kiosk_order_id {
                        let changed = sqlx::query(
                            "UPDATE kiosk_orders SET status='fulfilled',transaction_id=?,
                             fulfilled_at=?,updated_at=? WHERE id=? AND status IN ('pending','preparing')",
                        )
                        .bind(id.to_string())
                        .bind(&at_text)
                        .bind(&at_text)
                        .bind(order_id.to_string())
                        .execute(&mut *connection)
                        .await?;
                        if changed.rows_affected() != 1 {
                            return Err(AppError::Conflict(
                                "Kiosk order is no longer open for conversion".into(),
                            ));
                        }
                        event(
                            connection,
                            "kiosk_order",
                            order_id,
                            "kiosk_order.fulfilled",
                            Some(venue),
                            json!({"id":order_id,"transactionId":id,"updatedAt":at_text}),
                        )
                        .await?;
                    }
                    event(
                        connection,
                        "transaction",
                        id,
                        "transaction.created",
                        Some(venue),
                        json!({"id":id,"playerId":dto.player_id,"amount":money_f64(resolved.total)?,
                               "paymentStatus":payment.status,"inventoryLocationId":store,
                               "updatedAt":at_text}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.required(id).await
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateTransactionDto,
        actor: Option<Uuid>,
    ) -> Result<Transaction, AppError> {
        let status = dto.payment_status.clone();
        let notes = dto.notes.clone();
        if status.is_none() && notes.is_none() {
            return Err(AppError::BadRequest(
                "At least one of paymentStatus or notes must be provided".into(),
            ));
        }
        if status.as_deref().is_some_and(|value| {
            !matches!(
                value,
                "pending" | "completed" | "failed" | "refunded" | "credit"
            )
        }) {
            return Err(AppError::BadRequest("Invalid payment status".into()));
        }
        let at = timestamp(&Utc::now())?;
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let row: Option<(
                        String,
                        String,
                        Option<String>,
                        String,
                        String,
                        i64,
                        String,
                        i64,
                        i64,
                    )> = sqlx::query_as(
                        "SELECT payment_status,transaction_type,plan_id,player_id,location_id,
                                    amount,payment_method,cash_amount,online_amount
                             FROM transactions WHERE id=? AND deleted_at IS NULL",
                    )
                    .bind(id.to_string())
                    .fetch_optional(&mut *connection)
                    .await?;
                    let Some((
                        old,
                        kind,
                        plan,
                        player,
                        location,
                        amount,
                        method,
                        old_cash,
                        old_online,
                    )) = row
                    else {
                        return Err(AppError::NotFound(format!(
                            "Transaction with ID {id} not found"
                        )));
                    };
                    let next = status.as_deref().unwrap_or(&old);
                    validate_status_transition(&old, next, &method)?;
                    let (cash, online) = if old != "completed" && next == "completed" {
                        match method.as_str() {
                            "cash" => (amount, 0),
                            "online" => (0, amount),
                            "split_payment" if old_cash.checked_add(old_online) == Some(amount) => {
                                (old_cash, old_online)
                            }
                            _ => {
                                return Err(AppError::Conflict(
                                    "Completed transaction tender must equal amount".into(),
                                ));
                            }
                        }
                    } else {
                        (old_cash, old_online)
                    };
                    if next == "completed" && cash.checked_add(online) != Some(amount) {
                        return Err(AppError::Conflict(
                            "Completed transaction tender must equal amount".into(),
                        ));
                    }
                    let paid = if next == "completed" { amount } else { 0 };
                    sqlx::query(
                        "UPDATE transactions SET payment_status=?,paid_amount=?,
                         cash_amount=?,online_amount=?,
                         notes=COALESCE(?,notes),updated_by=COALESCE(?,updated_by),updated_at=?
                         WHERE id=?",
                    )
                    .bind(next)
                    .bind(paid)
                    .bind(cash)
                    .bind(online)
                    .bind(notes)
                    .bind(actor.map(|value| value.to_string()))
                    .bind(&at)
                    .bind(id.to_string())
                    .execute(&mut *connection)
                    .await?;
                    if old != "completed" && next == "completed" && kind == "plan_purchase" {
                        let plan = plan.ok_or_else(|| {
                            AppError::Internal("Plan purchase has no plan".into())
                        })?;
                        super::tenant_venue_repo::grant_balance_on_connection(
                            connection,
                            parse_uuid(&player)?,
                            parse_uuid(&plan)?,
                            id,
                            actor,
                            &at,
                        )
                        .await?;
                    }
                    let location = parse_uuid(&location)?;
                    event(
                        connection,
                        "transaction",
                        id,
                        "transaction.updated",
                        Some(location),
                        json!({"id":id,"paymentStatus":next,"updatedAt":at}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.required(id).await
    }

    async fn required(&self, id: Uuid) -> Result<Transaction, AppError> {
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("Transaction disappeared".into()))
    }
}

fn transaction_filters<'a>(
    query: &mut QueryBuilder<'a, Sqlite>,
    filters: &'a TransactionFilterDto,
) -> Result<(), AppError> {
    for (column, value) in [
        ("t.player_id", filters.player_id),
        ("t.plan_id", filters.plan_id),
        ("t.shift_id", filters.shift_id),
    ] {
        if let Some(value) = value {
            query
                .push(format!(" AND {column}="))
                .push_bind(value.to_string());
        }
    }
    for (column, value) in [
        ("t.transaction_type", filters.transaction_type.as_ref()),
        ("t.payment_method", filters.payment_method.as_ref()),
        ("t.payment_status", filters.payment_status.as_ref()),
    ] {
        if let Some(value) = value {
            query.push(format!(" AND {column}=")).push_bind(value);
        }
    }
    if let Some(value) = filters.transaction_date_from {
        query
            .push(" AND t.transaction_date>=")
            .push_bind(timestamp(&value)?);
    }
    if let Some(value) = filters.transaction_date_to {
        query
            .push(" AND t.transaction_date<=")
            .push_bind(timestamp(&value)?);
    }
    if let Some(value) = filters.min_amount {
        query.push(" AND t.amount>=").push_bind(money(value)?);
    }
    if let Some(value) = filters.max_amount {
        query.push(" AND t.amount<=").push_bind(money(value)?);
    }
    Ok(())
}

#[derive(Clone)]
struct Payment<'a> {
    method: &'a str,
    status: &'a str,
    paid: i64,
    cash: i64,
    online: i64,
    online_ref: Option<String>,
}

fn validated_payment<'a>(
    dto: &'a CreateTransactionDto,
    amount: i64,
) -> Result<Payment<'a>, AppError> {
    let method = dto.payment_method.as_str();
    if !matches!(method, "cash" | "online" | "split_payment" | "credit") {
        return Err(AppError::BadRequest("Invalid payment method".into()));
    }
    let requested_status = dto.payment_status.as_deref();
    let status = match (method, requested_status) {
        ("credit", None | Some("credit")) => "credit",
        ("credit", Some(_)) => {
            return Err(AppError::BadRequest(
                "Credit payment method requires credit status".into(),
            ))
        }
        (_, Some("credit")) => {
            return Err(AppError::BadRequest(
                "Credit status requires credit payment method".into(),
            ))
        }
        (_, Some("failed" | "refunded")) => {
            return Err(AppError::BadRequest(
                "New transactions cannot start failed or refunded".into(),
            ))
        }
        (_, Some("pending")) => "pending",
        (_, Some("completed")) => "completed",
        (_, None) => "pending",
        _ => return Err(AppError::BadRequest("Invalid payment status".into())),
    };
    if method == "credit"
        && (dto.cash_amount.map(money).transpose()?.unwrap_or(0) != 0
            || dto.online_amount.map(money).transpose()?.unwrap_or(0) != 0)
    {
        return Err(AppError::BadRequest(
            "Credit transactions cannot include tender amounts".into(),
        ));
    }
    let default_tender = if status == "completed" { amount } else { 0 };
    let (cash, online) = match method {
        "cash" => (
            dto.cash_amount
                .map(money)
                .transpose()?
                .unwrap_or(default_tender),
            0,
        ),
        "online" => (
            0,
            dto.online_amount
                .map(money)
                .transpose()?
                .unwrap_or(default_tender),
        ),
        "split_payment" => (
            dto.cash_amount.map(money).transpose()?.unwrap_or(0),
            dto.online_amount.map(money).transpose()?.unwrap_or(0),
        ),
        _ => (0, 0),
    };
    let paid = if method == "credit" {
        0
    } else if status == "completed" {
        amount
    } else {
        0
    };
    if status == "completed" && cash.checked_add(online) != Some(amount) {
        return Err(AppError::Conflict(
            "Completed transaction tender must equal amount".into(),
        ));
    }
    if status != "completed" && cash.checked_add(online).unwrap_or(i64::MAX) > paid {
        return Err(AppError::Conflict(
            "Pending transaction tender cannot exceed paid amount".into(),
        ));
    }
    let online_ref = validate_online_payment_ref_last4(
        method,
        Some(money_f64(online)?),
        dto.online_payment_ref_last4.clone(),
    )?;
    Ok(Payment {
        method,
        status,
        paid,
        cash,
        online,
        online_ref,
    })
}

fn validate_status_transition(old: &str, next: &str, method: &str) -> Result<(), AppError> {
    if old == next {
        return Ok(());
    }
    if old == "pending" && next == "completed" && method != "credit" {
        return Ok(());
    }
    Err(AppError::Conflict(format!(
        "Payment status cannot transition from {old} to {next}"
    )))
}

#[allow(clippy::too_many_arguments)]
async fn insert_transaction(
    connection: &mut SqliteConnection,
    id: Uuid,
    dto: &CreateTransactionDto,
    venue: Uuid,
    amount: i64,
    payment: &Payment<'_>,
    at: &str,
    actor: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO transactions(id,player_id,plan_id,shift_id,location_id,transaction_type,
         amount,paid_amount,cash_amount,online_amount,payment_method,payment_status,notes,
         online_payment_ref_last4,transaction_date,created_by,updated_by,created_at,updated_at)
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(id.to_string())
    .bind(dto.player_id.to_string())
    .bind(dto.plan_id.map(|value| value.to_string()))
    .bind(dto.shift_id.map(|value| value.to_string()))
    .bind(venue.to_string())
    .bind(&dto.transaction_type)
    .bind(amount)
    .bind(payment.paid)
    .bind(payment.cash)
    .bind(payment.online)
    .bind(payment.method)
    .bind(payment.status)
    .bind(&dto.notes)
    .bind(&payment.online_ref)
    .bind(at)
    .bind(actor.map(|value| value.to_string()))
    .bind(actor.map(|value| value.to_string()))
    .bind(at)
    .bind(at)
    .execute(connection)
    .await?;
    Ok(())
}

async fn validate_store(
    connection: &mut SqliteConnection,
    store: Uuid,
    venue: Uuid,
) -> Result<(), AppError> {
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM inventory_locations
         WHERE id=? AND venue_location_id=? AND kind='store' AND is_active=1 AND deleted_at IS NULL)",
    )
    .bind(store.to_string())
    .bind(venue.to_string())
    .fetch_one(connection)
    .await?;
    if !valid {
        return Err(AppError::BadRequest(
            "saleLocationId must reference an active store for the selected venue".into(),
        ));
    }
    Ok(())
}

async fn validate_player_and_shift(
    connection: &mut SqliteConnection,
    player: Uuid,
    shift: Option<Uuid>,
    venue: Uuid,
) -> Result<(), AppError> {
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users
         WHERE id=? AND role='player' AND is_active=1 AND deleted_at IS NULL)",
    )
    .bind(player.to_string())
    .fetch_one(&mut *connection)
    .await?;
    if !active {
        return Err(AppError::BadRequest("Player is not active".into()));
    }
    if let Some(shift) = shift {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM shifts
             WHERE id=? AND location_id=? AND status='active')",
        )
        .bind(shift.to_string())
        .bind(venue.to_string())
        .fetch_one(connection)
        .await?;
        if !valid {
            return Err(AppError::BadRequest(
                "Shift is not active at the selected venue".into(),
            ));
        }
    }
    Ok(())
}

struct PricingContext {
    timezone: String,
    night_start: String,
    night_end: String,
    rules: Vec<PricingRule>,
}

async fn pricing_context(
    connection: &mut SqliteConnection,
    venue: Uuid,
    timezone: String,
) -> Result<PricingContext, AppError> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT v.policy FROM pricing_rule_sets s
         JOIN pricing_rule_versions v ON v.id=s.active_version_id
         WHERE v.status='published' AND (s.availability_scope='ALL' OR EXISTS(
           SELECT 1 FROM pricing_rule_set_locations l
           WHERE l.rule_set_id=s.id AND l.location_id=?))
         ORDER BY s.id",
    )
    .bind(venue.to_string())
    .fetch_all(&mut *connection)
    .await?;
    let mut rules = Vec::new();
    for row in rows {
        let policy: PricingPolicy = serde_json::from_str(&row)
            .map_err(|error| AppError::Internal(format!("Invalid published policy: {error}")))?;
        rules.extend(
            policy
                .rules
                .into_iter()
                .filter(|rule| rule.target == PricingTarget::Products),
        );
    }
    let night_start = pricing_setting(connection, venue, "pricing.night_window_start")
        .await?
        .and_then(|value| serde_json::from_str::<String>(&value).ok())
        .unwrap_or_else(|| "23:00".into());
    let night_end = pricing_setting(connection, venue, "pricing.night_window_end")
        .await?
        .and_then(|value| serde_json::from_str::<String>(&value).ok())
        .unwrap_or_else(|| "08:00".into());
    Ok(PricingContext {
        timezone,
        night_start,
        night_end,
        rules,
    })
}

async fn plan_pricing_context(
    connection: &mut SqliteConnection,
    venue: Uuid,
) -> Result<PricingPolicy, AppError> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT v.policy FROM pricing_rule_sets s
         JOIN pricing_rule_versions v ON v.id=s.active_version_id
         WHERE v.status='published' AND (s.availability_scope='ALL' OR EXISTS(
           SELECT 1 FROM pricing_rule_set_locations l
           WHERE l.rule_set_id=s.id AND l.location_id=?))
         ORDER BY s.id",
    )
    .bind(venue.to_string())
    .fetch_all(&mut *connection)
    .await?;
    let mut combined = PricingPolicy {
        base_rate: "0".into(),
        rules: Vec::new(),
        rounding_scale: 2,
        minimum_price: None,
        maximum_price: None,
    };
    let mut found = false;
    for row in rows {
        let policy: PricingPolicy = serde_json::from_str(&row)
            .map_err(|error| AppError::Internal(format!("Invalid published policy: {error}")))?;
        let rules: Vec<_> = policy
            .rules
            .into_iter()
            .filter(|rule| rule.target == PricingTarget::Sessions)
            .collect();
        if rules.is_empty() {
            continue;
        }
        combined.rounding_scale = if found {
            combined.rounding_scale.min(policy.rounding_scale)
        } else {
            policy.rounding_scale
        };
        found = true;
        combined.rules.extend(rules);
        combined.minimum_price =
            stricter_price_bound(combined.minimum_price, policy.minimum_price, Decimal::max)?;
        combined.maximum_price =
            stricter_price_bound(combined.maximum_price, policy.maximum_price, Decimal::min)?;
    }
    Ok(combined)
}

fn stricter_price_bound(
    left: Option<String>,
    right: Option<String>,
    choose: fn(Decimal, Decimal) -> Decimal,
) -> Result<Option<String>, AppError> {
    match (left, right) {
        (Some(left), Some(right)) => Ok(Some(
            choose(
                Decimal::from_str(&left)
                    .map_err(|_| AppError::Internal("Invalid pricing bound".into()))?,
                Decimal::from_str(&right)
                    .map_err(|_| AppError::Internal("Invalid pricing bound".into()))?,
            )
            .to_string(),
        )),
        (left @ Some(_), None) => Ok(left),
        (None, right) => Ok(right),
    }
}

async fn pricing_setting(
    connection: &mut SqliteConnection,
    venue: Uuid,
    key: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT value FROM setting_overrides
         WHERE key=? AND (location_id=? OR location_id IS NULL)
         ORDER BY location_id IS NOT NULL DESC LIMIT 1",
    )
    .bind(key)
    .bind(venue.to_string())
    .fetch_optional(connection)
    .await
}

struct ResolvedOption {
    id: Uuid,
    group_name: String,
    name: String,
    price_delta: i64,
}

struct ResolvedLine {
    product_id: Uuid,
    product_name: String,
    product_sku: Option<String>,
    quantity: i32,
    unit_price: i64,
    options: Vec<ResolvedOption>,
}

struct ResolvedSale {
    lines: Vec<ResolvedLine>,
    stock: BTreeMap<Uuid, i32>,
    total: i64,
}

#[derive(Clone)]
struct KioskSnapshotLine {
    product_id: Uuid,
    product_name: String,
    product_sku: Option<String>,
    quantity: i32,
    unit_price: i64,
}

async fn resolve_sale(
    connection: &mut SqliteConnection,
    items: &[crate::models::CreateLineItemDto],
    venue: Uuid,
    store: Uuid,
    at: DateTime<Utc>,
    pricing: &PricingContext,
    kiosk_snapshots: Option<&[KioskSnapshotLine]>,
) -> Result<ResolvedSale, AppError> {
    let mut lines = Vec::with_capacity(items.len());
    let mut stock = BTreeMap::<Uuid, i32>::new();
    let mut total = 0_i64;
    let mut unused_snapshots = kiosk_snapshots.map(<[KioskSnapshotLine]>::to_vec);
    for item in items {
        if item.quantity <= 0 {
            return Err(AppError::BadRequest(
                "Product quantity must be positive".into(),
            ));
        }
        let row: Option<(String, Option<String>, i64, i64, String, bool)> = sqlx::query_as(
            "SELECT p.name,p.sku,COALESCE(pl.price,p.day_price),
                    COALESCE(pl.price,p.night_price),p.category,p.is_raw_material
             FROM products p
             LEFT JOIN product_locations pl ON pl.product_id=p.id AND pl.location_id=?
             WHERE p.id=? AND p.is_active=1 AND p.deleted_at IS NULL
               AND (p.availability_scope='ALL' OR pl.product_id IS NOT NULL)",
        )
        .bind(venue.to_string())
        .bind(item.product_id.to_string())
        .fetch_optional(&mut *connection)
        .await?;
        let Some((name, sku, day, night, category, raw)) = row else {
            return Err(AppError::NotFound(format!(
                "Product {} not found",
                item.product_id
            )));
        };
        if raw {
            return Err(AppError::BadRequest(format!(
                "{name} is a raw material and cannot be sold"
            )));
        }
        let groups: Vec<(String, String, bool, bool)> = sqlx::query_as(
            "SELECT id,name,required,multiple FROM product_option_groups
             WHERE product_id=? ORDER BY sort_order,id",
        )
        .bind(item.product_id.to_string())
        .fetch_all(&mut *connection)
        .await?;
        let selected: HashSet<Uuid> = item.option_ids.iter().copied().collect();
        if selected.len() != item.option_ids.len() {
            return Err(AppError::BadRequest("Duplicate product option".into()));
        }
        let mut options = Vec::new();
        let mut option_ingredients = BTreeMap::<Uuid, i32>::new();
        for (group_id, group_name, required, multiple) in groups {
            let rows: Vec<(String, String, i64)> = sqlx::query_as(
                "SELECT id,name,price_delta FROM product_options WHERE group_id=? ORDER BY sort_order,id",
            )
            .bind(&group_id)
            .fetch_all(&mut *connection)
            .await?;
            let chosen: Vec<_> = rows
                .into_iter()
                .filter(|(id, _, _)| parse_uuid(id).is_ok_and(|id| selected.contains(&id)))
                .collect();
            if required && chosen.is_empty() {
                return Err(AppError::BadRequest(format!(
                    "An option is required for {group_name}"
                )));
            }
            if !multiple && chosen.len() > 1 {
                return Err(AppError::BadRequest(format!(
                    "Only one option may be selected for {group_name}"
                )));
            }
            for (option_id, option_name, delta) in chosen {
                let option_id = parse_uuid(&option_id)?;
                let ingredients: Vec<(String, i32)> = sqlx::query_as(
                    "SELECT ingredient_id,quantity FROM product_option_ingredients WHERE option_id=?",
                )
                .bind(option_id.to_string())
                .fetch_all(&mut *connection)
                .await?;
                for (ingredient, quantity) in ingredients {
                    let entry = option_ingredients
                        .entry(parse_uuid(&ingredient)?)
                        .or_default();
                    *entry = entry.checked_add(quantity).ok_or_else(|| {
                        AppError::BadRequest("Ingredient quantity overflow".into())
                    })?;
                }
                options.push(ResolvedOption {
                    id: option_id,
                    group_name: group_name.clone(),
                    name: option_name,
                    price_delta: delta,
                });
            }
        }
        if options.len() != selected.len() {
            return Err(AppError::BadRequest(
                "Selected option does not belong to the product".into(),
            ));
        }
        let recipes: Vec<(String, i32)> = sqlx::query_as(
            "SELECT ingredient_id,quantity FROM product_recipe_items WHERE product_id=?",
        )
        .bind(item.product_id.to_string())
        .fetch_all(&mut *connection)
        .await?;
        if recipes.is_empty() {
            add_stock(&mut stock, item.product_id, item.quantity)?;
            for (ingredient, quantity) in option_ingredients {
                if quantity < 0 {
                    return Err(AppError::BadRequest(
                        "Option adjustments cannot create a negative ingredient requirement".into(),
                    ));
                }
                if quantity > 0 {
                    add_stock(
                        &mut stock,
                        ingredient,
                        quantity.checked_mul(item.quantity).ok_or_else(|| {
                            AppError::BadRequest("Ingredient quantity overflow".into())
                        })?,
                    )?;
                }
            }
        } else {
            let mut per_unit = BTreeMap::<Uuid, i32>::new();
            for (ingredient, quantity) in recipes {
                per_unit.insert(parse_uuid(&ingredient)?, quantity);
            }
            for (ingredient, adjustment) in option_ingredients {
                let entry = per_unit.entry(ingredient).or_default();
                *entry = entry
                    .checked_add(adjustment)
                    .ok_or_else(|| AppError::BadRequest("Ingredient quantity overflow".into()))?;
            }
            for (ingredient, quantity) in per_unit {
                if quantity < 0 {
                    return Err(AppError::BadRequest(
                        "Option adjustments cannot make an ingredient requirement negative".into(),
                    ));
                }
                if quantity > 0 {
                    add_stock(
                        &mut stock,
                        ingredient,
                        quantity.checked_mul(item.quantity).ok_or_else(|| {
                            AppError::BadRequest("Ingredient quantity overflow".into())
                        })?,
                    )?;
                }
            }
        }
        let (product_name, product_sku, unit_price) =
            if let Some(snapshots) = unused_snapshots.as_mut() {
                let index = snapshots
                    .iter()
                    .position(|snapshot| {
                        snapshot.product_id == item.product_id && snapshot.quantity == item.quantity
                    })
                    .ok_or_else(|| {
                        AppError::Conflict("Kiosk order items changed before conversion".into())
                    })?;
                let snapshot = snapshots.remove(index);
                (
                    snapshot.product_name,
                    snapshot.product_sku,
                    snapshot.unit_price,
                )
            } else {
                let base = PricingPolicyService::evaluate_product_price(
                    money_f64(day)?,
                    money_f64(night)?,
                    item.product_id,
                    &category,
                    &pricing.rules,
                    at,
                    &pricing.timezone,
                    &pricing.night_start,
                    &pricing.night_end,
                )?;
                let base = money(base)?;
                let option_delta = options.iter().try_fold(0_i64, |sum, option| {
                    sum.checked_add(option.price_delta)
                        .ok_or_else(|| AppError::BadRequest("Price overflow".into()))
                })?;
                (
                    name,
                    sku,
                    base.checked_add(option_delta)
                        .ok_or_else(|| AppError::BadRequest("Price overflow".into()))?
                        .max(0),
                )
            };
        total = total
            .checked_add(
                unit_price
                    .checked_mul(i64::from(item.quantity))
                    .ok_or_else(|| AppError::BadRequest("Transaction total overflow".into()))?,
            )
            .ok_or_else(|| AppError::BadRequest("Transaction total overflow".into()))?;
        lines.push(ResolvedLine {
            product_id: item.product_id,
            product_name,
            product_sku,
            quantity: item.quantity,
            unit_price,
            options,
        });
    }
    if unused_snapshots
        .as_ref()
        .is_some_and(|snapshots| !snapshots.is_empty())
    {
        return Err(AppError::Conflict(
            "Kiosk order items changed before conversion".into(),
        ));
    }
    for (product, needed) in &stock {
        let available: Option<i32> = sqlx::query_scalar(
            "SELECT quantity_pieces FROM location_stock
             WHERE inventory_location_id=? AND product_id=?",
        )
        .bind(store.to_string())
        .bind(product.to_string())
        .fetch_optional(&mut *connection)
        .await?;
        if available.unwrap_or(0) < *needed {
            return Err(AppError::Conflict(format!(
                "Insufficient stock for product {product}"
            )));
        }
    }
    Ok(ResolvedSale {
        lines,
        stock,
        total,
    })
}

fn add_stock(
    stock: &mut BTreeMap<Uuid, i32>,
    product: Uuid,
    quantity: i32,
) -> Result<(), AppError> {
    let entry = stock.entry(product).or_default();
    *entry = entry
        .checked_add(quantity)
        .ok_or_else(|| AppError::BadRequest("Stock quantity overflow".into()))?;
    Ok(())
}

async fn insert_sale_lines(
    connection: &mut SqliteConnection,
    transaction: Uuid,
    lines: &[ResolvedLine],
    at: &str,
) -> Result<(), AppError> {
    for line in lines {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO transaction_products(id,transaction_id,product_id,product_name,
             product_sku,quantity,unit_price,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)",
        )
        .bind(id.to_string())
        .bind(transaction.to_string())
        .bind(line.product_id.to_string())
        .bind(&line.product_name)
        .bind(&line.product_sku)
        .bind(line.quantity)
        .bind(line.unit_price)
        .bind(at)
        .bind(at)
        .execute(&mut *connection)
        .await?;
        for option in &line.options {
            sqlx::query(
                "INSERT INTO transaction_product_options(id,transaction_product_id,option_id,
                 group_name,name,price_delta) VALUES(?,?,?,?,?,?)",
            )
            .bind(Uuid::now_v7().to_string())
            .bind(id.to_string())
            .bind(option.id.to_string())
            .bind(&option.group_name)
            .bind(&option.name)
            .bind(option.price_delta)
            .execute(&mut *connection)
            .await?;
        }
    }
    Ok(())
}

async fn deduct_stock(
    connection: &mut SqliteConnection,
    store: Uuid,
    transaction: Uuid,
    stock: &BTreeMap<Uuid, i32>,
    actor: Option<Uuid>,
    at: &str,
) -> Result<(), AppError> {
    for (product, quantity) in stock {
        let changed = sqlx::query(
            "UPDATE location_stock SET quantity_pieces=quantity_pieces-?,updated_at=?
             WHERE inventory_location_id=? AND product_id=? AND quantity_pieces>=?",
        )
        .bind(quantity)
        .bind(at)
        .bind(store.to_string())
        .bind(product.to_string())
        .bind(quantity)
        .execute(&mut *connection)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(AppError::Conflict(format!(
                "Insufficient stock for product {product}"
            )));
        }
        sqlx::query(
            "INSERT INTO stock_movements(id,inventory_location_id,product_id,delta,movement_type,
             reference_id,reference_type,created_by,created_at)
             VALUES(?,?,?,?,'sale',?,'transaction',?,?)",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(store.to_string())
        .bind(product.to_string())
        .bind(-quantity)
        .bind(transaction.to_string())
        .bind(actor.map(|value| value.to_string()))
        .bind(at)
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

async fn ensure_credit_headroom(
    connection: &mut SqliteConnection,
    player: Uuid,
    amount: i64,
) -> Result<(), AppError> {
    let row: Option<(i64, bool, String)> = sqlx::query_as(
        "SELECT credit_limit,is_active,role FROM users WHERE id=? AND deleted_at IS NULL",
    )
    .bind(player.to_string())
    .fetch_optional(&mut *connection)
    .await?;
    let Some((limit, active, role)) = row else {
        return Err(AppError::BadRequest(
            "player not eligible for credit".into(),
        ));
    };
    if !active || role != "player" || limit <= 0 {
        return Err(AppError::Conflict(
            "credit not enabled for this player".into(),
        ));
    }
    let outstanding: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount-paid_amount),0) FROM transactions
         WHERE player_id=? AND payment_method='credit' AND payment_status='credit'
           AND deleted_at IS NULL",
    )
    .bind(player.to_string())
    .fetch_one(connection)
    .await?;
    if amount > limit.saturating_sub(outstanding) {
        return Err(AppError::Conflict("credit limit exceeded".into()));
    }
    Ok(())
}

async fn load_kiosk_conversion(
    connection: &mut SqliteConnection,
    order: Uuid,
    player: Uuid,
    venue: Uuid,
    lines: &[crate::models::CreateLineItemDto],
) -> Result<Vec<KioskSnapshotLine>, AppError> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT ko.player_id,s.location_id FROM kiosk_orders ko
         JOIN usage_sessions s ON s.id=ko.session_id
         WHERE ko.id=? AND ko.status IN ('pending','preparing') AND ko.transaction_id IS NULL",
    )
    .bind(order.to_string())
    .fetch_optional(&mut *connection)
    .await?;
    if !row.is_some_and(|(owner, location)| {
        owner == player.to_string() && location == venue.to_string()
    }) {
        return Err(AppError::Conflict(
            "Kiosk order is not open for this player and venue".into(),
        ));
    }
    if lines.iter().any(|line| !line.option_ids.is_empty()) {
        return Err(AppError::Conflict(
            "Kiosk order options cannot change during conversion".into(),
        ));
    }
    let rows: Vec<(String, String, Option<String>, i32, i64)> = sqlx::query_as(
        "SELECT product_id,product_name,product_sku,quantity,unit_price
         FROM kiosk_order_items WHERE order_id=? ORDER BY product_id,id",
    )
    .bind(order.to_string())
    .fetch_all(&mut *connection)
    .await?;
    let mut requested: Vec<(String, i32)> = lines
        .iter()
        .map(|line| (line.product_id.to_string(), line.quantity))
        .collect();
    requested.sort();
    let mut expected: Vec<(String, i32)> = rows
        .iter()
        .map(|(product_id, _, _, quantity, _)| (product_id.clone(), *quantity))
        .collect();
    expected.sort();
    if expected != requested {
        return Err(AppError::Conflict(
            "Kiosk order items changed before conversion".into(),
        ));
    }
    rows.into_iter()
        .map(
            |(product_id, product_name, product_sku, quantity, unit_price)| {
                Ok(KioskSnapshotLine {
                    product_id: parse_uuid(&product_id)?,
                    product_name,
                    product_sku,
                    quantity,
                    unit_price,
                })
            },
        )
        .collect()
}

#[derive(Clone)]
pub struct TenantCreditRepository {
    db: Arc<TenantDb>,
}

impl TenantCreditRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }

    pub async fn summary(&self, player: Uuid) -> Result<CreditSummary, AppError> {
        let row: Option<(i64, bool, String)> = sqlx::query_as(
            "SELECT credit_limit,is_active,role FROM users WHERE id=? AND deleted_at IS NULL",
        )
        .bind(player.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?;
        let (limit, active, role) =
            row.ok_or_else(|| AppError::BadRequest("player not eligible for credit".into()))?;
        let outstanding: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount-paid_amount),0) FROM transactions
             WHERE player_id=? AND payment_method='credit' AND payment_status='credit'
               AND deleted_at IS NULL",
        )
        .bind(player.to_string())
        .fetch_one(&self.db.read_pool()?)
        .await?;
        Ok(CreditSummary {
            player_id: player,
            credit_limit: money_f64(limit)?,
            outstanding: money_f64(outstanding)?,
            available: money_f64(limit.saturating_sub(outstanding).max(0))?,
            credit_enabled: active && role == "player" && limit > 0,
        })
    }

    pub async fn player_detail(&self, player: Uuid) -> Result<PlayerCreditDetail, AppError> {
        let summary = self.summary(player).await?;
        let transactions = sqlx::query_as(
            "SELECT unhex(replace(id,'-','')) transaction_id,transaction_type,
                    amount/10000.0 amount,paid_amount/10000.0 paid_amount,
                    (amount-paid_amount)/10000.0 remaining,payment_status,transaction_date,notes
             FROM transactions WHERE player_id=? AND payment_method='credit'
               AND payment_status='credit' AND amount>paid_amount AND deleted_at IS NULL
             ORDER BY transaction_date,id",
        )
        .bind(player.to_string())
        .fetch_all(&self.db.read_pool()?)
        .await?;
        Ok(PlayerCreditDetail {
            summary,
            transactions,
        })
    }

    pub async fn list_players(
        &self,
        filters: &CreditAccountFilterDto,
    ) -> Result<PaginationResult<CreditPlayerRow>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT player_id,username,first_name,last_name,phone_number,
                    credit_limit/10000.0 credit_limit,outstanding/10000.0 outstanding,
                    MAX(credit_limit-outstanding,0)/10000.0 available
             FROM (
               SELECT unhex(replace(u.id,'-','')) player_id,u.username,u.first_name,u.last_name,
                      u.phone_number,u.credit_limit,SUM(t.amount-t.paid_amount) outstanding
               FROM users u JOIN transactions t ON t.player_id=u.id
               AND t.payment_method='credit' AND t.payment_status='credit'
               AND t.deleted_at IS NULL
             WHERE u.role='player' AND u.deleted_at IS NULL",
        );
        credit_player_search(&mut query, filters);
        query.push(" GROUP BY u.id HAVING outstanding>0) q");
        let sort = match filters.sort_by.as_deref() {
            Some("username") => "username",
            Some("creditLimit") => "credit_limit",
            Some("available") => "available",
            _ => "outstanding",
        };
        let direction = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        query
            .push(format!(
                " ORDER BY {sort} {direction},player_id {direction} LIMIT "
            ))
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1) * limit);
        let data = query
            .build_query_as()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*) FROM (
               SELECT u.id,SUM(t.amount-t.paid_amount) outstanding
               FROM users u JOIN transactions t ON t.player_id=u.id
                 AND t.payment_method='credit' AND t.payment_status='credit'
                 AND t.deleted_at IS NULL
               WHERE u.role='player' AND u.deleted_at IS NULL",
        );
        credit_player_search(&mut count, filters);
        count.push(" GROUP BY u.id HAVING outstanding>0)");
        let total = count
            .build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(data, total, page, limit))
    }

    pub async fn set_limit(
        &self,
        player: Uuid,
        dto: &SetCreditLimitDto,
        actor: Option<Uuid>,
    ) -> Result<CreditSummary, AppError> {
        let limit = money(dto.credit_limit)?;
        if limit < 0 {
            return Err(AppError::BadRequest(
                "creditLimit must be greater than or equal to 0".into(),
            ));
        }
        let at = timestamp(&Utc::now())?;
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let changed = sqlx::query(
                        "UPDATE users SET credit_limit=?,updated_by=COALESCE(?,updated_by),updated_at=?
                         WHERE id=? AND role='player' AND deleted_at IS NULL",
                    )
                    .bind(limit)
                    .bind(actor.map(|value| value.to_string()))
                    .bind(&at)
                    .bind(player.to_string())
                    .execute(&mut *connection)
                    .await?;
                    if changed.rows_affected() != 1 {
                        return Err(AppError::NotFound(format!("Player with ID {player} not found")));
                    }
                    event(
                        connection,
                        "user",
                        player,
                        "credit.limit.updated",
                        None,
                        json!({"id":player,"creditLimit":money_f64(limit)?,"updatedAt":at}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.summary(player).await
    }

    pub async fn settle(
        &self,
        dto: SettleCreditDto,
        shift: Uuid,
        actor: Uuid,
    ) -> Result<CreditSettlement, AppError> {
        if dto.items.is_empty() {
            return Err(AppError::BadRequest(
                "At least one transaction must be selected".into(),
            ));
        }
        let mut seen = HashSet::new();
        let mut items = Vec::with_capacity(dto.items.len());
        let mut total = 0_i64;
        for item in dto.items {
            if !seen.insert(item.transaction_id) {
                return Err(AppError::BadRequest(
                    "A transaction may only be settled once per settlement".into(),
                ));
            }
            let amount = money(item.amount)?;
            if amount <= 0 {
                return Err(AppError::BadRequest(
                    "Each settlement amount must be greater than zero".into(),
                ));
            }
            total = total
                .checked_add(amount)
                .ok_or_else(|| AppError::BadRequest("Settlement total overflow".into()))?;
            items.push((item.transaction_id, amount));
        }
        let (cash, online) = settlement_tender(
            &dto.payment_method,
            dto.cash_amount,
            dto.online_amount,
            total,
        )?;
        let online_ref = validate_online_payment_ref_last4(
            &dto.payment_method,
            Some(money_f64(online)?),
            dto.online_payment_ref_last4,
        )?;
        let id = Uuid::now_v7();
        let at = timestamp(&Utc::now())?;
        let player = dto.player_id;
        let method = dto.payment_method;
        let notes = dto.notes;
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let shift_valid: bool = sqlx::query_scalar(
                        "SELECT EXISTS(SELECT 1 FROM shifts WHERE id=? AND user_id=? AND status='active')",
                    )
                    .bind(shift.to_string())
                    .bind(actor.to_string())
                    .fetch_one(&mut *connection)
                    .await?;
                    if !shift_valid {
                        return Err(AppError::BadRequest("Shift is not active for this user".into()));
                    }
                    sqlx::query(
                        "INSERT INTO credit_settlements(id,player_id,settled_by,shift_id,amount,
                         payment_method,cash_amount,online_amount,notes,online_payment_ref_last4,
                         settled_at,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)",
                    )
                    .bind(id.to_string())
                    .bind(player.to_string())
                    .bind(actor.to_string())
                    .bind(shift.to_string())
                    .bind(total)
                    .bind(&method)
                    .bind(cash)
                    .bind(online)
                    .bind(notes)
                    .bind(online_ref)
                    .bind(&at)
                    .bind(&at)
                    .bind(&at)
                    .execute(&mut *connection)
                    .await?;
                    for (transaction, applied) in items {
                        let row: Option<(i64, i64)> = sqlx::query_as(
                            "SELECT amount,paid_amount FROM transactions WHERE id=? AND player_id=?
                             AND payment_method='credit' AND payment_status='credit'
                             AND deleted_at IS NULL",
                        )
                        .bind(transaction.to_string())
                        .bind(player.to_string())
                        .fetch_optional(&mut *connection)
                        .await?;
                        let (amount, paid) = row.ok_or_else(|| {
                            AppError::NotFound(format!(
                                "Credit transaction {transaction} not found for player"
                            ))
                        })?;
                        let new_paid = paid.checked_add(applied).ok_or_else(|| {
                            AppError::BadRequest("Settlement amount overflow".into())
                        })?;
                        if new_paid > amount {
                            return Err(AppError::BadRequest(format!(
                                "Settlement exceeds remaining amount for transaction {transaction}"
                            )));
                        }
                        sqlx::query(
                            "INSERT INTO credit_settlement_items(id,settlement_id,transaction_id,
                             amount_applied,created_at) VALUES(?,?,?,?,?)",
                        )
                        .bind(Uuid::now_v7().to_string())
                        .bind(id.to_string())
                        .bind(transaction.to_string())
                        .bind(applied)
                        .bind(&at)
                        .execute(&mut *connection)
                        .await?;
                        sqlx::query(
                            "UPDATE transactions SET paid_amount=?,
                             payment_status=CASE WHEN ?=amount THEN 'completed' ELSE 'credit' END,
                             updated_by=?,updated_at=? WHERE id=?",
                        )
                        .bind(new_paid)
                        .bind(new_paid)
                        .bind(actor.to_string())
                        .bind(&at)
                        .bind(transaction.to_string())
                        .execute(&mut *connection)
                        .await?;
                        event(
                            connection,
                            "transaction",
                            transaction,
                            "transaction.credit_settled",
                            None,
                            json!({"id":transaction,"settlementId":id,
                                   "paidAmount":money_f64(new_paid)?,"updatedAt":at}),
                        )
                        .await?;
                    }
                    event(
                        connection,
                        "credit_settlement",
                        id,
                        "credit.settled",
                        None,
                        json!({"id":id,"playerId":player,"amount":money_f64(total)?,"updatedAt":at}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.find_settlement(id).await
    }

    pub async fn find_settlement(&self, id: Uuid) -> Result<CreditSettlement, AppError> {
        sqlx::query_as(
            "SELECT unhex(replace(id,'-','')) id,unhex(replace(player_id,'-','')) player_id,
                    unhex(replace(shift_id,'-','')) shift_id,
                    unhex(replace(settled_by,'-','')) settled_by,amount/10000.0 amount,
                    payment_method,cash_amount/10000.0 cash_amount,
                    online_amount/10000.0 online_amount,notes,online_payment_ref_last4,
                    settled_at,created_at,updated_at FROM credit_settlements
             WHERE id=? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credit settlement {id} not found")))
    }

    pub async fn list_settlements(
        &self,
        filters: &CreditSettlementFilterDto,
    ) -> Result<PaginationResult<CreditSettlementListRow>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT unhex(replace(cs.id,'-','')) id,
                    unhex(replace(cs.player_id,'-','')) player_id,p.username player_username,
                    unhex(replace(cs.shift_id,'-','')) shift_id,
                    unhex(replace(cs.settled_by,'-','')) settled_by,
                    a.username settled_by_username,cs.amount/10000.0 amount,cs.payment_method,
                    cs.cash_amount/10000.0 cash_amount,cs.online_amount/10000.0 online_amount,
                    cs.notes,cs.online_payment_ref_last4,cs.settled_at,
                    (SELECT COUNT(*) FROM credit_settlement_items i
                     WHERE i.settlement_id=cs.id) item_count
             FROM credit_settlements cs JOIN users p ON p.id=cs.player_id
             JOIN users a ON a.id=cs.settled_by WHERE cs.deleted_at IS NULL",
        );
        settlement_filters(&mut query, filters);
        let sort = match filters.sort_by.as_deref() {
            Some("amount") => "cs.amount",
            Some("playerUsername") => "p.username",
            Some("paymentMethod") => "cs.payment_method",
            _ => "cs.settled_at",
        };
        let direction = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        query
            .push(format!(
                " ORDER BY {sort} {direction},cs.id {direction} LIMIT "
            ))
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1) * limit);
        let data = query
            .build_query_as()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*) FROM credit_settlements cs
             JOIN users p ON p.id=cs.player_id WHERE cs.deleted_at IS NULL",
        );
        settlement_filters(&mut count, filters);
        let total = count
            .build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(data, total, page, limit))
    }

    pub async fn settlement_detail(&self, id: Uuid) -> Result<CreditSettlementDetail, AppError> {
        #[derive(FromRow)]
        struct Header {
            id: Uuid,
            player_id: Uuid,
            player_username: String,
            shift_id: Uuid,
            settled_by: Uuid,
            settled_by_username: String,
            amount: f64,
            payment_method: String,
            cash_amount: Option<f64>,
            online_amount: Option<f64>,
            notes: Option<String>,
            online_payment_ref_last4: Option<String>,
            settled_at: DateTime<Utc>,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
        }
        let header: Header = sqlx::query_as(
            "SELECT unhex(replace(cs.id,'-','')) id,
                    unhex(replace(cs.player_id,'-','')) player_id,p.username player_username,
                    unhex(replace(cs.shift_id,'-','')) shift_id,
                    unhex(replace(cs.settled_by,'-','')) settled_by,
                    a.username settled_by_username,cs.amount/10000.0 amount,cs.payment_method,
                    cs.cash_amount/10000.0 cash_amount,cs.online_amount/10000.0 online_amount,
                    cs.notes,cs.online_payment_ref_last4,cs.settled_at,cs.created_at,cs.updated_at
             FROM credit_settlements cs JOIN users p ON p.id=cs.player_id
             JOIN users a ON a.id=cs.settled_by WHERE cs.id=? AND cs.deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credit settlement {id} not found")))?;
        let items: Vec<CreditSettlementItemRow> = sqlx::query_as(
            "SELECT unhex(replace(t.id,'-','')) transaction_id,t.transaction_type,
                    t.transaction_date,t.amount/10000.0 original_amount,
                    i.amount_applied/10000.0 amount_applied,
                    MAX(t.amount-(
                      SELECT COALESCE(SUM(i2.amount_applied),0)
                      FROM credit_settlement_items i2
                      JOIN credit_settlements cs2 ON cs2.id=i2.settlement_id
                      WHERE i2.transaction_id=t.id AND cs2.deleted_at IS NULL
                        AND (cs2.settled_at<cs.settled_at
                             OR (cs2.settled_at=cs.settled_at AND cs2.id<=cs.id))
                    ),0)/10000.0 remaining_after
             FROM credit_settlement_items i
             JOIN credit_settlements cs ON cs.id=i.settlement_id
             JOIN transactions t ON t.id=i.transaction_id
             WHERE i.settlement_id=? ORDER BY t.transaction_date,t.id",
        )
        .bind(id.to_string())
        .fetch_all(&self.db.read_pool()?)
        .await?;
        Ok(CreditSettlementDetail {
            id: header.id,
            player_id: header.player_id,
            player_username: header.player_username,
            shift_id: header.shift_id,
            settled_by: header.settled_by,
            settled_by_username: header.settled_by_username,
            amount: header.amount,
            payment_method: header.payment_method,
            cash_amount: header.cash_amount,
            online_amount: header.online_amount,
            notes: header.notes,
            online_payment_ref_last4: header.online_payment_ref_last4,
            settled_at: header.settled_at,
            created_at: header.created_at,
            updated_at: header.updated_at,
            items,
        })
    }
}

fn credit_player_search<'a>(
    query: &mut QueryBuilder<'a, Sqlite>,
    filters: &'a CreditAccountFilterDto,
) {
    if let Some(search) = filters.search.as_ref().filter(|value| !value.is_empty()) {
        query
            .push(" AND (instr(lower(u.username),lower(")
            .push_bind(search)
            .push("))>0 OR instr(lower(COALESCE(u.first_name,'')),lower(")
            .push_bind(search)
            .push("))>0 OR instr(lower(COALESCE(u.last_name,'')),lower(")
            .push_bind(search)
            .push("))>0 OR instr(lower(COALESCE(u.phone_number,'')),lower(")
            .push_bind(search)
            .push("))>0)");
    }
}

fn settlement_filters<'a>(
    query: &mut QueryBuilder<'a, Sqlite>,
    filters: &'a CreditSettlementFilterDto,
) {
    if let Some(player) = filters.player_id {
        query
            .push(" AND cs.player_id=")
            .push_bind(player.to_string());
    }
    if let Some(method) = filters
        .payment_method
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        query.push(" AND cs.payment_method=").push_bind(method);
    }
    if let Some(start) = &filters.start_date {
        query.push(" AND cs.settled_at>=").push_bind(start);
    }
    if let Some(end) = &filters.end_date {
        query.push(" AND cs.settled_at<=").push_bind(end);
    }
    if let Some(search) = filters.search.as_ref().filter(|value| !value.is_empty()) {
        query
            .push(" AND (instr(lower(p.username),lower(")
            .push_bind(search)
            .push("))>0 OR instr(lower(COALESCE(p.first_name,'')),lower(")
            .push_bind(search)
            .push("))>0 OR instr(lower(COALESCE(p.last_name,'')),lower(")
            .push_bind(search)
            .push("))>0)");
    }
}

fn settlement_tender(
    method: &str,
    cash: Option<f64>,
    online: Option<f64>,
    total: i64,
) -> Result<(i64, i64), AppError> {
    let values = match method {
        "cash" => (cash.map(money).transpose()?.unwrap_or(total), 0),
        "online" => (0, online.map(money).transpose()?.unwrap_or(total)),
        "split_payment" => (
            cash.map(money).transpose()?.unwrap_or(0),
            online.map(money).transpose()?.unwrap_or(0),
        ),
        _ => {
            return Err(AppError::BadRequest(
                "Invalid settlement payment method".into(),
            ))
        }
    };
    if values.0.checked_add(values.1) != Some(total) {
        return Err(AppError::BadRequest(
            "Settlement tender must equal settlement total".into(),
        ));
    }
    Ok(values)
}

#[derive(Clone)]
pub struct TenantKioskOrderRepository {
    db: Arc<TenantDb>,
    timezone: String,
}

impl TenantKioskOrderRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self {
            db,
            timezone: "UTC".into(),
        }
    }

    pub fn with_timezone(mut self, timezone: String) -> Self {
        self.timezone = timezone;
        self
    }

    pub async fn place(
        &self,
        player: Uuid,
        device: Uuid,
        dto: crate::models::CreateKioskOrderDto,
    ) -> Result<KioskOrderWithItems, AppError> {
        if dto.line_items.is_empty() {
            return Err(AppError::BadRequest("lineItems must not be empty".into()));
        }
        let id = Uuid::now_v7();
        let at = Utc::now();
        let at_text = timestamp(&at)?;
        let note = dto.note;
        let timezone = self.timezone.clone();
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let session: (String, String) = sqlx::query_as(
                        "SELECT s.id,s.location_id FROM usage_sessions s
                             JOIN devices d ON d.id=s.device_id
                             JOIN users u ON u.id=s.player_id
                             WHERE s.player_id=? AND s.device_id=? AND s.end_time IS NULL
                               AND s.deleted_at IS NULL AND d.deleted_at IS NULL
                               AND u.is_active=1 AND u.deleted_at IS NULL",
                    )
                    .bind(player.to_string())
                    .bind(device.to_string())
                    .fetch_optional(&mut *connection)
                    .await?
                    .ok_or_else(|| AppError::not_found_code("KIOSK_NO_ACTIVE_SESSION"))?;
                    let session_id = parse_uuid(&session.0)?;
                    let venue = parse_uuid(&session.1)?;
                    let pricing = pricing_context(connection, venue, timezone).await?;
                    sqlx::query(
                        "INSERT INTO kiosk_orders(id,session_id,player_id,device_id,status,
                         player_note,created_at,updated_at) VALUES(?,?,?,?,'pending',?,?,?)",
                    )
                    .bind(id.to_string())
                    .bind(session_id.to_string())
                    .bind(player.to_string())
                    .bind(device.to_string())
                    .bind(note)
                    .bind(&at_text)
                    .bind(&at_text)
                    .execute(&mut *connection)
                    .await
                    .map_err(|error| {
                        if matches!(&error, sqlx::Error::Database(db) if db.is_unique_violation()) {
                            AppError::conflict_code("KIOSK_ORDER_ALREADY_OPEN", None)
                        } else {
                            AppError::from(error)
                        }
                    })?;
                    for item in dto.line_items {
                        if item.quantity <= 0 {
                            return Err(AppError::BadRequest(
                                "Product quantity must be positive".into(),
                            ));
                        }
                        let row: Option<(String, Option<String>, i64, i64, String)> =
                            sqlx::query_as(
                                "SELECT p.name,p.sku,COALESCE(pl.price,p.day_price),
                                        COALESCE(pl.price,p.night_price),p.category
                                 FROM products p
                                 LEFT JOIN product_locations pl
                                   ON pl.product_id=p.id AND pl.location_id=?
                                 WHERE p.id=? AND p.is_active=1 AND p.is_raw_material=0
                                   AND p.deleted_at IS NULL
                                   AND (p.availability_scope='ALL' OR pl.product_id IS NOT NULL)
                                   AND NOT EXISTS(SELECT 1 FROM product_option_groups g
                                                  WHERE g.product_id=p.id AND g.required=1)",
                            )
                            .bind(venue.to_string())
                            .bind(item.product_id.to_string())
                            .fetch_optional(&mut *connection)
                            .await?;
                        let (name, sku, day, night, category) = row.ok_or_else(|| {
                            AppError::BadRequest(format!(
                                "Product {} is not available",
                                item.product_id
                            ))
                        })?;
                        let price = PricingPolicyService::evaluate_product_price(
                            money_f64(day)?,
                            money_f64(night)?,
                            item.product_id,
                            &category,
                            &pricing.rules,
                            at,
                            &pricing.timezone,
                            &pricing.night_start,
                            &pricing.night_end,
                        )?;
                        sqlx::query(
                            "INSERT INTO kiosk_order_items(id,order_id,product_id,product_name,
                             product_sku,quantity,unit_price,created_at) VALUES(?,?,?,?,?,?,?,?)",
                        )
                        .bind(Uuid::now_v7().to_string())
                        .bind(id.to_string())
                        .bind(item.product_id.to_string())
                        .bind(name)
                        .bind(sku)
                        .bind(item.quantity)
                        .bind(money(price)?)
                        .bind(&at_text)
                        .execute(&mut *connection)
                        .await?;
                    }
                    event(
                        connection,
                        "kiosk_order",
                        id,
                        "kiosk_order.placed",
                        Some(venue),
                        json!({"id":id,"sessionId":session_id,"playerId":player,
                               "deviceId":device,"updatedAt":at_text}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.get(id).await
    }

    pub async fn get(&self, id: Uuid) -> Result<KioskOrderWithItems, AppError> {
        let row: Option<KioskDetailRow> = sqlx::query_as(
            "SELECT unhex(replace(ko.id,'-','')) id,
                    unhex(replace(ko.session_id,'-','')) session_id,
                    unhex(replace(ko.player_id,'-','')) player_id,
                    unhex(replace(ko.device_id,'-','')) device_id,ko.status,ko.player_note,
                    unhex(replace(ko.transaction_id,'-','')) transaction_id,
                    ko.created_at,ko.updated_at,ko.fulfilled_at,d.name device_name,
                    u.username player_username
             FROM kiosk_orders ko LEFT JOIN devices d ON d.id=ko.device_id
             LEFT JOIN users u ON u.id=ko.player_id WHERE ko.id=?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?;
        let row = row.ok_or_else(|| AppError::not_found_code("KIOSK_ORDER_NOT_FOUND"))?;
        let items = self.items(id).await?;
        Ok(row.with_items(items))
    }

    pub async fn current_for_player(
        &self,
        player: Uuid,
        device: Uuid,
    ) -> Result<Option<KioskOrderWithItems>, AppError> {
        let id: Option<Uuid> = sqlx::query_scalar(
            "SELECT unhex(replace(ko.id,'-','')) FROM kiosk_orders ko
             JOIN usage_sessions s ON s.id=ko.session_id
             WHERE ko.player_id=? AND ko.device_id=? AND ko.status IN ('pending','preparing')
               AND s.end_time IS NULL AND s.deleted_at IS NULL
             ORDER BY ko.created_at DESC,ko.id DESC LIMIT 1",
        )
        .bind(player.to_string())
        .bind(device.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?;
        match id {
            Some(id) => self.get(id).await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn list(
        &self,
        filters: &KioskOrderFilterDto,
    ) -> Result<PaginationResult<KioskOrderWithItems>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT unhex(replace(ko.id,'-','')) id,
                    unhex(replace(ko.session_id,'-','')) session_id,
                    unhex(replace(ko.player_id,'-','')) player_id,
                    unhex(replace(ko.device_id,'-','')) device_id,ko.status,ko.player_note,
                    unhex(replace(ko.transaction_id,'-','')) transaction_id,
                    ko.created_at,ko.updated_at,ko.fulfilled_at,d.name device_name,
                    u.username player_username FROM kiosk_orders ko
             LEFT JOIN devices d ON d.id=ko.device_id LEFT JOIN users u ON u.id=ko.player_id
             WHERE 1=1",
        );
        kiosk_filters(&mut query, filters);
        query
            .push(" ORDER BY ko.created_at DESC,ko.id DESC LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1) * limit);
        let rows: Vec<KioskDetailRow> = query
            .build_query_as()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut data = Vec::with_capacity(rows.len());
        for row in rows {
            let items = self.items(row.id).await?;
            data.push(row.with_items(items));
        }
        let mut count =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM kiosk_orders ko WHERE 1=1");
        kiosk_filters(&mut count, filters);
        let total = count
            .build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(data, total, page, limit))
    }

    pub async fn venue_for_order(&self, id: Uuid) -> Result<Uuid, AppError> {
        sqlx::query_scalar(
            "SELECT unhex(replace(s.location_id,'-','')) FROM kiosk_orders ko
             JOIN usage_sessions s ON s.id=ko.session_id WHERE ko.id=?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?
        .ok_or_else(|| AppError::not_found_code("KIOSK_ORDER_NOT_FOUND"))
    }

    pub async fn update_status(
        &self,
        id: Uuid,
        status: &str,
    ) -> Result<KioskOrderWithItems, AppError> {
        if !matches!(
            status,
            kiosk_order_status::PREPARING | kiosk_order_status::CANCELLED
        ) {
            return Err(AppError::BadRequest(format!(
                "Invalid status transition to {status}"
            )));
        }
        let status = status.to_owned();
        let at = timestamp(&Utc::now())?;
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let location: Option<String> = sqlx::query_scalar(
                        "UPDATE kiosk_orders SET status=?,updated_at=?
                         WHERE id=? AND status IN ('pending','preparing')
                         RETURNING (SELECT location_id FROM usage_sessions
                                    WHERE id=kiosk_orders.session_id)",
                    )
                    .bind(&status)
                    .bind(&at)
                    .bind(id.to_string())
                    .fetch_optional(&mut *connection)
                    .await?;
                    let location = location
                        .ok_or_else(|| AppError::not_found_code("KIOSK_ORDER_NOT_FOUND"))?;
                    event(
                        connection,
                        "kiosk_order",
                        id,
                        "kiosk_order.status_changed",
                        Some(parse_uuid(&location)?),
                        json!({"id":id,"status":status,"updatedAt":at}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.get(id).await
    }

    async fn items(&self, id: Uuid) -> Result<Vec<KioskOrderItem>, AppError> {
        Ok(sqlx::query_as(
            "SELECT unhex(replace(id,'-','')) id,unhex(replace(order_id,'-','')) order_id,
                    unhex(replace(product_id,'-','')) product_id,quantity,product_name,
                    unit_price/10000.0 unit_price,created_at FROM kiosk_order_items
             WHERE order_id=? ORDER BY created_at,id",
        )
        .bind(id.to_string())
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }
}

fn kiosk_filters<'a>(query: &mut QueryBuilder<'a, Sqlite>, filters: &'a KioskOrderFilterDto) {
    if let Some(status) = &filters.status {
        query.push(" AND ko.status=").push_bind(status);
    }
    if let Some(device) = filters.device_id {
        query
            .push(" AND ko.device_id=")
            .push_bind(device.to_string());
    }
}

#[derive(FromRow)]
struct KioskDetailRow {
    id: Uuid,
    session_id: Uuid,
    player_id: Uuid,
    device_id: Uuid,
    status: String,
    player_note: Option<String>,
    transaction_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    fulfilled_at: Option<DateTime<Utc>>,
    device_name: Option<String>,
    player_username: Option<String>,
}

impl KioskDetailRow {
    fn with_items(self, items: Vec<KioskOrderItem>) -> KioskOrderWithItems {
        KioskOrderWithItems::from_parts(
            KioskOrder {
                id: self.id,
                session_id: self.session_id,
                player_id: self.player_id,
                device_id: self.device_id,
                status: self.status,
                player_note: self.player_note,
                transaction_id: self.transaction_id,
                created_at: self.created_at,
                updated_at: self.updated_at,
                fulfilled_at: self.fulfilled_at,
            },
            items,
            self.device_name,
            self.player_username,
        )
    }
}
