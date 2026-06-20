use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Tz;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{get_or_set, keys, CacheService};
use crate::dto::{PaginationResult, WasteSummaryFilterDto};
use crate::error::AppError;
use crate::models::{
    normalize_inventory_location_kind, normalize_stock_waste_reason, CreateInventoryLocationDto,
    CreateStockReceiptDto, CreateStockTransferRequestDto, CreateStockWasteEventDto,
    CreateStockWasteLineDto, CreateStockAdjustmentDto, InventoryLocation, InventoryLocationFilterDto,
    LocationStockFilterDto, LocationStockRow, Product, StockReceipt, StockReceiptFilterDto,
    StockReceiptWithLines, StockAdjustment, StockAdjustmentFilterDto, StockAdjustmentWithLines,
    StockTransferFilterDto, StockTransferRequest,
    StockTransferRequestWithLines, StockWasteEvent, StockWasteEventWithLines, StockWasteFilterDto,
    UpdateInventoryLocationDto, WasteSummaryRow,
};
use crate::realtime::OutboxService;
use crate::repositories::InventoryRepository;

pub struct InventoryService {
    repo: InventoryRepository,
    cafe_timezone: String,
    outbox: OutboxService,
    cache: Arc<dyn CacheService>,
}

impl InventoryService {
    pub fn new(
        pool: PgPool,
        cafe_timezone: String,
        outbox: OutboxService,
        cache: Arc<dyn CacheService>,
    ) -> Self {
        Self {
            repo: InventoryRepository::new(pool),
            cafe_timezone,
            outbox,
            cache,
        }
    }

    async fn invalidate_location_stock(&self, location_id: Uuid) -> Result<(), AppError> {
        self.cache
            .invalidate_prefix(&format!("stock:level:{location_id}:"))
            .await
    }

    pub async fn stock_quantity_at(
        &self,
        location_id: Uuid,
        product_id: Uuid,
    ) -> Result<i32, AppError> {
        let cache_key = keys::stock_level(&location_id, &product_id);
        get_or_set(&*self.cache, &cache_key, keys::ttl::AGGREGATE, || async {
            self.repo.stock_quantity_at(location_id, product_id).await
        })
        .await
    }

    pub fn cafe_timezone(&self) -> &str {
        &self.cafe_timezone
    }

    pub fn repo(&self) -> &InventoryRepository {
        &self.repo
    }

    pub fn is_night_pricing_window(now: DateTime<Utc>, cafe_tz: &str) -> bool {
        let local = Self::to_local_datetime(now, cafe_tz);
        let hour = local.hour();
        hour >= 23 || hour < 8
    }

    pub fn effective_product_price(
        day_price: f64,
        night_price: f64,
        now: DateTime<Utc>,
        cafe_tz: &str,
    ) -> f64 {
        if Self::is_night_pricing_window(now, cafe_tz) {
            night_price
        } else {
            day_price
        }
    }

    pub fn effective_product_price_for_product(
        product: &Product,
        now: DateTime<Utc>,
        cafe_tz: &str,
    ) -> f64 {
        Self::effective_product_price(product.day_price, product.night_price, now, cafe_tz)
    }

    fn to_local_datetime(now: DateTime<Utc>, cafe_tz: &str) -> DateTime<Tz> {
        let tz: Tz = cafe_tz.parse().unwrap_or(chrono_tz::Asia::Kolkata);
        now.with_timezone(&tz)
    }

    pub async fn list_locations(
        &self,
        filters: InventoryLocationFilterDto,
    ) -> Result<PaginationResult<InventoryLocation>, AppError> {
        self.repo.list_locations(&filters).await
    }

    pub async fn get_location(&self, id: Uuid) -> Result<InventoryLocation, AppError> {
        self.repo
            .find_location_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Location {id} not found")))
    }

    pub async fn create_location(
        &self,
        dto: CreateInventoryLocationDto,
        actor_id: Option<Uuid>,
    ) -> Result<InventoryLocation, AppError> {
        let kind = normalize_inventory_location_kind(&dto.kind).ok_or_else(|| {
            AppError::BadRequest("kind must be warehouse or store".to_string())
        })?;
        let mut dto = dto;
        dto.kind = kind;
        self.repo.create_location(&dto, actor_id).await
    }

    pub async fn update_location(
        &self,
        id: Uuid,
        mut dto: UpdateInventoryLocationDto,
        actor_id: Option<Uuid>,
    ) -> Result<InventoryLocation, AppError> {
        if let Some(kind) = dto.kind.take() {
            dto.kind = normalize_inventory_location_kind(&kind);
            if dto.kind.is_none() {
                return Err(AppError::BadRequest(
                    "kind must be warehouse or store".to_string(),
                ));
            }
        }
        self.repo.update_location(id, &dto, actor_id).await
    }

    pub async fn delete_location(&self, id: Uuid) -> Result<InventoryLocation, AppError> {
        self.repo.soft_delete_location(id).await
    }

    pub async fn list_stock(
        &self,
        filters: LocationStockFilterDto,
    ) -> Result<PaginationResult<LocationStockRow>, AppError> {
        self.repo.list_stock(&filters).await
    }

    pub async fn receive_stock(
        &self,
        dto: CreateStockReceiptDto,
        created_by: Option<Uuid>,
    ) -> Result<StockReceiptWithLines, AppError> {
        if dto.lines.is_empty() {
            return Err(AppError::BadRequest("lines must not be empty".to_string()));
        }

        let location = self.get_location(dto.location_id).await?;
        if location.kind != "warehouse" {
            return Err(AppError::BadRequest(
                "Stock receipts must target a warehouse location".to_string(),
            ));
        }

        for line in &dto.lines {
            if line.box_quantity <= 0 {
                return Err(AppError::BadRequest(
                    "boxQuantity must be positive".to_string(),
                ));
            }
        }

        let (receipt, lines) = self.repo.create_receipt(&dto, created_by).await?;
        self.invalidate_location_stock(dto.location_id).await?;
        Ok(StockReceiptWithLines { receipt, lines })
    }

    pub async fn list_receipts(
        &self,
        filters: StockReceiptFilterDto,
    ) -> Result<PaginationResult<StockReceipt>, AppError> {
        self.repo.list_receipts(&filters).await
    }

    pub async fn get_receipt(&self, id: Uuid) -> Result<StockReceiptWithLines, AppError> {
        let receipt = sqlx::query_as::<_, StockReceipt>(
            r#"
            SELECT id, "locationId" as location_id, "vendorId" as vendor_id, notes,
                   "createdBy" as created_by, "createdAt" as created_at
            FROM stock_receipts WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.repo.pool())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Receipt {id} not found")))?;

        let lines = self.repo.receipt_lines(id).await?;
        Ok(StockReceiptWithLines { receipt, lines })
    }

    pub async fn create_adjustment(
        &self,
        dto: CreateStockAdjustmentDto,
        created_by: Option<Uuid>,
    ) -> Result<StockAdjustmentWithLines, AppError> {
        if dto.lines.is_empty() {
            return Err(AppError::BadRequest("lines must not be empty".to_string()));
        }
        if dto.notes.trim().len() < 3 {
            return Err(AppError::BadRequest(
                "notes must be at least 3 characters".to_string(),
            ));
        }

        self.get_location(dto.location_id).await?;

        for line in &dto.lines {
            if line.counted_pieces < 0 {
                return Err(AppError::BadRequest(
                    "countedPieces must be zero or positive".to_string(),
                ));
            }
        }

        let has_change = {
            let mut any = false;
            for line in &dto.lines {
                let current = self
                    .stock_quantity_at(dto.location_id, line.product_id)
                    .await?;
                if line.counted_pieces != current {
                    any = true;
                    break;
                }
            }
            any
        };
        if !has_change {
            return Err(AppError::BadRequest(
                "No quantity changes — counted pieces match current stock".to_string(),
            ));
        }

        let (adjustment, lines) = self.repo.create_adjustment(&dto, created_by).await?;
        self.invalidate_location_stock(dto.location_id).await?;
        Ok(StockAdjustmentWithLines { adjustment, lines })
    }

    pub async fn list_adjustments(
        &self,
        filters: StockAdjustmentFilterDto,
    ) -> Result<PaginationResult<StockAdjustment>, AppError> {
        self.repo.list_adjustments(&filters).await
    }

    pub async fn get_adjustment(&self, id: Uuid) -> Result<StockAdjustmentWithLines, AppError> {
        let adjustment = self
            .repo
            .find_adjustment_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Adjustment {id} not found")))?;
        let lines = self.repo.adjustment_lines(id).await?;
        Ok(StockAdjustmentWithLines { adjustment, lines })
    }

    pub async fn create_transfer_request(
        &self,
        dto: CreateStockTransferRequestDto,
        requested_by: Option<Uuid>,
    ) -> Result<StockTransferRequestWithLines, AppError> {
        if dto.lines.is_empty() {
            return Err(AppError::BadRequest("lines must not be empty".to_string()));
        }

        let from_location_id = match dto.from_location_id {
            Some(id) => id,
            None => self
                .repo
                .get_config_location_id("inventory.default_warehouse_id")
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest("Default warehouse location is not configured".to_string())
                })?,
        };

        let to_location_id = match dto.to_location_id {
            Some(id) => id,
            None => self
                .repo
                .get_config_location_id("inventory.default_store_id")
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest("Default store location is not configured".to_string())
                })?,
        };

        let from = self.get_location(from_location_id).await?;
        let to = self.get_location(to_location_id).await?;
        if from.kind != "warehouse" || to.kind != "store" {
            return Err(AppError::BadRequest(
                "Transfer requests must be from warehouse to store".to_string(),
            ));
        }

        let lines: Vec<(Uuid, i32)> = dto
            .lines
            .iter()
            .map(|l| {
                if l.quantity_pieces <= 0 {
                    return Err(AppError::BadRequest(
                        "quantityPieces must be positive".to_string(),
                    ));
                }
                Ok((l.product_id, l.quantity_pieces))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let (request, saved_lines) = self
            .repo
            .create_transfer_request(from_location_id, to_location_id, &lines, requested_by)
            .await?;

        let payload = serde_json::json!({
            "transfer_request_id": request.id.to_string(),
            "entity_type": "stock_transfer_request",
        });
        let _ = self
            .outbox
            .publish(
                "admin",
                "approval.requested",
                payload,
                Some("admin"),
                None,
                true,
            )
            .await;

        Ok(StockTransferRequestWithLines {
            request,
            lines: saved_lines,
        })
    }

    pub async fn list_transfer_requests(
        &self,
        filters: StockTransferFilterDto,
    ) -> Result<PaginationResult<StockTransferRequest>, AppError> {
        self.repo.list_transfer_requests(&filters).await
    }

    pub async fn get_transfer_request(
        &self,
        id: Uuid,
    ) -> Result<StockTransferRequestWithLines, AppError> {
        let request = self
            .repo
            .find_transfer_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Transfer request {id} not found")))?;
        let lines = self.repo.transfer_lines(id).await?;
        Ok(StockTransferRequestWithLines { request, lines })
    }

    pub async fn approve_transfer(
        &self,
        id: Uuid,
        approved_by: Uuid,
    ) -> Result<StockTransferRequest, AppError> {
        self.repo.approve_transfer(id, approved_by).await
    }

    pub async fn reject_transfer(
        &self,
        id: Uuid,
        rejection_reason: &str,
        rejected_by: Uuid,
    ) -> Result<StockTransferRequest, AppError> {
        if rejection_reason.trim().is_empty() {
            return Err(AppError::BadRequest(
                "rejectionReason is required".to_string(),
            ));
        }
        self.repo.reject_transfer(id, rejection_reason, rejected_by).await
    }

    pub async fn fulfill_transfer(
        &self,
        id: Uuid,
        fulfilled_by: Uuid,
    ) -> Result<StockTransferRequest, AppError> {
        let transfer = self.repo.fulfill_transfer(id, fulfilled_by).await?;
        self.invalidate_location_stock(transfer.from_location_id).await?;
        self.invalidate_location_stock(transfer.to_location_id).await?;
        Ok(transfer)
    }

    pub async fn create_waste_event(
        &self,
        dto: CreateStockWasteEventDto,
        created_by: Option<Uuid>,
    ) -> Result<StockWasteEventWithLines, AppError> {
        if dto.lines.is_empty() {
            return Err(AppError::BadRequest("lines must not be empty".to_string()));
        }

        self.get_location(dto.location_id).await?;
        Self::validate_waste_lines(&dto.lines)?;

        let (event, lines) = self.repo.create_waste_event(&dto, created_by).await?;

        let payload = serde_json::json!({
            "waste_event_id": event.id.to_string(),
            "entity_type": "stock_waste_event",
        });
        let _ = self
            .outbox
            .publish(
                "admin",
                "approval.requested",
                payload,
                Some("admin"),
                None,
                true,
            )
            .await;

        Ok(StockWasteEventWithLines { event, lines })
    }

    fn validate_waste_lines(lines: &[CreateStockWasteLineDto]) -> Result<(), AppError> {
        for line in lines {
            if line.quantity_pieces <= 0 {
                return Err(AppError::BadRequest(
                    "quantityPieces must be positive".to_string(),
                ));
            }
            if normalize_stock_waste_reason(&line.reason_code).is_none() {
                return Err(AppError::BadRequest(
                    "reasonCode must be expired, damaged, spoilage, sample, or other".to_string(),
                ));
            }
            if line.reason_code == "other" {
                let note_len = line.note.as_deref().unwrap_or("").trim().len();
                if note_len < 10 {
                    return Err(AppError::BadRequest(
                        "note must be at least 10 characters when reasonCode is other".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    pub async fn list_waste_events(
        &self,
        filters: StockWasteFilterDto,
    ) -> Result<PaginationResult<StockWasteEvent>, AppError> {
        self.repo.list_waste_events(&filters).await
    }

    pub async fn get_waste_event(&self, id: Uuid) -> Result<StockWasteEventWithLines, AppError> {
        let event = self
            .repo
            .find_waste_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Waste event {id} not found")))?;
        let lines = self.repo.waste_lines(id).await?;
        Ok(StockWasteEventWithLines { event, lines })
    }

    pub async fn approve_waste(
        &self,
        id: Uuid,
        approved_by: Uuid,
    ) -> Result<StockWasteEvent, AppError> {
        let event = self.repo.approve_waste(id, approved_by).await?;
        self.invalidate_location_stock(event.location_id).await?;

        if let Some(created_by) = event.created_by {
            let payload = serde_json::json!({
                "waste_event_id": event.id.to_string(),
                "status": "approved",
                "entity_type": "stock_waste_event",
            });
            let _ = self
                .outbox
                .publish(
                    &format!("user:{created_by}"),
                    "approval.decided",
                    payload,
                    None,
                    Some(created_by),
                    true,
                )
                .await;
        }

        Ok(event)
    }

    pub async fn reject_waste(
        &self,
        id: Uuid,
        rejection_reason: &str,
        rejected_by: Uuid,
    ) -> Result<StockWasteEvent, AppError> {
        if rejection_reason.trim().is_empty() {
            return Err(AppError::BadRequest(
                "rejectionReason is required".to_string(),
            ));
        }
        let event = self.repo.reject_waste(id, rejection_reason, rejected_by).await?;

        if let Some(created_by) = event.created_by {
            let payload = serde_json::json!({
                "waste_event_id": event.id.to_string(),
                "status": "rejected",
                "entity_type": "stock_waste_event",
            });
            let _ = self
                .outbox
                .publish(
                    &format!("user:{created_by}"),
                    "approval.decided",
                    payload,
                    None,
                    Some(created_by),
                    true,
                )
                .await;
        }

        Ok(event)
    }

    pub async fn waste_summary(
        &self,
        filters: WasteSummaryFilterDto,
    ) -> Result<Vec<WasteSummaryRow>, AppError> {
        self.repo
            .waste_summary(filters.location_id, filters.from, filters.to)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const TZ: &str = "Asia/Kolkata";

    #[test]
    fn day_price_during_business_hours() {
        let now = Utc.with_ymd_and_hms(2026, 6, 6, 10, 30, 0).unwrap();
        assert!(!InventoryService::is_night_pricing_window(now, TZ));
        assert_eq!(
            InventoryService::effective_product_price(100.0, 80.0, now, TZ),
            100.0
        );
    }

    #[test]
    fn night_price_at_23_00() {
        let now = Utc.with_ymd_and_hms(2026, 6, 6, 17, 30, 0).unwrap();
        assert!(InventoryService::is_night_pricing_window(now, TZ));
        assert_eq!(
            InventoryService::effective_product_price(100.0, 80.0, now, TZ),
            80.0
        );
    }

    #[test]
    fn night_price_before_08_00() {
        let now = Utc.with_ymd_and_hms(2026, 6, 6, 2, 0, 0).unwrap();
        assert!(InventoryService::is_night_pricing_window(now, TZ));
        assert_eq!(
            InventoryService::effective_product_price(50.0, 40.0, now, TZ),
            40.0
        );
    }

    #[test]
    fn day_price_at_08_00() {
        let now = Utc.with_ymd_and_hms(2026, 6, 6, 2, 30, 0).unwrap();
        assert!(!InventoryService::is_night_pricing_window(now, TZ));
        assert_eq!(
            InventoryService::effective_product_price(50.0, 40.0, now, TZ),
            50.0
        );
    }

    #[test]
    fn night_price_just_before_08_00() {
        let now = Utc.with_ymd_and_hms(2026, 6, 6, 2, 29, 0).unwrap();
        assert!(InventoryService::is_night_pricing_window(now, TZ));
        assert_eq!(
            InventoryService::effective_product_price(50.0, 40.0, now, TZ),
            40.0
        );
    }
}
