mod interpolation;
mod motion;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use bevy::asset::{AssetMetaCheck, AssetPlugin, RenderAssetUsages};
use bevy::image::{Image, ImagePlugin, TextureAtlas, TextureAtlasLayout};
use bevy::math::{URect, UVec2};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::{CompositeAlphaMode, WindowResolution};
use js_sys::Function;
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

#[cfg(target_arch = "wasm32")]
const WINDOW_TRANSPARENT: bool = true;
#[cfg(not(target_arch = "wasm32"))]
const WINDOW_TRANSPARENT: bool = false;

thread_local! {
    static STATUS_SINK: RefCell<Option<Function>> = const { RefCell::new(None) };
    static PENDING_WORLD_STATE: RefCell<Option<WorldSnapshot>> = const { RefCell::new(None) };
    static PENDING_ENTITY_RENDER_STATE: RefCell<Option<EntityRenderState>> = const { RefCell::new(None) };
    static PENDING_ENTITY_RENDER_ATLASES: RefCell<Vec<PendingEntityRenderAtlasImage>> = const { RefCell::new(Vec::new()) };
    static PENDING_MAP_RENDER_STATE: RefCell<Option<MapRenderState>> = const { RefCell::new(None) };
    static PENDING_MAP_RENDER_ATLASES: RefCell<Vec<PendingMapRenderAtlasImage>> = const { RefCell::new(Vec::new()) };
    static PENDING_MAP_CAMERA_OFFSET: Cell<(f32, f32)> = const { Cell::new((0.0, 0.0)) };
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

/// Map-tile atlas registry. Deliberately SEPARATE from
/// `RuntimeEntityRenderAtlases` so the entity render path's atlas-layout
/// retain logic (which evicts layouts not referenced by the entity snapshot)
/// cannot evict the map's layouts, and vice versa.
#[derive(Resource, Default, Clone)]
struct RuntimeMapRenderAtlases {
    images: HashMap<String, Handle<Image>>,
    layouts: HashMap<String, (Handle<TextureAtlasLayout>, HashMap<String, usize>)>,
}

/// Per-frame sub-tile camera scroll offset (screen-stage pixels). In the
/// fold-in camera model the map tiles already include the offset in their
/// `left`/`top`, so this stays (0, 0); the resource + `apply_map_camera_offset`
/// system exist for the alternative root-offset model.
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
    map: MapSceneCache,
    map_render: MapRenderSceneCache,
    mine_nodes: HashMap<(i32, i32), MineNodeHandles>,
}

#[derive(Default)]
struct MapSceneCache {
    blueprint: Option<MapSceneBlueprint>,
    spawned: Vec<Entity>,
}

/// Cache for the Bevy-native map-tile renderer: the single root entity holding
/// one child sprite per tile, plus the last-applied `MapRenderState` so the
/// low-cadence tile list (changes only on cell-change/animation) is diffed and
/// the whole root is despawned + respawned only when the tiles actually change.
#[derive(Default)]
struct MapRenderSceneCache {
    root: Option<Entity>,
    applied: Option<MapRenderState>,
    tiles: HashMap<String, Entity>,
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
}

#[derive(Clone)]
struct EntityRenderAtlasHandle {
    layout: Handle<TextureAtlasLayout>,
    rects: HashMap<String, usize>,
    image_key: Option<String>,
    image: Option<Handle<Image>>,
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
    is_player: bool,
    #[serde(default)]
    dead: bool,
    #[serde(default)]
    layers: Vec<EntityRenderLayer>,
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
/// with the DOM `WebGl2MapAtlasLayer`. `PartialEq` lets `sync_map_render` diff
/// against the last-applied snapshot and rebuild only on tile-list change.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapRenderState {
    enabled: bool,
    stage_width: f32,
    stage_height: f32,
    /// Atlas page descriptors (key + page dims + the source rects within the
    /// page). Carries the rect geometry the per-tile `atlas_rect_key` indexes
    /// into; mirrors `EntityRenderState.atlases` so the same layout-building
    /// logic applies. Optional/additive — pixels arrive separately via
    /// `setMir2MapRenderAtlas`.
    #[serde(default)]
    atlases: Vec<MapRenderAtlas>,
    #[serde(default)]
    tiles: Vec<MapTile>,
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
    #[serde(default)]
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

struct PendingMapRenderAtlasImage {
    key: String,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
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

    PENDING_MAP_RENDER_ATLASES.with(|pending| {
        pending.borrow_mut().push(PendingMapRenderAtlasImage {
            key,
            width,
            height,
            pixels,
        });
    });
}

#[wasm_bindgen(js_name = setMir2MapCameraOffset)]
pub fn set_mir2_map_camera_offset(x: f32, y: f32) {
    PENDING_MAP_CAMERA_OFFSET.with(|cell| cell.set((x, y)));
}

#[wasm_bindgen(js_name = bootMir2Runtime)]
pub fn boot_mir2_runtime() {
    console_error_panic_hook::set_once();

    publish_status("runtime-entered", "Bevy runtime entry reached");

    let mut app = App::new();
    app.insert_resource(ClearColor(FLOOR_COLOR))
        .insert_resource(RuntimeWorldState::default())
        .insert_resource(RuntimeEntityRenderState::default())
        .insert_resource(RuntimeEntityRenderAtlases::default())
        .insert_resource(RuntimeMapRenderState::default())
        .insert_resource(RuntimeMapRenderAtlases::default())
        .insert_resource(RuntimeMapCameraOffset::default())
        .insert_resource(SceneRegistry::default())
        .insert_resource(interpolation::SnapshotBuffer::default())
        .insert_resource(motion::EntityMotionTable::default())
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: ".".to_owned(),
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        canvas: Some("#mir2-web3-canvas".to_owned()),
                        composite_alpha_mode: WINDOW_COMPOSITE_ALPHA_MODE,
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        resolution: WindowResolution::new(1280, 720),
                        title: "mir2-web3".to_owned(),
                        transparent: WINDOW_TRANSPARENT,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (
                ingest_pending_world_state,
                motion::update_entity_motion_table,
                ingest_pending_entity_render_state,
                ingest_pending_entity_render_atlases,
                ingest_pending_map_render_state,
                update_runtime_camera_offset,
                ingest_pending_map_render_atlases,
                sync_map_render,
                sync_map_scene,
                sync_entities,
                sync_mine_nodes,
                sync_entity_render_layers,
                follow_player,
                apply_map_camera_offset,
            )
                .chain(),
        );

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
) {
    PENDING_WORLD_STATE.with(|pending| {
        if let Some(snapshot) = pending.borrow_mut().take() {
            // Build a buffered entry for interpolation.  The receipt time is
            // the Bevy elapsed seconds; if the TS producer supplied
            // `clientTimeMs` we note it but still use local time as the
            // authoritative clock so browser-clock skew can't distort lerp.
            let receipt_secs = time.elapsed_secs_f64();
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
    });
}

fn ingest_pending_entity_render_state(mut state: ResMut<RuntimeEntityRenderState>) {
    PENDING_ENTITY_RENDER_STATE.with(|pending| {
        if let Some(snapshot) = pending.borrow_mut().take() {
            state.snapshot = Some(snapshot);
        }
    });
}

fn ingest_pending_entity_render_atlases(
    mut atlas_resource: ResMut<RuntimeEntityRenderAtlases>,
    mut images: ResMut<Assets<Image>>,
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
}

fn ingest_pending_map_render_state(
    mut state: ResMut<RuntimeMapRenderState>,
    mut camera_offset: ResMut<RuntimeMapCameraOffset>,
) {
    PENDING_MAP_RENDER_STATE.with(|pending| {
        if let Some(snapshot) = pending.borrow_mut().take() {
            state.snapshot = Some(snapshot);
        }
    });
    // The camera offset is a cheap per-frame scalar pair; pull the latest value
    // each frame so `apply_map_camera_offset` (root-offset model) stays current.
    let (x, y) = PENDING_MAP_CAMERA_OFFSET.with(|cell| cell.get());
    camera_offset.x = x;
    camera_offset.y = y;
}

fn update_runtime_camera_offset(
    state: Res<RuntimeWorldState>,
    motion_table: Res<motion::EntityMotionTable>,
    mut camera_offset: ResMut<RuntimeMapCameraOffset>,
) {
    let Some(snapshot) = &state.snapshot else {
        camera_offset.x = 0.0;
        camera_offset.y = 0.0;
        return;
    };
    let Some(player_object_id) = &snapshot.player_object_id else {
        camera_offset.x = 0.0;
        camera_offset.y = 0.0;
        return;
    };
    let Some(entry) = motion_table.get(player_object_id) else {
        camera_offset.x = 0.0;
        camera_offset.y = 0.0;
        return;
    };

    let entity_offset = motion::compute_motion_offset(entry, motion_table.now_ms, 48.0, 32.0);
    camera_offset.x = -entity_offset.x;
    camera_offset.y = -entity_offset.y;
}

fn ingest_pending_map_render_atlases(
    mut atlas_resource: ResMut<RuntimeMapRenderAtlases>,
    mut images: ResMut<Assets<Image>>,
) {
    PENDING_MAP_RENDER_ATLASES.with(|pending| {
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
}

/// All map tiles render at z ≈ MAP_TILE_Z_BASE, far below entities (entity
/// layers sit at z ≈ layer.z/100000 ≈ 0.0..0.5, decor 0.3). The per-tile
/// `z * MAP_TILE_Z_STEP` term preserves the relative back→object→middle→
/// front→anim ordering among map tiles without ever crossing into entity z.
const MAP_TILE_Z_BASE: f32 = -50.0;
const MAP_TILE_Z_STEP: f32 = 1.0e-7;

/// Bevy-native map-tile renderer (Stage 1). Builds/refreshes the atlas-page
/// `TextureAtlasLayout`s (mirrors `sync_entity_render_atlas_layouts`), then
/// diffs the pushed `MapRenderState` against the last-applied snapshot. The
/// tile list changes only on cell-change/animation (low cadence), so on change
/// the whole map root is despawned (recursive) and respawned with one child
/// `Sprite` per tile — cheap enough for Stage 1 and never per-frame. When map
/// render is disabled/None this clears any spawned root and returns.
fn sync_map_render(
    mut commands: Commands,
    map_state: Res<RuntimeMapRenderState>,
    map_camera_offset: Res<RuntimeMapCameraOffset>,
    mut atlas_assets: ResMut<RuntimeMapRenderAtlases>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut registry: ResMut<SceneRegistry>,
    mut transform_query: Query<&mut Transform>,
    mut sprite_query: Query<&mut Sprite>,
) {
    let active = map_state
        .snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.enabled);
    if !active {
        if let Some(root) = registry.map_render.root.take() {
            commands.entity(root).despawn();
        }
        registry.map_render.applied = None;
        registry.map_render.tiles.clear();
        return;
    }

    let snapshot = map_state.snapshot.as_ref().unwrap();

    sync_map_render_atlas_layouts(snapshot, &mut atlas_assets, &mut texture_atlas_layouts);

    if registry.map_render.applied.as_ref() == Some(snapshot)
        && registry.map_render.tiles.len() == snapshot.tiles.len()
    {
        return;
    }

    let root_translation = Vec3::new(map_camera_offset.x, -map_camera_offset.y, MAP_TILE_Z_BASE);
    let root = match registry.map_render.root {
        Some(root) => {
            if let Ok(mut transform) = transform_query.get_mut(root) {
                transform.translation = root_translation;
            }
            root
        }
        None => {
            // The root MUST carry Visibility: child sprites have Visibility::Inherited, so
            // without a visibility ancestor their InheritedVisibility defaults to false.
            let root = commands
                .spawn((
                    Transform::from_translation(root_translation),
                    Visibility::Visible,
                ))
                .id();
            registry.map_render.root = Some(root);
            root
        }
    };

    let mut live_keys = HashSet::new();
    let mut spawned_count = 0usize;
    let mut updated_count = 0usize;
    let mut skipped_count = 0usize;

    for (index, tile) in snapshot.tiles.iter().enumerate() {
        let key = map_tile_key(index, tile);
        live_keys.insert(key.clone());
        let image_binding = map_render_image_binding(tile, &atlas_assets);
        let (image, texture_atlas) = match image_binding {
            Some(binding) => binding,
            None => {
                skipped_count += 1;
                continue;
            }
        };
        let local_transform = map_tile_transform(snapshot, tile);
        let custom_size = Some(Vec2::new(tile.width, tile.height));
        let color = Color::srgba(1.0, 1.0, 1.0, tile.opacity.unwrap_or(1.0));

        if let Some(entity) = registry.map_render.tiles.get(&key).copied() {
            if let Ok(mut transform) = transform_query.get_mut(entity) {
                *transform = local_transform;
            }
            if let Ok(mut sprite) = sprite_query.get_mut(entity) {
                sprite.image = image;
                sprite.texture_atlas = Some(texture_atlas);
                sprite.custom_size = custom_size;
                sprite.color = color;
            }
            updated_count += 1;
            continue;
        }

        let mut spawned_entity = None;
        commands.entity(root).with_children(|parent| {
            spawned_entity = Some(
                parent
                    .spawn((
                        Sprite {
                            image,
                            texture_atlas: Some(texture_atlas),
                            custom_size,
                            color,
                            ..default()
                        },
                        local_transform,
                    ))
                    .id(),
            );
        });
        if let Some(entity) = spawned_entity {
            registry.map_render.tiles.insert(key, entity);
            spawned_count += 1;
        }
    }

    let stale_keys: Vec<String> = registry
        .map_render
        .tiles
        .keys()
        .filter(|key| !live_keys.contains(*key))
        .cloned()
        .collect();
    let removed_count = stale_keys.len();
    for key in stale_keys {
        if let Some(entity) = registry.map_render.tiles.remove(&key) {
            commands.entity(entity).despawn();
        }
    }

    registry.map_render.applied = Some(snapshot.clone());
    publish_status(
        "map-render-synced",
        &format!(
            "Applied {} map tiles (+{} ~{} -{} skipped {})",
            snapshot.tiles.len(),
            spawned_count,
            updated_count,
            removed_count,
            skipped_count,
        ),
    );
}

fn map_tile_key(index: usize, tile: &MapTile) -> String {
    if !tile.key.is_empty() {
        return tile.key.clone();
    }
    format!(
        "{}:{}:{}:{}:{}",
        tile.atlas_key, tile.atlas_rect_key, tile.left, tile.top, index
    )
}

fn map_tile_transform(snapshot: &MapRenderState, tile: &MapTile) -> Transform {
    let local_x = tile.left + tile.width * 0.5 - snapshot.stage_width * 0.5;
    let local_y = snapshot.stage_height * 0.5 - (tile.top + tile.height * 0.5);
    let local_z = tile.z * MAP_TILE_Z_STEP;
    Transform::from_xyz(local_x, local_y, local_z)
}

/// Build the `TextureAtlasLayout` for each map atlas page not already cached.
/// Copy of `sync_entity_render_atlas_layouts`, writing into the SEPARATE
/// `RuntimeMapRenderAtlases.layouts` registry so the entity retain logic can't
/// evict map layouts. Map atlas pages are stable across the session, so no
/// retain/evict pass is needed here.
fn sync_map_render_atlas_layouts(
    snapshot: &MapRenderState,
    atlas_assets: &mut RuntimeMapRenderAtlases,
    texture_atlas_layouts: &mut Assets<TextureAtlasLayout>,
) {
    for atlas in &snapshot.atlases {
        if atlas_assets.layouts.contains_key(&atlas.key) {
            continue;
        }
        let mut layout = TextureAtlasLayout::new_empty(UVec2::new(atlas.width, atlas.height));
        let mut rects = HashMap::new();
        for rect in &atlas.rects {
            let index = layout.add_texture(URect {
                min: UVec2::new(rect.x, rect.y),
                max: UVec2::new(rect.x + rect.width, rect.y + rect.height),
            });
            rects.insert(rect.key.clone(), index);
        }
        let layout = texture_atlas_layouts.add(layout);
        atlas_assets
            .layouts
            .insert(atlas.key.clone(), (layout, rects));
    }
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

/// Root-offset camera model hook: set the map root's translation from the
/// per-frame `RuntimeMapCameraOffset` each frame. In the fold-in model the
/// offset is (0, 0) (tiles already carry it in `left`/`top`), so this leaves
/// the root at the screen-stage center. Kept additive so the alternative model
/// is a one-line producer change away.
fn apply_map_camera_offset(
    map_camera_offset: Res<RuntimeMapCameraOffset>,
    registry: Res<SceneRegistry>,
    mut transform_query: Query<&mut Transform>,
) {
    let Some(root) = registry.map_render.root else {
        return;
    };
    if let Ok(mut transform) = transform_query.get_mut(root) {
        transform.translation.x = map_camera_offset.x;
        transform.translation.y = -map_camera_offset.y;
        transform.translation.z = MAP_TILE_Z_BASE;
    }
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
    entity_render_state: Res<RuntimeEntityRenderState>,
    map_camera_offset: Res<RuntimeMapCameraOffset>,
    motion_table: Res<motion::EntityMotionTable>,
    mut registry: ResMut<SceneRegistry>,
    mut transform_query: Query<&mut Transform>,
    mut sprite_query: Query<&mut Sprite>,
) {
    let Some(snapshot) = &entity_render_state.snapshot else {
        clear_entity_render_layers(&mut commands, &mut registry);
        return;
    };

    if !snapshot.enabled {
        clear_entity_render_layers(&mut commands, &mut registry);
        return;
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
        for layer in &entity.layers {
            let layer_key = if layer.key.is_empty() {
                format!("{}:{}", entity.object_id, layer.path)
            } else {
                layer.key.clone()
            };
            alive.insert(layer_key.clone());
            let position = entity_render_layer_position(
                snapshot,
                entity,
                layer,
                &map_camera_offset,
                &motion_table,
            );
            let opacity = layer
                .opacity
                .unwrap_or(if entity.dead { 0.45 } else { 1.0 });
            let image_binding =
                entity_render_image_binding(layer, &asset_server, &atlas_assets, &registry);

            if let Some(handle) = registry.entity_render_layers.get_mut(&layer_key) {
                if handle.image_key != image_binding.image_key
                    || handle.atlas_key != image_binding.atlas_key
                    || handle.atlas_rect_key != image_binding.atlas_rect_key
                {
                    if let Ok(mut sprite) = sprite_query.get_mut(handle.entity) {
                        sprite.image = image_binding.image.clone();
                        sprite.texture_atlas = image_binding.texture_atlas.clone();
                        sprite.rect = None;
                    }
                    handle.image_key = image_binding.image_key.clone();
                    handle.atlas_key = image_binding.atlas_key.clone();
                    handle.atlas_rect_key = image_binding.atlas_rect_key.clone();
                }
                if let Ok(mut sprite) = sprite_query.get_mut(handle.entity) {
                    sprite.custom_size =
                        Some(Vec2::new(layer.width.max(1.0), layer.height.max(1.0)));
                    sprite.color = Color::srgba(1.0, 1.0, 1.0, opacity.clamp(0.0, 1.0));
                    sprite.texture_atlas = image_binding.texture_atlas.clone();
                    sprite.rect = None;
                }
                if let Ok(mut transform) = transform_query.get_mut(handle.entity) {
                    transform.translation = position;
                }
                continue;
            }

            let sprite_entity = commands
                .spawn((
                    MirEntityRenderLayer,
                    Sprite {
                        image: image_binding.image.clone(),
                        texture_atlas: image_binding.texture_atlas.clone(),
                        custom_size: Some(Vec2::new(layer.width.max(1.0), layer.height.max(1.0))),
                        color: Color::srgba(1.0, 1.0, 1.0, opacity.clamp(0.0, 1.0)),
                        ..default()
                    },
                    Transform::from_translation(position),
                ))
                .id();
            registry.entity_render_layers.insert(
                layer_key,
                EntityRenderLayerHandle {
                    entity: sprite_entity,
                    image_key: image_binding.image_key,
                    atlas_key: image_binding.atlas_key,
                    atlas_rect_key: image_binding.atlas_rect_key,
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
        }
    }
}

struct EntityRenderImageBinding {
    image: Handle<Image>,
    image_key: String,
    atlas_key: Option<String>,
    atlas_rect_key: Option<String>,
    texture_atlas: Option<TextureAtlas>,
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
        if registry.entity_render_atlases.contains_key(&atlas.key) {
            continue;
        }

        let mut layout = TextureAtlasLayout::new_empty(UVec2::new(atlas.width, atlas.height));
        let mut rects = HashMap::new();
        for rect in &atlas.rects {
            let index = layout.add_texture(URect {
                min: UVec2::new(rect.x, rect.y),
                max: UVec2::new(rect.x + rect.width, rect.y + rect.height),
            });
            rects.insert(rect.key.clone(), index);
        }
        let layout = texture_atlas_layouts.add(layout);
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
        registry.entity_render_atlases.insert(
            atlas.key.clone(),
            EntityRenderAtlasHandle {
                layout,
                rects,
                image_key,
                image,
            },
        );
    }

    registry
        .entity_render_atlases
        .retain(|key, _| alive.contains(key));
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
    }
}

fn clear_entity_render_layers(commands: &mut Commands, registry: &mut SceneRegistry) {
    for (_, handle) in registry.entity_render_layers.drain() {
        commands.entity(handle.entity).despawn();
    }
    registry.entity_render_atlases.clear();
}

fn entity_render_layer_position(
    snapshot: &EntityRenderState,
    entity: &EntityRenderEntry,
    layer: &EntityRenderLayer,
    map_camera_offset: &RuntimeMapCameraOffset,
    motion_table: &motion::EntityMotionTable,
) -> Vec3 {
    let entity_motion_offset = if entity.is_player {
        Vec2::ZERO
    } else {
        motion_table
            .get(&entity.object_id)
            .map(|entry| motion::compute_motion_offset(entry, motion_table.now_ms, 48.0, 32.0))
            .unwrap_or(Vec2::ZERO)
    };
    let camera_x = if entity.is_player {
        0.0
    } else {
        map_camera_offset.x
    };
    let camera_y = if entity.is_player {
        0.0
    } else {
        map_camera_offset.y
    };
    let left = layer.left + camera_x + entity_motion_offset.x;
    let top = layer.top + camera_y + entity_motion_offset.y;
    let center_x = left + layer.width * 0.5;
    let center_y = top + layer.height * 0.5;

    Vec3::new(
        center_x - snapshot.stage_width * 0.5,
        snapshot.stage_height * 0.5 - center_y,
        layer.z / 100_000.0,
    )
}

fn browser_asset_path(path: &str) -> String {
    path.trim_start_matches('/')
        .split('?')
        .next()
        .unwrap_or(path)
        .to_owned()
}

fn follow_player(
    state: Res<RuntimeWorldState>,
    entity_render_state: Res<RuntimeEntityRenderState>,
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
        camera_transform.translation.x = 0.0;
        camera_transform.translation.y = 0.0;
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
