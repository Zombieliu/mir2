use std::collections::BTreeMap;

use mir2_protocol::{ChatType, MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    LogicalWorldId, PartitionId, PartitionKey, PayloadDigest, SessionId, ZoneCommand, ZoneJoin,
    ZoneKey, ZoneManager,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

fn world() -> LogicalWorldId {
    LogicalWorldId::legacy_execution_domain("admission-world").unwrap()
}
fn join(map: &str, number: u32) -> ZoneCommand {
    ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new(format!("owner-{number}")),
        account_id: format!("account-{number}"),
        character_index: number as i32,
        object_id: 100 + number,
        name: format!("Owner{number}"),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 7,
        hp: 40,
        max_hp: 60,
        mp: 30,
        map_file_name: map.into(),
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    })
}
fn bound() -> ZoneManager {
    let mut manager = ZoneManager::new();
    manager.bind_logical_world(world()).unwrap();
    manager
}

#[test]
fn actual_join_command_is_numbered_once_and_cannot_rebind_its_world() {
    let mut manager = bound();
    let command = join("admission-a", 1);
    manager.handle(command.clone());
    let receipt = manager.last_admitted_action().unwrap();
    assert_eq!(receipt.id().world(), &world());
    assert_eq!(receipt.id().zone(), &ZoneKey::for_map("admission-a"));
    assert_eq!(receipt.id().source_sequence(), 1);
    assert_eq!(
        receipt.payload_digest(),
        &PayloadDigest::of_bytes(&serde_json::to_vec(&command).unwrap())
    );
    assert_eq!(manager.admitted_action_count(), 1);
    let original = manager.checkpoint_bytes().unwrap();
    manager.bind_logical_world(world()).unwrap();
    assert_eq!(manager.checkpoint_bytes().unwrap(), original);
    assert!(manager
        .bind_logical_world(LogicalWorldId::legacy_execution_domain("another-world").unwrap())
        .is_err());
    assert_eq!(manager.checkpoint_bytes().unwrap(), original);
}

#[test]
fn full_and_fast_timer_entry_points_do_not_double_number_a_single_zone() {
    let mut manager = bound();
    manager.handle(join("admission-timers", 1));
    manager.tick_all(100);
    assert_eq!(
        manager.admitted_action_count(),
        2,
        "small-zone fallback must not admit twice"
    );
    let full = manager
        .last_admitted_action()
        .unwrap()
        .payload_digest()
        .clone();
    manager.tick_pending_movement(100);
    assert_eq!(manager.admitted_action_count(), 3);
    assert_ne!(
        manager.last_admitted_action().unwrap().payload_digest(),
        &full,
        "movement-only and full timers have distinct canonical payloads"
    );
    manager.tick_all_sequential(101);
    assert_eq!(manager.admitted_action_count(), 4);
    manager.handle(ZoneCommand::Tick { now_ms: 102 });
    assert_eq!(manager.admitted_action_count(), 5);
    let before = manager.admitted_action_count();
    let partition = PartitionKey::whole_zone(ZoneKey::for_map("admission-timers"));
    manager
        .handle_for_partition(partition, ZoneCommand::Tick { now_ms: 103 })
        .unwrap();
    assert_eq!(manager.admitted_action_count(), before + 1);
}

#[test]
fn unknown_partitions_and_missing_or_foreign_routes_do_not_consume_identity() {
    let mut manager = bound();
    manager.handle(join("admission-routes", 1));
    let before = manager.checkpoint_bytes().unwrap();
    let turn = ZoneCommand::Turn {
        session_id: SessionId::new("owner-1"),
        direction: MirDirection::Left,
        now_ms: 10,
    };
    for partition in [
        PartitionKey {
            zone: ZoneKey::for_map("admission-routes"),
            partition_id: PartitionId(7),
        },
        PartitionKey::whole_zone(ZoneKey::for_map("another-map")),
    ] {
        assert!(manager
            .handle_for_partition(partition, turn.clone())
            .is_err());
        assert_eq!(manager.checkpoint_bytes().unwrap(), before);
    }
    assert!(manager
        .handle(ZoneCommand::Turn {
            session_id: SessionId::new("missing"),
            direction: MirDirection::Left,
            now_ms: 10
        })
        .is_empty());
    assert_eq!(manager.checkpoint_bytes().unwrap(), before);
}

#[test]
fn trusted_packet_payload_uses_the_actual_binary_protocol_frames() {
    let mut manager = bound();
    manager.handle(join("admission-packets", 1));
    let packet = ServerPacket::Chat {
        message: "wire identity".into(),
        chat_type: ChatType::Normal,
    };
    let command = ZoneCommand::BroadcastPackets {
        session_id: SessionId::new("owner-1"),
        owner_local_object_id: 101,
        packets: vec![packet.clone()],
        now_ms: 17,
    };
    let payload = serde_json::to_value(&command).unwrap();
    assert_eq!(
        payload["BroadcastPackets"]["packets"][0],
        serde_json::to_value(mir2_protocol::encode_server_packet(&packet).unwrap()).unwrap()
    );
    manager.handle(command.clone());
    assert_eq!(
        manager.last_admitted_action().unwrap().payload_digest(),
        &PayloadDigest::of_bytes(&serde_json::to_vec(&command).unwrap())
    );
}

#[test]
fn presentation_message_never_creates_a_map_or_an_unrecorded_authority() {
    let mut manager = bound();
    let absent = ZoneKey::for_map("absent-presentation-map");
    let before = manager.checkpoint_bytes().unwrap();
    assert!(manager
        .broadcast_world_event_message(absent.clone(), "No recipients")
        .is_empty());
    assert!(manager.partition_for_key(&absent).is_none());
    assert_eq!(manager.checkpoint_bytes().unwrap(), before);
    manager.handle(join("admission-message", 1));
    let before = manager.checkpoint_bytes().unwrap();
    let out = manager.broadcast_world_event_message(
        ZoneKey::for_map("admission-message"),
        "Existing recipients",
    );
    assert!(out.iter().any(|out| match out {
        mir2_simulation::ZoneOutbound::ToAll { packets }
        | mir2_simulation::ZoneOutbound::ToSession { packets, .. }
        | mir2_simulation::ZoneOutbound::ToMany { packets, .. } => packets.iter().any(
            |p| matches!(p, ServerPacket::Chat { message, .. } if message == "Existing recipients")
        ),
        _ => false,
    }));
    assert_eq!(manager.checkpoint_bytes().unwrap(), before);
}

#[test]
fn specialized_trusted_decisions_and_public_pet_timer_share_the_manager_stream() {
    let mut manager = bound();
    manager.handle(join("admission-special", 1));
    let session = SessionId::new("owner-1");
    manager.commit_owner_vitals(&session, 30, 60, 25, false);
    assert_eq!(manager.admitted_action_count(), 2);
    assert_eq!(manager.player_vitals(&session), Some((30, 60, 25)));
    let expected = serde_json::to_vec(&(
        1_u32,
        "owner_vitals_commit",
        (&session, 30_i32, 60_i32, 25_i32, false),
    ))
    .unwrap();
    assert_eq!(
        manager.last_admitted_action().unwrap().payload_digest(),
        &PayloadDigest::of_bytes(&expected)
    );
    manager
        .restore_player_saved_pets(
            &session,
            &mir2_simulation::ZoneSavedPetSnapshot::default(),
            10,
        )
        .unwrap();
    assert_eq!(manager.admitted_action_count(), 3);
    manager.sync_intelligent_creature(&session, None, Default::default(), Vec::new(), true, 10);
    assert_eq!(manager.admitted_action_count(), 4);
    manager.request_intelligent_creature_pickup(&session, false, Point { x: 10, y: 10 });
    assert_eq!(
        manager.admitted_action_count(),
        5,
        "a trusted gameplay rejection still has an admission identity"
    );
    manager.refresh_online_pets_at(10);
    assert_eq!(manager.admitted_action_count(), 6);
    manager.tick_all(10);
    assert_eq!(
        manager.admitted_action_count(),
        7,
        "inline pet refresh must reuse the enclosing tick admission"
    );
    let spawn = mir2_simulation::ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: 900,
        name: "Scarecrow".into(),
        name_colour_argb: -1,
        image: 1,
        ai: 0,
        disposition: Some(mir2_simulation::WorldEntityDisposition::Hostile),
        level: 1,
        max_hp: 20,
        hp: 20,
        experience: 1,
        move_speed_ms: 1000,
        attack_speed_ms: 1000,
        friendly_guild: None,
        position: Point { x: 12, y: 10 },
        direction: MirDirection::Left,
        defense: Default::default(),
        respawn: None,
        drops: Vec::new(),
    };
    assert!(
        manager
            .spawn_world_event_monster(ZoneKey::for_map("admission-special"), &spawn, 10)
            .0
    );
    assert_eq!(manager.admitted_action_count(), 8);
    assert!(manager
        .native_monster_snapshots(&ZoneKey::for_map("admission-special"))
        .iter()
        .any(|monster| monster.object_id == 900));
}

#[test]
fn v4_cold_restore_retains_world_sequence_and_receipt_without_online_authority() {
    let mut manager = bound();
    manager.handle(join("admission-save", 1));
    manager.tick_all(100);
    let receipt = manager.last_admitted_action().unwrap().clone();
    let bytes = manager.checkpoint_bytes().unwrap();
    let mut restored = ZoneManager::restore_checkpoint(&bytes).unwrap();
    assert_eq!(restored.logical_world(), Some(&world()));
    assert_eq!(restored.admitted_action_count(), 2);
    assert_eq!(restored.last_admitted_action(), Some(&receipt));
    assert!(restored
        .online_owner_proof_for_session(&SessionId::new("owner-1"))
        .is_none());
    restored.tick_pending_movement(101);
    assert_eq!(
        restored
            .last_admitted_action()
            .unwrap()
            .id()
            .source_sequence(),
        3
    );
    let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(original["version"], 4);
    for (field, value) in [
        ("admission_sequence", serde_json::json!(999)),
        (
            "logical_world",
            serde_json::to_value(LogicalWorldId::legacy_execution_domain("foreign").unwrap())
                .unwrap(),
        ),
    ] {
        let mut tampered = original.clone();
        tampered[field] = value;
        assert!(ZoneManager::restore_checkpoint(&serde_json::to_vec(&tampered).unwrap()).is_err());
    }
}

// Deliberately serialize the exact old committed shape, rather than relabel a
// v4 snapshot and silently leave newly introduced fields outside an old root.
#[derive(Serialize)]
struct LegacyFixture {
    version: u32,
    zones: Vec<(ZoneKey, String)>,
    session_zones: BTreeMap<SessionId, ZoneKey>,
    #[serde(skip_serializing_if = "String::is_empty")]
    state_root: String,
}
fn legacy(version: u32) -> Vec<u8> {
    let mut manager = ZoneManager::new();
    manager.handle(join("legacy-admission", 1));
    let current: serde_json::Value =
        serde_json::from_slice(&manager.checkpoint_bytes().unwrap()).unwrap();
    let mut fixture = LegacyFixture {
        version,
        zones: serde_json::from_value(current["zones"].clone()).unwrap(),
        session_zones: serde_json::from_value(current["session_zones"].clone()).unwrap(),
        state_root: String::new(),
    };
    if version == 3 {
        let mut hash = Sha256::new();
        hash.update(b"obelisk.mir2.zone-manager.v3\0");
        hash.update(serde_json::to_vec(&fixture).unwrap());
        fixture.state_root = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    }
    serde_json::to_vec(&fixture).unwrap()
}

#[test]
fn exact_legacy_v2_v3_load_then_bind_without_importing_future_admission_fields() {
    for version in [2, 3] {
        let bytes = legacy(version);
        let mut restored = ZoneManager::restore_checkpoint(&bytes).unwrap();
        assert!(restored.logical_world().is_none());
        assert_eq!(restored.admitted_action_count(), 0);
        assert!(restored.last_admitted_action().is_none());
        assert!(restored
            .player_transform(&SessionId::new("owner-1"))
            .is_some());
        restored.bind_logical_world(world()).unwrap();
        restored.tick_all(100);
        assert_eq!(
            restored
                .last_admitted_action()
                .unwrap()
                .id()
                .source_sequence(),
            1
        );
        let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for field in [
            "logical_world",
            "admission_sequence",
            "last_admission",
            "map_random_master",
        ] {
            let mut injected = original.clone();
            injected[field] = serde_json::json!(null);
            assert!(
                ZoneManager::restore_checkpoint(&serde_json::to_vec(&injected).unwrap()).is_err()
            );
        }
        if version == 3 {
            let mut tampered = original;
            tampered["session_zones"] = serde_json::json!({});
            assert!(
                ZoneManager::restore_checkpoint(&serde_json::to_vec(&tampered).unwrap()).is_err()
            );
        }
    }
}

fn runtime_value(manager: &ZoneManager, key: &ZoneKey) -> serde_json::Value {
    serde_json::from_slice(&manager.zone(key).unwrap().checkpoint_bytes().unwrap()).unwrap()
}

#[test]
fn shared_v4_base_deterministically_seeds_future_maps_and_real_business_draws() {
    let mut origin = bound();
    let existing = ZoneKey::for_map("seed-existing");
    origin.handle_for_key(existing.clone(), ZoneCommand::Tick { now_ms: 0 });
    let original_random = runtime_value(&origin, &existing)["combat_random"].clone();
    let base = origin.checkpoint_bytes().unwrap();
    let mut source = ZoneManager::restore_checkpoint(&base).unwrap();
    let mut standby = ZoneManager::restore_checkpoint(&base).unwrap();
    assert_eq!(
        runtime_value(&source, &existing)["combat_random"],
        original_random,
        "existing map entropy must not be rederived or replaced on restore"
    );
    let future = ZoneKey::for_map("seed-future");
    let mut actor = match join("seed-future", 2) {
        ZoneCommand::Join(actor) => actor,
        _ => unreachable!(),
    };
    actor.class = MirClass::Taoist;
    actor.level = 40;
    actor.hp = 1000;
    actor.max_hp = 1000;
    actor.mp = 1000;
    actor.chat_profile.attack_mode = 5;
    for manager in [&mut source, &mut standby] {
        manager.handle(ZoneCommand::Join(actor.clone()));
        manager.handle(ZoneCommand::sync_player_combat_state(
            actor.session_id.clone(),
            MirClass::Taoist,
            true,
            false,
            false,
            false,
            false,
            false,
        ));
        manager.handle(ZoneCommand::PlayerCastMagic {
            session_id: actor.session_id.clone(),
            object_id: 0,
            spell: Spell::SummonSkeleton,
            direction: MirDirection::Right,
            target: Point { x: 11, y: 10 },
            cast: true,
            level: 2,
            damage: 0,
            mp_cost: 7,
            cooldown_ms: 1000,
            now_ms: 10,
        });
        manager.tick_all(510);
        let checkpoint = runtime_value(manager, &future);
        let pet = checkpoint["native_monsters"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, pet)| {
                pet["master_object_id"] == actor.object_id && pet["name"] == "BoneFamiliar"
            })
            .expect("normal delayed summon must create the owned pet")
            .1;
        let pet_id = pet["master_object_id"].as_u64().unwrap();
        assert_eq!(pet_id, u64::from(actor.object_id));
        let pet_object_id = checkpoint["native_monsters"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, pet)| {
                pet["master_object_id"] == actor.object_id && pet["name"] == "BoneFamiliar"
            })
            .unwrap()
            .0
            .parse::<u32>()
            .unwrap();
        let target = serde_json::from_value(pet["position"].clone()).unwrap();
        manager.handle(ZoneCommand::PlayerCastMagic {
            session_id: actor.session_id.clone(),
            object_id: pet_object_id,
            spell: Spell::ElectricShock,
            direction: MirDirection::Right,
            target,
            cast: true,
            level: 3,
            damage: 0,
            mp_cost: 0,
            cooldown_ms: 1,
            now_ms: 1820,
        });
        manager.tick_all(2320);
    }
    let left = runtime_value(&source, &future);
    let right = runtime_value(&standby, &future);
    assert_eq!(left["combat_random"], right["combat_random"]);
    assert!(
        left["combat_random"]["draws"].as_u64().unwrap() > 0,
        "a real ElectricShock impact must consume business entropy"
    );
    assert_eq!(
        left["native_monsters"], right["native_monsters"],
        "real pet control outcome must be identical despite separate cold admission namespaces"
    );
    assert_ne!(
        left["combat_random"]["seed"], original_random["seed"],
        "different logical map keys must not share the random stream"
    );
    let original: serde_json::Value = serde_json::from_slice(&base).unwrap();
    for change in ["seed", "version", "missing"] {
        let mut value = original.clone();
        match change {
            "seed" => {
                value["map_random_master"]["seed"][0] =
                    ((value["map_random_master"]["seed"][0].as_u64().unwrap() + 1) % 256).into()
            }
            "version" => value["map_random_master"]["version"] = 99.into(),
            "missing" => {
                value.as_object_mut().unwrap().remove("map_random_master");
            }
            _ => unreachable!(),
        }
        assert!(ZoneManager::restore_checkpoint(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}

#[test]
#[cfg(feature = "test-support")]
fn numbered_parallel_and_serial_ticks_have_identical_nonempty_results_and_checkpoint() {
    let mut parallel = bound();
    for n in 1..=8 {
        parallel.handle(join(&format!("admission-map-{n}"), n));
    }
    let mut serial = parallel.fork_for_deterministic_reference();
    let mut nonempty = false;
    for now in [100, 4011, 8012] {
        let expected = serial.tick_all_sequential(now);
        let actual = parallel.tick_all(now);
        nonempty |= !actual.is_empty();
        assert_eq!(actual, expected);
        assert_eq!(
            parallel.last_admitted_action(),
            serial.last_admitted_action()
        );
        assert_eq!(
            parallel.last_admitted_action().unwrap().id().zone(),
            &ZoneKey::for_map("admission-map-8")
        );
        assert_eq!(
            parallel.checkpoint_bytes().unwrap(),
            serial.checkpoint_bytes().unwrap()
        );
    }
    assert!(nonempty);
    assert_eq!(parallel.admitted_action_count(), 32);
}
