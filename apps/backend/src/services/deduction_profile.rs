use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
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
        let mut ratio = ratio_at_time(local.time(), profile);
        let rule_local = to_local_datetime(
            cursor,
            profile.policy_timezone.as_deref().unwrap_or(cafe_tz),
        );
        let mut rules: Vec<_> = profile
            .policy_rules
            .iter()
            .filter(|rule| {
                rule.target == crate::models::PricingTarget::Deduction
                    && crate::services::PricingPolicyService::matches(
                        rule,
                        None,
                        None,
                        cursor,
                        rule_local.time(),
                        rule_local.weekday().number_from_monday() as u8,
                    )
            })
            .collect();
        rules.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        for rule in rules {
            match &rule.action {
                crate::models::PricingAction::Fixed { value } => {
                    ratio = value.parse::<f64>().expect("validated deduction speed")
                }
                crate::models::PricingAction::Multiplier { value } => {
                    ratio *= value
                        .parse::<f64>()
                        .expect("validated deduction multiplier")
                }
            }
        }
        let mut next_minute =
            cursor.with_second(0).unwrap().with_nanosecond(0).unwrap() + Duration::minutes(1);
        for time in profile
            .policy_rules
            .iter()
            .flat_map(|r| [r.start_time, r.end_time])
            .flatten()
        {
            let until = (time.num_seconds_from_midnight() as i64
                - rule_local.time().num_seconds_from_midnight() as i64)
                .rem_euclid(86400);
            let boundary = cursor.with_nanosecond(0).unwrap() + Duration::seconds(until);
            if boundary > cursor && boundary < next_minute {
                next_minute = boundary;
            }
        }
        for boundary in profile
            .policy_rules
            .iter()
            .flat_map(|r| [r.starts_at, r.ends_at])
            .flatten()
        {
            if boundary > cursor && boundary < next_minute {
                next_minute = boundary;
            }
        }
        let segment_end = if next_minute > end { end } else { next_minute };
        let secs = segment_end.signed_duration_since(cursor).num_milliseconds() as f64 / 1000.0;
        total += (secs / 60.0) * ratio;
        cursor = segment_end;
    }
    (total * 1e9).round() / 1e9
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
            policy_rules: vec![],
            policy_timezone: None,
        }
    }

    #[test]
    fn published_speed_rules_preserve_plan_speed_and_time_boundaries() {
        let mut profile = sample_profile();
        let rule: crate::models::PricingRule = serde_json::from_value(serde_json::json!({ "id": "night", "name": "night", "priority": 100, "target": "deduction", "deviceTypes": [], "weekdays": [4, 5], "startTime": "23:00:00", "endTime": "08:00:00", "action": { "type": "multiplier", "value": "1.25" } })).unwrap();
        profile.policy_rules = vec![rule];
        let start = Utc.with_ymd_and_hms(2026, 9, 24, 18, 0, 0).unwrap();
        assert!(
            (weighted_minutes_between(
                start,
                start + Duration::minutes(60),
                &profile,
                "Asia/Kolkata"
            ) - 75.0)
                .abs()
                < 1e-8
        );
        let boundary = Utc.with_ymd_and_hms(2026, 9, 24, 17, 29, 30).unwrap();
        assert!(
            (weighted_minutes_between(
                boundary,
                boundary + Duration::minutes(1),
                &profile,
                "Asia/Kolkata"
            ) - 1.375)
                .abs()
                < 1e-8
        );
        profile.policy_rules[0].start_time =
            Some(chrono::NaiveTime::from_hms_opt(22, 59, 45).unwrap());
        assert!(
            (weighted_minutes_between(
                boundary,
                boundary + Duration::minutes(1),
                &profile,
                "Asia/Kolkata"
            ) - 1.46875)
                .abs()
                < 1e-8
        );
        profile.policy_rules[0].start_time = None;
        profile.policy_rules[0].end_time = None;
        profile.policy_rules[0].action = crate::models::PricingAction::Fixed {
            value: "1.2".into(),
        };
        assert_eq!(
            weighted_minutes_between(
                start,
                start + Duration::minutes(60),
                &profile,
                "Asia/Kolkata"
            )
            .ceil(),
            72.0
        );
        let excluded = Utc.with_ymd_and_hms(2026, 9, 26, 18, 30, 0).unwrap();
        assert!(
            (weighted_minutes_between(
                excluded,
                excluded + Duration::minutes(60),
                &profile,
                "Asia/Kolkata"
            ) - 60.0)
                .abs()
                < 1e-8
        );
    }

    #[test]
    fn snapshot_retains_policy_timezone_separately_from_plan_timezone() {
        let mut profile = sample_profile();
        profile.policy_timezone = Some("UTC".into());
        profile.policy_rules = vec![serde_json::from_value(serde_json::json!({"id":"day","name":"day","target":"deduction","priority":100,"weekdays":[],"startTime":"12:00:00","endTime":"13:00:00","action":{"type":"multiplier","value":"1.25"}})).unwrap()];
        let start = Utc.with_ymd_and_hms(2026, 9, 24, 12, 30, 0).unwrap();
        assert_eq!(
            weighted_minutes_between(
                start,
                start + Duration::minutes(1),
                &profile,
                "Asia/Kolkata"
            ),
            1.875
        );
    }

    #[test]
    fn low_window_slower_burn() {
        let profile = sample_profile();
        // 08:00 IST = 02:30 UTC on a winter day — use fixed offset via Asia/Kolkata
        let start = Utc.with_ymd_and_hms(2026, 6, 7, 2, 30, 0).unwrap();
        let end = start + Duration::minutes(60);
        let weighted = weighted_minutes_between(start, end, &profile, "Asia/Kolkata");
        assert!(
            (weighted - 48.0).abs() < 0.1,
            "expected ~48 wallet min, got {weighted}"
        );
    }

    #[test]
    fn peak_window_faster_burn() {
        let profile = sample_profile();
        let start = Utc.with_ymd_and_hms(2026, 6, 7, 13, 0, 0).unwrap();
        let end = start + Duration::minutes(60);
        let weighted = weighted_minutes_between(start, end, &profile, "Asia/Kolkata");
        assert!(
            (weighted - 90.0).abs() < 0.1,
            "expected ~90 wallet min, got {weighted}"
        );
    }

    #[test]
    fn normal_window_one_to_one() {
        let profile = sample_profile();
        let start = Utc.with_ymd_and_hms(2026, 6, 7, 8, 0, 0).unwrap();
        let end = start + Duration::minutes(30);
        let weighted = weighted_minutes_between(start, end, &profile, "Asia/Kolkata");
        assert!(
            (weighted - 30.0).abs() < 0.1,
            "expected ~30 wallet min, got {weighted}"
        );
    }
}
