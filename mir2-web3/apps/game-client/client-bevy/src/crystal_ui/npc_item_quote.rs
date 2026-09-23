//! Pure Crystal NPC item-service price calculations.
//!
//! A repair quote is presentation only. It requires a current, concrete
//! `UserItem` tooltip source plus the live inventory durability/count so a
//! stale tooltip snapshot cannot become a transaction affordance.

use crate::inventory::ItemModel;

/// Source-faithful repair price before and after the current NPC multiplier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NpcRepairQuote {
    /// Crystal `UserItem.RepairPrice()` before the NPC rate/service factor.
    pub repair_price: u32,
    /// The float Crystal renders in `NPCDropDialog.InfoLabel` and compares
    /// against the player's gold before it sends a repair packet.
    pub displayed_total: f32,
    /// The `uint` cost used by the server after the client-side float check.
    pub total_price: u32,
}

/// Crystal's sale amount: adjusted `ItemData.Price` is multiplied by the
/// selected stack count before NPCDropDialog divides the total by two. Keeping
/// this order preserves odd-price stacks (`101 * 2 / 2 == 101`).
pub(crate) fn crystal_npc_sale_quote(item: &ItemModel, selected_count: u16) -> Option<u32> {
    let (source, user) = concrete_tooltip_source(item)?;
    let selected_count = u32::from(selected_count);
    if selected_count == 0
        || selected_count > item.quantity.min(u32::from(u16::MAX))
    {
        return None;
    }
    let unit_price = crystal_current_unit_price(item, source, user)?;
    unit_price.checked_mul(selected_count).map(|total| total / 2)
}

/// Mirrors Crystal's active `UserItem.Price` / `RepairPrice` arithmetic and
/// `NPCDropDialog` repair multipliers. The live `ItemModel` count and dura are
/// deliberate: network snapshots can refresh those fields after a tooltip was
/// built, while item Info, added stats, and rental state remain source data.
pub(crate) fn crystal_npc_repair_quote(
    item: &ItemModel,
    rate: f32,
    special: bool,
) -> Option<NpcRepairQuote> {
    if !rate.is_finite() || rate < 0.0 {
        return None;
    }

    let (source, user) = concrete_tooltip_source(item)?;
    if source.info.durability == 0 {
        return None;
    }

    // Snapshot metadata is the only authority for values that change after a
    // repair or stack update. Do not fall back to stale tooltip dura/count.
    let count = u16::try_from(item.quantity).ok()?;
    if count == 0 {
        return None;
    }
    let current_dura = item.durability_current?;
    let max_dura = item.durability_max?;
    if current_dura > max_dura {
        return None;
    }

    let stat_weight = crystal_added_stat_weight(user)?;
    let factor = 1.0 + stat_weight as f32 * 0.1;
    let template_price = source.info.price as f32;
    let template_durability = source.info.durability as f32;
    let max_dura_f = max_dura as f32;

    // `UserItem.RepairPrice`: full value is floor'd, then its added-stat
    // factor is cast to uint (truncated).
    let full_base = (max_dura_f * ((template_price / 2.0) / template_durability)
        + template_price / 2.0)
        .floor();
    let full_unit = source_u32(full_base * factor)?;

    let current_unit = crystal_current_unit_price(item, source, user)?;
    let count = u32::from(count);
    let full_price = full_unit.checked_mul(count)?;
    let current_price = current_unit.checked_mul(count)?;
    let mut repair_price = full_price.checked_sub(current_price)?;
    if user.rental_information.is_some() {
        repair_price = repair_price.checked_mul(2)?;
    }

    let service_multiplier = if special { 3.0 } else { 1.0 };
    let displayed_total = repair_price as f32 * service_multiplier * rate;
    let total_price = source_u32(displayed_total)?;
    Some(NpcRepairQuote {
        repair_price,
        displayed_total,
        total_price,
    })
}

fn concrete_tooltip_source(
    item: &ItemModel,
) -> Option<(
    &crate::inventory::CrystalItemTooltipSourceModel,
    &crate::inventory::CrystalUserItemModel,
)> {
    let source = item.tooltip_source.as_ref()?;
    let user = source.user_item.as_ref()?;
    let unique_id = item.unique_id?;
    (user.unique_id == unique_id && user.item_index == source.info.item_index).then_some((source, user))
}

/// Crystal `UserItem.Price` with source metadata plus live inventory dura.
/// Nondurable items have no live durability authority and use their adjusted
/// template price directly.
fn crystal_current_unit_price(
    item: &ItemModel,
    source: &crate::inventory::CrystalItemTooltipSourceModel,
    user: &crate::inventory::CrystalUserItemModel,
) -> Option<u32> {
    let stat_weight = crystal_added_stat_weight(user)?;
    let factor = 1.0 + stat_weight as f32 * 0.1;
    let template_price = source.info.price as f32;
    if source.info.durability == 0 {
        return source_u32(template_price * factor);
    }

    let current_dura = item.durability_current?;
    let max_dura = item.durability_max?;
    if current_dura > max_dura {
        return None;
    }
    let max_dura_f = max_dura as f32;
    let max_value = source_u32(
        max_dura_f * ((template_price / 2.0) / source.info.durability as f32),
    )?;
    // Crystal treats a zero live MaxDura as a zero ratio rather than an
    // unavailable value. Its resulting `Price` is the base half-price.
    let durability_ratio = if max_dura == 0 {
        0.0
    } else {
        current_dura as f32 / max_dura_f
    };
    let current_base =
        (max_value as f32 / 2.0 + (max_value as f32 / 2.0) * durability_ratio + template_price / 2.0)
            .floor();
    source_u32(current_base * factor)
}

/// Crystal `Stats.Count` is the sum of absolute stat values, rather than the
/// number of stat entries (`Shared/Data/Stat.cs`).
fn crystal_added_stat_weight(user: &crate::inventory::CrystalUserItemModel) -> Option<u32> {
    user.added_stats.iter().try_fold(0_u32, |weight, stat| {
        let value = stat.value.checked_abs()? as u32;
        weight.checked_add(value)
    })
}

fn source_u32(value: f32) -> Option<u32> {
    if value.is_finite() && (0.0..=u32::MAX as f32).contains(&value) {
        Some(value as u32)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        CrystalItemInfoModel, CrystalItemStatModel, CrystalItemTooltipSourceModel,
        CrystalUserItemModel,
    };

    fn item() -> ItemModel {
        ItemModel {
            unique_id: Some(41),
            quantity: 1,
            durability_current: Some(500),
            durability_max: Some(1000),
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: 77,
                    price: 1000,
                    durability: 1000,
                    ..Default::default()
                },
                user_item: Some(CrystalUserItemModel {
                    unique_id: 41,
                    item_index: 77,
                    current_dura: 500,
                    max_dura: 1000,
                    count: 1,
                    added_stats: vec![CrystalItemStatModel { stat: 5, value: 5 }],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn quote_uses_source_stat_weight_and_service_multiplier() {
        let item = item();
        assert_eq!(
            crystal_npc_repair_quote(&item, 1.0, false),
            Some(NpcRepairQuote {
                repair_price: 188,
                displayed_total: 188.0,
                total_price: 188,
            })
        );
        assert_eq!(
            crystal_npc_repair_quote(&item, 1.0, true),
            Some(NpcRepairQuote {
                repair_price: 188,
                displayed_total: 564.0,
                total_price: 564,
            })
        );
        assert_eq!(
            crystal_npc_repair_quote(&item, 1.25, false).map(|quote| quote.total_price),
            Some(235)
        );
    }

    #[test]
    fn rental_doubles_before_the_npc_service_multiplier() {
        let mut item = item();
        item.tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .rental_information = Some(Default::default());
        assert_eq!(
            crystal_npc_repair_quote(&item, 1.0, false),
            Some(NpcRepairQuote {
                repair_price: 376,
                displayed_total: 376.0,
                total_price: 376,
            })
        );
        assert_eq!(
            crystal_npc_repair_quote(&item, 1.0, true).map(|quote| quote.total_price),
            Some(1128)
        );
        assert_eq!(
            crystal_npc_repair_quote(&item, 1.5, true).map(|quote| quote.total_price),
            Some(1692)
        );
    }

    #[test]
    fn quote_uses_live_durability_and_rejects_incomplete_or_stale_source() {
        let mut item = item();
        item.quantity = 2;
        assert_eq!(
            crystal_npc_repair_quote(&item, 1.0, false).map(|quote| quote.total_price),
            Some(376),
            "live snapshot count supersedes the older tooltip value"
        );
        item.quantity = 1;
        item.durability_current = Some(250);
        assert_eq!(
            crystal_npc_repair_quote(&item, 1.0, false).map(|quote| quote.total_price),
            Some(282),
            "live snapshot durability supersedes the older tooltip value"
        );

        item.durability_current = None;
        assert_eq!(crystal_npc_repair_quote(&item, 1.0, false), None);
        item.durability_current = Some(250);
        item.tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .unique_id = 99;
        assert_eq!(crystal_npc_repair_quote(&item, 1.0, false), None);
    }

    #[test]
    fn sale_quote_multiplies_before_halving_and_requires_current_concrete_source() {
        let mut item = item();
        item.quantity = 2;
        item.durability_current = None;
        item.durability_max = None;
        let source = item.tooltip_source.as_mut().unwrap();
        source.info.price = 101;
        source.info.durability = 0;
        source.user_item.as_mut().unwrap().added_stats.clear();
        assert_eq!(crystal_npc_sale_quote(&item, 2), Some(101));
        item.quantity = 3;
        assert_eq!(crystal_npc_sale_quote(&item, 3), Some(151));
        assert_eq!(crystal_npc_sale_quote(&item, 4), None);
        item.tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .item_index = 999;
        assert_eq!(crystal_npc_sale_quote(&item, 3), None);
        item.tooltip_source.as_mut().unwrap().user_item = None;
        assert_eq!(crystal_npc_sale_quote(&item, 3), None);
    }

    #[test]
    fn durable_zero_max_uses_the_source_zero_ratio() {
        let mut item = item();
        item.durability_current = Some(0);
        item.durability_max = Some(0);
        item.tooltip_source
            .as_mut()
            .unwrap()
            .user_item
            .as_mut()
            .unwrap()
            .added_stats
            .clear();
        assert_eq!(crystal_npc_sale_quote(&item, 1), Some(250));
        assert_eq!(
            crystal_npc_repair_quote(&item, 1.0, false).map(|quote| quote.total_price),
            Some(0)
        );
    }

    #[test]
    fn quote_keeps_the_fractional_client_affordability_threshold() {
        let item = item();
        assert_eq!(
            crystal_npc_repair_quote(&item, 0.1, false),
            Some(NpcRepairQuote {
                repair_price: 188,
                displayed_total: 18.800001,
                total_price: 18,
            })
        );
    }
}
