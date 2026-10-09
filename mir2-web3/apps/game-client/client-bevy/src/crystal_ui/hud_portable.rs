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

/// Local display preferences, applied only with accepted complete HUD snapshots.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedHudPreferences { pub hp_view: bool }
impl Default for SharedHudPreferences {
    fn default() -> Self { Self { hp_view: true } }
}

pub struct Mir2PortableMainHudPlugin;
impl Plugin for Mir2PortableMainHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::read_model::UiReadModel>()
            .init_resource::<super::panel_navigation::PanelNavigation>()
            .init_resource::<SharedHudPreferences>()
            .add_systems(
                Update,
                (sync_main_hud, update_main_hud, sync_health_label_visibility)
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

/// The same compact/alternate gate as Native hud.rs; world health bars are independent.
fn sync_health_label_visibility(
    model: Res<crate::read_model::UiReadModel>,
    preferences: Res<SharedHudPreferences>,
    mut labels: Query<(&super::shared_hud::SharedLabel, &mut Node)>,
) {
    use super::shared_hud::SharedLabel;
    let hp_only = crystal_hp_only(&model);
    for (kind, mut node) in &mut labels {
        let visible = match kind {
            SharedLabel::Hp => preferences.hp_view,
            SharedLabel::Mp => preferences.hp_view && !hp_only,
            SharedLabel::AlternateTop | SharedLabel::AlternateBottom => !preferences.hp_view,
            _ => continue,
        };
        node.display = if visible { Display::Flex } else { Display::None };
    }
}

#[cfg(test)]
mod shared_hp_view_tests {
    use super::*;
    use super::super::shared_hud::SharedLabel;
    #[test]
    fn shared_hp_view_labels_swap_in_one_update_and_keep_native_strings() {
        let mut app = App::new();
        let model = crate::read_model::UiReadModel { player: crate::read_model::PlayerStats {
            hp: 12, max_hp: 15, mp: 4, max_mp: 11, level: 30, class_name: Some("Wizard".into()), ..Default::default() } };
        app.insert_resource(model).init_resource::<SharedHudPreferences>()
            .add_systems(Update, (update_main_hud, sync_health_label_visibility).chain());
        let hp = app.world_mut().spawn((SharedLabel::Hp, Node::default(), Text::default())).id();
        let mp = app.world_mut().spawn((SharedLabel::Mp, Node::default(), Text::default())).id();
        let top = app.world_mut().spawn((SharedLabel::AlternateTop, Node::default(), Text::default())).id();
        let bottom = app.world_mut().spawn((SharedLabel::AlternateBottom, Node::default(), Text::default())).id();
        app.update();
        assert_eq!(app.world().get::<Node>(hp).unwrap().display, Display::Flex);
        assert_eq!(app.world().get::<Node>(mp).unwrap().display, Display::Flex);
        assert_eq!(app.world().get::<Node>(top).unwrap().display, Display::None);
        app.world_mut().resource_mut::<SharedHudPreferences>().hp_view = false;
        app.update();
        assert_eq!(app.world().get::<Node>(hp).unwrap().display, Display::None);
        assert_eq!(app.world().get::<Node>(mp).unwrap().display, Display::None);
        assert_eq!(app.world().get::<Node>(top).unwrap().display, Display::Flex);
        assert_eq!(app.world().get::<Node>(bottom).unwrap().display, Display::Flex);
        assert_eq!(app.world().get::<Text>(top).unwrap().0, " 12    4 \n---------------");
        assert_eq!(app.world().get::<Text>(bottom).unwrap().0, " 15    11 ");
        assert_eq!(app.world().resource::<crate::read_model::UiReadModel>().player.hp, 12);
    }
    #[test]
    fn shared_hp_view_warrior_hides_only_compact_mp_and_restores_after_toggle() {
        let mut app = App::new();
        app.insert_resource(crate::read_model::UiReadModel { player: crate::read_model::PlayerStats {
            level: 25, class_name: Some("Warrior".into()), ..Default::default() } })
            .init_resource::<SharedHudPreferences>().add_systems(Update, sync_health_label_visibility);
        let hp = app.world_mut().spawn((SharedLabel::Hp, Node::default())).id();
        let mp = app.world_mut().spawn((SharedLabel::Mp, Node::default())).id();
        let top = app.world_mut().spawn((SharedLabel::AlternateTop, Node::default())).id();
        app.update();
        assert_eq!(app.world().get::<Node>(hp).unwrap().display, Display::Flex);
        assert_eq!(app.world().get::<Node>(mp).unwrap().display, Display::None);
        app.world_mut().resource_mut::<SharedHudPreferences>().hp_view = false; app.update();
        assert_eq!(app.world().get::<Node>(top).unwrap().display, Display::Flex);
        app.world_mut().resource_mut::<SharedHudPreferences>().hp_view = true; app.update();
        assert_eq!(app.world().get::<Node>(top).unwrap().display, Display::None);
        assert_eq!(app.world().get::<Node>(mp).unwrap().display, Display::None);
    }
}
