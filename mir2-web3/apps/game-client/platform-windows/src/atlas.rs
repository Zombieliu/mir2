//! Native entity-atlas loading and viewport render-state construction.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex, OnceLock,
};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::assets;
use mir2_bevy_runtime::entity_animation::AnimationAction;

const STAGE_WIDTH: f32 = 1024.0;
const STAGE_HEIGHT: f32 = 768.0;
const CELL_WIDTH: f32 = 48.0;
const CELL_HEIGHT: f32 = 32.0;
// Runtime map tiles multiply their cell depth by ten before converting to
// world-z. Entity layers must use the same band, with order 5 between the map
// middle (0) and front (10) layers for the same cell.
const ENTITY_DEPTH_GAIN: f32 = 10.0;
const ENTITY_MOUNT_ORDER: f32 = 2.0;
const ENTITY_REAR_WEAPON_ORDER: f32 = 4.0;
const ENTITY_BODY_ORDER: f32 = 5.0;
const ENTITY_HAIR_ORDER: f32 = 6.0;
const ENTITY_FRONT_WEAPON_ORDER: f32 = 7.0;
const MAP_FRONT_ORDER: f32 = crate::map_parser::MAP_FRONT_DEPTH_ORDER * ENTITY_DEPTH_GAIN;
const POST_WORLD_BAND_GAP: f32 = 20.0;
const TARGET_HIGHLIGHT_OPACITY: f64 = 0.3;
const ORIGINAL_FRAME_PIXEL_CACHE_LIMIT: usize = 256;

#[derive(Debug, Clone)]
struct StarterAtlasRect {
    key: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    offset_x: Option<i32>,
    offset_y: Option<i32>,
}

#[derive(Debug, Clone)]
struct StarterAtlasPage {
    key: String,
    image_url: String,
    width: u32,
    height: u32,
    expected_sha256: Option<String>,
    expected_image_bytes: Option<u64>,
    rects: Vec<StarterAtlasRect>,
}

#[derive(Debug)]
struct StarterAtlasIndex {
    pages: Vec<StarterAtlasPage>,
    rect_by_path: HashMap<String, (usize, usize)>,
}

#[derive(Debug, Clone)]
struct StarterAtlasPixelPage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

type StarterAtlasPixels = HashMap<String, StarterAtlasPixelPage>;

static STARTER_ATLAS_INDEX: OnceLock<Option<StarterAtlasIndex>> = OnceLock::new();
static STARTER_ATLAS_PIXELS: OnceLock<Option<StarterAtlasPixels>> = OnceLock::new();
static ORIGINAL_FRAME_GEOMETRY_CACHE: OnceLock<
    Mutex<HashMap<String, Option<HashMap<i64, OriginalFrameGeometry>>>>,
> = OnceLock::new();
static ORIGINAL_FRAME_PIXEL_CACHE: OnceLock<Mutex<OriginalFramePixelCache>> = OnceLock::new();
static RENDER_TRACE_STATE_LOGS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OriginalFrameGeometry {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) offset_x: i32,
    pub(crate) offset_y: i32,
}

#[derive(Default)]
struct OriginalFramePixelCache {
    frames: HashMap<String, Option<Arc<StarterAtlasPixelPage>>>,
    insertion_order: VecDeque<String>,
}

#[derive(bevy::prelude::Resource, Default)]
pub struct NativeRenderTrace {
    frames: u32,
}

/// Optional native-only renderer probe. Entity layers use a raw depth ending in
/// 5 while map layers end in 0, so this can verify that Bevy actually spawned
/// atlas sprites without exposing runtime-internal marker components.
pub fn trace_rendered_entity_sprites(
    mut trace: bevy::prelude::ResMut<NativeRenderTrace>,
    sprites: bevy::prelude::Query<(&bevy::prelude::Transform, &bevy::prelude::Sprite)>,
) {
    trace.frames = trace.frames.saturating_add(1);
    if trace.frames > 720 || trace.frames % 120 != 0 {
        return;
    }
    let candidates = sprites
        .iter()
        .filter_map(|(transform, sprite)| {
            let raw_depth = (transform.translation.z * 100_000.0).round() as i64;
            (raw_depth.rem_euclid(10) == 5).then_some((
                transform.translation,
                sprite.custom_size,
                sprite.texture_atlas.as_ref().map(|atlas| atlas.index),
                sprite.image.id(),
            ))
        })
        .collect::<Vec<_>>();
    eprintln!(
        "[atlas] Bevy entity sprites={} samples={:?}",
        candidates.len(),
        candidates.iter().take(4).collect::<Vec<_>>()
    );
}

fn starter_atlas_index() -> Option<&'static StarterAtlasIndex> {
    STARTER_ATLAS_INDEX
        .get_or_init(load_starter_atlas_index)
        .as_ref()
}

fn load_starter_atlas_index() -> Option<StarterAtlasIndex> {
    let path = assets::asset_path("bevy-entity-atlases/manifest.json")?;
    let manifest: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    parse_starter_atlas_manifest(&manifest)
}

fn parse_starter_atlas_manifest(manifest: &Value) -> Option<StarterAtlasIndex> {
    let mut pages = Vec::new();
    let mut rect_by_path = HashMap::new();
    let integrity_required = manifest
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .is_some_and(|version| version >= 2);

    for atlas in manifest.get("atlases")?.as_array()? {
        let atlas_key = atlas.get("key")?.as_str()?;
        let atlas_rects = atlas.get("rects")?.as_array()?;
        let page_descriptors = atlas
            .get("pages")
            .and_then(Value::as_array)
            .filter(|descriptors| !descriptors.is_empty())
            .map(|descriptors| descriptors.as_slice())
            .unwrap_or_else(|| std::slice::from_ref(atlas));

        for (manifest_page_index, descriptor) in page_descriptors.iter().enumerate() {
            let key = if manifest_page_index == 0 {
                atlas_key.to_owned()
            } else {
                format!("{atlas_key}:p{manifest_page_index}")
            };
            let image_url = descriptor
                .get("imageUrl")
                .or_else(|| atlas.get("imageUrl"))?
                .as_str()?
                .to_owned();
            let width = descriptor
                .get("width")
                .or_else(|| atlas.get("width"))?
                .as_u64()? as u32;
            let height = descriptor
                .get("height")
                .or_else(|| atlas.get("height"))?
                .as_u64()? as u32;
            let expected_sha256 = descriptor
                .get("sha256")
                .and_then(Value::as_str)
                .map(str::to_ascii_lowercase)
                .filter(|hash| {
                    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                });
            let expected_image_bytes = descriptor.get("imageBytes").and_then(Value::as_u64);
            if integrity_required
                && (expected_sha256.is_none()
                    || expected_image_bytes.is_none_or(|bytes| bytes == 0))
            {
                return None;
            }
            let page_vector_index = pages.len();
            let mut rects = Vec::new();

            for rect in atlas_rects {
                let rect_page_index =
                    rect.get("pageIndex").and_then(Value::as_u64).unwrap_or(0) as usize;
                if rect_page_index != manifest_page_index {
                    continue;
                }

                let rect_key = rect.get("key")?.as_str()?.to_owned();
                let frame_path = rect_key
                    .split_once('|')
                    .map(|(path, _)| path)
                    .unwrap_or(rect_key.as_str())
                    .to_owned();
                let parsed = StarterAtlasRect {
                    key: rect_key,
                    x: rect.get("x")?.as_u64()? as u32,
                    y: rect.get("y")?.as_u64()? as u32,
                    width: rect.get("width")?.as_u64()? as u32,
                    height: rect.get("height")?.as_u64()? as u32,
                    offset_x: rect
                        .get("offsetX")
                        .and_then(Value::as_i64)
                        .and_then(|value| i32::try_from(value).ok()),
                    offset_y: rect
                        .get("offsetY")
                        .and_then(Value::as_i64)
                        .and_then(|value| i32::try_from(value).ok()),
                };
                rect_by_path.insert(frame_path, (page_vector_index, rects.len()));
                rects.push(parsed);
            }

            pages.push(StarterAtlasPage {
                key,
                image_url,
                width,
                height,
                expected_sha256,
                expected_image_bytes,
                rects,
            });
        }
    }

    (!pages.is_empty()).then_some(StarterAtlasIndex {
        pages,
        rect_by_path,
    })
}

/// Decode a generated PNG into raw RGBA pixels.
fn decode_png_rgba(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let output_size = reader.output_buffer_size().unwrap_or(0);
    let mut buffer = vec![0u8; output_size];
    let info = reader.next_frame(&mut buffer).ok()?;
    let rgba = match (info.color_type, info.bit_depth) {
        (png::ColorType::Rgba, png::BitDepth::Eight) => buffer[..info.buffer_size()].to_vec(),
        (png::ColorType::Rgb, png::BitDepth::Eight) => rgb_to_rgba(&buffer[..info.buffer_size()]),
        (color_type, bit_depth) => {
            eprintln!("[atlas] unsupported PNG format {color_type:?}/{bit_depth:?}");
            return None;
        }
    };
    Some((info.width, info.height, rgba))
}

type DecodedStarterAtlasPage = (String, u32, u32, Vec<u8>, PathBuf);

fn decode_starter_entity_atlas_pages(
    index: &StarterAtlasIndex,
) -> Option<Vec<DecodedStarterAtlasPage>> {
    let mut decoded = Vec::new();
    for page in &index.pages {
        let path = assets::asset_path(&page.image_url)?;
        let bytes = fs::read(&path).ok()?;
        if page
            .expected_image_bytes
            .is_some_and(|expected| expected != bytes.len() as u64)
        {
            eprintln!(
                "[atlas] page {} byte count is {}, manifest expects {:?}",
                page.key,
                bytes.len(),
                page.expected_image_bytes
            );
            return None;
        }
        if let Some(expected) = &page.expected_sha256 {
            let actual = format!("{:x}", Sha256::digest(&bytes));
            if &actual != expected {
                eprintln!(
                    "[atlas] page {} SHA-256 is {actual}, manifest expects {expected}",
                    page.key
                );
                return None;
            }
        }
        let Some((width, height, pixels)) = decode_png_rgba(&bytes) else {
            eprintln!("[atlas] failed to decode {path:?}");
            return None;
        };
        if width != page.width || height != page.height {
            eprintln!(
                "[atlas] page {} dimensions are {width}x{height}, manifest expects {}x{}",
                page.key, page.width, page.height
            );
            return None;
        }
        decoded.push((page.key.clone(), width, height, pixels, path));
    }
    Some(decoded)
}

#[cfg(test)]
pub(crate) fn validate_starter_entity_atlas_pages_for_test() -> bool {
    starter_atlas_index()
        .and_then(decode_starter_entity_atlas_pages)
        .is_some()
}

fn rgb_to_rgba(rgb: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for chunk in rgb.chunks_exact(3) {
        rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
    }
    rgba
}

/// Load every manifest page before publishing any of them, so a partial asset
/// bundle cannot switch the renderer into an incomplete atlas state.
pub fn load_starter_entity_atlas() -> bool {
    let Some(index) = starter_atlas_index() else {
        eprintln!("[atlas] no entity atlas manifest found; keeping colored fallback");
        return false;
    };

    let Some(decoded) = decode_starter_entity_atlas_pages(index) else {
        eprintln!("[atlas] entity atlas page closure failed; keeping colored fallback");
        return false;
    };

    let mut pixel_pages = StarterAtlasPixels::new();
    for (key, width, height, pixels, path) in decoded {
        if !mir2_bevy_runtime::native_ingest::push_native_entity_render_atlas(
            key.clone(),
            width,
            height,
            pixels.clone(),
        ) {
            return false;
        }
        pixel_pages.insert(
            key.clone(),
            StarterAtlasPixelPage {
                width,
                height,
                rgba: pixels,
            },
        );
        eprintln!("[atlas] pushed {key} {width}x{height} ({path:?})");
    }
    let _ = STARTER_ATLAS_PIXELS.set(Some(pixel_pages));
    true
}

fn direction_index(direction: &str) -> u32 {
    match direction.to_ascii_lowercase().as_str() {
        "up" => 0,
        "upright" => 1,
        "right" => 2,
        "downright" => 3,
        "down" => 4,
        "downleft" => 5,
        "left" => 6,
        "upleft" => 7,
        _ => 4,
    }
}

/// Resolve the frame selected by the server's sprite metadata.
pub fn starter_frame(
    kind: &str,
    object_id: &str,
    direction: &str,
    direction_stride: Option<i64>,
    frame_base_offset: Option<i64>,
) -> (String, i64) {
    let frame = match (direction_stride, frame_base_offset) {
        (Some(stride), Some(base)) if stride > 0 && base >= 0 => {
            base + stride * i64::from(direction_index(direction))
        }
        _ => i64::from(
            object_id
                .bytes()
                .fold(0u8, |acc, byte| acc.wrapping_add(byte))
                % 8,
        ),
    };
    let library = match kind {
        "selfPlayer" | "player" => "/original-ui/AArmour/00",
        "monster" => "/original-ui/Monster/000",
        _ => "/original-ui/NPC/00",
    };
    (library.to_owned(), frame)
}

fn sprite_library(sprite: Option<&Value>, field: &str) -> Option<String> {
    let raw = sprite?.get(field)?.as_str()?.trim();
    if raw.is_empty() || raw.contains("..") || raw.contains(['\\', ':']) {
        return None;
    }
    if !raw
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "/_- ".contains(character))
    {
        return None;
    }
    let raw = raw
        .trim_matches('/')
        .strip_prefix("original-ui/")
        .unwrap_or(raw.trim_matches('/'));
    Some(format!("/original-ui/{raw}"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedNativeSprite {
    pub(crate) body_library: String,
    pub(crate) hair_library: Option<String>,
    pub(crate) weapon_library: Option<String>,
    pub(crate) weapon_library_secondary: Option<String>,
    pub(crate) mount_library: Option<String>,
    pub(crate) body_base_offset: i64,
    pub(crate) weapon_frame_offset: Option<i64>,
    pub(crate) mount_frame_offset: i64,
}

impl ResolvedNativeSprite {
    pub(crate) fn mounted(&self) -> bool {
        self.mount_library.is_some()
    }
}

fn sprite_number(sprite: Option<&Value>, field: &str) -> Option<i64> {
    sprite?.get(field)?.as_i64()
}

fn normalized_library_key(library: &str) -> &str {
    library
        .trim()
        .trim_matches('/')
        .strip_prefix("original-ui/")
        .unwrap_or_else(|| library.trim().trim_matches('/'))
}

fn is_player_sprite_library(library: &str) -> bool {
    let mut segments = normalized_library_key(library).split('/');
    let Some(family) = segments.next() else {
        return false;
    };
    let Some(name) = segments.next() else {
        return false;
    };
    !name.is_empty()
        && segments.next().is_none()
        && matches!(
            family,
            "CArmour"
                | "CHair"
                | "CWeapon"
                | "ARArmour"
                | "ARHair"
                | "ARWeapon"
                | "AArmour"
                | "AHair"
                | "AWeapon"
                | "Mount"
        )
}

fn player_frame_path_parts(frame_path: &str) -> Option<(String, i64, String)> {
    let relative = frame_path.trim().trim_start_matches('/').replace('\\', "/");
    let source_path = relative.strip_prefix("original-ui/")?;
    let (library, file_name) = source_path.rsplit_once('/')?;
    if !is_player_sprite_library(library) {
        return None;
    }
    let frame = file_name.strip_suffix(".png")?.parse::<i64>().ok()?;
    if frame < 0 || relative != format!("original-ui/{library}/{frame}.png") {
        return None;
    }
    Some((library.to_owned(), frame, relative))
}

fn parse_original_frame_geometry(
    payload: &Value,
    library: &str,
) -> Option<HashMap<i64, OriginalFrameGeometry>> {
    let normalized_library = normalized_library_key(library);
    let frames = payload.get("frames")?.as_array()?;
    let mut geometry = HashMap::with_capacity(frames.len());
    for frame in frames {
        let index = frame.get("index")?.as_i64()?;
        if index < 0 {
            continue;
        }
        let width = frame
            .get("width")?
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())?;
        let height = frame
            .get("height")?
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())?;
        let offset_x = frame
            .get("x")?
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())?;
        let offset_y = frame
            .get("y")?
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())?;
        if width == 0 || height == 0 {
            continue;
        }
        let Some(path) = frame.get("path").and_then(Value::as_str) else {
            continue;
        };
        let expected_path = format!("/original-ui/{normalized_library}/{index}.png");
        if path != expected_path {
            continue;
        }
        geometry.insert(
            index,
            OriginalFrameGeometry {
                width,
                height,
                offset_x,
                offset_y,
            },
        );
    }
    (!geometry.is_empty()).then_some(geometry)
}

fn load_original_frame_geometry(library: &str) -> Option<HashMap<i64, OriginalFrameGeometry>> {
    let meta_path = format!("original-ui/{}/meta.json", normalized_library_key(library));
    let path = assets::asset_path(&meta_path)?;
    let payload: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    parse_original_frame_geometry(&payload, library)
}

fn original_frame_geometry(library: &str, frame: i64) -> Option<OriginalFrameGeometry> {
    let library = normalized_library_key(library).to_owned();
    let cache = ORIGINAL_FRAME_GEOMETRY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().ok()?;
    if !cache.contains_key(&library) {
        let loaded = load_original_frame_geometry(&library);
        cache.insert(library.clone(), loaded);
    }
    cache
        .get(&library)
        .and_then(Option::as_ref)
        .and_then(|frames| frames.get(&frame))
        .copied()
}

/// Resolve the same source-frame size and draw offset used by the native body
/// renderer. Crystal's NPC quest marker anchors to `BodyLibrary` frame zero,
/// so UI overlays must consume this geometry instead of a fixed tile offset.
pub(crate) fn native_frame_geometry(library: &str, frame: i64) -> Option<OriginalFrameGeometry> {
    let normalized_library = normalized_library_key(library);
    let frame_path = format!("/original-ui/{normalized_library}/{frame}.png");
    let atlas_frame_path = frame_path.replace(' ', "%20");
    if let Some(rect) = starter_atlas_index().and_then(|index| {
        index
            .rect_by_path
            .get(&atlas_frame_path)
            .map(|(page_index, rect_index)| &index.pages[*page_index].rects[*rect_index])
    }) {
        if let (Some(offset_x), Some(offset_y)) = (rect.offset_x, rect.offset_y) {
            return Some(OriginalFrameGeometry {
                width: rect.width,
                height: rect.height,
                offset_x,
                offset_y,
            });
        }
    }
    original_frame_geometry(normalized_library, frame)
}

fn verified_player_frame_geometry(frame_path: &str) -> Option<OriginalFrameGeometry> {
    let (library, frame, relative) = player_frame_path_parts(frame_path)?;
    assets::asset_path(&relative).filter(|path| path.is_file())?;
    original_frame_geometry(&library, frame)
}

fn original_frame_pixels(frame_path: &str) -> Option<Arc<StarterAtlasPixelPage>> {
    let (_, _, cache_key) = player_frame_path_parts(frame_path)?;
    let cache = ORIGINAL_FRAME_PIXEL_CACHE.get_or_init(|| Mutex::new(Default::default()));
    if let Ok(cache) = cache.lock() {
        if let Some(cached) = cache.frames.get(&cache_key) {
            return cached.clone();
        }
    }

    let loaded = assets::asset_path(&cache_key)
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| decode_png_rgba(&bytes))
        .map(|(width, height, rgba)| {
            Arc::new(StarterAtlasPixelPage {
                width,
                height,
                rgba,
            })
        });

    let mut cache = cache.lock().ok()?;
    if let Some(cached) = cache.frames.get(&cache_key) {
        return cached.clone();
    }
    while cache.frames.len() >= ORIGINAL_FRAME_PIXEL_CACHE_LIMIT {
        let Some(oldest) = cache.insertion_order.pop_front() else {
            break;
        };
        cache.frames.remove(&oldest);
    }
    cache.insertion_order.push_back(cache_key.clone());
    cache.frames.insert(cache_key, loaded.clone());
    loaded
}

fn sprite_library_exists(library: &str) -> bool {
    let encoded_prefix = format!("{}/", library.trim_end_matches('/')).replace(' ', "%20");
    starter_atlas_index().is_some_and(|index| {
        index
            .rect_by_path
            .keys()
            .any(|path| path.starts_with(&encoded_prefix))
    }) || assets::asset_path(library.trim_start_matches('/')).is_some_and(|path| path.is_dir())
}

fn available_sprite_library(sprite: Option<&Value>, field: &str) -> Option<String> {
    sprite_library(sprite, field).filter(|library| sprite_library_exists(library))
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

pub(crate) fn resolved_native_sprite(
    entity: &Value,
    action: AnimationAction,
) -> ResolvedNativeSprite {
    let kind = entity
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("monster");
    let sprite = entity.get("sprite");
    let (fallback_library, _) = starter_frame(kind, "0", "down", None, None);
    let body_library = sprite_library(sprite, "bodyLibrary").unwrap_or(fallback_library);
    let hair_library = sprite_library(sprite, "hairLibrary");
    let mut weapon_library = sprite_library(sprite, "weaponLibrary");
    let mut weapon_library_secondary = sprite_library(sprite, "weaponLibrarySecondary");
    let mount_library = sprite_library(sprite, "mountLibrary");
    let transform_mounted = entity
        .get("transformType")
        .and_then(Value::as_i64)
        .is_some_and(|value| value >= 0)
        && entity.get("ridingMount").and_then(Value::as_bool) == Some(true)
        && entity
            .get("mountType")
            .and_then(Value::as_i64)
            .is_some_and(|value| value > 6);
    let body_base_offset = sprite_number(sprite, "frameBaseOffset").unwrap_or(0)
        + if transform_mounted { -416 } else { 0 };
    let mut weapon_frame_offset = sprite_number(sprite, "weaponFrameOffset");
    let mount_frame_offset = sprite_number(sprite, "mountFrameOffset").unwrap_or(0);

    if !matches!(kind, "selfPlayer" | "player" | "hero") || mount_library.is_some() {
        return ResolvedNativeSprite {
            body_library,
            hair_library,
            weapon_library,
            weapon_library_secondary,
            mount_library,
            body_base_offset,
            weapon_frame_offset,
            mount_frame_offset,
        };
    }

    let class_key = entity
        .get("classKey")
        .and_then(Value::as_str)
        .unwrap_or("warrior")
        .to_ascii_lowercase();
    let gender_key = entity
        .get("genderKey")
        .and_then(Value::as_str)
        .unwrap_or("male")
        .to_ascii_lowercase();
    let gender_body_offset = if gender_key == "female" { 808 } else { 0 };
    let gender_weapon_offset = if gender_key == "female" { 416 } else { 0 };
    let untransformed = entity
        .get("transformType")
        .and_then(Value::as_i64)
        .unwrap_or(-1)
        < 0;
    if action == AnimationAction::Harvest && untransformed {
        weapon_library = Some("/original-ui/CWeapon/01".to_owned());
        weapon_library_secondary = None;
        weapon_frame_offset = Some(gender_weapon_offset);
    }
    let alt_body_library = available_sprite_library(sprite, "altBodyLibrary");
    let alt_hair_library = available_sprite_library(sprite, "altHairLibrary");
    let alt_weapon_library = available_sprite_library(sprite, "altWeaponLibrary");
    let alt_weapon_library_secondary =
        available_sprite_library(sprite, "altWeaponLibrarySecondary");
    let archer_alt = class_key == "archer"
        && alt_body_library
            .as_deref()
            .is_some_and(|library| normalized_library_key(library).starts_with("ARArmour/"))
        && uses_archer_alt(action);
    let assassin_alt = class_key == "assassin"
        && alt_body_library
            .as_deref()
            .is_some_and(|library| normalized_library_key(library).starts_with("AArmour/"))
        && uses_assassin_alt(action);

    if archer_alt {
        return ResolvedNativeSprite {
            body_library: alt_body_library.unwrap_or(body_library),
            hair_library: alt_hair_library.or(hair_library),
            weapon_library: alt_weapon_library.or(weapon_library),
            weapon_library_secondary: None,
            mount_library,
            body_base_offset: sprite_number(sprite, "altFrameBaseOffset")
                .unwrap_or(gender_body_offset),
            weapon_frame_offset: sprite_number(sprite, "altWeaponFrameOffset")
                .or(weapon_frame_offset),
            mount_frame_offset,
        };
    }

    if assassin_alt {
        let has_alt_weapon = alt_weapon_library.is_some();
        return ResolvedNativeSprite {
            body_library: alt_body_library.unwrap_or(body_library),
            hair_library: alt_hair_library.or(hair_library),
            weapon_library: alt_weapon_library.or(weapon_library),
            weapon_library_secondary: alt_weapon_library_secondary.or(weapon_library_secondary),
            mount_library,
            body_base_offset: sprite_number(sprite, "altFrameBaseOffset")
                .unwrap_or(gender_body_offset),
            weapon_frame_offset: sprite_number(sprite, "altWeaponFrameOffset").or_else(|| {
                if has_alt_weapon {
                    Some(gender_weapon_offset)
                } else {
                    weapon_frame_offset
                }
            }),
            mount_frame_offset,
        };
    }

    ResolvedNativeSprite {
        body_library,
        hair_library,
        weapon_library,
        weapon_library_secondary,
        mount_library,
        body_base_offset,
        weapon_frame_offset,
        mount_frame_offset,
    }
}

fn payload_animation_action(entity: &Value) -> AnimationAction {
    match entity
        .get("_nativeAnimationAction")
        .and_then(Value::as_str)
        .unwrap_or("standing")
    {
        "harvest" => AnimationAction::Harvest,
        "show" => AnimationAction::Show,
        "hide" => AnimationAction::Hide,
        "walking" => AnimationAction::Walking,
        "running" => AnimationAction::Running,
        "attack1" => AnimationAction::Attack1,
        "attack2" => AnimationAction::Attack2,
        "attack3" => AnimationAction::Attack3,
        "attack4" => AnimationAction::Attack4,
        "attackRange1" => AnimationAction::AttackRange1,
        "attackRange2" => AnimationAction::AttackRange2,
        "dashAttack" => AnimationAction::DashAttack,
        "spell" => AnimationAction::Spell,
        "struck" => AnimationAction::Struck,
        "die" => AnimationAction::Die,
        "dead" => AnimationAction::Dead,
        "skeleton" => AnimationAction::Skeleton,
        "revive" => AnimationAction::Revive,
        _ => AnimationAction::Standing,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_entity_layer(
    index: &StarterAtlasIndex,
    used_rects: &mut HashMap<String, HashSet<String>>,
    key: String,
    library: &str,
    frame: i64,
    root_left: f32,
    root_top: f32,
    z: f32,
) -> Option<Value> {
    let frame_path = format!("{library}/{frame}.png");
    let atlas_frame_path = frame_path.replace(' ', "%20");
    let resolved = index
        .rect_by_path
        .get(&atlas_frame_path)
        .map(|(page_index, rect_index)| {
            let page = &index.pages[*page_index];
            (page, &page.rects[*rect_index])
        });
    let individual_geometry = resolved
        .is_none()
        .then(|| verified_player_frame_geometry(&frame_path));
    let individual_geometry = individual_geometry.flatten();
    if resolved.is_none() && individual_geometry.is_none() {
        return None;
    }

    let (width, height, source_offset_x, source_offset_y) = resolved
        .map(|(_, rect)| {
            (
                rect.width as f32,
                rect.height as f32,
                rect.offset_x.map(|value| value as f32),
                rect.offset_y.map(|value| value as f32),
            )
        })
        .or_else(|| {
            individual_geometry.map(|geometry| {
                (
                    geometry.width as f32,
                    geometry.height as f32,
                    Some(geometry.offset_x as f32),
                    Some(geometry.offset_y as f32),
                )
            })
        })?;
    let mut layer = json!({
        "key": key,
        "path": frame_path,
        "left": root_left + source_offset_x.unwrap_or((CELL_WIDTH - width) / 2.0),
        "top": root_top + source_offset_y.unwrap_or(CELL_HEIGHT - height),
        "width": width,
        "height": height,
        "z": z,
    });
    if let Some((page, rect)) = resolved {
        used_rects
            .entry(page.key.clone())
            .or_default()
            .insert(rect.key.clone());
        layer["atlasKey"] = json!(page.key);
        layer["atlasRectKey"] = json!(rect.key);
    }
    Some(layer)
}

fn weapon_is_rear(direction: &str) -> bool {
    matches!(
        direction.to_ascii_lowercase().as_str(),
        "left" | "up" | "upleft" | "downleft"
    )
}

fn assassin_primary_is_front(direction: &str) -> bool {
    matches!(
        direction.to_ascii_lowercase().as_str(),
        "upright" | "right" | "downright" | "down"
    )
}

fn scene_center(payload: &Value) -> (i64, i64) {
    payload
        .get("sceneView")
        .and_then(|view| view.get("center"))
        .map(|center| {
            (
                center.get("x").and_then(Value::as_i64).unwrap_or(0),
                center.get("y").and_then(Value::as_i64).unwrap_or(0),
            )
        })
        .unwrap_or((0, 0))
}

/// Build viewport-relative entity layers with exact Crystal geometry. Atlas
/// rects stay preferred; an unpacked individual PNG must have the same
/// per-library `meta.json` geometry consumed by Web. Missing metadata fails
/// closed instead of stretching the frame into an invented 48x64 rectangle.
#[cfg(test)]
pub fn build_entity_render_state(payload: &Value) -> Option<Value> {
    build_entity_render_state_with_frames(payload, &HashMap::new())
}

/// Build the native entity render state while allowing the Windows animation
/// clock to select each object's current Crystal draw-frame. The Gateway-owned
/// payload remains the authority for object identity, position, direction and
/// sprite libraries; overrides affect only the visual frame within that library.
#[cfg(test)]
pub fn build_entity_render_state_with_frames(
    payload: &Value,
    frame_overrides: &HashMap<String, i64>,
) -> Option<Value> {
    build_entity_render_state_internal(payload, frame_overrides, None, true)
}

pub(crate) fn build_entity_render_state_with_poses(
    payload: &Value,
    pose_overrides: &HashMap<String, (i64, AnimationAction)>,
) -> Option<Value> {
    build_entity_render_state_with_poses_and_effect_visibility(payload, pose_overrides, true)
}

pub(crate) fn build_entity_render_state_with_poses_and_effect_visibility(
    payload: &Value,
    pose_overrides: &HashMap<String, (i64, AnimationAction)>,
    effect_visible: bool,
) -> Option<Value> {
    build_entity_render_state_internal(
        payload,
        &HashMap::new(),
        Some(pose_overrides),
        effect_visible,
    )
}

fn build_entity_render_state_internal(
    payload: &Value,
    frame_overrides: &HashMap<String, i64>,
    pose_overrides: Option<&HashMap<String, (i64, AnimationAction)>>,
    effect_visible: bool,
) -> Option<Value> {
    let index = starter_atlas_index()?;
    build_entity_render_state_with_index(
        payload,
        frame_overrides,
        pose_overrides,
        effect_visible,
        index,
        STARTER_ATLAS_PIXELS.get().and_then(Option::as_ref),
    )
}

fn build_entity_render_state_with_index(
    payload: &Value,
    frame_overrides: &HashMap<String, i64>,
    pose_overrides: Option<&HashMap<String, (i64, AnimationAction)>>,
    effect_visible: bool,
    index: &StarterAtlasIndex,
    pixels: Option<&StarterAtlasPixels>,
) -> Option<Value> {
    let (center_x, center_y) = scene_center(payload);
    let entity_origin_x = (STAGE_WIDTH / 2.0 / CELL_WIDTH).floor() * CELL_WIDTH;
    let entity_origin_y = ((STAGE_HEIGHT / 2.0 / CELL_HEIGHT).floor() - 1.0) * CELL_HEIGHT;
    let mut used_rects: HashMap<String, HashSet<String>> = HashMap::new();
    let selected_object_id = normalized_object_id(payload.get("selectedObjectId"));
    let highlight_target = payload
        .get("_nativeHighlightTarget")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let mut entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(|entities| {
            entities
                .iter()
                .map(|entity| {
                    let object_id =
                        normalized_object_id(entity.get("objectId")).unwrap_or_default();
                    let kind = entity
                        .get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or("monster");
                    let x = entity.get("x").and_then(Value::as_i64).unwrap_or(0);
                    let y = entity.get("y").and_then(Value::as_i64).unwrap_or(0);
                    let direction = entity
                        .get("direction")
                        .and_then(Value::as_str)
                        .unwrap_or("down");
                    let sprite = entity.get("sprite");
                    let (_, fallback_frame) = starter_frame(
                        kind,
                        &object_id,
                        direction,
                        sprite
                            .and_then(|value| value.get("directionStride"))
                            .and_then(Value::as_i64),
                        sprite
                            .and_then(|value| value.get("frameBaseOffset"))
                            .and_then(Value::as_i64),
                    );
                    let action = pose_overrides
                        .and_then(|poses| poses.get(&object_id))
                        .map(|(_, action)| *action)
                        .unwrap_or_else(|| payload_animation_action(entity));
                    let resolved_sprite = resolved_native_sprite(entity, action);
                    let relative_frame = pose_overrides
                        .and_then(|poses| poses.get(&object_id))
                        .map(|(frame, _)| *frame)
                        .or_else(|| frame_overrides.get(&object_id).copied())
                        .unwrap_or_else(|| {
                            fallback_frame.saturating_sub(resolved_sprite.body_base_offset)
                        });
                    let body_frame = resolved_sprite
                        .body_base_offset
                        .saturating_add(relative_frame);
                    let root_left = entity_origin_x + (x - center_x) as f32 * CELL_WIDTH;
                    let root_top = entity_origin_y + (y - center_y) as f32 * CELL_HEIGHT;
                    let z_base = entity_z_base(x, y);
                    let mut layers = Vec::new();

                    let mount_library = resolved_sprite.mount_library.as_ref();
                    // Crystal DrawMount subtracts the mounted-player base (416)
                    // before asking the mount library to draw. Player death and
                    // revive use the ordinary 384/387 body frames, so that lookup
                    // is negative and produces no mount layer. Do not clamp it to
                    // frame zero: that incorrectly leaves a standing mount under
                    // a dead player.
                    if let (Some(library), Some(mount_relative_frame)) =
                        (&mount_library, relative_frame.checked_sub(416))
                    {
                        let mount_frame =
                            mount_relative_frame.saturating_add(resolved_sprite.mount_frame_offset);
                        if let Some(layer) = build_entity_layer(
                            index,
                            &mut used_rects,
                            format!("{object_id}:mount"),
                            library,
                            mount_frame,
                            root_left,
                            root_top,
                            z_base + ENTITY_MOUNT_ORDER,
                        ) {
                            layers.push(layer);
                        }
                    }

                    let weapon_frame = resolved_sprite
                        .weapon_frame_offset
                        .map(|offset| offset.saturating_add(relative_frame));
                    let primary_weapon = resolved_sprite.weapon_library.as_ref();
                    let secondary_weapon = resolved_sprite.weapon_library_secondary.as_ref();
                    if mount_library.is_none() {
                        let dual_weapon = secondary_weapon.is_some();
                        for (role, library) in [
                            ("weapon-primary", primary_weapon),
                            ("weapon-secondary", secondary_weapon),
                        ] {
                            let (Some(library), Some(frame)) = (library, weapon_frame) else {
                                continue;
                            };
                            let rear = if dual_weapon {
                                if role == "weapon-primary" {
                                    !assassin_primary_is_front(direction)
                                } else {
                                    assassin_primary_is_front(direction)
                                }
                            } else {
                                weapon_is_rear(direction)
                            };
                            if let Some(layer) = build_entity_layer(
                                index,
                                &mut used_rects,
                                format!("{object_id}:{role}"),
                                library,
                                frame,
                                root_left,
                                root_top,
                                z_base
                                    + if rear {
                                        ENTITY_REAR_WEAPON_ORDER
                                    } else {
                                        ENTITY_FRONT_WEAPON_ORDER
                                    },
                            ) {
                                layers.push(layer);
                            }
                        }
                    }

                    if let Some(layer) = build_entity_layer(
                        index,
                        &mut used_rects,
                        format!("{object_id}:body"),
                        &resolved_sprite.body_library,
                        body_frame,
                        root_left,
                        root_top,
                        z_base + ENTITY_BODY_ORDER,
                    ) {
                        layers.push(layer);
                    }
                    if let Some(library) = resolved_sprite.hair_library.as_ref() {
                        if let Some(layer) = build_entity_layer(
                            index,
                            &mut used_rects,
                            format!("{object_id}:hair"),
                            &library,
                            body_frame,
                            root_left,
                            root_top,
                            z_base + ENTITY_HAIR_ORDER,
                        ) {
                            layers.push(layer);
                        }
                    }
                    let actor_layer_count = layers.len();
                    if let Some(effect_frame) = effect_visible
                        .then(|| {
                            scarecrow_die_effect_frame(
                                &resolved_sprite.body_library,
                                action,
                                relative_frame,
                                direction,
                            )
                        })
                        .flatten()
                    {
                        if let Some(mut layer) = build_entity_layer(
                            index,
                            &mut used_rects,
                            format!("{object_id}:scarecrow-die-effect"),
                            &resolved_sprite.body_library,
                            effect_frame,
                            root_left,
                            root_top,
                            post_world_effect_z(payload, x, y),
                        ) {
                            layer["additive"] = json!(true);
                            layers.push(layer);
                        }
                    }
                    if entity.get("hidden").and_then(Value::as_bool) == Some(true) {
                        for layer in &mut layers {
                            layer["opacity"] = json!(0.5);
                        }
                    }
                    let mut rendered_entity = json!({
                        "objectId": object_id,
                        "kind": kind,
                        "isSelf": kind == "selfPlayer",
                        "dead": entity.get("dead").and_then(Value::as_bool).unwrap_or(false),
                        "gridX": x,
                        "gridY": y,
                        "_nativeTargetable": entity
                            .get("kind")
                            .and_then(Value::as_str)
                            .is_some_and(|kind| matches!(kind, "player" | "monster" | "npc")),
                        "_nativeActorLayerCount": actor_layer_count,
                        "layers": layers,
                    });
                    for field in [
                        "motionFromX",
                        "motionFromY",
                        "motionToX",
                        "motionToY",
                        "motionStartedMs",
                        "motionDurationMs",
                    ] {
                        if let Some(value) = entity.get(field).filter(|value| !value.is_null()) {
                            rendered_entity[field] = value.clone();
                        }
                    }
                    rendered_entity
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Crystal keeps MouseObject/SelfPlayer.MouseOver alive even when the
    // optional target highlight blend is disabled. Nameplates consume these
    // identities independently; only the duplicate sprite blend below is
    // gated by HighlightTarget.
    let hovered_object_id = hovered_object_at_cursor(payload, &entities, index, pixels);
    let self_hovered = self_hovered_at_cursor(payload, &entities, index, pixels);
    if highlight_target {
        for entity in &mut entities {
            let object_id = entity
                .get("objectId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let kind = entity
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let actor_layer_count = entity
                .get("_nativeActorLayerCount")
                .and_then(Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or(0);
            let selected = entity
                .get("_nativeTargetable")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && matches!(kind, "player" | "monster")
                && selected_object_id.as_deref() == Some(object_id.as_str());
            let hovered = hovered_object_id.as_deref() == Some(object_id.as_str()) && !selected;
            let Some(layers) = entity.get_mut("layers").and_then(Value::as_array_mut) else {
                continue;
            };
            if hovered {
                append_actor_highlight(
                    layers,
                    actor_layer_count,
                    &object_id,
                    HighlightBand::Hover,
                    payload,
                );
            }
            if selected {
                append_actor_highlight(
                    layers,
                    actor_layer_count,
                    &object_id,
                    HighlightBand::Selected,
                    payload,
                );
            }
        }
    }
    for entity in &mut entities {
        entity.as_object_mut().map(|object| {
            object.remove("_nativeActorLayerCount");
            object.remove("_nativeTargetable");
        });
    }

    let atlases = index
        .pages
        .iter()
        .filter_map(|page| {
            let used = used_rects.get(&page.key)?;
            let rects = page
                .rects
                .iter()
                .filter(|rect| used.contains(&rect.key))
                .map(|rect| {
                    json!({
                        "key": rect.key,
                        "x": rect.x,
                        "y": rect.y,
                        "width": rect.width,
                        "height": rect.height,
                    })
                })
                .collect::<Vec<_>>();
            Some(json!({
                "key": page.key,
                "width": page.width,
                "height": page.height,
                "rects": rects,
            }))
        })
        .collect::<Vec<_>>();

    let state = json!({
        "enabled": true,
        "stageWidth": STAGE_WIDTH,
        "stageHeight": STAGE_HEIGHT,
        "centerX": center_x,
        "centerY": center_y,
        "hoveredObjectId": hovered_object_id,
        "selfHovered": self_hovered,
        "atlases": atlases,
        "entities": entities,
    });
    if std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some()
        && RENDER_TRACE_STATE_LOGS.fetch_add(1, Ordering::Relaxed) < 2
    {
        let entity_count = state["entities"].as_array().map_or(0, Vec::len);
        let atlas_count = state["atlases"].as_array().map_or(0, Vec::len);
        let summaries = payload
            .get("entities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .zip(state["entities"].as_array().into_iter().flatten())
            .map(|(source, rendered)| {
                json!({
                    "objectId": rendered.get("objectId"),
                    "name": source.get("name"),
                    "kind": source.get("kind"),
                    "gridX": rendered.get("gridX"),
                    "gridY": rendered.get("gridY"),
                    "sprite": source.get("sprite"),
                    "path": rendered
                        .get("layers")
                        .and_then(Value::as_array)
                        .and_then(|layers| layers.first())
                        .and_then(|layer| layer.get("path")),
                })
            })
            .collect::<Vec<_>>();
        eprintln!(
            "[atlas] render state center=({center_x},{center_y}) entities={entity_count} atlases={atlas_count} summaries={}",
            Value::Array(summaries)
        );
    }
    Some(state)
}

/// Unit-test seam for packet/animation-to-atlas routing. Production callers
/// remain fail-closed on `starter_atlas_index()` and the complete Candidate
/// asset root; this helper cannot replace packaging or asset-closure gates.
#[cfg(test)]
pub(crate) fn build_entity_render_state_with_manifest_for_test(
    payload: &Value,
    pose_overrides: &HashMap<String, (i64, AnimationAction)>,
    effect_visible: bool,
    manifest: &Value,
) -> Option<Value> {
    let index = parse_starter_atlas_manifest(manifest)?;
    build_entity_render_state_with_index(
        payload,
        &HashMap::new(),
        Some(pose_overrides),
        effect_visible,
        &index,
        None,
    )
}

#[cfg(test)]
pub(crate) fn build_entity_render_state_with_manifest_and_pixels_for_test(
    payload: &Value,
    pose_overrides: &HashMap<String, (i64, AnimationAction)>,
    effect_visible: bool,
    manifest: &Value,
    pixels: &HashMap<String, (u32, u32, Vec<u8>)>,
) -> Option<Value> {
    let index = parse_starter_atlas_manifest(manifest)?;
    let pixels = pixels
        .iter()
        .map(|(key, (width, height, rgba))| {
            (
                key.clone(),
                StarterAtlasPixelPage {
                    width: *width,
                    height: *height,
                    rgba: rgba.clone(),
                },
            )
        })
        .collect::<StarterAtlasPixels>();
    build_entity_render_state_with_index(
        payload,
        &HashMap::new(),
        Some(pose_overrides),
        effect_visible,
        &index,
        Some(&pixels),
    )
}

fn entity_z_base(x: i64, y: i64) -> f32 {
    y.saturating_mul(1_000).saturating_add(x.saturating_mul(10)) as f32 * ENTITY_DEPTH_GAIN
}

fn normalized_object_id(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Number(number) => Some(number.to_string()),
        Value::String(string) if !string.is_empty() => Some(string.clone()),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HighlightBand {
    Hover,
    Selected,
}

fn append_actor_highlight(
    layers: &mut Vec<Value>,
    actor_layer_count: usize,
    object_id: &str,
    band: HighlightBand,
    payload: &Value,
) {
    let Some(actor_layers) = layers.get(..actor_layer_count) else {
        return;
    };
    let prefix = format!("{object_id}:");
    let highlights = actor_layers
        .iter()
        .map(|layer| {
            let key = layer.get("key").and_then(Value::as_str)?;
            let role = key.strip_prefix(&prefix)?;
            // DrawBlend is atomic across the complete actor composite. A layer
            // may be atlas-backed or a verified individual PNG; only a broken
            // half-atlas binding suppresses the full redraw.
            match (
                layer.get("atlasKey").and_then(Value::as_str),
                layer.get("atlasRectKey").and_then(Value::as_str),
            ) {
                (Some(_), Some(_)) => {}
                (None, None) => {
                    let frame_path = layer.get("path").and_then(Value::as_str)?;
                    let geometry = verified_player_frame_geometry(frame_path)?;
                    if layer.get("width").and_then(Value::as_f64) != Some(f64::from(geometry.width))
                        || layer.get("height").and_then(Value::as_f64)
                            != Some(f64::from(geometry.height))
                    {
                        return None;
                    }
                }
                _ => return None,
            }
            let normal_z = layer.get("z").and_then(Value::as_f64)? as f32;
            let (name, z) = match band {
                HighlightBand::Hover => ("hover-highlight", post_world_hover_z(payload, normal_z)),
                HighlightBand::Selected => {
                    ("target-highlight", post_world_selected_z(payload, normal_z))
                }
            };
            let mut highlight = layer.clone();
            highlight["key"] = json!(format!("{object_id}:{name}:{role}"));
            highlight["z"] = json!(z);
            highlight["opacity"] = json!(TARGET_HIGHLIGHT_OPACITY);
            highlight["additive"] = json!(false);
            Some(highlight)
        })
        .collect::<Option<Vec<_>>>();
    if let Some(highlights) = highlights {
        layers.extend(highlights);
    }
}

fn hovered_object_at_cursor(
    payload: &Value,
    entities: &[Value],
    index: &StarterAtlasIndex,
    pixels: Option<&StarterAtlasPixels>,
) -> Option<String> {
    let (cursor_x, cursor_y, cursor_grid_x, cursor_grid_y) = cursor_hit_context(payload)?;

    // Crystal scans the cursor tile's 5x5 neighbourhood from bottom-right to
    // top-left and each cell's object list in reverse insertion order.
    for y in ((cursor_grid_y - 2)..=(cursor_grid_y + 2)).rev() {
        for x in ((cursor_grid_x - 2)..=(cursor_grid_x + 2)).rev() {
            for entity in entities.iter().rev() {
                if entity.get("gridX").and_then(Value::as_i64) != Some(x)
                    || entity.get("gridY").and_then(Value::as_i64) != Some(y)
                    || entity.get("isSelf").and_then(Value::as_bool) == Some(true)
                    || entity.get("dead").and_then(Value::as_bool) == Some(true)
                    || entity.get("_nativeTargetable").and_then(Value::as_bool) != Some(true)
                {
                    continue;
                }
                let object_id = entity.get("objectId").and_then(Value::as_str)?;
                if (x == cursor_grid_x && y == cursor_grid_y)
                    || body_visible_pixel(entity, object_id, cursor_x, cursor_y, index, pixels)
                    || npc_body_bounds_fallback_hit(entity, cursor_x, cursor_y, index, pixels)
                {
                    return Some(object_id.to_owned());
                }
            }
        }
    }
    None
}

fn npc_body_bounds_fallback_hit(
    entity: &Value,
    cursor_x: f32,
    cursor_y: f32,
    index: &StarterAtlasIndex,
    pixels: Option<&StarterAtlasPixels>,
) -> bool {
    if entity.get("kind").and_then(Value::as_str) != Some("npc") {
        return false;
    }
    let Some(object_id) = entity.get("objectId").and_then(Value::as_str) else {
        return false;
    };
    match body_hit_availability(entity, object_id, cursor_x, cursor_y, index, pixels) {
        BodyHitAvailability::Hit => true,
        BodyHitAvailability::Miss => false,
        BodyHitAvailability::UnavailableWithinBounds => true,
        BodyHitAvailability::OutOfBounds => false,
    }
}

enum BodyHitAvailability {
    Hit,
    Miss,
    UnavailableWithinBounds,
    OutOfBounds,
}

fn body_hit_availability(
    entity: &Value,
    object_id: &str,
    cursor_x: f32,
    cursor_y: f32,
    index: &StarterAtlasIndex,
    pixels: Option<&StarterAtlasPixels>,
) -> BodyHitAvailability {
    let actor_layer_count = entity
        .get("_nativeActorLayerCount")
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(0);
    let body_key = format!("{object_id}:body");
    let Some(body) = entity
        .get("layers")
        .and_then(Value::as_array)
        .and_then(|layers| layers.get(..actor_layer_count))
        .and_then(|layers| layers.iter().find(|layer| layer["key"] == body_key))
    else {
        return BodyHitAvailability::OutOfBounds;
    };
    let Some(left) = body.get("left").and_then(Value::as_f64).map(|v| v as f32) else {
        return BodyHitAvailability::OutOfBounds;
    };
    let Some(top) = body.get("top").and_then(Value::as_f64).map(|v| v as f32) else {
        return BodyHitAvailability::OutOfBounds;
    };
    let Some(width) = body.get("width").and_then(Value::as_f64).map(|v| v as f32) else {
        return BodyHitAvailability::OutOfBounds;
    };
    let Some(height) = body.get("height").and_then(Value::as_f64).map(|v| v as f32) else {
        return BodyHitAvailability::OutOfBounds;
    };
    if cursor_x < left || cursor_y < top || cursor_x >= left + width || cursor_y >= top + height {
        return BodyHitAvailability::OutOfBounds;
    }
    let local_x = (cursor_x - left).floor() as u32;
    let local_y = (cursor_y - top).floor() as u32;
    let atlas_key = body.get("atlasKey").and_then(Value::as_str);
    let rect_key = body.get("atlasRectKey").and_then(Value::as_str);
    if atlas_key.is_none() && rect_key.is_none() {
        let Some(frame_path) = body.get("path").and_then(Value::as_str) else {
            return BodyHitAvailability::UnavailableWithinBounds;
        };
        let Some(frame_pixels) = original_frame_pixels(frame_path) else {
            return BodyHitAvailability::UnavailableWithinBounds;
        };
        if frame_pixels.width as f32 != width || frame_pixels.height as f32 != height {
            return BodyHitAvailability::UnavailableWithinBounds;
        }
        let Some(alpha_index) = local_y
            .checked_mul(frame_pixels.width)
            .and_then(|row| row.checked_add(local_x))
            .and_then(|pixel| pixel.checked_mul(4))
            .and_then(|offset| offset.checked_add(3))
            .and_then(|offset| usize::try_from(offset).ok())
        else {
            return BodyHitAvailability::UnavailableWithinBounds;
        };
        return if frame_pixels.rgba.get(alpha_index).copied().unwrap_or(0) > 0 {
            BodyHitAvailability::Hit
        } else {
            BodyHitAvailability::Miss
        };
    }
    let (Some(atlas_key), Some(rect_key)) = (atlas_key, rect_key) else {
        return BodyHitAvailability::UnavailableWithinBounds;
    };
    let Some(page) = index.pages.iter().find(|page| page.key == atlas_key) else {
        return BodyHitAvailability::UnavailableWithinBounds;
    };
    let Some(rect) = page.rects.iter().find(|rect| rect.key == rect_key) else {
        return BodyHitAvailability::UnavailableWithinBounds;
    };
    let Some(pixel_page) = pixels.and_then(|pages| pages.get(atlas_key)) else {
        return BodyHitAvailability::UnavailableWithinBounds;
    };
    if pixel_page.width != page.width || pixel_page.height != page.height {
        return BodyHitAvailability::UnavailableWithinBounds;
    }
    if local_x >= rect.width || local_y >= rect.height {
        return BodyHitAvailability::Miss;
    }
    let Some(pixel_x) = rect.x.checked_add(local_x) else {
        return BodyHitAvailability::UnavailableWithinBounds;
    };
    let Some(pixel_y) = rect.y.checked_add(local_y) else {
        return BodyHitAvailability::UnavailableWithinBounds;
    };
    if pixel_x >= pixel_page.width || pixel_y >= pixel_page.height {
        return BodyHitAvailability::UnavailableWithinBounds;
    }
    let Some(alpha_index) = pixel_y
        .checked_mul(pixel_page.width)
        .and_then(|row| row.checked_add(pixel_x))
        .and_then(|pixel| pixel.checked_mul(4))
        .and_then(|offset| offset.checked_add(3))
        .and_then(|offset| usize::try_from(offset).ok())
    else {
        return BodyHitAvailability::UnavailableWithinBounds;
    };
    if pixel_page.rgba.get(alpha_index).copied().unwrap_or(0) > 0 {
        BodyHitAvailability::Hit
    } else {
        BodyHitAvailability::Miss
    }
}

fn self_hovered_at_cursor(
    payload: &Value,
    entities: &[Value],
    index: &StarterAtlasIndex,
    pixels: Option<&StarterAtlasPixels>,
) -> bool {
    let Some((cursor_x, cursor_y, cursor_grid_x, cursor_grid_y)) = cursor_hit_context(payload)
    else {
        return false;
    };
    entities.iter().any(|entity| {
        if entity.get("isSelf").and_then(Value::as_bool) != Some(true)
            || entity.get("dead").and_then(Value::as_bool) == Some(true)
        {
            return false;
        }
        let Some(object_id) = entity.get("objectId").and_then(Value::as_str) else {
            return false;
        };
        let x = entity.get("gridX").and_then(Value::as_i64);
        let y = entity.get("gridY").and_then(Value::as_i64);
        (x == Some(cursor_grid_x) && y == Some(cursor_grid_y))
            || body_visible_pixel(entity, object_id, cursor_x, cursor_y, index, pixels)
    })
}

fn cursor_hit_context(payload: &Value) -> Option<(f32, f32, i64, i64)> {
    let cursor = payload.get("_nativeHoverCursor")?;
    let cursor_x = cursor.get("x").and_then(Value::as_f64)? as f32;
    let cursor_y = cursor.get("y").and_then(Value::as_f64)? as f32;
    if !(0.0..STAGE_WIDTH).contains(&cursor_x) || !(0.0..STAGE_HEIGHT).contains(&cursor_y) {
        return None;
    }
    let (center_x, center_y) = scene_center(payload);
    let cursor_grid_x = center_x + i64::from((cursor_x / CELL_WIDTH).floor() as i32) - 10;
    let cursor_grid_y = center_y + i64::from((cursor_y / CELL_HEIGHT).floor() as i32) - 11;
    Some((cursor_x, cursor_y, cursor_grid_x, cursor_grid_y))
}

fn body_visible_pixel(
    entity: &Value,
    object_id: &str,
    cursor_x: f32,
    cursor_y: f32,
    index: &StarterAtlasIndex,
    pixels: Option<&StarterAtlasPixels>,
) -> bool {
    matches!(
        body_hit_availability(entity, object_id, cursor_x, cursor_y, index, pixels),
        BodyHitAvailability::Hit
    )
}

#[cfg(test)]
pub(crate) fn map_tile_draw_z_for_test(x: i32, y: i32, layer_z: f32) -> f32 {
    ((y.saturating_mul(1_000).saturating_add(x.saturating_mul(10))) as f32 + layer_z)
        * ENTITY_DEPTH_GAIN
}

/// Crystal redraws the hovered object and then the selected object at 30%
/// after the complete world pass, then calls `MirObject.DrawEffects`. Keep all
/// three post-world passes in non-overlapping GPU depth bands while retaining
/// normal object/layer order inside each band.
pub(crate) fn post_world_depth_bounds(payload: &Value) -> (f32, f32) {
    let viewport = crate::map_parser::MapViewport::from_gateway_payload(payload);
    let start_x = viewport.center_x.saturating_sub(viewport.draw_margin_x());
    let start_y = viewport.center_y.saturating_sub(viewport.draw_margin_y());
    let end_x = viewport.center_x.saturating_add(viewport.draw_margin_x());
    let end_y = viewport.center_y.saturating_add(viewport.draw_margin_y());

    let mut min_world_z = entity_z_base(i64::from(start_x), i64::from(start_y));
    let mut max_world_z = entity_z_base(i64::from(end_x), i64::from(end_y)) + MAP_FRONT_ORDER;
    if let Some(entities) = payload.get("entities").and_then(Value::as_array) {
        for entity in entities {
            let x = entity.get("x").and_then(Value::as_i64).unwrap_or(0);
            let y = entity.get("y").and_then(Value::as_i64).unwrap_or(0);
            let z = entity_z_base(x, y);
            min_world_z = min_world_z.min(z);
            max_world_z = max_world_z.max(z + MAP_FRONT_ORDER);
        }
    }

    (min_world_z, max_world_z)
}

fn post_world_hover_z(payload: &Value, normal_layer_z: f32) -> f32 {
    let (min_world_z, max_world_z) = post_world_depth_bounds(payload);
    max_world_z + POST_WORLD_BAND_GAP + (normal_layer_z - min_world_z)
}

fn post_world_selected_z(payload: &Value, normal_layer_z: f32) -> f32 {
    let (min_world_z, max_world_z) = post_world_depth_bounds(payload);
    let world_span = max_world_z - min_world_z;
    max_world_z
        + POST_WORLD_BAND_GAP
        + world_span
        + POST_WORLD_BAND_GAP
        + (normal_layer_z - min_world_z)
}

pub(crate) fn post_world_effect_z_from_bounds(
    (min_world_z, max_world_z): (f32, f32),
    normal_layer_z: f32,
) -> f32 {
    let world_span = max_world_z - min_world_z;
    max_world_z
        + POST_WORLD_BAND_GAP
        + world_span
        + POST_WORLD_BAND_GAP
        + world_span
        + POST_WORLD_BAND_GAP
        + (normal_layer_z - min_world_z).max(0.0)
}

#[cfg(test)]
pub(crate) fn post_world_highlight_band_ceiling((min_world_z, max_world_z): (f32, f32)) -> f32 {
    let world_span = max_world_z - min_world_z;
    max_world_z + POST_WORLD_BAND_GAP + world_span + POST_WORLD_BAND_GAP + world_span
}

fn post_world_effect_z(payload: &Value, object_x: i64, object_y: i64) -> f32 {
    post_world_effect_z_from_bounds(
        post_world_depth_bounds(payload),
        entity_z_base(object_x, object_y),
    )
}

fn scarecrow_die_effect_frame(
    body_library: &str,
    action: AnimationAction,
    draw_frame: i64,
    direction: &str,
) -> Option<i64> {
    if action != AnimationAction::Die || normalized_library_key(body_library) != "Monster/005" {
        return None;
    }
    const DIE_START: i64 = 144;
    const DIE_DIRECTION_STRIDE: i64 = 10;
    const DIE_FRAME_COUNT: i64 = 10;
    const EFFECT_START: i64 = 224;
    let action_start = DIE_START
        .saturating_add(i64::from(direction_index(direction)).saturating_mul(DIE_DIRECTION_STRIDE));
    let phase = draw_frame.saturating_sub(action_start);
    (0..DIE_FRAME_COUNT)
        .contains(&phase)
        .then_some(EFFECT_START.saturating_add(phase))
}

/// Minimal in-memory atlas geometry used only to prove VIS-01 routing. The
/// rectangle coordinates are intentionally synthetic; source frame semantics
/// stay locked by the production animation descriptors and explicit path
/// assertions, while Candidate packaging still verifies the real manifest.
#[cfg(test)]
pub(crate) fn scarecrow_routing_atlas_manifest_fixture() -> Value {
    routing_atlas_manifest_fixture(&[
        "/original-ui/Monster/003/16.png",
        "/original-ui/Monster/005/40.png",
        "/original-ui/Monster/005/184.png",
        "/original-ui/Monster/005/187.png",
        "/original-ui/Monster/005/224.png",
        "/original-ui/Monster/005/227.png",
    ])
}

#[cfg(test)]
pub(crate) fn routing_atlas_manifest_fixture(frame_paths: &[&str]) -> Value {
    let rects = frame_paths
        .iter()
        .enumerate()
        .map(|(index, frame_path)| {
            json!({
                "key": format!("{frame_path}|48x64"),
                "x": 1 + index * 50,
                "y": 2,
                "width": 48,
                "height": 64,
                "offsetX": 0,
                "offsetY": 0
            })
        })
        .collect::<Vec<_>>();
    json!({
        "atlases": [{
            "key": "vis01-routing-fixture",
            "width": 2048,
            "height": 128,
            "imageUrl": "/test-only/vis01-scarecrow-routing.png",
            "rects": rects
        }]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_frame_is_deterministic_and_kind_aware() {
        assert_eq!(
            starter_frame("player", "1001", "down", None, None),
            starter_frame("player", "1001", "down", None, None)
        );
        assert!(starter_frame("player", "x", "down", None, None)
            .0
            .contains("AArmour"));
        assert!(starter_frame("npc", "x", "down", None, None)
            .0
            .contains("NPC"));
        assert!(starter_frame("monster", "x", "down", None, None)
            .0
            .contains("Monster"));
    }

    #[test]
    fn starter_frame_uses_direction_stride_to_pick_frame() {
        let (_, up) = starter_frame("monster", "1", "up", Some(6), Some(0));
        let (_, down) = starter_frame("monster", "1", "down", Some(6), Some(0));
        let (_, right) = starter_frame("monster", "1", "right", Some(6), Some(0));
        assert_eq!(up, 0);
        assert_eq!(down, 24);
        assert_eq!(right, 12);
    }

    #[test]
    fn manifest_parser_routes_each_rect_to_its_declared_page() {
        let manifest = json!({
            "atlases": [{
                "key": "starter",
                "width": 32,
                "height": 32,
                "imageUrl": "/atlas.png",
                "pages": [
                    { "width": 32, "height": 32, "imageUrl": "/atlas.png" },
                    { "width": 64, "height": 64, "imageUrl": "/atlas-p1.png" }
                ],
                "rects": [
                    {
                        "key": "/original-ui/Monster/003/0.png|8x9",
                        "x": 1, "y": 2, "width": 8, "height": 9,
                        "offsetX": 3, "offsetY": -4
                    },
                    {
                        "key": "/original-ui/Monster/004/0.png|10x11",
                        "x": 5, "y": 6, "width": 10, "height": 11,
                        "pageIndex": 1
                    }
                ]
            }]
        });

        let index = parse_starter_atlas_manifest(&manifest).expect("multi-page manifest");
        assert_eq!(index.pages.len(), 2);
        assert_eq!(index.pages[0].key, "starter");
        assert_eq!(index.pages[1].key, "starter:p1");
        assert_eq!(index.pages[1].image_url, "/atlas-p1.png");
        assert_eq!(index.rect_by_path["/original-ui/Monster/003/0.png"], (0, 0));
        assert_eq!(index.rect_by_path["/original-ui/Monster/004/0.png"], (1, 0));
        assert_eq!(index.pages[0].rects[0].offset_x, Some(3));
        assert_eq!(index.pages[0].rects[0].offset_y, Some(-4));
    }

    #[test]
    fn original_frame_meta_maps_web_xy_to_crystal_draw_offsets() {
        let payload = json!({
            "frames": [
                {
                    "index": 0,
                    "width": 64,
                    "height": 76,
                    "x": 6,
                    "y": -48,
                    "path": "/original-ui/CArmour/01/0.png"
                },
                {
                    "index": 1,
                    "width": 60,
                    "height": 80,
                    "x": 9,
                    "y": -49,
                    "path": "/original-ui/Other/1.png"
                },
                {
                    "index": 2,
                    "width": 60,
                    "height": 80,
                    "x": 9,
                    "y": -49
                }
            ]
        });
        let frames = parse_original_frame_geometry(&payload, "CArmour/01")
            .expect("valid Web sprite metadata");
        assert_eq!(
            frames.get(&0),
            Some(&OriginalFrameGeometry {
                width: 64,
                height: 76,
                offset_x: 6,
                offset_y: -48,
            })
        );
        assert!(!frames.contains_key(&1), "foreign frame paths fail closed");
        assert!(!frames.contains_key(&2), "missing frame paths fail closed");
    }

    #[test]
    fn packed_manifest_and_web_meta_share_geometry_semantics() {
        let index = starter_atlas_index().expect("starter atlas index");
        let (page_index, rect_index) = index.rect_by_path["/original-ui/CArmour/00/0.png"];
        let rect = &index.pages[page_index].rects[rect_index];
        let geometry = original_frame_geometry("/original-ui/CArmour/00", 0)
            .expect("base armour Web metadata");
        assert_eq!(rect.width, geometry.width);
        assert_eq!(rect.height, geometry.height);
        assert_eq!(rect.offset_x, Some(geometry.offset_x));
        assert_eq!(rect.offset_y, Some(geometry.offset_y));
    }

    #[test]
    fn native_frame_geometry_exposes_crystal_npc_frame_zero_anchor_data() {
        assert_eq!(
            native_frame_geometry("/original-ui/NPC/05", 0),
            Some(OriginalFrameGeometry {
                width: 56,
                height: 72,
                offset_x: 13,
                offset_y: -52,
            })
        );
    }

    #[test]
    fn unpacked_player_library_uses_exact_meta_for_actions_directions_and_gender() {
        let index = starter_atlas_index().expect("starter atlas index");
        assert!(!index
            .rect_by_path
            .contains_key("/original-ui/CArmour/01/0.png"));

        let action_ranges = [
            (0_i64, 4_i64),
            (32, 6),
            (80, 6),
            (96, 8),
            (136, 6),
            (160, 8),
            (184, 6),
            (232, 8),
            (296, 6),
            (344, 2),
            (360, 3),
            (384, 4),
            (416, 6),
        ];
        for gender_offset in [0_i64, 808_i64] {
            for (start, count) in action_ranges {
                // The kept same-origin CArmour/01 export ends at frame 1263;
                // the final female DashAttack directions remain a remote-asset
                // gate and must not be pretended into existence here.
                if gender_offset == 808 && start == 416 {
                    continue;
                }
                for direction in 0_i64..8 {
                    for phase in [0_i64, count - 1] {
                        let frame = gender_offset + start + direction * count + phase;
                        let expected = original_frame_geometry("CArmour/01", frame)
                            .unwrap_or_else(|| panic!("CArmour/01 frame {frame} metadata"));
                        let mut used_rects = HashMap::new();
                        let layer = build_entity_layer(
                            index,
                            &mut used_rects,
                            format!("player:body:{frame}"),
                            "/original-ui/CArmour/01",
                            frame,
                            100.0,
                            200.0,
                            5.0,
                        )
                        .unwrap_or_else(|| panic!("CArmour/01 frame {frame} layer"));
                        assert_eq!(layer["width"], json!(expected.width as f32));
                        assert_eq!(layer["height"], json!(expected.height as f32));
                        assert_eq!(layer["left"], json!(100.0 + expected.offset_x as f32));
                        assert_eq!(layer["top"], json!(200.0 + expected.offset_y as f32));
                        assert!(layer.get("atlasKey").is_none());
                        assert!(layer.get("atlasRectKey").is_none());
                    }
                }
            }
        }
    }

    #[test]
    fn unpacked_player_frame_keeps_pixel_hit_and_atomic_highlight() {
        let index = starter_atlas_index().expect("starter atlas index");
        let layer = build_entity_layer(
            index,
            &mut HashMap::new(),
            "player:body".to_owned(),
            "/original-ui/CArmour/01",
            0,
            100.0,
            200.0,
            5.0,
        )
        .expect("unpacked player body");
        assert!(layer.get("atlasKey").is_none());
        assert!(layer.get("atlasRectKey").is_none());

        let pixels = original_frame_pixels("/original-ui/CArmour/01/0.png")
            .expect("unpacked player frame pixels");
        let opaque = pixels
            .rgba
            .chunks_exact(4)
            .position(|pixel| pixel[3] > 0)
            .expect("opaque player pixel");
        let transparent = pixels
            .rgba
            .chunks_exact(4)
            .position(|pixel| pixel[3] == 0)
            .expect("transparent player pixel");
        let left = layer["left"].as_f64().expect("layer left") as f32;
        let top = layer["top"].as_f64().expect("layer top") as f32;
        let cursor_for = |pixel: usize| {
            (
                left + (pixel as u32 % pixels.width) as f32 + 0.5,
                top + (pixel as u32 / pixels.width) as f32 + 0.5,
            )
        };
        let entity = json!({
            "_nativeActorLayerCount": 1,
            "layers": [layer.clone()]
        });
        let (opaque_x, opaque_y) = cursor_for(opaque);
        assert!(body_visible_pixel(
            &entity, "player", opaque_x, opaque_y, index, None
        ));
        let (transparent_x, transparent_y) = cursor_for(transparent);
        assert!(!body_visible_pixel(
            &entity,
            "player",
            transparent_x,
            transparent_y,
            index,
            None,
        ));

        let mut broken_layer = layer.clone();
        broken_layer["atlasKey"] = json!("half-binding");
        let mut broken_layers = vec![broken_layer];
        append_actor_highlight(
            &mut broken_layers,
            1,
            "player",
            HighlightBand::Selected,
            &json!({"sceneView": {"center": {"x": 257, "y": 594}}}),
        );
        assert_eq!(
            broken_layers.len(),
            1,
            "half-atlas bindings fail the complete highlight closed"
        );

        let mut layers = vec![layer];
        append_actor_highlight(
            &mut layers,
            1,
            "player",
            HighlightBand::Selected,
            &json!({"sceneView": {"center": {"x": 257, "y": 594}}}),
        );
        assert_eq!(layers.len(), 2, "standalone composite keeps DrawBlend pass");
        assert_eq!(layers[1]["key"], json!("player:target-highlight:body"));
        assert_eq!(layers[1]["path"], json!("/original-ui/CArmour/01/0.png"));
        assert_eq!(layers[1]["opacity"], json!(TARGET_HIGHLIGHT_OPACITY));
        assert!(layers[1].get("atlasKey").is_none());
        assert!(layers[1].get("atlasRectKey").is_none());
    }

    #[test]
    fn missing_atlas_png_or_meta_never_invents_player_geometry() {
        let index = starter_atlas_index().expect("starter atlas index");
        assert!(build_entity_layer(
            index,
            &mut HashMap::new(),
            "player:body".to_owned(),
            "/original-ui/CArmour/not-exported",
            0,
            0.0,
            0.0,
            0.0,
        )
        .is_none());
    }

    #[test]
    fn entity_render_state_uses_manifest_geometry_and_viewport_coordinates() {
        let payload = json!({
            "sceneView": { "center": { "x": 9, "y": 7 } },
            "entities": [{
                "objectId": 2001,
                "kind": "monster",
                "x": 9,
                "y": 7,
                "direction": "down",
                "sprite": {
                    "bodyLibrary": "Monster/003",
                    "directionStride": 6,
                    "frameBaseOffset": 0
                }
            }]
        });
        let state = build_entity_render_state(&payload).expect("starter manifest");
        let layer = &state["entities"][0]["layers"][0];
        assert_eq!(layer["left"], json!(484.0));
        assert_eq!(layer["top"], json!(344.0));
        assert_eq!(layer["z"], json!(70_905.0));
        assert_eq!(layer["key"], json!("2001:body"));
        assert_eq!(layer["atlasKey"], json!("starter-bichon-base:p6"));
        assert!(layer["atlasRectKey"]
            .as_str()
            .is_some_and(|key| key.contains("/Monster/003/24.png|")));

        let rect_key = layer["atlasRectKey"].as_str().expect("rect key");
        let rect = state["atlases"][0]["rects"]
            .as_array()
            .and_then(|rects| rects.iter().find(|rect| rect["key"] == rect_key))
            .expect("used rect geometry");
        assert_ne!(rect["x"], json!(0));
        assert_eq!(rect["width"], json!(48));
        assert_eq!(rect["height"], json!(29));
    }

    #[test]
    fn entity_render_state_preserves_native_motion_window_for_runtime_interpolation() {
        let payload = json!({
            "sceneView": { "center": { "x": 9, "y": 7 } },
            "entities": [{
                "objectId": 2001,
                "kind": "monster",
                "x": 9,
                "y": 7,
                "direction": "down",
                "motionFromX": 8.0,
                "motionFromY": 7.0,
                "motionToX": 9.0,
                "motionToY": 7.0,
                "motionStartedMs": 1700000000100_u64,
                "motionDurationMs": 600,
                "sprite": {
                    "bodyLibrary": "Monster/003",
                    "directionStride": 6,
                    "frameBaseOffset": 0
                }
            }]
        });

        let state = build_entity_render_state(&payload).expect("starter manifest");
        let entity = &state["entities"][0];
        assert_eq!(entity["motionFromX"], json!(8.0));
        assert_eq!(entity["motionFromY"], json!(7.0));
        assert_eq!(entity["motionToX"], json!(9.0));
        assert_eq!(entity["motionToY"], json!(7.0));
        assert_eq!(entity["motionStartedMs"], json!(1_700_000_000_100_u64));
        assert_eq!(entity["motionDurationMs"], json!(600));
    }

    #[test]
    fn animation_frame_override_selects_current_draw_frame() {
        let payload = json!({
            "sceneView": { "center": { "x": 9, "y": 7 } },
            "entities": [{
                "objectId": 2001,
                "kind": "monster",
                "x": 9,
                "y": 7,
                "direction": "down",
                "sprite": {
                    "bodyLibrary": "Monster/003",
                    "directionStride": 6,
                    "frameBaseOffset": 0
                }
            }]
        });
        let state = build_entity_render_state_with_frames(
            &payload,
            &HashMap::from([("2001".to_owned(), 25)]),
        )
        .expect("animated render state");
        assert!(state["entities"][0]["layers"][0]["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Monster/003/25.png")));
    }

    #[test]
    fn scarecrow_die_adds_source_frame_224_plus_phase_as_additive_layer() {
        let manifest = scarecrow_routing_atlas_manifest_fixture();
        let payload = json!({
            "sceneView": {
                "center": { "x": 9, "y": 7 },
                "width": 19,
                "height": 15
            },
            "entities": [
                {
                    "objectId": 2005,
                    "kind": "monster",
                    "x": 9,
                    "y": 7,
                    "direction": "down",
                    "sprite": {
                        "bodyLibrary": "Monster/005",
                        "directionStride": 10,
                        "frameBaseOffset": 0
                    }
                },
                {
                    "objectId": 2003,
                    "kind": "monster",
                    "x": 9,
                    "y": 13,
                    "direction": "down",
                    "sprite": {
                        "bodyLibrary": "Monster/003",
                        "directionStride": 4,
                        "frameBaseOffset": 0
                    }
                }
            ]
        });
        // Monster/005 Die starts at 144 with stride 10. Down is direction 4,
        // so draw frame 187 is phase 3 and Crystal's effect frame is 224 + 3.
        let dying = build_entity_render_state_with_manifest_for_test(
            &payload,
            &HashMap::from([("2005".to_owned(), (187, AnimationAction::Die))]),
            true,
            &manifest,
        )
        .expect("Scarecrow dying render state");
        let layers = dying["entities"][0]["layers"]
            .as_array()
            .expect("Scarecrow layers");
        let effect = layers
            .iter()
            .find(|layer| layer["key"] == json!("2005:scarecrow-die-effect"))
            .expect("source additive death layer");
        assert!(effect["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("/Monster/005/227.png")));
        assert_eq!(effect["additive"], json!(true));
        assert!(effect["atlasRectKey"].as_str().is_some());
        let effect_z = effect["z"].as_f64().expect("post-world z");
        let deeper_body_z = dying["entities"][1]["layers"]
            .as_array()
            .expect("deeper object layers")
            .iter()
            .find(|layer| layer["key"] == json!("2003:body"))
            .and_then(|layer| layer["z"].as_f64())
            .expect("deeper object body z");
        let viewport = crate::map_parser::MapViewport::from_gateway_payload(&payload);
        let deepest_x = viewport.center_x + viewport.draw_margin_x();
        let deepest_y = viewport.center_y + viewport.draw_margin_y();
        let empty_cell = crate::map_parser::MapCell {
            back_index: -1,
            back_image: 0,
            middle_index: -1,
            middle_image: 0,
            front_index: -1,
            front_image: 0,
            front_animation_frame: 0,
            front_animation_tick: 0,
            middle_animation_frame: 0,
            middle_animation_tick: 0,
            tile_animation_image: 0,
            tile_animation_offset: 0,
            tile_animation_frames: 0,
            light: 0,
        };
        let map_width = u16::try_from(deepest_x + 1).expect("test map width");
        let map_height = u16::try_from(deepest_y + 1).expect("test map height");
        let mut map = crate::map_parser::ParsedMap {
            width: map_width,
            height: map_height,
            cells: vec![empty_cell; usize::from(map_width) * usize::from(map_height)],
        };
        let deepest_index = deepest_x as usize * usize::from(map_height) + deepest_y as usize;
        map.cells[deepest_index].front_index = 2;
        map.cells[deepest_index].front_image = 1;
        let deepest_margin_front = crate::map_parser::resolve_map_tile_draws(&map)
            .into_iter()
            .find(|draw| {
                draw.layer == crate::map_parser::TileLayer::Front
                    && viewport.retains_cell(draw.x, draw.y)
            })
            .expect("actual map parser retains the deepest guard-band front tile");
        let deepest_visible_front_z = f64::from(
            ((deepest_margin_front.y * 1_000 + deepest_margin_front.x * 10) as f32
                + deepest_margin_front.z)
                * ENTITY_DEPTH_GAIN,
        );
        assert!(effect_z > deeper_body_z);
        assert!(effect_z > deepest_visible_front_z);

        let standing = build_entity_render_state_with_manifest_for_test(
            &payload,
            &HashMap::new(),
            true,
            &manifest,
        )
        .expect("Scarecrow standing state");
        assert!(standing["entities"][0]["layers"]
            .as_array()
            .expect("standing layers")
            .iter()
            .all(|layer| layer["additive"] != json!(true)));

        let effects_disabled = build_entity_render_state_with_manifest_for_test(
            &payload,
            &HashMap::from([("2005".to_owned(), (187, AnimationAction::Die))]),
            false,
            &manifest,
        )
        .expect("effects disabled state");
        assert!(effects_disabled["entities"][0]["layers"]
            .as_array()
            .expect("disabled layers")
            .iter()
            .all(|layer| layer["additive"] != json!(true)));
    }

    #[test]
    fn scarecrow_die_effect_phase_covers_all_directions_and_rejects_boundaries() {
        for (direction_index, direction) in [
            "up",
            "upright",
            "right",
            "downright",
            "down",
            "downleft",
            "left",
            "upleft",
        ]
        .into_iter()
        .enumerate()
        {
            let start = 144 + i64::try_from(direction_index).unwrap() * 10;
            assert_eq!(
                scarecrow_die_effect_frame("Monster/005", AnimationAction::Die, start, direction),
                Some(224)
            );
            assert_eq!(
                scarecrow_die_effect_frame(
                    "Monster/005",
                    AnimationAction::Die,
                    start + 9,
                    direction
                ),
                Some(233)
            );
            assert_eq!(
                scarecrow_die_effect_frame(
                    "Monster/005",
                    AnimationAction::Die,
                    start - 1,
                    direction
                ),
                None
            );
            assert_eq!(
                scarecrow_die_effect_frame(
                    "Monster/005",
                    AnimationAction::Die,
                    start + 10,
                    direction
                ),
                None
            );
        }
        assert_eq!(
            scarecrow_die_effect_frame("Monster/003", AnimationAction::Die, 144, "up"),
            None
        );
        assert_eq!(
            scarecrow_die_effect_frame("Monster/005", AnimationAction::Dead, 144, "up"),
            None
        );
    }

    #[test]
    fn player_composite_uses_body_hair_weapon_offsets_and_depth_order() {
        let payload = json!({
            "sceneView": { "center": { "x": 9, "y": 7 } },
            "entities": [{
                "objectId": 1001,
                "kind": "selfPlayer",
                "x": 9,
                "y": 7,
                "direction": "up",
                "sprite": {
                    "bodyLibrary": "CArmour/00",
                    "hairLibrary": "CHair/00",
                    "weaponLibrary": "CWeapon/00",
                    "frameBaseOffset": 808,
                    "weaponFrameOffset": 416,
                    "directionStride": 4
                }
            }]
        });
        let state = build_entity_render_state_with_frames(
            &payload,
            &HashMap::from([("1001".to_owned(), 33)]),
        )
        .expect("composite render state");
        let layers = state["entities"][0]["layers"]
            .as_array()
            .expect("composite layers");
        let find = |suffix: &str| {
            layers
                .iter()
                .find(|layer| {
                    layer["key"]
                        .as_str()
                        .is_some_and(|key| key.ends_with(suffix))
                })
                .expect("named layer")
        };
        assert!(find(":body")["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("/CArmour/00/841.png")));
        assert!(find(":hair")["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("/CHair/00/841.png")));
        assert!(find(":weapon-primary")["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("/CWeapon/00/449.png")));
        assert!(
            find(":weapon-primary")["z"].as_f64().unwrap() < find(":body")["z"].as_f64().unwrap()
        );
        assert!(find(":hair")["z"].as_f64().unwrap() > find(":body")["z"].as_f64().unwrap());
    }

    #[test]
    fn archer_walk_and_range_two_use_alt_layers_then_standing_returns_to_common_body() {
        let payload = json!({
            "sceneView": { "center": { "x": 9, "y": 7 } },
            "entities": [{
                "objectId": 1002,
                "kind": "player",
                "classKey": "archer",
                "genderKey": "male",
                "x": 9,
                "y": 7,
                "direction": "down",
                "sprite": {
                    "bodyLibrary": "CArmour/00",
                    "hairLibrary": "CHair/00",
                    "weaponLibrary": "CWeapon/00",
                    "altBodyLibrary": "ARArmour/00",
                    "altHairLibrary": "ARHair/00",
                    "altWeaponLibrary": "ARWeapon/00 S",
                    "altFrameBaseOffset": 0,
                    "altWeaponFrameOffset": 0,
                    "frameBaseOffset": 0,
                    "weaponFrameOffset": 0,
                    "directionStride": 4
                }
            }]
        });
        let walking = build_entity_render_state_with_poses(
            &payload,
            &HashMap::from([("1002".to_owned(), (24, AnimationAction::Walking))]),
        )
        .expect("archer walking render state");
        let walking_layers = walking["entities"][0]["layers"]
            .as_array()
            .expect("archer layers");
        assert!(walking_layers.iter().any(|layer| {
            layer["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/ARArmour/00/24.png"))
        }));
        assert!(walking_layers.iter().any(|layer| {
            layer["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/ARHair/00/24.png"))
        }));
        assert_eq!(
            walking_layers
                .iter()
                .filter(|layer| layer["key"]
                    .as_str()
                    .is_some_and(|key| key.contains("weapon")))
                .count(),
            1
        );
        assert!(walking_layers.iter().any(|layer| {
            layer["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/ARWeapon/00 S/24.png"))
        }));

        let archer = &payload["entities"][0];
        let range_two = resolved_native_sprite(archer, AnimationAction::AttackRange2);
        assert_eq!(range_two.body_library, "/original-ui/ARArmour/00");
        assert_eq!(
            range_two.hair_library.as_deref(),
            Some("/original-ui/ARHair/00")
        );
        assert_eq!(
            range_two.weapon_library.as_deref(),
            Some("/original-ui/ARWeapon/00 S")
        );
        assert!(range_two.weapon_library_secondary.is_none());

        let standing = build_entity_render_state_with_poses(
            &payload,
            &HashMap::from([("1002".to_owned(), (0, AnimationAction::Standing))]),
        )
        .expect("archer standing render state");
        assert!(standing["entities"][0]["layers"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|layer| layer["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/CArmour/00/0.png"))));
    }

    #[test]
    fn harvest_forces_crystal_cweapon_one_for_untransformed_player() {
        let resolved = resolved_native_sprite(
            &json!({
                "kind": "player",
                "genderKey": "female",
                "weapon": -1,
                "transformType": -1,
                "sprite": {
                    "bodyLibrary": "CArmour/00",
                    "frameBaseOffset": 808,
                    "weaponFrameOffset": null
                }
            }),
            AnimationAction::Harvest,
        );
        assert_eq!(
            resolved.weapon_library.as_deref(),
            Some("/original-ui/CWeapon/01")
        );
        assert_eq!(resolved.weapon_frame_offset, Some(416));

        let transformed_mount = resolved_native_sprite(
            &json!({
                "kind": "player",
                "transformType": 4,
                "ridingMount": true,
                "mountType": 7,
                "sprite": {
                    "bodyLibrary": "TransformRide2/04",
                    "mountLibrary": "Mount/07",
                    "frameBaseOffset": 0,
                    "mountFrameOffset": 0
                }
            }),
            AnimationAction::Standing,
        );
        assert_eq!(
            transformed_mount.body_library,
            "/original-ui/TransformRide2/04"
        );
        assert_eq!(transformed_mount.body_base_offset, -416);
    }

    #[test]
    fn assassin_melee_uses_alt_body_hair_and_dual_directional_weapons() {
        let payload = json!({
            "sceneView": { "center": { "x": 9, "y": 7 } },
            "entities": [{
                "objectId": 1003,
                "kind": "player",
                "classKey": "assassin",
                "genderKey": "male",
                "x": 9,
                "y": 7,
                "direction": "down",
                "sprite": {
                    "bodyLibrary": "CArmour/00",
                    "hairLibrary": "CHair/00",
                    "weaponLibrary": "CWeapon/00",
                    "altBodyLibrary": "AArmour/00",
                    "altHairLibrary": "AHair/00",
                    "altWeaponLibrary": "AWeapon/00 R",
                    "altWeaponLibrarySecondary": "AWeapon/00 L",
                    "altFrameBaseOffset": 0,
                    "altWeaponFrameOffset": 0,
                    "frameBaseOffset": 0,
                    "weaponFrameOffset": 0,
                    "directionStride": 4
                }
            }]
        });
        let state = build_entity_render_state_with_poses(
            &payload,
            &HashMap::from([("1003".to_owned(), (160, AnimationAction::Attack1))]),
        )
        .expect("assassin attack render state");
        let layers = state["entities"][0]["layers"]
            .as_array()
            .expect("assassin layers");
        assert!(layers.iter().any(|layer| {
            layer["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/AArmour/00/160.png"))
        }));
        assert!(layers.iter().any(|layer| {
            layer["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/AHair/00/160.png"))
        }));
        let primary = layers
            .iter()
            .find(|layer| layer["key"] == "1003:weapon-primary")
            .expect("primary assassin weapon");
        let secondary = layers
            .iter()
            .find(|layer| layer["key"] == "1003:weapon-secondary")
            .expect("secondary assassin weapon");
        assert!(primary["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("/AWeapon/00 R/160.png")));
        assert!(secondary["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("/AWeapon/00 L/160.png")));
        assert!(primary["z"].as_f64().unwrap() > secondary["z"].as_f64().unwrap());

        let entity = &payload["entities"][0];
        assert_eq!(
            resolved_native_sprite(entity, AnimationAction::Die).body_library,
            "/original-ui/AArmour/00"
        );
        assert_eq!(
            resolved_native_sprite(entity, AnimationAction::DashAttack).body_library,
            "/original-ui/AArmour/00"
        );
        assert_eq!(
            resolved_native_sprite(entity, AnimationAction::Dead).body_library,
            "/original-ui/CArmour/00"
        );
        assert_eq!(
            resolved_native_sprite(entity, AnimationAction::Revive).body_library,
            "/original-ui/CArmour/00"
        );
    }

    #[test]
    fn mounted_player_adds_mount_and_suppresses_weapon_layers() {
        let payload = json!({
            "sceneView": { "center": { "x": 9, "y": 7 } },
            "entities": [{
                "objectId": 1001,
                "kind": "selfPlayer",
                "x": 9,
                "y": 7,
                "direction": "down",
                "sprite": {
                    "bodyLibrary": "CArmour/00",
                    "hairLibrary": "CHair/00",
                    "weaponLibrary": "CWeapon/00",
                    "mountLibrary": "Mount/00",
                    "frameBaseOffset": 0,
                    "weaponFrameOffset": 0,
                    "mountFrameOffset": 0,
                    "directionStride": 4
                }
            }]
        });
        let state = build_entity_render_state_with_frames(
            &payload,
            &HashMap::from([("1001".to_owned(), 448)]),
        )
        .expect("mounted render state");
        let layers = state["entities"][0]["layers"]
            .as_array()
            .expect("mounted layers");
        assert!(layers.iter().any(|layer| {
            layer["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/Mount/00/32.png"))
        }));
        assert!(!layers.iter().any(|layer| {
            layer["key"]
                .as_str()
                .is_some_and(|key| key.contains("weapon"))
        }));

        let dead_state = build_entity_render_state_with_poses(
            &payload,
            &HashMap::from([("1001".to_owned(), (387, AnimationAction::Dead))]),
        )
        .expect("mounted player death render state");
        let dead_layers = dead_state["entities"][0]["layers"]
            .as_array()
            .expect("mounted player death layers");
        assert!(dead_layers.iter().any(|layer| {
            layer["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/CArmour/00/387.png"))
        }));
        assert!(!dead_layers.iter().any(|layer| {
            layer["key"]
                .as_str()
                .is_some_and(|key| key.ends_with(":mount"))
        }));
    }

    #[test]
    fn hidden_is_half_opacity_but_dead_is_not_implicitly_faded() {
        let payload = json!({
            "sceneView": { "center": { "x": 9, "y": 7 } },
            "entities": [
                {
                    "objectId": 1001,
                    "kind": "player",
                    "x": 9,
                    "y": 7,
                    "direction": "down",
                    "hidden": true,
                    "dead": false,
                    "sprite": {"bodyLibrary": "CArmour/00", "frameBaseOffset": 0}
                },
                {
                    "objectId": 1002,
                    "kind": "monster",
                    "x": 10,
                    "y": 7,
                    "direction": "down",
                    "hidden": false,
                    "dead": true,
                    "sprite": {"bodyLibrary": "Monster/004", "frameBaseOffset": 0}
                }
            ]
        });
        let state = build_entity_render_state_with_frames(
            &payload,
            &HashMap::from([("1001".to_owned(), 0), ("1002".to_owned(), 153)]),
        )
        .expect("entity render state");
        let hidden_layers = state["entities"][0]["layers"]
            .as_array()
            .expect("hidden layers");
        assert!(hidden_layers
            .iter()
            .all(|layer| layer["opacity"] == json!(0.5)));
        let dead_layers = state["entities"][1]["layers"]
            .as_array()
            .expect("dead layers");
        assert!(dead_layers
            .iter()
            .all(|layer| layer.get("opacity").is_none()));
    }

    fn hover_fixture_manifest() -> Value {
        json!({
            "atlases": [{
                "key": "hover-fixture",
                "width": 8,
                "height": 8,
                "imageUrl": "/test-only/hover.png",
                "rects": [{
                    "key": "/original-ui/Monster/001/0.png|4x4",
                    "x": 2,
                    "y": 1,
                    "width": 4,
                    "height": 4,
                    "offsetX": -47,
                    "offsetY": 0
                }, {
                    "key": "/original-ui/NPC/00/0.png|4x4",
                    "x": 2,
                    "y": 1,
                    "width": 4,
                    "height": 4,
                    "offsetX": -47,
                    "offsetY": 0
                }]
            }]
        })
    }

    fn hover_fixture_pixels(
        alpha_local_x: u32,
        alpha_local_y: u32,
    ) -> HashMap<String, (u32, u32, Vec<u8>)> {
        let mut rgba = vec![0; 8 * 8 * 4];
        let pixel_x = 2 + alpha_local_x;
        let pixel_y = 1 + alpha_local_y;
        rgba[((pixel_y * 8 + pixel_x) * 4 + 3) as usize] = 255;
        HashMap::from([("hover-fixture".to_owned(), (8, 8, rgba))])
    }

    fn hover_fixture_entity(object_id: u32, kind: &str, x: i64, dead: bool) -> Value {
        let body_library = if kind == "npc" {
            "NPC/00"
        } else {
            "Monster/001"
        };
        json!({
            "objectId": object_id,
            "kind": kind,
            "x": x,
            "y": 10,
            "direction": "up",
            "dead": dead,
            "sprite": {"bodyLibrary": body_library, "frameBaseOffset": 0}
        })
    }

    fn has_layer(state: &Value, key: &str) -> bool {
        state["entities"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|entity| entity["layers"].as_array().into_iter().flatten())
            .any(|layer| layer["key"].as_str() == Some(key))
    }

    #[test]
    fn hover_uses_body_alpha_and_same_tile_shortcut_but_fails_closed_without_pixels() {
        let manifest = hover_fixture_manifest();
        let pixels = hover_fixture_pixels(1, 1);
        let poses = HashMap::from([("2001".to_owned(), (0, AnimationAction::Standing))]);
        let mut payload = json!({
            "sceneView": {"center": {"x": 10, "y": 10}, "width": 19, "height": 15},
            "_nativeHighlightTarget": true,
            // The frame is offset left into the centre tile. This cursor hits
            // its one opaque body pixel while not sharing the actor's tile.
            "_nativeHoverCursor": {"x": 482.2, "y": 353.2},
            "entities": [hover_fixture_entity(2001, "monster", 11, false)]
        });
        let opaque = build_entity_render_state_with_manifest_and_pixels_for_test(
            &payload, &poses, true, &manifest, &pixels,
        )
        .expect("opaque hover state");
        assert!(has_layer(&opaque, "2001:hover-highlight:body"));
        assert_eq!(opaque["hoveredObjectId"], json!("2001"));

        payload["_nativeHoverCursor"] = json!({"x": 481.2, "y": 352.2});
        let transparent = build_entity_render_state_with_manifest_and_pixels_for_test(
            &payload, &poses, true, &manifest, &pixels,
        )
        .expect("transparent hover state");
        assert!(!has_layer(&transparent, "2001:hover-highlight:body"));
        assert!(transparent["hoveredObjectId"].is_null());

        payload["_nativeHoverCursor"] = json!({"x": 482.2, "y": 353.2});
        let missing_pixels = build_entity_render_state_with_manifest_and_pixels_for_test(
            &payload,
            &poses,
            true,
            &manifest,
            &HashMap::new(),
        )
        .expect("missing pixel cache state");
        assert!(!has_layer(&missing_pixels, "2001:hover-highlight:body"));
        assert!(missing_pixels["hoveredObjectId"].is_null());

        payload["_nativeHoverCursor"] = json!({"x": 529.0, "y": 353.0});
        let same_tile = build_entity_render_state_with_manifest_and_pixels_for_test(
            &payload,
            &poses,
            true,
            &manifest,
            &HashMap::new(),
        )
        .expect("same-tile shortcut state");
        assert!(has_layer(&same_tile, "2001:hover-highlight:body"));
        assert_eq!(same_tile["hoveredObjectId"], json!("2001"));
    }

    #[test]
    fn hover_scan_allows_npc_excludes_self_and_dead_and_uses_reverse_cell_order() {
        let manifest = hover_fixture_manifest();
        let pixels = hover_fixture_pixels(1, 1);
        let poses = HashMap::from([
            ("1000".to_owned(), (0, AnimationAction::Standing)),
            ("2001".to_owned(), (0, AnimationAction::Standing)),
            ("3001".to_owned(), (0, AnimationAction::Standing)),
            ("3002".to_owned(), (0, AnimationAction::Standing)),
        ]);
        let payload = json!({
            "sceneView": {"center": {"x": 10, "y": 10}},
            "_nativeHoverCursor": {"x": 482.2, "y": 353.2},
            "entities": [
                hover_fixture_entity(1000, "selfPlayer", 11, false),
                hover_fixture_entity(2001, "monster", 11, true),
                hover_fixture_entity(3001, "npc", 11, false),
                hover_fixture_entity(3002, "npc", 11, false)
            ]
        });
        let state = build_entity_render_state_with_manifest_and_pixels_for_test(
            &payload, &poses, true, &manifest, &pixels,
        )
        .expect("NPC hover state");
        assert!(has_layer(&state, "3002:hover-highlight:body"));
        assert!(!has_layer(&state, "3001:hover-highlight:body"));
        assert!(!has_layer(&state, "2001:hover-highlight:body"));
        assert!(!has_layer(&state, "1000:hover-highlight:body"));
        assert_eq!(state["hoveredObjectId"], json!("3002"));
        assert_eq!(state["selfHovered"], json!(true));
    }

    #[test]
    fn npc_hover_falls_back_to_body_bounds_when_pixels_are_unavailable() {
        let manifest = hover_fixture_manifest();
        let poses = HashMap::from([
            ("3001".to_owned(), (0, AnimationAction::Standing)),
            ("2001".to_owned(), (0, AnimationAction::Standing)),
        ]);

        let npc_payload = json!({
            "sceneView": {"center": {"x": 10, "y": 10}, "width": 19, "height": 15},
            "_nativeHighlightTarget": true,
            "_nativeHoverCursor": {"x": 482.2, "y": 353.2},
            "entities": [hover_fixture_entity(3001, "npc", 11, false)]
        });
        let npc_state = build_entity_render_state_with_manifest_and_pixels_for_test(
            &npc_payload,
            &poses,
            true,
            &manifest,
            &HashMap::new(),
        )
        .expect("npc bounds fallback state");
        assert_eq!(npc_state["hoveredObjectId"], json!("3001"));
        assert!(has_layer(&npc_state, "3001:hover-highlight:body"));

        let monster_payload = json!({
            "sceneView": {"center": {"x": 10, "y": 10}, "width": 19, "height": 15},
            "_nativeHighlightTarget": true,
            "_nativeHoverCursor": {"x": 482.2, "y": 353.2},
            "entities": [hover_fixture_entity(2001, "monster", 11, false)]
        });
        let monster_state = build_entity_render_state_with_manifest_and_pixels_for_test(
            &monster_payload,
            &poses,
            true,
            &manifest,
            &HashMap::new(),
        )
        .expect("monster fail-closed state");
        assert!(monster_state["hoveredObjectId"].is_null());
        assert!(!has_layer(&monster_state, "2001:hover-highlight:body"));
    }

    #[test]
    fn highlight_setting_gates_hover_and_selection_and_selected_band_follows_hover() {
        let manifest = hover_fixture_manifest();
        let pixels = hover_fixture_pixels(1, 1);
        let poses = HashMap::from([
            ("2001".to_owned(), (0, AnimationAction::Standing)),
            ("2002".to_owned(), (0, AnimationAction::Standing)),
        ]);
        let mut payload = json!({
            "sceneView": {"center": {"x": 10, "y": 10}},
            "selectedObjectId": 2002,
            "_nativeHighlightTarget": true,
            "_nativeHoverCursor": {"x": 482.2, "y": 353.2},
            "entities": [
                hover_fixture_entity(2001, "monster", 11, false),
                hover_fixture_entity(2002, "monster", 12, false)
            ]
        });
        let state = build_entity_render_state_with_manifest_and_pixels_for_test(
            &payload, &poses, true, &manifest, &pixels,
        )
        .expect("hover and selected state");
        let hover = state["entities"][0]["layers"]
            .as_array()
            .expect("hover layers")
            .iter()
            .find(|layer| layer["key"] == "2001:hover-highlight:body")
            .expect("hover redraw");
        let selected = state["entities"][1]["layers"]
            .as_array()
            .expect("selected layers")
            .iter()
            .find(|layer| layer["key"] == "2002:target-highlight:body")
            .expect("selected redraw");
        assert!(hover["z"].as_f64() < selected["z"].as_f64());

        payload["selectedObjectId"] = json!(2001);
        let same = build_entity_render_state_with_manifest_and_pixels_for_test(
            &payload, &poses, true, &manifest, &pixels,
        )
        .expect("same hover and selection state");
        assert!(has_layer(&same, "2001:target-highlight:body"));
        assert!(!has_layer(&same, "2001:hover-highlight:body"));

        payload["_nativeHighlightTarget"] = json!(false);
        let disabled = build_entity_render_state_with_manifest_and_pixels_for_test(
            &payload, &poses, true, &manifest, &pixels,
        )
        .expect("disabled highlight state");
        assert!(!has_layer(&disabled, "2001:target-highlight:body"));
        assert!(!has_layer(&disabled, "2001:hover-highlight:body"));
        assert_eq!(disabled["hoveredObjectId"], json!("2001"));
    }

    #[test]
    fn rgb_to_rgba_expands_with_alpha() {
        assert_eq!(
            rgb_to_rgba(&[1, 2, 3, 4, 5, 6]),
            vec![1, 2, 3, 255, 4, 5, 6, 255]
        );
    }
}
