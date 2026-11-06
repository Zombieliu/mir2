use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{SimulationConfig, SimulationSession};

fn login_demo(session: &mut SimulationSession) {
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: "demo".to_string(),
        password: "demo".to_string(),
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
}

#[test]
fn yin_devil_node_stays_immobile_and_emits_support_attack() {
    let mut config = SimulationConfig::default();
    config.visible_monsters.clear();
    let mut session = SimulationSession::new(config);
    login_demo(&mut session);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });

    let spawn_packets = session.stage5_command(
        "event.spawn",
        vec!["YinDevilNode".to_string(), "1".to_string()],
    );
    let node_id = spawn_packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info } if info.name == "YinDevilNode" => {
                Some(info.object_id)
            }
            _ => None,
        })
        .or_else(|| {
            session
                .world_snapshot()
                .entities
                .iter()
                .find(|entity| entity.name == "YinDevilNode")
                .map(|entity| entity.object_id)
        })
        .expect("YinDevilNode should spawn in the QA-only simulation fixture");
    let origin = session
        .world_snapshot()
        .entities
        .iter()
        .find(|entity| entity.object_id == node_id)
        .map(|entity| (entity.x, entity.y))
        .expect("YinDevilNode position");

    let mut saw_support_attack = false;
    for _ in 0..8 {
        let packets = session.tick();
        saw_support_attack |= packets.iter().any(|packet| {
            matches!(
                packet,
                ServerPacket::ObjectAttack { info } if info.object_id == node_id
            )
        });
        assert!(!packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectRangeAttack { info } if info.object_id == node_id
        )));
        let snapshot = session.world_snapshot();
        let node = snapshot
            .entities
            .iter()
            .find(|entity| entity.object_id == node_id)
            .expect("YinDevilNode remains visible");
        assert_eq!((node.x, node.y), origin);
    }
    assert!(
        saw_support_attack,
        "friendly YinDevilNode should cast ObjectAttack"
    );
}

#[test]
fn yin_devil_node_never_forges_a_monster_target_buff() {
    let mut config = SimulationConfig::default();
    config.visible_monsters.clear();
    let mut session = SimulationSession::new(config);
    login_demo(&mut session);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let spawn_packets = session.stage5_command(
        "event.spawn",
        vec!["YinDevilNode".to_string(), "1".to_string()],
    );
    let node_id = spawn_packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::ObjectMonster { info } if info.name == "YinDevilNode" => {
                Some(info.object_id)
            }
            _ => None,
        })
        .expect("YinDevilNode spawn packet");
    for _ in 0..8 {
        for packet in session.tick() {
            assert!(
                !matches!(packet, ServerPacket::AddBuff { ref buff } if buff.object_id == node_id)
            );
        }
    }
}
