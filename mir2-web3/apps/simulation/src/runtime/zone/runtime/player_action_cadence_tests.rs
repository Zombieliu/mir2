//! Crystal HumanObject action/cast admission, independent of personal ticks.
use super::*;
use mir2_protocol::{MirClass, MirGender};

const OWNER: u32 = 10_001;
const TARGET: u32 = 20_001;

fn fixture(class: MirClass) -> (ZoneRuntime, SessionId) {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    let owner = SessionId::new("cadence-owner");
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: owner.clone(),
        account_id: "cadence-account".into(),
        character_index: 1,
        object_id: OWNER,
        name: "Cadence Owner".into(),
        class,
        gender: MirGender::Male,
        level: 30,
        hp: 100,
        max_hp: 100,
        mp: 200,
        map_file_name: "0".into(),
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    zone.handle(ZoneCommand::sync_player_combat_state(
        owner.clone(),
        class,
        true,
        false,
        true,
        false,
        false,
        false,
    ));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner.clone(),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: TARGET,
            name: "Scarecrow".into(),
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            disposition: Some(crate::WorldEntityDisposition::Hostile),
            level: 1,
            max_hp: 10_000,
            hp: 10_000,
            experience: 1,
            move_speed_ms: 60_000,
            attack_speed_ms: 60_000,
            friendly_guild: None,
            position: Point { x: 11, y: 10 },
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
    });
    (zone, owner)
}

fn magic(owner: &SessionId, spell: Spell, delay: u64, now_ms: u64) -> ZoneCommand {
    let self_spell = zone_magic_targets_self(spell);
    ZoneCommand::PlayerCastMagic {
        session_id: owner.clone(),
        object_id: if self_spell { OWNER } else { TARGET },
        spell,
        direction: MirDirection::Right,
        target: if self_spell {
            Point { x: 10, y: 10 }
        } else {
            Point { x: 11, y: 10 }
        },
        cast: true,
        level: 0,
        damage: 8,
        mp_cost: 3,
        cooldown_ms: delay,
        now_ms,
    }
}

fn accepted(out: &[ZoneOutbound], spell: Spell) -> bool {
    out.iter().any(|item| {
        matches!(item,
        ZoneOutbound::ToSession { packets, .. }
            if packets.iter().any(|packet| matches!(packet,
                ServerPacket::Magic { spell: actual, cast: true, .. } if *actual == spell)
                || matches!(packet, ServerPacket::MagicCast { spell: Spell::ShoulderDash }
                    if spell == Spell::ShoulderDash)))
    })
}

fn assert_rejected(zone: &mut ZoneRuntime, owner: &SessionId, command: ZoneCommand) {
    let before = zone.players[owner].clone();
    let hits = zone.pending_native_hits.len();
    let out = zone.handle(command);
    assert!(
        !out.iter().any(|item| match item {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.iter().any(|packet| matches!(
                packet,
                ServerPacket::Magic { cast: true, .. }
                    | ServerPacket::ObjectMagic { cast: true, .. }
                    | ServerPacket::ObjectProjectile { .. }
                    | ServerPacket::MagicCast { .. }
                    | ServerPacket::ObjectAttack { .. }
            )),
            _ => false,
        }),
        "rejected requests must not animate or launch another hit: {out:?}"
    );
    assert_eq!(zone.players[owner].mp, before.mp);
    assert_eq!(zone.players[owner].position, before.position);
    assert_eq!(
        zone.players[owner].magic_ready_at_ms,
        before.magic_ready_at_ms
    );
    assert_eq!(
        zone.players[owner].next_spell_ready_at_ms,
        before.next_spell_ready_at_ms
    );
    assert_eq!(zone.pending_native_hits.len(), hits);
}

fn melee(owner: &SessionId, now_ms: u64) -> ZoneCommand {
    ZoneCommand::PlayerAttackObject {
        session_id: owner.clone(),
        object_id: TARGET,
        direction: MirDirection::Right,
        spell: Spell::None as u8,
        level: 0,
        attack_type: 0,
        damage: 8,
        now_ms,
    }
}

#[test]
fn wizard_cannot_alternate_spells_to_bypass_crystal_global_delay() {
    let (mut zone, owner) = fixture(MirClass::Wizard);
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::FireBall, 1_800, 0)),
        Spell::FireBall
    ));
    assert_rejected(
        &mut zone,
        &owner,
        magic(&owner, Spell::GreatFireBall, 1_800, 300),
    );
    assert_rejected(
        &mut zone,
        &owner,
        magic(&owner, Spell::GreatFireBall, 1_800, 1_799),
    );
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::GreatFireBall, 1_799),
        Some(1)
    );
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::GreatFireBall, 1_800, 1_800)),
        Spell::GreatFireBall
    ));
}

#[test]
fn taoist_uses_the_later_of_global_delay_and_own_skill_delay() {
    let (mut zone, owner) = fixture(MirClass::Taoist);
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::Healing, 6_000, 0)),
        Spell::Healing
    ));
    assert_rejected(
        &mut zone,
        &owner,
        magic(&owner, Spell::SoulFireBall, 1_800, 1_799),
    );
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::SoulFireBall, 1_800, 1_800)),
        Spell::SoulFireBall
    ));
    assert_rejected(
        &mut zone,
        &owner,
        magic(&owner, Spell::Healing, 6_000, 5_999),
    );
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::Healing, 5_999),
        Some(1)
    );
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::Healing, 6_000, 6_000)),
        Spell::Healing
    ));
}

#[test]
fn flame_field_retains_its_longer_crystal_global_delay() {
    let (mut zone, owner) = fixture(MirClass::Wizard);
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::FlameField, 1_800, 0)),
        Spell::FlameField
    ));
    assert_rejected(
        &mut zone,
        &owner,
        magic(&owner, Spell::FireBall, 1_800, 1_800),
    );
    assert_rejected(
        &mut zone,
        &owner,
        magic(&owner, Spell::FireBall, 1_800, 2_499),
    );
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::FireBall, 1_800, 2_500)),
        Spell::FireBall
    ));
}

#[test]
fn accepted_cast_blocks_melee_until_the_600ms_action_boundary() {
    let (mut zone, owner) = fixture(MirClass::Wizard);
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::FireBall, 1_800, 0)),
        Spell::FireBall
    ));
    assert_rejected(&mut zone, &owner, melee(&owner, 599));
    let out = zone.handle(melee(&owner, 600));
    assert!(zone_outbounds_contain_accepted_player_action(
        &out, &owner, OWNER
    ));
}

#[test]
fn melee_blocks_cast_until_550ms_without_extending_the_swing_cooldown() {
    let (mut zone, owner) = fixture(MirClass::Taoist);
    assert!(zone_outbounds_contain_accepted_player_action(
        &zone.handle(melee(&owner, 0)),
        &owner,
        OWNER
    ));
    assert_rejected(&mut zone, &owner, magic(&owner, Spell::Healing, 6_000, 549));
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::Healing, 549),
        Some(1)
    );
    assert_eq!(zone.players[&owner].next_attack_ready_at_ms, 600);
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::Healing, 6_000, 550)),
        Spell::Healing
    ));
}

#[test]
fn movement_blocks_cast_until_600ms_and_the_public_getter_agrees() {
    let (mut zone, owner) = fixture(MirClass::Wizard);
    zone.handle(ZoneCommand::Walk {
        session_id: owner.clone(),
        direction: MirDirection::Down,
        seq: 1,
        now_ms: 0,
    });
    zone.tick(0);
    assert_eq!(zone.players[&owner].position, Point { x: 10, y: 11 });
    assert_rejected(
        &mut zone,
        &owner,
        magic(&owner, Spell::FireBall, 1_800, 599),
    );
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::FireBall, 599),
        Some(1)
    );
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::FireBall, 1_800, 600)),
        Spell::FireBall
    ));
}

#[test]
fn new_movement_after_cast_waits_for_action_lock_then_runs_once() {
    let (mut zone, owner) = fixture(MirClass::Wizard);
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::FireBall, 1_800, 0)),
        Spell::FireBall
    ));
    zone.handle(ZoneCommand::Walk {
        session_id: owner.clone(),
        direction: MirDirection::Down,
        seq: 1,
        now_ms: 1,
    });
    zone.tick(599);
    assert_eq!(zone.players[&owner].position, Point { x: 10, y: 10 });
    zone.tick(600);
    assert_eq!(zone.players[&owner].position, Point { x: 10, y: 11 });
    zone.tick(601);
    assert_eq!(zone.players[&owner].position, Point { x: 10, y: 11 });
}

#[test]
fn warrior_shoulder_dash_cannot_repeat_or_move_before_its_action_lock() {
    let (mut zone, owner) = fixture(MirClass::Warrior);
    let out = zone.handle(magic(&owner, Spell::ShoulderDash, 6_000, 0));
    assert!(accepted(&out, Spell::ShoulderDash));
    assert!(
        !out.iter().any(|item| match item {
            ZoneOutbound::ToSession { packets, .. } | ZoneOutbound::ToMany { packets, .. } =>
                packets.iter().any(|p| matches!(
                    p,
                    ServerPacket::Magic {
                        spell: Spell::ShoulderDash,
                        ..
                    } | ServerPacket::ObjectMagic {
                        spell: Spell::ShoulderDash,
                        ..
                    }
                )),
            _ => false,
        }),
        "ShoulderDash uses its dash presentation, not another spell animation"
    );
    assert!(
        !out.iter().any(|item| matches!(item,
        ZoneOutbound::ToMany { packets, .. } | ZoneOutbound::ToAll { packets }
            if packets.iter().any(|p| matches!(p, ServerPacket::MagicCast { .. })))),
        "a nearby player must not receive another owner's unscoped cooldown ack"
    );
    assert!(out.iter().any(|item| match item {
        ZoneOutbound::ToSession { packets, .. } | ZoneOutbound::ToMany { packets, .. } =>
            packets.iter().any(|p| matches!(
                p,
                ServerPacket::MagicCast {
                    spell: Spell::ShoulderDash
                }
            )),
        _ => false,
    }));
    let after = zone.players[&owner].position.clone();
    zone.handle(ZoneCommand::Walk {
        session_id: owner.clone(),
        direction: MirDirection::Down,
        seq: 1,
        now_ms: 1,
    });
    zone.tick(599);
    assert_eq!(zone.players[&owner].position, after);
    zone.tick(600);
    assert_eq!(
        zone.players[&owner].position,
        Point {
            x: after.x,
            y: after.y + 1
        }
    );
    assert_rejected(
        &mut zone,
        &owner,
        magic(&owner, Spell::ShoulderDash, 6_000, 5_999),
    );
    assert!(accepted(
        &zone.handle(magic(&owner, Spell::ShoulderDash, 6_000, 6_000)),
        Spell::ShoulderDash
    ));
}

fn flame_prepare(owner: &SessionId, now_ms: u64) -> ZoneCommand {
    ZoneCommand::PreparePlayerFlamingSword {
        session_id: owner.clone(),
        level: 0,
        now_ms,
    }
}

fn flame_ack(out: &[ZoneOutbound], owner: &SessionId, can_use: bool) -> usize {
    out.iter()
        .filter_map(|item| match item {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if session_id == owner => Some(packets),
            _ => None,
        })
        .flatten()
        .filter(|packet| {
            matches!(packet,
        ServerPacket::SpellToggle { spell: Spell::FlamingSword, can_use: actual, .. }
            if *actual == can_use)
        })
        .count()
}

fn rejoin_data(zone: &ZoneRuntime, owner: &SessionId, map: &str) -> ZoneJoin {
    let p = &zone.players[owner];
    ZoneJoin {
        session_id: owner.clone(),
        account_id: p.account_id.clone(),
        character_index: p.character_index,
        object_id: p.object_id,
        name: p.name.clone(),
        class: p.class,
        gender: p.gender,
        level: p.level,
        hp: p.hp,
        max_hp: p.max_hp,
        mp: p.mp,
        map_file_name: map.into(),
        position: p.position.clone(),
        direction: p.direction,
        chat_profile: p.chat_profile.clone(),
        combat_stats: p.combat_stats.clone(),
    }
}

#[test]
fn flaming_sword_consumption_retains_exact_deadline_and_charges_once() {
    let (mut zone, owner) = fixture(MirClass::Warrior);
    let cost = i32::from(crystal_magic_by_spell("FlamingSword").unwrap().base_cost);
    let prepared = zone.handle(flame_prepare(&owner, 100));
    assert_eq!(flame_ack(&prepared, &owner, true), 1);
    assert_eq!(zone.players[&owner].mp, 200 - cost);
    assert_eq!(zone.players[&owner].next_spell_ready_at_ms, 0);
    assert_eq!(zone.players[&owner].movement_ready_at_ms, 0);
    let hit = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: owner.clone(),
        object_id: TARGET,
        direction: MirDirection::Right,
        spell: Spell::FlamingSword as u8,
        level: 0,
        attack_type: 0,
        damage: 8,
        now_ms: 101,
    });
    assert_eq!(flame_ack(&hit, &owner, false), 1);
    assert!(!zone.players[&owner].flaming_sword_armed);
    assert_eq!(zone.players[&owner].flaming_sword_ready_at_ms, 10_100);
    let early = zone.handle(flame_prepare(&owner, 10_099));
    assert_eq!(flame_ack(&early, &owner, true), 0);
    assert_eq!(zone.players[&owner].mp, 200 - cost);
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::FlamingSword, 10_099),
        Some(1)
    );
    let ready = zone.handle(flame_prepare(&owner, 10_100));
    assert_eq!(flame_ack(&ready, &owner, true), 1);
    assert_eq!(zone.players[&owner].mp, 200 - 2 * cost);
}

#[test]
fn flaming_sword_mana_death_and_rejected_melee_do_not_consume_charge() {
    let (mut zone, owner) = fixture(MirClass::Warrior);
    let cost = i32::from(crystal_magic_by_spell("FlamingSword").unwrap().base_cost);
    zone.players.get_mut(&owner).unwrap().mp = cost;
    assert_eq!(
        flame_ack(&zone.handle(flame_prepare(&owner, 0)), &owner, true),
        0
    );
    assert_eq!(zone.players[&owner].mp, cost);
    zone.players.get_mut(&owner).unwrap().mp = cost + 1;
    assert_eq!(
        flame_ack(&zone.handle(flame_prepare(&owner, 100)), &owner, true),
        1
    );
    assert_eq!(zone.players[&owner].mp, 1);
    zone.players.get_mut(&owner).unwrap().movement_ready_at_ms = 600;
    let early = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: owner.clone(),
        object_id: TARGET,
        direction: MirDirection::Right,
        spell: Spell::FlamingSword as u8,
        level: 0,
        attack_type: 0,
        damage: 8,
        now_ms: 599,
    });
    assert_eq!(flame_ack(&early, &owner, false), 0);
    assert!(zone.players[&owner].flaming_sword_armed);
    let missing = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: owner.clone(),
        object_id: TARGET + 99,
        direction: MirDirection::Right,
        spell: Spell::FlamingSword as u8,
        level: 0,
        attack_type: 0,
        damage: 8,
        now_ms: 600,
    });
    assert_eq!(flame_ack(&missing, &owner, false), 0);
    assert!(zone.players[&owner].flaming_sword_armed);
    zone.handle(ZoneCommand::SyncPlayerVitals {
        session_id: owner.clone(),
        hp: 0,
        max_hp: 100,
        mp: 200,
    });
    assert_eq!(
        flame_ack(&zone.handle(flame_prepare(&owner, 10_099)), &owner, true),
        0
    );
    let expired = zone.tick(10_100);
    assert_eq!(flame_ack(&expired, &owner, false), 1);
    assert_eq!(flame_ack(&zone.tick(10_101), &owner, false), 0);
    assert_eq!(
        flame_ack(&zone.handle(flame_prepare(&owner, 10_101)), &owner, true),
        0
    );
}

#[test]
fn flaming_sword_expiry_is_checked_without_a_maintenance_tick() {
    let (mut zone, owner) = fixture(MirClass::Warrior);
    zone.handle(flame_prepare(&owner, 100));
    assert!(!zone.player_flaming_sword_armed(&owner, 10_100));
    let forged = zone.handle(ZoneCommand::PlayerAttackObject {
        session_id: owner.clone(),
        object_id: TARGET,
        direction: MirDirection::Right,
        spell: Spell::FlamingSword as u8,
        level: 0,
        attack_type: 0,
        damage: 100,
        now_ms: 10_100,
    });
    assert_eq!(flame_ack(&forged, &owner, false), 1);
    assert!(!forged.iter().any(|item| match item {
        ZoneOutbound::ToSession { packets, .. } | ZoneOutbound::ToMany { packets, .. } => packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::ObjectAttack { .. })),
        _ => false,
    }));
    assert_eq!(zone.native_monsters[&TARGET].hp, 10_000);
    assert_eq!(flame_ack(&zone.tick(10_101), &owner, false), 0);
    let ready = zone.handle(flame_prepare(&owner, 10_102));
    assert_eq!(flame_ack(&ready, &owner, true), 1);
}

#[test]
fn flaming_sword_and_action_clocks_survive_same_and_cross_map_join_but_not_logout() {
    use crate::runtime::zone::ZoneManager;
    let (mut zone, owner) = fixture(MirClass::Warrior);
    zone.handle(flame_prepare(&owner, 100));
    zone.players.get_mut(&owner).unwrap().movement_ready_at_ms = 700;
    zone.players
        .get_mut(&owner)
        .unwrap()
        .next_attack_ready_at_ms = 800;
    zone.players.get_mut(&owner).unwrap().next_spell_ready_at_ms = 1_900;
    zone.players
        .get_mut(&owner)
        .unwrap()
        .magic_ready_at_ms
        .insert(Spell::ShoulderDash as u8, 6_100);
    let same_join = rejoin_data(&zone, &owner, "0");
    zone.handle(ZoneCommand::Join(same_join));
    assert!(zone.player_flaming_sword_armed(&owner, 101));
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::ShoulderDash, 101),
        Some(5_999)
    );
    assert_eq!(zone.players[&owner].next_attack_ready_at_ms, 800);
    let mut manager = ZoneManager::new();
    let first_join = rejoin_data(&zone, &owner, "0");
    // Seed the manager's presence mapping before installing the trusted clocks.
    assert!(manager.install_empty_zone(zone));
    manager.join(first_join.clone());
    assert!(manager.install_empty_zone(ZoneRuntime::new_with_collision(
        ZoneKey::for_map("1"),
        ZoneCollision::unbounded()
    )));
    let mut cross_join = first_join;
    cross_join.map_file_name = "1".into();
    manager.join(cross_join.clone());
    let transferred = manager.zone(&ZoneKey::for_map("1")).unwrap();
    assert!(transferred.player_flaming_sword_armed(&owner, 101));
    assert_eq!(
        transferred.player_magic_cooldown_remaining_ms(&owner, Spell::FlamingSword, 10_099),
        Some(1)
    );
    assert_eq!(
        transferred.player_magic_cooldown_remaining_ms(&owner, Spell::ShoulderDash, 101),
        Some(5_999)
    );
    assert_eq!(transferred.players[&owner].movement_ready_at_ms, 700);
    manager.handle(ZoneCommand::Leave {
        session_id: owner.clone(),
    });
    manager.join(cross_join);
    let fresh = manager.zone(&ZoneKey::for_map("1")).unwrap();
    assert!(!fresh.player_flaming_sword_armed(&owner, 101));
    assert_eq!(
        fresh.player_magic_cooldown_remaining_ms(&owner, Spell::FlamingSword, 101),
        Some(0)
    );
    assert_eq!(
        fresh.player_magic_cooldown_remaining_ms(&owner, Spell::ShoulderDash, 101),
        Some(0)
    );
}

#[test]
fn flame_transfer_clock_never_crosses_account_character_or_object_identity() {
    for mismatch in 0..3 {
        let (mut zone, owner) = fixture(MirClass::Warrior);
        zone.handle(flame_prepare(&owner, 100));
        let mut different = rejoin_data(&zone, &owner, "0");
        match mismatch {
            0 => different.account_id.push_str("-other"),
            1 => different.character_index += 1,
            _ => different.object_id += 1,
        }
        zone.handle(ZoneCommand::Join(different));
        assert!(!zone.player_flaming_sword_armed(&owner, 101));
        assert_eq!(
            zone.player_magic_cooldown_remaining_ms(&owner, Spell::FlamingSword, 101),
            Some(0)
        );
    }
}

#[test]
fn flame_preparation_and_expiry_are_isolated_to_the_correct_owner() {
    let (mut zone, owner) = fixture(MirClass::Warrior);
    let other = SessionId::new("cadence-other");
    let mut other_join = rejoin_data(&zone, &owner, "0");
    other_join.session_id = other.clone();
    other_join.account_id = "other-account".into();
    other_join.object_id += 1;
    other_join.position.y += 2;
    zone.handle(ZoneCommand::Join(other_join));
    let armed = zone.handle(flame_prepare(&owner, 100));
    assert_eq!(flame_ack(&armed, &owner, true), 1);
    assert_eq!(flame_ack(&armed, &other, true), 0);
    assert!(!zone.player_flaming_sword_armed(&other, 101));
    assert_eq!(
        flame_ack(&zone.handle(flame_prepare(&other, 200)), &other, true),
        1
    );
    let first_expiry = zone.tick(10_100);
    assert_eq!(flame_ack(&first_expiry, &owner, false), 1);
    assert_eq!(flame_ack(&first_expiry, &other, false), 0);
    assert!(zone.player_flaming_sword_armed(&other, 10_100));
}

#[test]
fn flaming_sword_never_enters_the_active_magic_damage_or_spend_path() {
    let (mut zone, owner) = fixture(MirClass::Warrior);
    assert_rejected(
        &mut zone,
        &owner,
        magic(&owner, Spell::FlamingSword, 1, 100),
    );
    assert!(!zone.can_player_cast_magic(
        &owner,
        TARGET,
        Spell::FlamingSword,
        MirDirection::Right,
        &Point { x: 11, y: 10 },
        true,
        8,
        3,
        1,
        100
    ));
    assert_eq!(zone.players[&owner].mp, 200);
    zone.handle(flame_prepare(&owner, 101));
    assert_rejected(
        &mut zone,
        &owner,
        magic(&owner, Spell::FlamingSword, 1, 102),
    );
    assert!(zone.player_flaming_sword_armed(&owner, 102));
}
