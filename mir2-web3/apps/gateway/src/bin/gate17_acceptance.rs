use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_gateway::economy::{
    EconomyBalanceKey, EconomyLeg, EconomyReconciliationReport, EconomyTransactionEnvelope,
    EconomyTransactionKind, PostgresEconomyStore,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gate17Report {
    schema_version: u32,
    generated_at_ms: u64,
    run_id: String,
    assertions: BTreeMap<String, bool>,
    balances: BTreeMap<String, i64>,
    duplicate_receipts: usize,
    recovered_dispatches: usize,
    reconciliation: EconomyReconciliationReport,
    success: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("MIR2_ECONOMY_DATABASE_URL")
        .map_err(|_| "MIR2_ECONOMY_DATABASE_URL is required")?;
    let output = PathBuf::from(
        env::var("MIR2_GATE17_ACCEPTANCE_OUT")
            .unwrap_or_else(|_| "docs/generated/gate17/gate17-acceptance.json".to_string()),
    );
    let now = now_ms();
    let run_id = format!("gate17-{now}");
    let alice = format!("{run_id}-alice");
    let bob = format!("{run_id}-bob");
    let sword = format!("{run_id}-sword-1");
    let consumer = format!("{run_id}-settlement");
    let store = Arc::new(PostgresEconomyStore::new(database_url));
    store.ensure_migrated()?;

    let alice_gold = EconomyBalanceKey::gold(&alice, 0);
    let bob_gold = EconomyBalanceKey::gold(&bob, 0);
    let alice_sword = EconomyBalanceKey::item(&alice, 0, &sword);
    let bob_sword = EconomyBalanceKey::item(&bob, 0, &sword);

    let seed_alice = envelope(
        &run_id,
        "seed-alice",
        EconomyTransactionKind::Adjustment,
        vec![leg(alice_gold.clone(), 100)],
    );
    let seed_bob = envelope(
        &run_id,
        "seed-bob",
        EconomyTransactionKind::Adjustment,
        vec![leg(bob_sword.clone(), 1)],
    );
    let first_seed = store.transact(&seed_alice)?;
    let duplicate_seed = store.transact(&seed_alice)?;
    store.transact(&seed_bob)?;

    let consume = envelope(
        &run_id,
        "consume-gold",
        EconomyTransactionKind::Consume,
        vec![leg(alice_gold.clone(), -10)],
    );
    let consume_receipt = store.transact(&consume)?;
    let duplicate_consume = store.transact(&consume)?;

    let reward = envelope(
        &run_id,
        "reward-gold",
        EconomyTransactionKind::Reward,
        vec![leg(bob_gold.clone(), 5)],
    );
    store.transact(&reward)?;

    let trade = envelope(
        &run_id,
        "trade-gold-for-sword",
        EconomyTransactionKind::Trade,
        vec![
            leg(alice_gold.clone(), -30),
            leg(bob_gold.clone(), 30),
            leg(bob_sword.clone(), -1),
            leg(alice_sword.clone(), 1),
        ],
    );
    let trade_receipt = store.transact(&trade)?;
    let duplicate_trade = store.transact(&trade)?;

    let balances_before_rejected = [
        store.balance(&alice_gold)?,
        store.balance(&bob_gold)?,
        store.balance(&alice_sword)?,
        store.balance(&bob_sword)?,
    ];
    let rejected = envelope(
        &run_id,
        "rejected-overspend",
        EconomyTransactionKind::Consume,
        vec![leg(alice_gold.clone(), -1_000_000)],
    );
    let insufficient_rejected = store.transact(&rejected).is_err();
    let balances_after_rejected = [
        store.balance(&alice_gold)?,
        store.balance(&bob_gold)?,
        store.balance(&alice_sword)?,
        store.balance(&bob_sword)?,
    ];
    let duplicate_unique_item = envelope(
        &run_id,
        "rejected-duplicate-unique-item",
        EconomyTransactionKind::Reward,
        vec![leg(bob_sword.clone(), 1)],
    );
    let duplicate_unique_item_rejected = store.transact(&duplicate_unique_item).is_err();

    let concurrent = envelope(
        &run_id,
        "concurrent-reward",
        EconomyTransactionKind::Reward,
        vec![leg(alice_gold.clone(), 7)],
    );
    let mut concurrent_receipts = Vec::new();
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..8 {
            let store = Arc::clone(&store);
            let concurrent = concurrent.clone();
            handles.push(scope.spawn(move || store.transact(&concurrent)));
        }
        for handle in handles {
            concurrent_receipts.push(handle.join().expect("economy worker thread"));
        }
    });
    let concurrent_receipts = concurrent_receipts
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    let contested_item_id = format!("{run_id}-contested-item");
    let contested_alice = EconomyBalanceKey::item(&alice, 0, &contested_item_id);
    let contested_bob = EconomyBalanceKey::item(&bob, 0, &contested_item_id);
    let contested_rewards = [
        envelope(
            &run_id,
            "contested-item-alice",
            EconomyTransactionKind::Reward,
            vec![leg(contested_alice.clone(), 1)],
        ),
        envelope(
            &run_id,
            "contested-item-bob",
            EconomyTransactionKind::Reward,
            vec![leg(contested_bob.clone(), 1)],
        ),
    ];
    let mut contested_results = Vec::new();
    thread::scope(|scope| {
        let handles = contested_rewards
            .iter()
            .cloned()
            .map(|reward| {
                let store = Arc::clone(&store);
                scope.spawn(move || store.transact(&reward))
            })
            .collect::<Vec<_>>();
        for handle in handles {
            contested_results.push(handle.join().expect("unique item worker thread"));
        }
    });
    let contested_item_commits = contested_results
        .iter()
        .filter(|result| result.is_ok())
        .count();

    let crash_worker = format!("{run_id}-crashed-worker");
    let crash_claim = store.claim_outbox(&crash_worker, now, 100, 10)?;
    if crash_claim.is_empty() {
        return Err("Gate 17 expected pending outbox events".into());
    }
    let crash_event = crash_claim[0].event.clone();
    let first_inbox = store.ingest(&consumer, &crash_event, now)?;
    if first_inbox.duplicate {
        return Err("first Gate 17 inbox delivery unexpectedly duplicated".into());
    }
    // Deliberately do not ACK: this models a dispatcher crash after the sink
    // committed its inbox row but before the source marked the outbox event.
    let recovered_at = now.saturating_add(11);
    let recovered_dispatches = store.dispatch_once(
        &format!("{run_id}-recovery-worker"),
        &consumer,
        recovered_at,
        100,
    )?;

    let dead_letter_asset = EconomyBalanceKey::item(&bob, 0, format!("{run_id}-dlq-token"));
    let dead_letter_envelope = envelope(
        &run_id,
        "dead-letter-redrive",
        EconomyTransactionKind::Reward,
        vec![leg(dead_letter_asset, 1)],
    );
    let dead_letter_receipt = store.transact(&dead_letter_envelope)?;
    let failure_worker = format!("{run_id}-failure-worker");
    let failed_claim = store.claim_outbox(&failure_worker, recovered_at + 1, 1, 5_000)?;
    if failed_claim.len() != 1 || failed_claim[0].event.event_id != dead_letter_receipt.event_id {
        return Err("Gate 17 did not claim the dead-letter test event".into());
    }
    store.mark_delivery_failed(
        &failure_worker,
        &dead_letter_receipt.event_id,
        "synthetic downstream outage",
        recovered_at + 1,
        1,
    )?;
    let dead_letter_detected = store.reconcile(recovered_at + 2)?.dead_letter_count == 1;
    store.redrive_dead_letter(&dead_letter_receipt.event_id, recovered_at + 3)?;
    let redriven = store.dispatch_once(
        &format!("{run_id}-redrive-worker"),
        &consumer,
        recovered_at + 3,
        10,
    )? == 1;

    let all_event_ids = [
        first_seed.event_id.clone(),
        consume_receipt.event_id.clone(),
        trade_receipt.event_id.clone(),
        concurrent.event_id()?,
        dead_letter_receipt.event_id.clone(),
    ];
    let all_dispatched = all_event_ids.iter().all(|event_id| {
        store.outbox_status(event_id).ok().flatten().as_deref() == Some("dispatched")
    });
    let crash_inbox_exactly_once = store.inbox_count(&consumer, &crash_event.event_id)? == 1;
    let reconciliation = store.reconcile(recovered_at.saturating_add(1))?;

    let balances = BTreeMap::from([
        ("aliceGold".to_string(), store.balance(&alice_gold)?),
        ("bobGold".to_string(), store.balance(&bob_gold)?),
        ("aliceSword".to_string(), store.balance(&alice_sword)?),
        ("bobSword".to_string(), store.balance(&bob_sword)?),
    ]);
    let assertions = BTreeMap::from([
        (
            "duplicateSeedDidNotDoubleCredit".to_string(),
            duplicate_seed.duplicate && balances_before_rejected[0] == 60,
        ),
        (
            "duplicateConsumeDidNotDoubleDebit".to_string(),
            duplicate_consume.duplicate,
        ),
        (
            "tradeCommittedAtomically".to_string(),
            balances_before_rejected == [60, 35, 1, 0],
        ),
        (
            "duplicateTradeDidNotReplay".to_string(),
            duplicate_trade.duplicate,
        ),
        (
            "insufficientBalanceRolledBack".to_string(),
            insufficient_rejected && balances_before_rejected == balances_after_rejected,
        ),
        (
            "uniqueItemCannotHaveTwoOwners".to_string(),
            duplicate_unique_item_rejected
                && store.balance(&alice_sword)? == 1
                && store.balance(&bob_sword)? == 0,
        ),
        (
            "concurrentUniqueItemAwardCommittedOnce".to_string(),
            contested_item_commits == 1
                && store.balance(&contested_alice)? + store.balance(&contested_bob)? == 1,
        ),
        (
            "concurrentIdempotencyCommittedOnce".to_string(),
            concurrent_receipts
                .iter()
                .filter(|receipt| !receipt.duplicate)
                .count()
                == 1
                && concurrent_receipts
                    .iter()
                    .filter(|receipt| receipt.duplicate)
                    .count()
                    == 7
                && balances.get("aliceGold") == Some(&67),
        ),
        (
            "crashAfterInboxRecovered".to_string(),
            recovered_dispatches > 0 && crash_inbox_exactly_once,
        ),
        (
            "deadLetterDetectedAndRedriven".to_string(),
            dead_letter_detected && redriven,
        ),
        ("allOutboxDispatched".to_string(), all_dispatched),
        (
            "reconciliationHealthy".to_string(),
            reconciliation.healthy
                && reconciliation.pending_count == 0
                && reconciliation.expired_delivery_count == 0
                && reconciliation.dead_letter_count == 0
                && reconciliation.transaction_without_outbox_count == 0
                && reconciliation.negative_balance_count == 0,
        ),
    ]);
    let success = assertions.values().all(|value| *value);
    let report = Gate17Report {
        schema_version: 1,
        generated_at_ms: now_ms(),
        run_id,
        assertions,
        balances,
        duplicate_receipts: 3 + concurrent_receipts
            .iter()
            .filter(|receipt| receipt.duplicate)
            .count(),
        recovered_dispatches,
        reconciliation,
        success,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &output,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!("Wrote {}", output.display());
    if !report.success {
        std::process::exit(1);
    }
    Ok(())
}

fn envelope(
    run_id: &str,
    key: &str,
    transaction_kind: EconomyTransactionKind,
    legs: Vec<EconomyLeg>,
) -> EconomyTransactionEnvelope {
    EconomyTransactionEnvelope {
        idempotency_key: format!("{run_id}:{key}"),
        transaction_kind,
        zone_id: "mir2-map-0".to_string(),
        fencing_generation: 17,
        source_sequence: 1,
        created_at_ms: now_ms(),
        legs,
        metadata: BTreeMap::new(),
    }
}

fn leg(balance: EconomyBalanceKey, delta: i64) -> EconomyLeg {
    EconomyLeg { balance, delta }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
