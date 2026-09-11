//! Strict loader for the generated map-atlas pack embedded in Android assets.
//!
//! The licensed source files and generated pack stay outside Git. Gradle may
//! stage an approved `generated/map-atlas` directory into the APK; this module
//! validates the compact manifest, decodes the PNG pages off the Bevy render
//! thread, and sends raw RGBA pages through the bounded native ingress queue.

use serde::Deserialize;
use std::{collections::HashSet, fmt, io::Cursor};

pub(crate) const MAP_ATLAS_MANIFEST_ASSET: &str = "generated/map-atlas/manifest.json";
const MAP_ATLAS_KIND: &str = "mir2-map-atlas-manifest";
const MAP_ATLAS_SCHEMA_VERSION: u32 = 2;
const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_PAGE_COUNT: usize = 128;
const MAX_PAGE_PNG_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOTAL_PNG_BYTES: usize = 64 * 1024 * 1024;
const MAX_PAGE_PIXELS: usize = 4 * 1024 * 1024;
const MAX_TOTAL_PIXELS: usize = 24 * 1024 * 1024;
const MAX_RECT_COUNT: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorldAssetError(String);

impl WorldAssetError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for WorldAssetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactManifest {
    schema_version: u32,
    kind: String,
    pages: Vec<CompactPage>,
    #[serde(default)]
    stats: Option<CompactStats>,
}

#[derive(Debug, Deserialize)]
struct CompactPage {
    l: String,
    p: u32,
    w: u32,
    h: u32,
    b: u64,
    u: String,
    #[serde(default)]
    r: Vec<[u32; 5]>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactStats {
    library_count: u64,
    atlas_page_count: u64,
    source_count: u64,
    image_bytes: u64,
    max_page_bytes: u64,
    max_page_pixels: u64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MapAtlasRectDescriptor {
    pub(crate) key: String,
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MapAtlasPageDescriptor {
    pub(crate) key: String,
    pub(crate) asset_path: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) declared_png_bytes: usize,
    pub(crate) rects: Vec<MapAtlasRectDescriptor>,
}

#[derive(Debug)]
pub(crate) struct DecodedMapAtlasPage {
    pub(crate) key: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct PackagedMapAtlasBundle {
    pub(crate) descriptors: Vec<MapAtlasPageDescriptor>,
    pub(crate) pages: Vec<DecodedMapAtlasPage>,
    pub(crate) source_count: usize,
    pub(crate) compressed_bytes: usize,
    pub(crate) rgba_bytes: usize,
}

#[cfg(target_os = "android")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagedMapAtlasSummary {
    pub(crate) page_count: usize,
    pub(crate) source_count: usize,
    pub(crate) compressed_bytes: usize,
    pub(crate) rgba_bytes: usize,
    pub(crate) map_object_rgba_bytes: usize,
    pub(crate) map_width: u16,
    pub(crate) map_height: u16,
    pub(crate) map_atlas_count: usize,
    pub(crate) map_tile_count: usize,
    pub(crate) map_standalone_tile_count: usize,
    pub(crate) unresolved_draw_count: usize,
    pub(crate) entity_page_count: usize,
    pub(crate) entity_count: usize,
    pub(crate) entity_layer_count: usize,
    pub(crate) unresolved_entity_count: usize,
    pub(crate) entity_unindexed_rect_count: usize,
    pub(crate) entity_compressed_bytes: usize,
    pub(crate) entity_rgba_bytes: usize,
}

#[cfg(target_os = "android")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackagedMapAtlasLoadEvent {
    Ready(PackagedMapAtlasSummary),
    Failed(String),
}

fn safe_library_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.split('/').all(|component| {
            !component.is_empty()
                && !matches!(component, "." | "..")
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
}

fn normalize_page_asset_path(url: &str) -> Result<String, WorldAssetError> {
    let Some(path) = url.strip_prefix("/generated/map-atlas/") else {
        return Err(WorldAssetError::new(
            "atlas page URL is outside generated/map-atlas",
        ));
    };
    if path.is_empty()
        || !path.ends_with(".png")
        || path.contains("..")
        || path.contains('\\')
        || path.contains(['?', '#', '\0'])
        || !path.is_ascii()
    {
        return Err(WorldAssetError::new(
            "atlas page URL is not a safe PNG asset path",
        ));
    }
    Ok(format!("generated/map-atlas/{path}"))
}

fn validate_manifest(
    manifest: CompactManifest,
) -> Result<(Vec<MapAtlasPageDescriptor>, usize, usize), WorldAssetError> {
    if manifest.schema_version != MAP_ATLAS_SCHEMA_VERSION || manifest.kind != MAP_ATLAS_KIND {
        return Err(WorldAssetError::new(
            "unsupported map-atlas manifest schema",
        ));
    }
    if manifest.pages.is_empty() || manifest.pages.len() > MAX_PAGE_COUNT {
        return Err(WorldAssetError::new(
            "map-atlas page count is out of bounds",
        ));
    }

    let mut page_keys = HashSet::new();
    let mut asset_paths = HashSet::new();
    let mut rect_keys = HashSet::new();
    let mut libraries = HashSet::new();
    let mut total_png_bytes = 0usize;
    let mut total_pixels = 0usize;
    let mut total_rects = 0usize;
    let mut max_page_bytes = 0usize;
    let mut max_page_pixels = 0usize;
    let mut descriptors = Vec::with_capacity(manifest.pages.len());

    for page in manifest.pages {
        if !safe_library_name(&page.l) {
            return Err(WorldAssetError::new("map-atlas library key is invalid"));
        }
        if page.w == 0 || page.h == 0 {
            return Err(WorldAssetError::new(
                "map-atlas page dimensions must be non-zero",
            ));
        }
        let page_pixels = (page.w as usize)
            .checked_mul(page.h as usize)
            .ok_or_else(|| WorldAssetError::new("map-atlas page dimensions overflow"))?;
        if page_pixels > MAX_PAGE_PIXELS {
            return Err(WorldAssetError::new(
                "map-atlas page pixel count exceeds the limit",
            ));
        }
        let declared_png_bytes = usize::try_from(page.b)
            .map_err(|_| WorldAssetError::new("map-atlas page byte count overflows"))?;
        if declared_png_bytes == 0 || declared_png_bytes > MAX_PAGE_PNG_BYTES {
            return Err(WorldAssetError::new(
                "map-atlas page byte count is out of bounds",
            ));
        }
        total_png_bytes = total_png_bytes
            .checked_add(declared_png_bytes)
            .ok_or_else(|| WorldAssetError::new("map-atlas total byte count overflows"))?;
        total_pixels = total_pixels
            .checked_add(page_pixels)
            .ok_or_else(|| WorldAssetError::new("map-atlas total pixel count overflows"))?;
        if total_png_bytes > MAX_TOTAL_PNG_BYTES || total_pixels > MAX_TOTAL_PIXELS {
            return Err(WorldAssetError::new(
                "map-atlas pack exceeds the memory budget",
            ));
        }

        let key = format!("map:{}#p{}", page.l, page.p);
        let asset_path = normalize_page_asset_path(&page.u)?;
        if !page_keys.insert(key.clone()) || !asset_paths.insert(asset_path.clone()) {
            return Err(WorldAssetError::new(
                "map-atlas page key or asset path is duplicated",
            ));
        }
        libraries.insert(page.l.clone());

        total_rects = total_rects
            .checked_add(page.r.len())
            .ok_or_else(|| WorldAssetError::new("map-atlas rect count overflows"))?;
        if total_rects > MAX_RECT_COUNT {
            return Err(WorldAssetError::new(
                "map-atlas rect count exceeds the limit",
            ));
        }
        let mut rects = Vec::with_capacity(page.r.len());
        for [frame, x, y, width, height] in page.r {
            if width == 0
                || height == 0
                || x.checked_add(width).is_none_or(|right| right > page.w)
                || y.checked_add(height).is_none_or(|bottom| bottom > page.h)
            {
                return Err(WorldAssetError::new("map-atlas rect is outside its page"));
            }
            let rect_key = format!("{}#{frame}", page.l);
            if !rect_keys.insert(rect_key.clone()) {
                return Err(WorldAssetError::new("map-atlas rect key is duplicated"));
            }
            rects.push(MapAtlasRectDescriptor {
                key: rect_key,
                x,
                y,
                width,
                height,
            });
        }

        max_page_bytes = max_page_bytes.max(declared_png_bytes);
        max_page_pixels = max_page_pixels.max(page_pixels);
        descriptors.push(MapAtlasPageDescriptor {
            key,
            asset_path,
            width: page.w,
            height: page.h,
            declared_png_bytes,
            rects,
        });
    }

    if let Some(stats) = manifest.stats {
        let expected = [
            (stats.library_count, libraries.len() as u64),
            (stats.atlas_page_count, descriptors.len() as u64),
            (stats.source_count, total_rects as u64),
            (stats.image_bytes, total_png_bytes as u64),
            (stats.max_page_bytes, max_page_bytes as u64),
            (stats.max_page_pixels, max_page_pixels as u64),
        ];
        if expected.iter().any(|(declared, actual)| declared != actual) {
            return Err(WorldAssetError::new(
                "map-atlas manifest stats do not match its pages",
            ));
        }
    }

    Ok((descriptors, total_rects, total_png_bytes))
}

fn decode_png(
    bytes: &[u8],
    descriptor: &MapAtlasPageDescriptor,
) -> Result<Vec<u8>, WorldAssetError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| WorldAssetError::new(format!("map-atlas PNG header rejected: {error}")))?;
    if reader.info().animation_control.is_some() {
        return Err(WorldAssetError::new("animated PNG pages are not supported"));
    }
    if reader.info().width != descriptor.width || reader.info().height != descriptor.height {
        return Err(WorldAssetError::new(
            "map-atlas PNG dimensions do not match the manifest",
        ));
    }
    let output_size = reader
        .output_buffer_size()
        .ok_or_else(|| WorldAssetError::new("map-atlas decoded size is unavailable"))?;
    let max_rgba_bytes = (descriptor.width as usize)
        .checked_mul(descriptor.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| WorldAssetError::new("map-atlas decoded size overflows"))?;
    if output_size == 0 || output_size > max_rgba_bytes {
        return Err(WorldAssetError::new(
            "map-atlas decoded size exceeds the page budget",
        ));
    }
    let mut decoded = vec![0; output_size];
    let frame = reader
        .next_frame(&mut decoded)
        .map_err(|error| WorldAssetError::new(format!("map-atlas PNG data rejected: {error}")))?;
    if frame.width != descriptor.width
        || frame.height != descriptor.height
        || frame.bit_depth != png::BitDepth::Eight
    {
        return Err(WorldAssetError::new(
            "map-atlas PNG output format is invalid",
        ));
    }
    let source = &decoded[..frame.buffer_size()];
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
            return Err(WorldAssetError::new("indexed PNG was not expanded"));
        }
    };
    if rgba.len() != max_rgba_bytes {
        return Err(WorldAssetError::new("map-atlas RGBA byte count is invalid"));
    }
    Ok(rgba)
}

pub(crate) fn load_map_atlas_bundle<F>(
    mut read_asset: F,
) -> Result<PackagedMapAtlasBundle, WorldAssetError>
where
    F: FnMut(&str, usize) -> Result<Vec<u8>, WorldAssetError>,
{
    let manifest_bytes = read_asset(MAP_ATLAS_MANIFEST_ASSET, MAX_MANIFEST_BYTES)?;
    if manifest_bytes.is_empty() || manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err(WorldAssetError::new(
            "map-atlas manifest byte count is out of bounds",
        ));
    }
    let manifest: CompactManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| WorldAssetError::new(format!("map-atlas manifest rejected: {error}")))?;
    let (descriptors, source_count, compressed_bytes) = validate_manifest(manifest)?;

    // Decode every page successfully before publishing any page to Bevy. This
    // avoids exposing a half-validated asset pack to the render runtime.
    let mut pages = Vec::with_capacity(descriptors.len());
    let mut rgba_bytes = 0usize;
    for descriptor in &descriptors {
        let bytes = read_asset(&descriptor.asset_path, descriptor.declared_png_bytes)?;
        if bytes.len() != descriptor.declared_png_bytes {
            return Err(WorldAssetError::new(
                "map-atlas PNG byte count does not match the manifest",
            ));
        }
        let rgba = decode_png(&bytes, descriptor)?;
        rgba_bytes = rgba_bytes
            .checked_add(rgba.len())
            .ok_or_else(|| WorldAssetError::new("map-atlas RGBA total overflows"))?;
        pages.push(DecodedMapAtlasPage {
            key: descriptor.key.clone(),
            width: descriptor.width,
            height: descriptor.height,
            rgba,
        });
    }

    Ok(PackagedMapAtlasBundle {
        descriptors,
        pages,
        source_count,
        compressed_bytes,
        rgba_bytes,
    })
}

#[cfg(target_os = "android")]
#[derive(Default)]
struct AndroidMapAtlasLoadState {
    active: bool,
    generation: u64,
    event: Option<PackagedMapAtlasLoadEvent>,
}

#[cfg(target_os = "android")]
static ANDROID_MAP_ATLAS_LOAD: std::sync::Mutex<AndroidMapAtlasLoadState> =
    std::sync::Mutex::new(AndroidMapAtlasLoadState {
        active: false,
        generation: 0,
        event: None,
    });

#[cfg(target_os = "android")]
fn render_load_is_current(generation: u64) -> bool {
    let state = ANDROID_MAP_ATLAS_LOAD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.active && state.generation == generation
}

#[cfg(target_os = "android")]
fn wait_for_native_render_queue(generation: u64) -> bool {
    const LOW_WATER_BYTES: usize = 16 * 1024 * 1024;
    const POLLS: usize = 1_000;
    for _ in 0..POLLS {
        if !render_load_is_current(generation) {
            return false;
        }
        match mir2_bevy_runtime::native_ingest::native_pending_buffer_bytes() {
            Some(bytes) if bytes <= LOW_WATER_BYTES => return true,
            Some(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
            None => return false,
        }
    }
    false
}

#[cfg(target_os = "android")]
fn read_packaged_asset(path: &str, max_bytes: usize) -> Result<Vec<u8>, WorldAssetError> {
    use std::{ffi::CString, io::Read};

    let app = bevy::android::ANDROID_APP
        .get()
        .ok_or_else(|| WorldAssetError::new("Android asset manager is not initialized"))?;
    let asset_name = CString::new(path)
        .map_err(|_| WorldAssetError::new("Android asset name contains a NUL byte"))?;
    let mut asset = app.asset_manager().open(&asset_name).ok_or_else(|| {
        WorldAssetError::new(format!("packaged Android asset is missing: {path}"))
    })?;
    if asset.length() == 0 || asset.length() > max_bytes {
        return Err(WorldAssetError::new(format!(
            "packaged Android asset has an invalid byte count: {path}"
        )));
    }
    let mut bytes = Vec::with_capacity(asset.length());
    asset.read_to_end(&mut bytes).map_err(|error| {
        WorldAssetError::new(format!("packaged Android asset could not be read: {error}"))
    })?;
    if bytes.len() > max_bytes {
        return Err(WorldAssetError::new(
            "packaged Android asset exceeds its byte limit",
        ));
    }
    Ok(bytes)
}

/// Starts one background load for the current scene/session. A later reset may
/// request another load; immutable atlas keys safely replace their old images.
#[cfg(target_os = "android")]
pub(crate) fn request_packaged_map_atlas_load(
    scene: crate::world_projection::ProjectedScene,
    world_snapshot: String,
    request_id: u64,
) -> bool {
    let mut state = ANDROID_MAP_ATLAS_LOAD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if state.active {
        return false;
    }
    state.generation = state.generation.wrapping_add(1).max(1);
    let generation = state.generation;
    state.active = true;
    state.event = None;
    drop(state);

    std::thread::spawn(move || {
        let loaded = load_map_atlas_bundle(read_packaged_asset).and_then(|bundle| {
            let map_name = scene
                .map_file_name
                .strip_suffix(".map.gz")
                .or_else(|| scene.map_file_name.strip_suffix(".map"))
                .unwrap_or(&scene.map_file_name)
                .to_ascii_lowercase();
            let map_objects =
                crate::map_objects::load_map_object_pack(&map_name, read_packaged_asset)?;
            let map_render = crate::map_render::load_map_render_state(
                &scene,
                &bundle.descriptors,
                &map_objects,
                request_id,
                read_packaged_asset,
            )?;
            let map_object_images = crate::map_objects::decode_selected_map_objects(
                &map_objects,
                &map_render.standalone_source_keys,
                read_packaged_asset,
            )?;
            let entity_render = crate::entity_render::load_entity_render_state(
                &world_snapshot,
                &scene,
                request_id,
                read_packaged_asset,
            )?;
            Ok((bundle, map_render, map_object_images, entity_render))
        });
        let event = match loaded {
            Ok((mut bundle, map_render, map_object_images, entity_render)) => {
                let active_map_atlases = map_render
                    .atlas_keys
                    .iter()
                    .map(String::as_str)
                    .collect::<std::collections::HashSet<_>>();
                bundle
                    .pages
                    .retain(|page| active_map_atlases.contains(page.key.as_str()));
                drop(active_map_atlases);
                let active_map_rgba_bytes = bundle
                    .pages
                    .iter()
                    .map(|page| page.rgba.len())
                    .sum::<usize>();
                let map_object_rgba_bytes = map_object_images
                    .iter()
                    .map(|image| image.rgba.len())
                    .sum::<usize>();
                let summary = PackagedMapAtlasSummary {
                    page_count: bundle.pages.len(),
                    source_count: bundle.source_count,
                    compressed_bytes: bundle.compressed_bytes,
                    rgba_bytes: active_map_rgba_bytes,
                    map_object_rgba_bytes,
                    map_width: map_render.map_width,
                    map_height: map_render.map_height,
                    map_atlas_count: map_render.atlas_count,
                    map_tile_count: map_render.tile_count,
                    map_standalone_tile_count: map_render.standalone_tile_count,
                    unresolved_draw_count: map_render.unresolved_draw_count,
                    entity_page_count: entity_render.pages.len(),
                    entity_count: entity_render.entity_count,
                    entity_layer_count: entity_render.layer_count,
                    unresolved_entity_count: entity_render.unresolved_entity_count,
                    entity_unindexed_rect_count: entity_render.unindexed_rect_count,
                    entity_compressed_bytes: entity_render.compressed_bytes,
                    entity_rgba_bytes: entity_render.rgba_bytes,
                };
                // Publish the render-state pair before its image batch. The
                // runtime deliberately clears map images while no active map
                // state exists; sending pages first therefore lets a fast
                // render frame ingest and immediately discard them while the
                // Android producer is still decoding/queueing later objects.
                // An active state is safe to expose early because both the map
                // and entity sync paths wait for every referenced image before
                // committing any sprites or a render-ready receipt.
                let state = ANDROID_MAP_ATLAS_LOAD
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if state.generation != generation {
                    return;
                }
                let live_render_accepted = crate::live_entity::install_render(
                    &entity_render.json,
                    &entity_render.live_directions_json,
                );
                let states_accepted = live_render_accepted
                    && mir2_bevy_runtime::native_ingest::push_native_map_render_state(
                        map_render.json,
                    )
                    && mir2_bevy_runtime::native_ingest::push_native_entity_render_state(
                        entity_render.json,
                    );
                drop(state);

                let images_accepted = states_accepted
                    && bundle.pages.into_iter().all(|page| {
                        wait_for_native_render_queue(generation)
                            && mir2_bevy_runtime::native_ingest::push_native_map_render_atlas(
                                page.key,
                                page.width,
                                page.height,
                                page.rgba,
                            )
                    })
                    && map_object_images.into_iter().all(|image| {
                        wait_for_native_render_queue(generation)
                            && mir2_bevy_runtime::native_ingest::push_native_map_render_atlas(
                                image.key,
                                image.width,
                                image.height,
                                image.rgba,
                            )
                    })
                    && entity_render.pages.into_iter().all(|page| {
                        wait_for_native_render_queue(generation)
                            && mir2_bevy_runtime::native_ingest::push_native_entity_render_atlas(
                                page.key,
                                page.width,
                                page.height,
                                page.rgba,
                            )
                    });
                let queue_ready = images_accepted && wait_for_native_render_queue(generation);
                let mut state = ANDROID_MAP_ATLAS_LOAD
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if state.generation != generation {
                    return;
                }
                if queue_ready {
                    state.active = false;
                    state.event = Some(PackagedMapAtlasLoadEvent::Ready(summary));
                    return;
                } else {
                    // This also removes already-consumed pages if the bounded
                    // queue ever rejects the tail of a pack.
                    crate::live_entity::clear_with_presentation_reset();
                    mir2_bevy_runtime::native_ingest::push_native_scene_reset();
                    state.active = false;
                    state.event = Some(PackagedMapAtlasLoadEvent::Failed(
                        "Bevy rejected the packaged map-atlas or draw-state batch".into(),
                    ));
                    return;
                }
            }
            Err(error) => PackagedMapAtlasLoadEvent::Failed(error.to_string()),
        };
        let mut state = ANDROID_MAP_ATLAS_LOAD
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.generation != generation {
            return;
        }
        state.active = false;
        state.event = Some(event);
    });
    true
}

#[cfg(target_os = "android")]
pub(crate) fn cancel_packaged_map_atlas_load() {
    crate::live_entity::clear_with_presentation_reset();
    let mut state = ANDROID_MAP_ATLAS_LOAD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.generation = state.generation.wrapping_add(1).max(1);
    state.active = false;
    state.event = None;
}

#[cfg(target_os = "android")]
pub(crate) fn poll_packaged_map_atlas_load() -> Option<PackagedMapAtlasLoadEvent> {
    ANDROID_MAP_ATLAS_LOAD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .event
        .take()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn rgba_png(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(pixels)
            .unwrap();
        bytes
    }

    fn fixture_manifest(png_bytes: usize) -> Vec<u8> {
        format!(
            r#"{{"schemaVersion":2,"kind":"mir2-map-atlas-manifest","pages":[{{"l":"WemadeMir2-Tiles","p":0,"w":2,"h":1,"b":{png_bytes},"u":"/generated/map-atlas/WemadeMir2-Tiles/p0.hash.png","r":[[7,0,0,1,1],[8,1,0,1,1]]}}],"stats":{{"libraryCount":1,"atlasPageCount":1,"sourceCount":2,"imageBytes":{png_bytes},"maxPageBytes":{png_bytes},"maxPagePixels":2}}}}"#
        )
        .into_bytes()
    }

    fn fixture_assets() -> HashMap<String, Vec<u8>> {
        let png = rgba_png(2, 1, &[255, 0, 0, 255, 0, 255, 0, 128]);
        HashMap::from([
            (MAP_ATLAS_MANIFEST_ASSET.into(), fixture_manifest(png.len())),
            (
                "generated/map-atlas/WemadeMir2-Tiles/p0.hash.png".into(),
                png,
            ),
        ])
    }

    fn load_fixture(
        assets: &HashMap<String, Vec<u8>>,
    ) -> Result<PackagedMapAtlasBundle, WorldAssetError> {
        load_map_atlas_bundle(|path, max_bytes| {
            let bytes = assets
                .get(path)
                .cloned()
                .ok_or_else(|| WorldAssetError::new(format!("missing fixture: {path}")))?;
            if bytes.len() > max_bytes {
                return Err(WorldAssetError::new("fixture exceeds requested byte limit"));
            }
            Ok(bytes)
        })
    }

    #[test]
    fn compact_pack_is_validated_and_decoded_to_rgba() {
        let bundle = load_fixture(&fixture_assets()).unwrap();
        assert_eq!(bundle.source_count, 2);
        assert_eq!(bundle.pages.len(), 1);
        assert_eq!(bundle.pages[0].key, "map:WemadeMir2-Tiles#p0");
        assert_eq!((bundle.pages[0].width, bundle.pages[0].height), (2, 1));
        assert_eq!(bundle.pages[0].rgba, [255, 0, 0, 255, 0, 255, 0, 128]);
        assert_eq!(bundle.descriptors[0].rects[1].key, "WemadeMir2-Tiles#8");
    }

    #[test]
    fn manifest_rejects_page_path_traversal_before_reading_png() {
        let mut assets = fixture_assets();
        let manifest = String::from_utf8(assets[MAP_ATLAS_MANIFEST_ASSET].clone())
            .unwrap()
            .replace(
                "/generated/map-atlas/WemadeMir2-Tiles/p0.hash.png",
                "/generated/map-atlas/../secret.png",
            );
        assets.insert(MAP_ATLAS_MANIFEST_ASSET.into(), manifest.into_bytes());
        let error = load_fixture(&assets).unwrap_err();
        assert!(error.to_string().contains("safe PNG asset path"));
    }

    #[test]
    fn manifest_rejects_stats_drift() {
        let mut assets = fixture_assets();
        let manifest = String::from_utf8(assets[MAP_ATLAS_MANIFEST_ASSET].clone())
            .unwrap()
            .replace("\"sourceCount\":2", "\"sourceCount\":3");
        assets.insert(MAP_ATLAS_MANIFEST_ASSET.into(), manifest.into_bytes());
        let error = load_fixture(&assets).unwrap_err();
        assert!(error.to_string().contains("stats do not match"));
    }

    #[test]
    fn png_dimensions_must_match_manifest() {
        let mut assets = fixture_assets();
        let png = rgba_png(1, 1, &[255, 255, 255, 255]);
        let old_size = assets["generated/map-atlas/WemadeMir2-Tiles/p0.hash.png"].len();
        assets.insert(
            "generated/map-atlas/WemadeMir2-Tiles/p0.hash.png".into(),
            png.clone(),
        );
        let manifest = String::from_utf8(assets[MAP_ATLAS_MANIFEST_ASSET].clone())
            .unwrap()
            .replace(&old_size.to_string(), &png.len().to_string());
        assets.insert(MAP_ATLAS_MANIFEST_ASSET.into(), manifest.into_bytes());
        let error = load_fixture(&assets).unwrap_err();
        assert!(error.to_string().contains("dimensions do not match"));
    }

    #[test]
    fn configured_external_pack_is_valid_when_present() {
        let Ok(root) = std::env::var("MIR2_ANDROID_WORLD_ASSET_ROOT") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let bundle = load_map_atlas_bundle(|path, max_bytes| {
            let bytes = std::fs::read(root.join(path)).map_err(|error| {
                WorldAssetError::new(format!("configured fixture could not be read: {error}"))
            })?;
            if bytes.is_empty() || bytes.len() > max_bytes {
                return Err(WorldAssetError::new(
                    "configured fixture has an invalid byte count",
                ));
            }
            Ok(bytes)
        })
        .unwrap();
        assert!(!bundle.pages.is_empty());
        assert_eq!(bundle.pages.len(), bundle.descriptors.len());
    }
}
