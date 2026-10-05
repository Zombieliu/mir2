//! Headless tests measure the painter through Bevy UI layout, not a copied slot table.
use super::*;
use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
use bevy::ui::UiGlobalTransform;
use crate::inventory::ItemModel;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct HostAction(StoragePaintAction);

#[derive(Resource)]
struct Fixture {
    storage: StorageModel,
    options: StoragePaintOptions,
    layout: StoragePaintLayout,
    camera: Entity,
    assets_available: bool,
}

fn spawn_fixture(mut commands: Commands, assets: Res<AssetServer>, fixture: Res<Fixture>) {
    commands
        .spawn((
            Node { width: Val::Px(1600.0), height: Val::Px(1200.0), ..default() },
            UiTargetCamera(fixture.camera),
        ))
        .with_children(|parent| {
            paint_storage_with_layout(
                parent, fixture.assets_available.then_some(assets.as_ref()),
                &fixture.storage, &PlayerStats::default(), &fixture.options,
                fixture.layout, HostAction,
            );
        });
}

fn painted(
    storage: StorageModel,
    mut options: StoragePaintOptions,
    layout: StoragePaintLayout,
    assets_available: bool,
) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::input::InputPlugin,
        bevy::image::ImagePlugin::default(),
        bevy::transform::TransformPlugin,
        bevy::camera::visibility::VisibilityPlugin,
        bevy::text::TextPlugin,
        bevy::ui::UiPlugin,
    ));
    app.init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<bevy::image::TextureAtlasLayout>()
        .init_asset::<bevy::mesh::Mesh>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/public/original-ui/fonts/NotoSansCJKsc-Regular.otf"
    )).expect("packaged UI font");
    let font = app.world_mut().resource_mut::<Assets<Font>>().add(Font::from_bytes(bytes));
    options.font = Some(TextFont { font: font.into(), font_size: FontSize::Px(10.0), ..default() });
    let size = UVec2::new(1600, 1200);
    let camera = app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo { physical_size: size, scale_factor: 1.0 }),
                ..default()
            },
            viewport: Some(Viewport { physical_size: size, ..default() }),
            ..default()
        },
    )).id();
    app.insert_resource(Fixture { storage, options, layout, camera, assets_available });
    app.add_systems(Startup, spawn_fixture);
    for _ in 0..4 { app.update(); }
    app
}

fn measured(node: &ComputedNode, transform: &UiGlobalTransform) -> CrystalRect {
    let size = node.size() * node.inverse_scale_factor;
    let center = transform.affine().translation * node.inverse_scale_factor;
    CrystalRect::new(center.x - size.x * 0.5, center.y - size.y * 0.5, size.x, size.y)
}

fn cells(world: &mut World) -> Vec<(StoragePaintCell, CrystalRect, Option<HostAction>)> {
    let mut result: Vec<_> = world
        .query::<(&StoragePaintCell, &ComputedNode, &UiGlobalTransform, Option<&HostAction>)>()
        .iter(world)
        .map(|(cell, node, transform, host)| (*cell, measured(node, transform), host.copied()))
        .collect();
    result.sort_by_key(|row| row.0.slot);
    result
}

fn approx(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.1, "actual={actual}, expected={expected}");
}

fn item(slot: u32, unique_id: u64) -> ItemModel {
    ItemModel {
        container: 4, slot, unique_id: Some(unique_id), quantity: 7,
        key: "source-potion".into(), name: "Potion".into(), icon: 116,
        ..default()
    }
}

#[test]
fn actual_tree_has_all_eighty_live_cells_and_empty_cell_intents_at_original_geometry() {
    let mut storage = StorageModel::new();
    storage.items.push(item(0, 7));
    let mut app = painted(storage, StoragePaintOptions::default(), StoragePaintLayout::default(), true);
    let world = app.world_mut();
    let panel = world.query_filtered::<(&ComputedNode, &UiGlobalTransform), With<StoragePaintPanel>>()
        .single(world).map(|(node, transform)| measured(node, transform)).unwrap();
    approx(panel.width, 388.0);
    approx(panel.height, 346.0);
    let (panel_entity, _) = world.query::<(Entity, &StoragePaintPanel)>().single(world).unwrap();
    let (grid_entity, grid_parent) = world
        .query_filtered::<(Entity, &ChildOf), With<StoragePaintGrid>>()
        .single(world).unwrap();
    assert_eq!(grid_parent.parent(), panel_entity);
    for parent in world.query_filtered::<&ChildOf, With<StoragePaintCell>>().iter(world) {
        assert_eq!(parent.parent(), grid_entity);
    }
    let rows = cells(world);
    assert_eq!(rows.len(), 80);
    for (slot, (cell, rect, host)) in rows.iter().enumerate() {
        assert_eq!(cell.slot, slot as u32);
        assert!(cell.enabled);
        assert_eq!(*host, Some(HostAction(StoragePaintAction::Cell {
            slot: slot as u32, unique_id: (slot == 0).then_some(7),
        })));
        approx(rect.width, 36.0);
        approx(rect.height, 32.0);
        let (x, y) = rect.center();
        assert_eq!(rows.iter().filter(|row| row.1.contains(x, y)).count(), 1);
    }
    approx(rows[0].1.left - panel.left, 9.0);
    approx(rows[0].1.top - panel.top, 60.0);
    approx(rows[79].1.left - panel.left, 342.0);
    approx(rows[79].1.top - panel.top, 291.0);
    let actions: Vec<_> = world.query::<&HostAction>().iter(world).map(|host| host.0).collect();
    for action in [StoragePaintAction::Close, StoragePaintAction::Page(0),
        StoragePaintAction::Page(1), StoragePaintAction::Password] {
        assert!(actions.contains(&action));
    }
    assert!(world.query::<&Text>().iter(world).any(|text| text.0 == "7"));
}

#[test]
fn selection_requires_live_slot_and_uid_and_every_text_uses_the_host_font() {
    for (selected_uid, expected) in [(7, 1), (99, 0)] {
        let mut storage = StorageModel::new();
        storage.items.push(item(0, 7));
        let options = StoragePaintOptions {
            selection: Some(StorageItemSelection { slot: 0, unique_id: selected_uid }),
            ..default()
        };
        let mut app = painted(storage, options, StoragePaintLayout::default(), true);
        let world = app.world_mut();
        assert_eq!(world.query::<&StoragePaintSelection>().iter(world).count(), expected);
        let expected_font = world.resource::<Fixture>().options.font.as_ref().unwrap().font.clone();
        for font in world.query::<&TextFont>().iter(world) {
            assert_eq!(font.font, expected_font);
            assert_eq!(font.font_size, FontSize::Px(10.0));
        }
    }
}

#[test]
fn password_cover_blocks_measured_grid_and_does_not_map_any_transfer_cell() {
    let mut storage = StorageModel::new();
    storage.has_password = true;
    storage.unlocked = false;
    storage.items.push(item(0, 7));
    let mut app = painted(storage, StoragePaintOptions::default(), StoragePaintLayout::default(), true);
    let world = app.world_mut();
    let rows = cells(world);
    assert_eq!(rows.len(), 80);
    assert!(rows.iter().all(|row| !row.0.enabled && row.2.is_none()));
    let (kind, node, transform, focus, z, _) = world
        .query::<(&StoragePaintBlocker, &ComputedNode, &UiGlobalTransform, &FocusPolicy, &ZIndex, &Button)>()
        .single(world).unwrap();
    assert_eq!(*kind, StoragePaintBlocker::Password);
    assert_eq!(*focus, FocusPolicy::Block);
    assert_eq!(z.0, 1);
    let cover = measured(node, transform);
    for (_, rect, _) in rows {
        let (x, y) = rect.center();
        assert!(cover.contains(x, y));
    }
    assert!(world.query::<&HostAction>().iter(world).any(|host| host.0 == StoragePaintAction::Password));
}

#[test]
fn unrented_page_hides_cells_behind_real_cover_but_keeps_page_and_rent_controls() {
    let mut app = painted(StorageModel::new(), StoragePaintOptions { page: 1, ..default() },
        StoragePaintLayout::default(), true);
    let world = app.world_mut();
    let rows = cells(world);
    assert_eq!(rows.len(), 80);
    assert_eq!(rows.first().unwrap().0.slot, 80);
    assert_eq!(rows.last().unwrap().0.slot, 159);
    assert!(rows.iter().all(|row| !row.0.enabled && row.2.is_none() && row.1.width == 0.0));
    assert_eq!(world.query_filtered::<&Node, With<StoragePaintGrid>>().single(world).unwrap().display, Display::None);
    let (kind, node, transform, focus) = world
        .query::<(&StoragePaintBlocker, &ComputedNode, &UiGlobalTransform, &FocusPolicy)>()
        .single(world).unwrap();
    assert_eq!(*kind, StoragePaintBlocker::Rental);
    assert_eq!(*focus, FocusPolicy::Block);
    let cover = measured(node, transform);
    approx(cover.width, 372.0);
    approx(cover.height, 265.0);
    let actions: Vec<_> = world.query::<&HostAction>().iter(world).map(|host| host.0).collect();
    assert!(actions.contains(&StoragePaintAction::Page(1)));
    assert!(actions.contains(&StoragePaintAction::Rent));
    assert!(world.query::<&Text>().iter(world).any(|text| text.0 == "Expanded Storage Locked"));
}

#[test]
fn authoritative_partial_expansion_enables_only_valid_cells_and_formats_expiry() {
    let storage = StorageModel {
        size: 100, has_expanded: true, expiry: 621_355_968_000_000_000,
        ..StorageModel::new()
    };
    let mut app = painted(storage, StoragePaintOptions { page: 1, ..default() },
        StoragePaintLayout::default(), true);
    let world = app.world_mut();
    let rows = cells(world);
    assert_eq!(rows.iter().filter(|row| row.0.enabled).count(), 20);
    assert!(rows.iter().all(|row| row.0.enabled == (row.0.slot < 100)));
    assert!(rows.iter().filter(|row| row.0.slot >= 100).all(|row| row.2.is_none()));
    assert_eq!(world.query::<&StoragePaintBlocker>().iter(world).count(), 0);
    assert!(world.query::<&Text>().iter(world).any(|text|
        text.0 == "Expanded Storage Expires On1970/01/01 00:00:00"));
}

#[test]
fn touch_scale_measures_minimum_css_cell_size_and_scales_selection_and_fonts() {
    let css_scale = 0.5;
    let scale = 1.28 / css_scale;
    let mut storage = StorageModel::new();
    storage.items.push(item(0, 7));
    let options = StoragePaintOptions {
        selection: Some(StorageItemSelection { slot: 0, unique_id: 7 }), ..default()
    };
    let mut app = painted(storage, options, StoragePaintLayout { scale }, true);
    let world = app.world_mut();
    let rows = cells(world);
    for (_, rect, _) in &rows {
        assert!(rect.width * css_scale >= 40.0);
        assert!(rect.height * css_scale >= 40.0);
    }
    let selected = world
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<StoragePaintSelection>>()
        .single(world).map(|(node, transform)| measured(node, transform)).unwrap();
    approx(selected.left, rows[0].1.left);
    approx(selected.top, rows[0].1.top);
    approx(selected.width, rows[0].1.width);
    approx(selected.height, rows[0].1.height);
    for font in world.query::<&TextFont>().iter(world) {
        assert_eq!(font.font_size, FontSize::Px(10.0 * scale));
    }
}

#[test]
fn unavailable_assets_or_invalid_scale_never_fabricate_a_ready_panel() {
    for (assets_available, scale) in [(false, 1.0), (true, 0.0), (true, f32::NAN)] {
        let mut app = painted(StorageModel::new(), StoragePaintOptions::default(),
            StoragePaintLayout { scale }, assets_available);
        let world = app.world_mut();
        assert_eq!(world.query::<&StoragePaintPanel>().iter(world).count(), 0);
        assert_eq!(world.query::<&StoragePaintCell>().iter(world).count(), 0);
    }
}

#[test]
fn invalid_expiry_stays_absent_and_known_utc_ticks_are_decoded_without_raw_tick_labels() {
    assert_eq!(storage_expiry_label(0), None);
    assert_eq!(storage_expiry_label(0x3fff_ffff_ffff_ffff), None);
    assert_eq!(storage_expiry_label(621_355_968_000_000_000), Some("1970/01/01 00:00:00".into()));
}
