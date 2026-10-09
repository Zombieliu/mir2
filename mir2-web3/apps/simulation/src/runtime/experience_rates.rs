//! Crystal PlayerObject.GainExp ordered positive rate contributions.
//! Callers supply rates only after authoritative relationship/buff/presence checks.
/// Apply lover, mentee, then general EXP percentages, truncating each stage.
/// This helper grants no eligibility; a missing/invalid relationship uses zero.
pub fn apply_crystal_experience_rates(base: u32, lover: i32, mentee: i32, general: i32) -> u32 {
    [lover, mentee, general].into_iter().fold(base, |amount, rate| {
        let bonus = u64::from(amount) * rate.max(0) as u64 / 100;
        amount.saturating_add(u32::try_from(bonus).unwrap_or(u32::MAX))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn experience_rates_follow_source_order_and_per_stage_rounding() {
        assert_eq!(apply_crystal_experience_rates(1000, 5, 10, 20), 1386);
        assert_ne!(apply_crystal_experience_rates(1000, 5, 10, 20), 1380);
        assert_eq!(apply_crystal_experience_rates(19, 5, 10, 20), 24);
        assert_eq!(apply_crystal_experience_rates(100, 0, 0, 20), 120);
    }
    #[test]
    fn experience_rates_do_not_invent_negative_or_overflow_awards() {
        assert_eq!(apply_crystal_experience_rates(100, -5, -10, -20), 100);
        assert_eq!(apply_crystal_experience_rates(u32::MAX, i32::MAX, i32::MAX, i32::MAX), u32::MAX);
        assert_eq!(apply_crystal_experience_rates(0, 5, 10, 20), 0);
    }
}
