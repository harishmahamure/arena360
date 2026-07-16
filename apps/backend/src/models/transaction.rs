use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::transaction_product::TransactionProductResponse;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: Uuid,
    pub player_id: Uuid,
    pub transaction_type: String,
    pub plan_id: Option<Uuid>,
    pub shift_id: Option<Uuid>,
    pub amount: f64,
    pub paid_amount: f64,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
    pub payment_method: String,
    pub payment_status: String,
    pub notes: Option<String>,
    pub online_payment_ref_last4: Option<String>,
    pub transaction_date: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransactionPlayerSummary {
    pub id: Uuid,
    pub username: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransactionPlanSummary {
    pub id: Uuid,
    pub name: String,
    pub plan_type: String,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResponse {
    pub id: Uuid,
    pub player_id: Uuid,
    pub transaction_type: String,
    pub plan_id: Option<Uuid>,
    pub shift_id: Option<Uuid>,
    pub amount: f64,
    pub paid_amount: f64,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
    pub payment_method: String,
    pub payment_status: String,
    pub notes: Option<String>,
    pub online_payment_ref_last4: Option<String>,
    pub transaction_date: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player: Option<TransactionPlayerSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<TransactionPlanSummary>,
}

#[derive(Debug, FromRow)]
pub struct TransactionRow {
    pub id: Uuid,
    pub player_id: Uuid,
    pub transaction_type: String,
    pub plan_id: Option<Uuid>,
    pub shift_id: Option<Uuid>,
    pub amount: f64,
    pub paid_amount: f64,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
    pub payment_method: String,
    pub payment_status: String,
    pub notes: Option<String>,
    pub online_payment_ref_last4: Option<String>,
    pub transaction_date: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub player_username: Option<String>,
    pub player_first_name: Option<String>,
    pub player_last_name: Option<String>,
    pub plan_name: Option<String>,
    pub plan_type: Option<String>,
    pub plan_price: Option<f64>,
}

impl TransactionRow {
    pub fn into_response(self) -> TransactionResponse {
        let player = self.player_username.map(|username| {
            let full_name = format!(
                "{} {}",
                self.player_first_name.clone().unwrap_or_default(),
                self.player_last_name.clone().unwrap_or_default()
            )
            .trim()
            .to_string();
            let name = if full_name.is_empty() {
                username.clone()
            } else {
                full_name
            };
            TransactionPlayerSummary {
                id: self.player_id,
                username,
                name,
            }
        });

        let plan = self.plan_id.and_then(|plan_id| {
            self.plan_name.map(|name| TransactionPlanSummary {
                id: plan_id,
                name,
                plan_type: self.plan_type.clone().unwrap_or_default(),
                price: self.plan_price.unwrap_or(0.0),
            })
        });

        TransactionResponse {
            id: self.id,
            player_id: self.player_id,
            transaction_type: self.transaction_type,
            plan_id: self.plan_id,
            shift_id: self.shift_id,
            amount: self.amount,
            paid_amount: self.paid_amount,
            cash_amount: self.cash_amount,
            online_amount: self.online_amount,
            payment_method: self.payment_method,
            payment_status: self.payment_status,
            notes: self.notes,
            online_payment_ref_last4: self.online_payment_ref_last4,
            transaction_date: self.transaction_date,
            created_by: self.created_by,
            updated_by: self.updated_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            player,
            plan,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransactionWithLineItems {
    pub id: Uuid,
    pub player_id: Uuid,
    pub transaction_type: String,
    pub plan_id: Option<Uuid>,
    pub shift_id: Option<Uuid>,
    pub amount: f64,
    pub paid_amount: f64,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
    pub payment_method: String,
    pub payment_status: String,
    pub notes: Option<String>,
    pub online_payment_ref_last4: Option<String>,
    pub transaction_date: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub line_items: Vec<TransactionProductResponse>,
}

impl TransactionWithLineItems {
    pub fn from_parts(t: Transaction, line_items: Vec<TransactionProductResponse>) -> Self {
        Self {
            id: t.id,
            player_id: t.player_id,
            transaction_type: t.transaction_type,
            plan_id: t.plan_id,
            shift_id: t.shift_id,
            amount: t.amount,
            paid_amount: t.paid_amount,
            cash_amount: t.cash_amount,
            online_amount: t.online_amount,
            payment_method: t.payment_method,
            payment_status: t.payment_status,
            notes: t.notes,
            online_payment_ref_last4: t.online_payment_ref_last4,
            transaction_date: t.transaction_date,
            created_by: t.created_by,
            updated_by: t.updated_by,
            created_at: t.created_at,
            updated_at: t.updated_at,
            deleted_at: t.deleted_at,
            line_items,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransactionDto {
    pub player_id: Uuid,
    pub transaction_type: String,
    pub plan_id: Option<Uuid>,
    pub shift_id: Option<Uuid>,
    pub amount: Option<f64>,
    pub payment_method: String,
    pub payment_status: Option<String>,
    pub notes: Option<String>,
    pub online_payment_ref_last4: Option<String>,
    pub transaction_date: Option<DateTime<Utc>>,
    pub cash_amount: Option<f64>,
    pub online_amount: Option<f64>,
    pub line_items: Option<Vec<super::transaction_product::CreateLineItemDto>>,
    pub sale_location_id: Option<Uuid>,
    pub kiosk_order_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTransactionDto {
    pub payment_status: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct TransactionFilterDto {
    pub player_id: Option<Uuid>,
    pub transaction_type: Option<String>,
    pub plan_id: Option<Uuid>,
    pub shift_id: Option<Uuid>,
    pub payment_method: Option<String>,
    pub payment_status: Option<String>,
    pub transaction_date_from: Option<DateTime<Utc>>,
    pub transaction_date_to: Option<DateTime<Utc>>,
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}
