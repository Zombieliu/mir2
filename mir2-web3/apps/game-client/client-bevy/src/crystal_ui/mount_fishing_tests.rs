use super::*;
use crate::inventory::CrystalItemInfoModel;
fn item(uid: u64, kind: u8, shape: i16, container: u8, slot: u32, slots: usize) -> ItemModel {
    ItemModel {
        unique_id: Some(uid),
        container,
        slot,
        name: format!("item{uid}"),
        tooltip_source: Some(CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel {
                item_type: kind,
                shape,
                ..Default::default()
            },
            user_item: Some(CrystalUserItemModel {
                unique_id: uid,
                slots: vec![None; slots],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn bag() -> InventoryModel {
    InventoryModel {
        items: vec![
            item(100, 19, 2, 2, 13, 4),
            item(200, 1, 49, 2, 0, 5),
            item(300, 22, 0, 0, 1, 0),
            item(400, 28, 0, 0, 2, 0),
        ],
        ..Default::default()
    }
}
#[test]
fn equipment_gate_uses_actual_type_shape_slot_and_identity() {
    let mut ui = MountFishingUi::default();
    assert!(!ui.show(EquipmentDialog::Mount));
    assert_eq!(ui.notice, Some(EquipmentNotice::NoMount));
    assert!(!ui.show(EquipmentDialog::Fishing));
    assert_eq!(ui.notice, Some(EquipmentNotice::NoFishingRod));
    let mut inventory = bag();
    ui.sync_equipment(&inventory);
    assert!(ui.show(EquipmentDialog::Mount));
    assert!(ui.show(EquipmentDialog::Fishing));
    inventory.items[1]
        .tooltip_source
        .as_mut()
        .unwrap()
        .info
        .shape = 48;
    ui.sync_equipment(&inventory);
    assert!(ui.rod.is_none());
    assert!(!ui.fishing_open);
    inventory.items[0].unique_id = Some(999);
    ui.sync_equipment(&inventory);
    assert!(ui.mount.is_none());
    assert!(!ui.mount_open);
}
#[test]
fn attachment_intent_preserves_host_identity_and_waits_for_exact_ack() {
    let inventory = bag();
    let mut ui = MountFishingUi::default();
    ui.sync_equipment(&inventory);
    assert!(ui
        .equip(EquipmentDialog::Mount, &inventory, 400, 0)
        .is_none());
    let packet = ui
        .equip(EquipmentDialog::Mount, &inventory, 300, 0)
        .unwrap();
    assert_eq!(
        packet,
        ClientPacket::EquipSlotItem {
            grid: MirGridType::Inventory,
            unique_id: 300,
            to: 0,
            grid_to: MirGridType::Mount,
            to_unique_id: 100
        }
    );
    assert!(ui
        .equip(EquipmentDialog::Fishing, &inventory, 400, 0)
        .is_none());
    assert!(!ui.observe(
        1,
        &ServerPacket::EquipSlotItem {
            grid: MirGridType::Inventory,
            unique_id: 400,
            to: 0,
            grid_to: MirGridType::Mount,
            success: true
        },
        1000
    ));
    assert!(ui.pending());
    assert!(ui.observe(
        1,
        &ServerPacket::EquipSlotItem {
            grid: MirGridType::Inventory,
            unique_id: 300,
            to: 0,
            grid_to: MirGridType::Mount,
            success: false
        },
        1000
    ));
    assert!(!ui.pending());
    assert!(!ui.mount.as_ref().unwrap().has_slot(0));
    let fishing = ui
        .equip(EquipmentDialog::Fishing, &inventory, 400, 0)
        .unwrap();
    ui.release_unsent(&packet);
    assert!(ui.pending());
    ui.release_unsent(&fishing);
    assert!(!ui.pending());
}
#[test]
fn removal_uses_raw_uid_even_when_catalog_label_is_missing() {
    let mut inventory = bag();
    inventory.items[1]
        .tooltip_source
        .as_mut()
        .unwrap()
        .user_item
        .as_mut()
        .unwrap()
        .slots[0] = Some(CrystalUserItemModel {
        unique_id: 777,
        ..Default::default()
    });
    let mut ui = MountFishingUi::default();
    ui.sync_equipment(&inventory);
    assert!(ui.rod.as_ref().unwrap().slots[0].is_none());
    assert_eq!(
        ui.remove(EquipmentDialog::Fishing, 0, 16),
        Some(ClientPacket::RemoveSlotItem {
            grid: MirGridType::Fishing,
            grid_to: MirGridType::Inventory,
            unique_id: 777,
            to: 16,
            from_unique_id: 200
        })
    );
}
#[test]
fn fishing_cast_requires_rod_water_facing_standing_transform_and_cooldown() {
    let mut ui = MountFishingUi::default();
    assert!(ui.cast(1000, true, true, 0).is_none());
    ui.sync_equipment(&bag());
    assert!(ui.cast(1000, false, true, 0).is_none());
    assert!(ui.cast(1000, true, false, 0).is_none());
    assert!(ui.cast(1000, true, true, 6).is_none());
    ui.standing = false;
    assert!(ui.cast(1000, true, true, 0).is_none());
    ui.standing = true;
    assert_eq!(
        ui.cast(1000, true, true, 0),
        Some(ClientPacket::FishingCast { cast_out: true })
    );
    assert!(ui.cast(1999, true, true, 0).is_none());
    assert!(ui.cast(2000, true, true, 0).is_some());
    ui.fishing = true;
    assert!(ui.cast(4000, true, true, 0).is_none());
}
#[test]
fn status_is_owner_authoritative_and_reel_removal_disables_auto() {
    let mut inventory = bag();
    inventory.items[1]
        .tooltip_source
        .as_mut()
        .unwrap()
        .user_item
        .as_mut()
        .unwrap()
        .slots[4] = Some(CrystalUserItemModel {
        unique_id: 888,
        ..Default::default()
    });
    let mut ui = MountFishingUi::default();
    ui.sync_equipment(&inventory);
    let packet = ServerPacket::FishingUpdate {
        object_id: 1,
        fishing: true,
        progress_percent: 120,
        chance_percent: -1,
        fishing_point: mir2_protocol::Point { x: 0, y: 0 },
        found_fish: true,
    };
    assert!(!ui.observe(2, &packet, 1000));
    assert!(!ui.status_open);
    assert!(ui.observe(1, &packet, 1000));
    assert!(ui.status_open);
    assert_eq!((ui.progress, ui.chance), (100, 0));
    assert_eq!(
        ui.action(EquipmentAction::AutoCast, 1000),
        Some(EquipmentIntent::Packet(
            ClientPacket::FishingChangeAutocast { auto_cast: true }
        ))
    );
    assert!(ui.escape(1000).is_none());
    ui.action(EquipmentAction::EscapeToggle, 1000);
    assert_eq!(
        ui.escape(1000),
        Some(EquipmentIntent::Packet(ClientPacket::FishingCast {
            cast_out: false
        }))
    );
    assert_eq!(
        ui.sync_equipment(&bag()),
        Some(ClientPacket::FishingChangeAutocast { auto_cast: false })
    );
    assert!(!ui.auto_cast);
}
#[test]
fn reel_requires_the_current_owner_bite_and_shares_one_pending_cancel() {
    let update = |fishing, found_fish| ServerPacket::FishingUpdate {
        object_id: 1,
        fishing,
        progress_percent: 100,
        chance_percent: 38,
        fishing_point: mir2_protocol::Point { x: 12, y: 15 },
        found_fish,
    };
    let cue = FishingBiteCue {
        owner_id: 1,
        generation: 7,
        started_at_ms: 10_000,
    };
    let mut ui = MountFishingUi::default();
    assert!(ui.observe(1, &update(true, true), 900));
    assert_eq!(ui.fishing_bite_frame(900), None);
    assert!(ui.action(EquipmentAction::Reel, 900).is_none());
    ui.present_fishing_bite(Some(FishingBiteCue { owner_id: 2, ..cue }), 10_000, 1000);
    assert_eq!(ui.fishing_bite_frame(1000), None);
    ui.present_fishing_bite(Some(cue), 10_000, 1000);
    assert_eq!(ui.fishing_reel_lease(), Some(cue));
    for (now, frame) in [
        (1000, 170),
        (1129, 170),
        (1130, 171),
        (2170, 179),
        (2300, 170),
    ] {
        assert_eq!(ui.fishing_bite_frame(now), Some(frame));
    }
    ui.present_fishing_bite(Some(cue), 10_130, 1130);
    assert_eq!(ui.fishing_bite_frame(1130), Some(171));

    let reel = ClientPacket::FishingCast { cast_out: false };
    assert_eq!(
        ui.action(EquipmentAction::Reel, 1130),
        Some(EquipmentIntent::Packet(reel.clone()))
    );
    assert!(ui.status_open);
    assert!(ui.observe(1, &update(true, true), 1140));
    assert!(ui.action(EquipmentAction::Reel, 1140).is_none());
    ui.escape_cancels = true;
    assert!(ui.escape(1150).is_none());
    assert!(!ui.status_open);
    assert!(ui.observe(1, &update(true, true), 1160));
    assert!(!ui.status_open);
    ui.release_unsent(&reel);
    assert!(ui.status_open);
    assert_eq!(ui.fishing_reel_lease(), Some(cue));

    assert!(ui.observe(1, &update(true, false), 1170));
    assert!(ui.action(EquipmentAction::Reel, 1170).is_none());
    assert_eq!(ui.fishing_bite_frame(1170), None);
    assert!(ui.observe(1, &update(false, false), 1180));
    ui.release_unsent(&reel);
    assert!(!ui.status_open);
    assert!(ui.observe(1, &update(true, true), 1190));
    assert!(ui.action(EquipmentAction::Reel, 1190).is_none());
}

#[test]
fn mount_cooldown_and_animation_match_original_variants() {
    let mut ui = MountFishingUi::default();
    let mut inventory = bag();
    ui.sync_equipment(&inventory);
    ui.observe(
        1,
        &ServerPacket::MountUpdate {
            object_id: 1,
            mount_type: 2,
            riding_mount: false,
        },
        1000,
    );
    assert!(ui.action(EquipmentAction::Ride, 1499).is_none());
    assert_eq!(
        ui.action(EquipmentAction::Ride, 1500),
        Some(EquipmentIntent::Packet(ClientPacket::Chat {
            message: "@ride".into(),
            linked_items: vec![]
        }))
    );
    ui.standing = false;
    assert!(ui.action(EquipmentAction::Ride, 2000).is_none());
    assert_eq!(ui.mount_animation_index(0), Some(1210));
    assert_eq!(ui.mount_animation_index(1500), Some(1225));
    assert_eq!(ui.mount_animation_index(1600), Some(1210));
    inventory.items[0]
        .tooltip_source
        .as_mut()
        .unwrap()
        .user_item
        .as_mut()
        .unwrap()
        .slots
        .push(None);
    ui.sync_equipment(&inventory);
    assert_eq!(ui.mount_animation_index(0), Some(1370));
    assert_eq!(
        view::attachment_rect(EquipmentDialog::Mount, 0, 4),
        Some(CrystalRect::new(37.0, 324.0, 34.0, 30.0))
    );
    assert_eq!(
        view::attachment_rect(EquipmentDialog::Mount, 4, 5),
        Some(CrystalRect::new(252.0, 323.0, 34.0, 30.0))
    );
    assert!(view::attachment_rect(EquipmentDialog::Mount, 4, 4).is_none());
    assert_eq!(
        view::attachment_rect(EquipmentDialog::Fishing, 4, 5),
        Some(CrystalRect::new(137.0, 241.0, 34.0, 30.0))
    );
}
