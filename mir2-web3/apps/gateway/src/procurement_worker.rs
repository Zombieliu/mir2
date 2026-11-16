//! Durable procurement dispatcher. The shared account store is the authority;
//! the subprocess only constructs/submits whitelisted chain transactions.
use mir2_protocol::procurement::{decode_hex, encode_hex, hex32};
use mir2_simulation::{procurement::*, SimulationConfig};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

const DRIVER: &[u8] = include_bytes!("../../../onchain/procurement/service/driver.mjs");
const CANONICAL: &[u8] = include_bytes!("../../../onchain/procurement/service/canonical.mjs");
const GRPC: &[u8] = include_bytes!("../../../onchain/procurement/service/grpc.mjs");
const DEPENDENCIES: &[u8] = include_bytes!("../../../onchain/procurement/package-lock.json");
const MAX_OUTPUT: u64 = 1024 * 1024;

fn now() -> u64 {
    mir2_simulation::monthly_card::monthly_card_now_ms()
}
fn string<'a>(v: &'a Value, field: &str) -> Result<&'a str, String> {
    v.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("procurementDriverMissing:{field}"))
}
fn integer(v: &Value, field: &str) -> Result<u64, String> {
    mir2_protocol::procurement::decimal(string(v, field)?).map_err(str::to_string)
}
fn bytes32(v: &Value, field: &str) -> Result<[u8; 32], String> {
    hex32(string(v, field)?).map_err(str::to_string)
}
fn check_domain(v: &Value, c: &ProcurementConfig) -> Result<(), String> {
    if v["enabled"] != true
        || v["environmentTag"] != "isolated-test"
        || !matches!(v["network"].as_str(), Some("testnet" | "localnet"))
        || bytes32(v, "networkHash")? != c.domain.network_hash
        || bytes32(v, "packageId")? != c.domain.package_id
        || bytes32(v, "dappStorageId")? != c.domain.dapp_storage_id
        || integer(v, "oracleKeyId")? != c.domain.oracle_key_id
        || c.domain.network_hash == [0; 32]
        || c.domain.package_id == [0; 32]
    {
        return Err("procurementDriverDomainMismatch".into());
    }
    for field in ["frameworkPackageId", "configId", "oraclePublicKey"] {
        bytes32(v, field)?;
    }
    let budget = integer(v, "gasBudgetMist")?;
    if budget == 0
        || budget > integer(v, "maxGasBudgetMist")?
        || budget > integer(v, "dailyGasBudgetMist")?
    {
        return Err("procurementGasBudgetInvalid".into());
    }
    Ok(())
}

pub(crate) trait ChainDriver: Send + Sync {
    fn call(&self, operation: &str, payload: Value) -> Result<Value, String>;
}

struct NodeDriver {
    node: PathBuf,
    script: PathBuf,
    config: Value,
}
impl NodeDriver {
    fn from_env(c: &ProcurementConfig) -> Result<Option<Self>, String> {
        let Some(config_path) = std::env::var_os("MIR2_PROCUREMENT_DRIVER_CONFIG") else {
            return Ok(None);
        };
        let config: Value = serde_json::from_slice(
            &std::fs::read(config_path).map_err(|_| "procurementDriverConfigUnavailable")?,
        )
        .map_err(|_| "procurementDriverConfigInvalid")?;
        check_domain(&config, c)?;
        let node = PathBuf::from(
            std::env::var_os("MIR2_PROCUREMENT_NODE").ok_or("procurementNodePathRequired")?,
        );
        let script = PathBuf::from(
            std::env::var_os("MIR2_PROCUREMENT_DRIVER").ok_or("procurementDriverPathRequired")?,
        );
        if !node.is_absolute() || !node.is_file() || !script.is_absolute() {
            return Err("procurementDriverPathInvalid".into());
        }
        let parent = script.parent().ok_or("procurementDriverPathInvalid")?;
        for (path, expected) in [
            (script.clone(), DRIVER),
            (parent.join("canonical.mjs"), CANONICAL),
            (parent.join("grpc.mjs"), GRPC),
            (
                parent
                    .parent()
                    .ok_or("procurementDriverPathInvalid")?
                    .join("package-lock.json"),
                DEPENDENCIES,
            ),
        ] {
            let actual = std::fs::read(path).map_err(|_| "procurementDriverFileMissing")?;
            // Text sources are pinned independently of Windows Git line endings.
            let normalized = |b: &[u8]| {
                b.iter()
                    .copied()
                    .filter(|b| *b != b'\r')
                    .collect::<Vec<_>>()
            };
            if Sha256::digest(normalized(&actual)) != Sha256::digest(normalized(expected)) {
                return Err("procurementDriverSourceMismatch".into());
            }
        }
        Ok(Some(Self {
            node,
            script,
            config,
        }))
    }
}
impl ChainDriver for NodeDriver {
    fn call(&self, operation: &str, payload: Value) -> Result<Value, String> {
        if !matches!(
            operation,
            "build" | "submitExact" | "queryFinality" | "queryOrder" | "replayEvents" | "readClock"
        ) {
            return Err("procurementDriverOperationRejected".into());
        }
        let request = serde_json::to_vec(&json!({"protocolVersion":1,"operation":operation,
            "config":self.config,"payload":payload}))
        .map_err(|_| "procurementDriverEncode")?;
        if request.len() > 262144 {
            return Err("procurementDriverInputTooLarge".into());
        }
        let mut command = Command::new(&self.node);
        command
            .arg(&self.script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = command
            .spawn()
            .map_err(|_| "procurementDriverStartFailed")?;
        let mut stdin = child.stdin.take().ok_or("procurementDriverStdin")?;
        if stdin.write_all(&request).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("procurementDriverWriteFailed".into());
        }
        drop(stdin);
        let stdout = child.stdout.take().ok_or("procurementDriverStdout")?;
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout
                .take(MAX_OUTPUT + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = sender.send(result);
        });
        let bytes = match receiver.recv_timeout(Duration::from_secs(20)) {
            Ok(Ok(bytes)) if bytes.len() <= MAX_OUTPUT as usize => bytes,
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("procurementChainResultUnknown".into());
            }
        };
        let status = child.wait().map_err(|_| "procurementChainResultUnknown")?;
        let envelope: Value =
            serde_json::from_slice(&bytes).map_err(|_| "procurementDriverReplyInvalid")?;
        if !status.success() || envelope["ok"] != true {
            return Err("procurementChainResultUnknown".into());
        }
        envelope
            .get("result")
            .cloned()
            .ok_or("procurementDriverReplyInvalid".into())
    }
}

pub struct ProcurementWorker {
    stop: Arc<AtomicBool>,
}
impl Drop for ProcurementWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
impl ProcurementWorker {
    pub fn start(config: SimulationConfig) -> Result<Option<Self>, String> {
        let driver = NodeDriver::from_env(&config.procurement_config)?;
        let Some(driver) = driver else {
            if config.procurement_config.enabled
                || !config.procurement_accounts_with_work()?.is_empty()
            {
                return Err("procurementRecoveryDriverRequired".into());
            }
            return Ok(None);
        };
        let stop = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&stop);
        thread::Builder::new()
            .name("procurement-outbox".into())
            .spawn(move || {
                let worker = format!("procurement-{}", std::process::id());
                let mut last_account = None;
                while !signal.load(Ordering::Acquire) {
                    if run_once(&config, &driver, &worker, &mut last_account).is_err() {
                        // No raw RPC response, private payload or key goes to logs.
                        eprintln!(
                            "procurement recovery pending; custody retained for reconciliation"
                        );
                    }
                    for _ in 0..20 {
                        if signal.load(Ordering::Acquire) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            })
            .map_err(|_| "procurementWorkerStartFailed")?;
        Ok(Some(Self { stop }))
    }
}

fn intent_json(i: &IntentV1) -> Value {
    json!({"domain":"MIR2_PROCUREMENT_INTENT_V1","network_hash":encode_hex(&i.network_hash),
        "package_id":encode_hex(&i.package_id),"dapp_storage_id":encode_hex(&i.dapp_storage_id),
        "operation":i.operation,"order_id":encode_hex(&i.order_id),"fill_id":encode_hex(&i.fill_id),
        "buyer_ref":encode_hex(&i.buyer_ref),"seller_ref":encode_hex(&i.seller_ref),"template_id":i.template_id,
        "conditions_hash":encode_hex(&i.conditions_hash),"quantity":i.quantity.to_string(),
        "price_gold":i.price_gold.to_string(),"expires_at_ms":i.expires_at_ms.to_string(),
        "rule_version":i.rule_version.to_string(),"nonce":encode_hex(&i.nonce),
        "valid_until_ms":i.valid_until_ms.to_string(),"actor_public_key":encode_hex(&i.actor_public_key)})
}
fn proof_json(p: &OracleProofV1) -> Value {
    json!({"domain":"MIR2_PROCUREMENT_ORACLE_V1","intent_hash":encode_hex(&p.intent_hash),
        "asset_lock_id":encode_hex(&p.asset_lock_id),"asset_instance_hash":encode_hex(&p.asset_instance_hash),
        "oracle_key_id":p.oracle_key_id.to_string(),"valid_until_ms":p.valid_until_ms.to_string(),"fence":p.fence.to_string()})
}

fn observed_receipt(
    c: &ProcurementConfig,
    order: &Order,
    value: &Value,
) -> Result<ChainReceipt, String> {
    if value["found"] != true
        || value["observation"]["coherent"] != true
        || value["observation"]["clock"]["finalized"] != true
        || value["receipt"]["finalized"] != true
        || value["receipt"]["success"] != true
    {
        return Err("procurementFinalityUnconfirmed".into());
    }
    let observation = &value["observation"];
    let clock = &observation["clock"];
    let transaction_checkpoint = integer(observation, "transactionCheckpoint")?;
    let checkpoint = integer(clock, "checkpoint")?;
    if checkpoint == 0
        || integer(value, "checkpoint")? < checkpoint.max(transaction_checkpoint)
        || string(observation, "transactionDigest")? != string(&value["receipt"], "digest")?
        || string(observation, "objectPreviousTransaction")? != string(&value["receipt"], "digest")?
        || bytes32(value, "orderId")? != order.publish.intent.order_id
    {
        return Err("procurementFinalityBindingInvalid".into());
    }
    let status = value["status"]
        .as_u64()
        .filter(|n| *n <= 4)
        .ok_or("procurementChainStateInvalid")? as u8;
    let fields = &value["fields"];
    let p = &order.publish.intent;
    if status != 0 {
        let lock = Sha256::digest([b"MIR2_ASSET_LOCK_V1".as_slice(), &p.hash()].concat());
        if bytes32(fields, "publish_intent_hash")? != p.hash()
            || bytes32(fields, "escrow_lock_id")? != <[u8; 32]>::from(lock)
            || integer(fields, "expires_at_ms")? != p.expires_at_ms
        {
            return Err("procurementChainTermsMismatch".into());
        }
    }
    let zero = [0; 32];
    let receipt = ChainReceipt {
        network_hash: c.domain.network_hash,
        package_id: c.domain.package_id,
        dapp_storage_id: c.domain.dapp_storage_id,
        order_id: p.order_id,
        object_id: string(value, "orderObjectId")?.into(),
        status,
        fill_id: if status == 0 {
            zero
        } else {
            bytes32(fields, "fill_id")?
        },
        buyer_ref: if status == 0 {
            p.buyer_ref
        } else {
            bytes32(fields, "buyer_ref")?
        },
        seller_ref: if status == 0 {
            zero
        } else {
            bytes32(fields, "seller_ref")?
        },
        template_id: if status == 0 {
            p.template_id
        } else {
            u32::try_from(
                fields["template_id"]
                    .as_u64()
                    .ok_or("procurementTemplateMissing")?,
            )
            .map_err(|_| "procurementTemplateInvalid")?
        },
        conditions_hash: if status == 0 {
            p.conditions_hash
        } else {
            bytes32(fields, "conditions_hash")?
        },
        price_gold: if status == 0 {
            p.price_gold
        } else {
            integer(fields, "price_gold")?
        },
        rule_version: if status == 0 {
            p.rule_version
        } else {
            integer(fields, "rule_version")?
        },
        asset_instance_hash: if status == 0 {
            zero
        } else {
            bytes32(fields, "asset_instance_hash")?
        },
        fence: if status == 0 {
            0
        } else {
            integer(fields, "fence")?
        },
        checkpoint,
        digest: string(observation, "transactionDigest")?.into(),
        event_seq: integer(observation, "eventSequence")?,
        chain_timestamp_ms: integer(clock, "timestampMs")?,
    };
    hex32(&receipt.object_id).map_err(str::to_string)?;
    if status != 0 && integer(fields, "quantity")? != 1 {
        return Err("procurementQuantityInvalid".into());
    }
    Ok(receipt)
}

fn find_buyer(config: &SimulationConfig, id: &str) -> Result<(String, Order), String> {
    config.procurement_find_order(id)
}

pub(crate) fn run_once(
    config: &SimulationConfig,
    driver: &dyn ChainDriver,
    worker: &str,
    last_account: &mut Option<String>,
) -> Result<(), String> {
    let mut failed = false;
    let accounts = config.procurement_accounts_with_work()?;
    if accounts.is_empty() {
        return Ok(());
    }
    let first = last_account
        .as_ref()
        .map(|last| accounts.partition_point(|a| a <= last) % accounts.len())
        .unwrap_or(0);
    for account in accounts
        .iter()
        .cycle()
        .skip(first)
        .take(accounts.len().min(128))
    {
        *last_account = Some(account.clone());
        for task in config.procurement_claim_outbox(&account, worker, now())? {
            if process_task(config, driver, worker, &account, &task).is_err() {
                // Failed transport/build/confirmation keeps every original asset
                // locked. Release only the worker lease, never the asset lock.
                let _ = config.procurement_record_submission(
                    &account,
                    &task.id,
                    worker,
                    now(),
                    None,
                    None,
                );
                failed = true;
            }
        }
        // Index/listener availability cannot delay the authoritative outbox.
        // Every asset task independently checks the fullnode's current order.
        if replay_account(config, driver, &account).is_err() {
            failed = true;
        }
    }
    if failed {
        Err("procurementRecoveryPending".into())
    } else {
        Ok(())
    }
}

fn replay_account(
    config: &SimulationConfig,
    driver: &dyn ChainDriver,
    account: &str,
) -> Result<(), String> {
    let source = format!(
        "sui-procurement:{}:{}",
        encode_hex(&config.procurement_config.domain.network_hash),
        encode_hex(&config.procurement_config.domain.package_id)
    );
    let mut ledger = config.procurement_view(account)?;
    let mut cursor = ledger
        .cursors
        .get(&source)
        .map(|s| serde_json::from_str::<Value>(s))
        .transpose()
        .map_err(|_| "procurementCursorInvalid")?;
    // Bounded backfill. A disconnect resumes from the same durable event ID.
    for _ in 0..4 {
        let page = driver.call("replayEvents", json!({"cursor":cursor,"limit":50}))?;
        let events = page["events"]
            .as_array()
            .ok_or("procurementEventPageInvalid")?;
        for event in events {
            let id = event
                .get("id")
                .filter(|v| v.is_object())
                .ok_or("procurementEventIdMissing")?;
            let event_cursor = event.get("cursor").unwrap_or(id);
            let next =
                serde_json::to_string(event_cursor).map_err(|_| "procurementCursorInvalid")?;
            let order_id = event_order_id(event);
            let order = order_id.as_ref().and_then(|id| ledger.orders.get(id));
            if let Some(order) =
                order.filter(|o| matches!(o.status, OrderStatus::Publishing | OrderStatus::Open))
            {
                let query = driver.call(
                    "queryOrder",
                    json!({"orderId":encode_hex(&order.publish.intent.order_id)}),
                )?;
                let receipt = observed_receipt(&config.procurement_config, order, &query)?;
                config.procurement_apply_chain_receipt_with_cursor(
                    &config.procurement_config,
                    account,
                    &receipt,
                    &source,
                    &next,
                )?;
                ledger = config.procurement_view(account)?;
            } else {
                config.procurement_advance_cursor(account, &source, &next)?;
            }
            cursor = Some(event_cursor.clone());
        }
        // Empty finalized checkpoints also advance the resumable scan. Asset
        // events above have completed before this page boundary is committed.
        if let Some(next) = page.get("nextCursor").filter(|v| !v.is_null()) {
            let encoded = serde_json::to_string(next).map_err(|_| "procurementCursorInvalid")?;
            config.procurement_advance_cursor(account, &source, &encoded)?;
            cursor = Some(next.clone());
        }
        if page["hasNextPage"] != true {
            break;
        }
    }
    Ok(())
}

fn event_order_id(event: &Value) -> Option<String> {
    let value = event
        .pointer("/parsedJson/order_id")
        .or_else(|| event.pointer("/parsedJson/entity_id"))?;
    let bytes = if let Some(s) = value.as_str() {
        hex32(s).ok()?
    } else {
        value
            .as_array()?
            .iter()
            .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
            .collect::<Option<Vec<_>>>()?
            .try_into()
            .ok()?
    };
    Some(encode_hex(&bytes)[2..].to_owned())
}

fn process_task(
    config: &SimulationConfig,
    driver: &dyn ChainDriver,
    worker: &str,
    account: &str,
    task: &OutboxTask,
) -> Result<(), String> {
    // A previous task in this same claimed batch, or another finalized
    // callback, may already have completed this task. Never revive its lease.
    let current = config.procurement_view(account)?;
    let task = current
        .outbox
        .get(&task.id)
        .ok_or("procurementJobMissing")?;
    if task.status == JobStatus::Confirmed {
        return Ok(());
    }
    let (buyer, order) = find_buyer(config, &task.order_id)?;
    let query = if let Some(object) = &order.object_id {
        json!({"orderObjectId":object})
    } else {
        json!({"orderId":encode_hex(&order.publish.intent.order_id)})
    };
    let observed = driver.call("queryOrder", query)?;
    let receipt = if observed["found"] == true {
        Some(observed_receipt(
            &config.procurement_config,
            &order,
            &observed,
        )?)
    } else {
        None
    };
    if receipt.is_none() && observed["observation"]["registryAbsence"] == true {
        let o = &observed["observation"];
        let clock = &o["clock"];
        if o["coherent"] != true || clock["finalized"] != true {
            return Err("procurementFinalityUnconfirmed".into());
        }
        if integer(clock, "timestampMs")? > order.publish.intent.valid_until_ms {
            let c = &config.procurement_config;
            config.procurement_apply_chain_absence(
                c,
                &buyer,
                &ChainAbsenceReceipt {
                    network_hash: c.domain.network_hash,
                    package_id: c.domain.package_id,
                    dapp_storage_id: c.domain.dapp_storage_id,
                    order_id: order.publish.intent.order_id,
                    clock_checkpoint: integer(clock, "checkpoint")?,
                    clock_digest: string(clock, "digest")?.into(),
                    chain_timestamp_ms: integer(clock, "timestampMs")?,
                    registry_version: string(o, "registryVersion")?.into(),
                    registry_previous_transaction: string(o, "registryPreviousTransaction")?.into(),
                    registry_absence: true,
                },
            )?;
            return Ok(());
        }
    }
    if let Some(r) = &receipt {
        let needs_apply = order.status == OrderStatus::Publishing
            || r.status != 1
            || r.fence != order.fence
            || task.proof.as_ref().is_some_and(|proof| {
                r.chain_timestamp_ms > proof.valid_until_ms || r.fence > proof.fence
            })
            || order.offers.iter().any(|(id, seller)| {
                config
                    .procurement_view(seller)
                    .ok()
                    .and_then(|l| l.locks.get(id).cloned())
                    .is_some_and(|lock| {
                        !lock.released
                            && (r.chain_timestamp_ms > lock.signed.intent.valid_until_ms
                                || r.fence > lock.fence)
                    })
            });
        if needs_apply {
            config.procurement_apply_chain_receipt(&config.procurement_config, &buyer, r)?;
        }
        if config
            .procurement_view(account)?
            .outbox
            .get(&task.id)
            .is_some_and(|job| job.status == JobStatus::Confirmed)
        {
            return Ok(());
        }
        if r.status >= 2
            || (r.status == 0 && r.chain_timestamp_ms > order.publish.intent.valid_until_ms)
        {
            return Ok(());
        }
    }
    let action = match task.kind {
        JobKind::Publish if receipt.as_ref().is_some_and(|r| r.status == 1) => return Ok(()),
        JobKind::Publish => {
            if receipt.is_some() {
                "activate"
            } else {
                "create"
            }
        }
        JobKind::Fill => "fill",
        JobKind::Cancel => "cancel",
        JobKind::Expire => "expire",
        JobKind::Reconcile => {
            if receipt.as_ref().is_none_or(|r| {
                r.status != 1 || r.chain_timestamp_ms < order.publish.intent.expires_at_ms
            }) {
                return config.procurement_release_watch(account, &task.id, worker, now(), 5000);
            }
            "expire"
        }
    };
    let object = receipt
        .as_ref()
        .map(|r| r.object_id.clone())
        .or_else(|| order.object_id.clone());
    // Never prepare a new transaction while a previous exact transaction for
    // this step may still land. Re-send only the already committed envelope.
    let prepared = if let Some(p) = task
        .signed_transactions
        .get(action)
        .filter(|p| !task.failed_transactions.contains_key(&p.transaction_digest))
    {
        p.clone()
    } else {
        let mut payload = json!({"action":action,"orderObjectId":object});
        if action == "activate" {
            payload["conditionsBcs"] = json!(encode_hex(
                &order.conditions.canonical_bcs().map_err(str::to_string)?
            ));
        }
        if let (Some(s), Some(p)) = (&task.signed, &task.proof) {
            payload["intent"] = intent_json(&s.intent);
            payload["proof"] = proof_json(p);
            payload["actorSignature"] = json!(encode_hex(&s.signature));
        }
        let built = driver.call("build", payload)?;
        if built["action"] != action || string(&built, "digest")?.is_empty() {
            return Err("procurementDriverTransactionMismatch".into());
        }
        let bytes = serde_json::to_vec(&built).map_err(|_| "procurementDriverEncode")?;
        let oracle_signature = built
            .get("oracleSignature")
            .and_then(Value::as_str)
            .map(decode_hex)
            .transpose()
            .map_err(str::to_string)?
            .unwrap_or_default();
        let p = PreparedTransaction {
            step: action.into(),
            transaction_digest: string(&built, "digest")?.into(),
            signed_transaction_bytes: bytes,
            oracle_signature,
        };
        config.procurement_prepare_transaction(account, &task.id, worker, now(), p.clone())?;
        p
    };
    let submitted: Value = serde_json::from_slice(&prepared.signed_transaction_bytes)
        .map_err(|_| "procurementSavedTransactionInvalid")?;
    if string(&submitted, "digest")? != prepared.transaction_digest {
        return Err("procurementSavedTransactionInvalid".into());
    }
    let finality = driver.call(
        "queryFinality",
        json!({"digest":prepared.transaction_digest}),
    )?;
    if finality["known"] == true && finality["finalized"] == true {
        if string(&finality, "digest")? != prepared.transaction_digest {
            return Err("procurementFinalityBindingInvalid".into());
        }
        if finality["success"] == false {
            return config.procurement_record_finalized_failure(
                account,
                &task.id,
                worker,
                now(),
                &prepared.transaction_digest,
                integer(&finality, "checkpoint")?,
            );
        }
        // Even a finalized failed transaction does not authorize a refund.
        // Query the order/fence/chain Clock on the next recovery turn.
        return config.procurement_record_submission(
            account,
            &task.id,
            worker,
            now(),
            Some(prepared.transaction_digest),
            object,
        );
    }
    let _ = driver.call("submitExact", submitted)?;
    config.procurement_record_submission(
        account,
        &task.id,
        worker,
        now(),
        Some(prepared.transaction_digest),
        object,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_driver_config_is_required_and_mainnet_is_rejected() {
        let mut c = ProcurementConfig::default();
        c.domain = Domain {
            network_hash: [1; 32],
            package_id: [2; 32],
            dapp_storage_id: [3; 32],
            oracle_key_id: 1,
        };
        let mut v = json!({"enabled":true,"environmentTag":"isolated-test","network":"testnet",
            "networkHash":encode_hex(&[1;32]),"packageId":encode_hex(&[2;32]),"dappStorageId":encode_hex(&[3;32]),
            "frameworkPackageId":encode_hex(&[4;32]),"configId":encode_hex(&[5;32]),"oraclePublicKey":encode_hex(&[6;32]),
            "oracleKeyId":"1","gasBudgetMist":"100","maxGasBudgetMist":"100","dailyGasBudgetMist":"1000"});
        assert!(check_domain(&v, &c).is_ok());
        v["network"] = json!("mainnet");
        assert!(check_domain(&v, &c).is_err());
        v["network"] = json!("testnet");
        v["gasBudgetMist"] = json!("101");
        assert!(check_domain(&v, &c).is_err());
        v["gasBudgetMist"] = json!("100");
        v["packageId"] = json!(encode_hex(&[9; 32]));
        assert!(check_domain(&v, &c).is_err());
    }
    #[test]
    fn internal_proof_wire_preserves_all_domain_and_lock_fields() {
        let p = OracleProofV1 {
            intent_hash: [1; 32],
            asset_lock_id: [2; 32],
            asset_instance_hash: [3; 32],
            oracle_key_id: u64::MAX,
            valid_until_ms: 123,
            fence: 8,
        };
        let v = proof_json(&p);
        assert_eq!(bytes32(&v, "asset_lock_id").unwrap(), [2; 32]);
        assert_eq!(integer(&v, "oracle_key_id").unwrap(), u64::MAX);
        assert_eq!(v["domain"], "MIR2_PROCUREMENT_ORACLE_V1");
    }
}

#[cfg(test)]
#[path = "procurement_worker_tests.rs"]
mod integration_tests;
