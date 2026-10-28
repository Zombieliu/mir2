//! Worker integration tests use the real File authority and real actor signatures.
//! Only the fullnode/build/submit boundary is replaced; no chain transaction is sent.
use super::*;
use ed25519_consensus::SigningKey;
use mir2_protocol::{procurement::EquipmentConditions, MirClass, MirGender};
use mir2_simulation::{AccountRecord, AccountStore, AccountStoreTransactionFault, CharacterRecord};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{atomic::AtomicUsize, Mutex},
};

static FILE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct FileFixture {
    config: SimulationConfig,
    path: PathBuf,
    first: Order,
    started: u64,
}
impl FileFixture {
    fn new(label: &str, count: usize) -> Self {
        Self::seed(label, count, |_, _, _| {})
    }
    fn seed(
        label: &str,
        count: usize,
        customize: impl FnOnce(&mut AccountStore, &ProcurementConfig, u64),
    ) -> Self {
        let started = now();
        let c = ProcurementConfig {
            enabled: true,
            allow_test_credits: true,
            domain: Domain {
                network_hash: [1; 32],
                package_id: [2; 32],
                dapp_storage_id: [3; 32],
                oracle_key_id: 1,
            },
            ..Default::default()
        };
        let mut store = AccountStore::new(character(0, "Buyer0"));
        store.accounts.clear();
        for n in 0..count {
            let account = format!("buyer-{n:03}");
            let mut a = AccountRecord::new(character(n as i32, &format!("Buyer{n}")));
            let save = a.saves.get_mut(&(n as i32)).unwrap();
            save.gold = 500_000; // The other 500,000 is already reserved below.
            let key = SigningKey::from([7; 32]); // Public, test-only actor key.
            let conditions = EquipmentConditions::default();
            let publish = sign(
                IntentV1 {
                    network_hash: c.domain.network_hash,
                    package_id: c.domain.package_id,
                    dapp_storage_id: c.domain.dapp_storage_id,
                    operation: 1,
                    order_id: digest(account.as_bytes()),
                    fill_id: [0; 32],
                    buyer_ref: actor_ref(&account, save),
                    seller_ref: [0; 32],
                    template_id: 245,
                    conditions_hash: digest(&conditions.canonical_bcs().unwrap()),
                    quantity: 1,
                    price_gold: 500_000,
                    expires_at_ms: started + 3_600_000,
                    rule_version: 1,
                    nonce: digest(format!("nonce-{account}").as_bytes()),
                    valid_until_ms: started + 300_000,
                    actor_public_key: key.verification_key().to_bytes(),
                },
                &key,
            );
            let id = hex(&publish.intent.order_id);
            let order = Order {
                id: id.clone(),
                buyer_account: account.clone(),
                buyer_character: n as i32,
                buyer_name: format!("Buyer{n}"),
                publish: publish.clone(),
                rule: c.rule.clone(),
                status: OrderStatus::Publishing,
                escrow_gold: 500_000,
                fee_batch: "explicit-test-credit".into(),
                fee_consumed: false,
                fee_refunded: false,
                fence: 0,
                object_id: None,
                offers: BTreeMap::new(),
                winning_fill: None,
                final_receipt: None,
                last_chain_transaction: None,
                conditions,
            };
            let mut ledger = ProcurementLedger::default();
            ledger.credits.insert(
                order.fee_batch.clone(),
                CreditBatch {
                    id: order.fee_batch.clone(),
                    source: CreditSource::Test,
                    granted: 10,
                    reserved: 1,
                    consumed: 0,
                    expires_at_ms: started + 86_400_000,
                    payment_order_id: None,
                },
            );
            ledger.outbox.insert(
                format!("publish:{id}"),
                task(
                    format!("publish:{id}"),
                    &id,
                    JobKind::Publish,
                    publish,
                    [0; 32],
                ),
            );
            ledger.orders.insert(id, order);
            a.billing = Some(Default::default());
            a.procurement = Some(ledger);
            store.accounts.insert(account, a);
        }
        customize(&mut store, &c, started);
        let first = store.accounts["buyer-000"]
            .procurement
            .as_ref()
            .unwrap()
            .orders
            .values()
            .next()
            .unwrap()
            .clone();
        for a in store.accounts.values() {
            a.procurement.as_ref().unwrap().validate().unwrap();
        }
        let path = std::env::temp_dir().join(format!(
            "mir2-worker-{label}-{}-{}-{}.json",
            std::process::id(),
            started,
            FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        // Seed before the authority is acquired. All subsequent changes go
        // through the production durable APIs, never the shared image mutex.
        std::fs::write(&path, serde_json::to_vec(&store).unwrap()).unwrap();
        let mut config = SimulationConfig::default().with_account_store_path(&path);
        config.procurement_config = c;
        config.procurement_view("buyer-000").unwrap();
        Self {
            config,
            path,
            first,
            started,
        }
    }
    fn disk(&self) -> AccountStore {
        serde_json::from_slice(&std::fs::read(&self.path).unwrap()).unwrap()
    }
    fn ledger(&self) -> ProcurementLedger {
        self.disk().accounts["buyer-000"]
            .procurement
            .clone()
            .unwrap()
    }
}

fn character(index: i32, name: &str) -> CharacterRecord {
    CharacterRecord {
        index,
        name: name.into(),
        level: 22,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    }
}
fn sign(intent: IntentV1, key: &SigningKey) -> SignedIntent {
    let signature = key.sign(&intent.hash()).to_bytes().to_vec();
    let signed = SignedIntent { intent, signature };
    signed.verify().unwrap();
    signed
}
fn task(
    id: String,
    order_id: &str,
    kind: JobKind,
    signed: SignedIntent,
    instance: [u8; 32],
) -> OutboxTask {
    let i = &signed.intent;
    let proof = OracleProofV1 {
        intent_hash: i.hash(),
        asset_lock_id: digest(&[b"MIR2_ASSET_LOCK_V1".as_slice(), &i.hash()].concat()),
        asset_instance_hash: instance,
        oracle_key_id: 1,
        valid_until_ms: i.valid_until_ms,
        fence: 0,
    };
    OutboxTask {
        id,
        order_id: order_id.into(),
        kind,
        signed: Some(signed),
        proof: Some(proof),
        status: JobStatus::Pending,
        attempts: 0,
        transaction_digests: vec![],
        object_id: None,
        lease_owner: None,
        lease_until_ms: 0,
        signed_transactions: BTreeMap::new(),
        failed_transactions: BTreeMap::new(),
        archived_transactions: BTreeMap::new(),
    }
}

#[derive(Default)]
struct MockState {
    calls: Vec<(String, Value)>,
    observations: VecDeque<Value>,
    pages: VecDeque<Value>,
    finalities: VecDeque<Value>,
    builds: usize,
    fail_submit: bool,
    fail_prepare: Option<SimulationConfig>,
}
#[derive(Default)]
struct MockChain(Mutex<MockState>);
impl MockChain {
    fn observations(&self, values: impl IntoIterator<Item = Value>) {
        self.0.lock().unwrap().observations.extend(values);
    }
    fn calls(&self, operation: &str) -> Vec<Value> {
        self.0
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|(op, _)| op == operation)
            .map(|(_, v)| v.clone())
            .collect()
    }
}
impl ChainDriver for MockChain {
    fn call(&self, operation: &str, payload: Value) -> Result<Value, String> {
        let mut s = self.0.lock().unwrap();
        s.calls.push((operation.into(), payload.clone()));
        match operation {
            "replayEvents" => Ok(s
                .pages
                .pop_front()
                .unwrap_or_else(|| json!({"events":[],"nextCursor":null,"hasNextPage":false}))),
            "queryOrder" => Ok(s
                .observations
                .pop_front()
                .unwrap_or_else(|| json!({"found":false}))),
            "build" => {
                s.builds += 1;
                if let Some(config) = s.fail_prepare.take() {
                    // All replay/claim writes have completed. Fail exactly
                    // the worker's required write-before-submit operation.
                    config.inject_account_store_transaction_fault(
                        AccountStoreTransactionFault::BeforeFileRename,
                    );
                }
                Ok(
                    json!({"action":payload["action"],"digest":format!("mock-{}-{}",payload["action"].as_str().unwrap(),s.builds),
                    "transactionBytes":format!("public-test-envelope-{}",s.builds),"oracleSignature":encode_hex(&[9;64])}),
                )
            }
            "queryFinality" => Ok(s
                .finalities
                .pop_front()
                .unwrap_or_else(|| json!({"known":false}))),
            "submitExact" if s.fail_submit => Err("procurementChainResultUnknown".into()),
            "submitExact" => Ok(json!({"sent":true})),
            _ => Err(format!("unexpectedMockOperation:{operation}")),
        }
    }
}

fn observation(order: &Order, status: u8, checkpoint: u64, clock_ms: u64) -> Value {
    let i = &order.publish.intent;
    let tx = format!("observed-{checkpoint}");
    json!({"found":true,"orderObjectId":encode_hex(&[8;32]),"orderId":encode_hex(&i.order_id),
        "status":status,"checkpoint":checkpoint.to_string(),
        "fields":{"publish_intent_hash":encode_hex(&i.hash()),
            "escrow_lock_id":encode_hex(&digest(&[b"MIR2_ASSET_LOCK_V1".as_slice(),&i.hash()].concat())),
            "expires_at_ms":i.expires_at_ms.to_string(),"fill_id":encode_hex(&[0;32]),
            "buyer_ref":encode_hex(&i.buyer_ref),"seller_ref":encode_hex(&[0;32]),"template_id":245,
            "conditions_hash":encode_hex(&i.conditions_hash),"quantity":"1","price_gold":i.price_gold.to_string(),
            "rule_version":i.rule_version.to_string(),"asset_instance_hash":encode_hex(&[0;32]),"fence":"0"},
        "observation":{"coherent":true,"clock":{"finalized":true,"checkpoint":checkpoint.to_string(),"timestampMs":clock_ms.to_string()},
            "transactionCheckpoint":checkpoint.to_string(),"transactionDigest":tx,"objectPreviousTransaction":tx,"eventSequence":"0"},
        "receipt":{"finalized":true,"success":true,"digest":tx}})
}
fn tick(f: &FileFixture, chain: &MockChain) -> Result<(), String> {
    run_once(&f.config, chain, "file-integration", &mut None)
}
fn assert_reserved(f: &FileFixture) {
    let disk = f.disk();
    let ledger = disk.accounts["buyer-000"].procurement.as_ref().unwrap();
    assert_eq!(ledger.orders[&f.first.id].escrow_gold, 500_000);
    assert_eq!(disk.accounts["buyer-000"].saves[&0].gold, 500_000);
    assert_eq!(
        (
            ledger.credits["explicit-test-credit"].reserved,
            ledger.credits["explicit-test-credit"].consumed
        ),
        (1, 0)
    );
    assert!(ledger.deliveries.is_empty());
}

#[test]
fn file_worker_create_is_private_activate_is_public_and_charges_once() {
    let f = FileFixture::new("two-phase", 1);
    let chain = MockChain::default();
    tick(&f, &chain).unwrap();
    assert_reserved(&f);
    assert!(f
        .config
        .procurement_public_orders(now())
        .unwrap()
        .is_empty());
    chain.observations([observation(&f.first, 0, 10, f.started + 1)]);
    tick(&f, &chain).unwrap();
    assert_reserved(&f);
    assert!(f
        .config
        .procurement_public_orders(now())
        .unwrap()
        .is_empty());
    chain.observations([observation(&f.first, 1, 11, f.started + 2)]);
    tick(&f, &chain).unwrap();
    chain.observations([observation(&f.first, 1, 12, f.started + 3)]);
    tick(&f, &chain).unwrap();
    let ledger = f.ledger();
    assert_eq!(ledger.orders[&f.first.id].status, OrderStatus::Open);
    assert_eq!(
        (
            ledger.credits["explicit-test-credit"].reserved,
            ledger.credits["explicit-test-credit"].consumed
        ),
        (0, 1)
    );
    assert_eq!(f.config.procurement_public_orders(now()).unwrap().len(), 1);
    let actions: Vec<_> = chain
        .calls("build")
        .iter()
        .map(|v| v["action"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(actions, ["create", "activate"]);
}

#[test]
fn file_worker_never_submits_when_prepared_envelope_cannot_commit() {
    let f = FileFixture::new("prepare-fault", 1);
    let chain = MockChain::default();
    chain.0.lock().unwrap().fail_prepare = Some(f.config.clone());
    assert_eq!(tick(&f, &chain).unwrap_err(), "procurementRecoveryPending");
    assert_eq!(chain.calls("build").len(), 1);
    assert!(chain.calls("queryFinality").is_empty());
    assert!(chain.calls("submitExact").is_empty());
    assert!(f.ledger().outbox[&format!("publish:{}", f.first.id)]
        .signed_transactions
        .is_empty());
    assert_reserved(&f);
}

#[test]
fn file_worker_unknown_restarts_with_identical_envelope_and_preserves_custody() {
    let f = FileFixture::new("unknown-restart", 1);
    let chain = MockChain::default();
    chain.0.lock().unwrap().fail_submit = true;
    assert!(tick(&f, &chain).is_err());
    let before = f.ledger().outbox[&format!("publish:{}", f.first.id)].clone();
    assert_eq!(before.status, JobStatus::Unknown);
    assert_reserved(&f);
    let FileFixture {
        config,
        path,
        first,
        started,
    } = f;
    let domain = config.procurement_config.clone();
    drop(config);
    let mut recovered = SimulationConfig::default().with_account_store_path(&path);
    recovered.procurement_config = domain;
    let f = FileFixture {
        config: recovered,
        path,
        first,
        started,
    };
    assert!(tick(&f, &chain).is_err());
    let after = f.ledger().outbox[&before.id].clone();
    assert_eq!(before.signed_transactions, after.signed_transactions);
    assert_eq!(before.signed, after.signed);
    assert_eq!(before.proof, after.proof);
    assert_eq!(chain.calls("build").len(), 1);
    let submitted = chain.calls("submitExact");
    assert_eq!(submitted.len(), 2);
    assert_eq!(submitted[0], submitted[1]);
    assert_reserved(&f);
}

#[test]
fn file_worker_only_finalized_failure_allows_archived_new_attempt_without_refund() {
    let f = FileFixture::new("finalized-failure", 1);
    let chain = MockChain::default();
    chain.0.lock().unwrap().finalities.push_back(json!({"known":true,"finalized":true,"success":false,"digest":"mock-create-1","checkpoint":"71"}));
    tick(&f, &chain).unwrap();
    assert!(chain.calls("submitExact").is_empty());
    assert_reserved(&f);
    let first = f.ledger().outbox[&format!("publish:{}", f.first.id)].clone();
    assert_eq!(first.failed_transactions["mock-create-1"], 71);
    tick(&f, &chain).unwrap();
    let second = f.ledger().outbox[&first.id].clone();
    assert_eq!(
        second.archived_transactions["mock-create-1"],
        first.signed_transactions["create"]
    );
    assert_eq!(
        second.signed_transactions["create"].transaction_digest,
        "mock-create-2"
    );
    assert_eq!(chain.calls("submitExact").len(), 1);
    assert_reserved(&f);
}

#[test]
fn file_worker_rotation_serves_account_beyond_first_128() {
    let f = FileFixture::new("rotation-129", 129);
    let chain = MockChain::default();
    let mut cursor = None;
    run_once(&f.config, &chain, "rotation", &mut cursor).unwrap();
    let visited = || {
        chain
            .calls("queryOrder")
            .into_iter()
            .filter_map(|v| v["orderId"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(visited().len(), 128);
    assert_eq!(cursor.as_deref(), Some("buyer-127"));
    run_once(&f.config, &chain, "rotation", &mut cursor).unwrap();
    assert_eq!(visited().len(), 129);
    let disk = f.disk();
    assert!(disk.accounts["buyer-128"]
        .procurement
        .as_ref()
        .unwrap()
        .outbox
        .values()
        .any(|j| j.attempts > 0 && !j.signed_transactions.is_empty()));
}

#[test]
fn file_worker_rejects_wrong_domain_changed_terms_and_unsupported_receipts() {
    for defect in [
        "domain",
        "conditions",
        "publishHash",
        "quantity",
        "status",
        "unfinalized",
        "foreignDigest",
    ] {
        let mut f = FileFixture::new(defect, 1);
        let chain = MockChain::default();
        let mut observed = observation(&f.first, 1, 20, f.started + 1);
        match defect {
            "domain" => f.config.procurement_config.domain.package_id = [19; 32],
            "conditions" => observed["fields"]["conditions_hash"] = json!(encode_hex(&[20; 32])),
            "publishHash" => {
                observed["fields"]["publish_intent_hash"] = json!(encode_hex(&[20; 32]))
            }
            "quantity" => observed["fields"]["quantity"] = json!("2"),
            "status" => observed["status"] = json!(5),
            "unfinalized" => observed["receipt"]["finalized"] = json!(false),
            _ => observed["receipt"]["digest"] = json!("another-transaction"),
        }
        chain.observations([observed]);
        assert!(tick(&f, &chain).is_err(), "accepted {defect}");
        let ledger = f.ledger();
        assert_eq!(
            ledger.orders[&f.first.id].status,
            OrderStatus::Publishing,
            "{defect}"
        );
        assert!(ledger.chain_inbox.is_empty(), "{defect}");
        assert!(chain.calls("submitExact").is_empty(), "{defect}");
        assert_reserved(&f);
    }
}

#[test]
fn file_worker_vector_event_and_empty_checkpoint_resume_from_durable_cursor() {
    let f = FileFixture::new("cursor-restart", 1);
    let chain = MockChain::default();
    let end = json!({"checkpoint":"30","eventOffset":"0"});
    chain.0.lock().unwrap().pages.extend([
        json!({"events":[{"id":{"txDigest":"event-tx","eventSeq":"0"},"parsedJson":{"order_id":f.first.publish.intent.order_id.to_vec()}}],
            "nextCursor":{"checkpoint":"29","eventOffset":"1"},"hasNextPage":true}),
        json!({"events":[],"nextCursor":end,"hasNextPage":true}),
    ]);
    chain.observations([observation(&f.first, 1, 29, f.started + 1)]);
    replay_account(&f.config, &chain, "buyer-000").unwrap();
    let ledger = f.ledger();
    assert_eq!(ledger.orders[&f.first.id].status, OrderStatus::Open);
    assert_eq!(ledger.credits["explicit-test-credit"].consumed, 1);
    assert_eq!(chain.calls("queryOrder").len(), 1); // Vector event matched the real order.
    let saved: Value = serde_json::from_str(ledger.cursors.values().next().unwrap()).unwrap();
    assert_eq!(saved, end);
    let FileFixture {
        config,
        path,
        first: _,
        started: _,
    } = f;
    let c = config.procurement_config.clone();
    drop(config);
    let mut config = SimulationConfig::default().with_account_store_path(&path);
    config.procurement_config = c;
    let resumed = MockChain::default();
    replay_account(&config, &resumed, "buyer-000").unwrap();
    assert_eq!(resumed.calls("replayEvents")[0]["cursor"], end);
}

fn fill_fixture(label: &str) -> FileFixture {
    FileFixture::seed(label, 1, |store, _, started| {
        let buyer = store.accounts.get_mut("buyer-000").unwrap();
        let l = buyer.procurement.as_mut().unwrap();
        let o = l.orders.values_mut().next().unwrap();
        o.status = OrderStatus::Open;
        o.fee_consumed = true;
        o.object_id = Some(encode_hex(&[8; 32]));
        l.credits.get_mut(&o.fee_batch).unwrap().reserved = 0;
        l.credits.get_mut(&o.fee_batch).unwrap().consumed = 1;
        l.outbox.values_mut().next().unwrap().status = JobStatus::Confirmed;
        let mut seller = AccountRecord::new(character(900, "Seller"));
        seller.saves.get_mut(&900).unwrap().gold = 10;
        let mut i = o.publish.intent.clone();
        i.operation = 2;
        i.fill_id = [42; 32];
        i.nonce = [43; 32];
        i.seller_ref = actor_ref("seller", &seller.saves[&900]);
        i.valid_until_ms = started + 60_000;
        i.actor_public_key = SigningKey::from([8; 32]).verification_key().to_bytes();
        let signed = sign(i, &SigningKey::from([8; 32]));
        // A complete, exact-index carrier with nondefault durability/refinement.
        // Production File validation checks UID custody and carrier identity.
        let item = serde_json::to_string(&json!({"key":"crystal-item-245","name":"DragonSword","icon":48,"slot":0,
            "unique_id":777,"container":"bag1","quantity":1,"description":"original equipment instance",
            "durability_current":12345,"durability_max":34000,"weight":58,"equip_slot":"weapon",
            "added_stats":[{"stat":5,"value":7}],"attack":0,"defence":0,"heal_hp":0,"heal_mp":0,
            "user_item_metadata":{"item_index":245,"refine_added":2,"wedding_ring":-1}})).unwrap();
        let fill = hex(&signed.intent.fill_id);
        o.offers.insert(fill.clone(), "seller".into());
        let mut sl = ProcurementLedger::default();
        sl.outbox.insert(
            format!("fill:{fill}"),
            task(
                format!("fill:{fill}"),
                &o.id,
                JobKind::Fill,
                signed.clone(),
                digest(item.as_bytes()),
            ),
        );
        sl.locks.insert(
            fill.clone(),
            EquipmentLock {
                id: fill,
                order_id: o.id.clone(),
                buyer_account: "buyer-000".into(),
                seller_character: 900,
                seller_name: "Seller".into(),
                signed,
                original_item_json: item.clone(),
                item_uid: 777,
                instance_hash: digest(item.as_bytes()),
                fence: 0,
                released: false,
            },
        );
        seller.procurement = Some(sl);
        seller.billing = Some(Default::default());
        store.accounts.insert("seller".into(), seller);
    })
}

#[test]
fn file_worker_fresh_open_fence_confirms_stale_fill_before_any_broadcast() {
    let f = fill_fixture("stale-fill-fence");
    let chain = MockChain::default();
    let mut fresh = observation(&f.first, 1, 40, f.started + 1);
    fresh["fields"]["fence"] = json!("1");
    chain.observations([fresh]);
    tick(&f, &chain).unwrap();
    let disk = f.disk();
    let seller = disk.accounts["seller"].procurement.as_ref().unwrap();
    assert_eq!(
        seller.outbox.values().next().unwrap().status,
        JobStatus::Confirmed
    );
    assert!(seller.locks.values().all(|l| l.released));
    assert_eq!(
        seller.deliveries.values().next().unwrap().item_uid,
        Some(777)
    );
    assert_eq!(f.ledger().orders[&f.first.id].escrow_gold, 500_000);
    assert!(chain.calls("build").is_empty());
    assert!(chain.calls("submitExact").is_empty());
}

#[test]
fn file_worker_final_fill_delivers_original_instance_once_without_rebroadcast() {
    let f = fill_fixture("filled-delivery");
    let chain = MockChain::default();
    let total_gold = |store: &AccountStore| -> u64 {
        store
            .accounts
            .values()
            .map(|a| {
                a.saves.values().map(|s| s.gold as u64).sum::<u64>()
                    + a.procurement.as_ref().map_or(0, |l| {
                        l.orders.values().map(|o| o.escrow_gold as u64).sum::<u64>()
                            + l.deliveries
                                .values()
                                .filter(|d| !d.claimed)
                                .map(|d| d.gold as u64)
                                .sum::<u64>()
                    })
            })
            .sum()
    };
    // File bootstrap can add its default account. Include its actual loaded
    // balance in both snapshots instead of assuming only two accounts exist.
    let initial = f.config.account_store.lock().unwrap().clone();
    let lock = initial.accounts["seller"]
        .procurement
        .as_ref()
        .unwrap()
        .locks
        .values()
        .next()
        .unwrap();
    let mut filled = observation(&f.first, 2, 41, f.started + 1);
    filled["fields"]["fill_id"] = json!(encode_hex(&lock.signed.intent.fill_id));
    filled["fields"]["seller_ref"] = json!(encode_hex(&lock.signed.intent.seller_ref));
    filled["fields"]["asset_instance_hash"] = json!(encode_hex(&lock.instance_hash));
    chain.observations([filled]);
    tick(&f, &chain).unwrap();
    tick(&f, &chain).unwrap();
    let disk = f.disk();
    let buyer = disk.accounts["buyer-000"].procurement.as_ref().unwrap();
    let seller = disk.accounts["seller"].procurement.as_ref().unwrap();
    assert_eq!(buyer.orders[&f.first.id].status, OrderStatus::Filled);
    assert_eq!(buyer.deliveries.len(), 1);
    assert_eq!(
        buyer
            .deliveries
            .values()
            .next()
            .unwrap()
            .original_item_json
            .as_deref(),
        Some(lock.original_item_json.as_str())
    );
    assert_eq!(
        seller
            .deliveries
            .values()
            .map(|d| d.gold as u64)
            .sum::<u64>(),
        500_000
    );
    assert_eq!(buyer.credits["explicit-test-credit"].consumed, 1);
    assert_eq!(total_gold(&disk), total_gold(&initial));
    assert!(seller
        .outbox
        .values()
        .all(|j| j.status == JobStatus::Confirmed));
    assert!(chain.calls("build").is_empty());
    assert!(chain.calls("submitExact").is_empty());
}
