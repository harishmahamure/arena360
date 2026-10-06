use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

const SCALE_FACTOR: i64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MoneyConversionError {
    #[error("money value has more than four fractional digits")]
    ExcessPrecision,
    #[error("money value is outside the scale-4 integer range")]
    OutOfRange,
}

/// Converts an exact decimal amount into the tenant schema's scale-4 integer representation.
pub fn decimal_to_scale4(value: Decimal) -> Result<i64, MoneyConversionError> {
    if value.round_dp(4) != value {
        return Err(MoneyConversionError::ExcessPrecision);
    }
    value
        .checked_mul(Decimal::from(SCALE_FACTOR))
        .and_then(|scaled| scaled.to_i64())
        .ok_or(MoneyConversionError::OutOfRange)
}

/// Converts a tenant scale-4 integer into an exact decimal amount.
pub fn scale4_to_decimal(value: i64) -> Decimal {
    Decimal::new(value, 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn every_scaled_integer_round_trips(value in any::<i64>()) {
            let decimal = scale4_to_decimal(value);
            prop_assert_eq!(decimal_to_scale4(decimal), Ok(value));
        }

    }

    #[test]
    fn money_rejects_precision_loss_and_integer_overflow() {
        assert_eq!(
            decimal_to_scale4(Decimal::new(1, 5)),
            Err(MoneyConversionError::ExcessPrecision)
        );
        assert_eq!(
            decimal_to_scale4(Decimal::MAX),
            Err(MoneyConversionError::OutOfRange)
        );
    }

    #[test]
    fn money_accepts_trailing_fractional_zeroes_and_boundaries() {
        assert_eq!(decimal_to_scale4("1.23000".parse().unwrap()), Ok(12_300));
        for value in [i64::MIN, i64::MAX, 0] {
            assert_eq!(decimal_to_scale4(scale4_to_decimal(value)), Ok(value));
        }
    }
}
