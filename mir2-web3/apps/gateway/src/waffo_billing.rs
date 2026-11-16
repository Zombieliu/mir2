//! WAFFO Merchant hosted Checkout. Only verified provider responses can fund
//! the existing game ledger; browser redirects never confirm a payment.
use jsonwebtoken::{DecodingKey, EncodingKey};
use mir2_simulation::{
    billing::{FiatPaymentProvider, RechargeOffer, RechargeOrder, VerifiedRechargePayment},
    billing_provenance::VerifiedPaymentRevenue,
    monthly_card::monthly_card_now_ms,
    SimulationConfig,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

#[path = "waffo_billing_protocol.rs"]
mod protocol;

const PROVIDER: FiatPaymentProvider = FiatPaymentProvider::Waffo;
const MAX_RESPONSE_BYTES: usize = 256 * 1024;

#[derive(Clone)]
pub(crate) struct WaffoBilling {
    pub(crate) offers: Vec<RechargeOffer>,
    pub(crate) livemode: bool,
    merchant_id: String,
    api_key: Arc<String>,
    signing_key: EncodingKey,
    verifying_key: DecodingKey,
    public_base: String,
    api_base: String,
    client: reqwest::Client,
}
impl std::fmt::Debug for WaffoBilling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaffoBilling")
            .field("livemode", &self.livemode)
            .field("offers", &self.offers.len())
            .finish_non_exhaustive()
    }
}
fn required(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("billingConfigurationMissing:{name}"))
}
fn pem(name: &str) -> Result<Vec<u8>, String> {
    let file = std::env::var(format!("{name}_FILE")).ok();
    if let Some(path) = file {
        if std::env::var_os(name).is_some() {
            return Err(format!("billingConfigurationInvalid:{name}"));
        }
        return std::fs::read(path).map_err(|_| format!("billingConfigurationInvalid:{name}"));
    }
    Ok(required(name)?.replace("\\n", "\n").into_bytes())
}
fn field<'a>(data: &'a Value, name: &str) -> Result<&'a str, String> {
    data.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| "billingProviderDataInvalid".into())
}
fn timestamp(raw: &str) -> Result<u64, String> {
    let value = time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
        .map_err(|_| "billingProviderDataInvalid")?
        .unix_timestamp_nanos();
    u64::try_from(value / 1_000_000).map_err(|_| "billingProviderDataInvalid".into())
}
fn requested_at(ms: u64) -> Result<String, String> {
    time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(ms) * 1_000_000)
        .map_err(|_| "billingProviderDataInvalid")?
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| "billingProviderDataInvalid".into())
}
fn merchant_order_id(order: &RechargeOrder) -> Result<&str, String> {
    order
        .id
        .strip_prefix("rch_")
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| "billingOrderMismatch".into())
}
fn payment_request_id(order: &RechargeOrder) -> Result<&str, String> {
    Ok(&merchant_order_id(order)?[..32])
}

impl WaffoBilling {
    pub(crate) fn from_env() -> Result<Option<Self>, String> {
        match std::env::var("MIR2_WAFFO_ENABLED").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("0" | "false") => return Ok(None),
            Ok("1" | "true") => {}
            _ => return Err("billingConfigurationInvalid:MIR2_WAFFO_ENABLED".into()),
        }
        let livemode = match required("MIR2_WAFFO_ENVIRONMENT")?.as_str() {
            "sandbox" => false,
            "production" => true,
            _ => return Err("billingConfigurationInvalid:MIR2_WAFFO_ENVIRONMENT".into()),
        };
        let merchant_id = required("MIR2_WAFFO_MERCHANT_ID")?;
        if !PROVIDER.valid_payment_id(&merchant_id) {
            return Err("billingWaffoMerchantInvalid".into());
        }
        let api_key = required("MIR2_WAFFO_API_KEY")?;
        if api_key.len() > 4096 || api_key.chars().any(char::is_whitespace) {
            return Err("billingWaffoApiKeyInvalid".into());
        }
        let signing_key = EncodingKey::from_rsa_pem(&pem("MIR2_WAFFO_PRIVATE_KEY")?)
            .map_err(|_| "billingWaffoPrivateKeyInvalid")?;
        let verifying_key = DecodingKey::from_rsa_pem(&pem("MIR2_WAFFO_PUBLIC_KEY")?)
            .map_err(|_| "billingWaffoPublicKeyInvalid")?;
        let public_base = required("MIR2_BILLING_PUBLIC_BASE_URL")?;
        let base = reqwest::Url::parse(&public_base).map_err(|_| "billingPublicUrlInvalid")?;
        if base.scheme() != "https"
            || base.host_str().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || public_base
                .chars()
                .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
        {
            return Err("billingPublicUrlInvalid".into());
        }
        let offers: Vec<RechargeOffer> =
            serde_json::from_str(&required("MIR2_WAFFO_RECHARGE_OFFERS")?)
                .map_err(|_| "billingOffersInvalid")?;
        let mut ids = std::collections::BTreeSet::new();
        if offers.is_empty() || offers.len() > 12 {
            return Err("billingOffersInvalid".into());
        }
        for offer in &offers {
            offer.validate()?;
            // Existing Credits and promotion quotes use integer USD cents.
            // Extend currency precision explicitly before offering other quotes.
            if offer.currency != "usd" || offer.amount_minor > 99_999_999 || !ids.insert(&offer.id)
            {
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
            offers,
            livemode,
            merchant_id,
            api_key: Arc::new(api_key),
            signing_key,
            verifying_key,
            public_base: public_base.trim_end_matches('/').into(),
            api_base: if livemode {
                "https://api.waffo.com"
            } else {
                "https://api-sandbox.waffo.com"
            }
            .into(),
            client,
        }))
    }
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
    pub(crate) fn public_offers(&self) -> Vec<Value> {
        self.offers.iter().map(|o| json!({"id":o.id,"label":o.label,"currency":o.currency,
            "amountMinor":o.amount_minor,"amountLabel":format!("USD {}",protocol::amount_string(o.amount_minor)),"credits":o.credits})).collect()
    }
    fn metadata(&self, account: &str, order: &RechargeOrder) -> String {
        json!({"provider":"waffo","environment":if self.livemode{"production"}else{"sandbox"},
            "accountId":account,"orderId":order.id,"characterIndex":order.character_index})
        .to_string()
    }
    async fn post(&self, path: &str, data: Value) -> Result<Value, String> {
        let raw = serde_json::to_vec(&data).map_err(|_| "billingProviderDataInvalid")?;
        let signature = protocol::sign(&raw, &self.signing_key)?;
        let mut response = self
            .client
            .post(format!("{}{path}", self.api_base))
            .header("Content-Type", "application/json")
            .header("X-API-KEY", self.api_key.as_str())
            .header("X-API-VERSION", "1.0.0")
            .header("X-SIGNATURE", signature)
            .body(raw)
            .send()
            .await
            .map_err(|_| "billingProviderResultUnconfirmed")?;
        if !response.status().is_success() {
            return Err("billingProviderResultUnconfirmed".into());
        }
        if response.headers().get_all("x-signature").iter().count() != 1 {
            return Err("billingProviderSignatureInvalid".into());
        }
        let signature = response
            .headers()
            .get("x-signature")
            .and_then(|h| h.to_str().ok())
            .ok_or("billingProviderSignatureInvalid")?
            .to_owned();
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "billingProviderResultUnconfirmed")?
        {
            if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err("billingProviderDataInvalid".into());
            }
            body.extend_from_slice(&chunk);
        }
        protocol::verify(&body, &signature, &self.verifying_key)?;
        let envelope: Value =
            serde_json::from_slice(&body).map_err(|_| "billingProviderDataInvalid")?;
        Ok(protocol::response_data(&envelope)?.clone())
    }
    async fn inquiry(&self, request: &str) -> Result<Value, String> {
        self.post("/api/v1/order/inquiry", json!({"paymentRequestId":request}))
            .await
    }
    fn validate_identity(
        &self,
        data: &Value,
        account: &str,
        order: &RechargeOrder,
    ) -> Result<(), String> {
        if order.provider != PROVIDER
            || order.livemode != self.livemode
            || field(data, "paymentRequestId")? != payment_request_id(order)?
            || field(data, "merchantOrderId")? != merchant_order_id(order)?
            || !PROVIDER.valid_payment_id(field(data, "acquiringOrderId")?)
            || order
                .checkout_session_id
                .as_deref()
                .is_some_and(|id| Some(id) != data["acquiringOrderId"].as_str())
        {
            return Err("billingPaymentMismatch".into());
        }
        let _ = account;
        Ok(())
    }
    fn validate_inquiry(
        &self,
        data: &Value,
        account: &str,
        order: &RechargeOrder,
    ) -> Result<(), String> {
        self.validate_identity(data, account, order)?;
        if field(&data["merchantInfo"], "merchantId")? != self.merchant_id
            || field(&data["userInfo"], "userId")? != account
            || field(data, "extendInfo")? != self.metadata(account, order)
            || field(data, "orderCurrency")? != "USD"
            || order.offer.currency != "usd"
            || protocol::amount_minor(field(data, "orderAmount")?)? != order.offer.amount_minor
        {
            return Err("billingPaymentMismatch".into());
        }
        Ok(())
    }
    pub(crate) async fn checkout(
        &self,
        config: &SimulationConfig,
        account: &str,
        order: &RechargeOrder,
    ) -> Result<Option<String>, String> {
        self.validate_store(config)?;
        if order.provider != PROVIDER || order.livemode != self.livemode {
            return Err("billingPaymentMismatch".into());
        }
        let claim_config = config.clone();
        let claim_account = account.to_owned();
        let claim_id = order.id.clone();
        let first = tokio::task::spawn_blocking(move || {
            claim_config.begin_recharge_provider_request(
                &claim_account,
                &claim_id,
                PROVIDER,
                monthly_card_now_ms(),
            )
        })
        .await
        .map_err(|_| "billingResultUnconfirmed")??;
        let data = if first {
            self.post("/api/v1/order/create", json!({
                "paymentRequestId":payment_request_id(order)?,"merchantOrderId":merchant_order_id(order)?,
                "orderAmount":protocol::amount_string(order.offer.amount_minor),"orderCurrency":"USD",
                "orderDescription":format!("Numeron {} Credits",order.offer.credits),"orderRequestedAt":requested_at(order.created_at_ms)?,
                "notifyUrl":format!("{}/v1/billing/waffo/webhook",self.public_base),
                "successRedirectUrl":format!("{}/v1/billing/return",self.public_base),
                "failedRedirectUrl":format!("{}/v1/billing/return",self.public_base),
                "cancelRedirectUrl":format!("{}/v1/billing/return",self.public_base),
                "merchantInfo":{"merchantId":self.merchant_id},"userInfo":{"userId":account,"userTerminal":"WEB"},
                "paymentInfo":{"productName":"ONE_TIME_PAYMENT"},"extendInfo":self.metadata(account,order)
            })).await?
        } else {
            // A timeout, restart, or concurrent retry can only inquire. Never
            // send a second create while the original result is unknown.
            let data = self.inquiry(payment_request_id(order)?).await?;
            self.validate_inquiry(&data, account, order)?;
            if field(&data, "orderStatus")? == "PAY_SUCCESS" {
                self.settle(config, account, order, &data, monthly_card_now_ms())
                    .await?;
                return Ok(None);
            }
            data
        };
        self.validate_identity(&data, account, order)?;
        if field(&data, "orderStatus")? == "ORDER_CLOSE" {
            return Err("billingProviderRejected".into());
        }
        let action: Value = serde_json::from_str(field(&data, "orderAction")?)
            .map_err(|_| "billingProviderDataInvalid")?;
        let url = field(&action, "webUrl")?.to_owned();
        if !protocol::valid_checkout_url(&url) {
            return Err("billingCheckoutUrlInvalid".into());
        }
        let config = config.clone();
        let account = account.to_owned();
        let id = order.id.clone();
        let acquiring = field(&data, "acquiringOrderId")?.to_owned();
        tokio::task::spawn_blocking(move || {
            config.bind_recharge_provider_checkout(
                &account,
                &id,
                PROVIDER,
                &acquiring,
                monthly_card_now_ms(),
            )
        })
        .await
        .map_err(|_| "billingResultUnconfirmed")??;
        Ok(Some(url))
    }
    async fn settle(
        &self,
        config: &SimulationConfig,
        account: &str,
        order: &RechargeOrder,
        data: &Value,
        now: u64,
    ) -> Result<(), String> {
        self.validate_inquiry(data, account, order)?;
        if field(data, "orderStatus")? != "PAY_SUCCESS" {
            return Ok(());
        }
        let completed = timestamp(field(data, "orderCompletedAt")?)?;
        if completed.saturating_add(999) < order.created_at_ms || completed > now {
            return Err("billingPaymentMismatch".into());
        }
        let completed = completed.max(order.created_at_ms);
        let acquiring = field(data, "acquiringOrderId")?.to_owned();
        let payment = VerifiedRechargePayment {
            session_id: acquiring.clone(),
            payment_intent_id: acquiring.clone(),
            amount_minor: order.offer.amount_minor,
            currency: order.offer.currency.clone(),
            livemode: self.livemode,
        };
        let revenue = VerifiedPaymentRevenue {
            provider: "waffo".into(),
            source_event_id: format!("payment:{acquiring}"),
            order_id: order.id.clone(),
            checkout_session_id: acquiring.clone(),
            payment_intent_id: acquiring.clone(),
            livemode: self.livemode,
            currency: order.offer.currency.clone(),
            gross_amount_minor: order.offer.amount_minor,
            // The payment API does not certify indirect tax. Preserve this
            // uncertainty; it cannot silently create eligible commission.
            indirect_tax_minor: None,
            discount_minor: None,
            paid_at_ms: completed,
            observed_at_ms: now,
            evidence_ref: format!(
                "waffo:order:{acquiring}:request:{}",
                payment_request_id(order)?
            ),
        };
        let config = config.clone();
        let account = account.to_owned();
        let id = order.id.clone();
        tokio::task::spawn_blocking(move || {
            config
                .confirm_recharge_provider_payment(&account, &id, PROVIDER, &payment, completed)?;
            config.record_verified_payment_revenue(&account, &revenue)
        })
        .await
        .map_err(|_| "billingResultUnconfirmed")?
    }
    async fn order_from_notification(
        &self,
        config: &SimulationConfig,
        data: &Value,
    ) -> Result<(String, RechargeOrder), String> {
        let meta: Value = serde_json::from_str(field(data, "extendInfo")?)
            .map_err(|_| "billingWebhookInvalid")?;
        let account = field(&meta, "accountId")?.to_owned();
        let id = field(&meta, "orderId")?.to_owned();
        let lookup = config.clone();
        let lookup_account = account.clone();
        let order = tokio::task::spawn_blocking(move || {
            lookup.recharge_order_for_payment(&lookup_account, &id)
        })
        .await
        .map_err(|_| "billingResultUnconfirmed")??;
        self.validate_inquiry(data, &account, &order)?;
        Ok((account, order))
    }
    pub(crate) fn sign_ack(&self, raw: &[u8]) -> Result<String, String> {
        protocol::sign(raw, &self.signing_key)
    }
    pub(crate) async fn webhook(
        &self,
        config: &SimulationConfig,
        signature: &str,
        raw: &[u8],
        now: u64,
    ) -> Result<(), String> {
        self.validate_store(config)?;
        if raw.len() > 64 * 1024 {
            return Err("billingWebhookInvalid".into());
        }
        protocol::verify(raw, signature, &self.verifying_key)?;
        let event: Value = serde_json::from_slice(raw).map_err(|_| "billingWebhookInvalid")?;
        let result = &event["result"];
        match field(&event, "eventType")? {
            "PAYMENT_NOTIFICATION" => {
                let (account, order) = self.order_from_notification(config, result).await?;
                let current = self.inquiry(payment_request_id(&order)?).await?;
                self.validate_inquiry(&current, &account, &order)?;
                if current["acquiringOrderId"] != result["acquiringOrderId"] {
                    return Err("billingPaymentMismatch".into());
                }
                self.settle(config, &account, &order, &current, now).await
            }
            "REFUND_NOTIFICATION" | "CHARGEBACK_NOTIFICATION" => {
                let refund = field(&event, "eventType")? == "REFUND_NOTIFICATION";
                let request = field(
                    result,
                    if refund {
                        "origPaymentRequestId"
                    } else {
                        "originalPaymentRequestId"
                    },
                )?;
                if request.len() != 32 || !request.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("billingWebhookInvalid".into());
                }
                let current = self.inquiry(request).await?;
                let (account, order) = self.order_from_notification(config, &current).await?;
                let event_id = field(
                    result,
                    if refund {
                        "refundRequestId"
                    } else {
                        "chargebackId"
                    },
                )?;
                if !PROVIDER.valid_event_id(event_id) {
                    return Err("billingWebhookInvalid".into());
                }
                if refund {
                    if field(result, "acquiringOrderId")? != field(&current, "acquiringOrderId")? {
                        return Err("billingPaymentMismatch".into());
                    }
                    if !matches!(
                        field(result, "refundStatus")?,
                        "ORDER_PARTIALLY_REFUNDED" | "ORDER_FULLY_REFUNDED"
                    ) {
                        return Ok(());
                    }
                } else if field(result, "merchantId")? != self.merchant_id
                    || field(result, "merchantOrderId")? != merchant_order_id(&order)?
                    || field(result, "originalOrderId")? != field(&current, "acquiringOrderId")?
                {
                    return Err("billingPaymentMismatch".into());
                }
                // Signed adverse facts cause a sticky hold. Refund amounts and
                // chargeback lifecycles need operator reconciliation; don't
                // fabricate a cumulative reversal or rewind a player's save.
                let config = config.clone();
                let id = order.id;
                let event_id = format!("{}:{event_id}", if refund { "refund" } else { "dispute" });
                tokio::task::spawn_blocking(move || {
                    config.mark_recharge_provider_review(
                        &account,
                        &id,
                        PROVIDER,
                        if refund {
                            "refund.waffo"
                        } else {
                            "charge.dispute.waffo"
                        },
                        &event_id,
                        now,
                    )
                })
                .await
                .map_err(|_| "billingResultUnconfirmed")?
            }
            _ => Err("billingWebhookUnsupported".into()),
        }
    }
}

#[cfg(test)]
#[path = "waffo_billing_tests.rs"]
mod tests;
