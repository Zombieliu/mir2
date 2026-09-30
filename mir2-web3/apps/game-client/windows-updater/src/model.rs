use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const META: [&str; 4] = [
    "PACKAGE-MANIFEST.json",
    "VERSION.json",
    "RELEASE-STATEMENT.json",
    "RELEASE-STATEMENT.p7s",
];
pub const MAX_META: u64 = 32 * 1024 * 1024;
pub const MAX_FILE: u64 = 512 * 1024 * 1024;
pub const MAX_TOTAL: u64 = 4 * 1024 * 1024 * 1024;
pub fn hash(bytes: &[u8]) -> String {
    format!("{:X}", Sha256::digest(bytes))
}
pub fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(&c))
}
pub fn relative(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 230 && value.is_ascii(),
        "invalid path"
    );
    ensure!(
        !value.bytes().any(|c| c < 32 || b"\\:\"<>|?*".contains(&c)),
        "unsafe Windows path"
    );
    for part in value.split('/') {
        ensure!(
            !part.is_empty() && part != "." && part != ".." && !part.ends_with(['.', ' ']),
            "unsafe path segment"
        );
        let stem = part.split('.').next().unwrap().to_ascii_uppercase();
        ensure!(
            !["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && (b'1'..=b'9').contains(&stem.as_bytes()[3])),
            "reserved Windows path"
        );
    }
    Ok(())
}
pub fn game_path(value: &str) -> Result<()> {
    relative(value)?;
    const ROOT: [&str; 12] = [
        "mir2-platform-windows.exe",
        "mir2-client.toml",
        "README-START.txt",
        "CONTROLS.txt",
        "KNOWN-ISSUES.md",
        "NotoSansTC-OFL.txt",
        "OFL-NotoSans.txt",
        "OFL-NotoSansArabic.txt",
        "OFL-NotoSansDevanagari.txt",
        "OFL-NotoSansThai.txt",
        "MULTILINGUAL-SOURCES.json",
        "BUILD-ATTESTATION.json",
    ];
    if ROOT.contains(&value) || META.contains(&value) {
        return Ok(());
    }
    ensure!(value.starts_with("mir2-assets/"), "non-client payload");
    let lower = value.to_ascii_lowercase();
    ensure!(
        !lower.split('/').any(|p| p == "logs"),
        "runtime logs cannot be payload"
    );
    for segment in lower.split('/') {
        for token in segment.split('.').skip(1) {
            ensure!(
                ![
                    "exe", "dll", "com", "scr", "cpl", "msi", "msp", "bat", "cmd", "ps1", "psm1",
                    "psd1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta", "reg", "lnk", "chm",
                    "jar", "py", "pyw", "sh", "bash", "zsh", "fish", "pdb", "ilk", "dmp"
                ]
                .contains(&token),
                "executable asset rejected"
            );
        }
    }
    ensure!(
        lower.ends_with(".png")
            || lower.ends_with(".json")
            || lower.ends_with(".map.gz")
            || lower.ends_with(".wav")
            || lower.ends_with(".ttf")
            || lower.ends_with(".otf")
            || lower.ends_with(".txt"),
        "unsupported asset"
    );
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}
impl FileEntry {
    pub fn validate(&self) -> Result<()> {
        relative(&self.path)?;
        ensure!(
            self.size <= MAX_FILE && valid_hash(&self.sha256),
            "invalid file size/hash"
        );
        Ok(())
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub excludes: Vec<String>,
    pub rule: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Manifest {
    pub schema: String,
    pub coverage: Coverage,
    pub file_count: usize,
    pub total_bytes: u64,
    pub aggregate_sha256: String,
    pub files: Vec<FileEntry>,
}
impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() as u64 <= MAX_META, "manifest too large");
        let result: Self = serde_json::from_slice(bytes)?;
        ensure!(
            result.schema == "mir2.windows.package-manifest.v4"
                && result
                    .coverage
                    .excludes
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>()
                    == META.into_iter().collect()
                && result.coverage.excludes.len() == 4,
            "manifest schema/coverage"
        );
        ensure!(
            result.files.len() == result.file_count
                && result.file_count <= 200000
                && result.file_count > 0
                && result.total_bytes <= MAX_TOTAL,
            "manifest bounds"
        );
        let mut paths = BTreeSet::new();
        let mut total = 0u64;
        let mut canonical = String::new();
        let mut sorted = result.files.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| a.path.cmp(&b.path)); // v1 permits only ASCII paths: same as Windows Ordinal.
        for file in sorted {
            file.validate()?;
            game_path(&file.path)?;
            ensure!(
                !META.contains(&file.path.as_str()),
                "metadata/payload overlap"
            );
            ensure!(
                paths.insert(file.path.to_ascii_lowercase()),
                "case-colliding manifest"
            );
            total = total
                .checked_add(file.size)
                .ok_or_else(|| anyhow::anyhow!("size overflow"))?;
            canonical.push_str(&format!("{}\t{}\t{}\n", file.path, file.size, file.sha256));
        }
        ensure!(
            total == result.total_bytes && hash(canonical.as_bytes()) == result.aggregate_sha256,
            "manifest aggregate mismatch"
        );
        let files = result.map();
        for file in result.files.iter() {
            let mut parent = file.path.as_str();
            while let Some((p, _)) = parent.rsplit_once('/') {
                ensure!(
                    !paths.contains(&p.to_ascii_lowercase()),
                    "file/directory collision"
                );
                parent = p;
            }
        }
        ensure!(
            files.contains_key("mir2-platform-windows.exe")
                && files.contains_key("BUILD-ATTESTATION.json"),
            "incomplete package"
        );
        Ok(result)
    }
    pub fn map(&self) -> BTreeMap<&str, &FileEntry> {
        self.files.iter().map(|f| (f.path.as_str(), f)).collect()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Component {
    pub directory: String,
    pub identity: String,
    pub metadata: Vec<FileEntry>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Feed {
    pub schema: String,
    pub channel: String,
    pub platform: String,
    pub sequence: u64,
    pub created_unix: i64,
    pub expires_unix: i64,
    pub min_bootstrap: u32,
    pub protocol: String,
    pub content: String,
    pub game: Component,
    pub engine: Component,
}
impl Feed {
    pub fn parse(bytes: &[u8], now: i64, fresh: bool) -> Result<Self> {
        ensure!(bytes.len() <= 32768, "feed too large");
        let result: Self = serde_json::from_slice(bytes)?;
        ensure!(
            result.schema == "mir2.windows.update-feed.v1"
                && result.channel == "invited"
                && result.platform == "windows-x64"
                && result.sequence > 0
                && result.min_bootstrap <= crate::BOOTSTRAP_VERSION
                && result.protocol == "crystal-mir2-v1"
                && result.content == "mir2.windows.package-manifest.v4",
            "incompatible update"
        );
        ensure!(
            result.created_unix > 0
                && result.expires_unix > result.created_unix
                && result.expires_unix - result.created_unix <= 31 * 86400,
            "invalid feed validity"
        );
        if fresh {
            ensure!(
                result.created_unix <= now + 300 && result.expires_unix >= now,
                "expired/future feed"
            );
        }
        for component in [&result.game, &result.engine] {
            relative(&component.directory)?;
            ensure!(
                component.directory.starts_with("releases/")
                    && !component.identity.is_empty()
                    && component.identity.len() < 100,
                "invalid release component"
            );
            let mut names = BTreeSet::new();
            for file in &component.metadata {
                file.validate()?;
                ensure!(
                    file.size <= MAX_META && names.insert(file.path.as_str()),
                    "metadata bounds"
                );
            }
        }
        ensure!(
            result
                .game
                .metadata
                .iter()
                .map(|f| f.path.as_str())
                .collect::<BTreeSet<_>>()
                == META.into_iter().collect(),
            "game metadata set"
        );
        ensure!(
            result
                .engine
                .metadata
                .iter()
                .map(|f| f.path.as_str())
                .collect::<BTreeSet<_>>()
                == ["ENGINE.json", "ENGINE.p7s"].into_iter().collect(),
            "engine metadata set"
        );
        Ok(result)
    }
    pub fn check_advance(&self, bytes: &[u8], previous: Option<(&Feed, &[u8])>) -> Result<()> {
        if let Some((old, old_bytes)) = previous {
            ensure!(self.sequence >= old.sequence, "update downgrade rejected");
            ensure!(
                self.sequence != old.sequence || bytes == old_bytes,
                "reused update sequence"
            );
        }
        Ok(())
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Engine {
    pub schema: String,
    pub version: u32,
    pub min_bootstrap: u32,
    pub source_revision: String,
    pub built_unix: i64,
    pub exe_size: u64,
    pub exe_sha256: String,
    pub launcher_sha256: String,
    pub cargo_lock_sha256: String,
}
impl Engine {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() < 32768, "engine descriptor too large");
        let e: Self = serde_json::from_slice(bytes)?;
        ensure!(
            e.schema == "mir2.windows.updater-engine.v1"
                && e.version > 0
                && e.min_bootstrap <= crate::BOOTSTRAP_VERSION
                && e.built_unix > 0
                && e.source_revision.len() == 40
                && e.source_revision.bytes().all(|c| c.is_ascii_hexdigit())
                && e.exe_size > 0
                && e.exe_size <= MAX_FILE
                && valid_hash(&e.exe_sha256)
                && valid_hash(&e.launcher_sha256)
                && valid_hash(&e.cargo_lock_sha256),
            "invalid engine descriptor"
        );
        Ok(e)
    }
}
pub fn bind_game(metadata: &BTreeMap<String, Vec<u8>>) -> Result<Manifest> {
    let stmt: serde_json::Value = serde_json::from_slice(&metadata["RELEASE-STATEMENT.json"])?;
    let version: serde_json::Value = serde_json::from_slice(&metadata["VERSION.json"])?;
    ensure!(
        stmt["schema"] == "mir2.windows.release-statement.v1"
            && version["schema"] == "mir2.windows.candidate-version.v4"
            && stmt["candidate"] == version["candidate"]
            && stmt["gitRevision"] == version["gitRevision"]
            && stmt["worktreeDirty"] == false
            && version["worktreeDirty"] == false
            && version["clientOnly"] == true
            && version["exeName"] == "mir2-platform-windows.exe",
        "invalid release identity"
    );
    ensure!(
        stmt["versionSha256"] == hash(&metadata["VERSION.json"])
            && stmt["packageManifestSha256"] == hash(&metadata["PACKAGE-MANIFEST.json"])
            && version["packageManifestSha256"] == stmt["packageManifestSha256"],
        "release binding mismatch"
    );
    let manifest = Manifest::parse(&metadata["PACKAGE-MANIFEST.json"])?;
    let files = manifest.map();
    ensure!(
        stmt["packageManifestAggregateSha256"] == manifest.aggregate_sha256
            && version["packageManifestAggregateSha256"] == manifest.aggregate_sha256
            && version["packageManifestFileCount"].as_u64() == Some(manifest.file_count as u64)
            && version["packageFileCount"].as_u64() == Some(manifest.file_count as u64 + 4)
            && stmt["exeSha256"] == files["mir2-platform-windows.exe"].sha256
            && version["exeSha256"] == stmt["exeSha256"]
            && version["exeSizeBytes"].as_u64() == Some(files["mir2-platform-windows.exe"].size)
            && stmt["buildAttestationSha256"] == files["BUILD-ATTESTATION.json"].sha256
            && version["buildAttestationSha256"] == stmt["buildAttestationSha256"],
        "payload identity mismatch"
    );
    Ok(manifest)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_paths_reject_escape_alias_stream_and_devices() {
        for name in [
            "../x",
            "/x",
            "a\\b",
            "C:/x",
            "a:x",
            "a/./x",
            "a//b",
            "a./x",
            "a /x",
            "a/NUL.png",
            "COM1",
            "a/con.txt",
            "logs/x.exe",
            "mir2-assets/x.cmd.png",
            "x.exe",
        ] {
            assert!(game_path(name).is_err(), "{name}");
        }
        for name in [
            "mir2-platform-windows.exe",
            "mir2-assets/original-ui/Items/47.png",
            "mir2-client.toml",
        ] {
            game_path(name).unwrap();
        }
    }
    #[test]
    fn bad_hashes_and_sizes_rejected() {
        assert!(!valid_hash(&"a".repeat(64)));
        assert!(FileEntry {
            path: "a".into(),
            size: MAX_FILE + 1,
            sha256: "A".repeat(64)
        }
        .validate()
        .is_err());
    }
}
