use super::*;
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

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
