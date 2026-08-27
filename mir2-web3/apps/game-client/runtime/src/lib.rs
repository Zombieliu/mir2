mod additive_material;
pub mod entity_animation;
mod entity_animation_bridge;
mod interpolation;
mod lighting;
mod local_motion;
mod motion;
mod movement_shadow;
#[cfg(not(target_arch = "wasm32"))]
pub mod native_ingest;
#[cfg(target_arch = "wasm32")]
#[path = "native_ingest_wasm.rs"]
mod native_ingest;
mod presentation_pose;
mod remote_motion;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use bevy::asset::{AssetMetaCheck, AssetPlugin, LoadState, RenderAssetUsages};
use bevy::camera::{visibility::RenderLayers, ClearColorConfig, RenderTarget};
use bevy::image::{Image, ImagePlugin, TextureAtlas, TextureAtlasLayout};
use bevy::math::{URect, UVec2};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::{CompositeAlphaMode, WindowResolution};
use js_sys::Function;
use mir2_client_bevy::pending_operations::{
    apply_inventory_operation_ack, apply_storage_operation_ack, mark_authoritative_refresh,
    reconcile_inventory_refresh, reconcile_mail_refresh, reconcile_shop_refresh,
    reconcile_storage_refresh, request_session_reset,
    request_session_reset_preserving_exact_game_shop_receipt, AuthoritativeModelDomain,
    AuthoritativeModelRevisions, InventoryOperationAck, InventoryOperationFeedback,
    PendingLifecycleSet, PendingOperationKey, PendingOperations, SessionResetGameShopPreservation,
    SessionResetRevision, StorageOperationAck,
};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

const TILE_SIZE: f32 = 32.0;
const FLOOR_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);
#[cfg(all(target_arch = "wasm32", feature = "webgpu"))]
const COMPILED_RENDER_BACKEND: &str = "webgpu";
#[cfg(all(target_arch = "wasm32", feature = "webgl2", not(feature = "webgpu")))]
const COMPILED_RENDER_BACKEND: &str = "webgl2";
#[cfg(not(any(
    all(target_arch = "wasm32", feature = "webgpu"),
    all(target_arch = "wasm32", feature = "webgl2", not(feature = "webgpu"))
)))]
const COMPILED_RENDER_BACKEND: &str = "native";

// Both wasm backends composite transparently over the DOM map/floor/UI layers, so
// Bevy can be the entity renderer on non-WebGPU too (the webgl2 build was opaque,
// which forced the DOM WebGl2EntityAtlasLayer to draw entities there).
//
// Alpha mode differs by backend: webgpu uses PreMultiplied; the webgl2 (wgpu GL)
// surface needs Auto — PreMultiplied makes the GL backend composite the drawn
// sprites to fully transparent (entities vanish while the DOM map still shows
// through), so Auto is required for visible entities on webgl2.
#[cfg(all(target_arch = "wasm32", feature = "webgpu"))]
const WINDOW_COMPOSITE_ALPHA_MODE: CompositeAlphaMode = CompositeAlphaMode::PreMultiplied;
#[cfg(not(all(target_arch = "wasm32", feature = "webgpu")))]
const WINDOW_COMPOSITE_ALPHA_MODE: CompositeAlphaMode = CompositeAlphaMode::Auto;

thread_local! {
    static STATUS_SINK: RefCell<Option<Function>> = const { RefCell::new(None) };
    static PENDING_WORLD_STATE: RefCell<Option<WorldSnapshot>> = const { RefCell::new(None) };
    static PENDING_ENTITY_RENDER_STATE: RefCell<Option<EntityRenderState>> = const { RefCell::new(None) };
    static PENDING_ENTITY_RENDER_ATLASES: RefCell<Vec<PendingEntityRenderAtlasImage>> = const { RefCell::new(Vec::new()) };
    static PENDING_MAP_RENDER_STATE: RefCell<Option<MapRenderState>> = const { RefCell::new(None) };
    static PENDING_MAP_RENDER_IMAGE_OPS: RefCell<Vec<PendingMapRenderImageOp>> = const { RefCell::new(Vec::new()) };
    static PENDING_MAP_CAMERA_OFFSET: Cell<(f32, f32)> = const { Cell::new((0.0, 0.0)) };
    static PENDING_EFFECT_RENDER_STATE: RefCell<Option<EffectRenderState>> = const { RefCell::new(None) };
    static PENDING_LIGHTING_RENDER_STATE: RefCell<Option<lighting::LightingRenderState>> = const { RefCell::new(None) };
    static PENDING_SCENE_RESET: Cell<bool> = const { Cell::new(false) };
    // Optional self-player motion window (from_x, from_y, to_x, to_y, started_ms,
    // expires_ms) for the display-Hz camera-scroll path (?bevySelfCamera=1). None
    // (the default) ⇒ `follow_player` keeps the camera pinned at origin = the
    // current fold-in behaviour.
    static PENDING_SELF_CAMERA_MOTION: Cell<Option<(f32, f32, f32, f32, f64, f64)>> = const { Cell::new(None) };
}

#[derive(Resource, Default, Clone)]
pub(crate) struct RuntimeWorldState {
    pub(crate) snapshot: Option<WorldSnapshot>,
}

#[derive(Resource, Default, Clone)]
struct RuntimeEntityRenderState {
    snapshot: Option<EntityRenderState>,
}

#[derive(Resource, Default, Clone)]
struct RuntimeEntityRenderAtlases {
    images: HashMap<String, Handle<Image>>,
}

#[derive(Resource, Default, Clone)]
struct RuntimeMapRenderState {
    snapshot: Option<MapRenderState>,
}

#[derive(Resource, Default, Clone)]
struct RuntimeEffectRenderState {
    snapshot: Option<EffectRenderState>,
}

#[derive(Resource, Default, Clone)]
struct RuntimeLightingRenderState {
    snapshot: Option<lighting::LightingRenderState>,
}

#[derive(Resource, Default)]
struct RuntimeLightingSceneResetTracker(u64);

#[derive(Resource, Default)]
struct RuntimeSessionResetTracker(u64);

#[derive(Resource, Default)]
struct SceneResetRevision(u64);

#[derive(Resource, Default)]
struct RuntimeSceneResetTracker(u64);

#[derive(Resource, Default)]
struct RuntimeSceneModelResetTracker(u64);

#[derive(Resource, Default)]
struct RuntimeEffectShadowCleanupTracker(u64);

/// Production native session-boundary pipeline, factored as a plugin so host
/// state-machine tests can exercise the exact same reset and receipt systems
/// without creating a renderer/window.
pub struct Mir2NativeSessionBoundaryPlugin;

impl Plugin for Mir2NativeSessionBoundaryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<mir2_client_bevy::read_model::UiReadModel>()
            .init_resource::<mir2_client_bevy::read_model::UiSurfaceSignals>()
            .init_resource::<mir2_client_bevy::map::MapModel>()
            .init_resource::<mir2_client_bevy::entities::EntityModelSet>()
            .init_resource::<mir2_client_bevy::inventory::InventoryModel>()
            .init_resource::<mir2_client_bevy::chat::ChatModel>()
            .init_resource::<mir2_client_bevy::mail::MailModel>()
            .init_resource::<mir2_client_bevy::shop::ShopModel>()
            .init_resource::<mir2_client_bevy::game_shop::GameShopModel>()
            .init_resource::<mir2_client_bevy::storage::StorageModel>()
            .init_resource::<mir2_client_bevy::skill_model::SkillModel>()
            .init_resource::<mir2_client_bevy::social::SocialModel>()
            .init_resource::<PendingOperations>()
            .init_resource::<InventoryOperationFeedback>()
            .init_resource::<AuthoritativeModelRevisions>()
            .init_resource::<SessionResetRevision>()
            .init_resource::<SessionResetGameShopPreservation>()
            .init_resource::<RuntimeSessionResetTracker>()
            .init_resource::<SceneResetRevision>()
            .init_resource::<native_ingest::NativeInbound>()
            .configure_sets(
                Update,
                PendingLifecycleSet::Ingest.before(PendingLifecycleSet::UiReset),
            )
            .add_systems(
                Update,
                (
                    ingest_pending_scene_and_data_reset,
                    apply_session_reset_to_runtime_models,
                    ingest_pending_game_shop_receipt,
                )
                    .chain()
                    .in_set(PendingLifecycleSet::Ingest),
            )
            .add_systems(PostUpdate, finalize_consumed_game_shop_preservation);
    }
}

/// Map-tile atlas registry. Deliberately SEPARATE from
/// `RuntimeEntityRenderAtlases` so the entity render path's atlas-layout
/// retain logic (which evicts layouts not referenced by the entity snapshot)
/// cannot evict the map's layouts, and vice versa.
#[derive(Resource, Default, Clone)]
struct RuntimeMapRenderAtlases {
    images: HashMap<String, Handle<Image>>,
    url_image_keys: HashSet<String>,
    layouts: HashMap<String, (Handle<TextureAtlasLayout>, HashMap<String, usize>)>,
    layout_rects: HashMap<String, HashMap<String, URect>>,
    layout_sizes: HashMap<String, UVec2>,
    revision: u64,
}

/// Per-frame sub-tile camera scroll offset (screen-stage pixels). In the
/// fold-in camera model the map tiles already include the offset in their
/// `left`/`top`, so this stays (0, 0); the resource is still read by
/// `sync_map_render` and folded into each tile's spawn position, so an
/// alternative offset-model producer that pushes a non-zero value keeps working
/// (applied whenever the tile set is respawned).
#[derive(Resource, Default, Clone, Copy)]
struct RuntimeMapCameraOffset {
    x: f32,
    y: f32,
}

#[derive(Resource, Default)]
struct SceneRegistry {
    entities: HashMap<String, SceneEntityHandles>,
    entity_render_layers: HashMap<String, EntityRenderLayerHandle>,
    entity_render_atlases: HashMap<String, EntityRenderAtlasHandle>,
    effect_render: HashMap<String, EffectRenderLayerHandle>,
    effect_render_masks: HashMap<String, EffectRenderLayerHandle>,
    effect_render_shadows: HashMap<String, EffectShadowLayerHandle>,
    effect_render_images: HashMap<String, Handle<Image>>,
    lighting_layers: HashMap<String, EffectRenderLayerHandle>,
    lighting_images: Vec<Handle<Image>>,
    lighting_darkness: Option<LightingDarknessHandle>,
    map: MapSceneCache,
    map_render: MapRenderSceneCache,
    mine_nodes: HashMap<(i32, i32), MineNodeHandles>,
}

/// Renderer-side counts used by the opt-in Windows native soak diagnostics.
///
/// These are deliberately separate counters instead of one aggregate scene
/// count: `entities` is the legacy object renderer, while
/// `entity_render_layers` is the retained native layer renderer. Combining
/// them would make a healthy renderer switch look like a leak.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NativeSoakCounts {
    snapshot_effects: usize,
    retained_effect_primary: usize,
    retained_effect_masks: usize,
    retained_effect_shadows: usize,
    retained_effect_images: usize,
    retained_entity_layers: usize,
    legacy_scene_entities: usize,
    entity_atlases: usize,
    map_render_tiles: usize,
    map_spawned_entities: usize,
    mine_nodes: usize,
    lighting_layers: usize,
    lighting_images: usize,
    additive_cache_entries: usize,
    additive_cache_live_entries: usize,
    additive_asset_count: usize,
}

/// Take a renderer-only snapshot without touching ECS entities or the native
/// ingest queue. The registry maps are the authoritative retained counts;
/// Bevy despawn commands are deferred and therefore unsuitable as same-frame
/// leak metrics.
#[cfg(not(target_arch = "wasm32"))]
fn native_soak_counts(
    registry: &SceneRegistry,
    effect_state: &RuntimeEffectRenderState,
    additive_cache: &additive_material::CrystalAdditiveMaterialCache,
    additive_materials: &Assets<additive_material::CrystalAdditiveMaterial>,
) -> NativeSoakCounts {
    NativeSoakCounts {
        snapshot_effects: effect_state
            .snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.effects.len()),
        retained_effect_primary: registry.effect_render.len(),
        retained_effect_masks: registry.effect_render_masks.len(),
        retained_effect_shadows: registry.effect_render_shadows.len(),
        retained_effect_images: registry.effect_render_images.len(),
        retained_entity_layers: registry.entity_render_layers.len(),
        legacy_scene_entities: registry.entities.len(),
        entity_atlases: registry.entity_render_atlases.len(),
        map_render_tiles: registry.map_render.tiles.len(),
        map_spawned_entities: registry.map.spawned.len(),
        mine_nodes: registry.mine_nodes.len(),
        lighting_layers: registry.lighting_layers.len(),
        lighting_images: registry.lighting_images.len(),
        additive_cache_entries: additive_cache.len(),
        additive_cache_live_entries: additive_cache.live_len(additive_materials),
        additive_asset_count: additive_materials.len(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
const NATIVE_SOAK_METRICS_INTERVAL_MS: u64 = 10_000;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct NativeSoakMetricsClock {
    initialized: bool,
    enabled: bool,
    last_sample_ms: Option<u64>,
}

#[cfg(not(target_arch = "wasm32"))]
fn native_soak_metrics_json(
    process_id: u32,
    timestamp_ms: u64,
    counts: &NativeSoakCounts,
) -> String {
    serde_json::json!({
        "processId": process_id,
        "timestampMs": timestamp_ms,
        "snapshotEffects": counts.snapshot_effects,
        "retainedEffectPrimary": counts.retained_effect_primary,
        "retainedEffectMasks": counts.retained_effect_masks,
        "retainedEffectShadows": counts.retained_effect_shadows,
        "retainedEffectImages": counts.retained_effect_images,
        "retainedEntityLayers": counts.retained_entity_layers,
        "legacySceneEntities": counts.legacy_scene_entities,
        "entityAtlases": counts.entity_atlases,
        "mapRenderTiles": counts.map_render_tiles,
        "mapSpawnedEntities": counts.map_spawned_entities,
        "mineNodes": counts.mine_nodes,
        "lightingLayers": counts.lighting_layers,
        "lightingImages": counts.lighting_images,
        "additiveCacheEntries": counts.additive_cache_entries,
        "additiveCacheLiveEntries": counts.additive_cache_live_entries,
        "additiveAssetCount": counts.additive_asset_count,
    })
    .to_string()
}

/// Emit one compact, local-only diagnostic line at most once per 10 seconds.
/// The environment is read once on the first system run, so the normal path
/// adds no per-frame environment lookup and no output. WASM never registers
/// this system and therefore remains a no-op.
#[cfg(not(target_arch = "wasm32"))]
fn emit_native_soak_metrics(
    time: Res<Time>,
    registry: Res<SceneRegistry>,
    effect_state: Res<RuntimeEffectRenderState>,
    additive_cache: Res<additive_material::CrystalAdditiveMaterialCache>,
    additive_materials: Res<Assets<additive_material::CrystalAdditiveMaterial>>,
    mut clock: Local<NativeSoakMetricsClock>,
) {
    if !clock.initialized {
        clock.enabled = std::env::var("MIR2_NATIVE_SOAK_METRICS")
            .map(|value| value.trim() == "1")
            .unwrap_or(false);
        clock.initialized = true;
    }
    if !clock.enabled {
        return;
    }

    let elapsed_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    if clock
        .last_sample_ms
        .is_some_and(|last| elapsed_ms.saturating_sub(last) < NATIVE_SOAK_METRICS_INTERVAL_MS)
    {
        return;
    }
    clock.last_sample_ms = Some(elapsed_ms);

    let counts = native_soak_counts(
        &registry,
        &effect_state,
        &additive_cache,
        &additive_materials,
    );
    let line = native_soak_metrics_json(std::process::id(), elapsed_ms, &counts);
    eprintln!("[native-soak] {line}");
}

#[derive(Default)]
struct MapSceneCache {
    blueprint: Option<MapSceneBlueprint>,
    spawned: Vec<Entity>,
}

/// Cache for the Bevy-native map-tile renderer: one TOP-LEVEL sprite entity per
/// tile (mirroring the entity render layers — NOT children of a shared root, so
/// no hierarchy visibility/transform propagation is needed for them to render),
/// keyed by the tile's stable `key` so `sync_map_render` RETAINS entities and
/// updates their Transform/Sprite IN PLACE across motion frames (only spawning
/// tiles that entered the viewport + despawning those that left) — exactly like
/// `sync_entity_render_layers`, instead of despawning + respawning all ~470
/// tiles every sub-cell-motion frame. `applied` stores only a lightweight
/// producer/image revision fingerprint, not a clone of the complete draw list.
#[derive(Default)]
struct MapRenderSceneCache {
    tiles: HashMap<String, MapRenderTileHandle>,
    applied: Option<AppliedMapRenderState>,
    generation: u64,
}

#[derive(Clone, Copy)]
struct MapRenderTileHandle {
    entity: Entity,
    last_seen_generation: u64,
}

#[derive(Clone, Copy)]
struct AppliedMapRenderState {
    producer_revision: Option<u64>,
    image_revision: u64,
}

#[derive(Clone, Copy)]
struct SceneEntityHandles {
    root: Entity,
    shadow: Entity,
    body: Entity,
    crest: Entity,
    facing: Entity,
    selection: Entity,
}

#[derive(Clone)]
struct EntityRenderLayerHandle {
    entity: Entity,
    image_key: String,
    atlas_key: Option<String>,
    atlas_rect_key: Option<String>,
    additive: bool,
}

#[derive(Clone)]
struct EntityRenderAtlasHandle {
    layout: Handle<TextureAtlasLayout>,
    rects: HashMap<String, usize>,
    uv_rects: HashMap<String, URect>,
    size: UVec2,
    image_key: Option<String>,
    image: Option<Handle<Image>>,
}

/// Retained additive scene-effect sprite (map/ground/cast/projectile/impact).
/// Keyed by the producer stable effect sprite key so sync_effect_render
/// updates Transform/Material IN PLACE instead of despawn + respawn per frame.
#[derive(Clone)]
struct EffectRenderLayerHandle {
    entity: Entity,
    image_key: String,
    additive: bool,
}

/// Retained procedural ground shadow for a scene effect. The mesh and material
/// are owned by the handle so removing an effect can release both GPU-facing
/// assets immediately instead of relying on asset-server ref counting.
#[derive(Clone)]
struct EffectShadowLayerHandle {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<ColorMaterial>,
}

#[derive(Clone)]
struct LightingDarknessHandle {
    composite_entity: Entity,
    buffer_camera: Entity,
    buffer_image: Handle<Image>,
    stage_size: UVec2,
    material: Handle<lighting::CrystalMultiplyMaterial>,
}

struct PendingEntityRenderAtlasImage {
    key: String,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct MirObject;

#[derive(Component)]
struct MirEntityRenderLayer;

#[derive(Component)]
struct MirEffectRenderLayer;

#[derive(Component)]
struct MirLightingBufferLayer;

#[derive(Component)]
struct MirLightingBufferCamera;

#[derive(Component)]
struct MirLightingComposite;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorldSnapshot {
    #[serde(default)]
    pub(crate) map_title: Option<String>,
    #[serde(default)]
    pub(crate) player_object_id: Option<String>,
    #[serde(default)]
    pub(crate) selected_object_id: Option<String>,
    #[serde(default)]
    pub(crate) scene_view: Option<SceneView>,
    #[serde(default)]
    pub(crate) terrain_patches: Vec<TerrainPatch>,
    #[serde(default)]
    pub(crate) decor_objects: Vec<DecorObject>,
    #[serde(default)]
    pub(crate) entities: Vec<WorldEntity>,
    #[serde(default)]
    pub(crate) mine_nodes: Vec<MineNode>,
    /// Optional timestamp (milliseconds) supplied by the TypeScript producer.
    /// Accepted for forward-compatibility; the runtime currently uses the Bevy
    /// local receipt time as the interpolation clock so browser-clock skew
    /// cannot distort lerp alpha.
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) client_time_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SceneView {
    center: GridPoint,
    width: u16,
    height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GridPoint {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerrainPatch {
    x: i32,
    y: i32,
    width: u16,
    height: u16,
    kind: TerrainKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TerrainKind {
    Grass,
    Dirt,
    Road,
    Water,
    Stone,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecorObject {
    id: String,
    x: i32,
    y: i32,
    kind: DecorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum DecorKind {
    Lantern,
    Banner,
    Tree,
    Rock,
    Campfire,
    Stump,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorldEntity {
    pub(crate) object_id: String,
    pub(crate) kind: EntityKind,
    pub(crate) name: String,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) direction: Option<String>,
    pub(crate) level: Option<u16>,
    /// Wall-clock `Date.now()` milliseconds at which the movement step began.
    /// Populated by the TS producer from `entity.movementStartedAt`.
    /// When absent the motion module uses the snapshot receipt time.
    #[serde(default)]
    pub(crate) movement_started_ms: Option<f64>,
    /// Duration of this movement step in milliseconds.
    /// Populated by the TS producer as `movementUntil - movementStartedAt`.
    /// When absent the motion module defaults to `motion::DEFAULT_STEP_DURATION_MS`.
    #[serde(default)]
    pub(crate) movement_duration_ms: Option<f64>,
}

/// A mineable cell's depletion stage, driven by the server `MineNodeState`
/// packet. Rendered as a coloured marker that fades as the vein is mined out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MineNode {
    x: i32,
    y: i32,
    stage: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityRenderState {
    enabled: bool,
    stage_width: f32,
    stage_height: f32,
    #[serde(default)]
    center_x: Option<i32>,
    #[serde(default)]
    center_y: Option<i32>,
    #[serde(default)]
    atlases: Vec<EntityRenderAtlas>,
    #[serde(default)]
    entities: Vec<EntityRenderEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityRenderAtlas {
    key: String,
    width: u32,
    height: u32,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    rects: Vec<EntityRenderAtlasRect>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityRenderAtlasRect {
    key: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityRenderEntry {
    object_id: String,
    #[serde(default)]
    is_self: bool,
    #[serde(default)]
    grid_x: Option<i32>,
    #[serde(default)]
    grid_y: Option<i32>,
    #[serde(default)]
    layers: Vec<EntityRenderLayer>,
    // Opt-in (`?bevyEntityInterp=1`) per-entity sub-cell motion window, in CSS-px
    // cell coordinates (48 × 32). Present only for NON-self entities under the
    // flag; absent (`None`) on the default path and for the self player ⇒ no
    // interpolation ⇒ byte-identical to the producer's fold. `from`/`to` are `f32`
    // because a move that begins mid-glide records a fractional `from` (see
    // `motion::compute_motion_offset_fractional`); the timestamp stays `f64`.
    #[serde(default)]
    motion_from_x: Option<f32>,
    #[serde(default)]
    motion_from_y: Option<f32>,
    #[serde(default)]
    motion_to_x: Option<f32>,
    #[serde(default)]
    motion_to_y: Option<f32>,
    #[serde(default)]
    motion_started_ms: Option<f64>,
    #[serde(default)]
    motion_duration_ms: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityRenderLayer {
    key: String,
    path: String,
    #[serde(default)]
    atlas_key: Option<String>,
    #[serde(default)]
    atlas_rect_key: Option<String>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    z: f32,
    #[serde(default)]
    opacity: Option<f32>,
    #[serde(default)]
    additive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum EntityKind {
    Player,
    SelfPlayer,
    Monster,
    Npc,
}

/// Snapshot of the current map-tile draw list pushed from the TS producer
/// (`buildMapTileDrawList`'s output). Mirrors `EntityRenderState` so the map
/// can be the Bevy-native renderer behind entities, at exact visual parity
/// with the DOM `WebGl2MapAtlasLayer`. Producer-side semantic deduplication keeps
/// revisions tied to real draw-list changes; the runtime retains tiles by key.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapRenderState {
    enabled: bool,
    stage_width: f32,
    stage_height: f32,
    #[serde(default)]
    ack_key: String,
    #[serde(default)]
    revision: Option<u64>,
    #[serde(default)]
    center_x: Option<i32>,
    #[serde(default)]
    center_y: Option<i32>,
    /// Atlas page descriptors (key + page dims + the source rects within the
    /// page). Carries the rect geometry the per-tile `atlas_rect_key` indexes
    /// into; mirrors `EntityRenderState.atlases` so the same layout-building
    /// logic applies. Optional/additive — pixels arrive separately via
    /// `setMir2MapRenderAtlas`.
    #[serde(default)]
    atlases: Vec<MapRenderAtlas>,
    #[serde(default)]
    tiles: Vec<MapTile>,
    #[serde(default)]
    standalone_tiles: Vec<MapStandaloneTile>,
    #[serde(default)]
    retained_image_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapRenderAtlas {
    key: String,
    width: u32,
    height: u32,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    rects: Vec<MapRenderAtlasRect>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapRenderAtlasRect {
    key: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapTile {
    // Stable per-tile identity (ViewportMapSprite key = spriteId:cellX:cellY:frame);
    // stable across sub-cell camera motion for static tiles. `sync_map_render` keys
    // its retained tile entities by this so it updates Transform/Sprite in place
    // instead of despawning + respawning the whole set every motion frame.
    key: String,
    atlas_key: String,
    // The TS producer feeds `buildMapTileDrawList`'s output verbatim, whose tile
    // field is `rectKey` (MapTileDraw); accept that wire name while keeping the
    // descriptive Rust field name.
    #[serde(rename = "rectKey")]
    atlas_rect_key: String,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    z: f32,
    #[serde(default)]
    opacity: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapStandaloneTile {
    key: String,
    image_key: String,
    #[serde(default)]
    image_url: Option<String>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    z: f32,
    #[serde(default)]
    opacity: Option<f32>,
    #[serde(default)]
    additive: bool,
}

fn map_render_active_image_keys(snapshot: &MapRenderState) -> HashSet<String> {
    snapshot
        .atlases
        .iter()
        .map(|atlas| atlas.key.clone())
        .chain(
            snapshot
                .standalone_tiles
                .iter()
                .map(|tile| tile.image_key.clone()),
        )
        .chain(snapshot.retained_image_keys.iter().cloned())
        .collect()
}

fn map_render_url_image_sources(snapshot: &MapRenderState) -> Vec<(String, String)> {
    snapshot
        .atlases
        .iter()
        .filter_map(|atlas| {
            atlas
                .image_url
                .as_deref()
                .map(|url| (atlas.key.clone(), browser_asset_path(url)))
        })
        .chain(snapshot.standalone_tiles.iter().filter_map(|tile| {
            tile.image_url
                .as_deref()
                .map(|url| (tile.image_key.clone(), browser_asset_path(url)))
        }))
        .collect()
}

/// Snapshot of the active scene-effect sprites pushed by the native Windows
/// producer (apps/game-client/platform-windows/src/effects.rs). Each entry is
/// one additive (Crystal SourceAlpha + One) sprite placed in screen-stage
/// coordinates, mirroring MapStandaloneTile + EntityRenderLayer. The native
/// producer only emits entries whose source frame PNG actually exists under the
/// asset root, so a missing asset never produces a sprite here.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EffectRenderState {
    enabled: bool,
    stage_width: f32,
    stage_height: f32,
    #[serde(default)]
    effects: Vec<EffectRenderEntry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EffectRenderEntry {
    /// Stable per-effect-sprite identity reused for retained in-place updates.
    key: String,
    /// Optional standalone PNG URL (native keyed effect frame). When absent
    /// the producer supplied a keyed atlas image via an out-of-band upload.
    #[serde(default)]
    image_url: Option<String>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    z: f32,
    #[serde(default)]
    additive: bool,
    /// Crystal DrawBlend rate; omitted legacy entries retain full strength.
    #[serde(default)]
    opacity: Option<f32>,
    #[serde(default)]
    mask_image_url: Option<String>,
    /// Mask geometry (optional). mask_x/mask_y and frame_x/frame_y are both
    /// expressed in the same local (frame) origin; the mask is placed at
    /// (mask_x - frame_x, mask_y - frame_y) relative to the primary anchor.
    #[serde(default)]
    mask_width: Option<f32>,
    #[serde(default)]
    mask_height: Option<f32>,
    #[serde(default)]
    mask_x: Option<f32>,
    #[serde(default)]
    mask_y: Option<f32>,
    #[serde(default)]
    frame_x: Option<f32>,
    #[serde(default)]
    frame_y: Option<f32>,
    #[serde(default)]
    shadow_x: Option<f32>,
    #[serde(default)]
    shadow_y: Option<f32>,
}

/// Active image keys for an effect snapshot (URL-loaded standalone frames,
/// including mask frames).
fn effect_render_active_image_keys(snapshot: &EffectRenderState) -> HashSet<String> {
    snapshot
        .effects
        .iter()
        .flat_map(|effect| {
            let mut keys = Vec::new();
            if let Some(url) = effect.image_url.as_ref() {
                keys.push(browser_asset_path(url));
            }
            if let Some(url) = effect.mask_image_url.as_ref() {
                keys.push(browser_asset_path(url));
            }
            keys
        })
        .collect()
}

struct PendingMapRenderAtlasImage {
    key: String,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

enum PendingMapRenderImageOp {
    Upload(PendingMapRenderAtlasImage),
    Evict(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MapSceneBlueprint {
    map_title: Option<String>,
    scene_view: Option<SceneView>,
    terrain_patches: Vec<TerrainPatch>,
    decor_objects: Vec<DecorObject>,
}

#[wasm_bindgen(js_name = setMir2StatusSink)]
pub fn set_mir2_status_sink(callback: Function) {
    STATUS_SINK.with(|sink| {
        sink.borrow_mut().replace(callback);
    });
}

#[wasm_bindgen(js_name = clearMir2StatusSink)]
pub fn clear_mir2_status_sink() {
    STATUS_SINK.with(|sink| {
        sink.borrow_mut().take();
    });
}

#[wasm_bindgen(js_name = getMir2RendererBackend)]
pub fn get_mir2_renderer_backend() -> String {
    COMPILED_RENDER_BACKEND.to_owned()
}

#[wasm_bindgen(js_name = resolveMir2EntityAnimationPoses)]
pub fn resolve_mir2_entity_animation_poses(snapshot_json: String) -> String {
    entity_animation_bridge::resolve_json(&snapshot_json)
}

#[wasm_bindgen(js_name = resetMir2EntityAnimations)]
pub fn reset_mir2_entity_animations() {
    entity_animation_bridge::reset();
}

#[wasm_bindgen(js_name = pushMir2MovementShadowEvent)]
pub fn push_mir2_movement_shadow_event(event_json: String) {
    local_motion::enqueue_local_motion_event_json(event_json.clone());
    remote_motion::enqueue_remote_motion_event_json(event_json.clone());
    movement_shadow::enqueue_movement_shadow_event_json(event_json);
}

#[wasm_bindgen(js_name = getMir2MovementShadowDiagnostics)]
pub fn get_mir2_movement_shadow_diagnostics() -> String {
    movement_shadow::get_movement_shadow_diagnostics_json()
}

#[wasm_bindgen(js_name = setMir2RemoteMotionPresentationEnabled)]
pub fn set_mir2_remote_motion_presentation_enabled(enabled: bool) {
    remote_motion::set_remote_motion_presentation_enabled(enabled);
}

#[wasm_bindgen(js_name = getMir2RemoteMotionPresentationDiagnostics)]
pub fn get_mir2_remote_motion_presentation_diagnostics() -> String {
    remote_motion::get_remote_motion_presentation_diagnostics_json()
}

#[wasm_bindgen(js_name = getMir2LocalMotionDiagnostics)]
pub fn get_mir2_local_motion_diagnostics() -> String {
    local_motion::get_local_motion_diagnostics_json()
}

#[wasm_bindgen(js_name = setMir2LocalMotionPresentationEnabled)]
pub fn set_mir2_local_motion_presentation_enabled(enabled: bool) {
    local_motion::set_local_motion_presentation_enabled(enabled);
}

#[wasm_bindgen(js_name = getMir2PresentationPoses)]
pub fn get_mir2_presentation_poses() -> String {
    presentation_pose::get_presentation_pose_json()
}

#[wasm_bindgen(js_name = setMir2PresentationPoseEnabled)]
pub fn set_mir2_presentation_pose_enabled(enabled: bool) {
    presentation_pose::set_presentation_pose_enabled(enabled);
}

#[wasm_bindgen(js_name = setMir2PresentationPoseSink)]
pub fn set_mir2_presentation_pose_sink(callback: Function) {
    presentation_pose::set_presentation_pose_sink(callback);
}

#[wasm_bindgen(js_name = clearMir2PresentationPoseSink)]
pub fn clear_mir2_presentation_pose_sink() {
    presentation_pose::clear_presentation_pose_sink();
}

#[wasm_bindgen(js_name = setMir2WorldState)]
pub fn set_mir2_world_state(snapshot_json: String) {
    match serde_json::from_str::<WorldSnapshot>(&snapshot_json) {
        Ok(snapshot) => {
            PENDING_WORLD_STATE.with(|pending| {
                *pending.borrow_mut() = Some(snapshot);
            });
        }
        Err(error) => publish_status("decode-error", &error.to_string()),
    }
}

/// Drop only retained scene/world presentation state. Inventory, mail, shop,
/// storage, HUD read models, UI pending operations, and login/session state are
/// intentionally left untouched. Native hosts use the matching
/// `push_native_scene_reset` entry point.
#[wasm_bindgen(js_name = resetMir2Scene)]
pub fn reset_mir2_scene() {
    PENDING_SCENE_RESET.with(|pending| pending.set(true));
}

#[wasm_bindgen(js_name = setMir2EntityRenderState)]
pub fn set_mir2_entity_render_state(snapshot_json: String) {
    match serde_json::from_str::<EntityRenderState>(&snapshot_json) {
        Ok(snapshot) => {
            PENDING_ENTITY_RENDER_STATE.with(|pending| {
                *pending.borrow_mut() = Some(snapshot);
            });
        }
        Err(error) => publish_status("entity-render-decode-error", &error.to_string()),
    }
}

#[wasm_bindgen(js_name = setMir2EntityRenderAtlas)]
pub fn set_mir2_entity_render_atlas(key: String, width: u32, height: u32, pixels: Vec<u8>) {
    if width == 0 || height == 0 {
        publish_status("entity-render-atlas-error", "Ignoring empty entity atlas");
        return;
    }

    let expected_len = width as usize * height as usize * 4;
    if pixels.len() != expected_len {
        publish_status(
            "entity-render-atlas-error",
            &format!(
                "Ignoring entity atlas {key}: expected {expected_len} RGBA bytes, got {}",
                pixels.len()
            ),
        );
        return;
    }

    PENDING_ENTITY_RENDER_ATLASES.with(|pending| {
        pending.borrow_mut().push(PendingEntityRenderAtlasImage {
            key,
            width,
            height,
            pixels,
        });
    });
}

#[wasm_bindgen(js_name = setMir2MapRenderState)]
pub fn set_mir2_map_render_state(json: String) {
    match serde_json::from_str::<MapRenderState>(&json) {
        Ok(snapshot) => {
            PENDING_MAP_RENDER_STATE.with(|pending| {
                *pending.borrow_mut() = Some(snapshot);
            });
        }
        Err(error) => publish_status("map-render-decode-error", &error.to_string()),
    }
}

#[wasm_bindgen(js_name = setMir2EffectRenderState)]
pub fn set_mir2_effect_render_state(json: String) {
    match serde_json::from_str::<EffectRenderState>(&json) {
        Ok(snapshot) => {
            PENDING_EFFECT_RENDER_STATE.with(|pending| {
                *pending.borrow_mut() = Some(snapshot);
            });
        }
        Err(error) => publish_status("effect-render-decode-error", &error.to_string()),
    }
}

/// Replace the retained native lighting snapshot. The schema is shared with
/// the Windows ingress and intentionally remains usable by a future WASM host.
#[wasm_bindgen(js_name = setMir2LightingRenderState)]
pub fn set_mir2_lighting_render_state(json: String) {
    match serde_json::from_str::<lighting::LightingRenderState>(&json) {
        Ok(snapshot) => {
            PENDING_LIGHTING_RENDER_STATE.with(|pending| {
                *pending.borrow_mut() = Some(snapshot);
            });
        }
        Err(error) => publish_status("lighting-render-decode-error", &error.to_string()),
    }
}

#[wasm_bindgen(js_name = setMir2MapRenderAtlas)]
pub fn set_mir2_map_render_atlas(key: String, width: u32, height: u32, pixels: Vec<u8>) {
    if width == 0 || height == 0 {
        publish_status("map-render-atlas-error", "Ignoring empty map atlas");
        return;
    }

    let expected_len = width as usize * height as usize * 4;
    if pixels.len() != expected_len {
        publish_status(
            "map-render-atlas-error",
            &format!(
                "Ignoring map atlas {key}: expected {expected_len} RGBA bytes, got {}",
                pixels.len()
            ),
        );
        return;
    }

    PENDING_MAP_RENDER_IMAGE_OPS.with(|pending| {
        pending.borrow_mut().push(PendingMapRenderImageOp::Upload(
            PendingMapRenderAtlasImage {
                key,
                width,
                height,
                pixels,
            },
        ));
    });
}

#[wasm_bindgen(js_name = evictMir2MapRenderImages)]
pub fn evict_mir2_map_render_images(keys_json: String) {
    queue_mir2_map_render_image_release(keys_json);
}

#[wasm_bindgen(js_name = releaseMir2MapRenderImages)]
pub fn release_mir2_map_render_images(keys_json: String) {
    queue_mir2_map_render_image_release(keys_json);
}

fn queue_mir2_map_render_image_release(keys_json: String) {
    match serde_json::from_str::<Vec<String>>(&keys_json) {
        Ok(keys) => PENDING_MAP_RENDER_IMAGE_OPS.with(|pending| {
            pending
                .borrow_mut()
                .extend(keys.into_iter().map(PendingMapRenderImageOp::Evict));
        }),
        Err(error) => publish_status("map-render-evict-decode-error", &error.to_string()),
    }
}

#[wasm_bindgen(js_name = setMir2MapCameraOffset)]
pub fn set_mir2_map_camera_offset(x: f32, y: f32) {
    PENDING_MAP_CAMERA_OFFSET.with(|cell| cell.set((x, y)));
}

/// Push the self-player's current motion window so the runtime can interpolate the
/// camera scroll at display refresh rate (instead of the ~33Hz React `motionNow`
/// clock). Opt-in: only the `?bevySelfCamera=1` producer calls this. Mirrors
/// `EntityMotionSnapshot` (`fromX,fromY,toX,toY,startedAt,expiresAt`). When the step
/// has elapsed (`now >= expires_ms`) the camera falls back to origin.
#[wasm_bindgen(js_name = setMir2SelfCameraMotion)]
pub fn set_mir2_self_camera_motion(
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    started_ms: f64,
    expires_ms: f64,
) {
    PENDING_SELF_CAMERA_MOTION
        .with(|cell| cell.set(Some((from_x, from_y, to_x, to_y, started_ms, expires_ms))));
}

/// Window/surface configuration for a Mir2 runtime host.
///
/// Platform-neutral so the same Bevy app can open a Web canvas (WASM host), a
/// native desktop window (Windows/macOS host) or a future Android surface.
#[derive(Debug, Clone)]
pub struct RuntimeWindowSpec {
    /// Web host: DOM canvas selector (e.g. `#mir2-web3-canvas`). Native hosts
    /// leave this `None` so winit creates a real OS window.
    pub canvas_selector: Option<String>,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub transparent: bool,
    pub fit_canvas_to_parent: bool,
    pub prevent_default_event_handling: bool,
    pub composite_alpha_mode: CompositeAlphaMode,
    /// Native hosts: filesystem root the AssetServer resolves relative paths
    /// against (e.g. the repo `apps/web` so `public/` atlas images load). WASM
    /// hosts leave this empty (`"."` — asset loading goes through the JS fetch
    /// path, not the filesystem).
    pub asset_root: String,
}

impl RuntimeWindowSpec {
    /// Web WASM host: transparent overlay over the DOM map/floor/UI layers.
    pub fn web() -> Self {
        Self {
            canvas_selector: Some("#mir2-web3-canvas".to_owned()),
            title: "mir2-web3".to_owned(),
            width: 1280,
            height: 720,
            transparent: true,
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            composite_alpha_mode: WINDOW_COMPOSITE_ALPHA_MODE,
            asset_root: ".".to_owned(),
        }
    }

    /// Native desktop host: a real OS window, opaque surface, no canvas.
    pub fn native(title: impl Into<String>) -> Self {
        Self {
            canvas_selector: None,
            title: title.into(),
            width: 1280,
            height: 720,
            transparent: false,
            fit_canvas_to_parent: false,
            prevent_default_event_handling: false,
            composite_alpha_mode: CompositeAlphaMode::Auto,
            asset_root: ".".to_owned(),
        }
    }
}

/// Build the shared Mir2 Bevy app for any host without running it.
///
/// The WASM `boot_mir2_runtime` entry and every native host (Windows, macOS,
/// later Android) call this with their own [`RuntimeWindowSpec`]. No DOM, canvas
/// selector or wasm API is assumed here.
pub fn build_runtime_app(spec: RuntimeWindowSpec) -> App {
    let mut app = App::new();
    app.insert_resource(ClearColor(FLOOR_COLOR))
        .insert_resource(RuntimeWorldState::default())
        .insert_resource(RuntimeEntityRenderState::default())
        .insert_resource(RuntimeEntityRenderAtlases::default())
        .insert_resource(RuntimeMapRenderState::default())
        .insert_resource(RuntimeMapRenderAtlases::default())
        .insert_resource(RuntimeEffectRenderState::default())
        .insert_resource(RuntimeLightingRenderState::default())
        .insert_resource(RuntimeLightingSceneResetTracker::default())
        .insert_resource(RuntimeMapCameraOffset::default())
        .insert_resource(SceneRegistry::default())
        .insert_resource(interpolation::SnapshotBuffer::default())
        .insert_resource(motion::EntityMotionTable::default())
        .insert_resource(presentation_pose::PresentationPoseBuffer::default())
        .insert_resource(mir2_client_bevy::read_model::UiReadModel::default())
        .insert_resource(mir2_client_bevy::read_model::UiSurfaceSignals::default())
        .insert_resource(mir2_client_bevy::map::MapModel::default())
        .insert_resource(mir2_client_bevy::entities::EntityModelSet::default())
        .insert_resource(mir2_client_bevy::inventory::InventoryModel::default())
        .insert_resource(mir2_client_bevy::chat::ChatModel::default())
        .insert_resource(mir2_client_bevy::mail::MailModel::default())
        .insert_resource(mir2_client_bevy::shop::ShopModel::default())
        .insert_resource(mir2_client_bevy::game_shop::GameShopModel::default())
        .insert_resource(mir2_client_bevy::storage::StorageModel::default())
        .insert_resource(mir2_client_bevy::skill_model::SkillModel::default())
        .insert_resource(mir2_client_bevy::social::SocialModel::default())
        .insert_resource(PendingOperations::default())
        .insert_resource(InventoryOperationFeedback::default())
        .insert_resource(AuthoritativeModelRevisions::default())
        .insert_resource(SessionResetRevision::default())
        .insert_resource(SessionResetGameShopPreservation::default())
        .insert_resource(RuntimeSessionResetTracker::default())
        .insert_resource(SceneResetRevision::default())
        .insert_resource(RuntimeSceneResetTracker::default())
        .insert_resource(RuntimeSceneModelResetTracker::default())
        .insert_resource(RuntimeEffectShadowCleanupTracker::default())
        .insert_resource(native_ingest::NativeInbound::new())
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: spec.asset_root.clone(),
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        canvas: spec.canvas_selector.clone(),
                        composite_alpha_mode: spec.composite_alpha_mode,
                        fit_canvas_to_parent: spec.fit_canvas_to_parent,
                        prevent_default_event_handling: spec.prevent_default_event_handling,
                        resolution: WindowResolution::new(spec.width, spec.height),
                        title: spec.title,
                        transparent: spec.transparent,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins((
            Mir2NativeSessionBoundaryPlugin,
            additive_material::CrystalAdditiveMaterialPlugin,
            lighting::CrystalMultiplyMaterialPlugin,
            motion::CrystalMoveClockPlugin,
            local_motion::LocalMotionPresentationShadowPlugin,
            movement_shadow::MovementShadowPlugin,
            remote_motion::RemoteMotionPresentationPlugin,
        ))
        .add_systems(Startup, setup_scene)
        .configure_sets(
            Update,
            PendingLifecycleSet::Ingest.before(PendingLifecycleSet::UiReset),
        )
        .add_systems(
            Update,
            (
                apply_scene_reset_to_runtime,
                cleanup_reset_effect_shadows,
                apply_scene_reset_to_lighting,
                apply_scene_reset_to_scene_models,
                ingest_pending_world_state,
            )
                .chain()
                .after(ingest_pending_game_shop_receipt)
                .in_set(PendingLifecycleSet::Ingest),
        )
        .add_systems(
            Update,
            (
                motion::update_entity_motion_table,
                ingest_pending_entity_render_state,
                ingest_pending_entity_render_atlases,
                ingest_pending_map_render_state,
            )
                .chain()
                .after(ingest_pending_world_state)
                .in_set(PendingLifecycleSet::Ingest),
        )
        .add_systems(
            Update,
            (
                ingest_pending_map_render_images,
                ingest_pending_effect_render_state,
                ingest_pending_lighting_render_state,
            )
                .chain()
                .after(ingest_pending_map_render_state)
                .in_set(PendingLifecycleSet::Ingest),
        )
        .add_systems(
            Update,
            (
                ingest_pending_ui_read_model,
                ingest_pending_map_model,
                ingest_pending_entity_model_set,
                ingest_pending_inventory_model,
            )
                .chain()
                .after(ingest_pending_effect_render_state)
                .in_set(PendingLifecycleSet::Ingest),
        )
        .add_systems(
            Update,
            (
                ingest_pending_inventory_operation_ack,
                ingest_pending_wallet_patch,
                ingest_pending_mail_model,
                ingest_pending_shop_model,
                ingest_pending_game_shop_info,
                ingest_pending_game_shop_stock,
                ingest_pending_npc_shop_service,
                ingest_pending_storage_patch,
                ingest_pending_storage_items,
                ingest_pending_storage_model,
                ingest_pending_skill_model,
                ingest_pending_social_model,
                ingest_pending_chat_line,
            )
                .chain()
                .after(ingest_pending_inventory_model)
                .in_set(PendingLifecycleSet::Ingest),
        )
        .add_systems(
            Update,
            (
                sync_map_render,
                sync_map_scene,
                sync_effect_render,
                sync_lighting_render,
                sync_entities,
                sync_mine_nodes,
                begin_presentation_pose_frame,
                sync_entity_render_layers,
                follow_player,
                publish_presentation_pose_frame,
            )
                .chain(),
        );
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(
        Update,
        emit_native_soak_metrics.after(publish_presentation_pose_frame),
    );
    app
}

#[wasm_bindgen(js_name = bootMir2Runtime)]
pub fn boot_mir2_runtime() {
    console_error_panic_hook::set_once();

    publish_status("runtime-entered", "Bevy runtime entry reached");

    let mut app = build_runtime_app(RuntimeWindowSpec::web());

    publish_status("running", "Handing off to Bevy app loop");
    app.run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera));
    publish_status("scene-ready", "Camera ready");
}

fn ingest_pending_world_state(
    mut state: ResMut<RuntimeWorldState>,
    mut snap_buf: ResMut<interpolation::SnapshotBuffer>,
    time: Res<Time>,
    native: Res<native_ingest::NativeInbound>,
) {
    // WASM path: thread-local cells written by the JS host.
    PENDING_WORLD_STATE.with(|pending| {
        if let Some(snapshot) = pending.borrow_mut().take() {
            apply_world_snapshot(&mut state, &mut snap_buf, time.elapsed_secs_f64(), snapshot);
        }
    });
    // Native path: cross-thread channel written by the gateway client.
    native.drain_matching(
        |message| matches!(message, native_ingest::NativeInboundMessage::WorldState(_)),
        |message| {
            if let native_ingest::NativeInboundMessage::WorldState(json) = message {
                if let Ok(snapshot) = serde_json::from_str::<WorldSnapshot>(&json) {
                    apply_world_snapshot(
                        &mut state,
                        &mut snap_buf,
                        time.elapsed_secs_f64(),
                        snapshot,
                    );
                } else {
                    publish_status("native-decode-error", "invalid native world snapshot");
                }
            }
        },
    );
}

fn apply_world_snapshot(
    state: &mut RuntimeWorldState,
    snap_buf: &mut interpolation::SnapshotBuffer,
    receipt_secs: f64,
    snapshot: WorldSnapshot,
) {
    // The receipt time is the Bevy elapsed seconds; if the TS producer
    // supplied `clientTimeMs` we note it but still use local time as the
    // authoritative clock so browser-clock skew can't distort lerp.
    let positions = snapshot
        .entities
        .iter()
        .map(|e| {
            (
                e.object_id.clone(),
                interpolation::EntityPos { x: e.x, y: e.y },
            )
        })
        .collect();
    snap_buf.push(interpolation::BufferedSnapshot {
        receipt_secs,
        positions,
    });

    state.snapshot = Some(snapshot);
}

fn ingest_pending_entity_render_state(
    mut state: ResMut<RuntimeEntityRenderState>,
    native: Res<native_ingest::NativeInbound>,
) {
    PENDING_ENTITY_RENDER_STATE.with(|pending| {
        if let Some(snapshot) = pending.borrow_mut().take() {
            state.snapshot = Some(snapshot);
        }
    });
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::EntityRenderState(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::EntityRenderState(json) = message {
                if let Ok(snapshot) = serde_json::from_str::<EntityRenderState>(&json) {
                    state.snapshot = Some(snapshot);
                } else {
                    publish_status("native-decode-error", "invalid native entity render state");
                }
            }
        },
    );
}

fn ingest_pending_entity_render_atlases(
    mut atlas_resource: ResMut<RuntimeEntityRenderAtlases>,
    mut images: ResMut<Assets<Image>>,
    native: Res<native_ingest::NativeInbound>,
) {
    PENDING_ENTITY_RENDER_ATLASES.with(|pending| {
        for atlas in pending.borrow_mut().drain(..) {
            let image = Image::new(
                Extent3d {
                    width: atlas.width,
                    height: atlas.height,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                atlas.pixels,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            let handle = images.add(image);
            atlas_resource.images.insert(atlas.key, handle);
        }
    });
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::EntityRenderAtlas { .. }
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::EntityRenderAtlas {
                key,
                width,
                height,
                pixels,
            } = message
            {
                let expected_len = (width as usize)
                    .checked_mul(height as usize)
                    .and_then(|pixels| pixels.checked_mul(4));
                if width == 0 || height == 0 || expected_len != Some(pixels.len()) {
                    publish_status("entity-render-atlas-error", "invalid native atlas pixels");
                    return;
                }
                let image = Image::new(
                    Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    pixels,
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                );
                let handle = images.add(image);
                atlas_resource.images.insert(key, handle);
            }
        },
    );
}

fn ingest_pending_ui_read_model(
    mut ui: ResMut<mir2_client_bevy::read_model::UiReadModel>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| matches!(message, native_ingest::NativeInboundMessage::UiReadModel(_)),
        |message| {
            if let native_ingest::NativeInboundMessage::UiReadModel(json) = message {
                match serde_json::from_str::<mir2_client_bevy::read_model::UiReadModel>(&json) {
                    Ok(model) => {
                        *ui = model;
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native ui read model");
                        eprintln!("[runtime] ui read model decode error: {error}");
                    }
                }
            }
        },
    );
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletPatch {
    #[serde(default)]
    gold: Option<u32>,
    #[serde(default)]
    credit: Option<u32>,
}

/// Apply packet-first absolute wallet values after the most recent full HUD
/// snapshot. The gateway computes these values from the authoritative packet
/// stream; this runtime only merges fields that are explicitly present.
fn ingest_pending_wallet_patch(
    mut ui: ResMut<mir2_client_bevy::read_model::UiReadModel>,
    mut inventory: ResMut<mir2_client_bevy::inventory::InventoryModel>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| matches!(message, native_ingest::NativeInboundMessage::WalletPatch(_)),
        |message| {
            if let native_ingest::NativeInboundMessage::WalletPatch(json) = message {
                match serde_json::from_str::<WalletPatch>(&json) {
                    Ok(patch) if patch.gold.is_some() || patch.credit.is_some() => {
                        if let Some(gold) = patch.gold {
                            ui.player.gold = gold;
                            inventory.gold = gold;
                        }
                        if let Some(credit) = patch.credit {
                            ui.player.credit = credit;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native wallet patch");
                        eprintln!("[runtime] wallet patch decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_map_model(
    mut map: ResMut<mir2_client_bevy::map::MapModel>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| matches!(message, native_ingest::NativeInboundMessage::MapModel(_)),
        |message| {
            if let native_ingest::NativeInboundMessage::MapModel(json) = message {
                match serde_json::from_str::<mir2_client_bevy::map::MapModel>(&json) {
                    Ok(model) => {
                        *map = model;
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native map model");
                        eprintln!("[runtime] map model decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_entity_model_set(
    mut entities: ResMut<mir2_client_bevy::entities::EntityModelSet>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::EntityModelSet(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::EntityModelSet(json) = message {
                match serde_json::from_str::<mir2_client_bevy::entities::EntityModelSet>(&json) {
                    Ok(model) => {
                        *entities = model;
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native entity model set");
                        eprintln!("[runtime] entity model set decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_inventory_model(
    mut inventory: ResMut<mir2_client_bevy::inventory::InventoryModel>,
    storage: Res<mir2_client_bevy::storage::StorageModel>,
    mut revisions: ResMut<AuthoritativeModelRevisions>,
    mut pending: ResMut<PendingOperations>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::InventoryModel(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::InventoryModel(json) = message {
                match serde_json::from_str::<mir2_client_bevy::inventory::InventoryModel>(&json) {
                    Ok(model) => {
                        let old = inventory.clone();
                        reconcile_inventory_refresh(&mut pending, &old, &model);
                        *inventory = model;
                        let storage_snapshot = storage.clone();
                        reconcile_storage_refresh(
                            &mut pending,
                            &inventory,
                            &storage_snapshot,
                            &storage_snapshot,
                        );
                        mark_authoritative_refresh(
                            &mut revisions,
                            AuthoritativeModelDomain::Inventory,
                        );
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native inventory model");
                        eprintln!("[runtime] inventory model decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_inventory_operation_ack(
    mut pending: ResMut<PendingOperations>,
    mut feedback: ResMut<InventoryOperationFeedback>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::InventoryOperationAck(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::InventoryOperationAck(json) = message {
                match serde_json::from_str::<InventoryOperationAck>(&json) {
                    Ok(ack) => {
                        apply_inventory_operation_ack(&mut pending, &mut feedback, ack);
                    }
                    Err(error) => {
                        publish_status(
                            "native-decode-error",
                            "invalid inventory operation acknowledgement",
                        );
                        eprintln!("[runtime] inventory operation ack decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_scene_and_data_reset(
    mut scene_reset: ResMut<SceneResetRevision>,
    mut reset: ResMut<SessionResetRevision>,
    mut revisions: ResMut<AuthoritativeModelRevisions>,
    mut pending: ResMut<PendingOperations>,
    mut preservation: ResMut<SessionResetGameShopPreservation>,
    native: Res<native_ingest::NativeInbound>,
) {
    PENDING_SCENE_RESET.with(|requested| {
        if requested.replace(false) {
            discard_pending_scene_thread_locals();
            scene_reset.0 = scene_reset.0.wrapping_add(1);
        }
    });
    native.discard_stale_data_before_latest_reset();
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::SceneReset
                    | native_ingest::NativeInboundMessage::DataReset
                    | native_ingest::NativeInboundMessage::DataResetPreservingExactGameShopReceipt(
                        _
                    )
            )
        },
        |message| {
            scene_reset.0 = scene_reset.0.wrapping_add(1);
            match message {
                native_ingest::NativeInboundMessage::DataReset => {
                    // DataReset is a session boundary and therefore includes
                    // the complete SceneReset semantics as well.
                    request_session_reset(&mut reset, &mut revisions, &mut pending);
                }
                native_ingest::NativeInboundMessage::DataResetPreservingExactGameShopReceipt(
                    receipt,
                ) => {
                    let _ = request_session_reset_preserving_exact_game_shop_receipt(
                        &mut reset,
                        &mut revisions,
                        &mut pending,
                        &mut preservation,
                        receipt,
                    );
                }
                native_ingest::NativeInboundMessage::SceneReset => {}
                _ => unreachable!("reset consumer matched a non-reset message"),
            }
        },
    );
}

fn discard_pending_scene_thread_locals() {
    PENDING_WORLD_STATE.with(|pending| *pending.borrow_mut() = None);
    PENDING_ENTITY_RENDER_STATE.with(|pending| *pending.borrow_mut() = None);
    PENDING_ENTITY_RENDER_ATLASES.with(|pending| pending.borrow_mut().clear());
    PENDING_MAP_RENDER_STATE.with(|pending| *pending.borrow_mut() = None);
    PENDING_MAP_RENDER_IMAGE_OPS.with(|pending| pending.borrow_mut().clear());
    PENDING_MAP_CAMERA_OFFSET.with(|offset| offset.set((0.0, 0.0)));
    PENDING_EFFECT_RENDER_STATE.with(|pending| *pending.borrow_mut() = None);
    PENDING_LIGHTING_RENDER_STATE.with(|pending| *pending.borrow_mut() = None);
}

fn apply_scene_reset_to_runtime(
    reset: Res<SceneResetRevision>,
    mut tracker: ResMut<RuntimeSceneResetTracker>,
    mut state: ResMut<RuntimeWorldState>,
    mut entity_render_state: ResMut<RuntimeEntityRenderState>,
    mut entity_atlases: ResMut<RuntimeEntityRenderAtlases>,
    mut map_render_state: ResMut<RuntimeMapRenderState>,
    mut map_atlases: ResMut<RuntimeMapRenderAtlases>,
    mut effect_render_state: ResMut<RuntimeEffectRenderState>,
    mut map_camera_offset: ResMut<RuntimeMapCameraOffset>,
    mut snapshots: ResMut<interpolation::SnapshotBuffer>,
    mut motion_table: ResMut<motion::EntityMotionTable>,
    mut presentation_poses: ResMut<presentation_pose::PresentationPoseBuffer>,
    mut registry: ResMut<SceneRegistry>,
    mut commands: Commands,
    mut additive_materials: ResMut<Assets<additive_material::CrystalAdditiveMaterial>>,
    mut additive_cache: ResMut<additive_material::CrystalAdditiveMaterialCache>,
) {
    if tracker.0 == reset.0 {
        return;
    }
    tracker.0 = reset.0;

    state.snapshot = None;
    entity_render_state.snapshot = None;
    entity_atlases.images.clear();
    map_render_state.snapshot = None;
    *map_atlases = RuntimeMapRenderAtlases::default();
    effect_render_state.snapshot = None;
    *map_camera_offset = RuntimeMapCameraOffset::default();
    *snapshots = interpolation::SnapshotBuffer::default();
    *motion_table = motion::EntityMotionTable::default();
    *presentation_poses = presentation_pose::PresentationPoseBuffer::default();

    clear_scene_registry(
        &mut commands,
        &mut registry,
        &mut additive_cache,
        &mut additive_materials,
    );
}

fn cleanup_reset_effect_shadows(
    reset: Res<SceneResetRevision>,
    mut tracker: ResMut<RuntimeEffectShadowCleanupTracker>,
    mut registry: ResMut<SceneRegistry>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut shadow_materials: ResMut<Assets<ColorMaterial>>,
) {
    if tracker.0 == reset.0 {
        return;
    }
    tracker.0 = reset.0;

    for (_, handle) in registry.effect_render_shadows.drain() {
        commands.entity(handle.entity).despawn();
        meshes.remove(handle.mesh.id());
        shadow_materials.remove(handle.material.id());
    }
}

fn apply_scene_reset_to_lighting(
    reset: Res<SceneResetRevision>,
    mut tracker: ResMut<RuntimeLightingSceneResetTracker>,
    mut state: ResMut<RuntimeLightingRenderState>,
) {
    if tracker.0 == reset.0 {
        return;
    }
    tracker.0 = reset.0;
    state.snapshot = None;
}

fn apply_scene_reset_to_scene_models(
    reset: Res<SceneResetRevision>,
    mut tracker: ResMut<RuntimeSceneModelResetTracker>,
    mut map: ResMut<mir2_client_bevy::map::MapModel>,
    mut entities: ResMut<mir2_client_bevy::entities::EntityModelSet>,
    mut surface_signals: ResMut<mir2_client_bevy::read_model::UiSurfaceSignals>,
    mut shop: ResMut<mir2_client_bevy::shop::ShopModel>,
    mut social: ResMut<mir2_client_bevy::social::SocialModel>,
) {
    if tracker.0 == reset.0 {
        return;
    }
    tracker.0 = reset.0;
    *map = mir2_client_bevy::map::MapModel::default();
    *entities = mir2_client_bevy::entities::EntityModelSet::default();
    social.clear_scene();
    surface_signals.npc_shop_open_requested = false;
    let _ = shop.apply_service_signal(mir2_client_bevy::shop::NpcShopServiceSignal::default());
}

fn apply_session_reset_to_runtime_models(
    reset: Res<SessionResetRevision>,
    mut tracker: ResMut<RuntimeSessionResetTracker>,
    mut ui: ResMut<mir2_client_bevy::read_model::UiReadModel>,
    mut surface_signals: ResMut<mir2_client_bevy::read_model::UiSurfaceSignals>,
    mut map: ResMut<mir2_client_bevy::map::MapModel>,
    mut entities: ResMut<mir2_client_bevy::entities::EntityModelSet>,
    mut inventory: ResMut<mir2_client_bevy::inventory::InventoryModel>,
    mut chat: ResMut<mir2_client_bevy::chat::ChatModel>,
    mut mail: ResMut<mir2_client_bevy::mail::MailModel>,
    mut shop: ResMut<mir2_client_bevy::shop::ShopModel>,
    mut game_shop: ResMut<mir2_client_bevy::game_shop::GameShopModel>,
    mut storage: ResMut<mir2_client_bevy::storage::StorageModel>,
    mut skills: ResMut<mir2_client_bevy::skill_model::SkillModel>,
    mut social: ResMut<mir2_client_bevy::social::SocialModel>,
    mut inventory_feedback: ResMut<InventoryOperationFeedback>,
    mut preservation: ResMut<SessionResetGameShopPreservation>,
) {
    if tracker.0 == reset.0 {
        return;
    }
    tracker.0 = reset.0;
    *ui = mir2_client_bevy::read_model::UiReadModel::default();
    surface_signals.npc_shop_open_requested = false;
    *map = mir2_client_bevy::map::MapModel::default();
    *entities = mir2_client_bevy::entities::EntityModelSet::default();
    *inventory = mir2_client_bevy::inventory::InventoryModel::default();
    *chat = mir2_client_bevy::chat::ChatModel::default();
    *mail = mir2_client_bevy::mail::MailModel::default();
    *shop = mir2_client_bevy::shop::ShopModel::default();
    let preserved_receipt = preservation.receipt_for(reset.0).cloned();
    if let Some(receipt) = preserved_receipt.as_ref() {
        let _ = game_shop.clear_session_preserving_exact_receipt(receipt);
    } else {
        game_shop.clear_session();
        let _ = preservation.clear_if_stale(reset.0);
    }
    *storage = mir2_client_bevy::storage::StorageModel::default();
    *skills = mir2_client_bevy::skill_model::SkillModel::default();
    social.clear_session();
    inventory_feedback.last = None;
}

fn decode_required_array(
    json: &str,
    field: &str,
    model_name: &str,
) -> Result<serde_json::Value, String> {
    let value = serde_json::from_str::<serde_json::Value>(json)
        .map_err(|error| format!("invalid native {model_name} JSON: {error}"))?;
    if !value.get(field).is_some_and(serde_json::Value::is_array) {
        return Err(format!(
            "invalid native {model_name} JSON: `{field}` must be an array"
        ));
    }
    Ok(value)
}

fn ingest_pending_mail_model(
    mut mail: ResMut<mir2_client_bevy::mail::MailModel>,
    mut revisions: ResMut<AuthoritativeModelRevisions>,
    mut pending: ResMut<PendingOperations>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| matches!(message, native_ingest::NativeInboundMessage::MailModel(_)),
        |message| {
            if let native_ingest::NativeInboundMessage::MailModel(json) = message {
                match decode_required_array(&json, "mails", "mail model").and_then(|value| {
                    serde_json::from_value::<mir2_client_bevy::mail::MailModel>(value)
                        .map_err(|error| error.to_string())
                }) {
                    Ok(model) => {
                        let old = mail.clone();
                        let selected_valid = model
                            .selected_id
                            .and_then(|id| model.mails.iter().find(|m| m.id == id).map(|_| id))
                            .or_else(|| {
                                mail.selected_id.and_then(|id| {
                                    model.mails.iter().find(|m| m.id == id).map(|_| id)
                                })
                            });
                        mail.mails = model.mails;
                        mail.selected_id = selected_valid;
                        reconcile_mail_refresh(&mut pending, &old, &mail);
                        mark_authoritative_refresh(&mut revisions, AuthoritativeModelDomain::Mail);
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native mail model");
                        eprintln!("[runtime] mail model decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_shop_model(
    mut shop: ResMut<mir2_client_bevy::shop::ShopModel>,
    mut revisions: ResMut<AuthoritativeModelRevisions>,
    mut pending: ResMut<PendingOperations>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| matches!(message, native_ingest::NativeInboundMessage::ShopModel(_)),
        |message| {
            if let native_ingest::NativeInboundMessage::ShopModel(json) = message {
                match decode_required_array(&json, "goods", "shop model").and_then(|value| {
                    serde_json::from_value::<mir2_client_bevy::shop::ShopModel>(value)
                        .map_err(|error| error.to_string())
                }) {
                    Ok(model) => {
                        let old = shop.clone();
                        let selected_valid = model
                            .selected_id
                            .and_then(|id| {
                                model.goods.iter().find(|g| g.unique_id == id).map(|_| id)
                            })
                            .or_else(|| {
                                shop.selected_id.and_then(|id| {
                                    model.goods.iter().find(|g| g.unique_id == id).map(|_| id)
                                })
                            });
                        shop.goods = model.goods;
                        shop.selected_id = selected_valid;
                        reconcile_shop_refresh(&mut pending, &old, &shop);
                        mark_authoritative_refresh(&mut revisions, AuthoritativeModelDomain::Shop);
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native shop model");
                        eprintln!("[runtime] shop model decode error: {error}");
                    }
                }
            }
        },
    );
}

/// Accumulate packet-first GameShopInfo entries. Unlike NPC ShopModel, a
/// GameShopInfo packet is one catalog row, so replacing the resource here
/// would silently lose most of Crystal's approximately 105 products.
fn ingest_pending_game_shop_info(
    mut game_shop: ResMut<mir2_client_bevy::game_shop::GameShopModel>,
    mut revisions: ResMut<AuthoritativeModelRevisions>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::GameShopInfo(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::GameShopInfo(json) = message {
                let value = match serde_json::from_str::<serde_json::Value>(&json) {
                    Ok(value) => value,
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native GameShopInfo");
                        eprintln!("[runtime] GameShopInfo decode error: {error}");
                        return;
                    }
                };
                let entry_value = value.get("entry").cloned().unwrap_or(value);
                match serde_json::from_value::<mir2_client_bevy::game_shop::GameShopEntry>(
                    entry_value,
                ) {
                    Ok(entry) if entry.game_shop_index >= 0 && entry.item_index >= 0 => {
                        game_shop.upsert(entry);
                        mark_authoritative_refresh(
                            &mut revisions,
                            AuthoritativeModelDomain::GameShop,
                        );
                    }
                    Ok(_) => {
                        publish_status("native-decode-error", "invalid native GameShopInfo index");
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native GameShopInfo");
                        eprintln!("[runtime] GameShopInfo entry decode error: {error}");
                    }
                }
            }
        },
    );
}

/// Apply stock-only patches without replacing the cash catalog or changing
/// selection/payment state. Unknown gIndex patches are retained by the model
/// until their GameShopInfo row arrives.
fn ingest_pending_game_shop_stock(
    mut game_shop: ResMut<mir2_client_bevy::game_shop::GameShopModel>,
    mut revisions: ResMut<AuthoritativeModelRevisions>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::GameShopStock(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::GameShopStock(json) = message {
                match serde_json::from_str::<mir2_client_bevy::game_shop::GameShopStockPatch>(&json)
                {
                    Ok(patch) if patch.game_shop_index >= 0 => {
                        game_shop.apply_stock_patch_value(patch);
                        mark_authoritative_refresh(
                            &mut revisions,
                            AuthoritativeModelDomain::GameShop,
                        );
                    }
                    Ok(_) => {
                        publish_status("native-decode-error", "invalid native GameShopStock index");
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native GameShopStock");
                        eprintln!("[runtime] GameShopStock decode error: {error}");
                    }
                }
            }
        },
    );
}

/// Apply only an exact native receipt. Catalog, stock, wallet, chat and mail
/// refreshes deliberately do not release this pending purchase.
fn ingest_pending_game_shop_receipt(
    mut game_shop: ResMut<mir2_client_bevy::game_shop::GameShopModel>,
    mut pending: ResMut<PendingOperations>,
    reset: Res<SessionResetRevision>,
    mut preservation: ResMut<SessionResetGameShopPreservation>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::GameShopReceipt(_)
            )
        },
        |message| {
            let native_ingest::NativeInboundMessage::GameShopReceipt(json) = message else {
                return;
            };
            let Ok(receipt) =
                serde_json::from_str::<mir2_client_bevy::game_shop::GameShopReceipt>(&json)
            else {
                publish_status("native-decode-error", "invalid native GameShopReceipt");
                return;
            };
            let request_id = receipt.request_id.clone();
            if game_shop.apply_receipt(receipt) {
                pending.release(&PendingOperationKey::GameShop(request_id.clone()));
                let _ = preservation.mark_consumed(reset.0, &request_id);
            }
        },
    );
}

/// Keep a preserving receipt alive for the complete `Update` schedule. Runtime
/// model ingest, overlay reset and UiState reconciliation all observe it before
/// this PostUpdate cleanup releases the retained payload.
fn finalize_consumed_game_shop_preservation(
    reset: Res<SessionResetRevision>,
    mut preservation: ResMut<SessionResetGameShopPreservation>,
) {
    let _ = preservation.clear_if_consumed(reset.0);
}

/// Apply one packet-authoritative NPC service transition and request the UI
/// surface. The goods list remains independent so Sell/Repair packets cannot
/// fabricate a Buy catalogue.
fn ingest_pending_npc_shop_service(
    mut shop: ResMut<mir2_client_bevy::shop::ShopModel>,
    mut surface_signals: ResMut<mir2_client_bevy::read_model::UiSurfaceSignals>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::NpcShopService(_)
            )
        },
        |message| {
            let native_ingest::NativeInboundMessage::NpcShopService(json) = message else {
                return;
            };
            let Ok(signal) =
                serde_json::from_str::<mir2_client_bevy::shop::NpcShopServiceSignal>(&json)
            else {
                publish_status("native-decode-error", "invalid native NPC service");
                return;
            };
            if shop.apply_service_signal(signal) {
                surface_signals.npc_shop_open_requested = true;
            } else {
                publish_status("native-decode-error", "invalid native NPC service");
            }
        },
    );
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StorageModelPatch {
    size: Option<u16>,
    has_password: Option<bool>,
    unlocked: Option<bool>,
    has_expanded: Option<bool>,
    expiry: Option<i64>,
    ack: Option<StorageOperationAck>,
}

impl StorageModelPatch {
    fn is_empty(&self) -> bool {
        self.size.is_none()
            && self.has_password.is_none()
            && self.unlocked.is_none()
            && self.has_expanded.is_none()
            && self.expiry.is_none()
            && self.ack.is_none()
    }

    fn apply_to(&self, storage: &mut mir2_client_bevy::storage::StorageModel) {
        if let Some(size) = self.size {
            storage.size = size;
        }
        if let Some(has_password) = self.has_password {
            storage.has_password = has_password;
        }
        if let Some(unlocked) = self.unlocked {
            storage.unlocked = unlocked;
        }
        if let Some(has_expanded) = self.has_expanded {
            storage.has_expanded = has_expanded;
        }
        if let Some(expiry) = self.expiry {
            storage.expiry = expiry;
        }
    }
}

fn ingest_pending_storage_patch(
    mut storage: ResMut<mir2_client_bevy::storage::StorageModel>,
    inventory: Res<mir2_client_bevy::inventory::InventoryModel>,
    mut revisions: ResMut<AuthoritativeModelRevisions>,
    mut pending: ResMut<PendingOperations>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::StoragePatch(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::StoragePatch(json) = message {
                match serde_json::from_str::<StorageModelPatch>(&json) {
                    Ok(patch) if !patch.is_empty() => {
                        let old = storage.clone();
                        if let Some(ack) = patch.ack.as_ref() {
                            apply_storage_operation_ack(&mut pending, ack);
                        }
                        patch.apply_to(&mut storage);
                        reconcile_storage_refresh(&mut pending, &inventory, &old, &storage);
                        mark_authoritative_refresh(
                            &mut revisions,
                            AuthoritativeModelDomain::Storage,
                        );
                    }
                    Ok(_) => {
                        publish_status("native-decode-error", "empty native storage patch");
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native storage patch");
                        eprintln!("[runtime] storage patch decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_storage_items(
    mut storage: ResMut<mir2_client_bevy::storage::StorageModel>,
    inventory: Res<mir2_client_bevy::inventory::InventoryModel>,
    mut revisions: ResMut<AuthoritativeModelRevisions>,
    mut pending: ResMut<PendingOperations>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::StorageItems(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::StorageItems(json) = message {
                match decode_required_array(&json, "items", "storage items").and_then(|value| {
                    serde_json::from_value::<Vec<mir2_client_bevy::inventory::ItemModel>>(
                        value.get("items").cloned().unwrap_or_default(),
                    )
                    .map_err(|error| error.to_string())
                }) {
                    Ok(items) => {
                        let old = storage.clone();
                        storage.items = items;
                        let selected_storage = storage.selected_storage_slot.filter(|slot| {
                            storage
                                .items
                                .iter()
                                .any(|item| item.container == 4 && item.slot == *slot)
                        });
                        storage.selected_storage_slot = selected_storage;
                        reconcile_storage_refresh(&mut pending, &inventory, &old, &storage);
                        mark_authoritative_refresh(
                            &mut revisions,
                            AuthoritativeModelDomain::Storage,
                        );
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native storage items");
                        eprintln!("[runtime] storage items decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_storage_model(
    mut storage: ResMut<mir2_client_bevy::storage::StorageModel>,
    inventory: Res<mir2_client_bevy::inventory::InventoryModel>,
    mut revisions: ResMut<AuthoritativeModelRevisions>,
    mut pending: ResMut<PendingOperations>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::StorageModel(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::StorageModel(json) = message {
                match decode_required_array(&json, "items", "storage model").and_then(|value| {
                    serde_json::from_value::<mir2_client_bevy::storage::StorageModel>(value)
                        .map_err(|error| error.to_string())
                }) {
                    Ok(model) => {
                        let old = storage.clone();
                        let drafts = (
                            storage.password_draft.clone(),
                            storage.new_password_draft.clone(),
                            storage.confirm_password_draft.clone(),
                        );
                        let selected_bag = storage.selected_bag_slot;
                        let selected_storage = storage.selected_storage_slot;
                        let selected_storage = model
                            .selected_storage_slot
                            .filter(|slot| {
                                model
                                    .items
                                    .iter()
                                    .any(|item| item.container == 4 && item.slot == *slot)
                            })
                            .or_else(|| {
                                selected_storage.filter(|slot| {
                                    model
                                        .items
                                        .iter()
                                        .any(|item| item.container == 4 && item.slot == *slot)
                                })
                            });
                        *storage = model;
                        // Restore UI-only drafts/selections
                        if storage.password_draft.is_empty() {
                            storage.password_draft = drafts.0;
                        }
                        if storage.new_password_draft.is_empty() {
                            storage.new_password_draft = drafts.1;
                        }
                        if storage.confirm_password_draft.is_empty() {
                            storage.confirm_password_draft = drafts.2;
                        }
                        if storage.selected_bag_slot.is_none() {
                            storage.selected_bag_slot = selected_bag;
                        }
                        storage.selected_storage_slot = selected_storage;
                        reconcile_storage_refresh(&mut pending, &inventory, &old, &storage);
                        mark_authoritative_refresh(
                            &mut revisions,
                            AuthoritativeModelDomain::Storage,
                        );
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native storage model");
                        eprintln!("[runtime] storage model decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_skill_model(
    mut skills: ResMut<mir2_client_bevy::skill_model::SkillModel>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| matches!(message, native_ingest::NativeInboundMessage::SkillModel(_)),
        |message| {
            if let native_ingest::NativeInboundMessage::SkillModel(json) = message {
                match serde_json::from_str::<mir2_client_bevy::skill_model::SkillModel>(&json) {
                    Ok(model) => {
                        *skills = model;
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native skill model");
                        eprintln!("[runtime] skill model decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_social_model(
    mut social: ResMut<mir2_client_bevy::social::SocialModel>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| matches!(message, native_ingest::NativeInboundMessage::SocialModel(_)),
        |message| {
            if let native_ingest::NativeInboundMessage::SocialModel(json) = message {
                match serde_json::from_str::<mir2_client_bevy::social::SocialModel>(&json) {
                    Ok(model) => social.apply_authoritative(model),
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native social model");
                        eprintln!("[runtime] social model decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_chat_line(
    mut chat: ResMut<mir2_client_bevy::chat::ChatModel>,
    native: Res<native_ingest::NativeInbound>,
) {
    native.drain_matching(
        |message| matches!(message, native_ingest::NativeInboundMessage::ChatLine(_)),
        |message| {
            if let native_ingest::NativeInboundMessage::ChatLine(json) = message {
                match serde_json::from_str::<mir2_client_bevy::chat::ChatLine>(&json) {
                    Ok(line) => {
                        chat.push(line);
                    }
                    Err(error) => {
                        publish_status("native-decode-error", "invalid native chat line");
                        eprintln!("[runtime] chat line decode error: {error}");
                    }
                }
            }
        },
    );
}

fn ingest_pending_map_render_state(
    mut state: ResMut<RuntimeMapRenderState>,
    mut camera_offset: ResMut<RuntimeMapCameraOffset>,
    native: Res<native_ingest::NativeInbound>,
) {
    PENDING_MAP_RENDER_STATE.with(|pending| {
        if let Some(snapshot) = pending.borrow_mut().take() {
            state.snapshot = Some(snapshot);
        }
    });
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::MapRenderState(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::MapRenderState(json) = message {
                if let Ok(snapshot) = serde_json::from_str::<MapRenderState>(&json) {
                    state.snapshot = Some(snapshot);
                } else {
                    publish_status("native-decode-error", "invalid native map render state");
                }
            }
        },
    );
    // The camera offset is a cheap per-frame scalar pair; pull the latest value
    // each frame so `sync_map_render` folds the current value into respawned tiles.
    let (x, y) = PENDING_MAP_CAMERA_OFFSET.with(|cell| cell.get());
    camera_offset.x = x;
    camera_offset.y = y;
}

fn ingest_pending_effect_render_state(
    mut state: ResMut<RuntimeEffectRenderState>,
    native: Res<native_ingest::NativeInbound>,
) {
    PENDING_EFFECT_RENDER_STATE.with(|pending| {
        if let Some(snapshot) = pending.borrow_mut().take() {
            state.snapshot = Some(snapshot);
        }
    });
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::EffectRenderState(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::EffectRenderState(json) = message {
                if let Ok(snapshot) = serde_json::from_str::<EffectRenderState>(&json) {
                    state.snapshot = Some(snapshot);
                } else {
                    publish_status("native-decode-error", "invalid native effect render state");
                }
            }
        },
    );
}

fn ingest_pending_lighting_render_state(
    mut state: ResMut<RuntimeLightingRenderState>,
    native: Res<native_ingest::NativeInbound>,
) {
    PENDING_LIGHTING_RENDER_STATE.with(|pending| {
        if let Some(snapshot) = pending.borrow_mut().take() {
            state.snapshot = Some(snapshot);
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    native.drain_matching(
        |message| {
            matches!(
                message,
                native_ingest::NativeInboundMessage::LightingRenderState(_)
            )
        },
        |message| {
            if let native_ingest::NativeInboundMessage::LightingRenderState(json) = message {
                if let Ok(snapshot) = serde_json::from_str::<lighting::LightingRenderState>(&json) {
                    state.snapshot = Some(snapshot);
                } else {
                    publish_status(
                        "native-decode-error",
                        "invalid native lighting render state",
                    );
                }
            }
        },
    );
    #[cfg(target_arch = "wasm32")]
    let _ = native;
}

/// Retained Crystal light buffer. A dedicated camera clears an offscreen image
/// to Crystal's darkness colour and adds every `Lighting/N.png` source on an
/// isolated render layer. A full-stage main-pass mesh then multiplies the
/// completed light-buffer RGB with the world. This preserves Crystal's
/// `scene * (darkness + lights)` equation instead of the visibly-wrong
/// `scene * darkness + lights` approximation. Day has no light pass.
fn sync_lighting_render(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    state: Res<RuntimeLightingRenderState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut additive_materials: ResMut<Assets<additive_material::CrystalAdditiveMaterial>>,
    mut additive_cache: ResMut<additive_material::CrystalAdditiveMaterialCache>,
    mut multiply_materials: ResMut<Assets<lighting::CrystalMultiplyMaterial>>,
    mut registry: ResMut<SceneRegistry>,
    mut additive_material_query: Query<
        &mut MeshMaterial2d<additive_material::CrystalAdditiveMaterial>,
    >,
    mut transform_query: Query<&mut Transform>,
    mut camera_query: Query<&mut Camera, With<MirLightingBufferCamera>>,
) {
    macro_rules! clear_lighting {
        () => {
            clear_lighting_render_layers(
                &mut commands,
                &mut registry,
                &mut additive_cache,
                &mut additive_materials,
                &mut multiply_materials,
                &mut images,
            )
        };
    }
    let Some(snapshot) = state.snapshot.as_ref() else {
        clear_lighting!();
        return;
    };
    let Some(setting) = lighting::effective_light_setting(snapshot) else {
        clear_lighting!();
        return;
    };
    let Some(darkness) = lighting::darkness_color(setting, snapshot.map_dark_light) else {
        clear_lighting!();
        return;
    };
    let Some(stage_size) = snapshot
        .enabled
        .then(|| lighting::validated_stage_size(snapshot))
        .flatten()
    else {
        clear_lighting!();
        return;
    };

    if registry
        .lighting_darkness
        .as_ref()
        .is_some_and(|handle| handle.stage_size != stage_size)
    {
        clear_lighting!();
    }

    // Preload all ten native light textures once. Crystal owns ten range slots;
    // retaining these handles prevents source churn while the player walks.
    if registry.lighting_images.len() != lighting::LIGHT_TEXTURE_COUNT {
        registry.lighting_images = (0..lighting::LIGHT_TEXTURE_COUNT)
            .map(|range| asset_server.load(lighting::light_texture_path(range)))
            .collect();
    }

    let dark_position = Vec3::new(0.0, 0.0, 500.0);
    if let Some(handle) = registry.lighting_darkness.as_mut() {
        if let Ok(mut camera) = camera_query.get_mut(handle.buffer_camera) {
            camera.clear_color = ClearColorConfig::Custom(darkness);
        }
        if let Ok(mut transform) = transform_query.get_mut(handle.composite_entity) {
            transform.translation = dark_position;
            transform.scale = Vec3::new(snapshot.stage_width, snapshot.stage_height, 1.0);
        }
    } else {
        let buffer_image = images.add(Image::new_target_texture(
            stage_size.x,
            stage_size.y,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        ));
        let buffer_camera = commands
            .spawn((
                Camera2d,
                Camera {
                    order: -1,
                    clear_color: ClearColorConfig::Custom(darkness),
                    ..default()
                },
                RenderTarget::Image(buffer_image.clone().into()),
                Msaa::Off,
                RenderLayers::layer(lighting::LIGHT_BUFFER_RENDER_LAYER),
                MirLightingBufferCamera,
            ))
            .id();
        let mesh = additive_cache.unit_quad(&mut meshes);
        let material = multiply_materials.add(lighting::CrystalMultiplyMaterial {
            light_buffer: buffer_image.clone(),
        });
        let composite_entity = commands
            .spawn((
                Mesh2d(mesh),
                MeshMaterial2d(material.clone()),
                Transform::from_translation(dark_position).with_scale(Vec3::new(
                    snapshot.stage_width,
                    snapshot.stage_height,
                    1.0,
                )),
                MirLightingComposite,
            ))
            .id();
        registry.lighting_darkness = Some(LightingDarknessHandle {
            composite_entity,
            buffer_camera,
            buffer_image,
            stage_size,
            material,
        });
    }

    let mut alive = HashSet::new();
    for light in lighting::resolved_lights(snapshot) {
        alive.insert(light.key.clone());
        let image = registry.lighting_images[light.range].clone();
        let position = lighting_layer_position(snapshot, &light);
        if let Some(handle) = registry.lighting_layers.get_mut(&light.key) {
            let image_key = lighting::light_texture_path(light.range);
            let cache_key = lighting_material_cache_key(&light.key);
            let material =
                additive_cache.material(&cache_key, image, light.opacity, &mut additive_materials);
            if handle.image_key != image_key {
                if let Ok(mut binding) = additive_material_query.get_mut(handle.entity) {
                    *binding = MeshMaterial2d(material);
                }
                handle.image_key = image_key;
            }
            if let Ok(mut transform) = transform_query.get_mut(handle.entity) {
                transform.translation = position;
                transform.scale = Vec3::new(light.width, light.height, 1.0);
            }
        } else {
            let mesh = additive_cache.unit_quad(&mut meshes);
            let cache_key = lighting_material_cache_key(&light.key);
            let material =
                additive_cache.material(&cache_key, image, light.opacity, &mut additive_materials);
            let entity = commands
                .spawn((
                    Mesh2d(mesh),
                    MeshMaterial2d(material),
                    Transform::from_translation(position).with_scale(Vec3::new(
                        light.width,
                        light.height,
                        1.0,
                    )),
                    RenderLayers::layer(lighting::LIGHT_BUFFER_RENDER_LAYER),
                    MirLightingBufferLayer,
                ))
                .id();
            registry.lighting_layers.insert(
                light.key.clone(),
                EffectRenderLayerHandle {
                    entity,
                    image_key: lighting::light_texture_path(light.range),
                    additive: true,
                },
            );
        }
    }

    let stale: Vec<String> = registry
        .lighting_layers
        .keys()
        .filter(|key| !alive.contains(*key))
        .cloned()
        .collect();
    for key in stale {
        if let Some(handle) = registry.lighting_layers.remove(&key) {
            commands.entity(handle.entity).despawn();
            additive_cache.evict(&lighting_material_cache_key(&key), &mut additive_materials);
        }
    }
}

fn lighting_material_cache_key(source_key: &str) -> String {
    format!("native-lighting:{source_key}")
}

fn lighting_layer_position(
    snapshot: &lighting::LightingRenderState,
    light: &lighting::ResolvedLight,
) -> Vec3 {
    Vec3::new(
        light.left + light.width * 0.5 - snapshot.stage_width * 0.5,
        snapshot.stage_height * 0.5 - (light.top + light.height * 0.5),
        0.0,
    )
}

fn clear_lighting_render_layers(
    commands: &mut Commands,
    registry: &mut SceneRegistry,
    additive_cache: &mut additive_material::CrystalAdditiveMaterialCache,
    additive_materials: &mut Assets<additive_material::CrystalAdditiveMaterial>,
    multiply_materials: &mut Assets<lighting::CrystalMultiplyMaterial>,
    images: &mut Assets<Image>,
) {
    for (key, handle) in registry.lighting_layers.drain() {
        commands.entity(handle.entity).despawn();
        additive_cache.evict(&lighting_material_cache_key(&key), additive_materials);
    }
    if let Some(handle) = registry.lighting_darkness.take() {
        commands.entity(handle.composite_entity).despawn();
        commands.entity(handle.buffer_camera).despawn();
        multiply_materials.remove(handle.material.id());
        images.remove(handle.buffer_image.id());
    }
    registry.lighting_images.clear();
}

/// RETAIN-IN-PLACE renderer for authoritative scene-effect sprites (map/
/// ground/cast/projectile/impact). Mirrors `sync_entity_render_layers` and the
/// map standalone-tile path: effects arrive as screen-stage rectangles with a
/// stable key, and this system spawns/updates/despawns one sprite per key so a
/// transient effect updates its frame by rewriting the image/transform instead
/// of despawn + respawn every tick. Additive effects use the shared Crystal
/// SourceAlpha + One material so bright frames add to the scene exactly like
/// Crystal DrawBlend. Mask frames render as a second additive layer at z+1;
/// shadow frames render below the primary at primary + shadow offset. When no
/// native effect snapshot is present the effect layer is a no-op, so WASM/Web
/// behavior is byte-identical.
fn sync_effect_render(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    state: Res<RuntimeEffectRenderState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut shadow_materials: ResMut<Assets<ColorMaterial>>,
    mut additive_materials: ResMut<Assets<additive_material::CrystalAdditiveMaterial>>,
    mut additive_cache: ResMut<additive_material::CrystalAdditiveMaterialCache>,
    mut registry: ResMut<SceneRegistry>,
    mut sprite_query: Query<&mut Sprite>,
    mut additive_material_query: Query<
        &mut MeshMaterial2d<additive_material::CrystalAdditiveMaterial>,
    >,
    mut transform_query: Query<&mut Transform>,
) {
    let Some(snapshot) = &state.snapshot else {
        clear_effect_render_layers(
            &mut commands,
            &mut registry,
            &mut additive_cache,
            &mut additive_materials,
            &mut meshes,
            &mut shadow_materials,
        );
        return;
    };
    if !snapshot.enabled {
        clear_effect_render_layers(
            &mut commands,
            &mut registry,
            &mut additive_cache,
            &mut additive_materials,
            &mut meshes,
            &mut shadow_materials,
        );
        return;
    }

    // Preload any standalone frame URLs referenced this snapshot (including masks).
    let active_image_keys = effect_render_active_image_keys(snapshot);
    for image_key in &active_image_keys {
        if !registry.effect_render_images.contains_key(image_key) {
            registry
                .effect_render_images
                .insert(image_key.clone(), asset_server.load(image_key.clone()));
        }
    }

    let mut alive = HashSet::new();
    let mut stale_images: HashSet<String> = registry.effect_render_images.keys().cloned().collect();
    for effect in &snapshot.effects {
        alive.insert(effect.key.clone());
        let opacity = effect.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
        let image_key = effect.image_url.as_ref().map(|url| browser_asset_path(url));
        if let Some(image_key) = &image_key {
            stale_images.remove(image_key);
        }
        let mask_image_key_opt = effect
            .mask_image_url
            .as_ref()
            .map(|url| browser_asset_path(url));
        if let Some(mask_key) = &mask_image_key_opt {
            stale_images.remove(mask_key);
        }
        let position = effect_render_layer_position(snapshot, effect);
        let bound_image = image_key
            .as_ref()
            .and_then(|key| registry.effect_render_images.get(key).cloned());
        let bound_mask_image = mask_image_key_opt
            .as_ref()
            .and_then(|key| registry.effect_render_images.get(key).cloned());

        // Primary: spawn/update in place, rebuild when additive toggles.
        if let Some(existing_additive) = registry.effect_render.get(&effect.key).map(|h| h.additive)
        {
            if existing_additive != effect.additive {
                if let Some(old) = registry.effect_render.remove(&effect.key) {
                    commands.entity(old.entity).despawn();
                    if old.additive {
                        additive_cache.evict(&effect.key, &mut additive_materials);
                    }
                }
                // Rebuild below.
            } else if let Some(handle) = registry.effect_render.get_mut(&effect.key) {
                let image_changed = handle.image_key != image_key.as_deref().unwrap_or_default();
                if let Some(image) = bound_image.clone() {
                    if effect.additive {
                        // Refresh on every snapshot because opacity can change
                        // without a frame/image change.
                        let material = additive_cache.material(
                            &effect.key,
                            image,
                            opacity,
                            &mut additive_materials,
                        );
                        if let Ok(mut binding) = additive_material_query.get_mut(handle.entity) {
                            *binding = MeshMaterial2d(material);
                        }
                    } else if let Ok(mut sprite) = sprite_query.get_mut(handle.entity) {
                        if image_changed {
                            sprite.image = image;
                        }
                        sprite.color = Color::srgba(1.0, 1.0, 1.0, opacity);
                    }
                }
                if image_changed {
                    handle.image_key = image_key.clone().unwrap_or_default();
                }
                if let Ok(mut transform) = transform_query.get_mut(handle.entity) {
                    transform.translation = position;
                    if effect.additive {
                        transform.scale =
                            Vec3::new(effect.width.max(1.0), effect.height.max(1.0), 1.0);
                    }
                }
            }
        }
        if !registry.effect_render.contains_key(&effect.key) {
            if let Some(image_key) = &image_key {
                if let Some(image) = registry.effect_render_images.get(image_key).cloned() {
                    let entity = if effect.additive {
                        let mesh = additive_cache.unit_quad(&mut meshes);
                        let material = additive_cache.material(
                            &effect.key,
                            image,
                            opacity,
                            &mut additive_materials,
                        );
                        commands
                            .spawn((
                                MirEffectRenderLayer,
                                Mesh2d(mesh),
                                MeshMaterial2d(material),
                                Transform::from_translation(position).with_scale(Vec3::new(
                                    effect.width.max(1.0),
                                    effect.height.max(1.0),
                                    1.0,
                                )),
                            ))
                            .id()
                    } else {
                        commands
                            .spawn((
                                MirEffectRenderLayer,
                                Sprite {
                                    image,
                                    custom_size: Some(Vec2::new(
                                        effect.width.max(1.0),
                                        effect.height.max(1.0),
                                    )),
                                    color: Color::srgba(1.0, 1.0, 1.0, opacity),
                                    ..default()
                                },
                                Transform::from_translation(position),
                            ))
                            .id()
                    };
                    registry.effect_render.insert(
                        effect.key.clone(),
                        EffectRenderLayerHandle {
                            entity,
                            image_key: image_key.clone(),
                            additive: effect.additive,
                        },
                    );
                }
            }
        }

        // Mask: second additive layer at z+1, strictly synced. The producer sends
        // both the primary frame's local offset (frame_x/frame_y) and the mask's
        // local offset (mask_x/mask_y) in the SAME origin; the mask is placed at
        // (mask_x-frame_x, mask_y-frame_y) relative to the primary anchor, so a
        // different frame/mask origin is honored exactly. Fallback centers the
        // mask on the primary frame when no geometry is supplied.
        if let Some(mask_image_key) = &mask_image_key_opt {
            let mask_key = format!("{}:mask", effect.key);
            let mask_w = effect.mask_width.unwrap_or(effect.width);
            let mask_h = effect.mask_height.unwrap_or(effect.height);
            let mask_dx = match (effect.mask_x, effect.frame_x) {
                (Some(mx), Some(fx)) => mx - fx,
                _ => (effect.width - mask_w) * 0.5,
            };
            let mask_dy = match (effect.mask_y, effect.frame_y) {
                (Some(my), Some(fy)) => my - fy,
                _ => (effect.height - mask_h) * 0.5,
            };
            let mask_left = effect.left + mask_dx;
            let mask_top = effect.top + mask_dy;
            let mask_center_x = mask_left + mask_w * 0.5;
            let mask_center_y = mask_top + mask_h * 0.5;
            let mask_pos = Vec3::new(
                mask_center_x - snapshot.stage_width * 0.5,
                snapshot.stage_height * 0.5 - mask_center_y,
                effect.z / 100_000.0 + 0.0001,
            );
            if let Some(handle) = registry.effect_render_masks.get_mut(&mask_key) {
                if let Some(image) = bound_mask_image.clone() {
                    // Keep the mask at the same DrawBlend rate as the primary,
                    // including an opacity-only retained update.
                    let material =
                        additive_cache.material(&mask_key, image, opacity, &mut additive_materials);
                    if let Ok(mut binding) = additive_material_query.get_mut(handle.entity) {
                        *binding = MeshMaterial2d(material);
                    }
                }
                if handle.image_key != *mask_image_key {
                    handle.image_key = mask_image_key.clone();
                }
                if let Ok(mut transform) = transform_query.get_mut(handle.entity) {
                    transform.translation = mask_pos;
                    transform.scale = Vec3::new(mask_w.max(1.0), mask_h.max(1.0), 1.0);
                }
            } else if let Some(image) = bound_mask_image.clone() {
                let mesh = additive_cache.unit_quad(&mut meshes);
                let material =
                    additive_cache.material(&mask_key, image, opacity, &mut additive_materials);
                let entity = commands
                    .spawn((
                        MirEffectRenderLayer,
                        Mesh2d(mesh),
                        MeshMaterial2d(material),
                        Transform::from_translation(mask_pos).with_scale(Vec3::new(
                            mask_w.max(1.0),
                            mask_h.max(1.0),
                            1.0,
                        )),
                    ))
                    .id();
                registry.effect_render_masks.insert(
                    mask_key,
                    EffectRenderLayerHandle {
                        entity,
                        image_key: mask_image_key.clone(),
                        additive: true,
                    },
                );
            }
        } else {
            let mask_key = format!("{}:mask", effect.key);
            if let Some(handle) = registry.effect_render_masks.remove(&mask_key) {
                commands.entity(handle.entity).despawn();
                additive_cache.evict(&mask_key, &mut additive_materials);
            }
        }

        // Shadow metadata is a complete pair: Some(0) is still a valid axis,
        // while a missing axis means the producer has no legal shadow for this
        // frame. The shadow is an independent procedural ellipse, so its offset
        // can never move or resize the primary effect frame.
        let shadow_key = format!("{}:shadow", effect.key);
        match (effect.shadow_x, effect.shadow_y) {
            (Some(shadow_x), Some(shadow_y)) => {
                let shadow_size = Vec2::new(
                    (effect.width * 0.70).max(8.0),
                    (effect.height * 0.24).max(4.0),
                );
                let primary_position = effect_render_layer_position(snapshot, effect);
                let shadow_position = Vec3::new(
                    primary_position.x + shadow_x,
                    primary_position.y - shadow_y,
                    primary_position.z - 0.0005,
                );

                if let Some(handle) = registry.effect_render_shadows.get(&shadow_key) {
                    if let Ok(mut transform) = transform_query.get_mut(handle.entity) {
                        transform.translation = shadow_position;
                        transform.scale = Vec3::new(shadow_size.x, shadow_size.y, 1.0);
                    }
                } else {
                    let mesh = meshes.add(Ellipse::new(0.5, 0.5));
                    let material = shadow_materials.add(ColorMaterial::from_color(Color::srgba(
                        0.02, 0.01, 0.01, 0.28,
                    )));
                    let entity = commands
                        .spawn((
                            MirEffectRenderLayer,
                            Mesh2d(mesh.clone()),
                            MeshMaterial2d(material.clone()),
                            Transform::from_translation(shadow_position).with_scale(Vec3::new(
                                shadow_size.x,
                                shadow_size.y,
                                1.0,
                            )),
                        ))
                        .id();
                    registry.effect_render_shadows.insert(
                        shadow_key,
                        EffectShadowLayerHandle {
                            entity,
                            mesh,
                            material,
                        },
                    );
                }
            }
            _ => remove_effect_shadow_layer(
                &shadow_key,
                &mut commands,
                &mut registry,
                &mut meshes,
                &mut shadow_materials,
            ),
        }
    }

    // Despawn retained effect sprites no longer in the snapshot (plus mask/shadow).
    let stale: Vec<String> = registry
        .effect_render
        .keys()
        .filter(|key| !alive.contains(*key))
        .cloned()
        .collect();
    for key in stale {
        if let Some(handle) = registry.effect_render.remove(&key) {
            commands.entity(handle.entity).despawn();
            if handle.additive {
                additive_cache.evict(&key, &mut additive_materials);
            }
        }
        let mask_key = format!("{}:mask", key);
        if let Some(handle) = registry.effect_render_masks.remove(&mask_key) {
            commands.entity(handle.entity).despawn();
            additive_cache.evict(&mask_key, &mut additive_materials);
        }
        let shadow_key = format!("{}:shadow", key);
        remove_effect_shadow_layer(
            &shadow_key,
            &mut commands,
            &mut registry,
            &mut meshes,
            &mut shadow_materials,
        );
    }

    // Evict standalone images no longer referenced by any effect.
    registry
        .effect_render_images
        .retain(|key, _| !stale_images.contains(key));
}

/// Screen-stage to centred world coords, Y flipped — identical to
/// `entity_render_layer_position` so effects share the entities/map z-space.
fn effect_render_layer_position(snapshot: &EffectRenderState, effect: &EffectRenderEntry) -> Vec3 {
    let center_x = effect.left + effect.width * 0.5;
    let center_y = effect.top + effect.height * 0.5;
    Vec3::new(
        center_x - snapshot.stage_width * 0.5,
        snapshot.stage_height * 0.5 - center_y,
        effect.z / 100_000.0,
    )
}

fn clear_effect_render_layers(
    commands: &mut Commands,
    registry: &mut SceneRegistry,
    additive_cache: &mut additive_material::CrystalAdditiveMaterialCache,
    additive_materials: &mut Assets<additive_material::CrystalAdditiveMaterial>,
    meshes: &mut Assets<Mesh>,
    shadow_materials: &mut Assets<ColorMaterial>,
) {
    for (key, handle) in registry.effect_render.drain() {
        commands.entity(handle.entity).despawn();
        if handle.additive {
            additive_cache.evict(&key, additive_materials);
        }
    }
    for (key, handle) in registry.effect_render_masks.drain() {
        commands.entity(handle.entity).despawn();
        additive_cache.evict(&key, additive_materials);
    }
    for (_, handle) in registry.effect_render_shadows.drain() {
        commands.entity(handle.entity).despawn();
        meshes.remove(handle.mesh.id());
        shadow_materials.remove(handle.material.id());
    }
    registry.effect_render_images.clear();
}

fn remove_effect_shadow_layer(
    key: &str,
    commands: &mut Commands,
    registry: &mut SceneRegistry,
    meshes: &mut Assets<Mesh>,
    shadow_materials: &mut Assets<ColorMaterial>,
) {
    if let Some(handle) = registry.effect_render_shadows.remove(key) {
        commands.entity(handle.entity).despawn();
        meshes.remove(handle.mesh.id());
        shadow_materials.remove(handle.material.id());
    }
}

/// Despawn every retained scene entity and clear every renderer-side scene
/// registry. This is intentionally narrower than the session read-model reset:
/// personal UI/read-model resources are not reachable from this function.
fn clear_scene_registry(
    commands: &mut Commands,
    registry: &mut SceneRegistry,
    additive_cache: &mut additive_material::CrystalAdditiveMaterialCache,
    additive_materials: &mut Assets<additive_material::CrystalAdditiveMaterial>,
) {
    for (_, handles) in registry.entities.drain() {
        commands.entity(handles.root).despawn();
    }
    for (key, handle) in registry.entity_render_layers.drain() {
        commands.entity(handle.entity).despawn();
        if handle.additive {
            additive_cache.evict(&entity_additive_material_key(&key), additive_materials);
        }
    }
    registry.entity_render_atlases.clear();

    // Keep the shadow handles until cleanup_reset_effect_shadows can remove
    // their mesh/material assets; this reset system intentionally has a
    // bounded parameter list because it is part of a chained system tuple.
    clear_effect_render_layers_for_scene_reset(
        commands,
        registry,
        additive_cache,
        additive_materials,
    );

    for entity in registry.map.spawned.drain(..) {
        commands.entity(entity).despawn();
    }
    for (_, handle) in registry.map_render.tiles.drain() {
        commands.entity(handle.entity).despawn();
    }
    for (_, handles) in registry.mine_nodes.drain() {
        commands.entity(handles.root).despawn();
    }

    registry.map = MapSceneCache::default();
    registry.map_render = MapRenderSceneCache::default();
}

fn clear_effect_render_layers_for_scene_reset(
    commands: &mut Commands,
    registry: &mut SceneRegistry,
    additive_cache: &mut additive_material::CrystalAdditiveMaterialCache,
    additive_materials: &mut Assets<additive_material::CrystalAdditiveMaterial>,
) {
    for (key, handle) in registry.effect_render.drain() {
        commands.entity(handle.entity).despawn();
        if handle.additive {
            additive_cache.evict(&key, additive_materials);
        }
    }
    for (key, handle) in registry.effect_render_masks.drain() {
        commands.entity(handle.entity).despawn();
        additive_cache.evict(&key, additive_materials);
    }
    registry.effect_render_images.clear();
}

fn ingest_pending_map_render_images(
    mut atlas_resource: ResMut<RuntimeMapRenderAtlases>,
    mut images: ResMut<Assets<Image>>,
) {
    PENDING_MAP_RENDER_IMAGE_OPS.with(|pending| {
        for operation in pending.borrow_mut().drain(..) {
            match operation {
                PendingMapRenderImageOp::Upload(atlas) => {
                    let image = Image::new(
                        Extent3d {
                            width: atlas.width,
                            height: atlas.height,
                            depth_or_array_layers: 1,
                        },
                        TextureDimension::D2,
                        atlas.pixels,
                        TextureFormat::Rgba8UnormSrgb,
                        RenderAssetUsages::default(),
                    );
                    let handle = images.add(image);
                    atlas_resource.url_image_keys.remove(&atlas.key);
                    atlas_resource.images.insert(atlas.key, handle);
                }
                PendingMapRenderImageOp::Evict(key) => {
                    atlas_resource.images.remove(&key);
                    atlas_resource.url_image_keys.remove(&key);
                }
            }
            // State JSON and image bytes travel over separate JS/WASM calls. Track
            // image mutations so an unchanged state is rebound after upload/evict.
            atlas_resource.revision = atlas_resource.revision.wrapping_add(1);
        }
    });
}

/// Stage 2 (unified y-sort) z scale. Map tiles and entities derive z from the
/// SAME `viewportDepthForCell`; the entity producer pre-multiplies by
/// MAP_TILE_ENTITY_DEPTH_GAIN (the `depth*10+order` in buildBevyEntityRenderState)
/// before the runtime's `/ MAP_TILE_Z_DENOM` (entity_render_layer_position). The
/// map producer feeds RAW depth, so apply the same ×10 here → map world-z lands on
/// the IDENTICAL band as entities (floor behind ≈0.2, tall fronts above actors ≈3.9,
/// objects interleave with actors by cell row) — Crystal's single y-sorted band.
const MAP_TILE_ENTITY_DEPTH_GAIN: f32 = 10.0;
const MAP_TILE_Z_DENOM: f32 = 100_000.0;

/// Bevy-native map-tile renderer. Builds/refreshes atlas-page layouts, then skips
/// an already-applied producer/image revision. The producer folds the camera
/// offset into each tile's `left`/`top`, like the entity render
/// layers), and on change the tiles are RETAINED and updated in place — keyed by
/// `tile.key` (stable across motion for static tiles) the system updates each
/// surviving tile's Transform, rebinds Sprite only when images change, spawns
/// tiles that entered the viewport, and
/// despawns those that left — exactly like `sync_entity_render_layers`, instead of
/// despawning + respawning all ~470 sprites every motion frame. Tiles are spawned
/// TOP-LEVEL (not children of a shared root): a child sprite's
/// `InheritedVisibility`/`GlobalTransform` depend on hierarchy propagation, and an
/// earlier root+children build rendered nothing (black floor) even with the root
/// `Visibility::Visible`; the entity layers never hit this because each sprite is
/// its own top-level entity. When map render is disabled/None this clears all
/// spawned tiles and returns.
fn sync_map_render(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    map_state: Res<RuntimeMapRenderState>,
    entity_render_state: Res<RuntimeEntityRenderState>,
    map_camera_offset: Res<RuntimeMapCameraOffset>,
    mut atlas_assets: ResMut<RuntimeMapRenderAtlases>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut additive_materials: ResMut<Assets<additive_material::CrystalAdditiveMaterial>>,
    mut additive_cache: ResMut<additive_material::CrystalAdditiveMaterialCache>,
    mut presentation_poses: ResMut<presentation_pose::PresentationPoseBuffer>,
    mut registry: ResMut<SceneRegistry>,
    mut sprite_query: Query<&mut Sprite>,
    mut additive_material_query: Query<
        &mut MeshMaterial2d<additive_material::CrystalAdditiveMaterial>,
    >,
    mut transform_query: Query<&mut Transform>,
    mut native_trace_state: Local<Option<String>>,
) {
    let active = map_state
        .snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.enabled);
    if !active {
        for (_, handle) in registry.map_render.tiles.drain() {
            commands.entity(handle.entity).despawn();
        }
        registry.map_render.applied = None;
        if !atlas_assets.images.is_empty() || !atlas_assets.layouts.is_empty() {
            atlas_assets.images.clear();
            atlas_assets.url_image_keys.clear();
            atlas_assets.layouts.clear();
            atlas_assets.layout_rects.clear();
            atlas_assets.layout_sizes.clear();
            atlas_assets.revision = atlas_assets.revision.wrapping_add(1);
        }
        presentation_poses.set_applied_map_provenance(None, None);
        return;
    }

    let snapshot = map_state.snapshot.as_ref().unwrap();

    // Map and entity producers can reach Bevy on adjacent React renders. Keep
    // the last complete map frame until the entity snapshot names the same
    // grid center; otherwise the map jumps one cell ahead while actors still
    // use the old center, forcing the presentation bridge to fall back.
    let map_center = snapshot
        .center_x
        .zip(snapshot.center_y)
        .map(|(x, y)| presentation_pose::PresentationGridCenter { x, y });
    let entity_center = entity_render_state
        .snapshot
        .as_ref()
        .filter(|entity_snapshot| entity_snapshot.enabled)
        .and_then(|entity_snapshot| entity_snapshot.center_x.zip(entity_snapshot.center_y))
        .map(|(x, y)| presentation_pose::PresentationGridCenter { x, y });
    if matches!((map_center, entity_center), (Some(map), Some(entity)) if map != entity) {
        trace_native_map_state(
            &mut native_trace_state,
            format!("waiting-center map={map_center:?} entity={entity_center:?}"),
        );
        return;
    }

    sync_map_render_atlas_layouts(
        snapshot,
        &asset_server,
        &mut atlas_assets,
        &mut texture_atlas_layouts,
    );

    if registry.map_render.applied.as_ref().is_some_and(|applied| {
        snapshot.revision.is_some()
            && applied.producer_revision == snapshot.revision
            && applied.image_revision == atlas_assets.revision
    }) {
        return;
    }

    // The atlas IMAGES arrive on a SEPARATE async channel (setMir2MapRenderAtlas →
    // ingest_pending_map_render_images) from the state push (setMir2MapRenderState),
    // and the page decode is async on the web side — so for the first frame(s) after
    // a snapshot the runtime can hold the tile list before its atlas images are
    // ingested. If we proceeded now, `map_render_image_binding` would return None for
    // every tile (image missing), bind 0 sprites, and then mark this snapshot
    // `applied` below — permanently LOCKING IN an empty map (the early-return above
    // never lets us retry once the images finally arrive). So defer until every atlas
    // page this snapshot references has an ingested image, retrying on later frames
    // WITHOUT marking it applied. (This race — not the tile z value — was the real
    // cause of the "band-z map vanishes" bug; z=-50 only ever "worked" when the
    // images happened to be ingested before the first sync.)
    let mut failed_url_asset = None;
    let atlases_ready = snapshot.atlases.iter().all(|atlas| {
        let Some(image) = atlas_assets.images.get(&atlas.key) else {
            return false;
        };
        if !atlas_assets.url_image_keys.contains(&atlas.key) {
            return true;
        }
        match asset_server.load_state(image.id()) {
            LoadState::Loaded => asset_server.is_loaded_with_dependencies(image.id()),
            LoadState::Failed(error) => {
                failed_url_asset = Some((atlas.key.clone(), error.to_string()));
                false
            }
            _ => false,
        }
    });
    if let Some((key, error)) = failed_url_asset {
        trace_native_map_state(
            &mut native_trace_state,
            format!("asset-failed key={key} error={error}"),
        );
        publish_map_status(
            "map-render-asset-error",
            &format!("Failed to load map atlas {key}: {error}"),
            &snapshot.ack_key,
            &[key],
        );
        return;
    }
    if !atlases_ready {
        let states = snapshot
            .atlases
            .iter()
            .map(|atlas| {
                let state = atlas_assets.images.get(&atlas.key).map_or_else(
                    || "missing-handle".to_owned(),
                    |image| format!("{:?}", asset_server.load_state(image.id())),
                );
                format!("{}={state}", atlas.key)
            })
            .collect::<Vec<_>>()
            .join(",");
        trace_native_map_state(
            &mut native_trace_state,
            format!(
                "waiting-atlases center={map_center:?} count={} states=[{states}]",
                snapshot.atlases.len()
            ),
        );
        return;
    }

    // Keep the previous complete frame visible while standalone textures decode.
    // Mutating only the ready subset would despawn retained tiles and expose holes.
    let standalone_images_ready = snapshot
        .standalone_tiles
        .iter()
        .all(|tile| atlas_assets.images.contains_key(&tile.image_key));
    if !standalone_images_ready {
        let missing = snapshot
            .standalone_tiles
            .iter()
            .filter(|tile| !atlas_assets.images.contains_key(&tile.image_key))
            .map(|tile| tile.image_key.as_str())
            .collect::<Vec<_>>()
            .join(",");
        trace_native_map_state(
            &mut native_trace_state,
            format!("waiting-standalone missing=[{missing}]"),
        );
        return;
    }

    // Animation-family textures are residency-only: they participate in the
    // atomic frame handoff but never produce map render entities themselves.
    let retained_images_ready = snapshot
        .retained_image_keys
        .iter()
        .all(|key| atlas_assets.images.contains_key(key));
    if !retained_images_ready {
        let missing = snapshot
            .retained_image_keys
            .iter()
            .filter(|key| !atlas_assets.images.contains_key(*key))
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(",");
        trace_native_map_state(
            &mut native_trace_state,
            format!("waiting-retained missing=[{missing}]"),
        );
        return;
    }

    // A loaded atlas page does not guarantee that every draw can bind: stale
    // manifests or an incorrect library-index mapping can still omit a layout
    // or rect. Keep rendering the valid subset, but surface exact coverage in
    // the native trace instead of silently turning skipped draws into black
    // background holes.
    let missing_bindings = map_render_missing_bindings(snapshot, &atlas_assets);

    // The producer uses the fold-in model (`cameraOffset` stays (0, 0); sub-cell
    // motion is already baked into `left`/`top`), so this offset is normally a
    // no-op — folded in here only so a future offset-model producer still positions
    // tiles correctly.
    let offset_x = map_camera_offset.x;
    let offset_y = -map_camera_offset.y;
    // RETAIN-IN-PLACE: update surviving tiles, spawn newcomers, despawn leavers —
    // mirrors sync_entity_render_layers so sub-cell motion is cheap Transform writes
    // rather than a full despawn+respawn of every tile each frame.
    let rebind_images = registry
        .map_render
        .applied
        .is_none_or(|applied| applied.image_revision != atlas_assets.revision);
    registry.map_render.generation = registry.map_render.generation.saturating_add(1);
    let generation = registry.map_render.generation;
    for tile in &snapshot.tiles {
        // Screen-stage → centred world coords, Y flipped — identical to
        // `entity_render_layer_position` so tiles share the entities' space.
        let world_x = tile.left + tile.width * 0.5 - snapshot.stage_width * 0.5 + offset_x;
        let world_y = snapshot.stage_height * 0.5 - (tile.top + tile.height * 0.5) + offset_y;
        // Unified y-sort with entities (Stage 2): same depth→world-z conversion as
        // entity_render_layer_position incl. the entity producer's ×10 gain.
        let world_z = tile.z * MAP_TILE_ENTITY_DEPTH_GAIN / MAP_TILE_Z_DENOM;
        if let Some(handle) = registry.map_render.tiles.get_mut(&tile.key) {
            handle.last_seen_generation = generation;
            if rebind_images {
                if let Some((image, texture_atlas)) = map_render_image_binding(tile, &atlas_assets)
                {
                    if let Ok(mut sprite) = sprite_query.get_mut(handle.entity) {
                        sprite.image = image;
                        sprite.texture_atlas = Some(texture_atlas);
                    }
                }
            }
            if let Ok(mut transform) = transform_query.get_mut(handle.entity) {
                transform.translation = Vec3::new(world_x, world_y, world_z);
            }
        } else {
            let Some((image, texture_atlas)) = map_render_image_binding(tile, &atlas_assets) else {
                continue;
            };
            let custom_size = Some(Vec2::new(tile.width, tile.height));
            let color = Color::srgba(1.0, 1.0, 1.0, tile.opacity.unwrap_or(1.0));
            let entity = commands
                .spawn((
                    Sprite {
                        image,
                        texture_atlas: Some(texture_atlas),
                        custom_size,
                        color,
                        ..default()
                    },
                    Transform::from_xyz(world_x, world_y, world_z),
                ))
                .id();
            registry.map_render.tiles.insert(
                tile.key.clone(),
                MapRenderTileHandle {
                    entity,
                    last_seen_generation: generation,
                },
            );
        }
    }

    for tile in &snapshot.standalone_tiles {
        let world_x = tile.left + tile.width * 0.5 - snapshot.stage_width * 0.5 + offset_x;
        let world_y = snapshot.stage_height * 0.5 - (tile.top + tile.height * 0.5) + offset_y;
        let world_z = tile.z * MAP_TILE_ENTITY_DEPTH_GAIN / MAP_TILE_Z_DENOM;
        if let Some(handle) = registry.map_render.tiles.get_mut(&tile.key) {
            handle.last_seen_generation = generation;
            if rebind_images {
                let image = atlas_assets
                    .images
                    .get(&tile.image_key)
                    .expect("standalone images were preflighted")
                    .clone();
                if tile.additive {
                    let material = additive_cache.material(
                        &tile.key,
                        image,
                        tile.opacity.unwrap_or(1.0),
                        &mut additive_materials,
                    );
                    if let Ok(mut binding) = additive_material_query.get_mut(handle.entity) {
                        *binding = MeshMaterial2d(material);
                    }
                } else if let Ok(mut sprite) = sprite_query.get_mut(handle.entity) {
                    sprite.image = image;
                    sprite.texture_atlas = None;
                }
            }
            if let Ok(mut transform) = transform_query.get_mut(handle.entity) {
                transform.translation = Vec3::new(world_x, world_y, world_z);
                if tile.additive {
                    transform.scale = Vec3::new(tile.width, tile.height, 1.0);
                }
            }
        } else {
            let image = atlas_assets
                .images
                .get(&tile.image_key)
                .expect("standalone images were preflighted")
                .clone();
            let entity = if tile.additive {
                let mesh = additive_cache.unit_quad(&mut meshes);
                let material = additive_cache.material(
                    &tile.key,
                    image,
                    tile.opacity.unwrap_or(1.0),
                    &mut additive_materials,
                );
                commands
                    .spawn((
                        Mesh2d(mesh),
                        MeshMaterial2d(material),
                        Transform::from_xyz(world_x, world_y, world_z).with_scale(Vec3::new(
                            tile.width,
                            tile.height,
                            1.0,
                        )),
                    ))
                    .id()
            } else {
                let custom_size = Some(Vec2::new(tile.width, tile.height));
                let color = Color::srgba(1.0, 1.0, 1.0, tile.opacity.unwrap_or(1.0));
                commands
                    .spawn((
                        Sprite {
                            image,
                            custom_size,
                            color,
                            ..default()
                        },
                        Transform::from_xyz(world_x, world_y, world_z),
                    ))
                    .id()
            };
            registry.map_render.tiles.insert(
                tile.key.clone(),
                MapRenderTileHandle {
                    entity,
                    last_seen_generation: generation,
                },
            );
        }
    }

    // Despawn tiles that left the viewport / are no longer in the snapshot.
    let stale: Vec<String> = registry
        .map_render
        .tiles
        .iter()
        .filter_map(|(key, handle)| {
            (handle.last_seen_generation != generation).then(|| key.clone())
        })
        .collect();
    for key in stale {
        if let Some(handle) = registry.map_render.tiles.remove(&key) {
            commands.entity(handle.entity).despawn();
        }
    }

    // The replacement frame is complete. Release URL/upload handles and atlas
    // layouts that are no longer referenced only now, after every surviving
    // sprite has rebound, so async page transitions never expose map holes.
    let active_atlas_keys: HashSet<String> = snapshot
        .atlases
        .iter()
        .map(|atlas| atlas.key.clone())
        .collect();
    let active_image_keys = map_render_active_image_keys(snapshot);
    let previous_image_count = atlas_assets.images.len();
    let previous_layout_count = atlas_assets.layouts.len();
    atlas_assets
        .images
        .retain(|key, _| active_image_keys.contains(key));
    atlas_assets
        .url_image_keys
        .retain(|key| active_image_keys.contains(key));
    atlas_assets
        .layouts
        .retain(|key, _| active_atlas_keys.contains(key));
    atlas_assets
        .layout_rects
        .retain(|key, _| active_atlas_keys.contains(key));
    atlas_assets
        .layout_sizes
        .retain(|key, _| active_atlas_keys.contains(key));
    if atlas_assets.images.len() != previous_image_count
        || atlas_assets.layouts.len() != previous_layout_count
    {
        atlas_assets.revision = atlas_assets.revision.wrapping_add(1);
    }

    let live_count = registry.map_render.tiles.len();
    let additive_count = snapshot
        .standalone_tiles
        .iter()
        .filter(|tile| tile.additive)
        .count();
    registry.map_render.applied = Some(AppliedMapRenderState {
        producer_revision: snapshot.revision,
        image_revision: atlas_assets.revision,
    });
    presentation_poses.set_applied_map_provenance(
        snapshot
            .center_x
            .zip(snapshot.center_y)
            .map(|(x, y)| presentation_pose::PresentationGridCenter { x, y }),
        snapshot.revision,
    );
    let mut presented_image_keys: Vec<String> = active_image_keys.into_iter().collect();
    presented_image_keys.sort();
    publish_map_status(
        "map-render-synced",
        &format!(
            "Applied {} map tiles + {} standalone tiles ({} additive, {} live)",
            snapshot.tiles.len(),
            snapshot.standalone_tiles.len(),
            additive_count,
            live_count,
        ),
        &snapshot.ack_key,
        &presented_image_keys,
    );
    trace_native_map_state(
        &mut native_trace_state,
        format!(
            "synced center={map_center:?} tiles={} standalone={} live={live_count} missingBindings={} sample=[{}]",
            snapshot.tiles.len(),
            snapshot.standalone_tiles.len(),
            missing_bindings.len(),
            missing_bindings.iter().take(8).cloned().collect::<Vec<_>>().join(",")
        ),
    );
}

fn trace_native_map_state(last: &mut Option<String>, message: String) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if (std::env::var_os("MIR2_NATIVE_TRACE_MAP").is_some()
            || std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some())
            && last.as_ref() != Some(&message)
        {
            eprintln!("[runtime-map] {message}");
            *last = Some(message);
        }
    }

    #[cfg(target_arch = "wasm32")]
    let _ = (last, message);
}

/// Build the `TextureAtlasLayout` for each map atlas page not already cached.
/// Copy of `sync_entity_render_atlas_layouts`, writing into the SEPARATE
/// `RuntimeMapRenderAtlases.layouts` registry so the entity retain logic can't
/// evict map layouts. Map atlas pages are stable across the session, so no
/// retain/evict pass is needed here.
fn sync_map_render_atlas_layouts(
    snapshot: &MapRenderState,
    asset_server: &AssetServer,
    atlas_assets: &mut RuntimeMapRenderAtlases,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
) {
    // Native MapRenderState carries URL-backed standalone images in the same
    // JSON as the atlas descriptors. WASM may upload those pixels through the
    // separate setMir2MapRenderAtlas channel, but the Windows host has no such
    // uploader. Queue every URL-backed image here so the atomic readiness gate
    // below can complete instead of waiting forever on standalone foregrounds.
    for (key, asset_path) in map_render_url_image_sources(snapshot) {
        if atlas_assets.images.contains_key(&key) {
            continue;
        }
        atlas_assets
            .images
            .insert(key.clone(), asset_server.load(asset_path));
        atlas_assets.url_image_keys.insert(key);
        atlas_assets.revision = atlas_assets.revision.wrapping_add(1);
    }

    for atlas in &snapshot.atlases {
        // The producer sends only the rects used by the current viewport. The
        // atlas page key remains stable as the player moves, so retaining the
        // first layout forever makes every newly encountered rect fail binding
        // and exposes the black window background as grid-aligned holes. Grow a
        // page's known rect set monotonically and rebuild only when it changes.
        let merged = merge_map_render_atlas_rects(atlas, atlas_assets);
        if merged.is_none() && atlas_assets.layouts.contains_key(&atlas.key) {
            continue;
        }
        let (size, known_rects) = merged.unwrap_or_else(|| {
            (
                UVec2::new(atlas.width, atlas.height),
                atlas_assets
                    .layout_rects
                    .get(&atlas.key)
                    .cloned()
                    .unwrap_or_default(),
            )
        });

        let mut layout = TextureAtlasLayout::new_empty(size);
        let mut rects = HashMap::new();
        let mut known_rects = known_rects.into_iter().collect::<Vec<_>>();
        known_rects.sort_by(|left, right| left.0.cmp(&right.0));
        for (key, geometry) in known_rects {
            let index = layout.add_texture(geometry);
            rects.insert(key, index);
        }
        let layout = texture_atlas_layouts.add(layout);
        if let Some((stale_layout, _)) = atlas_assets
            .layouts
            .insert(atlas.key.clone(), (layout, rects))
        {
            texture_atlas_layouts.remove(stale_layout.id());
        }
        atlas_assets.layout_sizes.insert(atlas.key.clone(), size);
        atlas_assets.revision = atlas_assets.revision.wrapping_add(1);
    }
}

fn merge_map_render_atlas_rects(
    atlas: &MapRenderAtlas,
    atlas_assets: &mut RuntimeMapRenderAtlases,
) -> Option<(UVec2, HashMap<String, URect>)> {
    let size = UVec2::new(atlas.width, atlas.height);
    let size_changed = atlas_assets.layout_sizes.get(&atlas.key) != Some(&size);
    if size_changed {
        atlas_assets.layout_rects.remove(&atlas.key);
    }
    let known = atlas_assets
        .layout_rects
        .entry(atlas.key.clone())
        .or_default();
    let mut changed = size_changed;
    for rect in &atlas.rects {
        let geometry = URect {
            min: UVec2::new(rect.x, rect.y),
            max: UVec2::new(rect.x + rect.width, rect.y + rect.height),
        };
        if known.get(&rect.key) != Some(&geometry) {
            known.insert(rect.key.clone(), geometry);
            changed = true;
        }
    }
    changed.then(|| (size, known.clone()))
}

/// Resolve a tile to its atlas page image + `TextureAtlas` (layout + sub-rect
/// index). Returns `None` when the page image hasn't been uploaded yet or the
/// rect key isn't in the page layout, so the tile is simply skipped this build.
fn map_render_image_binding(
    tile: &MapTile,
    atlas_assets: &RuntimeMapRenderAtlases,
) -> Option<(Handle<Image>, TextureAtlas)> {
    let (layout, rects) = atlas_assets.layouts.get(&tile.atlas_key)?;
    let index = *rects.get(&tile.atlas_rect_key)?;
    let image = atlas_assets.images.get(&tile.atlas_key)?.clone();
    Some((
        image,
        TextureAtlas {
            layout: layout.clone(),
            index,
        },
    ))
}

fn map_render_missing_bindings(
    snapshot: &MapRenderState,
    atlas_assets: &RuntimeMapRenderAtlases,
) -> Vec<String> {
    snapshot
        .tiles
        .iter()
        .filter_map(|tile| {
            let reason = match atlas_assets.layouts.get(&tile.atlas_key) {
                None => "layout",
                Some((_, rects)) if !rects.contains_key(&tile.atlas_rect_key) => "rect",
                Some(_) if !atlas_assets.images.contains_key(&tile.atlas_key) => "image",
                Some(_) => return None,
            };
            Some(format!(
                "{}:{}#{}:{reason}",
                tile.key, tile.atlas_key, tile.atlas_rect_key
            ))
        })
        .collect()
}

fn sync_map_scene(
    mut commands: Commands,
    state: Res<RuntimeWorldState>,
    map_state: Res<RuntimeMapRenderState>,
    mut registry: ResMut<SceneRegistry>,
) {
    // When the Bevy-native map renderer is active it draws the real map tiles;
    // the placeholder floor must not also draw (same pattern as `sync_entities`
    // early-returning when the entity render path is enabled). Despawn any
    // placeholder floor still spawned and clear the blueprint so it rebuilds
    // cleanly if map render is later disabled.
    if map_state
        .snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.enabled)
    {
        for entity in registry.map.spawned.drain(..) {
            commands.entity(entity).despawn();
        }
        registry.map.blueprint = None;
        return;
    }

    let Some(snapshot) = &state.snapshot else {
        return;
    };

    let blueprint = MapSceneBlueprint::from_snapshot(snapshot);
    if registry.map.blueprint.as_ref() == Some(&blueprint) {
        return;
    }

    for entity in registry.map.spawned.drain(..) {
        commands.entity(entity).despawn();
    }

    registry.map.spawned = spawn_map_scene(&mut commands, &blueprint);
    let title = blueprint
        .map_title
        .clone()
        .unwrap_or_else(|| "unknown".to_owned());
    registry.map.blueprint = Some(blueprint);
    publish_status(
        "scene-synced",
        &format!("Applied scene blueprint for {title}"),
    );
}

fn sync_entities(
    mut commands: Commands,
    state: Res<RuntimeWorldState>,
    entity_render_state: Res<RuntimeEntityRenderState>,
    snap_buf: Res<interpolation::SnapshotBuffer>,
    motion_table: Res<motion::EntityMotionTable>,
    time: Res<Time>,
    mut registry: ResMut<SceneRegistry>,
    mut transform_query: Query<&mut Transform>,
    mut sprite_query: Query<&mut Sprite>,
) {
    if entity_render_state
        .snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.enabled)
    {
        for (_, handles) in registry.entities.drain() {
            commands.entity(handles.root).despawn();
        }
        return;
    }

    let Some(snapshot) = &state.snapshot else {
        return;
    };

    // Compute interpolation parameters once for this frame.
    //
    // When the buffer holds two snapshots we lerp entity positions at
    // `render_time = now – INTERP_DELAY`.  If the buffer isn't ready yet
    // (first snapshot, or only one received so far) we fall through to the
    // plain snap path so there is no regression.
    let interp_params: Option<(f32, &interpolation::BufferedSnapshot)> = if snap_buf.ready() {
        let prev = snap_buf.prev.as_ref().unwrap();
        let next = snap_buf.next.as_ref().unwrap();
        let render_t = time.elapsed_secs_f64() - interpolation::INTERP_DELAY_SECS;
        let alpha =
            interpolation::interpolation_alpha(prev.receipt_secs, next.receipt_secs, render_t);
        Some((alpha, prev))
    } else {
        None
    };

    let mut alive = HashSet::new();

    for entity_data in &snapshot.entities {
        alive.insert(entity_data.object_id.clone());

        // Compute the render-position for this entity.
        //
        // Priority 1: wall-clock motion authority (motion.rs).  When the
        //   EntityMotionTable has an entry for this entity — meaning it received
        //   movement timing metadata from the TS producer — use that to place
        //   the entity at its smoothly-interpolated sub-tile position.
        //
        // Priority 2: Phase 0.4 snapshot lerp (fallback when motion table has
        //   no entry, e.g. before the TS producer sends timing metadata).
        //
        // Priority 3: snap to the grid cell in the latest snapshot.
        let position = if let Some(entry) = motion_table.get(&entity_data.object_id) {
            motion::world_position_with_motion(
                entity_data.x,
                entity_data.y,
                entry,
                motion_table.now_ms,
                TILE_SIZE,
            )
        } else {
            match &interp_params {
                Some((alpha, prev_snap)) => {
                    let target = tile_to_world(entity_data.x, entity_data.y);
                    if let Some(prev_pos) = prev_snap.positions.get(&entity_data.object_id) {
                        let from = tile_to_world(prev_pos.x, prev_pos.y);
                        interpolation::lerp_entity_pos(from, target, *alpha)
                    } else {
                        // New entity — snap it in at the current position.
                        target
                    }
                }
                None => tile_to_world(entity_data.x, entity_data.y),
            }
        };

        let is_selected = snapshot
            .selected_object_id
            .as_ref()
            .is_some_and(|selected_id| selected_id == &entity_data.object_id);

        if let Some(handles) = registry.entities.get(&entity_data.object_id).copied() {
            if let Ok(mut transform) = transform_query.get_mut(handles.root) {
                transform.translation = position;
            }
            update_entity_visuals(
                handles,
                entity_data,
                is_selected,
                &mut transform_query,
                &mut sprite_query,
            );
            continue;
        }

        let mut shadow = None;
        let mut body = None;
        let mut crest = None;
        let mut facing = None;
        let mut selection = None;
        let entity = commands
            .spawn((MirObject, Transform::from_translation(position)))
            .with_children(|parent| {
                selection = Some(
                    parent
                        .spawn((
                            Sprite::from_color(Color::NONE, Vec2::new(36.0, 36.0)),
                            Transform::from_xyz(0.0, -8.0, 0.0),
                        ))
                        .id(),
                );
                shadow = Some(
                    parent
                        .spawn((
                            Sprite::from_color(
                                Color::srgba(0.02, 0.02, 0.02, 0.22),
                                Vec2::new(26.0, 10.0),
                            ),
                            Transform::from_xyz(0.0, -10.0, 0.5),
                        ))
                        .id(),
                );
                body = Some(
                    parent
                        .spawn((
                            Sprite::from_color(entity_data.kind.color(), entity_data.body_size()),
                            Transform::from_xyz(0.0, 6.0, 1.0),
                        ))
                        .id(),
                );
                crest = Some(
                    parent
                        .spawn((
                            Sprite::from_color(
                                entity_data.kind.accent_color(&entity_data.name),
                                entity_data.crest_size(entity_data.body_size()),
                            ),
                            Transform::from_xyz(0.0, 16.0, 2.0),
                        ))
                        .id(),
                );
                facing = Some(
                    parent
                        .spawn((
                            Sprite::from_color(
                                entity_data.kind.facing_color(),
                                entity_data.kind.facing_size(),
                            ),
                            Transform::from_xyz(0.0, 0.0, 3.0),
                        ))
                        .id(),
                );
            })
            .id();

        let handles = SceneEntityHandles {
            root: entity,
            shadow: shadow.expect("shadow child"),
            body: body.expect("body child"),
            crest: crest.expect("crest child"),
            facing: facing.expect("facing child"),
            selection: selection.expect("selection child"),
        };

        update_entity_visuals(
            handles,
            entity_data,
            is_selected,
            &mut transform_query,
            &mut sprite_query,
        );

        registry
            .entities
            .insert(entity_data.object_id.clone(), handles);
    }

    let stale_ids: Vec<String> = registry
        .entities
        .keys()
        .filter(|object_id| !alive.contains(*object_id))
        .cloned()
        .collect();

    for object_id in stale_ids {
        if let Some(handles) = registry.entities.remove(&object_id) {
            commands.entity(handles.root).despawn();
        }
    }
}

fn sync_entity_render_layers(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    atlas_assets: Res<RuntimeEntityRenderAtlases>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut additive_materials: ResMut<Assets<additive_material::CrystalAdditiveMaterial>>,
    mut additive_cache: ResMut<additive_material::CrystalAdditiveMaterialCache>,
    entity_render_state: Res<RuntimeEntityRenderState>,
    motion_table: Res<motion::EntityMotionTable>,
    mut local_motion: ResMut<local_motion::LocalMotionPresentationShadow>,
    mut remote_motion: ResMut<remote_motion::RemoteMotionPresentation>,
    mut presentation_poses: ResMut<presentation_pose::PresentationPoseBuffer>,
    mut registry: ResMut<SceneRegistry>,
    mut transform_query: Query<&mut Transform>,
    mut sprite_query: Query<&mut Sprite>,
    mut additive_material_query: Query<
        &mut MeshMaterial2d<additive_material::CrystalAdditiveMaterial>,
    >,
) {
    let Some(snapshot) = &entity_render_state.snapshot else {
        clear_entity_render_layers(
            &mut commands,
            &mut registry,
            &mut additive_cache,
            &mut additive_materials,
        );
        presentation_poses.set_applied_entity_center(None);
        return;
    };

    if !snapshot.enabled {
        clear_entity_render_layers(
            &mut commands,
            &mut registry,
            &mut additive_cache,
            &mut additive_materials,
        );
        presentation_poses.set_applied_entity_center(None);
        return;
    }

    let entity_center = snapshot
        .center_x
        .zip(snapshot.center_y)
        .map(|(x, y)| presentation_pose::PresentationGridCenter { x, y });
    if matches!(
        (presentation_poses.applied_map_center(), entity_center),
        (Some(map), Some(entity)) if map != entity
    ) {
        // The matching map snapshot has not committed yet. Keep the previous
        // entity transforms/provenance so the renderer never publishes a
        // mixed-center frame; this state is retried on the next Bevy tick.
        return;
    }

    if local_motion.presentation_enabled() {
        let self_entity = snapshot.entities.iter().find(|entity| entity.is_self);
        let active_ts_window = active_self_camera_motion_window(motion_table.now_ms);
        let ts_allows_command = active_ts_window
            .map(|window| {
                local_motion.segment_matches_ts_window(window)
                    || local_motion.committed_segment_matches_ts_target(window)
            })
            .unwrap_or(true);
        let matching_center =
            entity_center.filter(|center| presentation_poses.applied_map_center() == Some(*center));
        let takeover_candidate = self_entity
            .zip(matching_center)
            .filter(|_| ts_allows_command)
            .and_then(|(self_entity, center)| {
                local_motion.candidate_offset_for_applied_center(
                    &self_entity.object_id,
                    center.x,
                    center.y,
                    motion_table.now_ms,
                    48.0,
                    32.0,
                )
            });

        if let Some(candidate) = takeover_candidate {
            local_motion.mark_presentation_committed();
            let motion = local_motion.presentation_phase().map(|phase| {
                presentation_pose::EntityPresentationMotion {
                    frame_index: phase.frame_index,
                    phase_count: phase.phase_count,
                    mode: phase.mode,
                    direction: phase.direction,
                }
            });
            presentation_poses.reconcile_local_command_for_applied_center(candidate, motion);
        }
    }

    let mut alive = HashSet::new();
    sync_entity_render_atlas_layouts(
        snapshot,
        &asset_server,
        &atlas_assets,
        &mut texture_atlas_layouts,
        &mut registry,
    );

    for entity in &snapshot.entities {
        // Sub-cell glide offset for this entity (CSS-px screen space). Default
        // path / self player ⇒ no motion window ⇒ Vec2::ZERO ⇒ byte-identical to
        // the producer's fold-in. Computed once per entity, applied to every layer.
        let remote_offset = (!entity.is_self)
            .then(|| entity.grid_x.zip(entity.grid_y))
            .flatten()
            .and_then(|(x, y)| {
                remote_motion.presentation_offset(
                    &entity.object_id,
                    x,
                    y,
                    motion_table.now_ms,
                    48.0,
                    32.0,
                )
            });
        let (motion_offset, pose_source) = if entity.is_self {
            let source = match presentation_poses.camera_source() {
                presentation_pose::CameraPoseSource::LocalCommand => {
                    presentation_pose::EntityPoseSource::LocalCommand
                }
                presentation_pose::CameraPoseSource::SelfWindow => {
                    presentation_pose::EntityPoseSource::SnapshotWindow
                }
                presentation_pose::CameraPoseSource::Static => {
                    presentation_pose::EntityPoseSource::Static
                }
            };
            (presentation_poses.self_entity_offset(), source)
        } else if let Some(offset) = remote_offset {
            (offset, presentation_pose::EntityPoseSource::RemotePacket)
        } else {
            let source = if entity_has_motion_window(entity) {
                presentation_pose::EntityPoseSource::SnapshotWindow
            } else {
                presentation_pose::EntityPoseSource::Static
            };
            (entity_interp_offset(entity, motion_table.now_ms), source)
        };
        presentation_poses.record_entity(&entity.object_id, motion_offset, pose_source);
        for layer in &entity.layers {
            let layer_key = if layer.key.is_empty() {
                format!("{}:{}", entity.object_id, layer.path)
            } else {
                layer.key.clone()
            };
            alive.insert(layer_key.clone());
            let position = entity_render_layer_position(snapshot, layer, motion_offset);
            let opacity = layer.opacity.unwrap_or(1.0);
            let image_binding =
                entity_render_image_binding(layer, &asset_server, &atlas_assets, &registry);

            if registry
                .entity_render_layers
                .get(&layer_key)
                .is_some_and(|handle| handle.additive != layer.additive)
            {
                if let Some(old) = registry.entity_render_layers.remove(&layer_key) {
                    commands.entity(old.entity).despawn();
                    if old.additive {
                        additive_cache.evict(
                            &entity_additive_material_key(&layer_key),
                            &mut additive_materials,
                        );
                    }
                }
            }

            if let Some(handle) = registry.entity_render_layers.get_mut(&layer_key) {
                let binding_changed = handle.image_key != image_binding.image_key
                    || handle.atlas_key != image_binding.atlas_key
                    || handle.atlas_rect_key != image_binding.atlas_rect_key;
                if layer.additive {
                    let material = additive_cache.material_with_uv(
                        &entity_additive_material_key(&layer_key),
                        image_binding.image.clone(),
                        opacity,
                        image_binding.uv_scale_offset,
                        &mut additive_materials,
                    );
                    if let Ok(mut binding) = additive_material_query.get_mut(handle.entity) {
                        *binding = MeshMaterial2d(material);
                    }
                } else if binding_changed {
                    if let Ok(mut sprite) = sprite_query.get_mut(handle.entity) {
                        sprite.image = image_binding.image.clone();
                        sprite.texture_atlas = image_binding.texture_atlas.clone();
                        sprite.rect = None;
                    }
                }
                handle.image_key = image_binding.image_key.clone();
                handle.atlas_key = image_binding.atlas_key.clone();
                handle.atlas_rect_key = image_binding.atlas_rect_key.clone();
                if !layer.additive {
                    if let Ok(mut sprite) = sprite_query.get_mut(handle.entity) {
                        sprite.custom_size =
                            Some(Vec2::new(layer.width.max(1.0), layer.height.max(1.0)));
                        sprite.color = Color::srgba(1.0, 1.0, 1.0, opacity.clamp(0.0, 1.0));
                        sprite.texture_atlas = image_binding.texture_atlas.clone();
                        sprite.rect = None;
                    }
                }
                if let Ok(mut transform) = transform_query.get_mut(handle.entity) {
                    transform.translation = position;
                    if layer.additive {
                        transform.scale =
                            Vec3::new(layer.width.max(1.0), layer.height.max(1.0), 1.0);
                    }
                }
                continue;
            }

            let sprite_entity = if layer.additive {
                let mesh = additive_cache.unit_quad(&mut meshes);
                let material = additive_cache.material_with_uv(
                    &entity_additive_material_key(&layer_key),
                    image_binding.image.clone(),
                    opacity,
                    image_binding.uv_scale_offset,
                    &mut additive_materials,
                );
                commands
                    .spawn((
                        MirEntityRenderLayer,
                        Mesh2d(mesh),
                        MeshMaterial2d(material),
                        Transform::from_translation(position).with_scale(Vec3::new(
                            layer.width.max(1.0),
                            layer.height.max(1.0),
                            1.0,
                        )),
                    ))
                    .id()
            } else {
                commands
                    .spawn((
                        MirEntityRenderLayer,
                        Sprite {
                            image: image_binding.image.clone(),
                            texture_atlas: image_binding.texture_atlas.clone(),
                            custom_size: Some(Vec2::new(
                                layer.width.max(1.0),
                                layer.height.max(1.0),
                            )),
                            color: Color::srgba(1.0, 1.0, 1.0, opacity.clamp(0.0, 1.0)),
                            ..default()
                        },
                        Transform::from_translation(position),
                    ))
                    .id()
            };
            registry.entity_render_layers.insert(
                layer_key,
                EntityRenderLayerHandle {
                    entity: sprite_entity,
                    image_key: image_binding.image_key,
                    atlas_key: image_binding.atlas_key,
                    atlas_rect_key: image_binding.atlas_rect_key,
                    additive: layer.additive,
                },
            );
        }
    }

    let stale_keys: Vec<String> = registry
        .entity_render_layers
        .keys()
        .filter(|key| !alive.contains(*key))
        .cloned()
        .collect();

    for key in stale_keys {
        if let Some(handle) = registry.entity_render_layers.remove(&key) {
            commands.entity(handle.entity).despawn();
            if handle.additive {
                additive_cache.evict(&entity_additive_material_key(&key), &mut additive_materials);
            }
        }
    }

    presentation_poses.set_applied_entity_center(entity_center);
}

struct EntityRenderImageBinding {
    image: Handle<Image>,
    image_key: String,
    atlas_key: Option<String>,
    atlas_rect_key: Option<String>,
    texture_atlas: Option<TextureAtlas>,
    uv_scale_offset: Vec4,
}

fn entity_additive_material_key(layer_key: &str) -> String {
    format!("entity:{layer_key}")
}

fn sync_entity_render_atlas_layouts(
    snapshot: &EntityRenderState,
    asset_server: &AssetServer,
    atlas_assets: &RuntimeEntityRenderAtlases,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
    registry: &mut SceneRegistry,
) {
    let mut alive = HashSet::new();

    for atlas in &snapshot.atlases {
        alive.insert(atlas.key.clone());
        let size = UVec2::new(atlas.width, atlas.height);
        let uploaded_image = atlas_assets.images.get(&atlas.key).cloned();
        let url_image = atlas.image_url.as_ref().map(|image_url| {
            let asset_path = browser_asset_path(image_url);
            (asset_path.clone(), asset_server.load(asset_path))
        });
        let (image_key, image) = if let Some((image_key, image)) = url_image {
            (Some(image_key), Some(image))
        } else if let Some(image) = uploaded_image {
            (Some(format!("atlas:{}", atlas.key)), Some(image))
        } else {
            (None, None)
        };

        // Prebuilt pages keep a stable page key while the web producer sends
        // only rects used by the current scene. A turn or attack can therefore
        // add rects under an existing key. Reusing the old layout made those
        // frames miss the atlas and fall back to asynchronous per-PNG loading,
        // which presented as disappearing monsters and skipped attacks.
        let layout_is_current =
            registry
                .entity_render_atlases
                .get(&atlas.key)
                .is_some_and(|existing| {
                    existing.size == size
                        && existing.image_key == image_key
                        && entity_render_atlas_contains_rects(&existing.rects, &atlas.rects)
                });
        if layout_is_current {
            continue;
        }

        let mut layout = TextureAtlasLayout::new_empty(size);
        let mut rects = HashMap::new();
        let mut uv_rects = HashMap::new();
        for rect in &atlas.rects {
            let uv_rect = URect {
                min: UVec2::new(rect.x, rect.y),
                max: UVec2::new(rect.x + rect.width, rect.y + rect.height),
            };
            let index = layout.add_texture(uv_rect);
            rects.insert(rect.key.clone(), index);
            uv_rects.insert(rect.key.clone(), uv_rect);
        }
        let layout = texture_atlas_layouts.add(layout);
        registry.entity_render_atlases.insert(
            atlas.key.clone(),
            EntityRenderAtlasHandle {
                layout,
                rects,
                uv_rects,
                size,
                image_key,
                image,
            },
        );
    }

    registry
        .entity_render_atlases
        .retain(|key, _| alive.contains(key));
}

fn entity_render_atlas_contains_rects(
    existing: &HashMap<String, usize>,
    incoming: &[EntityRenderAtlasRect],
) -> bool {
    incoming.iter().all(|rect| existing.contains_key(&rect.key))
}

fn entity_render_image_binding(
    layer: &EntityRenderLayer,
    asset_server: &AssetServer,
    atlas_assets: &RuntimeEntityRenderAtlases,
    registry: &SceneRegistry,
) -> EntityRenderImageBinding {
    if let (Some(atlas_key), Some(rect_key)) = (&layer.atlas_key, &layer.atlas_rect_key) {
        if let Some(atlas) = registry.entity_render_atlases.get(atlas_key) {
            if let Some(index) = atlas.rects.get(rect_key) {
                let uv_scale_offset = atlas
                    .uv_rects
                    .get(rect_key)
                    .map(|rect| entity_atlas_uv_scale_offset(atlas.size, *rect))
                    .unwrap_or(Vec4::new(1.0, 1.0, 0.0, 0.0));
                if let (Some(image_key), Some(image)) = (&atlas.image_key, &atlas.image) {
                    return EntityRenderImageBinding {
                        image: image.clone(),
                        image_key: image_key.clone(),
                        atlas_key: Some(atlas_key.clone()),
                        atlas_rect_key: Some(rect_key.clone()),
                        texture_atlas: Some(TextureAtlas {
                            layout: atlas.layout.clone(),
                            index: *index,
                        }),
                        uv_scale_offset,
                    };
                }

                if let Some(image) = atlas_assets.images.get(atlas_key) {
                    return EntityRenderImageBinding {
                        image: image.clone(),
                        image_key: format!("atlas:{atlas_key}"),
                        atlas_key: Some(atlas_key.clone()),
                        atlas_rect_key: Some(rect_key.clone()),
                        texture_atlas: Some(TextureAtlas {
                            layout: atlas.layout.clone(),
                            index: *index,
                        }),
                        uv_scale_offset,
                    };
                }
            }
        }
    }

    let asset_path = browser_asset_path(&layer.path);
    EntityRenderImageBinding {
        image: asset_server.load(asset_path.clone()),
        image_key: asset_path,
        atlas_key: None,
        atlas_rect_key: None,
        texture_atlas: None,
        uv_scale_offset: Vec4::new(1.0, 1.0, 0.0, 0.0),
    }
}

fn entity_atlas_uv_scale_offset(size: UVec2, rect: URect) -> Vec4 {
    if size.x == 0 || size.y == 0 {
        return Vec4::new(1.0, 1.0, 0.0, 0.0);
    }
    let width = size.x as f32;
    let height = size.y as f32;
    Vec4::new(
        (rect.max.x - rect.min.x) as f32 / width,
        (rect.max.y - rect.min.y) as f32 / height,
        rect.min.x as f32 / width,
        rect.min.y as f32 / height,
    )
}

fn clear_entity_render_layers(
    commands: &mut Commands,
    registry: &mut SceneRegistry,
    additive_cache: &mut additive_material::CrystalAdditiveMaterialCache,
    additive_materials: &mut Assets<additive_material::CrystalAdditiveMaterial>,
) {
    for (key, handle) in registry.entity_render_layers.drain() {
        commands.entity(handle.entity).despawn();
        if handle.additive {
            additive_cache.evict(&entity_additive_material_key(&key), additive_materials);
        }
    }
    registry.entity_render_atlases.clear();
}

/// Screen→world position for one entity layer.
///
/// `offset` is an additional CSS-px SCREEN-space translation applied to the
/// layer's `left`/`top` BEFORE the screen→world conversion. It is `Vec2::ZERO`
/// on the default path (so the result is byte-identical to the producer's
/// fold-in). Under `?bevyEntityInterp=1` it carries the per-entity sub-cell glide
/// (`entity_interp_offset`); because it is added in screen space — i.e. `top`
/// increases downward and the function then flips Y via `stage_h/2 - center_y` —
/// the sign matches the DOM fold automatically, with no manual axis flip here.
fn entity_render_layer_position(
    snapshot: &EntityRenderState,
    layer: &EntityRenderLayer,
    offset: Vec2,
) -> Vec3 {
    let center_x = layer.left + offset.x + layer.width * 0.5;
    let center_y = layer.top + offset.y + layer.height * 0.5;

    Vec3::new(
        center_x - snapshot.stage_width * 0.5,
        snapshot.stage_height * 0.5 - center_y,
        layer.z / 100_000.0,
    )
}

/// Per-entity sub-cell motion offset (CSS-px SCREEN space) for the opt-in
/// `?bevyEntityInterp=1` path. Returns `Vec2::ZERO` when the producer attached no
/// motion window (the default path, and always for the self player) so the
/// position math stays byte-identical to the fold-in.
///
/// When a window is present the runtime interpolates the glide every frame from
/// the Bevy display clock (`now_ms`, the same `EntityMotionTable.now_ms` PR #125's
/// self-camera uses) instead of the ~33Hz React `motionNow` fold — this is the
/// judder fix. `expires_ms = started_ms + duration_ms`.
fn entity_interp_offset(entity: &EntityRenderEntry, now_ms: f64) -> Vec2 {
    let (Some(from_x), Some(from_y), Some(to_x), Some(to_y), Some(started_ms), Some(duration_ms)) = (
        entity.motion_from_x,
        entity.motion_from_y,
        entity.motion_to_x,
        entity.motion_to_y,
        entity.motion_started_ms,
        entity.motion_duration_ms,
    ) else {
        return Vec2::ZERO;
    };

    motion::compute_motion_offset_fractional(
        from_x,
        from_y,
        to_x,
        to_y,
        started_ms,
        started_ms + duration_ms,
        now_ms,
        48.0,
        32.0,
    )
}

fn entity_has_motion_window(entity: &EntityRenderEntry) -> bool {
    entity.motion_from_x.is_some()
        && entity.motion_from_y.is_some()
        && entity.motion_to_x.is_some()
        && entity.motion_to_y.is_some()
        && entity.motion_started_ms.is_some()
        && entity.motion_duration_ms.is_some()
}

fn browser_asset_path(path: &str) -> String {
    path.trim_start_matches('/')
        .split('?')
        .next()
        .unwrap_or(path)
        .to_owned()
}

/// Display-Hz camera scroll for the opt-in `?bevySelfCamera=1` path. Reads the
/// self motion window pushed via `setMir2SelfCameraMotion` and interpolates the
/// sub-cell offset every frame (so the scroll no longer steps at the ~33Hz React
/// `motionNow` clock). Returns `Vec2::ZERO` when no window is set (default
/// fold-in) or the step has elapsed → camera stays pinned at origin.
///
/// Units: CSS-px screen space (the entity render layers' coordinate system, 48×32
/// per cell), Y flipped because `entity_render_layer_position` maps screen-top via
/// `stage_h/2 - top`. NOTE: the x/y signs are derived from the fold-in math but
/// have NOT been visually verified on a high-refresh display — if the world
/// scrolls the wrong way, flip the sign(s) here (this is the one spot).
fn self_camera_screen_offset(
    motion_table: &motion::EntityMotionTable,
) -> (Vec2, presentation_pose::CameraPoseSource) {
    let Some((from_x, from_y, to_x, to_y, started_ms, expires_ms)) =
        PENDING_SELF_CAMERA_MOTION.with(|cell| cell.get())
    else {
        return (Vec2::ZERO, presentation_pose::CameraPoseSource::Static);
    };
    let now = motion_table.now_ms;
    if expires_ms <= started_ms || now >= expires_ms || (from_x == to_x && from_y == to_y) {
        return (Vec2::ZERO, presentation_pose::CameraPoseSource::Static);
    }
    // Successive movement windows can start from the fractional pose of the
    // previous step. Preserve that value across the JS/WASM boundary; coercing
    // it to i32 makes the fallback camera disagree with the local-command pose.
    let offset = motion::compute_motion_offset_fractional(
        from_x, from_y, to_x, to_y, started_ms, expires_ms, now, 48.0, 32.0,
    );
    (-offset, presentation_pose::CameraPoseSource::SelfWindow)
}

fn active_self_camera_motion_window(now_ms: f64) -> Option<local_motion::LocalTsMotionWindow> {
    PENDING_SELF_CAMERA_MOTION.with(|cell| {
        cell.get()
            .map(|(from_x, from_y, to_x, to_y, started_ms, expires_ms)| {
                local_motion::LocalTsMotionWindow {
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    started_ms,
                    expires_ms,
                }
            })
            .filter(|window| {
                window.expires_ms > window.started_ms
                    && now_ms < window.expires_ms
                    && (window.from_x != window.to_x || window.from_y != window.to_y)
            })
    })
}

fn begin_presentation_pose_frame(
    entity_render_state: Res<RuntimeEntityRenderState>,
    motion_table: Res<motion::EntityMotionTable>,
    mut local_motion: ResMut<local_motion::LocalMotionPresentationShadow>,
    mut presentation_poses: ResMut<presentation_pose::PresentationPoseBuffer>,
) {
    if let Some(enabled) = presentation_pose::take_pending_enabled() {
        presentation_poses.set_enabled(enabled);
    }
    let renderer_enabled = entity_render_state
        .snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.enabled);
    presentation_poses.begin_frame(motion_table.now_ms, renderer_enabled);
    let (ts_camera_offset, ts_source) = self_camera_screen_offset(&motion_table);
    let mut selected_camera_offset = ts_camera_offset;
    let mut selected_source = ts_source;
    let mut selected_motion = None;

    if let Some(self_entity) = entity_render_state
        .snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.entities.iter().find(|entity| entity.is_self))
    {
        let active_ts_window = active_self_camera_motion_window(motion_table.now_ms);
        let matching_path =
            active_ts_window.is_some_and(|window| local_motion.segment_matches_ts_window(window));
        let committed_target = active_ts_window
            .is_some_and(|window| local_motion.committed_segment_matches_ts_target(window));

        if ts_source == presentation_pose::CameraPoseSource::SelfWindow {
            if let Some(ts_window) = active_ts_window {
                if matching_path {
                    if let Some(candidate) = local_motion.candidate_offset(
                        &self_entity.object_id,
                        ts_window.to_x.round() as i32,
                        ts_window.to_y.round() as i32,
                        motion_table.now_ms,
                        48.0,
                        32.0,
                    ) {
                        local_motion.compare_with_actual(
                            motion_table.now_ms,
                            &self_entity.object_id,
                            candidate,
                            -ts_camera_offset,
                            ts_window,
                        );
                    }
                } else if local_motion.has_matching_segment_target(
                    &self_entity.object_id,
                    ts_window.to_x.round() as i32,
                    ts_window.to_y.round() as i32,
                ) {
                    local_motion.record_ts_window_path_mismatch();
                } else {
                    local_motion.record_ts_window_target_mismatch();
                }
            }
        }

        // Take over from the center both render layers actually committed, not
        // from the newly requested authoritative center. ACKs can advance the
        // snapshot one tile before the map/entity systems apply that center;
        // comparing requested-to-applied made the first movement phases fall
        // back to the opposite-signed TypeScript camera window.
        let common_applied_center = presentation_poses.coherent_applied_center();
        let ts_allows_command = active_ts_window.is_none() || matching_path || committed_target;
        let takeover_candidate = common_applied_center
            .filter(|_| ts_allows_command)
            .and_then(|center| {
                local_motion.candidate_offset_for_applied_center(
                    &self_entity.object_id,
                    center.x,
                    center.y,
                    motion_table.now_ms,
                    48.0,
                    32.0,
                )
            });

        if local_motion.presentation_enabled() {
            if let Some(candidate) = takeover_candidate {
                local_motion.mark_presentation_committed();
                // The entity offset is relative to the center that both render
                // layers actually use. Its inverse camera offset keeps the
                // composed pose continuous when that center advances a cell.
                selected_camera_offset = -candidate;
                selected_source = presentation_pose::CameraPoseSource::LocalCommand;
                selected_motion = local_motion.presentation_phase().map(|phase| {
                    presentation_pose::EntityPresentationMotion {
                        frame_index: phase.frame_index,
                        phase_count: phase.phase_count,
                        mode: phase.mode,
                        direction: phase.direction,
                    }
                });
            } else if ts_source == presentation_pose::CameraPoseSource::Static
                && self_entity
                    .grid_x
                    .zip(self_entity.grid_y)
                    .is_some_and(|(x, y)| {
                        local_motion.has_matching_segment_target(&self_entity.object_id, x, y)
                    })
            {
                // Retain ownership of the exact settled zero after a matched
                // command; corrections clear the segment and stay TS-owned.
                selected_camera_offset = Vec2::ZERO;
                selected_source = presentation_pose::CameraPoseSource::LocalCommand;
            }
        }
    }

    presentation_poses.set_local_self_motion(selected_motion);
    presentation_poses.set_camera(selected_camera_offset, selected_source);
}

fn publish_presentation_pose_frame(
    presentation_poses: Res<presentation_pose::PresentationPoseBuffer>,
) {
    presentation_poses.publish_with(presentation_pose::push_presentation_pose_json);
}

fn follow_player(
    state: Res<RuntimeWorldState>,
    entity_render_state: Res<RuntimeEntityRenderState>,
    presentation_poses: Res<presentation_pose::PresentationPoseBuffer>,
    mut camera_query: Query<
        &mut Transform,
        (
            With<MainCamera>,
            Without<MirObject>,
            Without<MirEntityRenderLayer>,
        ),
    >,
) {
    if entity_render_state
        .snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.enabled)
    {
        let Ok(mut camera_transform) = camera_query.single_mut() else {
            return;
        };
        // Default (no self motion pushed) ⇒ Vec2::ZERO ⇒ camera pinned at origin =
        // the current fold-in behaviour, byte-identical. Non-zero only under
        // ?bevySelfCamera=1.
        let camera_offset = presentation_poses.camera_screen_offset();
        let translation = Vec2::new(-camera_offset.x, camera_offset.y);
        camera_transform.translation.x = translation.x;
        camera_transform.translation.y = translation.y;
        camera_transform.translation.z = 0.0;
        return;
    }

    let Some(snapshot) = &state.snapshot else {
        return;
    };

    let focus = snapshot
        .player_object_id
        .as_ref()
        .and_then(|player_object_id| {
            snapshot
                .entities
                .iter()
                .find(|entity| &entity.object_id == player_object_id)
                .map(|entity| GridPoint {
                    x: entity.x,
                    y: entity.y,
                })
        })
        .or_else(|| snapshot.scene_view.as_ref().map(|scene| scene.center));

    let Some(focus) = focus else {
        return;
    };

    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };

    camera_transform.translation.x = focus.x as f32 * TILE_SIZE;
    camera_transform.translation.y = -(focus.y as f32) * TILE_SIZE;
    camera_transform.translation.z = 1000.0;
}

/// Per-cell entities for a rendered mine vein: a constant dark rock base plus an
/// ore overlay whose colour and size shrink as the vein is mined out.
struct MineNodeHandles {
    root: Entity,
    ore: Entity,
}

/// Ore-overlay colour and size for a depletion stage (2 full / 1 cracked / 0 empty).
fn mine_ore_visual(stage: u8) -> (Color, Vec2) {
    let full = TILE_SIZE - 12.0;
    if stage >= 2 {
        (Color::srgba(0.85, 0.66, 0.28, 0.95), Vec2::splat(full)) // full vein: big bright ore
    } else if stage == 1 {
        (
            Color::srgba(0.70, 0.52, 0.26, 0.80),
            Vec2::splat(full * 0.6),
        ) // half-mined: smaller, dimmer
    } else {
        (
            Color::srgba(0.45, 0.40, 0.30, 0.35),
            Vec2::splat(full * 0.3),
        ) // depleted: tiny, faint
    }
}

/// Render mine veins as a rock base + an ore overlay, diffing against the
/// snapshot each frame (mirrors `sync_entities`): spawn new cells, shrink/fade
/// the ore overlay as the stage drops, and despawn cells that are gone.
fn sync_mine_nodes(
    mut commands: Commands,
    state: Res<RuntimeWorldState>,
    mut registry: ResMut<SceneRegistry>,
    mut sprite_query: Query<&mut Sprite>,
) {
    let Some(snapshot) = &state.snapshot else {
        return;
    };

    let mut alive: HashSet<(i32, i32)> = HashSet::new();
    for node in &snapshot.mine_nodes {
        let cell = (node.x, node.y);
        alive.insert(cell);
        let (ore_color, ore_size) = mine_ore_visual(node.stage);

        if let Some(handles) = registry.mine_nodes.get(&cell) {
            if let Ok(mut sprite) = sprite_query.get_mut(handles.ore) {
                sprite.color = ore_color;
                sprite.custom_size = Some(ore_size);
            }
            continue;
        }

        let mut translation = tile_to_world(node.x, node.y);
        translation.z = 0.2;
        let mut ore: Option<Entity> = None;
        let root = commands
            .spawn((MirObject, Transform::from_translation(translation)))
            .with_children(|parent| {
                parent.spawn((
                    Sprite::from_color(Color::srgb(0.18, 0.16, 0.15), Vec2::splat(TILE_SIZE - 8.0)),
                    Transform::from_xyz(0.0, 0.0, 0.0),
                ));
                ore = Some(
                    parent
                        .spawn((
                            Sprite::from_color(ore_color, ore_size),
                            Transform::from_xyz(0.0, 0.0, 0.02),
                        ))
                        .id(),
                );
            })
            .id();
        if let Some(ore) = ore {
            registry
                .mine_nodes
                .insert(cell, MineNodeHandles { root, ore });
        }
    }

    let stale: Vec<(i32, i32)> = registry
        .mine_nodes
        .keys()
        .filter(|cell| !alive.contains(*cell))
        .copied()
        .collect();
    for cell in stale {
        if let Some(handles) = registry.mine_nodes.remove(&cell) {
            commands.entity(handles.root).despawn();
        }
    }
}

fn tile_to_world(x: i32, y: i32) -> Vec3 {
    Vec3::new(x as f32 * TILE_SIZE, -(y as f32) * TILE_SIZE, 1.0)
}

fn spawn_map_scene(commands: &mut Commands, blueprint: &MapSceneBlueprint) -> Vec<Entity> {
    let Some(scene_view) = &blueprint.scene_view else {
        return Vec::new();
    };

    let mut spawned = Vec::new();
    let start_x = scene_view.center.x - i32::from(scene_view.width / 2);
    let start_y = scene_view.center.y - i32::from(scene_view.height / 2);

    for offset_y in 0..i32::from(scene_view.height) {
        for offset_x in 0..i32::from(scene_view.width) {
            let x = start_x + offset_x;
            let y = start_y + offset_y;
            let terrain = terrain_kind_at(x, y, &blueprint.terrain_patches);
            let variation = tile_variation(x, y);
            let mut translation = tile_to_world(x, y);
            translation.z = -0.75;

            let root = commands
                .spawn(Transform::from_translation(translation))
                .with_children(|parent| {
                    parent.spawn((
                        Sprite::from_color(
                            terrain.base_color(variation),
                            Vec2::splat(TILE_SIZE - 1.0),
                        ),
                        Transform::from_xyz(0.0, 0.0, 0.0),
                    ));
                    parent.spawn((
                        Sprite::from_color(
                            terrain.accent_color(variation),
                            terrain.accent_size(variation),
                        ),
                        Transform::from_xyz(
                            terrain.accent_offset(variation).x,
                            terrain.accent_offset(variation).y,
                            0.05,
                        ),
                    ));
                    parent.spawn((
                        Sprite::from_color(
                            Color::srgba(0.02, 0.01, 0.01, 0.05),
                            Vec2::new(TILE_SIZE - 2.0, 1.5),
                        ),
                        Transform::from_xyz(0.0, -(TILE_SIZE * 0.5) + 2.0, 0.08),
                    ));
                })
                .id();
            spawned.push(root);
        }
    }

    for decor in &blueprint.decor_objects {
        let mut translation = tile_to_world(decor.x, decor.y);
        translation.z = 0.3;

        let entity = commands
            .spawn(Transform::from_translation(translation))
            .with_children(|parent| {
                parent.spawn((
                    Sprite::from_color(Color::srgba(0.02, 0.02, 0.02, 0.10), Vec2::new(18.0, 8.0)),
                    Transform::from_xyz(0.0, -10.0, 0.0),
                ));

                match decor.kind {
                    DecorKind::Lantern => {
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.33, 0.24, 0.16), Vec2::new(4.0, 26.0)),
                            Transform::from_xyz(0.0, 8.0, 0.05),
                        ));
                        parent.spawn((
                            Sprite::from_color(
                                Color::srgb(0.95, 0.71, 0.28),
                                Vec2::new(10.0, 10.0),
                            ),
                            Transform::from_xyz(0.0, 18.0, 0.1),
                        ));
                        parent.spawn((
                            Sprite::from_color(
                                Color::srgba(1.0, 0.82, 0.34, 0.24),
                                Vec2::new(18.0, 18.0),
                            ),
                            Transform::from_xyz(0.0, 18.0, 0.08),
                        ));
                    }
                    DecorKind::Banner => {
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.28, 0.20, 0.15), Vec2::new(4.0, 30.0)),
                            Transform::from_xyz(0.0, 8.0, 0.05),
                        ));
                        parent.spawn((
                            Sprite::from_color(
                                Color::srgb(0.70, 0.21, 0.16),
                                Vec2::new(15.0, 18.0),
                            ),
                            Transform::from_xyz(8.0, 16.0, 0.1),
                        ));
                    }
                    DecorKind::Tree => {
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.31, 0.20, 0.12), Vec2::new(8.0, 18.0)),
                            Transform::from_xyz(0.0, 4.0, 0.05),
                        ));
                        parent.spawn((
                            Sprite::from_color(
                                Color::srgb(0.18, 0.40, 0.15),
                                Vec2::new(28.0, 22.0),
                            ),
                            Transform::from_xyz(0.0, 20.0, 0.1),
                        ));
                    }
                    DecorKind::Rock => {
                        parent.spawn((
                            Sprite::from_color(
                                Color::srgb(0.46, 0.43, 0.38),
                                Vec2::new(22.0, 14.0),
                            ),
                            Transform::from_xyz(0.0, 2.0, 0.08),
                        ));
                    }
                    DecorKind::Campfire => {
                        parent.spawn((
                            Sprite::from_color(Color::srgb(0.33, 0.21, 0.12), Vec2::new(18.0, 6.0)),
                            Transform::from_xyz(0.0, -2.0, 0.05),
                        ));
                        parent.spawn((
                            Sprite::from_color(
                                Color::srgb(0.96, 0.57, 0.19),
                                Vec2::new(10.0, 14.0),
                            ),
                            Transform::from_xyz(0.0, 8.0, 0.1),
                        ));
                        parent.spawn((
                            Sprite::from_color(
                                Color::srgba(1.0, 0.67, 0.24, 0.22),
                                Vec2::new(26.0, 22.0),
                            ),
                            Transform::from_xyz(0.0, 6.0, 0.08),
                        ));
                    }
                    DecorKind::Stump => {
                        parent.spawn((
                            Sprite::from_color(
                                Color::srgb(0.42, 0.29, 0.17),
                                Vec2::new(14.0, 10.0),
                            ),
                            Transform::from_xyz(0.0, 0.0, 0.08),
                        ));
                    }
                }
            })
            .id();
        spawned.push(entity);
    }

    spawned
}

fn update_entity_visuals(
    handles: SceneEntityHandles,
    entity_data: &WorldEntity,
    is_selected: bool,
    transform_query: &mut Query<&mut Transform>,
    sprite_query: &mut Query<&mut Sprite>,
) {
    let body_size = entity_data.body_size();
    let facing_offset = direction_to_world_offset(entity_data.direction.as_deref(), body_size);

    if let Ok(mut sprite) = sprite_query.get_mut(handles.body) {
        sprite.color = entity_data.kind.color();
        sprite.custom_size = Some(body_size);
    }

    if let Ok(mut transform) = transform_query.get_mut(handles.body) {
        transform.translation = Vec3::new(0.0, body_size.y * 0.2 - 2.0, 1.0);
    }

    if let Ok(mut sprite) = sprite_query.get_mut(handles.shadow) {
        sprite.custom_size = Some(Vec2::new(body_size.x + 10.0, 10.0));
    }

    if let Ok(mut sprite) = sprite_query.get_mut(handles.selection) {
        sprite.color = if is_selected {
            entity_data.kind.selection_color()
        } else {
            Color::NONE
        };
        sprite.custom_size = Some(body_size + Vec2::new(18.0, 12.0));
    }

    if let Ok(mut sprite) = sprite_query.get_mut(handles.crest) {
        sprite.color = Color::NONE;
        sprite.custom_size = Some(entity_data.crest_size(body_size));
    }

    if let Ok(mut transform) = transform_query.get_mut(handles.crest) {
        transform.translation = Vec3::new(0.0, body_size.y * 0.45, 2.0);
    }

    if let Ok(mut sprite) = sprite_query.get_mut(handles.facing) {
        sprite.color = Color::NONE;
        sprite.custom_size = Some(entity_data.kind.facing_size());
    }

    if let Ok(mut transform) = transform_query.get_mut(handles.facing) {
        transform.translation = Vec3::new(facing_offset.x, facing_offset.y + 4.0, 3.0);
    }
}

fn direction_to_world_offset(direction: Option<&str>, body_size: Vec2) -> Vec2 {
    let vector = match direction.unwrap_or("Down") {
        "Up" => Vec2::new(0.0, 1.0),
        "UpRight" => Vec2::new(1.0, 1.0).normalize(),
        "Right" => Vec2::new(1.0, 0.0),
        "DownRight" => Vec2::new(1.0, -1.0).normalize(),
        "Down" => Vec2::new(0.0, -1.0),
        "DownLeft" => Vec2::new(-1.0, -1.0).normalize(),
        "Left" => Vec2::new(-1.0, 0.0),
        "UpLeft" => Vec2::new(-1.0, 1.0).normalize(),
        _ => Vec2::new(0.0, -1.0),
    };

    Vec2::new(
        vector.x * (body_size.x * 0.5 + 7.0),
        vector.y * (body_size.y * 0.5 + 7.0),
    )
}

impl EntityKind {
    fn color(self) -> Color {
        match self {
            Self::SelfPlayer => Color::srgb(0.74, 0.58, 0.28),
            Self::Player => Color::srgb(0.60, 0.55, 0.42),
            Self::Monster => Color::srgb(0.55, 0.27, 0.18),
            Self::Npc => Color::srgb(0.48, 0.57, 0.33),
        }
    }

    fn size(self) -> Vec2 {
        match self {
            Self::SelfPlayer | Self::Player => Vec2::new(24.0, 32.0),
            Self::Monster => Vec2::new(28.0, 28.0),
            Self::Npc => Vec2::new(20.0, 30.0),
        }
    }

    fn selection_color(self) -> Color {
        match self {
            Self::SelfPlayer => Color::srgba(0.96, 0.75, 0.26, 0.22),
            Self::Player => Color::srgba(0.34, 0.67, 0.91, 0.18),
            Self::Monster => Color::srgba(0.91, 0.35, 0.24, 0.18),
            Self::Npc => Color::srgba(0.44, 0.78, 0.47, 0.18),
        }
    }

    fn facing_color(self) -> Color {
        match self {
            Self::SelfPlayer => Color::srgb(1.0, 0.92, 0.74),
            Self::Player => Color::srgb(0.84, 0.93, 1.0),
            Self::Monster => Color::srgb(1.0, 0.82, 0.74),
            Self::Npc => Color::srgb(0.89, 1.0, 0.87),
        }
    }

    fn facing_size(self) -> Vec2 {
        match self {
            Self::SelfPlayer | Self::Player => Vec2::new(8.0, 8.0),
            Self::Monster => Vec2::new(10.0, 10.0),
            Self::Npc => Vec2::new(7.0, 7.0),
        }
    }

    fn accent_color(self, name: &str) -> Color {
        let variant = name_seed(name) % 3;
        match (self, variant) {
            (Self::SelfPlayer, 0) => Color::srgb(1.0, 0.92, 0.74),
            (Self::SelfPlayer, 1) => Color::srgb(0.95, 0.83, 0.58),
            (Self::SelfPlayer, _) => Color::srgb(0.98, 0.87, 0.70),
            (Self::Player, 0) => Color::srgb(0.78, 0.92, 1.0),
            (Self::Player, 1) => Color::srgb(0.67, 0.84, 0.98),
            (Self::Player, _) => Color::srgb(0.58, 0.77, 0.94),
            (Self::Monster, 0) => Color::srgb(1.0, 0.70, 0.46),
            (Self::Monster, 1) => Color::srgb(0.94, 0.59, 0.34),
            (Self::Monster, _) => Color::srgb(0.91, 0.48, 0.29),
            (Self::Npc, 0) => Color::srgb(0.87, 1.0, 0.76),
            (Self::Npc, 1) => Color::srgb(0.72, 0.95, 0.66),
            (Self::Npc, _) => Color::srgb(0.63, 0.88, 0.58),
        }
    }
}

impl WorldEntity {
    fn body_size(&self) -> Vec2 {
        let mut size = self.kind.size();
        let level_bonus = f32::from(self.level.unwrap_or(1).min(30)) * 0.12;
        size.x += level_bonus;
        size.y += level_bonus * 1.8;
        size
    }

    fn crest_size(&self, body_size: Vec2) -> Vec2 {
        match self.kind {
            EntityKind::SelfPlayer | EntityKind::Player => Vec2::new(
                body_size.x * 0.45,
                8.0 + f32::from(self.level.unwrap_or(1).min(20)) * 0.15,
            ),
            EntityKind::Monster => Vec2::new(body_size.x * 0.6, body_size.y * 0.18),
            EntityKind::Npc => Vec2::new(body_size.x * 0.4, body_size.y * 0.18),
        }
    }
}

impl MapSceneBlueprint {
    fn from_snapshot(snapshot: &WorldSnapshot) -> Self {
        Self {
            map_title: snapshot.map_title.clone(),
            scene_view: snapshot.scene_view.clone(),
            terrain_patches: snapshot.terrain_patches.clone(),
            decor_objects: snapshot.decor_objects.clone(),
        }
    }
}

impl TerrainKind {
    fn base_color(self, variation: u32) -> Color {
        match self {
            Self::Grass => match variation % 3 {
                0 => Color::srgba(0.16, 0.24, 0.12, 0.28),
                1 => Color::srgba(0.18, 0.27, 0.14, 0.28),
                _ => Color::srgba(0.14, 0.22, 0.11, 0.28),
            },
            Self::Dirt => match variation % 3 {
                0 => Color::srgba(0.34, 0.22, 0.13, 0.30),
                1 => Color::srgba(0.30, 0.20, 0.11, 0.30),
                _ => Color::srgba(0.36, 0.24, 0.15, 0.30),
            },
            Self::Road => match variation % 3 {
                0 => Color::srgba(0.45, 0.35, 0.23, 0.30),
                1 => Color::srgba(0.40, 0.31, 0.20, 0.30),
                _ => Color::srgba(0.48, 0.38, 0.26, 0.30),
            },
            Self::Water => match variation % 3 {
                0 => Color::srgba(0.09, 0.25, 0.31, 0.26),
                1 => Color::srgba(0.11, 0.30, 0.37, 0.26),
                _ => Color::srgba(0.08, 0.22, 0.28, 0.26),
            },
            Self::Stone => match variation % 3 {
                0 => Color::srgba(0.42, 0.39, 0.34, 0.28),
                1 => Color::srgba(0.38, 0.35, 0.30, 0.28),
                _ => Color::srgba(0.46, 0.43, 0.37, 0.28),
            },
        }
    }

    fn accent_color(self, variation: u32) -> Color {
        match self {
            Self::Grass => match variation % 3 {
                0 => Color::srgba(0.26, 0.40, 0.20, 0.18),
                1 => Color::srgba(0.22, 0.34, 0.18, 0.18),
                _ => Color::srgba(0.29, 0.43, 0.24, 0.18),
            },
            Self::Dirt => match variation % 3 {
                0 => Color::srgba(0.46, 0.30, 0.18, 0.20),
                1 => Color::srgba(0.40, 0.27, 0.15, 0.20),
                _ => Color::srgba(0.52, 0.34, 0.20, 0.20),
            },
            Self::Road => match variation % 3 {
                0 => Color::srgba(0.56, 0.46, 0.31, 0.22),
                1 => Color::srgba(0.50, 0.40, 0.28, 0.22),
                _ => Color::srgba(0.60, 0.49, 0.34, 0.22),
            },
            Self::Water => match variation % 3 {
                0 => Color::srgba(0.20, 0.48, 0.58, 0.20),
                1 => Color::srgba(0.24, 0.56, 0.66, 0.20),
                _ => Color::srgba(0.16, 0.42, 0.54, 0.20),
            },
            Self::Stone => match variation % 3 {
                0 => Color::srgba(0.56, 0.54, 0.48, 0.18),
                1 => Color::srgba(0.50, 0.48, 0.43, 0.18),
                _ => Color::srgba(0.62, 0.59, 0.53, 0.18),
            },
        }
    }

    fn accent_size(self, variation: u32) -> Vec2 {
        match self {
            Self::Grass => {
                if variation % 2 == 0 {
                    Vec2::new(10.0, 12.0)
                } else {
                    Vec2::new(14.0, 8.0)
                }
            }
            Self::Dirt => Vec2::new(16.0, 10.0),
            Self::Road => Vec2::new(18.0, 6.0),
            Self::Water => Vec2::new(20.0, 10.0),
            Self::Stone => Vec2::new(16.0, 12.0),
        }
    }

    fn accent_offset(self, variation: u32) -> Vec2 {
        match self {
            Self::Grass => match variation % 3 {
                0 => Vec2::new(-8.0, 7.0),
                1 => Vec2::new(7.0, -5.0),
                _ => Vec2::new(3.0, 8.0),
            },
            Self::Dirt => Vec2::new(-4.0, 5.0),
            Self::Road => Vec2::new(0.0, -7.0),
            Self::Water => Vec2::new(5.0, 6.0),
            Self::Stone => Vec2::new(-5.0, -3.0),
        }
    }
}

fn terrain_kind_at(x: i32, y: i32, patches: &[TerrainPatch]) -> TerrainKind {
    patches
        .iter()
        .rev()
        .find(|patch| patch_contains(patch, x, y))
        .map(|patch| patch.kind)
        .unwrap_or(TerrainKind::Grass)
}

fn patch_contains(patch: &TerrainPatch, x: i32, y: i32) -> bool {
    x >= patch.x
        && x < patch.x + i32::from(patch.width)
        && y >= patch.y
        && y < patch.y + i32::from(patch.height)
}

fn tile_variation(x: i32, y: i32) -> u32 {
    let ux = u32::from_ne_bytes(x.to_ne_bytes());
    let uy = u32::from_ne_bytes(y.to_ne_bytes());
    (ux.wrapping_mul(31)).wrapping_add(uy.wrapping_mul(17))
}

fn name_seed(name: &str) -> u8 {
    name.bytes().fold(0u8, |acc, byte| acc.wrapping_add(byte))
}

fn publish_status(phase: &str, message: &str) {
    STATUS_SINK.with(|sink| {
        let callback_ref = sink.borrow();
        let Some(callback) = callback_ref.as_ref() else {
            return;
        };

        let payload = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("phase"),
            &JsValue::from_str(phase),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("message"),
            &JsValue::from_str(message),
        );
        let _ = callback.call1(&JsValue::NULL, &payload.into());
    });
}

fn publish_map_status(phase: &str, message: &str, ack_key: &str, image_keys: &[String]) {
    STATUS_SINK.with(|sink| {
        let callback_ref = sink.borrow();
        let Some(callback) = callback_ref.as_ref() else {
            return;
        };

        let payload = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("phase"),
            &JsValue::from_str(phase),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("message"),
            &JsValue::from_str(message),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("ackKey"),
            &JsValue::from_str(ack_key),
        );
        let keys = js_sys::Array::new();
        for key in image_keys {
            keys.push(&JsValue::from_str(key));
        }
        let _ = js_sys::Reflect::set(&payload, &JsValue::from_str("imageKeys"), &keys);
        let _ = callback.call1(&JsValue::NULL, &payload.into());
    });
}

#[cfg(test)]
mod entity_atlas_tests {
    use super::*;

    fn entity_sync_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Image>()
            .init_asset::<Mesh>()
            .init_asset::<TextureAtlasLayout>()
            .init_asset::<additive_material::CrystalAdditiveMaterial>()
            .init_resource::<RuntimeEntityRenderState>()
            .init_resource::<RuntimeEntityRenderAtlases>()
            .init_resource::<motion::EntityMotionTable>()
            .init_resource::<local_motion::LocalMotionPresentationShadow>()
            .init_resource::<remote_motion::RemoteMotionPresentation>()
            .init_resource::<presentation_pose::PresentationPoseBuffer>()
            .init_resource::<SceneRegistry>()
            .init_resource::<additive_material::CrystalAdditiveMaterialCache>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<TextureAtlasLayout>>()
            .init_resource::<Assets<additive_material::CrystalAdditiveMaterial>>()
            .add_systems(Update, sync_entity_render_layers);
        app
    }

    fn rect(key: &str) -> EntityRenderAtlasRect {
        EntityRenderAtlasRect {
            key: key.to_string(),
            x: 0,
            y: 0,
            width: 32,
            height: 48,
        }
    }

    #[test]
    fn cached_layout_must_cover_new_transient_action_rects() {
        let existing = HashMap::from([("standing".to_string(), 0)]);
        assert!(entity_render_atlas_contains_rects(
            &existing,
            &[rect("standing")]
        ));
        assert!(!entity_render_atlas_contains_rects(
            &existing,
            &[rect("standing"), rect("attack")]
        ));
    }

    #[test]
    fn additive_entity_layer_keeps_exact_atlas_uv_subrect() {
        let state: EntityRenderState = serde_json::from_str(
            r#"{
                "enabled":true,
                "stageWidth":1024,
                "stageHeight":768,
                "atlases":[{
                    "key":"starter:p6",
                    "width":100,
                    "height":200,
                    "imageUrl":"/bevy-entity-atlases/starter-p6.png",
                    "rects":[{
                        "key":"effect",
                        "x":10,
                        "y":20,
                        "width":30,
                        "height":40
                    }]
                }],
                "entities":[{
                    "objectId":"2005",
                    "layers":[{
                        "key":"2005:scarecrow-die-effect",
                        "path":"/original-ui/Monster/005/227.png",
                        "atlasKey":"starter:p6",
                        "atlasRectKey":"effect",
                        "left":0,
                        "top":0,
                        "width":76,
                        "height":72,
                        "z":1,
                        "additive":true
                    }]
                }]
            }"#,
        )
        .expect("additive entity state");
        assert!(state.entities[0].layers[0].additive);

        let uv = entity_atlas_uv_scale_offset(
            UVec2::new(2048, 1024),
            URect {
                min: UVec2::new(512, 256),
                max: UVec2::new(768, 384),
            },
        );
        assert_eq!(uv, Vec4::new(0.125, 0.125, 0.25, 0.25));
    }

    #[test]
    fn additive_entity_layer_uses_material_mesh_and_evicts_on_removal() {
        let mut app = entity_sync_test_app();
        let colliding_effect_key = "2005:scarecrow-die-effect";
        let colliding_effect_image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        app.world_mut().resource_scope(
            |world, mut cache: Mut<additive_material::CrystalAdditiveMaterialCache>| {
                let mut materials =
                    world.resource_mut::<Assets<additive_material::CrystalAdditiveMaterial>>();
                cache.material(
                    colliding_effect_key,
                    colliding_effect_image,
                    1.0,
                    &mut materials,
                );
            },
        );
        let snapshot: EntityRenderState = serde_json::from_str(
            r#"{
                "enabled":true,
                "stageWidth":1024,
                "stageHeight":768,
                "atlases":[{
                    "key":"starter:p6",
                    "width":100,
                    "height":200,
                    "imageUrl":"/bevy-entity-atlases/starter-p6.png",
                    "rects":[{
                        "key":"effect",
                        "x":10,
                        "y":20,
                        "width":30,
                        "height":40
                    }]
                }],
                "entities":[{
                    "objectId":"2005",
                    "layers":[{
                        "key":"2005:scarecrow-die-effect",
                        "path":"/original-ui/Monster/005/227.png",
                        "atlasKey":"starter:p6",
                        "atlasRectKey":"effect",
                        "left":480,
                        "top":352,
                        "width":76,
                        "height":72,
                        "z":50008,
                        "additive":true
                    }]
                }]
            }"#,
        )
        .expect("additive entity snapshot");
        app.world_mut()
            .resource_mut::<RuntimeEntityRenderState>()
            .snapshot = Some(snapshot);
        app.update();

        let entity = app.world().resource::<SceneRegistry>().entity_render_layers
            ["2005:scarecrow-die-effect"]
            .entity;
        assert!(app
            .world()
            .get::<MeshMaterial2d<additive_material::CrystalAdditiveMaterial>>(entity)
            .is_some());
        assert!(app.world().get::<Sprite>(entity).is_none());
        let material_handle = app
            .world()
            .get::<MeshMaterial2d<additive_material::CrystalAdditiveMaterial>>(entity)
            .expect("additive binding")
            .0
            .clone();
        let material = app
            .world()
            .resource::<Assets<additive_material::CrystalAdditiveMaterial>>()
            .get(&material_handle)
            .expect("additive atlas material");
        assert_eq!(material.uv_scale_offset(), Vec4::new(0.3, 0.2, 0.1, 0.1));
        assert_eq!(
            app.world()
                .resource::<additive_material::CrystalAdditiveMaterialCache>()
                .len(),
            2,
            "standalone effect and namespaced entity material coexist"
        );

        app.world_mut()
            .resource_mut::<RuntimeEntityRenderState>()
            .snapshot = Some(
            serde_json::from_str(
                r#"{"enabled":true,"stageWidth":1024,"stageHeight":768,"entities":[]}"#,
            )
            .unwrap(),
        );
        app.update();
        assert!(app
            .world()
            .resource::<SceneRegistry>()
            .entity_render_layers
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<additive_material::CrystalAdditiveMaterialCache>()
                .len(),
            1,
            "removing the entity cannot evict a standalone effect with the same raw key"
        );
        assert!(!app.world().entities().contains(entity));
    }

    #[test]
    fn entity_layer_rebuilds_between_sprite_and_additive_mesh_with_same_stable_key() {
        let snapshot = |additive: bool| -> EntityRenderState {
            serde_json::from_value(serde_json::json!({
                "enabled": true,
                "stageWidth": 1024,
                "stageHeight": 768,
                "atlases": [{
                    "key": "starter:p6",
                    "width": 100,
                    "height": 200,
                    "imageUrl": "/bevy-entity-atlases/starter-p6.png",
                    "rects": [{
                        "key": "effect",
                        "x": 10,
                        "y": 20,
                        "width": 30,
                        "height": 40
                    }]
                }],
                "entities": [{
                    "objectId": "2005",
                    "layers": [{
                        "key": "stable-layer",
                        "path": "/original-ui/Monster/005/227.png",
                        "atlasKey": "starter:p6",
                        "atlasRectKey": "effect",
                        "left": 480,
                        "top": 352,
                        "width": 76,
                        "height": 72,
                        "z": 50008,
                        "additive": additive
                    }]
                }]
            }))
            .expect("entity layer mode snapshot")
        };

        let mut app = entity_sync_test_app();
        app.world_mut()
            .resource_mut::<RuntimeEntityRenderState>()
            .snapshot = Some(snapshot(false));
        app.update();
        let sprite_entity =
            app.world().resource::<SceneRegistry>().entity_render_layers["stable-layer"].entity;
        assert!(app.world().get::<Sprite>(sprite_entity).is_some());

        app.world_mut()
            .resource_mut::<RuntimeEntityRenderState>()
            .snapshot = Some(snapshot(true));
        app.update();
        let additive_entity =
            app.world().resource::<SceneRegistry>().entity_render_layers["stable-layer"].entity;
        assert_ne!(additive_entity, sprite_entity);
        assert!(!app.world().entities().contains(sprite_entity));
        assert!(app
            .world()
            .get::<MeshMaterial2d<additive_material::CrystalAdditiveMaterial>>(additive_entity)
            .is_some());
        assert_eq!(
            app.world()
                .resource::<additive_material::CrystalAdditiveMaterialCache>()
                .len(),
            1
        );

        app.world_mut()
            .resource_mut::<RuntimeEntityRenderState>()
            .snapshot = Some(snapshot(false));
        app.update();
        let restored_sprite =
            app.world().resource::<SceneRegistry>().entity_render_layers["stable-layer"].entity;
        assert_ne!(restored_sprite, additive_entity);
        assert!(!app.world().entities().contains(additive_entity));
        assert!(app.world().get::<Sprite>(restored_sprite).is_some());
        assert_eq!(
            app.world()
                .resource::<additive_material::CrystalAdditiveMaterialCache>()
                .len(),
            0
        );
    }

    #[test]
    fn map_render_state_retains_animation_family_keys_without_drawing_them() {
        let retained: MapRenderState = serde_json::from_str(
            r#"{
                "enabled": true,
                "stageWidth": 1024,
                "stageHeight": 768,
                "atlases": [{"key":"map:atlas","width":512,"height":512}],
                "standaloneTiles": [{
                    "key":"effect:draw",
                    "imageKey":"effect:image",
                    "left":0,
                    "top":0,
                    "width":32,
                    "height":48,
                    "z":1
                }],
                "retainedImageKeys": ["player:standing", "player:walking"]
            }"#,
        )
        .expect("retainedImageKeys should deserialize");
        assert_eq!(
            retained.retained_image_keys,
            ["player:standing", "player:walking"]
        );
        assert_eq!(
            map_render_active_image_keys(&retained),
            HashSet::from([
                "map:atlas".to_string(),
                "effect:image".to_string(),
                "player:standing".to_string(),
                "player:walking".to_string(),
            ])
        );
        assert_eq!(retained.standalone_tiles.len(), 1);

        let defaulted: MapRenderState =
            serde_json::from_str(r#"{"enabled":true,"stageWidth":1024,"stageHeight":768}"#)
                .expect("retainedImageKeys should be optional");
        assert!(defaulted.retained_image_keys.is_empty());
    }

    #[test]
    fn map_render_state_accepts_standalone_image_urls() {
        let state: MapRenderState = serde_json::from_str(
            r#"{
                "enabled": true,
                "stageWidth": 1024,
                "stageHeight": 768,
                "standaloneTiles": [{
                    "key":"standalone:tree",
                    "imageKey":"standalone:WemadeMir2/Objects#7112",
                    "imageUrl":"/generated/native-map-keyed/pages/hash.png",
                    "left":10,
                    "top":20,
                    "width":64,
                    "height":96,
                    "z":1
                }]
            }"#,
        )
        .expect("standalone imageUrl should deserialize");
        assert_eq!(state.standalone_tiles.len(), 1);
        assert_eq!(
            state.standalone_tiles[0].image_url.as_deref(),
            Some("/generated/native-map-keyed/pages/hash.png")
        );
        assert_eq!(
            map_render_url_image_sources(&state),
            vec![(
                "standalone:WemadeMir2/Objects#7112".to_owned(),
                "generated/native-map-keyed/pages/hash.png".to_owned()
            )]
        );
    }

    #[test]
    fn map_render_url_sources_include_atlases_and_standalone_tiles() {
        let state: MapRenderState = serde_json::from_str(
            r#"{
                "enabled": true,
                "stageWidth": 1024,
                "stageHeight": 768,
                "atlases": [{
                    "key": "map:page",
                    "width": 512,
                    "height": 512,
                    "imageUrl": "/generated/map-atlas/page.png?rev=1"
                }],
                "standaloneTiles": [{
                    "key": "standalone:tree",
                    "imageKey": "standalone:tree:image",
                    "imageUrl": "/generated/native-map-keyed/tree.png",
                    "left": 0,
                    "top": 0,
                    "width": 64,
                    "height": 96,
                    "z": 1
                }]
            }"#,
        )
        .expect("URL-backed map state should deserialize");

        assert_eq!(
            map_render_url_image_sources(&state),
            vec![
                (
                    "map:page".to_owned(),
                    "generated/map-atlas/page.png".to_owned()
                ),
                (
                    "standalone:tree:image".to_owned(),
                    "generated/native-map-keyed/tree.png".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn map_render_binding_coverage_reports_missing_rects() {
        let state: MapRenderState = serde_json::from_str(
            r#"{
                "enabled": true,
                "stageWidth": 1024,
                "stageHeight": 768,
                "tiles": [
                    {"key":"ok","atlasKey":"map:page","rectKey":"tiles#1","left":0,"top":0,"width":96,"height":64,"z":0},
                    {"key":"hole","atlasKey":"map:page","rectKey":"tiles#2","left":96,"top":0,"width":96,"height":64,"z":0}
                ]
            }"#,
        )
        .expect("map render state should deserialize");
        let atlases = RuntimeMapRenderAtlases {
            images: HashMap::from([("map:page".to_owned(), Handle::<Image>::default())]),
            layouts: HashMap::from([(
                "map:page".to_owned(),
                (
                    Handle::<TextureAtlasLayout>::default(),
                    HashMap::from([("tiles#1".to_owned(), 0)]),
                ),
            )]),
            ..default()
        };

        assert_eq!(
            map_render_missing_bindings(&state, &atlases),
            ["hole:map:page#tiles#2:rect"]
        );
    }

    #[test]
    fn map_atlas_layout_accumulates_rects_across_viewports() {
        let atlas = |rect_key: &str, width: u32| MapRenderAtlas {
            key: "map:page".to_owned(),
            width,
            height: 512,
            image_url: None,
            rects: vec![MapRenderAtlasRect {
                key: rect_key.to_owned(),
                x: if rect_key == "tiles#1" { 0 } else { 96 },
                y: 0,
                width: 96,
                height: 64,
            }],
        };
        let mut assets = RuntimeMapRenderAtlases::default();

        let (size, first) = merge_map_render_atlas_rects(&atlas("tiles#1", 512), &mut assets)
            .expect("first viewport creates a layout");
        assets.layout_sizes.insert("map:page".to_owned(), size);
        assert_eq!(first.len(), 1);

        let (_, second) = merge_map_render_atlas_rects(&atlas("tiles#2", 512), &mut assets)
            .expect("a later viewport grows the stable page layout");
        assert_eq!(second.len(), 2);
        assert!(second.contains_key("tiles#1"));
        assert!(second.contains_key("tiles#2"));
        assert!(merge_map_render_atlas_rects(&atlas("tiles#2", 512), &mut assets).is_none());

        let (_, resized) = merge_map_render_atlas_rects(&atlas("tiles#2", 1024), &mut assets)
            .expect("page dimension changes rebuild from a clean rect set");
        assert_eq!(resized.len(), 1);
        assert!(!resized.contains_key("tiles#1"));
    }

    #[test]
    fn effect_render_state_deserializes_entries() {
        let state: EffectRenderState = serde_json::from_str(
            r#"{
                "enabled": true,
                "stageWidth": 1024,
                "stageHeight": 768,
                "effects": [{
                    "key": "fx-cast-1",
                    "imageUrl": "/original-effects/Magic/0.png",
                    "maskImageUrl": "/original-effects/Magic/0.png",
                    "left": 10.0,
                    "top": 20.0,
                    "width": 44.0,
                    "height": 75.0,
                    "z": 9.0,
                    "additive": true,
                    "opacity": 0.7
                }]
            }"#,
        )
        .expect("effect render state should deserialize");
        assert_eq!(state.effects.len(), 1);
        let entry = &state.effects[0];
        assert_eq!(entry.key, "fx-cast-1");
        assert!(entry.additive);
        assert_eq!(entry.opacity, Some(0.7));
        assert_eq!(
            entry.image_url.as_deref(),
            Some("/original-effects/Magic/0.png")
        );
        assert_eq!(
            entry.mask_image_url.as_deref(),
            Some("/original-effects/Magic/0.png")
        );
        assert!(effect_render_active_image_keys(&state).contains("original-effects/Magic/0.png"));
    }

    #[test]
    fn effect_render_position_matches_entity_layer_contract() {
        let state: EffectRenderState = serde_json::from_str(
            r#"{
                "enabled": true,
                "stageWidth": 1024,
                "stageHeight": 768,
                "effects": [{
                    "key": "ground-1",
                    "imageUrl": "/a.png",
                    "left": 488.0,
                    "top": 360.0,
                    "width": 48.0,
                    "height": 48.0,
                    "z": 4.8,
                    "additive": true
                }]
            }"#,
        )
        .expect("deserialize");
        let entry = &state.effects[0];
        let pos = effect_render_layer_position(&state, entry);
        // A screen-stage centre rect maps to the world origin (centred, Y-flip).
        assert!((pos.x - 0.0).abs() < 0.001);
        assert!((pos.y - 0.0).abs() < 0.001);
        assert_eq!(pos.z, 4.8 / 100_000.0);
    }
}

#[cfg(test)]
mod effect_mask_shadow_tests {
    use super::*;

    fn effect_state_with_mask(mask: Option<&str>, shadow: Option<(f32, f32)>) -> EffectRenderState {
        EffectRenderState {
            enabled: true,
            stage_width: 1024.0,
            stage_height: 768.0,
            effects: vec![EffectRenderEntry {
                key: "fx-1".to_owned(),
                image_url: Some("/original-effects/Magic/0.png".to_owned()),
                left: 480.0,
                top: 352.0,
                width: 48.0,
                height: 48.0,
                z: 9.0,
                additive: true,
                opacity: None,
                mask_image_url: mask.map(|s| s.to_owned()),
                mask_width: mask.map(|_| 32.0),
                mask_height: mask.map(|_| 32.0),
                mask_x: mask.map(|_| 470.0),
                mask_y: mask.map(|_| 340.0),
                frame_x: mask.map(|_| 480.0),
                frame_y: mask.map(|_| 352.0),
                shadow_x: shadow.map(|(x, _)| x),
                shadow_y: shadow.map(|(_, y)| y),
            }],
        }
    }

    #[test]
    fn frame_with_mask_emits_mask_render_state() {
        let state = effect_state_with_mask(Some("/original-effects/Magic/0.png"), None);
        assert!(state.effects[0].mask_image_url.is_some());
        let keys = effect_render_active_image_keys(&state);
        assert!(keys.contains("original-effects/Magic/0.png"));
    }

    #[test]
    fn missing_mask_falls_back_without_placeholder() {
        let state = effect_state_with_mask(None, None);
        assert!(state.effects[0].mask_image_url.is_none());
        let keys = effect_render_active_image_keys(&state);
        assert_eq!(keys.len(), 1);
    }

    #[test]
    fn effect_shadow_uses_manifest_offsets() {
        let state = effect_state_with_mask(None, Some((5.0, -3.0)));
        let entry = &state.effects[0];
        assert_eq!(entry.shadow_x, Some(5.0));
        assert_eq!(entry.shadow_y, Some(-3.0));
        let pos = effect_render_layer_position(&state, entry);
        let shadow_left = entry.left + 5.0;
        let shadow_top = entry.top - 3.0;
        let shadow_center_x = shadow_left + entry.width * 0.5;
        let shadow_center_y = shadow_top + entry.height * 0.5;
        let shadow_pos = Vec3::new(
            shadow_center_x - state.stage_width * 0.5,
            state.stage_height * 0.5 - shadow_center_y,
            entry.z / 100_000.0 - 0.0005,
        );
        assert!((shadow_pos.x - (pos.x + 5.0)).abs() < 0.01);
    }

    #[test]
    fn shadow_renders_below_primary_effect() {
        let state = effect_state_with_mask(None, Some((2.0, 2.0)));
        let entry = &state.effects[0];
        let primary_pos = effect_render_layer_position(&state, entry);
        let shadow_left = entry.left + 2.0;
        let shadow_top = entry.top + 2.0;
        let shadow_center_x = shadow_left + entry.width * 0.5;
        let shadow_center_y = shadow_top + entry.height * 0.5;
        let shadow_pos = Vec3::new(
            shadow_center_x - state.stage_width * 0.5,
            state.stage_height * 0.5 - shadow_center_y,
            entry.z / 100_000.0 - 0.0005,
        );
        assert!(shadow_pos.z < primary_pos.z);
    }

    #[test]
    fn additive_toggle_rebuilds_all_required_layers() {
        let mut cache = crate::additive_material::CrystalAdditiveMaterialCache::default();
        let mut materials = Assets::<crate::additive_material::CrystalAdditiveMaterial>::default();
        let mut images = Assets::<Image>::default();
        let img = images.add(Image::default());
        let key = "fx-1";
        let h1 = cache.material(key, img.clone(), 1.0, &mut materials);
        assert_eq!(cache.len(), 1);
        cache.evict(key, &mut materials);
        assert_eq!(cache.len(), 0);
        let h2 = cache.material(key, img, 1.0, &mut materials);
        assert_ne!(h1, h2);
    }

    fn sync_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Image>()
            .init_asset::<Mesh>()
            .init_asset::<ColorMaterial>()
            .init_asset::<crate::additive_material::CrystalAdditiveMaterial>()
            .init_asset::<crate::lighting::CrystalMultiplyMaterial>()
            .init_resource::<SceneRegistry>()
            .init_resource::<RuntimeEffectRenderState>()
            .init_resource::<RuntimeLightingRenderState>()
            .init_resource::<SceneResetRevision>()
            .init_resource::<RuntimeLightingSceneResetTracker>()
            .init_resource::<RuntimeEffectShadowCleanupTracker>()
            .init_resource::<crate::additive_material::CrystalAdditiveMaterialCache>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<ColorMaterial>>()
            .init_resource::<Assets<crate::additive_material::CrystalAdditiveMaterial>>()
            .init_resource::<Assets<crate::lighting::CrystalMultiplyMaterial>>()
            .add_systems(
                Update,
                (
                    apply_scene_reset_to_lighting,
                    sync_effect_render,
                    sync_lighting_render,
                    cleanup_reset_effect_shadows,
                )
                    .chain(),
            );
        app
    }

    #[test]
    fn sync_effect_render_spawns_primary_mask_and_shadow_and_cleans_up() {
        // Run the real ECS sync_effect_render system and verify the layer
        // lifecycle: spawn, update, despawn, and asset recycling.
        let mut app = sync_test_app();
        // A mask+shadow effect snapshot (geometry distinct from primary: the
        // producer sends real local values frame.x=-20, frame.y=0, maskX=3,
        // maskY=-7 in the SAME origin).
        let snapshot = EffectRenderState {
            enabled: true,
            stage_width: 1024.0,
            stage_height: 768.0,
            effects: vec![EffectRenderEntry {
                key: "fx-e2e".to_owned(),
                image_url: Some("/original-effects/Magic/0.png".to_owned()),
                left: 480.0,
                top: 352.0,
                width: 48.0,
                height: 48.0,
                z: 9.0,
                additive: true,
                opacity: Some(0.7),
                mask_image_url: Some("/original-effects/Magic/0.png".to_owned()),
                mask_width: Some(32.0),
                mask_height: Some(32.0),
                mask_x: Some(3.0),
                mask_y: Some(-7.0),
                frame_x: Some(-20.0),
                frame_y: Some(0.0),
                shadow_x: Some(4.0),
                shadow_y: Some(0.0),
            }],
        };
        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = Some(snapshot);
        app.update();

        let registry = app.world().resource::<SceneRegistry>();
        assert_eq!(registry.effect_render.len(), 1, "primary sprite spawned");
        assert!(registry.effect_render.contains_key("fx-e2e"));
        assert_eq!(registry.effect_render_masks.len(), 1, "mask spawned");
        assert!(registry.effect_render_masks.contains_key("fx-e2e:mask"));

        // Mask transform: primary left=480 top=352, frame.x=-20 frame.y=0, maskX=3
        // maskY=-7 => mask dx=3-(-20)=23, dy=-7-0=-7. Mask left=503 top=345.
        // Center=(503+16,345+16)=(519,361) -> world=(519-512,384-361)=(7,23).
        let mask_entity = registry.effect_render_masks["fx-e2e:mask"].entity;
        let mask_transform = *app.world().get::<Transform>(mask_entity).unwrap();
        assert!(
            (mask_transform.translation.x - 7.0).abs() < 0.5,
            "mask x uses maskX-frameX"
        );
        assert!(
            (mask_transform.translation.y - 23.0).abs() < 0.5,
            "mask y uses maskY-frameY"
        );
        assert!(
            (mask_transform.scale.x - 32.0).abs() < 0.1,
            "mask uses mask width"
        );

        // (4, 0) is valid: a single axis may be zero, and the shadow remains
        // below the primary without changing the primary transform.
        assert_eq!(registry.effect_render_shadows.len(), 1, "shadow spawned");
        let shadow_entity = registry.effect_render_shadows["fx-e2e:shadow"].entity;
        let primary_entity = registry.effect_render["fx-e2e"].entity;
        let primary_binding = app
            .world()
            .get::<MeshMaterial2d<crate::additive_material::CrystalAdditiveMaterial>>(
                primary_entity,
            )
            .expect("primary additive material binding");
        let primary_material = app
            .world()
            .resource::<Assets<crate::additive_material::CrystalAdditiveMaterial>>()
            .get(&primary_binding.0)
            .expect("primary additive material");
        assert!((primary_material.opacity() - 0.7).abs() < f32::EPSILON);
        let mask_binding = app
            .world()
            .get::<MeshMaterial2d<crate::additive_material::CrystalAdditiveMaterial>>(mask_entity)
            .expect("mask additive material binding");
        let mask_material = app
            .world()
            .resource::<Assets<crate::additive_material::CrystalAdditiveMaterial>>()
            .get(&mask_binding.0)
            .expect("mask additive material");
        assert!((mask_material.opacity() - 0.7).abs() < f32::EPSILON);
        let primary_transform = *app.world().get::<Transform>(primary_entity).unwrap();
        let shadow_transform = *app.world().get::<Transform>(shadow_entity).unwrap();
        assert!(shadow_transform.translation.z < primary_transform.translation.z);
        assert!(
            (shadow_transform.translation.x - (primary_transform.translation.x + 4.0)).abs() < 0.5
        );
        assert_ne!(shadow_transform.scale, primary_transform.scale);
        assert_eq!(
            app.world().resource::<Assets<ColorMaterial>>().len(),
            1,
            "shadow owns one procedural material"
        );
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            2,
            "primary unit quad and shadow ellipse are allocated"
        );

        // Update the same effect (retained in place, no new entities).
        let prim_count = app.world().resource::<SceneRegistry>().effect_render.len();
        let mask_count = app
            .world()
            .resource::<SceneRegistry>()
            .effect_render_masks
            .len();
        let shadow_count = app
            .world()
            .resource::<SceneRegistry>()
            .effect_render_shadows
            .len();
        let shadow_entity_after_first_update = app
            .world()
            .resource::<SceneRegistry>()
            .effect_render_shadows["fx-e2e:shadow"]
            .entity;
        app.update();
        let registry_after = app.world().resource::<SceneRegistry>();
        assert_eq!(
            registry_after.effect_render.len(),
            prim_count,
            "no duplicate primary on update"
        );
        assert_eq!(
            registry_after.effect_render_masks.len(),
            mask_count,
            "no duplicate mask on update"
        );
        assert_eq!(
            registry_after.effect_render_shadows.len(),
            shadow_count,
            "no duplicate shadow on update"
        );
        assert_eq!(
            registry_after.effect_render_shadows["fx-e2e:shadow"].entity,
            shadow_entity_after_first_update,
            "shadow entity retained on update"
        );

        // Remove the effect; all layers and materials are cleaned up.
        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = Some(EffectRenderState {
            enabled: true,
            stage_width: 1024.0,
            stage_height: 768.0,
            effects: vec![],
        });
        app.update();
        let registry = app.world().resource::<SceneRegistry>();
        assert!(registry.effect_render.is_empty(), "primary despawned");
        assert!(registry.effect_render_masks.is_empty(), "mask despawned");
        assert!(
            registry.effect_render_shadows.is_empty(),
            "shadow despawned"
        );
        assert!(
            registry.effect_render_images.is_empty(),
            "effect images released"
        );
        let cache = app
            .world()
            .resource::<crate::additive_material::CrystalAdditiveMaterialCache>();
        assert_eq!(cache.len(), 0, "materials recycled");
        assert_eq!(
            app.world().resource::<Assets<ColorMaterial>>().len(),
            0,
            "shadow materials recycled"
        );
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            1,
            "shadow ellipse mesh recycled while primary unit quad remains"
        );
    }

    #[test]
    fn sync_effect_render_no_shadow_when_only_single_axis_missing() {
        // A missing axis must not create a shadow entity, even though the other
        // axis is present.
        let mut app = sync_test_app();
        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = Some(EffectRenderState {
            enabled: true,
            stage_width: 1024.0,
            stage_height: 768.0,
            effects: vec![EffectRenderEntry {
                key: "fx-noshadow".to_owned(),
                image_url: Some("/original-effects/Magic/0.png".to_owned()),
                left: 480.0,
                top: 352.0,
                width: 48.0,
                height: 48.0,
                z: 9.0,
                additive: true,
                opacity: None,
                mask_image_url: None,
                mask_width: None,
                mask_height: None,
                mask_x: None,
                mask_y: None,
                frame_x: None,
                frame_y: None,
                shadow_x: None,
                shadow_y: Some(5.0),
            }],
        });
        app.update();
        let registry = app.world().resource::<SceneRegistry>();
        // Producer never emits a partial shadow pair; runtime must not create one.
        assert!(
            registry.effect_render_shadows.is_empty(),
            "no shadow when pair incomplete"
        );
        assert_eq!(registry.effect_render.len(), 1, "primary still rendered");
    }

    #[test]
    fn sync_effect_render_shadow_zero_axes_still_spawn_and_update_in_place() {
        let mut app = sync_test_app();
        let shadow_state = |x, y| EffectRenderState {
            enabled: true,
            stage_width: 1024.0,
            stage_height: 768.0,
            effects: vec![EffectRenderEntry {
                key: "fx-zero-axis".to_owned(),
                image_url: Some("/original-effects/Magic/0.png".to_owned()),
                left: 480.0,
                top: 352.0,
                width: 48.0,
                height: 48.0,
                z: 9.0,
                additive: true,
                opacity: None,
                mask_image_url: None,
                mask_width: None,
                mask_height: None,
                mask_x: None,
                mask_y: None,
                frame_x: None,
                frame_y: None,
                shadow_x: Some(x),
                shadow_y: Some(y),
            }],
        };

        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = Some(shadow_state(0.0, -5.0));
        app.update();
        let first = app
            .world()
            .resource::<SceneRegistry>()
            .effect_render_shadows["fx-zero-axis:shadow"]
            .clone();
        let first_transform = *app.world().get::<Transform>(first.entity).unwrap();
        assert_eq!(app.world().resource::<Assets<ColorMaterial>>().len(), 1);

        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = Some(shadow_state(6.0, 0.0));
        app.update();
        let registry = app.world().resource::<SceneRegistry>();
        let updated = registry.effect_render_shadows["fx-zero-axis:shadow"].clone();
        assert_eq!(
            updated.entity, first.entity,
            "zero-axis update retained entity"
        );
        assert_eq!(
            updated.material, first.material,
            "zero-axis update retained material"
        );
        let updated_transform = *app.world().get::<Transform>(updated.entity).unwrap();
        assert_ne!(updated_transform.translation, first_transform.translation);
        assert_eq!(app.world().resource::<Assets<ColorMaterial>>().len(), 1);

        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = Some(EffectRenderState {
            enabled: true,
            stage_width: 1024.0,
            stage_height: 768.0,
            effects: vec![],
        });
        app.update();
        assert!(app
            .world()
            .resource::<SceneRegistry>()
            .effect_render_shadows
            .is_empty());
        assert_eq!(app.world().resource::<Assets<ColorMaterial>>().len(), 0);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    }

    #[test]
    fn sync_effect_render_scene_reset_recycles_shadow_assets() {
        let mut app = sync_test_app();
        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = Some(effect_state_with_mask(None, Some((0.0, 0.0))));
        app.update();
        assert_eq!(
            app.world()
                .resource::<SceneRegistry>()
                .effect_render_shadows
                .len(),
            1
        );
        assert_eq!(app.world().resource::<Assets<ColorMaterial>>().len(), 1);

        // MapChanged and LogOut both advance the scene reset revision in the
        // native host. Clearing the snapshot before the revision models the
        // same boundary and prevents the renderer from recreating the layer.
        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = None;
        app.world_mut().resource_mut::<SceneResetRevision>().0 = 1;
        app.update();

        assert!(app
            .world()
            .resource::<SceneRegistry>()
            .effect_render_shadows
            .is_empty());
        assert_eq!(app.world().resource::<Assets<ColorMaterial>>().len(), 0);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    }

    #[test]
    fn sync_effect_render_mask_geometry_follows_nonzero_to_zero() {
        // A mask with real local values that resolve to a zero delta
        // (frame.x=-20, maskX=-20) still renders at the primary position.
        let mut app = sync_test_app();
        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = Some(EffectRenderState {
            enabled: true,
            stage_width: 1024.0,
            stage_height: 768.0,
            effects: vec![EffectRenderEntry {
                key: "fx-maskzero".to_owned(),
                image_url: Some("/original-effects/Magic/0.png".to_owned()),
                left: 480.0,
                top: 352.0,
                width: 48.0,
                height: 48.0,
                z: 9.0,
                additive: true,
                opacity: None,
                mask_image_url: Some("/original-effects/Magic/0.png".to_owned()),
                mask_width: Some(48.0),
                mask_height: Some(48.0),
                mask_x: Some(-20.0),
                mask_y: Some(0.0),
                frame_x: Some(-20.0),
                frame_y: Some(0.0),
                shadow_x: None,
                shadow_y: None,
            }],
        });
        app.update();
        let registry = app.world().resource::<SceneRegistry>();
        assert_eq!(
            registry.effect_render_masks.len(),
            1,
            "mask at zero delta still spawned"
        );
        // dx = -20-(-20) = 0, dy = 0-0 = 0 -> centered on primary (world x = 480+24-512 = -8).
        let mask_entity = registry.effect_render_masks["fx-maskzero:mask"].entity;
        let mask_transform = *app.world().get::<Transform>(mask_entity).unwrap();
        assert!(
            (mask_transform.translation.x - (-8.0)).abs() < 0.5,
            "zero-delta mask centered on primary x"
        );
    }

    #[test]
    fn sync_effect_render_disabled_snapshot_clears_all_layers() {
        // enabled=false must clear every effect layer and recycle materials.
        let mut app = sync_test_app();
        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = Some(EffectRenderState {
            enabled: true,
            stage_width: 1024.0,
            stage_height: 768.0,
            effects: vec![EffectRenderEntry {
                key: "fx-dis".to_owned(),
                image_url: Some("/original-effects/Magic/0.png".to_owned()),
                left: 480.0,
                top: 352.0,
                width: 48.0,
                height: 48.0,
                z: 9.0,
                additive: true,
                opacity: None,
                mask_image_url: Some("/original-effects/Magic/0.png".to_owned()),
                mask_width: Some(32.0),
                mask_height: Some(32.0),
                mask_x: Some(3.0),
                mask_y: Some(-7.0),
                frame_x: Some(-20.0),
                frame_y: Some(0.0),
                shadow_x: None,
                shadow_y: None,
            }],
        });
        app.update();
        assert_eq!(
            app.world()
                .resource::<SceneRegistry>()
                .effect_render_masks
                .len(),
            1
        );
        // Disable the snapshot.
        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = Some(EffectRenderState {
            enabled: false,
            stage_width: 1024.0,
            stage_height: 768.0,
            effects: vec![],
        });
        app.update();
        let registry = app.world().resource::<SceneRegistry>();
        assert!(
            registry.effect_render.is_empty(),
            "primary cleared on disable"
        );
        assert!(
            registry.effect_render_masks.is_empty(),
            "mask cleared on disable"
        );
        assert!(
            registry.effect_render_shadows.is_empty(),
            "shadow cleared on disable"
        );
        let cache = app
            .world()
            .resource::<crate::additive_material::CrystalAdditiveMaterialCache>();
        assert_eq!(cache.len(), 0, "materials recycled on disable");
    }

    #[test]
    fn sync_lighting_is_bounded_and_clears_on_map_logout_or_reconnect_reset() {
        let mut app = sync_test_app();
        app.world_mut()
            .resource_mut::<RuntimeLightingRenderState>()
            .snapshot = Some(lighting::LightingRenderState {
            enabled: true,
            stage_width: 1024.0,
            stage_height: 768.0,
            time_of_day_light_setting: Some(4),
            map_light_setting: None,
            light_setting: None,
            map_dark_light: 2,
            map_lights: (0..220)
                .map(|index| lighting::MapLightSource {
                    key: format!("map-{index}"),
                    draw_x: index as f32,
                    draw_y: index as f32,
                    light: 1,
                    offset_x: 0.0,
                    offset_y: 0.0,
                })
                .collect(),
            entity_lights: Vec::new(),
        });
        app.update();
        let (buffer_camera, buffer_image, composite_entity, multiply_material, first_layer) = {
            let registry = app.world().resource::<SceneRegistry>();
            assert_eq!(registry.lighting_layers.len(), lighting::MAX_NATIVE_LIGHTS);
            assert_eq!(
                registry.lighting_images.len(),
                lighting::LIGHT_TEXTURE_COUNT
            );
            let darkness = registry
                .lighting_darkness
                .as_ref()
                .expect("offscreen darkness buffer spawned");
            (
                darkness.buffer_camera,
                darkness.buffer_image.clone(),
                darkness.composite_entity,
                darkness.material.clone(),
                registry.lighting_layers.values().next().unwrap().entity,
            )
        };
        assert!(
            app.world()
                .get::<MirLightingBufferCamera>(buffer_camera)
                .is_some(),
            "one isolated light-buffer camera exists"
        );
        assert!(
            app.world()
                .get::<MirLightingComposite>(composite_entity)
                .is_some(),
            "one main-pass multiply composite exists"
        );
        assert!(
            app.world()
                .get::<MirLightingBufferLayer>(first_layer)
                .is_some(),
            "light sprites live in the isolated buffer"
        );
        assert!(
            app.world().get::<RenderLayers>(first_layer).is_some(),
            "light sprite cannot leak into the main scene pass"
        );
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(&buffer_image)
                .is_some(),
            "offscreen target is retained while active"
        );
        assert!(
            app.world()
                .resource::<Assets<crate::lighting::CrystalMultiplyMaterial>>()
                .get(&multiply_material)
                .is_some(),
            "multiply material samples the retained target"
        );

        // Map change, logout and reconnect all advance SceneResetRevision. Run
        // the real reset system, not a manually-cleared snapshot, and require
        // the renderer resources to disappear in that same update.
        app.world_mut().resource_mut::<SceneResetRevision>().0 = 1;
        app.update();
        let registry = app.world().resource::<SceneRegistry>();
        assert!(registry.lighting_layers.is_empty());
        assert!(registry.lighting_images.is_empty());
        assert!(registry.lighting_darkness.is_none());
        assert!(app.world().get_entity(buffer_camera).is_err());
        assert!(app.world().get_entity(composite_entity).is_err());
        assert!(app
            .world()
            .resource::<Assets<Image>>()
            .get(&buffer_image)
            .is_none());
        assert!(app
            .world()
            .resource::<Assets<crate::lighting::CrystalMultiplyMaterial>>()
            .get(&multiply_material)
            .is_none());
        assert_eq!(
            app.world()
                .resource::<crate::additive_material::CrystalAdditiveMaterialCache>()
                .len(),
            0
        );
    }

    #[test]
    fn sync_lighting_stage_resize_rebuilds_without_leaking_old_targets() {
        let mut app = sync_test_app();
        let state = |width, height| lighting::LightingRenderState {
            enabled: true,
            stage_width: width,
            stage_height: height,
            time_of_day_light_setting: Some(4),
            map_light_setting: None,
            light_setting: None,
            map_dark_light: 0,
            map_lights: Vec::new(),
            entity_lights: vec![lighting::EntityLightSource {
                key: "self".to_owned(),
                draw_x: width * 0.5,
                draw_y: height * 0.5,
                kind: "selfPlayer".to_owned(),
                light: Some(3),
                dead: false,
                is_self: true,
            }],
        };
        app.world_mut()
            .resource_mut::<RuntimeLightingRenderState>()
            .snapshot = Some(state(1024.0, 768.0));
        app.update();
        let (old_camera, old_composite, old_image, old_material) = {
            let registry = app.world().resource::<SceneRegistry>();
            let darkness = registry.lighting_darkness.as_ref().unwrap();
            (
                darkness.buffer_camera,
                darkness.composite_entity,
                darkness.buffer_image.clone(),
                darkness.material.clone(),
            )
        };

        app.world_mut()
            .resource_mut::<RuntimeLightingRenderState>()
            .snapshot = Some(state(800.0, 600.0));
        app.update();

        let registry = app.world().resource::<SceneRegistry>();
        let rebuilt = registry.lighting_darkness.as_ref().unwrap();
        assert_eq!(rebuilt.stage_size, UVec2::new(800, 600));
        assert_ne!(rebuilt.buffer_camera, old_camera);
        assert_ne!(rebuilt.composite_entity, old_composite);
        assert_ne!(rebuilt.buffer_image, old_image);
        assert_ne!(rebuilt.material, old_material);
        assert_eq!(registry.lighting_layers.len(), 1);
        assert_eq!(
            app.world()
                .resource::<crate::additive_material::CrystalAdditiveMaterialCache>()
                .len(),
            1
        );
        assert!(app.world().get_entity(old_camera).is_err());
        assert!(app.world().get_entity(old_composite).is_err());
        assert!(app
            .world()
            .resource::<Assets<Image>>()
            .get(&old_image)
            .is_none());
        assert!(app
            .world()
            .resource::<Assets<crate::lighting::CrystalMultiplyMaterial>>()
            .get(&old_material)
            .is_none());
    }
}

#[cfg(test)]
mod native_data_path_tests {
    use super::*;

    fn ingest_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<additive_material::CrystalAdditiveMaterial>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<additive_material::CrystalAdditiveMaterialCache>()
            .insert_resource(mir2_client_bevy::read_model::UiReadModel::default())
            .insert_resource(mir2_client_bevy::read_model::UiSurfaceSignals::default())
            .insert_resource(mir2_client_bevy::map::MapModel::default())
            .insert_resource(mir2_client_bevy::entities::EntityModelSet::default())
            .insert_resource(mir2_client_bevy::inventory::InventoryModel::default())
            .insert_resource(mir2_client_bevy::chat::ChatModel::default())
            .insert_resource(mir2_client_bevy::mail::MailModel::default())
            .insert_resource(mir2_client_bevy::shop::ShopModel::default())
            .insert_resource(mir2_client_bevy::game_shop::GameShopModel::default())
            .insert_resource(mir2_client_bevy::storage::StorageModel::default())
            .insert_resource(mir2_client_bevy::skill_model::SkillModel::default())
            .insert_resource(mir2_client_bevy::social::SocialModel::default())
            .insert_resource(PendingOperations::default())
            .insert_resource(InventoryOperationFeedback::default())
            .insert_resource(AuthoritativeModelRevisions::default())
            .insert_resource(SessionResetRevision::default())
            .insert_resource(SessionResetGameShopPreservation::default())
            .insert_resource(RuntimeSessionResetTracker::default())
            .insert_resource(RuntimeWorldState::default())
            .insert_resource(RuntimeEntityRenderState::default())
            .insert_resource(RuntimeEntityRenderAtlases::default())
            .insert_resource(RuntimeMapRenderState::default())
            .insert_resource(RuntimeMapRenderAtlases::default())
            .insert_resource(RuntimeEffectRenderState::default())
            .insert_resource(RuntimeMapCameraOffset::default())
            .insert_resource(interpolation::SnapshotBuffer::default())
            .insert_resource(motion::EntityMotionTable::default())
            .insert_resource(presentation_pose::PresentationPoseBuffer::default())
            .insert_resource(SceneRegistry::default())
            .insert_resource(SceneResetRevision::default())
            .insert_resource(RuntimeSceneResetTracker::default())
            .insert_resource(RuntimeSceneModelResetTracker::default())
            .insert_resource(native_ingest::NativeInbound::new())
            .add_systems(
                Update,
                (
                    ingest_pending_scene_and_data_reset,
                    apply_scene_reset_to_runtime,
                    apply_scene_reset_to_scene_models,
                    apply_session_reset_to_runtime_models,
                    ingest_pending_ui_read_model,
                    ingest_pending_map_model,
                    ingest_pending_entity_model_set,
                    ingest_pending_inventory_model,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    ingest_pending_inventory_operation_ack,
                    ingest_pending_wallet_patch,
                    ingest_pending_mail_model,
                    ingest_pending_shop_model,
                    ingest_pending_game_shop_info,
                    ingest_pending_game_shop_stock,
                    ingest_pending_game_shop_receipt,
                    ingest_pending_npc_shop_service,
                    ingest_pending_storage_patch,
                    ingest_pending_storage_items,
                    ingest_pending_storage_model,
                    ingest_pending_skill_model,
                    ingest_pending_social_model,
                    ingest_pending_chat_line,
                )
                    .chain()
                    .after(ingest_pending_inventory_model),
            );
        app
    }

    #[test]
    fn native_mail_shop_storage_payloads_update_preserve_reject_and_reset() {
        let _native_queue_guard = native_ingest::native_queue_test_guard();
        let mut app = ingest_app();

        assert!(native_ingest::push_native_mail_model(
            r#"{"mails":[{"id":7,"sender":"GM","subject":"Gift","body":"Hello","gold":10,"items":[{"name":"Potion"}],"claimed":false,"locked":false,"read":false}]}"#.to_owned()
        ));
        assert!(native_ingest::push_native_shop_model(
            r#"{"goods":[{"unique_id":9,"name":"Potion","price":50,"count":20,"stock":-1,"panel_type":0}]}"#.to_owned()
        ));
        assert!(native_ingest::push_native_storage_model(
            r#"{"items":[{"key":"sword","name":"Iron Sword","quantity":1,"slot":3,"container":4}],"size":30,"has_password":false,"unlocked":true,"has_expanded":false,"expiry":0}"#.to_owned()
        ));
        for index in 0..105 {
            assert!(native_ingest::push_native_game_shop_info(format!(
                r#"{{"itemIndex":1000,"gameShopIndex":{},"itemName":"Cash {}","goldPrice":10,"creditPrice":2,"count":1,"stock":0,"stockLevel":0,"canBuyGold":true,"canBuyCredit":true}}"#,
                index, index
            )));
        }
        assert!(native_ingest::push_native_game_shop_stock(
            r#"{"gIndex":42,"stockLevel":7}"#.to_owned()
        ));
        app.update();

        assert!(
            !app.world()
                .resource::<mir2_client_bevy::read_model::UiSurfaceSignals>()
                .npc_shop_open_requested
        );

        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::mail::MailModel>()
                .mails
                .len(),
            1
        );
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::shop::ShopModel>()
                .goods
                .len(),
            1
        );
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::storage::StorageModel>()
                .items
                .len(),
            1
        );
        {
            let game_shop = app
                .world()
                .resource::<mir2_client_bevy::game_shop::GameShopModel>();
            assert_eq!(game_shop.items.len(), 105);
            assert_eq!(game_shop.items[42].stock_level, 7);
        }

        app.world_mut()
            .resource_mut::<mir2_client_bevy::mail::MailModel>()
            .selected_id = Some(7);
        app.world_mut()
            .resource_mut::<mir2_client_bevy::shop::ShopModel>()
            .selected_id = Some(9);
        {
            let mut storage = app
                .world_mut()
                .resource_mut::<mir2_client_bevy::storage::StorageModel>();
            storage.selected_storage_slot = Some(3);
            storage.password_draft = "old-password".to_owned();
            storage.new_password_draft = "new-password".to_owned();
            storage.confirm_password_draft = "new-password".to_owned();
        }

        assert!(native_ingest::push_native_mail_model(
            r#"{"mails":[{"id":7,"sender":"GM","subject":"Gift","body":"Updated","gold":20,"items":[{"name":"Potion"}],"claimed":false,"locked":false,"read":true}]}"#.to_owned()
        ));
        assert!(native_ingest::push_native_shop_model(
            r#"{"goods":[{"unique_id":9,"name":"Potion","price":60,"count":20,"stock":-1,"panel_type":0}]}"#.to_owned()
        ));
        assert!(native_ingest::push_native_npc_shop_service(
            r#"{"mode":"buy","repairRate":null}"#.to_owned()
        ));
        assert!(native_ingest::push_native_storage_items(
            r#"{"items":[{"key":"sword","name":"Iron Sword","quantity":2,"slot":3,"container":4}]}"#.to_owned()
        ));
        app.update();

        assert!(
            app.world()
                .resource::<mir2_client_bevy::read_model::UiSurfaceSignals>()
                .npc_shop_open_requested
        );

        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::mail::MailModel>()
                .selected_id,
            Some(7)
        );
        {
            let shop = app.world().resource::<mir2_client_bevy::shop::ShopModel>();
            assert_eq!(shop.selected_id, None);
            assert!(shop.allows_buy());
        }
        assert!(native_ingest::push_native_npc_shop_service(
            r#"{"mode":"repair","repairRate":1.5}"#.to_owned()
        ));
        app.update();
        {
            let shop = app.world().resource::<mir2_client_bevy::shop::ShopModel>();
            assert!(shop.allows_repair());
            assert_eq!(shop.repair_rate, Some(1.5));
        }
        assert!(native_ingest::push_native_npc_shop_service(
            r#"{"mode":"specialRepair","repairRate":null}"#.to_owned()
        ));
        app.update();
        assert!(app
            .world()
            .resource::<mir2_client_bevy::shop::ShopModel>()
            .allows_repair());
        {
            let storage = app
                .world()
                .resource::<mir2_client_bevy::storage::StorageModel>();
            assert_eq!(storage.items[0].quantity, 2);
            assert_eq!(storage.selected_storage_slot, Some(3));
            assert_eq!(storage.password_draft, "old-password");
            assert_eq!(storage.new_password_draft, "new-password");
        }

        assert!(native_ingest::push_native_mail_model(
            r#"{"mails":{}}"#.to_owned()
        ));
        assert!(native_ingest::push_native_shop_model(
            r#"{"goods":{}}"#.to_owned()
        ));
        assert!(native_ingest::push_native_storage_model(
            r#"{"items":{}}"#.to_owned()
        ));
        app.update();
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::mail::MailModel>()
                .selected_id,
            Some(7)
        );
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::shop::ShopModel>()
                .selected_id,
            None
        );
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::storage::StorageModel>()
                .items[0]
                .quantity,
            2
        );

        assert!(native_ingest::push_native_mail_model(
            r#"{"mails":[{"id":7,"sender":"GM","subject":"Gift","body":"Unread baseline","gold":20,"items":[{"name":"Potion"}],"claimed":false,"locked":false,"read":false}]}"#.to_owned()
        ));
        app.update();
        let read_key = mir2_client_bevy::pending_operations::PendingOperationKey::ReadMail(7);
        assert!(app
            .world_mut()
            .resource_mut::<PendingOperations>()
            .try_begin(read_key.clone()));
        assert!(native_ingest::push_native_mail_model(
            r#"{"mails":[{"id":7,"sender":"GM","subject":"Gift","body":"Periodic unread","gold":20,"items":[{"name":"Potion"}],"claimed":false,"locked":false,"read":false}]}"#.to_owned()
        ));
        app.update();
        assert!(app
            .world()
            .resource::<PendingOperations>()
            .contains(&read_key));
        assert!(native_ingest::push_native_mail_model(
            r#"{"mails":[{"id":7,"sender":"GM","subject":"Gift","body":"Authoritative read","gold":20,"items":[{"name":"Potion"}],"claimed":false,"locked":false,"read":true}]}"#.to_owned()
        ));
        app.update();
        assert!(!app
            .world()
            .resource::<PendingOperations>()
            .contains(&read_key));
        assert!(
            app.world()
                .resource::<mir2_client_bevy::mail::MailModel>()
                .mails[0]
                .read
        );

        assert!(native_ingest::push_native_data_reset());
        app.update();
        assert!(app
            .world()
            .resource::<mir2_client_bevy::mail::MailModel>()
            .mails
            .is_empty());
        assert!(app
            .world()
            .resource::<mir2_client_bevy::shop::ShopModel>()
            .goods
            .is_empty());
        assert!(app
            .world()
            .resource::<mir2_client_bevy::storage::StorageModel>()
            .items
            .is_empty());
        assert!(app
            .world()
            .resource::<mir2_client_bevy::game_shop::GameShopModel>()
            .items
            .is_empty());
        assert!(
            !app.world()
                .resource::<mir2_client_bevy::read_model::UiSurfaceSignals>()
                .npc_shop_open_requested
        );
    }

    #[test]
    fn native_storage_nacks_release_exact_pending_operations_without_mutating_models() {
        let _native_queue_guard = native_ingest::native_queue_test_guard();
        let mut app = ingest_app();
        let unlock = PendingOperationKey::StorageUnlock;
        let remove = PendingOperationKey::StorageRemovePassword;
        let deposit = PendingOperationKey::StorageDeposit {
            unique_id: 77,
            from: 3,
            to: 9,
        };
        {
            let mut pending = app.world_mut().resource_mut::<PendingOperations>();
            assert!(pending.try_begin(unlock.clone()));
            assert!(pending.try_begin(remove.clone()));
            assert!(pending.try_begin(deposit.clone()));
        }

        assert!(native_ingest::push_native_storage_patch(
            r#"{"ack":{"operation":"unlock","success":false}}"#.to_owned()
        ));
        assert!(native_ingest::push_native_storage_patch(
            r#"{"ack":{"operation":"deposit","from":3,"to":9,"success":false}}"#.to_owned()
        ));
        app.update();

        let pending = app.world().resource::<PendingOperations>();
        assert!(!pending.contains(&unlock));
        assert!(!pending.contains(&deposit));
        assert!(pending.contains(&remove));
        let storage = app
            .world()
            .resource::<mir2_client_bevy::storage::StorageModel>();
        assert!(!storage.has_password);
        assert!(!storage.unlocked);
        assert!(storage.items.is_empty());
    }

    #[test]
    fn native_game_shop_receipt_requires_exact_pending_request() {
        let _native_queue_guard = native_ingest::native_queue_test_guard();
        let mut app = ingest_app();
        let request = app
            .world_mut()
            .resource_mut::<mir2_client_bevy::game_shop::GameShopModel>()
            .begin_purchase(31, 2, 1)
            .expect("reserve purchase");
        assert!(app
            .world_mut()
            .resource_mut::<PendingOperations>()
            .try_begin(PendingOperationKey::GameShop(request.request_id.clone())));

        assert!(native_ingest::push_native_game_shop_receipt(
            r#"{"protocol":"nativeGameShopReceiptV1","requestId":"gs-wrong","success":true,"gIndex":31,"quantity":2,"priceType":1,"mailId":999}"#.to_owned()
        ));
        app.update();
        assert!(app
            .world()
            .resource::<mir2_client_bevy::game_shop::GameShopModel>()
            .pending_purchase
            .is_some());
        assert_eq!(app.world().resource::<PendingOperations>().len(), 1);

        assert!(native_ingest::push_native_game_shop_receipt(format!(
            r#"{{"protocol":"nativeGameShopReceiptV1","requestId":"{}","success":true,"gIndex":31,"quantity":2,"priceType":1,"newStockLevel":3,"mailId":1842}}"#,
            request.request_id
        )));
        app.update();
        let model = app
            .world()
            .resource::<mir2_client_bevy::game_shop::GameShopModel>();
        assert!(model.pending_purchase.is_none());
        assert_eq!(
            model.last_receipt.as_ref().and_then(|r| r.mail_id),
            Some(1842)
        );
        assert!(app.world().resource::<PendingOperations>().is_empty());
    }

    #[test]
    fn scene_reset_clears_scene_same_frame_but_preserves_personal_models_and_pending() {
        let _native_queue_guard = native_ingest::native_queue_test_guard();
        let mut app = ingest_app();
        app.world_mut().resource_mut::<RuntimeWorldState>().snapshot =
            Some(serde_json::from_str(r#"{"entities":[]}"#).unwrap());
        app.world_mut()
            .resource_mut::<RuntimeMapRenderState>()
            .snapshot = Some(
            serde_json::from_str(r#"{"enabled":true,"stageWidth":1024,"stageHeight":768}"#)
                .unwrap(),
        );
        app.world_mut()
            .resource_mut::<RuntimeEntityRenderState>()
            .snapshot = Some(
            serde_json::from_str(r#"{"enabled":true,"stageWidth":1024,"stageHeight":768}"#)
                .unwrap(),
        );
        app.world_mut()
            .resource_mut::<RuntimeEffectRenderState>()
            .snapshot = Some(
            serde_json::from_str(
                r#"{"enabled":true,"stageWidth":1024,"stageHeight":768,"effects":[]}"#,
            )
            .unwrap(),
        );

        let map_entity = app.world_mut().spawn_empty().id();
        let effect_entity = app.world_mut().spawn_empty().id();
        let additive_entity = app.world_mut().spawn_empty().id();
        app.world_mut().resource_scope(
            |world, mut cache: Mut<additive_material::CrystalAdditiveMaterialCache>| {
                let mut materials =
                    world.resource_mut::<Assets<additive_material::CrystalAdditiveMaterial>>();
                cache.material(
                    &entity_additive_material_key("scene-additive"),
                    Handle::<Image>::default(),
                    1.0,
                    &mut materials,
                );
            },
        );
        {
            let mut registry = app.world_mut().resource_mut::<SceneRegistry>();
            registry.map.spawned.push(map_entity);
            registry.effect_render.insert(
                "fx".to_owned(),
                EffectRenderLayerHandle {
                    entity: effect_entity,
                    image_key: "fx.png".to_owned(),
                    additive: false,
                },
            );
            registry.entity_render_layers.insert(
                "scene-additive".to_owned(),
                EntityRenderLayerHandle {
                    entity: additive_entity,
                    image_key: "atlas:scene".to_owned(),
                    atlas_key: Some("scene".to_owned()),
                    atlas_rect_key: Some("effect".to_owned()),
                    additive: true,
                },
            );
        }

        app.world_mut()
            .resource_mut::<mir2_client_bevy::read_model::UiReadModel>()
            .player
            .name = Some("Still logged in".to_owned());
        app.world_mut()
            .resource_mut::<mir2_client_bevy::inventory::InventoryModel>()
            .gold = 123;
        app.world_mut()
            .resource_mut::<mir2_client_bevy::map::MapModel>()
            .center_x = 77;
        app.world_mut()
            .resource_mut::<mir2_client_bevy::entities::EntityModelSet>()
            .entities
            .push(mir2_client_bevy::entities::EntityModel {
                object_id: "scene-entity".to_owned(),
                kind: mir2_client_bevy::entities::EntityKind::Monster,
                name: "Scene entity".to_owned(),
                x: 1,
                y: 2,
                level: None,
                direction: None,
            });
        app.world_mut()
            .resource_mut::<mir2_client_bevy::mail::MailModel>()
            .selected_id = Some(7);
        app.world_mut()
            .resource_mut::<mir2_client_bevy::game_shop::GameShopModel>()
            .upsert(mir2_client_bevy::game_shop::GameShopEntry {
                game_shop_index: 1,
                item_index: 2,
                item_name: "Cash item".to_owned(),
                ..Default::default()
            });
        let pending_key = mir2_client_bevy::pending_operations::PendingOperationKey::StorageExpand;
        assert!(app
            .world_mut()
            .resource_mut::<PendingOperations>()
            .try_begin(pending_key.clone()));

        assert!(native_ingest::push_native_scene_reset());
        app.update();

        assert!(app
            .world()
            .resource::<RuntimeWorldState>()
            .snapshot
            .is_none());
        assert!(app
            .world()
            .resource::<RuntimeMapRenderState>()
            .snapshot
            .is_none());
        assert!(app
            .world()
            .resource::<RuntimeEntityRenderState>()
            .snapshot
            .is_none());
        assert!(app
            .world()
            .resource::<RuntimeEffectRenderState>()
            .snapshot
            .is_none());
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::map::MapModel>()
                .center_x,
            0
        );
        assert!(app
            .world()
            .resource::<mir2_client_bevy::entities::EntityModelSet>()
            .entities
            .is_empty());
        let registry = app.world().resource::<SceneRegistry>();
        assert!(registry.map.spawned.is_empty());
        assert!(registry.effect_render.is_empty());
        assert!(registry.entity_render_layers.is_empty());
        assert!(!app.world().entities().contains(map_entity));
        assert!(!app.world().entities().contains(effect_entity));
        assert!(!app.world().entities().contains(additive_entity));
        assert_eq!(
            app.world()
                .resource::<additive_material::CrystalAdditiveMaterialCache>()
                .len(),
            0
        );

        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::read_model::UiReadModel>()
                .player
                .name
                .as_deref(),
            Some("Still logged in")
        );
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::inventory::InventoryModel>()
                .gold,
            123
        );
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::mail::MailModel>()
                .selected_id,
            Some(7)
        );
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::game_shop::GameShopModel>()
                .items
                .len(),
            1
        );
        assert!(app
            .world()
            .resource::<PendingOperations>()
            .contains(&pending_key));
        assert_eq!(app.world().resource::<SessionResetRevision>().0, 0);
    }

    #[test]
    fn data_reset_includes_scene_reset_and_clears_all_models_and_pending() {
        let _native_queue_guard = native_ingest::native_queue_test_guard();
        let mut app = ingest_app();
        app.world_mut().resource_mut::<RuntimeWorldState>().snapshot =
            Some(serde_json::from_str(r#"{"entities":[]}"#).unwrap());
        let scene_entity = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<SceneRegistry>()
            .map
            .spawned
            .push(scene_entity);
        app.world_mut()
            .resource_mut::<mir2_client_bevy::inventory::InventoryModel>()
            .gold = 999;
        app.world_mut()
            .resource_mut::<mir2_client_bevy::game_shop::GameShopModel>()
            .upsert(mir2_client_bevy::game_shop::GameShopEntry {
                game_shop_index: 9,
                item_index: 10,
                item_name: "Account A cash item".to_owned(),
                ..Default::default()
            });
        app.world_mut()
            .resource_mut::<PendingOperations>()
            .try_begin(mir2_client_bevy::pending_operations::PendingOperationKey::StorageExpand);

        assert!(native_ingest::push_native_data_reset());
        app.update();

        assert!(app
            .world()
            .resource::<RuntimeWorldState>()
            .snapshot
            .is_none());
        assert!(app
            .world()
            .resource::<SceneRegistry>()
            .map
            .spawned
            .is_empty());
        assert!(!app.world().entities().contains(scene_entity));
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::inventory::InventoryModel>()
                .gold,
            0
        );
        assert!(app
            .world()
            .resource::<mir2_client_bevy::game_shop::GameShopModel>()
            .items
            .is_empty());
        assert!(app.world().resource::<PendingOperations>().is_empty());
        assert_eq!(app.world().resource::<SessionResetRevision>().0, 1);
        assert_eq!(app.world().resource::<SceneResetRevision>().0, 1);
    }

    #[test]
    fn data_reset_clears_all_character_read_models_before_next_account_snapshot() {
        let _native_queue_guard = native_ingest::native_queue_test_guard();
        let mut app = ingest_app();

        app.world_mut()
            .resource_mut::<mir2_client_bevy::read_model::UiReadModel>()
            .player
            .name = Some("Account A".to_owned());
        app.world_mut()
            .resource_mut::<mir2_client_bevy::map::MapModel>()
            .center_x = 321;
        app.world_mut()
            .resource_mut::<mir2_client_bevy::entities::EntityModelSet>()
            .entities
            .push(mir2_client_bevy::entities::EntityModel {
                object_id: "100".to_owned(),
                kind: mir2_client_bevy::entities::EntityKind::SelfPlayer,
                name: "Account A".to_owned(),
                x: 1,
                y: 2,
                level: Some(7),
                direction: Some("Down".to_owned()),
            });
        app.world_mut()
            .resource_mut::<mir2_client_bevy::inventory::InventoryModel>()
            .gold = 999;
        app.world_mut()
            .resource_mut::<mir2_client_bevy::chat::ChatModel>()
            .lines
            .push(mir2_client_bevy::chat::ChatLine {
                text: "A-only".to_owned(),
                channel: "normal".to_owned(),
            });
        app.world_mut()
            .resource_mut::<mir2_client_bevy::skill_model::SkillModel>()
            .skills
            .push(mir2_client_bevy::skill_model::SkillEntry {
                id: 1,
                name: "FireBall".to_owned(),
                level: 1,
                key: Some("1".to_owned()),
                cooldown_ms: 500,
                mp_cost: 5,
            });
        app.world_mut()
            .resource_mut::<PendingOperations>()
            .try_begin(mir2_client_bevy::pending_operations::PendingOperationKey::StorageExpand);

        assert!(native_ingest::push_native_data_reset());
        app.update();

        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::read_model::UiReadModel>()
                .player
                .name,
            None
        );
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::map::MapModel>()
                .center_x,
            0
        );
        assert!(app
            .world()
            .resource::<mir2_client_bevy::entities::EntityModelSet>()
            .entities
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::inventory::InventoryModel>()
                .gold,
            0
        );
        assert!(app
            .world()
            .resource::<mir2_client_bevy::chat::ChatModel>()
            .lines
            .is_empty());
        assert!(app
            .world()
            .resource::<mir2_client_bevy::skill_model::SkillModel>()
            .skills
            .is_empty());
        assert!(app.world().resource::<PendingOperations>().is_empty());
        assert_eq!(app.world().resource::<SessionResetRevision>().0, 1);
    }

    #[test]
    fn unchanged_periodic_inventory_snapshot_keeps_pending_until_exact_nack() {
        let _native_queue_guard = native_ingest::native_queue_test_guard();
        let mut app = ingest_app();
        *app.world_mut()
            .resource_mut::<mir2_client_bevy::inventory::InventoryModel>() =
            mir2_client_bevy::inventory::InventoryModel {
                gold: 10,
                items: vec![mir2_client_bevy::inventory::ItemModel {
                    unique_id: Some(10),
                    key: "small-hp-drug".into(),
                    name: "Potion".into(),
                    quantity: 5,
                    slot: 0,
                    container: 0,
                    ..mir2_client_bevy::inventory::ItemModel::default()
                }],
            };
        let key = mir2_client_bevy::pending_operations::PendingOperationKey::Split {
            grid: "inventory".into(),
            unique_id: 10,
            count: 2,
        };
        app.world_mut()
            .resource_mut::<PendingOperations>()
            .try_begin(key.clone());

        assert!(native_ingest::push_native_inventory_model(
            r#"{"gold":10,"items":[{"uniqueId":10,"key":"small-hp-drug","name":"Potion","quantity":5,"slot":0,"container":0}]}"#.to_owned()
        ));
        app.update();
        assert!(app.world().resource::<PendingOperations>().contains(&key));

        assert!(native_ingest::push_native_inventory_operation_ack(
            r#"{"operation":"split","grid":"Inventory","unique_id":10,"count":2,"success":false}"#
                .to_owned()
        ));
        app.update();
        assert!(!app.world().resource::<PendingOperations>().contains(&key));
        assert_eq!(
            app.world()
                .resource::<InventoryOperationFeedback>()
                .last
                .as_ref()
                .map(InventoryOperationAck::success),
            Some(false)
        );
    }

    #[test]
    fn reset_drops_account_a_models_but_preserves_account_b_models_queued_after_it() {
        let _native_queue_guard = native_ingest::native_queue_test_guard();
        let mut app = ingest_app();
        assert!(native_ingest::push_native_inventory_model(
            r#"{"gold":111,"items":[]}"#.to_owned()
        ));
        assert!(native_ingest::push_native_data_reset());
        assert!(native_ingest::push_native_inventory_model(
            r#"{"gold":222,"items":[]}"#.to_owned()
        ));
        assert!(native_ingest::push_native_ui_read_model(
            r#"{"player":{"name":"Account B","hp":10,"maxHp":10}}"#.to_owned()
        ));

        app.update();

        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::inventory::InventoryModel>()
                .gold,
            222
        );
        assert_eq!(
            app.world()
                .resource::<mir2_client_bevy::read_model::UiReadModel>()
                .player
                .name
                .as_deref(),
            Some("Account B")
        );
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod native_soak_metrics_tests {
    use super::*;

    #[test]
    fn native_soak_counts_empty_scene_are_zero() {
        let registry = SceneRegistry::default();
        let effect_state = RuntimeEffectRenderState::default();
        let additive_cache = additive_material::CrystalAdditiveMaterialCache::default();
        let additive_materials = Assets::<additive_material::CrystalAdditiveMaterial>::default();

        assert_eq!(
            native_soak_counts(
                &registry,
                &effect_state,
                &additive_cache,
                &additive_materials,
            ),
            NativeSoakCounts::default()
        );
    }

    #[test]
    fn native_soak_counts_separate_retained_layers_and_assets() {
        let mut registry = SceneRegistry::default();
        registry.effect_render.insert(
            "fx-primary".to_owned(),
            EffectRenderLayerHandle {
                entity: Entity::PLACEHOLDER,
                image_key: "primary.png".to_owned(),
                additive: true,
            },
        );
        registry.effect_render_masks.insert(
            "fx-primary:mask".to_owned(),
            EffectRenderLayerHandle {
                entity: Entity::PLACEHOLDER,
                image_key: "mask.png".to_owned(),
                additive: true,
            },
        );
        registry.effect_render_shadows.insert(
            "fx-primary:shadow".to_owned(),
            EffectShadowLayerHandle {
                entity: Entity::PLACEHOLDER,
                mesh: Handle::default(),
                material: Handle::default(),
            },
        );
        registry
            .effect_render_images
            .insert("primary.png".to_owned(), Handle::default());
        registry.entity_render_layers.insert(
            "player:body".to_owned(),
            EntityRenderLayerHandle {
                entity: Entity::PLACEHOLDER,
                image_key: "player.png".to_owned(),
                atlas_key: None,
                atlas_rect_key: None,
                additive: false,
            },
        );
        registry.entities.insert(
            "player".to_owned(),
            SceneEntityHandles {
                root: Entity::PLACEHOLDER,
                shadow: Entity::PLACEHOLDER,
                body: Entity::PLACEHOLDER,
                crest: Entity::PLACEHOLDER,
                facing: Entity::PLACEHOLDER,
                selection: Entity::PLACEHOLDER,
            },
        );
        registry.entity_render_atlases.insert(
            "players".to_owned(),
            EntityRenderAtlasHandle {
                layout: Handle::default(),
                rects: HashMap::new(),
                uv_rects: HashMap::new(),
                size: UVec2::new(1, 1),
                image_key: None,
                image: None,
            },
        );
        registry.map_render.tiles.insert(
            "tile-1".to_owned(),
            MapRenderTileHandle {
                entity: Entity::PLACEHOLDER,
                last_seen_generation: 1,
            },
        );
        registry.map.spawned.push(Entity::PLACEHOLDER);
        registry.mine_nodes.insert(
            (1, 2),
            MineNodeHandles {
                root: Entity::PLACEHOLDER,
                ore: Entity::PLACEHOLDER,
            },
        );
        registry.lighting_layers.insert(
            "light-1".to_owned(),
            EffectRenderLayerHandle {
                entity: Entity::PLACEHOLDER,
                image_key: "light.png".to_owned(),
                additive: false,
            },
        );
        registry.lighting_images.push(Handle::default());

        let effect_state = RuntimeEffectRenderState {
            snapshot: Some(
                serde_json::from_str(
                    r#"{
                        "enabled": true,
                        "stageWidth": 1024,
                        "stageHeight": 768,
                        "effects": [{
                            "key": "fx-primary",
                            "left": 0,
                            "top": 0,
                            "width": 1,
                            "height": 1,
                            "z": 1
                        }]
                    }"#,
                )
                .expect("minimal effect render state should deserialize"),
            ),
        };
        let mut additive_cache = additive_material::CrystalAdditiveMaterialCache::default();
        let mut additive_materials =
            Assets::<additive_material::CrystalAdditiveMaterial>::default();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::default());
        let cached_material =
            additive_cache.material("fx-primary", image, 1.0, &mut additive_materials);

        let counts = native_soak_counts(
            &registry,
            &effect_state,
            &additive_cache,
            &additive_materials,
        );

        assert_eq!(counts.snapshot_effects, 1);
        assert_eq!(counts.retained_effect_primary, 1);
        assert_eq!(counts.retained_effect_masks, 1);
        assert_eq!(counts.retained_effect_shadows, 1);
        assert_eq!(counts.retained_effect_images, 1);
        assert_eq!(counts.retained_entity_layers, 1);
        assert_eq!(counts.legacy_scene_entities, 1);
        assert_eq!(counts.entity_atlases, 1);
        assert_eq!(counts.map_render_tiles, 1);
        assert_eq!(counts.map_spawned_entities, 1);
        assert_eq!(counts.mine_nodes, 1);
        assert_eq!(counts.lighting_layers, 1);
        assert_eq!(counts.lighting_images, 1);
        assert_eq!(counts.additive_cache_entries, 1);
        assert_eq!(counts.additive_cache_live_entries, 1);
        assert_eq!(counts.additive_asset_count, 1);

        let encoded = native_soak_metrics_json(4_242, 12_345, &counts);
        let payload: serde_json::Value =
            serde_json::from_str(&encoded).expect("native soak metrics should be valid JSON");
        assert_eq!(payload["processId"], 4_242);
        assert_eq!(payload["timestampMs"], 12_345);
        assert_eq!(payload["snapshotEffects"], 1);
        assert_eq!(payload["additiveCacheEntries"], 1);
        assert_eq!(payload["additiveCacheLiveEntries"], 1);
        assert!(!encoded.contains('\n'));

        additive_materials.remove(cached_material.id());
        let stale = native_soak_counts(
            &registry,
            &effect_state,
            &additive_cache,
            &additive_materials,
        );
        assert_eq!(stale.additive_cache_entries, 1);
        assert_eq!(stale.additive_cache_live_entries, 0);
        assert_eq!(stale.additive_asset_count, 0);
    }
}
