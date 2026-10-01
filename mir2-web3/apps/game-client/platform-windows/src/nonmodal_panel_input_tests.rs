use super::*;
use bevy::prelude::Vec2;
use mir2_client_bevy::skill_model::{SkillBinding, SkillEntry};

fn panel_ui(panel: mir2_ui_core::state::UiPanel) -> NativePlayerUiState {
    let mut ui = NativePlayerUiState::default();
    if panel == mir2_ui_core::state::UiPanel::Skill {
        ui.toggle_skill();
    } else {
        ui.core.panel = panel;
    }
    ui
}

#[test]
fn actual_skills_and_other_ordinary_panels_allow_outside_world_clicks_at_native_scales() {
    for (panel, inside) in [
        (mir2_ui_core::state::UiPanel::Inventory, Vec2::new(20.0, 60.0)),
        (mir2_ui_core::state::UiPanel::Character, Vec2::new(780.0, 120.0)),
        (mir2_ui_core::state::UiPanel::Skill, Vec2::new(780.0, 120.0)),
        (mir2_ui_core::state::UiPanel::Options, Vec2::new(400.0, 300.0)),
        (mir2_ui_core::state::UiPanel::Menu, Vec2::new(1000.0, 400.0)),
        (mir2_ui_core::state::UiPanel::QuestLog, Vec2::new(220.0, 200.0)),
    ] {
        for scale in [1.0, 1.5, 2.0] {
            for (cursor, should_move) in [(inside, false), (Vec2::new(700.0, 500.0), true)] {
                let (mut app, receiver) = input_app();
                install_movement_clock_and_inbox(&mut app);
                let mut window = stage_window(cursor * scale);
                window.resolution.set(1024.0 * scale, 768.0 * scale);
                app.world_mut().spawn(window);
                app.insert_resource(ButtonInput::<MouseButton>::default());
                app.insert_resource(panel_ui(panel));
                app.insert_resource(NpcDialogModel::default());
                app.insert_resource(UiReadModel::default());
                app.insert_resource(movement_entities());
                let mut presentation = NativeEntityPresentation::default();
                presentation.set_hover_grid_context_for_test((10, 10), (576.0, 352.0));
                app.insert_resource(presentation);
                app.init_resource::<QuestUiIntentQueue>();
                app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
                app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
                app.update();
                let walked = receiver.try_iter().any(|c| matches!(c, GatewayCommand::Player(PlayerIntent::Walk { .. })));
                assert_eq!(walked, should_move, "{panel:?} scale={scale} cursor={cursor:?}");
                assert!(drain_world_intents(&mut app).is_empty(), "panel leaked an unrelated world action");
            }
        }
    }
}

#[test]
fn ordinary_panels_do_not_disable_keyboard_walk() {
    for panel in [
        mir2_ui_core::state::UiPanel::Inventory,
        mir2_ui_core::state::UiPanel::Character,
        mir2_ui_core::state::UiPanel::Skill,
        mir2_ui_core::state::UiPanel::Options,
        mir2_ui_core::state::UiPanel::Menu,
        mir2_ui_core::state::UiPanel::QuestLog,
    ] {
        let (mut app, receiver) = input_app();
        app.insert_resource(panel_ui(panel));
        app.world_mut().spawn(stage_window(Vec2::new(780.0, 120.0)));
        app.add_systems(bevy::prelude::Update, keyboard_movement_system);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW);
        app.update();
        assert!(matches!(receiver.try_recv(), Ok(GatewayCommand::Player(PlayerIntent::Walk { .. }))), "{panel:?} froze keyboard walking");
    }
}

#[test]
fn three_class_skill_shortcuts_still_send_with_the_real_skills_page_visible() {
    for spell in ["FireBall", "SoulFire", "ShoulderDash"] {
        let (mut app, receiver) = input_app();
        app.insert_resource(panel_ui(mir2_ui_core::state::UiPanel::Skill));
        app.insert_resource(world_entities());
        app.insert_resource(UiReadModel {
            player: mir2_client_bevy::read_model::PlayerStats { hp: 20, max_hp: 20, mp: 100, ..Default::default() },
            ..Default::default()
        });
        app.insert_resource(SkillModel {
            skills: vec![SkillEntry { id: 7, name: spell.into(), level: 1, key: Some(spell.into()), cooldown_ms: 1200, mp_cost: 0 }],
            bindings: vec![SkillBinding { skill_id: 7, spell: Some(spell.into()), hotkey: Some(1), cast_kind: Some("target".into()), offensive: Some(true), ..Default::default() }],
            ..Default::default()
        });
        app.add_systems(bevy::prelude::Update, keyboard_skill_system);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::F1);
        app.update();
        assert!(matches!(receiver.try_recv(), Ok(GatewayCommand::Wire(NativeOutboundCommand::Magic { spell: sent, .. })) if sent == spell), "{spell} was blocked by its list panel");
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().reset_all();
        app.world_mut().resource_mut::<NativePlayerUiState>().skill_assign.open = true;
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::F1);
        app.update();
        assert!(receiver.try_recv().is_err(), "assignment prompt leaked {spell}");
        app.world_mut().resource_mut::<NativePlayerUiState>().skill_assign.open = false;
        app.insert_resource(NpcDialogModel { is_open: true, ..Default::default() });
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().reset_all();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::F1);
        app.update();
        assert!(receiver.try_recv().is_err(), "NPC service leaked {spell}");
    }
}

#[test]
fn potion_shortcut_works_in_the_skills_page_but_chat_keeps_the_key() {
    let (mut app, receiver) = input_app();
    app.insert_resource(panel_ui(mir2_ui_core::state::UiPanel::Skill));
    let mut inventory = InventoryModel::default();
    inventory.items.push(mir2_client_bevy::inventory::ItemModel {
        container: 1, slot: 0, quantity: 2, unique_id: Some(17),
        key: "test-potion".into(), name: "Health Potion".into(), ..Default::default()
    });
    app.insert_resource(inventory);
    app.add_systems(bevy::prelude::Update, keyboard_skill_system);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();
    assert!(matches!(receiver.try_recv(), Ok(GatewayCommand::Wire(NativeOutboundCommand::UseItem { slot: Some(0), grid: Some(grid), .. })) if grid == "belt"));
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().reset_all();
    app.world_mut().resource_mut::<NativePlayerUiState>().core.chat_focused = true;
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();
    assert!(receiver.try_recv().is_err());
}

#[test]
fn an_idle_positive_cooldown_snapshot_expires_for_real_keyboard_dispatch() {
    let (mut app, receiver) = input_app();
    let skills: SkillModel = serde_json::from_value(serde_json::json!({
        "authority":{"sessionEpoch":1,"playerObjectId":1000,"snapshotSerial":4},
        "skills":[{"id":7,"name":"FireBall","spell":"FireBall","hotkey":1,
            "castKind":"target","cooldownRemainingTicks":2,"cooldownRemainingMs":1800}]
    })).unwrap();
    let mut ui = panel_ui(mir2_ui_core::state::UiPanel::Skill);
    ui.skill_bars.observe(&skills, std::time::Instant::now() - std::time::Duration::from_secs(5));
    app.insert_resource(ui);
    app.insert_resource(skills);
    app.insert_resource(UiReadModel {
        player: mir2_client_bevy::read_model::PlayerStats { hp: 20, max_hp: 20, ..Default::default() },
        ..Default::default()
    });
    app.add_systems(bevy::prelude::Update, keyboard_skill_system);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::F1);
    app.update();
    assert!(matches!(receiver.try_recv(), Ok(GatewayCommand::Wire(NativeOutboundCommand::Magic { spell, .. })) if spell == "FireBall"), "an unchanged expired snapshot locked F1 forever");
}

#[test]
fn hud_skills_press_consumes_the_pointer_without_dropping_selected_attack() {
    let (mut app, receiver) = input_app();
    install_movement_clock_and_inbox(&mut app);
    let (x, y) = spec::hud::SKILL.rect.center();
    app.world_mut().spawn(stage_window(Vec2::new(x, y)));
    app.insert_resource(ButtonInput::<MouseButton>::default());
    app.insert_resource(NativePlayerUiState::default());
    app.insert_resource(NpcDialogModel::default());
    app.insert_resource(UiReadModel::default());
    app.insert_resource(world_entities());
    app.insert_resource(NativeEntityPresentation::default());
    app.init_resource::<QuestUiIntentQueue>();
    app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
    // A real HUD press runs the production identity initialization; an idle
    // frame intentionally exits early without initializing movement state.
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().reset_all();
    app.world_mut().resource_mut::<WorldPointerMovementState>().pursue_attack_target(2001);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    app.update();
    assert_eq!(app.world().resource::<WorldPointerMovementState>().attack_target(), Some(2001));
    assert!(receiver.try_recv().is_err(), "HUD press leaked a world command");
    assert!(drain_world_intents(&mut app).is_empty());
}

#[test]
fn map_search_owns_keyboard_even_though_ordinary_map_is_nonmodal() {
    for (focused, should_move) in [(true, false), (false, true)] {
        let (mut app, receiver) = input_app();
        let mut ui = NativePlayerUiState::default();
        ui.core.panel = mir2_ui_core::state::UiPanel::BigMap;
        app.insert_resource(ui);
        let mut map = mir2_client_bevy::crystal_ui::overlays::BigMapUiState::default();
        map.search_focused = focused;
        app.insert_resource(map);
        app.world_mut().spawn(stage_window(Vec2::new(500.0, 300.0)));
        app.add_systems(bevy::prelude::Update, keyboard_movement_system);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW);
        app.update();
        let moved = receiver.try_iter().any(|command| matches!(command,
            GatewayCommand::Player(PlayerIntent::Walk { .. })));
        assert_eq!(moved, should_move, "search_focused={focused}");
    }
}

#[test]
fn npc_service_dialog_owns_keyboard_independently_of_the_skills_panel() {
    for open in [true, false] {
        let (mut app, receiver) = input_app();
        app.insert_resource(panel_ui(mir2_ui_core::state::UiPanel::Skill));
        app.insert_resource(NpcDialogModel { is_open: open, ..Default::default() });
        app.world_mut().spawn(stage_window(Vec2::new(700.0, 500.0)));
        app.add_systems(bevy::prelude::Update, keyboard_movement_system);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW);
        app.update();
        let moved = receiver.try_iter().any(|command| matches!(command,
            GatewayCommand::Player(PlayerIntent::Walk { .. })));
        assert_eq!(moved, !open, "independent NPC dialog is_open={open}");
    }
}
