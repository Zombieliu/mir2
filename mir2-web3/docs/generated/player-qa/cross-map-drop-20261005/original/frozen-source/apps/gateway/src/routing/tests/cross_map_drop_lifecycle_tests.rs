//! Ordinary local packets over a prepared saved character and source fixture.
//! This is not TCP/WSS, natural drop odds, level progression or player QA.
use super::*;
use crate::routing::pending_owned_kill_proof_key;
use serde_json::json;
use sha2::{Digest, Sha256};

const SOURCE: &str = "D001";
const DEST: &str = "D002";
const TARGET: u32 = 401_001;
const EXP: u32 = 20;

/// Narrow replacement for old internal tests' fabricated scalar award input.
/// Public Manager Join is already performed by normal StartGame in the caller.
/// This helper causes a prepared HP1 monster's real positive impact/death and
/// consumes its new source envelope; it never manufactures an online proof.
pub(super) fn issue_prepared_award(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    wanted: ZoneMonsterKillAward,
) -> Vec<ZoneMonsterKillAward> {
    static STEP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let now = SharedInProcessZoneSessionRuntime::zone_now_ms()
        + 2_000
        + STEP.fetch_add(2_000, std::sync::atomic::Ordering::Relaxed);
    let sid = runtime.current_zone_session_id().unwrap();
    let key = runtime.current_presence_key().unwrap();
    let mut state = runtime.zone_state.lock().unwrap();
    let position = state.zone_manager.player_transform(&sid).unwrap().0;
    let target = Point {
        x: position.x + 1,
        y: position.y,
    };
    let mut out = state.zone_manager.handle(ZoneCommand::SpawnMonster {
        session_id: sid.clone(),
        now_ms: now,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: wanted.monster_object_id,
            name: wanted.monster_name,
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 1,
            hp: 1,
            max_hp: 1,
            experience: wanted.experience,
            move_speed_ms: 100_000,
            attack_speed_ms: 100_000,
            friendly_guild: None,
            position: target,
            direction: MirDirection::Down,
            defense: Default::default(),
            respawn: None,
            drops: wanted.drops,
        },
    });
    out.extend(
        state
            .zone_manager
            .handle(ZoneCommand::sync_player_combat_state(
                sid.clone(),
                MirClass::Warrior,
                true,
                false,
                false,
                false,
                false,
                false,
            )),
    );
    out.extend(state.zone_manager.handle(ZoneCommand::PlayerAttackObject {
        session_id: sid,
        object_id: wanted.monster_object_id,
        direction: MirDirection::Right,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: now,
    }));
    out.extend(state.zone_manager.tick_all(now + 1_000));
    let awards = state.dispatch_zone_outbounds(out, Some(&key)).4;
    drop(state);
    assert_eq!(
        awards.len(),
        1,
        "prepared old fixture requires an actual positive source death"
    );
    awards
}

fn fixture(
    name: &str,
    config: Option<mir2_simulation::SimulationConfig>,
    seed: Option<u64>,
) -> SharedInProcessZoneSessionRuntime {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    if let Some(config) = config {
        runtime.inner = InProcessWorldRuntime::new(config);
    }
    start_new_runtime_with_class(&mut runtime, name, name, MirClass::Taoist);
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.level = 14;
    save.map_file_name = SOURCE.into();
    save.map_title = "OmaCave_1F".into();
    save.position = Point { x: 30, y: 329 };
    save.hp = 100;
    save.mp = 100;
    save.max_mp = 100;
    save.experience = 0;
    save.max_experience = 10_000;
    save.gold = 0;
    let powder = mir2_game_data::crystal_item_by_name("GreenPoison").unwrap();
    save.inventory_items_json = vec![json!({
        "key": format!("crystal-item-{}",powder.item_index), "name":powder.name,
        "icon":powder.image,"slot":1,"unique_id":71_001,"container":"bag1","quantity":3,
        "description":"prepared lawful source poison carrier","weight":powder.weight,
        "equip_slot":"amulet","attack":0,"defence":0,"heal_hp":0,"heal_mp":0,
        "user_item_metadata":{"item_index":powder.item_index}
    })
    .to_string()];
    save.skill_states_json = vec![json!({"key":"poisoning","name":"Poisoning",
        "description":"prepared learned skill","level":3,"experience":0,
        "cooldown_ticks":1,"cooldown_ends_at":0})
    .to_string()];
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    runtime
        .inner
        .force_authoritative_player_transform(save.position, MirDirection::Up);
    runtime.force_next_zone_transform_sync = true;
    runtime.sync_zone_snapshot();
    let sid = runtime.current_zone_session_id().unwrap();
    runtime.sync_authoritative_zone_combat_state(&sid).unwrap();
    assert_eq!(
        runtime.inner.world_snapshot().map_file_name.as_deref(),
        Some(SOURCE)
    );
    let mut target = shared_monster_entity(TARGET);
    target.name = "Skeleton".into();
    target.ai = Some(3);
    target.x = 31;
    target.y = 330;
    target.hp = Some(1);
    target.max_hp = Some(1);
    target.disposition = WorldEntityDisposition::Hostile;
    target.sprite = None;
    let mut gold = shared_gold_drop(0, 0, 0, None, Some(60));
    gold.source_monster = "Skeleton".into();
    gold.quantity = 25;
    let mut state = shared.lock().unwrap();
    state.sync_map_layer(
        SOURCE.into(),
        vec![target],
        BTreeSet::new(),
        vec![],
        BTreeSet::new(),
    );
    let out = state.zone_manager.handle(ZoneCommand::SpawnMonster {
        session_id: sid,
        now_ms: SharedInProcessZoneSessionRuntime::zone_now_ms(),
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: seed,
            object_id: TARGET,
            name: "Skeleton".into(),
            name_colour_argb: -1,
            image: 3,
            ai: 3,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 20,
            hp: 1,
            max_hp: 1,
            experience: EXP,
            move_speed_ms: 100_000,
            attack_speed_ms: 100_000,
            friendly_guild: None,
            position: Point { x: 31, y: 330 },
            direction: MirDirection::Up,
            defense: Default::default(),
            respawn: None,
            drops: if let Some(seed) = seed {
                mir2_simulation::zone_ground_drop_snapshots_for_monster_at_tick(
                    TARGET, "Skeleton", seed,
                )
            } else {
                vec![gold]
            },
        },
    });
    state.dispatch_zone_outbounds(out, None);
    drop(state);
    runtime.apply_pending_zone_packets();
    runtime
}

fn tick(runtime: &mut SharedInProcessZoneSessionRuntime, at: u64) {
    super::super::run_shared_zone_cadence_tick(&runtime.zone_state, at).unwrap();
}
fn source_proof(runtime: &SharedInProcessZoneSessionRuntime) -> String {
    runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .online_owner_proof_for_session(&runtime.current_zone_session_id().unwrap())
        .unwrap()
}
fn walk(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    direction: MirDirection,
    at: u64,
    expected_map: &str,
    expected: Point,
) -> Vec<ServerPacket> {
    let mut packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Walk { direction }))
        .unwrap();
    tick(runtime, at);
    packets.extend(runtime.apply_pending_zone_packets());
    let save = runtime.inner.active_character_checkpoint().unwrap();
    assert_eq!(
        save.map_file_name, expected_map,
        "ordinary movement must consume the imported route: {packets:?}"
    );
    assert_eq!(save.position, expected);
    packets
}
fn poison_and_depart(runtime: &mut SharedInProcessZoneSessionRuntime) -> u64 {
    let proof = source_proof(runtime);
    let cast = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Magic {
            object_id: runtime.inner.world_snapshot().player_object_id.unwrap(),
            spell: Spell::Poisoning,
            direction: MirDirection::DownRight,
            target_id: TARGET,
            location: Point { x: 31, y: 330 },
            spell_target_lock: true,
        }))
        .unwrap();
    assert!(
        cast.iter().any(|p| matches!(
            p,
            ServerPacket::Magic {
                spell: Spell::Poisoning,
                cast: true,
                ..
            }
        )),
        "{cast:?}"
    );
    assert!(
        cast.iter().any(|p| matches!(
            p,
            ServerPacket::ObjectPoisoned {
                object_id: TARGET,
                poison: 1
            }
        )),
        "{cast:?}"
    );
    assert!(
        cast.iter().any(|p| matches!(
            p,
            ServerPacket::DeleteItem {
                unique_id: 71_001,
                count: 1
            }
        )),
        "{cast:?}"
    );
    let base = SharedInProcessZoneSessionRuntime::zone_now_ms();
    let moved = walk(
        runtime,
        MirDirection::Up,
        base + 1_500,
        DEST,
        Point { x: 34, y: 323 },
    );
    assert!(moved
        .iter()
        .any(|p| matches!(p,ServerPacket::MapInformation{info} if info.file_name==DEST)));
    assert_eq!(
        source_proof(runtime),
        proof,
        "membership transfer must keep the global online Node"
    );
    base
}
fn source_kill(runtime: &mut SharedInProcessZoneSessionRuntime, base: u64) -> ZoneMonsterKillAward {
    tick(runtime, base + 2_100);
    let key = runtime.current_presence_key().unwrap();
    let mut shared = runtime.zone_state.lock().unwrap();
    let awards = shared.take_pending_zone_monster_kill_awards(&key);
    let owner_id = shared.players[&key].zone_object_id;
    let source_ids = shared.maps[SOURCE]
        .ground_drops
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let wrong_map_has_drop = shared.maps.get(DEST).is_some_and(|m| {
        m.ground_drops
            .values()
            .any(|d| d.source_monster == "Skeleton")
    });
    let source_birth_stays_in_source = awards.iter().flat_map(|a| &a.drops).all(|d| {
        shared
            .zone_manager
            .ground_drop_snapshot_for_key(&ZoneKey::for_map(DEST), d.object_id)
            .is_none()
    });
    shared.prepend_zone_monster_kill_awards(key, awards.clone());
    drop(shared);
    assert_eq!(
        awards.len(),
        1,
        "actual positive periodic damage must choose one cross-map owner"
    );
    assert_eq!(awards[0].monster_object_id, TARGET);
    assert_eq!(awards[0].experience, EXP);
    assert!(!awards[0].drops.is_empty());
    assert!(awards[0]
        .drops
        .iter()
        .all(|d| d.owner_object_id == Some(owner_id)));
    assert!(!wrong_map_has_drop);
    assert!(awards[0]
        .drops
        .iter()
        .all(|d| source_ids.contains(&d.object_id)));
    assert!(source_birth_stays_in_source);
    awards.into_iter().next().unwrap()
}
fn assert_private_proof_absent(
    runtime: &SharedInProcessZoneSessionRuntime,
    proof: &str,
    packets: &[ServerPacket],
) {
    let snapshot = serde_json::to_string(&runtime.inner.world_snapshot()).unwrap();
    for name in [
        "online_owner",
        "online_presence",
        "online_identities",
        "pending_zone_monster_kill_proofs",
        "issued_native_monster_awards",
    ] {
        assert!(
            !snapshot.contains(&format!("\"{name}\"")),
            "proof field leaked in WorldSnapshot: {name}"
        );
    }
    for packet in packets {
        let wire = mir2_protocol::encode_server_packet(packet).unwrap();
        assert!(
            !wire.windows(proof.len()).any(|w| w == proof.as_bytes()),
            "online proof leaked in client packet"
        );
    }
}
fn walk_to_drop(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    drop: &GroundDropSnapshot,
    base: u64,
) {
    // Spawn scatter is authoritative. Follow its actual reachable birth tile,
    // not the monster's former position or a direct transform/debug command.
    for step in 0..4_u64 {
        let p = runtime
            .inner
            .active_character_checkpoint()
            .unwrap()
            .position;
        let dx = (drop.x - p.x).signum();
        let dy = (drop.y - p.y).signum();
        let direction = match (dx, dy) {
            (0, 0) => return,
            (0, -1) => MirDirection::Up,
            (1, -1) => MirDirection::UpRight,
            (1, 0) => MirDirection::Right,
            (1, 1) => MirDirection::DownRight,
            (0, 1) => MirDirection::Down,
            (-1, 1) => MirDirection::DownLeft,
            (-1, 0) => MirDirection::Left,
            (-1, -1) => MirDirection::UpLeft,
            _ => unreachable!(),
        };
        walk(
            runtime,
            direction,
            base + 4_000 + step * 1_000,
            SOURCE,
            Point {
                x: p.x + dx,
                y: p.y + dy,
            },
        );
    }
    let p = runtime
        .inner
        .active_character_checkpoint()
        .unwrap()
        .position;
    assert_eq!(
        p,
        Point {
            x: drop.x,
            y: drop.y
        },
        "bounded ordinary path must reach actual source drop"
    );
}
fn reconnect(runtime: &mut SharedInProcessZoneSessionRuntime) -> SharedInProcessZoneSessionRuntime {
    let identity = runtime.inner.active_identity().unwrap();
    let config = runtime.inner.shared_mentor_config().unwrap();
    let shared = runtime.zone_state.clone();
    let logout = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    assert!(
        logout
            .iter()
            .any(|p| matches!(p, ServerPacket::LogOutSuccess { .. })),
        "normal logout must save: {logout:?}"
    );
    let mut next = shared_session_runtime(shared);
    next.inner = InProcessWorldRuntime::new(config);
    let login = next
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: identity.account_id.clone(),
            password: identity.account_id,
        }))
        .unwrap();
    assert!(
        login
            .iter()
            .any(|p| matches!(p, ServerPacket::LoginSuccess { .. })),
        "{login:?}"
    );
    let started = next
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: identity.character_index,
        }))
        .unwrap();
    assert!(
        started
            .iter()
            .any(|p| matches!(p, ServerPacket::UserInformation { .. })),
        "{started:?}"
    );
    next
}

#[test]
fn ordinary_walk_source_route_and_carried_poison_award_are_source_bound() {
    let bytes = include_bytes!(
        "../../../../../packages/game-data/data/generated/crystal_respawn_manifest.json"
    );
    let hash: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        hash,
        "5303f8093be7f15ddd9f860e6db1787ac96606508db3086bab38bdc4c6353dd3"
    );
    let source: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let maps = source["maps"].as_array().unwrap();
    let a = maps.iter().find(|m| m["map_file_name"] == SOURCE).unwrap();
    let b = maps.iter().find(|m| m["map_file_name"] == DEST).unwrap();
    assert_eq!(
        a["movements"][0],
        json!({"map_index":40,"source":{"x":30,"y":328},"destination":{"x":34,"y":323},"need_hole":false,"need_move":false,"conquest_index":0,"show_on_big_map":true,"icon":34})
    );
    assert_eq!(b["movements"][0]["map_index"], 39);
    let mut runtime = fixture("CrossRoute", None, None);
    let proof = source_proof(&runtime);
    let base = poison_and_depart(&mut runtime);
    let award = source_kill(&mut runtime, base);
    let packets = runtime.apply_pending_zone_packets();
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::GainExperience { amount: EXP }))
            .count(),
        1
    );
    assert!(
        !packets
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectDied{info} if info.object_id==TARGET)),
        "source AOI death must not reach a different map"
    );
    assert_private_proof_absent(&runtime, &proof, &packets);
    assert_eq!(runtime.inner.world_snapshot().player_experience, 20);
    let identity = runtime.inner.active_identity().unwrap();
    let config = runtime.inner.shared_mentor_config().unwrap();
    let saved = config.account_store.lock().unwrap().accounts[&identity.account_id].saves
        [&identity.character_index]
        .clone();
    assert_eq!(saved.experience, 20);
    assert_eq!(
        saved.guild_experience_journal.applied_kill_receipts.len(),
        1
    );
    let returned = walk(
        &mut runtime,
        MirDirection::Up,
        base + 3_000,
        SOURCE,
        Point { x: 30, y: 329 },
    );
    assert!(returned
        .iter()
        .any(|p| matches!(p,ServerPacket::MapInformation{info} if info.file_name==SOURCE)));
    assert_eq!(source_proof(&runtime), proof);
    walk_to_drop(&mut runtime, &award.drops[0], base);
    let pickup = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::PickUp))
        .unwrap();
    let amount = award
        .drops
        .iter()
        .filter_map(|d| match d.loot {
            GroundDropLootSnapshot::Gold { amount } => Some(amount),
            _ => None,
        })
        .sum::<u32>();
    assert!(
        pickup
            .iter()
            .any(|p| matches!(p,ServerPacket::GainedGold{gold} if *gold==amount)),
        "{pickup:?}"
    );
    assert_eq!(
        runtime.inner.active_character_checkpoint().unwrap().gold,
        amount
    );
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::PickUp))
        .unwrap();
    assert_eq!(
        runtime.inner.active_character_checkpoint().unwrap().gold,
        amount
    );
    let next = reconnect(&mut runtime);
    assert_eq!(
        next.inner.active_character_checkpoint().unwrap().gold,
        amount
    );
    assert_eq!(next.inner.world_snapshot().player_experience, 20);
    assert_ne!(
        source_proof(&next),
        proof,
        "fresh Login spawns a new global Node"
    );
}

#[test]
fn source_issued_award_rejects_every_identity_source_and_payload_substitution() {
    let mut runtime = fixture("CrossForgery", None, None);
    let base = poison_and_depart(&mut runtime);
    let award = source_kill(&mut runtime, base);
    let sid = runtime.current_zone_session_id().unwrap();
    let original = source_proof(&runtime);
    let key = runtime.current_presence_key().unwrap();
    let proof_key = pending_owned_kill_proof_key(&key, &award);
    let original_record = runtime
        .zone_state
        .lock()
        .unwrap()
        .pending_zone_monster_kill_proofs[&proof_key]
        .clone();
    for (field, value) in [
        ("namespace", json!("wrong")),
        ("epoch", json!(999)),
        ("session_id", json!("other")),
        ("account_id", json!("another-account")),
        ("character_index", json!(1)),
        ("object_id", json!(999)),
        ("extra", json!(true)),
    ] {
        let mut proof: serde_json::Value = serde_json::from_str(&original).unwrap();
        proof[field] = value;
        let mut state = runtime.zone_state.lock().unwrap();
        state.pending_zone_monster_kill_proofs.remove(&proof_key);
        state.dispatch_zone_outbounds(
            vec![ZoneOutbound::OwnedMonsterKillAward {
                session_id: sid.clone(),
                source: ZoneKey::for_map(SOURCE),
                online_owner: proof.to_string(),
                award: award.clone(),
            }],
            None,
        );
        assert!(
            !state
                .pending_zone_monster_kill_proofs
                .contains_key(&proof_key),
            "accepted forged {field}"
        );
    }
    let mut state = runtime.zone_state.lock().unwrap();
    for (source, mut changed) in [
        (ZoneKey::for_map(DEST), award.clone()),
        (ZoneKey::for_map(SOURCE), award.clone()),
    ] {
        if source.map_file_name == SOURCE {
            changed.experience += 1;
        }
        assert!(!state
            .zone_manager
            .issued_monster_award_is_current(&source, &sid, &original, &changed));
        state.dispatch_zone_outbounds(
            vec![ZoneOutbound::OwnedMonsterKillAward {
                session_id: sid.clone(),
                source,
                online_owner: original.clone(),
                award: changed,
            }],
            None,
        );
    }
    state
        .pending_zone_monster_kill_proofs
        .insert(proof_key, original_record);
    drop(state);
    assert_eq!(
        runtime
            .apply_pending_zone_packets()
            .iter()
            .filter(|p| matches!(p, ServerPacket::GainExperience { amount: EXP }))
            .count(),
        1
    );
    let mut state = runtime.zone_state.lock().unwrap();
    assert!(
        !state.zone_manager.issued_monster_award_is_current(
            &ZoneKey::for_map(SOURCE),
            &sid,
            &original,
            &award
        ),
        "confirmed issuance is consumed"
    );
    state.dispatch_zone_outbounds(
        vec![ZoneOutbound::OwnedMonsterKillAward {
            session_id: sid,
            source: ZoneKey::for_map(SOURCE),
            online_owner: original,
            award,
        }],
        None,
    );
    drop(state);
    assert!(runtime
        .apply_pending_zone_packets()
        .iter()
        .all(|p| !matches!(p, ServerPacket::GainExperience { .. })));
    assert_eq!(runtime.inner.world_snapshot().player_experience, 20);
}

#[test]
fn known_reward_save_failure_keeps_source_epoch_for_one_exact_retry() {
    let mut runtime = fixture("CrossRetry", None, None);
    let base = poison_and_depart(&mut runtime);
    let award = source_kill(&mut runtime, base);
    let proof = source_proof(&runtime);
    let config = runtime.inner.shared_mentor_config().unwrap();
    config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::BeforePersist,
    );
    let failed = runtime.apply_pending_zone_packets();
    assert!(!failed
        .iter()
        .any(|p| matches!(p, ServerPacket::GainExperience { .. })));
    assert_eq!(runtime.inner.world_snapshot().player_experience, 0);
    assert_eq!(source_proof(&runtime), proof);
    let sid = runtime.current_zone_session_id().unwrap();
    assert!(runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .issued_monster_award_is_current(&ZoneKey::for_map(SOURCE), &sid, &proof, &award));
    let success = runtime.apply_pending_zone_packets();
    assert_eq!(
        success
            .iter()
            .filter(|p| matches!(p, ServerPacket::GainExperience { amount: EXP }))
            .count(),
        1
    );
    assert_eq!(runtime.inner.world_snapshot().player_experience, 20);
    assert!(runtime
        .apply_pending_zone_packets()
        .iter()
        .all(|p| !matches!(p, ServerPacket::GainExperience { .. })));
}

#[test]
fn fresh_login_never_accepts_a_replayed_previous_online_award() {
    let mut runtime = fixture("CrossRelog", None, None);
    let base = poison_and_depart(&mut runtime);
    let award = source_kill(&mut runtime, base);
    let sid = runtime.current_zone_session_id().unwrap();
    let old = source_proof(&runtime);
    runtime.apply_pending_zone_packets();
    let mut next = reconnect(&mut runtime);
    assert_ne!(source_proof(&next), old);
    next.zone_state.lock().unwrap().dispatch_zone_outbounds(
        vec![ZoneOutbound::OwnedMonsterKillAward {
            session_id: sid,
            source: ZoneKey::for_map(SOURCE),
            online_owner: old,
            award,
        }],
        None,
    );
    assert!(next
        .apply_pending_zone_packets()
        .iter()
        .all(|p| !matches!(p, ServerPacket::GainExperience { .. })));
    assert_eq!(next.inner.world_snapshot().player_experience, 20);
}

#[test]
fn unknown_publication_retains_exact_source_receipt_without_faked_exp_success() {
    let root = std::path::PathBuf::from(
        std::env::var("MIR2_P3_RECEIPT_DIR").expect("isolated P3 receipt root"),
    )
    .join("fault-fixtures")
    .join(format!(
        "unknown-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let config = mir2_simulation::SimulationConfig::default()
        .with_account_store_path(root.join("accounts.json"));
    config.save_account_store().unwrap();
    let mut runtime = fixture("CrossUnknown", Some(config.clone()), None);
    let base = poison_and_depart(&mut runtime);
    let actual = source_kill(&mut runtime, base);
    let proof = source_proof(&runtime);
    let key = runtime.current_presence_key().unwrap();
    config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    let failed = runtime.apply_pending_zone_packets();
    assert!(failed.iter().all(|p| !matches!(
        p,
        ServerPacket::GainExperience { .. } | ServerPacket::LevelChanged { .. }
    )));
    assert!(
        config.save_account_store().is_err(),
        "unknown publication keeps repository frozen"
    );
    assert_eq!(source_proof(&runtime), proof);
    let queued = {
        let mut shared = runtime.zone_state.lock().unwrap();
        let awards = shared.take_pending_zone_monster_kill_awards(&key);
        shared.prepend_zone_monster_kill_awards(key.clone(), awards.clone());
        awards
    };
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].drops, actual.drops);
    assert_eq!(queued[0].monster_object_id, actual.monster_object_id);
    assert_eq!(queued[0].killed_at_ms, actual.killed_at_ms);
    assert!(queued[0].source_receipt_key.is_some());
    assert!(runtime.apply_zone_monster_kill_awards(queued).is_empty());
    let lease = ZoneOwnerLease::in_process(&ZoneId::new("test-shared-zone"));
    assert!(
        runtime.prepare_teardown_checkpoint(&lease).is_err(),
        "unconfirmed reward cannot become successful logout"
    );
    assert!(
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
            .is_err(),
        "ordinary logout must reject an unconfirmed save"
    );
    assert_eq!(
        source_proof(&runtime),
        proof,
        "rejected logout has not despawned the original online Node"
    );
}

#[test]
fn lost_proof_is_not_recreated_from_new_membership_or_fresh_login() {
    let mut runtime = fixture("CrossNoProof", None, None);
    let base = poison_and_depart(&mut runtime);
    let award = source_kill(&mut runtime, base);
    let key = runtime.current_presence_key().unwrap();
    let proof_key = pending_owned_kill_proof_key(&key, &award);
    runtime
        .zone_state
        .lock()
        .unwrap()
        .pending_zone_monster_kill_proofs
        .remove(&proof_key);
    assert!(runtime
        .apply_zone_monster_kill_awards(vec![award.clone()])
        .is_empty());
    {
        let mut state = runtime.zone_state.lock().unwrap();
        state.queue_zone_monster_kill_award(key.clone(), award.clone());
        assert!(state.take_pending_zone_monster_kill_awards(&key).is_empty());
    }
    assert_eq!(runtime.inner.world_snapshot().player_experience, 0);
    let mut next = reconnect(&mut runtime);
    assert!(next
        .apply_zone_monster_kill_awards(vec![award.clone()])
        .is_empty());
    {
        let mut state = next.zone_state.lock().unwrap();
        state.queue_zone_monster_kill_award(key.clone(), award);
        assert!(state.take_pending_zone_monster_kill_awards(&key).is_empty());
    }
    assert_eq!(next.inner.world_snapshot().player_experience, 0);
}

#[test]
fn cold_world_checkpoint_keeps_source_item_payload_and_ttl_but_revokes_all_online_owners() {
    let mut runtime = fixture("CrossCold", None, None);
    let base = poison_and_depart(&mut runtime);
    let award = source_kill(&mut runtime, base);
    let proof = source_proof(&runtime);
    let source = ZoneKey::for_map(SOURCE);
    let drop = &award.drops[0];
    let (checkpoint, clocks) = {
        let state = runtime.zone_state.lock().unwrap();
        (
            state.world_checkpoint().unwrap(),
            state
                .zone_manager
                .ground_drop_absolute_clocks_for_key(&source, drop.object_id)
                .unwrap(),
        )
    };
    assert_eq!(
        source_proof(&runtime),
        proof,
        "capturing cold state must not revoke a retained live runtime"
    );
    let cold = SharedInProcessZoneState::restore(checkpoint).unwrap();
    assert!(cold.players.is_empty());
    assert!(cold.pending_zone_monster_kill_proofs.is_empty());
    assert!(!cold.zone_manager.online_owner_proof_is_current(&proof));
    assert_eq!(
        cold.zone_manager
            .native_ground_drop_deadline_for_key(&source, drop.object_id),
        Some(None)
    );
    let restored = cold
        .zone_manager
        .ground_drop_snapshot_for_key(&source, drop.object_id)
        .unwrap();
    assert_eq!(restored.loot, drop.loot);
    assert_eq!(restored.owner_object_id, drop.owner_object_id);
    assert_eq!(
        cold.zone_manager
            .ground_drop_absolute_clocks_for_key(&source, drop.object_id)
            .unwrap()
            .1,
        clocks.1
    );
    assert_eq!(restored.ownership_remaining_ticks, None);
}

#[test]
fn source_item_exact_payload_receives_one_uid_and_survives_normal_save_login() {
    let mut selected = None;
    let mut non_single_item_rolls = Vec::new();
    for seed in 0..256_u64 {
        let rolls = mir2_simulation::zone_ground_drop_snapshots_for_monster_at_tick(
            TARGET, "Skeleton", seed,
        );
        let inventory = rolls
            .into_iter()
            .filter(|d| {
                matches!(
                    &d.loot,
                    GroundDropLootSnapshot::InventoryItem {
                        exact_item: Some(_),
                        ..
                    }
                )
            })
            .collect::<Vec<_>>();
        if inventory.len() == 1 {
            selected = Some((seed, inventory.into_iter().next().unwrap()));
            break;
        }
        non_single_item_rolls.push(seed);
    }
    let (seed, expected) =
        selected.expect("bounded source-rate seed search must keep its non-drop attempts");
    eprintln!("prepared source-rate roll seed={seed}; earlier empty/multiple-item rolls={non_single_item_rolls:?}; naturalBossDropAcceptance=false");
    let mut runtime = fixture("CrossItem", None, Some(seed));
    let base = poison_and_depart(&mut runtime);
    let award = source_kill(&mut runtime, base);
    let item = award
        .drops
        .iter()
        .find(|d| d.loot == expected.loot)
        .unwrap()
        .clone();
    let GroundDropLootSnapshot::InventoryItem {
        exact_item: Some(exact),
        ..
    } = &item.loot
    else {
        panic!("source-exact inventory drop")
    };
    assert!(!exact.uid_assigned);
    assert_eq!(exact.item.unique_id, 0);
    runtime.apply_pending_zone_packets();
    walk(
        &mut runtime,
        MirDirection::Up,
        base + 3_000,
        SOURCE,
        Point { x: 30, y: 329 },
    );
    walk_to_drop(&mut runtime, &item, base);
    let before = runtime.inner.active_character_checkpoint().unwrap();
    let pickup = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::PickUp))
        .unwrap();
    assert!(
        pickup
            .iter()
            .any(|p| matches!(p, ServerPacket::GainedItem { .. })),
        "source item must use ordinary PickUp: {pickup:?}"
    );
    let delivered = pickup
        .iter()
        .find_map(|p| match p {
            ServerPacket::GainedItem { item } => Some(item.clone()),
            _ => None,
        })
        .unwrap();
    let mut expected_wire = exact.item.clone();
    expected_wire.unique_id = delivered.unique_id;
    assert_eq!(
        delivered, expected_wire,
        "UID assignment cannot alter the source-exact item payload"
    );
    let after = runtime.inner.active_character_checkpoint().unwrap();
    let old_uids = before
        .inventory_items_json
        .iter()
        .map(|s| {
            serde_json::from_str::<serde_json::Value>(s).unwrap()["unique_id"]
                .as_u64()
                .unwrap()
        })
        .collect::<BTreeSet<_>>();
    let added = after
        .inventory_items_json
        .iter()
        .map(|s| serde_json::from_str::<serde_json::Value>(s).unwrap())
        .filter(|v| !old_uids.contains(&v["unique_id"].as_u64().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(added.len(), 1, "one source item produces one canonical UID");
    assert_ne!(added[0]["unique_id"].as_u64().unwrap(), 0);
    assert_eq!(
        added[0]["user_item_metadata"]["item_index"]
            .as_i64()
            .unwrap(),
        i64::from(exact.item.item_index)
    );
    assert_eq!(
        added[0]["quantity"].as_u64().unwrap(),
        u64::from(exact.item.count)
    );
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::PickUp))
        .unwrap();
    assert_eq!(
        runtime
            .inner
            .active_character_checkpoint()
            .unwrap()
            .inventory_items_json,
        after.inventory_items_json
    );
    let next = reconnect(&mut runtime);
    assert_eq!(
        next.inner
            .active_character_checkpoint()
            .unwrap()
            .inventory_items_json,
        after.inventory_items_json
    );
}

#[test]
fn ordinary_pickup_with_a_prepared_full_bag_restores_exact_original_custody_clocks() {
    let seed = 6; // Same retained source-rate roll verified by the item test.
    let mut runtime = fixture("CrossBagFull", None, Some(seed));
    let base = poison_and_depart(&mut runtime);
    let award = source_kill(&mut runtime, base);
    runtime.apply_pending_zone_packets();
    let item = award
        .drops
        .iter()
        .find(|d| {
            matches!(
                &d.loot,
                GroundDropLootSnapshot::InventoryItem {
                    exact_item: Some(_),
                    ..
                }
            )
        })
        .unwrap()
        .clone();
    walk(
        &mut runtime,
        MirDirection::Up,
        base + 3_000,
        SOURCE,
        Point { x: 30, y: 329 },
    );
    walk_to_drop(&mut runtime, &item, base);
    let source = ZoneKey::for_map(SOURCE);
    let (original, clocks, before_claim) = {
        let state = runtime.zone_state.lock().unwrap();
        let zone = state.zone_manager.zone(&source).unwrap();
        let checkpoint: serde_json::Value =
            serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
        (
            state
                .zone_manager
                .ground_drop_snapshot_for_key(&source, item.object_id)
                .unwrap(),
            state
                .zone_manager
                .ground_drop_absolute_clocks_for_key(&source, item.object_id)
                .unwrap(),
            checkpoint["next_ground_drop_claim_id"].as_u64().unwrap(),
        )
    };
    let proof = source_proof(&runtime);
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    let carrier: serde_json::Value = serde_json::from_str(&save.inventory_items_json[0]).unwrap();
    save.inventory_items_json = (0..save.inventory_capacity - 6)
        .map(|slot| {
            let mut item = carrier.clone();
            item["unique_id"] = json!(800_000 + u64::from(slot));
            item["slot"] = json!(slot % 40);
            item["container"] = json!(if slot < 40 { "bag1" } else { "bag2" });
            item["quantity"] = json!(1);
            item.to_string()
        })
        .collect();
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    let before = runtime.inner.active_character_checkpoint().unwrap();
    let rejected = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::PickUp))
        .unwrap();
    assert!(rejected
        .iter()
        .all(|p| !matches!(p, ServerPacket::GainedItem { .. })));
    assert_eq!(
        runtime
            .inner
            .active_character_checkpoint()
            .unwrap()
            .inventory_items_json,
        before.inventory_items_json
    );
    assert_eq!(source_proof(&runtime), proof);
    let (restored, after_clocks, after_claim) = {
        let state = runtime.zone_state.lock().unwrap();
        let zone = state.zone_manager.zone(&source).unwrap();
        let checkpoint: serde_json::Value =
            serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
        (
            state
                .zone_manager
                .ground_drop_snapshot_for_key(&source, item.object_id),
            state
                .zone_manager
                .ground_drop_absolute_clocks_for_key(&source, item.object_id),
            checkpoint["next_ground_drop_claim_id"].as_u64().unwrap(),
        )
    };
    assert!(
        after_claim > before_claim,
        "ordinary request actually reserved then canceled source custody"
    );
    assert_eq!(restored, Some(original));
    assert_eq!(after_clocks, Some(clocks));
}
