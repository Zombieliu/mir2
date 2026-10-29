//! History-selection tests use already-authenticated receipts. Windows CMS/key
//! verification is independently covered by the platform integration fixtures.
use super::{select_previous, LocalReceipt, ReceiptSource};
use crate::model::{hash, Component, Feed, FileEntry, META};

fn receipt(source: ReceiptSource, sequence: u64, version: &[u8]) -> LocalReceipt {
    let feed = Feed {
        schema: "mir2.windows.update-feed.v1".into(),
        channel: "invited".into(),
        platform: "windows-x64".into(),
        sequence,
        created_unix: 1_800_000_000,
        expires_unix: 1_802_592_000,
        min_bootstrap: 1,
        protocol: "crystal-mir2-v1".into(),
        content: "mir2.windows.package-manifest.v4".into(),
        game: Component {
            directory: format!("releases/game-{sequence}"),
            identity: format!("candidate-{sequence}"),
            metadata: META
                .into_iter()
                .map(|path| FileEntry {
                    path: path.into(),
                    size: if path == "VERSION.json" {
                        version.len() as u64
                    } else {
                        1
                    },
                    sha256: if path == "VERSION.json" {
                        hash(version)
                    } else {
                        hash(b"x")
                    },
                })
                .collect(),
        },
        engine: Component {
            directory: "releases/updater-1".into(),
            identity: "1".into(),
            metadata: ["ENGINE.json", "ENGINE.p7s"]
                .into_iter()
                .map(|path| FileEntry {
                    path: path.into(),
                    size: 1,
                    sha256: hash(b"x"),
                })
                .collect(),
        },
    };
    let bytes = serde_json::to_vec(&feed).unwrap();
    LocalReceipt {
        source,
        feed,
        bytes,
    }
}

#[test]
fn newer_installer_seed_overrules_preserved_older_history_and_rejects_replay() {
    let previous = select_previous(
        vec![
            receipt(ReceiptSource::Highest, 2, b"old version"),
            receipt(ReceiptSource::Accepted, 2, b"old version"),
            receipt(ReceiptSource::Seed, 3, b"new installed version"),
        ],
        b"new installed version",
    )
    .unwrap()
    .unwrap();
    assert_eq!(previous.0.sequence, 3);
    let replay = receipt(ReceiptSource::Highest, 2, b"old version");
    assert!(replay
        .feed
        .check_advance(&replay.bytes, Some((&previous.0, &previous.1)))
        .is_err());
}

#[test]
fn normal_update_keeps_newer_history_even_when_initial_seed_no_longer_matches() {
    let previous = select_previous(
        vec![
            receipt(ReceiptSource::Highest, 4, b"updated version"),
            receipt(ReceiptSource::Accepted, 4, b"updated version"),
            receipt(ReceiptSource::Seed, 3, b"initial version"),
        ],
        b"updated version",
    )
    .unwrap()
    .unwrap();
    assert_eq!(previous.0.sequence, 4);
    previous
        .0
        .check_advance(&previous.1, Some((&previous.0, &previous.1)))
        .unwrap();
}

#[test]
fn same_sequence_with_different_authenticated_bytes_fails_closed() {
    assert!(select_previous(
        vec![
            receipt(ReceiptSource::Highest, 3, b"one version"),
            receipt(ReceiptSource::Seed, 3, b"another version"),
        ],
        b"another version",
    )
    .is_err());
    // Do not hide a conflicting older receipt behind a higher maximum.
    assert!(select_previous(
        vec![
            receipt(ReceiptSource::Highest, 4, b"latest"),
            receipt(ReceiptSource::Accepted, 3, b"one version"),
            receipt(ReceiptSource::Seed, 3, b"another version"),
        ],
        b"latest",
    )
    .is_err());
}

#[test]
fn seed_only_requires_exact_installed_version_binding() {
    let matching = select_previous(
        vec![receipt(ReceiptSource::Seed, 3, b"initial version")],
        b"initial version",
    )
    .unwrap()
    .unwrap();
    assert_eq!(matching.0.sequence, 3);
    assert!(select_previous(
        vec![receipt(ReceiptSource::Seed, 3, b"initial version")],
        b"updated version with lost history",
    )
    .is_err());
    assert!(select_previous(Vec::new(), b"initial version").is_err());
}

#[test]
fn accepted_receipt_participates_even_if_highest_pointer_is_older() {
    let previous = select_previous(
        vec![
            receipt(ReceiptSource::Highest, 2, b"old version"),
            receipt(ReceiptSource::Accepted, 4, b"new version"),
            receipt(ReceiptSource::Seed, 3, b"initial version"),
        ],
        b"new version",
    )
    .unwrap()
    .unwrap();
    assert_eq!(previous.0.sequence, 4);
}
