//! Crystal HP/MP orb geometry and image-node painting shared by native and Web.

use bevy::prelude::*;
use bevy::ui::{widget::NodeImageMode, Display, PositionType, Val};

use super::spec::CrystalRect;
use crate::read_model::UiReadModel;

pub const ORB_WIDTH: f32 = 104.0;
pub const ORB_HEIGHT: f32 = 80.0;
pub const ORB_HALF_WIDTH: f32 = 50.0;
pub const ORB_HP_ONLY_WIDTH: f32 = 100.0;
pub const ORB_HP_SOURCE_LEFT: f32 = 0.0;
pub const ORB_MP_SOURCE_LEFT: f32 = 51.0;
pub const ORB_TOP: f32 = 646.0;
pub const SPLIT_ORB_IMAGE: &str = "original-ui/Prguse/4.png";
pub const HP_ONLY_ORB_IMAGE: &str = "original-ui/Prguse/6.png";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbSide {
    Hp,
    Mp,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudSourceRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl HudSourceRect {
    pub const fn new(left: f32, top: f32, width: f32, height: f32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    pub fn bevy_rect(self) -> bevy::math::Rect {
        bevy::math::Rect {
            min: Vec2::new(self.left, self.top),
            max: Vec2::new(self.left + self.width, self.top + self.height),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbClipGeometry {
    pub source: HudSourceRect,
    pub destination: CrystalRect,
}

pub const fn orb_source_rect(side: OrbSide, height: f32) -> HudSourceRect {
    let left = match side {
        OrbSide::Hp => ORB_HP_SOURCE_LEFT,
        OrbSide::Mp => ORB_MP_SOURCE_LEFT,
    };
    HudSourceRect::new(left, ORB_HEIGHT - height, ORB_HALF_WIDTH, height)
}

/// `slot_left/top` is the upper-left corner of the complete 104x80 orb.
pub fn orb_geometry_at(
    ratio: f32,
    side: OrbSide,
    hp_only: bool,
    slot_left: f32,
    slot_top: f32,
) -> OrbClipGeometry {
    let height = (ORB_HEIGHT * ratio.clamp(0.0, 1.0)).floor();
    if side == OrbSide::Hp && hp_only {
        OrbClipGeometry {
            source: HudSourceRect::new(0.0, ORB_HEIGHT - height, ORB_HP_ONLY_WIDTH, height),
            destination: CrystalRect::new(
                slot_left,
                slot_top + ORB_HEIGHT - height,
                ORB_HP_ONLY_WIDTH,
                height,
            ),
        }
    } else {
        OrbClipGeometry {
            source: orb_source_rect(side, height),
            destination: CrystalRect::new(
                slot_left + if side == OrbSide::Mp { 51.0 } else { 0.0 },
                slot_top + ORB_HEIGHT - height,
                ORB_HALF_WIDTH,
                height,
            ),
        }
    }
}

pub fn orb_clip_geometry(ratio: f32, side: OrbSide) -> OrbClipGeometry {
    orb_geometry_at(ratio, side, false, 0.0, ORB_TOP)
}

pub fn hp_only_orb_clip_geometry(ratio: f32) -> OrbClipGeometry {
    orb_geometry_at(ratio, OrbSide::Hp, true, 0.0, ORB_TOP)
}

pub fn crystal_hp_only(model: &UiReadModel) -> bool {
    model.player.level < 26
        && model
            .player
            .class_name
            .as_deref()
            .is_some_and(|class_name| class_name.eq_ignore_ascii_case("Warrior"))
}

pub const fn orb_image_path(side: OrbSide, hp_only: bool) -> &'static str {
    if matches!(side, OrbSide::Hp) && hp_only {
        HP_ONLY_ORB_IMAGE
    } else {
        SPLIT_ORB_IMAGE
    }
}

/// Shared Bevy image painter; callers attach their own typed marker/focus policy.
pub fn orb_image_nodes(
    asset_server: &AssetServer,
    side: OrbSide,
    ratio: f32,
    hp_only: bool,
    slot_left: f32,
    slot_top: f32,
) -> (Node, ImageNode) {
    orb_image_nodes_with_handle(
        asset_server.load(orb_image_path(side, hp_only)),
        side,
        ratio,
        hp_only,
        slot_left,
        slot_top,
    )
}

/// Paint with an already-requested image, without changing its load state.
pub fn orb_image_nodes_with_handle(
    handle: Handle<Image>,
    side: OrbSide,
    ratio: f32,
    hp_only: bool,
    slot_left: f32,
    slot_top: f32,
) -> (Node, ImageNode) {
    let geometry = orb_geometry_at(ratio, side, hp_only, slot_left, slot_top);
    let mut node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(geometry.destination.left),
        top: Val::Px(geometry.destination.top),
        width: Val::Px(geometry.destination.width),
        height: Val::Px(geometry.destination.height),
        ..default()
    };
    if side == OrbSide::Mp && hp_only {
        node.display = Display::None;
    }
    let image = ImageNode {
        image: handle,
        rect: Some(geometry.source.bevy_rect()),
        image_mode: NodeImageMode::Stretch,
        ..default()
    };
    (node, image)
}

pub fn update_orb_image_node(
    node: &mut Node,
    image: &mut ImageNode,
    asset_server: &AssetServer,
    side: OrbSide,
    ratio: f32,
    hp_only: bool,
    slot_left: f32,
    slot_top: f32,
) {
    update_orb_image_node_with_handle(
        node,
        image,
        asset_server.load(orb_image_path(side, hp_only)),
        side,
        ratio,
        hp_only,
        slot_left,
        slot_top,
    );
}

/// Update geometry and crop while retaining the caller's requested handle.
pub fn update_orb_image_node_with_handle(
    node: &mut Node,
    image: &mut ImageNode,
    handle: Handle<Image>,
    side: OrbSide,
    ratio: f32,
    hp_only: bool,
    slot_left: f32,
    slot_top: f32,
) {
    let geometry = orb_geometry_at(ratio, side, hp_only, slot_left, slot_top);
    node.display = if side == OrbSide::Mp && hp_only {
        Display::None
    } else {
        Display::Flex
    };
    node.left = Val::Px(geometry.destination.left);
    node.top = Val::Px(geometry.destination.top);
    node.width = Val::Px(geometry.destination.width);
    node.height = Val::Px(geometry.destination.height);
    image.image = handle;
    image.rect = Some(geometry.source.bevy_rect());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_model::PlayerStats;
    use bevy::asset::AssetApp;

    #[test]
    fn partial_empty_full_and_hp_only_geometry_keep_bottom_anchor() {
        let half = orb_geometry_at(0.5, OrbSide::Hp, false, 20.0, 600.0);
        assert_eq!(half.source, HudSourceRect::new(0.0, 40.0, 50.0, 40.0));
        assert_eq!(half.destination, CrystalRect::new(20.0, 640.0, 50.0, 40.0));
        assert_eq!(
            orb_geometry_at(0.0, OrbSide::Hp, false, 20.0, 600.0)
                .destination
                .height,
            0.0
        );
        assert_eq!(
            orb_geometry_at(2.0, OrbSide::Hp, true, 20.0, 600.0).source,
            HudSourceRect::new(0.0, 0.0, 100.0, 80.0)
        );
        assert_eq!(orb_clip_geometry(0.5, OrbSide::Hp).destination.top, 686.0);
        assert_eq!(orb_image_path(OrbSide::Hp, true), HP_ONLY_ORB_IMAGE);
        assert_eq!(orb_image_path(OrbSide::Mp, true), SPLIT_ORB_IMAGE);
    }

    #[test]
    fn warrior_boundary_remains_level_26() {
        let mut model = UiReadModel {
            player: PlayerStats {
                class_name: Some("warrior".into()),
                level: 25,
                ..default()
            },
        };
        assert!(crystal_hp_only(&model));
        model.player.level = 26;
        assert!(!crystal_hp_only(&model));
    }

    #[test]
    fn common_image_builder_and_updater_keep_source_and_node_in_sync() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Image>();
        let server = app.world().resource::<AssetServer>();
        let (mut node, mut image) = orb_image_nodes(server, OrbSide::Hp, 0.5, false, 12.0, 646.0);
        assert_eq!(node.left, Val::Px(12.0));
        assert_eq!(node.top, Val::Px(686.0));
        assert_eq!(node.height, Val::Px(40.0));
        assert_eq!(
            image.rect,
            Some(HudSourceRect::new(0.0, 40.0, 50.0, 40.0).bevy_rect())
        );
        update_orb_image_node(
            &mut node,
            &mut image,
            server,
            OrbSide::Hp,
            0.25,
            true,
            24.0,
            646.0,
        );
        assert_eq!(node.left, Val::Px(24.0));
        assert_eq!(node.top, Val::Px(706.0));
        assert_eq!(node.width, Val::Px(100.0));
        assert_eq!(node.height, Val::Px(20.0));
        assert_eq!(
            image.rect,
            Some(HudSourceRect::new(0.0, 60.0, 100.0, 20.0).bevy_rect())
        );
        assert_eq!(
            image.image.id(),
            server.load::<Image>(HP_ONLY_ORB_IMAGE).id()
        );
    }
}
