use super::super::tests::{help_button_test_app, press_help_button};

fn shop_good(id: u64, name: &str, price: u32, stock: i32) -> ShopGood {
    ShopGood {
        unique_id: id,
        name: name.into(),
        price,
        count: 1,
        stock,
        ..default()
    }
}

fn catalog(count: usize) -> ShopModel {
    ShopModel {
        service_mode: NpcShopServiceMode::Buy,
        goods: (1..=count)
            .map(|id| {
                if id == count {
                    shop_good(836, "PickAxe", 2500, 99)
                } else {
                    shop_good(id as u64, "WoodenSword", 50, 99)
                }
            })
            .collect(),
        ..default()
    }
}

fn app(count: usize, width: u32, height: u32) -> (App, Entity) {
    let mut app = App::new();
    let mut state = NativePlayerUiState::default();
    state.core.panel = mir2_ui_core::state::UiPanel::NpcShop;
    state.npc_shop_buy_tab = true;
    app.insert_resource(state)
        .insert_resource(catalog(count))
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        })
        .init_resource::<ShopUiState>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<MouseWheel>()
        .add_systems(Update, process);
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: bevy::window::WindowResolution::new(width, height),
                focused: true,
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    cursor(&mut app, window, 80., 100.);
    (app, window)
}

fn cursor(app: &mut App, window: Entity, x: f32, y: f32) {
    let mut window = app.world_mut().get_mut::<Window>(window).unwrap();
    let fit = super::super::super::metrics::CrystalStageTransform::fit_native(
        window.width(),
        window.height(),
    );
    let (x, y) = fit.logical_to_physical(
        x + NPC_GOODS_PANEL_ORIGIN.x as f32,
        y + NPC_GOODS_PANEL_ORIGIN.y as f32,
    );
    window.set_cursor_position(Some(Vec2::new(x, y)));
}

fn wheel(app: &mut App, window: Entity, y: f32, unit: MouseScrollUnit) {
    app.world_mut().write_message(MouseWheel {
        unit,
        x: 0.,
        y,
        window,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update();
}

#[test]
fn npc_shop_wheel_and_arrows_reach_the_last_pickaxe_without_money_or_world_commands() {
    for count in [11, 19] {
        for (width, height) in [(1024, 768), (1600, 900), (800, 1000)] {
            let (mut app, window) = app(count, width, height);
            app.world_mut().resource_mut::<ShopModel>().selected_id = Some(1);
            wheel(&mut app, window, -100., MouseScrollUnit::Line);
            assert_eq!(
                app.world().resource::<ShopUiState>().start_index,
                count - ROWS
            );
            let shop = app.world().resource::<ShopModel>();
            assert!(shop.selected_id.is_none());
            assert_eq!(
                shop.goods[app.world().resource::<ShopUiState>().start_index + 7].unique_id,
                836
            );
            // Fractional touchpad motion contributes exactly one row.
            let window_ref = app.world().get::<Window>(window).unwrap();
            let half_row = 16.
                * super::super::super::metrics::CrystalStageTransform::fit_native(
                    window_ref.width(),
                    window_ref.height(),
                )
                .scale;
            wheel(&mut app, window, half_row, MouseScrollUnit::Pixel);
            assert_eq!(
                app.world().resource::<ShopUiState>().start_index,
                count - ROWS
            );
            wheel(&mut app, window, half_row, MouseScrollUnit::Pixel);
            assert_eq!(
                app.world().resource::<ShopUiState>().start_index,
                count - ROWS - 1
            );
            wheel(&mut app, window, 100., MouseScrollUnit::Line);
            assert_eq!(app.world().resource::<ShopUiState>().start_index, 0);
        }
        let mut app = help_button_test_app();
        app.insert_resource(catalog(count));
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::NpcShop;
        for _ in 0..count {
            press_help_button(&mut app, OverlayButton::ShopPageDown);
        }
        assert_eq!(
            app.world().resource::<ShopUiState>().start_index,
            count - ROWS
        );
        for _ in 0..count {
            press_help_button(&mut app, OverlayButton::ShopPageUp);
        }
        assert_eq!(app.world().resource::<ShopUiState>().start_index, 0);
        assert!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents()
                .is_empty(),
            "scrolling cannot purchase, interact with an NPC or move the character"
        );
    }
}

#[test]
fn npc_shop_track_and_thumb_drag_reach_both_bounds_and_cancel_when_closed() {
    let (mut app, window) = app(19, 1600, 900);
    cursor(&mut app, window, 224., 58.);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear_just_pressed(MouseButton::Left);
    cursor(&mut app, window, 224., 500.);
    app.update();
    assert_eq!(app.world().resource::<ShopUiState>().start_index, 11);
    cursor(&mut app, window, 224., -20.);
    app.update();
    assert_eq!(app.world().resource::<ShopUiState>().start_index, 0);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::None;
    app.update();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::NpcShop;
    cursor(&mut app, window, 224., 280.);
    app.update();
    assert_eq!(
        app.world().resource::<ShopUiState>().start_index,
        0,
        "a held drag cannot resume after reopening"
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert_eq!(
        app.world().resource::<ShopUiState>().start_index,
        11,
        "a fresh track click selects the last page"
    );
}

#[test]
fn npc_shop_scroll_ignores_outside_unfocused_foreign_modal_and_repair_events() {
    for blocked in [
        "outside", "focus", "window", "modal", "notice", "repair", "closed", "nan",
    ] {
        let (mut app, window) = app(19, 1024, 768);
        let mut event_window = window;
        let mut y = -3.;
        match blocked {
            "outside" => cursor(&mut app, window, 250., 100.),
            "focus" => app.world_mut().get_mut::<Window>(window).unwrap().focused = false,
            "window" => event_window = app.world_mut().spawn_empty().id(),
            "modal" => {
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .npc_service_notice = Some("Notice".into())
            }
            "notice" => {
                let mut notice = crate::crystal_ui::notice::NoticeDialogState::default();
                notice.observe(crate::crystal_ui::notice::NoticePacketUpdate {
                    generation: 1,
                    sequence: 1,
                    title: "Notice".into(),
                    message: "Blocking notice".into(),
                });
                app.insert_resource(notice);
            }
            "repair" => {
                app.world_mut().resource_mut::<ShopModel>().service_mode =
                    NpcShopServiceMode::Repair
            }
            "closed" => {
                app.world_mut()
                    .resource_mut::<NativePlayerUiState>()
                    .core
                    .panel = mir2_ui_core::state::UiPanel::None
            }
            "nan" => y = f32::NAN,
            _ => unreachable!(),
        }
        wheel(&mut app, event_window, y, MouseScrollUnit::Line);
        assert_eq!(
            app.world().resource::<ShopUiState>().start_index,
            0,
            "{blocked}"
        );
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .core
            .panel = mir2_ui_core::state::UiPanel::NpcShop;
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .npc_service_notice = None;
        app.world_mut()
            .remove_resource::<crate::crystal_ui::notice::NoticeDialogState>();
        app.world_mut().resource_mut::<ShopModel>().service_mode = NpcShopServiceMode::Buy;
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        cursor(&mut app, window, 80., 100.);
        app.update();
        assert_eq!(
            app.world().resource::<ShopUiState>().start_index,
            0,
            "old wheel event must not leak later: {blocked}"
        );
    }
    for count in [0, 1, 8] {
        let (mut app, window) = app(count, 1024, 768);
        wheel(&mut app, window, -100., MouseScrollUnit::Line);
        assert_eq!(app.world().resource::<ShopUiState>().start_index, 0);
    }
}

#[test]
fn npc_shop_held_drag_cannot_resume_after_notice_npc_or_map_change() {
    for changed in ["notice", "npc", "map"] {
        let (mut app, window) = app(19, 1024, 768);
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(BigMapModel::default());
        cursor(&mut app, window, 224., 58.);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        match changed {
            "notice" => {
                let mut notice = crate::crystal_ui::notice::NoticeDialogState::default();
                notice.observe(crate::crystal_ui::notice::NoticePacketUpdate {
                    generation: 1,
                    sequence: 1,
                    title: "Notice".into(),
                    message: "Blocking notice".into(),
                });
                app.insert_resource(notice);
            }
            "npc" => {
                app.world_mut()
                    .resource_mut::<NpcDialogModel>()
                    .npc_object_id = Some(436)
            }
            "map" => app.world_mut().resource_mut::<BigMapModel>().reset_epoch += 1,
            _ => unreachable!(),
        }
        cursor(&mut app, window, 224., 280.);
        wheel(&mut app, window, -10., MouseScrollUnit::Line);
        // A changed NPC/map can accept fresh wheel input but never reuse the
        // old thumb grab; a modal discards both inputs until it closes.
        assert_eq!(
            app.world().resource::<ShopUiState>().start_index,
            if changed == "notice" { 0 } else { 10 },
            "{changed}"
        );
        app.world_mut()
            .remove_resource::<crate::crystal_ui::notice::NoticeDialogState>();
        app.world_mut().resource_mut::<ShopUiState>().start_index = 0;
        app.update();
        assert_eq!(
            app.world().resource::<ShopUiState>().start_index,
            0,
            "held drag and discarded wheel cannot resume: {changed}"
        );
    }
}
