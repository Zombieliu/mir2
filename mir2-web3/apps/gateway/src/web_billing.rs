//! Authenticated billing seams. Gameplay never waits for the Stripe network.
use super::*;
use crate::stripe_billing::StripeBilling;
use crate::fiat_billing::FiatBilling;
use mir2_simulation::billing::{FiatPaymentProvider, RechargeOrderState};
use mir2_simulation::monthly_card::monthly_card_now_ms;
#[path = "web_sui_usdc.rs"]
mod sui_usdc;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StatusRequest {
    #[serde(default)]
    pub include_payment_providers: bool,
    pub character_index: i32,
    pub request_id: u64,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CheckoutRequest {
    #[serde(default)]
    pub include_payment_providers: bool,
    #[serde(default)]
    pub provider: FiatPaymentProvider,
    pub character_index: i32,
    pub request_id: u64,
    pub order_request_id: String,
    pub offer_id: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BuyRequest {
    #[serde(default)]
    pub include_payment_providers: bool,
    pub character_index: i32,
    pub request_id: u64,
    pub order_request_id: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ActivateRequest {
    #[serde(default)]
    pub include_payment_providers: bool,
    pub character_index: i32,
    pub request_id: u64,
    pub item_unique_id: String,
}
#[derive(Debug, Clone)]
pub(super) enum Request {
    Status(StatusRequest),
    Checkout(CheckoutRequest),
    UsdcCheckout(CheckoutRequest),
    Buy(BuyRequest),
    Activate(ActivateRequest),
}
impl Request {
    fn include_payment_providers(&self) -> bool {
        match self {
            Self::Status(r) => r.include_payment_providers,
            Self::Checkout(r) | Self::UsdcCheckout(r) => r.include_payment_providers,
            Self::Buy(r) => r.include_payment_providers,
            Self::Activate(r) => r.include_payment_providers,
        }
    }
    fn character(&self) -> i32 {
        match self {
            Self::Status(r) => r.character_index,
            Self::Checkout(r) => r.character_index,
            Self::UsdcCheckout(r) => r.character_index,
            Self::Buy(r) => r.character_index,
            Self::Activate(r) => r.character_index,
        }
    }
    fn request_id(&self) -> u64 {
        match self {
            Self::Status(r) => r.request_id,
            Self::Checkout(r) => r.request_id,
            Self::UsdcCheckout(r) => r.request_id,
            Self::Buy(r) => r.request_id,
            Self::Activate(r) => r.request_id,
        }
    }
    fn operation(&self) -> &'static str {
        match self {
            Self::Status(_) => "status",
            Self::Checkout(_) => "checkout",
            Self::UsdcCheckout(_) => "usdcCheckout",
            Self::Buy(_) => "buyMonthlyCard",
            Self::Activate(_) => "activateMonthlyCard",
        }
    }
}
fn public_error(error: &str) -> &str {
    match error {
        "monthlyCardAccountBanned" => "billingAccountBanned",
        "monthlyCardCharacterMissing" | "billingCharacterSaveMissing" => "billingCharacterMissing",
        "monthlyCardRequestIdInvalid" => "billingRequestIdInvalid",
        "monthlyCardInsufficientCredit" | "insufficient game-shop credit at commit" => {
            "billingInsufficientCredit"
        }
        "monthlyCardShopUnavailable" => "billingMonthlyCardDisabled",
        "monthlyCardItemMissing" | "monthlyCardItemNotIssued" => "billingItemMissing",
        "monthlyCardItemInvalid" => "billingItemInvalid",
        "monthlyCardInventoryFull" => "billingInventoryFull",
        "billingAuthenticationRequired"
        | "billingAccountBanned"
        | "billingCharacterMissing"
        | "billingCharacterMismatch"
        | "billingOfferMissing"
        | "billingUnavailable"
        | "billingRequestIdInvalid"
        | "billingItemMissing"
        | "billingInventoryFull"
        | "billingInsufficientCredit"
        | "billingInsufficientCredits"
        | "billingCreditCapacityExceeded"
        | "billingMonthlyCardDisabled"
        | "billingCharacterOnline"
        | "billingTooManyRequests"
        | "billingOrderMismatch"
        | "billingItemInvalid" => error,
        _ => "billingResultUnconfirmed",
    }
}

#[cfg(test)]
#[path = "web_billing_tests.rs"]
mod tests;
fn reply(request: &Request, result: Result<Value, String>) -> Value {
    let mut payload = json!({"operation":request.operation(),"requestId":request.request_id(),"characterIndex":request.character()});
    match result {
        Ok(extra) => {
            for (key, value) in extra.as_object().into_iter().flatten() {
                payload[key] = value.clone();
            }
        }
        Err(error) => payload["error"] = json!(public_error(&error)),
    }
    json!({"type":"billing","payload":payload})
}
pub(super) fn error_reply(request: &Request, error: String) -> Value {
    reply(request, Err(error))
}
pub(super) struct SocketFence {
    authenticated: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
}
impl Drop for SocketFence {
    fn drop(&mut self) {
        self.authenticated.store(false, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}
pub(super) fn socket_fence(
    authenticated: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
) -> SocketFence {
    SocketFence {
        authenticated,
        generation,
    }
}
pub(super) async fn http_identity(
    identity: &Arc<IdentityService>,
    headers: &HeaderMap,
) -> Result<VerifiedIdentitySession, String> {
    let token = identity_bearer_token(headers).map_err(|_| "billingAuthenticationRequired")?;
    let identity = Arc::clone(identity);
    tokio::task::spawn_blocking(move || {
        identity
            .verify_session_token(&token)
            .map_err(|_| "billingAuthenticationRequired".to_string())
    })
    .await
    .map_err(|_| "billingAuthenticationRequired")?
}
pub(super) fn rate_limit(
    cache: &dyn crate::cache::GatewaySessionCache,
    account: &str,
    operation: &str,
) -> Result<(), String> {
    let count = cache
        .record_auth_attempt(&format!("billing:{account}:{operation}"), 60)?
        .0;
    if count
        > if matches!(operation, "status" | "activitiesStatus") {
            120
        } else {
            20
        }
    {
        return Err("billingTooManyRequests".into());
    }
    Ok(())
}
fn checkout_permit() -> Result<tokio::sync::OwnedSemaphorePermit, String> {
    static PERMITS: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
    PERMITS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(32)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| "billingTooManyRequests".into())
}
fn status_value(
    config: &GatewayConfig,
    account: &str,
    character: i32,
    provider: Option<&StripeBilling>,
    include_payment_providers: bool,
) -> Result<Value, String> {
    if let Some(provider) = provider {
        provider.validate_store(config)?;
    }
    let (credit, pending) = config.recharge_credit_status(account, character)?;
    let cards = config
        .list_billing_monthly_cards(account, character)?
        .into_iter()
        .map(|card| json!({"uniqueId":card.unique_id.to_string(),"label":card.label}))
        .collect::<Vec<_>>();
    let orders = config
        .recharge_orders(account, character)?
        .into_iter()
        .rev()
        .take(20)
        .map(|order| {
            let mut value = json!({"id":order.id,"state":order.state});
            if include_payment_providers { value["provider"] = json!(order.provider); }
            value
        })
        .collect::<Vec<_>>();
    let usdc = crate::sui_usdc_billing::UsdcBilling::from_env()?;
    if let Some(p) = &usdc {
        p.validate_store(config)?;
    }
    let usdc_enabled = usdc.as_ref().is_some_and(|p| p.enabled);
    let usdc_orders = config
        .usdc_recharge_orders(account, character)?
        .iter()
        .rev()
        .take(20)
        .map(crate::sui_usdc_billing::public_order)
        .collect::<Vec<_>>();
    let payment_providers = crate::fiat_billing::public_payment_providers(config, provider)?;
    let mut value =
        json!({"enabled":!payment_providers.is_empty()||usdc_enabled||config.billing_monthly_card_credit_price.is_some(),
        "checkoutEnabled":provider.is_some(),"credit":credit,"pendingCredit":pending,
        "offers":provider.map(StripeBilling::public_offers).unwrap_or_default(),
        "suiUsdcEnabled":usdc_enabled,
        "suiUsdcCheckoutOrigin":usdc.as_ref().map(|p|p.origin.as_str()),
        "suiUsdcOffers":usdc.as_ref().filter(|p|p.enabled).map(|p|p.public_offers()).unwrap_or_default(),
        "suiUsdcOrders":usdc_orders,
        "monthlyCardCreditPrice":config.billing_monthly_card_credit_price,
        "ownedMonthlyCards":cards,"orders":orders,
        "monthlyCard":config.refresh_monthly_card_status(account,monthly_card_now_ms())?});
    // Shipped clients deny unknown status/order fields. Registry is negotiated.
    if include_payment_providers { value["paymentProviders"] = json!(payment_providers); }
    Ok(value)
}
fn verified_account(
    identity: &IdentityService,
    verified: Option<&VerifiedIdentitySession>,
    authenticated: bool,
    account: Option<&str>,
) -> Result<String, String> {
    let account = account
        .filter(|_| authenticated)
        .ok_or("billingAuthenticationRequired")?;
    let verified = verified
        .filter(|v| v.account_id == account && v.expires_at_ms > monthly_card_now_ms())
        .ok_or("billingAuthenticationRequired")?;
    identity
        .touch_session(verified)
        .map_err(|_| "billingAuthenticationRequired")?;
    Ok(account.into())
}
fn durable(config: &GatewayConfig) -> Result<(), String> {
    if config.account_store_path.is_none() && config.account_store_database_url.is_none() {
        return Err("billingUnavailable".into());
    }
    Ok(())
}
fn inactive_lease(
    cache: &dyn crate::cache::GatewaySessionCache,
    account: &str,
    character: i32,
) -> Result<(crate::cache::GatewaySessionCacheKey, String), String> {
    let key = crate::cache::GatewaySessionCacheKey {
        account_id: account.into(),
        character_index: character,
    };
    if cache.get(&key).is_some() {
        return Err("billingCharacterOnline".into());
    }
    let mut bytes = [0u8; 32];
    rand::RngCore::try_fill_bytes(&mut rand::rngs::OsRng, &mut bytes)
        .map_err(|_| "billingResultUnconfirmed")?;
    let owner = format!(
        "billing-offline-{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    cache
        .acquire_route_lease(&key, &owner, 300)
        .map_err(|_| "billingCharacterOnline")?;
    if cache.get(&key).is_some() {
        let _ = cache.release_route_lease(&key, &owner);
        return Err("billingCharacterOnline".into());
    }
    Ok((key, owner))
}
fn release_confirmed_lease(
    cache: &dyn crate::cache::GatewaySessionCache,
    lease: Option<(crate::cache::GatewaySessionCacheKey, String)>,
    result: &Result<Value, String>,
) -> Result<(), String> {
    // Response loss does not imply rollback. Retain this temporary route fence
    // while Source may still be finishing. Durable revision checks and exact
    // unit/order receipts remain the final write and replay authority.
    if result
        .as_ref()
        .is_err_and(|e| public_error(e) == "billingResultUnconfirmed")
    {
        return Ok(());
    }
    if let Some((key, owner)) = lease {
        cache.release_route_lease(&key, &owner)?;
    }
    Ok(())
}

pub(super) async fn socket_action(
    session: &mut GatewaySession,
    identity: Arc<IdentityService>,
    verified: Option<&VerifiedIdentitySession>,
    authenticated: bool,
    account: Option<&str>,
    request: Request,
    sender: SharedWebSocketSender,
    cache: SharedGatewaySessionCache,
    socket_authenticated: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
) -> Result<(), String> {
    let account = match verified_account(&identity, verified, authenticated, account) {
        Ok(a) => a,
        Err(e) => {
            sender
                .send(Message::Text(reply(&request, Err(e)).to_string().into()))
                .await
                .map_err(|_| "billingSendFailed")?;
            return Ok(());
        }
    };
    let config = session.monthly_card_config()?.clone();
    let include_payment_providers = request.include_payment_providers();
    let provider = StripeBilling::from_env()?;
    rate_limit(cache.as_ref(), &account, request.operation())?;
    if let Request::UsdcCheckout(checkout) = &request {
        let result = tokio::task::block_in_place(|| -> Result<Value, String> {
            durable(&config)?;
            let usdc =
                crate::sui_usdc_billing::UsdcBilling::from_env()?.ok_or("billingUnavailable")?;
            let credential = verified.and_then(|v| identity.verified_wallet_credential(v).ok());
            let (url, _) = usdc.checkout_for_wallet(
                &config,
                &account,
                checkout.character_index,
                &checkout.order_request_id,
                &checkout.offer_id,
                credential
                    .as_ref()
                    .map(|c| (c.credential_id.as_str(), c.credential_subject.as_str())),
            )?;
            Ok(
                json!({"checkoutUrl":url,"status":status_value(&config,&account,checkout.character_index,provider.as_ref(), include_payment_providers)?}),
            )
        });
        sender
            .send(Message::Text(reply(&request, result).to_string().into()))
            .await
            .map_err(|_| "billingSendFailed")?;
        return Ok(());
    }
    if let Request::Checkout(checkout) = &request {
        let permit = checkout_permit()?;
        let prepared: Result<_, String> = tokio::task::block_in_place(|| {
            durable(&config)?;
            let selected = FiatBilling::from_env(checkout.provider)?.ok_or("billingUnavailable")?;
            selected.validate_store(&config)?;
            let offer = selected
                .offers()
                .iter()
                .find(|offer| offer.id == checkout.offer_id)
                .ok_or("billingOfferMissing")?;
            let order = config.prepare_recharge_order_for_provider(
                &account,
                checkout.character_index,
                &checkout.order_request_id,
                offer,
                monthly_card_now_ms(),
                selected.livemode(),
                selected.id(),
            )?;
            Ok((order, selected))
        });
        let (order, selected) = match prepared {
            Ok(tuple) => tuple,
            Err(e) => {
                sender
                    .send(Message::Text(reply(&request, Err(e)).to_string().into()))
                    .await
                    .map_err(|_| "billingSendFailed")?;
                return Ok(());
            }
        };
        let verified = verified.cloned().ok_or("billingAuthenticationRequired")?;
        let captured_generation = generation.load(Ordering::Acquire);
        tokio::spawn(async move {
            let _permit = permit;
            let result: Result<Value, String> = async {
                let url = if matches!(
                    order.state,
                    RechargeOrderState::PaidPending
                        | RechargeOrderState::Applied
                        | RechargeOrderState::Review
                ) {
                    None
                } else {
                    selected.checkout(&config, &account, &order).await?
                };
                let selected_id = selected.id();
                let status = tokio::task::spawn_blocking(move || {
                    status_value(&config, &account, order.character_index, provider.as_ref(), include_payment_providers)
                })
                .await
                .map_err(|_| "billingResultUnconfirmed")??;
                let mut value = json!({"checkoutUrl":url,"status":status});
                if include_payment_providers {
                    value["provider"] = json!(selected_id);
                    value["replayed"] = json!(value["checkoutUrl"].is_null());
                }
                Ok(value)
            }
            .await;
            if socket_authenticated.load(Ordering::Acquire)
                && generation.load(Ordering::Acquire) == captured_generation
                && identity.touch_session(&verified).is_ok()
            {
                let _ = sender
                    .send(Message::Text(reply(&request, result).to_string().into()))
                    .await;
            }
        });
        return Ok(());
    }
    let mut packets = Vec::new();
    let result = tokio::task::block_in_place(|| -> Result<Value, String> {
        durable(&config)?;
        let character = request.character();
        let active = session.active_identity();
        if active
            .as_ref()
            .is_some_and(|id| id.account_id != account || id.character_index != character)
        {
            return Err("billingCharacterMismatch".into());
        }
        let lease = if active.is_none() {
            Some(inactive_lease(cache.as_ref(), &account, character)?)
        } else {
            None
        };
        let operation = (|| -> Result<Value, String> {
            packets.extend(
                session
                    .execute_with_outcome(WorldCommand::BillingRefresh {
                        character_index: character,
                    })?
                    .packets,
            );
            match &request {
                Request::Buy(buy) => packets.extend(
                    session
                        .execute_with_outcome(WorldCommand::BillingBuyMonthlyCard {
                            character_index: character,
                            request_id: buy.order_request_id.clone(),
                        })?
                        .packets,
                ),
                Request::Activate(activate) => {
                    let uid = activate
                        .item_unique_id
                        .parse::<u64>()
                        .map_err(|_| "billingItemInvalid")?;
                    if uid == 0 || uid.to_string() != activate.item_unique_id {
                        return Err("billingItemInvalid".into());
                    }
                    packets.extend(
                        session
                            .execute_with_outcome(WorldCommand::BillingActivateMonthlyCard {
                                character_index: character,
                                unique_id: uid,
                            })?
                            .packets,
                    );
                }
                _ => {}
            }
            Ok(json!({"status":status_value(&config,&account,character,provider.as_ref(), include_payment_providers)?}))
        })();
        release_confirmed_lease(cache.as_ref(), lease, &operation)?;
        operation
    });
    for packet in packets {
        send_server_packet(&sender, &packet)
            .await
            .map_err(|_| "billingSendFailed")?;
    }
    sender
        .send(Message::Text(reply(&request, result).to_string().into()))
        .await
        .map_err(|_| "billingSendFailed".into())
}

pub(super) fn router() -> Router<WebState> {
    Router::new()
        .route("/v1/billing/stripe/webhook", post(webhook))
        .route("/v1/billing/waffo/webhook", post(waffo_webhook))
        .route("/v1/billing/status", get(http_status))
        .route("/v1/billing/refresh", post(http_refresh))
        .route("/v1/billing/monthly-card/buy", post(http_buy_monthly_card))
        .route(
            "/v1/billing/monthly-card/activate",
            post(http_activate_monthly_card),
        )
        .route("/v1/billing/checkout", post(http_checkout))
        .route("/v1/billing/return", get(return_page))
        .merge(sui_usdc::router())
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
}
fn private_response(status: StatusCode, value: Value) -> Response {
    (status, [(CACHE_CONTROL, "no-store")], Json(value)).into_response()
}
async fn webhook(
    State(state): State<WebState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let Some(signature) = headers
        .get("stripe-signature")
        .and_then(|h| h.to_str().ok())
    else {
        return private_response(
            StatusCode::BAD_REQUEST,
            json!({"error":"billingWebhookSignatureInvalid"}),
        );
    };
    let provider = match StripeBilling::from_env() {
        Ok(Some(p)) => p,
        _ => {
            return private_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({"error":"billingUnavailable"}),
            )
        }
    };
    if durable(&state.config).is_err() {
        return private_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":"billingUnavailable"}),
        );
    }
    match provider
        .webhook(&state.config, signature, &body, monthly_card_now_ms())
        .await
    {
        Ok(()) => private_response(StatusCode::OK, json!({"received":true})),
        Err(error) => {
            let rejected = error.starts_with("billingWebhook")
                || matches!(
                    error.as_str(),
                    "billingPaymentMismatch" | "billingProviderDataInvalid"
                );
            private_response(
                if rejected {
                    StatusCode::BAD_REQUEST
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                json!({"error":if rejected{"billingWebhookRejected"}else{"billingResultUnconfirmed"}}),
            )
        }
    }
}
async fn waffo_webhook(
    State(state): State<WebState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let signature = headers.get("x-signature").and_then(|h| h.to_str().ok());
    let Some(signature) = signature.filter(|_| headers.get_all("x-signature").iter().count() == 1) else {
        return private_response(StatusCode::BAD_REQUEST, json!({"message":"failed"}));
    };
    let provider = match crate::waffo_billing::WaffoBilling::from_env() {
        Ok(Some(p)) => p,
        _ => return private_response(StatusCode::SERVICE_UNAVAILABLE, json!({"message":"unknown"})),
    };
    let _permit = match checkout_permit() {
        Ok(p) => p,
        Err(_) => return private_response(StatusCode::TOO_MANY_REQUESTS, json!({"message":"unknown"})),
    };
    let (status, message) = match provider.webhook(&state.config, signature, &body, monthly_card_now_ms()).await {
        Ok(()) => (StatusCode::OK, "success"),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "failed"),
    };
    let raw = json!({"message":message}).to_string();
    match provider.sign_ack(raw.as_bytes()) {
        Ok(signature) => (status, [("cache-control", "no-store"), ("content-type", "application/json"), ("x-signature", signature.as_str())], raw).into_response(),
        Err(_) => private_response(StatusCode::SERVICE_UNAVAILABLE, json!({"message":"unknown"})),
    }
}
// These HTTP operations are for an inactive character only. The same shared
// Redis owner lease used by character-select billing fences an online Source.
// The underlying ordinary purchase/activation code owns all amounts and UIDs.
async fn http_offline_operation(state: WebState, headers: HeaderMap, request: Request) -> Response {
    let include_payment_providers = request.include_payment_providers();
    let identity = match http_identity(&state.identity, &headers).await {
        Ok(v) => v,
        Err(_) => {
            return private_response(
                StatusCode::UNAUTHORIZED,
                json!({"error":"billingAuthenticationRequired"}),
            )
        }
    };
    let result = tokio::task::spawn_blocking(move || -> Result<Value,String> {
        durable(&state.config)?;
        let account = identity.account_id;
        let character = request.character();
        rate_limit(state.session_cache.as_ref(), &account, request.operation())?;
        let lease = inactive_lease(state.session_cache.as_ref(), &account, character)?;
        let operation = (|| -> Result<Value,String> {
            let now = monthly_card_now_ms();
            let delivery = state.config.consume_inactive_pending_recharge_credits(&account, character, now)?;
            let receipt = match request {
                Request::Buy(q) => serde_json::to_value(state.config.buy_monthly_card_for_character(&account, character, &q.order_request_id, now)?).map_err(|_|"billingResultUnconfirmed")?,
                Request::Activate(q) => {
                    let uid = q.item_unique_id.parse::<u64>().map_err(|_|"billingItemInvalid")?;
                    if uid == 0 || uid.to_string() != q.item_unique_id { return Err("billingItemInvalid".into()); }
                    serde_json::to_value(state.config.activate_monthly_card_for_character(&account, character, uid, now)?).map_err(|_|"billingResultUnconfirmed")?
                },
                Request::Status(_) => Value::Null,
                _ => return Err("billingRequestInvalid".into()),
            };
            let provider = StripeBilling::from_env()?;
            Ok(json!({"receipt":receipt,"delivery":{"granted":delivery.granted,"revision":delivery.revision},"status":status_value(&state.config,&account,character,provider.as_ref(), include_payment_providers)?}))
        })();
        release_confirmed_lease(state.session_cache.as_ref(), Some(lease), &operation)?;
        operation
    }).await.unwrap_or_else(|_| Err("billingResultUnconfirmed".into()));
    match result {
        Ok(v) => private_response(StatusCode::OK, v),
        Err(e) => private_response(
            if e == "billingCharacterOnline" {
                StatusCode::CONFLICT
            } else if e == "billingTooManyRequests" {
                StatusCode::TOO_MANY_REQUESTS
            } else {
                StatusCode::BAD_REQUEST
            },
            json!({"error":public_error(&e)}),
        ),
    }
}
async fn http_refresh(
    State(s): State<WebState>,
    h: HeaderMap,
    Json(q): Json<StatusRequest>,
) -> Response {
    http_offline_operation(s, h, Request::Status(q)).await
}
async fn http_buy_monthly_card(
    State(s): State<WebState>,
    h: HeaderMap,
    Json(q): Json<BuyRequest>,
) -> Response {
    http_offline_operation(s, h, Request::Buy(q)).await
}
async fn http_activate_monthly_card(
    State(s): State<WebState>,
    h: HeaderMap,
    Json(q): Json<ActivateRequest>,
) -> Response {
    http_offline_operation(s, h, Request::Activate(q)).await
}

async fn http_status(
    State(state): State<WebState>,
    headers: HeaderMap,
    Query(request): Query<StatusRequest>,
) -> Response {
    let result = http_identity(&state.identity, &headers).await;
    let verified = match result {
        Ok(v) => v,
        Err(e) => {
            return private_response(StatusCode::UNAUTHORIZED, json!({"error":public_error(&e)}))
        }
    };
    let result = tokio::task::spawn_blocking(move || {
        durable(&state.config)?;
        rate_limit(state.session_cache.as_ref(), &verified.account_id, "status")?;
        let provider = StripeBilling::from_env()?;
        status_value(
            &state.config,
            &verified.account_id,
            request.character_index,
            provider.as_ref(),
            request.include_payment_providers,
        )
    })
    .await;
    match result {
        Ok(Ok(value)) => private_response(StatusCode::OK, value),
        Ok(Err(e)) => private_response(
            if e == "billingTooManyRequests" {
                StatusCode::TOO_MANY_REQUESTS
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            },
            json!({"error":public_error(&e)}),
        ),
        _ => private_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":"billingResultUnconfirmed"}),
        ),
    }
}
async fn http_checkout(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<CheckoutRequest>,
) -> Response {
    let include_payment_providers = request.include_payment_providers;
    let verified = match http_identity(&state.identity, &headers).await {
        Ok(v) => v,
        Err(_) => {
            return private_response(
                StatusCode::UNAUTHORIZED,
                json!({"error":"billingAuthenticationRequired"}),
            )
        }
    };
    let _permit = match checkout_permit() {
        Ok(p) => p,
        Err(e) => {
            return private_response(
                StatusCode::TOO_MANY_REQUESTS,
                json!({"error":public_error(&e)}),
            )
        }
    };
    let config = Arc::clone(&state.config);
    let cache = Arc::clone(&state.session_cache);
    let prepared = tokio::task::spawn_blocking(move || -> Result<_, String> {
        durable(&config)?;
        rate_limit(cache.as_ref(), &verified.account_id, "checkout")?;
        let provider = FiatBilling::from_env(request.provider)?.ok_or("billingUnavailable")?;
        provider.validate_store(&config)?;
        let offer = provider
            .offers()
            .iter()
            .find(|offer| offer.id == request.offer_id)
            .ok_or("billingOfferMissing")?;
        let order = config.prepare_recharge_order_for_provider(
            &verified.account_id,
            request.character_index,
            &request.order_request_id,
            offer,
            monthly_card_now_ms(),
            provider.livemode(),
            provider.id(),
        )?;
        Ok((config, provider, verified.account_id, order))
    })
    .await;
    let (config, provider, account, order) = match prepared {
        Ok(Ok(tuple)) => tuple,
        Ok(Err(e)) => {
            return private_response(
                if e == "billingTooManyRequests" {
                    StatusCode::TOO_MANY_REQUESTS
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                },
                json!({"error":public_error(&e)}),
            )
        }
        _ => {
            return private_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({"error":"billingResultUnconfirmed"}),
            )
        }
    };
    if matches!(
        order.state,
        RechargeOrderState::PaidPending | RechargeOrderState::Applied | RechargeOrderState::Review
    ) {
        let mut value = json!({"orderId":order.id,"checkoutUrl":Value::Null,"replayed":true});
        if include_payment_providers { value["provider"] = json!(provider.id()); }
        return private_response(
            StatusCode::OK,
            value,
        );
    }
    match provider.checkout(&config, &account, &order).await {
        Ok(url) => {
            let mut value = json!({"orderId":order.id,"checkoutUrl":url});
            if include_payment_providers { value["provider"] = json!(provider.id()); }
            private_response(StatusCode::OK, value)
        },
        Err(e) => private_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":public_error(&e)}),
        ),
    }
}
async fn return_page() -> Response {
    // Query parameters (including "paid") are cosmetic and never fulfill an order.
    (
        [
            ("cache-control", "no-store"),
            ("content-security-policy", BILLING_RETURN_CSP),
            ("referrer-policy", "no-referrer"),
            ("x-content-type-options", "nosniff"),
        ],
        Html(include_str!("billing_return.html")),
    )
        .into_response()
}

const BILLING_RETURN_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; script-src 'sha256-1xRCNPFcbB6K0cfn8LT1xKt5sTrLO7B1tKoDJM1SUBs='; connect-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
