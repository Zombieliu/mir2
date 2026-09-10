//! Runtime asset-root discovery for relocatable native builds.

use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static R2_HITS: AtomicUsize = AtomicUsize::new(0);
static LOCAL_HITS: AtomicUsize = AtomicUsize::new(0);

pub fn asset_hit_stats() -> (usize, usize) {
    (
        LOCAL_HITS.load(Ordering::Relaxed),
        R2_HITS.load(Ordering::Relaxed),
    )
}

pub fn has_local_full() -> bool {
    asset_root()
        .map(|root| {
            root.join("generated/crystal-packs/full/index.json")
                .is_file()
        })
        .unwrap_or(false)
}

pub const ASSET_ROOT_ENV: &str = "MIR2_NATIVE_ASSET_ROOT";
pub const ASSET_ROOT_ENV_ALIAS: &str = "MIR2_ASSET_ROOT";

/// Find map layout bytes in a packaged bundle or beside the real development
/// asset directory. Resolve a junction/symlink before looking for siblings:
/// `installed/mir2-assets/../lib` otherwise refers to the installation directory
/// on Windows, not to the checkout that the asset link targets.
pub fn crystal_map_path(asset_root: &Path, map_file_name: &str) -> Option<PathBuf> {
    let file_name = Path::new(map_file_name).file_name()?.to_str()?;
    if file_name != map_file_name || file_name.contains("..") || file_name.contains(['\\', ':']) {
        return None;
    }
    let stem = file_name
        .strip_suffix(".map.gz")
        .or_else(|| file_name.strip_suffix(".map"))
        .unwrap_or(file_name);
    if stem.is_empty() {
        return None;
    }
    let file_name = format!("{stem}.map.gz");
    for pack in ["crystal-map-pack", "generated/crystal-map-pack"] {
        let candidate = asset_root.join(pack).join(&file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let physical_root = asset_root.canonicalize().ok()?;
    let candidate = physical_root
        .parent()?
        .join("lib/generated/crystal-map-pack")
        .join(file_name);
    candidate.is_file().then_some(candidate)
}

/// Resolve the directory whose contents mirror `apps/web/public`.
///
/// Installed builds look beside the executable (or in a macOS app's Resources
/// directory). Development builds discover the repository from the process
/// working directory. No compile-machine path is embedded in the executable.
pub fn asset_root() -> Option<PathBuf> {
    match resolve_asset_root() {
        AssetRootStatus::Found(path) => Some(path),
        AssetRootStatus::Incomplete { .. } | AssetRootStatus::Missing { .. } => None,
    }
}

#[derive(Debug, Clone)]
pub enum AssetRootStatus {
    Found(PathBuf),
    Incomplete {
        path: PathBuf,
        diagnostics: AssetRootDiagnostics,
    },
    Missing {
        diagnostics: Vec<(PathBuf, AssetRootDiagnostics)>,
    },
}

pub fn resolve_asset_root() -> AssetRootStatus {
    let mut missing_diagnostics = Vec::new();

    if let Some(configured) =
        std::env::var_os(ASSET_ROOT_ENV).or_else(|| std::env::var_os(ASSET_ROOT_ENV_ALIAS))
    {
        let path = PathBuf::from(configured);
        let diagnostics = diagnose_asset_root(&path);
        missing_diagnostics.push((path.clone(), diagnostics));
        if diagnostics.is_complete {
            return AssetRootStatus::Found(path);
        }
        return AssetRootStatus::Incomplete { path, diagnostics };
    }

    let mut sticky_incomplete = None;
    for path in installed_asset_candidates() {
        let diagnostics = diagnose_asset_root(&path);
        let exists = path.is_dir();
        missing_diagnostics.push((path.clone(), diagnostics));
        if diagnostics.is_complete {
            return AssetRootStatus::Found(path);
        }
        if exists && sticky_incomplete.is_none() {
            sticky_incomplete = Some((path, diagnostics));
        }
    }
    if let Some((path, diagnostics)) = sticky_incomplete {
        return AssetRootStatus::Incomplete { path, diagnostics };
    }

    for path in development_asset_candidates() {
        let diagnostics = diagnose_asset_root(&path);
        missing_diagnostics.push((path.clone(), diagnostics));
        if diagnostics.is_complete {
            return AssetRootStatus::Found(path);
        }
    }

    AssetRootStatus::Missing {
        diagnostics: missing_diagnostics,
    }
}

pub fn require_asset_root() -> Result<PathBuf, String> {
    match resolve_asset_root() {
        AssetRootStatus::Found(path) => Ok(path),
        AssetRootStatus::Incomplete { path, diagnostics } => {
            Err(incomplete_asset_error(&path, diagnostics))
        }
        AssetRootStatus::Missing { diagnostics } => {
            let mut message = format!(
                "no Mir2 asset bundle found. Place a complete mir2-assets directory beside the executable, or set {ASSET_ROOT_ENV}."
            );
            message.push_str(" Required files: crystal-map-pack/0.map.gz (or generated/crystal-map-pack/0.map.gz; development also supports the physical asset root's sibling lib/generated/crystal-map-pack), bevy-entity-atlases/manifest.json, generated/map-atlas/manifest.json, generated/native-map-keyed/manifest.json, original-effects/effects.generated.json, original-ui/Items/meta.json, original-ui/Items/0.png, original-ui/Items/3792.png, original-ui/StateItem/meta.json, original-ui/StateItem/30.png, original-ui/StateItem/5152.png, original-ui/Prguse2/meta.json, original-ui/Prguse2/1202.png through 1205.png, and the four original-ui/Cursors native PNGs.");
            for (candidate, diag) in diagnostics {
                message.push_str(&format!(
                    "\n  candidate {} -> map_layout={} entity={} map={} native_map_keyed={} effect={} items={} state_items={} character_wings={} cursors={} complete={}",
                    candidate.display(),
                    diag.has_crystal_map_pack,
                    diag.has_entity_manifest,
                    diag.has_map_manifest,
                    diag.has_native_map_keyed_manifest,
                    diag.has_effect_manifest,
                    diag.has_item_icons,
                    diag.has_state_items,
                    diag.has_character_wings,
                    diag.has_crystal_cursors,
                    diag.is_complete
                ));
            }
            Err(message)
        }
    }
}

fn incomplete_asset_error(path: &Path, diagnostics: AssetRootDiagnostics) -> String {
    format!(
        "asset bundle at {} is incomplete (map_layout={} entity_manifest={} map_manifest={} native_map_keyed_manifest={} effect_manifest={} item_icons={} state_items={} character_wings={} crystal_cursors={}). Need crystal-map-pack/0.map.gz (or generated/crystal-map-pack/0.map.gz; development also supports the physical asset root's sibling lib/generated/crystal-map-pack), bevy-entity-atlases/manifest.json, generated/map-atlas/manifest.json, generated/native-map-keyed/manifest.json, original-effects/effects.generated.json, original-ui/Items/meta.json, original-ui/Items/0.png, original-ui/Items/3792.png, original-ui/StateItem/meta.json, original-ui/StateItem/30.png, original-ui/StateItem/5152.png, original-ui/Prguse2/meta.json, original-ui/Prguse2/1202.png through 1205.png, and the four original-ui/Cursors native PNGs. The window will not open with a missing pack.",
        path.display(),
        diagnostics.has_crystal_map_pack,
        diagnostics.has_entity_manifest,
        diagnostics.has_map_manifest,
        diagnostics.has_native_map_keyed_manifest,
        diagnostics.has_effect_manifest,
        diagnostics.has_item_icons,
        diagnostics.has_state_items,
        diagnostics.has_character_wings,
        diagnostics.has_crystal_cursors
    )
}

fn installed_asset_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(bin_dir) = executable.parent() {
            candidates.push(bin_dir.join("mir2-assets"));
            candidates.push(bin_dir.join("assets"));
            if let Some(contents_dir) = bin_dir.parent() {
                candidates.push(contents_dir.join("Resources/mir2-assets"));
            }
        }
    }
    candidates
}

fn development_asset_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        for ancestor in current_dir.ancestors() {
            candidates.push(ancestor.join("apps/web/public"));
            candidates.push(ancestor.join("mir2-web3/apps/web/public"));
        }
        candidates.push(current_dir.join("../../web/public"));
    }
    candidates
}

#[derive(Debug, Clone, Copy)]
pub struct AssetRootDiagnostics {
    pub is_complete: bool,
    pub has_crystal_map_pack: bool,
    pub has_entity_manifest: bool,
    pub has_map_manifest: bool,
    pub has_native_map_keyed_manifest: bool,
    pub has_effect_manifest: bool,
    pub has_item_icons: bool,
    pub has_state_items: bool,
    pub has_character_wings: bool,
    pub has_crystal_cursors: bool,
}

pub fn diagnose_asset_root(candidate: &Path) -> AssetRootDiagnostics {
    let has_crystal_map_pack = crystal_map_path(candidate, "0").is_some();
    let has_entity_manifest = candidate
        .join("bevy-entity-atlases/manifest.json")
        .is_file();
    let has_map_manifest = candidate
        .join("generated/map-atlas/manifest.json")
        .is_file();
    let has_native_map_keyed_manifest = candidate
        .join("generated/native-map-keyed/manifest.json")
        .is_file();
    let has_effect_manifest = candidate
        .join("original-effects/effects.generated.json")
        .is_file();
    let item_root = candidate.join("original-ui/Items");
    let has_item_icons = item_root.join("meta.json").is_file()
        && item_root.join("0.png").is_file()
        && item_root.join("3792.png").is_file();
    let state_item_root = candidate.join("original-ui/StateItem");
    let has_state_items = state_item_root.join("meta.json").is_file()
        && state_item_root.join("30.png").is_file()
        && state_item_root.join("5152.png").is_file();
    let wing_root = candidate.join("original-ui/Prguse2");
    let has_character_wings = wing_root.join("meta.json").is_file()
        && ["1202.png", "1203.png", "1204.png", "1205.png"]
            .iter()
            .all(|name| wing_root.join(name).is_file());
    let cursor_root = candidate.join("original-ui/Cursors");
    let has_crystal_cursors = [
        "Cursor_Default.png",
        "Cursor_Normal_Atk.png",
        "Cursor_Compulsion_Atk.png",
        "Cursor_Npc.png",
    ]
    .iter()
    .all(|name| cursor_root.join(name).is_file());
    let is_complete = has_crystal_map_pack
        && has_entity_manifest
        && has_map_manifest
        && has_native_map_keyed_manifest
        && has_effect_manifest
        && has_item_icons
        && has_state_items
        && has_character_wings
        && has_crystal_cursors;
    AssetRootDiagnostics {
        is_complete,
        has_crystal_map_pack,
        has_entity_manifest,
        has_map_manifest,
        has_native_map_keyed_manifest,
        has_effect_manifest,
        has_item_icons,
        has_state_items,
        has_character_wings,
        has_crystal_cursors,
    }
}

pub const R2_ASSET_BASE_URL_ENV: &str = "MIR2_R2_ASSET_BASE_URL";

fn r2_base_urls() -> Vec<String> {
    if let Ok(url) = std::env::var(R2_ASSET_BASE_URL_ENV) {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return vec![trimmed.to_owned()];
        }
    }
    // Fallback to the same R2 prefix the Web uses (production-web-assets.json).
    // Keep it hard-coded so a dev checkout without env still gets on-demand pages.
    vec![
        "https://assets.mir2.obelisk.build/mir2/v/20260730-fullcrystal-f71b89aa-gzip1".to_owned(),
        "https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev/mir2/v/20260730-fullcrystal-f71b89aa-gzip1".to_owned(),
    ]
}

pub(crate) fn r2_cache_dir() -> PathBuf {
    let p = PathBuf::from("F:\\mir2-r2-cache");
    if std::fs::create_dir_all(&p).is_ok() || p.is_dir() {
        return p;
    }
    std::env::temp_dir().join("mir2-r2-cache")
}

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

static R2_INFLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn r2_inflight() -> &'static Mutex<HashSet<String>> {
    R2_INFLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

fn ensure_r2_cached(web_path: &str) -> Option<PathBuf> {
    let relative = web_path.trim_start_matches('/');
    let cache_path = r2_cache_dir().join(relative);
    if cache_path.is_file() {
        return Some(cache_path);
    }
    // Non-blocking: if this page is already being fetched, just miss this
    // frame and let the background thread fill the cache. Never block the
    // Bevy main thread on a network round-trip (the 100ms Crystal tick).
    {
        let mut inflight = r2_inflight().lock().unwrap_or_else(|p| p.into_inner());
        if inflight.contains(web_path) {
            return None;
        }
        inflight.insert(web_path.to_owned());
    }
    let web_path_owned = web_path.to_owned();
    let cache_path_clone = cache_path.clone();
    std::thread::spawn(move || {
        let relative = web_path_owned.trim_start_matches('/');
        let cache_path = cache_path_clone;
        for base in r2_base_urls() {
            let url = format!("{}/{}", base.trim_end_matches('/'), relative);
            let Ok(resp) = ureq::get(&url).call() else {
                continue;
            };
            if resp.status() != 200 {
                continue;
            }
            let mut reader = resp.into_reader();
            if let Some(parent) = cache_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let tmp = cache_path.with_extension("tmp");
            let Ok(mut file) = std::fs::File::create(&tmp) else {
                continue;
            };
            if std::io::copy(&mut reader, &mut file).is_err() {
                let _ = std::fs::remove_file(&tmp);
                continue;
            }
            let _ = file.sync_all();
            drop(file);
            if std::fs::rename(&tmp, &cache_path).is_ok() {
                break;
            }
            let _ = std::fs::remove_file(&tmp);
        }
        r2_inflight()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&web_path_owned);
    });
    None
}

pub fn batch_prefetch_bichon_town() {
    // Fire-and-forget: ensure every Bichon town page in the map-atlas manifest
    // is at least queued for R2. The manifest itself is the first miss.
    let manifest_web = "generated/map-atlas/manifest.json";
    let manifest_cached = r2_cache_dir().join(manifest_web);
    // Kick the manifest first; next tick it will be present and we can fan out.
    if !manifest_cached.is_file() {
        let _ = ensure_r2_cached(manifest_web);
        return;
    }
    let Ok(data) = std::fs::read_to_string(&manifest_cached) else {
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) else {
        return;
    };
    let Some(atlases) = json.get("atlases").and_then(|v| v.as_array()) else {
        return;
    };
    for atlas in atlases {
        // Newer manifests use "pages": [{"u": "/generated/map-atlas/..."}, ...]
        if let Some(pages) = atlas.get("pages").and_then(|v| v.as_array()) {
            for page in pages {
                if let Some(u) = page.get("u").and_then(|v| v.as_str()) {
                    let _ = ensure_r2_cached(u);
                }
            }
        }
        // Legacy fallback: single "u" per atlas
        if let Some(u) = atlas.get("u").and_then(|v| v.as_str()) {
            let _ = ensure_r2_cached(u);
        }
    }
}

pub fn asset_path(web_path: &str) -> Option<PathBuf> {
    let relative = web_path.trim_start_matches('/');
    if relative.is_empty() || relative.contains(['\\', ':']) {
        return None;
    }
    let relative_path = Path::new(relative);
    if !relative_path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return None;
    }
    // 1) Local first — pure offline determinism.
    if let Some(root) = asset_root() {
        let local = root.join(relative_path);
        if local.is_file() {
            LOCAL_HITS.fetch_add(1, Ordering::Relaxed);
            return Some(local);
        }
        // Local bundle exists but this page is missing (e.g. full 4446 pages not
        // yet installed). Kick off R2 on-demand and miss this frame — next
        // tick the cache will be hit. Do not block the Bevy main thread.
        if let Some(cached) = ensure_r2_cached(web_path) {
            let n = R2_HITS.fetch_add(1, Ordering::Relaxed) + 1;
            let line = format!("[assets] R2 HIT #{n} {web_path} -> {}\n", cached.display());
            eprintln!("{line}");
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(r2_cache_dir().join("_r2_hits.log"))
                .and_then(|mut f| f.write_all(line.as_bytes()));
            return Some(cached);
        }
        // Kick off fetch for next tick
        let _ = ensure_r2_cached(web_path);
        return None;
    }
    // No local bundle at all — try R2 directly (dev without full).
    if let Some(cached) = ensure_r2_cached(web_path) {
        let n = R2_HITS.fetch_add(1, Ordering::Relaxed) + 1;
        let line = format!("[assets] R2 HIT #{n} {web_path} -> {}\n", cached.display());
        eprintln!("{line}");
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(r2_cache_dir().join("_r2_hits.log"))
            .and_then(|mut f| f.write_all(line.as_bytes()));
        return Some(cached);
    }
    let _ = ensure_r2_cached(web_path);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_fixture_root(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mir2-map-path-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&root).expect("unique map fixture directory");
        root
    }

    fn write_fixture_map(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().expect("map parent")).expect("map directory");
        std::fs::write(path, bytes).expect("fixture map bytes");
    }

    #[test]
    fn crystal_map_path_prefers_packaged_layouts_and_rejects_traversal() {
        let dir = map_fixture_root("packaged");
        let root = dir.join("public");
        std::fs::create_dir(&root).expect("asset directory");
        let sibling = dir.join("lib/generated/crystal-map-pack/0.map.gz");
        write_fixture_map(&sibling, b"development");
        assert_eq!(
            crystal_map_path(&root, "0").expect("development map"),
            sibling.canonicalize().expect("physical development map")
        );

        let generated = root.join("generated/crystal-map-pack/0.map.gz");
        write_fixture_map(&generated, b"generated");
        assert_eq!(crystal_map_path(&root, "0.map"), Some(generated));
        let packaged = root.join("crystal-map-pack/0.map.gz");
        write_fixture_map(&packaged, b"packaged");
        for name in ["0", "0.map", "0.map.gz"] {
            assert_eq!(crystal_map_path(&root, name), Some(packaged.clone()));
        }
        for name in ["", ".map", ".map.gz", "../0", "maps/0", "..\\0", "C:\\0"] {
            assert!(crystal_map_path(&root, name).is_none(), "rejected {name}");
        }
        assert!(crystal_map_path(&root, "missing").is_none());
        std::fs::remove_dir_all(dir).expect("remove isolated map fixture");
    }

    #[test]
    #[cfg(any(windows, unix))]
    fn crystal_map_path_follows_directory_alias_before_sibling_lookup() {
        let dir = map_fixture_root("alias");
        let physical_root = dir.join("checkout/apps/web/public");
        std::fs::create_dir_all(&physical_root).expect("physical asset directory");
        let physical_map = dir.join("checkout/apps/web/lib/generated/crystal-map-pack/0.map.gz");
        write_fixture_map(&physical_map, b"physical checkout map");
        let installed = dir.join("installed");
        std::fs::create_dir(&installed).expect("installation directory");
        // A same-named sibling at the alias location must not shadow the map
        // belonging to the real asset directory.
        write_fixture_map(
            &installed.join("lib/generated/crystal-map-pack/0.map.gz"),
            b"unrelated installation sibling",
        );
        let alias = installed.join("mir2-assets");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let output = std::process::Command::new("powershell.exe")
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "$ErrorActionPreference = 'Stop'; New-Item -ItemType Junction -Path $env:MIR2_TEST_MAP_ALIAS -Target $env:MIR2_TEST_MAP_TARGET | Out-Null",
                ])
                .env("MIR2_TEST_MAP_ALIAS", &alias)
                .env("MIR2_TEST_MAP_TARGET", &physical_root)
                .creation_flags(0x08000000) // CREATE_NO_WINDOW: filesystem fixture only.
                .output()
                .expect("create Windows directory junction");
            assert!(
                output.status.success(),
                "junction fixture failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&physical_root, &alias).expect("create directory symlink");

        for name in ["0", "0.map", "0.map.gz"] {
            let resolved =
                crystal_map_path(&alias, name).expect("map through relocated asset root");
            assert_eq!(resolved, physical_map.canonicalize().expect("physical map"));
            assert_eq!(std::fs::read(resolved).unwrap(), b"physical checkout map");
        }
        assert!(diagnose_asset_root(&alias).has_crystal_map_pack);
        #[cfg(windows)]
        std::fs::remove_dir(&alias).expect("remove junction, keeping target intact");
        #[cfg(unix)]
        std::fs::remove_file(&alias).expect("remove symlink, keeping target intact");
        assert!(physical_map.is_file());
        std::fs::remove_dir_all(dir).expect("remove isolated map fixture");
    }

    #[test]
    fn repository_asset_root_is_discovered_without_compile_time_paths() {
        let root = asset_root().expect("repo checkout should expose apps/web/public");
        let diagnostics = diagnose_asset_root(&root);
        assert!(diagnostics.has_crystal_map_pack);
        assert!(diagnostics.has_entity_manifest);
        assert!(diagnostics.has_map_manifest);
        assert!(diagnostics.has_native_map_keyed_manifest);
        assert!(diagnostics.has_effect_manifest);
        assert!(diagnostics.has_item_icons);
        assert!(diagnostics.has_state_items);
        assert!(diagnostics.has_character_wings);
        assert!(diagnostics.has_crystal_cursors);
        assert!(diagnostics.is_complete);
        match resolve_asset_root() {
            AssetRootStatus::Found(path) => assert_eq!(path, root),
            other => panic!("expected found asset root, got {other:?}"),
        }
    }

    #[test]
    fn asset_path_rejects_parent_traversal() {
        assert!(asset_path("../private").is_none());
        assert!(asset_path("maps/../../private").is_none());
        assert!(asset_path(r"..\private").is_none());
        assert!(asset_path(r"C:\private").is_none());
        assert!(asset_path("./private").is_none());
    }

    #[test]
    fn incomplete_root_is_not_a_complete_asset_bundle() {
        let dir =
            std::env::temp_dir().join(format!("mir2-asset-incomplete-{}", std::process::id()));
        let entity_dir = dir.join("bevy-entity-atlases");
        std::fs::create_dir_all(&entity_dir).expect("temp asset dir");
        std::fs::write(entity_dir.join("manifest.json"), "{}").expect("entity manifest");
        let diagnostics = diagnose_asset_root(&dir);
        assert!(diagnostics.has_entity_manifest);
        assert!(!diagnostics.has_map_manifest);
        assert!(!diagnostics.has_native_map_keyed_manifest);
        assert!(!diagnostics.has_effect_manifest);
        assert!(!diagnostics.has_item_icons);
        assert!(!diagnostics.has_state_items);
        assert!(!diagnostics.has_character_wings);
        assert!(!diagnostics.has_crystal_cursors);
        assert!(!diagnostics.is_complete);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn complete_render_manifests_without_item_icons_fail_closed() {
        let dir =
            std::env::temp_dir().join(format!("mir2-asset-missing-items-{}", std::process::id()));
        for relative in [
            "bevy-entity-atlases/manifest.json",
            "generated/map-atlas/manifest.json",
            "generated/native-map-keyed/manifest.json",
            "original-effects/effects.generated.json",
        ] {
            let path = dir.join(relative);
            std::fs::create_dir_all(path.parent().expect("asset parent")).expect("temp asset dir");
            std::fs::write(path, "{}").expect("asset manifest");
        }

        let diagnostics = diagnose_asset_root(&dir);
        assert!(diagnostics.has_entity_manifest);
        assert!(diagnostics.has_map_manifest);
        assert!(diagnostics.has_native_map_keyed_manifest);
        assert!(diagnostics.has_effect_manifest);
        assert!(!diagnostics.has_item_icons);
        assert!(!diagnostics.is_complete);

        let item_root = dir.join("original-ui/Items");
        std::fs::create_dir_all(&item_root).expect("item icon root");
        std::fs::write(item_root.join("meta.json"), "{}").expect("item meta");
        std::fs::write(item_root.join("0.png"), []).expect("first item icon");
        std::fs::write(item_root.join("3792.png"), []).expect("last item icon");
        let items_only = diagnose_asset_root(&dir);
        assert!(items_only.has_item_icons);
        assert!(!items_only.has_state_items);
        assert!(!items_only.has_character_wings);
        assert!(!items_only.has_crystal_cursors);
        assert!(!items_only.is_complete);

        let state_item_root = dir.join("original-ui/StateItem");
        std::fs::create_dir_all(&state_item_root).expect("state item root");
        std::fs::write(state_item_root.join("meta.json"), "{}").expect("state item meta");
        std::fs::write(state_item_root.join("30.png"), []).expect("first state item");
        std::fs::write(state_item_root.join("5152.png"), []).expect("tail state item");
        let state_items = diagnose_asset_root(&dir);
        assert!(state_items.has_state_items);
        assert!(!state_items.has_character_wings);
        assert!(!state_items.has_crystal_cursors);
        assert!(!state_items.is_complete);

        let wing_root = dir.join("original-ui/Prguse2");
        std::fs::create_dir_all(&wing_root).expect("character wing root");
        std::fs::write(wing_root.join("meta.json"), "{}").expect("character wing meta");
        for name in ["1202.png", "1203.png", "1204.png"] {
            std::fs::write(wing_root.join(name), []).expect("character wing image");
        }
        assert!(
            !diagnose_asset_root(&dir).has_character_wings,
            "all four effect/gender variants are required"
        );
        std::fs::write(wing_root.join("1205.png"), []).expect("last character wing image");
        let character_wings = diagnose_asset_root(&dir);
        assert!(character_wings.has_character_wings);
        assert!(!character_wings.has_crystal_cursors);
        assert!(!character_wings.is_complete);

        let cursor_root = dir.join("original-ui/Cursors");
        std::fs::create_dir_all(&cursor_root).expect("cursor root");
        for name in [
            "Cursor_Default.png",
            "Cursor_Normal_Atk.png",
            "Cursor_Compulsion_Atk.png",
            "Cursor_Npc.png",
        ] {
            std::fs::write(cursor_root.join(name), []).expect("cursor image");
        }
        let missing_map = diagnose_asset_root(&dir);
        assert!(missing_map.has_crystal_cursors);
        assert!(!missing_map.has_crystal_map_pack);
        assert!(
            !missing_map.is_complete,
            "textures alone must not pass startup"
        );
        assert!(incomplete_asset_error(&dir, missing_map).contains("map_layout=false"));
        assert!(incomplete_asset_error(&dir, missing_map).contains("0.map.gz"));

        write_fixture_map(
            &dir.join("crystal-map-pack/0.map.gz"),
            b"map presence fixture",
        );
        let complete = diagnose_asset_root(&dir);
        assert!(complete.has_crystal_map_pack);
        assert!(complete.is_complete);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
