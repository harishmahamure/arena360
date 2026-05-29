use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::error::AppError;

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RevenueByPaymentMethodDto {
    pub plan: f64,
    pub merchandise: f64,
    pub total: f64,
    pub cash_revenue: f64,
    pub online_revenue: f64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransactionStatsDto {
    pub total_transactions: i64,
    pub completed_transactions: i64,
    pub pending_transactions: i64,
    pub failed_transactions: i64,
    pub average_transaction_amount: f64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatsDto {
    pub total_sessions: i64,
    pub active_sessions: i64,
    pub completed_sessions: i64,
    pub total_hours: f64,
    pub total_minutes: i64,
    pub average_session_duration: f64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserStatsDto {
    pub total_users: i64,
    pub active_users: i64,
    pub total_players: i64,
    pub active_players: i64,
    pub new_users_this_period: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanStatsDto {
    pub total_active_plans: i64,
    pub total_expired_plans: i64,
    pub plans_by_type: Vec<PlanTypeStat>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PlanTypeStat {
    #[serde(rename = "type")]
    pub plan_type: String,
    pub count: i64,
    pub revenue: f64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatsDto {
    pub total_devices: i64,
    pub active_devices: i64,
    pub device_utilization: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TopPerformersDto {
    pub top_plans: Vec<serde_json::Value>,
    pub top_players: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RevenueTrendDto {
    pub date: String,
    pub cash_revenue: f64,
    pub online_revenue: f64,
    pub total_revenue: f64,
    pub transaction_count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffPlayerStatsDto {
    pub active_players: i64,
    pub new_players_in_period: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffDeviceStatsDto {
    pub total: i64,
    pub available: i64,
    pub in_use: i64,
}

#[derive(Debug, Serialize, ToSchema)]
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

#[derive(Debug, Serialize, ToSchema)]
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

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PeriodDto {
    pub start_date: String,
    pub end_date: String,
    pub label: String,
    pub previous_label: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PeriodPair<T> {
    pub current: T,
    pub previous: T,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PeriodPairRevenueByPaymentMethod {
    pub current: RevenueByPaymentMethodDto,
    pub previous: RevenueByPaymentMethodDto,
}

#[derive(Debug, Serialize, ToSchema)]
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
}

impl StatsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_dashboard_stats(
        &self,
        start_date: Option<String>,
        end_date: Option<String>,
    ) -> Result<DashboardStatsDto, AppError> {
        let now = Utc::now();
        let period_start = start_date
            .and_then(|s| DateTime::parse_from_rfc3339(&format!("{s}T00:00:00Z")).ok())
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|| start_of_month(now));
        let period_end = end_date
            .and_then(|s| DateTime::parse_from_rfc3339(&format!("{s}T23:59:59Z")).ok())
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or(now);

        let diff_days = (period_end - period_start).num_days().max(1);
        let prev_start = period_start - Duration::days(diff_days);
        let prev_end = period_end - Duration::days(diff_days);

        let revenue_current = self.revenue_stats(period_start, period_end).await?;
        let revenue_previous = self.revenue_stats(prev_start, prev_end).await?;
        let tx_current = self.transaction_stats(period_start, period_end).await?;
        let tx_previous = self.transaction_stats(prev_start, prev_end).await?;
        let usage_current = self.usage_stats(period_start, period_end).await?;
        let usage_previous = self.usage_stats(prev_start, prev_end).await?;
        let users = self.user_stats(period_start, period_end).await?;
        let plans = self.plan_stats().await?;
        let devices = self.device_stats().await?;

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
            top_performers: TopPerformersDto {
                top_plans: vec![],
                top_players: vec![],
            },
            revenue_trend: vec![],
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

    pub async fn get_revenue_by_payment_method(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        prev_start: DateTime<Utc>,
        prev_end: DateTime<Utc>,
    ) -> Result<PeriodPair<RevenueByPaymentMethodDto>, AppError> {
        Ok(PeriodPair {
            current: self.revenue_stats(start, end).await?,
            previous: self.revenue_stats(prev_start, prev_end).await?,
        })
    }

    pub async fn get_usage_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        prev_start: DateTime<Utc>,
        prev_end: DateTime<Utc>,
    ) -> Result<PeriodPair<UsageStatsDto>, AppError> {
        Ok(PeriodPair {
            current: self.usage_stats(start, end).await?,
            previous: self.usage_stats(prev_start, prev_end).await?,
        })
    }

    async fn revenue_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<RevenueByPaymentMethodDto, AppError> {
        let row: (Option<f64>, Option<f64>, Option<f64>, Option<f64>) = sqlx::query_as(
            r#"
            SELECT
                COALESCE(SUM(CASE WHEN "transactionType" = 'plan_purchase' THEN amount::float8 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN "transactionType" = 'product_purchase' THEN amount::float8 ELSE 0 END), 0),
                COALESCE(SUM(COALESCE("cashAmount", 0)::float8), 0),
                COALESCE(SUM(COALESCE("onlineAmount", 0)::float8), 0)
            FROM transactions
            WHERE "createdAt" BETWEEN $1 AND $2 AND "deletedAt" IS NULL
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await?;

        let plan = row.0.unwrap_or(0.0);
        let merchandise = row.1.unwrap_or(0.0);
        Ok(RevenueByPaymentMethodDto {
            plan,
            merchandise,
            total: plan + merchandise,
            cash_revenue: row.2.unwrap_or(0.0),
            online_revenue: row.3.unwrap_or(0.0),
        })
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
                COUNT(*) FILTER (WHERE "paymentStatus" = 'completed'),
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

    async fn device_stats(&self) -> Result<DeviceStatsDto, AppError> {
        let row: (i64, i64) = sqlx::query_as(
            r#"
            SELECT COUNT(*), COUNT(*) FILTER (WHERE status IN ('operational', 'available', 'in_use'))
            FROM devices WHERE "deletedAt" IS NULL
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(DeviceStatsDto {
            total_devices: row.0,
            active_devices: row.1,
            device_utilization: vec![],
        })
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

fn start_of_month(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.with_day(1)
        .unwrap_or(dt)
        .with_hour(0)
        .unwrap_or(dt)
        .with_minute(0)
        .unwrap_or(dt)
        .with_second(0)
        .unwrap_or(dt)
        .with_nanosecond(0)
        .unwrap_or(dt)
}
