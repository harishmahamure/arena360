//! Aggregated tenant business reporting from an authorized database snapshot.
use super::report_reader::ReportReader;
use crate::error::AppError;
use chrono::{DateTime, Duration, NaiveDate, Utc};
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
        zone: chrono_tz::Tz,
    ) -> Result<Self, AppError> {
        let today = now.with_timezone(&zone).date_naive();
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
                "Choose 1–366 days ending on or before today in the tenant calendar".into(),
            ));
        }
        let start = super::calendar::boundary(start_date,zone)?;
        Ok(Self {
            start,
            end: super::calendar::boundary(end_date + Duration::days(1),zone)?.min(now),
            previous_start: super::calendar::boundary(start_date - Duration::days(days),zone)?,
            start_date,
            end_date,
        })
    }
}

const SALES: &str = crate::report_sql!("business/sales.sql");

// Split each session at local hour boundaries and clip to the requested interval.
// A session spanning midnight contributes occupied time to both dates, not just its start.
const HOURS: &str = crate::report_sql!("business/hours.sql");
const STATIONS: &str = crate::report_sql!("business/stations.sql");
const CUSTOMER_CTE: &str = crate::report_sql!("business/customer_cte.sql");
const CUSTOMER_SUMMARY: &str = crate::report_sql!("business/customer_summary.sql");
const CUSTOMER_DETAILS: &str = crate::report_sql!("business/customer_details.sql");
const PLANS: &str = crate::report_sql!("business/plans.sql");
const WALLETS: &str = crate::report_sql!("business/wallets.sql");
const PRODUCTS: &str = crate::report_sql!("business/products.sql");
const POS: &str = crate::report_sql!("business/pos.sql");
const STAFF: &str = crate::report_sql!("business/staff.sql");

impl ReportReader {
    #[cfg(feature = "duckdb-analytics")]
    async fn hourly_usage(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<UsageHour>,AppError> {
        let closed: Vec<UsageHour> = self.query(HOURS).bind(start).bind(end).fetch_all().await?;
        let open: Vec<(DateTime<Utc>,)> = self.query("SELECT start_time FROM report_sessions WHERE end_time IS NULL AND start_time < $1").bind(end).fetch_all().await?;
        let mut rows = std::collections::BTreeMap::new();
        for row in closed { rows.insert((row.date.clone(),row.hour),row); }
        for (original,) in open {
            if end <= original.max(start) { continue; }
            for bucket in super::session_hours::split(original.max(start),end,self.timezone())? {
                if bucket.occupied_seconds == 0 { continue; }
                let key = (bucket.local_date.to_string(),bucket.local_hour as u8);
                let row = rows.entry(key.clone()).or_insert(UsageHour {date:key.0,weekday:bucket.weekday as u8,hour:key.1,hours:0.0,starts:0});
                row.hours += bucket.occupied_seconds as f64 / 3600.0;
                if original >= start && bucket.is_start_hour { row.starts += 1; }
            }
        }
        Ok(rows.into_values().collect())
    }
    #[cfg(not(feature = "duckdb-analytics"))]
    async fn hourly_usage(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<UsageHour>, AppError> {
        let sessions: Vec<(DateTime<Utc>, Option<DateTime<Utc>>)> = self.query(HOURS).bind(start).bind(end).fetch_all().await?;
        let mut rows = std::collections::BTreeMap::new();
        for (original, stop) in sessions {
            self.ensure_ready().await?;
            let stop = stop.unwrap_or(end).min(end);
            let clipped = original.max(start);
            if stop <= clipped {continue;}
            for bucket in super::session_hours::split(clipped,stop,self.timezone())? {
                if bucket.occupied_seconds==0 {continue;}
                let key = (bucket.local_date.to_string(),bucket.local_hour as u8);
                let row = rows.entry(key.clone()).or_insert(UsageHour {date:key.0,weekday:bucket.weekday as u8,hour:key.1,hours:0.0,starts:0});
                row.hours += bucket.occupied_seconds as f64/3600.0;
                if bucket.is_start_hour && original>=start && original<end {row.starts+=1;}
            }
        }
        self.ensure_ready().await?;
        Ok(rows.into_values().collect())
    }
    pub async fn business_report(&self, window: Window) -> Result<BusinessReport, AppError> {
        self.ensure_ready().await?;
        self.check_window(window.previous_start,window.end)?;
        let (start, end, prior) = (window.start, window.end, window.previous_start);
        let now = self.now();
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
            self.query::<SalesDay>(SALES)
                .bind(prior)
                .bind(end)
                .fetch_all(),
            self.hourly_usage(start,end),
            self.query::<Station>(STATIONS)
                .bind(start)
                .bind(end)
                .fetch_all(),
            self.query::<CustomerSummary>(format!("{CUSTOMER_CTE}{CUSTOMER_SUMMARY}"))
                .bind(start)
                .bind(end)
                .bind(prior)
                .fetch_one(),
            self.query::<Customer>(format!("{CUSTOMER_CTE}{CUSTOMER_DETAILS}"))
                .bind(start)
                .bind(end)
                .bind(prior)
                .fetch_all(),
            self.query::<PlanSales>(PLANS)
                .bind(start)
                .bind(end)
                .fetch_all(),
            self.query::<WalletSummary>(WALLETS).bind(now).fetch_one(),
            self.query::<ProductSales>(PRODUCTS)
                .bind(start)
                .bind(end)
                .fetch_all(),
            self.query::<PosSummary>(POS)
                .bind(start)
                .bind(end)
                .fetch_one(),
            self.query::<StaffPerformance>(STAFF)
                .bind(start)
                .bind(end)
                .fetch_all(),
        )?;
        Ok(BusinessReport {
            period: BusinessPeriod {
                start_date: window.start_date.to_string(),
                end_date: window.end_date.to_string(),
                previous_start_date: prior
                    .with_timezone(&self.timezone())
                    .date_naive()
                    .to_string(),
                observed_until: crate::time::utc_timestamp(&end),
                timezone: self.timezone().to_string(),
            },
            generated_at: crate::time::utc_timestamp(&now),
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
        let w = Window::new(Some("2026-10-01"), Some("2026-10-02"), now, chrono_tz::Asia::Kolkata).unwrap();
        assert_eq!(crate::time::utc_timestamp(&w.start), "2026-09-30T18:30:00Z");
        assert_eq!(crate::time::utc_timestamp(&w.end), "2026-10-02T18:30:00Z");
        assert_eq!((w.start - w.previous_start).num_days(), 2);
        for (a, b) in [
            ("2026-02-30", "2026-03-01"),
            ("2026-10-03", "2026-10-01"),
            ("2020-01-01", "2026-10-01"),
            ("2026-10-04", "2026-10-04"),
        ] {
            assert!(Window::new(Some(a), Some(b), now, chrono_tz::Asia::Kolkata).is_err());
        }
        assert_eq!(
            Window::new(Some("2026-10-03"), Some("2026-10-03"), now, chrono_tz::Asia::Kolkata)
                .unwrap()
                .end,
            now
        );
    }
}
