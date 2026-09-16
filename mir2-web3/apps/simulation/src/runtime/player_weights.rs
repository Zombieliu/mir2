//! Authoritative carried weights for the player status page.
//! Source: HumanObject.RefreshBagWeight/RefreshEquipmentStats/RefreshSocketStats.
use super::{items::crystal_item_template_for_item_key, resources::InventoryResource};
use crate::config::PlayerWeightsSnapshot;
use mir2_game_data::CrystalItemTemplate;
use mir2_protocol::MirClass;

fn weight(template: &CrystalItemTemplate, count: u32) -> u32 {
    // UserItem.Weight: Amulet and Bait are charged once, including a stack.
    u32::from(template.weight).saturating_mul(if matches!(template.item_type, 8 | 30) { 1 } else { count })
        .min(i32::MAX as u32)
}
fn add(total: &mut u32, value: u32) { *total = total.saturating_add(value).min(i32::MAX as u32); }
fn equipment_weight(out: &mut PlayerWeightsSnapshot, kind: u8, value: u32) {
    add(if matches!(kind, 1 | 12) { &mut out.hand } else { &mut out.wear }, value);
}

/// Unknown legacy equipment templates have no authoritative weight in their
/// saved representation. Return None rather than manufacture a zero weight.
pub(super) fn compute(inventory: &InventoryResource, level: u16, class: MirClass, riding: bool) -> Option<PlayerWeightsSnapshot> {
    let mut out = PlayerWeightsSnapshot::default();
    for item in inventory.inventory_items.iter().chain(&inventory.belt_items) {
        let template = crystal_item_template_for_item_key(&item.key)?;
        add(&mut out.bag, weight(&template, item.quantity));
    }
    for item in &inventory.equipment_items {
        let base = crystal_item_template_for_item_key(&item.key)?;
        let real = mir2_game_data::crystal_real_item_for_player(&base, level, class);
        equipment_weight(&mut out, real.item_type, weight(&base, item.quantity));
        // Broken equipment still weighs something; its sockets do not activate.
        if (base.durability > 0 && item.durability_current == 0) || matches!(base.shape, 49 | 50) { continue; }
        if base.item_type == 19 && !riding { continue; }
        for socket in &item.socketed {
            let base = crystal_item_template_for_item_key(&socket.key)?;
            let real = mir2_game_data::crystal_real_item_for_player(&base, level, class);
            // Socket weight precedes its own broken-item check in Crystal.
            equipment_weight(&mut out, real.item_type, weight(&base, socket.quantity));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EquipmentSlot, ItemContainer};
    use super::super::{equipment::equipment_state_from_item_state, items::embedded_item_state_from_template};
    fn item(kind: u8) -> super::super::items::ItemState {
        let template = mir2_game_data::crystal_item_manifest().items.into_iter()
            .find(|t| t.item_type == kind && t.weight > 0 && !matches!(t.shape,49|50)).unwrap();
        embedded_item_state_from_template(&template, ItemContainer::Bag1, 0)
    }
    #[test]
    fn player_weights_count_real_bag_belt_and_single_weight_stacks() {
        let mut inv = InventoryResource::new(80); inv.inventory_items.clear(); inv.belt_items.clear(); inv.equipment_items.clear();
        let mut potion = item(13); potion.quantity = 3;
        let mut amulet = item(8); amulet.quantity = 200;
        let mut bait = item(30); bait.quantity = 50;
        let expected = u32::from(potion.weight)*3+u32::from(amulet.weight)+u32::from(bait.weight);
        inv.inventory_items = vec![potion, bait]; inv.belt_items = vec![amulet];
        let actual = compute(&inv, 40, MirClass::Warrior, false).unwrap();
        assert_eq!(actual, PlayerWeightsSnapshot { bag: expected, hand: 0, wear: 0 });
    }
    #[test]
    fn player_weights_broken_root_and_socket_mount_gates_follow_source() {
        let mut inv = InventoryResource::new(80); inv.inventory_items.clear(); inv.belt_items.clear(); inv.equipment_items.clear();
        let weapon = item(1); let torch = item(12); let armour = item(2); let socket = item(24);
        let expected_hand = u32::from(weapon.weight)+u32::from(torch.weight);
        let expected_wear = u32::from(armour.weight);
        let mut broken = equipment_state_from_item_state(&armour, EquipmentSlot::Armour);
        broken.durability_current = 0; broken.socketed = vec![socket.clone()];
        inv.equipment_items = vec![equipment_state_from_item_state(&weapon, EquipmentSlot::Weapon), equipment_state_from_item_state(&torch, EquipmentSlot::Torch), broken];
        let actual = compute(&inv,40,MirClass::Warrior,false).unwrap();
        assert_eq!(actual.hand, expected_hand); assert_eq!(actual.wear, expected_wear);
        let mount = item(19); let mount_weight = u32::from(mount.weight); let socket_weight = u32::from(socket.weight);
        let mut equipped = equipment_state_from_item_state(&mount, EquipmentSlot::Mount); equipped.socketed = vec![socket];
        inv.equipment_items = vec![equipped];
        assert_eq!(compute(&inv,40,MirClass::Warrior,false).unwrap().wear, mount_weight);
        assert_eq!(compute(&inv,40,MirClass::Warrior,true).unwrap().wear, mount_weight+socket_weight);
    }
    #[test]
    fn player_weights_unknown_source_is_explicitly_unavailable() {
        let mut inv = InventoryResource::new(80); inv.inventory_items.clear(); inv.belt_items.clear(); inv.equipment_items.clear();
        let mut unknown = item(1); unknown.key = "missing-authoritative-template".into(); inv.inventory_items.push(unknown);
        assert!(compute(&inv,40,MirClass::Warrior,false).is_none());
    }
}
