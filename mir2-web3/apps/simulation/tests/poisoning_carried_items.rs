//! Isolated in-memory casts and inventory receipts for the explicitly custom
//! Poisoning rule. These fixtures never use a service or a player save.

use mir2_game_data::crystal_item_by_name;
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, EquipmentSlot, SimulationConfig,
    SimulationSession, WorldEntitySnapshot, ZoneCollision, ZoneCommand, ZoneKey, ZoneOutbound,
    ZoneRuntime,
};
use serde_json::{json, Value};

const TARGET: u32 = 98_701;

fn carried(name: &str, container: &str, slot: u8, uid: u64, quantity: u32) -> Value {
    let info = crystal_item_by_name(name).expect("ordinary catalogue item");
    json!({
        "key": format!("crystal-item-{}", info.item_index), "name": info.name,
        "icon": info.image, "slot": slot, "unique_id": uid, "container": container,
        "quantity": quantity, "description": "isolated carried-poison fixture",
        "durability_current": null, "durability_max": null, "weight": info.weight,
        "equip_slot": "amulet", "attack": 0, "defence": 0, "heal_hp": 0, "heal_mp": 0,
        "user_item_metadata": {"item_index": info.item_index}
    })
}

fn equipped(name: &str, quantity: u32) -> Value {
    let info = crystal_item_by_name(name).expect("ordinary equipped item");
    json!({
        "key": format!("crystal-item-{}", info.item_index), "name": info.name,
        "icon": info.image, "slot": EquipmentSlot::Amulet, "shape": info.shape,
        "quantity": quantity, "description": "isolated equipped-poison fixture",
        "durability_current": info.durability.max(1), "durability_max": info.durability.max(1),
        "attack": 0, "defence": 0
    })
}

fn fixture(bag: Vec<Value>, belt: Vec<Value>, equipment: Vec<Value>) -> SimulationSession {
    let mut config = SimulationConfig::default();
    // Keep the in-memory scene independent of Crystal's built-in map spawn
    // table and safe zones; no filesystem map override or service is involved.
    config.map.file_name = "poisoning-carried-fixture".into();
    config.safe_zones.clear();
    config.visible_players.clear();
    config.visible_monsters.clear();
    let character = CharacterRecord {
        index: 0,
        name: "CarriedPoisonFixture".into(),
        level: 50,
        class: MirClass::Taoist,
        gender: MirGender::Male,
    };
    let mut save = CharacterSaveRecord::new(character.clone());
    save.map_file_name = config.map.file_name.clone();
    save.map_title = config.map.title.clone();
    save.position = config.spawn.clone();
    save.mp = 500;
    save.inventory_capacity = 86;
    save.inventory_items_json = bag.iter().map(Value::to_string).collect();
    save.belt_items_json = belt.iter().map(Value::to_string).collect();
    save.equipment_items_json = equipment.iter().map(Value::to_string).collect();
    save.equipment_items_explicit_empty = true;
    save.skill_states_json = ["poisoning", "soulfireball", "plague"]
        .map(|key| {
            json!({
                "key": key, "name": key, "description": "isolated cast fixture",
                "level": 3, "experience": 0, "hotkey": 0, "cooldown_ticks": 1,
                "delay_ms": 1, "cooldown_ends_at": 0, "cast_time_ms": 0
            })
            .to_string()
        })
        .into();
    let mut account = AccountRecord::empty();
    account.characters.push(character);
    account.saves.insert(0, save);
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .insert("carried-poison-fixture".into(), account);
    let mut session = SimulationSession::new(config);
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "carried-poison-fixture".into(),
            password: "demo".into(),
        })
        .iter()
        .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })));
    let start = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(
        start
            .iter()
            .any(|p| matches!(p, ServerPacket::StartGame { result: 4, .. })),
        "{start:?}"
    );
    let snapshot = session.world_snapshot();
    let actor = snapshot
        .entities
        .iter()
        .find(|e| Some(e.object_id) == snapshot.player_object_id)
        .unwrap();
    let target: WorldEntitySnapshot = serde_json::from_value(json!({
        "objectId": TARGET, "kind": "monster", "name": "Yob", "x": actor.x + 2,
        "y": actor.y, "direction": MirDirection::Left, "hp": 100, "maxHp": 100,
        "light": 0, "nameColourArgb": -1, "dead": false, "disposition": "hostile", "questIds": []
    }))
    .unwrap();
    assert!(
        session.apply_shared_entity_snapshot(&target),
        "isolated authoritative target mirror"
    );
    assert!(
        session.zone_monster_spawn_snapshot(TARGET).is_some(),
        "fixture target exists"
    );
    session
}

fn quantity(session: &SimulationSession, uid: u64) -> u32 {
    let snapshot = session.world_snapshot();
    snapshot
        .inventory_items
        .iter()
        .chain(&snapshot.belt_items)
        .find(|item| item.unique_id == uid)
        .map_or(0, |item| item.quantity)
}

fn assert_delete(packets: &[ServerPacket], uid: u64) {
    assert!(
        matches!(packets, [ServerPacket::DeleteItem { unique_id, count: 1 }] if *unique_id == uid),
        "exact selected stack debit: {packets:?}"
    );
}

fn assert_shared_cast_and_debit(session: &mut SimulationSession, uid: u64, shape: u8) {
    let key = format!(
        "crystal-item-{}",
        crystal_item_by_name(if shape == 1 {
            "GreenPoison"
        } else {
            "RedPoison"
        })
        .unwrap()
        .item_index
    );
    let components = session
        .shared_skill_item_consumption_components(Spell::Poisoning)
        .expect("Poisoning admission");
    assert_eq!(components.len(), 1);
    assert_eq!((&components[0].item_key, components[0].quantity), (&key, 1));
    let item_param = session.shared_skill_item_param(Spell::Poisoning);
    assert_eq!(
        item_param, shape,
        "colour must belong to the selected debit stack"
    );
    let join = session.active_zone_join_snapshot("poison-caster").unwrap();
    let sid = join.session_id.clone();
    let target = session.zone_monster_spawn_snapshot(TARGET).unwrap();
    let point = target.position.clone();
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map(&join.map_file_name),
        ZoneCollision::unbounded(),
    );
    zone.handle(ZoneCommand::Join(join));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: sid.clone(),
        monster: target,
        now_ms: 0,
    });
    let (level, damage, mp_cost, cooldown_ms) =
        session.zone_magic_attack_profile(Spell::Poisoning).unwrap();
    assert!(zone.can_player_cast_magic(
        &sid,
        TARGET,
        Spell::Poisoning,
        MirDirection::Right,
        &point,
        true,
        damage,
        mp_cost,
        cooldown_ms,
        10
    ));
    let receipt = session.commit_shared_skill_item_consumption_transaction(Spell::Poisoning);
    assert!(receipt.committed);
    assert_delete(&receipt.packets, uid);
    let out = zone.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: sid,
        object_id: TARGET,
        spell: Spell::Poisoning,
        direction: MirDirection::Right,
        target: point,
        cast: true,
        level,
        damage,
        mp_cost,
        cooldown_ms,
        item_param,
        now_ms: 10,
    });
    assert!(out.iter().any(|out| match out {
        ZoneOutbound::ToAll { packets } | ZoneOutbound::ToSession { packets, .. }
        | ZoneOutbound::ToMany { packets, .. } => packets.iter().any(|packet|
            matches!(packet, ServerPacket::ObjectPoisoned { object_id: TARGET, poison } if *poison == u16::from(shape))),
        _ => false,
    }), "actual Zone poison colour: {out:?}");
}

fn magic(session: &SimulationSession, target_id: u32, point: Option<Point>) -> ClientPacket {
    let snapshot = session.world_snapshot();
    let target = session.zone_monster_spawn_snapshot(TARGET).unwrap();
    ClientPacket::Magic {
        object_id: snapshot.player_object_id.unwrap(),
        spell: Spell::Poisoning,
        direction: MirDirection::Right,
        target_id,
        location: point.unwrap_or(target.position),
        spell_target_lock: true,
    }
}

#[test]
fn poisoning_bag_green_and_red_bind_actual_zone_colour_to_exact_receipt_debit() {
    for (name, shape) in [("GreenPoison", 1), ("RedPoison", 2)] {
        for (container, slot, uid) in [("bag1", 8, 71_008), ("bag2", 1, 73_001)] {
            let mut session = fixture(vec![carried(name, container, slot, uid, 2)], vec![], vec![]);
            assert_shared_cast_and_debit(&mut session, uid, shape);
            assert_eq!(quantity(&session, uid), 1);
        }
    }
}

#[test]
fn poisoning_preserves_equipped_priority_then_selects_bag_before_belt() {
    let mut session = fixture(
        vec![
            carried("GreenPoison", "bag1", 8, 71_008, 1),
            carried("RedPoison", "bag2", 0, 73_000, 1),
        ],
        vec![carried("RedPoison", "belt", 0, 72_000, 1)],
        vec![equipped("RedPoison", 1)],
    );
    assert_shared_cast_and_debit(&mut session, 9, 2);
    assert_eq!(quantity(&session, 71_008), 1);
    assert_eq!(quantity(&session, 72_000), 1);
    assert_shared_cast_and_debit(&mut session, 71_008, 1);
    assert_eq!(quantity(&session, 71_008), 0);
    assert_shared_cast_and_debit(&mut session, 73_000, 2);
    assert_eq!(quantity(&session, 73_000), 0);
    assert_shared_cast_and_debit(&mut session, 72_000, 2);
    assert_eq!(quantity(&session, 72_000), 0);
    assert_eq!(session.shared_skill_item_param(Spell::Poisoning), 0);
    assert!(session
        .shared_skill_item_consumption_components(Spell::Poisoning)
        .is_none());
    let exhausted = session.commit_shared_skill_item_consumption_transaction(Spell::Poisoning);
    assert!(!exhausted.committed && exhausted.packets.is_empty());
}

#[test]
fn poisoning_carried_slot_order_is_deterministic_and_belt_uses_real_uid() {
    let mut bag = fixture(
        vec![
            carried("RedPoison", "bag1", 12, 71_012, 1),
            carried("GreenPoison", "bag1", 7, 71_007, 1),
        ],
        vec![],
        vec![],
    );
    assert_shared_cast_and_debit(&mut bag, 71_007, 1);
    assert_eq!(quantity(&bag, 71_012), 1);
    let mut belt = fixture(
        vec![],
        vec![
            carried("GreenPoison", "belt", 5, 72_005, 1),
            carried("RedPoison", "belt", 1, 72_001, 1),
        ],
        vec![],
    );
    assert_shared_cast_and_debit(&mut belt, 72_001, 2);
    assert_eq!(quantity(&belt, 72_005), 1);
}

#[test]
fn poisoning_missing_broken_or_non_poison_supply_does_not_debit() {
    let mut broken = carried("GreenPoison", "bag1", 3, 71_003, 2);
    broken["durability_current"] = json!(0);
    broken["durability_max"] = json!(100);
    let mut session = fixture(
        vec![carried("Amulet", "bag1", 1, 71_001, 20), broken],
        vec![],
        vec![],
    );
    assert!(session
        .shared_skill_item_consumption_components(Spell::Poisoning)
        .is_none());
    assert_eq!(session.shared_skill_item_param(Spell::Poisoning), 0);
    let before = session.world_snapshot().inventory_items;
    let receipt = session.commit_shared_skill_item_consumption_transaction(Spell::Poisoning);
    assert!(!receipt.committed && receipt.packets.is_empty());
    assert_eq!(session.world_snapshot().inventory_items, before);
    let command = magic(&session, TARGET, None);
    assert!(
        !session.handle_packet(command).iter().any(|packet| matches!(
            packet,
            ServerPacket::DeleteItem { .. } | ServerPacket::Magic { cast: true, .. }
        ))
    );
    assert_eq!(quantity(&session, 71_001), 20);
    assert_eq!(quantity(&session, 71_003), 2);
}

#[test]
fn poisoning_ordinary_personal_cast_consumes_carried_green_and_red_once() {
    for (name, shape) in [("GreenPoison", 1), ("RedPoison", 2)] {
        let mut session = fixture(vec![carried(name, "bag1", 8, 71_008, 2)], vec![], vec![]);
        let command = magic(&session, TARGET, None);
        let out = session.handle_packet(command);
        assert!(
            out.iter().any(|p| matches!(
                p,
                ServerPacket::DeleteItem {
                    unique_id: 71_008,
                    count: 1
                }
            )),
            "{out:?}"
        );
        assert_eq!(quantity(&session, 71_008), 1);
        let mut delayed = Vec::new();
        for _ in 0..2 {
            delayed.extend(session.tick());
        }
        assert!(out.iter().chain(delayed.iter()).any(|p|
            matches!(p, ServerPacket::ObjectPoisoned { object_id: TARGET, poison } if *poison == shape)),
            "actual personal poison colour: {delayed:?}");
    }
}

#[test]
fn poisoning_rejected_target_mp_and_cooldown_preserve_carried_supply() {
    for rejection in ["target", "mp", "cooldown"] {
        let mut session = fixture(
            vec![carried("GreenPoison", "bag1", 8, 71_008, 3)],
            vec![],
            vec![],
        );
        if rejection == "mp" {
            session.force_authoritative_player_vitals(None, Some(0));
        }
        if rejection == "cooldown" {
            let command = magic(&session, TARGET, None);
            let out = session.handle_packet(command);
            assert!(out
                .iter()
                .any(|p| matches!(p, ServerPacket::DeleteItem { count: 1, .. })));
        }
        let before = quantity(&session, 71_008);
        let command = magic(
            &session,
            if rejection == "target" {
                999_999
            } else {
                TARGET
            },
            None,
        );
        let rejected = session.handle_packet(command);
        assert!(
            !rejected
                .iter()
                .any(|p| matches!(p, ServerPacket::DeleteItem { .. })),
            "{rejection}: {rejected:?}"
        );
        assert_eq!(
            quantity(&session, 71_008),
            before,
            "{rejection} must not debit"
        );
    }
}

#[test]
fn poisoning_carried_rule_does_not_extend_other_poison_or_amulet_spells() {
    let mut session = fixture(
        vec![carried("GreenPoison", "bag1", 8, 71_008, 20)],
        vec![],
        vec![],
    );
    for spell in [
        Spell::PoisonSword,
        Spell::PoisonShot,
        Spell::CrippleShot,
        Spell::PoisonCloud,
        Spell::SoulFireBall,
    ] {
        assert!(
            session
                .shared_skill_item_consumption_components(spell)
                .is_none(),
            "{spell:?}"
        );
        let receipt = session.commit_shared_skill_item_consumption_transaction(spell);
        assert!(
            !receipt.committed && receipt.packets.is_empty(),
            "{spell:?}"
        );
        assert_eq!(quantity(&session, 71_008), 20);
    }
    let mut plague = fixture(
        vec![carried("RedPoison", "bag1", 8, 71_008, 20)],
        vec![],
        vec![equipped("Amulet", 10)],
    );
    assert_eq!(plague.shared_skill_item_param(Spell::Plague), 0);
    assert_eq!(
        plague
            .shared_skill_item_consumption_components(Spell::Plague)
            .unwrap()
            .len(),
        1
    );
    assert!(
        plague
            .commit_shared_skill_item_consumption_transaction(Spell::Plague)
            .committed
    );
    assert_eq!(quantity(&plague, 71_008), 20);
}
