use super::*;
use ed25519_consensus::SigningKey;
use mir2_protocol::{procurement as wire, MirClass, MirGender};
fn character(index: i32, name: &str) -> crate::CharacterRecord {
    crate::CharacterRecord {
        index,
        name: name.into(),
        level: 22,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    }
}
fn config() -> ProcurementConfig {
    ProcurementConfig {
        enabled: true,
        allow_test_credits: true,
        domain: Domain {
            network_hash: [1; 32],
            package_id: [2; 32],
            dapp_storage_id: [3; 32],
            oracle_key_id: 1,
        },
        ..Default::default()
    }
}
fn fixture() -> (AccountStore, ProcurementConfig, SigningKey) {
    let mut s = AccountStore::new(character(0, "Buyer"));
    let mut a = s.accounts.remove("demo").unwrap();
    a.saves.get_mut(&0).unwrap().gold = 1_000_000;
    let key = SigningKey::from([7; 32]);
    let pk = key.verification_key().to_bytes();
    let mut l = ProcurementLedger::default();
    l.bindings.insert(
        hex(&pk),
        ActorBinding {
            character_index: 0,
            character_name: "Buyer".into(),
            public_key: pk,
            valid_until_ms: 999999,
        },
    );
    l.credits.insert(
        "test".into(),
        CreditBatch {
            id: "test".into(),
            source: CreditSource::Test,
            granted: 10,
            reserved: 0,
            consumed: 0,
            expires_at_ms: 999999,
            payment_order_id: None,
        },
    );
    a.procurement = Some(l);
    a.billing = Some(Default::default());
    s.accounts.insert("buyer".into(), a);
    for (id, index) in [("seller", 1), ("other", 2)] {
        let mut a = AccountRecord::new(character(index, id));
        a.saves.get_mut(&index).unwrap().gold = 10;
        let mut l = ProcurementLedger::default();
        l.bindings.insert(
            hex(&pk),
            ActorBinding {
                character_index: index,
                character_name: id.into(),
                public_key: pk,
                valid_until_ms: 999999,
            },
        );
        a.procurement = Some(l);
        a.billing = Some(Default::default());
        s.accounts.insert(id.into(), a);
    }
    (s, config(), key)
}
fn intent(s: &AccountStore, c: &ProcurementConfig, k: &SigningKey) -> SignedIntent {
    let save = &s.accounts["buyer"].saves[&0];
    let i = IntentV1 {
        network_hash: c.domain.network_hash,
        package_id: c.domain.package_id,
        dapp_storage_id: c.domain.dapp_storage_id,
        operation: 1,
        order_id: [4; 32],
        fill_id: [0; 32],
        buyer_ref: actor_ref("buyer", save),
        seller_ref: [0; 32],
        template_id: 245,
        conditions_hash: digest(
            &wire::EquipmentConditions::default()
                .canonical_bcs()
                .unwrap(),
        ),
        quantity: 1,
        price_gold: 500000,
        expires_at_ms: 90000,
        rule_version: c.rule.version,
        nonce: [5; 32],
        valid_until_ms: 50000,
        actor_public_key: k.verification_key().to_bytes(),
    };
    sign(i, k)
}
fn sign(i: IntentV1, k: &SigningKey) -> SignedIntent {
    let sig = k.sign(&i.hash()).to_bytes().to_vec();
    SignedIntent {
        intent: i,
        signature: sig,
    }
}
fn tx<T>(
    s: &mut AccountStore,
    f: impl FnOnce(&mut AccountStore) -> Result<T, String>,
) -> Result<T, String> {
    let mut staged = s.clone();
    let t = f(&mut staged)?;
    for a in staged.accounts.values() {
        validate_account(a)?
    }
    validate_store(&staged)?;
    *s = staged;
    Ok(t)
}
fn publish_one(s: &mut AccountStore, c: &ProcurementConfig, k: &SigningKey) -> SignedIntent {
    let signed = intent(s, c, k);
    tx(s, |s| {
        let mut save = s.accounts["buyer"].saves[&0].clone();
        publish(
            s,
            c,
            "buyer",
            &mut save,
            "publish1",
            &signed,
            Default::default(),
            1000,
        )?;
        s.accounts.get_mut("buyer").unwrap().saves.insert(0, save);
        Ok(())
    })
    .unwrap();
    signed
}
fn receipt(
    c: &ProcurementConfig,
    signed: &SignedIntent,
    status: u8,
    sequence: u64,
) -> ChainReceipt {
    let i = &signed.intent;
    ChainReceipt {
        network_hash: c.domain.network_hash,
        package_id: c.domain.package_id,
        dapp_storage_id: c.domain.dapp_storage_id,
        order_id: i.order_id,
        object_id: "0xorder".into(),
        status,
        fill_id: i.fill_id,
        buyer_ref: i.buyer_ref,
        seller_ref: i.seller_ref,
        template_id: i.template_id,
        conditions_hash: i.conditions_hash,
        price_gold: i.price_gold,
        rule_version: i.rule_version,
        asset_instance_hash: [0; 32],
        fence: 0,
        checkpoint: 10 + sequence,
        digest: format!("transaction-{sequence}"),
        event_seq: 0,
        chain_timestamp_ms: 2000 + sequence,
    }
}
fn fill(
    s: &mut AccountStore,
    c: &ProcurementConfig,
    k: &SigningKey,
    p: &SignedIntent,
    seller: &str,
    n: u8,
) -> (SignedIntent, String) {
    let mut i = p.intent.clone();
    i.operation = 2;
    i.fill_id = [n; 32];
    i.nonce = [n + 1; 32];
    let save = s.accounts[seller].saves.values().next().unwrap().clone();
    i.seller_ref = actor_ref(seller, &save);
    let signed = sign(i, k);
    let json = crate::runtime::procurement::test_item(n as u64 + 100);
    tx(s, |s| {
        offer(
            s,
            c,
            seller,
            "buyer",
            &save,
            &format!("fill{n}"),
            &signed,
            json.clone(),
            n as u64 + 100,
            2500,
        )
    })
    .unwrap();
    (signed, json)
}
fn total_gold(s: &AccountStore) -> u64 {
    s.accounts
        .values()
        .map(|a| {
            a.saves.values().map(|x| x.gold as u64).sum::<u64>()
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
}
#[test]
fn procurement_normal_fill_preserves_instance_and_conserves_gold() {
    let (mut s, c, k) = fixture();
    let initial = total_gold(&s);
    let p = publish_one(&mut s, &c, &k);
    assert_eq!(s.accounts["buyer"].saves[&0].gold, 500000);
    tx(&mut s, |s| {
        apply_receipt(s, &c, "buyer", &receipt(&c, &p, 1, 1))
    })
    .unwrap();
    let (f, json) = fill(&mut s, &c, &k, &p, "seller", 8);
    let mut r = receipt(&c, &f, 2, 2);
    r.asset_instance_hash = digest(json.as_bytes());
    tx(&mut s, |s| apply_receipt(s, &c, "buyer", &r)).unwrap();
    let l = s.accounts["buyer"].procurement.as_ref().unwrap();
    assert_eq!(
        l.deliveries[&format!("item:{}", hex(&p.intent.order_id))]
            .original_item_json
            .as_deref(),
        Some(json.as_str())
    );
    assert_eq!(total_gold(&s), initial);
    assert_eq!(l.credits["test"].consumed, 1);
    tx(&mut s, |s| apply_receipt(s, &c, "buyer", &r)).unwrap();
    assert_eq!(total_gold(&s), initial);
}
#[test]
fn procurement_insufficient_gold_and_quota_atomic() {
    for gold_fail in [true, false] {
        let (mut s, c, k) = fixture();
        if gold_fail {
            s.accounts
                .get_mut("buyer")
                .unwrap()
                .saves
                .get_mut(&0)
                .unwrap()
                .gold = 1
        } else {
            s.accounts
                .get_mut("buyer")
                .unwrap()
                .procurement
                .as_mut()
                .unwrap()
                .credits
                .clear()
        }
        let before = serde_json::to_vec(&s).unwrap();
        let p = intent(&s, &c, &k);
        assert!(tx(&mut s, |s| {
            let mut save = s.accounts["buyer"].saves[&0].clone();
            publish(s, &c, "buyer", &mut save, "p", &p, Default::default(), 1000)
        })
        .is_err());
        assert_eq!(serde_json::to_vec(&s).unwrap(), before)
    }
}
#[test]
fn procurement_forgery_and_cross_environment_replay_rejected() {
    let (s, c, k) = fixture();
    let p = intent(&s, &c, &k);
    for kind in 0..4 {
        let mut forged = p.clone();
        match kind {
            0 => forged.intent.price_gold += 1,
            1 => forged.intent.package_id = [9; 32],
            2 => forged.intent.quantity = 2,
            _ => forged.signature[0] ^= 1,
        };
        assert!(forged.verify().is_err())
    }
    let mut other = c.clone();
    other.domain.network_hash = [9; 32];
    let save = s.accounts["buyer"].saves[&0].clone();
    assert!(verify_intent(&other, "buyer", &save, &p, 1, 1000).is_err());
    assert!(verify_intent(&c, "seller", &save, &p, 1, 1000).is_err());
}
#[test]
fn procurement_wrong_bound_equipped_conditions_rejected() {
    let json = crate::runtime::procurement::test_item(88);
    assert!(crate::runtime::procurement::test_eligible(&json, &Default::default()).is_ok());
    for key in [
        "key",
        "soul_bound_id",
        "rental_locked",
        "container",
        "quantity",
    ] {
        let mut j: serde_json::Value = serde_json::from_str(&json).unwrap();
        j[key] = match key {
            "key" => serde_json::json!("crystal-item-244"),
            "soul_bound_id" => serde_json::json!(9),
            "rental_locked" => serde_json::json!(true),
            "container" => serde_json::json!("Equipment"),
            _ => serde_json::json!(2),
        };
        assert!(
            crate::runtime::procurement::test_eligible(&j.to_string(), &Default::default())
                .is_err()
        );
    }
    let c = wire::EquipmentConditions {
        min_current_durability: 20000,
        ..Default::default()
    };
    assert!(crate::runtime::procurement::test_eligible(&json, &c).is_err());
}
#[test]
fn procurement_concurrent_fills_only_one_winner() {
    let (mut s, c, k) = fixture();
    let initial = total_gold(&s);
    let p = publish_one(&mut s, &c, &k);
    tx(&mut s, |s| {
        apply_receipt(s, &c, "buyer", &receipt(&c, &p, 1, 1))
    })
    .unwrap();
    let (f, json) = fill(&mut s, &c, &k, &p, "seller", 8);
    let (g, other) = fill(&mut s, &c, &k, &p, "other", 9);
    let mut r = receipt(&c, &f, 2, 2);
    r.asset_instance_hash = digest(json.as_bytes());
    let mut losing = receipt(&c, &g, 2, 3);
    losing.asset_instance_hash = digest(other.as_bytes());
    let shared = std::sync::Arc::new(std::sync::Mutex::new(s));
    let handles = [r, losing]
        .into_iter()
        .map(|r| {
            let s = shared.clone();
            let c = c.clone();
            std::thread::spawn(move || {
                tx(&mut s.lock().unwrap(), |s| {
                    apply_receipt(s, &c, "buyer", &r)
                })
                .is_ok()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|x| *x)
            .count(),
        1
    );
    assert_eq!(total_gold(&shared.lock().unwrap()), initial);
}
#[test]
fn procurement_cancel_fill_mutually_exclusive() {
    let (mut s, c, k) = fixture();
    let p = publish_one(&mut s, &c, &k);
    tx(&mut s, |s| {
        apply_receipt(s, &c, "buyer", &receipt(&c, &p, 1, 1))
    })
    .unwrap();
    let (f, json) = fill(&mut s, &c, &k, &p, "seller", 8);
    tx(&mut s, |s| {
        apply_receipt(s, &c, "buyer", &receipt(&c, &p, 3, 4))
    })
    .unwrap();
    let before = serde_json::to_vec(&s).unwrap();
    let mut r = receipt(&c, &f, 2, 5);
    r.asset_instance_hash = digest(json.as_bytes());
    assert!(tx(&mut s, |s| apply_receipt(s, &c, "buyer", &r)).is_err());
    assert_eq!(serde_json::to_vec(&s).unwrap(), before);
}
#[test]
fn procurement_expiry_requires_chain_clock() {
    let (mut s, c, k) = fixture();
    let p = publish_one(&mut s, &c, &k);
    tx(&mut s, |s| {
        apply_receipt(s, &c, "buyer", &receipt(&c, &p, 1, 1))
    })
    .unwrap();
    let mut r = receipt(&c, &p, 4, 4);
    assert!(tx(&mut s, |s| apply_receipt(s, &c, "buyer", &r)).is_err());
    r.chain_timestamp_ms = p.intent.expires_at_ms;
    tx(&mut s, |s| apply_receipt(s, &c, "buyer", &r)).unwrap();
    assert_eq!(
        s.accounts["buyer"].procurement.as_ref().unwrap().credits["test"].consumed,
        1
    )
}
#[test]
fn procurement_publish_failure_waits_proof_deadline() {
    let (mut s, c, k) = fixture();
    let initial = total_gold(&s);
    let p = publish_one(&mut s, &c, &k);
    let mut r = receipt(&c, &p, 0, 1);
    tx(&mut s, |s| apply_receipt(s, &c, "buyer", &r)).unwrap();
    assert_eq!(
        s.accounts["buyer"].procurement.as_ref().unwrap().credits["test"].reserved,
        1
    );
    r.digest = "final-clock".into();
    r.chain_timestamp_ms = p.intent.valid_until_ms + 1;
    tx(&mut s, |s| apply_receipt(s, &c, "buyer", &r)).unwrap();
    let l = s.accounts["buyer"].procurement.as_ref().unwrap();
    assert_eq!(l.credits["test"].reserved, 0);
    assert_eq!(l.credits["test"].consumed, 0);
    assert_eq!(total_gold(&s), initial)
}
#[test]
fn procurement_crash_unknown_does_not_unlock() {
    let (mut s, c, k) = fixture();
    let p = publish_one(&mut s, &c, &k);
    let id = format!("publish:{}", hex(&p.intent.order_id));
    s.accounts
        .get_mut("buyer")
        .unwrap()
        .procurement
        .as_mut()
        .unwrap()
        .outbox
        .get_mut(&id)
        .unwrap()
        .status = JobStatus::Unknown;
    let restored: AccountStore = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
    assert_eq!(restored.accounts["buyer"].saves[&0].gold, 500000);
    assert_eq!(
        restored.accounts["buyer"]
            .procurement
            .as_ref()
            .unwrap()
            .orders[&hex(&p.intent.order_id)]
            .escrow_gold,
        500000
    );
}
#[test]
fn procurement_disabled_changed_rule_keeps_old_cancel() {
    let (mut s, c, k) = fixture();
    let p = publish_one(&mut s, &c, &k);
    tx(&mut s, |s| {
        apply_receipt(s, &c, "buyer", &receipt(&c, &p, 1, 1))
    })
    .unwrap();
    let mut c = c;
    c.enabled = false;
    c.rule.version = 2;
    let mut i = p.intent.clone();
    i.operation = 3;
    i.nonce = [18; 32];
    let signed = sign(i, &k);
    let save = s.accounts["buyer"].saves[&0].clone();
    tx(&mut s, |s| {
        cancel(s, &c, "buyer", &save, "cancel", &signed, 3000)
    })
    .unwrap();
    tx(&mut s, |s| {
        apply_receipt(s, &c, "buyer", &receipt(&c, &p, 3, 3))
    })
    .unwrap();
}
#[test]
fn procurement_old_schema_rejects_new_authority() {
    let (mut s, _, _) = fixture();
    s.schema_version = 10;
    assert!(crate::billing::validate_complete_store(&s).is_err())
}
#[test]
fn procurement_rust_sdk_move_fixed_vector_bytes_and_signatures() {
    let v: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../onchain/procurement/fixtures/intent-v1.json"
    ))
    .unwrap();
    let w: wire::IntentV1 = serde_json::from_value(v["intent"].clone()).unwrap();
    let i = IntentV1::try_from(&w).unwrap();
    assert_eq!(
        wire::encode_hex(&i.bcs_bytes()),
        v["canonical_intent_hex"].as_str().unwrap()
    );
    assert_eq!(
        wire::encode_hex(&i.hash()),
        v["intent_sha256"].as_str().unwrap()
    );
    SignedIntent {
        intent: i,
        signature: wire::decode_hex(v["actor_signature"].as_str().unwrap()).unwrap(),
    }
    .verify()
    .unwrap();
    let p = &v["proof"];
    let h = |k: &str| wire::hex32(p[k].as_str().unwrap()).unwrap();
    let d = |k: &str| wire::decimal(p[k].as_str().unwrap()).unwrap();
    let proof = OracleProofV1 {
        intent_hash: h("intent_hash"),
        asset_lock_id: h("asset_lock_id"),
        asset_instance_hash: h("asset_instance_hash"),
        oracle_key_id: d("oracle_key_id"),
        valid_until_ms: d("valid_until_ms"),
        fence: d("fence"),
    };
    assert_eq!(
        wire::encode_hex(&proof.bcs_bytes()),
        v["canonical_proof_hex"].as_str().unwrap()
    );
    assert_eq!(
        wire::encode_hex(&proof.hash()),
        v["proof_sha256"].as_str().unwrap()
    );
    let key = ed25519_consensus::VerificationKey::try_from(
        wire::hex32(v["oracle_public_key"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    let sig = ed25519_consensus::Signature::try_from(
        wire::decode_hex(v["oracle_signature"].as_str().unwrap())
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    key.verify(&sig, &proof.hash()).unwrap();
}
fn durable_fixture(label: &str) -> (SimulationConfig, ProcurementConfig, SigningKey) {
    let (s, c, k) = fixture();
    let path = std::env::temp_dir().join(format!(
        "mir2-procurement-{label}-{}-{}.json",
        std::process::id(),
        hex(&digest(
            format!("{:?}", std::time::SystemTime::now()).as_bytes()
        ))
    ));
    let config = SimulationConfig::default().with_account_store_path(path);
    config
        .commit_account_store_transaction(
            &[
                "demo".into(),
                "buyer".into(),
                "seller".into(),
                "other".into(),
            ],
            |store| {
                store.accounts = s.accounts.clone();
                Ok(())
            },
        )
        .unwrap();
    (config, c, k)
}
fn durable_publish(
    config: &SimulationConfig,
    c: &ProcurementConfig,
    k: &SigningKey,
) -> Result<SignedIntent, String> {
    config.commit_account_store_transaction(&["buyer".into()], |s| {
        let p = intent(s, c, k);
        let mut save = s.accounts["buyer"].saves[&0].clone();
        publish(
            s,
            c,
            "buyer",
            &mut save,
            "publish1",
            &p,
            Default::default(),
            1000,
        )?;
        s.accounts.get_mut("buyer").unwrap().saves.insert(0, save);
        Ok(p)
    })
}
#[test]
fn procurement_real_file_failed_commit_rolls_back_gold_quota_and_outbox() {
    let (config, c, k) = durable_fixture("rollback");
    let before =
        serde_json::to_vec(&config.account_store.lock().unwrap().accounts["buyer"]).unwrap();
    config.inject_account_store_transaction_fault(
        crate::AccountStoreTransactionFault::BeforeFileRename,
    );
    assert!(durable_publish(&config, &c, &k).is_err());
    assert_eq!(
        serde_json::to_vec(&config.account_store.lock().unwrap().accounts["buyer"]).unwrap(),
        before
    );
    durable_publish(&config, &c, &k).unwrap();
    assert_eq!(
        config.procurement_view("buyer").unwrap().credits["test"].reserved,
        1
    );
}
#[test]
fn procurement_real_file_commit_unknown_freezes_and_keeps_durable_outbox() {
    let (config, c, k) = durable_fixture("unknown");
    config.inject_account_store_transaction_fault(
        crate::AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync,
    );
    let e = durable_publish(&config, &c, &k).unwrap_err();
    assert!(e.contains("COMMIT_OUTCOME_UNKNOWN"), "{e}");
    assert!(config.ensure_account_store_writable().is_err());
    let bytes = std::fs::read(config.account_store_path.as_ref().unwrap()).unwrap();
    let disk: AccountStore = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(disk.accounts["buyer"].saves[&0].gold, 500000);
    assert_eq!(
        disk.accounts["buyer"]
            .procurement
            .as_ref()
            .unwrap()
            .outbox
            .len(),
        1
    );
    assert!(durable_publish(&config, &c, &k).is_err());
}
#[test]
fn procurement_chain_success_database_failure_reconciles_exactly_once() {
    let (config, c, k) = durable_fixture("reconcile");
    let p = durable_publish(&config, &c, &k).unwrap();
    let r = receipt(&c, &p, 1, 1);
    config.inject_account_store_transaction_fault(
        crate::AccountStoreTransactionFault::BeforeFileRename,
    );
    assert!(config
        .procurement_apply_chain_receipt(&c, "buyer", &r)
        .is_err());
    assert_eq!(
        config.procurement_view("buyer").unwrap().credits["test"].consumed,
        0
    );
    config
        .procurement_apply_chain_receipt_with_cursor(&c, "buyer", &r, "test-finality", "cursor-1")
        .unwrap();
    config
        .procurement_apply_chain_receipt_with_cursor(&c, "buyer", &r, "test-finality", "cursor-1")
        .unwrap();
    let l = config.procurement_view("buyer").unwrap();
    assert_eq!(l.credits["test"].consumed, 1);
    assert_eq!(l.cursors["test-finality"], "cursor-1");
    assert_eq!(l.chain_inbox.len(), 1);
}
#[test]
fn procurement_worker_retains_exact_transaction_across_slow_rpc_and_rejects_expired_owner() {
    let (config, c, k) = durable_fixture("slow-rpc-lease");
    durable_publish(&config, &c, &k).unwrap();
    let jobs = config
        .procurement_claim_outbox("buyer", "first", 2000)
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].lease_until_ms, 122000);
    assert!(config
        .procurement_claim_outbox("buyer", "second", 62001)
        .unwrap()
        .is_empty());
    let prepared = PreparedTransaction {
        step: "create".into(),
        transaction_digest: "slow-rpc-digest".into(),
        signed_transaction_bytes: vec![1, 2, 3],
        oracle_signature: vec![5; 64],
    };
    config
        .procurement_prepare_transaction("buyer", &jobs[0].id, "first", 62001, prepared.clone())
        .unwrap();
    let reclaimed = config
        .procurement_claim_outbox("buyer", "second", 122000)
        .unwrap();
    assert_eq!(reclaimed.len(), 1);
    assert_eq!(reclaimed[0].signed_transactions["create"], prepared);
    assert!(config
        .procurement_record_submission("buyer", &jobs[0].id, "first", 122001, None, None)
        .is_err());
    config
        .procurement_record_submission("buyer", &jobs[0].id, "second", 122001, None, None)
        .unwrap();
    let ledger = config.procurement_view("buyer").unwrap();
    assert_eq!(
        ledger.outbox[&jobs[0].id].signed_transactions["create"],
        prepared
    );
    assert_eq!(ledger.credits["test"].reserved, 1);
    assert_eq!(ledger.credits["test"].consumed, 0);
}
#[test]
fn procurement_signed_transaction_bytes_are_immutable_and_unknown_preserves_them() {
    let (config, c, k) = durable_fixture("signedtx");
    let p = durable_publish(&config, &c, &k).unwrap();
    let jobs = config
        .procurement_claim_outbox("buyer", "worker", 2000)
        .unwrap();
    assert_eq!(jobs.len(), 1);
    let prepared = PreparedTransaction {
        step: "create".into(),
        transaction_digest: "exact-digest".into(),
        signed_transaction_bytes: vec![1, 2, 3],
        oracle_signature: vec![5; 64],
    };
    config
        .procurement_prepare_transaction("buyer", &jobs[0].id, "worker", 2001, prepared.clone())
        .unwrap();
    let mut different = prepared.clone();
    different.signed_transaction_bytes.push(4);
    assert!(config
        .procurement_prepare_transaction("buyer", &jobs[0].id, "worker", 2002, different)
        .is_err());
    config
        .procurement_record_submission("buyer", &jobs[0].id, "worker", 2003, None, None)
        .unwrap();
    let l = config.procurement_view("buyer").unwrap();
    let j = &l.outbox[&format!("publish:{}", hex(&p.intent.order_id))];
    assert_eq!(j.status, JobStatus::Unknown);
    assert_eq!(j.signed_transactions["create"], prepared);
    assert_eq!(
        config.account_store.lock().unwrap().accounts["buyer"].saves[&0].gold,
        500000
    );
}
#[test]
fn procurement_only_finalized_failed_attempt_can_replace_exact_bytes() {
    let (config, c, k) = durable_fixture("failed-attempt");
    let p = durable_publish(&config, &c, &k).unwrap();
    let job = config
        .procurement_claim_outbox("buyer", "worker", 2000)
        .unwrap()
        .remove(0);
    let prepared = PreparedTransaction {
        step: "create".into(),
        transaction_digest: "failed-digest".into(),
        signed_transaction_bytes: vec![1, 2, 3],
        oracle_signature: vec![5; 64],
    };
    config
        .procurement_prepare_transaction("buyer", &job.id, "worker", 2001, prepared.clone())
        .unwrap();
    let mut retry = prepared.clone();
    retry.transaction_digest = "new-digest".into();
    retry.signed_transaction_bytes.push(4);
    assert!(config
        .procurement_prepare_transaction("buyer", &job.id, "worker", 2002, retry.clone())
        .is_err());
    assert!(config
        .procurement_record_finalized_failure("buyer", &job.id, "worker", 2002, "unprepared", 8)
        .is_err());
    assert!(config
        .procurement_record_finalized_failure(
            "buyer",
            &job.id,
            "worker",
            2002,
            &prepared.transaction_digest,
            0
        )
        .is_err());
    assert!(config
        .procurement_record_finalized_failure(
            "buyer",
            &job.id,
            "wrong-worker",
            2002,
            &prepared.transaction_digest,
            8
        )
        .is_err());
    config
        .procurement_record_finalized_failure(
            "buyer",
            &job.id,
            "worker",
            2002,
            &prepared.transaction_digest,
            8,
        )
        .unwrap();
    config
        .procurement_record_finalized_failure(
            "buyer",
            &job.id,
            "worker",
            2002,
            &prepared.transaction_digest,
            8,
        )
        .unwrap();
    assert!(config
        .procurement_record_finalized_failure(
            "buyer",
            &job.id,
            "worker",
            2002,
            &prepared.transaction_digest,
            9
        )
        .is_err());
    let l = config.procurement_view("buyer").unwrap();
    assert_eq!(l.orders[&hex(&p.intent.order_id)].escrow_gold, 500000);
    assert_eq!(l.credits["test"].reserved, 1);
    assert_eq!(l.credits["test"].consumed, 0);
    assert!(l.deliveries.is_empty());
    assert_eq!(l.outbox[&job.id].status, JobStatus::Pending);
    config
        .procurement_claim_outbox("buyer", "worker", 2003)
        .unwrap();
    let mut same_digest_changed_bytes = prepared.clone();
    same_digest_changed_bytes.signed_transaction_bytes.push(9);
    assert!(config
        .procurement_prepare_transaction(
            "buyer",
            &job.id,
            "worker",
            2004,
            same_digest_changed_bytes
        )
        .is_err());
    config
        .procurement_prepare_transaction("buyer", &job.id, "worker", 2004, retry.clone())
        .unwrap();
    let l = config.procurement_view("buyer").unwrap();
    assert_eq!(
        l.outbox[&job.id].failed_transactions[&prepared.transaction_digest],
        8
    );
    assert_eq!(
        l.outbox[&job.id].archived_transactions[&prepared.transaction_digest],
        prepared
    );
    assert_eq!(l.outbox[&job.id].signed_transactions["create"], retry);
    // Restart preserves both exact attempts and the finalized-failure evidence.
    let disk: AccountStore = serde_json::from_slice(
        &std::fs::read(config.account_store_path.as_ref().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        disk.accounts["buyer"].procurement.as_ref().unwrap().outbox[&job.id],
        l.outbox[&job.id]
    );
}
#[test]
fn procurement_late_worker_submission_never_revives_finalized_job() {
    let (config, c, k) = durable_fixture("late-submission");
    let p = durable_publish(&config, &c, &k).unwrap();
    let job = config
        .procurement_claim_outbox("buyer", "worker", 2000)
        .unwrap()
        .remove(0);
    config
        .procurement_apply_chain_receipt(&c, "buyer", &receipt(&c, &p, 1, 1))
        .unwrap();
    config
        .procurement_record_submission("buyer", &job.id, "worker", 2001, Some("late".into()), None)
        .unwrap();
    config
        .procurement_record_submission("buyer", &job.id, "worker", 2002, None, None)
        .unwrap();
    let l = config.procurement_view("buyer").unwrap();
    assert_eq!(l.outbox[&job.id].status, JobStatus::Confirmed);
    assert_eq!(l.credits["test"].consumed, 1);
    assert!(!l.outbox[&job.id]
        .transaction_digests
        .iter()
        .any(|d| d == "late"));
}
#[test]
fn procurement_short_order_authentication_never_outlives_order() {
    let (mut s, c, k) = fixture();
    let mut i = intent(&s, &c, &k).intent;
    i.expires_at_ms = 2000;
    i.valid_until_ms = 2000;
    let signed = sign(i.clone(), &k);
    let mut save = s.accounts["buyer"].saves[&0].clone();
    publish(
        &mut s,
        &c,
        "buyer",
        &mut save,
        "short",
        &signed,
        Default::default(),
        1000,
    )
    .unwrap();
    assert_eq!(save.gold, 500000);
    i.valid_until_ms += 1;
    let malformed = sign(i, &k);
    assert!(verify_intent(&c, "buyer", &save, &malformed, 1, 1000).is_err());
}
#[test]
fn procurement_cancel_proof_reuses_original_gold_lock_and_new_authorization() {
    let (mut s, c, k) = fixture();
    let p = publish_one(&mut s, &c, &k);
    tx(&mut s, |s| {
        apply_receipt(s, &c, "buyer", &receipt(&c, &p, 1, 1))
    })
    .unwrap();
    let mut i = p.intent.clone();
    i.operation = 3;
    i.nonce = [26; 32];
    let cancel_intent = sign(i, &k);
    let save = s.accounts["buyer"].saves[&0].clone();
    tx(&mut s, |s| {
        cancel(
            s,
            &c,
            "buyer",
            &save,
            "cancel-original-lock",
            &cancel_intent,
            3000,
        )
    })
    .unwrap();
    let l = s.accounts["buyer"].procurement.as_ref().unwrap();
    let original = l
        .outbox
        .values()
        .find(|j| j.kind == JobKind::Publish)
        .unwrap()
        .proof
        .as_ref()
        .unwrap();
    let cancellation = l
        .outbox
        .values()
        .find(|j| j.kind == JobKind::Cancel)
        .unwrap()
        .proof
        .as_ref()
        .unwrap();
    assert_eq!(cancellation.asset_lock_id, original.asset_lock_id);
    assert_eq!(cancellation.intent_hash, cancel_intent.intent.hash());
    assert_ne!(cancellation.intent_hash, p.intent.hash());
}
#[test]
fn procurement_cancel_retry_waits_fresh_chain_expiry_and_keeps_old_attempt() {
    for revoke in [false, true] {
        let (mut s, c, k) = fixture();
        let p = publish_one(&mut s, &c, &k);
        tx(&mut s, |s| {
            apply_receipt(s, &c, "buyer", &receipt(&c, &p, 1, 1))
        })
        .unwrap();
        let save = s.accounts["buyer"].saves[&0].clone();
        let mut i = p.intent.clone();
        i.operation = 3;
        i.nonce = [27; 32];
        i.valid_until_ms = 4000;
        let first = sign(i.clone(), &k);
        tx(&mut s, |s| {
            cancel(s, &c, "buyer", &save, "cancel-first", &first, 3000)
        })
        .unwrap();
        let first_id = s.accounts["buyer"]
            .procurement
            .as_ref()
            .unwrap()
            .outbox
            .values()
            .find(|j| j.kind == JobKind::Cancel)
            .unwrap()
            .id
            .clone();
        let original = s.accounts["buyer"].procurement.as_ref().unwrap().outbox[&first_id].clone();
        i.nonce = [28; 32];
        i.valid_until_ms = 8000;
        let second = sign(i, &k);
        assert!(tx(&mut s, |s| cancel(
            s,
            &c,
            "buyer",
            &save,
            "cancel-second",
            &second,
            3500
        ))
        .is_err());
        let mut r = receipt(&c, &p, 1, 2);
        r.chain_timestamp_ms = 4000;
        tx(&mut s, |s| apply_receipt(s, &c, "buyer", &r)).unwrap();
        assert_ne!(
            s.accounts["buyer"].procurement.as_ref().unwrap().outbox[&first_id].status,
            JobStatus::Confirmed
        );
        if revoke {
            r.fence = 1;
        } else {
            r.chain_timestamp_ms = 4001;
        }
        r.checkpoint += 1;
        tx(&mut s, |s| apply_receipt(s, &c, "buyer", &r)).unwrap();
        tx(&mut s, |s| {
            cancel(s, &c, "buyer", &save, "cancel-second", &second, 4001)
        })
        .unwrap();
        let l = s.accounts["buyer"].procurement.as_ref().unwrap();
        let old = &l.outbox[&first_id];
        assert_eq!(old.status, JobStatus::Confirmed);
        assert_eq!(old.signed, original.signed);
        assert_eq!(old.proof, original.proof);
        assert_eq!(
            l.outbox
                .values()
                .filter(|j| j.kind == JobKind::Cancel)
                .count(),
            2
        );
        let new = l
            .outbox
            .values()
            .find(|j| j.kind == JobKind::Cancel && j.status != JobStatus::Confirmed)
            .unwrap();
        assert_ne!(old.id, new.id);
        assert_eq!(new.proof.as_ref().unwrap().fence, r.fence);
        assert_eq!(
            old.proof.as_ref().unwrap().asset_lock_id,
            new.proof.as_ref().unwrap().asset_lock_id
        );
        assert_eq!(l.credits["test"].consumed, 1);
        assert_eq!(l.orders[&hex(&p.intent.order_id)].escrow_gold, 500000);
    }
}
#[test]
fn procurement_gift_unknown_and_expired_quota_are_not_paid_service() {
    for kind in 0..3 {
        let (mut s, c, k) = fixture();
        let b = s
            .accounts
            .get_mut("buyer")
            .unwrap()
            .procurement
            .as_mut()
            .unwrap()
            .credits
            .get_mut("test")
            .unwrap();
        match kind {
            0 => b.source = CreditSource::Gift,
            1 => b.source = CreditSource::Unknown,
            _ => b.expires_at_ms = 1,
        };
        let signed = intent(&s, &c, &k);
        let mut save = s.accounts["buyer"].saves[&0].clone();
        assert_eq!(
            publish(
                &mut s,
                &c,
                "buyer",
                &mut save,
                "p",
                &signed,
                Default::default(),
                1000
            )
            .unwrap_err(),
            "procurementServiceCreditsInsufficient"
        );
    }
}
#[test]
fn procurement_weak_purpose_key_cannot_authorize_an_intent() {
    let (s, c, k) = fixture();
    let mut p = intent(&s, &c, &k);
    p.intent.actor_public_key = [0; 32];
    let mut identity = vec![0; 64];
    identity[0] = 1;
    p.signature = identity;
    assert!(p.verify().is_err());
}
#[test]
fn procurement_open_watch_survives_offline_until_terminal() {
    let (mut s, c, k) = fixture();
    let p = publish_one(&mut s, &c, &k);
    tx(&mut s, |s| {
        apply_receipt(s, &c, "buyer", &receipt(&c, &p, 1, 1))
    })
    .unwrap();
    let id = hex(&p.intent.order_id);
    assert_eq!(
        s.accounts["buyer"].procurement.as_ref().unwrap().outbox[&format!("watch:{id}")].status,
        JobStatus::Pending
    );
    let mut r = receipt(&c, &p, 4, 9);
    r.chain_timestamp_ms = p.intent.expires_at_ms;
    tx(&mut s, |s| apply_receipt(s, &c, "buyer", &r)).unwrap();
    assert!(s.accounts["buyer"]
        .procurement
        .as_ref()
        .unwrap()
        .outbox
        .values()
        .all(|j| j.status == JobStatus::Confirmed));
}
#[test]
fn procurement_old_schema10_and11_billing_fence_predicate_rejects_new_store() {
    let (s, _, _) = fixture();
    assert_eq!(s.schema_version, crate::billing::BILLING_SCHEMA_VERSION);
    assert!(s.accounts.values().any(|a| a.billing.is_some()));
    for a in s.accounts.values() {
        if a.billing.is_some() {
            for old_schema in [10, 11] {
                assert_ne!(
                    s.schema_version, old_schema,
                    "old validate_complete_store would reject billingSchemaInvalid"
                );
            }
        }
    }
    let schema = crate::MIGRATIONS
        .iter()
        .find(|(v, _)| *v == "0018_procurement_authority_guard")
        .unwrap()
        .1;
    assert!(schema.contains("BEFORE UPDATE OF raw_json OR DELETE"));
    assert!(schema.contains("ERASE") || schema.contains("erased"));
}
#[test]
fn procurement_disabled_config_keeps_recovery_domain_and_rejects_partial_config() {
    let vars = BTreeMap::from([
        ("MIR2_PROCUREMENT_ENABLED", "0".to_owned()),
        ("MIR2_PROCUREMENT_NETWORK_HASH", wire::encode_hex(&[1; 32])),
        ("MIR2_PROCUREMENT_PACKAGE_ID", wire::encode_hex(&[2; 32])),
        (
            "MIR2_PROCUREMENT_DAPP_STORAGE_ID",
            wire::encode_hex(&[3; 32]),
        ),
        ("MIR2_PROCUREMENT_ORACLE_KEY_ID", "1".to_owned()),
    ]);
    let c = ProcurementConfig::from_values(|k| vars.get(k).cloned()).unwrap();
    assert!(!c.enabled);
    assert_eq!(c.domain.package_id, [2; 32]);
    assert!(
        ProcurementConfig::from_values(|k| if k == "MIR2_PROCUREMENT_PACKAGE_ID" {
            Some(wire::encode_hex(&[2; 32]))
        } else {
            None
        })
        .is_err()
    );
    assert!(!ProcurementConfig::from_values(|_| None).unwrap().enabled);
}
fn procurement_runtime_vars() -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        ("MIR2_PROCUREMENT_ENABLED", "1".into()),
        ("MIR2_PROCUREMENT_TEST_CREDITS", "1".into()),
        (
            "MIR2_PROCUREMENT_NETWORK_HASH",
            wire::encode_hex(&digest(b"4c78adac")),
        ),
        ("MIR2_PROCUREMENT_PACKAGE_ID", wire::encode_hex(&[2; 32])),
        (
            "MIR2_PROCUREMENT_DAPP_STORAGE_ID",
            wire::encode_hex(&[3; 32]),
        ),
        ("MIR2_PROCUREMENT_ORACLE_KEY_ID", "1".into()),
    ])
}
#[test]
fn procurement_playtest_gate_normalizes_all_three_markers_and_never_allows_production() {
    for flag in [None, Some("0"), Some("false"), Some("1"), Some("true")] {
        for alias in [
            "staging",
            "stage",
            "preprod",
            "preproduction",
            "production",
            "prod",
        ] {
            for marker in ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV"] {
                let mut vars = procurement_runtime_vars();
                vars.insert(marker, format!(" \t{}  ", alias.to_ascii_uppercase()));
                if let Some(flag) = flag {
                    vars.insert("MIR2_PLAYTEST_TESTNET", flag.into());
                }
                let result = ProcurementConfig::from_values(|key| vars.get(key).cloned());
                let expected =
                    matches!(flag, Some("1" | "true")) && !matches!(alias, "production" | "prod");
                assert_eq!(result.is_ok(), expected, "{marker}={alias}, flag {flag:?}");
                if let Ok(config) = result {
                    assert!(config.allow_test_credits);
                    assert!(!config.production_charging_enabled);
                }
            }
        }
    }
}
#[test]
fn procurement_playtest_flag_rejects_implicit_or_unreadable_opt_in() {
    for flag in ["", "yes", "TRUE", "FALSE", " 1", "true ", "2"] {
        let mut vars = procurement_runtime_vars();
        vars.insert("MIR2_PLAYTEST_TESTNET", flag.into());
        assert_eq!(
            ProcurementConfig::from_values(|key| vars.get(key).cloned()).unwrap_err(),
            "procurementPlaytestFlagInvalid"
        );
    }
    let vars = procurement_runtime_vars();
    assert!(ProcurementConfig::from_values_with_runtime(
        |key| vars.get(key).cloned(),
        [Ok(None), Ok(None), Ok(None)],
        Err(())
    )
    .is_err());
    for slot in 0..3 {
        let mut markers = [Ok(Some("staging".into())), Ok(None), Ok(None)];
        markers[slot] = Err(());
        assert!(ProcurementConfig::from_values_with_runtime(
            |key| vars.get(key).cloned(),
            markers,
            Ok(Some("true".into()))
        )
        .is_err());
    }
}
#[test]
fn procurement_playtest_gate_rejects_conflicting_production_even_without_free_credits() {
    let mut vars = procurement_runtime_vars();
    vars.insert("MIR2_PROCUREMENT_TEST_CREDITS", "0".into());
    vars.insert("MIR2_PLAYTEST_TESTNET", "1".into());
    vars.insert("MIR2_RUNTIME_ENV", "test".into());
    vars.insert("MIR2_DEPLOYMENT_ENV", "staging".into());
    vars.insert("MIR2_ENV", "  PrOd  ".into());
    assert!(ProcurementConfig::from_values(|key| vars.get(key).cloned()).is_err());
}
#[test]
fn procurement_playtest_opt_in_requires_the_real_testnet_network_domain() {
    let mut vars = procurement_runtime_vars();
    vars.insert("MIR2_ENV", "unknown".into());
    vars.insert("MIR2_PLAYTEST_TESTNET", "true".into());
    assert!(ProcurementConfig::from_values(|key| vars.get(key).cloned()).is_ok());
    for network in ["35834a8a", "localnet", "unknown"] {
        vars.insert(
            "MIR2_PROCUREMENT_NETWORK_HASH",
            wire::encode_hex(&digest(network.as_bytes())),
        );
        assert_eq!(
            ProcurementConfig::from_values(|key| vars.get(key).cloned()).unwrap_err(),
            "procurementPlaytestTestnetRequired"
        );
    }
    // Local isolated fixtures retain their existing explicit domain without
    // acquiring public staging permission from an absent flag.
    vars.remove("MIR2_PLAYTEST_TESTNET");
    assert!(ProcurementConfig::from_values(|key| vars.get(key).cloned()).is_ok());
}
#[test]
fn procurement_disabled_playtest_recovery_preserves_domain_and_never_enables_charging() {
    let mut vars = procurement_runtime_vars();
    vars.insert("MIR2_PROCUREMENT_ENABLED", "0".into());
    vars.insert("MIR2_PROCUREMENT_TEST_CREDITS", "0".into());
    vars.insert("MIR2_PLAYTEST_TESTNET", "1".into());
    vars.insert("MIR2_ENV", "  StAgInG ".into());
    let config = ProcurementConfig::from_values(|key| vars.get(key).cloned()).unwrap();
    assert!(!config.enabled);
    assert!(!config.allow_test_credits);
    assert!(!config.production_charging_enabled);
    assert_eq!(config.domain.network_hash, digest(b"4c78adac"));
    assert_eq!(config.domain.package_id, [2; 32]);
}
#[test]
fn procurement_absent_phase_one_refunds_only_with_expired_finality_proof() {
    let (config, c, k) = durable_fixture("absence");
    let p = durable_publish(&config, &c, &k).unwrap();
    let mut r = ChainAbsenceReceipt {
        network_hash: c.domain.network_hash,
        package_id: c.domain.package_id,
        dapp_storage_id: c.domain.dapp_storage_id,
        order_id: p.intent.order_id,
        clock_checkpoint: 40,
        clock_digest: "finalized-clock".into(),
        chain_timestamp_ms: p.intent.valid_until_ms,
        registry_version: "9".into(),
        registry_previous_transaction: "registry-final-tx".into(),
        registry_absence: true,
    };
    assert!(config
        .procurement_apply_chain_absence(&c, "buyer", &r)
        .unwrap_err()
        .contains("StillLive"));
    let mut forged = r.clone();
    forged.package_id = [9; 32];
    forged.chain_timestamp_ms += 1;
    assert!(config
        .procurement_apply_chain_absence(&c, "buyer", &forged)
        .is_err());
    r.chain_timestamp_ms += 1;
    config
        .procurement_apply_chain_absence(&c, "buyer", &r)
        .unwrap();
    config
        .procurement_apply_chain_absence(&c, "buyer", &r)
        .unwrap();
    let l = config.procurement_view("buyer").unwrap();
    assert_eq!(l.credits["test"].reserved, 0);
    assert_eq!(l.credits["test"].consumed, 0);
    assert_eq!(l.absence_inbox.len(), 1);
    assert_eq!(
        l.orders[&hex(&p.intent.order_id)].status,
        OrderStatus::PublishFailed
    );
    assert!(l.orders[&hex(&p.intent.order_id)].object_id.is_none());
    assert_eq!(l.deliveries.values().map(|d| d.gold).sum::<u32>(), 500000);
    config
        .procurement_advance_cursor("buyer", "source", "after-unrelated-event")
        .unwrap();
    assert_eq!(
        config.procurement_view("buyer").unwrap().cursors["source"],
        "after-unrelated-event"
    );
}
#[test]
fn procurement_registry_absence_cannot_override_known_created_object() {
    let (config, c, k) = durable_fixture("known-object");
    let p = durable_publish(&config, &c, &k).unwrap();
    config
        .procurement_apply_chain_receipt(&c, "buyer", &receipt(&c, &p, 0, 1))
        .unwrap();
    let r = ChainAbsenceReceipt {
        network_hash: c.domain.network_hash,
        package_id: c.domain.package_id,
        dapp_storage_id: c.domain.dapp_storage_id,
        order_id: p.intent.order_id,
        clock_checkpoint: 40,
        clock_digest: "clock".into(),
        chain_timestamp_ms: p.intent.valid_until_ms + 1,
        registry_version: "9".into(),
        registry_previous_transaction: "root".into(),
        registry_absence: true,
    };
    assert!(config
        .procurement_apply_chain_absence(&c, "buyer", &r)
        .is_err());
    assert_eq!(
        config.procurement_view("buyer").unwrap().credits["test"].reserved,
        1
    );
}
#[test]
fn procurement_actor_reference_survives_character_rename() {
    let (s, _, _) = fixture();
    let mut save = s.accounts["buyer"].saves[&0].clone();
    let before = actor_ref("buyer", &save);
    save.character.name = "RenamedBuyer".into();
    assert_eq!(actor_ref("buyer", &save), before);
    save.character.index += 1;
    assert_ne!(actor_ref("buyer", &save), before);
    assert_ne!(actor_ref("different-account", &save), before);
}
#[test]
fn procurement_free_test_package_requires_confirmation_and_grants_once_per_account_version() {
    let (mut s, c, _) = fixture();
    let before_gold = total_gold(&s);
    let save = s.accounts["buyer"].saves[&0].clone();
    let package = test_quota_package_id(&c.rule);
    let q = tx(&mut s, |s| {
        quote_test_quota(s, &c, "buyer", &save, "quota-quote", &package, 1000)
    })
    .unwrap();
    assert_eq!(q.service_fee.quota_source, "test");
    assert_eq!(q.service_fee.credit_cost, 0);
    assert!(q.intent.is_none() && q.canonical_intent_hex.is_none());
    assert_eq!(
        s.accounts["buyer"]
            .procurement
            .as_ref()
            .unwrap()
            .credits
            .len(),
        1
    );
    assert_eq!(
        tx(&mut s, |s| quote_test_quota(
            s,
            &c,
            "buyer",
            &save,
            "quota-quote",
            &package,
            1500
        ))
        .unwrap(),
        q
    );
    assert!(!tx(&mut s, |s| confirm_test_quota(
        s,
        &c,
        "buyer",
        &save,
        &q.quote_id,
        2000
    ))
    .unwrap());
    assert!(tx(&mut s, |s| confirm_test_quota(
        s,
        &c,
        "buyer",
        &save,
        &q.quote_id,
        2001
    ))
    .unwrap());
    let other = tx(&mut s, |s| {
        quote_test_quota(s, &c, "buyer", &save, "second-quote", &package, 2500)
    })
    .unwrap();
    assert!(tx(&mut s, |s| confirm_test_quota(
        s,
        &c,
        "buyer",
        &save,
        &other.quote_id,
        3000
    ))
    .unwrap());
    let mut disabled = c.clone();
    disabled.enabled = false;
    disabled.allow_test_credits = false;
    assert!(tx(&mut s, |s| confirm_test_quota(
        s,
        &disabled,
        "buyer",
        &save,
        &q.quote_id,
        u64::MAX
    ))
    .unwrap());
    let l = s.accounts["buyer"].procurement.as_ref().unwrap();
    assert_eq!(l.quota_purchases.len(), 1);
    let receipt = &l.quota_purchases[&package];
    assert_eq!(receipt.source, CreditSource::Test);
    assert_eq!(receipt.units, 10);
    assert_eq!(
        l.credits[&receipt.credit_batch_id].source,
        CreditSource::Test
    );
    assert_eq!(l.credits[&receipt.credit_batch_id].granted, 10);
    assert!(l.credits[&receipt.credit_batch_id]
        .payment_order_id
        .is_none());
    assert_eq!(total_gold(&s), before_gold);
    assert!(l.outbox.is_empty());
}
#[test]
fn procurement_concurrent_test_package_confirmations_grant_one_batch() {
    let (mut s, c, _) = fixture();
    let save = s.accounts["buyer"].saves[&0].clone();
    let package = test_quota_package_id(&c.rule);
    let quotes = ["first", "second"].map(|request| {
        tx(&mut s, |s| {
            quote_test_quota(s, &c, "buyer", &save, request, &package, 1000)
        })
        .unwrap()
    });
    let shared = std::sync::Arc::new(std::sync::Mutex::new(s));
    let handles = quotes
        .into_iter()
        .map(|q| {
            let shared = shared.clone();
            let c = c.clone();
            let save = save.clone();
            std::thread::spawn(move || {
                tx(&mut shared.lock().unwrap(), |s| {
                    confirm_test_quota(s, &c, "buyer", &save, &q.quote_id, 2000)
                })
                .unwrap()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|replayed| !replayed)
            .count(),
        1
    );
    let s = shared.lock().unwrap();
    let l = s.accounts["buyer"].procurement.as_ref().unwrap();
    assert_eq!(l.quota_purchases.len(), 1);
    assert_eq!(l.credits.values().map(|b| b.granted).sum::<u32>(), 20); // 10 original + 10 package.
}
#[test]
fn procurement_test_package_production_gates_ownership_expiry_and_rule_snapshot() {
    let (mut s, c, _) = fixture();
    let save = s.accounts["buyer"].saves[&0].clone();
    let package = test_quota_package_id(&c.rule);
    for kind in 0..3 {
        let mut forbidden = c.clone();
        match kind {
            0 => forbidden.enabled = false,
            1 => forbidden.allow_test_credits = false,
            _ => forbidden.production_charging_enabled = true,
        }
        let before = serde_json::to_vec(&s).unwrap();
        assert!(tx(&mut s, |s| quote_test_quota(
            s,
            &forbidden,
            "buyer",
            &save,
            "forbidden",
            &package,
            1000
        ))
        .is_err());
        assert_eq!(serde_json::to_vec(&s).unwrap(), before);
    }
    let q = tx(&mut s, |s| {
        quote_test_quota(s, &c, "buyer", &save, "quota", &package, 1000)
    })
    .unwrap();
    let expiry = s.accounts["buyer"]
        .procurement
        .as_ref()
        .unwrap()
        .quota_quotes[&q.quote_id]
        .valid_until_ms;
    assert!(tx(&mut s, |s| confirm_test_quota(
        s,
        &c,
        "buyer",
        &save,
        &q.quote_id,
        expiry
    ))
    .is_err());
    let mut wrong = save.clone();
    wrong.character.index += 1;
    assert!(tx(&mut s, |s| confirm_test_quota(
        s,
        &c,
        "buyer",
        &wrong,
        &q.quote_id,
        2000
    ))
    .is_err());
    let mut changed = c.clone();
    changed.rule.fee_units = 2;
    assert!(tx(&mut s, |s| quote_test_quota(
        s,
        &changed,
        "buyer",
        &save,
        "reuse-version",
        &package,
        2000
    ))
    .unwrap_err()
    .contains("VersionReused"));
    changed.rule.version = 2;
    assert!(!tx(&mut s, |s| confirm_test_quota(
        s,
        &changed,
        "buyer",
        &save,
        &q.quote_id,
        2000
    ))
    .unwrap());
    let l = s.accounts["buyer"].procurement.as_ref().unwrap();
    assert_eq!(l.quota_purchases[&package].rule, c.rule);
    assert_eq!(l.quota_purchases[&package].units, 10);
}
#[test]
fn procurement_test_package_file_transaction_rollback_and_unknown_commit_are_recoverable() {
    for unknown in [false, true] {
        let (config, c, _) = durable_fixture(if unknown {
            "quota-unknown"
        } else {
            "quota-rollback"
        });
        let save = config.account_store.lock().unwrap().accounts["buyer"].saves[&0].clone();
        let package = test_quota_package_id(&c.rule);
        let q = config
            .commit_account_store_transaction(&["buyer".into()], |s| {
                quote_test_quota(s, &c, "buyer", &save, "quota", &package, 1000)
            })
            .unwrap();
        config.inject_account_store_transaction_fault(if unknown {
            crate::AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync
        } else {
            crate::AccountStoreTransactionFault::BeforeFileRename
        });
        let result = config.commit_account_store_transaction(&["buyer".into()], |s| {
            confirm_test_quota(s, &c, "buyer", &save, &q.quote_id, 2000)
        });
        assert!(result.is_err());
        let disk: AccountStore = serde_json::from_slice(
            &std::fs::read(config.account_store_path.as_ref().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            disk.accounts["buyer"]
                .procurement
                .as_ref()
                .unwrap()
                .quota_purchases
                .len(),
            usize::from(unknown)
        );
        assert_eq!(disk.accounts["buyer"].saves[&0].gold, save.gold);
        if unknown {
            assert!(config.ensure_account_store_writable().is_err());
            let mut restored = disk;
            assert!(
                confirm_test_quota(&mut restored, &c, "buyer", &save, &q.quote_id, 2001).unwrap()
            );
            for a in restored.accounts.values() {
                validate_account(a).unwrap();
            }
        } else {
            config
                .commit_account_store_transaction(&["buyer".into()], |s| {
                    confirm_test_quota(s, &c, "buyer", &save, &q.quote_id, 2001)
                })
                .unwrap();
            assert_eq!(
                config
                    .procurement_view("buyer")
                    .unwrap()
                    .quota_purchases
                    .len(),
                1
            );
        }
    }
}
#[test]
fn procurement_rule_json_is_complete_strict_bounded_and_version_checked() {
    let mut rule = Rule::default();
    rule.version = 7;
    rule.fee_units = 2;
    rule.cancel_fee_refundable = true;
    rule.expiry_fee_refundable = true;
    rule.maximum_lifetime_ms = 86400000;
    rule.maximum_authorization_ms = 60000;
    let json = serde_json::to_string(&rule).unwrap();
    let parse = |text: &str, version: Option<&str>| {
        ProcurementConfig::from_values(|k| match k {
            "MIR2_PROCUREMENT_RULE_JSON" => Some(text.into()),
            "MIR2_PROCUREMENT_RULE_VERSION" => version.map(str::to_owned),
            _ => None,
        })
    };
    assert_eq!(parse(&json, Some("7")).unwrap().rule, rule);
    assert!(parse(&json, Some("8")).is_err());
    let mut value = serde_json::to_value(&rule).unwrap();
    value["extra"] = true.into();
    assert!(parse(&value.to_string(), None).is_err());
    value.as_object_mut().unwrap().remove("extra");
    value.as_object_mut().unwrap().remove("fee_units");
    assert!(parse(&value.to_string(), None).is_err());
    for name in [
        "version",
        "fee_units",
        "maximum_lifetime_ms",
        "maximum_authorization_ms",
    ] {
        let mut value = serde_json::to_value(&rule).unwrap();
        value[name] = 0.into();
        assert!(parse(&value.to_string(), None).is_err());
    }
    let mut value = serde_json::to_value(&rule).unwrap();
    value["maximum_authorization_ms"] = 900001.into();
    assert!(parse(&value.to_string(), None).is_err());
    assert_eq!(test_quota_package_units(&rule).unwrap(), 20);
}
#[test]
fn procurement_configured_fee_and_refund_rule_is_frozen_on_order() {
    let (mut s, mut c, k) = fixture();
    c.rule.version = 4;
    c.rule.fee_units = 2;
    c.rule.cancel_fee_refundable = true;
    c.rule.expiry_fee_refundable = true;
    c.rule.maximum_lifetime_ms = 86400000;
    c.rule.maximum_authorization_ms = 60000;
    let frozen = c.rule.clone();
    let p = publish_one(&mut s, &c, &k);
    tx(&mut s, |s| {
        apply_receipt(s, &c, "buyer", &receipt(&c, &p, 1, 1))
    })
    .unwrap();
    assert_eq!(
        s.accounts["buyer"].procurement.as_ref().unwrap().credits["test"].consumed,
        2
    );
    c.enabled = false;
    c.rule.version = 5;
    c.rule.fee_units = 99;
    c.rule.cancel_fee_refundable = false;
    c.rule.expiry_fee_refundable = false;
    tx(&mut s, |s| {
        apply_receipt(s, &c, "buyer", &receipt(&c, &p, 3, 3))
    })
    .unwrap();
    let l = s.accounts["buyer"].procurement.as_ref().unwrap();
    assert_eq!(l.orders[&hex(&p.intent.order_id)].rule, frozen);
    assert!(l.orders[&hex(&p.intent.order_id)].fee_refunded);
    assert_eq!(l.credits["test"].consumed, 0);
    assert_eq!(
        l.deliveries[&format!("refund:{}", hex(&p.intent.order_id))].gold,
        500000
    );
}
#[test]
fn procurement_schema11_without_new_quota_authority_migrates_and_legacy_quotes_remain_compatible() {
    let (config, c, k) = durable_fixture("schema11-quota-forward");
    let signed = intent(&config.account_store.lock().unwrap(), &c, &k);
    config
        .commit_account_store_transaction(&["buyer".into()], |s| {
            s.accounts
                .get_mut("buyer")
                .unwrap()
                .procurement
                .as_mut()
                .unwrap()
                .quotes
                .insert(
                    "legacy-quote".into(),
                    StoredQuote {
                        public: wire::ProcurementQuote {
                            quote_id: "legacy-quote".into(),
                            operation: "publish".into(),
                            intent: Some(signed.intent.to_wire()),
                            canonical_intent_hex: Some(wire::encode_hex(
                                &signed.intent.bcs_bytes(),
                            )),
                            conditions: Default::default(),
                            item_uid: None,
                            item_description: None,
                            service_fee: Default::default(),
                            quota_package_id: None,
                        },
                        intent: signed.intent.clone(),
                        buyer_account: "buyer".into(),
                        item_uid: None,
                        character_index: 0,
                        character_name: "Buyer".into(),
                        request_id: "legacy-publish".into(),
                        rule: None,
                    },
                );
            Ok(())
        })
        .unwrap();
    let mut value = serde_json::to_value(config.account_store.lock().unwrap().clone()).unwrap();
    value["schemaVersion"] = 11.into();
    // Field is serialized as schemaVersion; keep the check explicit below.
    for account in value["accounts"].as_object_mut().unwrap().values_mut() {
        if let Some(l) = account["procurement"].as_object_mut() {
            l.remove("quota_quotes");
            l.remove("quota_purchases");
        }
    }
    let path = std::env::temp_dir().join(format!(
        "mir2-procurement-schema11-{}.json",
        hex(&digest(
            format!("{:?}", std::time::SystemTime::now()).as_bytes()
        ))
    ));
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let loaded = AccountStore::load_or_new(&path, character(90, "migration-test"));
    assert_eq!(
        loaded.schema_version,
        crate::billing::BILLING_SCHEMA_VERSION
    );
    crate::billing::validate_complete_store(&loaded).unwrap();
    assert!(loaded.accounts["buyer"]
        .procurement
        .as_ref()
        .unwrap()
        .quota_quotes
        .is_empty());
    assert_eq!(
        loaded.accounts["buyer"]
            .procurement
            .as_ref()
            .unwrap()
            .quotes["legacy-quote"]
            .intent,
        signed.intent
    );
}
#[test]
fn procurement_schema11_cannot_claim_new_quota_authority() {
    let (mut s, c, _) = fixture();
    let save = s.accounts["buyer"].saves[&0].clone();
    let package = test_quota_package_id(&c.rule);
    quote_test_quota(&mut s, &c, "buyer", &save, "quota", &package, 1000).unwrap();
    s.schema_version = 11;
    let path = std::env::temp_dir().join(format!(
        "mir2-procurement-false11-{}.json",
        hex(&digest(
            format!("{:?}", std::time::SystemTime::now()).as_bytes()
        ))
    ));
    std::fs::write(&path, serde_json::to_vec(&s).unwrap()).unwrap();
    let loaded = AccountStore::load_or_new(&path, character(90, "migration-test"));
    assert_eq!(loaded.schema_version, 11);
    assert!(crate::billing::validate_complete_store(&loaded).is_err());
}
