use std::env;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use mir2_gateway::{Gate14AuthoritativeState, Gate14FinalizedRecord, Gate14QuorumClient};
use postgres::{Client, NoTls};
use serde::Serialize;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
struct ProjectionStatus {
    finalized_height: u64,
    state_root: String,
    db_available: bool,
    redis_available: bool,
    rebuilt_count: u64,
    projected_at_ms: u64,
    last_error: Option<String>,
}

#[derive(Clone)]
struct ProjectorState {
    projector_id: String,
    quorum: Gate14QuorumClient,
    database_url: String,
    redis_url: Option<String>,
    status: Arc<RwLock<ProjectionStatus>>,
    force_rebuild: Arc<AtomicBool>,
    started_at_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectorStatusResponse {
    role: &'static str,
    projector_id: String,
    healthy: bool,
    finalized_height: u64,
    state_root: String,
    postgres_authoritative: bool,
    database_available: bool,
    redis_authoritative: bool,
    redis_available: bool,
    rebuilt_count: u64,
    projected_at_ms: u64,
    started_at_ms: u64,
    last_error: Option<String>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("gate14 projector failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let projector_id =
        env::var("GATE14_PROJECTOR_ID").unwrap_or_else(|_| "projector-a".to_string());
    let validator_urls = env::var("GATE14_VALIDATOR_URLS")
        .unwrap_or_else(|_| {
            "http://127.0.0.1:19400,http://127.0.0.1:19401,http://127.0.0.1:19402,http://127.0.0.1:19403"
                .to_string()
        })
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let database_url = env::var("GATE14_DATABASE_URL")
        .map_err(|_| "GATE14_DATABASE_URL is required".to_string())?;
    let redis_url = env::var("GATE14_REDIS_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let bind = SocketAddr::from_str(
        &env::var("GATE14_PROJECTOR_BIND").unwrap_or_else(|_| "0.0.0.0:9600".to_string()),
    )
    .map_err(|error| format!("invalid GATE14_PROJECTOR_BIND: {error}"))?;
    let state = ProjectorState {
        projector_id,
        quorum: Gate14QuorumClient::new(validator_urls)?,
        database_url,
        redis_url,
        status: Arc::new(RwLock::new(ProjectionStatus::default())),
        force_rebuild: Arc::new(AtomicBool::new(true)),
        started_at_ms: now_ms(),
    };
    let worker = state.clone();
    tokio::spawn(async move {
        loop {
            project_once(&worker).await;
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });
    let app = Router::new()
        .route("/healthz", get(health))
        .route("/metrics", get(metrics))
        .route("/v1/status", get(status))
        .route("/v1/rebuild", post(rebuild))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|error| format!("bind Gate 14 projector {bind} failed: {error}"))?;
    axum::serve(listener, app)
        .await
        .map_err(|error| format!("serve Gate 14 projector failed: {error}"))
}

async fn project_once(state: &ProjectorState) {
    let snapshot = match state.quorum.quorum_state().await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            state.status.write().await.last_error = Some(error);
            return;
        }
    };
    let current = state.status.read().await.clone();
    let force = state.force_rebuild.swap(false, Ordering::SeqCst);
    if !force
        && current.db_available
        && current.finalized_height == snapshot.state.finalized_height
        && current.state_root == snapshot.state_root
    {
        return;
    }
    let records = match state.quorum.finalized_since(0).await {
        Ok(records) => records,
        Err(error) => {
            state.status.write().await.last_error = Some(error);
            return;
        }
    };
    let database_url = state.database_url.clone();
    let authoritative = snapshot.state.clone();
    let root = snapshot.state_root.clone();
    let projector_id = state.projector_id.clone();
    let projection = tokio::task::spawn_blocking(move || {
        write_projection(
            &database_url,
            &projector_id,
            &authoritative,
            &records,
            &root,
        )
    })
    .await;
    match projection {
        Ok(Ok(())) => {
            let redis_available = match state.redis_url.clone() {
                Some(redis_url) => {
                    let key = format!("obelisk:gate14:projection:{}", state.projector_id);
                    let value = serde_json::to_vec(&snapshot.state).unwrap_or_default();
                    tokio::task::spawn_blocking(move || redis_set(&redis_url, &key, &value, 30))
                        .await
                        .is_ok_and(|result| result.is_ok())
                }
                None => false,
            };
            let mut status = state.status.write().await;
            status.finalized_height = snapshot.state.finalized_height;
            status.state_root = snapshot.state_root;
            status.db_available = true;
            status.redis_available = redis_available;
            status.rebuilt_count = status.rebuilt_count.saturating_add(1);
            status.projected_at_ms = now_ms();
            status.last_error = None;
        }
        Ok(Err(error)) => {
            let mut status = state.status.write().await;
            status.db_available = false;
            status.last_error = Some(error);
        }
        Err(error) => {
            let mut status = state.status.write().await;
            status.db_available = false;
            status.last_error = Some(format!("projection task failed: {error}"));
        }
    }
}

fn write_projection(
    database_url: &str,
    projector_id: &str,
    state: &Gate14AuthoritativeState,
    records: &[Gate14FinalizedRecord],
    state_root: &str,
) -> Result<(), String> {
    let mut client = Client::connect(database_url, NoTls)
        .map_err(|error| format!("connect Gate 14 Postgres failed: {error}"))?;
    client
        .batch_execute(
            "
            CREATE TABLE IF NOT EXISTS gate14_projection_meta (
                projector_id TEXT PRIMARY KEY,
                finalized_height BIGINT NOT NULL,
                state_root TEXT NOT NULL,
                authoritative_state JSONB NOT NULL,
                projected_at_ms BIGINT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS gate14_finalized_commands (
                height BIGINT PRIMARY KEY,
                sequence BIGINT NOT NULL UNIQUE,
                idempotency_key TEXT NOT NULL UNIQUE,
                command_digest TEXT NOT NULL,
                commonware_digest TEXT NOT NULL,
                signer_count INTEGER NOT NULL,
                command JSONB NOT NULL,
                state_root TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS gate14_accounts (
                account_id TEXT PRIMARY KEY
            );
            CREATE TABLE IF NOT EXISTS gate14_characters (
                character_id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                name TEXT NOT NULL,
                gold BIGINT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS gate14_inventory (
                character_id TEXT NOT NULL,
                item_id TEXT NOT NULL,
                quantity BIGINT NOT NULL,
                PRIMARY KEY (character_id, item_id)
            );
            CREATE TABLE IF NOT EXISTS gate14_placements (
                zone_id TEXT PRIMARY KEY,
                generation BIGINT NOT NULL,
                primary_host_id TEXT NOT NULL,
                replica_host_ids JSONB NOT NULL,
                expires_at_ms BIGINT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS gate14_session_leases (
                session_id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                character_id TEXT NOT NULL,
                gateway_id TEXT NOT NULL,
                zone_id TEXT NOT NULL,
                fencing_token BIGINT NOT NULL,
                expires_at_ms BIGINT NOT NULL
            );
            ",
        )
        .map_err(|error| format!("create Gate 14 projection schema failed: {error}"))?;
    let mut tx = client
        .transaction()
        .map_err(|error| format!("begin Gate 14 projection transaction failed: {error}"))?;
    tx.batch_execute(
        "
        DELETE FROM gate14_finalized_commands;
        DELETE FROM gate14_inventory;
        DELETE FROM gate14_characters;
        DELETE FROM gate14_accounts;
        DELETE FROM gate14_placements;
        DELETE FROM gate14_session_leases;
        ",
    )
    .map_err(|error| format!("clear Gate 14 projection failed: {error}"))?;
    for record in records {
        let command = serde_json::to_value(&record.command)
            .map_err(|error| format!("encode projected command failed: {error}"))?;
        tx.execute(
            "INSERT INTO gate14_finalized_commands
             (height, sequence, idempotency_key, command_digest, commonware_digest, signer_count, command, state_root)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &(record.height as i64),
                &(record.command.sequence as i64),
                &record.command.idempotency_key,
                &record.command_digest,
                &record.commonware_digest,
                &(record.signer_count as i32),
                &command,
                &record.state_root,
            ],
        )
        .map_err(|error| format!("insert projected command failed: {error}"))?;
    }
    for account in state.accounts.values() {
        tx.execute(
            "INSERT INTO gate14_accounts (account_id) VALUES ($1)",
            &[&account.account_id],
        )
        .map_err(|error| format!("insert projected account failed: {error}"))?;
        for character in account.characters.values() {
            tx.execute(
                "INSERT INTO gate14_characters (character_id, account_id, name, gold)
                 VALUES ($1,$2,$3,$4)",
                &[
                    &character.character_id,
                    &account.account_id,
                    &character.name,
                    &(character.gold as i64),
                ],
            )
            .map_err(|error| format!("insert projected character failed: {error}"))?;
            for (item_id, quantity) in &character.inventory {
                tx.execute(
                    "INSERT INTO gate14_inventory (character_id, item_id, quantity)
                     VALUES ($1,$2,$3)",
                    &[&character.character_id, item_id, &(*quantity as i64)],
                )
                .map_err(|error| format!("insert projected inventory failed: {error}"))?;
            }
        }
    }
    for placement in state.placements.values() {
        let replicas = serde_json::to_value(&placement.replica_host_ids)
            .map_err(|error| format!("encode placement replicas failed: {error}"))?;
        tx.execute(
            "INSERT INTO gate14_placements
             (zone_id, generation, primary_host_id, replica_host_ids, expires_at_ms)
             VALUES ($1,$2,$3,$4,$5)",
            &[
                &placement.zone_id,
                &(placement.generation as i64),
                &placement.primary_host_id,
                &replicas,
                &(placement.expires_at_ms as i64),
            ],
        )
        .map_err(|error| format!("insert projected placement failed: {error}"))?;
    }
    for lease in state.session_leases.values() {
        tx.execute(
            "INSERT INTO gate14_session_leases
             (session_id, account_id, character_id, gateway_id, zone_id, fencing_token, expires_at_ms)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
            &[
                &lease.session_id,
                &lease.account_id,
                &lease.character_id,
                &lease.gateway_id,
                &lease.zone_id,
                &(lease.fencing_token as i64),
                &(lease.expires_at_ms as i64),
            ],
        )
        .map_err(|error| format!("insert projected session lease failed: {error}"))?;
    }
    let authoritative_state = serde_json::to_value(state)
        .map_err(|error| format!("encode authoritative snapshot failed: {error}"))?;
    tx.execute(
        "INSERT INTO gate14_projection_meta
         (projector_id, finalized_height, state_root, authoritative_state, projected_at_ms)
         VALUES ($1,$2,$3,$4,$5)
         ON CONFLICT (projector_id) DO UPDATE SET
           finalized_height=EXCLUDED.finalized_height,
           state_root=EXCLUDED.state_root,
           authoritative_state=EXCLUDED.authoritative_state,
           projected_at_ms=EXCLUDED.projected_at_ms",
        &[
            &projector_id,
            &(state.finalized_height as i64),
            &state_root,
            &authoritative_state,
            &(now_ms() as i64),
        ],
    )
    .map_err(|error| format!("write Gate 14 projection metadata failed: {error}"))?;
    tx.commit()
        .map_err(|error| format!("commit Gate 14 projection failed: {error}"))
}

async fn health(State(state): State<ProjectorState>) -> impl IntoResponse {
    let status = state.status.read().await;
    if status.db_available && status.last_error.is_none() {
        (StatusCode::OK, "ok\n")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "projection unavailable\n")
    }
}

async fn status(State(state): State<ProjectorState>) -> Json<ProjectorStatusResponse> {
    let status = state.status.read().await;
    Json(ProjectorStatusResponse {
        role: "disposable-postgres-projector",
        projector_id: state.projector_id.clone(),
        healthy: status.db_available && status.last_error.is_none(),
        finalized_height: status.finalized_height,
        state_root: status.state_root.clone(),
        postgres_authoritative: false,
        database_available: status.db_available,
        redis_authoritative: false,
        redis_available: status.redis_available,
        rebuilt_count: status.rebuilt_count,
        projected_at_ms: status.projected_at_ms,
        started_at_ms: state.started_at_ms,
        last_error: status.last_error.clone(),
    })
}

async fn rebuild(State(state): State<ProjectorState>) -> Json<serde_json::Value> {
    state.force_rebuild.store(true, Ordering::SeqCst);
    Json(serde_json::json!({"accepted": true, "projectorId": state.projector_id}))
}

async fn metrics(State(state): State<ProjectorState>) -> String {
    let status = state.status.read().await;
    format!(
        "# HELP obelisk_gate14_projection_height Last Postgres projected height.\n\
         # TYPE obelisk_gate14_projection_height gauge\n\
         obelisk_gate14_projection_height{{projector=\"{}\"}} {}\n\
         # HELP obelisk_gate14_projection_database_available Postgres projection availability.\n\
         # TYPE obelisk_gate14_projection_database_available gauge\n\
         obelisk_gate14_projection_database_available{{projector=\"{}\"}} {}\n\
         # HELP obelisk_gate14_projection_redis_available Redis cache availability.\n\
         # TYPE obelisk_gate14_projection_redis_available gauge\n\
         obelisk_gate14_projection_redis_available{{projector=\"{}\"}} {}\n",
        metric_label(&state.projector_id),
        status.finalized_height,
        metric_label(&state.projector_id),
        u8::from(status.db_available),
        metric_label(&state.projector_id),
        u8::from(status.redis_available)
    )
}

fn redis_set(redis_url: &str, key: &str, value: &[u8], ttl_seconds: u64) -> Result<(), String> {
    let address = redis_address(redis_url)?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(1))
        .map_err(|error| format!("Redis connect failed: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .map_err(|error| format!("Redis read timeout failed: {error}"))?;
    let ttl = ttl_seconds.to_string();
    let command = resp_command(&[b"SET", key.as_bytes(), value, b"EX", ttl.as_bytes()]);
    stream
        .write_all(&command)
        .map_err(|error| format!("Redis SET write failed: {error}"))?;
    let mut response = [0_u8; 64];
    let read = stream
        .read(&mut response)
        .map_err(|error| format!("Redis SET read failed: {error}"))?;
    if response[..read].starts_with(b"+OK") {
        Ok(())
    } else {
        Err("Redis SET was not acknowledged".to_string())
    }
}

fn redis_address(redis_url: &str) -> Result<SocketAddr, String> {
    let without_scheme = redis_url
        .strip_prefix("redis://")
        .ok_or_else(|| "GATE14_REDIS_URL must start with redis://".to_string())?;
    let authority = without_scheme
        .split('/')
        .next()
        .unwrap_or(without_scheme)
        .rsplit('@')
        .next()
        .unwrap_or(without_scheme);
    let address = if authority.contains(':') {
        authority.to_string()
    } else {
        format!("{authority}:6379")
    };
    address
        .to_socket_addrs()
        .map_err(|error| format!("resolve Redis address failed: {error}"))?
        .next()
        .ok_or_else(|| "Redis address did not resolve".to_string())
}

fn resp_command(parts: &[&[u8]]) -> Vec<u8> {
    let mut output = format!("*{}\r\n", parts.len()).into_bytes();
    for part in parts {
        output.extend_from_slice(format!("${}\r\n", part.len()).as_bytes());
        output.extend_from_slice(part);
        output.extend_from_slice(b"\r\n");
    }
    output
}

fn metric_label(value: &str) -> String {
    value.replace(['\\', '"', '\n', '\r'], "_")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
