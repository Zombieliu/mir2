//! Regression against the actual shared dialog tree and Bevy's focus system.
//! Authored box geometry is supplied headlessly; native pixels/touch remain a
//! separate APK acceptance gate. No duplicate shop reducer or purchase rules.
use super::*;
use bevy::app::HierarchyPropagatePlugin;
use bevy::ecs::system::RunSystemOnce;
use bevy::input::touch::Touches;
use bevy::ui::{ComputedUiTargetCamera, UiGlobalTransform, UiScale, UiStack, UiTargetCamera};
use bevy::window::{PrimaryWindow, WindowRef};

fn dialog_app(page: usize) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(PostUpdate),
    ))
    .init_resource::<UiScale>()
    .init_resource::<UiStack>()
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<Touches>()
    .add_systems(Startup, move |mut commands: Commands| {
        let window = commands.spawn((Window::default(), PrimaryWindow)).id();
        let camera = commands
            .spawn((
                Camera2d,
                Camera::default(),
                bevy::camera::RenderTarget::Window(WindowRef::Entity(window)),
            ))
            .id();
        let model = GameShopModel {
            items: (0..105)
                .map(|i| GameShopEntry {
                    game_shop_index: 2000 + i,
                    item_index: 1000 + i,
                item_name: format!("Catalog row {i:03}"),
                    category: "Regression".into(),
                    gold_price: 100,
                    can_buy_gold: true,
                    ..default()
                })
                .collect(),
            ..default()
        };
        let mut state = NativePlayerUiState::default();
        state.game_shop_page = page;
        state.game_shop_dialog.class = Some(0);
        commands
            .spawn((
                Node {
                    width: Val::Px(696.0),
                    height: Val::Px(476.0),
                    ..default()
                },
                UiTargetCamera(camera),
                FocusPolicy::Pass,
            ))
            .with_children(|parent| {
                render(
                    parent,
                    None,
                    &model,
                    &UiReadModel::default(),
                    &state,
                    &InventoryModel::default(),
                    None,
                )
            });
    })
    .add_systems(Update, bevy::ui::update::propagate_ui_target_cameras);
    app.update();
    app.update();
    // Retain real components, hierarchy and paint order from the dialog.
    // Only deterministic authored geometry replaces GPU/text layout here.
    let roots: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, (With<Node>, Without<ChildOf>)>()
        .iter(app.world())
        .collect();
    let mut paint_order = Vec::new();
    for root in roots {
        apply_authored_boxes(app.world_mut(), root, Vec2::ZERO, &mut paint_order);
    }
    app.world_mut().insert_resource(UiStack {
        partition: vec![0..paint_order.len()],
        uinodes: paint_order,
    });
    app
}

fn pixels(value: Val) -> f32 {
    if let Val::Px(value) = value {
        value
    } else {
        0.0
    }
}

fn apply_authored_boxes(
    world: &mut World,
    entity: Entity,
    parent_origin: Vec2,
    paint_order: &mut Vec<Entity>,
) {
    let Some(node) = world.get::<Node>(entity).cloned() else {
        return;
    };
    let origin = parent_origin + Vec2::new(pixels(node.left), pixels(node.top));
    let size = Vec2::new(pixels(node.width), pixels(node.height));
    world.entity_mut(entity).insert((
        ComputedNode { size, ..default() },
        UiGlobalTransform::from_translation(origin + size / 2.0),
        InheritedVisibility::VISIBLE,
    ));
    paint_order.push(entity);
    let children = world
        .get::<Children>(entity)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    for child in children {
        apply_authored_boxes(world, child, origin, paint_order);
    }
}

fn press_actual_button(app: &mut App, action: OverlayButton) -> Interaction {
    let (entity, node) = app
        .world_mut()
        .query::<(Entity, &OverlayButton, &Node)>()
        .iter(app.world())
        .find_map(|(entity, candidate, node)| {
            (*candidate == action).then_some((entity, node.clone()))
        })
        .expect("real dialog button must exist");
    let point = Vec2::new(
        pixels(node.left) + pixels(node.width) / 2.0,
        pixels(node.top) + pixels(node.height) / 2.0,
    );
    app.world_mut()
        .query::<&mut Window>()
        .single_mut(app.world_mut())
        .unwrap()
        .set_physical_cursor_position(Some(point.as_dvec2()));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut()
        .run_system_once(bevy::ui::ui_focus_system)
        .unwrap();
    *app.world().get::<Interaction>(entity).unwrap()
}

#[test]
fn game_shop_next_arrow_receives_press_through_overlapping_page_label() {
    let mut app = dialog_app(0);
    assert_eq!(
        press_actual_button(&mut app, OverlayButton::GameShopPageNext),
        Interaction::Pressed,
        "page text must not capture the next-arrow press"
    );
}

#[test]
fn game_shop_previous_arrow_receives_press_through_overlapping_page_label() {
    let mut app = dialog_app(1);
    assert_eq!(
        press_actual_button(&mut app, OverlayButton::GameShopPagePrev),
        Interaction::Pressed,
        "page text must not capture the previous-arrow press"
    );
}

#[test]
fn game_shop_category_and_search_receive_press_through_their_labels() {
    for action in [
        OverlayButton::GameShopControl(GameShopAction::Category(0)),
        OverlayButton::GameShopControl(GameShopAction::Search),
    ] {
        let mut app = dialog_app(0);
        assert_eq!(
            press_actual_button(&mut app, action),
            Interaction::Pressed,
            "{action:?}"
        );
    }
}

#[test]
fn game_shop_decorative_text_and_its_box_explicitly_pass_input() {
    let mut app = dialog_app(0);
    let labels: Vec<_> = app
        .world_mut()
        .query::<(Entity, &Text, &ChildOf)>()
        .iter(app.world())
        .map(|(entity, _, parent)| (entity, parent.parent()))
        .collect();
    assert!(
        labels.len() > 20,
        "exercise populated dialog, not a standalone fake label"
    );
    for (text, box_entity) in labels {
        assert_eq!(
            app.world().get::<FocusPolicy>(text),
            Some(&FocusPolicy::Pass)
        );
        assert_eq!(
            app.world().get::<FocusPolicy>(box_entity),
            Some(&FocusPolicy::Pass)
        );
    }
}

fn cached_dialog_app(page: usize) -> App {
    let mut app = super::super::tests::overlay_render_test_app();
    let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
    state.core.panel = mir2_ui_core::state::UiPanel::GameShop;
    state.game_shop_page = page;
    state.game_shop_dialog.class = Some(0);
    app.world_mut().resource_mut::<GameShopModel>().items = (0..105)
        .map(|i| GameShopEntry {
            game_shop_index: 2000 + i,
            item_index: 1000 + i,
            item_name: format!("Catalog row {i:03}"),
            category: "Regression".into(),
            gold_price: 100,
            can_buy_gold: true,
            ..default()
        })
        .collect();
    app.update();
    app
}

fn visible_page(app: &mut App) -> Vec<String> {
    app.world_mut()
        .query::<&Text>()
        .iter(app.world())
        .filter(|text| text.0.ends_with(" / 14"))
        .map(|text| text.0.clone())
        .collect()
}

fn visible_catalog_ids(app: &mut App) -> Vec<i32> {
    let mut ids = app
        .world_mut()
        .query::<&OverlayButton>()
        .iter(app.world())
        .filter_map(|action| match action {
            OverlayButton::GameShopControl(GameShopAction::Buy(index)) => Some(*index),
            _ => None,
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}

fn shop_children(app: &mut App) -> Vec<Entity> {
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<OverlayGameShop>>()
        .single(app.world())
        .unwrap();
    app.world().get::<Children>(root).unwrap().iter().collect()
}

#[test]
fn game_shop_retained_window_renders_every_changed_page_and_all_server_rows() {
    let mut app = cached_dialog_app(0);
    let mut reachable = Vec::new();
    for page in 0..14 {
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .game_shop_page = page;
        app.update();
        assert_eq!(
            visible_page(&mut app),
            vec![format!("{} / 14", page + 1)],
            "changing only the page must invalidate the actual retained window"
        );
        let first = 2000 + (page * 8) as i32;
        assert_eq!(
            visible_catalog_ids(&mut app),
            (first..(first + 8).min(2105)).collect::<Vec<_>>()
        );
        reachable.extend(visible_catalog_ids(&mut app));
    }
    assert_eq!(reachable, (2000..2105).collect::<Vec<_>>());
    assert_eq!(app.world().resource::<GameShopModel>().items.len(), 105);
}

#[test]
fn game_shop_retained_window_renders_previous_page_without_an_unrelated_edit() {
    let mut app = cached_dialog_app(1);
    assert_eq!(visible_page(&mut app), vec!["2 / 14"]);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .game_shop_page = 0;
    app.update();
    assert_eq!(visible_page(&mut app), vec!["1 / 14"]);
    assert_eq!(
        visible_catalog_ids(&mut app),
        (2000..2008).collect::<Vec<_>>()
    );
}

#[test]
fn game_shop_unchanged_page_keeps_existing_children_for_active_input() {
    let mut app = cached_dialog_app(0);
    let children = shop_children(&mut app);
    for _ in 0..3 {
        app.update();
        assert_eq!(
            shop_children(&mut app),
            children,
            "a page-key fix must not restore rebuilding on every frame"
        );
    }
}
