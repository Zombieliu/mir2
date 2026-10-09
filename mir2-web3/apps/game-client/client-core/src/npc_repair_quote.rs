//! Pure Crystal NPC repair quotation. Read-only pricing, never an item command.
//! The host supplies concrete source metadata and independently current live fields.

#[derive(Debug, Clone, Copy)]
pub struct NpcRepairSource<'a> {
    pub live_unique_id: Option<u64>,
    pub source_unique_id: u64,
    pub template_item_index: i32,
    pub source_item_index: i32,
    pub template_price: u32,
    pub template_durability: u16,
    pub live_count: u32,
    pub live_current_dura: Option<u16>,
    pub live_max_dura: Option<u16>,
    pub added_stat_values: &'a [i32],
    pub rental: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NpcRepairQuote {
    pub repair_price: u32,
    /// The original f32 client display and affordability threshold.
    pub displayed_total: f32,
    /// The original truncated u32 server deduction, not the client gold gate.
    pub total_price: u32,
}

pub fn repair_quote(source: Option<NpcRepairSource<'_>>, rate: f32, special: bool) -> Option<NpcRepairQuote> {
    if !rate.is_finite() || rate < 0.0 { return None; }
    let source = source?;
    if source.live_unique_id? != source.source_unique_id
        || source.template_item_index != source.source_item_index
        || source.template_durability == 0 { return None; }
    let count = u16::try_from(source.live_count).ok()?;
    if count == 0 { return None; }
    let current_dura = source.live_current_dura?;
    let max_dura = source.live_max_dura?;
    if current_dura > max_dura { return None; }
    let stat_weight = source.added_stat_values.iter().try_fold(0_u32, |weight, value| {
        weight.checked_add(value.checked_abs()? as u32)
    })?;
    let factor = 1.0 + stat_weight as f32 * 0.1;
    let template_price = source.template_price as f32;
    let template_durability = source.template_durability as f32;
    let max_dura_f = max_dura as f32;
    let full_base = (max_dura_f * ((template_price / 2.0) / template_durability)
        + template_price / 2.0).floor();
    let full_unit = source_u32(full_base * factor)?;
    // Preserve the separately truncated MaxValue before CurrentPrice floor.
    let max_value = source_u32(max_dura_f * ((template_price / 2.0) / template_durability))?;
    let durability_ratio = if max_dura == 0 { 0.0 } else { current_dura as f32 / max_dura_f };
    let current_base = (max_value as f32 / 2.0 + (max_value as f32 / 2.0) * durability_ratio
        + template_price / 2.0).floor();
    let current_unit = source_u32(current_base * factor)?;
    let count = u32::from(count);
    let full_price = full_unit.checked_mul(count)?;
    let current_price = current_unit.checked_mul(count)?;
    let mut repair_price = full_price.checked_sub(current_price)?;
    if source.rental { repair_price = repair_price.checked_mul(2)?; }
    let service_multiplier = if special { 3.0 } else { 1.0 };
    let displayed_total = repair_price as f32 * service_multiplier * rate;
    let total_price = source_u32(displayed_total)?;
    Some(NpcRepairQuote { repair_price, displayed_total, total_price })
}

pub fn repair_is_affordable(quote: NpcRepairQuote, gold: u32) -> bool {
    gold as f32 >= quote.displayed_total
}

/// Keep the Native inclusive f32 boundary and Rust saturating float-to-u32 cast.
fn source_u32(value: f32) -> Option<u32> {
    if value.is_finite() && (0.0..=u32::MAX as f32).contains(&value) { Some(value as u32) }
    else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(values: &[i32]) -> NpcRepairSource<'_> {
        NpcRepairSource { live_unique_id: Some(41), source_unique_id: 41,
            template_item_index: 77, source_item_index: 77, template_price: 1000,
            template_durability: 1000, live_count: 1, live_current_dura: Some(500),
            live_max_dura: Some(1000), added_stat_values: values, rental: false }
    }

    #[test]
    fn source_stats_normal_special_and_rate_keep_native_prices() {
        assert_eq!(repair_quote(Some(source(&[5])), 1.0, false),
            Some(NpcRepairQuote { repair_price: 188, displayed_total: 188.0, total_price: 188 }));
        assert_eq!(repair_quote(Some(source(&[5])), 1.0, true).unwrap().total_price, 564);
        assert_eq!(repair_quote(Some(source(&[5])), 1.25, false).unwrap().total_price, 235);
        assert_eq!(repair_quote(Some(source(&[-5])), 1.0, false), repair_quote(Some(source(&[5])), 1.0, false));
    }

    #[test]
    fn rental_doubles_before_special_and_rate() {
        let mut s = source(&[5]); s.rental = true;
        assert_eq!(repair_quote(Some(s), 1.0, false).unwrap().repair_price, 376);
        assert_eq!(repair_quote(Some(s), 1.0, true).unwrap().total_price, 1128);
        assert_eq!(repair_quote(Some(s), 1.5, true).unwrap().total_price, 1692);
    }

    #[test]
    fn live_count_and_durability_are_the_only_mutable_authority() {
        let mut s = source(&[5]); s.live_count = 2;
        assert_eq!(repair_quote(Some(s), 1.0, false).unwrap().total_price, 376);
        s.live_count = 1; s.live_current_dura = Some(250);
        assert_eq!(repair_quote(Some(s), 1.0, false).unwrap().total_price, 282);
        s.live_current_dura = None; assert_eq!(repair_quote(Some(s), 1.0, false), None);
        s.live_current_dura = Some(250); s.live_max_dura = None;
        assert_eq!(repair_quote(Some(s), 1.0, false), None);
    }

    #[test]
    fn absent_or_mismatched_source_never_fakes_a_zero_quote() {
        assert_eq!(repair_quote(None, 1.0, false), None);
        let mut s = source(&[]); s.live_unique_id = None;
        assert_eq!(repair_quote(Some(s), 1.0, false), None);
        s.live_unique_id = Some(42); assert_eq!(repair_quote(Some(s), 1.0, false), None);
        s.live_unique_id = Some(41); s.source_item_index = 78;
        assert_eq!(repair_quote(Some(s), 1.0, false), None);
        s.source_item_index = 77; s.template_durability = 0;
        assert_eq!(repair_quote(Some(s), 1.0, false), None);
    }

    #[test]
    fn zero_live_max_and_zero_rate_are_real_known_quotes() {
        let mut s = source(&[]); s.live_current_dura = Some(0); s.live_max_dura = Some(0);
        assert_eq!(repair_quote(Some(s), 1.0, false).unwrap().total_price, 0);
        assert_eq!(repair_quote(Some(source(&[5])), 0.0, false).unwrap().displayed_total, 0.0);
        s.live_unique_id = Some(0); s.source_unique_id = 0;
        assert!(repair_quote(Some(s), 1.0, false).is_some());
        s.live_unique_id = Some(u64::MAX); s.source_unique_id = u64::MAX;
        assert!(repair_quote(Some(s), 1.0, false).is_some());
    }

    #[test]
    fn fractional_price_preserves_the_untruncated_float_gold_gate() {
        let quote = repair_quote(Some(source(&[5])), 0.1, false).unwrap();
        assert_eq!(quote.displayed_total, 18.800001);
        assert_eq!(quote.total_price, 18);
        assert!(!repair_is_affordable(quote, 18));
        assert!(repair_is_affordable(quote, 19));
        let high = NpcRepairQuote { repair_price: 0, displayed_total: 16_777_216.0, total_price: 16_777_216 };
        assert!(repair_is_affordable(high, 16_777_217));
    }

    #[test]
    fn invalid_rates_counts_stats_and_arithmetic_overflow_are_unknown() {
        for rate in [f32::NAN, f32::INFINITY, -1.0] {
            assert_eq!(repair_quote(Some(source(&[])), rate, false), None);
        }
        for count in [0, 65536] {
            let mut s = source(&[]); s.live_count = count;
            assert_eq!(repair_quote(Some(s), 1.0, false), None);
        }
        let mut s = source(&[]); s.live_current_dura = Some(1001);
        assert_eq!(repair_quote(Some(s), 1.0, false), None);
        assert_eq!(repair_quote(Some(source(&[i32::MIN])), 1.0, false), None);
        assert_eq!(repair_quote(Some(source(&[i32::MAX, i32::MAX, 2])), 1.0, false), None);
        s = source(&[]); s.template_price = u32::MAX; s.live_count = 2;
        assert_eq!(repair_quote(Some(s), 1.0, false), None);
        assert_eq!(repair_quote(Some(source(&[5])), f32::MAX, true), None);
    }
}
