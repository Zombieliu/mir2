//! Source publication fault classification. The writer callback is simulated;
//! real PostgreSQL atomicity is verified separately in gateway PG tests.
use super::*;

#[test]
fn mail_transfer_unknown_source_writer_freezes_unfenced_clones_without_retry() {
    let mut config = SimulationConfig::default();
    config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
    config.account_store_database_url =
        Some("postgresql://fixture@127.0.0.1:55491/mir2_zone_lease_test_unopened".into());
    let clone = config.clone();
    let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    let error = config
        .save_account_store_with_plan_writer(Some("demo"), |_, mode, plan| {
            assert_eq!(mode, AccountStoreDatabaseMode::SourceOfTruth);
            assert!(plan.zone_save_fence.is_none());
            Err("MAIL_ITEM_TRANSFER_COMMIT_OUTCOME_UNKNOWN: simulated writer response loss".into())
        })
        .unwrap_err();
    assert!(error.starts_with("MAIL_ITEM_TRANSFER_COMMIT_OUTCOME_UNKNOWN"));
    assert!(clone.ensure_account_store_writable().is_err());
    let ran = std::sync::atomic::AtomicBool::new(false);
    assert!(clone
        .commit_account_store_transaction(&["demo".into()], |_| {
            ran.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .is_err());
    assert!(!ran.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
        before
    );
}
