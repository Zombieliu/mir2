//! Read-only Crystal HP image on the already-owned portable Quest UI camera.

use bevy::asset::LoadState;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiGlobalTransform, UiSystems, UiTargetCamera};
use bevy::window::{Window, WindowRef};

use crate::crystal_ui::hud_orb::{
    crystal_hp_only, orb_geometry_at, orb_image_nodes_with_handle, orb_image_path,
    update_orb_image_node_with_handle, HudSourceRect, OrbSide, ORB_HEIGHT, ORB_WIDTH,
};
use crate::pending_operations::PendingLifecycleSet;
use crate::portable_quest_ui::QuestUiTargetCamera;
use crate::read_model::UiReadModel;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HpOrbSlot {
    pub left: f32,
    pub top: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HpOrbRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl HpOrbRect {
    fn from_source(rect: HudSourceRect) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
        }
    }
    fn from_crystal(rect: crate::crystal_ui::CrystalRect) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
        }
    }
}

/// Applied atomically with the Quest snapshot; no independent HP authority.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct HpOrbHostContext {
    pub generation: u64,
    pub revision: u64,
    pub in_game: bool,
    pub player_known: bool,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: Option<i32>,
    pub max_mp: Option<i32>,
    pub hp_only: bool,
    pub slot: Option<HpOrbSlot>,
    pub logical_size: Option<Vec2>,
    pub window_matches: bool,
}

#[derive(Component)]
pub struct HpOrbRoot;
#[derive(Component)]
pub struct HpOrbImage;
#[derive(Component)]
pub struct MpOrbImage;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
struct PaintedOrb {
    generation: u64,
    revision: u64,
    value: i32,
    max_value: i32,
    hp_only: bool,
    slot: HpOrbSlot,
    side: OrbSide,
}

/// A failed image is requested only once for each session generation and orb path.
/// Keeping both path handles also prevents a 6 -> 4 -> 6 class change from
/// retrying the first failed path within the same generation.
#[derive(Resource, Default)]
struct HpOrbImageLease {
    generation: u64,
    split: Option<OrbImageAttempt>,
    hp_only: Option<OrbImageAttempt>,
}

struct OrbImageAttempt {
    handle: Handle<Image>,
    // A new generation can adopt an older in-flight load. If it fails, that
    // generation still gets exactly one request of its own.
    retry_if_adopted_load_fails: bool,
}

impl HpOrbImageLease {
    fn slot_mut(&mut self, hp_only: bool) -> &mut Option<OrbImageAttempt> {
        if hp_only {
            &mut self.hp_only
        } else {
            &mut self.split
        }
    }

    fn current(&self, generation: u64, hp_only: bool) -> Option<&Handle<Image>> {
        if self.generation != generation {
            return None;
        }
        (if hp_only { &self.hp_only } else { &self.split })
            .as_ref()
            .map(|attempt| &attempt.handle)
    }

    fn image(&mut self, server: &AssetServer, generation: u64, hp_only: bool) -> Handle<Image> {
        if self.generation != generation {
            self.generation = generation;
            self.split = None;
            self.hp_only = None;
        }
        let path = orb_image_path(OrbSide::Hp, hp_only);
        let attempt = self.slot_mut(hp_only).get_or_insert_with(|| {
            if let Some(handle) = server.get_handle::<Image>(path) {
                if matches!(server.get_load_state(handle.id()), Some(LoadState::Loading)) {
                    return OrbImageAttempt {
                        handle,
                        retry_if_adopted_load_fails: true,
                    };
                }
                if matches!(server.get_load_state(handle.id()), Some(LoadState::Loaded)) {
                    return OrbImageAttempt {
                        handle,
                        retry_if_adopted_load_fails: false,
                    };
                }
            }
            OrbImageAttempt {
                handle: server.load(path),
                retry_if_adopted_load_fails: false,
            }
        });
        if attempt.retry_if_adopted_load_fails {
            match server.get_load_state(attempt.handle.id()) {
                Some(LoadState::Failed(_)) | Some(LoadState::NotLoaded) => {
                    attempt.handle = server.load(path);
                    attempt.retry_if_adopted_load_fails = false;
                }
                Some(LoadState::Loaded) => attempt.retry_if_adopted_load_fails = false,
                _ => {}
            }
        }
        attempt.handle.clone()
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct HpOrbObservation {
    pub ready: bool,
    pub generation: u64,
    pub revision: u64,
    pub hp: i32,
    pub max_hp: i32,
    pub hp_only: bool,
    pub slot: Option<HpOrbSlot>,
    pub image: Option<&'static str>,
    pub source: Option<HpOrbRect>,
    pub destination: Option<HpOrbRect>,
    pub layout: Option<HpOrbRect>,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct MpOrbObservation {
    pub ready: bool,
    pub generation: u64,
    pub revision: u64,
    pub mp: i32,
    pub max_mp: i32,
    pub hp_only: bool,
    pub slot: Option<HpOrbSlot>,
    pub image: Option<&'static str>,
    pub source: Option<HpOrbRect>,
    pub destination: Option<HpOrbRect>,
    pub layout: Option<HpOrbRect>,
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum HpOrbObservationSet {
    Observe,
}

pub struct Mir2PortableHpOrbUiPlugin;

impl Plugin for Mir2PortableHpOrbUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HpOrbHostContext>()
            .init_resource::<HpOrbObservation>()
            .init_resource::<MpOrbObservation>()
            .init_resource::<HpOrbImageLease>()
            .add_systems(Startup, spawn_orb_root)
            .add_systems(Update, sync_orb_image.after(PendingLifecycleSet::Ingest))
            .add_systems(
                PostUpdate,
                (observe_orb, observe_mp_orb)
                    .in_set(HpOrbObservationSet::Observe)
                    .after(UiSystems::Layout),
            );
    }
}

fn spawn_orb_root(mut commands: Commands, target: Option<Res<QuestUiTargetCamera>>) {
    let Some(target) = target else {
        return;
    };
    commands.spawn((
        HpOrbRoot,
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
    context: &HpOrbHostContext,
    model: &UiReadModel,
) -> Option<(HpOrbSlot, PaintedOrb)> {
    valid_context_side(context, model, OrbSide::Hp)
}

fn valid_context_side(
    context: &HpOrbHostContext,
    model: &UiReadModel,
    side: OrbSide,
) -> Option<(HpOrbSlot, PaintedOrb)> {
    let slot = context.slot?;
    let size = context.logical_size?;
    if !context.in_game
        || !context.player_known
        || !context.window_matches
        || context.generation == 0
        || model.player.level == 0
        || !model
            .player
            .class_name
            .as_ref()
            .is_some_and(|class| !class.is_empty())
        || context.hp_only != crystal_hp_only(model)
        || !slot.left.is_finite()
        || !slot.top.is_finite()
        || !size.is_finite()
        || size.x < ORB_WIDTH
        || size.y < ORB_HEIGHT
        || slot.left < 0.0
        || slot.top < 0.0
        || slot.left + ORB_WIDTH > size.x + 1.0
        || slot.top + ORB_HEIGHT > size.y + 1.0
    {
        return None;
    }
    let (value, max_value) = match side {
        OrbSide::Hp
            if context.max_hp > 0
                && context.hp >= 0
                && context.hp == model.player.hp
                && context.max_hp == model.player.max_hp =>
        {
            (context.hp, context.max_hp)
        }
        OrbSide::Mp if !context.hp_only => {
            let (Some(mp), Some(max_mp)) = (context.mp, context.max_mp) else {
                return None;
            };
            if mp < 0 || max_mp <= 0 || mp != model.player.mp || max_mp != model.player.max_mp {
                return None;
            }
            (mp, max_mp)
        }
        _ => return None,
    };
    Some((
        slot,
        PaintedOrb {
            generation: context.generation,
            revision: context.revision,
            value,
            max_value,
            hp_only: context.hp_only,
            slot,
            side,
        },
    ))
}

fn sync_orb_image(
    mut commands: Commands,
    context: Res<HpOrbHostContext>,
    model: Res<UiReadModel>,
    server: Res<AssetServer>,
    mut lease: ResMut<HpOrbImageLease>,
    mut roots: Query<
        (Entity, &mut Node),
        (With<HpOrbRoot>, Without<HpOrbImage>, Without<MpOrbImage>),
    >,
    mut hp_images: Query<
        (&mut Node, &mut ImageNode, &mut PaintedOrb),
        (With<HpOrbImage>, Without<HpOrbRoot>, Without<MpOrbImage>),
    >,
    mut mp_images: Query<
        (&mut Node, &mut ImageNode, &mut PaintedOrb),
        (With<MpOrbImage>, Without<HpOrbRoot>, Without<HpOrbImage>),
    >,
) {
    let Ok((root, mut root_node)) = roots.single_mut() else {
        return;
    };
    let hp_next = valid_context(&context, &model);
    let mp_next = valid_context_side(&context, &model, OrbSide::Mp);
    if hp_next.is_none() && mp_next.is_none() {
        root_node.display = Display::None;
        return;
    }
    root_node.display = Display::Flex;
    if let Some((slot, next)) = hp_next {
        let handle = lease.image(&server, next.generation, next.hp_only);
        let ratio = model.player.normalized_hp();
        if let Ok((mut node, mut image, mut painted)) = hp_images.single_mut() {
            if *painted != next || image.image.id() != handle.id() || node.display == Display::None
            {
                update_orb_image_node_with_handle(
                    &mut node,
                    &mut image,
                    handle,
                    OrbSide::Hp,
                    ratio,
                    next.hp_only,
                    slot.left,
                    slot.top,
                );
                *painted = next;
            }
        } else {
            let (node, image) = orb_image_nodes_with_handle(
                handle,
                OrbSide::Hp,
                ratio,
                next.hp_only,
                slot.left,
                slot.top,
            );
            commands.entity(root).with_children(|parent| {
                parent.spawn((HpOrbImage, next, node, image, FocusPolicy::Pass));
            });
        }
    } else if let Ok((mut node, _, _)) = hp_images.single_mut() {
        node.display = Display::None;
    }
    if let Some((slot, next)) = mp_next {
        // Both sides of the split orb use the same strong generation/path lease.
        let handle = lease.image(&server, next.generation, false);
        let ratio = model.player.normalized_mp();
        if let Ok((mut node, mut image, mut painted)) = mp_images.single_mut() {
            if *painted != next || image.image.id() != handle.id() || node.display == Display::None
            {
                update_orb_image_node_with_handle(
                    &mut node,
                    &mut image,
                    handle,
                    OrbSide::Mp,
                    ratio,
                    false,
                    slot.left,
                    slot.top,
                );
                *painted = next;
            }
        } else {
            let (node, image) =
                orb_image_nodes_with_handle(handle, OrbSide::Mp, ratio, false, slot.left, slot.top);
            commands.entity(root).with_children(|parent| {
                parent.spawn((MpOrbImage, next, node, image, FocusPolicy::Pass));
            });
        }
    } else if let Ok((mut node, _, _)) = mp_images.single_mut() {
        node.display = Display::None;
    }
}

fn logical_rect(node: &ComputedNode, transform: &UiGlobalTransform) -> HpOrbRect {
    let scale = node.inverse_scale_factor;
    let center = transform.affine().translation * scale;
    let size = node.size() * scale;
    HpOrbRect {
        left: center.x - size.x * 0.5,
        top: center.y - size.y * 0.5,
        width: size.x,
        height: size.y,
    }
}

fn rect_matches(measured: HpOrbRect, expected: HpOrbRect) -> bool {
    [
        measured.left - expected.left,
        measured.top - expected.top,
        measured.width - expected.width,
        measured.height - expected.height,
    ]
    .into_iter()
    .all(|delta| delta.is_finite() && delta.abs() <= 1.5)
}

struct RootView<'a> {
    entity: Entity,
    node: &'a Node,
    computed: &'a ComputedNode,
    transform: &'a UiGlobalTransform,
    visible: &'a InheritedVisibility,
    target: &'a UiTargetCamera,
    focus: &'a FocusPolicy,
}

struct ImageView<'a> {
    parent: &'a ChildOf,
    node: &'a Node,
    image: &'a ImageNode,
    painted: &'a PaintedOrb,
    computed: &'a ComputedNode,
    transform: &'a UiGlobalTransform,
    visible: &'a InheritedVisibility,
    focus: &'a FocusPolicy,
}

/// Inspect actual ECS lineage and measured geometry; asset readiness is a
/// separate host observation so a failed load can never acquire image ownership.
fn measured_current_image(
    context: &HpOrbHostContext,
    model: &UiReadModel,
    root: RootView<'_>,
    image: ImageView<'_>,
    camera: Entity,
    expected_image: &Handle<Image>,
    loaded: bool,
) -> Option<(HpOrbRect, HpOrbRect, HpOrbRect)> {
    measured_current_image_side(
        context,
        model,
        root,
        image,
        camera,
        expected_image,
        loaded,
        OrbSide::Hp,
    )
}

#[allow(clippy::too_many_arguments)]
fn measured_current_image_side(
    context: &HpOrbHostContext,
    model: &UiReadModel,
    root: RootView<'_>,
    image: ImageView<'_>,
    camera: Entity,
    expected_image: &Handle<Image>,
    loaded: bool,
    side: OrbSide,
) -> Option<(HpOrbRect, HpOrbRect, HpOrbRect)> {
    let (slot, expected_stamp) = valid_context_side(context, model, side)?;
    let size = context.logical_size?;
    if root.target.0 != camera
        || *root.focus != FocusPolicy::Pass
        || root.node.display == Display::None
        || !root.visible.get()
        || !rect_matches(
            logical_rect(root.computed, root.transform),
            HpOrbRect {
                left: 0.0,
                top: 0.0,
                width: size.x,
                height: size.y,
            },
        )
        || image.parent.parent() != root.entity
        || *image.painted != expected_stamp
        || *image.focus != FocusPolicy::Pass
        || image.node.display == Display::None
        || !image.visible.get()
        || !loaded
        || image.image.image.id() != expected_image.id()
    {
        return None;
    }
    let ratio = match side {
        OrbSide::Hp => model.player.normalized_hp(),
        OrbSide::Mp => model.player.normalized_mp(),
    };
    let geometry = orb_geometry_at(ratio, side, expected_stamp.hp_only, slot.left, slot.top);
    let destination = HpOrbRect::from_crystal(geometry.destination);
    let layout = logical_rect(image.computed, image.transform);
    if image.image.rect != Some(geometry.source.bevy_rect())
        || !rect_matches(layout, destination)
        || image.node.width != Val::Px(destination.width)
        || image.node.height != Val::Px(destination.height)
        || !layout.width.is_finite()
        || !layout.height.is_finite()
        || (expected_stamp.value > 0 && layout.height < 1.0)
    {
        return None;
    }
    Some((HpOrbRect::from_source(geometry.source), destination, layout))
}

#[allow(clippy::too_many_arguments)]
fn observe_orb(
    context: Res<HpOrbHostContext>,
    model: Res<UiReadModel>,
    server: Res<AssetServer>,
    lease: Res<HpOrbImageLease>,
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
        With<HpOrbRoot>,
    >,
    images: Query<
        (
            Entity,
            &ChildOf,
            &Node,
            &ImageNode,
            &PaintedOrb,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            &FocusPolicy,
        ),
        With<HpOrbImage>,
    >,
    mut observation: ResMut<HpOrbObservation>,
) {
    *observation = HpOrbObservation::default();
    let Some((slot, _)) = valid_context(&context, &model) else {
        return;
    };
    let Some(target) = target else {
        return;
    };
    let Ok(render_target) = camera_targets.get(target.0) else {
        return;
    };
    let RenderTarget::Window(WindowRef::Entity(window_entity)) = render_target else {
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
        root_camera,
        root_focus,
    )) = roots.single()
    else {
        return;
    };
    let Ok((_entity, parent, node, image, painted, computed, transform, visible, focus)) =
        images.single()
    else {
        return;
    };
    let image_path = orb_image_path(OrbSide::Hp, painted.hp_only);
    let Some(expected_image) = lease.current(context.generation, painted.hp_only) else {
        return;
    };
    let loaded = matches!(
        server.get_load_state(image.image.id()),
        Some(LoadState::Loaded)
    );
    let Some((source, destination, layout)) = measured_current_image(
        &context,
        &model,
        RootView {
            entity: root_entity,
            node: root_node,
            computed: root_computed,
            transform: root_transform,
            visible: root_visible,
            target: root_camera,
            focus: root_focus,
        },
        ImageView {
            parent,
            node,
            image,
            painted,
            computed,
            transform,
            visible,
            focus,
        },
        target.0,
        expected_image,
        loaded,
    ) else {
        return;
    };
    observation.ready = true;
    observation.generation = painted.generation;
    observation.revision = painted.revision;
    observation.hp = painted.value;
    observation.max_hp = painted.max_value;
    observation.hp_only = painted.hp_only;
    observation.slot = Some(slot);
    observation.image = Some(image_path);
    observation.source = Some(source);
    observation.destination = Some(destination);
    observation.layout = Some(layout);
}

#[allow(clippy::too_many_arguments)]
fn observe_mp_orb(
    context: Res<HpOrbHostContext>,
    model: Res<UiReadModel>,
    server: Res<AssetServer>,
    lease: Res<HpOrbImageLease>,
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
        With<HpOrbRoot>,
    >,
    images: Query<
        (
            &ChildOf,
            &Node,
            &ImageNode,
            &PaintedOrb,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            &FocusPolicy,
        ),
        With<MpOrbImage>,
    >,
    mut observation: ResMut<MpOrbObservation>,
) {
    *observation = MpOrbObservation::default();
    let Some((slot, _)) = valid_context_side(&context, &model, OrbSide::Mp) else {
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
        root_camera,
        root_focus,
    )) = roots.single()
    else {
        return;
    };
    let Ok((parent, node, image, painted, computed, transform, visible, focus)) = images.single()
    else {
        return;
    };
    let Some(expected_image) = lease.current(context.generation, false) else {
        return;
    };
    let loaded = matches!(
        server.get_load_state(image.image.id()),
        Some(LoadState::Loaded)
    );
    let Some((source, destination, layout)) = measured_current_image_side(
        &context,
        &model,
        RootView {
            entity: root_entity,
            node: root_node,
            computed: root_computed,
            transform: root_transform,
            visible: root_visible,
            target: root_camera,
            focus: root_focus,
        },
        ImageView {
            parent,
            node,
            image,
            painted,
            computed,
            transform,
            visible,
            focus,
        },
        target.0,
        expected_image,
        loaded,
        OrbSide::Mp,
    ) else {
        return;
    };
    observation.ready = true;
    observation.generation = painted.generation;
    observation.revision = painted.revision;
    observation.mp = painted.value;
    observation.max_mp = painted.max_value;
    observation.hp_only = false;
    observation.slot = Some(slot);
    observation.image = Some(orb_image_path(OrbSide::Mp, false));
    observation.source = Some(source);
    observation.destination = Some(destination);
    observation.layout = Some(layout);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_model::PlayerStats;
    use bevy::asset::{
        io::{
            memory::{Dir, MemoryAssetReader},
            AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, Reader,
        },
        AssetApp, AssetPlugin,
    };
    use std::{
        path::Path,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        thread,
        time::{Duration, Instant},
    };

    #[derive(Clone)]
    struct CountingOrbReader {
        memory: MemoryAssetReader,
        split_reads: Arc<AtomicUsize>,
        hp_only_reads: Arc<AtomicUsize>,
        split_fail_through: Arc<AtomicUsize>,
        hp_only_fail_through: Arc<AtomicUsize>,
        hold_first_hp_only: Arc<AtomicBool>,
        hold_first_split: Arc<AtomicBool>,
    }

    impl AssetReader for CountingOrbReader {
        async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
            let attempt = if path == Path::new(orb_image_path(OrbSide::Hp, true)) {
                self.hp_only_reads.fetch_add(1, Ordering::SeqCst) + 1
            } else if path == Path::new(orb_image_path(OrbSide::Hp, false)) {
                self.split_reads.fetch_add(1, Ordering::SeqCst) + 1
            } else {
                return self.memory.read(path).await;
            };
            if path == Path::new(orb_image_path(OrbSide::Hp, true)) {
                let until = Instant::now() + Duration::from_secs(3);
                while attempt == 1
                    && self.hold_first_hp_only.load(Ordering::SeqCst)
                    && Instant::now() < until
                {
                    thread::sleep(Duration::from_millis(1));
                }
                if attempt <= self.hp_only_fail_through.load(Ordering::SeqCst) {
                    return Err(AssetReaderError::NotFound(path.to_path_buf()));
                }
            } else {
                let until = Instant::now() + Duration::from_secs(3);
                while attempt == 1
                    && self.hold_first_split.load(Ordering::SeqCst)
                    && Instant::now() < until
                {
                    thread::sleep(Duration::from_millis(1));
                }
                if attempt <= self.split_fail_through.load(Ordering::SeqCst) {
                    return Err(AssetReaderError::NotFound(path.to_path_buf()));
                }
            }
            self.memory.read(path).await
        }

        async fn read_meta<'a>(
            &'a self,
            path: &'a Path,
        ) -> Result<impl Reader + 'a, AssetReaderError> {
            self.memory.read_meta(path).await
        }

        async fn read_directory<'a>(
            &'a self,
            path: &'a Path,
        ) -> Result<Box<PathStream>, AssetReaderError> {
            self.memory.read_directory(path).await
        }

        async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
            self.memory.is_directory(path).await
        }
    }

    fn counted_app(
        fail_through: usize,
        hold_first: bool,
    ) -> (App, Arc<AtomicUsize>, Arc<AtomicUsize>, Arc<AtomicBool>) {
        let (app, split, hp_only, hold_hp, _) =
            counted_app_with_split(fail_through, hold_first, usize::MAX, false);
        (app, split, hp_only, hold_hp)
    }

    fn counted_app_with_split(
        fail_through: usize,
        hold_first: bool,
        split_fail_through: usize,
        hold_first_split: bool,
    ) -> (
        App,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
        Arc<AtomicBool>,
        Arc<AtomicBool>,
    ) {
        let root = Dir::default();
        root.insert_asset(
            Path::new(orb_image_path(OrbSide::Hp, true)),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../web/public/original-ui/Prguse/6.png"
            ))
            .as_slice(),
        );
        root.insert_asset(
            Path::new(orb_image_path(OrbSide::Mp, false)),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../web/public/original-ui/Prguse/4.png"
            ))
            .as_slice(),
        );
        let split_reads = Arc::new(AtomicUsize::new(0));
        let hp_only_reads = Arc::new(AtomicUsize::new(0));
        let hold_first_hp_only = Arc::new(AtomicBool::new(hold_first));
        let hold_first_split = Arc::new(AtomicBool::new(hold_first_split));
        let reader = CountingOrbReader {
            memory: MemoryAssetReader { root },
            split_reads: split_reads.clone(),
            hp_only_reads: hp_only_reads.clone(),
            split_fail_through: Arc::new(AtomicUsize::new(split_fail_through)),
            hp_only_fail_through: Arc::new(AtomicUsize::new(fail_through)),
            hold_first_hp_only: hold_first_hp_only.clone(),
            hold_first_split: hold_first_split.clone(),
        };
        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || Box::new(reader.clone())),
        );
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                watch_for_changes_override: Some(false),
                use_asset_processor_override: Some(false),
                ..default()
            },
            bevy::image::ImagePlugin::default(),
        ));
        app.register_asset_loader(bevy::image::ImageLoader::new(
            bevy::image::CompressedImageFormats::NONE,
        ));
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(model(50));
        let mut context = context();
        context.hp_only = true;
        app.insert_resource(context);
        app.world_mut().resource_mut::<UiReadModel>().player.level = 25;
        app.add_plugins(Mir2PortableHpOrbUiPlugin);
        (
            app,
            split_reads,
            hp_only_reads,
            hold_first_hp_only,
            hold_first_split,
        )
    }

    fn advance_until(app: &mut App, condition: impl Fn(&World) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            app.update();
            if condition(app.world()) {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }
        let world = app.world();
        let lease = world.resource::<HpOrbImageLease>();
        let server = world.resource::<AssetServer>();
        panic!(
            "asset fixture did not reach expected state: context={:?}, lease_generation={}, hp_only_state={:?}, split_state={:?}",
            world.resource::<HpOrbHostContext>(),
            lease.generation,
            lease.current(lease.generation, true).and_then(|h| server.get_load_state(h.id())),
            lease.current(lease.generation, false).and_then(|h| server.get_load_state(h.id())),
        );
    }

    fn model(hp: i32) -> UiReadModel {
        UiReadModel {
            player: PlayerStats {
                hp,
                max_hp: 100,
                level: 30,
                class_name: Some("Warrior".into()),
                ..default()
            },
        }
    }

    fn context() -> HpOrbHostContext {
        HpOrbHostContext {
            generation: 7,
            revision: 11,
            in_game: true,
            player_known: true,
            hp: 50,
            max_hp: 100,
            mp: None,
            max_mp: None,
            hp_only: false,
            slot: Some(HpOrbSlot {
                left: 0.0,
                top: 646.0,
            }),
            logical_size: Some(Vec2::new(1024.0, 768.0)),
            window_matches: true,
        }
    }

    fn ecs_fixture(
        hp: i32,
    ) -> (
        World,
        Entity,
        Entity,
        Entity,
        Handle<Image>,
        HpOrbHostContext,
        UiReadModel,
    ) {
        let mut world = World::new();
        let camera = world.spawn_empty().id();
        let mut context = context();
        context.hp = hp;
        let model = model(hp);
        let (_, stamp) = valid_context(&context, &model).expect("current fixture context");
        let root = world
            .spawn((
                HpOrbRoot,
                UiTargetCamera(camera),
                FocusPolicy::Pass,
                Node {
                    display: Display::Flex,
                    ..default()
                },
                ComputedNode {
                    size: Vec2::new(1024.0, 768.0),
                    ..default()
                },
                UiGlobalTransform::from_xy(512.0, 384.0),
                InheritedVisibility::VISIBLE,
            ))
            .id();
        let geometry =
            orb_geometry_at(model.player.normalized_hp(), OrbSide::Hp, false, 0.0, 646.0);
        let destination = geometry.destination;
        let handle = Handle::<Image>::default();
        let image = world
            .spawn((
                HpOrbImage,
                ChildOf(root),
                FocusPolicy::Pass,
                stamp,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(destination.left),
                    top: Val::Px(destination.top),
                    width: Val::Px(destination.width),
                    height: Val::Px(destination.height),
                    ..default()
                },
                ImageNode {
                    image: handle.clone(),
                    rect: Some(geometry.source.bevy_rect()),
                    ..default()
                },
                ComputedNode {
                    size: Vec2::new(destination.width, destination.height),
                    ..default()
                },
                UiGlobalTransform::from_xy(
                    destination.left + destination.width / 2.0,
                    destination.top + destination.height / 2.0,
                ),
                InheritedVisibility::VISIBLE,
            ))
            .id();
        (world, root, image, camera, handle, context, model)
    }

    fn ecs_measure(
        world: &World,
        root: Entity,
        child: Entity,
        camera: Entity,
        handle: &Handle<Image>,
        context: &HpOrbHostContext,
        model: &UiReadModel,
        loaded: bool,
    ) -> Option<(HpOrbRect, HpOrbRect, HpOrbRect)> {
        measured_current_image(
            context,
            model,
            RootView {
                entity: root,
                node: world.get::<Node>(root)?,
                computed: world.get::<ComputedNode>(root)?,
                transform: world.get::<UiGlobalTransform>(root)?,
                visible: world.get::<InheritedVisibility>(root)?,
                target: world.get::<UiTargetCamera>(root)?,
                focus: world.get::<FocusPolicy>(root)?,
            },
            ImageView {
                parent: world.get::<ChildOf>(child)?,
                node: world.get::<Node>(child)?,
                image: world.get::<ImageNode>(child)?,
                painted: world.get::<PaintedOrb>(child)?,
                computed: world.get::<ComputedNode>(child)?,
                transform: world.get::<UiGlobalTransform>(child)?,
                visible: world.get::<InheritedVisibility>(child)?,
                focus: world.get::<FocusPolicy>(child)?,
            },
            camera,
            handle,
            loaded,
        )
    }

    #[test]
    fn context_requires_current_player_epoch_slot_and_window() {
        let current = model(50);
        assert!(valid_context(&context(), &current).is_some());
        let mut stale = context();
        stale.revision = 12;
        stale.hp = 40;
        assert!(valid_context(&stale, &current).is_none());
        let mut missing = context();
        missing.player_known = false;
        assert!(valid_context(&missing, &current).is_none());
        missing = context();
        missing.max_hp = 0;
        assert!(valid_context(&missing, &current).is_none());
        missing = context();
        missing.window_matches = false;
        assert!(valid_context(&missing, &current).is_none());
        missing = context();
        missing.slot = Some(HpOrbSlot {
            left: 1000.0,
            top: 646.0,
        });
        assert!(valid_context(&missing, &current).is_none());
        missing = context();
        missing.in_game = false;
        assert!(valid_context(&missing, &current).is_none());
        missing = context();
        missing.hp = 0;
        assert!(valid_context(&missing, &model(0)).is_some());
    }

    #[test]
    fn measured_rect_uses_inverse_layout_scale_and_rejects_zero_default() {
        let computed = ComputedNode {
            size: Vec2::new(100.0, 80.0),
            inverse_scale_factor: 0.5,
            ..default()
        };
        let measured = logical_rect(&computed, &UiGlobalTransform::from_xy(50.0, 40.0));
        assert!(rect_matches(
            measured,
            HpOrbRect {
                left: 0.0,
                top: 0.0,
                width: 50.0,
                height: 40.0,
            }
        ));
        assert!(!rect_matches(
            logical_rect(&ComputedNode::default(), &UiGlobalTransform::default()),
            measured
        ));
    }

    #[test]
    fn ecs_current_tree_requires_loaded_image_layout_lineage_and_stamp() {
        let (mut world, root, child, camera, handle, context, model) = ecs_fixture(50);
        let (_, destination, layout) =
            ecs_measure(&world, root, child, camera, &handle, &context, &model, true)
                .expect("current measured tree");
        assert_eq!(destination, layout);
        assert!(
            ecs_measure(&world, root, child, camera, &handle, &context, &model, false).is_none()
        );
        world.entity_mut(child).insert(ComputedNode::default());
        assert!(
            ecs_measure(&world, root, child, camera, &handle, &context, &model, true).is_none()
        );
        world.entity_mut(child).insert(ComputedNode {
            size: Vec2::new(50.0, 40.0),
            ..default()
        });
        let mut stale = context;
        stale.revision += 1;
        assert!(ecs_measure(&world, root, child, camera, &handle, &stale, &model, true).is_none());
        let wrong_parent = world.spawn_empty().id();
        world.entity_mut(child).insert(ChildOf(wrong_parent));
        assert!(
            ecs_measure(&world, root, child, camera, &handle, &context, &model, true).is_none()
        );
    }

    #[test]
    fn ecs_zero_hp_is_a_measured_empty_image_not_an_unlaid_default() {
        let (mut world, root, child, camera, handle, context, model) = ecs_fixture(0);
        let (_, destination, layout) =
            ecs_measure(&world, root, child, camera, &handle, &context, &model, true)
                .expect("empty HP has a measured root and image");
        assert_eq!(destination.height, 0.0);
        assert_eq!(layout.height, 0.0);
        world.entity_mut(root).insert(ComputedNode::default());
        assert!(
            ecs_measure(&world, root, child, camera, &handle, &context, &model, true).is_none()
        );
    }

    #[test]
    fn headless_plugin_spawns_only_pass_through_image_and_hides_on_logout() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Image>();
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(QuestUiTargetCamera(camera));
        app.insert_resource(model(50));
        app.insert_resource(context());
        app.add_plugins(Mir2PortableHpOrbUiPlugin);
        app.update();
        let world = app.world_mut();
        let root = world
            .query_filtered::<Entity, With<HpOrbRoot>>()
            .single(world)
            .unwrap();
        let image = world
            .query_filtered::<Entity, With<HpOrbImage>>()
            .single(world)
            .unwrap();
        assert_eq!(world.get::<ChildOf>(image).unwrap().parent(), root);
        assert_eq!(world.get::<FocusPolicy>(root), Some(&FocusPolicy::Pass));
        assert_eq!(world.get::<FocusPolicy>(image), Some(&FocusPolicy::Pass));
        assert_eq!(world.get::<Node>(root).unwrap().display, Display::Flex);
        assert!(
            !world.resource::<HpOrbObservation>().ready,
            "a spawned but unlaid image must not own the DOM orb"
        );
        world.resource_mut::<HpOrbHostContext>().in_game = false;
        app.update();
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::None
        );
        assert!(!app.world().resource::<HpOrbObservation>().ready);
    }

    #[test]
    fn failed_reader_is_requested_once_per_generation_and_path_despite_repaint_and_hiding() {
        let (mut app, split_reads, hp_only_reads, _) = counted_app(usize::MAX, false);
        advance_until(&mut app, |world| {
            world
                .resource::<HpOrbImageLease>()
                .current(7, true)
                .is_some_and(|handle| {
                    matches!(
                        world.resource::<AssetServer>().get_load_state(handle.id()),
                        Some(LoadState::Failed(_))
                    )
                })
        });
        assert_eq!(hp_only_reads.load(Ordering::SeqCst), 1);
        for revision in 12..100 {
            let hp = 1 + revision as i32 % 99;
            app.world_mut().resource_mut::<UiReadModel>().player.hp = hp;
            let mut context = app.world_mut().resource_mut::<HpOrbHostContext>();
            context.revision = revision;
            context.hp = hp;
            context.slot = Some(HpOrbSlot {
                left: (revision % 20) as f32,
                top: 646.0,
            });
            drop(context);
            app.update();
        }
        app.world_mut().resource_mut::<HpOrbHostContext>().slot = None;
        for _ in 0..50 {
            app.update();
        }
        {
            let mut context = app.world_mut().resource_mut::<HpOrbHostContext>();
            context.slot = Some(HpOrbSlot {
                left: 0.0,
                top: 646.0,
            });
            context.window_matches = false;
        }
        for _ in 0..20 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<HpOrbHostContext>()
            .window_matches = true;
        app.update();
        assert_eq!(hp_only_reads.load(Ordering::SeqCst), 1);
        assert!(!app.world().resource::<HpOrbObservation>().ready);

        app.world_mut().resource_mut::<UiReadModel>().player.level = 30;
        app.world_mut().resource_mut::<HpOrbHostContext>().hp_only = false;
        advance_until(&mut app, |world| {
            world
                .resource::<HpOrbImageLease>()
                .current(7, false)
                .is_some_and(|handle| {
                    matches!(
                        world.resource::<AssetServer>().get_load_state(handle.id()),
                        Some(LoadState::Failed(_))
                    )
                })
        });
        assert_eq!(split_reads.load(Ordering::SeqCst), 1);
        app.world_mut().resource_mut::<UiReadModel>().player.level = 25;
        app.world_mut().resource_mut::<HpOrbHostContext>().hp_only = true;
        for _ in 0..80 {
            app.update();
        }
        assert_eq!(
            hp_only_reads.load(Ordering::SeqCst),
            1,
            "returning to a failed path must not rearm it"
        );

        app.world_mut()
            .resource_mut::<HpOrbHostContext>()
            .generation = 8;
        advance_until(&mut app, |_| hp_only_reads.load(Ordering::SeqCst) == 2);
        assert_eq!(hp_only_reads.load(Ordering::SeqCst), 2);
        for _ in 0..80 {
            app.update();
        }
        assert_eq!(hp_only_reads.load(Ordering::SeqCst), 2);

        let (mut fresh, _, fresh_reads, _) = counted_app(usize::MAX, false);
        advance_until(&mut fresh, |_| fresh_reads.load(Ordering::SeqCst) == 1);
        assert_eq!(
            fresh_reads.load(Ordering::SeqCst),
            1,
            "new App may request anew"
        );
    }

    #[test]
    fn loaded_image_repaints_current_geometry_without_new_reader_request() {
        let (mut app, _, hp_only_reads, _) = counted_app(0, false);
        advance_until(&mut app, |world| {
            world
                .resource::<HpOrbImageLease>()
                .current(7, true)
                .is_some_and(|handle| {
                    matches!(
                        world.resource::<AssetServer>().get_load_state(handle.id()),
                        Some(LoadState::Loaded)
                    )
                })
        });
        let original = app
            .world()
            .resource::<HpOrbImageLease>()
            .current(7, true)
            .unwrap()
            .id();
        app.world_mut().resource_mut::<UiReadModel>().player.hp = 25;
        {
            let mut context = app.world_mut().resource_mut::<HpOrbHostContext>();
            context.hp = 25;
            context.revision += 1;
            context.slot = Some(HpOrbSlot {
                left: 20.0,
                top: 646.0,
            });
        }
        app.update();
        let mut query = app
            .world_mut()
            .query_filtered::<(&Node, &ImageNode), With<HpOrbImage>>();
        let (node, image) = query.single(app.world()).unwrap();
        assert_eq!(node.left, Val::Px(20.0));
        assert_eq!(node.height, Val::Px(20.0));
        assert_eq!(image.image.id(), original);
        assert_eq!(hp_only_reads.load(Ordering::SeqCst), 1);

        app.world_mut().resource_mut::<UiReadModel>().player.hp = 0;
        app.world_mut().resource_mut::<HpOrbHostContext>().hp = 0;
        app.update();
        let (node, image) = query.single(app.world()).unwrap();
        assert_eq!(node.height, Val::Px(0.0));
        assert_eq!(image.image.id(), original);
        assert_eq!(hp_only_reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn new_generation_adopts_older_loading_then_retries_only_if_it_fails() {
        for fail_through in [0, 1] {
            let (mut app, _, hp_only_reads, hold) = counted_app(fail_through, true);
            advance_until(&mut app, |_| hp_only_reads.load(Ordering::SeqCst) == 1);
            app.world_mut()
                .resource_mut::<HpOrbHostContext>()
                .generation = 8;
            app.update();
            assert_eq!(
                hp_only_reads.load(Ordering::SeqCst),
                1,
                "overlap must adopt the in-flight request"
            );
            hold.store(false, Ordering::SeqCst);
            advance_until(&mut app, |world| {
                world
                    .resource::<HpOrbImageLease>()
                    .current(8, true)
                    .is_some_and(|handle| {
                        matches!(
                            world.resource::<AssetServer>().get_load_state(handle.id()),
                            Some(LoadState::Loaded)
                        )
                    })
            });
            assert_eq!(
                hp_only_reads.load(Ordering::SeqCst),
                if fail_through == 0 { 1 } else { 2 }
            );
            for _ in 0..80 {
                app.update();
            }
            assert_eq!(
                hp_only_reads.load(Ordering::SeqCst),
                if fail_through == 0 { 1 } else { 2 }
            );
        }
    }

    #[test]
    fn mp_requires_known_current_stats_and_obeys_native_class_threshold() {
        let mut context = context();
        let mut model = model(50);
        model.player.mp = 0;
        model.player.max_mp = 80;
        assert!(valid_context_side(&context, &model, OrbSide::Mp).is_none());
        context.mp = Some(0);
        context.max_mp = Some(80);
        assert!(valid_context_side(&context, &model, OrbSide::Mp).is_some());
        let geometry =
            orb_geometry_at(model.player.normalized_mp(), OrbSide::Mp, false, 0.0, 646.0);
        assert_eq!(geometry.destination.height, 0.0);
        assert_eq!(geometry.destination.left, 51.0);
        context.mp = Some(-1);
        assert!(valid_context_side(&context, &model, OrbSide::Mp).is_none());
        context.mp = Some(0);
        context.max_mp = Some(0);
        assert!(valid_context_side(&context, &model, OrbSide::Mp).is_none());
        context.max_mp = Some(80);
        model.player.level = 25;
        context.hp_only = true;
        assert!(valid_context_side(&context, &model, OrbSide::Mp).is_none());
        model.player.level = 26;
        context.hp_only = false;
        assert!(valid_context_side(&context, &model, OrbSide::Mp).is_some());
        model.player.class_name = Some("Wizard".into());
        model.player.level = 1;
        assert!(valid_context_side(&context, &model, OrbSide::Mp).is_some());
    }

    #[test]
    fn mp_ecs_measure_rejects_stale_tree_and_unlaid_zero() {
        let (mut world, root, _, camera, handle, mut context, mut model) = ecs_fixture(50);
        context.mp = Some(0);
        context.max_mp = Some(80);
        model.player.mp = 0;
        model.player.max_mp = 80;
        let (_, stamp) = valid_context_side(&context, &model, OrbSide::Mp).unwrap();
        let geometry = orb_geometry_at(0.0, OrbSide::Mp, false, 0.0, 646.0);
        let destination = geometry.destination;
        let child = world
            .spawn((
                MpOrbImage,
                ChildOf(root),
                FocusPolicy::Pass,
                stamp,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(destination.left),
                    top: Val::Px(destination.top),
                    width: Val::Px(destination.width),
                    height: Val::Px(destination.height),
                    ..default()
                },
                ImageNode {
                    image: handle.clone(),
                    rect: Some(geometry.source.bevy_rect()),
                    ..default()
                },
                ComputedNode {
                    size: Vec2::new(destination.width, 0.0),
                    ..default()
                },
                UiGlobalTransform::from_xy(
                    destination.left + destination.width / 2.0,
                    destination.top,
                ),
                InheritedVisibility::VISIBLE,
            ))
            .id();
        let measure = |world: &World, context: &HpOrbHostContext, loaded: bool| {
            measured_current_image_side(
                context,
                &model,
                RootView {
                    entity: root,
                    node: world.get::<Node>(root)?,
                    computed: world.get::<ComputedNode>(root)?,
                    transform: world.get::<UiGlobalTransform>(root)?,
                    visible: world.get::<InheritedVisibility>(root)?,
                    target: world.get::<UiTargetCamera>(root)?,
                    focus: world.get::<FocusPolicy>(root)?,
                },
                ImageView {
                    parent: world.get::<ChildOf>(child)?,
                    node: world.get::<Node>(child)?,
                    image: world.get::<ImageNode>(child)?,
                    painted: world.get::<PaintedOrb>(child)?,
                    computed: world.get::<ComputedNode>(child)?,
                    transform: world.get::<UiGlobalTransform>(child)?,
                    visible: world.get::<InheritedVisibility>(child)?,
                    focus: world.get::<FocusPolicy>(child)?,
                },
                camera,
                &handle,
                loaded,
                OrbSide::Mp,
            )
        };
        assert_eq!(measure(&world, &context, true).unwrap().1.height, 0.0);
        assert!(measure(&world, &context, false).is_none());
        world.entity_mut(child).insert(ComputedNode::default());
        assert!(measure(&world, &context, true).is_none());
        world.entity_mut(child).insert(ComputedNode {
            size: Vec2::new(50.0, 0.0),
            ..default()
        });
        context.revision += 1;
        assert!(measure(&world, &context, true).is_none());
        context.revision -= 1;
        context.slot = None;
        assert!(measure(&world, &context, true).is_none());
        context.slot = Some(HpOrbSlot {
            left: 0.0,
            top: 646.0,
        });
        world.entity_mut(child).get_mut::<ImageNode>().unwrap().rect = None;
        assert!(measure(&world, &context, true).is_none());
    }

    #[test]
    fn shared_split_loader_is_one_attempt_per_generation_for_hp_and_mp() {
        let (mut app, split_reads, _, _, _) =
            counted_app_with_split(usize::MAX, false, usize::MAX, false);
        {
            let mut model = app.world_mut().resource_mut::<UiReadModel>();
            model.player.level = 30;
            model.player.mp = 40;
            model.player.max_mp = 80;
        }
        {
            let mut context = app.world_mut().resource_mut::<HpOrbHostContext>();
            context.hp_only = false;
            context.mp = Some(40);
            context.max_mp = Some(80);
        }
        advance_until(&mut app, |world| {
            world
                .resource::<HpOrbImageLease>()
                .current(7, false)
                .is_some_and(|h| {
                    matches!(
                        world.resource::<AssetServer>().get_load_state(h.id()),
                        Some(LoadState::Failed(_))
                    )
                })
        });
        assert_eq!(split_reads.load(Ordering::SeqCst), 1);
        let mut query = app
            .world_mut()
            .query_filtered::<&ImageNode, With<MpOrbImage>>();
        assert_eq!(query.iter(app.world()).count(), 1);
        for revision in 12..80 {
            let mp = revision as i32 % 81;
            app.world_mut().resource_mut::<UiReadModel>().player.mp = mp;
            let mut context = app.world_mut().resource_mut::<HpOrbHostContext>();
            context.mp = Some(mp);
            context.revision = revision;
            drop(context);
            app.update();
        }
        app.world_mut().resource_mut::<HpOrbHostContext>().slot = None;
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(split_reads.load(Ordering::SeqCst), 1);
        assert!(!app.world().resource::<MpOrbObservation>().ready);
        app.world_mut().resource_mut::<HpOrbHostContext>().slot = context().slot;
        app.world_mut()
            .resource_mut::<HpOrbHostContext>()
            .generation = 8;
        advance_until(&mut app, |_| split_reads.load(Ordering::SeqCst) == 2);
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(split_reads.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn low_warrior_never_requests_split_for_mp_and_loaded_mp_repaints() {
        let (mut app, split_reads, hp_only_reads, _, _) =
            counted_app_with_split(0, false, 0, false);
        {
            let mut model = app.world_mut().resource_mut::<UiReadModel>();
            model.player.level = 25;
            model.player.mp = 40;
            model.player.max_mp = 80;
        }
        {
            let mut context = app.world_mut().resource_mut::<HpOrbHostContext>();
            context.hp_only = true;
            context.mp = Some(40);
            context.max_mp = Some(80);
        }
        advance_until(&mut app, |_| hp_only_reads.load(Ordering::SeqCst) == 1);
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(split_reads.load(Ordering::SeqCst), 0);
        assert!(!app.world().resource::<MpOrbObservation>().ready);
        app.world_mut().resource_mut::<UiReadModel>().player.level = 26;
        app.world_mut().resource_mut::<HpOrbHostContext>().hp_only = false;
        advance_until(&mut app, |world| {
            world
                .resource::<HpOrbImageLease>()
                .current(7, false)
                .is_some_and(|h| {
                    matches!(
                        world.resource::<AssetServer>().get_load_state(h.id()),
                        Some(LoadState::Loaded)
                    )
                })
        });
        assert_eq!(split_reads.load(Ordering::SeqCst), 1);
        app.world_mut().resource_mut::<UiReadModel>().player.mp = 0;
        {
            let mut context = app.world_mut().resource_mut::<HpOrbHostContext>();
            context.mp = Some(0);
            context.revision += 1;
        }
        app.update();
        let mut query = app
            .world_mut()
            .query_filtered::<(&Node, &ImageNode), With<MpOrbImage>>();
        let (node, image) = query.single(app.world()).unwrap();
        assert_eq!(node.left, Val::Px(51.0));
        assert_eq!(node.height, Val::Px(0.0));
        assert_eq!(
            image.rect,
            Some(
                orb_geometry_at(0.0, OrbSide::Mp, false, 0.0, 646.0)
                    .source
                    .bevy_rect()
            )
        );
        assert_eq!(split_reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn hp_and_mp_visibility_are_independent_on_the_shared_root() {
        let (mut app, split_reads, _, _, _) = counted_app_with_split(0, false, 0, false);
        app.update();
        app.update();
        let root = app
            .world_mut()
            .query_filtered::<Entity, With<HpOrbRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<MpOrbImage>>()
                .iter(app.world())
                .count(),
            0
        );
        {
            let mut model = app.world_mut().resource_mut::<UiReadModel>();
            model.player.level = 30;
            model.player.mp = 40;
            model.player.max_mp = 80;
            model.player.hp = 20;
        }
        {
            let mut context = app.world_mut().resource_mut::<HpOrbHostContext>();
            context.mp = Some(40);
            context.max_mp = Some(80);
            context.hp_only = false;
        }
        app.update();
        let hp = app
            .world_mut()
            .query_filtered::<Entity, With<HpOrbImage>>()
            .single(app.world())
            .unwrap();
        let mp = app
            .world_mut()
            .query_filtered::<Entity, With<MpOrbImage>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::Flex
        );
        assert_eq!(app.world().get::<Node>(hp).unwrap().display, Display::None);
        assert_eq!(app.world().get::<Node>(mp).unwrap().display, Display::Flex);
        assert_eq!(split_reads.load(Ordering::SeqCst), 1);
        app.world_mut().resource_mut::<HpOrbHostContext>().mp = None;
        app.update();
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::None
        );
    }

    #[test]
    fn split_generation_overlap_retries_once_and_updates_both_painted_handles() {
        for fail_through in [0, 1] {
            let (mut app, split_reads, _, _, hold) =
                counted_app_with_split(0, false, fail_through, true);
            {
                let mut model = app.world_mut().resource_mut::<UiReadModel>();
                model.player.level = 30;
                model.player.mp = 40;
                model.player.max_mp = 80;
            }
            {
                let mut context = app.world_mut().resource_mut::<HpOrbHostContext>();
                context.mp = Some(40);
                context.max_mp = Some(80);
                context.hp_only = false;
            }
            advance_until(&mut app, |_| split_reads.load(Ordering::SeqCst) == 1);
            app.world_mut()
                .resource_mut::<HpOrbHostContext>()
                .generation = 8;
            app.update();
            assert_eq!(split_reads.load(Ordering::SeqCst), 1);
            hold.store(false, Ordering::SeqCst);
            advance_until(&mut app, |world| {
                world
                    .resource::<HpOrbImageLease>()
                    .current(8, false)
                    .is_some_and(|handle| {
                        matches!(
                            world.resource::<AssetServer>().get_load_state(handle.id()),
                            Some(LoadState::Loaded)
                        )
                    })
            });
            app.update();
            let expected = app
                .world()
                .resource::<HpOrbImageLease>()
                .current(8, false)
                .unwrap()
                .id();
            let mut hp = app
                .world_mut()
                .query_filtered::<&ImageNode, With<HpOrbImage>>();
            let mut mp = app
                .world_mut()
                .query_filtered::<&ImageNode, With<MpOrbImage>>();
            assert_eq!(hp.single(app.world()).unwrap().image.id(), expected);
            assert_eq!(mp.single(app.world()).unwrap().image.id(), expected);
            assert_eq!(
                split_reads.load(Ordering::SeqCst),
                if fail_through == 0 { 1 } else { 2 }
            );
        }
    }
}
