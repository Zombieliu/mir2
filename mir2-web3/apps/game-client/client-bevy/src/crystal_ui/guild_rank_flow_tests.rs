//! Ordinary UI helpers and real typed packet cursor; no GPU/human acceptance.
use super::*;
use crate::social::{guild_rank_native_tests as wire, SocialModel, SocialPendingOperation};
use std::time::Duration;

fn fixture(name: &str) -> (NativePlayerUiState, SocialModel, NativePlayerUiIntentQueue) {
    let mut state = NativePlayerUiState::default();
    state.guild_panel.guild_name = Some("Rank Guild".into());
    state.guild_panel.owner_name = "Leader".into();
    state.guild_panel.now_ms = 100;
    state.selected_guild_rank = Some(2);
    state.guild_rank_name_draft = name.into();
    (state, wire::seeded(0), NativePlayerUiIntentQueue::default())
}

fn process_hidden(state: NativePlayerUiState, social: SocialModel,
    queue: NativePlayerUiIntentQueue, now_ms: u64) -> App {
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_millis(now_ms));
    let mut app = App::new();
    app.insert_resource(state).insert_resource(social).insert_resource(queue)
        .insert_resource(GuildHost::default()).insert_resource(time)
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(Update, process);
    app.update();
    app
}

fn submit(state: &mut NativePlayerUiState, social: &mut SocialModel,
    queue: &mut NativePlayerUiIntentQueue) {
    save_rank_name(state, social, queue);
    assert!(matches!(&queue.drain_intents()[..],
        [NativePlayerUiIntent::GuildEditMember { change_type: 3, rank_index: 2,
            name, rank_name }] if name.is_empty() && rank_name == &state.guild_rank_name_draft));
}

#[test]
fn guild_rank_matching_typed_terminal_releases_exact_pending_and_editor_before_deadline() {
    let (mut state, mut local, mut queue) = fixture("Veterans");
    let mut network = local.clone();
    let unrelated = SocialPendingOperation::GroupAdd { name: "Friend".into() };
    local.begin_pending(unrelated.clone());
    submit(&mut state, &mut local, &mut queue);
    assert_eq!(local.pending.len(), 2);
    assert!(wire::apply_typed(&mut network, wire::terminal("Leader", 2, "Veterans")));
    local.apply_authoritative(network);
    assert_eq!(local.pending, vec![unrelated.clone()], "typed 7 uses nested rank, not absent changeType");
    let mut app = process_hidden(state, local, queue, 200);
    {
        let state = app.world().resource::<NativePlayerUiState>();
        assert_eq!(state.guild_panel.rank_name_ready_ms, 0, "ACK retires editor wait");
        assert_eq!(state.guild_rank_name_draft, "Veterans");
    }
    let mut state = app.world_mut().remove_resource::<NativePlayerUiState>().unwrap();
    let mut local = app.world_mut().remove_resource::<SocialModel>().unwrap();
    let mut queue = app.world_mut().remove_resource::<NativePlayerUiIntentQueue>().unwrap();
    state.guild_rank_name_draft = "Explorers".into();
    submit(&mut state, &mut local, &mut queue);
    assert!(local.pending.contains(&unrelated));
}

#[test]
fn guild_rank_metadata_255_matching_text_never_completes_request() {
    let (mut state, mut local, mut queue) = fixture("Veterans");
    let mut network = local.clone();
    submit(&mut state, &mut local, &mut queue);
    let pending = local.pending.clone();
    assert!(wire::apply_typed(&mut network, wire::full("Veterans")));
    local.apply_authoritative(network);
    assert_eq!(local.pending, pending, "canonical state is not a source terminal");
    let app = process_hidden(state, local, queue, 200);
    assert_eq!(app.world().resource::<NativePlayerUiState>().guild_panel.rank_name_ready_ms, 5100);
}

#[test]
fn guild_rank_foreign_actor_wrong_rank_or_text_does_not_release_request() {
    for packet in [wire::terminal("Officer", 2, "Veterans"),
        wire::terminal("Leader", 1, "Officer"), wire::terminal("Leader", 2, "Scouts")] {
        let (mut state, mut local, mut queue) = fixture("Veterans");
        let mut network = local.clone();
        submit(&mut state, &mut local, &mut queue);
        let pending = local.pending.clone();
        assert!(wire::apply_typed(&mut network, packet));
        local.apply_authoritative(network);
        assert_eq!(local.pending, pending);
        let app = process_hidden(state, local, queue, 200);
        assert_eq!(app.world().resource::<NativePlayerUiState>().guild_rank_name_draft, "Veterans");
    }
}

#[test]
fn guild_rank_hidden_timeout_is_strictly_after_five_seconds_and_keeps_other_pending() {
    for now_ms in [5099, 5100, 5101] {
        let (mut state, mut local, mut queue) = fixture("Veterans");
        let unrelated = SocialPendingOperation::GuildNotice { notice: vec!["memo".into()] };
        local.begin_pending(unrelated.clone());
        submit(&mut state, &mut local, &mut queue);
        let mut app = process_hidden(state, local, queue, now_ms);
        let social = app.world().resource::<SocialModel>();
        assert_eq!(social.pending.len(), if now_ms > 5100 { 1 } else { 2 }, "hidden panel must retire only timed-out rename at {now_ms}");
        assert!(social.pending.contains(&unrelated));
        assert_eq!(app.world().resource::<NativePlayerUiState>().guild_rank_name_draft, "Veterans");
        assert_eq!(social.guild.ranks[2].name, "Scouts", "timeout never renames optimistically");
        if now_ms > 5100 {
            let mut state = app.world_mut().remove_resource::<NativePlayerUiState>().unwrap();
            let mut local = app.world_mut().remove_resource::<SocialModel>().unwrap();
            let mut queue = app.world_mut().remove_resource::<NativePlayerUiIntentQueue>().unwrap();
            submit(&mut state, &mut local, &mut queue);
        }
    }
}

#[test]
fn guild_rank_same_name_source_terminal_still_completes() {
    let (mut state, mut local, mut queue) = fixture("Scouts");
    let mut network = local.clone();
    submit(&mut state, &mut local, &mut queue);
    assert!(wire::apply_typed(&mut network, wire::terminal("Leader", 2, "Scouts")));
    local.apply_authoritative(network);
    assert!(local.pending.is_empty(), "a source reply need not change the old text");
}

#[test]
fn guild_rank_leave_and_rejoin_same_guild_scope_revokes_old_rename_only() {
    let (mut state, mut local, mut queue) = fixture("Veterans");
    let mut network = local.clone();
    let unrelated = SocialPendingOperation::GroupAdd { name: "Friend".into() };
    local.begin_pending(unrelated.clone());
    submit(&mut state, &mut local, &mut queue);
    assert!(wire::apply_typed(&mut network, wire::status("", "", -1)));
    assert!(wire::apply_typed(&mut network, wire::status("Rank Guild", "Leader", 0)));
    assert!(wire::apply_typed(&mut network, wire::full("Scouts")));
    local.apply_authoritative(network);
    assert_eq!(local.pending, vec![unrelated], "same display name does not preserve old membership context");
    assert_eq!(local.guild.ranks[2].name, "Scouts");
}

#[test]
fn guild_rank_scene_retains_pending_and_session_reset_retires_it() {
    let (mut state, mut local, mut queue) = fixture("Veterans");
    submit(&mut state, &mut local, &mut queue);
    let pending = local.pending.clone();
    local.clear_scene();
    assert_eq!(local.pending, pending);
    assert_eq!(local.guild.name.as_deref(), Some("Rank Guild"));
    local.clear_session();
    assert!(local.pending.is_empty());
    let app = process_hidden(state, local, queue, 200);
    assert_eq!(app.world().resource::<NativePlayerUiState>().guild_panel.rank_name_ready_ms, 0);
}

#[test]
fn guild_rank_full_queue_refuses_before_pending_and_input_follows_utf16_admission() {
    let (mut state, mut local, mut queue) = fixture("Veterans");
    for _ in 0..MAX_QUEUED { assert!(queue.push_intent(NativePlayerUiIntent::GuildBuffUpdate(GuildBuffRequest { action: 1, id: 9 }))); }
    let before = local.clone();
    save_rank_name(&mut state, &mut local, &mut queue);
    assert_eq!(local, before);
    assert_eq!(state.guild_panel.rank_name_ready_ms, 0);
    for name in ["ab".to_owned(), "bad\\rank".to_owned(), "😀".repeat(11)] {
        let (mut state, mut local, mut queue) = fixture(&name);
        save_rank_name(&mut state, &mut local, &mut queue);
        assert!(queue.drain_intents().is_empty(), "invalid source rank admitted: {name:?}");
        assert!(local.pending.is_empty());
    }
    for name in [" A ".to_owned(), "   ".to_owned(), "😀".repeat(10)] {
        let (mut state, mut local, mut queue) = fixture(&name);
        submit(&mut state, &mut local, &mut queue);
    }
}

#[test]
fn guild_rank_cached_old_source_projection_does_not_ack_a_new_request() {
    let (mut state, mut local, mut queue) = fixture("Veterans");
    let mut network = local.clone();
    assert!(wire::apply_typed(&mut network, wire::terminal("Leader", 2, "Veterans")));
    local.apply_authoritative(network.clone());
    submit(&mut state, &mut local, &mut queue);
    let pending = local.pending.clone();
    local.apply_authoritative(network);
    assert_eq!(local.pending, pending, "cached proof is not a new source arrival");
}

#[test]
fn guild_rank_nonrename_status_does_not_ack_matching_text() {
    for status in [6, 8, 255] {
        let (mut state, mut local, mut queue) = fixture("Veterans");
        let mut network = local.clone();
        submit(&mut state, &mut local, &mut queue);
        let pending = local.pending.clone();
        let mut packet = wire::terminal("Leader", 2, "Veterans");
        if let mir2_protocol::ServerPacket::GuildMemberChange { status: value, .. } = &mut packet {
            *value = status;
        }
        assert!(wire::apply_typed(&mut network, packet));
        local.apply_authoritative(network);
        assert_eq!(local.pending, pending);
    }
}

#[test]
fn guild_rank_editor_releases_after_terminal_then_peer_delta_in_same_frame() {
    let (mut state, mut local, mut queue) = fixture("Veterans");
    let mut network = local.clone();
    submit(&mut state, &mut local, &mut queue);
    assert!(wire::apply_typed(&mut network, wire::terminal("Leader", 2, "Veterans")));
    local.apply_authoritative(network.clone());
    assert!(wire::apply_typed(&mut network, wire::terminal("Officer", 1, "Officer")));
    local.apply_authoritative(network);
    let app = process_hidden(state, local, queue, 200);
    let state = app.world().resource::<NativePlayerUiState>();
    assert_eq!(state.guild_panel.rank_name_ready_ms, 0);
    assert!(state.guild_panel.rank_name_submission.is_none());
    assert_eq!(state.guild_rank_name_draft, "Veterans");
    assert!(!state.guild_panel.rank_name_unconfirmed);
}

#[test]
fn guild_rank_new_membership_editor_keeps_actual_owner_and_clock_for_first_command() {
    let (mut state, social, queue) = fixture("Veterans");
    state.guild_panel.guild_name = Some("Old Guild".into());
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_millis(200));
    let mut ui = crate::read_model::UiReadModel::default();
    ui.player.name = Some("Leader".into());
    let mut app = App::new();
    app.insert_resource(state).insert_resource(social).insert_resource(queue)
        .insert_resource(GuildHost::default()).insert_resource(time).insert_resource(ui)
        .add_message::<bevy::input::mouse::MouseWheel>().add_systems(Update, process);
    app.update();
    let state = app.world().resource::<NativePlayerUiState>();
    assert_eq!(state.guild_panel.owner_name, "Leader");
    assert_eq!(state.guild_panel.now_ms, 200);
    assert_eq!(state.guild_panel.guild_name.as_deref(), Some("Rank Guild"));
}
