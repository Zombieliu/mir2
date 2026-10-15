//! Read-only Crystal experience image on the portable Quest UI camera.

use bevy::asset::LoadState;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiGlobalTransform, UiSystems, UiTargetCamera};
use bevy::window::{Window, WindowRef};

use crate::crystal_ui::{hud_bar::horizontal_bar_rect, spec};
use crate::pending_operations::PendingLifecycleSet;
use crate::portable_quest_ui::QuestUiTargetCamera;
use crate::read_model::UiReadModel;

pub const EXPERIENCE_IMAGE: &str = "original-ui/Prguse/8.png";
const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExperienceBarRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct ExperienceBarHostContext {
    pub generation: u64,
    pub revision: u64,
    pub in_game: bool,
    pub host_visible: bool,
    pub experience: Option<i64>,
    pub max_experience: Option<i64>,
    pub slot: Option<ExperienceBarRect>,
    pub logical_size: Option<Vec2>,
    pub window_matches: bool,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct ExperienceBarObservation {
    pub ready: bool,
    pub generation: u64,
    pub revision: u64,
    pub experience: i64,
    pub max_experience: i64,
    pub slot: Option<ExperienceBarRect>,
    pub image: Option<&'static str>,
    pub source: Option<ExperienceBarRect>,
    pub destination: Option<ExperienceBarRect>,
    pub layout: Option<ExperienceBarRect>,
}

#[derive(Component)]
struct ExperienceRoot;
#[derive(Component)]
struct ExperienceImage;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
struct PaintedExperience {
    generation: u64,
    revision: u64,
    experience: i64,
    max_experience: i64,
    slot: ExperienceBarRect,
}

/// One failed request per generation. A new generation may retry a failed path.
#[derive(Resource, Default)]
struct ExperienceImageLease {
    generation: u64,
    handle: Option<Handle<Image>>,
    adopted_loading: bool,
}

impl ExperienceImageLease {
    fn image(&mut self, server: &AssetServer, generation: u64) -> Handle<Image> {
        if self.generation != generation {
            self.generation = generation;
            self.handle = None;
            self.adopted_loading = false;
        }
        if self.handle.is_none() {
            if let Some(handle) = server.get_handle::<Image>(EXPERIENCE_IMAGE) {
                self.adopted_loading =
                    matches!(server.get_load_state(handle.id()), Some(LoadState::Loading));
                if self.adopted_loading
                    || matches!(server.get_load_state(handle.id()), Some(LoadState::Loaded))
                {
                    self.handle = Some(handle);
                }
            }
            if self.handle.is_none() {
                self.handle = Some(server.load(EXPERIENCE_IMAGE));
            }
        }
        let handle = self.handle.as_mut().expect("image lease initialized");
        if self.adopted_loading {
            match server.get_load_state(handle.id()) {
                Some(LoadState::Failed(_)) | Some(LoadState::NotLoaded) => {
                    *handle = server.load(EXPERIENCE_IMAGE);
                    self.adopted_loading = false;
                }
                Some(LoadState::Loaded) => self.adopted_loading = false,
                _ => {}
            }
        }
        handle.clone()
    }
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum ExperienceBarObservationSet {
    Observe,
}

pub struct Mir2PortableExperienceBarUiPlugin;

impl Plugin for Mir2PortableExperienceBarUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExperienceBarHostContext>()
            .init_resource::<ExperienceBarObservation>()
            .init_resource::<ExperienceImageLease>()
            .add_systems(Startup, spawn_root)
            .add_systems(Update, sync_image.after(PendingLifecycleSet::Ingest))
            .add_systems(
                PostUpdate,
                observe_image
                    .in_set(ExperienceBarObservationSet::Observe)
                    .after(UiSystems::Layout),
            );
    }
}

fn spawn_root(mut commands: Commands, target: Option<Res<QuestUiTargetCamera>>) {
    let Some(target) = target else {
        return;
    };
    commands.spawn((
        ExperienceRoot,
        UiTargetCamera(target.0),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::None,
            ..default()
        },
        FocusPolicy::Pass,
        GlobalZIndex(950),
    ));
}

fn valid_context(
    context: &ExperienceBarHostContext,
    model: &UiReadModel,
) -> Option<PaintedExperience> {
    let (Some(experience), Some(max_experience), Some(slot), Some(size)) = (
        context.experience,
        context.max_experience,
        context.slot,
        context.logical_size,
    ) else {
        return None;
    };
    if !context.in_game
        || !context.host_visible
        || !context.window_matches
        || context.generation == 0
        || max_experience <= 0
        || experience.unsigned_abs() > MAX_SAFE_JS_INTEGER
        || max_experience as u64 > MAX_SAFE_JS_INTEGER
        || experience != model.player.experience
        || max_experience != model.player.max_experience
        || !size.is_finite()
        || size.x < 1.0
        || size.y < 1.0
        || ![slot.left, slot.top, slot.width, slot.height]
            .into_iter()
            .all(f32::is_finite)
        || slot.left < 0.0
        || slot.top < 0.0
        || slot.width <= 0.0
        || slot.height <= 0.0
        || slot.width > 16_384.0
        || slot.height > 16_384.0
        || slot.left + slot.width > size.x + 0.75
        || slot.top + slot.height > size.y + 0.75
        || (slot.width / slot.height - 1004.0 / 8.0).abs() > 1.0
    {
        return None;
    }
    Some(PaintedExperience {
        generation: context.generation,
        revision: context.revision,
        experience,
        max_experience,
        slot,
    })
}

fn geometry(
    stamp: PaintedExperience,
    model: &UiReadModel,
) -> (ExperienceBarRect, ExperienceBarRect) {
    let source = horizontal_bar_rect(
        spec::hud::EXPERIENCE_BAR.rect,
        model.player.normalized_experience(),
        3.0,
    );
    let source = ExperienceBarRect {
        left: 0.0,
        top: 0.0,
        width: source.width,
        height: 8.0,
    };
    let destination = ExperienceBarRect {
        left: stamp.slot.left,
        top: stamp.slot.top,
        width: source.width * (stamp.slot.width / 1004.0),
        height: stamp.slot.height,
    };
    (source, destination)
}

fn image_rect(rect: ExperienceBarRect) -> Rect {
    Rect::from_corners(
        Vec2::new(rect.left, rect.top),
        Vec2::new(rect.left + rect.width, rect.top + rect.height),
    )
}

fn image_node(destination: ExperienceBarRect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(destination.left),
        top: Val::Px(destination.top),
        width: Val::Px(destination.width),
        height: Val::Px(destination.height),
        ..default()
    }
}

fn sync_image(
    mut commands: Commands,
    context: Res<ExperienceBarHostContext>,
    model: Res<UiReadModel>,
    server: Res<AssetServer>,
    mut lease: ResMut<ExperienceImageLease>,
    mut roots: Query<(Entity, &mut Node), (With<ExperienceRoot>, Without<ExperienceImage>)>,
    mut images: Query<(&mut Node, &mut ImageNode, &mut PaintedExperience), With<ExperienceImage>>,
) {
    let Ok((root, mut root_node)) = roots.single_mut() else {
        return;
    };
    let Some(stamp) = valid_context(&context, &model) else {
        root_node.display = Display::None;
        return;
    };
    root_node.display = Display::Flex;
    let handle = lease.image(&server, stamp.generation);
    let (source, destination) = geometry(stamp, &model);
    if let Ok((mut node, mut image, mut painted)) = images.single_mut() {
        if *painted != stamp
            || image.image.id() != handle.id()
            || image.rect != Some(image_rect(source))
        {
            *node = image_node(destination);
            image.image = handle;
            image.rect = Some(image_rect(source));
            image.image_mode = NodeImageMode::Stretch;
            *painted = stamp;
        }
    } else {
        commands.entity(root).with_children(|parent| {
            parent.spawn((
                ExperienceImage,
                stamp,
                image_node(destination),
                ImageNode {
                    image: handle,
                    rect: Some(image_rect(source)),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
                FocusPolicy::Pass,
            ));
        });
    }
}

fn logical_rect(node: &ComputedNode, transform: &UiGlobalTransform) -> ExperienceBarRect {
    let scale = node.inverse_scale_factor;
    let center = transform.affine().translation * scale;
    let size = node.size() * scale;
    ExperienceBarRect {
        left: center.x - size.x * 0.5,
        top: center.y - size.y * 0.5,
        width: size.x,
        height: size.y,
    }
}

fn rect_matches(a: ExperienceBarRect, b: ExperienceBarRect) -> bool {
    [
        a.left - b.left,
        a.top - b.top,
        a.width - b.width,
        a.height - b.height,
    ]
    .into_iter()
    .all(|delta| delta.is_finite() && delta.abs() <= 0.75)
}

fn painted_node_matches(
    expected: PaintedExperience,
    painted: PaintedExperience,
    loaded: bool,
    source: ExperienceBarRect,
    destination: ExperienceBarRect,
    actual_source: Option<Rect>,
    actual_size: (Val, Val),
    layout: ExperienceBarRect,
) -> bool {
    painted == expected
        && loaded
        && actual_source == Some(image_rect(source))
        && actual_size == (Val::Px(destination.width), Val::Px(destination.height))
        && rect_matches(layout, destination)
}

#[allow(clippy::too_many_arguments)]
fn observe_image(
    context: Res<ExperienceBarHostContext>,
    model: Res<UiReadModel>,
    server: Res<AssetServer>,
    lease: Res<ExperienceImageLease>,
    target: Option<Res<QuestUiTargetCamera>>,
    camera_targets: Query<&RenderTarget>,
    windows: Query<&Window>,
    roots: Query<
        (
            Entity,
            &Node,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            &UiTargetCamera,
            &FocusPolicy,
        ),
        With<ExperienceRoot>,
    >,
    images: Query<
        (
            &ChildOf,
            &Node,
            &ImageNode,
            &PaintedExperience,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            &FocusPolicy,
        ),
        With<ExperienceImage>,
    >,
    mut observation: ResMut<ExperienceBarObservation>,
) {
    *observation = ExperienceBarObservation::default();
    let Some(stamp) = valid_context(&context, &model) else {
        return;
    };
    let Some(target) = target else {
        return;
    };
    let Ok(RenderTarget::Window(WindowRef::Entity(window_entity))) = camera_targets.get(target.0)
    else {
        return;
    };
    let Ok(window) = windows.get(*window_entity) else {
        return;
    };
    let Some(size) = context.logical_size else {
        return;
    };
    if (window.width() - size.x).abs() > 1.0 || (window.height() - size.y).abs() > 1.0 {
        return;
    }
    let Ok((
        root_entity,
        root_node,
        root_computed,
        root_transform,
        root_visible,
        root_target,
        root_focus,
    )) = roots.single()
    else {
        return;
    };
    let Ok((parent, node, image, painted, computed, transform, visible, focus)) = images.single()
    else {
        return;
    };
    let Some(expected_handle) = lease
        .handle
        .as_ref()
        .filter(|_| lease.generation == stamp.generation)
    else {
        return;
    };
    let (source, destination) = geometry(stamp, &model);
    let layout = logical_rect(computed, transform);
    if root_target.0 != target.0
        || *root_focus != FocusPolicy::Pass
        || root_node.display == Display::None
        || !root_visible.get()
        || !rect_matches(
            logical_rect(root_computed, root_transform),
            ExperienceBarRect {
                left: 0.0,
                top: 0.0,
                width: size.x,
                height: size.y,
            },
        )
        || parent.parent() != root_entity
        || *focus != FocusPolicy::Pass
        || node.display == Display::None
        || !visible.get()
        || image.image.id() != expected_handle.id()
        || image.image_mode != NodeImageMode::Stretch
        || !painted_node_matches(
            stamp,
            *painted,
            matches!(
                server.get_load_state(image.image.id()),
                Some(LoadState::Loaded)
            ),
            source,
            destination,
            image.rect,
            (node.width, node.height),
            layout,
        )
    {
        return;
    }
    observation.ready = true;
    observation.generation = stamp.generation;
    observation.revision = stamp.revision;
    observation.experience = stamp.experience;
    observation.max_experience = stamp.max_experience;
    observation.slot = Some(stamp.slot);
    observation.image = Some(EXPERIENCE_IMAGE);
    observation.source = Some(source);
    observation.destination = Some(destination);
    observation.layout = Some(layout);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_pair_and_scaled_crop() {
        let mut context = ExperienceBarHostContext {
            generation: 1,
            revision: 2,
            in_game: true,
            host_visible: true,
            experience: Some(0),
            max_experience: Some(100),
            slot: Some(ExperienceBarRect {
                left: 9.0,
                top: 759.0,
                width: 1004.0,
                height: 8.0,
            }),
            logical_size: Some(Vec2::new(1024.0, 768.0)),
            window_matches: true,
        };
        let mut model = UiReadModel::default();
        model.player.max_experience = 100;
        let stamp = valid_context(&context, &model).unwrap();
        assert_eq!(geometry(stamp, &model).0.width, 0.0);
        model.player.experience = -10;
        context.experience = Some(-10);
        assert_eq!(
            geometry(valid_context(&context, &model).unwrap(), &model)
                .0
                .width,
            0.0
        );
        model.player.experience = 200;
        context.experience = Some(200);
        context.slot = Some(ExperienceBarRect {
            left: 4.5,
            top: 379.5,
            width: 502.0,
            height: 4.0,
        });
        context.logical_size = Some(Vec2::new(512.0, 384.0));
        let (source, destination) = geometry(valid_context(&context, &model).unwrap(), &model);
        assert_eq!(source.width, 1001.0);
        assert_eq!((destination.width, destination.height), (500.5, 4.0));
        // A one-pixel threshold must use PlayerStats' f64 ratio cast to f32
        // before the native f32 multiplication/floor crop.
        model.player.experience = 1;
        model.player.max_experience = 1001;
        context.experience = Some(1);
        context.max_experience = Some(1001);
        assert_eq!(
            geometry(valid_context(&context, &model).unwrap(), &model)
                .0
                .width,
            1.0
        );
        model.player.max_experience = 1002;
        context.max_experience = Some(1002);
        assert_eq!(
            geometry(valid_context(&context, &model).unwrap(), &model)
                .0
                .width,
            0.0
        );
        context.max_experience = None;
        assert!(valid_context(&context, &model).is_none());
        context.max_experience = Some(0);
        model.player.max_experience = 0;
        assert!(valid_context(&context, &model).is_none());
        context.max_experience = Some(i64::MAX);
        model.player.max_experience = i64::MAX;
        assert!(valid_context(&context, &model).is_none());
    }

    #[test]
    fn observation_yields_until_asset_and_post_layout_node_match_current_revision() {
        let slot = ExperienceBarRect {
            left: 9.0,
            top: 759.0,
            width: 1004.0,
            height: 8.0,
        };
        let stamp = PaintedExperience {
            generation: 1,
            revision: 2,
            experience: 50,
            max_experience: 100,
            slot,
        };
        let source = ExperienceBarRect {
            left: 0.0,
            top: 0.0,
            width: 500.0,
            height: 8.0,
        };
        let destination = ExperienceBarRect {
            left: 9.0,
            top: 759.0,
            width: 500.0,
            height: 8.0,
        };
        let current = |painted, loaded, actual_source, size, layout| {
            painted_node_matches(
                stamp,
                painted,
                loaded,
                source,
                destination,
                actual_source,
                size,
                layout,
            )
        };
        let image = Some(image_rect(source));
        let size = (Val::Px(500.0), Val::Px(8.0));
        assert!(
            !current(stamp, false, image, size, destination),
            "loading/failed image stays React-owned"
        );
        assert!(!current(
            PaintedExperience {
                revision: 1,
                ..stamp
            },
            true,
            image,
            size,
            destination
        ));
        assert!(!current(stamp, true, None, size, destination));
        assert!(!current(
            stamp,
            true,
            image,
            (Val::Px(0.0), Val::Px(8.0)),
            destination
        ));
        assert!(!current(
            stamp,
            true,
            image,
            size,
            ExperienceBarRect {
                top: 750.0,
                ..destination
            }
        ));
        assert!(current(stamp, true, image, size, destination));
    }
}
