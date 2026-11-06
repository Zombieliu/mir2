//! Runtime asset-root discovery for relocatable native builds.

use std::path::{Component, Path, PathBuf};

pub const ASSET_ROOT_ENV: &str = "MIR2_NATIVE_ASSET_ROOT";
pub const ASSET_ROOT_ENV_ALIAS: &str = "MIR2_ASSET_ROOT";

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
            message.push_str(" Required files: bevy-entity-atlases/manifest.json, generated/map-atlas/manifest.json, original-effects/effects.generated.json.");
            for (candidate, diag) in diagnostics {
                message.push_str(&format!(
                    "\n  candidate {} -> entity={} map={} effect={} complete={}",
                    candidate.display(),
                    diag.has_entity_manifest,
                    diag.has_map_manifest,
                    diag.has_effect_manifest,
                    diag.is_complete
                ));
            }
            Err(message)
        }
    }
}

fn incomplete_asset_error(path: &Path, diagnostics: AssetRootDiagnostics) -> String {
    format!(
        "asset bundle at {} is incomplete (entity_manifest={} map_manifest={} effect_manifest={}). Need bevy-entity-atlases/manifest.json, generated/map-atlas/manifest.json, and original-effects/effects.generated.json. The window will not open with a missing pack.",
        path.display(),
        diagnostics.has_entity_manifest,
        diagnostics.has_map_manifest,
        diagnostics.has_effect_manifest
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
    pub has_entity_manifest: bool,
    pub has_map_manifest: bool,
    pub has_effect_manifest: bool,
}

pub fn diagnose_asset_root(candidate: &Path) -> AssetRootDiagnostics {
    let has_entity_manifest = candidate
        .join("bevy-entity-atlases/manifest.json")
        .is_file();
    let has_map_manifest = candidate
        .join("generated/map-atlas/manifest.json")
        .is_file();
    let has_effect_manifest = candidate
        .join("original-effects/effects.generated.json")
        .is_file();
    let is_complete = has_entity_manifest && has_map_manifest && has_effect_manifest;
    AssetRootDiagnostics {
        is_complete,
        has_entity_manifest,
        has_map_manifest,
        has_effect_manifest,
    }
}

pub fn asset_path(web_path: &str) -> Option<PathBuf> {
    let relative = web_path.trim_start_matches('/');
    if relative.is_empty() || relative.contains(['\\', ':']) {
        return None;
    }
    let relative = Path::new(relative);
    if !relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return None;
    }
    asset_root().map(|root| root.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_asset_root_is_discovered_without_compile_time_paths() {
        let root = asset_root().expect("repo checkout should expose apps/web/public");
        let diagnostics = diagnose_asset_root(&root);
        assert!(diagnostics.has_entity_manifest);
        assert!(diagnostics.has_map_manifest);
        assert!(diagnostics.has_effect_manifest);
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
        assert!(!diagnostics.has_effect_manifest);
        assert!(!diagnostics.is_complete);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
