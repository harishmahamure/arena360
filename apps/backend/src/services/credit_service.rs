use crate::error::AppError;
use crate::models::{
    CreditAccountFilterDto, CreditPlayerRow, CreditPortfolioSummary, CreditSettlement,
    CreditSettlementDetail, CreditSettlementFilterDto, CreditSettlementListRow, CreditSummary,
    PlayerCreditDetail, SetCreditLimitDto, SettleCreditDto,
};
use crate::repositories::TenantCreditRepository;
use crate::tenancy::TenantDb;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
pub struct CreditService;
impl CreditService {
    pub fn new() -> Self {
        Self
    }
    pub async fn summary_tenant(
        &self,
        db: Arc<TenantDb>,
        player_id: Uuid,
    ) -> Result<CreditSummary, AppError> {
        TenantCreditRepository::new(db).summary(player_id).await
    }

    pub async fn get_player_credit_tenant(
        &self,
        db: Arc<TenantDb>,
        player_id: Uuid,
    ) -> Result<PlayerCreditDetail, AppError> {
        TenantCreditRepository::new(db)
            .player_detail(player_id)
            .await
    }

    pub async fn list_credit_players_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: CreditAccountFilterDto,
    ) -> Result<crate::dto::PaginationResult<CreditPlayerRow>, AppError> {
        TenantCreditRepository::new(db).list_players(&filters).await
    }

    pub async fn portfolio_summary(&self) -> Result<CreditPortfolioSummary, AppError> {
        crate::analytics::ClickHouse::from_env()
            .get_portfolio_summary()
            .await
    }

    pub async fn list_settlements_tenant(
        &self,
        db: Arc<TenantDb>,
        filters: CreditSettlementFilterDto,
    ) -> Result<crate::dto::PaginationResult<CreditSettlementListRow>, AppError> {
        TenantCreditRepository::new(db)
            .list_settlements(&filters)
            .await
    }

    pub async fn get_settlement_tenant(
        &self,
        db: Arc<TenantDb>,
        id: Uuid,
    ) -> Result<CreditSettlementDetail, AppError> {
        TenantCreditRepository::new(db).settlement_detail(id).await
    }

    pub async fn settle_tenant(
        &self,
        db: Arc<TenantDb>,
        dto: SettleCreditDto,
        shift_id: Uuid,
        actor_id: Uuid,
    ) -> Result<CreditSettlement, AppError> {
        TenantCreditRepository::new(db)
            .settle(dto, shift_id, actor_id)
            .await
    }

    pub async fn set_limit_tenant(
        &self,
        db: Arc<TenantDb>,
        player_id: Uuid,
        dto: SetCreditLimitDto,
        actor_id: Option<Uuid>,
    ) -> Result<CreditSummary, AppError> {
        TenantCreditRepository::new(db)
            .set_limit(player_id, &dto, actor_id)
            .await
    }
}
