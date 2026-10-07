use crate::error::AppError;
use crate::models::{
    CreateTransactionDto, Transaction, TransactionFilterDto, TransactionResponse,
    TransactionWithLineItems, UpdateTransactionDto,
};
use crate::repositories::TenantTransactionRepository;
use crate::tenancy::TenantDb;
use crate::validation::{
    optional_payment_status, require_payment_method, require_payment_status,
    require_transaction_type,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
pub struct TransactionService;
impl TransactionService {
    pub fn new() -> Self {
        Self
    }
    pub async fn list_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: TransactionFilterDto,
    ) -> Result<crate::dto::PaginationResult<TransactionResponse>, AppError> {
        TenantTransactionRepository::new(db).list(&filters).await
    }

    pub async fn get_by_id_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
    ) -> Result<Transaction, AppError> {
        TenantTransactionRepository::new(db)
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Transaction with ID {id} not found")))
    }

    pub async fn get_by_id_with_items_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
    ) -> Result<TransactionWithLineItems, AppError> {
        TenantTransactionRepository::new(db)
            .get_with_items(id)
            .await
    }

    pub async fn create_tenant(
        &self,
        db: Arc<TenantDb>,
        mut dto: CreateTransactionDto,
        actor_id: Option<Uuid>,
    ) -> Result<Transaction, AppError> {
        sanitize_create_transaction(&mut dto)?;
        let timezone = db.timezone().await?;
        TenantTransactionRepository::new(db)
            .with_timezone(timezone)
            .create(dto, actor_id)
            .await
    }

    pub async fn update_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
        mut dto: UpdateTransactionDto,
        actor_id: Option<Uuid>,
    ) -> Result<Transaction, AppError> {
        if let Some(status) = dto.payment_status.take() {
            dto.payment_status = Some(require_payment_status(Some(status))?);
        }
        TenantTransactionRepository::new(db)
            .update(id, &dto, actor_id)
            .await
    }
}

fn sanitize_create_transaction(dto: &mut CreateTransactionDto) -> Result<(), AppError> {
    dto.transaction_type = require_transaction_type(Some(dto.transaction_type.clone()))?;
    dto.payment_method = require_payment_method(Some(dto.payment_method.clone()))?;
    dto.payment_status = optional_payment_status(dto.payment_status.clone())?;
    Ok(())
}
