use super::*;
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

#[test]
fn accepted_thrusting_trains_only_an_adjacent_primary_and_never_a_rejected_retry() {
    for distance in [1, 2] {
        let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
        let mut runtime = shared_session_runtime(shared);
        start_new_runtime(&mut runtime, &format!("thrust-xp-{distance}"), "ThrustXP");
        let mut save = runtime.inner.active_character_checkpoint().unwrap();
        save.character.level = 28;
        save.skill_states_json = vec![serde_json::json!({
            "key": "thrusting", "name": "Thrusting", "description": "",
            "level": 0, "experience": 0, "cooldown_ticks": 0, "cooldown_ends_at": 0
        })
        .to_string()];
        runtime
            .inner
            .restore_active_character_checkpoint(&save)
            .unwrap();
        runtime
            .execute(WorldCommand::ClientPacket(ClientPacket::SpellToggle {
                spell: Spell::Thrusting,
                toggle_state: 1,
            }))
            .unwrap();
        assert_eq!(
            runtime.inner.zone_melee_attack_profile(Spell::Thrusting).0,
            Spell::Thrusting
        );
        // SpellToggle can advance the personal mirror. Position relative to
        // the authoritative shared target, not that independently ticked copy.
        let target = runtime
            .world_snapshot()
            .entities
            .into_iter()
            .find(|entity| {
                entity.kind == WorldEntityKind::Monster
                    && !entity.dead
                    && entity.hp.is_some_and(|hp| hp > 1)
                    && entity.disposition == WorldEntityDisposition::Hostile
            })
            .unwrap();
        let session_id = runtime.current_zone_session_id().unwrap();
        // A map obstacle may make SyncPlayerTransform choose a nearby legal
        // cell. Never overwrite its result with the requested private position:
        // directional Attack intentionally resolves from the Zone presence.
        let mut rejected_origins = Vec::new();
        let direction = [
            (MirDirection::Right, 1, 0), (MirDirection::Down, 0, 1),
            (MirDirection::Left, -1, 0), (MirDirection::Up, 0, -1),
            (MirDirection::DownRight, 1, 1), (MirDirection::DownLeft, -1, 1),
            (MirDirection::UpLeft, -1, -1), (MirDirection::UpRight, 1, -1),
        ].into_iter().find_map(|(direction, dx, dy)| {
            let requested = Point { x: target.x - dx * distance, y: target.y - dy * distance };
            runtime.dispatch_zone_player_command(ZoneCommand::SyncPlayerTransform {
                session_id: session_id.clone(), position: requested.clone(), direction,
            }, false);
            let (actual, has_primary) = {
                let state = runtime.zone_state.lock().unwrap();
                (state.zone_manager.player_transform(&session_id).unwrap().0,
                    state.zone_manager.melee_primary_target_present(&session_id, direction, None))
            };
            if actual != requested {
                rejected_origins.push((requested, actual));
                return None;
            }
            if distance == 2 && has_primary { return None; }
            let snapshot = runtime.inner.world_snapshot();
            let presence = runtime.authoritative_self_entity_for_snapshot(&snapshot).unwrap();
            assert_eq!((presence.x, presence.y), (actual.x, actual.y));
            Some(direction)
        }).unwrap_or_else(|| panic!("no legal distance-{distance} origin around {target:?}; rejected={rejected_origins:?}"));
        eprintln!("distance-{distance} fixture rejected origin corrections: {rejected_origins:?}");
        let attack = || {
            WorldCommand::ClientPacket(ClientPacket::Attack {
                spell: Spell::Thrusting,
                direction,
            })
        };
        let prepared = runtime
            .prepare_zone_native_player_attack(&attack())
            .unwrap_or_else(|| {
                panic!("distance={distance}, direction={direction:?}, target={target:?}")
            });
        assert_eq!(
            prepared.object_id, target.object_id,
            "directional acquisition must use the intended target"
        );
        let accepted = runtime.execute(attack()).unwrap();
        assert!(
            accepted
                .iter()
                .any(|p| matches!(p, ServerPacket::ObjectAttack { info }
            if info.spell == Spell::Thrusting as u8)),
            "distance={distance}: {accepted:?}"
        );
        let experience = |runtime: &SharedInProcessZoneSessionRuntime| {
            let save = runtime.inner.active_character_checkpoint().unwrap();
            save.skill_states_json
                .iter()
                .map(|skill| serde_json::from_str::<serde_json::Value>(skill).unwrap())
                .find(|skill| skill["key"] == "thrusting")
                .unwrap()["experience"]
                .as_u64()
                .unwrap_or(0)
        };
        let expected = experience(&runtime);
        if distance == 1 {
            assert!(expected > 0, "accepted primary trains the skill");
        } else {
            assert_eq!(expected, 0, "second-cell-only hit does not train");
        }
        let retry = runtime.execute(attack()).unwrap();
        assert!(!retry
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectAttack { .. })));
        assert_eq!(experience(&runtime), expected);
    }
}

fn only_zone_player_accuracy(shared: &Arc<Mutex<SharedInProcessZoneState>>) -> i64 {
    let manager_bytes = shared
        .lock()
        .expect("shared state should lock")
        .zone_manager
        .checkpoint_bytes()
        .expect("Zone manager checkpoint should encode");
    let manager: serde_json::Value =
        serde_json::from_slice(&manager_bytes).expect("Zone manager checkpoint should be JSON");
    let runtime_encoded = manager["zones"]
        .as_array()
        .and_then(|zones| zones.first())
        .and_then(|zone| zone.as_array())
        .and_then(|zone| zone.get(1))
        .and_then(serde_json::Value::as_str)
        .expect("checkpoint should contain one encoded Zone");
    let runtime_bytes = STANDARD
        .decode(runtime_encoded)
        .expect("Zone runtime checkpoint should decode");
    let runtime: serde_json::Value =
        serde_json::from_slice(&runtime_bytes).expect("Zone runtime checkpoint should be JSON");
    runtime["players"]
        .as_object()
        .and_then(|players| players.values().next())
        .and_then(|player| player["combat_stats"]["accuracy"].as_i64())
        .expect("checkpoint should contain the only player's accuracy")
}

fn trusted_personal_combat_stats(
    runtime: &SharedInProcessZoneSessionRuntime,
) -> mir2_simulation::ZonePlayerCombatStats {
    runtime.inner.zone_player_combat_stats()
}

#[test]
fn next_accepted_zone_melee_uses_accuracy_from_passive_level_up() {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_new_runtime(&mut runtime, "fencing-stat-sync", "FencingSync");

    let mut save = runtime
        .inner
        .active_character_checkpoint()
        .expect("active character checkpoint");
    save.character.level = 60;
    save.skill_states_json = vec![serde_json::json!({
        "key": "fencing",
        "name": "Fencing",
        "description": "",
        "level": 2,
        "experience": 1299,
        "cooldown_ticks": 0,
        "cooldown_ends_at": 0
    })
    .to_string()];
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .expect("Fencing fixture should restore");

    let session_id = runtime
        .current_zone_session_id()
        .expect("runtime should own a Zone session");
    let stale_stats = trusted_personal_combat_stats(&runtime);
    let _ = runtime.dispatch_zone_player_command(
        ZoneCommand::UpdatePlayerCombatStats {
            session_id,
            stats: stale_stats,
        },
        false,
    );
    assert_eq!(
        only_zone_player_accuracy(&shared),
        i64::from(stale_stats.accuracy)
    );

    let progression = runtime.inner.commit_zone_melee_attack_spell(Spell::None);
    assert!(progression.iter().any(|packet| matches!(
        packet,
        ServerPacket::MagicLeveled {
            spell: Spell::Fencing,
            level: 3,
            ..
        }
    )));
    let leveled_stats = trusted_personal_combat_stats(&runtime);
    assert!(leveled_stats.accuracy > stale_stats.accuracy);
    assert_eq!(
        only_zone_player_accuracy(&shared),
        i64::from(stale_stats.accuracy),
        "personal progression alone must not mutate shared Zone state"
    );

    let (target, _) = prepare_gateway_range_fixture(&mut runtime, 1);
    assert_eq!(
        only_zone_player_accuracy(&shared),
        i64::from(stale_stats.accuracy),
        "the positioning fixture must leave Zone combat stats stale before the attack"
    );
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Attack {
            direction: MirDirection::Right,
            spell: Spell::None,
        }))
        .expect("next melee attack should execute");

    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectAttack { info }
                if info.spell == Spell::None as u8 && info.direction == MirDirection::Right
        )),
        "{packets:?}; target={target:?}"
    );
    assert_eq!(
        only_zone_player_accuracy(&shared),
        i64::from(leveled_stats.accuracy),
        "the accepted attack must resolve with the newly leveled passive stats"
    );
}
