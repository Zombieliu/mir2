use mir2_protocol::{ClientPacket, MirDirection, Point, ServerPacket};
use mir2_simulation::{SimulationConfig, SimulationSession, VisibleNpcRecord};

fn started_storage_session() -> SimulationSession {
    let mut config = SimulationConfig::default();
    config.visible_npcs.push(VisibleNpcRecord {
        object_id: 4_991,
        name: "Warehouse Keeper".to_string(),
        image: 5,
        colour_argb: -1,
        position: Point { x: 331, y: 270 },
        direction: MirDirection::Left,
        quest_ids: Vec::new(),
        script_key: Some("BichonProvince/Warehouse-D002".to_string()),
    });
    let mut session = SimulationSession::new(config);
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    let _ = session.interact(4_991);
    let _ = session.select_npc_dialog_target("@Storage");
    session
}

#[test]
fn storage_v2_echoes_exact_request_ids_for_ack_and_nack() {
    let mut session = started_storage_session();

    let stored = session.handle_packet(ClientPacket::StoreItemV2 {
        request_id: "st-0000000000000001".to_string(),
        from: 2,
        to: 4,
    });
    assert!(stored.iter().any(|packet| matches!(
        packet,
        ServerPacket::StoreItemV2 {
            request_id,
            from: 2,
            to: 4,
            success: true,
        } if request_id == "st-0000000000000001"
    )));

    let rejected = session.handle_packet(ClientPacket::StoreItemV2 {
        request_id: "st-0000000000000002".to_string(),
        from: 40,
        to: 4,
    });
    assert_eq!(
        rejected,
        vec![ServerPacket::StoreItemV2 {
            request_id: "st-0000000000000002".to_string(),
            from: 40,
            to: 4,
            success: false,
        }]
    );

    let taken_back = session.handle_packet(ClientPacket::TakeBackItemV2 {
        request_id: "st-0000000000000003".to_string(),
        from: 4,
        to: 6,
    });
    assert!(taken_back.iter().any(|packet| matches!(
        packet,
        ServerPacket::TakeBackItemV2 {
            request_id,
            from: 4,
            to: 6,
            success: true,
        } if request_id == "st-0000000000000003"
    )));
}

#[test]
fn invalid_storage_request_id_fails_before_mutation() {
    let mut session = started_storage_session();
    let before = session.world_snapshot();

    let packets = session.handle_packet(ClientPacket::TakeBackItemV2 {
        request_id: "bad\nline".to_string(),
        from: 0,
        to: 6,
    });

    assert!(packets.is_empty());
    let after = session.world_snapshot();
    assert_eq!(before.inventory_items, after.inventory_items);
    assert_eq!(before.storage_items, after.storage_items);
}
