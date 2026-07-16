use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    activity_kind, ConvertKioskOrderDto, CreateKioskOrderDto, CreateLineItemDto,
    CreateTransactionDto, KioskMenuProduct, KioskOrderFilterDto, KioskOrderWithItems,
    kiosk_order_status,
};
use crate::realtime::OutboxService;
use crate::repositories::{InventoryRepository, KioskOrderRepository, SessionRepository};
use crate::services::{
    InventoryService, NotificationService, Recipients, RecordNotification, TransactionService,
};

pub struct KioskOrderService {
    repo: KioskOrderRepository,
    sessions: SessionRepository,
    inventory_repo: InventoryRepository,
    notifications: NotificationService,
    outbox: OutboxService,
    cafe_timezone: String,
}

impl KioskOrderService {
    pub fn new(
        pool: sqlx::PgPool,
        notifications: NotificationService,
        outbox: OutboxService,
        cafe_timezone: String,
    ) -> Self {
        Self {
            repo: KioskOrderRepository::new(pool.clone()),
            sessions: SessionRepository::new(pool.clone()),
            inventory_repo: InventoryRepository::new(pool),
            notifications,
            outbox,
            cafe_timezone,
        }
    }

    pub async fn list_menu(&self) -> Result<Vec<KioskMenuProduct>, AppError> {
        let sale_location_id = self
            .inventory_repo
            .get_config_location_id("pos.default_sale_location_id")
            .await?;

        let now = Utc::now();
        let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, String, f64, f64, i32)>(
            r#"
            SELECT p.id,
                   p.name,
                   p.description,
                   p.category::text,
                   p."dayPrice"::float8,
                   p."nightPrice"::float8,
                   COALESCE(ls."quantityPieces", p."stockQuantity", 0) as stock
            FROM products p
            LEFT JOIN location_stock ls
              ON ls."productId" = p.id
             AND ls."locationId" = $1
            WHERE p."deletedAt" IS NULL
              AND p."isActive" = true
            ORDER BY p.category, p.name
            "#,
        )
        .bind(sale_location_id)
        .fetch_all(self.repo.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, name, description, category, day_price, night_price, stock)| {
                    let price = InventoryService::effective_product_price(
                        day_price,
                        night_price,
                        now,
                        &self.cafe_timezone,
                    );
                    KioskMenuProduct {
                        id,
                        name,
                        description,
                        category,
                        price,
                        stock_available: stock,
                        in_stock: stock > 0,
                    }
                },
            )
            .collect())
    }

    pub async fn place_order(
        &self,
        player_id: Uuid,
        device_id: Uuid,
        dto: CreateKioskOrderDto,
    ) -> Result<KioskOrderWithItems, AppError> {
        if dto.line_items.is_empty() {
            return Err(AppError::BadRequest(
                "lineItems must not be empty".to_string(),
            ));
        }

        let open = self
            .sessions
            .find_open_session_for_player(player_id)
            .await?
            .ok_or_else(|| AppError::not_found_code("KIOSK_NO_ACTIVE_SESSION"))?;

        if open.device_id != device_id {
            return Err(AppError::not_found_code("KIOSK_NO_ACTIVE_SESSION"));
        }

        let sale_location_id = self
            .inventory_repo
            .get_config_location_id("pos.default_sale_location_id")
            .await?;

        let now = Utc::now();
        let mut resolved: Vec<(Uuid, i32, String, f64)> = Vec::new();

        for item in &dto.line_items {
            if item.quantity <= 0 {
                return Err(AppError::BadRequest(format!(
                    "Quantity must be positive for product {}",
                    item.product_id
                )));
            }

            let row: Option<(String, f64, f64, bool)> = sqlx::query_as(
                r#"
                SELECT p.name,
                       p."dayPrice"::float8,
                       p."nightPrice"::float8,
                       p."isActive"
                FROM products p
                WHERE p.id = $1 AND p."deletedAt" IS NULL
                "#,
            )
            .bind(item.product_id)
            .fetch_optional(self.repo.pool())
            .await?;

            let (name, day_price, night_price, is_active) = row.ok_or_else(|| {
                AppError::NotFound(format!("Product {} not found", item.product_id))
            })?;

            if !is_active {
                return Err(AppError::BadRequest(format!(
                    "Product {} is not available",
                    item.product_id
                )));
            }

            let unit_price = InventoryService::effective_product_price(
                day_price,
                night_price,
                now,
                &self.cafe_timezone,
            );

            resolved.push((item.product_id, item.quantity, name, unit_price));
        }

        let _ = sale_location_id;

        let order = self
            .repo
            .create_with_items(
                open.session_id,
                player_id,
                device_id,
                dto.note,
                &resolved,
            )
            .await?;

        self.notify_order_placed(&order).await?;

        Ok(order)
    }

    async fn notify_order_placed(&self, order: &KioskOrderWithItems) -> Result<(), AppError> {
        let device_name = order.device_name.clone().unwrap_or_else(|| "Unknown PC".to_string());
        let username = order
            .player_username
            .clone()
            .unwrap_or_else(|| "player".to_string());

        let items_summary: Vec<_> = order
            .line_items
            .iter()
            .map(|i| {
                json!({
                    "productName": i.product_name,
                    "quantity": i.quantity,
                })
            })
            .collect();

        let payload = json!({
            "orderId": order.id.to_string(),
            "sessionId": order.session_id.to_string(),
            "deviceId": order.device_id.to_string(),
            "deviceName": device_name,
            "playerUsername": username,
            "playerId": order.player_id.to_string(),
            "items": items_summary,
        });

        let _ = self
            .outbox
            .publish(
                "staff",
                "kiosk_order.placed",
                payload.clone(),
                Some("staff"),
                None,
                false,
            )
            .await;

        let title = format!("Order from {device_name} — {username}");
        let summary = order
            .line_items
            .iter()
            .map(|i| format!("{}× {}", i.quantity, i.product_name))
            .collect::<Vec<_>>()
            .join(", ");

        let _ = self
            .notifications
            .record(RecordNotification {
                kind: activity_kind::KIOSK_ORDER_PLACED.to_string(),
                title,
                summary: Some(summary),
                payload,
                actor_user_id: None,
                entity_type: Some("kiosk_order".to_string()),
                entity_id: Some(order.id),
                recipients: Recipients::AllStaff,
            })
            .await;

        Ok(())
    }

    pub async fn current_order_for_player(
        &self,
        player_id: Uuid,
        device_id: Uuid,
    ) -> Result<Option<KioskOrderWithItems>, AppError> {
        let open = self
            .sessions
            .find_open_session_for_player(player_id)
            .await?;

        let Some(open) = open else {
            return Ok(None);
        };

        if open.device_id != device_id {
            return Ok(None);
        }

        let order = self.repo.find_open_for_session(open.session_id).await?;
        match order {
            Some(o) => Ok(Some(self.repo.get_with_details(o.id).await?)),
            None => Ok(None),
        }
    }

    pub async fn list(&self, filters: KioskOrderFilterDto) -> Result<crate::dto::PaginationResult<KioskOrderWithItems>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<KioskOrderWithItems, AppError> {
        self.repo.get_with_details(id).await
    }

    pub async fn update_status(
        &self,
        id: Uuid,
        status: &str,
    ) -> Result<KioskOrderWithItems, AppError> {
        if status == kiosk_order_status::CANCELLED {
            let order = self.repo.update_status(id, status).await?;
            let details = self.repo.get_with_details(order.id).await?;
            let _ = self
                .notifications
                .record(RecordNotification {
                    kind: activity_kind::KIOSK_ORDER_CANCELLED.to_string(),
                    title: format!(
                        "Order cancelled — {}",
                        details.device_name.as_deref().unwrap_or("PC")
                    ),
                    summary: None,
                    payload: json!({ "orderId": id.to_string() }),
                    actor_user_id: None,
                    entity_type: Some("kiosk_order".to_string()),
                    entity_id: Some(id),
                    recipients: Recipients::AllStaff,
                })
                .await;
            return Ok(details);
        }

        self.repo.update_status(id, status).await?;
        self.repo.get_with_details(id).await
    }

    pub async fn mark_fulfilled(
        &self,
        order_id: Uuid,
        transaction_id: Uuid,
    ) -> Result<(), AppError> {
        let order = self.repo.mark_fulfilled(order_id, transaction_id).await?;
        let details = self.repo.get_with_details(order.id).await?;
        let _ = self
            .notifications
            .record(RecordNotification {
                kind: activity_kind::KIOSK_ORDER_FULFILLED.to_string(),
                title: format!(
                    "Order fulfilled — {}",
                    details.device_name.as_deref().unwrap_or("PC")
                ),
                summary: Some(format!("Sale recorded · tx {}", transaction_id)),
                payload: json!({
                    "orderId": order_id.to_string(),
                    "transactionId": transaction_id.to_string(),
                }),
                actor_user_id: None,
                entity_type: Some("kiosk_order".to_string()),
                entity_id: Some(order_id),
                recipients: Recipients::AllStaff,
            })
            .await;
        Ok(())
    }

    pub fn build_transaction_dto(
        order: &KioskOrderWithItems,
        convert: &ConvertKioskOrderDto,
    ) -> CreateTransactionDto {
        CreateTransactionDto {
            player_id: order.player_id,
            transaction_type: "product_purchase".to_string(),
            plan_id: None,
            shift_id: None,
            amount: None,
            payment_method: convert.payment_method.clone(),
            payment_status: convert.payment_status.clone(),
            notes: convert.notes.clone(),
            online_payment_ref_last4: convert.online_payment_ref_last4.clone(),
            transaction_date: None,
            cash_amount: convert.cash_amount,
            online_amount: convert.online_amount,
            line_items: Some(
                order
                    .line_items
                    .iter()
                    .map(|i| CreateLineItemDto {
                        product_id: i.product_id,
                        quantity: i.quantity,
                        unit_price: Some(i.unit_price),
                    })
                    .collect(),
            ),
            sale_location_id: convert.sale_location_id,
            kiosk_order_id: Some(order.id),
        }
    }

    pub async fn convert_to_sale(
        &self,
        order_id: Uuid,
        convert: ConvertKioskOrderDto,
        shift_id: Uuid,
        actor_id: Uuid,
        actor_role: Option<&str>,
        transactions: &TransactionService,
        cash_registers: &Arc<crate::services::CashRegisterService>,
    ) -> Result<crate::models::Transaction, AppError> {
        let order = self.get_by_id(order_id).await?;
        if !kiosk_order_status::OPEN.contains(&order.status.as_str()) {
            return Err(AppError::Conflict(
                "Order is not open for conversion".to_string(),
            ));
        }

        let mut dto = Self::build_transaction_dto(&order, &convert);
        dto.shift_id = Some(shift_id);
        let tx = transactions
            .create(dto, Some(actor_id), actor_role, cash_registers)
            .await?;
        Ok(tx)
    }
}
