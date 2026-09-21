use super::*;

fn priced_item() -> (ItemState, CrystalItemTemplate) {
    let mut item = item_state_from_equipment_state(
        seed_equipment_items()[0].clone(), ItemContainer::Bag1, 0,
    );
    let mut template = crystal_item_template_for_item_key(&item.key).unwrap().clone();
    template.price = 1000;
    template.durability = 1000;
    item.quantity = 1;
    item.durability_max = Some(1000);
    item.durability_current = Some(500);
    item.added_attack = 0;
    item.added_defence = 0;
    item.added_stats = vec![UserItemStat { stat: 99, value: 5 }];
    item.user_item_metadata = None;
    item.rental_owner_name.clear();
    item.rental_binding_flags = 0;
    item.rental_expiry_binary_datetime = 0;
    item.rental_locked = false;
    (item, template)
}

#[test]
fn crystal_price_uses_absolute_stat_values_and_source_rounding() {
    let (mut item, template) = priced_item();
    assert_eq!(crystal_item_added_stat_weight(&item), 5);
    assert_eq!(crystal_item_current_price(&item, &template, 5), 1312);
    assert_eq!(crystal_item_repair_price(&item, &template), 188);
    assert_eq!(crystal_npc_repair_cost(&item, &template, 1.0, true), 564);
    item.added_stats[0].value = -5;
    assert_eq!(crystal_item_repair_price(&item, &template), 188);
    item.quantity = 3;
    assert_eq!(crystal_item_repair_price(&item, &template), 564);
}

#[test]
fn crystal_repair_rental_multiplier_follows_wire_presence() {
    let (mut item, template) = priced_item();
    item.rental_owner_name = "Owner".into();
    assert_eq!(crystal_item_repair_price(&item, &template), 376);
    assert_eq!(crystal_npc_repair_cost(&item, &template, 1.5, true), 1692);
    item.rental_owner_name.clear();
    // Preserve Some(default), which is distinct from absent rental metadata.
    let mut wire = user_item_from_item_state(&item);
    wire.rental_information = Some(mir2_protocol::UserItemRentalInformation {
        owner_name: String::new(), binding_flags: 0,
        expiry_binary_datetime: 0, rental_locked: false,
    });
    item = try_item_state_from_user_item(item, &wire).unwrap();
    assert_eq!(crystal_item_repair_price(&item, &template), 376);
    item.user_item_metadata = None;
    assert_eq!(crystal_item_repair_price(&item, &template), 188);
}

#[test]
fn crystal_repair_full_and_non_durable_items_are_free() {
    let (mut item, mut template) = priced_item();
    item.durability_current = item.durability_max;
    assert_eq!(crystal_item_repair_price(&item, &template), 0);
    template.durability = 0;
    assert_eq!(crystal_item_repair_price(&item, &template), 0);
}
