//! Shared Assassin/Archer effects from Crystal HumanObject. Personal sessions
//! own learned identities and inventory; this module never mutates their
//! private monsters. All delayed actions retain live online/life proofs.
use super::super::five_class_receipts::ZoneFiveClassPracticeSpell;
use super::super::types::ZoneMonsterDefense;
use super::*;
use entity_combat::ZoneCombatEntityRef;
use owned_pet_combat::OwnedPetOwner;

const HASTE: u8 = 3;
const LIGHT_BODY: u8 = 8;
const MOON_LIGHT: u8 = 13;
const DARK_BODY: u8 = 14;
const CONCENTRATION: u8 = 15;
const ELEMENTAL_BARRIER: u8 = 25;
const BLEEDING: u16 = 128;
const DELAYED_EXPLOSION: u16 = 64;
const ORB_EXP: [u32; 4] = [50, 100, 150, 200];

#[derive(Debug, Clone, Default)]
pub(super) struct FiveClassSkillState {
    last_now_ms: u64,
    actors: BTreeMap<SessionId, FiveClassActor>,
    pending: Vec<PendingFiveClassSkill>,
    explosions: Vec<FiveClassExplosion>,
    bleeds: Vec<FiveClassBleed>,
    clones: BTreeMap<u32, FiveClassClone>,
    traps: BTreeMap<u32, FiveClassTrap>,
    vampires: Vec<FiveClassVampire>,
    cleanup_outbound: Vec<ZoneOutbound>,
}

#[derive(Debug, Clone)]
struct FiveClassActor {
    owner: OwnedPetOwner,
    /// Server-owned actual Attacked calls, including same-tick multi-hits.
    /// Retained only for the same live Human through a map transition.
    impact_sequence: u64,
    elements: u32,
    concentration_level: u8,
    concentration_interrupted_until: u64,
    concentration_enabled: bool,
    mp_eater_count: i32,
    mp_eater_armed: bool,
    hemorrhage_count: i32,
    hemorrhage_armed: bool,
    stealth_spell: Option<Spell>,
    stealth_level: u8,
}

impl FiveClassActor {
    fn new(owner: OwnedPetOwner) -> Self {
        Self {
            owner,
            impact_sequence: 0,
            elements: 0,
            concentration_level: 0,
            concentration_interrupted_until: 0,
            concentration_enabled: false,
            mp_eater_count: 0,
            mp_eater_armed: false,
            hemorrhage_count: 0,
            hemorrhage_armed: false,
            stealth_spell: None,
            stealth_level: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct FiveClassTarget {
    pub(super) entity: ZoneCombatEntityRef,
    pub(super) online: Option<OnlineOwner>,
    pub(super) launch_position: Point,
}

/// A live same-Human map transition, never a saved or client-supplied value.
#[derive(Debug, Clone)]
pub(crate) struct OnlineFiveClassTransfer {
    last_now_ms: u64,
    actor: FiveClassActor,
    vampires: Vec<FiveClassVampire>,
    buffs: BTreeMap<u8, super::super::types::ZonePlayerBuff>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::zone::{ZoneChatProfile, ZoneCommand, ZoneJoin, ZonePlayerCombatStats};
    use crate::WorldEntityDisposition;

    fn fixture(class: MirClass) -> (ZoneRuntime, SessionId) {
        fixture_in_map(class, "five-class-skills-unit")
    }

    fn fixture_in_map(class: MirClass, map: &str) -> (ZoneRuntime, SessionId) {
        // This private skill arena tests combat mechanics, not map loading.
        // Unknown ordinary maps remain unavailable in production.
        let mut zone = ZoneRuntime::new_with_collision(
            ZoneKey::for_map(map),
            super::super::super::ZoneCollision::unbounded(),
        );
        fixture_with_zone(class, map, zone, Point { x: 20, y: 20 })
    }

    fn cold_fixture() -> (ZoneRuntime, SessionId) {
        // Cold restore reconstructs ordinary terrain before verifying its root.
        // Keep the original relative actor/summon geometry on real map 0.
        let zone = ZoneRuntime::new(ZoneKey::for_map("0"));
        assert!(
            zone.has_available_collision(),
            "cold fixture requires real terrain"
        );
        for point in [
            Point { x: 322, y: 299 },
            Point { x: 323, y: 299 },
            Point { x: 323, y: 298 },
        ] {
            assert!(
                !zone.collision.is_blocked(&point),
                "ordinary cold-fixture cell must be open"
            );
        }
        fixture_with_zone(MirClass::Assassin, "0", zone, Point { x: 322, y: 299 })
    }

    fn fixture_with_zone(
        class: MirClass,
        map: &str,
        mut zone: ZoneRuntime,
        position: Point,
    ) -> (ZoneRuntime, SessionId) {
        let session = SessionId::new("native-owner");
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session.clone(),
            account_id: "native-owner".into(),
            character_index: 1,
            object_id: 101,
            name: "native-owner".into(),
            class,
            gender: mir2_protocol::MirGender::Male,
            level: 50,
            hp: 500,
            max_hp: 500,
            mp: 500,
            map_file_name: map.into(),
            position,
            direction: MirDirection::Right,
            chat_profile: ZoneChatProfile {
                attack_mode: 5,
                ..Default::default()
            },
            combat_stats: ZonePlayerCombatStats {
                min_dc: 40,
                max_dc: 40,
                min_mc: 40,
                max_mc: 40,
                min_ac: 2,
                max_ac: 2,
                accuracy: 30,
                agility: 0,
                max_mp: 500,
                attack_speed: 2,
                meditation_level: (class == MirClass::Archer).then_some(3),
                mp_eater_level: (class == MirClass::Assassin).then_some(0),
                hemorrhage_level: (class == MirClass::Assassin).then_some(0),
                five_class_learned_mask: (1u64 << 34) - 1,
                ..Default::default()
            },
        }));
        assert!(
            zone.owned_pet_owner(&session).is_some(),
            "fixture uses real online membership"
        );
        (zone, session)
    }

    fn monster(zone: &mut ZoneRuntime, id: u32, point: Point, level: u16) {
        zone.spawn_world_event_monster(
            &ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: id,
                name: "Yob".into(),
                name_colour_argb: -1,
                image: 1,
                ai: 0,
                disposition: Some(WorldEntityDisposition::Hostile),
                level,
                hp: 10_000,
                max_hp: 10_000,
                experience: 0,
                move_speed_ms: 1000,
                attack_speed_ms: 2000,
                friendly_guild: None,
                position: point,
                direction: MirDirection::Left,
                defense: Default::default(),
                respawn: None,
                drops: Vec::new(),
            },
            0,
        );
    }

    fn victim(zone: &mut ZoneRuntime, hp: i32) -> SessionId {
        victim_class(zone, hp, MirClass::Warrior)
    }

    fn victim_class(zone: &mut ZoneRuntime, hp: i32, class: MirClass) -> SessionId {
        let position = offset_point(&zone.players[&SessionId::new("native-owner")].position, MirDirection::Right, 1);
        let session = SessionId::new("native-victim");
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session.clone(),
            account_id: "native-victim".into(),
            character_index: 2,
            object_id: 202,
            name: "native-victim".into(),
            class,
            gender: mir2_protocol::MirGender::Male,
            level: 50,
            hp,
            max_hp: 500,
            mp: 500,
            map_file_name: zone.key.map_file_name.clone(),
            position,
            direction: MirDirection::Left,
            chat_profile: ZoneChatProfile {
                attack_mode: 5,
                ..Default::default()
            },
            combat_stats: ZonePlayerCombatStats {
                max_mp: 500,
                five_class_learned_mask: (1u64 << 34) - 1,
                // Source default PvpCanResistPoison=false bypasses this
                // for another Human; monster-origin resistance remains.
                poison_resist: 10,
                ..Default::default()
            },
        }));
        assert!(zone.owned_pet_owner(&session).is_some());
        session
    }

    fn practice_count(out: &[ZoneOutbound], spell: Spell) -> usize {
        out.iter()
            .filter(|item| {
                matches!(item, ZoneOutbound::FiveClassMagicPractice { receipt }
                if receipt.spell.spell() == spell)
            })
            .count()
    }

    fn cast(
        zone: &mut ZoneRuntime,
        session: &SessionId,
        spell: Spell,
        level: u8,
        target_id: u32,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let target = zone
            .five_class_target(target_id)
            .and_then(|t| zone.five_class_target_position(&t))
            .unwrap_or_else(|| zone.players[session].position.clone());
        zone.try_cast_five_class_magic(
            session,
            target_id,
            spell,
            MirDirection::Right,
            target,
            true,
            level,
            0,
            7,
            1000,
            0,
            now,
        )
        .expect("source-specialized spell")
    }

    fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
        out.iter()
            .flat_map(|o| outbound_packets(o).iter())
            .collect()
    }

    fn critical_effect_count(out: &[ZoneOutbound]) -> usize {
        packets(out).iter().filter(|p| matches!(p,
            ServerPacket::ObjectEffect { info } if info.effect == five_class_damage::CRITICAL_EFFECT
        )).count()
    }

    fn guaranteed_critical_time(zone: &ZoneRuntime, source: &SessionId, target: u32) -> u64 {
        (100..300)
            .find(|now| five_class_damage::critical_roll(&zone.players[source], target, *now, 1))
            .expect("a source-capped 18 CriticalRate has a real successful roll")
    }

    #[test]
    fn repeated_same_tick_impacts_roll_critical_independently_for_human_and_monster() {
        for player_target in [false, true] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            zone.players
                .get_mut(&session)
                .unwrap()
                .combat_stats
                .critical_rate = 10;
            zone.players
                .get_mut(&session)
                .unwrap()
                .combat_stats
                .critical_damage = 5;
            let victim_session = if player_target {
                Some(victim(&mut zone, 500))
            } else {
                monster(&mut zone, 900, Point { x: 21, y: 20 }, 50);
                None
            };
            let target_id = if player_target { 202 } else { 900 };
            let now = (100..300)
                .find(|now| {
                    let player = &zone.players[&session];
                    five_class_damage::critical_roll(player, target_id, *now, 1)
                        != five_class_damage::critical_roll(player, target_id, *now, 2)
                })
                .expect("independent source roll for each actual Attacked call");
            let owner = zone.owned_pet_owner(&session).unwrap();
            let target = zone.five_class_target(target_id).unwrap();
            let first = zone.five_class_hit(&owner, &target, 100, EntityDefence::AC, now);
            let second = zone.five_class_hit(&owner, &target, 100, EntityDefence::AC, now);
            assert_eq!([first.0.min(second.0), first.0.max(second.0)], [100, 200]);
            assert_eq!(
                critical_effect_count(&first.1) + critical_effect_count(&second.1),
                1
            );
            let all_packets = packets(&first.1).into_iter().chain(packets(&second.1));
            assert_eq!(
                all_packets
                    .filter(|packet| matches!(packet,
                ServerPacket::DamageIndicator { object_id, damage_type: 2, .. }
                    if *object_id == target_id))
                    .count(),
                1
            );
            assert_eq!(zone.five_class_skills.actors[&session].impact_sequence, 2);
            if let Some(victim) = victim_session {
                assert_eq!(zone.players[&victim].hp, 200);
                let damage_receipts = first
                    .1
                    .iter()
                    .chain(&second.1)
                    .filter(|out| {
                        matches!(
                            out,
                            ZoneOutbound::PlayerDamaged {
                                settlement: Some(_),
                                ..
                            }
                        )
                    })
                    .count();
                assert_eq!(damage_receipts, 2);
            } else {
                assert_eq!(zone.native_monsters[&900].hp, 9700);
            }
        }
    }

    #[test]
    fn actual_hit_resolution_keeps_source_final_damage_separate_from_truncated_hp() {
        for target_kind in ["player", "wild", "owned-clone"] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            let (target_id, raw, expected_applied) = match target_kind {
                "player" => {
                    victim(&mut zone, 20);
                    (202, 100, 20)
                }
                "wild" => {
                    monster(&mut zone, 900, Point { x: 21, y: 20 }, 50);
                    (900, 20_000, 10_000)
                }
                _ => {
                    let master = victim_class(&mut zone, 500, MirClass::Assassin);
                    let id = real_owned_clone(&mut zone, &master);
                    assert_eq!(zone.native_monsters[&id].hp, 1500);
                    (id, 2000, 1500)
                }
            };
            let owner = zone.owned_pet_owner(&session).unwrap();
            let target = zone.five_class_target(target_id).unwrap();
            let (settled, out) =
                zone.five_class_hit_with_resolution(&owner, &target, raw, EntityDefence::AC, 500);
            assert_eq!(settled.resolved_damage, raw, "target={target_kind}");
            assert_eq!(
                settled.applied_damage, expected_applied,
                "target={target_kind}"
            );
            assert_eq!(zone.five_class_skills.actors[&session].impact_sequence, 1);
            assert_eq!(critical_effect_count(&out), 0);
            if target_kind == "player" {
                let target_session = SessionId::new("native-victim");
                assert_eq!(zone.players[&target_session].hp, 0);
                assert!(zone.players[&target_session].dead);
                assert_eq!(
                    out.iter()
                        .filter(|out| matches!(out,
                    ZoneOutbound::PlayerDamaged { session_id, damage: 20, settlement: Some(_), .. }
                        if *session_id == target_session))
                        .count(),
                    1
                );
            } else {
                assert_eq!(zone.native_monsters[&target_id].hp, 0);
                assert!(zone.native_monsters[&target_id].dead);
            }
        }
    }

    #[test]
    fn rejected_blocked_and_reflected_impacts_have_no_original_target_resolution() {
        for outcome in ["blocked", "stale-owner", "reflected"] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            let target_session = victim(&mut zone, 500);
            let owner = zone.owned_pet_owner(&session).unwrap();
            let target = zone.five_class_target(202).unwrap();
            match outcome {
                "blocked" => {
                    let stats = &mut zone.players.get_mut(&target_session).unwrap().combat_stats;
                    stats.min_ac = 1000;
                    stats.max_ac = 1000;
                }
                "stale-owner" => {
                    zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                        session_id: session.clone(),
                        hp: 0,
                        max_hp: 500,
                        mp: 500,
                        dead: true,
                    });
                }
                _ => {
                    zone.players
                        .get_mut(&target_session)
                        .unwrap()
                        .combat_stats
                        .reflect = 100
                }
            }
            let (settled, out) =
                zone.five_class_hit_with_resolution(&owner, &target, 100, EntityDefence::AC, 100);
            assert_eq!(
                settled,
                five_class_damage::HitSettlement::default(),
                "{outcome}"
            );
            assert_eq!(zone.players[&target_session].hp, 500);
            assert!(!out.iter().any(|out| matches!(out,
                ZoneOutbound::PlayerDamaged { session_id, .. } if *session_id == target_session)));
            if outcome == "reflected" {
                assert_eq!(zone.players[&session].hp, 402);
                assert_eq!(zone.five_class_skills.actors[&session].impact_sequence, 1);
                assert_eq!(
                    zone.five_class_skills.actors[&target_session].impact_sequence,
                    1
                );
            } else if outcome == "blocked" {
                assert_eq!(zone.five_class_skills.actors[&session].impact_sequence, 1);
            } else {
                assert!(zone
                    .five_class_skills
                    .actors
                    .get(&session)
                    .is_none_or(|actor| actor.impact_sequence == 0));
            }
        }
    }

    #[test]
    fn armour_block_precedes_critical_energy_heal_stealth_removal_and_shield_duration() {
        for player_target in [false, true] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            let source = zone.players.get_mut(&session).unwrap();
            source.combat_stats.critical_rate = 18; // original Source cap
            source.combat_stats.critical_damage = 10;
            let target_id = if player_target { 202 } else { 900 };
            let victim_session = if player_target {
                let victim = victim(&mut zone, 100);
                let player = zone.players.get_mut(&victim).unwrap();
                player.combat_stats.min_ac = 75;
                player.combat_stats.max_ac = 75;
                player.combat_stats.energy_shield_percent = 100;
                player.combat_stats.energy_shield_hp_gain = 30;
                player.hidden = true;
                zone.apply_native_player_buff(&victim, MOON_LIGHT, 10_000, vec![], 0);
                zone.apply_native_player_buff(
                    &victim,
                    CRYSTAL_MAGIC_SHIELD_BUFF_TYPE,
                    10_000,
                    vec![],
                    0,
                );
                Some(victim)
            } else {
                monster(&mut zone, 900, Point { x: 21, y: 20 }, 50);
                let monster = zone.native_monsters.get_mut(&900).unwrap();
                monster.defense.min_ac = 75;
                monster.defense.max_ac = 75;
                None
            };
            let now = guaranteed_critical_time(&zone, &session, target_id);
            let owner = zone.owned_pet_owner(&session).unwrap();
            let target = zone.five_class_target(target_id).unwrap();
            let (damage, out) = zone.five_class_hit(&owner, &target, 50, EntityDefence::AC, now);
            assert_eq!(
                damage, 0,
                "critical amplification cannot bypass a Source armour block"
            );
            assert_eq!(critical_effect_count(&out), 0);
            assert!(packets(&out).iter().any(|packet| matches!(packet,
                ServerPacket::DamageIndicator { damage: 0, damage_type: 1, object_id }
                    if *object_id == target_id)));
            assert!(!out.iter().any(|out| matches!(
                out,
                ZoneOutbound::PlayerDamaged { .. }
                    | ZoneOutbound::PlayerHealed { .. }
                    | ZoneOutbound::FiveClassMagicPractice { .. }
            )));
            if let Some(victim) = victim_session {
                assert_eq!(zone.players[&victim].hp, 100);
                assert!(zone.players[&victim].hidden);
                assert!(zone.players[&victim].buffs.contains_key(&MOON_LIGHT));
                assert_eq!(
                    zone.players[&victim].buffs[&CRYSTAL_MAGIC_SHIELD_BUFF_TYPE].expires_at_ms,
                    Some(10_000)
                );
                assert_eq!(zone.owned_pet_brown_until(&zone.players[&session]), 0);
            } else {
                assert_eq!(zone.native_monsters[&900].hp, 10_000);
            }
        }
    }

    #[test]
    fn source_zero_critical_damage_keeps_real_critical_effect_without_invented_bonus() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        zone.players
            .get_mut(&session)
            .unwrap()
            .combat_stats
            .critical_rate = 18;
        let victim = victim(&mut zone, 500);
        let stats = &mut zone.players.get_mut(&victim).unwrap().combat_stats;
        stats.min_ac = 20;
        stats.max_ac = 20;
        let now = guaranteed_critical_time(&zone, &session, 202);
        let owner = zone.owned_pet_owner(&session).unwrap();
        let target = zone.five_class_target(202).unwrap();
        let (damage, out) = zone.five_class_hit(&owner, &target, 100, EntityDefence::AC, now);
        assert_eq!(
            damage, 80,
            "Source CriticalDamage0 adds exactly zero before AC20"
        );
        assert_eq!(zone.players[&victim].hp, 420);
        assert_eq!(critical_effect_count(&out), 1);
        assert!(packets(&out).iter().any(|p| matches!(
            p,
            ServerPacket::DamageIndicator {
                damage: 80,
                damage_type: 2,
                object_id: 202
            }
        )));
    }

    #[test]
    fn delayed_skill_uses_current_critical_stats_at_real_impact_without_a_launch_roll() {
        let (mut zone, session) = fixture(MirClass::Archer);
        monster(&mut zone, 900, Point { x: 21, y: 20 }, 50);
        let mut equipped = zone.players[&session].clone();
        equipped.combat_stats.critical_rate = 18;
        equipped.combat_stats.critical_damage = 5;
        let impact_at = (700..900)
            .find(|now| five_class_damage::critical_roll(&equipped, 900, *now, 1))
            .unwrap();
        let out = cast(
            &mut zone,
            &session,
            Spell::VampireShot,
            2,
            900,
            impact_at - 550,
        );
        let raw = zone.five_class_skills.pending[0].damage;
        assert!(raw > 0);
        assert_eq!(zone.native_monsters[&900].hp, 10_000);
        assert_eq!(critical_effect_count(&out), 0);
        let stats = &mut zone.players.get_mut(&session).unwrap().combat_stats;
        stats.critical_rate = 18;
        stats.critical_damage = 5;
        let out = zone.tick_five_class_skills(impact_at);
        assert_eq!(zone.native_monsters[&900].hp, 10_000 - raw * 2);
        assert_eq!(critical_effect_count(&out), 1);
        assert_eq!(practice_count(&out, Spell::VampireShot), 1);
    }

    #[test]
    fn source_magic_resist_is_checked_before_critical_but_none_defence_bypasses_it() {
        for defence in [EntityDefence::MAC, EntityDefence::None] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            let victim = victim(&mut zone, 500);
            zone.players
                .get_mut(&victim)
                .unwrap()
                .combat_stats
                .magic_resist = 2;
            let now = (100..300)
                .find(|now| crate::runtime::combat::crystal_accuracy_roll(*now, 101, 202, 10) < 2)
                .unwrap();
            let owner = zone.owned_pet_owner(&session).unwrap();
            let target = zone.five_class_target(202).unwrap();
            let (damage, out) = zone.five_class_hit(&owner, &target, 100, defence, now);
            let magic = five_class_damage::magical(defence);
            assert_eq!(damage, if magic { 0 } else { 100 });
            assert_eq!(zone.players[&victim].hp, if magic { 500 } else { 400 });
            if magic {
                assert!(packets(&out).iter().any(|p| matches!(
                    p,
                    ServerPacket::DamageIndicator {
                        damage: 0,
                        damage_type: 1,
                        object_id: 202
                    }
                )));
                assert!(!out.iter().any(|o| matches!(
                    o,
                    ZoneOutbound::PlayerDamaged { .. }
                        | ZoneOutbound::PlayerHealed { .. }
                        | ZoneOutbound::FiveClassMagicPractice { .. }
                )));
            }
        }
    }

    #[test]
    fn human_armour_and_attack_bonus_use_trusted_projection_once_for_ac_and_mac() {
        for defence in [EntityDefence::AC, EntityDefence::MAC] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            zone.players
                .get_mut(&session)
                .unwrap()
                .combat_stats
                .attack_bonus = 7;
            let victim = victim(&mut zone, 500);
            let stats = &mut zone.players.get_mut(&victim).unwrap().combat_stats;
            stats.min_ac = 30;
            stats.max_ac = 30;
            stats.min_mac = 30;
            stats.max_mac = 30;
            // Full trusted totals include base20 + this real mirrored buff10.
            zone.apply_native_player_buff(
                &victim,
                63,
                10_000,
                vec![
                    UserItemStat { stat: 0, value: 10 },
                    UserItemStat { stat: 1, value: 10 },
                    UserItemStat { stat: 2, value: 10 },
                    UserItemStat { stat: 3, value: 10 },
                ],
                0,
            );
            let owner = zone.owned_pet_owner(&session).unwrap();
            let target = zone.five_class_target(202).unwrap();
            let (damage, out) = zone.five_class_hit(&owner, &target, 100, defence, 100);
            assert_eq!(damage, 77, "100 + AttackBonus7 - full armour30, not buff40");
            assert_eq!(zone.players[&victim].hp, 423);
            assert_eq!(critical_effect_count(&out), 0);
            assert!(out.iter().any(|o| matches!(o,
                ZoneOutbound::PlayerDamaged { damage: 77, settlement: Some(receipt), .. }
                    if receipt.hp_before == 500 && receipt.hp_after == 423)));
        }
    }

    #[test]
    fn source_damage_reduction_energy_heal_critical_and_shield_leases_follow_real_order() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let source = zone.players.get_mut(&session).unwrap();
        source.combat_stats.critical_rate = 18;
        source.combat_stats.critical_damage = 5;
        let victim = victim(&mut zone, 300);
        let stats = &mut zone.players.get_mut(&victim).unwrap().combat_stats;
        stats.min_ac = 5;
        stats.max_ac = 5;
        stats.damage_reduction_percent = 20;
        stats.energy_shield_percent = 100;
        stats.energy_shield_hp_gain = 30;
        let now = guaranteed_critical_time(&zone, &session, 202);
        for kind in [CRYSTAL_MAGIC_SHIELD_BUFF_TYPE, ELEMENTAL_BARRIER] {
            zone.apply_native_player_buff(
                &victim,
                kind,
                10_000,
                vec![UserItemStat {
                    stat: CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT,
                    value: 10,
                }],
                now,
            );
        }
        // A real EnergyShield mirror exists as well as its trusted projection.
        // The canonical damage hook must not trigger this healing a second time.
        zone.apply_native_player_buff(
            &victim,
            20,
            10_000,
            vec![
                UserItemStat {
                    stat: CRYSTAL_STAT_ENERGY_SHIELD_PERCENT,
                    value: 100,
                },
                UserItemStat {
                    stat: CRYSTAL_STAT_ENERGY_SHIELD_HP_GAIN,
                    value: 30,
                },
            ],
            now,
        );
        let owner = zone.owned_pet_owner(&session).unwrap();
        let target = zone.five_class_target(202).unwrap();
        let (damage, out) = zone.five_class_hit(&owner, &target, 101, EntityDefence::AC, now);
        // Source subtracts floor(101*.20), then doubles81, then subtracts AC5.
        assert_eq!(damage, 157);
        assert_eq!(
            zone.players[&victim].hp, 173,
            "300 + one heal30 - actual damage157"
        );
        assert_eq!(critical_effect_count(&out), 1);
        let healing = out
            .iter()
            .filter_map(|o| match o {
                ZoneOutbound::PlayerHealed {
                    amount,
                    settlement: Some(receipt),
                    ..
                } => Some((
                    *amount,
                    receipt.hp_before,
                    receipt.hp_after,
                    receipt.receipt_sequence,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(healing.len(), 1);
        assert_eq!(healing[0].0, 30);
        assert_eq!((healing[0].1, healing[0].2), (300, 330));
        let damage_receipts = out
            .iter()
            .filter_map(|o| match o {
                ZoneOutbound::PlayerDamaged {
                    damage,
                    settlement: Some(receipt),
                    ..
                } => Some((
                    *damage,
                    receipt.hp_before,
                    receipt.hp_after,
                    receipt.receipt_sequence,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(damage_receipts.len(), 1);
        assert_eq!(damage_receipts[0].0, 157);
        assert_eq!((damage_receipts[0].1, damage_receipts[0].2), (330, 173));
        assert_eq!(damage_receipts[0].3, healing[0].3 + 1);
        for kind in [CRYSTAL_MAGIC_SHIELD_BUFF_TYPE, ELEMENTAL_BARRIER] {
            let buff = &zone.players[&victim].buffs[&kind];
            assert_eq!(buff.expires_at_ms, Some(now + 580)); // 10000 - 157*60
            assert_eq!(buff.buff.expire_time, 580);
            assert!(packets(&out).iter().any(|packet| matches!(packet,
                ServerPacket::AddBuff { buff } if buff.buff_type == kind
                    && buff.expire_time == 580
                    && buff.stats == vec![UserItemStat {
                        stat: CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT, value: 10 }])));
        }
    }

    #[test]
    fn reduction_can_block_before_energy_and_shortens_no_shield_lease() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let victim = victim(&mut zone, 100);
        let stats = &mut zone.players.get_mut(&victim).unwrap().combat_stats;
        stats.min_ac = 51;
        stats.max_ac = 51;
        stats.damage_reduction_percent = 50;
        stats.energy_shield_percent = 100;
        stats.energy_shield_hp_gain = 30;
        zone.apply_native_player_buff(
            &victim,
            ELEMENTAL_BARRIER,
            10_000,
            vec![UserItemStat {
                stat: CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT,
                value: 50,
            }],
            0,
        );
        let owner = zone.owned_pet_owner(&session).unwrap();
        let target = zone.five_class_target(202).unwrap();
        let (damage, out) = zone.five_class_hit(&owner, &target, 101, EntityDefence::AC, 100);
        assert_eq!(
            damage, 0,
            "reduced51 equals AC51, Source blocks before EnergyShield"
        );
        assert_eq!(zone.players[&victim].hp, 100);
        assert_eq!(
            zone.players[&victim].buffs[&ELEMENTAL_BARRIER].expires_at_ms,
            Some(10_000)
        );
        assert!(!out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::PlayerHealed { .. })));
    }

    #[test]
    fn bleeding_and_green_change_hp_without_rolling_critical_again() {
        for mask in [BLEEDING, CRYSTAL_POISON_GREEN] {
            for player_target in [false, true] {
                let (mut zone, session) = fixture(MirClass::Assassin);
                let source = zone.players.get_mut(&session).unwrap();
                source.combat_stats.critical_rate = 18;
                source.combat_stats.critical_damage = 10;
                let target_id = if player_target { 202 } else { 900 };
                let victim_session = if player_target {
                    Some(victim(&mut zone, 500))
                } else {
                    monster(&mut zone, 900, Point { x: 21, y: 20 }, 50);
                    None
                };
                let now = guaranteed_critical_time(&zone, &session, target_id);
                let owner = zone.owned_pet_owner(&session).unwrap();
                let target = zone.five_class_target(target_id).unwrap();
                if mask == BLEEDING {
                    zone.five_class_apply_bleeding(&owner, &target, 20, 2, now);
                } else {
                    zone.five_class_apply_poison(
                        &owner,
                        &target,
                        CRYSTAL_POISON_GREEN,
                        20,
                        2,
                        1000,
                        now,
                    );
                }
                let out = if mask == CRYSTAL_POISON_GREEN && player_target {
                    zone.tick_owned_human_poisons(now + 1001)
                } else {
                    zone.tick_five_class_bleeds(now + 1001)
                };
                if let Some(victim) = victim_session {
                    assert_eq!(zone.players[&victim].hp, 480);
                    assert!(out.iter().any(|o| matches!(
                        o,
                        ZoneOutbound::PlayerDamaged {
                            damage: 20,
                            settlement: Some(_),
                            ..
                        }
                    )));
                } else {
                    assert_eq!(zone.native_monsters[&900].hp, 9980);
                }
                assert_eq!(critical_effect_count(&out), 0);
                assert!(!packets(&out)
                    .iter()
                    .any(|p| matches!(p, ServerPacket::DamageIndicator { damage_type: 2, .. })));
                assert!(zone
                    .five_class_skills
                    .actors
                    .get(&session)
                    .is_none_or(|actor| actor.impact_sequence == 0));
            }
        }
    }

    #[test]
    fn reflection_uses_current_reverse_permission_and_reverse_typed_damage() {
        for reverse_allowed in [false, true] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            let victim = victim(&mut zone, 500);
            let player = zone.players.get_mut(&victim).unwrap();
            player.combat_stats.reflect = 100;
            player.combat_stats.min_ac = 9999;
            player.combat_stats.max_ac = 9999;
            if !reverse_allowed {
                player.chat_profile.attack_mode = 0;
            }
            let owner = zone.owned_pet_owner(&session).unwrap();
            let target = zone.five_class_target(202).unwrap();
            let reverse_target = zone.five_class_target(101).unwrap();
            assert_eq!(
                zone.five_class_target_hostile(&victim, &reverse_target, 100),
                reverse_allowed
            );
            let (damage, out) = zone.five_class_hit(&owner, &target, 100, EntityDefence::AC, 100);
            assert_eq!(
                damage, 0,
                "Reflect cancels original impact before its armour block"
            );
            assert_eq!(zone.players[&victim].hp, 500);
            assert_eq!(
                zone.players[&session].hp,
                if reverse_allowed { 402 } else { 500 }
            );
            assert_eq!(packets(&out).iter().filter(|packet| matches!(packet,
                ServerPacket::ObjectEffect { info } if info.effect == five_class_damage::REFLECT_EFFECT
                    && info.object_id == 202)).count(), usize::from(reverse_allowed));
            assert_eq!(
                out.iter()
                    .filter(|o| matches!(o,
                ZoneOutbound::PlayerDamaged { session_id, damage: 98, settlement: Some(_), .. }
                    if *session_id == session))
                    .count(),
                usize::from(reverse_allowed)
            );
            assert!(!out
                .iter()
                .any(|o| matches!(o, ZoneOutbound::FiveClassMagicPractice { .. })));
        }
    }

    #[test]
    fn invalid_two_sided_full_reflection_is_bounded_without_invented_damage() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let victim = victim(&mut zone, 500);
        zone.players.get_mut(&session).unwrap().combat_stats.reflect = 100;
        zone.players.get_mut(&victim).unwrap().combat_stats.reflect = 100;
        let owner = zone.owned_pet_owner(&session).unwrap();
        let target = zone.five_class_target(202).unwrap();
        let (damage, out) = zone.five_class_hit(&owner, &target, 100, EntityDefence::AC, 100);
        assert_eq!(damage, 0);
        assert_eq!(zone.players[&session].hp, 500);
        assert_eq!(zone.players[&victim].hp, 500);
        assert_eq!(packets(&out).iter().filter(|p| matches!(p,
            ServerPacket::ObjectEffect { info } if info.effect == five_class_damage::REFLECT_EFFECT
        )).count(), usize::from(five_class_damage::MAX_REFLECT_CHAIN));
        assert!(!out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { .. })));
    }

    #[test]
    fn owned_clone_uses_one_impact_armour_and_critical_pass_and_real_life_authority() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let clone_owner = victim_class(&mut zone, 500, MirClass::Assassin);
        let owner_stats = &mut zone.players.get_mut(&clone_owner).unwrap().combat_stats;
        owner_stats.min_ac = 30;
        owner_stats.max_ac = 30;
        // Source DarkBody accepts a live friendly/self target, never null.
        cast(&mut zone, &clone_owner, Spell::DarkBody, 2, 202, 100);
        let clone_id = *zone.five_class_skills.clones.keys().next().unwrap();
        let source = zone.players.get_mut(&session).unwrap();
        source.combat_stats.critical_rate = 18;
        source.combat_stats.critical_damage = 5;
        let now = guaranteed_critical_time(&zone, &session, clone_id);
        let owner = zone.owned_pet_owner(&session).unwrap();
        let target = zone.five_class_target(clone_id).unwrap();
        assert!(zone.five_class_target_hostile(&session, &target, now));
        let (damage, out) = zone.five_class_hit(&owner, &target, 100, EntityDefence::AC, now);
        assert_eq!(
            damage, 170,
            "Source raw100 doubles before AC30, never a second armour pass"
        );
        assert_eq!(zone.native_monsters[&clone_id].hp, 1330);
        assert_eq!(critical_effect_count(&out), 1);
        // A true owner death invalidates the clone authority retained above.
        zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
            session_id: clone_owner,
            hp: 0,
            max_hp: 500,
            mp: 500,
            dead: true,
        });
        let before = zone.native_monsters.get(&clone_id).map(|m| m.hp);
        let (damage, out) = zone.five_class_hit(&owner, &target, 100, EntityDefence::AC, now + 1);
        assert_eq!(damage, 0);
        assert_eq!(zone.native_monsters.get(&clone_id).map(|m| m.hp), before);
        assert!(!out.iter().any(|o| matches!(
            o,
            ZoneOutbound::PlayerDamaged { .. } | ZoneOutbound::FiveClassMagicPractice { .. }
        )));
    }

    #[test]
    fn unlearned_or_wrong_class_spell_cannot_debit_mana_or_emit_practice() {
        for class in [MirClass::Archer, MirClass::Assassin] {
            let (mut zone, session) = fixture(class);
            zone.players
                .get_mut(&session)
                .unwrap()
                .combat_stats
                .five_class_learned_mask &= !ZoneFiveClassPracticeSpell::Haste.learned_bit();
            let output = cast(&mut zone, &session, Spell::Haste, 2, 0, 100);
            assert_eq!(zone.players[&session].mp, 500);
            assert!(zone.five_class_skills.pending.is_empty());
            assert!(!output
                .iter()
                .any(|o| matches!(o, ZoneOutbound::FiveClassMagicPractice { .. })));
        }
    }

    #[test]
    fn haste_uses_source_half_second_delay_duration_and_cooldown() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        cast(&mut zone, &session, Spell::Haste, 2, 0, 100);
        assert_eq!(zone.players[&session].mp, 493);
        assert!(!zone.players[&session].buffs.contains_key(&HASTE));
        assert!(zone.tick_five_class_skills(599).is_empty());
        let out = zone.tick_five_class_skills(600);
        let buff = &zone.players[&session].buffs[&HASTE];
        assert_eq!(buff.expires_at_ms, Some(55_600));
        assert!(buff
            .buff
            .stats
            .iter()
            .any(|stat| stat.stat == 14 && stat.value == 6));
        assert!(out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::FiveClassMagicPractice { .. })));
        cast(&mut zone, &session, Spell::Haste, 2, 0, 700);
        assert_eq!(zone.players[&session].mp, 493);
        assert_eq!(
            zone.players[&session].buffs[&HASTE].expires_at_ms,
            Some(55_600)
        );
        // Trusted projection is buff-complete; native Haste must not be added a second time.
        zone.players
            .get_mut(&session)
            .unwrap()
            .combat_stats
            .attack_speed = 8;
        zone.players.get_mut(&session).unwrap().level = 10;
        assert_eq!(source_attack_delay(&zone.players[&session]), 780);
    }

    #[test]
    fn stealth_damage_bonus_is_consumed_once_and_expired_buff_cannot_supply_it() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        cast(&mut zone, &session, Spell::MoonLight, 2, 0, 100);
        assert!(zone.players[&session].hidden);
        let magic = crystal_magic_by_spell("MoonLight").unwrap();
        assert_eq!(
            zone.take_five_class_stealth_attack(&session, 101).0,
            crate::runtime::skills::crystal_magic_get_power(&magic, 2)
        );
        assert!(!zone.players[&session].hidden);
        assert_eq!(zone.take_five_class_stealth_attack(&session, 102).0, 0);
        cast(&mut zone, &session, Spell::MoonLight, 2, 0, 2100);
        let expiry = zone.players[&session].buffs[&MOON_LIGHT]
            .expires_at_ms
            .unwrap();
        assert_eq!(zone.take_five_class_stealth_attack(&session, expiry).0, 0);
    }

    #[test]
    fn expired_moonlight_reveals_the_authoritative_player_without_an_attack() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        cast(&mut zone, &session, Spell::MoonLight, 2, 0, 100);
        let expiry = zone.players[&session].buffs[&MOON_LIGHT]
            .expires_at_ms
            .unwrap();
        zone.tick_five_class_skills(expiry - 1);
        assert!(zone.players[&session].hidden);
        let out = zone.tick_five_class_skills(expiry);
        assert!(!zone.players[&session].hidden);
        assert!(packets(&out).iter().any(|p| matches!(
            p,
            ServerPacket::ObjectHidden {
                object_id: 101,
                hidden: false
            }
        )));
        assert!(!zone.players[&session].buffs.contains_key(&MOON_LIGHT));
    }

    #[test]
    fn poison_sword_hits_only_source_five_cell_front_arc_and_ticks_each_second() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let center = zone.players[&session].position.clone();
        for (i, offset) in (-1..=3).enumerate() {
            monster(
                &mut zone,
                900 + i as u32,
                offset_point(
                    &center,
                    zone_rotated_direction(MirDirection::Right, offset),
                    1,
                ),
                10,
            );
        }
        monster(
            &mut zone,
            999,
            offset_point(&center, zone_rotated_direction(MirDirection::Right, 4), 1),
            10,
        );
        cast(&mut zone, &session, Spell::PoisonSword, 2, 0, 100);
        for id in 900..=904 {
            assert_ne!(
                native_monster_current_poison(&zone.native_monsters[&id]) & CRYSTAL_POISON_GREEN,
                0
            );
        }
        assert_eq!(
            native_monster_current_poison(&zone.native_monsters[&999]) & CRYSTAL_POISON_GREEN,
            0
        );
        zone.tick_five_class_skills(100);
        let hp = zone.native_monsters[&900].hp;
        assert!(hp < 10_000);
        zone.tick_five_class_skills(1099);
        assert_eq!(zone.native_monsters[&900].hp, hp);
        zone.tick_five_class_skills(1100);
        assert!(zone.native_monsters[&900].hp < hp);
        assert_eq!(zone.native_monsters[&999].hp, 10_000);
    }

    #[test]
    fn trap_rejects_too_high_monster_and_applies_actual_control_after_delay() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        monster(&mut zone, 900, Point { x: 21, y: 20 }, 52);
        cast(&mut zone, &session, Spell::Trap, 2, 900, 100);
        assert_eq!(zone.players[&session].mp, 500);
        zone.native_monsters.get_mut(&900).unwrap().level = 51;
        cast(&mut zone, &session, Spell::Trap, 2, 900, 200);
        assert_eq!(zone.players[&session].mp, 493);
        assert_eq!(zone.native_monsters[&900].control_until_ms, 0);
        let settled = zone.tick_five_class_skills(700);
        assert_eq!(zone.native_monsters[&900].control_until_ms, 60_700);
        assert!(zone.objects.values().any(|object| matches!(object.packet, ServerPacket::ObjectSpell { ref info } if info.spell == Spell::Trap)));
        assert!(zone.objects.values().any(|object| {
            matches!(object.packet, ServerPacket::ObjectSpell { ref info } if info.spell == Spell::Trap)
                && object.expires_at_ms == Some(60_700)
                && matches!(&object.packet, ServerPacket::ObjectSpell { info } if !info.param)
        }));
        assert!(packets(&settled).iter().any(|packet| matches!(packet,
            ServerPacket::ObjectSpell { info } if info.spell == Spell::Trap
        )));
    }

    fn assert_object_retired(zone: &ZoneRuntime, id: u32, position: &Point) {
        assert!(
            !zone.objects.contains_key(&id),
            "world Node must be retired"
        );
        assert!(
            !zone.object_grid.neighbors(position).contains(&id),
            "spatial index cannot retain a ghost object"
        );
        assert!(zone
            .players
            .values()
            .all(|player| !player.visible_object_ids.contains(&id)));
    }

    fn only_trap(zone: &ZoneRuntime) -> u32 {
        assert_eq!(zone.five_class_skills.traps.len(), 1);
        *zone.five_class_skills.traps.keys().next().unwrap()
    }

    fn removed(out: &[ZoneOutbound], id: u32) -> bool {
        packets(out).iter().any(|packet| {
            matches!(packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == id)
        })
    }

    fn real_owned_clone(zone: &mut ZoneRuntime, master: &SessionId) -> u32 {
        let master_id = zone.players[master].object_id;
        let born = cast(zone, master, Spell::DarkBody, 2, master_id, 100);
        let id = *zone
            .five_class_skills
            .clones
            .iter()
            .find(|(_, clone)| clone.owner.online.session_id == *master)
            .map(|(id, _)| id)
            .expect("actual DarkBody creates a live clone");
        assert!(!zone.native_monsters[&id].hostile_to_player);
        assert!(
            zone.player_can_attack_owned_pet(master, id, 100),
            "real current owned life admits its own All-mode master"
        );
        assert!(packets(&born).iter().any(|packet| matches!(packet,
            ServerPacket::ObjectPlayer { info } if info.object_id == id)));
        id
    }

    fn real_owned_bone_familiar(zone: &mut ZoneRuntime, master: &SessionId) -> u32 {
        assert_eq!(zone.players[master].class, MirClass::Taoist);
        let target = offset_point(&zone.players[master].position, MirDirection::Up, 1);
        let launched = zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: master.clone(),
            object_id: 0,
            spell: Spell::SummonSkeleton,
            direction: MirDirection::Up,
            target,
            cast: true,
            level: 2,
            damage: 0,
            mp_cost: 7,
            cooldown_ms: 1000,
            now_ms: 100,
        });
        assert!(packets(&launched).iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::SummonSkeleton,
                cast: true,
                ..
            }
        )));
        zone.resolve_pending_native_summons(600);
        let id = *zone
            .native_monsters
            .iter()
            .find(|(_, monster)| {
                monster.name == "BoneFamiliar" && monster.owner_session_id.as_ref() == Some(master)
            })
            .map(|(id, _)| id)
            .expect("real Source summon path creates BoneFamiliar");
        assert!(
            zone.player_can_attack_owned_pet(master, id, 100),
            "real current owned life admits its own All-mode master"
        );
        assert_eq!(
            zone.objects[&id].expires_at_ms, None,
            "Taoist's permanent pet can legitimately survive the full Trap lease"
        );
        id
    }

    #[test]
    fn trap_controls_real_enemy_clone_clears_its_target_and_releases_on_actual_attacked() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let master = victim_class(&mut zone, 500, MirClass::Assassin);
        zone.players.get_mut(&master).unwrap().chat_profile.pet_mode = 4;
        let pet = real_owned_clone(&mut zone, &master);
        let target = zone.five_class_target(pet).unwrap();
        let point = zone.native_monsters[&pet].position.clone();
        zone.focus_owned_pets(&master, 101, 150);
        assert_eq!(
            zone.selected_owned_pet_target(pet, 150).unwrap().object_id,
            101
        );
        assert!(zone.owned_pet_can_move_now(pet));
        let launch = cast(&mut zone, &session, Spell::Trap, 2, pet, 200);
        assert_eq!(
            practice_count(&launch, Spell::Trap),
            1,
            "Source launch trains once"
        );
        let born = zone.tick_five_class_skills(700);
        let id = only_trap(&zone);
        assert_eq!(
            practice_count(&born, Spell::Trap),
            1,
            "successful Source birth trains again"
        );
        assert_eq!(zone.native_monsters[&pet].control_until_ms, 60_700);
        assert_eq!(
            zone.five_class_owned_trap_shock_until(&target.entity),
            Some(60_700)
        );
        assert!(zone.owned_monster_shocked(pet, 5000));
        assert!(zone.owned_monster_shock_blocks_move(pet, 5000));
        assert!(!zone.owned_pet_can_move_now(pet));
        assert!(
            zone.selected_owned_pet_target(pet, 5000).is_none(),
            "Source Trap sets actual assigned Target=null; Focus does not auto-search"
        );
        let paused = zone.try_tick_five_class_clone(pet, 5000).unwrap();
        assert!(paused.is_empty());
        assert_eq!(zone.native_monsters[&pet].position, point);
        assert_eq!(zone.players[&session].hp, 500);
        let owner = zone.owned_pet_owner(&session).unwrap();
        let hit = zone.five_class_hit(&owner, &target, 10, EntityDefence::AC, 5001);
        assert_eq!(hit.0, 10);
        assert_eq!(zone.native_monsters[&pet].hp, 1490);
        assert_eq!(zone.native_monsters[&pet].control_until_ms, 0);
        assert_eq!(
            zone.native_monsters[&pet].assassin_trap_control_until_ms,
            None
        );
        assert_eq!(
            zone.five_class_owned_trap_shock_until(&target.entity),
            Some(0)
        );
        assert!(!zone.owned_monster_shock_blocks_move(pet, 5001));
        assert!(zone.owned_pet_can_move_now(pet));
        assert!(removed(&hit.1, id));
        assert_object_retired(&zone, id, &point);
    }

    #[test]
    fn owned_trap_recast_keeps_latest_real_shock_then_exact_expiry_releases_permanent_pet() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let master = victim_class(&mut zone, 500, MirClass::Taoist);
        zone.players.get_mut(&master).unwrap().chat_profile.pet_mode = 4;
        let pet = real_owned_bone_familiar(&mut zone, &master);
        let target = zone.five_class_target(pet).unwrap();
        let point = zone.native_monsters[&pet].position.clone();
        let magic = crystal_magic_by_spell("Trap").unwrap();
        let cost = i32::from(magic.base_cost) + 2 * i32::from(magic.level_cost);
        let delay =
            u64::from(magic.delay_base).saturating_sub(2 * u64::from(magic.delay_reduction));
        assert_eq!((cost, delay), (18, 30_000));
        let source_cast = |zone: &mut ZoneRuntime, now| {
            zone.handle(ZoneCommand::PlayerCastMagic {
                session_id: session.clone(),
                object_id: pet,
                spell: Spell::Trap,
                direction: MirDirection::Right,
                target: point.clone(),
                cast: true,
                level: 2,
                damage: 99_999,
                mp_cost: cost,
                cooldown_ms: delay,
                now_ms: now,
            })
        };
        source_cast(&mut zone, 700);
        zone.tick_five_class_skills(1200);
        let old_id = only_trap(&zone);
        assert_eq!(
            zone.five_class_owned_trap_shock_until(&target.entity),
            Some(61_200)
        );
        let mp = zone.players[&session].mp;
        let rejected = source_cast(&mut zone, 30_699);
        assert_eq!(zone.players[&session].mp, mp);
        assert_eq!(practice_count(&rejected, Spell::Trap), 0);
        assert_eq!(zone.five_class_skills.traps.len(), 1);
        let relaunched = source_cast(&mut zone, 30_700);
        assert_eq!(practice_count(&relaunched, Spell::Trap), 1);
        zone.tick_five_class_skills(31_200);
        let fresh_id = *zone
            .five_class_skills
            .traps
            .keys()
            .find(|id| **id != old_id)
            .unwrap();
        assert_eq!(zone.five_class_skills.traps.len(), 2);
        assert_eq!(
            zone.five_class_owned_trap_shock_until(&target.entity),
            Some(91_200)
        );
        let old_expired = zone.tick_five_class_skills(61_200);
        assert!(removed(&old_expired, old_id));
        assert_object_retired(&zone, old_id, &point);
        assert!(zone.objects.contains_key(&fresh_id));
        assert_eq!(
            zone.five_class_owned_trap_shock_until(&target.entity),
            Some(91_200)
        );
        assert!(zone.owned_monster_shock_blocks_move(pet, 91_199));
        assert!(!zone.owned_pet_can_move_now(pet));
        let expired = zone.tick_five_class_skills(91_200);
        assert!(removed(&expired, fresh_id));
        assert_object_retired(&zone, fresh_id, &point);
        assert_eq!(
            zone.five_class_owned_trap_shock_until(&target.entity),
            Some(0)
        );
        assert_eq!(zone.native_monsters[&pet].control_until_ms, 0);
        assert_eq!(
            zone.native_monsters[&pet].assassin_trap_control_until_ms,
            None
        );
        assert!(!zone.owned_monster_shock_blocks_move(pet, 91_200));
        assert!(zone.owned_pet_can_move_now(pet));
        assert_eq!(zone.native_monsters[&pet].summon_skill_level, 2);
        assert_eq!(
            zone.native_monsters[&pet].hp, 180,
            "Source BoneFamiliar140 + SummonSkeleton level2*20, uninjured"
        );
        assert!(!zone.native_monsters[&pet].dead);
        assert!(zone.objects.contains_key(&pet));
    }

    #[test]
    fn owned_trap_retirement_does_not_overwrite_a_later_real_owned_shock_lease() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let master = victim_class(&mut zone, 500, MirClass::Taoist);
        let pet = real_owned_bone_familiar(&mut zone, &master);
        let target = zone.five_class_target(pet).unwrap();
        let point = zone.native_monsters[&pet].position.clone();
        cast(&mut zone, &session, Spell::Trap, 2, pet, 700);
        zone.tick_five_class_skills(1200);
        let id = only_trap(&zone);
        // Directly exercise the approved typed clock installation contract;
        // this is a lease replacement test, not a claim of casting another spell.
        zone.apply_five_class_prevalidated_owned_trap_shock(&target.entity, 90_000, 2000);
        let expired = zone.tick_five_class_skills(61_200);
        assert!(removed(&expired, id));
        assert_object_retired(&zone, id, &point);
        assert_eq!(
            zone.five_class_owned_trap_shock_until(&target.entity),
            Some(90_000)
        );
        assert!(zone.owned_monster_shock_blocks_move(pet, 61_200));
        assert!(!zone.owned_pet_can_move_now(pet));
        assert_eq!(
            zone.native_monsters[&pet].assassin_trap_control_until_ms,
            None
        );
    }

    #[test]
    fn cold_real_owned_bone_trap_clears_only_orphan_control_and_retained_visual_state() {
        let (mut zone, session) = cold_fixture();
        let master = victim_class(&mut zone, 500, MirClass::Taoist);
        let pet = real_owned_bone_familiar(&mut zone, &master);
        let base_colour = match &zone.objects[&pet].packet {
            ServerPacket::ObjectMonster { info } => info.name_colour_argb,
            _ => panic!("real BoneFamiliar uses ObjectMonster"),
        };
        let point = zone.native_monsters[&pet].position.clone();
        cast(&mut zone, &session, Spell::Trap, 2, pet, 700);
        zone.tick_five_class_skills(1200);
        let trap_id = only_trap(&zone);
        assert_eq!(
            zone.native_monsters[&pet].assassin_trap_control_until_ms,
            Some(61_200)
        );
        assert_eq!(zone.native_monsters[&pet].control_until_ms, 61_200);
        assert!(zone.owned_monster_shock_blocks_move(pet, 1200));
        assert!(zone.object_grid.neighbors(&point).contains(&trap_id));
        let mut cold = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(cold.native_monsters[&pet].hp, 180);
        assert!(!cold.native_monsters[&pet].dead);
        assert_eq!(
            cold.native_monsters[&pet].assassin_trap_control_until_ms,
            None
        );
        assert_eq!(cold.native_monsters[&pet].control_until_ms, 0);
        assert!(!cold.owned_monster_shock_blocks_move(pet, 1200));
        assert_object_retired(&cold, trap_id, &point);
        assert!(
            cold.owned_pet_owner(&master).is_none(),
            "cold restore still revokes online authority"
        );
        let ServerPacket::ObjectMonster { info } = &cold.objects[&pet].packet else {
            panic!("permanent real BoneFamiliar survives cold restore");
        };
        assert_eq!(info.shock_time, 0);
        assert_eq!(info.name_colour_argb, base_colour);
        let once = cold.checkpoint_bytes().unwrap();
        cold.clear_cold_five_class_state();
        assert_eq!(
            cold.checkpoint_bytes().unwrap(),
            once,
            "second cleanup is idempotent"
        );
    }

    #[test]
    fn cold_real_owned_bone_trap_preserves_a_newer_typed_shock_replacement() {
        let (mut zone, session) = cold_fixture();
        let master = victim_class(&mut zone, 500, MirClass::Taoist);
        let pet = real_owned_bone_familiar(&mut zone, &master);
        let target = zone.five_class_target(pet).unwrap();
        let point = zone.native_monsters[&pet].position.clone();
        cast(&mut zone, &session, Spell::Trap, 2, pet, 700);
        zone.tick_five_class_skills(1200);
        let trap_id = only_trap(&zone);
        // Exercise the typed replacement lease, not a claim of casting ElectricShock.
        zone.apply_five_class_prevalidated_owned_trap_shock(&target.entity, 90_000, 2000);
        let cold = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(cold.native_monsters[&pet].hp, 180);
        assert_eq!(
            cold.native_monsters[&pet].assassin_trap_control_until_ms,
            None
        );
        assert!(cold.owned_monster_shock_blocks_move(pet, 89_999));
        assert_object_retired(&cold, trap_id, &point);
        let ServerPacket::ObjectMonster { info } = &cold.objects[&pet].packet else {
            panic!("real permanent pet remains");
        };
        assert_eq!(info.shock_time, 88_000);
        assert_eq!(info.name_colour_argb, 0xFFCD853F_u32 as i32);
    }

    #[test]
    fn cold_real_owned_bone_without_assassin_marker_preserves_typed_control() {
        let (mut zone, _) = cold_fixture();
        let master = victim_class(&mut zone, 500, MirClass::Taoist);
        let pet = real_owned_bone_familiar(&mut zone, &master);
        let target = zone.five_class_target(pet).unwrap();
        // A real retained pet and typed control contract; no fabricated checkpoint or Trap.
        zone.apply_five_class_prevalidated_owned_trap_shock(&target.entity, 90_000, 700);
        assert_eq!(
            zone.native_monsters[&pet].assassin_trap_control_until_ms,
            None
        );
        let cold = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        assert!(cold.owned_monster_shock_blocks_move(pet, 89_999));
        let ServerPacket::ObjectMonster { info } = &cold.objects[&pet].packet else {
            panic!("real permanent pet remains");
        };
        assert_eq!(info.shock_time, 89_300);
        assert_eq!(info.name_colour_argb, 0xFFCD853F_u32 as i32);
        assert_eq!(cold.native_monsters[&pet].hp, 180);
        assert!(!cold.native_monsters[&pet].dead);
    }

    #[test]
    fn trap_rejects_friendly_modes_and_attacker_safe_without_debit_or_false_training() {
        for gate in [
            "peace",
            "group",
            "guild",
            "enemy-guild",
            "red-brown",
            "attacker-safe",
        ] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            let master = victim_class(&mut zone, 500, MirClass::Assassin);
            let pet = real_owned_clone(&mut zone, &master);
            match gate {
                "peace" => {
                    zone.players
                        .get_mut(&session)
                        .unwrap()
                        .chat_profile
                        .attack_mode = 0
                }
                "group" => {
                    zone.players
                        .get_mut(&session)
                        .unwrap()
                        .chat_profile
                        .attack_mode = 1;
                    zone.players
                        .get_mut(&master)
                        .unwrap()
                        .chat_profile
                        .group_members
                        .push("native-owner".into());
                }
                "guild" => {
                    let source = &mut zone.players.get_mut(&session).unwrap().chat_profile;
                    source.attack_mode = 2;
                    source.guild_name = Some("same-guild".into());
                    zone.players
                        .get_mut(&master)
                        .unwrap()
                        .chat_profile
                        .guild_name = Some("same-guild".into());
                }
                "enemy-guild" => {
                    let source = &mut zone.players.get_mut(&session).unwrap().chat_profile;
                    source.attack_mode = 3;
                    source.guild_name = Some("attacker-guild".into());
                    zone.players
                        .get_mut(&master)
                        .unwrap()
                        .chat_profile
                        .guild_name = Some("master-guild".into());
                }
                "red-brown" => {
                    zone.players
                        .get_mut(&session)
                        .unwrap()
                        .chat_profile
                        .attack_mode = 4
                }
                _ => {
                    zone.players
                        .get_mut(&session)
                        .unwrap()
                        .chat_profile
                        .in_safe_zone = true
                }
            }
            let target = zone.five_class_target(pet).unwrap();
            assert!(
                !zone.five_class_target_hostile(&session, &target, 200),
                "{gate}"
            );
            let launch = cast(&mut zone, &session, Spell::Trap, 2, pet, 200);
            let birth = zone.tick_five_class_skills(700);
            assert_eq!(zone.players[&session].mp, 500, "{gate}");
            assert_eq!(
                practice_count(&launch, Spell::Trap) + practice_count(&birth, Spell::Trap),
                0,
                "{gate}"
            );
            assert!(zone.five_class_skills.traps.is_empty());
            assert_eq!(zone.native_monsters[&pet].control_until_ms, 0);
            assert_eq!(zone.five_class_owned_trap_shock_until(&target.entity), None);
            assert!(zone.owned_pet_can_move_now(pet));
        }
    }

    #[test]
    fn trap_preserves_source_own_all_before_safe_and_rejects_enemy_pet_in_real_safe_zone() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let own_pet = real_owned_clone(&mut zone, &session);
        zone.players
            .get_mut(&session)
            .unwrap()
            .chat_profile
            .in_safe_zone = true;
        let target = zone.five_class_target(own_pet).unwrap();
        assert!(
            zone.five_class_target_hostile(&session, &target, 2000),
            "Monster.IsAttackTarget returns own-pet All before safe-zone checks"
        );
        let launch = cast(&mut zone, &session, Spell::Trap, 2, own_pet, 2000);
        let birth = zone.tick_five_class_skills(2500);
        assert_eq!(
            practice_count(&launch, Spell::Trap) + practice_count(&birth, Spell::Trap),
            2
        );
        assert!(zone.owned_monster_shock_blocks_move(own_pet, 2500));
        assert_eq!(zone.native_monsters[&own_pet].control_until_ms, 62_500);

        let (mut zone, session) = fixture_in_map(MirClass::Assassin, "0");
        let master = victim_class(&mut zone, 500, MirClass::Assassin);
        let map = mir2_game_data::crystal_map_respawns_ref("0").expect("real Bichon source map");
        let safe = map.safe_zones.first().expect("Source Bichon safe zone");
        let inside = Point {
            x: safe.location.x + i32::from(safe.size),
            y: safe.location.y,
        };
        let outside = Point {
            x: inside.x + 1,
            y: inside.y,
        };
        assert!(map
            .safe_zones
            .iter()
            .all(
                |safe| (outside.x - safe.location.x).abs() > i32::from(safe.size)
                    || (outside.y - safe.location.y).abs() > i32::from(safe.size)
            ));
        for (sid, position) in [(&session, outside), (&master, inside.clone())] {
            zone.handle(ZoneCommand::SyncPlayerTransform {
                session_id: sid.clone(),
                position,
                direction: MirDirection::Right,
            });
        }
        let pet = real_owned_clone(&mut zone, &master);
        assert_eq!(zone.native_monsters[&pet].position, inside);
        assert!(
            !zone.players[&session].chat_profile.in_safe_zone,
            "this gate uses the actual pet tile, not an invented master safe flag"
        );
        let target = zone.five_class_target(pet).unwrap();
        assert!(!zone.five_class_target_hostile(&session, &target, 200));
        let launch = cast(&mut zone, &session, Spell::Trap, 2, pet, 200);
        let birth = zone.tick_five_class_skills(700);
        assert_eq!(zone.players[&session].mp, 500);
        assert_eq!(
            practice_count(&launch, Spell::Trap) + practice_count(&birth, Spell::Trap),
            0
        );
        assert!(zone.five_class_skills.traps.is_empty());
        assert_eq!(zone.five_class_owned_trap_shock_until(&target.entity), None);
    }

    #[test]
    fn trap_rechecks_delayed_owned_target_and_keeps_only_real_launch_training_on_cancel() {
        for invalidate_owner in [false, true] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            let master = victim_class(&mut zone, 500, MirClass::Assassin);
            let pet = real_owned_clone(&mut zone, &master);
            let launch = cast(&mut zone, &session, Spell::Trap, 2, pet, 200);
            assert_eq!(practice_count(&launch, Spell::Trap), 1);
            assert_eq!(zone.players[&session].mp, 493);
            if invalidate_owner {
                zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: master,
                    hp: 0,
                    max_hp: 500,
                    mp: 500,
                    dead: true,
                });
            } else {
                zone.players
                    .get_mut(&session)
                    .unwrap()
                    .chat_profile
                    .attack_mode = 0;
            }
            let birth = zone.tick_five_class_skills(700);
            assert_eq!(practice_count(&birth, Spell::Trap), 0);
            assert!(zone.five_class_skills.traps.is_empty());
            assert!(zone
                .native_monsters
                .get(&pet)
                .is_none_or(|monster| monster.assassin_trap_control_until_ms.is_none()
                    && monster.control_until_ms == 0));
        }
    }

    #[test]
    fn trap_positive_attacked_releases_real_node_and_action_clocks_but_armour_block_does_not() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let point = Point { x: 21, y: 20 };
        monster(&mut zone, 900, point.clone(), 10);
        let target = zone.five_class_target(900).unwrap();
        let owner = zone.owned_pet_owner(&session).unwrap();
        let before_ai = zone.native_monsters[&900].next_ai_ready_at_ms;
        let before_attack = zone.native_monsters[&900].next_attack_ready_at_ms;
        cast(&mut zone, &session, Spell::Trap, 2, 900, 200);
        zone.tick_five_class_skills(700);
        let id = only_trap(&zone);
        assert_eq!(
            zone.native_monsters[&900].assassin_trap_control_until_ms,
            Some(60_700)
        );
        assert!(zone.players[&session].visible_object_ids.contains(&id));
        zone.native_monsters.get_mut(&900).unwrap().defense.min_ac = 1000;
        zone.native_monsters.get_mut(&900).unwrap().defense.max_ac = 1000;
        let miss = zone.five_class_hit(&owner, &target, 10, EntityDefence::AC, 800);
        assert_eq!(miss.0, 0);
        assert_eq!(zone.native_monsters[&900].hp, 10_000);
        assert_eq!(zone.native_monsters[&900].control_until_ms, 60_700);
        assert!(!removed(&miss.1, id));
        assert!(zone.objects.contains_key(&id));
        zone.native_monsters.get_mut(&900).unwrap().defense.min_ac = 0;
        zone.native_monsters.get_mut(&900).unwrap().defense.max_ac = 0;
        let hit = zone.five_class_hit(&owner, &target, 10, EntityDefence::AC, 801);
        assert_eq!(hit.0, 10);
        assert_eq!(zone.native_monsters[&900].hp, 9990);
        assert_eq!(zone.native_monsters[&900].control_until_ms, 0);
        assert_eq!(
            zone.native_monsters[&900].assassin_trap_control_until_ms,
            None
        );
        assert_eq!(
            zone.native_monsters[&900].next_ai_ready_at_ms,
            before_ai.max(801)
        );
        assert_eq!(
            zone.native_monsters[&900].next_attack_ready_at_ms,
            before_attack.max(801)
        );
        assert!(removed(&hit.1, id));
        assert!(zone.five_class_skills.traps.is_empty());
        assert_object_retired(&zone, id, &point);
    }

    #[test]
    fn trap_nonlethal_environment_struck_and_actual_bleeding_changehp_keep_shocktime() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let point = Point { x: 21, y: 20 };
        monster(&mut zone, 900, point.clone(), 10);
        let target = zone.five_class_target(900).unwrap();
        cast(&mut zone, &session, Spell::Trap, 2, 900, 200);
        zone.tick_five_class_skills(700);
        let id = only_trap(&zone);
        let struck = zone.resolve_native_monster_environment_struck(&target.entity, 10, false, 800);
        assert_eq!(struck.0, 10);
        assert_eq!(zone.native_monsters[&900].hp, 9990);
        assert!(!removed(&struck.1, id));
        assert_eq!(zone.native_monsters[&900].control_until_ms, 60_700);
        zone.tick_five_class_skills(801);
        assert!(zone.objects.contains_key(&id));
        zone.players
            .get_mut(&session)
            .unwrap()
            .combat_stats
            .mp_eater_level = None;
        zone.native_monsters.get_mut(&900).unwrap().defense.min_ac = 9999;
        zone.native_monsters.get_mut(&900).unwrap().defense.max_ac = 9999;
        let owner = zone.owned_pet_owner(&session).unwrap();
        for index in 0..56 {
            let now = 1000 + index * 1000;
            let launch = zone.five_class_melee_launch(&session, 900, 40, now);
            let hit = zone.five_class_hit(
                &owner,
                &target,
                launch.0,
                launch.1.unwrap_or(EntityDefence::ACAgility),
                now,
            );
            assert_eq!(hit.0, 0, "ordinary primary hit is really blocked");
            assert!(!removed(&hit.1, id));
        }
        assert_eq!(zone.native_monsters[&900].hp, 9990);
        let periodic = zone.tick_five_class_skills(56_000);
        assert_eq!(
            zone.native_monsters[&900].hp, 9949,
            "actual Source Bleeding ChangeHP ignores armour"
        );
        assert!(!removed(&periodic, id));
        assert_eq!(zone.native_monsters[&900].control_until_ms, 60_700);
        assert_eq!(
            zone.native_monsters[&900].assassin_trap_control_until_ms,
            Some(60_700)
        );
        assert!(zone.objects.contains_key(&id));
        assert!(zone.object_grid.neighbors(&point).contains(&id));
        assert!(zone.players[&session].visible_object_ids.contains(&id));
    }

    #[test]
    fn trap_lethal_attacked_retires_target_bound_node_without_waiting_sixty_seconds() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let point = Point { x: 21, y: 20 };
        monster(&mut zone, 900, point.clone(), 10);
        let target = zone.five_class_target(900).unwrap();
        let owner = zone.owned_pet_owner(&session).unwrap();
        cast(&mut zone, &session, Spell::Trap, 2, 900, 200);
        zone.tick_five_class_skills(700);
        let id = only_trap(&zone);
        let hit = zone.five_class_hit(&owner, &target, 20_000, EntityDefence::AC, 800);
        assert!(hit.0 > 0);
        assert!(zone.native_monsters[&900].dead);
        assert_eq!(zone.native_monsters[&900].hp, 0);
        assert!(removed(&hit.1, id));
        assert_eq!(
            zone.native_monsters[&900].assassin_trap_control_until_ms,
            None
        );
        assert!(zone.five_class_skills.traps.is_empty());
        assert_object_retired(&zone, id, &point);
    }

    #[test]
    fn trap_target_despawn_and_new_incarnation_cannot_transfer_old_shocktime() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let point = Point { x: 21, y: 20 };
        monster(&mut zone, 900, point.clone(), 10);
        let old_incarnation = zone.native_monsters[&900].incarnation;
        cast(&mut zone, &session, Spell::Trap, 2, 900, 200);
        zone.tick_five_class_skills(700);
        let old_id = only_trap(&zone);
        zone.despawn_world_event_monster(900, 800);
        monster(&mut zone, 900, point.clone(), 10);
        assert_ne!(zone.native_monsters[&900].incarnation, old_incarnation);
        assert_eq!(zone.native_monsters[&900].hp, 10_000);
        assert_eq!(zone.native_monsters[&900].control_until_ms, 0);
        let out = zone.tick_five_class_skills(801);
        assert!(removed(&out, old_id));
        assert_object_retired(&zone, old_id, &point);
        assert!(
            zone.objects.contains_key(&900),
            "fresh monster Node is intact"
        );
        assert_eq!(
            zone.native_monsters[&900].assassin_trap_control_until_ms,
            None
        );
        cast(&mut zone, &session, Spell::Trap, 2, 900, 2000);
        zone.tick_five_class_skills(2500);
        let fresh_id = only_trap(&zone);
        assert_ne!(fresh_id, old_id);
        assert_eq!(zone.native_monsters[&900].control_until_ms, 62_500);
        assert_eq!(
            zone.native_monsters[&900].assassin_trap_control_until_ms,
            Some(62_500)
        );
        assert!(zone.objects.contains_key(&fresh_id));
    }

    #[test]
    fn trap_recast_keeps_source_aoi_and_deadlines_then_releases_all_nodes_on_real_attacked() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let point = Point { x: 21, y: 20 };
        monster(&mut zone, 900, point.clone(), 10);
        cast(&mut zone, &session, Spell::Trap, 2, 900, 200);
        zone.tick_five_class_skills(700);
        let old_id = only_trap(&zone);
        cast(&mut zone, &session, Spell::Trap, 2, 900, 2000);
        zone.tick_five_class_skills(2500);
        assert_eq!(zone.five_class_skills.traps.len(), 2);
        let fresh_id = *zone
            .five_class_skills
            .traps
            .keys()
            .find(|id| **id != old_id)
            .unwrap();
        assert_eq!(zone.native_monsters[&900].control_until_ms, 62_500);
        assert!(zone.tick_five_class_skills(60_699).is_empty());
        assert!(zone.objects.contains_key(&old_id));
        let expired = zone.tick_five_class_skills(60_700);
        assert!(removed(&expired, old_id));
        assert_object_retired(&zone, old_id, &point);
        assert!(zone.objects.contains_key(&fresh_id));
        assert_eq!(zone.native_monsters[&900].control_until_ms, 62_500);
        let owner = zone.owned_pet_owner(&session).unwrap();
        let target = zone.five_class_target(900).unwrap();
        let hit = zone.five_class_hit(&owner, &target, 10, EntityDefence::AC, 60_701);
        assert_eq!(hit.0, 10);
        assert!(removed(&hit.1, fresh_id));
        assert_object_retired(&zone, fresh_id, &point);
        assert_eq!(zone.native_monsters[&900].control_until_ms, 0);
        assert_eq!(zone.native_monsters[&900].next_ai_ready_at_ms, 60_701);
        assert_eq!(zone.native_monsters[&900].next_attack_ready_at_ms, 60_701);
    }

    #[test]
    fn trap_expires_at_exact_source_deadline_or_prior_shocktime_clear() {
        for early in [false, true] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            let point = Point { x: 21, y: 20 };
            monster(&mut zone, 900, point.clone(), 10);
            cast(&mut zone, &session, Spell::Trap, 2, 900, 200);
            zone.tick_five_class_skills(700);
            let id = only_trap(&zone);
            assert!(zone.object_grid.neighbors(&point).contains(&id));
            let now = if early {
                // SpellObject's direct ShockTime==0 gate, in addition to the
                // actual damage tests above; this is not a claimed hit test.
                zone.native_monsters.get_mut(&900).unwrap().control_until_ms = 0;
                800
            } else {
                zone.tick_five_class_skills(60_699);
                assert!(zone.objects.contains_key(&id));
                60_700
            };
            let out = zone.tick_five_class_skills(now);
            assert!(removed(&out, id));
            assert_object_retired(&zone, id, &point);
            assert_eq!(zone.native_monsters[&900].control_until_ms, 0);
            assert_eq!(
                zone.native_monsters[&900].assassin_trap_control_until_ms,
                None
            );
            assert!(zone.five_class_skills.traps.is_empty());
        }
    }

    #[test]
    fn dark_body_metadata_and_native_cast_accept_live_friendly_self_target() {
        let skill = crate::runtime::skills::crystal_skill_state("DarkBody", 2).unwrap();
        let snapshot = skill.snapshot(0, mir2_game_data::LanguageCode::English);
        assert_eq!(snapshot.cast_kind, "target");
        assert!(!snapshot.offensive);
        let (mut zone, session) = fixture(MirClass::Assassin);
        let out = cast(&mut zone, &session, Spell::DarkBody, 2, 101, 100);
        assert_eq!(zone.players[&session].mp, 493);
        assert_eq!(zone.five_class_skills.clones.len(), 1);
        assert!(packets(&out).iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                spell: Spell::DarkBody,
                cast: true,
                ..
            }
        )));
    }

    #[test]
    fn aimed_casts_enforce_source_range_and_ten_tile_find_object_boundary() {
        for (spell, class, boundary) in [
            (Spell::DarkBody, MirClass::Assassin, 10),
            (Spell::Trap, MirClass::Assassin, 9),
            (Spell::DelayedExplosion, MirClass::Archer, 9),
            (Spell::VampireShot, MirClass::Archer, 9),
            (Spell::PoisonShot, MirClass::Archer, 9),
            (Spell::CrippleShot, MirClass::Archer, 9),
            (Spell::ElementalShot, MirClass::Archer, 9),
            (Spell::CatTongue, MirClass::Assassin, 9),
        ] {
            for distance in [boundary, boundary + 1] {
                let (mut zone, session) = fixture(class);
                if spell == Spell::ElementalShot {
                    zone.five_class_obtain_element(&session, true);
                }
                monster(
                    &mut zone,
                    900,
                    Point {
                        x: 20 + distance,
                        y: 20,
                    },
                    10,
                );
                let output = cast(&mut zone, &session, spell, 2, 900, 100);
                assert_eq!(
                    zone.players[&session].mp,
                    if distance == boundary { 493 } else { 500 },
                    "{spell:?} distance {distance}"
                );
                if distance > boundary {
                    assert!(zone.five_class_skills.pending.is_empty());
                    assert!(zone.five_class_skills.clones.is_empty());
                    assert!(!output
                        .iter()
                        .any(|o| matches!(o, ZoneOutbound::FiveClassMagicPractice { .. })));
                }
            }
        }
    }

    #[test]
    fn cat_tongue_delays_an_ac_hit_and_only_positive_damage_controls_and_practices() {
        for player_target in [false, true] {
            for blocked in [false, true] {
                let (mut zone, session) = fixture(MirClass::Assassin);
                let victim_session = if player_target {
                    Some(victim(&mut zone, 500))
                } else {
                    monster(&mut zone, 900, Point { x: 21, y: 20 }, 10);
                    None
                };
                let id = if player_target { 202 } else { 900 };
                let (ac, mac) = if blocked { (9999, 0) } else { (0, 9999) };
                if let Some(victim) = &victim_session {
                    let stats = &mut zone.players.get_mut(victim).unwrap().combat_stats;
                    stats.min_ac = ac;
                    stats.max_ac = ac;
                    stats.min_mac = mac;
                    stats.max_mac = mac;
                } else {
                    let stats = &mut zone.native_monsters.get_mut(&id).unwrap().defense;
                    stats.min_ac = ac;
                    stats.max_ac = ac;
                    stats.min_mac = mac;
                    stats.max_mac = mac;
                }
                // Exercise source roll eight: it becomes Stun under the
                // default PvP setting, and for every non-Human target.
                let due = (600..1600)
                    .find(|now| {
                        five_class_damage::source_chance_roll(*now, 101, id, 1, 1119, 10) == 8
                    })
                    .unwrap();
                let launched = cast(&mut zone, &session, Spell::CatTongue, 2, id, due - 550);
                assert_eq!(practice_count(&launched, Spell::CatTongue), 0);
                assert_eq!(zone.five_class_skills.pending.len(), 1);
                let hp_before = if let Some(victim) = &victim_session {
                    zone.players[victim].hp
                } else {
                    zone.native_monsters[&id].hp
                };
                assert_eq!(
                    practice_count(&zone.tick_five_class_skills(due - 1), Spell::CatTongue),
                    0
                );
                let settled = zone.tick_five_class_skills(due);
                let (hp_after, poison) = if let Some(victim) = &victim_session {
                    let player = &zone.players[victim];
                    (player.hp, zone_player_active_status(player, due))
                } else {
                    let monster = &zone.native_monsters[&id];
                    (monster.hp, native_monster_current_poison(monster))
                };
                assert_eq!(
                    hp_after < hp_before,
                    !blocked,
                    "Human={player_target}, blocked={blocked}"
                );
                assert_eq!(
                    practice_count(&settled, Spell::CatTongue),
                    usize::from(!blocked)
                );
                assert_eq!(poison & CRYSTAL_POISON_STUN != 0, !blocked);
                assert_eq!(poison & (CRYSTAL_POISON_FROZEN | CRYSTAL_POISON_SLOW), 0);
                if blocked {
                    assert!(!packets(&settled).iter().any(|packet| matches!(packet,
                        ServerPacket::ObjectPoisoned { object_id, .. } if *object_id == id)));
                }
            }
        }
    }

    #[test]
    fn cat_tongue_slow_uses_the_real_source_lease_and_never_freezes_by_default() {
        for player_target in [false, true] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            let victim_session = if player_target {
                Some(victim(&mut zone, 500))
            } else {
                monster(&mut zone, 900, Point { x: 21, y: 20 }, 10);
                None
            };
            let id = if player_target { 202 } else { 900 };
            let due = (600..1600)
                .find(|now| five_class_damage::source_chance_roll(*now, 101, id, 1, 1119, 10) == 9)
                .unwrap();
            cast(&mut zone, &session, Spell::CatTongue, 2, id, due - 550);
            let settled = zone.tick_five_class_skills(due);
            assert_eq!(practice_count(&settled, Spell::CatTongue), 1);
            if let Some(victim) = &victim_session {
                let player = &zone.players[victim];
                assert_eq!(zone_player_active_status(player, due), CRYSTAL_POISON_SLOW);
                assert_eq!(zone_player_move_delay_ms(player, true, due), 1200);
            } else {
                assert_eq!(
                    native_monster_current_poison(&zone.native_monsters[&id]),
                    CRYSTAL_POISON_SLOW
                );
            }
            // The actual spawn clamps to Yob's imported 1500/2500 speeds.
            // ProcessPoison adds 100 on every pass, even between poison ticks.
            if !player_target {
                let target = &zone.native_monsters[&id];
                assert_eq!((target.move_speed_ms, target.attack_speed_ms), (1500, 2500));
            }
            zone.tick_entity_combat(due);
            if !player_target {
                let target = &zone.native_monsters[&id];
                assert_eq!((target.move_speed_ms, target.attack_speed_ms), (1600, 2600));
            }
            zone.tick_entity_combat(due + 500);
            if !player_target {
                assert_eq!(zone.native_monsters[&id].move_speed_ms, 1700);
                assert_eq!(zone.native_monsters[&id].attack_speed_ms, 2700);
                assert!(!native_monster_control_active(
                    &zone.native_monsters[&id],
                    due
                ));
            }
            for tick in 1..=8 {
                zone.tick_entity_combat(due + tick * 1001);
            }
            if let Some(victim) = &victim_session {
                assert_eq!(
                    zone_player_active_status(&zone.players[victim], due + 8008),
                    0
                );
            } else {
                let target = &zone.native_monsters[&id];
                assert_eq!(
                    native_monster_current_poison(target) & CRYSTAL_POISON_SLOW,
                    0
                );
                let template = crystal_monster_by_name("Yob").unwrap();
                assert_eq!(target.move_speed_ms, u64::from(template.move_speed));
                assert_eq!(target.attack_speed_ms, u64::from(template.attack_speed));
                assert_eq!(
                    target.next_attack_ready_at_ms,
                    due + 8008 + target.attack_speed_ms
                );
            }
        }
    }

    #[test]
    fn cat_tongue_tracks_the_same_live_target_without_an_archer_launch_cell_fence() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        monster(&mut zone, 900, Point { x: 21, y: 20 }, 10);
        cast(&mut zone, &session, Spell::CatTongue, 2, 900, 100);
        zone.native_monsters.get_mut(&900).unwrap().position = Point { x: 30, y: 20 };
        let out = zone.tick_five_class_skills(650);
        assert!(zone.native_monsters[&900].hp < 10_000);
        assert_eq!(practice_count(&out, Spell::CatTongue), 1);
    }

    #[test]
    fn green_poison_sources_keep_only_strongest_single_timer_and_correct_owner() {
        // Use the actual legacy wild-monster entry point used by Taoist
        // Poisoning, then both real tick queues in the runtime's order.
        for replacement in [0, 1, 2] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            let taoist = SessionId::new("native-taoist");
            zone.handle(ZoneCommand::Join(ZoneJoin {
                session_id: taoist.clone(),
                account_id: "native-taoist".into(),
                character_index: 2,
                object_id: 202,
                name: "native-taoist".into(),
                class: MirClass::Taoist,
                gender: mir2_protocol::MirGender::Male,
                level: 50,
                hp: 500,
                max_hp: 500,
                mp: 500,
                map_file_name: "five-class-skills-unit".into(),
                position: Point { x: 20, y: 21 },
                direction: MirDirection::Right,
                chat_profile: ZoneChatProfile {
                    attack_mode: 5,
                    ..Default::default()
                },
                combat_stats: ZonePlayerCombatStats {
                    max_mp: 500,
                    ..Default::default()
                },
            }));
            monster(&mut zone, 900, Point { x: 21, y: 20 }, 10);
            cast(&mut zone, &session, Spell::PoisonSword, 2, 0, 100);
            zone.tick_five_class_skills(100);
            let original = zone.five_class_skills.bleeds[0].value;
            let hp = zone.native_monsters[&900].hp;
            let damage = match replacement {
                0 => 0,                   // weaker green, one damage per tick
                1 => (original - 1) * 15, // equal value refreshes to new source
                _ => (original + 1) * 15, // stronger value replaces old source
            };
            let applied = zone.apply_native_monster_taoist_poison(
                900,
                &taoist,
                202,
                0,
                damage,
                CRYSTAL_POISON_GREEN,
                200,
            );
            assert!(!applied.is_empty());
            let scalar = zone.native_monsters[&900].damage_poison_value;
            zone.tick_five_class_skills(200);
            zone.tick_native_monster_damage_poisons(200);
            assert_eq!(zone.native_monsters[&900].hp, hp);
            if scalar >= original {
                assert!(zone.five_class_skills.bleeds.is_empty());
                assert_eq!(
                    zone.native_monsters[&900]
                        .damage_poison_owner_session_id
                        .as_ref(),
                    Some(&taoist)
                );
            } else {
                assert_eq!(zone.five_class_skills.bleeds.len(), 1);
                assert_eq!(
                    zone.five_class_skills.bleeds[0].owner.online.session_id,
                    session
                );
                assert_eq!(zone.native_monsters[&900].damage_poison, 0);
                assert!(zone.native_monsters[&900]
                    .damage_poison_owner_session_id
                    .is_none());
            }
            zone.tick_five_class_skills(1100);
            zone.tick_native_monster_damage_poisons(1100);
            assert_eq!(
                zone.native_monsters[&900].hp,
                if scalar >= original {
                    hp
                } else {
                    hp - original
                }
            );
            zone.tick_five_class_skills(2200);
            zone.tick_native_monster_damage_poisons(2200);
            assert_eq!(
                zone.native_monsters[&900].hp,
                if scalar >= original {
                    hp - scalar
                } else {
                    hp - 2 * original
                }
            );
            assert_ne!(
                native_monster_current_poison(&zone.native_monsters[&900]) & CRYSTAL_POISON_GREEN,
                0
            );
        }
    }

    #[test]
    fn gather_element_is_attacker_only_and_concentration_recovers_after_movement() {
        let (mut zone, session) = fixture(MirClass::Archer);
        cast(&mut zone, &session, Spell::Concentration, 2, 0, 100);
        zone.five_class_player_moved(&session, 200);
        assert_eq!(
            zone.five_class_skills.actors[&session].concentration_interrupted_until,
            3200
        );
        let out = zone.tick_five_class_skills(3200);
        assert!(packets(&out).iter().any(|packet| matches!(
            packet,
            ServerPacket::SetConcentration {
                enabled: true,
                interrupted: false,
                ..
            }
        )));
        assert!(zone
            .five_class_damage_committed(900, 101, 40, false, 3300)
            .is_empty());
        assert!(zone
            .five_class_damage_committed(101, 900, 0, false, 3300)
            .is_empty());
        let now = (3300..3400)
            .find(|now| zone_deterministic_roll(*now, 101, 0x6A74, 10) >= 2)
            .unwrap();
        zone.five_class_damage_committed(101, 900, 40, false, now);
        assert_eq!(zone.five_class_skills.actors[&session].elements, 1);
    }

    #[test]
    fn elemental_shot_no_orbs_gathers_and_spending_keeps_source_one_progress() {
        let (mut zone, session) = fixture(MirClass::Archer);
        let out = cast(&mut zone, &session, Spell::ElementalShot, 2, 0, 100);
        assert_eq!(zone.five_class_skills.actors[&session].elements, 100);
        assert!(packets(&out).iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic {
                target_id: 0,
                cast: true,
                ..
            }
        )));
        monster(&mut zone, 900, Point { x: 21, y: 20 }, 50);
        let out = cast(&mut zone, &session, Spell::ElementalShot, 2, 900, 2000);
        assert!(packets(&out)
            .iter()
            .any(|packet| matches!(packet, ServerPacket::Magic { target_id: 900, .. })));
        zone.tick_five_class_skills(2550);
        assert!(zone.native_monsters[&900].hp < 10_000);
        assert_eq!(zone.five_class_skills.actors[&session].elements, 1);
    }

    #[test]
    fn delayed_explosion_has_real_stages_and_purification_cancels_the_lease() {
        for purified in [false, true] {
            let (mut zone, session) = fixture(MirClass::Archer);
            monster(&mut zone, 900, Point { x: 21, y: 20 }, 50);
            monster(&mut zone, 901, Point { x: 21, y: 21 }, 50);
            cast(&mut zone, &session, Spell::DelayedExplosion, 2, 900, 100);
            zone.tick_five_class_skills(650);
            assert!(zone.native_monsters[&900].hp < 10_000);
            assert_eq!(zone.native_monsters[&901].hp, 10_000);
            if purified {
                zone.native_monsters.get_mut(&900).unwrap().entity_poison &= !DELAYED_EXPLOSION;
            }
            for now in [2650, 4650, 6650, 8650, 10650] {
                zone.tick_five_class_skills(now);
            }
            assert_eq!(zone.native_monsters[&901].hp, 10_000, "no premature AoE");
            zone.tick_five_class_skills(12650);
            assert_eq!(zone.native_monsters[&901].hp < 10_000, !purified);
            assert!(zone.five_class_skills.explosions.is_empty());
        }
    }

    #[test]
    fn vampire_shot_heals_in_ten_hp_ticks_instead_of_immediately() {
        let (mut zone, session) = fixture(MirClass::Archer);
        zone.players.get_mut(&session).unwrap().hp = 100;
        monster(&mut zone, 900, Point { x: 21, y: 20 }, 50);
        cast(&mut zone, &session, Spell::VampireShot, 3, 900, 100);
        zone.tick_five_class_skills(650);
        assert_eq!(zone.players[&session].hp, 100);
        assert!(!zone.five_class_skills.vampires.is_empty());
        zone.tick_five_class_skills(1650);
        assert_eq!(
            zone.players[&session].hp, 100,
            "source strict Time > VampTime"
        );
        zone.tick_five_class_skills(1651);
        assert_eq!(zone.players[&session].hp, 110);
        zone.tick_five_class_skills(2151);
        assert_eq!(zone.players[&session].hp, 110);
        zone.tick_five_class_skills(2152);
        assert_eq!(zone.players[&session].hp, 120);
    }

    #[test]
    fn mp_eater_arms_next_attack_and_emits_authoritative_mana_settlement() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        let source = zone.players.get_mut(&session).unwrap();
        source.mp = 30;
        source.combat_stats.accuracy = 198;
        source.combat_stats.hemorrhage_level = None;
        monster(&mut zone, 900, Point { x: 21, y: 20 }, 50);
        let first = zone.five_class_melee_launch(&session, 900, 40, 100);
        assert!(first.1.is_none());
        assert_eq!(zone.players[&session].mp, 30);
        let second = zone.five_class_melee_launch(&session, 900, 40, 1000);
        assert!(matches!(second.1, Some(EntityDefence::ACAgility)));
        assert_eq!(zone.players[&session].mp, 275);
        assert!(second.2.iter().any(|o| matches!(o, ZoneOutbound::PlayerManaChanged { settlement, .. }
            if settlement.mp_before == 30 && settlement.mp_after == 275 && settlement.receipt_sequence > 0)));
        assert_eq!(
            zone.native_monsters[&900].hp, 10_000,
            "launch does not fake a hit"
        );
    }

    #[test]
    fn hemorrhage_level_zero_needs_fifty_five_launches_then_one_real_tick() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        zone.players
            .get_mut(&session)
            .unwrap()
            .combat_stats
            .mp_eater_level = None;
        monster(&mut zone, 900, Point { x: 21, y: 20 }, 50);
        for now in 1..=55 {
            assert!(zone
                .five_class_melee_launch(&session, 900, 40, now)
                .1
                .is_none());
        }
        assert_eq!(zone.native_monsters[&900].hp, 10_000);
        zone.five_class_melee_launch(&session, 900, 40, 56);
        assert_eq!(zone.native_monsters[&900].hp, 10_000);
        zone.tick_five_class_skills(56);
        assert_eq!(zone.native_monsters[&900].hp, 9959);
        assert_eq!(
            native_monster_current_poison(&zone.native_monsters[&900]) & BLEEDING,
            0
        );
        assert!(zone.five_class_skills.bleeds.is_empty());
    }

    #[test]
    fn hemorrhage_player_poison_death_has_current_typed_causality_even_if_melee_misses() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        zone.players
            .get_mut(&session)
            .unwrap()
            .combat_stats
            .mp_eater_level = None;
        let victim = victim(&mut zone, 30);
        let stats = &mut zone.players.get_mut(&victim).unwrap().combat_stats;
        stats.min_ac = 9999;
        stats.max_ac = 9999;
        let owner = zone.owned_pet_owner(&session).unwrap();
        let owner_life = zone.players[&session].life_generation;
        let victim_life = zone.players[&victim].life_generation;
        let target = zone.five_class_target(202).unwrap();
        for now in 1..=55 {
            zone.five_class_melee_launch(&session, 202, 40, now);
        }
        let launch = zone.five_class_melee_launch(&session, 202, 40, 56);
        assert_eq!(
            zone.owned_pet_brown_until(&zone.players[&session]),
            60_056,
            "ApplyPoison browns on application before the later ordinary AC miss"
        );
        let missed = zone.five_class_hit(&owner, &target, launch.0, launch.1.unwrap(), 56);
        assert_eq!(missed.0, 0);
        assert_eq!(zone.players[&victim].hp, 30);
        assert!(zone.five_class_skills.bleeds.len() == 1);
        let out = zone.tick_five_class_skills(56);
        assert!(zone.players[&victim].dead);
        assert_eq!(zone.players[&victim].hp, 0);
        let receipts = out
            .iter()
            .filter_map(|out| match out {
                ZoneOutbound::OwnedPetPlayerKill { receipt } => Some(receipt),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(receipts.len(), 1);
        let receipt = receipts[0];
        assert!(receipt.direct_player);
        assert_eq!(receipt.owner_session_id, session);
        assert_eq!(receipt.owner_account_id, "native-owner");
        assert_eq!(receipt.owner_character_index, 1);
        assert_eq!(receipt.owner_object_id, 101);
        assert_eq!(receipt.owner_life_generation, owner_life);
        assert_eq!(receipt.owner_online_identity, owner.online.encoded());
        assert_eq!(receipt.victim_session_id, victim);
        assert_eq!(receipt.victim_object_id, 202);
        assert_eq!(receipt.victim_life_generation, victim_life);
        assert!(receipt.unlawful);
        assert!(!zone
            .tick_five_class_skills(1057)
            .iter()
            .any(|out| matches!(out, ZoneOutbound::OwnedPetPlayerKill { .. })));
    }

    #[test]
    fn hemorrhage_old_life_cannot_hurt_a_revived_actor_or_issue_a_kill_receipt() {
        for renew_owner in [false, true] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            zone.players
                .get_mut(&session)
                .unwrap()
                .combat_stats
                .mp_eater_level = None;
            let victim = victim(&mut zone, 30);
            for now in 1..=56 {
                zone.five_class_melee_launch(&session, 202, 40, now);
            }
            assert_eq!(zone.five_class_skills.bleeds.len(), 1);
            let renewed = if renew_owner { &session } else { &victim };
            zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: renewed.clone(),
                hp: 0,
                max_hp: 500,
                mp: 500,
                dead: true,
            });
            zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: renewed.clone(),
                hp: 500,
                max_hp: 500,
                mp: 500,
                dead: false,
            });
            let out = zone.tick_five_class_skills(57);
            assert_eq!(zone.players[&victim].hp, if renew_owner { 30 } else { 500 });
            assert!(zone.five_class_skills.bleeds.is_empty());
            assert!(!out
                .iter()
                .any(|out| matches!(out, ZoneOutbound::OwnedPetPlayerKill { .. })));
        }
    }

    #[test]
    fn hemorrhage_application_respects_friendship_and_ordinary_guild_war() {
        for at_war in [false, true] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            zone.players
                .get_mut(&session)
                .unwrap()
                .combat_stats
                .mp_eater_level = None;
            let victim = victim(&mut zone, 500);
            let source_profile = &mut zone.players.get_mut(&session).unwrap().chat_profile;
            source_profile.guild_name = Some("ordinary-guild-a".into());
            let target_profile = &mut zone.players.get_mut(&victim).unwrap().chat_profile;
            target_profile.guild_name = Some("ordinary-guild-b".into());
            if at_war {
                target_profile
                    .active_guild_wars
                    .push("ordinary-guild-a".into());
            }
            for now in 1..=56 {
                zone.five_class_melee_launch(&session, 202, 40, now);
            }
            assert_eq!(zone.five_class_skills.bleeds.len(), 1);
            assert_eq!(
                zone.players[&victim].hp, 500,
                "application does not masquerade as melee damage"
            );
            assert_eq!(
                zone.owned_pet_brown_until(&zone.players[&session]),
                if at_war { 0 } else { 60_056 }
            );
        }
        for mode in [1, 2] {
            let (mut zone, session) = fixture(MirClass::Assassin);
            let victim = victim(&mut zone, 500);
            let source_profile = &mut zone.players.get_mut(&session).unwrap().chat_profile;
            source_profile.attack_mode = mode;
            source_profile.guild_name = Some("friendly-guild".into());
            let target_profile = &mut zone.players.get_mut(&victim).unwrap().chat_profile;
            target_profile.guild_name = Some("friendly-guild".into());
            target_profile.group_members.push("native-owner".into());
            for now in 1..=56 {
                assert_eq!(zone.five_class_melee_launch(&session, 202, 40, now).0, 40);
            }
            assert!(zone.five_class_skills.bleeds.is_empty());
            assert_eq!(zone.players[&victim].poison & BLEEDING, 0);
            assert_eq!(zone.owned_pet_brown_until(&zone.players[&session]), 0);
        }
    }

    #[test]
    fn hemorrhage_application_warzone_exemption_uses_the_real_active_region() {
        use crate::conquest::{default_sabuk_defenses, sabuk_policy, SharedConquestRecord};
        for (map, active, inside, exempt) in [
            ("0150", true, false, true),
            ("0150", false, false, false),
            ("3", true, false, false),
            ("3", true, true, true),
        ] {
            let (mut zone, session) = fixture_in_map(MirClass::Assassin, map);
            zone.players
                .get_mut(&session)
                .unwrap()
                .combat_stats
                .mp_eater_level = None;
            let victim = victim(&mut zone, 500);
            if inside {
                for (player, position) in [
                    (&session, Point { x: 672, y: 330 }),
                    (&victim, Point { x: 673, y: 330 }),
                ] {
                    zone.handle(ZoneCommand::SyncPlayerTransform {
                        session_id: player.clone(),
                        position,
                        direction: MirDirection::Right,
                    });
                }
            }
            let mut record = SharedConquestRecord::new(1);
            record.defenses = default_sabuk_defenses();
            record.owner_guild_id = Some("11111111111111111111111111111111".into());
            record.attacker_guild_id = Some("22222222222222222222222222222222".into());
            record.war_active = active;
            record.battle_day = active.then_some(0);
            record.war_ends_ms = 1_100_000;
            zone.apply_conquest_projection(&sabuk_policy(), &record, &[], 1_000_000, 1)
                .unwrap();
            assert_eq!(
                zone.conquest_player_is_in_war_region(&zone.players[&session], 56),
                exempt
            );
            for now in 1..=56 {
                zone.five_class_melee_launch(&session, 202, 40, now);
            }
            assert_eq!(zone.five_class_skills.bleeds.len(), 1);
            assert_eq!(
                zone.owned_pet_brown_until(&zone.players[&session]),
                if exempt { 0 } else { 60_056 },
                "map={map} active={active} inside={inside}"
            );
        }
    }

    #[test]
    fn archer_orbs_survive_real_aoi_reentry_and_live_transfer_but_clear_on_new_life() {
        let (mut zone, session) = fixture(MirClass::Archer);
        zone.five_class_obtain_element(&session, true);
        let observer = victim(&mut zone, 500);
        let packet = zone.handle(ZoneCommand::Leave {
            session_id: observer.clone(),
        });
        assert!(packets(&packet).iter().any(|packet| matches!(packet,
            ServerPacket::ObjectRemove { object_id } if *object_id == 202)));
        // This ordinary Join recomputes visibility from the canonical player,
        // rather than relying on having observed SetElemental earlier.
        let rejoin = zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: observer,
            account_id: "native-victim".into(),
            character_index: 2,
            object_id: 202,
            name: "native-victim".into(),
            class: MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 50,
            hp: 500,
            max_hp: 500,
            mp: 500,
            map_file_name: "five-class-skills-unit".into(),
            position: Point { x: 21, y: 20 },
            direction: MirDirection::Left,
            chat_profile: ZoneChatProfile::default(),
            combat_stats: ZonePlayerCombatStats::default(),
        }));
        assert!(packets(&rejoin).iter().any(|packet| matches!(packet,
            ServerPacket::ObjectPlayer { info }
            if info.object_id == 101 && info.element_orb_effect == 2
                && info.element_orb_level == 100 && info.element_orb_max == 200)));
        let transfer = zone.take_online_five_class_state(&session).unwrap();
        {
            let player = zone.players.get_mut(&session).unwrap();
            player.element_orb_effect = 0;
            player.element_orb_level = 0;
            player.element_orb_max = 0;
        }
        zone.restore_online_five_class_state(&session, transfer);
        let player = &zone.players[&session];
        assert_eq!(
            (
                player.element_orb_effect,
                player.element_orb_level,
                player.element_orb_max
            ),
            (2, 100, 200)
        );
        zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
            session_id: session.clone(),
            hp: 0,
            max_hp: 500,
            mp: 500,
            dead: true,
        });
        let player = &zone.players[&session];
        assert_eq!(
            (
                player.element_orb_effect,
                player.element_orb_level,
                player.element_orb_max
            ),
            (0, 0, 0)
        );
    }

    #[test]
    fn backstep_checks_every_occupied_cell_and_reports_real_travel() {
        let (mut zone, session) = fixture(MirClass::Archer);
        monster(&mut zone, 900, Point { x: 18, y: 20 }, 10);
        let out = cast(&mut zone, &session, Spell::BackStep, 3, 0, 100);
        assert_eq!(zone.players[&session].position, Point { x: 19, y: 20 });
        assert!(packets(&out)
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectBackStep { distance: 1, .. })));
        assert_eq!(zone.players[&session].movement_ready_at_ms, 600);
        assert_eq!(zone.occupancy.get(&(19, 20)), Some(&session));
    }

    #[test]
    fn clone_is_authoritative_player_appearance_and_explodes_only_in_source_rays() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        monster(&mut zone, 900, Point { x: 21, y: 20 }, 10);
        monster(&mut zone, 901, Point { x: 22, y: 21 }, 10); // off both 16 source rays
        cast(&mut zone, &session, Spell::DarkBody, 2, 900, 100);
        let id = *zone.five_class_skills.clones.keys().next().unwrap();
        assert_eq!(zone.native_monsters[&id].max_hp, 1500);
        assert_eq!(
            zone.native_monsters[&id].owner_session_id.as_ref(),
            Some(&session)
        );
        assert!(matches!(
            zone.objects[&id].packet,
            ServerPacket::ObjectPlayer { .. }
        ));
        assert!(
            zone.player_can_attack_owned_pet(&session, id, 600),
            "the clone has a real Node and current life"
        );
        assert_eq!(
            zone.selected_owned_pet_target(id, 600)
                .unwrap()
                .reference
                .object_id(),
            900,
            "the lawful launch target must bind after the clone Node exists"
        );
        let position = zone.native_monsters[&id].position.clone();
        assert!(zone.object_grid.neighbors(&position).contains(&id));
        assert!(zone.players[&session].visible_object_ids.contains(&id));
        zone.try_tick_five_class_clone(id, 10_100);
        assert!(
            zone.native_monsters.contains_key(&id),
            "source expiry is strict >"
        );
        assert!(zone.object_grid.neighbors(&position).contains(&id));
        let out = zone.try_tick_five_class_clone(id, 10_101).unwrap();
        assert!(!zone.native_monsters.contains_key(&id));
        assert!(zone.native_monsters[&900].hp < 10_000);
        assert_eq!(zone.native_monsters[&901].hp, 10_000);
        assert!(removed(&out, id));
        assert_object_retired(&zone, id, &position);
    }

    #[test]
    fn clone_source_recast_explosion_retires_node_grid_and_visibility_immediately() {
        let (mut zone, session) = fixture(MirClass::Assassin);
        monster(&mut zone, 900, Point { x: 21, y: 20 }, 10);
        cast(&mut zone, &session, Spell::DarkBody, 2, 900, 100);
        let id = *zone.five_class_skills.clones.keys().next().unwrap();
        let position = zone.native_monsters[&id].position.clone();
        assert_eq!(zone.native_monsters[&id].hp, 1500);
        assert!(zone.object_grid.neighbors(&position).contains(&id));
        let out = cast(&mut zone, &session, Spell::DarkBody, 2, 900, 2000);
        assert!(zone.five_class_skills.clones.is_empty());
        assert!(!zone.native_monsters.contains_key(&id));
        assert!(zone.native_monsters[&900].hp < 10_000);
        assert!(removed(&out, id));
        assert_object_retired(&zone, id, &position);
    }

    #[test]
    fn stale_incarnation_never_receives_old_projectile_or_practice() {
        let (mut zone, session) = fixture(MirClass::Archer);
        monster(&mut zone, 900, Point { x: 21, y: 20 }, 50);
        cast(&mut zone, &session, Spell::VampireShot, 2, 900, 100);
        let next = zone.allocate_monster_incarnation();
        zone.native_monsters.get_mut(&900).unwrap().incarnation = next;
        let out = zone.tick_five_class_skills(650);
        assert_eq!(zone.native_monsters[&900].hp, 10_000);
        assert!(zone.five_class_skills.vampires.is_empty());
        assert!(!out
            .iter()
            .any(|o| matches!(o, ZoneOutbound::FiveClassMagicPractice { .. })));
    }

    #[test]
    fn live_transfer_retains_deadlines_but_new_life_cannot_adopt_it() {
        for new_life in [false, true] {
            let (mut zone, session) = fixture(MirClass::Archer);
            cast(&mut zone, &session, Spell::Concentration, 2, 0, 100);
            zone.five_class_obtain_element(&session, true);
            zone.tick_five_class_skills(1100);
            let transfer = zone.take_online_five_class_state(&session).unwrap();
            assert!(zone.players[&session].buffs.contains_key(&CONCENTRATION));
            if new_life {
                zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: session.clone(),
                    hp: 0,
                    max_hp: 500,
                    mp: 500,
                    dead: true,
                });
                zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: session.clone(),
                    hp: 500,
                    max_hp: 500,
                    mp: 500,
                    dead: false,
                });
            }
            let out = zone.restore_online_five_class_state(&session, transfer);
            if new_life {
                assert!(out.is_empty());
                assert!(!zone.five_class_skills.actors.contains_key(&session));
            } else {
                let buff = &zone.players[&session].buffs[&CONCENTRATION];
                assert_eq!(buff.expires_at_ms, Some(75_100));
                assert_eq!(buff.buff.expire_time, 74_000);
                assert_eq!(zone.five_class_skills.actors[&session].elements, 100);
                assert!(packets(&out).iter().any(|p| matches!(
                    p,
                    ServerPacket::SetElemental {
                        value: 100,
                        enabled: true,
                        ..
                    }
                )));
            }
        }
    }
}

#[derive(Debug, Clone)]
struct PendingFiveClassSkill {
    owner: OwnedPetOwner,
    due_ms: u64,
    spell: Spell,
    level: u8,
    damage: i32,
    action: FiveClassAction,
}

#[derive(Debug, Clone)]
enum FiveClassAction {
    SelfBuff,
    Projectile(FiveClassTarget),
    Direction {
        origin: Point,
        direction: MirDirection,
    },
    Trap(FiveClassTarget),
}

#[derive(Debug, Clone)]
struct FiveClassExplosion {
    owner: OwnedPetOwner,
    target: FiveClassTarget,
    damage: i32,
    level: u8,
    next_tick_ms: u64,
    inflicted_at_ms: u64,
    stage: u8,
    ticks: u64,
    duration: u64,
}

#[derive(Debug, Clone)]
struct FiveClassBleed {
    owner: OwnedPetOwner,
    target: FiveClassTarget,
    value: i32,
    ticks_left: u64,
    next_tick_ms: u64,
    mask: u16,
    tick_speed_ms: u64,
}

#[derive(Debug, Clone)]
struct FiveClassClone {
    owner: OwnedPetOwner,
    incarnation: u64,
    expires_at_ms: u64,
    attack_power_spent: i32,
}

/// Live SpellObject.Target binding. Neither a saved object id nor a new
/// monster in the same spawn slot can acquire this lease's ShockTime.
#[derive(Debug, Clone)]
struct FiveClassTrap {
    target: FiveClassTarget,
    position: Point,
    expires_at_ms: u64,
    before_control_until_ms: u64,
    before_ai_ready_at_ms: u64,
    before_attack_ready_at_ms: u64,
    original_owned_shock_until_ms: u64,
}

#[derive(Debug, Clone)]
struct FiveClassVampire {
    owner: OwnedPetOwner,
    amount: i32,
    next_tick_ms: u64,
}

fn specialized(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::Haste
            | Spell::SwiftFeet
            | Spell::LightBody
            | Spell::MoonLight
            | Spell::DarkBody
            | Spell::PoisonSword
            | Spell::HeavenlySword
            | Spell::CrescentSlash
            | Spell::FlashDash
            | Spell::FireBurst
            | Spell::Trap
            | Spell::Concentration
            | Spell::ElementalShot
            | Spell::ElementalBarrier
            | Spell::BackStep
            | Spell::DelayedExplosion
            | Spell::VampireShot
            | Spell::PoisonShot
            | Spell::CrippleShot
            | Spell::CatTongue
    )
}

fn aimed(spell: Spell) -> bool {
    matches!(
        spell,
        Spell::DarkBody
            | Spell::Trap
            | Spell::DelayedExplosion
            | Spell::VampireShot
            | Spell::PoisonShot
            | Spell::CrippleShot
            | Spell::CatTongue
    )
}

fn elemental_available(elements: u32) -> bool {
    elements >= ORB_EXP[0]
}

fn clear_superseded_wild_green_timer(monster: &mut ZoneNativeMonster) {
    monster.damage_poison &= !CRYSTAL_POISON_GREEN;
    if monster.damage_poison == 0 {
        monster.damage_poison_value = 0;
        monster.damage_poison_next_damage_at_ms = 0;
        monster.damage_poison_expires_at_ms = 0;
        monster.damage_poison_owner_session_id = None;
        monster.damage_poison_owner_object_id = 0;
    }
}

fn five_class_poison_attack_bonus(player: &ZonePlayer, now: u64) -> i32 {
    // The trusted projection already accumulates the mirrored buff stats.
    let maximum = player.combat_stats.poison_attack.max(0);
    if maximum == 0 {
        0
    } else {
        zone_roll_stat_range(0, maximum - 1, now, player.object_id, 0xC10D)
    }
}

fn orb_count(elements: u32) -> usize {
    ORB_EXP
        .iter()
        .take_while(|threshold| elements >= **threshold)
        .count()
}

pub(super) fn source_attack_delay(player: &ZonePlayer) -> u64 {
    // Trusted stats already include the mirrored native Haste buff.
    let speed = player.combat_stats.attack_speed;
    (1400_i32
        .saturating_sub(
            speed
                .saturating_mul(60)
                .saturating_add((i32::from(player.level) * 14).min(370)),
        )
        .max(550)) as u64
}

impl ZoneRuntime {
    fn five_class_actor_mut(&mut self, session: &SessionId) -> Option<&mut FiveClassActor> {
        let owner = self.owned_pet_owner(session)?;
        let actor = self
            .five_class_skills
            .actors
            .entry(session.clone())
            .or_insert_with(|| FiveClassActor::new(owner.clone()));
        if actor.owner != owner {
            *actor = FiveClassActor::new(owner);
        }
        Some(actor)
    }

    pub(super) fn five_class_target(&self, id: u32) -> Option<FiveClassTarget> {
        if let Some((session, player)) = self
            .players
            .iter()
            .find(|(_, player)| player.object_id == id)
        {
            return Some(FiveClassTarget {
                entity: self.native_entity_player_ref(session)?,
                online: Some(self.online_presence.get(session)?.owner.clone()),
                launch_position: player.position.clone(),
            });
        }
        let monster = self.native_monsters.get(&id)?;
        Some(FiveClassTarget {
            entity: self.native_entity_monster_ref(id)?,
            online: None,
            launch_position: monster.position.clone(),
        })
    }

    fn five_class_target_current(&self, target: &FiveClassTarget) -> bool {
        self.entity_ref_exists(&target.entity, false)
            && target.online.as_ref().is_none_or(|owner| {
                self.online_presence
                    .get(&owner.session_id)
                    .is_some_and(|presence| presence.owner == *owner)
            })
    }

    pub(super) fn five_class_target_position(&self, target: &FiveClassTarget) -> Option<Point> {
        if !self.five_class_target_current(target) {
            return None;
        }
        match &target.entity {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                self.players.get(session_id).map(|p| p.position.clone())
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => self
                .native_monsters
                .get(object_id)
                .map(|m| m.position.clone()),
        }
    }

    pub(super) fn five_class_target_hostile(
        &self,
        session: &SessionId,
        target: &FiveClassTarget,
        now: u64,
    ) -> bool {
        if !self.five_class_target_current(target) {
            return false;
        }
        match &target.entity {
            ZoneCombatEntityRef::Player {
                session_id: victim, ..
            } => self
                .players
                .get(session)
                .zip(self.players.get(victim))
                .is_some_and(|(source, target)| {
                    self.conquest_player_can_attack_player(source, target, now)
                }),
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                self.native_monsters.get(object_id).is_some_and(|m| {
                    !m.dead
                        && m.hp > 0
                        && monster_visibility_is_attackable(m)
                        && if m.owner_session_id.is_some() {
                            self.player_can_attack_owned_pet(session, *object_id, now)
                        } else {
                            m.hostile_to_player
                        }
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn can_cast_five_class_magic(
        &self,
        session_id: &SessionId,
        object_id: u32,
        spell: Spell,
        _direction: MirDirection,
        target: &Point,
        cast: bool,
        _level: u8,
        _damage: i32,
        mp_cost: i32,
        _cooldown_ms: u64,
        _item_param: u8,
        now_ms: u64,
    ) -> Option<bool> {
        if !specialized(spell) {
            return None;
        }
        let Some(player) = self.players.get(session_id) else {
            return Some(false);
        };
        let Some(identity) = ZoneFiveClassPracticeSpell::from_spell(spell) else {
            return Some(false);
        };
        if player.class != identity.class()
            || player.combat_stats.five_class_learned_mask & identity.learned_bit() == 0
            || self.owned_pet_owner(session_id).is_none()
        {
            return Some(false);
        }
        if cast
            && (player.mp < mp_cost.max(0)
                || now_ms < player.next_spell_ready_at_ms
                || now_ms < player.movement_ready_at_ms
                || zone_player_status_blocks_cast(player, now_ms)
                || now_ms
                    < player
                        .magic_ready_at_ms
                        .get(&(spell as u8))
                        .copied()
                        .unwrap_or_default())
        {
            return Some(false);
        }
        if matches!(spell, Spell::DarkBody)
            && (crystal_monster_by_name("AssassinClone").is_none()
                || self
                    .npc_teleport_config
                    .map(&self.key.map_file_name)
                    .is_some_and(|m| m.no_pets))
        {
            return Some(false);
        }
        let elements = self
            .five_class_skills
            .actors
            .get(session_id)
            .filter(|actor| self.owned_pet_owner_is_current(&actor.owner))
            .map_or(0, |actor| actor.elements);
        let needs_target =
            aimed(spell) || (spell == Spell::ElementalShot && elemental_available(elements));
        if matches!(spell, Spell::ElementalShot | Spell::ElementalBarrier)
            && player.combat_stats.meditation_level.is_none()
        {
            return Some(false);
        }
        if !needs_target {
            return Some(true);
        }
        let Some(victim) = self.five_class_target(object_id) else {
            return Some(false);
        };
        let Some(position) = self.five_class_target_position(&victim) else {
            return Some(false);
        };
        // HumanObject.Magic first checks MagicInfo.Range, then resolves a
        // target through FindObject(targetID, 10). Range zero has no first
        // restriction, but must not bypass that object lookup boundary.
        let source_range =
            crystal_magic_by_spell(&format!("{spell:?}")).map_or(10, |magic| match magic.range {
                0 => 10,
                range => i32::from(range).min(10),
            });
        if (spell != Spell::DarkBody
            && !self.five_class_target_hostile(session_id, &victim, now_ms))
            || zone_tile_distance(target, &position) > 1
            || zone_tile_distance(&player.position, &position) > source_range
        {
            return Some(false);
        }
        if spell == Spell::Trap {
            return Some(
                self.native_monsters
                    .get(&object_id)
                    .is_some_and(|monster| monster.level < player.level.saturating_add(2)),
            );
        }
        if spell == Spell::DarkBody {
            return Some(true);
        }
        let trickshot = player.class == MirClass::Archer && player.combat_stats.mental_state == 1;
        Some(trickshot || self.native_projectile_path_clear(&player.position, &position))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_cast_five_class_magic(
        &mut self,
        session_id: &SessionId,
        object_id: u32,
        spell: Spell,
        direction: MirDirection,
        target: Point,
        cast: bool,
        level: u8,
        damage: i32,
        mp_cost: i32,
        cooldown_ms: u64,
        item_param: u8,
        now_ms: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        self.five_class_skills.last_now_ms = self.five_class_skills.last_now_ms.max(now_ms);
        let accepted = self.can_cast_five_class_magic(
            session_id,
            object_id,
            spell,
            direction,
            &target,
            cast,
            level,
            damage,
            mp_cost,
            cooldown_ms,
            item_param,
            now_ms,
        )?;
        if !accepted {
            return Some(self.correct_player_location(session_id, now_ms));
        }
        let owner = self.owned_pet_owner(session_id)?;
        let elements = self.five_class_actor_mut(session_id)?.elements;
        let player = self.players.get_mut(session_id)?;
        player.direction = direction;
        if cast {
            player.change_owner_mp(-mp_cost.max(0));
            player.movement_actions.clear();
            player.movement_ready_at_ms = now_ms.saturating_add(600);
            player.next_attack_ready_at_ms = player
                .next_attack_ready_at_ms
                .max(now_ms.saturating_add(600));
            player.next_spell_ready_at_ms =
                now_ms.saturating_add(if spell == Spell::FlashDash { 300 } else { 1800 });
            player
                .magic_ready_at_ms
                .insert(spell as u8, now_ms.saturating_add(cooldown_ms.max(1)));
        }
        let player = player.clone();
        let mut out = self.take_owner_health_through(session_id, u64::MAX);
        out.push(ZoneOutbound::ToSession {
            session_id: session_id.clone(),
            packets: vec![user_location_packet(&player)],
        });
        if !cast {
            return Some(out);
        }
        // Magic itself breaks the two Assassin stealth buffs in HumanObject.
        out.extend(self.five_class_clear_stealth(session_id));
        let mut presentation_cast = true;
        let mut presentation_target = object_id;
        let mut action = None;
        let raw = self.five_class_spell_damage(&player, &target, spell, level, damage, now_ms);
        match spell {
            Spell::Haste | Spell::LightBody | Spell::ElementalBarrier => {
                action = Some((500, FiveClassAction::SelfBuff));
            }
            Spell::SwiftFeet | Spell::MoonLight | Spell::Concentration => {
                out.extend(self.five_class_self_buff(session_id, spell, level, now_ms));
            }
            Spell::DarkBody => {
                out.extend(self.five_class_dark_body(&owner, object_id, level, now_ms))
            }
            Spell::PoisonSword => {
                for offset in -1..=3 {
                    let point = offset_point(
                        &player.position,
                        zone_rotated_direction(direction, offset),
                        1,
                    );
                    if let Some(victim) =
                        self.five_class_first_target_in_cell(session_id, &point, now_ms)
                    {
                        let ticks = (3 + raw.max(0) / 10 + i32::from(level) * 3).max(1) as u64;
                        let value = raw.max(0) / 10
                            + i32::from(level)
                            + 1
                            + five_class_poison_attack_bonus(&player, now_ms);
                        out.extend(self.five_class_apply_poison(
                            &owner,
                            &victim,
                            CRYSTAL_POISON_GREEN,
                            value,
                            ticks,
                            1000,
                            now_ms,
                        ));
                    }
                }
                out.extend(self.five_class_practice(session_id, spell, now_ms));
            }
            Spell::FireBurst => {
                let packets = self.apply_native_player_repulsion(session_id, level, now_ms);
                out.extend(self.five_class_present(session_id, packets));
                // Repulsion levels only when an actual push succeeds.
                if out.iter().any(|o| {
                    outbound_packets(o)
                        .iter()
                        .any(|p| matches!(p, ServerPacket::ObjectPushed { .. }))
                }) {
                    out.extend(self.five_class_practice(session_id, spell, now_ms));
                }
            }
            Spell::FlashDash | Spell::BackStep => {
                out.extend(self.five_class_direction_move(session_id, spell, level, raw, now_ms));
            }
            Spell::HeavenlySword | Spell::CrescentSlash => {
                let delay = if spell == Spell::HeavenlySword {
                    500
                } else {
                    source_attack_delay(&player)
                };
                action = Some((
                    delay,
                    FiveClassAction::Direction {
                        origin: player.position.clone(),
                        direction,
                    },
                ));
            }
            Spell::Trap => {
                action = self
                    .five_class_target(object_id)
                    .map(|target| (500, FiveClassAction::Trap(target)));
                out.extend(self.five_class_practice(session_id, spell, now_ms));
            }
            Spell::ElementalShot if !elemental_available(elements) => {
                out.extend(self.five_class_obtain_element(session_id, true));
                out.extend(self.five_class_practice(session_id, spell, now_ms));
                presentation_target = 0;
            }
            _ => {
                if let Some(victim) = self.five_class_target(object_id) {
                    let distance =
                        zone_tile_distance(&player.position, &victim.launch_position).max(0) as u64;
                    action = Some((500 + distance * 50, FiveClassAction::Projectile(victim)));
                } else {
                    presentation_cast = false;
                }
            }
        }
        if let Some((delay, action)) = action {
            self.five_class_skills.pending.push(PendingFiveClassSkill {
                owner,
                due_ms: now_ms.saturating_add(delay),
                spell,
                level,
                damage: raw,
                action,
            });
        }
        if !matches!(spell, Spell::BackStep | Spell::FlashDash) {
            out.push(ZoneOutbound::ToSession {
                session_id: session_id.clone(),
                packets: vec![ServerPacket::Magic {
                    spell,
                    target_id: presentation_target,
                    target: target.clone(),
                    cast: presentation_cast,
                    level,
                    secondary_target_ids: Vec::new(),
                }],
            });
            out.extend(self.five_class_present(
                session_id,
                vec![
                    ServerPacket::ObjectMagic {
                        object_id: player.object_id,
                        location: player.position.clone(),
                        direction,
                        spell,
                        target_id: presentation_target,
                        target,
                        cast: presentation_cast,
                        level,
                        self_broadcast: false,
                        secondary_target_ids: Vec::new(),
                    },
                    ServerPacket::ObjectMana {
                        info: ObjectManaInfo {
                            object_id: player.object_id,
                            percent: zone_five_class_mana_percent(&player),
                        },
                    },
                ],
            ));
        }
        Some(out)
    }

    fn five_class_spell_damage(
        &self,
        player: &ZonePlayer,
        target: &Point,
        spell: Spell,
        level: u8,
        fallback: i32,
        now: u64,
    ) -> i32 {
        let Some(magic) = crystal_magic_by_spell(&format!("{spell:?}")) else {
            return fallback.max(0);
        };
        let (min, max) = if player.class == MirClass::Archer {
            (player.combat_stats.min_mc, player.combat_stats.max_mc)
        } else {
            (player.combat_stats.min_dc, player.combat_stats.max_dc)
        };
        let special_arrow = matches!(
            spell,
            Spell::VampireShot | Spell::PoisonShot | Spell::CrippleShot
        );
        let minimum = if special_arrow {
            let distance = zone_tile_distance(&player.position, target);
            min.saturating_sub((i64::from(min) * i64::from(9 - distance)).div_euclid(9) as i32)
        } else {
            min
        };
        let mut power = zone_roll_attack_power(
            minimum,
            max,
            player.combat_stats.luck,
            now,
            player.object_id,
            0x5C50 + u64::from(spell as u8),
        );
        if spell == Spell::ElementalShot {
            power = power.saturating_add(
                4 * orb_count(
                    self.five_class_skills
                        .actors
                        .get(&player.session_id)
                        .map_or(0, |a| a.elements),
                ) as i32,
            );
        }
        if spell == Spell::CrescentSlash
            && zone_deterministic_roll(now, player.object_id as usize, 0xC5, 100)
                <= player.combat_stats.accuracy.max(0) as u64
        {
            power = power.saturating_mul(2);
        }
        let damage = crate::runtime::combat::crystal_magic_damage_from_base(&magic, level, power);
        if special_arrow {
            let percent = match player.combat_stats.mental_state {
                1 => 55 + 5 * i32::from(player.combat_stats.mental_state_level.min(3)),
                2 => 80,
                _ => 100,
            };
            damage.saturating_mul(percent) / 100
        } else {
            damage
        }
    }

    fn five_class_present(
        &mut self,
        session: &SessionId,
        packets: Vec<ServerPacket>,
    ) -> Vec<ZoneOutbound> {
        if packets.is_empty() {
            return Vec::new();
        }
        let Some(player) = self.players.get(session) else {
            return Vec::new();
        };
        vec![ZoneOutbound::ToMany {
            session_ids: self.player_status_recipients(player.object_id, &player.position),
            packets,
        }]
    }

    fn five_class_first_target_in_cell(
        &self,
        session: &SessionId,
        point: &Point,
        now: u64,
    ) -> Option<FiveClassTarget> {
        self.players
            .values()
            .map(|p| p.object_id)
            .chain(self.native_monsters.keys().copied())
            .filter_map(|id| self.five_class_target(id))
            .find(|target| {
                self.five_class_target_position(target).as_ref() == Some(point)
                    && self.five_class_target_hostile(session, target, now)
            })
    }

    fn five_class_self_buff(
        &mut self,
        session: &SessionId,
        spell: Spell,
        level: u8,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session).cloned() else {
            return Vec::new();
        };
        let (buff_type, duration, stats) = match spell {
            Spell::Haste => (
                HASTE,
                25_000 + 15_000 * u64::from(level),
                vec![UserItemStat {
                    stat: 14,
                    value: 2 + 2 * i32::from(level),
                }],
            ),
            Spell::LightBody => (
                LIGHT_BODY,
                30_000 * (u64::from(level) + 1),
                vec![UserItemStat {
                    stat: 11,
                    value: 2 * (i32::from(level) + 1),
                }],
            ),
            Spell::SwiftFeet => (
                CRYSTAL_SWIFT_FEET_BUFF_TYPE,
                25_000 + 5_000 * u64::from(level),
                Vec::new(),
            ),
            Spell::MoonLight | Spell::DarkBody => {
                let ac = zone_roll_attack_power(
                    player.combat_stats.min_ac,
                    player.combat_stats.max_ac,
                    player.combat_stats.luck,
                    now,
                    player.object_id,
                    0xDAC,
                );
                let duration = (ac.max(0) as u64 + (u64::from(level) + 1) * 5) * 500;
                (
                    if spell == Spell::MoonLight {
                        MOON_LIGHT
                    } else {
                        DARK_BODY
                    },
                    duration,
                    Vec::new(),
                )
            }
            Spell::Concentration => (
                CONCENTRATION,
                45_000 + 15_000 * u64::from(level),
                Vec::new(),
            ),
            Spell::ElementalBarrier => {
                if player.buffs.contains_key(&ELEMENTAL_BARRIER) {
                    return Vec::new();
                }
                let elements = self.five_class_actor_mut(session).map_or(0, |a| a.elements);
                if !elemental_available(elements) {
                    let mut out = self.five_class_obtain_element(session, true);
                    out.extend(self.five_class_practice(session, spell, now));
                    return out;
                }
                let orb_power = (orb_count(elements) * 2) as i32;
                let power = zone_roll_attack_power(
                    player.combat_stats.min_mc,
                    player.combat_stats.max_mc,
                    player.combat_stats.luck,
                    now,
                    player.object_id,
                    0xBA11,
                );
                let duration = crystal_magic_by_spell("ElementalBarrier")
                    .map(|magic| {
                        crate::runtime::skills::crystal_magic_get_power(&magic, level)
                            .saturating_add(power)
                    })
                    .unwrap_or(power)
                    .saturating_add(orb_power)
                    .max(1) as u64
                    * 1000;
                if let Some(actor) = self.five_class_actor_mut(session) {
                    actor.elements = 0;
                }
                (
                    ELEMENTAL_BARRIER,
                    duration,
                    vec![UserItemStat {
                        stat: CRYSTAL_STAT_DAMAGE_REDUCTION_PERCENT,
                        value: (i32::from(level) + 1) * 10,
                    }],
                )
            }
            _ => return Vec::new(),
        };
        let mut packets = self.apply_native_player_buff(session, buff_type, duration, stats, now);
        if let Some(actor) = self.five_class_actor_mut(session) {
            if matches!(spell, Spell::MoonLight | Spell::DarkBody) {
                actor.stealth_spell = Some(spell);
                actor.stealth_level = level;
            }
            if spell == Spell::Concentration {
                actor.concentration_level = level;
                actor.concentration_enabled = true;
                actor.concentration_interrupted_until = 0;
            }
        }
        if matches!(spell, Spell::MoonLight | Spell::DarkBody) {
            if let Some(player) = self.players.get_mut(session) {
                player.hidden = true;
            }
            packets.push(ServerPacket::ObjectHidden {
                object_id: player.object_id,
                hidden: true,
            });
        }
        if spell == Spell::Concentration {
            packets.push(ServerPacket::SetConcentration {
                object_id: player.object_id,
                enabled: true,
                interrupted: false,
            });
        }
        if spell == Spell::ElementalBarrier {
            packets.push(effect(player.object_id, 13, 0));
        }
        let mut out = self.five_class_present(session, packets);
        if spell == Spell::ElementalBarrier {
            out.extend(self.five_class_obtain_element(session, false));
        }
        out.extend(self.five_class_practice(session, spell, now));
        out
    }

    fn five_class_obtain_element(&mut self, session: &SessionId, cast: bool) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get(session).cloned() else {
            return Vec::new();
        };
        let Some(level) = player.combat_stats.meditation_level else {
            return Vec::new();
        };
        let Some(actor) = self.five_class_actor_mut(session) else {
            return Vec::new();
        };
        let mut packets = Vec::new();
        if cast {
            actor.elements = ORB_EXP[0];
            if level >= 3 {
                packets.push(ServerPacket::SetElemental {
                    object_id: player.object_id,
                    enabled: true,
                    casted: true,
                    value: ORB_EXP[0],
                    element_type: 1,
                    exp_last: ORB_EXP[3],
                });
                actor.elements = ORB_EXP[1];
            }
        } else {
            // Source deliberately increments to one when spending all orbs.
            actor.elements = actor
                .elements
                .saturating_add(1)
                .min(ORB_EXP[usize::from(level.min(3))]);
        }
        let elements = actor.elements;
        let element_type = ORB_EXP
            .iter()
            .position(|threshold| *threshold == elements)
            .map_or(0, |i| i as u32 + 1);
        packets.push(ServerPacket::SetElemental {
            object_id: player.object_id,
            enabled: elemental_available(elements),
            casted: cast,
            value: elements,
            element_type,
            exp_last: ORB_EXP[3],
        });
        let projection = self.players.get_mut(session).unwrap();
        projection.element_orb_effect = orb_count(elements) as u32;
        projection.element_orb_level = elements;
        projection.element_orb_max = ORB_EXP[3];
        self.five_class_present(session, packets)
    }

    pub(super) fn five_class_player_moved(
        &mut self,
        session: &SessionId,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        self.five_class_skills.last_now_ms = self.five_class_skills.last_now_ms.max(now);
        let enabled = self
            .players
            .get(session)
            .is_some_and(|player| player.buffs.contains_key(&CONCENTRATION));
        if !enabled {
            return Vec::new();
        }
        let Some(actor) = self.five_class_actor_mut(session) else {
            return Vec::new();
        };
        let first = actor.concentration_interrupted_until == 0;
        actor.concentration_interrupted_until = now.saturating_add(3000);
        if !first {
            return Vec::new();
        }
        let id = self.players[session].object_id;
        self.five_class_present(
            session,
            vec![ServerPacket::SetConcentration {
                object_id: id,
                enabled: true,
                interrupted: true,
            }],
        )
    }

    pub(super) fn five_class_clear_stealth(&mut self, session: &SessionId) -> Vec<ZoneOutbound> {
        let had_stealth = self
            .five_class_skills
            .actors
            .get(session)
            .is_some_and(|actor| actor.stealth_spell.is_some());
        let Some(player) = self.players.get_mut(session) else {
            return Vec::new();
        };
        let mut packets = Vec::new();
        for buff_type in [MOON_LIGHT, DARK_BODY] {
            if player.buffs.remove(&buff_type).is_some() {
                packets.push(ServerPacket::RemoveBuff {
                    object_id: player.object_id,
                    buff_type,
                });
            }
        }
        if packets.is_empty() && !had_stealth {
            return Vec::new();
        }
        player.hidden =
            player.buffs.contains_key(&CRYSTAL_HIDING_BUFF_TYPE) || player.buffs.contains_key(&114); // ClearRing
        packets.push(ServerPacket::ObjectHidden {
            object_id: player.object_id,
            hidden: player.hidden,
        });
        if let Some(actor) = self.five_class_skills.actors.get_mut(session) {
            actor.stealth_spell = None;
        }
        self.five_class_present(session, packets)
    }

    pub(super) fn take_five_class_stealth_attack(
        &mut self,
        session: &SessionId,
        now: u64,
    ) -> (i32, Vec<ZoneOutbound>) {
        let Some(actor) = self
            .five_class_skills
            .actors
            .get(session)
            .cloned()
            .filter(|actor| self.owned_pet_owner_is_current(&actor.owner))
        else {
            return (0, Vec::new());
        };
        let Some(spell) = actor.stealth_spell else {
            return (0, Vec::new());
        };
        let buff_type = if spell == Spell::MoonLight {
            MOON_LIGHT
        } else {
            DARK_BODY
        };
        let active = self
            .players
            .get(session)
            .and_then(|player| player.buffs.get(&buff_type))
            .is_some_and(|buff| buff.expires_at_ms.is_none_or(|expiry| now < expiry));
        let bonus = if active {
            crystal_magic_by_spell(&format!("{spell:?}"))
                .map(|magic| {
                    crate::runtime::skills::crystal_magic_get_power(&magic, actor.stealth_level)
                })
                .unwrap_or(0)
        } else {
            0
        };
        (bonus, self.five_class_clear_stealth(session))
    }
}

fn effect(object_id: u32, effect: u8, effect_type: u32) -> ServerPacket {
    ServerPacket::ObjectEffect {
        info: ObjectEffectInfo {
            object_id,
            effect,
            effect_type,
            delay_time: 0,
            time: 0,
        },
    }
}

fn outbound_packets(out: &ZoneOutbound) -> &[ServerPacket] {
    match out {
        ZoneOutbound::ToSession { packets, .. }
        | ZoneOutbound::ToMany { packets, .. }
        | ZoneOutbound::ToAll { packets } => packets,
        _ => &[],
    }
}

impl ZoneRuntime {
    fn five_class_create_trap(
        &mut self,
        session: &SessionId,
        target: FiveClassTarget,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let object_id = target.entity.object_id();
        let Some(player) = self.players.get(session).cloned() else {
            return Vec::new();
        };
        if player.class != MirClass::Assassin
            || player.combat_stats.five_class_learned_mask
                & ZoneFiveClassPracticeSpell::Trap.learned_bit()
                == 0
            || self.owned_pet_owner(session).is_none()
            || !matches!(target.entity, ZoneCombatEntityRef::Monster { .. })
            || !self.five_class_target_hostile(session, &target, now)
        {
            return Vec::new();
        }
        let Some(monster) = self.native_monsters.get(&object_id).cloned() else {
            return Vec::new();
        };
        if monster.level >= player.level.saturating_add(2) {
            return Vec::new();
        }
        // A recast refreshes ShockTime, but the original ordinary action clocks
        // must not become the previous Trap's sixty-second synthetic freeze.
        let inherited = self
            .five_class_skills
            .traps
            .values()
            .filter(|trap| trap.target.entity == target.entity)
            .min_by_key(|trap| trap.expires_at_ms)
            .cloned();
        let (before_control, before_ai, before_attack, original_owned_shock) =
            if let Some(previous) = inherited {
                (
                    previous.before_control_until_ms,
                    previous.before_ai_ready_at_ms,
                    previous.before_attack_ready_at_ms,
                    previous.original_owned_shock_until_ms,
                )
            } else {
                (
                    if monster.control_poison == 0 {
                        0
                    } else {
                        monster.control_until_ms
                    },
                    monster.next_ai_ready_at_ms,
                    monster.next_attack_ready_at_ms,
                    self.five_class_owned_trap_shock_until(&target.entity)
                        .unwrap_or(0),
                )
            };
        // The old three-class helper only admits hostile-to-player wild
        // monsters. Source Monster.IsAttackTarget(Human) also admits lawful
        // owned pets, through the exact mode/safe/group/guild checks above.
        let expires_at_ms = now.saturating_add(60_000);
        let controlled = self.native_monsters.get_mut(&object_id).unwrap();
        controlled.control_until_ms = controlled.control_until_ms.max(expires_at_ms);
        controlled.next_ai_ready_at_ms = controlled.next_ai_ready_at_ms.max(expires_at_ms);
        controlled.next_attack_ready_at_ms = controlled.next_attack_ready_at_ms.max(expires_at_ms);
        controlled.assassin_trap_control_until_ms = Some(expires_at_ms);
        self.clear_native_entity_target(object_id);
        // Real owned ShockTime also clears its assigned target, freezes the
        // pet's movement gate and updates its existing Source name/clock packet.
        let mut out =
            self.apply_five_class_prevalidated_owned_trap_shock(&target.entity, expires_at_ms, now);
        let id = self.unique_object_id(0);
        let position = monster.position;
        let packet = ServerPacket::ObjectSpell {
            info: ObjectSpellInfo {
                object_id: id,
                location: position.clone(),
                spell: Spell::Trap,
                direction: player.direction,
                // Source SpellObject.GetInfo only ExplosiveTrap sets Param.
                param: false,
            },
        };
        self.visibility_now_ms = self.visibility_now_ms.max(now);
        self.object_grid.insert(id, &position);
        self.objects.insert(
            id,
            ZoneObject {
                object_id: id,
                position: position.clone(),
                packet,
                health: None,
                mana: None,
                expires_at_ms: Some(expires_at_ms),
                buffs: BTreeMap::new(),
            },
        );
        self.five_class_skills.traps.insert(
            id,
            FiveClassTrap {
                target,
                position,
                expires_at_ms,
                before_control_until_ms: before_control,
                before_ai_ready_at_ms: before_ai,
                before_attack_ready_at_ms: before_attack,
                original_owned_shock_until_ms: original_owned_shock,
            },
        );
        out.extend(self.diff_all_zone_object_visibility());
        // HumanObject.Trap trains once at legal launch; Map.CompleteMagic
        // trains again only after the real target-bound Trap was spawned.
        out.extend(self.five_class_practice(session, Spell::Trap, now));
        out
    }

    fn five_class_trap_node_is_current(&self, id: u32, trap: &FiveClassTrap) -> bool {
        self.objects.get(&id).is_some_and(|object| {
            object.position == trap.position
                && object.expires_at_ms == Some(trap.expires_at_ms)
                && matches!(&object.packet, ServerPacket::ObjectSpell { info }
                    if info.object_id == id && info.spell == Spell::Trap)
        })
    }

    fn five_class_retire_trap(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let Some(trap) = self.five_class_skills.traps.remove(&id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut release_owned = false;
        // Never release a fresh same-id monster or shorten a later independent
        // control/action. Only the exact deadlines installed by this Trap are
        // owned here; the baseline can include an earlier real poison control.
        if self.entity_ref_exists(&trap.target.entity, true) {
            if let Some(monster) = self
                .native_monsters
                .get_mut(&trap.target.entity.object_id())
            {
                if monster.assassin_trap_control_until_ms == Some(trap.expires_at_ms) {
                    release_owned = true;
                    monster.assassin_trap_control_until_ms = None;
                    if monster.control_until_ms == trap.expires_at_ms {
                        monster.control_until_ms = if trap.before_control_until_ms > now {
                            trap.before_control_until_ms
                        } else {
                            0
                        };
                    }
                    if monster.next_ai_ready_at_ms == trap.expires_at_ms {
                        monster.next_ai_ready_at_ms = trap.before_ai_ready_at_ms.max(now);
                    }
                    if monster.next_attack_ready_at_ms == trap.expires_at_ms {
                        monster.next_attack_ready_at_ms = trap.before_attack_ready_at_ms.max(now);
                    }
                }
            }
        }
        if release_owned {
            out.extend(self.release_five_class_owned_trap_shock(
                &trap.target.entity,
                trap.expires_at_ms,
                if trap.original_owned_shock_until_ms > now {
                    trap.original_owned_shock_until_ms
                } else {
                    0
                },
                now,
            ));
        }
        if !self.five_class_trap_node_is_current(id, &trap) {
            return out;
        }
        let observers = self
            .players
            .iter()
            .filter_map(|(session, player)| {
                player
                    .visible_object_ids
                    .contains(&id)
                    .then(|| session.clone())
            })
            .collect::<Vec<_>>();
        self.remove_retained_zone_object(id);
        if !observers.is_empty() {
            out.push(ZoneOutbound::ToMany {
                session_ids: observers,
                packets: vec![ServerPacket::ObjectRemove { object_id: id }],
            });
        }
        out
    }

    /// Source MonsterObject.Attacked clears ShockTime only after an admitted
    /// positive hit. Struck/PoisonDamage merely ChangeHP and do not use this
    /// entry. The caller must provide its actual resolved positive result.
    pub(super) fn five_class_release_trap_on_direct_hit(
        &mut self,
        target_id: u32,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let ids = self
            .five_class_skills
            .traps
            .iter()
            .filter_map(|(id, trap)| {
                (trap.target.entity.object_id() == target_id
                    && self.entity_ref_exists(&trap.target.entity, true)
                    && self.five_class_trap_node_is_current(*id, trap))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        let mut out = Vec::new();
        for id in ids {
            out.extend(self.five_class_retire_trap(id, now));
        }
        out
    }

    fn tick_five_class_traps(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let expired = self
            .five_class_skills
            .traps
            .iter()
            .filter_map(|(id, trap)| {
                (now >= trap.expires_at_ms
                    || !self.five_class_trap_node_is_current(*id, trap)
                    || !self.entity_ref_exists(&trap.target.entity, false)
                    || self.five_class_owned_trap_shock_until(&trap.target.entity) == Some(0)
                    || self
                        .native_monsters
                        .get(&trap.target.entity.object_id())
                        .is_none_or(|monster| monster.control_until_ms == 0))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        let mut out = Vec::new();
        for id in expired {
            out.extend(self.five_class_retire_trap(id, now));
        }
        out
    }

    pub(super) fn tick_five_class_skills(&mut self, now: u64) -> Vec<ZoneOutbound> {
        self.five_class_skills.last_now_ms = self.five_class_skills.last_now_ms.max(now);
        let mut out = std::mem::take(&mut self.five_class_skills.cleanup_outbound);
        out.extend(self.tick_five_class_traps(now));
        self.five_class_skills.clones.retain(|id, clone| {
            self.native_monsters
                .get(id)
                .is_some_and(|monster| monster.incarnation == clone.incarnation)
        });
        let pending = std::mem::take(&mut self.five_class_skills.pending);
        for action in pending {
            if !self.owned_pet_owner_is_current(&action.owner) {
                continue;
            }
            if now < action.due_ms {
                self.five_class_skills.pending.push(action);
                continue;
            }
            let session = &action.owner.online.session_id;
            match action.action {
                FiveClassAction::SelfBuff => {
                    out.extend(self.five_class_self_buff(session, action.spell, action.level, now))
                }
                FiveClassAction::Projectile(target) => {
                    let position = self.five_class_target_position(&target);
                    let valid = position.as_ref().is_some_and(|position| {
                        // CatTongue's CompleteMagic follows its live target;
                        // source does not retain/check the launch location.
                        action.spell == Spell::CatTongue
                            || zone_tile_distance(position, &target.launch_position) <= 2
                    }) && self.five_class_target_hostile(session, &target, now);
                    if action.spell == Spell::ElementalShot && !valid {
                        if let Some(actor) = self.five_class_actor_mut(session) {
                            actor.elements = 0;
                        }
                        out.extend(self.five_class_obtain_element(session, false));
                        continue;
                    }
                    if !valid {
                        continue;
                    }
                    let defence = if matches!(action.spell, Spell::FlashDash | Spell::CatTongue) {
                        EntityDefence::AC
                    } else {
                        EntityDefence::MAC
                    };
                    let (damage, hits) =
                        self.five_class_hit(&action.owner, &target, action.damage, defence, now);
                    out.extend(hits);
                    if damage > 0 && action.spell != Spell::CatTongue {
                        out.extend(self.five_class_practice(session, action.spell, now));
                    }
                    match action.spell {
                        Spell::CatTongue => {
                            if damage > 0 {
                                out.extend(self.five_class_cat_tongue_control(
                                    &action.owner,
                                    &target,
                                    action.level,
                                    now,
                                ));
                                out.extend(self.five_class_practice(session, action.spell, now));
                            }
                        }
                        Spell::FlashDash => {
                            if zone_deterministic_roll(
                                now,
                                action.owner.online.object_id as usize,
                                0xF1A5,
                                15,
                            ) <= u64::from(action.level) + 1
                                && self.five_class_poison_resisted(&target, now)
                            {
                                out.extend(self.five_class_stun(
                                    &target,
                                    u64::from(action.level) + 1,
                                    now,
                                ));
                            }
                        }
                        Spell::ElementalShot => {
                            out.extend(self.five_class_elemental_knockback(
                                session,
                                &target,
                                action.level,
                                now,
                            ));
                            if let Some(actor) = self.five_class_actor_mut(session) {
                                actor.elements = 0;
                            }
                            out.extend(self.five_class_obtain_element(session, false));
                        }
                        Spell::DelayedExplosion => {
                            if self.five_class_target_current(&target)
                                && !self
                                    .five_class_skills
                                    .explosions
                                    .iter()
                                    .any(|e| e.target.entity == target.entity)
                            {
                                let duration = (action.damage.max(0) as u64) * 2
                                    + (u64::from(action.level) + 1) * 7;
                                self.five_class_skills.explosions.push(FiveClassExplosion {
                                    owner: action.owner.clone(),
                                    target: target.clone(),
                                    damage: action.damage,
                                    level: action.level,
                                    next_tick_ms: now.saturating_add(2000),
                                    inflicted_at_ms: now.saturating_add(4000),
                                    stage: 0,
                                    ticks: 0,
                                    duration,
                                });
                                out.extend(self.five_class_mark_target(
                                    &target,
                                    DELAYED_EXPLOSION,
                                    true,
                                    now,
                                ));
                                out.extend(self.five_class_target_packets(
                                    &target,
                                    vec![effect(target.entity.object_id(), 15, 0)],
                                ));
                                out.extend(self.five_class_practice(session, action.spell, now));
                            }
                        }
                        Spell::VampireShot | Spell::PoisonShot | Spell::CrippleShot => {
                            // CompleteMagic applies marker/healing effects only
                            // after the real MAC projectile resolves.
                            if damage > 0 {
                                out.extend(self.five_class_special_arrow_effects(
                                    &action.owner,
                                    &target,
                                    action.spell,
                                    action.level,
                                    action.damage,
                                    now,
                                ));
                            }
                        }
                        _ => {}
                    }
                }
                FiveClassAction::Direction { origin, direction } => {
                    let points = if action.spell == Spell::HeavenlySword {
                        (1..=3)
                            .map(|distance| offset_point(&origin, direction, distance))
                            .collect::<Vec<_>>()
                    } else {
                        (-2..=2)
                            .map(|offset| {
                                offset_point(&origin, zone_rotated_direction(direction, offset), 1)
                            })
                            .collect()
                    };
                    let defence = if action.spell == Spell::HeavenlySword {
                        EntityDefence::MAC
                    } else {
                        EntityDefence::AC
                    };
                    let mut trained = false;
                    for point in points {
                        let targets = if action.spell == Spell::CrescentSlash {
                            self.five_class_targets_in_square(session, &point, 0, now)
                        } else {
                            self.five_class_first_target_in_cell(session, &point, now)
                                .into_iter()
                                .collect()
                        };
                        let has_objects =
                            self.objects.values().any(|object| object.position == point)
                                || self.players.values().any(|player| player.position == point);
                        for target in targets {
                            let (damage, hits) = self.five_class_hit(
                                &action.owner,
                                &target,
                                action.damage,
                                defence,
                                now,
                            );
                            trained |= damage > 0;
                            out.extend(hits);
                        }
                        if action.spell == Spell::CrescentSlash && has_objects {
                            out.extend(self.five_class_practice(session, action.spell, now));
                        }
                    }
                    if action.spell == Spell::HeavenlySword && trained {
                        out.extend(self.five_class_practice(session, action.spell, now));
                    }
                }
                FiveClassAction::Trap(target) => {
                    if self.five_class_target_hostile(session, &target, now) {
                        if let Some(player) = self.players.get(session).cloned() {
                            if self
                                .native_monsters
                                .get(&target.entity.object_id())
                                .is_some_and(|m| m.level < player.level.saturating_add(2))
                            {
                                out.extend(self.five_class_create_trap(session, target, now));
                            }
                        }
                    }
                }
            }
        }
        let explosions = std::mem::take(&mut self.five_class_skills.explosions);
        for mut explosion in explosions {
            if !self.owned_pet_owner_is_current(&explosion.owner)
                || !self.five_class_target_current(&explosion.target)
            {
                continue;
            }
            let current_poison = match &explosion.target.entity {
                ZoneCombatEntityRef::Player { session_id, .. } => self.players[session_id].poison,
                ZoneCombatEntityRef::Monster { object_id, .. } => {
                    native_monster_current_poison(&self.native_monsters[object_id])
                }
            };
            if current_poison & DELAYED_EXPLOSION == 0 {
                continue;
            }
            if now < explosion.next_tick_ms {
                self.five_class_skills.explosions.push(explosion);
                continue;
            }
            explosion.ticks = explosion.ticks.saturating_add(1);
            explosion.next_tick_ms = now.saturating_add(2000);
            if now > explosion.inflicted_at_ms {
                explosion.stage = explosion.stage.saturating_add(1);
            }
            if explosion.stage == 1 && now > explosion.inflicted_at_ms {
                explosion.inflicted_at_ms = explosion.next_tick_ms.saturating_add(3000);
            }
            out.extend(self.five_class_target_packets(
                &explosion.target,
                vec![effect(
                    explosion.target.entity.object_id(),
                    15,
                    u32::from(explosion.stage),
                )],
            ));
            if explosion.stage >= 2 {
                let center = self.five_class_target_position(&explosion.target).unwrap();
                let targets = self.five_class_targets_in_square(
                    &explosion.owner.online.session_id,
                    &center,
                    1,
                    now,
                );
                for target in targets {
                    out.extend(
                        self.five_class_hit(
                            &explosion.owner,
                            &target,
                            explosion.damage,
                            EntityDefence::MAC,
                            now,
                        )
                        .1,
                    );
                }
                out.extend(self.five_class_mark_target(
                    &explosion.target,
                    DELAYED_EXPLOSION,
                    false,
                    now,
                ));
            } else if explosion.ticks < explosion.duration {
                self.five_class_skills.explosions.push(explosion);
            } else {
                out.extend(self.five_class_mark_target(
                    &explosion.target,
                    DELAYED_EXPLOSION,
                    false,
                    now,
                ));
            }
        }
        out.extend(self.tick_five_class_bleeds(now));
        let vampires = std::mem::take(&mut self.five_class_skills.vampires);
        for mut vampire in vampires {
            if !self.owned_pet_owner_is_current(&vampire.owner) {
                continue;
            }
            if now > vampire.next_tick_ms {
                let amount = vampire.amount.min(10);
                vampire.amount -= amount;
                vampire.next_tick_ms = now.saturating_add(500);
                out.extend(
                    self.apply_native_player_heal(vampire.owner.online.session_id.clone(), amount),
                );
            }
            if vampire.amount > 0 {
                self.five_class_skills.vampires.push(vampire);
            }
        }
        let actors = self
            .five_class_skills
            .actors
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for session in actors {
            let Some(actor) = self.five_class_skills.actors.get(&session).cloned() else {
                continue;
            };
            if !self.owned_pet_owner_is_current(&actor.owner) {
                self.five_class_skills.actors.remove(&session);
                continue;
            }
            if let Some(spell) = actor.stealth_spell {
                let kind = if spell == Spell::MoonLight {
                    MOON_LIGHT
                } else {
                    DARK_BODY
                };
                let active = self.players[&session]
                    .buffs
                    .get(&kind)
                    .is_some_and(|buff| buff.expires_at_ms.is_none_or(|expiry| now < expiry));
                if !active {
                    out.extend(self.five_class_clear_stealth(&session));
                }
            }
            let expired_barrier = self.players[&session]
                .buffs
                .get(&ELEMENTAL_BARRIER)
                .is_some_and(|buff| buff.expires_at_ms.is_some_and(|expiry| now >= expiry));
            if expired_barrier {
                let player = self.players.get_mut(&session).unwrap();
                player.buffs.remove(&ELEMENTAL_BARRIER);
                let id = player.object_id;
                out.extend(self.five_class_present(
                    &session,
                    vec![
                        ServerPacket::RemoveBuff {
                            object_id: id,
                            buff_type: ELEMENTAL_BARRIER,
                        },
                        effect(id, 14, 0),
                    ],
                ));
            }
            let enabled = self.players[&session]
                .buffs
                .get(&CONCENTRATION)
                .is_some_and(|buff| buff.expires_at_ms.is_none_or(|expiry| now < expiry));
            if actor.concentration_enabled
                && (!enabled
                    || (actor.concentration_interrupted_until > 0
                        && now >= actor.concentration_interrupted_until))
            {
                if let Some(state) = self.five_class_skills.actors.get_mut(&session) {
                    state.concentration_enabled = enabled;
                    state.concentration_interrupted_until = 0;
                }
                let id = self.players[&session].object_id;
                out.extend(self.five_class_present(
                    &session,
                    vec![ServerPacket::SetConcentration {
                        object_id: id,
                        enabled,
                        interrupted: false,
                    }],
                ));
            }
        }
        out
    }

    fn five_class_target_packets(
        &mut self,
        target: &FiveClassTarget,
        packets: Vec<ServerPacket>,
    ) -> Vec<ZoneOutbound> {
        let Some(position) = self.five_class_target_position(target) else {
            return Vec::new();
        };
        let recipients = match &target.entity {
            ZoneCombatEntityRef::Player { object_id, .. } => {
                self.player_status_recipients(*object_id, &position)
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                self.native_monster_visible_recipients(*object_id, &position)
            }
        };
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets,
        }]
    }

    pub(super) fn five_class_hit(
        &mut self,
        owner: &OwnedPetOwner,
        target: &FiveClassTarget,
        raw: i32,
        defence: EntityDefence,
        now: u64,
    ) -> (i32, Vec<ZoneOutbound>) {
        let (settled, out) = self.five_class_hit_with_resolution(owner, target, raw, defence, now);
        (settled.applied_damage, out)
    }

    /// One actual Attacked call: Source returns its full final damage even if
    /// ChangeHP truncates the victim's remaining HP. Consumers such as Vampirism
    /// need that untruncated result, without rolling armour/critical twice.
    pub(super) fn five_class_hit_with_resolution(
        &mut self,
        owner: &OwnedPetOwner,
        target: &FiveClassTarget,
        raw: i32,
        defence: EntityDefence,
        now: u64,
    ) -> (five_class_damage::HitSettlement, Vec<ZoneOutbound>) {
        self.five_class_hit_inner(owner, target, raw, defence, now, 0)
    }

    fn five_class_miss(
        &mut self,
        target: &FiveClassTarget,
    ) -> (five_class_damage::HitSettlement, Vec<ZoneOutbound>) {
        (
            five_class_damage::HitSettlement::default(),
            self.five_class_target_packets(
                target,
                vec![ServerPacket::DamageIndicator {
                    damage: 0,
                    damage_type: 1,
                    object_id: target.entity.object_id(),
                }],
            ),
        )
    }

    /// Human/Monster.Attacked rolls critical on the current, typed impact.
    /// Reflect calls Attacked in the reverse direction with a newly admitted
    /// owner, rather than changing HP using the original caster's authority.
    fn five_class_hit_inner(
        &mut self,
        owner: &OwnedPetOwner,
        target: &FiveClassTarget,
        raw: i32,
        defence: EntityDefence,
        now: u64,
        reflect_depth: u8,
    ) -> (five_class_damage::HitSettlement, Vec<ZoneOutbound>) {
        let session = &owner.online.session_id;
        if !self.owned_pet_owner_is_current(owner)
            || !self.five_class_target_hostile(session, target, now)
        {
            return (five_class_damage::HitSettlement::default(), Vec::new());
        }
        let source = self.players[session].clone();
        self.five_class_skills.last_now_ms = self.five_class_skills.last_now_ms.max(now);
        let Some(actor) = self.five_class_actor_mut(session) else {
            return (five_class_damage::HitSettlement::default(), Vec::new());
        };
        let Some(sequence) = actor.impact_sequence.checked_add(1) else {
            return (five_class_damage::HitSettlement::default(), Vec::new());
        };
        actor.impact_sequence = sequence;
        let magical = five_class_damage::magical(defence);
        let uses_agility = matches!(
            defence,
            EntityDefence::ACAgility | EntityDefence::MACAgility | EntityDefence::Agility
        );
        match &target.entity {
            ZoneCombatEntityRef::Player {
                session_id: victim, ..
            } => {
                let player = self.players[victim].clone();
                // Source MapObject.GetArmour: MAC checks MagicResist; the
                // agility variants also check Accuracy. Human totals already
                // include mirrored buffs, so neither stat is added twice.
                if (magical
                    && crate::runtime::combat::crystal_accuracy_roll(
                        now,
                        source.object_id,
                        player.object_id,
                        10,
                    ) < player.combat_stats.magic_resist.clamp(0, 10) as u64)
                    || (uses_agility
                        && crate::runtime::combat::crystal_accuracy_roll(
                            now,
                            source.object_id,
                            player.object_id,
                            player.combat_stats.agility.max(0) as u64 + 1,
                        ) > source.combat_stats.accuracy.max(0) as u64)
                {
                    return self.five_class_miss(target);
                }
                let prepared =
                    five_class_damage::prepare_player(&source, &player, raw, defence, now);
                // Reflect precedes DamageReduction/block. Even an unlawful
                // reverse attack cancels the original hit, as Source does;
                // it must not produce an effect or inherit original authority.
                if five_class_damage::reflection_roll(&source, &player, now, sequence) {
                    if reflect_depth >= five_class_damage::MAX_REFLECT_CHAIN {
                        // Invalid 100% reflect on both Humans otherwise recurses
                        // forever in Source. Bound it without inventing damage.
                        return (five_class_damage::HitSettlement::default(), Vec::new());
                    }
                    if let Some(reverse_owner) = self.owned_pet_owner(victim) {
                        if let Some(reverse_target) = self
                            .five_class_target(source.object_id)
                            .filter(|t| self.five_class_target_hostile(victim, t, now))
                        {
                            let (_, mut out) = self.five_class_hit_inner(
                                &reverse_owner,
                                &reverse_target,
                                prepared.reflection_power,
                                defence,
                                now,
                                reflect_depth + 1,
                            );
                            out.extend(self.five_class_target_packets(
                                target,
                                vec![effect(
                                    player.object_id,
                                    five_class_damage::REFLECT_EFFECT,
                                    0,
                                )],
                            ));
                            return (five_class_damage::HitSettlement::default(), out);
                        }
                    }
                    return (five_class_damage::HitSettlement::default(), Vec::new());
                }
                if prepared.blocked() {
                    return self.five_class_miss(target);
                }
                let mut out = if player.hidden {
                    self.five_class_clear_stealth(victim)
                } else {
                    Vec::new()
                };
                out.extend(self.five_class_trigger_energy_shield(
                    victim,
                    source.object_id,
                    sequence,
                    now,
                ));
                let resolved =
                    five_class_damage::finish(prepared, &source, player.object_id, now, sequence);
                let shield_packets =
                    self.five_class_shorten_damage_shields(victim, resolved.damage, now);
                out.extend(self.five_class_target_packets(target, shield_packets));
                let critical_out = if resolved.critical {
                    self.five_class_target_packets(
                        target,
                        vec![effect(
                            player.object_id,
                            five_class_damage::CRITICAL_EFFECT,
                            0,
                        )],
                    )
                } else {
                    Vec::new()
                };
                // Use HP after EnergyShield. Comparing to its earlier HP would
                // under-report the actual damage and lose positive-hit receipts.
                let before_damage = self.players[victim].hp;
                let Some((packets, settlements)) = self
                    .apply_native_player_pvp_damage_prevalidated(
                        victim,
                        source.object_id,
                        resolved.damage,
                        if resolved.critical { 2 } else { 0 },
                        now,
                        true,
                    )
                else {
                    return (five_class_damage::HitSettlement::default(), out);
                };
                let applied = before_damage.saturating_sub(self.players[victim].hp).max(0);
                if applied > 0 {
                    out.extend(critical_out);
                }
                out.extend(settlements);
                out.push(ZoneOutbound::ToMany {
                    session_ids: self.player_status_recipients(player.object_id, &player.position),
                    packets,
                });
                out.extend(self.five_class_damage_committed(
                    source.object_id,
                    player.object_id,
                    applied,
                    !magical,
                    now,
                ));
                (
                    five_class_damage::HitSettlement {
                        resolved_damage: resolved.damage,
                        applied_damage: applied,
                    },
                    out,
                )
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let monster = self.native_monsters[object_id].clone();
                // Monster templates are base values; their own buff stats are
                // not a Human projection and need adding exactly once. Owned
                // and wild targets use the same single defence/critical pass.
                let agility = monster
                    .defense
                    .agility
                    .saturating_add(zone_native_monster_buff_stat_total(&monster, 11))
                    .max(0);
                let resist = zone_native_monster_buff_stat_total(&monster, 30).clamp(0, 10);
                if (magical
                    && crate::runtime::combat::crystal_accuracy_roll(
                        now,
                        source.object_id,
                        *object_id,
                        10,
                    ) < resist as u64)
                    || (uses_agility
                        && crate::runtime::combat::crystal_accuracy_roll(
                            now,
                            source.object_id,
                            *object_id,
                            agility as u64 + 1,
                        ) > source.combat_stats.accuracy.max(0) as u64)
                {
                    return self.five_class_miss(target);
                }
                let prepared = five_class_damage::prepare_monster(
                    &source, &monster, *object_id, raw, defence, now,
                );
                if prepared.blocked() {
                    return self.five_class_miss(target);
                }
                let resolved =
                    five_class_damage::finish(prepared, &source, *object_id, now, sequence);
                let critical_out = if resolved.critical {
                    self.five_class_target_packets(
                        target,
                        vec![effect(*object_id, five_class_damage::CRITICAL_EFFECT, 0)],
                    )
                } else {
                    Vec::new()
                };
                let hit = PendingNativeMonsterHit {
                    ready_at_ms: now,
                    session_id: session.clone(),
                    attacker_object_id: source.object_id,
                    object_id: *object_id,
                    damage: resolved.damage,
                    force_experience_owner: false,
                    fire_bounce: None,
                    soulfire_practice: None,
                    journey_event: None,
                };
                let mut out = if monster.owner_session_id.is_some() {
                    self.settle_five_class_owned_monster_hit(hit, now)
                } else {
                    self.resolve_pending_native_monster_hit(hit, now)
                };
                let applied = monster
                    .hp
                    .saturating_sub(self.native_monsters.get(object_id).map_or(0, |m| m.hp))
                    .max(0);
                if applied > 0 && resolved.critical {
                    for outbound in &mut out {
                        let packets = match outbound {
                            ZoneOutbound::ToSession { packets, .. }
                            | ZoneOutbound::ToMany { packets, .. } => packets,
                            _ => continue,
                        };
                        for packet in packets {
                            if let ServerPacket::DamageIndicator {
                                object_id: damaged_id,
                                damage_type,
                                ..
                            } = packet
                            {
                                if *damaged_id == *object_id {
                                    *damage_type = 2;
                                }
                            }
                        }
                    }
                    let mut critical_first = critical_out;
                    critical_first.append(&mut out);
                    out = critical_first;
                }
                (
                    five_class_damage::HitSettlement {
                        // No admitted Monster damage means no Source hit return:
                        // e.g. a current special target rejected the canonical HP
                        // entry after the one real defence/critical pass.
                        resolved_damage: if applied > 0 { resolved.damage } else { 0 },
                        applied_damage: applied,
                    },
                    out,
                )
            }
        }
    }

    fn five_class_apply_poison(
        &mut self,
        owner: &OwnedPetOwner,
        target: &FiveClassTarget,
        mask: u16,
        value: i32,
        ticks: u64,
        tick_speed_ms: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.owned_pet_owner_is_current(owner)
            || !self.five_class_target_hostile(&owner.online.session_id, target, now)
        {
            return Vec::new();
        }
        match &target.entity {
            ZoneCombatEntityRef::Player { .. } => self.apply_owned_human_poison(
                owner,
                &target.entity,
                mask,
                value,
                ticks,
                tick_speed_ms,
                true,
                now,
            ),
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                if self.native_monsters[object_id].owner_session_id.is_some() {
                    return self.apply_owned_human_poison(
                        owner,
                        &target.entity,
                        mask,
                        value,
                        ticks,
                        tick_speed_ms,
                        true,
                        now,
                    );
                }
                let monster = self.native_monsters.get_mut(object_id).unwrap();
                if mask != CRYSTAL_POISON_GREEN
                    || monster.ai == 142
                    || !hell_accepts_poison(monster)
                    || (monster.damage_poison & mask != 0
                        && monster.damage_poison_expires_at_ms > now
                        && monster.damage_poison_value > value)
                {
                    return Vec::new();
                }
                if self.five_class_skills.bleeds.iter().any(|poison| {
                    poison.target.entity == target.entity
                        && poison.mask == mask
                        && poison.value > value
                }) {
                    return Vec::new();
                }
                // This source can tick at 1000 or 2000ms. The old wild green
                // queue has one global 2000ms period, so retain a typed lease
                // here and clear its superseded scalar timer.
                clear_superseded_wild_green_timer(monster);
                self.five_class_skills
                    .bleeds
                    .retain(|poison| poison.target.entity != target.entity || poison.mask != mask);
                self.five_class_skills.bleeds.push(FiveClassBleed {
                    owner: owner.clone(),
                    target: target.clone(),
                    value,
                    ticks_left: ticks,
                    next_tick_ms: now,
                    mask,
                    tick_speed_ms,
                });
                self.five_class_mark_target(target, mask, true, now)
            }
        }
    }

    fn five_class_mark_target(
        &mut self,
        target: &FiveClassTarget,
        mask: u16,
        enabled: bool,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.five_class_target_current(target) {
            return Vec::new();
        }
        let poison = match &target.entity {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let player = self.players.get_mut(session_id).unwrap();
                if enabled {
                    zone_add_player_status_poison(
                        player,
                        mask,
                        u64::MAX.saturating_sub(now + 1),
                        now,
                    );
                } else {
                    player.poison &= !mask;
                    player.native_status_poison &= !mask;
                    player.native_status_poison_deadlines.remove(&mask);
                }
                player.poison
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let monster = self.native_monsters.get_mut(object_id).unwrap();
                if enabled {
                    monster.entity_poison |= mask;
                } else {
                    monster.entity_poison &= !mask;
                }
                native_monster_current_poison(monster)
            }
        };
        self.five_class_target_packets(
            target,
            vec![ServerPacket::ObjectPoisoned {
                object_id: target.entity.object_id(),
                poison,
            }],
        )
    }

    fn five_class_targets_in_square(
        &self,
        session: &SessionId,
        center: &Point,
        distance: i32,
        now: u64,
    ) -> Vec<FiveClassTarget> {
        self.players
            .values()
            .map(|p| p.object_id)
            .chain(self.native_monsters.keys().copied())
            .filter_map(|id| self.five_class_target(id))
            .filter(|target| {
                self.five_class_target_position(target)
                    .is_some_and(|position| zone_tile_distance(center, &position) <= distance)
                    && self.five_class_target_hostile(session, target, now)
            })
            .collect()
    }

    fn five_class_elemental_knockback(
        &mut self,
        session: &SessionId,
        target: &FiveClassTarget,
        level: u8,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(source) = self.players.get(session).cloned() else {
            return Vec::new();
        };
        let Some(position) = self.five_class_target_position(target) else {
            return Vec::new();
        };
        let target_level = match &target.entity {
            ZoneCombatEntityRef::Player { session_id, .. } => self.players[session_id].level,
            ZoneCombatEntityRef::Monster { object_id, .. } => self.native_monsters[object_id].level,
        };
        let elements = self
            .five_class_skills
            .actors
            .get(session)
            .map_or(0, |a| a.elements);
        if target_level >= source.level
            || zone_deterministic_roll(now, source.object_id as usize, 0xE17, 20)
                >= 6 + u64::from(level) * 3
                    + u64::from(elements)
                    + u64::from(source.level - target_level)
        {
            return Vec::new();
        }
        let Some(direction) = zone_direction_toward(&source.position, &position) else {
            return Vec::new();
        };
        let distance = 1
            + i32::from(level).saturating_sub(1).max(0)
            + zone_deterministic_roll(now, source.object_id as usize, 0xE18, 2) as i32;
        let mut packets = Vec::new();
        let mut current = position;
        for _ in 0..distance {
            let next = offset_point(&current, direction, 1);
            let can_move = match &target.entity {
                ZoneCombatEntityRef::Player { session_id, .. } => {
                    self.can_occupy(&next, Some(session_id))
                }
                ZoneCombatEntityRef::Monster { object_id, .. } => {
                    self.can_native_monster_occupy(*object_id, &next)
                }
            };
            if !can_move {
                break;
            }
            match &target.entity {
                ZoneCombatEntityRef::Player { session_id, .. } => {
                    self.occupancy.remove(&tile_key(&current));
                    self.occupancy.insert(tile_key(&next), session_id.clone());
                    self.players.get_mut(session_id).unwrap().position = next.clone();
                    self.player_grid.moved(session_id, &next);
                    self.ecs.move_player(session_id, &next);
                }
                ZoneCombatEntityRef::Monster { object_id, .. } => {
                    self.native_monsters.get_mut(object_id).unwrap().position = next.clone();
                }
            }
            current = next;
            packets.push(ServerPacket::ObjectPushed {
                object_id: target.entity.object_id(),
                location: current.clone(),
                direction: zone_rotated_direction(direction, 4),
            });
        }
        self.apply_zone_object_packets(&packets, now);
        let mut out = self.five_class_target_packets(target, packets);
        if let ZoneCombatEntityRef::Player { session_id, .. } = &target.entity {
            out.extend(self.five_class_player_moved(session_id, now));
            out.push(ZoneOutbound::ToSession {
                session_id: session_id.clone(),
                packets: vec![user_location_packet(&self.players[session_id])],
            });
            out.push(ZoneOutbound::SaveTransform {
                session_id: session_id.clone(),
                position: current,
                direction: self.players[session_id].direction,
            });
        }
        out.extend(self.diff_all_zone_object_visibility());
        out.extend(self.diff_visibility_for(session));
        out
    }

    /// Crystal HumanObject:3078,3109. Counters advance on a legal melee launch,
    /// even if its later hit misses. `damage_base` is the source's shared base,
    /// already updated by stealth and FatalSword, but not a MPEater/Hemorrhage
    /// or DoubleSlash final. None means retain the preceding normal final.
    pub(super) fn five_class_melee_launch(
        &mut self,
        session: &SessionId,
        target_id: u32,
        damage_base: i32,
        now: u64,
    ) -> (i32, Option<EntityDefence>, Vec<ZoneOutbound>) {
        self.five_class_skills.last_now_ms = self.five_class_skills.last_now_ms.max(now);
        let Some(owner) = self.owned_pet_owner(session) else {
            return (damage_base, None, Vec::new());
        };
        let Some(target) = self
            .five_class_target(target_id)
            .filter(|target| self.five_class_target_hostile(session, target, now))
        else {
            return (damage_base, None, Vec::new());
        };
        let source = self.players[session].clone();
        if source.class != MirClass::Assassin {
            return (damage_base, None, Vec::new());
        }
        let mut raw = damage_base;
        let mut defence = None;
        let mut out = Vec::new();
        if let Some(level) = source.combat_stats.mp_eater_level.filter(|_| {
            source.combat_stats.five_class_learned_mask
                & ZoneFiveClassPracticeSpell::MPEater.learned_bit()
                != 0
        }) {
            let base = 1 + source.combat_stats.accuracy.max(0) / 2;
            let width = i32::from(level) * 5;
            let increment = base
                + if width > 0 {
                    zone_deterministic_roll(now, source.object_id as usize, 0x4D50, width as u64)
                        as i32
                } else {
                    0
                };
            let armed = if let Some(actor) = self.five_class_actor_mut(session) {
                actor.mp_eater_count = actor.mp_eater_count.saturating_add(increment);
                let armed = actor.mp_eater_armed;
                if armed {
                    actor.mp_eater_armed = false;
                    actor.mp_eater_count = 0;
                } else if actor.mp_eater_count >= 100 {
                    actor.mp_eater_armed = true;
                }
                armed
            } else {
                false
            };
            if armed {
                if let Some(magic) = crystal_magic_by_spell("MPEater") {
                    raw = crate::runtime::combat::crystal_magic_damage_from_base(
                        &magic,
                        level,
                        damage_base,
                    );
                    defence = Some(EntityDefence::ACAgility);
                }
                let amount = 5 * (i32::from(level) + source.combat_stats.accuracy.max(0) / 4);
                if let ZoneCombatEntityRef::Player {
                    session_id: victim, ..
                } = &target.entity
                {
                    out.extend(self.five_class_change_mana(victim, -amount));
                }
                out.extend(self.five_class_change_mana(session, amount));
                out.extend(self.five_class_target_packets(
                    &target,
                    vec![effect(target_id, 16, source.object_id)],
                ));
                out.extend(self.five_class_practice(session, Spell::MPEater, now));
            }
        }
        if let Some(level) = source.combat_stats.hemorrhage_level.filter(|_| {
            source.combat_stats.five_class_learned_mask
                & ZoneFiveClassPracticeSpell::Hemorrhage.learned_bit()
                != 0
        }) {
            let width = i32::from(level) * 2;
            let increment = 1 + if width > 0 {
                zone_deterministic_roll(now, source.object_id as usize, 0x4845, width as u64) as i32
            } else {
                0
            };
            let armed = if let Some(actor) = self.five_class_actor_mut(session) {
                actor.hemorrhage_count = actor.hemorrhage_count.saturating_add(increment);
                let armed = actor.hemorrhage_armed;
                if armed {
                    actor.hemorrhage_armed = false;
                    actor.hemorrhage_count = 0;
                } else if actor.hemorrhage_count >= 55 {
                    actor.hemorrhage_armed = true;
                }
                armed
            } else {
                false
            };
            if armed {
                if let Some(magic) = crystal_magic_by_spell("Hemorrhage") {
                    raw = crate::runtime::combat::crystal_magic_damage_from_base(
                        &magic,
                        level,
                        damage_base,
                    );
                    defence = Some(EntityDefence::ACAgility);
                }
                let ticks = (i32::from(level) * 2 + source.combat_stats.luck / 6).max(1) as u64;
                let value = source.combat_stats.max_dc.saturating_add(1).max(1);
                out.extend(self.five_class_apply_bleeding(&owner, &target, value, ticks, now));
                out.extend(self.five_class_target_packets(&target, vec![effect(target_id, 17, 0)]));
                out.extend(self.five_class_practice(session, Spell::Hemorrhage, now));
            }
        }
        (raw, defence, out)
    }

    fn five_class_change_mana(&mut self, session: &SessionId, amount: i32) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get_mut(session) else {
            return Vec::new();
        };
        // Human.ChangeMP caps the requested operation before its zero guard.
        // A negative operation at MP0 still emits exact owner health, while
        // the economic settlement below exists only for a real pool delta.
        let before = player.mp;
        let max_mp = player.combat_stats.max_mp.max(0);
        let amount = if player.mp.saturating_add(amount) > max_mp {
            max_mp.saturating_sub(player.mp)
        } else {
            amount
        };
        if amount == 0 {
            return Vec::new();
        }
        player.change_owner_mp(amount);
        let settlement = (before != player.mp).then(|| player.mana_settlement(before));
        let packet = ServerPacket::ObjectMana {
            info: ObjectManaInfo {
                object_id: player.object_id,
                percent: zone_five_class_mana_percent(&player),
            },
        };
        let mut out = self.take_owner_health_through(session, u64::MAX);
        if let Some(settlement) = settlement {
            out.push(ZoneOutbound::PlayerManaChanged {
                session_id: session.clone(),
                settlement,
            });
        }
        out.extend(self.five_class_present(session, vec![packet]));
        out
    }

    /// GatherElement belongs to the attacking Human, not the victim, poison
    /// timer, or pet. Root calls this after a positive authoritative direct hit.
    pub(super) fn five_class_damage_committed(
        &mut self,
        attacker_id: u32,
        _target_id: u32,
        damage: i32,
        _is_melee: bool,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        self.five_class_skills.last_now_ms = self.five_class_skills.last_now_ms.max(now);
        if damage <= 0 {
            return Vec::new();
        }
        let Some((session, source)) = self
            .players
            .iter()
            .find(|(_, p)| p.object_id == attacker_id)
            .map(|(sid, p)| (sid.clone(), p.clone()))
        else {
            return Vec::new();
        };
        if source.class != MirClass::Archer {
            return Vec::new();
        }
        let Some(level) = source.combat_stats.meditation_level.filter(|_| {
            source.combat_stats.five_class_learned_mask
                & ZoneFiveClassPracticeSpell::Meditation.learned_bit()
                != 0
        }) else {
            return Vec::new();
        };
        let concentration =
            self.five_class_skills
                .actors
                .get(&session)
                .filter(|actor| {
                    self.owned_pet_owner_is_current(&actor.owner)
                        && actor.concentration_enabled
                        && now >= actor.concentration_interrupted_until
                        && source.buffs.get(&CONCENTRATION).is_some_and(|buff| {
                            buff.expires_at_ms.is_none_or(|expiry| now < expiry)
                        })
                })
                .map_or(0, |actor| i32::from(actor.concentration_level) + 1);
        let threshold = 8 - i32::from(level) - concentration;
        if (zone_deterministic_roll(now, attacker_id as usize, 0x6A74, 10) as i32) < threshold {
            return Vec::new();
        }
        let mut out = self.five_class_obtain_element(&session, false);
        out.extend(self.five_class_practice(&session, Spell::Meditation, now));
        out
    }

    fn five_class_apply_bleeding(
        &mut self,
        owner: &OwnedPetOwner,
        target: &FiveClassTarget,
        value: i32,
        ticks: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.owned_pet_owner_is_current(owner)
            || !self.five_class_target_hostile(&owner.online.session_id, target, now)
        {
            return Vec::new();
        }
        if let ZoneCombatEntityRef::Player { session_id, .. } = &target.entity {
            let caster = self.players[&owner.online.session_id].clone();
            let victim = self.players[session_id].clone();
            // Source ApplyPoison browns before duplicate/duration rejection,
            // independently of its later poison HP ticks and LastHitter.
            self.apply_owned_human_poison_application_brown(&caster, &victim, now);
        }
        if self.five_class_skills.bleeds.iter().any(|bleed| {
            bleed.target.entity == target.entity
                && bleed.mask == BLEEDING
                && bleed.ticks_left > ticks
        }) {
            return Vec::new();
        }
        self.five_class_skills
            .bleeds
            .retain(|bleed| bleed.target.entity != target.entity || bleed.mask != BLEEDING);
        self.five_class_skills.bleeds.push(FiveClassBleed {
            owner: owner.clone(),
            target: target.clone(),
            value,
            ticks_left: ticks,
            next_tick_ms: now,
            mask: BLEEDING,
            tick_speed_ms: 1000,
        });
        self.five_class_mark_target(target, BLEEDING, true, now)
    }

    fn five_class_poison_resisted(&self, target: &FiveClassTarget, now: u64) -> bool {
        // FlashDash's explicit source gate disables PvP stun when the source
        // default PvpCanResistPoison=false. Human ApplyPoison otherwise accepts
        // Human poison without a resist roll; Hemorrhage does not use this gate.
        if matches!(target.entity, ZoneCombatEntityRef::Player { .. }) {
            return false;
        }
        let resist = match &target.entity {
            ZoneCombatEntityRef::Player { session_id, .. } => self
                .players
                .get(session_id)
                .map_or(10, |p| p.combat_stats.poison_resist),
            ZoneCombatEntityRef::Monster { object_id, .. } => self
                .native_monsters
                .get(object_id)
                .map_or(0, |m| zone_native_monster_buff_stat_total(m, 24)),
        };
        zone_deterministic_roll(now, target.entity.object_id() as usize, 0xB1EE, 10) as i32
            >= resist
    }

    fn tick_five_class_bleeds(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let leases = std::mem::take(&mut self.five_class_skills.bleeds);
        let mut out = Vec::new();
        for mut lease in leases {
            if !self.five_class_target_current(&lease.target) {
                continue;
            }
            if !self.owned_pet_owner_is_current(&lease.owner)
                || !self.five_class_target_hostile(
                    &lease.owner.online.session_id,
                    &lease.target,
                    now,
                )
            {
                out.extend(self.five_class_mark_target(&lease.target, lease.mask, false, now));
                continue;
            }
            if lease.mask == CRYSTAL_POISON_GREEN {
                // Other native Human spells still publish wild green poison
                // into the scalar timer. Crystal keeps exactly one same-type
                // poison: equal/stronger replaces it; weaker cannot reset a
                // stronger poison's remaining time or ownership. Reconcile
                // before either timer can tick, so two professions cannot
                // cause duplicate green damage on one monster.
                let replaced = match &lease.target.entity {
                    ZoneCombatEntityRef::Monster { object_id, .. } => {
                        let monster = self.native_monsters.get_mut(object_id).unwrap();
                        if monster.damage_poison & CRYSTAL_POISON_GREEN == 0 {
                            false
                        } else if monster.damage_poison_expires_at_ms > now
                            && monster.damage_poison_value >= lease.value
                        {
                            true
                        } else {
                            clear_superseded_wild_green_timer(monster);
                            false
                        }
                    }
                    ZoneCombatEntityRef::Player { .. } => false,
                };
                if replaced {
                    // Clear this lease's entity marker; the replacement's
                    // scalar green mask remains visible until its own expiry.
                    out.extend(self.five_class_mark_target(&lease.target, lease.mask, false, now));
                    continue;
                }
            }
            let current_mask = match &lease.target.entity {
                ZoneCombatEntityRef::Player { session_id, .. } => self.players[session_id].poison,
                ZoneCombatEntityRef::Monster { object_id, .. } => {
                    native_monster_current_poison(&self.native_monsters[object_id])
                }
            };
            if current_mask & lease.mask == 0 {
                continue;
            } // Purification already removed it.
            if now < lease.next_tick_ms {
                self.five_class_skills.bleeds.push(lease);
                continue;
            }
            match &lease.target.entity {
                ZoneCombatEntityRef::Player {
                    session_id,
                    object_id,
                    ..
                } => {
                    // Green and Bleeding share ProcessPoison's actual Human
                    // LastHitter path. Rebuild it through current online/life
                    // authority before damage can issue a death receipt; do
                    // not turn a poison tick into a new brown-on-hit event.
                    if lease.value > 0
                        && self.players.get(session_id).is_some_and(|player| {
                            player.hp > 0 && !player.dead && !player.combat_stats.gm_never_die
                        })
                    {
                        self.record_owned_pet_player_hit_kind(
                            lease.owner.online.object_id,
                            session_id,
                            now,
                            false,
                        );
                    }
                    out.extend(self.resolve_native_player_damage_kind(
                        PendingNativePlayerHit {
                            ready_at_ms: now,
                            attacker_object_id: lease.owner.online.object_id,
                            attacker_ai: 0,
                            target_session_id: session_id.clone(),
                            target_object_id: *object_id,
                            damage: lease.value,
                            magic: false,
                        },
                        now,
                        true,
                        false,
                        true,
                        false,
                    ));
                }
                ZoneCombatEntityRef::Monster { object_id, .. } => {
                    // The typed launch owner/target are checked above, so this
                    // indirect settlement cannot inherit a new same-id owner.
                    if let Some(result) = self.apply_native_monster_damage_internal(
                        *object_id,
                        lease.value,
                        Some(&lease.owner.online.session_id),
                        now,
                        NativeMonsterDamageCause::Indirect,
                        Some(NativeExperienceActor::PeriodicPoison {
                            owner_object_id: lease.owner.online.object_id,
                        }),
                    ) {
                        out.extend(
                            self.entity_monster_damage_outcome(
                                lease.owner.online.object_id,
                                *object_id,
                                result,
                                None,
                                now,
                            )
                            .1,
                        );
                    }
                }
            }
            if lease.mask == BLEEDING {
                out.extend(self.five_class_target_packets(
                    &lease.target,
                    vec![effect(lease.target.entity.object_id(), 18, 0)],
                ));
            }
            lease.ticks_left = lease.ticks_left.saturating_sub(1);
            lease.next_tick_ms = now.saturating_add(lease.tick_speed_ms);
            if lease.ticks_left > 0 && self.five_class_target_current(&lease.target) {
                self.five_class_skills.bleeds.push(lease);
            } else {
                out.extend(self.five_class_mark_target(&lease.target, lease.mask, false, now));
            }
        }
        out
    }

    fn five_class_add_vampire_pool(
        &mut self,
        owner: &OwnedPetOwner,
        raw: i32,
        level: u8,
        now: u64,
    ) {
        let amount = raw.max(0).saturating_mul(i32::from(level) + 1) / 4;
        if amount <= 0 {
            return;
        }
        if let Some(pool) = self
            .five_class_skills
            .vampires
            .iter_mut()
            .find(|pool| pool.owner == *owner)
        {
            pool.amount = pool.amount.saturating_add(amount);
        } else {
            self.five_class_skills.vampires.push(FiveClassVampire {
                owner: owner.clone(),
                amount,
                next_tick_ms: now.saturating_add(1000),
            });
        }
    }

    fn five_class_special_arrow_effects(
        &mut self,
        owner: &OwnedPetOwner,
        target: &FiveClassTarget,
        spell: Spell,
        level: u8,
        raw: i32,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.owned_pet_owner_is_current(owner) {
            return Vec::new();
        }
        let session = &owner.online.session_id;
        let source = self.players[session].clone();
        let ticks = (raw.max(0) as u64) * 2 + (u64::from(level) + 1) * 7;
        let poison_value =
            raw.max(0) / 25 + i32::from(level) + 1 + five_class_poison_attack_bonus(&source, now);
        let mut out = Vec::new();
        match spell {
            Spell::VampireShot => {
                let packets = self.apply_native_player_arrow_marker_buff(
                    session,
                    CRYSTAL_ARROW_BUFF_VAMPIRE_SHOT,
                    level,
                    now,
                );
                out.extend(self.five_class_present(session, packets));
                self.five_class_add_vampire_pool(owner, raw, level, now);
            }
            Spell::PoisonShot => {
                let packets = self.apply_native_player_arrow_marker_buff(
                    session,
                    CRYSTAL_ARROW_BUFF_POISON_SHOT,
                    level,
                    now,
                );
                out.extend(self.five_class_present(session, packets));
                out.extend(self.five_class_apply_poison(
                    owner,
                    target,
                    CRYSTAL_POISON_GREEN,
                    poison_value,
                    ticks,
                    2000,
                    now,
                ));
            }
            Spell::CrippleShot => {
                let active = |kind| {
                    source
                        .buffs
                        .get(&kind)
                        .is_some_and(|buff| buff.expires_at_ms.is_none_or(|expiry| now < expiry))
                };
                let vampire = active(CRYSTAL_ARROW_BUFF_VAMPIRE_SHOT);
                let poison = active(CRYSTAL_ARROW_BUFF_POISON_SHOT);
                if !vampire && !poison {
                    return out;
                }
                let Some(center) = self.five_class_target_position(target) else {
                    return out;
                };
                let targets = self.five_class_targets_in_square(session, &center, 1, now);
                if targets.is_empty() {
                    return out;
                }
                let mut removed = Vec::new();
                for kind in [
                    CRYSTAL_ARROW_BUFF_VAMPIRE_SHOT,
                    CRYSTAL_ARROW_BUFF_POISON_SHOT,
                ] {
                    if self
                        .players
                        .get_mut(session)
                        .unwrap()
                        .buffs
                        .remove(&kind)
                        .is_some()
                    {
                        removed.push(ServerPacket::RemoveBuff {
                            object_id: source.object_id,
                            buff_type: kind,
                        });
                    }
                }
                out.extend(self.five_class_present(session, removed));
                for neighbour in targets {
                    if vampire {
                        // Original CompleteMagic deliberately attacks the
                        // primary target once per eligible 3x3 neighbour.
                        out.extend(
                            self.five_class_hit(owner, target, raw, EntityDefence::MAC, now)
                                .1,
                        );
                        self.five_class_add_vampire_pool(owner, raw, level, now);
                    }
                    if poison {
                        out.extend(self.five_class_apply_poison(
                            owner,
                            &neighbour,
                            CRYSTAL_POISON_GREEN,
                            poison_value,
                            ticks,
                            2000,
                            now,
                        ));
                    }
                }
            }
            _ => {}
        }
        out
    }

    fn five_class_direction_move(
        &mut self,
        session: &SessionId,
        spell: Spell,
        level: u8,
        raw: i32,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(source) = self.players.get(session).cloned() else {
            return Vec::new();
        };
        let Some(owner) = self.owned_pet_owner(session) else {
            return Vec::new();
        };
        let backward = spell == Spell::BackStep;
        let direction = if backward {
            zone_rotated_direction(source.direction, 4)
        } else {
            source.direction
        };
        let wanted = if backward {
            i32::from(level).max(1)
        } else if level > 1 {
            1
        } else {
            0
        };
        let mut destination = source.position.clone();
        let mut distance = 0;
        if !zone_player_status_blocks_movement(&source, now) {
            for _ in 0..wanted {
                let next = offset_point(&destination, direction, 1);
                if !self.can_occupy(&next, Some(session)) {
                    break;
                }
                destination = next;
                distance += 1;
            }
        }
        let mut out = Vec::new();
        if distance > 0 {
            self.occupancy.remove(&tile_key(&source.position));
            self.occupancy
                .insert(tile_key(&destination), session.clone());
            let player = self.players.get_mut(session).unwrap();
            player.position = destination.clone();
            player.movement_actions.clear();
            self.player_grid.moved(session, &destination);
            self.ecs.move_player(session, &destination);
            out.push(ZoneOutbound::SaveTransform {
                session_id: session.clone(),
                position: destination.clone(),
                direction: source.direction,
            });
            out.extend(self.five_class_player_moved(session, now));
            out.extend(self.diff_all_zone_object_visibility());
            out.extend(self.diff_visibility_for(session));
        }
        let mut owner_packets = vec![ServerPacket::MagicCast { spell }];
        let object_packet = if backward {
            if distance > 0 {
                owner_packets.push(ServerPacket::UserBackStep {
                    location: destination.clone(),
                    direction: source.direction,
                });
                out.extend(self.five_class_practice(session, spell, now));
            }
            ServerPacket::ObjectBackStep {
                movement: ObjectMovement {
                    object_id: source.object_id,
                    position: destination.clone(),
                    direction: source.direction,
                },
                distance,
            }
        } else if distance > 0 {
            owner_packets.push(ServerPacket::UserDashAttack {
                location: destination.clone(),
                direction: source.direction,
            });
            ServerPacket::ObjectDashAttack {
                object_id: source.object_id,
                location: destination.clone(),
                direction: source.direction,
                distance,
            }
        } else {
            ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id: source.object_id,
                    location: destination.clone(),
                    direction: source.direction,
                    spell: Spell::None as u8,
                    level: 0,
                    attack_type: 0,
                },
            }
        };
        let packet = object_packet;
        let player = self.players.get_mut(session).unwrap();
        player.movement_ready_at_ms = now.saturating_add(if backward { 500 } else { 300 });
        if !backward {
            let delay = source_attack_delay(&source).saturating_sub(120).max(300);
            player.next_attack_ready_at_ms = now.saturating_add(delay);
            player.next_spell_ready_at_ms = now.saturating_add(300);
            let front = offset_point(&destination, source.direction, 1);
            let targets = self.five_class_targets_in_square(session, &front, 0, now);
            if !targets.is_empty() {
                out.extend(self.five_class_practice(session, spell, now));
            }
            for target in targets {
                self.five_class_skills.pending.push(PendingFiveClassSkill {
                    owner: owner.clone(),
                    due_ms: now.saturating_add(delay),
                    spell,
                    level,
                    damage: raw,
                    action: FiveClassAction::Projectile(target),
                });
            }
        }
        owner_packets.push(user_location_packet(&self.players[session]));
        out.push(ZoneOutbound::ToSession {
            session_id: session.clone(),
            packets: owner_packets,
        });
        out.extend(self.five_class_present(session, vec![packet]));
        out
    }

    fn five_class_cat_tongue_control(
        &mut self,
        owner: &OwnedPetOwner,
        target: &FiveClassTarget,
        level: u8,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.owned_pet_owner_is_current(owner)
            || !self.five_class_target_hostile(&owner.online.session_id, target, now)
        {
            return Vec::new();
        }
        let Some(sequence) = self
            .five_class_skills
            .actors
            .get(&owner.online.session_id)
            .filter(|actor| actor.owner == *owner)
            .map(|actor| actor.impact_sequence)
        else {
            return Vec::new();
        };
        let mut roll = five_class_damage::source_chance_roll(
            now,
            owner.online.object_id,
            target.entity.object_id(),
            sequence,
            1_117 + u64::from(level),
            10,
        );
        // HumanObject.CompleteMagic:6398 subtracts four on a non-Human,
        // or on a Human with the source default PvpCanFreeze=false. Hence
        // outcomes eight/nine become Stun/Slow, never a wild/PvP freeze.
        if roll >= 8 {
            roll -= 4;
        }
        let duration = (u64::from(level) + 1) * 3;
        if roll > 4 {
            // The source Slow owns count-based expiry and actual speed
            // penalties. This entry is already prevalidated by the delayed
            // typed owner, target, hostility and positive AC settlement.
            // Human ApplyPoison does not resist a Human by source default.
            let Some(source) = self.native_entity_player_ref(&owner.online.session_id) else {
                return Vec::new();
            };
            return self.apply_prevalidated_entity_slow(
                &source,
                &target.entity,
                duration,
                1000,
                now,
                false,
            );
        }
        match &target.entity {
            ZoneCombatEntityRef::Player { session_id, .. } => self
                .apply_native_player_status_poison(
                    session_id,
                    CRYSTAL_POISON_STUN,
                    duration * 1000,
                    now,
                ),
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let monster = self.native_monsters.get_mut(object_id).unwrap();
                if now >= monster.control_until_ms {
                    monster.control_poison = 0;
                }
                monster.control_poison |= CRYSTAL_POISON_STUN;
                monster.control_until_ms = monster
                    .control_until_ms
                    .max(now.saturating_add(duration * 1000));
                let packet = ServerPacket::ObjectPoisoned {
                    object_id: *object_id,
                    poison: native_monster_current_poison(monster),
                };
                self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
                self.five_class_target_packets(target, vec![packet])
            }
        }
    }

    fn five_class_stun(
        &mut self,
        target: &FiveClassTarget,
        seconds: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.five_class_target_current(target) {
            return Vec::new();
        }
        match &target.entity {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                zone_add_player_status_poison(
                    self.players.get_mut(session_id).unwrap(),
                    16,
                    seconds.saturating_mul(1000),
                    now,
                );
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let monster = self.native_monsters.get_mut(object_id).unwrap();
                monster.control_poison |= 16;
                monster.control_until_ms = monster
                    .control_until_ms
                    .max(now.saturating_add(seconds.saturating_mul(1000)));
            }
        }
        let poison = match &target.entity {
            ZoneCombatEntityRef::Player { session_id, .. } => self.players[session_id].poison,
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                native_monster_current_poison(&self.native_monsters[object_id])
            }
        };
        self.five_class_target_packets(
            target,
            vec![
                ServerPacket::ObjectPoisoned {
                    object_id: target.entity.object_id(),
                    poison,
                },
                effect(target.entity.object_id(), 5, 0),
            ],
        )
    }

    fn five_class_dark_body(
        &mut self,
        owner: &OwnedPetOwner,
        target_id: u32,
        level: u8,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let session = &owner.online.session_id;
        if !self.owned_pet_owner_is_current(owner) {
            return Vec::new();
        }
        if let Some(id) = self
            .five_class_skills
            .clones
            .iter()
            .find(|(_, clone)| clone.owner == *owner)
            .map(|(id, _)| *id)
        {
            // Recasting dismisses the existing real clone via its explosion.
            return self.five_class_clone_die(id, now, true);
        }
        let Some(template) = crystal_monster_by_name("AssassinClone") else {
            return Vec::new();
        };
        let source = self.players[session].clone();
        let id = self.unique_object_id(0);
        let spawn = ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: id,
            name: template.name,
            name_colour_argb: -1,
            image: template.image,
            ai: 59,
            disposition: Some(crate::WorldEntityDisposition::Friendly),
            level: template.level,
            max_hp: 1500,
            hp: 1500,
            experience: 0,
            move_speed_ms: 100,
            attack_speed_ms: source_attack_delay(&source),
            friendly_guild: None,
            defense: ZoneMonsterDefense {
                agility: source.combat_stats.agility,
                min_ac: source.combat_stats.min_ac,
                max_ac: source.combat_stats.max_ac,
                min_mac: source.combat_stats.min_mac,
                max_mac: source.combat_stats.max_mac,
            },
            position: source.position.clone(),
            direction: source.direction,
            respawn: None,
            drops: Vec::new(),
        };
        let mut monster = ZoneNativeMonster::from_spawn(&spawn, id);
        monster.incarnation = self.allocate_monster_incarnation();
        monster.owner_session_id = Some(session.clone());
        monster.owner_player_object_id = source.object_id;
        monster.master_object_id = source.object_id;
        monster.summon_skill_level = level;
        monster.move_speed_ms = 100;
        monster.attack_speed_ms = source_attack_delay(&source);
        // Spawned() imposes its own two-second attack grace.
        monster.next_ai_ready_at_ms = now.saturating_add(500);
        monster.next_attack_ready_at_ms = now.saturating_add(2000);
        let incarnation = monster.incarnation;
        self.native_monsters.insert(id, monster);
        // HumanAssassin.GetInfo is ObjectPlayer, including the master's looks.
        // The real Node must exist before owned-life/target registration:
        // entity_ref_exists deliberately refuses a pet absent from objects.
        let mut appearance = source.clone();
        appearance.object_id = id;
        appearance.buffs.clear();
        appearance.hidden = false;
        let packets = object_player_packets(&appearance);
        self.apply_zone_object_packets(&packets, now);
        self.register_owned_pet_life(id);
        self.five_class_skills.clones.insert(
            id,
            FiveClassClone {
                owner: owner.clone(),
                incarnation,
                expires_at_ms: now.saturating_add(10_000),
                attack_power_spent: 0,
            },
        );
        if let Some(target) = self.five_class_target(target_id) {
            if let Some(position) = self.five_class_target_position(&target) {
                self.assigned_owned_pet_native_target(
                    id,
                    NativeSummonTarget {
                        object_id: target_id,
                        position,
                    },
                );
            }
        }
        let mut out = self.diff_all_zone_object_visibility();
        out.extend(self.five_class_self_buff(session, Spell::DarkBody, level, now));
        out
    }

    pub(super) fn try_tick_five_class_clone(
        &mut self,
        id: u32,
        now: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        let clone = self.five_class_skills.clones.get(&id)?.clone();
        let Some(monster) = self.native_monsters.get(&id).cloned() else {
            self.five_class_skills.clones.remove(&id);
            return Some(Vec::new());
        };
        if monster.incarnation != clone.incarnation
            || !self.owned_pet_owner_is_current(&clone.owner)
        {
            return Some(self.five_class_clone_die(id, now, false));
        }
        if monster.dead || monster.hp <= 0 || now > clone.expires_at_ms {
            return Some(self.five_class_clone_die(id, now, true));
        }
        if now < monster.next_ai_ready_at_ms {
            return Some(Vec::new());
        }
        let source = self.players[&clone.owner.online.session_id].clone();
        let selected = self.selected_owned_pet_target(id, now);
        if let Some(target) = selected.as_ref() {
            if zone_tile_distance(&monster.position, &target.position) <= 1
                && now >= monster.next_attack_ready_at_ms
                && self.owned_pet_can_launch_now(id)
            {
                if clone.attack_power_spent >= 500 {
                    return Some(self.five_class_clone_die(id, now, true));
                }
                let direction = zone_direction_toward(&monster.position, &target.position)
                    .unwrap_or(monster.direction);
                let raw = zone_roll_attack_power(
                    source.combat_stats.min_dc,
                    source.combat_stats.max_dc,
                    source.combat_stats.luck,
                    now,
                    id,
                    0xC10A,
                );
                let queued = self.queue_owned_pet_hit(
                    id,
                    &target.reference,
                    raw,
                    EntityDefence::ACAgility,
                    now.saturating_add(300),
                    now,
                );
                if queued || raw == 0 {
                    let monster = self.native_monsters.get_mut(&id).unwrap();
                    monster.direction = direction;
                    monster.next_ai_ready_at_ms = now.saturating_add(300);
                    monster.next_attack_ready_at_ms =
                        now.saturating_add(source_attack_delay(&source));
                    self.five_class_skills
                        .clones
                        .get_mut(&id)
                        .unwrap()
                        .attack_power_spent = clone.attack_power_spent.saturating_add(raw);
                    let packet = ServerPacket::ObjectAttack {
                        info: ObjectAttackInfo {
                            object_id: id,
                            location: monster.position.clone(),
                            direction,
                            spell: Spell::None as u8,
                            level: 0,
                            attack_type: 0,
                        },
                    };
                    let target = self.five_class_target(id).unwrap();
                    return Some(self.five_class_target_packets(&target, vec![packet]));
                }
            }
        }
        if !self.owned_pet_can_move_now(id) {
            return Some(Vec::new());
        }
        let goal = selected.map(|t| t.position).unwrap_or(source.position);
        // Source clone can run two cells, degrades to one when endpoint is
        // blocked. Validate intermediate collision as well for the shared grid.
        let toward = zone_direction_toward(&monster.position, &goal).unwrap_or(monster.direction);
        let mut directions = vec![toward];
        if goal == monster.position {
            directions.extend((1..=7).map(|offset| zone_rotated_direction(toward, offset)));
        }
        for direction in directions {
            let mid = offset_point(&monster.position, direction, 1);
            if !self.can_native_monster_occupy(id, &mid) {
                continue;
            }
            let far = offset_point(&monster.position, direction, 2);
            let run = self.can_native_monster_occupy(id, &far);
            let position = if run { far } else { mid };
            let m = self.native_monsters.get_mut(&id).unwrap();
            m.position = position.clone();
            m.direction = direction;
            m.next_ai_ready_at_ms = now.saturating_add(300);
            m.next_attack_ready_at_ms = m.next_attack_ready_at_ms.max(now.saturating_add(100));
            let movement = ObjectMovement {
                object_id: id,
                position,
                direction,
            };
            let packet = if run {
                ServerPacket::ObjectRun { movement }
            } else {
                ServerPacket::ObjectWalk { movement }
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            let target = self.five_class_target(id).unwrap();
            let mut out = self.diff_all_zone_object_visibility();
            out.extend(self.five_class_target_packets(&target, vec![packet]));
            return Some(out);
        }
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .next_ai_ready_at_ms = now.saturating_add(100);
        Some(Vec::new())
    }

    fn five_class_clone_die(&mut self, id: u32, now: u64, explode: bool) -> Vec<ZoneOutbound> {
        let Some(clone) = self.five_class_skills.clones.remove(&id) else {
            return Vec::new();
        };
        let Some(monster) = self.native_monsters.get(&id).cloned() else {
            return Vec::new();
        };
        if monster.incarnation != clone.incarnation {
            return Vec::new();
        }
        let mut out = Vec::new();
        if explode && self.owned_pet_owner_is_current(&clone.owner) {
            let source = self.players[&clone.owner.online.session_id].clone();
            let stats = source.combat_stats;
            let critical = if zone_deterministic_roll(now, id as usize, 0xC10E, 100)
                <= stats.accuracy.max(0) as u64
            {
                stats.max_dc.saturating_mul(2)
            } else {
                stats.min_dc.saturating_mul(2)
            };
            let raw = (stats.min_dc / 5 + 4 * (i32::from(monster.level) / 20))
                .saturating_mul(critical)
                .saturating_div(20)
                .saturating_add(stats.max_dc)
                .max(0);
            for distance in 1..=2 {
                for offset in 0..8 {
                    let position = offset_point(
                        &monster.position,
                        zone_rotated_direction(MirDirection::Up, offset),
                        distance,
                    );
                    let targets = self.five_class_targets_in_square(
                        &clone.owner.online.session_id,
                        &position,
                        0,
                        now,
                    );
                    for target in targets {
                        out.extend(
                            self.five_class_hit(&clone.owner, &target, raw, EntityDefence::AC, now)
                                .1,
                        );
                    }
                }
            }
        }
        let recipients = self.native_monster_visible_recipients(id, &monster.position);
        out.push(ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![
                ServerPacket::ObjectDied {
                    info: ObjectDiedInfo {
                        object_id: id,
                        location: monster.position,
                        direction: monster.direction,
                        kind: 2,
                    },
                },
                ServerPacket::ObjectRemove { object_id: id },
            ],
        });
        self.clear_owned_pet_life(id);
        self.native_monsters.remove(&id);
        self.objects.remove(&id);
        self.object_grid.remove(&id);
        self.removed_object_ids.insert(id);
        for player in self.players.values_mut() {
            player.visible_object_ids.remove(&id);
        }
        out.extend(self.diff_all_zone_object_visibility());
        out
    }

    pub(super) fn clear_five_class_life(&mut self, object_id: u32) {
        if let Some(player) = self.players.values_mut().find(|p| p.object_id == object_id) {
            player.element_orb_effect = 0;
            player.element_orb_level = 0;
            player.element_orb_max = 0;
        }
        self.five_class_skills
            .actors
            .retain(|_, actor| actor.owner.online.object_id != object_id);
        self.five_class_skills.pending.retain(|action| {
            action.owner.online.object_id != object_id
                && match &action.action {
                    FiveClassAction::Projectile(target) | FiveClassAction::Trap(target) => {
                        target.entity.object_id() != object_id
                    }
                    _ => true,
                }
        });
        let leases = std::mem::take(&mut self.five_class_skills.bleeds);
        for lease in leases {
            if lease.owner.online.object_id == object_id
                || lease.target.entity.object_id() == object_id
            {
                let cleared = self.five_class_mark_target(&lease.target, lease.mask, false, 0);
                self.five_class_skills.cleanup_outbound.extend(cleared);
            } else {
                self.five_class_skills.bleeds.push(lease);
            }
        }
        let explosions = std::mem::take(&mut self.five_class_skills.explosions);
        for lease in explosions {
            if lease.owner.online.object_id == object_id
                || lease.target.entity.object_id() == object_id
            {
                let cleared =
                    self.five_class_mark_target(&lease.target, DELAYED_EXPLOSION, false, 0);
                self.five_class_skills.cleanup_outbound.extend(cleared);
            } else {
                self.five_class_skills.explosions.push(lease);
            }
        }
        self.five_class_skills
            .vampires
            .retain(|vampire| vampire.owner.online.object_id != object_id);
        // Clone removal is handled by the real owned-pet lifecycle / next AI
        // dispatch so it can emit ObjectRemove, never by silently hiding it.
    }

    pub(crate) fn take_online_five_class_state(
        &mut self,
        session: &SessionId,
    ) -> Option<OnlineFiveClassTransfer> {
        let actor = self.five_class_skills.actors.remove(session)?;
        if !self.owned_pet_owner_is_current(&actor.owner) {
            return None;
        }
        let pools = std::mem::take(&mut self.five_class_skills.vampires);
        let mut vampires = Vec::new();
        for pool in pools {
            if pool.owner == actor.owner {
                vampires.push(pool);
            } else {
                self.five_class_skills.vampires.push(pool);
            }
        }
        let player = self.players.get(session)?;
        let mut buffs = BTreeMap::new();
        for kind in [
            HASTE,
            LIGHT_BODY,
            CRYSTAL_SWIFT_FEET_BUFF_TYPE,
            MOON_LIGHT,
            DARK_BODY,
            CONCENTRATION,
            ELEMENTAL_BARRIER,
        ] {
            if let Some(buff) = player.buffs.get(&kind) {
                buffs.insert(kind, buff.clone());
            }
        }
        Some(OnlineFiveClassTransfer {
            last_now_ms: self.five_class_skills.last_now_ms,
            actor,
            vampires,
            buffs,
        })
    }

    pub(crate) fn restore_online_five_class_state(
        &mut self,
        session: &SessionId,
        transfer: OnlineFiveClassTransfer,
    ) -> Vec<ZoneOutbound> {
        let now_ms = self.five_class_skills.last_now_ms.max(transfer.last_now_ms);
        let Some(owner) = self.owned_pet_owner(session) else {
            return Vec::new();
        };
        if owner != transfer.actor.owner {
            return Vec::new();
        } // same live identity and combat life only
        self.five_class_skills
            .actors
            .insert(session.clone(), transfer.actor);
        self.five_class_skills.last_now_ms = now_ms;
        self.five_class_skills.vampires.extend(transfer.vampires);
        let mut packets = Vec::new();
        let player = self.players.get_mut(session).unwrap();
        for (kind, mut buff) in transfer.buffs {
            if buff.expires_at_ms.is_some_and(|expiry| expiry <= now_ms) {
                continue;
            }
            if let Some(expiry) = buff.expires_at_ms {
                buff.buff.expire_time =
                    i64::try_from(expiry.saturating_sub(now_ms)).unwrap_or(i64::MAX);
            }
            if player
                .buffs
                .get(&kind)
                .is_some_and(|existing| existing.expires_at_ms > buff.expires_at_ms)
            {
                continue;
            }
            packets.push(ServerPacket::AddBuff {
                buff: buff.buff.clone(),
            });
            player.buffs.insert(kind, buff);
        }
        player.hidden = [MOON_LIGHT, DARK_BODY, CRYSTAL_HIDING_BUFF_TYPE, 114]
            .iter()
            .any(|kind| player.buffs.contains_key(kind));
        packets.push(ServerPacket::ObjectHidden {
            object_id: player.object_id,
            hidden: player.hidden,
        });
        let state = &mut self.five_class_skills.actors.get_mut(session).unwrap();
        if !player.buffs.contains_key(&MOON_LIGHT) && !player.buffs.contains_key(&DARK_BODY) {
            state.stealth_spell = None;
        }
        state.concentration_enabled = player.buffs.contains_key(&CONCENTRATION);
        if !state.concentration_enabled {
            state.concentration_interrupted_until = 0;
        }
        packets.push(ServerPacket::SetConcentration {
            object_id: player.object_id,
            enabled: state.concentration_enabled,
            interrupted: state.concentration_interrupted_until > now_ms,
        });
        let count = orb_count(state.elements) as u32;
        player.element_orb_effect = count;
        player.element_orb_level = state.elements;
        player.element_orb_max =
            if player.class == MirClass::Archer && player.combat_stats.meditation_level.is_some() {
                ORB_EXP[3]
            } else {
                0
            };
        packets.push(ServerPacket::SetElemental {
            object_id: player.object_id,
            enabled: elemental_available(state.elements),
            casted: false,
            value: state.elements,
            element_type: count,
            exp_last: ORB_EXP[3],
        });
        self.five_class_present(session, packets)
    }
}
