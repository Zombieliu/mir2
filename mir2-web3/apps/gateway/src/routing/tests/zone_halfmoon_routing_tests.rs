use super::super::point_in_direction;
use super::*;

fn armed_runtime(spell: Spell) -> SharedInProcessZoneSessionRuntime {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared);
    start_new_runtime(&mut runtime, "halfmoon-routing", "MoonBlade");
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.level = 60;
    save.skill_states_json = vec![serde_json::json!({
        "key": format!("{spell:?}").to_ascii_lowercase(),
        "name": format!("{spell:?}"), "description": "", "level": 3,
        "experience": 0, "cooldown_ticks": 0, "cooldown_ends_at": 0
    })
    .to_string()];
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::SpellToggle {
            spell,
            toggle_state: 1,
        }))
        .unwrap();
    assert!(packets.iter().any(|packet| matches!(packet,
        ServerPacket::SpellToggle { spell: actual, can_use: true, .. } if *actual == spell)));
    runtime
}

fn shared_targets(runtime: &mut SharedInProcessZoneSessionRuntime, cells: &[(u32, i32, i32)]) {
    let key = runtime.current_presence_key().unwrap();
    let mut state = runtime.zone_state.lock().unwrap();
    let presence = state.players.get_mut(&key).unwrap();
    presence.entity.x = 1000;
    presence.entity.y = 1000;
    let map = state.maps.get_mut("0").unwrap();
    map.entities.clear();
    for &(id, x, y) in cells {
        let mut monster = shared_monster_entity(id);
        monster.x = x;
        monster.y = y;
        map.entities.insert(id, monster);
    }
}

fn directional_attack(spell: Spell) -> WorldCommand {
    WorldCommand::ClientPacket(ClientPacket::Attack {
        direction: MirDirection::Right,
        spell,
    })
}

#[test]
fn armed_halfmoon_directional_attack_acquires_side_only_target_before_plain_swing_fallback() {
    for spell in [Spell::HalfMoon, Spell::CrossHalfMoon] {
        for requested in [Spell::None, spell] {
            let mut runtime = armed_runtime(spell);
            shared_targets(&mut runtime, &[(260_901, 1000, 1001)]);
            let attack = runtime
                .prepare_zone_native_player_attack(&directional_attack(requested))
                .expect(
                    "an armed area swing must acquire its side target with an empty front cell",
                );
            assert_eq!(attack.object_id, 260_901);
            assert!(
                matches!(attack.kind, ZoneNativePlayerAttackKind::Melee { spell: actual, .. }
                if actual == spell as u8)
            );
        }
    }
}

#[test]
fn halfmoon_directional_target_search_preserves_primary_cell_priority() {
    let mut runtime = armed_runtime(Spell::HalfMoon);
    shared_targets(
        &mut runtime,
        &[(260_901, 1000, 1001), (260_902, 1001, 1000)],
    );
    let attack = runtime
        .prepare_zone_native_player_attack(&directional_attack(Spell::None))
        .unwrap();
    assert_eq!(attack.object_id, 260_902);
}

#[test]
fn halfmoon_does_not_acquire_rear_targets_while_cross_halfmoon_does() {
    for spell in [Spell::HalfMoon, Spell::CrossHalfMoon] {
        let mut runtime = armed_runtime(spell);
        shared_targets(&mut runtime, &[(260_901, 999, 1000)]);
        let attack = runtime.prepare_zone_native_player_attack(&directional_attack(Spell::None));
        if spell == Spell::HalfMoon {
            assert!(attack.is_none());
        } else {
            assert_eq!(attack.unwrap().object_id, 260_901);
        }
    }
}

#[test]
fn disabled_halfmoon_cannot_expand_an_untrusted_directional_attack() {
    let mut runtime = armed_runtime(Spell::HalfMoon);
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::SpellToggle {
            spell: Spell::HalfMoon,
            toggle_state: 0,
        }))
        .unwrap();
    shared_targets(&mut runtime, &[(260_901, 1000, 1001)]);
    assert!(runtime
        .prepare_zone_native_player_attack(&directional_attack(Spell::HalfMoon))
        .is_none());
}

#[test]
fn public_attack_with_empty_front_reaches_shared_halfmoon_impact() {
    let mut runtime = armed_runtime(Spell::HalfMoon);
    let (target, _) = prepare_gateway_range_fixture(&mut runtime, 1);
    let snapshot = runtime.world_snapshot();
    let origin = runtime
        .authoritative_self_entity_for_snapshot(&snapshot)
        .unwrap();
    assert_eq!((target.x, target.y), (origin.x + 1, origin.y));
    let origin = Point {
        x: origin.x,
        y: origin.y,
    };
    let direction = [
        MirDirection::Up,
        MirDirection::UpRight,
        MirDirection::DownRight,
    ]
    .into_iter()
    .find(|direction| {
        let front = point_in_direction(&origin, *direction);
        !snapshot.entities.iter().any(|entity| {
            !entity.dead
                && matches!(
                    entity.kind,
                    WorldEntityKind::Monster | WorldEntityKind::Player
                )
                && entity.x == front.x
                && entity.y == front.y
        })
    })
    .expect("fixture needs an empty front cell with the monster in the HalfMoon fan");
    let command = WorldCommand::ClientPacket(ClientPacket::Attack {
        direction,
        spell: Spell::None,
    });
    let attack = runtime.prepare_zone_native_player_attack(&command).unwrap();
    assert_eq!(attack.object_id, target.object_id);
    let packets = runtime.execute(command).unwrap();
    assert!(
        packets.iter().any(
            |packet| matches!(packet, ServerPacket::ObjectAttack { info }
        if info.spell == Spell::HalfMoon as u8)
        ),
        "{packets:?}"
    );
    let impact = runtime.dispatch_zone_player_command(
        ZoneCommand::Tick {
            now_ms: SharedInProcessZoneSessionRuntime::zone_now_ms() + 310,
        },
        false,
    );
    assert!(
        impact.iter().any(
            |packet| matches!(packet, ServerPacket::DamageIndicator { object_id, .. }
        if *object_id == target.object_id)
        ),
        "{impact:?}"
    );
}
