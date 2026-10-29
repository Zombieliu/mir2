//! One authenticated store backs native reads and Bevy. Only the signed launch
//! loader installs it; environment variables never mount unsigned catalogs.
use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, PathStream, Reader, VecReader,
};
use mir2_resource_pack::PackStore;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

static STORE: OnceLock<Arc<PackStore>> = OnceLock::new();
static FAILURE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
pub fn install(store: Arc<PackStore>) -> Result<(), String> {
    STORE
        .set(store)
        .map_err(|_| "a resource generation is already mounted".to_owned())
}
pub fn store() -> Option<&'static Arc<PackStore>> {
    STORE.get()
}
pub fn failure() -> Option<String> {
    FAILURE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}
fn record(error: String) -> String {
    let mut failure = FAILURE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if failure.is_none() {
        *failure = Some(error.clone());
    }
    error
}
pub fn logical_path(value: &str) -> Result<&str, String> {
    let value = value.trim_start_matches('/');
    mir2_resource_pack::validate_logical_path(value).map_err(|e| e.to_string())?;
    Ok(value)
}
pub fn read_from(store: &PackStore, path: &str) -> Result<Vec<u8>, String> {
    let key = logical_path(path)?;
    store
        .read(key)
        .map_err(|e| format!("packed resource {key}: {e:#}"))
}
pub fn read(path: &str) -> Result<Vec<u8>, String> {
    read_from(
        store().ok_or_else(|| "no packed generation mounted".to_owned())?,
        path,
    )
    .map_err(record)
}
/// Audio has source-authored optional choices (Main.wav or Login2.wav). An
/// absent catalog entry is only a probe; a declared entry that cannot be read
/// is a generation failure. Neither case invokes loose/network discovery.
pub fn read_if_present(path: &str) -> Result<Vec<u8>, String> {
    let store = store().ok_or_else(|| "no packed generation mounted".to_owned())?;
    let key = logical_path(path).map_err(record)?;
    if !store.exists(key) {
        return Err(format!("optional resource is not in catalog: {key}"));
    }
    read(path)
}
#[derive(Clone)]
pub struct PackedAssetReader {
    store: Arc<PackStore>,
}
impl PackedAssetReader {
    pub fn new(store: Arc<PackStore>) -> Self {
        Self { store }
    }
    fn error(&self, message: String) -> AssetReaderError {
        // Independent readers used by verification must not poison the mounted
        // generation. Only errors from its actual reader block readiness.
        let message = if store().is_some_and(|store| Arc::ptr_eq(store, &self.store)) {
            record(message)
        } else {
            message
        };
        AssetReaderError::Io(Arc::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )))
    }
}
impl AssetReader for PackedAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let key = path
            .to_str()
            .ok_or_else(|| self.error("non-UTF8 resource path".into()))?;
        Ok(VecReader::new(
            read_from(&self.store, key).map_err(|e| self.error(e))?,
        ))
    }
    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf()))
    }
    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        let key = directory_key(path).map_err(|e| self.error(e))?;
        let entries = self
            .store
            .list_prefix(key)
            .map_err(|e| self.error(e.to_string()))?;
        let prefix = if key.is_empty() {
            String::new()
        } else {
            format!("{key}/")
        };
        let mut children = std::collections::BTreeSet::new();
        for entry in entries {
            if let Some(rest) = entry.strip_prefix(&prefix) {
                if let Some(first) = rest.split('/').next().filter(|part| !part.is_empty()) {
                    children.insert(PathBuf::from(format!("{prefix}{first}")));
                }
            }
        }
        Ok(Box::new(futures_util::stream::iter(children)))
    }
    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        let key = directory_key(path).map_err(|e| self.error(e))?;
        Ok(key.is_empty() || self.store.is_directory(key))
    }
}
fn directory_key(path: &Path) -> Result<&str, String> {
    let key = path
        .to_str()
        .ok_or_else(|| "non-UTF8 resource directory".to_owned())?;
    if key.is_empty() {
        return Ok(key);
    }
    logical_path(key.strip_suffix('/').unwrap_or(key))
}

/// Observe a composed and laid out interactive login, never elapsed time or
/// process liveness. Glyph layout proves the selected locale's fonts resolved;
/// every image in the actual login subtree must be decoded by AssetServer.
pub fn login_ready(world: &bevy::prelude::World) -> bool {
    use bevy::prelude::*;
    use mir2_client_bevy::{
        crystal_ui::login::CrystalLoginAction,
        native_shell::{NativeShellModel, NativeShellScreen},
    };
    if world
        .get_resource::<NativeShellModel>()
        .is_none_or(|model| model.screen != NativeShellScreen::Login)
    {
        return false;
    }
    let Some(server) = world.get_resource::<AssetServer>() else {
        return false;
    };
    let Some(images) = world.get_resource::<Assets<Image>>() else {
        return false;
    };
    let visible_layout = |entity: &bevy::ecs::world::EntityRef<'_>| {
        entity
            .get::<ComputedNode>()
            .is_some_and(|node| !node.is_empty())
            && entity
                .get::<InheritedVisibility>()
                .is_none_or(|value| value.get())
    };
    let Some(content) = world.iter_entities().find_map(|entity| {
        (entity.get::<CrystalLoginAction>() == Some(&CrystalLoginAction::DubheWallet)
            && visible_layout(&entity))
        .then(|| entity.get::<ChildOf>().map(ChildOf::parent))
        .flatten()
    }) else {
        return false;
    };
    let descendant = |mut entity: Entity| {
        // ECS relationship cycles are rejected by Bevy; the bound also fails
        // closed if a damaged/custom hierarchy escapes that invariant.
        for _ in 0..64 {
            if entity == content {
                return true;
            }
            let Some(parent) = world.get::<ChildOf>(entity) else {
                return false;
            };
            entity = parent.parent();
        }
        false
    };
    let mut login = false;
    let mut cancel = false;
    let mut text_ready = false;
    for entity in world
        .iter_entities()
        .filter(|entity| descendant(entity.id()))
    {
        if let Some(action) = entity.get::<CrystalLoginAction>() {
            if !visible_layout(&entity) {
                return false;
            }
            login |= *action == CrystalLoginAction::Login;
            cancel |= *action == CrystalLoginAction::Cancel;
        }
        if let Some(image) = entity.get::<ImageNode>() {
            if !images.contains(image.image.id())
                || !server.is_loaded_with_dependencies(image.image.id())
            {
                return false;
            }
        }
        if let Some(text) = entity
            .get::<Text>()
            .filter(|text| !text.0.trim().is_empty())
        {
            let _ = text;
            if !entity
                .get::<bevy::text::TextLayoutInfo>()
                .is_some_and(|layout| {
                    !layout.glyphs.is_empty() && layout.size.x > 0.0 && layout.size.y > 0.0
                })
            {
                return false;
            }
            text_ready = true;
        }
    }
    let background_ready = world.iter_entities().any(|entity| {
        entity.get::<ImageNode>().is_some_and(|image| {
            server
                .get_path(image.image.id())
                .is_some_and(|path| path.path() == Path::new("original-ui/ChrSel/0.png"))
                && visible_layout(&entity)
                && images.contains(image.image.id())
                && server.is_loaded_with_dependencies(image.image.id())
        })
    });
    login && cancel && text_ready && background_ready
}
pub fn source() -> Option<AssetSourceBuilder> {
    let store = Arc::clone(store()?);
    Some(AssetSourceBuilder::new(move || {
        Box::new(PackedAssetReader::new(Arc::clone(&store)))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn sync_and_bevy_read_same_pack_without_loose_files_or_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let path = "original-ui/Test/0.png";
        std::fs::create_dir_all(source.join("original-ui/Test")).unwrap();
        let bytes = b"exact png bytes";
        std::fs::write(source.join(path), bytes).unwrap();
        let input = mir2_resource_pack::ResourceInput {
            path: path.into(),
            size: bytes.len() as u64,
            sha256: mir2_resource_pack::sha256(bytes),
        };
        let objects = temp.path().join("objects");
        let out = mir2_resource_pack::build(&source, &objects, &[input], None, Default::default())
            .unwrap();
        let store =
            Arc::new(PackStore::open(&objects, &out.catalog_bytes, &out.catalog_sha256).unwrap());
        std::fs::remove_dir_all(&source).unwrap();
        assert_eq!(read_from(&store, &format!("/{path}")).unwrap(), bytes);
        assert!(read_from(&store, "../private").is_err());
        assert!(read_from(&store, "original-ui/missing.png").is_err());
        let reader = PackedAssetReader::new(store);
        let mut loaded = Vec::new();
        let mut payload = reader.read(Path::new(path)).await.unwrap();
        bevy::asset::AsyncReadExt::read_to_end(&mut payload, &mut loaded)
            .await
            .unwrap();
        assert_eq!(loaded, bytes);
        assert!(reader
            .is_directory(Path::new("original-ui/Test"))
            .await
            .unwrap());
        assert!(reader
            .read(Path::new("original-ui/missing.png"))
            .await
            .is_err());
        assert!(reader.is_directory(Path::new("../private")).await.is_err());
        use futures_util::StreamExt;
        let mut children = reader
            .read_directory(Path::new("original-ui"))
            .await
            .unwrap();
        assert_eq!(
            children.next().await,
            Some(PathBuf::from("original-ui/Test"))
        );
        assert_eq!(children.next().await, None);
        let mut leaves = reader
            .read_directory(Path::new("original-ui/Test/"))
            .await
            .unwrap();
        assert_eq!(leaves.next().await, Some(PathBuf::from(path)));
        assert_eq!(leaves.next().await, None);
    }

    #[test]
    fn invalid_catalog_missing_or_corrupt_pack_never_mounts_loose_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        std::fs::create_dir(&source).unwrap();
        let bytes = b"unchanged resource";
        std::fs::write(source.join("frame.png"), bytes).unwrap();
        let input = mir2_resource_pack::ResourceInput {
            path: "frame.png".into(),
            size: bytes.len() as u64,
            sha256: mir2_resource_pack::sha256(bytes),
        };
        let objects = temp.path().join("objects");
        let out = mir2_resource_pack::build(&source, &objects, &[input], None, Default::default())
            .unwrap();
        assert!(PackStore::open(&objects, &out.catalog_bytes, &"0".repeat(64)).is_err());
        let pack = objects.join(out.catalog.packs[0].file_name());
        let original = std::fs::read(&pack).unwrap();
        let mut damaged = original.clone();
        *damaged.last_mut().unwrap() ^= 1;
        std::fs::write(&pack, damaged).unwrap();
        assert!(PackStore::open(&objects, &out.catalog_bytes, &out.catalog_sha256).is_err());
        std::fs::write(&pack, original).unwrap();
        std::fs::remove_file(&pack).unwrap();
        assert!(PackStore::open(&objects, &out.catalog_bytes, &out.catalog_sha256).is_err());
        // The valid loose source still exists. It has no role in pack reads.
        assert_eq!(std::fs::read(source.join("frame.png")).unwrap(), bytes);
    }

    #[test]
    fn login_model_and_process_alive_are_insufficient_for_ready() {
        let mut world = bevy::prelude::World::new();
        assert!(!login_ready(&world));
        let mut model = mir2_client_bevy::native_shell::NativeShellModel::default();
        model.screen = mir2_client_bevy::native_shell::NativeShellScreen::Login;
        world.insert_resource(model);
        assert!(!login_ready(&world));
    }
}
