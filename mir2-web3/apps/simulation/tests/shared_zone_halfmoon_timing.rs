use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey,
    ZoneMonsterDefense, ZoneMonsterSpawn, ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};

fn owner() -> SessionId {
    SessionId::new("halfmoon-owner")
}

fn zone() -> ZoneRuntime {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: owner(),
        account_id: "halfmoon-account".into(),
        character_index: 0,
        object_id: 101,
        name: "Warrior".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 28,
        hp: 500,
        max_hp: 500,
        mp: 100,
        map_file_name: "0".into(),
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: 40,
            max_dc: 40,
            accuracy: 100,
            ..Default::default()
        },
    }));
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
    for (object_id, position) in [
        (201, Point { x: 11, y: 10 }),
        (202, Point { x: 11, y: 9 }),
        (203, Point { x: 11, y: 11 }),
    ] {
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: owner(),
            monster: ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id,
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
                position,
                direction: MirDirection::Left,
                defense: ZoneMonsterDefense::default(),
                respawn: None,
                drops: Vec::new(),
            },
            now_ms: 0,
        });
    }
    zone
}

fn hits(events: &[ZoneOutbound], target_id: u32) -> Vec<i32> {
    events
        .iter()
        .flat_map(|event| match event {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.as_slice(),
            _ => &[],
        })
        .filter_map(|packet| match packet {
            ServerPacket::DamageIndicator {
                object_id, damage, ..
            } if *object_id == target_id => Some(*damage),
            _ => None,
        })
        .collect()
}

#[test]
fn halfmoon_side_hits_keep_the_same_delay_regardless_of_selected_target() {
    for spell in [Spell::HalfMoon, Spell::CrossHalfMoon] {
        for selected in [201, 202] {
            let mut zone = zone();
            let mut immediate = zone.handle(ZoneCommand::PlayerAttackObject {
                session_id: owner(),
                object_id: selected,
                direction: MirDirection::Right,
                spell: spell as u8,
                level: 0,
                attack_type: 0,
                damage: 40,
                now_ms: 10,
            });
            immediate.extend(zone.tick(10));
            assert_eq!(
                hits(&immediate, 201),
                [40],
                "{spell:?}, selected={selected}"
            );
            let before_impact = zone.tick(309);
            for target in [202, 203] {
                assert!(
                    hits(&immediate, target).is_empty(),
                    "{spell:?}, selected={selected}"
                );
                assert!(hits(&before_impact, target).is_empty());
            }
            let impact = zone.tick(310);
            let secondary_damage = if spell == Spell::HalfMoon { 12 } else { 16 };
            for target in [202, 203] {
                assert_eq!(
                    hits(&impact, target),
                    [secondary_damage],
                    "{spell:?}, selected={selected}"
                );
            }
            assert!(hits(&impact, 201).is_empty());
            assert!(hits(&zone.tick(311), selected).is_empty());
        }
    }
}
