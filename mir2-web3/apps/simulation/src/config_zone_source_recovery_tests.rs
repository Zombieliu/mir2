//! Pure opaque-readset/cache guards. Fixtures deliberately do not claim that a
//! PostgreSQL lease/read or durable recovery marker has been executed.
use super::*;
use mir2_protocol::ClientPacket;

fn config() -> SimulationConfig {
    let mut config = SimulationConfig::default();
    let mut session = crate::SimulationSession::new(config.clone());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    config.account_store_database_url = Some("postgresql://source-user:private-test-secret@unused.invalid/recovery?options=-csearch_path%3Dsource_namespace".into());
    config
        .with_zone_save_fence(
            zone_save_fence::ZoneSaveFence::new("recover-zone".into(), "successor".into(), 9)
                .unwrap(),
        )
        .unwrap()
}
fn canonical_accounts(config: &SimulationConfig) -> BTreeMap<String, AccountRecord> {
    config.account_store.lock().unwrap().accounts.clone()
}

#[test]
fn zone_source_recovery_rejects_unbound_file_mirror_and_replica_sources_before_connect() {
    assert!(SimulationConfig::default()
        .prepare_zone_source_recovery(&[])
        .is_err());
    let config = config();
    let mut mirror = config.clone();
    mirror.account_store_database_mode = AccountStoreDatabaseMode::Mirror;
    assert!(mirror.prepare_zone_source_recovery(&[]).is_err());
    assert!(config
        .fork_for_replica_apply()
        .unwrap()
        .prepare_zone_source_recovery(&[])
        .is_err());
    let mut unbound = SimulationConfig::default();
    unbound.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    unbound.account_store_database_url = config.account_store_database_url.clone();
    assert!(unbound.prepare_zone_source_recovery(&[]).is_err());
}

#[test]
fn zone_source_recovery_opaque_binding_includes_full_dsn_fence_and_original_cache() {
    let config = config();
    let source = config
        .zone_source_recovery_fixture(canonical_accounts(&config), 4)
        .unwrap();
    source.validate_source_binding(&config).unwrap();
    let mut options = config.clone();
    options
        .account_store_database_url
        .as_mut()
        .unwrap()
        .push_str("&application_name=other");
    assert!(source.validate_source_binding(&options).is_err());
    assert!(source
        .validate_target_database(options.account_store_database_url.as_deref().unwrap())
        .is_err());
    let other_owner = config
        .clone()
        .with_zone_save_fence(
            zone_save_fence::ZoneSaveFence::new("recover-zone".into(), "other-owner".into(), 9)
                .unwrap(),
        )
        .unwrap();
    assert!(source.validate_source_binding(&other_owner).is_err());
    let other_epoch = config
        .clone()
        .with_zone_save_fence(
            zone_save_fence::ZoneSaveFence::new("recover-zone".into(), "successor".into(), 10)
                .unwrap(),
        )
        .unwrap();
    assert!(source.validate_source_binding(&other_epoch).is_err());
    assert!(source
        .validate_source_binding(&config.fork_with_isolated_account_store().unwrap())
        .is_err());
    let debug = format!("{source:?}");
    assert!(!debug.contains("private-test-secret"));
    assert!(!debug.contains("postgresql://"));
    let mut rotated = config.clone();
    rotated.account_store_database_url =
        Some("postgresql://rotated-user:new-secret@unused.invalid/recovery".into());
    let rotated_source = rotated
        .zone_source_recovery_fixture(canonical_accounts(&rotated), 4)
        .unwrap();
    assert_eq!(source.source_digest(), rotated_source.source_digest());
    assert!(source.validate_source_binding(&rotated).is_err());
}

#[test]
fn zone_source_recovery_digest_covers_source_versions_and_full_personal_checkpoint() {
    let config = config();
    let accounts = canonical_accounts(&config);
    let a = config
        .zone_source_recovery_fixture(accounts.clone(), 4)
        .unwrap();
    let b = config
        .zone_source_recovery_fixture(accounts.clone(), 5)
        .unwrap();
    assert_ne!(a.source_digest(), b.source_digest());
    let mut changed = accounts;
    changed
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&0)
        .unwrap()
        .gold += 7;
    let c = config.zone_source_recovery_fixture(changed, 4).unwrap();
    assert_ne!(a.source_digest(), c.source_digest());
    assert_eq!(a.account_ids(), &["demo".to_string()]);
    assert_eq!(a.character_keys(), &[("demo".to_string(), 0)]);
}

#[test]
fn zone_source_recovery_cache_install_is_canonical_guarded_and_idempotent() {
    let config = config();
    let before = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    let mut accounts = canonical_accounts(&config);
    let save = accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap();
    save.revision += 1;
    save.gold -= 7;
    let expected = save.clone();
    let source = config.zone_source_recovery_fixture(accounts, 4).unwrap();
    assert_eq!(
        config.account_store.lock().unwrap().accounts["demo"].saves[&0].gold,
        before.gold
    );
    config.install_zone_source_recovery_cache(&source).unwrap();
    source.validate_installed_cache(&config).unwrap();
    config.install_zone_source_recovery_cache(&source).unwrap();
    let store = config.account_store.lock().unwrap();
    assert_eq!(
        to_json(&store.accounts["demo"].saves[&0]).unwrap(),
        to_json(&expected).unwrap()
    );
    assert_eq!(store.source_account_version("demo"), Some(4));
    assert_eq!(store.source_save_version("demo", 0), Some(4));
}

#[test]
fn zone_source_recovery_never_overwrites_a_cache_advanced_after_prepare_or_install() {
    let config = config();
    let source = config
        .zone_source_recovery_fixture(canonical_accounts(&config), 4)
        .unwrap();
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut("demo")
        .unwrap()
        .saves
        .get_mut(&0)
        .unwrap()
        .gold += 11;
    assert!(config.install_zone_source_recovery_cache(&source).is_err());
    let source = config
        .zone_source_recovery_fixture(canonical_accounts(&config), 4)
        .unwrap();
    config.install_zone_source_recovery_cache(&source).unwrap();
    config
        .account_store
        .lock()
        .unwrap()
        .source_account_versions
        .insert("demo".into(), 5);
    assert!(config.install_zone_source_recovery_cache(&source).is_err());
    assert_eq!(
        config
            .account_store
            .lock()
            .unwrap()
            .source_account_version("demo"),
        Some(5)
    );
}

#[test]
fn zone_source_recovery_empty_read_set_still_retains_exact_successor_binding() {
    let config = config();
    let source = config
        .zone_source_recovery_fixture(BTreeMap::new(), 4)
        .unwrap();
    assert!(source.account_ids().is_empty());
    assert!(source.character_keys().is_empty());
    assert_eq!(source.fence().fencing_token(), 9);
    assert_eq!(source.source_digest().len(), 64);
    config.install_zone_source_recovery_cache(&source).unwrap();
    assert!(!config.account_store.lock().unwrap().accounts.is_empty());
}

#[test]
fn zone_source_recovery_cache_install_never_clears_the_shared_unknown_freeze() {
    let config = config();
    let source = config
        .zone_source_recovery_fixture(canonical_accounts(&config), 4)
        .unwrap();
    config.freeze_account_store_writes("original unresolved operation".into());
    config.install_zone_source_recovery_cache(&source).unwrap();
    assert!(config.ensure_account_store_writable().is_err());
    assert!(config.clone().ensure_account_store_writable().is_err());
}

#[test]
fn zone_source_recovery_retirement_requires_loaded_account_and_all_three_absences() {
    let config = config();
    let source = config
        .zone_source_recovery_fixture(canonical_accounts(&config), 4)
        .unwrap();
    assert!(!source.is_retired_character("demo", 0).unwrap());
    assert!(source.is_retired_character("missing-account", 0).is_err());
    assert!(source.is_retired_character("demo", -1).is_err());
    let mut retired = canonical_accounts(&config);
    retired.get_mut("demo").unwrap().characters.clear();
    retired.get_mut("demo").unwrap().saves.clear();
    let source = config.zone_source_recovery_fixture(retired, 5).unwrap();
    assert!(source.is_retired_character("demo", 0).unwrap());
    assert_eq!(source.account_ids(), &["demo".to_string()]);
    assert!(source.character_keys().is_empty());
    let mut inconsistent = canonical_accounts(&config);
    inconsistent.get_mut("demo").unwrap().saves.clear();
    let source = config
        .zone_source_recovery_fixture(inconsistent, 5)
        .unwrap();
    assert!(source.is_retired_character("demo", 0).is_err());
}
