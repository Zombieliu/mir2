use super::*;
use mir2_client_bevy::quest_model::QuestReward;

#[test]
fn world_ingest_keeps_new_quest_item_info_for_rendered_rewards() {
    let context = GatewaySessionContext::default();
    let (shell_sender, _shell_receiver) = std::sync::mpsc::channel();
    let (gameplay_sender, gameplay_receiver) = std::sync::mpsc::channel();
    let mut snapshot_log_counter = 0;
    let mut gameplay_adapter = NativeGameplayAdapter::default();
    let mut last_world_payload = None;
    let mut last_wallet = None;
    let mut map_packet_cursor = NativeMapPacketCursor::default();
    let mut ui_cursor = NativeUiPlayerCursor {
        level: Some(20),
        class_name: Some("Warrior".to_owned()),
        ..Default::default()
    };
    let mut in_flight_claim_mail_id = None;
    let mut send_mail_in_flight = false;
    let mut pending_mail_feedback = VecDeque::new();
    let mut skill_cursor = SkillPacketCursor::default();
    let mut social_cursor = SocialModel::default();
    let mut push_world_state = |_: String| true;
    let mut ingest = |envelope: Value| {
        handle_gateway_text_with_world_ingest(
            &envelope.to_string(),
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
        .expect("valid gateway envelope")
    };

    let fencing_info = json!({
        "index": 973, "name": "Fencing", "image": 3640,
        "item_type": 13, "durability": 1000, "slots": 0,
        "class_based": true, "level_based": true,
        "stats": [{"stat": 12, "value": 7}]
    });
    assert_eq!(
        ingest(json!({
            "type": "packet", "packet": "NewQuestInfo",
            "payload": {
                "id": 3,
                "info": {"index": 3, "rewards_fixed_item": [
                    {"item": fencing_info, "count": 1}
                ]},
                "rewards": {"items": [
                    {"itemIndex": 973, "name": "Fencing", "icon": 3640, "count": 1}
                ]}
            }
        })),
        WorldSnapshotIngestOutcome::NotSnapshot
    );
    // An unknown catalogue index without a concrete wire ItemInfo must stay
    // source-less; the adapter must not invent an owned item or tooltip.
    assert_eq!(
        ingest(json!({
            "type": "packet", "packet": "NewQuestInfo",
            "payload": {
                "id": 4,
                "info": {"index": 4, "rewards_fixed_item": [{}]},
                "rewards": {"items": [
                    {"itemIndex": 1_000_000, "name": "Unknown", "count": 1}
                ]}
            }
        })),
        WorldSnapshotIngestOutcome::NotSnapshot
    );
    assert_eq!(
        ingest(json!({
            "type": "worldSnapshot",
            "payload": {"questLog": [
                {"questId": 3, "stage": "available"},
                {"questId": 4, "stage": "available"}
            ]}
        })),
        WorldSnapshotIngestOutcome::Applied
    );

    let snapshot = gameplay_receiver
        .try_recv()
        .expect("world-ingest gameplay snapshot");
    let fencing = snapshot
        .quests
        .active_quests
        .iter()
        .find(|quest| quest.quest_index == 3)
        .expect("Fencing quest");
    let Some(QuestReward::Item {
        tooltip_source: Some(source),
        ..
    }) = fencing.rewards.first()
    else {
        panic!("world-ingest must deliver the enriched reward");
    };
    assert_eq!(source.info.item_index, 973);
    assert_eq!(source.info.image, 3640);
    assert_eq!(
        (source.info.class_based, source.info.level_based),
        (true, true)
    );
    assert_eq!(
        (source.info.stats[0].stat, source.info.stats[0].value),
        (12, 7)
    );
    let preview = source
        .user_item
        .as_ref()
        .expect("QuestCell preview UserItem");
    assert_eq!(
        (preview.unique_id, preview.item_index, preview.count),
        (0, 973, 0)
    );
    assert!(!preview.identified);

    let unknown = snapshot
        .quests
        .active_quests
        .iter()
        .find(|quest| quest.quest_index == 4)
        .expect("unknown quest");
    assert!(matches!(
        unknown.rewards.first(),
        Some(QuestReward::Item {
            tooltip_source: None,
            ..
        })
    ));
}
