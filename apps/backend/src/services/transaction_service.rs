use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    CreateLineItemDto, CreateTransactionDto, PurchaseBalanceDto, Transaction,
    TransactionFilterDto, TransactionWithLineItems, UpdateTransactionDto,
};
use crate::realtime::OutboxService;
use crate::repositories::{TransactionProductRepository, TransactionRepository};
use crate::services::{BalanceService, EventService};

pub struct TransactionService {
    repo: TransactionRepository,
    line_item_repo: TransactionProductRepository,
    balances: Arc<BalanceService>,
    events: EventService,
    outbox: OutboxService,
}

impl TransactionService {
    pub fn new(pool: PgPool, balances: Arc<BalanceService>, events: EventService, outbox: OutboxService) -> Self {
        Self {
            line_item_repo: TransactionProductRepository::new(pool.clone()),
            repo: TransactionRepository::new(pool),
            balances,
            events,
            outbox,
        }
    }

    pub async fn list(
        &self,
        filters: TransactionFilterDto,
    ) -> Result<crate::dto::PaginationResult<Transaction>, AppError> {
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

        let mut db_tx = self.repo.pool.begin().await?;

        let resolved_items = Self::resolve_and_validate_stock(&mut db_tx, line_items).await?;

        let server_total: f64 = resolved_items
            .iter()
            .map(|(_, qty, price)| *qty as f64 * price)
            .sum();

        let payment_status = dto
            .payment_status
            .as_deref()
            .unwrap_or("pending")
            .to_string();
        let transaction_date = dto.transaction_date.unwrap_or_else(Utc::now);

        let mut dto_for_insert = dto;
        dto_for_insert.amount = Some(server_total);

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
            let rows = sqlx::query(
                r#"UPDATE products SET "stockQuantity" = "stockQuantity" - $1, "updatedAt" = NOW()
                   WHERE id = $2 AND "stockQuantity" >= $1 AND "deletedAt" IS NULL
                   RETURNING id"#,
            )
            .bind(*qty)
            .bind(*product_id)
            .fetch_optional(&mut *db_tx)
            .await?;

            if rows.is_none() {
                return Err(AppError::Conflict(format!(
                    "Insufficient stock for product {product_id}"
                )));
            }
        }

        db_tx.commit().await?;

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
            let _ = self.outbox.publish(
                "admin",
                "transaction.sale_completed",
                payload,
                Some("admin"),
                None,
                true,
            ).await;
        }

        Ok(transaction)
    }

    async fn resolve_and_validate_stock(
        db_tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        line_items: &[CreateLineItemDto],
    ) -> Result<Vec<(Uuid, i32, f64)>, AppError> {
        let mut resolved = Vec::with_capacity(line_items.len());
        for item in line_items {
            if item.quantity <= 0 {
                return Err(AppError::BadRequest(format!(
                    "Quantity must be positive for product {}",
                    item.product_id
                )));
            }
            let row: Option<(i32, f64)> = sqlx::query_as(
                r#"SELECT "stockQuantity", price::float8
                   FROM products
                   WHERE id = $1 AND "deletedAt" IS NULL
                   FOR UPDATE"#,
            )
            .bind(item.product_id)
            .fetch_optional(&mut **db_tx)
            .await?;

            let (stock, db_price) = row.ok_or_else(|| {
                AppError::NotFound(format!("Product {} not found", item.product_id))
            })?;

            if stock < item.quantity {
                return Err(AppError::Conflict(format!(
                    "Insufficient stock for product {} (available: {}, requested: {})",
                    item.product_id, stock, item.quantity
                )));
            }

            let unit_price = item.unit_price.unwrap_or(db_price);
            resolved.push((item.product_id, item.quantity, unit_price));
        }
        Ok(resolved)
    }

    pub async fn create(
        &self,
        dto: CreateTransactionDto,
        actor_id: Option<Uuid>,
        actor_role: Option<&str>,
        cash_registers: &crate::services::CashRegisterService,
    ) -> Result<Transaction, AppError> {
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

        let payment_status = dto
            .payment_status
            .as_deref()
            .unwrap_or("pending")
            .to_string();
        let transaction_date = dto.transaction_date.unwrap_or_else(Utc::now);

        let transaction = self
            .repo
            .create(&dto, amount, transaction_date, &payment_status, actor_id)
            .await?;

        if dto.transaction_type == "plan_purchase" && payment_status == "completed" {
            if let Some(plan_id) = dto.plan_id {
                self.balances
                    .purchase_or_recharge(
                        PurchaseBalanceDto {
                            player_id: dto.player_id,
                            plan_id,
                            transaction_id: Some(transaction.id),
                        },
                        actor_id,
                    )
                    .await?;
            }
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
            let _ = self.outbox.publish(
                "admin",
                "transaction.sale_completed",
                payload,
                Some("admin"),
                None,
                true,
            ).await;
        }

        Ok(transaction)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateTransactionDto,
        actor_id: Option<Uuid>,
        cash_registers: &crate::services::CashRegisterService,
    ) -> Result<Transaction, AppError> {
        if dto.payment_status.is_none() && dto.notes.is_none() {
            return Err(AppError::BadRequest(
                "At least one of paymentStatus or notes must be provided".to_string(),
            ));
        }

        let old_txn = self.get_by_id(id).await?;
        let updated = self.repo.update(id, &dto, actor_id).await?;

        if old_txn.payment_status != "completed" && updated.payment_status == "completed" {
            let cash_portion = match updated.payment_method.as_str() {
                "cash" => updated.cash_amount.unwrap_or(updated.amount),
                "split_payment" => updated.cash_amount.unwrap_or(0.0),
                _ => 0.0,
            };
            if cash_portion > 0.0 {
                if let (Some(shift_id), Some(actor_id)) = (updated.shift_id, actor_id) {
                    let register = cash_registers.get_by_shift(shift_id).await?;
                    cash_registers
                        .add_entry(
                            register.register.id,
                            crate::models::CreateCashRegisterEntryDto {
                                entry_type: "cash_in".to_string(),
                                amount: cash_portion,
                                reason: Some(format!("Transaction {}", updated.id)),
                                reference_type: Some("transaction".to_string()),
                                reference_id: Some(updated.id),
                            },
                            actor_id,
                        )
                        .await?;
                }
            }
        }

        Ok(updated)
    }
}
