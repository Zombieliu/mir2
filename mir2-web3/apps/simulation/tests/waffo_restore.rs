//! Backup restore cannot erase the only durable record of a WAFFO request that
//! may already have reached the provider, including before a payment receipt.
use mir2_simulation::{
    billing::{FiatPaymentProvider, RechargeOffer, RechargeOrder},
    billing_provenance::CreditProvenance,
    SimulationConfig,
};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const NOW: u64 = 1_800_000_000_000;

struct BackupFile(PathBuf);
impl BackupFile {
    fn new(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "mir2-waffo-restore-{label}-{}-{}.json",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
        )))
    }
}
impl Drop for BackupFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn prepare(config: &SimulationConfig) -> RechargeOrder {
    config
        .prepare_recharge_order_for_provider(
            "demo",
            config.default_character.index,
            "waffo-unknown-create",
            &RechargeOffer {
                id: "credits-100".into(),
                label: "100 Credits".into(),
                currency: "usd".into(),
                amount_minor: 100,
                credits: 100,
            },
            NOW,
            false,
            FiatPaymentProvider::Waffo,
        )
        .unwrap()
}

fn start_without_revenue_receipt(config: &SimulationConfig, order: &RechargeOrder) {
    assert!(config
        .begin_recharge_provider_request("demo", &order.id, FiatPaymentProvider::Waffo, NOW + 1,)
        .unwrap());
    // A valid minimal imported ledger may have a submitted request but no
    // wallet/provenance entries yet. Zero-credit bookkeeping must not be the
    // condition that protects a possibly charged request from backup rollback.
    let mut store = config.account_store.lock().unwrap();
    let account = store.accounts.get_mut("demo").unwrap();
    let ledger = account.billing.as_mut().unwrap();
    assert!(ledger.provenance.payments.is_empty());
    assert!(ledger.provenance.adverse_events.is_empty());
    ledger.provenance = CreditProvenance::default();
    ledger.validate().unwrap();
    assert!(ledger.provenance.is_empty());
}

fn snapshot(config: &SimulationConfig) -> serde_json::Value {
    serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap()
}

#[test]
fn waffo_restore_cannot_remove_unknown_provider_request_before_first_receipt() {
    let config = SimulationConfig::default();
    let backup = BackupFile::new("before-order");
    config.backup_account_store(&backup.0).unwrap();
    let order = prepare(&config);
    start_without_revenue_receipt(&config, &order);
    let before = snapshot(&config);

    assert_eq!(
        config
            .restore_account_store_from_backup(&backup.0)
            .unwrap_err(),
        "billingFinancialRestoreRequiresReconciliation"
    );
    assert_eq!(snapshot(&config), before);
    assert!(!config
        .begin_recharge_provider_request("demo", &order.id, FiatPaymentProvider::Waffo, NOW + 2,)
        .unwrap());
    assert_eq!(
        config
            .recharge_credit_status("demo", config.default_character.index)
            .unwrap(),
        (0, 0)
    );
}

#[test]
fn waffo_restore_cannot_rewind_single_sender_marker_to_prepared_order() {
    let config = SimulationConfig::default();
    let order = prepare(&config);
    let backup = BackupFile::new("before-send");
    config.backup_account_store(&backup.0).unwrap();
    start_without_revenue_receipt(&config, &order);
    let before = snapshot(&config);

    assert_eq!(
        config
            .restore_account_store_from_backup(&backup.0)
            .unwrap_err(),
        "billingFinancialRestoreRequiresReconciliation"
    );
    assert_eq!(snapshot(&config), before);
    assert!(!config
        .begin_recharge_provider_request("demo", &order.id, FiatPaymentProvider::Waffo, NOW + 2,)
        .unwrap());
}

#[test]
fn waffo_restore_with_identical_unknown_request_keeps_recovery_inquiry_only() {
    let config = SimulationConfig::default();
    let order = prepare(&config);
    start_without_revenue_receipt(&config, &order);
    let expected = config
        .recharge_order_for_payment("demo", &order.id)
        .unwrap();
    let backup = BackupFile::new("matching");
    config.backup_account_store(&backup.0).unwrap();

    config.restore_account_store_from_backup(&backup.0).unwrap();
    assert_eq!(
        config
            .recharge_order_for_payment("demo", &order.id)
            .unwrap(),
        expected
    );
    assert!(!config
        .begin_recharge_provider_request("demo", &order.id, FiatPaymentProvider::Waffo, NOW + 2,)
        .unwrap());
}
