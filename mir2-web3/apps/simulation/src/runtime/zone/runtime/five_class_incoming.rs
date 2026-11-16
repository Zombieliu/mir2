//! Current trusted Human defence for Assassin/Archer victims of legacy attacks.
//! Source HumanObject.Attacked: reduction and armour block precede EnergyShield.
//! Classic victim behaviour and legacy attacker critical clocks stay separate.
use super::*;

pub(super) fn is_new_profession(player: &ZonePlayer) -> bool {
    matches!(player.class, MirClass::Assassin | MirClass::Archer)
}

fn scaled(value: i32, numerator: i32, denominator: i32) -> i32 {
    ((i64::from(value.max(0)) * i64::from(numerator)) / i64::from(denominator))
        .clamp(0, i64::from(i32::MAX)) as i32
}

pub(super) fn incoming_damage_with_armour(
    player: &ZonePlayer,
    raw: i32,
    magic: bool,
    now: u64,
    subtract_armour: bool,
) -> i32 {
    // combat_stats is the complete trusted equipment/passive/buff projection.
    // Never add a mirrored MaxAC/MaxMAC or reduction buff a second time.
    let stats = player.combat_stats;
    let (min, max) = if magic {
        (stats.min_mac, stats.max_mac)
    } else {
        (stats.min_ac, stats.max_ac)
    };
    let mut armour = if subtract_armour {
        zone_roll_stat_range(
            min.max(0),
            max.max(min).max(0),
            now,
            player.object_id,
            if magic { 0x3AC } else { 0x4AC },
        )
    } else {
        0
    };
    let poison = zone_player_active_status(player, now);
    if poison & CRYSTAL_POISON_RED != 0 {
        armour = scaled(armour, 9, 10);
    }
    let power = if poison & CRYSTAL_POISON_STUN != 0 {
        scaled(raw, 6, 5)
    } else {
        raw.max(0)
    };
    let power = power.saturating_sub(scaled(
        power,
        stats.damage_reduction_percent.clamp(0, 100),
        100,
    ));
    power.saturating_sub(armour).max(0)
}

/// Use the full trusted attack projection without adding mirrored buffs twice.
/// The typed five-class consumer owns defence/reflect/Energy/critical/HP order.
pub(super) fn classic_melee_power(
    attacker: &ZonePlayer,
    fallback: i32,
    now: u64,
    area: Option<(Spell, u8, bool)>,
    spell: Spell,
    level: u8,
) -> i32 {
    let stats = attacker.combat_stats;
    let raw = if stats.has_authoritative_damage() {
        zone_roll_attack_power(
            stats.min_dc,
            stats.max_dc,
            stats.luck,
            now,
            attacker.object_id,
            0x5DC,
        )
    } else {
        zone_player_native_damage(attacker, fallback)
    };
    if area.is_some() {
        zone_melee_area_damage_base(raw, area)
    } else {
        zone_apply_melee_skill_damage(spell, level, raw)
    }
}

pub(super) fn energy_stats(player: &ZonePlayer) -> (i32, i32) {
    (
        player.combat_stats.energy_shield_percent.clamp(0, 100),
        player.combat_stats.energy_shield_hp_gain.max(0),
    )
}

impl ZoneRuntime {
    pub(super) fn five_class_legacy_admitted_hit_cleanup(
        &mut self,
        session: &SessionId,
        damage: i32,
        now: u64,
        reveal: bool,
    ) -> Vec<ZoneOutbound> {
        if damage <= 0 || !self.players.get(session).is_some_and(is_new_profession) {
            return Vec::new();
        }
        let mut out = if reveal {
            self.five_class_clear_stealth(session)
        } else {
            Vec::new()
        };
        let shields = self.five_class_shorten_damage_shields(session, damage, now);
        if !shields.is_empty() {
            out.push(ZoneOutbound::ToAll { packets: shields });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldEntityDisposition;
    use crate::runtime::zone::{
        ZoneChatProfile, ZoneJoin, ZoneMonsterDefense, ZoneMonsterSpawn, ZonePlayerCombatStats,
    };
    use mir2_protocol::{MirGender, UserItemStat};

    fn fixture(class: MirClass, armour: i32, energy: bool) -> (ZoneRuntime, SessionId) {
        let mut zone = ZoneRuntime::new_with_collision(
            ZoneKey::for_map("five-class-incoming"),
            ZoneCollision::unbounded(),
        );
        let session = SessionId::new("real-new-profession-victim");
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session.clone(),
            account_id: "victim-account".into(),
            character_index: 2,
            object_id: 101,
            name: "Victim".into(),
            class,
            gender: MirGender::Male,
            level: 50,
            hp: 100,
            max_hp: 200,
            mp: 100,
            map_file_name: "five-class-incoming".into(),
            position: Point { x: 10, y: 10 },
            direction: MirDirection::Right,
            chat_profile: ZoneChatProfile {
                attack_mode: 5,
                ..Default::default()
            },
            combat_stats: ZonePlayerCombatStats {
                min_ac: armour,
                max_ac: armour,
                min_mac: armour,
                max_mac: armour,
                damage_reduction_percent: 20,
                energy_shield_percent: if energy { 100 } else { 0 },
                energy_shield_hp_gain: if energy { 10 } else { 0 },
                ..Default::default()
            },
        }));
        // A real mirrored buff: its armour is already included in the full stats.
        zone.apply_native_player_buff(
            &session,
            8,
            10_000,
            vec![
                UserItemStat { stat: 0, value: 10 },
                UserItemStat { stat: 1, value: 10 },
                UserItemStat { stat: 2, value: 10 },
                UserItemStat { stat: 3, value: 10 },
            ],
            0,
        );
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: session.clone(),
            now_ms: 0,
            monster: ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: 900,
                name: "Scarecrow".into(),
                name_colour_argb: -1,
                image: 5,
                ai: 0,
                disposition: Some(WorldEntityDisposition::Hostile),
                level: 1,
                max_hp: 100,
                hp: 100,
                experience: 1,
                move_speed_ms: 60_000,
                attack_speed_ms: 60_000,
                friendly_guild: None,
                position: Point { x: 30, y: 30 },
                direction: MirDirection::Left,
                defense: ZoneMonsterDefense::default(),
                respawn: None,
                drops: vec![],
            },
        });
        assert!(zone.native_monsters[&900].owner_session_id.is_none());
        assert!(zone.objects.contains_key(&900));
        (zone, session)
    }

    fn join_attacker(zone: &mut ZoneRuntime, class: MirClass, dc: i32, mc: i32) -> SessionId {
        let session = SessionId::new("ordinary-classic-attacker");
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session.clone(),
            account_id: "classic-attacker-account".into(),
            character_index: 1,
            object_id: 202,
            name: "Attacker".into(),
            class,
            gender: MirGender::Male,
            level: 50,
            hp: 200,
            max_hp: 200,
            mp: 100,
            map_file_name: "five-class-incoming".into(),
            position: Point { x: 9, y: 10 },
            direction: MirDirection::Right,
            chat_profile: ZoneChatProfile {
                attack_mode: 5,
                ..Default::default()
            },
            combat_stats: ZonePlayerCombatStats {
                min_dc: dc,
                max_dc: dc,
                min_mc: mc,
                max_mc: mc,
                accuracy: 100,
                luck: 100,
                ..Default::default()
            },
        }));
        zone.handle(ZoneCommand::sync_player_combat_state(
            session.clone(),
            class,
            false,
            false,
            true,
            false,
            false,
            false,
        ));
        session
    }

    fn melee(attacker: &SessionId, now: u64) -> ZoneCommand {
        ZoneCommand::PlayerAttackObject {
            session_id: attacker.clone(),
            object_id: 101,
            direction: MirDirection::Right,
            spell: Spell::None as u8,
            level: 0,
            attack_type: 0,
            damage: 99_999,
            now_ms: now,
        }
    }

    fn incoming(session: &SessionId, magic: bool, damage: i32) -> PendingNativePlayerHit {
        PendingNativePlayerHit {
            ready_at_ms: 1000,
            attacker_object_id: 900,
            attacker_ai: 0,
            target_session_id: session.clone(),
            target_object_id: 101,
            damage,
            magic,
        }
    }

    fn amounts(out: &[ZoneOutbound]) -> (Vec<i32>, Vec<i32>) {
        let healed = out
            .iter()
            .filter_map(|x| match x {
                ZoneOutbound::PlayerHealed {
                    amount, settlement, ..
                } => {
                    assert!(settlement.is_some(), "real canonical HP settlement");
                    Some(*amount)
                }
                _ => None,
            })
            .collect();
        let damaged = out
            .iter()
            .filter_map(|x| match x {
                ZoneOutbound::PlayerDamaged {
                    damage, settlement, ..
                } => {
                    assert!(settlement.is_some(), "real canonical HP settlement");
                    Some(*damage)
                }
                _ => None,
            })
            .collect();
        (healed, damaged)
    }

    #[test]
    fn five_class_legacy_monster_impacts_use_full_armour_once_and_source_reduction_order() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            for magic in [false, true] {
                let (mut zone, session) = fixture(class, 20, false);
                // Actual existing wild-hit consumer and canonical Zone HP/outbounds.
                let out =
                    zone.resolve_pending_native_player_hit(incoming(&session, magic, 101), 1000);
                assert_eq!(zone.players[&session].hp, 39);
                assert_eq!(amounts(&out), (vec![], vec![61]));
            }
        }
    }

    #[test]
    fn five_class_legacy_positive_impacts_heal_from_full_equipment_stats_once_before_damage() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 20, true);
            assert_eq!(zone_player_buff_stat_total(&zone.players[&session], 125), 0);
            let out = zone.resolve_pending_native_player_hit(incoming(&session, false, 101), 1000);
            assert_eq!(zone.players[&session].hp, 49);
            assert_eq!(amounts(&out), (vec![10], vec![61]));
            let heal_index = out
                .iter()
                .position(|x| matches!(x, ZoneOutbound::PlayerHealed { .. }))
                .unwrap();
            let damage_index = out
                .iter()
                .position(|x| matches!(x, ZoneOutbound::PlayerDamaged { .. }))
                .unwrap();
            assert!(heal_index < damage_index);
        }
    }

    #[test]
    fn five_class_legacy_armour_block_has_no_heal_damage_or_false_hiding_cleanup() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 1000, true);
            zone.apply_native_player_buff(&session, 13, 10_000, vec![], 0);
            zone.apply_native_player_buff(&session, 25, 10_000, vec![], 0);
            zone.players.get_mut(&session).unwrap().hidden = true;
            let out = zone.resolve_pending_native_player_hit(incoming(&session, false, 101), 1000);
            assert!(out.is_empty());
            assert_eq!(zone.players[&session].hp, 100);
            assert!(zone.players[&session].hidden);
            assert!(zone.players[&session].buffs.contains_key(&13));
            assert_eq!(
                zone.players[&session].buffs[&25].expires_at_ms,
                Some(10_000)
            );
            assert!(zone
                .apply_native_player_pvp_damage(&session, 202, 0, 0, 1000)
                .is_none());
            assert_eq!(zone.players[&session].hp, 100);
        }
    }

    #[test]
    fn five_class_legacy_positive_hit_reveals_only_new_hiding_and_shortens_true_shield_deadline() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 20, false);
            for kind in [13, 14, CRYSTAL_HIDING_BUFF_TYPE, 114, 24, 25] {
                zone.apply_native_player_buff(&session, kind, 10_000, vec![], 0);
            }
            zone.players.get_mut(&session).unwrap().hidden = true;
            let out = zone.resolve_pending_native_player_hit(incoming(&session, false, 101), 1000);
            assert_eq!(zone.players[&session].hp, 39);
            assert_eq!(amounts(&out), (vec![], vec![61]));
            assert!(!zone.players[&session].buffs.contains_key(&13));
            assert!(!zone.players[&session].buffs.contains_key(&14));
            assert!(zone.players[&session]
                .buffs
                .contains_key(&CRYSTAL_HIDING_BUFF_TYPE));
            assert!(zone.players[&session].buffs.contains_key(&114));
            assert!(zone.players[&session].hidden);
            for kind in [24, 25] {
                assert_eq!(
                    zone.players[&session].buffs[&kind].expires_at_ms,
                    Some(6340)
                );
                assert_eq!(zone.players[&session].buffs[&kind].buff.expire_time, 5340);
            }
        }
    }

    #[test]
    fn five_class_legacy_unmitigated_dot_never_uses_armour_reduction_energy_or_reveal() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 1000, true);
            zone.apply_native_player_buff(&session, 13, 10_000, vec![], 0);
            zone.apply_native_player_buff(&session, 25, 10_000, vec![], 0);
            let out =
                zone.resolve_native_player_unmitigated_damage(incoming(&session, false, 17), 1000);
            assert_eq!(zone.players[&session].hp, 83);
            assert_eq!(amounts(&out), (vec![], vec![17]));
            assert!(zone.players[&session].buffs.contains_key(&13));
            assert_eq!(
                zone.players[&session].buffs[&25].expires_at_ms,
                Some(10_000)
            );
        }
    }

    #[test]
    fn five_class_legacy_classic_victim_retains_original_rounded_buff_only_defence() {
        let (mut zone, session) = fixture(MirClass::Warrior, 20, true);
        let out = zone.resolve_pending_native_player_hit(incoming(&session, false, 101), 1000);
        // Classic victim path remains 101 - full20 - mirrored10 and no equipment-only heal.
        assert_eq!(zone.players[&session].hp, 29);
        assert_eq!(amounts(&out), (vec![], vec![71]));
    }

    #[test]
    fn five_class_ordinary_warrior_primary_melee_uses_canonical_energy_reveal_and_clock() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 20, true);
            let attacker = join_attacker(&mut zone, MirClass::Warrior, 101, 0);
            zone.apply_native_player_buff(&session, 13, 10_000, vec![], 0);
            zone.apply_native_player_buff(&session, 25, 10_000, vec![], 0);
            zone.players.get_mut(&session).unwrap().hidden = true;
            let out = zone.handle(melee(&attacker, 1000));
            assert_eq!(zone.players[&session].hp, 49);
            assert_eq!(amounts(&out), (vec![10], vec![61]));
            assert!(!zone.players[&session].buffs.contains_key(&13));
            assert!(!zone.players[&session].hidden);
            assert_eq!(zone.players[&session].buffs[&25].expires_at_ms, Some(6340));
        }
    }

    #[test]
    fn five_class_ordinary_warrior_armour_block_precedes_critical_and_energy() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 100, true);
            let attacker = join_attacker(&mut zone, MirClass::Warrior, 101, 0);
            let stats = &mut zone.players.get_mut(&attacker).unwrap().combat_stats;
            stats.critical_rate = 18;
            stats.critical_damage = 50;
            let now = (1..100)
                .find(|now| {
                    zone_apply_player_critical(
                        101,
                        &zone.players[&attacker].combat_stats,
                        202,
                        *now,
                    ) > 101
                })
                .unwrap();
            let out = zone.handle(melee(&attacker, now));
            assert_eq!(zone.players[&session].hp, 100);
            assert_eq!(amounts(&out), (vec![], vec![]));
            assert!(out
                .iter()
                .any(|out| matches!(out, ZoneOutbound::ToAll { packets }
                if packets.iter().any(|p| matches!(p, ServerPacket::ObjectAttack { .. })))));
        }
    }

    #[test]
    fn five_class_ordinary_wizard_fireball_real_cast_queue_settles_at_600ms() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 20, true);
            // Source level-three FireBall power2 + MPower8/4*4 =10, plus MC91 =101.
            let attacker = join_attacker(&mut zone, MirClass::Wizard, 0, 91);
            let launch = zone.handle(ZoneCommand::PlayerCastMagic {
                session_id: attacker.clone(),
                object_id: 101,
                spell: Spell::FireBall,
                direction: MirDirection::Right,
                target: Point { x: 10, y: 10 },
                cast: true,
                level: 3,
                damage: 99_999,
                mp_cost: 9,
                cooldown_ms: 1800,
                now_ms: 50,
            });
            assert_eq!(zone.players[&attacker].mp, 91);
            assert_eq!(amounts(&launch), (vec![], vec![]));
            assert_eq!(zone.players[&session].hp, 100);
            let early = zone.tick(599);
            assert_eq!(amounts(&early), (vec![], vec![]));
            assert_eq!(zone.players[&session].hp, 100);
            let settled = zone.tick(600);
            assert_eq!(zone.players[&session].hp, 49);
            assert_eq!(amounts(&settled), (vec![10], vec![61]));
            assert_eq!(amounts(&zone.tick(601)), (vec![], vec![]));
        }
    }

    #[test]
    fn five_class_overkill_shield_loss_uses_full_damage_while_hp_receipt_is_clipped() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 20, false);
            zone.players.get_mut(&session).unwrap().hp = 30;
            zone.apply_native_player_buff(&session, 25, 10_000, vec![], 0);
            let out = zone.resolve_pending_native_player_hit(incoming(&session, false, 101), 1000);
            assert_eq!(zone.players[&session].hp, 0);
            assert_eq!(amounts(&out), (vec![], vec![30]));
            assert_eq!(zone.players[&session].buffs[&25].expires_at_ms, Some(6340));
            assert_eq!(zone.players[&session].buffs[&25].buff.expire_time, 5340);
        }
    }

    #[test]
    fn five_class_environment_struck_reveals_before_block_without_energy_or_shield_loss() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 1000, true);
            zone.apply_native_player_buff(&session, 13, 10_000, vec![], 0);
            zone.apply_native_player_buff(&session, 25, 10_000, vec![], 0);
            zone.players.get_mut(&session).unwrap().hidden = true;
            let out =
                zone.resolve_native_player_environment_struck(incoming(&session, false, 101), 1000);
            assert_eq!(zone.players[&session].hp, 100);
            assert_eq!(amounts(&out), (vec![], vec![]));
            assert!(!zone.players[&session].hidden);
            assert!(!zone.players[&session].buffs.contains_key(&13));
            assert_eq!(
                zone.players[&session].buffs[&25].expires_at_ms,
                Some(10_000)
            );
            assert!(!out.is_empty(), "real remove-buff/visibility outbound");
        }
    }

    #[test]
    fn five_class_environment_struck_preserves_original_safe_dead_gm_and_identity_gates() {
        for guard in 0..4 {
            let (mut zone, session) = fixture(MirClass::Assassin, 1000, true);
            zone.apply_native_player_buff(&session, 13, 10_000, vec![], 0);
            let player = zone.players.get_mut(&session).unwrap();
            player.hidden = true;
            match guard {
                0 => player.chat_profile.in_safe_zone = true,
                1 => player.dead = true,
                2 => player.combat_stats.gm_never_die = true,
                3 => player.object_id = 102,
                _ => unreachable!(),
            }
            let out =
                zone.resolve_native_player_environment_struck(incoming(&session, false, 101), 1000);
            assert!(out.is_empty());
            assert_eq!(zone.players[&session].hp, 100);
            assert!(zone.players[&session].hidden);
            assert!(zone.players[&session].buffs.contains_key(&13));
        }
    }

    #[test]
    fn five_class_classic_primary_positive_critical_amplifies_power_before_final_armour() {
        let (mut zone, session) = fixture(MirClass::Assassin, 20, false);
        let attacker = join_attacker(&mut zone, MirClass::Warrior, 101, 0);
        let player = zone.players.get_mut(&session).unwrap();
        player.hp = 1000;
        player.max_hp = 1000;
        let stats = &mut zone.players.get_mut(&attacker).unwrap().combat_stats;
        stats.critical_rate = 18;
        stats.critical_damage = 5;
        let now = (1000..1100)
            .find(|now| five_class_damage::critical_roll(&zone.players[&attacker], 101, *now, 1))
            .unwrap();
        let out = zone.handle(melee(&attacker, now));
        // 101-floor(101*20/100)=81; Source critical81*2-armour20=142 (not61*2).
        assert_eq!(zone.players[&session].hp, 858);
        assert_eq!(amounts(&out), (vec![], vec![142]));
    }

    #[test]
    fn five_class_classic_primary_uses_live_attack_bonus_and_reflect_authority() {
        let (mut zone, session) = fixture(MirClass::Archer, 20, false);
        let attacker = join_attacker(&mut zone, MirClass::Warrior, 101, 0);
        zone.players
            .get_mut(&attacker)
            .unwrap()
            .combat_stats
            .attack_bonus = 5;
        let out = zone.handle(melee(&attacker, 1000));
        assert_eq!(zone.players[&session].hp, 35);
        assert_eq!(amounts(&out), (vec![], vec![65]));
        let (mut zone, session) = fixture(MirClass::Archer, 20, false);
        let attacker = join_attacker(&mut zone, MirClass::Warrior, 101, 0);
        zone.players.get_mut(&session).unwrap().combat_stats.reflect = 100;
        let out = zone.handle(melee(&attacker, 1000));
        assert_eq!(zone.players[&session].hp, 100);
        assert_eq!(zone.players[&attacker].hp, 99);
        assert_eq!(amounts(&out), (vec![], vec![101]));
    }

    fn cast_vampirism(zone: &mut ZoneRuntime, attacker: &SessionId) -> Vec<ZoneOutbound> {
        zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: attacker.clone(),
            object_id: 101,
            spell: Spell::Vampirism,
            direction: MirDirection::Right,
            target: Point { x: 10, y: 10 },
            cast: true,
            level: 3,
            damage: 99_999,
            mp_cost: 65,
            cooldown_ms: 1800,
            now_ms: 50,
        })
    }

    #[test]
    fn five_class_real_vampirism_overkill_retains_unclipped_pool_and_original_regen_clocks() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 20, false);
            zone.players.get_mut(&session).unwrap().hp = 30;
            // Source Lv3 Vampirism adds24 power to MC77: raw101, reduction20%, armour20 ->61.
            let attacker = join_attacker(&mut zone, MirClass::Wizard, 0, 77);
            zone.players.get_mut(&attacker).unwrap().hp = 100;
            let launch = cast_vampirism(&mut zone, &attacker);
            assert_eq!(zone.players[&attacker].mp, 35);
            assert_eq!(amounts(&launch), (vec![], vec![]));
            assert_eq!(zone.owned_human_vampirism_amount_for_test(&attacker), 0);
            assert_eq!(amounts(&zone.tick(549)), (vec![], vec![]));
            assert_eq!(zone.players[&session].hp, 30);
            let hit = zone.tick(550);
            assert_eq!(amounts(&hit), (vec![], vec![30]));
            assert!(zone.players[&session].dead);
            assert_eq!(zone.owned_human_vampirism_amount_for_test(&attacker), 61);
            assert_eq!(amounts(&zone.tick(1550)), (vec![], vec![]));
            assert_eq!(zone.players[&attacker].hp, 100);
            assert_eq!(amounts(&zone.tick(1551)), (vec![10], vec![]));
            assert_eq!(zone.players[&attacker].hp, 110);
            assert_eq!(zone.owned_human_vampirism_amount_for_test(&attacker), 51);
            assert_eq!(amounts(&zone.tick(2051)), (vec![], vec![]));
            assert_eq!(amounts(&zone.tick(2052)), (vec![10], vec![]));
            assert_eq!(zone.owned_human_vampirism_amount_for_test(&attacker), 41);
        }
    }

    #[test]
    fn five_class_real_vampirism_blocked_and_retired_owner_or_life_never_credit_a_pool() {
        for guard in 0..3 {
            let (mut zone, session) = fixture(
                MirClass::Assassin,
                if guard == 0 { 1000 } else { 20 },
                false,
            );
            let attacker = join_attacker(&mut zone, MirClass::Wizard, 0, 77);
            cast_vampirism(&mut zone, &attacker);
            match guard {
                1 => {
                    zone.handle(ZoneCommand::Leave {
                        session_id: attacker.clone(),
                    });
                }
                2 => {
                    zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                        session_id: session.clone(),
                        hp: 0,
                        max_hp: 200,
                        mp: 100,
                        dead: true,
                    });
                    zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                        session_id: session.clone(),
                        hp: 100,
                        max_hp: 200,
                        mp: 100,
                        dead: false,
                    });
                }
                _ => {}
            }
            let hit = zone.tick(550);
            assert_eq!(amounts(&hit), (vec![], vec![]));
            assert_eq!(zone.players[&session].hp, 100);
            assert_eq!(zone.owned_human_vampirism_amount_for_test(&attacker), 0);
        }
    }

    #[test]
    fn five_class_real_entity_struck_clears_only_moon_dark_before_mac_block() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 1000, true);
            for kind in [2, 13, 14, 25, 114] {
                zone.apply_native_player_buff(&session, kind, 10_000, vec![], 0);
            }
            zone.players.get_mut(&session).unwrap().hidden = true;
            let source = zone.native_entity_monster_ref(900).unwrap();
            let target = zone.native_entity_player_ref(&session).unwrap();
            let (dealt, out) = zone.resolve_native_entity_hit(
                &source,
                &target,
                101,
                EntityDefence::MAC,
                true,
                1000,
            );
            assert_eq!(dealt, 0);
            assert_eq!(amounts(&out), (vec![], vec![]));
            assert_eq!(zone.players[&session].hp, 100);
            assert!(!zone.players[&session].buffs.contains_key(&13));
            assert!(!zone.players[&session].buffs.contains_key(&14));
            assert!(zone.players[&session].buffs.contains_key(&2));
            assert!(zone.players[&session].buffs.contains_key(&114));
            assert_eq!(
                zone.players[&session].buffs[&25].expires_at_ms,
                Some(10_000)
            );
            assert!(!out.is_empty());
        }
    }

    #[test]
    fn five_class_real_entity_struck_preserves_safe_gm_dead_and_typed_life_gates() {
        for guard in 0..4 {
            let (mut zone, session) = fixture(MirClass::Assassin, 1000, true);
            zone.apply_native_player_buff(&session, 13, 10_000, vec![], 0);
            let source = zone.native_entity_monster_ref(900).unwrap();
            let target = zone.native_entity_player_ref(&session).unwrap();
            let player = zone.players.get_mut(&session).unwrap();
            player.hidden = true;
            match guard {
                0 => player.chat_profile.in_safe_zone = true,
                1 => player.chat_profile.is_gm = true,
                2 => player.dead = true,
                3 => player.life_generation += 1,
                _ => unreachable!(),
            }
            let (dealt, out) = zone.resolve_native_entity_hit(
                &source,
                &target,
                101,
                EntityDefence::MAC,
                true,
                1000,
            );
            assert_eq!(dealt, 0);
            assert!(out.is_empty());
            assert_eq!(zone.players[&session].hp, 100);
            assert!(zone.players[&session].buffs.contains_key(&13));
        }
    }

    #[test]
    fn five_class_real_entity_attacked_reports_positive_damage_even_after_larger_energy_heal() {
        for class in [MirClass::Assassin, MirClass::Archer] {
            let (mut zone, session) = fixture(class, 0, true);
            let source = zone.native_entity_monster_ref(900).unwrap();
            let target = zone.native_entity_player_ref(&session).unwrap();
            //7-floor(7*20/100)=6. Energy heals10; net HP+4 is still an admitted6 hit.
            let (dealt, out) =
                zone.resolve_native_entity_hit(&source, &target, 7, EntityDefence::AC, false, 1000);
            assert_eq!(dealt, 6);
            assert_eq!(amounts(&out), (vec![10], vec![6]));
            assert_eq!(zone.players[&session].hp, 104);
            zone.players.get_mut(&session).unwrap().hp = 3;
            zone.players
                .get_mut(&session)
                .unwrap()
                .combat_stats
                .energy_shield_percent = 0;
            let (dealt, out) =
                zone.resolve_native_entity_hit(&source, &target, 7, EntityDefence::AC, false, 1001);
            assert_eq!(dealt, 6);
            assert_eq!(amounts(&out), (vec![], vec![3]));
            assert!(zone.players[&session].dead);
        }
    }
}
