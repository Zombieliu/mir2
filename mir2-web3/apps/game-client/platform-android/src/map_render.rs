//! Android map-layout adapter for the shared Bevy `MapRenderState` contract.
//!
//! This first bounded producer supports Crystal type-100 maps and atlas-backed
//! floor-safe images. Alpha-keyed/additive object layers remain unresolved
//! until the separate native keyed-object pack is connected.

use crate::{
    map_objects::MapObjectPack,
    world_assets::{MapAtlasPageDescriptor, WorldAssetError},
    world_projection::ProjectedScene,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

const STAGE_WIDTH: f32 = 1024.0;
const STAGE_HEIGHT: f32 = 768.0;
const CELL_WIDTH: f32 = 48.0;
const CELL_HEIGHT: f32 = 32.0;
const MAP_RENDER_GUARD_CELLS: i32 = 6;
const CRYSTAL_VIEW_RANGE_X: i32 = 16;
const CRYSTAL_VIEW_RANGE_Y: i32 = 17;
const CRYSTAL_FLOOR_LOOKAHEAD_ROWS: i32 = 5;
const CRYSTAL_OBJECT_LOOKAHEAD_ROWS: i32 = 25;
const MAP_FRONT_DEPTH_ORDER: f32 = 1.0;
const MAP_TILE_ANIMATION_DEPTH_ORDER: f32 = -0.5;
const MAP_FLOOR_DEPTH_MIN: f32 = -2.0;
const MAP_FLOOR_DEPTH_SPAN: f64 = 1.0;
const MAX_DECOMPRESSED_MAP_BYTES: usize = 32 * 1024 * 1024;
const MAX_MAP_CELLS: usize = 1_000_000;

#[derive(Debug, Clone)]
struct MapCell {
    back_index: i16,
    back_image: i32,
    middle_index: i16,
    middle_image: i16,
    front_index: i16,
    front_image: i16,
    front_animation_frame: u8,
    front_animation_tick: u8,
    middle_animation_frame: u8,
    middle_animation_tick: u8,
    tile_animation_image: i16,
    tile_animation_offset: i16,
    tile_animation_frames: u8,
}

#[derive(Debug, Clone)]
struct ParsedMap {
    width: u16,
    height: u16,
    cells: Vec<MapCell>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TileLayer {
    Back,
    TileAnimation,
    Middle,
    Front,
}

#[derive(Debug, Clone)]
struct MapTileDraw {
    x: i32,
    y: i32,
    layer: TileLayer,
    library: String,
    frame_index: i32,
    additive: bool,
    frame_count: u32,
    animation_tick: u32,
    frame_step: i32,
    z: f32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MapRenderState {
    #[serde(rename = "_nativeWorldRequest")]
    native_world_request: u64,
    enabled: bool,
    stage_width: f32,
    stage_height: f32,
    ack_key: String,
    revision: u64,
    center_x: i32,
    center_y: i32,
    unresolved_draw_count: usize,
    atlases: Vec<MapRenderAtlas>,
    tiles: Vec<MapTile>,
    standalone_tiles: Vec<MapStandaloneTile>,
    retained_image_keys: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MapRenderAtlas {
    key: String,
    width: u32,
    height: u32,
    rects: Vec<MapRenderAtlasRect>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MapRenderAtlasRect {
    key: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MapTile {
    key: String,
    atlas_key: String,
    #[serde(rename = "rectKey")]
    atlas_rect_key: String,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    z: f32,
    animation_phase: u32,
    animation_frame_count: u32,
    animation_tick: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MapStandaloneTile {
    key: String,
    image_key: String,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    z: f32,
    additive: bool,
    animation_phase: u32,
    animation_frame_count: u32,
    animation_tick: u32,
}

#[derive(Debug)]
pub(crate) struct MapRenderProduct {
    pub(crate) json: String,
    pub(crate) tile_count: usize,
    pub(crate) standalone_tile_count: usize,
    pub(crate) atlas_count: usize,
    pub(crate) unresolved_draw_count: usize,
    pub(crate) map_width: u16,
    pub(crate) map_height: u16,
    pub(crate) atlas_keys: Vec<String>,
    pub(crate) standalone_source_keys: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct Viewport {
    center_x: i32,
    center_y: i32,
    width: i32,
    height: i32,
}

impl From<&ProjectedScene> for Viewport {
    fn from(scene: &ProjectedScene) -> Self {
        Self {
            center_x: scene.center_x,
            center_y: scene.center_y,
            width: scene.width.clamp(1, 128),
            height: scene.height.clamp(1, 128),
        }
    }
}

impl Viewport {
    fn draw_margin_x(self) -> i32 {
        (self.width / 2 + MAP_RENDER_GUARD_CELLS).max(CRYSTAL_VIEW_RANGE_X)
    }

    fn draw_margin_y(self) -> i32 {
        (self.height / 2 + MAP_RENDER_GUARD_CELLS).max(CRYSTAL_VIEW_RANGE_Y)
    }

    fn floor_draw_bottom_margin_y(self) -> i32 {
        self.draw_margin_y()
            .max(CRYSTAL_VIEW_RANGE_Y + CRYSTAL_FLOOR_LOOKAHEAD_ROWS)
    }

    fn object_draw_bottom_margin_y(self) -> i32 {
        self.draw_margin_y()
            .max(CRYSTAL_VIEW_RANGE_Y + CRYSTAL_OBJECT_LOOKAHEAD_ROWS)
    }

    fn retains_draw(self, layer: TileLayer, x: i32, y: i32) -> bool {
        if x.abs_diff(self.center_x) > self.draw_margin_x() as u32 {
            return false;
        }
        let min_y = self.center_y.saturating_sub(self.draw_margin_y());
        let max_y = self.center_y.saturating_add(match layer {
            TileLayer::Back => self.floor_draw_bottom_margin_y(),
            TileLayer::TileAnimation | TileLayer::Middle | TileLayer::Front => {
                self.object_draw_bottom_margin_y()
            }
        });
        (min_y..=max_y).contains(&y)
    }
}

fn normalize_map_stem(map_file_name: &str) -> Result<String, WorldAssetError> {
    if map_file_name.is_empty()
        || map_file_name.len() > 128
        || !map_file_name.is_ascii()
        || map_file_name.contains(['/', '\\', '\0'])
        || map_file_name.contains("..")
    {
        return Err(WorldAssetError::new(
            "map file name is not a safe asset key",
        ));
    }
    let stem = map_file_name
        .strip_suffix(".map.gz")
        .or_else(|| map_file_name.strip_suffix(".map"))
        .unwrap_or(map_file_name)
        .to_ascii_lowercase();
    if stem.is_empty()
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(WorldAssetError::new(
            "map file name is not a safe asset key",
        ));
    }
    Ok(stem)
}

fn parse_type100_map(bytes: &[u8]) -> Result<ParsedMap, WorldAssetError> {
    if bytes.len() < 8 || bytes[2] != 0x43 || bytes[3] != 0x23 {
        return Err(WorldAssetError::new(
            "packaged map is not a supported Crystal type-100 layout",
        ));
    }
    let width = u16::from_le_bytes([bytes[4], bytes[5]]);
    let height = u16::from_le_bytes([bytes[6], bytes[7]]);
    let cell_count = usize::from(width)
        .checked_mul(usize::from(height))
        .ok_or_else(|| WorldAssetError::new("packaged map dimensions overflow"))?;
    if width == 0 || height == 0 || cell_count > MAX_MAP_CELLS {
        return Err(WorldAssetError::new(
            "packaged map dimensions are out of bounds",
        ));
    }
    let required = 8usize
        .checked_add(
            cell_count
                .checked_mul(26)
                .ok_or_else(|| WorldAssetError::new("packaged map cell bytes overflow"))?,
        )
        .ok_or_else(|| WorldAssetError::new("packaged map byte count overflows"))?;
    if required > MAX_DECOMPRESSED_MAP_BYTES || bytes.len() < required {
        return Err(WorldAssetError::new(
            "packaged map cell data is incomplete or oversized",
        ));
    }

    let mut cells = Vec::with_capacity(cell_count);
    let mut offset = 8usize;
    for _ in 0..cell_count {
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
            tile_animation_image: i16::from_le_bytes([bytes[offset + 20], bytes[offset + 21]]),
            tile_animation_offset: i16::from_le_bytes([bytes[offset + 22], bytes[offset + 23]]),
            tile_animation_frames: bytes[offset + 24],
        });
        offset += 26;
    }
    Ok(ParsedMap {
        width,
        height,
        cells,
    })
}

fn middle_animation_count(animation_frame: u8) -> u8 {
    if animation_frame == 0 || animation_frame >= 0xff {
        0
    } else {
        animation_frame & 0x0f
    }
}

fn front_animation_count(animation_frame: u8) -> u8 {
    if animation_frame > 0 {
        animation_frame & 0x7f
    } else {
        0
    }
}

fn middle_is_additive(animation_frame: u8) -> bool {
    let count = middle_animation_count(animation_frame);
    count == 8 || count == 10 || (animation_frame & 0x80) != 0
}

fn library_key_for_index(index: i16) -> String {
    if let Some(key) = mir3_library_key(index, 200, "WemadeMir3") {
        return key;
    }
    if let Some(key) = mir3_library_key(index, 300, "ShandaMir3") {
        return key;
    }
    match index {
        0 => "WemadeMir2/Tiles".to_owned(),
        1 => "WemadeMir2/SmTiles".to_owned(),
        2 => "WemadeMir2/Objects".to_owned(),
        value if (3..=29).contains(&value) => format!("WemadeMir2/Objects{}", value - 1),
        90 => "WemadeMir2/Objects_32bit".to_owned(),
        100 => "ShandaMir2/Tiles".to_owned(),
        value if (101..=109).contains(&value) => format!("ShandaMir2/Tiles{}", value - 99),
        110 => "ShandaMir2/SmTiles".to_owned(),
        value if (111..=119).contains(&value) => {
            format!("ShandaMir2/SmTiles{}", value - 109)
        }
        120 => "ShandaMir2/Objects".to_owned(),
        value if (121..=150).contains(&value) => {
            format!("ShandaMir2/Objects{}", value - 119)
        }
        190 => "ShandaMir2/AniTiles1".to_owned(),
        _ => "WemadeMir2/Tiles".to_owned(),
    }
}

fn mir3_library_key(index: i16, base_index: i16, root: &str) -> Option<String> {
    let offset = i32::from(index) - i32::from(base_index);
    if !(0..75).contains(&offset) {
        return None;
    }
    let state_index = usize::try_from(offset / 15).ok()?;
    let slot = usize::try_from(offset % 15).ok()?;
    let name = [
        "Tilesc",
        "Tiles30c",
        "Tiles5c",
        "SmTilesc",
        "Housesc",
        "Cliffsc",
        "Dungeonsc",
        "Innersc",
        "Furnituresc",
        "Wallsc",
        "SmObjectsc",
        "Animationsc",
        "Object1c",
        "Object2c",
    ]
    .get(slot)?;
    if root == "WemadeMir3" {
        if matches!(*name, "Object1c" | "Object2c") {
            return Some(format!("{root}/{name}"));
        }
        let folder = ["", "Wood", "Sand", "Snow", "Forest"].get(state_index)?;
        return Some(if folder.is_empty() {
            format!("{root}/{name}")
        } else {
            format!("{root}/{folder}/{name}")
        });
    }
    let suffix = ["", "wood", "sand", "snow", "forest"].get(state_index)?;
    Some(format!("{root}/{name}{suffix}"))
}

fn resolve_draws(map: &ParsedMap, viewport: Viewport) -> Vec<MapTileDraw> {
    let min_x = viewport
        .center_x
        .saturating_sub(viewport.draw_margin_x())
        .max(0);
    let max_x = viewport
        .center_x
        .saturating_add(viewport.draw_margin_x())
        .min(i32::from(map.width).saturating_sub(1));
    let min_y = viewport
        .center_y
        .saturating_sub(viewport.draw_margin_y())
        .max(0);
    let max_y = viewport
        .center_y
        .saturating_add(viewport.object_draw_bottom_margin_y())
        .min(i32::from(map.height).saturating_sub(1));
    let mut draws = Vec::new();
    if min_x > max_x || min_y > max_y {
        return draws;
    }
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            let cell = &map.cells[x as usize * usize::from(map.height) + y as usize];
            let back_frame = (cell.back_image & 0x1fff_ffff) - 1;
            if cell.back_index >= 0 && back_frame >= 0 && x % 2 == 0 && y % 2 == 0 {
                draws.push(MapTileDraw {
                    x,
                    y,
                    layer: TileLayer::Back,
                    library: library_key_for_index(cell.back_index),
                    frame_index: back_frame,
                    additive: false,
                    frame_count: 1,
                    animation_tick: 0,
                    frame_step: 0,
                    z: -2.0,
                });
            }
            let tile_animation_frame = i32::from(cell.tile_animation_image) - 1;
            let tile_animation_count = u32::from(cell.tile_animation_frames);
            if tile_animation_frame >= 0 && tile_animation_count > 0 {
                draws.push(MapTileDraw {
                    x,
                    y,
                    layer: TileLayer::TileAnimation,
                    library: library_key_for_index(190),
                    frame_index: tile_animation_frame,
                    additive: false,
                    frame_count: tile_animation_count,
                    animation_tick: 0,
                    frame_step: i32::from(cell.tile_animation_offset) ^ 0x2000,
                    z: MAP_TILE_ANIMATION_DEPTH_ORDER,
                });
            }
            let middle_frame = i32::from(cell.middle_image) - 1;
            if middle_frame >= 0 {
                draws.push(MapTileDraw {
                    x,
                    y,
                    layer: TileLayer::Middle,
                    library: library_key_for_index(cell.middle_index),
                    frame_index: middle_frame,
                    additive: middle_is_additive(cell.middle_animation_frame),
                    frame_count: u32::from(middle_animation_count(cell.middle_animation_frame))
                        .max(1),
                    animation_tick: u32::from(cell.middle_animation_tick),
                    frame_step: 1,
                    z: 0.0,
                });
            }
            let front_frame = (i32::from(cell.front_image) & 0x7fff) - 1;
            if cell.front_index >= 0 && front_frame >= 0 {
                draws.push(MapTileDraw {
                    x,
                    y,
                    layer: TileLayer::Front,
                    library: library_key_for_index(cell.front_index),
                    frame_index: front_frame,
                    additive: (cell.front_animation_frame & 0x80) != 0,
                    frame_count: u32::from(front_animation_count(cell.front_animation_frame))
                        .max(1),
                    animation_tick: u32::from(cell.front_animation_tick),
                    frame_step: 1,
                    z: MAP_FRONT_DEPTH_ORDER,
                });
            }
        }
    }
    draws.retain(|draw| viewport.retains_draw(draw.layer, draw.x, draw.y));
    draws
}

fn library_requires_standalone(library: &str) -> bool {
    let segment = library
        .rsplit('/')
        .next()
        .unwrap_or(library)
        .to_ascii_lowercase();
    if matches!(segment.as_str(), "object1c" | "object2c") {
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
    [
        "furniture",
        "wall",
        "animation",
        "house",
        "cliff",
        "dungeon",
        "inner",
    ]
    .iter()
    .any(|stem| {
        segment == *stem
            || segment == format!("{stem}s")
            || segment == format!("{stem}c")
            || segment == format!("{stem}sc")
    })
}

fn floor_sized_frame(width: u32, height: u32) -> bool {
    (width == CELL_WIDTH as u32 && height == CELL_HEIGHT as u32)
        || (width == (CELL_WIDTH * 2.0) as u32 && height == (CELL_HEIGHT * 2.0) as u32)
}

fn map_draw_is_floor(layer: TileLayer, animated: bool, width: u32, height: u32) -> bool {
    layer == TileLayer::Back || (!animated && floor_sized_frame(width, height))
}

fn map_floor_depth(map: &ParsedMap, x: i32, y: i32) -> f32 {
    let width = u64::from(map.width);
    let height = u64::from(map.height);
    let cell_count = width.saturating_mul(height);
    if cell_count == 0 {
        return MAP_FLOOR_DEPTH_MIN;
    }
    let x = u64::try_from(x.max(0))
        .unwrap_or(0)
        .min(width.saturating_sub(1));
    let y = u64::try_from(y.max(0))
        .unwrap_or(0)
        .min(height.saturating_sub(1));
    let rank = y
        .saturating_mul(width)
        .saturating_add(x)
        .min(cell_count.saturating_sub(1));
    (f64::from(MAP_FLOOR_DEPTH_MIN) + rank as f64 / cell_count as f64 * MAP_FLOOR_DEPTH_SPAN) as f32
}

fn map_render_revision(stem: &str, viewport: Viewport) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    const REVISION_SCHEMA: &[u8] = b"mir2-native-map-render-v1";
    let mut revision = FNV_OFFSET_BASIS;
    for byte in REVISION_SCHEMA
        .iter()
        .copied()
        .chain(stem.bytes())
        .chain(viewport.center_x.to_le_bytes())
        .chain(viewport.center_y.to_le_bytes())
        .chain(viewport.width.to_le_bytes())
        .chain(viewport.height.to_le_bytes())
        .chain([1])
    {
        revision ^= u64::from(byte);
        revision = revision.wrapping_mul(FNV_PRIME);
    }
    revision
}

fn build_render_state(
    map: &ParsedMap,
    scene: &ProjectedScene,
    descriptors: &[MapAtlasPageDescriptor],
    map_objects: &MapObjectPack,
    stem: &str,
    request_id: u64,
) -> Result<MapRenderProduct, WorldAssetError> {
    let viewport = Viewport::from(scene);
    let mut rect_index = HashMap::new();
    for (page_index, page) in descriptors.iter().enumerate() {
        for (rect_index_in_page, rect) in page.rects.iter().enumerate() {
            rect_index.insert(rect.key.as_str(), (page_index, rect_index_in_page));
        }
    }

    let mut tiles = Vec::new();
    let mut standalone_tiles = Vec::new();
    let mut standalone_source_keys = BTreeSet::new();
    let mut used: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut unresolved_draw_count = 0usize;
    let tile_origin_x = (STAGE_WIDTH / 2.0 / CELL_WIDTH).floor() * CELL_WIDTH
        - (STAGE_WIDTH / 2.0 / CELL_WIDTH).floor();
    let tile_origin_y = ((STAGE_HEIGHT / 2.0 / CELL_HEIGHT).floor() - 1.0) * CELL_HEIGHT;

    for draw in resolve_draws(map, viewport) {
        if draw.additive || library_requires_standalone(&draw.library) {
            let requested_frame_count = draw.frame_count.max(1);
            let mut resolved = Vec::with_capacity(requested_frame_count as usize);
            for phase in 0..requested_frame_count {
                let Some(frame_index) = i32::try_from(phase)
                    .ok()
                    .and_then(|phase| draw.frame_step.checked_mul(phase))
                    .and_then(|offset| draw.frame_index.checked_add(offset))
                else {
                    resolved.clear();
                    break;
                };
                let source_key = format!("{}#{frame_index}", draw.library);
                let Some(entry) = map_objects.entries.get(&source_key) else {
                    resolved.clear();
                    break;
                };
                resolved.push((phase, source_key, entry));
            }
            let family_complete = resolved.len() == requested_frame_count as usize;
            if !family_complete {
                let source_key = format!("{}#{}", draw.library, draw.frame_index);
                let Some(entry) = map_objects.entries.get(&source_key) else {
                    unresolved_draw_count += 1;
                    continue;
                };
                unresolved_draw_count += 1;
                resolved = vec![(0, source_key, entry)];
            }
            let effective_frame_count = if family_complete {
                requested_frame_count
            } else {
                1
            };
            let animated = effective_frame_count > 1;
            let cell_left = tile_origin_x + (draw.x - viewport.center_x) as f32 * CELL_WIDTH;
            let cell_top = tile_origin_y + (draw.y - viewport.center_y) as f32 * CELL_HEIGHT;
            let cell_depth = (draw.y * 1_000 + draw.x * 10) as f32 + draw.z;
            let layer_key = match draw.layer {
                TileLayer::Back => "back",
                TileLayer::TileAnimation => "tile-animation",
                TileLayer::Middle => "mid",
                TileLayer::Front => "front",
            };
            for (phase, source_key, entry) in resolved {
                let draw_as_floor =
                    map_draw_is_floor(draw.layer, animated, entry.width, entry.height);
                let z = if draw_as_floor {
                    map_floor_depth(map, draw.x, draw.y)
                } else {
                    cell_depth
                };
                let (left, top) = if draw_as_floor {
                    (cell_left, cell_top)
                } else if let Some((offset_x, offset_y)) = entry.offset {
                    (cell_left + offset_x as f32, cell_top + offset_y as f32)
                } else {
                    (
                        cell_left + (CELL_WIDTH - entry.width as f32) / 2.0,
                        cell_top + CELL_HEIGHT - entry.height as f32,
                    )
                };
                let base_key = format!("{layer_key}:{}:{}", draw.x, draw.y);
                standalone_tiles.push(MapStandaloneTile {
                    key: if animated {
                        format!("{base_key}:anim:{phase}")
                    } else {
                        base_key
                    },
                    image_key: entry.image_key.clone(),
                    left,
                    top,
                    width: entry.width as f32,
                    height: entry.height as f32,
                    z,
                    additive: draw.additive,
                    animation_phase: phase,
                    animation_frame_count: effective_frame_count,
                    animation_tick: draw.animation_tick,
                });
                standalone_source_keys.insert(source_key);
            }
            continue;
        }
        let requested_frame_count = draw.frame_count.max(1);
        let mut resolved_frames = Vec::with_capacity(requested_frame_count as usize);
        for phase in 0..requested_frame_count {
            let Some(frame_index) = i32::try_from(phase)
                .ok()
                .and_then(|phase| draw.frame_step.checked_mul(phase))
                .and_then(|offset| draw.frame_index.checked_add(offset))
            else {
                resolved_frames.clear();
                break;
            };
            let key = format!("{}#{frame_index}", draw.library);
            let Some(&(page_index, rect_index_in_page)) = rect_index.get(key.as_str()) else {
                resolved_frames.clear();
                break;
            };
            resolved_frames.push((phase, page_index, rect_index_in_page));
        }
        let family_complete = resolved_frames.len() == requested_frame_count as usize;
        if !family_complete {
            let key = format!("{}#{}", draw.library, draw.frame_index);
            let Some(&(page_index, rect_index_in_page)) = rect_index.get(key.as_str()) else {
                unresolved_draw_count += 1;
                continue;
            };
            resolved_frames = vec![(0, page_index, rect_index_in_page)];
        }
        let effective_frame_count = if family_complete {
            requested_frame_count
        } else {
            1
        };
        let animated = effective_frame_count > 1;
        let cell_left = tile_origin_x + (draw.x - viewport.center_x) as f32 * CELL_WIDTH;
        let cell_top = tile_origin_y + (draw.y - viewport.center_y) as f32 * CELL_HEIGHT;
        let cell_depth = (draw.y * 1_000 + draw.x * 10) as f32 + draw.z;
        let layer_key = match draw.layer {
            TileLayer::Back => "back",
            TileLayer::TileAnimation => "tile-animation",
            TileLayer::Middle => "mid",
            TileLayer::Front => "front",
        };

        for (phase, page_index, rect_index_in_page) in resolved_frames {
            let page = &descriptors[page_index];
            let rect = &page.rects[rect_index_in_page];
            used.entry(page_index)
                .or_default()
                .insert(rect_index_in_page);
            let draw_as_floor = map_draw_is_floor(draw.layer, animated, rect.width, rect.height);
            let z = if draw_as_floor {
                map_floor_depth(map, draw.x, draw.y)
            } else {
                cell_depth
            };
            let (left, top) = if draw_as_floor {
                (cell_left, cell_top)
            } else {
                (
                    cell_left + (CELL_WIDTH - rect.width as f32) / 2.0,
                    cell_top + CELL_HEIGHT - rect.height as f32,
                )
            };
            let base_key = format!("{layer_key}:{}:{}", draw.x, draw.y);
            tiles.push(MapTile {
                key: if animated {
                    format!("{base_key}:anim:{phase}")
                } else {
                    base_key
                },
                atlas_key: page.key.clone(),
                atlas_rect_key: rect.key.clone(),
                left,
                top,
                width: rect.width as f32,
                height: rect.height as f32,
                z,
                animation_phase: phase,
                animation_frame_count: effective_frame_count,
                animation_tick: draw.animation_tick,
            });
        }
    }

    if tiles.is_empty() {
        return Err(WorldAssetError::new(
            "packaged map and atlas have no resolvable viewport tiles",
        ));
    }
    let atlases = used
        .into_iter()
        .map(|(page_index, rect_indexes)| {
            let page = &descriptors[page_index];
            MapRenderAtlas {
                key: page.key.clone(),
                width: page.width,
                height: page.height,
                rects: rect_indexes
                    .into_iter()
                    .map(|rect_index_in_page| {
                        let rect = &page.rects[rect_index_in_page];
                        MapRenderAtlasRect {
                            key: rect.key.clone(),
                            x: rect.x,
                            y: rect.y,
                            width: rect.width,
                            height: rect.height,
                        }
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    let state = MapRenderState {
        native_world_request: request_id,
        enabled: true,
        stage_width: STAGE_WIDTH,
        stage_height: STAGE_HEIGHT,
        ack_key: format!(
            "native-map:{stem}:{}:{}",
            viewport.center_x, viewport.center_y
        ),
        revision: map_render_revision(stem, viewport),
        center_x: viewport.center_x,
        center_y: viewport.center_y,
        unresolved_draw_count,
        atlases,
        tiles,
        standalone_tiles,
        retained_image_keys: Vec::new(),
    };
    Ok(MapRenderProduct {
        tile_count: state.tiles.len(),
        standalone_tile_count: state.standalone_tiles.len(),
        atlas_count: state.atlases.len(),
        unresolved_draw_count,
        map_width: map.width,
        map_height: map.height,
        atlas_keys: state
            .atlases
            .iter()
            .map(|atlas| atlas.key.clone())
            .collect(),
        standalone_source_keys: standalone_source_keys.into_iter().collect(),
        json: serde_json::to_string(&state)
            .map_err(|error| WorldAssetError::new(format!("map render state rejected: {error}")))?,
    })
}

pub(crate) fn load_map_render_state<F>(
    scene: &ProjectedScene,
    descriptors: &[MapAtlasPageDescriptor],
    map_objects: &MapObjectPack,
    request_id: u64,
    mut read_asset: F,
) -> Result<MapRenderProduct, WorldAssetError>
where
    F: FnMut(&str, usize) -> Result<Vec<u8>, WorldAssetError>,
{
    let stem = normalize_map_stem(&scene.map_file_name)?;
    let asset_path = format!("generated/crystal-map-pack/{stem}.map");
    let bytes = read_asset(&asset_path, MAX_DECOMPRESSED_MAP_BYTES)?;
    if bytes.is_empty() || bytes.len() > MAX_DECOMPRESSED_MAP_BYTES {
        return Err(WorldAssetError::new(
            "packaged map byte count is out of bounds",
        ));
    }
    let map = parse_type100_map(&bytes)?;
    build_render_state(&map, scene, descriptors, map_objects, &stem, request_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_assets::load_map_atlas_bundle;

    fn objects() -> MapObjectPack {
        MapObjectPack {
            entries: HashMap::new(),
        }
    }

    fn type100_fixture() -> Vec<u8> {
        let width = 4u16;
        let height = 4u16;
        let mut bytes = vec![0; 8 + usize::from(width) * usize::from(height) * 26];
        bytes[2] = 0x43;
        bytes[3] = 0x23;
        bytes[4..6].copy_from_slice(&width.to_le_bytes());
        bytes[6..8].copy_from_slice(&height.to_le_bytes());
        for index in 0..usize::from(width) * usize::from(height) {
            let offset = 8 + index * 26;
            bytes[offset..offset + 2].copy_from_slice(&(-1i16).to_le_bytes());
            bytes[offset + 6..offset + 8].copy_from_slice(&(-1i16).to_le_bytes());
            bytes[offset + 10..offset + 12].copy_from_slice(&(-1i16).to_le_bytes());
        }
        let offset = 8 + (2 * usize::from(height) + 2) * 26;
        bytes[offset..offset + 2].copy_from_slice(&0i16.to_le_bytes());
        bytes[offset + 2..offset + 6].copy_from_slice(&8i32.to_le_bytes());
        bytes
    }

    fn descriptor() -> MapAtlasPageDescriptor {
        MapAtlasPageDescriptor {
            key: "map:WemadeMir2/Tiles#p0".into(),
            asset_path: "generated/map-atlas/WemadeMir2-Tiles/p0.png".into(),
            width: 96,
            height: 64,
            declared_png_bytes: 1,
            rects: vec![crate::world_assets::MapAtlasRectDescriptor {
                key: "WemadeMir2/Tiles#7".into(),
                x: 0,
                y: 0,
                width: 96,
                height: 64,
            }],
        }
    }

    fn scene() -> ProjectedScene {
        ProjectedScene {
            map_file_name: "0".into(),
            center_x: 2,
            center_y: 2,
            width: 3,
            height: 3,
        }
    }

    #[test]
    fn type100_floor_draw_uses_shared_runtime_wire_shape() {
        let map = parse_type100_map(&type100_fixture()).unwrap();
        let product =
            build_render_state(&map, &scene(), &[descriptor()], &objects(), "0", 7).unwrap();
        assert_eq!(product.tile_count, 1);
        assert_eq!(product.atlas_count, 1);
        assert_eq!((product.map_width, product.map_height), (4, 4));
        let value: serde_json::Value = serde_json::from_str(&product.json).unwrap();
        assert_eq!(value["_nativeWorldRequest"], 7);
        assert_eq!(value["ackKey"], "native-map:0:2:2");
        assert_eq!(value["tiles"][0]["rectKey"], "WemadeMir2/Tiles#7");
        assert_eq!(value["atlases"][0]["imageUrl"], serde_json::Value::Null);
        assert_eq!(product.atlas_keys, ["map:WemadeMir2/Tiles#p0"]);
    }

    #[test]
    fn packaged_loader_rejects_unsafe_names_and_builds_valid_map() {
        let bytes = type100_fixture();
        let product = load_map_render_state(
            &scene(),
            &[descriptor()],
            &objects(),
            9,
            |path, max_bytes| {
                assert_eq!(path, "generated/crystal-map-pack/0.map");
                assert!(bytes.len() <= max_bytes);
                Ok(bytes.clone())
            },
        )
        .unwrap();
        assert_eq!(product.tile_count, 1);

        let mut invalid = scene();
        invalid.map_file_name = "../0".into();
        let error = load_map_render_state(
            &invalid,
            &[descriptor()],
            &objects(),
            9,
            |_, _| unreachable!(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("safe asset key"));
    }

    #[test]
    fn configured_bichon_pack_builds_real_floor_frame_when_present() {
        let Ok(root) = std::env::var("MIR2_ANDROID_WORLD_ASSET_ROOT") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let read = |path: &str, max_bytes: usize| {
            let bytes = std::fs::read(root.join(path)).map_err(|error| {
                WorldAssetError::new(format!("configured fixture could not be read: {error}"))
            })?;
            if bytes.is_empty() || bytes.len() > max_bytes {
                return Err(WorldAssetError::new(
                    "configured fixture has an invalid byte count",
                ));
            }
            Ok(bytes)
        };
        let bundle = load_map_atlas_bundle(read).unwrap();
        let scene = ProjectedScene {
            map_file_name: "0".into(),
            center_x: 302,
            center_y: 634,
            width: 22,
            height: 18,
        };
        let objects = crate::map_objects::load_map_object_pack("0", read).unwrap();
        let product =
            load_map_render_state(&scene, &bundle.descriptors, &objects, 11, read).unwrap();
        let state: serde_json::Value = serde_json::from_str(&product.json).unwrap();
        let visible_atlas_tiles = state["tiles"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|tile| {
                let atlas_key = tile["atlasKey"].as_str().unwrap();
                let rect_key = tile["rectKey"].as_str().unwrap();
                let page_index = bundle
                    .descriptors
                    .iter()
                    .position(|descriptor| descriptor.key == atlas_key)
                    .unwrap();
                let descriptor = &bundle.descriptors[page_index];
                let rect = descriptor
                    .rects
                    .iter()
                    .find(|rect| rect.key == rect_key)
                    .unwrap();
                let page = &bundle.pages[page_index];
                (rect.y..rect.y + rect.height).any(|y| {
                    (rect.x..rect.x + rect.width).any(|x| {
                        let offset = ((y * page.width + x) * 4) as usize;
                        page.rgba[offset + 3] != 0
                            && page.rgba[offset..offset + 3]
                                .iter()
                                .any(|channel| *channel != 0)
                    })
                })
            })
            .count();
        assert_eq!((product.map_width, product.map_height), (700, 700));
        assert_eq!(
            product.tile_count, 607,
            "real viewport remains deterministic"
        );
        assert_eq!(product.atlas_count, 7);
        assert_eq!(product.atlas_keys.len(), product.atlas_count);
        assert_eq!(visible_atlas_tiles, product.tile_count);
        assert_eq!(
            product.unresolved_draw_count, 0,
            "the initial Bichon viewport has complete standalone object coverage"
        );
        assert!(!product.standalone_source_keys.is_empty());
    }
}
