use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, SimulationConfig, SimulationSession, WorldEntityDisposition, ZoneCollision,
    ZoneCommand, ZoneJoin, ZoneKey, ZoneManager, ZoneMonsterDefense, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};

fn owner() -> SessionId {
    SessionId::new("thrust-owner")
}

fn player(id: u32, position: Point) -> ZoneJoin {
    ZoneJoin {
        session_id: if id == 101 {
            owner()
        } else {
            SessionId::new(format!("target-{id}"))
        },
        account_id: format!("account-{id}"),
        character_index: 0,
        object_id: id,
        name: format!("Warrior{id}"),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 28,
        hp: 500,
        max_hp: 500,
        mp: 100,
        map_file_name: "0".into(),
        position,
        direction: MirDirection::Right,
        chat_profile: mir2_simulation::ZoneChatProfile {
            attack_mode: 5,
            ..Default::default()
        },
        combat_stats: ZonePlayerCombatStats {
            min_dc: 40,
            max_dc: 40,
            accuracy: 100,
            min_ac: 16,
            max_ac: 16,
            ..Default::default()
        },
    }
}

fn zone() -> ZoneRuntime {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    zone.handle(ZoneCommand::Join(player(101, Point { x: 10, y: 10 })));
    zone.handle(ZoneCommand::sync_player_combat_state(
        owner(),
        MirClass::Warrior,
        true,
        false,
        true,
        false,
        false,
        false,
    ));
    zone
}

fn monster(id: u32, x: i32, y: i32) -> ZoneMonsterSpawn {
    ZoneMonsterSpawn {
        crystal_drop_seed: None,
        object_id: id,
        name: "Scarecrow".into(),
        name_colour_argb: -1,
        image: 5,
        ai: 0,
        disposition: Some(WorldEntityDisposition::Hostile),
        level: 1,
        max_hp: 500,
        hp: 500,
        experience: 1,
        move_speed_ms: 600,
        attack_speed_ms: 1200,
        friendly_guild: None,
        position: Point { x, y },
        direction: MirDirection::Left,
        defense: ZoneMonsterDefense {
            min_ac: 16,
            max_ac: 16,
            ..Default::default()
        },
        respawn: None,
        drops: Vec::new(),
    }
}

fn spawn(zone: &mut ZoneRuntime, target: ZoneMonsterSpawn) {
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner(),
        monster: target,
        now_ms: 0,
    });
}

fn attack(zone: &mut ZoneRuntime, id: u32, spell: Spell, level: u8) -> Vec<ZoneOutbound> {
    let mut result = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: owner(),
        object_id: id,
        direction: MirDirection::Right,
        spell: spell as u8,
        level,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    result.extend(zone.tick(10));
    result.extend(zone.tick(310));
    result
}

fn hits(outbounds: &[ZoneOutbound], id: u32) -> Vec<i32> {
    outbounds
        .iter()
        .flat_map(|outbound| match outbound {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.as_slice(),
            _ => &[],
        })
        .filter_map(|packet| match packet {
            ServerPacket::DamageIndicator {
                object_id, damage, ..
            } if *object_id == id => Some(*damage),
            _ => None,
        })
        .collect()
}

#[test]
fn thrusting_front_is_full_weapon_damage_and_second_tile_bypasses_armour() {
    for selected in [201, 202] {
        for (level, secondary_damage) in [(0, 10), (1, 20), (2, 30), (3, 40)] {
            let mut zone = zone();
            spawn(&mut zone, monster(201, 11, 10));
            spawn(&mut zone, monster(202, 12, 10));
            spawn(&mut zone, monster(203, 12, 11));
            let events = attack(&mut zone, selected, Spell::Thrusting, level);
            assert_eq!(
                hits(&events, 201),
                [24],
                "front cell, selected={selected}, level={level}"
            );
            assert_eq!(
                hits(&events, 202),
                [secondary_damage],
                "second cell, selected={selected}, level={level}"
            );
            assert!(
                hits(&events, 203).is_empty(),
                "off-axis cell must not be hit"
            );
        }
    }
}

#[test]
fn thrusting_rejects_off_axis_targets_and_does_not_turn_square_range_into_reach() {
    let mut zone = zone();
    spawn(&mut zone, monster(201, 12, 11));
    let events = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: owner(),
        object_id: 201,
        direction: MirDirection::Right,
        spell: Spell::Thrusting as u8,
        level: 0,
        attack_type: 0,
        damage: 999,
        now_ms: 10,
    });
    assert!(hits(&events, 201).is_empty());
    // Rejection must not consume the swing's cooldown.
    spawn(&mut zone, monster(202, 11, 10));
    let events = attack(&mut zone, 202, Spell::Thrusting, 0);
    assert_eq!(hits(&events, 202), [24]);
}

#[test]
fn thrusting_materialized_target_uses_the_same_first_and_second_cell_rules() {
    for (x, expected) in [(11, 24), (12, 10)] {
        let mut zone = zone();
        let mut events = zone.handle(ZoneCommand::PlayerAttackMaterializedObject {
            session_id: owner(),
            object_id: 201,
            monster: Some(monster(201, x, 10)),
            direction: MirDirection::Right,
            spell: Spell::Thrusting as u8,
            level: 0,
            attack_type: 0,
            damage: 999,
            now_ms: 10,
        });
        events.extend(zone.tick(10));
        assert_eq!(hits(&events, 201), [expected]);
    }
}

#[test]
fn thrusting_pvp_and_mixed_monster_player_cells_share_the_damage_rules() {
    for selected in [201, 202] {
        let mut zone = zone();
        zone.handle(ZoneCommand::Join(player(201, Point { x: 11, y: 10 })));
        zone.handle(ZoneCommand::Join(player(202, Point { x: 12, y: 10 })));
        let events = attack(&mut zone, selected, Spell::Thrusting, 0);
        assert_eq!(hits(&events, 201), [24]);
        assert_eq!(hits(&events, 202), [10]);

        let mut zone = self::zone();
        spawn(&mut zone, monster(201, 11, 10));
        zone.handle(ZoneCommand::Join(player(202, Point { x: 12, y: 10 })));
        let events = attack(&mut zone, selected, Spell::Thrusting, 0);
        assert_eq!(hits(&events, 201), [24]);
        assert_eq!(hits(&events, 202), [10]);
    }
}

#[test]
fn halfmoon_keeps_full_front_damage_and_only_scales_original_three_extra_cells() {
    let mut zone = zone();
    for (id, x, y) in [
        (201, 11, 10),
        (202, 11, 9),
        (203, 11, 11),
        (204, 10, 11),
        (205, 9, 11),
    ] {
        spawn(&mut zone, monster(id, x, y));
    }
    let events = attack(&mut zone, 201, Spell::HalfMoon, 0);
    assert_eq!(hits(&events, 201), [24]);
    for id in [202, 203, 204] {
        assert_eq!(hits(&events, id), [12]);
    }
    assert!(hits(&events, 205).is_empty());
}

#[test]
fn thrusting_secondary_formula_truncates_and_primary_armour_can_block_completely() {
    let mut zone = zone();
    zone.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: owner(),
        stats: ZonePlayerCombatStats {
            min_dc: 27,
            max_dc: 27,
            accuracy: 100,
            ..Default::default()
        },
    });
    let mut front = monster(201, 11, 10);
    front.defense.min_ac = 99;
    front.defense.max_ac = 99;
    let mut second = monster(202, 12, 10);
    second.defense.min_ac = 99;
    second.defense.max_ac = 99;
    spawn(&mut zone, front);
    spawn(&mut zone, second);
    let events = attack(&mut zone, 201, Spell::Thrusting, 0);
    assert!(hits(&events, 201).iter().all(|damage| *damage == 0));
    assert_eq!(hits(&events, 202), [6]);
}

fn session_with_weapon_skills() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let mut save = session
        .active_character_checkpoint()
        .expect("active character");
    save.character.level = 28;
    save.skill_states_json = ["thrusting", "halfmoon", "slaying"]
        .into_iter()
        .map(|key| {
            serde_json::json!({
                "key": key, "name": key, "description": "", "level": 0, "experience": 0,
                "cooldown_ticks": 0, "cooldown_ends_at": 0
            })
            .to_string()
        })
        .collect();
    session
        .restore_active_character_checkpoint(&save)
        .expect("skill fixture");
    session
}

#[test]
fn accepted_front_weapon_skill_progresses_and_second_only_thrust_does_not() {
    let mut session = session_with_weapon_skills();
    let no_primary = session.commit_zone_melee_attack_spell_with_primary(Spell::Thrusting, false);
    assert!(!no_primary.iter().any(|p| matches!(
        p,
        ServerPacket::MagicLeveled {
            spell: Spell::Thrusting,
            ..
        }
    )));
    for spell in [Spell::Thrusting, Spell::HalfMoon, Spell::Slaying] {
        let packets = session.commit_zone_melee_attack_spell_with_primary(spell, true);
        assert_eq!(
            packets
                .iter()
                .filter(
                    |p| matches!(p, ServerPacket::MagicLeveled { spell: actual, experience, .. }
            if *actual == spell && *experience > 0)
                )
                .count(),
            1,
            "{spell:?}: {packets:?}"
        );
    }
    let saved = session
        .active_character_checkpoint()
        .expect("trained checkpoint");
    session
        .restore_active_character_checkpoint(&saved)
        .expect("saved progress");
    assert_eq!(
        session
            .active_character_checkpoint()
            .unwrap()
            .skill_states_json,
        saved.skill_states_json
    );
}

#[test]
fn primary_query_does_not_train_on_friendly_unknown_or_hidden_front_occupants() {
    for (disposition, ai) in [
        (Some(WorldEntityDisposition::Friendly), 0),
        (Some(WorldEntityDisposition::Neutral), 0),
        (None, 0),
        (Some(WorldEntityDisposition::Hostile), 5),
    ] {
        let mut zone = zone();
        let mut front = monster(201, 11, 10);
        front.disposition = disposition;
        front.ai = ai;
        spawn(&mut zone, front);
        spawn(&mut zone, monster(202, 12, 10));
        let primary = zone.melee_primary_target_present(&owner(), MirDirection::Right, None);
        assert!(!primary, "disposition={disposition:?}, ai={ai}");
        assert_eq!(
            hits(&attack(&mut zone, 202, Spell::Thrusting, 0), 202),
            [10]
        );
    }

    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    let mut attacker = player(101, Point { x: 10, y: 10 });
    attacker.chat_profile.attack_mode = 1;
    attacker.chat_profile.group_members = vec!["Warrior201".into()];
    zone.handle(ZoneCommand::Join(attacker));
    zone.handle(ZoneCommand::Join(player(201, Point { x: 11, y: 10 })));
    zone.handle(ZoneCommand::sync_player_combat_state(
        owner(),
        MirClass::Warrior,
        true,
        false,
        true,
        false,
        false,
        false,
    ));
    spawn(&mut zone, monster(202, 12, 10));
    assert!(!zone.melee_primary_target_present(&owner(), MirDirection::Right, None));
    let events = attack(&mut zone, 202, Spell::Thrusting, 0);
    assert!(hits(&events, 201).is_empty());
    assert_eq!(hits(&events, 202), [10]);
}

#[test]
fn primary_query_accepts_only_new_legal_materialized_front_targets() {
    let mut zone = zone();
    let front = monster(201, 11, 10);
    assert!(zone.melee_primary_target_present(&owner(), MirDirection::Right, Some(&front)));
    for candidate in [
        monster(202, 12, 10),
        ZoneMonsterSpawn {
            hp: 0,
            ..front.clone()
        },
        ZoneMonsterSpawn {
            disposition: Some(WorldEntityDisposition::Friendly),
            ..front.clone()
        },
        ZoneMonsterSpawn {
            ai: 5,
            ..front.clone()
        },
    ] {
        assert!(!zone.melee_primary_target_present(
            &owner(),
            MirDirection::Right,
            Some(&candidate)
        ));
    }
    spawn(
        &mut zone,
        ZoneMonsterSpawn {
            hp: 0,
            ..front.clone()
        },
    );
    assert!(
        !zone.melee_primary_target_present(&owner(), MirDirection::Right, Some(&front)),
        "stale materialization must not replace the Zone's dead target"
    );

    let mut manager = ZoneManager::new();
    assert!(manager.install_empty_zone(ZoneRuntime::new_with_collision(
        ZoneKey::for_map("0"),
        ZoneCollision::unbounded()
    )));
    manager.join(player(101, Point { x: 10, y: 10 }));
    assert!(manager.melee_primary_target_present(&owner(), MirDirection::Right, Some(&front)));
    assert!(!manager.melee_primary_target_present(
        &SessionId::new("absent"),
        MirDirection::Right,
        Some(&front)
    ));
}

#[test]
fn lethal_primary_capture_still_awards_accepted_weapon_skill_practice() {
    let mut zone = zone();
    spawn(
        &mut zone,
        ZoneMonsterSpawn {
            hp: 1,
            ..monster(201, 11, 10)
        },
    );
    let primary = zone.melee_primary_target_present(&owner(), MirDirection::Right, None);
    assert!(primary);
    let events = attack(&mut zone, 201, Spell::Thrusting, 0);
    assert_eq!(hits(&events, 201), [1]);
    assert!(!zone.melee_primary_target_present(&owner(), MirDirection::Right, None));
    let mut session = session_with_weapon_skills();
    let packets = session.commit_zone_melee_attack_spell_with_primary(Spell::Thrusting, primary);
    assert!(
        packets.iter().any(|p| matches!(p,
        ServerPacket::MagicLeveled { spell: Spell::Thrusting, experience, .. } if *experience > 0))
    );
}
