use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreditSummary {
    pub player_id: Uuid,
    pub credit_limit: f64,
    pub outstanding: f64,
    pub available: f64,
    pub credit_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CreditPlayerRow {
    pub player_id: Uuid,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone_number: Option<String>,
    pub credit_limit: f64,
    pub outstanding: f64,
    pub available: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct OutstandingTxnRow {
    pub transaction_id: Uuid,
    pub transaction_type: String,
    pub amount: f64,
    pub paid_amount: f64,
    pub remaining: f64,
    pub payment_status: String,
    pub transaction_date: DateTime<Utc>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCreditDetail {
    pub summary: CreditSummary,
    pub transactions: Vec<OutstandingTxnRow>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreditSettlement {
    pub id: Uuid,
    pub player_id: Uuid,
    pub shift_id: Uuid,
    pub settled_by: Uuid,
    pub amount: f64,
    pub payment_method: String,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
    pub notes: Option<String>,
    pub settled_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettleItemDto {
    pub transaction_id: Uuid,
    pub amount: f64,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettleCreditDto {
    pub player_id: Uuid,
    pub items: Vec<SettleItemDto>,
    pub payment_method: String,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetCreditLimitDto {
    pub credit_limit: f64,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CreditAccountFilterDto {
    pub search: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CreditSettlementFilterDto {
    pub search: Option<String>,
    pub player_id: Option<Uuid>,
    pub payment_method: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CreditSettlementListRow {
    pub id: Uuid,
    pub player_id: Uuid,
    pub player_username: String,
    pub shift_id: Uuid,
    pub settled_by: Uuid,
    pub settled_by_username: String,
    pub amount: f64,
    pub payment_method: String,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
    pub notes: Option<String>,
    pub settled_at: DateTime<Utc>,
    pub item_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CreditSettlementItemRow {
    pub transaction_id: Uuid,
    pub transaction_type: String,
    pub transaction_date: DateTime<Utc>,
    pub original_amount: f64,
    pub amount_applied: f64,
    pub remaining_after: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreditSettlementDetail {
    pub id: Uuid,
    pub player_id: Uuid,
    pub player_username: String,
    pub shift_id: Uuid,
    pub settled_by: Uuid,
    pub settled_by_username: String,
    pub amount: f64,
    pub payment_method: String,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
    pub notes: Option<String>,
    pub settled_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub items: Vec<CreditSettlementItemRow>,
}

/// Pure helper: available credit headroom.
pub fn compute_available(credit_limit: f64, outstanding: f64) -> f64 {
    (credit_limit - outstanding).max(0.0)
}

/// Pure helper: validate settlement line items and payment split.
pub fn validate_settlement_items(
    items: &[SettleItemDto],
    payment_method: &str,
    cash_amount: Option<f64>,
    online_amount: Option<f64>,
) -> Result<f64, String> {
    if items.is_empty() {
        return Err("At least one transaction must be selected".to_string());
    }

    let total: f64 = items.iter().map(|i| i.amount).sum();
    if total <= 0.0 {
        return Err("Settlement total must be greater than zero".to_string());
    }

    for item in items {
        if item.amount <= 0.0 {
            return Err("Each settlement amount must be greater than zero".to_string());
        }
    }

    let cash = match payment_method {
        "cash" => cash_amount.unwrap_or(total),
        "split_payment" => cash_amount.unwrap_or(0.0),
        "online" => 0.0,
        _ => 0.0,
    };
    let online = match payment_method {
        "online" => online_amount.unwrap_or(total),
        "split_payment" => online_amount.unwrap_or(0.0),
        _ => 0.0,
    };

    if payment_method == "split_payment" {
        let sum = cash + online;
        if (sum - total).abs() > 0.01 {
            return Err(format!(
                "Cash ({cash}) + online ({online}) must equal settlement total ({total})"
            ));
        }
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_available_never_negative() {
        assert_eq!(compute_available(100.0, 80.0), 20.0);
        assert_eq!(compute_available(100.0, 150.0), 0.0);
    }

    #[test]
    fn validate_settlement_items_rejects_empty() {
        let err = validate_settlement_items(&[], "cash", None, None).unwrap_err();
        assert!(err.contains("At least one"));
    }

    #[test]
    fn validate_settlement_split_must_balance() {
        let items = vec![SettleItemDto {
            transaction_id: Uuid::new_v4(),
            amount: 30.0,
        }];
        assert!(
            validate_settlement_items(&items, "split_payment", Some(10.0), Some(10.0)).is_err()
        );
        assert!(validate_settlement_items(&items, "split_payment", Some(20.0), Some(10.0)).is_ok());
    }
}
