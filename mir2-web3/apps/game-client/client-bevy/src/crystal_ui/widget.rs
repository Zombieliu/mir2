//! Shared Crystal-style widgets for the native shell.

use bevy::prelude::*;
use bevy::ui::{
    AlignItems, Display, FocusPolicy, JustifyContent, Node, Overflow, PositionType, UiRect,
    UiSystems, Val,
};
use bevy::window::PrimaryWindow;

use super::assets::CrystalButtonAssetSet;
use super::spec::{CrystalButtonSpec, CrystalRect, STAGE_HEIGHT, STAGE_WIDTH};
use super::typography::{crystal_text_font, CRYSTAL_DEFAULT_FONT_SIZE_PX};

pub const CRYSTAL_HINT_BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);
pub const CRYSTAL_HINT_BORDER: Color = Color::srgb_u8(144, 144, 0);
pub const CRYSTAL_HINT_TEXT: Color = Color::srgb_u8(255, 255, 0);
pub const CRYSTAL_HINT_CURSOR_Y_OFFSET: f32 = 20.0;
pub const CRYSTAL_HINT_Z_INDEX: i32 = 20_000;
pub const CRYSTAL_ITEM_HINT_BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.8);
pub const CRYSTAL_ITEM_HINT_BORDER: Color = Color::srgb_u8(148, 146, 148);
pub const CRYSTAL_ITEM_HINT_BROKEN_BORDER: Color = Color::srgb_u8(255, 0, 0);
pub const CRYSTAL_ITEM_HINT_TEXT: Color = Color::WHITE;
pub const CRYSTAL_ITEM_HINT_CURSOR_OFFSET: f32 = 28.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrystalHintStyle {
    Control,
    Item { broken: bool },
}

/// Source-authored hover text attached to a Crystal UI hit target.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct CrystalHint(pub String, pub CrystalHintStyle);

impl CrystalHint {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into(), CrystalHintStyle::Control)
    }

    pub fn item(text: impl Into<String>, broken: bool) -> Self {
        Self(text.into(), CrystalHintStyle::Item { broken })
    }
}

#[derive(Component, Debug)]
pub struct CrystalHintOverlayRoot;

#[derive(Component, Debug)]
pub struct CrystalHintOverlayText;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrystalHintOverlayStyle(pub CrystalHintStyle);

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
struct CrystalHintOverlayTarget(Option<Entity>);

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
struct CrystalHintOverlayLayoutState {
    last_window_bounds: Option<Vec2>,
    last_window_scale_factor: Option<f32>,
}

pub struct Mir2CrystalHintPlugin;

impl Plugin for Mir2CrystalHintPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_crystal_hint_overlay)
            .add_systems(Update, sync_crystal_hint_overlay)
            // Text measurement is refreshed in UiSystems::Content. Position the
            // overlay before layout so the same frame's transform uses the new
            // cursor position. A changed label remains hidden for one layout
            // pass, preventing a stale-size flash when hints change length.
            .add_systems(
                PostUpdate,
                position_crystal_hint_overlay
                    .after(UiSystems::Content)
                    .before(UiSystems::Layout),
            );
    }
}

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

pub fn spawn_crystal_image_button<T: Bundle>(
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

fn spawn_crystal_hint_overlay(mut commands: Commands) {
    commands
        .spawn((
            CrystalHintOverlayRoot,
            CrystalHintOverlayStyle(CrystalHintStyle::Control),
            CrystalHintOverlayTarget::default(),
            CrystalHintOverlayLayoutState::default(),
            Node {
                position_type: PositionType::Absolute,
                // Keep the hidden absolute node in layout so Taffy preserves
                // its measured size across hover leave/re-entry. Display::None
                // zeroes ComputedNode and causes a one-frame edge-clamp jump.
                display: Display::Flex,
                border: UiRect::all(Val::Px(1.0)),
                max_width: Val::Percent(90.0),
                max_height: Val::Percent(90.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(CRYSTAL_HINT_BACKGROUND),
            BorderColor::all(CRYSTAL_HINT_BORDER),
            FocusPolicy::Pass,
            Visibility::Hidden,
            GlobalZIndex(CRYSTAL_HINT_Z_INDEX),
        ))
        .with_children(|root| {
            root.spawn((
                CrystalHintOverlayText,
                Text::new(""),
                crystal_text_font(CRYSTAL_DEFAULT_FONT_SIZE_PX),
                TextColor(CRYSTAL_HINT_TEXT),
                TextShadow {
                    offset: Vec2::splat(1.0),
                    color: Color::BLACK,
                },
                FocusPolicy::Pass,
            ));
        });
}

fn sync_crystal_hint_overlay(
    hints: Query<(
        Entity,
        &Interaction,
        &CrystalHint,
        Option<&CrystalImageButton>,
    )>,
    mut roots: Query<
        (
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut CrystalHintOverlayStyle,
            &mut CrystalHintOverlayTarget,
            &mut Visibility,
        ),
        With<CrystalHintOverlayRoot>,
    >,
    mut texts: Query<(&mut Text, &mut TextColor), With<CrystalHintOverlayText>>,
) {
    let selected = hints
        .iter()
        .filter(|(_, interaction, _, image_button)| {
            **interaction == Interaction::Hovered
                && image_button.is_none_or(|button| button.enabled)
        })
        .min_by_key(|(entity, _, _, _)| entity.to_bits())
        .map(|(entity, _, hint, _)| (entity, hint));

    let Ok((mut root, mut background, mut border, mut overlay_style, mut target, mut visibility)) =
        roots.single_mut()
    else {
        return;
    };
    let Ok((mut text, mut text_color)) = texts.single_mut() else {
        if root.display != Display::Flex {
            root.display = Display::Flex;
        }
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    if let Some((entity, selected)) = selected.filter(|(_, value)| !value.0.is_empty()) {
        let content_changed =
            target.0 != Some(entity) || text.0 != selected.0 || overlay_style.0 != selected.1;
        if text.0 != selected.0 {
            text.0.clone_from(&selected.0);
        }
        if target.0 != Some(entity) {
            target.0 = Some(entity);
        }
        if overlay_style.0 != selected.1 {
            overlay_style.0 = selected.1;
        }
        match selected.1 {
            CrystalHintStyle::Control => {
                if background.0 != CRYSTAL_HINT_BACKGROUND {
                    background.0 = CRYSTAL_HINT_BACKGROUND;
                }
                let desired = BorderColor::all(CRYSTAL_HINT_BORDER);
                if *border != desired {
                    *border = desired;
                }
                if text_color.0 != CRYSTAL_HINT_TEXT {
                    text_color.0 = CRYSTAL_HINT_TEXT;
                }
            }
            CrystalHintStyle::Item { broken } => {
                if background.0 != CRYSTAL_ITEM_HINT_BACKGROUND {
                    background.0 = CRYSTAL_ITEM_HINT_BACKGROUND;
                }
                let desired = BorderColor::all(if broken {
                    CRYSTAL_ITEM_HINT_BROKEN_BORDER
                } else {
                    CRYSTAL_ITEM_HINT_BORDER
                });
                if *border != desired {
                    *border = desired;
                }
                if text_color.0 != CRYSTAL_ITEM_HINT_TEXT {
                    text_color.0 = CRYSTAL_ITEM_HINT_TEXT;
                }
            }
        }
        if root.display != Display::Flex {
            root.display = Display::Flex;
        }
        if content_changed && *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    } else {
        if target.0.is_some() {
            target.0 = None;
        }
        if root.display != Display::Flex {
            root.display = Display::Flex;
        }
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
    }
}

fn position_crystal_hint_overlay(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut roots: Query<
        (
            &mut Node,
            &ComputedNode,
            &CrystalHintOverlayStyle,
            &CrystalHintOverlayTarget,
            &mut CrystalHintOverlayLayoutState,
            &mut Visibility,
        ),
        With<CrystalHintOverlayRoot>,
    >,
    texts: Query<Ref<Text>, With<CrystalHintOverlayText>>,
) {
    let Ok((mut node, computed, style, target, mut layout_state, mut visibility)) =
        roots.single_mut()
    else {
        return;
    };
    if target.0.is_none() {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    }
    let Ok(window) = windows.single() else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Ok(text) = texts.single() else {
        *visibility = Visibility::Hidden;
        return;
    };
    let bounds = Vec2::new(window.resolution.width(), window.resolution.height());
    let bounds_changed = layout_state.last_window_bounds != Some(bounds);
    let scale_factor = window.scale_factor();
    let scale_factor_changed = layout_state.last_window_scale_factor != Some(scale_factor);
    if bounds_changed {
        layout_state.last_window_bounds = Some(bounds);
    }
    if scale_factor_changed {
        layout_state.last_window_scale_factor = Some(scale_factor);
    }
    if text.is_changed() || bounds_changed || scale_factor_changed {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    }
    let logical_size = computed.size() * computed.inverse_scale_factor;
    let position = crystal_hint_position_for_bounds(
        style.0,
        cursor,
        logical_size,
        bounds,
    );
    let desired_left = Val::Px(position.x);
    let desired_top = Val::Px(position.y);
    if node.left != desired_left {
        node.left = desired_left;
    }
    if node.top != desired_top {
        node.top = desired_top;
    }
    if *visibility != Visibility::Visible {
        *visibility = Visibility::Visible;
    }
}

pub fn crystal_hint_position(cursor: Vec2, size: Vec2) -> Vec2 {
    crystal_hint_position_for_style(CrystalHintStyle::Control, cursor, size)
}

pub fn crystal_hint_position_for_style(style: CrystalHintStyle, cursor: Vec2, size: Vec2) -> Vec2 {
    crystal_hint_position_for_bounds(style, cursor, size, Vec2::new(STAGE_WIDTH, STAGE_HEIGHT))
}

pub fn crystal_hint_position_for_bounds(
    style: CrystalHintStyle,
    cursor: Vec2,
    size: Vec2,
    bounds: Vec2,
) -> Vec2 {
    let max_x = (bounds.x - size.x - 1.0).max(0.0);
    let max_y = (bounds.y - size.y - 1.0).max(0.0);
    let candidate = match style {
        CrystalHintStyle::Control => {
            Vec2::new(cursor.x - size.x, cursor.y + CRYSTAL_HINT_CURSOR_Y_OFFSET)
        }
        CrystalHintStyle::Item { .. } => cursor + Vec2::splat(CRYSTAL_ITEM_HINT_CURSOR_OFFSET),
    };
    Vec2::new(candidate.x.clamp(0.0, max_x), candidate.y.clamp(0.0, max_y))
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

    #[test]
    fn hint_position_matches_crystal_offset_and_clamps_all_edges() {
        assert_eq!(
            crystal_hint_position(Vec2::new(100.0, 100.0), Vec2::new(40.0, 12.0)),
            Vec2::new(60.0, 120.0)
        );
        assert_eq!(
            crystal_hint_position(Vec2::new(5.0, 5.0), Vec2::new(40.0, 12.0)),
            Vec2::new(0.0, 25.0)
        );
        assert_eq!(
            crystal_hint_position(Vec2::new(1023.0, 767.0), Vec2::new(100.0, 20.0)),
            Vec2::new(923.0, 747.0)
        );
        assert_eq!(
            crystal_hint_position(Vec2::new(500.0, 500.0), Vec2::new(1200.0, 900.0)),
            Vec2::ZERO
        );
        assert_eq!(
            crystal_hint_position_for_style(
                CrystalHintStyle::Item { broken: false },
                Vec2::new(100.0, 100.0),
                Vec2::new(80.0, 40.0),
            ),
            Vec2::new(128.0, 128.0)
        );
        assert_eq!(
            crystal_hint_position_for_style(
                CrystalHintStyle::Item { broken: true },
                Vec2::new(1020.0, 760.0),
                Vec2::new(80.0, 40.0),
            ),
            Vec2::new(943.0, 727.0)
        );
    }

    #[test]
    fn hint_overlay_shows_only_one_hovered_nonempty_hint_and_preserves_layout_when_hidden() {
        let mut app = App::new();
        let root = app
            .world_mut()
            .spawn((
                CrystalHintOverlayRoot,
                CrystalHintOverlayStyle(CrystalHintStyle::Control),
                CrystalHintOverlayTarget::default(),
                Node::default(),
                BackgroundColor(CRYSTAL_HINT_BACKGROUND),
                BorderColor::all(CRYSTAL_HINT_BORDER),
                Visibility::Hidden,
            ))
            .id();
        let text = app
            .world_mut()
            .spawn((
                CrystalHintOverlayText,
                Text::new(""),
                TextColor(CRYSTAL_HINT_TEXT),
            ))
            .id();
        let hovered = app
            .world_mut()
            .spawn((Interaction::Hovered, CrystalHint::new("Inventory")))
            .id();
        app.world_mut()
            .spawn((Interaction::Pressed, CrystalHint::new("Ignored")));
        app.add_systems(Update, sync_crystal_hint_overlay);

        app.update();

        assert_eq!(
            app.world().entity(root).get::<Node>().unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().entity(text).get::<Text>().unwrap().0,
            "Inventory"
        );
        assert_eq!(
            *app.world().entity(root).get::<Visibility>().unwrap(),
            Visibility::Hidden,
            "a new label stays hidden until its measured layout is ready"
        );

        app.world_mut()
            .entity_mut(hovered)
            .insert(Interaction::None);
        app.update();
        assert_eq!(
            app.world().entity(root).get::<Node>().unwrap().display,
            Display::Flex,
            "a hidden tooltip must remain measured instead of collapsing to zero"
        );
        assert_eq!(
            app.world()
                .entity(root)
                .get::<CrystalHintOverlayTarget>()
                .unwrap()
                .0,
            None
        );

        app.world_mut()
            .entity_mut(hovered)
            .insert(Interaction::Hovered);
        app.update();
        assert_eq!(
            app.world().entity(root).get::<Node>().unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world()
                .entity(root)
                .get::<CrystalHintOverlayTarget>()
                .unwrap()
                .0,
            Some(hovered)
        );
    }

    #[test]
    fn position_system_never_revives_a_hidden_overlay_without_a_hover_target() {
        let mut app = App::new();
        let target = app.world_mut().spawn_empty().id();
        let mut window = Window::default();
        window.resolution.set(1024.0, 768.0);
        window.set_cursor_position(Some(Vec2::new(100.0, 100.0)));
        let scale_factor = window.scale_factor();
        app.world_mut().spawn((window, PrimaryWindow));
        let root = app
            .world_mut()
            .spawn((
                CrystalHintOverlayRoot,
                CrystalHintOverlayStyle(CrystalHintStyle::Control),
                CrystalHintOverlayTarget(Some(target)),
                CrystalHintOverlayLayoutState {
                    last_window_bounds: Some(Vec2::new(1024.0, 768.0)),
                    last_window_scale_factor: Some(scale_factor),
                },
                Node {
                    display: Display::Flex,
                    ..default()
                },
                ComputedNode::default(),
                Visibility::Hidden,
            ))
            .id();
        app.world_mut()
            .spawn((CrystalHintOverlayText, Text::new("Inventory")));
        app.add_systems(Update, position_crystal_hint_overlay);

        app.update();
        app.update();
        assert_eq!(
            *app.world().entity(root).get::<Visibility>().unwrap(),
            Visibility::Visible
        );

        app.world_mut()
            .entity_mut(root)
            .get_mut::<CrystalHintOverlayTarget>()
            .unwrap()
            .0 = None;
        *app.world_mut()
            .entity_mut(root)
            .get_mut::<Visibility>()
            .unwrap() = Visibility::Hidden;
        app.update();

        assert_eq!(
            *app.world().entity(root).get::<Visibility>().unwrap(),
            Visibility::Hidden
        );
    }

    #[test]
    fn hint_position_uses_current_window_logical_bounds() {
        assert_eq!(
            crystal_hint_position_for_bounds(
                CrystalHintStyle::Control,
                Vec2::new(1100.0, 600.0),
                Vec2::new(120.0, 20.0),
                Vec2::new(1280.0, 720.0),
            ),
            Vec2::new(980.0, 620.0)
        );
        assert_eq!(
            crystal_hint_position_for_bounds(
                CrystalHintStyle::Item { broken: false },
                Vec2::new(1270.0, 710.0),
                Vec2::new(120.0, 40.0),
                Vec2::new(1280.0, 720.0),
            ),
            Vec2::new(1159.0, 679.0)
        );
    }
}
