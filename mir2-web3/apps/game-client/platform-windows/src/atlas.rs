//! Native entity-atlas loading and viewport render-state construction.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    OnceLock,
};

use serde_json::{json, Value};

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
    rects: Vec<StarterAtlasRect>,
}

#[derive(Debug)]
struct StarterAtlasIndex {
    pages: Vec<StarterAtlasPage>,
    rect_by_path: HashMap<String, (usize, usize)>,
}

static STARTER_ATLAS_INDEX: OnceLock<Option<StarterAtlasIndex>> = OnceLock::new();
static RENDER_TRACE_STATE_LOGS: AtomicUsize = AtomicUsize::new(0);

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
fn decode_png_rgba(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let bytes = fs::read(path).ok()?;
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

    let mut decoded = Vec::new();
    for page in &index.pages {
        let Some(path) = assets::asset_path(&page.image_url) else {
            eprintln!("[atlas] invalid entity atlas path {}", page.image_url);
            return false;
        };
        let Some((width, height, pixels)) = decode_png_rgba(&path) else {
            eprintln!("[atlas] failed to decode {path:?}");
            return false;
        };
        if width != page.width || height != page.height {
            eprintln!(
                "[atlas] page {} dimensions are {width}x{height}, manifest expects {}x{}",
                page.key, page.width, page.height
            );
            return false;
        }
        decoded.push((page.key.clone(), width, height, pixels, path));
    }

    for (key, width, height, pixels, path) in decoded {
        if !mir2_bevy_runtime::native_ingest::push_native_entity_render_atlas(
            key.clone(),
            width,
            height,
            pixels,
        ) {
            return false;
        }
        eprintln!("[atlas] pushed {key} {width}x{height} ({path:?})");
    }
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
        AnimationAction::Walking | AnimationAction::Running | AnimationAction::AttackRange1
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
    required: bool,
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
    if !required
        && resolved.is_none()
        && !assets::asset_path(&frame_path).is_some_and(|path| path.is_file())
    {
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
        .unwrap_or((CELL_WIDTH, CELL_HEIGHT * 2.0, None, None));
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

/// Build viewport-relative entity layers with exact rect geometry from the
/// generated manifest. Missing atlas entries fall back to the individual PNG
/// path rather than inventing a rect key or coordinates.
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
    build_entity_render_state_internal(payload, frame_overrides, None)
}

pub(crate) fn build_entity_render_state_with_poses(
    payload: &Value,
    pose_overrides: &HashMap<String, (i64, AnimationAction)>,
) -> Option<Value> {
    build_entity_render_state_internal(payload, &HashMap::new(), Some(pose_overrides))
}

fn build_entity_render_state_internal(
    payload: &Value,
    frame_overrides: &HashMap<String, i64>,
    pose_overrides: Option<&HashMap<String, (i64, AnimationAction)>>,
) -> Option<Value> {
    let index = starter_atlas_index()?;
    let (center_x, center_y) = scene_center(payload);
    let entity_origin_x = (STAGE_WIDTH / 2.0 / CELL_WIDTH).floor() * CELL_WIDTH;
    let entity_origin_y = ((STAGE_HEIGHT / 2.0 / CELL_HEIGHT).floor() - 1.0) * CELL_HEIGHT;
    let mut used_rects: HashMap<String, HashSet<String>> = HashMap::new();

    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .map(|entities| {
            entities
                .iter()
                .map(|entity| {
                    let object_id = entity
                        .get("objectId")
                        .and_then(|value| match value {
                            Value::Number(number) => Some(number.to_string()),
                            Value::String(string) => Some(string.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
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
                    let cell_depth = (y * 1_000 + x * 10) as f32;
                    let z_base = cell_depth * ENTITY_DEPTH_GAIN;
                    let mut layers = Vec::new();

                    let mount_library = resolved_sprite.mount_library.as_ref();
                    if let Some(library) = &mount_library {
                        let mount_frame = relative_frame
                            .saturating_sub(416)
                            .saturating_add(resolved_sprite.mount_frame_offset);
                        if let Some(layer) = build_entity_layer(
                            index,
                            &mut used_rects,
                            format!("{object_id}:mount"),
                            library,
                            mount_frame,
                            root_left,
                            root_top,
                            z_base + ENTITY_MOUNT_ORDER,
                            false,
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
                                false,
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
                        true,
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
                            false,
                        ) {
                            layers.push(layer);
                        }
                    }
                    if entity.get("hidden").and_then(Value::as_bool) == Some(true) {
                        for layer in &mut layers {
                            layer["opacity"] = json!(0.5);
                        }
                    }

                    json!({
                        "objectId": object_id,
                        "isSelf": kind == "selfPlayer",
                        "dead": entity.get("dead").and_then(Value::as_bool).unwrap_or(false),
                        "gridX": x,
                        "gridY": y,
                        "layers": layers,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

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
    fn archer_walk_uses_alt_body_hair_and_single_weapon_then_returns_to_common_standing() {
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

    #[test]
    fn rgb_to_rgba_expands_with_alpha() {
        assert_eq!(
            rgb_to_rgba(&[1, 2, 3, 4, 5, 6]),
            vec![1, 2, 3, 255, 4, 5, 6, 255]
        );
    }
}
