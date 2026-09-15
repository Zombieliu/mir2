use super::*;
use mir2_simulation::ZoneSoulFirePracticeReceipt;

fn fixture(
    name: &str,
    experience: u16,
) -> (
    Arc<Mutex<SharedInProcessZoneState>>,
    SharedInProcessZoneSessionRuntime,
) {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_new_runtime_with_class(&mut runtime, name, name, MirClass::Taoist);
    equip_runtime_crystal_items(
        &mut runtime,
        &[("Amulet", mir2_simulation::EquipmentSlot::Amulet, 100)],
    );
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.level = 23;
    save.skill_states_json = vec![serde_json::json!({
        "key": "soulfireball", "name": "SoulFireBall", "description": "", "level": 0,
        "experience": experience, "cooldown_ticks": 0, "cooldown_ends_at": 0
    })
    .to_string()];
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    runtime.sync_zone_snapshot();
    (shared, runtime)
}

fn receipt(
    runtime: &SharedInProcessZoneSessionRuntime,
    cast_at_ms: u64,
) -> ZoneSoulFirePracticeReceipt {
    let identity = runtime.inner.active_identity().unwrap();
    let key = ZonePresenceKey::from_identity(&identity);
    let state = runtime.zone_state.lock().unwrap();
    let session_id = state.zone_sessions[&key].clone();
    ZoneSoulFirePracticeReceipt {
        session_id: session_id.clone(),
        account_id: identity.account_id,
        character_index: identity.character_index,
        object_id: state.players[&key].zone_object_id,
        life_generation: state
            .zone_manager
            .player_life_generation(&session_id)
            .unwrap(),
        zone_key: ZoneKey::for_map(&state.players[&key].map_file_name),
        cast_at_ms,
        target_object_id: 9101,
        target_location: Point { x: 12, y: 10 },
        damage: 7,
    }
}

fn skill(runtime: &SharedInProcessZoneSessionRuntime) -> (u8, u16) {
    let save = runtime.inner.active_character_checkpoint().unwrap();
    let skill = save
        .skill_states_json
        .iter()
        .map(|value| serde_json::from_str::<serde_json::Value>(value).unwrap())
        .find(|value| value["key"] == "soulfireball")
        .unwrap();
    (
        skill["level"].as_u64().unwrap() as u8,
        skill["experience"].as_u64().unwrap() as u16,
    )
}

fn queue(runtime: &SharedInProcessZoneSessionRuntime, receipts: Vec<ZoneSoulFirePracticeReceipt>) {
    runtime.zone_state.lock().unwrap().dispatch_zone_outbounds(
        receipts
            .into_iter()
            .map(|receipt| ZoneOutbound::SoulFirePractice { receipt })
            .collect(),
        None,
    );
}

#[test]
fn zone_soulfire_practice_owner_only_dedupes_queue_and_later_replay() {
    let (shared, mut owner) = fixture("SfbOwner", 0);
    let mut observer = shared_session_runtime(shared.clone());
    start_new_runtime_with_class(
        &mut observer,
        "SfbObserver",
        "SfbObserver",
        MirClass::Taoist,
    );
    let hit = receipt(&owner, 0);
    queue(&owner, vec![hit.clone(), hit.clone()]);
    let observer_packets = observer.apply_pending_zone_packets();
    assert!(!observer_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::MagicLeveled {
            spell: Spell::SoulFireBall,
            ..
        }
    )));
    assert_eq!(skill(&owner), (0, 0));
    let packets = owner.apply_pending_zone_packets();
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(
                packet,
                ServerPacket::MagicLeveled {
                    spell: Spell::SoulFireBall,
                    ..
                }
            ))
            .count(),
        1
    );
    let progressed = skill(&owner);
    assert!((1..=3).contains(&progressed.1));
    queue(&owner, vec![hit]);
    assert!(owner
        .apply_pending_zone_packets()
        .iter()
        .all(|packet| !matches!(packet, ServerPacket::MagicLeveled { .. })));
    assert_eq!(skill(&owner), progressed);
}

#[test]
fn zone_soulfire_practice_invalid_owner_zero_damage_and_stale_life_are_rejected() {
    let (_, mut runtime) = fixture("SfbInvalid", 0);
    let valid = receipt(&runtime, 100);
    let mut invalid = Vec::new();
    for case in 0..8 {
        let mut hit = valid.clone();
        match case {
            0 => hit.damage = 0,
            1 => hit.target_object_id = 0,
            2 => hit.account_id.push_str("-other"),
            3 => hit.character_index += 1,
            4 => hit.object_id += 1,
            5 => hit.life_generation += 1,
            6 => hit.zone_key = ZoneKey::for_map("3"),
            _ => hit.session_id = SessionId::new("old-session"),
        }
        invalid.push(hit);
    }
    queue(&runtime, invalid);
    assert!(!runtime
        .apply_pending_zone_packets()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MagicLeveled { .. })));
    assert_eq!(skill(&runtime), (0, 0));
    queue(&runtime, vec![valid]);
    assert_eq!(
        runtime.apply_pending_zone_soulfire_practice(false).len(),
        1,
        "invalid input must not consume valid cast identity"
    );
}

#[test]
fn zone_soulfire_practice_pending_and_dedupe_progress_survive_shared_checkpoint() {
    let (_, mut runtime) = fixture("SfbCheckpoint", 0);
    let hit = receipt(&runtime, 100);
    queue(&runtime, vec![hit.clone()]);
    {
        let mut state = runtime.zone_state.lock().unwrap();
        let encoded = serde_json::to_vec(&state.checkpoint().unwrap()).unwrap();
        *state =
            SharedInProcessZoneState::restore(serde_json::from_slice(&encoded).unwrap()).unwrap();
    }
    assert_eq!(runtime.apply_pending_zone_soulfire_practice(false).len(), 1);
    let progressed = skill(&runtime);
    {
        let mut state = runtime.zone_state.lock().unwrap();
        let encoded = serde_json::to_vec(&state.checkpoint().unwrap()).unwrap();
        *state =
            SharedInProcessZoneState::restore(serde_json::from_slice(&encoded).unwrap()).unwrap();
    }
    queue(&runtime, vec![hit]);
    assert!(runtime
        .apply_pending_zone_soulfire_practice(false)
        .is_empty());
    assert_eq!(skill(&runtime), progressed);
}

#[test]
fn zone_soulfire_practice_obeys_original_character_level_gate() {
    let (_, mut runtime) = fixture("SfbGate", 0);
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.level = 17;
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    queue(&runtime, vec![receipt(&runtime, 100)]);
    assert!(runtime
        .apply_pending_zone_soulfire_practice(false)
        .is_empty());
    assert_eq!(skill(&runtime), (0, 0));
    save.character.level = 18;
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    queue(&runtime, vec![receipt(&runtime, 1900)]);
    assert_eq!(runtime.apply_pending_zone_soulfire_practice(false).len(), 1);
    assert!((1..=3).contains(&skill(&runtime).1));
}

#[test]
fn zone_soulfire_practice_normal_teardown_checkpoint_includes_prior_resolved_practice() {
    let (_, mut runtime) = fixture("SfbTeardown", 0);
    queue(&runtime, vec![receipt(&runtime, 100)]);
    let lease = ZoneOwnerLease::in_process(&ZoneId::new("test-shared-zone"));
    let prepared = runtime
        .prepare_teardown_checkpoint(&lease)
        .unwrap()
        .unwrap();
    let saved = prepared
        .checkpoint()
        .skill_states_json
        .iter()
        .map(|value| serde_json::from_str::<serde_json::Value>(value).unwrap())
        .find(|value| value["key"] == "soulfireball")
        .unwrap();
    assert!((1..=3).contains(&saved["experience"].as_u64().unwrap()));
}

#[test]
fn zone_soulfire_practice_stale_queued_receipt_cannot_follow_reconnect_or_revive() {
    for revive in [false, true] {
        let (_, mut runtime) = fixture(if revive { "SfbRevive" } else { "SfbReconnect" }, 0);
        let hit = receipt(&runtime, 100);
        queue(&runtime, vec![hit.clone()]);
        let key = runtime.current_presence_key().unwrap();
        {
            let mut state = runtime.zone_state.lock().unwrap();
            if revive {
                let (_, max_hp, mp) = state.zone_manager.player_vitals(&hit.session_id).unwrap();
                state.zone_manager.handle(ZoneCommand::SyncPlayerVitals {
                    session_id: hit.session_id.clone(),
                    hp: 0,
                    max_hp,
                    mp,
                });
                state.zone_manager.handle(ZoneCommand::SyncPlayerVitals {
                    session_id: hit.session_id.clone(),
                    hp: max_hp,
                    max_hp,
                    mp,
                });
            } else {
                // A reconnect must use a new authenticated online incarnation,
                // even if its account/character and display name are unchanged.
                state.players.get_mut(&key).unwrap().zone_object_id += 1;
            }
        }
        assert!(runtime
            .apply_pending_zone_soulfire_practice(false)
            .is_empty());
        assert_eq!(skill(&runtime), (0, 0));
    }
}

#[test]
fn zone_soulfire_practice_dead_caster_keeps_lawful_hit_and_logout_fence_drains_only_prior_receipts()
{
    let (_, mut runtime) = fixture("SfbFence", 0);
    let hit = receipt(&runtime, 100);
    queue(&runtime, vec![hit.clone()]);
    let key = runtime.current_presence_key().unwrap();
    {
        let mut state = runtime.zone_state.lock().unwrap();
        let (_, max_hp, mp) = state.zone_manager.player_vitals(&hit.session_id).unwrap();
        state.zone_manager.handle(ZoneCommand::SyncPlayerVitals {
            session_id: hit.session_id.clone(),
            hp: 0,
            max_hp,
            mp,
        });
        state.begin_teardown_fence(&key).unwrap();
    }
    let mut late = hit.clone();
    late.cast_at_ms += 1800;
    queue(&runtime, vec![late]);
    let packets = runtime.apply_pending_zone_soulfire_practice(true);
    assert_eq!(packets.len(), 1);
    assert!((1..=3).contains(&skill(&runtime).1));
}

#[test]
fn zone_soulfire_practice_resolved_hit_is_saved_reloaded_and_next_public_cast_uses_level_one() {
    let (_, mut runtime) = fixture("SfbReload", 1299);
    let character_index = runtime.inner.active_identity().unwrap().character_index;
    let (target, _) = prepare_gateway_range_fixture(&mut runtime, 2);
    let session_id = runtime.current_zone_session_id().unwrap();
    let now = shared_gateway_now_ms();
    let launch = runtime.dispatch_zone_player_command(
        ZoneCommand::PlayerCastMagicWithItem {
            session_id,
            object_id: target.object_id,
            spell: Spell::SoulFireBall,
            direction: MirDirection::Right,
            target: Point {
                x: target.x,
                y: target.y,
            },
            cast: true,
            level: 0,
            damage: 7,
            mp_cost: 3,
            cooldown_ms: 1800,
            item_param: 0,
            now_ms: now,
        },
        false,
    );
    assert!(launch.iter().any(|packet| matches!(
        packet,
        ServerPacket::Magic {
            spell: Spell::SoulFireBall,
            cast: true,
            level: 0,
            ..
        }
    )));
    assert_eq!(
        skill(&runtime),
        (0, 1299),
        "launch must not advance practice"
    );
    let resolved =
        runtime.dispatch_zone_player_command(ZoneCommand::Tick { now_ms: now + 1 }, false);
    assert!(resolved.iter().any(|packet| matches!(packet, ServerPacket::DamageIndicator { damage, object_id, .. } if *damage > 0 && *object_id == target.object_id)), "{resolved:?}");
    assert!(resolved.iter().any(|packet| matches!(
        packet,
        ServerPacket::MagicLeveled {
            spell: Spell::SoulFireBall,
            level: 1,
            experience: 0..=2,
            ..
        }
    )));
    let progressed = skill(&runtime);
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index,
        }))
        .unwrap();
    assert_eq!(
        skill(&runtime),
        progressed,
        "normal logout/startgame must preserve earned practice"
    );
    let (target, _) = prepare_gateway_range_fixture(&mut runtime, 2);
    let local = runtime.inner.world_snapshot().player_object_id.unwrap();
    let next = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Magic {
            object_id: local,
            spell: Spell::SoulFireBall,
            direction: MirDirection::Right,
            target_id: target.object_id,
            location: Point {
                x: target.x,
                y: target.y,
            },
            spell_target_lock: true,
        }))
        .unwrap();
    assert!(
        next.iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::SoulFireBall,
                cast: true,
                level: 1,
                ..
            }
        )),
        "{next:?}"
    );
    assert!(
        next.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectMagic {
                spell: Spell::SoulFireBall,
                level: 1,
                ..
            }
        )),
        "{next:?}"
    );
}
