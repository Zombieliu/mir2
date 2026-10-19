//! Authenticated HTTP adapter for the NEW PostgreSQL hunting economy. This
//! executes in the game Gateway, never the admin/BI process. No reward or burn
//! registration endpoint exists here, and no old snapshot/gold is rewritten.
use super::{identity_bearer_token, validate_websocket_origin, WebState};
use crate::identity::VerifiedIdentitySession;
use axum::{
    extract::{Query, State},
    http::{header::CACHE_CONTROL, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use mir2_economy_service::{
    self as economy, AuthorizedWalletSnapshot, ConversionReceipt, EconomyError, EconomyResult,
    Idempotent, SourceContext, WalletOwner,
};
use postgres::{Client, Transaction};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

type HttpResult = Result<Response, Response>;
static HTTP_DATABASE_SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct HuntingWalletQuery {
    character_index: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct HuntingConversionRequest {
    character_index: i32,
    gross_gold: String,
    request_id: String,
}

fn response(status: StatusCode, value: Value) -> Response {
    let mut result = (status, Json(value)).into_response();
    result.headers_mut().insert(
        CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    result
}
fn error(status: StatusCode, code: &str, message: &str) -> Response {
    response(status, json!({"code":code,"error":message}))
}
fn unavailable() -> Response {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "economyUnavailable",
        "Economy service is unavailable; retry the same request.",
    )
}
fn economy_error(value: EconomyError) -> Response {
    match value {
        EconomyError::Unauthenticated => error(
            StatusCode::UNAUTHORIZED,
            "identityInactive",
            "An active identity session is required.",
        ),
        EconomyError::AccountUnavailable => error(
            StatusCode::FORBIDDEN,
            "accountUnavailable",
            "The account is unavailable.",
        ),
        EconomyError::CharacterNotFound => error(
            StatusCode::NOT_FOUND,
            "characterNotFound",
            "The character was not found for this account.",
        ),
        EconomyError::InvalidInput(_) => error(
            StatusCode::BAD_REQUEST,
            "invalidRequest",
            "Invalid economy request.",
        ),
        EconomyError::Overflow(_) => error(
            StatusCode::BAD_REQUEST,
            "amountOutOfRange",
            "The amount exceeds the supported range.",
        ),
        EconomyError::Disabled => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "economyDisabled",
            "The hunting economy is disabled.",
        ),
        EconomyError::RequestConflict => error(
            StatusCode::CONFLICT,
            "requestConflict",
            "This request ID was already used for another amount.",
        ),
        EconomyError::InsufficientBoundGold => error(
            StatusCode::CONFLICT,
            "insufficientBoundGold",
            "Insufficient bound gold.",
        ),
        EconomyError::InsufficientPermits => error(
            StatusCode::CONFLICT,
            "insufficientPermits",
            "当前全服变现额度不足，请等待。",
        ),
        EconomyError::SettlementRequired { .. } => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "settlementPending",
            "Daily settlement is catching up; retry the same request.",
        ),
        EconomyError::OutcomeUnknown => response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"code":"outcomeUnknown","error":"The original conversion may have committed. Retry with the same request ID and amount.","outcomeUnknown":true,"retrySameRequest":true}),
        ),
        _ => unavailable(),
    }
}
fn permit() -> Result<OwnedSemaphorePermit, Response> {
    HTTP_DATABASE_SLOTS
        .get_or_init(|| Arc::new(Semaphore::new(16)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            error(
                StatusCode::TOO_MANY_REQUESTS,
                "economyBusy",
                "Economy service is busy; retry the same request.",
            )
        })
}
fn runtime(
    state: &WebState,
    headers: &HeaderMap,
) -> Result<(crate::hunting_clock::HuntingRuntimeConfig, String), Response> {
    validate_websocket_origin(headers).map_err(|_| {
        error(
            StatusCode::FORBIDDEN,
            "originNotAllowed",
            "This request origin is not allowed.",
        )
    })?;
    let token = identity_bearer_token(headers).map_err(|_| {
        error(
            StatusCode::UNAUTHORIZED,
            "identityRequired",
            "An identity bearer token is required.",
        )
    })?;
    let runtime = crate::hunting_clock::hunting_http_runtime(&state.config)
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "economyDisabled",
                "The hunting economy is disabled.",
            )
        })?;
    Ok((runtime, token))
}
fn validate_index(index: i32) -> Result<(), Response> {
    if index < 0 {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalidCharacterIndex",
            "characterIndex must be nonnegative.",
        ));
    }
    Ok(())
}
fn gross_amount(value: &str) -> EconomyResult<u64> {
    if value.is_empty()
        || value.len() > 19
        || !value.bytes().all(|b| b.is_ascii_digit())
        || value.starts_with('0')
    {
        return Err(EconomyError::InvalidInput("gross_gold"));
    }
    let amount = value
        .parse::<u64>()
        .map_err(|_| EconomyError::InvalidInput("gross_gold"))?;
    if amount > i64::MAX as u64 {
        return Err(EconomyError::Overflow("gross_gold"));
    }
    Ok(amount)
}
fn validate_request_id(value: &str) -> EconomyResult<()> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > 256
        || value.chars().any(char::is_control)
    {
        return Err(EconomyError::InvalidInput("request_id"));
    }
    Ok(())
}
fn database_error(error: postgres::Error) -> EconomyError {
    EconomyError::Database {
        sqlstate: error.code().map(|code| code.code().into()),
    }
}

/// Lock order after the economy realm lock: account SHARE -> identity session
/// SHARE -> character SHARE. Current identity revocation writes only session
/// (or credential -> session), never session -> account/character. Game saves
/// acquire account UPDATE first, so they wait before touching character rows.
/// The same locks linearize ban/revoke/delete against conversion commit.
fn authorize_owner(
    tx: &mut Transaction<'_>,
    _clock: SourceContext,
    verified: &VerifiedIdentitySession,
    index: i32,
) -> EconomyResult<WalletOwner> {
    if index < 0 {
        return Err(EconomyError::InvalidInput("character_index"));
    }
    let account = tx
        .query_opt(
            "SELECT is_banned,ban_until_ms FROM accounts WHERE account_id=$1 FOR SHARE",
            &[&verified.account_id],
        )
        .map_err(database_error)?
        .ok_or(EconomyError::AccountUnavailable)?;
    let session=tx.query_opt("SELECT account_id,expires_at_ms,revoked_at_ms FROM identity_sessions WHERE session_id=$1 FOR SHARE",&[&verified.session_id])
        .map_err(database_error)?.ok_or(EconomyError::Unauthenticated)?;
    if session.get::<_, String>("account_id") != verified.account_id
        || session.get::<_, Option<i64>>("revoked_at_ms").is_some()
    {
        return Err(EconomyError::Unauthenticated);
    }
    let character=tx.query_opt("SELECT hunting_incarnation::text AS incarnation FROM characters WHERE account_id=$1 AND character_index=$2 FOR SHARE",&[&verified.account_id,&index])
        .map_err(database_error)?.ok_or(EconomyError::CharacterNotFound)?;
    // Source row locks may wait. Re-read DB time AFTER waiting; a token that
    // expires while blocked must not gain an authorization from a stale clock.
    let now: i64 = tx
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .map_err(database_error)?
        .get(0);
    let expiry = session.get::<_, i64>("expires_at_ms");
    if expiry <= now || u64::try_from(expiry).ok() != Some(verified.expires_at_ms) {
        return Err(EconomyError::Unauthenticated);
    }
    let banned = account.get::<_, bool>("is_banned");
    let until = account.get::<_, Option<i64>>("ban_until_ms");
    if banned && until.map_or(true, |end| end > now) {
        return Err(EconomyError::AccountUnavailable);
    }
    let incarnation: String = character.get("incarnation");
    Ok(WalletOwner {
        account_id: verified.account_id.clone(),
        character_index: index,
        incarnation: format!("pg-character-v1:{incarnation}"),
    })
}

fn pooled<T>(
    runtime: &crate::hunting_clock::HuntingRuntimeConfig,
    action: impl FnOnce(&mut Client) -> EconomyResult<T>,
) -> Result<T, Response> {
    mir2_simulation::with_account_store_postgres_client(&runtime.database_url, |client| {
        Ok(action(client))
    })
    .map_err(|_| unavailable())?
    .map_err(economy_error)
}
fn verified_identity(
    identity: &crate::identity::IdentityService,
    token: &str,
) -> Result<VerifiedIdentitySession, Response> {
    identity
        .verify_session_token(token)
        .map_err(|_| economy_error(EconomyError::Unauthenticated))
}
fn convert_authenticated(
    client: &mut Client,
    realm: &str,
    verified: &VerifiedIdentitySession,
    index: i32,
    request_id: &str,
    gross: u64,
) -> EconomyResult<Idempotent<ConversionReceipt>> {
    let attempt = |client: &mut Client| {
        economy::convert_authorized_with_timeouts(
            client,
            realm,
            request_id,
            gross,
            2000,
            10000,
            |tx, clock| authorize_owner(tx, clock, verified, index),
        )
    };
    match attempt(client) {
        Err(EconomyError::SettlementRequired { .. }) => {
            economy::settle_due_with_timeouts(client, realm, 366, 2000, 10000)?;
            attempt(client)
        }
        other => other,
    }
}
fn wallet_value(snapshot: AuthorizedWalletSnapshot) -> Value {
    let initialized = snapshot.wallet.is_some();
    let wallet = snapshot.wallet.unwrap_or_default();
    json!({"characterIndex":snapshot.owner.character_index,"initialized":initialized,"enabled":snapshot.enabled,
        "boundGold":wallet.bound_gold.to_string(),"unboundGold":wallet.unbound_gold.to_string(),"walletRevision":wallet.revision.to_string(),
        "permitStockAtoms":snapshot.realm.pool.stock_atoms.to_string(),"policyVersion":snapshot.realm.policy_version,
        "settledThroughDay":snapshot.realm.settled_through_day})
}
fn conversion_value(result: Idempotent<ConversionReceipt>) -> Value {
    let receipt = result.receipt;
    json!({"committed":true,"replayed":result.replayed,"requestId":receipt.request_id,"operationKey":receipt.operation_key,
        "characterIndex":receipt.owner.character_index,"policyVersion":receipt.policy_version,"businessDay":receipt.business_day,
        "committedAtMs":receipt.committed_at_ms,"grossGold":receipt.quote.bound_gold_debit.to_string(),"feeGold":receipt.quote.fee_unbound_gold.to_string(),
        "netGold":receipt.quote.net_unbound_gold_credit.to_string(),"consumedPermitAtoms":receipt.quote.license_atoms_debit.to_string(),
        "boundGold":receipt.wallet_after.bound_gold.to_string(),"unboundGold":receipt.wallet_after.unbound_gold.to_string(),
        "walletRevision":receipt.wallet_after.revision.to_string(),"permitStockAtoms":receipt.pool_after_atoms.to_string()})
}

pub(super) async fn hunting_wallet(
    State(state): State<WebState>,
    headers: HeaderMap,
    Query(query): Query<HuntingWalletQuery>,
) -> HttpResult {
    validate_index(query.character_index)?;
    let (runtime, token) = runtime(&state, &headers)?;
    let admission = permit()?;
    tokio::task::spawn_blocking(move || {
        let _admission = admission;
        let verified = verified_identity(&state.identity, &token)?;
        let snapshot = pooled(&runtime, |client| {
            economy::read_wallet_authorized_with_timeouts(
                client,
                &runtime.realm_id,
                2000,
                10000,
                |tx, clock| authorize_owner(tx, clock, &verified, query.character_index),
            )
        })?;
        Ok(response(StatusCode::OK, wallet_value(snapshot)))
    })
    .await
    .map_err(|_| unavailable())?
}
pub(super) async fn convert_bound_gold(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(request): Json<HuntingConversionRequest>,
) -> HttpResult {
    validate_index(request.character_index)?;
    let gross = gross_amount(&request.gross_gold).map_err(economy_error)?;
    validate_request_id(&request.request_id).map_err(economy_error)?;
    let (runtime, token) = runtime(&state, &headers)?;
    let admission = permit()?;
    tokio::task::spawn_blocking(move || {
        let _admission = admission;
        let verified = verified_identity(&state.identity, &token)?;
        let result = pooled(&runtime, |client| {
            convert_authenticated(
                client,
                &runtime.realm_id,
                &verified,
                request.character_index,
                &request.request_id,
                gross,
            )
        })?;
        Ok(response(StatusCode::OK, conversion_value(result)))
    })
    .await
    .map_err(|_| unavailable())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::IdentityService;
    use mir2_economy_policy::{Policy, Rational};
    use postgres::NoTls;
    use std::{
        sync::mpsc,
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn strict_http_conversion_parser_and_exact_decimal_range() {
        let parsed: HuntingConversionRequest = serde_json::from_str(
            r#"{"characterIndex":0,"grossGold":"1000000","requestId":"req-1"}"#,
        )
        .unwrap();
        assert_eq!(gross_amount(&parsed.gross_gold).unwrap(), 1_000_000);
        for body in [
            r#"{"characterIndex":0,"grossGold":1000000,"requestId":"r"}"#,
            r#"{"characterIndex":0,"grossGold":"1","requestId":"r","accountId":"victim"}"#,
            r#"{"characterIndex":0,"grossGold":"1","grossGold":"2","requestId":"r"}"#,
            r#"{"characterIndex":0,"grossGold":"1","requestId":"r","incarnation":"chosen"}"#,
            r#"{"characterIndex":0,"grossGold":"1","requestId":"r","k":1}"#,
        ] {
            assert!(serde_json::from_str::<HuntingConversionRequest>(body).is_err());
        }
        assert!(serde_json::from_str::<HuntingWalletQuery>(
            r#"{"characterIndex":0,"accountId":"victim"}"#
        )
        .is_err());
        for query in [
            "/v1/economy/hunting-wallet?characterIndex=0&accountId=victim",
            "/v1/economy/hunting-wallet?characterIndex=0&characterIndex=1",
        ] {
            assert!(Query::<HuntingWalletQuery>::try_from_uri(&query.parse().unwrap()).is_err());
        }
        for value in [
            "",
            "0",
            "01",
            "+1",
            "-1",
            "1.0",
            "1e6",
            " 1",
            "１",
            "9223372036854775808",
        ] {
            assert!(gross_amount(value).is_err(), "reject invalid decimal input");
        }
        assert_eq!(
            gross_amount("9223372036854775807").unwrap(),
            i64::MAX as u64
        );
        assert!(validate_request_id(" r ").is_err());
        assert!(validate_request_id("x\n").is_err());
        assert!(validate_request_id(&"x".repeat(257)).is_err());
        assert!(validate_index(-1).is_err());
    }

    #[test]
    fn identity_bearer_never_uses_cookies_and_revoked_token_cannot_authorize() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            axum::http::HeaderValue::from_static("token=not-an-authorization"),
        );
        assert!(identity_bearer_token(&headers).is_err());
        let identity = IdentityService::local_for_tests();
        let grant = identity
            .issue_session(
                "hunting-local",
                "password",
                "hunting-local",
                "127.0.0.1",
                "hunting-test",
            )
            .unwrap();
        let verified = verified_identity(&identity, &grant.token).unwrap();
        assert!(verified_identity(&identity, "fabricated-token").is_err());
        identity
            .revoke_session(&verified, &verified.session_id, "test")
            .unwrap();
        assert_eq!(
            verified_identity(&identity, &grant.token)
                .unwrap_err()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            economy_error(EconomyError::OutcomeUnknown).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn wallet_projection_preserves_integers_beyond_javascript_precision() {
        let value = wallet_value(AuthorizedWalletSnapshot {
            owner: WalletOwner {
                account_id: "test".into(),
                character_index: 2,
                incarnation: "private-incarnation".into(),
            },
            wallet: Some(economy::WalletBalance {
                bound_gold: 9_007_199_254_740_993,
                unbound_gold: 1,
                revision: 2,
            }),
            realm: economy::RealmState {
                policy_version: "v1".into(),
                pool: mir2_economy_policy::PoolState::default(),
                genesis_day: 1,
                settled_through_day: 0,
                revision: 0,
            },
            enabled: true,
        });
        assert_eq!(value["boundGold"], "9007199254740993");
        assert!(value.get("incarnation").is_none());
        assert!(value.get("accountId").is_none());
    }

    fn local_http_state() -> WebState {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        WebState {
            config: Arc::new(crate::GatewayConfig::default()),
            deploy_revision: None,
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            chat_hub: crate::tcp::chat_broadcast::ChatBroadcastHub::for_tests(),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            reconnect_sessions: Arc::new(super::super::ReconnectSessionStore::default()),
            capacity: Arc::new(super::super::GatewayCapacityState::unlimited()),
            gameplay_event_sink: None,
            identity: Arc::new(IdentityService::local_for_tests()),
            injector: crate::inject::LiveSessionInjector::default(),
            spectator: crate::spectator::SpectatorHub::from_env(),
            ai_live: crate::ai_live::AiLiveHub::new(
                crate::ai_live::AiLiveConfig::disabled_for_tests(
                    std::env::temp_dir()
                        .join(format!("mir2-hunting-http-{}-{unique}", std::process::id())),
                ),
            )
            .unwrap(),
            channel_identity: crate::ChannelIdentityRegistry::in_memory(),
        }
    }
    fn local_origin_headers() -> HeaderMap {
        let origin = std::env::var("MIR2_ALLOWED_WEB_ORIGINS")
            .ok()
            .and_then(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .find(|value| !value.is_empty())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "http://localhost".into());
        let mut headers = HeaderMap::new();
        headers.insert(axum::http::header::ORIGIN, origin.parse().unwrap());
        headers
    }
    #[tokio::test]
    async fn hunting_http_handlers_require_bearer_and_default_config_never_connects() {
        let state = local_http_state();
        let headers = local_origin_headers();
        let missing = hunting_wallet(
            State(state.clone()),
            headers.clone(),
            Query(HuntingWalletQuery { character_index: 0 }),
        )
        .await
        .unwrap_err();
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
        let mut authenticated = headers;
        authenticated.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer unused-in-disabled-mode"),
        );
        let disabled = hunting_wallet(
            State(state.clone()),
            authenticated.clone(),
            Query(HuntingWalletQuery { character_index: 0 }),
        )
        .await
        .unwrap_err();
        assert_eq!(disabled.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(disabled.headers().get(CACHE_CONTROL).unwrap(), "no-store");
        let disabled = convert_bound_gold(
            State(state),
            authenticated,
            Json(HuntingConversionRequest {
                character_index: 0,
                gross_gold: "100".into(),
                request_id: "default-disabled".into(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(disabled.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    fn pg() -> Client {
        let url = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
            .expect("ignored test requires an explicitly isolated PostgreSQL URL");
        let config: postgres::Config = url
            .parse()
            .expect("valid isolated test PostgreSQL configuration");
        assert!(config
            .get_hostaddrs()
            .iter()
            .all(std::net::IpAddr::is_loopback));
        assert!(config
            .get_dbname()
            .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")));
        assert!(!config.get_hosts().is_empty() && config.get_hosts().iter().all(|host| matches!(host,
            postgres::config::Host::Tcp(name) if matches!(name.as_str(), "localhost" | "127.0.0.1" | "::1"))),
            "acceptance fixtures require a loopback host");
        config
            .connect(NoTls)
            .expect("isolated PostgreSQL connection")
    }
    fn test_policy() -> Policy {
        Policy {
            schema_version: 1,
            policy_version: "gateway-auth-v1".into(),
            enabled: true,
            k: Rational {
                numerator: 1,
                denominator: 10,
            },
            conversion_fee: Rational {
                numerator: 2,
                denominator: 100,
            },
            daily_decay: Rational {
                numerator: 1,
                denominator: 150,
            },
            baseline_daily_mint_atoms: 72_000_000,
            daily_mint_cap_multiplier: Rational {
                numerator: 3,
                denominator: 2,
            },
            business_timezone: "Asia/Shanghai".into(),
            settlement_hour: 4,
        }
    }
    struct PgFixture {
        client: Client,
        realm: String,
        verified: VerifiedIdentitySession,
        owner: WalletOwner,
    }
    fn pg_fixture(label: &str) -> PgFixture {
        let mut client = pg();
        mir2_simulation::apply_migrations(&mut client).unwrap();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let realm = format!("http-auth-{label}-{}-{nonce}", std::process::id());
        let account = realm.clone();
        client
            .execute(
                "INSERT INTO accounts(account_id,raw_json) VALUES($1,$2)",
                &[&account, &json!({})],
            )
            .unwrap();
        client.execute("INSERT INTO characters(account_id,character_index,character_name,class,gender,level,raw_json) VALUES($1,0,'Isolated','warrior','male',1,$2)",&[&account,&json!({})]).unwrap();
        let identity = IdentityService::local_for_tests();
        let grant = identity
            .issue_session(
                &account,
                "password",
                &account,
                "127.0.0.1",
                "hunting-pg-test",
            )
            .unwrap();
        let verified = identity.verify_session_token(&grant.token).unwrap();
        client.execute("INSERT INTO identity_sessions(session_id,account_id,auth_method,issued_at_ms,last_seen_at_ms,expires_at_ms) VALUES($1,$2,'password',$3,$3,$4)",
            &[&verified.session_id,&account,&(grant.session.issued_at_ms as i64),&(verified.expires_at_ms as i64)]).unwrap();
        economy::initialize_realm(&mut client, &realm, &test_policy()).unwrap();
        let snapshot = economy::read_wallet_authorized(&mut client, &realm, |tx, clock| {
            authorize_owner(tx, clock, &verified, 0)
        })
        .unwrap();
        assert!(
            snapshot.wallet.is_none(),
            "read must not initialize a wallet"
        );
        let owner = snapshot.owner;
        economy::credit_bound_with_source(
            &mut client,
            &realm,
            &owner,
            "fixture-production",
            "fixture",
            10_000,
            |_, _| Ok(()),
        )
        .unwrap();
        // Isolated acceptance fixture only: establish a closed past day. The
        // production service offers neither backdating nor permit injection.
        let day = economy::read_realm(&mut client, &realm)
            .unwrap()
            .genesis_day
            - 1;
        client.execute("UPDATE hunting_economy_pools SET genesis_day=$2,settled_through_day=$3 WHERE realm_id=$1",&[&realm,&day,&(day-1)]).unwrap();
        let now: i64 = client
            .query_one(
                "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
                &[],
            )
            .unwrap()
            .get(0);
        client.execute("INSERT INTO hunting_economy_operations(realm_id,operation_key,operation_kind,request_digest,receipt_json,receipt_digest,business_day,committed_at_ms) VALUES($1,'fixture','burn','fixture',$2,'fixture',$3,$4)",&[&realm,&json!({}),&day,&now]).unwrap();
        client.execute("INSERT INTO hunting_economy_burn_events(realm_id,event_id,operation_key,layer,source_kind,source_key,gold_equivalent,valuation_version,policy_version,business_day,committed_at_ms) VALUES($1,'fixture','fixture','daily','fixture','fixture',100000,'fixture','gateway-auth-v1',$2,$3)",&[&realm,&day,&now]).unwrap();
        economy::settle_due(&mut client, &realm, 31).unwrap();
        PgFixture {
            client,
            realm,
            verified,
            owner,
        }
    }
    fn guarded_convert(
        f: &mut PgFixture,
        id: &str,
        amount: u64,
    ) -> EconomyResult<Idempotent<ConversionReceipt>> {
        convert_authenticated(&mut f.client, &f.realm, &f.verified, 0, id, amount)
    }

    #[test]
    #[ignore = "requires explicitly isolated PostgreSQL and migration 0018; run with --ignored"]
    fn hunting_http_postgres_guard_checks_owner_ban_revocation_and_uuid_incarnation() {
        let mut f = pg_fixture("owner");
        let first = guarded_convert(&mut f, "first", 100).unwrap();
        assert!(!first.replayed);
        assert_eq!(first.receipt.wallet_after.unbound_gold, 98);
        assert!(guarded_convert(&mut f, "first", 100).unwrap().replayed);
        assert!(matches!(
            economy::read_wallet_authorized(&mut f.client, &f.realm, |tx, clock| authorize_owner(
                tx,
                clock,
                &f.verified,
                99
            )),
            Err(EconomyError::CharacterNotFound)
        ));
        let foreign_account = format!("{}-foreign", f.realm);
        f.client
            .execute(
                "INSERT INTO accounts(account_id,raw_json) VALUES($1,$2)",
                &[&foreign_account, &json!({})],
            )
            .unwrap();
        f.client.execute("INSERT INTO characters(account_id,character_index,character_name,class,gender,level,raw_json) VALUES($1,99,'Other','warrior','male',1,$2)",&[&foreign_account,&json!({})]).unwrap();
        assert!(matches!(
            economy::read_wallet_authorized(&mut f.client, &f.realm, |tx, clock| authorize_owner(
                tx,
                clock,
                &f.verified,
                99
            )),
            Err(EconomyError::CharacterNotFound)
        ));
        f.client
            .execute(
                "UPDATE accounts SET is_banned=true WHERE account_id=$1",
                &[&f.verified.account_id],
            )
            .unwrap();
        assert!(matches!(
            guarded_convert(&mut f, "banned", 100),
            Err(EconomyError::AccountUnavailable)
        ));
        f.client
            .execute(
                "UPDATE accounts SET ban_until_ms=0 WHERE account_id=$1",
                &[&f.verified.account_id],
            )
            .unwrap();
        assert!(
            economy::read_wallet_authorized(&mut f.client, &f.realm, |tx, clock| authorize_owner(
                tx,
                clock,
                &f.verified,
                0
            ))
            .is_ok(),
            "expired temporary ban follows normal login semantics"
        );
        f.client
            .execute(
                "UPDATE accounts SET is_banned=false WHERE account_id=$1",
                &[&f.verified.account_id],
            )
            .unwrap();
        f.client
            .execute(
                "UPDATE identity_sessions SET revoked_at_ms=1 WHERE session_id=$1",
                &[&f.verified.session_id],
            )
            .unwrap();
        assert!(
            matches!(
                guarded_convert(&mut f, "first", 100),
                Err(EconomyError::Unauthenticated)
            ),
            "even original receipts need current authorization"
        );
        f.client
            .execute(
                "UPDATE identity_sessions SET revoked_at_ms=NULL WHERE session_id=$1",
                &[&f.verified.session_id],
            )
            .unwrap();
        f.client
            .execute(
                "UPDATE identity_sessions SET expires_at_ms=issued_at_ms+1 WHERE session_id=$1",
                &[&f.verified.session_id],
            )
            .unwrap();
        assert!(matches!(
            guarded_convert(&mut f, "expired", 100),
            Err(EconomyError::Unauthenticated)
        ));
        f.client
            .execute(
                "UPDATE identity_sessions SET expires_at_ms=$2 WHERE session_id=$1",
                &[&f.verified.session_id, &(f.verified.expires_at_ms as i64)],
            )
            .unwrap();
        // UUID changes even if deletion/recreation occur in one transaction.
        let mut tx = f.client.transaction().unwrap();
        tx.execute(
            "DELETE FROM characters WHERE account_id=$1 AND character_index=0",
            &[&f.verified.account_id],
        )
        .unwrap();
        tx.execute("INSERT INTO characters(account_id,character_index,character_name,class,gender,level,raw_json) VALUES($1,0,'Isolated','warrior','male',1,$2)",&[&f.verified.account_id,&json!({})]).unwrap();
        tx.commit().unwrap();
        let reborn = economy::read_wallet_authorized(&mut f.client, &f.realm, |tx, clock| {
            authorize_owner(tx, clock, &f.verified, 0)
        })
        .unwrap();
        assert_ne!(reborn.owner.incarnation, f.owner.incarnation);
        assert!(reborn.wallet.is_none());
        assert!(matches!(
            guarded_convert(&mut f, "first", 100),
            Err(EconomyError::InsufficientBoundGold)
        ));
        assert_eq!(
            economy::read_wallet(&mut f.client, &f.realm, &f.owner)
                .unwrap()
                .unwrap()
                .unbound_gold,
            98,
            "orphaned old assets are preserved, never inherited or silently destroyed"
        );
    }

    #[test]
    #[ignore = "requires explicitly isolated PostgreSQL and migration 0018; run with --ignored"]
    fn hunting_http_postgres_revoke_waits_for_the_authorized_conversion_commit() {
        let mut f = pg_fixture("revoke-race");
        let realm = f.realm.clone();
        let verified = f.verified.clone();
        let (locked_tx, locked_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let conversion = thread::spawn(move || {
            let mut client = pg();
            economy::convert_authorized(&mut client, &realm, "race", 100, |tx, clock| {
                let owner = authorize_owner(tx, clock, &verified, 0)?;
                let pid: i32 = tx
                    .query_one("SELECT pg_backend_pid()", &[])
                    .map_err(database_error)?
                    .get(0);
                locked_tx.send(pid).unwrap();
                release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                Ok(owner)
            })
        });
        let owner_pid = locked_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let session = f.verified.session_id.clone();
        let (pid_tx, pid_rx) = mpsc::channel();
        let revocation = thread::spawn(move || {
            let mut client = pg();
            let pid: i32 = client
                .query_one("SELECT pg_backend_pid()", &[])
                .unwrap()
                .get(0);
            pid_tx.send(pid).unwrap();
            client.execute("UPDATE identity_sessions SET revoked_at_ms=floor(extract(epoch FROM clock_timestamp())*1000)::bigint WHERE session_id=$1",&[&session]).unwrap()
        });
        let revoker_pid = pid_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut blocked = false;
        while Instant::now() < deadline {
            blocked = f
                .client
                .query_one(
                    "SELECT $1::int=ANY(pg_blocking_pids($2::int))",
                    &[&owner_pid, &revoker_pid],
                )
                .unwrap()
                .get(0);
            if blocked {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        release_tx.send(()).unwrap();
        let committed = conversion.join().unwrap().unwrap();
        assert!(
            blocked,
            "revocation must block on the actual conversion's authorization row lock"
        );
        assert_eq!(revocation.join().unwrap(), 1);
        assert_eq!(committed.receipt.wallet_after.unbound_gold, 98);
        assert!(matches!(
            guarded_convert(&mut f, "race", 100),
            Err(EconomyError::Unauthenticated)
        ));
        assert_eq!(
            economy::read_wallet(&mut f.client, &f.realm, &f.owner)
                .unwrap()
                .unwrap()
                .unbound_gold,
            98
        );
    }
}
