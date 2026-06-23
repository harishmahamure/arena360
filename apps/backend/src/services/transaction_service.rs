use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, CacheService};
use crate::error::AppError;
use crate::models::{
    CreateLineItemDto, CreateTransactionDto, PurchaseBalanceDto, Transaction, TransactionFilterDto,
    TransactionResponse, TransactionWithLineItems, UpdateTransactionDto,
};
use crate::realtime::{publish_balance_updated_for_player, OutboxService};
use crate::repositories::{InventoryRepository, TransactionProductRepository, TransactionRepository};
use crate::services::{BalanceService, CreditService, EventService, InventoryService, NotificationService};
use crate::validation::{
    optional_payment_status, require_payment_method, require_payment_status,
    require_transaction_type,
};

pub struct TransactionService {
    repo: TransactionRepository,
    line_item_repo: TransactionProductRepository,
    inventory_repo: InventoryRepository,
    balances: Arc<BalanceService>,
    credit: Arc<CreditService>,
    events: EventService,
    outbox: OutboxService,
    notifications: NotificationService,
    cafe_timezone: String,
    cache: Arc<dyn CacheService>,
}

impl TransactionService {
    pub fn new(
        pool: PgPool,
        balances: Arc<BalanceService>,
        credit: Arc<CreditService>,
        events: EventService,
        outbox: OutboxService,
        notifications: NotificationService,
        cafe_timezone: String,
        cache: Arc<dyn CacheService>,
    ) -> Self {
        Self {
            line_item_repo: TransactionProductRepository::new(pool.clone()),
            inventory_repo: InventoryRepository::new(pool.clone()),
            repo: TransactionRepository::new(pool),
            balances,
            credit,
            events,
            outbox,
            notifications,
            cafe_timezone,
            cache,
        }
    }

    async fn invalidate_stats_cache(&self) {
        let _ = cache::invalidate_stats(&*self.cache).await;
    }

    pub async fn list(
        &self,
        filters: TransactionFilterDto,
    ) -> Result<crate::dto::PaginationResult<TransactionResponse>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Transaction, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Transaction with ID {id} not found")))
    }

    pub async fn get_by_id_with_items(
        &self,
        id: Uuid,
    ) -> Result<TransactionWithLineItems, AppError> {
        let transaction = self.get_by_id(id).await?;
        let line_items = self.line_item_repo.list_by_transaction(id).await?;
        Ok(TransactionWithLineItems::from_parts(
            transaction,
            line_items,
        ))
    }

    fn compute_cash_portion(payment_method: &str, cash_amount: Option<f64>, amount: f64) -> f64 {
        match payment_method {
            "cash" => cash_amount.unwrap_or(amount),
            "split_payment" => cash_amount.unwrap_or(0.0),
            _ => 0.0,
        }
    }

    /// Derive `cashAmount` / `onlineAmount` columns from payment method when the client omits them.
    fn populate_payment_amounts(dto: &mut CreateTransactionDto, amount: f64) {
        match dto.payment_method.as_str() {
            "cash" => {
                dto.cash_amount = Some(dto.cash_amount.unwrap_or(amount));
                dto.online_amount = Some(dto.online_amount.unwrap_or(0.0));
            }
            "online" => {
                dto.cash_amount = Some(dto.cash_amount.unwrap_or(0.0));
                dto.online_amount = Some(dto.online_amount.unwrap_or(amount));
            }
            "split_payment" => {
                dto.cash_amount = Some(dto.cash_amount.unwrap_or(0.0));
                dto.online_amount = Some(dto.online_amount.unwrap_or(0.0));
            }
            _ => {
                dto.cash_amount = Some(0.0);
                dto.online_amount = Some(0.0);
            }
        }
    }

    async fn apply_cash_register_effects(
        &self,
        transaction: &Transaction,
        actor_id: Option<Uuid>,
        cash_registers: &crate::services::CashRegisterService,
    ) -> Result<(), AppError> {
        let cash_portion = Self::compute_cash_portion(
            &transaction.payment_method,
            transaction.cash_amount,
            transaction.amount,
        );
        if transaction.payment_status == "completed" && cash_portion > 0.0 {
            if let (Some(shift_id), Some(actor_id)) = (transaction.shift_id, actor_id) {
                let register = cash_registers.get_by_shift(shift_id).await?;
                cash_registers
                    .add_entry(
                        register.register.id,
                        crate::models::CreateCashRegisterEntryDto {
                            entry_type: "cash_in".to_string(),
                            amount: cash_portion,
                            reason: Some(format!("Transaction {}", transaction.id)),
                            reference_type: Some("transaction".to_string()),
                            reference_id: Some(transaction.id),
                        },
                        actor_id,
                    )
                    .await?;
            }
        }
        Ok(())
    }

    async fn prepare_credit_purchase(
        &self,
        player_id: Uuid,
        amount: f64,
    ) -> Result<String, AppError> {
        self.credit.validate_eligibility(player_id, amount).await?;
        Ok("credit".to_string())
    }

    fn should_grant_plan_benefit(payment_method: &str, payment_status: &str) -> bool {
        payment_status == "completed" || payment_method == "credit"
    }

    async fn create_product_purchase(
        &self,
        dto: CreateTransactionDto,
        actor_id: Option<Uuid>,
        actor_role: Option<&str>,
        cash_registers: &crate::services::CashRegisterService,
    ) -> Result<Transaction, AppError> {
        let line_items = dto.line_items.as_ref().ok_or_else(|| {
            AppError::BadRequest(
                "lineItems is required for product_purchase transactions".to_string(),
            )
        })?;
        if line_items.is_empty() {
            return Err(AppError::BadRequest(
                "lineItems must not be empty for product_purchase transactions".to_string(),
            ));
        }

        let sale_location_id = match dto.sale_location_id {
            Some(id) => id,
            None => self
                .inventory_repo
                .get_config_location_id("pos.default_sale_location_id")
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest(
                        "saleLocationId is required for product_purchase transactions".to_string(),
                    )
                })?,
        };

        let location = self
            .inventory_repo
            .find_location_by_id(sale_location_id)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("Sale location {sale_location_id} not found"))
            })?;

        if location.kind != "store" {
            return Err(AppError::BadRequest(
                "saleLocationId must reference a store location".to_string(),
            ));
        }

        let mut db_tx = self.repo.pool.begin().await?;
        let now = dto.transaction_date.unwrap_or_else(Utc::now);

        let resolved_items = Self::resolve_and_validate_stock(
            &mut db_tx,
            line_items,
            sale_location_id,
            &self.cafe_timezone,
            now,
        )
        .await?;

        let server_total: f64 = resolved_items
            .iter()
            .map(|(_, qty, price)| *qty as f64 * price)
            .sum();

        let is_credit = dto.payment_method == "credit";
        let payment_status = if is_credit {
            self.prepare_credit_purchase(dto.player_id, server_total)
                .await?
        } else {
            dto.payment_status
                .as_deref()
                .unwrap_or("pending")
                .to_string()
        };
        let transaction_date = dto.transaction_date.unwrap_or_else(Utc::now);

        let mut dto_for_insert = dto;
        dto_for_insert.amount = Some(server_total);
        Self::populate_payment_amounts(&mut dto_for_insert, server_total);

        let transaction = TransactionRepository::create_in_tx(
            &mut db_tx,
            &dto_for_insert,
            server_total,
            transaction_date,
            &payment_status,
            actor_id,
        )
        .await?;

        let items_for_insert: Vec<CreateLineItemDto> = resolved_items
            .iter()
            .map(|(product_id, qty, price)| CreateLineItemDto {
                product_id: *product_id,
                quantity: *qty,
                unit_price: Some(*price),
            })
            .collect();

        TransactionProductRepository::insert_many(
            &mut db_tx,
            transaction.id,
            &items_for_insert,
            actor_id,
        )
        .await?;

        for (product_id, qty, _) in &resolved_items {
            InventoryRepository::deduct_sale_stock_in_tx(
                &mut db_tx,
                sale_location_id,
                *product_id,
                *qty,
                transaction.id,
                actor_id,
            )
            .await?;
        }

        db_tx.commit().await?;

        let _ = self
            .cache
            .invalidate_prefix(&format!("stock:level:{sale_location_id}:"))
            .await;

        if is_credit {
            let _ = self
                .credit
                .invalidate_player_credit(transaction.player_id)
                .await;
        }

        self.apply_cash_register_effects(&transaction, actor_id, cash_registers)
            .await?;

        self.events
            .publish_transaction_created(&transaction.id.to_string());

        // Notify admin channel when a staff member processes a sale
        if actor_role == Some("staff") {
            let payload = serde_json::json!({
                "transaction_id": transaction.id.to_string(),
                "amount": transaction.amount,
                "payment_method": transaction.payment_method,
                "transaction_type": transaction.transaction_type,
            });
            let _ = self
                .outbox
                .publish(
                    "admin",
                    "transaction.sale_completed",
                    payload.clone(),
                    Some("admin"),
                    None,
                    true,
                )
                .await;
            if let Some(actor) = actor_id {
                let _ = self
                    .notifications
                    .record_staff_sale(&transaction, actor)
                    .await;
            }
        }

        self.invalidate_stats_cache().await;
        Ok(transaction)
    }

    async fn resolve_and_validate_stock(
        db_tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        line_items: &[CreateLineItemDto],
        sale_location_id: Uuid,
        cafe_timezone: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<Vec<(Uuid, i32, f64)>, AppError> {
        let mut resolved = Vec::with_capacity(line_items.len());
        for item in line_items {
            if item.quantity <= 0 {
                return Err(AppError::BadRequest(format!(
                    "Quantity must be positive for product {}",
                    item.product_id
                )));
            }
            let row: Option<(i32, f64, f64)> = sqlx::query_as(
                r#"
                SELECT COALESCE(ls."quantityPieces", 0),
                       p."dayPrice"::float8,
                       p."nightPrice"::float8
                FROM products p
                LEFT JOIN location_stock ls
                  ON ls."productId" = p.id AND ls."locationId" = $2
                WHERE p.id = $1 AND p."deletedAt" IS NULL
                FOR UPDATE OF p
                "#,
            )
            .bind(item.product_id)
            .bind(sale_location_id)
            .fetch_optional(&mut **db_tx)
            .await?;

            let (stock, day_price, night_price) = row.ok_or_else(|| {
                AppError::NotFound(format!("Product {} not found", item.product_id))
            })?;

            if stock < item.quantity {
                return Err(AppError::Conflict(format!(
                    "Insufficient stock for product {} (available: {}, requested: {})",
                    item.product_id, stock, item.quantity
                )));
            }

            let unit_price = InventoryService::effective_product_price(
                day_price,
                night_price,
                now,
                cafe_timezone,
            );
            resolved.push((item.product_id, item.quantity, unit_price));
        }
        Ok(resolved)
    }

    pub async fn create(
        &self,
        mut dto: CreateTransactionDto,
        actor_id: Option<Uuid>,
        actor_role: Option<&str>,
        cash_registers: &crate::services::CashRegisterService,
    ) -> Result<Transaction, AppError> {
        sanitize_create_transaction(&mut dto)?;
        if dto.transaction_type == "product_purchase" {
            return self
                .create_product_purchase(dto, actor_id, actor_role, cash_registers)
                .await;
        }

        if dto.transaction_type == "plan_purchase" && dto.plan_id.is_none() {
            return Err(AppError::BadRequest(
                "planId is required for plan_purchase transactions".to_string(),
            ));
        }

        let amount = match dto.amount {
            Some(amount) => amount,
            None if dto.transaction_type == "plan_purchase" => {
                let plan_id = dto.plan_id.ok_or_else(|| {
                    AppError::BadRequest(
                        "planId is required for plan_purchase transactions".to_string(),
                    )
                })?;
                self.repo.plan_price(plan_id).await?.ok_or_else(|| {
                    AppError::NotFound(format!("Plan with ID {plan_id} not found"))
                })?
            }
            None => {
                return Err(AppError::BadRequest(
                    "amount is required for this transaction type".to_string(),
                ));
            }
        };

        if amount < 0.0 {
            return Err(AppError::BadRequest(
                "amount must be greater than or equal to 0".to_string(),
            ));
        }

        let is_credit = dto.payment_method == "credit";
        let payment_status = if is_credit {
            self.prepare_credit_purchase(dto.player_id, amount).await?
        } else {
            dto.payment_status
                .as_deref()
                .unwrap_or("pending")
                .to_string()
        };
        let transaction_date = dto.transaction_date.unwrap_or_else(Utc::now);

        Self::populate_payment_amounts(&mut dto, amount);

        let transaction = self
            .repo
            .create(&dto, amount, transaction_date, &payment_status, actor_id)
            .await?;

        if dto.transaction_type == "plan_purchase"
            && Self::should_grant_plan_benefit(&dto.payment_method, &payment_status)
        {
            if let Some(plan_id) = dto.plan_id {
                let balance = self
                    .balances
                    .purchase_or_recharge(
                        PurchaseBalanceDto {
                            player_id: dto.player_id,
                            plan_id,
                            transaction_id: Some(transaction.id),
                        },
                        actor_id,
                    )
                    .await?;

                publish_balance_updated_for_player(
                    &self.repo.pool,
                    &self.outbox,
                    dto.player_id,
                    &balance,
                )
                .await;
            }
        }

        if is_credit {
            let _ = self
                .credit
                .invalidate_player_credit(dto.player_id)
                .await;
        }

        self.apply_cash_register_effects(&transaction, actor_id, cash_registers)
            .await?;

        self.events
            .publish_transaction_created(&transaction.id.to_string());

        if actor_role == Some("staff") {
            let payload = serde_json::json!({
                "transaction_id": transaction.id.to_string(),
                "amount": transaction.amount,
                "payment_method": transaction.payment_method,
                "transaction_type": transaction.transaction_type,
            });
            let _ = self
                .outbox
                .publish(
                    "admin",
                    "transaction.sale_completed",
                    payload.clone(),
                    Some("admin"),
                    None,
                    true,
                )
                .await;
            if let Some(actor) = actor_id {
                let _ = self
                    .notifications
                    .record_staff_sale(&transaction, actor)
                    .await;
            }
        }

        self.invalidate_stats_cache().await;
        Ok(transaction)
    }

    pub async fn update(
        &self,
        id: Uuid,
        mut dto: UpdateTransactionDto,
        actor_id: Option<Uuid>,
        cash_registers: &crate::services::CashRegisterService,
    ) -> Result<Transaction, AppError> {
        if let Some(status) = dto.payment_status.take() {
            dto.payment_status = Some(require_payment_status(Some(status))?);
        }
        if dto.payment_status.is_none() && dto.notes.is_none() {
            return Err(AppError::BadRequest(
                "At least one of paymentStatus or notes must be provided".to_string(),
            ));
        }

        let old_txn = self.get_by_id(id).await?;
        let updated = self.repo.update(id, &dto, actor_id).await?;

        if old_txn.payment_status != "completed" && updated.payment_status == "completed" {
            let plan_already_granted = Self::should_grant_plan_benefit(
                &old_txn.payment_method,
                &old_txn.payment_status,
            );
            if updated.transaction_type == "plan_purchase"
                && !plan_already_granted
                && Self::should_grant_plan_benefit(&updated.payment_method, &updated.payment_status)
            {
                if let Some(plan_id) = updated.plan_id {
                    let balance = self
                        .balances
                        .purchase_or_recharge(
                            PurchaseBalanceDto {
                                player_id: updated.player_id,
                                plan_id,
                                transaction_id: Some(updated.id),
                            },
                            actor_id,
                        )
                        .await?;

                    publish_balance_updated_for_player(
                        &self.repo.pool,
                        &self.outbox,
                        updated.player_id,
                        &balance,
                    )
                    .await;
                }
            }

            if updated.payment_method == "credit" {
                let _ = self
                    .credit
                    .invalidate_player_credit(updated.player_id)
                    .await;
            }

            self.apply_cash_register_effects(&updated, actor_id, cash_registers)
                .await?;
        }

        self.invalidate_stats_cache().await;
        Ok(updated)
    }
}

fn sanitize_create_transaction(dto: &mut CreateTransactionDto) -> Result<(), AppError> {
    dto.transaction_type = require_transaction_type(Some(dto.transaction_type.clone()))?;
    dto.payment_method = require_payment_method(Some(dto.payment_method.clone()))?;
    dto.payment_status = optional_payment_status(dto.payment_status.clone())?;
    Ok(())
}
