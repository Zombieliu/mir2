use super::{
    AccountStore, AccountStoreTransactionFault, SimulationConfig,
    ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "mir2-account-store-durability-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("durability test directory should be created");
    path
}

fn live_demo_password(config: &SimulationConfig) -> String {
    config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned")
        .accounts
        .get("demo")
        .expect("default demo account should exist")
        .password
        .clone()
}

fn disk_demo_password(path: &Path) -> String {
    let bytes = fs::read(path).expect("account store snapshot should exist");
    serde_json::from_slice::<AccountStore>(&bytes)
        .expect("account store snapshot should decode")
        .accounts
        .get("demo")
        .expect("default demo account should exist on disk")
        .password
        .clone()
}

fn set_demo_password(store: &mut AccountStore, password: &str) -> Result<(), String> {
    store
        .accounts
        .get_mut("demo")
        .ok_or_else(|| "default demo account should exist".to_string())?
        .password = password.to_string();
    Ok(())
}

#[test]
fn pre_rename_failure_keeps_live_and_disk_old_but_allows_a_later_write() {
    let directory = unique_temp_dir("pre-rename");
    let path = directory.join("accounts.json");
    let config = SimulationConfig::default().with_account_store_path(path.clone());
    config
        .save_account_store()
        .expect("baseline account store should be durable");
    let original_password = live_demo_password(&config);

    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
    let error = config
        .commit_account_store_transaction(&["demo".to_string()], |store| {
            set_demo_password(store, "must-not-publish")
        })
        .expect_err("pre-rename failure should reject the transaction");

    assert!(error.contains("not committed"));
    assert!(error.contains("before account-store rename"));
    assert_eq!(live_demo_password(&config), original_password);
    assert_eq!(disk_demo_password(&path), original_password);

    config
        .commit_account_store_transaction(&["demo".to_string()], |store| {
            set_demo_password(store, "allowed-after-pre-rename")
        })
        .expect("known pre-rename failure must not freeze later writes");
    assert_eq!(live_demo_password(&config), "allowed-after-pre-rename");
    assert_eq!(disk_demo_password(&path), "allowed-after-pre-rename");

    fs::remove_dir_all(directory).expect("durability test directory should be removed");
}

#[test]
fn post_rename_unknown_keeps_live_old_and_freezes_all_shared_writers() {
    let directory = unique_temp_dir("post-rename");
    let path = directory.join("accounts.json");
    let config = SimulationConfig::default().with_account_store_path(path.clone());
    config
        .save_account_store()
        .expect("baseline account store should be durable");
    let original_password = live_demo_password(&config);
    let cloned = config.clone();

    config.inject_account_store_transaction_fault(
        AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    let error = config
        .commit_account_store_transaction(&["demo".to_string()], |store| {
            set_demo_password(store, "published-but-unacknowledged")
        })
        .expect_err("post-rename fault should report an unknown outcome");

    assert!(error.contains(ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN));
    assert!(error.contains("commit outcome unknown"));
    assert_eq!(live_demo_password(&config), original_password);
    assert_eq!(
        disk_demo_password(&path),
        "published-but-unacknowledged",
        "the replacement may already be visible even though durability was not acknowledged"
    );

    let closure_ran = Arc::new(AtomicBool::new(false));
    let closure_flag = Arc::clone(&closure_ran);
    let retry_error = cloned
        .commit_account_store_transaction(&["demo".to_string()], move |_| {
            closure_flag.store(true, Ordering::SeqCst);
            Ok(())
        })
        .expect_err("a clone must share the frozen write state");
    assert!(retry_error.contains("writes are frozen"));
    assert!(!closure_ran.load(Ordering::SeqCst));

    let full_save_error = config
        .save_account_store()
        .expect_err("full-store save must be blocked while frozen");
    assert!(full_save_error.contains("writes are frozen"));
    let account_save_error = config
        .save_account_store_account("demo")
        .expect_err("account save must be blocked while frozen");
    assert!(account_save_error.contains("writes are frozen"));
    assert_eq!(
        disk_demo_password(&path),
        "published-but-unacknowledged",
        "old live state must not overwrite the possibly committed replacement"
    );

    fs::remove_dir_all(directory).expect("durability test directory should be removed");
}
