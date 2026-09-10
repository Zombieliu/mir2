//! Shared entity combat foundation. Crystal MonsterObject.cs:1918,2465,2686,
//! 2782 and MapObject.cs:460. Deliberately bounded to wild monsters attacking
//! real player-owned monsters; pet PvP, Hero and EXPOwner arbitration are open.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum ZoneCombatEntityRef {
    Player {
        session_id: SessionId,
        object_id: u32,
        life_generation: u64,
    },
    Monster {
        object_id: u32,
        incarnation: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorldEntityDisposition;

    fn fixture() -> (ZoneRuntime, ZoneCombatEntityRef, ZoneCombatEntityRef) {
        let mut zone = ZoneRuntime::new(ZoneKey::for_map("entity-unit"));
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new("owner"),
            account_id: "owner".into(),
            character_index: 1,
            object_id: 101,
            name: "owner".into(),
            class: MirClass::Taoist,
            gender: mir2_protocol::MirGender::Male,
            level: 40,
            hp: 100,
            max_hp: 100,
            mp: 100,
            map_file_name: "entity-unit".into(),
            position: Point { x: 10, y: 20 },
            direction: MirDirection::Right,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: SessionId::new("owner"),
            object_id: 0,
            spell: Spell::SummonSkeleton,
            direction: MirDirection::Right,
            target: Point { x: 11, y: 20 },
            cast: true,
            level: 2,
            damage: 0,
            mp_cost: 0,
            cooldown_ms: 1000,
            now_ms: 10,
        });
        zone.tick(510);
        let pet = *zone
            .native_monsters
            .iter()
            .find(|(_, m)| m.owner_session_id.is_some())
            .unwrap()
            .0;
        zone.spawn_world_event_monster(
            &ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: 9000,
                name: "ArcherGuard".into(),
                name_colour_argb: -1,
                image: 139,
                ai: 0,
                disposition: Some(WorldEntityDisposition::Hostile),
                level: 40,
                hp: 9999,
                max_hp: 9999,
                experience: 0,
                move_speed_ms: 300,
                attack_speed_ms: 2000,
                friendly_guild: None,
                position: Point { x: 12, y: 20 },
                direction: MirDirection::Left,
                defense: Default::default(),
                respawn: None,
                drops: Vec::new(),
            },
            511,
        );
        let source = zone.native_entity_monster_ref(9000).unwrap();
        let target = zone.native_entity_monster_ref(pet).unwrap();
        (zone, source, target)
    }
    fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
        out.iter()
            .flat_map(|o| match o {
                ZoneOutbound::ToSession { packets, .. }
                | ZoneOutbound::ToMany { packets, .. }
                | ZoneOutbound::ToAll { packets } => packets.iter().collect(),
                _ => Vec::new(),
            })
            .collect()
    }
    fn buff(zone: &mut ZoneRuntime, id: u32, stats: &[(u8, i32)]) {
        zone.native_monsters.get_mut(&id).unwrap().buffs.insert(
            52,
            super::super::super::types::ZonePlayerBuff {
                buff: ClientBuff {
                    buff_type: 52,
                    visible: true,
                    object_id: id,
                    expire_time: 0,
                    infinite: true,
                    paused: false,
                    stats: stats
                        .iter()
                        .map(|&(stat, value)| UserItemStat { stat, value })
                        .collect(),
                    values: Vec::new(),
                },
                expires_at_ms: None,
                notify_owner_on_expiry: false,
            },
        );
    }

    #[test]
    fn native_entity_armour_uses_both_buff_bounds_and_poison_rates() {
        let (mut zone, source, target) = fixture();
        let id = target.object_id();
        let m = zone.native_monsters.get_mut(&id).unwrap();
        m.defense.min_ac = 0;
        m.defense.max_ac = 0;
        buff(&mut zone, id, &[(0, 20), (1, 20)]);
        let (damage, _) =
            zone.resolve_native_entity_hit(&source, &target, 30, EntityDefence::AC, false, 600);
        assert_eq!(damage, 10, "MinAC buff must not permit a 0..20 roll");
        zone.native_monsters.get_mut(&id).unwrap().control_poison = 2 | 16;
        let (damage, _) =
            zone.resolve_native_entity_hit(&source, &target, 30, EntityDefence::AC, false, 601);
        assert_eq!(
            damage, 35,
            "Red halves armour; Stun scales raw damage before armour"
        );
    }

    #[test]
    fn native_entity_struck_skips_resist_uses_zero_attacker_and_respects_stone_trap() {
        let (mut zone, source, target) = fixture();
        let id = target.object_id();
        buff(&mut zone, id, &[(30, 10)]);
        assert_eq!(
            zone.resolve_native_entity_hit(&source, &target, 20, EntityDefence::MAC, false, 600)
                .0,
            0
        );
        let (damage, out) =
            zone.resolve_native_entity_hit(&source, &target, 20, EntityDefence::MAC, true, 600);
        assert!(damage > 0);
        assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==id && info.attacker_id==0)));
        zone.native_monsters.get_mut(&id).unwrap().ai = 255;
        assert_eq!(
            zone.resolve_native_entity_hit(&source, &target, 20, EntityDefence::AC, true, 601)
                .0,
            0
        );
        assert!(
            zone.resolve_native_entity_hit(&source, &target, 20, EntityDefence::AC, false, 601)
                .0
                > 0
        );
    }

    #[test]
    fn native_entity_visible_impact_allows_corpse_but_checks_hidden_and_life() {
        let (mut zone, source, target) = fixture();
        zone.native_monsters.get_mut(&9000).unwrap().dead = true;
        zone.native_monsters.get_mut(&9000).unwrap().hp = 0;
        assert!(!zone.native_entity_can_attack(&source, &target, EntityTargetPurpose::Search, 600));
        assert!(zone.native_entity_can_attack(
            &source,
            &target,
            EntityTargetPurpose::VisibleImpact,
            600
        ));
        zone.native_monsters
            .get_mut(&target.object_id())
            .unwrap()
            .level = 41;
        if let ServerPacket::ObjectMonster { info } =
            &mut zone.objects.get_mut(&target.object_id()).unwrap().packet
        {
            info.hidden = true;
        }
        assert!(!zone.native_entity_can_attack(
            &source,
            &target,
            EntityTargetPurpose::VisibleImpact,
            600
        ));
        assert!(zone.native_entity_can_attack(&source, &target, EntityTargetPurpose::Impact, 600));
        zone.native_monsters
            .get_mut(&target.object_id())
            .unwrap()
            .incarnation += 1;
        assert_eq!(
            zone.resolve_native_entity_hit(&source, &target, 20, EntityDefence::None, false, 600)
                .0,
            0
        );
    }

    #[test]
    fn native_entity_green_uses_monster_duration_and_survives_source_corpse_checkpoint() {
        let (mut zone, source, target) = fixture();
        let id = target.object_id();
        let before = zone.native_monsters[&id].hp;
        zone.apply_native_entity_green_poison(&source, &target, 10, 5, 2000, 600);
        zone.apply_native_entity_green_poison(&source, &target, 1, 10, 2000, 600);
        zone.native_monsters.get_mut(&9000).unwrap().dead = true;
        zone.native_monsters.get_mut(&9000).unwrap().hp = 0;
        let out = zone.tick_entity_combat(601);
        assert_eq!(zone.native_monsters[&id].hp, before - 10);
        assert!(!packets(&out)
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectStruck { .. })));
        zone.tick_entity_combat(2601);
        assert_eq!(
            zone.native_monsters[&id].hp,
            before - 10,
            "poison clock is strict now > deadline"
        );
        let mut restored =
            ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        for now in [2602, 4603, 6604, 8605] {
            assert_eq!(
                zone.tick_entity_combat(now),
                restored.tick_entity_combat(now)
            );
        }
        assert_eq!(
            zone.native_monsters[&id].hp,
            before - 40,
            "Duration 5 expires before the fifth damage tick"
        );
        assert_eq!(zone.native_monsters[&id].entity_poison, 0);
    }

    #[test]
    fn native_entity_green_retires_actual_source_and_does_not_requeue_dead_target() {
        let (mut zone, source, target) = fixture();
        let id = target.object_id();
        let before = zone.native_monsters[&id].hp;
        zone.apply_native_entity_green_poison(&source, &target, 10, 5, 2000, 600);
        zone.retire_entity_combat_life(source.object_id());
        let out = zone.tick_entity_combat(601);
        assert_eq!(zone.native_monsters[&id].hp, before);
        assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectPoisoned{object_id,poison}if *object_id==id && *poison==0)));
        zone.apply_native_entity_green_poison(&source, &target, 9999, 5, 2000, 700);
        zone.tick_entity_combat(701);
        assert!(zone.native_monsters[&id].dead);
        assert_eq!(zone.native_monsters[&id].entity_poison, 0);
        assert!(zone.entity_combat.as_ref().unwrap().poisons.is_empty());
    }

    #[test]
    fn native_entity_player_reference_rejects_pre_revival_generation() {
        let (mut zone, source, _) = fixture();
        let session = SessionId::new("owner");
        let old = zone.native_entity_player_ref(&session).unwrap();
        for hp in [0, 100] {
            zone.handle(ZoneCommand::SyncPlayerVitals {
                session_id: session.clone(),
                hp,
                max_hp: 100,
                mp: 100,
            });
        }
        assert_ne!(old, zone.native_entity_player_ref(&session).unwrap());
        assert_eq!(
            zone.resolve_native_entity_hit(&source, &old, 99, EntityDefence::None, false, 700)
                .0,
            0
        );
        assert_eq!(zone.player_vitals(&session).unwrap().0, 100);
    }

    #[test]
    fn native_entity_slow_increments_each_process_without_freezing_or_refreshing() {
        let (mut zone, source, target) = fixture();
        let id = target.object_id();
        let initial = zone.native_monsters[&id].move_speed_ms;
        // The leaf already passed resistance; Monster.ApplyPoison never rolls again.
        buff(&mut zone, id, &[(31, 10)]);
        zone.apply_native_entity_slow(&source, &target, 5, 1000, 600);
        zone.apply_native_entity_slow(&source, &target, 99, 1000, 600);
        assert_eq!(zone.entity_combat.as_ref().unwrap().slows.len(), 1);
        assert_eq!(zone.entity_combat.as_ref().unwrap().slows[0].duration, 5);
        zone.tick_entity_combat(601);
        assert_eq!(
            zone.native_monsters[&id].move_speed_ms,
            initial.saturating_add(100).min(3500)
        );
        zone.tick_entity_combat(602);
        assert_eq!(
            zone.native_monsters[&id].move_speed_ms,
            initial.saturating_add(200).min(3500)
        );
        assert_eq!(zone.entity_combat.as_ref().unwrap().slows[0].count, 1);
        assert_eq!(
            zone.native_monsters[&id].control_until_ms, 0,
            "Slow does not borrow the freeze clock"
        );
        assert!(!native_monster_control_active(
            &zone.native_monsters[&id],
            602
        ));
        for now in 603..650 {
            zone.tick_entity_combat(now);
        }
        assert_eq!(zone.native_monsters[&id].move_speed_ms, 3500);
        assert_eq!(zone.native_monsters[&id].attack_speed_ms, 3500);
    }

    #[test]
    fn native_entity_slow_survives_corpse_checkpoint_and_restores_info_speeds_on_expiry() {
        let (mut zone, source, target) = fixture();
        let id = target.object_id();
        let template = crystal_monster_by_name(&zone.native_monsters[&id].name).unwrap();
        let (base_move, base_attack) = (
            u64::from(template.move_speed),
            u64::from(template.attack_speed),
        );
        zone.apply_native_entity_slow(&source, &target, 3, 1000, 600);
        zone.tick_entity_combat(601);
        zone.native_monsters
            .get_mut(&source.object_id())
            .unwrap()
            .dead = true;
        zone.native_monsters
            .get_mut(&source.object_id())
            .unwrap()
            .hp = 0;
        let mut restored =
            ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        for now in [1601, 1602, 2602] {
            assert_eq!(
                zone.tick_entity_combat(now),
                restored.tick_entity_combat(now)
            );
        }
        assert_ne!(
            zone.native_monsters[&id].entity_poison & CRYSTAL_POISON_SLOW,
            0
        );
        assert_eq!(
            zone.tick_entity_combat(2603),
            restored.tick_entity_combat(2603)
        );
        let m = &zone.native_monsters[&id];
        assert_eq!(
            (m.move_speed_ms, m.attack_speed_ms),
            (base_move, base_attack)
        );
        assert_eq!(m.next_attack_ready_at_ms, 2603 + base_attack);
        assert_eq!(m.entity_poison & CRYSTAL_POISON_SLOW, 0);
    }

    #[test]
    fn native_entity_slow_source_retirement_clears_only_slow_and_target_death_retires_lease() {
        let (mut zone, source, target) = fixture();
        let id = target.object_id();
        zone.apply_native_entity_slow(&source, &target, 5, 1000, 600);
        zone.tick_entity_combat(601);
        zone.retire_entity_combat_life(source.object_id());
        zone.tick_entity_combat(602);
        assert_eq!(
            zone.native_monsters[&id].entity_poison & CRYSTAL_POISON_SLOW,
            0
        );
        assert!(zone.entity_combat.as_ref().unwrap().slows.is_empty());
        zone.apply_native_entity_slow(&source, &target, 5, 1000, 700);
        zone.apply_native_entity_green_poison(&source, &target, 9999, 5, 1000, 700);
        zone.tick_entity_combat(701);
        assert!(zone.native_monsters[&id].dead);
        assert!(zone.entity_combat.as_ref().unwrap().slows.is_empty());
        assert_eq!(zone.native_monsters[&id].entity_poison, 0);
        assert!(zone
            .apply_native_entity_slow(&source, &target, 5, 1000, 702)
            .is_empty());
    }

    #[test]
    fn native_entity_player_slow_has_human_second_resist_and_source_lease() {
        let (mut zone, source, _) = fixture();
        let session = SessionId::new("owner");
        let target = zone.native_entity_player_ref(&session).unwrap();
        zone.players
            .get_mut(&session)
            .unwrap()
            .combat_stats
            .poison_resist = 10;
        assert!(zone
            .apply_native_entity_slow(&source, &target, 4, 1000, 600)
            .is_empty());
        assert!(!zone_player_slowed(&zone.players[&session], 600));
        zone.players
            .get_mut(&session)
            .unwrap()
            .combat_stats
            .poison_resist = 0;
        zone.apply_native_entity_slow(&source, &target, 4, 1000, 601);
        assert!(zone_player_slowed(&zone.players[&session], 601));
        zone.native_monsters
            .get_mut(&source.object_id())
            .unwrap()
            .dead = true;
        zone.native_monsters
            .get_mut(&source.object_id())
            .unwrap()
            .hp = 0;
        zone.tick_entity_combat(602);
        assert!(zone_player_slowed(&zone.players[&session], 602));
        zone.retire_entity_combat_life(source.object_id());
        zone.tick_entity_combat(603);
        assert!(!zone_player_slowed(&zone.players[&session], 603));
        assert!(zone.players[&session]
            .native_status_poison_deadlines
            .is_empty());
    }
    #[test]
    fn native_entity_paralysis_has_independent_clock_no_refresh_and_real_freeze() {
        let (mut zone, source, target) = fixture();
        let id = target.object_id();
        buff(&mut zone, id, &[(31, 10)]);
        zone.apply_native_entity_slow(&source, &target, 8, 1000, 600);
        zone.apply_native_entity_paralysis(&source, &target, 3, 1000, 600);
        zone.apply_native_entity_paralysis(&source, &target, 99, 1000, 600);
        assert_eq!(zone.entity_combat.as_ref().unwrap().controls[0].duration, 3);
        assert!(native_monster_control_active(
            &zone.native_monsters[&id],
            600
        ));
        assert_eq!(
            zone.native_monsters[&id].control_until_ms, 0,
            "independent lease never takes the legacy control clock"
        );
        zone.tick_entity_combat(601);
        zone.native_monsters
            .get_mut(&source.object_id())
            .unwrap()
            .dead = true;
        zone.native_monsters
            .get_mut(&source.object_id())
            .unwrap()
            .hp = 0;
        let mut restored =
            ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        for now in [1601, 1602, 2602] {
            assert_eq!(
                zone.tick_entity_combat(now),
                restored.tick_entity_combat(now)
            );
        }
        assert!(native_monster_control_active(
            &zone.native_monsters[&id],
            2602
        ));
        assert_eq!(
            zone.tick_entity_combat(2603),
            restored.tick_entity_combat(2603)
        );
        assert!(!native_monster_control_active(
            &zone.native_monsters[&id],
            2603
        ));
        assert_ne!(
            zone.native_monsters[&id].entity_poison & CRYSTAL_POISON_SLOW,
            0
        );
        assert_eq!(
            zone.native_monsters[&id].entity_poison & CRYSTAL_POISON_PARALYSIS,
            0
        );
    }

    #[test]
    fn native_entity_real_purification_removes_leases_without_cancelling_inflight_damage() {
        let (mut zone, source, _) = fixture();
        let session = SessionId::new("owner");
        let target = zone.native_entity_player_ref(&session).unwrap();
        zone.apply_native_entity_slow(&source, &target, 4, 1000, 600);
        zone.apply_native_entity_paralysis(&source, &target, 4, 1000, 600);
        zone.entity_combat.as_mut().unwrap().hits.push(EntityHit {
            source: source.clone(),
            target: target.clone(),
            damage: 1,
            defence: EntityDefence::None,
            due_ms: 9999,
        });
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new("healer"),
            account_id: "healer".into(),
            character_index: 1,
            object_id: 102,
            name: "healer".into(),
            class: MirClass::Taoist,
            gender: mir2_protocol::MirGender::Male,
            level: 50,
            hp: 100,
            max_hp: 100,
            mp: 100,
            map_file_name: "entity-unit".into(),
            position: Point { x: 10, y: 21 },
            direction: MirDirection::Up,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        let out = zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: SessionId::new("healer"),
            object_id: 101,
            spell: Spell::Purification,
            direction: MirDirection::Up,
            target: Point { x: 10, y: 20 },
            cast: true,
            level: 3,
            damage: 0,
            mp_cost: 0,
            cooldown_ms: 500,
            now_ms: 2000,
        });
        assert!(packets(&out).iter().any(|p| matches!(
            p,
            ServerPacket::ObjectPoisoned {
                object_id: 101,
                poison: 0
            }
        )));
        let state = zone.entity_combat.as_ref().unwrap();
        assert!(state.slows.is_empty() && state.controls.is_empty());
        assert_eq!(state.hits.len(), 1);
        zone.apply_native_entity_slow(&source, &target, 9, 1000, 2001);
        zone.apply_native_entity_paralysis(&source, &target, 9, 1000, 2001);
        assert_eq!(zone.entity_combat.as_ref().unwrap().slows[0].duration, 9);
        assert_eq!(zone.entity_combat.as_ref().unwrap().controls[0].duration, 9);
        zone.tick_entity_combat(2002);
        assert!(zone_player_slowed(&zone.players[&session], 2002));
        assert!(zone_player_status_blocks_movement(
            &zone.players[&session],
            2002
        ));
    }

    #[test]
    fn native_entity_legacy_state_does_not_serialize_empty_new_status_fields() {
        let old = serde_json::json!({"hits":[],"poisons":[]});
        let restored: ZoneEntityCombatState = serde_json::from_value(old.clone()).unwrap();
        assert_eq!(serde_json::to_value(restored).unwrap(), old);
    }
    fn spawn_target_child(zone: &mut ZoneRuntime, id: u32, ai: u8, position: Point) {
        zone.spawn_world_event_monster(
            &ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: id,
                name: "ArcherGuard".into(),
                name_colour_argb: -1,
                image: 139,
                ai,
                disposition: Some(WorldEntityDisposition::Hostile),
                level: 40,
                hp: 9999,
                max_hp: 9999,
                experience: 0,
                move_speed_ms: 300,
                attack_speed_ms: 2000,
                friendly_guild: None,
                position,
                direction: MirDirection::Left,
                defense: Default::default(),
                respawn: None,
                drops: Vec::new(),
            },
            550,
        );
    }

    #[test]
    fn native_entity_inherited_pet_target_is_not_replaced_by_nearer_player() {
        let (mut zone, _, target) = fixture();
        spawn_target_child(&mut zone, 9001, 0, Point { x: 15, y: 20 });
        assert!(zone.set_native_entity_target(9001, &target, 550));
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new("nearer"),
            account_id: "nearer".into(),
            character_index: 1,
            object_id: 102,
            name: "nearer".into(),
            class: MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 40,
            hp: 100,
            max_hp: 100,
            mp: 100,
            map_file_name: "entity-unit".into(),
            position: Point { x: 15, y: 19 },
            direction: MirDirection::Down,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        assert_eq!(
            zone.native_entity_targets(
                9001,
                &Point { x: 15, y: 20 },
                7,
                EntityTargetPurpose::Search,
                600
            )[0]
            .object_id,
            102
        );
        let out = zone
            .try_tick_native_owned_monster_target(9001, 600)
            .unwrap();
        assert_eq!(
            zone.selected_native_entity_target(9001, 600)
                .unwrap()
                .reference,
            target
        );
        assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectWalk{movement}if movement.object_id==9001 && movement.position==Point{x:14,y:20})));
        let mut restored =
            ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        let mut fork = zone.transaction_fork();
        assert_eq!(
            restored
                .selected_native_entity_target(9001, 901)
                .unwrap()
                .reference,
            target
        );
        let out = zone
            .try_tick_native_owned_monster_target(9001, 901)
            .unwrap();
        assert_eq!(
            out,
            restored
                .try_tick_native_owned_monster_target(9001, 901)
                .unwrap()
        );
        assert_eq!(
            out,
            fork.try_tick_native_owned_monster_target(9001, 901)
                .unwrap()
        );
        assert_eq!(zone.native_monsters[&9001].position, Point { x: 13, y: 20 });
    }

    #[test]
    fn native_entity_inherited_generic_ranged_profile_executes_real_delayed_pet_damage() {
        let (mut zone, _, target) = fixture();
        zone.apply_native_monster_scripted_death_damage(9000, None, 550);
        spawn_target_child(&mut zone, 9001, 20, Point { x: 15, y: 20 });
        assert!(zone.set_native_entity_target(9001, &target, 550));
        let out = zone.tick(600);
        assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectRangeAttack{info}if info.object_id==9001 && info.target_id==target.object_id())));
        let hit = zone
            .entity_combat
            .as_ref()
            .unwrap()
            .hits
            .iter()
            .find(|h| h.source.object_id() == 9001)
            .unwrap();
        assert_eq!(
            hit.damage, 765,
            "AI20 ranged retains its existing triple-DC profile"
        );
        assert_eq!(hit.due_ms, 900);
        assert_eq!(zone.native_monsters[&9001].next_attack_ready_at_ms, 2600);
        zone.tick(899);
        assert!(zone.native_monsters[&target.object_id()].hp > 0);
        let out = zone.tick(900);
        assert_eq!(zone.native_monsters[&target.object_id()].hp, 0);
        assert!(!out.iter().any(|o| matches!(
            o,
            ZoneOutbound::PlayerDamaged { .. } | ZoneOutbound::MonsterKillAward { .. }
        )));
    }

    #[test]
    fn native_entity_target_relations_retire_both_deaths_and_never_cross_player_lives() {
        let (mut zone, source, target) = fixture();
        assert!(zone.set_native_entity_target(source.object_id(), &target, 550));
        zone.apply_native_monster_scripted_death_damage(target.object_id(), None, 600);
        assert!(zone
            .selected_native_entity_target(source.object_id(), 600)
            .is_none());
        assert!(zone.entity_combat.as_ref().unwrap().targets.is_empty());
        let player = zone
            .native_entity_player_ref(&SessionId::new("owner"))
            .unwrap();
        assert!(zone.set_native_entity_target(source.object_id(), &player, 601));
        for hp in [0, 100] {
            zone.handle(ZoneCommand::SyncPlayerVitals {
                session_id: SessionId::new("owner"),
                hp,
                max_hp: 100,
                mp: 100,
            });
        }
        assert!(zone
            .selected_native_entity_target(source.object_id(), 602)
            .is_none());
        assert!(!zone.set_native_entity_target(source.object_id(), &player, 602));
        let current = zone
            .native_entity_player_ref(&SessionId::new("owner"))
            .unwrap();
        assert!(zone.set_native_entity_target(source.object_id(), &current, 603));
        zone.apply_native_monster_scripted_death_damage(source.object_id(), None, 604);
        assert!(zone.entity_combat.as_ref().unwrap().targets.is_empty());
    }

    #[test]
    fn native_entity_target_data_range_removal_is_permanent_and_transaction_fork_is_isolated() {
        let (mut zone, source, target) = fixture();
        let player = zone
            .native_entity_player_ref(&SessionId::new("owner"))
            .unwrap();
        assert!(zone.set_native_entity_target(source.object_id(), &target, 550));
        let mut fork = zone.transaction_fork();
        assert!(fork.set_native_entity_target(source.object_id(), &player, 551));
        assert_eq!(
            zone.selected_native_entity_target(source.object_id(), 551)
                .unwrap()
                .reference,
            target
        );
        assert_eq!(
            fork.selected_native_entity_target(source.object_id(), 551)
                .unwrap()
                .reference,
            player
        );
        fork.handle(ZoneCommand::SyncPlayerTransform {
            session_id: SessionId::new("owner"),
            position: Point { x: 40, y: 20 },
            direction: MirDirection::Right,
        });
        fork.tick_entity_combat(600);
        assert!(fork.entity_combat.as_ref().unwrap().targets.is_empty());
        fork.handle(ZoneCommand::SyncPlayerTransform {
            session_id: SessionId::new("owner"),
            position: Point { x: 10, y: 20 },
            direction: MirDirection::Left,
        });
        assert!(fork
            .selected_native_entity_target(source.object_id(), 601)
            .is_none());
        assert!(
            !zone.set_native_entity_target(target.object_id(), &player, 601),
            "owned source cannot obtain a PvP target through this API"
        );
    }
    #[test]
    fn native_entity_dragon_statue_struck_immunity_does_not_block_attacked_damage() {
        let (mut zone, source, target) = fixture();
        let id = target.object_id();
        // Configure the actual summoned entity with the statue's attack
        // behavior; this is a defence fixture, not a claim of a tame producer.
        zone.native_monsters.get_mut(&id).unwrap().ai = 54;
        let before = zone.native_monsters[&id].hp;
        assert_eq!(
            zone.resolve_native_entity_hit(&source, &target, 20, EntityDefence::AC, true, 600),
            (0, Vec::new())
        );
        assert_eq!(zone.native_monsters[&id].hp, before);
        let (damage, out) =
            zone.resolve_native_entity_hit(&source, &target, 20, EntityDefence::AC, false, 601);
        assert!(damage > 0);
        assert_eq!(zone.native_monsters[&id].hp, before - damage);
        assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==id && info.attacker_id==source.object_id())));
    }
    #[test]
    fn hell_knight_rejects_environment_struck_but_remains_attackable() {
        let (mut zone, source, target) = fixture();
        // Same shared HellKnight rule for map damage and direct actor Struck.
        let pet = target.object_id();
        zone.native_monsters.get_mut(&pet).unwrap().ai = 99;
        let before = zone.native_monsters[&pet].hp;
        assert_eq!(
            zone.resolve_native_monster_environment_struck(&target, 1000, true, 600),
            (0, Vec::new())
        );
        assert_eq!(
            zone.resolve_native_entity_hit(&source, &target, 1000, EntityDefence::AC, true, 600),
            (0, Vec::new())
        );
        assert_eq!(zone.native_monsters[&pet].hp, before);
        let (damage, _) =
            zone.resolve_native_entity_hit(&source, &target, 20, EntityDefence::AC, false, 601);
        assert!(damage > 0);
        assert_eq!(zone.native_monsters[&pet].hp, before - damage);
    }

    #[test]
    fn native_entity_dazed_blocks_actual_pet_attacks_but_not_pursuit_while_frozen_blocks_both() {
        for bit in [1024, 8] {
            let (mut zone, source, target) = fixture();
            let pet = target.object_id();
            spawn_target_child(&mut zone, 9001, 0, Point { x: 13, y: 20 });
            // A stationary live combat target lets the test exercise the
            // pet's own movement/attack gates without an unrelated lethal hit.
            zone.native_monsters
                .get_mut(&9001)
                .unwrap()
                .next_ai_ready_at_ms = u64::MAX;
            zone.apply_native_entity_hell_poison(&source, &target, bit, 0, 5, 1000, 550);
            zone.resolve_native_monster_environment_struck(&source, 20000, false, 560);
            let start = zone.native_monsters[&pet].position.clone();
            for now in [600, 2000, 3001, 4002] {
                let out = zone.tick(now);
                assert!(!packets(&out)
                    .iter()
                    .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==pet)));
                if bit == 8 {
                    assert_eq!(zone.native_monsters[&pet].position, start);
                }
            }
            if bit == 1024 {
                assert_ne!(
                    zone.native_monsters[&pet].position, start,
                    "Dazed allows real pursuit at the template movement cadence"
                );
            }
            let out = zone.tick(5003);
            assert_eq!(zone.native_monsters[&pet].entity_poison & bit, 0);
            if bit == 1024 {
                let mut attacked = packets(&out)
                    .iter()
                    .any(|p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==pet));
                for now in (5303..=10003).step_by(100) {
                    let out = zone.tick(now);
                    attacked |= packets(&out).iter().any(
                        |p| matches!(p,ServerPacket::ObjectAttack{info}if info.object_id==pet),
                    );
                    if attacked {
                        break;
                    }
                }
                assert!(attacked, "actual shared summon resumes its attack after Dazed expires and it reaches melee range");
            } else {
                assert_ne!(
                    zone.native_monsters[&pet].position, start,
                    "Frozen expiry restores pursuit"
                );
            }
        }
    }

    #[test]
    fn native_entity_bleeding_is_independent_of_green_uses_duration_and_no_struck_animation() {
        let (mut zone, source, target) = fixture();
        let pet = target.object_id();
        let hp = zone.native_monsters[&pet].hp;
        zone.apply_native_entity_green_poison(&source, &target, 3, 5, 1000, 600);
        zone.apply_native_entity_hell_poison(&source, &target, 128, 7, 3, 1000, 600);
        assert_eq!(zone.entity_combat.as_ref().unwrap().poisons.len(), 2);
        let out = zone.tick_entity_combat(601);
        assert_eq!(zone.native_monsters[&pet].hp, hp - 10);
        assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectEffect{info}if info.object_id==pet && info.effect==18)));
        assert!(!packets(&out)
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectStruck { .. })));
        let mut restored =
            ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        for now in [1602, 2603] {
            assert_eq!(
                zone.tick_entity_combat(now),
                restored.tick_entity_combat(now)
            );
        }
        assert_eq!(
            zone.native_monsters[&pet].hp,
            hp - 23,
            "green 3*3 plus bleeding 2*7; expiry happens before its third damage"
        );
        assert_eq!(zone.native_monsters[&pet].entity_poison, 1);
    }

    #[test]
    fn native_entity_environment_struck_hits_unowned_target_without_wild_aggro_or_fake_credit() {
        let (mut zone, source, _) = fixture();
        spawn_target_child(&mut zone, 9001, 0, Point { x: 15, y: 20 });
        let target = zone.native_entity_monster_ref(9001).unwrap();
        zone.native_monsters.get_mut(&9001).unwrap().defense.min_mac = 5;
        zone.native_monsters.get_mut(&9001).unwrap().defense.max_mac = 5;
        buff(&mut zone, 9001, &[(30, 10)]);
        assert_eq!(
            zone.resolve_native_entity_hit(&source, &target, 20, EntityDefence::MAC, false, 600)
                .0,
            0,
            "wild-to-wild attack remains closed"
        );
        let before = zone.native_monsters[&9001].hp;
        let (damage, out) = zone.resolve_native_monster_environment_struck(&target, 20, true, 600);
        assert_eq!(
            damage, 15,
            "MAC Struck rolls armour, never magic resistance"
        );
        assert_eq!(zone.native_monsters[&9001].hp, before - 15);
        assert!(packets(&out).iter().any(|p|matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==9001 && info.attacker_id==0)));
        let (_, out) = zone.resolve_native_monster_environment_struck(&target, 20000, true, 601);
        assert!(zone.native_monsters[&9001].dead);
        assert!(!out.iter().any(|o| matches!(
            o,
            ZoneOutbound::MonsterKillAward { .. } | ZoneOutbound::PlayerDamaged { .. }
        )));
        assert!(zone
            .resolve_native_monster_environment_struck(&target, 20, true, 602)
            .1
            .is_empty());
    }

    #[test]
    fn native_entity_red_is_a_timed_armour_rate_not_direct_damage_or_refreshable_shortening() {
        let (mut zone, source, target) = fixture();
        let pet = target.object_id();
        zone.native_monsters.get_mut(&pet).unwrap().defense.min_ac = 20;
        zone.native_monsters.get_mut(&pet).unwrap().defense.max_ac = 20;
        let hp = zone.native_monsters[&pet].hp;
        zone.apply_native_entity_red_poison(&source, &target, 3, 1000, 600);
        zone.apply_native_entity_red_poison(&source, &target, 1, 1000, 600);
        assert_eq!(zone.entity_combat.as_ref().unwrap().controls[0].duration, 3);
        zone.tick_entity_combat(601);
        assert_eq!(zone.native_monsters[&pet].hp, hp);
        assert_eq!(
            zone.resolve_native_entity_hit(&source, &target, 30, EntityDefence::AC, false, 602)
                .0,
            20
        );
        zone.tick_entity_combat(1602);
        zone.tick_entity_combat(2603);
        assert_eq!(
            zone.resolve_native_entity_hit(&source, &target, 30, EntityDefence::AC, false, 2604)
                .0,
            10
        );
    }
}
impl ZoneCombatEntityRef {
    pub(super) fn object_id(&self) -> u32 {
        match self {
            Self::Player { object_id, .. } | Self::Monster { object_id, .. } => *object_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EntityTargetPurpose {
    Search,
    VisibleImpact,
    Impact,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(super) enum EntityDefence {
    ACAgility,
    AC,
    MACAgility,
    MAC,
    Agility,
    None,
}

#[derive(Debug, Clone)]
pub(super) struct NativeEntityTarget {
    pub reference: ZoneCombatEntityRef,
    pub object_id: u32,
    pub position: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntityHit {
    source: ZoneCombatEntityRef,
    target: ZoneCombatEntityRef,
    damage: i32,
    defence: EntityDefence,
    due_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntityGreenPoison {
    #[serde(
        default = "entity_green_bit",
        skip_serializing_if = "entity_is_green_bit"
    )]
    poison: u16,
    source: ZoneCombatEntityRef,
    target: ZoneCombatEntityRef,
    value: i32,
    duration: u64,
    count: u64,
    tick_speed_ms: u64,
    next_tick_ms: u64,
    source_removed: bool,
}
fn entity_green_bit() -> u16 {
    1
}
fn entity_is_green_bit(bit: &u16) -> bool {
    *bit == 1
}
pub(super) fn native_entity_attack_blocked(monster: &ZoneNativeMonster, _now: u64) -> bool {
    monster.entity_poison & 1024 != 0
}
fn entity_monster_defended_damage(
    m: &ZoneNativeMonster,
    object_id: u32,
    raw_damage: i32,
    defence: EntityDefence,
    source_id: u32,
    now: u64,
) -> i32 {
    let (minimum, maximum) = match defence {
        EntityDefence::ACAgility | EntityDefence::AC => {
            horned_stat_range(m, HornedStat::Ac, m.defense.min_ac, m.defense.max_ac, now)
        }
        EntityDefence::MACAgility | EntityDefence::MAC => horned_stat_range(
            m,
            HornedStat::Mac,
            m.defense.min_mac,
            m.defense.max_mac,
            now,
        ),
        _ => (0, 0),
    };
    let (min_bonus, max_bonus) = match defence {
        EntityDefence::ACAgility | EntityDefence::AC => (
            zone_native_monster_buff_stat_total(m, 0),
            zone_native_monster_buff_stat_total(m, 1),
        ),
        EntityDefence::MACAgility | EntityDefence::MAC => (
            zone_native_monster_buff_stat_total(m, 2),
            zone_native_monster_buff_stat_total(m, 3),
        ),
        _ => (0, 0),
    };
    let minimum = minimum.saturating_add(min_bonus).max(0);
    let mut armour = zone_roll_stat_range(
        minimum,
        maximum.saturating_add(max_bonus).max(minimum).max(0),
        now,
        source_id,
        u64::from(object_id),
    );
    let mask = native_monster_current_poison(m);
    if mask & 2 != 0 {
        armour /= 2;
    }
    (if mask & 16 != 0 {
        raw_damage.saturating_mul(3) / 2
    } else {
        raw_damage
    })
    .saturating_sub(armour)
    .max(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntitySlowPoison {
    source: ZoneCombatEntityRef,
    target: ZoneCombatEntityRef,
    duration: u64,
    count: u64,
    tick_speed_ms: u64,
    next_tick_ms: u64,
    source_removed: bool,
    base_move_speed_ms: u64,
    base_attack_speed_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct ZoneEntityCombatState {
    hits: Vec<EntityHit>,
    poisons: Vec<EntityGreenPoison>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    slows: Vec<EntitySlowPoison>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    controls: Vec<EntityStatusPoison>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    targets: BTreeMap<u32, EntityTargetRelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntityTargetRelation {
    source: ZoneCombatEntityRef,
    target: ZoneCombatEntityRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntityStatusPoison {
    source: ZoneCombatEntityRef,
    target: ZoneCombatEntityRef,
    poison: u16,
    duration: u64,
    count: u64,
    tick_speed_ms: u64,
    next_tick_ms: u64,
    source_removed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<EntityPoisonOrigin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum EntityPoisonOrigin {
    TreeQueenRoot { id: u32, start: u64, expires: u64 },
}

impl ZoneRuntime {
    pub(super) fn native_entity_monster_ref(&self, object_id: u32) -> Option<ZoneCombatEntityRef> {
        let m = self.native_monsters.get(&object_id)?;
        Some(ZoneCombatEntityRef::Monster {
            object_id,
            incarnation: m.incarnation,
        })
    }

    pub(super) fn native_entity_player_ref(
        &self,
        session_id: &SessionId,
    ) -> Option<ZoneCombatEntityRef> {
        let p = self.players.get(session_id)?;
        Some(ZoneCombatEntityRef::Player {
            session_id: session_id.clone(),
            object_id: p.object_id,
            life_generation: p.life_generation,
        })
    }

    fn entity_ref_exists(&self, reference: &ZoneCombatEntityRef, allow_dead: bool) -> bool {
        match reference {
            ZoneCombatEntityRef::Player {
                session_id,
                object_id,
                life_generation,
            } => self.players.get(session_id).is_some_and(|p| {
                p.object_id == *object_id
                    && p.life_generation == *life_generation
                    && (allow_dead || (!p.dead && p.hp > 0))
            }),
            ZoneCombatEntityRef::Monster {
                object_id,
                incarnation,
            } => self.native_monsters.get(object_id).is_some_and(|m| {
                m.incarnation == *incarnation
                    && self.objects.contains_key(object_id)
                    && (allow_dead || (!m.dead && m.hp > 0))
            }),
        }
    }

    pub(super) fn native_entity_can_attack(
        &self,
        source: &ZoneCombatEntityRef,
        target: &ZoneCombatEntityRef,
        purpose: EntityTargetPurpose,
        now: u64,
    ) -> bool {
        if source.object_id() == target.object_id()
            || !self.entity_ref_exists(source, purpose != EntityTargetPurpose::Search)
            || !self.entity_ref_exists(target, false)
        {
            return false;
        }
        let ZoneCombatEntityRef::Monster {
            object_id: source_id,
            ..
        } = source
        else {
            return false;
        };
        let source_monster = &self.native_monsters[source_id];
        // Do not invent pet-versus-pet aggro or turn passive NPCs into attackers.
        if source_monster.owner_session_id.is_some() || !source_monster.hostile_to_player {
            return false;
        }
        match target {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let p = &self.players[session_id];
                if p.chat_profile.is_gm
                    || p.combat_stats.gm_never_die
                    || p.chat_profile.in_safe_zone
                {
                    return false;
                }
                if source_monster.friendly_guild.as_ref().is_some_and(|g| {
                    p.chat_profile
                        .guild_name
                        .as_ref()
                        .is_some_and(|pg| pg.eq_ignore_ascii_case(g))
                }) {
                    return false;
                }
                if purpose == EntityTargetPurpose::Search
                    && now < source_monster.hallucination_until_ms
                {
                    return false;
                }
                if purpose != EntityTargetPurpose::Impact
                    && p.hidden
                    && !self.entity_cool_eye_sees(source_monster, p.level)
                {
                    return false;
                }
                if matches!(source_monster.ai, 6 | 113) && p.chat_profile.pk_points < 200 {
                    return false;
                }
                true
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let m = &self.native_monsters[object_id];
                let Some(owner_session) = m.owner_session_id.as_ref() else {
                    return false;
                };
                let Some(owner) = self.players.get(owner_session) else {
                    return false;
                };
                if owner.object_id != zone_native_summon_owner_player_object_id(m) {
                    return false;
                }
                if source_monster.friendly_guild.as_ref().is_some_and(|g| {
                    owner
                        .chat_profile
                        .guild_name
                        .as_ref()
                        .is_some_and(|pg| pg.eq_ignore_ascii_case(g))
                }) {
                    return false;
                }
                if !monster_visibility_is_attackable(m) {
                    return false;
                }
                // Player-owned summons remain valid for wild monsters even in
                // their owner's safe zone (MonsterObject.IsAttackTarget).
                if purpose != EntityTargetPurpose::Impact {
                    let hidden = self.objects.get(object_id).is_some_and(|o| matches!(&o.packet, ServerPacket::ObjectMonster { info } if info.hidden));
                    if hidden {
                        if !self.entity_cool_eye_sees(source_monster, m.level) {
                            return false;
                        }
                    }
                }
                // Guards and Tao guards have distinct pet safety rules.
                if matches!(source_monster.ai, 6 | 113) {
                    return !matches!(m.ai, 1 | 2 | 3) && owner.chat_profile.pk_points >= 200;
                }
                if source_monster.ai == 58 {
                    return !matches!(m.ai, 1 | 2 | 3) && owner.chat_profile.attack_mode != 0;
                }
                true
            }
        }
    }

    fn entity_cool_eye_sees(&self, source: &ZoneNativeMonster, target_level: u16) -> bool {
        source.level >= target_level
            && crystal_monster_by_name(&source.name).is_some_and(|t| {
                u64::from(t.cool_eye)
                    > crate::runtime::combat::crystal_accuracy_roll(
                        0,
                        source.incarnation as u32,
                        source.incarnation as u32,
                        100,
                    )
            })
    }

    pub(super) fn native_entity_monster_targets(
        &self,
        source_id: u32,
        centre: &Point,
        radius: i32,
        purpose: EntityTargetPurpose,
        now: u64,
    ) -> Vec<NativeEntityTarget> {
        let Some(source) = self.native_entity_monster_ref(source_id) else {
            return Vec::new();
        };
        let mut targets: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                let reference = self.native_entity_monster_ref(id)?;
                (zone_tile_distance(&m.position, centre) <= radius
                    && self.native_entity_can_attack(&source, &reference, purpose, now))
                .then(|| NativeEntityTarget {
                    reference,
                    object_id: id,
                    position: m.position.clone(),
                })
            })
            .collect();
        targets.sort_by_key(|t| {
            (
                zone_tile_distance(&t.position, centre),
                t.position.y,
                t.position.x,
                t.object_id,
            )
        });
        targets
    }

    /// Mixed candidates for species whose area attacks include players and pets.
    /// The monster-only API remains separate for leaves retaining player logic.
    pub(super) fn native_entity_targets(
        &self,
        source_id: u32,
        centre: &Point,
        radius: i32,
        purpose: EntityTargetPurpose,
        now: u64,
    ) -> Vec<NativeEntityTarget> {
        let mut targets =
            self.native_entity_monster_targets(source_id, centre, radius, purpose, now);
        let Some(source) = self.native_entity_monster_ref(source_id) else {
            return targets;
        };
        targets.extend(self.players.iter().filter_map(|(session, p)| {
            let reference = self.native_entity_player_ref(session)?;
            (zone_tile_distance(&p.position, centre) <= radius
                && self.native_entity_can_attack(&source, &reference, purpose, now))
            .then(|| NativeEntityTarget {
                reference,
                object_id: p.object_id,
                position: p.position.clone(),
            })
        }));
        targets.sort_by_key(|t| {
            (
                zone_tile_distance(&t.position, centre),
                t.position.y,
                t.position.x,
                t.object_id,
            )
        });
        targets
    }

    pub(super) fn native_entity_poison_resist(&self, target: &ZoneCombatEntityRef) -> i32 {
        if !self.entity_ref_exists(target, false) {
            return 0;
        }
        match target {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                self.players[session_id].combat_stats.poison_resist
            }
            // Current native template schema has no base PoisonResist projection.
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                zone_native_monster_buff_stat_total(&self.native_monsters[object_id], 31)
            }
        }
        .clamp(0, 10)
    }

    pub(super) fn clear_entity_combat_target_life(&mut self, object_id: u32) {
        if let Some(state) = &mut self.entity_combat {
            // Target relations require live endpoints even though already
            // launched attacks and poisons may outlive their source's death.
            state.targets.retain(|_, r| {
                r.source.object_id() != object_id && r.target.object_id() != object_id
            });
            state.hits.retain(|h| h.target.object_id() != object_id);
            state.poisons.retain(|p| p.target.object_id() != object_id);
            state.controls.retain(|p| p.target.object_id() != object_id);
            for slow in state
                .slows
                .iter()
                .filter(|p| p.target.object_id() == object_id)
            {
                if let ZoneCombatEntityRef::Monster { incarnation, .. } = &slow.target {
                    if let Some(m) = self
                        .native_monsters
                        .get_mut(&object_id)
                        .filter(|m| m.incarnation == *incarnation)
                    {
                        m.move_speed_ms = slow.base_move_speed_ms;
                        m.attack_speed_ms = slow.base_attack_speed_ms;
                    }
                }
            }
            state.slows.retain(|p| p.target.object_id() != object_id);
        }
        if let Some(m) = self.native_monsters.get_mut(&object_id) {
            m.entity_poison = 0;
        }
    }

    pub(super) fn retire_entity_combat_life(&mut self, object_id: u32) {
        if let Some(state) = &mut self.entity_combat {
            state.hits.retain(|h| h.source.object_id() != object_id);
            for poison in &mut state.poisons {
                if poison.source.object_id() == object_id {
                    poison.source_removed = true;
                }
            }
            for slow in &mut state.slows {
                if slow.source.object_id() == object_id {
                    slow.source_removed = true;
                }
            }
        }
        if let Some(state) = &mut self.entity_combat {
            for poison in &mut state.controls {
                if poison.source.object_id() == object_id && poison.origin.is_none() {
                    poison.source_removed = true;
                }
            }
        }
        self.clear_entity_combat_target_life(object_id);
    }

    /// Purification clears effects only; already launched attacks remain valid.
    /// The caller clears the player's replicated poison bits and deadlines.
    pub(super) fn clear_entity_player_poison_leases(&mut self, object_id: u32) {
        if let Some(state) = &mut self.entity_combat {
            let keep = |target: &ZoneCombatEntityRef| !matches!(target, ZoneCombatEntityRef::Player {object_id:id,..} if *id == object_id);
            state.poisons.retain(|p| keep(&p.target));
            state.slows.retain(|p| keep(&p.target));
            state.controls.retain(|p| keep(&p.target));
        }
    }

    /// A real target relationship, not player credit, LastHitter or pet PvP.
    /// Assignment (summon inheritance/forced targeting) does not require sight.
    pub(super) fn set_native_entity_target(
        &mut self,
        source_id: u32,
        target: &ZoneCombatEntityRef,
        now: u64,
    ) -> bool {
        let Some(source) = self.native_entity_monster_ref(source_id) else {
            return false;
        };
        if !self.entity_ref_exists(&source, false)
            || !self.native_entity_can_attack(&source, target, EntityTargetPurpose::Impact, now)
        {
            return false;
        }
        self.entity_combat
            .get_or_insert_with(Default::default)
            .targets
            .insert(
                source_id,
                EntityTargetRelation {
                    source,
                    target: target.clone(),
                },
            );
        true
    }

    pub(super) fn clear_native_entity_target(&mut self, source_id: u32) {
        if let Some(state) = &mut self.entity_combat {
            state.targets.remove(&source_id);
        }
    }

    pub(super) fn selected_native_entity_target(
        &self,
        source_id: u32,
        now: u64,
    ) -> Option<NativeEntityTarget> {
        let relation = self.entity_combat.as_ref()?.targets.get(&source_id)?;
        if !self.entity_ref_exists(&relation.source, false)
            || !self.native_entity_can_attack(
                &relation.source,
                &relation.target,
                EntityTargetPurpose::Impact,
                now,
            )
        {
            return None;
        }
        let position = match &relation.target {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                self.players.get(session_id)?.position.clone()
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                self.native_monsters.get(object_id)?.position.clone()
            }
        };
        // MonsterObject.Process:1198 / Globals.DataRange=16. A retained target
        // may be outside its acquisition ViewRange, but not outside DataRange.
        if zone_tile_distance(&self.native_monsters.get(&source_id)?.position, &position) > 16 {
            return None;
        }
        Some(NativeEntityTarget {
            reference: relation.target.clone(),
            object_id: relation.target.object_id(),
            position,
        })
    }

    fn prune_native_entity_targets(&mut self, now: u64) {
        let stale = self
            .entity_combat
            .as_ref()
            .map(|s| {
                s.targets
                    .keys()
                    .filter(|id| self.selected_native_entity_target(**id, now).is_none())
                    .copied()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(state) = &mut self.entity_combat {
            for id in stale {
                state.targets.remove(&id);
            }
        }
    }

    /// Ordinary AI0's first real monster-target path. Other species keep their
    /// dedicated launch/cadence logic and call the entity APIs explicitly.
    pub(super) fn try_tick_native_owned_monster_target(
        &mut self,
        object_id: u32,
        now: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&object_id)?.clone();
        if m.ai != 0 || m.owner_session_id.is_some() || !m.hostile_to_player {
            return None;
        }
        let view = crystal_monster_by_name(&m.name)
            .map(|t| i32::from(t.view_range))
            .unwrap_or(7);
        let candidates = self.native_entity_targets(
            object_id,
            &m.position,
            view,
            EntityTargetPurpose::Search,
            now,
        );
        // FindTarget gives any qualified StoneTrap precedence over normal
        // nearest targets; FindAllTargets/area enumeration keeps ring order.
        let selected = self.selected_native_entity_target(object_id, now);
        let is_stone = |t: &NativeEntityTarget| {
            self.native_monsters
                .get(&t.object_id)
                .is_some_and(|m| m.ai == 255)
        };
        let inherited = selected.is_some();
        let target = selected
            .as_ref()
            .filter(|t| is_stone(t))
            .cloned()
            .or_else(|| candidates.iter().find(|t| is_stone(t)).cloned())
            .or(selected)
            .or_else(|| candidates.first().cloned())?;
        // Ordinary player acquisition retains the established player path;
        // explicit inherited/forced player relations use this typed path.
        if !inherited && matches!(target.reference, ZoneCombatEntityRef::Player { .. }) {
            return None;
        }
        self.set_native_entity_target(object_id, &target.reference, now);
        self.tick_bound_native_entity_target(object_id, target, now)
    }

    /// Root calls this only after species-specific and passive handlers. It
    /// consumes an inherited target using the existing generic attack profile.
    pub(super) fn try_tick_inherited_entity_target(
        &mut self,
        object_id: u32,
        now: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let mut target = self.selected_native_entity_target(object_id, now)?;
        let m = self.native_monsters.get(&object_id)?;
        if m.ai != 34
            && !self
                .native_monsters
                .get(&target.object_id)
                .is_some_and(|m| m.ai == 255)
        {
            let view = crystal_monster_by_name(&m.name)
                .map(|t| i32::from(t.view_range))
                .unwrap_or(7);
            if let Some(stone) = self
                .native_entity_monster_targets(
                    object_id,
                    &m.position,
                    view,
                    EntityTargetPurpose::Search,
                    now,
                )
                .into_iter()
                .find(|t| self.native_monsters[&t.object_id].ai == 255)
            {
                self.set_native_entity_target(object_id, &stone.reference, now);
                target = stone;
            }
        }
        self.tick_bound_native_entity_target(object_id, target, now)
    }

    fn tick_bound_native_entity_target(
        &mut self,
        object_id: u32,
        target: NativeEntityTarget,
        now: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&object_id)?.clone();
        if m.dead || m.owner_session_id.is_some() || !m.hostile_to_player {
            return None;
        }
        if now < m.next_ai_ready_at_ms
            || native_monster_control_active(&m, now)
            || !monster_visibility_can_act(&m, now)
            || !sleep_monster_can_act(&m)
            || !shinsu_can_act(&m, now)
        {
            return Some(Vec::new());
        }
        let source = self.native_entity_monster_ref(object_id)?;
        let direction = zone_direction_toward(&m.position, &target.position)?;
        let direction = if m.ai == 54 { m.direction } else { direction };
        let ranged = zone_native_monster_prefers_ranged(&m, &target.position)
            && points_within_action_range(
                &m.position,
                &target.position,
                ZONE_NATIVE_MONSTER_RANGED_MAX,
            );
        if !ranged && !native_monster_adjacent_to(&m.position, &target.position) {
            if !monster_visibility_can_move(&m)
                || !sleep_monster_can_move(&m)
                || !great_fox_can_move(&m)
                || !great_fox_movement_ready(&m, now)
            {
                return Some(Vec::new());
            }
            let old_visible: BTreeSet<_> = self
                .players
                .iter()
                .filter_map(|(id, p)| {
                    p.visible_object_ids
                        .contains(&object_id)
                        .then(|| id.clone())
                })
                .collect();
            let destination = offset_point(&m.position, direction, 1);
            if !self.can_native_monster_occupy(object_id, &destination) {
                return Some(self.turn_native_monster(object_id, direction, now));
            }
            let live = self.native_monsters.get_mut(&object_id)?;
            live.position = destination.clone();
            live.direction = direction;
            live.next_ai_ready_at_ms = now.saturating_add(live.move_speed_ms.max(300));
            live.next_attack_ready_at_ms = live
                .next_attack_ready_at_ms
                .max(now.saturating_add(live.move_speed_ms));
            let packet = ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id,
                    position: destination.clone(),
                    direction,
                },
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            let mut out = self.diff_all_zone_object_visibility();
            out.push(ZoneOutbound::ToMany {
                session_ids: self
                    .players
                    .iter()
                    .filter_map(|(id, p)| {
                        (old_visible.contains(id) && p.visible_object_ids.contains(&object_id))
                            .then(|| id.clone())
                    })
                    .collect(),
                packets: vec![packet],
            });
            return Some(out);
        }
        if now <= m.next_attack_ready_at_ms {
            return Some(Vec::new());
        }
        if native_entity_attack_blocked(&m, now) {
            return Some(Vec::new());
        }
        let (damage, magic) =
            zone_native_monster_player_attack_damage(&m, &target.position, now, object_id);
        let live = self.native_monsters.get_mut(&object_id)?;
        live.direction = direction;
        live.next_ai_ready_at_ms = now.saturating_add(300);
        live.next_attack_ready_at_ms = now.saturating_add(m.attack_speed_ms);
        if damage > 0 {
            self.entity_combat
                .get_or_insert_with(Default::default)
                .hits
                .push(EntityHit {
                    source,
                    target: target.reference,
                    damage,
                    defence: if magic {
                        EntityDefence::MAC
                    } else {
                        EntityDefence::ACAgility
                    },
                    due_ms: now.saturating_add(300),
                });
        }
        let packet = if ranged {
            ServerPacket::ObjectRangeAttack {
                info: ObjectRangeAttackInfo {
                    object_id,
                    location: m.position.clone(),
                    direction,
                    target_id: target.object_id,
                    target: target.position.clone(),
                    attack_type: zone_native_monster_range_attack_type(
                        m.ai,
                        &m.position,
                        &target.position,
                    ),
                    spell: 0,
                    level: 0,
                },
            }
        } else {
            ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id,
                    location: m.position.clone(),
                    direction,
                    spell: 0,
                    level: 0,
                    attack_type: 0,
                },
            }
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        Some(vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_combat_recipients(
                object_id,
                target.object_id,
                &m.position,
            ),
            packets: vec![packet],
        }])
    }

    /// Attacked(raw, defence) and Struck(raw, defence) are distinct. Struck
    /// rolls armour but does not perform attacker accuracy or magic-resist rolls.
    pub(super) fn resolve_native_entity_hit(
        &mut self,
        source: &ZoneCombatEntityRef,
        target: &ZoneCombatEntityRef,
        raw_damage: i32,
        defence: EntityDefence,
        struck: bool,
        now: u64,
    ) -> (i32, Vec<ZoneOutbound>) {
        if raw_damage <= 0
            || !self.native_entity_can_attack(source, target, EntityTargetPurpose::Impact, now)
        {
            return (0, Vec::new());
        }
        let source_id = source.object_id();
        let source_monster = self.native_monsters[&source_id].clone();
        let accuracy = crystal_monster_by_name(&source_monster.name)
            .map(|t| t.accuracy)
            .unwrap_or_default()
            .saturating_add(zone_native_monster_buff_stat_total(&source_monster, 10))
            .max(0);
        let (agility, resist) = match target {
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let m = &self.native_monsters[object_id];
                (
                    m.defense
                        .agility
                        .saturating_add(zone_native_monster_buff_stat_total(m, 11)),
                    zone_native_monster_buff_stat_total(m, 30),
                )
            }
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let p = &self.players[session_id];
                (p.combat_stats.agility, p.combat_stats.magic_resist)
            }
        };
        let magic = matches!(defence, EntityDefence::MAC | EntityDefence::MACAgility);
        let uses_agility = matches!(
            defence,
            EntityDefence::ACAgility | EntityDefence::MACAgility | EntityDefence::Agility
        );
        let missed = !struck
            && ((magic
                && crate::runtime::combat::crystal_accuracy_roll(
                    now,
                    source_id,
                    target.object_id(),
                    10,
                ) < resist.clamp(0, 10) as u64)
                || (uses_agility
                    && crate::runtime::combat::crystal_accuracy_roll(
                        now,
                        source_id,
                        target.object_id().wrapping_add(0xEA),
                        agility.max(0) as u64 + 1,
                    ) > accuracy as u64));
        if missed {
            return (0, self.entity_miss_packet(source_id, target));
        }
        if let ZoneCombatEntityRef::Player {
            session_id,
            object_id,
            ..
        } = target
        {
            let before = self.players[session_id].hp;
            let out = self.resolve_native_player_damage(
                PendingNativePlayerHit {
                    ready_at_ms: now,
                    attacker_object_id: if struck { 0 } else { source_id },
                    attacker_ai: source_monster.ai,
                    target_session_id: session_id.clone(),
                    target_object_id: *object_id,
                    damage: raw_damage,
                    magic,
                },
                now,
                matches!(defence, EntityDefence::Agility | EntityDefence::None),
                !struck,
            );
            return ((before - self.players[session_id].hp).max(0), out);
        }
        let object_id = target.object_id();
        let m = self.native_monsters[&object_id].clone();
        // DragonStatue.Struck and StoneTrap.Struck both reject this entry;
        // their ordinary Attacked path still changes real HP.
        if struck && (!stone_trap_allows_struck(&m) || !hell_accepts_struck(&m) || m.ai == 54) {
            return (0, Vec::new());
        }
        let damage =
            entity_monster_defended_damage(&m, object_id, raw_damage, defence, source_id, now);
        if damage == 0 {
            return (
                0,
                if struck {
                    Vec::new()
                } else {
                    self.entity_miss_packet(source_id, target)
                },
            );
        }
        if !struck {
            if let Some(out) = self.trap_rock_direct_hit(object_id, now) {
                return (0, out);
            }
            if let Some(monster) = self.native_monsters.get_mut(&object_id) {
                armadillo_attacked(monster, object_id, now);
                passive_monster_on_damage(monster, object_id, None, now);
                sleep_monster_on_damage(monster, object_id, None, now);
            }
        }
        let result = if struck {
            self.apply_native_monster_damage(object_id, damage, None, now)
        } else {
            self.apply_native_monster_direct_damage(object_id, damage, None, now)
        };
        let Some(result) = result else {
            return (0, Vec::new());
        };
        let (actual, mut out) = self.entity_monster_damage_outcome(
            source_id,
            object_id,
            result,
            Some(if struck { 0 } else { source_id }),
            now,
        );
        if !struck {
            out.extend(self.snow_wolf_attacked(object_id, actual, false, now));
        }
        (actual, out)
    }

    /// MapQuake/MapSkill Struck has no actor and therefore no hostile-target
    /// gate. It still requires the real current monster instance and honours
    /// Struck immunity; it never creates a fake player or an EXP owner.
    pub(super) fn resolve_native_monster_environment_struck(
        &mut self,
        target: &ZoneCombatEntityRef,
        value: i32,
        magic: bool,
        now: u64,
    ) -> (i32, Vec<ZoneOutbound>) {
        if value <= 0
            || !matches!(target, ZoneCombatEntityRef::Monster { .. })
            || !self.entity_ref_exists(target, false)
        {
            return (0, Vec::new());
        }
        let id = target.object_id();
        let m = self.native_monsters[&id].clone();
        if !stone_trap_allows_struck(&m) || !hell_accepts_struck(&m) || m.ai == 54 {
            return (0, Vec::new());
        }
        let damage = entity_monster_defended_damage(
            &m,
            id,
            value,
            if magic {
                EntityDefence::MAC
            } else {
                EntityDefence::AC
            },
            0,
            now,
        );
        if damage == 0 {
            return (0, Vec::new());
        }
        let Some(result) = self.apply_native_monster_damage(id, damage, None, now) else {
            return (0, Vec::new());
        };
        self.entity_monster_damage_outcome(0, id, result, Some(0), now)
    }

    fn entity_miss_packet(
        &self,
        source_id: u32,
        target: &ZoneCombatEntityRef,
    ) -> Vec<ZoneOutbound> {
        let position = match target {
            ZoneCombatEntityRef::Monster { object_id, .. } => self
                .native_monsters
                .get(object_id)
                .map(|m| m.position.clone()),
            ZoneCombatEntityRef::Player { session_id, .. } => {
                self.players.get(session_id).map(|p| p.position.clone())
            }
        };
        position
            .map(|p| {
                vec![ZoneOutbound::ToMany {
                    session_ids: self.native_monster_combat_recipients(
                        source_id,
                        target.object_id(),
                        &p,
                    ),
                    packets: vec![ServerPacket::DamageIndicator {
                        damage: 0,
                        damage_type: 1,
                        object_id: target.object_id(),
                    }],
                }]
            })
            .unwrap_or_default()
    }

    fn entity_monster_damage_outcome(
        &mut self,
        source_id: u32,
        object_id: u32,
        result: NativeMonsterDamageResult,
        struck_attacker: Option<u32>,
        now: u64,
    ) -> (i32, Vec<ZoneOutbound>) {
        let (damage, percent, killed, _, _, position, direction, owner, _, _) = result;
        debug_assert!(
            owner.is_none(),
            "unattributed damage never fabricates player credit"
        );
        let mut packets = Vec::new();
        if let Some(attacker_id) = struck_attacker {
            packets.push(ServerPacket::ObjectStruck {
                info: ObjectStruckInfo {
                    object_id,
                    attacker_id,
                    location: position.clone(),
                    direction,
                },
            });
        }
        packets.push(ServerPacket::DamageIndicator {
            damage,
            damage_type: 0,
            object_id,
        });
        packets.push(ServerPacket::ObjectHealth {
            info: ObjectHealthInfo {
                object_id,
                percent,
                expire: 0,
            },
        });
        if killed {
            self.clear_entity_combat_target_life(object_id);
            packets.push(ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id,
                    location: position.clone(),
                    direction,
                    kind: 0,
                },
            });
        }
        self.apply_zone_object_packets(&packets, now);
        let out = vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_combat_recipients(source_id, object_id, &position),
            packets,
        }];
        // Wild→pet has Master!=null; environmental Struck has no owner.
        // Existing EXPOwner arbitration is not invented from target/session IDs.
        (damage, out)
    }

    /// Already admitted by the species' PoisonTarget roll. Monster ApplyPoison
    /// has no second HumanObject resistance roll and default ignoreDefence=true.
    pub(super) fn apply_native_entity_green_poison(
        &mut self,
        source: &ZoneCombatEntityRef,
        target: &ZoneCombatEntityRef,
        value: i32,
        duration: u64,
        tick_speed_ms: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if duration == 0
            || value < 0
            || !self.native_entity_can_attack(source, target, EntityTargetPurpose::Impact, now)
        {
            return Vec::new();
        }
        if let ZoneCombatEntityRef::Player {
            session_id,
            object_id,
            ..
        } = target
        {
            return self.apply_native_player_green_poison(
                source.object_id(),
                session_id,
                *object_id,
                value,
                duration,
                tick_speed_ms,
                now,
            );
        }
        let state = self.entity_combat.get_or_insert_with(Default::default);
        if state
            .poisons
            .iter()
            .any(|p| p.target == *target && p.poison == 1 && p.value > value)
        {
            return Vec::new();
        }
        state
            .poisons
            .retain(|p| p.target != *target || p.poison != 1);
        state.poisons.push(EntityGreenPoison {
            poison: 1,
            source: source.clone(),
            target: target.clone(),
            value,
            duration,
            count: 0,
            tick_speed_ms: tick_speed_ms.max(1),
            next_tick_ms: 0,
            source_removed: false,
        });
        self.set_entity_green_mask(target, true, now)
    }

    /// HellBomb's native victim path has already passed its one resistance and
    /// chance check. Player victims continue through the existing Human path.
    pub(super) fn apply_native_entity_hell_poison(
        &mut self,
        source: &ZoneCombatEntityRef,
        target: &ZoneCombatEntityRef,
        bit: u16,
        value: i32,
        duration: u64,
        tick_speed_ms: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !matches!(target, ZoneCombatEntityRef::Monster { .. })
            || !matches!(bit, 8 | 1024 | 128)
            || value < 0
            || duration == 0
            || !self.native_entity_can_attack(source, target, EntityTargetPurpose::Impact, now)
        {
            return Vec::new();
        }
        if bit != 128 {
            let was_active =
                native_monster_current_poison(&self.native_monsters[&target.object_id()]) & bit
                    != 0;
            let mut out = self.apply_native_entity_status(
                source,
                target,
                bit,
                duration,
                tick_speed_ms,
                now,
                false,
            );
            if bit == 1024
                && !was_active
                && self.native_monsters[&target.object_id()].entity_poison & bit != 0
            {
                out.push(ZoneOutbound::ToMany {
                    session_ids: self.native_monster_visible_recipients(
                        target.object_id(),
                        &self.native_monsters[&target.object_id()].position,
                    ),
                    packets: vec![ServerPacket::ObjectEffect {
                        info: ObjectEffectInfo {
                            object_id: target.object_id(),
                            effect: 26,
                            effect_type: 0,
                            delay_time: 0,
                            time: u32::try_from(duration.saturating_mul(tick_speed_ms))
                                .unwrap_or(u32::MAX),
                        },
                    }],
                });
            }
            return out;
        }
        let state = self.entity_combat.get_or_insert_with(Default::default);
        if state.poisons.iter().any(|p| {
            p.target == *target && p.poison == 128 && p.duration.saturating_sub(p.count) > duration
        }) {
            return Vec::new();
        }
        state
            .poisons
            .retain(|p| p.target != *target || p.poison != 128);
        state.poisons.push(EntityGreenPoison {
            poison: 128,
            source: source.clone(),
            target: target.clone(),
            value,
            duration,
            count: 0,
            tick_speed_ms: tick_speed_ms.max(1),
            next_tick_ms: 0,
            source_removed: false,
        });
        self.set_entity_monster_poison_mask(target, 128, true, now)
    }

    fn set_entity_green_mask(
        &mut self,
        target: &ZoneCombatEntityRef,
        active: bool,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        self.set_entity_monster_poison_mask(target, 1, active, now)
    }

    fn set_entity_monster_poison_mask(
        &mut self,
        target: &ZoneCombatEntityRef,
        bit: u16,
        active: bool,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.entity_ref_exists(target, true) {
            return Vec::new();
        }
        let id = target.object_id();
        let Some(m) = self.native_monsters.get_mut(&id) else {
            return Vec::new();
        };
        let previous = m.entity_poison;
        if active {
            m.entity_poison |= bit;
        } else {
            m.entity_poison &= !bit;
        }
        if previous == m.entity_poison {
            return Vec::new();
        }
        let position = m.position.clone();
        let packet = ServerPacket::ObjectPoisoned {
            object_id: id,
            poison: native_monster_current_poison(m),
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &position),
            packets: vec![packet],
        }]
    }

    /// Species already performed PoisonTarget's resist/chance gate. Monsters
    /// accept immediately; HumanObject.ApplyPoison has one additional resist.
    pub(super) fn apply_native_entity_slow(
        &mut self,
        source: &ZoneCombatEntityRef,
        target: &ZoneCombatEntityRef,
        duration: u64,
        tick_speed_ms: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if duration == 0
            || !self.native_entity_can_attack(source, target, EntityTargetPurpose::Impact, now)
        {
            return Vec::new();
        }
        if self
            .entity_combat
            .as_ref()
            .is_some_and(|s| s.slows.iter().any(|p| p.target == *target))
        {
            return Vec::new();
        }
        let (base_move_speed_ms, base_attack_speed_ms) = match target {
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let m = &self.native_monsters[object_id];
                if native_monster_current_poison(m) & CRYSTAL_POISON_SLOW != 0 {
                    return Vec::new();
                }
                crystal_monster_by_name(&m.name)
                    .map(|t| (u64::from(t.move_speed), u64::from(t.attack_speed)))
                    .unwrap_or((m.move_speed_ms, m.attack_speed_ms))
            }
            ZoneCombatEntityRef::Player { session_id, .. } => {
                if zone_player_active_status(&self.players[session_id], now) & CRYSTAL_POISON_SLOW
                    != 0
                    || crate::runtime::combat::crystal_accuracy_roll(
                        now,
                        source.object_id(),
                        target.object_id().wrapping_add(0x5100),
                        10,
                    ) < self.native_entity_poison_resist(target) as u64
                {
                    return Vec::new();
                }
                (0, 0)
            }
        };
        self.entity_combat
            .get_or_insert_with(Default::default)
            .slows
            .push(EntitySlowPoison {
                source: source.clone(),
                target: target.clone(),
                duration,
                count: 0,
                tick_speed_ms: tick_speed_ms.max(1),
                next_tick_ms: 0,
                source_removed: false,
                base_move_speed_ms,
                base_attack_speed_ms,
            });
        match target {
            ZoneCombatEntityRef::Monster { .. } => {
                self.set_entity_monster_poison_mask(target, CRYSTAL_POISON_SLOW, true, now)
            }
            // The lease controls source retirement and Crystal count-based
            // expiry; the player bit stays in the existing independent clocks.
            ZoneCombatEntityRef::Player { session_id, .. } => self
                .apply_native_player_status_poison(session_id, CRYSTAL_POISON_SLOW, u64::MAX, now),
        }
    }

    fn finish_entity_slow(&mut self, slow: &EntitySlowPoison, now: u64) -> Vec<ZoneOutbound> {
        if !self.entity_ref_exists(&slow.target, true) {
            return Vec::new();
        }
        match &slow.target {
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let m = self.native_monsters.get_mut(object_id).unwrap();
                m.move_speed_ms = slow.base_move_speed_ms;
                m.attack_speed_ms = slow.base_attack_speed_ms;
                m.next_attack_ready_at_ms = now.saturating_add(m.attack_speed_ms);
                self.set_entity_monster_poison_mask(&slow.target, CRYSTAL_POISON_SLOW, false, now)
            }
            ZoneCombatEntityRef::Player { .. } => {
                self.finish_entity_player_status(&slow.target, CRYSTAL_POISON_SLOW)
            }
        }
    }

    fn finish_entity_player_status(
        &mut self,
        target: &ZoneCombatEntityRef,
        bit: u16,
    ) -> Vec<ZoneOutbound> {
        if !self.entity_ref_exists(target, true) {
            return Vec::new();
        }
        let ZoneCombatEntityRef::Player { session_id, .. } = target else {
            return Vec::new();
        };
        let p = self.players.get_mut(session_id).unwrap();
        let old = p.poison;
        p.native_status_poison_deadlines.remove(&bit);
        p.native_status_poison &= !bit;
        p.poison &= !bit;
        p.native_status_poison_expires_at_ms =
            p.native_status_poison_deadlines.values().copied().max();
        if old == p.poison {
            return Vec::new();
        }
        let (object_id, position, poison) = (p.object_id, p.position.clone(), p.poison);
        vec![ZoneOutbound::ToMany {
            session_ids: self.player_status_recipients(object_id, &position),
            packets: vec![ServerPacket::ObjectPoisoned { object_id, poison }],
        }]
    }

    /// PoisonTarget admission belongs to the species. HumanObject then rolls
    /// resistance once more, while MonsterObject directly admits the lease.
    pub(super) fn apply_native_entity_paralysis(
        &mut self,
        source: &ZoneCombatEntityRef,
        target: &ZoneCombatEntityRef,
        duration: u64,
        tick_speed_ms: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        self.apply_native_entity_status(
            source,
            target,
            CRYSTAL_POISON_PARALYSIS,
            duration,
            tick_speed_ms,
            now,
            true,
        )
    }

    pub(super) fn apply_native_entity_red_poison(
        &mut self,
        source: &ZoneCombatEntityRef,
        target: &ZoneCombatEntityRef,
        duration: u64,
        tick_speed_ms: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        self.apply_native_entity_status(source, target, 2, duration, tick_speed_ms, now, true)
    }

    /// The Queen supplies initial attack admission/attribution; the actual
    /// spell node, including an invisible child cell, owns this poison lease.
    pub(super) fn apply_native_tree_root_paralysis(
        &mut self,
        queen: &ZoneCombatEntityRef,
        target: &ZoneCombatEntityRef,
        root_id: u32,
        root_start: u64,
        root_expires: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if now > root_expires
            || self.entity_combat.as_ref().is_some_and(|s| {
                s.controls
                    .iter()
                    .any(|p| p.target == *target && p.poison == 32)
            })
        {
            return Vec::new();
        }
        let out = self.apply_native_entity_paralysis(queen, target, 5, 1000, now);
        if let Some(poison) = self.entity_combat.as_mut().and_then(|s| {
            s.controls
                .iter_mut()
                .find(|p| p.source == *queen && p.target == *target && p.poison == 32)
        }) {
            poison.origin = Some(EntityPoisonOrigin::TreeQueenRoot {
                id: root_id,
                start: root_start,
                expires: root_expires,
            });
        }
        out
    }

    fn entity_status_source_exists(&self, poison: &EntityStatusPoison, now: u64) -> bool {
        match &poison.origin {
            Some(EntityPoisonOrigin::TreeQueenRoot { id, start, expires }) => {
                self.tree_queen_root_poison_origin_alive(*id, *start, *expires, now)
            }
            None => !poison.source_removed && self.entity_ref_exists(&poison.source, true),
        }
    }

    fn apply_native_entity_status(
        &mut self,
        source: &ZoneCombatEntityRef,
        target: &ZoneCombatEntityRef,
        bit: u16,
        duration: u64,
        tick_speed_ms: u64,
        now: u64,
        human_resist: bool,
    ) -> Vec<ZoneOutbound> {
        let no_refresh = matches!(bit, 8 | 32);
        if duration == 0
            || !matches!(bit, 2 | 8 | 32 | 1024)
            || !self.native_entity_can_attack(source, target, EntityTargetPurpose::Impact, now)
            || self.entity_combat.as_ref().is_some_and(|s| {
                s.controls.iter().any(|p| {
                    p.target == *target
                        && p.poison == bit
                        && (no_refresh || p.duration.saturating_sub(p.count) > duration)
                })
            })
        {
            return Vec::new();
        }
        match target {
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                if no_refresh
                    && native_monster_current_poison(&self.native_monsters[object_id]) & bit != 0
                {
                    return Vec::new();
                }
            }
            ZoneCombatEntityRef::Player { session_id, .. } => {
                if (no_refresh
                    && zone_player_active_status(&self.players[session_id], now) & bit != 0)
                    || (human_resist
                        && crate::runtime::combat::crystal_accuracy_roll(
                            now,
                            source.object_id(),
                            target.object_id().wrapping_add(0x3200),
                            10,
                        ) < self.native_entity_poison_resist(target) as u64)
                {
                    return Vec::new();
                }
            }
        }
        self.entity_combat
            .get_or_insert_with(Default::default)
            .controls
            .retain(|p| p.target != *target || p.poison != bit);
        self.entity_combat
            .get_or_insert_with(Default::default)
            .controls
            .push(EntityStatusPoison {
                source: source.clone(),
                target: target.clone(),
                poison: bit,
                duration,
                count: 0,
                tick_speed_ms: tick_speed_ms.max(1),
                next_tick_ms: 0,
                source_removed: false,
                origin: None,
            });
        match target {
            ZoneCombatEntityRef::Monster { .. } => {
                self.set_entity_monster_poison_mask(target, bit, true, now)
            }
            ZoneCombatEntityRef::Player { session_id, .. } => {
                self.apply_native_player_status_poison(session_id, bit, u64::MAX, now)
            }
        }
    }

    pub(super) fn tick_entity_combat(&mut self, now: u64) -> Vec<ZoneOutbound> {
        self.prune_native_entity_targets(now);
        let Some(mut state) = self.entity_combat.take() else {
            return Vec::new();
        };
        let hits = std::mem::take(&mut state.hits);
        let poisons = std::mem::take(&mut state.poisons);
        let slows = std::mem::take(&mut state.slows);
        let controls = std::mem::take(&mut state.controls);
        self.entity_combat = Some(state);
        let mut out = Vec::new();
        for mut poison in controls {
            let mut expired = !self.entity_ref_exists(&poison.target, false)
                || !self.entity_status_source_exists(&poison, now);
            if !expired && now > poison.next_tick_ms {
                poison.count = poison.count.saturating_add(1);
                poison.next_tick_ms = now.saturating_add(poison.tick_speed_ms);
                expired = poison.count >= poison.duration;
            }
            if expired {
                out.extend(match &poison.target {
                    ZoneCombatEntityRef::Monster { .. } => self.set_entity_monster_poison_mask(
                        &poison.target,
                        poison.poison,
                        false,
                        now,
                    ),
                    ZoneCombatEntityRef::Player { .. } => {
                        self.finish_entity_player_status(&poison.target, poison.poison)
                    }
                });
            } else {
                self.entity_combat
                    .get_or_insert_with(Default::default)
                    .controls
                    .push(poison);
            }
        }
        for mut slow in slows {
            if !self.entity_ref_exists(&slow.target, false) {
                out.extend(self.finish_entity_slow(&slow, now));
                continue;
            }
            if slow.source_removed || !self.entity_ref_exists(&slow.source, true) {
                out.extend(self.finish_entity_slow(&slow, now));
                continue;
            }
            if now > slow.next_tick_ms {
                slow.count = slow.count.saturating_add(1);
                slow.next_tick_ms = now.saturating_add(slow.tick_speed_ms);
                if slow.count >= slow.duration {
                    out.extend(self.finish_entity_slow(&slow, now));
                    continue;
                }
            }
            if let ZoneCombatEntityRef::Monster { object_id, .. } = &slow.target {
                let m = self.native_monsters.get_mut(object_id).unwrap();
                // Crystal increments on every ProcessPoison, even between its
                // duration ticks. This is a speed penalty, never a freeze.
                m.move_speed_ms = m.move_speed_ms.saturating_add(100).min(3500);
                m.attack_speed_ms = m.attack_speed_ms.saturating_add(100).min(3500);
            }
            self.entity_combat
                .get_or_insert_with(Default::default)
                .slows
                .push(slow);
        }
        // Existing poison leases tick before newly landed attacks can apply a
        // fresh lease. Duration/strict time checks match MonsterObject.ProcessPoison.
        for mut p in poisons {
            if !self.entity_ref_exists(&p.target, false) {
                continue;
            }
            if p.source_removed || !self.entity_ref_exists(&p.source, true) {
                out.extend(self.set_entity_monster_poison_mask(&p.target, p.poison, false, now));
                continue;
            }
            if now > p.next_tick_ms {
                p.count = p.count.saturating_add(1);
                p.next_tick_ms = now.saturating_add(p.tick_speed_ms);
                if p.count >= p.duration {
                    out.extend(
                        self.set_entity_monster_poison_mask(&p.target, p.poison, false, now),
                    );
                    continue;
                }
                if p.poison == 128 {
                    out.push(ZoneOutbound::ToMany {
                        session_ids: self.native_monster_visible_recipients(
                            p.target.object_id(),
                            &self.native_monsters[&p.target.object_id()].position,
                        ),
                        packets: vec![ServerPacket::ObjectEffect {
                            info: ObjectEffectInfo {
                                object_id: p.target.object_id(),
                                effect: 18,
                                effect_type: 0,
                                delay_time: 0,
                                time: 0,
                            },
                        }],
                    });
                }
                if p.value > 0 {
                    if let Some(result) =
                        self.apply_native_monster_damage(p.target.object_id(), p.value, None, now)
                    {
                        let (_, events) = self.entity_monster_damage_outcome(
                            p.source.object_id(),
                            p.target.object_id(),
                            result,
                            None,
                            now,
                        );
                        out.extend(events);
                    }
                }
            }
            if self.entity_ref_exists(&p.target, false) {
                self.entity_combat
                    .get_or_insert_with(Default::default)
                    .poisons
                    .push(p);
            }
        }
        for hit in hits {
            if now < hit.due_ms {
                self.entity_combat
                    .get_or_insert_with(Default::default)
                    .hits
                    .push(hit);
                continue;
            }
            let (_, events) = self.resolve_native_entity_hit(
                &hit.source,
                &hit.target,
                hit.damage,
                hit.defence,
                false,
                now,
            );
            out.extend(events);
        }
        out
    }
}
