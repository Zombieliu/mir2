use super::*;

fn adjacent_players() -> (
    SharedInProcessZoneSessionRuntime,
    SharedInProcessZoneSessionRuntime,
) {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut attacker = shared_session_runtime(shared.clone());
    let mut target = shared_session_runtime(shared);
    start_new_runtime(&mut attacker, "mode-attacker", "ModeBlade");
    start_new_runtime(&mut target, "mode-target", "ModeTarget");
    equip_runtime_crystal_items_for_class(
        &mut attacker,
        MirClass::Warrior,
        &[
            (
                "BloodStealerSword",
                mir2_simulation::EquipmentSlot::Weapon,
                1,
            ),
            (
                "DragonPendant5",
                mir2_simulation::EquipmentSlot::Necklace,
                1,
            ),
            (
                "SharpBracelet",
                mir2_simulation::EquipmentSlot::BraceletLeft,
                1,
            ),
            (
                "SurvivalBracelet",
                mir2_simulation::EquipmentSlot::BraceletRight,
                1,
            ),
        ],
    );
    // Trusted fixture placement only; mode changes and attacks below use the
    // same public ClientPackets as the browser instead of WorldCommand::Attack.
    for (runtime, location) in [(&mut attacker, "100:100"), (&mut target, "101:100")] {
        runtime
            .execute(WorldCommand::TransferMap {
                key: format!("crystal:0:{location}"),
            })
            .expect("PvP fixture placement should execute");
    }
    target
        .inner
        .force_authoritative_player_vitals_with_max_hp(Some(10_000), Some(10_000), None);
    target.sync_zone_snapshot();
    let attacker_stats = attacker.inner.zone_player_combat_stats();
    let target_stats = target.inner.zone_player_combat_stats();
    assert!(attacker_stats.min_dc > 0);
    assert!(attacker_stats.accuracy >= target_stats.agility);
    assert!(!attacker.world_snapshot().in_safe_zone);
    assert!(!target.world_snapshot().in_safe_zone);
    (attacker, target)
}

fn set_mode(attacker: &mut SharedInProcessZoneSessionRuntime, mode: u8) {
    let packets = attacker
        .execute(WorldCommand::ClientPacket(ClientPacket::ChangeAMode {
            mode,
        }))
        .expect("public attack-mode packet should execute");
    assert!(packets.iter().any(
        |packet| matches!(packet, ServerPacket::ChangeAMode { mode: actual } if *actual == mode)
    ));
    let session = attacker.current_zone_session_id().unwrap();
    let state = attacker.zone_state.lock().unwrap();
    let profile = state
        .zone_manager
        .zone(&ZoneKey::for_map("0"))
        .unwrap()
        .player_chat_profile(&session)
        .unwrap();
    assert_eq!(profile.attack_mode, mode, "mode must reach shared Zone");
}

fn attack_packet() -> WorldCommand {
    WorldCommand::ClientPacket(ClientPacket::Attack {
        direction: MirDirection::Right,
        spell: Spell::None,
    })
}

fn drain(target: &mut SharedInProcessZoneSessionRuntime) -> Vec<ServerPacket> {
    target
        .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
            time: 1,
        }))
        .expect("observer should drain shared packets")
}

fn assert_blocked(
    attacker: &mut SharedInProcessZoneSessionRuntime,
    target: &mut SharedInProcessZoneSessionRuntime,
) {
    let hp_before = target.world_snapshot().player_hp;
    let owner_packets = attacker.execute(attack_packet()).unwrap();
    let observer_packets = drain(target);
    assert_eq!(target.world_snapshot().player_hp, hp_before);
    for packets in [&owner_packets, &observer_packets] {
        assert!(
            !packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::ObjectAttack { .. }
                    | ServerPacket::ObjectStruck { .. }
                    | ServerPacket::DamageIndicator { .. }
            )),
            "rejected PvP must not broadcast an attack/hit: {packets:?}"
        );
    }
}

#[test]
fn public_all_mode_directional_attack_damages_remote_player_and_delivers_both_hits() {
    let (mut attacker, mut target) = adjacent_players();
    set_mode(&mut attacker, 5);
    let target_id = attacker
        .world_snapshot()
        .entities
        .iter()
        .find(|entity| entity.kind == WorldEntityKind::Player && entity.name == "ModeTarget")
        .unwrap()
        .object_id;
    let target_local_id = target.world_snapshot().player_object_id.unwrap();
    let resolved = attacker
        .prepare_zone_native_player_attack(&attack_packet())
        .unwrap();
    assert!(resolved.is_player_target);
    assert_eq!(resolved.object_id, target_id);
    let _ = drain(&mut target);
    let hp_before = target.world_snapshot().player_hp.unwrap();
    let owner_packets = attacker.execute(attack_packet()).unwrap();
    let observer_packets = drain(&mut target);
    assert!(target.world_snapshot().player_hp.unwrap() < hp_before);
    assert!(owner_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })));
    assert!(owner_packets.iter().any(|packet| matches!(packet,
        ServerPacket::ObjectStruck { info } if info.object_id == target_id)));
    assert!(observer_packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })));
    assert!(observer_packets.iter().any(|packet| matches!(packet,
        ServerPacket::ObjectStruck { info } if info.object_id == target_local_id)));
}

#[test]
fn public_switch_from_all_to_peace_rejects_directional_player_attack() {
    let (mut attacker, mut target) = adjacent_players();
    set_mode(&mut attacker, 5);
    set_mode(&mut attacker, 0);
    // This is the first attack in this session, so a cooldown cannot explain
    // the rejection after the mode changes.
    assert_blocked(&mut attacker, &mut target);
}

#[test]
fn public_all_mode_directional_attack_rejects_either_players_safe_zone() {
    let map = mir2_game_data::crystal_map_respawns_ref("0").expect("Bichon map geometry");
    let safe_zone = map
        .safe_zones
        .iter()
        .find(|zone| zone.start_point)
        .expect("Bichon starting safe zone");
    let assert_safety = |runtime: &SharedInProcessZoneSessionRuntime, expected| {
        assert_eq!(runtime.inner.world_snapshot().in_safe_zone, expected);
        assert_eq!(runtime.world_snapshot().in_safe_zone, expected);
        let session = runtime.current_zone_session_id().unwrap();
        let state = runtime.zone_state.lock().unwrap();
        assert_eq!(
            state
                .zone_manager
                .zone(&ZoneKey::for_map("0"))
                .unwrap()
                .player_chat_profile(&session)
                .unwrap()
                .in_safe_zone,
            expected,
            "personal geometry and authoritative combat profile must agree"
        );
    };
    for protect_attacker in [true, false] {
        let (mut attacker, mut target) = adjacent_players();
        // Trusted placement at the actual imported safe-area boundary. The
        // old fixture set only a chat flag while both players stood outside;
        // a normal authoritative refresh correctly replaced that false input.
        // Keep the target one tile to the right in both cases, with exactly
        // one player inside, so range/mode cannot explain the rejection.
        let edge = if protect_attacker {
            safe_zone.location.x + i32::from(safe_zone.size)
        } else {
            safe_zone.location.x - i32::from(safe_zone.size) - 1
        };
        for (runtime, x) in [(&mut attacker, edge), (&mut target, edge + 1)] {
            runtime
                .execute(WorldCommand::TransferMap {
                    key: format!("crystal:0:{x}:{}", safe_zone.location.y),
                })
                .expect("real safe-boundary fixture placement");
        }
        set_mode(&mut attacker, 5);
        assert_safety(&attacker, protect_attacker);
        assert_safety(&target, !protect_attacker);
        assert!(attacker
            .prepare_zone_native_player_attack(&attack_packet())
            .expect("real adjacent player must resolve before safe-area policy")
            .is_player_target);
        assert_blocked(&mut attacker, &mut target);
        assert_safety(&attacker, protect_attacker);
        assert_safety(&target, !protect_attacker);
    }
}
