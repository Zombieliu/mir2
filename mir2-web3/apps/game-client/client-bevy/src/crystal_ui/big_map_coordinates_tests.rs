use super::*;
use crate::big_map::{BigMapInfo, BigMapNpc, BigMapPoint};
use bevy::window::WindowResolution;
use mir2_ui_core::state::UiPanel;

fn model(image: i32, width: i32, height: i32) -> BigMapModel {
    let mut model = BigMapModel::default();
    model.set_current_map(1);
    model.apply_new_map_info(
        1,
        BigMapInfo {
            title: "Bichon".into(),
            width,
            height,
            big_map: image,
            movements: vec![],
            npcs: vec![],
        },
    );
    model.player_location = Some(BigMapPoint { x: 337, y: 268 });
    model
}

fn window(width: u32, height: u32) -> Window {
    Window {
        resolution: WindowResolution::new(width, height),
        focused: true,
        ..default()
    }
}

fn set_panel_cursor(window: &mut Window, point: (f32, f32)) {
    let transform = CrystalStageTransform::fit(window.width(), window.height());
    let physical = transform.logical_to_physical(
        CRYSTAL_BIGMAP_PANEL_RECT.left + point.0,
        CRYSTAL_BIGMAP_PANEL_RECT.top + point.1,
    );
    window.set_cursor_position(Some(Vec2::new(physical.0, physical.1)));
}

fn set_tile_cursor(window: &mut Window, model: &BigMapModel, tile: (i32, i32), offset: f32) {
    let g = BigMapImageGeometry::for_model(model).unwrap();
    let map = model.active_map().unwrap();
    set_panel_cursor(
        window,
        (
            g.left + (tile.0 as f32 + offset) * g.width / map.info.width as f32,
            g.top + (tile.1 as f32 + offset) * g.height / map.info.height as f32,
        ),
    );
}

#[test]
fn hover_coordinates_match_navigation_tiles_for_scaled_centered_map_images() {
    for (image, map_width, map_height) in [(8, 600, 400), (14, 300, 800), (101, 700, 700)] {
        let model = model(image, map_width, map_height);
        for (width, height) in [(1024, 768), (1600, 900), (800, 600), (800, 1000)] {
            let mut window = window(width, height);
            for tile in [
                (0, 0),
                (map_width / 2, map_height / 3),
                (map_width - 1, map_height - 1),
            ] {
                set_tile_cursor(&mut window, &model, tile, 0.4);
                assert_eq!(
                    tile_under_cursor(&window, &model),
                    Some(tile),
                    "image={image}, window={width}x{height}"
                );
            }
        }
    }
}

#[test]
fn hover_ignores_image_margins_buttons_npc_list_and_letterboxing() {
    let model = model(14, 300, 800);
    let g = BigMapImageGeometry::for_model(&model).unwrap();
    let mut window = window(1600, 900);
    for point in [
        (g.left - 0.1, g.top),
        (g.left, g.top - 0.1),
        (g.left + g.width, g.top),
        (g.left, g.top + g.height),
        (20.0, 60.0),
        (600.0, 100.0),
        (410.0, 470.0),
    ] {
        set_panel_cursor(&mut window, point);
        assert_eq!(tile_under_cursor(&window, &model), None, "{point:?}");
    }
    window.set_cursor_position(Some(Vec2::new(1.0, 450.0)));
    assert_eq!(tile_under_cursor(&window, &model), None);
    window.set_cursor_position(None);
    assert_eq!(tile_under_cursor(&window, &model), None);
    set_panel_cursor(&mut window, (g.left, g.top));
    assert_eq!(tile_under_cursor(&window, &model), Some((0, 0)));
}

#[derive(Resource, Default)]
struct TextChanges(usize);

fn observe_text_changes(
    changed: Query<(), (With<CoordinateLabel>, Changed<Text>)>,
    mut count: ResMut<TextChanges>,
) {
    count.0 += changed.iter().count();
}

fn render_fixture(
    mut commands: Commands,
    mut panel: Query<(Entity, &mut Node), With<OverlayBigMap>>,
    ui: Res<NativePlayerUiState>,
    model: Res<BigMapModel>,
    renderer: Res<BigMapUiState>,
    read_model: Res<UiReadModel>,
    assets: Res<AssetServer>,
    mut cache: Local<RenderCache>,
) {
    fill_panel(
        &mut commands,
        &mut panel,
        ui.bigmap_open(),
        &model,
        &renderer,
        &read_model,
        Some(&assets),
        &mut cache,
    );
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Image>()
        .init_resource::<BigMapUiState>()
        .init_resource::<UiReadModel>()
        .init_resource::<TextChanges>();
    let mut shell = NativeShellModel::default();
    shell.screen = NativeShellScreen::InGame;
    let mut ui = NativePlayerUiState::default();
    ui.core.panel = UiPanel::BigMap;
    let model = model(101, 700, 700);
    let mut window = window(1024, 768);
    set_tile_cursor(&mut window, &model, (123, 456), 0.2);
    app.insert_resource(shell)
        .insert_resource(ui)
        .insert_resource(model);
    app.world_mut().spawn((window, PrimaryWindow));
    app.world_mut().spawn((OverlayBigMap, Node::default()));
    app.add_systems(
        Update,
        (render_fixture, update, observe_text_changes).chain(),
    );
    app
}

fn label(app: &mut App) -> Option<(Entity, String)> {
    let world = app.world_mut();
    world
        .query_filtered::<(Entity, &Text), With<CoordinateLabel>>()
        .iter(world)
        .next()
        .map(|(entity, text)| (entity, text.0.clone()))
}

fn move_to_tile(app: &mut App, tile: (i32, i32), offset: f32) {
    let model = app.world().resource::<BigMapModel>().clone();
    let world = app.world_mut();
    let mut query = world.query::<&mut Window>();
    let mut window = query.single_mut(world).unwrap();
    set_tile_cursor(&mut window, &model, tile, offset);
}

fn move_outside_image(app: &mut App) {
    let world = app.world_mut();
    let mut query = world.query::<&mut Window>();
    set_panel_cursor(&mut query.single_mut(world).unwrap(), (620.0, 200.0));
}

#[test]
fn hover_updates_retained_label_without_rebuilding_or_mutating_same_tile() {
    let mut app = app();
    app.update();
    let (entity, text) = label(&mut app).unwrap();
    assert_eq!(text, "[ 123, 456 ]");
    let changes = app.world().resource::<TextChanges>().0;
    for offset in [0.3, 0.4, 0.5, 0.6] {
        move_to_tile(&mut app, (123, 456), offset);
        app.update();
        assert_eq!(label(&mut app), Some((entity, "[ 123, 456 ]".into())));
        assert_eq!(app.world().resource::<TextChanges>().0, changes);
    }
    move_to_tile(&mut app, (124, 455), 0.4);
    app.update();
    assert_eq!(label(&mut app), Some((entity, "[ 124, 455 ]".into())));
    assert_eq!(app.world().resource::<TextChanges>().0, changes + 1);
    assert_eq!(
        app.world().resource::<BigMapModel>().player_location,
        Some(BigMapPoint { x: 337, y: 268 })
    );
}

#[test]
fn mouse_leave_retains_last_coordinate_but_map_session_and_world_changes_clear_it() {
    let mut app = app();
    app.update();
    move_outside_image(&mut app);
    app.update();
    assert_eq!(label(&mut app).unwrap().1, "[ 123, 456 ]");

    app.world_mut().resource_mut::<BigMapModel>().reset_epoch += 1;
    app.update();
    assert_eq!(label(&mut app).unwrap().1, "");
    move_to_tile(&mut app, (200, 300), 0.4);
    app.update();
    assert_eq!(label(&mut app).unwrap().1, "[ 200, 300 ]");
    app.world_mut().resource_mut::<BigMapModel>().view = BigMapView::WorldMap;
    app.update();
    assert_eq!(label(&mut app).unwrap().1, "");
    move_outside_image(&mut app);
    app.world_mut().resource_mut::<BigMapModel>().view = BigMapView::CurrentMap;
    app.update();
    assert_eq!(label(&mut app).unwrap().1, "");
    move_to_tile(&mut app, (100, 200), 0.4);
    app.update();
    app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::Login;
    app.update();
    assert_eq!(label(&mut app).unwrap().1, "");
}

#[test]
fn closing_or_losing_focus_clears_pointer_coordinate_before_reopening() {
    let mut app = app();
    app.update();
    {
        let world = app.world_mut();
        let mut query = world.query::<&mut Window>();
        query.single_mut(world).unwrap().focused = false;
    }
    app.update();
    assert_eq!(label(&mut app).unwrap().1, "");
    move_outside_image(&mut app);
    {
        let world = app.world_mut();
        let mut query = world.query::<&mut Window>();
        query.single_mut(world).unwrap().focused = true;
    }
    app.update();
    assert_eq!(label(&mut app).unwrap().1, "");
    move_to_tile(&mut app, (200, 300), 0.4);
    app.update();
    let entity = label(&mut app).unwrap().0;
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = UiPanel::None;
    app.update();
    assert!(label(&mut app).is_none());
    move_outside_image(&mut app);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = UiPanel::BigMap;
    app.update();
    let new_label = label(&mut app).unwrap();
    assert_ne!(new_label.0, entity);
    assert_eq!(new_label.1, "");
}

#[test]
fn preview_map_uses_its_own_dimensions_and_never_keeps_current_map_coordinates() {
    let mut app = app();
    app.update();
    move_outside_image(&mut app);
    {
        let mut model = app.world_mut().resource_mut::<BigMapModel>();
        model.apply_new_map_info(
            2,
            BigMapInfo {
                title: "Preview".into(),
                width: 300,
                height: 800,
                big_map: 14,
                movements: vec![],
                npcs: vec![],
            },
        );
        model.active_map_index = Some(2);
    }
    app.update();
    assert_eq!(label(&mut app).unwrap().1, "");
    move_to_tile(&mut app, (150, 799), 0.4);
    app.update();
    assert_eq!(label(&mut app).unwrap().1, "[ 150, 799 ]");
    let world = app.world_mut();
    assert_eq!(
        world
            .query::<&super::super::BigMapPlayerEntity>()
            .iter(world)
            .count(),
        0
    );
}

#[test]
fn missing_image_or_invalid_dimensions_never_display_player_coordinate_as_pointer() {
    for (image, width, height) in [
        (0, 700, 700),
        (i32::MAX, 700, 700),
        (101, 0, 700),
        (101, 700, 0),
    ] {
        let mut app = app();
        app.insert_resource(model(image, width, height));
        app.update();
        assert_eq!(label(&mut app).unwrap().1, "");
    }
}

#[test]
fn map_render_cache_refreshes_npc_selection_scroll_player_and_renderer_state() {
    let mut app = app();
    {
        let mut model = app.world_mut().resource_mut::<BigMapModel>();
        model.maps.get_mut(&1).unwrap().info.npcs = (1..=25)
            .map(|id| BigMapNpc {
                index: id as i32,
                file_name: format!("NPC{id}"),
                map_index: 1,
                image: 0,
                rate: 0,
                big_map_icon: 0,
                icon: 0,
                can_teleport_to: false,
                object_id: id,
                name: format!("NPC {id}"),
                location: BigMapPoint {
                    x: id as i32 * 10,
                    y: 250,
                },
                show_on_big_map: true,
            })
            .collect();
    }
    app.update();
    let original = label(&mut app).unwrap().0;
    app.world_mut()
        .resource_mut::<BigMapModel>()
        .selected_npc_object_id = Some(2);
    app.update();
    let selected = label(&mut app).unwrap().0;
    assert_ne!(selected, original);
    app.world_mut().resource_mut::<BigMapModel>().npc_scroll_row = 2;
    app.update();
    let scrolled = label(&mut app).unwrap().0;
    assert_ne!(scrolled, selected);
    {
        let world = app.world_mut();
        let ids = world
            .query::<&super::super::BigMapNpcRowEntity>()
            .iter(world)
            .map(|npc| npc.object_id)
            .collect::<Vec<_>>();
        assert!(!ids.contains(&1));
        assert!(!ids.contains(&2));
        assert!(ids.contains(&3));
    }
    app.world_mut()
        .resource_mut::<BigMapModel>()
        .player_location = Some(BigMapPoint { x: 400, y: 500 });
    app.update();
    let moved = label(&mut app).unwrap().0;
    assert_ne!(moved, scrolled);
    {
        let world = app.world_mut();
        let players = world
            .query::<&super::super::BigMapPlayerEntity>()
            .iter(world)
            .map(|player| player.location)
            .collect::<Vec<_>>();
        assert_eq!(players, vec![BigMapPoint { x: 400, y: 500 }]);
    }
    app.world_mut()
        .resource_mut::<BigMapUiState>()
        .search_focused = true;
    app.update();
    assert_ne!(label(&mut app).unwrap().0, moved);
    assert_eq!(label(&mut app).unwrap().1, "[ 123, 456 ]");
}
