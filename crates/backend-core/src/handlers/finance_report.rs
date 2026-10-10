use crate::{app::AppState, dto::ApiResult, error::AppError, middleware::AdminUser};
use axum::extract::{Query, State};
use chrono::{Duration, NaiveDate};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportQuery {
    pub venue_location_id: Option<uuid::Uuid>,
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
    headers: axum::http::HeaderMap,
) -> ApiResult<Value> {
    let scope = crate::access::scope::report_scope_tenant(
        state.business_db(&claims).await?,
        &claims,
        &headers,
        query.venue_location_id,
        "finance:read",
    )
    .await?;
    let (start,until) = bounds(&query)?;
    let location_label = if scope.is_none() { "All locations" } else { "Selected locations" };
    let reader = state.report_reader(state.business_db(&claims).await?,scope).await?;
    let start_at = start.and_hms_opt(0,0,0).unwrap().and_utc();
    let until_at = until.and_hms_opt(0,0,0).unwrap().and_utc();
    reader.check_window(start_at,until_at)?;
    let mut report = reader.finance_report(start_at,until_at).await?;
    report["startDate"] = serde_json::json!(query.start_date);
    report["endDate"] = serde_json::json!(query.end_date);
    report["timezone"] = serde_json::json!("UTC");
    report["currency"] = serde_json::json!("INR");
    report["locationLabel"] = serde_json::json!(location_label);
    crate::dto::ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn date_range_is_inclusive_and_bounded() {
        let q = |a: &str, b: &str| ReportQuery {
            venue_location_id: None,
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
