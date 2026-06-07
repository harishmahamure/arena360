use chrono::{DateTime, Duration, Utc};
use chrono_tz::Tz;

use crate::models::deduction_profile::{ratio_at_time, DeductionProfile};

pub fn to_local_datetime(now: DateTime<Utc>, cafe_tz: &str) -> DateTime<Tz> {
    let tz: Tz = cafe_tz.parse().unwrap_or(chrono_tz::Asia::Kolkata);
    now.with_timezone(&tz)
}

/// Wallet minutes consumed between two UTC instants using the plan profile.
pub fn weighted_minutes_between(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    profile: &DeductionProfile,
    cafe_tz: &str,
) -> f64 {
    if end <= start {
        return 0.0;
    }

    let mut total = 0.0;
    let mut cursor = start;
    while cursor < end {
        let local = to_local_datetime(cursor, cafe_tz);
        let ratio = ratio_at_time(local.time(), profile);
        let next_minute = cursor + Duration::minutes(1);
        let segment_end = if next_minute > end { end } else { next_minute };
        let secs = segment_end
            .signed_duration_since(cursor)
            .num_milliseconds() as f64
            / 1000.0;
        total += (secs / 60.0) * ratio;
        cursor = segment_end;
    }
    total
}

pub fn wall_minutes_between(start: DateTime<Utc>, end: DateTime<Utc>) -> f64 {
    if end <= start {
        return 0.0;
    }
    let duration_ms = end.signed_duration_since(start).num_milliseconds();
    (duration_ms as f64) / (1000.0 * 60.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_profile() -> DeductionProfile {
        DeductionProfile {
            peak_window_start: "18:00:00".to_string(),
            peak_window_end: "23:00:00".to_string(),
            peak_ratio: 1.5,
            low_window_start: "07:00:00".to_string(),
            low_window_end: "11:00:00".to_string(),
            low_ratio: 0.8,
        }
    }

    #[test]
    fn low_window_slower_burn() {
        let profile = sample_profile();
        // 08:00 IST = 02:30 UTC on a winter day — use fixed offset via Asia/Kolkata
        let start = Utc.with_ymd_and_hms(2026, 6, 7, 2, 30, 0).unwrap();
        let end = start + Duration::minutes(60);
        let weighted = weighted_minutes_between(start, end, &profile, "Asia/Kolkata");
        assert!((weighted - 48.0).abs() < 0.1, "expected ~48 wallet min, got {weighted}");
    }

    #[test]
    fn peak_window_faster_burn() {
        let profile = sample_profile();
        let start = Utc.with_ymd_and_hms(2026, 6, 7, 13, 0, 0).unwrap();
        let end = start + Duration::minutes(60);
        let weighted = weighted_minutes_between(start, end, &profile, "Asia/Kolkata");
        assert!((weighted - 90.0).abs() < 0.1, "expected ~90 wallet min, got {weighted}");
    }

    #[test]
    fn normal_window_one_to_one() {
        let profile = sample_profile();
        let start = Utc.with_ymd_and_hms(2026, 6, 7, 8, 0, 0).unwrap();
        let end = start + Duration::minutes(30);
        let weighted = weighted_minutes_between(start, end, &profile, "Asia/Kolkata");
        assert!((weighted - 30.0).abs() < 0.1, "expected ~30 wallet min, got {weighted}");
    }
}
