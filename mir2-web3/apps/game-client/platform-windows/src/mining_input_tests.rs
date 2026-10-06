use super::*;
use mir2_client_bevy::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, ItemModel};

fn app_with_mining_gesture() -> (
    bevy::prelude::App,
    std::sync::mpsc::Receiver<GatewayCommand>,
) {
    let (mut app, receiver) = input_app();
    app.world_mut().spawn(Window::default());
    let mut mouse = ButtonInput::<MouseButton>::default();
    mouse.press(MouseButton::Left);
    mouse.clear_just_pressed(MouseButton::Left);
    app.insert_resource(mouse);
    app.insert_resource(NativePlayerUiState::default());
    app.insert_resource(NpcDialogModel::default());
    app.init_resource::<QuestUiIntentQueue>();
    app.init_resource::<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiIntentQueue>();
    let mut read = UiReadModel::default();
    read.player.hp = 100;
    read.player.max_hp = 100;
    app.insert_resource(read);
    app.insert_resource(NativeWorldClickState {
        riding_mount: Some(false),
        dazed: Some(false),
        fishing: Some(false),
        class: Some("Warrior".into()),
        ..Default::default()
    });
    let inventory = InventoryModel {
        items: vec![ItemModel {
            unique_id: Some(700),
            key: "crystal-item:836".into(),
            quantity: 1,
            container: 2,
            slot: 0,
            equip_slot: Some("weapon".into()),
            durability_current: Some(10_000),
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: 836,
                    item_type: 1,
                    can_mine: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let mining_tool = mining_input::equipped_mining_tool(Some(&inventory));
    app.insert_resource(inventory);
    app.world_mut()
        .resource_mut::<NativeEntityPresentation>()
        .observe_packet_payload(
            serde_json::json!({
                "mapFileName": "D401", "sceneView": {"center": {"x": 0, "y": 0}},
                "entities": [{"objectId": "1000", "kind": "selfPlayer", "x": 0, "y": 0,
                    "direction": "right", "dead": false, "sprite": {"bodyLibrary": "CArmour/00"}}]
            }),
            0,
        );
    // This is an existing gesture, not the first player snapshot. Otherwise
    // the identity reset would cancel every case before the tested input.
    app.world_mut()
        .resource_mut::<WorldPointerMovementState>()
        .observe_identity("1000", (0, 0), "right");
    app.world_mut()
        .resource_mut::<WorldPointerMovementState>()
        .directional_attack = Some(mining_input::DirectionalAttackGesture {
        id: 7,
        direction: "right",
        origin: (0, 0),
        map_file: "D401".into(),
        mining_tool,
    });
    app.world_mut()
        .resource_mut::<QuestUiIntentQueue>()
        .push_intent(QuestUiIntent::AttackDirection {
            direction: "right".into(),
            mining: true,
            gesture_id: 7,
        });
    (app, receiver)
}

#[test]
fn mining_native_manual_input_or_ui_cancels_unsent_swing_immediately() {
    for case in [
        "right",
        "keyboard",
        "escape",
        "release",
        "npcDialog",
        "unfocused",
        "death",
        "equipChange",
        "mapChange",
    ] {
        let (mut app, _receiver) = app_with_mining_gesture();
        if case == "right" {
            // Cancellation should not need any downloaded map. Test the
            // escape through the ordinary unknown-map movement fallback.
            app.world_mut()
                .resource_mut::<NativeEntityPresentation>()
                .set_hover_grid_context_for_test((0, 0), (528., 352.));
        }
        match case {
            "right" => app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right),
            "keyboard" => app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ArrowRight),
            "escape" => app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Escape),
            "release" => app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left),
            "npcDialog" => app.world_mut().resource_mut::<NpcDialogModel>().is_open = true,
            "unfocused" => {
                let world = app.world_mut();
                world.query::<&mut Window>().single_mut(world).unwrap().focused = false;
            }
            "death" => app.world_mut().resource_mut::<UiReadModel>().player.hp = 0,
            "equipChange" => app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(701),
            "mapChange" => app.world_mut().resource_mut::<NativeEntityPresentation>().observe_packet_payload(
                serde_json::json!({"mapFileName":"D402", "sceneView":{"center":{"x":0,"y":0}}, "entities":[]}), 1),
            _ => unreachable!(),
        }
        app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
        app.update();
        assert!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .directional_attack
                .is_none(),
            "{case}"
        );
        assert!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents()
                .iter()
                .all(|intent| !matches!(intent, QuestUiIntent::AttackDirection { .. })),
            "{case}"
        );
    }
}

#[test]
fn mining_native_bridge_forwards_only_latest_current_gesture_without_spell() {
    let (mut app, receiver) = app_with_mining_gesture();
    app.world_mut().resource_mut::<QuestUiIntentQueue>().clear();
    app.world_mut()
        .resource_mut::<QuestUiIntentQueue>()
        .push_intent(QuestUiIntent::AttackDirection {
            direction: "left".into(),
            mining: true,
            gesture_id: 6,
        });
    app.world_mut()
        .resource_mut::<QuestUiIntentQueue>()
        .push_intent(QuestUiIntent::AttackDirection {
            direction: "right".into(),
            mining: true,
            gesture_id: 7,
        });
    app.add_systems(
        bevy::prelude::Update,
        crate::gameplay_bridge::forward_quest_ui_intents,
    );
    app.update();
    assert!(matches!(receiver.try_recv().unwrap(), GatewayCommand::Wire(
        NativeOutboundCommand::AttackDirection {direction, spell: None}) if direction == "right"));
    assert!(receiver.try_recv().is_err());
    app.world_mut()
        .resource_mut::<WorldPointerMovementState>()
        .directional_attack = None;
    app.world_mut()
        .resource_mut::<QuestUiIntentQueue>()
        .push_intent(QuestUiIntent::AttackDirection {
            direction: "right".into(),
            mining: true,
            gesture_id: 7,
        });
    app.update();
    assert!(
        receiver.try_recv().is_err(),
        "a stale queued request cannot revive a canceled gesture"
    );
}

#[test]
fn mining_native_saturated_sender_never_retains_an_old_directional_attack() {
    use crate::gateway::CommandSource;
    let (mut app, _unbounded) = app_with_mining_gesture();
    let (sender, mut receiver) = crate::gateway::command_channel(8);
    for _ in 0..8 {
        sender
            .send(GatewayCommand::Player(PlayerIntent::Walk {
                direction: "up".into(),
            }))
            .unwrap();
    }
    app.insert_resource(GatewayCommands::new(sender));
    app.add_systems(
        bevy::prelude::Update,
        crate::gameplay_bridge::forward_quest_ui_intents,
    );
    app.update();
    assert_eq!(app.world().resource::<QuestUiIntentQueue>().retry_len(), 0);
    assert!(app.world().resource::<QuestUiIntentQueue>().is_empty());
    let sent: Vec<_> = std::iter::from_fn(|| receiver.try_command().ok()).collect();
    assert!(sent.iter().all(|command| !matches!(
        command,
        GatewayCommand::Wire(NativeOutboundCommand::AttackDirection { .. })
    )));
    app.world_mut()
        .resource_mut::<WorldPointerMovementState>()
        .directional_attack = None;
    app.update();
    assert!(receiver.try_command().is_err());
}
