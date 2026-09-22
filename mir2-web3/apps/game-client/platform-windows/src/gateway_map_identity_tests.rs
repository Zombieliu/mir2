use super::*;

#[test]
fn map_identity_stale_snapshot_receipts_do_not_project_actors() {
    let adapter = NativeGameplayAdapter::default();
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut skills = SkillPacketCursor::default();
    let mut old = json!({"mapFileName":"0", "entities":[{"objectId":99,"kind":"monster"}],
        "playerObjectId":1, "knownSkills":[],
        "questOperationAck":{"operation":"acceptQuest","requestId":"qs-map-test","npcIndex":9,"questIndex":44,"success":true},
        "skillKeyAck":{"requestId":73,"spell":"FireBall","key":16,"oldKey":0,"accepted":true}});
    forward_stale_map_receipts(&mut old, &adapter, &sender, &mut skills).unwrap();
    let receipt = receiver.try_recv().unwrap();
    assert!(receipt.big_map_only);
    assert!(receipt.quest_operation_ack.is_some());
    assert!(receipt.entity_render_payload.is_none());
    assert!(receipt.zone_entity_tiles.is_empty());
    assert_eq!(
        skills.snapshot_serial, 1,
        "player skill receipt passed through authority observation"
    );
    assert!(receiver.try_recv().is_err());
    let mut hero = json!({"mapFileName":"0", "playerObjectId":1,
        "stage5Systems":{"heroLearnedMagics":[{"spell":"FireBall","key":24}]},
        "skillKeyAck":{"requestId":74,"spell":"FireBall","key":24,"oldKey":0,"accepted":true}});
    forward_stale_map_receipts(&mut hero, &adapter, &sender, &mut skills).unwrap();
    assert_eq!(skills.hero.skill_key_ack.as_ref().unwrap().request_id, 74);
    assert!(
        receiver.try_recv().is_err(),
        "hero receipt must not produce a gameplay/world snapshot"
    );
    let mut invalid = json!({"questOperationAck":{"operation":"acceptQuest"}});
    assert!(forward_stale_map_receipts(&mut invalid, &adapter, &sender, &mut skills).is_err());
}

#[test]
fn map_identity_packet_replaces_source_metadata_before_partial_snapshot() {
    let mut cursor = NativeMapPacketCursor::default();
    cursor.observe_map_information(&json!({
        "fileName": "0", "title": "BichonProvince", "mapIndex": 1, "miniMap": 101,
    }));
    let source = json!({"mapFileName": "0", "mapTitle": "BichonProvince", "miniMapIndex": 101});
    cursor.observe_map_information(&json!({
        "fileName": "D401", "title": "DeadMineEntrance", "mapIndex": 47, "miniMap": 8,
    }));
    assert!(cursor.snapshot_is_from_previous_map(&source));
    let mut partial = json!({"mapTitle": "BichonProvince", "miniMapIndex": 101,
        "sceneView": {"center": {"x": 38, "y": 160}}});
    cursor.merge_into_same_map_snapshot(&mut partial);
    assert_eq!(partial["mapFileName"], json!("D401"));
    assert_eq!(partial["mapTitle"], json!("DeadMineEntrance"));
    assert_eq!(partial["mapIndex"], json!(47));
    assert_eq!(partial["miniMapIndex"], json!(8));
    assert_eq!(transform_map_model(&partial)["miniMapIndex"], json!(8));
    assert_eq!(
        transform_world_snapshot(&partial)["playerStats"]["mapName"],
        json!("DeadMineEntrance")
    );
}

#[test]
fn map_identity_missing_or_zero_minimap_cannot_reuse_previous_image_and_reconnect_resets() {
    let mut world = json!({"mapFileName": "0", "mapTitle": "BichonProvince", "miniMapIndex": 101,
        "mapMusic": 9, "bigMapIndex": 3});
    let mut cursor = NativeMapPacketCursor::default();
    cursor.observe_map_information(&json!({"fileName": "0", "miniMap": 101}));
    let destination = json!({"fileName": "D401", "title": "DeadMineEntrance"});
    assert!(apply_map_information_to_world_payload(
        &mut world,
        &destination
    ));
    assert!(world.get("miniMapIndex").is_none());
    assert!(world.get("mapMusic").is_none());
    cursor.observe_map_information(&destination);
    let mut partial = json!({"miniMapIndex": 101});
    cursor.merge_into_same_map_snapshot(&mut partial);
    assert!(partial.get("miniMapIndex").is_none());
    cursor.observe_map_information(&json!({"fileName": "D401", "miniMap": 0}));
    partial["miniMapIndex"] = json!(8);
    cursor.merge_into_same_map_snapshot(&mut partial);
    assert!(partial.get("miniMapIndex").is_none());
    cursor.reset();
    let mut reconnect =
        json!({"mapFileName": "0", "mapTitle": "BichonProvince", "miniMapIndex": 101});
    assert!(!cursor.snapshot_is_from_previous_map(&reconnect));
    cursor.merge_into_same_map_snapshot(&mut reconnect);
    assert_eq!(reconnect["mapFileName"], json!("0"));
    assert_eq!(reconnect["miniMapIndex"], json!(101));
}

#[test]
fn map_identity_information_only_transfer_rejects_source_but_keeps_receipts() {
    let context = GatewaySessionContext::default();
    let (shell_sender, _shell_receiver) = std::sync::mpsc::channel();
    let (gameplay_sender, gameplay_receiver) = std::sync::mpsc::channel();
    let mut snapshot_log_counter = 0;
    let mut gameplay_adapter = NativeGameplayAdapter::default();
    let mut last_world_payload = None;
    let mut last_wallet = None;
    let mut map_packet_cursor = NativeMapPacketCursor::default();
    let mut ui_cursor = NativeUiPlayerCursor::default();
    let mut in_flight_claim_mail_id = None;
    let mut send_mail_in_flight = false;
    let mut pending_mail_feedback = VecDeque::new();
    let mut skill_cursor = SkillPacketCursor::default();
    let mut social_cursor = SocialModel::default();
    let mut push_world_state = |_: String| true;
    macro_rules! ingest {
        ($envelope:expr) => {{
            handle_gateway_text_with_world_ingest(
                &$envelope.to_string(),
                &mut snapshot_log_counter,
                &context,
                &shell_sender,
                &mut gameplay_adapter,
                &gameplay_sender,
                &mut last_world_payload,
                &mut last_wallet,
                &mut map_packet_cursor,
                &mut ui_cursor,
                &mut in_flight_claim_mail_id,
                &mut send_mail_in_flight,
                &mut pending_mail_feedback,
                &mut skill_cursor,
                &mut social_cursor,
                &mut push_world_state,
            )
            .expect("gateway event")
        }};
    }

    let source = json!({"type":"worldSnapshot","payload":{"mapFileName":"0","mapTitle":"BichonProvince","miniMapIndex":101,"playerObjectId":1,"entities":[{"objectId":1,"kind":"selfPlayer","x":320,"y":270}],"sceneView":{"center":{"x":320,"y":270}}}});
    assert_eq!(ingest!(source), WorldSnapshotIngestOutcome::Applied);
    ingest!(
        json!({"type":"packet","packet":"MapInformation","payload":{"mapIndex":47,"fileName":"D401","title":"DeadMineEntrance","miniMap":8}})
    );
    ingest!(
        json!({"type":"packet","packet":"UserLocation","payload":{"x":38,"y":160,"direction":"Down"}})
    );
    assert_eq!(
        last_world_payload.as_ref().unwrap()["mapFileName"],
        json!("D401")
    );
    assert_eq!(
        last_world_payload.as_ref().unwrap()["miniMapIndex"],
        json!(8)
    );
    while gameplay_receiver.try_recv().is_ok() {}
    let stale = json!({"type":"worldSnapshot","payload":{"mapFileName":"0","mapTitle":"BichonProvince","entities":[{"objectId":99,"kind":"monster"}],"questOperationAck":{"operation":"acceptQuest","requestId":"qs-map-test","npcIndex":9,"questIndex":44,"success":true}}});
    assert_eq!(ingest!(stale), WorldSnapshotIngestOutcome::NotSnapshot);
    let receipt = gameplay_receiver.try_recv().unwrap();
    assert!(receipt.big_map_only && receipt.quest_operation_ack.is_some());
    assert!(receipt.entity_render_payload.is_none());
    assert_eq!(
        last_world_payload.as_ref().unwrap()["mapFileName"],
        json!("D401")
    );
    for file in [None, Some("D401")] {
        let mut partial = json!({"mapTitle":"BichonProvince","miniMapIndex":101,"sceneView":{"center":{"x":30,"y":179}}});
        if let Some(file) = file {
            partial["mapFileName"] = json!(file);
        }
        assert_eq!(
            ingest!(json!({"type":"worldSnapshot","payload":partial})),
            WorldSnapshotIngestOutcome::Applied
        );
        let world = last_world_payload.as_ref().unwrap();
        assert_eq!(world["mapTitle"], json!("DeadMineEntrance"));
        assert_eq!(world["miniMapIndex"], json!(8));
        assert_eq!(
            transform_world_snapshot(world)["playerStats"]["mapName"],
            json!("DeadMineEntrance")
        );
    }
}

#[test]
fn actual_d401_catalog_map_information_and_snapshot_keep_minimap_and_quest_identity() {
    use crate::native_protocol::{parse_inbound_event, InboundEvent};

    fn observe(adapter: &mut NativeGameplayAdapter, envelope: &str) {
        let InboundEvent::Packet(packet) =
            parse_inbound_event(envelope).expect("actual map packet envelope")
        else {
            panic!("expected packet envelope");
        };
        adapter.observe_packet(&packet);
    }

    let manifest = mir2_game_data::crystal_respawn_manifest_ref();
    let map = |file_name: &str| {
        manifest
            .maps
            .iter()
            .find(|map| map.map_file_name == file_name)
            .expect("imported Crystal map")
    };
    assert_eq!((map("0").map_index, map("0").mini_map), (1, 101));
    assert_eq!((map("D001").map_index, map("D001").mini_map), (39, 1));
    assert_eq!((map("D401").map_index, map("D401").mini_map), (47, 8));

    let mut adapter = NativeGameplayAdapter::default();
    adapter.observe_world_snapshot(&json!({
        "mapIndex": 1,
        "mapFileName": "0",
        "mapTitle": "BichonProvince",
        "miniMapIndex": 101,
        "playerObjectId": 1000,
        "entities": [{"objectId": 1000, "kind": "selfPlayer", "x": 659, "y": 215}],
    }));
    observe(
        &mut adapter,
        r#"{"type":"packet","packet":"WorldMapSetup","payload":{"setup":{"enabled":true,"icons":[{"imageIndex":101,"title":"BichonProvince","mapIndex":1},{"imageIndex":8,"title":"DeadMineEntrance","mapIndex":47}]},"teleportToNpcCost":3000}}"#,
    );
    observe(
        &mut adapter,
        r#"{"type":"packet","packet":"NewMapInfo","payload":{"mapIndex":1,"info":{"title":"BichonProvince","width":960,"height":640,"bigMap":101,"movements":[],"npcs":[]}}}"#,
    );
    observe(
        &mut adapter,
        r#"{"type":"packet","packet":"MapInformation","payload":{"mapIndex":47,"fileName":"D401","title":"DeadMineEntrance","miniMapIndex":8,"bigMapIndex":8}}"#,
    );
    // Transfer invalidates map definitions containing old scene NPC object IDs.
    // The normal GetMapInfo response repopulates the destination after transfer.
    observe(
        &mut adapter,
        r#"{"type":"packet","packet":"NewMapInfo","payload":{"mapIndex":47,"info":{"title":"DeadMineEntrance","width":200,"height":200,"bigMap":8,"movements":[],"npcs":[]}}}"#,
    );
    observe(
        &mut adapter,
        r#"{"type":"packet","packet":"UserLocation","payload":{"x":24,"y":182,"direction":"Down"}}"#,
    );

    let big_map = adapter.big_map_snapshot().big_map;
    let current = big_map.current_map().expect("D401 map definition");
    assert_eq!(big_map.current_map_index, Some(47));
    assert_eq!(big_map.active_map_index, Some(47));
    assert_eq!(current.info.title, "DeadMineEntrance");
    assert_eq!(current.info.big_map, 8);
    assert_eq!(
        big_map.player_location,
        Some(mir2_client_bevy::big_map::BigMapPoint { x: 24, y: 182 })
    );

    let map_model = transform_map_model(&json!({
        "mapFileName": "D401", "miniMapIndex": 8,
        "sceneView": {"center": {"x": 24, "y": 182}},
    }));
    assert_eq!(map_model["miniMapIndex"], json!(8));
    let targets = mir2_client_bevy::quest_destination::authored_target_map_indices(2_110_013);
    assert_eq!(targets, vec![47], "imported D401 quest target");
    assert!(
        targets.contains(&big_map.current_map_index.unwrap()),
        "the current D401 identity must replace Bichon before quest routing"
    );
}
