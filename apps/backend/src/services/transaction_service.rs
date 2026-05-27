use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{AssignPlanDto, CreateTransactionDto, Transaction, TransactionFilterDto, UpdateTransactionDto};
use crate::repositories::TransactionRepository;
use crate::services::{EventService, PlayerPlanService};

pub struct TransactionService {
    repo: TransactionRepository,
    player_plans: Arc<PlayerPlanService>,
    events: EventService,
}

impl TransactionService {
    pub fn new(pool: PgPool, player_plans: Arc<PlayerPlanService>, events: EventService) -> Self {
        Self {
            repo: TransactionRepository::new(pool),
            player_plans,
            events,
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

    pub async fn create(&self, dto: CreateTransactionDto) -> Result<Transaction, AppError> {
        if dto.transaction_type == "plan_purchase" && dto.plan_id.is_none() {
            return Err(AppError::BadRequest(
                "planId is required for plan_purchase transactions".to_string(),
            ));
        }

        let amount = match dto.amount {
            Some(amount) => amount,
            None if dto.transaction_type == "plan_purchase" => {
                let plan_id = dto.plan_id.ok_or_else(|| {
                    AppError::BadRequest("planId is required for plan_purchase transactions".to_string())
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
            .create(&dto, amount, transaction_date, &payment_status)
            .await?;

        if dto.transaction_type == "plan_purchase" && payment_status == "completed" {
            if let Some(plan_id) = dto.plan_id {
                self.player_plans
                    .assign_plan_to_player(AssignPlanDto {
                        player_id: dto.player_id,
                        plan_id,
                        transaction_id: Some(transaction.id),
                        purchase_date: Some(transaction.transaction_date),
                    })
                    .await?;
            }
        }

        self.events
            .publish_transaction_created(&transaction.id.to_string());

        Ok(transaction)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateTransactionDto,
    ) -> Result<Transaction, AppError> {
        if dto.payment_status.is_none() && dto.notes.is_none() {
            return Err(AppError::BadRequest(
                "At least one of paymentStatus or notes must be provided".to_string(),
            ));
        }

        self.repo.update(id, &dto).await
    }
}
