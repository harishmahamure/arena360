use crate::error::AppError;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

/// First valid instant of a tenant calendar date, including skipped midnights/dates.
pub fn boundary(date: NaiveDate, zone: Tz) -> Result<DateTime<Utc>, AppError> {
    let mut naive = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::BadRequest("Invalid calendar boundary".into()))?;
    for _ in 0..=86400 {
        if let Some(t) = zone.from_local_datetime(&naive).earliest() {
            return Ok(t.with_timezone(&Utc));
        }
        naive += chrono::Duration::seconds(1);
    }
    Err(AppError::BadRequest(
        "Unresolvable calendar boundary".into(),
    ))
}
