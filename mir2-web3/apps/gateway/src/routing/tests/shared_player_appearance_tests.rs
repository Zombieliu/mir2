//! Ordinary authenticated bootstrap and equipment ingress must project real
//! personal looks into the shared Zone's first and subsequent ObjectPlayer.
//! Progression/equipment preparation is fixture-only; equip/remove are packets.
use super::*;
use mir2_simulation::{world_entity_sprite_from_object_player, EquipmentSlot};

fn start(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    account: &str,
    name: &str,
    class: MirClass,
    gender: MirGender,
) -> Vec<ServerPacket> {
    for packet in [
        ClientPacket::NewAccount {
            account_id: account.into(),
            password: account.into(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        },
        ClientPacket::Login {
            account_id: account.into(),
            password: account.into(),
        },
    ] {
        runtime.execute(WorldCommand::ClientPacket(packet)).unwrap();
    }
    let index = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::NewCharacter {
            name: name.into(),
            gender,
            class,
        }))
        .unwrap()
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .expect("ordinary creation should return an authenticated character");
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: index,
        }))
        .unwrap()
}

fn info(packets: &[ServerPacket], name: &str) -> ObjectPlayerInfo {
    packets
        .iter()
        .rev()
        .find_map(|packet| match packet {
            ServerPacket::ObjectPlayer { info } if info.name == name => Some(info.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no canonical appearance for {name}: {packets:?}"))
}

fn drain(runtime: &mut SharedInProcessZoneSessionRuntime) -> Vec<ServerPacket> {
    runtime.execute(WorldCommand::Tick).unwrap()
}

fn assert_owner_sprite(info: &ObjectPlayerInfo, runtime: &SharedInProcessZoneSessionRuntime) {
    let snapshot = runtime.inner.world_snapshot();
    let owner = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    assert_eq!(info.transform_type, -1);
    assert_eq!(info.class, owner.class.unwrap());
    assert_eq!(info.gender, owner.gender.unwrap());
    assert_eq!(
        serde_json::to_value(world_entity_sprite_from_object_player(info)).unwrap(),
        serde_json::to_value(owner.sprite.as_ref().unwrap()).unwrap(),
        "remote packet-first sprite must match authenticated owner's complete sprite"
    );
}

#[test]
fn ordinary_bootstrap_projects_both_players_for_three_classes_and_both_genders() {
    for (class_index, class) in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist]
        .into_iter()
        .enumerate()
    {
        for (gender_index, gender) in [MirGender::Male, MirGender::Female].into_iter().enumerate() {
            let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
            let mut observer = shared_session_runtime(shared.clone());
            let mut subject = shared_session_runtime(shared);
            start(
                &mut observer,
                "look-observer",
                "Observer",
                MirClass::Warrior,
                MirGender::Male,
            );
            let name = format!("Look{class_index}{gender_index}");
            let bootstrap = start(
                &mut subject,
                &format!("look-{class_index}-{gender_index}"),
                &name,
                class,
                gender,
            );
            assert_owner_sprite(&info(&bootstrap, "Observer"), &observer);
            let packets = drain(&mut observer);
            assert_owner_sprite(&info(&packets, &name), &subject);
        }
    }
}

#[test]
fn ordinary_weapon_and_armour_remove_and_equip_refresh_existing_observer() {
    for (index, class) in [MirClass::Warrior, MirClass::Wizard, MirClass::Taoist]
        .into_iter()
        .enumerate()
    {
        let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut observer = shared_session_runtime(shared.clone());
        let mut subject = shared_session_runtime(shared);
        start(
            &mut observer,
            "gear-observer",
            "Observer",
            MirClass::Warrior,
            MirGender::Male,
        );
        let name = format!("Gear{index}");
        start(
            &mut subject,
            &format!("gear-{index}"),
            &name,
            class,
            MirGender::Male,
        );
        equip_runtime_crystal_items_for_class(&mut subject, class, &[]);
        assert_owner_sprite(&info(&drain(&mut observer), &name), &subject);
        for (template, slot, to) in [
            ("WoodenSword", EquipmentSlot::Weapon, 0),
            ("LightArmour(M)", EquipmentSlot::Armour, 1),
        ] {
            let template = mir2_game_data::crystal_item_by_name(template).unwrap();
            let key = format!("crystal-item-{}", template.item_index);
            subject
                .execute(WorldCommand::Stage5Command {
                    action: "qa.giveItem".into(),
                    args: vec![key.clone()],
                })
                .unwrap();
            let uid = subject
                .inner
                .world_snapshot()
                .inventory_items
                .iter()
                .find(|item| item.key == key)
                .unwrap()
                .unique_id;
            let equipped = subject
                .execute(WorldCommand::ClientPacket(ClientPacket::EquipItem {
                    grid: MirGridType::Inventory,
                    unique_id: uid,
                    to,
                }))
                .unwrap();
            assert!(equipped
                .iter()
                .any(|packet| matches!(packet, ServerPacket::EquipItem { success: true, .. })));
            assert_owner_sprite(&info(&drain(&mut observer), &name), &subject);
            let removed = subject
                .execute(WorldCommand::ClientPacket(ClientPacket::RemoveItem {
                    grid: MirGridType::Inventory,
                    unique_id: uid,
                    to: 0,
                }))
                .unwrap();
            assert!(removed
                .iter()
                .any(|packet| matches!(packet, ServerPacket::RemoveItem { success: true, .. })));
            let observed = info(&drain(&mut observer), &name);
            assert_owner_sprite(&observed, &subject);
            if slot == EquipmentSlot::Weapon {
                assert_eq!(observed.weapon, -1);
                assert!(world_entity_sprite_from_object_player(&observed)
                    .weapon_library
                    .is_none());
            }
            let equipped = subject
                .execute(WorldCommand::ClientPacket(ClientPacket::EquipItem {
                    grid: MirGridType::Inventory,
                    unique_id: uid,
                    to,
                }))
                .unwrap();
            assert!(equipped
                .iter()
                .any(|packet| matches!(packet, ServerPacket::EquipItem { success: true, .. })));
            assert_owner_sprite(&info(&drain(&mut observer), &name), &subject);
        }
        subject.sync_zone_snapshot();
        assert!(
            !drain(&mut observer).iter().any(|packet| matches!(packet,
            ServerPacket::ObjectPlayer { info } if info.name == name)),
            "unchanged personal looks must not resend the actor on every tick"
        );
    }
}

#[test]
fn map_transfer_and_aoi_reentry_keep_equipped_remote_sprite() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut observer = shared_session_runtime(shared.clone());
    let mut subject = shared_session_runtime(shared);
    start(
        &mut observer,
        "map-observer",
        "Observer",
        MirClass::Warrior,
        MirGender::Male,
    );
    start(
        &mut subject,
        "map-subject",
        "Traveller",
        MirClass::Taoist,
        MirGender::Male,
    );
    equip_runtime_crystal_items_for_class(
        &mut subject,
        MirClass::Taoist,
        &[
            ("WoodenSword", EquipmentSlot::Weapon, 1),
            ("SoulArmour(M)", EquipmentSlot::Armour, 1),
        ],
    );
    drain(&mut observer);
    observer
        .execute(WorldCommand::TransferMap {
            key: "crystal:0102:3:7".into(),
        })
        .unwrap();
    let incoming = subject
        .execute(WorldCommand::TransferMap {
            key: "crystal:0102:4:7".into(),
        })
        .unwrap();
    assert_owner_sprite(&info(&incoming, "Observer"), &observer);
    assert_owner_sprite(&info(&drain(&mut observer), "Traveller"), &subject);
    subject
        .execute(WorldCommand::TransferMap {
            key: "crystal:0:330:270".into(),
        })
        .unwrap();
    drain(&mut observer);
    subject
        .execute(WorldCommand::TransferMap {
            key: "crystal:0102:4:7".into(),
        })
        .unwrap();
    assert_owner_sprite(&info(&drain(&mut observer), "Traveller"), &subject);
}
