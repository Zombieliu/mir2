//! Ordinary Magic packets through the Gateway's shared in-process inventory
//! service. These isolated fixtures open no listener and use no player store.
use super::*;
use serde_json::{json, Value};

const TARGET: u32 = 260_970;

fn poison(name: &str, container: &str, slot: u8, uid: u64, quantity: u32) -> Value {
    let info = mir2_game_data::crystal_item_by_name(name).unwrap();
    json!({
        "key": format!("crystal-item-{}", info.item_index), "name": info.name,
        "icon": info.image, "slot": slot, "unique_id": uid, "container": container,
        "quantity": quantity, "description": "isolated Gateway carried-poison fixture",
        "durability_current": null, "durability_max": null, "weight": info.weight,
        "equip_slot": "amulet", "attack": 0, "defence": 0, "heal_hp": 0, "heal_mp": 0,
        "user_item_metadata": {"item_index": info.item_index}
    })
}

fn fixture(
    name: &str,
    bag: Vec<Value>,
    belt: Vec<Value>,
    equipment: Option<&str>,
) -> SharedInProcessZoneSessionRuntime {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_new_runtime_with_class(&mut runtime, name, name, MirClass::Taoist);
    if let Some(name) = equipment {
        equip_runtime_crystal_items(
            &mut runtime,
            &[(name, mir2_simulation::EquipmentSlot::Amulet, 1)],
        );
    }
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.level = 50;
    save.position = Point { x: 340, y: 550 };
    save.mp = 500;
    save.max_mp = 500;
    save.inventory_capacity = 86;
    save.inventory_items_json = bag.iter().map(Value::to_string).collect();
    save.belt_items_json = belt.iter().map(Value::to_string).collect();
    if equipment.is_none() {
        save.equipment_items_json.clear();
    }
    save.equipment_items_explicit_empty = equipment.is_none();
    save.skill_states_json = vec![json!({
        "key": "poisoning", "name": "Poisoning", "description": "isolated Gateway fixture",
        "level": 3, "experience": 0, "cooldown_ticks": 1, "cooldown_ends_at": 0
    })
    .to_string()];
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    let key = runtime.current_presence_key().unwrap();
    let mut target = shared_monster_entity(TARGET);
    target.name = "Yob".into();
    target.ai = Some(0);
    target.x = 342;
    target.y = 550;
    target.hp = Some(100);
    target.max_hp = Some(100);
    target.disposition = mir2_simulation::WorldEntityDisposition::Hostile;
    target.sprite = None;
    {
        let mut state = shared.lock().unwrap();
        state.update_player_transform(&key, save.position.clone(), MirDirection::Right);
        state.sync_map_layer(
            "0".into(),
            vec![target.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
    }
    runtime
        .inner
        .force_authoritative_player_transform(save.position.clone(), MirDirection::Right);
    runtime.sync_zone_snapshot();
    let sid = runtime.current_zone_session_id().unwrap();
    runtime.sync_authoritative_zone_combat_state(&sid).unwrap();
    runtime.dispatch_zone_player_command(
        ZoneCommand::SyncPlayerTransform {
            session_id: sid.clone(),
            position: save.position.clone(),
            direction: MirDirection::Right,
        },
        false,
    );
    let snapshot = runtime.inner.world_snapshot();
    // Checkpoint restoration updates the personal session. The shared Zone
    // remains the vitals authority and needs the matching trusted fixture.
    runtime.dispatch_zone_player_command(
        ZoneCommand::SyncPlayerVitals {
            session_id: sid.clone(),
            hp: snapshot.player_hp.unwrap(),
            max_hp: snapshot.player_max_hp.unwrap(),
            mp: 500,
        },
        false,
    );
    assert_eq!(
        shared
            .lock()
            .unwrap()
            .zone_manager
            .player_vitals(&sid)
            .unwrap()
            .2,
        500
    );
    assert!(shared.lock().unwrap().shared_entity("0", TARGET).is_some());
    if runtime
        .inner
        .shared_skill_item_consumption_components(Spell::Poisoning)
        .is_some()
    {
        assert!(runtime
            .inner
            .zone_magic_attack_profile(Spell::Poisoning)
            .is_some());
        assert!(
            runtime
                .prepare_zone_native_player_attack(&magic_command(&runtime, TARGET))
                .is_some(),
            "ordinary Magic must enter the real Gateway Zone dispatcher"
        );
    }
    runtime
}

fn quantity(runtime: &SharedInProcessZoneSessionRuntime, uid: u64) -> u32 {
    let snapshot = runtime.inner.world_snapshot();
    snapshot
        .inventory_items
        .iter()
        .chain(&snapshot.belt_items)
        .find(|item| item.unique_id == uid)
        .map_or(0, |item| item.quantity)
}

fn magic_command(runtime: &SharedInProcessZoneSessionRuntime, target_id: u32) -> WorldCommand {
    let own = runtime.inner.world_snapshot().player_object_id.unwrap();
    WorldCommand::ClientPacket(ClientPacket::Magic {
        object_id: own,
        spell: Spell::Poisoning,
        direction: MirDirection::Right,
        target_id,
        location: Point { x: 342, y: 550 },
        spell_target_lock: true,
    })
}

fn cast(runtime: &mut SharedInProcessZoneSessionRuntime, target_id: u32) -> Vec<ServerPacket> {
    runtime.execute(magic_command(runtime, target_id)).unwrap()
}

fn accepted_debit(out: &[ServerPacket], uid: u64, shape: u16) {
    assert!(
        out.iter().any(|p| matches!(
            p,
            ServerPacket::Magic {
                spell: Spell::Poisoning,
                cast: true,
                ..
            }
        )),
        "{out:?}"
    );
    let debits = out
        .iter()
        .filter_map(|p| match p {
            ServerPacket::DeleteItem { unique_id, count } => Some((*unique_id, *count)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(debits, vec![(uid, 1)]);
    assert!(out.iter().any(|p| matches!(p, ServerPacket::ObjectPoisoned { object_id: TARGET, poison } if *poison == shape)), "{out:?}");
}

#[test]
fn gateway_poisoning_bag2_green_red_commits_exact_uid_colour_and_rejects_cooldown() {
    for (name, fixture_name, shape) in [
        ("GreenPoison", "PoisonBagGreen", 1),
        ("RedPoison", "PoisonBagRed", 2),
    ] {
        let mut runtime = fixture(
            fixture_name,
            vec![poison(name, "bag2", 1, 73_001, 2)],
            vec![],
            None,
        );
        accepted_debit(&cast(&mut runtime, TARGET), 73_001, shape);
        assert_eq!(quantity(&runtime, 73_001), 1);
        let rejected = cast(&mut runtime, TARGET);
        assert!(
            !rejected.iter().any(|p| matches!(
                p,
                ServerPacket::DeleteItem { .. } | ServerPacket::Magic { cast: true, .. }
            )),
            "{rejected:?}"
        );
        assert_eq!(quantity(&runtime, 73_001), 1);
    }
}

#[test]
fn gateway_poisoning_belt_and_equipped_priority_use_the_selected_receipt_only() {
    let mut belt = fixture(
        "PoisonBelt",
        vec![],
        vec![poison("GreenPoison", "belt", 1, 72_001, 1)],
        None,
    );
    accepted_debit(&cast(&mut belt, TARGET), 72_001, 1);
    assert_eq!(quantity(&belt, 72_001), 0);
    let mut equipped = fixture(
        "PoisonEquipped",
        vec![poison("GreenPoison", "bag1", 8, 71_008, 1)],
        vec![poison("GreenPoison", "belt", 1, 72_001, 1)],
        Some("RedPoison"),
    );
    accepted_debit(&cast(&mut equipped, TARGET), 9, 2);
    assert_eq!(quantity(&equipped, 71_008), 1);
    assert_eq!(quantity(&equipped, 72_001), 1);
}

#[test]
fn gateway_poisoning_missing_or_rejected_unknown_target_does_not_debit() {
    let mut missing = fixture(
        "PoisonMissing",
        vec![poison("Amulet", "bag1", 8, 71_008, 10)],
        vec![],
        None,
    );
    let rejected = cast(&mut missing, TARGET);
    assert!(
        !rejected.iter().any(|p| matches!(
            p,
            ServerPacket::DeleteItem { .. } | ServerPacket::Magic { cast: true, .. }
        )),
        "{rejected:?}"
    );
    assert_eq!(quantity(&missing, 71_008), 10);
    let mut unknown = fixture(
        "PoisonUnknown",
        vec![poison("RedPoison", "bag1", 8, 71_008, 2)],
        vec![],
        None,
    );
    let rejected = cast(&mut unknown, 999_999);
    assert!(
        !rejected.iter().any(|p| matches!(
            p,
            ServerPacket::DeleteItem { .. } | ServerPacket::Magic { cast: true, .. }
        )),
        "{rejected:?}"
    );
    assert_eq!(quantity(&unknown, 71_008), 2);
}
