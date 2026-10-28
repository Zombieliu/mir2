use super::*;
use flate2::{write::GzEncoder, Compression};
use std::cell::RefCell;

#[derive(Default)]
struct CapturedStatus {
    rows: RefCell<Vec<(String, u32)>>,
    cancel: std::cell::Cell<bool>,
}
impl Status for CapturedStatus {
    fn set(&self, key: &str, percent: u32) {
        self.rows.borrow_mut().push((key.into(), percent));
    }
    fn cancelled(&self) -> bool {
        self.cancel.get()
    }
}

struct OnlyArchive {
    bytes: Vec<u8>,
    downloads: RefCell<usize>,
}
impl Source for OnlyArchive {
    fn get(&self, _: &str, _: u64, _: &mut dyn FnMut(u64) -> Result<()>) -> Result<Vec<u8>> {
        anyhow::bail!("this bounded test does not fetch signed metadata")
    }
    fn download(
        &self,
        _: &str,
        _: &FileEntry,
        path: &Path,
        cb: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()> {
        *self.downloads.borrow_mut() += 1;
        safe::write_new(path, &self.bytes)?;
        cb(self.bytes.len() as u64)
    }
}

fn file(path: &str, bytes: &[u8]) -> FileEntry {
    FileEntry {
        path: path.into(),
        size: bytes.len() as u64,
        sha256: hash(bytes),
    }
}

#[test]
fn red_native_local_phases_are_translated_for_nine_locales() {
    for locale in ["en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"] {
        for phase in ["extracting", "staging", "preparing", "installing"] {
            let label = crate::ui::stage(locale, phase);
            assert_ne!(
                label, phase,
                "{locale}/{phase} is an untranslated unknown stage"
            );
            assert_ne!(label, crate::ui::stage(locale, "downloading"));
        }
    }
}

#[test]
fn red_actual_bundle_reports_completed_entries_not_only_verifying_30() {
    let root = tempfile::tempdir().unwrap();
    let data = [
        ("mir2-assets/a.txt", &b"alpha"[..]),
        ("mir2-assets/b.txt", &b"beta"[..]),
    ];
    let files: Vec<_> = data.iter().map(|(p, b)| file(p, b)).collect();
    let mut raw = delivery::BUNDLE_MAGIC.to_vec();
    raw.extend_from_slice(&(data.len() as u32).to_le_bytes());
    for (path, bytes) in data {
        raw.extend_from_slice(&(path.len() as u16).to_le_bytes());
        raw.extend_from_slice(path.as_bytes());
        raw.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        raw.extend_from_slice(bytes);
    }
    let mut zip = GzEncoder::new(Vec::new(), Compression::fast());
    zip.write_all(&raw).unwrap();
    let bytes = zip.finish().unwrap();
    let archive = file("delivery/progress.m2b.gz", &bytes);
    let descriptor = delivery::Delivery {
        schema: "mir2.windows.delivery.v1".into(),
        candidate: "test".into(),
        package_manifest_sha256: hash(b"manifest"),
        version_sha256: hash(b"version"),
        built_unix: now(),
        patches: vec![],
        bundles: vec![delivery::Bundle {
            archive: archive.clone(),
            files: files.iter().map(|f| f.path.clone()).collect(),
        }],
    };
    let manifest = Manifest {
        schema: "mir2.windows.package-manifest.v4".into(),
        coverage: Coverage {
            excludes: META.iter().map(|p| p.to_string()).collect(),
            rule: "test".into(),
        },
        file_count: files.len(),
        total_bytes: files.iter().map(|f| f.size).sum(),
        aggregate_sha256: hash(b"unused lower-level manifest"),
        files: files.clone(),
    };
    let source = OnlyArchive {
        bytes,
        downloads: RefCell::new(0),
    };
    let component = Component {
        directory: "releases/test".into(),
        identity: "test".into(),
        metadata: vec![],
    };
    let status = CapturedStatus::default();
    let mut transfers = Transfers::default();
    let mut stats = delivery::Stats::default();
    delivery::Acceleration {
        root: root.path(),
        source: &source,
        component: &component,
        status: &status,
        total: manifest.total_bytes,
        transfers: &mut transfers,
        stats: &mut stats,
    }
    .run(&descriptor, &manifest, &files)
    .unwrap();
    assert_eq!(*source.downloads.borrow(), 1);
    assert_eq!(transfers.downloaded_bytes, archive.size);
    for entry in &files {
        assert!(safe::matches(
            &root
                .path()
                .join(format!(".update/downloads/{}", entry.sha256)),
            entry
        )
        .unwrap());
    }
    assert!(
        status
            .rows
            .borrow()
            .iter()
            .any(|(key, _)| key == "extracting:0/2"),
        "no extraction start/count"
    );
    assert!(
        status
            .rows
            .borrow()
            .iter()
            .any(|(key, _)| key == "extracting:2/2"),
        "no completed extraction count"
    );
    let trace = fs::read_to_string(root.path().join(".update/last-run.jsonl")).unwrap();
    assert!(trace
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .any(|v| v["event"] == "local-phase-end"
            && v["detail"]["phase"] == "extracting"
            && v["detail"]["completed"] == 2
            && v["detail"]["outcome"] == "complete"));
}

#[test]
fn red_legacy_transaction_produces_bounded_actual_phase_end_trace() {
    let root = tempfile::tempdir().unwrap();
    safe::write_new(&root.path().join("game/personal-file.txt"), b"keep").unwrap();
    let changes = vec![
        transaction::stage_bytes(root.path(), "game/README-START.txt", b"readme").unwrap(),
        transaction::stage_bytes(root.path(), "game/CONTROLS.txt", b"controls").unwrap(),
    ];
    transaction::prepare_and_apply(root.path(), &changes).unwrap();
    assert!(transaction::pending_activation(root.path()).unwrap());
    let trace = fs::read_to_string(root.path().join(".update/last-run.jsonl"))
        .expect("legacy transaction does not expose its local prepare/commit duration");
    let rows: Vec<serde_json::Value> = trace
        .lines()
        .map(|x| serde_json::from_str(x).unwrap())
        .filter(|x: &serde_json::Value| x["event"] == "local-phase-end")
        .collect();
    for phase in ["preparing", "installing"] {
        let row = rows
            .iter()
            .find(|x| x["detail"]["phase"] == phase)
            .expect("missing real transaction phase");
        assert_eq!(row["detail"]["completed"], 2);
        assert_eq!(row["detail"]["total"], 2);
        assert!(row["detail"]["elapsedMs"].is_number());
        assert_eq!(row["detail"]["outcome"], "complete");
    }
    assert_eq!(
        rows.len(),
        2,
        "trace must be phase-end only, not one disk row per file"
    );
    transaction::rollback(root.path()).unwrap();
    assert_eq!(
        fs::read(root.path().join("game/personal-file.txt")).unwrap(),
        b"keep"
    );
}

fn phase_rows(root: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(root.join(".update/last-run.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|row| row["event"] == "local-phase-end")
        .collect()
}

#[test]
fn cached_payloads_advance_staging_counts_with_zero_wire_and_preserve_personal() {
    let root = tempfile::tempdir().unwrap();
    let files = vec![
        file("README-START.txt", b"readme"),
        file("CONTROLS.txt", b"controls"),
        file("mir2-assets/a.txt", b"asset"),
    ];
    for entry in &files {
        let bytes = match entry.path.as_str() {
            "README-START.txt" => &b"readme"[..],
            "CONTROLS.txt" => &b"controls"[..],
            _ => &b"asset"[..],
        };
        safe::write_new(
            &root
                .path()
                .join(format!(".update/downloads/{}", entry.sha256)),
            bytes,
        )
        .unwrap();
    }
    safe::write_new(&root.path().join("game/personal-file.txt"), b"personal").unwrap();
    let raw = CapturedStatus::default();
    let status = ThrottledStatus::new(&raw);
    let source = OnlyArchive {
        bytes: vec![],
        downloads: RefCell::new(0),
    };
    let mut transfers = Transfers::default();
    let changes = stage_payloads(
        root.path(),
        &source,
        "unused",
        &status,
        &files,
        19,
        &mut transfers,
    )
    .unwrap();
    assert_eq!(*source.downloads.borrow(), 0);
    assert_eq!(transfers.downloaded_files, 0);
    assert_eq!(transfers.wire_bytes, 0);
    assert!(raw.rows.borrow().iter().any(|(k, _)| k == "staging:3/3"));
    for entry in &files {
        assert!(safe::matches(
            &root.path().join(".update/staging/game").join(&entry.path),
            entry
        )
        .unwrap());
    }
    let rows = phase_rows(root.path());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["detail"]["cacheHits"], 3);
    assert_eq!(rows[0]["detail"]["completed"], 3);
    assert_eq!(rows[0]["detail"]["writtenBytes"], 19);
    assert!(rows[0]["detail"]["payloadResolveMs"].is_number());
    assert!(rows[0]["detail"]["copyFlushMs"].is_number());
    transaction::prepare_and_apply_observed(
        root.path(),
        &changes,
        &mut TransactionStatus { status: &status },
    )
    .unwrap();
    assert!(raw.rows.borrow().iter().any(|(k, _)| k == "preparing:3/3"));
    assert!(raw.rows.borrow().iter().any(|(k, _)| k == "installing:3/3"));
    transaction::rollback_observed(root.path(), &mut TransactionStatus { status: &status })
        .unwrap();
    assert!(raw.rows.borrow().iter().any(|(k, _)| k == "recovering:3/3"));
    assert_eq!(
        fs::read(root.path().join("game/personal-file.txt")).unwrap(),
        b"personal"
    );
    assert_eq!(phase_rows(root.path()).len(), 4);
}

#[test]
fn stage_failure_preserves_actual_partial_count_and_completed_cache() {
    let root = tempfile::tempdir().unwrap();
    let good = file("README-START.txt", b"good");
    let bad = file("CONTROLS.txt", b"expected");
    safe::write_new(
        &root
            .path()
            .join(format!(".update/downloads/{}", good.sha256)),
        b"good",
    )
    .unwrap();
    safe::write_new(&root.path().join("game/personal-file.txt"), b"personal").unwrap();
    let source = OnlyArchive {
        bytes: b"corrupted".to_vec(),
        downloads: RefCell::new(0),
    };
    let raw = CapturedStatus::default();
    let status = ThrottledStatus::new(&raw);
    assert!(stage_payloads(
        root.path(),
        &source,
        "unused",
        &status,
        &[good.clone(), bad.clone()],
        12,
        &mut Transfers::default()
    )
    .is_err());
    let rows = phase_rows(root.path());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["detail"]["outcome"], "unfinished");
    assert_eq!(rows[0]["detail"]["completed"], 1);
    assert_eq!(rows[0]["detail"]["total"], 2);
    assert!(safe::matches(
        &root
            .path()
            .join(format!(".update/downloads/{}", good.sha256)),
        &good
    )
    .unwrap());
    assert!(!root
        .path()
        .join(format!(".update/downloads/{}.part", bad.sha256))
        .exists());
    assert!(!root.path().join(".update/transaction.json").exists());
    assert!(!root.path().join("game/README-START.txt").exists());
    assert_eq!(
        fs::read(root.path().join("game/personal-file.txt")).unwrap(),
        b"personal"
    );
}

#[test]
fn display_cadence_throttles_only_snapshots_and_forces_phase_and_final() {
    let raw = CapturedStatus::default();
    let status = ThrottledStatus::new(&raw);
    let start = Instant::now();
    status.send_at("staging:0/10", 70, false, start);
    for index in 1..10 {
        status.send_at(
            &format!("staging:{index}/10"),
            75,
            false,
            start + Duration::from_millis(index * 10),
        );
    }
    assert_eq!(raw.rows.borrow().len(), 1);
    status.send_at(
        "staging:9/10",
        80,
        false,
        start + Duration::from_millis(199),
    );
    assert_eq!(raw.rows.borrow().len(), 1);
    status.send_at(
        "staging:9/10",
        80,
        false,
        start + Duration::from_millis(200),
    );
    assert_eq!(raw.rows.borrow().len(), 2);
    status.send_at(
        "staging:10/10",
        84,
        true,
        start + Duration::from_millis(201),
    );
    status.send_at(
        "preparing:0/10",
        85,
        false,
        start + Duration::from_millis(202),
    );
    assert_eq!(raw.rows.borrow().len(), 4);
    raw.cancel.set(true);
    assert!(status.cancelled(), "cancellation must not be throttled");
}

#[test]
fn commit_observer_cancellation_does_not_interrupt_applying_journal() {
    struct Cancel<'a> {
        status: &'a CapturedStatus,
    }
    impl transaction::ProgressObserver for Cancel<'_> {
        fn observe(&mut self, phase: &'static str, done: usize, _: usize, boundary: bool) {
            if phase == "installing" && done == 1 && !boundary {
                self.status.cancel.set(true);
            }
        }
    }
    let root = tempfile::tempdir().unwrap();
    let status = CapturedStatus::default();
    safe::write_new(&root.path().join("game/personal-file.txt"), b"personal").unwrap();
    let changes = vec![
        transaction::stage_bytes(root.path(), "game/README-START.txt", b"a").unwrap(),
        transaction::stage_bytes(root.path(), "game/CONTROLS.txt", b"b").unwrap(),
    ];
    transaction::prepare_and_apply_observed(root.path(), &changes, &mut Cancel { status: &status })
        .unwrap();
    assert!(status.cancelled());
    assert!(transaction::pending_activation(root.path()).unwrap());
    assert!(
        !transaction::recover(root.path()).unwrap(),
        "committed is pending launch, not an applying crash"
    );
    assert!(
        progress(&status, "launching", 100).is_err(),
        "cancelled commit must not launch"
    );
    let rows = phase_rows(root.path());
    assert_eq!(
        rows.iter()
            .find(|x| x["detail"]["phase"] == "installing")
            .unwrap()["detail"]["outcome"],
        "complete"
    );
    assert_eq!(
        fs::read(root.path().join("game/CONTROLS.txt")).unwrap(),
        b"b"
    );
    transaction::rollback(root.path()).unwrap();
}

#[test]
fn interrupted_commit_observation_retains_recovery_schema_and_exact_old_files() {
    struct Nothing;
    impl transaction::ProgressObserver for Nothing {
        fn observe(&mut self, _: &'static str, _: usize, _: usize, _: bool) {}
    }
    let root = tempfile::tempdir().unwrap();
    safe::write_new(&root.path().join("game/README-START.txt"), b"old").unwrap();
    safe::write_new(&root.path().join("game/personal-file.txt"), b"personal").unwrap();
    let changes = vec![
        transaction::stage_bytes(root.path(), "game/README-START.txt", b"new").unwrap(),
        transaction::stage_bytes(root.path(), "game/CONTROLS.txt", b"new controls").unwrap(),
    ];
    assert!(transaction::apply_steps_observed(
        root.path(),
        &changes,
        |step| {
            ensure!(step != 1, "owned interruption");
            Ok(())
        },
        &mut Nothing
    )
    .is_err());
    let j: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join(".update/transaction.json")).unwrap())
            .unwrap();
    assert_eq!(j["schema"], 1);
    assert_eq!(j["phase"], "applying");
    let rows = phase_rows(root.path());
    let install = rows
        .iter()
        .find(|r| r["detail"]["phase"] == "installing")
        .unwrap();
    assert_eq!(install["detail"]["completed"], 1);
    assert_eq!(install["detail"]["outcome"], "unfinished");
    assert!(transaction::recover(root.path()).unwrap());
    assert_eq!(
        fs::read(root.path().join("game/README-START.txt")).unwrap(),
        b"old"
    );
    assert!(!root.path().join("game/CONTROLS.txt").exists());
    assert_eq!(
        fs::read(root.path().join("game/personal-file.txt")).unwrap(),
        b"personal"
    );
    assert!(!transaction::recover(root.path()).unwrap());
}

#[test]
fn completed_counts_render_in_all_nine_locales_without_download_label() {
    for locale in ["en", "zh-TW", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"] {
        for phase in [
            "extracting",
            "staging",
            "preparing",
            "installing",
            "checking-installed",
            "recovering",
        ] {
            let rendered = crate::ui::stage(locale, &format!("{phase}:2/3"));
            assert!(rendered.ends_with("(2 / 3)"));
            assert!(rendered.starts_with(&crate::ui::stage(locale, phase)));
            assert_ne!(rendered, crate::ui::stage(locale, "downloading"));
        }
    }
    assert_eq!(crate::ui::stage("en", "staging:4/3"), "staging:4/3");
}

#[test]
fn cancellation_before_staging_keeps_verified_cache_and_never_opens_journal() {
    let root = tempfile::tempdir().unwrap();
    let entry = file("README-START.txt", b"verified");
    safe::write_new(
        &root
            .path()
            .join(format!(".update/downloads/{}", entry.sha256)),
        b"verified",
    )
    .unwrap();
    safe::write_new(&root.path().join("game/personal-file.txt"), b"personal").unwrap();
    let status = CapturedStatus::default();
    status.cancel.set(true);
    let source = OnlyArchive {
        bytes: vec![],
        downloads: RefCell::new(0),
    };
    assert!(stage_payloads(
        root.path(),
        &source,
        "unused",
        &status,
        &[entry.clone()],
        entry.size,
        &mut Transfers::default()
    )
    .is_err());
    assert_eq!(*source.downloads.borrow(), 0);
    assert!(safe::matches(
        &root
            .path()
            .join(format!(".update/downloads/{}", entry.sha256)),
        &entry
    )
    .unwrap());
    assert!(!root
        .path()
        .join(".update/staging/game/README-START.txt")
        .exists());
    assert!(!root.path().join(".update/transaction.json").exists());
    assert_eq!(phase_rows(root.path())[0]["detail"]["completed"], 0);
    assert_eq!(
        phase_rows(root.path())[0]["detail"]["outcome"],
        "unfinished"
    );
    assert_eq!(
        fs::read(root.path().join("game/personal-file.txt")).unwrap(),
        b"personal"
    );
}
