//! OFFLINE, controlled QA bootstrap; never an ordinary-growth acceptance result.
//! Creates only a new owned local store in an externally prepared protected root,
//! using normal account/character packets. Preserves the built-in demo fixture.
//! After normal StartGame, only level, gold and the matching XP baseline are set.
//! No Gateway, socket, database, existing store, item grant or admin command.
//!
//! Windows only. Required environment inputs (no command-line settings):
//! MIR2_LEISURE_QA_ROOT: <this checkout>/mir2-web3/.mir2-player-qa/leisure-owned-<marker>
//! MIR2_LEISURE_QA_MARKER: fresh 32 lowercase hexadecimal characters
//! MIR2_LEISURE_QA_ACK: OFFLINE_CONTROLLED_LEVEL100_GOLD5000000_NOT_ORDINARY_GROWTH
//! MIR2_LEISURE_QA_ACCOUNT: leisureqa100
//! MIR2_LEISURE_QA_CHARACTER: LeisureQA100
//! MIR2_LEISURE_QA_PASSWORD: private random 20..64 printable ASCII characters
//! First run the separately reviewed prepare-controlled-root01.ps1 for the exact
//! compiled source project. Root must verify its ACLs before and after this tool.
//! Rust validates the exact read-only marker, paths and reparse attributes; it
//! does not inspect or claim to verify Windows ACLs itself.
//! Refuses all other MIR2/Crystal overrides and network/backend environment inputs.
//! Failure retains partial owned outputs; there is deliberately no reuse option.

use std::env;

fn main() {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!("Offline Windows QA bootstrap only. See this example's required MIR2_LEISURE_QA_* environment inputs. Fixed controlled level 100 and gold 5000000; no ordinary growth claim. No settings or password are accepted on the command line; no existing output can be reused.");
        return;
    }
    if !args.is_empty() {
        eprintln!("seed_leisure_acceptance: command-line settings and credentials are forbidden");
        std::process::exit(2);
    }
    #[cfg(windows)]
    let result = offline::run();
    #[cfg(not(windows))]
    let result: Result<serde_json::Value, &str> = Err("this bounded bootstrap supports Windows only");
    match result {
        Ok(receipt) => println!("{receipt}"),
        Err(error) => {
            // Errors are static stage labels: no packet/store/credential Debug.
            eprintln!("seed_leisure_acceptance: {error}; partial owned output, if any, is retained");
            std::process::exit(2);
        }
    }
}

#[cfg(windows)]
mod offline {
    use std::{
        collections::BTreeSet,
        env,
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        os::windows::fs::{MetadataExt, OpenOptionsExt},
        path::{Component, Path, PathBuf, Prefix},
    };

    use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
    use mir2_simulation::{AccountRecord, AccountStore, CharacterSaveRecord, SimulationConfig, SimulationSession};
    use serde_json::{json, Value};

    type SeedResult<T> = Result<T, &'static str>;
    const ACCOUNT: &str = "leisureqa100";
    const CHARACTER: &str = "LeisureQA100";
    const ACK: &str = "OFFLINE_CONTROLLED_LEVEL100_GOLD5000000_NOT_ORDINARY_GROWTH";
    const LEVEL: u16 = 100;
    const GOLD: u32 = 5_000_000;
    // Existing Crystal CRYSTAL_EXPERIENCE_LIST[99], runtime/leveling.rs.
    // This fixture does not replace the curve or claim a level-100 runtime cap.
    const MAX_EXPERIENCE: i64 = 5_400_000_000;
    const MARKER_FILE: &str = ".leisure-acceptance-owned.json";
    const MARKER_SCHEMA: &str = "mir2.leisure-owned-qa.v2";
    const PREPARED_BY: &str = "prepare-controlled-root01.ps1";
    const READ_ONLY: u32 = 1;
    const REPARSE_POINT: u32 = 0x400;
    const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const SHARE_READ: u32 = 1;
    const SHARE_WRITE: u32 = 2;

    const INPUTS: [&str; 6] = [
        "MIR2_LEISURE_QA_ROOT", "MIR2_LEISURE_QA_MARKER", "MIR2_LEISURE_QA_ACK",
        "MIR2_LEISURE_QA_ACCOUNT", "MIR2_LEISURE_QA_CHARACTER", "MIR2_LEISURE_QA_PASSWORD",
    ];

    fn forbidden_environment_key(key: &str) -> bool {
        let name = key.to_ascii_uppercase();
        if INPUTS.contains(&name.as_str()) {
            return false;
        }
        name.starts_with("MIR2_") || name.starts_with("CRYSTAL_")
            || name.starts_with("PG") || name.contains("DATABASE")
            || name.contains("POSTGRES") || name.contains("REDIS")
            || name.contains("BACKEND") || name.contains("GATEWAY")
            || name.contains("ENDPOINT") || name.contains("_RPC")
            || name.ends_with("_URL") || name.ends_with("_URI") || name.ends_with("_DSN")
            || name.ends_with("_HOST") || name.ends_with("_PORT") || name.ends_with("_ADDR")
            || name.contains("ALLOW_EXISTING")
            || matches!(name.as_str(), "HTTP_PROXY" | "HTTPS_PROXY" | "ALL_PROXY" | "NO_PROXY" | "RUST_LOG")
    }

    fn offline_environment() -> SeedResult<()> {
        for (key, value) in env::vars_os() {
            if forbidden_environment_key(&key.to_string_lossy())
                || value.to_string_lossy().contains("://")
            {
                return Err("remove network/database/backend/service and unrelated runtime environment inputs");
            }
        }
        Ok(())
    }

    fn required(key: &str) -> SeedResult<String> {
        env::var(key).map_err(|_| "all documented QA environment inputs are required")
    }

    fn valid_marker(marker: &str) -> bool {
        marker.len() == 32 && marker.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }

    fn validate_expected(ack: &str, account: &str, character: &str) -> SeedResult<()> {
        if ack != ACK || account != ACCOUNT || character != CHARACTER {
            return Err("exact controlled-QA acknowledgement and expected fixed identities are required");
        }
        Ok(())
    }

    fn validate_password(password: &str) -> SeedResult<()> {
        if !(20..=64).contains(&password.len())
            || !password.bytes().all(|b| b.is_ascii_graphic())
            || password.bytes().collect::<BTreeSet<_>>().len() < 8
        {
            return Err("private password must have 20..64 printable ASCII characters and eight distinct characters");
        }
        Ok(())
    }

    fn require_absent(path: &Path) -> SeedResult<()> {
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err("refusing existing output, redirect or inaccessible path; use a new owned QA root"),
        }
    }

    fn pin_directory(path: &Path) -> SeedResult<File> {
        let metadata = fs::symlink_metadata(path).map_err(|_| "cannot inspect owned QA ancestor")?;
        if !metadata.is_dir() || metadata.file_attributes() & REPARSE_POINT != 0 {
            return Err("owned QA ancestors must be ordinary directories without reparse points");
        }
        // Read/write sharing permits normal atomic publication of child files.
        // Omitting delete sharing still prevents rename/delete or replacement
        // of the pinned directory while this process prepares its child files.
        let handle = OpenOptions::new().read(true).share_mode(SHARE_READ | SHARE_WRITE)
            .custom_flags(BACKUP_SEMANTICS | OPEN_REPARSE_POINT)
            .open(path).map_err(|_| "cannot pin owned QA directory")?;
        let checked = handle.metadata().map_err(|_| "cannot inspect pinned QA directory")?;
        if !checked.is_dir() || checked.file_attributes() & REPARSE_POINT != 0 {
            return Err("pinned QA directory is redirected");
        }
        Ok(handle)
    }

    fn create_new_file(path: &Path, bytes: &[u8]) -> SeedResult<File> {
        let mut file = OpenOptions::new().read(true).write(true).create_new(true)
            .share_mode(SHARE_READ).custom_flags(OPEN_REPARSE_POINT)
            .open(path).map_err(|_| "cannot exclusively create a new owned QA file")?;
        let metadata = file.metadata().map_err(|_| "cannot inspect new owned QA file")?;
        if !metadata.is_file() || metadata.file_attributes() & REPARSE_POINT != 0 {
            return Err("new owned QA file is not an ordinary file");
        }
        file.write_all(bytes).and_then(|_| file.sync_all())
            .map_err(|_| "cannot durably initialize owned QA file")?;
        Ok(file)
    }

    struct OwnedRoot {
        store: PathBuf,
        receipt: PathBuf,
        _directories: Vec<File>,
        _marker: File,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct PreparedMarker {
        schema: String,
        marker: String,
        acknowledgement: String,
        expected_account: String,
        expected_character: String,
        prepared_by: String,
        acl_prepared_externally: bool,
        controlled_bootstrap: bool,
        no_ordinary_growth_claim: bool,
    }

    fn validate_marker_payload(bytes: &[u8], marker: &str) -> SeedResult<()> {
        if bytes.len() > 4096 { return Err("prepared marker exceeds its strict bound"); }
        let prepared: PreparedMarker = serde_json::from_slice(bytes)
            .map_err(|_| "prepared marker has invalid, duplicate or unexpected fields")?;
        if prepared.schema != MARKER_SCHEMA || prepared.marker != marker
            || prepared.acknowledgement != ACK || prepared.expected_account != ACCOUNT
            || prepared.expected_character != CHARACTER || prepared.prepared_by != PREPARED_BY
            || !prepared.acl_prepared_externally || !prepared.controlled_bootstrap
            || !prepared.no_ordinary_growth_claim
        { return Err("prepared marker does not exactly authorize this controlled bootstrap"); }
        Ok(())
    }

    fn open_prepared_marker(path: &Path, marker: &str) -> SeedResult<File> {
        let mut file = OpenOptions::new().read(true).share_mode(SHARE_READ)
            .custom_flags(OPEN_REPARSE_POINT).open(path)
            .map_err(|_| "externally prepared authorization marker is required")?;
        let metadata = file.metadata().map_err(|_| "cannot inspect prepared marker")?;
        if !metadata.is_file() || metadata.file_attributes() & REPARSE_POINT != 0
            || metadata.file_attributes() & READ_ONLY == 0
        { return Err("prepared marker must be an ordinary read-only file without reparse redirection"); }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file).take(4097).read_to_end(&mut bytes)
            .map_err(|_| "cannot read bounded prepared marker")?;
        validate_marker_payload(&bytes, marker)?;
        Ok(file)
    }

    fn bind_prepared_root(supplied: &Path, marker: &str) -> SeedResult<OwnedRoot> {
        if !valid_marker(marker) || !supplied.is_absolute()
            || supplied.to_string_lossy().split(['/', '\\']).any(|part| matches!(part, "." | ".."))
            || !matches!(supplied.components().next(), Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        {
            return Err("QA root requires a local absolute disk path, no dot components, and a fresh lowercase hex marker");
        }
        // Derive the checkout from the compiled crate location, never a drive,
        // user home, CWD, production path or previously used acceptance store.
        let project = Path::new(env!("CARGO_MANIFEST_DIR")).parent()
            .and_then(Path::parent).ok_or("cannot resolve source project root")?;
        let parent = project.join(".mir2-player-qa");
        let mut ancestors: Vec<_> = parent.ancestors().collect();
        ancestors.reverse();
        let mut directories = Vec::new();
        for ancestor in ancestors {
            directories.push(pin_directory(ancestor)?);
        }
        let expected_directory = format!("leisure-owned-{marker}");
        if supplied.file_name().and_then(|s| s.to_str()) != Some(expected_directory.as_str())
            || supplied.parent().ok_or("QA root has no parent")?.canonicalize()
                .map_err(|_| "QA root parent is unavailable")?
                != parent.canonicalize().map_err(|_| "owned QA parent is unavailable")?
        {
            return Err("QA root must be a fresh leisure-owned-marker direct child of this checkout's .mir2-player-qa");
        }
        // Inspect the caller's lexical ancestors as well; an alternate alias
        // must not redirect into the canonical approved parent.
        for ancestor in supplied.parent().ok_or("QA root has no parent")?.ancestors() {
            directories.push(pin_directory(ancestor)?);
        }
        // ACL preparation belongs to the reviewed external PowerShell step.
        // Rust accepts only its exact marker-only directory, never an existing
        // store or an arbitrary nonempty QA root, and never repairs its ACLs.
        directories.push(pin_directory(supplied)?);
        let entries: Vec<_> = fs::read_dir(supplied).map_err(|_| "prepared root cannot be inspected")?
            .collect::<Result<_, _>>().map_err(|_| "prepared root contains an inaccessible entry")?;
        if entries.len() != 1 || entries[0].file_name() != std::ffi::OsStr::new(MARKER_FILE) {
            return Err("prepared root must contain only its authorization marker; existing outputs cannot be reused");
        }
        let marker_file = open_prepared_marker(&supplied.join(MARKER_FILE), marker)?;
        let store = supplied.join("accounts.json");
        let receipt = supplied.join("public-receipt.json");
        require_absent(&store)?;
        require_absent(&receipt)?;
        Ok(OwnedRoot { store, receipt, _directories: directories, _marker: marker_file })
    }

    fn checkpoint(session: &SimulationSession) -> SeedResult<CharacterSaveRecord> {
        session.active_character_checkpoint().ok_or("complete active checkpoint is required")
    }

    fn same_except(before: &CharacterSaveRecord, after: &CharacterSaveRecord, fields: &[&str]) -> SeedResult<()> {
        let mut before = serde_json::to_value(before).map_err(|_| "checkpoint comparison failed")?;
        let mut after = serde_json::to_value(after).map_err(|_| "checkpoint comparison failed")?;
        for field in fields {
            for value in [&mut before, &mut after] {
                if *field == "character.level" {
                    value["character"].as_object_mut().ok_or("checkpoint character is invalid")?.remove("level");
                } else {
                    value.as_object_mut().ok_or("checkpoint is invalid")?.remove(*field);
                }
            }
        }
        if before != after {
            return Err("checkpoint changed outside the explicitly allowed controlled preconditions or normal reentry bookkeeping");
        }
        Ok(())
    }

    fn controlled_draft(before: &CharacterSaveRecord) -> CharacterSaveRecord {
        let mut draft = before.clone();
        draft.character.level = LEVEL;
        draft.gold = GOLD;
        draft.experience = 0;
        draft.max_experience = MAX_EXPERIENCE;
        draft
    }

    fn verify_controlled(before: &CharacterSaveRecord, after: &CharacterSaveRecord, saved: bool) -> SeedResult<()> {
        // The exported typed constructor uses the same Crystal base-vitals
        // calculation as normal RefreshStats; no private helper is imported.
        let expected = CharacterSaveRecord::new(after.character.clone());
        if after.character.level != LEVEL || after.gold != GOLD || after.experience != 0
            || after.max_experience != MAX_EXPERIENCE || after.max_hp != expected.max_hp || after.max_mp != expected.max_mp
            || (saved && before.revision.checked_add(1) != Some(after.revision))
            || (!saved && before.revision != after.revision)
        {
            return Err("controlled preconditions, derived stat pools or ordinary save revision did not match");
        }
        // Current HP/MP remain unchanged. Their new maxima are recalculated by
        // normal restore/RefreshStats, never assigned or filled by this tool.
        same_except(before, after, &["character.level", "gold", "experience", "max_experience", "max_hp", "max_mp", "revision"])
    }

    fn login_start(config: &SimulationConfig, password: &str, expected_index: i32) -> SeedResult<SimulationSession> {
        let mut session = SimulationSession::new(config.clone());
        let packets = session.try_handle_packet(ClientPacket::Login {
            account_id: ACCOUNT.into(), password: password.into(),
        }).map_err(|_| "normal verification Login failed")?;
        let accepted = packets.iter().any(|packet| matches!(packet,
            ServerPacket::LoginSuccess { characters } if characters.len() == 1
                && characters[0].index == expected_index && characters[0].name == CHARACTER
                && characters[0].class == MirClass::Warrior && characters[0].gender == MirGender::Male));
        if !accepted { return Err("normal verification Login roster did not match the fixed identity"); }
        let packets = session.try_handle_packet(ClientPacket::StartGame { character_index: expected_index })
            .map_err(|_| "normal verification StartGame failed")?;
        if !packets.iter().any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })) {
            return Err("normal verification StartGame was rejected");
        }
        Ok(session)
    }

    pub(super) fn run() -> SeedResult<Value> {
        offline_environment()?;
        validate_expected(&required(INPUTS[2])?, &required(INPUTS[3])?, &required(INPUTS[4])?)?;
        let password = required(INPUTS[5])?;
        validate_password(&password)?;
        let owned = bind_prepared_root(&PathBuf::from(required(INPUTS[0])?), &required(INPUTS[1])?)?;
        // crystal_full in the real Gateway is CrystalWorld plus no authored
        // content profile. Do not call the platinum or starter-slice builders.
        let base = SimulationConfig::default().with_crystal_world_runtime();
        if base.content_profile.is_some() || base.account_store_database_url.is_some()
            || !base.onchain_mine_nodes.is_empty() || base.save_recovery_dir.is_some()
        { return Err("offline Crystal full configuration contains an unexpected external dependency"); }
        let expected_default: AccountRecord = {
            let store = base.account_store.lock().map_err(|_| "fresh memory store is unavailable")?;
            if store.accounts.len() != 1 { return Err("fresh configuration has unexpected account fixtures"); }
            store.accounts.get("demo").cloned().ok_or("built-in default fixture is missing")?
        };
        let default_baseline = serde_json::to_value(&expected_default)
            .map_err(|_| "cannot capture complete built-in fixture baseline")?;
        // File loader always installs its built-in default fixture. Preserve
        // that exact record rather than deleting it or ever logging into it.
        // The protected prepared root still has no account store at this point.
        require_absent(&owned.store)?;
        let config = base.with_account_store_path(&owned.store);
        // Normal NewAccount below owns the actual writable-Source check. No
        // crate-private authority API or bypass is needed in this example.
        if config.account_store_database_url.is_some() { return Err("new File authority has an external backend"); }
        {
            let store = config.account_store.lock().map_err(|_| "new File authority is unavailable")?;
            let fixture = store.accounts.get("demo").ok_or("new File authority lost its built-in fixture")?;
            if store.accounts.len() != 1 || serde_json::to_value(fixture)
                .map_err(|_| "cannot compare bound built-in fixture")? != default_baseline
            { return Err("new File authority differs from the complete built-in fixture baseline"); }
        }
        let mut session = SimulationSession::new(config.clone());
        let packets = session.try_handle_packet(ClientPacket::NewAccount {
            account_id: ACCOUNT.into(), password: password.clone(), birth_date_binary: 0,
            user_name: String::new(), secret_question: String::new(), secret_answer: String::new(), email_address: String::new(),
        }).map_err(|_| "normal NewAccount failed")?;
        if !matches!(packets.as_slice(), [ServerPacket::NewAccount { result: 8 }]) {
            return Err("normal NewAccount was rejected");
        }
        let packets = session.try_handle_packet(ClientPacket::Login {
            account_id: ACCOUNT.into(), password: password.clone(),
        }).map_err(|_| "normal initial Login failed")?;
        if !matches!(packets.as_slice(), [ServerPacket::LoginSuccess { characters }] if characters.is_empty()) {
            return Err("fresh account Login must have an empty character roster");
        }
        let packets = session.try_handle_packet(ClientPacket::NewCharacter {
            name: CHARACTER.into(), gender: MirGender::Male, class: MirClass::Warrior,
        }).map_err(|_| "normal NewCharacter failed")?;
        let index = match packets.as_slice() {
            [ServerPacket::NewCharacterSuccess { char_info }] if char_info.name == CHARACTER
                && char_info.level == 0 && char_info.class == MirClass::Warrior && char_info.gender == MirGender::Male => char_info.index,
            _ => return Err("normal NewCharacter identity or starting level did not match"),
        };
        let packets = session.try_handle_packet(ClientPacket::StartGame { character_index: index })
            .map_err(|_| "normal initial StartGame failed")?;
        if !packets.iter().any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })) {
            return Err("normal initial StartGame was rejected");
        }
        let before = checkpoint(&session)?;
        if before.character.level != 1 || before.gold != 0 || before.experience != 0 {
            return Err("fresh normal character has unexpected progression or funds");
        }
        let draft = controlled_draft(&before);
        same_except(&before, &draft, &["character.level", "gold", "experience", "max_experience"])?;
        session.restore_active_character_checkpoint(&draft).map_err(|_| "normal typed checkpoint validation rejected controlled preconditions")?;
        verify_controlled(&before, &checkpoint(&session)?, false)?;
        session.save_active_character().map_err(|_| "normal CAS save rejected controlled preconditions")?;
        let saved = checkpoint(&session)?;
        verify_controlled(&before, &saved, true)?;
        drop(session);
        let verification = login_start(&config, &password, index)?;
        let reentered = checkpoint(&verification)?;
        if reentered.revision <= saved.revision { return Err("normal reentry did not advance the saved authority"); }
        let mut expected_events = saved.default_npc_events.clone();
        expected_events.next_sequence = expected_events.next_sequence.checked_add(1).ok_or("Login hook sequence exhausted")?;
        expected_events.committed_sequence = expected_events.committed_sequence.checked_add(1).ok_or("Login hook sequence exhausted")?;
        if !saved.default_npc_events.pending.is_empty() || reentered.default_npc_events != expected_events {
            return Err("normal reentry changed more than the single committed Login hook");
        }
        same_except(&saved, &reentered, &["revision", "default_npc_events"])?;
        // Read back only the freshly created owned store, never a supplied old
        // store. Exactly one new QA identity may coexist with the unchanged
        // built-in fixture; all typed Login packets above use ACCOUNT only.
        let durable: AccountStore = serde_json::from_slice(&fs::read(&owned.store).map_err(|_| "owned store readback failed")?)
            .map_err(|_| "owned store readback is invalid")?;
        let account = durable.accounts.get(ACCOUNT).ok_or("owned account is missing")?;
        let default_after = durable.accounts.get("demo").ok_or("built-in fixture was removed")?;
        if durable.accounts.len() != 2 || serde_json::to_value(default_after)
            .map_err(|_| "cannot compare final built-in fixture")? != default_baseline
            || account.gm_level != 0 || account.characters.len() != 1 || account.saves.len() != 1
        {
            return Err("owned store identity cardinality or normal-player privilege changed");
        }
        let persisted = account.saves.get(&index).ok_or("owned character save is missing")?;
        same_except(&reentered, persisted, &[])?;
        if account.characters[0] != persisted.character { return Err("owned roster and saved character differ"); }
        let receipt = json!({
            "controlledBootstrap":true, "noOrdinaryGrowthClaim":true,
            "uniqueNewQaAccount":true, "defaultFixturePreserved":true, "noDemoFallbackLogin":true,
            "level":LEVEL, "gold":GOLD,
            "inventoryStacks":reentered.inventory_items_json.len(),
            "beltStacks":reentered.belt_items_json.len(),
            "equipmentStacks":reentered.equipment_items_json.len(),
            "storageStacks":reentered.storage_items_json.len()
        });
        let receipt_bytes = serde_json::to_vec_pretty(&receipt).map_err(|_| "cannot encode redacted public receipt")?;
        drop(create_new_file(&owned.receipt, &receipt_bytes)?);
        Ok(receipt)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use mir2_simulation::{AccountStoreRepository, FileAccountStoreRepository};

        #[test]
        fn directory_pin_allows_atomic_store_publication_but_blocks_directory_rename() {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let root = env::temp_dir().join(format!(
                "mir2-leisure-pin-{}-{unique}", std::process::id()
            ));
            let replacement = root.with_file_name(format!(
                "mir2-leisure-pin-{}-{unique}-replacement", std::process::id()
            ));
            require_absent(&root).unwrap();
            require_absent(&replacement).unwrap();
            fs::create_dir(&root).expect("fresh nonsecret test directory must be created");
            let primary = root.join("accounts.json");
            let default_character = SimulationConfig::default().default_character;
            let store = AccountStore::new(default_character.clone());
            let repository = FileAccountStoreRepository::new(&primary);

            let legacy_pin = OpenOptions::new().read(true).share_mode(SHARE_READ)
                .custom_flags(BACKUP_SEMANTICS | OPEN_REPARSE_POINT)
                .open(&root).expect("legacy directory pin must open");
            assert!(repository.save(&store).is_err(),
                "read-only sharing must reproduce the atomic-publication failure");
            let missing = fs::symlink_metadata(&primary)
                .expect_err("failed publication must not create the primary");
            assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);
            drop(legacy_pin);

            let current_pin = pin_directory(&root).expect("current directory pin must open");
            repository.save(&store).expect("normal atomic store publication must succeed");
            let readback = repository.load(default_character)
                .expect("published store must pass typed normal repository loading");
            assert_eq!(serde_json::to_value(&readback).unwrap(), serde_json::to_value(&store).unwrap());
            assert!(fs::rename(&root, &replacement).is_err(),
                "a live directory pin must still deny directory rename/replacement");
            assert!(root.is_dir());
            require_absent(&replacement).unwrap();
            drop(current_pin);
            // Retain only this fresh test's nonsecret outputs; never delete or
            // reuse an acceptance root or recursively clean a temporary path.
        }

        #[test]
        fn refuses_network_backend_and_unrelated_runtime_environment_names() {
            for key in ["DATABASE_URL", "PGHOST", "PGPASSWORD", "MIR2_ACCOUNT_STORE_BACKEND", "REDIS_URL", "HTTP_PROXY", "MIR2_GM_ACCOUNTS", "MIR2_WS_LOAD_SEED_ALLOW_EXISTING", "CRYSTAL_CLIENT_ROOT"] {
                assert!(forbidden_environment_key(key));
            }
            for key in INPUTS { assert!(!forbidden_environment_key(key)); }
        }

        #[test]
        fn marker_and_expected_identities_cannot_select_arbitrary_reuse_targets() {
            assert!(valid_marker("0123456789abcdef0123456789abcdef"));
            for marker in ["", "../accounts", "0123456789ABCDEF0123456789ABCDEF", "0123456789abcdef0123456789abcdeg"] { assert!(!valid_marker(marker)); }
            assert!(validate_expected(ACK, ACCOUNT, CHARACTER).is_ok());
            assert!(validate_expected("", ACCOUNT, CHARACTER).is_err());
            assert!(validate_expected(ACK, "other", CHARACTER).is_err());
            assert!(validate_expected(ACK, ACCOUNT, "other").is_err());
        }

        #[test]
        fn private_password_validation_rejects_weak_or_nonprintable_inputs() {
            assert!(validate_password("ExampleSyntheticQaPass42!").is_ok());
            for password in ["", "short", "aaaaaaaaaaaaaaaaaaaa", "ExampleSyntheticQaPass42!\n"] { assert!(validate_password(password).is_err()); }
        }

        #[test]
        fn existing_files_and_directories_are_refused_without_reading_them() {
            let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
            assert!(require_absent(crate_root).is_err());
            assert!(require_absent(&crate_root.join("Cargo.toml")).is_err());
        }

        #[test]
        fn externally_prepared_marker_requires_the_exact_version_and_authorization() {
            let marker = "0123456789abcdef0123456789abcdef";
            let payload = json!({
                "schema":MARKER_SCHEMA, "marker":marker, "acknowledgement":ACK,
                "expectedAccount":ACCOUNT, "expectedCharacter":CHARACTER,
                "preparedBy":PREPARED_BY, "aclPreparedExternally":true,
                "controlledBootstrap":true, "noOrdinaryGrowthClaim":true
            });
            assert!(validate_marker_payload(&serde_json::to_vec(&payload).unwrap(), marker).is_ok());
            let mut changed = payload.clone();
            changed["schema"] = "mir2.leisure-owned-qa.v1".into();
            assert!(validate_marker_payload(&serde_json::to_vec(&changed).unwrap(), marker).is_err());
            let mut changed = payload.clone();
            changed["unexpected"] = true.into();
            assert!(validate_marker_payload(&serde_json::to_vec(&changed).unwrap(), marker).is_err());
            let mut changed = payload;
            changed["aclPreparedExternally"] = false.into();
            assert!(validate_marker_payload(&serde_json::to_vec(&changed).unwrap(), marker).is_err());
            assert!(validate_marker_payload(&vec![b' '; 4097], marker).is_err());
        }

        #[test]
        fn controlled_draft_preserves_items_skills_quests_and_every_unlisted_field() {
            let base = SimulationConfig::default();
            let before = CharacterSaveRecord::new(base.default_character.clone());
            let draft = controlled_draft(&before);
            let allowed = ["character.level", "gold", "experience", "max_experience"];
            assert!(same_except(&before, &draft, &allowed).is_ok());
            assert_eq!(draft.character.level, 100);
            assert_eq!(draft.gold, 5_000_000);
            assert_eq!(draft.max_experience, 5_400_000_000);
            for field in 0..3 {
                let mut changed = draft.clone();
                match field {
                    0 => changed.inventory_items_json.push("unexpected".into()),
                    1 => changed.skill_states_json.push("unexpected".into()),
                    _ => changed.quest_states_json.push("unexpected".into()),
                }
                assert!(same_except(&before, &changed, &allowed).is_err());
            }
        }
    }
}
