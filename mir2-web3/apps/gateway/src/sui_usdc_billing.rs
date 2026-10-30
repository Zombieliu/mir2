//! Testnet-only USDC provider. Durable game receipts, not browser callbacks,
//! authorize credits. The issuer never signs a player's USDC payment.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, KeyInit, Mac};
use mir2_simulation::{billing::RechargeOffer, sui_usdc_billing::*, SimulationConfig};
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

const MAX_BYTES: usize = 1024 * 1024;
const PINS: &[(&str, &[u8])] = &[
    (
        "service/cli.mjs",
        include_bytes!("../../../onchain/usdc-recharge/service/cli.mjs"),
    ),
    (
        "service/driver.mjs",
        include_bytes!("../../../onchain/usdc-recharge/service/driver.mjs"),
    ),
    (
        "service/model.mjs",
        include_bytes!("../../../onchain/usdc-recharge/service/model.mjs"),
    ),
    (
        "package.json",
        include_bytes!("../../../onchain/usdc-recharge/package.json"),
    ),
    (
        "package-lock.json",
        include_bytes!("../../../onchain/usdc-recharge/package-lock.json"),
    ),
];
pub(crate) fn now() -> u64 {
    mir2_simulation::monthly_card::monthly_card_now_ms()
}
pub(crate) fn sha(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn flag(value: Option<&str>) -> Result<bool, String> {
    match value {
        None | Some("0" | "false") => Ok(false),
        Some("1" | "true") => Ok(true),
        _ => Err("usdcConfigurationInvalid".into()),
    }
}
/// Public playtest opt-in permits staging only. Production and unreadable
/// markers always fail closed, including when only recovering existing orders.
fn validate_testnet_runtime(
    markers: impl IntoIterator<Item = Result<Option<String>, ()>>,
    playtest_testnet: Result<Option<String>, ()>,
) -> Result<(), String> {
    let playtest_testnet = flag(
        playtest_testnet
            .map_err(|_| "usdcTestnetConfigurationRequired")?
            .as_deref(),
    )?;
    for marker in markers {
        let value = marker.map_err(|_| "usdcTestnetConfigurationRequired")?;
        if value.is_some_and(|value| match value.trim().to_ascii_lowercase().as_str() {
            "production" | "prod" => true,
            "staging" | "stage" | "preprod" | "preproduction" => !playtest_testnet,
            _ => false,
        }) {
            return Err("usdcTestnetConfigurationRequired".into());
        }
    }
    Ok(())
}
fn validate_testnet_driver_config(config: &Value) -> Result<(), String> {
    if config["enabled"] != true
        || config["environmentTag"] != "isolated-test"
        || config["network"] != "testnet"
        || config["chainIdentifier"] != TESTNET_CHAIN_IDENTIFIER
        || config["coinType"] != TESTNET_USDC_COIN_TYPE
    {
        return Err("usdcTestnetConfigurationRequired".into());
    }
    Ok(())
}
fn required(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| "usdcConfigurationMissing".into())
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v[key].as_str().ok_or("usdcDriverReplyInvalid".into())
}
fn decimal(v: &Value, key: &str) -> Result<u64, String> {
    text(v, key)?
        .parse()
        .map_err(|_| "usdcDriverReplyInvalid".into())
}
fn hex_address(v: &str) -> bool {
    v.len() == 66
        && v.starts_with("0x")
        && v[2..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && v[2..].bytes().any(|b| b != b'0')
}
fn prefixed(v: &str) -> String {
    format!("0x{}", v.strip_prefix("0x").unwrap_or(v))
}

pub(crate) fn valid_origin(raw: &str) -> bool {
    if raw.len() > 512
        || !raw.is_ascii()
        || raw
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace() || b == b'\\')
    {
        return false;
    }
    reqwest::Url::parse(raw).is_ok_and(|u| {
        u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none()
            && u.path() == "/"
            && u.host_str().is_some()
            && u.origin().ascii_serialization() == raw
            && (u.scheme() == "https"
                || (u.scheme() == "http" && u.host_str() == Some("127.0.0.1")))
    })
}
pub(crate) trait UsdcDriver: Send + Sync {
    fn call(&self, operation: &str, params: Value) -> Result<Value, String>;
}
struct NodeDriver {
    node: PathBuf,
    script: PathBuf,
    config: Value,
}
impl UsdcDriver for NodeDriver {
    fn call(&self, operation: &str, params: Value) -> Result<Value, String> {
        if !matches!(
            operation,
            "configureValidate"
                | "prepareInvoice"
                | "submitPrepared"
                | "observeCreate"
                | "buildPay"
                | "buildPayBytes"
                | "observeInvoice"
                | "verifyWalletProof"
        ) {
            return Err("usdcOperationRejected".into());
        }
        let input = serde_json::to_vec(
            &json!({"operation":operation,"config":self.config,"params":params}),
        )
        .map_err(|_| "usdcDriverInputInvalid")?;
        if input.len() > MAX_BYTES {
            return Err("usdcDriverInputInvalid".into());
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
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|_| "usdcDriverUnavailable")?;
        let write = child
            .stdin
            .take()
            .ok_or("usdcDriverUnavailable")?
            .write_all(&input);
        if write.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("usdcResultUnknown".into());
        }
        let output = child.stdout.take().ok_or("usdcDriverUnavailable")?;
        let (tx, rx) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut b = Vec::new();
            let r = output
                .take((MAX_BYTES + 1) as u64)
                .read_to_end(&mut b)
                .map(|_| b);
            let _ = tx.send(r);
        });
        let bytes = match rx.recv_timeout(Duration::from_secs(20)) {
            Ok(Ok(b)) if b.len() <= MAX_BYTES => b,
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("usdcResultUnknown".into());
            }
        };
        let success = child.wait().map_err(|_| "usdcResultUnknown")?.success();
        let reply: Value = serde_json::from_slice(&bytes).map_err(|_| "usdcResultUnknown")?;
        if !success || reply["ok"] != true {
            return Err("usdcResultUnknown".into());
        }
        reply
            .get("result")
            .cloned()
            .ok_or("usdcDriverReplyInvalid".into())
    }
}

#[derive(Clone)]
pub(crate) struct UsdcBilling {
    pub enabled: bool,
    pub domain: UsdcDomain,
    pub offers: Vec<RechargeOffer>,
    pub origin: String,
    checkout_key: Arc<Vec<u8>>,
    pub driver: Arc<dyn UsdcDriver>,
}
impl UsdcBilling {
    pub(crate) fn from_env() -> Result<Option<Self>, String> {
        let enabled = flag(std::env::var("MIR2_SUI_USDC_ENABLED").ok().as_deref())?;
        let Some(config_path) = std::env::var_os("MIR2_SUI_USDC_DRIVER_CONFIG") else {
            return if enabled {
                Err("usdcConfigurationMissing".into())
            } else {
                Ok(None)
            };
        };
        validate_testnet_runtime(
            ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV"]
                .into_iter()
                .map(|name| match std::env::var(name) {
                    Ok(value) => Ok(Some(value)),
                    Err(std::env::VarError::NotPresent) => Ok(None),
                    Err(std::env::VarError::NotUnicode(_)) => Err(()),
                }),
            match std::env::var("MIR2_PLAYTEST_TESTNET") {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(std::env::VarError::NotUnicode(_)) => Err(()),
            },
        )?;
        let bytes = std::fs::read(config_path).map_err(|_| "usdcConfigurationMissing")?;
        if bytes.len() > 64 * 1024 {
            return Err("usdcConfigurationInvalid".into());
        }
        let c: Value = serde_json::from_slice(&bytes).map_err(|_| "usdcConfigurationInvalid")?;
        validate_testnet_driver_config(&c)?;
        let rule_version = std::env::var("MIR2_SUI_USDC_RULE_VERSION")
            .unwrap_or("1".into())
            .parse::<u64>()
            .map_err(|_| "usdcRuleVersionInvalid")?;
        let domain = UsdcDomain {
            chain_identifier: TESTNET_CHAIN_IDENTIFIER.into(),
            package_id: text(&c, "packageId")?.into(),
            config_id: text(&c, "configId")?.into(),
            merchant: text(&c, "merchantAddress")?.into(),
            coin_type: TESTNET_USDC_COIN_TYPE.into(),
            rule_version,
        };
        domain.validate()?;
        let budget = decimal(&c, "gasBudgetMist")?;
        if budget == 0 || budget > 50_000_000 {
            return Err("usdcGasBudgetInvalid".into());
        }
        let origin = required("MIR2_SUI_USDC_PUBLIC_ORIGIN")?;
        if !valid_origin(&origin) {
            return Err("usdcCheckoutOriginInvalid".into());
        }
        let ledger = PathBuf::from(required("MIR2_SUI_USDC_GAS_LEDGER_PATH")?);
        let maximum = std::env::var("MIR2_SUI_USDC_GAS_MAX_MIST")
            .unwrap_or("50000000".into())
            .parse::<u64>()
            .map_err(|_| "usdcGasBudgetInvalid")?;
        if !ledger.is_absolute() || maximum < budget || maximum > 1_000_000_000 {
            return Err("usdcGasBudgetInvalid".into());
        }
        let offers: Vec<RechargeOffer> = serde_json::from_str(
            &std::env::var("MIR2_SUI_USDC_OFFERS_JSON").unwrap_or("[]".into()),
        )
        .map_err(|_| "usdcOffersInvalid")?;
        let mut ids = std::collections::BTreeSet::new();
        if offers.len() > 12 || (enabled && offers.is_empty()) {
            return Err("usdcOffersInvalid".into());
        }
        for offer in &offers {
            offer.validate()?;
            if offer.currency != "usd"
                || offer.amount_minor.checked_mul(10_000).is_none()
                || !ids.insert(&offer.id)
            {
                return Err("usdcOffersInvalid".into());
            }
        }
        let key = std::env::var("MIR2_SUI_USDC_CHECKOUT_KEY").unwrap_or_default();
        if (enabled || !key.is_empty())
            && (key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("usdcCheckoutKeyInvalid".into());
        }
        let key = key
            .as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).expect("hex"), 16).expect("hex"))
            .collect();
        let node = PathBuf::from(required("MIR2_SUI_USDC_NODE")?);
        let script = PathBuf::from(required("MIR2_SUI_USDC_DRIVER")?);
        if !node.is_absolute() || !node.is_file() || !script.is_absolute() {
            return Err("usdcDriverPathInvalid".into());
        }
        let root = script
            .parent()
            .and_then(|p| p.parent())
            .ok_or("usdcDriverPathInvalid")?;
        for (relative, expected) in PINS {
            let actual =
                std::fs::read(root.join(relative)).map_err(|_| "usdcDriverSourceMissing")?;
            let normalize = |b: &[u8]| String::from_utf8_lossy(b).replace("\r\n", "\n");
            if sha(normalize(&actual).as_bytes()) != sha(normalize(expected).as_bytes()) {
                return Err("usdcDriverSourceMismatch".into());
            }
        }
        if script.canonicalize().map_err(|_| "usdcDriverPathInvalid")?
            != root
                .join("service/cli.mjs")
                .canonicalize()
                .map_err(|_| "usdcDriverPathInvalid")?
        {
            return Err("usdcDriverPathInvalid".into());
        }
        Ok(Some(Self {
            enabled,
            domain,
            offers,
            origin,
            checkout_key: Arc::new(key),
            driver: Arc::new(NodeDriver {
                node,
                script,
                config: c,
            }),
        }))
    }
    pub(crate) fn validate_store(&self, config: &SimulationConfig) -> Result<(), String> {
        if config.account_store_path.is_none() && config.account_store_database_url.is_none() {
            return Err("billingDurableStoreRequired".into());
        }
        // The existing Source billing tick uses this configured price as its
        // durable recharge polling gate. Keep Gateway and Source consistent.
        if config.billing_monthly_card_credit_price.is_none() {
            return Err("billingMonthlyCardPriceRequired".into());
        }
        Ok(())
    }
    fn token(&self, account: &str, character: i32, request: &str) -> Result<String, String> {
        if self.checkout_key.len() != 32 {
            return Err("usdcCheckoutKeyInvalid".into());
        }
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.checkout_key)
            .map_err(|_| "usdcCheckoutKeyInvalid")?;
        mac.update(
            &serde_json::to_vec(&("MIR2_USDC_CHECKOUT_V1", account, character, request))
                .map_err(|_| "usdcEncodingFailed")?,
        );
        Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
    }
    pub(crate) fn checkout(
        &self,
        config: &SimulationConfig,
        account: &str,
        character: i32,
        request: &str,
        offer_id: &str,
    ) -> Result<(String, UsdcRechargeOrder), String> {
        self.validate_store(config)?;
        let token = self.token(account, character, request)?;
        // Recover the same frozen order after client re-login, an offer change,
        // or disabling new sales. Reopening never creates or pays another one.
        if let Some(order) = config
            .usdc_recharge_orders(account, character)?
            .into_iter()
            .find(|o| o.request_id == request)
        {
            self.ensure_domain(&order)?;
            if order.offer.id != offer_id || order.capability_hash != sha(token.as_bytes()) {
                return Err("billingRequestIdReused".into());
            }
            let url = format!(
                "{}/v1/billing/sui-usdc/pay#{}.{}",
                self.origin, order.id, token
            );
            return Ok((url, order));
        }
        if !self.enabled {
            return Err("billingUnavailable".into());
        }
        let offer = self
            .offers
            .iter()
            .find(|o| o.id == offer_id)
            .ok_or("billingOfferMissing")?;
        let at = now();
        let order = config.prepare_usdc_recharge_order(
            account,
            character,
            request,
            offer,
            &self.domain,
            &sha(token.as_bytes()),
            at.checked_add(30 * 60 * 1000).ok_or("usdcClockInvalid")?,
            at.checked_add(20 * 60 * 1000).ok_or("usdcClockInvalid")?,
            at,
        )?;
        let url = format!(
            "{}/v1/billing/sui-usdc/pay#{}.{}",
            self.origin, order.id, token
        );
        Ok((url, order))
    }
    pub(crate) fn public_offers(&self) -> Vec<Value> {
        self.offers.iter().map(|o|json!({"id":o.id,"label":o.label,"credits":o.credits,"amountMicros":(o.amount_minor*10_000).to_string(),"network":"testnet"})).collect()
    }
    pub(crate) fn ensure_domain(&self, o: &UsdcRechargeOrder) -> Result<(), String> {
        let a = &o.domain;
        let b = &self.domain;
        if a.chain_identifier == b.chain_identifier
            && a.package_id == b.package_id
            && a.config_id == b.config_id
            && a.merchant == b.merchant
            && a.coin_type == b.coin_type
        {
            Ok(())
        } else {
            Err("usdcRecoveryDomainMismatch".into())
        }
    }
}
pub(crate) fn chain_order(o: &UsdcRechargeOrder) -> Result<Value, String> {
    Ok(
        json!({"orderRef":prefixed(&order_ref(&o.id)),"accountRef":prefixed(&o.account_ref),"snapshotHash":prefixed(&o.frozen_hash()?),"payer":o.payer.as_ref().ok_or("usdcPayerMissing")?,"merchant":o.domain.merchant,"amountMicros":o.amount_micro.to_string(),"expiresAtMs":o.expires_at_ms.to_string()}),
    )
}
pub(crate) fn wallet_message(
    o: &UsdcRechargeOrder,
    payer: &str,
    origin: &str,
) -> Result<String, String> {
    if !hex_address(payer) {
        return Err("usdcPayerInvalid".into());
    }
    Ok(format!("MIR2 Sui USDC recharge wallet confirmation v1\nOrigin: {origin}\nNetwork: sui:testnet ({})\nOrder: {}\nAccount reference: {}\nCharacter: {}\nPayer: {payer}\nMerchant: {}\nCoin type: {}\nUSDC (6 decimals): {}.{:06}\nCredits: {}\nOffer: {}\nRule: {}\nPackage: {}\nConfig: {}\nCreated (ms): {}\nPay before (ms): {}\nThis message binds this wallet to this one recharge invoice. It does not transfer funds; the wallet will request a separate payment transaction.",o.domain.chain_identifier,o.id,o.account_ref,o.character_index,o.domain.merchant,o.domain.coin_type,o.amount_micro/1_000_000,o.amount_micro%1_000_000,o.offer.credits,o.offer.id,o.domain.rule_version,o.domain.package_id,o.domain.config_id,o.created_at_ms,o.expires_at_ms))
}
pub(crate) fn public_order(o: &UsdcRechargeOrder) -> Value {
    public_order_at(o, now())
}
fn public_order_at(o: &UsdcRechargeOrder, now_ms: u64) -> Value {
    let state = if now_ms >= o.expires_at_ms
        && ((o.prepared_tx.is_none()
            && matches!(
                o.state,
                UsdcOrderState::Prepared | UsdcOrderState::PayerBound
            ))
            || (o.state == UsdcOrderState::InvoiceBound && o.payment_hint.is_none()))
    {
        "expired"
    } else {
        match o.state {
            UsdcOrderState::Prepared => "prepared",
            UsdcOrderState::PayerBound | UsdcOrderState::InvoicePrepared => "invoicePending",
            UsdcOrderState::InvoiceBound => {
                if o.payment_hint.is_some() {
                    "paymentSubmitted"
                } else {
                    "awaitingPayment"
                }
            }
            UsdcOrderState::PaidPending => "paidPending",
            UsdcOrderState::Applied => "applied",
            _ => "review",
        }
    };
    json!({"id":o.id,"orderRequestId":o.request_id,"offerId":o.offer.id,"credits":o.offer.credits,"amountMicros":o.amount_micro.to_string(),"network":"testnet","state":state,"createdAtMs":o.created_at_ms,"expiresAtMs":o.expires_at_ms,"transactionDigest":o.receipt.as_ref().map(|r|r.digest.as_str()).or(o.payment_hint.as_deref())})
}

pub(crate) fn run_order(
    config: &SimulationConfig,
    provider: &UsdcBilling,
    account: &str,
    id: &str,
    worker: &str,
) -> Result<(), String> {
    let Some(mut o) = config.claim_usdc_recharge_job(account, id, worker, now())? else {
        return Ok(());
    };
    let result = (|| {
        provider.ensure_domain(&o)?;
        if o.payer.is_none() || o.receipt.is_some() {
            return Ok(());
        }
        let terms = chain_order(&o)?;
        if o.prepared_tx.is_none() {
            if config
                .usdc_recharge_active_creation(&o.domain)?
                .is_some_and(|(_, active)| active.id != o.id)
            {
                return Ok(());
            }
            let result = provider
                .driver
                .call("prepareInvoice", json!({"order":terms}))?;
            let tx = UsdcPreparedTransaction {
                bytes_base64: text(&result, "bytesB64")?.into(),
                signature_base64: text(&result, "signature")?.into(),
                digest: text(&result, "digest")?.into(),
            };
            o = config.record_usdc_prepared_transaction(account, id, worker, &tx, now())?;
        }
        if o.invoice.is_none() {
            let t = o
                .prepared_tx
                .as_ref()
                .ok_or("usdcPreparedTransactionMissing")?;
            let prepared =
                json!({"bytesB64":t.bytes_base64,"signature":t.signature_base64,"digest":t.digest});
            let _ = provider
                .driver
                .call("submitPrepared", json!({"prepared":prepared,"order":terms}));
            let observed = provider
                .driver
                .call("observeCreate", json!({"prepared":prepared,"order":terms}))?;
            if observed["state"] == "failed" {
                config.mark_usdc_recharge_review(account, id, worker, "createFailed", now())?;
                return Ok(());
            }
            if observed["state"] != "created" {
                return Err("usdcResultUnknown".into());
            }
            let invoice = UsdcInvoiceBinding {
                invoice_id: text(&observed, "invoiceId")?.into(),
                create_digest: text(&observed, "digest")?.into(),
                checkpoint: decimal(&observed, "checkpoint")?,
            };
            o = config.bind_usdc_recharge_invoice(account, id, worker, &invoice, now())?;
        }
        let invoice = o.invoice.as_ref().ok_or("usdcInvoiceMissing")?;
        let observed = provider.driver.call(
            "observeInvoice",
            json!({"invoiceId":invoice.invoice_id,"order":terms}),
        )?;
        if observed["state"] != "paid" {
            return Ok(());
        }
        let r = &observed["receipt"];
        // Recheck every frozen field even though the pinned verifier has decoded
        // the finalized BCS event. Browser-supplied hints never enter this path.
        let network = prefixed(&sha(o.domain.chain_identifier.as_bytes()));
        if text(r, "networkHash")? != network
            || text(r, "orderRef")? != prefixed(&order_ref(&o.id))
            || text(r, "accountRef")? != prefixed(&o.account_ref)
            || text(r, "snapshotHash")? != prefixed(&o.frozen_hash()?)
            || decimal(r, "expiresAtMs")? != o.expires_at_ms
        {
            return Err("usdcPaymentBindingMismatch".into());
        }
        let receipt = VerifiedUsdcReceipt {
            chain_identifier: o.domain.chain_identifier.clone(),
            package_id: text(r, "packageId")?.into(),
            config_id: text(r, "configId")?.into(),
            invoice_id: text(r, "invoiceId")?.into(),
            order_id: o.id.clone(),
            account_ref: o.account_ref.clone(),
            payer: text(r, "payer")?.into(),
            merchant: text(r, "merchant")?.into(),
            coin_type: text(r, "coinType")?.into(),
            amount_micro: decimal(r, "amountMicros")?,
            frozen_hash: o.frozen_hash()?,
            digest: text(r, "transactionDigest")?.into(),
            event_seq: decimal(r, "eventSequence")?,
            checkpoint: decimal(r, "checkpoint")?,
            confirmed_at_ms: decimal(r, "paidAtMs")?,
        };
        config.confirm_usdc_recharge_payment(account, id, &receipt, now())?;
        Ok(())
    })();
    // A committed exact transaction is never discarded on timeout. Releasing
    // only the processing lease permits the next worker to query/retry it.
    if result
        .as_ref()
        .is_err_and(|e: &String| e.contains("COMMIT_OUTCOME_UNKNOWN"))
    {
        return result;
    }
    let _ = config.release_usdc_recharge_job(account, id, worker, now());
    result
}
pub struct SuiUsdcWorker {
    stop: Arc<AtomicBool>,
}
impl Drop for SuiUsdcWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
impl SuiUsdcWorker {
    pub fn start(config: SimulationConfig) -> Result<Option<Self>, String> {
        let provider = UsdcBilling::from_env()?;
        let candidates = config.usdc_recharge_reconcile_candidates(4096)?;
        let Some(provider) = provider else {
            return if candidates.is_empty() {
                Ok(None)
            } else {
                Err("usdcRecoveryDriverRequired".into())
            };
        };
        provider.validate_store(&config)?;
        for (_, order) in &candidates {
            provider.ensure_domain(order)?;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        thread::Builder::new()
            .name("sui-usdc-recovery".into())
            .spawn(move || {
                let mut nonce = [0u8; 32];
                use rand::RngCore;
                if rand::rngs::OsRng.try_fill_bytes(&mut nonce).is_err() {
                    return;
                }
                let worker = format!("usdc-{}-{}", std::process::id(), sha(&nonce));
                let mut last: Option<(String, String)> = None;
                let mut priority = true;
                while !signal.load(Ordering::Acquire) {
                    let active = if priority {
                        config
                            .usdc_recharge_active_creation(&provider.domain)
                            .ok()
                            .flatten()
                    } else {
                        None
                    };
                    priority = !priority;
                    if let Some((account, order)) = active {
                        let _ = run_order(&config, &provider, &account, &order.id, &worker);
                    } else if let Ok(orders) = config.usdc_recharge_reconcile_candidates_after(
                        1,
                        last.as_ref().map(|(a, o)| (a.as_str(), o.as_str())),
                    ) {
                        if let Some((account, order)) = orders.first() {
                            last = Some((account.clone(), order.id.clone()));
                            let _ = run_order(&config, &provider, account, &order.id, &worker);
                        } else {
                            last = None;
                        }
                    }
                    for _ in 0..30 {
                        if signal.load(Ordering::Acquire) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            })
            .map_err(|_| "usdcRecoveryWorkerUnavailable")?;
        Ok(Some(Self { stop }))
    }
}

#[cfg(test)]
#[path = "sui_usdc_billing_tests.rs"]
mod durable_tests;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkout_origins_are_exact() {
        for s in [
            "https://game.example",
            "https://game.example:8443",
            "http://127.0.0.1:17830",
        ] {
            assert!(valid_origin(s), "{s}");
        }
        for s in [
            "http://game.example",
            "http://localhost:17830",
            "http://127.1:17830",
            "https://user@game.example",
            "https://game.example/redirect",
            "https://game.example/",
            "https://game.example?x=y",
            "https://game.example#fragment",
            "https://game.example\\@evil.invalid",
        ] {
            assert!(!valid_origin(s), "{s}");
        }
    }
    #[test]
    fn flags_fail_closed() {
        assert_eq!(flag(None).unwrap(), false);
        assert!(flag(Some("1")).unwrap());
        assert!(flag(Some("yes")).is_err());
    }
    #[test]
    fn canonical_addresses_reject_aliases() {
        assert!(hex_address(&format!("0x{}1", "0".repeat(63))));
        assert!(!hex_address("0x1"));
        assert!(!hex_address(&format!("0x{}", "0".repeat(64))));
    }
}
