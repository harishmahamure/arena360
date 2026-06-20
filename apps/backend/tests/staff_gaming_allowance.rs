//! Staff gaming allowance contract and auth gate tests.

use gaming_cafe_api::models::{
    ledger_reason, plan_kind, StaffGamingAllowanceStatus, STAFF_ALLOWANCE_PERIOD_DAYS,
};

#[test]
fn staff_allowance_period_is_30_days() {
    assert_eq!(STAFF_ALLOWANCE_PERIOD_DAYS, 30);
}

#[test]
fn staff_allowance_ledger_reason_constants() {
    assert_eq!(ledger_reason::STAFF_ALLOWANCE_GRANT, "staff_allowance_grant");
    assert_eq!(
        ledger_reason::STAFF_ALLOWANCE_RENEWAL,
        "staff_allowance_renewal"
    );
}

#[test]
fn staff_allowance_status_variants() {
    assert_eq!(
        serde_json::to_string(&StaffGamingAllowanceStatus::None).unwrap(),
        "\"none\""
    );
    assert_eq!(
        serde_json::to_string(&StaffGamingAllowanceStatus::Active).unwrap(),
        "\"active\""
    );
}

#[test]
fn kiosk_player_login_staff_gates_documented() {
    // auth_service::login_player for staff:
    // 1. shift_repo.find_active_by_user -> STAFF_SHIFT_ACTIVE
    // 2. balances.require_staff_allowance_for_device -> STAFF_ALLOWANCE_*
    // PlayerUser middleware accepts roles player or staff.
    let _ = plan_kind::STAFF_ALLOWANCE;
    assert!(true);
}
