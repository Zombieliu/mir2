//! Source-only item geometry from the approved APK pack. No identity inference,
//! gameplay mutation, Android filesystem path, PNG scaling or fake offsets.
use mir2_client_bevy::native_inventory_ingress::{NativeItemFrameGeometry, NativeItemLibrary};
use serde::Deserialize;
use std::collections::BTreeMap;

const MAX_META_BYTES: usize = 2 * 1024 * 1024;
const MAX_FRAME_COUNT: usize = 65_536;

fn source_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Deserialize)]
struct Metadata {
    version: u32,
    count: u32,
    frames: Vec<Frame>,
}

#[derive(Deserialize)]
struct Frame {
    index: u16,
    width: u16,
    height: u16,
    x: i32,
    y: i32,
    path: String,
}

#[derive(Default)]
pub(crate) struct AndroidItemGeometry {
    items: BTreeMap<u16, NativeItemFrameGeometry>,
    state_items: BTreeMap<u16, NativeItemFrameGeometry>,
}

fn parse_library(
    library: &str,
    bytes: &[u8],
) -> Result<BTreeMap<u16, NativeItemFrameGeometry>, &'static str> {
    if bytes.is_empty() || bytes.len() > MAX_META_BYTES {
        return Err("Invalid item geometry byte count");
    }
    let metadata: Metadata =
        serde_json::from_slice(bytes).map_err(|_| "Invalid item geometry JSON")?;
    if metadata.version != 3
        || metadata.count == 0
        || metadata.count as usize > MAX_FRAME_COUNT
        || metadata.frames.is_empty()
        || metadata.frames.len() > metadata.count as usize
    {
        return Err("Invalid item geometry schema");
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut frames = BTreeMap::new();
    for frame in metadata.frames {
        if u32::from(frame.index) >= metadata.count
            || !seen.insert(frame.index)
            || frame.width > 2048
            || frame.height > 2048
            || !(-8192..=8192).contains(&frame.x)
            || !(-8192..=8192).contains(&frame.y)
            || frame.path != format!("/original-ui/{library}/{}.png", frame.index)
        {
            return Err("Invalid item geometry frame");
        }
        // Match the Windows host: zero-size source frames have no draw geometry.
        // Items uses the full bitmap size but deliberately ignores library x/y.
        if frame.width > 0 && frame.height > 0 {
            frames.insert(
                frame.index,
                NativeItemFrameGeometry {
                    width: frame.width,
                    height: frame.height,
                    x: if library == "Items" { 0 } else { frame.x },
                    y: if library == "Items" { 0 } else { frame.y },
                },
            );
        }
    }
    Ok(frames)
}

impl AndroidItemGeometry {
    pub(crate) fn load(
        mut read: impl FnMut(&str, usize) -> Result<Vec<u8>, &'static str>,
    ) -> Result<Self, &'static str> {
        // Only these two literal APK paths may be read, never a server-supplied path.
        let items = read("original-ui/Items/meta.json", MAX_META_BYTES)?;
        let state_items = read("original-ui/StateItem/meta.json", MAX_META_BYTES)?;
        Ok(Self {
            items: parse_library("Items", &items)?,
            state_items: parse_library("StateItem", &state_items)?,
        })
    }

    pub(crate) fn frame(
        &self,
        library: NativeItemLibrary,
        index: u16,
    ) -> Option<NativeItemFrameGeometry> {
        match library {
            NativeItemLibrary::Items => self.items.get(&index),
            NativeItemLibrary::StateItem => self.state_items.get(&index),
        }
        .copied()
    }
}

#[cfg(target_os = "android")]
pub(crate) fn packaged() -> Result<&'static AndroidItemGeometry, &'static str> {
    static CACHE: std::sync::OnceLock<Result<AndroidItemGeometry, &'static str>> =
        std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            let mut identities = Vec::new();
            let geometry = AndroidItemGeometry::load(|path, max| {
                let bytes = crate::world_assets::read_packaged_asset(path, max)
                    .map_err(|_| "Packaged item geometry unavailable")?;
                identities.push((path.to_owned(), source_sha256(&bytes)));
                Ok(bytes)
            })?;
            bevy::log::info!(
                items = geometry.items.len(),
                state_items = geometry.state_items.len(),
                sources = ?identities,
                "ANDROID_ITEM_GEOMETRY_READY"
            );
            Ok(geometry)
        })
        .as_ref()
        .map_err(|error| *error)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn metadata(library: &str) -> Value {
        json!({"version":3,"count":5380,"frames":[
            {"index":7,"width":36,"height":26,"x":-347,"y":3,
             "path":format!("/original-ui/{library}/7.png")},
            {"index":30,"width":28,"height":57,"x":75,"y":186,
             "path":format!("/original-ui/{library}/30.png")},
            {"index":42,"width":0,"height":0,"x":0,"y":0,
             "path":format!("/original-ui/{library}/42.png")} ]})
    }

    pub(crate) fn fixture() -> AndroidItemGeometry {
        AndroidItemGeometry::load(|path, max| {
            assert_eq!(max, MAX_META_BYTES);
            Ok(metadata(if path.contains("/Items/") {
                "Items"
            } else {
                "StateItem"
            })
            .to_string()
            .into_bytes())
        })
        .unwrap()
    }

    #[test]
    fn source_identity_is_lowercase_sha256_bytes() {
        assert_eq!(
            source_sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sparse_catalogue_is_not_dense_count_and_items_ignore_library_offsets() {
        let pack = fixture();
        assert_eq!(
            pack.frame(NativeItemLibrary::Items, 7),
            Some(NativeItemFrameGeometry {
                width: 36,
                height: 26,
                x: 0,
                y: 0
            })
        );
        assert_eq!(pack.frame(NativeItemLibrary::Items, 8), None);
    }

    #[test]
    fn state_items_retain_original_equipment_offsets_and_empty_frames_do_not_draw() {
        let pack = fixture();
        assert_eq!(
            pack.frame(NativeItemLibrary::StateItem, 30),
            Some(NativeItemFrameGeometry {
                width: 28,
                height: 57,
                x: 75,
                y: 186
            })
        );
        assert_eq!(pack.frame(NativeItemLibrary::StateItem, 42), None);
    }

    #[test]
    fn both_literal_sources_are_required_and_bytes_are_bounded() {
        let mut paths = Vec::new();
        assert!(AndroidItemGeometry::load(|path, _| {
            paths.push(path.to_owned());
            if path.contains("/Items/") {
                Ok(metadata("Items").to_string().into_bytes())
            } else {
                Err("Missing StateItem metadata")
            }
        })
        .is_err());
        assert_eq!(
            paths,
            [
                "original-ui/Items/meta.json",
                "original-ui/StateItem/meta.json"
            ]
        );
        assert!(parse_library("Items", &[]).is_err());
        assert!(parse_library("Items", &vec![b' '; MAX_META_BYTES + 1]).is_err());
    }

    #[test]
    fn duplicate_indices_are_rejected_even_when_the_bitmap_is_empty() {
        let mut meta = metadata("StateItem");
        let duplicate = meta["frames"][2].clone();
        meta["frames"].as_array_mut().unwrap().push(duplicate);
        assert!(parse_library("StateItem", meta.to_string().as_bytes()).is_err());
    }

    #[test]
    fn wrong_library_path_index_and_unsafe_paths_are_rejected() {
        for path in [
            "/original-ui/StateItem/7.png",
            "/original-ui/Items/8.png",
            "../../7.png",
            "https://example.invalid/7.png",
        ] {
            let mut meta = metadata("Items");
            meta["frames"][0]["path"] = json!(path);
            assert!(parse_library("Items", meta.to_string().as_bytes()).is_err());
        }
        let mut meta = metadata("Items");
        meta["count"] = json!(7);
        assert!(parse_library("Items", meta.to_string().as_bytes()).is_err());
    }

    #[test]
    fn invalid_sizes_offsets_and_schema_are_rejected_without_partial_library() {
        for (field, value) in [
            ("width", json!(-1)),
            ("height", json!(2049)),
            ("index", json!(1.5)),
            ("x", json!("75")),
            ("y", json!(8193)),
        ] {
            let mut meta = metadata("Items");
            meta["frames"][0][field] = value;
            assert!(parse_library("Items", meta.to_string().as_bytes()).is_err());
        }
        for (field, value) in [
            ("version", json!(2)),
            ("count", json!(0)),
            ("frames", json!([])),
        ] {
            let mut meta = metadata("Items");
            meta[field] = value;
            assert!(parse_library("Items", meta.to_string().as_bytes()).is_err());
        }
    }
}
