use std::sync::{Arc, Mutex};

use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::SimulationConfig;

use super::{
    execute_session_action, ExplicitWorldLeaveState, OwnedSessionRoute, SessionAction,
    SharedBackgroundRouteRefreshRecord,
};
use crate::cache::{
    refresh_session_cache_with_route_lease, GatewaySessionCache, InMemoryGatewaySessionCache,
    RedisGatewaySessionCache,
};
use crate::GatewaySession;

fn login(config: &SimulationConfig, create: bool) -> GatewaySession {
    let mut session = GatewaySession::new(config.clone());
    if create {
        session
            .try_handle_packet(ClientPacket::NewAccount {
                account_id: "routefixture".into(),
                password: "routefixturepw".into(),
                birth_date_binary: 0,
                user_name: String::new(),
                secret_question: String::new(),
                secret_answer: String::new(),
                email_address: String::new(),
            })
            .unwrap();
    }
    let packets = session
        .try_handle_packet(ClientPacket::Login {
            account_id: "routefixture".into(),
            password: "routefixturepw".into(),
        })
        .unwrap();
    let mut index = packets.iter().find_map(|packet| match packet {
        ServerPacket::LoginSuccess { characters } => {
            characters.first().map(|character| character.index)
        }
        _ => None,
    });
    if create {
        index = session
            .try_handle_packet(ClientPacket::NewCharacter {
                name: "RouteTest".into(),
                class: MirClass::Warrior,
                gender: MirGender::Male,
            })
            .unwrap()
            .iter()
            .find_map(|packet| match packet {
                ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
                _ => None,
            });
    }
    session
        .try_handle_packet(ClientPacket::StartGame {
            character_index: index.unwrap(),
        })
        .unwrap();
    assert!(session.active_identity().is_some());
    session
}

fn cached(
    cache: &InMemoryGatewaySessionCache,
    session: &GatewaySession,
) -> SharedBackgroundRouteRefreshRecord {
    let record = refresh_session_cache_with_route_lease(cache, session, 60)
        .unwrap()
        .unwrap();
    Arc::new(Mutex::new(Some((record, session.session_id().to_string()))))
}

#[test]
fn explicit_leave_releases_captured_route_after_identity_cleared_before_relogin() {
    for packet in [ClientPacket::LogOut, ClientPacket::Disconnect] {
        let disconnects = matches!(&packet, ClientPacket::Disconnect);
        let config = SimulationConfig::default();
        let mut session = login(&config, true);
        let cache = InMemoryGatewaySessionCache::default();
        let refresh = cached(&cache, &session);
        let route = OwnedSessionRoute::capture(&session).unwrap();
        let packets =
            execute_session_action(&mut session, SessionAction::Packet(packet), true, true)
                .unwrap();
        if disconnects {
            assert!(packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::Disconnect { reason: 0 })));
        } else {
            assert!(session.active_identity().is_none());
        }
        let mut leave = ExplicitWorldLeaveState::default();
        assert!(leave
            .finish(&cache, &session, Some(route.clone()), &packets, &refresh)
            .unwrap());
        assert!(leave.completed);
        assert!(leave.pending_route.is_none());
        assert!(refresh.lock().unwrap().is_none());
        assert!(cache.get(&route.key).is_none());
        assert_eq!(cache.route_lease_count(), 0);
        cache
            .acquire_route_lease(&route.key, "next-socket", 60)
            .unwrap();
        assert_eq!(
            cache.route_lease_count(),
            1,
            "normal relogin must not wait for lease expiry"
        );
    }
}

#[test]
fn explicit_leave_failed_save_preserves_identity_lease_and_refresh() {
    let config = SimulationConfig::default();
    let mut current = login(&config, true);
    let mut stale = login(&config, false);
    let cache = InMemoryGatewaySessionCache::default();
    let refresh = cached(&cache, &stale);
    let route = OwnedSessionRoute::capture(&stale).unwrap();
    current.save_active_character().unwrap();
    let failure = execute_session_action(
        &mut stale,
        SessionAction::Packet(ClientPacket::LogOut),
        true,
        true,
    )
    .unwrap_err();
    assert!(failure.contains("stale"));
    let mut leave = ExplicitWorldLeaveState::default();
    assert!(!leave
        .finish(&cache, &stale, Some(route.clone()), &[], &refresh)
        .unwrap());
    assert!(!leave.completed);
    assert!(stale.active_identity().is_some());
    assert!(refresh.lock().unwrap().is_some());
    assert_eq!(cache.route_lease_count(), 1);
    assert!(cache.get(&route.key).is_some());
}

#[test]
fn explicit_leave_logout_failed_packet_never_claims_completed_leave() {
    let session = GatewaySession::new(SimulationConfig::default());
    let cache = InMemoryGatewaySessionCache::default();
    let refresh = Arc::new(Mutex::new(None));
    let mut leave = ExplicitWorldLeaveState::default();
    assert!(!leave
        .finish(
            &cache,
            &session,
            None,
            &[ServerPacket::LogOutFailed],
            &refresh
        )
        .unwrap());
    assert!(!leave.completed);
}

#[test]
fn explicit_leave_cache_failure_keeps_captured_owner_for_retry_without_reentering_world() {
    let config = SimulationConfig::default();
    let mut session = login(&config, true);
    let cache = InMemoryGatewaySessionCache::default();
    let refresh = cached(&cache, &session);
    let route = OwnedSessionRoute::capture(&session).unwrap();
    let packets = execute_session_action(
        &mut session,
        SessionAction::Packet(ClientPacket::LogOut),
        true,
        true,
    )
    .unwrap();
    let unavailable =
        RedisGatewaySessionCache::new("redis://127.0.0.1:1", "unused-release-fixture", 60);
    let mut leave = ExplicitWorldLeaveState::default();
    assert!(leave
        .finish(
            &unavailable,
            &session,
            Some(route.clone()),
            &packets,
            &refresh
        )
        .is_err());
    assert!(
        leave.completed,
        "cache outage cannot undo successful save/logout"
    );
    assert!(session.active_identity().is_none());
    assert!(
        refresh.lock().unwrap().is_none(),
        "old background refresh must stop"
    );
    assert!(
        leave.pending_route.is_some(),
        "retain the cleared identity's owner for cleanup retry"
    );
    leave.retry_release(&cache).unwrap();
    assert!(leave.pending_route.is_none());
    assert!(cache.get(&route.key).is_none());
    assert_eq!(cache.route_lease_count(), 0);
}

#[test]
fn explicit_leave_old_owner_cannot_delete_new_lease_even_before_new_record_is_published() {
    let session = login(&SimulationConfig::default(), true);
    let cache = InMemoryGatewaySessionCache::default();
    cached(&cache, &session);
    let route = OwnedSessionRoute::capture(&session).unwrap();
    cache.release_route_lease(&route.key, &route.owner).unwrap();
    cache
        .acquire_route_lease(&route.key, "new-owner", 60)
        .unwrap();
    assert!(!cache
        .release_owned_session_route(&route.key, &route.owner, &route.character_name)
        .unwrap());
    assert!(
        cache.get(&route.key).is_some(),
        "old record is retained while new owner publishes"
    );
    assert!(cache
        .acquire_route_lease(&route.key, "third-owner", 60)
        .is_err());
    assert!(cache.renew_route_lease(&route.key, "new-owner", 60).is_ok());
}

#[test]
fn explicit_leave_old_owner_cannot_delete_new_record_or_refresh_it_back() {
    let session = login(&SimulationConfig::default(), true);
    let cache = InMemoryGatewaySessionCache::default();
    let refresh = cached(&cache, &session);
    let route = OwnedSessionRoute::capture(&session).unwrap();
    let old_record = refresh.lock().unwrap().as_ref().unwrap().0.clone();
    assert!(cache
        .release_owned_session_route(&route.key, &route.owner, &route.character_name)
        .unwrap());
    assert!(!cache
        .refresh_owned_route_lease_record(old_record.clone(), &route.owner, 60)
        .unwrap());
    cache
        .acquire_route_lease(&route.key, "new-owner", 60)
        .unwrap();
    let mut new_record = old_record.clone();
    new_record.route_lease_owner = Some("new-owner".into());
    new_record.gateway_session_id = Some("new-owner".into());
    cache.put(new_record.clone());
    assert!(!cache
        .release_owned_session_route(&route.key, &route.owner, &route.character_name)
        .unwrap());
    assert!(cache
        .refresh_owned_route_lease_record(old_record, &route.owner, 60)
        .is_err());
    assert_eq!(cache.get(&route.key), Some(new_record));
    assert_eq!(cache.route_lease_count(), 1);
}
