use mir2_resource_pack::{build, sha256, BuildOptions, PackStore, ResourceInput};
use std::{fs, sync::Arc};

fn fixture() -> (tempfile::TempDir, Vec<u8>, String) {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let mut inputs = Vec::new();
    for (path, bytes) in [
        ("a.png", b"same exact bytes".as_slice()),
        ("alias.png", b"same exact bytes".as_slice()),
    ] {
        fs::write(source.join(path), bytes).unwrap();
        inputs.push(ResourceInput {
            path: path.into(),
            size: bytes.len() as u64,
            sha256: sha256(bytes),
        });
    }
    let result = build(
        &source,
        &temp.path().join("objects"),
        &inputs,
        None,
        BuildOptions::default(),
    )
    .unwrap();
    (temp, result.catalog_bytes, result.catalog_sha256)
}

#[test]
fn ordinary_store_does_not_enable_diagnostics() {
    let (temp, bytes, digest) = fixture();
    let store = PackStore::open(temp.path().join("objects"), &bytes, &digest).unwrap();
    assert_eq!(store.read("a.png").unwrap(), b"same exact bytes");
    assert!(store.read_profile_snapshot().is_none());
}

#[test]
fn profiled_reads_preserve_aliases_and_exclude_failed_probes() {
    let (temp, bytes, digest) = fixture();
    let store = PackStore::open_profiled(temp.path().join("objects"), &bytes, &digest).unwrap();
    assert!(store.read_profile_snapshot().unwrap().resources.is_empty());
    for path in ["a.png", "alias.png", "a.png"] {
        assert_eq!(store.read(path).unwrap(), b"same exact bytes");
    }
    assert!(store.read("absent.png").is_err());
    assert!(store.read("../outside.png").is_err());
    let snapshot = store.read_profile_snapshot().unwrap();
    assert_eq!(snapshot.catalog_sha256, digest);
    assert_eq!(snapshot.successful_read_operations, 3);
    assert_eq!(snapshot.failed_read_operations, 2);
    assert_eq!(snapshot.resources.len(), 2);
    assert_eq!(snapshot.resources["a.png"], 2);
    assert_eq!(snapshot.resources["alias.png"], 1);
    assert!(!snapshot.counters_overflowed && !snapshot.poison_recovered);
    let encoded = serde_json::to_value(snapshot).unwrap();
    assert_eq!(encoded["schema"], "mir2.native.verified-resource-reads.v1");
    assert_eq!(encoded["catalogSha256"], digest);
}

#[test]
fn concurrent_reads_and_snapshots_do_not_lose_verified_dependencies() {
    let (temp, bytes, digest) = fixture();
    let store =
        Arc::new(PackStore::open_profiled(temp.path().join("objects"), &bytes, &digest).unwrap());
    let readers = (0..8)
        .map(|index| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                let path = if index % 2 == 0 { "a.png" } else { "alias.png" };
                for _ in 0..64 {
                    assert_eq!(store.read(path).unwrap(), b"same exact bytes");
                    let snapshot = store.read_profile_snapshot().unwrap();
                    assert_eq!(
                        snapshot.resources.values().sum::<u64>(),
                        snapshot.successful_read_operations
                    );
                }
            })
        })
        .collect::<Vec<_>>();
    for reader in readers {
        reader.join().unwrap();
    }
    let snapshot = store.read_profile_snapshot().unwrap();
    assert_eq!(snapshot.successful_read_operations, 512);
    assert_eq!(snapshot.failed_read_operations, 0);
    assert_eq!(snapshot.resources["a.png"], 256);
    assert_eq!(snapshot.resources["alias.png"], 256);
}

#[test]
fn diagnostics_do_not_admit_a_missing_pack_or_wrong_catalog_hash() {
    let (temp, bytes, digest) = fixture();
    assert!(
        PackStore::open_profiled(temp.path().join("objects"), &bytes, &"0".repeat(64)).is_err()
    );
    let file = fs::read_dir(temp.path().join("objects"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::remove_file(file).unwrap();
    assert!(PackStore::open_profiled(temp.path().join("objects"), &bytes, &digest).is_err());
}
