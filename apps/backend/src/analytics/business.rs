//! Aggregated business reporting. Every fact is read from ClickHouse; no ledger fallback.
use super::{query_as, ClickHouse};
use crate::error::AppError;
use chrono::{DateTime, Duration, FixedOffset, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

macro_rules! row {
    ($name:ident { $($field:ident : $kind:ty),* $(,)? }) => {
        #[derive(Debug, Serialize, Deserialize, ToSchema)]
        #[serde(rename_all = "camelCase")]
        pub struct $name { $(pub $field: $kind),* }
    }
}
row!(SalesDay {
    date: String,
    revenue: f64,
    plan_revenue: f64,
    pos_revenue: f64,
    transactions: u64,
    buyers: u64
});
row!(UsageHour {
    date: String,
    weekday: u8,
    hour: u8,
    hours: f64,
    starts: u64
});
row!(Station {
    id: String,
    name: String,
    location: String,
    status: String,
    sessions: u64,
    hours: f64
});
row!(CustomerSummary {
    visitors: u64,
    new_visitors: u64,
    repeat_visitors: u64,
    frequent_visitors: u64,
    at_risk: u64,
    prior_visitors: u64,
    retained_visitors: u64
});
row!(Customer {
    id: String,
    name: String,
    visits: u64,
    first_visit: String,
    last_visit: String,
    spend: f64,
    days_absent: i64
});
row!(PlanSales {
    id: String,
    name: String,
    purchases: u64,
    buyers: u64,
    repeat_buyers: u64,
    revenue: f64,
    hours_sold: f64
});
row!(WalletSummary {
    holders: u64,
    active_wallets: u64,
    expiring_7_days: u64,
    expired_wallets: u64,
    remaining_hours: f64
});
row!(ProductSales {
    id: String,
    name: String,
    quantity: i64,
    revenue: f64,
    transactions: u64
});
row!(PosSummary {
    revenue: f64,
    transactions: u64,
    visiting_customers: u64,
    attached_customers: u64
});
row!(StaffPerformance {
    id: String,
    name: String,
    shift_hours: f64,
    revenue: f64,
    transactions: u64,
    sessions: u64
});
row!(BusinessPeriod {
    start_date: String,
    end_date: String,
    previous_start_date: String,
    observed_until: String,
    timezone: String
});
row!(BusinessReport {
    period: BusinessPeriod, generated_at: String, daily_sales: Vec<SalesDay>, hourly_usage: Vec<UsageHour>,
    stations: Vec<Station>, customers: CustomerSummary, customer_details: Vec<Customer>, plans: Vec<PlanSales>,
    wallets: WalletSummary, products: Vec<ProductSales>, pos: PosSummary, staff: Vec<StaffPerformance>
});

pub struct Window {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub previous_start: DateTime<Utc>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}
impl Window {
    pub fn new(
        start: Option<&str>,
        end: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<Self, AppError> {
        let ist = FixedOffset::east_opt(19_800).unwrap();
        let today = now.with_timezone(&ist).date_naive();
        let parse = |value: &str| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map_err(|_| AppError::BadRequest("Use calendar dates in YYYY-MM-DD format".into()))
        };
        let start_date = start
            .map(parse)
            .transpose()?
            .unwrap_or(today - Duration::days(29));
        let end_date = end.map(parse).transpose()?.unwrap_or(today);
        let days = (end_date - start_date).num_days() + 1;
        if !(1..=366).contains(&days) || start_date > today || end_date > today {
            return Err(AppError::BadRequest(
                "Choose 1–366 days ending on or before today (IST)".into(),
            ));
        }
        let midnight = |d: NaiveDate| {
            d.and_hms_opt(0, 0, 0)
                .unwrap()
                .and_local_timezone(ist)
                .unwrap()
                .with_timezone(&Utc)
        };
        let start = midnight(start_date);
        Ok(Self {
            start,
            end: midnight(end_date + Duration::days(1)).min(now),
            previous_start: start - Duration::days(days),
            start_date,
            end_date,
        })
    }
}

const SALES: &str = r#"SELECT toString(toDate("transactionDate",'Asia/Kolkata')),
 sum(amount)::Float64, sumIf(amount,"transactionType"='plan_purchase')::Float64,
 sumIf(amount,"transactionType"='product_purchase')::Float64, count(), uniqExact("playerId")
 FROM transactions WHERE "deletedAt" IS NULL AND "paymentStatus" IN ('completed','credit')
 AND "transactionDate">=$1 AND "transactionDate"<$2 GROUP BY 1 ORDER BY 1"#;

// Split each session at local hour boundaries and clip to the requested interval.
// A session spanning midnight contributes occupied time to both dates, not just its start.
const HOURS: &str = r#"WITH clipped AS (
 SELECT "startTime" AS original_start, greatest("startTime",$1) AS a,
 least(coalesce("endTime",$2),$2) AS b FROM usage_sessions
 WHERE "deletedAt" IS NULL AND "startTime"<$2 AND coalesce("endTime",$2)>$1
 ), buckets AS (SELECT *, addHours(toStartOfHour(a,'Asia/Kolkata'),n) AS h FROM clipped
 ARRAY JOIN range(toUInt64(greatest(0,dateDiff('hour',toStartOfHour(a,'Asia/Kolkata'),toStartOfHour(b-INTERVAL 1 MICROSECOND,'Asia/Kolkata')))+1)) AS n
 WHERE b>a)
 SELECT toString(toDate(h,'Asia/Kolkata')), toDayOfWeek(h,0,'Asia/Kolkata'), toHour(h,'Asia/Kolkata'),
 sum(dateDiff('second',greatest(a,h),least(b,h+INTERVAL 1 HOUR)))/3600.0,
 countIf(original_start>=h AND original_start<h+INTERVAL 1 HOUR)
 FROM buckets GROUP BY 1,2,3 ORDER BY 1,3"#;
const STATIONS: &str = r#"SELECT toString(d.id), d.name, coalesce(nullIf(d.location,''),'Unassigned'), d.status,
 countIf(s."startTime">=$1 AND s."startTime"<$2),
 coalesce(sum(greatest(0,dateDiff('second',greatest(s."startTime",$1),least(coalesce(s."endTime",$2),$2)))),0)/3600.0
 FROM devices d LEFT JOIN usage_sessions s ON s."deviceId"=d.id AND s."deletedAt" IS NULL
 AND s."startTime"<$2 AND coalesce(s."endTime",$2)>$1
 WHERE d."deletedAt" IS NULL GROUP BY d.id,d.name,d.location,d.status ORDER BY 6 DESC,d.name"#;
const CUSTOMER_CTE: &str = r#"WITH visits AS (
 SELECT b."playerId" AS player, min(s."startTime") AS first_visit, max(s."startTime") AS last_visit,
 countIf(s."startTime">=$1) AS visits, countIf(s."startTime">=$3 AND s."startTime"<$1) AS prior
 FROM usage_sessions s INNER JOIN player_plan_balances b ON b.id=s."balanceId"
 WHERE s."deletedAt" IS NULL AND coalesce(b.kind,'time')!='staff_allowance' AND s."startTime"<$2 GROUP BY player
 ) "#;
const CUSTOMER_SUMMARY: &str = r#"SELECT countIf(visits>0), countIf(visits>0 AND first_visit>=$1),
 countIf(visits>0 AND first_visit<$1), countIf(visits>=3),
 countIf(dateDiff('day',last_visit,$2)>=30 AND dateDiff('day',last_visit,$2)<90),
 countIf(prior>0),countIf(prior>0 AND visits>0) FROM visits"#;
const CUSTOMER_DETAILS: &str = r#"SELECT toString(v.player),coalesce(u.username,'Deleted player'),v.visits,
 toString(toDate(v.first_visit,'Asia/Kolkata')),toString(toDate(v.last_visit,'Asia/Kolkata')),
 coalesce(t.spend,0),dateDiff('day',v.last_visit,$2)
 FROM visits v LEFT JOIN users u ON u.id=v.player
 LEFT JOIN (SELECT "playerId",sum(amount)::Float64 AS spend FROM transactions
 WHERE "deletedAt" IS NULL AND "paymentStatus" IN ('completed','credit') AND "transactionDate">=$1 AND "transactionDate"<$2 GROUP BY "playerId") t ON t."playerId"=v.player
 ORDER BY v.last_visit DESC LIMIT 500"#;
const PLANS: &str = r#"SELECT toString(p.id),p.name,sum(t.purchases),count(),countIf(t.purchases>1),sum(t.revenue),
 sum(t.purchases*coalesce(p."timeCredits",0))/60.0
 FROM (SELECT "planId","playerId",count() AS purchases,sum(amount)::Float64 AS revenue FROM transactions
 WHERE "deletedAt" IS NULL AND "paymentStatus" IN ('completed','credit') AND "transactionType"='plan_purchase'
 AND "transactionDate">=$1 AND "transactionDate"<$2 GROUP BY "planId","playerId") t
 INNER JOIN plans p ON p.id=t."planId" GROUP BY p.id,p.name ORDER BY 6 DESC"#;
const WALLETS: &str = r#"SELECT uniqExactIf("playerId",status='active' AND "expiryDate">$1 AND "remainingMinutes">0),
 countIf(status='active' AND "expiryDate">$1 AND "remainingMinutes">0),
 countIf(status='active' AND "expiryDate">$1 AND "expiryDate"<=$1+INTERVAL 7 DAY AND "remainingMinutes">0),
 countIf(status='expired' OR "expiryDate"<=$1),
 coalesce(sumIf("remainingMinutes",status='active' AND "expiryDate">$1 AND "remainingMinutes">0),0)/60.0
 FROM player_plan_balances WHERE "deletedAt" IS NULL AND coalesce(kind,'time')!='staff_allowance'"#;
const PRODUCTS: &str = r#"SELECT toString(p.id),p.name,sum(l.quantity)::Int64,sum(l.quantity*l."unitPrice")::Float64,uniqExact(t.id)
 FROM transaction_products l INNER JOIN transactions t ON t.id=l."transactionId" INNER JOIN products p ON p.id=l."productId"
 WHERE t."deletedAt" IS NULL AND t."paymentStatus" IN ('completed','credit') AND t."transactionDate">=$1 AND t."transactionDate"<$2
 GROUP BY p.id,p.name ORDER BY 4 DESC LIMIT 100"#;
const POS: &str = r#"WITH visitors AS (SELECT DISTINCT b."playerId" AS player FROM usage_sessions s
 INNER JOIN player_plan_balances b ON b.id=s."balanceId" WHERE s."deletedAt" IS NULL AND coalesce(b.kind,'time')!='staff_allowance' AND s."startTime">=$1 AND s."startTime"<$2)
 SELECT coalesce(sum(amount),0)::Float64,count(),(SELECT count() FROM visitors),
 uniqExactIf("playerId","playerId" IN (SELECT player FROM visitors)) FROM transactions
 WHERE "deletedAt" IS NULL AND "paymentStatus" IN ('completed','credit') AND "transactionType"='product_purchase'
 AND "transactionDate">=$1 AND "transactionDate"<$2"#;
const STAFF: &str = r#"WITH hours AS (SELECT "userId" AS actor,
 sum(greatest(0,dateDiff('second',greatest("clockIn",$1),least(coalesce("clockOut",$2),$2))))/3600.0 AS hours
 FROM shifts WHERE "clockIn"<$2 AND coalesce("clockOut",$2)>$1 GROUP BY actor),
 sales AS (SELECT "createdBy" AS actor,sum(amount)::Float64 AS revenue,count() AS transactions FROM transactions
 WHERE "deletedAt" IS NULL AND "paymentStatus" IN ('completed','credit') AND "transactionDate">=$1 AND "transactionDate"<$2 GROUP BY actor),
 sessions AS (SELECT "createdBy" AS actor,count() AS sessions FROM usage_sessions WHERE "deletedAt" IS NULL AND "startTime">=$1 AND "startTime"<$2 GROUP BY actor)
 SELECT toString(u.id),u.username,coalesce(h.hours,0),coalesce(t.revenue,0),coalesce(t.transactions,0),coalesce(s.sessions,0)
 FROM users u LEFT JOIN hours h ON h.actor=u.id LEFT JOIN sales t ON t.actor=u.id LEFT JOIN sessions s ON s.actor=u.id
 WHERE u.role IN ('admin','staff') AND u."deletedAt" IS NULL ORDER BY 4 DESC"#;

impl ClickHouse {
    pub async fn business_report(&self, window: Window) -> Result<BusinessReport, AppError> {
        let ready = self
            .execute(
                "SELECT count() FROM analytics_ready WHERE id=2 FORMAT TabSeparated",
                &[],
            )
            .await?;
        if ready.trim() == "0" {
            return Err(super::client::unavailable(
                "Business analytics projections are still backfilling",
            ));
        }
        let (start, end, prior) = (window.start, window.end, window.previous_start);
        let now = Utc::now();
        let (
            daily_sales,
            hourly_usage,
            stations,
            customers,
            customer_details,
            plans,
            wallets,
            products,
            pos,
            staff,
        ) = tokio::try_join!(
            query_as::<SalesDay>(SALES)
                .bind(prior)
                .bind(end)
                .fetch_all(self),
            query_as::<UsageHour>(HOURS)
                .bind(start)
                .bind(end)
                .fetch_all(self),
            query_as::<Station>(STATIONS)
                .bind(start)
                .bind(end)
                .fetch_all(self),
            query_as::<CustomerSummary>(format!("{CUSTOMER_CTE}{CUSTOMER_SUMMARY}"))
                .bind(start)
                .bind(end)
                .bind(prior)
                .fetch_one(self),
            query_as::<Customer>(format!("{CUSTOMER_CTE}{CUSTOMER_DETAILS}"))
                .bind(start)
                .bind(end)
                .bind(prior)
                .fetch_all(self),
            query_as::<PlanSales>(PLANS)
                .bind(start)
                .bind(end)
                .fetch_all(self),
            query_as::<WalletSummary>(WALLETS).bind(now).fetch_one(self),
            query_as::<ProductSales>(PRODUCTS)
                .bind(start)
                .bind(end)
                .fetch_all(self),
            query_as::<PosSummary>(POS)
                .bind(start)
                .bind(end)
                .fetch_one(self),
            query_as::<StaffPerformance>(STAFF)
                .bind(start)
                .bind(end)
                .fetch_all(self),
        )?;
        Ok(BusinessReport {
            period: BusinessPeriod {
                start_date: window.start_date.to_string(),
                end_date: window.end_date.to_string(),
                previous_start_date: prior
                    .with_timezone(&FixedOffset::east_opt(19_800).unwrap())
                    .date_naive()
                    .to_string(),
                observed_until: end.to_rfc3339(),
                timezone: "Asia/Kolkata".into(),
            },
            generated_at: now.to_rfc3339(),
            daily_sales,
            hourly_usage,
            stations,
            customers,
            customer_details,
            plans,
            wallets,
            products,
            pos,
            staff,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_are_bounded_and_use_ist_calendar_days() {
        let now = "2026-10-03T03:00:00Z".parse().unwrap();
        let w = Window::new(Some("2026-10-01"), Some("2026-10-02"), now).unwrap();
        assert_eq!(w.start.to_rfc3339(), "2026-09-30T18:30:00+00:00");
        assert_eq!(w.end.to_rfc3339(), "2026-10-02T18:30:00+00:00");
        assert_eq!((w.start - w.previous_start).num_days(), 2);
        for (a, b) in [
            ("2026-02-30", "2026-03-01"),
            ("2026-10-03", "2026-10-01"),
            ("2020-01-01", "2026-10-01"),
            ("2026-10-04", "2026-10-04"),
        ] {
            assert!(Window::new(Some(a), Some(b), now).is_err());
        }
        assert_eq!(
            Window::new(Some("2026-10-03"), Some("2026-10-03"), now)
                .unwrap()
                .end,
            now
        );
    }
}
