//! Pure admission for the fixed Windows NPC Pearl-buy path.
//! A quote is the displayed unit price multiplied by the clamped quantity,
//! not proof of server cost, purchase completion, delivery or an economic ACK.
//! Hosts own raw catalog custody, wallet provenance and transport barriers.

pub const NPC_PEARL_QUANTITY_MIN: u16 = 1;
pub const NPC_PEARL_QUANTITY_MAX: u16 = 99;
pub const NPC_PEARL_BAG_ENTRY_LIMIT: usize = 46;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcPearlGood {
    /// Raw NPCPearlGoods UserItem identity, including zero.
    pub unique_id: u64,
    pub use_pearls: bool,
    pub unit_price: u32,
    /// Every negative signed stock value is unlimited on this Windows path.
    pub stock: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcPearlBuyFacts {
    pub allows_buy: bool,
    pub selected: Option<NpcPearlGood>,
    pub quantity: u16,
    /// None is unknown, including for a zero-price good. Native retains u32.
    pub balance: Option<u32>,
    /// Number of container-0 item entries, not stack slack or Bag capacity.
    pub occupied_bag_entries: usize,
}

/// Stable ABI1 reason codes; zero in the adapter means admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NpcPearlBuyDeny {
    ServiceUnavailable = 1,
    NoSelection = 2,
    NotPearl = 3,
    UnknownWallet = 4,
    InsufficientStock = 5,
    InsufficientPearls = 6,
    BagFull = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcPearlBuyPlan {
    /// Preserve the fixed Windows quantity widget limit, independent of stock.
    pub max_quantity: u16,
    pub quote: Option<u32>,
    pub admitted_count: Option<u16>,
    pub item_index: Option<u64>,
    pub denial: Option<NpcPearlBuyDeny>,
}

/// The authoritative Gateway/Windows displayed unit projection uses f32.
/// It is separate from the server's total purchase cost and is never an ACK.
pub fn npc_pearl_catalog_unit_price(info_price: u32, rate: f32) -> Option<u32> {
    if !rate.is_finite() || rate < 0.0 { return None; }
    Some((info_price as f32 * rate).floor() as u32)
}

pub fn plan_npc_pearl_buy(facts: NpcPearlBuyFacts) -> NpcPearlBuyPlan {
    use NpcPearlBuyDeny as Deny;
    let count = facts.quantity.clamp(NPC_PEARL_QUANTITY_MIN, NPC_PEARL_QUANTITY_MAX);
    let quote = facts.selected.filter(|good| good.use_pearls)
        .map(|good| good.unit_price.saturating_mul(u32::from(count)));
    let denied = |denial| NpcPearlBuyPlan {
        max_quantity: NPC_PEARL_QUANTITY_MAX, quote,
        admitted_count: None, item_index: None, denial: Some(denial),
    };
    if !facts.allows_buy { return denied(Deny::ServiceUnavailable); }
    let Some(good) = facts.selected else { return denied(Deny::NoSelection); };
    if !good.use_pearls { return denied(Deny::NotPearl); }
    if good.stock >= 0 && (good.stock as u32) < u32::from(count) {
        return denied(Deny::InsufficientStock);
    }
    let Some(balance) = facts.balance else { return denied(Deny::UnknownWallet); };
    let Some(total) = quote else { return denied(Deny::NotPearl); };
    if balance < total {
        return denied(Deny::InsufficientPearls);
    }
    if facts.occupied_bag_entries >= NPC_PEARL_BAG_ENTRY_LIMIT {
        return denied(Deny::BagFull);
    }
    NpcPearlBuyPlan {
        max_quantity: NPC_PEARL_QUANTITY_MAX, quote,
        admitted_count: Some(count), item_index: Some(good.unique_id), denial: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> NpcPearlBuyFacts {
        NpcPearlBuyFacts {
            allows_buy: true,
            selected: Some(NpcPearlGood { unique_id: 0, use_pearls: true, unit_price: 10, stock: -1 }),
            quantity: 2, balance: Some(20), occupied_bag_entries: 0,
        }
    }

    #[test]
    fn pearl_quantity_clamp_and_quote_preserve_windows_saturation() {
        let mut input = facts(); input.quantity = 0;
        let plan = plan_npc_pearl_buy(input);
        assert_eq!((plan.max_quantity, plan.quote, plan.admitted_count), (99, Some(10), Some(1)));
        input.quantity = u16::MAX; input.balance = Some(990);
        let plan = plan_npc_pearl_buy(input);
        assert_eq!((plan.quote, plan.admitted_count), (Some(990), Some(99)));
        input.selected.as_mut().unwrap().unit_price = u32::MAX;
        input.balance = Some(u32::MAX);
        let plan = plan_npc_pearl_buy(input);
        assert_eq!((plan.quote, plan.admitted_count), (Some(u32::MAX), Some(99)));
        input.balance = Some(u32::MAX - 1);
        assert_eq!(plan_npc_pearl_buy(input).denial, Some(NpcPearlBuyDeny::InsufficientPearls));
    }

    #[test]
    fn pearl_signed_stock_negative_is_unlimited_and_finite_stock_is_exact() {
        let mut input = facts();
        for stock in [-1, -2, i32::MIN] {
            input.selected.as_mut().unwrap().stock = stock;
            assert_eq!(plan_npc_pearl_buy(input).admitted_count, Some(2));
        }
        for stock in [0, 1] {
            input.selected.as_mut().unwrap().stock = stock;
            assert_eq!(plan_npc_pearl_buy(input).denial, Some(NpcPearlBuyDeny::InsufficientStock));
        }
        input.selected.as_mut().unwrap().stock = 2;
        assert_eq!(plan_npc_pearl_buy(input).admitted_count, Some(2));
    }

    #[test]
    fn pearl_unknown_wallet_rejects_free_goods_and_native_u32_range_survives() {
        let mut input = facts(); input.balance = None;
        input.selected.as_mut().unwrap().unit_price = 0;
        assert_eq!(plan_npc_pearl_buy(input).denial, Some(NpcPearlBuyDeny::UnknownWallet));
        input.balance = Some(0);
        assert_eq!(plan_npc_pearl_buy(input).admitted_count, Some(2));
        input.selected.as_mut().unwrap().unit_price = i32::MAX as u32 + 1;
        input.quantity = 1; input.balance = Some(i32::MAX as u32 + 1);
        assert_eq!(plan_npc_pearl_buy(input).admitted_count, Some(1));
        input.balance = Some(i32::MAX as u32);
        assert_eq!(plan_npc_pearl_buy(input).denial, Some(NpcPearlBuyDeny::InsufficientPearls));
    }

    #[test]
    fn pearl_bag_uses_fixed_entry_boundary_without_gold_stack_slack() {
        let mut input = facts(); input.occupied_bag_entries = 45;
        assert_eq!(plan_npc_pearl_buy(input).admitted_count, Some(2));
        for occupied in [46, 47, usize::MAX] {
            input.occupied_bag_entries = occupied;
            let plan = plan_npc_pearl_buy(input);
            assert_eq!(plan.denial, Some(NpcPearlBuyDeny::BagFull));
            assert_eq!((plan.quote, plan.admitted_count, plan.item_index), (Some(20), None, None));
        }
    }

    #[test]
    fn pearl_service_selection_and_currency_facts_fail_closed() {
        let mut input = facts(); input.allows_buy = false;
        assert_eq!(plan_npc_pearl_buy(input).denial, Some(NpcPearlBuyDeny::ServiceUnavailable));
        input.allows_buy = true; input.selected = None;
        let plan = plan_npc_pearl_buy(input);
        assert_eq!((plan.quote, plan.denial), (None, Some(NpcPearlBuyDeny::NoSelection)));
        input = facts(); input.selected.as_mut().unwrap().use_pearls = false;
        let plan = plan_npc_pearl_buy(input);
        assert_eq!((plan.quote, plan.denial), (None, Some(NpcPearlBuyDeny::NotPearl)));
    }

    #[test]
    fn pearl_admission_preserves_raw_uid_zero_and_full_native_identity() {
        let mut input = facts();
        for unique_id in [0, 1, u64::MAX] {
            input.selected.as_mut().unwrap().unique_id = unique_id;
            let plan = plan_npc_pearl_buy(input);
            assert_eq!((plan.item_index, plan.admitted_count, plan.denial), (Some(unique_id), Some(2), None));
        }
    }
    #[test]
    fn pearl_catalog_unit_price_preserves_gateway_f32_projection_and_real_free_goods() {
        assert_eq!(npc_pearl_catalog_unit_price(101, 1.25), Some(126));
        assert_eq!(npc_pearl_catalog_unit_price(0, 1.0), Some(0));
        assert_eq!(npc_pearl_catalog_unit_price(100, 0.0), Some(0));
        assert_eq!(npc_pearl_catalog_unit_price(16_777_217, 1.0), Some(16_777_216));
        assert_eq!(npc_pearl_catalog_unit_price(u32::MAX, 1.0), Some(u32::MAX));
        assert_eq!(npc_pearl_catalog_unit_price(u32::MAX, f32::MAX), Some(u32::MAX));
        for rate in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(npc_pearl_catalog_unit_price(100, rate), None);
        }
    }

}
