//! On-chain command landing (M3, WF-4) — applies chain-confirmed events relayed from the
//! Sui mine (DESIGN §4) to the authoritative Sim: grant ore to the bag, credit gold.
//!
//! Server stays authoritative: these land ONLY on chain-confirmed events injected by the
//! trusted Relayer (never on the raw player path — see `validate_production_player_command`).
//! Idempotency place #3 (DESIGN §0.2): every command carries an idempotency key
//! (`tx_digest:event_seq`); a duplicate is a strict no-op. This is additive — it never
//! touches P0 server-mining (`runtime/mining.rs`: stones / payout / regen), which keeps
//! working unchanged.

use std::collections::HashSet;

use bevy_ecs::prelude::{Resource, World};
use mir2_game_data::crystal_item_by_name;
use mir2_protocol::ServerPacket;

use super::drops::can_gain_gold;
use super::inventory::{add_or_increment_item_with_random_metadata, can_gain_item_quantity};
use super::items::{crystal_item_key_for_template, user_item_from_item_state};
use super::resources::{InventoryResource, PlayerRuntimeResource};
use crate::config::ItemContainer;

/// Idempotency place #3: keys of chain-confirmed commands already applied this session.
#[derive(Resource, Debug, Clone, Default)]
pub(super) struct OnchainCommandLog {
    applied: HashSet<String>,
}

impl OnchainCommandLog {
    /// Claim a key: returns `true` the first time (and records it), `false` if already
    /// applied. The dedup gate every on-chain command passes through.
    fn claim(&mut self, key: &str) -> bool {
        self.applied.insert(key.to_string())
    }
}

/// Claim `key` against the world's command log, inserting the log on first use.
fn claim_key(world: &mut World, key: &str) -> bool {
    world
        .get_resource_or_insert_with(OnchainCommandLog::default)
        .claim(key)
}

/// Map an on-chain `OreKind` variant to its Crystal ore item name (mine set 1 = BlackIron
/// sink + Gold/Silver/Copper; set 2 = Platinum/Ruby/Nephrite/Amethyst). Mirrors the names
/// in `mining.rs` `builtin_mine_sets`.
fn ore_item_name(ore_kind: &str) -> Option<&'static str> {
    Some(match ore_kind {
        "BlackIron" => "BlackIronOre",
        "Gold" => "GoldOre",
        "Silver" => "SilverOre",
        "Copper" => "CopperOre",
        "Platinum" => "PlatinumOre",
        "Ruby" => "RubyOre",
        "Nephrite" => "NephriteOre",
        "Amethyst" => "AmethystOre",
        _ => return None,
    })
}

/// Crystal ore durability is the unit count * 1000; cap each item at u16::MAX, so a single
/// ore item carries at most ~65 units. Larger grants are split across items.
const ORE_UNITS_PER_ITEM: u64 = (u16::MAX as u64) / 1000;

/// `GrantOnchainOre`: add `amount` units of `ore_kind` ore to the player's bag, mirroring
/// the P0 `give_mine_payout` item shape (ore dura = units * 1000). Idempotent on `key`.
/// Returns one `GainedItem` per ore item added; empty on duplicate / unknown ore / no
/// space. The chain ledger remains the source of truth if the bag is full (M6 hardening:
/// mail the remainder).
pub(super) fn grant_onchain_ore(
    world: &mut World,
    ore_kind: &str,
    amount: u64,
    key: &str,
) -> Vec<ServerPacket> {
    if amount == 0 {
        return Vec::new();
    }
    if !claim_key(world, key) {
        return Vec::new(); // idempotency place #3: already applied
    }
    let Some(name) = ore_item_name(ore_kind) else {
        return Vec::new();
    };
    let Some(template) = crystal_item_by_name(name) else {
        return Vec::new();
    };
    let item_key = crystal_item_key_for_template(&template);
    let description = template
        .tooltip
        .clone()
        .unwrap_or_else(|| format!("{} delivered from the on-chain mine.", template.name));

    let mut packets = Vec::new();
    let mut remaining = amount;
    while remaining > 0 {
        let units = remaining.min(ORE_UNITS_PER_ITEM);
        let dura = units.saturating_mul(1_000).min(u64::from(u16::MAX)) as u16;
        {
            let inventory = world.resource::<InventoryResource>();
            if !can_gain_item_quantity(inventory, ItemContainer::Bag1, &item_key, 1) {
                break; // bag full — stop; chain ledger is authoritative for the remainder
            }
        }
        let item = add_or_increment_item_with_random_metadata(
            world,
            ItemContainer::Bag1,
            &item_key,
            &template.name,
            &description,
            30,
            1,
            u16::from(template.weight.max(1)),
            Some(dura),
            Some(dura.max(1)),
            0,
            0,
            Vec::new(),
            false,
            0,
        );
        packets.push(ServerPacket::GainedItem {
            item: user_item_from_item_state(&item),
        });
        remaining -= units;
    }
    packets
}

/// `CreditGoldFromOre`: credit `gold` to the player (from redeeming on-chain ore — the
/// ore→gold RATE was applied upstream/at M5). Idempotent on `key`. Returns `GainedGold`.
pub(super) fn credit_gold_from_ore(world: &mut World, gold: u32, key: &str) -> Vec<ServerPacket> {
    if gold == 0 {
        return Vec::new();
    }
    if !claim_key(world, key) {
        return Vec::new();
    }
    if !can_gain_gold(world.resource::<PlayerRuntimeResource>(), gold) {
        return Vec::new(); // would overflow the u32 gold cap
    }
    {
        let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
        runtime.gold = runtime.gold.saturating_add(gold);
    }
    vec![ServerPacket::GainedGold { gold }]
}
