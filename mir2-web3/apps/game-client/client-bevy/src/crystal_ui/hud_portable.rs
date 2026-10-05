//! The portable quest surface only needs the Crystal HUD layer ordering.
pub const HUD_Z_INDEX: i32 = 950;
pub use super::hud_bar::horizontal_bar_rect;

pub use super::hud_orb::{
    crystal_hp_only, hp_only_orb_clip_geometry, orb_clip_geometry, orb_source_rect, HudSourceRect,
    OrbClipGeometry, OrbSide, ORB_HALF_WIDTH, ORB_HEIGHT, ORB_HP_ONLY_WIDTH, ORB_HP_SOURCE_LEFT,
    ORB_MP_SOURCE_LEFT, ORB_TOP, ORB_WIDTH,
};

use super::shared_hud::{spawn_main_hud, update_main_hud, SharedMainHudRoot};
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiTargetCamera};

#[derive(Resource, Clone)]
pub struct SharedHudSurface {
    pub camera: Entity,
    pub window: Entity,
    pub font: Handle<Font>,
    pub active: bool,
    pub generation: u64,
    pub revision: u64,
}

pub struct Mir2PortableMainHudPlugin;
impl Plugin for Mir2PortableMainHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::read_model::UiReadModel>()
            .init_resource::<super::panel_navigation::PanelNavigation>()
            .add_systems(
                Update,
                (sync_main_hud, update_main_hud)
                    .chain()
                    .after(crate::pending_operations::PendingLifecycleSet::Ingest),
            );
    }
}

fn sync_main_hud(
    mut commands: Commands,
    server: Res<AssetServer>,
    model: Res<crate::read_model::UiReadModel>,
    surface: Res<SharedHudSurface>,
    mut roots: Query<&mut Node, With<SharedMainHudRoot>>,
    mut fonts: Query<&mut TextFont, With<super::shared_hud::SharedLabel>>,
) {
    if let Ok(mut root) = roots.single_mut() {
        root.display = if surface.active {
            Display::Flex
        } else {
            Display::None
        };
        if surface.is_changed() {
            for mut font in &mut fonts {
                font.font = bevy::text::FontSource::Handle(surface.font.clone());
            }
        }
        return;
    }
    if !surface.active {
        return;
    }
    commands
        .spawn((
            SharedMainHudRoot,
            Visibility::Hidden,
            UiTargetCamera(surface.camera),
            GlobalZIndex(HUD_Z_INDEX),
            FocusPolicy::Pass,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(1024.),
                height: Val::Px(768.),
                ..Default::default()
            },
        ))
        .with_children(|p| spawn_main_hud(p, &server, &model, Some(&surface.font)));
}
