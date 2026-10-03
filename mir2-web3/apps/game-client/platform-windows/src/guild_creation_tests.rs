use super::*;
use bevy::prelude::{App, Update};
use mir2_client_bevy::crystal_ui::overlays::social_bond_dialog::BondPromptKind;
use mir2_protocol::{ClientPacket, ServerPacket};

fn creator_dialog() -> NpcDialogModel {
    NpcDialogModel {
        is_open: true,
        npc_object_id: Some(427),
        options: vec![NpcDialogOption {
            option_id: "@CREATEGUILD".into(),
            label: "Request Guild creation".into(),
            enabled: true,
        }],
        ..Default::default()
    }
}
fn creator_map() -> BigMapModel {
    BigMapModel {
        reset_epoch: 7,
        current_map_index: Some(228),
        ..Default::default()
    }
}
fn creator_context() -> GuildCreationContext {
    native_guild_creation_context(Some(&creator_dialog()), Some(&creator_map())).unwrap()
}
fn creator_app() -> (App, std::sync::mpsc::Receiver<GatewayCommand>) {
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut app = App::new();
    app.insert_resource(NativeShellModel {
        screen: NativeShellScreen::InGame,
        ..Default::default()
    })
    .insert_resource(NativePlayerUiState::default())
    .insert_resource(creator_dialog())
    .insert_resource(creator_map())
    .insert_resource(UiReadModel::default())
    .insert_resource(QuestTracker::default())
    .init_resource::<QuestUiIntentQueue>()
    .init_resource::<NativePlayerUiIntentQueue>()
    .init_resource::<PendingOperations>()
    .insert_resource(GatewayCommands::new(sender))
    .add_systems(Update, forward_quest_ui_intents);
    (app, receiver)
}
fn begin_prompt(app: &mut App) -> u64 {
    let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
    assert!(ui
        .social_bonds
        .begin_guild_creation_request(creator_context()));
    ui.social_bonds.observe(&ServerPacket::GuildNameRequest);
    ui.social_bonds.prompt.as_ref().unwrap().revision
}
fn answer_name(app: &mut App, revision: u64) -> ClientPacket {
    let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
    ui.social_bonds.input_name(revision, "Knights");
    ui.social_bonds.answer(revision, true).unwrap()
}

#[test]
fn native_guild_creation_original_npc_then_typed_name_round_trip_is_one_shot() {
    let (mut app, receiver) = creator_app();
    for _ in 0..2 {
        app.world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::SelectNpcDialog {
                target: "@CREATEGUILD".into(),
            });
    }
    app.update();
    assert!(
        matches!(receiver.try_recv(), Ok(GatewayCommand::Wire(NativeOutboundCommand::SelectNpcDialog { target })) if target == "@CREATEGUILD")
    );
    assert!(receiver.try_recv().is_err());
    let revision = {
        let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
        assert!(
            ui.social_bonds.prompt.is_none(),
            "requesting an NPC service is not its authorization reply"
        );
        ui.social_bonds.observe(&ServerPacket::GuildNameRequest);
        assert!(matches!(
            ui.social_bonds.prompt.as_ref().unwrap().kind,
            BondPromptKind::GuildName { .. }
        ));
        ui.social_bonds.prompt.as_ref().unwrap().revision
    };
    let packet = answer_name(&mut app, revision);
    app.world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .push_intent(NativePlayerUiIntent::SocialBondPacket(packet.clone()));
    app.update();
    assert!(
        matches!(receiver.try_recv(), Ok(GatewayCommand::Wire(NativeOutboundCommand::GuildNameReturn { name })) if name == "Knights")
    );
    assert!(receiver.try_recv().is_err());
    // A late retry cannot dispatch the already consumed name a second time.
    app.world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .push_intent(NativePlayerUiIntent::SocialBondPacket(packet));
    app.update();
    assert!(receiver.try_recv().is_err());
}

#[test]
fn native_guild_creation_requires_current_enabled_link_npc_and_map_identity() {
    for boundary in 0..6 {
        let (mut app, receiver) = creator_app();
        match boundary {
            0 => app.world_mut().resource_mut::<NpcDialogModel>().is_open = false,
            1 => {
                app.world_mut()
                    .resource_mut::<NpcDialogModel>()
                    .npc_object_id = None
            }
            2 => app
                .world_mut()
                .resource_mut::<NpcDialogModel>()
                .options
                .clear(),
            3 => app.world_mut().resource_mut::<NpcDialogModel>().options[0].enabled = false,
            4 => {
                app.world_mut().remove_resource::<BigMapModel>();
            }
            5 => {
                app.world_mut()
                    .resource_mut::<BigMapModel>()
                    .current_map_index = None
            }
            _ => unreachable!(),
        }
        app.world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::SelectNpcDialog {
                target: "@CREATEGUILD".into(),
            });
        app.update();
        assert!(receiver.try_recv().is_err(), "boundary={boundary}");
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .social_bonds
            .observe(&ServerPacket::GuildNameRequest);
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .social_bonds
            .prompt
            .is_none());
    }
}

#[test]
fn native_guild_creation_unsolicited_name_return_never_reaches_gateway() {
    let (mut app, receiver) = creator_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .push_intent(NativePlayerUiIntent::SocialBondPacket(
            ClientPacket::GuildNameReturn {
                name: "Forged".into(),
            },
        ));
    app.update();
    assert!(receiver.try_recv().is_err());
}

#[test]
fn native_guild_creation_queued_name_retires_on_map_npc_logout_or_exit() {
    for boundary in 0..4 {
        let (mut app, receiver) = creator_app();
        let revision = begin_prompt(&mut app);
        let packet = answer_name(&mut app, revision);
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .push_intent(NativePlayerUiIntent::SocialBondPacket(packet));
        match boundary {
            0 => app.world_mut().resource_mut::<BigMapModel>().reset_epoch = 8,
            1 => {
                app.world_mut()
                    .resource_mut::<NpcDialogModel>()
                    .npc_object_id = Some(428)
            }
            2 => {
                app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::Login
            }
            3 => {
                app.world_mut()
                    .resource_mut::<QuestUiIntentQueue>()
                    .push_intent(QuestUiIntent::SelectNpcDialog {
                        target: "@Exit".into(),
                    });
            }
            _ => unreachable!(),
        }
        app.update();
        assert!(
            !receiver.try_iter().any(|c| matches!(
                c,
                GatewayCommand::Wire(NativeOutboundCommand::GuildNameReturn { .. })
            )),
            "boundary={boundary}"
        );
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .social_bonds
            .prompt
            .is_none());
    }
}

#[test]
fn native_guild_creation_transport_rejection_restores_draft_without_fabricating_success() {
    let (mut app, receiver) = creator_app();
    let revision = begin_prompt(&mut app);
    let packet = answer_name(&mut app, revision);
    app.world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .push_intent(NativePlayerUiIntent::SocialBondPacket(packet));
    drop(receiver);
    app.update();
    let ui = app.world().resource::<NativePlayerUiState>();
    assert!(
        matches!(&ui.social_bonds.prompt.as_ref().unwrap().kind, BondPromptKind::GuildName { text } if text == "Knights")
    );
    assert_eq!(ui.social_bonds.prompt.as_ref().unwrap().revision, revision);
    assert!(app
        .world()
        .resource::<UiReadModel>()
        .player
        .guild_name
        .is_none());
}

#[test]
fn native_guild_creation_packet_order_cannot_reopen_a_changed_service() {
    for boundary in 0..3 {
        let mut ui = NativePlayerUiState::default();
        ui.social_bonds
            .begin_guild_creation_request(creator_context());
        let packet = NativeGameplaySnapshot {
            big_map_only: true,
            big_map: creator_map(),
            social_bond_packet: Some(ServerPacket::GuildNameRequest),
            ..Default::default()
        };
        observe_native_social_snapshot(&mut ui, &packet);
        assert!(ui.social_bonds.prompt.is_some());
        match boundary {
            0 => observe_native_social_snapshot(
                &mut ui,
                &NativeGameplaySnapshot {
                    big_map: creator_map(),
                    dialog: NpcDialogModel::default(),
                    ..Default::default()
                },
            ),
            1 => observe_native_social_snapshot(
                &mut ui,
                &NativeGameplaySnapshot {
                    big_map_only: true,
                    big_map: BigMapModel {
                        reset_epoch: 8,
                        ..creator_map()
                    },
                    ..Default::default()
                },
            ),
            2 => ui.request_npc_service_exit(),
            _ => unreachable!(),
        }
        observe_native_social_snapshot(&mut ui, &packet);
        assert!(ui.social_bonds.prompt.is_none(), "boundary={boundary}");
    }
}
