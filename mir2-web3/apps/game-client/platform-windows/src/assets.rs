//! Runtime asset-root discovery for relocatable native builds.

use std::path::{Path, PathBuf};

pub const ASSET_ROOT_ENV: &str = "MIR2_NATIVE_ASSET_ROOT";

/// Resolve the directory whose contents mirror `apps/web/public`.
///
/// Installed builds look beside the executable (or in a macOS app's Resources
/// directory). Development builds discover the repository from the process
/// working directory. No compile-machine path is embedded in the executable.
pub fn asset_root() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(configured) = std::env::var_os(ASSET_ROOT_ENV) {
        candidates.push(PathBuf::from(configured));
    }

    if let Ok(executable) = std::env::current_exe() {
        if let Some(bin_dir) = executable.parent() {
            candidates.push(bin_dir.join("mir2-assets"));
            candidates.push(bin_dir.join("assets"));
            if let Some(contents_dir) = bin_dir.parent() {
                candidates.push(contents_dir.join("Resources/mir2-assets"));
            }
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        for ancestor in current_dir.ancestors() {
            candidates.push(ancestor.join("apps/web/public"));
            candidates.push(ancestor.join("mir2-web3/apps/web/public"));
        }
        candidates.push(current_dir.join("../../web/public"));
    }

    candidates
        .into_iter()
        .find(|candidate| is_mir2_asset_root(candidate))
}

pub fn asset_path(web_path: &str) -> Option<PathBuf> {
    let relative = web_path.trim_start_matches('/');
    if relative.is_empty() || relative.split('/').any(|part| part == "..") {
        return None;
    }
    asset_root().map(|root| root.join(relative))
}

fn is_mir2_asset_root(candidate: &Path) -> bool {
    candidate
        .join("bevy-entity-atlases/manifest.json")
        .is_file()
        || candidate
            .join("generated/map-atlas/manifest.json")
            .is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_asset_root_is_discovered_without_compile_time_paths() {
        let root = asset_root().expect("repo checkout should expose apps/web/public");
        assert!(is_mir2_asset_root(&root));
    }

    #[test]
    fn asset_path_rejects_parent_traversal() {
        assert!(asset_path("../private").is_none());
    }
}
