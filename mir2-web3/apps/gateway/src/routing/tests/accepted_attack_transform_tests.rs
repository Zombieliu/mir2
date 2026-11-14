//! Accepted physical attacks must carry the Zone transform through gateway
//! presence and the ordinary pre-LogOut save, not only an animation/correction.
//! Fixture placement/vitals below are trusted test setup. Commands under test
//! are normal public Attack, ChangeAMode, LogOut, Login and StartGame commands.
//! No pre-attack Turn, skills, special equipment or fixed damage are required.
use super::*;

const OWNER_POSITION: Point = Point { x: 100, y: 100 };
const PEER_POSITION: Point = Point { x: 101, y: 100 };

fn isolated_shared_state() -> Arc<Mutex<SharedInProcessZoneState>> {
    let mut config = GatewayConfig::default();
    config.visible_monsters.clear();
    config.visible_npcs.clear();
    config.map_hazards.clear();
    let mut state = SharedInProcessZoneState::new();
    state.experience_authority = Some(config);
    Arc::new(Mutex::new(state))
}

fn place_down(runtime: &mut SharedInProcessZoneSessionRuntime, position: Point) {
    // Keep all three starting transforms equal and deliberately different from
    // Right/Left. Pre-turning to the attack direction would hide the regression.
    let session_id = runtime.current_zone_session_id().unwrap();
    runtime.dispatch_zone_player_command(
        ZoneCommand::SyncPlayerTransform {
            session_id,
            position: position.clone(),
            direction: MirDirection::Down,
        },
        false,
    );
    runtime
        .inner
        .force_authoritative_player_transform(position.clone(), MirDirection::Down);
    runtime
        .inner
        .force_authoritative_player_vitals_with_max_hp(Some(10_000), Some(10_000), None);
    runtime.sync_zone_snapshot();
    assert_transform(runtime, &position, MirDirection::Down);
}

fn assert_transform(
    runtime: &SharedInProcessZoneSessionRuntime,
    position: &Point,
    direction: MirDirection,
) {
    for snapshot in [runtime.world_snapshot(), runtime.inner.world_snapshot()] {
        let owner = snapshot
            .entities
            .iter()
            .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
            .expect("live owner must be visible");
        assert_eq!((owner.x, owner.y), (position.x, position.y));
        assert_eq!(owner.direction, direction, "gateway/personal owner facing");
    }
    let key = runtime.current_presence_key().unwrap();
    let session_id = runtime.current_zone_session_id().unwrap();
    let state = runtime.zone_state.lock().unwrap();
    let presence = state.players.get(&key).expect("live gateway presence");
    assert_eq!(
        (presence.entity.x, presence.entity.y, presence.entity.direction),
        (position.x, position.y, direction),
        "gateway presence must follow the accepted Zone transform"
    );
    assert_eq!(
        state.zone_manager.player_transform(&session_id),
        Some((position.clone(), direction)),
        "authoritative Zone transform"
    );
}

fn set_attack_mode(runtime: &mut SharedInProcessZoneSessionRuntime, mode: u8) {
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::ChangeAMode { mode }))
        .unwrap();
    assert!(packets.iter().any(|packet| matches!(
        packet, ServerPacket::ChangeAMode { mode: actual } if *actual == mode
    )));
}

fn directional_attack(direction: MirDirection) -> WorldCommand {
    WorldCommand::ClientPacket(ClientPacket::Attack {
        direction,
        spell: Spell::None,
    })
}

fn assert_launched(
    runtime: &SharedInProcessZoneSessionRuntime,
    packets: &[ServerPacket],
    direction: MirDirection,
) {
    let local_id = runtime.inner.world_snapshot().player_object_id.unwrap();
    assert!(packets.iter().any(|packet| matches!(
        packet, ServerPacket::ObjectAttack { info }
            if info.object_id == local_id && info.direction == direction
    )), "this command must actually launch ordinary melee: {packets:?}");
}

fn normal_logout_and_fresh_login(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    position: &Point,
    direction: MirDirection,
) {
    let identity = runtime.inner.active_identity().unwrap();
    let old_key = runtime.current_presence_key().unwrap();
    let old_session = runtime.current_zone_session_id().unwrap();
    let config = runtime
        .zone_state
        .lock()
        .unwrap()
        .experience_authority
        .clone()
        .unwrap();
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .expect("ordinary LogOut must save before acknowledging");
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
    assert!(runtime.inner.active_identity().is_none());
    {
        let state = runtime.zone_state.lock().unwrap();
        assert!(!state.players.contains_key(&old_key));
        assert!(state.zone_manager.player_transform(&old_session).is_none());
    }
    {
        let store = config.account_store.lock().unwrap();
        let save = store.accounts[&identity.account_id]
            .saves
            .get(&identity.character_index)
            .expect("normal logout persisted the existing character");
        assert_eq!(&save.position, position);
        assert_eq!(save.direction, direction, "persisted facing, before reentry");
    }
    // A new personal runtime AND a new shared Zone can only get this transform
    // from the normal saved character, not the prior online presence or Leave.
    let mut fresh_state = SharedInProcessZoneState::new();
    fresh_state.experience_authority = Some(config);
    let mut fresh = shared_session_runtime(Arc::new(Mutex::new(fresh_state)));
    let login = fresh
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: identity.account_id.clone(),
            password: identity.account_id,
        }))
        .unwrap();
    assert!(login
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    let start = fresh
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: identity.character_index,
        }))
        .unwrap();
    assert!(start.iter().any(|packet| matches!(
        packet, ServerPacket::StartGame { result: 4, .. }
    )));
    assert!(start.iter().any(|packet| matches!(
        packet, ServerPacket::UserInformation { info }
            if info.location == *position && info.direction == direction
    )), "fresh initial UserInformation must carry the saved transform");
    assert_transform(&fresh, position, direction);
    assert!(fresh
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
}

fn adjacent_players() -> (
    SharedInProcessZoneSessionRuntime,
    SharedInProcessZoneSessionRuntime,
) {
    let shared = isolated_shared_state();
    let mut owner = shared_session_runtime(shared.clone());
    let mut peer = shared_session_runtime(shared);
    start_new_runtime(&mut owner, "attack-transform-owner", "TransformOwner");
    start_new_runtime(&mut peer, "attack-transform-peer", "TransformPeer");
    place_down(&mut owner, OWNER_POSITION);
    place_down(&mut peer, PEER_POSITION);
    assert!(!owner.world_snapshot().in_safe_zone);
    assert!(!peer.world_snapshot().in_safe_zone);
    (owner, peer)
}

#[test]
fn accepted_pvp_attack_transform_reaches_presence_and_normal_fresh_save() {
    let (mut owner, mut peer) = adjacent_players();
    set_attack_mode(&mut owner, 5);
    set_attack_mode(&mut peer, 5);
    for (runtime, position, direction) in [
        (&mut owner, &OWNER_POSITION, MirDirection::Right),
        (&mut peer, &PEER_POSITION, MirDirection::Left),
    ] {
        assert_transform(runtime, position, MirDirection::Down);
        let command = directional_attack(direction);
        assert!(runtime
            .prepare_zone_native_player_attack(&command)
            .expect("the public direction must resolve the adjacent shared player")
            .is_player_target);
        let packets = runtime.execute(command).unwrap();
        assert_launched(runtime, &packets, direction);
        assert_transform(runtime, position, direction);
    }
    normal_logout_and_fresh_login(&mut owner, &OWNER_POSITION, MirDirection::Right);
    normal_logout_and_fresh_login(&mut peer, &PEER_POSITION, MirDirection::Left);
}

#[test]
fn accepted_pve_object_attack_transform_reaches_presence_and_normal_fresh_save() {
    for direction in [MirDirection::Right, MirDirection::Left] {
        let shared = isolated_shared_state();
        let mut owner = shared_session_runtime(shared.clone());
        start_new_runtime(&mut owner, "attack-transform-pve", "TransformPve");
        place_down(&mut owner, OWNER_POSITION);
        let mut target = shared_monster_entity(260_943);
        target.ai = Some(2);
        target.disposition = WorldEntityDisposition::Hostile;
        target.hp = Some(10_000);
        target.max_hp = Some(10_000);
        target.x = OWNER_POSITION.x + if direction == MirDirection::Right { 1 } else { -1 };
        target.y = OWNER_POSITION.y;
        shared.lock().unwrap().sync_map_layer(
            "0".to_owned(),
            vec![target.clone()],
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        );
        assert_transform(&owner, &OWNER_POSITION, MirDirection::Down);
        // Public object-target attack must derive its own facing from the
        // shared target. No caller-authored transform or Turn is sent.
        let command = WorldCommand::Attack { object_id: target.object_id };
        assert!(!owner
            .prepare_zone_native_player_attack(&command)
            .expect("shared live monster must be resolved")
            .is_player_target);
        let packets = owner.execute(command).unwrap();
        assert_launched(&owner, &packets, direction);
        assert_transform(&owner, &OWNER_POSITION, direction);
        normal_logout_and_fresh_login(&mut owner, &OWNER_POSITION, direction);
    }
}

#[test]
fn rejected_directional_pvp_does_not_rotate_authority_or_saved_facing() {
    let (mut owner, mut peer) = adjacent_players();
    set_attack_mode(&mut owner, 0);
    let command = directional_attack(MirDirection::Right);
    assert!(owner
        .prepare_zone_native_player_attack(&command)
        .expect("rejection must be policy, not an absent target")
        .is_player_target);
    let peer_hp = peer.world_snapshot().player_hp;
    let packets = owner.execute(command).unwrap();
    assert!(!packets.iter().any(|packet| matches!(
        packet, ServerPacket::ObjectAttack { .. } | ServerPacket::DamageIndicator { .. }
    )), "peace-mode first attack must be rejected: {packets:?}");
    assert_transform(&owner, &OWNER_POSITION, MirDirection::Down);
    peer.execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive { time: 1 }))
        .unwrap();
    assert_eq!(peer.world_snapshot().player_hp, peer_hp);
    normal_logout_and_fresh_login(&mut owner, &OWNER_POSITION, MirDirection::Down);
    assert!(peer
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap()
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));
}
