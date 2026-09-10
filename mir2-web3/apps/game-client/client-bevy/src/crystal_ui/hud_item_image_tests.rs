//! Exercise persistent belt image updates without a window or renderer.
use super::*;
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use bevy::ecs::system::RunSystemOnce;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};

fn original_png(index: u16) -> Image {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../web/public/original-ui/Items/{index}.png"));
    Image::from_buffer(
        &std::fs::read(path).unwrap(),
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        bevy::asset::RenderAssetUsages::default(),
    )
    .unwrap()
}

fn belt_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<InventoryModel>()
        // Register in reverse order; the same explicit production dependency
        // must still select the image before measuring it within this frame.
        .add_systems(Update, layout_original_item_images)
        .add_systems(
            Update,
            update_hud_inventory
                .run_if(resource_changed::<InventoryModel>)
                .before(layout_original_item_images),
        );
    app.world_mut()
        .run_system_once(
            |mut commands: Commands, assets: Res<AssetServer>, inventory: Res<InventoryModel>| {
                commands.spawn(Node::default()).with_children(|parent| {
                    spawn_belt_slot(parent, &assets, &inventory, 0);
                });
            },
        )
        .unwrap();
    app
}

fn icon(app: &mut App) -> (Entity, Handle<Image>) {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(Entity, &ImageNode), With<CrystalHudBeltIcon>>();
    let (entity, image) = query.single(world).unwrap();
    (entity, image.image.clone())
}

#[test]
fn primary_belt_selects_current_image_before_layout_and_never_reuses_an_empty_white_texture() {
    let mut app = belt_app();
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(Handle::<Image>::default().id(), original_png(7))
        .unwrap();
    app.update();
    let (entity, _) = icon(&mut app);
    assert_eq!(
        app.world().get::<Node>(entity).unwrap().display,
        Display::None
    );
    app.world_mut().resource_mut::<InventoryModel>().items = vec![ItemModel {
        unique_id: Some(7),
        container: 1,
        slot: 0,
        quantity: 300,
        icon: 7,
        icon_width: 600,
        icon_height: 1,
        tooltip_source: Some(CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel {
                item_type: 8,
                shape: 0,
                image: 2960,
                stack_size: 300,
                ..default()
            },
            real_info: Some(CrystalItemInfoModel {
                image: 999,
                ..default()
            }),
            user_item: Some(CrystalUserItemModel {
                count: 1,
                ..default()
            }),
            ..default()
        }),
        ..default()
    }];
    for (count, expected) in [(300, 3662), (199, 3660), (200, 3661), (1, 3660)] {
        app.world_mut().resource_mut::<InventoryModel>().items[0].quantity = count;
        app.update();
        let (entity, handle) = icon(&mut app);
        assert_eq!(
            handle.path().unwrap().to_string(),
            format!("original-ui/Items/{expected}.png")
        );
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .insert(handle.id(), original_png(expected))
            .unwrap();
        // Inventory is unchanged: late image availability must still lay out.
        app.update();
        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(
            (node.left, node.top, node.width, node.height),
            (Val::Px(0.0), Val::Px(1.0), Val::Px(32.0), Val::Px(30.0))
        );
        assert_eq!(node.display, Display::Flex);
        let world = app.world_mut();
        let mut counts = world.query_filtered::<&Text, With<CrystalHudBeltItem>>();
        assert_eq!(counts.single(world).unwrap().0, count.to_string());
    }
    {
        let mut inventory = app.world_mut().resource_mut::<InventoryModel>();
        inventory.items[0].tooltip_source.as_mut().unwrap().info = CrystalItemInfoModel {
            image: 0,
            ..default()
        };
    }
    app.update();
    let (entity, handle) = icon(&mut app);
    assert_eq!(
        handle.path().unwrap().to_string(),
        "original-ui/Items/0.png"
    );
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(handle.id(), original_png(0))
        .unwrap();
    app.update();
    let node = app.world().get::<Node>(entity).unwrap();
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (Val::Px(0.0), Val::Px(4.0), Val::Px(32.0), Val::Px(23.0))
    );
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .clear();
    app.update();
    assert_eq!(icon(&mut app).1, Handle::<Image>::default());
    assert_eq!(
        app.world().get::<Node>(entity).unwrap().display,
        Display::None
    );
}

#[test]
fn primary_belt_integer_alpha_offsets_and_clipping_match_32_by_32_source_cell() {
    let mut app = belt_app();
    app.world_mut().resource_mut::<InventoryModel>().items = vec![ItemModel {
        unique_id: Some(7),
        container: 1,
        slot: 0,
        quantity: 50,
        tooltip_source: Some(CrystalItemTooltipSourceModel {
            info: CrystalItemInfoModel {
                item_type: 8,
                shape: 1,
                image: 2960,
                stack_size: 150,
                ..default()
            },
            ..default()
        }),
        ..default()
    }];
    app.update();
    let (entity, handle) = icon(&mut app);
    assert_eq!(
        handle.path().unwrap().to_string(),
        "original-ui/Items/3674.png"
    );
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(handle.id(), original_png(3674))
        .unwrap();
    app.update();
    let node = app.world().get::<Node>(entity).unwrap();
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (Val::Px(5.0), Val::Px(2.0), Val::Px(24.0), Val::Px(27.0))
    );
    let parent = app.world().get::<ChildOf>(entity).unwrap().parent();
    let hit = app.world().get::<Node>(parent).unwrap();
    assert_eq!(
        (hit.width, hit.height, hit.overflow),
        (Val::Px(32.0), Val::Px(32.0), Overflow::DEFAULT)
    );
}
