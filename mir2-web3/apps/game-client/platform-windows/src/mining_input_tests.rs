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
        owner: "1000".into(),
        owner_stamp: None,
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
    let stamp = sender.ownership_fence().unwrap().test_world_ready(1_000, 0);
    for _ in 0..8 {
        sender
            .send_with_stamp(
                GatewayCommand::Player(PlayerIntent::Walk {
                    direction: "up".into(),
                }),
                Some(stamp),
            )
            .unwrap();
    }
    let commands = GatewayCommands::new(sender);
    assert!(commands.activate_world_stamp(stamp));
    app.world_mut()
        .resource_mut::<WorldPointerMovementState>()
        .directional_attack
        .as_mut()
        .unwrap()
        .owner_stamp = Some(stamp);
    app.insert_resource(commands);
    app.add_systems(
        bevy::prelude::Update,
        crate::gameplay_bridge::forward_quest_ui_intents,
    );
    app.update();
    assert_eq!(app.world().resource::<QuestUiIntentQueue>().retry_len(), 0);
    assert!(app.world().resource::<QuestUiIntentQueue>().is_empty());
    let sent: Vec<_> = std::iter::from_fn(|| receiver.try_command().ok()).collect();
    assert_eq!(sent.len(), 8, "the actual bounded lane remained saturated");
    assert!(sent
        .iter()
        .all(|command| matches!(command, GatewayCommand::Owned(_))));
    assert!(sent.iter().all(|command| !matches!(
        command.payload_for_test(),
        GatewayCommand::Wire(NativeOutboundCommand::AttackDirection { .. })
    )));
    assert!(sent.iter().all(|command| matches!(
        command.payload_for_test(),
        GatewayCommand::Player(PlayerIntent::Walk { direction }) if direction == "up"
    )));
    app.world_mut()
        .resource_mut::<WorldPointerMovementState>()
        .directional_attack = None;
    app.update();
    assert!(receiver.try_command().is_err());
}

fn auto_hit_scene(app: &mut bevy::prelude::App, map: &str, direction: &str, cursor: (f32, f32)) {
    let mut presentation = app.world_mut().resource_mut::<NativeEntityPresentation>();
    presentation.set_hover_grid_context_for_test((24, 181), cursor);
    presentation.observe_packet_payload(
        serde_json::json!({
            "mapFileName": map, "sceneView": {"center": {"x": 24, "y": 181}},
            "entities": [{"objectId": "1000", "kind": "selfPlayer", "x": 24, "y": 181,
                "direction": direction, "dead": false, "sprite": {"bodyLibrary": "CArmour/00"}}]
        }),
        0,
    );
    presentation.set_hovered_object_id_for_test(None);
}

fn auto_hit_app(
    direction: &str,
) -> (
    bevy::prelude::App,
    std::sync::mpsc::Receiver<GatewayCommand>,
) {
    // Use the same public D401 collision pack as the native loader, not a
    // synthetic mining cell or an input/debug bypass.
    assert_eq!(
        crate::map_parser::map_cell_blocks_player_movement("D401", 24, 181),
        Some(false)
    );
    assert_eq!(
        crate::map_parser::map_cell_blocks_player_movement("D401", 23, 181),
        Some(true)
    );
    let (mut app, receiver) = app_with_mining_gesture();
    {
        let world = app.world_mut();
        *world.query::<&mut Window>().single_mut(world).unwrap() =
            stage_window(bevy::prelude::Vec2::new(456., 368.));
        world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
        world.resource_mut::<QuestUiIntentQueue>().clear();
        {
            let mut movement = world.resource_mut::<WorldPointerMovementState>();
            *movement = WorldPointerMovementState::default();
            movement.observe_identity("1000", (24, 181), direction);
        }
        let mut entities = world.resource_mut::<EntityModelSet>();
        let owner = entities
            .entities
            .iter_mut()
            .find(|entity| entity.kind == EntityKind::SelfPlayer)
            .unwrap();
        owner.x = 24;
        owner.y = 181;
        owner.direction = Some(direction.into());
    }
    app.world_mut().resource_mut::<UiReadModel>().player.level = 12;
    auto_hit_scene(&mut app, "D401", direction, (456., 368.));
    install_movement_clock_and_inbox(&mut app);
    app.init_resource::<bevy::ecs::message::Messages<bevy::window::WindowEvent>>();
    app.add_systems(
        bevy::prelude::Update,
        (
            mouse_world_interaction_system,
            crate::gameplay_bridge::forward_quest_ui_intents,
        )
            .chain(),
    );
    (app, receiver)
}

fn clear_pointer_edges(app: &mut bevy::prelude::App) {
    let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    for button in [MouseButton::Left, MouseButton::Right] {
        mouse.clear_just_pressed(button);
        mouse.clear_just_released(button);
    }
}

fn pulse_at_wall(app: &mut bevy::prelude::App) {
    ordered_pointer_motion(app, bevy::prelude::Vec2::new(456., 368.));
    ordered_pointer_button(app, MouseButton::Left, true);
    ordered_pointer_button(app, MouseButton::Left, false);
}

// UI ownership tests run just beyond the level-12 deadline. At the third
// exact 1232 ms step, elapsed_secs_f64() * 1000 is 3695.9999999999995 while
// the prior deadline is 3696. Keep exact cadence assertions in the dedicated
// AutoHit pacing tests below; this 1 ms margin does not change production time.
const MINING_UI_TICK_MS: u64 = 1233;

#[track_caller]
fn assert_only_mining_swing(
    app: &bevy::prelude::App,
    receiver: &std::sync::mpsc::Receiver<GatewayCommand>,
    context: &str,
) {
    let received = receiver.try_recv();
    let movement = app.world().resource::<WorldPointerMovementState>();
    assert!(matches!(&received, Ok(GatewayCommand::Wire(
        NativeOutboundCommand::AttackDirection { direction, spell: None })) if direction == "left"),
        "{context}: expected mining swing, got {received:?}; gesture={:?}, now_ms={:?}, deadline_ms={:?}",
        movement.directional_attack,
        movement_now_ms(Some(app.world().resource::<bevy::prelude::Time>()), None),
        movement.next_attack_request_at_ms);
    assert!(
        receiver.try_recv().is_err(),
        "{context}: bag input must not emit an additional world interaction"
    );
}

fn arm_mining(
    app: &mut bevy::prelude::App,
    receiver: &std::sync::mpsc::Receiver<GatewayCommand>,
) -> u64 {
    pulse_at_wall(app);
    app.update();
    assert_only_mining_swing(app, receiver, "arm AutoHit");
    clear_pointer_edges(app);
    app.world()
        .resource::<WorldPointerMovementState>()
        .directional_attack
        .as_ref()
        .unwrap()
        .id
}

#[test]
fn mining_inventory_native_bindings_toggle_and_close_preserve_the_same_auto_hit() {
    for (function, key, remap) in [
        ("Inventory", KeyCode::F9, false),
        ("Inventory2", KeyCode::KeyI, false),
        ("Inventory2", KeyCode::KeyB, true),
        ("Closeall", KeyCode::Escape, false),
        ("Closeall", KeyCode::KeyB, true),
    ] {
        let (mut app, receiver) = auto_hit_app("left");
        let id = arm_mining(&mut app, &receiver);
        if remap {
            let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
            ui.keyboard
                .bindings
                .retain(|binding| binding.function == function);
            ui.keyboard.bindings[0].key = "B".into();
        }
        // Input runs before the UI applies the binding. Test both the opening
        // edge (None) and closing edge (Inventory), then its resulting state.
        let panels = if function == "Closeall" {
            vec![mir2_ui_core::state::UiPanel::Inventory]
        } else {
            vec![
                mir2_ui_core::state::UiPanel::None,
                mir2_ui_core::state::UiPanel::Inventory,
            ]
        };
        for panel in panels {
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .core
                .panel = panel;
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
            advance_movement_clock(&mut app, MINING_UI_TICK_MS);
            app.update();
            assert_only_mining_swing(
                &app,
                &receiver,
                &format!("binding {function} {key:?} remap={remap} panel={panel:?}: key down"),
            );
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .core
                .panel = if panel == mir2_ui_core::state::UiPanel::None {
                mir2_ui_core::state::UiPanel::Inventory
            } else {
                mir2_ui_core::state::UiPanel::None
            };
            advance_movement_clock(&mut app, MINING_UI_TICK_MS);
            app.update();
            assert_only_mining_swing(
                &app,
                &receiver,
                &format!(
                    "binding {function} {key:?} remap={remap} panel={panel:?}: resulting UI state"
                ),
            );
            assert_eq!(
                app.world()
                    .resource::<WorldPointerMovementState>()
                    .directional_attack
                    .as_ref()
                    .unwrap()
                    .id,
                id,
                "{function} {key:?}"
            );
        }
        if function == "Closeall" {
            // The same Closeall binding with no bag is a stop/menu action.
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
            app.update();
            assert!(app
                .world()
                .resource::<WorldPointerMovementState>()
                .directional_attack
                .is_none());
            assert!(receiver.try_recv().is_err());
        }
    }
}

#[test]
fn mining_inventory_native_open_bag_allows_outside_start_and_passive_item_hover() {
    let (mut app, receiver) = auto_hit_app("left");
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::Inventory;
    let id = arm_mining(&mut app, &receiver);
    // The hover producer shields bag items from world targeting. AutoHit keeps
    // its established direction even with no world tile/entity under the mouse.
    ordered_pointer_motion(&mut app, bevy::prelude::Vec2::new(25., 50.));
    auto_hit_scene(&mut app, "D401", "left", (-1., -1.));
    advance_movement_clock(&mut app, MINING_UI_TICK_MS);
    app.update();
    assert_only_mining_swing(&app, &receiver, "passive bag item hover");
    assert_eq!(
        app.world()
            .resource::<WorldPointerMovementState>()
            .directional_attack
            .as_ref()
            .unwrap()
            .id,
        id
    );
}

#[test]
fn mining_inventory_native_ui_owned_pointer_never_becomes_a_world_hold() {
    for (button, point, panel) in [
        (
            MouseButton::Left,
            bevy::prelude::Vec2::new(25., 50.),
            mir2_ui_core::state::UiPanel::Inventory,
        ),
        (
            MouseButton::Right,
            bevy::prelude::Vec2::new(25., 50.),
            mir2_ui_core::state::UiPanel::Inventory,
        ),
        (
            MouseButton::Left,
            bevy::prelude::Vec2::new(938., 702.),
            mir2_ui_core::state::UiPanel::None,
        ),
        (
            MouseButton::Left,
            bevy::prelude::Vec2::new(305., 10.),
            mir2_ui_core::state::UiPanel::Inventory,
        ),
    ] {
        let (mut app, receiver) = auto_hit_app("left");
        let id = arm_mining(&mut app, &receiver);
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = panel;
        ordered_pointer_motion(&mut app, point);
        ordered_pointer_button(&mut app, button, true);
        // Same-frame down in the bag/HUD followed by a world cursor must still
        // belong to the UI; later held frames must not become right-click run.
        ordered_pointer_motion(&mut app, bevy::prelude::Vec2::new(456., 368.));
        for frame in 0..2 {
            advance_movement_clock(&mut app, MINING_UI_TICK_MS);
            app.update();
            assert_only_mining_swing(
                &app,
                &receiver,
                &format!("pointer {button:?} origin={point:?} panel={panel:?}: held frame {frame}"),
            );
            assert!(
                app.world()
                    .resource::<ButtonInput<MouseButton>>()
                    .pressed(button),
                "UI must retain its original input"
            );
            clear_pointer_edges(&mut app);
        }
        ordered_pointer_button(&mut app, button, false);
        advance_movement_clock(&mut app, MINING_UI_TICK_MS);
        app.update();
        assert_only_mining_swing(
            &app,
            &receiver,
            &format!("pointer {button:?} origin={point:?} panel={panel:?}: release"),
        );
        assert_eq!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .directional_attack
                .as_ref()
                .unwrap()
                .id,
            id
        );
        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .pending
            .is_empty());
    }
}

#[test]
fn mining_inventory_native_world_down_cannot_borrow_a_bag_release_and_ui_cannot_arm() {
    for already_mining in [false, true] {
        let (mut app, receiver) = auto_hit_app("left");
        if already_mining {
            arm_mining(&mut app, &receiver);
        }
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::Inventory;
        let bag = bevy::prelude::Vec2::new(25., 50.);
        let wall = bevy::prelude::Vec2::new(456., 368.);
        ordered_pointer_motion(&mut app, if already_mining { wall } else { bag });
        ordered_pointer_button(&mut app, MouseButton::Left, true);
        ordered_pointer_motion(&mut app, if already_mining { bag } else { wall });
        ordered_pointer_button(&mut app, MouseButton::Left, false);
        advance_movement_clock(&mut app, 1232);
        app.update();
        assert!(receiver.try_recv().is_err());
        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .directional_attack
            .is_none());
        clear_pointer_edges(&mut app);
        ordered_pointer_motion(&mut app, wall);
        advance_movement_clock(&mut app, 1232);
        app.update();
        assert!(receiver.try_recv().is_err());
    }
}

#[test]
fn mining_inventory_native_modal_transaction_and_rebound_other_action_still_cancel() {
    for case in ["inspect", "move", "trade", "otherBinding"] {
        let (mut app, receiver) = auto_hit_app("left");
        arm_mining(&mut app, &receiver);
        {
            let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
            ui.core.panel = mir2_ui_core::state::UiPanel::Inventory;
            match case {
                "inspect" => {
                    ui.inspect = Some(mir2_client_bevy::crystal_ui::overlays::ItemInspect {
                        container: 0,
                        slot: 0,
                        unique_id: Some(900),
                        key: "ore".into(),
                        name: "Ore".into(),
                        quantity: 1,
                    })
                }
                "move" => {
                    ui.inventory_operation = Some(
                        mir2_client_bevy::crystal_ui::overlays::InventoryOperationDraft::Move {
                            source_slot: 0,
                            unique_id: 900,
                        },
                    )
                }
                "trade" => ui.trade_dialog.open = true,
                "otherBinding" => {
                    ui.keyboard
                        .bindings
                        .retain(|binding| binding.function == "Inventory2");
                    ui.keyboard.bindings[0].function = "Options".into();
                }
                _ => unreachable!(),
            }
        }
        if case == "otherBinding" {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyI);
        }
        advance_movement_clock(&mut app, 1232);
        app.update();
        assert!(receiver.try_recv().is_err(), "{case}");
        assert!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .directional_attack
                .is_none(),
            "{case}"
        );
    }
}

#[test]
fn mining_inventory_bridge_preserves_bag_but_rechecks_late_transaction_guards() {
    for transaction in [false, true] {
        let (mut app, receiver) = app_with_mining_gesture();
        {
            let mut ui = app.world_mut().resource_mut::<NativePlayerUiState>();
            ui.core.panel = mir2_ui_core::state::UiPanel::Inventory;
            if transaction {
                ui.inventory_operation = Some(
                    mir2_client_bevy::crystal_ui::overlays::InventoryOperationDraft::Move {
                        source_slot: 0,
                        unique_id: 900,
                    },
                );
            }
        }
        // Deliberately omit the input system: a guard that changes after input
        // queued a swing must still be enforced by the actual sender.
        app.add_systems(
            bevy::prelude::Update,
            crate::gameplay_bridge::forward_quest_ui_intents,
        );
        app.update();
        if transaction {
            assert!(receiver.try_recv().is_err());
            assert!(app
                .world()
                .resource::<WorldPointerMovementState>()
                .directional_attack
                .is_none());
        } else {
            assert!(matches!(receiver.try_recv().unwrap(), GatewayCommand::Wire(
                NativeOutboundCommand::AttackDirection { direction, spell: None }) if direction == "right"));
            assert!(receiver.try_recv().is_err());
            assert_eq!(
                app.world()
                    .resource::<WorldPointerMovementState>()
                    .directional_attack
                    .as_ref()
                    .unwrap()
                    .id,
                7
            );
        }
    }
}

#[test]
fn mining_auto_hit_native_pulse_turns_then_needs_authoritative_ack_and_new_click() {
    let (mut app, receiver) = auto_hit_app("up");
    // Coalesced down/move/up: the final cursor points right, but only the
    // accepted down on the left wall owns this click's direction.
    ordered_pointer_motion(&mut app, bevy::prelude::Vec2::new(456., 368.));
    ordered_pointer_button(&mut app, MouseButton::Left, true);
    ordered_pointer_motion(&mut app, bevy::prelude::Vec2::new(552., 368.));
    ordered_pointer_button(&mut app, MouseButton::Left, false);
    auto_hit_scene(&mut app, "D401", "up", (552., 368.));
    app.update();
    assert!(
        matches!(receiver.try_recv().unwrap(), GatewayCommand::Player(
        PlayerIntent::Turn { direction }) if direction == "left")
    );
    assert!(receiver.try_recv().is_err());
    assert!(app
        .world()
        .resource::<WorldPointerMovementState>()
        .directional_attack
        .is_none());
    assert_eq!(
        app.world()
            .resource::<WorldPointerMovementState>()
            .authoritative_direction
            .as_deref(),
        Some("up")
    );

    clear_pointer_edges(&mut app);
    pulse_at_wall(&mut app);
    app.update();
    assert!(
        matches!(receiver.try_recv().unwrap(), GatewayCommand::Player(
        PlayerIntent::Turn { direction }) if direction == "left")
    );
    assert!(
        receiver.try_recv().is_err(),
        "another unacknowledged click must still only turn"
    );
    clear_pointer_edges(&mut app);
    app.world_mut().resource_mut::<EntityModelSet>().entities[0].direction = Some("left".into());
    push_test_movement_ack(&app, 24, 181, "left");
    app.update();
    assert!(
        receiver.try_recv().is_err(),
        "the turn ACK alone must not arm AutoHit"
    );
    assert!(app
        .world()
        .resource::<WorldPointerMovementState>()
        .directional_attack
        .is_none());

    pulse_at_wall(&mut app);
    app.update();
    assert!(matches!(receiver.try_recv().unwrap(), GatewayCommand::Wire(
        NativeOutboundCommand::AttackDirection { direction, spell: None }) if direction == "left"));
    assert!(receiver.try_recv().is_err());
    let id = app
        .world()
        .resource::<WorldPointerMovementState>()
        .directional_attack
        .as_ref()
        .unwrap()
        .id;
    assert!(!app
        .world()
        .resource::<ButtonInput<MouseButton>>()
        .pressed(MouseButton::Left));
    clear_pointer_edges(&mut app);

    // An established AutoHit must not acquire a new NPC or stop because its
    // cursor crossed passive HUD/left the window. It keeps the accepted wall.
    auto_hit_scene(&mut app, "D401", "left", (-1., -1.));
    app.world_mut()
        .resource_mut::<NativeEntityPresentation>()
        .set_hovered_object_id_for_test(Some("2001"));
    {
        let world = app.world_mut();
        world
            .query::<&mut Window>()
            .single_mut(world)
            .unwrap()
            .set_cursor_position(None);
    }
    advance_movement_clock(&mut app, 1231);
    app.update();
    assert!(
        receiver.try_recv().is_err(),
        "AutoHit retains Crystal's level-12 cooldown"
    );
    advance_movement_clock(&mut app, 1);
    app.update();
    assert!(matches!(receiver.try_recv().unwrap(), GatewayCommand::Wire(
        NativeOutboundCommand::AttackDirection { direction, spell: None }) if direction == "left"));
    assert!(receiver.try_recv().is_err());
    assert_eq!(
        app.world()
            .resource::<WorldPointerMovementState>()
            .directional_attack
            .as_ref()
            .unwrap()
            .id,
        id
    );
    {
        let world = app.world_mut();
        world
            .query::<&mut Window>()
            .single_mut(world)
            .unwrap()
            .set_cursor_position(Some(bevy::prelude::Vec2::new(512., 720.)));
        world
            .resource_mut::<NativePlayerUiState>()
            .status_hud
            .hovered = true;
    }
    advance_movement_clock(&mut app, 1232);
    app.update();
    assert!(matches!(receiver.try_recv().unwrap(), GatewayCommand::Wire(
        NativeOutboundCommand::AttackDirection { direction, spell: None }) if direction == "left"));
    assert!(receiver.try_recv().is_err());
    assert_eq!(
        app.world()
            .resource::<WorldPointerMovementState>()
            .directional_attack
            .as_ref()
            .unwrap()
            .id,
        id
    );
}

#[test]
fn mining_auto_hit_native_pulse_requires_world_press_origin_and_empty_down_tile() {
    for case in ["uiDown", "occupiedDown"] {
        let (mut app, receiver) = auto_hit_app("left");
        if case == "uiDown" {
            // Source Character HUD button (905,692,20,20), not an assumed
            // whole-footer hit box.
            ordered_pointer_motion(&mut app, bevy::prelude::Vec2::new(915., 702.));
        } else {
            app.world_mut()
                .resource_mut::<EntityModelSet>()
                .entities
                .push(EntityModel {
                    object_id: "2001".into(),
                    kind: EntityKind::Npc,
                    name: "Npc".into(),
                    x: 23,
                    y: 181,
                    level: None,
                    direction: None,
                });
            ordered_pointer_motion(&mut app, bevy::prelude::Vec2::new(456., 368.));
        }
        ordered_pointer_button(&mut app, MouseButton::Left, true);
        ordered_pointer_motion(&mut app, bevy::prelude::Vec2::new(456., 368.));
        ordered_pointer_button(&mut app, MouseButton::Left, false);
        app.update();
        assert!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .directional_attack
                .is_none(),
            "{case}"
        );
        assert!(
            std::iter::from_fn(|| receiver.try_recv().ok()).all(|command| !matches!(
                command,
                GatewayCommand::Wire(NativeOutboundCommand::AttackDirection { .. })
            )),
            "{case}"
        );
    }
}

#[test]
fn mining_auto_hit_native_cancellation_never_rearms_after_owner_ui_tool_or_focus_recovers() {
    for case in [
        "ui",
        "blur",
        "tool",
        "owner",
        "map",
        "origin",
        "death",
        "unknownState",
        "newMouse",
        "escape",
    ] {
        let (mut app, receiver) = auto_hit_app("left");
        pulse_at_wall(&mut app);
        app.update();
        assert!(matches!(
            receiver.try_recv().unwrap(),
            GatewayCommand::Wire(NativeOutboundCommand::AttackDirection { spell: None, .. })
        ));
        let id = app
            .world()
            .resource::<WorldPointerMovementState>()
            .directional_attack
            .as_ref()
            .unwrap()
            .id;
        clear_pointer_edges(&mut app);
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::AttackDirection {
                direction: "left".into(),
                mining: true,
                gesture_id: id,
            }));
        match case {
            "ui" => {
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .core
                    .panel = mir2_ui_core::state::UiPanel::Storage
            }
            "blur" => {
                let world = app.world_mut();
                world
                    .query::<&mut Window>()
                    .single_mut(world)
                    .unwrap()
                    .focused = false;
            }
            "tool" => {
                app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(701)
            }
            "owner" => {
                app.world_mut().resource_mut::<EntityModelSet>().entities[0].object_id =
                    "2000".into()
            }
            "map" => auto_hit_scene(&mut app, "D402", "left", (456., 368.)),
            "origin" => {
                app.world_mut().resource_mut::<EntityModelSet>().entities[0].x = 25;
                push_test_movement_ack(&app, 25, 181, "left");
            }
            "death" => app.world_mut().resource_mut::<UiReadModel>().player.hp = 0,
            "unknownState" => {
                app.world_mut()
                    .resource_mut::<NativeWorldClickState>()
                    .dazed = None
            }
            "newMouse" => app
                .world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Middle),
            "escape" => app
                .world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape),
            _ => unreachable!(),
        }
        app.update();
        assert!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .directional_attack
                .is_none(),
            "{case}"
        );
        assert!(
            receiver.try_recv().is_err(),
            "{case}: no stale swing may leave the bridge"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::None;
        app.world_mut().resource_mut::<InventoryModel>().items[0].unique_id = Some(700);
        app.world_mut().resource_mut::<UiReadModel>().player.hp = 100;
        app.world_mut()
            .resource_mut::<NativeWorldClickState>()
            .dazed = Some(false);
        {
            let world = app.world_mut();
            world
                .query::<&mut Window>()
                .single_mut(world)
                .unwrap()
                .focused = true;
            let mut entities = world.resource_mut::<EntityModelSet>();
            entities.entities[0].object_id = "1000".into();
            entities.entities[0].x = 24;
        }
        auto_hit_scene(&mut app, "D401", "left", (456., 368.));
        push_test_movement_ack(&app, 24, 181, "left");
        advance_movement_clock(&mut app, 2000);
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "{case}: recovery cannot replay the canceled click"
        );
        assert!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .directional_attack
                .is_none(),
            "{case}"
        );
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::AttackDirection {
                direction: "left".into(),
                mining: true,
                gesture_id: id,
            }));
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "{case}: an old queued id cannot rearm AutoHit"
        );
        assert!(
            app.world().resource::<QuestUiIntentQueue>().is_empty(),
            "{case}"
        );
    }
}

#[test]
fn mining_auto_hit_bridge_epoch_or_permission_failure_retires_the_established_gesture() {
    use crate::gateway::CommandSource;

    for case in ["reconnect", "unknownState", "dialog"] {
        let (mut app, _unbounded) = app_with_mining_gesture();
        let (sender, mut receiver) = crate::gateway::command_channel(8);
        let fence = sender.ownership_fence().unwrap();
        let stamp = fence.test_world_ready(1000, 0);
        let commands = GatewayCommands::new(sender);
        assert!(commands.activate_world_stamp(stamp));
        app.world_mut()
            .resource_mut::<WorldPointerMovementState>()
            .directional_attack
            .as_mut()
            .unwrap()
            .owner_stamp = Some(stamp);
        if case == "reconnect" {
            assert!(commands.activate_world_stamp(fence.test_reconnect(1000, 0)));
        } else if case == "unknownState" {
            app.world_mut()
                .resource_mut::<NativeWorldClickState>()
                .fishing = None;
        } else {
            app.world_mut().resource_mut::<NpcDialogModel>().is_open = true;
        }
        app.insert_resource(commands);
        app.add_systems(
            bevy::prelude::Update,
            crate::gameplay_bridge::forward_quest_ui_intents,
        );
        app.update();
        assert!(receiver.try_command().is_err(), "{case}");
        assert!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .directional_attack
                .is_none(),
            "{case}"
        );
        app.world_mut().resource_mut::<NpcDialogModel>().is_open = false;
        app.world_mut()
            .resource_mut::<NativeWorldClickState>()
            .fishing = Some(false);
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .push_intent(QuestUiIntent::AttackDirection {
                direction: "right".into(),
                mining: true,
                gesture_id: 7,
            }));
        app.update();
        assert!(
            receiver.try_command().is_err(),
            "{case}: a recovered sender cannot reuse the old id"
        );
        assert_eq!(app.world().resource::<QuestUiIntentQueue>().retry_len(), 0);
        assert!(app.world().resource::<QuestUiIntentQueue>().is_empty());
    }
}

#[test]
fn mining_auto_hit_native_alt_pulse_preserves_harvest_precedence_over_shift_and_mining() {
    for shift in [false, true] {
        let (mut app, receiver) = auto_hit_app("left");
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::AltLeft);
            if shift {
                keys.press(KeyCode::ShiftLeft);
            }
        }
        pulse_at_wall(&mut app);
        app.update();
        assert!(matches!(receiver.try_recv().unwrap(), GatewayCommand::Wire(
            NativeOutboundCommand::Harvest { direction }) if direction == "left"));
        assert!(
            receiver.try_recv().is_err(),
            "shift={shift}: Alt must not also turn or mine"
        );
        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .directional_attack
            .is_none());
        assert_eq!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .harvest_direction,
            Some("left")
        );
        assert!(app.world().resource::<QuestUiIntentQueue>().is_empty());

        clear_pointer_edges(&mut app);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        advance_movement_clock(&mut app, 3000);
        app.update();
        assert!(
            receiver.try_recv().is_err(),
            "a released Alt pulse cannot become AutoHit later"
        );
        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .directional_attack
            .is_none());
        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .harvest_direction
            .is_none());
        assert!(app.world().resource::<QuestUiIntentQueue>().is_empty());
    }
}
