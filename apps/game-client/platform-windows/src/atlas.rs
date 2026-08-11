//! Native entity-atlas loading and viewport render-state construction.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use serde_json::{json, Value};

use crate::assets;

const STAGE_WIDTH: f32 = 1024.0;
const STAGE_HEIGHT: f32 = 768.0;
const CELL_WIDTH: f32 = 48.0;
const CELL_HEIGHT: f32 = 32.0;

#[derive(Debug, Clone)]
struct StarterAtlasRect {
    key: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
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

fn starter_atlas_index() -> Option<&'static StarterAtlasIndex> {
    STARTER_ATLAS_INDEX
        .get_or_init(load_starter_atlas_index)
        .as_ref()
}

fn load_starter_atlas_index() -> Option<StarterAtlasIndex> {
    let path = assets::asset_path("bevy-entity-atlases/manifest.json")?;
    let manifest: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let mut pages = Vec::new();
    let mut rect_by_path = HashMap::new();

    for atlas in manifest.get("atlases")?.as_array()? {
        let key = atlas.get("key")?.as_str()?.to_owned();
        let image_url = atlas.get("imageUrl")?.as_str()?.to_owned();
        let width = atlas.get("width")?.as_u64()? as u32;
        let height = atlas.get("height")?.as_u64()? as u32;
        let mut rects = Vec::new();

        for rect in atlas.get("rects")?.as_array()? {
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
            };
            rect_by_path.insert(frame_path, (pages.len(), rects.len()));
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

    (!pages.is_empty()).then_some(StarterAtlasIndex {
        pages,
        rect_by_path,
    })
}

/// Whether every page in the starter atlas is available to the native host.
pub fn has_starter_atlas() -> bool {
    starter_atlas_index().is_some_and(|index| {
        index
            .pages
            .iter()
            .all(|page| assets::asset_path(&page.image_url).is_some_and(|path| path.is_file()))
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
pub fn build_entity_render_state(payload: &Value) -> Option<Value> {
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
                    let (library, frame) = starter_frame(
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
                    let frame_path = format!("{library}/{frame}.png");
                    let resolved =
                        index
                            .rect_by_path
                            .get(&frame_path)
                            .map(|(page_index, rect_index)| {
                                let page = &index.pages[*page_index];
                                (page, &page.rects[*rect_index])
                            });

                    let (width, height) = resolved
                        .map(|(_, rect)| (rect.width as f32, rect.height as f32))
                        .unwrap_or((CELL_WIDTH, CELL_HEIGHT * 2.0));
                    let root_left = entity_origin_x + (x - center_x) as f32 * CELL_WIDTH;
                    let root_top = entity_origin_y + (y - center_y) as f32 * CELL_HEIGHT;
                    let mut layer = json!({
                        "key": "body",
                        "path": frame_path,
                        "left": root_left + (CELL_WIDTH - width) / 2.0,
                        "top": root_top + CELL_HEIGHT - height,
                        "width": width,
                        "height": height,
                        "z": (y * 1_000 + x * 10) as f32,
                    });
                    if let Some((page, rect)) = resolved {
                        used_rects
                            .entry(page.key.clone())
                            .or_default()
                            .insert(rect.key.clone());
                        layer["atlasKey"] = json!(page.key);
                        layer["atlasRectKey"] = json!(rect.key);
                    }

                    json!({
                        "objectId": object_id,
                        "isSelf": kind == "selfPlayer",
                        "gridX": x,
                        "gridY": y,
                        "layers": [layer],
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

    Some(json!({
        "enabled": true,
        "stageWidth": STAGE_WIDTH,
        "stageHeight": STAGE_HEIGHT,
        "centerX": center_x,
        "centerY": center_y,
        "atlases": atlases,
        "entities": entities,
    }))
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
    fn entity_render_state_uses_manifest_geometry_and_viewport_coordinates() {
        let payload = json!({
            "sceneView": { "center": { "x": 9, "y": 7 } },
            "entities": [{
                "objectId": 2001,
                "kind": "monster",
                "x": 9,
                "y": 7,
                "direction": "down",
                "sprite": { "directionStride": 6, "frameBaseOffset": 0 }
            }]
        });
        let state = build_entity_render_state(&payload).expect("starter manifest");
        let layer = &state["entities"][0]["layers"][0];
        assert_eq!(layer["left"], json!(474.0));
        assert_eq!(layer["top"], json!(276.0));
        assert!(layer["atlasRectKey"]
            .as_str()
            .is_some_and(|key| key.contains("/Monster/000/24.png|")));

        let rect_key = layer["atlasRectKey"].as_str().expect("rect key");
        let rect = state["atlases"][0]["rects"]
            .as_array()
            .and_then(|rects| rects.iter().find(|rect| rect["key"] == rect_key))
            .expect("used rect geometry");
        assert_ne!(rect["x"], json!(0));
        assert_eq!(rect["width"], json!(60));
        assert_eq!(rect["height"], json!(108));
    }

    #[test]
    fn rgb_to_rgba_expands_with_alpha() {
        assert_eq!(
            rgb_to_rgba(&[1, 2, 3, 4, 5, 6]),
            vec![1, 2, 3, 255, 4, 5, 6, 255]
        );
    }
}
