//! Finite Crystal control-poison clocks belong to an online life, not a map.
//! The setup produces paralysis through a genuine CaveMaggot attack, never a
//! fabricated ObjectPoisoned packet or a direct poison-field write.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCollision, ZoneCommand, ZoneJoin,
    ZoneKey, ZoneManager, ZoneMonsterSpawn, ZoneOutbound, ZoneRuntime,
};

const OWNER_ID: u32 = 101;
const POISON_APPLIED_MS: u64 = 600;
const ORIGINAL_EXPIRY_MS: u64 = POISON_APPLIED_MS + 5_000;

fn join() -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new("online-poison-clock"),
        account_id: "online-poison-clock-account".into(), character_index: 0,
        object_id: OWNER_ID, name: "PoisonClockWizard".into(),
        class: MirClass::Wizard, gender: MirGender::Male, level: 26,
        hp: 10_000, max_hp: 10_000, mp: 100,
        map_file_name: "D022".into(), position: Point { x: 330, y: 270 },
        direction: MirDirection::Right,
        chat_profile: Default::default(), combat_stats: Default::default(),
    }
}

fn owner_packets(outbounds: &[ZoneOutbound], owner: &SessionId) -> Vec<ServerPacket> {
    outbounds.iter().flat_map(|outbound| match outbound {
        ZoneOutbound::ToSession { session_id, packets } if session_id == owner => packets.clone(),
        ZoneOutbound::ToMany { session_ids, packets } if session_ids.contains(owner) => packets.clone(),
        ZoneOutbound::ToAll { packets } => packets.clone(),
        _ => Vec::new(),
    }).collect()
}

fn fixture() -> (ZoneManager, ZoneJoin) {
    let mut manager = ZoneManager::new();
    for map in ["D022", "0"] {
        assert!(manager.install_empty_zone(ZoneRuntime::new_with_collision(
            ZoneKey::for_map(map), ZoneCollision::unbounded())));
    }
    let actor = join();
    manager.join(actor.clone());
    // Match the established production paralysis fixture: this real AI7
    // attack at 0ms resolves at 600ms and deterministically applies five seconds.
    manager.handle(ZoneCommand::SpawnMonster {
        session_id: actor.session_id.clone(), now_ms: 0,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None, object_id: 9_004, name: "CaveMaggot".into(),
            name_colour_argb: -1, image: 900, ai: 7,
            disposition: Some(WorldEntityDisposition::Hostile), level: 4,
            max_hp: 20, hp: 20, experience: 6,
            move_speed_ms: 600, attack_speed_ms: 1_200, friendly_guild: None,
            position: Point { x: 331, y: 270 }, direction: MirDirection::Left,
            defense: Default::default(), respawn: None, drops: Vec::new(),
        },
    });
    let attack = manager.tick_all(0);
    assert!(owner_packets(&attack, &actor.session_id).iter().any(|packet| matches!(packet,
        ServerPacket::ObjectAttack { info } if info.object_id == 9_004)));
    let hit = manager.tick_all(POISON_APPLIED_MS);
    assert!(owner_packets(&hit, &actor.session_id).iter().any(|packet| matches!(packet,
        ServerPacket::ObjectPoisoned { object_id: OWNER_ID, poison: 32 })),
        "the failure control requires an authoritative, finite AI7 paralysis receipt");
    (manager, actor)
}

fn transfer(manager: &mut ZoneManager, actor: &ZoneJoin, map: &str, position: Point) -> Vec<ServerPacket> {
    let mut next = actor.clone();
    let (hp, max_hp, mp) = manager.player_vitals(&actor.session_id).unwrap();
    next.hp = hp; next.max_hp = max_hp; next.mp = mp;
    next.map_file_name = map.into(); next.position = position;
    owner_packets(&manager.join(next), &actor.session_id)
}

fn walk(manager: &mut ZoneManager, actor: &ZoneJoin, sequence: u64, now_ms: u64) {
    manager.handle(ZoneCommand::Walk {
        session_id: actor.session_id.clone(), direction: MirDirection::Right,
        seq: sequence, now_ms,
    });
    manager.tick_all(now_ms);
}

#[test]
fn online_poison_handoff_preserves_remaining_time_and_clears_at_original_deadline() {
    let (mut manager, actor) = fixture();
    let landing = Point { x: 328, y: 264 };
    transfer(&mut manager, &actor, "0", landing.clone());
    walk(&mut manager, &actor, 1, ORIGINAL_EXPIRY_MS - 1);
    assert_eq!(manager.player_transform(&actor.session_id).unwrap().0, landing,
        "online transfer cannot prematurely cure the five-second paralysis");
    let expired = manager.tick_all(ORIGINAL_EXPIRY_MS);
    assert!(owner_packets(&expired, &actor.session_id).iter().any(|packet| matches!(packet,
        ServerPacket::ObjectPoisoned { object_id: OWNER_ID, poison: 0 })),
        "the destination must publish zero at the original deadline");
    walk(&mut manager, &actor, 2, ORIGINAL_EXPIRY_MS);
    assert_eq!(manager.player_transform(&actor.session_id).unwrap().0, Point { x: 329, y: 264 });
}

#[test]
fn online_poison_handoff_calibrates_destination_without_refreshing_the_clock() {
    let (mut manager, actor) = fixture();
    let first = transfer(&mut manager, &actor, "0", Point { x: 328, y: 264 });
    assert!(first.iter().any(|packet| matches!(packet,
        ServerPacket::ObjectPoisoned { object_id: OWNER_ID, poison: 32 })),
        "the destination bootstrap must publish the actual current mask");
    manager.tick_all(4_000);
    let final_landing = Point { x: 390, y: 270 };
    let second = transfer(&mut manager, &actor, "D022", final_landing.clone());
    assert!(second.iter().any(|packet| matches!(packet,
        ServerPacket::ObjectPoisoned { object_id: OWNER_ID, poison: 32 })));
    walk(&mut manager, &actor, 1, ORIGINAL_EXPIRY_MS - 1);
    assert_eq!(manager.player_transform(&actor.session_id).unwrap().0, final_landing);
    let expired = manager.tick_all(ORIGINAL_EXPIRY_MS);
    assert!(owner_packets(&expired, &actor.session_id).iter().any(|packet| matches!(packet,
        ServerPacket::ObjectPoisoned { object_id: OWNER_ID, poison: 0 })),
        "returning to the source map cannot restart the five-second deadline");
    walk(&mut manager, &actor, 2, ORIGINAL_EXPIRY_MS);
    assert_eq!(manager.player_transform(&actor.session_id).unwrap().0, Point { x: 391, y: 270 });
}

#[test]
fn online_poison_handoff_also_corrects_unsent_observer_bootstrap() {
    let (mut manager, actor) = fixture();
    let mut observer = actor.clone();
    observer.session_id = SessionId::new("destination-poison-observer");
    observer.account_id = "destination-poison-observer-account".into();
    observer.object_id = 202;
    observer.map_file_name = "0".into();
    observer.position = Point { x: 331, y: 264 };
    manager.join(observer.clone());
    let mut next = actor.clone();
    next.map_file_name = "0".into();
    next.position = Point { x: 328, y: 264 };
    let entered = manager.join(next);
    assert!(owner_packets(&entered, &observer.session_id).iter().any(|packet| matches!(packet,
        ServerPacket::ObjectPlayer { info } if info.object_id == OWNER_ID && info.poison == 32)),
        "a destination observer must see the carried poison in the initial ObjectPlayer");
    let expired = manager.tick_all(ORIGINAL_EXPIRY_MS);
    for recipient in [&actor.session_id, &observer.session_id] {
        assert!(owner_packets(&expired, recipient).iter().any(|packet| matches!(packet,
            ServerPacket::ObjectPoisoned { object_id: OWNER_ID, poison: 0 })));
    }
}

#[test]
fn explicit_leave_then_login_does_not_inherit_an_online_map_clock() {
    let (mut manager, actor) = fixture();
    manager.handle(ZoneCommand::Leave { session_id: actor.session_id.clone() });
    let mut fresh = actor.clone();
    fresh.map_file_name = "0".into();
    fresh.position = Point { x: 328, y: 264 };
    manager.join(fresh);
    walk(&mut manager, &actor, 1, 1_000);
    assert_eq!(manager.player_transform(&actor.session_id).unwrap().0, Point { x: 329, y: 264 },
        "only an uninterrupted online handoff may transfer the old Zone clock");
}

#[test]
fn different_authenticated_character_cannot_inherit_a_poison_handoff() {
    let (mut manager, actor) = fixture();
    let mut different = actor.clone();
    different.account_id = "different-account".into();
    different.character_index = 1;
    different.map_file_name = "0".into();
    different.position = Point { x: 328, y: 264 };
    let entered = manager.join(different);
    assert!(owner_packets(&entered, &actor.session_id).iter().any(|packet| matches!(packet,
        ServerPacket::ObjectPoisoned { object_id: OWNER_ID, poison: 0 })));
    walk(&mut manager, &actor, 1, 1_000);
    assert_eq!(manager.player_transform(&actor.session_id).unwrap().0, Point { x: 329, y: 264 });
}
