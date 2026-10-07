//! SQLite procurement workflows. Receipts, stock, expenses and cash commit together.
use crate::repositories::{
    tenant_back_office::{event, money, money_f64, now, write},
    TenantInventoryRepository,
};
use crate::{
    dto::PaginationResult,
    error::AppError,
    models::*,
    tenancy::{format_sqlite_timestamp, TenantDb},
};
use chrono::Datelike;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde_json::json;
use sqlx::{QueryBuilder, Sqlite, SqliteConnection};
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;
#[derive(Clone)]
pub struct TenantProcurementService {
    db: Arc<TenantDb>,
    timezone: String,
}
struct Snapshot {
    product: Uuid,
    boxes: i32,
    units: i32,
    cost: i64,
    rate: i64,
    subtotal: i64,
    tax: i64,
    total: i64,
}
impl TenantProcurementService {
    pub fn new(db: Arc<TenantDb>, timezone: String) -> Self {
        Self { db, timezone }
    }
    const ORDER_SELECT: &'static str = r#"
                        SELECT unhex(replace(id, '-' , '' )) AS id, po_number, unhex(replace(vendor_id, '-' , '' )) AS
                        vendor_id, unhex(replace(destination_location_id, '-' , '' )) AS destination_location_id, status,
                        expected_delivery_date, subtotal/10000.0 AS subtotal, discount/10000.0 AS discount, tax/10000.0 AS
                        tax, freight/10000.0 AS freight, total/10000.0 AS total, notes, rejection_reason, version,
                        unhex(replace(created_by, '-' , '' )) AS created_by, unhex(replace(submitted_by, '-' , '' )) AS
                        submitted_by, submitted_at, unhex(replace(approved_by, '-' , '' )) AS approved_by, approved_at,
                        unhex(replace(ordered_by, '-' , '' )) AS ordered_by, ordered_at, unhex(replace(cancelled_by, '-' ,
                        '' )) AS cancelled_by, cancelled_at, created_at, updated_at FROM purchase_orders
                    "#;
    const LINE_SELECT: &'static str = r#"
                        SELECT unhex(replace(pol.id, '-' , '' )) AS id, unhex(replace(pol.purchase_order_id, '-' , '' )) AS
                        purchase_order_id, unhex(replace(pol.product_id, '-' , '' )) AS product_id, p.name AS product_name,
                        p.sku AS product_sku, pol.ordered_boxes, pol.received_boxes, pol.units_per_box_snapshot,
                        pol.box_cost_snapshot/10000.0 AS box_cost_snapshot, pol.tax_rate/10000.0 AS tax_rate,
                        pol.line_subtotal/10000.0 AS line_subtotal, pol.line_tax/10000.0 AS line_tax, pol.line_total/10000.0
                        AS line_total FROM purchase_order_lines pol JOIN products p ON p.id=pol.product_id
                    "#;
    pub async fn get_order(&self, id: Uuid) -> Result<PurchaseOrderWithLines, AppError> {
        // Both statements share one reader transaction, preserving a consistent order version.
        let mut tx = self.db.read_pool()?.begin().await?;
        let row = Self::get(&mut tx, id).await?;
        tx.commit().await?;
        Ok(row)
    }
    async fn get(c: &mut SqliteConnection, id: Uuid) -> Result<PurchaseOrderWithLines, AppError> {
        let order = sqlx::query_as(&format!("{} WHERE id=?", Self::ORDER_SELECT))
            .bind(id.to_string())
            .fetch_optional(&mut *c)
            .await?
            .ok_or_else(|| AppError::not_found_code("PURCHASE_ORDER_NOT_FOUND"))?;
        let lines = sqlx::query_as(&format!(
            "{} WHERE pol.purchase_order_id=? ORDER BY p.name,pol.id",
            Self::LINE_SELECT
        ))
        .bind(id.to_string())
        .fetch_all(c)
        .await?;
        Ok(PurchaseOrderWithLines { order, lines })
    }
    pub async fn list_orders(
        &self,
        f: PurchaseOrderFilterDto,
    ) -> Result<PaginationResult<PurchaseOrder>, AppError> {
        let page = f.page.unwrap_or(1).max(1);
        let limit = f.limit.unwrap_or(20).clamp(1, 100);
        let mut q = QueryBuilder::<Sqlite>::new(format!("{} WHERE 1=1", Self::ORDER_SELECT));
        Self::filters(&mut q, &f);
        q.push(" ORDER BY updated_at DESC,id DESC LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1).saturating_mul(limit));
        let pool = self.db.read_pool()?;
        let items = q.build_query_as().fetch_all(&pool).await?;
        let mut count =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM purchase_orders WHERE 1=1");
        Self::filters(&mut count, &f);
        let total = count.build_query_scalar().fetch_one(&pool).await?;
        Ok(PaginationResult::new(items, total, page, limit))
    }
    fn filters(q: &mut QueryBuilder<Sqlite>, f: &PurchaseOrderFilterDto) {
        if let Some(status) = &f.status {
            q.push(" AND status=").push_bind(status.clone());
        }
        if let Some(id) = f.vendor_id {
            q.push(" AND vendor_id=").push_bind(id.to_string());
        }
        if let Some(id) = f.destination_location_id {
            q.push(" AND destination_location_id=")
                .push_bind(id.to_string());
        }
        if let Some(search) = &f.search {
            q.push(" AND po_number LIKE ")
                .push_bind(format!("%{search}%"));
        }
    }
    fn nonnegative(value: f64) -> Result<i64, AppError> {
        let n = money(value)?;
        if n < 0 {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_FINANCIALS_INVALID",
                None,
            ));
        }
        Ok(n)
    }
    fn tax(subtotal: i64, rate: i64) -> Result<i64, AppError> {
        // rate is percent scaled by 10,000. Round exactly once per line, to four decimals.
        Decimal::from(subtotal)
            .checked_mul(Decimal::from(rate))
            .and_then(|value| value.checked_div(Decimal::from(1_000_000)))
            .and_then(|value| value.round().to_i64())
            .ok_or_else(|| AppError::BadRequest("Purchase order tax overflow".into()))
    }
    fn add(a: i64, b: i64) -> Result<i64, AppError> {
        a.checked_add(b)
            .ok_or_else(|| AppError::BadRequest("Purchase order amount overflow".into()))
    }
    async fn snapshots(
        c: &mut SqliteConnection,
        inputs: &[PurchaseOrderLineInput],
    ) -> Result<Vec<Snapshot>, AppError> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        if inputs.is_empty() {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_LINES_INVALID",
                None,
            ));
        }
        for line in inputs {
            if line.ordered_boxes <= 0 || !seen.insert(line.product_id) {
                return Err(AppError::bad_request_code(
                    "PURCHASE_ORDER_LINES_INVALID",
                    None,
                ));
            }
            let units = sqlx::query_scalar(
                "SELECT units_per_purchase_unit FROM products WHERE id=? AND deleted_at IS NULL",
            )
            .bind(line.product_id.to_string())
            .fetch_optional(&mut *c)
            .await?
            .ok_or_else(|| AppError::bad_request_code("PURCHASE_ORDER_PRODUCT_INVALID", None))?;
            let cost = Self::nonnegative(line.box_cost)?;
            let rate = Self::nonnegative(line.tax_rate.unwrap_or(0.0))?;
            let subtotal = cost
                .checked_mul(i64::from(line.ordered_boxes))
                .ok_or_else(|| AppError::BadRequest("Purchase order amount overflow".into()))?;
            let tax = Self::tax(subtotal, rate)?;
            result.push(Snapshot {
                product: line.product_id,
                boxes: line.ordered_boxes,
                units,
                cost,
                rate,
                subtotal,
                tax,
                total: Self::add(subtotal, tax)?,
            });
        }
        Ok(result)
    }
    async fn references(
        c: &mut SqliteConnection,
        vendor: Uuid,
        location: Uuid,
    ) -> Result<InventoryLocation, AppError> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM vendors WHERE id=? AND is_active=1)")
                .bind(vendor.to_string())
                .fetch_one(&mut *c)
                .await?;
        if !exists {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_REFERENCE_INVALID",
                None,
            ));
        }
        TenantInventoryRepository::location(c, location).await
    }
    async fn insert_lines(
        c: &mut SqliteConnection,
        id: Uuid,
        lines: Vec<Snapshot>,
    ) -> Result<(), AppError> {
        for line in lines {
            sqlx::query(r#"
                        INSERT INTO purchase_order_lines(id, purchase_order_id, product_id, ordered_boxes,
                        units_per_box_snapshot, box_cost_snapshot, tax_rate, line_subtotal, line_tax, line_total) VALUES(?,
                        ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#).bind(Uuid::now_v7().to_string()).bind(id.to_string()).bind(line.product.to_string()).bind(line.boxes).bind(line.units).bind(line.cost).bind(line.rate).bind(line.subtotal).bind(line.tax).bind(line.total).execute(&mut *c).await?;
        }
        Ok(())
    }
    fn totals(
        lines: &[Snapshot],
        discount: i64,
        freight: i64,
    ) -> Result<(i64, i64, i64), AppError> {
        let mut subtotal = 0;
        let mut tax = 0;
        for line in lines {
            subtotal = Self::add(subtotal, line.subtotal)?;
            tax = Self::add(tax, line.tax)?;
        }
        let total = Self::add(Self::add(subtotal, tax)?, freight)?
            .checked_sub(discount)
            .ok_or_else(|| AppError::BadRequest("Purchase order amount overflow".into()))?
            .max(0);
        Ok((subtotal, tax, total))
    }
    pub async fn create_order(
        &self,
        dto: CreatePurchaseOrderDto,
        actor: Uuid,
    ) -> Result<PurchaseOrderWithLines, AppError> {
        let tz: chrono_tz::Tz = self
            .timezone
            .parse()
            .map_err(|_| AppError::BadRequest("Invalid tenant timezone".into()))?;
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let location = Self::references(c, dto.vendor_id, dto.destination_location_id).await?;
                    let lines = Self::snapshots(c, &dto.lines).await?;
                    let discount = Self::nonnegative(dto.discount.unwrap_or(0.0))?;
                    let freight = Self::nonnegative(dto.freight.unwrap_or(0.0))?;
                    let (subtotal, tax, total) = Self::totals(&lines, discount, freight)?;
                    let sequence: i64 = sqlx::query_scalar("UPDATE purchase_order_number_counter SET next_value=next_value+1 WHERE singleton=1 RETURNING next_value-1").fetch_one(&mut *c).await?;
                    let number = format!("PO-{}-{sequence:06}", chrono::Utc::now().with_timezone(&tz).year());
                    let id = Uuid::now_v7();
                    let ts = now()?;
                    sqlx::query(r#"
                        INSERT INTO purchase_orders(id, po_number, vendor_id, destination_location_id,
                        expected_delivery_date, subtotal, discount, tax, freight, total, notes, created_by, created_at,
                        updated_at) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#).bind(id.to_string()).bind(number).bind(dto.vendor_id.to_string()).bind(dto.destination_location_id.to_string()).bind(dto.expected_delivery_date.map(|x| x.to_string())).bind(subtotal).bind(discount).bind(tax).bind(freight).bind(total).bind(dto.notes).bind(actor.to_string()).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    Self::insert_lines(c, id, lines).await?;
                    let row = Self::get(c, id).await?;
                    event(c, "purchase_order", id, "procurement.order_created", Some(location.venue_location_id), false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn update_order(
        &self,
        id: Uuid,
        dto: UpdatePurchaseOrderDto,
        actor: Uuid,
    ) -> Result<PurchaseOrderWithLines, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::get(c, id).await?;
                    if !matches!(old.order.status.as_str(), "draft" | "rejected") {
                        return Err(AppError::conflict_code("PURCHASE_ORDER_NOT_EDITABLE", None));
                    }
                    if old.order.version != dto.version {
                        return Err(AppError::conflict_code("PURCHASE_ORDER_VERSION_CONFLICT", None));
                    }
                    let vendor = dto.vendor_id.unwrap_or(old.order.vendor_id);
                    let dest = dto.destination_location_id.unwrap_or(old.order.destination_location_id);
                    let location = Self::references(c, vendor, dest).await?;
                    let (stored_discount,stored_freight):(i64,i64)=sqlx::query_as("SELECT discount,freight FROM purchase_orders WHERE id=?").bind(id.to_string()).fetch_one(&mut *c).await?;
                    let discount=match dto.discount{Some(value)=>Self::nonnegative(value)?,None=>stored_discount};
                    let freight=match dto.freight{Some(value)=>Self::nonnegative(value)?,None=>stored_freight};
                    let replacing = dto.lines.is_some();
                    let lines = if let Some(inputs) = dto.lines {
                        Self::snapshots(c, &inputs).await?
                    } else {
                        // Preserve purchase-unit and financial snapshots when changing header fields only.
                        let stored: Vec<(String, i32, i32, i64, i64, i64, i64, i64)> = sqlx::query_as(r#"
                        SELECT product_id, ordered_boxes, units_per_box_snapshot, box_cost_snapshot, tax_rate,
                        line_subtotal, line_tax, line_total FROM purchase_order_lines WHERE purchase_order_id=? ORDER BY id
                    "#).bind(id.to_string()).fetch_all(&mut *c).await?;
                        stored.into_iter().map(|(product, boxes, units, cost, rate, subtotal, tax, total)| Ok(Snapshot { product: Uuid::parse_str(&product).map_err(|e| AppError::Internal(e.to_string()))?, boxes, units, cost, rate, subtotal, tax, total })).collect::<Result<Vec<_>, AppError>>()?
                    };
                    let (subtotal, tax, total) = Self::totals(&lines, discount, freight)?;
                    sqlx::query(r#"
                        UPDATE purchase_orders SET vendor_id=?, destination_location_id=?,
                        expected_delivery_date=COALESCE(?, expected_delivery_date), notes=COALESCE(?, notes), subtotal=?,
                        discount=?, tax=?, freight=?, total=?, status= 'draft' , rejection_reason=NULL, version=version+1,
                        updated_at=? WHERE id=?
                    "#).bind(vendor.to_string()).bind(dest.to_string()).bind(dto.expected_delivery_date.map(|x| x.to_string())).bind(dto.notes).bind(subtotal).bind(discount).bind(tax).bind(freight).bind(total).bind(now()?).bind(id.to_string()).execute(&mut *c).await?;
                    if replacing {
                        sqlx::query("DELETE FROM purchase_order_lines WHERE purchase_order_id=?").bind(id.to_string()).execute(&mut *c).await?;
                        Self::insert_lines(c, id, lines).await?;
                    }
                    let row = Self::get(c, id).await?;
                    event(c, "purchase_order", id, "procurement.order_updated", Some(location.venue_location_id), false, json!({"order":row,"actorId":actor})).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn transition(
        &self,
        id: Uuid,
        action: &str,
        reason: Option<String>,
        actor: Uuid,
    ) -> Result<PurchaseOrderWithLines, AppError> {
        let (allowed, status, by, at): (&[&str], &str, &str, &str) = match action {
            "submit" => (
                &["draft", "rejected"],
                "submitted",
                "submitted_by",
                "submitted_at",
            ),
            "approve" => (&["submitted"], "approved", "approved_by", "approved_at"),
            "reject" => (&["submitted"], "rejected", "approved_by", "approved_at"),
            "mark_ordered" => (&["approved"], "ordered", "ordered_by", "ordered_at"),
            "cancel" => (
                &["draft", "submitted", "approved", "ordered"],
                "cancelled",
                "cancelled_by",
                "cancelled_at",
            ),
            _ => {
                return Err(AppError::bad_request_code(
                    "PURCHASE_ORDER_INVALID_ACTION",
                    None,
                ))
            }
        };
        if status == "rejected" && reason.as_deref().unwrap_or("").trim().len() < 3 {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_REJECTION_REASON_REQUIRED",
                None,
            ));
        }
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::get(c, id).await?;
                    if !allowed.contains(&old.order.status.as_str()) {
                        return Err(AppError::conflict_code("PURCHASE_ORDER_INVALID_TRANSITION", None));
                    }
                    let location = TenantInventoryRepository::location(c, old.order.destination_location_id).await?;
                    let ts = now()?;
                    sqlx::query(&format!("UPDATE purchase_orders SET status=?,{by}=?,{at}=?,rejection_reason=?,version=version+1,updated_at=? WHERE id=?")).bind(status).bind(actor.to_string()).bind(&ts).bind(if status == "rejected" { reason } else { None }).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
                    let row = Self::get(c, id).await?;
                    event(c, "purchase_order", id, "procurement.order_transitioned", Some(location.venue_location_id), false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn receive(
        &self,
        id: Uuid,
        dto: ReceivePurchaseOrderDto,
        actor: Uuid,
    ) -> Result<ReceivePurchaseOrderResponse, AppError> {
        if dto.invoice_reference.trim().is_empty() || dto.lines.is_empty() {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_RECEIPT_INVALID",
                None,
            ));
        }
        if !matches!(dto.payment_method.as_str(), "cash" | "online") {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_RECEIPT_PAYMENT_INVALID",
                None,
            ));
        }
        if dto.payment_method == "online"
            && dto
                .payment_account
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_RECEIPT_PAYMENT_ACCOUNT_REQUIRED",
                None,
            ));
        }
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::get(c, id).await?;
                    let order = old.order;
                    if !matches!(order.status.as_str(), "approved" | "ordered" | "partially_received") {
                        return Err(AppError::conflict_code("PURCHASE_ORDER_NOT_RECEIVABLE", None));
                    }
                    let approver = order.approved_by.ok_or_else(|| AppError::conflict_code("PURCHASE_ORDER_NOT_APPROVED", None))?;
                    let location = TenantInventoryRepository::location(c, order.destination_location_id).await?;
                    let mut seen = HashSet::new();
                    let mut lines = Vec::new();
                    let mut subtotal = 0;
                    let mut tax = 0;
                    for input in &dto.lines {
                        if !seen.insert(input.purchase_order_line_id) {
                            return Err(AppError::bad_request_code("PURCHASE_ORDER_RECEIPT_DUPLICATE_LINE", None));
                        }
                        let line = old.lines.iter().find(|x| x.id == input.purchase_order_line_id).ok_or_else(|| AppError::bad_request_code("PURCHASE_ORDER_LINE_NOT_FOUND", None))?;
                        let accepted = input.accepted_boxes;
                        let rejected = input.rejected_boxes.unwrap_or(0);
                        if accepted <= 0 || rejected < 0 {
                            return Err(AppError::bad_request_code("PURCHASE_ORDER_RECEIPT_QUANTITY_INVALID", None));
                        }
                        let delivered = accepted.checked_add(rejected).ok_or_else(|| AppError::conflict_code("PURCHASE_ORDER_OVER_RECEIPT", None))?;
                        if delivered > line.ordered_boxes - line.received_boxes {
                            return Err(AppError::conflict_code("PURCHASE_ORDER_OVER_RECEIPT", None));
                        }
                        let pieces = accepted.checked_mul(line.units_per_box_snapshot).ok_or_else(|| AppError::BadRequest("Receipt quantity overflow".into()))?;
                        // Read integers directly, avoiding a float round-trip for stored snapshots.
                        let (cost, rate): (i64, i64) = sqlx::query_as("SELECT box_cost_snapshot,tax_rate FROM purchase_order_lines WHERE id=?").bind(line.id.to_string()).fetch_one(&mut *c).await?;
                        let line_subtotal = cost.checked_mul(i64::from(accepted)).ok_or_else(|| AppError::BadRequest("Receipt amount overflow".into()))?;
                        let line_tax = Self::tax(line_subtotal, rate)?;
                        subtotal = Self::add(subtotal, line_subtotal)?;
                        tax = Self::add(tax, line_tax)?;
                        lines.push((line.clone(), accepted, rejected, delivered, pieces, cost, rate, Self::add(line_subtotal, line_tax)?));
                    }
                    let total = Self::add(subtotal, tax)?;
                    if total <= 0 {
                        return Err(AppError::bad_request_code("PURCHASE_ORDER_RECEIPT_TOTAL_INVALID", None));
                    }
                    let ts = now()?;
                    let date = if let Some(date) = dto.receipt_date { format_sqlite_timestamp(&date).map_err(|e| AppError::BadRequest(e.to_string()))? } else { ts.clone() };
                    let receipt = Uuid::now_v7();
                    sqlx::query(r#"
                        INSERT INTO stock_receipts(id, inventory_location_id, vendor_id, purchase_order_id,
                        invoice_reference, payment_method, payment_account, receipt_date, subtotal, tax, total, notes,
                        created_by, created_at) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#).bind(receipt.to_string()).bind(order.destination_location_id.to_string()).bind(order.vendor_id.to_string()).bind(id.to_string()).bind(dto.invoice_reference.trim()).bind(&dto.payment_method).bind(&dto.payment_account).bind(&date).bind(subtotal).bind(tax).bind(total).bind(dto.notes).bind(actor.to_string()).bind(&ts).execute(&mut *c).await.map_err(|e| match &e {
                        sqlx::Error::Database(d) if d.is_unique_violation() => AppError::conflict_code("PURCHASE_ORDER_RECEIPT_DUPLICATE", None),
                        _ => AppError::Database(e),
                    })?;
                    let receipt_snapshot=json!({"id":receipt,"locationId":order.destination_location_id,"vendorId":order.vendor_id,"purchaseOrderId":id,"invoiceReference":dto.invoice_reference.trim(),"receiptDate":date,"subtotal":money_f64(subtotal)?,"tax":money_f64(tax)?,"total":money_f64(total)?,"paymentMethod":dto.payment_method,"lines":lines.iter().map(|(line,accepted,rejected,_,pieces,cost,rate,total)|json!({"purchaseOrderLineId":line.id,"productId":line.product_id,"acceptedBoxes":accepted,"rejectedBoxes":rejected,"piecesAdded":pieces,"boxCostSnapshotScale4":cost,"taxRateScale4":rate,"lineTotalScale4":total})).collect::<Vec<_>>()});
                    for (line, accepted, rejected, delivered, pieces, cost, rate, total) in lines {
                        // box_quantity stores the delivery;
                        // Public boxQuantity continues to mean accepted boxes.
                        sqlx::query(r#"
                        INSERT INTO stock_receipt_lines(id, receipt_id, product_id, purchase_order_line_id, box_quantity,
                        accepted_box_quantity, rejected_box_quantity, pieces_added, box_cost_snapshot, tax_rate, line_total)
                        VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#).bind(Uuid::now_v7().to_string()).bind(receipt.to_string()).bind(line.product_id.to_string()).bind(line.id.to_string()).bind(delivered).bind(accepted).bind(rejected).bind(pieces).bind(cost).bind(rate).bind(total).execute(&mut *c).await?;
                        TenantInventoryRepository::adjust_stock(c, order.destination_location_id, line.product_id, pieces, "receipt", receipt, "purchase_order_receipt", Some(actor)).await?;
                        sqlx::query("UPDATE purchase_order_lines SET received_boxes=received_boxes+? WHERE id=?").bind(accepted).bind(line.id.to_string()).execute(&mut *c).await?;
                    }
                    let remaining: i64 = sqlx::query_scalar("SELECT SUM(ordered_boxes-received_boxes) FROM purchase_order_lines WHERE purchase_order_id=?").bind(id.to_string()).fetch_one(&mut *c).await?;
                    sqlx::query("UPDATE purchase_orders SET status=?,version=version+1,updated_at=? WHERE id=?").bind(if remaining == 0 { "received" } else { "partially_received" }).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
                    // Stable category identity. Existing custom categories, including an inactive one, are preserved.
                    const CATEGORY: &str = "00000000-0000-4000-8000-000000000047";
                    sqlx::query("INSERT OR IGNORE INTO expense_categories(id,name,created_at,updated_at) VALUES(?,'Inventory purchases',?,?)").bind(CATEGORY).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    let category: String = sqlx::query_scalar("SELECT id FROM expense_categories WHERE lower(name)='inventory purchases' AND is_active=1").fetch_optional(&mut *c).await?.ok_or_else(|| AppError::conflict_code("INVENTORY_EXPENSE_CATEGORY_REQUIRED", None))?;
                    let (shift, register) = if dto.payment_method == "cash" {
                        let row: Option<(String, String)> = sqlx::query_as(r#"
                        SELECT s.id, cr.id FROM shifts s JOIN cash_registers cr ON cr.shift_id=s.id AND cr.status= 'open'
                        WHERE s.user_id=? AND s.status= 'active' AND s.location_id=?
                    "#).bind(actor.to_string()).bind(location.venue_location_id.to_string()).fetch_optional(&mut *c).await?;
                        let (s, r) = row.ok_or_else(|| AppError::conflict_code("CASH_REGISTER_REQUIRED", None))?;
                        (Some(s), Some(r))
                    } else {
                        (None, None)
                    };
                    let expense = Uuid::now_v7();
                    let entry = register.as_ref().map(|_| Uuid::now_v7());
                    if let (Some(register), Some(entry)) = (register, entry) {
                        sqlx::query(r#"
                        INSERT INTO cash_register_entries(id, cash_register_id, entry_type, amount, reason, reference_id,
                        reference_type, created_by, created_at) VALUES(?, ?, 'cash_out' , ?, ?, ?, 'expense' , ?, ?)
                    "#).bind(entry.to_string()).bind(&register).bind(total).bind(format!("Inventory purchase {}", order.po_number)).bind(expense.to_string()).bind(actor.to_string()).bind(&ts).execute(&mut *c).await?;
                        event(c,"cash_register_entry",entry,"cash_register.entry_created",Some(location.venue_location_id),false,json!({"id":entry,"cashRegisterId":register,"entryType":"cash_out","amount":money_f64(total)?,"referenceId":expense,"referenceType":"expense"})).await?;
                    }
                    sqlx::query(r#"
                        INSERT INTO expenses(id, category_id, vendor_id, amount, payment_method, payment_account,
                        description, expense_date, approval_status, approved_by, approved_at, shift_id,
                        cash_register_entry_id, source_type, source_id, created_by, updated_by, created_at, updated_at)
                        VALUES(?, ?, ?, ?, ?, ?, ?, ?, 'approved' , ?, ?, ?, ?, 'stock_receipt' , ?, ?, ?, ?, ?)
                    "#).bind(expense.to_string()).bind(category).bind(order.vendor_id.to_string()).bind(total).bind(dto.payment_method).bind(dto.payment_account).bind(format!("Inventory receipt {} for {}", dto.invoice_reference.trim(), order.po_number)).bind(date).bind(approver.to_string()).bind(&ts).bind(shift).bind(entry.map(|x| x.to_string())).bind(receipt.to_string()).bind(actor.to_string()).bind(actor.to_string()).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    event(c, "expense", expense, "expense.created", Some(location.venue_location_id), false, json!({"id":expense,"amount":money_f64(total)?,"sourceType":"stock_receipt","sourceId":receipt,"approvalStatus":"approved"})).await?;
                    event(c,"stock_receipt",receipt,"inventory.receipt_created",Some(location.venue_location_id),false,receipt_snapshot).await?;
                    let purchase_order = Self::get(c, id).await?;
                    let result = ReceivePurchaseOrderResponse { purchase_order, receipt_id: receipt, expense_id: expense };
                    event(c, "purchase_order", id, "procurement.order_received", Some(location.venue_location_id), false, json!(result)).await?;
                    Ok(result)
                })
            }),
        )
        .await
    }
    pub async fn list_reorder_rules(&self) -> Result<Vec<InventoryReorderRule>, AppError> {
        Ok(sqlx::query_as::<_, InventoryReorderRule>(
            r#"
                        SELECT unhex(replace(r.id, '-' , '' )) AS id, unhex(replace(r.inventory_location_id, '-' , '' )) AS
                        location_id, unhex(replace(r.product_id, '-' , '' )) AS product_id, p.name as product_name,
                        r.minimum_pieces as minimum_pieces, r.target_pieces as target_pieces,
                        unhex(replace(r.preferred_vendor_id, '-' , '' )) AS preferred_vendor_id, r.lead_time_days as
                        lead_time_days, r.is_active as is_active, r.created_at as created_at, r.updated_at as updated_at
                        FROM inventory_reorder_rules r JOIN products p ON p.id=r.product_id ORDER BY p.name
                    "#,
        )
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }
    pub async fn reorder_suggestions(&self) -> Result<Vec<ReorderSuggestion>, AppError> {
        Ok(sqlx::query_as::<_, ReorderSuggestion>(
            r#"
                        SELECT unhex(replace(r.id, '-' , '' )) AS rule_id, unhex(replace(r.inventory_location_id, '-' , ''
                        )) AS location_id, l.name as location_name, unhex(replace(r.product_id, '-' , '' )) AS product_id,
                        p.name as product_name, COALESCE(ls.quantity_pieces, 0) as current_pieces, r.minimum_pieces as
                        minimum_pieces, r.target_pieces as target_pieces, MAX(r.target_pieces-COALESCE(ls.quantity_pieces,
                        0), 0) as suggested_pieces, unhex(replace(r.preferred_vendor_id, '-' , '' )) AS preferred_vendor_id,
                        r.lead_time_days as lead_time_days FROM inventory_reorder_rules r JOIN inventory_locations l ON
                        l.id=r.inventory_location_id JOIN products p ON p.id=r.product_id LEFT JOIN location_stock ls ON
                        ls.inventory_location_id=r.inventory_location_id AND ls.product_id=r.product_id WHERE
                        r.is_active=true AND l.deleted_at IS NULL AND l.is_active=1 AND p.deleted_at IS NULL AND
                        COALESCE(ls.quantity_pieces, 0) <= r.minimum_pieces ORDER BY
                        (r.minimum_pieces-COALESCE(ls.quantity_pieces, 0)) DESC, p.name
                    "#,
        )
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }
    pub async fn list_movements(
        &self,
        filters: StockMovementFilterDto,
    ) -> Result<PaginationResult<StockMovementRow>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(30).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);
        let base = " FROM stock_movements m JOIN inventory_locations l ON l.id=m.inventory_location_id JOIN products p ON p.id=m.product_id WHERE 1=1";
        let mut q = QueryBuilder::<Sqlite>::new(format!(
            r#"
                        SELECT unhex(replace(m.id, '-' , '' )) AS id, unhex(replace(m.inventory_location_id, '-' , '' )) AS
                        location_id, l.name as location_name, unhex(replace(m.product_id, '-' , '' )) AS product_id, p.name
                        as product_name, m.delta, m.movement_type as movement_type, unhex(replace(m.reference_id, '-' , ''
                        )) AS reference_id, m.reference_type as reference_type, unhex(replace(m.created_by, '-' , '' )) AS
                        created_by, m.created_at as created_at{base}
                    "#
        ));
        Self::apply_movement_filters(&mut q, &filters)?;
        q.push(" ORDER BY m.created_at DESC,m.id DESC LIMIT ");
        q.push_bind(limit);
        q.push(" OFFSET ");
        q.push_bind(offset);
        let data = q
            .build_query_as::<StockMovementRow>()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut c = QueryBuilder::<Sqlite>::new(format!("SELECT COUNT(*){base}"));
        Self::apply_movement_filters(&mut c, &filters)?;
        let total: (i64,) = c.build_query_as().fetch_one(&self.db.read_pool()?).await?;
        Ok(PaginationResult::new(data, total.0, page, limit))
    }
    pub async fn upsert_reorder_rule(
        &self,
        dto: UpsertInventoryReorderRuleDto,
        actor: Uuid,
    ) -> Result<InventoryReorderRule, AppError> {
        if dto.minimum_pieces < 0
            || dto.target_pieces < dto.minimum_pieces
            || dto.lead_time_days.is_some_and(|x| x < 0)
        {
            return Err(AppError::bad_request_code("REORDER_RULE_INVALID", None));
        }
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let location = TenantInventoryRepository::location(c, dto.location_id).await?;
                    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM products WHERE id=? AND deleted_at IS NULL)").bind(dto.product_id.to_string()).fetch_one(&mut *c).await?;
                    if !valid {
                        return Err(AppError::bad_request_code("REORDER_RULE_INVALID", None));
                    }
                    if let Some(vendor) = dto.preferred_vendor_id {
                        Self::references(c, vendor, dto.location_id).await?;
                    }
                    let ts = now()?;
                    let id: String = sqlx::query_scalar(r#"
                        INSERT INTO inventory_reorder_rules(id, inventory_location_id, product_id, minimum_pieces,
                        target_pieces, preferred_vendor_id, lead_time_days, is_active, created_by, updated_by, created_at,
                        updated_at) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(inventory_location_id,
                        product_id) DO UPDATE SET minimum_pieces=excluded.minimum_pieces,
                        target_pieces=excluded.target_pieces, preferred_vendor_id=excluded.preferred_vendor_id,
                        lead_time_days=excluded.lead_time_days, is_active=excluded.is_active,
                        updated_by=excluded.updated_by, updated_at=excluded.updated_at RETURNING id
                    "#).bind(Uuid::now_v7().to_string()).bind(dto.location_id.to_string()).bind(dto.product_id.to_string()).bind(dto.minimum_pieces).bind(dto.target_pieces).bind(dto.preferred_vendor_id.map(|x| x.to_string())).bind(dto.lead_time_days.unwrap_or(0)).bind(dto.is_active.unwrap_or(true)).bind(actor.to_string()).bind(actor.to_string()).bind(&ts).bind(&ts).fetch_one(&mut *c).await?;
                    let row: InventoryReorderRule = sqlx::query_as(r#"
                        SELECT unhex(replace(r.id, '-' , '' )) AS id, unhex(replace(r.inventory_location_id, '-' , '' )) AS
                        location_id, unhex(replace(r.product_id, '-' , '' )) AS product_id, p.name AS product_name,
                        r.minimum_pieces, r.target_pieces, unhex(replace(r.preferred_vendor_id, '-' , '' )) AS
                        preferred_vendor_id, r.lead_time_days, r.is_active, r.created_at, r.updated_at FROM
                        inventory_reorder_rules r JOIN products p ON p.id=r.product_id WHERE r.id=?
                    "#).bind(&id).fetch_one(&mut *c).await?;
                    event(c, "inventory_reorder_rule", row.id, "inventory.reorder_rule_updated", Some(location.venue_location_id), false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn overview(&self) -> Result<InventoryOverviewDto, AppError> {
        Err(AppError::Api {
            code: "ANALYTICS_UNAVAILABLE".into(),
            status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
            details: None,
        })
    }
    fn apply_movement_filters(
        q: &mut QueryBuilder<Sqlite>,
        f: &StockMovementFilterDto,
    ) -> Result<(), AppError> {
        for (column, value) in [
            ("inventory_location_id", f.location_id),
            ("product_id", f.product_id),
            ("reference_id", f.reference_id),
            ("created_by", f.actor_id),
        ] {
            if let Some(id) = value {
                q.push(format!(" AND m.{column}="))
                    .push_bind(id.to_string());
            }
        }
        for (column, value) in [
            ("movement_type", &f.movement_type),
            ("reference_type", &f.reference_type),
        ] {
            if let Some(value) = value {
                q.push(format!(" AND m.{column}=")).push_bind(value.clone());
            }
        }
        if let Some(date) = f.from {
            q.push(" AND m.created_at>=").push_bind(
                crate::tenancy::format_sqlite_timestamp(&date)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        if let Some(date) = f.to {
            q.push(" AND m.created_at<=").push_bind(
                crate::tenancy::format_sqlite_timestamp(&date)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        Ok(())
    }
}
