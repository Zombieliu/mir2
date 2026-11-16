use super::*;
use crate::inventory::{CrystalItemTooltipSourceModel, CrystalUserItemModel, ItemModel};
fn model() -> HeroModel {
    let info=serde_json::from_value(serde_json::json!({"object_id":12,"name":"Hero","class":"Warrior","gender":"Male","level":20,"hair":0,"hp":100,"mp":10,"experience":0,"max_experience":100,"inventory":[],"equipment":[],"magics":[],"auto_pot":false,"auto_hp_percent":30,"auto_mp_percent":30,"hp_item_index":0,"mp_item_index":0})).unwrap();
    let mut m = HeroModel {
        info: Some(info),
        ..Default::default()
    };
    m.info.as_mut().unwrap().inventory = Some(vec![None; 10]);
    m.info.as_mut().unwrap().equipment = Some(vec![None; 14]);
    m
}
fn add(m: &mut HeroModel, slot: u8, uid: u64, count: u16, kind: u8) {
    let raw = CrystalUserItemModel {
        unique_id: uid,
        item_index: 658,
        count,
        ..Default::default()
    };
    m.info.as_mut().unwrap().inventory.as_mut().unwrap()[slot as usize] =
        Some(serde_json::from_value(serde_json::to_value(raw).unwrap()).unwrap());
    let mut source = CrystalItemTooltipSourceModel::default();
    source.info.item_index = 658;
    source.info.item_type = kind;
    source.info.required_class = 31;
    source.info.required_gender = 3;
    source.info.stack_size = 20;
    m.inventory_view.items.push(ItemModel {
        container: 0,
        slot: u32::from(slot),
        unique_id: Some(uid),
        quantity: u32::from(count),
        tooltip_source: Some(source),
        ..Default::default()
    });
}
#[test]
fn auto_equip_and_potion_plans_keep_exact_hero_grid_and_uid() {
    let mut m = model();
    add(&mut m, 2, 71, 1, 1);
    assert_eq!(
        plan(&m, 2),
        HeroUsePlan::Packet(C::EquipItem {
            grid: G::HeroInventory,
            unique_id: 71,
            to: 0
        })
    );
    m.inventory_view.items[0]
        .tooltip_source
        .as_mut()
        .unwrap()
        .info
        .item_type = 13;
    assert_eq!(
        plan(&m, 2),
        HeroUsePlan::Packet(C::UseItem {
            grid: G::HeroInventory,
            unique_id: 71
        })
    );
    m.inventory_view.items[0]
        .tooltip_source
        .as_mut()
        .unwrap()
        .info
        .shape = 4;
    assert_eq!(
        plan(&m, 2),
        HeroUsePlan::ConfirmPotion(C::UseItem {
            grid: G::HeroInventory,
            unique_id: 71
        })
    );
}
#[test]
fn restock_waits_for_empty_belt_and_revalidates_source_uid() {
    let mut m = model();
    add(&mut m, 0, 71, 1, 13);
    add(&mut m, 2, 72, 5, 13);
    let c = restock_candidate(&m, 0).unwrap();
    assert_eq!(c, (0, 2, 72));
    assert!(!restock_ready(&m, c));
    m.info.as_mut().unwrap().inventory.as_mut().unwrap()[0] = None;
    assert!(restock_ready(&m, c));
    m.info.as_mut().unwrap().inventory.as_mut().unwrap()[2]
        .as_mut()
        .unwrap()
        .unique_id = 99;
    assert!(!restock_ready(&m, c));
}
#[test]
fn remove_uses_raw_belt_fallback_without_negative_index() {
    let mut m = model();
    for slot in 2..10 {
        add(&mut m, slot, 70 + u64::from(slot), 1, 1);
    }
    let item = m.info.as_ref().unwrap().inventory.as_ref().unwrap()[2].clone();
    m.info.as_mut().unwrap().equipment.as_mut().unwrap()[0] = item;
    assert_eq!(
        remove_plan(&m, 0),
        Some(C::RemoveItem {
            grid: G::HeroInventory,
            unique_id: 72,
            to: 0
        })
    );
}
#[test]
fn player_hero_transfers_reject_occupied_incompatible_and_preserve_stack_uids() {
    use super::super::cross::{transfer_packet, Cell};
    let mut m = model();
    add(&mut m, 0, 71, 1, 13);
    add(&mut m, 2, 72, 5, 13);
    let a = &m.inventory_view.items[0];
    let b = &m.inventory_view.items[1];
    assert_eq!(
        transfer_packet(Cell::Player(3), Cell::Hero(1), a, None),
        Some(C::TransferHeroItem { from: 3, to: 1 })
    );
    assert_eq!(
        transfer_packet(Cell::Hero(1), Cell::Player(3), a, None),
        Some(C::TakeBackHeroItem { from: 1, to: 3 })
    );
    assert_eq!(
        transfer_packet(Cell::Player(3), Cell::Hero(2), a, Some(b)),
        Some(C::MergeItem {
            grid_from: G::Inventory,
            grid_to: G::HeroInventory,
            id_from: 71,
            id_to: 72
        })
    );
    let mut full = b.clone();
    full.quantity = 20;
    assert!(transfer_packet(Cell::Player(3), Cell::Hero(2), a, Some(&full)).is_none());
}
