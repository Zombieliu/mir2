//! Normal authenticated packet admission must not reintroduce a STUN lock.
use super::*;
use serde_json::json;

fn fixture(status_key: &str) -> SharedInProcessZoneSessionRuntime {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    let character_name = if status_key == "crystal-stun" {
        "StunEscape"
    } else {
        "DazeEscape"
    };
    start_new_runtime(&mut runtime, status_key, character_name);
    let position = Point { x: 330, y: 270 };
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.position = position.clone();
    save.buff_states_json = vec![json!({
        "key": status_key, "name": status_key, "description": "isolated finite-status fixture",
        "expires_at_tick": 1_000_000, "attack_bonus": 0, "defence_bonus": 0, "stats": []
    })
    .to_string()];
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    runtime
        .inner
        .force_authoritative_player_transform(position.clone(), MirDirection::Right);
    runtime.sync_zone_snapshot();
    let key = runtime.current_presence_key().unwrap();
    let mut target = shared_monster_entity(260_909);
    target.ai = Some(2);
    target.x = 331;
    target.y = 270;
    {
        let mut state = shared.lock().unwrap();
        state.update_player_transform(&key, position, MirDirection::Right);
        state.sync_map_layer(
            "0".into(),
            vec![target],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
    }
    assert!(runtime
        .inner
        .world_snapshot()
        .active_buffs
        .iter()
        .any(|buff| buff.key == status_key));
    runtime
}

#[test]
fn crowded_stunned_owner_can_send_normal_melee_packet() {
    let mut runtime = fixture("crystal-stun");
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Attack {
            direction: MirDirection::Right,
            spell: Spell::None,
        }))
        .unwrap();
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })),
        "trusted personal admission must not turn STUN into an attack/movement lock: {packets:?}"
    );
}

#[test]
fn crowded_dazed_owner_cannot_bypass_physical_admission_with_normal_packet() {
    let mut runtime = fixture("crystal-dazed");
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Attack {
            direction: MirDirection::Right,
            spell: Spell::None,
        }))
        .unwrap();
    assert!(!packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })));
}

fn magic_fixture(status_key: Option<&str>) -> SharedInProcessZoneSessionRuntime {
    let mut runtime = fixture(status_key.unwrap_or("crystal-stun"));
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.class = MirClass::Wizard;
    save.character.level = 20;
    save.mp = 100;
    save.max_mp = 100;
    if status_key.is_none() {
        save.buff_states_json.clear();
    }
    save.skill_states_json = vec![json!({
        "key": "fireball", "name": "FireBall", "description": "isolated cast fixture",
        "level": 0, "experience": 0, "cooldown_ticks": 0, "cooldown_ends_at": 0
    })
    .to_string()];
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    let sid = runtime.current_zone_session_id().unwrap();
    runtime.dispatch_zone_player_command(
        ZoneCommand::SyncPlayerVitals {
            session_id: sid,
            hp: save.hp,
            max_hp: save.max_hp,
            mp: 100,
        },
        false,
    );
    runtime.sync_zone_snapshot();
    runtime
}

fn fireball_command(runtime: &SharedInProcessZoneSessionRuntime) -> WorldCommand {
    WorldCommand::ClientPacket(ClientPacket::Magic {
        object_id: runtime.inner.world_snapshot().player_object_id.unwrap(),
        spell: Spell::FireBall,
        direction: MirDirection::Right,
        target_id: 260_909,
        location: Point { x: 331, y: 270 },
        spell_target_lock: true,
    })
}

#[test]
fn crowded_uncontrolled_owner_can_cast_normal_magic_packet() {
    let mut runtime = magic_fixture(None);
    let packets = runtime.execute(fireball_command(&runtime)).unwrap();
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::FireBall,
                cast: true,
                ..
            }
        )),
        "ordinary learned Magic must reach the shared dispatcher: {packets:?}"
    );
}

#[test]
fn crowded_blindness_does_not_become_a_native_spell_lock() {
    let mut runtime = magic_fixture(Some("crystal-blindness"));
    let packets = runtime.execute(fireball_command(&runtime)).unwrap();
    assert!(
        packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::FireBall,
                cast: true,
                ..
            }
        )),
        "Crystal CanCast omits Blindness: {packets:?}"
    );
}

#[test]
fn crowded_status_owner_cannot_bypass_cast_lock_or_spend_with_normal_magic_packet() {
    for status in ["crystal-stun", "crystal-dazed"] {
        let mut runtime = magic_fixture(Some(status));
        let sid = runtime.current_zone_session_id().unwrap();
        let before_mp = runtime
            .zone_state
            .lock()
            .unwrap()
            .zone_manager
            .player_vitals(&sid)
            .unwrap()
            .2;
        let packets = runtime.execute(fireball_command(&runtime)).unwrap();
        assert!(
            !packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::Magic { cast: true, .. }
                    | ServerPacket::ObjectMagic { cast: true, .. }
            )),
            "{status} must reject an ordinary cast: {packets:?}"
        );
        assert_eq!(
            runtime
                .zone_state
                .lock()
                .unwrap()
                .zone_manager
                .player_vitals(&sid)
                .unwrap()
                .2,
            before_mp,
            "rejected {status} casting must not spend MP"
        );
        // Clearing the finite personal status must immediately restore casting;
        // a rejected packet cannot leave a hidden shared spell deadline.
        let mut save = runtime.inner.active_character_checkpoint().unwrap();
        save.buff_states_json.clear();
        runtime
            .inner
            .restore_active_character_checkpoint(&save)
            .unwrap();
        let accepted = runtime.execute(fireball_command(&runtime)).unwrap();
        assert!(
            accepted.iter().any(|packet| matches!(
                packet,
                ServerPacket::Magic {
                    spell: Spell::FireBall,
                    cast: true,
                    ..
                }
            )),
            "rejection must not consume the spell deadline: {accepted:?}"
        );
    }
}
