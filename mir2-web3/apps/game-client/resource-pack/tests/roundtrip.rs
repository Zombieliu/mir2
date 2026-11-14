use mir2_resource_pack::{
    build, sha256, BuildOptions, Catalog, PackStore, ResourceInput, MAX_PACKS,
};
use std::{fs, path::Path, sync::Arc};

fn input(root: &Path, path: &str, bytes: &[u8]) -> ResourceInput {
    let target = root.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, bytes).unwrap();
    ResourceInput {
        path: path.into(),
        size: bytes.len() as u64,
        sha256: sha256(bytes),
    }
}
fn encoded(value: &serde_json::Value) -> (Vec<u8>, String) {
    let bytes = serde_json::to_vec(value).unwrap();
    let sha = sha256(&bytes);
    (bytes, sha)
}

#[test]
fn exact_random_reads_aliases_and_parallel_positional_io() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let objects = temp.path().join("objects");
    let inputs = [
        input(&source, "assets/a.png", b"first exact bytes"),
        input(&source, "assets/sub/b.map.gz", b"second exact bytes"),
        input(&source, "fonts/empty.txt", b""),
    ];
    let result = build(&source, &objects, &inputs, None, BuildOptions::default()).unwrap();
    assert_eq!(result.new_packs.len(), 1);
    let store =
        Arc::new(PackStore::open(&objects, &result.catalog_bytes, &result.catalog_sha256).unwrap());
    assert_eq!(store.catalog_sha256(), result.catalog_sha256);
    assert!(
        store.is_directory("") && store.is_directory("assets") && store.is_directory("assets/")
    );
    assert!(!store.is_directory("assets/a.png"));
    assert_eq!(
        store.list_prefix("assets/").unwrap(),
        vec!["assets/a.png", "assets/sub/b.map.gz"]
    );
    assert!(store.list_prefix("../").is_err());
    let handles = (0..12)
        .map(|index| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                for _ in 0..32 {
                    let (path, bytes): (&str, &[u8]) = if index % 2 == 0 {
                        ("assets/a.png", b"first exact bytes")
                    } else {
                        ("assets/sub/b.map.gz", b"second exact bytes")
                    };
                    assert_eq!(store.read(path).unwrap(), bytes);
                    assert_eq!(store.read("fonts/empty.txt").unwrap(), b"");
                }
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().unwrap();
    }
    assert!(!store.exists("missing.png"));
    assert!(store.read("missing.png").is_err());
}

#[test]
fn directory_queries_distinguish_sibling_prefixes_and_exact_files() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let objects = temp.path().join("objects");
    let inputs = [
        input(&source, "foo/bar2/a.png", b"sibling directory content"),
        input(&source, "foo/bar.txt", b"a file, not a directory"),
        input(&source, "z-last/last.png", b"final sorted resource"),
    ];
    let result = build(&source, &objects, &inputs, None, BuildOptions::default()).unwrap();
    let store = PackStore::open(&objects, &result.catalog_bytes, &result.catalog_sha256).unwrap();
    assert!(store.is_directory("") && store.is_directory("foo"));
    assert!(store.is_directory("foo/bar2") && store.is_directory("foo/bar2/"));
    for path in [
        "foo/bar",
        "foo/b",
        "foo/bar.txt",
        "foo/bar2/a.png",
        "z-last/last.png",
        "zz-missing",
        "../foo",
    ] {
        assert!(!store.is_directory(path), "must not be a directory: {path}");
    }
    assert!(store.list_prefix("foo/bar").unwrap().is_empty());
    assert_eq!(store.list_prefix("foo/bar2").unwrap(), ["foo/bar2/a.png"]);
}

#[test]
fn next_generation_reuses_content_rename_alias_and_deletion_never_resurrects() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let objects = temp.path().join("objects");
    let inputs = [
        input(&source, "old/a.png", b"reusable content"),
        input(&source, "old/deleted.png", b"deleted content"),
    ];
    let first = build(&source, &objects, &inputs, None, BuildOptions::default()).unwrap();
    let previous = PackStore::open(&objects, &first.catalog_bytes, &first.catalog_sha256).unwrap();
    // Reuse requires no old/new loose file when the authoritative SHA is known.
    fs::remove_file(source.join("old/a.png")).unwrap();
    let next = [
        ResourceInput {
            path: "new/renamed.png".into(),
            size: inputs[0].size,
            sha256: inputs[0].sha256.clone(),
        },
        ResourceInput {
            path: "new/alias.png".into(),
            size: inputs[0].size,
            sha256: inputs[0].sha256.clone(),
        },
    ];
    let second = build(
        &source,
        &objects,
        &next,
        Some(&previous),
        BuildOptions::default(),
    )
    .unwrap();
    assert!(second.new_packs.is_empty());
    assert_eq!(second.reused_resources, 2);
    assert_eq!(first.catalog.packs, second.catalog.packs);
    let store = PackStore::open(&objects, &second.catalog_bytes, &second.catalog_sha256).unwrap();
    assert_eq!(store.read("new/renamed.png").unwrap(), b"reusable content");
    assert_eq!(store.read("new/alias.png").unwrap(), b"reusable content");
    assert!(
        !store.exists("old/a.png")
            && !store.exists("old/deleted.png")
            && !store.is_directory("old")
    );
    assert!(store.list_prefix("old").unwrap().is_empty());
    let mut third_inputs = next.to_vec();
    third_inputs.push(input(&source, "new/changed.png", b"only new content"));
    let third = build(
        &source,
        &objects,
        &third_inputs,
        Some(&store),
        BuildOptions::default(),
    )
    .unwrap();
    assert_eq!(third.new_packs.len(), 1);
    assert_eq!(third.reused_resources, 2);
    assert_eq!(third.catalog.packs.len(), 2);
    assert_eq!(
        PackStore::open(&objects, &third.catalog_bytes, &third.catalog_sha256)
            .unwrap()
            .read("new/changed.png")
            .unwrap(),
        b"only new content"
    );
}

#[test]
fn catalog_digest_duplicate_keys_unknown_fields_and_exact_pack_closure() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let objects = temp.path().join("objects");
    let result = build(
        &source,
        &objects,
        &[input(&source, "a.png", b"a")],
        None,
        BuildOptions::default(),
    )
    .unwrap();
    let catalog_path = temp.path().join("catalog.json");
    mir2_resource_pack::write_catalog_new(
        &catalog_path,
        &result.catalog_bytes,
        &result.catalog_sha256,
    )
    .unwrap();
    assert!(mir2_resource_pack::write_catalog_new(
        &catalog_path,
        &result.catalog_bytes,
        &result.catalog_sha256
    )
    .is_err());
    assert_eq!(fs::read(catalog_path).unwrap(), result.catalog_bytes);
    assert!(Catalog::parse(&result.catalog_bytes, &"0".repeat(64)).is_err());
    let original: serde_json::Value = serde_json::from_slice(&result.catalog_bytes).unwrap();
    for mutation in [
        "unknown",
        "missingPack",
        "extraPack",
        "caseAlias",
        "offset",
        "codec",
        "fileDirectory",
    ] {
        let mut value = original.clone();
        match mutation {
            "unknown" => value["extra"] = true.into(),
            "missingPack" => value["packs"] = serde_json::json!([]),
            "extraPack" => value["packs"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"sha256":"1".repeat(64),"size":100})),
            "caseAlias" => {
                value["resources"]["A.png"] = value["resources"]["a.png"].clone();
            }
            "offset" => value["resources"]["a.png"]["offset"] = u64::MAX.into(),
            "codec" => value["resources"]["a.png"]["codec"] = "gzip".into(),
            _ => {
                value["resources"]["a.png/x.png"] = value["resources"]["a.png"].clone();
            }
        }
        let (bytes, hash) = encoded(&value);
        assert!(Catalog::parse(&bytes, &hash).is_err(), "{mutation}");
    }
    let entry = serde_json::to_string(&original["resources"]["a.png"]).unwrap();
    let duplicate = format!("{{\"schema\":\"mir2.native.resource-catalog.v1\",\"resources\":{{\"a.png\":{entry},\"a.png\":{entry}}},\"packs\":{}}}", serde_json::to_string(&original["packs"]).unwrap());
    assert!(Catalog::parse(duplicate.as_bytes(), &sha256(duplicate.as_bytes())).is_err());
    let mut value = original;
    value["packs"] = serde_json::Value::Array(
        (0..=MAX_PACKS)
            .map(|index| serde_json::json!({"sha256":format!("{index:064X}"),"size":100}))
            .collect(),
    );
    let (bytes, hash) = encoded(&value);
    assert!(Catalog::parse(&bytes, &hash).is_err());
}

#[test]
fn malformed_pack_index_is_rejected_even_with_rebound_fixture_object_digest() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let objects = temp.path().join("objects");
    let result = build(
        &source,
        &objects,
        &[input(&source, "a.png", b"a"), input(&source, "b.png", b"b")],
        None,
        BuildOptions::default(),
    )
    .unwrap();
    let bytes = fs::read(objects.join(result.catalog.packs[0].file_name())).unwrap();
    for at in [0usize, 8, 12, 16, 24, 64, 80, 88, 96] {
        let mut corrupt = bytes.clone();
        corrupt[at] ^= 1;
        let hash = sha256(&corrupt);
        fs::write(
            objects.join(format!("{}.m2p", hash.to_ascii_lowercase())),
            &corrupt,
        )
        .unwrap();
        let mut catalog = result.catalog.clone();
        catalog.packs[0].sha256 = hash.clone();
        for entry in catalog.resources.values_mut() {
            entry.pack_sha256 = hash.clone();
        }
        let json = serde_json::to_vec(&catalog).unwrap();
        assert!(
            PackStore::open(&objects, &json, &sha256(&json)).is_err(),
            "corrupt offset {at}"
        );
    }
    let mut value: serde_json::Value = serde_json::from_slice(&result.catalog_bytes).unwrap();
    value["resources"]["a.png"]["offset"] = (result.catalog.resources["a.png"].offset + 1).into();
    let (json, hash) = encoded(&value);
    assert!(PackStore::open(&objects, &json, &hash).is_err());
}

#[test]
fn invalid_paths_wrong_source_and_create_only_conflict_never_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let objects = temp.path().join("objects");
    let good = input(&source, "a.png", b"correct source");
    for path in [
        "../evil.png",
        "/absolute.png",
        "a/../b.png",
        "NUL.png",
        "a.png:stream",
        "code.dll",
        "x.ps1.txt",
        "a//b.png",
    ] {
        let mut bad = good.clone();
        bad.path = path.into();
        assert!(
            build(&source, &objects, &[bad], None, BuildOptions::default()).is_err(),
            "{path}"
        );
    }
    let mut wrong = good.clone();
    wrong.sha256 = "0".repeat(64);
    assert!(build(&source, &objects, &[wrong], None, BuildOptions::default()).is_err());
    let first = build(
        &source,
        &objects,
        &[good.clone()],
        None,
        BuildOptions::default(),
    )
    .unwrap();
    let second = build(
        &source,
        &objects,
        &[good.clone()],
        None,
        BuildOptions::default(),
    )
    .unwrap();
    assert_eq!(first.catalog_bytes, second.catalog_bytes);
    assert!(second.new_packs.is_empty());
    let target = objects.join(first.catalog.packs[0].file_name());
    fs::write(&target, b"retained conflicting immutable bytes").unwrap();
    assert!(build(&source, &objects, &[good], None, BuildOptions::default()).is_err());
    assert_eq!(
        fs::read(target).unwrap(),
        b"retained conflicting immutable bytes"
    );
}

#[test]
fn hardlinked_sources_and_objects_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let objects = temp.path().join("objects");
    let item = input(&source, "a.png", b"small resource");
    fs::hard_link(source.join("a.png"), source.join("alias.png")).unwrap();
    assert!(build(
        &source,
        &objects,
        &[item.clone()],
        None,
        BuildOptions::default()
    )
    .is_err());
    fs::remove_file(source.join("alias.png")).unwrap();
    let result = build(&source, &objects, &[item], None, BuildOptions::default()).unwrap();
    fs::hard_link(
        objects.join(result.catalog.packs[0].file_name()),
        objects.join("other-link.m2p"),
    )
    .unwrap();
    assert!(PackStore::open(&objects, &result.catalog_bytes, &result.catalog_sha256).is_err());
}

#[cfg(windows)]
#[test]
fn windows_actual_read_lease_denies_write_delete_and_directory_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let objects = temp.path().join("objects");
    let result = build(
        &source,
        &objects,
        &[input(&source, "a.png", b"lease")],
        None,
        BuildOptions::default(),
    )
    .unwrap();
    let target = objects.join(result.catalog.packs[0].file_name());
    let store = PackStore::open(&objects, &result.catalog_bytes, &result.catalog_sha256).unwrap();
    assert!(fs::OpenOptions::new().write(true).open(&target).is_err());
    assert!(fs::remove_file(&target).is_err());
    assert!(fs::rename(&objects, temp.path().join("moved")).is_err());
    assert_eq!(store.read("a.png").unwrap(), b"lease");
    drop(store);
    assert!(fs::OpenOptions::new().write(true).open(&target).is_ok());
}

#[cfg(unix)]
#[test]
fn symlink_source_or_object_directory_is_rejected() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let objects = temp.path().join("objects");
    let item = input(&source, "a.png", b"plain");
    symlink(source.join("a.png"), source.join("link.png")).unwrap();
    let mut linked = item.clone();
    linked.path = "link.png".into();
    assert!(build(&source, &objects, &[linked], None, BuildOptions::default()).is_err());
    let result = build(&source, &objects, &[item], None, BuildOptions::default()).unwrap();
    let alias = temp.path().join("linked-objects");
    symlink(&objects, &alias).unwrap();
    assert!(PackStore::open(alias, &result.catalog_bytes, &result.catalog_sha256).is_err());
}
