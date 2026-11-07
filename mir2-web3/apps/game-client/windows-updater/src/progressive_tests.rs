use super::*;
use crate::update::DownloadProgress;
use mir2_resource_pack::{build, sha256, BuildOptions, BuildOutput, ResourceInput};
use std::{fs, path::PathBuf, sync::atomic::AtomicUsize};

struct Fixture {
    _temp: tempfile::TempDir,
    source: PathBuf,
    root: PathBuf,
    build: BuildOutput,
    scope: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let loose = temp.path().join("loose");
        fs::create_dir(&loose).unwrap();
        let mut inputs = Vec::new();
        for (name, bytes) in [
            ("boot.png", b"boot".as_slice()),
            ("background.png", b"background".as_slice()),
            ("urgent.png", b"foreground".as_slice()),
        ] {
            fs::write(loose.join(name), bytes).unwrap();
            inputs.push(ResourceInput {
                path: name.into(),
                size: bytes.len() as u64,
                sha256: sha256(bytes),
            });
        }
        let source = temp.path().join("original");
        let build = build(
            &loose,
            &source,
            &inputs,
            None,
            BuildOptions {
                target_pack_bytes: 96,
            },
        )
        .unwrap();
        assert_eq!(build.catalog.packs.len(), 3);
        let root = temp.path().join("install");
        fs::create_dir_all(root.join("objects/packs")).unwrap();
        let scope = serde_json::to_vec(&serde_json::json!({
            "schema":"mir2.native.required-resource-scope.v1", "id":"test",
            "catalogSha256":build.catalog_sha256,"requiredPaths":["boot.png"]
        }))
        .unwrap();
        let boot = &build.catalog.resources["boot.png"].pack_sha256;
        let object = build
            .catalog
            .packs
            .iter()
            .find(|p| &p.sha256 == boot)
            .unwrap();
        fs::copy(
            source.join(object.file_name()),
            root.join("objects/packs").join(object.file_name()),
        )
        .unwrap();
        Self {
            _temp: temp,
            source,
            root,
            build,
            scope,
        }
    }
    fn store(&self) -> Arc<ScopedPackStore> {
        Arc::new(
            ScopedPackStore::open(
                self.root.join("objects/packs"),
                &self.build.catalog_bytes,
                &self.build.catalog_sha256,
                &self.scope,
                &sha256(&self.scope),
            )
            .unwrap(),
        )
    }
}

#[derive(Clone)]
struct Files {
    root: PathBuf,
    calls: Arc<AtomicUsize>,
    failed: Arc<AtomicBool>,
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    partial_once: Arc<AtomicBool>,
}
impl Files {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            calls: Arc::new(AtomicUsize::new(0)),
            failed: Arc::new(AtomicBool::new(false)),
            entered: Arc::new(AtomicBool::new(false)),
            release: Arc::new(AtomicBool::new(true)),
            partial_once: Arc::new(AtomicBool::new(false)),
        }
    }
}
impl Source for Files {
    fn get(&self, _: &str, _: u64, _: &mut dyn FnMut(u64) -> Result<()>) -> Result<Vec<u8>> {
        anyhow::bail!("metadata denied")
    }
    fn download(
        &self,
        _: &str,
        _: &model::FileEntry,
        _: &Path,
        _: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()> {
        anyhow::bail!("use resume fixture")
    }
    fn permits_partial_resume(&self) -> bool {
        true
    }
    fn download_resuming(
        &self,
        path: &str,
        entry: &model::FileEntry,
        destination: &Path,
        progress: &mut dyn FnMut(DownloadProgress) -> Result<()>,
    ) -> Result<u64> {
        ensure!(
            path == entry.path && path.starts_with("objects/packs/"),
            "unexpected path"
        );
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.entered.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.release.load(Ordering::Acquire) {
            ensure!(Instant::now() < deadline, "fixture gate deadline");
            thread::sleep(Duration::from_millis(1));
        }
        let bytes = fs::read(self.root.join(Path::new(path).file_name().unwrap()))?;
        ensure!(!self.failed.load(Ordering::Relaxed), "offline fixture");
        let mut file = safe::open_download(destination)?;
        let reused = file.metadata()?.len();
        progress(DownloadProgress {
            received: 0,
            reused,
        })?;
        use std::io::{Seek, SeekFrom, Write};
        file.seek(SeekFrom::Start(reused))?;
        if self.partial_once.swap(false, Ordering::Relaxed) {
            let n = (bytes.len() - reused as usize).min(20);
            progress(DownloadProgress {
                received: n as u64,
                reused,
            })?;
            file.write_all(&bytes[reused as usize..reused as usize + n])?;
            file.sync_all()?;
            anyhow::bail!("interrupted fixture");
        }
        progress(DownloadProgress {
            received: entry.size - reused,
            reused,
        })?;
        file.write_all(&bytes[reused as usize..])?;
        file.sync_all()?;
        Ok(reused)
    }
}
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !condition() {
        assert!(Instant::now() < deadline, "condition deadline");
        thread::sleep(Duration::from_millis(5));
    }
}
fn stop(downloads: Arc<ProgressiveDownloads>) {
    let mut downloads = Arc::try_unwrap(downloads).ok().unwrap();
    downloads.shared.stop.store(true, Ordering::Relaxed);
    downloads.shared.changed.notify_all();
    downloads.worker.take().unwrap().join().unwrap();
}

#[test]
fn completes_exact_declared_objects_and_admits_without_selector_or_executable_writes() {
    let f = Fixture::new();
    let files = Files::new(f.source.clone());
    let downloads =
        ProgressiveDownloads::start_inner(&f.root, &"A".repeat(64), f.store(), files.clone())
            .unwrap();
    until(|| downloads.snapshot().unwrap().complete);
    assert_eq!(
        downloads.read("urgent.png").unwrap(),
        ScopedRead::Ready(b"foreground".to_vec())
    );
    assert_eq!(files.calls.load(Ordering::Relaxed), 2);
    let snapshot = downloads.snapshot().unwrap();
    assert_eq!(snapshot.available_packs, 3);
    assert_eq!(snapshot.available_bytes, snapshot.total_bytes);
    assert!(!f.root.join("state/active-generation.json").exists());
    assert!(!f.root.join("generations").exists());
    assert!(downloads.read("../secret").is_err());
    assert!(downloads.read("unknown.png").is_err());
    stop(downloads);
}

#[test]
fn pending_is_nonblocking_and_urgent_preempts_background_without_spending_retry_budget() {
    let f = Fixture::new();
    let files = Files::new(f.source.clone());
    files.release.store(false, Ordering::Relaxed);
    let downloads =
        ProgressiveDownloads::start_inner(&f.root, &"B".repeat(64), f.store(), files.clone())
            .unwrap();
    until(|| files.entered.load(Ordering::Acquire));
    let pending = downloads.store.pending_objects().unwrap();
    let first = pending.iter().min_by_key(|p| p.size).unwrap();
    let urgent_path = f
        .build
        .catalog
        .resources
        .iter()
        .find(|(path, e)| path.as_str() != "boot.png" && e.pack_sha256 != first.sha256)
        .unwrap()
        .0;
    assert!(matches!(
        downloads.read(urgent_path).unwrap(),
        ScopedRead::Pending(_)
    ));
    assert_eq!(downloads.snapshot().unwrap().foreground_pending, 1);
    files.release.store(true, Ordering::Release);
    until(|| matches!(downloads.read(urgent_path), Ok(ScopedRead::Ready(_))));
    until(|| downloads.snapshot().unwrap().complete);
    assert!(downloads.shared.queue.lock().unwrap().attempts.is_empty());
    assert_eq!(files.calls.load(Ordering::Relaxed), 3);
    stop(downloads);
}

#[test]
fn interrupted_prefix_survives_restart_and_is_never_admitted_until_full_hash_matches() {
    let f = Fixture::new();
    let files = Files::new(f.source.clone());
    files.partial_once.store(true, Ordering::Relaxed);
    let downloads =
        ProgressiveDownloads::start_inner(&f.root, &"C".repeat(64), f.store(), files.clone())
            .unwrap();
    until(|| downloads.snapshot().unwrap().last_error.is_some());
    let partials = f
        .root
        .join(format!(".update/progressive-parts/{}", "C".repeat(64)));
    let partial = fs::read_dir(&partials)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(fs::metadata(&partial).unwrap().len(), 20);
    assert_eq!(downloads.snapshot().unwrap().available_packs, 1);
    stop(downloads);
    let resumed =
        ProgressiveDownloads::start_inner(&f.root, &"C".repeat(64), f.store(), files.clone())
            .unwrap();
    until(|| resumed.snapshot().unwrap().complete);
    assert_eq!(files.calls.load(Ordering::Relaxed), 3);
    assert!(!partial.exists());
    stop(resumed);
}

#[test]
fn corrupt_cached_optional_object_is_not_admitted_or_replaced_by_network() {
    let f = Fixture::new();
    let store = f.store();
    for object in store.pending_objects().unwrap() {
        fs::write(
            f.root.join("objects/packs").join(object.file_name()),
            b"corrupt cached bytes",
        )
        .unwrap();
    }
    let files = Files::new(f.source.clone());
    let downloads =
        ProgressiveDownloads::start_inner(&f.root, &"D".repeat(64), store, files.clone()).unwrap();
    until(|| downloads.snapshot().unwrap().last_error.is_some());
    assert_eq!(downloads.snapshot().unwrap().available_packs, 1);
    assert_eq!(files.calls.load(Ordering::Relaxed), 0);
    assert!(matches!(
        downloads.read("urgent.png").unwrap(),
        ScopedRead::Pending(_)
    ));
    stop(downloads);
}

#[test]
fn untrusted_resume_record_cannot_supply_unsigned_objects_or_different_environment() {
    let f = Fixture::new();
    let store = f.store();
    let record = f
        .root
        .join(format!("state/progressive/{}.json", "E".repeat(64)));
    let mut value = ResumeRecord {
        schema: RECORD_SCHEMA.into(),
        manifest_sha256: "E".repeat(64),
        catalog_sha256: store.catalog_sha256().into(),
        scope_sha256: store.scope_sha256().into(),
        foreground: BTreeSet::from(["F".repeat(64)]),
    };
    safe::atomic_write(&record, &serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(ProgressiveDownloads::start_inner(
        &f.root,
        &"E".repeat(64),
        store.clone(),
        Files::new(f.source.clone())
    )
    .is_err());
    value.foreground.clear();
    value.scope_sha256 = "0".repeat(64);
    safe::atomic_write(&record, &serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(ProgressiveDownloads::start_inner(
        &f.root,
        &"E".repeat(64),
        store,
        Files::new(f.source.clone())
    )
    .is_err());
}

#[test]
fn async_waiter_is_woken_on_admission_and_ready_reads_do_not_accumulate_waiters() {
    struct Signal(AtomicUsize);
    impl std::task::Wake for Signal {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let f = Fixture::new();
    let files = Files::new(f.source.clone());
    files.release.store(false, Ordering::Relaxed);
    let downloads =
        ProgressiveDownloads::start_inner(&f.root, &"F".repeat(64), f.store(), files.clone())
            .unwrap();
    until(|| files.entered.load(Ordering::Acquire));
    let signal = Arc::new(Signal(AtomicUsize::new(0)));
    let waker = Waker::from(signal.clone());
    let mut cx = TaskContext::from_waker(&waker);
    assert!(downloads.poll_read("urgent.png", &mut cx).is_pending());
    files.release.store(true, Ordering::Release);
    until(|| downloads.snapshot().unwrap().complete);
    assert!(signal.0.load(Ordering::Relaxed) > 0);
    assert!(matches!(
        downloads.poll_read("urgent.png", &mut cx),
        Poll::Ready(Ok(_))
    ));
    assert!(downloads.shared.queue.lock().unwrap().wakers.is_empty());
    stop(downloads);
}

#[test]
fn preemption_counts_body_bytes_already_received_before_cancelling_the_write() {
    struct Body;
    impl Source for Body {
        fn get(&self, _: &str, _: u64, _: &mut dyn FnMut(u64) -> Result<()>) -> Result<Vec<u8>> {
            unreachable!()
        }
        fn download(
            &self,
            _: &str,
            _: &model::FileEntry,
            _: &Path,
            progress: &mut dyn FnMut(u64) -> Result<()>,
        ) -> Result<()> {
            progress(7)
        }
        fn download_resuming(
            &self,
            _: &str,
            _: &model::FileEntry,
            _: &Path,
            progress: &mut dyn FnMut(DownloadProgress) -> Result<()>,
        ) -> Result<u64> {
            progress(DownloadProgress {
                received: 7,
                reused: 0,
            })?;
            Ok(0)
        }
    }
    let shared = Shared {
        queue: Mutex::new(Queue {
            foreground: BTreeSet::from(["other".into()]),
            ..Queue::default()
        }),
        changed: Condvar::new(),
        stop: AtomicBool::new(false),
        wire: AtomicU64::new(0),
    };
    let source = PrioritySource {
        source: &Body,
        shared: &shared,
        sha: "background",
        foreground: false,
    };
    let entry = model::FileEntry {
        path: "objects/packs/test.m2p".into(),
        size: 7,
        sha256: "A".repeat(64),
    };
    let mut counted = 0;
    let error = source
        .download_resuming(
            "objects/packs/test.m2p",
            &entry,
            Path::new("unused"),
            &mut |p| {
                counted += p.received;
                Ok(())
            },
        )
        .unwrap_err();
    assert!(error.is::<Preempted>());
    assert_eq!(counted, 7);
    let mut counted = 0;
    assert!(source
        .download(
            "objects/packs/test.m2p",
            &entry,
            Path::new("unused"),
            &mut |n| {
                counted += n;
                Ok(())
            }
        )
        .unwrap_err()
        .is::<Preempted>());
    assert_eq!(counted, 7);
}

#[test]
fn another_pack_success_cannot_hide_an_exhausted_failure_and_explicit_retry_recovers() {
    let f = Fixture::new();
    let files = Files::new(f.source.clone());
    files.release.store(false, Ordering::Relaxed);
    let downloads =
        ProgressiveDownloads::start_inner(&f.root, &"1".repeat(64), f.store(), files.clone())
            .unwrap();
    until(|| files.entered.load(Ordering::Acquire));
    let pending = downloads.store.pending_objects().unwrap();
    let first = pending.iter().min_by_key(|p| p.size).unwrap();
    let failed = pending.iter().find(|p| p.sha256 != first.sha256).unwrap();
    {
        let mut queue = downloads.shared.queue.lock().unwrap();
        queue.attempts.insert(failed.sha256.clone(), MAX_ATTEMPTS);
        queue
            .failures
            .insert(failed.sha256.clone(), "unrecovered other pack".into());
    }
    files.release.store(true, Ordering::Release);
    until(|| downloads.snapshot().unwrap().available_packs == 2);
    assert_eq!(
        downloads.snapshot().unwrap().last_error.as_deref(),
        Some("unrecovered other pack")
    );
    assert!(!downloads.snapshot().unwrap().complete);
    downloads.retry().unwrap();
    until(|| downloads.snapshot().unwrap().complete);
    assert!(downloads.snapshot().unwrap().last_error.is_none());
    stop(downloads);
}

#[test]
fn waiter_callback_may_reenter_the_queue_after_worker_unlocks() {
    struct Reenter {
        shared: Arc<Shared>,
        observed: Arc<AtomicUsize>,
    }
    impl std::task::Wake for Reenter {
        fn wake(self: Arc<Self>) {
            self.observed.store(
                if self.shared.queue.try_lock().is_ok() {
                    1
                } else {
                    2
                },
                Ordering::Release,
            );
        }
    }
    let f = Fixture::new();
    let files = Files::new(f.source.clone());
    files.release.store(false, Ordering::Relaxed);
    let downloads =
        ProgressiveDownloads::start_inner(&f.root, &"2".repeat(64), f.store(), files.clone())
            .unwrap();
    until(|| files.entered.load(Ordering::Acquire));
    let observed = Arc::new(AtomicUsize::new(0));
    let waker = Waker::from(Arc::new(Reenter {
        shared: downloads.shared.clone(),
        observed: observed.clone(),
    }));
    assert!(downloads
        .poll_read("urgent.png", &mut TaskContext::from_waker(&waker))
        .is_pending());
    files.release.store(true, Ordering::Release);
    until(|| downloads.snapshot().unwrap().complete);
    assert_eq!(observed.load(Ordering::Acquire), 1);
    stop(downloads);
}

#[test]
fn resume_journal_io_failure_keeps_worker_alive_and_recovers_after_repair() {
    let f = Fixture::new();
    let files = Files::new(f.source.clone());
    files.release.store(false, Ordering::Relaxed);
    let downloads =
        ProgressiveDownloads::start_inner(&f.root, &"3".repeat(64), f.store(), files.clone())
            .unwrap();
    until(|| files.entered.load(Ordering::Acquire));
    let blocked = f
        .root
        .join(format!("state/progressive/{}.tmp", "3".repeat(64)));
    // A directory where a journal scratch file belongs produces a real I/O
    // failure without filling the user's disk or modifying existing files.
    fs::create_dir_all(&blocked).unwrap();
    assert!(matches!(
        downloads.read("urgent.png"),
        Ok(ScopedRead::Pending(_))
    ));
    files.release.store(true, Ordering::Release);
    until(|| {
        downloads
            .snapshot()
            .unwrap()
            .last_error
            .as_deref()
            .is_some_and(|e| e.starts_with("Resource resume record unavailable:"))
    });
    fs::remove_dir(&blocked).unwrap();
    downloads.retry().unwrap();
    until(|| downloads.snapshot().unwrap().complete);
    assert!(downloads.snapshot().unwrap().last_error.is_none());
    stop(downloads);
}
