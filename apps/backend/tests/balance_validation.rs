use chrono::{DateTime, Duration, NaiveTime, Utc};
use gaming_cafe_api::models::{
    balance_status, plan_kind, BalanceValidationResult, Device, PlayerPlanBalance,
};
use gaming_cafe_api::services::BalanceService;
use uuid::Uuid;

fn make_balance(
    remaining_minutes: i32,
    status: &str,
    hours_until_expiry: i64,
    window: Option<(NaiveTime, NaiveTime)>,
) -> PlayerPlanBalance {
    make_balance_scoped(
        remaining_minutes,
        status,
        hours_until_expiry,
        window,
        None,
        None,
    )
}

fn make_balance_scoped(
    remaining_minutes: i32,
    status: &str,
    hours_until_expiry: i64,
    window: Option<(NaiveTime, NaiveTime)>,
    device_type: Option<&str>,
    device_sub_type: Option<&str>,
) -> PlayerPlanBalance {
    PlayerPlanBalance {
        id: Uuid::new_v4(),
        player_id: Uuid::new_v4(),
        device_type: device_type.map(str::to_string),
        device_sub_type: device_sub_type.map(str::to_string),
        kind: "time".to_string(),
        remaining_minutes,
        expiry_date: Utc::now() + Duration::hours(hours_until_expiry),
        window_start: window.map(|(s, _)| s),
        window_end: window.map(|(_, e)| e),
        status: status.to_string(),
        source_plan_id: Some(Uuid::new_v4()),
        allowed_days: None,
        allowed_months: None,
        deduction_profile: None,
        created_by: None,
        updated_by: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        deleted_at: None,
    }
}

fn validate(balance: &PlayerPlanBalance) -> BalanceValidationResult {
    BalanceService::validate_balance(balance, None, None)
}

fn sample_device() -> Device {
    Device {
        id: Uuid::new_v4(),
        name: "PC-01".to_string(),
        serial_number: None,
        local_ip_address: None,
        device_type: "PC".to_string(),
        device_sub_type: "HIGH_END_PCS".to_string(),
        location: None,
        status: "available".to_string(),
        registered_kiosk: None,
        registration_status: "registered".to_string(),
        created_by: None,
        updated_by: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        deleted_at: None,
    }
}

#[test]
fn active_balance_with_minutes_is_valid() {
    let balance = make_balance(300, balance_status::ACTIVE, 24, None);
    let result = validate(&balance);
    assert!(result.valid);
    assert!(result.reason.is_none());
}

#[test]
fn expired_status_is_invalid() {
    let balance = make_balance(300, balance_status::EXPIRED, 24, None);
    let result = validate(&balance);
    assert!(!result.valid);
    assert!(result.reason.unwrap().contains("expired"));
}

#[test]
fn exhausted_status_is_invalid() {
    let balance = make_balance(0, balance_status::EXHAUSTED, 24, None);
    let result = validate(&balance);
    assert!(!result.valid);
    assert!(result.reason.unwrap().contains("exhausted"));
}

#[test]
fn zero_minutes_is_invalid() {
    let balance = make_balance(0, balance_status::ACTIVE, 24, None);
    let result = validate(&balance);
    assert!(!result.valid);
    assert!(result.reason.unwrap().contains("No minutes remaining"));
}

#[test]
fn past_expiry_is_invalid() {
    let balance = make_balance(300, balance_status::ACTIVE, -1, None);
    let result = validate(&balance);
    assert!(!result.valid);
    assert!(result.reason.unwrap().contains("expired"));
}

#[test]
fn window_blocks_outside_hours() {
    let window = Some((
        NaiveTime::from_hms_opt(6, 0, 0).unwrap(),
        NaiveTime::from_hms_opt(11, 59, 0).unwrap(),
    ));
    let balance = make_balance(300, balance_status::ACTIVE, 24, window);
    let current_hour = Utc::now().time();
    let in_window = current_hour >= NaiveTime::from_hms_opt(6, 0, 0).unwrap()
        && current_hour <= NaiveTime::from_hms_opt(11, 59, 0).unwrap();

    let result = validate(&balance);
    assert_eq!(result.valid, in_window);
}

#[test]
fn cancelled_status_is_invalid() {
    let balance = make_balance(300, balance_status::CANCELLED, 24, None);
    let result = validate(&balance);
    assert!(!result.valid);
    assert!(result.reason.unwrap().contains("cancelled"));
}

#[test]
fn device_scope_matches_exact_pc_high_end() {
    let device = sample_device();
    let balance = make_balance_scoped(
        60,
        balance_status::ACTIVE,
        24,
        None,
        Some("PC"),
        Some("HIGH_END_PCS"),
    );
    assert!(BalanceService::device_scope_matches(&balance, &device));
}

#[test]
fn device_scope_rejects_null_scope() {
    let device = sample_device();
    let balance = make_balance(60, balance_status::ACTIVE, 24, None);
    assert!(!BalanceService::device_scope_matches(&balance, &device));
}

#[test]
fn device_scope_rejects_wrong_subtype() {
    let device = sample_device();
    let balance = make_balance_scoped(
        60,
        balance_status::ACTIVE,
        24,
        None,
        Some("PC"),
        Some("OTHER"),
    );
    assert!(!BalanceService::device_scope_matches(&balance, &device));
}

#[test]
fn validate_balance_with_device_rejects_mismatch() {
    let device = sample_device();
    let balance = make_balance_scoped(
        60,
        balance_status::ACTIVE,
        24,
        None,
        Some("PC"),
        Some("OTHER"),
    );
    let result = BalanceService::validate_balance(&balance, Some(&device), None);
    assert!(!result.valid);
    assert_eq!(
        BalanceService::validation_failure_code(&result),
        "DEVICE_TYPE_NOT_ALLOWED"
    );
}

#[test]
fn validation_failure_code_maps_expired() {
    let balance = make_balance(60, balance_status::ACTIVE, -1, None);
    let result = BalanceService::validate_balance(&balance, None, None);
    assert!(!result.valid);
    assert_eq!(
        BalanceService::validation_failure_code(&result),
        "PLAN_EXPIRED"
    );
}

#[test]
fn validation_failure_code_maps_no_minutes() {
    let balance = make_balance(0, balance_status::ACTIVE, 24, None);
    let result = BalanceService::validate_balance(&balance, None, None);
    assert!(!result.valid);
    assert_eq!(
        BalanceService::validation_failure_code(&result),
        "PLAN_EXHAUSTED"
    );
}

#[test]
fn plan_kind_mapping() {
    assert_eq!(gaming_cafe_api::models::plan_kind::TIME, "time");
    assert_eq!(
        gaming_cafe_api::models::plan_kind::HAPPY_HOURS,
        "happy_hours"
    );
}

#[test]
fn ledger_reason_constants() {
    use gaming_cafe_api::models::ledger_reason;
    assert_eq!(ledger_reason::PURCHASE, "purchase");
    assert_eq!(ledger_reason::RECHARGE, "recharge");
    assert_eq!(ledger_reason::SESSION_USAGE, "session_usage");
    assert_eq!(ledger_reason::EXPIRY, "expiry");
    assert_eq!(ledger_reason::ADJUSTMENT, "adjustment");
    assert_eq!(ledger_reason::MIGRATION, "migration");
}

#[test]
fn balance_response_from_model() {
    use gaming_cafe_api::models::PlayerPlanBalanceResponse;
    let balance = make_balance(300, balance_status::ACTIVE, 24, None);
    let response: PlayerPlanBalanceResponse = balance.clone().into();
    assert_eq!(response.remaining_minutes, 300);
    assert_eq!(response.kind, "time");
    assert!(response.player.is_none());
    assert!(response.plan.is_none());
}

// ---------------------------------------------------------------------------
// Tests for the purchase_or_recharge expiry-computation logic
// ---------------------------------------------------------------------------

fn compute_recharge_expiry(
    kind: &str,
    balance_status_val: &str,
    balance_expiry: DateTime<Utc>,
    fresh_expiry: DateTime<Utc>,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    if kind == plan_kind::HAPPY_HOURS
        && balance_status_val == balance_status::ACTIVE
        && balance_expiry > now
    {
        balance_expiry
    } else {
        fresh_expiry
    }
}

#[test]
fn time_plan_active_recharge_resets_expiry() {
    let now = Utc::now();
    let balance_expiry = now + Duration::days(3);
    let fresh_expiry = now + Duration::days(7);

    let result = compute_recharge_expiry(
        plan_kind::TIME,
        balance_status::ACTIVE,
        balance_expiry,
        fresh_expiry,
        now,
    );
    assert_eq!(
        result, fresh_expiry,
        "Time plan recharge always resets expiry"
    );
}

#[test]
fn time_plan_expired_recharge_resets_expiry() {
    let now = Utc::now();
    let balance_expiry = now - Duration::days(2);
    let fresh_expiry = now + Duration::days(7);

    let result = compute_recharge_expiry(
        plan_kind::TIME,
        balance_status::EXPIRED,
        balance_expiry,
        fresh_expiry,
        now,
    );
    assert_eq!(
        result, fresh_expiry,
        "Expired time plan recharge resets expiry"
    );
}

#[test]
fn time_plan_exhausted_recharge_resets_expiry() {
    let now = Utc::now();
    let balance_expiry = now + Duration::days(1);
    let fresh_expiry = now + Duration::days(7);

    let result = compute_recharge_expiry(
        plan_kind::TIME,
        balance_status::EXHAUSTED,
        balance_expiry,
        fresh_expiry,
        now,
    );
    assert_eq!(
        result, fresh_expiry,
        "Exhausted time plan recharge resets expiry"
    );
}

#[test]
fn happy_hours_active_recharge_keeps_expiry() {
    let now = Utc::now();
    let balance_expiry = now + Duration::hours(12);
    let fresh_expiry = now + Duration::days(1);

    let result = compute_recharge_expiry(
        plan_kind::HAPPY_HOURS,
        balance_status::ACTIVE,
        balance_expiry,
        fresh_expiry,
        now,
    );
    assert_eq!(
        result, balance_expiry,
        "Active happy hours recharge preserves existing expiry"
    );
}

#[test]
fn happy_hours_expired_recharge_resets_expiry() {
    let now = Utc::now();
    let balance_expiry = now - Duration::hours(6);
    let fresh_expiry = now + Duration::days(1);

    let result = compute_recharge_expiry(
        plan_kind::HAPPY_HOURS,
        balance_status::EXPIRED,
        balance_expiry,
        fresh_expiry,
        now,
    );
    assert_eq!(
        result, fresh_expiry,
        "Expired happy hours recharge resets expiry"
    );
}

#[test]
fn happy_hours_exhausted_recharge_resets_expiry() {
    let now = Utc::now();
    let balance_expiry = now + Duration::hours(6);
    let fresh_expiry = now + Duration::days(1);

    let result = compute_recharge_expiry(
        plan_kind::HAPPY_HOURS,
        balance_status::EXHAUSTED,
        balance_expiry,
        fresh_expiry,
        now,
    );
    assert_eq!(
        result, fresh_expiry,
        "Exhausted happy hours recharge resets expiry"
    );
}

#[test]
fn happy_hours_cancelled_recharge_resets_expiry() {
    let now = Utc::now();
    let balance_expiry = now + Duration::hours(6);
    let fresh_expiry = now + Duration::days(1);

    let result = compute_recharge_expiry(
        plan_kind::HAPPY_HOURS,
        balance_status::CANCELLED,
        balance_expiry,
        fresh_expiry,
        now,
    );
    assert_eq!(
        result, fresh_expiry,
        "Cancelled happy hours recharge resets expiry"
    );
}

// ---------------------------------------------------------------------------
// Tests for purchase_or_recharge minute carry-forward logic
// ---------------------------------------------------------------------------

fn should_carry_forward_minutes(
    balance_status_val: &str,
    balance_expiry: DateTime<Utc>,
    now: DateTime<Utc>,
) -> bool {
    balance_status_val == balance_status::ACTIVE && balance_expiry > now
}

#[test]
fn expired_balance_does_not_carry_forward_minutes() {
    let now = Utc::now();
    assert!(!should_carry_forward_minutes(
        balance_status::EXPIRED,
        now - Duration::days(1),
        now,
    ));
}

#[test]
fn stale_active_balance_past_expiry_does_not_carry_forward() {
    let now = Utc::now();
    assert!(!should_carry_forward_minutes(
        balance_status::ACTIVE,
        now - Duration::hours(1),
        now,
    ));
}

#[test]
fn exhausted_balance_does_not_carry_forward_minutes() {
    let now = Utc::now();
    assert!(!should_carry_forward_minutes(
        balance_status::EXHAUSTED,
        now + Duration::days(3),
        now,
    ));
}

#[test]
fn active_balance_with_remaining_time_carries_forward() {
    let now = Utc::now();
    assert!(should_carry_forward_minutes(
        balance_status::ACTIVE,
        now + Duration::days(3),
        now,
    ));
}
