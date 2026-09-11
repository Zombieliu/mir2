//! Android producer for the shared Bevy `EntityRenderState` contract.
//!
//! The gateway snapshot remains authoritative for object identity and grid
//! position. This adapter only resolves the snapshot's Crystal sprite metadata
//! against the same immutable entity atlas used by the Web client.

use crate::{world_assets::WorldAssetError, world_projection::ProjectedScene};
use mir2_bevy_runtime::entity_animation::{
    AnimationAction, AnimationCatalog, Direction, EntityKind, FrameDescriptor,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    io::Cursor,
};

pub(crate) const ENTITY_ATLAS_MANIFEST_ASSET: &str = "bevy-entity-atlases/manifest.json";
const DNITEMS_META_ASSET: &str = "original-ui/DNItems/meta.json";
const ENTITY_ATLAS_KIND: &str = "mir2-bevy-entity-atlas-manifest";
const ENTITY_ATLAS_SCHEMA_VERSION: u32 = 2;
const STAGE_WIDTH: f32 = 1024.0;
const STAGE_HEIGHT: f32 = 768.0;
const CELL_WIDTH: f32 = 48.0;
const CELL_HEIGHT: f32 = 32.0;
const ENTITY_LEFT_ORIGIN: f32 = 480.0;
const ENTITY_TOP_ORIGIN: f32 = 352.0;
const MAX_MANIFEST_BYTES: usize = 16 * 1024 * 1024;
const MAX_PAGE_COUNT: usize = 16;
const MAX_RECT_COUNT: usize = 100_000;
const MAX_PAGE_PNG_BYTES: usize = 8 * 1024 * 1024;
const MAX_PAGE_PIXELS: usize = 4 * 1024 * 1024;
const MAX_SELECTED_RGBA_BYTES: usize = 128 * 1024 * 1024;
const MAX_RENDER_STATE_BYTES: usize = 64 * 1024 * 1024;
const MAX_DNITEMS_META_BYTES: usize = 4 * 1024 * 1024;
const MAX_DNITEM_FRAMES: usize = 6_000;
const DIRECTIONS: [&str; 8] = [
    "Up",
    "UpRight",
    "Right",
    "DownRight",
    "Down",
    "DownLeft",
    "Left",
    "UpLeft",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityAtlasManifest {
    schema_version: u32,
    kind: String,
    atlases: Vec<EntityAtlasManifestEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityAtlasManifestEntry {
    key: String,
    width: u32,
    height: u32,
    pages: Vec<EntityAtlasPage>,
    rects: Vec<EntityAtlasRect>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityAtlasPage {
    image_file: String,
    width: u32,
    height: u32,
    image_bytes: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntityAtlasRect {
    key: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    offset_x: i32,
    offset_y: i32,
    frame_index: i32,
    #[serde(default)]
    page_index: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    player_object_id: String,
    entities: Vec<SnapshotEntity>,
    #[serde(default)]
    ground_drops: Vec<SnapshotGroundDrop>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotGroundDrop {
    object_id: String,
    x: i32,
    y: i32,
    image: u16,
}

#[derive(Debug, Deserialize)]
struct DnItemsMetadata {
    version: u32,
    count: usize,
    frames: Vec<DnItemMetadataFrame>,
}

#[derive(Debug, Deserialize)]
struct DnItemMetadataFrame {
    index: u16,
    width: u32,
    height: u32,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotEntity {
    object_id: String,
    kind: String,
    x: i32,
    y: i32,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    dead: bool,
    #[serde(default)]
    class_key: Option<String>,
    #[serde(default, rename = "class")]
    class_name: Option<String>,
    #[serde(default)]
    sprite: Option<SnapshotSprite>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotSprite {
    body_library: String,
    #[serde(default)]
    hair_library: Option<String>,
    #[serde(default)]
    weapon_library: Option<String>,
    #[serde(default)]
    weapon_library_secondary: Option<String>,
    #[serde(default)]
    alt_body_library: Option<String>,
    #[serde(default)]
    alt_hair_library: Option<String>,
    #[serde(default)]
    alt_weapon_library: Option<String>,
    #[serde(default)]
    alt_weapon_library_secondary: Option<String>,
    #[serde(default)]
    mount_library: Option<String>,
    #[serde(default)]
    frame_base_offset: i32,
    #[serde(default)]
    weapon_frame_offset: Option<i32>,
    #[serde(default)]
    alt_frame_base_offset: Option<i32>,
    #[serde(default)]
    alt_weapon_frame_offset: Option<i32>,
    #[serde(default = "default_frame_count")]
    frame_count: i32,
    #[serde(default = "default_direction_stride")]
    direction_stride: i32,
    #[serde(default)]
    mount_frame_offset: Option<i32>,
}

const fn default_frame_count() -> i32 {
    4
}

const fn default_direction_stride() -> i32 {
    4
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityRenderState {
    #[serde(rename = "_nativeWorldRequest")]
    native_world_request: u64,
    enabled: bool,
    stage_width: f32,
    stage_height: f32,
    center_x: i32,
    center_y: i32,
    unresolved_entity_count: usize,
    atlases: Vec<EntityRenderAtlas>,
    entities: Vec<EntityRenderEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityRenderAtlas {
    key: String,
    width: u32,
    height: u32,
    rects: Vec<EntityRenderAtlasRect>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityRenderAtlasRect {
    key: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityRenderEntry {
    object_id: String,
    is_self: bool,
    grid_x: i32,
    grid_y: i32,
    layers: Vec<EntityRenderLayer>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityDirectionState {
    #[serde(rename = "_nativeWorldRequest")]
    native_world_request: u64,
    entities: Vec<EntityDirectionEntry>,
    ground_item_frames: BTreeMap<u16, GroundItemFrame>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GroundItemFrame {
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityDirectionEntry {
    object_id: String,
    prototype: Value,
    direction_layers: BTreeMap<String, Vec<EntityRenderLayer>>,
    action_layers: BTreeMap<String, EntityActionLayers>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityActionLayers {
    interval_ms: u64,
    frames: Vec<Vec<EntityRenderLayer>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntityRenderLayer {
    key: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    atlas_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    atlas_rect_key: Option<String>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    z: f32,
    opacity: f32,
}

#[derive(Debug)]
pub(crate) struct DecodedEntityAtlasPage {
    pub(crate) key: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct EntityRenderProduct {
    pub(crate) json: String,
    pub(crate) live_directions_json: String,
    pub(crate) pages: Vec<DecodedEntityAtlasPage>,
    pub(crate) entity_count: usize,
    pub(crate) layer_count: usize,
    pub(crate) unresolved_entity_count: usize,
    pub(crate) compressed_bytes: usize,
    pub(crate) rgba_bytes: usize,
}

fn safe_component_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 192
        && value.split('/').all(|part| {
            !part.is_empty()
                && !matches!(part, "." | "..")
                && part.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b' ' | b'.')
                })
        })
}

fn normalize_library(value: &str) -> Option<String> {
    let value = value
        .replace('\\', "/")
        .trim_start_matches('/')
        .strip_prefix("original-ui/")
        .unwrap_or_else(|| value.trim_start_matches('/'))
        .trim_matches('/')
        .to_owned();
    safe_component_path(&value).then_some(value)
}

fn safe_page_file(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.ends_with(".png")
        && value.is_ascii()
        && !value.contains(['/', '\\', '\0'])
        && !value.contains("..")
}

fn page_key(atlas_key: &str, page_index: usize) -> String {
    if page_index == 0 {
        atlas_key.to_owned()
    } else {
        format!("{atlas_key}#p{page_index}")
    }
}

fn direction_index(direction: Option<&str>) -> i32 {
    match direction {
        Some("Up") => 0,
        Some("UpRight") => 1,
        Some("Right") => 2,
        Some("DownRight") => 3,
        Some("Down") => 4,
        Some("DownLeft") => 5,
        Some("Left") => 6,
        Some("UpLeft") => 7,
        _ => 4,
    }
}

fn animation_direction(direction: &str) -> Direction {
    match direction {
        "Up" => Direction::Up,
        "UpRight" => Direction::UpRight,
        "Right" => Direction::Right,
        "DownRight" => Direction::DownRight,
        "DownLeft" => Direction::DownLeft,
        "Left" => Direction::Left,
        "UpLeft" => Direction::UpLeft,
        _ => Direction::Down,
    }
}

fn animation_kind(kind: &str) -> Option<EntityKind> {
    match kind {
        "selfPlayer" | "player" | "hero" => Some(EntityKind::Player),
        "monster" => Some(EntityKind::Monster),
        "npc" => Some(EntityKind::Npc),
        _ => None,
    }
}

fn animation_action_name(action: AnimationAction) -> &'static str {
    match action {
        AnimationAction::Walking => "walking",
        AnimationAction::Running => "running",
        AnimationAction::Harvest => "harvest",
        AnimationAction::Attack1 => "attack1",
        AnimationAction::Attack2 => "attack2",
        AnimationAction::Attack3 => "attack3",
        AnimationAction::Attack4 => "attack4",
        AnimationAction::AttackRange1 => "attackRange1",
        AnimationAction::AttackRange2 => "attackRange2",
        AnimationAction::DashAttack => "dashAttack",
        AnimationAction::Spell => "spell",
        AnimationAction::Struck => "struck",
        AnimationAction::Die => "die",
        AnimationAction::Dead => "dead",
        AnimationAction::Skeleton => "skeleton",
        AnimationAction::Revive => "revive",
        _ => "",
    }
}

fn uses_archer_alt(action: AnimationAction) -> bool {
    matches!(
        action,
        AnimationAction::Walking
            | AnimationAction::Running
            | AnimationAction::AttackRange1
            | AnimationAction::AttackRange2
    )
}

fn uses_assassin_alt(action: AnimationAction) -> bool {
    matches!(
        action,
        AnimationAction::Standing
            | AnimationAction::Walking
            | AnimationAction::Running
            | AnimationAction::Attack1
            | AnimationAction::Attack2
            | AnimationAction::Attack3
            | AnimationAction::Attack4
            | AnimationAction::DashAttack
            | AnimationAction::Spell
            | AnimationAction::Struck
            | AnimationAction::Die
    )
}

fn archer_player_catalog() -> AnimationCatalog {
    let mut catalog = AnimationCatalog::crystal_player();
    for (action, descriptor) in [
        (
            AnimationAction::Walking,
            FrameDescriptor::from_crystal(0, 6, 0, 100, false),
        ),
        (
            AnimationAction::Running,
            FrameDescriptor::from_crystal(48, 6, 0, 100, false),
        ),
        (
            AnimationAction::AttackRange2,
            FrameDescriptor::from_crystal(160, 8, 0, 100, false),
        ),
    ] {
        catalog
            .insert(action, descriptor)
            .expect("bounded Crystal archer descriptor");
    }
    catalog
}

fn mounted_player_catalog() -> AnimationCatalog {
    let mut catalog = AnimationCatalog::new();
    for (action, descriptor) in [
        (
            AnimationAction::Standing,
            FrameDescriptor::from_crystal(416, 4, 0, 500, false),
        ),
        (
            AnimationAction::Harvest,
            FrameDescriptor::from_crystal(416, 4, 0, 500, false),
        ),
        (
            AnimationAction::Walking,
            FrameDescriptor::from_crystal(448, 8, 0, 100, false),
        ),
        (
            AnimationAction::Running,
            FrameDescriptor::from_crystal(512, 6, 0, 100, false),
        ),
        (
            AnimationAction::Struck,
            FrameDescriptor::from_crystal(560, 3, 0, 100, false),
        ),
        (
            AnimationAction::Die,
            FrameDescriptor::from_crystal(384, 4, 0, 100, false),
        ),
        (
            AnimationAction::Dead,
            FrameDescriptor::from_crystal(387, 1, 3, 1000, false),
        ),
        (
            AnimationAction::Revive,
            FrameDescriptor::from_crystal(384, 4, 0, 100, true),
        ),
    ] {
        catalog
            .insert(action, descriptor)
            .expect("bounded Crystal mounted descriptor");
    }
    let attack = FrameDescriptor::from_crystal(584, 6, 0, 100, false);
    for action in [
        AnimationAction::Attack1,
        AnimationAction::Attack2,
        AnimationAction::Attack3,
        AnimationAction::Attack4,
        AnimationAction::AttackRange1,
        AnimationAction::AttackRange2,
        AnimationAction::Spell,
    ] {
        catalog
            .insert(action, attack)
            .expect("bounded Crystal mounted attack descriptor");
    }
    catalog
}

fn weapon_is_rear(direction: Option<&str>) -> bool {
    matches!(direction, Some("Left" | "Up" | "UpLeft" | "DownLeft"))
}

fn assassin_primary_is_front(direction: &str) -> bool {
    matches!(direction, "UpRight" | "Right" | "DownRight" | "Down")
}

fn atlas_source_path(rect_key: &str) -> Option<String> {
    let (path, dimensions) = rect_key.split_once('|')?;
    let path = path.replace("%20", " ");
    let relative = path.strip_prefix("/original-ui/")?;
    let (library, frame_file) = relative.rsplit_once('/')?;
    if path.contains('%')
        || !safe_component_path(library)
        || frame_file
            .strip_suffix(".png")
            .and_then(|frame| frame.parse::<i32>().ok())
            .filter(|frame| *frame >= 0)
            .is_none()
        || !dimensions.contains('x')
    {
        return None;
    }
    Some(path)
}

fn frame_path(library: &str, frame: i32) -> String {
    format!("/original-ui/{library}/{frame}.png")
}

fn parse_ground_item_frames(
    bytes: &[u8],
) -> Result<BTreeMap<u16, GroundItemFrame>, WorldAssetError> {
    if bytes.is_empty() || bytes.len() > MAX_DNITEMS_META_BYTES {
        return Err(WorldAssetError::new(
            "DNItems metadata byte count is out of bounds",
        ));
    }
    let metadata: DnItemsMetadata = serde_json::from_slice(bytes)
        .map_err(|error| WorldAssetError::new(format!("DNItems metadata rejected: {error}")))?;
    if metadata.version != 3
        || metadata.count != metadata.frames.len()
        || metadata.frames.is_empty()
        || metadata.frames.len() > MAX_DNITEM_FRAMES
    {
        return Err(WorldAssetError::new(
            "DNItems metadata version or frame count is invalid",
        ));
    }
    let mut frames = BTreeMap::new();
    for frame in metadata.frames {
        if frame.path != format!("/original-ui/DNItems/{}.png", frame.index)
            || frame.width > 256
            || frame.height > 256
            || frames.contains_key(&frame.index)
        {
            return Err(WorldAssetError::new(
                "DNItems metadata frame is invalid or duplicated",
            ));
        }
        if frame.width != 0 && frame.height != 0 {
            frames.insert(
                frame.index,
                GroundItemFrame {
                    width: frame.width,
                    height: frame.height,
                },
            );
        }
    }
    Ok(frames)
}

fn fallback_sprite(entity: &SnapshotEntity) -> Option<SnapshotSprite> {
    matches!(entity.kind.as_str(), "selfPlayer" | "player").then(|| SnapshotSprite {
        body_library: "CArmour/00".into(),
        hair_library: None,
        weapon_library: None,
        weapon_library_secondary: None,
        alt_body_library: None,
        alt_hair_library: None,
        alt_weapon_library: None,
        alt_weapon_library_secondary: None,
        mount_library: None,
        frame_base_offset: 0,
        weapon_frame_offset: None,
        alt_frame_base_offset: None,
        alt_weapon_frame_offset: None,
        frame_count: 4,
        direction_stride: 4,
        mount_frame_offset: None,
    })
}

fn prototype_descriptor(kind: &str, class_key: &str, dead: bool, sprite: &SnapshotSprite) -> Value {
    let kind = match kind {
        "selfPlayer" | "player" | "hero" => "player",
        other => other,
    };
    json!({
        "kind": kind,
        "classKey": class_key,
        "dead": dead,
        "sprite": {
            "bodyLibrary": sprite.body_library,
            "hairLibrary": sprite.hair_library,
            "weaponLibrary": sprite.weapon_library,
            "weaponLibrarySecondary": sprite.weapon_library_secondary,
            "altBodyLibrary": sprite.alt_body_library,
            "altHairLibrary": sprite.alt_hair_library,
            "altWeaponLibrary": sprite.alt_weapon_library,
            "altWeaponLibrarySecondary": sprite.alt_weapon_library_secondary,
            "mountLibrary": sprite.mount_library,
            "frameBaseOffset": sprite.frame_base_offset,
            "weaponFrameOffset": sprite.weapon_frame_offset,
            "altFrameBaseOffset": sprite.alt_frame_base_offset,
            "altWeaponFrameOffset": sprite.alt_weapon_frame_offset,
            "frameCount": sprite.frame_count,
            "directionStride": sprite.direction_stride,
            "mountFrameOffset": sprite.mount_frame_offset,
        },
    })
}

fn resolve_rect<'a>(
    rect_by_path: &'a HashMap<String, EntityAtlasRect>,
    library: &str,
    frame: i32,
) -> Option<&'a EntityAtlasRect> {
    // A strict render receipt must describe the requested authoritative pose,
    // not silently replace a missing direction/frame with library frame zero.
    rect_by_path.get(&frame_path(library, frame))
}

fn decode_page(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>, WorldAssetError> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| WorldAssetError::new("entity-atlas page dimensions overflow"))?;
    if pixels == 0 || pixels > MAX_PAGE_PIXELS {
        return Err(WorldAssetError::new(
            "entity-atlas page dimensions are out of bounds",
        ));
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| {
        WorldAssetError::new(format!("entity-atlas PNG header rejected: {error}"))
    })?;
    if reader.info().animation_control.is_some()
        || reader.info().width != width
        || reader.info().height != height
    {
        return Err(WorldAssetError::new(
            "entity-atlas PNG dimensions or animation do not match the manifest",
        ));
    }
    let output_size = reader
        .output_buffer_size()
        .ok_or_else(|| WorldAssetError::new("entity-atlas decoded size is unavailable"))?;
    let max_rgba_bytes = pixels
        .checked_mul(4)
        .ok_or_else(|| WorldAssetError::new("entity-atlas RGBA size overflows"))?;
    if output_size == 0 || output_size > max_rgba_bytes {
        return Err(WorldAssetError::new(
            "entity-atlas decoded size exceeds the page budget",
        ));
    }
    let mut output = vec![0; output_size];
    let frame = reader.next_frame(&mut output).map_err(|error| {
        WorldAssetError::new(format!("entity-atlas PNG data rejected: {error}"))
    })?;
    if frame.width != width || frame.height != height || frame.bit_depth != png::BitDepth::Eight {
        return Err(WorldAssetError::new(
            "entity-atlas PNG output format is invalid",
        ));
    }
    let source = &output[..frame.buffer_size()];
    let rgba = match frame.color_type {
        png::ColorType::Rgba => source.to_vec(),
        png::ColorType::Rgb => source
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => source
            .chunks_exact(2)
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        png::ColorType::Grayscale => source
            .iter()
            .flat_map(|value| [*value, *value, *value, 255])
            .collect(),
        png::ColorType::Indexed => {
            return Err(WorldAssetError::new(
                "entity-atlas indexed PNG was not expanded",
            ));
        }
    };
    if rgba.len() != max_rgba_bytes {
        return Err(WorldAssetError::new(
            "entity-atlas RGBA byte count is invalid",
        ));
    }
    Ok(rgba)
}

pub(crate) fn load_entity_render_state<F>(
    snapshot_json: &str,
    scene: &ProjectedScene,
    request_id: u64,
    mut read_asset: F,
) -> Result<EntityRenderProduct, WorldAssetError>
where
    F: FnMut(&str, usize) -> Result<Vec<u8>, WorldAssetError>,
{
    let snapshot: Snapshot = serde_json::from_str(snapshot_json)
        .map_err(|error| WorldAssetError::new(format!("entity snapshot rejected: {error}")))?;
    if snapshot.entities.is_empty()
        || snapshot
            .entities
            .len()
            .checked_add(snapshot.ground_drops.len())
            .is_none_or(|count| count > 8192)
    {
        return Err(WorldAssetError::new(
            "entity/drop snapshot count is out of bounds",
        ));
    }
    if !snapshot
        .entities
        .iter()
        .any(|entity| entity.object_id == snapshot.player_object_id && entity.kind == "selfPlayer")
    {
        return Err(WorldAssetError::new(
            "entity snapshot does not contain its authoritative self player",
        ));
    }
    let mut object_ids = HashSet::new();
    if snapshot.entities.iter().any(|entity| {
        entity
            .object_id
            .parse::<u32>()
            .ok()
            .filter(|id| *id != 0)
            .is_none()
            || entity.x < 0
            || entity.y < 0
            || !object_ids.insert(entity.object_id.clone())
    }) || snapshot.ground_drops.iter().any(|drop| {
        drop.object_id
            .parse::<u32>()
            .ok()
            .filter(|id| *id != 0)
            .is_none()
            || drop.x < 0
            || drop.y < 0
            || !object_ids.insert(drop.object_id.clone())
    }) {
        return Err(WorldAssetError::new(
            "entity/drop snapshot identity or position is invalid",
        ));
    }

    let ground_item_meta = read_asset(DNITEMS_META_ASSET, MAX_DNITEMS_META_BYTES)?;
    let ground_item_frames = parse_ground_item_frames(&ground_item_meta)?;

    let manifest_bytes = read_asset(ENTITY_ATLAS_MANIFEST_ASSET, MAX_MANIFEST_BYTES)?;
    if manifest_bytes.is_empty() || manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err(WorldAssetError::new(
            "entity-atlas manifest byte count is out of bounds",
        ));
    }
    let manifest: EntityAtlasManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|error| {
            WorldAssetError::new(format!("entity-atlas manifest rejected: {error}"))
        })?;
    if manifest.schema_version != ENTITY_ATLAS_SCHEMA_VERSION
        || manifest.kind != ENTITY_ATLAS_KIND
        || manifest.atlases.len() != 1
    {
        return Err(WorldAssetError::new(
            "unsupported entity-atlas manifest schema",
        ));
    }
    let atlas = manifest
        .atlases
        .into_iter()
        .next()
        .expect("validated atlas");
    if atlas.key.is_empty()
        || atlas.key.len() > 128
        || atlas.pages.is_empty()
        || atlas.pages.len() > MAX_PAGE_COUNT
        || atlas.rects.is_empty()
        || atlas.rects.len() > MAX_RECT_COUNT
        || atlas.width == 0
        || atlas.height == 0
    {
        return Err(WorldAssetError::new(
            "entity-atlas dimensions or entry count are out of bounds",
        ));
    }
    for page in &atlas.pages {
        if !safe_page_file(&page.image_file)
            || page.width != atlas.width
            || page.height != atlas.height
            || page.image_bytes == 0
            || page.image_bytes > MAX_PAGE_PNG_BYTES
        {
            return Err(WorldAssetError::new(
                "entity-atlas page descriptor is invalid",
            ));
        }
    }

    let mut rect_keys = HashSet::new();
    let mut rect_by_path = HashMap::with_capacity(atlas.rects.len());
    let mut available_libraries = HashSet::new();
    for rect in &atlas.rects {
        let source_path = atlas_source_path(&rect.key)
            .ok_or_else(|| WorldAssetError::new("entity-atlas rect key is invalid"))?;
        if !rect_keys.insert(rect.key.clone())
            || rect.page_index >= atlas.pages.len()
            || rect.width == 0
            || rect.height == 0
            || rect
                .x
                .checked_add(rect.width)
                .is_none_or(|right| right > atlas.width)
            || rect
                .y
                .checked_add(rect.height)
                .is_none_or(|bottom| bottom > atlas.height)
            || rect.frame_index < 0
            || rect_by_path
                .insert(source_path.clone(), rect.clone())
                .is_some()
        {
            return Err(WorldAssetError::new(
                "entity-atlas rect descriptor is invalid or duplicated",
            ));
        }
        if let Some((library, _)) = source_path.rsplit_once('/') {
            available_libraries.insert(library.trim_start_matches("/original-ui/").to_owned());
        }
    }

    let mut used_rects: BTreeMap<usize, BTreeMap<String, EntityAtlasRect>> = BTreeMap::new();
    let mut entries = Vec::new();
    let mut direction_entries = Vec::new();
    let mut unresolved_entity_count = 0usize;
    for entity in snapshot.entities {
        let dx = entity.x.saturating_sub(scene.center_x);
        let dy = entity.y.saturating_sub(scene.center_y);
        if dx.abs() > 24 || dy.abs() > 32 {
            continue;
        }
        let Some(sprite) = entity.sprite.clone().or_else(|| fallback_sprite(&entity)) else {
            unresolved_entity_count += 1;
            entries.push(EntityRenderEntry {
                object_id: entity.object_id,
                is_self: false,
                grid_x: entity.x,
                grid_y: entity.y,
                layers: Vec::new(),
            });
            continue;
        };
        if !(1..=64).contains(&sprite.frame_count) || !(1..=64).contains(&sprite.direction_stride) {
            return Err(WorldAssetError::new(
                "entity sprite frame metadata is out of bounds",
            ));
        }
        let Some(body_library) = normalize_library(&sprite.body_library) else {
            return Err(WorldAssetError::new(
                "entity sprite body library is invalid",
            ));
        };
        let hair_library = sprite.hair_library.as_deref().and_then(normalize_library);
        let weapon_library = sprite.weapon_library.as_deref().and_then(normalize_library);
        let weapon_library_secondary = sprite
            .weapon_library_secondary
            .as_deref()
            .and_then(normalize_library);
        let frame_base_offset = sprite.frame_base_offset;
        let weapon_frame_offset = sprite.weapon_frame_offset;
        let class_key = entity
            .class_key
            .as_deref()
            .or(entity.class_name.as_deref())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let stride = sprite.direction_stride;
        let mounted = sprite.mount_library.is_some();
        let root_left = ENTITY_LEFT_ORIGIN + dx as f32 * CELL_WIDTH;
        let root_top = ENTITY_TOP_ORIGIN + dy as f32 * CELL_HEIGHT;
        let depth = 4096 + dy * 128 + dx * 2 + 64;

        let requested_for_frame =
            |direction_name: &str, relative_frame: i32, action: AnimationAction| {
                let is_player = matches!(entity.kind.as_str(), "selfPlayer" | "player" | "hero");
                let use_alt = is_player
                    && !mounted
                    && ((class_key == "assassin" && uses_assassin_alt(action))
                        || (class_key == "archer" && uses_archer_alt(action)));
                let alt_body = sprite
                    .alt_body_library
                    .as_deref()
                    .and_then(normalize_library)
                    .filter(|library| available_libraries.contains(library));
                let use_alt = use_alt && alt_body.is_some();
                let resolved_body = if use_alt {
                    alt_body.unwrap_or_else(|| body_library.clone())
                } else {
                    body_library.clone()
                };
                let resolved_hair = if use_alt {
                    sprite
                        .alt_hair_library
                        .as_deref()
                        .and_then(normalize_library)
                        .filter(|library| available_libraries.contains(library))
                        .or_else(|| hair_library.clone())
                } else {
                    hair_library.clone()
                };
                let resolved_weapon = if use_alt {
                    sprite
                        .alt_weapon_library
                        .as_deref()
                        .and_then(normalize_library)
                        .filter(|library| available_libraries.contains(library))
                        .or_else(|| weapon_library.clone())
                } else {
                    weapon_library.clone()
                };
                let resolved_weapon_secondary = if use_alt {
                    sprite
                        .alt_weapon_library_secondary
                        .as_deref()
                        .and_then(normalize_library)
                        .filter(|library| available_libraries.contains(library))
                        .or_else(|| weapon_library_secondary.clone())
                } else {
                    weapon_library_secondary.clone()
                };
                let resolved_body_offset = if use_alt {
                    sprite.alt_frame_base_offset.unwrap_or(frame_base_offset)
                } else {
                    frame_base_offset
                };
                let resolved_weapon_offset = if use_alt {
                    sprite.alt_weapon_frame_offset.or(weapon_frame_offset)
                } else {
                    weapon_frame_offset
                };
                let body_frame = resolved_body_offset.saturating_add(relative_frame);
                let weapon_frame =
                    resolved_weapon_offset.map(|offset| offset.saturating_add(relative_frame));
                let mut requested = Vec::<(&str, String, i32)>::new();
                if let Some(mount) = sprite.mount_library.as_deref().and_then(normalize_library) {
                    if let Some(mount_relative_frame) = relative_frame.checked_sub(416) {
                        requested.push((
                            "mount",
                            mount,
                            sprite
                                .mount_frame_offset
                                .unwrap_or_default()
                                .saturating_add(mount_relative_frame),
                        ));
                    }
                }
                let mut weapons = Vec::new();
                if let (Some(library), Some(frame)) = (resolved_weapon, weapon_frame) {
                    weapons.push(("weapon", library, frame));
                }
                if let (Some(library), Some(frame)) = (resolved_weapon_secondary, weapon_frame) {
                    weapons.push(("weaponSecondary", library, frame));
                }
                let dual_weapon = weapons.len() == 2;
                if !mounted {
                    requested.extend(
                        weapons
                            .iter()
                            .filter(|(role, _, _)| {
                                if dual_weapon {
                                    if *role == "weapon" {
                                        !assassin_primary_is_front(direction_name)
                                    } else {
                                        assassin_primary_is_front(direction_name)
                                    }
                                } else {
                                    weapon_is_rear(Some(direction_name))
                                }
                            })
                            .cloned(),
                    );
                }
                requested.push(("body", resolved_body, body_frame));
                if let Some(hair) = resolved_hair {
                    requested.push(("hair", hair, body_frame));
                }
                if !mounted {
                    requested.extend(weapons.into_iter().filter(|(role, _, _)| {
                        if dual_weapon {
                            if *role == "weapon" {
                                assassin_primary_is_front(direction_name)
                            } else {
                                !assassin_primary_is_front(direction_name)
                            }
                        } else {
                            !weapon_is_rear(Some(direction_name))
                        }
                    }));
                }
                requested
            };
        let mut resolve_direction_layers = |requested: Vec<(&str, String, i32)>| {
            let mut layers = Vec::new();
            let mut missing_body = false;
            for (order, (role, library, frame)) in requested.into_iter().enumerate() {
                let Some(rect) = resolve_rect(&rect_by_path, &library, frame) else {
                    if matches!(role, "body" | "mount") {
                        missing_body = true;
                    }
                    continue;
                };
                let path = atlas_source_path(&rect.key).expect("validated rect path");
                let atlas_page_key = page_key(&atlas.key, rect.page_index);
                used_rects
                    .entry(rect.page_index)
                    .or_default()
                    .insert(rect.key.clone(), rect.clone());
                layers.push(EntityRenderLayer {
                    key: format!("{}:{role}:0", entity.object_id),
                    path,
                    atlas_key: Some(atlas_page_key),
                    atlas_rect_key: Some(rect.key.clone()),
                    left: root_left + rect.offset_x as f32,
                    top: root_top + rect.offset_y as f32,
                    width: rect.width as f32,
                    height: rect.height as f32,
                    z: depth as f32 * 10.0 + order as f32,
                    opacity: if entity.dead { 0.45 } else { 1.0 },
                });
            }
            (layers, missing_body)
        };
        let selected_direction = entity
            .direction
            .as_deref()
            .filter(|direction| DIRECTIONS.contains(direction))
            .unwrap_or("Down");
        let mut direction_layers = BTreeMap::new();
        let standing_frame = |direction_name: &str| {
            direction_index(Some(direction_name))
                .saturating_mul(stride)
                .saturating_add(if mounted { 416 } else { 0 })
        };
        let (selected_layers, selected_missing_body) =
            resolve_direction_layers(requested_for_frame(
                selected_direction,
                standing_frame(selected_direction),
                AnimationAction::Standing,
            ));
        let selected_ready = !selected_missing_body && !selected_layers.is_empty();
        if selected_ready {
            direction_layers.insert(selected_direction.to_owned(), selected_layers.clone());
            for direction in DIRECTIONS
                .into_iter()
                .filter(|direction| *direction != selected_direction)
            {
                let (layers, missing_body) = resolve_direction_layers(requested_for_frame(
                    direction,
                    standing_frame(direction),
                    AnimationAction::Standing,
                ));
                if !missing_body && !layers.is_empty() {
                    direction_layers.insert(direction.to_owned(), layers);
                }
            }
        }
        let mut action_layers = BTreeMap::new();
        // The packet path owns the action clock, but it must never resolve
        // untrusted asset names. Precompute only exact rects present in the
        // immutable packaged atlas. Player catalogs match the shared Windows
        // mounted and Archer tables; Assassin actions use the ordinary Crystal
        // player timings with their authoritative alternate layer set.
        if selected_ready {
            if let Some(kind) = animation_kind(&entity.kind) {
                let alt_body_available = sprite
                    .alt_body_library
                    .as_deref()
                    .and_then(normalize_library)
                    .is_some_and(|library| available_libraries.contains(&library));
                let catalog = if kind == EntityKind::Player && mounted {
                    mounted_player_catalog()
                } else if kind == EntityKind::Player && class_key == "archer" && alt_body_available
                {
                    archer_player_catalog()
                } else {
                    AnimationCatalog::crystal_default(kind)
                };
                for action in [
                    AnimationAction::Walking,
                    AnimationAction::Running,
                    AnimationAction::Harvest,
                    AnimationAction::Attack1,
                    AnimationAction::Attack2,
                    AnimationAction::Attack3,
                    AnimationAction::Attack4,
                    AnimationAction::AttackRange1,
                    AnimationAction::AttackRange2,
                    AnimationAction::DashAttack,
                    AnimationAction::Spell,
                    AnimationAction::Struck,
                    AnimationAction::Die,
                    AnimationAction::Dead,
                    AnimationAction::Skeleton,
                    AnimationAction::Revive,
                ] {
                    let Some(descriptor) = catalog.descriptor(action).copied() else {
                        continue;
                    };
                    for direction in DIRECTIONS {
                        let mut frames = Vec::with_capacity(usize::from(descriptor.frame_count));
                        let mut complete = true;
                        for phase in 0..descriptor.frame_count {
                            let relative_frame =
                                descriptor.draw_frame(animation_direction(direction), phase);
                            let (layers, missing_body) = resolve_direction_layers(
                                requested_for_frame(direction, relative_frame, action),
                            );
                            if missing_body || layers.is_empty() {
                                complete = false;
                                break;
                            }
                            frames.push(layers);
                        }
                        if complete {
                            action_layers.insert(
                                format!("{}:{direction}", animation_action_name(action)),
                                EntityActionLayers {
                                    interval_ms: descriptor.frame_interval_ms,
                                    frames,
                                },
                            );
                        }
                    }
                }
            }
        }
        if !selected_ready {
            unresolved_entity_count += 1;
        }
        entries.push(EntityRenderEntry {
            is_self: entity.object_id == snapshot.player_object_id,
            object_id: entity.object_id.clone(),
            grid_x: entity.x,
            grid_y: entity.y,
            layers: selected_layers,
        });
        if !direction_layers.is_empty() {
            direction_entries.push(EntityDirectionEntry {
                object_id: entity.object_id,
                prototype: prototype_descriptor(&entity.kind, &class_key, entity.dead, &sprite),
                direction_layers,
                action_layers,
            });
        }
    }
    for drop in snapshot.ground_drops {
        let dx = drop.x.saturating_sub(scene.center_x);
        let dy = drop.y.saturating_sub(scene.center_y);
        if dx.abs() > 24 || dy.abs() > 32 {
            continue;
        }
        let layers = ground_item_frames
            .get(&drop.image)
            .map(|frame| {
                let root_left = ENTITY_LEFT_ORIGIN + dx as f32 * CELL_WIDTH;
                let root_top = ENTITY_TOP_ORIGIN + dy as f32 * CELL_HEIGHT;
                let depth = 4096 + dy * 128 + dx * 2 + 64;
                vec![EntityRenderLayer {
                    key: format!("{}:ground-item", drop.object_id),
                    path: format!("/original-ui/DNItems/{}.png", drop.image),
                    atlas_key: None,
                    atlas_rect_key: None,
                    left: root_left + (CELL_WIDTH - frame.width as f32) / 2.0,
                    top: root_top + (CELL_HEIGHT - frame.height as f32) / 2.0,
                    width: frame.width as f32,
                    height: frame.height as f32,
                    z: depth as f32 * 10.0,
                    opacity: 1.0,
                }]
            })
            .unwrap_or_default();
        if layers.is_empty() {
            unresolved_entity_count += 1;
        }
        entries.push(EntityRenderEntry {
            object_id: drop.object_id,
            is_self: false,
            grid_x: drop.x,
            grid_y: drop.y,
            layers,
        });
    }

    let used_pages: BTreeSet<usize> = used_rects.keys().copied().collect();
    let mut render_atlases = Vec::with_capacity(used_pages.len());
    for page_index in &used_pages {
        let page = &atlas.pages[*page_index];
        let rects = used_rects[page_index]
            .values()
            .map(|rect| EntityRenderAtlasRect {
                key: rect.key.clone(),
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            })
            .collect();
        render_atlases.push(EntityRenderAtlas {
            key: page_key(&atlas.key, *page_index),
            width: page.width,
            height: page.height,
            rects,
        });
    }

    let mut pages = Vec::with_capacity(used_pages.len());
    let mut compressed_bytes = ground_item_meta.len();
    let mut rgba_bytes = 0usize;
    for page_index in used_pages {
        let page = &atlas.pages[page_index];
        let asset_path = format!("bevy-entity-atlases/{}", page.image_file);
        let bytes = read_asset(&asset_path, page.image_bytes)?;
        if bytes.len() != page.image_bytes {
            return Err(WorldAssetError::new(
                "entity-atlas PNG byte count does not match the manifest",
            ));
        }
        let rgba = decode_page(&bytes, page.width, page.height)?;
        compressed_bytes = compressed_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| WorldAssetError::new("entity-atlas PNG total overflows"))?;
        rgba_bytes = rgba_bytes
            .checked_add(rgba.len())
            .ok_or_else(|| WorldAssetError::new("entity-atlas RGBA total overflows"))?;
        if rgba_bytes > MAX_SELECTED_RGBA_BYTES {
            return Err(WorldAssetError::new(
                "selected entity-atlas pages exceed the memory budget",
            ));
        }
        pages.push(DecodedEntityAtlasPage {
            key: page_key(&atlas.key, page_index),
            width: page.width,
            height: page.height,
            rgba,
        });
    }

    let layer_count = entries.iter().map(|entry| entry.layers.len()).sum();
    let state = EntityRenderState {
        native_world_request: request_id,
        enabled: true,
        stage_width: STAGE_WIDTH,
        stage_height: STAGE_HEIGHT,
        center_x: scene.center_x,
        center_y: scene.center_y,
        unresolved_entity_count,
        atlases: render_atlases,
        entities: entries,
    };
    let live_directions_json = serde_json::to_string(&EntityDirectionState {
        native_world_request: request_id,
        entities: direction_entries,
        ground_item_frames,
    })
    .map_err(|error| {
        WorldAssetError::new(format!(
            "entity direction sidecar could not be encoded: {error}"
        ))
    })?;
    let json = serde_json::to_string(&state).map_err(|error| {
        WorldAssetError::new(format!("entity render state could not be encoded: {error}"))
    })?;
    if json
        .len()
        .checked_add(live_directions_json.len())
        .is_none_or(|bytes| bytes > MAX_RENDER_STATE_BYTES)
    {
        return Err(WorldAssetError::new(
            "entity render state exceeds the native message budget",
        ));
    }
    Ok(EntityRenderProduct {
        json,
        live_directions_json,
        pages,
        entity_count: state.entities.len(),
        layer_count,
        unresolved_entity_count,
        compressed_bytes,
        rgba_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba_png() -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[255, 0, 0, 255])
            .unwrap();
        bytes
    }

    fn dnitems_meta() -> Vec<u8> {
        serde_json::json!({
            "version": 3,
            "count": 2,
            "frames": [
                {"index":0,"width":20,"height":13,"path":"/original-ui/DNItems/0.png"},
                {"index":112,"width":16,"height":10,"path":"/original-ui/DNItems/112.png"}
            ]
        })
        .to_string()
        .into_bytes()
    }

    fn scene() -> ProjectedScene {
        ProjectedScene {
            map_file_name: "0".into(),
            center_x: 300,
            center_y: 630,
            width: 19,
            height: 15,
        }
    }

    #[test]
    fn authoritative_objects_resolve_into_shared_entity_layers() {
        let png = rgba_png();
        let manifest = serde_json::json!({
            "schemaVersion": 2,
            "kind": ENTITY_ATLAS_KIND,
            "atlases": [{
                "key": "starter",
                "width": 1,
                "height": 1,
                "pages": [{"imageFile":"starter.png","width":1,"height":1,"imageBytes":png.len()}],
                "rects": [
                    {
                        "key":"/original-ui/CArmour/00/16.png|1x1",
                        "x":0,"y":0,"width":1,"height":1,"offsetX":8,"offsetY":-48,
                        "frameIndex":16,"pageIndex":0
                    },
                    {
                        "key":"/original-ui/CArmour/00/8.png|1x1",
                        "x":0,"y":0,"width":1,"height":1,"offsetX":7,"offsetY":-47,
                        "frameIndex":8,"pageIndex":0
                    }
                ]
            }]
        })
        .to_string()
        .into_bytes();
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[{
                "objectId":"42","kind":"selfPlayer","x":300,"y":630,"direction":"Down",
                "sprite":{"bodyLibrary":"CArmour/00","frameBaseOffset":0,"directionStride":4}
            }]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 17, |path, _| match path {
            DNITEMS_META_ASSET => Ok(dnitems_meta()),
            ENTITY_ATLAS_MANIFEST_ASSET => Ok(manifest.clone()),
            "bevy-entity-atlases/starter.png" => Ok(png.clone()),
            _ => Err(WorldAssetError::new("missing fixture")),
        })
        .unwrap();
        assert_eq!(product.entity_count, 1);
        assert_eq!(product.layer_count, 1);
        assert_eq!(product.unresolved_entity_count, 0);
        assert_eq!(product.pages.len(), 1);
        let state: serde_json::Value = serde_json::from_str(&product.json).unwrap();
        assert_eq!(state["_nativeWorldRequest"], 17);
        assert_eq!(state["entities"][0]["isSelf"], true);
        assert_eq!(state["entities"][0]["layers"][0]["left"], 488.0);
        assert_eq!(state["entities"][0]["layers"][0]["top"], 304.0);
        assert_eq!(state["entities"][0]["layers"][0]["atlasKey"], "starter");
        assert!(state["entities"][0].get("directionLayers").is_none());
        let live: serde_json::Value = serde_json::from_str(&product.live_directions_json).unwrap();
        assert_eq!(
            live["entities"][0]["directionLayers"]["Down"][0]["atlasRectKey"],
            "/original-ui/CArmour/00/16.png|1x1"
        );
        assert_eq!(
            live["entities"][0]["directionLayers"]["Right"][0]["atlasRectKey"],
            "/original-ui/CArmour/00/8.png|1x1"
        );
        assert_eq!(
            live["entities"][0]["prototype"]["sprite"]["bodyLibrary"],
            "CArmour/00"
        );
        assert_eq!(live["entities"][0]["prototype"]["kind"], "player");
        assert_eq!(live["entities"][0]["prototype"]["classKey"], "");
    }

    #[test]
    fn authoritative_ground_drops_use_exact_packaged_dnitems_frames() {
        let png = rgba_png();
        let manifest = serde_json::json!({
            "schemaVersion":2,"kind":ENTITY_ATLAS_KIND,
            "atlases":[{"key":"starter","width":1,"height":1,
                "pages":[{"imageFile":"starter.png","width":1,"height":1,"imageBytes":png.len()}],
                "rects":[{"key":"/original-ui/CArmour/00/16.png|1x1",
                    "x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,
                    "frameIndex":16,"pageIndex":0}]}]
        })
        .to_string()
        .into_bytes();
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[{"objectId":"42","kind":"selfPlayer","x":300,"y":630,
                "direction":"Down","sprite":{"bodyLibrary":"CArmour/00",
                "frameBaseOffset":0,"directionStride":4}}],
            "groundDrops":[
                {"objectId":"50","name":"Potion","nameColourArgb":-1,
                    "x":301,"y":630,"quantity":1,"image":0,"dropKind":"item"},
                {"objectId":"51","name":"Gold","nameColourArgb":-1,
                    "x":300,"y":631,"quantity":25,"image":112,"dropKind":"gold"}
            ]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 28, |path, _| match path {
            DNITEMS_META_ASSET => Ok(dnitems_meta()),
            ENTITY_ATLAS_MANIFEST_ASSET => Ok(manifest.clone()),
            "bevy-entity-atlases/starter.png" => Ok(png.clone()),
            _ => Err(WorldAssetError::new("missing fixture")),
        })
        .unwrap();
        assert_eq!(product.entity_count, 3);
        assert_eq!(product.layer_count, 3);
        assert_eq!(product.unresolved_entity_count, 0);
        let state: Value = serde_json::from_str(&product.json).unwrap();
        let item = &state["entities"][1];
        assert_eq!(item["layers"][0]["path"], "/original-ui/DNItems/0.png");
        assert!(item["layers"][0].get("atlasKey").is_none());
        assert_eq!(item["layers"][0]["left"], 542.0);
        assert_eq!(item["layers"][0]["top"], 361.5);
        let gold = &state["entities"][2];
        assert_eq!(gold["layers"][0]["path"], "/original-ui/DNItems/112.png");
        assert_eq!(gold["layers"][0]["left"], 496.0);
        assert_eq!(gold["layers"][0]["top"], 395.0);
        let live: Value = serde_json::from_str(&product.live_directions_json).unwrap();
        assert_eq!(live["groundItemFrames"]["0"]["width"], 20);
        assert_eq!(live["groundItemFrames"]["112"]["height"], 10);
    }

    #[test]
    fn packaged_atlas_precomputes_bounded_packet_action_frames() {
        let png = rgba_png();
        let mut rects = vec![serde_json::json!({
            "key":"/original-ui/CArmour/00/16.png|1x1",
            "x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,
            "frameIndex":16,"pageIndex":0
        })];
        rects.extend((160..=165).map(|frame| {
            serde_json::json!({
                "key":format!("/original-ui/CArmour/00/{frame}.png|1x1"),
                "x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,
                "frameIndex":frame,"pageIndex":0
            })
        }));
        for range in [56..=61, 104..=109] {
            rects.extend(range.map(|frame| {
                serde_json::json!({
                    "key":format!("/original-ui/CArmour/00/{frame}.png|1x1"),
                    "x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,
                    "frameIndex":frame,"pageIndex":0
                })
            }));
        }
        let manifest = serde_json::json!({
            "schemaVersion":2,"kind":ENTITY_ATLAS_KIND,
            "atlases":[{"key":"starter","width":1,"height":1,
                "pages":[{"imageFile":"starter.png","width":1,"height":1,"imageBytes":png.len()}],
                "rects":rects}]
        })
        .to_string()
        .into_bytes();
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[{"objectId":"42","kind":"selfPlayer","x":300,"y":630,
                "direction":"Down","class":"Warrior",
                "sprite":{"bodyLibrary":"CArmour/00","frameBaseOffset":0,"directionStride":4}}]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 27, |path, _| match path {
            DNITEMS_META_ASSET => Ok(dnitems_meta()),
            ENTITY_ATLAS_MANIFEST_ASSET => Ok(manifest.clone()),
            "bevy-entity-atlases/starter.png" => Ok(png.clone()),
            _ => Err(WorldAssetError::new("missing fixture")),
        })
        .unwrap();
        let live: Value = serde_json::from_str(&product.live_directions_json).unwrap();
        assert_eq!(
            live["entities"][0]["actionLayers"]["attack1:Down"]["intervalMs"],
            100
        );
        let frames = live["entities"][0]["actionLayers"]["attack1:Down"]["frames"]
            .as_array()
            .unwrap();
        assert_eq!(frames.len(), 6);
        assert_eq!(
            frames[0][0]["atlasRectKey"],
            "/original-ui/CArmour/00/160.png|1x1"
        );
        assert_eq!(
            frames[5][0]["atlasRectKey"],
            "/original-ui/CArmour/00/165.png|1x1"
        );
        for (action, first, last) in [("walking", 56, 61), ("running", 104, 109)] {
            let frames = live["entities"][0]["actionLayers"][format!("{action}:Down")]["frames"]
                .as_array()
                .unwrap();
            assert_eq!(frames.len(), 6);
            assert_eq!(
                frames[0][0]["atlasRectKey"],
                format!("/original-ui/CArmour/00/{first}.png|1x1")
            );
            assert_eq!(
                frames[5][0]["atlasRectKey"],
                format!("/original-ui/CArmour/00/{last}.png|1x1")
            );
        }
    }

    #[test]
    fn unknown_monster_stays_an_object_but_is_reported_unresolved() {
        let png = rgba_png();
        let manifest = serde_json::json!({
            "schemaVersion": 2,
            "kind": ENTITY_ATLAS_KIND,
            "atlases": [{
                "key": "starter", "width":1, "height":1,
                "pages":[{"imageFile":"starter.png","width":1,"height":1,"imageBytes":png.len()}],
                "rects":[{"key":"/original-ui/CArmour/00/16.png|1x1","x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,"frameIndex":16}]
            }]
        }).to_string().into_bytes();
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[
                {"objectId":"42","kind":"selfPlayer","x":300,"y":630,"direction":"Down"},
                {"objectId":"43","kind":"monster","x":301,"y":630,"direction":"Down"}
            ]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 18, |path, _| match path {
            DNITEMS_META_ASSET => Ok(dnitems_meta()),
            ENTITY_ATLAS_MANIFEST_ASSET => Ok(manifest.clone()),
            "bevy-entity-atlases/starter.png" => Ok(png.clone()),
            _ => Err(WorldAssetError::new("missing fixture")),
        })
        .unwrap();
        assert_eq!(product.entity_count, 2);
        assert_eq!(product.layer_count, 1);
        assert_eq!(product.unresolved_entity_count, 1);
    }

    #[test]
    fn missing_authoritative_pose_does_not_fall_back_to_frame_zero() {
        let png = rgba_png();
        let manifest = serde_json::json!({
            "schemaVersion": 2,
            "kind": ENTITY_ATLAS_KIND,
            "atlases": [{
                "key": "starter", "width":1, "height":1,
                "pages":[{"imageFile":"starter.png","width":1,"height":1,"imageBytes":png.len()}],
                "rects":[{"key":"/original-ui/CArmour/00/0.png|1x1","x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,"frameIndex":0}]
            }]
        }).to_string().into_bytes();
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[{
                "objectId":"42","kind":"selfPlayer","x":300,"y":630,"direction":"Down",
                "sprite":{"bodyLibrary":"CArmour/00","frameBaseOffset":0,"directionStride":4}
            }]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 21, |path, _| match path {
            DNITEMS_META_ASSET => Ok(dnitems_meta()),
            ENTITY_ATLAS_MANIFEST_ASSET => Ok(manifest.clone()),
            "bevy-entity-atlases/starter.png" => Ok(png.clone()),
            _ => Err(WorldAssetError::new("missing fixture")),
        })
        .unwrap();
        assert_eq!(product.entity_count, 1);
        assert_eq!(product.layer_count, 0);
        assert_eq!(product.unresolved_entity_count, 1);
        assert!(product.pages.is_empty());
    }

    #[test]
    fn assassin_standing_uses_authoritative_alt_body_hair_and_dual_weapons() {
        let png = rgba_png();
        let libraries = [
            "CArmour/00",
            "AArmour/00",
            "AHair/00",
            "AWeapon/00 R",
            "AWeapon/00 L",
        ];
        let rects: Vec<_> = libraries
            .iter()
            .enumerate()
            .map(|(index, library)| {
                serde_json::json!({
                    "key":format!("/original-ui/{library}/16.png|1x1"),
                    "x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,
                    "frameIndex":16,"pageIndex":0,"_fixtureIndex":index
                })
            })
            .collect();
        let manifest = serde_json::json!({
            "schemaVersion": 2,
            "kind": ENTITY_ATLAS_KIND,
            "atlases": [{
                "key":"starter","width":1,"height":1,
                "pages":[{"imageFile":"starter.png","width":1,"height":1,"imageBytes":png.len()}],
                "rects":rects
            }]
        })
        .to_string()
        .into_bytes();
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[{
                "objectId":"42","kind":"selfPlayer","classKey":"assassin",
                "x":300,"y":630,"direction":"Down",
                "sprite":{
                    "bodyLibrary":"CArmour/00","frameBaseOffset":0,
                    "altBodyLibrary":"AArmour/00","altHairLibrary":"AHair/00",
                    "altWeaponLibrary":"AWeapon/00 R",
                    "altWeaponLibrarySecondary":"AWeapon/00 L",
                    "altFrameBaseOffset":0,"altWeaponFrameOffset":0,
                    "frameCount":4,"directionStride":4
                }
            }]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 19, |path, _| match path {
            DNITEMS_META_ASSET => Ok(dnitems_meta()),
            ENTITY_ATLAS_MANIFEST_ASSET => Ok(manifest.clone()),
            "bevy-entity-atlases/starter.png" => Ok(png.clone()),
            _ => Err(WorldAssetError::new("missing fixture")),
        })
        .unwrap();
        assert_eq!(product.layer_count, 4);
        assert_eq!(product.unresolved_entity_count, 0);
        let state: serde_json::Value = serde_json::from_str(&product.json).unwrap();
        let paths: Vec<_> = state["entities"][0]["layers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|layer| layer["path"].as_str().unwrap())
            .collect();
        assert_eq!(
            paths,
            [
                "/original-ui/AWeapon/00 L/16.png",
                "/original-ui/AArmour/00/16.png",
                "/original-ui/AHair/00/16.png",
                "/original-ui/AWeapon/00 R/16.png",
            ]
        );
    }

    #[test]
    fn assassin_actions_use_alt_catalog_layers_and_decode_space_paths() {
        let png = rgba_png();
        let mut rects = Vec::new();
        for frame in std::iter::once(16).chain(160..=165) {
            for library in ["AArmour/00", "AHair/00", "AWeapon/00%20R", "AWeapon/00%20L"] {
                rects.push(serde_json::json!({
                    "key":format!("/original-ui/{library}/{frame}.png|1x1"),
                    "x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,
                    "frameIndex":frame,"pageIndex":0
                }));
            }
        }
        let manifest = serde_json::json!({
            "schemaVersion":2,"kind":ENTITY_ATLAS_KIND,
            "atlases":[{"key":"starter","width":1,"height":1,
                "pages":[{"imageFile":"starter.png","width":1,"height":1,"imageBytes":png.len()}],
                "rects":rects}]
        })
        .to_string()
        .into_bytes();
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[{
                "objectId":"42","kind":"selfPlayer","classKey":"assassin",
                "x":300,"y":630,"direction":"Down",
                "sprite":{
                    "bodyLibrary":"CArmour/00","frameBaseOffset":0,
                    "altBodyLibrary":"AArmour/00","altHairLibrary":"AHair/00",
                    "altWeaponLibrary":"AWeapon/00 R",
                    "altWeaponLibrarySecondary":"AWeapon/00 L",
                    "altFrameBaseOffset":0,"altWeaponFrameOffset":0,
                    "frameCount":4,"directionStride":4
                }
            }]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 31, |path, _| match path {
            DNITEMS_META_ASSET => Ok(dnitems_meta()),
            ENTITY_ATLAS_MANIFEST_ASSET => Ok(manifest.clone()),
            "bevy-entity-atlases/starter.png" => Ok(png.clone()),
            _ => Err(WorldAssetError::new("missing fixture")),
        })
        .unwrap();
        let live: Value = serde_json::from_str(&product.live_directions_json).unwrap();
        let frames = live["entities"][0]["actionLayers"]["attack1:Down"]["frames"]
            .as_array()
            .unwrap();
        assert_eq!(frames.len(), 6);
        assert_eq!(frames[0].as_array().map(Vec::len), Some(4));
        assert_eq!(frames[0][0]["path"], "/original-ui/AWeapon/00 L/160.png");
        assert_eq!(frames[5][3]["path"], "/original-ui/AWeapon/00 R/165.png");
    }

    #[test]
    fn archer_walk_uses_compact_alt_catalog_when_library_is_packaged() {
        let png = rgba_png();
        let mut rects = vec![serde_json::json!({
            "key":"/original-ui/CArmour/00/16.png|1x1",
            "x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,
            "frameIndex":16,"pageIndex":0
        })];
        for frame in 24..=29 {
            for library in ["ARArmour/00", "ARWeapon/00%20S"] {
                rects.push(serde_json::json!({
                    "key":format!("/original-ui/{library}/{frame}.png|1x1"),
                    "x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,
                    "frameIndex":frame,"pageIndex":0
                }));
            }
        }
        let manifest = serde_json::json!({
            "schemaVersion":2,"kind":ENTITY_ATLAS_KIND,
            "atlases":[{"key":"starter","width":1,"height":1,
                "pages":[{"imageFile":"starter.png","width":1,"height":1,"imageBytes":png.len()}],
                "rects":rects}]
        })
        .to_string()
        .into_bytes();
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[{
                "objectId":"42","kind":"selfPlayer","classKey":"archer",
                "x":300,"y":630,"direction":"Down",
                "sprite":{
                    "bodyLibrary":"CArmour/00","frameBaseOffset":0,
                    "altBodyLibrary":"ARArmour/00",
                    "altWeaponLibrary":"ARWeapon/00 S",
                    "altFrameBaseOffset":0,"altWeaponFrameOffset":0,
                    "frameCount":4,"directionStride":4
                }
            }]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 32, |path, _| match path {
            DNITEMS_META_ASSET => Ok(dnitems_meta()),
            ENTITY_ATLAS_MANIFEST_ASSET => Ok(manifest.clone()),
            "bevy-entity-atlases/starter.png" => Ok(png.clone()),
            _ => Err(WorldAssetError::new("missing fixture")),
        })
        .unwrap();
        let live: Value = serde_json::from_str(&product.live_directions_json).unwrap();
        let frames = live["entities"][0]["actionLayers"]["walking:Down"]["frames"]
            .as_array()
            .unwrap();
        assert_eq!(frames.len(), 6);
        assert_eq!(frames[0][0]["path"], "/original-ui/ARArmour/00/24.png");
        assert_eq!(frames[5][1]["path"], "/original-ui/ARWeapon/00 S/29.png");
    }

    #[test]
    fn mounted_standing_uses_crystal_416_body_band_and_mount_offset() {
        let png = rgba_png();
        let manifest = serde_json::json!({
            "schemaVersion": 2,
            "kind": ENTITY_ATLAS_KIND,
            "atlases": [{
                "key":"starter","width":1,"height":1,
                "pages":[{"imageFile":"starter.png","width":1,"height":1,"imageBytes":png.len()}],
                "rects":[
                    {"key":"/original-ui/CArmour/00/432.png|1x1","x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,"frameIndex":432},
                    {"key":"/original-ui/Mount/00/16.png|1x1","x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,"frameIndex":16},
                    {"key":"/original-ui/CWeapon/00/432.png|1x1","x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,"frameIndex":432}
                ]
            }]
        })
        .to_string()
        .into_bytes();
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[{
                "objectId":"42","kind":"selfPlayer","x":300,"y":630,"direction":"Down",
                "sprite":{
                    "bodyLibrary":"CArmour/00","weaponLibrary":"CWeapon/00",
                    "mountLibrary":"Mount/00","frameBaseOffset":0,"weaponFrameOffset":0,
                    "mountFrameOffset":0,"frameCount":4,"directionStride":4
                }
            }]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 20, |path, _| match path {
            DNITEMS_META_ASSET => Ok(dnitems_meta()),
            ENTITY_ATLAS_MANIFEST_ASSET => Ok(manifest.clone()),
            "bevy-entity-atlases/starter.png" => Ok(png.clone()),
            _ => Err(WorldAssetError::new("missing fixture")),
        })
        .unwrap();
        assert_eq!(product.layer_count, 2);
        let state: serde_json::Value = serde_json::from_str(&product.json).unwrap();
        let paths: Vec<_> = state["entities"][0]["layers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|layer| layer["path"].as_str().unwrap())
            .collect();
        assert_eq!(
            paths,
            [
                "/original-ui/Mount/00/16.png",
                "/original-ui/CArmour/00/432.png"
            ]
        );
    }

    #[test]
    fn mounted_walk_uses_exact_body_and_mount_action_bands() {
        let png = rgba_png();
        let mut rects = vec![
            serde_json::json!({"key":"/original-ui/CArmour/00/432.png|1x1","x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,"frameIndex":432}),
            serde_json::json!({"key":"/original-ui/Mount/00/16.png|1x1","x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,"frameIndex":16}),
        ];
        for (body, mount) in (480..=487).zip(64..=71) {
            rects.push(serde_json::json!({"key":format!("/original-ui/CArmour/00/{body}.png|1x1"),"x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,"frameIndex":body}));
            rects.push(serde_json::json!({"key":format!("/original-ui/Mount/00/{mount}.png|1x1"),"x":0,"y":0,"width":1,"height":1,"offsetX":0,"offsetY":0,"frameIndex":mount}));
        }
        let manifest = serde_json::json!({
            "schemaVersion":2,"kind":ENTITY_ATLAS_KIND,
            "atlases":[{"key":"starter","width":1,"height":1,
                "pages":[{"imageFile":"starter.png","width":1,"height":1,"imageBytes":png.len()}],
                "rects":rects}]
        })
        .to_string()
        .into_bytes();
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[{
                "objectId":"42","kind":"selfPlayer","x":300,"y":630,"direction":"Down",
                "sprite":{"bodyLibrary":"CArmour/00","mountLibrary":"Mount/00",
                    "frameBaseOffset":0,"mountFrameOffset":0,"frameCount":4,"directionStride":4}
            }]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 33, |path, _| match path {
            DNITEMS_META_ASSET => Ok(dnitems_meta()),
            ENTITY_ATLAS_MANIFEST_ASSET => Ok(manifest.clone()),
            "bevy-entity-atlases/starter.png" => Ok(png.clone()),
            _ => Err(WorldAssetError::new("missing fixture")),
        })
        .unwrap();
        let live: Value = serde_json::from_str(&product.live_directions_json).unwrap();
        let frames = live["entities"][0]["actionLayers"]["walking:Down"]["frames"]
            .as_array()
            .unwrap();
        assert_eq!(frames.len(), 8);
        assert_eq!(frames[0][0]["path"], "/original-ui/Mount/00/64.png");
        assert_eq!(frames[0][1]["path"], "/original-ui/CArmour/00/480.png");
        assert_eq!(frames[7][0]["path"], "/original-ui/Mount/00/71.png");
        assert_eq!(frames[7][1]["path"], "/original-ui/CArmour/00/487.png");
    }

    #[test]
    fn configured_shared_atlas_resolves_real_player_frame_when_present() {
        let Ok(root) = std::env::var("MIR2_ANDROID_ENTITY_ASSET_ROOT") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[{
                "objectId":"42","kind":"selfPlayer","x":302,"y":634,"direction":"Down",
                "sprite":{"bodyLibrary":"CArmour/00","frameBaseOffset":0,"directionStride":4}
            }]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 19, |asset, max_bytes| {
            let path = if asset == DNITEMS_META_ASSET {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../web/public/original-ui/DNItems/meta.json")
            } else {
                root.join(asset)
            };
            let bytes = std::fs::read(path).map_err(|error| {
                WorldAssetError::new(format!(
                    "configured entity asset could not be read: {error}"
                ))
            })?;
            if bytes.is_empty() || bytes.len() > max_bytes {
                return Err(WorldAssetError::new(
                    "configured entity asset has an invalid byte count",
                ));
            }
            Ok(bytes)
        })
        .unwrap();
        assert_eq!(product.entity_count, 1);
        assert_eq!(product.layer_count, 1);
        assert_eq!(product.unresolved_entity_count, 0);
        assert!((1..=7).contains(&product.pages.len()));
        assert!(product.compressed_bytes > 1_000_000);
        assert_eq!(product.rgba_bytes, product.pages.len() * 2048 * 2048 * 4);
        let live: serde_json::Value = serde_json::from_str(&product.live_directions_json).unwrap();
        assert_eq!(
            live["entities"][0]["directionLayers"]
                .as_object()
                .map(serde_json::Map::len),
            Some(8)
        );
        assert_eq!(
            live["entities"][0]["actionLayers"]["attack1:Down"]["frames"]
                .as_array()
                .map(Vec::len),
            Some(6)
        );
    }

    #[test]
    fn configured_shared_atlas_resolves_real_assassin_layers_when_present() {
        let Ok(root) = std::env::var("MIR2_ANDROID_ENTITY_ASSET_ROOT") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let snapshot = serde_json::json!({
            "playerObjectId":"42",
            "entities":[{
                "objectId":"42","kind":"selfPlayer","classKey":"assassin",
                "x":300,"y":630,"direction":"Down",
                "sprite":{
                    "bodyLibrary":"CArmour/00","frameBaseOffset":0,
                    "altBodyLibrary":"AArmour/00","altHairLibrary":"AHair/00",
                    "altWeaponLibrary":"AWeapon/00 R",
                    "altWeaponLibrarySecondary":"AWeapon/00 L",
                    "altFrameBaseOffset":0,"altWeaponFrameOffset":0,
                    "frameCount":4,"directionStride":4
                }
            }]
        })
        .to_string();
        let product = load_entity_render_state(&snapshot, &scene(), 34, |asset, max_bytes| {
            let path = if asset == DNITEMS_META_ASSET {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../web/public/original-ui/DNItems/meta.json")
            } else {
                root.join(asset)
            };
            let bytes = std::fs::read(path).map_err(|error| {
                WorldAssetError::new(format!(
                    "configured entity asset could not be read: {error}"
                ))
            })?;
            if bytes.is_empty() || bytes.len() > max_bytes {
                return Err(WorldAssetError::new(
                    "configured entity asset has an invalid byte count",
                ));
            }
            Ok(bytes)
        })
        .unwrap();
        assert_eq!(product.unresolved_entity_count, 0);
        let live: Value = serde_json::from_str(&product.live_directions_json).unwrap();
        let frames = live["entities"][0]["actionLayers"]["attack1:Down"]["frames"]
            .as_array()
            .unwrap();
        assert_eq!(frames.len(), 6);
        assert_eq!(frames[0][0]["path"], "/original-ui/AWeapon/00 L/160.png");
        assert_eq!(frames[5][3]["path"], "/original-ui/AWeapon/00 R/165.png");
    }
}
