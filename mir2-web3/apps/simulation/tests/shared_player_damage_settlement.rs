//! The shared Zone hands the personal session an already resolved HP loss.
//! Use normal authentication/StartGame, with isolated account-store fixtures;
//! no QA command or private ECS access can bypass the public settlement path.

use mir2_protocol::{ClientPacket, ServerPacket, UserItem};
use mir2_simulation::{CharacterSaveRecord, SimulationConfig, SimulationSession};
use serde_json::{json, Value};

const RED_POISON: &str = "crystal-red-poison";

fn config_with_red_poison() -> SimulationConfig {
    let config = SimulationConfig::default();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&0)
        .unwrap()
        .buff_states_json
        .push(
            json!({
                "key": RED_POISON,
                "name": "Red poison",
                "description": "Damage amplification fixture",
                "expires_at_tick": 100_000,
                "attack_bonus": 0,
                "defence_bonus": 0,
                "stats": []
            })
            .to_string(),
        );
    config
}

fn login(config: &SimulationConfig) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(session.active_zone_join_snapshot("fixture").is_some());
    session
}

fn hp(session: &SimulationSession) -> i32 {
    let hp = session.world_snapshot().player_hp.unwrap();
    assert_eq!(
        session.active_zone_join_snapshot("fixture").unwrap().hp,
        hp,
        "the shared join projection must use the same settled HP"
    );
    assert_eq!(
        session.active_character_checkpoint().unwrap().hp,
        hp,
        "the private save snapshot must use the same settled HP"
    );
    hp
}

#[test]
fn shared_damage_requires_an_active_world_and_positive_loss() {
    let config = SimulationConfig::default();
    let mut session = SimulationSession::new(config.clone());
    let before = session.world_snapshot().tick;
    assert!(!session.apply_zone_player_damage(i32::MAX));
    assert_eq!(session.world_snapshot().tick, before);
    assert!(session.active_character_checkpoint().is_none());

    let mut session = login(&config);
    let before_hp = hp(&session);
    let before_tick = session.world_snapshot().tick;
    assert!(!session.apply_zone_player_damage(0));
    assert!(!session.apply_zone_player_damage(-10));
    assert_eq!(hp(&session), before_hp);
    assert_eq!(session.world_snapshot().tick, before_tick);
}

#[test]
fn shared_settlement_does_not_amplify_active_red_poison_twice() {
    let mut session = login(&config_with_red_poison());
    assert!(session
        .world_snapshot()
        .active_buffs
        .iter()
        .any(|buff| buff.key == RED_POISON && buff.remaining_ticks > 0));
    let before = hp(&session);
    assert!(before > 12);

    // A 10-point base hit was already amplified by the Zone to 12. The
    // personal layer must commit 12, rather than amplifying again to 14.
    assert!(!session.apply_zone_player_damage(12));
    assert_eq!(hp(&session), before - 12);
}

#[test]
fn shared_damage_cannot_create_an_early_death_by_reapplying_red_poison() {
    let mut session = login(&config_with_red_poison());
    let before = hp(&session);
    assert!(before > 6);
    assert!(!session.apply_zone_player_damage(before - 1));
    assert_eq!(hp(&session), 1);
    assert!(session.apply_zone_player_damage(1));
    assert_eq!(hp(&session), 0);
}

#[test]
fn shared_lethal_damage_reports_once_per_life_and_revive_opens_a_new_life() {
    let mut session = login(&SimulationConfig::default());
    let before = hp(&session);
    assert!(before > 0);
    assert!(session.apply_zone_player_damage(before));
    assert_eq!(hp(&session), 0);
    let dead_tick = session.world_snapshot().tick;

    assert!(!session.apply_zone_player_damage(i32::MAX));
    assert!(!session.apply_zone_player_damage(1));
    assert_eq!(hp(&session), 0);
    assert_eq!(
        session.world_snapshot().tick,
        dead_tick,
        "an already dead life must not produce another damage transition"
    );

    let revived = session.handle_packet(ClientPacket::TownRevive);
    assert!(revived
        .iter()
        .any(|packet| matches!(packet, ServerPacket::Revived)));
    assert!(hp(&session) > 0);
    assert!(session.apply_zone_player_damage(i32::MAX));
    assert_eq!(hp(&session), 0);
    assert!(!session.apply_zone_player_damage(1));
}

#[test]
fn personal_gm_immunity_is_projected_but_cannot_revoke_a_zone_settlement() {
    let config = SimulationConfig::default();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .gm_level = 10;
    let mut session = login(&config);
    assert!(!session.zone_player_combat_stats().gm_never_die);
    session.handle_packet(ClientPacket::Chat {
        message: "@SUPERMAN".into(),
        linked_items: vec![],
    });
    assert!(session.zone_player_combat_stats().gm_never_die);
    assert!(
        session
            .active_zone_join_snapshot("fixture")
            .unwrap()
            .combat_stats
            .gm_never_die
    );

    // This can represent a hit resolved before an immunity toggle. Only the
    // Zone may admit/reject it; the private mirror must commit its exact loss.
    let before = hp(&session);
    assert!(!session.apply_zone_player_damage(5));
    assert_eq!(hp(&session), before - 5);
    assert!(session.apply_zone_player_damage(before - 5));
    assert_eq!(hp(&session), 0);
    assert!(!session.apply_zone_player_damage(1));
}

#[test]
fn settled_hp_is_saved_and_loaded_through_the_normal_character_path() {
    let config = config_with_red_poison();
    let mut session = login(&config);
    let expected = hp(&session) - 12;
    assert!(!session.apply_zone_player_damage(12));
    session.save_active_character().unwrap();

    let mut loaded = login(&config);
    assert_eq!(hp(&loaded), expected);
    assert!(loaded.apply_zone_player_damage(expected));
    loaded.save_active_character().unwrap();

    let mut dead_loaded = login(&config);
    assert_eq!(hp(&dead_loaded), 0);
    assert!(!dead_loaded.apply_zone_player_damage(1));
}

const RENTAL_OWNER_INDEX: i32 = 78;

fn rental_fixture(equipped: bool) -> (SimulationConfig, SimulationSession, String, u64) {
    rental_fixture_for_slot(equipped.then_some("weapon"), Some(899_001))
}

fn rental_fixture_for_slot(
    equipment_slot: Option<&str>,
    exact_uid: Option<u64>,
) -> (SimulationConfig, SimulationSession, String, u64) {
    let equipped = equipment_slot.is_some();
    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut("demo").unwrap();
        let mut owner = account.characters[0].clone();
        owner.index = RENTAL_OWNER_INDEX;
        owner.name = "Lender".into();
        account.characters.push(owner.clone());
        account
            .saves
            .insert(owner.index, CharacterSaveRecord::new(owner));
    }
    let mut session = login(&config);
    // Configure an exact, unexpired rental through the public saved-state
    // interface. The test action itself uses only shared settlement methods.
    let mut save = session.active_character_checkpoint().unwrap();
    let states = if equipped {
        &mut save.equipment_items_json
    } else {
        &mut save.inventory_items_json
    };
    let item = states
        .iter_mut()
        .find(|state| {
            let item: Value = serde_json::from_str(state).unwrap();
            if let Some(slot) = equipment_slot {
                item["slot"] == slot
            } else {
                item["key"] == "dagger"
            }
        })
        .expect("ordinary starter loadout must contain the fixture item");
    let mut value: Value = serde_json::from_str(item).unwrap();
    let key = value["key"].as_str().unwrap().to_owned();
    let uid = if equipped {
        exact_uid.unwrap_or_else(|| match equipment_slot.unwrap() {
            "weapon" => 0,
            "armour" => 1,
            slot => panic!("unsupported legacy fixture slot {slot}"),
        })
    } else {
        value["unique_id"].as_u64().unwrap()
    };
    if equipped && exact_uid.is_some() {
        value["user_item_unique_id"] = json!(uid);
        // Capturing an exact worn UID also requires its real protocol item
        // index; legacy starter equipment intentionally has neither sidecar.
        let item_index: i32 = key.strip_prefix("crystal-item-").unwrap().parse().unwrap();
        if value["user_item_metadata"].is_null() {
            value["user_item_metadata"] = json!({ "item_index": item_index });
        } else {
            value["user_item_metadata"]["item_index"] = json!(item_index);
        }
    }
    value["rental_owner_name"] = json!("Lender");
    // 2099-01-01 UTC DateTime ticks: safely unexpired without changing clocks.
    value["rental_expiry_binary_datetime"] = json!(662_065_056_000_000_000_i64);
    value["rental_binding_flags"] = json!(0);
    value["rental_locked"] = json!(false);
    *item = value.to_string();
    save.has_rented_item = true;
    // Force the existing personal penalty to select items, so the rental must
    // be returned before that selection rather than merely survive by chance.
    save.pk_points = 300;
    session.restore_active_character_checkpoint(&save).unwrap();
    (config, session, key, uid)
}

fn reload_with_advertised_equipment(
    config: &SimulationConfig,
    session: &mut SimulationSession,
) -> Vec<Option<UserItem>> {
    session.save_active_character().unwrap();
    let mut loaded = SimulationSession::new(config.clone());
    let login = loaded.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = loaded.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let equipment = start
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::UserInformation { info } => info.equipment,
            _ => None,
        })
        .expect("normal StartGame must advertise equipped protocol instances");
    *session = loaded;
    equipment
}

fn returned_rental_items(config: &SimulationConfig) -> Vec<Value> {
    let store = config.account_store.lock().unwrap();
    let save = &store.accounts["demo"].saves[&RENTAL_OWNER_INDEX];
    let Some(systems) = &save.stage5_systems_json else {
        return vec![];
    };
    let systems: Value = serde_json::from_str(systems).unwrap();
    systems["mail"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|mail| mail["subject"] == "Rental item returned")
        .flat_map(|mail| mail["itemStatesJson"].as_array().unwrap().iter())
        .map(|item| serde_json::from_str(item.as_str().unwrap()).unwrap())
        .collect()
}

#[test]
fn shared_death_returns_bag_rental_immediately_before_drop_selection() {
    let (config, mut session, key, uid) = rental_fixture(false);
    assert!(session.apply_zone_player_damage(i32::MAX));
    let packets = session.apply_zone_player_death_penalty();
    assert!(packets.iter().any(|packet| matches!(packet,
        ServerPacket::DeleteItem { unique_id, count: 1 } if *unique_id == uid)));
    let save = session.active_character_checkpoint().unwrap();
    assert!(!save.has_rented_item);
    assert!(!save.inventory_items_json.iter().any(|state| {
        let item: Value = serde_json::from_str(state).unwrap();
        item["unique_id"] == uid
    }));
    let returned = returned_rental_items(&config);
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0]["key"], key);
    assert_eq!(returned[0]["unique_id"], uid);
    assert_eq!(returned[0]["rental_locked"], true);
    // A later ordinary personal tick must not return this item a second time.
    session.tick();
    assert_eq!(returned_rental_items(&config).len(), 1);
}

#[test]
fn shared_death_returns_equipped_rental_before_equipment_drop_selection() {
    let (config, mut session, key, uid) = rental_fixture(true);
    let client_equipment = reload_with_advertised_equipment(&config, &mut session);
    let worn = client_equipment[0].as_ref().unwrap();
    assert_eq!(worn.unique_id, uid);
    assert_ne!(uid, 0, "exact instance differs from its legacy weapon slot");
    assert!(session.apply_zone_player_damage(i32::MAX));
    let packets = session.apply_zone_player_death_penalty();
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet,
        ServerPacket::DeleteItem { unique_id, count }
            if *unique_id == worn.unique_id && *count == worn.count))
            .count(),
        1,
        "Crystal deletes the equipment instance by its advertised UID/count"
    );
    let save = session.active_character_checkpoint().unwrap();
    assert!(!save.has_rented_item);
    assert!(!save.equipment_items_json.iter().any(|state| {
        let item: Value = serde_json::from_str(state).unwrap();
        item["user_item_unique_id"] == uid
    }));
    let returned = returned_rental_items(&config);
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0]["key"], key);
    assert_eq!(returned[0]["unique_id"], uid);
    session.tick();
    assert_eq!(returned_rental_items(&config).len(), 1);
}

#[test]
fn shared_death_retains_legacy_armour_uid_in_delete_packet_and_return_mail() {
    let (config, mut session, key, uid) = rental_fixture_for_slot(Some("armour"), None);
    let client_equipment = reload_with_advertised_equipment(&config, &mut session);
    let worn = client_equipment[1].as_ref().unwrap();
    assert_eq!((worn.unique_id, uid), (1, 1));
    assert!(session.apply_zone_player_damage(i32::MAX));
    let packets = session.apply_zone_player_death_penalty();
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet,
        ServerPacket::DeleteItem { unique_id, count }
            if *unique_id == worn.unique_id && *count == worn.count))
            .count(),
        1
    );
    let returned = returned_rental_items(&config);
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0]["key"], key);
    assert_eq!(
        returned[0]["unique_id"], worn.unique_id,
        "a legacy armour return must retain UID 1, not acquire bag-slot-zero UID 0"
    );
    assert!(!session
        .active_character_checkpoint()
        .unwrap()
        .equipment_items_json
        .iter()
        .any(|state| {
            let item: Value = serde_json::from_str(state).unwrap();
            item["slot"] == "armour"
        }));
    session.tick();
    assert_eq!(returned_rental_items(&config).len(), 1);
}

#[test]
fn rental_expiry_uses_the_same_advertised_equipment_uid_as_shared_death() {
    let (config, mut session, _, uid) = rental_fixture(true);
    let client_equipment = reload_with_advertised_equipment(&config, &mut session);
    let worn = client_equipment[0].as_ref().unwrap();
    assert_eq!(worn.unique_id, uid);
    let mut save = session.active_character_checkpoint().unwrap();
    for state in &mut save.equipment_items_json {
        let mut item: Value = serde_json::from_str(state).unwrap();
        if item["user_item_unique_id"] == uid {
            item["rental_expiry_binary_datetime"] = json!(1_i64);
            *state = item.to_string();
        }
    }
    session.restore_active_character_checkpoint(&save).unwrap();
    let packets = session.tick();
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet,
        ServerPacket::DeleteItem { unique_id, count }
            if *unique_id == worn.unique_id && *count == worn.count))
            .count(),
        1
    );
    let returned = returned_rental_items(&config);
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0]["unique_id"], uid);
    let again = session.tick();
    assert!(!again.iter().any(|packet| matches!(packet,
        ServerPacket::DeleteItem { unique_id, .. } if *unique_id == uid)));
    assert_eq!(returned_rental_items(&config).len(), 1);
}

#[test]
fn shared_death_penalty_cannot_return_a_living_players_rental() {
    let (config, mut session, _, _) = rental_fixture(false);
    let before = session.active_character_checkpoint().unwrap();
    assert!(session.apply_zone_player_death_penalty().is_empty());
    assert_eq!(
        session
            .active_character_checkpoint()
            .unwrap()
            .inventory_items_json,
        before.inventory_items_json
    );
    assert!(returned_rental_items(&config).is_empty());
}
