//! Optional SLG materials use explicit extension indices; imported Crystal
//! rows are never renamed, repurposed or edited.
use super::CrystalItemTemplate;
pub const RAW_STONE_ROUGH: i32 = 50_001;
pub const RAW_STONE_FINE: i32 = 50_002;
pub const RAW_STONE_PRECIOUS: i32 = 50_003;
pub const RAW_STONE_INDICES: [i32; 3] = [RAW_STONE_ROUGH, RAW_STONE_FINE, RAW_STONE_PRECIOUS];

pub(super) fn extend(items: &mut Vec<CrystalItemTemplate>) {
    let base = items.iter().find(|t| t.item_index == 824).expect("CopperOre template").clone();
    for (index, name) in RAW_STONE_INDICES.into_iter().zip([
        "RoughRawStone", "FineRawStone", "PreciousRawStone"]) {
        assert!(!items.iter().any(|t| t.item_index == index), "stone extension index collision");
        let mut t = base.clone();
        t.item_index = index;
        t.name = name.into();
        t.item_type = 16; // normal crafting material; no potion/spell/equipment action
        t.grade = 0;
        t.shape = 0;
        t.weight = 4;
        t.durability = 0;
        t.stack_size = 1;
        t.price = 0; // raw stones have no guaranteed NPC resale or shop source
        t.start_item = false;
        t.need_identify = false;
        t.can_mine = false;
        t.slots = 0;
        t.stats.clear();
        t.tooltip = Some("Bring this sealed stone to a mining workshop. Its contents are fixed.".into());
        items.push(t);
    }
}
