use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreateInventoryLocationDto, CreateStockReceiptDto,
    CreateStockWasteEventDto, InventoryLocation, InventoryLocationFilterDto,
    LocationStockFilterDto, LocationStockRow, StockReceipt, StockReceiptFilterDto,
    StockReceiptLine, StockAdjustment, StockAdjustmentFilterDto, StockAdjustmentLine,
    CreateStockAdjustmentDto, StockTransferFilterDto, StockTransferLine, StockTransferRequest,
    StockWasteEvent, StockWasteFilterDto, StockWasteLine, UpdateInventoryLocationDto,
    WasteSummaryRow,
};

pub struct InventoryRepository {
    pool: PgPool,
}

impl InventoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    const LOCATION_SELECT: &'static str = r#"
        SELECT id, name, kind::text as kind, "isActive" as is_active,
               "createdBy" as created_by, "updatedBy" as updated_by,
               "createdAt" as created_at, "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM inventory_locations
    "#;

    pub async fn find_location_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<InventoryLocation>, AppError> {
        let query = format!(
            "{} WHERE id = $1 AND \"deletedAt\" IS NULL",
            Self::LOCATION_SELECT
        );
        Ok(sqlx::query_as::<_, InventoryLocation>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn list_locations(
        &self,
        filters: &InventoryLocationFilterDto,
    ) -> Result<PaginationResult<InventoryLocation>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, name, kind::text as kind, \"isActive\" as is_active, \
             \"createdBy\" as created_by, \"updatedBy\" as updated_by, \
             \"createdAt\" as created_at, \"updatedAt\" as updated_at, \
             \"deletedAt\" as deleted_at \
             FROM inventory_locations WHERE \"deletedAt\" IS NULL",
        );

        if let Some(kind) = &filters.kind {
            builder.push(" AND kind::text = ");
            builder.push_bind(kind);
        }
        if let Some(is_active) = filters.is_active {
            builder.push(" AND \"isActive\" = ");
            builder.push_bind(is_active);
        }

        builder.push(" ORDER BY name ASC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<InventoryLocation>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT COUNT(*) FROM inventory_locations WHERE \"deletedAt\" IS NULL",
        );
        if let Some(kind) = &filters.kind {
            count_builder.push(" AND kind::text = ");
            count_builder.push_bind(kind);
        }
        if let Some(is_active) = filters.is_active {
            count_builder.push(" AND \"isActive\" = ");
            count_builder.push_bind(is_active);
        }

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }

    pub async fn create_location(
        &self,
        dto: &CreateInventoryLocationDto,
        actor_id: Option<Uuid>,
    ) -> Result<InventoryLocation, AppError> {
        let is_active = dto.is_active.unwrap_or(true);
        Ok(sqlx::query_as::<_, InventoryLocation>(
            r#"
            INSERT INTO inventory_locations (id, name, kind, "isActive", "createdBy", "updatedBy")
            VALUES (gen_random_uuid(), $1, $2::inventory_location_kind, $3, $4, $4)
            RETURNING id, name, kind::text as kind, "isActive" as is_active,
                      "createdBy" as created_by, "updatedBy" as updated_by,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(&dto.name)
        .bind(&dto.kind)
        .bind(is_active)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn update_location(
        &self,
        id: Uuid,
        dto: &UpdateInventoryLocationDto,
        actor_id: Option<Uuid>,
    ) -> Result<InventoryLocation, AppError> {
        let row = sqlx::query_as::<_, InventoryLocation>(
            r#"
            UPDATE inventory_locations SET
                name = COALESCE($2, name),
                kind = COALESCE($3::inventory_location_kind, kind),
                "isActive" = COALESCE($4, "isActive"),
                "updatedBy" = COALESCE($5, "updatedBy"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id, name, kind::text as kind, "isActive" as is_active,
                      "createdBy" as created_by, "updatedBy" as updated_by,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(&dto.name)
        .bind(&dto.kind)
        .bind(dto.is_active)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?;

        row.ok_or_else(|| AppError::NotFound(format!("Location {id} not found")))
    }

    pub async fn soft_delete_location(&self, id: Uuid) -> Result<InventoryLocation, AppError> {
        let row = sqlx::query_as::<_, InventoryLocation>(
            r#"
            UPDATE inventory_locations SET "deletedAt" = NOW(), "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id, name, kind::text as kind, "isActive" as is_active,
                      "createdBy" as created_by, "updatedBy" as updated_by,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        row.ok_or_else(|| AppError::NotFound(format!("Location {id} not found")))
    }

    pub async fn list_stock(
        &self,
        filters: &LocationStockFilterDto,
    ) -> Result<PaginationResult<LocationStockRow>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT ls.\"locationId\" as location_id, ls.\"productId\" as product_id, \
             ls.\"quantityPieces\" as quantity_pieces, p.name as product_name, p.sku as product_sku, \
             ls.\"createdAt\" as created_at, ls.\"updatedAt\" as updated_at \
             FROM location_stock ls \
             INNER JOIN products p ON p.id = ls.\"productId\" AND p.\"deletedAt\" IS NULL \
             WHERE 1=1",
        );

        if let Some(location_id) = filters.location_id {
            builder.push(" AND ls.\"locationId\" = ");
            builder.push_bind(location_id);
        }
        if let Some(product_id) = filters.product_id {
            builder.push(" AND ls.\"productId\" = ");
            builder.push_bind(product_id);
        }

        builder.push(" ORDER BY p.name ASC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<LocationStockRow>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM location_stock ls WHERE 1=1");
        if let Some(location_id) = filters.location_id {
            count_builder.push(" AND ls.\"locationId\" = ");
            count_builder.push_bind(location_id);
        }
        if let Some(product_id) = filters.product_id {
            count_builder.push(" AND ls.\"productId\" = ");
            count_builder.push_bind(product_id);
        }

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }

    pub async fn stock_quantity_at(
        &self,
        location_id: Uuid,
        product_id: Uuid,
    ) -> Result<i32, AppError> {
        let row: Option<(i32,)> = sqlx::query_as(
            r#"SELECT "quantityPieces" FROM location_stock
               WHERE "locationId" = $1 AND "productId" = $2"#,
        )
        .bind(location_id)
        .bind(product_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(q,)| q).unwrap_or(0))
    }

    pub async fn create_adjustment(
        &self,
        dto: &CreateStockAdjustmentDto,
        created_by: Option<Uuid>,
    ) -> Result<(StockAdjustment, Vec<StockAdjustmentLine>), AppError> {
        let mut tx = self.pool.begin().await?;

        let adjustment = sqlx::query_as::<_, StockAdjustment>(
            r#"
            INSERT INTO stock_adjustments (id, "locationId", notes, "createdBy")
            VALUES (gen_random_uuid(), $1, $2, $3)
            RETURNING id, "locationId" as location_id, notes,
                      "createdBy" as created_by, "createdAt" as created_at
            "#,
        )
        .bind(dto.location_id)
        .bind(dto.notes.trim())
        .bind(created_by)
        .fetch_one(&mut *tx)
        .await?;

        let mut saved_lines = Vec::with_capacity(dto.lines.len());
        for line in &dto.lines {
            let previous: Option<(i32,)> = sqlx::query_as(
                r#"SELECT "quantityPieces" FROM location_stock
                   WHERE "locationId" = $1 AND "productId" = $2 FOR UPDATE"#,
            )
            .bind(dto.location_id)
            .bind(line.product_id)
            .fetch_optional(&mut *tx)
            .await?;
            let previous_pieces = previous.map(|(q,)| q).unwrap_or(0);
            let delta = line.counted_pieces - previous_pieces;

            let saved = sqlx::query_as::<_, StockAdjustmentLine>(
                r#"
                INSERT INTO stock_adjustment_lines (
                    id, "adjustmentId", "productId", "previousPieces", "countedPieces", "deltaPieces"
                )
                VALUES (gen_random_uuid(), $1, $2, $3, $4, $5)
                RETURNING id, "adjustmentId" as adjustment_id, "productId" as product_id,
                          "previousPieces" as previous_pieces, "countedPieces" as counted_pieces,
                          "deltaPieces" as delta_pieces
                "#,
            )
            .bind(adjustment.id)
            .bind(line.product_id)
            .bind(previous_pieces)
            .bind(line.counted_pieces)
            .bind(delta)
            .fetch_one(&mut *tx)
            .await?;

            if delta != 0 {
                Self::adjust_stock_in_tx(
                    &mut tx,
                    dto.location_id,
                    line.product_id,
                    delta,
                    "adjustment",
                    adjustment.id,
                    "stock_adjustment",
                    created_by,
                )
                .await?;
            }

            saved_lines.push(saved);
        }

        tx.commit().await?;
        Ok((adjustment, saved_lines))
    }

    pub async fn list_adjustments(
        &self,
        filters: &StockAdjustmentFilterDto,
    ) -> Result<PaginationResult<StockAdjustment>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, \"locationId\" as location_id, notes, \"createdBy\" as created_by, \
             \"createdAt\" as created_at FROM stock_adjustments WHERE 1=1",
        );

        if let Some(location_id) = filters.location_id {
            builder.push(" AND \"locationId\" = ");
            builder.push_bind(location_id);
        }

        builder.push(" ORDER BY \"createdAt\" DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<StockAdjustment>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM stock_adjustments WHERE 1=1");
        if let Some(location_id) = filters.location_id {
            count_builder.push(" AND \"locationId\" = ");
            count_builder.push_bind(location_id);
        }

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }

    pub async fn find_adjustment_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StockAdjustment>, AppError> {
        Ok(sqlx::query_as::<_, StockAdjustment>(
            r#"
            SELECT id, "locationId" as location_id, notes, "createdBy" as created_by,
                   "createdAt" as created_at
            FROM stock_adjustments WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn adjustment_lines(
        &self,
        adjustment_id: Uuid,
    ) -> Result<Vec<StockAdjustmentLine>, AppError> {
        Ok(sqlx::query_as::<_, StockAdjustmentLine>(
            r#"
            SELECT id, "adjustmentId" as adjustment_id, "productId" as product_id,
                   "previousPieces" as previous_pieces, "countedPieces" as counted_pieces,
                   "deltaPieces" as delta_pieces
            FROM stock_adjustment_lines WHERE "adjustmentId" = $1
            "#,
        )
        .bind(adjustment_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_config_location_id(&self, key: &str) -> Result<Option<Uuid>, AppError> {
        let row: Option<(String,)> =
            sqlx::query_as(r#"SELECT value #>> '{}' FROM configurations WHERE key = $1"#)
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.and_then(|(v,)| Uuid::parse_str(&v).ok()))
    }

    pub async fn create_receipt(
        &self,
        dto: &CreateStockReceiptDto,
        created_by: Option<Uuid>,
    ) -> Result<(StockReceipt, Vec<StockReceiptLine>), AppError> {
        let mut tx = self.pool.begin().await?;

        let receipt = sqlx::query_as::<_, StockReceipt>(
            r#"
            INSERT INTO stock_receipts (id, "locationId", "vendorId", notes, "createdBy")
            VALUES (gen_random_uuid(), $1, $2, $3, $4)
            RETURNING id, "locationId" as location_id, "vendorId" as vendor_id, notes,
                      "createdBy" as created_by, "createdAt" as created_at
            "#,
        )
        .bind(dto.location_id)
        .bind(dto.vendor_id)
        .bind(&dto.notes)
        .bind(created_by)
        .fetch_one(&mut *tx)
        .await?;

        let mut lines = Vec::with_capacity(dto.lines.len());
        for line in &dto.lines {
            let product: (i32,) = sqlx::query_as(
                r#"SELECT "unitsPerPurchaseUnit" FROM products WHERE id = $1 AND "deletedAt" IS NULL"#,
            )
            .bind(line.product_id)
            .fetch_one(&mut *tx)
            .await?;

            let pieces_added = line.box_quantity * product.0;
            let saved = sqlx::query_as::<_, StockReceiptLine>(
                r#"
                INSERT INTO stock_receipt_lines (id, "receiptId", "productId", "boxQuantity", "piecesAdded")
                VALUES (gen_random_uuid(), $1, $2, $3, $4)
                RETURNING id, "receiptId" as receipt_id, "productId" as product_id,
                          "boxQuantity" as box_quantity, "piecesAdded" as pieces_added
                "#,
            )
            .bind(receipt.id)
            .bind(line.product_id)
            .bind(line.box_quantity)
            .bind(pieces_added)
            .fetch_one(&mut *tx)
            .await?;

            Self::adjust_stock_in_tx(
                &mut tx,
                dto.location_id,
                line.product_id,
                pieces_added,
                "receipt",
                receipt.id,
                "stock_receipt",
                created_by,
            )
            .await?;

            lines.push(saved);
        }

        tx.commit().await?;
        Ok((receipt, lines))
    }

    pub async fn list_receipts(
        &self,
        filters: &StockReceiptFilterDto,
    ) -> Result<PaginationResult<StockReceipt>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, \"locationId\" as location_id, \"vendorId\" as vendor_id, notes, \
             \"createdBy\" as created_by, \"createdAt\" as created_at \
             FROM stock_receipts WHERE 1=1",
        );

        if let Some(location_id) = filters.location_id {
            builder.push(" AND \"locationId\" = ");
            builder.push_bind(location_id);
        }

        builder.push(" ORDER BY \"createdAt\" DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<StockReceipt>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM stock_receipts WHERE 1=1");
        if let Some(location_id) = filters.location_id {
            count_builder.push(" AND \"locationId\" = ");
            count_builder.push_bind(location_id);
        }

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }

    pub async fn receipt_lines(&self, receipt_id: Uuid) -> Result<Vec<StockReceiptLine>, AppError> {
        Ok(sqlx::query_as::<_, StockReceiptLine>(
            r#"
            SELECT id, "receiptId" as receipt_id, "productId" as product_id,
                   "boxQuantity" as box_quantity, "piecesAdded" as pieces_added
            FROM stock_receipt_lines WHERE "receiptId" = $1
            "#,
        )
        .bind(receipt_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn create_transfer_request(
        &self,
        from_location_id: Uuid,
        to_location_id: Uuid,
        lines: &[(Uuid, i32)],
        requested_by: Option<Uuid>,
    ) -> Result<(StockTransferRequest, Vec<StockTransferLine>), AppError> {
        let mut tx = self.pool.begin().await?;

        let request = sqlx::query_as::<_, StockTransferRequest>(
            r#"
            INSERT INTO stock_transfer_requests (
                id, "fromLocationId", "toLocationId", status, "requestedBy"
            )
            VALUES (gen_random_uuid(), $1, $2, 'pending', $3)
            RETURNING id, "fromLocationId" as from_location_id, "toLocationId" as to_location_id,
                      status::text as status, "requestedBy" as requested_by,
                      "approvedBy" as approved_by, "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason, "fulfilledBy" as fulfilled_by,
                      "fulfilledAt" as fulfilled_at, "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(from_location_id)
        .bind(to_location_id)
        .bind(requested_by)
        .fetch_one(&mut *tx)
        .await?;

        let mut saved_lines = Vec::with_capacity(lines.len());
        for (product_id, qty) in lines {
            let line = sqlx::query_as::<_, StockTransferLine>(
                r#"
                INSERT INTO stock_transfer_lines (id, "transferRequestId", "productId", "quantityPieces")
                VALUES (gen_random_uuid(), $1, $2, $3)
                RETURNING id, "transferRequestId" as transfer_request_id, "productId" as product_id,
                          "quantityPieces" as quantity_pieces
                "#,
            )
            .bind(request.id)
            .bind(product_id)
            .bind(qty)
            .fetch_one(&mut *tx)
            .await?;
            saved_lines.push(line);
        }

        tx.commit().await?;
        Ok((request, saved_lines))
    }

    pub async fn find_transfer_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StockTransferRequest>, AppError> {
        Ok(sqlx::query_as::<_, StockTransferRequest>(
            r#"
            SELECT id, "fromLocationId" as from_location_id, "toLocationId" as to_location_id,
                   status::text as status, "requestedBy" as requested_by,
                   "approvedBy" as approved_by, "approvedAt" as approved_at,
                   "rejectionReason" as rejection_reason, "fulfilledBy" as fulfilled_by,
                   "fulfilledAt" as fulfilled_at, "createdAt" as created_at,
                   "updatedAt" as updated_at
            FROM stock_transfer_requests WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn transfer_lines(
        &self,
        request_id: Uuid,
    ) -> Result<Vec<StockTransferLine>, AppError> {
        Ok(sqlx::query_as::<_, StockTransferLine>(
            r#"
            SELECT id, "transferRequestId" as transfer_request_id, "productId" as product_id,
                   "quantityPieces" as quantity_pieces
            FROM stock_transfer_lines WHERE "transferRequestId" = $1
            "#,
        )
        .bind(request_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_transfer_requests(
        &self,
        filters: &StockTransferFilterDto,
    ) -> Result<PaginationResult<StockTransferRequest>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, \"fromLocationId\" as from_location_id, \"toLocationId\" as to_location_id, \
             status::text as status, \"requestedBy\" as requested_by, \
             \"approvedBy\" as approved_by, \"approvedAt\" as approved_at, \
             \"rejectionReason\" as rejection_reason, \"fulfilledBy\" as fulfilled_by, \
             \"fulfilledAt\" as fulfilled_at, \"createdAt\" as created_at, \
             \"updatedAt\" as updated_at FROM stock_transfer_requests WHERE 1=1",
        );

        if let Some(status) = &filters.status {
            builder.push(" AND status::text = ");
            builder.push_bind(status);
        }
        if let Some(from) = filters.from_location_id {
            builder.push(" AND \"fromLocationId\" = ");
            builder.push_bind(from);
        }
        if let Some(to) = filters.to_location_id {
            builder.push(" AND \"toLocationId\" = ");
            builder.push_bind(to);
        }

        builder.push(" ORDER BY \"createdAt\" DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<StockTransferRequest>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM stock_transfer_requests WHERE 1=1");
        if let Some(status) = &filters.status {
            count_builder.push(" AND status::text = ");
            count_builder.push_bind(status);
        }
        if let Some(from) = filters.from_location_id {
            count_builder.push(" AND \"fromLocationId\" = ");
            count_builder.push_bind(from);
        }
        if let Some(to) = filters.to_location_id {
            count_builder.push(" AND \"toLocationId\" = ");
            count_builder.push_bind(to);
        }

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }

    pub async fn approve_transfer(
        &self,
        id: Uuid,
        approved_by: Uuid,
    ) -> Result<StockTransferRequest, AppError> {
        let row = sqlx::query_as::<_, StockTransferRequest>(
            r#"
            UPDATE stock_transfer_requests SET
                status = 'approved',
                "approvedBy" = $2,
                "approvedAt" = NOW(),
                "updatedAt" = NOW()
            WHERE id = $1 AND status = 'pending'
            RETURNING id, "fromLocationId" as from_location_id, "toLocationId" as to_location_id,
                      status::text as status, "requestedBy" as requested_by,
                      "approvedBy" as approved_by, "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason, "fulfilledBy" as fulfilled_by,
                      "fulfilledAt" as fulfilled_at, "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(approved_by)
        .fetch_optional(&self.pool)
        .await?;

        row.ok_or_else(|| AppError::NotFound(format!("Transfer request {id} not found or not pending")))
    }

    pub async fn reject_transfer(
        &self,
        id: Uuid,
        reason: &str,
        rejected_by: Uuid,
    ) -> Result<StockTransferRequest, AppError> {
        let row = sqlx::query_as::<_, StockTransferRequest>(
            r#"
            UPDATE stock_transfer_requests SET
                status = 'rejected',
                "rejectionReason" = $2,
                "approvedBy" = $3,
                "approvedAt" = NOW(),
                "updatedAt" = NOW()
            WHERE id = $1 AND status = 'pending'
            RETURNING id, "fromLocationId" as from_location_id, "toLocationId" as to_location_id,
                      status::text as status, "requestedBy" as requested_by,
                      "approvedBy" as approved_by, "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason, "fulfilledBy" as fulfilled_by,
                      "fulfilledAt" as fulfilled_at, "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(reason)
        .bind(rejected_by)
        .fetch_optional(&self.pool)
        .await?;

        row.ok_or_else(|| AppError::NotFound(format!("Transfer request {id} not found or not pending")))
    }

    pub async fn fulfill_transfer(
        &self,
        id: Uuid,
        fulfilled_by: Uuid,
    ) -> Result<StockTransferRequest, AppError> {
        let mut tx = self.pool.begin().await?;

        let request = sqlx::query_as::<_, StockTransferRequest>(
            r#"
            SELECT id, "fromLocationId" as from_location_id, "toLocationId" as to_location_id,
                   status::text as status, "requestedBy" as requested_by,
                   "approvedBy" as approved_by, "approvedAt" as approved_at,
                   "rejectionReason" as rejection_reason, "fulfilledBy" as fulfilled_by,
                   "fulfilledAt" as fulfilled_at, "createdAt" as created_at,
                   "updatedAt" as updated_at
            FROM stock_transfer_requests WHERE id = $1 FOR UPDATE
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Transfer request {id} not found")))?;

        if request.status != "approved" {
            return Err(AppError::BadRequest(
                "Transfer request must be approved before fulfillment".to_string(),
            ));
        }

        let lines = Self::transfer_lines_in_tx(&mut tx, id).await?;
        for line in &lines {
            let available: Option<(i32,)> = sqlx::query_as(
                r#"SELECT "quantityPieces" FROM location_stock
                   WHERE "locationId" = $1 AND "productId" = $2 FOR UPDATE"#,
            )
            .bind(request.from_location_id)
            .bind(line.product_id)
            .fetch_optional(&mut *tx)
            .await?;

            let stock = available.map(|(q,)| q).unwrap_or(0);
            if stock < line.quantity_pieces {
                return Err(AppError::Conflict(format!(
                    "Insufficient warehouse stock for product {} (available: {}, requested: {})",
                    line.product_id, stock, line.quantity_pieces
                )));
            }

            Self::adjust_stock_in_tx(
                &mut tx,
                request.from_location_id,
                line.product_id,
                -line.quantity_pieces,
                "transfer_out",
                request.id,
                "stock_transfer_request",
                Some(fulfilled_by),
            )
            .await?;

            Self::adjust_stock_in_tx(
                &mut tx,
                request.to_location_id,
                line.product_id,
                line.quantity_pieces,
                "transfer_in",
                request.id,
                "stock_transfer_request",
                Some(fulfilled_by),
            )
            .await?;
        }

        let updated = sqlx::query_as::<_, StockTransferRequest>(
            r#"
            UPDATE stock_transfer_requests SET
                status = 'fulfilled',
                "fulfilledBy" = $2,
                "fulfilledAt" = NOW(),
                "updatedAt" = NOW()
            WHERE id = $1
            RETURNING id, "fromLocationId" as from_location_id, "toLocationId" as to_location_id,
                      status::text as status, "requestedBy" as requested_by,
                      "approvedBy" as approved_by, "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason, "fulfilledBy" as fulfilled_by,
                      "fulfilledAt" as fulfilled_at, "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(fulfilled_by)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(updated)
    }

    async fn transfer_lines_in_tx(
        tx: &mut Transaction<'_, Postgres>,
        request_id: Uuid,
    ) -> Result<Vec<StockTransferLine>, AppError> {
        Ok(sqlx::query_as::<_, StockTransferLine>(
            r#"
            SELECT id, "transferRequestId" as transfer_request_id, "productId" as product_id,
                   "quantityPieces" as quantity_pieces
            FROM stock_transfer_lines WHERE "transferRequestId" = $1
            "#,
        )
        .bind(request_id)
        .fetch_all(&mut **tx)
        .await?)
    }

    pub async fn create_waste_event(
        &self,
        dto: &CreateStockWasteEventDto,
        created_by: Option<Uuid>,
    ) -> Result<(StockWasteEvent, Vec<StockWasteLine>), AppError> {
        let mut tx = self.pool.begin().await?;

        let event = sqlx::query_as::<_, StockWasteEvent>(
            r#"
            INSERT INTO stock_waste_events (id, "locationId", status, notes, "createdBy")
            VALUES (gen_random_uuid(), $1, 'pending', $2, $3)
            RETURNING id, "locationId" as location_id, status::text as status, notes,
                      "approvedBy" as approved_by, "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason, "createdBy" as created_by,
                      "createdAt" as created_at, "updatedAt" as updated_at
            "#,
        )
        .bind(dto.location_id)
        .bind(&dto.notes)
        .bind(created_by)
        .fetch_one(&mut *tx)
        .await?;

        let mut lines = Vec::with_capacity(dto.lines.len());
        for line in &dto.lines {
            let saved = sqlx::query_as::<_, StockWasteLine>(
                r#"
                INSERT INTO stock_waste_lines (
                    id, "wasteEventId", "productId", "quantityPieces", "reasonCode", note
                )
                VALUES (gen_random_uuid(), $1, $2, $3, $4::stock_waste_reason, $5)
                RETURNING id, "wasteEventId" as waste_event_id, "productId" as product_id,
                          "quantityPieces" as quantity_pieces, "reasonCode"::text as reason_code, note
                "#,
            )
            .bind(event.id)
            .bind(line.product_id)
            .bind(line.quantity_pieces)
            .bind(&line.reason_code)
            .bind(&line.note)
            .fetch_one(&mut *tx)
            .await?;
            lines.push(saved);
        }

        tx.commit().await?;
        Ok((event, lines))
    }

    pub async fn find_waste_by_id(&self, id: Uuid) -> Result<Option<StockWasteEvent>, AppError> {
        Ok(sqlx::query_as::<_, StockWasteEvent>(
            r#"
            SELECT id, "locationId" as location_id, status::text as status, notes,
                   "approvedBy" as approved_by, "approvedAt" as approved_at,
                   "rejectionReason" as rejection_reason, "createdBy" as created_by,
                   "createdAt" as created_at, "updatedAt" as updated_at
            FROM stock_waste_events WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn waste_lines(&self, event_id: Uuid) -> Result<Vec<StockWasteLine>, AppError> {
        Ok(sqlx::query_as::<_, StockWasteLine>(
            r#"
            SELECT id, "wasteEventId" as waste_event_id, "productId" as product_id,
                   "quantityPieces" as quantity_pieces, "reasonCode"::text as reason_code, note
            FROM stock_waste_lines WHERE "wasteEventId" = $1
            "#,
        )
        .bind(event_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_waste_events(
        &self,
        filters: &StockWasteFilterDto,
    ) -> Result<PaginationResult<StockWasteEvent>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, \"locationId\" as location_id, status::text as status, notes, \
             \"approvedBy\" as approved_by, \"approvedAt\" as approved_at, \
             \"rejectionReason\" as rejection_reason, \"createdBy\" as created_by, \
             \"createdAt\" as created_at, \"updatedAt\" as updated_at \
             FROM stock_waste_events WHERE 1=1",
        );

        if let Some(status) = &filters.status {
            builder.push(" AND status::text = ");
            builder.push_bind(status);
        }
        if let Some(location_id) = filters.location_id {
            builder.push(" AND \"locationId\" = ");
            builder.push_bind(location_id);
        }
        if let Some(from) = filters.from {
            builder.push(" AND \"createdAt\" >= ");
            builder.push_bind(from);
        }
        if let Some(to) = filters.to {
            builder.push(" AND \"createdAt\" <= ");
            builder.push_bind(to);
        }

        builder.push(" ORDER BY \"createdAt\" DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<StockWasteEvent>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM stock_waste_events WHERE 1=1");
        if let Some(status) = &filters.status {
            count_builder.push(" AND status::text = ");
            count_builder.push_bind(status);
        }
        if let Some(location_id) = filters.location_id {
            count_builder.push(" AND \"locationId\" = ");
            count_builder.push_bind(location_id);
        }
        if let Some(from) = filters.from {
            count_builder.push(" AND \"createdAt\" >= ");
            count_builder.push_bind(from);
        }
        if let Some(to) = filters.to {
            count_builder.push(" AND \"createdAt\" <= ");
            count_builder.push_bind(to);
        }

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;
        Ok(PaginationResult::new(rows, total.0, page, limit))
    }

    pub async fn approve_waste(
        &self,
        id: Uuid,
        approved_by: Uuid,
    ) -> Result<StockWasteEvent, AppError> {
        let mut tx = self.pool.begin().await?;

        let event = sqlx::query_as::<_, StockWasteEvent>(
            r#"
            SELECT id, "locationId" as location_id, status::text as status, notes,
                   "approvedBy" as approved_by, "approvedAt" as approved_at,
                   "rejectionReason" as rejection_reason, "createdBy" as created_by,
                   "createdAt" as created_at, "updatedAt" as updated_at
            FROM stock_waste_events WHERE id = $1 FOR UPDATE
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Waste event {id} not found")))?;

        if event.status != "pending" {
            return Err(AppError::BadRequest(
                "Waste event is not pending approval".to_string(),
            ));
        }

        let lines = Self::waste_lines_in_tx(&mut tx, id).await?;
        for line in &lines {
            let available: Option<(i32,)> = sqlx::query_as(
                r#"SELECT "quantityPieces" FROM location_stock
                   WHERE "locationId" = $1 AND "productId" = $2 FOR UPDATE"#,
            )
            .bind(event.location_id)
            .bind(line.product_id)
            .fetch_optional(&mut *tx)
            .await?;

            let stock = available.map(|(q,)| q).unwrap_or(0);
            if stock < line.quantity_pieces {
                return Err(AppError::Conflict(format!(
                    "Insufficient stock for product {} (available: {}, waste: {})",
                    line.product_id, stock, line.quantity_pieces
                )));
            }

            Self::adjust_stock_in_tx(
                &mut tx,
                event.location_id,
                line.product_id,
                -line.quantity_pieces,
                "waste",
                event.id,
                "stock_waste_event",
                Some(approved_by),
            )
            .await?;
        }

        let updated = sqlx::query_as::<_, StockWasteEvent>(
            r#"
            UPDATE stock_waste_events SET
                status = 'approved',
                "approvedBy" = $2,
                "approvedAt" = NOW(),
                "updatedAt" = NOW()
            WHERE id = $1
            RETURNING id, "locationId" as location_id, status::text as status, notes,
                      "approvedBy" as approved_by, "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason, "createdBy" as created_by,
                      "createdAt" as created_at, "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(approved_by)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(updated)
    }

    async fn waste_lines_in_tx(
        tx: &mut Transaction<'_, Postgres>,
        event_id: Uuid,
    ) -> Result<Vec<StockWasteLine>, AppError> {
        Ok(sqlx::query_as::<_, StockWasteLine>(
            r#"
            SELECT id, "wasteEventId" as waste_event_id, "productId" as product_id,
                   "quantityPieces" as quantity_pieces, "reasonCode"::text as reason_code, note
            FROM stock_waste_lines WHERE "wasteEventId" = $1
            "#,
        )
        .bind(event_id)
        .fetch_all(&mut **tx)
        .await?)
    }

    pub async fn reject_waste(
        &self,
        id: Uuid,
        reason: &str,
        rejected_by: Uuid,
    ) -> Result<StockWasteEvent, AppError> {
        let row = sqlx::query_as::<_, StockWasteEvent>(
            r#"
            UPDATE stock_waste_events SET
                status = 'rejected',
                "rejectionReason" = $2,
                "approvedBy" = $3,
                "approvedAt" = NOW(),
                "updatedAt" = NOW()
            WHERE id = $1 AND status = 'pending'
            RETURNING id, "locationId" as location_id, status::text as status, notes,
                      "approvedBy" as approved_by, "approvedAt" as approved_at,
                      "rejectionReason" as rejection_reason, "createdBy" as created_by,
                      "createdAt" as created_at, "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(reason)
        .bind(rejected_by)
        .fetch_optional(&self.pool)
        .await?;

        row.ok_or_else(|| AppError::NotFound(format!("Waste event {id} not found or not pending")))
    }

    pub async fn waste_summary(
        &self,
        location_id: Option<Uuid>,
        from: Option<chrono::DateTime<chrono::Utc>>,
        to: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<WasteSummaryRow>, AppError> {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            SELECT wl."reasonCode"::text as reason_code,
                   wl."productId" as product_id,
                   p.name as product_name,
                   we."locationId" as location_id,
                   il.name as location_name,
                   SUM(wl."quantityPieces")::bigint as total_pieces,
                   SUM(
                     wl."quantityPieces"::float8 *
                     COALESCE(p."purchasePricePerBox", p."purchasePrice", 0)::float8 /
                     GREATEST(p."unitsPerPurchaseUnit", 1)
                   )::float8 as estimated_cost
            FROM stock_waste_lines wl
            INNER JOIN stock_waste_events we ON we.id = wl."wasteEventId"
            INNER JOIN products p ON p.id = wl."productId"
            INNER JOIN inventory_locations il ON il.id = we."locationId"
            WHERE we.status = 'approved'
            "#,
        );

        if let Some(location_id) = location_id {
            builder.push(" AND we.\"locationId\" = ");
            builder.push_bind(location_id);
        }
        if let Some(from) = from {
            builder.push(" AND we.\"approvedAt\" >= ");
            builder.push_bind(from);
        }
        if let Some(to) = to {
            builder.push(" AND we.\"approvedAt\" <= ");
            builder.push_bind(to);
        }

        builder.push(
            r#" GROUP BY wl."reasonCode", wl."productId", p.name, we."locationId", il.name
                ORDER BY total_pieces DESC"#,
        );

        Ok(builder.build_query_as::<WasteSummaryRow>().fetch_all(&self.pool).await?)
    }

    pub async fn deduct_sale_stock_in_tx(
        tx: &mut Transaction<'_, sqlx::Postgres>,
        location_id: Uuid,
        product_id: Uuid,
        quantity: i32,
        reference_id: Uuid,
        actor_id: Option<Uuid>,
    ) -> Result<(), AppError> {
        let available: Option<(i32,)> = sqlx::query_as(
            r#"SELECT "quantityPieces" FROM location_stock
               WHERE "locationId" = $1 AND "productId" = $2 FOR UPDATE"#,
        )
        .bind(location_id)
        .bind(product_id)
        .fetch_optional(&mut **tx)
        .await?;

        let stock = available.map(|(q,)| q).unwrap_or(0);
        if stock < quantity {
            return Err(AppError::Conflict(format!(
                "Insufficient stock for product {product_id} at location (available: {stock}, requested: {quantity})"
            )));
        }

        Self::adjust_stock_in_tx(
            tx,
            location_id,
            product_id,
            -quantity,
            "sale",
            reference_id,
            "transaction",
            actor_id,
        )
        .await
    }

    async fn adjust_stock_in_tx(
        tx: &mut Transaction<'_, Postgres>,
        location_id: Uuid,
        product_id: Uuid,
        delta: i32,
        movement_type: &str,
        reference_id: Uuid,
        reference_type: &str,
        created_by: Option<Uuid>,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO location_stock ("locationId", "productId", "quantityPieces")
            VALUES ($1, $2, GREATEST($3, 0))
            ON CONFLICT ("locationId", "productId")
            DO UPDATE SET
                "quantityPieces" = location_stock."quantityPieces" + $3,
                "updatedAt" = NOW()
            "#,
        )
        .bind(location_id)
        .bind(product_id)
        .bind(delta)
        .execute(&mut **tx)
        .await?;

        if delta < 0 {
            let ok: Option<(bool,)> = sqlx::query_as(
                r#"SELECT ("quantityPieces" >= 0) as ok FROM location_stock
                   WHERE "locationId" = $1 AND "productId" = $2"#,
            )
            .bind(location_id)
            .bind(product_id)
            .fetch_optional(&mut **tx)
            .await?;

            if ok.map(|(v,)| v) != Some(true) {
                return Err(AppError::Conflict(
                    "Stock adjustment would result in negative quantity".to_string(),
                ));
            }
        }

        sqlx::query(
            r#"
            INSERT INTO stock_movements (
                id, "locationId", "productId", delta, "movementType",
                "referenceId", "referenceType", "createdBy"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4::stock_movement_type,
                $5, $6, $7
            )
            "#,
        )
        .bind(location_id)
        .bind(product_id)
        .bind(delta)
        .bind(movement_type)
        .bind(reference_id)
        .bind(reference_type)
        .bind(created_by)
        .execute(&mut **tx)
        .await?;

        Self::sync_store_stock_quantity_in_tx(tx, location_id, product_id).await?;
        Ok(())
    }

    async fn sync_store_stock_quantity_in_tx(
        tx: &mut Transaction<'_, Postgres>,
        location_id: Uuid,
        product_id: Uuid,
    ) -> Result<(), AppError> {
        let kind: Option<(String,)> = sqlx::query_as(
            r#"SELECT kind::text FROM inventory_locations WHERE id = $1"#,
        )
        .bind(location_id)
        .fetch_optional(&mut **tx)
        .await?;

        if kind.map(|(k,)| k) == Some("store".to_string()) {
            let qty: Option<(i32,)> = sqlx::query_as(
                r#"SELECT "quantityPieces" FROM location_stock
                   WHERE "locationId" = $1 AND "productId" = $2"#,
            )
            .bind(location_id)
            .bind(product_id)
            .fetch_optional(&mut **tx)
            .await?;

            if let Some((quantity,)) = qty {
                sqlx::query(
                    r#"UPDATE products SET "stockQuantity" = $1, "updatedAt" = NOW() WHERE id = $2"#,
                )
                .bind(quantity)
                .bind(product_id)
                .execute(&mut **tx)
                .await?;
            }
        }
        Ok(())
    }
}
