//! Brand override layer for the original-IP reskin.
//!
//! Display-time remapping of Crystal proper nouns to original-IP names. This is
//! intentionally a *display-boundary* transform: the Crystal source manifests and
//! every internal name-keyed lookup (`crystal_monster_by_name`, drop tables,
//! recipes, respawns) keep using the original Crystal names as stable internal
//! IDs. Only the player-facing string is swapped, right before it is written into
//! an outbound `ServerPacket`.
//!
//! When `data/brand_overrides.json` is empty (the shipped default), every function
//! is an identity passthrough, so behaviour is byte-identical to an unbranded build
//! and Crystal parity tests are unaffected. Fill the override maps (slice first,
//! then in waves) to flip names to the original IP.
//!
//! Wiring status of display-name populate sites:
//! - monster: WIRED — `NewMonsterInfo` (`runtime/packets.rs`), scene snapshot
//!   (`config.rs` `VisibleMonsterRecord`), respawn announce (`runtime/monsters.rs`),
//!   zone spawns (`runtime/zone/runtime.rs`, x2).
//! - npc:     WIRED — `NewNpcInfo` + `ObjectNpc` origin (`runtime/packets.rs`),
//!   scene snapshot (`config.rs` `VisibleNpcRecord`).
//! - item:    WIRED — `runtime/items.rs::item_info_from_crystal_template`.
//! - map:     WIRED — `runtime/npc_script.rs::crystal_npc_move_map_information`.
//! - text:    WIRED — NPC dialogue via `resolve_crystal_npc_runtime_tokens`
//!   (`runtime/npc_script.rs`). Quest / other free text can be wired the same
//!   way. Note: substring substitution — curate override keys to avoid common
//!   words (longest-match-first reduces but does not eliminate false hits).
//!
//! DisplayName path (WIRED): live `ObjectMonster` and `ObjectNpc` are branded at
//! the packet field in `runtime/packets.rs` (the resolved `name` var stays
//! unbranded so `crystal_monster_effect_for_name` keeps keying on the Crystal
//! name); ground item drops are branded at *construction* in `runtime/drops.rs`
//! (override -> literal), which covers every drop-name read. Player / Hero
//! `DisplayName`s are intentionally NOT branded.
//!
//! Localization interaction: overrides key on Crystal names, so a `localized`
//! display name only flips when its resolved value still equals the Crystal key.
//! Add branded localization entries later to translate the original-IP names per
//! language (the construction-site item path already drops the stale original
//! localization when an override applies).
//!
//! DO NOT brand: `runtime/zone/packets.rs` (object-id rebase forwarders) and
//! `runtime/monsters.rs` `monster_name` (the internal drop/AI lookup key).

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Default, Clone, Deserialize)]
struct BrandOverrides {
    #[serde(default)]
    monster: HashMap<String, String>,
    #[serde(default)]
    item: HashMap<String, String>,
    #[serde(default)]
    npc: HashMap<String, String>,
    #[serde(default)]
    map: HashMap<String, String>,
    #[serde(default)]
    magic: HashMap<String, String>,
}

impl BrandOverrides {
    fn is_empty(&self) -> bool {
        self.monster.is_empty()
            && self.item.is_empty()
            && self.npc.is_empty()
            && self.map.is_empty()
            && self.magic.is_empty()
    }
}

fn overrides() -> &'static BrandOverrides {
    static OVERRIDES: OnceLock<BrandOverrides> = OnceLock::new();
    OVERRIDES.get_or_init(|| {
        serde_json::from_str(include_str!("../data/brand_overrides.json")).unwrap_or_default()
    })
}

fn remap(table: &HashMap<String, String>, raw: &str) -> String {
    table.get(raw).cloned().unwrap_or_else(|| raw.to_string())
}

/// Branded display name for a monster (falls back to the Crystal name).
pub fn monster_name(raw: &str) -> String {
    remap(&overrides().monster, raw)
}

/// Branded display name for an item.
pub fn item_name(raw: &str) -> String {
    remap(&overrides().item, raw)
}

/// Branded display name for an NPC.
pub fn npc_name(raw: &str) -> String {
    remap(&overrides().npc, raw)
}

/// Branded display title for a map / zone.
pub fn map_title(raw: &str) -> String {
    remap(&overrides().map, raw)
}

/// Branded display name for a magic / skill (player-facing string only).
pub fn magic_name(raw: &str) -> String {
    remap(&overrides().magic, raw)
}

/// Substitute branded proper nouns embedded in free text (NPC dialogue, quest
/// bodies). Longest key first, so multi-word names win over their substrings.
/// Tier 2 of the reskin; wire into dialogue emission per vertical slice.
pub fn scrub_text(input: &str) -> String {
    scrub_with(overrides(), input)
}

fn scrub_with(o: &BrandOverrides, input: &str) -> String {
    if o.is_empty() {
        return input.to_string();
    }
    let mut pairs: Vec<(&String, &String)> = o
        .monster
        .iter()
        .chain(o.item.iter())
        .chain(o.npc.iter())
        .chain(o.map.iter())
        .chain(o.magic.iter())
        .collect();
    pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    let mut out = input.to_string();
    for (from, to) in pairs {
        if out.contains(from.as_str()) {
            out = out.replace(from.as_str(), to.as_str());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_names_pass_through_regardless_of_shipped_overrides() {
        // Holds whether or not brand_overrides.json has been populated.
        let sentinel = "__unbranded_sentinel_name__";
        assert_eq!(monster_name(sentinel), sentinel);
        assert_eq!(item_name(sentinel), sentinel);
        assert_eq!(npc_name(sentinel), sentinel);
        assert_eq!(map_title(sentinel), sentinel);
        assert_eq!(magic_name(sentinel), sentinel);
    }

    #[test]
    fn remap_swaps_known_keys_only() {
        let mut table = HashMap::new();
        table.insert("ArcherGuard".to_string(), "WardenSentinel".to_string());
        assert_eq!(remap(&table, "ArcherGuard"), "WardenSentinel");
        assert_eq!(remap(&table, "BugBat"), "BugBat");
    }

    #[test]
    fn scrub_text_is_identity_when_empty() {
        let empty = BrandOverrides::default();
        assert_eq!(scrub_with(&empty, "Travel to Sabuk."), "Travel to Sabuk.");
    }

    #[test]
    fn scrub_text_replaces_longest_match_first() {
        let mut o = BrandOverrides::default();
        o.map.insert("Zuma".to_string(), "Ashgrove".to_string());
        o.map
            .insert("Zuma Temple".to_string(), "Ashgrove Shrine".to_string());
        // "Zuma Temple" must win over the shorter "Zuma".
        assert_eq!(
            scrub_with(&o, "Enter the Zuma Temple now"),
            "Enter the Ashgrove Shrine now"
        );
    }

    #[test]
    fn brand_override_keys_exist_in_their_manifests() {
        // Guards the data the reskin fills in: every override key must be a real
        // Crystal name, so a typo cannot silently fail to rebrand. Empty
        // overrides (the shipped default) pass this trivially.
        use std::collections::HashSet;
        let o = overrides();

        let monsters: HashSet<String> = crate::crystal_monster_manifest()
            .monsters
            .iter()
            .map(|m| m.name.clone())
            .collect();
        for k in o.monster.keys() {
            assert!(monsters.contains(k), "unknown monster brand key: {k:?}");
        }

        let items: HashSet<String> = crate::crystal_item_manifest()
            .items
            .iter()
            .map(|i| i.name.clone())
            .collect();
        for k in o.item.keys() {
            assert!(items.contains(k), "unknown item brand key: {k:?}");
        }

        let npcs: HashSet<String> = crate::crystal_npc_info_manifest()
            .npcs
            .iter()
            .map(|n| n.name.clone())
            .collect();
        for k in o.npc.keys() {
            assert!(npcs.contains(k), "unknown npc brand key: {k:?}");
        }

        let maps: HashSet<String> = crate::crystal_respawn_manifest()
            .maps
            .iter()
            .map(|m| m.map_title.clone())
            .collect();
        for k in o.map.keys() {
            assert!(maps.contains(k), "unknown map brand key: {k:?}");
        }

        let magics: HashSet<String> = crate::crystal_magic_manifest()
            .magics
            .iter()
            .map(|m| m.name.clone())
            .collect();
        for k in o.magic.keys() {
            assert!(magics.contains(k), "unknown magic brand key: {k:?}");
        }
    }
}
