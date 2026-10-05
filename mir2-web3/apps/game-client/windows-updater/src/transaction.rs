//! All mutations are journaled BEFORE activation. A power loss cannot launch a
//! mixed package: recovery runs under the installation lock before game launch.
use crate::{
    fs_safe as safe,
    model::{hash, FileEntry, MAX_META},
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::Instant};

/// Display-only observer; no fallible/cancellation result can interrupt commit.
pub trait ProgressObserver {
    fn observe(&mut self, phase: &'static str, completed: usize, total: usize, boundary: bool);
}
struct NoProgress;
impl ProgressObserver for NoProgress {
    fn observe(&mut self, _: &'static str, _: usize, _: usize, _: bool) {}
}
struct Phase<'a, O: ProgressObserver> {
    root: &'a Path,
    observer: &'a mut O,
    phase: &'static str,
    total: usize,
    completed: usize,
    started: Instant,
    complete: bool,
}
impl<'a, O: ProgressObserver> Phase<'a, O> {
    fn new(root: &'a Path, observer: &'a mut O, phase: &'static str, total: usize) -> Self {
        let started = Instant::now();
        observer.observe(phase, 0, total, true);
        Self {
            root,
            observer,
            phase,
            total,
            completed: 0,
            started,
            complete: false,
        }
    }
    fn advance(&mut self, completed: usize) {
        self.completed = completed;
        self.observer
            .observe(self.phase, self.completed, self.total, false);
    }
    fn finish(&mut self) {
        self.complete = true;
    }
}
impl<O: ProgressObserver> Drop for Phase<'_, O> {
    fn drop(&mut self) {
        self.observer
            .observe(self.phase, self.completed, self.total, true);
        crate::update::log(
            self.root,
            "local-phase-end",
            serde_json::json!({"phase":self.phase,
            "completed":self.completed,"total":self.total,"elapsedMs":self.started.elapsed().as_millis(),
            "outcome":if self.complete {"complete"} else {"unfinished"}}),
        );
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub path: String,
    pub new: Option<FileEntry>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    path: String,
    backup: Option<FileEntry>,
    new: Option<FileEntry>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema: u32,
    phase: String,
    records: Vec<Record>,
}
fn scope(path: &str) -> Result<()> {
    crate::model::relative(path)?;
    if let Some(game) = path.strip_prefix("game/") {
        crate::model::game_path(game)
    } else {
        ensure!(
            [
                "updater/active.txt",
                ".update/accepted-feed.json",
                ".update/accepted-feed.p7s"
            ]
            .contains(&path),
            "invalid transaction scope"
        );
        Ok(())
    }
}
fn journal_path(root: &Path) -> std::path::PathBuf {
    root.join(".update/transaction.json")
}
fn save(root: &Path, journal: &Journal) -> Result<()> {
    let bytes = serde_json::to_vec(journal)?;
    ensure!(
        bytes.len() as u64 <= MAX_META * 2,
        "recovery journal too large"
    );
    safe::atomic_write(&journal_path(root), &bytes)
}
fn load(root: &Path) -> Result<Option<Journal>> {
    let path = journal_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let j: Journal = serde_json::from_slice(&safe::read_bounded(&path, MAX_META * 2)?)?;
    ensure!(
        j.schema == 1
            && ["applying", "committed", "accepted", "rolled-back"].contains(&j.phase.as_str())
            && j.records.len() <= 200010,
        "invalid recovery journal"
    );
    let mut seen = std::collections::BTreeSet::new();
    for r in &j.records {
        scope(&r.path)?;
        ensure!(
            seen.insert(r.path.to_ascii_lowercase()),
            "duplicate recovery entry"
        );
        for f in [&r.backup, &r.new].into_iter().flatten() {
            f.validate()?;
            ensure!(f.path == r.path, "recovery hash/path mismatch");
        }
    }
    Ok(Some(j))
}
fn install_staged(source: &Path, target: &Path, entry: &FileEntry) -> Result<()> {
    ensure!(
        safe::matches(source, entry)?,
        "staged/backup file hash mismatch"
    );
    safe::ancestors(target)?;
    fs::create_dir_all(target.parent().unwrap())?;
    let temp = target.with_file_name(format!(
        "{}.mir2-update-tmp",
        target.file_name().unwrap().to_string_lossy()
    ));
    safe::ancestors(&temp)?;
    if temp.exists() {
        safe::regular(&temp)?;
        fs::remove_file(&temp)?;
    }
    fs::copy(source, &temp)?;
    fs::OpenOptions::new().write(true).open(&temp)?.sync_all()?;
    ensure!(safe::matches(&temp, entry)?, "replacement hash mismatch");
    crate::platform::replace_file(&temp, target)?;
    Ok(())
}
/// Activation consumes expendable staging, while recovery keeps using the
/// copying path above so its verified backups survive a failed/retried restore.
/// The journal and every old-file backup are durable before this is called.
fn activate_staged(source: &Path, target: &Path, entry: &FileEntry) -> Result<()> {
    ensure!(safe::matches(source, entry)?, "staged file hash mismatch");
    safe::ancestors(target)?;
    fs::create_dir_all(target.parent().unwrap())?;
    // Also covers transaction callers that did not use stage_bytes. Flush the
    // verified source itself; copying it to another dirty file is unnecessary.
    fs::OpenOptions::new()
        .write(true)
        .open(source)?
        .sync_all()?;
    // Windows uses same-volume MOVEFILE_WRITE_THROUGH, without COPY_ALLOWED.
    // A crash rolls back from backups; recovery never needs new staged files.
    crate::platform::replace_file(source, target)?;
    Ok(())
}
fn activate_record(root: &Path, record: &Record) -> Result<()> {
    let target = safe::target(root, &record.path)?;
    if let Some(new) = &record.new {
        activate_staged(
            &safe::target(&root.join(".update/staging"), &record.path)?,
            &target,
            new,
        )?;
    } else if target.exists() {
        safe::regular(&target)?;
        fs::remove_file(target)?;
    }
    Ok(())
}
fn independent_payload(record: &Record) -> bool {
    record.path.strip_prefix("game/").is_some_and(|path| {
        path != "mir2-platform-windows.exe" && !crate::model::META.contains(&path)
    })
}
pub fn recover(root: &Path) -> Result<bool> {
    recover_observed(root, &mut NoProgress)
}
pub fn recover_observed(root: &Path, observer: &mut impl ProgressObserver) -> Result<bool> {
    if let Some(j) = load(root)? {
        if j.phase == "applying" {
            rollback_observed(root, observer)?;
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn pending_activation(root: &Path) -> Result<bool> {
    Ok(load(root)?.is_some_and(|j| j.phase == "committed"))
}
pub fn rollback(root: &Path) -> Result<()> {
    rollback_observed(root, &mut NoProgress)
}
pub fn rollback_observed(root: &Path, observer: &mut impl ProgressObserver) -> Result<()> {
    ensure!(
        !crate::platform::game_is_running(&root.join("game/mir2-platform-windows.exe"))?,
        "close the game before recovery"
    );
    let Some(mut j) = load(root)? else {
        return Ok(());
    };
    ensure!(
        j.phase == "applying" || j.phase == "committed",
        "rollback unavailable"
    );
    let mut phase = Phase::new(root, observer, "recovering", j.records.len());
    // Check every backup before changing anything; retained journal permits retry.
    for r in &j.records {
        if let Some(old) = &r.backup {
            ensure!(
                safe::matches(&safe::target(&root.join(".update/backup"), &r.path)?, old)?,
                "rollback backup invalid"
            );
        }
    }
    let exe = root.join("game/mir2-platform-windows.exe");
    let mut game_guard = crate::platform::GameGuard::new(&exe)?;
    for (index, r) in j.records.iter().rev().enumerate() {
        if r.path == "game/mir2-platform-windows.exe" {
            game_guard.release();
        }
        ensure!(
            !if r.path == "game/mir2-platform-windows.exe" {
                crate::platform::game_is_running(&exe)?
            } else {
                game_guard.is_running()?
            },
            "game started during recovery"
        );
        let target = safe::target(root, &r.path)?;
        if let Some(old) = &r.backup {
            install_staged(
                &safe::target(&root.join(".update/backup"), &r.path)?,
                &target,
                old,
            )?
        } else if target.exists() {
            safe::regular(&target)?;
            fs::remove_file(target)?;
        }
        if r.path == "game/mir2-platform-windows.exe" {
            ensure!(!game_guard.is_running()?, "game started during recovery");
        }
        phase.advance(index + 1);
    }
    j.phase = "rolled-back".into();
    save(root, &j)?;
    phase.finish();
    Ok(())
}
pub fn accept(root: &Path) -> Result<()> {
    if let Some(mut j) = load(root)? {
        if j.phase == "committed" {
            j.phase = "accepted".into();
            save(root, &j)?;
        }
    }
    Ok(())
}
pub fn prepare_and_apply(root: &Path, changes: &[Change]) -> Result<()> {
    prepare_and_apply_observed(root, changes, &mut NoProgress)
}
pub fn prepare_and_apply_observed(
    root: &Path,
    changes: &[Change],
    observer: &mut impl ProgressObserver,
) -> Result<()> {
    apply_steps_observed(root, changes, |_| Ok(()), observer)
}
#[cfg(test)]
fn apply_steps(
    root: &Path,
    changes: &[Change],
    after: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    apply_steps_observed(root, changes, after, &mut NoProgress)
}
pub(crate) fn apply_steps_observed(
    root: &Path,
    changes: &[Change],
    mut after: impl FnMut(usize) -> Result<()>,
    observer: &mut impl ProgressObserver,
) -> Result<()> {
    ensure!(
        !crate::platform::game_is_running(&root.join("game/mir2-platform-windows.exe"))?,
        "game is running"
    );
    if let Some(j) = load(root)? {
        ensure!(
            j.phase != "applying" && j.phase != "committed",
            "recovery/first-launch acceptance required"
        );
    }
    let mut preparing = Phase::new(root, observer, "preparing", changes.len());
    safe::clear_scratch(root, "backup")?;
    let mut j = Journal {
        schema: 1,
        phase: "applying".into(),
        records: vec![],
    };
    let mut seen = std::collections::BTreeSet::new();
    // Reject all scope/alias errors before dispatching any scratch work.
    for change in changes {
        scope(&change.path)?;
        ensure!(
            seen.insert(change.path.to_ascii_lowercase()),
            "duplicate activation entry"
        );
        if let Some(new) = &change.new {
            new.validate()?;
            ensure!(new.path == change.path, "activation mismatch");
        }
    }
    let worker_count = crate::local_io::workers(changes.len());
    j.records = crate::local_io::run(
        changes.iter().map(Ok),
        worker_count,
        |change| {
            let target = safe::target(root, &change.path)?;
            let backup = if target.exists() {
                let (size, sha256) = safe::digest_file(&target)?;
                let entry = FileEntry {
                    path: change.path.clone(),
                    size,
                    sha256,
                };
                entry.validate()?;
                Some(entry)
            } else {
                None
            };
            Ok(Record {
                path: change.path.clone(),
                backup,
                new: change.new.clone(),
            })
        },
        |_, _| {},
    )?;
    let mut required = 64 * 1024 * 1024u64;
    let mut largest = 0;
    for record in &j.records {
        if let Some(old) = &record.backup {
            required = required
                .checked_add(old.size)
                .ok_or_else(|| anyhow::anyhow!("backup space overflow"))?;
            largest = largest.max(old.size);
        }
        if let Some(new) = &record.new {
            largest = largest.max(new.size);
        }
    }
    // Keep metadata/pointers last. For an initial installation, putting the
    // verified executable first permits live exclusion for subsequent assets;
    // otherwise its missing path forces a full process snapshot per asset.
    // The installation lock/journal still quarantine the entire mixed package.
    if let Some(index) = j
        .records
        .iter()
        .position(|r| r.path == "game/mir2-platform-windows.exe" && r.new.is_some())
    {
        let executable = j.records.remove(index);
        j.records.insert(0, executable);
    }
    ensure!(
        crate::platform::free_bytes(root)? >= required + largest,
        "not enough space for activation and rollback"
    );
    // Copy and flush every backup before the journal grants permission to mutate.
    crate::local_io::run(
        j.records.iter().map(Ok),
        worker_count,
        |r| {
            if let Some(old) = &r.backup {
                let dest = safe::target(&root.join(".update/backup"), &r.path)?;
                fs::create_dir_all(dest.parent().unwrap())?;
                fs::copy(safe::target(root, &r.path)?, &dest)?;
                fs::OpenOptions::new().write(true).open(&dest)?.sync_all()?;
                ensure!(
                    safe::matches(&dest, old)?,
                    "backup changed during preparation"
                );
            }
            if let Some(new) = &r.new {
                ensure!(
                    safe::matches(&safe::target(&root.join(".update/staging"), &r.path)?, new)?,
                    "missing staged file"
                );
            }
            Ok(())
        },
        |_, completed| preparing.advance(completed),
    )?;
    save(root, &j)?;
    preparing.finish();
    drop(preparing);
    let mut installing = Phase::new(root, observer, "installing", j.records.len());
    after(0)?;
    let exe = root.join("game/mir2-platform-windows.exe");
    let mut game_guard = crate::platform::GameGuard::new(&exe)?;
    let mut index = 0;
    while index < j.records.len() {
        let r = &j.records[index];
        if independent_payload(r) {
            ensure!(!game_guard.is_running()?, "game started during update");
            let end = index
                + j.records[index..]
                    .iter()
                    .take_while(|r| independent_payload(r))
                    .count();
            if game_guard.excluded() {
                let excluded = &game_guard;
                crate::local_io::try_run(
                    j.records[index..end].iter().map(|r| {
                        excluded.verify_excluded()?;
                        Ok(r)
                    }),
                    crate::local_io::workers(end - index),
                    |r| {
                        // The live exclusion remains held throughout every
                        // queued write. Validate it on the actual worker too.
                        excluded.verify_excluded()?;
                        activate_record(root, r)
                    },
                    |_, completed| {
                        installing.advance(index + completed);
                        after(index + completed)
                    },
                )?;
                index = end;
                continue;
            }
            // Read-only/locked EXEs retain serial fresh process checks. Never
            // queue writes when a live exclusion could not be acquired.
        }
        if r.path == "game/mir2-platform-windows.exe" {
            game_guard.release();
        }
        ensure!(
            !if r.path == "game/mir2-platform-windows.exe" {
                crate::platform::game_is_running(&exe)?
            } else {
                game_guard.is_running()?
            },
            "game started during update"
        );
        activate_record(root, r)?;
        if r.path == "game/mir2-platform-windows.exe" {
            ensure!(!game_guard.is_running()?, "game started during update");
        }
        installing.advance(index + 1);
        after(index + 1)?;
        index += 1;
    }
    j.phase = "committed".into();
    save(root, &j)?;
    installing.finish();
    Ok(())
}
pub fn stage_bytes(root: &Path, path: &str, bytes: &[u8]) -> Result<Change> {
    scope(path)?;
    safe::write_new(&safe::target(&root.join(".update/staging"), path)?, bytes)?;
    Ok(Change {
        path: path.into(),
        new: Some(FileEntry {
            path: path.into(),
            size: bytes.len() as u64,
            sha256: hash(bytes),
        }),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interruption_at_every_commit_step_recovers_before_launch() {
        for stop in 0..=4 {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            safe::write_new(&root.join("game/mir2-platform-windows.exe"), b"old exe").unwrap();
            safe::write_new(&root.join("game/README-START.txt"), b"old readme").unwrap();
            safe::write_new(&root.join("game/logs/user.log"), b"user progress").unwrap();
            let changes = vec![
                stage_bytes(root, "game/mir2-platform-windows.exe", b"new exe").unwrap(),
                stage_bytes(root, "game/README-START.txt", b"new readme").unwrap(),
                stage_bytes(root, "game/CONTROLS.txt", b"new controls").unwrap(),
                stage_bytes(root, "updater/active.txt", &"A".repeat(64).into_bytes()).unwrap(),
            ];
            let result = apply_steps(root, &changes, |step| {
                ensure!(step != stop, "simulated abrupt process loss");
                Ok(())
            });
            assert!(result.is_err());
            assert!(recover(root).unwrap());
            assert_eq!(
                fs::read(root.join("game/mir2-platform-windows.exe")).unwrap(),
                b"old exe"
            );
            assert_eq!(
                fs::read(root.join("game/README-START.txt")).unwrap(),
                b"old readme"
            );
            assert!(!root.join("game/CONTROLS.txt").exists());
            assert!(!root.join("updater/active.txt").exists());
            assert_eq!(
                fs::read(root.join("game/logs/user.log")).unwrap(),
                b"user progress"
            );
            assert!(!recover(root).unwrap());
        }
    }
    #[test]
    fn committed_update_can_roll_back_after_failed_first_launch() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        safe::write_new(&root.join("game/mir2-platform-windows.exe"), b"old").unwrap();
        let changes = vec![stage_bytes(root, "game/mir2-platform-windows.exe", b"new").unwrap()];
        prepare_and_apply(root, &changes).unwrap();
        assert!(!recover(root).unwrap());
        rollback(root).unwrap();
        assert_eq!(
            fs::read(root.join("game/mir2-platform-windows.exe")).unwrap(),
            b"old"
        );
    }
    #[test]
    fn invalid_or_corrupted_recovery_never_touches_external_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let raw=br#"{"schema":1,"phase":"applying","records":[{"path":"../outside","backup":null,"new":null}]}"#;
        safe::write_new(&journal_path(root), raw).unwrap();
        assert!(recover(root).is_err());
    }
    #[test]
    fn bad_staging_is_rejected_before_original_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        safe::write_new(&root.join("game/mir2-platform-windows.exe"), b"old").unwrap();
        let change = stage_bytes(root, "game/mir2-platform-windows.exe", b"new").unwrap();
        fs::write(
            root.join(".update/staging/game/mir2-platform-windows.exe"),
            b"bad",
        )
        .unwrap();
        assert!(prepare_and_apply(root, &[change]).is_err());
        assert_eq!(
            fs::read(root.join("game/mir2-platform-windows.exe")).unwrap(),
            b"old"
        );
    }
    #[test]
    fn consumed_staging_recovers_and_executable_precedes_assets_but_not_metadata() {
        for stop in 1..=3 {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            safe::write_new(&root.join("game/VERSION.json"), b"old metadata").unwrap();
            let changes = vec![
                stage_bytes(root, "game/mir2-assets/a.png", b"new asset").unwrap(),
                stage_bytes(root, "game/mir2-platform-windows.exe", b"new executable").unwrap(),
                stage_bytes(root, "game/VERSION.json", b"new metadata").unwrap(),
            ];
            let result = apply_steps(root, &changes, |step| {
                if step == 1 {
                    assert_eq!(
                        fs::metadata(root.join("game/mir2-platform-windows.exe"))?.len(),
                        b"new executable".len() as u64
                    );
                    assert!(!root
                        .join(".update/staging/game/mir2-platform-windows.exe")
                        .exists());
                    assert!(!root.join("game/mir2-assets/a.png").exists());
                    assert_eq!(fs::read(root.join("game/VERSION.json"))?, b"old metadata");
                }
                ensure!(
                    step != stop,
                    "simulated process loss after consumed staging"
                );
                Ok(())
            });
            assert!(result.is_err());
            assert!(recover(root).unwrap());
            assert!(!root.join("game/mir2-platform-windows.exe").exists());
            assert!(!root.join("game/mir2-assets/a.png").exists());
            assert_eq!(
                fs::read(root.join("game/VERSION.json")).unwrap(),
                b"old metadata"
            );
            assert_eq!(
                fs::read(root.join(".update/backup/game/VERSION.json")).unwrap(),
                b"old metadata"
            );
        }
    }

    #[test]
    fn staged_mutation_after_preparation_rejects_then_restores_consumed_predecessor() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        safe::write_new(
            &root.join("game/mir2-platform-windows.exe"),
            b"old executable",
        )
        .unwrap();
        let changes = vec![
            stage_bytes(root, "game/mir2-platform-windows.exe", b"new executable").unwrap(),
            stage_bytes(root, "game/mir2-assets/a.png", b"expected bytes").unwrap(),
        ];
        let result = apply_steps(root, &changes, |step| {
            if step == 1 {
                fs::write(
                    root.join(".update/staging/game/mir2-assets/a.png"),
                    b"tampered bytes",
                )?;
            }
            Ok(())
        });
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("staged file hash mismatch"));
        assert!(!root.join("game/mir2-assets/a.png").exists());
        assert!(recover(root).unwrap());
        assert_eq!(
            fs::read(root.join("game/mir2-platform-windows.exe")).unwrap(),
            b"old executable"
        );
    }

    #[test]
    fn parallel_partial_activation_joins_before_full_journal_rollback_and_keeps_metadata_last() {
        for tamper in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            safe::write_new(
                &root.join("game/mir2-platform-windows.exe"),
                b"old executable",
            )
            .unwrap();
            safe::write_new(&root.join("game/VERSION.json"), b"old metadata").unwrap();
            safe::write_new(&root.join("game/logs/personal.log"), b"personal witness").unwrap();
            let mut changes = Vec::new();
            for index in 0..64 {
                let path = format!("game/mir2-assets/parallel/{index}.png");
                if index % 2 == 0 {
                    safe::write_new(&root.join(&path), format!("old {index}").as_bytes()).unwrap();
                }
                changes.push(stage_bytes(root, &path, format!("new {index}").as_bytes()).unwrap());
            }
            changes.push(
                stage_bytes(root, "game/mir2-platform-windows.exe", b"new executable").unwrap(),
            );
            changes.push(stage_bytes(root, "game/VERSION.json", b"new metadata").unwrap());
            let result = apply_steps(root, &changes, |step| {
                if step == 1 {
                    assert_eq!(load(root)?.unwrap().records.len(), 66);
                    if tamper {
                        fs::write(
                            root.join(".update/staging/game/mir2-assets/parallel/12.png"),
                            b"bad bytes",
                        )?;
                    }
                }
                ensure!(tamper || step != 12, "interrupted parallel activation");
                Ok(())
            });
            let error = result.unwrap_err().to_string();
            assert!(error.contains(if tamper {
                "staged file hash mismatch"
            } else {
                "interrupted parallel activation"
            }));
            assert_eq!(load(root).unwrap().unwrap().phase, "applying");
            assert_eq!(
                fs::read(root.join("game/VERSION.json")).unwrap(),
                b"old metadata"
            );
            let snapshot = (0..64)
                .map(|index| {
                    fs::read(root.join(format!("game/mir2-assets/parallel/{index}.png"))).ok()
                })
                .collect::<Vec<_>>();
            std::thread::sleep(std::time::Duration::from_millis(20));
            assert_eq!(
                snapshot,
                (0..64)
                    .map(|index| fs::read(
                        root.join(format!("game/mir2-assets/parallel/{index}.png"))
                    )
                    .ok())
                    .collect::<Vec<_>>()
            );
            assert!(snapshot
                .iter()
                .flatten()
                .any(|bytes| bytes.starts_with(b"new ")));
            assert!(recover(root).unwrap());
            for index in 0..64 {
                let path = format!("game/mir2-assets/parallel/{index}.png");
                if index % 2 == 0 {
                    let old = format!("old {index}").into_bytes();
                    assert_eq!(fs::read(root.join(&path)).unwrap(), old);
                    assert_eq!(
                        fs::read(root.join(".update/backup").join(&path)).unwrap(),
                        old
                    );
                } else {
                    assert!(!root.join(&path).exists());
                }
            }
            assert_eq!(
                fs::read(root.join("game/mir2-platform-windows.exe")).unwrap(),
                b"old executable"
            );
            assert_eq!(
                fs::read(root.join("game/VERSION.json")).unwrap(),
                b"old metadata"
            );
            assert_eq!(
                fs::read(root.join("game/logs/personal.log")).unwrap(),
                b"personal witness"
            );
            assert!(!recover(root).unwrap());
        }
    }

    #[cfg(windows)]
    #[test]
    fn live_exclusion_blocks_game_start_and_rename_between_replacements_then_releases() {
        use std::os::windows::process::CommandExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("game")).unwrap();
        fs::copy(
            std::env::current_exe().unwrap(),
            root.join("game/mir2-platform-windows.exe"),
        )
        .unwrap();
        safe::write_new(&root.join("game/mir2-assets/a.png"), b"old asset").unwrap();
        let changes = vec![
            stage_bytes(root, "game/mir2-assets/a.png", b"new asset").unwrap(),
            stage_bytes(root, "game/mir2-assets/b.png", b"second asset").unwrap(),
        ];
        let mut blocked_start = false;
        let result = apply_steps(root, &changes, |step| {
            if step == 1 {
                let child = std::process::Command::new(root.join("game/mir2-platform-windows.exe"))
                    .args(["--exact", "platform::tests::fresh_probe_detects_a_later_image_mapping_and_keeps_locked_missing_readonly_fallbacks", "--test-threads=1"])
                    .env("MIR2_UPDATER_OWNED_PROCESS_PROBE", root)
                    .creation_flags(0x08000000)
                    .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
                    .spawn();
                blocked_start = child
                    .as_ref()
                    .err()
                    .is_some_and(|e| e.raw_os_error() == Some(32));
                if let Ok(mut child) = child {
                    fs::write(root.join("stop"), b"normal owned unexpected child exit")?;
                    child.wait()?;
                }
                ensure!(blocked_start, "owned image startup was not excluded");
                ensure!(
                    fs::rename(
                        root.join("game/mir2-platform-windows.exe"),
                        root.join("renamed.exe")
                    )
                    .is_err(),
                    "live guard allowed executable rename"
                );
            }
            Ok(())
        });
        result.unwrap();
        assert!(blocked_start);
        assert!(root.join("game/mir2-assets/b.png").exists());
        assert_eq!(
            fs::read(root.join("game/mir2-assets/a.png")).unwrap(),
            b"new asset"
        );
        rollback(root).unwrap();
        assert_eq!(
            fs::read(root.join("game/mir2-assets/a.png")).unwrap(),
            b"old asset"
        );
        assert!(!root.join("game/mir2-assets/b.png").exists());
        assert!(fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join("game/mir2-platform-windows.exe"))
            .is_ok());
    }
}
