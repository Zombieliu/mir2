use super::*;
use crate::update::{self, Source, Status};
use flate2::{write::GzEncoder, Compression};
use std::time::Instant;
use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
};
use tempfile::TempDir;

fn entry(path: &str, bytes: &[u8]) -> FileEntry {
    FileEntry {
        path: path.into(),
        size: bytes.len() as u64,
        sha256: hash(bytes),
    }
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn write_artifact(root: &Path, bytes: &[u8], extension: &str) -> (PathBuf, FileEntry) {
    let entry = entry(&format!("delivery/{}.{}", hash(bytes), extension), bytes);
    let path = root.join(&entry.path);
    safe::write_new(&path, bytes).unwrap();
    (path, entry)
}

fn pack_raw(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = BUNDLE_MAGIC.to_vec();
    bytes.extend_from_slice(&(files.len() as u32).to_le_bytes());
    for (path, data) in files {
        bytes.extend_from_slice(&(path.len() as u16).to_le_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.extend_from_slice(&(data.len() as u64).to_le_bytes());
        bytes.extend_from_slice(data);
    }
    bytes
}

fn delta_raw(target_size: u64, operations: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = DELTA_MAGIC.to_vec();
    bytes.extend_from_slice(&(operations.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&target_size.to_le_bytes());
    for op in operations {
        bytes.extend_from_slice(op);
    }
    bytes
}

fn copy_op(offset: u64, size: u64) -> Vec<u8> {
    let mut op = vec![0];
    op.extend_from_slice(&offset.to_le_bytes());
    op.extend_from_slice(&size.to_le_bytes());
    op
}

fn literal_op(bytes: &[u8]) -> Vec<u8> {
    let mut op = vec![1];
    op.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    op.extend_from_slice(bytes);
    op
}

fn manifest(files: &[FileEntry]) -> Manifest {
    let mut sorted = files.to_vec();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let canonical = sorted
        .iter()
        .map(|e| format!("{}\t{}\t{}\n", e.path, e.size, e.sha256))
        .collect::<String>();
    Manifest::parse(&serde_json::to_vec(&serde_json::json!({
        "schema":"mir2.windows.package-manifest.v4", "coverage":{"excludes":META,"rule":"all regular files except metadata"},
        "fileCount":files.len(),"totalBytes":files.iter().map(|e|e.size).sum::<u64>(),
        "aggregateSha256":hash(canonical.as_bytes()),"files":files
    })).unwrap()).unwrap()
}

#[test]
fn delivery_is_bound_to_exact_release_and_has_no_url_or_unknown_field_surface() {
    let files = [
        entry("mir2-platform-windows.exe", b"exe"),
        entry("BUILD-ATTESTATION.json", b"attest"),
    ];
    let manifest = manifest(&files);
    let archive = entry("delivery/a.m2b.gz", b"gzip");
    let descriptor = Delivery {
        schema: "mir2.windows.delivery.v1".into(),
        candidate: "candidate-1".into(),
        package_manifest_sha256: hash(b"manifest"),
        version_sha256: hash(b"version"),
        built_unix: 100,
        bundles: vec![Bundle {
            archive,
            files: files.iter().map(|e| e.path.clone()).collect(),
        }],
        patches: vec![],
    };
    let valid = serde_json::to_value(&descriptor).unwrap();
    let parse = |value: &serde_json::Value| {
        Delivery::parse(
            &serde_json::to_vec(value).unwrap(),
            "candidate-1",
            b"manifest",
            b"version",
            &manifest,
            100,
        )
    };
    parse(&valid).unwrap();
    for key in [
        "schema",
        "candidate",
        "versionSha256",
        "packageManifestSha256",
    ] {
        let mut bad = valid.clone();
        bad[key] = "wrong".into();
        assert!(parse(&bad).is_err(), "{key}");
    }
    for value in [0, 401] {
        let mut bad = valid.clone();
        bad["builtUnix"] = value.into();
        assert!(parse(&bad).is_err());
    }
    let mut bad = valid.clone();
    bad["url"] = "https://outside.invalid/payload".into();
    assert!(parse(&bad).is_err());
    for path in [
        "https://outside.invalid/a.m2b.gz",
        "../a.m2b.gz",
        "delivery/../a.m2b.gz",
        "a.m2b.gz",
        "delivery/a.exe",
    ] {
        let mut bad = valid.clone();
        bad["bundles"][0]["archive"]["path"] = path.into();
        assert!(parse(&bad).is_err(), "{path}");
    }
    let mut bad = valid.clone();
    bad["bundles"][0]["files"][1] = "mir2-client.toml".into();
    assert!(parse(&bad).is_err());
    let mut bad = valid.clone();
    bad["bundles"][0]["files"][1] = "mir2-platform-windows.exe".into();
    assert!(parse(&bad).is_err());
    let mut bad = valid.clone();
    bad["bundles"][0]["archive"]["size"] = (MAX_COMPRESSED + 1).into();
    assert!(parse(&bad).is_err());
}

#[test]
fn bundle_streams_exact_files_into_hash_cache_and_never_into_live_installation() {
    let root = TempDir::new().unwrap();
    safe::write_new(&root.path().join("game/personal-file.txt"), b"personal").unwrap();
    let first = entry("mir2-platform-windows.exe", b"signed exe");
    let second = entry("mir2-assets/x.json", b"{\"asset\":true}");
    let bytes = gzip(&pack_raw(&[
        (&first.path, b"signed exe"),
        (&second.path, b"{\"asset\":true}"),
    ]));
    let (path, archive) = write_artifact(root.path(), &bytes, "m2b.gz");
    extract_bundle(
        root.path(),
        &path,
        &archive,
        &[&first, &second],
        &mut || Ok(()),
    )
    .unwrap();
    for target in [&first, &second] {
        assert!(safe::matches(&payload_cache(root.path(), target).unwrap(), target).unwrap());
        assert!(!root.path().join("game").join(&target.path).exists());
    }
    // Valid cache reuse still consumes/verifies every archive byte.
    extract_bundle(
        root.path(),
        &path,
        &archive,
        &[&first, &second],
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(
        fs::read(root.path().join("game/personal-file.txt")).unwrap(),
        b"personal"
    );
}

#[test]
fn parallel_bundle_shared_hashes_large_and_empty_entries_verify_every_body_and_join_on_cancel() {
    let paths: Vec<_> = (0..64)
        .map(|i| format!("mir2-assets/pipeline/{i}.png"))
        .collect();
    let data: Vec<Vec<u8>> = (0..64)
        .map(|i| {
            if i % 20 == 0 {
                vec![17; 512 * 1024 + 1]
            } else if i % 11 == 0 {
                Vec::new()
            } else {
                vec![(i % 7) as u8; 2048]
            }
        })
        .collect();
    let entries: Vec<_> = paths
        .iter()
        .zip(&data)
        .map(|(path, bytes)| entry(path, bytes))
        .collect();
    let pairs: Vec<_> = paths
        .iter()
        .zip(&data)
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect();
    let expected: Vec<_> = entries.iter().collect();
    let root = TempDir::new().unwrap();
    let (path, archive) = write_artifact(root.path(), &gzip(&pack_raw(&pairs)), "m2b.gz");
    let mut counts = Vec::new();
    extract_bundle_observed(
        root.path(),
        &path,
        &archive,
        &expected,
        &mut || Ok(()),
        &mut |n| counts.push(n),
    )
    .unwrap();
    assert_eq!(counts, (1..=64).collect::<Vec<_>>());
    for entry in &entries {
        assert!(safe::matches(&payload_cache(root.path(), entry).unwrap(), entry).unwrap());
    }
    // A repeated body with the same expected hash must not be silently skipped.
    let mut corrupt = data.clone();
    corrupt[8][0] ^= 1;
    let pairs: Vec<_> = paths
        .iter()
        .zip(&corrupt)
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect();
    let (path, archive) = write_artifact(root.path(), &gzip(&pack_raw(&pairs)), "m2b.gz");
    assert!(extract_bundle(root.path(), &path, &archive, &expected, &mut || Ok(())).is_err());
    assert!(!root.path().join("game").exists());

    let cancelled = TempDir::new().unwrap();
    let (path, archive) = write_artifact(cancelled.path(), &gzip(&pack_raw(&pairs)), "m2b.gz");
    let mut checks = 0;
    assert!(
        extract_bundle(cancelled.path(), &path, &archive, &expected, &mut || {
            checks += 1;
            ensure!(checks < 24, "cancelled producer");
            Ok(())
        })
        .is_err()
    );
    let count =
        || fs::read_dir(cancelled.path().join(".update/downloads")).map_or(0, |r| r.count());
    let completed = count();
    std::thread::sleep(std::time::Duration::from_millis(30));
    assert_eq!(
        count(),
        completed,
        "no cache writer survives cancellation return"
    );
    assert!(!cancelled.path().join(".update/transaction.json").exists());
    assert!(!cancelled.path().join("game").exists());
}

#[test]
fn bundles_reject_traversal_unknown_duplicate_alias_length_count_and_size() {
    let first = entry("mir2-assets/a.json", b"a");
    let second = entry("mir2-assets/b.json", b"b");
    let good = pack_raw(&[(&first.path, b"a"), (&second.path, b"b")]);
    let mut bad_count = good.clone();
    bad_count[8..12].copy_from_slice(&1u32.to_le_bytes());
    let mut bad_size = good.clone();
    let size_offset = 14 + first.path.len();
    bad_size[size_offset..size_offset + 8].copy_from_slice(&2u64.to_le_bytes());
    let mut bad_length = good.clone();
    bad_length[12..14].copy_from_slice(&231u16.to_le_bytes());
    let mut bad_magic = good.clone();
    bad_magic[0] ^= 1;
    let cases = [
        pack_raw(&[("../escape.json", b"a"), (&second.path, b"b")]),
        pack_raw(&[("mir2-assets/unknown.json", b"a"), (&second.path, b"b")]),
        pack_raw(&[(&first.path, b"a"), (&first.path, b"a")]),
        pack_raw(&[("mir2-assets/A.json", b"a"), (&second.path, b"b")]),
        bad_count,
        bad_size,
        bad_length,
        bad_magic,
    ];
    for (index, raw) in cases.into_iter().enumerate() {
        let root = TempDir::new().unwrap();
        let (path, archive) = write_artifact(root.path(), &gzip(&raw), "m2b.gz");
        assert!(
            extract_bundle(
                root.path(),
                &path,
                &archive,
                &[&first, &second],
                &mut || Ok(())
            )
            .is_err(),
            "case {index}"
        );
        assert!(!root.path().join("escape.json").exists());
        assert!(!root.path().join("game").exists());
    }
}

#[test]
fn bundles_reject_tampered_truncated_crc_and_both_kinds_of_trailing_bytes() {
    let target = entry("mir2-assets/a.json", b"data");
    let raw = pack_raw(&[(&target.path, b"data")]);
    let good = gzip(&raw);
    let mut bad_content = raw.clone();
    *bad_content.last_mut().unwrap() ^= 1;
    let mut trailing = raw.clone();
    trailing.push(0);
    let mut compressed_trailing = good.clone();
    compressed_trailing.push(0);
    let mut concatenated = good.clone();
    concatenated.extend(gzip(b"extra"));
    let mut crc = good.clone();
    let index = crc.len() - 8;
    crc[index] ^= 1;
    for bytes in [
        gzip(&bad_content),
        good[..good.len() - 3].to_vec(),
        gzip(&trailing),
        compressed_trailing,
        concatenated,
        crc,
    ] {
        let root = TempDir::new().unwrap();
        let (path, archive) = write_artifact(root.path(), &bytes, "m2b.gz");
        assert!(extract_bundle(root.path(), &path, &archive, &[&target], &mut || Ok(())).is_err());
        let cache = payload_cache(root.path(), &target).unwrap();
        if cache.exists() {
            assert!(safe::matches(&cache, &target).unwrap());
        }
        assert!(!cache.with_extension("delivery-part").exists());
    }
    let root = TempDir::new().unwrap();
    let (path, archive) = write_artifact(root.path(), &good, "m2b.gz");
    let mut changed = good.clone();
    changed[0] ^= 1;
    fs::write(&path, changed).unwrap();
    assert!(extract_bundle(root.path(), &path, &archive, &[&target], &mut || Ok(())).is_err());
    assert!(!payload_cache(root.path(), &target).unwrap().exists());
}

#[test]
fn bundles_enforce_expanded_and_case_collision_bounds_before_writing() {
    let root = TempDir::new().unwrap();
    let first = FileEntry {
        path: "mir2-assets/a.json".into(),
        size: MAX_FILE,
        sha256: hash(b"x"),
    };
    let second = entry("mir2-assets/b.json", b"x");
    let archive = entry("delivery/a.m2b.gz", b"x");
    assert!(extract_bundle(
        root.path(),
        &root.path().join("missing"),
        &archive,
        &[&first, &second],
        &mut || Ok(())
    )
    .is_err());
    let mut alias = second.clone();
    alias.path = "mir2-assets/B.json".into();
    assert!(extract_bundle(
        root.path(),
        &root.path().join("missing"),
        &archive,
        &[&second, &alias],
        &mut || Ok(())
    )
    .is_err());
    assert!(!root.path().join(".update").exists());
}

fn delta_fixture(
    raw: &[u8],
    base_bytes: &[u8],
    target_bytes: &[u8],
) -> (TempDir, PathBuf, FileEntry, PathBuf, FileEntry, FileEntry) {
    let root = TempDir::new().unwrap();
    let (patch_path, patch) = write_artifact(root.path(), &gzip(raw), "m2d.gz");
    let base = entry("mir2-platform-windows.exe", base_bytes);
    let target = entry(&base.path, target_bytes);
    let base_path = root.path().join("game").join(&base.path);
    safe::write_new(&base_path, base_bytes).unwrap();
    (root, patch_path, patch, base_path, base, target)
}

#[test]
fn delta_copy_literal_reconstructs_exact_target_and_preserves_base() {
    let target_bytes = b"abcINSERTxyz";
    let raw = delta_raw(
        target_bytes.len() as u64,
        &[copy_op(0, 3), literal_op(b"INSERT"), copy_op(3, 3)],
    );
    let (root, path, patch, base_path, base, target) = delta_fixture(&raw, b"abcxyz", target_bytes);
    apply_delta(
        root.path(),
        &path,
        &patch,
        &base_path,
        &base,
        &target,
        &mut || Ok(()),
    )
    .unwrap();
    assert!(safe::matches(&payload_cache(root.path(), &target).unwrap(), &target).unwrap());
    assert_eq!(fs::read(base_path).unwrap(), b"abcxyz");
}

#[test]
fn delta_rejects_base_mismatch_offset_overflow_outside_copy_and_unknown_ops() {
    let mut unknown = literal_op(b"abc");
    unknown[0] = 2;
    for ops in [
        vec![copy_op(2, 3)],
        vec![copy_op(u64::MAX, 3)],
        vec![unknown],
        vec![copy_op(0, 0)],
        vec![literal_op(b"abcd")],
    ] {
        let raw = delta_raw(3, &ops);
        let (root, path, patch, base_path, base, target) = delta_fixture(&raw, b"abc", b"abc");
        assert!(apply_delta(
            root.path(),
            &path,
            &patch,
            &base_path,
            &base,
            &target,
            &mut || Ok(())
        )
        .is_err());
        assert_eq!(fs::read(base_path).unwrap(), b"abc");
        assert!(!payload_cache(root.path(), &target).unwrap().exists());
    }
    let raw = delta_raw(3, &[copy_op(0, 3)]);
    let (root, path, patch, base_path, mut base, target) = delta_fixture(&raw, b"abc", b"abc");
    base.sha256 = hash(b"wrong");
    assert!(apply_delta(
        root.path(),
        &path,
        &patch,
        &base_path,
        &base,
        &target,
        &mut || Ok(())
    )
    .is_err());
    assert!(!root.path().join(".update/downloads").exists());
}

#[test]
fn delta_rejects_truncation_hash_size_operation_count_and_trailing_data() {
    let good = delta_raw(3, &[literal_op(b"abc")]);
    let mut count = good.clone();
    count[8..12].copy_from_slice(&(MAX_OPERATIONS + 1).to_le_bytes());
    let mut size = good.clone();
    size[12..20].copy_from_slice(&4u64.to_le_bytes());
    let mut trailing = good.clone();
    trailing.push(0);
    for raw in [
        good[..good.len() - 1].to_vec(),
        delta_raw(3, &[]),
        delta_raw(3, &[literal_op(b"abd")]),
        count,
        size,
        trailing,
    ] {
        let (root, path, patch, base_path, base, target) = delta_fixture(&raw, b"abc", b"abc");
        assert!(apply_delta(
            root.path(),
            &path,
            &patch,
            &base_path,
            &base,
            &target,
            &mut || Ok(())
        )
        .is_err());
        let cache = payload_cache(root.path(), &target).unwrap();
        assert!(!cache.exists());
        assert!(!cache.with_extension("delivery-part").exists());
    }
    let (root, path, patch, base_path, base, target) = delta_fixture(&good, b"abc", b"abc");
    let mut compressed = safe::read_bounded(&path, MAX_COMPRESSED).unwrap();
    compressed.extend(gzip(b"extra"));
    let (new_path, new_patch) = write_artifact(root.path(), &compressed, "m2d.gz");
    assert!(apply_delta(
        root.path(),
        &new_path,
        &new_patch,
        &base_path,
        &base,
        &target,
        &mut || Ok(())
    )
    .is_err());
    assert!(safe::matches(&path, &patch).unwrap());
}

#[test]
fn cancellation_and_hard_linked_cache_do_not_modify_targets() {
    let target = entry("mir2-assets/a.json", &vec![b'a'; 100_000]);
    let bytes = gzip(&pack_raw(&[(&target.path, &vec![b'a'; 100_000])]));
    let root = TempDir::new().unwrap();
    let (path, archive) = write_artifact(root.path(), &bytes, "m2b.gz");
    let mut checks = 0;
    assert!(
        extract_bundle(root.path(), &path, &archive, &[&target], &mut || {
            checks += 1;
            ensure!(checks < 4, "cancelled");
            Ok(())
        })
        .is_err()
    );
    let cache = payload_cache(root.path(), &target).unwrap();
    assert!(!cache.exists());
    assert!(!cache.with_extension("delivery-part").exists());
    let outside = root.path().join("personal.json");
    safe::write_new(&outside, b"personal").unwrap();
    fs::create_dir_all(cache.parent().unwrap()).unwrap();
    fs::hard_link(&outside, &cache).unwrap();
    assert!(extract_bundle(root.path(), &path, &archive, &[&target], &mut || Ok(())).is_err());
    assert_eq!(fs::read(outside).unwrap(), b"personal");
}

#[test]
fn binary_delta_has_measured_small_transfer_and_exact_output() {
    let mut state = 0x12345678u32;
    let base_bytes = (0..1_048_576)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect::<Vec<_>>();
    let mut target_bytes = base_bytes.clone();
    let start = 512_000usize;
    target_bytes[start..start + 256].fill(0xAB);
    let raw = delta_raw(
        target_bytes.len() as u64,
        &[
            copy_op(0, start as u64),
            literal_op(&target_bytes[start..start + 256]),
            copy_op(
                (start + 256) as u64,
                (target_bytes.len() - start - 256) as u64,
            ),
        ],
    );
    let (root, path, patch, base_path, base, target) =
        delta_fixture(&raw, &base_bytes, &target_bytes);
    let started = Instant::now();
    apply_delta(
        root.path(),
        &path,
        &patch,
        &base_path,
        &base,
        &target,
        &mut || Ok(()),
    )
    .unwrap();
    assert!(patch.size < target.size / 1000);
    assert!(safe::matches(&payload_cache(root.path(), &target).unwrap(), &target).unwrap());
    println!(
        "delta_measurement patchBytes={} targetBytes={} elapsedMs={}",
        patch.size,
        target.size,
        started.elapsed().as_millis()
    );
}

#[derive(Default)]
struct Quiet {
    cancelled: AtomicBool,
}
impl Status for Quiet {
    fn set(&self, _: &str, _: u32) {}
    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

struct MemorySource {
    files: BTreeMap<String, Vec<u8>>,
    downloads: RefCell<Vec<String>>,
    gets: RefCell<Vec<String>>,
    fail_after: BTreeMap<String, usize>,
}
impl MemorySource {
    fn new(files: BTreeMap<String, Vec<u8>>) -> Self {
        Self {
            files,
            downloads: RefCell::new(vec![]),
            gets: RefCell::new(vec![]),
            fail_after: BTreeMap::new(),
        }
    }
}
impl Source for MemorySource {
    fn get(
        &self,
        path: &str,
        limit: u64,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Vec<u8>> {
        self.gets.borrow_mut().push(path.into());
        let bytes = self.files.get(path).context("optional fixture absent")?;
        ensure!(bytes.len() as u64 <= limit, "fixture response bound");
        progress(bytes.len() as u64)?;
        Ok(bytes.clone())
    }
    fn download(
        &self,
        path: &str,
        _: &FileEntry,
        destination: &Path,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()> {
        self.downloads.borrow_mut().push(path.into());
        let bytes = self.files.get(path).context("fixture absent")?;
        if let Some(count) = self.fail_after.get(path) {
            let count = (*count).min(bytes.len());
            safe::write_new(destination, &bytes[..count])?;
            progress(count as u64)?;
            anyhow::bail!("injected interrupted transfer");
        }
        safe::write_new(destination, bytes)?;
        progress(bytes.len() as u64 / 2)?;
        progress(bytes.len() as u64 / 2)?;
        progress(bytes.len() as u64)?;
        // Deliberately leave verification to cached_payload as a regression
        // against a source adapter that accidentally trusts transport bytes.
        Ok(())
    }
}

fn component() -> Component {
    Component {
        directory: "releases/game-test".into(),
        identity: "candidate-test".into(),
        metadata: vec![],
    }
}

fn descriptor(bundle: Bundle) -> Delivery {
    Delivery {
        schema: "mir2.windows.delivery.v1".into(),
        candidate: "candidate-test".into(),
        package_manifest_sha256: hash(b"manifest"),
        version_sha256: hash(b"version"),
        built_unix: update::now(),
        bundles: vec![bundle],
        patches: vec![],
    }
}

fn cache_and_stage(
    root: &Path,
    source: &impl Source,
    files: &[FileEntry],
    transfers: &mut update::Transfers,
) -> Vec<crate::transaction::Change> {
    let total = files.iter().map(|f| f.size).sum();
    files
        .iter()
        .map(|file| {
            let (cache, _) = update::cached_payload(
                root,
                source,
                &format!("{}/{}", component().directory, file.path),
                file,
                &Quiet::default(),
                (0, total),
                transfers,
            )
            .unwrap();
            let path = format!("game/{}", file.path);
            let target = safe::target(&root.join(".update/staging"), &path).unwrap();
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(cache, target).unwrap();
            let mut staged = file.clone();
            staged.path = path.clone();
            crate::transaction::Change {
                path,
                new: Some(staged),
            }
        })
        .collect()
}

#[test]
fn initial_bundle_avoids_hundreds_of_requests_and_transaction_preserves_personal_files() {
    let root = TempDir::new().unwrap();
    let started = Instant::now();
    let mut data = vec![
        ("mir2-platform-windows.exe".to_owned(), b"new exe".to_vec()),
        ("BUILD-ATTESTATION.json".to_owned(), b"attestation".to_vec()),
    ];
    for index in 0..600 {
        data.push((
            format!("mir2-assets/frames/{index}.json"),
            format!(
                "{{\"i\":{index},\"pad\":\"{}\"}}",
                "abcdefghijklmnop".repeat(64)
            )
            .into_bytes(),
        ));
    }
    let files = data
        .iter()
        .map(|(path, bytes)| entry(path, bytes))
        .collect::<Vec<_>>();
    let manifest = manifest(&files);
    let compressed = gzip(&pack_raw(
        &data
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
            .collect::<Vec<_>>(),
    ));
    let archive = entry("delivery/initial.m2b.gz", &compressed);
    let bundle = descriptor(Bundle {
        archive: archive.clone(),
        files: files.iter().map(|f| f.path.clone()).collect(),
    });
    let source = MemorySource::new(
        [(
            format!("{}/{}", component().directory, archive.path),
            compressed,
        )]
        .into_iter()
        .collect(),
    );
    safe::write_new(&root.path().join("game/personal-file.txt"), b"personal").unwrap();
    safe::write_new(&root.path().join("preferences/locale.json"), b"zh-TW").unwrap();
    let mut transfers = update::Transfers::default();
    let mut stats = Stats::default();
    Acceleration {
        root: root.path(),
        source: &source,
        component: &component(),
        status: &Quiet::default(),
        total: manifest.total_bytes,
        transfers: &mut transfers,
        stats: &mut stats,
    }
    .run(&bundle, &manifest, &files)
    .unwrap();
    let changes = cache_and_stage(root.path(), &source, &files, &mut transfers);
    crate::transaction::prepare_and_apply(root.path(), &changes).unwrap();
    for file in &files {
        assert!(safe::matches(&root.path().join("game").join(&file.path), file).unwrap());
    }
    assert_eq!(source.downloads.borrow().len(), 1);
    assert_eq!(stats.bundles, 1);
    assert_eq!(stats.fallbacks, 0);
    assert_eq!(transfers.downloaded_files, 1);
    assert_eq!(transfers.downloaded_bytes, archive.size);
    assert_eq!(transfers.wire_bytes, archive.size);
    assert_eq!(
        fs::read(root.path().join("game/personal-file.txt")).unwrap(),
        b"personal"
    );
    assert_eq!(
        fs::read(root.path().join("preferences/locale.json")).unwrap(),
        b"zh-TW"
    );
    crate::transaction::rollback(root.path()).unwrap();
    assert!(!root.path().join("game/mir2-platform-windows.exe").exists());
    assert_eq!(
        fs::read(root.path().join("game/personal-file.txt")).unwrap(),
        b"personal"
    );
    assert_eq!(
        fs::read(root.path().join("preferences/locale.json")).unwrap(),
        b"zh-TW"
    );
    println!("bundle_measurement files={} requests=1 compressedBytes={} targetBytes={} elapsedMs={} rollbackToSeed=true",files.len(),archive.size,manifest.total_bytes,started.elapsed().as_millis());
}

#[test]
fn corrupt_bundle_falls_back_counts_discarded_bytes_and_reuses_verified_partial_entries() {
    for corruption in ["entry", "compressed", "interrupted"] {
        let root = TempDir::new().unwrap();
        let data = [
            ("mir2-platform-windows.exe", b"new exe".as_slice()),
            ("BUILD-ATTESTATION.json", b"attestation".as_slice()),
        ];
        let files = data
            .iter()
            .map(|(path, bytes)| entry(path, bytes))
            .collect::<Vec<_>>();
        let manifest = manifest(&files);
        let mut raw = pack_raw(&data);
        if corruption == "entry" {
            *raw.last_mut().unwrap() ^= 1;
        }
        let good = gzip(&raw);
        let archive = entry("delivery/corrupt.m2b.gz", &good);
        let bundle = descriptor(Bundle {
            archive: archive.clone(),
            files: files.iter().map(|f| f.path.clone()).collect(),
        });
        let archive_path = format!("{}/{}", component().directory, archive.path);
        let mut compressed = good.clone();
        if corruption == "compressed" {
            compressed[0] ^= 1;
        }
        let mut objects = data
            .iter()
            .map(|(path, bytes)| {
                (
                    format!("{}/{}", component().directory, path),
                    bytes.to_vec(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        objects.insert(archive_path.clone(), compressed);
        let mut source = MemorySource::new(objects);
        if corruption == "interrupted" {
            source.fail_after.insert(archive_path.clone(), 7);
        }
        let mut transfers = update::Transfers::default();
        let mut stats = Stats::default();
        Acceleration {
            root: root.path(),
            source: &source,
            component: &component(),
            status: &Quiet::default(),
            total: manifest.total_bytes,
            transfers: &mut transfers,
            stats: &mut stats,
        }
        .run(&bundle, &manifest, &files)
        .unwrap();
        cache_and_stage(root.path(), &source, &files, &mut transfers);
        assert_eq!(stats.bundles, 0);
        assert_eq!(stats.fallbacks, 1);
        assert_eq!(
            transfers.downloaded_files,
            if corruption == "entry" { 2 } else { 3 }
        );
        let discarded = if corruption == "interrupted" {
            7
        } else {
            archive.size
        };
        let raw_downloaded = if corruption == "entry" {
            files[1].size
        } else {
            manifest.total_bytes
        };
        assert_eq!(
            transfers.downloaded_bytes,
            discarded + raw_downloaded,
            "{corruption}"
        );
        assert_eq!(transfers.wire_bytes, transfers.downloaded_bytes);
        for file in &files {
            assert!(safe::matches(&payload_cache(root.path(), file).unwrap(), file).unwrap());
        }
        assert!(!payload_cache(root.path(), &archive)
            .unwrap()
            .with_extension("part")
            .exists());
    }
}

#[test]
fn invalid_or_missing_optional_signature_never_fetches_accelerators() {
    let root = TempDir::new().unwrap();
    let files = [
        entry("mir2-platform-windows.exe", b"exe"),
        entry("BUILD-ATTESTATION.json", b"attest"),
    ];
    let manifest = manifest(&files);
    let bundle = descriptor(Bundle {
        archive: entry("delivery/a.m2b.gz", b"gzip"),
        files: files.iter().map(|f| f.path.clone()).collect(),
    });
    let metadata = [
        ("PACKAGE-MANIFEST.json".into(), b"manifest".to_vec()),
        ("VERSION.json".into(), b"version".to_vec()),
    ]
    .into_iter()
    .collect();
    for signature in [Some(b"not a CMS signature".to_vec()), None] {
        let mut objects = BTreeMap::from([(
            format!("{}/DELIVERY.json", component().directory),
            serde_json::to_vec(&bundle).unwrap(),
        )]);
        if let Some(signature) = signature {
            objects.insert(format!("{}/DELIVERY.p7s", component().directory), signature);
        }
        let source = MemorySource::new(objects);
        let mut transfers = update::Transfers::default();
        let mut stats = Stats::default();
        assert!(update::optional_delivery(
            root.path(),
            &source,
            &component(),
            (&metadata, &manifest),
            &Quiet::default(),
            &mut transfers,
            &mut stats
        )
        .unwrap()
        .is_none());
        assert_eq!(stats.fallbacks, 1);
        assert!(source.downloads.borrow().is_empty());
        assert_eq!(source.gets.borrow().len(), 2);
    }
}

#[test]
fn small_asset_repair_does_not_download_an_initial_bundle() {
    let root = TempDir::new().unwrap();
    let files = [
        entry("mir2-platform-windows.exe", b"exe"),
        entry("BUILD-ATTESTATION.json", b"attest"),
        entry("mir2-assets/a.json", b"asset"),
    ];
    let manifest = manifest(&files);
    let bundle = descriptor(Bundle {
        archive: entry("delivery/initial.m2b.gz", b"unneeded"),
        files: files.iter().map(|f| f.path.clone()).collect(),
    });
    let source = MemorySource::new(BTreeMap::from([(
        format!("{}/{}", component().directory, files[2].path),
        b"asset".to_vec(),
    )]));
    let mut transfers = update::Transfers::default();
    let mut stats = Stats::default();
    Acceleration {
        root: root.path(),
        source: &source,
        component: &component(),
        status: &Quiet::default(),
        total: files[2].size,
        transfers: &mut transfers,
        stats: &mut stats,
    }
    .run(&bundle, &manifest, &files[2..])
    .unwrap();
    cache_and_stage(root.path(), &source, &files[2..], &mut transfers);
    assert_eq!(stats.bundles, 0);
    assert_eq!(source.downloads.borrow().len(), 1);
    assert!(source.downloads.borrow()[0].ends_with("/mir2-assets/a.json"));
}

#[test]
fn delta_base_mismatch_and_bad_patch_fall_back_without_touching_live_base() {
    let base_bytes = vec![b'a'; 2000];
    let target_bytes = vec![b'b'; 2000];
    let files = [
        entry("mir2-platform-windows.exe", &target_bytes),
        entry("BUILD-ATTESTATION.json", b"attest"),
    ];
    let manifest = manifest(&files);
    for bad_base in [true, false] {
        let root = TempDir::new().unwrap();
        let base_path = root.path().join("game/mir2-platform-windows.exe");
        safe::write_new(&base_path, &base_bytes).unwrap();
        let compressed = gzip(&delta_raw(
            target_bytes.len() as u64,
            &[copy_op(u64::MAX, target_bytes.len() as u64)],
        ));
        let patch = entry("delivery/exe.m2d.gz", &compressed);
        let descriptor = Delivery {
            schema: "mir2.windows.delivery.v1".into(),
            candidate: "candidate-test".into(),
            package_manifest_sha256: hash(b"manifest"),
            version_sha256: hash(b"version"),
            built_unix: update::now(),
            bundles: vec![],
            patches: vec![Delta {
                path: files[0].path.clone(),
                base_size: base_bytes.len() as u64,
                base_sha256: hash(if bad_base {
                    b"wrong".as_slice()
                } else {
                    &base_bytes
                }),
                patch: patch.clone(),
            }],
        };
        let source = MemorySource::new(BTreeMap::from([
            (
                format!("{}/{}", component().directory, patch.path),
                compressed,
            ),
            (
                format!("{}/{}", component().directory, files[0].path),
                target_bytes.clone(),
            ),
        ]));
        let mut transfers = update::Transfers::default();
        let mut stats = Stats::default();
        Acceleration {
            root: root.path(),
            source: &source,
            component: &component(),
            status: &Quiet::default(),
            total: files[0].size,
            transfers: &mut transfers,
            stats: &mut stats,
        }
        .run(&descriptor, &manifest, &files[..1])
        .unwrap();
        let changes = cache_and_stage(root.path(), &source, &files[..1], &mut transfers);
        assert_eq!(fs::read(&base_path).unwrap(), base_bytes);
        assert_eq!(
            transfers.downloaded_bytes,
            files[0].size + if bad_base { 0 } else { patch.size }
        );
        assert_eq!(stats.patches, 0);
        assert_eq!(stats.fallbacks, usize::from(!bad_base));
        crate::transaction::prepare_and_apply(root.path(), &changes).unwrap();
        assert!(safe::matches(&base_path, &files[0]).unwrap());
        crate::transaction::rollback(root.path()).unwrap();
        assert_eq!(fs::read(base_path).unwrap(), base_bytes);
    }
}

#[test]
fn pinned_source_encodes_object_identity_and_rejects_url_syntax() {
    let base = url::Url::parse("https://assets.mir2.obelisk.build/client-updates/").unwrap();
    for path in [
        "../latest.json",
        "/latest.json",
        "https://outside.invalid/x",
        "C:/x",
        "a\\b",
        "a/./b",
    ] {
        assert!(
            update::HttpsSource::object_url(&base, path).is_err(),
            "{path}"
        );
    }
    let url =
        update::HttpsSource::object_url(&base, "releases/game/asset%2Fname#1 file.json").unwrap();
    assert_eq!(url.as_str(),"https://assets.mir2.obelisk.build/client-updates/releases/game/asset%252Fname%231%20file.json");
    assert_eq!(url.origin(), base.origin());
    for feed in [
        "http://outside.invalid/latest.json",
        "https://u:p@outside.invalid/latest.json",
        "https://outside.invalid/latest.json?q=x",
        "https://outside.invalid/latest.json#x",
    ] {
        assert!(update::HttpsSource::pinned_base(feed).is_err());
    }
}

fn discovery_feed() -> Vec<u8> {
    let meta = |names: &[&str]| names.iter().map(|path| entry(path, b"x")).collect();
    serde_json::to_vec(&Feed {
        schema: "mir2.windows.update-feed.v1".into(),
        channel: "invited".into(),
        platform: "windows-x64".into(),
        sequence: 1,
        created_unix: update::now(),
        expires_unix: update::now() + 86400,
        min_bootstrap: 1,
        protocol: "crystal-mir2-v1".into(),
        content: "mir2.windows.package-manifest.v4".into(),
        game: Component {
            directory: "releases/game-test".into(),
            identity: "candidate-test".into(),
            metadata: meta(&META),
        },
        engine: Component {
            directory: "releases/updater-1".into(),
            identity: "1".into(),
            metadata: meta(&["ENGINE.json", "ENGINE.p7s"]),
        },
    })
    .unwrap()
}

#[test]
fn persistent_discovery_cms_failure_uses_three_pairs_and_never_activates() {
    let root = TempDir::new().unwrap();
    let bytes = discovery_feed();
    let sig = b"not CMS".to_vec();
    let source = MemorySource::new(BTreeMap::from([
        ("latest.json".into(), bytes.clone()),
        ("latest.p7s".into(), sig.clone()),
    ]));
    let mut transfers = update::Transfers::default();
    let error = update::fresh_discovery(root.path(), &source, &Quiet::default(), &mut transfers)
        .unwrap_err();
    assert!(error.to_string().contains("three fresh pairs"));
    assert_eq!(source.gets.borrow().len(), 6);
    assert_eq!(transfers.wire_bytes, 3 * (bytes.len() + sig.len()) as u64);
    assert!(source.downloads.borrow().is_empty());
    assert!(!root.path().join("game").exists());
    assert!(!root.path().join(".update/transaction.json").exists());
    assert!(!root.path().join(".update/highest.txt").exists());
    // Parsing/compatibility failures are not mistaken for a promotion race.
    let mut bad: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    bad["schema"] = "wrong".into();
    let source = MemorySource::new(BTreeMap::from([
        ("latest.json".into(), serde_json::to_vec(&bad).unwrap()),
        ("latest.p7s".into(), sig),
    ]));
    assert!(update::fresh_discovery(
        root.path(),
        &source,
        &Quiet::default(),
        &mut update::Transfers::default()
    )
    .is_err());
    assert_eq!(source.gets.borrow().len(), 2);
}

#[test]
fn discovery_progress_counts_partial_and_oversized_reads_once() {
    let mut received = 0;
    let result = update::discovery_object(
        |callback| {
            callback(4)?;
            callback(4)?;
            anyhow::bail!("interrupted")
        },
        &mut |bytes| {
            received += bytes;
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(received, 4);
    let result = update::discovery_object(
        |callback| {
            callback(32769)?;
            Ok(vec![])
        },
        &mut |bytes| {
            received += bytes;
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(received, 4 + 32769);
}

// These opt-in tests consume actual production-pin CMS artifacts. They never
// generate signatures, invoke a game/updater process, or touch an installation.
#[cfg(windows)]
mod signed_packages {
    use super::*;

    struct Files {
        feed: PathBuf,
        game: PathBuf,
        engine: PathBuf,
        delivery: PathBuf,
        game_directory: String,
        engine_directory: String,
        downloads: RefCell<Vec<String>>,
        gets: RefCell<Vec<String>>,
        tamper_delivery_signature: bool,
    }
    impl Files {
        fn file(&self, path: &str) -> PathBuf {
            if let Some(rest) = path.strip_prefix(&format!("{}/", self.game_directory)) {
                if rest.starts_with("delivery/")
                    || ["DELIVERY.json", "DELIVERY.p7s"].contains(&rest)
                {
                    self.delivery.join(rest)
                } else {
                    self.game.join(rest)
                }
            } else if let Some(rest) = path.strip_prefix(&format!("{}/", self.engine_directory)) {
                self.engine.join(rest)
            } else {
                self.feed.join(path)
            }
        }
    }
    impl Source for Files {
        fn get(
            &self,
            path: &str,
            limit: u64,
            progress: &mut dyn FnMut(u64) -> Result<()>,
        ) -> Result<Vec<u8>> {
            relative(path)?;
            self.gets.borrow_mut().push(path.into());
            let mut bytes = safe::read_bounded(&self.file(path), limit)?;
            if self.tamper_delivery_signature && path.ends_with("/DELIVERY.p7s") {
                *bytes.last_mut().context("empty fixture signature")? ^= 1;
            }
            progress(bytes.len() as u64)?;
            Ok(bytes)
        }
        fn download(
            &self,
            path: &str,
            file: &FileEntry,
            destination: &Path,
            progress: &mut dyn FnMut(u64) -> Result<()>,
        ) -> Result<()> {
            relative(path)?;
            self.downloads.borrow_mut().push(path.into());
            let input = self.file(path);
            ensure!(
                safe::matches(&input, file)?,
                "signed fixture artifact hash mismatch"
            );
            safe::ancestors(destination)?;
            fs::create_dir_all(destination.parent().unwrap())?;
            fs::copy(input, destination)?;
            progress(file.size)?;
            Ok(())
        }
    }
    fn config() -> serde_json::Value {
        let path = std::env::var_os("MIR2_UPDATER_DELIVERY_FIXTURES")
            .expect("set public signed fixture config");
        serde_json::from_slice(&safe::read_bounded(&PathBuf::from(path), 32768).unwrap()).unwrap()
    }
    fn path(config: &serde_json::Value, key: &str) -> PathBuf {
        PathBuf::from(config[key].as_str().unwrap())
    }
    fn copy_tree(input: &Path, output: &Path) {
        safe::ancestors(input).unwrap();
        safe::ancestors(output).unwrap();
        fs::create_dir_all(output).unwrap();
        for item in fs::read_dir(input).unwrap() {
            let item = item.unwrap();
            let input = item.path();
            let output = output.join(item.file_name());
            if item.file_type().unwrap().is_dir() {
                copy_tree(&input, &output);
            } else {
                safe::regular(&input).unwrap();
                fs::copy(&input, output).unwrap();
            }
        }
    }
    fn source(config: &serde_json::Value) -> Files {
        let feed = path(config, "feed");
        let parsed: serde_json::Value =
            serde_json::from_slice(&safe::read_bounded(&feed.join("latest.json"), 32768).unwrap())
                .unwrap();
        Files {
            feed,
            game: path(config, "newPackage"),
            engine: path(config, "engine"),
            delivery: path(config, "deliveryDirectory"),
            game_directory: parsed["game"]["directory"].as_str().unwrap().into(),
            engine_directory: parsed["engine"]["directory"].as_str().unwrap().into(),
            downloads: RefCell::new(vec![]),
            gets: RefCell::new(vec![]),
            tamper_delivery_signature: false,
        }
    }
    fn fresh_root(config: &serde_json::Value, name: &str) -> PathBuf {
        let root = path(config, "output").join(name);
        assert!(!root.exists(), "use a fresh owned fixture output");
        fs::create_dir_all(&root).unwrap();
        root
    }
    fn personal(root: &Path) {
        safe::write_new(&root.join("game/logs/retained.log"), b"retained diagnostic").unwrap();
        safe::write_new(&root.join("game/personal-file.txt"), b"personal file").unwrap();
        safe::write_new(&root.join("preferences/locale.json"), b"zh-TW").unwrap();
        safe::write_new(
            &root.join("preferences/settings.json"),
            b"personal settings",
        )
        .unwrap();
    }
    fn preserved(root: &Path) {
        assert_eq!(
            fs::read(root.join("game/logs/retained.log")).unwrap(),
            b"retained diagnostic"
        );
        assert_eq!(
            fs::read(root.join("game/personal-file.txt")).unwrap(),
            b"personal file"
        );
        assert_eq!(
            fs::read(root.join("preferences/locale.json")).unwrap(),
            b"zh-TW"
        );
        assert_eq!(
            fs::read(root.join("preferences/settings.json")).unwrap(),
            b"personal settings"
        );
    }
    fn verify_all(root: &Path, manifest: &Manifest) {
        for entry in &manifest.files {
            assert!(
                safe::matches(&root.join("game").join(&entry.path), entry).unwrap(),
                "{}",
                entry.path
            );
        }
    }
    fn report(root: &Path, name: &str, value: serde_json::Value) {
        safe::write_new(
            &root.parent().unwrap().join(name),
            &serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }

    #[test]
    #[ignore = "requires MIR2_UPDATER_DELIVERY_FIXTURES with distinct valid old/current signed feeds"]
    fn signed_discovery_promotion_race_recovers_and_permanent_tampering_rejects() {
        struct Race {
            old: Vec<u8>,
            new: Vec<u8>,
            signature: Vec<u8>,
            requests: RefCell<Vec<String>>,
            persistent_bad: bool,
        }
        impl Source for Race {
            fn get(
                &self,
                path: &str,
                limit: u64,
                progress: &mut dyn FnMut(u64) -> Result<()>,
            ) -> Result<Vec<u8>> {
                let first = self.requests.borrow().is_empty();
                self.requests.borrow_mut().push(path.into());
                let bytes = if path == "latest.json" {
                    if first && !self.persistent_bad {
                        &self.old
                    } else {
                        &self.new
                    }
                } else {
                    &self.signature
                };
                ensure!(bytes.len() as u64 <= limit, "fixture bound");
                progress(bytes.len() as u64)?;
                Ok(bytes.clone())
            }
            fn download(
                &self,
                _: &str,
                _: &FileEntry,
                _: &Path,
                _: &mut dyn FnMut(u64) -> Result<()>,
            ) -> Result<()> {
                anyhow::bail!("discovery must not request payloads")
            }
        }
        let config = config();
        let root = fresh_root(&config, "discovery-race");
        let feed = path(&config, "feed");
        let old = safe::read_bounded(&path(&config, "oldSeed"), 32768).unwrap();
        let new = safe::read_bounded(&feed.join("latest.json"), 32768).unwrap();
        let signature = safe::read_bounded(&feed.join("latest.p7s"), 32768).unwrap();
        assert_ne!(old, new);
        let race = Race {
            old,
            new,
            signature,
            requests: RefCell::new(vec![]),
            persistent_bad: false,
        };
        let mut transfers = update::Transfers::default();
        let (_, accepted, _) =
            update::fresh_discovery(&root, &race, &Quiet::default(), &mut transfers).unwrap();
        assert_eq!(accepted, race.new);
        assert_eq!(race.requests.borrow().len(), 4);
        assert_eq!(
            transfers.wire_bytes,
            (race.old.len() + race.new.len() + 2 * race.signature.len()) as u64
        );
        assert!(!root.join(".update/transaction.json").exists());
        let accepted_wire_bytes = transfers.wire_bytes;
        let mut bad = race;
        *bad.signature.last_mut().unwrap() ^= 1;
        bad.persistent_bad = true;
        bad.requests.borrow_mut().clear();
        let mut rejected = update::Transfers::default();
        assert!(update::fresh_discovery(&root, &bad, &Quiet::default(), &mut rejected).is_err());
        assert_eq!(bad.requests.borrow().len(), 6);
        assert!(!root.join("game").exists());
        assert!(!root.join(".update/highest.txt").exists());
        report(
            &root,
            "signed-discovery-report.json",
            serde_json::json!({"source":"local-signed-feeds","passed":true,"racePairs":2,"raceWireBytes":accepted_wire_bytes,"permanentTamperPairs":3,"tamperWireBytes":rejected.wire_bytes,"activated":false,"gameLaunched":false}),
        );
    }

    #[test]
    #[ignore = "requires MIR2_UPDATER_DELIVERY_FIXTURES with public signed game/delivery/engine artifacts"]
    fn signed_metadata_only_seed_uses_bundles_and_failed_first_launch_restores_incomplete_seed() {
        let config = config();
        let root = fresh_root(&config, "bundle-seed");
        let source = source(&config);
        copy_tree(&path(&config, "bundle"), &root);
        fs::create_dir_all(root.join("game")).unwrap();
        for name in META {
            fs::copy(source.game.join(name), root.join("game").join(name)).unwrap();
        }
        fs::copy(
            source.feed.join("latest.json"),
            root.join("updater/seed-feed.json"),
        )
        .unwrap();
        fs::copy(
            source.feed.join("latest.p7s"),
            root.join("updater/seed-feed.p7s"),
        )
        .unwrap();
        personal(&root);
        assert!(update::verify_launch(&root).is_err());
        let started = Instant::now();
        let outcome = update::check_update(&root, &source, &Quiet::default()).unwrap();
        let elapsed = started.elapsed().as_millis();
        let manifest = Manifest::parse(
            &safe::read_bounded(&source.game.join("PACKAGE-MANIFEST.json"), MAX_META).unwrap(),
        )
        .unwrap();
        assert!(outcome.delivery.bundles > 0);
        assert_eq!(outcome.changed_game_files, manifest.file_count);
        assert!(outcome.downloaded_files < manifest.file_count / 100);
        assert!(outcome.activated);
        verify_all(&root, &manifest);
        preserved(&root);
        let requests = source.downloads.borrow().len();
        let gets = source.gets.borrow().len();
        let pending = update::check_update(&root, &source, &Quiet::default()).unwrap();
        assert!(pending.activated);
        assert_eq!(pending.downloaded_bytes, 0);
        assert_eq!(pending.wire_bytes, 0);
        assert_eq!(source.downloads.borrow().len(), requests);
        assert_eq!(source.gets.borrow().len(), gets);
        update::quarantine_and_rollback(&root).unwrap();
        assert!(update::verify_launch(&root).is_err());
        assert!(!root.join("game/mir2-platform-windows.exe").exists());
        preserved(&root);
        assert!(update::check_update(&root, &source, &Quiet::default()).is_err());
        report(
            &root,
            "signed-bundle-seed-report.json",
            serde_json::json!({"source":"local-signed-package","passed":true,"outcome":outcome,"elapsedMs":elapsed,"allTargetHashesVerified":true,"preservedPersonalFiles":true,"pendingLaunchRecheckWireBytes":0,"rolledBackToIncompleteSeed":true,"incompleteSeedCannotLaunch":true,"failedReleaseQuarantined":true,"gameLaunched":false,"requests":source.downloads.borrow().as_slice()}),
        );
    }

    #[test]
    #[ignore = "requires MIR2_UPDATER_DELIVERY_FIXTURES whose old EXE exactly matches a signed delta base"]
    fn signed_delta_then_tampered_delivery_fallback_preserve_state_and_zero_payload_recheck() {
        let config = config();
        let root = fresh_root(&config, "delta-and-fallback");
        let old = path(&config, "oldPackage");
        let mut source = source(&config);
        copy_tree(&old, &root.join("game"));
        copy_tree(&path(&config, "bundle"), &root);
        fs::copy(
            path(&config, "oldSeed"),
            root.join("updater/seed-feed.json"),
        )
        .unwrap();
        fs::copy(
            path(&config, "oldSeedSignature"),
            root.join("updater/seed-feed.p7s"),
        )
        .unwrap();
        personal(&root);
        let old_manifest = Manifest::parse(
            &safe::read_bounded(&old.join("PACKAGE-MANIFEST.json"), MAX_META).unwrap(),
        )
        .unwrap();
        let new_manifest = Manifest::parse(
            &safe::read_bounded(&source.game.join("PACKAGE-MANIFEST.json"), MAX_META).unwrap(),
        )
        .unwrap();
        assert_ne!(
            old_manifest.map()["mir2-platform-windows.exe"].sha256,
            new_manifest.map()["mir2-platform-windows.exe"].sha256,
            "delta requires differing genuine Candidates"
        );
        let started = Instant::now();
        let delta = update::check_update(&root, &source, &Quiet::default()).unwrap();
        let delta_elapsed = started.elapsed().as_millis();
        assert!(delta.delivery.patches > 0);
        assert!(delta.downloaded_bytes < new_manifest.map()["mir2-platform-windows.exe"].size);
        verify_all(&root, &new_manifest);
        preserved(&root);
        crate::transaction::rollback(&root).unwrap();
        verify_all(&root, &old_manifest);
        preserved(&root);
        // Remove only known target cache files in this fresh owned test root;
        // otherwise safe reuse would hide the per-file fallback measurement.
        for file in &new_manifest.files {
            let cache = payload_cache(&root, file).unwrap();
            if cache.exists() {
                safe::regular(&cache).unwrap();
                fs::remove_file(cache).unwrap();
            }
        }
        source.tamper_delivery_signature = true;
        source.downloads.borrow_mut().clear();
        let started = Instant::now();
        let fallback = update::check_update(&root, &source, &Quiet::default()).unwrap();
        let fallback_elapsed = started.elapsed().as_millis();
        assert_eq!(fallback.delivery.patches, 0);
        assert_eq!(fallback.delivery.bundles, 0);
        assert_eq!(fallback.delivery.fallbacks, 1);
        assert!(source
            .downloads
            .borrow()
            .iter()
            .any(|p| p.ends_with("/mir2-platform-windows.exe")));
        assert!(!source
            .downloads
            .borrow()
            .iter()
            .any(|p| p.contains("/delivery/")));
        verify_all(&root, &new_manifest);
        preserved(&root);
        crate::transaction::accept(&root).unwrap();
        let delivery_gets = source
            .gets
            .borrow()
            .iter()
            .filter(|p| p.ends_with("/DELIVERY.json"))
            .count();
        let recheck = update::check_update(&root, &source, &Quiet::default()).unwrap();
        assert_eq!(recheck.downloaded_files, 0);
        assert_eq!(recheck.downloaded_bytes, 0);
        assert_eq!(recheck.changed_game_files, 0);
        assert_eq!(
            source
                .gets
                .borrow()
                .iter()
                .filter(|p| p.ends_with("/DELIVERY.json"))
                .count(),
            delivery_gets
        );
        report(
            &root,
            "signed-delta-and-fallback-report.json",
            serde_json::json!({"source":"local-signed-package","passed":true,"delta":delta,"deltaElapsedMs":delta_elapsed,"tamperedDescriptorFallback":fallback,"fallbackElapsedMs":fallback_elapsed,"acceptedRecheck":recheck,"allTargetHashesVerified":true,"rollbackVerified":true,"preservedPersonalFiles":true,"gameLaunched":false}),
        );
    }
}
