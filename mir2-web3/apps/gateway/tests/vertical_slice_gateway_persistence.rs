//! Gateway-layer evidence for the ordinary Bichon vertical slice.
//!
//! The richer combat, quest, drop, pickup, and reward sequence is exercised
//! without privileged commands by `mir2-simulation`'s
//! `ordinary_candidate_loop` integration test.  This companion test owns the
//! missing boundary: a fresh account and character must traverse the Gateway
//! command route, save through logout, then reload through a newly constructed
//! Gateway session.  It deliberately uses only normal Crystal client packets.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::{GatewayConfig, GatewaySession};
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, ServerPacket};
use mir2_simulation::WorldEntityKind;

const TEST_RECOVERY_MAC_KEY: [u8; 32] = [
    0x10, 0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87, 0x98, 0xa9, 0xba, 0xcb, 0xdc, 0xed, 0xfe, 0x0f,
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xf1, 0x02,
];

struct SaveFileGuard(PathBuf);

impl Drop for SaveFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn unique_save_path() -> SaveFileGuard {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    SaveFileGuard(std::env::temp_dir().join(format!(
        "mir2-gateway-vertical-slice-{}-{nanos}.json",
        std::process::id()
    )))
}

fn file_backed_gateway_config(path: PathBuf) -> GatewayConfig {
    GatewayConfig::default()
        .with_account_store_path(path)
        .with_save_recovery_mac_key(TEST_RECOVERY_MAC_KEY)
        .expect("test-only file store must have a valid recovery MAC key")
}

fn login(session: &mut GatewaySession, account_id: &str, password: &str) {
    let packets = session
        .try_handle_packet(ClientPacket::Login {
            account_id: account_id.to_string(),
            password: password.to_string(),
        })
        .expect("Gateway Login should execute");
    assert!(
        packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "Gateway Login should succeed: {packets:?}"
    );
}

fn player(session: &GatewaySession) -> mir2_simulation::WorldEntitySnapshot {
    session
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .expect("Gateway StartGame should expose the self player")
}

#[test]
fn gateway_fresh_account_bichon_logout_and_new_session_reload_are_authoritative() {
    let save_guard = unique_save_path();
    let save_path = save_guard.0.clone();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let account_id = format!("gateway_slice_{}_{}", std::process::id(), suffix);
    let password = "GatewaySlice42!";

    let mut first = GatewaySession::new(file_backed_gateway_config(save_path.clone()));
    let created = first
        .try_handle_packet(ClientPacket::NewAccount {
            account_id: account_id.clone(),
            password: password.to_string(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        })
        .expect("Gateway NewAccount should execute");
    assert!(
        created
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 })),
        "Gateway NewAccount should create an ordinary account: {created:?}"
    );
    login(&mut first, &account_id, password);

    let created_character = first.handle_packet(ClientPacket::NewCharacter {
        name: format!("GateSlice{}", std::process::id()),
        gender: MirGender::Male,
        class: MirClass::Warrior,
    });
    let character_index = created_character
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Gateway NewCharacter should succeed: {created_character:?}"));

    let started = first.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        started.iter().any(|packet| matches!(
            packet,
            ServerPacket::StartGame {
                result: 4,
                resolution
            } if *resolution > 0
        )),
        "Gateway StartGame should enter Bichon: {started:?}"
    );
    let initial = first.world_snapshot();
    assert_eq!(initial.map_file_name.as_deref(), Some("0"));
    assert!(initial
        .map_title
        .as_deref()
        .is_some_and(|title| title.to_ascii_lowercase().contains("bichon")));

    let player_before = player(&first);
    let turned = first.handle_packet(ClientPacket::Turn {
        direction: MirDirection::Left,
    });
    assert!(
        turned
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserLocation { .. })),
        "Gateway Turn must reach the authoritative Zone: {turned:?}"
    );
    let player_after = player(&first);
    assert_eq!(player_after.direction, MirDirection::Left);
    assert_eq!(
        (player_after.x, player_after.y),
        (player_before.x, player_before.y),
        "a turn must not manufacture movement"
    );

    let logout = first.handle_packet(ClientPacket::LogOut);
    assert!(
        logout
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })),
        "Gateway LogOut should save then acknowledge: {logout:?}"
    );
    drop(first);

    let mut second = GatewaySession::new(file_backed_gateway_config(save_path.clone()));
    login(&mut second, &account_id, password);
    let restarted = second.handle_packet(ClientPacket::StartGame { character_index });
    assert!(
        restarted
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "new Gateway session should restore its selected character: {restarted:?}"
    );
    let reloaded = player(&second);
    assert_eq!(
        (reloaded.x, reloaded.y, reloaded.direction),
        (player_after.x, player_after.y, player_after.direction),
        "a new Gateway session must reload the saved authoritative transform"
    );

    drop(second);
    drop(save_guard);
    assert!(
        !save_path.exists(),
        "Gateway evidence save file should be cleaned up"
    );
}
