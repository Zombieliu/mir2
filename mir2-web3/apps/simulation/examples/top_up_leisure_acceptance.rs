//! Offline, one-shot controlled QA funds addition. Never ordinary income.
//! Only the existing isolated LeisureQA100 store and a pinned prior SHA qualify.
//! Uses the normal typed File repository with exclusive writer/recovery locks.
//! No login, packet, item grant, nutrition change or result selection occurs.
//! Root must independently check the existing owner/SYSTEM ACL before execution.
//! Inputs: MIR2_LEISURE_TOPUP_ROOT, MIR2_LEISURE_TOPUP_ACK,
//! MIR2_LEISURE_TOPUP_EXPECTED_SHA. No command-line arguments are accepted.
use std::env;

fn main() {
    if env::args_os().len() != 1 {
        eprintln!("top_up_leisure_acceptance: arguments are forbidden");
        std::process::exit(2);
    }
    #[cfg(windows)]
    let result = offline::run();
    #[cfg(not(windows))]
    let result: Result<serde_json::Value, &'static str> = Err("Windows-only controlled helper");
    match result {
        Ok(receipt) => println!("{receipt}"),
        Err(stage) => {
            eprintln!(
                "top_up_leisure_acceptance: {stage}; retain all outputs; never retry automatically"
            );
            std::process::exit(2);
        }
    }
}

#[cfg(windows)]
mod offline {
    use mir2_simulation::{
        AccountStore, AccountStoreRepository, FileAccountStoreRepository, SimulationConfig,
    };
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{
        env,
        fs::{self, File, OpenOptions},
        io::Write,
        os::windows::fs::{MetadataExt, OpenOptionsExt},
        path::{Path, PathBuf},
    };

    type Result<T> = std::result::Result<T, &'static str>;
    const ACCOUNT: &str = "leisureqa100";
    const CHARACTER: &str = "LeisureQA100";
    const ADD: u32 = 3_000_000;
    const OLD_SHA: &str = "1baa6b1cd456546e04e2deb43b12bbb27ac346fb4959eaa100bd53e7daf90ced";
    const ACK: &str = "OFFLINE_CONTROLLED_ADD3000000_GOLD_NOT_ORDINARY_INCOME";
    const ROOT_LEAF: &str = "leisure-owned-a1f834ad906c40bc93c1bd0a33d630cc";
    const INPUTS: [&str; 3] = [
        "MIR2_LEISURE_TOPUP_ROOT",
        "MIR2_LEISURE_TOPUP_ACK",
        "MIR2_LEISURE_TOPUP_EXPECTED_SHA",
    ];
    const REPARSE: u32 = 0x400;
    const OPEN_REPARSE: u32 = 0x0020_0000;

    fn sha(bytes: &[u8]) -> String {
        Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
    }
    fn read(path: &Path) -> Result<Vec<u8>> {
        fs::read(path).map_err(|_| "read failed")
    }
    fn plain(path: &Path, directory: bool) -> Result<()> {
        let m = fs::symlink_metadata(path).map_err(|_| "required ordinary path missing")?;
        if m.is_dir() != directory || m.file_attributes() & REPARSE != 0 {
            return Err("redirected or wrong-kind path refused");
        }
        Ok(())
    }
    fn create(path: &Path, bytes: &[u8]) -> Result<()> {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(1)
            .custom_flags(OPEN_REPARSE)
            .open(path)
            .map_err(|_| "write-once output already exists or unavailable")?;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| "write-once output flush failed")
    }
    fn lock(path: &Path) -> Result<File> {
        plain(path, false)?;
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .custom_flags(OPEN_REPARSE)
            .open(path)
            .map_err(|_| "authority is busy; exclusive lock refused")?;
        if f.metadata().map_err(|_| "lock metadata failed")?.len() != 0 {
            return Err("nonempty authority lock refused");
        }
        Ok(f)
    }
    fn absent(path: &Path) -> Result<()> {
        match fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err("pending reconciliation, existing output or inaccessible path refused"),
        }
    }
    fn comparable(store: &AccountStore, index: i32) -> Result<Value> {
        let mut v = serde_json::to_value(store).map_err(|_| "typed store comparison failed")?;
        let save = v
            .get_mut("accounts")
            .and_then(|x| x.get_mut(ACCOUNT))
            .and_then(|x| x.get_mut("saves"))
            .and_then(|x| x.get_mut(index.to_string()))
            .and_then(Value::as_object_mut)
            .ok_or("target checkpoint absent")?;
        save.remove("gold");
        save.remove("revision");
        Ok(v)
    }
    fn draft(before: &AccountStore) -> Result<(AccountStore, i32, u32, u32, u64, u64)> {
        let account = before
            .accounts
            .get(ACCOUNT)
            .ok_or("fixed isolated account absent")?;
        if before.accounts.len() != 2
            || !before.accounts.contains_key("demo")
            || account.gm_level != 0
            || account.characters.len() != 1
            || account.saves.len() != 1
        {
            return Err("fixed ordinary QA identity shape differs");
        }
        let character = &account.characters[0];
        if character.name != CHARACTER || character.level != 100 {
            return Err("fixed controlled character identity differs");
        }
        let index = character.index;
        let save = account
            .saves
            .get(&index)
            .ok_or("fixed character checkpoint absent")?;
        if save.character != *character {
            return Err("roster/checkpoint identity mismatch");
        }
        let old_gold = save.gold;
        let new_gold = old_gold.checked_add(ADD).ok_or("gold addition overflow")?;
        let old_revision = save.revision;
        let new_revision = old_revision.checked_add(1).ok_or("revision exhausted")?;
        let mut after = before.clone();
        let next = after
            .accounts
            .get_mut(ACCOUNT)
            .unwrap()
            .saves
            .get_mut(&index)
            .unwrap();
        next.gold = new_gold;
        next.revision = new_revision;
        if comparable(before, index)? != comparable(&after, index)? {
            return Err("mutation escaped Gold/revision checkpoint fields");
        }
        Ok((after, index, old_gold, new_gold, old_revision, new_revision))
    }
    // The ordinary File writer and recovery locks stay held across this exact
    // expected-byte comparison, typed atomic replacement and readback. A receipt
    // intent is create-new before any write, so unknown outcomes cannot retry.
    fn apply_locked(
        path: &Path,
        expected_sha: &str,
        intent: &Path,
        backup: &Path,
        receipt: &Path,
    ) -> Result<Value> {
        absent(&path.with_file_name("accounts.json.reconciliation.pending"))?;
        let original = read(path)?;
        if sha(&original) != expected_sha {
            return Err("stale expected account-store SHA");
        }
        let repo = FileAccountStoreRepository::new(path);
        let before = repo
            .load(SimulationConfig::default().default_character)
            .map_err(|_| "normal typed repository load rejected")?;
        let raw: Value =
            serde_json::from_slice(&original).map_err(|_| "original typed JSON invalid")?;
        if raw != serde_json::to_value(&before).map_err(|_| "normal load comparison failed")? {
            return Err("normal loader would change original store semantics");
        }
        let (after, index, old_gold, new_gold, old_revision, new_revision) = draft(&before)?;
        let expected =
            serde_json::to_value(&after).map_err(|_| "expected typed store encoding failed")?;
        let invariant_sha = sha(&serde_json::to_vec(&comparable(&before, index)?)
            .map_err(|_| "invariant encoding failed")?);
        create(intent,b"Controlled Gold+3000000 entered. Never retry this operation; inspect public receipt and expected store hash.\n")?;
        create(backup, &original)?;
        if sha(&read(path)?) != expected_sha {
            return Err("account-store changed before CAS publication");
        }
        repo.save(&after)
            .map_err(|_| "normal atomic repository save refused or outcome unknown")?;
        let persisted_bytes = read(path)?;
        let persisted = repo
            .load(SimulationConfig::default().default_character)
            .map_err(|_| "normal durable readback rejected")?;
        if serde_json::to_value(&persisted).map_err(|_| "durable comparison failed")? != expected {
            return Err("durable checkpoint differs from exact Gold/revision successor");
        }
        let result = json!({"schema":"mir2.leisure-controlled-gold-topup.v1","controlledPrecondition":true,"ordinaryIncomeClaimed":false,"addedGold":ADD,"goldBefore":old_gold,"goldAfter":new_gold,"revisionBefore":old_revision,"revisionAfter":new_revision,"storeBeforeSha256":expected_sha,"storeAfterSha256":sha(&persisted_bytes),"storeAfterLength":persisted_bytes.len(),"unchangedStoreExceptGoldRevision":true,"unchangedStoreSemanticSha256":invariant_sha,"itemOrMaterialOrNutritionOrPositionChanged":false,"normalTypedRepositoryLoadSaveReadback":true,"exclusiveWriterAndRecoveryLocksRequired":true,"oneShotIntentPrecedesCommit":true,"noLoginOrBootstrap":true});
        create(
            receipt,
            &serde_json::to_vec_pretty(&result).map_err(|_| "public receipt encoding failed")?,
        )?;
        Ok(result)
    }
    pub fn run() -> Result<Value> {
        for (k, v) in env::vars_os() {
            let key = k.to_string_lossy().to_ascii_uppercase();
            if (key.starts_with("MIR2_") && !INPUTS.contains(&key.as_str()))
                || key.starts_with("CRYSTAL_")
                || key.starts_with("PG")
                || key.contains("DATABASE")
                || key.contains("REDIS")
                || v.to_string_lossy().contains("://")
            {
                return Err("unrelated runtime/network environment refused");
            }
        }
        if env::var(INPUTS[1]).ok().as_deref() != Some(ACK)
            || env::var(INPUTS[2]).ok().as_deref() != Some(OLD_SHA)
        {
            return Err("explicit fixed controlled authorization or old SHA missing");
        }
        let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.mir2-player-qa")
            .join(ROOT_LEAF)
            .canonicalize()
            .map_err(|_| "compiled isolated root missing")?;
        let supplied = PathBuf::from(env::var_os(INPUTS[0]).ok_or("isolated root missing")?);
        if !supplied.is_absolute()
            || supplied
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            || supplied
                .canonicalize()
                .map_err(|_| "isolated root unavailable")?
                != expected
        {
            return Err("only exact compiled isolated root is admitted");
        }
        let mut directory_pins = Vec::new();
        for p in supplied.ancestors() {
            plain(p, true)?;
            directory_pins.push(
                OpenOptions::new()
                    .read(true)
                    .share_mode(3)
                    .custom_flags(0x0200_0000 | OPEN_REPARSE)
                    .open(p)
                    .map_err(|_| "directory pin refused")?,
            );
        }
        let marker = supplied.join(".leisure-acceptance-owned.json");
        plain(&marker, false)?;
        if sha(&read(&marker)?)
            != "53c811a82a8e74658834c2d50976b9695e6e16d625bcbf0dcc6f0156ff496c48"
        {
            return Err("original authorization marker changed");
        }
        let store = supplied.join("accounts.json");
        plain(&store, false)?;
        for p in [
            supplied.join("private-runtime"),
            supplied.join("private-runtime/recovery"),
            supplied.join("private-runtime/recovery/quarantine"),
        ] {
            plain(&p, true)?;
            directory_pins.push(
                OpenOptions::new()
                    .read(true)
                    .share_mode(3)
                    .custom_flags(0x0200_0000 | OPEN_REPARSE)
                    .open(&p)
                    .map_err(|_| "recovery directory pin refused")?,
            );
        }
        let _writer = lock(&supplied.join("accounts.json.writer.lock"))?;
        let _recovery =
            lock(&supplied.join("private-runtime/recovery/.mir2-recovery-exclusive.lock"))?;
        let _resume = lock(&supplied.join(".controlled-resume-exclusive.lock"))?;
        let recovery = supplied.join("private-runtime/recovery");
        plain(&recovery, true)?;
        let names = fs::read_dir(&recovery)
            .map_err(|_| "recovery inspection failed")?
            .map(|x| x.map(|x| x.file_name().to_string_lossy().into_owned()))
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|_| "recovery inspection failed")?;
        if names.iter().any(|n| {
            !matches!(
                n.as_str(),
                ".mir2-directory-durability"
                    | ".mir2-recovery-exclusive.lock"
                    | ".mir2-recovery-owner-v1.json"
                    | "quarantine"
            )
        }) {
            return Err("pending recovery state refused");
        }
        let quarantine = recovery.join("quarantine");
        for entry in fs::read_dir(&quarantine).map_err(|_| "quarantine inspection failed")? {
            let entry = entry.map_err(|_| "quarantine inspection failed")?;
            if entry.file_name() != ".mir2-directory-durability" {
                return Err("quarantined recovery record refused");
            }
            plain(&entry.path(), false)?;
            if sha(&read(&entry.path())?)
                != "49f358ea406bd4a68597120e55264cd2a128cbd8441f790e1813a75535790ac3"
            {
                return Err("quarantine durability sentinel changed");
            }
        }
        apply_locked(
            &store,
            OLD_SHA,
            &supplied.join("topup3000000-attempt01.txt"),
            &supplied.join("topup3000000-before01.private.json"),
            &supplied.join("topup3000000-public-receipt01.json"),
        )
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use mir2_simulation::{AccountRecord, CharacterSaveRecord};
        fn fixture() -> AccountStore {
            let mut character = SimulationConfig::default().default_character;
            character.index = 2;
            character.name = CHARACTER.into();
            character.level = 100;
            let mut store = AccountStore::new(SimulationConfig::default().default_character);
            store.next_character_index = 3;
            let mut account = AccountRecord::new(character.clone());
            let mut save = CharacterSaveRecord::new(character);
            save.gold = 805160;
            save.revision = 17;
            account.saves.insert(2, save);
            store.accounts.insert(ACCOUNT.into(), account);
            store
        }
        #[test]
        fn exact_gold_and_revision_successor_preserves_every_other_serialized_field() {
            let before = fixture();
            let (after, index, g0, g1, r0, r1) = draft(&before).unwrap();
            assert_eq!((g0, g1, r0, r1), (805160, 3805160, 17, 18));
            assert_eq!(
                comparable(&before, index).unwrap(),
                comparable(&after, index).unwrap()
            );
        }
        #[test]
        fn refuses_wrong_identity_privilege_and_overflow() {
            let mut s = fixture();
            s.accounts.get_mut(ACCOUNT).unwrap().gm_level = 1;
            assert!(draft(&s).is_err());
            let mut s = fixture();
            s.accounts
                .get_mut(ACCOUNT)
                .unwrap()
                .saves
                .get_mut(&2)
                .unwrap()
                .gold = u32::MAX;
            assert!(draft(&s).is_err());
            let mut s = fixture();
            s.accounts.get_mut(ACCOUNT).unwrap().characters[0].name = "other".into();
            assert!(draft(&s).is_err());
        }
        #[test]
        fn real_repository_cas_readback_and_write_once_retry() {
            let root = env::temp_dir().join(format!(
                "mir2-controlled-topup-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            let path = root.join("accounts.json");
            let repo = FileAccountStoreRepository::new(&path);
            repo.save(&fixture()).unwrap();
            let expected = sha(&read(&path).unwrap());
            let intent = root.join("attempt");
            let backup = root.join("backup");
            let receipt = root.join("receipt");
            assert!(apply_locked(&path, "bad-sha", &intent, &backup, &receipt).is_err());
            assert!(!intent.exists());
            assert_eq!(sha(&read(&path).unwrap()), expected);
            let r = apply_locked(&path, &expected, &intent, &backup, &receipt).unwrap();
            assert_eq!(r["goldAfter"], 3805160);
            let after = sha(&read(&path).unwrap());
            assert!(apply_locked(&path, &expected, &intent, &backup, &receipt).is_err());
            assert!(apply_locked(&path, &after, &intent, &backup, &receipt).is_err());
            assert_eq!(sha(&read(&path).unwrap()), after);
            assert_eq!(sha(&read(&backup).unwrap()), expected);
        }
        #[test]
        fn existing_exclusive_lock_blocks_second_writer() {
            let path = env::temp_dir().join(format!(
                "mir2-topup-lock-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::write(&path, []).unwrap();
            let _held = lock(&path).unwrap();
            assert!(lock(&path).is_err());
        }
        #[test]
        fn pending_file_authority_refuses_before_attempt_or_mutation() {
            let root = env::temp_dir().join(format!(
                "mir2-topup-pending-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            let path = root.join("accounts.json");
            FileAccountStoreRepository::new(&path)
                .save(&fixture())
                .unwrap();
            let expected = sha(&read(&path).unwrap());
            fs::write(
                root.join("accounts.json.reconciliation.pending"),
                b"pending",
            )
            .unwrap();
            assert!(apply_locked(
                &path,
                &expected,
                &root.join("attempt"),
                &root.join("backup"),
                &root.join("receipt")
            )
            .is_err());
            assert!(!root.join("attempt").exists());
            assert_eq!(sha(&read(&path).unwrap()), expected);
        }
    }
}
