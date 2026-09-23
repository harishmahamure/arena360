use std::collections::HashSet;

use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreatePurchaseOrderDto, InventoryOverviewDto, InventoryReorderRule, PurchaseOrder,
    PurchaseOrderFilterDto, PurchaseOrderLine, PurchaseOrderLineInput, PurchaseOrderWithLines,
    ReceivePurchaseOrderDto, ReceivePurchaseOrderResponse, ReorderSuggestion,
    StockMovementFilterDto, StockMovementRow, UpdatePurchaseOrderDto,
    UpsertInventoryReorderRuleDto,
};

#[derive(Clone)]
pub struct ProcurementService {
    pool: PgPool,
}

impl ProcurementService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const ORDER_SELECT: &'static str = r#"
        SELECT id, "poNumber" as po_number, "vendorId" as vendor_id,
               "destinationLocationId" as destination_location_id, status::text as status,
               "expectedDeliveryDate" as expected_delivery_date,
               subtotal::float8 as subtotal, discount::float8 as discount,
               tax::float8 as tax, freight::float8 as freight, total::float8 as total,
               notes, "rejectionReason" as rejection_reason, version,
               "createdBy" as created_by, "submittedBy" as submitted_by,
               "submittedAt" as submitted_at, "approvedBy" as approved_by,
               "approvedAt" as approved_at, "orderedBy" as ordered_by,
               "orderedAt" as ordered_at, "cancelledBy" as cancelled_by,
               "cancelledAt" as cancelled_at, "createdAt" as created_at,
               "updatedAt" as updated_at
        FROM purchase_orders
    "#;

    const LINE_SELECT: &'static str = r#"
        SELECT pol.id, pol."purchaseOrderId" as purchase_order_id,
               pol."productId" as product_id, p.name as product_name, p.sku as product_sku,
               pol."orderedBoxes" as ordered_boxes, pol."receivedBoxes" as received_boxes,
               pol."unitsPerBoxSnapshot" as units_per_box_snapshot,
               pol."boxCostSnapshot"::float8 as box_cost_snapshot,
               pol."taxRate"::float8 as tax_rate,
               pol."lineSubtotal"::float8 as line_subtotal,
               pol."lineTax"::float8 as line_tax,
               pol."lineTotal"::float8 as line_total
        FROM purchase_order_lines pol
        JOIN products p ON p.id = pol."productId"
    "#;

    fn transition_spec(
        action: &str,
    ) -> Option<(
        &'static [&'static str],
        &'static str,
        &'static str,
        &'static str,
    )> {
        match action {
            "submit" => Some((
                &["draft", "rejected"],
                "submitted",
                "submittedBy",
                "submittedAt",
            )),
            "approve" => Some((&["submitted"], "approved", "approvedBy", "approvedAt")),
            "reject" => Some((&["submitted"], "rejected", "approvedBy", "approvedAt")),
            "mark_ordered" => Some((&["approved"], "ordered", "orderedBy", "orderedAt")),
            "cancel" => Some((
                &["draft", "submitted", "approved", "ordered"],
                "cancelled",
                "cancelledBy",
                "cancelledAt",
            )),
            _ => None,
        }
    }

    pub async fn list_orders(
        &self,
        filters: PurchaseOrderFilterDto,
    ) -> Result<PaginationResult<PurchaseOrder>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;
        let mut query = QueryBuilder::<Postgres>::new(Self::ORDER_SELECT);
        query.push(" WHERE 1=1");
        Self::apply_order_filters(&mut query, &filters);
        query.push(" ORDER BY \"updatedAt\" DESC LIMIT ");
        query.push_bind(limit);
        query.push(" OFFSET ");
        query.push_bind(offset);
        let data = query
            .build_query_as::<PurchaseOrder>()
            .fetch_all(&self.pool)
            .await?;

        let mut count =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM purchase_orders WHERE 1=1");
        Self::apply_order_filters(&mut count, &filters);
        let total: (i64,) = count.build_query_as().fetch_one(&self.pool).await?;
        Ok(PaginationResult::new(data, total.0, page, limit))
    }

    fn apply_order_filters<'a>(
        query: &mut QueryBuilder<'a, Postgres>,
        filters: &'a PurchaseOrderFilterDto,
    ) {
        if let Some(status) = &filters.status {
            query.push(" AND status = ");
            query.push_bind(status);
            query.push("::purchase_order_status");
        }
        if let Some(vendor_id) = filters.vendor_id {
            query.push(" AND \"vendorId\" = ");
            query.push_bind(vendor_id);
        }
        if let Some(location_id) = filters.destination_location_id {
            query.push(" AND \"destinationLocationId\" = ");
            query.push_bind(location_id);
        }
        if let Some(search) = &filters.search {
            query.push(" AND \"poNumber\" ILIKE ");
            query.push_bind(format!("%{}%", search.trim()));
        }
    }

    pub async fn get_order(&self, id: Uuid) -> Result<PurchaseOrderWithLines, AppError> {
        let order_query = format!("{} WHERE id = $1", Self::ORDER_SELECT);
        let order = sqlx::query_as::<_, PurchaseOrder>(&order_query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::not_found_code("PURCHASE_ORDER_NOT_FOUND"))?;
        let line_query = format!(
            "{} WHERE pol.\"purchaseOrderId\" = $1 ORDER BY p.name",
            Self::LINE_SELECT
        );
        let lines = sqlx::query_as::<_, PurchaseOrderLine>(&line_query)
            .bind(id)
            .fetch_all(&self.pool)
            .await?;
        Ok(PurchaseOrderWithLines { order, lines })
    }

    pub async fn create_order(
        &self,
        dto: CreatePurchaseOrderDto,
        actor_id: Uuid,
    ) -> Result<PurchaseOrderWithLines, AppError> {
        Self::validate_lines(&dto.lines)?;
        Self::validate_financials(dto.discount.unwrap_or(0.0), dto.freight.unwrap_or(0.0))?;
        let mut tx = self.pool.begin().await?;
        Self::validate_vendor_and_location(&mut tx, dto.vendor_id, dto.destination_location_id)
            .await?;
        let (subtotal, tax, total) = Self::totals(
            &dto.lines,
            dto.discount.unwrap_or(0.0),
            dto.freight.unwrap_or(0.0),
        );
        let po_number: String = sqlx::query_scalar(
            r#"SELECT 'PO-' || to_char(CURRENT_DATE, 'YYYY') || '-' || lpad(nextval('purchase_order_number_seq')::text, 6, '0')"#,
        )
        .fetch_one(&mut *tx)
        .await?;
        let order = sqlx::query_as::<_, PurchaseOrder>(&format!(
            r#"INSERT INTO purchase_orders
                (id, "poNumber", "vendorId", "destinationLocationId", status,
                 "expectedDeliveryDate", subtotal, discount, tax, freight, total, notes,
                 "createdBy", "createdAt", "updatedAt")
               VALUES (gen_random_uuid(), $1, $2, $3, 'draft', $4, $5, $6, $7, $8, $9, $10, $11, NOW(), NOW())
               RETURNING {}"#,
            Self::order_columns()
        ))
        .bind(po_number)
        .bind(dto.vendor_id)
        .bind(dto.destination_location_id)
        .bind(dto.expected_delivery_date)
        .bind(subtotal)
        .bind(dto.discount.unwrap_or(0.0))
        .bind(tax)
        .bind(dto.freight.unwrap_or(0.0))
        .bind(total)
        .bind(dto.notes)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await?;
        Self::insert_lines(&mut tx, order.id, &dto.lines).await?;
        tx.commit().await?;
        self.get_order(order.id).await
    }

    pub async fn update_order(
        &self,
        id: Uuid,
        dto: UpdatePurchaseOrderDto,
        actor_id: Uuid,
    ) -> Result<PurchaseOrderWithLines, AppError> {
        if let Some(lines) = &dto.lines {
            Self::validate_lines(lines)?;
        }
        let current = self.get_order(id).await?;
        if !matches!(current.order.status.as_str(), "draft" | "rejected") {
            return Err(AppError::conflict_code("PURCHASE_ORDER_NOT_EDITABLE", None));
        }
        if current.order.version != dto.version {
            return Err(AppError::conflict_code(
                "PURCHASE_ORDER_VERSION_CONFLICT",
                None,
            ));
        }
        let vendor_id = dto.vendor_id.unwrap_or(current.order.vendor_id);
        let location_id = dto
            .destination_location_id
            .unwrap_or(current.order.destination_location_id);
        let lines_for_totals = match &dto.lines {
            Some(lines) => lines.clone(),
            None => current
                .lines
                .iter()
                .map(|line| PurchaseOrderLineInput {
                    product_id: line.product_id,
                    ordered_boxes: line.ordered_boxes,
                    box_cost: line.box_cost_snapshot,
                    tax_rate: Some(line.tax_rate),
                })
                .collect(),
        };
        let discount = dto.discount.unwrap_or(current.order.discount);
        let freight = dto.freight.unwrap_or(current.order.freight);
        Self::validate_financials(discount, freight)?;
        let (subtotal, tax, total) = Self::totals(&lines_for_totals, discount, freight);
        let mut tx = self.pool.begin().await?;
        Self::validate_vendor_and_location(&mut tx, vendor_id, location_id).await?;
        let updated: Option<Uuid> = sqlx::query_scalar(
            r#"UPDATE purchase_orders SET "vendorId"=$3, "destinationLocationId"=$4,
                 "expectedDeliveryDate"=COALESCE($5,"expectedDeliveryDate"), discount=$6,
                 freight=$7, subtotal=$8, tax=$9, total=$10, notes=COALESCE($11,notes),
                 status='draft', "rejectionReason"=NULL, version=version+1, "updatedAt"=NOW()
               WHERE id=$1 AND version=$2 AND status IN ('draft','rejected') RETURNING id"#,
        )
        .bind(id)
        .bind(dto.version)
        .bind(vendor_id)
        .bind(location_id)
        .bind(dto.expected_delivery_date)
        .bind(discount)
        .bind(freight)
        .bind(subtotal)
        .bind(tax)
        .bind(total)
        .bind(dto.notes)
        .fetch_optional(&mut *tx)
        .await?;
        if updated.is_none() {
            return Err(AppError::conflict_code(
                "PURCHASE_ORDER_VERSION_CONFLICT",
                None,
            ));
        }
        if dto.lines.is_some() {
            sqlx::query(r#"DELETE FROM purchase_order_lines WHERE "purchaseOrderId"=$1"#)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            Self::insert_lines(&mut tx, id, &lines_for_totals).await?;
        }
        let _ = actor_id;
        tx.commit().await?;
        self.get_order(id).await
    }

    pub async fn transition(
        &self,
        id: Uuid,
        action: &str,
        actor_id: Uuid,
        reason: Option<String>,
    ) -> Result<PurchaseOrderWithLines, AppError> {
        let (from, to, actor_column, at_column) = Self::transition_spec(action)
            .ok_or_else(|| AppError::bad_request_code("PURCHASE_ORDER_INVALID_ACTION", None))?;
        if action == "reject" && reason.as_deref().unwrap_or("").trim().len() < 3 {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_REJECTION_REASON_REQUIRED",
                None,
            ));
        }
        let query = format!(
            "UPDATE purchase_orders SET status=$2::purchase_order_status, \"{actor_column}\"=$3, \"{at_column}\"=NOW(), \"rejectionReason\"=$4, version=version+1, \"updatedAt\"=NOW() WHERE id=$1 AND status::text = ANY($5) RETURNING id"
        );
        let updated: Option<Uuid> = sqlx::query_scalar(&query)
            .bind(id)
            .bind(to)
            .bind(actor_id)
            .bind(if action == "reject" { reason } else { None })
            .bind(from)
            .fetch_optional(&self.pool)
            .await?;
        if updated.is_none() {
            return Err(AppError::conflict_code(
                "PURCHASE_ORDER_INVALID_TRANSITION",
                None,
            ));
        }
        self.get_order(id).await
    }

    pub async fn receive(
        &self,
        id: Uuid,
        dto: ReceivePurchaseOrderDto,
        actor_id: Uuid,
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
        let mut tx = self.pool.begin().await?;
        let order_query = format!("{} WHERE id=$1 FOR UPDATE", Self::ORDER_SELECT);
        let order = sqlx::query_as::<_, PurchaseOrder>(&order_query)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::not_found_code("PURCHASE_ORDER_NOT_FOUND"))?;
        if !matches!(
            order.status.as_str(),
            "approved" | "ordered" | "partially_received"
        ) {
            return Err(AppError::conflict_code(
                "PURCHASE_ORDER_NOT_RECEIVABLE",
                None,
            ));
        }
        let approved_by = order
            .approved_by
            .ok_or_else(|| AppError::conflict_code("PURCHASE_ORDER_NOT_APPROVED", None))?;
        let receipt_id = Uuid::new_v4();
        let mut receipt_subtotal = 0.0;
        let mut receipt_tax = 0.0;
        let mut receipt_lines = Vec::new();
        let mut receipt_line_ids = HashSet::new();
        for input in &dto.lines {
            if !receipt_line_ids.insert(input.purchase_order_line_id) {
                return Err(AppError::bad_request_code(
                    "PURCHASE_ORDER_RECEIPT_DUPLICATE_LINE",
                    None,
                ));
            }
            let line = sqlx::query_as::<_, PurchaseOrderLine>(&format!(
                "{} WHERE pol.id=$1 AND pol.\"purchaseOrderId\"=$2 FOR UPDATE",
                Self::LINE_SELECT
            ))
            .bind(input.purchase_order_line_id)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::bad_request_code("PURCHASE_ORDER_LINE_NOT_FOUND", None))?;
            let outstanding = line.ordered_boxes - line.received_boxes;
            Self::validate_receipt_quantity(
                input.accepted_boxes,
                input.rejected_boxes.unwrap_or(0),
                outstanding,
            )?;
            let line_subtotal = input.accepted_boxes as f64 * line.box_cost_snapshot;
            let line_tax = line_subtotal * line.tax_rate / 100.0;
            receipt_subtotal += line_subtotal;
            receipt_tax += line_tax;
            receipt_lines.push((
                line,
                input.accepted_boxes,
                input.rejected_boxes.unwrap_or(0),
                line_subtotal + line_tax,
            ));
        }
        let receipt_total = receipt_subtotal + receipt_tax;
        sqlx::query(
            r#"INSERT INTO stock_receipts
                (id,"locationId","vendorId",notes,"createdBy","createdAt","purchaseOrderId",
                 "invoiceReference","paymentMethod","paymentAccount","receiptDate",subtotal,tax,total)
               VALUES ($1,$2,$3,$4,$5,NOW(),$6,$7,$8,$9,COALESCE($10,NOW()),$11,$12,$13)"#,
        ).bind(receipt_id).bind(order.destination_location_id).bind(order.vendor_id)
          .bind(&dto.notes).bind(actor_id).bind(id).bind(dto.invoice_reference.trim())
          .bind(&dto.payment_method).bind(&dto.payment_account).bind(dto.receipt_date)
          .bind(receipt_subtotal).bind(receipt_tax).bind(receipt_total).execute(&mut *tx).await
          .map_err(|error| match &error {
              sqlx::Error::Database(db) if db.is_unique_violation() => AppError::conflict_code("PURCHASE_ORDER_RECEIPT_DUPLICATE", None),
              _ => AppError::Database(error),
          })?;

        for (line, accepted, rejected, line_total) in &receipt_lines {
            let pieces = accepted * line.units_per_box_snapshot;
            sqlx::query(
                r#"INSERT INTO stock_receipt_lines
                    (id,"receiptId","productId","boxQuantity","piecesAdded","purchaseOrderLineId",
                     "acceptedBoxQuantity","rejectedBoxQuantity","boxCostSnapshot","taxRate","lineTotal")
                   VALUES (gen_random_uuid(),$1,$2,$3,$4,$5,$3,$6,$7,$8,$9)"#,
            ).bind(receipt_id).bind(line.product_id).bind(*accepted).bind(pieces).bind(line.id)
              .bind(*rejected).bind(line.box_cost_snapshot).bind(line.tax_rate).bind(*line_total)
              .execute(&mut *tx).await?;
            sqlx::query(
                r#"INSERT INTO location_stock ("locationId","productId","quantityPieces","createdAt","updatedAt")
                   VALUES ($1,$2,$3,NOW(),NOW())
                   ON CONFLICT ("locationId","productId") DO UPDATE SET
                     "quantityPieces"=location_stock."quantityPieces"+EXCLUDED."quantityPieces", "updatedAt"=NOW()"#,
            ).bind(order.destination_location_id).bind(line.product_id).bind(pieces).execute(&mut *tx).await?;
            sqlx::query(
                r#"INSERT INTO stock_movements
                    (id,"locationId","productId",delta,"movementType","referenceId","referenceType","createdBy","createdAt")
                   VALUES (gen_random_uuid(),$1,$2,$3,'receipt',$4,'purchase_order_receipt',$5,NOW())"#,
            ).bind(order.destination_location_id).bind(line.product_id).bind(pieces).bind(receipt_id)
              .bind(actor_id).execute(&mut *tx).await?;
            sqlx::query(
                r#"UPDATE purchase_order_lines SET "receivedBoxes"="receivedBoxes"+$2 WHERE id=$1"#,
            )
            .bind(line.id)
            .bind(*accepted)
            .execute(&mut *tx)
            .await?;
        }

        let remaining: i64 = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM("orderedBoxes"-"receivedBoxes"),0) FROM purchase_order_lines WHERE "purchaseOrderId"=$1"#,
        ).bind(id).fetch_one(&mut *tx).await?;
        let new_status = if remaining == 0 {
            "received"
        } else {
            "partially_received"
        };
        sqlx::query(r#"UPDATE purchase_orders SET status=$2::purchase_order_status,version=version+1,"updatedAt"=NOW() WHERE id=$1"#)
            .bind(id).bind(new_status).execute(&mut *tx).await?;

        let category_id: Uuid = sqlx::query_scalar(
            r#"SELECT id FROM expense_categories WHERE lower(name)='inventory purchases' AND "isActive"=true LIMIT 1"#,
        ).fetch_one(&mut *tx).await?;
        let (shift_id, register_id) = if dto.payment_method == "cash" {
            let row: Option<(Uuid, Uuid)> = sqlx::query_as(
                r#"SELECT s.id, cr.id FROM shifts s JOIN cash_registers cr ON cr."shiftId"=s.id AND cr.status='open'
                   WHERE s."userId"=$1 AND s.status='active' LIMIT 1"#,
            ).bind(actor_id).fetch_optional(&mut *tx).await?;
            let (shift, register) =
                row.ok_or_else(|| AppError::conflict_code("CASH_REGISTER_REQUIRED", None))?;
            (Some(shift), Some(register))
        } else {
            (None, None)
        };
        let expense_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO expenses
                (id,"categoryId","vendorId",amount,"paymentMethod","paymentAccount",description,
                 "expenseDate","isRecurring","approvalStatus","approvedBy","approvedAt","shiftId",
                 "createdBy","updatedBy","createdAt","updatedAt","sourceType","sourceId")
               VALUES ($1,$2,$3,$4,$5,$6,$7,COALESCE($8,NOW()),false,'approved',$9,NOW(),$10,$11,$11,NOW(),NOW(),'stock_receipt',$12)"#,
        ).bind(expense_id).bind(category_id).bind(order.vendor_id).bind(receipt_total)
          .bind(&dto.payment_method).bind(&dto.payment_account)
          .bind(format!("Inventory receipt {} for {}", dto.invoice_reference.trim(), order.po_number))
          .bind(dto.receipt_date).bind(approved_by).bind(shift_id).bind(actor_id).bind(receipt_id)
          .execute(&mut *tx).await?;
        if let Some(register_id) = register_id {
            let entry_id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO cash_register_entries
                    (id,"cashRegisterId","entryType",amount,reason,"referenceId","referenceType","createdBy","createdAt")
                   VALUES ($1,$2,'cash_out',$3,$4,$5,'expense',$6,NOW())"#,
            ).bind(entry_id).bind(register_id).bind(receipt_total)
              .bind(format!("Inventory purchase {}", order.po_number)).bind(expense_id).bind(actor_id)
              .execute(&mut *tx).await?;
            sqlx::query(r#"UPDATE expenses SET "cashRegisterEntryId"=$2 WHERE id=$1"#)
                .bind(expense_id)
                .bind(entry_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(ReceivePurchaseOrderResponse {
            purchase_order: self.get_order(id).await?,
            receipt_id,
            expense_id,
        })
    }

    pub async fn list_reorder_rules(&self) -> Result<Vec<InventoryReorderRule>, AppError> {
        Ok(sqlx::query_as::<_, InventoryReorderRule>(
            r#"SELECT r.id,r."locationId" as location_id,r."productId" as product_id,
                      p.name as product_name,r."minimumPieces" as minimum_pieces,
                      r."targetPieces" as target_pieces,r."preferredVendorId" as preferred_vendor_id,
                      r."leadTimeDays" as lead_time_days,r."isActive" as is_active,
                      r."createdAt" as created_at,r."updatedAt" as updated_at
               FROM inventory_reorder_rules r JOIN products p ON p.id=r."productId"
               ORDER BY p.name"#,
        ).fetch_all(&self.pool).await?)
    }

    pub async fn upsert_reorder_rule(
        &self,
        dto: UpsertInventoryReorderRuleDto,
        actor_id: Uuid,
    ) -> Result<InventoryReorderRule, AppError> {
        if dto.minimum_pieces < 0 || dto.target_pieces < dto.minimum_pieces {
            return Err(AppError::bad_request_code("REORDER_RULE_INVALID", None));
        }
        Ok(sqlx::query_as::<_, InventoryReorderRule>(
            r#"INSERT INTO inventory_reorder_rules
                (id,"locationId","productId","minimumPieces","targetPieces","preferredVendorId",
                 "leadTimeDays","isActive","createdBy","updatedBy","createdAt","updatedAt")
               VALUES (gen_random_uuid(),$1,$2,$3,$4,$5,$6,$7,$8,$8,NOW(),NOW())
               ON CONFLICT ("locationId","productId") DO UPDATE SET
                 "minimumPieces"=EXCLUDED."minimumPieces","targetPieces"=EXCLUDED."targetPieces",
                 "preferredVendorId"=EXCLUDED."preferredVendorId","leadTimeDays"=EXCLUDED."leadTimeDays",
                 "isActive"=EXCLUDED."isActive","updatedBy"=$8,"updatedAt"=NOW()
               RETURNING id,"locationId" as location_id,"productId" as product_id,
                 (SELECT name FROM products WHERE id="productId") as product_name,
                 "minimumPieces" as minimum_pieces,"targetPieces" as target_pieces,
                 "preferredVendorId" as preferred_vendor_id,"leadTimeDays" as lead_time_days,
                 "isActive" as is_active,"createdAt" as created_at,"updatedAt" as updated_at"#,
        ).bind(dto.location_id).bind(dto.product_id).bind(dto.minimum_pieces).bind(dto.target_pieces)
          .bind(dto.preferred_vendor_id).bind(dto.lead_time_days.unwrap_or(0))
          .bind(dto.is_active.unwrap_or(true)).bind(actor_id).fetch_one(&self.pool).await?)
    }

    pub async fn reorder_suggestions(&self) -> Result<Vec<ReorderSuggestion>, AppError> {
        Ok(sqlx::query_as::<_, ReorderSuggestion>(
            r#"SELECT r.id as rule_id,r."locationId" as location_id,l.name as location_name,
                      r."productId" as product_id,p.name as product_name,
                      COALESCE(ls."quantityPieces",0)::int as current_pieces,
                      r."minimumPieces" as minimum_pieces,r."targetPieces" as target_pieces,
                      GREATEST(r."targetPieces"-COALESCE(ls."quantityPieces",0),0)::int as suggested_pieces,
                      r."preferredVendorId" as preferred_vendor_id,r."leadTimeDays" as lead_time_days
               FROM inventory_reorder_rules r
               JOIN inventory_locations l ON l.id=r."locationId"
               JOIN products p ON p.id=r."productId"
               LEFT JOIN location_stock ls ON ls."locationId"=r."locationId" AND ls."productId"=r."productId"
               WHERE r."isActive"=true AND COALESCE(ls."quantityPieces",0) <= r."minimumPieces"
               ORDER BY (r."minimumPieces"-COALESCE(ls."quantityPieces",0)) DESC,p.name"#,
        ).fetch_all(&self.pool).await?)
    }

    pub async fn list_movements(
        &self,
        filters: StockMovementFilterDto,
    ) -> Result<PaginationResult<StockMovementRow>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(30).clamp(1, 100);
        let offset = (page - 1) * limit;
        let base = " FROM stock_movements m JOIN inventory_locations l ON l.id=m.\"locationId\" JOIN products p ON p.id=m.\"productId\" WHERE 1=1";
        let mut q=QueryBuilder::<Postgres>::new(format!("SELECT m.id,m.\"locationId\" as location_id,l.name as location_name,m.\"productId\" as product_id,p.name as product_name,m.delta,m.\"movementType\"::text as movement_type,m.\"referenceId\" as reference_id,m.\"referenceType\" as reference_type,m.\"createdBy\" as created_by,m.\"createdAt\" as created_at{base}"));
        Self::apply_movement_filters(&mut q, &filters);
        q.push(" ORDER BY m.\"createdAt\" DESC LIMIT ");
        q.push_bind(limit);
        q.push(" OFFSET ");
        q.push_bind(offset);
        let data = q
            .build_query_as::<StockMovementRow>()
            .fetch_all(&self.pool)
            .await?;
        let mut c = QueryBuilder::<Postgres>::new(format!("SELECT COUNT(*){base}"));
        Self::apply_movement_filters(&mut c, &filters);
        let total: (i64,) = c.build_query_as().fetch_one(&self.pool).await?;
        Ok(PaginationResult::new(data, total.0, page, limit))
    }

    fn apply_movement_filters<'a>(
        q: &mut QueryBuilder<'a, Postgres>,
        f: &'a StockMovementFilterDto,
    ) {
        if let Some(v) = f.location_id {
            q.push(" AND m.\"locationId\"=");
            q.push_bind(v);
        }
        if let Some(v) = f.product_id {
            q.push(" AND m.\"productId\"=");
            q.push_bind(v);
        }
        if let Some(v) = &f.movement_type {
            q.push(" AND m.\"movementType\"=");
            q.push_bind(v);
            q.push("::stock_movement_type");
        }
        if let Some(v) = f.reference_id {
            q.push(" AND m.\"referenceId\"=");
            q.push_bind(v);
        }
        if let Some(v) = &f.reference_type {
            q.push(" AND m.\"referenceType\"=");
            q.push_bind(v);
        }
        if let Some(v) = f.actor_id {
            q.push(" AND m.\"createdBy\"=");
            q.push_bind(v);
        }
        if let Some(v) = f.from {
            q.push(" AND m.\"createdAt\">=");
            q.push_bind(v);
        }
        if let Some(v) = f.to {
            q.push(" AND m.\"createdAt\"<=");
            q.push_bind(v);
        }
    }

    pub async fn overview(&self) -> Result<InventoryOverviewDto, AppError> {
        let (pieces,value):(i64,f64)=sqlx::query_as(r#"SELECT COALESCE(SUM(ls."quantityPieces"),0)::bigint,COALESCE(SUM(ls."quantityPieces"*COALESCE(p."purchasePricePerBox"/NULLIF(p."unitsPerPurchaseUnit",0),p."purchasePrice",0)),0)::float8 FROM location_stock ls JOIN products p ON p.id=ls."productId""#).fetch_one(&self.pool).await?;
        let low:i64=sqlx::query_scalar(r#"SELECT COUNT(*) FROM inventory_reorder_rules r LEFT JOIN location_stock ls ON ls."locationId"=r."locationId" AND ls."productId"=r."productId" WHERE r."isActive"=true AND COALESCE(ls."quantityPieces",0)>0 AND COALESCE(ls."quantityPieces",0)<=r."minimumPieces""#).fetch_one(&self.pool).await?;
        let out:i64=sqlx::query_scalar(r#"SELECT COUNT(*) FROM inventory_reorder_rules r LEFT JOIN location_stock ls ON ls."locationId"=r."locationId" AND ls."productId"=r."productId" WHERE r."isActive"=true AND COALESCE(ls."quantityPieces",0)=0"#).fetch_one(&self.pool).await?;
        let po:i64=sqlx::query_scalar("SELECT COUNT(*) FROM purchase_orders WHERE status IN ('submitted','approved','ordered','partially_received')").fetch_one(&self.pool).await?;
        let transfers: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM stock_transfer_requests WHERE status IN ('pending','approved')",
        )
        .fetch_one(&self.pool)
        .await?;
        let waste: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM stock_waste_events WHERE status='pending'")
                .fetch_one(&self.pool)
                .await?;
        let recent = self
            .list_movements(StockMovementFilterDto {
                limit: Some(8),
                ..Default::default()
            })
            .await?
            .data;
        Ok(InventoryOverviewDto {
            total_pieces: pieces,
            estimated_stock_value: value,
            low_stock_products: low,
            out_of_stock_products: out,
            open_purchase_orders: po,
            pending_transfers: transfers,
            pending_waste_events: waste,
            recent_movements: recent,
        })
    }

    fn validate_lines(lines: &[PurchaseOrderLineInput]) -> Result<(), AppError> {
        let mut product_ids = HashSet::new();
        if lines.is_empty()
            || lines.iter().any(|l| {
                !product_ids.insert(l.product_id)
                    || l.ordered_boxes <= 0
                    || l.box_cost < 0.0
                    || l.tax_rate.unwrap_or(0.0) < 0.0
            })
        {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_LINES_INVALID",
                None,
            ));
        }
        Ok(())
    }
    fn validate_financials(discount: f64, freight: f64) -> Result<(), AppError> {
        if discount < 0.0 || freight < 0.0 {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_FINANCIALS_INVALID",
                None,
            ));
        }
        Ok(())
    }
    fn validate_receipt_quantity(
        accepted: i32,
        rejected: i32,
        outstanding: i32,
    ) -> Result<(), AppError> {
        if accepted <= 0 || rejected < 0 {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_RECEIPT_QUANTITY_INVALID",
                None,
            ));
        }
        if i64::from(accepted) + i64::from(rejected) > i64::from(outstanding) {
            return Err(AppError::conflict_code("PURCHASE_ORDER_OVER_RECEIPT", None));
        }
        Ok(())
    }
    fn totals(lines: &[PurchaseOrderLineInput], discount: f64, freight: f64) -> (f64, f64, f64) {
        let subtotal = lines
            .iter()
            .map(|l| l.ordered_boxes as f64 * l.box_cost)
            .sum::<f64>();
        let tax = lines
            .iter()
            .map(|l| l.ordered_boxes as f64 * l.box_cost * l.tax_rate.unwrap_or(0.0) / 100.0)
            .sum::<f64>();
        (
            subtotal,
            tax,
            (subtotal - discount + tax + freight).max(0.0),
        )
    }
    async fn validate_vendor_and_location(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        vendor: Uuid,
        location: Uuid,
    ) -> Result<(), AppError> {
        let valid_vendor: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM vendors WHERE id=$1 AND "isActive"=true)"#,
        )
        .bind(vendor)
        .fetch_one(&mut **tx)
        .await?;
        let valid_location:bool=sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM inventory_locations WHERE id=$1 AND "isActive"=true AND "deletedAt" IS NULL)"#).bind(location).fetch_one(&mut **tx).await?;
        if !valid_vendor || !valid_location {
            return Err(AppError::bad_request_code(
                "PURCHASE_ORDER_REFERENCE_INVALID",
                None,
            ));
        }
        Ok(())
    }
    async fn insert_lines(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        order_id: Uuid,
        lines: &[PurchaseOrderLineInput],
    ) -> Result<(), AppError> {
        for line in lines {
            let units:i32=sqlx::query_scalar(r#"SELECT "unitsPerPurchaseUnit" FROM products WHERE id=$1 AND "deletedAt" IS NULL"#).bind(line.product_id).fetch_optional(&mut **tx).await?.ok_or_else(||AppError::bad_request_code("PURCHASE_ORDER_PRODUCT_INVALID",None))?;
            let subtotal = line.ordered_boxes as f64 * line.box_cost;
            let tax = subtotal * line.tax_rate.unwrap_or(0.0) / 100.0;
            sqlx::query(r#"INSERT INTO purchase_order_lines(id,"purchaseOrderId","productId","orderedBoxes","receivedBoxes","unitsPerBoxSnapshot","boxCostSnapshot","taxRate","lineSubtotal","lineTax","lineTotal") VALUES(gen_random_uuid(),$1,$2,$3,0,$4,$5,$6,$7,$8,$9)"#).bind(order_id).bind(line.product_id).bind(line.ordered_boxes).bind(units).bind(line.box_cost).bind(line.tax_rate.unwrap_or(0.0)).bind(subtotal).bind(tax).bind(subtotal+tax).execute(&mut **tx).await?;
        }
        Ok(())
    }
    fn order_columns() -> &'static str {
        r#"id,"poNumber" as po_number,"vendorId" as vendor_id,"destinationLocationId" as destination_location_id,status::text as status,"expectedDeliveryDate" as expected_delivery_date,subtotal::float8 as subtotal,discount::float8 as discount,tax::float8 as tax,freight::float8 as freight,total::float8 as total,notes,"rejectionReason" as rejection_reason,version,"createdBy" as created_by,"submittedBy" as submitted_by,"submittedAt" as submitted_at,"approvedBy" as approved_by,"approvedAt" as approved_at,"orderedBy" as ordered_by,"orderedAt" as ordered_at,"cancelledBy" as cancelled_by,"cancelledAt" as cancelled_at,"createdAt" as created_at,"updatedAt" as updated_at"#
    }
}

#[cfg(test)]
mod tests {
    use super::ProcurementService;
    use crate::models::PurchaseOrderLineInput;
    use uuid::Uuid;

    #[test]
    fn purchase_order_transition_graph_blocks_receive_and_cancel_after_partial_receipt() {
        let submit = ProcurementService::transition_spec("submit").expect("submit transition");
        assert_eq!(submit.0, &["draft", "rejected"]);
        assert_eq!(submit.1, "submitted");

        let cancel = ProcurementService::transition_spec("cancel").expect("cancel transition");
        assert!(!cancel.0.contains(&"partially_received"));
        assert!(!cancel.0.contains(&"received"));
        assert!(ProcurementService::transition_spec("receive").is_none());
    }

    #[test]
    fn purchase_order_totals_snapshot_boxes_tax_discount_and_freight() {
        let lines = vec![PurchaseOrderLineInput {
            product_id: Uuid::new_v4(),
            ordered_boxes: 2,
            box_cost: 100.0,
            tax_rate: Some(5.0),
        }];
        let (subtotal, tax, total) = ProcurementService::totals(&lines, 10.0, 20.0);
        assert_eq!(subtotal, 200.0);
        assert_eq!(tax, 10.0);
        assert_eq!(total, 220.0);
    }

    #[test]
    fn purchase_order_validation_rejects_duplicate_products_and_negative_adjustments() {
        let product_id = Uuid::new_v4();
        let lines = vec![
            PurchaseOrderLineInput {
                product_id,
                ordered_boxes: 1,
                box_cost: 100.0,
                tax_rate: None,
            },
            PurchaseOrderLineInput {
                product_id,
                ordered_boxes: 2,
                box_cost: 100.0,
                tax_rate: None,
            },
        ];
        assert!(ProcurementService::validate_lines(&lines).is_err());
        assert!(ProcurementService::validate_financials(-1.0, 0.0).is_err());
        assert!(ProcurementService::validate_financials(0.0, -1.0).is_err());
    }

    #[test]
    fn receipt_quantity_counts_rejected_boxes_against_outstanding() {
        assert!(ProcurementService::validate_receipt_quantity(3, 2, 5).is_ok());
        assert!(ProcurementService::validate_receipt_quantity(3, 3, 5).is_err());
        assert!(ProcurementService::validate_receipt_quantity(0, 1, 5).is_err());
    }
}
