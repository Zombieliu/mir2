//! Native host intent-order tests, not a server/NPC purchase acceptance test.
use super::*;
use crate::gateway_bridge::{AndroidGatewayOutboundQueue, ANDROID_GATEWAY_QUEUE_CAPACITY};
use mir2_client_bevy::{
    crystal_ui::overlays::NativePlayerUiState,
    quest_ui::{QuestUiIntent, QuestUiIntentQueue},
};
use mir2_ui_core::effect::GatewayCommand;

fn app_with_intents(intents: impl IntoIterator<Item = QuestUiIntent>, full: bool) -> App {
    let mut player = NativePlayerUiState::default();
    player.request_npc_service_exit();
    let mut queue = QuestUiIntentQueue::default();
    for intent in intents {
        assert!(queue.push_intent(intent));
    }
    let mut gateway = AndroidGatewayOutboundQueue::default();
    if full {
        for _ in 0..ANDROID_GATEWAY_QUEUE_CAPACITY {
            gateway
                .enqueue(GatewayCommand::SelectNpcDialog {
                    target: "@existing".into(),
                })
                .unwrap();
        }
    }
    let mut app = App::new();
    app.insert_resource(NativeShellModel {
        screen: Screen::InGame,
        ..default()
    })
    .insert_resource(player)
    .insert_resource(queue)
    .insert_resource(gateway)
    .add_systems(Update, forward_quest_ui_intents);
    app.world_mut().spawn(Window {
        focused: true,
        ..default()
    });
    app
}

#[test]
fn npc_host_reopen_requires_an_accepted_fresh_request() {
    for (intent, begins) in [
        (QuestUiIntent::InteractNpc { npc_object_id: 7 }, true),
        (
            QuestUiIntent::SelectNpcDialog {
                target: "@BuySell".into(),
            },
            true,
        ),
        (
            QuestUiIntent::SelectNpcDialog {
                target: "@EXIT".into(),
            },
            false,
        ),
    ] {
        for full in [false, true] {
            let mut app = app_with_intents([intent.clone()], full);
            app.update();
            assert_eq!(
                app.world()
                    .resource::<NativePlayerUiState>()
                    .accepts_npc_service_reply(),
                begins && !full,
                "intent={intent:?}, full={full}"
            );
        }
    }
}

#[test]
fn npc_host_batch_order_preserves_last_exit_even_if_enqueue_fails() {
    for exit_last in [true, false] {
        for full in [false, true] {
            let targets = if exit_last {
                ["@BuySell", "@Exit"]
            } else {
                ["@Exit", "@BuySell"]
            };
            let mut app = app_with_intents(
                targets.map(|target| QuestUiIntent::SelectNpcDialog {
                    target: target.into(),
                }),
                full,
            );
            app.update();
            assert_eq!(
                app.world()
                    .resource::<NativePlayerUiState>()
                    .accepts_npc_service_reply(),
                !exit_last && !full,
                "exit_last={exit_last}, full={full}"
            );
        }
    }
    // A previous accepted request must not defeat a failed Exit write.
    let mut app = app_with_intents(
        [QuestUiIntent::SelectNpcDialog {
            target: "@exit".into(),
        }],
        true,
    );
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .begin_npc_service_request();
    app.update();
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .accepts_npc_service_reply());
}

#[test]
fn npc_host_inactive_or_unfocused_intents_do_not_reopen_services() {
    for starting in [false, true] {
        let mut app = app_with_intents([QuestUiIntent::InteractNpc { npc_object_id: 7 }], false);
        if starting {
            app.world_mut().resource_mut::<NativeShellModel>().screen = Screen::StartingGame;
        } else {
            let mut query = app.world_mut().query::<&mut Window>();
            query.single_mut(app.world_mut()).unwrap().focused = false;
        }
        app.update();
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .accepts_npc_service_reply());
    }
}

#[test]
fn npc_host_received_packets_require_current_play_phase_and_ui_reply_gate() {
    let raw = json!({"type":"packet","packet":"NPCSell","payload":null}).to_string();
    let mut host = HostState::default();
    host.npc
        .snapshot(
            &json!({"playerObjectId":42,"mapFileName":"0",
        "entities":[{"objectId":42,"kind":"selfPlayer","name":"Fixture"}]})
            .to_string(),
            host.player.presentation_cursor(),
        )
        .unwrap();
    for (phase, screen, accepts, expected) in [
        ("DISCONNECTED", Screen::InGame, true, false),
        ("STARTING", Screen::InGame, true, false),
        ("IN_GAME", Screen::StartingGame, true, false),
        ("IN_GAME", Screen::InGame, false, false),
        ("IN_GAME", Screen::InGame, true, true),
    ] {
        host.phase = phase.into();
        assert_eq!(
            host.accept_npc_packet(screen, &raw, accepts).unwrap(),
            expected
        );
    }
    host.reset_personal();
    assert!(!host.accept_npc_packet(Screen::InGame, &raw, true).unwrap());
}
