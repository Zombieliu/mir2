//! Actual Native queue -> transport, with typed local context and ordinary wire fields.
use super::*;
use bevy::prelude::App;
use serde_json::json;

fn social(guild: &str) -> SocialModel {
    let mut social = SocialModel::default();
    assert!(social.apply_network_packet(
        "UserInformation",
        &json!({"name":"Alice","realId":41,"objectId":9001})
    ));
    assert!(social.apply_network_packet(
        "GuildStatus",
        &json!({"guildName":guild,"guildRankName":"Officer","myRankId":1,"myOptions":1})
    ));
    assert!(social.apply_network_packet(
        "GuildMemberChange",
        &json!({"name":"","rankIndex":0,"status":255,"ranks":[
        {"name":"Leader","index":0,"options":255,"members":[{"name":"Chief","id":1,"online":true}]},
        {"name":"Officer","index":1,"options":1,"members":[{"name":"Alice","id":41,"online":true}]},
        {"name":"Member","index":2,"options":0,"members":[{"name":"Bob","id":42,"online":false}]}]})
    ));
    social
}

fn read_fixture(
    in_game: bool,
    sender: crate::gateway::GatewayCommandSender,
) -> (App, GuildRosterReadRequest) {
    let mut social = social("Guild");
    let request = social.guild_roster_read_request(1).unwrap();
    let mut queue = NativePlayerUiIntentQueue::default();
    assert!(queue.push_guild_roster_read_pending(&mut social, request.clone()));
    let mut ui = NativePlayerUiState::default();
    ui.guild_panel.owner_name = "Alice".into();
    ui.guild_panel.guild_name = Some("Guild".into());
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
fn native_guild_read_wire_real_bounded_full_releases_unsent_then_allows_one_fresh_read() {
    let (sender, mut receiver) = crate::gateway::test_command_channel(8);
    for _ in 0..8 {
        assert!(sender
            .send(GatewayCommand::Wire(
                NativeOutboundCommand::RequestGuildInfo { info_type: 1 }
            ))
            .is_ok());
    }
    let (mut app, _) = read_fixture(true, sender);
    app.update();
    assert!(
        app.world().resource::<SocialModel>().pending.is_empty(),
        "definitely-unsent read must release its exact budget"
    );
    for _ in 0..8 {
        assert!(receiver.try_recv_for_test().is_ok());
    }
    assert!(receiver.try_recv_for_test().is_err());
    let request = app
        .world()
        .resource::<SocialModel>()
        .guild_roster_read_request(1)
        .unwrap();
    app.world_mut().resource_scope(
        |world, mut queue: bevy::prelude::Mut<NativePlayerUiIntentQueue>| {
            assert!(queue.push_guild_roster_read_pending(
                &mut world.resource_mut::<SocialModel>(),
                request.clone()
            ));
        },
    );
    app.update();
    assert!(matches!(
        receiver.try_recv_for_test().unwrap(),
        GatewayCommand::Wire(NativeOutboundCommand::RequestGuildInfo { info_type: 1 })
    ));
    assert!(receiver.try_recv_for_test().is_err());
    assert_eq!(
        app.world().resource::<SocialModel>().pending,
        vec![SocialPendingOperation::GuildRosterRead(request)]
    );
    app.update();
    assert!(
        receiver.try_recv_for_test().is_err(),
        "accepted read is not retransmitted or released without a real full reply"
    );
    let mut incoming = app.world().resource::<SocialModel>().clone();
    incoming.pending.clear();
    let ranks = serde_json::to_value(&incoming.guild.ranks).unwrap();
    assert!(incoming.apply_network_packet(
        "GuildMemberChange",
        &json!({"name":"","status":255,"rankIndex":0,"ranks":ranks})
    ));
    app.world_mut()
        .resource_mut::<SocialModel>()
        .apply_authoritative(incoming);
    assert!(app.world().resource::<SocialModel>().pending.is_empty());
}

#[test]
fn native_guild_read_wire_non_game_cancels_only_unsent_read_without_releasing_group() {
    let (sender, mut receiver) = crate::gateway::test_command_channel(8);
    let (mut app, _) = read_fixture(false, sender);
    let other = SocialPendingOperation::GroupAdd {
        name: "Friend".into(),
    };
    app.world_mut()
        .resource_mut::<SocialModel>()
        .begin_pending(other.clone());
    app.update();
    assert!(receiver.try_recv_for_test().is_err());
    assert_eq!(app.world().resource::<SocialModel>().pending, vec![other]);
}

#[test]
fn native_guild_read_wire_old_scope_cannot_consume_successor_same_info_type() {
    let (sender, mut receiver) = crate::gateway::test_command_channel(8);
    let (mut app, old) = read_fixture(true, sender);
    let mut next = social("Other");
    next.guild.scope_epoch = old.scope_epoch + 1;
    let request = next.guild_roster_read_request(1).unwrap();
    app.world_mut().insert_resource(next);
    app.world_mut().resource_scope(
        |world, mut queue: bevy::prelude::Mut<NativePlayerUiIntentQueue>| {
            assert!(queue.push_guild_roster_read_pending(
                &mut world.resource_mut::<SocialModel>(),
                request.clone()
            ));
        },
    );
    app.update();
    assert!(receiver.try_recv_for_test().is_ok());
    assert!(receiver.try_recv_for_test().is_err());
    assert_eq!(
        app.world().resource::<SocialModel>().pending,
        vec![SocialPendingOperation::GuildRosterRead(request)]
    );
}

fn fixture(
    in_game: bool,
    sender: mpsc::Sender<GatewayCommand>,
) -> (App, GuildRankPermissionRequest) {
    let mut social = social("Guild");
    let request = social
        .guild_rank_permission_request("Alice", 2, 3, true)
        .unwrap();
    let mut queue = NativePlayerUiIntentQueue::default();
    assert!(queue.push_guild_rank_permission_pending(&mut social, request.clone()));
    let mut ui = NativePlayerUiState::default();
    ui.guild_panel.owner_name = "Alice".into();
    ui.guild_panel.guild_name = Some("Guild".into());
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
fn native_guild_permission_wire_acceptance_preserves_pending_without_optimistic_options_or_replay()
{
    let (sender, receiver) = mpsc::channel();
    let (mut app, request) = fixture(true, sender);
    app.update();
    let sent = receiver.try_iter().collect::<Vec<_>>();
    assert!(
        matches!(&sent[..],[GatewayCommand::Wire(NativeOutboundCommand::EditGuildMember{change_type:5,rank_index:2,name,rank_name})] if name=="true"&&rank_name=="3")
    );
    let social = app.world().resource::<SocialModel>();
    assert!(social
        .pending
        .contains(&SocialPendingOperation::GuildRankPermission(request)));
    assert_eq!(social.guild.ranks[2].options, 0);
    app.update();
    assert_eq!(receiver.try_iter().count(), 0);
}

#[test]
fn native_guild_permission_wire_definitely_unsent_releases_only_exact_request() {
    for in_game in [false, true] {
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        let (mut app, _) = fixture(in_game, sender);
        let unrelated = SocialPendingOperation::GroupAdd {
            name: "Friend".into(),
        };
        app.world_mut()
            .resource_mut::<SocialModel>()
            .begin_pending(unrelated.clone());
        app.update();
        assert_eq!(
            app.world().resource::<SocialModel>().pending,
            vec![unrelated]
        );
    }
}

#[test]
fn native_guild_permission_wire_rechecks_authority_identity_and_visible_guild_at_dispatch() {
    for change in 0..4 {
        let (sender, receiver) = mpsc::channel();
        let (mut app, _) = fixture(true, sender);
        match change {
            0 => {
                app.world_mut()
                    .resource_mut::<SocialModel>()
                    .guild
                    .my_options = 0
            }
            1 => {
                app.world_mut()
                    .resource_mut::<SocialModel>()
                    .viewer
                    .as_mut()
                    .unwrap()
                    .character_index = 99
            }
            2 => {
                app.world_mut()
                    .resource_mut::<SocialModel>()
                    .guild
                    .scope_epoch += 1
            }
            _ => {
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .guild_panel
                    .guild_name = Some("Other".into())
            }
        }
        app.update();
        assert_eq!(receiver.try_iter().count(), 0);
        assert!(app.world().resource::<SocialModel>().pending.is_empty());
    }
}

#[test]
fn native_guild_permission_wire_naked_type5_cannot_borrow_another_typed_request() {
    let (sender, receiver) = mpsc::channel();
    let (mut app, request) = fixture(true, sender);
    let queue = &mut app.world_mut().resource_mut::<NativePlayerUiIntentQueue>();
    queue.drain_intents();
    assert!(queue.push_intent(NativePlayerUiIntent::GuildEditMember {
        change_type: 5,
        rank_index: 2,
        name: "true".into(),
        rank_name: "3".into()
    }));
    app.update();
    assert_eq!(receiver.try_iter().count(), 0);
    assert!(app
        .world()
        .resource::<SocialModel>()
        .pending
        .contains(&SocialPendingOperation::GuildRankPermission(request)));
}

#[test]
fn native_guild_permission_wire_retired_context_cannot_consume_same_payload_in_successor_guild() {
    let (sender, receiver) = mpsc::channel();
    let (mut app, old) = fixture(true, sender);
    let mut next = social("Other");
    next.guild.scope_epoch = old.scope_epoch + 1;
    let request = next
        .guild_rank_permission_request("Alice", 2, 3, true)
        .unwrap();
    app.world_mut().insert_resource(next);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .guild_panel
        .guild_name = Some("Other".into());
    app.world_mut().resource_scope(
        |world, mut queue: bevy::prelude::Mut<NativePlayerUiIntentQueue>| {
            assert!(queue.push_guild_rank_permission_pending(
                &mut world.resource_mut::<SocialModel>(),
                request.clone()
            ));
        },
    );
    app.update();
    assert_eq!(receiver.try_iter().count(), 1);
    assert_eq!(
        app.world().resource::<SocialModel>().pending,
        vec![SocialPendingOperation::GuildRankPermission(request)]
    );
}
