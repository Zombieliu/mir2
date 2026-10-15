#[cfg(test)]
mod regression {
    use crate::model::*;
    fn entry(path: &str, bytes: &[u8]) -> FileEntry {
        FileEntry {
            path: path.into(),
            size: bytes.len() as u64,
            sha256: hash(bytes),
        }
    }
    fn feed() -> Feed {
        Feed {
            schema: "mir2.windows.update-feed.v1".into(),
            channel: "invited".into(),
            platform: "windows-x64".into(),
            sequence: 2,
            created_unix: 1000,
            expires_unix: 2000,
            min_bootstrap: 1,
            protocol: "crystal-mir2-v1".into(),
            content: "mir2.windows.package-manifest.v4".into(),
            game: Component {
                directory: "releases/game-r6".into(),
                identity: "WN-r6".into(),
                metadata: META.iter().map(|p| entry(p, b"x")).collect(),
            },
            engine: Component {
                directory: "releases/updater-v1".into(),
                identity: "1".into(),
                metadata: ["ENGINE.json", "ENGINE.p7s"]
                    .iter()
                    .map(|p| entry(p, b"x"))
                    .collect(),
            },
        }
    }
    #[test]
    fn signed_sequence_may_advance_but_not_rollback_or_reuse() {
        let old = feed();
        let bytes = serde_json::to_vec(&old).unwrap();
        let mut next = feed();
        next.sequence = 3;
        next.check_advance(b"new", Some((&old, &bytes))).unwrap();
        next.sequence = 1;
        assert!(next.check_advance(b"old", Some((&old, &bytes))).is_err());
        next.sequence = 2;
        assert!(next
            .check_advance(b"changed", Some((&old, &bytes)))
            .is_err());
        next.check_advance(&bytes, Some((&old, &bytes))).unwrap();
    }
    #[test]
    fn freshness_compatibility_and_metadata_sets_are_required() {
        let valid = serde_json::to_vec(&feed()).unwrap();
        Feed::parse(&valid, 1500, true).unwrap();
        assert!(Feed::parse(&valid, 3000, true).is_err());
        assert!(Feed::parse(&valid, 600, true).is_err());
        Feed::parse(&valid, 3000, false).unwrap(); // authenticated historic receipts
        for field in ["channel", "platform", "schema", "protocol", "content"] {
            let mut value: serde_json::Value = serde_json::from_slice(&valid).unwrap();
            value[field] = "wrong".into();
            assert!(
                Feed::parse(&serde_json::to_vec(&value).unwrap(), 1500, true).is_err(),
                "{field}"
            );
        }
        let mut value: serde_json::Value = serde_json::from_slice(&valid).unwrap();
        value["minBootstrap"] = 2.into();
        assert!(Feed::parse(&serde_json::to_vec(&value).unwrap(), 1500, true).is_err());
        value["minBootstrap"] = 1.into();
        value["game"]["directory"] = "../escape".into();
        assert!(Feed::parse(&serde_json::to_vec(&value).unwrap(), 1500, true).is_err());
        let mut f = feed();
        f.engine.metadata.push(entry("cmd.exe", b"bad"));
        assert!(Feed::parse(&serde_json::to_vec(&f).unwrap(), 1500, true).is_err());
    }
    #[test]
    fn manifest_rejects_case_insensitive_file_directory_collision() {
        let mut entries = vec![
            entry("mir2-platform-windows.exe", b"x"),
            entry("BUILD-ATTESTATION.json", b"x"),
            entry("mir2-assets/a.json", b"x"),
            entry("mir2-assets/A.json/tile.png", b"x"),
        ];
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let canon = entries
            .iter()
            .map(|f| format!("{}\t{}\t{}\n", f.path, f.size, f.sha256))
            .collect::<String>();
        let bytes = serde_json::to_vec(
            &serde_json::json!({"schema":"mir2.windows.package-manifest.v4",
            "coverage":{"excludes":META,"rule":"signed"},"fileCount":entries.len(),"totalBytes":4,
            "aggregateSha256":hash(canon.as_bytes()),"files":entries}),
        )
        .unwrap();
        assert!(Manifest::parse(&bytes).is_err());
    }
}
