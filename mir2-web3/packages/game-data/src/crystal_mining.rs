//! Original MapInfo.MineIndex/MineZones and Configs/Mines.ini. No synthetic
//! floor veins are introduced. Four inconsistent source item names have an
//! explicit, identity-checked compatibility mapping below.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMineZone {
    pub x: i32,
    pub y: i32,
    pub size: u16,
    pub mine_index: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMiningMap {
    pub map_index: i32,
    pub map_file_name: String,
    pub map_title: String,
    pub mine_index: u8,
    pub mine_zones: Vec<CrystalMineZone>,
}

impl CrystalMiningMap {
    /// CreateMine applies rectangle overrides before the whole-map fallback.
    /// Eligibility still requires an in-bounds, statically blocked wall cell.
    pub fn mine_index_at(&self, x: i32, y: i32) -> u8 {
        self.mine_zones
            .iter()
            .rev()
            .find(|zone| {
                let size = i32::from(zone.size);
                x >= zone.x - size && x < zone.x + size && y >= zone.y - size && y < zone.y + size
            })
            .map_or(self.mine_index, |zone| zone.mine_index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMineDrop {
    pub item_name: String,
    pub min_slot: u16,
    pub max_slot: u16,
    pub min_dura: u16,
    pub max_dura: u16,
    pub bonus_chance: u8,
    pub max_bonus_dura: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalMineSet {
    pub mine_index: u8,
    pub name: String,
    pub spot_regen_rate_minutes: u8,
    pub max_stones: u8,
    pub hit_rate: u8,
    pub drop_rate: u8,
    pub total_slots: u16,
    pub drops: Vec<CrystalMineDrop>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrystalMiningManifest {
    pub schema_version: u32,
    pub crystal_db_version: u32,
    pub crystal_db_custom_version: u32,
    pub source_map_count: usize,
    pub source_db_sha256: String,
    pub source_mines_sha256: String,
    pub maps: Vec<CrystalMiningMap>,
    pub mine_sets: Vec<CrystalMineSet>,
}

pub fn crystal_mining_manifest() -> &'static CrystalMiningManifest {
    static MANIFEST: OnceLock<CrystalMiningManifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../data/generated/crystal_mining_manifest.json"
        ))
        .expect("source-bound mining manifest must be valid")
    })
}

pub fn crystal_mining_map(file_name: &str) -> Option<&'static CrystalMiningMap> {
    let file_name = file_name
        .trim()
        .strip_suffix(".map")
        .unwrap_or(file_name.trim());
    crystal_mining_manifest()
        .maps
        .iter()
        .find(|map| map.map_file_name.eq_ignore_ascii_case(file_name))
}

pub fn crystal_mine_set(index: u8) -> Option<&'static CrystalMineSet> {
    crystal_mining_manifest()
        .mine_sets
        .iter()
        .find(|set| set.mine_index == index)
}

/// The original Mines.ini uses four `*Ore` names absent from its own MirDB.
/// Repair only those exact source identities; never create a synthetic item.
pub fn crystal_mining_item(name: &str) -> Option<crate::CrystalItemTemplate> {
    if let Some(item) = crate::crystal_item_by_name(name) {
        return Some(item);
    }
    let (index, actual) = match name {
        "PlatinumOre" => (831, "Platinum"),
        "RubyOre" => (830, "Ruby"),
        "NephriteOre" => (832, "Nephrite"),
        "AmethystOre" => (829, "Amethyst"),
        _ => return None,
    };
    crate::crystal_item_by_index(index).filter(|item| item.name == actual && item.item_type == 14)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_mining_metadata_matches_database_and_legal_mines() {
        let m = crystal_mining_manifest();
        assert_eq!(
            (m.source_map_count, m.maps.len(), m.mine_sets.len()),
            (464, 23, 2)
        );
        assert_eq!(
            m.source_db_sha256,
            "829121f4d762ea4427325a767e326a9c04794eb51450ada8a72a0b857705f075"
        );
        assert_eq!(
            m.source_mines_sha256,
            "4b9ca19d06832da42abfc2c9cdcbfed746825c3c28ff8149c8a7ffc7d579d1c9"
        );
        assert_eq!(
            crystal_mining_map("D401").unwrap().mine_index_at(20, 181),
            1
        );
        assert_eq!(crystal_mining_map("d2031.map").unwrap().mine_index, 2);
        assert!(crystal_mining_map("0").is_none());
        for set in &m.mine_sets {
            assert_eq!(
                (
                    set.spot_regen_rate_minutes,
                    set.max_stones,
                    set.hit_rate,
                    set.drop_rate
                ),
                (5, 80, 25, 10)
            );
        }
    }

    #[test]
    fn mineral_name_repair_uses_only_the_original_item_identities() {
        for (name, id) in [
            ("PlatinumOre", 831),
            ("RubyOre", 830),
            ("NephriteOre", 832),
            ("AmethystOre", 829),
        ] {
            let item = crystal_mining_item(name).unwrap();
            assert_eq!(
                (
                    item.item_index,
                    item.item_type,
                    item.durability,
                    item.stack_size
                ),
                (id, 14, 10_000, 1)
            );
        }
        assert!(crystal_mining_item("DiamondOre").is_none());
    }

    #[test]
    fn overlapping_mine_rectangles_use_last_source_override_and_exclusive_edges() {
        let map = CrystalMiningMap {
            map_index: 1,
            map_file_name: "fixture".into(),
            map_title: "".into(),
            mine_index: 1,
            mine_zones: vec![
                CrystalMineZone {
                    x: 20,
                    y: 20,
                    size: 5,
                    mine_index: 2,
                },
                CrystalMineZone {
                    x: 22,
                    y: 22,
                    size: 3,
                    mine_index: 0,
                },
            ],
        };
        assert_eq!(map.mine_index_at(19, 19), 0);
        assert_eq!(map.mine_index_at(15, 15), 2);
        assert_eq!(map.mine_index_at(25, 24), 1);
        assert_eq!(map.mine_index_at(24, 25), 1);
    }

    #[test]
    fn classic_profile_carries_all_admitted_mines_and_original_minerals() {
        let profile = crate::platinum_176_profile();
        let maps: Vec<_> = crystal_mining_manifest()
            .maps
            .iter()
            .filter(|mine| {
                profile
                    .map_whitelist
                    .iter()
                    .any(|map| map.file_name == mine.map_file_name)
            })
            .collect();
        assert_eq!(maps.len(), 14);
        for name in [
            "PickAxe",
            "CopperOre",
            "SilverOre",
            "GoldOre",
            "BlackIronOre",
            "Amethyst",
            "Ruby",
            "Platinum",
            "Nephrite",
        ] {
            assert!(
                profile.item_whitelist.iter().any(|item| item == name),
                "{name} must not be filtered out"
            );
        }
        assert_eq!(crate::platinum_176_profile_bundle().summary.items, 203);
    }
}
