//! Source-PNG and headless ECS checks only: no renderer, WindowPlugin or GUI.
use super::*;
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use bevy::ecs::system::RunSystemOnce;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceFixture {
    source_library_sha256: String,
    verified_original_png_count: usize,
    different_size_count: usize,
    different35_pixel_offset_count: usize,
    frames: Vec<[i32; 5]>,
}

fn fixture() -> SourceFixture {
    serde_json::from_str(include_str!(
        "../../test-fixtures/original-item-true-sizes.json"
    ))
    .unwrap()
}

fn original_png(index: u16) -> Image {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/public/original-ui/Items");
    Image::from_buffer(
        &std::fs::read(root.join(format!("{index}.png"))).unwrap(),
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        bevy::asset::RenderAssetUsages::default(),
    )
    .unwrap()
}

pub(super) fn load_original_images(world: &mut World) {
    let handles = world
        .query_filtered::<&ImageNode, With<OriginalItemImage>>()
        .iter(world)
        .filter_map(|image| {
            let path = image.image.path()?.to_string();
            let index = path
                .strip_prefix("original-ui/Items/")?
                .strip_suffix(".png")?
                .parse::<u16>()
                .unwrap();
            Some((image.image.clone(), index))
        })
        .collect::<Vec<_>>();
    for (handle, index) in handles {
        world
            .resource_mut::<Assets<Image>>()
            .insert(handle.id(), original_png(index))
            .unwrap();
    }
    world.run_system_once(layout_original_item_images).unwrap();
}

fn source(index: u16) -> CrystalItemTooltipSourceModel {
    CrystalItemTooltipSourceModel {
        info: CrystalItemInfoModel {
            item_index: 400,
            image: index,
            ..default()
        },
        ..default()
    }
}

fn item(index: u16, container: u8, slot: u32) -> ItemModel {
    ItemModel {
        unique_id: Some(100 + u64::from(slot)),
        key: format!("source-{index}"),
        name: format!("Source {index}"),
        quantity: 1,
        container,
        slot,
        // Deliberately wrong legacy metadata must not determine source pixels.
        icon: 999,
        icon_width: 1,
        icon_height: 600,
        tooltip_source: Some(source(index)),
        ..default()
    }
}

fn icon_nodes(world: &mut World) -> Vec<(u16, Entity, CrystalRect)> {
    world
        .query_filtered::<(Entity, &ImageNode, &Node), With<OriginalItemImage>>()
        .iter(world)
        .filter_map(|(entity, image, node)| {
            let path = image.image.path()?.to_string();
            let index = path
                .strip_prefix("original-ui/Items/")?
                .strip_suffix(".png")?
                .parse()
                .unwrap();
            let (Val::Px(left), Val::Px(top), Val::Px(width), Val::Px(height)) =
                (node.left, node.top, node.width, node.height)
            else {
                panic!("unresolved original icon {index}")
            };
            assert_eq!(node.display, Display::Flex);
            Some((index, entity, CrystalRect::new(left, top, width, height)))
        })
        .collect()
}

// The old frame-size-only regression calls this stronger actual-PNG replacement.
pub(super) fn assert_inventory_icon_geometry_and_missing_source() {
    let mut app = tests::overlay_render_test_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::Inventory;
    let unknown = ItemModel {
        name: "NotAnIcon".into(),
        quantity: 1,
        slot: 3,
        ..default()
    };
    app.world_mut().resource_mut::<InventoryModel>().items =
        vec![item(7, 0, 0), item(30, 0, 1), item(0, 0, 2), unknown];
    let before = app.world().resource::<InventoryModel>().items.clone();
    app.update();
    {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&Node, With<OriginalItemImage>>();
        assert_eq!(query.iter(world).count(), 3);
        assert!(query.iter(world).all(|node| node.display == Display::None));
    }
    load_original_images(app.world_mut());
    let mut actual = icon_nodes(app.world_mut())
        .into_iter()
        .map(|(index, _, rect)| (index, rect))
        .collect::<Vec<_>>();
    actual.sort_by_key(|(index, _)| *index);
    assert_eq!(
        actual,
        vec![
            (0, CrystalRect::new(2.0, 4.0, 32.0, 23.0)),
            (7, CrystalRect::new(0.0, 3.0, 36.0, 26.0)),
            (30, CrystalRect::new(1.0, 3.0, 36.0, 25.0)),
        ]
    );
    let world = app.world_mut();
    assert!(!world
        .query::<&Text>()
        .iter(world)
        .any(|text| text.0.contains("NotAn")));
    assert_eq!(world.resource::<InventoryModel>().items, before);
}

#[test]
fn primary_all_1003_source_pngs_use_alpha_size_for_every_native_cell_family() {
    let fixture = fixture();
    assert_eq!(
        fixture.source_library_sha256,
        "5d5f6e0251d2e5f7d87cb18352be2c2999ea311a7aa988de63dcf2fa78f9fb5a"
    );
    assert_eq!(fixture.verified_original_png_count, 1003);
    assert_eq!(fixture.frames.len(), 1003);
    assert_eq!(
        (
            fixture.different_size_count,
            fixture.different35_pixel_offset_count
        ),
        (550, 478)
    );
    let metadata: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../web/public/original-ui/Items/meta.json"
    ))
    .unwrap();
    let mut exported = metadata["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["index"].as_i64().unwrap() as i32)
        .collect::<Vec<_>>();
    exported.sort_unstable();
    assert_eq!(
        exported,
        fixture
            .frames
            .iter()
            .map(|frame| frame[0])
            .collect::<Vec<_>>()
    );
    assert!(fixture
        .frames
        .windows(2)
        .all(|pair| pair[0][0] < pair[1][0]));
    let mut app = App::new();
    app.init_resource::<Assets<Image>>()
        .add_systems(Update, layout_original_item_images);
    let mut expected = Vec::new();
    for [index, width, height, true_width, true_height] in fixture.frames {
        let image = original_png(u16::try_from(index).unwrap());
        assert_eq!(
            (image.width(), image.height()),
            (width as u32, height as u32)
        );
        let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        // Bag/equipment/storage/trade; belt; NPC goods; Guild; amount box.
        for (cell_width, cell_height) in [(36, 32), (32, 32), (40, 32), (35, 35), (38, 34)] {
            let entity = app
                .world_mut()
                .spawn((
                    OriginalItemImage {
                        cell_width,
                        cell_height,
                    },
                    ImageNode {
                        image: handle.clone(),
                        ..default()
                    },
                    Node::default(),
                ))
                .id();
            expected.push((
                entity,
                index,
                CrystalRect::new(
                    ((cell_width - true_width) / 2) as f32,
                    ((cell_height - true_height) / 2) as f32,
                    width as f32,
                    height as f32,
                ),
            ));
        }
    }
    app.update();
    assert_eq!(expected.len(), 5015);
    for (entity, index, expected) in expected {
        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(
            (node.left, node.top, node.width, node.height),
            (
                Val::Px(expected.left),
                Val::Px(expected.top),
                Val::Px(expected.width),
                Val::Px(expected.height)
            ),
            "full source bitmap {index}, not a trimmed or stretched rectangle"
        );
        assert_eq!(node.display, Display::Flex);
    }
}

#[test]
fn primary_default_white_texture_is_not_an_item_and_missing_assets_hide_previous_images() {
    let mut app = App::new();
    app.init_resource::<Assets<Image>>()
        .add_systems(Update, layout_original_item_images);
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .insert(Handle::<Image>::default().id(), original_png(7))
        .unwrap();
    let entity = app
        .world_mut()
        .spawn((
            OriginalItemImage {
                cell_width: 36,
                cell_height: 32,
            },
            ImageNode::default(),
            Node::default(),
        ))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<Node>(entity).unwrap().display,
        Display::None
    );
    let handle = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(original_png(0));
    app.world_mut().get_mut::<ImageNode>(entity).unwrap().image = handle.clone();
    app.update();
    assert_eq!(
        app.world().get::<Node>(entity).unwrap().display,
        Display::Flex
    );
    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .remove(handle.id());
    app.update();
    assert_eq!(
        app.world().get::<Node>(entity).unwrap().display,
        Display::None
    );
    app.world_mut().remove_resource::<Assets<Image>>();
    app.update();
    assert_eq!(
        app.world().get::<Node>(entity).unwrap().display,
        Display::None
    );
}

#[test]
fn primary_equipment_and_storage_use_default_mir_item_cell_size_without_clipping() {
    let mut app = tests::overlay_render_test_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::Character;
    app.world_mut().resource_mut::<InventoryModel>().items =
        (0..14).map(|slot| item(116, 2, slot)).collect();
    app.update();
    load_original_images(app.world_mut());
    let icons = icon_nodes(app.world_mut());
    assert_eq!(icons.len(), 14);
    for (_, entity, rect) in icons {
        assert_eq!(rect, CrystalRect::new(-4.0, 1.0, 44.0, 30.0));
        let cell = app.world().get::<ChildOf>(entity).unwrap().parent();
        let node = app.world().get::<Node>(cell).unwrap();
        assert_eq!(
            (node.width, node.height, node.overflow),
            (Val::Px(36.0), Val::Px(32.0), Overflow::DEFAULT)
        );
    }
    app.world_mut()
        .resource_mut::<InventoryModel>()
        .items
        .clear();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::Storage;
    *app.world_mut().resource_mut::<StorageModel>() = StorageModel {
        items: vec![item(116, 4, 0)],
        ..StorageModel::new()
    };
    app.update();
    load_original_images(app.world_mut());
    let icons = icon_nodes(app.world_mut());
    assert_eq!(icons.len(), 1);
    assert_eq!(icons[0].2, CrystalRect::new(-4.0, 1.0, 44.0, 30.0));
    let cell = app.world().get::<ChildOf>(icons[0].1).unwrap().parent();
    let node = app.world().get::<Node>(cell).unwrap();
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (Val::Px(9.0), Val::Px(60.0), Val::Px(36.0), Val::Px(32.0))
    );
}

#[test]
fn primary_inventory_outer_rows_allow_original_oversized_draws_without_widening_hit_targets() {
    let mut app = tests::overlay_render_test_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::Inventory;
    app.world_mut().resource_mut::<InventoryModel>().items =
        vec![item(116, 0, 0), item(116, 0, 39)];
    app.update();
    load_original_images(app.world_mut());
    for (_, entity, rect) in icon_nodes(app.world_mut()) {
        assert_eq!(rect, CrystalRect::new(-4.0, 1.0, 44.0, 30.0));
        let mut ancestor = entity;
        while let Some(parent) = app.world().get::<ChildOf>(ancestor) {
            ancestor = parent.parent();
            if let Some(node) = app.world().get::<Node>(ancestor) {
                assert_eq!(
                    node.overflow,
                    Overflow::DEFAULT,
                    "no implicit cell/grid/window crop"
                );
            }
        }
    }
}

#[test]
fn primary_source_count_changes_reselect_and_remeasure_without_mutating_item_metadata() {
    let mut app = tests::overlay_render_test_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::Inventory;
    let mut value = item(2960, 0, 0);
    let source = value.tooltip_source.as_mut().unwrap();
    source.info.item_type = 8;
    source.info.shape = 1;
    source.info.stack_size = 150;
    source.real_info = Some(CrystalItemInfoModel {
        image: 123,
        ..default()
    });
    source.user_item = Some(CrystalUserItemModel {
        count: 1,
        ..default()
    });
    app.world_mut().resource_mut::<InventoryModel>().items = vec![value];
    for (count, index, left, top, width, height) in [
        (49, 3673, 10.0, 2.0, 16.0, 28.0),
        (50, 3674, 7.0, 2.0, 24.0, 27.0),
        (100, 2960, 5.0, 1.0, 28.0, 29.0),
        (150, 3675, 4.0, 1.0, 28.0, 29.0),
        (1, 3673, 10.0, 2.0, 16.0, 28.0),
    ] {
        app.world_mut().resource_mut::<InventoryModel>().items[0].quantity = count;
        app.update();
        load_original_images(app.world_mut());
        let images = icon_nodes(app.world_mut());
        assert_eq!(images.len(), 1);
        assert_eq!(
            (images[0].0, images[0].2),
            (index, CrystalRect::new(left, top, width, height))
        );
        let world = app.world_mut();
        assert!(world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0 == count.to_string()));
        let source = world.resource::<InventoryModel>().items[0]
            .tooltip_source
            .as_ref()
            .unwrap();
        assert_eq!(source.info.image, 2960);
        assert_eq!(source.user_item.as_ref().unwrap().count, 1);
    }
}

#[test]
fn primary_npc_source_zero_and_oversized_icons_use_40_by_32_area_not_frame_metadata() {
    let mut app = tests::overlay_render_test_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::NpcShop;
    {
        let mut shop = app.world_mut().resource_mut::<ShopModel>();
        shop.service_mode = NpcShopServiceMode::Buy;
        shop.goods = [0, 30, 116]
            .into_iter()
            .map(|index| ShopGood {
                unique_id: u64::from(index) + 1,
                count: 1,
                icon: 999,
                icon_width: 1,
                icon_height: 600,
                tooltip_source: Some(source(index)),
                ..default()
            })
            .collect();
    }
    app.update();
    load_original_images(app.world_mut());
    let mut images = icon_nodes(app.world_mut())
        .into_iter()
        .map(|(index, _, rect)| (index, rect))
        .collect::<Vec<_>>();
    images.sort_by_key(|(index, _)| *index);
    assert_eq!(
        images,
        vec![
            (0, CrystalRect::new(4.0, 4.0, 32.0, 23.0)),
            (30, CrystalRect::new(3.0, 3.0, 36.0, 25.0)),
            (116, CrystalRect::new(-2.0, 1.0, 44.0, 30.0)),
        ]
    );
    let world = app.world_mut();
    assert!(world
        .query_filtered::<&Node, With<OverlayNpcShopGoodCell>>()
        .iter(world)
        .all(|node| node.overflow == Overflow::DEFAULT));
}

#[test]
fn primary_trade_rows_use_current_user_item_image_and_unstretched_original_pixels() {
    let mut app = tests::overlay_render_test_app();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::Trade;
    let mut poison = source(2961);
    poison.info.item_type = 8;
    poison.info.shape = 2;
    poison.info.stack_size = 150;
    app.world_mut()
        .resource_mut::<crate::social::SocialModel>()
        .trade
        .partner_items = vec![
        crate::social::TradeItemModel {
            unique_id: Some(100),
            count: 50,
            tooltip_source: Some(poison),
            ..default()
        },
        crate::social::TradeItemModel {
            unique_id: Some(101),
            count: 1,
            tooltip_source: Some(source(0)),
            ..default()
        },
    ];
    app.update();
    load_original_images(app.world_mut());
    let mut images = icon_nodes(app.world_mut())
        .into_iter()
        .map(|(index, _, rect)| (index, rect))
        .collect::<Vec<_>>();
    images.sort_by_key(|(index, _)| *index);
    assert_eq!(
        images,
        vec![
            (0, CrystalRect::new(2.0, 4.0, 32.0, 23.0)),
            (3671, CrystalRect::new(7.0, 2.0, 24.0, 27.0)),
        ]
    );
}
