//! Rust map-file parser + atlas-backed MapRenderState builder.
//!
//! Reads Crystal type-100 `.map` files (the modern C# header format used by the
//! demo Bichon map), resolves each cell's middle/front tile to a library frame,
//! indexes the packed map-atlas manifest, and produces a `MapRenderState` JSON
//! the shared runtime renders with real texture atlases.
//!
//! Mirrors the Web loader:
//! - `parseType100Map` (crystal-map-loader.ts)
//! - `mapLibraryKeyForIndex` (index -> "WemadeMir2/Tiles" etc.)
//! - `mapAtlasRectKeyForPath` ("/original-map/<lib>/<frame>.png" -> "<lib>#<frame>")

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::{json, Value};

use crate::assets;

const STAGE_WIDTH: f32 = 1024.0;
const STAGE_HEIGHT: f32 = 768.0;
const CELL_WIDTH: f32 = 48.0;
const CELL_HEIGHT: f32 = 32.0;

/// A parsed type-100 map cell.
#[derive(Debug, Clone)]
pub struct MapCell {
    pub back_index: i16,
    pub back_image: i32,
    pub middle_index: i16,
    pub middle_image: i16,
    pub front_index: i16,
    pub front_image: i16,
    /// `frontAnimationFrame`: high bit = additive, low 7 bits = frame count.
    pub front_animation_frame: u8,
    /// `frontAnimationTick`: frames-per-repeat (0 => repeat once).
    #[allow(dead_code)]
    pub front_animation_tick: u8,
    /// `middleAnimationFrame`: high bit = additive, low 7 bits = frame count.
    pub middle_animation_frame: u8,
    #[allow(dead_code)]
    pub middle_animation_tick: u8,
}

/// A single map tile draw (back, middle, or front layer) resolved to an atlas rect.
#[derive(Debug, Clone)]
pub struct MapTileDraw {
    pub x: i32,
    pub y: i32,
    pub layer: TileLayer,
    pub library: String,
    pub frame_index: i32,
    pub additive: bool,
    /// Number of animation frames this tile cycles through (>= 1).
    #[allow(dead_code)]
    pub frame_count: u32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileLayer {
    Back,
    Middle,
    Front,
}

/// A parsed type-100 map.
#[derive(Debug, Clone)]
pub struct ParsedMap {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<MapCell>,
}

/// Parse a Crystal type-100 `.map` byte buffer.
///
/// Header: `01 43 23` magic, then width@4, height@6 (i16 LE); each cell is 26
/// bytes from offset 8.
pub fn parse_type100_map(bytes: &[u8]) -> Option<ParsedMap> {
    if bytes.len() < 8 || bytes[2] != 0x43 || bytes[3] != 0x23 {
        return None;
    }
    let width = u16::from_le_bytes([bytes[4], bytes[5]]);
    let height = u16::from_le_bytes([bytes[6], bytes[7]]);
    let cell_bytes = 8usize + width as usize * height as usize * 26;
    if bytes.len() < cell_bytes {
        return None;
    }

    let mut cells = Vec::with_capacity(width as usize * height as usize);
    let mut offset = 8usize;
    for _ in 0..(width as usize * height as usize) {
        cells.push(MapCell {
            back_index: i16::from_le_bytes([bytes[offset], bytes[offset + 1]]),
            back_image: i32::from_le_bytes([
                bytes[offset + 2],
                bytes[offset + 3],
                bytes[offset + 4],
                bytes[offset + 5],
            ]),
            middle_index: i16::from_le_bytes([bytes[offset + 6], bytes[offset + 7]]),
            middle_image: i16::from_le_bytes([bytes[offset + 8], bytes[offset + 9]]),
            front_index: i16::from_le_bytes([bytes[offset + 10], bytes[offset + 11]]),
            front_image: i16::from_le_bytes([bytes[offset + 12], bytes[offset + 13]]),
            front_animation_frame: bytes[offset + 16],
            front_animation_tick: bytes[offset + 17],
            middle_animation_frame: bytes[offset + 18],
            middle_animation_tick: bytes[offset + 19],
        });
        offset += 26;
    }
    Some(ParsedMap {
        width,
        height,
        cells,
    })
}

/// Map a cell library index to a library key (mirrors `mapLibraryKeyForIndex`).
pub fn library_key_for_index(index: i16) -> String {
    match index {
        0 => "WemadeMir2/Tiles".to_owned(),
        1 => "WemadeMir2/SmTiles".to_owned(),
        2 => "WemadeMir2/Objects".to_owned(),
        idx if (3..=29).contains(&idx) => format!("WemadeMir2/Objects{}", idx - 1),
        90 => "WemadeMir2/Objects_32bit".to_owned(),
        100 => "ShandaMir2/Tiles".to_owned(),
        idx if (101..=109).contains(&idx) => format!("ShandaMir2/Tiles{}", idx - 99),
        110 => "ShandaMir2/SmTiles".to_owned(),
        idx if (111..=119).contains(&idx) => format!("ShandaMir2/SmTiles{}", idx - 109),
        120 => "ShandaMir2/Objects".to_owned(),
        idx if (121..=150).contains(&idx) => format!("ShandaMir2/Objects{}", idx - 119),
        190 => "ShandaMir2/AniTiles1".to_owned(),
        _ => "WemadeMir2/Tiles".to_owned(),
    }
}

/// Build the atlas-rect key for a library + frame index (mirrors
/// `mapAtlasRectKeyForPath`: "<library>#<frame>").
pub fn atlas_rect_key(library: &str, frame_index: i32) -> String {
    format!("{library}#{frame_index}")
}

/// Decode the middle-layer animation frame count (mirrors
/// `decodeCrystalMiddleAnimationCount`).
pub fn middle_animation_count(animation_frame: u8) -> u8 {
    if animation_frame == 0 || animation_frame >= 0xff {
        0
    } else {
        animation_frame & 0x0f
    }
}

/// Decode the front-layer animation frame count (mirrors
/// `decodeCrystalFrontAnimationCount`).
pub fn front_animation_count(animation_frame: u8) -> u8 {
    if animation_frame > 0 {
        animation_frame & 0x7f
    } else {
        0
    }
}

/// Whether a front animation frame is additive (high bit set).
#[allow(dead_code)]
pub fn front_is_additive(animation_frame: u8) -> bool {
    (animation_frame & 0x80) != 0
}

/// Crystal/Web additive routing for middle-layer map cells.
pub fn middle_is_additive(animation_frame: u8) -> bool {
    let count = middle_animation_count(animation_frame);
    count == 8 || count == 10 || (animation_frame & 0x80) != 0
}

/// Resolve a cell's back + middle + front layers into atlas-resolved tile draws.
///
/// Each layer becomes one `MapTileDraw` (single frame render for the current
/// slice; `frame_count` carries the animation width so a future frame-cycling
/// renderer can expand it).
pub fn resolve_map_tile_draws(map: &ParsedMap) -> Vec<MapTileDraw> {
    let mut draws = Vec::new();
    // Type-100 cells are serialized x-major, matching the authoritative Web
    // parser's `cells[x * height + y]` lookup.
    for x in 0..map.width {
        for y in 0..map.height {
            let cell = &map.cells[x as usize * map.height as usize + y as usize];

            // Back layer: Crystal's primary floor. Omitting this layer leaves
            // most walkable cells transparent/black even when middle/front
            // object overlays are present.
            let back_frame = (cell.back_image & 0x1fff_ffff) - 1;
            if cell.back_index >= 0 && back_frame >= 0 && x % 2 == 0 && y % 2 == 0 {
                draws.push(MapTileDraw {
                    x: x as i32,
                    y: y as i32,
                    layer: TileLayer::Back,
                    library: library_key_for_index(cell.back_index),
                    frame_index: back_frame,
                    additive: false,
                    frame_count: 1,
                    z: -2.0,
                });
            }

            // Middle layer: primary ground tile (index 0/1/2 => Tiles/SmTiles/Objects).
            let middle_frame = i32::from(cell.middle_image) - 1;
            if middle_frame >= 0 {
                draws.push(MapTileDraw {
                    x: x as i32,
                    y: y as i32,
                    layer: TileLayer::Middle,
                    library: library_key_for_index(cell.middle_index),
                    frame_index: middle_frame,
                    additive: middle_is_additive(cell.middle_animation_frame),
                    frame_count: u32::from(middle_animation_count(cell.middle_animation_frame))
                        .max(1),
                    z: 0.0,
                });
            }

            // Front layer: overlays drawn on top of the ground.
            let front_frame = (i32::from(cell.front_image) & 0x7fff) - 1;
            if cell.front_index >= 0 && front_frame >= 0 {
                draws.push(MapTileDraw {
                    x: x as i32,
                    y: y as i32,
                    layer: TileLayer::Front,
                    library: library_key_for_index(cell.front_index),
                    frame_index: front_frame,
                    additive: front_is_additive(cell.front_animation_frame),
                    frame_count: u32::from(front_animation_count(cell.front_animation_frame))
                        .max(1),
                    z: 1.0,
                });
            }
        }
    }
    draws
}

/// Locate a local map pack file (`.map.gz`) for the given map file name.
fn find_map_file(map_file_name: &str) -> Option<PathBuf> {
    let file_name = Path::new(map_file_name).file_name()?.to_str()?;
    if file_name != map_file_name || file_name.contains("..") {
        return None;
    }
    let stem = file_name
        .strip_suffix(".map.gz")
        .or_else(|| file_name.strip_suffix(".map"))
        .unwrap_or(file_name);
    let root = assets::asset_root()?;
    [
        root.join("crystal-map-pack"),
        root.join("generated/crystal-map-pack"),
        root.join("../lib/generated/crystal-map-pack"),
    ]
    .into_iter()
    .map(|pack| pack.join(format!("{stem}.map.gz")))
    .find(|candidate| candidate.is_file())
}

/// Locate the map-atlas manifest built by `assets:map-atlas:build`.
fn find_map_atlas_manifest() -> Option<PathBuf> {
    assets::asset_path("generated/map-atlas/manifest.json").filter(|path| path.is_file())
}

fn find_native_keyed_manifest() -> Option<PathBuf> {
    assets::asset_path("generated/native-map-keyed/manifest.json").filter(|path| path.is_file())
}

/// Atlas page index: `atlasKey -> (imageUrl, rectKey -> (x,y,w,h))`.
struct AtlasIndex {
    pages: HashMap<String, AtlasPage>,
    rect_to_atlas: HashMap<String, String>,
    rects: HashMap<String, AtlasRect>,
}

#[derive(Clone)]
struct AtlasPage {
    image_url: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct AtlasRect {
    key: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Clone)]
struct StandaloneIndex {
    entries: HashMap<String, StandaloneAsset>,
}

#[derive(Clone)]
struct StandaloneAsset {
    image_url: String,
    width: u32,
    height: u32,
    placement_mode: StandalonePlacementMode,
    offset_x: i32,
    offset_y: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StandalonePlacementMode {
    BottomLeft,
    SourceOffset,
}

fn build_original_map_frame_path(library: &str, frame_index: i32) -> String {
    format!("/original-map/{library}/{frame_index}.png")
}

fn map_path_requires_alpha_key(path: &str) -> bool {
    let normalized = path.split('?').next().unwrap_or(path).to_ascii_lowercase();
    if !normalized.starts_with("/original-map/") {
        return false;
    }
    let Some(segment) = normalized.rsplit('/').nth(1) else {
        return false;
    };
    map_library_segment_requires_alpha_key(segment)
}

fn map_library_segment_requires_alpha_key(segment: &str) -> bool {
    if segment == "object1c" || segment == "object2c" {
        return true;
    }
    if let Some(rest) = segment.strip_prefix("objects") {
        return rest.is_empty()
            || rest == "_32bit"
            || rest.chars().all(|character| character.is_ascii_digit());
    }
    if let Some(rest) = segment.strip_prefix("smobjects") {
        return rest.is_empty() || rest.chars().all(|character| character.is_ascii_digit());
    }

    fn matches_optional_plural_c(segment: &str, stem: &str) -> bool {
        segment == stem
            || segment == format!("{stem}s")
            || segment == format!("{stem}c")
            || segment == format!("{stem}sc")
    }

    matches_optional_plural_c(segment, "furniture")
        || matches_optional_plural_c(segment, "wall")
        || matches_optional_plural_c(segment, "animation")
        || matches_optional_plural_c(segment, "house")
        || matches_optional_plural_c(segment, "cliff")
        || matches_optional_plural_c(segment, "dungeon")
        || matches_optional_plural_c(segment, "inner")
}

fn standalone_image_key(rect_key: &str, additive: bool) -> String {
    if additive {
        format!("standalone-additive:{rect_key}")
    } else {
        format!("standalone:{rect_key}")
    }
}

fn standalone_tile_key(draw: &MapTileDraw, rect_key: &str) -> String {
    format!(
        "standalone:{}:{}:{}:{}",
        if draw.additive { "additive" } else { "normal" },
        draw.x,
        draw.y,
        rect_key
    )
}

/// Load + index the map-atlas manifest (compact schema v2 or legacy `atlases`).
fn load_atlas_index() -> Option<AtlasIndex> {
    let path = find_map_atlas_manifest()?;
    let text = fs::read_to_string(&path).ok()?;
    let manifest: Value = serde_json::from_str(&text).ok()?;

    let mut pages = HashMap::new();
    let mut rect_to_atlas = HashMap::new();
    let mut indexed_rects = HashMap::new();

    // Compact schema v2: `pages: [{l,p,w,h,u,r:[[frame,x,y,w,h],...]}]`
    if let Some(pages_val) = manifest.get("pages").and_then(Value::as_array) {
        for page in pages_val {
            let (Some(l), Some(p), Some(w), Some(h), Some(u)) = (
                page.get("l").and_then(Value::as_str),
                page.get("p").and_then(Value::as_u64),
                page.get("w").and_then(Value::as_u64),
                page.get("h").and_then(Value::as_u64),
                page.get("u").and_then(Value::as_str),
            ) else {
                continue;
            };
            let atlas_key = format!("map:{l}#p{p}");
            pages.insert(
                atlas_key.clone(),
                AtlasPage {
                    image_url: u.to_owned(),
                    width: w as u32,
                    height: h as u32,
                },
            );
            if let Some(rects) = page.get("r").and_then(Value::as_array) {
                for rect in rects {
                    let arr = match rect.as_array() {
                        Some(arr) if arr.len() == 5 => arr,
                        _ => continue,
                    };
                    let frame = arr[0]
                        .as_u64()
                        .or_else(|| arr[0].as_str().and_then(|s| s.parse().ok()));
                    let Some(frame) = frame else { continue };
                    let key = format!("{l}#{frame}");
                    let (Some(x), Some(y), Some(width), Some(height)) = (
                        arr[1].as_u64(),
                        arr[2].as_u64(),
                        arr[3].as_u64(),
                        arr[4].as_u64(),
                    ) else {
                        continue;
                    };
                    rect_to_atlas.insert(key.clone(), atlas_key.clone());
                    indexed_rects.insert(
                        key.clone(),
                        AtlasRect {
                            key,
                            x: x as u32,
                            y: y as u32,
                            width: width as u32,
                            height: height as u32,
                        },
                    );
                }
            }
        }
    }
    // Legacy schema v1: `atlases: [{key,imageUrl,rects:[{key,x,y,w,h}]}]`
    else if let Some(atlases) = manifest.get("atlases").and_then(Value::as_array) {
        for atlas in atlases {
            let (Some(key), Some(image_url), Some(w), Some(h)) = (
                atlas.get("key").and_then(Value::as_str),
                atlas.get("imageUrl").and_then(Value::as_str),
                atlas.get("width").and_then(Value::as_u64),
                atlas.get("height").and_then(Value::as_u64),
            ) else {
                continue;
            };
            pages.insert(
                key.to_owned(),
                AtlasPage {
                    image_url: image_url.to_owned(),
                    width: w as u32,
                    height: h as u32,
                },
            );
            if let Some(rects) = atlas.get("rects").and_then(Value::as_array) {
                for rect in rects {
                    let (Some(rect_key), Some(x), Some(y), Some(width), Some(height)) = (
                        rect.get("key").and_then(Value::as_str),
                        rect.get("x").and_then(Value::as_u64),
                        rect.get("y").and_then(Value::as_u64),
                        rect.get("width")
                            .or_else(|| rect.get("w"))
                            .and_then(Value::as_u64),
                        rect.get("height")
                            .or_else(|| rect.get("h"))
                            .and_then(Value::as_u64),
                    ) else {
                        continue;
                    };
                    rect_to_atlas.insert(rect_key.to_owned(), key.to_owned());
                    indexed_rects.insert(
                        rect_key.to_owned(),
                        AtlasRect {
                            key: rect_key.to_owned(),
                            x: x as u32,
                            y: y as u32,
                            width: width as u32,
                            height: height as u32,
                        },
                    );
                }
            }
        }
    }

    if pages.is_empty() {
        return None;
    }
    Some(AtlasIndex {
        pages,
        rect_to_atlas,
        rects: indexed_rects,
    })
}

static NATIVE_KEYED_INDEX_CACHE: OnceLock<Mutex<Option<(PathBuf, StandaloneIndex)>>> =
    OnceLock::new();

fn load_native_keyed_index() -> Option<StandaloneIndex> {
    let path = find_native_keyed_manifest()?;
    let cache = NATIVE_KEYED_INDEX_CACHE.get_or_init(|| Mutex::new(None));
    {
        let cached = cache.lock().ok()?;
        if let Some((cached_path, index)) = cached.as_ref() {
            if cached_path == &path {
                return Some(index.clone());
            }
        }
    }
    let text = fs::read_to_string(&path).ok()?;
    let manifest: Value = serde_json::from_str(&text).ok()?;
    let mut entries = HashMap::new();
    for entry in manifest
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(key), Some(image_url), Some(width), Some(height)) = (
            entry.get("key").and_then(Value::as_str),
            entry.get("imageUrl").and_then(Value::as_str),
            entry.get("width").and_then(Value::as_u64),
            entry.get("height").and_then(Value::as_u64),
        ) else {
            continue;
        };
        let placement_mode = match entry.get("placementMode").and_then(Value::as_str) {
            Some("source-offset") => StandalonePlacementMode::SourceOffset,
            _ => StandalonePlacementMode::BottomLeft,
        };
        let offset_x = entry.get("offsetX").and_then(Value::as_i64).unwrap_or(0) as i32;
        let offset_y = entry.get("offsetY").and_then(Value::as_i64).unwrap_or(0) as i32;
        entries.insert(
            key.to_owned(),
            StandaloneAsset {
                image_url: image_url.to_owned(),
                width: width as u32,
                height: height as u32,
                placement_mode,
                offset_x,
                offset_y,
            },
        );
    }
    let index = StandaloneIndex { entries };
    if let Ok(mut cached) = cache.lock() {
        *cached = Some((path, index.clone()));
    }
    Some(index)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapViewport {
    pub center_x: i32,
    pub center_y: i32,
    pub width: i32,
    pub height: i32,
}

impl MapViewport {
    pub fn from_gateway_payload(payload: &Value) -> Self {
        let scene_view = payload.get("sceneView");
        let center = scene_view.and_then(|view| view.get("center"));
        Self {
            center_x: center
                .and_then(|value| value.get("x"))
                .and_then(Value::as_i64)
                .unwrap_or(0) as i32,
            center_y: center
                .and_then(|value| value.get("y"))
                .and_then(Value::as_i64)
                .unwrap_or(0) as i32,
            width: scene_view
                .and_then(|view| view.get("width"))
                .and_then(Value::as_i64)
                .unwrap_or(19)
                .clamp(1, 128) as i32,
            height: scene_view
                .and_then(|view| view.get("height"))
                .and_then(Value::as_i64)
                .unwrap_or(15)
                .clamp(1, 128) as i32,
        }
    }
}

/// Build a viewport-relative `MapRenderState` with the exact rect geometry the
/// runtime requires for texture-atlas lookup.
pub fn build_map_render_state(map: &ParsedMap, viewport: MapViewport) -> Option<Value> {
    let atlas_index = load_atlas_index()?;
    let standalone_index = load_native_keyed_index();

    build_map_render_state_with_indexes(map, viewport, &atlas_index, standalone_index.as_ref())
}

fn build_map_render_state_with_indexes(
    map: &ParsedMap,
    viewport: MapViewport,
    atlas_index: &AtlasIndex,
    standalone_index: Option<&StandaloneIndex>,
) -> Option<Value> {
    let draws = resolve_map_tile_draws(map);
    let mut used_rects: HashMap<String, HashSet<String>> = HashMap::new();
    let mut tiles: Vec<Value> = Vec::new();
    let mut standalone_tiles: Vec<Value> = Vec::new();
    let mut missing_standalone = HashSet::new();
    let margin_x = viewport.width / 2 + 6;
    let margin_y = viewport.height / 2 + 6;
    let tile_origin_x = (STAGE_WIDTH / 2.0 / CELL_WIDTH).floor() * CELL_WIDTH
        - (STAGE_WIDTH / 2.0 / CELL_WIDTH).floor();
    let tile_origin_y = ((STAGE_HEIGHT / 2.0 / CELL_HEIGHT).floor() - 1.0) * CELL_HEIGHT;

    for draw in &draws {
        if (draw.x - viewport.center_x).abs() > margin_x
            || (draw.y - viewport.center_y).abs() > margin_y
        {
            continue;
        }
        let rect_key = atlas_rect_key(&draw.library, draw.frame_index);
        let image_path = build_original_map_frame_path(&draw.library, draw.frame_index);
        let requires_standalone = draw.additive || map_path_requires_alpha_key(&image_path);
        let cell_left = tile_origin_x + (draw.x - viewport.center_x) as f32 * CELL_WIDTH;
        let cell_top = tile_origin_y + (draw.y - viewport.center_y) as f32 * CELL_HEIGHT;
        let depth = (draw.y * 1_000 + draw.x * 10) as f32 + draw.z;
        if requires_standalone {
            if let Some(asset) = standalone_index.and_then(|index| index.entries.get(&rect_key)) {
                let (left, top) = match asset.placement_mode {
                    StandalonePlacementMode::BottomLeft => {
                        (cell_left, cell_top + CELL_HEIGHT - asset.height as f32)
                    }
                    StandalonePlacementMode::SourceOffset => (
                        cell_left + asset.offset_x as f32,
                        cell_top + CELL_HEIGHT - asset.height as f32 + asset.offset_y as f32,
                    ),
                };
                standalone_tiles.push(json!({
                    "key": standalone_tile_key(draw, &rect_key),
                    "imageKey": standalone_image_key(&rect_key, draw.additive),
                    "imageUrl": asset.image_url,
                    "left": left,
                    "top": top,
                    "width": asset.width,
                    "height": asset.height,
                    "z": depth,
                    "additive": draw.additive,
                }));
            } else {
                missing_standalone.insert(rect_key.clone());
            }
            continue;
        }
        let Some(atlas_key) = atlas_index.rect_to_atlas.get(&rect_key).cloned() else {
            continue;
        };
        let Some(rect) = atlas_index.rects.get(&rect_key) else {
            continue;
        };
        used_rects
            .entry(atlas_key.clone())
            .or_default()
            .insert(rect_key.clone());
        let floor_sized = (rect.width == CELL_WIDTH as u32 && rect.height == CELL_HEIGHT as u32)
            || (rect.width == (CELL_WIDTH * 2.0) as u32
                && rect.height == (CELL_HEIGHT * 2.0) as u32);
        let draw_as_floor = draw.layer == TileLayer::Back || (draw.frame_count == 1 && floor_sized);
        let (left, top) = if draw_as_floor {
            (cell_left, cell_top)
        } else {
            (
                cell_left + (CELL_WIDTH - rect.width as f32) / 2.0,
                cell_top + CELL_HEIGHT - rect.height as f32,
            )
        };
        let layer_key = match draw.layer {
            TileLayer::Back => "back",
            TileLayer::Middle => "mid",
            TileLayer::Front => "front",
        };
        tiles.push(json!({
            "key": format!("{}:{}:{}", layer_key, draw.x, draw.y),
            "atlasKey": atlas_key,
            "rectKey": rect_key,
            "left": left,
            "top": top,
            "width": rect.width,
            "height": rect.height,
            "z": depth,
        }));
    }

    let atlases: Vec<Value> = used_rects
        .iter()
        .filter_map(|(key, used)| {
            let page = atlas_index.pages.get(key)?;
            let rects = used
                .iter()
                .filter_map(|rect_key| atlas_index.rects.get(rect_key))
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
                "key": key,
                "width": page.width,
                "height": page.height,
                "imageUrl": page.image_url,
                "rects": rects,
            }))
        })
        .collect();

    if !missing_standalone.is_empty() {
        eprintln!(
            "[native-map] skipped {} standalone tiles with missing native keyed/additive assets",
            missing_standalone.len()
        );
    }

    if atlases.is_empty() && tiles.is_empty() && standalone_tiles.is_empty() {
        return None;
    }

    Some(json!({
        "enabled": true,
        "stageWidth": STAGE_WIDTH,
        "stageHeight": STAGE_HEIGHT,
        "centerX": viewport.center_x,
        "centerY": viewport.center_y,
        "ackKey": format!("native-map:{}:{}", viewport.center_x, viewport.center_y),
        "tiles": tiles,
        "atlases": atlases,
        "standaloneTiles": standalone_tiles,
        "retainedImageKeys": [],
    }))
}

/// Locate + parse a local `.map.gz` for the given map file name.
pub fn load_map(map_file_name: &str) -> Option<ParsedMap> {
    let path = find_map_file(map_file_name)?;
    let compressed = fs::read(&path).ok()?;
    let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).ok()?;
    parse_type100_map(&bytes)
}

/// Whether a local map pack + atlas are available for real map rendering.
pub fn has_local_map_atlas() -> bool {
    find_map_file("0").is_some() && find_map_atlas_manifest().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn middle_cell(library: i16, frame: i16) -> MapCell {
        MapCell {
            back_index: -1,
            back_image: 0,
            middle_index: library,
            middle_image: frame + 1,
            front_index: -1,
            front_image: 0,
            front_animation_frame: 0,
            front_animation_tick: 0,
            middle_animation_frame: 0,
            middle_animation_tick: 0,
        }
    }

    fn viewport() -> MapViewport {
        MapViewport {
            center_x: 0,
            center_y: 0,
            width: 19,
            height: 15,
        }
    }

    fn atlas_index_for(rect_key: &str, width: u32, height: u32) -> AtlasIndex {
        let atlas_key = "map:test#p0".to_owned();
        AtlasIndex {
            pages: HashMap::from([(
                atlas_key.clone(),
                AtlasPage {
                    image_url: "/generated/map-atlas/test/p0.png".to_owned(),
                    width: 512,
                    height: 512,
                },
            )]),
            rect_to_atlas: HashMap::from([(rect_key.to_owned(), atlas_key.clone())]),
            rects: HashMap::from([(
                rect_key.to_owned(),
                AtlasRect {
                    key: rect_key.to_owned(),
                    x: 1,
                    y: 1,
                    width,
                    height,
                },
            )]),
        }
    }

    fn standalone_index_for(
        rect_key: &str,
        image_url: &str,
        width: u32,
        height: u32,
    ) -> StandaloneIndex {
        StandaloneIndex {
            entries: HashMap::from([(
                rect_key.to_owned(),
                StandaloneAsset {
                    image_url: image_url.to_owned(),
                    width,
                    height,
                    placement_mode: StandalonePlacementMode::BottomLeft,
                    offset_x: 0,
                    offset_y: 0,
                },
            )]),
        }
    }

    fn standalone_index_for_source_offset(
        rect_key: &str,
        image_url: &str,
        width: u32,
        height: u32,
        offset_x: i32,
        offset_y: i32,
    ) -> StandaloneIndex {
        StandaloneIndex {
            entries: HashMap::from([(
                rect_key.to_owned(),
                StandaloneAsset {
                    image_url: image_url.to_owned(),
                    width,
                    height,
                    placement_mode: StandalonePlacementMode::SourceOffset,
                    offset_x,
                    offset_y,
                },
            )]),
        }
    }

    #[test]
    fn type100_map_parses_dimensions_and_cells() {
        // Build a tiny 2x2 map: magic + width/height + 4 cells * 26 bytes.
        let mut bytes = vec![0x01, 0x00, 0x43, 0x23];
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        for _ in 0..4 {
            bytes.extend_from_slice(&[0u8; 26]);
        }
        let map = parse_type100_map(&bytes).expect("parse");
        assert_eq!(map.width, 2);
        assert_eq!(map.height, 2);
        assert_eq!(map.cells.len(), 4);
    }

    #[test]
    fn library_key_mapping_matches_web() {
        assert_eq!(library_key_for_index(0), "WemadeMir2/Tiles");
        assert_eq!(library_key_for_index(1), "WemadeMir2/SmTiles");
        assert_eq!(library_key_for_index(2), "WemadeMir2/Objects");
        assert_eq!(library_key_for_index(5), "WemadeMir2/Objects4");
        assert_eq!(library_key_for_index(100), "ShandaMir2/Tiles");
        assert_eq!(library_key_for_index(120), "ShandaMir2/Objects");
    }

    #[test]
    fn atlas_rect_key_matches_map_atlas_rect_key_for_path() {
        assert_eq!(
            atlas_rect_key("WemadeMir2/Tiles", 901),
            "WemadeMir2/Tiles#901"
        );
    }

    #[test]
    fn wrong_magic_rejected() {
        assert!(parse_type100_map(&[0, 0, 0, 0, 0, 0, 0, 0]).is_none());
    }

    #[test]
    fn animation_counts_decode_like_the_web() {
        assert_eq!(front_animation_count(0), 0);
        assert_eq!(front_animation_count(0x05), 5);
        assert_eq!(front_animation_count(0x8A), 0x0A); // additive + 10 frames
        assert!(front_is_additive(0x8A));
        assert!(!front_is_additive(0x0A));
        assert_eq!(middle_animation_count(0), 0);
        assert_eq!(middle_animation_count(8), 8);
        assert_eq!(middle_animation_count(0x88), 8);
        assert_eq!(middle_animation_count(0xff), 0);
        assert!(middle_is_additive(8));
        assert!(middle_is_additive(10));
        assert!(middle_is_additive(0x88));
    }

    #[test]
    fn resolve_tile_draws_includes_front_layer() {
        // 2x2 map: cell 0 middle=WemadeMir2/Tiles#901, cell 1 has a front layer.
        let mut bytes = vec![0x01, 0x00, 0x43, 0x23];
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());

        let mut cell_bytes = [0u8; 26];
        // middle: index 0 (Tiles), image 902 (=> frame 901)
        cell_bytes[6] = 0;
        cell_bytes[7] = 0;
        cell_bytes[8] = 0x86;
        cell_bytes[9] = 0x03;
        bytes.extend_from_slice(&cell_bytes);

        let mut blank = [0u8; 26];
        // Give cells 1-3 a valid middle frame too (Tiles frame 902).
        blank[8] = 0x87;
        blank[9] = 0x03;
        bytes.extend_from_slice(&blank);
        bytes.extend_from_slice(&blank);
        bytes.extend_from_slice(&blank);

        let map = parse_type100_map(&bytes).expect("parse");
        let draws = resolve_map_tile_draws(&map);
        assert_eq!(draws.len(), 4); // one middle draw per cell
        assert_eq!(draws[0].layer, TileLayer::Middle);
        assert_eq!(draws[0].library, "WemadeMir2/Tiles");
        assert_eq!(draws[0].frame_index, 901);
    }

    #[test]
    fn type100_back_layer_is_the_primary_floor_at_the_cell_origin() {
        let mut bytes = vec![0x01, 0x00, 0x43, 0x23];
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        let mut cell = [0u8; 26];
        cell[0..2].copy_from_slice(&0i16.to_le_bytes());
        cell[2..6].copy_from_slice(&902i32.to_le_bytes());
        bytes.extend_from_slice(&cell);

        let map = parse_type100_map(&bytes).expect("type-100 map");
        let draw = resolve_map_tile_draws(&map).remove(0);
        assert_eq!(draw.layer, TileLayer::Back);
        assert_eq!(draw.library, "WemadeMir2/Tiles");
        assert_eq!(draw.frame_index, 901);

        let state = build_map_render_state(
            &map,
            MapViewport {
                center_x: 0,
                center_y: 0,
                width: 19,
                height: 15,
            },
        )
        .expect("back-layer atlas state");
        assert_eq!(state["tiles"][0]["key"], json!("back:0:0"));
        assert_eq!(state["tiles"][0]["left"], json!(470.0));
        assert_eq!(state["tiles"][0]["top"], json!(352.0));
    }

    #[test]
    fn front_layer_draws_on_top() {
        // Single cell with a front overlay (Objects library).
        let mut bytes = vec![0x01, 0x00, 0x43, 0x23];
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());

        let mut cell_bytes = [0u8; 26];
        // middle: Tiles frame 1
        cell_bytes[6] = 0;
        cell_bytes[8] = 0x02;
        // front: Objects (index 2), frame 1
        cell_bytes[10] = 0x02;
        cell_bytes[11] = 0;
        cell_bytes[12] = 0x02;
        cell_bytes[13] = 0;
        bytes.extend_from_slice(&cell_bytes);

        let map = parse_type100_map(&bytes).expect("parse");
        let draws = resolve_map_tile_draws(&map);
        assert_eq!(draws.len(), 2);
        assert!(draws.iter().any(|d| d.layer == TileLayer::Front));
        let front = draws.iter().find(|d| d.layer == TileLayer::Front).unwrap();
        assert_eq!(front.library, "WemadeMir2/Objects");
        assert_eq!(front.z, 1.0);
    }

    #[test]
    fn odd_cells_do_not_emit_back_layer_draws() {
        let map = ParsedMap {
            width: 2,
            height: 2,
            cells: vec![
                MapCell {
                    back_index: 0,
                    back_image: 2,
                    middle_index: -1,
                    middle_image: 0,
                    front_index: -1,
                    front_image: 0,
                    front_animation_frame: 0,
                    front_animation_tick: 0,
                    middle_animation_frame: 0,
                    middle_animation_tick: 0,
                },
                MapCell {
                    back_index: 0,
                    back_image: 2,
                    middle_index: -1,
                    middle_image: 0,
                    front_index: -1,
                    front_image: 0,
                    front_animation_frame: 0,
                    front_animation_tick: 0,
                    middle_animation_frame: 0,
                    middle_animation_tick: 0,
                },
                MapCell {
                    back_index: 0,
                    back_image: 2,
                    middle_index: -1,
                    middle_image: 0,
                    front_index: -1,
                    front_image: 0,
                    front_animation_frame: 0,
                    front_animation_tick: 0,
                    middle_animation_frame: 0,
                    middle_animation_tick: 0,
                },
                MapCell {
                    back_index: 0,
                    back_image: 2,
                    middle_index: -1,
                    middle_image: 0,
                    front_index: -1,
                    front_image: 0,
                    front_animation_frame: 0,
                    front_animation_tick: 0,
                    middle_animation_frame: 0,
                    middle_animation_tick: 0,
                },
            ],
        };
        let draws = resolve_map_tile_draws(&map);
        let back_positions: Vec<(i32, i32)> = draws
            .iter()
            .filter(|draw| draw.layer == TileLayer::Back)
            .map(|draw| (draw.x, draw.y))
            .collect();
        assert_eq!(back_positions, vec![(0, 0)]);
    }

    #[test]
    fn non_square_type100_cells_resolve_x_major_coordinates() {
        let map = ParsedMap {
            width: 2,
            height: 3,
            cells: (0..6).map(|frame| middle_cell(0, frame)).collect(),
        };
        let draws = resolve_map_tile_draws(&map);
        assert_eq!((draws[0].x, draws[0].y, draws[0].frame_index), (0, 0, 0));
        assert_eq!((draws[2].x, draws[2].y, draws[2].frame_index), (0, 2, 2));
        assert_eq!((draws[3].x, draws[3].y, draws[3].frame_index), (1, 0, 3));
    }

    #[test]
    fn middle_animation_uses_its_own_type100_field() {
        let mut bytes = vec![0x01, 0x00, 0x43, 0x23];
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        let mut cell = [0u8; 26];
        cell[8] = 1;
        cell[16] = 9;
        cell[18] = 3;
        bytes.extend_from_slice(&cell);
        let map = parse_type100_map(&bytes).expect("type-100 map");
        let draw = resolve_map_tile_draws(&map).remove(0);
        assert_eq!(draw.frame_count, 3);
    }

    #[test]
    fn map_render_state_includes_used_manifest_rect_geometry() {
        let map = ParsedMap {
            width: 1,
            height: 1,
            cells: vec![middle_cell(110, 80)],
        };
        let state = build_map_render_state(
            &map,
            MapViewport {
                center_x: 0,
                center_y: 0,
                width: 19,
                height: 15,
            },
        )
        .expect("map atlas state");
        assert_eq!(state["tiles"][0]["rectKey"], json!("ShandaMir2/SmTiles#80"));
        let rect = &state["atlases"][0]["rects"][0];
        assert_eq!(rect["x"], json!(1));
        assert_eq!(rect["y"], json!(1));
        assert_eq!(rect["width"], json!(48));
        assert_eq!(rect["height"], json!(32));
        assert_eq!(state["tiles"][0]["left"], json!(470.0));
        assert_eq!(state["tiles"][0]["top"], json!(352.0));
        assert_eq!(state["stageWidth"], json!(STAGE_WIDTH));
        assert_eq!(state["stageHeight"], json!(STAGE_HEIGHT));
    }

    #[test]
    fn alpha_key_object_routes_to_standalone_without_atlas_fallback() {
        let map = ParsedMap {
            width: 1,
            height: 1,
            cells: vec![middle_cell(2, 7112)],
        };
        let rect_key = atlas_rect_key("WemadeMir2/Objects", 7112);
        let atlas = atlas_index_for(&rect_key, 48, 32);
        let standalone = standalone_index_for(
            &rect_key,
            "/generated/native-map-keyed/pages/hash.png",
            64,
            96,
        );
        let state =
            build_map_render_state_with_indexes(&map, viewport(), &atlas, Some(&standalone))
                .expect("standalone state");
        assert_eq!(state["tiles"].as_array().unwrap().len(), 0);
        assert_eq!(state["standaloneTiles"].as_array().unwrap().len(), 1);
        assert_eq!(
            state["standaloneTiles"][0]["imageUrl"],
            json!("/generated/native-map-keyed/pages/hash.png")
        );
        assert_eq!(
            state["standaloneTiles"][0]["imageKey"],
            json!("standalone:WemadeMir2/Objects#7112")
        );
        assert_eq!(
            state["standaloneTiles"][0]["key"],
            json!("standalone:normal:0:0:WemadeMir2/Objects#7112")
        );
        assert_eq!(state["standaloneTiles"][0]["left"], json!(470.0));
        assert_eq!(state["standaloneTiles"][0]["top"], json!(288.0));
    }

    #[test]
    fn non_offset_standalone_uses_bottom_left_not_centered_anchor() {
        let map = ParsedMap {
            width: 1,
            height: 1,
            cells: vec![middle_cell(2, 9000)],
        };
        let rect_key = atlas_rect_key("WemadeMir2/Objects", 9000);
        let atlas = atlas_index_for(&rect_key, 48, 32);
        let standalone = standalone_index_for(
            &rect_key,
            "/generated/native-map-keyed/pages/wide.png",
            96,
            64,
        );
        let state =
            build_map_render_state_with_indexes(&map, viewport(), &atlas, Some(&standalone))
                .expect("wide standalone state");
        assert_eq!(
            state["standaloneTiles"][0]["left"],
            json!(470.0),
            "ordinary object placement must stay at cellLeft even when width != 48"
        );
    }

    #[test]
    fn additive_front_routes_to_standalone_even_when_atlas_has_rect() {
        let map = ParsedMap {
            width: 1,
            height: 1,
            cells: vec![MapCell {
                back_index: -1,
                back_image: 0,
                middle_index: -1,
                middle_image: 0,
                front_index: 2,
                front_image: 2,
                front_animation_frame: 0x81,
                front_animation_tick: 0,
                middle_animation_frame: 0,
                middle_animation_tick: 0,
            }],
        };
        let rect_key = atlas_rect_key("WemadeMir2/Objects", 1);
        let atlas = atlas_index_for(&rect_key, 48, 32);
        let standalone =
            standalone_index_for(&rect_key, "/original-map/WemadeMir2/Objects/1.png", 48, 64);
        let state =
            build_map_render_state_with_indexes(&map, viewport(), &atlas, Some(&standalone))
                .expect("additive standalone state");
        assert_eq!(state["tiles"].as_array().unwrap().len(), 0);
        assert_eq!(state["standaloneTiles"][0]["additive"], json!(true));
        assert_eq!(
            state["standaloneTiles"][0]["imageKey"],
            json!("standalone-additive:WemadeMir2/Objects#1")
        );
        assert_eq!(
            state["standaloneTiles"][0]["key"],
            json!("standalone:additive:0:0:WemadeMir2/Objects#1")
        );
    }

    #[test]
    fn missing_standalone_entry_skips_black_key_objects_instead_of_using_raw_atlas() {
        let map = ParsedMap {
            width: 1,
            height: 1,
            cells: vec![middle_cell(2, 7112)],
        };
        let rect_key = atlas_rect_key("WemadeMir2/Objects", 7112);
        let atlas = atlas_index_for(&rect_key, 48, 32);
        let state = build_map_render_state_with_indexes(&map, viewport(), &atlas, None);
        assert!(
            state.is_none(),
            "missing keyed object must not fall back to raw atlas"
        );
    }

    #[test]
    fn missing_authoritative_map_does_not_fall_back_to_bichon() {
        assert!(load_map("definitely-missing-map").is_none());
    }

    #[test]
    fn source_offset_standalone_preserves_crystal_object_placement() {
        let map = ParsedMap {
            width: 1,
            height: 1,
            cells: vec![middle_cell(2, 2723)],
        };
        let rect_key = atlas_rect_key("WemadeMir2/Objects", 2723);
        let atlas = atlas_index_for(&rect_key, 48, 32);
        let standalone = standalone_index_for_source_offset(
            &rect_key,
            "/generated/native-map-keyed/pages/raw.png",
            100,
            145,
            -51,
            -113,
        );
        let state =
            build_map_render_state_with_indexes(&map, viewport(), &atlas, Some(&standalone))
                .expect("source-offset standalone state");
        assert_eq!(state["standaloneTiles"][0]["left"], json!(419.0));
        assert_eq!(state["standaloneTiles"][0]["top"], json!(126.0));
    }

    #[test]
    fn standalone_keys_change_when_representation_changes() {
        let rect_key = atlas_rect_key("WemadeMir2/Objects", 1);
        let normal = MapTileDraw {
            x: 3,
            y: 4,
            layer: TileLayer::Front,
            library: "WemadeMir2/Objects".to_owned(),
            frame_index: 1,
            additive: false,
            frame_count: 1,
            z: 1.0,
        };
        let additive = MapTileDraw {
            additive: true,
            ..normal.clone()
        };
        assert_ne!(
            standalone_tile_key(&normal, &rect_key),
            standalone_tile_key(&additive, &rect_key)
        );
    }
}
