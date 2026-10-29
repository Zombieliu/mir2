//! Real File authority; only the Sui fullnode/issuer boundary is replaced.
use super::*;
use mir2_simulation::{AccountStore, AccountStoreTransactionFault};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize},
    Mutex,
};
static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
#[test]
fn usdc_runtime_gate_checks_all_markers_and_normalizes_production_aliases() {
    for alias in [
        "production",
        "prod",
        "staging",
        "stage",
        "preprod",
        "preproduction",
    ] {
        for slot in 0..3 {
            let mut markers = [Some("development".into()), Some("test".into()), None];
            markers[slot] = Some(format!("  {}\t", alias.to_ascii_uppercase()));
            assert_eq!(
                validate_testnet_runtime(markers.map(Ok)).unwrap_err(),
                "usdcTestnetConfigurationRequired",
                "alias {alias}, marker {slot}"
            );
        }
    }
}
#[test]
fn usdc_runtime_gate_accepts_absent_and_explicit_local_test_markers() {
    assert!(validate_testnet_runtime([Ok(None), Ok(None), Ok(None)]).is_ok());
    for value in [
        "development",
        "dev",
        "test",
        "isolated-test",
        "local",
        " \t",
    ] {
        assert!(validate_testnet_runtime([Ok(Some(value.into())), Ok(None), Ok(None)]).is_ok());
    }
}
#[test]
fn usdc_runtime_gate_fails_closed_on_unreadable_marker_and_conflicting_aliases() {
    assert!(validate_testnet_runtime([Ok(Some("development".into())), Err(()), Ok(None)]).is_err());
    assert!(validate_testnet_runtime([
        Ok(Some("test".into())),
        Ok(Some("stage".into())),
        Ok(Some("development".into()))
    ])
    .is_err());
}
fn address(c: char) -> String {
    format!("0x{}", c.to_string().repeat(64))
}
struct Fixture {
    config: SimulationConfig,
    path: PathBuf,
    order: UsdcRechargeOrder,
    at: u64,
}
impl Fixture {
    fn new(label: &str) -> Self {
        let at = now();
        let initial = SimulationConfig::default();
        let path = std::env::temp_dir().join(format!(
            "mir2-usdc-gateway-{label}-{}-{at}-{}.json",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(
            &path,
            serde_json::to_vec(&*initial.account_store.lock().unwrap()).unwrap(),
        )
        .unwrap();
        let config = SimulationConfig::default()
            .with_account_store_path(&path)
            .with_billing_monthly_card_credit_price(Some(100))
            .unwrap();
        let offer = RechargeOffer {
            id: "configured-usd-fixture".into(),
            label: "Explicit test fixture".into(),
            currency: "usd".into(),
            amount_minor: 100,
            credits: 100,
        };
        let d = UsdcDomain {
            chain_identifier: TESTNET_CHAIN_IDENTIFIER.into(),
            package_id: address('1'),
            config_id: address('2'),
            merchant: address('3'),
            coin_type: TESTNET_USDC_COIN_TYPE.into(),
            rule_version: 1,
        };
        let order = config
            .prepare_usdc_recharge_order(
                "demo",
                config.default_character.index,
                "request-one",
                &offer,
                &d,
                &"c".repeat(64),
                at + 1_800_000,
                at + 1_200_000,
                at,
            )
            .unwrap();
        let order = config
            .bind_usdc_recharge_payer("demo", &order.id, &address('4'), &"c".repeat(64), at + 1)
            .unwrap();
        Self {
            config,
            path,
            order,
            at,
        }
    }
    fn provider(&self, driver: Arc<dyn UsdcDriver>) -> UsdcBilling {
        UsdcBilling {
            enabled: true,
            domain: self.order.domain.clone(),
            offers: vec![self.order.offer.clone()],
            origin: "http://127.0.0.1:17830".into(),
            checkout_key: Arc::new(vec![42; 32]),
            driver,
        }
    }
    fn disk(&self) -> AccountStore {
        serde_json::from_slice(&std::fs::read(&self.path).unwrap()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
struct Mock {
    config: SimulationConfig,
    path: PathBuf,
    terms: Value,
    at: u64,
    prepares: AtomicUsize,
    submits: AtomicUsize,
    unknown_once: AtomicBool,
    paid: AtomicBool,
    bad: AtomicBool,
    fail_prepared_commit: AtomicBool,
    calls: Mutex<Vec<String>>,
}
impl Mock {
    fn new(f: &Fixture) -> Self {
        Self {
            config: f.config.clone(),
            path: f.path.clone(),
            terms: chain_order(&f.order).unwrap(),
            at: f.at,
            prepares: AtomicUsize::new(0),
            submits: AtomicUsize::new(0),
            unknown_once: AtomicBool::new(false),
            paid: AtomicBool::new(false),
            bad: AtomicBool::new(false),
            fail_prepared_commit: AtomicBool::new(false),
            calls: Mutex::new(vec![]),
        }
    }
}
impl UsdcDriver for Mock {
    fn call(&self, op: &str, p: Value) -> Result<Value, String> {
        self.calls.lock().unwrap().push(op.into());
        if p.get("order").is_some() {
            assert_eq!(p["order"], self.terms);
        }
        match op {
            "prepareInvoice" => {
                self.prepares.fetch_add(1, Ordering::SeqCst);
                if self.fail_prepared_commit.swap(false, Ordering::SeqCst) {
                    self.config.inject_account_store_transaction_fault(
                        AccountStoreTransactionFault::BeforePersist,
                    );
                }
                Ok(json!({"bytesB64":"cHJlcGFyZWQ=","signature":"c2ln","digest":"1".repeat(44)}))
            }
            "submitPrepared" => {
                self.submits.fetch_add(1, Ordering::SeqCst);
                let disk: AccountStore =
                    serde_json::from_slice(&std::fs::read(&self.path).unwrap()).unwrap();
                let o = disk.accounts["demo"]
                    .billing
                    .as_ref()
                    .unwrap()
                    .sui_usdc_orders
                    .values()
                    .next()
                    .unwrap();
                let tx = o
                    .prepared_tx
                    .as_ref()
                    .expect("exact transaction must be durable before broadcast");
                assert_eq!(p["prepared"]["bytesB64"], tx.bytes_base64);
                assert_eq!(p["prepared"]["signature"], tx.signature_base64);
                assert_eq!(p["prepared"]["digest"], tx.digest);
                Ok(json!({"submitted":true,"digest":tx.digest}))
            }
            "observeCreate" => {
                if self.unknown_once.swap(false, Ordering::SeqCst) {
                    return Ok(json!({"state":"unknown"}));
                }
                Ok(
                    json!({"state":"created","invoiceId":address('5'),"digest":"1".repeat(44),"checkpoint":"12"}),
                )
            }
            "observeInvoice" => {
                if !self.paid.load(Ordering::SeqCst) {
                    return Ok(json!({"state":"unpaid"}));
                }
                let mut receipt = self.terms.clone();
                receipt["networkHash"] = json!(prefixed(&sha(TESTNET_CHAIN_IDENTIFIER.as_bytes())));
                receipt["packageId"] = json!(address('1'));
                receipt["configId"] = json!(address('2'));
                receipt["invoiceId"] = json!(address('5'));
                receipt["coinType"] = json!(TESTNET_USDC_COIN_TYPE);
                receipt["transactionDigest"] = json!("3".repeat(44));
                receipt["eventSequence"] = json!("0");
                receipt["checkpoint"] = json!("13");
                receipt["paidAtMs"] = json!((self.at + 2).to_string());
                if self.bad.load(Ordering::SeqCst) {
                    receipt["merchant"] = json!(address('9'));
                }
                Ok(json!({"state":"paid","receipt":receipt}))
            }
            _ => panic!("unexpected mock operation"),
        }
    }
}
#[test]
fn usdc_worker_restarts_exact_creation_and_applies_confirmed_payment_once() {
    let f = Fixture::new("restart");
    let driver = Arc::new(Mock::new(&f));
    driver.unknown_once.store(true, Ordering::SeqCst);
    let p = f.provider(driver.clone());
    assert!(run_order(&f.config, &p, "demo", &f.order.id, "worker-one").is_err());
    assert!(f.disk().accounts["demo"]
        .billing
        .as_ref()
        .unwrap()
        .sui_usdc_orders[&f.order.id]
        .prepared_tx
        .is_some());
    let restarted = SimulationConfig::default()
        .with_account_store_path(&f.path)
        .with_billing_monthly_card_credit_price(Some(100))
        .unwrap();
    run_order(&restarted, &p, "demo", &f.order.id, "worker-two").unwrap();
    assert_eq!(driver.prepares.load(Ordering::SeqCst), 1);
    assert_eq!(driver.submits.load(Ordering::SeqCst), 2);
    let character = f.order.character_index;
    let before = restarted
        .recharge_credit_status("demo", character)
        .unwrap()
        .0;
    driver.paid.store(true, Ordering::SeqCst);
    run_order(&restarted, &p, "demo", &f.order.id, "worker-three").unwrap();
    assert_eq!(
        restarted.recharge_credit_status("demo", character).unwrap(),
        (before, 100)
    );
    let applied = restarted
        .consume_inactive_pending_recharge_credits("demo", character, f.at + 3)
        .unwrap();
    assert_eq!(applied.granted, 100);
    run_order(&restarted, &p, "demo", &f.order.id, "worker-four").unwrap();
    assert_eq!(
        restarted
            .consume_inactive_pending_recharge_credits("demo", character, f.at + 4)
            .unwrap()
            .granted,
        0
    );
    assert_eq!(
        restarted.recharge_credit_status("demo", character).unwrap(),
        (before + 100, 0)
    );
}
#[test]
fn usdc_worker_rejects_mismatched_final_receipt_without_wallet_credit() {
    let f = Fixture::new("mismatch");
    let driver = Arc::new(Mock::new(&f));
    driver.paid.store(true, Ordering::SeqCst);
    driver.bad.store(true, Ordering::SeqCst);
    let p = f.provider(driver);
    assert!(run_order(&f.config, &p, "demo", &f.order.id, "worker").is_err());
    let o = f.config.usdc_recharge_order("demo", &f.order.id).unwrap();
    assert!(o.receipt.is_none());
    assert_eq!(
        f.config
            .recharge_credit_status("demo", o.character_index)
            .unwrap()
            .1,
        0
    );
}
#[test]
fn usdc_worker_does_not_broadcast_when_prepared_bytes_cannot_commit() {
    let f = Fixture::new("before-persist");
    let driver = Arc::new(Mock::new(&f));
    let p = f.provider(driver.clone());
    driver.fail_prepared_commit.store(true, Ordering::SeqCst);
    assert!(run_order(&f.config, &p, "demo", &f.order.id, "worker").is_err());
    assert_eq!(driver.prepares.load(Ordering::SeqCst), 1);
    assert_eq!(driver.submits.load(Ordering::SeqCst), 0);
    assert!(f.disk().accounts["demo"]
        .billing
        .as_ref()
        .unwrap()
        .sui_usdc_orders[&f.order.id]
        .prepared_tx
        .is_none());
}
#[test]
fn usdc_reopen_preserves_frozen_order_when_sales_and_offer_are_disabled() {
    let f = Fixture::new("reopen");
    let d = Arc::new(Mock::new(&f));
    let mut p = f.provider(d);
    let (url, created) = p
        .checkout(
            &f.config,
            "demo",
            f.order.character_index,
            "another-request",
            &f.order.offer.id,
        )
        .unwrap();
    p.enabled = false;
    p.offers.clear();
    let (reopened, same) = p
        .checkout(
            &f.config,
            "demo",
            f.order.character_index,
            "another-request",
            &f.order.offer.id,
        )
        .unwrap();
    assert_eq!(url, reopened);
    assert_eq!(created, same);
    assert_eq!(
        f.config
            .usdc_recharge_orders("demo", f.order.character_index)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn usdc_public_confirmed_unpaid_invoice_expires_at_exact_deadline_without_changing_authority() {
    let f = Fixture::new("public-expiry-boundary");
    assert_eq!(
        public_order_at(&f.order, f.order.expires_at_ms)["state"],
        "expired"
    );
    let p = f.provider(Arc::new(Mock::new(&f)));
    run_order(&f.config, &p, "demo", &f.order.id, "worker").unwrap();
    let o = f.config.usdc_recharge_order("demo", &f.order.id).unwrap();
    let before = std::fs::read(&f.path).unwrap();
    assert_eq!(o.state, UsdcOrderState::InvoiceBound);
    assert_eq!(
        public_order_at(&o, o.expires_at_ms - 1)["state"],
        "awaitingPayment"
    );
    for at in [o.expires_at_ms, o.expires_at_ms + 1] {
        assert_eq!(public_order_at(&o, at)["state"], "expired");
    }
    assert_eq!(std::fs::read(&f.path).unwrap(), before);
    assert_eq!(
        f.config.usdc_recharge_order("demo", &o.id).unwrap().state,
        UsdcOrderState::InvoiceBound
    );
    assert!(f
        .config
        .usdc_recharge_reconcile_candidates(100)
        .unwrap()
        .iter()
        .any(|(_, candidate)| candidate.id == o.id));
}

#[test]
fn usdc_public_expiry_does_not_hide_unknown_creation_submitted_payment_or_delivered_credit() {
    let f = Fixture::new("public-expiry-recovery-states");
    let driver = Arc::new(Mock::new(&f));
    driver.unknown_once.store(true, Ordering::SeqCst);
    let p = f.provider(driver.clone());
    assert!(run_order(&f.config, &p, "demo", &f.order.id, "worker").is_err());
    let unknown = f.config.usdc_recharge_order("demo", &f.order.id).unwrap();
    assert_eq!(unknown.state, UsdcOrderState::InvoicePrepared);
    assert_eq!(
        public_order_at(&unknown, unknown.expires_at_ms)["state"],
        "invoicePending"
    );
    run_order(&f.config, &p, "demo", &f.order.id, "worker").unwrap();
    let submitted = f
        .config
        .submit_usdc_payment_hint(
            "demo",
            &f.order.id,
            &"c".repeat(64),
            &"3".repeat(44),
            f.at + 3,
        )
        .unwrap();
    assert_eq!(
        public_order_at(&submitted, submitted.expires_at_ms)["state"],
        "paymentSubmitted"
    );
    driver.paid.store(true, Ordering::SeqCst);
    run_order(&f.config, &p, "demo", &f.order.id, "worker").unwrap();
    let paid = f.config.usdc_recharge_order("demo", &f.order.id).unwrap();
    assert_eq!(
        public_order_at(&paid, paid.expires_at_ms + 1)["state"],
        "paidPending"
    );
    f.config
        .consume_inactive_pending_recharge_credits(
            "demo",
            paid.character_index,
            paid.expires_at_ms + 1,
        )
        .unwrap();
    let applied = f.config.usdc_recharge_order("demo", &f.order.id).unwrap();
    assert_eq!(
        public_order_at(&applied, applied.expires_at_ms + 1)["state"],
        "applied"
    );
}
