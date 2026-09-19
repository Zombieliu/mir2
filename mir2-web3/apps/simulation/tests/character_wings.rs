//! CharacterDialog's wing is server equipment appearance, not a UI guess.
//! Crystal: HumanObject.RefreshEquipmentStats + GetUpdateInfo.

use mir2_game_data::{crystal_item_by_name, crystal_real_item_for_player};
use mir2_protocol::{ClientPacket, MirClass, MirGender, MirGridType, ServerPacket};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, SimulationConfig, SimulationSession,
    WorldEntityKind, WorldEntitySnapshot,
};
use serde_json::{json, Value};

const ACCOUNT: &str = "character-wing-parity";
const ARMOUR_UID: u64 = 70_001;

fn armour(name: &str) -> Value {
    let info = crystal_item_by_name(name).expect("source armour catalogue row");
    json!({
        "key": format!("crystal-item-{}", info.item_index),
        "slot": "armour", "name": info.name, "icon": info.image,
        "shape": info.shape, "description": "Character wing fixture",
        "durability_current": info.durability, "durability_max": info.durability,
        "user_item_unique_id": ARMOUR_UID,
        "user_item_metadata": {"item_index": info.item_index},
        "attack": 0, "defence": 0
    })
}

fn start(equipment: Vec<Value>, gender: MirGender) -> SimulationSession {
    let character = CharacterRecord {
        index: 0,
        name: "WingParity".to_owned(),
        level: 50,
        class: MirClass::Warrior,
        gender,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.equipment_items_explicit_empty = true;
    save.equipment_items_json = equipment.into_iter().map(|item| item.to_string()).collect();
    save.inventory_items_json.clear();
    save.belt_items_json.clear();
    let config = SimulationConfig::default();
    let mut account = AccountRecord::empty();
    // Explicit in-memory QA authority for the @SETLIGHT appearance-packet check.
    account.gm_level = 1;
    account.characters.push(character);
    account.saves.insert(0, save);
    config
        .account_store
        .lock()
        .expect("account store")
        .accounts
        .insert(ACCOUNT.to_owned(), account);
    let mut session = SimulationSession::new(config);
    login_and_start(&mut session);
    session
}

fn login_and_start(session: &mut SimulationSession) {
    let login = session.handle_packet(ClientPacket::Login {
        account_id: ACCOUNT.to_owned(),
        password: "demo".to_owned(),
    });
    assert!(login
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(
        start
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })),
        "start packets: {start:?}"
    );
}

fn self_player(session: &SimulationSession) -> WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("authoritative self player")
}

#[test]
fn character_wings_follow_real_armour_catalogue_for_both_genders_and_effects() {
    for (name, gender, effect) in [
        ("HeavenArmour(M)", MirGender::Male, 1),
        ("HeavenArmour(F)", MirGender::Female, 1),
        ("MirArmour(M)", MirGender::Male, 2),
        ("MirArmour(F)", MirGender::Female, 2),
    ] {
        let info = crystal_item_by_name(name).unwrap();
        let real = crystal_real_item_for_player(&info, 50, MirClass::Warrior);
        assert_eq!(real.effect, effect);
        let session = start(vec![armour(name)], gender);
        let player = self_player(&session);
        assert_eq!(player.wing_effect, Some(effect), "{name}");
        assert_eq!(serde_json::to_value(&player).unwrap()["wingEffect"], effect);
        assert!(
            session
                .world_snapshot()
                .entities
                .iter()
                .filter(|entity| entity.kind != WorldEntityKind::SelfPlayer)
                .all(|entity| entity.wing_effect.is_none()),
            "do not copy self wings to other actors"
        );
    }
}

#[test]
fn character_wings_clear_for_no_armour_plain_armour_and_broken_base_durability() {
    for equipment in [
        Vec::new(),
        vec![armour("BaseDress(M)")],
        vec![{
            let mut item = armour("HeavenArmour(M)");
            item["durability_current"] = json!(0);
            item
        }],
        vec![{
            let mut item = armour("MirArmour(M)");
            item["durability_current"] = json!(0);
            item["durability_max"] = json!(0);
            item
        }],
    ] {
        let session = start(equipment, MirGender::Male);
        let player = self_player(&session);
        assert_eq!(player.wing_effect, Some(0));
        assert_eq!(serde_json::to_value(player).unwrap()["wingEffect"], 0);
    }
}

#[test]
fn character_wings_use_catalogue_identity_not_client_declared_effect() {
    for (name, expected) in [
        ("BaseDress(M)", 0),
        ("HeavenArmour(M)", 1),
        ("MirArmour(M)", 2),
    ] {
        let mut item = armour(name);
        item["wing_effect"] = json!(2);
        item["wingEffect"] = json!(2);
        let session = start(vec![item], MirGender::Male);
        assert_eq!(
            self_player(&session).wing_effect,
            Some(expected),
            "catalogue {name}"
        );
    }
    let mut legacy = armour("HeavenArmour(M)");
    legacy.as_object_mut().unwrap().remove("user_item_metadata");
    legacy
        .as_object_mut()
        .unwrap()
        .remove("user_item_unique_id");
    assert_eq!(
        self_player(&start(vec![legacy], MirGender::Male)).wing_effect,
        Some(1)
    );
}

#[test]
fn character_wings_unequip_reequip_save_reload_and_player_update_keep_exact_state() {
    let mut session = start(vec![armour("HeavenArmour(M)")], MirGender::Male);
    for expected in [1, 0, 1] {
        assert_eq!(self_player(&session).wing_effect, Some(expected));
        let packets = session.handle_packet(ClientPacket::Chat {
            message: "@SETLIGHT 3".to_owned(),
            linked_items: Vec::new(),
        });
        assert!(
            packets.iter().any(|p| matches!(p,
                ServerPacket::PlayerUpdate { wing_effect, light: 3, .. } if *wing_effect == expected
            )),
            "light refresh must not wipe or resurrect wings: {packets:?}"
        );
        if expected == 1 {
            let packets = session.handle_packet(ClientPacket::RemoveItem {
                grid: MirGridType::Inventory,
                unique_id: ARMOUR_UID,
                to: 0, // Current Gateway contract uses normalized bag indexes.
            });
            assert!(
                packets.iter().any(|p| matches!(
                    p,
                    ServerPacket::RemoveItem {
                        unique_id: ARMOUR_UID,
                        success: true,
                        ..
                    }
                )),
                "remove packets: {packets:?}"
            );
            let snapshot = session.world_snapshot();
            let removed = snapshot
                .inventory_items
                .iter()
                .find(|item| item.unique_id == ARMOUR_UID)
                .expect("the exact armour instance must reach the first free bag cell");
            assert_eq!(removed.slot, 0);
            assert!(snapshot
                .equipment_items
                .iter()
                .all(|item| item.unique_id != Some(ARMOUR_UID)));
        } else {
            let packets = session.handle_packet(ClientPacket::EquipItem {
                grid: MirGridType::Inventory,
                unique_id: ARMOUR_UID,
                to: 1,
            });
            assert!(
                packets.iter().any(|p| matches!(
                    p,
                    ServerPacket::EquipItem {
                        unique_id: ARMOUR_UID,
                        success: true,
                        ..
                    }
                )),
                "equip packets: {packets:?}"
            );
        }
    }
    assert_eq!(self_player(&session).wing_effect, Some(0));
    session
        .save_active_character()
        .expect("save exact unequipped state");
    session.handle_packet(ClientPacket::LogOut);
    login_and_start(&mut session);
    assert_eq!(self_player(&session).wing_effect, Some(0));
}

#[test]
fn character_wings_missing_snapshot_field_remains_unknown_not_zero() {
    let session = start(vec![armour("HeavenArmour(M)")], MirGender::Male);
    let mut legacy = serde_json::to_value(self_player(&session)).unwrap();
    legacy.as_object_mut().unwrap().remove("wingEffect");
    let restored: WorldEntitySnapshot = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored.wing_effect, None);
    assert!(serde_json::to_value(restored)
        .unwrap()
        .get("wingEffect")
        .is_none());
}
