//! Cash register opening balance carry-forward tests.
//! Integration tests: `cargo test --test cash_register_carry_forward -- --ignored`

use gaming_cafe_api::app::build_state;
use gaming_cafe_api::config::load_dotenv;
use gaming_cafe_api::models::{
    ClockInDto, ClockOutDto, CloseCashRegisterDto, InitiateDepositDto,
};
use gaming_cafe_api::services::CashRegisterService;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

async fn setup() -> Option<Arc<gaming_cafe_api::app::AppState>> {
    load_dotenv();
    if std::env::var("DATABASE_URL").is_err() && std::env::var("DB_HOST").is_err() {
        return None;
    }
    Some(build_state().await)
}

async fn find_staff_user_id(pool: &PgPool) -> Option<Uuid> {
    sqlx::query_scalar(
        r#"SELECT id FROM users WHERE role = 'staff' AND "isActive" = true LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

async fn find_admin_user_id(pool: &PgPool) -> Option<Uuid> {
    sqlx::query_scalar(
        r#"SELECT id FROM users WHERE role = 'admin' AND "isActive" = true LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

async fn cleanup_active_shift(
    state: &gaming_cafe_api::app::AppState,
    user_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(shift) = state.shifts.get_active(user_id).await? {
        state.shifts.force_close(shift.id, user_id).await?;
    }
    Ok(())
}

async fn open_shift_with_register(
    state: &gaming_cafe_api::app::AppState,
    user_id: Uuid,
    opening: f64,
) -> Result<(Uuid, Uuid), Box<dyn std::error::Error>> {
    let shift = state
        .shifts
        .clock_in(
            user_id,
            ClockInDto {
                notes: Some("carry-forward test".to_string()),
            },
            user_id,
        )
        .await?;
    let register = state
        .cash_registers
        .ensure_open_for_shift(shift.id, user_id, opening)
        .await?;
    Ok((shift.id, register.id))
}

async fn close_shift_with_register(
    state: &gaming_cafe_api::app::AppState,
    user_id: Uuid,
    shift_id: Uuid,
    register_id: Uuid,
    closing: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    state
        .cash_registers
        .close(
            register_id,
            CloseCashRegisterDto {
                closing_balance: closing,
                closing_denominations: None,
                notes: Some("carry-forward test close".to_string()),
            },
            user_id,
        )
        .await?;
    state
        .shifts
        .clock_out(
            user_id,
            ClockOutDto {
                notes: Some("carry-forward test".to_string()),
            },
            user_id,
        )
        .await?;
    let _ = shift_id;
    Ok(())
}

#[test]
fn opening_from_closing_subtracts_all_deposits() {
    assert_eq!(
        CashRegisterService::opening_from_closing_and_deposits(6000.0, 5000.0),
        1000.0
    );
    assert_eq!(
        CashRegisterService::opening_from_closing_and_deposits(5000.0, 0.0),
        5000.0
    );
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn carry_forward_uses_previous_closing_without_deposit() {
    let Some(state) = setup().await else {
        return;
    };
    let Some(user_id) = find_staff_user_id(&state.db).await else {
        return;
    };

    cleanup_active_shift(&state, user_id)
        .await
        .expect("cleanup");

    let (shift1, register1) = open_shift_with_register(&state, user_id, 0.0)
        .await
        .expect("open shift 1");
    close_shift_with_register(&state, user_id, shift1, register1, 5000.0)
        .await
        .expect("close shift 1");

    let shift2 = state
        .shifts
        .clock_in(
            user_id,
            ClockInDto {
                notes: Some("carry-forward test day 2".to_string()),
            },
            user_id,
        )
        .await
        .expect("clock in shift 2");
    let register2 = state
        .cash_registers
        .carry_forward_balance(user_id, shift2.id, user_id)
        .await
        .expect("carry forward");

    assert!(
        (register2.opening_balance - 5000.0).abs() < 0.01,
        "expected opening 5000, got {}",
        register2.opening_balance
    );

    cleanup_active_shift(&state, user_id)
        .await
        .expect("cleanup after test");
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn carry_forward_subtracts_pending_deposit_cash_out() {
    let Some(state) = setup().await else {
        return;
    };
    let Some(user_id) = find_staff_user_id(&state.db).await else {
        return;
    };

    cleanup_active_shift(&state, user_id)
        .await
        .expect("cleanup");

    let (shift1, register1) = open_shift_with_register(&state, user_id, 1000.0)
        .await
        .expect("open shift 1");

    state
        .cash_deposits
        .initiate(
            InitiateDepositDto {
                cash_register_id: register1,
                shift_id: shift1,
                amount: 5000.0,
                denominations: serde_json::json!({}),
                notes: Some("carry-forward test deposit".to_string()),
            },
            user_id,
        )
        .await
        .expect("initiate deposit");

    close_shift_with_register(&state, user_id, shift1, register1, 6000.0)
        .await
        .expect("close shift 1");

    let shift2 = state
        .shifts
        .clock_in(
            user_id,
            ClockInDto {
                notes: Some("carry-forward test day 2".to_string()),
            },
            user_id,
        )
        .await
        .expect("clock in shift 2");
    let register2 = state
        .cash_registers
        .carry_forward_balance(user_id, shift2.id, user_id)
        .await
        .expect("carry forward");

    assert!(
        (register2.opening_balance - 1000.0).abs() < 0.01,
        "expected opening 1000 after pending deposit, got {}",
        register2.opening_balance
    );

    cleanup_active_shift(&state, user_id)
        .await
        .expect("cleanup after test");
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn carry_forward_subtracts_approved_deposit_cash_out() {
    let Some(state) = setup().await else {
        return;
    };
    let Some(user_id) = find_staff_user_id(&state.db).await else {
        return;
    };
    let Some(admin_id) = find_admin_user_id(&state.db).await else {
        return;
    };

    cleanup_active_shift(&state, user_id)
        .await
        .expect("cleanup");

    let (shift1, register1) = open_shift_with_register(&state, user_id, 1000.0)
        .await
        .expect("open shift 1");

    let deposit = state
        .cash_deposits
        .initiate(
            InitiateDepositDto {
                cash_register_id: register1,
                shift_id: shift1,
                amount: 5000.0,
                denominations: serde_json::json!({}),
                notes: Some("carry-forward test deposit".to_string()),
            },
            user_id,
        )
        .await
        .expect("initiate deposit");

    state
        .cash_deposits
        .approve(deposit.id, "bank", admin_id)
        .await
        .expect("approve deposit");

    close_shift_with_register(&state, user_id, shift1, register1, 6000.0)
        .await
        .expect("close shift 1");

    let shift2 = state
        .shifts
        .clock_in(
            user_id,
            ClockInDto {
                notes: Some("carry-forward test day 2".to_string()),
            },
            user_id,
        )
        .await
        .expect("clock in shift 2");
    let register2 = state
        .cash_registers
        .carry_forward_balance(user_id, shift2.id, user_id)
        .await
        .expect("carry forward");

    assert!(
        (register2.opening_balance - 1000.0).abs() < 0.01,
        "expected opening 1000 after approved deposit, got {}",
        register2.opening_balance
    );

    cleanup_active_shift(&state, user_id)
        .await
        .expect("cleanup after test");
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn ensure_shift_for_staff_login_recovers_stale_closed_register() {
    let Some(state) = setup().await else {
        return;
    };
    let Some(user_id) = find_staff_user_id(&state.db).await else {
        return;
    };

    cleanup_active_shift(&state, user_id)
        .await
        .expect("cleanup");

    let (stale_shift_id, register_id) = open_shift_with_register(&state, user_id, 2500.0)
        .await
        .expect("open stale shift");

    state
        .cash_registers
        .close(
            register_id,
            CloseCashRegisterDto {
                closing_balance: 7500.0,
                closing_denominations: None,
                notes: Some("stale shift register closed".to_string()),
            },
            user_id,
        )
        .await
        .expect("close register without clock out");

    let recovered = state
        .shifts
        .ensure_shift_for_staff_login(user_id, user_id)
        .await
        .expect("recover stale shift");

    assert_ne!(
        recovered.id, stale_shift_id,
        "expected a fresh shift after stale recovery"
    );

    let register = state
        .cash_registers
        .carry_forward_balance(user_id, recovered.id, user_id)
        .await
        .expect("carry forward after recovery");

    assert!(
        (register.opening_balance - 7500.0).abs() < 0.01,
        "expected opening 7500 from stale close, got {}",
        register.opening_balance
    );

    cleanup_active_shift(&state, user_id)
        .await
        .expect("cleanup after test");
}
