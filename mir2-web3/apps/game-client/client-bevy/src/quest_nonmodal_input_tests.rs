//! Headless production UI/input regressions; no live account or window.

use super::*;
use mir2_ui_core::state::{UiPanel, UiScreen};

fn fixture() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>()
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .add_plugins(Mir2QuestUiPlugin);
    app.update();
    app.world_mut().resource_mut::<NativePlayerUiState>().core.screen = UiScreen::InGame;
    app.world_mut().resource_mut::<QuestTracker>().active_quests.push(Quest {
        quest_index: 7,
        accept_npc_index: Some(10),
        finish_npc_index: Some(11),
        title: "Patrol fixture".into(),
        npc_name: Some("Guard".into()),
        group: Some("BichonProvince".into()),
        min_level_needed: 1,
        detail: default(),
        status: crate::quest_model::QuestStatus::InProgress,
        objectives: Vec::new(),
        rewards: Vec::new(),
        unknown_text: None,
    });
    app.world_mut().resource_mut::<NativePlayerUiState>().core.panel = UiPanel::QuestLog;
    app.update();
    app
}

fn node<T: Component>(app: &mut App) -> Node {
    let world = app.world_mut();
    world.query_filtered::<&Node, With<T>>().single(world).unwrap().clone()
}

fn press(app: &mut App, keys: &[KeyCode]) {
    let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    input.reset_all();
    for key in keys {
        input.press(*key);
    }
}

fn click_detail_row(app: &mut App) {
    let world = app.world_mut();
    let button = world.query::<(Entity, &QuestUiButton)>().iter(world)
        .find_map(|(entity, action)| matches!(action,
            QuestUiButton::SelectQuest { quest_index: 7 }).then_some(entity))
        .expect("rendered Diary row");
    world.entity_mut(button).insert(Interaction::Pressed);
    app.update();
    assert_eq!(app.world().resource::<QuestUiState>().detail_quest_index, Some(7));
}

fn assert_frame<T: Component>(app: &mut App, asset: &str, left: f32, top: f32) {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(&Node, &ImageNode), With<T>>();
    let (node, image) = query.single(world).unwrap();
    assert_eq!(node.display, Display::Flex);
    assert_eq!((node.left, node.top, node.width, node.height),
        (Val::Px(left), Val::Px(top), Val::Px(316.0), Val::Px(466.0)));
    assert_eq!(image.image.path().unwrap().to_string(), asset);
}

#[test]
fn rendered_diary_and_detail_capture_only_their_source_frame_bounds() {
    let mut app = fixture();
    click_detail_row(&mut app);
    assert_frame::<QuestLogPanel>(&mut app, "original-ui/Prguse/961.png", 192.0, 60.0);
    assert_frame::<QuestDetailPanel>(&mut app, "original-ui/Prguse/960.png", 532.0, 60.0);
    assert_eq!(node::<QuestUiModalBlocker>(&mut app).display, Display::None);
    assert_eq!(node::<QuestConfirmationBlocker>(&mut app).display, Display::None);

    let state = app.world().resource::<QuestUiState>();
    assert!(!state.blocks_world_input());
    for (x, y, captured) in [
        (192.0, 60.0, true), (507.0, 525.0, true),
        (508.0, 100.0, false), (200.0, 526.0, false),
        (532.0, 60.0, true), (847.0, 525.0, true),
        (848.0, 100.0, false), (900.0, 600.0, false),
    ] {
        assert_eq!(state.captures_world_pointer_at(x, y, true), captured, "{x},{y}");
    }
    assert!(!blocks_gameplay_input(
        Some(app.world().resource::<NativePlayerUiState>()),
        app.world().resource::<NpcDialogModel>(),
    ));

    press(&mut app, &[KeyCode::Tab]);
    app.update();
    assert!(app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents()
        .contains(&QuestUiIntent::PickUpTile), "Diary must keep ordinary keyboard pickup");
}

#[test]
fn q_and_escape_hide_only_diary_while_independent_detail_keeps_its_pointer_bounds() {
    for close_key in [KeyCode::KeyQ, KeyCode::Escape] {
        let mut app = fixture();
        click_detail_row(&mut app);
        press(&mut app, &[close_key]);
        app.update();
        assert!(!app.world().resource::<NativePlayerUiState>().quest_open());
        assert_eq!(node::<QuestLogPanel>(&mut app).display, Display::None);
        assert_frame::<QuestDetailPanel>(&mut app, "original-ui/Prguse/960.png", 532.0, 60.0);
        assert_eq!(node::<QuestUiModalBlocker>(&mut app).display, Display::None);
        let state = app.world().resource::<QuestUiState>();
        assert!(!state.blocks_world_input());
        assert!(!state.captures_world_pointer_at(200.0, 100.0, false));
        assert!(state.captures_world_pointer_at(600.0, 100.0, false));
        assert!(!state.captures_world_pointer_at(900.0, 600.0, false));

        press(&mut app, &[KeyCode::KeyQ]);
        app.update();
        assert!(app.world().resource::<NativePlayerUiState>().quest_open());
        assert_eq!(app.world().resource::<QuestUiState>().detail_quest_index, Some(7));
    }
}

#[test]
fn actual_confirmation_and_alert_keep_full_stage_and_keyboard_capture() {
    for alert in [false, true] {
        let mut app = fixture();
        click_detail_row(&mut app);
        if alert {
            app.world_mut().resource_mut::<QuestUiState>().show_quest_alert("Select a reward");
        } else {
            app.world_mut().resource_mut::<QuestUiState>().request_abandon_confirmation(7);
        }
        app.update();
        let blocker = node::<QuestConfirmationBlocker>(&mut app);
        assert_eq!(blocker.display, Display::Flex);
        assert_eq!((blocker.width, blocker.height), (Val::Percent(100.0), Val::Percent(100.0)));
        assert_eq!(node::<QuestConfirmationPanel>(&mut app).display, Display::Flex);
        let state = app.world().resource::<QuestUiState>();
        assert!(state.blocks_world_input());
        assert!(state.captures_world_pointer_at(900.0, 600.0, true));

        press(&mut app, &[KeyCode::Tab, KeyCode::KeyQ]);
        app.update();
        assert!(app.world().resource::<NativePlayerUiState>().quest_open());
        assert!(app.world().resource::<QuestUiState>().blocks_world_input());
        assert!(app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents().is_empty());

        press(&mut app, &[KeyCode::Escape]);
        app.update();
        assert!(!app.world().resource::<QuestUiState>().blocks_world_input());
        assert!(app.world().resource::<NativePlayerUiState>().quest_open());
        assert_eq!(app.world().resource::<QuestUiState>().detail_quest_index, Some(7));
        assert_eq!(node::<QuestConfirmationBlocker>(&mut app).display, Display::None);
    }
}

#[test]
fn npc_dialog_and_native_transactions_keep_their_existing_global_guard() {
    let mut app = fixture();
    app.world_mut().resource_mut::<NpcDialogModel>().is_open = true;
    app.update();
    assert_eq!(node::<QuestUiModalBlocker>(&mut app).display, Display::Flex);
    assert!(blocks_gameplay_input(
        Some(app.world().resource::<NativePlayerUiState>()),
        app.world().resource::<NpcDialogModel>(),
    ));
    press(&mut app, &[KeyCode::Tab]);
    app.update();
    assert!(app.world_mut().resource_mut::<QuestUiIntentQueue>().drain_intents().is_empty());

    let closed_dialog = NpcDialogModel::default();
    let mut native = NativePlayerUiState::default();
    native.core.screen = UiScreen::InGame;
    native.core.panel = UiPanel::NpcShop;
    assert!(blocks_gameplay_input(Some(&native), &closed_dialog));
    let npc_list = QuestUiState { npc_quest_list_open: true, ..default() };
    assert!(npc_list.captures_world_pointer_at(900.0, 600.0, false));
}

#[test]
fn authoritative_removal_clears_hidden_detail_and_diary_selection_capture() {
    for completed_snapshot in [false, true] {
        let mut app = fixture();
        click_detail_row(&mut app);
        assert_eq!(app.world().resource::<QuestUiState>().selected_quest_index, Some(7));
        if completed_snapshot {
            app.world_mut().resource_mut::<QuestTracker>().complete(
                crate::quest_model::QuestCompleteUpdate {
                    quest_index: 7,
                    status: crate::quest_model::QuestStatus::Completed,
                    rewards: Vec::new(),
                    unknown_text: None,
                },
            );
            // A present completed quest still renders its Detail. The next
            // authoritative current-quest snapshot removes it after turn-in.
            app.update();
            assert_eq!(node::<QuestDetailPanel>(&mut app).display, Display::Flex);
            app.world_mut().resource_mut::<QuestTracker>().apply(
                crate::quest_model::QuestSnapshot { quests: Vec::new() },
            );
        } else {
            assert!(app.world_mut().resource_mut::<QuestTracker>().remove(7).is_some());
        }
        app.update();
        assert_eq!(node::<QuestDetailPanel>(&mut app).display, Display::None);
        let state = app.world().resource::<QuestUiState>();
        assert_eq!(state.detail_quest_index, None);
        assert_eq!(state.selected_quest_index, None);
        assert!(!state.captures_world_pointer_at(600.0, 100.0, true));
        assert!(!state.blocks_world_input());
        // The Diary remains a real visible window with its own rectangle.
        assert!(state.captures_world_pointer_at(200.0, 100.0, true));
    }
}
