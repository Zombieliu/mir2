//! Actual Native permission queue and overlay handler; no window, transport or local success.
use super::*;
use crate::social::{
    guild_rank_native_tests as wire, GuildRankPermissionRequest, SocialModel,
    SocialPendingOperation, MAX_SOCIAL_PENDING,
};
use mir2_protocol::ServerPacket;
use serde_json::json;

fn fixture() -> (SocialModel, NativePlayerUiIntentQueue) {
    let mut social = wire::seeded(0);
    assert!(social.apply_network_packet(
        "UserInformation",
        &json!({"info":{"name":"Leader","realId":1}}),
    ));
    let viewer = social
        .viewer
        .as_ref()
        .expect("ordinary UserInformation establishes viewer");
    assert_eq!(viewer.name, "Leader");
    assert_eq!(viewer.character_index, 1);
    assert!(social.guild.roster_complete && !social.guild.roster_needs_refresh);
    (social, NativePlayerUiIntentQueue::default())
}

fn request(social: &SocialModel, option: u8) -> GuildRankPermissionRequest {
    social
        .guild_rank_permission_request("Leader", 2, option, true)
        .expect("real complete editable rank")
}

fn typed_intent(request: &GuildRankPermissionRequest) -> NativePlayerUiIntent {
    NativePlayerUiIntent::GuildRankPermission(request.clone())
}

fn rank_packet(status: u8, actor: &str, options: u8) -> ServerPacket {
    let mut ranks = wire::full_ranks("Scouts");
    ranks[2].options = options;
    ServerPacket::GuildMemberChange {
        name: actor.into(),
        rank_index: 0,
        status,
        ranks: if status == 255 {
            ranks
        } else {
            vec![ranks.remove(2)]
        },
    }
}

#[test]
fn guild_permission_flow_enqueues_only_typed_context_without_optimistic_permissions() {
    let (mut social, mut queue) = fixture();
    let before = social.guild.clone();
    let request = request(&social, 1);
    assert_eq!(request.guild_name, "Rank Guild");
    assert_eq!(request.scope_epoch, social.guild.scope_epoch);
    assert_eq!(request.requester.character_index, 1);
    assert!(queue.push_guild_rank_permission_pending(&mut social, request.clone()));
    assert_eq!(
        social.guild, before,
        "queue admission cannot change rank options, wallet or membership"
    );
    assert_eq!(
        social.pending,
        vec![SocialPendingOperation::GuildRankPermission(request.clone())]
    );
    assert_eq!(queue.drain_intents(), vec![typed_intent(&request)]);
    assert_eq!(social.guild.ranks[2].options, 1);
    assert_eq!(social.guild.my_options, 255);
}

#[test]
fn guild_permission_flow_generic_social_entry_rejects_both_naked_type5_and_typed_variant() {
    let (mut social, mut queue) = fixture();
    let request = request(&social, 1);
    for intent in [
        NativePlayerUiIntent::GuildEditMember {
            change_type: 5,
            rank_index: 2,
            name: "true".into(),
            rank_name: "1".into(),
        },
        typed_intent(&request),
    ] {
        assert!(!queue.push_social_pending(&mut social, intent));
        assert!(social.pending.is_empty());
        assert!(queue.drain_intents().is_empty());
    }
    assert!(queue.push_guild_rank_permission_pending(&mut social, request.clone()));
    assert_eq!(queue.drain_intents(), vec![typed_intent(&request)]);
}

#[test]
fn guild_permission_flow_same_rank_bit_stays_unique_after_queue_drain_and_opposite_full_projection()
{
    let (mut social, mut queue) = fixture();
    let first = request(&social, 1);
    assert!(queue.push_guild_rank_permission_pending(&mut social, first.clone()));
    assert!(!queue.push_guild_rank_permission_pending(&mut social, first.clone()));
    assert_eq!(queue.drain_intents(), vec![typed_intent(&first)]);
    assert!(
        !queue.push_guild_rank_permission_pending(&mut social, first.clone()),
        "drain is not the server receipt"
    );
    assert!(wire::apply_typed(&mut social, rank_packet(255, "", 3)));
    assert_eq!(
        social.pending,
        vec![SocialPendingOperation::GuildRankPermission(first.clone())],
        "255 cannot ACK an input"
    );
    let opposite = social
        .guild_rank_permission_request("Leader", 2, 1, false)
        .expect("server bit now permits a distinct opposite request");
    assert_ne!(opposite, first);
    assert!(
        !queue.push_guild_rank_permission_pending(&mut social, opposite),
        "one unresolved key per scope/rank/bit, not per desired bool"
    );
    assert_eq!(social.pending.len(), 1);
    assert!(queue.drain_intents().is_empty());
}

#[test]
fn guild_permission_flow_other_bits_are_independent_but_keep_the_original_server_options() {
    let (mut social, mut queue) = fixture();
    let before = social.guild.clone();
    let first = request(&social, 1);
    let second = request(&social, 2);
    assert!(queue.push_guild_rank_permission_pending(&mut social, first.clone()));
    assert!(queue.push_guild_rank_permission_pending(&mut social, second.clone()));
    assert_eq!(
        social.pending,
        vec![
            SocialPendingOperation::GuildRankPermission(first.clone()),
            SocialPendingOperation::GuildRankPermission(second.clone())
        ]
    );
    assert_eq!(
        queue.drain_intents(),
        vec![typed_intent(&first), typed_intent(&second)]
    );
    assert_eq!(social.guild, before);
}

#[test]
fn guild_permission_flow_uses_the_same_finite_eight_budget_as_other_social_operations() {
    assert_eq!(MAX_SOCIAL_PENDING, 8);
    for occupied in [7, 8] {
        let (mut social, mut queue) = fixture();
        let first = request(&social, 1);
        let second = request(&social, 2);
        for index in 0..occupied {
            assert!(social.begin_pending(SocialPendingOperation::GroupAdd {
                name: format!("Other{index}")
            }));
        }
        let unrelated = social.pending.clone();
        assert_eq!(
            queue.push_guild_rank_permission_pending(&mut social, first.clone()),
            occupied == 7
        );
        assert_eq!(social.pending.len(), MAX_SOCIAL_PENDING);
        assert_eq!(&social.pending[..occupied], unrelated.as_slice());
        assert!(!queue.push_guild_rank_permission_pending(&mut social, second));
        assert_eq!(
            social.pending.len(),
            MAX_SOCIAL_PENDING,
            "rejected ninth operation leaves no reservation"
        );
        assert_eq!(
            queue.drain_intents(),
            if occupied == 7 {
                vec![typed_intent(&first)]
            } else {
                vec![]
            }
        );
    }
}

#[test]
fn guild_permission_flow_full_transport_queue_rejects_before_creating_pending() {
    let (mut social, mut queue) = fixture();
    let request = request(&social, 1);
    for _ in 0..MAX_QUEUED {
        assert!(
            queue.push_intent(NativePlayerUiIntent::GuildBuffUpdate(GuildBuffRequest {
                action: 1,
                id: 9
            }))
        );
    }
    let before = social.clone();
    assert!(!queue.push_guild_rank_permission_pending(&mut social, request));
    assert_eq!(social, before);
    let queued = queue.drain_intents();
    assert_eq!(
        queued.len(),
        MAX_QUEUED,
        "no older item is evicted to create a stranded social request"
    );
    assert!(queued
        .iter()
        .all(|intent| matches!(intent, NativePlayerUiIntent::GuildBuffUpdate(_))));
}

#[test]
fn guild_permission_flow_rechecks_authority_after_request_creation_at_actual_queue_admission() {
    for mode in [
        "permission",
        "complete",
        "refresh",
        "guild",
        "scope",
        "viewer",
        "role",
        "rank",
        "state",
    ] {
        let (mut social, mut queue) = fixture();
        let request = request(&social, 1);
        match mode {
            "permission" => {
                let mut packet = wire::status("Rank Guild", "Leader", 0);
                if let ServerPacket::GuildStatus { my_options, .. } = &mut packet {
                    *my_options = 0;
                }
                assert!(wire::apply_typed(&mut social, packet));
            }
            "complete" => social.guild.roster_complete = false,
            "refresh" => social.guild.roster_needs_refresh = true,
            "guild" => assert!(wire::apply_typed(
                &mut social,
                wire::status("Other Guild", "Leader", 0)
            )),
            "scope" => social.guild.scope_epoch += 1,
            "viewer" => social.viewer.as_mut().unwrap().character_index = 99,
            "role" => social.guild.my_rank_id = 3,
            "rank" => {
                social.guild.ranks.retain(|rank| rank.index != 2);
            }
            "state" => {
                // A full snapshot can change the bit but cannot complete the old operation.
                assert!(wire::apply_typed(&mut social, rank_packet(255, "", 3)));
            }
            _ => unreachable!(),
        }
        assert!(!social.guild_rank_permission_current(&request), "{mode}");
        let before = social.clone();
        assert!(
            !queue.push_guild_rank_permission_pending(&mut social, request),
            "{mode}"
        );
        assert_eq!(social, before, "{mode}");
        assert!(queue.drain_intents().is_empty(), "{mode}");
    }
}

#[test]
fn guild_permission_flow_builder_and_queue_reject_wrong_requester_or_forged_viewer() {
    let (mut social, mut queue) = fixture();
    for requester in ["", "leader", "Officer", "Foreign"] {
        assert!(social
            .guild_rank_permission_request(requester, 2, 1, true)
            .is_none());
    }
    assert!(social
        .guild_rank_permission_request("Leader", 2, 8, true)
        .is_none());
    assert!(social
        .guild_rank_permission_request("Leader", 99, 1, true)
        .is_none());
    assert!(
        social
            .guild_rank_permission_request("Leader", 2, 0, true)
            .is_none(),
        "already-set checkbox is not a mutation"
    );
    let valid = request(&social, 1);
    let mut wrong_name = valid.clone();
    wrong_name.requester.name = "Officer".into();
    let mut wrong_id = valid.clone();
    wrong_id.requester.character_index = 2;
    for forged in [wrong_name, wrong_id] {
        assert!(!queue.push_guild_rank_permission_pending(&mut social, forged));
    }
    assert!(social.pending.is_empty());
    assert!(queue.drain_intents().is_empty());
    social.viewer = None;
    assert!(social
        .guild_rank_permission_request("Leader", 2, 1, true)
        .is_none());
    assert!(!queue.push_guild_rank_permission_pending(&mut social, valid));
}

#[test]
fn guild_permission_flow_nonleader_cannot_edit_a_higher_rank() {
    let mut social = wire::seeded(1);
    assert!(social.apply_network_packet(
        "UserInformation",
        &json!({"info":{"name":"Officer","realId":2}})
    ));
    assert_eq!(social.guild.my_rank_id, 1);
    assert!(social
        .guild_rank_permission_request("Officer", 0, 1, false)
        .is_none());
    let own = social
        .guild_rank_permission_request("Officer", 1, 1, true)
        .expect("own rank is not above the viewer");
    let mut queue = NativePlayerUiIntentQueue::default();
    assert!(queue.push_guild_rank_permission_pending(&mut social, own.clone()));
    assert_eq!(queue.drain_intents(), vec![typed_intent(&own)]);
}

#[test]
fn guild_permission_flow_map_keeps_pending_but_session_reset_retires_queue_and_context() {
    let (mut social, mut queue) = fixture();
    let request = request(&social, 1);
    assert!(queue.push_guild_rank_permission_pending(&mut social, request.clone()));
    let pending = social.pending.clone();
    social.clear_scene();
    assert_eq!(social.pending, pending);
    assert!(social.guild_rank_permission_current(&request));
    assert_eq!(queue.drain_intents(), vec![typed_intent(&request)]);
    social.clear_session();
    queue.clear();
    assert!(social.pending.is_empty() && social.viewer.is_none());
    assert!(!queue.push_guild_rank_permission_pending(&mut social, request));
    assert!(queue.drain_intents().is_empty());
}

#[test]
fn guild_permission_flow_same_name_leave_rejoin_cannot_reuse_old_scope() {
    let (mut social, mut queue) = fixture();
    let old = request(&social, 1);
    assert!(wire::apply_typed(&mut social, wire::status("", "", -1)));
    assert!(wire::apply_typed(
        &mut social,
        wire::status("Rank Guild", "Leader", 0)
    ));
    assert!(wire::apply_typed(&mut social, wire::full("Scouts")));
    let fresh = request(&social, 1);
    assert_ne!(fresh.scope_epoch, old.scope_epoch);
    assert!(!queue.push_guild_rank_permission_pending(&mut social, old));
    assert!(queue.push_guild_rank_permission_pending(&mut social, fresh.clone()));
    assert_eq!(queue.drain_intents(), vec![typed_intent(&fresh)]);
}

#[test]
fn guild_permission_flow_real_status7_releases_only_the_matching_bit_and_actor() {
    let (mut local, mut queue) = fixture();
    let first = request(&local, 1);
    let second = request(&local, 2);
    assert!(queue.push_guild_rank_permission_pending(&mut local, first.clone()));
    assert!(queue.push_guild_rank_permission_pending(&mut local, second.clone()));
    let unrelated = SocialPendingOperation::GroupAdd {
        name: "Friend".into(),
    };
    assert!(local.begin_pending(unrelated.clone()));
    assert_eq!(
        queue.drain_intents(),
        vec![typed_intent(&first), typed_intent(&second)]
    );
    let mut network = local.clone();
    assert!(wire::apply_typed(
        &mut network,
        rank_packet(7, "Officer", 3)
    ));
    local.apply_authoritative(network.clone());
    assert_eq!(
        local.pending.len(),
        3,
        "foreign actor is not the local permission receipt"
    );
    assert!(wire::apply_typed(&mut network, rank_packet(7, "Leader", 3)));
    local.apply_authoritative(network);
    assert_eq!(
        local.pending,
        vec![
            SocialPendingOperation::GuildRankPermission(second),
            unrelated
        ]
    );
    assert_eq!(
        local.guild.ranks[2].options, 3,
        "checkbox changes only on the ordinary server packet"
    );
}

fn button_app() -> App {
    let (social, queue) = fixture();
    let mut app = super::super::tests::help_button_test_app();
    app.insert_resource(social).insert_resource(queue);
    {
        let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
        state.core.panel = mir2_ui_core::state::UiPanel::Guild;
        state.guild_left_page = GuildLeftPage::Ranks;
        state.selected_guild_rank = Some(2);
        state.guild_panel.guild_name = Some("Rank Guild".into());
        state.guild_panel.owner_name = "Leader".into();
    }
    app
}

fn press_option(app: &mut App, option: u8, now_ms: u64) {
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .guild_panel
        .now_ms = now_ms;
    super::super::tests::press_help_button(app, OverlayButton::GuildRankTogglePermission(option));
}

#[test]
fn guild_permission_flow_actual_overlay_handler_enforces_native_300ms_without_optimistic_options() {
    let mut app = button_app();
    press_option(&mut app, 1, 100);
    let first = app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents();
    assert!(
        matches!(&first[..], [NativePlayerUiIntent::GuildRankPermission(request)]
        if request.rank_index == 2 && request.option == 1 && request.enabled)
    );
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .guild_panel
            .rank_option_ready_ms,
        400
    );
    press_option(&mut app, 2, 399);
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
    assert_eq!(app.world().resource::<SocialModel>().pending.len(), 1);
    press_option(&mut app, 2, 400);
    let second = app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents();
    assert!(
        matches!(&second[..], [NativePlayerUiIntent::GuildRankPermission(request)]
        if request.rank_index == 2 && request.option == 2 && request.enabled)
    );
    assert_eq!(app.world().resource::<SocialModel>().pending.len(), 2);
    assert_eq!(
        app.world().resource::<SocialModel>().guild.ranks[2].options,
        1
    );
    assert_eq!(
        app.world()
            .resource::<NativePlayerUiState>()
            .guild_panel
            .rank_option_ready_ms,
        700
    );
}

#[test]
fn guild_permission_flow_actual_overlay_rejects_invalid_option_foreign_owner_or_revoked_rank_permission(
) {
    for mode in ["option", "requester", "permission", "complete"] {
        let mut app = button_app();
        if mode == "requester" {
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .guild_panel
                .owner_name = "Officer".into();
        }
        if mode == "permission" {
            app.world_mut()
                .resource_mut::<SocialModel>()
                .guild
                .my_options = 0;
        }
        if mode == "complete" {
            app.world_mut()
                .resource_mut::<SocialModel>()
                .guild
                .roster_complete = false;
        }
        press_option(&mut app, if mode == "option" { 8 } else { 1 }, 100);
        assert!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents()
                .is_empty(),
            "{mode}"
        );
        assert!(
            app.world().resource::<SocialModel>().pending.is_empty(),
            "{mode}"
        );
        assert_eq!(
            app.world().resource::<SocialModel>().guild.ranks[2].options,
            1
        );
    }
}

#[test]
fn guild_permission_refresh_actual_panel_coalesces_unknown_member_until_complete_roster() {
    let (mut social, queue) = fixture();
    assert!(social.apply_network_packet(
        "GuildMemberChange",
        &json!({"name":"NewMember","rankIndex":0,"status":2,"ranks":[]})
    ));
    assert!(social.guild.roster_needs_refresh);
    assert!(
        !social
            .guild
            .members
            .iter()
            .any(|member| member.name == "NewMember"),
        "a name-only event cannot invent a stable member id"
    );
    let mut state = NativePlayerUiState::default();
    state.core.panel = mir2_ui_core::state::UiPanel::Guild;
    state.guild_panel.guild_name = social.guild.name.clone();
    state.guild_panel.owner_name = "Leader".into();
    let mut shell = NativeShellModel::default();
    shell.screen = NativeShellScreen::InGame;
    let mut app = App::new();
    app.insert_resource(state)
        .insert_resource(social)
        .insert_resource(queue)
        .insert_resource(shell)
        .insert_resource(GuildHost::default())
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(Update, process);
    let reads = |queue: &mut NativePlayerUiIntentQueue| {
        queue
            .drain_intents()
            .into_iter()
            .filter(|intent| {
                matches!(
                    intent,
                    NativePlayerUiIntent::GuildRosterRead(request) if request.info_type==1
                )
            })
            .count()
    };
    app.update();
    assert_eq!(
        reads(&mut app.world_mut().resource_mut::<NativePlayerUiIntentQueue>()),
        1
    );
    assert!(matches!(
        &app.world().resource::<SocialModel>().pending[..],
        [SocialPendingOperation::GuildRosterRead(_)]
    ));
    let mut network = app.world().resource::<SocialModel>().clone();
    network.pending.clear();
    assert!(wire::apply_typed(
        &mut network,
        wire::status("Rank Guild", "Leader", 0)
    ));
    app.world_mut()
        .resource_mut::<SocialModel>()
        .apply_authoritative(network.clone());
    app.update();
    assert_eq!(
        reads(&mut app.world_mut().resource_mut::<NativePlayerUiIntentQueue>()),
        0
    );
    assert_eq!(
        app.world().resource::<SocialModel>().pending.len(),
        1,
        "GuildStatus prelude must not release the outstanding read"
    );
    assert!(wire::apply_typed(&mut network, wire::full("Scouts")));
    app.world_mut()
        .resource_mut::<SocialModel>()
        .apply_authoritative(network);
    app.update();
    assert_eq!(
        reads(&mut app.world_mut().resource_mut::<NativePlayerUiIntentQueue>()),
        0
    );
    assert!(app.world().resource::<SocialModel>().pending.is_empty());
    assert!(
        !app.world()
            .resource::<SocialModel>()
            .guild
            .roster_needs_refresh
    );
}
