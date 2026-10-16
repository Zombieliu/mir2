use super::*;
use crate::config::{EquipmentSlot, Stage5SystemsState, WorldEntityKind};
use crate::runtime::equipment::EquipmentState;
use crate::runtime::packets::{self_player_update_packet, self_zone_player_appearance};
use crate::runtime::resources::{InventoryResource, MountResource};
use crate::{
    world_entity_sprite_from_object_player, AccountRecord, CharacterRecord, CharacterSaveRecord,
    SessionId, SimulationConfig, SimulationSession, ZoneCommand, ZoneManager, ZoneOutbound,
};
use mir2_protocol::{ClientPacket, MirGender, MirGridType, ObjectPlayerInfo, Point, ServerPacket};
use serde_json::{json, Value};

const ACCOUNT: &str = "fashion-projection";
const ARMOUR_UID: u64 = 70_001;
const REPLACEMENT_UID: u64 = 70_003;

fn equipment(index: i32, slot: EquipmentSlot, uid: u64) -> EquipmentState {
    let template = mir2_game_data::crystal_item_by_index(index).unwrap();
    serde_json::from_value(json!({
        "key": format!("crystal-item-{index}"), "slot": slot,
        "name": template.name, "icon": template.image, "shape": template.shape,
        "description": "Owned fashion projection fixture's real equipment",
        "durability_current": template.durability, "durability_max": template.durability,
        "user_item_unique_id": uid, "user_item_metadata": {"item_index":index},
        "attack":0, "defence":0
    }))
    .unwrap()
}

fn start(
    class: MirClass,
    gender: MirGender,
    armour: bool,
) -> (SimulationSession, SimulationConfig) {
    let character = CharacterRecord {
        index: 0,
        name: "FashionSubject".into(),
        level: 50,
        class,
        gender,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.position = Point { x: 330, y: 270 };
    save.equipment_items_explicit_empty = true;
    save.gold = 12_345;
    save.credit = 123;
    save.belt_items_json.clear();
    save.inventory_items_json.clear();
    let weapon = match class {
        MirClass::Assassin => 281,
        MirClass::Archer => 298,
        _ => 221,
    };
    let mut real_equipment = vec![equipment(weapon, EquipmentSlot::Weapon, 70_002)];
    if armour {
        real_equipment.push(equipment(
            if gender == MirGender::Female {
                320
            } else {
                319
            },
            EquipmentSlot::Armour,
            ARMOUR_UID,
        ));
    }
    save.equipment_items_json = real_equipment
        .iter()
        .map(|item| serde_json::to_string(item).unwrap())
        .collect();
    let config = SimulationConfig::default();
    let mut account = AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0, save);
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .insert(ACCOUNT.into(), account);
    let mut session = SimulationSession::new(config.clone());
    login_and_start(&mut session);
    (session, config)
}

fn login_and_start(session: &mut SimulationSession) {
    let login = session.handle_packet(ClientPacket::Login {
        account_id: ACCOUNT.into(),
        password: "demo".into(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "{packets:?}"
    );
}

fn select(session: &mut SimulationSession, style: Option<&str>) {
    let mut resource = session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>();
    let appearance = &mut resource.stage5_systems.appearance;
    if let Some(id) = style {
        // This is a test-owned previously crafted wardrobe. The separate real
        // atelier transaction tests prove debits, grants and durable receipts.
        appearance.owned_fashions.insert(id.into());
    }
    appearance.selected_fashion = style.map(str::to_owned);
}

fn physical_state(session: &SimulationSession) -> Value {
    let snapshot = session.world_snapshot();
    json!({
        "equipment":snapshot.equipment_items,
        "inventory":snapshot.inventory_items,
        "belt":snapshot.belt_items,
        "gold":snapshot.gold,"credit":snapshot.credit,
        "hp":snapshot.player_hp,"maxHp":snapshot.player_max_hp,
        "mp":snapshot.player_mp,"maxMp":snapshot.player_max_mp,
        "stats":snapshot.player_crystal_stats,
        "weights":snapshot.player_weights,"weight":snapshot.current_weight,
        "cityCurrencies":snapshot.city_currencies
    })
}

fn observer_info(outbounds: &[ZoneOutbound], observer: &str, object_id: u32) -> ObjectPlayerInfo {
    let observer = SessionId::new(observer);
    outbounds
        .iter()
        .filter_map(|outbound| match outbound {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if *session_id == observer => Some(packets),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&observer) => Some(packets),
            ZoneOutbound::ToAll { packets } => Some(packets),
            _ => None,
        })
        .flatten()
        .find_map(|packet| match packet {
            ServerPacket::ObjectPlayer { info } if info.object_id == object_id => {
                Some(info.clone())
            }
            _ => None,
        })
        .expect("observer receives the authoritative complete appearance")
}

fn assert_projection(session: &SimulationSession, shape: u16, image: Option<u32>) {
    let snapshot = session.world_snapshot();
    assert_eq!(snapshot.cosmetic_armour_image, image);
    let own = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    let own_sprite = own.sprite.as_ref().unwrap();
    assert_eq!(own_sprite.body_library, format!("CArmour/{shape:02}"));
    // Crystal's no-armour Zone descriptor retains -1; the renderer and legacy
    // owner PlayerUpdate display body 00. Fashion must not rewrite that absence.
    let zone_shape = if shape == 0 && image.is_none() {
        -1
    } else {
        shape as i16
    };
    let appearance = self_zone_player_appearance(session.app.world()).unwrap();
    assert_eq!(appearance.armour, zone_shape);
    let update = self_player_update_packet(session.app.world(), appearance.light).unwrap();
    assert!(matches!(update, ServerPacket::PlayerUpdate { armour, .. } if armour == shape as i16));

    let subject = session
        .active_zone_join_snapshot("fashion-subject")
        .unwrap();
    let mut observer = subject.clone();
    observer.session_id = SessionId::new("fashion-observer");
    observer.account_id = "fashion-observer-account".into();
    observer.character_index = 1;
    observer.object_id += 100;
    observer.name = "Observer".into();
    observer.position.x += 1;
    let mut manager = ZoneManager::new();
    manager.join(observer);
    let outbounds = manager.join_with_appearance(subject.clone(), appearance);
    let info = observer_info(&outbounds, "fashion-observer", subject.object_id);
    assert_eq!(info.armour, zone_shape);
    let seen = world_entity_sprite_from_object_player(&info);
    assert_eq!(seen.body_library, own_sprite.body_library);
    assert_eq!(seen.alt_body_library, own_sprite.alt_body_library);
    assert_eq!(seen.frame_base_offset, own_sprite.frame_base_offset);
    assert_eq!(seen.weapon_library, own_sprite.weapon_library);
    assert_eq!(seen.alt_weapon_library, own_sprite.alt_weapon_library);
    assert_eq!(seen.mount_library, own_sprite.mount_library);
}

#[test]
fn fashion_validation_requires_known_owned_selection_and_supported_class() {
    let mut appearance = Stage5AppearanceState::default();
    for class in [
        MirClass::Warrior,
        MirClass::Wizard,
        MirClass::Taoist,
        MirClass::Assassin,
        MirClass::Archer,
    ] {
        assert!(validate_appearance(&appearance, class).is_ok());
    }
    appearance.owned_fashions.insert("traveler".into());
    appearance.selected_fashion = Some("traveler".into());
    for class in [
        MirClass::Warrior,
        MirClass::Wizard,
        MirClass::Taoist,
        MirClass::Assassin,
        MirClass::Archer,
    ] {
        assert!(validate_appearance(&appearance, class).is_ok());
    }
    appearance.selected_fashion = Some("jade-gala".into());
    assert!(validate_appearance(&appearance, MirClass::Warrior).is_err());
    appearance.owned_fashions.insert("jade-gala".into());
    assert!(validate_appearance(&appearance, MirClass::Warrior).is_ok());
    assert!(validate_appearance(&appearance, MirClass::Wizard).is_ok());
    assert!(validate_appearance(&appearance, MirClass::Taoist).is_ok());
    assert!(validate_appearance(&appearance, MirClass::Assassin).is_err());
    assert!(validate_appearance(&appearance, MirClass::Archer).is_err());
    appearance.selected_fashion = None;
    assert!(
        validate_appearance(&appearance, MirClass::Archer).is_ok(),
        "unsupported unselected owned artwork is not equipped"
    );
    appearance.owned_fashions.insert("CArmour/99".into());
    assert!(validate_appearance(&appearance, MirClass::Warrior).is_err());
    appearance.owned_fashions.remove("CArmour/99");
    appearance.selected_fashion = Some("CArmour/99".into());
    assert!(validate_appearance(&appearance, MirClass::Warrior).is_err());
}

#[test]
fn fashion_five_classes_two_genders_all_projections_preserve_exact_physical_state() {
    for class in [
        MirClass::Warrior,
        MirClass::Wizard,
        MirClass::Taoist,
        MirClass::Assassin,
        MirClass::Archer,
    ] {
        for gender in [MirGender::Male, MirGender::Female] {
            for real_armour in [false, true] {
                let (mut session, _) = start(class, gender, real_armour);
                let before = physical_state(&session);
                let stats = crate::runtime::stats::compute_player_stats(session.app.world());
                select(&mut session, Some("traveler"));
                assert_projection(
                    &session,
                    1,
                    Some(if gender == MirGender::Female { 80 } else { 60 }),
                );
                if matches!(
                    class,
                    MirClass::Warrior | MirClass::Wizard | MirClass::Taoist
                ) {
                    select(&mut session, Some("jade-gala"));
                    assert_projection(
                        &session,
                        10,
                        Some(if gender == MirGender::Female {
                            605
                        } else {
                            595
                        }),
                    );
                }
                assert_eq!(physical_state(&session), before);
                assert_eq!(
                    crate::runtime::stats::compute_player_stats(session.app.world()),
                    stats
                );
                select(&mut session, None);
                assert_projection(&session, if real_armour { 2 } else { 0 }, None);
                assert_eq!(physical_state(&session), before);
            }
        }
    }
}

#[test]
fn fashion_real_armour_equip_clear_save_and_new_session_keep_separate_custody() {
    let (mut session, config) = start(MirClass::Warrior, MirGender::Male, true);
    let replacement = crate::runtime::equipment::item_state_from_equipment_state(
        equipment(45, EquipmentSlot::Armour, REPLACEMENT_UID),
        crate::ItemContainer::Bag1,
        0,
    );
    session
        .app
        .world_mut()
        .resource_mut::<InventoryResource>()
        .inventory_items
        .push(replacement);
    select(&mut session, Some("traveler"));
    let packets = session.handle_packet(ClientPacket::EquipItem {
        grid: MirGridType::Inventory,
        unique_id: REPLACEMENT_UID,
        to: 1,
    });
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::EquipItem {
                unique_id: REPLACEMENT_UID,
                success: true,
                ..
            }
        )),
        "{packets:?}"
    );
    assert_projection(&session, 1, Some(60));
    let snapshot = session.world_snapshot();
    assert!(snapshot
        .equipment_items
        .iter()
        .any(|item| item.unique_id == Some(REPLACEMENT_UID)));
    assert!(snapshot
        .inventory_items
        .iter()
        .any(|item| item.unique_id == ARMOUR_UID));
    let before = physical_state(&session);
    let stats = crate::runtime::stats::compute_player_stats(session.app.world());
    select(&mut session, None);
    assert_projection(&session, 10, None);
    assert_eq!(physical_state(&session), before);
    assert_eq!(
        crate::runtime::stats::compute_player_stats(session.app.world()),
        stats
    );
    select(&mut session, Some("traveler"));
    session.save_active_character().unwrap();
    session.handle_packet(ClientPacket::LogOut);
    let mut restored = SimulationSession::new(config);
    login_and_start(&mut restored);
    assert_projection(&restored, 1, Some(60));
    assert_eq!(physical_state(&restored), before);
    select(&mut restored, None);
    assert_projection(&restored, 10, None);
    assert_eq!(physical_state(&restored), before);
    restored.save_active_character().unwrap();
    restored.handle_packet(ClientPacket::LogOut);
    login_and_start(&mut restored);
    assert_projection(&restored, 10, None);
    assert_eq!(physical_state(&restored), before);
}

#[test]
fn fashion_unknown_unowned_unsupported_or_unauthed_source_falls_back_to_real_armour() {
    let (mut session, _) = start(MirClass::Assassin, MirGender::Female, true);
    for id in ["unknown", "jade-gala"] {
        select(&mut session, Some(id));
        assert_projection(&session, 2, None);
    }
    select(&mut session, Some("traveler"));
    session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .appearance
        .owned_fashions
        .clear();
    assert_projection(&session, 2, None);
    select(&mut session, Some("traveler"));
    session
        .app
        .world_mut()
        .resource_mut::<SessionResource>()
        .account_id = None;
    assert!(selected_style(session.app.world()).is_none());
    assert_eq!(armour_image(session.app.world()), None);
    assert_eq!(
        self_zone_player_appearance(session.app.world())
            .unwrap()
            .armour,
        2
    );
}

#[test]
fn fashion_zone_refresh_late_join_and_transform_mount_weapon_rules_keep_authority() {
    let (mut session, _) = start(MirClass::Warrior, MirGender::Female, true);
    select(&mut session, Some("traveler"));
    {
        let mut mount = session.app.world_mut().resource_mut::<MountResource>();
        mount.riding_mount = true;
        mount.mount_type = 3;
    }
    let own = session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    let sprite = own.sprite.unwrap();
    assert_eq!(sprite.mount_library.as_deref(), Some("Mount/03"));
    assert!(sprite.weapon_library.is_none() && sprite.alt_weapon_library.is_none());
    assert_projection(&session, 1, Some(80));

    let subject = session.active_zone_join_snapshot("subject").unwrap();
    let mut observer = subject.clone();
    observer.session_id = SessionId::new("observer");
    observer.account_id = "observer-account".into();
    observer.character_index += 1;
    observer.object_id += 100;
    observer.position.x += 1;
    let mut manager = ZoneManager::new();
    manager.join(observer.clone());
    manager.join_with_appearance(
        subject.clone(),
        self_zone_player_appearance(session.app.world()).unwrap(),
    );
    manager.handle(ZoneCommand::BroadcastPackets {
        session_id: subject.session_id.clone(),
        owner_local_object_id: subject.object_id,
        now_ms: 1,
        packets: vec![ServerPacket::TransformUpdate {
            object_id: subject.object_id,
            transform_type: 4,
        }],
    });
    select(&mut session, Some("jade-gala"));
    let packets = manager.handle(ZoneCommand::SyncPlayerAppearance {
        session_id: subject.session_id.clone(),
        appearance: self_zone_player_appearance(session.app.world()).unwrap(),
    });
    let seen = observer_info(&packets, "observer", subject.object_id);
    assert_eq!((seen.armour, seen.transform_type), (10, 4));
    assert_eq!(
        world_entity_sprite_from_object_player(&seen).body_library,
        "Transform/04"
    );
    let mut late = observer.clone();
    late.session_id = SessionId::new("late");
    late.account_id = "late-account".into();
    late.character_index += 1;
    late.object_id += 1;
    late.position.x += 1;
    let packets = manager.join(late);
    let seen = observer_info(&packets, "late", subject.object_id);
    assert_eq!(
        (seen.armour, seen.transform_type, seen.riding_mount),
        (10, 4, true)
    );
    let mut outside_aoi = subject.position.clone();
    outside_aoi.x += 100;
    manager.handle(ZoneCommand::SyncPlayerTransform {
        session_id: subject.session_id.clone(),
        position: outside_aoi,
        direction: subject.direction,
    });
    let reentry = manager.handle(ZoneCommand::SyncPlayerTransform {
        session_id: subject.session_id.clone(),
        position: subject.position.clone(),
        direction: subject.direction,
    });
    let seen = observer_info(&reentry, "observer", subject.object_id);
    assert_eq!(
        (seen.armour, seen.transform_type, seen.riding_mount),
        (10, 4, true)
    );
    manager.handle(ZoneCommand::BroadcastPackets {
        session_id: subject.session_id.clone(),
        owner_local_object_id: subject.object_id,
        now_ms: 2,
        packets: vec![ServerPacket::TransformUpdate {
            object_id: subject.object_id,
            transform_type: -1,
        }],
    });
    select(&mut session, None);
    let packets = manager.handle(ZoneCommand::SyncPlayerAppearance {
        session_id: subject.session_id,
        appearance: self_zone_player_appearance(session.app.world()).unwrap(),
    });
    let seen = observer_info(&packets, "observer", subject.object_id);
    assert_eq!(
        (seen.armour, seen.transform_type, seen.riding_mount),
        (2, -1, true)
    );
    let sprite = world_entity_sprite_from_object_player(&seen);
    assert_eq!(sprite.body_library, "CArmour/02");
    assert_eq!(sprite.mount_library.as_deref(), Some("Mount/03"));
}

#[test]
fn fashion_legacy_serialization_and_public_projection_do_not_leak_private_receipts() {
    let legacy: Stage5AppearanceState = serde_json::from_value(json!({"hair":3})).unwrap();
    assert_eq!(legacy.hair, 3);
    assert!(
        legacy.owned_fashions.is_empty()
            && legacy.selected_fashion.is_none()
            && legacy.atelier_receipts.is_empty()
    );
    assert_eq!(serde_json::to_value(&legacy).unwrap(), json!({"hair":3}));
    let (mut session, _) = start(MirClass::Warrior, MirGender::Male, false);
    select(&mut session, Some("traveler"));
    session
        .app
        .world_mut()
        .resource_mut::<Stage5SystemsResource>()
        .stage5_systems
        .appearance
        .atelier_receipts
        .insert(
            "private-request".into(),
            "private-actor-material-witness".into(),
        );
    let snapshot = session.world_snapshot();
    let durable = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        durable["stage5Systems"]["appearance"]["atelierReceipts"]["private-request"],
        "private-actor-material-witness"
    );
    let public = serde_json::to_value(snapshot.client_view()).unwrap();
    assert!(public["stage5Systems"]["appearance"]
        .get("atelierReceipts")
        .is_none());
    assert!(!public
        .to_string()
        .contains("private-actor-material-witness"));
    assert_eq!(public["cosmeticArmourImage"], 60);
    assert_eq!(
        public["stage5Systems"]["appearance"]["selectedFashion"],
        "traveler"
    );
    let mut old_snapshot = durable;
    old_snapshot
        .as_object_mut()
        .unwrap()
        .remove("cosmeticArmourImage");
    let decoded: crate::WorldSnapshot = serde_json::from_value(old_snapshot).unwrap();
    assert_eq!(decoded.cosmetic_armour_image, None);
    let saved = crate::runtime::save::snapshot_active_character_save(session.app.world()).unwrap();
    let systems: Stage5SystemsState =
        serde_json::from_str(saved.stage5_systems_json.as_deref().unwrap()).unwrap();
    assert_eq!(
        systems.appearance.selected_fashion.as_deref(),
        Some("traveler")
    );
    assert_eq!(
        systems.appearance.atelier_receipts["private-request"],
        "private-actor-material-witness"
    );
}
