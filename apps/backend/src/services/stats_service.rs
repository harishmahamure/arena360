use chrono::{DateTime, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use utoipa::ToSchema;

use crate::cache::{get_or_set, keys, CacheService};
use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RevenueByPaymentMethodDto {
    #[serde(default)]
    pub plan: f64,
    #[serde(default)]
    pub merchandise: f64,
    #[serde(default)]
    pub total: f64,
    #[serde(default)]
    pub cash_revenue: f64,
    #[serde(default)]
    pub online_revenue: f64,
    #[serde(default)]
    pub credit_revenue: f64,
    #[serde(default)]
    pub plan_transaction_count: i64,
    #[serde(default)]
    pub product_transaction_count: i64,
    #[serde(default)]
    pub plan_cash_revenue: f64,
    #[serde(default)]
    pub plan_online_revenue: f64,
    #[serde(default)]
    pub plan_credit_revenue: f64,
    #[serde(default)]
    pub product_cash_revenue: f64,
    #[serde(default)]
    pub product_online_revenue: f64,
    #[serde(default)]
    pub product_credit_revenue: f64,
    #[serde(default)]
    pub plan_cash_count: i64,
    #[serde(default)]
    pub plan_online_count: i64,
    #[serde(default)]
    pub plan_credit_count: i64,
    #[serde(default)]
    pub product_cash_count: i64,
    #[serde(default)]
    pub product_online_count: i64,
    #[serde(default)]
    pub product_credit_count: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransactionStatsDto {
    pub total_transactions: i64,
    pub completed_transactions: i64,
    pub pending_transactions: i64,
    pub failed_transactions: i64,
    pub average_transaction_amount: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatsDto {
    pub total_sessions: i64,
    pub active_sessions: i64,
    pub completed_sessions: i64,
    pub total_hours: f64,
    pub total_minutes: i64,
    pub average_session_duration: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserStatsDto {
    pub total_users: i64,
    pub active_users: i64,
    pub total_players: i64,
    pub active_players: i64,
    pub new_users_this_period: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanStatsDto {
    pub total_active_plans: i64,
    pub total_expired_plans: i64,
    pub plans_by_type: Vec<PlanTypeStat>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PlanTypeStat {
    #[serde(rename = "type")]
    pub plan_type: String,
    pub count: i64,
    pub revenue: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatsDto {
    pub total_devices: i64,
    pub active_devices: i64,
    pub device_utilization: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TopPerformersDto {
    pub top_plans: Vec<serde_json::Value>,
    pub top_players: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RevenueTrendDto {
    pub date: String,
    pub cash_revenue: f64,
    pub online_revenue: f64,
    pub total_revenue: f64,
    pub transaction_count: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffPlayerStatsDto {
    pub active_players: i64,
    pub new_players_in_period: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffDeviceStatsDto {
    pub total: i64,
    pub available: i64,
    pub in_use: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffDashboardStatsDto {
    pub period: PeriodDto,
    pub shift: Option<PeriodDto>,
    pub sessions: UsageStatsDto,
    pub transactions: TransactionStatsDto,
    pub revenue: RevenueByPaymentMethodDto,
    pub shift_revenue: Option<RevenueByPaymentMethodDto>,
    pub players: StaffPlayerStatsDto,
    pub devices: StaffDeviceStatsDto,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStatsDto {
    pub period: PeriodDto,
    pub revenue: PeriodPair<RevenueByPaymentMethodDto>,
    pub transactions: PeriodPair<TransactionStatsDto>,
    pub usage: PeriodPair<UsageStatsDto>,
    pub users: UserStatsDto,
    pub plans: PlanStatsDto,
    pub devices: DeviceStatsDto,
    pub top_performers: TopPerformersDto,
    pub revenue_trend: Vec<RevenueTrendDto>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PeriodDto {
    pub start_date: String,
    pub end_date: String,
    pub label: String,
    pub previous_label: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PeriodPair<T> {
    pub current: T,
    pub previous: T,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PeriodPairRevenueByPaymentMethod {
    pub current: RevenueByPaymentMethodDto,
    pub previous: RevenueByPaymentMethodDto,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PeriodPairUsageStats {
    pub current: UsageStatsDto,
    pub previous: UsageStatsDto,
}

impl From<PeriodPair<RevenueByPaymentMethodDto>> for PeriodPairRevenueByPaymentMethod {
    fn from(value: PeriodPair<RevenueByPaymentMethodDto>) -> Self {
        Self {
            current: value.current,
            previous: value.previous,
        }
    }
}

impl From<PeriodPair<UsageStatsDto>> for PeriodPairUsageStats {
    fn from(value: PeriodPair<UsageStatsDto>) -> Self {
        Self {
            current: value.current,
            previous: value.previous,
        }
    }
}

pub struct StatsService {
    pool: PgPool,
    cache: Arc<dyn CacheService>,
}

#[derive(Serialize)]
struct StatsDashboardKey {
    start: String,
    end: String,
}

#[derive(Serialize)]
struct StatsStaffKey {
    start: String,
    end: String,
    shift_start: Option<String>,
}

#[derive(Serialize)]
struct StatsPeriodPairKey {
    start: String,
    end: String,
    prev_start: String,
    prev_end: String,
}

#[derive(Debug, sqlx::FromRow)]
struct RevenueStatsRow {
    plan: f64,
    merchandise: f64,
    cash_revenue: f64,
    online_revenue: f64,
    credit_revenue: f64,
    plan_transaction_count: i64,
    product_transaction_count: i64,
    plan_cash_revenue: f64,
    plan_online_revenue: f64,
    plan_credit_revenue: f64,
    product_cash_revenue: f64,
    product_online_revenue: f64,
    product_credit_revenue: f64,
    plan_cash_count: i64,
    plan_online_count: i64,
    plan_credit_count: i64,
    product_cash_count: i64,
    product_online_count: i64,
    product_credit_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct TopPlanRow {
    plan_id: uuid::Uuid,
    plan_name: String,
    revenue: f64,
    purchase_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct TopPlayerRow {
    player_id: uuid::Uuid,
    player_name: String,
    total_spent: f64,
    total_sessions: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct DeviceUtilizationRow {
    device_id: uuid::Uuid,
    device_name: String,
    total_sessions: i64,
    total_hours: f64,
}

#[derive(Debug, sqlx::FromRow)]
struct SettlementRevenueTotalsRow {
    settlement_total: f64,
    settlement_cash: f64,
    settlement_online: f64,
}

#[derive(Debug, sqlx::FromRow)]
struct SettlementRevenueByTypeRow {
    plan_cash: f64,
    plan_online: f64,
    product_cash: f64,
    product_online: f64,
}

#[derive(Debug, sqlx::FromRow)]
struct SettlementTrendRow {
    date: chrono::NaiveDate,
    cash_revenue: f64,
    online_revenue: f64,
    total_revenue: f64,
}

#[derive(Debug, sqlx::FromRow)]
struct RevenueTrendRow {
    date: chrono::NaiveDate,
    cash_revenue: f64,
    online_revenue: f64,
    total_revenue: f64,
    transaction_count: i64,
}

impl StatsService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>) -> Self {
        Self { pool, cache }
    }

    pub async fn get_dashboard_stats(
        &self,
        start_date: Option<String>,
        end_date: Option<String>,
    ) -> Result<DashboardStatsDto, AppError> {
        let now = Utc::now();
        let period_start =
            parse_date_start(start_date.as_deref()).unwrap_or_else(|| start_of_day(now));
        let period_end = parse_date_end(end_date.as_deref()).unwrap_or(now);

        let diff_days = (period_end - period_start).num_days().max(1);
        let prev_start = period_start - Duration::days(diff_days);
        let prev_end = period_end - Duration::days(diff_days);

        let cache_key = keys::stats_dashboard(&keys::filter_hash(&StatsDashboardKey {
            start: format_date_key(period_start),
            end: format_date_key(period_end),
        }));

        get_or_set(
            &*self.cache,
            &cache_key,
            keys::ttl::AGGREGATE,
            || async {
                self.compute_dashboard_stats(period_start, period_end, prev_start, prev_end)
                    .await
            },
        )
        .await
    }

    async fn compute_dashboard_stats(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        prev_start: DateTime<Utc>,
        prev_end: DateTime<Utc>,
    ) -> Result<DashboardStatsDto, AppError> {
        let revenue_current = self.revenue_stats(period_start, period_end).await?;
        let revenue_previous = self.revenue_stats(prev_start, prev_end).await?;
        let tx_current = self.transaction_stats(period_start, period_end).await?;
        let tx_previous = self.transaction_stats(prev_start, prev_end).await?;
        let usage_current = self.usage_stats(period_start, period_end).await?;
        let usage_previous = self.usage_stats(prev_start, prev_end).await?;
        let users = self.user_stats(period_start, period_end).await?;
        let plans = self.plan_stats().await?;
        let devices = self.device_stats(period_start, period_end).await?;
        let top_performers = self.top_performers_stats(period_start, period_end).await?;
        let revenue_trend = self.revenue_trend_stats(period_start, period_end).await?;

        Ok(DashboardStatsDto {
            period: PeriodDto {
                start_date: period_start.to_rfc3339(),
                end_date: period_end.to_rfc3339(),
                label: format!(
                    "{} - {}",
                    period_start.format("%Y-%m-%d"),
                    period_end.format("%Y-%m-%d")
                ),
                previous_label: format!(
                    "{} - {}",
                    prev_start.format("%Y-%m-%d"),
                    prev_end.format("%Y-%m-%d")
                ),
            },
            revenue: PeriodPair {
                current: revenue_current,
                previous: revenue_previous,
            },
            transactions: PeriodPair {
                current: tx_current,
                previous: tx_previous,
            },
            usage: PeriodPair {
                current: usage_current,
                previous: usage_previous,
            },
            users,
            plans,
            devices,
            top_performers,
            revenue_trend,
        })
    }

    pub async fn get_staff_dashboard_stats(
        &self,
        start_date: Option<String>,
        end_date: Option<String>,
        shift_start: Option<String>,
    ) -> Result<StaffDashboardStatsDto, AppError> {
        let now = Utc::now();
        let period_start =
            parse_date_start(start_date.as_deref()).unwrap_or_else(|| start_of_day(now));
        let period_end = parse_date_end(end_date.as_deref()).unwrap_or(now);

        let cache_key = keys::stats_staff(&keys::filter_hash(&StatsStaffKey {
            start: format_date_key(period_start),
            end: format_date_key(period_end),
            shift_start: shift_start.clone(),
        }));

        get_or_set(
            &*self.cache,
            &cache_key,
            keys::ttl::AGGREGATE,
            || async {
                self.compute_staff_dashboard_stats(
                    period_start,
                    period_end,
                    shift_start,
                    now,
                )
                .await
            },
        )
        .await
    }

    async fn compute_staff_dashboard_stats(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        shift_start: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<StaffDashboardStatsDto, AppError> {
        let revenue = self.revenue_stats(period_start, period_end).await?;
        let transactions = self.transaction_stats(period_start, period_end).await?;
        let sessions = self.usage_stats(period_start, period_end).await?;
        let players = self.staff_player_stats(period_start, period_end).await?;
        let devices = self.staff_device_stats().await?;

        let (shift, shift_revenue) = if let Some(shift_start_raw) = shift_start {
            let shift_start_dt = DateTime::parse_from_rfc3339(&shift_start_raw)
                .map(|d| d.with_timezone(&Utc))
                .map_err(|_| {
                    AppError::BadRequest("shiftStart must be a valid ISO 8601 datetime".to_string())
                })?;
            let shift_end = now;
            let shift_revenue = self.revenue_stats(shift_start_dt, shift_end).await?;
            (
                Some(PeriodDto {
                    start_date: shift_start_dt.to_rfc3339(),
                    end_date: shift_end.to_rfc3339(),
                    label: "Current shift".to_string(),
                    previous_label: String::new(),
                }),
                Some(shift_revenue),
            )
        } else {
            (None, None)
        };

        Ok(StaffDashboardStatsDto {
            period: PeriodDto {
                start_date: period_start.to_rfc3339(),
                end_date: period_end.to_rfc3339(),
                label: format!(
                    "{} - {}",
                    period_start.format("%Y-%m-%d"),
                    period_end.format("%Y-%m-%d")
                ),
                previous_label: String::new(),
            },
            shift,
            sessions,
            transactions,
            revenue,
            shift_revenue,
            players,
            devices,
        })
    }

    pub fn resolve_stats_period(
        start_date: Option<String>,
        end_date: Option<String>,
    ) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
        let start =
            parse_date_start(start_date.as_deref()).unwrap_or_else(|| start_of_day(now));
        let end = parse_date_end(end_date.as_deref()).unwrap_or(now);
        (start, end)
    }

    pub async fn get_revenue_by_payment_method(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        prev_start: DateTime<Utc>,
        prev_end: DateTime<Utc>,
    ) -> Result<PeriodPair<RevenueByPaymentMethodDto>, AppError> {
        let cache_key = keys::stats_revenue(&keys::filter_hash(&StatsPeriodPairKey {
            start: format_date_key(start),
            end: format_date_key(end),
            prev_start: format_date_key(prev_start),
            prev_end: format_date_key(prev_end),
        }));

        get_or_set(
            &*self.cache,
            &cache_key,
            keys::ttl::AGGREGATE,
            || async {
                Ok(PeriodPair {
                    current: self.revenue_stats(start, end).await?,
                    previous: self.revenue_stats(prev_start, prev_end).await?,
                })
            },
        )
        .await
    }

    pub async fn get_usage_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        prev_start: DateTime<Utc>,
        prev_end: DateTime<Utc>,
    ) -> Result<PeriodPair<UsageStatsDto>, AppError> {
        let cache_key = keys::stats_usage(&keys::filter_hash(&StatsPeriodPairKey {
            start: format_date_key(start),
            end: format_date_key(end),
            prev_start: format_date_key(prev_start),
            prev_end: format_date_key(prev_end),
        }));

        get_or_set(
            &*self.cache,
            &cache_key,
            keys::ttl::AGGREGATE,
            || async {
                Ok(PeriodPair {
                    current: self.usage_stats(start, end).await?,
                    previous: self.usage_stats(prev_start, prev_end).await?,
                })
            },
        )
        .await
    }

    async fn revenue_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<RevenueByPaymentMethodDto, AppError> {
        let row: RevenueStatsRow = sqlx::query_as(
            r#"
            SELECT
                COALESCE(SUM(CASE WHEN "transactionType"::text = 'plan_purchase' THEN amount::float8 ELSE 0 END), 0) AS plan,
                COALESCE(SUM(CASE WHEN "transactionType"::text = 'product_purchase' THEN amount::float8 ELSE 0 END), 0) AS merchandise,
                COALESCE(SUM(
                    CASE
                        WHEN "paymentMethod"::text = 'cash' THEN amount::float8
                        WHEN "paymentMethod"::text = 'split_payment' THEN COALESCE("cashAmount", 0)::float8
                        ELSE 0
                    END
                ), 0) AS cash_revenue,
                COALESCE(SUM(
                    CASE
                        WHEN "paymentMethod"::text = 'online' THEN amount::float8
                        WHEN "paymentMethod"::text = 'split_payment' THEN COALESCE("onlineAmount", 0)::float8
                        ELSE 0
                    END
                ), 0) AS online_revenue,
                COALESCE(SUM(
                    CASE WHEN "paymentMethod"::text = 'credit' THEN amount::float8 ELSE 0 END
                ), 0) AS credit_revenue,
                COUNT(*) FILTER (WHERE "transactionType"::text = 'plan_purchase') AS plan_transaction_count,
                COUNT(*) FILTER (WHERE "transactionType"::text = 'product_purchase') AS product_transaction_count,
                COALESCE(SUM(
                    CASE
                        WHEN "transactionType"::text = 'plan_purchase' AND "paymentMethod"::text = 'cash' THEN amount::float8
                        WHEN "transactionType"::text = 'plan_purchase' AND "paymentMethod"::text = 'split_payment' THEN COALESCE("cashAmount", 0)::float8
                        ELSE 0
                    END
                ), 0) AS plan_cash_revenue,
                COALESCE(SUM(
                    CASE
                        WHEN "transactionType"::text = 'plan_purchase' AND "paymentMethod"::text = 'online' THEN amount::float8
                        WHEN "transactionType"::text = 'plan_purchase' AND "paymentMethod"::text = 'split_payment' THEN COALESCE("onlineAmount", 0)::float8
                        ELSE 0
                    END
                ), 0) AS plan_online_revenue,
                COALESCE(SUM(
                    CASE
                        WHEN "transactionType"::text = 'plan_purchase' AND "paymentMethod"::text = 'credit' THEN amount::float8
                        ELSE 0
                    END
                ), 0) AS plan_credit_revenue,
                COALESCE(SUM(
                    CASE
                        WHEN "transactionType"::text = 'product_purchase' AND "paymentMethod"::text = 'cash' THEN amount::float8
                        WHEN "transactionType"::text = 'product_purchase' AND "paymentMethod"::text = 'split_payment' THEN COALESCE("cashAmount", 0)::float8
                        ELSE 0
                    END
                ), 0) AS product_cash_revenue,
                COALESCE(SUM(
                    CASE
                        WHEN "transactionType"::text = 'product_purchase' AND "paymentMethod"::text = 'online' THEN amount::float8
                        WHEN "transactionType"::text = 'product_purchase' AND "paymentMethod"::text = 'split_payment' THEN COALESCE("onlineAmount", 0)::float8
                        ELSE 0
                    END
                ), 0) AS product_online_revenue,
                COALESCE(SUM(
                    CASE
                        WHEN "transactionType"::text = 'product_purchase' AND "paymentMethod"::text = 'credit' THEN amount::float8
                        ELSE 0
                    END
                ), 0) AS product_credit_revenue,
                COUNT(*) FILTER (
                    WHERE "transactionType"::text = 'plan_purchase'
                      AND (
                        "paymentMethod"::text = 'cash'
                        OR ("paymentMethod"::text = 'split_payment' AND COALESCE("cashAmount", 0) > 0)
                      )
                ) AS plan_cash_count,
                COUNT(*) FILTER (
                    WHERE "transactionType"::text = 'plan_purchase'
                      AND (
                        "paymentMethod"::text = 'online'
                        OR ("paymentMethod"::text = 'split_payment' AND COALESCE("onlineAmount", 0) > 0)
                      )
                ) AS plan_online_count,
                COUNT(*) FILTER (
                    WHERE "transactionType"::text = 'plan_purchase' AND "paymentMethod"::text = 'credit'
                ) AS plan_credit_count,
                COUNT(*) FILTER (
                    WHERE "transactionType"::text = 'product_purchase'
                      AND (
                        "paymentMethod"::text = 'cash'
                        OR ("paymentMethod"::text = 'split_payment' AND COALESCE("cashAmount", 0) > 0)
                      )
                ) AS product_cash_count,
                COUNT(*) FILTER (
                    WHERE "transactionType"::text = 'product_purchase'
                      AND (
                        "paymentMethod"::text = 'online'
                        OR ("paymentMethod"::text = 'split_payment' AND COALESCE("onlineAmount", 0) > 0)
                      )
                ) AS product_online_count,
                COUNT(*) FILTER (
                    WHERE "transactionType"::text = 'product_purchase' AND "paymentMethod"::text = 'credit'
                ) AS product_credit_count
            FROM transactions
            WHERE "createdAt" BETWEEN $1 AND $2
              AND "deletedAt" IS NULL
              AND "paymentStatus"::text IN ('completed', 'credit')
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await?;

        let settlement_totals = self.settlement_revenue_totals(start, end).await?;
        let settlement_by_type = self.settlement_revenue_by_type(start, end).await?;

        let plan_cash = row.plan_cash_revenue + settlement_by_type.plan_cash;
        let plan_online = row.plan_online_revenue + settlement_by_type.plan_online;
        let plan_credit = row.plan_credit_revenue;
        let product_cash = row.product_cash_revenue + settlement_by_type.product_cash;
        let product_online = row.product_online_revenue + settlement_by_type.product_online;
        let product_credit = row.product_credit_revenue;

        let plan = plan_cash + plan_online + plan_credit;
        let merchandise = product_cash + product_online + product_credit;
        let cash_revenue = row.cash_revenue + settlement_totals.settlement_cash;
        let online_revenue = row.online_revenue + settlement_totals.settlement_online;
        let credit_revenue = row.credit_revenue;
        let total = plan + merchandise;

        Ok(RevenueByPaymentMethodDto {
            plan,
            merchandise,
            total,
            cash_revenue,
            online_revenue,
            credit_revenue,
            plan_transaction_count: row.plan_transaction_count,
            product_transaction_count: row.product_transaction_count,
            plan_cash_revenue: plan_cash,
            plan_online_revenue: plan_online,
            plan_credit_revenue: plan_credit,
            product_cash_revenue: product_cash,
            product_online_revenue: product_online,
            product_credit_revenue: product_credit,
            plan_cash_count: row.plan_cash_count,
            plan_online_count: row.plan_online_count,
            plan_credit_count: row.plan_credit_count,
            product_cash_count: row.product_cash_count,
            product_online_count: row.product_online_count,
            product_credit_count: row.product_credit_count,
        })
    }

    async fn settlement_revenue_totals(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<SettlementRevenueTotalsRow, AppError> {
        sqlx::query_as(
            r#"
            SELECT
                COALESCE(SUM(cs.amount), 0)::float8 AS settlement_total,
                COALESCE(SUM(
                    CASE
                        WHEN cs."paymentMethod" = 'cash' THEN cs.amount
                        WHEN cs."paymentMethod" = 'split_payment' THEN COALESCE(cs."cashAmount", 0)
                        ELSE 0
                    END
                ), 0)::float8 AS settlement_cash,
                COALESCE(SUM(
                    CASE
                        WHEN cs."paymentMethod" = 'online' THEN cs.amount
                        WHEN cs."paymentMethod" = 'split_payment' THEN COALESCE(cs."onlineAmount", 0)
                        ELSE 0
                    END
                ), 0)::float8 AS settlement_online
            FROM credit_settlements cs
            WHERE cs."settledAt" BETWEEN $1 AND $2
              AND cs."deletedAt" IS NULL
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    async fn settlement_revenue_by_type(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<SettlementRevenueByTypeRow, AppError> {
        sqlx::query_as(
            r#"
            SELECT
                COALESCE(SUM(
                    CASE WHEN t."transactionType"::text = 'plan_purchase' THEN
                        CASE
                            WHEN cs."paymentMethod" = 'cash' THEN csi."amountApplied"
                            WHEN cs."paymentMethod" = 'online' THEN 0
                            WHEN cs."paymentMethod" = 'split_payment' AND cs.amount > 0 THEN
                                csi."amountApplied" * (COALESCE(cs."cashAmount", 0) / cs.amount)
                            ELSE 0
                        END
                    ELSE 0 END
                ), 0)::float8 AS plan_cash,
                COALESCE(SUM(
                    CASE WHEN t."transactionType"::text = 'plan_purchase' THEN
                        CASE
                            WHEN cs."paymentMethod" = 'online' THEN csi."amountApplied"
                            WHEN cs."paymentMethod" = 'cash' THEN 0
                            WHEN cs."paymentMethod" = 'split_payment' AND cs.amount > 0 THEN
                                csi."amountApplied" * (COALESCE(cs."onlineAmount", 0) / cs.amount)
                            ELSE 0
                        END
                    ELSE 0 END
                ), 0)::float8 AS plan_online,
                COALESCE(SUM(
                    CASE WHEN t."transactionType"::text = 'product_purchase' THEN
                        CASE
                            WHEN cs."paymentMethod" = 'cash' THEN csi."amountApplied"
                            WHEN cs."paymentMethod" = 'online' THEN 0
                            WHEN cs."paymentMethod" = 'split_payment' AND cs.amount > 0 THEN
                                csi."amountApplied" * (COALESCE(cs."cashAmount", 0) / cs.amount)
                            ELSE 0
                        END
                    ELSE 0 END
                ), 0)::float8 AS product_cash,
                COALESCE(SUM(
                    CASE WHEN t."transactionType"::text = 'product_purchase' THEN
                        CASE
                            WHEN cs."paymentMethod" = 'online' THEN csi."amountApplied"
                            WHEN cs."paymentMethod" = 'cash' THEN 0
                            WHEN cs."paymentMethod" = 'split_payment' AND cs.amount > 0 THEN
                                csi."amountApplied" * (COALESCE(cs."onlineAmount", 0) / cs.amount)
                            ELSE 0
                        END
                    ELSE 0 END
                ), 0)::float8 AS product_online
            FROM credit_settlements cs
            INNER JOIN credit_settlement_items csi ON csi."settlementId" = cs.id
            INNER JOIN transactions t ON t.id = csi."transactionId"
            WHERE cs."settledAt" BETWEEN $1 AND $2
              AND cs."deletedAt" IS NULL
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    async fn transaction_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<TransactionStatsDto, AppError> {
        let row: (i64, i64, i64, i64, Option<f64>) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*),
                COUNT(*) FILTER (WHERE "paymentStatus"::text IN ('completed', 'credit')),
                COUNT(*) FILTER (WHERE "paymentStatus" = 'pending'),
                COUNT(*) FILTER (WHERE "paymentStatus" = 'failed'),
                AVG(amount::float8)
            FROM transactions
            WHERE "createdAt" BETWEEN $1 AND $2 AND "deletedAt" IS NULL
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await?;

        Ok(TransactionStatsDto {
            total_transactions: row.0,
            completed_transactions: row.1,
            pending_transactions: row.2,
            failed_transactions: row.3,
            average_transaction_amount: row.4.unwrap_or(0.0),
        })
    }

    async fn usage_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<UsageStatsDto, AppError> {
        let row: (i64, i64, i64, Option<i64>, Option<f64>) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*),
                COUNT(*) FILTER (WHERE "endTime" IS NULL),
                COUNT(*) FILTER (WHERE "endTime" IS NOT NULL),
                COALESCE(SUM("durationMinutes"), 0),
                AVG("durationMinutes"::float8)
            FROM usage_sessions
            WHERE "startTime" BETWEEN $1 AND $2 AND "deletedAt" IS NULL
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await?;

        let total_minutes = row.3.unwrap_or(0);
        Ok(UsageStatsDto {
            total_sessions: row.0,
            active_sessions: row.1,
            completed_sessions: row.2,
            total_hours: total_minutes as f64 / 60.0,
            total_minutes,
            average_session_duration: row.4.unwrap_or(0.0),
        })
    }

    async fn user_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<UserStatsDto, AppError> {
        let row: (i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*),
                COUNT(*) FILTER (WHERE "isActive" = true),
                COUNT(*) FILTER (WHERE role = 'player'),
                COUNT(*) FILTER (WHERE role = 'player' AND "isActive" = true),
                COUNT(*) FILTER (WHERE "createdAt" BETWEEN $1 AND $2)
            FROM users
            WHERE "deletedAt" IS NULL
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await?;

        Ok(UserStatsDto {
            total_users: row.0,
            active_users: row.1,
            total_players: row.2,
            active_players: row.3,
            new_users_this_period: row.4,
        })
    }

    async fn plan_stats(&self) -> Result<PlanStatsDto, AppError> {
        let active: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM player_plan_balances WHERE status = 'active' AND "deletedAt" IS NULL"#,
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0,));
        let expired: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM player_plan_balances WHERE status = 'expired' AND "deletedAt" IS NULL"#,
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0,));

        Ok(PlanStatsDto {
            total_active_plans: active.0,
            total_expired_plans: expired.0,
            plans_by_type: vec![],
        })
    }

    async fn device_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<DeviceStatsDto, AppError> {
        let row: (i64, i64) = sqlx::query_as(
            r#"
            SELECT COUNT(*), COUNT(*) FILTER (WHERE status IN ('operational', 'available', 'in_use'))
            FROM devices WHERE "deletedAt" IS NULL
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        let utilization_rows: Vec<DeviceUtilizationRow> = sqlx::query_as(
            r#"
            SELECT
                d.id AS device_id,
                d.name AS device_name,
                COUNT(s.id) AS total_sessions,
                COALESCE(SUM(s."durationMinutes"), 0)::float8 / 60.0 AS total_hours
            FROM devices d
            INNER JOIN usage_sessions s ON s."deviceId" = d.id
            WHERE d."deletedAt" IS NULL
              AND s."deletedAt" IS NULL
              AND s."startTime" BETWEEN $1 AND $2
            GROUP BY d.id, d.name
            ORDER BY total_hours DESC, total_sessions DESC
            LIMIT 10
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        let period_minutes = (end - start).num_minutes().max(1) as f64;
        let device_utilization = utilization_rows
            .into_iter()
            .map(|row| {
                let utilization_minutes = row.total_hours * 60.0;
                let utilization_percentage =
                    ((utilization_minutes / period_minutes) * 100.0).clamp(0.0, 100.0);
                serde_json::json!({
                    "deviceId": row.device_id,
                    "deviceName": row.device_name,
                    "totalSessions": row.total_sessions,
                    "totalHours": row.total_hours,
                    "utilizationPercentage": utilization_percentage,
                })
            })
            .collect();

        Ok(DeviceStatsDto {
            total_devices: row.0,
            active_devices: row.1,
            device_utilization,
        })
    }

    async fn top_performers_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<TopPerformersDto, AppError> {
        let top_plans: Vec<TopPlanRow> = sqlx::query_as(
            r#"
            SELECT
                p.id AS plan_id,
                p.name AS plan_name,
                COALESCE(SUM(t.amount::float8), 0) AS revenue,
                COUNT(*) AS purchase_count
            FROM transactions t
            INNER JOIN plans p ON p.id = t."planId"
            WHERE t."createdAt" BETWEEN $1 AND $2
              AND t."deletedAt" IS NULL
              AND p."deletedAt" IS NULL
              AND t."paymentStatus"::text IN ('completed', 'credit')
              AND t."transactionType"::text = 'plan_purchase'
            GROUP BY p.id, p.name
            ORDER BY revenue DESC, purchase_count DESC
            LIMIT 5
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        let top_players: Vec<TopPlayerRow> = sqlx::query_as(
            r#"
            SELECT
                u.id AS player_id,
                COALESCE(
                    NULLIF(TRIM(CONCAT(COALESCE(u."firstName", ''), ' ', COALESCE(u."lastName", ''))), ''),
                    u.username
                ) AS player_name,
                COALESCE(SUM(t.amount::float8), 0) AS total_spent,
                COALESCE(session_counts.total_sessions, 0) AS total_sessions
            FROM transactions t
            INNER JOIN users u ON u.id = t."playerId"
            LEFT JOIN (
                SELECT
                    ppb."playerId" AS player_id,
                    COUNT(*) AS total_sessions
                FROM usage_sessions s
                INNER JOIN player_plan_balances ppb ON ppb.id = s."balanceId"
                WHERE s."deletedAt" IS NULL
                  AND ppb."deletedAt" IS NULL
                  AND s."startTime" BETWEEN $1 AND $2
                GROUP BY ppb."playerId"
            ) session_counts ON session_counts.player_id = u.id
            WHERE t."createdAt" BETWEEN $1 AND $2
              AND t."deletedAt" IS NULL
              AND u."deletedAt" IS NULL
              AND u.role = 'player'
              AND t."paymentStatus"::text IN ('completed', 'credit')
            GROUP BY u.id, u.username, u."firstName", u."lastName", session_counts.total_sessions
            ORDER BY total_spent DESC, total_sessions DESC
            LIMIT 5
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(TopPerformersDto {
            top_plans: top_plans
                .into_iter()
                .map(|row| {
                    serde_json::json!({
                        "planId": row.plan_id,
                        "planName": row.plan_name,
                        "revenue": row.revenue,
                        "purchaseCount": row.purchase_count,
                    })
                })
                .collect(),
            top_players: top_players
                .into_iter()
                .map(|row| {
                    serde_json::json!({
                        "playerId": row.player_id,
                        "playerName": row.player_name,
                        "totalSpent": row.total_spent,
                        "totalSessions": row.total_sessions,
                    })
                })
                .collect(),
        })
    }

    async fn revenue_trend_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<RevenueTrendDto>, AppError> {
        let rows: Vec<RevenueTrendRow> = sqlx::query_as(
            r#"
            SELECT
                DATE(t."createdAt" AT TIME ZONE 'Asia/Kolkata') AS date,
                COALESCE(SUM(
                    CASE
                        WHEN t."paymentMethod"::text = 'cash' THEN t.amount::float8
                        WHEN t."paymentMethod"::text = 'split_payment' THEN COALESCE(t."cashAmount", 0)::float8
                        ELSE 0
                    END
                ), 0) AS cash_revenue,
                COALESCE(SUM(
                    CASE
                        WHEN t."paymentMethod"::text = 'online' THEN t.amount::float8
                        WHEN t."paymentMethod"::text = 'split_payment' THEN COALESCE(t."onlineAmount", 0)::float8
                        ELSE 0
                    END
                ), 0) AS online_revenue,
                COALESCE(SUM(t.amount::float8), 0) AS total_revenue,
                COUNT(*) AS transaction_count
            FROM transactions t
            WHERE t."createdAt" BETWEEN $1 AND $2
              AND t."deletedAt" IS NULL
              AND t."paymentStatus"::text IN ('completed', 'credit')
            GROUP BY DATE(t."createdAt" AT TIME ZONE 'Asia/Kolkata')
            ORDER BY date ASC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        let settlement_rows = self.settlement_trend_stats(start, end).await?;

        let mut by_date: std::collections::BTreeMap<String, RevenueTrendDto> = rows
            .into_iter()
            .map(|row| {
                (
                    row.date.format("%Y-%m-%d").to_string(),
                    RevenueTrendDto {
                        date: row.date.format("%Y-%m-%d").to_string(),
                        cash_revenue: row.cash_revenue,
                        online_revenue: row.online_revenue,
                        total_revenue: row.total_revenue,
                        transaction_count: row.transaction_count,
                    },
                )
            })
            .collect();

        for row in settlement_rows {
            let key = row.date.format("%Y-%m-%d").to_string();
            by_date
                .entry(key.clone())
                .and_modify(|entry| {
                    entry.cash_revenue += row.cash_revenue;
                    entry.online_revenue += row.online_revenue;
                    entry.total_revenue += row.total_revenue;
                })
                .or_insert(RevenueTrendDto {
                    date: key,
                    cash_revenue: row.cash_revenue,
                    online_revenue: row.online_revenue,
                    total_revenue: row.total_revenue,
                    transaction_count: 0,
                });
        }

        Ok(by_date.into_values().collect())
    }

    async fn settlement_trend_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<SettlementTrendRow>, AppError> {
        sqlx::query_as(
            r#"
            SELECT
                DATE(cs."settledAt" AT TIME ZONE 'Asia/Kolkata') AS date,
                COALESCE(SUM(
                    CASE
                        WHEN cs."paymentMethod" = 'cash' THEN cs.amount
                        WHEN cs."paymentMethod" = 'split_payment' THEN COALESCE(cs."cashAmount", 0)
                        ELSE 0
                    END
                ), 0)::float8 AS cash_revenue,
                COALESCE(SUM(
                    CASE
                        WHEN cs."paymentMethod" = 'online' THEN cs.amount
                        WHEN cs."paymentMethod" = 'split_payment' THEN COALESCE(cs."onlineAmount", 0)
                        ELSE 0
                    END
                ), 0)::float8 AS online_revenue,
                COALESCE(SUM(cs.amount), 0)::float8 AS total_revenue
            FROM credit_settlements cs
            WHERE cs."settledAt" BETWEEN $1 AND $2
              AND cs."deletedAt" IS NULL
            GROUP BY DATE(cs."settledAt" AT TIME ZONE 'Asia/Kolkata')
            ORDER BY date ASC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)
    }

    async fn staff_player_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<StaffPlayerStatsDto, AppError> {
        let row: (i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE role = 'player' AND "isActive" = true),
                COUNT(*) FILTER (WHERE role = 'player' AND "createdAt" BETWEEN $1 AND $2)
            FROM users
            WHERE "deletedAt" IS NULL
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await?;

        Ok(StaffPlayerStatsDto {
            active_players: row.0,
            new_players_in_period: row.1,
        })
    }

    async fn staff_device_stats(&self) -> Result<StaffDeviceStatsDto, AppError> {
        let row: (i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*),
                COUNT(*) FILTER (WHERE status IN ('available', 'operational')),
                COUNT(*) FILTER (WHERE status = 'in_use')
            FROM devices
            WHERE "deletedAt" IS NULL
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(StaffDeviceStatsDto {
            total: row.0,
            available: row.1,
            in_use: row.2,
        })
    }
}

fn format_date_key(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d").to_string()
}

fn parse_date_start(value: Option<&str>) -> Option<DateTime<Utc>> {
    value.and_then(|s| {
        DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.with_timezone(&Utc))
            .or_else(|| {
                DateTime::parse_from_rfc3339(&format!("{s}T00:00:00+05:30"))
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            })
            .or_else(|| {
                DateTime::parse_from_rfc3339(&format!("{s}T00:00:00Z"))
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            })
    })
}

fn parse_date_end(value: Option<&str>) -> Option<DateTime<Utc>> {
    value.and_then(|s| {
        DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.with_timezone(&Utc))
            .or_else(|| {
                DateTime::parse_from_rfc3339(&format!("{s}T23:59:59+05:30"))
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            })
            .or_else(|| {
                DateTime::parse_from_rfc3339(&format!("{s}T23:59:59Z"))
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            })
    })
}

fn start_of_day(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.with_hour(0)
        .unwrap_or(dt)
        .with_minute(0)
        .unwrap_or(dt)
        .with_second(0)
        .unwrap_or(dt)
        .with_nanosecond(0)
        .unwrap_or(dt)
}