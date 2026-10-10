use chrono::{NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::parse_time;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeductionProfile {
    pub peak_window_start: String,
    pub peak_window_end: String,
    pub peak_ratio: f64,
    pub low_window_start: String,
    pub low_window_end: String,
    pub low_ratio: f64,
    /// Published rules captured for this session, already filtered to its device.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_rules: Vec<crate::models::PricingRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_timezone: Option<String>,
}

impl DeductionProfile {
    pub fn normal() -> Self {
        Self {
            peak_window_start: "00:00:00".into(),
            peak_window_end: "00:00:00".into(),
            peak_ratio: 1.0,
            low_window_start: "00:00:00".into(),
            low_window_end: "00:00:00".into(),
            low_ratio: 1.0,
            policy_rules: vec![],
            policy_timezone: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.policy_rules.is_empty() {
            return Err(
                "Policy rules are configured in pricing policies, not plan profiles".into(),
            );
        }
        for field in [
            &self.peak_window_start,
            &self.peak_window_end,
            &self.low_window_start,
            &self.low_window_end,
        ] {
            if parse_time(field).is_err() {
                return Err(format!("Invalid time format: {field}"));
            }
        }
        if self.peak_ratio <= 1.0 {
            return Err("peakRatio must be greater than 1".to_string());
        }
        if self.low_ratio <= 0.0 || self.low_ratio >= 1.0 {
            return Err("lowRatio must be between 0 and 1 (exclusive)".to_string());
        }
        if windows_overlap(self) {
            return Err("Peak and low windows must not overlap".to_string());
        }
        Ok(())
    }
}

pub fn minute_in_window(minute_of_day: i32, start: &str, end: &str) -> bool {
    let Ok(start_t) = parse_time(start) else {
        return false;
    };
    let Ok(end_t) = parse_time(end) else {
        return false;
    };
    time_in_window(minute_of_day, start_t, end_t)
}

fn minute_of_day(time: NaiveTime) -> i32 {
    time.hour() as i32 * 60 + time.minute() as i32
}

fn time_in_window(minute: i32, start: NaiveTime, end: NaiveTime) -> bool {
    let s = minute_of_day(start);
    let e = minute_of_day(end);
    if s == e {
        return false;
    }
    if s < e {
        minute >= s && minute < e
    } else {
        minute >= s || minute < e
    }
}

pub fn ratio_at_time(local_time: NaiveTime, profile: &DeductionProfile) -> f64 {
    let minute = minute_of_day(local_time);
    if minute_in_window(minute, &profile.peak_window_start, &profile.peak_window_end) {
        return profile.peak_ratio;
    }
    if minute_in_window(minute, &profile.low_window_start, &profile.low_window_end) {
        return profile.low_ratio;
    }
    1.0
}

pub fn windows_overlap(profile: &DeductionProfile) -> bool {
    for minute in 0..(24 * 60) {
        if minute_in_window(minute, &profile.peak_window_start, &profile.peak_window_end)
            && minute_in_window(minute, &profile.low_window_start, &profile.low_window_end)
        {
            return true;
        }
    }
    false
}
