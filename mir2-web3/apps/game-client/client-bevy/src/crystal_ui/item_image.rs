//! Crystal `Client/MirGraphics/MLibrary.cs:959-1059`.
//! `GetTrueSize` measures nonzero-alpha bounds; Draw still uses the full bitmap.

use bevy::prelude::*;

/// Source cell bounds, independent of the full image's dimensions and alpha
/// origin. Also used by the persistent HUD belt, not only rebuilt dialogs.
#[derive(Component)]
pub struct OriginalItemImage {
    pub cell_width: i32,
    pub cell_height: i32,
}

/// Bag-only presentation scale. Other item surfaces retain their original
/// pixel geometry when this component is absent.
#[derive(Component, Debug, Clone, Copy)]
pub struct OriginalItemPaintScale(pub f32);

pub(super) fn original_item_image_bundle(
    asset_server: &AssetServer,
    index: Option<u16>,
    cell_width: i32,
    cell_height: i32,
) -> (OriginalItemImage, Node, ImageNode) {
    (
        OriginalItemImage {
            cell_width,
            cell_height,
        },
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            ..default()
        },
        ImageNode {
            image: index
                .map(|index| asset_server.load(format!("original-ui/Items/{index}.png")))
                .unwrap_or_default(),
            ..default()
        },
    )
}

pub(super) fn spawn_original_item_image(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    index: u16,
    cell_width: i32,
    cell_height: i32,
) {
    spawn_original_item_image_tinted(parent, asset_server, index, cell_width, cell_height, Color::WHITE);
}

/// Crystal applies locked-cell colour and opacity to the bitmap itself;
/// transparent padding must remain transparent and keep its original layout.
pub(super) fn spawn_original_item_image_tinted(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    index: u16,
    cell_width: i32,
    cell_height: i32,
    color: Color,
) {
    let (cell, node, mut image) = original_item_image_bundle(
        asset_server,
        Some(index),
        cell_width,
        cell_height,
    );
    image.color = color;
    parent.spawn((cell, node, image));
}

pub fn layout_original_item_images(
    images: Option<Res<Assets<Image>>>,
    mut icons: Query<(&OriginalItemImage, &ImageNode, Option<&OriginalItemPaintScale>, &mut Node)>,
    mut true_sizes: Local<
        std::collections::HashMap<bevy::asset::AssetId<Image>, Option<(i32, i32)>>,
    >,
) {
    let Some(images) = images else {
        for (_, _, _, mut node) in &mut icons {
            node.display = Display::None;
        }
        return;
    };
    // Cache only dimensions. Same-handle reloads and removals invalidate them.
    if images.is_changed() {
        true_sizes.clear();
    }
    for (cell, image_node, paint_scale, mut node) in &mut icons {
        // Bevy can install a real white texture for the default image handle.
        // An empty belt cell must never turn that into a fabricated item.
        if image_node.image == Handle::<Image>::default() {
            node.display = Display::None;
            continue;
        }
        let Some(image) = images.get(&image_node.image) else {
            node.display = Display::None;
            continue;
        };
        let (Ok(width), Ok(height)) = (
            i32::try_from(image.texture_descriptor.size.width),
            i32::try_from(image.texture_descriptor.size.height),
        ) else {
            node.display = Display::None;
            continue;
        };
        if width <= 0 || height <= 0 {
            node.display = Display::None;
            continue;
        }
        let true_size = true_sizes.entry(image_node.image.id()).or_insert_with(|| {
            use bevy::render::render_resource::{TextureDimension, TextureFormat};
            if image.texture_descriptor.dimension != TextureDimension::D2
                || image.texture_descriptor.size.depth_or_array_layers != 1
                || !matches!(
                    image.texture_descriptor.format,
                    TextureFormat::Rgba8Unorm
                        | TextureFormat::Rgba8UnormSrgb
                        | TextureFormat::Bgra8Unorm
                        | TextureFormat::Bgra8UnormSrgb
                )
            {
                return None;
            }
            crystal_true_size_rgba8(width as u32, height as u32, image.data.as_deref()?)
        });
        let Some((true_width, true_height)) = *true_size else {
            node.display = Display::None;
            continue;
        };
        // C# divides integers toward zero. GetTrueSize returns alpha-bounds
        // SIZE only: Draw neither crops nor subtracts the alpha/library origin.
        let scale = paint_scale.map_or(1.0, |scale| scale.0);
        if !scale.is_finite() || scale <= 0.0 {
            node.display = Display::None;
            continue;
        }
        node.left = Val::Px(if scale == 1.0 {
            ((cell.cell_width - true_width) / 2) as f32
        } else {
            (cell.cell_width as f32 - true_width as f32 * scale) * 0.5
        });
        node.top = Val::Px(if scale == 1.0 {
            ((cell.cell_height - true_height) / 2) as f32
        } else {
            (cell.cell_height as f32 - true_height as f32 * scale) * 0.5
        });
        node.width = Val::Px(width as f32 * scale);
        node.height = Val::Px(height as f32 * scale);
        node.display = Display::Flex;
    }
}

/// Return only the source visible-bounds size, never a cropped image or offset.
/// The all-transparent source frame deliberately retains its full dimensions.
pub(super) fn crystal_true_size_rgba8(
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Option<(i32, i32)> {
    let width = i32::try_from(width).ok().filter(|width| *width > 0)?;
    let height = i32::try_from(height).ok().filter(|height| *height > 0)?;
    let expected = usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)?;
    if pixels.len() != expected {
        return None;
    }
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (index, pixel) in pixels.chunks_exact(4).enumerate() {
        if pixel[3] == 0 {
            continue;
        }
        let x = (index % width as usize) as i32;
        let y = (index / width as usize) as i32;
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + 1);
        bottom = bottom.max(y + 1);
    }
    if right == 0 {
        Some((width, height))
    } else {
        Some((right - left, bottom - top))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bag_paint_scale_changes_icon_pixels_without_changing_default_item_geometry() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .add_systems(Update, layout_original_item_images);
        let image = Image::new_fill(
            bevy::render::render_resource::Extent3d {
                width: 32, height: 30, depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            &[255, 255, 255, 255],
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::default(),
        );
        let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        let default_icon = app.world_mut().spawn((
            OriginalItemImage { cell_width: 36, cell_height: 32 },
            ImageNode { image: handle.clone(), ..default() }, Node::default(),
        )).id();
        let touch_icon = app.world_mut().spawn((
            OriginalItemImage { cell_width: 96, cell_height: 85 },
            OriginalItemPaintScale(2.67),
            ImageNode { image: handle, ..default() }, Node::default(),
        )).id();
        app.update();
        let original = app.world().get::<Node>(default_icon).unwrap();
        assert_eq!((original.left, original.top, original.width, original.height),
            (Val::Px(2.0), Val::Px(1.0), Val::Px(32.0), Val::Px(30.0)));
        let touch = app.world().get::<Node>(touch_icon).unwrap();
        assert_eq!((touch.width, touch.height), (Val::Px(32.0 * 2.67), Val::Px(30.0 * 2.67)));
        assert!(matches!(touch.left, Val::Px(left) if left > 5.0));
        assert!(matches!(touch.top, Val::Px(top) if top > 2.0));
    }

    #[test]
    fn true_size_uses_any_nonzero_alpha_not_rgb_and_does_not_return_trim_origin() {
        let mut pixels = vec![255u8; 8 * 6 * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = 0; // Nonzero transparent RGB is not a visible pixel.
        }
        pixels[(2 * 8 + 3) * 4 + 3] = 1;
        pixels[(4 * 8 + 6) * 4 + 3] = 255;
        assert_eq!(crystal_true_size_rgba8(8, 6, &pixels), Some((4, 3)));
        // The caller must still draw all 8x6 pixels, with no subtraction of
        // the visible region's (3,2) origin: that is Crystal's actual formula.
    }

    #[test]
    fn transparent_frame_falls_back_to_full_size_but_invalid_rgba_is_not_guessed() {
        assert_eq!(crystal_true_size_rgba8(4, 3, &[0; 4 * 3 * 4]), Some((4, 3)));
        assert_eq!(crystal_true_size_rgba8(0, 3, &[]), None);
        assert_eq!(crystal_true_size_rgba8(4, 3, &[0; 47]), None);
        assert_eq!(crystal_true_size_rgba8(4, 3, &[0; 49]), None);
        assert_eq!(crystal_true_size_rgba8(u32::MAX, 3, &[]), None);
    }
}
