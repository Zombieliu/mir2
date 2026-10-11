//! Trusted Zone packet projection through a normal authenticated personal
//! session. No wall clock is injected or advanced. These are component checks,
//! not public/native gameplay acceptance.
use super::*;
use crate::{AccountRecord, CharacterRecord};
use mir2_protocol::{ClientPacket, MirClass, MirGender};
use std::time::{Duration, Instant};

fn ordinary_session() -> SimulationSession {
    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        let mut record = AccountRecord::new(CharacterRecord {
            index: 7,
            name: "ZoneBuffClock".into(),
            level: 50,
            class: MirClass::Taoist,
            gender: MirGender::Male,
        });
        record.password = "isolated-zone-buff-clock-password".into();
        store.accounts.insert("isolated-zone-buff-clock".into(), record);
    }
    let mut session = SimulationSession::new(config);
    assert!(session.handle_packet(ClientPacket::Login {
        account_id: "isolated-zone-buff-clock".into(),
        password: "isolated-zone-buff-clock-password".into(),
    }).iter().any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session.try_handle_packet(ClientPacket::StartGame { character_index: 7 })
        .unwrap().iter().any(|packet| matches!(packet, ServerPacket::UserInformation { .. })));
    session
}

fn zone_buff(session: &SimulationSession, duration_ms: i64) -> ClientBuff {
    ClientBuff {
        buff_type: 12,
        object_id: super::super::components::current_player_object_id(session.app.world()).unwrap(),
        expire_time: duration_ms,
        infinite: false,
        visible: true,
        paused: false,
        stats: Vec::new(),
        values: Vec::new(),
    }
}

#[test]
fn finite_zone_projection_does_not_expire_from_personal_tick_count() {
    let mut session = ordinary_session();
    let buff = zone_buff(&session, 21_000);
    let actor = buff.object_id;
    session.apply_zone_player_buff_packets(&[ServerPacket::AddBuff { buff }], actor);
    let initial_tick = session.current_tick();
    let started = Instant::now();
    let mut removed = 0;
    for _ in 0..40 {
        removed += session.tick_shared_zone_personal_state().iter().filter(|packet|
            matches!(packet, ServerPacket::RemoveBuff { object_id, buff_type: 12 } if *object_id == actor)).count();
    }
    let elapsed = started.elapsed();
    eprintln!("zone-buff-clock: personalTicks={} elapsedMs={} curseRemovals={removed}",
        session.current_tick() - initial_tick, elapsed.as_millis());
    assert!(elapsed < Duration::from_secs(21), "test did not observe the pre-expiry interval");
    assert_eq!(removed, 0, "a trusted Zone duration cannot expire from private tick count");
    let projected = session.app.world().resource::<BuffResource>().buffs.iter()
        .find(|buff| buff.key == "curse").expect("actual Zone Curse projection must remain");
    assert!(projected.remaining_ms(session.current_tick()) > 0);
    session.try_handle_packet(ClientPacket::LogOut).unwrap();
}

#[test]
fn finite_zone_projection_still_expires_once_on_real_time() {
    let mut session = ordinary_session();
    let buff = zone_buff(&session, 100);
    let actor = buff.object_id;
    session.apply_zone_player_buff_packets(&[ServerPacket::AddBuff { buff }], actor);
    std::thread::sleep(Duration::from_millis(125));
    let packets = session.tick_shared_zone_personal_state();
    assert_eq!(packets.iter().filter(|packet| matches!(packet,
        ServerPacket::RemoveBuff { object_id, buff_type: 12 } if *object_id == actor)).count(), 1);
    assert!(!session.tick_shared_zone_personal_state().iter().any(|packet|
        matches!(packet, ServerPacket::RemoveBuff { buff_type: 12, .. })));
    session.try_handle_packet(ClientPacket::LogOut).unwrap();
}

#[test]
fn native_zone_removal_clears_projection_without_waiting_for_its_deadline() {
    let mut session = ordinary_session();
    let buff = zone_buff(&session, 21_000);
    let actor = buff.object_id;
    session.apply_zone_player_buff_packets(&[ServerPacket::AddBuff { buff }], actor);
    session.apply_zone_player_buff_packets(&[ServerPacket::RemoveBuff {
        object_id: actor, buff_type: 12,
    }], actor);
    assert!(!session.app.world().resource::<BuffResource>().buffs.iter().any(|buff| buff.key == "curse"));
    for _ in 0..40 {
        assert!(!session.tick_shared_zone_personal_state().iter().any(|packet|
            matches!(packet, ServerPacket::RemoveBuff { buff_type: 12, .. })));
    }
    session.try_handle_packet(ClientPacket::LogOut).unwrap();
}

#[test]
fn paused_projection_resumes_only_on_a_matching_owner_packet() {
    let mut session = ordinary_session();
    let mut buff = zone_buff(&session, 120);
    buff.paused = true;
    let actor = buff.object_id;
    session.apply_zone_player_buff_packets(&[ServerPacket::AddBuff { buff }], actor);
    std::thread::sleep(Duration::from_millis(150));
    for _ in 0..40 {
        assert!(!session.tick_shared_zone_personal_state().iter().any(|packet|
            matches!(packet, ServerPacket::RemoveBuff { buff_type: 12, .. })));
    }
    let remaining = || session.app.world().resource::<BuffResource>().buffs.iter()
        .find(|buff| buff.key == "curse").unwrap().remaining_ms(session.current_tick());
    assert_eq!(remaining(), 120);
    session.apply_zone_player_buff_packets(&[ServerPacket::PauseBuff {
        object_id: actor + 1_000, buff_type: 12, paused: false,
    }], actor);
    assert_eq!(session.app.world().resource::<BuffResource>().buffs.iter()
        .find(|buff| buff.key == "curse").unwrap().remaining_ms(session.current_tick()), 120);
    session.apply_zone_player_buff_packets(&[ServerPacket::PauseBuff {
        object_id: actor, buff_type: 12, paused: false,
    }], actor);
    std::thread::sleep(Duration::from_millis(145));
    assert_eq!(session.tick_shared_zone_personal_state().iter().filter(|packet|
        matches!(packet, ServerPacket::RemoveBuff { object_id, buff_type: 12 } if *object_id == actor)).count(), 1);
    session.try_handle_packet(ClientPacket::LogOut).unwrap();
}

#[test]
fn finite_projection_keeps_existing_scalar_save_duration_layout() {
    let mut session = ordinary_session();
    let buff = zone_buff(&session, 21_000);
    let actor = buff.object_id;
    session.apply_zone_player_buff_packets(&[ServerPacket::AddBuff { buff }], actor);
    let projected = session.app.world().resource::<BuffResource>().buffs.iter()
        .find(|buff| buff.key == "curse").unwrap();
    let encoded = serde_json::to_value(projected).unwrap();
    assert!(encoded["real_time_duration"].is_u64());
    assert!(encoded.get("source_infinite_expire_time_ms").is_none());
    #[derive(serde::Deserialize)]
    struct PriorClockLayout { expires_at_tick: u64, real_time_duration: Option<u64> }
    let prior: PriorClockLayout = serde_json::from_value(encoded.clone()).unwrap();
    assert_ne!(prior.expires_at_tick, u64::MAX);
    assert!(prior.real_time_duration.unwrap() <= 21_000);
    let restored: super::super::buffs::BuffState = serde_json::from_value(encoded).unwrap();
    assert!(!restored.infinite());
    assert!(!restored.expired(u64::MAX), "private tick cannot expire the restored monotonic duration");
    assert!(restored.remaining_ms(u64::MAX) > 0);
    session.try_handle_packet(ClientPacket::LogOut).unwrap();
}
