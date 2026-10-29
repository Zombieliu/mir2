use super::*;
use std::{collections::BTreeMap, sync::atomic::AtomicUsize};

fn entry(path: &str, bytes: &[u8]) -> FileEntry {
    FileEntry {
        path: path.into(),
        size: bytes.len() as u64,
        sha256: model::hash(bytes),
    }
}
fn manifest() -> GenerationManifest {
    let pack = entry(
        &format!(
            "objects/packs/{}.m2p",
            model::hash(b"pack").to_ascii_lowercase()
        ),
        b"pack",
    );
    GenerationManifest {
        schema: "mir2.windows.generation.v1".into(),
        sequence: 100,
        min_engine: 2,
        game_source_revision: "a".repeat(40),
        engine_source_revision: "b".repeat(40),
        protocol: "crystal-mir2-v1".into(),
        content_profile: "platinum_fiveclass".into(),
        exe: entry("mir2-platform-windows.exe", b"exe"),
        signed_config: entry("mir2-client.toml", b"config"),
        catalog: entry("catalog.json", b"{}"),
        packs: vec![pack],
    }
}
fn feed(manifest: &GenerationManifest) -> PackFeed {
    let bytes = serde_json::to_vec(manifest).unwrap();
    let digest = model::hash(&bytes);
    let base = directory(&digest).unwrap();
    let now = crate::update::now();
    PackFeed {
        schema: "mir2.windows.pack-feed.v1".into(),
        channel: "invited".into(),
        platform: "windows-x64".into(),
        sequence: manifest.sequence,
        created_unix: now,
        expires_unix: now + 3600,
        min_engine: 2,
        manifest: entry(&format!("{base}/GENERATION.json"), &bytes),
        signature: entry(&format!("{base}/GENERATION.p7s"), b"untrusted"),
    }
}
fn legacy() -> Selection {
    Selection::LegacyV4 {
        candidate: "known-legacy".into(),
        version_sha256: "A".repeat(64),
        exe_sha256: "B".repeat(64),
    }
}
fn pending() -> ActiveRecord {
    ActiveRecord {
        schema: "mir2.windows.active-generation.v1".into(),
        active: Selection::Generation {
            manifest_sha256: "C".repeat(64),
            sequence: 100,
        },
        previous: Some(legacy()),
        health: Health::Pending,
        attempt: "D".repeat(64),
        launch_started: false,
    }
}

#[test]
fn manifest_closes_paths_sources_engine_profile_counts_and_sizes() {
    let valid = manifest();
    assert!(GenerationManifest::parse(&serde_json::to_vec(&valid).unwrap()).is_ok());
    for index in 0..10 {
        let mut bad = valid.clone();
        match index {
            0 => bad.min_engine = crate::ENGINE_VERSION + 1,
            1 => bad.game_source_revision = "A".repeat(40),
            2 => bad.engine_source_revision = "../source".into(),
            3 => bad.exe.path = "../outside.exe".into(),
            4 => bad.signed_config.path = "userdata/config.toml".into(),
            5 => bad.catalog.size = MAX_CATALOG + 1,
            6 => bad.packs.push(bad.packs[0].clone()),
            7 => bad.packs[0].path = "objects/packs/unknown.m2p".into(),
            8 => bad.content_profile = "../profile".into(),
            9 => bad.packs[0].size = MAX_PACK + 1,
            _ => unreachable!(),
        }
        assert!(
            GenerationManifest::parse(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "case {index}"
        );
    }
    let mut unknown = serde_json::to_value(&valid).unwrap();
    unknown["url"] = "https://other.example/".into();
    assert!(GenerationManifest::parse(&serde_json::to_vec(&unknown).unwrap()).is_err());
}
#[test]
fn feed_expiry_namespace_signature_identity_and_bounds_fail_closed() {
    let valid = feed(&manifest());
    let now = crate::update::now();
    assert!(PackFeed::parse(&serde_json::to_vec(&valid).unwrap(), now, true).is_ok());
    for index in 0..6 {
        let mut bad = valid.clone();
        match index {
            0 => bad.expires_unix = now - 1,
            1 => bad.created_unix = now + 301,
            2 => bad.manifest.path = "generations/other/GENERATION.json".into(),
            3 => bad.signature.path = "../signature".into(),
            4 => bad.sequence = 0,
            5 => bad.manifest.size = MAX_GENERATION_METADATA + 1,
            _ => unreachable!(),
        }
        assert!(PackFeed::parse(&serde_json::to_vec(&bad).unwrap(), now, true).is_err());
    }
    assert!(authenticated_feed(&serde_json::to_vec(&valid).unwrap(), b"", true).is_err());
    assert!(authenticate(b"signed body changed", b"not CMS", now).is_err());
}
#[test]
fn authenticated_installed_baseline_rejects_replay_without_highest_pointer() {
    // Pure policy inputs represent already authenticated signed feed/manifest
    // values. The real-CMS integration separately tests their provenance.
    let current = feed(&manifest());
    let bytes = serde_json::to_vec(&current).unwrap();
    let identity = model::hash(&bytes);
    let installed_manifest = current.manifest.sha256.clone();
    assert!(check_installed_advance(
        &current,
        &bytes,
        current.sequence,
        &installed_manifest,
        &identity
    )
    .is_ok());
    let mut older = current.clone();
    older.sequence -= 1;
    assert!(check_installed_advance(
        &older,
        &serde_json::to_vec(&older).unwrap(),
        current.sequence,
        &installed_manifest,
        &identity
    )
    .is_err());
    let mut replaced = current.clone();
    replaced.expires_unix += 1;
    assert!(check_installed_advance(
        &replaced,
        &serde_json::to_vec(&replaced).unwrap(),
        current.sequence,
        &installed_manifest,
        &identity
    )
    .is_err());
    let mut different_manifest = current.clone();
    different_manifest.manifest.sha256 = "A".repeat(64);
    assert!(check_installed_advance(
        &different_manifest,
        &serde_json::to_vec(&different_manifest).unwrap(),
        current.sequence,
        &installed_manifest,
        &identity
    )
    .is_err());
    let mut newer = current.clone();
    newer.sequence = u64::MAX;
    assert!(check_installed_advance(
        &newer,
        &serde_json::to_vec(&newer).unwrap(),
        current.sequence,
        &installed_manifest,
        &identity
    )
    .is_ok());
}
#[test]
fn healthy_continuation_requires_an_explicit_authenticated_selector() {
    let root = tempfile::tempdir().unwrap();
    assert!(verified_healthy_selection(root.path()).unwrap().is_none());
    save_record(root.path(), &pending()).unwrap();
    assert!(verified_healthy_selection(root.path()).unwrap().is_none());
    let mut record = pending();
    record.health = Health::Healthy;
    save_record(root.path(), &record).unwrap();
    assert!(verified_healthy_selection(root.path()).is_err());
    record.active = legacy();
    record.previous = None;
    save_record(root.path(), &record).unwrap();
    assert!(verified_healthy_selection(root.path()).is_err());
}
#[test]
fn selector_contains_rollback_and_pending_in_one_atomic_record_without_live_writes() {
    let root = tempfile::tempdir().unwrap();
    let game = root.path().join("game/personal.log");
    safe::write_new(&game, b"personal witness").unwrap();
    assert!(active_record(root.path()).unwrap().is_none());
    let record = pending();
    save_record(root.path(), &record).unwrap();
    let selected = active_record(root.path()).unwrap().unwrap();
    assert_eq!(selected.health, Health::Pending);
    assert_eq!(selected.previous, Some(legacy()));
    assert!(verify_generation_for_launch(root.path(), &"C".repeat(64)).is_err());
    assert_eq!(
        active_record(root.path()).unwrap().unwrap().active,
        record.active
    );
    assert_eq!(fs::read(&game).unwrap(), b"personal witness");
    let mut bad = pending();
    bad.previous = None;
    save_record(root.path(), &bad).unwrap();
    assert!(active_record(root.path()).is_err());
    let mut bad = pending();
    bad.attempt = "../escape".into();
    save_record(root.path(), &bad).unwrap();
    assert!(active_record(root.path()).is_err());
}
#[test]
fn first_launch_obligation_is_durable_attempt_bound_and_never_implicitly_healthy() {
    let root = tempfile::tempdir().unwrap();
    let record = pending();
    save_record(root.path(), &record).unwrap();
    let original = fs::read(root.path().join(SELECTOR)).unwrap();
    assert!(begin_launch(root.path(), &"E".repeat(64)).is_err());
    assert!(accept(root.path(), &record.attempt).is_err());
    assert!(rollback(root.path(), &"E".repeat(64)).is_err());
    assert_eq!(fs::read(root.path().join(SELECTOR)).unwrap(), original);
    // Missing pending bytes request recovery, but an unauthenticated/missing
    // predecessor can never be selected. The signed fixture covers a valid
    // check-only release, which stays pending instead of being rolled back.
    assert!(recover(root.path()).is_err());
    begin_launch(root.path(), &record.attempt).unwrap();
    let started = active_record(root.path()).unwrap().unwrap();
    assert!(started.launch_started && started.health == Health::Pending);
    // Neither a failed health verification nor an unavailable prior signed
    // generation may claim success or erase the recovery obligation.
    assert!(accept(root.path(), &record.attempt).is_err());
    assert!(recover(root.path()).is_err());
    let retained = active_record(root.path()).unwrap().unwrap();
    assert!(retained.launch_started && retained.health == Health::Pending);
    assert_eq!(retained.previous, record.previous);
    assert!(!root.path().join(HIGHEST).exists());
}
#[test]
fn immutable_metadata_retains_existing_objects_and_never_overwrites_collisions() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("generations/control.json");
    immutable_bytes(&path, b"signed bytes").unwrap();
    immutable_bytes(&path, b"signed bytes").unwrap();
    assert!(immutable_bytes(&path, b"other bytes").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"signed bytes");
    let source = root.path().join("new.part");
    safe::write_new(&source, b"new").unwrap();
    assert!(crate::platform::promote_new(&source, &path).is_err());
    assert_eq!(fs::read(source).unwrap(), b"new");
    assert_eq!(fs::read(path).unwrap(), b"signed bytes");
}

struct MemorySource {
    pair: Option<(Vec<u8>, Vec<u8>)>,
    objects: BTreeMap<String, Vec<u8>>,
    active: AtomicUsize,
    finished: AtomicUsize,
    fail: Option<String>,
}
impl MemorySource {
    fn empty() -> Self {
        Self {
            pair: None,
            objects: BTreeMap::new(),
            active: AtomicUsize::new(0),
            finished: AtomicUsize::new(0),
            fail: None,
        }
    }
}
struct Flight<'a> {
    active: &'a AtomicUsize,
    finished: &'a AtomicUsize,
}
impl Drop for Flight<'_> {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
        self.finished.fetch_add(1, Ordering::SeqCst);
    }
}
impl Source for MemorySource {
    fn get(
        &self,
        path: &str,
        limit: u64,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Vec<u8>> {
        let bytes = self
            .objects
            .get(path)
            .context("unexpected object GET")?
            .clone();
        ensure!(bytes.len() as u64 <= limit, "fake source bound");
        progress(bytes.len() as u64)?;
        Ok(bytes)
    }
    fn download(
        &self,
        path: &str,
        _: &FileEntry,
        destination: &Path,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()> {
        self.active.fetch_add(1, Ordering::SeqCst);
        let _flight = Flight {
            active: &self.active,
            finished: &self.finished,
        };
        std::thread::sleep(Duration::from_millis(2));
        ensure!(
            self.fail.as_deref() != Some(path),
            "injected object failure"
        );
        let bytes = self
            .objects
            .get(path)
            .context("unexpected object download")?;
        progress(bytes.len() as u64)?;
        safe::write_new(destination, bytes)
    }
}
impl PackSource for MemorySource {
    fn packed_pair(
        &self,
        _: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        Ok(self.pair.clone())
    }
}
struct TestStatus;
impl Status for TestStatus {
    fn set(&self, _: &str, _: u32) {}
    fn cancelled(&self) -> bool {
        false
    }
}

#[test]
fn only_absent_new_channel_before_migration_can_return_legacy_fallback() {
    let root = tempfile::tempdir().unwrap();
    let source = MemorySource::empty();
    assert!(check_update(root.path(), &source, &TestStatus)
        .unwrap()
        .is_none());
    let mut record = pending();
    record.health = Health::Healthy;
    save_record(root.path(), &record).unwrap();
    assert!(check_update(root.path(), &source, &TestStatus).is_err());
    assert_eq!(
        active_record(root.path()).unwrap().unwrap().active,
        record.active
    );
}
#[test]
fn invalid_cms_is_not_legacy_absence_or_a_selector_change() {
    let root = tempfile::tempdir().unwrap();
    let mut source = MemorySource::empty();
    source.pair = Some((serde_json::to_vec(&feed(&manifest())).unwrap(), vec![]));
    assert!(check_update(root.path(), &source, &TestStatus).is_err());
    assert!(active_record(root.path()).unwrap().is_none());
    assert!(!root.path().join(HIGHEST).exists());
}
#[test]
fn whole_object_hash_and_immutable_publication_precede_selection() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("objects/packs/test.m2p");
    let parts = root.path().join(".update/pack-downloads");
    let object = entry("object", b"correct");
    let mut source = MemorySource::empty();
    source.objects.insert("object".into(), b"corrupt".to_vec());
    let cancel = AtomicBool::new(false);
    let wire = AtomicU64::new(0);
    assert!(ensure_object(&source, "object", &object, &target, &parts, &cancel, &wire).is_err());
    assert!(!target.exists() && !root.path().join(SELECTOR).exists());
    let part = parts.join(format!("{}.part", object.sha256));
    fs::remove_file(part).unwrap();
    source.objects.insert("object".into(), b"correct".to_vec());
    assert!(ensure_object(&source, "object", &object, &target, &parts, &cancel, &wire).unwrap());
    assert!(!ensure_object(&source, "object", &object, &target, &parts, &cancel, &wire).unwrap());
    assert_eq!(fs::read(target).unwrap(), b"correct");
}
#[test]
fn immutable_corruption_and_linked_partial_fail_without_overwriting_personal_files() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("objects/packs/object.m2p");
    let parts = root.path().join(".update/pack-downloads");
    let object = entry("object", b"correct");
    let cancel = AtomicBool::new(false);
    let wire = AtomicU64::new(0);
    let mut source = MemorySource::empty();
    source.objects.insert("object".into(), b"correct".to_vec());
    safe::write_new(&target, b"corrupt").unwrap();
    assert!(ensure_object(&source, "object", &object, &target, &parts, &cancel, &wire).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"corrupt");
    assert_eq!(source.finished.load(Ordering::SeqCst), 0);
    fs::remove_file(&target).unwrap();
    let personal = root.path().join("userdata/player.log");
    safe::write_new(&personal, b"personal witness").unwrap();
    fs::create_dir_all(&parts).unwrap();
    fs::hard_link(&personal, parts.join(format!("{}.part", object.sha256))).unwrap();
    assert!(ensure_object(&source, "object", &object, &target, &parts, &cancel, &wire).is_err());
    assert_eq!(fs::read(personal).unwrap(), b"personal witness");
    assert!(!target.exists() && !root.path().join(SELECTOR).exists());
}
#[test]
fn failed_parallel_pack_download_joins_all_writers_and_keeps_active_and_personal() {
    let root = tempfile::tempdir().unwrap();
    let mut source = MemorySource::empty();
    let mut m = manifest();
    m.packs.clear();
    for index in 0..32 {
        let bytes = format!("pack {index}").into_bytes();
        let hash = model::hash(&bytes);
        let path = format!("objects/packs/{}.m2p", hash.to_ascii_lowercase());
        m.packs.push(entry(&path, &bytes));
        source.objects.insert(path, bytes);
    }
    source.fail = Some(m.packs[2].path.clone());
    safe::write_new(&root.path().join("game/logs/personal.log"), b"witness").unwrap();
    save_record(root.path(), &pending()).unwrap();
    let before = fs::read(root.path().join(SELECTOR)).unwrap();
    assert!(download_packs(&source, &m, root.path(), &TestStatus, &AtomicU64::new(0)).is_err());
    assert_eq!(source.active.load(Ordering::SeqCst), 0);
    let finished = source.finished.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(15));
    assert_eq!(source.finished.load(Ordering::SeqCst), finished);
    assert_eq!(fs::read(root.path().join(SELECTOR)).unwrap(), before);
    assert_eq!(
        fs::read(root.path().join("game/logs/personal.log")).unwrap(),
        b"witness"
    );
}
