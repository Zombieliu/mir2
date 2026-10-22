use super::*;
use crate::model::{hash, Manifest, MAX_META};
use std::{cell::Cell, fs, time::Instant};

#[derive(Default)]
struct Quiet {
    cancelled: Cell<bool>,
}
impl Status for Quiet {
    fn set(&self, _: &str, _: u32) {}
    fn cancelled(&self) -> bool {
        self.cancelled.get()
    }
}
fn entry(path: &str, bytes: &[u8]) -> FileEntry {
    FileEntry {
        path: path.into(),
        size: bytes.len() as u64,
        sha256: hash(bytes),
    }
}
fn write(root: &Path, path: &str, bytes: &[u8]) {
    let target = root.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, bytes).unwrap();
}

#[test]
fn serial_and_parallel_full_hashes_keep_manifest_pending_order_and_caller_progress() {
    let dir = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    let pending = [1, 7, 25, 49];
    for index in 0..64 {
        // Deliberately reverse path order and vary I/O sizes. Result order must
        // follow the manifest, not alphabetical paths or worker completion.
        let path = format!("mir2-assets/nested/{:03}.json", 63 - index);
        let bytes = vec![index as u8; if index % 4 == 0 { 65536 } else { 37 }];
        files.push(entry(&path, &bytes));
        if !pending.contains(&index) {
            write(dir.path(), &path, &bytes);
        }
    }
    for workers in [1, 4] {
        let caller = std::thread::current().id();
        let mut progress = Vec::new();
        let result = scan_with_workers(dir.path(), &files, &Quiet::default(), workers, |done| {
            assert_eq!(std::thread::current().id(), caller);
            progress.push(done);
        })
        .unwrap();
        assert_eq!(result, pending);
        assert_eq!(progress, (1..=files.len()).collect::<Vec<_>>());
    }
}

#[test]
fn same_length_and_restored_mtime_corruption_still_requires_repair() {
    let dir = tempfile::tempdir().unwrap();
    let path = "mir2-assets/same.json";
    let expected = entry(path, b"good-data");
    write(dir.path(), path, b"good-data");
    let target = dir.path().join(path);
    let original = fs::metadata(&target).unwrap();
    let modified = original.modified().unwrap();
    fs::write(&target, b"evil-data").unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&target)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    let changed = fs::metadata(&target).unwrap();
    assert_eq!(changed.len(), original.len());
    assert_eq!(changed.modified().unwrap(), modified);
    for workers in [1, 4] {
        assert_eq!(
            scan_with_workers(
                dir.path(),
                std::slice::from_ref(&expected),
                &Quiet::default(),
                workers,
                |_| {}
            )
            .unwrap(),
            vec![0]
        );
    }
}

#[test]
fn missing_length_mismatch_empty_and_valid_files_have_exact_pending_indexes() {
    let dir = tempfile::tempdir().unwrap();
    let files = vec![
        entry("mir2-assets/short.json", b"longer"),
        entry("mir2-assets/missing-empty.json", b""),
        entry("mir2-assets/empty.json", b""),
        entry("mir2-assets/good.json", b"good"),
    ];
    write(dir.path(), &files[0].path, b"short");
    write(dir.path(), &files[2].path, b"");
    write(dir.path(), &files[3].path, b"good");
    for workers in [1, 4] {
        assert_eq!(
            scan_with_workers(dir.path(), &files, &Quiet::default(), workers, |_| {}).unwrap(),
            vec![0, 1]
        );
    }
}

#[test]
fn hard_link_and_unsafe_paths_fail_without_changing_external_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let external = outside.path().join("original.json");
    fs::write(&external, b"preserved").unwrap();
    let linked = dir.path().join("linked.json");
    fs::hard_link(&external, &linked).unwrap();
    for workers in [1, 4] {
        let result = scan_with_workers(
            dir.path(),
            &[entry("linked.json", b"preserved")],
            &Quiet::default(),
            workers,
            |_| {},
        );
        assert!(result.unwrap_err().to_string().contains("hard-linked"));
        for path in [
            "../original.json",
            "asset.json:stream",
            "CON",
            "/absolute.json",
        ] {
            assert!(
                scan_with_workers(
                    dir.path(),
                    &[entry(path, b"preserved")],
                    &Quiet::default(),
                    workers,
                    |_| {}
                )
                .is_err(),
                "accepted {path}"
            );
        }
    }
    assert_eq!(fs::read(external).unwrap(), b"preserved");
    assert_eq!(fs::read(linked).unwrap(), b"preserved");
}

#[test]
fn producer_cancellation_refuses_pending_result_and_next_phase() {
    struct CancelAfterPolls(Cell<usize>);
    impl Status for CancelAfterPolls {
        fn set(&self, _: &str, _: u32) {}
        fn cancelled(&self) -> bool {
            let polls = self.0.get() + 1;
            self.0.set(polls);
            // The first poll is scan admission; the second is the producer's
            // first job, before any worker completion can be observed.
            polls >= 2
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let files = (0..128)
        .map(|i| entry(&format!("missing-{i}.json"), b"payload"))
        .collect::<Vec<_>>();
    for workers in [1, 4] {
        let status = CancelAfterPolls(Cell::new(0));
        let next_phase = Cell::new(false);
        let result = scan_with_workers(dir.path(), &files, &status, workers, |_| {
            panic!("cancelled producer emitted a completion")
        })
        .map(|_| next_phase.set(true));
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert!(!next_phase.get());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}

#[test]
fn observer_cancellation_is_checked_after_callback_including_final_file() {
    let dir = tempfile::tempdir().unwrap();
    for workers in [1, 4] {
        for count in [1, 32] {
            let status = Quiet::default();
            let files = (0..count)
                .map(|i| entry(&format!("missing-{i}.json"), b"payload"))
                .collect::<Vec<_>>();
            let stop_at = count.min(3);
            let next_phase = Cell::new(false);
            let mut observed = 0;
            let result = scan_with_workers(dir.path(), &files, &status, workers, |done| {
                observed = done;
                if done == stop_at {
                    status.cancelled.set(true);
                }
            })
            .map(|_| next_phase.set(true));
            assert!(result.unwrap_err().to_string().contains("cancelled"));
            assert_eq!(observed, stop_at);
            assert!(!next_phase.get());
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        }
    }
}

#[test]
fn cancelled_empty_scan_and_invalid_worker_counts_cannot_succeed() {
    let dir = tempfile::tempdir().unwrap();
    let cancelled = Quiet {
        cancelled: Cell::new(true),
    };
    assert!(scan(dir.path(), &[], &cancelled, |_| {}).is_err());
    for workers in [0, 5] {
        assert!(scan_with_workers(dir.path(), &[], &Quiet::default(), workers, |_| {}).is_err());
    }
}

/// Opt in only with an absolute game directory and its absolute real manifest.
/// This does not acquire an install lock, create fixtures, repair files, log to
/// the installation or contact a server. Run against a quiescent installation.
#[test]
#[ignore = "read-only real manifest benchmark; requires MIR2_UPDATER_BENCH_GAME_DIR, MIR2_UPDATER_BENCH_MANIFEST and MIR2_UPDATER_BENCH_WORKERS (1 or 4)"]
fn readonly_installed_scan_benchmark() {
    let total_start = Instant::now();
    let game_dir = std::path::PathBuf::from(
        std::env::var_os("MIR2_UPDATER_BENCH_GAME_DIR").expect("absolute game directory required"),
    );
    let manifest_path = std::path::PathBuf::from(
        std::env::var_os("MIR2_UPDATER_BENCH_MANIFEST").expect("absolute manifest path required"),
    );
    let workers = std::env::var("MIR2_UPDATER_BENCH_WORKERS")
        .expect("explicit 1 or 4 workers required")
        .parse::<usize>()
        .unwrap();
    assert!(game_dir.is_absolute() && manifest_path.is_absolute());
    assert!(game_dir.is_dir());
    assert!([1, 4].contains(&workers));
    safe::ancestors(&game_dir).unwrap();
    let read_start = Instant::now();
    let before = safe::read_bounded(&manifest_path, MAX_META).unwrap();
    let before_hash = hash(&before);
    let read_ms = read_start.elapsed().as_millis();
    let parse_start = Instant::now();
    let manifest = Manifest::parse(&before).unwrap();
    let parse_ms = parse_start.elapsed().as_millis();
    let scan_start = Instant::now();
    let mut completed = 0;
    let pending = scan_with_workers(
        &game_dir,
        &manifest.files,
        &Quiet::default(),
        workers,
        |done| completed = done,
    )
    .unwrap();
    let scan_ms = scan_start.elapsed().as_millis();
    assert_eq!(completed, manifest.file_count);
    let pending_identity = pending
        .iter()
        .map(|index| {
            let file = &manifest.files[*index];
            format!("{index}\t{}\t{}\t{}\n", file.path, file.size, file.sha256)
        })
        .collect::<String>();
    let recheck_start = Instant::now();
    let after = safe::read_bounded(&manifest_path, MAX_META).unwrap();
    let after_hash = hash(&after);
    assert_eq!(
        before_hash, after_hash,
        "benchmark manifest changed during scan"
    );
    let recheck_ms = recheck_start.elapsed().as_millis();
    println!(
        "{}",
        serde_json::json!({
            "benchmark":"readonly-installed-scan", "readOnly":true,
            "workers":workers, "files":manifest.file_count, "manifestTotalBytes":manifest.total_bytes,
            "completed":completed, "pending":pending.len(), "pendingDigest":hash(pending_identity.as_bytes()),
            "manifestSha256Before":before_hash, "manifestSha256After":after_hash,
            "manifestReadMs":read_ms, "manifestParseMs":parse_ms, "scanMs":scan_ms,
            "manifestRecheckMs":recheck_ms, "totalMs":total_start.elapsed().as_millis()
        })
    );
}
