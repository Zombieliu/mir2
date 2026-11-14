//! Code-owned decorative skin primitives. No file access, asset source or input routing.
use bevy::prelude::*;
use bevy::sprite::{BorderRect, SliceScaleMode, TextureSlicer};
use bevy::ui::widget::NodeImageMode;
use bevy::ui::FocusPolicy;

use super::hud::CrystalHudAction;
use super::hud_layout::ReferenceHudArtwork;

/// Preserve the newly painted corner caps at every target size. Only the
/// empty center and straight edge strips adapt; this never stretches an old HUD.
pub(crate) fn frame_image(art: &ReferenceHudArtwork, corner_scale: f32, alpha: f32) -> ImageNode {
    ImageNode {
        image: art.panel.clone(),
        color: Color::srgba(1.0, 1.0, 1.0, alpha),
        image_mode: NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect::axes(256.0, 160.0),
            center_scale_mode: SliceScaleMode::Stretch,
            sides_scale_mode: SliceScaleMode::Stretch,
            max_corner_scale: corner_scale,
        }),
        ..default()
    }
}

fn piece(
    parent: &mut ChildSpawnerCommands,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    color: Color,
) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(top),
            width: Val::Px(width),
            height: Val::Px(height),
            ..default()
        },
        BackgroundColor(color),
        FocusPolicy::Pass,
    ));
}

/// Small code-native pictograms leave the typed button and its translated hint
/// on the parent. Every decorative leaf passes pointer focus through.
pub(crate) fn action_icon(
    parent: &mut ChildSpawnerCommands,
    action: CrystalHudAction,
    button_width: f32,
) {
    let left = (button_width - 16.0) * 0.5;
    let top = 4.0;
    let ivory = Color::srgb(0.9, 0.89, 0.82);
    let cyan = Color::srgb(0.18, 0.78, 0.86);
    let dark = Color::srgb(0.05, 0.06, 0.07);
    match action {
        CrystalHudAction::Character => {
            piece(parent, left + 6.0, top, 4.0, 4.0, ivory);
            piece(parent, left + 3.0, top + 6.0, 10.0, 6.0, ivory);
            piece(parent, left + 1.0, top + 8.0, 14.0, 4.0, ivory);
            piece(parent, left + 7.0, top + 7.0, 2.0, 5.0, dark);
        }
        CrystalHudAction::Inventory => {
            piece(parent, left + 4.0, top, 8.0, 2.0, ivory);
            piece(parent, left + 2.0, top + 3.0, 12.0, 10.0, ivory);
            piece(parent, left + 4.0, top + 5.0, 8.0, 5.0, dark);
            piece(parent, left + 5.0, top + 10.0, 6.0, 2.0, cyan);
        }
        CrystalHudAction::Skill => {
            piece(parent, left + 7.0, top, 2.0, 14.0, ivory);
            piece(parent, left + 1.0, top + 6.0, 14.0, 2.0, ivory);
            for (x, y) in [(3.0, 2.0), (11.0, 2.0), (3.0, 10.0), (11.0, 10.0)] {
                piece(parent, left + x, top + y, 2.0, 2.0, cyan);
            }
        }
        CrystalHudAction::Quest => {
            piece(parent, left + 3.0, top, 2.0, 14.0, ivory);
            piece(parent, left + 5.0, top + 1.0, 9.0, 6.0, ivory);
            piece(parent, left + 12.0, top + 6.0, 2.0, 2.0, ivory);
        }
        CrystalHudAction::Option => {
            piece(parent, left + 4.0, top + 2.0, 8.0, 10.0, ivory);
            piece(parent, left + 2.0, top + 4.0, 12.0, 6.0, ivory);
            piece(parent, left + 6.0, top, 4.0, 14.0, ivory);
            piece(parent, left, top + 5.0, 16.0, 4.0, ivory);
            piece(parent, left + 6.0, top + 5.0, 4.0, 4.0, dark);
        }
        CrystalHudAction::Menu => {
            for row in 0..3 {
                for col in 0..3 {
                    piece(
                        parent,
                        left + 2.0 + col as f32 * 4.0,
                        top + 1.0 + row as f32 * 4.0,
                        3.0,
                        3.0,
                        ivory,
                    );
                }
            }
        }
        _ => {}
    }
}
