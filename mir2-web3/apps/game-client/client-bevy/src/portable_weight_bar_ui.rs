//! Read-only Crystal weight image on the portable Quest UI camera.

use bevy::asset::LoadState;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiGlobalTransform, UiSystems, UiTargetCamera};
use bevy::window::{Window, WindowRef};

use crate::crystal_ui::{
    hud_bar::{horizontal_bar_rect, weight_bar_asset},
    spec,
};
use crate::pending_operations::PendingLifecycleSet;
use crate::portable_quest_ui::QuestUiTargetCamera;
use crate::read_model::UiReadModel;

const WEIGHT_IMAGES: [&str; 3] = [
    "original-ui/Prguse/76.png",
    "original-ui/UI_32bit/473.png",
    "original-ui/UI_32bit/472.png",
];

fn selected_image(ratio: f32) -> &'static str {
    match weight_bar_asset(ratio) {
        ("Prguse", 76) => WEIGHT_IMAGES[0],
        ("UI_32bit", 473) => WEIGHT_IMAGES[1],
        _ => WEIGHT_IMAGES[2],
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightBarRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct WeightBarHostContext {
    pub generation: u64,
    pub revision: u64,
    pub in_game: bool,
    pub host_visible: bool,
    pub current_weight: Option<u16>,
    pub max_weight: Option<u16>,
    pub slot: Option<WeightBarRect>,
    pub logical_size: Option<Vec2>,
    pub window_matches: bool,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct WeightBarObservation {
    pub ready: bool,
    pub generation: u64,
    pub revision: u64,
    pub current_weight: u16,
    pub max_weight: u16,
    pub slot: Option<WeightBarRect>,
    pub image: Option<&'static str>,
    pub source: Option<WeightBarRect>,
    pub destination: Option<WeightBarRect>,
    pub layout: Option<WeightBarRect>,
}

#[derive(Component)]
struct WeightRoot;
#[derive(Component)]
struct WeightImage;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
struct PaintedWeight {
    generation: u64,
    revision: u64,
    current_weight: u16,
    max_weight: u16,
    slot: WeightBarRect,
}

/// Each selected path is requested at most once per generation; switching back
/// retains the original handle, including a failed one.
#[derive(Resource, Default)]
struct WeightImageLease {
    generation: u64,
    handles: [Option<Handle<Image>>; 3],
}

impl WeightImageLease {
    fn image(
        &mut self,
        server: &AssetServer,
        generation: u64,
        path: &'static str,
    ) -> Handle<Image> {
        if self.generation != generation {
            self.generation = generation;
            self.handles = Default::default();
        }
        let index = WEIGHT_IMAGES
            .iter()
            .position(|candidate| *candidate == path)
            .expect("selected weight image is registered");
        if self.handles[index].is_none() {
            self.handles[index] = Some(server.load(path));
        }
        self.handles[index]
            .as_ref()
            .expect("image lease initialized")
            .clone()
    }

    fn selected(&self, generation: u64, path: &'static str) -> Option<&Handle<Image>> {
        (self.generation == generation)
            .then(|| {
                WEIGHT_IMAGES
                    .iter()
                    .position(|candidate| *candidate == path)
            })
            .flatten()
            .and_then(|index| self.handles[index].as_ref())
    }
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum WeightBarObservationSet {
    Observe,
}

pub struct Mir2PortableWeightBarUiPlugin;

impl Plugin for Mir2PortableWeightBarUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeightBarHostContext>()
            .init_resource::<WeightBarObservation>()
            .init_resource::<WeightImageLease>()
            .add_systems(Startup, spawn_root)
            .add_systems(Update, sync_image.after(PendingLifecycleSet::Ingest))
            .add_systems(
                PostUpdate,
                observe_image
                    .in_set(WeightBarObservationSet::Observe)
                    .after(UiSystems::Layout),
            );
    }
}

fn spawn_root(mut commands: Commands, target: Option<Res<QuestUiTargetCamera>>) {
    let Some(target) = target else {
        return;
    };
    commands.spawn((
        WeightRoot,
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

fn valid_context(context: &WeightBarHostContext, model: &UiReadModel) -> Option<PaintedWeight> {
    let (Some(current_weight), Some(max_weight), Some(slot), Some(size)) = (
        context.current_weight,
        context.max_weight,
        context.slot,
        context.logical_size,
    ) else {
        return None;
    };
    if !context.in_game
        || !context.host_visible
        || !context.window_matches
        || context.generation == 0
        || max_weight <= 0
        || current_weight != model.player.current_weight
        || max_weight != model.player.max_weight
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
        || (slot.width / slot.height - 76.0 / 12.0).abs() > 1.0
    {
        return None;
    }
    Some(PaintedWeight {
        generation: context.generation,
        revision: context.revision,
        current_weight,
        max_weight,
        slot,
    })
}

fn geometry(stamp: PaintedWeight, model: &UiReadModel) -> (WeightBarRect, WeightBarRect) {
    let source = horizontal_bar_rect(
        spec::hud::WEIGHT_BAR.rect,
        model.player.normalized_weight(),
        2.0,
    );
    let source = WeightBarRect {
        left: 0.0,
        top: 0.0,
        width: source.width,
        height: 12.0,
    };
    let destination = WeightBarRect {
        left: stamp.slot.left,
        top: stamp.slot.top,
        width: source.width * (stamp.slot.width / 76.0),
        height: stamp.slot.height,
    };
    (source, destination)
}

fn image_rect(rect: WeightBarRect) -> Rect {
    Rect::from_corners(
        Vec2::new(rect.left, rect.top),
        Vec2::new(rect.left + rect.width, rect.top + rect.height),
    )
}

fn image_node(destination: WeightBarRect) -> Node {
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
    context: Res<WeightBarHostContext>,
    model: Res<UiReadModel>,
    server: Res<AssetServer>,
    mut lease: ResMut<WeightImageLease>,
    mut roots: Query<(Entity, &mut Node), (With<WeightRoot>, Without<WeightImage>)>,
    mut images: Query<(&mut Node, &mut ImageNode, &mut PaintedWeight), With<WeightImage>>,
) {
    let Ok((root, mut root_node)) = roots.single_mut() else {
        return;
    };
    let Some(stamp) = valid_context(&context, &model) else {
        root_node.display = Display::None;
        return;
    };
    root_node.display = Display::Flex;
    let path = selected_image(model.player.normalized_weight());
    let handle = lease.image(&server, stamp.generation, path);
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
                WeightImage,
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

fn logical_rect(node: &ComputedNode, transform: &UiGlobalTransform) -> WeightBarRect {
    let scale = node.inverse_scale_factor;
    let center = transform.affine().translation * scale;
    let size = node.size() * scale;
    WeightBarRect {
        left: center.x - size.x * 0.5,
        top: center.y - size.y * 0.5,
        width: size.x,
        height: size.y,
    }
}

fn rect_matches(a: WeightBarRect, b: WeightBarRect) -> bool {
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
    expected: PaintedWeight,
    painted: PaintedWeight,
    loaded: bool,
    source: WeightBarRect,
    destination: WeightBarRect,
    actual_source: Option<Rect>,
    actual_size: (Val, Val),
    layout: WeightBarRect,
) -> bool {
    painted == expected
        && loaded
        && actual_source == Some(image_rect(source))
        && actual_size == (Val::Px(destination.width), Val::Px(destination.height))
        && rect_matches(layout, destination)
}

#[allow(clippy::too_many_arguments)]
fn observe_image(
    context: Res<WeightBarHostContext>,
    model: Res<UiReadModel>,
    server: Res<AssetServer>,
    lease: Res<WeightImageLease>,
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
        With<WeightRoot>,
    >,
    images: Query<
        (
            &ChildOf,
            &Node,
            &ImageNode,
            &PaintedWeight,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            &FocusPolicy,
        ),
        With<WeightImage>,
    >,
    mut observation: ResMut<WeightBarObservation>,
) {
    *observation = WeightBarObservation::default();
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
    let path = selected_image(model.player.normalized_weight());
    let Some(expected_handle) = lease.selected(stamp.generation, path) else {
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
            WeightBarRect {
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
    observation.current_weight = stamp.current_weight;
    observation.max_weight = stamp.max_weight;
    observation.slot = Some(stamp.slot);
    observation.image = Some(path);
    observation.source = Some(source);
    observation.destination = Some(destination);
    observation.layout = Some(layout);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(current: u16, max: u16) -> (WeightBarHostContext, UiReadModel) {
        let mut model = UiReadModel::default();
        model.player.current_weight = current;
        model.player.max_weight = max;
        (
            WeightBarHostContext {
                generation: 1,
                revision: 1,
                in_game: true,
                host_visible: true,
                current_weight: Some(current),
                max_weight: Some(max),
                slot: Some(WeightBarRect {
                    left: 919.0,
                    top: 719.0,
                    width: 76.0,
                    height: 12.0,
                }),
                logical_size: Some(Vec2::new(1024.0, 768.0)),
                window_matches: true,
            },
            model,
        )
    }

    #[test]
    fn native_thresholds_and_crop_include_empty_full_and_over_max() {
        for (current, max, image, crop) in [
            (0, 100, WEIGHT_IMAGES[0], 0.0),
            (1, 74, WEIGHT_IMAGES[0], 1.0),
            (50, 100, WEIGHT_IMAGES[0], 37.0),
            (51, 100, WEIGHT_IMAGES[1], 37.0),
            (75, 100, WEIGHT_IMAGES[1], 55.0),
            (76, 100, WEIGHT_IMAGES[2], 56.0),
            (100, 100, WEIGHT_IMAGES[2], 74.0),
            (200, 100, WEIGHT_IMAGES[2], 74.0),
        ] {
            let (context, model) = fixture(current, max);
            let stamp = valid_context(&context, &model).unwrap();
            assert_eq!(selected_image(model.player.normalized_weight()), image);
            assert_eq!(geometry(stamp, &model).0.width, crop);
            assert_eq!(geometry(stamp, &model).1.width, crop);
        }
    }

    #[test]
    fn raw_pair_and_layout_must_match_current_model() {
        let (mut context, mut model) = fixture(0, 100);
        assert!(valid_context(&context, &model).is_some());
        context.current_weight = None;
        assert!(valid_context(&context, &model).is_none());
        context.current_weight = Some(0);
        context.max_weight = None;
        assert!(valid_context(&context, &model).is_none());
        context.max_weight = Some(0);
        assert!(valid_context(&context, &model).is_none());
        context.max_weight = Some(100);
        model.player.current_weight = 1;
        assert!(valid_context(&context, &model).is_none());
        model.player.current_weight = 0;
        context.window_matches = false;
        assert!(valid_context(&context, &model).is_none());
        context.window_matches = true;
        context.slot.as_mut().unwrap().top = 900.0;
        assert!(valid_context(&context, &model).is_none());
    }

    #[test]
    fn painted_revision_asset_and_post_layout_gate_readiness() {
        let (context, model) = fixture(50, 100);
        let stamp = valid_context(&context, &model).unwrap();
        let (source, destination) = geometry(stamp, &model);
        let size = (Val::Px(destination.width), Val::Px(destination.height));
        assert!(painted_node_matches(
            stamp,
            stamp,
            true,
            source,
            destination,
            Some(image_rect(source)),
            size,
            destination
        ));
        assert!(!painted_node_matches(
            stamp,
            PaintedWeight {
                revision: 0,
                ..stamp
            },
            true,
            source,
            destination,
            Some(image_rect(source)),
            size,
            destination
        ));
        assert!(!painted_node_matches(
            stamp,
            stamp,
            false,
            source,
            destination,
            Some(image_rect(source)),
            size,
            destination
        ));
        assert!(!painted_node_matches(
            stamp,
            stamp,
            true,
            source,
            destination,
            Some(image_rect(source)),
            size,
            WeightBarRect {
                top: 700.0,
                ..destination
            }
        ));
    }
}
