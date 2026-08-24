use mir2_simulation::{
    FileUserItemUidAuthority, UserItemUidAllocator, UserItemUidError, UserItemUidReason,
    UserItemUidStore, USER_ITEM_UID_MAX,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn wait_bounded(&mut self, timeout: Duration) -> Result<ExitStatus, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let poll = self
                .child
                .as_mut()
                .expect("child guard is armed")
                .try_wait();
            match poll {
                Ok(Some(_)) => {
                    let status = self
                        .child
                        .as_mut()
                        .expect("child guard is armed")
                        .wait()
                        .map_err(|error| {
                            format!("failed to wait for completed UID child process: {error}")
                        })?;
                    let _ = self.child.take();
                    return Ok(status);
                }
                Err(error) => {
                    return Err(format!("failed to poll UID child process: {error}"));
                }
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => {
                    return Err("UID child process exceeded the bounded timeout".to_owned());
                }
            }
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mir2-user-item-uid-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated UID test directory");
        Self { path }
    }

    fn state_path(&self) -> PathBuf {
        self.path.join("user-item-uid.v1.json")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn issue_reason() -> UserItemUidReason {
    UserItemUidReason::MonsterDrop
}

fn overwrite_state(path: &Path, contents: &str) {
    fs::write(path, contents).expect("overwrite test UID sidecar");
}

#[test]
fn first_uid_is_one_and_restart_is_monotonic() {
    let directory = TestDirectory::new("restart");
    let state_path = directory.state_path();
    let allocator = UserItemUidAllocator::initialize_file(&state_path, 0).expect("initialize");

    assert_eq!(allocator.issued_through().expect("initial state"), 0);
    assert_eq!(allocator.issue(issue_reason()).expect("first UID").get(), 1);
    assert_eq!(
        allocator.issue(issue_reason()).expect("second UID").get(),
        2
    );
    drop(allocator);

    let reopened = UserItemUidAllocator::open_file(&state_path).expect("reopen");
    assert_eq!(reopened.issued_through().expect("reopened state"), 2);
    assert_eq!(reopened.issue(issue_reason()).expect("third UID").get(), 3);
}

#[test]
fn nonzero_initial_floor_issues_floor_plus_one() {
    let directory = TestDirectory::new("nonzero-floor");
    let state_path = directory.state_path();
    let allocator = UserItemUidAllocator::initialize_file(&state_path, 41).expect("initialize");

    assert_eq!(allocator.issued_through().expect("initial floor"), 41);
    assert_eq!(
        allocator
            .issue(UserItemUidReason::CharacterStartItem)
            .expect("issue after nonzero floor")
            .get(),
        42
    );
}

#[test]
fn raise_floor_is_monotonic_durable_and_restart_safe() {
    let directory = TestDirectory::new("raise-floor");
    let state_path = directory.state_path();
    let allocator = UserItemUidAllocator::initialize_file(&state_path, 40).expect("initialize");

    assert_eq!(
        allocator
            .ensure_issued_through_at_least(12)
            .expect("lower floor is a no-op"),
        40
    );
    let unchanged: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).expect("read unchanged state"))
            .expect("parse unchanged state");
    assert_eq!(unchanged["generation"].as_u64(), Some(1));

    assert_eq!(
        allocator
            .ensure_issued_through_at_least(75)
            .expect("raise floor"),
        75
    );
    let raised: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).expect("read raised state"))
            .expect("parse raised state");
    assert_eq!(raised["issuedThrough"].as_str(), Some("75"));
    assert_eq!(raised["generation"].as_u64(), Some(2));
    drop(allocator);

    let reopened = UserItemUidAllocator::open_file(&state_path).expect("reopen");
    assert_eq!(reopened.issued_through().expect("durable raised floor"), 75);
    assert_eq!(
        reopened
            .issue(issue_reason())
            .expect("post-restart UID")
            .get(),
        76
    );
}

#[test]
fn initialize_new_refuses_existing_state() {
    let directory = TestDirectory::new("existing");
    let state_path = directory.state_path();
    UserItemUidAllocator::initialize_file(&state_path, 0).expect("initialize");

    assert!(matches!(
        UserItemUidAllocator::initialize_file(&state_path, 0),
        Err(UserItemUidError::AlreadyInitialized { .. })
    ));
}

#[test]
fn open_existing_refuses_missing_state() {
    let directory = TestDirectory::new("missing");
    let state_path = directory.state_path();

    assert!(matches!(
        UserItemUidAllocator::open_file(&state_path),
        Err(UserItemUidError::MissingState { .. })
    ));
}

#[test]
fn parent_directory_components_are_rejected() {
    let directory = TestDirectory::new("parent-component");
    let state_path = directory
        .path
        .join("not-created")
        .join("..")
        .join("user-item-uid.v1.json");

    assert!(matches!(
        UserItemUidAllocator::initialize_file(&state_path, 0),
        Err(UserItemUidError::InvalidPath { .. })
    ));
}

#[test]
fn missing_state_after_initialization_fails_closed() {
    let directory = TestDirectory::new("removed");
    let state_path = directory.state_path();
    let allocator = UserItemUidAllocator::initialize_file(&state_path, 0).expect("initialize");
    drop(allocator);
    fs::remove_file(&state_path).expect("remove exact test sidecar");

    assert!(matches!(
        UserItemUidAllocator::open_file(&state_path),
        Err(UserItemUidError::MissingState { .. })
    ));
}

#[test]
fn corrupt_truncated_and_unknown_state_fail_closed() {
    let cases = [
        ("corrupt", "not-json"),
        ("truncated", "{\"schemaVersion\":1,\"issuedThrough\":\"7\""),
        (
            "unknown",
            "{\"schemaVersion\":1,\"issuedThrough\":\"7\",\"generation\":8,\"extra\":true}",
        ),
        (
            "noncanonical",
            "{\"schemaVersion\":1,\"issuedThrough\":\"007\",\"generation\":8}",
        ),
        (
            "negative",
            "{\"schemaVersion\":1,\"issuedThrough\":\"-1\",\"generation\":8}",
        ),
        (
            "overflow",
            "{\"schemaVersion\":1,\"issuedThrough\":\"18446744073709551616\",\"generation\":8}",
        ),
    ];

    for (label, contents) in cases {
        let directory = TestDirectory::new(label);
        let state_path = directory.state_path();
        let allocator = UserItemUidAllocator::initialize_file(&state_path, 0).expect("initialize");
        drop(allocator);
        overwrite_state(&state_path, contents);

        assert!(matches!(
            UserItemUidAllocator::open_file(&state_path),
            Err(UserItemUidError::CorruptState { .. })
        ));
    }
}

#[cfg(any(unix, windows))]
#[test]
fn symbolic_link_state_is_never_followed() {
    let directory = TestDirectory::new("symlink");
    let state_path = directory.state_path();
    let allocator = UserItemUidAllocator::initialize_file(&state_path, 0).expect("initialize");
    drop(allocator);
    let backing_path = directory.path.join("backing-state.json");
    fs::rename(&state_path, &backing_path).expect("move real sidecar");

    #[cfg(unix)]
    std::os::unix::fs::symlink(&backing_path, &state_path).expect("create test symlink");

    #[cfg(windows)]
    if let Err(error) = std::os::windows::fs::symlink_file(&backing_path, &state_path) {
        if error.kind() == std::io::ErrorKind::PermissionDenied
            || error.raw_os_error() == Some(1314)
        {
            eprintln!(
                "SKIP symbolic_link_state_is_never_followed: Windows symlink privilege unavailable: {error}"
            );
            return;
        }
        panic!("create test symlink: {error}");
    }

    assert!(matches!(
        UserItemUidAllocator::open_file(&state_path),
        Err(UserItemUidError::UnsafePath { .. })
    ));
}

#[cfg(windows)]
#[test]
fn windows_rejects_unc_verbatim_unc_and_device_paths_before_io() {
    let unsafe_paths = [
        r"\\server\share\user-item-uid.v1.json",
        r"\\?\UNC\server\share\user-item-uid.v1.json",
        r"\\.\C:\user-item-uid.v1.json",
        r"\\?\GLOBALROOT\Device\HarddiskVolume1\user-item-uid.v1.json",
    ];
    for path in unsafe_paths {
        assert!(
            matches!(
                UserItemUidAllocator::initialize_file(path, 0),
                Err(UserItemUidError::InvalidPath { .. })
            ),
            "unsafe Windows path was not rejected: {path}"
        );
    }
}

#[test]
fn exhaustion_never_wraps_or_reuses_zero() {
    let directory = TestDirectory::new("exhaustion");
    let state_path = directory.state_path();
    let allocator = UserItemUidAllocator::initialize_file(&state_path, 0).expect("initialize");
    drop(allocator);
    overwrite_state(
        &state_path,
        &format!(
            "{{\"schemaVersion\":1,\"issuedThrough\":\"{USER_ITEM_UID_MAX}\",\"generation\":17}}"
        ),
    );

    let reopened = UserItemUidAllocator::open_file(&state_path).expect("reopen max state");
    assert!(matches!(
        reopened.issue(issue_reason()),
        Err(UserItemUidError::Exhausted {
            issued_through: USER_ITEM_UID_MAX,
            generation: 17
        })
    ));
    assert_eq!(
        reopened.issued_through().expect("max remains committed"),
        USER_ITEM_UID_MAX
    );
}

#[test]
fn floor_raise_refuses_generation_overflow_without_mutation() {
    let directory = TestDirectory::new("generation-overflow");
    let state_path = directory.state_path();
    let allocator = UserItemUidAllocator::initialize_file(&state_path, 5).expect("initialize");
    drop(allocator);
    overwrite_state(
        &state_path,
        "{\"schemaVersion\":1,\"issuedThrough\":\"5\",\"generation\":18446744073709551615}",
    );

    let reopened = UserItemUidAllocator::open_file(&state_path).expect("reopen");
    assert!(matches!(
        reopened.ensure_issued_through_at_least(6),
        Err(UserItemUidError::Exhausted {
            issued_through: 5,
            generation: u64::MAX
        })
    ));
    assert_eq!(reopened.issued_through().expect("floor unchanged"), 5);
}

#[test]
fn replacing_authority_directory_is_detected_fail_closed() {
    let directory = TestDirectory::new("directory-replacement");
    let live_directory = directory.path.join("authority");
    fs::create_dir(&live_directory).expect("create authority directory");
    let state_path = live_directory.join("user-item-uid.v1.json");
    let authority =
        FileUserItemUidAuthority::initialize_new(&state_path, 0).expect("initialize authority");
    let moved_directory = directory.path.join("authority-original");
    fs::rename(&live_directory, &moved_directory).expect("move original authority directory");
    fs::create_dir(&live_directory).expect("create replacement authority directory");
    fs::copy(
        moved_directory.join("user-item-uid.v1.json"),
        live_directory.join("user-item-uid.v1.json"),
    )
    .expect("copy replacement state");
    fs::copy(
        moved_directory.join("user-item-uid.v1.lock"),
        live_directory.join("user-item-uid.v1.lock"),
    )
    .expect("copy replacement lock");

    assert!(matches!(
        authority.issue_one(issue_reason()),
        Err(UserItemUidError::AuthorityIdentityChanged { .. })
    ));
}

#[test]
fn replacing_lock_file_is_detected_fail_closed() {
    let directory = TestDirectory::new("lock-replacement");
    let state_path = directory.state_path();
    let authority =
        FileUserItemUidAuthority::initialize_new(&state_path, 0).expect("initialize authority");
    let lock_path = authority.lock_path().to_path_buf();
    let original_lock = directory.path.join("original.lock");
    fs::rename(&lock_path, &original_lock).expect("move original lock");
    fs::write(&lock_path, b"").expect("create replacement lock");

    assert!(matches!(
        authority.issue_one(issue_reason()),
        Err(UserItemUidError::AuthorityIdentityChanged { .. })
    ));
}

#[cfg(windows)]
#[test]
fn windows_file_id_info_detects_recreated_lock_identity() {
    let directory = TestDirectory::new("windows-file-id-info-lock-replacement");
    let state_path = directory.state_path();
    let authority =
        FileUserItemUidAuthority::initialize_new(&state_path, 0).expect("initialize authority");
    let lock_path = authority.lock_path().to_path_buf();
    let original_lock = directory.path.join("windows-original.lock");
    fs::rename(&lock_path, &original_lock).expect("move original lock");
    fs::write(&lock_path, b"").expect("create replacement lock");

    // On Windows, this comparison is backed by
    // GetFileInformationByHandleEx(FileIdInfo)'s 128-bit FILE_ID_128 value.
    assert!(matches!(
        authority.issue_one(issue_reason()),
        Err(UserItemUidError::AuthorityIdentityChanged { .. })
    ));
}

#[test]
fn concurrent_threads_issue_one_contiguous_set() {
    const THREADS: usize = 8;
    const PER_THREAD: usize = 64;

    let directory = TestDirectory::new("threads");
    let allocator = Arc::new(
        UserItemUidAllocator::initialize_file(directory.state_path(), 0).expect("initialize"),
    );
    let issued = Arc::new(Mutex::new(Vec::new()));
    let workers: Vec<_> = (0..THREADS)
        .map(|_| {
            let allocator = Arc::clone(&allocator);
            let issued = Arc::clone(&issued);
            thread::spawn(move || {
                let local: Vec<u64> = (0..PER_THREAD)
                    .map(|_| allocator.issue(issue_reason()).expect("issue").get())
                    .collect();
                issued.lock().expect("issued lock").extend(local);
            })
        })
        .collect();

    for worker in workers {
        worker.join().expect("UID worker thread");
    }

    let mut issued = Arc::try_unwrap(issued)
        .expect("all worker references dropped")
        .into_inner()
        .expect("issued values");
    issued.sort_unstable();
    let expected: Vec<u64> = (1..=(THREADS * PER_THREAD) as u64).collect();
    assert_eq!(issued, expected);
    assert_eq!(
        allocator.issued_through().expect("thread high-water mark"),
        expected.len() as u64
    );
}

#[test]
fn concurrent_child_processes_share_the_file_authority() {
    if std::env::var("MIR2_UID_CHILD_MODE").ok().as_deref() == Some("1") {
        run_uid_child_worker();
        return;
    }

    const CHILDREN: usize = 3;
    const PER_CHILD: usize = 32;

    let directory = TestDirectory::new("processes");
    let state_path = directory.state_path();
    UserItemUidAllocator::initialize_file(&state_path, 0).expect("initialize");
    let executable = std::env::current_exe().expect("current integration-test executable");

    let go_path = directory.path.join("go");
    let mut ready_paths = Vec::new();
    let mut children: Vec<(ChildGuard, PathBuf)> = Vec::new();
    for child_index in 0..CHILDREN {
        let ready_path = directory.path.join(format!("child-{child_index}.ready"));
        let output_path = directory.path.join(format!("child-{child_index}.txt"));
        let child_guard = ChildGuard::new(
            Command::new(&executable)
                .arg("--exact")
                .arg("concurrent_child_processes_share_the_file_authority")
                .arg("--nocapture")
                .env("MIR2_UID_CHILD_MODE", "1")
                .env("MIR2_UID_CHILD_STATE_PATH", &state_path)
                .env("MIR2_UID_CHILD_READY_PATH", &ready_path)
                .env("MIR2_UID_CHILD_GO_PATH", &go_path)
                .env("MIR2_UID_CHILD_OUTPUT_PATH", &output_path)
                .env("MIR2_UID_CHILD_ISSUE_COUNT", PER_CHILD.to_string())
                .spawn()
                .expect("spawn UID child process"),
        );
        ready_paths.push(ready_path);
        children.push((child_guard, output_path));
    }

    if !wait_until(Duration::from_secs(10), || {
        ready_paths.iter().all(|path| path.is_file())
    }) {
        panic!("UID child processes did not all reach the ready barrier within 10 seconds");
    }
    fs::write(&go_path, b"go").expect("release UID child barrier");

    let mut issued = Vec::new();
    let mut child_failures = Vec::new();
    for (mut child_guard, output_path) in children {
        let status = match child_guard.wait_bounded(Duration::from_secs(20)) {
            Ok(status) => status,
            Err(error) => {
                child_failures.push(error);
                continue;
            }
        };
        if !status.success() {
            child_failures.push(format!("UID child process failed: {status}"));
            continue;
        }
        let output = fs::read_to_string(output_path).expect("read UID child output");
        issued.extend(
            output
                .lines()
                .map(|line| line.parse::<u64>().expect("child UID is decimal")),
        );
    }
    assert!(
        child_failures.is_empty(),
        "UID child failures: {}",
        child_failures.join("; ")
    );

    issued.sort_unstable();
    let expected: Vec<u64> = (1..=(CHILDREN * PER_CHILD) as u64).collect();
    assert_eq!(issued, expected);
    let reopened = UserItemUidAllocator::open_file(&state_path).expect("reopen");
    assert_eq!(
        reopened.issued_through().expect("process high-water mark"),
        expected.len() as u64
    );
}

fn run_uid_child_worker() {
    let state_path = std::env::var("MIR2_UID_CHILD_STATE_PATH").expect("child state path");
    let ready_path = std::env::var("MIR2_UID_CHILD_READY_PATH").expect("child ready path");
    let go_path = std::env::var("MIR2_UID_CHILD_GO_PATH").expect("child go path");
    let output_path = std::env::var("MIR2_UID_CHILD_OUTPUT_PATH").expect("child output path");
    let count = std::env::var("MIR2_UID_CHILD_ISSUE_COUNT")
        .expect("child issue count")
        .parse::<usize>()
        .expect("decimal child issue count");
    let allocator = UserItemUidAllocator::open_file(state_path).expect("child open authority");
    fs::write(ready_path, b"ready").expect("publish child ready barrier");
    assert!(
        wait_until(Duration::from_secs(10), || Path::new(&go_path).is_file()),
        "child timed out waiting for go barrier"
    );
    let mut output = String::new();
    for _ in 0..count {
        let uid = allocator.issue(issue_reason()).expect("child issue");
        output.push_str(&format!("{}\n", uid.get()));
    }
    fs::write(output_path, output).expect("write child UID output");
}

fn wait_until(timeout: Duration, predicate: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    predicate()
}
