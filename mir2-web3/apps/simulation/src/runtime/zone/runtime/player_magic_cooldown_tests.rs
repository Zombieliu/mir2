use super::*;
use crate::runtime::zone::types::{ZoneChatProfile, ZonePlayerCombatStats};
use mir2_protocol::{MirClass, MirGender};

const OWNER_OBJECT_ID: u32 = 10_001;
const TARGET_OBJECT_ID: u32 = 20_001;

fn join_taoist(zone: &mut ZoneRuntime, session: &str, object_id: u32) -> SessionId {
    let session_id = SessionId::new(session);
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: session_id.clone(),
        account_id: format!("{session}-account"),
        character_index: i32::try_from(object_id).expect("test object id fits i32"),
        object_id,
        name: session.to_string(),
        class: MirClass::Taoist,
        gender: MirGender::Male,
        level: 23,
        hp: 100,
        max_hp: 100,
        mp: 100,
        map_file_name: zone.key().map_file_name.clone(),
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Right,
        chat_profile: ZoneChatProfile::default(),
        combat_stats: ZonePlayerCombatStats {
            max_sc: 10,
            ..Default::default()
        },
    }));
    zone.handle(ZoneCommand::sync_player_combat_state(
        session_id.clone(),
        MirClass::Taoist,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    session_id
}

fn spawn_target(zone: &mut ZoneRuntime, owner: &SessionId) {
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner.clone(),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: TARGET_OBJECT_ID,
            name: "Deer".to_string(),
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            disposition: Some(crate::WorldEntityDisposition::Neutral),
            level: 1,
            max_hp: 100,
            hp: 100,
            experience: 1,
            move_speed_ms: 600,
            attack_speed_ms: 1_200,
            friendly_guild: None,
            position: Point { x: 12, y: 10 },
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
    });
}

fn cast(owner: &SessionId, spell: Spell, target_id: u32, cast: bool, now_ms: u64) -> ZoneCommand {
    ZoneCommand::PlayerCastMagic {
        session_id: owner.clone(),
        object_id: target_id,
        spell,
        direction: MirDirection::Right,
        target: Point { x: 12, y: 10 },
        cast,
        level: 0,
        damage: 8,
        mp_cost: 3,
        cooldown_ms: 1_800,
        now_ms,
    }
}

fn contains_accepted_magic(outbounds: &[ZoneOutbound], spell: Spell, target_id: u32) -> bool {
    outbounds.iter().any(|outbound| {
        matches!(outbound,
            ZoneOutbound::ToSession { packets, .. }
                if packets.iter().any(|packet| matches!(packet,
                    ServerPacket::Magic {
                        spell: received_spell,
                        target_id: received_target_id,
                        cast: true,
                        ..
                    } if *received_spell == spell && *received_target_id == target_id
                ))
        )
    })
}

#[test]
fn accepted_soulfire_uses_zone_clock_until_exact_spell_ready_time() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    let owner = join_taoist(&mut zone, "owner", OWNER_OBJECT_ID);
    spawn_target(&mut zone, &owner);

    let accepted = zone.handle(cast(&owner, Spell::SoulFireBall, TARGET_OBJECT_ID, true, 0));
    assert!(contains_accepted_magic(
        &accepted,
        Spell::SoulFireBall,
        TARGET_OBJECT_ID,
    ));
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::SoulFireBall, 0),
        Some(1_800),
    );
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::SoulFireBall, 1_000),
        Some(800),
    );
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::SoulFireBall, 1_799),
        Some(1),
    );
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::SoulFireBall, 1_800),
        Some(0),
    );

    let recast = zone.handle(cast(
        &owner,
        Spell::SoulFireBall,
        TARGET_OBJECT_ID,
        true,
        1_800,
    ));
    assert!(contains_accepted_magic(
        &recast,
        Spell::SoulFireBall,
        TARGET_OBJECT_ID,
    ));
}

#[test]
fn cooldown_getter_includes_global_gate_without_cross_owner_leakage() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    let owner = join_taoist(&mut zone, "owner", OWNER_OBJECT_ID);
    let other = join_taoist(&mut zone, "other", OWNER_OBJECT_ID + 1);
    spawn_target(&mut zone, &owner);

    assert!(contains_accepted_magic(
        &zone.handle(cast(&owner, Spell::SoulFireBall, TARGET_OBJECT_ID, true, 0,)),
        Spell::SoulFireBall,
        TARGET_OBJECT_ID,
    ));
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::FireBall, 0),
        Some(300),
        "a different spell remains gated by the shared Crystal action delay",
    );
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::FireBall, 300),
        Some(0),
    );
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&other, Spell::SoulFireBall, 0),
        Some(0),
        "another owner has no inherited cooldown",
    );
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&SessionId::new("missing"), Spell::SoulFireBall, 0),
        None,
    );
}

#[test]
fn uncast_and_rejected_magic_do_not_create_or_extend_cooldown() {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    let owner = join_taoist(&mut zone, "owner", OWNER_OBJECT_ID);
    spawn_target(&mut zone, &owner);

    let uncast = zone.handle(cast(
        &owner,
        Spell::SoulFireBall,
        TARGET_OBJECT_ID,
        false,
        0,
    ));
    assert!(!contains_accepted_magic(
        &uncast,
        Spell::SoulFireBall,
        TARGET_OBJECT_ID,
    ));
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::SoulFireBall, 0),
        Some(0),
    );

    let rejected = zone.handle(cast(&owner, Spell::SoulFireBall, 99_999, true, 0));
    assert!(!contains_accepted_magic(
        &rejected,
        Spell::SoulFireBall,
        99_999,
    ));
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::SoulFireBall, 0),
        Some(0),
    );

    assert!(contains_accepted_magic(
        &zone.handle(cast(&owner, Spell::SoulFireBall, TARGET_OBJECT_ID, true, 0,)),
        Spell::SoulFireBall,
        TARGET_OBJECT_ID,
    ));
    let duplicate = zone.handle(cast(
        &owner,
        Spell::SoulFireBall,
        TARGET_OBJECT_ID,
        true,
        200,
    ));
    assert!(!contains_accepted_magic(
        &duplicate,
        Spell::SoulFireBall,
        TARGET_OBJECT_ID,
    ));
    assert_eq!(
        zone.player_magic_cooldown_remaining_ms(&owner, Spell::SoulFireBall, 200),
        Some(1_600),
        "rejection must preserve the original accepted deadline",
    );
}
