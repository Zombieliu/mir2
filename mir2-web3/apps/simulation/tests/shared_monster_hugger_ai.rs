//! Crystal AI69/70 shared explosion timing. ArcherGuard is an explicit nonzero stat fixture.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    GroundDropLootSnapshot, GroundDropSnapshot, SessionId, WorldEntityDisposition, ZoneCommand,
    ZoneJoin, ZoneKey, ZoneMonsterSpawn, ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
const ID: u32 = 9157;
fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn fixture(ai: u8, name: &str, player_x: i32, monster_id: u32) -> ZoneRuntime {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("hugger-fixture"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("player"),
        account_id: "test".into(),
        character_index: 1,
        object_id: 101,
        name: "red".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 10000,
        max_hp: 10000,
        mp: 100,
        map_file_name: "hugger-fixture".into(),
        position: point(player_x, 20),
        direction: MirDirection::Left,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    }));
    let mut profile = z.player_chat_profile(&SessionId::new("player")).unwrap();
    profile.pk_points = 0;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("player"),
        profile,
    });
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("player"),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            object_id: monster_id,
            name: name.into(),
            name_colour_argb: -1,
            image: 139,
            ai,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 0,
            max_hp: 9999,
            hp: 9999,
            experience: 100,
            move_speed_ms: 300,
            attack_speed_ms: 2000,
            friendly_guild: None,
            position: point(20, 20),
            direction: MirDirection::Down,
            defense: Default::default(),
            respawn: None,
            drops: vec![GroundDropSnapshot {
                object_id: 0,
                name: "Gold".into(),
                name_colour_argb: -1,
                icon: 0,
                x: 0,
                y: 0,
                quantity: 1,
                source_monster: name.into(),
                owner_object_id: None,
                ownership_remaining_ticks: None,
                loot: GroundDropLootSnapshot::Gold { amount: 7 },
            }],
        },
    });
    z
}
fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.as_slice(),
            _ => &[],
        })
        .collect()
}
fn hp(z: &ZoneRuntime) -> i32 {
    z.player_vitals(&SessionId::new("player")).unwrap().0
}
fn walk(z: &mut ZoneRuntime, direction: MirDirection, now: u64) {
    z.handle(ZoneCommand::Walk {
        session_id: SessionId::new("player"),
        direction,
        seq: 1,
        now_ms: now,
    });
    z.tick(now);
}

#[test]
fn poison_hugger_real_template_does_not_invent_damage_from_zero_dc() {
    let mut z = fixture(69, "PoisonHugger", 21, ID);
    let death = z.tick(2001);
    assert!(packets(&death)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectDied{info} if info.object_id==ID)));
    let hit = z.tick(2501);
    assert_eq!(hp(&z), 10000);
    assert!(!packets(&hit)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectPoisoned{poison,..} if poison&1!=0)));
}

#[test]
fn poison_hugger_captures_victims_on_death_and_restores_delayed_damage() {
    let mut z = fixture(69, "ArcherGuard", 21, ID);
    for (session, id, x) in [("near", 102, 23), ("far", 103, 100)] {
        z.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new(session),
            account_id: session.into(),
            character_index: id as i32,
            object_id: id,
            name: session.into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 40,
            hp: 10000,
            max_hp: 10000,
            mp: 100,
            map_file_name: "hugger-fixture".into(),
            position: point(x, 20),
            direction: MirDirection::Left,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
    }
    let death = z.tick(2001);
    let death_recipients: Vec<_> = death
        .iter()
        .filter_map(|o| match o {
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if packets
                .iter()
                .any(|p| matches!(p,ServerPacket::ObjectDied{info} if info.object_id==ID)) =>
            {
                Some(session_ids)
            }
            _ => None,
        })
        .flatten()
        .collect();
    assert!(death_recipients.contains(&&SessionId::new("near")));
    assert!(!death_recipients.contains(&&SessionId::new("far")));
    walk(&mut z, MirDirection::Right, 2200);
    assert_eq!(
        z.player_position(&SessionId::new("player")),
        Some(point(22, 20))
    );
    let mut z = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    z.tick(2500);
    assert_eq!(hp(&z), 10000);
    z.tick(2501);
    assert_eq!(
        hp(&z),
        9745,
        "AI69 retains the captured victim outside blast radius"
    );
    let repeated = z.tick(2501);
    assert!(
        !packets(&repeated)
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectStruck{info} if info.object_id==101)),
        "checkpointed explosion action is consumed once; poison has its own clock"
    );
}

#[test]
fn hugger_scans_at_completion_and_clears_old_player_life() {
    let mut z = fixture(70, "ArcherGuard", 22, ID);
    z.tick(300001);
    assert!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == ID)
            .unwrap()
            .dead
    );
    walk(&mut z, MirDirection::Left, 300100);
    z.tick(300501);
    assert_eq!(hp(&z), 9745, "AI70 finds a victim who entered after death");
    let mut z = fixture(69, "ArcherGuard", 21, ID);
    z.tick(2001);
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("player"),
    });
    let result = z.tick(2501);
    assert!(!packets(&result)
        .iter()
        .any(|p| matches!(p, ServerPacket::DamageIndicator { object_id: 101, .. })));
}

#[test]
fn hugger_successful_explosion_applies_real_green_status_and_expires_it() {
    let mut witnessed = false;
    for id in ID..ID + 32 {
        let mut z = fixture(69, "ArcherGuard", 21, id);
        z.tick(2001);
        let hit = z.tick(2501);
        if packets(&hit)
            .iter()
            .any(|p| matches!(p,ServerPacket::ObjectPoisoned{object_id:101,poison} if poison&1!=0))
        {
            witnessed = true;
            for (index, now) in [2502, 4503, 6504, 8505].into_iter().enumerate() {
                z.tick(now);
                assert_eq!(
                    hp(&z),
                    9745 - 255 * (index as i32 + 1),
                    "SC poison is real damage and keeps the source cadence"
                );
            }
            let mut expired = z.tick(10506);
            assert_eq!(hp(&z), 8470, "five SC ticks, not a wall-clock tint only");
            expired.extend(z.tick(10507));
            assert!(packets(&expired).iter().any(
                |p| matches!(p,ServerPacket::ObjectPoisoned{object_id:101,poison} if poison&1==0)
            ));
            break;
        }
    }
    assert!(witnessed, "seeded 1/5 poison path must be exercised");
}

#[test]
fn hugger_tagged_suicide_awards_once_and_never_rewards_expired_or_offline_owner() {
    for mode in ["live", "expired", "offline", "rejoined"] {
        let mut z = fixture(69, "PoisonHugger", 21, ID);
        z.handle(ZoneCommand::sync_player_combat_state(
            SessionId::new("player"),
            MirClass::Warrior,
            false,
            false,
            true,
            false,
            false,
            false,
        ));
        z.handle(ZoneCommand::UpdatePlayerCombatStats {
            session_id: SessionId::new("player"),
            stats: ZonePlayerCombatStats {
                min_dc: 10,
                max_dc: 10,
                accuracy: 100,
                ..Default::default()
            },
        });
        z.handle(ZoneCommand::PlayerAttackObject {
            session_id: SessionId::new("player"),
            object_id: ID,
            direction: MirDirection::Left,
            spell: 0,
            level: 0,
            attack_type: 0,
            damage: 1,
            now_ms: 1,
        });
        z.tick(1);
        assert_eq!(
            z.native_monster_snapshots()
                .iter()
                .find(|m| m.object_id == ID)
                .unwrap()
                .hp,
            9989,
            "tag requires a real nonlethal hit"
        );
        if mode == "offline" || mode == "rejoined" {
            z.handle(ZoneCommand::Leave {
                session_id: SessionId::new("player"),
            });
        }
        if mode == "rejoined" {
            z.handle(ZoneCommand::Join(ZoneJoin {
                session_id: SessionId::new("player"),
                account_id: "test".into(),
                character_index: 101,
                object_id: 101,
                name: "new life".into(),
                class: MirClass::Warrior,
                gender: MirGender::Male,
                level: 40,
                hp: 10000,
                max_hp: 10000,
                mp: 100,
                map_file_name: "hugger-fixture".into(),
                position: point(21, 20),
                direction: MirDirection::Left,
                chat_profile: Default::default(),
                combat_stats: Default::default(),
            }));
        }
        let death_time = if mode == "expired" { 6002 } else { 2001 };
        let death = z.tick(death_time);
        let awards: Vec<_> = death
            .iter()
            .filter_map(|o| match o {
                ZoneOutbound::MonsterKillAward { award, .. } => Some(award),
                _ => None,
            })
            .collect();
        assert_eq!(awards.len(), usize::from(mode == "live"));
        if let Some(award) = awards.first() {
            assert_eq!(award.experience, 100);
            assert_eq!(award.drops.len(), 1);
            assert!(award.drops[0].object_id > 0);
        }
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        let later = restored.tick(death_time + 500);
        assert!(
            !later
                .iter()
                .any(|o| matches!(o, ZoneOutbound::MonsterKillAward { .. })),
            "death explosion does not issue the award twice"
        );
    }
}

#[test]
fn hugger_max_poison_resistance_blocks_green_without_blocking_blast_damage() {
    for id in ID..ID + 16 {
        let mut z = fixture(69, "ArcherGuard", 21, id);
        z.handle(ZoneCommand::UpdatePlayerCombatStats {
            session_id: SessionId::new("player"),
            stats: ZonePlayerCombatStats {
                poison_resist: 10,
                ..Default::default()
            },
        });
        z.tick(2001);
        let out = z.tick(2501);
        assert_eq!(hp(&z), 9745);
        assert!(!packets(&out).iter().any(
            |p| matches!(p,ServerPacket::ObjectPoisoned{object_id:101,poison} if poison&1!=0)
        ));
    }
}

fn owned_hugger_fixture(ai: u8) -> (ZoneRuntime, u32, Point) {
    owned_hugger_fixture_at(ai, 1500)
}
fn owned_hugger_fixture_at(ai: u8, spawn_at: u64) -> (ZoneRuntime, u32, Point) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("hugger-owned-fixture"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("owner"),
        account_id: "owner".into(),
        character_index: 1,
        object_id: 101,
        name: "owner".into(),
        class: MirClass::Archer,
        gender: MirGender::Male,
        level: 60,
        hp: 100000,
        max_hp: 100000,
        mp: 100,
        map_file_name: "hugger-owned-fixture".into(),
        position: point(20, 24),
        direction: MirDirection::Up,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats {
            min_dc: 100000,
            max_dc: 100000,
            accuracy: 255,
            ..Default::default()
        },
    }));
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: mir2_protocol::Spell::SummonToad,
        direction: MirDirection::Up,
        target: point(20, 20),
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 0,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    let born = z.tick(1500);
    let (pet, pos) = packets(&born)
        .into_iter()
        .find_map(|p| match p {
            ServerPacket::ObjectMonster { info } if info.name == "SpittingToad" => {
                Some((info.object_id, info.location.clone()))
            }
            _ => None,
        })
        .expect("public owned toad");
    z.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("owner"),
        position: point(pos.x + 10, pos.y),
        direction: MirDirection::Up,
    });
    assert!(
        z.spawn_world_event_monster(
            &ZoneMonsterSpawn {
                object_id: ID,
                name: "Guard".into(),
                name_colour_argb: -1,
                image: 5,
                ai,
                disposition: Some(WorldEntityDisposition::Hostile),
                level: 60,
                hp: 10000,
                max_hp: 10000,
                experience: 0,
                move_speed_ms: 500,
                attack_speed_ms: 5000,
                friendly_guild: None,
                position: point(pos.x, pos.y - 1),
                direction: MirDirection::Down,
                defense: Default::default(),
                respawn: None,
                drops: Vec::new()
            },
            spawn_at
        )
        .0
    );
    (z, pet, pos)
}
fn native_state(z: &ZoneRuntime, id: u32) -> serde_json::Value {
    let value: serde_json::Value = serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap();
    value["native_monsters"][id.to_string()].clone()
}
#[test]
fn poison_hugger_captures_actual_owned_monster_and_finishes_after_500ms() {
    let (mut z, pet, _) = owned_hugger_fixture(69);
    let hp = native_state(&z, pet)["hp"].as_i64().unwrap();
    z.tick(3501);
    assert_eq!(native_state(&z, ID)["dead"], true);
    assert!(native_state(&z, ID)["special_ai"]["hugger"]["hits"]
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["target"] == pet && h["session"].is_null()));
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(4000), restored.tick(4000));
    assert_eq!(native_state(&z, pet)["hp"], hp);
    assert_eq!(z.tick(4001), restored.tick(4001));
    assert!(native_state(&z, pet)["hp"].as_i64().unwrap() < hp);
}
#[test]
fn hugger_keeps_a_real_owned_target_and_uses_300ms_melee() {
    let (mut z, pet, _) = owned_hugger_fixture(70);
    let hp = native_state(&z, pet)["hp"].as_i64().unwrap();
    let out = z.tick(3501);
    assert_eq!(native_state(&z, ID)["dead"], false);
    assert!(packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==ID)));
    assert_eq!(native_state(&z, pet)["hp"], hp);
    z.tick(3800);
    assert_eq!(native_state(&z, pet)["hp"], hp);
    z.tick(3801);
    assert!(native_state(&z, pet)["hp"].as_i64().unwrap() < hp);
}
#[test]
fn poison_hugger_retired_owned_target_does_not_receive_captured_blast() {
    let (mut z, pet, _) = owned_hugger_fixture(69);
    z.tick(3501);
    z.despawn_world_event_monster(pet, 3600);
    let out = z.tick(4001);
    assert!(!packets(&out)
        .iter()
        .any(|p| matches!(p,ServerPacket::DamageIndicator{object_id,..}if *object_id==pet)));
}

#[test]
fn hugger_blocked_first_player_stops_later_owned_monster_blast() {
    let (mut z, pet, pos) = owned_hugger_fixture(70);
    z.handle(ZoneCommand::SyncPlayerTransform {
        session_id: SessionId::new("owner"),
        position: point(pos.x, pos.y - 2),
        direction: MirDirection::Down,
    });
    z.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("owner"),
        MirClass::Archer,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    z.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("owner"),
        stats: ZonePlayerCombatStats {
            min_dc: 100000,
            max_dc: 100000,
            accuracy: 255,
            min_ac: 100000,
            max_ac: 100000,
            ..Default::default()
        },
    });
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("owner"),
        object_id: ID,
        direction: MirDirection::Down,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 3501,
    });
    z.tick(3501);
    assert_eq!(native_state(&z, ID)["dead"], true);
    let hp = native_state(&z, pet)["hp"].as_i64().unwrap();
    z.tick(4001);
    assert_eq!(native_state(&z,pet)["hp"],hp,"first ring target is the protected player above the source; return prevents later toad hit");
}
#[test]
fn hugger_green_death_clears_observer_mask_and_cannot_poison_reused_id() {
    let mut poisoned = None;
    // Vary only legal source spawn time; no modified RNG/checkpoint fixture.
    for spawn_at in 1500..1550 {
        let at = spawn_at + 2001;
        let (mut z, pet, pos) = owned_hugger_fixture_at(69, spawn_at);
        z.tick(at);
        z.tick(at + 500);
        let value = native_state(&z, pet);
        if value["dead"] == false && value["entity_poison"].as_u64().unwrap_or(0) & 1 != 0 {
            poisoned = Some((z, pet, pos, at + 500));
            break;
        }
    }
    let (mut z, pet, pos, start) =
        poisoned.expect("an admitted nonzero Green poison on a real owned monster");
    let mut death_output = None;
    for step in 1..=10 {
        let out = z.tick(start + step * 2001);
        if native_state(&z, pet)["dead"] == true {
            death_output = Some((out, start + step * 2001));
            break;
        }
    }
    let (out, dead_at) = death_output.expect("real periodic poison reaches native death");
    assert!(packets(&out).iter().any(
        |p| matches!(p,ServerPacket::ObjectPoisoned{object_id,poison}if *object_id==pet&&*poison==0)
    ));
    z.despawn_world_event_monster(pet, dead_at + 1);
    // New unowned neutral incarnation is a real producer spawn, not forged ownership.
    assert!(
        z.spawn_world_event_monster(
            &ZoneMonsterSpawn {
                object_id: pet,
                name: "Deer".into(),
                name_colour_argb: -1,
                image: 0,
                ai: 2,
                disposition: Some(WorldEntityDisposition::Neutral),
                level: 1,
                hp: 1000,
                max_hp: 1000,
                experience: 0,
                move_speed_ms: 100000,
                attack_speed_ms: 100000,
                friendly_guild: None,
                position: pos,
                direction: MirDirection::Down,
                defense: Default::default(),
                respawn: None,
                drops: Vec::new()
            },
            dead_at + 1
        )
        .0
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(z.tick(dead_at + 2002), restored.tick(dead_at + 2002));
    assert_eq!(native_state(&z, pet)["hp"], 1000);
    assert_eq!(
        native_state(&z, pet)["entity_poison"].as_u64().unwrap_or(0),
        0
    );
}
