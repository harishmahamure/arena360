use crate::{
    app::AppState,
    dto::{ok, ApiResult},
    error::AppError,
    middleware::AdminUser,
};
use axum::extract::{Query, State};
use chrono::{Duration, NaiveDate};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportQuery {
    pub start_date: String,
    pub end_date: String,
}

fn bounds(query: &ReportQuery) -> Result<(NaiveDate, NaiveDate), AppError> {
    let parse = |s: &str| {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| AppError::BadRequest("Use YYYY-MM-DD dates".into()))
    };
    let start = parse(&query.start_date)?;
    let end = parse(&query.end_date)?;
    if start > end || (end - start).num_days() > 365 {
        return Err(AppError::BadRequest(
            "Choose an ordered date range of at most 366 days".into(),
        ));
    }
    let until = end
        .checked_add_signed(Duration::days(1))
        .ok_or_else(|| AppError::BadRequest("Invalid end date".into()))?;
    Ok((start, until))
}

pub async fn report(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Query(query): Query<ReportQuery>,
) -> ApiResult<Value> {
    super::kitchen::require_venue(&claims)?;
    let (start, until) = bounds(&query)?;
    let settings = state
        .config
        .effective(
            crate::models::DEFAULT_ORGANIZATION_ID,
            crate::models::EffectiveSettingsQuery {
                location_id: None,
                category: None,
            },
        )
        .await?;
    let currency = settings
        .iter()
        .find(|s| s.key == "pricing.currency")
        .and_then(|s| s.value.as_str())
        .unwrap_or("INR");
    let mut result = crate::analytics::ClickHouse::from_env()
        .finance_report(
            start.and_hms_opt(0, 0, 0).unwrap().and_utc(),
            until.and_hms_opt(0, 0, 0).unwrap().and_utc(),
        )
        .await?;
    result["currency"] = currency.into();
    result["startDate"] = query.start_date.into();
    result["endDate"] = query.end_date.into();
    result["timezone"] = "UTC".into();
    ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn date_range_is_inclusive_and_bounded() {
        let q = |a: &str, b: &str| ReportQuery {
            start_date: a.into(),
            end_date: b.into(),
        };
        let (a, b) = bounds(&q("2026-10-02", "2026-10-02")).unwrap();
        assert_eq!((b - a).num_days(), 1);
        assert!(bounds(&q("2026-10-03", "2026-10-02")).is_err());
        assert!(bounds(&q("2025-01-01", "2026-01-02")).is_err());
        assert!(bounds(&q("2026-02-30", "2026-03-01")).is_err());
    }
}
