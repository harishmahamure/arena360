use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    compute_available, validate_settlement_items, CreditAccountFilterDto, CreditPlayerRow,
    CreditSettlement, CreditSummary, PlayerCreditDetail, SetCreditLimitDto, SettleCreditDto,
};
use crate::repositories::CreditRepository;
use crate::services::CashRegisterService;

pub struct CreditService {
    repo: CreditRepository,
}

impl CreditService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: CreditRepository::new(pool),
        }
    }

    pub async fn validate_eligibility(
        &self,
        player_id: Uuid,
        amount: f64,
    ) -> Result<CreditSummary, AppError> {
        let (credit_enabled, credit_limit) = self.repo.is_eligible_player(player_id).await?;

        if !credit_enabled {
            return Err(AppError::Conflict(
                "credit not enabled for this player".to_string(),
            ));
        }

        let outstanding = self.repo.sum_outstanding(player_id).await?;
        let available = compute_available(credit_limit, outstanding);

        if amount > available + 0.001 {
            return Err(AppError::Conflict(format!(
                "credit limit exceeded: available {available:.2}"
            )));
        }

        Ok(CreditSummary {
            player_id,
            credit_limit,
            outstanding,
            available,
            credit_enabled: true,
        })
    }

    pub async fn summary(&self, player_id: Uuid) -> Result<CreditSummary, AppError> {
        let (credit_enabled, credit_limit) = self.repo.is_eligible_player(player_id).await?;
        let outstanding = self.repo.sum_outstanding(player_id).await?;
        let available = compute_available(credit_limit, outstanding);

        Ok(CreditSummary {
            player_id,
            credit_limit,
            outstanding,
            available,
            credit_enabled,
        })
    }

    pub async fn list_credit_players(
        &self,
        filters: CreditAccountFilterDto,
    ) -> Result<crate::dto::PaginationResult<CreditPlayerRow>, AppError> {
        self.repo.list_credit_players(&filters).await
    }

    pub async fn get_player_credit(&self, player_id: Uuid) -> Result<PlayerCreditDetail, AppError> {
        let summary = self.summary(player_id).await?;
        let transactions = self.repo.list_outstanding_txns(player_id).await?;
        Ok(PlayerCreditDetail {
            summary,
            transactions,
        })
    }

    pub async fn settle(
        &self,
        dto: SettleCreditDto,
        shift_id: Uuid,
        actor_id: Uuid,
        cash_registers: &CashRegisterService,
    ) -> Result<CreditSettlement, AppError> {
        let total = validate_settlement_items(
            &dto.items,
            &dto.payment_method,
            dto.cash_amount,
            dto.online_amount,
        )
        .map_err(AppError::BadRequest)?;

        let settlement = self
            .repo
            .create_settlement(
                dto.player_id,
                shift_id,
                actor_id,
                total,
                &dto.payment_method,
                dto.cash_amount,
                dto.online_amount,
                dto.notes.as_deref(),
                &dto.items,
            )
            .await?;

        let cash_portion = match dto.payment_method.as_str() {
            "cash" => dto.cash_amount.unwrap_or(total),
            "split_payment" => dto.cash_amount.unwrap_or(0.0),
            _ => 0.0,
        };

        if cash_portion > 0.0 {
            let register = cash_registers.get_by_shift(shift_id).await?;
            cash_registers
                .add_entry(
                    register.register.id,
                    crate::models::CreateCashRegisterEntryDto {
                        entry_type: "cash_in".to_string(),
                        amount: cash_portion,
                        reason: Some(format!("Credit settlement {}", settlement.id)),
                        reference_type: Some("credit_settlement".to_string()),
                        reference_id: Some(settlement.id),
                    },
                    actor_id,
                )
                .await?;
        }

        Ok(settlement)
    }

    pub async fn set_limit(
        &self,
        player_id: Uuid,
        dto: SetCreditLimitDto,
        actor_id: Option<Uuid>,
    ) -> Result<CreditSummary, AppError> {
        if dto.credit_limit < 0.0 {
            return Err(AppError::BadRequest(
                "creditLimit must be greater than or equal to 0".to_string(),
            ));
        }

        self.repo
            .set_credit_limit(player_id, dto.credit_limit, actor_id)
            .await?;

        self.summary(player_id).await
    }
}
