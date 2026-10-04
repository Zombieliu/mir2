//! Authenticated ordinary Poisoning -> native monster lethal hit -> periodic
//! poison kill -> durable Gateway reward. Isolated in-process account/map data;
//! not TCP/WSS, native-client, or human acceptance evidence.
use super::*;
use serde_json::json;

const TARGET: u32 = 391_001;
const KILLER: u32 = 391_002;
const EXP: u32 = 20;

fn fixture(name: &str, level_up: bool) -> SharedInProcessZoneSessionRuntime {
    fixture_with_config(name, level_up, None)
}

fn fixture_with_config(
    name: &str,
    level_up: bool,
    config: Option<mir2_simulation::SimulationConfig>,
) -> SharedInProcessZoneSessionRuntime {
    let state = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(state.clone());
    if let Some(config) = config {
        runtime.inner = InProcessWorldRuntime::new(config);
    }
    start_new_runtime_with_class(&mut runtime, name, name, MirClass::Taoist);
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.level = 14;
    save.position = Point { x: 340, y: 550 };
    save.hp = 1;
    save.mp = 100;
    save.max_mp = 100;
    save.max_experience = if level_up { 100 } else { 10_000 };
    save.experience = if level_up { 90 } else { 0 };
    save.bind_point = Some(mir2_simulation::CharacterBindPoint {
        map_file_name: "0".into(),
        position: Point { x: 334, y: 259 },
    });
    let powder = mir2_game_data::crystal_item_by_name("GreenPoison").unwrap();
    save.inventory_items_json = vec![json!({
        "key": format!("crystal-item-{}", powder.item_index), "name": powder.name,
        "icon": powder.image, "slot": 1, "unique_id": 71_001, "container": "bag1",
        "quantity": 3, "description": "isolated poison death-chain fixture",
        "weight": powder.weight, "equip_slot": "amulet", "attack": 0, "defence": 0,
        "heal_hp": 0, "heal_mp": 0, "user_item_metadata": {"item_index": powder.item_index}
    })
    .to_string()];
    save.skill_states_json = vec![json!({
        "key": "poisoning", "name": "Poisoning", "description": "isolated learned skill",
        "level": 3, "experience": 0, "cooldown_ticks": 1, "cooldown_ends_at": 0
    })
    .to_string()];
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    runtime
        .inner
        .force_authoritative_player_transform(save.position.clone(), MirDirection::Right);
    runtime.force_next_zone_transform_sync = true;
    runtime.sync_zone_snapshot();
    let sid = runtime.current_zone_session_id().unwrap();
    runtime.sync_authoritative_zone_combat_state(&sid).unwrap();
    let starting_pools = runtime.inner.world_snapshot();
    runtime.dispatch_zone_player_command(
        ZoneCommand::SyncPlayerVitals {
            session_id: sid.clone(),
            hp: 1,
            max_hp: starting_pools.player_max_hp.unwrap(),
            mp: starting_pools.player_mp.unwrap(),
        },
        false,
    );
    let mut target = shared_monster_entity(TARGET);
    target.name = "Skeleton".into();
    target.ai = Some(3);
    target.x = 342;
    target.y = 550;
    target.hp = Some(1);
    target.max_hp = Some(1);
    target.disposition = WorldEntityDisposition::Hostile;
    target.sprite = None;
    state.lock().unwrap().sync_map_layer(
        "0".into(),
        vec![target],
        BTreeSet::new(),
        Vec::new(),
        BTreeSet::new(),
    );
    spawn(&mut runtime, TARGET, Point { x: 342, y: 550 }, 1, EXP);
    runtime.apply_pending_zone_packets();
    runtime
}

fn spawn(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    id: u32,
    position: Point,
    hp: i32,
    experience: u32,
) {
    let sid = runtime.current_zone_session_id().unwrap();
    let mut state = runtime.zone_state.lock().unwrap();
    let out = state.zone_manager.handle(ZoneCommand::SpawnMonster {
        session_id: sid,
        now_ms: SharedInProcessZoneSessionRuntime::zone_now_ms(),
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: id,
            name: "Skeleton".into(),
            name_colour_argb: -1,
            image: 3,
            ai: 3,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 20,
            hp,
            max_hp: hp,
            experience,
            move_speed_ms: 100_000,
            attack_speed_ms: 100_000,
            friendly_guild: None,
            position,
            direction: MirDirection::Up,
            defense: Default::default(),
            respawn: None,
            drops: vec![],
        },
    });
    state.dispatch_zone_outbounds(out, None);
}

fn tick(runtime: &mut SharedInProcessZoneSessionRuntime, now_ms: u64) {
    super::super::run_shared_zone_cadence_tick(&runtime.zone_state, now_ms).unwrap();
}

/// Return the exact award chosen by production poison arbitration. The original
/// stays queued for the normal Gateway consumer (or the fenced teardown).
fn poison_after_real_death(
    runtime: &mut SharedInProcessZoneSessionRuntime,
) -> ZoneMonsterKillAward {
    poison_after_real_death_with_receipts(runtime).0
}

fn poison_after_real_death_with_receipts(
    runtime: &mut SharedInProcessZoneSessionRuntime,
) -> (ZoneMonsterKillAward, Vec<QueuedZoneVitalDelta>) {
    let owner = runtime.inner.world_snapshot().player_object_id.unwrap();
    let cast = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Magic {
            object_id: owner,
            spell: Spell::Poisoning,
            direction: MirDirection::Right,
            target_id: TARGET,
            location: Point { x: 342, y: 550 },
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
    assert!(cast.iter().any(|p| matches!(
        p,
        ServerPacket::DeleteItem {
            unique_id: 71_001,
            count: 1
        }
    )));
    let base = SharedInProcessZoneSessionRuntime::zone_now_ms();
    spawn(runtime, KILLER, Point { x: 340, y: 551 }, 10_000, 0);
    tick(runtime, base + 10);
    tick(runtime, base + 700);
    let key = runtime.current_presence_key().unwrap();
    let death_deltas = {
        let mut state = runtime.zone_state.lock().unwrap();
        let deltas = state.take_pending_zone_player_damages(&key);
        for delta in deltas.iter().cloned() {
            state.queue_zone_player_vital_delta(key.clone(), delta);
        }
        deltas
    };
    assert!(death_deltas.iter().any(|delta| matches!(delta, QueuedZoneVitalDelta::Settled { settlement, .. } if settlement.death_transition)));
    let death = runtime.apply_pending_zone_packets();
    assert!(
        death
            .iter()
            .any(|p| matches!(p, ServerPacket::Death { .. })),
        "actual native hit must kill before poison: {death:?}"
    );
    assert!(death.iter().any(|p| matches!(p, ServerPacket::DamageIndicator { object_id, damage, .. } if *object_id != TARGET && *damage > 0)), "{death:?}");
    assert_eq!(runtime.inner.world_snapshot().player_hp, Some(0));
    assert!(own_dead(runtime));
    tick(runtime, base + 2_100);
    let key = runtime.current_presence_key().unwrap();
    let awards = {
        let mut state = runtime.zone_state.lock().unwrap();
        let awards = state.take_pending_zone_monster_kill_awards(&key);
        state.prepend_zone_monster_kill_awards(key, awards.clone());
        awards
    };
    assert_eq!(
        awards.len(),
        1,
        "ordinary periodic poison must choose one dead solo owner"
    );
    assert_eq!(awards[0].monster_object_id, TARGET);
    assert_eq!(awards[0].monster_name, "Skeleton");
    assert_eq!(awards[0].experience, EXP);
    let award = awards[0].clone();
    (award, death_deltas)
}

fn own_dead(runtime: &SharedInProcessZoneSessionRuntime) -> bool {
    runtime
        .inner
        .world_snapshot()
        .entities
        .iter()
        .find(|e| e.kind == WorldEntityKind::SelfPlayer)
        .unwrap()
        .dead
}

fn assert_same_owner_life(runtime: &SharedInProcessZoneSessionRuntime, dead: bool) {
    let identity = runtime.inner.active_identity().unwrap();
    let key = runtime.current_presence_key().unwrap();
    let sid = runtime.current_zone_session_id().unwrap();
    let personal = runtime.inner.world_snapshot();
    let self_entity = personal
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    let (session, reverse_key, name, zone_life, zone_pools) = {
        let state = runtime.zone_state.lock().unwrap();
        (
            state.zone_sessions.get(&key).cloned(),
            state.zone_session_keys.get(&sid).cloned(),
            state.players.get(&key).unwrap().entity.name.clone(),
            state.zone_manager.player_is_dead(&sid),
            state.zone_manager.player_vitals(&sid),
        )
    };
    assert_eq!(key.account_id, identity.account_id);
    assert_eq!(key.character_index, identity.character_index);
    assert_eq!(session, Some(sid));
    assert_eq!(reverse_key, Some(key));
    assert_eq!(name, self_entity.name);
    assert_eq!(self_entity.dead, dead);
    assert_eq!(zone_life, Some(dead));
    assert_eq!(
        zone_pools,
        Some((
            personal.player_hp.unwrap(),
            personal.player_max_hp.unwrap(),
            personal.player_mp.unwrap()
        ))
    );
}

fn assert_award(packets: &[ServerPacket], level_up: bool) {
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::GainExperience { amount: EXP }))
            .count(),
        1,
        "{packets:?}"
    );
    assert_eq!(
        packets
            .iter()
            .filter(|p| matches!(p, ServerPacket::LevelChanged { .. }))
            .count(),
        usize::from(level_up),
        "{packets:?}"
    );
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectDied { info } if info.object_id == TARGET)),
        "{packets:?}"
    );
    assert!(!packets.iter().any(|p| matches!(
        p,
        ServerPacket::Revived | ServerPacket::ObjectRevived { .. }
    )));
}

#[test]
fn dead_poison_non_level_award_is_durable_and_does_not_revive() {
    let mut runtime = fixture("DeadNoLevel", false);
    poison_after_real_death(&mut runtime);
    let packets = runtime.apply_pending_zone_packets();
    assert_award(&packets, false);
    assert_eq!(
        runtime.inner.world_snapshot().player_experience,
        i64::from(EXP)
    );
    assert_eq!(runtime.inner.world_snapshot().player_hp, Some(0));
    assert!(own_dead(&runtime));
    assert_same_owner_life(&runtime, true);
    let config = runtime.inner.shared_mentor_config().unwrap();
    let identity = runtime.inner.active_identity().unwrap();
    let store = config.account_store.lock().unwrap();
    let saved = &store.accounts[&identity.account_id].saves[&identity.character_index];
    assert_eq!(saved.experience, i64::from(EXP));
    assert_eq!(
        saved.guild_experience_journal.applied_kill_receipts.len(),
        1
    );
}

#[test]
fn dead_poison_level_refills_pools_without_online_revive() {
    let mut runtime = fixture("DeadLevel", true);
    poison_after_real_death(&mut runtime);
    let sid = runtime.current_zone_session_id().unwrap();
    let generation = runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .player_life_generation(&sid)
        .unwrap();
    let packets = runtime.apply_pending_zone_packets();
    assert_award(&packets, true);
    let after = runtime.inner.world_snapshot();
    assert_eq!(
        after.player_hp, after.player_max_hp,
        "Crystal LevelUp refills HP even while Dead"
    );
    assert_eq!(after.player_mp, after.player_max_mp);
    assert_eq!(after.player_experience, 10);
    let after_generation = runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .player_life_generation(&sid);
    assert_eq!(
        after_generation,
        Some(generation),
        "level-up is not a revive transition"
    );
    assert!(
        own_dead(&runtime),
        "full HP must not clear the independent online Dead"
    );
    assert_same_owner_life(&runtime, true);
}

#[test]
fn dead_poison_level_rejects_actions_and_town_revive_still_works() {
    let mut runtime = fixture("DeadActions", true);
    poison_after_real_death(&mut runtime);
    runtime.apply_pending_zone_packets();
    let before = runtime.inner.world_snapshot();
    let sid = runtime.current_zone_session_id().unwrap();
    let position = runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .player_transform(&sid)
        .unwrap();
    for packet in [
        ClientPacket::Walk {
            direction: MirDirection::Left,
        },
        ClientPacket::Run {
            direction: MirDirection::Left,
        },
        ClientPacket::Attack {
            direction: MirDirection::Down,
            spell: Spell::None,
        },
        ClientPacket::Magic {
            object_id: before.player_object_id.unwrap(),
            spell: Spell::Poisoning,
            direction: MirDirection::Down,
            target_id: KILLER,
            location: Point { x: 340, y: 551 },
            spell_target_lock: true,
        },
    ] {
        let packets = runtime.execute(WorldCommand::ClientPacket(packet)).unwrap();
        assert!(!packets.iter().any(|p| matches!(p, ServerPacket::ObjectAttack { info } if info.object_id == before.player_object_id.unwrap()) || matches!(p, ServerPacket::Magic { cast: true, .. })), "dead owner admitted an action: {packets:?}");
    }
    tick(
        &mut runtime,
        SharedInProcessZoneSessionRuntime::zone_now_ms() + 4_000,
    );
    let after_position = runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .player_transform(&sid);
    assert_eq!(after_position, Some(position));
    let generation = runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .player_life_generation(&sid)
        .unwrap();
    let revived = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::TownRevive))
        .unwrap();
    assert_eq!(
        revived
            .iter()
            .filter(|p| matches!(p, ServerPacket::Revived))
            .count(),
        1,
        "full HP corpse must accept TownRevive: {revived:?}"
    );
    assert!(!own_dead(&runtime));
    let revived_generation = runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .player_life_generation(&sid);
    assert_eq!(revived_generation, Some(generation + 1));
    let repeated = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::TownRevive))
        .unwrap();
    assert!(!repeated.iter().any(|p| matches!(p, ServerPacket::Revived)));
    let repeated_generation = runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .player_life_generation(&sid);
    assert_eq!(repeated_generation, revived_generation);
}

#[test]
fn dead_poison_fenced_teardown_saves_latest_level_pools() {
    let mut runtime = fixture("DeadTeardown", true);
    poison_after_real_death(&mut runtime);
    let lease = ZoneOwnerLease::in_process(&ZoneId::new("test-shared-zone"));
    let prepared = runtime
        .prepare_teardown_checkpoint(&lease)
        .unwrap()
        .unwrap();
    let saved = prepared.checkpoint();
    assert_eq!(saved.character.level, 15);
    assert_eq!(saved.experience, 10);
    assert_eq!(
        saved.hp, saved.max_hp,
        "teardown must not overwrite the committed refill with pre-award HP0"
    );
    assert_eq!(saved.mp, saved.max_mp);
    assert!(
        own_dead(&runtime),
        "teardown drain must retain online life until actual removal"
    );
}

#[test]
fn dead_poison_known_save_failure_preserves_life_then_retry_and_duplicate() {
    let mut runtime = fixture("DeadRetry", true);
    let award = poison_after_real_death(&mut runtime);
    let config = runtime.inner.shared_mentor_config().unwrap();
    config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::BeforePersist,
    );
    let failed = runtime.apply_pending_zone_packets();
    assert!(!failed.iter().any(|p| matches!(
        p,
        ServerPacket::GainExperience { .. }
            | ServerPacket::LevelChanged { .. }
            | ServerPacket::Revived
    )));
    assert_eq!(runtime.inner.world_snapshot().player_experience, 90);
    assert_eq!(runtime.inner.world_snapshot().player_hp, Some(0));
    assert!(own_dead(&runtime));
    let retry = runtime.apply_pending_zone_packets();
    assert_eq!(
        retry
            .iter()
            .filter(|p| matches!(p, ServerPacket::GainExperience { amount: EXP }))
            .count(),
        1
    );
    assert!(own_dead(&runtime));
    let identity = runtime.inner.active_identity().unwrap();
    let key = config.account_store.lock().unwrap().accounts[&identity.account_id].saves
        [&identity.character_index]
        .guild_experience_journal
        .applied_kill_receipts
        .keys()
        .next()
        .unwrap()
        .clone();
    let mut duplicate = award;
    duplicate.source_receipt_key = Some(key);
    runtime.account_inventory_service = Arc::new(InProcessAccountInventoryService::new());
    assert!(runtime
        .apply_zone_monster_kill_awards(vec![duplicate.clone()])
        .is_empty());
    assert_eq!(runtime.inner.world_snapshot().player_experience, 10);
    assert!(own_dead(&runtime));
    duplicate.experience += 1;
    assert!(
        runtime
            .apply_zone_monster_kill_awards(vec![duplicate])
            .is_empty(),
        "same durable key with altered payload must reject"
    );
    assert_eq!(runtime.inner.world_snapshot().player_experience, 10);
    assert!(own_dead(&runtime));
}

#[test]
fn dead_poison_retained_bootstrap_preserves_online_life() {
    let mut runtime = fixture("DeadResume", true);
    poison_after_real_death(&mut runtime);
    runtime.apply_pending_zone_packets();
    let character_index = runtime.inner.active_identity().unwrap().character_index;
    runtime
        .inner
        .execute(WorldCommand::ReplayRetainedStartGameBootstrap { character_index })
        .unwrap();
    assert!(
        own_dead(&runtime),
        "retained authenticated runtime is not a fresh PlayerObject"
    );
}

fn learned_magic_fixture(name: &str, spell: Spell) -> SharedInProcessZoneSessionRuntime {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    let class = if spell == Spell::FireBall {
        MirClass::Wizard
    } else {
        MirClass::Taoist
    };
    start_new_runtime_with_class(&mut runtime, name, name, class);
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.level = 20;
    save.position = Point { x: 340, y: 550 };
    save.hp = 1;
    save.mp = 100;
    save.max_mp = 100;
    let (key, display) = match spell {
        Spell::FireBall => ("fireball", "FireBall"),
        Spell::Healing => ("healing", "Healing"),
        Spell::Poisoning => ("poisoning", "Poisoning"),
        _ => unreachable!(),
    };
    save.skill_states_json = vec![json!({
        "key": key, "name": display, "description": "isolated valid dead-Magic admission fence",
        "level": 0, "experience": 0, "cooldown_ticks": 0, "cooldown_ends_at": 0
    })
    .to_string()];
    if spell == Spell::Poisoning {
        let powder = mir2_game_data::crystal_item_by_name("GreenPoison").unwrap();
        save.inventory_items_json = vec![json!({
            "key": format!("crystal-item-{}", powder.item_index), "name": powder.name,
            "icon": powder.image, "slot": 1, "unique_id": 71_001, "container": "bag1",
            "quantity": 3, "description": "isolated native admission fixture", "weight": powder.weight,
            "equip_slot": "amulet", "attack": 0, "defence": 0, "heal_hp": 0, "heal_mp": 0,
            "user_item_metadata": {"item_index": powder.item_index}
        }).to_string()];
    }
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    runtime.force_next_zone_transform_sync = true;
    runtime.sync_zone_snapshot();
    let sid = runtime.current_zone_session_id().unwrap();
    runtime.sync_authoritative_zone_combat_state(&sid).unwrap();
    let starting_pools = runtime.inner.world_snapshot();
    runtime.dispatch_zone_player_command(
        ZoneCommand::SyncPlayerVitals {
            session_id: sid,
            hp: 1,
            max_hp: starting_pools.player_max_hp.unwrap(),
            mp: starting_pools.player_mp.unwrap(),
        },
        false,
    );
    let mut target = shared_monster_entity(TARGET);
    target.name = "Skeleton".into();
    target.ai = Some(3);
    target.x = 342;
    target.y = 550;
    target.hp = Some(100);
    target.max_hp = Some(100);
    target.disposition = WorldEntityDisposition::Hostile;
    target.sprite = None;
    shared.lock().unwrap().sync_map_layer(
        "0".into(),
        vec![target],
        BTreeSet::new(),
        Vec::new(),
        BTreeSet::new(),
    );
    spawn(&mut runtime, TARGET, Point { x: 342, y: 550 }, 100, 0);
    runtime.apply_pending_zone_packets();
    runtime
}

fn native_hp(runtime: &SharedInProcessZoneSessionRuntime, id: u32) -> i32 {
    runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .native_monster_snapshots(&ZoneKey::for_map("0"))
        .into_iter()
        .find(|m| m.object_id == id)
        .unwrap()
        .hp
}

fn ordinary_magic(runtime: &SharedInProcessZoneSessionRuntime, spell: Spell) -> WorldCommand {
    let own = runtime.inner.world_snapshot().player_object_id.unwrap();
    WorldCommand::ClientPacket(ClientPacket::Magic {
        object_id: own,
        spell,
        direction: MirDirection::Up,
        target_id: if spell == Spell::Healing { own } else { TARGET },
        location: if spell == Spell::Healing {
            Point { x: 340, y: 550 }
        } else {
            Point { x: 342, y: 550 }
        },
        spell_target_lock: true,
    })
}

#[test]
fn native_zero_hp_death_rejects_valid_magic_without_spend_hit_or_turn() {
    for (spell, alive_name, dead_name) in [
        (Spell::FireBall, "AliveFire", "DeadFire"),
        (Spell::Healing, "AliveHeal", "DeadHeal"),
        (Spell::Poisoning, "AlivePoison", "DeadPoison"),
    ] {
        // Positive control has no prior spell/action deadline. Prove this exact
        // learned packet reaches native execution and really spends MP/material.
        let mut alive = learned_magic_fixture(alive_name, spell);
        let starting_mp = alive.inner.world_snapshot().player_mp.unwrap();
        let alive_cast = alive.execute(ordinary_magic(&alive, spell)).unwrap();
        assert!(
            alive_cast
                .iter()
                .any(|p| matches!(p, ServerPacket::Magic { cast: true, .. })),
            "{spell:?}: {alive_cast:?}"
        );
        assert!(
            alive.inner.world_snapshot().player_mp.unwrap() < starting_mp,
            "{spell:?} control must spend real MP"
        );
        if spell == Spell::Poisoning {
            assert!(alive_cast.iter().any(|p| matches!(
                p,
                ServerPacket::DeleteItem {
                    unique_id: 71_001,
                    count: 1
                }
            )));
        }
        let mut dead = learned_magic_fixture(dead_name, spell);
        let base = SharedInProcessZoneSessionRuntime::zone_now_ms();
        spawn(&mut dead, KILLER, Point { x: 340, y: 551 }, 10_000, 0);
        tick(&mut dead, base + 10);
        tick(&mut dead, base + 700);
        let death = dead.apply_pending_zone_packets();
        assert!(
            death
                .iter()
                .any(|p| matches!(p, ServerPacket::Death { .. })),
            "{spell:?}: real native death required {death:?}"
        );
        assert_eq!(dead.inner.world_snapshot().player_hp, Some(0));
        let sid = dead.current_zone_session_id().unwrap();
        let before = dead.inner.active_character_checkpoint().unwrap();
        let before_transform = dead
            .zone_state
            .lock()
            .unwrap()
            .zone_manager
            .player_transform(&sid)
            .unwrap();
        let target_hp = native_hp(&dead, TARGET);
        let rejected = dead.execute(ordinary_magic(&dead, spell)).unwrap();
        tick(&mut dead, base + 2_100);
        let after = dead.inner.active_character_checkpoint().unwrap();
        assert_eq!(after.mp, before.mp, "{spell:?} dead cast must not spend MP");
        assert_eq!(
            after.inventory_items_json, before.inventory_items_json,
            "{spell:?} dead cast must not debit powder"
        );
        assert_eq!(
            native_hp(&dead, TARGET),
            target_hp,
            "{spell:?} dead cast must not cause a delayed target hit"
        );
        let after_transform = dead
            .zone_state
            .lock()
            .unwrap()
            .zone_manager
            .player_transform(&sid);
        assert_eq!(
            after_transform,
            Some(before_transform.clone()),
            "{spell:?} dead Magic must not turn"
        );
        assert!(!rejected.iter().any(|p| matches!(
            p,
            ServerPacket::Magic { cast: true, .. }
                | ServerPacket::ObjectMagic { cast: true, .. }
                | ServerPacket::DeleteItem { .. }
        )));
        let turned = dead
            .execute(WorldCommand::ClientPacket(ClientPacket::Turn {
                direction: MirDirection::Left,
            }))
            .unwrap();
        tick(&mut dead, base + 3_100);
        let after_turn = dead
            .zone_state
            .lock()
            .unwrap()
            .zone_manager
            .player_transform(&sid);
        assert_eq!(
            after_turn,
            Some(before_transform),
            "explicit dead Turn must not change facing: {turned:?}"
        );
        assert!(own_dead(&dead));
    }
}

#[test]
fn acknowledged_private_death_also_rejects_native_magic_before_zone_mirror_catches_up() {
    // Isolated split-authority fixture corresponding to the source admission
    // edge observed by the separately retained R17 ordinary network audit.
    let mut runtime = learned_magic_fixture("PrivateDead", Spell::FireBall);
    runtime
        .inner
        .force_authoritative_player_vitals(Some(0), None);
    let local = runtime.inner.world_snapshot().player_object_id.unwrap();
    runtime.owner_dead_entity_ids.insert(local);
    let sid = runtime.current_zone_session_id().unwrap();
    let zone_before = runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .player_vitals(&sid)
        .unwrap();
    assert!(zone_before.0 > 0);
    let before = runtime.inner.active_character_checkpoint().unwrap();
    let before_direction = before.direction;
    let target_hp = native_hp(&runtime, TARGET);
    let packets = runtime
        .execute(ordinary_magic(&runtime, Spell::FireBall))
        .unwrap();
    tick(
        &mut runtime,
        SharedInProcessZoneSessionRuntime::zone_now_ms() + 1_000,
    );
    let after = runtime.inner.active_character_checkpoint().unwrap();
    assert_eq!(
        after.mp, before.mp,
        "positive old Zone HP cannot authorize a dead private caster"
    );
    assert_eq!(after.direction, before_direction);
    assert_eq!(native_hp(&runtime, TARGET), target_hp);
    assert!(!packets
        .iter()
        .any(|p| matches!(p, ServerPacket::Magic { cast: true, .. })));
    assert!(own_dead(&runtime));
}

#[test]
fn real_poison_logout_relogin_distinguishes_zero_hp_from_full_hp_dead_save() {
    for (name, level_up) in [("DeadLoginZero", false), ("DeadLoginFull", true)] {
        let mut runtime = fixture(name, level_up);
        let mut award = poison_after_real_death(&mut runtime);
        runtime.apply_pending_zone_packets();
        let config = runtime.inner.shared_mentor_config().unwrap();
        let before = runtime.inner.active_character_checkpoint().unwrap();
        assert!(own_dead(&runtime));
        let logout = runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
            .unwrap();
        assert!(logout
            .iter()
            .any(|p| matches!(p, ServerPacket::LogOutSuccess { .. })));
        let mut next =
            shared_session_runtime(Arc::new(Mutex::new(SharedInProcessZoneState::new())));
        next.inner = InProcessWorldRuntime::new(config.clone());
        next.execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: name.into(),
            password: name.into(),
        }))
        .unwrap();
        next.execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: before.character.index,
        }))
        .unwrap();
        let after = next.inner.active_character_checkpoint().unwrap();
        assert!(
            !own_dead(&next),
            "a genuinely new PlayerObject starts alive"
        );
        assert_eq!(after.hp, after.max_hp);
        assert_eq!(after.character.level, before.character.level);
        assert_eq!(after.experience, before.experience);
        assert_eq!(
            after.position,
            if level_up {
                before.position
            } else {
                before.bind_point.unwrap().position
            }
        );
        assert_eq!(
            after.guild_experience_journal.applied_kill_receipts.len(),
            1
        );
        award.source_receipt_key = after
            .guild_experience_journal
            .applied_kill_receipts
            .keys()
            .next()
            .cloned();
        assert!(next.apply_zone_monster_kill_awards(vec![award]).is_empty());
        assert_eq!(
            next.inner.world_snapshot().player_experience,
            after.experience
        );
        assert!(!own_dead(&next));
    }
}

#[test]
fn genuine_dead_receipts_replayed_after_level_and_revive_do_not_repeat_penalty() {
    let mut runtime = fixture("DeadReceipt", true);
    let (_, deltas) = poison_after_real_death_with_receipts(&mut runtime);
    runtime.apply_pending_zone_packets();
    let before = runtime.inner.active_character_checkpoint().unwrap();
    let tick_before = runtime.inner.world_snapshot().tick;
    let duplicate = runtime.apply_zone_player_damages(deltas.clone());
    let after = runtime.inner.active_character_checkpoint().unwrap();
    assert_eq!(after.hp, before.hp);
    assert_eq!(after.inventory_items_json, before.inventory_items_json);
    assert_eq!(after.equipment_items_json, before.equipment_items_json);
    assert_eq!(runtime.inner.world_snapshot().tick, tick_before);
    assert!(duplicate.is_empty());
    assert!(own_dead(&runtime));
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::TownRevive))
        .unwrap();
    assert!(!own_dead(&runtime));
    let before = runtime.inner.active_character_checkpoint().unwrap();
    let duplicate = runtime.apply_zone_player_damages(deltas);
    let after = runtime.inner.active_character_checkpoint().unwrap();
    assert_eq!(after.hp, before.hp);
    assert_eq!(after.inventory_items_json, before.inventory_items_json);
    assert_eq!(after.equipment_items_json, before.equipment_items_json);
    assert!(duplicate.is_empty());
    assert!(!own_dead(&runtime));
}

#[test]
fn unknown_poison_award_publication_freezes_without_fake_success_or_revive() {
    let root = std::env::temp_dir().join(format!(
        "mir2-dead-source-unknown-{}-{}",
        std::process::id(),
        SharedInProcessZoneSessionRuntime::zone_now_ms()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let config = mir2_simulation::SimulationConfig::default()
        .with_account_store_path(root.join("accounts.json"));
    config.save_account_store().unwrap();
    let mut runtime = fixture_with_config("DeadUnknown", true, Some(config.clone()));
    let actual_award = poison_after_real_death(&mut runtime);
    config.inject_account_store_transaction_fault(
        mir2_simulation::AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    let packets = runtime.apply_pending_zone_packets();
    assert!(!packets.iter().any(|p| matches!(
        p,
        ServerPacket::GainExperience { .. }
            | ServerPacket::LevelChanged { .. }
            | ServerPacket::Revived
            | ServerPacket::ObjectRevived { .. }
    )));
    assert!(
        config.save_account_store().is_err(),
        "an unknown publication must retain the existing repository freeze"
    );
    assert!(own_dead(&runtime));
    let key = runtime.current_presence_key().unwrap();
    let pending = runtime
        .zone_state
        .lock()
        .unwrap()
        .take_pending_zone_monster_kill_awards(&key);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].monster_object_id, actual_award.monster_object_id);
    assert_eq!(pending[0].experience, actual_award.experience);
    assert!(pending[0].source_receipt_key.is_some());
    let rejected = runtime.apply_zone_monster_kill_awards(pending);
    assert!(rejected.is_empty());
    assert!(own_dead(&runtime));
    let lease = ZoneOwnerLease::in_process(&ZoneId::new("test-shared-zone"));
    assert!(
        runtime.prepare_teardown_checkpoint(&lease).is_err(),
        "unconfirmed reward publication must not become a successful teardown"
    );
}

fn carried_item(name: &str, uid: u64, slot: u8) -> String {
    let template = mir2_game_data::crystal_item_by_name(name).unwrap();
    json!({
        "key": format!("crystal-item-{}", template.item_index), "name": template.name,
        "icon": template.image, "slot": slot, "unique_id": uid, "container": "bag1",
        "quantity": 1, "description": "isolated full-HP corpse item fence", "weight": template.weight,
        "attack": 0, "defence": 0, "heal_hp": 0, "heal_mp": 0,
        "user_item_metadata": {"item_index": template.item_index}
    }).to_string()
}

#[test]
fn full_hp_poison_level_corpse_blocks_normal_potion_but_scroll_explicitly_revives() {
    let mut runtime = fixture("DeadScroll", true);
    poison_after_real_death(&mut runtime);
    runtime.apply_pending_zone_packets();
    assert!(own_dead(&runtime));
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.inventory_items_json
        .push(carried_item("(HP)DrugSmall", 71_002, 2));
    save.inventory_items_json
        .push(carried_item("ResurrectionScroll", 71_003, 3));
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    let denied = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id: 71_002,
            grid: MirGridType::Inventory,
        }))
        .unwrap();
    assert!(
        !denied.iter().any(|p| matches!(
            p,
            ServerPacket::DeleteItem { .. } | ServerPacket::UseItem { success: true, .. }
        )),
        "{denied:?}"
    );
    assert!(runtime
        .inner
        .world_snapshot()
        .inventory_items
        .iter()
        .any(|item| item.unique_id == 71_002));
    assert!(own_dead(&runtime));
    let sid = runtime.current_zone_session_id().unwrap();
    let generation = runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .player_life_generation(&sid)
        .unwrap();
    let revived = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id: 71_003,
            grid: MirGridType::Inventory,
        }))
        .unwrap();
    assert!(
        revived.iter().any(|p| matches!(
            p,
            ServerPacket::UseItem {
                unique_id: 71_003,
                success: true,
                ..
            }
        )),
        "{revived:?}"
    );
    assert!(revived
        .iter()
        .any(|p| matches!(p, ServerPacket::ObjectRevived { .. })));
    assert!(!runtime
        .inner
        .world_snapshot()
        .inventory_items
        .iter()
        .any(|item| item.unique_id == 71_003));
    assert!(!own_dead(&runtime));
    let after_generation = runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .player_life_generation(&sid);
    assert_eq!(after_generation, Some(generation + 1));
    let zone_dead = runtime
        .zone_state
        .lock()
        .unwrap()
        .zone_manager
        .player_is_dead(&sid);
    assert_eq!(zone_dead, Some(false));
}
