//! Shared Crystal-style widgets for the native shell.

use bevy::prelude::*;
use bevy::ui::{AlignItems, JustifyContent, Node, PositionType, Val};

use super::assets::CrystalButtonAssetSet;
use super::spec::{CrystalButtonSpec, CrystalRect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrystalButtonVisualState {
    Normal,
    Hover,
    Pressed,
    Disabled,
}

impl CrystalButtonVisualState {
    pub fn asset_path(self, assets: &CrystalButtonAssetSet) -> &str {
        match self {
            Self::Normal => assets.normal.as_str(),
            Self::Hover => assets.hover.as_str(),
            Self::Pressed => assets.pressed.as_str(),
            Self::Disabled => assets.disabled.as_deref().unwrap_or(assets.normal.as_str()),
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct CrystalImageButton {
    pub assets: CrystalButtonAssetSet,
    pub focused: bool,
    pub enabled: bool,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalImageButtonSprite;

pub fn resolve_button_visual_state(
    interaction: Option<Interaction>,
    focused: bool,
    enabled: bool,
) -> CrystalButtonVisualState {
    if !enabled {
        return CrystalButtonVisualState::Disabled;
    }

    match interaction {
        Some(Interaction::Pressed) => CrystalButtonVisualState::Pressed,
        Some(Interaction::Hovered) => CrystalButtonVisualState::Hover,
        Some(Interaction::None) | None if focused => CrystalButtonVisualState::Hover,
        Some(Interaction::None) | None => CrystalButtonVisualState::Normal,
    }
}

pub fn rect_contains(rect: CrystalRect, x: f32, y: f32) -> bool {
    x >= rect.left && x < rect.left + rect.width && y >= rect.top && y < rect.top + rect.height
}

pub fn button_image_offset(spec: CrystalButtonSpec) -> (f32, f32) {
    let _ = spec;
    // Crystal MirButton draws its image at the control's DisplayLocation.
    // Oversized art like Login OK (48x48 over a 42x42 hit rect) overflows
    // down/right from the control origin instead of being centered.
    (0.0, 0.0)
}

pub fn spawn_crystal_image_button<T: Component>(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    spec: CrystalButtonSpec,
    assets: CrystalButtonAssetSet,
    action: T,
    focused: bool,
    enabled: bool,
) {
    let (image_left, image_top) = button_image_offset(spec);
    let path = resolve_button_visual_state(None, focused, enabled)
        .asset_path(&assets)
        .to_owned();

    let mut entity = parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(spec.rect.left),
            top: Val::Px(spec.rect.top),
            width: Val::Px(spec.rect.width),
            height: Val::Px(spec.rect.height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        CrystalImageButton {
            assets,
            focused,
            enabled,
        },
        action,
    ));

    if enabled {
        entity.insert(Button);
    }

    entity.with_children(|button| {
        button.spawn((
            CrystalImageButtonSprite,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(image_left),
                top: Val::Px(image_top),
                width: Val::Px(spec.image_width),
                height: Val::Px(spec.image_height),
                ..default()
            },
            ImageNode {
                image: asset_server.load(path),
                ..default()
            },
        ));
    });
}

pub fn sync_crystal_image_buttons(
    asset_server: Res<AssetServer>,
    buttons: Query<
        (&CrystalImageButton, Option<&Interaction>, &Children),
        Or<(
            Added<CrystalImageButton>,
            Changed<CrystalImageButton>,
            Changed<Interaction>,
        )>,
    >,
    mut sprites: Query<&mut ImageNode, With<CrystalImageButtonSprite>>,
) {
    for (button, interaction, children) in &buttons {
        let state =
            resolve_button_visual_state(interaction.copied(), button.focused, button.enabled);
        let image = asset_server.load(state.asset_path(&button.assets).to_owned());
        for child in children.iter() {
            let Ok(mut node) = sprites.get_mut(child) else {
                continue;
            };
            node.image = image.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_assets() -> CrystalButtonAssetSet {
        CrystalButtonAssetSet {
            normal: "normal.png".to_owned(),
            hover: "hover.png".to_owned(),
            pressed: "pressed.png".to_owned(),
            disabled: None,
        }
    }

    #[test]
    fn visual_state_prefers_pressed_then_hover_then_focus() {
        assert_eq!(
            resolve_button_visual_state(Some(Interaction::Pressed), false, true),
            CrystalButtonVisualState::Pressed
        );
        assert_eq!(
            resolve_button_visual_state(Some(Interaction::Hovered), false, true),
            CrystalButtonVisualState::Hover
        );
        assert_eq!(
            resolve_button_visual_state(Some(Interaction::None), true, true),
            CrystalButtonVisualState::Hover
        );
        assert_eq!(
            resolve_button_visual_state(Some(Interaction::None), false, false),
            CrystalButtonVisualState::Disabled
        );
    }

    #[test]
    fn disabled_state_uses_normal_frame_path() {
        let assets = demo_assets();
        assert_eq!(
            CrystalButtonVisualState::Disabled.asset_path(&assets),
            "normal.png"
        );
    }

    #[test]
    fn explicit_disabled_asset_overrides_normal_frame_path() {
        let assets = demo_assets().with_disabled("disabled.png");
        assert_eq!(
            CrystalButtonVisualState::Disabled.asset_path(&assets),
            "disabled.png"
        );
    }

    #[test]
    fn hit_testing_uses_control_rect_not_intrinsic_image_size() {
        let rect = CrystalRect::new(575.0, 355.0, 42.0, 42.0);
        assert!(rect_contains(rect, 575.0, 355.0));
        assert!(rect_contains(rect, 616.99, 396.99));
        assert!(!rect_contains(rect, 617.0, 397.0));
        assert!(!rect_contains(rect, 572.0, 352.0));
    }

    #[test]
    fn oversized_button_art_stays_anchored_to_control_origin() {
        let spec = CrystalButtonSpec::new(
            "Title",
            320,
            321,
            322,
            CrystalRect::new(575.0, 355.0, 42.0, 42.0),
            48.0,
            48.0,
        );
        assert_eq!(button_image_offset(spec), (0.0, 0.0));
    }
}
