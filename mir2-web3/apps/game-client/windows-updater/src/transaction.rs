//! All mutations are journaled BEFORE activation. A power loss cannot launch a
//! mixed package: recovery runs under the installation lock before game launch.
use crate::{
    fs_safe as safe,
    model::{hash, FileEntry, MAX_META},
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
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
pub fn recover(root: &Path) -> Result<bool> {
    if let Some(j) = load(root)? {
        if j.phase == "applying" {
            rollback(root)?;
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn pending_activation(root: &Path) -> Result<bool> {
    Ok(load(root)?.is_some_and(|j| j.phase == "committed"))
}
pub fn rollback(root: &Path) -> Result<()> {
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
    // Check every backup before changing anything; retained journal permits retry.
    for r in &j.records {
        if let Some(old) = &r.backup {
            ensure!(
                safe::matches(&safe::target(&root.join(".update/backup"), &r.path)?, old)?,
                "rollback backup invalid"
            );
        }
    }
    for r in j.records.iter().rev() {
        ensure!(
            !crate::platform::game_is_running(&root.join("game/mir2-platform-windows.exe"))?,
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
    }
    j.phase = "rolled-back".into();
    save(root, &j)?;
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
    apply_steps(root, changes, |_| Ok(()))
}
fn apply_steps(
    root: &Path,
    changes: &[Change],
    mut after: impl FnMut(usize) -> Result<()>,
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
    safe::clear_scratch(root, "backup")?;
    let mut j = Journal {
        schema: 1,
        phase: "applying".into(),
        records: vec![],
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut required = 64 * 1024 * 1024u64;
    let mut largest = 0;
    for change in changes {
        scope(&change.path)?;
        ensure!(
            seen.insert(change.path.to_ascii_lowercase()),
            "duplicate activation entry"
        );
        let target = safe::target(root, &change.path)?;
        let backup = if target.exists() {
            let (size, sha256) = safe::digest_file(&target)?;
            let entry = FileEntry {
                path: change.path.clone(),
                size,
                sha256,
            };
            entry.validate()?;
            required = required
                .checked_add(size)
                .ok_or_else(|| anyhow::anyhow!("backup space overflow"))?;
            largest = largest.max(size);
            Some(entry)
        } else {
            None
        };
        if let Some(new) = &change.new {
            new.validate()?;
            ensure!(new.path == change.path, "activation mismatch");
            largest = largest.max(new.size);
        }
        j.records.push(Record {
            path: change.path.clone(),
            backup,
            new: change.new.clone(),
        });
    }
    ensure!(
        crate::platform::free_bytes(root)? >= required + largest,
        "not enough space for activation and rollback"
    );
    // Copy and flush every backup before the journal grants permission to mutate.
    for r in &j.records {
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
    }
    save(root, &j)?;
    after(0)?;
    for (index, r) in j.records.iter().enumerate() {
        ensure!(
            !crate::platform::game_is_running(&root.join("game/mir2-platform-windows.exe"))?,
            "game started during update"
        );
        let target = safe::target(root, &r.path)?;
        if let Some(new) = &r.new {
            install_staged(
                &safe::target(&root.join(".update/staging"), &r.path)?,
                &target,
                new,
            )?
        } else if target.exists() {
            safe::regular(&target)?;
            fs::remove_file(target)?;
        }
        after(index + 1)?;
    }
    j.phase = "committed".into();
    save(root, &j)?;
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
}
