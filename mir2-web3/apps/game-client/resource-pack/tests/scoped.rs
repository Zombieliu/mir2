use mir2_resource_pack::{
    build,
    scoped::{RequiredResourceScope, MAX_SCOPE_BYTES, REQUIRED_SCOPE_SCHEMA},
    sha256, BuildOptions, BuildOutput, PackObject, PackStore, ResourceInput, ScopedPackStore,
    ScopedRead, MAX_RESOURCES,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
};

struct Fixture {
    temp: tempfile::TempDir,
    objects: PathBuf,
    build: BuildOutput,
}
impl Fixture {
    fn with(files: &[(&str, &[u8])], target: u64) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("loose");
        fs::create_dir(&source).unwrap();
        let objects = temp.path().join("original-objects");
        let inputs = files
            .iter()
            .map(|(name, bytes)| {
                let path = source.join(name);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, bytes).unwrap();
                ResourceInput {
                    path: (*name).into(),
                    size: bytes.len() as u64,
                    sha256: sha256(bytes),
                }
            })
            .collect::<Vec<_>>();
        let build = build(
            &source,
            &objects,
            &inputs,
            None,
            BuildOptions {
                target_pack_bytes: target,
            },
        )
        .unwrap();
        Self {
            temp,
            objects,
            build,
        }
    }
    fn new() -> Self {
        Self::with(
            &[
                ("core/a.png", b"required-a"),
                ("core/alias.png", b"required-a"),
                ("world/b.png", b"deferred-b"),
                ("world/c.png", b"deferred-c"),
            ],
            96,
        )
    }
    fn object(&self, path: &str) -> PackObject {
        let sha = &self.build.catalog.resources[path].pack_sha256;
        self.build
            .catalog
            .packs
            .iter()
            .find(|p| &p.sha256 == sha)
            .unwrap()
            .clone()
    }
    fn copy(&self, path: &str, cache: &Path) {
        let object = self.object(path);
        fs::copy(
            self.objects.join(object.file_name()),
            cache.join(object.file_name()),
        )
        .unwrap();
    }
    fn cache(&self, name: &str, paths: &[&str]) -> PathBuf {
        let cache = self.temp.path().join(name);
        fs::create_dir(&cache).unwrap();
        let mut copied = BTreeSet::new();
        for path in paths {
            if copied.insert(self.object(path).sha256) {
                self.copy(path, &cache);
            }
        }
        cache
    }
    fn scope(&self) -> Value {
        json!({"schema":REQUIRED_SCOPE_SCHEMA,"catalogSha256":self.build.catalog_sha256,
            "id":"login-and-first-map","requiredPaths":["core/a.png"]})
    }
    fn open(&self, cache: &Path) -> ScopedPackStore {
        open(cache, &self.build.catalog_bytes, &self.scope()).unwrap()
    }
}
fn encoded(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}
fn open(cache: &Path, catalog: &[u8], scope: &Value) -> anyhow::Result<ScopedPackStore> {
    let bytes = encoded(scope);
    ScopedPackStore::open(cache, catalog, &sha256(catalog), &bytes, &sha256(&bytes))
}

#[test]
fn shared_scope_api_selects_required_descriptors_before_object_files_exist() {
    let f = Fixture::new();
    let scope_bytes = encoded(&f.scope());
    let catalog =
        mir2_resource_pack::Catalog::parse(&f.build.catalog_bytes, &f.build.catalog_sha256)
            .unwrap();
    let scope = RequiredResourceScope::parse(
        &scope_bytes,
        &sha256(&scope_bytes),
        &f.build.catalog_sha256,
        &catalog,
    )
    .unwrap();
    assert_eq!(
        scope.required_objects(&catalog).unwrap(),
        [f.object("core/a.png")]
    );
    assert!(!f.temp.path().join("no-cache-needed").exists());
    assert!(RequiredResourceScope::parse(
        &scope_bytes,
        &"0".repeat(64),
        &f.build.catalog_sha256,
        &catalog
    )
    .is_err());
    assert!(RequiredResourceScope::parse(
        &scope_bytes,
        &sha256(&scope_bytes),
        &"0".repeat(64),
        &catalog
    )
    .is_err());
    let mut changed = scope.clone();
    changed.required_paths.push("missing.png".into());
    assert!(changed.required_objects(&catalog).is_err());
    let mut changed = scope;
    changed.required_paths.push("core/a.png".into());
    assert!(changed.required_objects(&catalog).is_err());
}

#[test]
fn pending_means_declared_not_missing_and_disk_presence_never_implicitly_admits() {
    let f = Fixture::new();
    let cache = f.cache("all-on-disk", &["core/a.png", "world/b.png", "world/c.png"]);
    let b = f.object("world/b.png");
    // A malformed deferred file is deliberately present before open. Neither
    // opening a required scope nor reading a Pending path examines its bytes.
    fs::write(cache.join(b.file_name()), b"not a pack").unwrap();
    let store = f.open(&cache);
    assert!(store.exists("world/b.png") && !store.exists("missing.png"));
    assert_eq!(
        store.read("core/a.png").unwrap(),
        ScopedRead::Ready(b"required-a".to_vec())
    );
    assert_eq!(
        store.read("core/alias.png").unwrap(),
        ScopedRead::Ready(b"required-a".to_vec())
    );
    assert_eq!(
        store.read("world/b.png").unwrap(),
        ScopedRead::Pending(b.clone())
    );
    assert!(store.read("missing.png").is_err());
    assert!(store.read("../core/a.png").is_err());
    assert_eq!(store.required_objects(), [f.object("core/a.png")]);
    assert_eq!(store.pending_objects().unwrap().len(), 2);
    assert!(!store.is_fully_available().unwrap());
    assert!(store.admit_cached_pack(&b.sha256).is_err());
    assert_eq!(
        store.read("world/b.png").unwrap(),
        ScopedRead::Pending(b.clone())
    );
    f.copy("world/b.png", &cache);
    assert_eq!(
        store.read("world/b.png").unwrap(),
        ScopedRead::Pending(b.clone())
    );
    assert!(store
        .admit_cached_pack(&b.sha256.to_ascii_lowercase())
        .unwrap());
    assert!(!store.admit_cached_pack(&b.sha256).unwrap());
    assert_eq!(
        store.read("world/b.png").unwrap(),
        ScopedRead::Ready(b"deferred-b".to_vec())
    );
    assert_eq!(store.pending_objects().unwrap(), [f.object("world/c.png")]);
    assert!(store
        .admit_cached_pack(&f.object("world/c.png").sha256)
        .unwrap());
    assert!(store.is_fully_available().unwrap());
    assert!(store.pending_objects().unwrap().is_empty());
    assert!(store.admit_cached_pack(&"0".repeat(64)).is_err());
    assert!(store.admit_cached_pack("../not-a-digest").is_err());
    assert_eq!(store.catalog_sha256(), f.build.catalog_sha256);
    assert_eq!(store.scope().id, "login-and-first-map");
    assert_eq!(store.scope_sha256(), sha256(&encoded(&f.scope())));
}

#[test]
fn scope_and_catalog_have_independent_pins_and_strict_bounded_schema() {
    let f = Fixture::new();
    let cache = f.cache("cache", &["core/a.png"]);
    let base = f.scope();
    let sb = encoded(&base);
    assert!(ScopedPackStore::open(
        &cache,
        &f.build.catalog_bytes,
        &"0".repeat(64),
        &sb,
        &sha256(&sb)
    )
    .is_err());
    assert!(ScopedPackStore::open(
        &cache,
        &f.build.catalog_bytes,
        &f.build.catalog_sha256,
        &sb,
        &"0".repeat(64)
    )
    .is_err());
    let mut changed = sb.clone();
    changed.push(b' ');
    assert!(ScopedPackStore::open(
        &cache,
        &f.build.catalog_bytes,
        &f.build.catalog_sha256,
        &changed,
        &sha256(&sb)
    )
    .is_err());
    let mut cases = Vec::new();
    let mut v = base.clone();
    v["schema"] = json!("wrong");
    cases.push(v);
    let mut v = base.clone();
    v["catalogSha256"] = json!("0".repeat(64));
    cases.push(v);
    let mut v = base.clone();
    v["catalogSha256"] = json!("unsafe");
    cases.push(v);
    let mut v = base.clone();
    v["unknown"] = json!(true);
    cases.push(v);
    for id in [
        "".to_owned(),
        "a".repeat(65),
        "中文".into(),
        "line\nbreak".into(),
        "space id".into(),
    ] {
        let mut v = base.clone();
        v["id"] = json!(id);
        cases.push(v);
    }
    for paths in [
        json!([]),
        json!(["core/a.png", "core/a.png"]),
        json!(["core/alias.png", "core/a.png"]),
        json!(["core/A.png", "core/a.png"]),
        json!(["../unsafe"]),
        json!(["core/a.png/child"]),
        json!(["missing.png"]),
    ] {
        let mut v = base.clone();
        v["requiredPaths"] = paths;
        cases.push(v);
    }
    for (i, v) in cases.iter().enumerate() {
        assert!(open(&cache, &f.build.catalog_bytes, v).is_err(), "case {i}");
    }
    let duplicate=format!("{{\"schema\":\"{REQUIRED_SCOPE_SCHEMA}\",\"schema\":\"{REQUIRED_SCOPE_SCHEMA}\",\"catalogSha256\":\"{}\",\"id\":\"id\",\"requiredPaths\":[\"core/a.png\"]}}",f.build.catalog_sha256);
    assert!(ScopedPackStore::open(
        &cache,
        &f.build.catalog_bytes,
        &f.build.catalog_sha256,
        duplicate.as_bytes(),
        &sha256(duplicate.as_bytes())
    )
    .is_err());
    let too_big = vec![b' '; MAX_SCOPE_BYTES + 1];
    assert!(ScopedPackStore::open(
        &cache,
        &f.build.catalog_bytes,
        &f.build.catalog_sha256,
        &too_big,
        &sha256(&too_big)
    )
    .is_err());
    let mut v = base.clone();
    v["requiredPaths"] = json!(vec!["p"; MAX_RESOURCES + 1]);
    assert!(encoded(&v).len() < MAX_SCOPE_BYTES);
    assert!(open(&cache, &f.build.catalog_bytes, &v).is_err());
    let mut valid = base;
    valid["id"] = json!("x".repeat(64));
    assert_eq!(
        open(&cache, &f.build.catalog_bytes, &valid)
            .unwrap()
            .scope()
            .id
            .len(),
        64
    );
}

#[test]
fn required_missing_bad_hash_and_hardlinks_fail_but_default_full_never_skips_missing() {
    let f = Fixture::new();
    let empty = f.cache("empty", &[]);
    assert!(open(&empty, &f.build.catalog_bytes, &f.scope()).is_err());
    let cache = f.cache("required-only", &["core/a.png"]);
    let store = f.open(&cache);
    assert!(!store.is_fully_available().unwrap());
    assert!(PackStore::open(&cache, &f.build.catalog_bytes, &f.build.catalog_sha256).is_err());
    drop(store);
    let p = cache.join(f.object("core/a.png").file_name());
    let original = fs::read(&p).unwrap();
    let mut bad = original.clone();
    *bad.last_mut().unwrap() ^= 1;
    fs::write(&p, &bad).unwrap();
    assert!(open(&cache, &f.build.catalog_bytes, &f.scope()).is_err());
    fs::write(&p, &original).unwrap();
    fs::hard_link(&p, cache.join("hardlink.m2p")).unwrap();
    assert!(open(&cache, &f.build.catalog_bytes, &f.scope()).is_err());
    fs::remove_file(cache.join("hardlink.m2p")).unwrap();
    assert!(open(&cache, &f.build.catalog_bytes, &f.scope()).is_ok());
}

#[test]
fn optional_absence_or_hardlink_failure_keeps_pending_and_does_not_poison_required_reads() {
    let f = Fixture::new();
    let cache = f.cache("cache", &["core/a.png"]);
    let store = f.open(&cache);
    let b = f.object("world/b.png");
    assert!(store.admit_cached_pack(&b.sha256).is_err());
    f.copy("world/b.png", &cache);
    let link = cache.join("linked-optional.m2p");
    fs::hard_link(cache.join(b.file_name()), &link).unwrap();
    assert!(store.admit_cached_pack(&b.sha256).is_err());
    assert_eq!(
        store.read("world/b.png").unwrap(),
        ScopedRead::Pending(b.clone())
    );
    assert_eq!(
        store.read("core/a.png").unwrap(),
        ScopedRead::Ready(b"required-a".to_vec())
    );
    fs::remove_file(link).unwrap();
    assert!(store.admit_cached_pack(&b.sha256).unwrap());
}

#[test]
fn every_catalog_extent_in_a_required_pack_is_checked_not_only_required_paths() {
    let f = Fixture::with(&[("a.png", b"required"), ("b.png", b"deferred")], 1024);
    let cache = f.cache("one-pack", &["a.png"]);
    let mut catalog = serde_json::to_value(&f.build.catalog).unwrap();
    let offset = catalog["resources"]["b.png"]["offset"].as_u64().unwrap();
    catalog["resources"]["b.png"]["offset"] = json!(offset - 1);
    let cb = encoded(&catalog);
    let scope = json!({"schema":REQUIRED_SCOPE_SCHEMA,"catalogSha256":sha256(&cb),"id":"required-a-only","requiredPaths":["a.png"]});
    assert!(mir2_resource_pack::Catalog::parse(&cb, &sha256(&cb)).is_ok());
    assert!(open(&cache, &cb, &scope).is_err());
}

#[test]
fn deferred_extent_mismatch_is_rejected_on_admission_without_publishing() {
    let f = Fixture::new();
    let cache = f.cache("cache", &["core/a.png", "world/b.png"]);
    let mut catalog = serde_json::to_value(&f.build.catalog).unwrap();
    let offset = catalog["resources"]["world/b.png"]["offset"]
        .as_u64()
        .unwrap();
    catalog["resources"]["world/b.png"]["offset"] = json!(offset - 1);
    let cb = encoded(&catalog);
    let mut scope = f.scope();
    scope["catalogSha256"] = json!(sha256(&cb));
    let store = open(&cache, &cb, &scope).unwrap();
    let b = f.object("world/b.png");
    assert_eq!(
        store.read("world/b.png").unwrap(),
        ScopedRead::Pending(b.clone())
    );
    assert!(store.admit_cached_pack(&b.sha256).is_err());
    assert_eq!(store.read("world/b.png").unwrap(), ScopedRead::Pending(b));
    assert_eq!(store.pending_objects().unwrap().len(), 2);
}

#[test]
fn admitted_reads_still_hash_resource_content_and_never_turn_corruption_into_pending() {
    let f = Fixture::new();
    let cache = f.cache("cache", &["core/a.png"]);
    let original = f.object("world/b.png");
    let mut data = fs::read(f.objects.join(original.file_name())).unwrap();
    data[f.build.catalog.resources["world/b.png"].offset as usize] ^= 1;
    // This adversarial fixture rebinds the whole-object digest but leaves a
    // lying per-resource digest/index. Admission follows full-store SHA/index
    // semantics; the unchanged per-read hash must still reject the payload.
    let rebound = sha256(&data);
    fs::write(
        cache.join(format!("{}.m2p", rebound.to_ascii_lowercase())),
        data,
    )
    .unwrap();
    let mut catalog = serde_json::to_value(&f.build.catalog).unwrap();
    for p in catalog["packs"].as_array_mut().unwrap() {
        if p["sha256"] == original.sha256 {
            p["sha256"] = json!(rebound);
        }
    }
    catalog["resources"]["world/b.png"]["packSha256"] = json!(rebound);
    let cb = encoded(&catalog);
    let mut scope = f.scope();
    scope["catalogSha256"] = json!(sha256(&cb));
    let store = open(&cache, &cb, &scope).unwrap();
    assert!(store.admit_cached_pack(&rebound).unwrap());
    assert!(store.read("world/b.png").is_err());
    assert!(!store.admit_cached_pack(&rebound).unwrap());
    assert_eq!(store.pending_objects().unwrap(), [f.object("world/c.png")]);
}

#[test]
fn concurrent_reads_and_duplicate_admissions_publish_exactly_once_per_object() {
    let f = Fixture::new();
    let cache = f.cache("cache", &["core/a.png", "world/b.png", "world/c.png"]);
    let store = Arc::new(f.open(&cache));
    let barrier = Arc::new(Barrier::new(9));
    let mut workers = Vec::new();
    for i in 0..8 {
        let s = Arc::clone(&store);
        let gate = Arc::clone(&barrier);
        let b = f.object("world/b.png");
        let c = f.object("world/c.png");
        workers.push(std::thread::spawn(move || {
            gate.wait();
            let mut admissions = 0;
            for n in 0..128 {
                assert_eq!(
                    s.read("core/a.png").unwrap(),
                    ScopedRead::Ready(b"required-a".to_vec())
                );
                for (path, data, object) in [
                    ("world/b.png", b"deferred-b".as_slice(), &b),
                    ("world/c.png", b"deferred-c".as_slice(), &c),
                ] {
                    match s.read(path).unwrap() {
                        ScopedRead::Ready(bytes) => assert_eq!(bytes, data),
                        ScopedRead::Pending(pack) => assert_eq!(&pack, object),
                    }
                }
                if i < 4 && n == 0 {
                    admissions += s
                        .admit_cached_pack(if i % 2 == 0 { &b.sha256 } else { &c.sha256 })
                        .unwrap() as usize;
                }
            }
            admissions
        }));
    }
    barrier.wait();
    assert_eq!(
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .sum::<usize>(),
        2
    );
    assert!(store.is_fully_available().unwrap());
    assert!(store.pending_objects().unwrap().is_empty());
}

#[cfg(windows)]
#[test]
fn windows_required_and_later_admitted_handles_keep_write_delete_and_directory_leases() {
    use std::fs::OpenOptions;
    let f = Fixture::new();
    let cache = f.cache("cache", &["core/a.png", "world/b.png"]);
    let store = Arc::new(f.open(&cache));
    let a = cache.join(f.object("core/a.png").file_name());
    let b = cache.join(f.object("world/b.png").file_name());
    assert!(OpenOptions::new().write(true).open(&a).is_err());
    assert!(fs::remove_file(&a).is_err());
    assert!(fs::rename(&cache, f.temp.path().join("moved-cache")).is_err());
    // Deferred disk files have no hidden lease or eager verification.
    assert!(OpenOptions::new().write(true).open(&b).is_ok());
    assert!(store
        .admit_cached_pack(&f.object("world/b.png").sha256)
        .unwrap());
    assert!(OpenOptions::new().write(true).open(&b).is_err());
    assert!(fs::remove_file(&b).is_err());
    let retained = Arc::clone(&store);
    drop(store);
    assert!(OpenOptions::new().write(true).open(&b).is_err());
    assert_eq!(
        retained.read("world/b.png").unwrap(),
        ScopedRead::Ready(b"deferred-b".to_vec())
    );
    drop(retained);
    assert!(OpenOptions::new().write(true).open(&a).is_ok());
    assert!(OpenOptions::new().write(true).open(&b).is_ok());
}

#[cfg(unix)]
#[test]
fn required_and_deferred_symlinks_are_rejected_by_existing_safe_io() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let cache = f.cache("cache", &["core/a.png"]);
    let a = f.object("core/a.png");
    fs::remove_file(cache.join(a.file_name())).unwrap();
    symlink(f.objects.join(a.file_name()), cache.join(a.file_name())).unwrap();
    assert!(open(&cache, &f.build.catalog_bytes, &f.scope()).is_err());
    fs::remove_file(cache.join(a.file_name())).unwrap();
    f.copy("core/a.png", &cache);
    let store = f.open(&cache);
    let b = f.object("world/b.png");
    symlink(f.objects.join(b.file_name()), cache.join(b.file_name())).unwrap();
    assert!(store.admit_cached_pack(&b.sha256).is_err());
    assert_eq!(store.read("world/b.png").unwrap(), ScopedRead::Pending(b));
}
