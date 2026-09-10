//! Crystal `Client/MirGraphics/MLibrary.cs:959-1059`.
//! `GetTrueSize` measures nonzero-alpha bounds; Draw still uses the full bitmap.

use bevy::prelude::*;

/// Source cell bounds, independent of the full image's dimensions and alpha
/// origin. Also used by the persistent HUD belt, not only rebuilt dialogs.
#[derive(Component)]
pub(super) struct OriginalItemImage {
    pub cell_width: i32,
    pub cell_height: i32,
}

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
    parent.spawn(original_item_image_bundle(
        asset_server,
        Some(index),
        cell_width,
        cell_height,
    ));
}

pub(super) fn layout_original_item_images(
    images: Option<Res<Assets<Image>>>,
    mut icons: Query<(&OriginalItemImage, &ImageNode, &mut Node)>,
    mut true_sizes: Local<
        std::collections::HashMap<bevy::asset::AssetId<Image>, Option<(i32, i32)>>,
    >,
) {
    let Some(images) = images else {
        for (_, _, mut node) in &mut icons {
            node.display = Display::None;
        }
        return;
    };
    // Cache only dimensions. Same-handle reloads and removals invalidate them.
    if images.is_changed() {
        true_sizes.clear();
    }
    for (cell, image_node, mut node) in &mut icons {
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
        node.left = Val::Px(((cell.cell_width - true_width) / 2) as f32);
        node.top = Val::Px(((cell.cell_height - true_height) / 2) as f32);
        node.width = Val::Px(width as f32);
        node.height = Val::Px(height as f32);
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
