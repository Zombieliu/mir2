//! Restricted browser handoff: one recharge capability, never a game login.
use super::*;
use crate::sui_usdc_billing::{self as usdc, UsdcBilling};
use mir2_simulation::sui_usdc_billing::{UsdcOrderState, UsdcPaymentMode, UsdcRechargeOrder};

const HTML: &str = include_str!("sui_usdc_checkout.html");
const SCRIPT: &str = include_str!("sui_usdc_checkout.js");
const DUBHE_SCRIPT: &str = include_str!("sui_usdc_dubhe_wallet.bundle.js");
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
enum CheckoutOperation {
    Status {},
    Challenge { payer: String },
    BindPayer { payer: String, signature: String },
    BuildPay {},
    BuildPayBytes {},
    Submitted { digest: String },
}
pub(super) fn router() -> Router<WebState> {
    Router::new()
        .route("/v1/billing/sui-usdc/checkout", post(checkout))
        .route("/v1/billing/sui-usdc/pay", get(page))
        .route("/v1/billing/sui-usdc/pay.js", get(script))
        .route("/v1/billing/sui-usdc/dubhe-wallet.js", get(dubhe_script))
        .route("/v1/billing/sui-usdc/session", post(operation))
}
fn safe_response(status: StatusCode, value: Value) -> Response {
    (
        status,
        [
            ("cache-control", "no-store"),
            ("referrer-policy", "no-referrer"),
            ("x-content-type-options", "nosniff"),
        ],
        Json(value),
    )
        .into_response()
}
fn asset(body: &'static str, content: &'static str) -> Response {
    (StatusCode::OK,[("content-type",content),("cache-control","no-store"),("referrer-policy","no-referrer"),("x-content-type-options","nosniff"),("content-security-policy","default-src 'none'; script-src 'self'; connect-src 'self' wss://relay.walletconnect.com https://relay.walletconnect.com; img-src 'self' data:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'" )],body).into_response()
}
async fn page() -> Response {
    asset(HTML, "text/html; charset=utf-8")
}
async fn script() -> Response {
    asset(SCRIPT, "text/javascript; charset=utf-8")
}
async fn dubhe_script() -> Response {
    asset(DUBHE_SCRIPT, "text/javascript; charset=utf-8")
}

fn wallet_connect_project(value: Option<&str>) -> Option<&str> {
    value.filter(|value| value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn dubhe_wallet_config() -> Value {
    let configured = std::env::var("MIR2_SUI_USDC_WALLETCONNECT_PROJECT_ID").ok();
    let project = wallet_connect_project(configured.as_deref());
    json!({"available":project.is_some(),"projectId":project,"chain":"sui:testnet"})
}
fn error(error: String) -> Response {
    let code = match error.as_str() {
        "billingUnavailable"
        | "billingOfferMissing"
        | "billingCreditCapacityExceeded"
        | "billingRequestIdReused"
        | "billingCharacterMissing"
        | "billingAccountBanned"
        | "billingTooManyRequests" => error.as_str(),
        "usdcCheckoutExpired" => "usdcCheckoutExpired",
        "usdcPayerMismatch" | "usdcPayerInvalid" => "usdcPayerMismatch",
        "usdcInvoiceNotReady" => "usdcInvoiceNotReady",
        "usdcPaymentAlreadySubmitted" => "usdcPaymentAlreadySubmitted",
        "usdcWalletProofInvalid" => "usdcWalletProofInvalid",
        "usdcCheckoutAuthenticationRequired" => "usdcCheckoutAuthenticationRequired",
        _ => "usdcResultUnconfirmed",
    };
    safe_response(
        if code == "usdcCheckoutAuthenticationRequired" {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::BAD_REQUEST
        },
        json!({"error":code}),
    )
}
fn same_origin(headers: &HeaderMap, provider: &UsdcBilling) -> Result<(), String> {
    if headers.get("origin").and_then(|h| h.to_str().ok()) != Some(provider.origin.as_str()) {
        return Err("usdcCheckoutAuthenticationRequired".into());
    }
    Ok(())
}
fn capability(headers: &HeaderMap) -> Result<(&str, &str), String> {
    let cap = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or("usdcCheckoutAuthenticationRequired")?;
    let (id, token) = cap
        .split_once('.')
        .ok_or("usdcCheckoutAuthenticationRequired")?;
    if !id.strip_prefix("usdc_").is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) || token.len() != 43
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err("usdcCheckoutAuthenticationRequired".into());
    }
    Ok((id, token))
}
fn authorize(
    config: &GatewayConfig,
    headers: &HeaderMap,
    provider: &UsdcBilling,
) -> Result<(String, UsdcRechargeOrder, String), String> {
    same_origin(headers, provider)?;
    let (id, token) = capability(headers)?;
    let (account, order) = config
        .find_usdc_recharge_order(id)?
        .ok_or("usdcCheckoutAuthenticationRequired")?;
    let actual = usdc::sha(token.as_bytes());
    let equal = actual.len() == order.capability_hash.len()
        && actual
            .bytes()
            .zip(order.capability_hash.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0;
    if !equal {
        return Err("usdcCheckoutAuthenticationRequired".into());
    }
    if usdc::now() >= order.capability_expires_at_ms {
        return Err("usdcCheckoutExpired".into());
    }
    provider.ensure_domain(&order)?;
    Ok((account, order, actual))
}
fn browser_state(o: &UsdcRechargeOrder) -> Value {
    let mut state = json!({"order":usdc::public_order(o),"payer":o.payer,"merchant":o.domain.merchant,"coinType":o.domain.coin_type,"chain":"sui:testnet","decimals":6,"testPayment":true,"gasPayer":"wallet","invoiceId":o.invoice.as_ref().map(|i|i.invoice_id.as_str()),"chainConfirmed":o.receipt.is_some()||o.transfer_receipt.is_some(),"gameDelivered":o.applied.is_some(),"payExpired":usdc::now()>=o.expires_at_ms,"dubheWallet":dubhe_wallet_config()});
    if o.payment_mode == UsdcPaymentMode::DirectTransfer {
        let available = o.state == UsdcOrderState::PayerBound
            && o.transfer_receipt.is_none()
            && o.applied.is_none()
            && o.payment_hint.is_none()
            && usdc::now() < o.expires_at_ms;
        let mut url = reqwest::Url::parse("https://wallet.dubhe.obelisk.build/send")
            .expect("fixed wallet origin");
        url.query_pairs_mut().extend_pairs([
            ("recipient", o.domain.merchant.clone()),
            ("coinType", o.domain.coin_type.clone()),
            (
                "amount",
                format!(
                    "{}.{:06}",
                    o.amount_micro / 1_000_000,
                    o.amount_micro % 1_000_000
                ),
            ),
            ("requestId", o.id.clone()),
            ("request", "1".to_string()),
        ]);
        state["paymentMode"] = json!("direct_transfer");
        state["directTransfer"] = json!({"available":available,"payerVerified":true,
            "payer":o.payer,"merchant":o.domain.merchant,"chain":"sui:testnet","coinType":o.domain.coin_type,
            "amountMicros":o.amount_micro.to_string(),"expiresAtMs":o.expires_at_ms,
            "walletRequestUrl":if available {Some(url.as_str())} else {None}});
    }
    state
}
async fn checkout(
    State(state): State<WebState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let verified = match http_identity(&state.identity, &headers).await {
        Ok(v) => v,
        Err(_) => {
            return safe_response(
                StatusCode::UNAUTHORIZED,
                json!({"error":"billingAuthenticationRequired"}),
            )
        }
    };
    let request: CheckoutRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            return safe_response(
                StatusCode::BAD_REQUEST,
                json!({"error":"billingRequestInvalid"}),
            )
        }
    };
    let result = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        rate_limit(
            state.session_cache.as_ref(),
            &verified.account_id,
            "usdcCheckout",
        )?;
        let config = state.config.clone();
        let provider = UsdcBilling::from_env()?.ok_or("billingUnavailable")?;
        same_origin(&headers, &provider)?;
        let credential = state.identity.verified_wallet_credential(&verified).ok();
        let (url, order) = provider.checkout_for_wallet(
            &config,
            &verified.account_id,
            request.character_index,
            &request.order_request_id,
            &request.offer_id,
            credential
                .as_ref()
                .map(|c| (c.credential_id.as_str(), c.credential_subject.as_str())),
        )?;
        Ok(json!({"orderId":order.id,"checkoutUrl":url,"requestId":request.request_id}))
    })
    .await
    .unwrap_or_else(|_| Err("usdcResultUnconfirmed".into()));
    match result {
        Ok(v) => safe_response(StatusCode::OK, v),
        Err(e) => error(e),
    }
}
async fn operation(
    State(state): State<WebState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if body.len() > 8192 {
        return safe_response(
            StatusCode::BAD_REQUEST,
            json!({"error":"usdcRequestInvalid"}),
        );
    }
    let op: CheckoutOperation = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            return safe_response(
                StatusCode::BAD_REQUEST,
                json!({"error":"usdcRequestInvalid"}),
            )
        }
    };
    let permit = match checkout_permit() {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    let result=tokio::task::spawn_blocking(move||->Result<Value,String>{
        let _permit=permit;
        let config=state.config.clone();let provider=UsdcBilling::from_env()?.ok_or("billingUnavailable")?;
        let(account,order,cap_hash)=authorize(&config,&headers,&provider)?;
        rate_limit(state.session_cache.as_ref(),&account,if matches!(&op,CheckoutOperation::Status{}){"status"}else{"usdcWallet"})?;
        match op {
            CheckoutOperation::Status{}=>Ok(browser_state(&order)),
            CheckoutOperation::Challenge{payer}=>{
                if !order.payment_mode.is_invoice(){return Err("usdcOperationRejected".into());}
                if usdc::now()>=order.expires_at_ms{return Err("usdcCheckoutExpired".into());}
                if order.payer.as_ref().is_some_and(|p|p!=&payer){return Err("usdcPayerMismatch".into());}
                Ok(json!({"message":usdc::wallet_message(&order,&payer,&provider.origin)?,"state":browser_state(&order)}))
            }
            CheckoutOperation::BindPayer{payer,signature}=>{
                if !order.payment_mode.is_invoice(){return Err("usdcOperationRejected".into());}
                if signature.len()>2048{return Err("usdcWalletProofInvalid".into());}
                let message=usdc::wallet_message(&order,&payer,&provider.origin)?;
                let proof=provider.driver.call("verifyWalletProof",json!({"message":message,"signature":signature,"address":payer}))?;
                if proof["verified"]!=true||proof["address"]!=payer{return Err("usdcWalletProofInvalid".into());}
                let bound=config.bind_usdc_recharge_payer(&account,&order.id,&payer,&cap_hash,usdc::now())?;
                Ok(browser_state(&bound))
            }
            operation @ (CheckoutOperation::BuildPay{}|CheckoutOperation::BuildPayBytes{})=>{
                if !order.payment_mode.is_invoice(){return Err("usdcOperationRejected".into());}
                if usdc::now()>=order.expires_at_ms{return Err("usdcCheckoutExpired".into());}
                if order.payment_hint.is_some()||order.has_payment_receipt(){return Err("usdcPaymentAlreadySubmitted".into());}
                let invoice=order.invoice.as_ref().ok_or("usdcInvoiceNotReady")?;
                let bytes=matches!(operation,CheckoutOperation::BuildPayBytes{});
                let built=provider.driver.call(if bytes{"buildPayBytes"}else{"buildPay"},json!({"invoiceId":invoice.invoice_id,"order":usdc::chain_order(&order)?}))?;
                let field=if bytes{"transactionBytesB64"}else{"transactionJson"};
                if built["invoiceId"]!=invoice.invoice_id||built["payer"]!=order.payer.as_deref().unwrap_or("")||built["gasPayer"]!="wallet"||built[field].as_str().is_none_or(|s|s.is_empty()||s.len()>256*1024){return Err("usdcDriverReplyInvalid".into());}
                Ok(json!({(field):built[field],"state":browser_state(&order)}))
            }
            CheckoutOperation::Submitted{digest}=>{
                // Native transfers are discovered independently. A browser hint
                // must not disable the QR or masquerade as a submitted payment.
                if !order.payment_mode.is_invoice(){return Err("usdcOperationRejected".into());}
                let submitted=config.submit_usdc_payment_hint(&account,&order.id,&cap_hash,&digest,usdc::now())?;
                Ok(browser_state(&submitted))
            }
        }
    }).await.unwrap_or_else(|_|Err("usdcResultUnconfirmed".into()));
    match result {
        Ok(v) => safe_response(StatusCode::OK, v),
        Err(e) => error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn direct_order() -> UsdcRechargeOrder {
        use mir2_simulation::{
            billing::RechargeOffer,
            sui_usdc_billing::{UsdcDomain, TESTNET_CHAIN_IDENTIFIER, TESTNET_USDC_COIN_TYPE},
            SimulationConfig,
        };
        let config = SimulationConfig::default();
        let now = usdc::now();
        let mut order = config
            .prepare_usdc_recharge_order(
                "demo",
                config.default_character.index,
                "browser-direct-fixture",
                &RechargeOffer {
                    id: "test-usdc".into(),
                    label: "Test fixture".into(),
                    currency: "usd".into(),
                    amount_minor: 100,
                    credits: 100,
                },
                &UsdcDomain {
                    chain_identifier: TESTNET_CHAIN_IDENTIFIER.into(),
                    package_id: format!("0x{}", "1".repeat(64)),
                    config_id: format!("0x{}", "2".repeat(64)),
                    merchant: format!("0x{}", "3".repeat(64)),
                    coin_type: TESTNET_USDC_COIN_TYPE.into(),
                    rule_version: 1,
                },
                &"c".repeat(64),
                now + 1_800_000,
                now + 1_200_000,
                now,
            )
            .unwrap();
        order.payment_mode = UsdcPaymentMode::DirectTransfer;
        order.payer = Some(format!("0x{}", "4".repeat(64)));
        order.state = UsdcOrderState::PayerBound;
        order.validate().unwrap();
        order
    }
    #[test]
    fn native_checkout_card_exposes_only_frozen_public_fields_and_exact_wallet_url() {
        let order = direct_order();
        let state = browser_state(&order);
        assert_eq!(state["paymentMode"], "direct_transfer");
        assert_eq!(state["directTransfer"]["available"], true);
        assert_eq!(state["order"]["state"], "awaitingPayment");
        assert_eq!(
            state["directTransfer"]["payer"],
            order.payer.as_deref().unwrap()
        );
        let url = reqwest::Url::parse(
            state["directTransfer"]["walletRequestUrl"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            url.origin().ascii_serialization(),
            "https://wallet.dubhe.obelisk.build"
        );
        assert_eq!(url.path(), "/send");
        assert_eq!(url.query_pairs().count(), 5);
        let pairs: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(pairs["recipient"], order.domain.merchant);
        assert_eq!(pairs["amount"], "1.000000");
        assert_eq!(pairs["requestId"], order.id);
        assert_eq!(pairs["coinType"], order.domain.coin_type);
        assert_eq!(pairs["request"], "1");
        assert!(!state.to_string().contains(&order.capability_hash));
    }
    #[test]
    fn invoice_and_expired_native_orders_never_enable_direct_payment() {
        let mut order = direct_order();
        order.payment_mode = UsdcPaymentMode::Invoice;
        order.state = UsdcOrderState::Prepared;
        order.payer = None;
        let state = browser_state(&order);
        assert!(state.get("directTransfer").is_none());
        order.payment_mode = UsdcPaymentMode::DirectTransfer;
        order.state = UsdcOrderState::PayerBound;
        order.payer = Some(format!("0x{}", "4".repeat(64)));
        order.expires_at_ms = usdc::now();
        let state = browser_state(&order);
        assert_eq!(state["payExpired"], true);
        assert_eq!(state["directTransfer"]["available"], false);
        assert!(state["directTransfer"]["walletRequestUrl"].is_null());
        assert_eq!(state["order"]["state"], "expired");
    }
    #[test]
    fn capability_never_accepts_game_tokens_or_query_strings() {
        let mut h = HeaderMap::new();
        h.insert(
            "authorization",
            format!("Bearer usdc_{}.{}", "a".repeat(64), "x".repeat(43))
                .parse()
                .unwrap(),
        );
        assert!(capability(&h).is_ok());
        for v in [
            "Bearer game-session-token".to_string(),
            format!(
                "Bearer usdc_{}.{}?redirect=https://evil.invalid",
                "a".repeat(64),
                "x".repeat(43)
            ),
            format!("Bearer usdc_{}.{}", "A".repeat(64), "x".repeat(43)),
        ] {
            h.insert("authorization", v.parse().unwrap());
            assert!(capability(&h).is_err());
        }
    }
    #[test]
    fn payloads_reject_prices_finality_and_unknown_fields() {
        for v in [
            json!({"operation":"submitted","digest":"anything","paid":true}),
            json!({"operation":"bindPayer","payer":"0x1","signature":"sig","credits":100000}),
            json!({"operation":"buildPay","amountMicros":"1"}),
            json!({"operation":"buildPayBytes","payer":"0x1"}),
        ] {
            assert!(serde_json::from_value::<CheckoutOperation>(v).is_err());
        }
    }
    #[test]
    fn qr_project_configuration_is_public_but_never_a_url_or_secret() {
        assert!(wallet_connect_project(None).is_none());
        assert_eq!(
            wallet_connect_project(Some("0123456789abcdef0123456789abcdef")),
            Some("0123456789abcdef0123456789abcdef")
        );
        for invalid in [
            "",
            "https://evil.invalid",
            " short ",
            "0123456789abcdef0123456789abcdef?",
            "g123456789abcdef0123456789abcdef",
        ] {
            assert!(wallet_connect_project(Some(invalid)).is_none());
        }
        assert!(
            serde_json::from_value::<CheckoutOperation>(json!({"operation":"buildPayBytes"}))
                .is_ok()
        );
        assert!(DUBHE_SCRIPT.contains("sui_signAndExecuteTransaction"));
    }
    #[test]
    fn checkout_page_uses_no_game_bearer_or_remote_scripts() {
        assert!(!HTML.contains("https://"));
        assert!(SCRIPT.contains("sui:testnet"));
        assert!(SCRIPT.contains("history.replaceState"));
        assert!(!SCRIPT.contains("localStorage"));
        assert!(!SCRIPT.contains("innerHTML"));
        assert!(SCRIPT.contains("sui:signAndExecuteTransaction"));
    }
}
