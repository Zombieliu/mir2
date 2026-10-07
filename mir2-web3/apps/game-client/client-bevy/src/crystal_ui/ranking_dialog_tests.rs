use super::*;

fn reply(kind: u8, count: i32, names: &[&str]) -> ServerPacket {
    ServerPacket::Rankings {
        rank_type: kind,
        my_rank: 2,
        count,
        listings: Vec::new(),
        listing_details: names
            .iter()
            .enumerate()
            .map(|(i, name)| RankCharacterInfo {
                player_id: 50 + i as i64,
                name: (*name).into(),
                level: 30,
                class: MirClass::Warrior,
            })
            .collect(),
    }
}

#[test]
fn filter_change_waits_for_old_untagged_reply_without_showing_old_rows() {
    let mut ui = RankingDialogUi::default();
    ui.show(10);
    assert!(ui.poll_request(10).is_some());
    ui.action(RankingAction::OnlineOnly, 20, &mut 0);
    assert!(ui.poll_request(21).is_none());
    assert!(ui.apply_packet(&reply(0, 1, &["old offline row"])));
    assert!(ui.rows.is_empty());
    assert!(matches!(
        ui.poll_request(21),
        Some(ClientPacket::GetRanking {
            online_only: true,
            rank_index: 0,
            ..
        })
    ));
    ui.apply_packet(&reply(0, 1, &["online row"]));
    assert_eq!(ui.rows[0].name, "online row");
}

#[test]
fn rapid_scroll_debounces_to_latest_page_and_inspect_waits_for_page_reply() {
    let mut ui = RankingDialogUi::default();
    ui.show(0);
    ui.poll_request(0);
    ui.apply_packet(&reply(0, 100, &["first"]));
    ui.move_rows(8, 100); // Crystal's direction-based wheel semantics.
    ui.move_rows(1, 200);
    assert_eq!(ui.query.row_offset, 2);
    assert!(ui.poll_request(699).is_none());
    assert!(matches!(
        ui.poll_request(700),
        Some(ClientPacket::GetRanking { rank_index: 2, .. })
    ));
    let mut inspect_until = 0;
    assert!(ui
        .action(RankingAction::Inspect(0), 800, &mut inspect_until)
        .is_none());
    ui.apply_packet(&reply(0, 100, &["third"]));
    assert!(matches!(
        ui.action(RankingAction::Inspect(0), 801, &mut inspect_until),
        Some(ClientPacket::Inspect {
            object_id: 50,
            ranking: true,
            hero: false
        })
    ));
    assert!(ui
        .action(RankingAction::Inspect(0), 900, &mut inspect_until)
        .is_none());
}

#[test]
fn empty_short_and_malformed_rankings_do_not_create_invalid_rows_or_scroll_nan() {
    let mut ui = RankingDialogUi::default();
    ui.show(1);
    ui.poll_request(1);
    assert!(!ui.apply_packet(&reply(5, 100, &["wrong class"])));
    ui.apply_packet(&reply(0, -20, &["invalid excess"]));
    assert!(ui.rows.is_empty());
    assert_eq!(ui.thumb_y(), 113.0);
    ui.drag_thumb(368.0, 2);
    assert_eq!(ui.query.row_offset, 0);
    ui.drag_thumb(f32::NAN, 3);
    assert!(ui.thumb_y().is_finite());
}

#[test]
fn hide_cancels_debounce_but_preserves_inflight_context_until_response() {
    let mut ui = RankingDialogUi::default();
    ui.show(0);
    ui.poll_request(0);
    ui.hide();
    ui.show(2);
    assert!(ui.poll_request(2).is_none());
    ui.apply_packet(&reply(0, 1, &["one"]));
    assert!(ui.poll_request(3).is_some());
    ui.hide();
    assert!(ui.poll_request(1000).is_none());
}

#[test]
fn unsent_release_matches_exact_request_and_retries_without_duplicate_inflight() {
    let mut ui = RankingDialogUi::default();
    ui.show(0);
    let sent = ui.poll_request(0).unwrap();
    ui.release_unsent(
        &ClientPacket::GetRanking {
            rank_type: 1,
            rank_index: 0,
            online_only: false,
        },
        1,
    );
    assert!(ui.poll_request(1).is_none());
    ui.release_unsent(&sent, 2);
    assert_eq!(ui.poll_request(2), Some(sent));
    assert!(ui.poll_request(3).is_none());
}

#[test]
fn thumb_endpoints_and_short_last_page_obey_authoritative_count() {
    let mut ui = RankingDialogUi::default();
    ui.show(0);
    ui.poll_request(0);
    ui.apply_packet(&reply(0, 25, &["one"]));
    ui.drag_thumb(368.0, 10);
    assert_eq!(ui.query.row_offset, 5);
    assert_eq!(ui.thumb_y(), 367.0);
    ui.poll_request(510);
    ui.apply_packet(&reply(0, 6, &["last", "excess"]));
    assert_eq!(ui.rows.len(), 1);
}

#[test]
fn ordinary_queue_pressure_cannot_evict_a_correlated_ranking_request() {
    let mut queue = NativePlayerUiIntentQueue::default();
    queue.push_intent(NativePlayerUiIntent::GetRanking {
        rank_type: 0,
        rank_index: 0,
        online_only: false,
    });
    for i in 0..MAX_QUEUED + 4 {
        queue.push_intent(NativePlayerUiIntent::Chat {
            message: i.to_string(),
        });
    }
    assert!(queue
        .drain_intents()
        .iter()
        .any(|i| matches!(i, NativePlayerUiIntent::GetRanking { .. })));
}

#[test]
fn shared_ranking_inspect_zero_id_keeps_expected_name_literal_flags_and_strict_clock() {
    let mut ui = RankingDialogUi::default();
    ui.show(0);
    ui.poll_request(0);
    ui.apply_packet(&reply(0, 1, &["zero player"]));
    ui.rows[0].player_id = 0;
    let mut ready_ms = 1000;
    let untouched = ui.player_inspect.clone();
    for now_ms in [999, 1000] {
        assert_eq!(ui.action(RankingAction::Inspect(0), now_ms, &mut ready_ms), None);
        assert_eq!(ready_ms, 1000);
        assert_eq!(ui.player_inspect, untouched);
    }
    assert_eq!(ui.action(RankingAction::Inspect(0), 1001, &mut ready_ms),
        Some(ClientPacket::Inspect { object_id: 0, ranking: true, hero: false }));
    assert_eq!(ready_ms, 1501);
    let mut expected = player_inspect::PlayerInspectUi::default();
    expected.request("zero player".into());
    assert_eq!(ui.player_inspect, expected);
    assert_eq!(ui.action(RankingAction::Inspect(0), 1501, &mut ready_ms), None);
    assert_eq!(ui.player_inspect, expected);
    assert_eq!(ready_ms, 1501);
    assert_eq!(ui.action(RankingAction::Inspect(0), 1502, &mut ready_ms),
        Some(ClientPacket::Inspect { object_id: 0, ranking: true, hero: false }));
    assert_eq!(ready_ms, 2002);
}

#[test]
fn shared_ranking_inspect_closed_loading_pending_missing_and_invalid_ids_do_not_mutate() {
    let mut loaded = RankingDialogUi::default();
    loaded.show(0);
    loaded.poll_request(0);
    loaded.apply_packet(&reply(0, 1, &["loaded"]));
    loaded.player_inspect.request("previous request".into());
    let mut cases = Vec::new();
    let mut closed = loaded.clone(); closed.hide(); cases.push((closed, 0));
    let mut loading = loaded.clone(); loading.due_ms = Some(1000); cases.push((loading, 0));
    let mut pending = loaded.clone(); pending.requested = Some(pending.query); cases.push((pending, 0));
    cases.push((loaded.clone(), 1));
    for player_id in [i64::MIN, -1, i64::from(u32::MAX) + 1, i64::MAX] {
        let mut invalid = loaded.clone(); invalid.rows[0].player_id = player_id;
        cases.push((invalid, 0));
    }
    for (mut ui, row) in cases {
        let previous = ui.clone(); let mut ready_ms = 100;
        assert_eq!(ui.action(RankingAction::Inspect(row), 1000, &mut ready_ms), None);
        assert_eq!(ui, previous);
        assert_eq!(ready_ms, 100);
    }
    loaded.rows[0].player_id = i64::from(u32::MAX);
    assert_eq!(loaded.action(RankingAction::Inspect(0), 1000, &mut 0),
        Some(ClientPacket::Inspect { object_id: u32::MAX, ranking: true, hero: false }));
}

#[test]
fn shared_ranking_inspect_native_overflow_saturates_without_reopening_the_clock() {
    let mut ui = RankingDialogUi::default();
    ui.show(0);
    ui.poll_request(0);
    ui.apply_packet(&reply(0, 1, &["last clock"]));
    let mut ready_ms = u64::MAX - 500;
    assert_eq!(ui.action(RankingAction::Inspect(0), u64::MAX - 499, &mut ready_ms),
        Some(ClientPacket::Inspect { object_id: 50, ranking: true, hero: false }));
    assert_eq!(ready_ms, u64::MAX);
    let previous = ui.clone();
    assert_eq!(ui.action(RankingAction::Inspect(0), u64::MAX, &mut ready_ms), None);
    assert_eq!(ui, previous);
    assert_eq!(ready_ms, u64::MAX);
}
