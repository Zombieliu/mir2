//! Authenticated billing seams. Gameplay never waits for the Stripe network.
use super::*;
use crate::stripe_billing::StripeBilling;
use mir2_simulation::billing::RechargeOrderState;
use mir2_simulation::monthly_card::monthly_card_now_ms;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StatusRequest {
    pub character_index: i32,
    pub request_id: u64,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CheckoutRequest {
    pub character_index: i32,
    pub request_id: u64,
    pub order_request_id: String,
    pub offer_id: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BuyRequest {
    pub character_index: i32,
    pub request_id: u64,
    pub order_request_id: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ActivateRequest {
    pub character_index: i32,
    pub request_id: u64,
    pub item_unique_id: String,
}
#[derive(Debug, Clone)]
pub(super) enum Request {
    Status(StatusRequest),
    Checkout(CheckoutRequest),
    Buy(BuyRequest),
    Activate(ActivateRequest),
}
impl Request {
    fn character(&self) -> i32 {
        match self {
            Self::Status(r) => r.character_index,
            Self::Checkout(r) => r.character_index,
            Self::Buy(r) => r.character_index,
            Self::Activate(r) => r.character_index,
        }
    }
    fn request_id(&self) -> u64 {
        match self {
            Self::Status(r) => r.request_id,
            Self::Checkout(r) => r.request_id,
            Self::Buy(r) => r.request_id,
            Self::Activate(r) => r.request_id,
        }
    }
    fn operation(&self) -> &'static str {
        match self {
            Self::Status(_) => "status",
            Self::Checkout(_) => "checkout",
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
async fn http_identity(
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
fn rate_limit(
    cache: &dyn crate::cache::GatewaySessionCache,
    account: &str,
    operation: &str,
) -> Result<(), String> {
    let count = cache
        .record_auth_attempt(&format!("billing:{account}:{operation}"), 60)?
        .0;
    if count > if operation == "status" { 120 } else { 20 } {
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
        .map(|order| json!({"id":order.id,"state":order.state}))
        .collect::<Vec<_>>();
    Ok(
        json!({"enabled":provider.is_some()||config.billing_monthly_card_credit_price.is_some(),
        "checkoutEnabled":provider.is_some(),"credit":credit,"pendingCredit":pending,
        "offers":provider.map(StripeBilling::public_offers).unwrap_or_default(),
        "monthlyCardCreditPrice":config.billing_monthly_card_credit_price,
        "ownedMonthlyCards":cards,"orders":orders,
        "monthlyCard":config.refresh_monthly_card_status(account,monthly_card_now_ms())?}),
    )
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
    let provider = StripeBilling::from_env()?;
    rate_limit(cache.as_ref(), &account, request.operation())?;
    if let Request::Checkout(checkout) = &request {
        let permit = checkout_permit()?;
        let prepared: Result<_, String> = tokio::task::block_in_place(|| {
            durable(&config)?;
            let provider = provider.as_ref().ok_or("billingUnavailable")?;
            provider.validate_store(&config)?;
            let offer = provider
                .offers
                .iter()
                .find(|offer| offer.id == checkout.offer_id)
                .ok_or("billingOfferMissing")?;
            config.prepare_recharge_order(
                &account,
                checkout.character_index,
                &checkout.order_request_id,
                offer,
                monthly_card_now_ms(),
                provider.livemode,
            )
        });
        let order = match prepared {
            Ok(order) => order,
            Err(e) => {
                sender
                    .send(Message::Text(reply(&request, Err(e)).to_string().into()))
                    .await
                    .map_err(|_| "billingSendFailed")?;
                return Ok(());
            }
        };
        let provider = provider.expect("provider validated while preparing order");
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
                    Some(provider.checkout(&config, &account, &order).await?)
                };
                let status = tokio::task::spawn_blocking(move || {
                    status_value(&config, &account, order.character_index, Some(&provider))
                })
                .await
                .map_err(|_| "billingResultUnconfirmed")??;
                Ok(json!({"checkoutUrl":url,"status":status}))
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
            Ok(json!({"status":status_value(&config,&account,character,provider.as_ref())?}))
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
        .route("/v1/billing/status", get(http_status))
        .route("/v1/billing/checkout", post(http_checkout))
        .route("/v1/billing/return", get(return_page))
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
        let provider = StripeBilling::from_env()?.ok_or("billingUnavailable")?;
        provider.validate_store(&config)?;
        let offer = provider
            .offers
            .iter()
            .find(|offer| offer.id == request.offer_id)
            .ok_or("billingOfferMissing")?;
        let order = config.prepare_recharge_order(
            &verified.account_id,
            request.character_index,
            &request.order_request_id,
            offer,
            monthly_card_now_ms(),
            provider.livemode,
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
        return private_response(
            StatusCode::OK,
            json!({"orderId":order.id,"checkoutUrl":Value::Null,"replayed":true}),
        );
    }
    match provider.checkout(&config, &account, &order).await {
        Ok(url) => private_response(
            StatusCode::OK,
            json!({"orderId":order.id,"checkoutUrl":url}),
        ),
        Err(e) => private_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":public_error(&e)}),
        ),
    }
}
async fn return_page() -> Response {
    // Query parameters (including "paid") are cosmetic and never fulfill an order.
    ([("cache-control","no-store"),("content-security-policy", BILLING_RETURN_CSP),
        ("referrer-policy","no-referrer"), ("x-content-type-options", "nosniff")],Html(include_str!("billing_return.html")))
        .into_response()
}

const BILLING_RETURN_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; script-src 'sha256-1xRCNPFcbB6K0cfn8LT1xKt5sTrLO7B1tKoDJM1SUBs='; connect-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
