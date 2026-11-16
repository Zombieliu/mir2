//! Bounded real pack bytes and policy tests. Manifest/scope inputs to private
//! helpers stand for authenticated metadata; these fixtures do not bypass CMS.
use super::*;
use mir2_resource_pack::{BuildOptions, ResourceInput, ScopedRead};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

fn entry(path: &str, bytes: &[u8]) -> FileEntry {
    FileEntry {
        path: path.into(),
        size: bytes.len() as u64,
        sha256: model::hash(bytes),
    }
}
struct Fixture {
    temp: tempfile::TempDir,
    source_objects: PathBuf,
    catalog: Vec<u8>,
    scope: Vec<u8>,
    manifest: GenerationManifest,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        let mut inputs = Vec::new();
        for (path, bytes) in [
            ("boot.png", b"required".as_slice()),
            ("world.png", b"deferred".as_slice()),
        ] {
            fs::write(source.join(path), bytes).unwrap();
            inputs.push(ResourceInput {
                path: path.into(),
                size: bytes.len() as u64,
                sha256: model::hash(bytes),
            });
        }
        let source_objects = temp.path().join("source-objects");
        let build = mir2_resource_pack::build(
            &source,
            &source_objects,
            &inputs,
            None,
            BuildOptions {
                target_pack_bytes: 96,
            },
        )
        .unwrap();
        assert_eq!(build.catalog.packs.len(), 2);
        let scope = serde_json::to_vec(&serde_json::json!({
            "schema":"mir2.native.required-resource-scope.v1", "id":"login-test",
            "catalogSha256":build.catalog_sha256, "requiredPaths":["boot.png"]
        }))
        .unwrap();
        let manifest = GenerationManifest {
            schema: "mir2.windows.generation.v2".into(),
            sequence: 123,
            min_engine: 2,
            game_source_revision: "a".repeat(40),
            engine_source_revision: "b".repeat(40),
            protocol: "crystal-mir2-v1".into(),
            content_profile: "platinum_fiveclass".into(),
            exe: entry("mir2-platform-windows.exe", b"exe"),
            signed_config: entry("mir2-client.toml", b"config"),
            catalog: entry("catalog.json", &build.catalog_bytes),
            packs: build
                .catalog
                .packs
                .iter()
                .map(|p| FileEntry {
                    path: format!("objects/packs/{}", p.file_name()),
                    size: p.size,
                    sha256: p.sha256.clone(),
                })
                .collect(),
            required_scope: Some(entry("required-scope.json", &scope)),
        };
        Self {
            temp,
            source_objects,
            catalog: build.catalog_bytes,
            scope,
            manifest,
        }
    }
    fn root(&self) -> PathBuf {
        let root = self.temp.path().join("install");
        fs::create_dir_all(root.join("objects/packs")).unwrap();
        root
    }
    fn required(&self) -> Vec<FileEntry> {
        required_pack_entries(&self.manifest, &self.catalog, Some(&self.scope)).unwrap()
    }
    fn copy_required(&self, root: &Path) {
        for entry in self.required() {
            fs::copy(
                self.source_objects
                    .join(Path::new(&entry.path).file_name().unwrap()),
                root.join(&entry.path),
            )
            .unwrap();
        }
    }
    fn source(&self, digest: &str) -> Files {
        let mut objects = BTreeMap::new();
        let directory = directory(digest).unwrap();
        objects.insert(format!("{directory}/catalog.json"), self.catalog.clone());
        objects.insert(
            format!("{directory}/required-scope.json"),
            self.scope.clone(),
        );
        for entry in &self.manifest.packs {
            objects.insert(
                entry.path.clone(),
                fs::read(
                    self.source_objects
                        .join(Path::new(&entry.path).file_name().unwrap()),
                )
                .unwrap(),
            );
        }
        Files {
            objects,
            gets: Mutex::new(vec![]),
            downloads: Mutex::new(vec![]),
        }
    }
}
struct Files {
    objects: BTreeMap<String, Vec<u8>>,
    gets: Mutex<Vec<String>>,
    downloads: Mutex<Vec<String>>,
}
impl Source for Files {
    fn get(
        &self,
        path: &str,
        limit: u64,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Vec<u8>> {
        self.gets.lock().unwrap().push(path.into());
        let bytes = self.objects.get(path).context("unexpected metadata GET")?;
        ensure!(bytes.len() as u64 <= limit, "fixture bound");
        progress((bytes.len() / 2) as u64)?;
        progress(bytes.len() as u64)?;
        Ok(bytes.clone())
    }
    fn download(
        &self,
        path: &str,
        _: &FileEntry,
        target: &Path,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()> {
        self.downloads.lock().unwrap().push(path.into());
        let bytes = self
            .objects
            .get(path)
            .context("unexpected payload request")?;
        progress(bytes.len() as u64)?;
        safe::write_new(target, bytes)
    }
}
impl PackSource for Files {
    fn packed_pair(
        &self,
        _: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        anyhow::bail!("pending check must not discover a replacement")
    }
}
#[derive(Default)]
struct Observe {
    rows: RefCell<Vec<(u64, u64)>>,
    prompts: RefCell<Vec<u64>>,
    reject: bool,
    cancel_on_bytes: bool,
    cancelled: Cell<bool>,
}
impl Status for Observe {
    fn set(&self, _: &str, _: u32) {}
    fn cancelled(&self) -> bool {
        self.cancelled.get()
    }
    fn transfer(&self, received: u64, total: u64) {
        self.rows.borrow_mut().push((received, total));
        if self.cancel_on_bytes && received > 0 {
            self.cancelled.set(true);
        }
    }
    fn confirm_update(&self, _: &str, bytes: u64) -> Result<()> {
        self.prompts.borrow_mut().push(bytes);
        ensure!(!self.reject, "fixture player cancellation");
        Ok(())
    }
}

#[test]
fn progressive_v2_requires_scope_and_v1_rejects_even_explicit_null_without_changing_v1_bytes() {
    let f = Fixture::new();
    assert!(GenerationManifest::parse(&serde_json::to_vec(&f.manifest).unwrap()).is_ok());
    let mut v1 = f.manifest.clone();
    v1.schema = "mir2.windows.generation.v1".into();
    v1.required_scope = None;
    let original = serde_json::to_vec(&v1).unwrap();
    assert!(!String::from_utf8_lossy(&original).contains("requiredScope"));
    assert_eq!(
        serde_json::to_vec(&GenerationManifest::parse(&original).unwrap()).unwrap(),
        original
    );
    for schema in ["mir2.windows.generation.v1", "mir2.windows.generation.v2"] {
        let mut null = serde_json::to_value(&v1).unwrap();
        null["schema"] = schema.into();
        null["requiredScope"] = serde_json::Value::Null;
        assert!(GenerationManifest::parse(&serde_json::to_vec(&null).unwrap()).is_err());
    }
    assert!(GenerationManifest::parse(
        &serde_json::to_vec(&GenerationManifest {
            schema: "mir2.windows.generation.v2".into(),
            ..v1.clone()
        })
        .unwrap()
    )
    .is_err());
    v1.required_scope = f.manifest.required_scope.clone();
    assert!(
        GenerationManifest::parse(&serde_json::to_vec(&v1).unwrap()).is_err(),
        "v2 cannot be relabeled as v1"
    );
    for (path, size) in [
        ("../required-scope.json", 1),
        ("scope.json", 1),
        ("required-scope.json", 0),
        ("required-scope.json", MAX_REQUIRED_SCOPE + 1),
    ] {
        let mut bad = f.manifest.clone();
        let entry = bad.required_scope.as_mut().unwrap();
        entry.path = path.into();
        entry.size = size;
        assert!(GenerationManifest::parse(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}

#[test]
fn progressive_required_set_is_derived_from_both_authenticated_digests_and_full_pack_closure() {
    let f = Fixture::new();
    let required = f.required();
    assert_eq!(required.len(), 1);
    let mut tampered = f.scope.clone();
    tampered[0] ^= 1;
    assert!(required_pack_entries(&f.manifest, &f.catalog, Some(&tampered)).is_err());
    let mut bad = f.manifest.clone();
    bad.catalog.sha256 = "F".repeat(64);
    assert!(required_pack_entries(&bad, &f.catalog, Some(&f.scope)).is_err());
    let mut bad = f.manifest.clone();
    bad.packs.pop();
    assert!(required_pack_entries(&bad, &f.catalog, Some(&f.scope)).is_err());
    let mut scope: serde_json::Value = serde_json::from_slice(&f.scope).unwrap();
    for (field, value) in [
        ("catalogSha256", serde_json::json!("E".repeat(64))),
        ("requiredPaths", serde_json::json!(["absent.png"])),
    ] {
        let original = scope[field].clone();
        scope[field] = value;
        let bytes = serde_json::to_vec(&scope).unwrap();
        let mut bad = f.manifest.clone();
        bad.required_scope = Some(entry("required-scope.json", &bytes));
        assert!(required_pack_entries(&bad, &f.catalog, Some(&bytes)).is_err());
        scope[field] = original;
    }
    assert!(required_pack_entries(&f.manifest, &f.catalog, None).is_err());
}

#[test]
fn progressive_missing_required_fails_but_optional_is_typed_pending_without_file_fallback() {
    let f = Fixture::new();
    let root = f.root();
    assert!(open_packs(&root, &f.manifest, &f.catalog, Some(&f.scope)).is_err());
    f.copy_required(&root);
    let lease = open_packs(&root, &f.manifest, &f.catalog, Some(&f.scope)).unwrap();
    let PackLease::Scoped { _store: store } = lease else {
        panic!("v2 must be scoped")
    };
    assert!(
        matches!(store.read("boot.png").unwrap(),ScopedRead::Ready(bytes) if bytes==b"required")
    );
    let ScopedRead::Pending(optional) = store.read("world.png").unwrap() else {
        panic!("missing content must be Pending")
    };
    fs::write(
        root.join("objects/packs").join(optional.file_name()),
        b"wrong optional bytes",
    )
    .unwrap();
    assert!(
        matches!(store.read("world.png").unwrap(), ScopedRead::Pending(_)),
        "unadmitted disk bytes are not content"
    );
    assert!(store.read("not-in-catalog.png").is_err());
    assert!(!store.is_fully_available().unwrap());
}

#[test]
fn progressive_scope_downgrade_never_relaxes_v1_full_pack_verification() {
    let f = Fixture::new();
    let root = f.root();
    f.copy_required(&root);
    let mut v1 = f.manifest.clone();
    v1.schema = "mir2.windows.generation.v1".into();
    v1.required_scope = None;
    assert_eq!(
        required_pack_entries(&v1, &f.catalog, None).unwrap().len(),
        2
    );
    assert!(open_packs(&root, &v1, &f.catalog, None).is_err());
    assert!(open_packs(&root, &v1, &f.catalog, Some(&f.scope)).is_err());
}

#[test]
fn progressive_download_stage_requests_only_derived_required_objects_and_preserves_state() {
    let f = Fixture::new();
    let root = f.root();
    let source = f.source(&"A".repeat(64));
    safe::write_new(&root.join(SELECTOR), b"prior selector witness").unwrap();
    safe::write_new(&root.join("userdata/player.log"), b"personal witness").unwrap();
    let required = f.required();
    let count = download_packs(
        &source,
        &required,
        &root,
        &Observe::default(),
        &AtomicU64::new(0),
    )
    .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        *source.downloads.lock().unwrap(),
        vec![required[0].path.clone()]
    );
    assert_eq!(
        fs::read(root.join(SELECTOR)).unwrap(),
        b"prior selector witness"
    );
    assert_eq!(
        fs::read(root.join("userdata/player.log")).unwrap(),
        b"personal witness"
    );
    let PackLease::Scoped { _store: store } =
        open_packs(&root, &f.manifest, &f.catalog, Some(&f.scope)).unwrap()
    else {
        panic!()
    };
    assert!(matches!(
        store.read("world.png").unwrap(),
        ScopedRead::Pending(_)
    ));
}

#[test]
fn progressive_planning_fetches_bounded_metadata_before_consent_without_payload_or_selector_writes()
{
    let f = Fixture::new();
    let root = f.root();
    let digest = "C".repeat(64);
    // The real manifest may be signed with noncanonical whitespace: the feed's
    // authenticated digest, never a local reserialization, is the remote base.
    let generation = root.join(directory(&digest).unwrap());
    let source = f.source(&digest);
    let status = Observe {
        reject: true,
        ..Default::default()
    };
    let wire = AtomicU64::new(0);
    let plan = fetch_metadata(
        &root,
        &generation,
        &digest,
        &source,
        &f.manifest,
        &status,
        &wire,
    )
    .unwrap();
    assert_eq!(source.gets.lock().unwrap().len(), 2);
    assert!(source.downloads.lock().unwrap().is_empty());
    assert_eq!(
        wire.load(Ordering::SeqCst),
        (f.catalog.len() + f.scope.len()) as u64
    );
    assert_eq!(
        *status.rows.borrow().last().unwrap(),
        (wire.load(Ordering::SeqCst), wire.load(Ordering::SeqCst))
    );
    assert!(confirm_generation_update(
        &root,
        &generation,
        &f.manifest,
        &plan.packs,
        false,
        &status
    )
    .is_err());
    let expected = f.manifest.exe.size
        + f.manifest.signed_config.size
        + f.manifest.catalog.size
        + f.scope.len() as u64
        + plan.packs[0].size;
    assert_eq!(*status.prompts.borrow(), vec![expected]);
    assert!(!generation.exists() && !root.join(SELECTOR).exists());
    assert_eq!(fs::read_dir(root.join("objects/packs")).unwrap().count(), 0);
}

#[test]
fn progressive_cached_metadata_is_reused_only_at_exact_hash_without_a_second_get() {
    let f = Fixture::new();
    let root = f.root();
    let digest = "D".repeat(64);
    let generation = root.join(directory(&digest).unwrap());
    let source = f.source(&digest);
    safe::write_new(&generation.join("catalog.json"), &f.catalog).unwrap();
    safe::write_new(&generation.join("required-scope.json"), &f.scope).unwrap();
    let wire = AtomicU64::new(0);
    let plan = fetch_metadata(
        &root,
        &generation,
        &digest,
        &source,
        &f.manifest,
        &Observe::default(),
        &wire,
    )
    .unwrap();
    assert!(source.gets.lock().unwrap().is_empty());
    assert_eq!(wire.load(Ordering::SeqCst), 0);
    assert_eq!(
        plan.control_bytes(&f.manifest.catalog),
        Some(f.catalog.as_slice())
    );
    fs::write(generation.join("required-scope.json"), b"tampered").unwrap();
    assert!(fetch_metadata(
        &root,
        &generation,
        &digest,
        &source,
        &f.manifest,
        &Observe::default(),
        &wire
    )
    .is_err());
    assert!(source.gets.lock().unwrap().is_empty());
    assert_eq!(
        fs::read(generation.join("required-scope.json")).unwrap(),
        b"tampered"
    );
}

#[test]
fn progressive_metadata_tamper_and_mid_body_cancellation_leave_no_payload_or_selection() {
    for cancel in [false, true] {
        let f = Fixture::new();
        let root = f.root();
        let digest = "E".repeat(64);
        let generation = root.join(directory(&digest).unwrap());
        let mut source = f.source(&digest);
        if !cancel {
            source
                .objects
                .get_mut(&format!(
                    "{}/required-scope.json",
                    directory(&digest).unwrap()
                ))
                .unwrap()[0] ^= 1;
        }
        let status = Observe {
            cancel_on_bytes: cancel,
            ..Default::default()
        };
        assert!(fetch_metadata(
            &root,
            &generation,
            &digest,
            &source,
            &f.manifest,
            &status,
            &AtomicU64::new(0)
        )
        .is_err());
        assert!(source.downloads.lock().unwrap().is_empty());
        assert!(!generation.exists() && !root.join(SELECTOR).exists());
        assert!(status.prompts.borrow().is_empty());
    }
}

#[test]
fn progressive_optional_absence_adds_no_update_bytes_or_prompt_to_selected_generation() {
    let f = Fixture::new();
    let root = f.root();
    let generation = root.join("generations/test");
    f.copy_required(&root);
    for (entry, bytes) in [
        (&f.manifest.exe, b"exe".as_slice()),
        (&f.manifest.signed_config, b"config".as_slice()),
        (&f.manifest.catalog, f.catalog.as_slice()),
        (
            f.manifest.required_scope.as_ref().unwrap(),
            f.scope.as_slice(),
        ),
    ] {
        safe::write_new(&generation.join(&entry.path), bytes).unwrap();
    }
    let status = Observe::default();
    let required = f.required();
    confirm_generation_update(&root, &generation, &f.manifest, &required, true, &status).unwrap();
    assert!(status.prompts.borrow().is_empty());
    confirm_generation_update(&root, &generation, &f.manifest, &required, false, &status).unwrap();
    assert_eq!(*status.prompts.borrow(), vec![0]);
}

#[test]
fn progressive_pending_selector_does_not_fetch_or_substitute_unsigned_scope_metadata() {
    let f = Fixture::new();
    let root = f.root();
    let digest = model::hash(&serde_json::to_vec(&f.manifest).unwrap());
    let source = f.source(&digest);
    let record = ActiveRecord {
        schema: "mir2.windows.active-generation.v1".into(),
        active: Selection::Generation {
            manifest_sha256: digest.clone(),
            sequence: 123,
        },
        previous: None,
        health: Health::FirstLaunchPending,
        attempt: "F".repeat(64),
        launch_started: false,
    };
    save_record(&root, &record).unwrap();
    let before = fs::read(root.join(SELECTOR)).unwrap();
    safe::write_new(
        &root
            .join(directory(&digest).unwrap())
            .join("GENERATION.json"),
        &serde_json::to_vec(&f.manifest).unwrap(),
    )
    .unwrap();
    assert!(
        check_update(&root, &source, &Observe::default()).is_err(),
        "unsigned pending generation must fail before scope or payload discovery"
    );
    assert_eq!(fs::read(root.join(SELECTOR)).unwrap(), before);
    assert!(source.gets.lock().unwrap().is_empty() && source.downloads.lock().unwrap().is_empty());
    assert!(!root.join(HIGHEST).exists());
}

#[cfg(windows)]
#[test]
fn progressive_required_pack_lease_excludes_mutation_through_activation_boundary() {
    let f = Fixture::new();
    let root = f.root();
    f.copy_required(&root);
    let required = root.join(&f.required()[0].path);
    let lease = open_packs(&root, &f.manifest, &f.catalog, Some(&f.scope)).unwrap();
    assert!(fs::write(&required, b"tamper").is_err());
    assert!(fs::remove_file(&required).is_err());
    drop(lease);
    assert!(fs::remove_file(required).is_ok());
}
