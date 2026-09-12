//! Strict loader for the immutable standalone Crystal map-object pack.

use crate::world_assets::WorldAssetError;
use serde::Deserialize;
use std::{collections::HashMap, io::Cursor};

pub(crate) const MAP_OBJECT_MANIFEST_ASSET: &str = "generated/native-map-keyed/manifest.json";
const MAP_OBJECT_KIND: &str = "mir2-native-map-keyed-manifest";
const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_ENTRY_COUNT: usize = 100_000;
const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_IMAGE_PIXELS: usize = 4096 * 4096;
const MAX_SELECTED_RGBA_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    kind: String,
    map_file_names: Vec<String>,
    entries: Vec<ManifestEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestEntry {
    key: String,
    image_url: String,
    width: u32,
    height: u32,
    #[serde(default)]
    placement_mode: Option<String>,
    #[serde(default)]
    offset_x: Option<i32>,
    #[serde(default)]
    offset_y: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MapObjectEntry {
    pub(crate) image_key: String,
    pub(crate) asset_path: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) offset: Option<(i32, i32)>,
}

#[derive(Debug)]
pub(crate) struct MapObjectPack {
    pub(crate) entries: HashMap<String, MapObjectEntry>,
}

#[derive(Debug)]
pub(crate) struct DecodedMapObjectImage {
    pub(crate) key: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

fn safe_map_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn safe_entry_key(value: &str) -> bool {
    let Some((library, frame)) = value.rsplit_once('#') else {
        return false;
    };
    !library.is_empty()
        && library.len() <= 192
        && library.split('/').all(|part| {
            !part.is_empty()
                && !matches!(part, "." | "..")
                && part.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b' ' | b'.')
                })
        })
        && frame.parse::<u32>().is_ok()
}

fn normalize_image_path(url: &str) -> Result<(String, String), WorldAssetError> {
    let Some(file) = url.strip_prefix("/generated/native-map-keyed/pages/") else {
        return Err(WorldAssetError::new(
            "map-object image URL is outside its packaged root",
        ));
    };
    let Some(hash) = file.strip_suffix(".png") else {
        return Err(WorldAssetError::new("map-object image is not a PNG"));
    };
    if hash.len() != 64
        || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        || file.contains(['/', '\\', '\0'])
    {
        return Err(WorldAssetError::new(
            "map-object image URL is not content-addressed",
        ));
    }
    Ok((
        format!("native-map-keyed:{hash}"),
        format!("generated/native-map-keyed/pages/{file}"),
    ))
}

pub(crate) fn load_map_object_pack<F>(
    map_file_name: &str,
    mut read_asset: F,
) -> Result<MapObjectPack, WorldAssetError>
where
    F: FnMut(&str, usize) -> Result<Vec<u8>, WorldAssetError>,
{
    let bytes = read_asset(MAP_OBJECT_MANIFEST_ASSET, MAX_MANIFEST_BYTES)?;
    if bytes.is_empty() || bytes.len() > MAX_MANIFEST_BYTES {
        return Err(WorldAssetError::new(
            "map-object manifest byte count is out of bounds",
        ));
    }
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|error| WorldAssetError::new(format!("map-object manifest rejected: {error}")))?;
    if manifest.schema_version != 1
        || manifest.kind != MAP_OBJECT_KIND
        || manifest.entries.len() > MAX_ENTRY_COUNT
        || !safe_map_name(map_file_name)
        || !manifest
            .map_file_names
            .iter()
            .any(|name| name == map_file_name)
    {
        return Err(WorldAssetError::new(
            "unsupported or mismatched map-object manifest",
        ));
    }
    let mut entries = HashMap::with_capacity(manifest.entries.len());
    for entry in manifest.entries {
        if !safe_entry_key(&entry.key)
            || entry.width == 0
            || entry.height == 0
            || (entry.width as usize)
                .checked_mul(entry.height as usize)
                .is_none_or(|pixels| pixels > MAX_IMAGE_PIXELS)
        {
            return Err(WorldAssetError::new("map-object manifest entry is invalid"));
        }
        let offset = match entry.placement_mode.as_deref() {
            None => {
                if entry.offset_x.is_some() || entry.offset_y.is_some() {
                    return Err(WorldAssetError::new(
                        "map-object offset lacks a placement mode",
                    ));
                }
                None
            }
            Some("source-offset") => Some((
                entry
                    .offset_x
                    .ok_or_else(|| WorldAssetError::new("map-object offsetX is missing"))?,
                entry
                    .offset_y
                    .ok_or_else(|| WorldAssetError::new("map-object offsetY is missing"))?,
            )),
            Some(_) => {
                return Err(WorldAssetError::new(
                    "map-object placement mode is unsupported",
                ));
            }
        };
        let (image_key, asset_path) = normalize_image_path(&entry.image_url)?;
        if entries
            .insert(
                entry.key,
                MapObjectEntry {
                    image_key,
                    asset_path,
                    width: entry.width,
                    height: entry.height,
                    offset,
                },
            )
            .is_some()
        {
            return Err(WorldAssetError::new(
                "map-object manifest entry key is duplicated",
            ));
        }
    }
    Ok(MapObjectPack { entries })
}

fn decode_png(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>, WorldAssetError> {
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| WorldAssetError::new("map-object dimensions overflow"))?;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| {
        WorldAssetError::new(format!("map-object PNG header rejected: {error}"))
    })?;
    if reader.info().animation_control.is_some()
        || reader.info().width != width
        || reader.info().height != height
    {
        return Err(WorldAssetError::new(
            "map-object PNG dimensions or animation do not match the manifest",
        ));
    }
    let output_size = reader
        .output_buffer_size()
        .ok_or_else(|| WorldAssetError::new("map-object decoded size is unavailable"))?;
    let expected = pixels
        .checked_mul(4)
        .ok_or_else(|| WorldAssetError::new("map-object RGBA size overflows"))?;
    if output_size == 0 || output_size > expected {
        return Err(WorldAssetError::new(
            "map-object decoded size exceeds its budget",
        ));
    }
    let mut output = vec![0; output_size];
    let frame = reader
        .next_frame(&mut output)
        .map_err(|error| WorldAssetError::new(format!("map-object PNG data rejected: {error}")))?;
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
                "map-object indexed PNG was not expanded",
            ));
        }
    };
    if rgba.len() != expected {
        return Err(WorldAssetError::new(
            "map-object RGBA byte count is invalid",
        ));
    }
    Ok(rgba)
}

pub(crate) fn decode_selected_map_objects<F>(
    pack: &MapObjectPack,
    keys: &[String],
    mut read_asset: F,
) -> Result<Vec<DecodedMapObjectImage>, WorldAssetError>
where
    F: FnMut(&str, usize) -> Result<Vec<u8>, WorldAssetError>,
{
    let mut by_image = HashMap::new();
    for key in keys {
        let entry = pack
            .entries
            .get(key)
            .ok_or_else(|| WorldAssetError::new("selected map-object entry is missing"))?;
        by_image
            .entry(entry.image_key.clone())
            .or_insert_with(|| entry.clone());
    }
    let mut decoded = Vec::with_capacity(by_image.len());
    let mut total_rgba = 0usize;
    for (image_key, entry) in by_image {
        let bytes = read_asset(&entry.asset_path, MAX_IMAGE_BYTES)?;
        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
            return Err(WorldAssetError::new(
                "map-object PNG byte count is out of bounds",
            ));
        }
        let rgba = decode_png(&bytes, entry.width, entry.height)?;
        total_rgba = total_rgba
            .checked_add(rgba.len())
            .ok_or_else(|| WorldAssetError::new("map-object RGBA total overflows"))?;
        if total_rgba > MAX_SELECTED_RGBA_BYTES {
            return Err(WorldAssetError::new(
                "selected map objects exceed the memory budget",
            ));
        }
        decoded.push(DecodedMapObjectImage {
            key: image_key,
            width: entry.width,
            height: entry.height,
            rgba,
        });
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_map_scoped_and_content_addressed() {
        let hash = "a".repeat(64);
        let json = serde_json::json!({
            "schemaVersion":1,"kind":MAP_OBJECT_KIND,"mapFileName":"0","mapFileNames":["0"],
            "entries":[{"key":"WemadeMir2/Objects#7","imageUrl":format!("/generated/native-map-keyed/pages/{hash}.png"),"width":48,"height":96}]
        }).to_string().into_bytes();
        let pack = load_map_object_pack("0", |path, _| {
            assert_eq!(path, MAP_OBJECT_MANIFEST_ASSET);
            Ok(json.clone())
        })
        .unwrap();
        assert_eq!(pack.entries.len(), 1);
        assert_eq!(
            pack.entries["WemadeMir2/Objects#7"].image_key,
            format!("native-map-keyed:{hash}")
        );
        assert!(load_map_object_pack("1", |_, _| Ok(json.clone())).is_err());
    }
}
