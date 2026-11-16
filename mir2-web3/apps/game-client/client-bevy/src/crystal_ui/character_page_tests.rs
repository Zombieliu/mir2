use super::*;
#[test]
fn m9_fourteen_authored_equipment_cells_are_unique_and_keep_mount_thirteen() {
    let mut slots = CRYSTAL_CHARACTER_EQUIPMENT_SLOTS.map(|(slot, _)| slot);
    slots.sort();
    assert_eq!(slots, std::array::from_fn::<_, 14, _>(|i| i as u32));
    assert_eq!(
        CRYSTAL_CHARACTER_EQUIPMENT_SLOTS[3],
        (13, CrystalRect::new(211., 152., 36., 32.))
    );
    assert!(CRYSTAL_CHARACTER_EQUIPMENT_SLOTS
        .iter()
        .all(|(_, r)| r.width == 36. && r.height == 32.));
}
#[test]
fn m9_three_classes_two_genders_use_source_hair_without_guessing() {
    for class in ["warrior", "wizard", "taoist"] {
        assert_eq!(
            crystal_character_hair_frame(Some(class), Some("male"), Some(0))
                .unwrap()
                .index,
            441
        );
        assert_eq!(
            crystal_character_hair_frame(Some(class), Some("female"), Some(0))
                .unwrap()
                .index,
            481
        );
    }
    assert_eq!(
        crystal_character_hair_frame(Some("assassin"), Some("male"), Some(0))
            .unwrap()
            .rect,
        CrystalRect::new(131., 172., 16., 21.)
    );
    assert!(crystal_character_hair_frame(Some("warrior"), None, Some(0)).is_none());
    assert!(crystal_character_hair_frame(Some("warrior"), Some("male"), None).is_none());
    assert!(crystal_character_hair_frame(Some("warrior"), Some("male"), Some(9)).is_none());
}
#[test]
fn m9_shared_layers_keep_wing_armour_weapon_helmet_order_and_helmet_suppression() {
    let mut inventory = InventoryModel::default();
    inventory.items.clear();
    for slot in [0, 1, 2] {
        inventory.items.push(ItemModel {
            container: 2,
            slot,
            state_image: 30 + slot as u16,
            state_image_width: 28,
            state_image_height: 57,
            ..default()
        });
    }
    let mut ui = UiReadModel::default();
    ui.player.gender = Some("male".into());
    ui.player.class_name = Some("warrior".into());
    ui.player.hair = Some(0);
    ui.player.wing_effect = Some(1);
    let layers = crystal_character_paper_doll_layers(&inventory, &ui);
    assert_eq!(
        layers.iter().map(|l| l.frame.index).collect::<Vec<_>>(),
        vec![1202, 31, 30, 32]
    );
    assert_eq!(layers[0].blend, CrystalCharacterBlend::DrawBlend);
    inventory.items.retain(|i| i.slot != 1);
    assert!(crystal_character_paper_doll_layers(&inventory, &ui)
        .iter()
        .all(|l| l.blend == CrystalCharacterBlend::Alpha));
    inventory
        .items
        .iter_mut()
        .find(|i| i.slot == 2)
        .unwrap()
        .state_image_width = 0;
    assert!(crystal_character_paper_doll_layers(&inventory, &ui)
        .iter()
        .all(|l| l.frame.library != "Prguse"));
    inventory.items.retain(|i| i.slot != 2);
    assert_eq!(
        crystal_character_paper_doll_layers(&inventory, &ui)
            .last()
            .unwrap()
            .frame
            .index,
        441
    );
}
#[test]
fn m9_state_item_geometry_is_intrinsic_and_missing_frame_suppresses_layer() {
    let mut item:ItemModel=serde_json::from_value(serde_json::json!({"key":"a","name":"a","quantity":1,"slot":0,"container":2,
        "stateImage":30,"stateImageX":75,"stateImageY":186,"stateImageWidth":28,"stateImageHeight":57})).unwrap();
    assert_eq!(
        crystal_character_state_item_frame(&item).unwrap().rect,
        CrystalRect::new(75., 186., 28., 57.)
    );
    item.state_image_x = -19;
    assert_eq!(
        crystal_character_state_item_frame(&item).unwrap().rect.left,
        -19.
    );
    item.state_image_width = 0;
    assert!(crystal_character_state_item_frame(&item).is_none());
}
