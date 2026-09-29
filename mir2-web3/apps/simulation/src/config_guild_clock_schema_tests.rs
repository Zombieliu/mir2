use super::*;
const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
fn path() -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "mir2-clock-schema-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("accounts.json")
}
pub(in crate::config) fn fixture_guild(store: &AccountStore) -> SharedGuildRecord {
    let character = &store.accounts["demo"].characters[0];
    SharedGuildRecord {
        id: "123456789abcdef0123456789abcdef0".into(),
        name: "SchemaQA".into(),
        revision: 1,
        level: 0,
        experience: 0,
        spare_points: 0,
        gold: 44,
        ranks: vec![SharedGuildRank {
            index: 0,
            name: "Leader".into(),
            options: 255,
        }],
        members: vec![SharedGuildMember { membership_epoch: 0,
            identity: Stage5FriendIdentity {
                account_id: "demo".into(),
                character_index: character.index,
            },
            name: character.name.clone(),
            rank_index: 0,
        }],
        notice: vec![],
        storage: BTreeMap::new(),
        buffs: BTreeMap::new(),
        last_buff_tick_ms: 0,
        experience_receipts: BTreeSet::new(), experience_receipt_payloads: Default::default(),
    }
}
#[test]
fn guild_clock_schema3_guilds_migrate_but_impossible_clock_and_future_schema_freeze() {
    let default = SimulationConfig::default().default_character;
    let mut store = AccountStore::new(default.clone());
    let guild = fixture_guild(&store);
    store.shared_guilds.insert(guild.id.clone(), guild.clone());
    store.schema_version = 3;
    let migrated = store.clone().migrate_to_current_schema();
    assert_eq!(migrated.schema_version, ACCOUNT_STORE_SCHEMA_VERSION);
    assert_eq!(migrated.shared_guilds[&guild.id], guild);
    assert!(migrated.guild_clock.is_none());
    for future in [false, true] {
        let path = path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut invalid = store.clone();
        if future {
            invalid.schema_version = ACCOUNT_STORE_SCHEMA_VERSION + 1;
        } else {
            invalid.guild_clock = Some(
                GuildClockRecord::default()
                    .acquire(A, 1000, 120000)
                    .unwrap(),
            );
        }
        let bytes = serde_json::to_vec(&invalid).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(FileAccountStoreRepository::new(&path)
            .load(default.clone())
            .is_err());
        let config = SimulationConfig::default().with_account_store_path(&path);
        assert!(config.save_account_store().is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        drop(config);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
#[test]
fn guild_clock_full_file_restore_fences_old_owner_and_scoped_writes_cannot_change_clock() {
    let path = path();
    let config = SimulationConfig::default().with_account_store_path(&path);
    let original = GuildClockRecord {
        generation: 40,
        ..GuildClockRecord::default()
            .acquire(A, 1000, 120000)
            .unwrap()
    };
    {
        config.account_store.lock().unwrap().guild_clock = Some(original.clone());
    }
    config.save_account_store().unwrap();
    let mut backup = config.account_store.lock().unwrap().clone();
    backup.guild_clock = Some(GuildClockRecord::default().acquire(B, 500, 120000).unwrap());
    let backup_path = path.parent().unwrap().join("backup.json");
    backup.save_to_path(&backup_path).unwrap();
    config
        .restore_account_store_from_backup(&backup_path)
        .unwrap();
    let restored = config
        .account_store
        .lock()
        .unwrap()
        .guild_clock
        .clone()
        .unwrap();
    assert!(restored.generation > original.generation);
    assert!(restored.owner_token.is_none());
    assert_eq!(restored.minute_anchor_ms, 500);
    assert!(restored
        .advance(&original.lease().unwrap(), 1001, 120000)
        .is_err());
    let before = fs::read(&path).unwrap();
    let error = config
        .commit_account_store_transaction(&["demo".into()], |store| {
            store.guild_clock = Some(original);
            Ok(())
        })
        .unwrap_err();
    assert!(error.contains("guild_clock"));
    assert_eq!(fs::read(&path).unwrap(), before);
    let scoped = config
        .account_store
        .lock()
        .unwrap()
        .scoped_to_accounts(&["demo"]);
    assert_eq!(scoped.guild_clock, Some(restored));
    drop(config);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn guild_clock_delayed_global_loop_charges_one_interval_not_wall_clock_debt() {
    let start = GuildClockRecord::default()
        .acquire(A, 1000, 300000)
        .unwrap();
    let advance = start
        .advance(&start.lease().unwrap(), 181000, 300000)
        .unwrap();
    assert_eq!(advance.admitted_minutes, 1);
    assert_eq!(advance.next.minute_anchor_ms, 181000);
    assert_eq!(
        advance
            .next
            .advance(&advance.next.lease().unwrap(), 181001, 300000)
            .unwrap()
            .admitted_minutes,
        0
    );
    // Source ActivateBuff has no private anchor: a newly activated guild buff
    // participates in the next global Process exactly once, even after a delay.
    let config = SimulationConfig::default();
    let mut guild = fixture_guild(&config.account_store.lock().unwrap());
    let definition = mir2_game_data::crystal_guild_buff_definitions()
        .iter()
        .find(|definition| definition.time_limit > 0)
        .unwrap();
    guild.buffs.insert(
        definition.id,
        SharedGuildBuff {
            id: definition.id,
            active: true,
            remaining_minutes: definition.time_limit,
        },
    );
    crate::runtime::advance_shared_guild_minutes(&mut guild, advance.admitted_minutes);
    assert_eq!(
        guild.buffs[&definition.id].remaining_minutes,
        definition.time_limit - 1
    );
}
#[test]
fn guild_clock_unknown_full_restore_commit_freezes_without_file_write_or_compensation() {
    let path = path();
    let config = SimulationConfig::default().with_account_store_path(&path);
    config.save_account_store().unwrap();
    let second = SimulationConfig::default().with_account_store_path(&path);
    let original = fs::read(&path).unwrap();
    let mut backup = config.account_store.lock().unwrap().clone();
    backup.accounts.get_mut("demo").unwrap().password = "changed-in-backup".into();
    let backup_path = path.parent().unwrap().join("backup.json");
    backup.save_to_path(&backup_path).unwrap();
    let config =
        config.with_account_store_database_url("postgresql://clock-probe@127.0.0.1:1/clock_probe");
    config.inject_account_store_repository_writer_probe(Err(format!(
        "{CLOCK_COMMIT_OUTCOME_UNKNOWN}: response lost after COMMIT"
    )));
    let error = config
        .restore_account_store_from_backup(&backup_path)
        .unwrap_err();
    assert!(error.contains(CLOCK_COMMIT_OUTCOME_UNKNOWN));
    assert_eq!(
        config.account_store_repository_writer_probe_invocations(),
        1
    );
    assert!(config.ensure_account_store_writable().is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(config.save_account_store().is_err());
    assert_eq!(
        config.account_store_repository_writer_probe_invocations(),
        1
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(second.save_account_store().is_err());
    assert!(second
        .commit_account_store_transaction(&["demo".into()], |store| {
            store.accounts.get_mut("demo").unwrap().password = "should-not-persist".into();
            Ok(())
        })
        .is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    drop(second);
    drop(config);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
