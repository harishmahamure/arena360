use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rusqlite::{functions::FunctionFlags, Connection};
use rust_decimal::Decimal;
fn timestamp(s: String) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e)))
}
pub fn register(connection: &Connection, zone: chrono_tz::Tz) -> rusqlite::Result<()> {
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    connection.create_scalar_function("report_local_date", 1, flags, move |c| {
        Ok(timestamp(c.get(0)?)?
            .with_timezone(&zone)
            .date_naive()
            .to_string())
    })?;
    connection.create_scalar_function("report_add_days", 2, flags, |c| {
        let time = timestamp(c.get(0)?)?;
        let days: i64 = c.get(1)?;
        let delta = Duration::try_days(days)
            .ok_or_else(|| rusqlite::Error::UserFunctionError("invalid day interval".into()))?;
        let time = time
            .checked_add_signed(delta)
            .ok_or_else(|| rusqlite::Error::UserFunctionError("invalid day interval".into()))?;
        Ok(time.to_rfc3339_opts(SecondsFormat::Micros, true))
    })?;
    connection.create_scalar_function("date_diff", 3, flags, |c| {
        let unit: String = c.get(0)?;
        let a: Option<String> = c.get(1)?;
        let b: Option<String> = c.get(2)?;
        let (Some(a), Some(b)) = (a, b) else {
            return Ok(None);
        };
        let (a, b) = (timestamp(a)?, timestamp(b)?);
        Ok(Some(match unit.as_str() {
            "second" => b.timestamp() - a.timestamp(),
            "day" => (b.date_naive() - a.date_naive()).num_days(),
            _ => {
                return Err(rusqlite::Error::UserFunctionError(
                    "unsupported date_diff unit".into(),
                ))
            }
        }))
    })?;
    connection.create_scalar_function("report_money", 1, flags, |c| {
        Ok(c.get::<Option<f64>>(0)?.map(|v| v / 10_000.0))
    })?;
    connection.create_scalar_function("report_money_text", 2, flags, |c| {
        let value: i64 = c.get(0)?;
        let scale: u32 = c.get(1)?;
        if scale > 4 {
            return Err(rusqlite::Error::UserFunctionError(
                "invalid money scale".into(),
            ));
        }
        let mut value = Decimal::new(value, 4)
            .round_dp_with_strategy(scale, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
        value.rescale(scale);
        Ok(value.to_string())
    })?;
    // Split collections proportionally without integer division or intermediate float rounding.
    connection.create_scalar_function("report_allocate", 3, flags, |c| {
        let (applied, part, total): (i64, i64, i64) = (c.get(0)?, c.get(1)?, c.get(2)?);
        let value = Decimal::from(applied)
            .checked_mul(Decimal::from(part))
            .and_then(|v| v.checked_div(Decimal::from(total)))
            .ok_or_else(|| {
                rusqlite::Error::UserFunctionError("invalid settlement allocation".into())
            })?;
        use rust_decimal::prelude::ToPrimitive;
        value.to_f64().ok_or_else(|| {
            rusqlite::Error::UserFunctionError("settlement allocation overflow".into())
        })
    })?;
    Ok(())
}
