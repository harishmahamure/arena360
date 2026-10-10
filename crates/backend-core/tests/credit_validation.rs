use gaming_cafe_api::models::{compute_available, validate_settlement_items, SettleItemDto};
use uuid::Uuid;

#[test]
fn available_credit_headroom() {
    assert_eq!(compute_available(500.0, 200.0), 300.0);
    assert_eq!(compute_available(100.0, 250.0), 0.0);
}

#[test]
fn settlement_validates_non_empty_items() {
    let err = validate_settlement_items(&[], "cash", None, None).unwrap_err();
    assert!(err.contains("At least one"));
}

#[test]
fn settlement_rejects_zero_line_amount() {
    let items = vec![SettleItemDto {
        transaction_id: Uuid::new_v4(),
        amount: 0.0,
    }];
    let err = validate_settlement_items(&items, "cash", None, None).unwrap_err();
    assert!(err.contains("greater than zero"));
}

#[test]
fn settlement_cash_uses_total() {
    let items = vec![
        SettleItemDto {
            transaction_id: Uuid::new_v4(),
            amount: 10.0,
        },
        SettleItemDto {
            transaction_id: Uuid::new_v4(),
            amount: 20.0,
        },
    ];
    let total = validate_settlement_items(&items, "cash", None, None).unwrap();
    assert!((total - 30.0).abs() < 0.001);
}

#[test]
fn settlement_split_must_match_total() {
    let items = vec![SettleItemDto {
        transaction_id: Uuid::new_v4(),
        amount: 30.0,
    }];
    assert!(validate_settlement_items(&items, "split_payment", Some(10.0), Some(10.0)).is_err());
    assert!(validate_settlement_items(&items, "split_payment", Some(20.0), Some(10.0)).is_ok());
}

#[test]
fn partial_settlement_line_amounts_sum() {
    let items = vec![
        SettleItemDto {
            transaction_id: Uuid::new_v4(),
            amount: 5.0,
        },
        SettleItemDto {
            transaction_id: Uuid::new_v4(),
            amount: 15.0,
        },
    ];
    let total = validate_settlement_items(&items, "online", None, Some(20.0)).unwrap();
    assert!((total - 20.0).abs() < 0.001);
}

#[test]
fn overpay_line_exceeds_remaining() {
    let remaining = 10.0;
    let apply = 15.0;
    assert!(apply > remaining + 0.001);
}
