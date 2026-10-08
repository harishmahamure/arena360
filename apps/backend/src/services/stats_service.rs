use crate::analytics::report_reader::ReportReader;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<T>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PeriodPairRevenueByPaymentMethod {
    pub current: RevenueByPaymentMethodDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<RevenueByPaymentMethodDto>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PeriodPairUsageStats {
    pub current: UsageStatsDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<UsageStatsDto>,
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

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FinanceReconciliationMetricsDto {
    pub open_count: i64,
    pub closed_count: i64,
    pub reconciled_count: i64,
    pub pending_reconcile_count: i64,
    pub total_deposited: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FinanceReconciliationStatsDto {
    pub period: PeriodDto,
    pub metrics: PeriodPair<FinanceReconciliationMetricsDto>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FinanceDepositMetricsDto {
    pub pending_count: i64,
    pub pending_amount: f64,
    pub approved_count: i64,
    pub approved_amount: f64,
    pub rejected_count: i64,
    pub rejected_amount: f64,
    pub bank_amount: f64,
    pub home_amount: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FinanceDepositStatsDto {
    pub period: PeriodDto,
    pub metrics: PeriodPair<FinanceDepositMetricsDto>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FinanceVarianceMetricsDto {
    pub total_variance: f64,
    pub average_variance: f64,
    pub over_count: i64,
    pub short_count: i64,
    pub even_count: i64,
    pub register_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FinanceVarianceRegisterRow {
    pub id: Uuid,
    pub shift_id: Uuid,
    pub status: String,
    pub variance: f64,
    pub closing_balance: Option<f64>,
    pub expected_closing: Option<f64>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FinanceVarianceStatsDto {
    pub period: PeriodDto,
    pub metrics: PeriodPair<FinanceVarianceMetricsDto>,
    pub registers: Vec<FinanceVarianceRegisterRow>,
}

pub struct StatsService {
    pool: ReportReader,
    cache: Arc<dyn CacheService>,
}

#[derive(Serialize)]
struct StatsDashboardKey {
    start: String,
    end: String,
    compare: bool,
}

#[derive(Serialize)]
struct StatsFinanceKey {
    start: String,
    end: String,
    compare: bool,
}

#[derive(Serialize)]
struct StatsStaffKey {
    start: String,
    end: String,
    shift_start: Option<String>,
}

#[derive(Serialize)]
struct StatsPeriodPairKey {
    compare: bool,
    start: String,
    end: String,
    prev_start: String,
    prev_end: String,
}

#[derive(Debug, Deserialize)]
struct RevenueStatsRow {
    _plan: f64,
    _merchandise: f64,
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

#[derive(Debug, Deserialize)]
struct TopPlanRow {
    plan_id: uuid::Uuid,
    plan_name: String,
    revenue: f64,
    purchase_count: i64,
}

#[derive(Debug, Deserialize)]
struct TopPlayerRow {
    player_id: uuid::Uuid,
    player_name: String,
    total_spent: f64,
    total_sessions: i64,
}

#[derive(Debug, Deserialize)]
struct DeviceUtilizationRow {
    device_id: uuid::Uuid,
    device_name: String,
    total_sessions: i64,
    total_hours: f64,
}

#[derive(Debug, Deserialize)]
struct SettlementRevenueTotalsRow {
    _settlement_total: f64,
    settlement_cash: f64,
    settlement_online: f64,
}

#[derive(Debug, Deserialize)]
struct SettlementRevenueByTypeRow {
    plan_cash: f64,
    plan_online: f64,
    product_cash: f64,
    product_online: f64,
}

#[derive(Debug, Deserialize)]
struct SettlementTrendRow {
    date: chrono::NaiveDate,
    cash_revenue: f64,
    online_revenue: f64,
    total_revenue: f64,
}

#[derive(Debug, Deserialize)]
struct RevenueTrendRow {
    date: chrono::NaiveDate,
    cash_revenue: f64,
    online_revenue: f64,
    total_revenue: f64,
    transaction_count: i64,
}

impl StatsService {
    pub fn previous_window(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> (DateTime<Utc>,DateTime<Utc>) {
        previous_window(start,end,self.pool.timezone())
    }
    pub async fn get_business_report(
        &self,
        window: crate::analytics::business::Window,
    ) -> Result<crate::analytics::business::BusinessReport, AppError> {
        self.pool.ensure_ready().await?;
        self.pool.check_window(window.previous_start, window.end)?;
        let key = format!(
            "stats:duckdb:business:v3:{}:{}:{}",
            self.pool.cache_key(),
            window.start_date,
            window.end_date
        );
        get_or_set(
            &*self.cache,
            &key,
            std::time::Duration::from_secs(30),
            || async { self.pool.business_report(window).await },
        )
        .await
    }

    pub fn new(pool: ReportReader, cache: Arc<dyn CacheService>) -> Self {
        Self { pool, cache }
    }

    pub async fn get_dashboard_stats(
        &self,
        start_date: Option<String>,
        end_date: Option<String>,
        compare: bool,
    ) -> Result<DashboardStatsDto, AppError> {
        self.pool.ensure_ready().await?;
        let now = self.pool.now();
        let period_start =
            parse_date_start(start_date.as_deref(), self.pool.timezone()).unwrap_or_else(|| start_of_day(now, self.pool.timezone()));
        let period_end = parse_date_end(end_date.as_deref(), self.pool.timezone()).unwrap_or(now);
        self.pool.check_window(period_start, period_end)?;

        if compare {
            let (a,b) = self.previous_window(period_start,period_end);
            self.pool.check_window(a,b)?;
        }
        let cache_key = keys::stats_dashboard(&keys::filter_hash(&StatsDashboardKey {
            start: format_date_key(period_start),
            end: format_date_key(period_end),
            compare,
        }));

        let cache_key = format!("{cache_key}:scope:{}", self.pool.cache_key());
        get_or_set(&*self.cache, &cache_key, keys::ttl::AGGREGATE, || async {
            self.compute_dashboard_stats(period_start, period_end, compare)
                .await
        })
        .await
    }

    async fn compute_dashboard_stats(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        compare: bool,
    ) -> Result<DashboardStatsDto, AppError> {
        let (prev_start, prev_end) = previous_window(period_start, period_end, self.pool.timezone());
        let revenue_current = self.revenue_stats(period_start, period_end).await?;
        let revenue_previous = if compare {
            Some(self.revenue_stats(prev_start, prev_end).await?)
        } else {
            None
        };
        let tx_current = self.transaction_stats(period_start, period_end).await?;
        let tx_previous = if compare {
            Some(self.transaction_stats(prev_start, prev_end).await?)
        } else {
            None
        };
        let usage_current = self.usage_stats(period_start, period_end).await?;
        let usage_previous = if compare {
            Some(self.usage_stats(prev_start, prev_end).await?)
        } else {
            None
        };
        let users = self.user_stats(period_start, period_end).await?;
        let plans = self.plan_stats().await?;
        let devices = self.device_stats(period_start, period_end).await?;
        let top_performers = self.top_performers_stats(period_start, period_end).await?;
        let revenue_trend = self.revenue_trend_stats(period_start, period_end).await?;

        Ok(DashboardStatsDto {
            period: period_dto(period_start, period_end, compare, prev_start, prev_end),
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
        self.pool.ensure_ready().await?;
        let now = self.pool.now();
        let period_start =
            parse_date_start(start_date.as_deref(), self.pool.timezone()).unwrap_or_else(|| start_of_day(now, self.pool.timezone()));
        let period_end = parse_date_end(end_date.as_deref(), self.pool.timezone()).unwrap_or(now);
        self.pool.check_window(period_start, period_end)?;

        if let Some(raw) = shift_start.as_deref() {
            let shift = DateTime::parse_from_rfc3339(raw).map(|t|t.with_timezone(&Utc))
                .map_err(|_|AppError::BadRequest("Invalid shift start".into()))?;
            self.pool.check_window(shift,now)?;
        }
        let cache_key = keys::stats_staff(&keys::filter_hash(&StatsStaffKey {
            start: format_date_key(period_start),
            end: format_date_key(period_end),
            shift_start: shift_start.clone(),
        }));

        let cache_key = format!("{cache_key}:scope:{}", self.pool.cache_key());
        get_or_set(&*self.cache, &cache_key, keys::ttl::AGGREGATE, || async {
            self.compute_staff_dashboard_stats(period_start, period_end, shift_start, now)
                .await
        })
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
                    start_date: crate::time::utc_timestamp(&shift_start_dt),
                    end_date: crate::time::utc_timestamp(&shift_end),
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
                start_date: crate::time::utc_timestamp(&period_start),
                end_date: crate::time::utc_timestamp(&period_end),
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
        &self,
        start_date: Option<String>,
        end_date: Option<String>,
    ) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = self.pool.now();
        let start = parse_date_start(start_date.as_deref(), self.pool.timezone()).unwrap_or_else(|| start_of_day(now, self.pool.timezone()));
        let end = parse_date_end(end_date.as_deref(), self.pool.timezone()).unwrap_or(now);
        (start, end)
    }

    pub async fn get_revenue_by_payment_method(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        prev_start: DateTime<Utc>,
        prev_end: DateTime<Utc>,
        compare: bool,
    ) -> Result<PeriodPair<RevenueByPaymentMethodDto>, AppError> {
        self.pool.ensure_ready().await?;
        self.pool.check_window(start,end)?;
        if compare { self.pool.check_window(prev_start,prev_end)?; }
        let cache_key = keys::stats_revenue(&keys::filter_hash(&StatsPeriodPairKey {
            compare,
            start: format_date_key(start),
            end: format_date_key(end),
            prev_start: format_date_key(prev_start),
            prev_end: format_date_key(prev_end),
        }));

        let cache_key = format!("{cache_key}:scope:{}", self.pool.cache_key());
        get_or_set(&*self.cache, &cache_key, keys::ttl::AGGREGATE, || async {
            Ok(PeriodPair {
                current: self.revenue_stats(start, end).await?,
                previous: if compare {
                    Some(self.revenue_stats(prev_start, prev_end).await?)
                } else {
                    None
                },
            })
        })
        .await
    }

    pub async fn get_usage_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        prev_start: DateTime<Utc>,
        prev_end: DateTime<Utc>,
        compare: bool,
    ) -> Result<PeriodPair<UsageStatsDto>, AppError> {
        self.pool.ensure_ready().await?;
        self.pool.check_window(start,end)?;
        if compare { self.pool.check_window(prev_start,prev_end)?; }
        let cache_key = keys::stats_usage(&keys::filter_hash(&StatsPeriodPairKey {
            compare,
            start: format_date_key(start),
            end: format_date_key(end),
            prev_start: format_date_key(prev_start),
            prev_end: format_date_key(prev_end),
        }));

        let cache_key = format!("{cache_key}:scope:{}", self.pool.cache_key());
        get_or_set(&*self.cache, &cache_key, keys::ttl::AGGREGATE, || async {
            Ok(PeriodPair {
                current: self.usage_stats(start, end).await?,
                previous: if compare {
                    Some(self.usage_stats(prev_start, prev_end).await?)
                } else {
                    None
                },
            })
        })
        .await
    }

    async fn revenue_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<RevenueByPaymentMethodDto, AppError> {
        let row: RevenueStatsRow = self.pool.query(
            include_str!("../analytics/queries/stats/revenue_stats_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_one()
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
        self.pool.query(
            include_str!("../analytics/queries/stats/settlement_revenue_totals_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_one()
        .await
    }

    async fn settlement_revenue_by_type(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<SettlementRevenueByTypeRow, AppError> {
        self.pool.query(
            include_str!("../analytics/queries/stats/settlement_revenue_by_type_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_one()
        .await
    }

    async fn transaction_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<TransactionStatsDto, AppError> {
        let row: (i64, i64, i64, i64, Option<f64>) = self.pool.query(
            include_str!("../analytics/queries/stats/transaction_stats_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_one()
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
        let row: (i64, i64, i64, Option<i64>, Option<f64>) = self.pool.query(
            include_str!("../analytics/queries/stats/usage_stats_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_one()
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
        let row: (i64, i64, i64, i64, i64) = self.pool.query(
            include_str!("../analytics/queries/stats/user_stats_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_one()
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
        let active: (i64,) = self.pool.query(
            include_str!("../analytics/queries/stats/plan_stats_1.sql"),
        )
        .fetch_one()
        .await?;
        let expired: (i64,) = self.pool.query(
            include_str!("../analytics/queries/stats/plan_stats_2.sql"),
        )
        .fetch_one()
        .await?;

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
        let row: (i64, i64) = self.pool.query(
            include_str!("../analytics/queries/stats/device_stats_1.sql"),
        )
        .fetch_one()
        .await?;

        let utilization_rows: Vec<DeviceUtilizationRow> = self.pool.query(
            include_str!("../analytics/queries/stats/device_stats_2.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_all()
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
        let top_plans: Vec<TopPlanRow> = self.pool.query(
            include_str!("../analytics/queries/stats/top_performers_stats_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_all()
        .await?;

        let top_players: Vec<TopPlayerRow> = self.pool.query(
            include_str!("../analytics/queries/stats/top_performers_stats_2.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_all()
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
        let rows: Vec<RevenueTrendRow> = self.pool.query(
            include_str!("../analytics/queries/stats/revenue_trend_stats_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_all()
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
        self.pool.query(
            include_str!("../analytics/queries/stats/settlement_trend_stats_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_all()
        .await
    }

    async fn staff_player_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<StaffPlayerStatsDto, AppError> {
        let row: (i64, i64) = self.pool.query(
            include_str!("../analytics/queries/stats/staff_player_stats_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_one()
        .await?;

        Ok(StaffPlayerStatsDto {
            active_players: row.0,
            new_players_in_period: row.1,
        })
    }

    async fn staff_device_stats(&self) -> Result<StaffDeviceStatsDto, AppError> {
        let row: (i64, i64, i64) = self.pool.query(
            include_str!("../analytics/queries/stats/staff_device_stats_1.sql"),
        )
        .fetch_one()
        .await?;

        Ok(StaffDeviceStatsDto {
            total: row.0,
            available: row.1,
            in_use: row.2,
        })
    }

    pub async fn get_finance_reconciliation_stats(
        &self,
        start_date: Option<String>,
        end_date: Option<String>,
        compare: bool,
    ) -> Result<FinanceReconciliationStatsDto, AppError> {
        self.pool.ensure_ready().await?;
        let (period_start, period_end) = self.resolve_stats_period(start_date, end_date);
        let (prev_start, prev_end) = previous_window(period_start, period_end, self.pool.timezone());
        self.pool.check_window(period_start,period_end)?;
        self.pool.check_window(prev_start,prev_end)?;

        let cache_key = keys::stats_dashboard(&keys::filter_hash(&StatsFinanceKey {
            start: format_date_key(period_start),
            end: format_date_key(period_end),
            compare,
        }));
        let cache_key = format!("{cache_key}:finance-recon");

        let cache_key = format!("{cache_key}:scope:{}", self.pool.cache_key());
        get_or_set(&*self.cache, &cache_key, keys::ttl::AGGREGATE, || async {
            let current = self
                .finance_reconciliation_metrics(period_start, period_end)
                .await?;
            let previous = if compare {
                Some(
                    self.finance_reconciliation_metrics(prev_start, prev_end)
                        .await?,
                )
            } else {
                None
            };
            Ok(FinanceReconciliationStatsDto {
                period: period_dto(period_start, period_end, compare, prev_start, prev_end),
                metrics: PeriodPair { current, previous },
            })
        })
        .await
    }

    async fn finance_reconciliation_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<FinanceReconciliationMetricsDto, AppError> {
        let row: (i64, i64, i64, i64, f64) = self.pool.query(
            include_str!("../analytics/queries/stats/finance_reconciliation_metrics_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_one()
        .await?;

        Ok(FinanceReconciliationMetricsDto {
            open_count: row.0,
            closed_count: row.1,
            reconciled_count: row.2,
            pending_reconcile_count: row.3,
            total_deposited: row.4,
        })
    }

    pub async fn get_finance_deposit_stats(
        &self,
        start_date: Option<String>,
        end_date: Option<String>,
        compare: bool,
    ) -> Result<FinanceDepositStatsDto, AppError> {
        self.pool.ensure_ready().await?;
        let (period_start, period_end) = self.resolve_stats_period(start_date, end_date);
        let (prev_start, prev_end) = previous_window(period_start, period_end, self.pool.timezone());
        self.pool.check_window(period_start,period_end)?;
        self.pool.check_window(prev_start,prev_end)?;

        let cache_key = keys::stats_dashboard(&keys::filter_hash(&StatsFinanceKey {
            start: format_date_key(period_start),
            end: format_date_key(period_end),
            compare,
        }));
        let cache_key = format!("{cache_key}:finance-deposits");

        let cache_key = format!("{cache_key}:scope:{}", self.pool.cache_key());
        get_or_set(&*self.cache, &cache_key, keys::ttl::AGGREGATE, || async {
            let current = self
                .finance_deposit_metrics(period_start, period_end)
                .await?;
            let previous = if compare {
                Some(self.finance_deposit_metrics(prev_start, prev_end).await?)
            } else {
                None
            };
            Ok(FinanceDepositStatsDto {
                period: period_dto(period_start, period_end, compare, prev_start, prev_end),
                metrics: PeriodPair { current, previous },
            })
        })
        .await
    }

    async fn finance_deposit_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<FinanceDepositMetricsDto, AppError> {
        let row: (i64, f64, i64, f64, i64, f64, f64, f64) = self.pool.query(
            include_str!("../analytics/queries/stats/finance_deposit_metrics_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_one()
        .await?;

        Ok(FinanceDepositMetricsDto {
            pending_count: row.0,
            pending_amount: row.1,
            approved_count: row.2,
            approved_amount: row.3,
            rejected_count: row.4,
            rejected_amount: row.5,
            bank_amount: row.6,
            home_amount: row.7,
        })
    }

    pub async fn get_finance_variance_stats(
        &self,
        start_date: Option<String>,
        end_date: Option<String>,
        compare: bool,
    ) -> Result<FinanceVarianceStatsDto, AppError> {
        self.pool.ensure_ready().await?;
        let (period_start, period_end) = self.resolve_stats_period(start_date, end_date);
        let (prev_start, prev_end) = previous_window(period_start, period_end, self.pool.timezone());
        self.pool.check_window(period_start,period_end)?;
        self.pool.check_window(prev_start,prev_end)?;

        let cache_key = keys::stats_dashboard(&keys::filter_hash(&StatsFinanceKey {
            start: format_date_key(period_start),
            end: format_date_key(period_end),
            compare,
        }));
        let cache_key = format!("{cache_key}:finance-variance");

        let cache_key = format!("{cache_key}:scope:{}", self.pool.cache_key());
        get_or_set(&*self.cache, &cache_key, keys::ttl::AGGREGATE, || async {
            let current = self
                .finance_variance_metrics(period_start, period_end)
                .await?;
            let previous = if compare {
                Some(self.finance_variance_metrics(prev_start, prev_end).await?)
            } else {
                None
            };
            let registers = self
                .finance_variance_registers(period_start, period_end)
                .await?;
            Ok(FinanceVarianceStatsDto {
                period: period_dto(period_start, period_end, compare, prev_start, prev_end),
                metrics: PeriodPair { current, previous },
                registers,
            })
        })
        .await
    }

    async fn finance_variance_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<FinanceVarianceMetricsDto, AppError> {
        let row: (f64, f64, i64, i64, i64, i64) = self.pool.query(
            include_str!("../analytics/queries/stats/finance_variance_metrics_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_one()
        .await?;

        Ok(FinanceVarianceMetricsDto {
            total_variance: row.0,
            average_variance: row.1,
            over_count: row.2,
            short_count: row.3,
            even_count: row.4,
            register_count: row.5,
        })
    }

    async fn finance_variance_registers(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<FinanceVarianceRegisterRow>, AppError> {
        Ok(self.pool.query::<FinanceVarianceRegisterRow>(
            include_str!("../analytics/queries/stats/finance_variance_registers_1.sql"),
        )
        .bind(start)
        .bind(end)
        .fetch_all()
        .await?)
    }
}

fn previous_window(
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    zone: chrono_tz::Tz,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let diff_days = (period_end.with_timezone(&zone).date_naive()
        - period_start.with_timezone(&zone).date_naive()).num_days().max(1);
    let shift = |instant: DateTime<Utc>| {
        let local = instant.with_timezone(&zone);
        let mut target = local.naive_local() - Duration::days(diff_days);
        use chrono::TimeZone;
        // Choose the first occurrence in a fold and advance to the first valid
        // instant in a gap, preserving calendar time across DST transitions.
        for _ in 0..=86400 {
            if let Some(value) = zone.from_local_datetime(&target).earliest() {
                return value.with_timezone(&Utc);
            }
            target += Duration::seconds(1);
        }
        instant - Duration::days(diff_days)
    };
    (shift(period_start), shift(period_end))
}

fn period_dto(
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    compare: bool,
    prev_start: DateTime<Utc>,
    prev_end: DateTime<Utc>,
) -> PeriodDto {
    PeriodDto {
        start_date: crate::time::utc_timestamp(&period_start),
        end_date: crate::time::utc_timestamp(&period_end),
        label: format!(
            "{} - {}",
            period_start.format("%Y-%m-%d"),
            period_end.format("%Y-%m-%d")
        ),
        previous_label: if compare {
            format!(
                "{} - {}",
                prev_start.format("%Y-%m-%d"),
                prev_end.format("%Y-%m-%d")
            )
        } else {
            String::new()
        },
    }
}

fn format_date_key(dt: DateTime<Utc>) -> String {
    crate::time::utc_timestamp(&dt)
}

fn parse_date_start(value: Option<&str>, zone: chrono_tz::Tz) -> Option<DateTime<Utc>> {
    let value = value?;
    DateTime::parse_from_rfc3339(value).ok().map(|t| t.with_timezone(&Utc)).or_else(|| {
        let date = chrono::NaiveDate::parse_from_str(value,"%Y-%m-%d").ok()?;
        crate::analytics::calendar::boundary(date,zone).ok()
    })
}
fn parse_date_end(value: Option<&str>, zone: chrono_tz::Tz) -> Option<DateTime<Utc>> {
    let value = value?;
    DateTime::parse_from_rfc3339(value).ok().map(|t| t.with_timezone(&Utc)).or_else(|| {
        let date = chrono::NaiveDate::parse_from_str(value,"%Y-%m-%d").ok()?.succ_opt()?;
        crate::analytics::calendar::boundary(date,zone).ok()?.checked_sub_signed(Duration::seconds(1))
    })
}
fn start_of_day(dt: DateTime<Utc>, zone: chrono_tz::Tz) -> DateTime<Utc> {
    crate::analytics::calendar::boundary(dt.with_timezone(&zone).date_naive(),zone).unwrap_or(dt)
}

#[cfg(test)]
mod calendar_tests {
    use super::*;
    #[test]
    fn comparisons_preserve_local_clock_across_dst_and_legacy_month_offset() {
        let parse = |s: &str| DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc);
        let (start,end) = previous_window(parse("2026-03-09T00:00:00-04:00"), parse("2026-03-09T23:59:59-04:00"), chrono_tz::America::New_York);
        assert_eq!(start,parse("2026-03-08T00:00:00-05:00"));
        assert_eq!(end,parse("2026-03-08T23:59:59-04:00"));
        let (start,end) = previous_window(parse("2026-09-01T00:00:00+05:30"), parse("2026-09-30T23:59:59+05:30"), chrono_tz::Asia::Kolkata);
        assert_eq!(start,parse("2026-08-03T00:00:00+05:30"));
        assert_eq!(end,parse("2026-09-01T23:59:59+05:30"));
    }
}
