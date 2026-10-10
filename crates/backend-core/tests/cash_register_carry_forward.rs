//! Carry-forward, stale shift recovery and deposit variance on tenant SQLite.
mod support;
use gaming_cafe_api::{
    models::*,
    repositories::{
        TenantCashDepositRepository, TenantCashRegisterRepository, TenantShiftRepository,
    },
};
use serde_json::json;
use support::TenantFixture;
use uuid::Uuid;
struct Fixture {
    f: TenantFixture,
    venue: Uuid,
    staff: Uuid,
    other: Uuid,
}
impl Fixture {
    async fn new() -> Self {
        let f = TenantFixture::new().await;
        let venue = f.venue("main").await;
        let staff = f.staff(None, vec![]).await;
        let other = f.staff(None, vec![]).await;
        Self {
            f,
            venue,
            staff,
            other,
        }
    }
    fn registers(&self) -> TenantCashRegisterRepository {
        TenantCashRegisterRepository::new(self.f.db.clone())
    }
    fn shifts(&self) -> TenantShiftRepository {
        TenantShiftRepository::new(self.f.db.clone())
    }
    fn deposits(&self) -> TenantCashDepositRepository {
        TenantCashDepositRepository::new(self.f.db.clone())
    }
    async fn start(&self, staff: Uuid, opening: f64) -> ShiftStartResponseDto {
        self.shifts()
            .start_confirmed(
                staff,
                serde_json::from_value(
                    json!({"venueLocationId":self.venue,"openingBalance":opening}),
                )
                .unwrap(),
                staff,
            )
            .await
            .unwrap()
    }
    async fn close(&self, start: &ShiftStartResponseDto, amount: f64) {
        self.registers()
            .close_register(
                start.cash_register.id,
                &serde_json::from_value(json!({"closingBalance":amount})).unwrap(),
                self.staff,
            )
            .await
            .unwrap();
        self.shifts()
            .close(start.shift.id, None, self.staff)
            .await
            .unwrap();
    }
    async fn deposit(&self, start: &ShiftStartResponseDto, amount: f64) -> CashDeposit {
        self.deposits().create(&serde_json::from_value(json!({"cashRegisterId":start.cash_register.id,"shiftId":start.shift.id,"amount":amount,"denominations":{}})).unwrap(),self.staff).await.unwrap()
    }
}
#[tokio::test]
async fn carry_forward_uses_previous_closing_without_deposit() {
    let f = Fixture::new().await;
    let start = f.start(f.staff, 0.0).await;
    f.close(&start, 5000.0).await;
    let carry = f
        .registers()
        .preview_carry_forward_balance_for(f.venue)
        .await
        .unwrap();
    assert_eq!(carry, 5000.0);
    assert_eq!(
        f.start(f.staff, carry).await.cash_register.opening_balance,
        5000.0
    );
    f.f.close().await;
}
async fn deposit_carry(approve: bool) {
    let f = Fixture::new().await;
    let start = f.start(f.staff, 1000.0).await;
    let deposit = f.deposit(&start, 5000.0).await;
    if approve {
        f.deposits()
            .approve(deposit.id, "bank", f.other)
            .await
            .unwrap();
    }
    f.close(&start, 6000.0).await;
    assert_eq!(
        f.registers()
            .preview_carry_forward_balance_for(f.venue)
            .await
            .unwrap(),
        1000.0
    );
    f.f.close().await;
}
#[tokio::test]
async fn carry_forward_subtracts_pending_deposit_cash_out() {
    deposit_carry(false).await;
}
#[tokio::test]
async fn carry_forward_subtracts_approved_deposit_cash_out() {
    deposit_carry(true).await;
}
#[tokio::test]
async fn confirmed_start_recovers_stale_closed_register() {
    let f = Fixture::new().await;
    let stale = f.start(f.staff, 2500.0).await;
    f.registers()
        .close_register(
            stale.cash_register.id,
            &serde_json::from_value(json!({"closingBalance":7500.0})).unwrap(),
            f.staff,
        )
        .await
        .unwrap();
    let carry = f
        .registers()
        .preview_carry_forward_balance_for(f.venue)
        .await
        .unwrap();
    let recovered = f.start(f.staff, carry).await;
    assert_ne!(recovered.shift.id, stale.shift.id);
    assert!(!recovered.resumed);
    assert_eq!(recovered.cash_register.opening_balance, 7500.0);
    assert_eq!(
        f.shifts()
            .find_by_id(stale.shift.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "completed"
    );
    f.f.close().await;
}
#[tokio::test]
async fn carry_forward_uses_counted_closing_with_variance() {
    let f = Fixture::new().await;
    let start = f.start(f.staff, 892.0).await;
    f.close(&start, 792.0).await;
    assert_eq!(
        f.registers()
            .find_by_id(start.cash_register.id)
            .await
            .unwrap()
            .unwrap()
            .variance,
        Some(-100.0)
    );
    assert_eq!(
        f.registers()
            .preview_carry_forward_balance_for(f.venue)
            .await
            .unwrap(),
        792.0
    );
    f.f.close().await;
}
#[tokio::test]
async fn update_opening_corrects_stale_opening() {
    let f = Fixture::new().await;
    let start = f.start(f.staff, 892.0).await;
    let corrected = f
        .registers()
        .update_opening_balance(start.cash_register.id, 792.0, None, f.staff)
        .await
        .unwrap();
    assert_eq!(corrected.opening_balance, 792.0);
    // Resuming a live shift cannot silently replace its confirmed opening count.
    let resumed = f.start(f.staff, 999.0).await;
    assert!(resumed.resumed);
    assert_eq!(resumed.cash_register.opening_balance, 792.0);
    f.f.close().await;
}
#[tokio::test]
async fn carry_forward_uses_venue_predecessor_across_staff() {
    let f = Fixture::new().await;
    let start = f.start(f.staff, 892.0).await;
    f.close(&start, 792.0).await;
    let carry = f
        .registers()
        .preview_carry_forward_balance_for(f.venue)
        .await
        .unwrap();
    assert_eq!(
        f.start(f.other, carry).await.cash_register.opening_balance,
        792.0
    );
    let other_venue = f.f.venue("other").await;
    assert_eq!(
        f.registers()
            .preview_carry_forward_balance_for(other_venue)
            .await
            .unwrap(),
        0.0
    );
    f.f.close().await;
}
#[tokio::test]
async fn approve_deposit_clears_register_variance() {
    let f = Fixture::new().await;
    let start = f.start(f.staff, 660.0).await;
    f.registers()
        .add_entry(
            start.cash_register.id,
            &serde_json::from_value(json!({"entryType":"cash_in","amount":35})).unwrap(),
            f.staff,
        )
        .await
        .unwrap();
    let deposit = f.deposit(&start, 500.0).await;
    let closed = f
        .registers()
        .close_register(
            start.cash_register.id,
            &serde_json::from_value(json!({"closingBalance":1195.0})).unwrap(),
            f.staff,
        )
        .await
        .unwrap();
    assert_eq!(closed.variance, Some(500.0));
    f.deposits()
        .approve(deposit.id, "bank", f.other)
        .await
        .unwrap();
    let updated = f
        .registers()
        .get_by_id(start.cash_register.id)
        .await
        .unwrap();
    assert_eq!(updated.register.variance, Some(0.0));
    assert_eq!(updated.register.total_deposited, Some(500.0));
    f.f.close().await;
}
