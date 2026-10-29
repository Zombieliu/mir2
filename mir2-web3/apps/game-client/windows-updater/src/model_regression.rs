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
    const SIMPLIFIED_ROOT_FILES: [&str; 5] = [
        "README.zh-CN.txt",
        "BILLING-HAN-SOURCE.json",
        "OFL-NotoSansSC-BillingHan.txt",
        "SIMPLIFIED-SOURCE.json",
        "OFL-NotoSansCJKsc.txt",
    ];

    #[test]
    fn simplified_release_root_files_are_allowed_exactly() {
        for path in SIMPLIFIED_ROOT_FILES {
            game_path(path).unwrap();
        }
    }

    #[test]
    fn simplified_release_root_near_misses_remain_rejected() {
        for path in SIMPLIFIED_ROOT_FILES {
            for near_miss in [
                path.to_ascii_lowercase(),
                format!("extra-{path}"),
                format!("{path}.json"),
                format!("{path}.exe"),
                format!("nested/{path}"),
                format!("{path}/data.txt"),
            ] {
                assert!(game_path(&near_miss).is_err(), "{near_miss}");
            }
            game_path(&format!("mir2-assets/legal/{path}")).unwrap();
            for near in [
                format!("mir2-assets/legal/{}", path.to_ascii_lowercase()),
                format!("mir2-assets/legal/extra-{path}"),
                format!("mir2-assets/legal/nested/{path}"),
                format!("mir2-assets/legal/{path}.json"),
            ] {
                assert!(game_path(&near).is_err(), "{near}");
            }
        }
    }

    #[test]
    fn document_layout_accepts_old_or_new_complete_and_rejects_mix_partial_or_duplicate() {
        let parse = |documents: Vec<String>| {
            let mut files = vec![
                entry("mir2-platform-windows.exe", b"x"),
                entry("BUILD-ATTESTATION.json", b"x"),
            ];
            files.extend(documents.iter().map(|path| entry(path, b"license")));
            files.sort_by(|a, b| a.path.cmp(&b.path));
            let canonical = files
                .iter()
                .map(|file| format!("{}\t{}\t{}\n", file.path, file.size, file.sha256))
                .collect::<String>();
            Manifest::parse(&serde_json::to_vec(&serde_json::json!({
                "schema":"mir2.windows.package-manifest.v4", "coverage":{"excludes":META,"rule":"exact"},
                "fileCount":files.len(), "totalBytes":files.iter().map(|file|file.size).sum::<u64>(),
                "aggregateSha256":hash(canonical.as_bytes()), "files":files
            })).unwrap())
        };
        let old = SIMPLIFIED_DOCUMENTS
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();
        let new = SIMPLIFIED_DOCUMENTS
            .iter()
            .map(|name| format!("mir2-assets/legal/{name}"))
            .collect::<Vec<_>>();
        assert!(parse(vec![]).is_ok()); // historical ROOT12 release
        assert!(parse(old.clone()).is_ok()); // immutable r10
        assert!(parse(new.clone()).is_ok());
        assert!(parse(old[..1].to_vec()).is_err());
        assert!(parse(new[..1].to_vec()).is_err());
        let mut mixed = old.clone();
        mixed[0] = new[0].clone();
        assert!(parse(mixed).is_err());
        let mut duplicated = old;
        duplicated.push(new[0].clone());
        assert!(parse(duplicated).is_err());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an existing CMS-signed Candidate; reads only, never launches"]
    fn simplified_release_existing_signed_candidate_reads_complete_manifest() {
        let root = std::path::PathBuf::from(
            std::env::var_os("MIR2_UPDATER_TEST_CANDIDATE_ROOT")
                .expect("set MIR2_UPDATER_TEST_CANDIDATE_ROOT to the immutable signed Candidate"),
        );
        assert!(root.is_absolute());
        // Read the immutable package through the same bounded metadata, pinned
        // CMS, manifest/binding and executable/attestation gates as startup.
        let metadata: std::collections::BTreeMap<String, Vec<u8>> = META
            .iter()
            .map(|name| {
                (
                    (*name).to_owned(),
                    crate::fs_safe::read_bounded(&root.join(name), MAX_META).unwrap(),
                )
            })
            .collect();
        crate::platform::verify_cms(
            &metadata["RELEASE-STATEMENT.json"],
            &metadata["RELEASE-STATEMENT.p7s"],
            crate::SIGNING_KEY,
        )
        .unwrap();
        let manifest = bind_game(&metadata).unwrap();
        assert_eq!(manifest.file_count, manifest.files.len());
        let files = manifest.map();
        for path in ["mir2-platform-windows.exe", "BUILD-ATTESTATION.json"] {
            assert!(crate::fs_safe::matches(&root.join(path), files[path]).unwrap());
        }
        for path in SIMPLIFIED_ROOT_FILES {
            assert!(files.contains_key(path), "{path}");
        }
        let version: serde_json::Value = serde_json::from_slice(&metadata["VERSION.json"]).unwrap();
        let identity = version["candidate"].as_str().unwrap();
        println!(
            "Verified pinned CMS and complete manifest: {identity}, {} payload files, {} bytes",
            manifest.file_count, manifest.total_bytes
        );
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
