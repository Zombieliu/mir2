//! Trusted five-class outcomes, exercised through authenticated profile sessions
//! and the actual shared Zone producer. No client payload is treated as a receipt.
use super::*;
use mir2_simulation::{
    ZoneFiveClassPracticeReceipt, ZoneFiveClassPracticeSpell, ZoneManaSettlement, ZoneRuntime,
};

type SharedState = Arc<Mutex<SharedInProcessZoneState>>;

fn learn_source_skills(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    skills: &[(ZoneFiveClassPracticeSpell, u8)],
) {
    // Trusted save restoration represents books already learnt by this account;
    // both learned projection and cast costs still come from the real profile.
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.level = 50;
    save.skill_states_json = skills
        .iter()
        .map(|(spell, level)| {
            serde_json::json!({
                "key": spell.skill_key(), "name": format!("{:?}", spell.spell()),
                "description": "", "level": level, "experience": 0,
                "cooldown_ticks": 0, "cooldown_ends_at": 0
            })
            .to_string()
        })
        .collect();
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    let vitals = runtime.inner.local_player_vitals_snapshot();
    let max_mp = runtime.inner.zone_player_combat_stats().max_mp;
    assert!(max_mp > 0, "fixture must retain Source class-derived mana");
    runtime
        .inner
        .force_authoritative_player_vitals(vitals.player_max_hp, Some(max_mp));
    runtime.sync_zone_snapshot();
    let learned = runtime
        .inner
        .zone_player_combat_stats()
        .five_class_learned_mask;
    for (spell, _) in skills {
        assert_ne!(
            learned & spell.learned_bit(),
            0,
            "missing Source skill {spell:?}"
        );
        assert!(runtime
            .inner
            .zone_magic_attack_profile(spell.spell())
            .is_some());
    }
}

fn fixture(
    name: &str,
    class: MirClass,
    skills: &[(ZoneFiveClassPracticeSpell, u8)],
) -> (SharedState, SharedInProcessZoneSessionRuntime) {
    let mut state = SharedInProcessZoneState::new();
    state.experience_authority = Some(GatewayConfig::default().with_platinum_fiveclass_profile());
    let shared = Arc::new(Mutex::new(state));
    let mut owner = shared_session_runtime(shared.clone());
    start_new_runtime_with_class(&mut owner, name, name, class);
    learn_source_skills(&mut owner, skills);

    let key = owner.current_presence_key().unwrap();
    let session = owner.current_zone_session_id().unwrap();
    let mut join = owner
        .inner
        .active_zone_join_snapshot(session.as_str())
        .unwrap();
    let commands = owner.authoritative_zone_combat_commands(&session).unwrap();
    let mut state = shared.lock().unwrap();
    join.object_id = state.players[&key].zone_object_id;
    // A real empty shared Zone isolates outcome ordering from unrelated map AI.
    // This fixture does not claim terrain, NPC or natural-map acceptance.
    state.zone_manager = ZoneManager::new();
    assert!(state
        .zone_manager
        .install_empty_zone(ZoneRuntime::new_with_collision(
            ZoneKey::for_map(&join.map_file_name),
            ZoneCollision::unbounded(),
        )));
    state.zone_manager.handle(ZoneCommand::Join(join));
    for command in commands {
        state.zone_manager.handle(command);
    }
    assert_eq!(state.zone_manager.player_life_generation(&session), Some(0));
    drop(state);
    (shared, owner)
}

fn skill_from_values(values: &[String], spell: ZoneFiveClassPracticeSpell) -> Option<(u8, u16)> {
    values
        .iter()
        .map(|value| serde_json::from_str::<serde_json::Value>(value).unwrap())
        .find(|value| value["key"] == spell.skill_key())
        .map(|value| {
            (
                value["level"].as_u64().unwrap() as u8,
                value["experience"].as_u64().unwrap() as u16,
            )
        })
}

fn skill(
    runtime: &SharedInProcessZoneSessionRuntime,
    spell: ZoneFiveClassPracticeSpell,
) -> (u8, u16) {
    skill_from_values(
        &runtime
            .inner
            .active_character_checkpoint()
            .unwrap()
            .skill_states_json,
        spell,
    )
    .expect("fixture must have this learned skill")
}

fn source_cast_outcomes(
    runtime: &SharedInProcessZoneSessionRuntime,
    spell: ZoneFiveClassPracticeSpell,
    now: u64,
) -> Vec<ZoneOutbound> {
    let (level, damage, mp_cost, cooldown_ms) = runtime
        .inner
        .zone_magic_attack_profile(spell.spell())
        .unwrap();
    let session_id = runtime.current_zone_session_id().unwrap();
    let mut state = runtime.zone_state.lock().unwrap();
    let (position, direction) = state.zone_manager.player_transform(&session_id).unwrap();
    let mut outcomes = state.zone_manager.handle(ZoneCommand::PlayerCastMagic {
        session_id,
        object_id: 0,
        spell: spell.spell(),
        direction,
        target: position,
        cast: true,
        level,
        damage,
        mp_cost,
        cooldown_ms,
        now_ms: now,
    });
    outcomes.extend(state.zone_manager.tick_all(now + 500));
    outcomes
}

fn source_practice(
    runtime: &SharedInProcessZoneSessionRuntime,
    spell: ZoneFiveClassPracticeSpell,
    now: u64,
) -> ZoneFiveClassPracticeReceipt {
    let outcomes = source_cast_outcomes(runtime, spell, now);
    let receipts: Vec<_> = outcomes
        .into_iter()
        .filter_map(|outbound| match outbound {
            ZoneOutbound::FiveClassMagicPractice { receipt } if receipt.spell == spell => {
                Some(receipt)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        receipts.len(),
        1,
        "actual Source cast must issue exactly one {spell:?} receipt"
    );
    let receipt = receipts.into_iter().next().unwrap();
    assert!(receipt.sequence > 0);
    assert!(runtime
        .zone_state
        .lock()
        .unwrap()
        .five_class_practice_owner_is_current(&runtime.current_presence_key().unwrap(), &receipt));
    receipt
}

fn queue_practice(
    runtime: &SharedInProcessZoneSessionRuntime,
    receipts: Vec<ZoneFiveClassPracticeReceipt>,
) {
    runtime.zone_state.lock().unwrap().dispatch_zone_outbounds(
        receipts
            .into_iter()
            .map(|receipt| ZoneOutbound::FiveClassMagicPractice { receipt })
            .collect(),
        None,
    );
}

fn practice_packets(packets: &[ServerPacket], spell: ZoneFiveClassPracticeSpell) -> usize {
    packets
        .iter()
        .filter(|packet| {
            matches!(packet,
        ServerPacket::MagicLeveled { spell: actual, .. } if *actual == spell.spell())
        })
        .count()
}

#[test]
fn five_class_receipt_real_assassin_and_archer_practice_is_owner_only_and_replay_safe() {
    for (name, spell) in [
        ("FcHaste", ZoneFiveClassPracticeSpell::Haste),
        ("FcConcentration", ZoneFiveClassPracticeSpell::Concentration),
    ] {
        let (shared, mut owner) = fixture(name, spell.class(), &[(spell, 0)]);
        let mut observer = shared_session_runtime(shared);
        let observer_name = format!("{name}O");
        start_new_runtime_with_class(&mut observer, &observer_name, &observer_name, spell.class());
        let receipt = source_practice(
            &owner,
            spell,
            SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000,
        );
        queue_practice(&owner, vec![receipt.clone(), receipt.clone()]);
        assert_eq!(
            practice_packets(&observer.apply_pending_zone_packets(), spell),
            0
        );
        assert_eq!(skill(&owner, spell), (0, 0));
        let packets = owner.apply_pending_zone_packets();
        assert_eq!(practice_packets(&packets, spell), 1);
        let progressed = skill(&owner, spell);
        assert!((1..=3).contains(&progressed.1));
        queue_practice(&owner, vec![receipt]);
        assert_eq!(
            practice_packets(&owner.apply_pending_zone_packets(), spell),
            0
        );
        assert_eq!(skill(&owner, spell), progressed);
    }
}

#[test]
fn five_class_receipt_invalid_identity_cannot_poison_valid_sequence() {
    let spell = ZoneFiveClassPracticeSpell::Haste;
    let (_, mut owner) = fixture("FcIdentity", MirClass::Assassin, &[(spell, 0)]);
    let receipt = source_practice(
        &owner,
        spell,
        SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000,
    );
    // Only negative cases alter a copy of an actually produced internal result.
    let mut invalid = vec![receipt.clone(); 8];
    invalid[0].account_id.push_str("-other");
    invalid[1].character_index += 1;
    invalid[2].object_id = 0;
    invalid[3].session_id = SessionId::new("not-this-authenticated-session");
    invalid[4].zone_key = ZoneKey::for_map("not-this-map");
    invalid[5].life_generation += 1;
    invalid[6].sequence = 0;
    invalid[7].spell = ZoneFiveClassPracticeSpell::Concentration;
    for mut forged in invalid {
        if forged.sequence > 0 {
            forged.sequence = u64::MAX;
        }
        assert!(owner.apply_five_class_practice_receipt(forged).is_empty());
        assert_eq!(skill(&owner, spell), (0, 0));
    }
    assert_eq!(
        practice_packets(&owner.apply_five_class_practice_receipt(receipt), spell),
        1
    );
    assert!((1..=3).contains(&skill(&owner, spell).1));
}

#[test]
fn five_class_receipt_unlearned_skill_rejected_without_consuming_server_receipt() {
    let spell = ZoneFiveClassPracticeSpell::Haste;
    let (_, mut owner) = fixture("FcUnlearned", MirClass::Assassin, &[(spell, 0)]);
    let receipt = source_practice(
        &owner,
        spell,
        SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000,
    );
    let learned_save = owner.inner.active_character_checkpoint().unwrap();
    let mut absent = learned_save.clone();
    absent.skill_states_json.clear();
    owner
        .inner
        .restore_active_character_checkpoint(&absent)
        .unwrap();
    owner.sync_zone_snapshot();
    assert_eq!(
        owner
            .inner
            .zone_player_combat_stats()
            .five_class_learned_mask
            & spell.learned_bit(),
        0
    );
    assert!(owner
        .apply_five_class_practice_receipt(receipt.clone())
        .is_empty());
    assert!(skill_from_values(
        &owner
            .inner
            .active_character_checkpoint()
            .unwrap()
            .skill_states_json,
        spell
    )
    .is_none());
    owner
        .inner
        .restore_active_character_checkpoint(&learned_save)
        .unwrap();
    owner.sync_zone_snapshot();
    assert_eq!(
        practice_packets(&owner.apply_five_class_practice_receipt(receipt), spell),
        1
    );
}

#[test]
fn five_class_receipt_out_of_order_earlier_outcome_cannot_advance_another_skill() {
    let haste = ZoneFiveClassPracticeSpell::Haste;
    let light = ZoneFiveClassPracticeSpell::LightBody;
    let (_, mut owner) = fixture("FcSequence", MirClass::Assassin, &[(haste, 0), (light, 0)]);
    let now = SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000;
    let earlier = source_practice(&owner, haste, now);
    let later = source_practice(&owner, light, now + 3_000);
    assert!(later.sequence > earlier.sequence);
    queue_practice(&owner, vec![later.clone(), earlier, later]);
    let packets = owner.apply_pending_zone_packets();
    assert_eq!(practice_packets(&packets, light), 1);
    assert_eq!(practice_packets(&packets, haste), 0);
    assert_eq!(skill(&owner, haste), (0, 0));
    assert!((1..=3).contains(&skill(&owner, light).1));
}

#[test]
fn five_class_receipt_old_life_is_rejected_after_real_zone_death_and_revive() {
    let haste = ZoneFiveClassPracticeSpell::Haste;
    let light = ZoneFiveClassPracticeSpell::LightBody;
    let (_, mut owner) = fixture("FcLife", MirClass::Assassin, &[(haste, 0), (light, 0)]);
    let now = SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000;
    let old = source_practice(&owner, haste, now);
    queue_practice(&owner, vec![old.clone()]);
    {
        let mut state = owner.zone_state.lock().unwrap();
        let (_, max_hp, mp) = state.zone_manager.player_vitals(&old.session_id).unwrap();
        state
            .zone_manager
            .handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: old.session_id.clone(),
                hp: 0,
                max_hp,
                mp,
                dead: true,
            });
        state
            .zone_manager
            .handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: old.session_id.clone(),
                hp: max_hp,
                max_hp,
                mp,
                dead: false,
            });
        assert_eq!(
            state.zone_manager.player_life_generation(&old.session_id),
            Some(old.life_generation + 1)
        );
    }
    assert_eq!(
        practice_packets(&owner.apply_pending_zone_packets(), haste),
        0
    );
    assert_eq!(skill(&owner, haste), (0, 0));
    let fresh = source_practice(&owner, light, now + 3_000);
    assert!(fresh.life_generation > old.life_generation);
    assert_eq!(
        practice_packets(&owner.apply_five_class_practice_receipt(fresh), light),
        1
    );
}

#[test]
fn five_class_receipt_actual_logout_startgame_retires_old_online_object() {
    let haste = ZoneFiveClassPracticeSpell::Haste;
    let light = ZoneFiveClassPracticeSpell::LightBody;
    let (_, mut owner) = fixture("FcReconnect", MirClass::Assassin, &[(haste, 0), (light, 0)]);
    let old = source_practice(
        &owner,
        haste,
        SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000,
    );
    let character_index = owner.inner.active_identity().unwrap().character_index;
    owner
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    owner
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index,
        }))
        .unwrap();
    let key = owner.current_presence_key().unwrap();
    assert_ne!(
        owner.zone_state.lock().unwrap().players[&key].zone_object_id,
        old.object_id
    );
    assert!(owner.apply_five_class_practice_receipt(old).is_empty());
    assert_eq!(skill(&owner, haste), (0, 0));
    let fresh = source_practice(
        &owner,
        light,
        SharedInProcessZoneSessionRuntime::zone_now_ms() + 4_000,
    );
    assert_eq!(
        practice_packets(&owner.apply_five_class_practice_receipt(fresh), light),
        1
    );
}

#[test]
fn five_class_receipt_teardown_saves_prior_queue_and_rejects_late_arrival() {
    let haste = ZoneFiveClassPracticeSpell::Haste;
    let light = ZoneFiveClassPracticeSpell::LightBody;
    let (_, mut owner) = fixture("FcTeardown", MirClass::Assassin, &[(haste, 0), (light, 0)]);
    let now = SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000;
    let prior = source_practice(&owner, haste, now);
    let late = source_practice(&owner, light, now + 3_000);
    queue_practice(&owner, vec![prior]);
    let lease = ZoneOwnerLease::in_process(&ZoneId::new("test-shared-zone"));
    let prepared = owner.prepare_teardown_checkpoint(&lease).unwrap().unwrap();
    let saved = skill_from_values(&prepared.checkpoint().skill_states_json, haste).unwrap();
    assert!(
        (1..=3).contains(&saved.1),
        "resolved, queued practice must survive the save fence"
    );
    queue_practice(&owner, vec![late.clone()]);
    assert!(owner.apply_five_class_practice_receipt(late).is_empty());
    assert_eq!(
        practice_packets(&owner.apply_pending_zone_packets(), light),
        0
    );
    assert_eq!(skill(&owner, light), (0, 0));
}

fn source_mp_eater(
    owner: &mut SharedInProcessZoneSessionRuntime,
    now: u64,
) -> (ZoneManaSettlement, ZoneFiveClassPracticeReceipt) {
    let session = owner.current_zone_session_id().unwrap();
    let key = owner.current_presence_key().unwrap();
    let (attack_spell, level, damage) = owner.inner.zone_melee_attack_profile(Spell::None);
    let commands = owner.authoritative_zone_combat_commands(&session).unwrap();
    let projected = owner.inner.zone_player_combat_stats();
    assert_eq!(projected.mp_eater_level, Some(3));
    assert_ne!(
        projected.five_class_learned_mask & ZoneFiveClassPracticeSpell::MPEater.learned_bit(),
        0
    );
    owner.inner.force_authoritative_player_vitals(None, Some(0));
    let mut state = owner.zone_state.lock().unwrap();
    let (position, _) = state.zone_manager.player_transform(&session).unwrap();
    let (hp, max_hp, _) = state.zone_manager.player_vitals(&session).unwrap();
    state.zone_manager.handle(ZoneCommand::SyncPlayerVitals {
        session_id: session.clone(),
        hp,
        max_hp,
        mp: 0,
    });
    for command in commands {
        state.zone_manager.handle(command);
    }
    let monster_id = 94_101;
    state.zone_manager.handle(ZoneCommand::SpawnMonster {
        session_id: session.clone(),
        now_ms: now,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: monster_id,
            name: "Yob".into(),
            name_colour_argb: -1,
            image: 1,
            ai: 0,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 1,
            hp: 100_000,
            max_hp: 100_000,
            experience: 0,
            move_speed_ms: 1_000,
            attack_speed_ms: 2_000,
            friendly_guild: None,
            position: Point {
                x: position.x + 1,
                y: position.y,
            },
            direction: MirDirection::Left,
            defense: ZoneMonsterDefense::default(),
            respawn: None,
            drops: Vec::new(),
        },
    });
    let target = state
        .zone_manager
        .native_monster_snapshots(&ZoneKey::for_map(&state.players[&key].map_file_name))
        .into_iter()
        .find(|monster| monster.object_id == monster_id)
        .unwrap();
    let dx = target.position.x - position.x;
    let dy = target.position.y - position.y;
    assert_eq!(
        dx.abs().max(dy.abs()),
        1,
        "actual spawned target must be in Source melee range"
    );
    let direction = match (dx, dy) {
        (0, -1) => MirDirection::Up,
        (1, -1) => MirDirection::UpRight,
        (1, 0) => MirDirection::Right,
        (1, 1) => MirDirection::DownRight,
        (0, 1) => MirDirection::Down,
        (-1, 1) => MirDirection::DownLeft,
        (-1, 0) => MirDirection::Left,
        (-1, -1) => MirDirection::UpLeft,
        _ => unreachable!("range assertion restricts the actual adjacent target"),
    };
    // Source charges MPEater on lawful attacks, arms at 100, and consumes it on
    // the NEXT launch. A generous bound retains those real clocks and rolls.
    for attempt in 0..=101 {
        let outcomes = state.zone_manager.handle(ZoneCommand::PlayerAttackObject {
            session_id: session.clone(),
            object_id: monster_id,
            direction,
            spell: attack_spell as u8,
            level,
            attack_type: 0,
            damage,
            now_ms: now + attempt * 2_000,
        });
        let mana = outcomes.iter().find_map(|outbound| match outbound {
            ZoneOutbound::PlayerManaChanged {
                session_id,
                settlement,
            } if *session_id == session => Some(*settlement),
            _ => None,
        });
        if let Some(mana) = mana {
            let practice = outcomes
                .into_iter()
                .find_map(|outbound| match outbound {
                    ZoneOutbound::FiveClassMagicPractice { receipt }
                        if receipt.spell == ZoneFiveClassPracticeSpell::MPEater =>
                    {
                        Some(receipt)
                    }
                    _ => None,
                })
                .expect("a consumed Source MPEater must also issue its practice receipt");
            assert_eq!(mana.mp_before, 0);
            assert!(mana.mp_after > 0 && mana.mp_after <= projected.max_mp);
            assert_eq!(mana.object_id, state.players[&key].zone_object_id);
            assert_eq!(
                mana.life_generation,
                state.zone_manager.player_life_generation(&session).unwrap()
            );
            assert_eq!(
                state.zone_manager.player_vitals(&session).unwrap().2,
                mana.mp_after
            );
            return (mana, practice);
        }
    }
    panic!("Source MPEater did not arm/consume after 102 lawful, clock-separated attacks");
}

#[test]
fn five_class_receipt_actual_mp_eater_mana_order_is_independent_from_practice() {
    let haste = ZoneFiveClassPracticeSpell::Haste;
    let light = ZoneFiveClassPracticeSpell::LightBody;
    let (_, mut owner) = fixture(
        "FcMana",
        MirClass::Assassin,
        &[
            (haste, 0),
            (light, 0),
            (ZoneFiveClassPracticeSpell::MPEater, 3),
        ],
    );
    let now = SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000;
    let first = source_practice(&owner, haste, now);
    let second = source_practice(&owner, light, now + 3_000);
    queue_practice(&owner, vec![first, second]);
    owner.apply_pending_zone_packets();
    let before_hp = owner.inner.local_player_vitals_snapshot().player_hp;
    let (mana, practice) = source_mp_eater(&mut owner, now + 6_000);
    assert!(practice.sequence > mana.receipt_sequence);
    // Level-three practice has no further XP, but is still a consumed ordering
    // event. It must not consume the independent first MP settlement.
    assert!(owner.apply_five_class_practice_receipt(practice).is_empty());
    assert!(owner.apply_five_class_mana_receipt(mana));
    assert_eq!(
        owner.inner.local_player_vitals_snapshot().player_mp,
        Some(mana.mp_after)
    );
    assert_eq!(
        owner.inner.local_player_vitals_snapshot().player_hp,
        before_hp
    );
    owner.inner.force_authoritative_player_vitals(None, Some(0));
    assert!(
        !owner.apply_five_class_mana_receipt(mana),
        "duplicate must not refill spent mana"
    );
    assert_eq!(
        owner.inner.local_player_vitals_snapshot().player_mp,
        Some(0)
    );
}

#[test]
fn five_class_receipt_mana_rejects_invalid_shape_life_and_online_owner_without_consuming_sequence()
{
    let (_, mut owner) = fixture(
        "FcManaReject",
        MirClass::Assassin,
        &[(ZoneFiveClassPracticeSpell::MPEater, 3)],
    );
    let (mana, _) = source_mp_eater(
        &mut owner,
        SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000,
    );
    let max_mp = owner.inner.zone_player_combat_stats().max_mp;
    let mut invalid = vec![mana; 7];
    invalid[0].receipt_sequence = 0;
    invalid[1].object_id = 0;
    invalid[2].life_generation += 1;
    invalid[3].mp_before = -1;
    invalid[4].mp_after = -1;
    invalid[5].mp_after = max_mp + 1;
    invalid[6].mp_after = invalid[6].mp_before;
    for mut bad in invalid {
        if bad.receipt_sequence > 0 {
            bad.receipt_sequence = u64::MAX;
        }
        assert!(!owner.apply_five_class_mana_receipt(bad));
        assert_eq!(
            owner.inner.local_player_vitals_snapshot().player_mp,
            Some(0)
        );
    }
    assert!(owner.apply_five_class_mana_receipt(mana));
    assert_eq!(
        owner.inner.local_player_vitals_snapshot().player_mp,
        Some(mana.mp_after)
    );
}

#[test]
fn five_class_receipt_mana_teardown_fence_refuses_late_internal_result() {
    let (_, mut owner) = fixture(
        "FcManaFence",
        MirClass::Assassin,
        &[(ZoneFiveClassPracticeSpell::MPEater, 3)],
    );
    let (mana, _) = source_mp_eater(
        &mut owner,
        SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000,
    );
    let key = owner.current_presence_key().unwrap();
    let session = owner.current_zone_session_id().unwrap();
    {
        let mut state = owner.zone_state.lock().unwrap();
        state.begin_teardown_fence(&key).unwrap();
        state.dispatch_zone_outbounds(
            vec![ZoneOutbound::PlayerManaChanged {
                session_id: session,
                settlement: mana,
            }],
            None,
        );
        assert!(state.take_pending_zone_player_damages(&key).is_empty());
    }
    assert!(!owner.apply_five_class_mana_receipt(mana));
    assert_eq!(
        owner.inner.local_player_vitals_snapshot().player_mp,
        Some(0)
    );
    owner.release_teardown_fence().unwrap();
    assert!(
        owner.apply_five_class_mana_receipt(mana),
        "fence rejection must not advance the independent MP watermark"
    );
}

#[test]
fn five_class_receipt_mana_old_life_rejected_and_new_life_real_result_accepted() {
    let (_, mut owner) = fixture(
        "FcManaLife",
        MirClass::Assassin,
        &[(ZoneFiveClassPracticeSpell::MPEater, 3)],
    );
    let now = SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000;
    let (old, _) = source_mp_eater(&mut owner, now);
    let session = owner.current_zone_session_id().unwrap();
    {
        let mut state = owner.zone_state.lock().unwrap();
        let (_, max_hp, mp) = state.zone_manager.player_vitals(&session).unwrap();
        state
            .zone_manager
            .handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: session.clone(),
                hp: 0,
                max_hp,
                mp,
                dead: true,
            });
        state
            .zone_manager
            .handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: session.clone(),
                hp: max_hp,
                max_hp,
                mp,
                dead: false,
            });
        assert_eq!(
            state.zone_manager.player_life_generation(&session),
            Some(old.life_generation + 1)
        );
    }
    owner.inner.force_authoritative_player_vitals(None, Some(0));
    assert!(!owner.apply_five_class_mana_receipt(old));
    assert_eq!(
        owner.inner.local_player_vitals_snapshot().player_mp,
        Some(0)
    );
    let (fresh, _) = source_mp_eater(&mut owner, now + 220_000);
    assert_eq!(fresh.life_generation, old.life_generation + 1);
    assert!(owner.apply_five_class_mana_receipt(fresh));
    assert_eq!(
        owner.inner.local_player_vitals_snapshot().player_mp,
        Some(fresh.mp_after)
    );
}

#[test]
fn five_class_receipt_real_hp_watermark_does_not_consume_first_mp_settlement() {
    let (shared, mut owner) = fixture(
        "FcManaHp",
        MirClass::Assassin,
        &[(ZoneFiveClassPracticeSpell::MPEater, 3)],
    );
    let mut attacker = shared_session_runtime(shared.clone());
    start_new_runtime_with_class(
        &mut attacker,
        "FcHpAttacker",
        "FcHpAttacker",
        MirClass::Warrior,
    );
    learn_source_skills(&mut attacker, &[]);
    let owner_session = owner.current_zone_session_id().unwrap();
    let attacker_session = attacker.current_zone_session_id().unwrap();
    let owner_key = owner.current_presence_key().unwrap();
    let owner_join = owner
        .inner
        .active_zone_join_snapshot(owner_session.as_str())
        .unwrap();
    let attacker_join = attacker
        .inner
        .active_zone_join_snapshot(attacker_session.as_str())
        .unwrap();
    let commands = attacker
        .authoritative_zone_combat_commands(&attacker_session)
        .unwrap();
    let (spell, level, damage) = attacker.inner.zone_melee_attack_profile(Spell::None);
    let now = SharedInProcessZoneSessionRuntime::zone_now_ms() + 2_000;
    let mut state = shared.lock().unwrap();
    // As in the existing real vital-receipt fixture, a server-authored lawful
    // PvP relationship isolates settlement ordering from safe-zone policy QA.
    for (session, mut profile) in [
        (owner_session.clone(), owner_join.chat_profile),
        (attacker_session.clone(), attacker_join.chat_profile),
    ] {
        profile.in_safe_zone = false;
        profile.attack_mode = 5;
        state.zone_manager.handle(ZoneCommand::UpdateChatProfile {
            session_id: session,
            profile,
        });
    }
    for command in commands {
        state.zone_manager.handle(command);
    }
    let position = state
        .zone_manager
        .player_transform(&owner_session)
        .unwrap()
        .0;
    let mut accepted_direction = None;
    for (point, direction) in [
        (
            Point {
                x: position.x + 1,
                y: position.y,
            },
            MirDirection::Left,
        ),
        (
            Point {
                x: position.x - 1,
                y: position.y,
            },
            MirDirection::Right,
        ),
        (
            Point {
                x: position.x,
                y: position.y + 1,
            },
            MirDirection::Up,
        ),
        (
            Point {
                x: position.x,
                y: position.y - 1,
            },
            MirDirection::Down,
        ),
    ] {
        state.zone_manager.handle(ZoneCommand::SyncPlayerTransform {
            session_id: attacker_session.clone(),
            position: point.clone(),
            direction,
        });
        if state
            .zone_manager
            .player_transform(&attacker_session)
            .unwrap()
            .0
            == point
        {
            accepted_direction = Some(direction);
            break;
        }
    }
    let direction = accepted_direction.expect("actual Zone must admit one free adjacent PvP tile");
    let owner_id = state.players[&owner_key].zone_object_id;
    let mut hp_results = Vec::new();
    for attempt in 0..64 {
        let outcomes = state.zone_manager.handle(ZoneCommand::PlayerAttackObject {
            session_id: attacker_session.clone(),
            object_id: owner_id,
            direction,
            spell: spell as u8,
            level,
            attack_type: 0,
            damage,
            now_ms: now + attempt * 2_000,
        });
        for outcome in outcomes {
            if let ZoneOutbound::PlayerDamaged {
                session_id,
                damage,
                settlement: Some(settlement),
            } = outcome
            {
                if session_id == owner_session {
                    assert!(damage > 0 && !settlement.death_transition);
                    hp_results.push(QueuedZoneVitalDelta::Settled {
                        delta: damage,
                        settlement,
                        death_location: None,
                    });
                }
            }
        }
        if hp_results.len() >= 2 {
            break;
        }
    }
    assert_eq!(
        hp_results.len(),
        2,
        "two real Source PvP impacts must stamp owner HP settlements"
    );
    let last_hp_sequence = hp_results
        .iter()
        .filter_map(|result| match result {
            QueuedZoneVitalDelta::Settled { settlement, .. } => Some(settlement.receipt_sequence),
            _ => None,
        })
        .max()
        .unwrap();
    drop(state);
    owner.apply_zone_player_damages(hp_results);
    let (mana, _) = source_mp_eater(&mut owner, now + 130_000);
    assert!(
        last_hp_sequence > mana.receipt_sequence,
        "fixture must order independent HP sequence above the first MP sequence"
    );
    assert!(owner.apply_five_class_mana_receipt(mana));
    assert_eq!(
        owner.inner.local_player_vitals_snapshot().player_mp,
        Some(mana.mp_after)
    );
}
