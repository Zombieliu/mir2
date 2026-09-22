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
        "fileName": "0", "title": "BichonProvince", "mapIndex": 1, "miniMap": 1,
    }));
    let source = json!({"mapFileName": "0", "mapTitle": "BichonProvince", "miniMapIndex": 1});
    cursor.observe_map_information(&json!({
        "fileName": "D401", "title": "DeadMineEntrance", "mapIndex": 401, "miniMap": 8,
    }));
    assert!(cursor.snapshot_is_from_previous_map(&source));
    let mut partial = json!({"mapTitle": "BichonProvince", "miniMapIndex": 1,
        "sceneView": {"center": {"x": 38, "y": 160}}});
    cursor.merge_into_same_map_snapshot(&mut partial);
    assert_eq!(partial["mapFileName"], json!("D401"));
    assert_eq!(partial["mapTitle"], json!("DeadMineEntrance"));
    assert_eq!(partial["mapIndex"], json!(401));
    assert_eq!(partial["miniMapIndex"], json!(8));
    assert_eq!(transform_map_model(&partial)["miniMapIndex"], json!(8));
    assert_eq!(
        transform_world_snapshot(&partial)["playerStats"]["mapName"],
        json!("DeadMineEntrance")
    );
}

#[test]
fn map_identity_missing_or_zero_minimap_cannot_reuse_previous_image_and_reconnect_resets() {
    let mut world = json!({"mapFileName": "0", "mapTitle": "BichonProvince", "miniMapIndex": 1,
        "mapMusic": 9, "bigMapIndex": 3});
    let mut cursor = NativeMapPacketCursor::default();
    cursor.observe_map_information(&json!({"fileName": "0", "miniMap": 1}));
    let destination = json!({"fileName": "D401", "title": "DeadMineEntrance"});
    assert!(apply_map_information_to_world_payload(
        &mut world,
        &destination
    ));
    assert!(world.get("miniMapIndex").is_none());
    assert!(world.get("mapMusic").is_none());
    cursor.observe_map_information(&destination);
    let mut partial = json!({"miniMapIndex": 1});
    cursor.merge_into_same_map_snapshot(&mut partial);
    assert!(partial.get("miniMapIndex").is_none());
    cursor.observe_map_information(&json!({"fileName": "D401", "miniMap": 0}));
    partial["miniMapIndex"] = json!(8);
    cursor.merge_into_same_map_snapshot(&mut partial);
    assert!(partial.get("miniMapIndex").is_none());
    cursor.reset();
    let mut reconnect =
        json!({"mapFileName": "0", "mapTitle": "BichonProvince", "miniMapIndex": 1});
    assert!(!cursor.snapshot_is_from_previous_map(&reconnect));
    cursor.merge_into_same_map_snapshot(&mut reconnect);
    assert_eq!(reconnect["mapFileName"], json!("0"));
    assert_eq!(reconnect["miniMapIndex"], json!(1));
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

    let source = json!({"type":"worldSnapshot","payload":{"mapFileName":"0","mapTitle":"BichonProvince","miniMapIndex":1,"playerObjectId":1,"entities":[{"objectId":1,"kind":"selfPlayer","x":320,"y":270}],"sceneView":{"center":{"x":320,"y":270}}}});
    assert_eq!(ingest!(source), WorldSnapshotIngestOutcome::Applied);
    ingest!(
        json!({"type":"packet","packet":"MapInformation","payload":{"mapIndex":401,"fileName":"D401","title":"DeadMineEntrance","miniMap":8}})
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
        let mut partial = json!({"mapTitle":"BichonProvince","miniMapIndex":1,"sceneView":{"center":{"x":30,"y":179}}});
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
