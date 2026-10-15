//! Crystal fresh ordinary Gold Trade name-tag expiry; no client clock or wire change.
use std::time::{SystemTime, UNIX_EPOCH};
use mir2_protocol::UserItemExpireInfo;

pub(super) const MAX_TICKS: i64 = 3_155_378_975_999_999_999;
pub(super) const UTC_KIND: i64 = 1_i64 << 62;
const UNIX_EPOCH_TICKS: i64 = 621_355_968_000_000_000;
const SECOND_TICKS: i64 = 10_000_000;
const DAY_TICKS: i64 = 864_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpiryError { Clock, DateRange }

/// Equivalent clock kind to Crystal Envir.Now (UTC). Capture once per purchase.
pub(super) fn capture_npc_trade_utc_ticks() -> Result<i64, ExpiryError> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| ExpiryError::Clock)?;
    let seconds = i64::try_from(now.as_secs()).map_err(|_| ExpiryError::Clock)?;
    let ticks = seconds.checked_mul(SECOND_TICKS)
        .and_then(|ticks| ticks.checked_add(UNIX_EPOCH_TICKS))
        .and_then(|ticks| ticks.checked_add(i64::from(now.subsec_nanos() / 100)))
        .ok_or(ExpiryError::Clock)?;
    (0..=MAX_TICKS).contains(&ticks).then_some(ticks).ok_or(ExpiryError::Clock)
}

/// Regex \[(.*?)\] without Singleline: first SUCCESSFUL pair, not first '['.
/// An embedded '[' is part of the capture; only '\n' prevents dot matching.
fn first_tag(name: &str) -> Option<&str> {
    let bytes = name.as_bytes();
    for (start, byte) in bytes.iter().enumerate() {
        if *byte != b'[' { continue; }
        for end in start + 1..bytes.len() {
            match bytes[end] {
                b'\n' => break,
                b']' => return Some(&name[start + 1..end]),
                _ => {}
            }
        }
    }
    None
}

pub(super) fn has_expiry_tag(name: &str) -> bool { first_tag(name).is_some() }

// Crystal's unanchored [0-9]*[a-zA-Z]* always matches at offset zero, including
// an empty match. Overflow in Int32.TryParse makes num zero, not a saturated max.
fn parameter(value: &str) -> (u32, &str) {
    let bytes = value.as_bytes();
    let mut end = 0;
    let mut numeric = Some(0_i32);
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        numeric = numeric.and_then(|n| n.checked_mul(10)
            .and_then(|n| n.checked_add(i32::from(bytes[end] - b'0'))));
        end += 1;
    }
    let start = end;
    while end < bytes.len() && bytes[end].is_ascii_alphabetic() { end += 1; }
    (numeric.unwrap_or(0) as u32, &value[start..end])
}

fn leap(year: i64) -> bool { year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) }
fn month_days(year: i64, month: i64) -> i64 {
    match month { 2 => if leap(year) { 29 } else { 28 }, 4 | 6 | 9 | 11 => 30, _ => 31 }
}
fn days_before_year(year: i64) -> i64 {
    let y = year - 1;
    365 * y + y / 4 - y / 100 + y / 400
}
fn date_from_days(days: i64) -> (i64, i64, i64) {
    let mut remaining = days;
    let n400 = remaining / 146_097;
    remaining %= 146_097;
    let n100 = (remaining / 36_524).min(3);
    remaining -= n100 * 36_524;
    let n4 = remaining / 1_461;
    remaining %= 1_461;
    let n1 = (remaining / 365).min(3);
    remaining -= n1 * 365;
    let year = n400 * 400 + n100 * 100 + n4 * 4 + n1 + 1;
    let mut month = 1;
    while remaining >= month_days(year, month) {
        remaining -= month_days(year, month);
        month += 1;
    }
    (year, month, remaining + 1)
}
fn add_months(ticks: i64, months: u32) -> Result<i64, ExpiryError> {
    if months > 120_000 { return Err(ExpiryError::DateRange); }
    let (year, month, day) = date_from_days(ticks / DAY_TICKS);
    let ordinal = (year - 1).checked_mul(12)
        .and_then(|n| n.checked_add(month - 1))
        .and_then(|n| n.checked_add(i64::from(months))).ok_or(ExpiryError::DateRange)?;
    if ordinal >= 9_999 * 12 { return Err(ExpiryError::DateRange); }
    let year = ordinal / 12 + 1;
    let month = ordinal % 12 + 1;
    let day = day.min(month_days(year, month));
    let mut days = days_before_year(year) + day - 1;
    for m in 1..month { days += month_days(year, m); }
    days.checked_mul(DAY_TICKS).and_then(|n| n.checked_add(ticks % DAY_TICKS))
        .filter(|n| *n <= MAX_TICKS).ok_or(ExpiryError::DateRange)
}

pub(super) fn fresh_expire_info(name: &str, now_utc_ticks: i64)
    -> Result<Option<UserItemExpireInfo>, ExpiryError>
{
    let Some(tag) = first_tag(name) else { return Ok(None); };
    let (number, unit) = parameter(tag);
    if !matches!(unit, "m" | "h" | "d" | "M" | "y") {
        // DateTime.MaxValue has Unspecified kind, unlike timed UTC results.
        return Ok(Some(UserItemExpireInfo { expiry_binary_datetime: MAX_TICKS }));
    }
    if !(0..=MAX_TICKS).contains(&now_utc_ticks) { return Err(ExpiryError::DateRange); }
    let ticks = match unit {
        "M" => add_months(now_utc_ticks, number)?,
        "y" => {
            if number > 10_000 { return Err(ExpiryError::DateRange); }
            add_months(now_utc_ticks, number.checked_mul(12).ok_or(ExpiryError::DateRange)?)?
        }
        _ => {
            let unit_ticks = match unit { "m" => 60 * SECOND_TICKS,
                "h" => 3_600 * SECOND_TICKS, _ => DAY_TICKS };
            i64::from(number).checked_mul(unit_ticks)
                .and_then(|n| now_utc_ticks.checked_add(n))
                .filter(|n| *n <= MAX_TICKS).ok_or(ExpiryError::DateRange)?
        }
    };
    Ok(Some(UserItemExpireInfo { expiry_binary_datetime: ticks | UTC_KIND }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn expiry(name: &str, ticks: i64) -> i64 {
        fresh_expire_info(name, ticks).unwrap().unwrap().expiry_binary_datetime
    }
    #[test]
    fn npc_gold_trade_expiry_first_successful_bracket_and_prefix_grammar() {
        let now = 638_422_560_000_000_000;
        for name in ["x[3h!ignored]", "x[unclosed\ntext[3h]", "x[unclosed\n[3h]"] {
            assert_eq!(expiry(name, now), (now + 108_000_000_000) | UTC_KIND);
        }
        for name in ["x[][3h]", "x[?3h]", "x[3hours]", "x[3hX]", "x[a[b]", "x[ 3h]", "x[-3h]"] {
            assert_eq!(expiry(name, now), MAX_TICKS, "{name}");
        }
        assert_eq!(expiry("x[3h\rrest]", now), (now + 108_000_000_000) | UTC_KIND);
        for name in ["x", "x[3h", "x[3\nh]"] { assert!(!has_expiry_tag(name)); }
    }
    #[test]
    fn npc_gold_trade_expiry_duration_units_and_kind_are_exact() {
        let now = 638_422_560_123_456_789;
        for (name, added) in [("x[2m]", 1_200_000_000), ("x[2h]", 72_000_000_000),
            ("x[2d]", 1_728_000_000_000), ("x[0m]", 0), ("x[m]", 0)] {
            assert_eq!(expiry(name, now), (now + added) | UTC_KIND);
        }
        assert_eq!(fresh_expire_info("untagged", now), Ok(None));
        assert_eq!(expiry("x[H]", now), MAX_TICKS);
        assert_eq!(expiry("x[]", now), MAX_TICKS);
    }
    #[test]
    fn npc_gold_trade_expiry_gregorian_month_year_clamp_preserves_time() {
        // Independently specified UTC dates: 2024-01-31, 02-29, 03-31 and 2025-02-28.
        let subsecond = 123_456_789;
        assert_eq!(expiry("x[1M]", 638_422_560_000_000_000 + subsecond),
            (638_447_616_000_000_000 + subsecond) | UTC_KIND);
        assert_eq!(expiry("x[2M]", 638_422_560_000_000_000 + subsecond),
            (638_474_400_000_000_000 + subsecond) | UTC_KIND);
        assert_eq!(expiry("x[1y]", 638_447_616_000_000_000 + subsecond),
            (638_762_976_000_000_000 + subsecond) | UTC_KIND);
        assert_eq!(expiry("x[1M]", 30 * DAY_TICKS), 58 * DAY_TICKS | UTC_KIND);
        assert_eq!(expiry("x[0M]", MAX_TICKS), MAX_TICKS | UTC_KIND);
        // Gregorian century controls: 1900 is not leap; 2000 is leap.
        assert_eq!(expiry("x[1M]", 693_625 * DAY_TICKS), 693_653 * DAY_TICKS | UTC_KIND);
        assert_eq!(expiry("x[1y]", 630_873_792_000_000_000), 631_189_152_000_000_000 | UTC_KIND);
        assert_eq!(expiry("x[119987M]", 0), 3_155_352_192_000_000_000 | UTC_KIND);
    }
    #[test]
    fn npc_gold_trade_expiry_int32_overflow_zero_and_date_range_fail_closed() {
        let now = 638_422_560_000_000_000;
        assert_eq!(expiry("x[2147483648m]", now), now | UTC_KIND);
        assert_eq!(expiry("x[0000000000000000000001m]", now), (now + 600_000_000) | UTC_KIND);
        for name in ["x[2147483647d]", "x[120001M]", "x[10001y]", "x[9999y]"] {
            assert_eq!(fresh_expire_info(name, now), Err(ExpiryError::DateRange), "{name}");
        }
        for name in ["x[1m]", "x[1M]", "x[1y]"] {
            assert_eq!(fresh_expire_info(name, MAX_TICKS), Err(ExpiryError::DateRange));
        }
        assert_eq!(fresh_expire_info("x[1m]", -1), Err(ExpiryError::DateRange));
        assert_eq!(fresh_expire_info("x[1m]", UTC_KIND), Err(ExpiryError::DateRange));
    }
}
