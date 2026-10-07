//! Tenant-local inventory reads and atomic mutations. No operational PostgreSQL access.
use super::tenant_back_office::{event, now, write};
use crate::{dto::PaginationResult, error::AppError, models::*, tenancy::TenantDb};
use serde_json::json;
use sqlx::{QueryBuilder, Sqlite, SqliteConnection};
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone)]
pub struct TenantInventoryRepository {
    db: Arc<TenantDb>,
}
impl TenantInventoryRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    const LOCATION_SELECT: &'static str = r#"
                        SELECT unhex(replace(id, '-' , '' )) AS id, unhex(replace(venue_location_id, '-' , '' )) AS
                        venue_location_id, name, kind, is_active, unhex(replace(created_by, '-' , '' )) AS created_by,
                        unhex(replace(updated_by, '-' , '' )) AS updated_by, created_at, updated_at, deleted_at FROM
                        inventory_locations
                    "#;
    pub async fn find_location_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<InventoryLocation>, AppError> {
        let query = format!(
            "{} WHERE id = $1 AND deleted_at IS NULL",
            Self::LOCATION_SELECT
        );
        Ok(sqlx::query_as::<_, InventoryLocation>(&query)
            .bind(id.to_string())
            .fetch_optional(&self.db.read_pool()?)
            .await?)
    }
    pub async fn list_locations(
        &self,
        filters: &InventoryLocationFilterDto,
    ) -> Result<PaginationResult<InventoryLocation>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);
        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT unhex(replace(id,'-','')) AS id, unhex(replace(venue_location_id,'-','')) AS venue_location_id, name, kind as kind, is_active as is_active, \
             unhex(replace(created_by,'-','')) AS created_by, unhex(replace(updated_by,'-','')) AS updated_by, \
             created_at as created_at, updated_at as updated_at, \
             deleted_at as deleted_at \
             FROM inventory_locations WHERE deleted_at IS NULL",
        );
        if let Some(kind) = &filters.kind {
            builder.push(" AND kind = ");
            builder.push_bind(kind);
        }
        if let Some(venue_location_id) = filters.venue_location_id {
            builder
                .push(" AND venue_location_id = ")
                .push_bind(venue_location_id.to_string());
        }
        if let Some(is_active) = filters.is_active {
            builder.push(" AND is_active = ");
            builder.push_bind(is_active);
        }
        builder.push(" ORDER BY name ASC, id ASC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);
        let rows = builder
            .build_query_as::<InventoryLocation>()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(*) FROM inventory_locations WHERE deleted_at IS NULL");
        if let Some(kind) = &filters.kind {
            count_builder.push(" AND kind = ");
            count_builder.push_bind(kind);
        }
        if let Some(venue_location_id) = filters.venue_location_id {
            count_builder
                .push(" AND venue_location_id = ")
                .push_bind(venue_location_id.to_string());
        }
        if let Some(is_active) = filters.is_active {
            count_builder.push(" AND is_active = ");
            count_builder.push_bind(is_active);
        }
        let total: (i64,) = count_builder
            .build_query_as()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }
    pub async fn list_stock(
        &self,
        filters: &LocationStockFilterDto,
    ) -> Result<PaginationResult<LocationStockRow>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);
        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT unhex(replace(ls.inventory_location_id,'-','')) AS location_id, l.name as location_name, unhex(replace(ls.product_id,'-','')) AS product_id, \
             ls.quantity_pieces as quantity_pieces, p.name as product_name, p.sku as product_sku, \
             ls.created_at as created_at, ls.updated_at as updated_at \
             FROM location_stock ls \
             INNER JOIN inventory_locations l ON l.id = ls.inventory_location_id AND l.deleted_at IS NULL \
             INNER JOIN products p ON p.id = ls.product_id AND p.deleted_at IS NULL \
             LEFT JOIN inventory_reorder_rules rr ON rr.inventory_location_id=ls.inventory_location_id AND rr.product_id=ls.product_id AND rr.is_active=true \
             WHERE 1=1",
        );
        if let Some(location_id) = filters.location_id {
            builder.push(" AND ls.inventory_location_id = ");
            builder.push_bind(location_id.to_string());
        }
        if let Some(venue_location_id) = filters.venue_location_id {
            builder
                .push(" AND l.venue_location_id = ")
                .push_bind(venue_location_id.to_string());
        }
        if let Some(product_id) = filters.product_id {
            builder.push(" AND ls.product_id = ");
            builder.push_bind(product_id.to_string());
        }
        if let Some(search) = filters
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            builder.push(" AND (p.name LIKE ");
            builder.push_bind(format!("%{search}%"));
            builder.push(" OR p.sku LIKE ");
            builder.push_bind(format!("%{search}%"));
            builder.push(")");
        }
        if filters.low_stock.unwrap_or(false) {
            builder.push(" AND rr.id IS NOT NULL AND ls.quantity_pieces <= rr.minimum_pieces");
        }
        let sort_column = match filters.sort_by.as_deref() {
            Some("quantity") => "ls.quantity_pieces",
            Some("location") => "l.name",
            _ => "p.name",
        };
        let sort_order = if filters.sort_order.as_deref() == Some("desc") {
            "DESC"
        } else {
            "ASC"
        };
        builder.push(format!(
            " ORDER BY {sort_column} {sort_order}, ls.inventory_location_id, ls.product_id LIMIT "
        ));
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);
        let rows = builder
            .build_query_as::<LocationStockRow>()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT COUNT(*) FROM location_stock ls \
             INNER JOIN inventory_locations l ON l.id=ls.inventory_location_id AND l.deleted_at IS NULL \
             INNER JOIN products p ON p.id=ls.product_id AND p.deleted_at IS NULL \
             LEFT JOIN inventory_reorder_rules rr ON rr.inventory_location_id=ls.inventory_location_id \
               AND rr.product_id=ls.product_id AND rr.is_active=true WHERE 1=1",
        );
        if let Some(location_id) = filters.location_id {
            count_builder.push(" AND ls.inventory_location_id = ");
            count_builder.push_bind(location_id.to_string());
        }
        if let Some(venue_location_id) = filters.venue_location_id {
            count_builder
                .push(" AND l.venue_location_id = ")
                .push_bind(venue_location_id.to_string());
        }
        if let Some(product_id) = filters.product_id {
            count_builder.push(" AND ls.product_id = ");
            count_builder.push_bind(product_id.to_string());
        }
        if let Some(search) = filters
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            count_builder.push(" AND (p.name LIKE ");
            count_builder.push_bind(format!("%{search}%"));
            count_builder.push(" OR p.sku LIKE ");
            count_builder.push_bind(format!("%{search}%"));
            count_builder.push(")");
        }
        if filters.low_stock.unwrap_or(false) {
            count_builder
                .push(" AND rr.id IS NOT NULL AND ls.quantity_pieces <= rr.minimum_pieces");
        }
        let total: (i64,) = count_builder
            .build_query_as()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }
    pub async fn stock_quantity_at(
        &self,
        location_id: Uuid,
        product_id: Uuid,
    ) -> Result<i32, AppError> {
        let row: Option<(i32,)> = sqlx::query_as(
            r#"SELECT quantity_pieces FROM location_stock
               WHERE inventory_location_id = $1 AND product_id = $2"#,
        )
        .bind(location_id.to_string())
        .bind(product_id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?;
        Ok(row.map(|(q,)| q).unwrap_or(0))
    }
    pub async fn list_adjustments(
        &self,
        filters: &StockAdjustmentFilterDto,
    ) -> Result<PaginationResult<StockAdjustment>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);
        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT unhex(replace(id,'-','')) AS id, unhex(replace(inventory_location_id,'-','')) AS location_id, notes, unhex(replace(created_by,'-','')) AS created_by, \
             created_at as created_at FROM stock_adjustments WHERE 1=1",
        );
        if let Some(location_id) = filters.location_id {
            builder.push(" AND inventory_location_id = ");
            builder.push_bind(location_id.to_string());
        }
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);
        let rows = builder
            .build_query_as::<StockAdjustment>()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(*) FROM stock_adjustments WHERE 1=1");
        if let Some(location_id) = filters.location_id {
            count_builder.push(" AND inventory_location_id = ");
            count_builder.push_bind(location_id.to_string());
        }
        let total: (i64,) = count_builder
            .build_query_as()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }
    pub async fn find_adjustment_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StockAdjustment>, AppError> {
        Ok(sqlx::query_as::<_, StockAdjustment>(
            r#"
            SELECT unhex(replace(id,'-','')) AS id, unhex(replace(inventory_location_id,'-','')) AS location_id, notes, unhex(replace(created_by,'-','')) AS created_by,
                   created_at as created_at
            FROM stock_adjustments WHERE id = $1
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }
    pub async fn adjustment_lines(
        &self,
        adjustment_id: Uuid,
    ) -> Result<Vec<StockAdjustmentLine>, AppError> {
        Ok(sqlx::query_as::<_, StockAdjustmentLine>(
            r#"
            SELECT unhex(replace(id,'-','')) AS id, unhex(replace(adjustment_id,'-','')) AS adjustment_id, unhex(replace(product_id,'-','')) AS product_id,
                   previous_pieces as previous_pieces, counted_pieces as counted_pieces,
                   delta_pieces as delta_pieces
            FROM stock_adjustment_lines WHERE adjustment_id = $1
            "#,
        )
        .bind(adjustment_id.to_string())
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }
    pub async fn list_receipts(
        &self,
        filters: &StockReceiptFilterDto,
    ) -> Result<PaginationResult<StockReceipt>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);
        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT unhex(replace(id,'-','')) AS id, unhex(replace(inventory_location_id,'-','')) AS location_id, unhex(replace(vendor_id,'-','')) AS vendor_id, notes, \
             unhex(replace(created_by,'-','')) AS created_by, created_at as created_at \
             FROM stock_receipts WHERE 1=1",
        );
        if let Some(location_id) = filters.location_id {
            builder.push(" AND inventory_location_id = ");
            builder.push_bind(location_id.to_string());
        }
        if let Some(from) = filters.from {
            builder.push(" AND created_at >= ");
            builder.push_bind(
                crate::tenancy::format_sqlite_timestamp(&from)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        if let Some(to) = filters.to {
            builder.push(" AND created_at <= ");
            builder.push_bind(
                crate::tenancy::format_sqlite_timestamp(&to)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);
        let rows = builder
            .build_query_as::<StockReceipt>()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(*) FROM stock_receipts WHERE 1=1");
        if let Some(location_id) = filters.location_id {
            count_builder.push(" AND inventory_location_id = ");
            count_builder.push_bind(location_id.to_string());
        }
        if let Some(from) = filters.from {
            count_builder.push(" AND created_at >= ");
            count_builder.push_bind(
                crate::tenancy::format_sqlite_timestamp(&from)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        if let Some(to) = filters.to {
            count_builder.push(" AND created_at <= ");
            count_builder.push_bind(
                crate::tenancy::format_sqlite_timestamp(&to)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        let total: (i64,) = count_builder
            .build_query_as()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }
    pub async fn receipt_lines(&self, receipt_id: Uuid) -> Result<Vec<StockReceiptLine>, AppError> {
        Ok(sqlx::query_as::<_, StockReceiptLine>(
            r#"
            SELECT unhex(replace(id,'-','')) AS id, unhex(replace(receipt_id,'-','')) AS receipt_id, unhex(replace(product_id,'-','')) AS product_id,
                   COALESCE(accepted_box_quantity,box_quantity) as box_quantity, pieces_added as pieces_added
            FROM stock_receipt_lines WHERE receipt_id = $1
            "#,
        )
        .bind(receipt_id.to_string())
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }
    pub async fn find_transfer_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StockTransferRequest>, AppError> {
        Ok(sqlx::query_as::<_, StockTransferRequest>(
            r#"
            SELECT unhex(replace(id,'-','')) AS id, unhex(replace(from_location_id,'-','')) AS from_location_id, unhex(replace(to_location_id,'-','')) AS to_location_id,
                   status as status, unhex(replace(requested_by,'-','')) AS requested_by,
                   unhex(replace(approved_by,'-','')) AS approved_by, approved_at as approved_at,
                   rejection_reason as rejection_reason, unhex(replace(fulfilled_by,'-','')) AS fulfilled_by,
                   fulfilled_at as fulfilled_at, created_at as created_at,
                   updated_at as updated_at
            FROM stock_transfer_requests WHERE id = $1
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }
    pub async fn transfer_lines(
        &self,
        request_id: Uuid,
    ) -> Result<Vec<StockTransferLine>, AppError> {
        Ok(sqlx::query_as::<_, StockTransferLine>(
            r#"
            SELECT unhex(replace(id,'-','')) AS id, unhex(replace(transfer_request_id,'-','')) AS transfer_request_id, unhex(replace(product_id,'-','')) AS product_id,
                   quantity_pieces as quantity_pieces
            FROM stock_transfer_lines WHERE transfer_request_id = $1
            "#,
        )
        .bind(request_id.to_string())
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }
    pub async fn list_transfer_requests(
        &self,
        filters: &StockTransferFilterDto,
    ) -> Result<PaginationResult<StockTransferRequest>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);
        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT unhex(replace(id,'-','')) AS id, unhex(replace(from_location_id,'-','')) AS from_location_id, unhex(replace(to_location_id,'-','')) AS to_location_id, \
             status as status, unhex(replace(requested_by,'-','')) AS requested_by, \
             unhex(replace(approved_by,'-','')) AS approved_by, approved_at as approved_at, \
             rejection_reason as rejection_reason, unhex(replace(fulfilled_by,'-','')) AS fulfilled_by, \
             fulfilled_at as fulfilled_at, created_at as created_at, \
             updated_at as updated_at FROM stock_transfer_requests WHERE 1=1",
        );
        if let Some(status) = &filters.status {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
        if let Some(from) = filters.from_location_id {
            builder.push(" AND from_location_id = ");
            builder.push_bind(from.to_string());
        }
        if let Some(to) = filters.to_location_id {
            builder.push(" AND to_location_id = ");
            builder.push_bind(to.to_string());
        }
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);
        let rows = builder
            .build_query_as::<StockTransferRequest>()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(*) FROM stock_transfer_requests WHERE 1=1");
        if let Some(status) = &filters.status {
            count_builder.push(" AND status = ");
            count_builder.push_bind(status);
        }
        if let Some(from) = filters.from_location_id {
            count_builder.push(" AND from_location_id = ");
            count_builder.push_bind(from.to_string());
        }
        if let Some(to) = filters.to_location_id {
            count_builder.push(" AND to_location_id = ");
            count_builder.push_bind(to.to_string());
        }
        let total: (i64,) = count_builder
            .build_query_as()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }
    pub async fn find_waste_by_id(&self, id: Uuid) -> Result<Option<StockWasteEvent>, AppError> {
        Ok(sqlx::query_as::<_, StockWasteEvent>(
            r#"
            SELECT unhex(replace(id,'-','')) AS id, unhex(replace(inventory_location_id,'-','')) AS location_id, status as status, notes,
                   unhex(replace(approved_by,'-','')) AS approved_by, approved_at as approved_at,
                   rejection_reason as rejection_reason, unhex(replace(created_by,'-','')) AS created_by,
                   created_at as created_at, updated_at as updated_at
            FROM stock_waste_events WHERE id = $1
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }
    pub async fn waste_lines(&self, event_id: Uuid) -> Result<Vec<StockWasteLine>, AppError> {
        Ok(sqlx::query_as::<_, StockWasteLine>(
            r#"
            SELECT unhex(replace(id,'-','')) AS id, unhex(replace(waste_event_id,'-','')) AS waste_event_id, unhex(replace(product_id,'-','')) AS product_id,
                   quantity_pieces as quantity_pieces, reason_code as reason_code, note
            FROM stock_waste_lines WHERE waste_event_id = $1
            "#,
        )
        .bind(event_id.to_string())
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }
    pub async fn list_waste_events(
        &self,
        filters: &StockWasteFilterDto,
    ) -> Result<PaginationResult<StockWasteEvent>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1).saturating_mul(limit);
        let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "SELECT unhex(replace(id,'-','')) AS id, unhex(replace(inventory_location_id,'-','')) AS location_id, status as status, notes, \
             unhex(replace(approved_by,'-','')) AS approved_by, approved_at as approved_at, \
             rejection_reason as rejection_reason, unhex(replace(created_by,'-','')) AS created_by, \
             created_at as created_at, updated_at as updated_at \
             FROM stock_waste_events WHERE 1=1",
        );
        if let Some(status) = &filters.status {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
        if let Some(location_id) = filters.location_id {
            builder.push(" AND inventory_location_id = ");
            builder.push_bind(location_id.to_string());
        }
        if let Some(from) = filters.from {
            builder.push(" AND created_at >= ");
            builder.push_bind(
                crate::tenancy::format_sqlite_timestamp(&from)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        if let Some(to) = filters.to {
            builder.push(" AND created_at <= ");
            builder.push_bind(
                crate::tenancy::format_sqlite_timestamp(&to)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);
        let rows = builder
            .build_query_as::<StockWasteEvent>()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new("SELECT COUNT(*) FROM stock_waste_events WHERE 1=1");
        if let Some(status) = &filters.status {
            count_builder.push(" AND status = ");
            count_builder.push_bind(status);
        }
        if let Some(location_id) = filters.location_id {
            count_builder.push(" AND inventory_location_id = ");
            count_builder.push_bind(location_id.to_string());
        }
        if let Some(from) = filters.from {
            count_builder.push(" AND created_at >= ");
            count_builder.push_bind(
                crate::tenancy::format_sqlite_timestamp(&from)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        if let Some(to) = filters.to {
            count_builder.push(" AND created_at <= ");
            count_builder.push_bind(
                crate::tenancy::format_sqlite_timestamp(&to)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
        let total: (i64,) = count_builder
            .build_query_as()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }
    pub async fn create_location(
        &self,
        dto: &CreateInventoryLocationDto,
        actor: Option<Uuid>,
    ) -> Result<InventoryLocation, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let venue = dto.venue_location_id.ok_or_else(|| AppError::bad_request_code("LOCATION_REQUIRED", None))?;
                    let kind = dto.kind.trim().to_lowercase();
                    if !matches!(kind.as_str(), "warehouse" | "store") {
                        return Err(AppError::BadRequest("Invalid location kind".into()));
                    }
                    let id = Uuid::now_v7();
                    let ts = now()?;
                    sqlx::query("INSERT INTO inventory_locations(id,venue_location_id,name,kind,is_active,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)").bind(id.to_string()).bind(venue.to_string()).bind(dto.name.trim()).bind(kind).bind(dto.is_active.unwrap_or(true)).bind(actor.map(|x| x.to_string())).bind(actor.map(|x| x.to_string())).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    let row = Self::location_row(c, id).await?;
                    event(c, "inventory_location", id, "inventory.location_created", Some(venue), false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn update_location(
        &self,
        id: Uuid,
        dto: &UpdateInventoryLocationDto,
        actor: Option<Uuid>,
    ) -> Result<InventoryLocation, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    Self::location_row(c, id).await?;
                    let kind = dto.kind.map(|x| x.trim().to_lowercase());
                    if kind.as_deref().is_some_and(|x| !matches!(x, "warehouse" | "store")) {
                        return Err(AppError::BadRequest("Invalid location kind".into()));
                    }
                    sqlx::query("UPDATE inventory_locations SET name=COALESCE(?,name),kind=COALESCE(?,kind),is_active=COALESCE(?,is_active),venue_location_id=COALESCE(?,venue_location_id),updated_by=?,updated_at=? WHERE id=?").bind(dto.name).bind(kind).bind(dto.is_active).bind(dto.venue_location_id.map(|x| x.to_string())).bind(actor.map(|x| x.to_string())).bind(now()?).bind(id.to_string()).execute(&mut *c).await?;
                    let row = Self::location_row(c, id).await?;
                    event(c, "inventory_location", id, "inventory.location_updated", Some(row.venue_location_id), false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn soft_delete_location(&self, id: Uuid) -> Result<InventoryLocation, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let row = Self::location_row(c, id).await?;
                    let ts = now()?;
                    sqlx::query("UPDATE inventory_locations SET deleted_at=?,updated_at=?,is_active=0 WHERE id=?").bind(&ts).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
                    event(c, "inventory_location", id, "inventory.location_deleted", Some(row.venue_location_id), true, json!({"id":id})).await?;
                    Ok(sqlx::query_as(&format!("{} WHERE id=?", Self::LOCATION_SELECT)).bind(id.to_string()).fetch_one(&mut *c).await?)
                })
            }),
        )
        .await
    }
    async fn location_row(
        c: &mut SqliteConnection,
        id: Uuid,
    ) -> Result<InventoryLocation, AppError> {
        sqlx::query_as(&format!(
            "{} WHERE id=? AND deleted_at IS NULL",
            Self::LOCATION_SELECT
        ))
        .bind(id.to_string())
        .fetch_optional(c)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Location {id} not found")))
    }
    pub(crate) async fn location(
        c: &mut SqliteConnection,
        id: Uuid,
    ) -> Result<InventoryLocation, AppError> {
        let row = Self::location_row(c, id).await?;
        if !row.is_active {
            return Err(AppError::Conflict("Inventory location is inactive".into()));
        }
        Ok(row)
    }
    pub async fn get_config_location_id(&self, key: &str) -> Result<Option<Uuid>, AppError> {
        let value: Option<String> = sqlx::query_scalar("SELECT json_extract(value,'$') FROM setting_overrides WHERE key=? AND location_id IS NULL").bind(key).fetch_optional(&self.db.read_pool()?).await?;
        Ok(value.and_then(|x| Uuid::parse_str(&x).ok()))
    }
    pub(crate) async fn adjust_stock(
        c: &mut SqliteConnection,
        location: Uuid,
        product: Uuid,
        delta: i32,
        kind: &str,
        reference: Uuid,
        reference_type: &str,
        actor: Option<Uuid>,
    ) -> Result<(), AppError> {
        if delta == 0 {
            return Ok(());
        }
        let previous: i32 = sqlx::query_scalar("SELECT quantity_pieces FROM location_stock WHERE inventory_location_id=? AND product_id=?").bind(location.to_string()).bind(product.to_string()).fetch_optional(&mut *c).await?.unwrap_or(0);
        let quantity = previous
            .checked_add(delta)
            .filter(|x| *x >= 0)
            .ok_or_else(|| {
                AppError::Conflict("Insufficient stock or stock quantity overflow".into())
            })?;
        let ts = now()?;
        sqlx::query("INSERT INTO location_stock(inventory_location_id,product_id,quantity_pieces,created_at,updated_at) VALUES(?,?,?,?,?) ON CONFLICT(inventory_location_id,product_id) DO UPDATE SET quantity_pieces=excluded.quantity_pieces,updated_at=excluded.updated_at").bind(location.to_string()).bind(product.to_string()).bind(quantity).bind(&ts).bind(&ts).execute(&mut *c).await?;
        let movement = Uuid::now_v7();
        sqlx::query("INSERT INTO stock_movements(id,inventory_location_id,product_id,delta,movement_type,reference_id,reference_type,created_by,created_at) VALUES(?,?,?,?,?,?,?,?,?)").bind(movement.to_string()).bind(location.to_string()).bind(product.to_string()).bind(delta).bind(kind).bind(reference.to_string()).bind(reference_type).bind(actor.map(|x| x.to_string())).bind(&ts).execute(&mut *c).await?;
        let venue: Option<String> =
            sqlx::query_scalar("SELECT venue_location_id FROM inventory_locations WHERE id=?")
                .bind(location.to_string())
                .fetch_optional(&mut *c)
                .await?;
        event(c, "stock_movement", movement, "inventory.stock_changed", venue.and_then(|x| Uuid::parse_str(&x).ok()), false, json!({"id":movement,"locationId":location,"productId":product,"delta":delta,"quantityPieces":quantity,"movementType":kind,"referenceId":reference,"referenceType":reference_type})).await
    }
    pub async fn create_receipt(
        &self,
        dto: &CreateStockReceiptDto,
        actor: Option<Uuid>,
    ) -> Result<(StockReceipt, Vec<StockReceiptLine>), AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    if dto.exceptional_reason.trim().len() < 5 {
                        return Err(AppError::bad_request_code("DIRECT_RECEIPT_REASON_REQUIRED", None));
                    }
                    Self::valid_lines(dto.lines.iter().map(|x| (x.product_id, x.box_quantity)))?;
                    let location = Self::location(c, dto.location_id).await?;
                    let id = Uuid::now_v7();
                    let ts = now()?;
                    sqlx::query("INSERT INTO stock_receipts(id,inventory_location_id,vendor_id,notes,exceptional_reason,created_by,receipt_date,created_at) VALUES(?,?,?,?,?,?,?,?)").bind(id.to_string()).bind(dto.location_id.to_string()).bind(dto.vendor_id.map(|x| x.to_string())).bind(dto.notes).bind(dto.exceptional_reason.trim()).bind(actor.map(|x| x.to_string())).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    let mut lines = Vec::new();
                    for input in dto.lines {
                        let units: i32 = sqlx::query_scalar("SELECT units_per_purchase_unit FROM products WHERE id=? AND deleted_at IS NULL").bind(input.product_id.to_string()).fetch_optional(&mut *c).await?.ok_or_else(|| AppError::BadRequest("Invalid receipt product".into()))?;
                        let pieces = input.box_quantity.checked_mul(units).filter(|x| *x > 0).ok_or_else(|| AppError::BadRequest("Receipt quantity overflow".into()))?;
                        let line = StockReceiptLine { id: Uuid::now_v7(), receipt_id: id, product_id: input.product_id, box_quantity: input.box_quantity, pieces_added: pieces };
                        sqlx::query("INSERT INTO stock_receipt_lines(id,receipt_id,product_id,box_quantity,pieces_added) VALUES(?,?,?,?,?)").bind(line.id.to_string()).bind(id.to_string()).bind(line.product_id.to_string()).bind(line.box_quantity).bind(pieces).execute(&mut *c).await?;
                        Self::adjust_stock(c, dto.location_id, line.product_id, pieces, "receipt", id, "stock_receipt", actor).await?;
                        lines.push(line);
                    }
                    let receipt = sqlx::query_as::<_, StockReceipt>("SELECT unhex(replace(id,'-','')) AS id,unhex(replace(inventory_location_id,'-','')) AS location_id,unhex(replace(vendor_id,'-','')) AS vendor_id,notes,unhex(replace(created_by,'-','')) AS created_by,created_at FROM stock_receipts WHERE id=?").bind(id.to_string()).fetch_one(&mut *c).await?;
                    event(c, "stock_receipt", id, "inventory.receipt_created", Some(location.venue_location_id), false, json!({"receipt":receipt,"lines":lines})).await?;
                    Ok((receipt, lines))
                })
            }),
        )
        .await
    }
    fn valid_lines(lines: impl IntoIterator<Item = (Uuid, i32)>) -> Result<(), AppError> {
        let mut seen = HashSet::new();
        for (id, quantity) in lines {
            if quantity <= 0 || !seen.insert(id) {
                return Err(AppError::BadRequest(
                    "Quantities must be positive and products unique".into(),
                ));
            }
        }
        if seen.is_empty() {
            return Err(AppError::BadRequest("At least one line is required".into()));
        }
        Ok(())
    }
    pub async fn create_adjustment(
        &self,
        dto: &CreateStockAdjustmentDto,
        actor: Option<Uuid>,
    ) -> Result<(StockAdjustment, Vec<StockAdjustmentLine>), AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    if dto.notes.trim().len() < 3 || dto.lines.is_empty() {
                        return Err(AppError::BadRequest("Adjustment notes and lines are required".into()));
                    }
                    let location = Self::location(c, dto.location_id).await?;
                    let mut seen = HashSet::new();
                    let id = Uuid::now_v7();
                    let ts = now()?;
                    sqlx::query("INSERT INTO stock_adjustments(id,inventory_location_id,notes,created_by,created_at) VALUES(?,?,?,?,?)").bind(id.to_string()).bind(dto.location_id.to_string()).bind(dto.notes.trim()).bind(actor.map(|x| x.to_string())).bind(&ts).execute(&mut *c).await?;
                    let mut lines = Vec::new();
                    for input in dto.lines {
                        if input.counted_pieces < 0 || !seen.insert(input.product_id) {
                            return Err(AppError::BadRequest("Invalid adjustment quantity or duplicate product".into()));
                        }
                        let previous: i32 = sqlx::query_scalar("SELECT quantity_pieces FROM location_stock WHERE inventory_location_id=? AND product_id=?").bind(dto.location_id.to_string()).bind(input.product_id.to_string()).fetch_optional(&mut *c).await?.unwrap_or(0);
                        let line = StockAdjustmentLine { id: Uuid::now_v7(), adjustment_id: id, product_id: input.product_id, previous_pieces: previous, counted_pieces: input.counted_pieces, delta_pieces: input.counted_pieces - previous };
                        sqlx::query("INSERT INTO stock_adjustment_lines(id,adjustment_id,product_id,previous_pieces,counted_pieces,delta_pieces) VALUES(?,?,?,?,?,?)").bind(line.id.to_string()).bind(id.to_string()).bind(line.product_id.to_string()).bind(previous).bind(line.counted_pieces).bind(line.delta_pieces).execute(&mut *c).await?;
                        Self::adjust_stock(c, dto.location_id, line.product_id, line.delta_pieces, "adjustment", id, "stock_adjustment", actor).await?;
                        lines.push(line);
                    }
                    if lines.iter().all(|x| x.delta_pieces == 0) {
                        return Err(AppError::BadRequest("Adjustment does not change stock".into()));
                    }
                    let row = StockAdjustment { id, location_id: dto.location_id, notes: dto.notes.trim().into(), created_by: actor, created_at: ts.parse().map_err(|e| AppError::Internal(format!("Invalid timestamp: {e}")))? };
                    event(c, "stock_adjustment", id, "inventory.adjustment_created", Some(location.venue_location_id), false, json!({"adjustment":row,"lines":lines})).await?;
                    Ok((row, lines))
                })
            }),
        )
        .await
    }
    const TRANSFER_SELECT: &'static str = "SELECT unhex(replace(id,'-','')) AS id,unhex(replace(from_location_id,'-','')) AS from_location_id,unhex(replace(to_location_id,'-','')) AS to_location_id,status,unhex(replace(requested_by,'-','')) AS requested_by,unhex(replace(approved_by,'-','')) AS approved_by,approved_at,rejection_reason,unhex(replace(fulfilled_by,'-','')) AS fulfilled_by,fulfilled_at,created_at,updated_at FROM stock_transfer_requests";
    async fn transfer(
        c: &mut SqliteConnection,
        id: Uuid,
    ) -> Result<StockTransferRequest, AppError> {
        Ok(
            sqlx::query_as(&format!("{} WHERE id=?", Self::TRANSFER_SELECT))
                .bind(id.to_string())
                .fetch_optional(c)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Transfer request {id} not found")))?,
        )
    }
    pub async fn create_transfer_request(
        &self,
        from: Uuid,
        to: Uuid,
        lines: &[(Uuid, i32)],
        actor: Option<Uuid>,
    ) -> Result<(StockTransferRequest, Vec<StockTransferLine>), AppError> {
        let lines = lines.to_vec();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    Self::valid_lines(lines.iter().copied())?;
                    let src = Self::location(c, from).await?;
                    let dest = Self::location(c, to).await?;
                    if src.kind != "warehouse" || dest.kind != "store" || from == to {
                        return Err(AppError::BadRequest("Transfer must be from a warehouse to a store".into()));
                    }
                    let id = Uuid::now_v7();
                    let ts = now()?;
                    sqlx::query("INSERT INTO stock_transfer_requests(id,from_location_id,to_location_id,status,requested_by,created_at,updated_at) VALUES(?,?,?,'pending',?,?,?)").bind(id.to_string()).bind(from.to_string()).bind(to.to_string()).bind(actor.map(|x| x.to_string())).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    let mut saved = Vec::new();
                    for (product, quantity) in lines {
                        let line = StockTransferLine { id: Uuid::now_v7(), transfer_request_id: id, product_id: product, quantity_pieces: quantity };
                        sqlx::query("INSERT INTO stock_transfer_lines(id,transfer_request_id,product_id,quantity_pieces) VALUES(?,?,?,?)").bind(line.id.to_string()).bind(id.to_string()).bind(product.to_string()).bind(quantity).execute(&mut *c).await?;
                        saved.push(line);
                    }
                    let row = Self::transfer(c, id).await?;
                    event(c, "stock_transfer", id, "approval.requested", Some(dest.venue_location_id), false, json!({"transfer_request_id":id,"entity_type":"stock_transfer_request","requestedBy":actor,"request":row,"lines":saved})).await?;
                    Ok((row, saved))
                })
            }),
        )
        .await
    }
    pub async fn approve_transfer(
        &self,
        id: Uuid,
        actor: Uuid,
    ) -> Result<StockTransferRequest, AppError> {
        self.decide_transfer(id, actor, None).await
    }
    pub async fn reject_transfer(
        &self,
        id: Uuid,
        reason: &str,
        actor: Uuid,
    ) -> Result<StockTransferRequest, AppError> {
        if reason.trim().is_empty() {
            return Err(AppError::BadRequest("Rejection reason is required".into()));
        }
        self.decide_transfer(id, actor, Some(reason.trim().into()))
            .await
    }
    async fn decide_transfer(
        &self,
        id: Uuid,
        actor: Uuid,
        reason: Option<String>,
    ) -> Result<StockTransferRequest, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::transfer(c, id).await?;
                    if old.status != "pending" {
                        return Err(AppError::Conflict("Transfer is not pending".into()));
                    }
                    let dest = Self::location(c, old.to_location_id).await?;
                    let status = if reason.is_some() { "rejected" } else { "approved" };
                    let ts = now()?;
                    sqlx::query("UPDATE stock_transfer_requests SET status=?,approved_by=?,approved_at=?,rejection_reason=?,updated_at=? WHERE id=?").bind(status).bind(actor.to_string()).bind(&ts).bind(reason).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
                    let row = Self::transfer(c, id).await?;
                    event(c, "stock_transfer", id, "approval.decided", Some(dest.venue_location_id), false, json!({"transfer_request_id":id,"entity_type":"stock_transfer_request","status":row.status,"requestedBy":row.requested_by,"request":row})).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn fulfill_transfer(
        &self,
        id: Uuid,
        actor: Uuid,
    ) -> Result<StockTransferRequest, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::transfer(c, id).await?;
                    if old.status != "approved" {
                        return Err(AppError::Conflict("Transfer must be approved before fulfillment".into()));
                    }
                    Self::location(c, old.from_location_id).await?;
                    let dest = Self::location(c, old.to_location_id).await?;
                    let lines: Vec<(String, i32)> = sqlx::query_as("SELECT product_id,quantity_pieces FROM stock_transfer_lines WHERE transfer_request_id=? ORDER BY product_id").bind(id.to_string()).fetch_all(&mut *c).await?;
                    for (product, quantity) in lines {
                        let product = Uuid::parse_str(&product).map_err(|e| AppError::Internal(e.to_string()))?;
                        Self::adjust_stock(c, old.from_location_id, product, -quantity, "transfer_out", id, "stock_transfer_request", Some(actor)).await?;
                        Self::adjust_stock(c, old.to_location_id, product, quantity, "transfer_in", id, "stock_transfer_request", Some(actor)).await?;
                    }
                    let ts = now()?;
                    sqlx::query("UPDATE stock_transfer_requests SET status='fulfilled',fulfilled_by=?,fulfilled_at=?,updated_at=? WHERE id=?").bind(actor.to_string()).bind(&ts).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
                    let row = Self::transfer(c, id).await?;
                    event(c, "stock_transfer", id, "inventory.transfer_fulfilled", Some(dest.venue_location_id), false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    const WASTE_SELECT: &'static str = "SELECT unhex(replace(id,'-','')) AS id,unhex(replace(inventory_location_id,'-','')) AS location_id,status,notes,unhex(replace(approved_by,'-','')) AS approved_by,approved_at,rejection_reason,unhex(replace(created_by,'-','')) AS created_by,created_at,updated_at FROM stock_waste_events";
    async fn waste(c: &mut SqliteConnection, id: Uuid) -> Result<StockWasteEvent, AppError> {
        Ok(
            sqlx::query_as(&format!("{} WHERE id=?", Self::WASTE_SELECT))
                .bind(id.to_string())
                .fetch_optional(c)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Waste event {id} not found")))?,
        )
    }
    pub async fn create_waste_event(
        &self,
        dto: &CreateStockWasteEventDto,
        actor: Option<Uuid>,
    ) -> Result<(StockWasteEvent, Vec<StockWasteLine>), AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    if dto.lines.is_empty() {
                        return Err(AppError::BadRequest("At least one waste line is required".into()));
                    }
                    let location = Self::location(c, dto.location_id).await?;
                    let id = Uuid::now_v7();
                    let ts = now()?;
                    sqlx::query("INSERT INTO stock_waste_events(id,inventory_location_id,notes,created_by,created_at,updated_at) VALUES(?,?,?,?,?,?)").bind(id.to_string()).bind(dto.location_id.to_string()).bind(dto.notes).bind(actor.map(|x| x.to_string())).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    let mut lines = Vec::new();
                    for input in dto.lines {
                        let reason = input.reason_code.trim().to_lowercase();
                        if input.quantity_pieces <= 0 || !matches!(reason.as_str(), "expired" | "damaged" | "spoilage" | "sample" | "other") || (reason == "other" && input.note.as_deref().unwrap_or("").trim().len() < 10) {
                            return Err(AppError::BadRequest("Invalid waste quantity, reason or note".into()));
                        }
                        let line = StockWasteLine { id: Uuid::now_v7(), waste_event_id: id, product_id: input.product_id, quantity_pieces: input.quantity_pieces, reason_code: reason, note: input.note };
                        sqlx::query("INSERT INTO stock_waste_lines(id,waste_event_id,product_id,quantity_pieces,reason_code,note) VALUES(?,?,?,?,?,?)").bind(line.id.to_string()).bind(id.to_string()).bind(line.product_id.to_string()).bind(line.quantity_pieces).bind(&line.reason_code).bind(&line.note).execute(&mut *c).await?;
                        lines.push(line);
                    }
                    let row = Self::waste(c, id).await?;
                    event(c, "stock_waste", id, "approval.requested", Some(location.venue_location_id), false, json!({"waste_event_id":id,"entity_type":"stock_waste_event","requestedBy":actor,"event":row,"lines":lines})).await?;
                    Ok((row, lines))
                })
            }),
        )
        .await
    }
    pub async fn approve_waste(&self, id: Uuid, actor: Uuid) -> Result<StockWasteEvent, AppError> {
        self.decide_waste(id, actor, None).await
    }
    pub async fn reject_waste(
        &self,
        id: Uuid,
        reason: &str,
        actor: Uuid,
    ) -> Result<StockWasteEvent, AppError> {
        if reason.trim().is_empty() {
            return Err(AppError::BadRequest("Rejection reason is required".into()));
        }
        self.decide_waste(id, actor, Some(reason.trim().into()))
            .await
    }
    async fn decide_waste(
        &self,
        id: Uuid,
        actor: Uuid,
        reason: Option<String>,
    ) -> Result<StockWasteEvent, AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let old = Self::waste(c, id).await?;
                    if old.status != "pending" {
                        return Err(AppError::Conflict("Waste event is not pending".into()));
                    }
                    let location = Self::location(c, old.location_id).await?;
                    if reason.is_none() {
                        let lines: Vec<(String, i32)> = sqlx::query_as("SELECT product_id,quantity_pieces FROM stock_waste_lines WHERE waste_event_id=? ORDER BY product_id,id").bind(id.to_string()).fetch_all(&mut *c).await?;
                        for (product, quantity) in lines {
                            Self::adjust_stock(c, old.location_id, Uuid::parse_str(&product).map_err(|e| AppError::Internal(e.to_string()))?, -quantity, "waste", id, "stock_waste_event", Some(actor)).await?;
                        }
                    }
                    let status = if reason.is_some() { "rejected" } else { "approved" };
                    let ts = now()?;
                    sqlx::query("UPDATE stock_waste_events SET status=?,approved_by=?,approved_at=?,rejection_reason=?,updated_at=? WHERE id=?").bind(status).bind(actor.to_string()).bind(&ts).bind(reason).bind(&ts).bind(id.to_string()).execute(&mut *c).await?;
                    let row = Self::waste(c, id).await?;
                    event(c, "stock_waste", id, "approval.decided", Some(location.venue_location_id), false, json!({"waste_event_id":id,"entity_type":"stock_waste_event","status":row.status,"requestedBy":row.created_by,"event":row})).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn get_location(&self, id: Uuid) -> Result<InventoryLocation, AppError> {
        self.find_location_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Location {id} not found")))
    }
    pub async fn get_receipt(&self, id: Uuid) -> Result<StockReceiptWithLines, AppError> {
        let receipt=sqlx::query_as("SELECT unhex(replace(id,'-','')) AS id,unhex(replace(inventory_location_id,'-','')) AS location_id,unhex(replace(vendor_id,'-','')) AS vendor_id,notes,unhex(replace(created_by,'-','')) AS created_by,created_at FROM stock_receipts WHERE id=?").bind(id.to_string()).fetch_optional(&self.db.read_pool()?).await?.ok_or_else(||AppError::NotFound(format!("Receipt {id} not found")))?;
        Ok(StockReceiptWithLines {
            receipt,
            lines: self.receipt_lines(id).await?,
        })
    }
    pub async fn get_adjustment(&self, id: Uuid) -> Result<StockAdjustmentWithLines, AppError> {
        let adjustment = self
            .find_adjustment_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Adjustment {id} not found")))?;
        Ok(StockAdjustmentWithLines {
            adjustment,
            lines: self.adjustment_lines(id).await?,
        })
    }
    pub async fn get_transfer_request(
        &self,
        id: Uuid,
    ) -> Result<StockTransferRequestWithLines, AppError> {
        let request = self
            .find_transfer_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Transfer request {id} not found")))?;
        Ok(StockTransferRequestWithLines {
            request,
            lines: self.transfer_lines(id).await?,
        })
    }
    pub async fn get_waste_event(&self, id: Uuid) -> Result<StockWasteEventWithLines, AppError> {
        let event = self
            .find_waste_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Waste event {id} not found")))?;
        Ok(StockWasteEventWithLines {
            event,
            lines: self.waste_lines(id).await?,
        })
    }
    pub async fn request_transfer(
        &self,
        dto: CreateStockTransferRequestDto,
        actor: Option<Uuid>,
    ) -> Result<StockTransferRequestWithLines, AppError> {
        let from = match dto.from_location_id {
            Some(id) => id,
            None => self
                .get_config_location_id("inventory.default_warehouse_id")
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest("Default warehouse location is not configured".into())
                })?,
        };
        let to = match dto.to_location_id {
            Some(id) => id,
            None => self
                .get_config_location_id("inventory.default_store_id")
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest("Default store location is not configured".into())
                })?,
        };
        let lines = dto
            .lines
            .into_iter()
            .map(|x| (x.product_id, x.quantity_pieces))
            .collect::<Vec<_>>();
        let (request, lines) = self
            .create_transfer_request(from, to, &lines, actor)
            .await?;
        Ok(StockTransferRequestWithLines { request, lines })
    }
}
