//! Local ordinary queue/wire checks. No TCP/WSS, service or native acceptance.
use super::*;
use bevy::prelude::App;
use mir2_client_bevy::social::GuildRankRenameRequest;
use mir2_protocol::{GuildMember, GuildRank, ServerPacket};

fn apply(model: &mut SocialModel, packet: ServerPacket) {
    let value = serde_json::to_value(packet).unwrap();
    let (name, payload) = value.as_object().unwrap().iter().next().unwrap();
    assert!(model.apply_network_packet(name, payload));
}

fn guild_status(name: &str) -> ServerPacket {
    ServerPacket::GuildStatus {
        guild_name: name.into(),
        guild_rank_name: "Leader".into(),
        level: 0,
        experience: 0,
        max_experience: 1000,
        gold: 0,
        spare_points: 0,
        member_count: 1,
        max_members: 50,
        voting: false,
        item_count: 0,
        buff_count: 0,
        my_options: 255,
        my_rank_id: 0,
    }
}

fn fixture(in_game: bool, sender: mpsc::Sender<GatewayCommand>) -> (App, GuildRankRenameRequest) {
    let mut social = SocialModel::default();
    apply(&mut social, guild_status("Rank Guild"));
    apply(
        &mut social,
        ServerPacket::GuildMemberChange {
            name: String::new(),
            rank_index: 0,
            status: 255,
            ranks: vec![
                GuildRank {
                    name: "Leader".into(),
                    options: 255,
                    index: 0,
                    members: vec![GuildMember {
                        name: "Leader".into(),
                        id: 1,
                        last_login_binary_datetime: 0,
                        has_voted: false,
                        online: true,
                    }],
                },
                GuildRank {
                    name: "Officers".into(),
                    options: 1,
                    index: 1,
                    members: vec![],
                },
                GuildRank {
                    name: "Scouts".into(),
                    options: 0,
                    index: 2,
                    members: vec![],
                },
            ],
        },
    );
    let request = social
        .guild_rank_rename_request("Leader", 2, "Veterans", 5100)
        .unwrap();
    let mut queue = NativePlayerUiIntentQueue::default();
    assert!(queue.push_guild_rank_rename_pending(&mut social, request.clone()));
    let mut ui = NativePlayerUiState::default();
    ui.guild_panel.owner_name = "Leader".into();
    ui.guild_panel.guild_name = Some("Rank Guild".into());
    ui.guild_panel.now_ms = 100;
    ui.guild_panel.rank_name_submission = Some(request.clone());
    ui.guild_panel.rank_name_ready_ms = 5100;
    ui.selected_guild_rank = Some(2);
    ui.guild_rank_name_draft = "Veterans".into();
    let mut shell = NativeShellModel::default();
    if in_game {
        shell.screen = NativeShellScreen::InGame;
    }
    let mut app = App::new();
    app.insert_resource(shell)
        .insert_resource(social)
        .insert_resource(queue)
        .insert_resource(ui)
        .init_resource::<QuestUiIntentQueue>()
        .insert_resource(GatewayCommands::new(sender))
        .add_systems(bevy::prelude::Update, forward_quest_ui_intents);
    (app, request)
}

#[test]
fn guild_rank_transport_acceptance_keeps_exact_pending_and_does_not_claim_success() {
    let (sender, receiver) = mpsc::channel();
    let (mut app, request) = fixture(true, sender);
    let event = app.world().resource::<SocialModel>().last_event.clone();
    app.update();
    let sent = receiver.try_iter().collect::<Vec<_>>();
    assert!(
        matches!(&sent[..], [GatewayCommand::Wire(NativeOutboundCommand::EditGuildMember {
        change_type: 3, rank_index: 2, name, rank_name
    })] if name.is_empty() && rank_name == "Veterans")
    );
    let social = app.world().resource::<SocialModel>();
    assert_eq!(
        social.pending,
        vec![SocialPendingOperation::GuildRankRename(request)]
    );
    assert_eq!(social.last_event, event);
    assert_eq!(social.guild.ranks[2].name, "Scouts");
    app.update();
    assert_eq!(
        receiver.try_iter().count(),
        0,
        "accepted sends are not replayed"
    );
}

#[test]
fn guild_rank_unsent_command_releases_only_exact_context_and_restores_matching_editor() {
    for in_game in [true, false] {
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        let (mut app, _) = fixture(in_game, sender);
        let unrelated = SocialPendingOperation::GroupAdd {
            name: "Friend".into(),
        };
        app.world_mut()
            .resource_mut::<SocialModel>()
            .begin_pending(unrelated.clone());
        let before = app.world().resource::<SocialModel>().guild.clone();
        app.update();
        assert_eq!(
            app.world().resource::<SocialModel>().pending,
            vec![unrelated]
        );
        assert_eq!(app.world().resource::<SocialModel>().guild, before);
        let ui = app.world().resource::<NativePlayerUiState>();
        assert!(ui.guild_panel.rank_name_submission.is_none());
        assert_eq!(ui.guild_panel.rank_name_ready_ms, 0);
        assert_eq!(ui.guild_rank_name_draft, "Veterans");
    }
}

#[test]
fn guild_rank_failure_for_other_editor_does_not_clear_current_context_or_focus() {
    let (sender, receiver) = mpsc::channel();
    drop(receiver);
    let (mut app, _) = fixture(true, sender);
    let other = app
        .world()
        .resource::<SocialModel>()
        .guild_rank_rename_request("Leader", 1, "Captains", 6100)
        .unwrap();
    app.world_mut()
        .resource_mut::<SocialModel>()
        .begin_pending(SocialPendingOperation::GuildRankRename(other.clone()));
    {
        let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
        ui.selected_guild_rank = Some(1);
        ui.guild_rank_name_draft = "Captains".into();
        ui.guild_panel.rank_name_submission = Some(other.clone());
        ui.guild_panel.rank_name_ready_ms = 6100;
    }
    app.update();
    assert_eq!(
        app.world().resource::<SocialModel>().pending,
        vec![SocialPendingOperation::GuildRankRename(other.clone())]
    );
    let ui = app.world().resource::<NativePlayerUiState>();
    assert_eq!(ui.guild_panel.rank_name_submission, Some(other));
    assert_eq!(ui.guild_panel.rank_name_ready_ms, 6100);
    assert!(!ui.guild_rank_name_focused);
}

#[test]
fn guild_rank_queued_old_scope_or_expired_input_is_not_transmitted() {
    for expired in [false, true] {
        let (sender, receiver) = mpsc::channel();
        let (mut app, _) = fixture(true, sender);
        if expired {
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .guild_panel
                .now_ms = 5101;
        } else {
            let mut incoming = app.world().resource::<SocialModel>().clone();
            incoming.pending.clear();
            apply(&mut incoming, guild_status("New Guild"));
            app.world_mut()
                .resource_mut::<SocialModel>()
                .apply_authoritative(incoming);
        }
        app.update();
        assert_eq!(
            receiver.try_iter().count(),
            0,
            "consumed old editor input must not mutate a new Guild"
        );
        assert!(app.world().resource::<SocialModel>().pending.is_empty());
    }
}
