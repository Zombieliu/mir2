use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{SimulationConfig, SimulationSession, WorldSnapshot};

fn login_demo(session: &mut SimulationSession) {
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
}

fn started_session() -> SimulationSession {
    let mut session = SimulationSession::new(SimulationConfig::default());
    login_demo(&mut session);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    session
}

fn player_tile(snapshot: &WorldSnapshot) -> (i32, i32) {
    let object_id = snapshot
        .player_object_id
        .expect("started session should have a player object id");
    let player = snapshot
        .entities
        .iter()
        .find(|entity| entity.object_id == object_id)
        .expect("started session should include the player entity");
    (player.x, player.y)
}

#[test]
fn pre_start_big_map_commands_are_silent() {
    let mut session = SimulationSession::new(SimulationConfig::default());

    assert!(session
        .handle_packet(ClientPacket::RequestMapInfo { map_index: 1 })
        .is_empty());
    assert!(session
        .handle_packet(ClientPacket::SearchMap {
            text: "Bic".to_string(),
        })
        .is_empty());
    assert!(session
        .handle_packet(ClientPacket::TeleportToNpc { object_id: 1 })
        .is_empty());
}

#[test]
fn request_map_info_uses_crystal_packets_and_connection_cache() {
    let mut session = started_session();

    let packets = session.handle_packet(ClientPacket::RequestMapInfo { map_index: 1 });
    assert_eq!(packets.len(), 2);
    assert!(matches!(
        &packets[0],
        ServerPacket::WorldMapSetup {
            setup,
            teleport_to_npc_cost: 3_000,
        } if !setup.enabled && setup.icons.is_empty()
    ));
    assert!(matches!(
        &packets[1],
        ServerPacket::NewMapInfo { map_index: 1, info }
            if info.title == "BichonProvince"
                && info.width == 700
                && info.height == 700
                && info.big_map == 101
                && info.movements.len() == 6
                && info.npcs.len() == 40
                && info.npcs.iter().all(|npc| !npc.can_teleport_to)
    ));
    assert!(!packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::MapInformation { .. })));

    assert!(session
        .handle_packet(ClientPacket::RequestMapInfo { map_index: 1 })
        .is_empty());
    assert!(session
        .handle_packet(ClientPacket::RequestMapInfo { map_index: -1 })
        .is_empty());
}

#[test]
fn search_map_is_case_insensitive_and_stably_selects_first_map() {
    let mut session = started_session();

    let packets = session.handle_packet(ClientPacket::SearchMap {
        text: "  nAt  ".to_string(),
    });
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::NewMapInfo { map_index: 34, info }
            if info.title == "NaturalCave"
    )));
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::SearchMapResult {
            map_index: 34,
            npc_index: 0,
        }
    )));

    let repeated = session.handle_packet(ClientPacket::SearchMap {
        text: "NAT".to_string(),
    });
    assert_eq!(
        repeated,
        vec![ServerPacket::SearchMapResult {
            map_index: 34,
            npc_index: 0,
        }]
    );
}

#[test]
fn search_map_returns_npc_and_handles_unicode_miss() {
    let mut session = started_session();

    let npc_packets = session.handle_packet(ClientPacket::SearchMap {
        text: "gIl".to_string(),
    });
    assert!(npc_packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::SearchMapResult {
            map_index: 1,
            npc_index: 1,
        }
    )));

    let unicode_packets = session.handle_packet(ClientPacket::SearchMap {
        text: "不存在".to_string(),
    });
    assert_eq!(
        unicode_packets,
        vec![ServerPacket::SearchMapResult {
            map_index: -1,
            npc_index: 0,
        }]
    );
}

#[test]
fn search_map_rejects_empty_short_and_oversize_queries() {
    let mut session = started_session();

    for text in [String::new(), "ab".to_string(), "x".repeat(65)] {
        assert!(session
            .handle_packet(ClientPacket::SearchMap { text })
            .is_empty());
    }
}

#[test]
fn teleport_to_npc_is_a_silent_noop_without_eligible_imported_destination() {
    let mut session = started_session();
    let before = session.world_snapshot();

    let packets = session.handle_packet(ClientPacket::TeleportToNpc { object_id: 1 });
    let after = session.world_snapshot();

    assert!(packets.is_empty());
    assert_eq!(after.map_file_name, before.map_file_name);
    assert_eq!(player_tile(&after), player_tile(&before));
    assert_eq!(after.gold, before.gold);
}

#[test]
fn logout_resets_connection_scoped_map_info_cache() {
    let mut session = started_session();
    let first = session.handle_packet(ClientPacket::RequestMapInfo { map_index: 1 });
    assert!(matches!(
        first.first(),
        Some(ServerPacket::WorldMapSetup { .. })
    ));

    session.handle_packet(ClientPacket::LogOut);
    login_demo(&mut session);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let second = session.handle_packet(ClientPacket::RequestMapInfo { map_index: 1 });
    assert!(matches!(
        second.first(),
        Some(ServerPacket::WorldMapSetup { .. })
    ));
    assert!(second
        .iter()
        .any(|packet| matches!(packet, ServerPacket::NewMapInfo { map_index: 1, .. })));
}
