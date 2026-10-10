use super::*;
use mir2_protocol::MirGender;

// The bundled D024 terrain has the original line-hit and recall cells open.
const REAL_MAP: &str = "d024";

fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn join(name: &str, id: u32, position: Point) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(name),
        account_id: format!("account-{name}"),
        character_index: 1,
        object_id: id,
        name: name.into(),
        class: MirClass::Taoist,
        gender: MirGender::Male,
        level: 40,
        hp: 1000,
        max_hp: 1000,
        mp: 1000,
        map_file_name: "owned-pet-unit".into(),
        position,
        direction: MirDirection::Down,
        chat_profile: super::super::super::types::ZoneChatProfile {
            attack_mode: 5,
            ..Default::default()
        },
        combat_stats: Default::default(),
    }
}
fn summon(zone: &mut ZoneRuntime, session: &str, position: Point, spell: Spell, now: u64) -> u32 {
    let owner = zone.players[&SessionId::new(session)].object_id;
    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new(session),
        object_id: 0,
        spell,
        direction: MirDirection::Right,
        target: position,
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1000,
        now_ms: now,
    });
    zone.tick(now + 500);
    *zone
        .native_monsters
        .iter()
        .find(|(_, p)| {
            p.master_object_id == owner
                && !p.dead
                && p.name
                    == if spell == Spell::SummonShinsu {
                        "Shinsu"
                    } else {
                        "BoneFamiliar"
                    }
        })
        .unwrap()
        .0
}
fn fixture() -> (ZoneRuntime, u32, ZoneCombatEntityRef) {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("owned-pet-unit"));
    z.handle(ZoneCommand::Join(join("owner", 101, point(10, 20))));
    z.handle(ZoneCommand::Join(join("victim", 102, point(10, 21))));
    for name in ["owner", "victim"] {
        z.handle(ZoneCommand::sync_player_combat_state(
            SessionId::new(name),
            MirClass::Taoist,
            true,
            false,
            false,
            false,
            false,
            false,
        ));
    }
    let pet = summon(&mut z, "owner", point(11, 20), Spell::SummonSkeleton, 10);
    let victim = z
        .native_entity_player_ref(&SessionId::new("victim"))
        .unwrap();
    (z, pet, victim)
}
fn threaten_owner(z: &mut ZoneRuntime) {
    z.record_owned_pet_player_hit(102, &SessionId::new("owner"), 600);
}
fn set_mode(z: &mut ZoneRuntime, mode: u8) {
    z.players
        .get_mut(&SessionId::new("owner"))
        .unwrap()
        .chat_profile
        .attack_mode = mode;
}
fn can(z: &ZoneRuntime, pet: u32, target: &ZoneCombatEntityRef, now: u64) -> bool {
    z.owned_pet_can_attack(
        &z.owned_pet_life(pet).unwrap(),
        target,
        EntityTargetPurpose::Impact,
        now,
    )
}

#[test]
fn owned_pet_player_matrix_uses_source_guild_enemy_and_200_threshold() {
    let (mut z, pet, victim) = fixture();
    assert!(
        !can(&z, pet, &victim, 600),
        "All alone never invents a player threat"
    );
    threaten_owner(&mut z);
    z.owned_pet_state_mut()
        .players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .brown_until_ms = 0;
    for (mode, allowed) in [
        (0, false),
        (1, true),
        (2, true),
        (3, false),
        (4, false),
        (5, true),
    ] {
        set_mode(&mut z, mode);
        assert_eq!(can(&z, pet, &victim, 600), allowed, "mode {mode}");
    }
    set_mode(&mut z, 1);
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .chat_profile
        .group_members = vec!["OWNER".into()];
    assert!(!can(&z, pet, &victim, 600));
    set_mode(&mut z, 2);
    for sid in ["owner", "victim"] {
        z.players
            .get_mut(&SessionId::new(sid))
            .unwrap()
            .chat_profile
            .guild_name = Some("SameGuild".into());
    }
    assert!(
        can(&z, pet, &victim, 600),
        "Crystal pet Guild branch is true even with same guild"
    );
    set_mode(&mut z, 4);
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .chat_profile
        .pk_points = 199;
    assert!(!can(&z, pet, &victim, 600));
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .chat_profile
        .pk_points = 200;
    assert!(can(&z, pet, &victim, 600));
}

#[test]
fn owned_pet_last_hitter_and_brown_have_distinct_strict_deadlines() {
    let (mut z, pet, victim) = fixture();
    threaten_owner(&mut z);
    set_mode(&mut z, 4);
    assert!(can(&z, pet, &victim, 10_600));
    assert!(
        !can(&z, pet, &victim, 10_601),
        "LastHitter clears strictly after 10s"
    );
    let life = z.owned_pet_life(pet).unwrap();
    z.owned_pet_state_mut().targets.insert(
        pet,
        OwnedPetTarget {
            life,
            target: victim.clone(),
        },
    );
    assert!(can(&z, pet, &victim, 60_599));
    assert!(
        !can(&z, pet, &victim, 60_600),
        "player Brown check uses now < BrownTime"
    );
}

#[test]
fn owned_pet_safe_owner_victim_and_gm_are_rechecked_at_impact() {
    let (mut z, pet, victim) = fixture();
    threaten_owner(&mut z);
    for sid in ["owner", "victim"] {
        z.players
            .get_mut(&SessionId::new(sid))
            .unwrap()
            .chat_profile
            .in_safe_zone = true;
        assert!(!can(&z, pet, &victim, 600));
        z.players
            .get_mut(&SessionId::new(sid))
            .unwrap()
            .chat_profile
            .in_safe_zone = false;
    }
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .chat_profile
        .is_gm = true;
    assert!(!can(&z, pet, &victim, 600));
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .chat_profile
        .is_gm = false;
    assert!(z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .chat_profile
        .in_safe_zone = true;
    assert!(z.tick_owned_pet_hits(900).is_empty());
    assert_eq!(z.players[&SessionId::new("victim")].hp, 1000);
}

#[test]
fn owned_pet_pmode_moves_attacks_searches_and_clears_targets_independently() {
    let (mut z, pet, victim) = fixture();
    threaten_owner(&mut z);
    for (mode, moves, attacks, searches) in [
        (0, true, true, true),
        (1, true, false, false),
        (2, false, true, true),
        (3, false, false, false),
        (4, true, true, false),
    ] {
        let mut profile = z.players[&SessionId::new("owner")].chat_profile.clone();
        profile.pet_mode = mode;
        z.update_chat_profile(&SessionId::new("owner"), profile);
        z.clear_owned_pet_owner_targets(&SessionId::new("owner"));
        assert_eq!(z.owned_pet_can_move_now(pet), moves, "move {mode}");
        assert_eq!(z.owned_pet_can_launch_now(pet), attacks, "attack {mode}");
        assert_eq!(
            z.selected_owned_pet_target(pet, 600).is_some(),
            searches,
            "search {mode}"
        );
        if mode == 4 {
            z.focus_owned_pets(&SessionId::new("owner"), victim.object_id(), 600);
            assert!(z.selected_owned_pet_target(pet, 600).is_some());
        }
    }
}

#[test]
fn owned_pet_focus_sets_target_for_accepted_missed_melee() {
    let (mut z, pet, victim) = fixture();
    z.players
        .get_mut(&SessionId::new("owner"))
        .unwrap()
        .chat_profile
        .pet_mode = 4;
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .combat_stats
        .min_ac = 100;
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .combat_stats
        .max_ac = 100;
    let out = z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("owner"),
        object_id: 102,
        direction: MirDirection::Down,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 620,
    });
    assert!(zone_outbounds_contain_accepted_player_action(
        &out,
        &SessionId::new("owner"),
        101
    ));
    assert_eq!(z.players[&SessionId::new("victim")].hp, 1000);
    assert_eq!(
        z.selected_owned_pet_target(pet, 620).unwrap().reference,
        victim
    );
}

#[test]
fn owned_pet_focus_does_not_assign_rejected_range_or_same_group() {
    let (mut z, pet, _) = fixture();
    z.players
        .get_mut(&SessionId::new("owner"))
        .unwrap()
        .chat_profile
        .pet_mode = 4;
    let accepted = z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("owner"),
        object_id: 102,
        direction: MirDirection::Down,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 620,
    });
    assert!(zone_outbounds_contain_accepted_player_action(
        &accepted,
        &SessionId::new("owner"),
        101
    ));
    z.clear_owned_pet_owner_targets(&SessionId::new("owner"));
    z.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("owner"),
        object_id: 102,
        direction: MirDirection::Down,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 621,
    });
    assert!(
        z.selected_owned_pet_target(pet, 621).is_none(),
        "cooldown rejection cannot arm Focus"
    );
    set_mode(&mut z, 1);
    z.players
        .get_mut(&SessionId::new("owner"))
        .unwrap()
        .chat_profile
        .group_members = vec!["victim".into()];
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .chat_profile
        .group_members = vec!["owner".into()];
    z.focus_owned_pets(&SessionId::new("owner"), 102, 1200);
    assert!(z.selected_owned_pet_target(pet, 1200).is_none());
}

#[test]
fn owned_pet_typed_queue_rejects_revived_victim_recreated_pet_and_rejoined_owner() {
    for invalidation in [0, 1, 2] {
        let (mut z, pet, victim) = fixture();
        threaten_owner(&mut z);
        assert!(z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
        match invalidation {
            0 => {
                z.players
                    .get_mut(&SessionId::new("victim"))
                    .unwrap()
                    .life_generation += 1
            }
            1 => z.native_monsters.get_mut(&pet).unwrap().incarnation += 1,
            _ => {
                let j = join("owner", 101, point(10, 20));
                z.local_online_identities.admit(&j, false);
                z.refresh_local_online_presence();
            }
        }
        assert!(
            z.tick_owned_pet_hits(900).is_empty(),
            "invalidation {invalidation}"
        );
        assert_eq!(z.players[&SessionId::new("victim")].hp, 1000);
    }
}

#[test]
fn owned_pet_generic_wild_guard_remains_closed() {
    let (mut z, pet, victim) = fixture();
    threaten_owner(&mut z);
    let source = z.native_entity_monster_ref(pet).unwrap();
    assert!(!z.native_entity_can_attack(&source, &victim, EntityTargetPurpose::Impact, 600));
    assert_eq!(
        z.resolve_native_entity_hit(&source, &victim, 20, EntityDefence::AC, false, 600)
            .0,
        0
    );
    assert_eq!(z.players[&SessionId::new("victim")].hp, 1000);
    assert!(z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
    z.tick_owned_pet_hits(900);
    assert_eq!(
        z.players[&SessionId::new("victim")].hp,
        980,
        "only dedicated owned authority works"
    );
}

#[test]
fn owned_pet_death_receipt_is_causal_owner_bound_and_acknowledged_once() {
    let (mut z, pet, victim) = fixture();
    threaten_owner(&mut z);
    z.players.get_mut(&SessionId::new("victim")).unwrap().hp = 10;
    // Clear the aggressor's BrownTime to exercise unlawful settlement.
    z.owned_pet_state_mut()
        .players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .brown_until_ms = 0;
    assert!(z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
    let out = z.tick_owned_pet_hits(900);
    let receipt = out
        .iter()
        .find_map(|o| {
            if let ZoneOutbound::OwnedPetPlayerKill { receipt } = o {
                Some(receipt.clone())
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(receipt.owner_session_id, SessionId::new("owner"));
    assert_eq!(receipt.pet_object_id, pet);
    assert!(receipt.unlawful);
    assert!(!receipt.protected_by_law);
    assert!(z.issued_owned_pet_kill_is_current(&receipt));
    assert!(receipt.curse_roll < 4);
    let mut changed_roll = receipt.clone();
    changed_roll.curse_roll = (changed_roll.curse_roll + 1) & 3;
    assert!(!z.issued_owned_pet_kill_is_current(&changed_roll));
    let fork = z.transaction_fork();
    assert!(fork.issued_owned_pet_kill_is_current(&receipt));
    let mut forged = receipt.clone();
    forged.pet_incarnation += 1;
    assert!(!z.issued_owned_pet_kill_is_current(&forged));
    z.acknowledge_owned_pet_kill(&forged);
    assert!(z.issued_owned_pet_kill_is_current(&receipt));
    z.acknowledge_owned_pet_kill(&receipt);
    assert!(!z.issued_owned_pet_kill_is_current(&receipt));
    assert!(z.tick_owned_pet_hits(901).is_empty());
}

#[test]
fn owned_pet_pet_matrix_keeps_enemy_guild_and_attacker_red_brown_semantics() {
    let (mut z, pet, _) = fixture();
    let other = summon(&mut z, "victim", point(11, 21), Spell::SummonSkeleton, 20);
    let target = z.native_entity_monster_ref(other).unwrap();
    z.record_owned_pet_player_hit(101, &SessionId::new("victim"), 600);
    for (mode, allowed) in [
        (0, false),
        (1, true),
        (2, true),
        (3, true),
        (4, false),
        (5, true),
    ] {
        set_mode(&mut z, mode);
        assert_eq!(can(&z, pet, &target, 600), allowed, "pet mode {mode}");
    }
    set_mode(&mut z, 4);
    z.players
        .get_mut(&SessionId::new("owner"))
        .unwrap()
        .chat_profile
        .pk_points = 200;
    assert!(can(&z, pet, &target, 600));
    // Keep threat valid independently of the brown deadline.
    let life = z.owned_pet_life(pet).unwrap();
    z.owned_pet_state_mut().targets.insert(
        pet,
        OwnedPetTarget {
            life,
            target: target.clone(),
        },
    );
    assert!(
        can(&z, pet, &target, 60_600),
        "pet Brown check includes exact deadline"
    );
    assert!(!can(&z, pet, &target, 60_601));
    set_mode(&mut z, 1);
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .chat_profile
        .group_members = vec!["owner".into()];
    assert!(!can(&z, pet, &target, 600));
}

#[test]
fn owned_pet_owner_death_kills_pets_and_cancels_all_pending_authority() {
    let (mut z, pet, victim) = fixture();
    threaten_owner(&mut z);
    z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600);
    let out = z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: SessionId::new("owner"),
        hp: 0,
        max_hp: 1000,
        mp: 1000,
        dead: true,
    });
    assert!(z.native_monsters[&pet].dead);
    assert_eq!(z.native_monsters[&pet].hp, 0);
    assert!(out.iter().any(|o| matches!(o,ZoneOutbound::ToMany{packets,..}if packets.iter().any(|p|matches!(p,ServerPacket::ObjectDied{info}if info.object_id==pet)))));
    assert!(z.tick_owned_pet_hits(900).is_empty());
    assert_eq!(z.players[&SessionId::new("victim")].hp, 1000);
}

#[test]
fn owned_pet_checkpoint_authenticates_queue_and_cold_decode_revokes_online_authority() {
    let (mut z, pet, victim) = fixture();
    threaten_owner(&mut z);
    z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600);
    let saved = z.checkpoint_bytes().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(
        value["entity_combat"]["owned_pet"]["hits"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut restored = ZoneRuntime::restore_checkpoint(&saved).unwrap();
    assert!(
        restored.tick_owned_pet_hits(900).is_empty(),
        "cold decode does not mint the old owner Node"
    );
    let mut forged = value;
    forged["entity_combat"]["owned_pet"]["hits"][0]["damage"] = serde_json::json!(999);
    assert!(ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&forged).unwrap()).is_err());
}

#[test]
fn owned_pet_impact_rechecks_group_gm_and_source_mode_without_canceling_launched_none() {
    for change in [0, 1, 2] {
        let (mut z, pet, victim) = fixture();
        threaten_owner(&mut z);
        assert!(z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
        match change {
            0 => {
                set_mode(&mut z, 1);
                z.players
                    .get_mut(&SessionId::new("victim"))
                    .unwrap()
                    .chat_profile
                    .group_members = vec!["owner".into()];
            }
            1 => {
                z.players
                    .get_mut(&SessionId::new("victim"))
                    .unwrap()
                    .chat_profile
                    .is_gm = true
            }
            _ => set_mode(&mut z, 3),
        }
        assert!(z.tick_owned_pet_hits(900).is_empty());
        assert_eq!(z.players[&SessionId::new("victim")].hp, 1000);
    }
    let (mut z, pet, victim) = fixture();
    threaten_owner(&mut z);
    assert!(z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
    z.players
        .get_mut(&SessionId::new("owner"))
        .unwrap()
        .chat_profile
        .pet_mode = 3;
    z.tick_owned_pet_hits(900);
    assert_eq!(
        z.players[&SessionId::new("victim")].hp,
        980,
        "CompleteAttack rechecks target, not PMode"
    );
}

#[test]
fn owned_pet_death_clears_brown_before_the_next_life() {
    let (mut z, _, _) = fixture();
    threaten_owner(&mut z);
    assert_eq!(
        z.owned_pet_brown_until(&z.players[&SessionId::new("victim")]),
        60_600
    );
    z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: SessionId::new("victim"),
        hp: 0,
        max_hp: 1000,
        mp: 1000,
        dead: true,
    });
    assert_eq!(
        z.owned_pet_brown_until(&z.players[&SessionId::new("victim")]),
        600
    );
    z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: SessionId::new("victim"),
        hp: 1000,
        max_hp: 1000,
        mp: 1000,
        dead: false,
    });
    assert_eq!(
        z.owned_pet_brown_until(&z.players[&SessionId::new("victim")]),
        600
    );
}

#[test]
fn owned_pet_death_receipt_captures_victim_pk_brown_and_law_before_later_changes() {
    for (pk, brown, war, unlawful) in [
        (199, 0, false, true),
        (200, 0, false, false),
        (199, 900, false, false),
        (199, 0, true, false),
    ] {
        let (mut z, pet, victim) = fixture();
        threaten_owner(&mut z);
        z.players.get_mut(&SessionId::new("victim")).unwrap().hp = 10;
        z.players
            .get_mut(&SessionId::new("victim"))
            .unwrap()
            .chat_profile
            .pk_points = pk;
        z.owned_pet_state_mut()
            .players
            .get_mut(&SessionId::new("victim"))
            .unwrap()
            .brown_until_ms = brown;
        if war {
            z.players
                .get_mut(&SessionId::new("owner"))
                .unwrap()
                .chat_profile
                .guild_name = Some("Attackers".into());
            let target = z.players.get_mut(&SessionId::new("victim")).unwrap();
            target.chat_profile.guild_name = Some("Defenders".into());
            target.chat_profile.active_guild_wars = vec!["Attackers".into()];
        }
        assert!(z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
        let out = z.tick_owned_pet_hits(900);
        let receipt = out
            .iter()
            .find_map(|o| {
                if let ZoneOutbound::OwnedPetPlayerKill { receipt } = o {
                    Some(receipt.clone())
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(
            receipt.unlawful, unlawful,
            "pk={pk}, brown={brown}, war={war}"
        );
        assert_eq!(receipt.protected_by_law, war);
        set_mode(&mut z, 0);
        let target = z.players.get_mut(&SessionId::new("victim")).unwrap();
        target.chat_profile.pk_points = 300;
        target.chat_profile.active_guild_wars.clear();
        assert!(
            z.issued_owned_pet_kill_is_current(&receipt),
            "settlement uses the death-time receipt"
        );
    }
}

#[test]
fn owned_pet_brown_uses_victim_at_war_and_pet_hits_do_not_brown_owner() {
    let (mut z, pet, victim) = fixture();
    z.players
        .get_mut(&SessionId::new("owner"))
        .unwrap()
        .chat_profile
        .guild_name = Some("Attackers".into());
    let target = z.players.get_mut(&SessionId::new("victim")).unwrap();
    target.chat_profile.guild_name = Some("Defenders".into());
    target.chat_profile.active_guild_wars = vec!["Attackers".into()];
    z.record_owned_pet_player_hit(101, &SessionId::new("victim"), 600);
    assert_eq!(
        z.owned_pet_brown_until(&z.players[&SessionId::new("owner")]),
        0
    );
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .chat_profile
        .active_guild_wars
        .clear();
    z.players
        .get_mut(&SessionId::new("owner"))
        .unwrap()
        .chat_profile
        .active_guild_wars = vec!["Defenders".into()];
    z.record_owned_pet_player_hit(101, &SessionId::new("victim"), 601);
    assert_eq!(
        z.owned_pet_brown_until(&z.players[&SessionId::new("owner")]),
        60_601,
        "the victim's AtWar is decisive"
    );
    z.owned_pet_state_mut()
        .players
        .get_mut(&SessionId::new("owner"))
        .unwrap()
        .brown_until_ms = 0;
    let life = z.owned_pet_life(pet).unwrap();
    z.owned_pet_state_mut().targets.insert(
        pet,
        OwnedPetTarget {
            life,
            target: victim,
        },
    );
    z.record_owned_pet_player_hit(pet, &SessionId::new("victim"), 602);
    assert_eq!(
        z.owned_pet_brown_until(&z.players[&SessionId::new("owner")]),
        0
    );
}

#[test]
fn owned_shinsu_pve_hits_real_hostile_line_cells() {
    let mut z = ZoneRuntime::new(ZoneKey::for_map(REAL_MAP));
    assert!(z.has_available_collision(), "real fixture terrain must load");
    let mut owner = join("owner", 101, point(20, 20));
    owner.map_file_name = REAL_MAP.into();
    owner.chat_profile.attack_mode = 0;
    owner.hp = 100_000;
    owner.max_hp = 100_000;
    owner.combat_stats.min_ac = 100_000;
    owner.combat_stats.max_ac = 100_000;
    z.handle(ZoneCommand::Join(owner));
    let pet = summon(&mut z, "owner", point(21, 20), Spell::SummonShinsu, 10);
    for id in [9111, 9112] {
        z.handle(ZoneCommand::SpawnMonster {
            session_id: SessionId::new("owner"),
            monster: ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: id,
                name: "Scarecrow".into(),
                name_colour_argb: -1,
                image: 5,
                ai: 0,
                disposition: Some(crate::config::WorldEntityDisposition::Hostile),
                level: 30,
                max_hp: 1000,
                hp: 1000,
                experience: 0,
                move_speed_ms: 3000,
                attack_speed_ms: 5000,
                friendly_guild: None,
                defense: Default::default(),
                position: point(22 + (id - 9111) as i32, 20),
                direction: MirDirection::Down,
                respawn: None,
                drops: Vec::new(),
            },
            now_ms: 520,
        });
    }
    // Real wild AI first establishes Target = pet before the pet can acquire
    // the hostile monster under MonsterObject.IsAttackTarget(owned pet).
    z.tick(2510);
    z.tick(2511);
    let launch = z.tick(3512);
    assert!(launch.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..}if packets.iter().any(|p|matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==pet)))));
    assert_eq!(
        z.owned_pet_state().unwrap().hits.len(),
        2,
        "both real line cells should be queued"
    );
    let before = z.native_monsters[&9111].hp;
    let _out = z.tick(4062);
    assert!(
        z.native_monsters[&9111].hp < before,
        "hits remaining {:?}, source/targets {:?}, monsters {:?}",
        z.owned_pet_state().unwrap().hits,
        z.owned_pet_state().unwrap().targets,
        z.native_monsters
            .iter()
            .map(|(id, m)| (*id, m.position.clone(), m.hp))
            .collect::<Vec<_>>()
    );
}

#[test]
fn owned_pet_focus_requires_human_attack_permission_and_non_friendly_pet() {
    let (mut z, pet, _) = fixture();
    let victim = SessionId::new("victim");
    let attacker = z.owned_pet_owner(&victim).unwrap();
    let target = z.native_entity_monster_ref(pet).unwrap();
    for (mode, expected) in [
        (0, false),
        (1, true),
        (2, true),
        (3, false),
        (4, false),
        (5, false),
    ] {
        z.players.get_mut(&victim).unwrap().chat_profile.attack_mode = mode;
        assert_eq!(
            z.owned_pet_owner_target_is_hostile(&attacker, &target, 600),
            expected,
            "mode={mode}"
        );
    }
    z.players.get_mut(&victim).unwrap().chat_profile.attack_mode = 1;
    z.players
        .get_mut(&SessionId::new("owner"))
        .unwrap()
        .chat_profile
        .group_members = vec!["victim".into()];
    assert!(!z.owned_pet_owner_target_is_hostile(&attacker, &target, 600));
    z.players.get_mut(&victim).unwrap().chat_profile.attack_mode = 2;
    for session in [&victim, &SessionId::new("owner")] {
        z.players.get_mut(session).unwrap().chat_profile.guild_name = Some("SameGuild".into());
    }
    assert!(!z.owned_pet_owner_target_is_hostile(&attacker, &target, 600));
    z.players.get_mut(&victim).unwrap().chat_profile.attack_mode = 4;
    z.record_owned_pet_player_hit(101, &victim, 600);
    let deadline = z.owned_pet_brown_until(&z.players[&SessionId::new("owner")]);
    assert!(z.owned_pet_owner_target_is_hostile(&attacker, &target, deadline - 1));
    assert!(!z.owned_pet_owner_target_is_hostile(&attacker, &target, deadline));
}

fn born_owned_pet_death() -> (ZoneRuntime, ZoneOwnedPetPlayerKillReceipt) {
    let (mut z, pet, victim) = fixture();
    threaten_owner(&mut z);
    z.players.get_mut(&SessionId::new("victim")).unwrap().hp = 10;
    z.owned_pet_state_mut()
        .players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .brown_until_ms = 0;
    assert!(z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
    let receipt = z
        .tick_owned_pet_hits(900)
        .into_iter()
        .find_map(|out| match out {
            ZoneOutbound::OwnedPetPlayerKill { receipt } => Some(receipt),
            _ => None,
        })
        .expect("real pet damage must cause actual Death before a receipt is born");
    (z, receipt)
}

#[test]
fn owned_pet_birth_receipt_survives_owner_logout_cold_restore_and_exact_ack() {
    let (mut z, receipt) = born_owned_pet_death();
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("owner"),
    });
    assert!(
        z.issued_owned_pet_kill_is_current(&receipt),
        "logout cannot revoke an already born death"
    );
    assert!(
        z.transaction_fork()
            .issued_owned_pet_kill_is_current(&receipt)
    );
    let checkpoint = z.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&checkpoint).unwrap();
    assert!(
        restored.issued_owned_pet_kill_is_current(&receipt),
        "authenticated cold ledger retains the causal identity"
    );
    assert!(
        restored.online_presence.is_empty(),
        "cold restore must not revive old online attackers"
    );
    assert!(restored.tick_owned_pet_hits(901).is_empty());
    restored.acknowledge_owned_pet_kill(&receipt);
    assert!(!restored.issued_owned_pet_kill_is_current(&receipt));
    restored.acknowledge_owned_pet_kill(&receipt);
    assert!(restored.tick_owned_pet_hits(902).is_empty());
}

#[test]
fn owned_pet_birth_receipt_rejects_every_identity_law_roll_and_checkpoint_tamper() {
    let (mut z, receipt) = born_owned_pet_death();
    let original = serde_json::to_value(&receipt).unwrap();
    for (field, replacement) in [
        ("owner_account_id", serde_json::json!("other-account")),
        ("owner_character_index", serde_json::json!(99)),
        ("owner_online_identity", serde_json::json!("other-epoch")),
        (
            "owner_life_generation",
            serde_json::json!(receipt.owner_life_generation + 1),
        ),
        (
            "pet_incarnation",
            serde_json::json!(receipt.pet_incarnation + 1),
        ),
        (
            "victim_life_generation",
            serde_json::json!(receipt.victim_life_generation + 1),
        ),
        (
            "protected_by_law",
            serde_json::json!(!receipt.protected_by_law),
        ),
        ("unlawful", serde_json::json!(!receipt.unlawful)),
        ("direct_player", serde_json::json!(!receipt.direct_player)),
        (
            "curse_roll",
            serde_json::json!((receipt.curse_roll + 1) & 3),
        ),
        ("at_ms", serde_json::json!(receipt.at_ms + 1)),
    ] {
        let mut changed = original.clone();
        changed[field] = replacement;
        let forged: ZoneOwnedPetPlayerKillReceipt = serde_json::from_value(changed).unwrap();
        assert!(
            !z.issued_owned_pet_kill_is_current(&forged),
            "field={field}"
        );
        z.acknowledge_owned_pet_kill(&forged);
        assert!(z.issued_owned_pet_kill_is_current(&receipt));
    }
    let mut wrong_zone = receipt.clone();
    wrong_zone.zone_key = ZoneKey::for_map("wrong-receipt-zone");
    z.acknowledge_owned_pet_kill(&wrong_zone);
    assert!(z.issued_owned_pet_kill_is_current(&receipt));
    let mut checkpoint: serde_json::Value =
        serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap();
    checkpoint["entity_combat"]["owned_pet"]["issued_kills"][receipt.sequence.to_string()]["owner_account_id"] =
        serde_json::json!("forged");
    assert!(ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&checkpoint).unwrap()).is_err());
}

#[test]
fn owned_pet_owner_logout_before_impact_never_births_damage_or_death_receipt() {
    let (mut z, pet, victim) = fixture();
    threaten_owner(&mut z);
    z.players.get_mut(&SessionId::new("victim")).unwrap().hp = 10;
    assert!(z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("owner"),
    });
    let output = z.tick(900);
    assert_eq!(z.players[&SessionId::new("victim")].hp, 10);
    assert!(
        !output
            .iter()
            .any(|out| matches!(out, ZoneOutbound::OwnedPetPlayerKill { .. }))
    );
    assert!(z.owned_pet_state().unwrap().issued_kills.is_empty());
}

fn queue_human_pet_hit(z: &mut ZoneRuntime, pet: u32, defence: EntityDefence) {
    z.enqueue_native_player_monster_hit(
        PendingNativeMonsterHit {
            force_experience_owner: false,
            ready_at_ms: 900,
            session_id: SessionId::new("victim"),
            attacker_object_id: 102,
            object_id: pet,
            damage: 40,
            soulfire_practice: None,
            journey_event: None,
            fire_bounce: None,
        },
        defence,
        600,
    );
    assert_eq!(z.owned_pet_state().unwrap().human_hits.len(), 1);
}

#[test]
fn owned_pet_human_queue_rechecks_actual_target_incarnation_and_current_armour() {
    for replacement in [false, true] {
        let (mut z, pet, _) = fixture();
        let before = z.native_monsters[&pet].hp;
        queue_human_pet_hit(&mut z, pet, EntityDefence::AC);
        assert!(z.tick_owned_human_pet_hits(899).is_empty());
        if replacement {
            z.native_monsters.get_mut(&pet).unwrap().incarnation += 1;
            z.register_owned_pet_life(pet);
        } else {
            let target = z.native_monsters.get_mut(&pet).unwrap();
            target.defense.min_ac = 100;
            target.defense.max_ac = 100;
        }
        z.tick_owned_human_pet_hits(900);
        assert_eq!(
            z.native_monsters[&pet].hp, before,
            "recreated={replacement}"
        );
        assert!(z.owned_pet_state().unwrap().human_hits.is_empty());
    }
}

#[test]
fn owned_pet_human_queue_cold_restore_and_unproven_generic_hit_fail_closed() {
    let (mut z, pet, _) = fixture();
    let before = z.native_monsters[&pet].hp;
    queue_human_pet_hit(&mut z, pet, EntityDefence::None);
    let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    assert!(restored.tick_owned_human_pet_hits(900).is_empty());
    assert_eq!(restored.native_monsters[&pet].hp, before);
    let mut raw = z.owned_pet_state().unwrap().human_hits[0].hit.clone();
    raw.ready_at_ms = 900;
    assert!(
        z.resolve_pending_native_monster_hit(raw, 900).is_empty(),
        "the generic queue cannot authorize Human→owned pet"
    );
    assert_eq!(z.native_monsters[&pet].hp, before);
    z.tick_owned_human_pet_hits(900);
    assert!(
        z.native_monsters[&pet].hp < before,
        "only the typed queue carries live authority"
    );
}

#[test]
fn owned_pet_human_pet_brown_repeats_no_packet_and_expiry_uses_each_viewer_guild_colour() {
    let (mut z, pet, _) = fixture();
    let victim = SessionId::new("victim");
    let owner = SessionId::new("owner");
    z.players.get_mut(&victim).unwrap().chat_profile.guild_name = Some("AttackerGuild".into());
    z.players
        .get_mut(&victim)
        .unwrap()
        .chat_profile
        .active_guild_wars = vec!["ElsewhereGuild".into()];
    z.players.get_mut(&owner).unwrap().chat_profile.guild_name = Some("MasterGuild".into());
    z.players
        .get_mut(&owner)
        .unwrap()
        .chat_profile
        .active_guild_wars = vec!["AttackerGuild".into()];
    queue_human_pet_hit(&mut z, pet, EntityDefence::None);
    z.tick_owned_human_pet_hits(900);
    let start = z.flush_owned_player_colours(900);
    assert!(start.iter().any(|o| matches!(o, ZoneOutbound::ToSession { session_id, packets }
        if *session_id == victim && packets.iter().any(|p| matches!(p, ServerPacket::ColourChanged { name_colour_argb }
        if *name_colour_argb == 0xFF8B4513_u32 as i32)))));
    assert!(z.flush_owned_player_colours(900).is_empty());
    assert!(z.flush_owned_player_colours(60_899).is_empty());
    let expiry = z.flush_owned_player_colours(60_900);
    assert!(expiry.iter().any(|o| matches!(o, ZoneOutbound::ToSession { session_id, packets }
        if *session_id == victim && packets.iter().any(|p| matches!(p, ServerPacket::ColourChanged { name_colour_argb }
        if *name_colour_argb == 0xFF0000FF_u32 as i32)))));
    assert!(expiry.iter().any(|o| matches!(o, ZoneOutbound::ToSession { session_id, packets }
        if *session_id == owner && packets.iter().any(|p| matches!(p, ServerPacket::ObjectColourChanged { object_id: 102, name_colour_argb }
        if *name_colour_argb == 0xFFFFA500_u32 as i32)))));
    assert_eq!(
        z.players[&victim].name_colour_argb, 0xFF0000FF_u32 as i32,
        "viewer Orange never overwrites the actor's own Blue cache"
    );
}

fn owned_pet_test_map_flags(z: &mut ZoneRuntime, fight: bool, no_fight: bool) {
    let name = z.key.map_file_name.clone();
    z.npc_teleport_config.maps.insert(
        name.clone(),
        super::super::super::types::ZoneMapMetadata {
            map_index: 1,
            file_name: name,
            title: "owned pet rule fixture".into(),
            mini_map: 0,
            big_map: 0,
            lights: 0,
            map_dark_light: 0,
            music: 0,
            weather: 0,
            fight,
            no_fight,
            no_experience: false,
            no_group: false,
            no_pets: false,
        },
    );
}

#[test]
fn owned_pet_no_fight_guards_human_targets_but_preserves_monster_attacker_branch() {
    let (mut z, pet, victim) = fixture();
    owned_pet_test_map_flags(&mut z, false, true);
    let owner = SessionId::new("owner");
    assert!(!z.conquest_player_can_attack_player(
        &z.players[&owner],
        &z.players[&SessionId::new("victim")],
        600
    ));
    assert!(!z.melee_primary_target_present_at(&owner, MirDirection::Down, None, 600));
    let rejected = z.handle(ZoneCommand::PlayerAttackObject {
        session_id: owner,
        object_id: 102,
        direction: MirDirection::Down,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 600,
    });
    assert!(
        !rejected
            .iter()
            .any(|out| matches!(out, ZoneOutbound::PlayerDamaged { .. }))
    );
    // An already existing threat is independently legal for the MonsterObject
    // branch; NoFight is absent from PlayerObject.IsAttackTarget(MonsterObject).
    threaten_owner(&mut z);
    assert!(can(&z, pet, &victim, 600));
    let before = z.players[&SessionId::new("victim")].hp;
    assert!(z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
    z.tick_owned_pet_hits(900);
    assert!(z.players[&SessionId::new("victim")].hp < before);
}

#[test]
fn owned_pet_fight_uses_trusted_map_at_birth_and_never_reinterprets_later_map_bits() {
    let (mut z, pet, victim) = fixture();
    owned_pet_test_map_flags(&mut z, true, false);
    threaten_owner(&mut z);
    assert_eq!(
        z.owned_pet_brown_until(&z.players[&SessionId::new("victim")]),
        0,
        "HumanObject PvP aggression is exempt when the victim's map Fight bit is set"
    );
    assert!(z.conquest_player_kill_is_lawful(&SessionId::new("owner"), 101, 102, 600));
    z.players.get_mut(&SessionId::new("victim")).unwrap().hp = 10;
    assert!(z.queue_owned_pet_hit(pet, &victim, 20, EntityDefence::AC, 900, 600));
    let receipt = z
        .tick_owned_pet_hits(900)
        .into_iter()
        .find_map(|out| match out {
            ZoneOutbound::OwnedPetPlayerKill { receipt } => Some(receipt),
            _ => None,
        })
        .unwrap();
    assert!(receipt.protected_by_law);
    assert!(!receipt.unlawful);
    owned_pet_test_map_flags(&mut z, false, true);
    assert!(z.issued_owned_pet_kill_is_current(&receipt));
    let mut changed = receipt.clone();
    changed.protected_by_law = false;
    assert!(!z.issued_owned_pet_kill_is_current(&changed));
}

fn direct_human_fatal(z: &mut ZoneRuntime, source: u32, now: u64) -> Vec<ZoneOutbound> {
    z.players.get_mut(&SessionId::new("victim")).unwrap().hp = 10;
    z.resolve_native_player_unmitigated_damage(
        PendingNativePlayerHit {
            ready_at_ms: now,
            attacker_object_id: source,
            attacker_ai: 0,
            target_session_id: SessionId::new("victim"),
            target_object_id: 102,
            damage: 10,
            magic: false,
        },
        now,
    )
}

#[test]
fn owned_pet_common_human_death_receipt_uses_source_red200_brown_and_war() {
    for (pk, brown, fight, guild_war, protected, unlawful) in [
        (100, 0, false, false, false, true),
        (199, 0, false, false, false, true),
        (200, 0, false, false, false, false),
        (0, 900, false, false, false, false),
        (0, 899, false, false, false, true),
        (0, 0, true, false, true, false),
        (0, 0, false, true, true, false),
    ] {
        let (mut z, _, _) = fixture();
        owned_pet_test_map_flags(&mut z, fight, false);
        z.record_owned_pet_player_hit(101, &SessionId::new("victim"), 600);
        let target = z.players.get_mut(&SessionId::new("victim")).unwrap();
        target.chat_profile.pk_points = pk;
        if guild_war {
            target.chat_profile.guild_name = Some("VictimGuild".into());
            target.chat_profile.active_guild_wars = vec!["OwnerGuild".into()];
            z.players
                .get_mut(&SessionId::new("owner"))
                .unwrap()
                .chat_profile
                .guild_name = Some("OwnerGuild".into());
        }
        z.owned_pet_state_mut()
            .players
            .get_mut(&SessionId::new("victim"))
            .unwrap()
            .brown_until_ms = brown;
        let out = direct_human_fatal(&mut z, 101, 900);
        let receipt = out
            .into_iter()
            .find_map(|o| match o {
                ZoneOutbound::OwnedPetPlayerKill { receipt } => Some(receipt),
                _ => None,
            })
            .unwrap();
        assert!(receipt.direct_player);
        assert_eq!((receipt.pet_object_id, receipt.pet_incarnation), (0, 0));
        assert_eq!(
            (receipt.protected_by_law, receipt.unlawful),
            (protected, unlawful),
            "pk={pk},brown={brown},fight={fight},war={guild_war}"
        );
        assert_eq!(receipt.owner_account_id, "account-owner");
        assert!(z.issued_owned_pet_kill_is_current(&receipt));
        assert!(receipt.curse_roll < 4);
    }
}

#[test]
fn owned_pet_common_human_last_hitter_has_strict_expiry_and_wild_overrides_it() {
    for (now, billed) in [(10_600, true), (10_601, false)] {
        let (mut z, _, _) = fixture();
        z.record_owned_pet_player_hit(101, &SessionId::new("victim"), 600);
        let out = direct_human_fatal(&mut z, 0, now);
        assert_eq!(
            out.iter()
                .filter(|o| matches!(o, ZoneOutbound::OwnedPetPlayerKill { .. }))
                .count(),
            usize::from(billed)
        );
    }
    let (mut z, _, _) = fixture();
    // Environment loss preserves LastHitter; a real non-owner monster hit
    // replaces it and must not bill an earlier Human.
    z.record_owned_pet_player_hit(101, &SessionId::new("victim"), 600);
    let source = 9000;
    z.spawn_world_event_monster(
        &ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: source,
            name: "Deer".into(),
            name_colour_argb: -1,
            image: 4,
            ai: 2,
            disposition: None,
            level: 1,
            hp: 100,
            max_hp: 100,
            experience: 0,
            move_speed_ms: 3000,
            attack_speed_ms: 5000,
            friendly_guild: None,
            position: point(12, 21),
            direction: MirDirection::Down,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
        700,
    );
    let death = direct_human_fatal(&mut z, source, 900);
    assert!(
        !death
            .iter()
            .any(|o| matches!(o, ZoneOutbound::OwnedPetPlayerKill { .. }))
    );
}

#[test]
fn owned_pet_human_poison_uses_mac_once_recovery_and_strict_final_tick() {
    let (mut z, _, target) = fixture();
    let caster = z.owned_pet_owner(&SessionId::new("owner")).unwrap();
    let victim = z.players.get_mut(&SessionId::new("victim")).unwrap();
    victim.combat_stats.min_mac = 3;
    victim.combat_stats.max_mac = 3;
    victim.combat_stats.poison_recovery = 1;
    assert!(
        !z.apply_owned_human_poison(
            &caster,
            &target,
            CRYSTAL_POISON_GREEN,
            10,
            4,
            1000,
            false,
            600
        )
        .is_empty()
    );
    assert_eq!(z.owned_pet_state().unwrap().poisons[0].value, 7);
    assert_eq!(z.owned_pet_state().unwrap().poisons[0].ticks_left, 3);
    // Armour is sampled at application, then source ticks are raw HP loss.
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .combat_stats
        .min_mac = 1000;
    z.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .combat_stats
        .max_mac = 1000;
    z.tick_owned_human_poisons(601);
    assert_eq!(z.players[&SessionId::new("victim")].hp, 993);
    z.tick_owned_human_poisons(1601);
    assert_eq!(z.players[&SessionId::new("victim")].hp, 993);
    z.tick_owned_human_poisons(1602);
    z.tick_owned_human_poisons(2603);
    assert_eq!(z.players[&SessionId::new("victim")].hp, 979);
    assert_ne!(
        z.players[&SessionId::new("victim")].poison & CRYSTAL_POISON_GREEN,
        0
    );
    z.tick_owned_human_poisons(2604);
    assert_eq!(
        z.players[&SessionId::new("victim")].poison & CRYSTAL_POISON_GREEN,
        0
    );
    assert!(z.owned_pet_state().unwrap().poisons.is_empty());
}

#[test]
fn owned_pet_human_poison_checks_source_node_and_cold_revoke_without_clearing_new_life() {
    for source in ["death", "leave", "rejoin", "cold"] {
        let (mut z, _, target) = fixture();
        let caster = z.owned_pet_owner(&SessionId::new("owner")).unwrap();
        z.apply_owned_human_poison(
            &caster,
            &target,
            CRYSTAL_POISON_GREEN,
            7,
            12,
            1000,
            true,
            600,
        );
        match source {
            "death" => {
                z.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: SessionId::new("owner"),
                    hp: 0,
                    max_hp: 1000,
                    mp: 1000,
                    dead: true,
                });
            }
            "leave" | "rejoin" => {
                z.handle(ZoneCommand::Leave {
                    session_id: SessionId::new("owner"),
                });
                if source == "rejoin" {
                    z.handle(ZoneCommand::Join(join("owner", 101, point(10, 20))));
                }
            }
            _ => {
                z = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
            }
        }
        z.tick_owned_human_poisons(601);
        let victim = &z.players[&SessionId::new("victim")];
        assert_eq!(
            victim.hp,
            if source == "death" { 993 } else { 1000 },
            "{source}"
        );
        assert_eq!(
            victim.poison & CRYSTAL_POISON_GREEN != 0,
            source == "death",
            "{source}"
        );
    }
    let (mut z, _, target) = fixture();
    let caster = z.owned_pet_owner(&SessionId::new("owner")).unwrap();
    z.apply_owned_human_poison(
        &caster,
        &target,
        CRYSTAL_POISON_GREEN,
        7,
        12,
        1000,
        true,
        600,
    );
    let old = z.owned_pet_state().unwrap().poisons[0].clone();
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("victim"),
    });
    z.handle(ZoneCommand::Join(join("victim", 102, point(10, 21))));
    let fresh = z
        .native_entity_player_ref(&SessionId::new("victim"))
        .unwrap();
    z.apply_owned_human_poison(
        &caster,
        &fresh,
        CRYSTAL_POISON_GREEN,
        9,
        12,
        1000,
        true,
        601,
    );
    assert!(z.clear_owned_human_poison(&old, 602).is_empty());
    assert_ne!(
        z.players[&SessionId::new("victim")].poison & CRYSTAL_POISON_GREEN,
        0
    );
}

#[test]
fn owned_pet_human_poison_refresh_preserves_stronger_green_and_longer_red() {
    let (mut z, _, target) = fixture();
    let caster = z.owned_pet_owner(&SessionId::new("owner")).unwrap();
    z.apply_owned_human_poison(
        &caster,
        &target,
        CRYSTAL_POISON_GREEN,
        10,
        3,
        1000,
        true,
        600,
    );
    assert!(
        z.apply_owned_human_poison(
            &caster,
            &target,
            CRYSTAL_POISON_GREEN,
            9,
            12,
            1000,
            true,
            601
        )
        .is_empty()
    );
    assert_eq!(z.owned_pet_state().unwrap().poisons[0].ticks_left, 3);
    z.apply_owned_human_poison(&caster, &target, CRYSTAL_POISON_RED, 0, 12, 1000, true, 600);
    assert!(
        z.apply_owned_human_poison(&caster, &target, CRYSTAL_POISON_RED, 0, 11, 1000, true, 601)
            .is_empty()
    );
    assert_eq!(
        z.owned_pet_state()
            .unwrap()
            .poisons
            .iter()
            .find(|p| p.mask == CRYSTAL_POISON_RED)
            .unwrap()
            .ticks_left,
        12
    );
}

#[test]
fn owned_pet_actual_electric_shock_producer_respects_no_pets_and_typed_impact() {
    for change in [
        "none", "no_pets", "logout", "rejoin", "cold", "replace", "safe", "peace",
    ] {
        let (mut z, pet, _) = fixture();
        owned_pet_test_map_flags(&mut z, false, false);
        let sid = SessionId::new("owner");
        // Own current pet takes the guaranteed source branch after the level3
        // first roll. This tests the public command producer without RNG luck.
        z.handle(ZoneCommand::PlayerCastMagic {
            session_id: sid.clone(),
            object_id: pet,
            spell: Spell::ElectricShock,
            direction: MirDirection::Right,
            target: point(11, 20),
            cast: true,
            level: 3,
            damage: 0,
            mp_cost: 0,
            cooldown_ms: 1,
            now_ms: 1820,
        });
        assert_eq!(z.owned_pet_state().unwrap().monster_spells.len(), 1);
        match change {
            "no_pets" => {
                z.npc_teleport_config
                    .maps
                    .get_mut(&z.key.map_file_name)
                    .unwrap()
                    .no_pets = true
            }
            "logout" | "rejoin" => {
                z.handle(ZoneCommand::Leave {
                    session_id: sid.clone(),
                });
                if change == "rejoin" {
                    z.handle(ZoneCommand::Join(join("owner", 101, point(10, 20))));
                }
            }
            "cold" => z = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap(),
            "replace" => {
                z.native_monsters.get_mut(&pet).unwrap().incarnation += 1;
            }
            "safe" => {
                z.players.get_mut(&sid).unwrap().chat_profile.in_safe_zone = true;
            }
            "peace" => {
                z.players.get_mut(&sid).unwrap().chat_profile.attack_mode = 0;
            }
            _ => {}
        }
        z.tick_owned_human_monster_controls(2319);
        assert!(!z.owned_monster_shocked(pet, 2319));
        let out = z.tick_owned_human_monster_controls(2320);
        // Master==attacker/All precedes pet safe in the source Human branch.
        let shock = change == "none" || change == "safe";
        assert_eq!(z.owned_monster_shocked(pet, 2320), shock, "{change}");
        if change == "no_pets" {
            assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToSession{packets,..} if packets.iter().any(|p|matches!(p,ServerPacket::Chat{chat_type:mir2_protocol::ChatType::System,..})))));
        }
        if shock {
            let deadline = 27320;
            assert!(z.owned_monster_shock_blocks_move(pet, deadline));
            assert!(!z.owned_monster_shock_blocks_move(pet, deadline + 1));
            assert!(!z.owned_monster_shocked(pet, deadline));
        }
    }
}

#[test]
fn owned_pet_no_pets_stops_real_summon_producer_and_tame_clock_preserves_source_policy() {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("owned-pet-unit"));
    z.handle(ZoneCommand::Join(join("owner", 101, point(10, 20))));
    owned_pet_test_map_flags(&mut z, false, false);
    z.npc_teleport_config
        .maps
        .get_mut(&z.key.map_file_name)
        .unwrap()
        .no_pets = true;
    z.handle(ZoneCommand::PlayerCastMagic {
        session_id: SessionId::new("owner"),
        object_id: 0,
        spell: Spell::SummonSkeleton,
        direction: MirDirection::Right,
        target: point(11, 20),
        cast: true,
        level: 2,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1000,
        now_ms: 10,
    });
    z.tick(510);
    assert!(z.native_monsters.is_empty());
    assert!(z.pending_native_summons.is_empty());
    assert_eq!(z.players[&SessionId::new("owner")].mp, 993);
    for saved in [false, true] {
        let (mut z, pet, _) = fixture();
        let caster = z.owned_pet_owner(&SessionId::new("victim")).unwrap();
        z.npc_teleport_config.pet_save = saved;
        let old = z.owned_pet_life(pet).unwrap();
        let old_pet_level = z.native_monsters[&pet].summon_skill_level;
        z.tame_owned_monster(&caster, pet, 3, 620);
        let new = z.owned_pet_life(pet).unwrap();
        assert_ne!(old.pet, new.pet);
        assert_eq!(new.owner, caster);
        assert_eq!(
            z.native_monsters[&pet].hp,
            z.native_monsters[&pet].max_hp / 10
        );
        assert_eq!(z.owned_monster_clock(pet).unwrap().max_pet_level, 7);
        assert_eq!(z.native_monsters[&pet].summon_skill_level, old_pet_level);
        assert_eq!(
            z.owned_monster_clock(pet).unwrap().tame_until_ms,
            if saved { 0 } else { 3_600_620 }
        );
        z.tick_owned_monster_clocks(3_600_620);
        assert_eq!(z.owned_pet_life(pet).is_some(), saved);
    }
}

#[test]
fn owned_pet_binding_release_clears_each_nested_source_center_effect() {
    let (mut z, pet, _) = fixture();
    let neighbour = summon(&mut z, "owner", point(12, 20), Spell::SummonShinsu, 2010);
    assert_ne!(neighbour, pet);
    for id in [pet, neighbour] {
        z.shock_owned_monster(id, 30_000, 2600);
        z.owned_monster_clock_mut(id).unwrap().binding_center = true;
    }
    let out = z.release_owned_monster_shock(pet, 2700);
    for id in [pet, neighbour] {
        let clock = z.owned_monster_clock(id).unwrap();
        assert_eq!(clock.shock_until_ms, 0);
        assert!(!clock.binding_center);
        assert!(out.iter().any(|o| matches!(o, ZoneOutbound::ToMany {packets,..}
            if packets.iter().any(|p| matches!(p, ServerPacket::SetBindingShot { object_id, enabled:false, .. } if *object_id == id)))));
        assert!(
            matches!(z.objects[&id].packet, ServerPacket::ObjectMonster { ref info } if !info.binding_shot_center && info.shock_time == 0)
        );
    }
}

#[test]
fn owned_pet_real_summon_grows_source_stats_and_separate_gain_exp_steps() {
    let (mut z, pet, _) = fixture();
    let sid = SessionId::new("owner");
    let template = crystal_monster_by_name("BoneFamiliar").unwrap();
    let m = &z.native_monsters[&pet];
    assert_eq!(m.summon_skill_level, 2);
    assert_eq!(m.max_hp, template.hp + 40);
    assert_eq!(m.hp, m.max_hp);
    assert_eq!(m.defense.min_ac, template.min_ac + 4);
    assert_eq!(m.defense.max_mac, template.max_mac + 4);
    assert_eq!(
        m.move_speed_ms,
        u64::from(template.move_speed).saturating_sub(780).max(400)
    );
    assert_eq!(
        m.attack_speed_ms,
        u64::from(template.attack_speed)
            .saturating_sub(420)
            .max(400)
    );
    assert_eq!(z.owned_monster_clock(pet).unwrap().max_pet_level, 6);
    assert!(
        matches!(&z.objects[&pet].packet, ServerPacket::ObjectMonster {info} if info.name_colour_argb == 0xFF7FFFD4_u32 as i32)
    );
    let admission = z.capture_player_pet_experience(&sid, 600).unwrap();
    let one = admission.apply_earned_steps(&[60_000]).unwrap();
    let ignored_zero = admission.apply_earned_steps(&[60_000, 0]).unwrap();
    assert_eq!(ignored_zero, one, "GainExp(0) returns before PetExp");
    let two = admission.apply_earned_steps(&[60_000, 1]).unwrap();
    assert_eq!((one.pets[0].level, one.pets[0].experience), (3, 120_000));
    assert_eq!((two.pets[0].level, two.pets[0].experience), (4, 40_003));
    // Damage while the existing durable source CAS is in flight stays applied.
    z.native_monsters.get_mut(&pet).unwrap().hp -= 11;
    let wounded_hp = z.native_monsters[&pet].hp;
    let out = z.mirror_earned_steps(&sid, &admission, &[60_000, 1], 620);
    let m = &z.native_monsters[&pet];
    assert_eq!(m.hp, wounded_hp);
    assert_eq!(m.max_hp, template.hp + 80);
    assert_eq!(m.defense.min_ac, template.min_ac + 8);
    assert_eq!(m.summon_skill_level, 4);
    assert_eq!(z.owned_monster_clock(pet).unwrap().pet_experience, 40_003);
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..} if packets.iter().any(|p|matches!(p,ServerPacket::ObjectHealth{info} if info.object_id==pet)))));
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..} if packets.iter().any(|p|matches!(p,ServerPacket::ObjectColourChanged{object_id,name_colour_argb} if *object_id==pet && *name_colour_argb == 0xFF6A5ACD_u32 as i32)))));
    assert!(
        z.mirror_earned_steps(&sid, &admission, &[60_000, 1], 621)
            .is_empty()
    );
    assert_eq!(z.owned_monster_clock(pet).unwrap().pet_experience, 40_003);
}

#[test]
fn owned_pet_gain_exp_range_death_cap_and_uint_wrapping_match_source() {
    let sid = SessionId::new("owner");
    for distance in [16, 17] {
        let (mut z, pet, _) = fixture();
        z.native_monsters.get_mut(&pet).unwrap().position = point(10 + distance, 20);
        let admission = z.capture_player_pet_experience(&sid, 600).unwrap();
        let after = admission.apply_earned_steps(&[20_000]).unwrap();
        assert_eq!(after.pets[0].level, if distance == 16 { 3 } else { 2 });
        z.native_monsters.get_mut(&pet).unwrap().dead = true;
        z.native_monsters.get_mut(&pet).unwrap().hp = 0;
        assert!(
            z.mirror_earned_steps(&sid, &admission, &[20_000], 610)
                .is_empty()
        );
        assert!(
            z.capture_player_pet_experience(&sid, 610)
                .unwrap()
                .before_snapshot()
                .is_empty()
        );
    }
    let (mut z, pet, _) = fixture();
    z.native_monsters.get_mut(&pet).unwrap().summon_skill_level = 6;
    z.owned_monster_clock_mut(pet).unwrap().pet_experience = 7;
    let capped = z.capture_player_pet_experience(&sid, 600).unwrap();
    assert_eq!(
        capped.apply_earned_steps(&[u32::MAX]).unwrap().pets[0].experience,
        7
    );
    z.native_monsters.get_mut(&pet).unwrap().summon_skill_level = 2;
    z.owned_monster_clock_mut(pet).unwrap().pet_experience = u32::MAX - 2;
    let wraps = z.capture_player_pet_experience(&sid, 600).unwrap();
    let after = wraps.apply_earned_steps(&[1]).unwrap();
    assert_eq!((after.pets[0].level, after.pets[0].experience), (2, 0));
}

#[test]
fn owned_pet_saved_snapshot_has_only_canonical_pet_info_and_new_lives() {
    let (mut z, pet, _) = fixture();
    let sid = SessionId::new("owner");
    z.native_monsters.get_mut(&pet).unwrap().hp -= 13;
    z.owned_monster_clock_mut(pet).unwrap().pet_experience = 987;
    let old = z.owned_pet_life(pet).unwrap();
    let snapshot = z.capture_player_saved_pets(&sid, 600);
    assert_eq!(snapshot.pets.len(), 1);
    let json = serde_json::to_value(&snapshot).unwrap();
    let record = json["pets"][0].as_object().unwrap();
    assert_eq!(
        record
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "monster_index",
            "hp",
            "experience",
            "level",
            "max_pet_level"
        ]
        .map(str::to_string)
        .into_iter()
        .collect()
    );
    let mut unknown = json.clone();
    unknown["pets"][0]["old_epoch"] = 1.into();
    assert!(serde_json::from_value::<ZoneSavedPetSnapshot>(unknown).is_err());
    for mutation in ["index", "hp", "level"] {
        let mut invalid = json.clone();
        match mutation {
            "index" => invalid["pets"][0]["monster_index"] = (-999).into(),
            "hp" => invalid["pets"][0]["hp"] = 0.into(),
            _ => invalid["pets"][0]["level"] = 8.into(),
        }
        let invalid: ZoneSavedPetSnapshot = serde_json::from_value(invalid).unwrap();
        assert!(z.restore_player_saved_pets(&sid, &invalid, 601).is_err());
        assert_eq!(z.native_monsters.len(), 1);
    }
    assert!(
        z.restore_player_saved_pets(&sid, &snapshot, 602)
            .unwrap()
            .is_empty(),
        "live adoption cannot clone pets"
    );
    z.handle(ZoneCommand::Leave {
        session_id: sid.clone(),
    });
    assert!(z.native_monsters.is_empty());
    z.handle(ZoneCommand::Join(join("owner", 101, point(10, 20))));
    let out = z.restore_player_saved_pets(&sid, &snapshot, 1000).unwrap();
    let fresh_id = *z.native_monsters.keys().next().unwrap();
    let fresh = z.owned_pet_life(fresh_id).unwrap();
    assert_ne!(old.owner.online.epoch, fresh.owner.online.epoch);
    assert_ne!(old.pet, fresh.pet);
    assert_eq!(z.native_monsters[&fresh_id].hp, snapshot.pets[0].hp);
    assert_eq!(z.owned_monster_clock(fresh_id).unwrap().pet_experience, 987);
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..} if packets.iter().any(|p|matches!(p,ServerPacket::ObjectMonster{info} if info.master_object_id == 101)))));
    z.native_monsters.get_mut(&fresh_id).unwrap().hp = 0;
    z.native_monsters.get_mut(&fresh_id).unwrap().dead = true;
    assert!(z.capture_player_saved_pets(&sid, 1001).is_empty());
    assert!(
        z.restore_player_saved_pets(&sid, &snapshot, 1001)
            .unwrap()
            .is_empty(),
        "persisted data cannot resurrect current dead pet"
    );
}

#[test]
fn owned_pet_saved_policy_and_wizard_remaining_tame_clock_match_source() {
    for (class, saved, expected) in [
        (MirClass::Taoist, false, true),
        (MirClass::Wizard, false, true),
        (MirClass::Warrior, false, false),
        (MirClass::Archer, false, false),
        (MirClass::Warrior, true, true),
        (MirClass::Archer, true, true),
    ] {
        let (mut z, _, _) = fixture();
        z.players.get_mut(&SessionId::new("owner")).unwrap().class = class;
        z.npc_teleport_config.pet_save = saved;
        assert_eq!(
            !z.capture_player_saved_pets(&SessionId::new("owner"), 600)
                .is_empty(),
            expected
        );
    }
    let (mut z, pet, _) = fixture();
    let sid = SessionId::new("victim");
    let caster = z.owned_pet_owner(&sid).unwrap();
    z.tame_owned_monster(&caster, pet, 3, 620);
    z.players.get_mut(&sid).unwrap().class = MirClass::Wizard;
    let snapshot = z.capture_player_saved_pets(&sid, 1620);
    assert_eq!(snapshot.pets[0].tame_remaining_ms, Some(3_599_000));
    assert_eq!(snapshot.pets[0].level, 2);
    z.handle(ZoneCommand::Leave {
        session_id: sid.clone(),
    });
    let mut fresh = join("victim", 102, point(10, 21));
    fresh.class = MirClass::Wizard;
    z.handle(ZoneCommand::Join(fresh));
    z.restore_player_saved_pets(&sid, &snapshot, 5000).unwrap();
    let fresh_pet = *z.native_monsters.keys().next().unwrap();
    assert_eq!(
        z.owned_monster_clock(fresh_pet).unwrap().tame_until_ms,
        3_604_000
    );
}

#[test]
fn owned_pet_no_pets_entry_freezes_real_ai_but_gain_exp_still_applies() {
    let (mut z, pet, victim) = fixture();
    let sid = SessionId::new("owner");
    owned_pet_test_map_flags(&mut z, false, false);
    z.npc_teleport_config
        .maps
        .get_mut(&z.key.map_file_name)
        .unwrap()
        .no_pets = true;
    threaten_owner(&mut z);
    let out = z.apply_owned_pet_map_entry_rules(&sid, 600);
    assert!(z.owned_monster_frozen(pet));
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..} if packets.iter().any(|p|matches!(p,ServerPacket::ObjectTurn{movement} if movement.object_id==pet)))));
    for mode in [0, 1, 2, 3, 4] {
        z.players.get_mut(&sid).unwrap().chat_profile.pet_mode = mode;
        assert!(!z.owned_pet_can_move_now(pet));
        assert!(!z.owned_pet_can_launch_now(pet));
        assert!(z.tick_native_monster(pet, 2600).is_empty());
    }
    let admission = z.capture_player_pet_experience(&sid, 2600).unwrap();
    z.mirror_earned_steps(&sid, &admission, &[20_000], 2600);
    assert_eq!(z.native_monsters[&pet].summon_skill_level, 3);
    z.npc_teleport_config
        .maps
        .get_mut(&z.key.map_file_name)
        .unwrap()
        .no_pets = false;
    z.apply_owned_pet_map_entry_rules(&sid, 2601);
    assert!(!z.owned_monster_frozen(pet));
    assert!(z.owned_pet_can_launch_now(pet));
    assert!(can(&z, pet, &victim, 2601));
}

fn detached_online_pet_fixture(
    mode: u8,
    no_pets: bool,
) -> (ZoneRuntime, ZoneRuntime, u32, ZoneCombatEntityRef) {
    let (mut source, pet, victim) = fixture();
    let sid = SessionId::new("owner");
    source.players.get_mut(&sid).unwrap().chat_profile.pet_mode = mode;
    threaten_owner(&mut source);
    let book = source.local_online_identities.clone();
    let original = source.online_player_snapshot(&book);
    source.ingest_online_presence(original.clone(), true);
    let clock = source.player_owned_pet_clock(&sid).unwrap();
    let vital = source.player_vital_clock(&sid).unwrap();
    assert!(source.prepare_player_online_pet_transfer(&sid));
    source.handle(ZoneCommand::Leave {
        session_id: sid.clone(),
    });
    assert!(!source.players.contains_key(&sid));
    assert!(source.native_monsters.contains_key(&pet));
    let mut destination = ZoneRuntime::new(ZoneKey::for_map(REAL_MAP));
    assert!(
        destination.has_available_collision(),
        "real recall terrain must load"
    );
    owned_pet_test_map_flags(&mut destination, false, false);
    destination
        .npc_teleport_config
        .maps
        .get_mut(&destination.key.map_file_name)
        .unwrap()
        .no_pets = no_pets;
    source
        .npc_teleport_config
        .maps
        .extend(destination.npc_teleport_config.maps.clone());
    destination.ingest_online_presence(original, true);
    let mut arriving = join("owner", 101, point(40, 50));
    arriving.map_file_name = destination.key.map_file_name.clone();
    arriving.chat_profile.pet_mode = mode;
    destination.handle(ZoneCommand::Join(arriving));
    destination.restore_player_vital_clock(&sid, vital);
    destination.restore_player_owned_pet_clock(&sid, clock);
    let mut actual = source.online_player_snapshot(&book);
    actual.extend(destination.online_player_snapshot(&book));
    source.ingest_online_presence(actual.clone(), true);
    destination.ingest_online_presence(actual, true);
    (source, destination, pet, victim)
}

#[test]
fn owned_pet_online_map_recall_uses_source_mode_and_same_online_node() {
    for mode in [0, 1, 2, 3, 4] {
        let (mut old, mut current, pet, _) = detached_online_pet_fixture(mode, false);
        let before = old.owned_pet_life(pet).unwrap();
        let hp = old.native_monsters[&pet].hp;
        let (recalls, out) = old.drain_online_pet_recalls(2600);
        let follows = matches!(mode, 0 | 1 | 4);
        assert_eq!(recalls.len(), usize::from(follows), "mode{mode}");
        assert!(
            !old.players.contains_key(&SessionId::new("owner")),
            "Master metadata never becomes a world actor"
        );
        if follows {
            assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..}if packets.iter().any(|p|matches!(p,ServerPacket::ObjectRemove{object_id}if *object_id==pet)))));
            let moved = current.adopt_online_pet_recall(recalls.into_iter().next().unwrap(), 2600);
            let after = current.owned_pet_life(pet).unwrap();
            assert_eq!(
                before, after,
                "ordinary same-Node recall retains source pet life when its ID is free"
            );
            assert_eq!(current.native_monsters[&pet].hp, hp);
            assert_eq!(current.native_monsters[&pet].position, point(40, 49));
            assert!(old.native_monsters.is_empty());
            assert!(moved.iter().any(|o|matches!(o,ZoneOutbound::ToSession{session_id,packets}if *session_id==SessionId::new("owner") && packets.iter().any(|p|matches!(p,ServerPacket::ObjectMonster{info}if info.object_id==pet && info.master_object_id==101)))));
            assert!(moved.iter().any(|o|matches!(o,ZoneOutbound::ToMany{session_ids,packets}if session_ids.contains(&SessionId::new("owner")) && packets.iter().any(|p|matches!(p,ServerPacket::ObjectHealth{info}if info.object_id==pet && info.percent==100)))));
        } else {
            assert!(old.owned_pet_life(pet).is_some());
            assert!(current.native_monsters.is_empty());
            assert!(
                !old.capture_player_saved_pets(&SessionId::new("owner"), 2600)
                    .is_empty()
            );
            let distant = old
                .capture_player_pet_experience(&SessionId::new("owner"), 2600)
                .unwrap();
            assert_eq!(
                distant.before_snapshot(),
                distant.apply_earned_steps(&[1_000_000]).unwrap(),
                "other-map pets receive no GainExp"
            );
        }
    }
}

#[test]
fn owned_pet_old_map_attack_uses_current_remote_master_mode_group_and_life() {
    let (mut old, mut current, pet, victim) = detached_online_pet_fixture(2, false);
    assert!(can(&old, pet, &victim, 2600));
    let attacked = old.tick_native_monster(pet, 2600);
    assert!(attacked.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..} if packets.iter().any(|p|matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==pet)))));
    let hp = old.players[&SessionId::new("victim")].hp;
    let impact = old.tick_owned_pet_hits(2900);
    assert!(old.players[&SessionId::new("victim")].hp < hp);
    assert!(!impact.is_empty());
    let sid = SessionId::new("owner");
    current
        .players
        .get_mut(&sid)
        .unwrap()
        .chat_profile
        .attack_mode = 0;
    let book = old.local_online_identities.clone();
    let mut actual = old.online_player_snapshot(&book);
    actual.extend(current.online_player_snapshot(&book));
    old.ingest_online_presence(actual.clone(), true);
    assert!(!can(&old, pet, &victim, 3000));
    current
        .players
        .get_mut(&sid)
        .unwrap()
        .chat_profile
        .attack_mode = 1;
    old.players
        .get_mut(&SessionId::new("victim"))
        .unwrap()
        .chat_profile
        .group_members = vec!["OWNER".into()];
    actual = old.online_player_snapshot(&book);
    actual.extend(current.online_player_snapshot(&book));
    old.ingest_online_presence(actual, true);
    assert!(!can(&old, pet, &victim, 3000));
    current.players.get_mut(&sid).unwrap().dead = true;
    current.players.get_mut(&sid).unwrap().hp = 0;
    let mut actual = old.online_player_snapshot(&book);
    actual.extend(current.online_player_snapshot(&book));
    old.ingest_online_presence(actual, true);
    let (_, death) = old.drain_online_pet_recalls(3001);
    assert!(old.native_monsters[&pet].dead);
    assert!(death.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..}if packets.iter().any(|p|matches!(p,ServerPacket::ObjectDied{info}if info.object_id==pet)))));
}

#[test]
fn owned_pet_no_pets_old_map_freeze_and_allowed_map_force_recall_even_attack_only() {
    let (mut old, mut current, pet, victim) = detached_online_pet_fixture(2, true);
    let (caps, out) = old.drain_online_pet_recalls(2600);
    assert!(caps.is_empty());
    assert!(old.owned_monster_frozen(pet));
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToSession{packets,..}if packets.iter().any(|p|matches!(p,ServerPacket::Chat{..})))));
    assert!(old.tick_native_monster(pet, 2600).is_empty());
    assert!(!old.owned_pet_can_launch_now(pet));
    assert!(
        old.drain_online_pet_recalls(2601).1.is_empty(),
        "entry notice stays quiet after freeze"
    );
    old.npc_teleport_config
        .maps
        .get_mut(&current.key.map_file_name)
        .unwrap()
        .no_pets = false;
    current
        .npc_teleport_config
        .maps
        .get_mut(&current.key.map_file_name)
        .unwrap()
        .no_pets = false;
    let (caps, _) = old.drain_online_pet_recalls(2602);
    assert_eq!(
        caps.len(),
        1,
        "source Frozen forces allowed-map recall despite AttackOnly"
    );
    current.adopt_online_pet_recall(caps.into_iter().next().unwrap(), 2602);
    assert!(current.owned_pet_life(pet).is_some());
    assert!(!current.owned_monster_frozen(pet));
    assert!(
        !can(&current, pet, &victim, 2602),
        "old-map victim cannot become a new-map hit"
    );
}

#[test]
fn owned_pet_online_recall_cannot_adopt_into_changed_epoch_or_revive_old_hits() {
    for change in ["logout", "rejoin", "cold"] {
        let (mut old, mut current, pet, _) = detached_online_pet_fixture(0, false);
        let (mut caps, _) = old.drain_online_pet_recalls(2600);
        assert_eq!(caps.len(), 1);
        let sid = SessionId::new("owner");
        if change == "cold" {
            current =
                ZoneRuntime::restore_checkpoint(&current.checkpoint_bytes().unwrap()).unwrap();
        } else {
            current.online_presence.remove(&sid);
            current.handle(ZoneCommand::Leave {
                session_id: sid.clone(),
            });
            if change == "rejoin" {
                current.managed_online_identity = false;
                current.handle(ZoneCommand::Join(join("owner", 101, point(40, 50))));
            }
        }
        assert!(
            current
                .adopt_online_pet_recall(caps.remove(0), 2601)
                .is_empty(),
            "{change}"
        );
        assert!(!current.native_monsters.contains_key(&pet));
    }
    let (mut old, _, pet, _) = detached_online_pet_fixture(2, false);
    let out = old.remove_player_detached_pets(&SessionId::new("owner"));
    assert!(!old.native_monsters.contains_key(&pet));
    assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..}if packets.iter().any(|p|matches!(p,ServerPacket::ObjectRemove{object_id}if *object_id==pet)))));
}

#[test]
fn owned_pet_earned_steps_save_only_source_allowed_classes_without_losing_live_growth() {
    for (class, pet_save, saved) in [
        (MirClass::Taoist, false, true),
        (MirClass::Wizard, false, true),
        (MirClass::Warrior, false, false),
        (MirClass::Archer, false, false),
        (MirClass::Archer, true, true),
    ] {
        let (mut z, pet, _) = fixture();
        let sid = SessionId::new("owner");
        z.players.get_mut(&sid).unwrap().class = class;
        z.npc_teleport_config.pet_save = pet_save;
        let admission = z.capture_player_pet_experience(&sid, 600).unwrap();
        assert_eq!(admission.before_snapshot().pets.len(), 1);
        assert_eq!(
            admission.before_saved_snapshot().pets.len(),
            usize::from(saved)
        );
        let live = admission.apply_earned_steps(&[20_000]).unwrap();
        assert_eq!(live.pets[0].level, 3);
        let stored = admission.saved_after_earned_steps(&[20_000]).unwrap();
        assert_eq!(stored.pets.len(), usize::from(saved));
        if saved {
            assert_eq!(stored, live);
        }
        z.mirror_earned_steps(&sid, &admission, &[20_000], 620);
        assert_eq!(z.native_monsters[&pet].summon_skill_level, 3);
    }
}

#[test]
fn owned_pet_wizard_warm_tame_time_is_excluded_from_five_field_durable_state() {
    for cold in [false, true] {
        let (mut z, pet, _) = fixture();
        let sid = SessionId::new("victim");
        z.players.get_mut(&sid).unwrap().class = MirClass::Wizard;
        let caster = z.owned_pet_owner(&sid).unwrap();
        z.owned_monster_clock_mut(pet).unwrap().pet_experience = 91;
        z.tame_owned_monster(&caster, pet, 3, 620);
        assert_eq!(z.owned_monster_clock(pet).unwrap().pet_experience, 91);
        let warm = z.capture_player_saved_pets(&sid, 1620);
        assert_eq!(warm.pets[0].tame_remaining_ms, Some(3_599_000));
        let durable = warm.for_durable_storage();
        assert_eq!(durable, warm.without_tame_time());
        assert!(durable.pets[0].tame_remaining_ms.is_none());
        let json = serde_json::to_value(&durable).unwrap();
        assert_eq!(json["pets"][0].as_object().unwrap().len(), 5);
        let durable: ZoneSavedPetSnapshot = serde_json::from_value(json).unwrap();
        z.handle(ZoneCommand::Leave {
            session_id: sid.clone(),
        });
        let mut arriving = join("victim", 102, point(10, 21));
        arriving.class = MirClass::Wizard;
        z.handle(ZoneCommand::Join(arriving));
        let restore = if cold { &durable } else { &warm };
        z.restore_player_saved_pets(&sid, restore, 5000).unwrap();
        let fresh = *z.native_monsters.keys().next().unwrap();
        let deadline = if cold { 5000 } else { 3_604_000 };
        assert_eq!(
            z.owned_monster_clock(fresh).unwrap().tame_until_ms,
            deadline
        );
        z.tick_owned_monster_clocks(deadline - 1);
        assert!(z.owned_pet_life(fresh).is_some());
        let out = z.tick_owned_monster_clocks(deadline);
        assert!(z.owned_pet_life(fresh).is_none(), "source >= TameTime");
        assert_eq!(z.native_monsters[&fresh].master_object_id, 0);
        assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..}if packets.iter().any(|p|matches!(p,ServerPacket::ObjectName{object_id,..}if *object_id==fresh)))));
        assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..}if packets.iter().any(|p|matches!(p,ServerPacket::ObjectColourChanged{object_id,name_colour_argb}if *object_id==fresh && *name_colour_argb == -1)))));
        assert!(
            matches!(&z.objects[&fresh].packet,ServerPacket::ObjectMonster{info} if info.name_colour_argb == -1)
        );
    }
}

#[test]
fn owned_pet_recall_uses_static_back_then_owner_and_does_not_teleport_same_map() {
    for blocked_back in [false, true] {
        let (mut old, mut current, pet, _) = detached_online_pet_fixture(0, false);
        let back = point(40, 49);
        let mut occupant = join("occupant", 103, back.clone());
        occupant.map_file_name = current.key.map_file_name.clone();
        current.handle(ZoneCommand::Join(occupant));
        assert_eq!(current.players[&SessionId::new("occupant")].position, back);
        if blocked_back {
            // Only this synthetic static-wall branch installs a trusted arena;
            // ordinary recall and strict cold recovery retain the real terrain.
            current.collision = ZoneCollision::unbounded().with_blocked_cells([back.clone()]);
        }
        let (mut recalls, _) = old.drain_online_pet_recalls(2600);
        let out = current.adopt_online_pet_recall(recalls.remove(0), 2600);
        let expected = if blocked_back { point(40, 50) } else { back };
        assert_eq!(current.native_monsters[&pet].position, expected);
        assert!(out.iter().any(|o|matches!(o,ZoneOutbound::ToMany{packets,..}if packets.iter().any(|p|matches!(p,ServerPacket::ObjectTeleportIn{object_id,..}if *object_id==pet)))));
    }
    let (mut z, pet, _) = fixture();
    let distant = point(27, 20);
    z.native_monsters.get_mut(&pet).unwrap().position = distant.clone();
    let (recalls, out) = z.drain_online_pet_recalls(2600);
    assert!(recalls.is_empty());
    assert!(out.is_empty());
    assert_eq!(z.native_monsters[&pet].position, distant);
}

#[test]
fn owned_pet_immediate_monster_master_never_borrows_ultimate_human_authority() {
    let (mut z, pet, victim) = fixture();
    threaten_owner(&mut z);
    assert!(can(&z, pet, &victim, 2600));
    assert!(z.queue_owned_pet_hit(pet, &victim, 50, EntityDefence::AC, 2900, 2600));
    z.native_monsters.get_mut(&pet).unwrap().master_object_id = 700;
    assert!(z.owned_pet_life(pet).is_none());
    let hp = z.players[&SessionId::new("victim")].hp;
    assert!(z.tick_owned_pet_hits(2900).is_empty());
    assert_eq!(z.players[&SessionId::new("victim")].hp, hp);
    assert!(z.owned_pet_state().unwrap().issued_kills.is_empty());
}
