//! Five-class operations retain exact owner pool history across R24 routing.
//! Fixtures provide trusted learned projections and real online/life identity;
//! no display packet or forged settlement changes the authoritative pools.
use super::*;
use crate::{ZoneChatProfile, ZoneFiveClassPracticeSpell, ZoneOwnerHealthChange,
    ZoneOwnerHealthCursor, ZonePlayerCombatStats};

fn join(
    zone: &mut ZoneRuntime,
    name: &str,
    id: u32,
    class: MirClass,
    hp: i32,
    mp: i32,
    x: i32,
) -> SessionId {
    let session = SessionId::new(name);
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: session.clone(),
        account_id: name.into(),
        character_index: 1,
        object_id: id,
        name: name.into(),
        class,
        gender: mir2_protocol::MirGender::Male,
        level: 50,
        hp,
        max_hp: 500,
        mp,
        map_file_name: zone.key.map_file_name.clone(),
        position: Point { x, y: 20 },
        direction: MirDirection::Right,
        chat_profile: ZoneChatProfile { attack_mode: 5, ..Default::default() },
        combat_stats: ZonePlayerCombatStats {
            min_dc: 40, max_dc: 40, min_mc: 40, max_mc: 40,
            accuracy: 30, max_mp: 500, ..Default::default()
        },
    }));
    assert!(zone.owned_pet_owner(&session).is_some());
    session
}

fn fixture(class: MirClass, mp: i32) -> (ZoneRuntime, SessionId) {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("five-class-owner-health"), ZoneCollision::unbounded());
    let owner = join(&mut zone, "five-health-owner", 101, class, 300, mp, 20);
    (zone, owner)
}

fn changes(out: &[ZoneOutbound]) -> Vec<ZoneOwnerHealthChange> {
    out.iter().filter_map(|out| match out {
        ZoneOutbound::OwnerHealthChanged { change } => Some(change.clone()),
        _ => None,
    }).collect()
}

fn contains(out: &ZoneOutbound, expected: impl Fn(&ServerPacket) -> bool) -> bool {
    match out {
        ZoneOutbound::ToSession { packets, .. }
        | ZoneOutbound::ToMany { packets, .. }
        | ZoneOutbound::ToAll { packets } => packets.iter().any(expected),
        _ => false,
    }
}

fn assert_next_change(before: &ZoneOwnerHealthCursor, change: &ZoneOwnerHealthChange, step: u64) {
    assert_eq!(change.cursor.session_id, before.session_id);
    assert_eq!(change.cursor.online_owner, before.online_owner);
    assert_eq!(change.cursor.object_id, before.object_id);
    assert_eq!(change.cursor.life_generation, before.life_generation);
    assert_eq!(change.cursor.health_sequence, before.health_sequence + step);
}

fn cast(session: &SessionId, spell: Spell, target_id: u32, target: Point, now: u64)
    -> ZoneCommand {
    ZoneCommand::PlayerCastMagic {
        session_id: session.clone(), object_id: target_id, spell,
        direction: MirDirection::Right, target, cast: true, level: 0,
        damage: 0, mp_cost: 7, cooldown_ms: 1000, now_ms: now,
    }
}

#[test]
fn five_class_owner_health_specialized_cost_precedes_action_and_cooldown_retry() {
    for (class, identity) in [
        (MirClass::Assassin, ZoneFiveClassPracticeSpell::Haste),
        (MirClass::Archer, ZoneFiveClassPracticeSpell::Concentration),
    ] {
        let (mut zone, owner) = fixture(class, 100);
        zone.players.get_mut(&owner).unwrap().combat_stats.five_class_learned_mask =
            identity.learned_bit();
        let before = zone.owner_health_cursor(&owner).unwrap();
        let command = |now| cast(&owner, identity.spell(), 101, Point { x: 20, y: 20 }, now);
        let out = zone.handle(command(100));
        let events = changes(&out);
        assert_eq!(events.len(), 1, "{class:?}: {out:?}");
        assert_next_change(&before, &events[0], 1);
        assert!(!events[0].cursor.dead);
        assert_eq!((events[0].hp_before, events[0].hp), (300, 300));
        assert_eq!((events[0].mp_before, events[0].mp), (100, 93));
        let health = out.iter().position(|o|
            matches!(o, ZoneOutbound::OwnerHealthChanged { .. })).unwrap();
        let action = out.iter().position(|o| contains(o, |p|
            matches!(p, ServerPacket::Magic { spell, cast: true, .. }
                | ServerPacket::ObjectMagic { spell, cast: true, .. }
                if *spell == identity.spell()))).unwrap();
        assert!(health < action, "cost must precede the accepted action: {out:?}");
        let accepted_cursor = zone.owner_health_cursor(&owner).unwrap();
        let rejected = zone.handle(command(101));
        assert!(changes(&rejected).is_empty());
        assert!(!rejected.iter().any(|o| contains(o, |p|
            matches!(p, ServerPacket::Magic { cast: true, .. }
                | ServerPacket::ObjectMagic { cast: true, .. }))));
        assert_eq!(zone.owner_health_cursor(&owner).unwrap(), accepted_cursor);
        let retry = zone.handle(command(2100));
        let retry_events = changes(&retry);
        assert_eq!(retry_events.len(), 1, "expired cooldown must admit one retry: {retry:?}");
        assert_next_change(&before, &retry_events[0], 2);
        assert_eq!((retry_events[0].mp_before, retry_events[0].mp), (93, 86));
        assert!(zone.drain_owner_health_changes().is_empty());
    }
}

#[test]
fn five_class_owner_health_unlearned_or_unaffordable_cast_has_no_pool_operation() {
    for (class, identity) in [
        (MirClass::Assassin, ZoneFiveClassPracticeSpell::Haste),
        (MirClass::Archer, ZoneFiveClassPracticeSpell::Concentration),
    ] {
        for (learned, mp) in [(false, 100), (true, 6)] {
            let (mut zone, owner) = fixture(class, mp);
            if learned {
                zone.players.get_mut(&owner).unwrap().combat_stats.five_class_learned_mask =
                    identity.learned_bit();
            }
            let before = zone.owner_health_cursor(&owner).unwrap();
            let out = zone.handle(cast(&owner, identity.spell(), 101,
                Point { x: 20, y: 20 }, 100));
            assert!(changes(&out).is_empty(), "{class:?}, learned={learned}: {out:?}");
            assert!(!out.iter().any(|o| contains(o, |p|
                matches!(p, ServerPacket::Magic { cast: true, .. }
                    | ServerPacket::ObjectMagic { cast: true, .. }))));
            assert_eq!(zone.owner_health_cursor(&owner).unwrap(), before);
            assert_eq!(zone.player_vitals(&owner), Some((300, 500, mp)));
            assert!(zone.drain_owner_health_changes().is_empty());
        }
    }
}

#[test]
fn five_class_owner_health_pvp_arrow_charges_once_before_launch_not_on_impacts() {
    for spell in [Spell::StraightShot, Spell::DoubleShot] {
        let (mut zone, owner) = fixture(MirClass::Archer, 100);
        zone.players.get_mut(&owner).unwrap().combat_stats.five_class_learned_mask =
            ZoneFiveClassPracticeSpell::from_spell(spell).unwrap().learned_bit();
        join(&mut zone, "five-health-victim", 202, MirClass::Warrior, 500, 500, 22);
        zone.handle(ZoneCommand::sync_player_combat_state(owner.clone(),
            MirClass::Archer, true, false, true, false, false, false));
        let before = zone.owner_health_cursor(&owner).unwrap();
        let command = |now| cast(&owner, spell, 202, Point { x: 22, y: 20 }, now);
        let out = zone.handle(command(100));
        let events = changes(&out);
        assert_eq!(events.len(), 1, "{spell:?}: {out:?}");
        assert_next_change(&before, &events[0], 1);
        assert_eq!((events[0].hp, events[0].mp_before, events[0].mp), (300, 100, 93));
        let health = out.iter().position(|o|
            matches!(o, ZoneOutbound::OwnerHealthChanged { .. })).unwrap();
        let action = out.iter().position(|o| contains(o, |p|
            matches!(p, ServerPacket::Magic { spell: actual, cast: true, .. }
                | ServerPacket::ObjectMagic { spell: actual, cast: true, .. }
                if *actual == spell))).unwrap();
        assert!(health < action, "{out:?}");
        let count = if spell == Spell::DoubleShot { 2 } else { 1 };
        assert_eq!(zone.five_class_pvp.pending_count_for_test(), count);
        let accepted_cursor = zone.owner_health_cursor(&owner).unwrap();
        let rejected = zone.handle(command(101));
        assert!(changes(&rejected).is_empty());
        assert_eq!(zone.owner_health_cursor(&owner).unwrap(), accepted_cursor);
        assert_eq!(zone.five_class_pvp.pending_count_for_test(), count);
        let impacts = zone.tick_five_class_pvp(800);
        assert!(changes(&impacts).iter().all(|e| e.cursor.session_id != owner));
        assert_eq!(zone.owner_health_cursor(&owner).unwrap(), accepted_cursor);
        assert_eq!(zone.five_class_pvp.pending_count_for_test(), 0);
        assert!(zone.drain_owner_health_changes().is_empty());
    }
}

fn energy_fixture(hp: i32) -> (ZoneRuntime, SessionId, SessionId) {
    let (mut zone, owner) = fixture(MirClass::Assassin, 100);
    let victim = join(&mut zone, "five-energy-victim", 202, MirClass::Archer, hp, 20, 21);
    let stats = &mut zone.players.get_mut(&victim).unwrap().combat_stats;
    stats.energy_shield_percent = 100;
    stats.energy_shield_hp_gain = 10;
    (zone, owner, victim)
}

#[test]
fn five_class_owner_health_typed_energy_hit_retains_two_changes_when_net_hp_is_zero() {
    let (mut zone, owner, victim) = energy_fixture(300);
    let before = zone.owner_health_cursor(&victim).unwrap();
    let owner_proof = zone.owned_pet_owner(&owner).unwrap();
    let target_proof = zone.five_class_target(202).unwrap();
    let (damage, out) = zone.five_class_hit(&owner_proof, &target_proof,
        10, EntityDefence::None, 100);
    assert_eq!(damage, 10);
    let events = changes(&out);
    assert_eq!(events.len(), 2, "{out:?}");
    for (i, event) in events.iter().enumerate() {
        assert_next_change(&before, event, i as u64 + 1);
        assert!(!event.cursor.dead);
    }
    assert_eq!(events.iter().map(|e| (e.hp_before, e.hp, e.mp)).collect::<Vec<_>>(),
        vec![(300, 310, 20), (310, 300, 20)]);
    let heal_event = out.iter().position(|o| matches!(o,
        ZoneOutbound::OwnerHealthChanged { change } if change.hp == 310)).unwrap();
    let heal_receipt = out.iter().position(|o| matches!(o,
        ZoneOutbound::PlayerHealed { amount: 10, settlement: Some(s), .. }
        if s.hp_before == 300 && s.hp_after == 310 && !s.death_transition)).unwrap();
    let damage_event = out.iter().position(|o| matches!(o,
        ZoneOutbound::OwnerHealthChanged { change } if change.hp_before == 310)).unwrap();
    let damage_receipt = out.iter().position(|o| matches!(o,
        ZoneOutbound::PlayerDamaged { damage: 10, settlement: Some(s), .. }
        if s.hp_before == 310 && s.hp_after == 300 && !s.death_transition)).unwrap();
    assert!(heal_event < heal_receipt && heal_receipt < damage_event
        && damage_event < damage_receipt, "{out:?}");
    assert_eq!(zone.player_vitals(&victim), Some((300, 500, 20)));
    assert!(zone.drain_owner_health_changes().is_empty());
}

#[test]
fn five_class_owner_health_energy_then_lethal_hit_orders_death_before_zero_hp() {
    let (mut zone, owner, victim) = energy_fixture(3);
    let before = zone.owner_health_cursor(&victim).unwrap();
    let owner_proof = zone.owned_pet_owner(&owner).unwrap();
    let target_proof = zone.five_class_target(202).unwrap();
    let (damage, out) = zone.five_class_hit(&owner_proof, &target_proof,
        100, EntityDefence::None, 100);
    assert_eq!(damage, 13, "actual damage clips HP after the real 10 HP heal");
    let events = changes(&out);
    assert_eq!(events.len(), 2, "{out:?}");
    assert_next_change(&before, &events[0], 1);
    assert_next_change(&before, &events[1], 2);
    assert_eq!((events[0].hp_before, events[0].hp, events[0].cursor.dead), (3, 13, false));
    assert_eq!((events[1].hp_before, events[1].hp, events[1].cursor.dead), (13, 0, true));
    let heal = out.iter().position(|o| matches!(o,
        ZoneOutbound::OwnerHealthChanged { change } if change.hp == 13)).unwrap();
    let death = out.iter().position(|o| contains(o, |p|
        matches!(p, ServerPacket::Death { .. }))).unwrap();
    let zero = out.iter().position(|o| matches!(o,
        ZoneOutbound::OwnerHealthChanged { change } if change.hp == 0)).unwrap();
    let settlement = out.iter().position(|o| matches!(o,
        ZoneOutbound::PlayerDamaged { damage: 13, settlement: Some(s), .. }
        if s.hp_before == 13 && s.hp_after == 0 && s.death_transition)).unwrap();
    assert!(heal < death && death < zero && zero < settlement, "{out:?}");
    assert_eq!(out.iter().filter(|o| contains(o, |p|
        matches!(p, ServerPacket::Death { .. }))).count(), 1);
    assert_eq!(zone.owner_health_cursor(&victim).unwrap(), events[1].cursor);
    let repeat = zone.five_class_hit(&owner_proof, &target_proof,
        100, EntityDefence::None, 101).1;
    assert!(changes(&repeat).is_empty(), "retired target life cannot notify again");
    assert!(zone.drain_owner_health_changes().is_empty());
}

#[test]
fn five_class_owner_health_mp_eater_zero_operation_has_no_fake_mana_settlement() {
    for initial_mp in [0, 490, 500] {
        let (mut zone, owner) = fixture(MirClass::Assassin, initial_mp);
        let victim = join(&mut zone, "five-mp-victim", 202, MirClass::Warrior, 500, 0, 21);
        let stats = &mut zone.players.get_mut(&owner).unwrap().combat_stats;
        stats.accuracy = 200;
        stats.mp_eater_level = Some(0);
        stats.five_class_learned_mask = ZoneFiveClassPracticeSpell::MPEater.learned_bit();
        zone.handle(ZoneCommand::sync_player_combat_state(owner.clone(),
            MirClass::Assassin, true, false, true, false, false, false));
        let before_owner = zone.owner_health_cursor(&owner).unwrap();
        let before_victim = zone.owner_health_cursor(&victim).unwrap();
        let owner_mana_sequence = zone.players[&owner].mana_receipt_sequence;
        let victim_mana_sequence = zone.players[&victim].mana_receipt_sequence;
        let attack = |now| ZoneCommand::PlayerAttackObject {
            session_id: owner.clone(), object_id: 202, direction: MirDirection::Right,
            spell: Spell::None as u8, level: 0, attack_type: 0, damage: 40, now_ms: now,
        };
        let arm = zone.handle(attack(100));
        assert!(arm.iter().any(|o| contains(o, |p|
            matches!(p, ServerPacket::ObjectAttack { .. }))), "{arm:?}");
        assert!(changes(&arm).is_empty());
        let out = zone.handle(attack(2100));
        assert!(out.iter().any(|o| contains(o, |p|
            matches!(p, ServerPacket::ObjectAttack { .. }))), "{out:?}");
        let events = changes(&out);
        assert_eq!(events.len(), 1 + usize::from(initial_mp < 500));
        let victim_event = events.iter().find(|e| e.cursor.session_id == victim).unwrap();
        assert_next_change(&before_victim, victim_event, 1);
        assert_eq!((victim_event.hp_before, victim_event.hp,
            victim_event.mp_before, victim_event.mp), (500, 500, 0, 0));
        assert!(!out.iter().any(|o| matches!(o,
            ZoneOutbound::PlayerManaChanged { session_id, .. } if *session_id == victim)));
        assert_eq!(zone.players[&victim].mana_receipt_sequence, victim_mana_sequence,
            "MP0 negative notification must not consume an economic receipt sequence");
        let expected_mp = (initial_mp + 250).min(500);
        let source_events = events.iter().filter(|e| e.cursor.session_id == owner)
            .collect::<Vec<_>>();
        let settlements = out.iter().filter_map(|o| match o {
            ZoneOutbound::PlayerManaChanged { session_id, settlement }
                if *session_id == owner => Some(*settlement),
            _ => None,
        }).collect::<Vec<_>>();
        if initial_mp < 500 {
            assert_eq!(source_events.len(), 1);
            assert_next_change(&before_owner, source_events[0], 1);
            assert_eq!((source_events[0].mp_before, source_events[0].mp),
                (initial_mp, expected_mp));
            assert_eq!(settlements.len(), 1);
            assert_eq!((settlements[0].mp_before, settlements[0].mp_after),
                (initial_mp, expected_mp));
            assert_eq!(settlements[0].object_id, before_owner.object_id);
            assert_eq!(settlements[0].life_generation, before_owner.life_generation);
            assert_eq!(settlements[0].receipt_sequence, owner_mana_sequence + 1);
        } else {
            assert!(source_events.is_empty() && settlements.is_empty());
            assert_eq!(zone.owner_health_cursor(&owner).unwrap(), before_owner);
            assert_eq!(zone.players[&owner].mana_receipt_sequence, owner_mana_sequence);
        }
        assert_eq!(zone.player_vitals(&owner).unwrap().2, expected_mp);
        assert_eq!(out.iter().filter(|o| matches!(o,
            ZoneOutbound::FiveClassMagicPractice { receipt }
                if receipt.spell == ZoneFiveClassPracticeSpell::MPEater)).count(), 1);
        let after_owner = zone.owner_health_cursor(&owner).unwrap();
        let after_victim = zone.owner_health_cursor(&victim).unwrap();
        let duplicate = zone.handle(attack(2101));
        assert!(changes(&duplicate).is_empty());
        assert!(!duplicate.iter().any(|o| matches!(o,
            ZoneOutbound::FiveClassMagicPractice { .. } | ZoneOutbound::PlayerManaChanged { .. })));
        assert_eq!(zone.owner_health_cursor(&owner).unwrap(), after_owner);
        assert_eq!(zone.owner_health_cursor(&victim).unwrap(), after_victim);
        assert!(zone.drain_owner_health_changes().is_empty());
    }
}
