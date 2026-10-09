//! Server-owned Stripe Checkout. No client prices, secrets, redirects or payment
//! success pages are accepted as proof of settlement.
use hmac::{Hmac, KeyInit, Mac};
use mir2_simulation::{
    billing::{RechargeOffer, RechargeOrder, VerifiedRechargePayment},
    SimulationConfig,
};
use serde_json::{json, Value};
use sha2::Sha256;
use std::{sync::Arc, time::Duration};

pub const STRIPE_API_VERSION: &str = "2026-09-30.endive";
const MAX_PROVIDER_BYTES: usize = 256 * 1024;
const MAX_WEBHOOK_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub(crate) struct StripeBilling {
    key: Arc<String>,
    webhook_secret: Arc<String>,
    pub(crate) offers: Vec<RechargeOffer>,
    pub(crate) livemode: bool,
    public_base: String,
    api_base: String,
    client: reqwest::Client,
}
impl std::fmt::Debug for StripeBilling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StripeBilling")
            .field("livemode", &self.livemode)
            .field("offers", &self.offers.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn valid_checkout_url(raw: &str) -> bool {
    if raw.len() > 4096
        || !raw.is_ascii()
        || raw.bytes().any(|b| {
            b.is_ascii_control()
                || b.is_ascii_whitespace()
                || matches!(
                    b,
                    b'\\' | b'"' | b'`' | b'<' | b'>' | b'|' | b'^' | b'{' | b'}'
                )
        })
    {
        return false;
    }
    reqwest::Url::parse(raw).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("checkout.stripe.com")
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url
                .path()
                .strip_prefix("/c/pay/")
                .is_some_and(|id| !id.is_empty())
    })
}
fn provider_id(id: &str, prefix: &str) -> bool {
    id.starts_with(prefix)
        && id.len() > prefix.len()
        && id.len() <= 255
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| "billingProviderDataInvalid".into())
}
fn env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("billingConfigurationMissing:{name}"))
}

fn loopback_test_return_flag(value: Option<&str>) -> Result<bool, String> {
    match value {
        None | Some("0" | "false") => Ok(false),
        Some("1" | "true") => Ok(true),
        _ => Err("billingConfigurationInvalid:MIR2_STRIPE_ALLOW_LOOPBACK_TEST_RETURN".into()),
    }
}

fn valid_return_base(raw: &str, livemode: bool, allow_loopback_test: bool) -> bool {
    if raw
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return false;
    }
    let Ok(base) = reqwest::Url::parse(raw) else {
        return false;
    };
    if base.host_str().is_none()
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return false;
    }
    if base.scheme() == "https" {
        return true;
    }
    // The Stripe CLI forwards signed sandbox events without exposing a server.
    // Only an explicit test-mode opt-in permits an HTTP cosmetic return page.
    // Check the original authority too: URL parsing normalizes numeric aliases.
    if livemode || !allow_loopback_test || base.scheme() != "http" {
        return false;
    }
    let Some(authority) = raw
        .strip_prefix("http://")
        .and_then(|s| s.split('/').next())
    else {
        return false;
    };
    ["127.0.0.1", "[::1]"].into_iter().any(|host| {
        authority == host
            || authority.strip_prefix(host).is_some_and(|suffix| {
                suffix.strip_prefix(':').is_some_and(|port| {
                    !port.is_empty()
                        && port.bytes().all(|b| b.is_ascii_digit())
                        && port.parse::<u16>().is_ok_and(|port| port != 0)
                })
            })
    })
}

impl StripeBilling {
    pub(crate) fn validate_store(&self, config: &SimulationConfig) -> Result<(), String> {
        if config.account_store_path.is_none() && config.account_store_database_url.is_none() {
            return Err("billingDurableStoreRequired".into());
        }
        if config.billing_monthly_card_credit_price.is_none() {
            return Err("billingMonthlyCardPriceRequired".into());
        }
        if self.livemode
            && (config.account_store_database_url.is_none()
                || config.account_store_database_mode
                    != mir2_simulation::AccountStoreDatabaseMode::SourceOfTruth)
        {
            return Err("billingLiveSourceStoreRequired".into());
        }
        Ok(())
    }
    pub(crate) fn from_env() -> Result<Option<Self>, String> {
        match std::env::var("MIR2_STRIPE_ENABLED").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("0") | Ok("false") => return Ok(None),
            Ok("1") | Ok("true") => {}
            _ => return Err("billingConfigurationInvalid:MIR2_STRIPE_ENABLED".into()),
        }
        let key = env("MIR2_STRIPE_SECRET_KEY")?;
        let livemode = if key.starts_with("sk_live_") || key.starts_with("rk_live_") {
            true
        } else if key.starts_with("sk_test_") || key.starts_with("rk_test_") {
            false
        } else {
            return Err("billingStripeKeyInvalid".into());
        };
        if key.len() < 24 || key.chars().any(char::is_whitespace) {
            return Err("billingStripeKeyInvalid".into());
        }
        let secret = env("MIR2_STRIPE_WEBHOOK_SECRET")?;
        if !secret.starts_with("whsec_")
            || secret.len() < 24
            || secret.chars().any(char::is_whitespace)
        {
            return Err("billingWebhookSecretInvalid".into());
        }
        let public_base = env("MIR2_BILLING_PUBLIC_BASE_URL")?;
        let loopback_setting = match std::env::var("MIR2_STRIPE_ALLOW_LOOPBACK_TEST_RETURN") {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(_) => {
                return Err(
                    "billingConfigurationInvalid:MIR2_STRIPE_ALLOW_LOOPBACK_TEST_RETURN".into(),
                )
            }
        };
        let allow_loopback_test = loopback_test_return_flag(loopback_setting.as_deref())?;
        if !valid_return_base(&public_base, livemode, allow_loopback_test) {
            return Err("billingPublicUrlInvalid".into());
        }
        let offers: Vec<RechargeOffer> = serde_json::from_str(&env("MIR2_STRIPE_RECHARGE_OFFERS")?)
            .map_err(|_| "billingOffersInvalid")?;
        if offers.is_empty() || offers.len() > 12 {
            return Err("billingOffersInvalid".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for offer in &offers {
            offer.validate()?;
            if offer.amount_minor > 99_999_999 || !ids.insert(&offer.id) {
                return Err("billingOffersInvalid".into());
            }
            // Stripe charges ISK/UGX in legacy hundredths, with no fractions.
            if matches!(offer.currency.as_str(), "isk" | "ugx") && offer.amount_minor % 100 != 0 {
                return Err("billingOffersInvalid".into());
            }
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| "billingProviderUnavailable")?;
        Ok(Some(Self {
            key: Arc::new(key),
            webhook_secret: Arc::new(secret),
            offers,
            livemode,
            public_base: public_base.trim_end_matches('/').into(),
            api_base: "https://api.stripe.com/v1".into(),
            client,
        }))
    }
    pub(crate) fn public_offers(&self) -> Vec<Value> {
        self.offers.iter().map(|offer| {
            // Stripe zero-decimal currencies are displayed without a fabricated
            // decimal conversion. Others use the server's minor-unit amount.
            let zero = matches!(offer.currency.as_str(), "bif"|"clp"|"djf"|"gnf"|"jpy"|"kmf"|"krw"|"mga"|"pyg"|"rwf"|"vnd"|"vuv"|"xaf"|"xof"|"xpf");
            let amount = if zero { offer.amount_minor.to_string() }
                else { format!("{}.{:02}", offer.amount_minor/100, offer.amount_minor%100) };
            json!({"id":offer.id,"label":offer.label,"currency":offer.currency,
                "amountMinor":offer.amount_minor,"amountLabel":format!("{} {amount}",offer.currency.to_uppercase()),"credits":offer.credits})
        }).collect()
    }
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}{path}", self.api_base))
            .bearer_auth(self.key.as_str())
            .header("Stripe-Version", STRIPE_API_VERSION)
    }
    async fn response(request: reqwest::RequestBuilder) -> Result<Value, String> {
        let mut response = request
            .send()
            .await
            .map_err(|_| "billingProviderResultUnconfirmed")?;
        if !response.status().is_success() {
            return Err("billingProviderUnavailable".into());
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_PROVIDER_BYTES as u64)
        {
            return Err("billingProviderDataInvalid".into());
        }
        let mut data = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "billingProviderResultUnconfirmed")?
        {
            if data.len().saturating_add(chunk.len()) > MAX_PROVIDER_BYTES {
                return Err("billingProviderDataInvalid".into());
            }
            data.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&data).map_err(|_| "billingProviderDataInvalid".into())
    }
    pub(crate) async fn checkout(
        &self,
        config: &SimulationConfig,
        account: &str,
        order: &RechargeOrder,
    ) -> Result<String, String> {
        self.validate_store(config)?;
        let session = if let Some(id) = &order.checkout_session_id {
            self.checkout_session(id).await?
        } else {
            if mir2_simulation::monthly_card::monthly_card_now_ms()
                .saturating_sub(order.created_at_ms)
                > 20 * 60 * 60 * 1000
            {
                // Stripe may prune idempotency keys after 24h. Never issue a
                // second session for an old, response-lost unbound order.
                return Err("billingResultUnconfirmed".into());
            }
            // Order survives response loss. The SAME key and frozen tuple are
            // retried; a different SKU cannot replace the order's payment.
            let params = vec![
                ("mode", "payment".to_owned()),
                // Endive uses Dashboard-managed methods; preserve our card-only
                // policy with its supported per-session eligibility filter.
                ("allowed_payment_method_types[0]", "card".to_owned()),
                (
                    "line_items[0][price_data][currency]",
                    order.offer.currency.clone(),
                ),
                (
                    "line_items[0][price_data][unit_amount]",
                    order.offer.amount_minor.to_string(),
                ),
                (
                    "line_items[0][price_data][product_data][name]",
                    order.offer.label.clone(),
                ),
                ("line_items[0][quantity]", "1".into()),
                ("client_reference_id", order.id.clone()),
                ("metadata[mir2_order_id]", order.id.clone()),
                ("metadata[mir2_account_id]", account.into()),
                (
                    "metadata[mir2_character_index]",
                    order.character_index.to_string(),
                ),
                (
                    "payment_intent_data[metadata][mir2_order_id]",
                    order.id.clone(),
                ),
                (
                    "payment_intent_data[metadata][mir2_account_id]",
                    account.into(),
                ),
                (
                    "payment_intent_data[metadata][mir2_character_index]",
                    order.character_index.to_string(),
                ),
                (
                    "success_url",
                    format!("{}/v1/billing/return?result=paid", self.public_base),
                ),
                (
                    "cancel_url",
                    format!("{}/v1/billing/return?result=cancelled", self.public_base),
                ),
            ];
            Self::response(
                self.request(reqwest::Method::POST, "/checkout/sessions")
                    .header("Idempotency-Key", format!("mir2-recharge-{}", order.id))
                    .form(&params),
            )
            .await?
        };
        self.validate_session(&session, account, order)?;
        let id = field(&session, "id")?.to_owned();
        let url = field(&session, "url")?.to_owned();
        if !valid_checkout_url(&url) {
            return Err("billingCheckoutUrlInvalid".into());
        }
        let config = config.clone();
        let account = account.to_owned();
        let order_id = order.id.clone();
        tokio::task::spawn_blocking(move || {
            config.bind_recharge_checkout(
                &account,
                &order_id,
                &id,
                mir2_simulation::monthly_card::monthly_card_now_ms(),
            )
        })
        .await
        .map_err(|_| "billingResultUnconfirmed")??;
        Ok(url)
    }
    async fn checkout_session(&self, id: &str) -> Result<Value, String> {
        if !provider_id(id, "cs_") {
            return Err("billingProviderDataInvalid".into());
        }
        Self::response(self.request(reqwest::Method::GET, &format!("/checkout/sessions/{id}")))
            .await
    }
    fn validate_session(
        &self,
        session: &Value,
        account: &str,
        order: &RechargeOrder,
    ) -> Result<(), String> {
        let metadata = &session["metadata"];
        if field(session, "object")? != "checkout.session"
            || field(session, "mode")? != "payment"
            || field(session, "client_reference_id")? != order.id
            || field(metadata, "mir2_order_id")? != order.id
            || field(metadata, "mir2_account_id")? != account
            || field(metadata, "mir2_character_index")? != order.character_index.to_string()
            || session["amount_total"].as_u64() != Some(order.offer.amount_minor)
            || field(session, "currency")? != order.offer.currency
            || session["livemode"].as_bool() != Some(order.livemode)
            || order.livemode != self.livemode
            || !provider_id(field(session, "id")?, "cs_")
            || order
                .checkout_session_id
                .as_deref()
                .is_some_and(|id| Some(id) != session["id"].as_str())
        {
            return Err("billingPaymentMismatch".into());
        }
        Ok(())
    }
    pub(crate) async fn webhook(
        &self,
        config: &SimulationConfig,
        signature: &str,
        raw: &[u8],
        now_ms: u64,
    ) -> Result<(), String> {
        self.validate_store(config)?;
        verify_signature(self.webhook_secret.as_str(), signature, raw, now_ms)?;
        let event: Value = serde_json::from_slice(raw).map_err(|_| "billingWebhookInvalid")?;
        if event["livemode"].as_bool() != Some(self.livemode)
            || !event["account"].is_null()
            || field(&event, "api_version")? != STRIPE_API_VERSION
            || !provider_id(field(&event, "id")?, "evt_")
        {
            return Err("billingWebhookInvalid".into());
        }
        let kind = field(&event, "type")?;
        if matches!(
            kind,
            "checkout.session.completed" | "checkout.session.async_payment_succeeded"
        ) {
            let session = self
                .checkout_session(field(&event["data"]["object"], "id")?)
                .await?;
            let (account, character, id) = metadata_identity(&session["metadata"])?;
            let account = account.to_owned();
            let id = id.to_owned();
            let lookup = config.clone();
            let lookup_account = account.clone();
            let lookup_id = id.clone();
            let order = tokio::task::spawn_blocking(move || {
                lookup.recharge_order_for_payment(&lookup_account, &lookup_id)
            })
            .await
            .map_err(|_| "billingResultUnconfirmed")??;
            self.validate_session(&session, &account, &order)?;
            if order.character_index != character {
                return Err("billingPaymentMismatch".into());
            }
            if session["payment_status"].as_str() != Some("paid") {
                return Ok(());
            }
            let payment_intent = field(&session, "payment_intent")?;
            if !provider_id(payment_intent, "pi_") {
                return Err("billingProviderDataInvalid".into());
            }
            let payment = VerifiedRechargePayment {
                session_id: field(&session, "id")?.into(),
                payment_intent_id: payment_intent.into(),
                amount_minor: order.offer.amount_minor,
                currency: order.offer.currency,
                livemode: self.livemode,
            };
            let config = config.clone();
            tokio::task::spawn_blocking(move || {
                config.confirm_recharge_payment(&account, &id, &payment, now_ms)
            })
            .await
            .map_err(|_| "billingResultUnconfirmed")??;
        } else if kind == "charge.refunded" || kind.starts_with("charge.dispute.") {
            let object = &event["data"]["object"];
            let charge_id = if kind == "charge.refunded" {
                field(object, "id")?
            } else {
                field(object, "charge")?
            };
            if !provider_id(charge_id, "ch_") {
                return Err("billingWebhookInvalid".into());
            }
            let charge = Self::response(
                self.request(reqwest::Method::GET, &format!("/charges/{charge_id}")),
            )
            .await?;
            if charge["object"].as_str() != Some("charge")
                || charge["id"].as_str() != Some(charge_id)
                || charge["livemode"].as_bool() != Some(self.livemode)
            {
                return Err("billingPaymentMismatch".into());
            }
            let payment_id = field(&charge, "payment_intent")?;
            if !provider_id(payment_id, "pi_") {
                return Err("billingWebhookInvalid".into());
            }
            let intent = Self::response(self.request(
                reqwest::Method::GET,
                &format!("/payment_intents/{payment_id}"),
            ))
            .await?;
            let (account, character, id) = metadata_identity(&intent["metadata"])?;
            let account = account.to_owned();
            let id = id.to_owned();
            let lookup = config.clone();
            let lookup_account = account.clone();
            let lookup_id = id.clone();
            let order = tokio::task::spawn_blocking(move || {
                lookup.recharge_order_for_payment(&lookup_account, &lookup_id)
            })
            .await
            .map_err(|_| "billingResultUnconfirmed")??;
            if intent["object"].as_str() != Some("payment_intent")
                || order.character_index != character
                || intent["id"].as_str() != Some(payment_id)
                || intent["livemode"].as_bool() != Some(self.livemode)
                || order.livemode != self.livemode
                || intent["amount"].as_u64() != Some(order.offer.amount_minor)
                || intent["currency"].as_str() != Some(order.offer.currency.as_str())
                || charge["amount"].as_u64() != Some(order.offer.amount_minor)
                || charge["currency"].as_str() != Some(order.offer.currency.as_str())
                || order
                    .payment_intent_id
                    .as_deref()
                    .is_some_and(|old| old != payment_id)
            {
                return Err("billingPaymentMismatch".into());
            }
            // An adverse event is retained for operator review. Never roll back
            // a whole save, erase purchased items or silently subtract spent points.
            let config = config.clone();
            let kind = kind.to_owned();
            let event_id = field(&event, "id")?.to_owned();
            tokio::task::spawn_blocking(move || {
                config.mark_recharge_review(&account, &id, &kind, &event_id, now_ms)
            })
            .await
            .map_err(|_| "billingResultUnconfirmed")??;
        }
        Ok(())
    }
}
fn metadata_identity(metadata: &Value) -> Result<(&str, i32, &str), String> {
    Ok((
        field(metadata, "mir2_account_id")?,
        field(metadata, "mir2_character_index")?
            .parse()
            .map_err(|_| "billingWebhookInvalid")?,
        field(metadata, "mir2_order_id")?,
    ))
}

pub(crate) fn verify_signature(
    secret: &str,
    header: &str,
    raw: &[u8],
    now_ms: u64,
) -> Result<(), String> {
    if raw.len() > MAX_WEBHOOK_BYTES || header.len() > 8192 {
        return Err("billingWebhookInvalid".into());
    }
    let mut timestamp = None;
    let mut signatures = Vec::new();
    for part in header.split(',') {
        let Some((key, value)) = part.trim().split_once('=') else {
            continue;
        };
        if key == "t" {
            if timestamp.is_some() {
                return Err("billingWebhookSignatureInvalid".into());
            }
            timestamp = Some(
                value
                    .parse::<u64>()
                    .map_err(|_| "billingWebhookSignatureInvalid")?,
            );
        } else if key == "v1"
            && signatures.len() < 16
            && value.len() == 64
            && value.bytes().all(|b| b.is_ascii_hexdigit())
        {
            let parsed = (0..32)
                .map(|i| u8::from_str_radix(&value[2 * i..2 * i + 2], 16))
                .collect::<Result<Vec<_>, _>>();
            if let Ok(bytes) = parsed {
                signatures.push(bytes);
            }
        }
    }
    let timestamp = timestamp.ok_or("billingWebhookSignatureInvalid")?;
    if timestamp.abs_diff(now_ms / 1000) > 300 {
        return Err("billingWebhookSignatureExpired".into());
    }
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(secret.as_bytes())
        .map_err(|_| "billingWebhookSignatureInvalid")?;
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(raw);
    if !signatures
        .iter()
        .any(|signature| mac.clone().verify_slice(signature).is_ok())
    {
        return Err("billingWebhookSignatureInvalid".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "stripe_billing_tests.rs"]
mod provider_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stripe_return_base_requires_https_unless_explicit_test_loopback() {
        for livemode in [false, true] {
            for opt_in in [false, true] {
                assert!(valid_return_base(
                    "https://billing.example.test",
                    livemode,
                    opt_in
                ));
                assert!(!valid_return_base(
                    "http://billing.example.test",
                    livemode,
                    opt_in
                ));
                for local in ["http://127.0.0.1:7121", "http://[::1]:7121/"] {
                    assert_eq!(
                        valid_return_base(local, livemode, opt_in),
                        !livemode && opt_in
                    );
                }
            }
        }
        for invalid in [
            "http://localhost:7121",
            "http://192.168.1.1:7121",
            "http://0.0.0.0:7121",
            "http://127.0.0.1.evil.test",
            "http://user@127.0.0.1:7121",
            "http://127.0.0.1:7121?redirect=1",
            "http://127.0.0.1:7121#fragment",
            "http://2130706433:7121",
            "http://0x7f000001:7121",
            "http://127.1:7121",
            "http://127.0.0.1:0",
            "http://127.0.0.1:65536",
            "http://[::1]:7121\\path",
            "http://127.0.0.1:7121\n",
            " https://billing.example.test",
            "https://user:password@billing.example.test",
            "file:///C:/return",
            "not-a-url",
        ] {
            assert!(!valid_return_base(invalid, false, true), "{invalid}");
        }
    }

    #[test]
    fn stripe_loopback_opt_in_is_disabled_by_default_and_rejects_invalid_flags() {
        for value in [None, Some("0"), Some("false")] {
            assert_eq!(loopback_test_return_flag(value), Ok(false));
        }
        for value in ["1", "true"] {
            assert_eq!(loopback_test_return_flag(Some(value)), Ok(true));
        }
        for value in ["", "yes", "TRUE", "2", " true "] {
            assert!(loopback_test_return_flag(Some(value)).is_err());
        }
    }

    fn signature(secret: &str, stamp: u64, raw: &[u8]) -> String {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(stamp.to_string().as_bytes());
        mac.update(b".");
        mac.update(raw);
        let digest = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<String>();
        format!("t={stamp},v1={digest}")
    }
    #[test]
    fn stripe_signature_rejects_tamper_stale_future_wrong_secret_and_duplicate_timestamp() {
        let raw = br#"{"id":"evt_fixture"}"#;
        let header = signature("fixture-only-secret", 1000, raw);
        assert!(verify_signature("fixture-only-secret", &header, raw, 1_000_000).is_ok());
        for (secret, body, now) in [
            ("wrong", raw.as_slice(), 1_000_000),
            ("fixture-only-secret", b"{}".as_slice(), 1_000_000),
            ("fixture-only-secret", raw.as_slice(), 1_301_000),
            ("fixture-only-secret", raw.as_slice(), 699_000),
        ] {
            assert!(verify_signature(secret, &header, body, now).is_err());
        }
        assert!(
            verify_signature("fixture-only-secret", &(header + ",t=1000"), raw, 1_000_000).is_err()
        );
    }
    #[test]
    fn stripe_checkout_urls_cannot_open_untrusted_host_credentials_or_shell_scheme() {
        assert!(valid_checkout_url(
            "https://checkout.stripe.com/c/pay/cs_test_fixture#opaque"
        ));
        for bad in [
            "http://checkout.stripe.com/c/pay/a",
            "https://checkout.stripe.com.evil.test/c/pay/a",
            "https://attacker@checkout.stripe.com/c/pay/a",
            "https://checkout.stripe.com:444/c/pay/a",
            "file:///C:/test.exe",
            "https://evil.test/c/pay/a",
            "https://checkout.stripe.com.evil/c/pay/a",
            "https://checkout.stripe.com/c/pay/",
            "https://checkout.stripe.com/c/pay/a\n",
            "https://checkout.stripe.com/c/pay/a#opaque\"argument",
            "https://checkout.stripe.com\\@evil.test/c/pay/a",
        ] {
            assert!(!valid_checkout_url(bad), "{bad}");
        }
    }
}
