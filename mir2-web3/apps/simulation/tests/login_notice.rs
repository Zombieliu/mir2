use mir2_protocol::{ClientPacket, Notice, ServerPacket};
use mir2_simulation::{SimulationConfig, SimulationSession};

fn login_demo(session: &mut SimulationSession) {
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_owned(),
        password: "demo".to_owned(),
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
}

fn notices(packets: &[ServerPacket]) -> Vec<&Notice> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            ServerPacket::UpdateNotice { notice } => Some(notice),
            _ => None,
        })
        .collect()
}

#[test]
fn crystal_start_game_delivers_one_project_owned_notice_per_gameplay_session() {
    let config = SimulationConfig::default().with_crystal_world_runtime();
    let expected = config
        .login_notice
        .clone()
        .expect("Crystal runtime should configure a login notice");
    assert!(!expected.message.contains("LOMCN"));
    assert!(!expected.message.contains("Supercode"));

    let mut session = SimulationSession::new(config);
    login_demo(&mut session);

    let first = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(notices(&first), vec![&expected]);

    let duplicate = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(notices(&duplicate).is_empty());

    let _ = session.handle_packet(ClientPacket::LogOut);
    let next_session = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert_eq!(notices(&next_session), vec![&expected]);
}

#[test]
fn login_notice_is_never_sent_before_successful_start_game() {
    let mut session =
        SimulationSession::new(SimulationConfig::default().with_crystal_world_runtime());

    let rejected = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    assert!(matches!(
        rejected.as_slice(),
        [ServerPacket::StartGame { result: 1, .. }]
    ));
    assert!(notices(&rejected).is_empty());
}

#[test]
fn empty_or_explicitly_disabled_login_notice_fails_closed() {
    for config in [
        SimulationConfig::default()
            .with_crystal_world_runtime()
            .with_login_notice(Notice {
                title: "Ignored".to_owned(),
                message: "   \r\n".to_owned(),
            }),
        SimulationConfig::default()
            .with_crystal_world_runtime()
            .without_login_notice(),
    ] {
        let mut session = SimulationSession::new(config);
        login_demo(&mut session);
        let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(notices(&packets).is_empty());
    }
}
