//! One tenant-calendar splitter for closed sessions and live report clipping.
use crate::error::AppError;
use chrono::{DateTime, Datelike, NaiveDate, Offset, Timelike, Utc};
use chrono_tz::Tz;
#[derive(Debug, Clone)]
pub struct HourBucket {
    pub hour_start: DateTime<Utc>,
    pub local_date: NaiveDate,
    pub weekday: u32,
    pub local_hour: u32,
    pub occupied_seconds: i64,
    pub is_start_hour: bool,
}
fn instant(second: i64) -> Result<DateTime<Utc>, AppError> {
    DateTime::from_timestamp(second, 0)
        .ok_or_else(|| AppError::Internal("Hour boundary outside timestamp range".into()))
}
fn label(time: DateTime<Utc>, zone: Tz) -> (NaiveDate, u32, i32) {
    let t = time.with_timezone(&zone);
    (t.date_naive(), t.hour(), t.offset().fix().local_minus_utc())
}
fn boundary(
    mut lo: i64,
    mut hi: i64,
    zone: Tz,
    key: (NaiveDate, u32, i32),
    entering: bool,
) -> Result<i64, AppError> {
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        let same = label(instant(mid)?, zone) == key;
        if same == entering {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Ok(hi)
}
/// Boundaries use local hour/date AND offset, so repeated DST hours have distinct
/// UTC keys. Non-hour offsets and half-hour DST transitions are handled identically.
/// Whole-second clipping matches date_diff('second', ...) in report definitions.
pub fn split(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    zone: Tz,
) -> Result<Vec<HourBucket>, AppError> {
    if end < start {
        return Err(AppError::Internal("Session ends before it starts".into()));
    }
    let mut cursor = start.timestamp();
    let end = end.timestamp();
    let mut rows = Vec::new();
    loop {
        let key = label(instant(cursor)?, zone);
        let hour_start = boundary(
            cursor
                .checked_sub(3600)
                .ok_or_else(|| AppError::Internal("Hour underflow".into()))?,
            cursor,
            zone,
            key,
            true,
        )?;
        let next = boundary(
            cursor,
            cursor
                .checked_add(3600)
                .ok_or_else(|| AppError::Internal("Hour overflow".into()))?,
            zone,
            key,
            false,
        )?;
        let stop = end.min(next);
        rows.push(HourBucket {
            hour_start: instant(hour_start)?,
            local_date: key.0,
            weekday: key.0.weekday().number_from_monday(),
            local_hour: key.1,
            occupied_seconds: stop - cursor,
            is_start_hour: rows.is_empty(),
        });
        if stop >= end {
            break;
        }
        cursor = stop;
    }
    Ok(rows)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn t(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }
    #[test]
    fn ist_midnight_and_half_hour_boundaries() {
        let r = split(
            t("2026-10-01T18:20:00Z"),
            t("2026-10-01T19:10:00Z"),
            chrono_tz::Asia::Kolkata,
        )
        .unwrap();
        assert_eq!(
            r.iter()
                .map(|r| (r.local_date.to_string(), r.local_hour, r.occupied_seconds))
                .collect::<Vec<_>>(),
            vec![
                ("2026-10-01".into(), 23, 600),
                ("2026-10-02".into(), 0, 2400)
            ]
        );
        assert!(r[0].is_start_hour);
        assert!(!r[1].is_start_hour);
        assert_eq!(r[1].hour_start, t("2026-10-01T18:30:00Z"));
    }
    #[test]
    fn repeated_and_skipped_dst_hours_are_distinct() {
        let r = split(
            t("2026-11-01T05:30:00Z"),
            t("2026-11-01T07:30:00Z"),
            chrono_tz::America::New_York,
        )
        .unwrap();
        assert_eq!(
            r.iter().map(|r| r.local_hour).collect::<Vec<_>>(),
            vec![1, 1, 2]
        );
        assert_ne!(r[0].hour_start, r[1].hour_start);
        assert_eq!(r.iter().map(|r| r.occupied_seconds).sum::<i64>(), 7200);
        let r = split(
            t("2026-03-08T06:30:00Z"),
            t("2026-03-08T07:30:00Z"),
            chrono_tz::America::New_York,
        )
        .unwrap();
        assert_eq!(
            r.iter().map(|r| r.local_hour).collect::<Vec<_>>(),
            vec![1, 3]
        );
    }
    #[test]
    fn half_hour_dst_transition_and_zero_length() {
        let r = split(
            t("2026-04-04T14:40:00Z"),
            t("2026-04-04T15:40:00Z"),
            chrono_tz::Australia::Lord_Howe,
        )
        .unwrap();
        assert_eq!(r.iter().map(|r| r.occupied_seconds).sum::<i64>(), 3600);
        assert_eq!(r.len(), 3);
        assert_eq!(r[1].hour_start, t("2026-04-04T15:00:00Z"));
        let r = split(
            t("2026-10-01T00:00:00Z"),
            t("2026-10-01T00:00:00Z"),
            chrono_tz::UTC,
        )
        .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].occupied_seconds, 0);
        assert!(split(
            t("2026-10-02T00:00:00Z"),
            t("2026-10-01T00:00:00Z"),
            chrono_tz::UTC
        )
        .is_err());
    }
}
