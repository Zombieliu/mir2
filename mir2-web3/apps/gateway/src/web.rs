use std::collections::HashMap;
use std::env;
use std::io;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use mir2_game_data::crystal_item_by_index;
use mir2_protocol::{
    crystal_stat_label, packet_payload_hex, server_packet_display_name,
    server_packet_raw_display_name, ClientAuction, ClientFriend, ClientIntelligentCreature,
    ClientPacket, ClientQuestInfo, MirDirection, MirGridType, Point, QuestItemReward, ServerPacket,
    UserItem, UserItemStat,
};
use mir2_simulation::{
    deliver_stage5_system_mail, Stage5MailDelivery, Stage5MailTargetKind, WorldCommand,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::auth::verify_passkey_gateway_token;
use crate::browser_commands::{
    default_drop_count, default_market_max_shape, parse_class, parse_direction, parse_gender,
    parse_grid, parse_move_mode, parse_spell, parse_spell_name,
};
use crate::cache::{
    gateway_session_cache_from_env, gateway_session_cache_status,
    refresh_session_cache_with_route_lease, remove_owned_session_cache, session_cache_key,
    session_cache_record, GatewaySessionCacheKey, GatewaySessionCacheRecord,
    GatewaySessionCacheStatus, SharedGatewaySessionCache,
};
use crate::events::{
    default_gameplay_event_sink_from_env, gameplay_event_sink_status, GameplayEventSinkStatus,
    SharedGameplayEventSink,
};
use crate::routing::shared_tick_probe_metrics;
use crate::session::catch_gateway_panic;
use crate::{GatewayConfig, GatewaySession, ZoneRegistry};

// ---------------------------------------------------------------------------
// mir2-probe — Gateway-side counters/timers (L5).
//
// Pure module-level `static` atomic counters so the shape of `WebState` is
// unchanged and no locking overhead is added to the hot tick loop. The prober
// reads them via `GET /metrics` (see `metrics_handler`). Cost per tick: 1
// `Instant::now()` call + 3 atomic adds. Memory: 8 `AtomicU64` (64 bytes).
// Reset to 0 at process startup; no per-session reset.
// ---------------------------------------------------------------------------

static PROBE_TICK_COUNT: AtomicU64 = AtomicU64::new(0);
static PROBE_TICK_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static PROBE_TICK_MS_LAST: AtomicU64 = AtomicU64::new(0);
static PROBE_TICK_MS_MAX: AtomicU64 = AtomicU64::new(0);
static PROBE_SAVE_COUNT: AtomicU64 = AtomicU64::new(0);
static PROBE_SAVE_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static PROBE_SAVE_MS_LAST: AtomicU64 = AtomicU64::new(0);
static PROBE_SAVE_MS_MAX: AtomicU64 = AtomicU64::new(0);
static PROBE_WORLD_SNAPSHOT_COUNT: AtomicU64 = AtomicU64::new(0);
static PROBE_STARTED_AT_MS: AtomicU64 = AtomicU64::new(0);

fn probe_mark_started() {
    if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        let _ = PROBE_STARTED_AT_MS.compare_exchange(
            0,
            now.as_millis() as u64,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }
}

/// Record one tick's wall-time cost (ms). `start` should be the `Instant`
/// captured just before the tick closure ran.
fn probe_record_tick(start: Instant) {
    let ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    PROBE_TICK_COUNT.fetch_add(1, Ordering::Relaxed);
    PROBE_TICK_MS_TOTAL.fetch_add(ms, Ordering::Relaxed);
    PROBE_TICK_MS_LAST.store(ms, Ordering::Relaxed);
    // Cheap max update: compare-swap loop. Avoids read-modify-write rebuild.
    let mut current_max = PROBE_TICK_MS_MAX.load(Ordering::Relaxed);
    while ms > current_max {
        match PROBE_TICK_MS_MAX.compare_exchange(
            current_max,
            ms,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(actual) => current_max = actual,
        }
    }
}

/// Record one save closure's wall-time cost (ms).
fn probe_record_save(start: Instant) {
    let ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    PROBE_SAVE_COUNT.fetch_add(1, Ordering::Relaxed);
    PROBE_SAVE_MS_TOTAL.fetch_add(ms, Ordering::Relaxed);
    PROBE_SAVE_MS_LAST.store(ms, Ordering::Relaxed);
    let mut current_max = PROBE_SAVE_MS_MAX.load(Ordering::Relaxed);
    while ms > current_max {
        match PROBE_SAVE_MS_MAX.compare_exchange(
            current_max,
            ms,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(actual) => current_max = actual,
        }
    }
}

fn probe_record_world_snapshot() {
    PROBE_WORLD_SNAPSHOT_COUNT.fetch_add(1, Ordering::Relaxed);
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GatewayProbeMetrics {
    started_at_ms: u64,
    sampled_at_ms: u64,
    tick: ProbeMetricSummary,
    shared_tick: crate::routing::SharedTickProbeMetrics,
    save: ProbeMetricSummary,
    world_snapshot_count: u64,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeMetricSummary {
    count: u64,
    ms_total: u64,
    ms_last: u64,
    ms_max: u64,
    ms_avg: u64,
}

fn probe_metric_summary(count: u64, total: u64, last: u64, max: u64) -> ProbeMetricSummary {
    let avg = if count > 0 { total / count } else { 0 };
    ProbeMetricSummary {
        count,
        ms_total: total,
        ms_last: last,
        ms_max: max,
        ms_avg: avg,
    }
}

#[derive(Clone)]
struct WebState {
    config: Arc<GatewayConfig>,
    zone_registry: Arc<ZoneRegistry>,
    session_cache: SharedGatewaySessionCache,
    reconnect_sessions: Arc<ReconnectSessionStore>,
    capacity: Arc<GatewayCapacityState>,
    gameplay_event_sink: Option<SharedGameplayEventSink>,
    /// On-chain command injection registry (M4, WF-5) — routes Relayer commands to live sessions.
    injector: crate::inject::LiveSessionInjector,
}

#[derive(Debug)]
struct GatewayCapacityState {
    max_ws_connections: Option<usize>,
    max_active_sessions: Option<usize>,
    max_reconnect_leases: Option<usize>,
    max_login_in_flight: Option<usize>,
    max_new_character_in_flight: Option<usize>,
    max_start_game_in_flight: Option<usize>,
    current_ws_connections: AtomicUsize,
    current_active_sessions: AtomicUsize,
    current_reconnect_leases: AtomicUsize,
    current_login_in_flight: AtomicUsize,
    current_new_character_in_flight: AtomicUsize,
    current_start_game_in_flight: AtomicUsize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct GatewayCapacityStatus {
    max_ws_connections: Option<usize>,
    max_active_sessions: Option<usize>,
    max_reconnect_leases: Option<usize>,
    max_login_in_flight: Option<usize>,
    max_new_character_in_flight: Option<usize>,
    max_start_game_in_flight: Option<usize>,
    current_ws_connections: usize,
    current_active_sessions: usize,
    current_reconnect_leases: usize,
    current_login_in_flight: usize,
    current_new_character_in_flight: usize,
    current_start_game_in_flight: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatewayCapacityKind {
    WebSocketConnection,
    ActiveSession,
    ReconnectLease,
    Login,
    NewCharacter,
    StartGame,
}

#[derive(Debug)]
struct GatewayCapacityPermit {
    state: Arc<GatewayCapacityState>,
    kind: GatewayCapacityKind,
}

impl GatewayCapacityState {
    fn from_env() -> Self {
        Self {
            max_ws_connections: positive_usize_env("MIR2_GATEWAY_MAX_WS_CONNECTIONS"),
            max_active_sessions: positive_usize_env("MIR2_GATEWAY_MAX_ACTIVE_SESSIONS"),
            max_reconnect_leases: positive_usize_env("MIR2_GATEWAY_MAX_RECONNECT_LEASES"),
            max_login_in_flight: positive_usize_env("MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT"),
            max_new_character_in_flight: positive_usize_env(
                "MIR2_GATEWAY_MAX_NEW_CHARACTER_IN_FLIGHT",
            ),
            max_start_game_in_flight: positive_usize_env("MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT"),
            current_ws_connections: AtomicUsize::new(0),
            current_active_sessions: AtomicUsize::new(0),
            current_reconnect_leases: AtomicUsize::new(0),
            current_login_in_flight: AtomicUsize::new(0),
            current_new_character_in_flight: AtomicUsize::new(0),
            current_start_game_in_flight: AtomicUsize::new(0),
        }
    }

    #[cfg(test)]
    fn unlimited() -> Self {
        Self {
            max_ws_connections: None,
            max_active_sessions: None,
            max_reconnect_leases: None,
            max_login_in_flight: None,
            max_new_character_in_flight: None,
            max_start_game_in_flight: None,
            current_ws_connections: AtomicUsize::new(0),
            current_active_sessions: AtomicUsize::new(0),
            current_reconnect_leases: AtomicUsize::new(0),
            current_login_in_flight: AtomicUsize::new(0),
            current_new_character_in_flight: AtomicUsize::new(0),
            current_start_game_in_flight: AtomicUsize::new(0),
        }
    }

    #[cfg(test)]
    fn with_limits(
        max_ws_connections: Option<usize>,
        max_active_sessions: Option<usize>,
        max_reconnect_leases: Option<usize>,
    ) -> Self {
        Self {
            max_ws_connections,
            max_active_sessions,
            max_reconnect_leases,
            max_login_in_flight: None,
            max_new_character_in_flight: None,
            max_start_game_in_flight: None,
            current_ws_connections: AtomicUsize::new(0),
            current_active_sessions: AtomicUsize::new(0),
            current_reconnect_leases: AtomicUsize::new(0),
            current_login_in_flight: AtomicUsize::new(0),
            current_new_character_in_flight: AtomicUsize::new(0),
            current_start_game_in_flight: AtomicUsize::new(0),
        }
    }

    #[cfg(test)]
    fn with_action_limits(
        max_login_in_flight: Option<usize>,
        max_new_character_in_flight: Option<usize>,
        max_start_game_in_flight: Option<usize>,
    ) -> Self {
        Self {
            max_ws_connections: None,
            max_active_sessions: None,
            max_reconnect_leases: None,
            max_login_in_flight,
            max_new_character_in_flight,
            max_start_game_in_flight,
            current_ws_connections: AtomicUsize::new(0),
            current_active_sessions: AtomicUsize::new(0),
            current_reconnect_leases: AtomicUsize::new(0),
            current_login_in_flight: AtomicUsize::new(0),
            current_new_character_in_flight: AtomicUsize::new(0),
            current_start_game_in_flight: AtomicUsize::new(0),
        }
    }

    fn status(&self) -> GatewayCapacityStatus {
        GatewayCapacityStatus {
            max_ws_connections: self.max_ws_connections,
            max_active_sessions: self.max_active_sessions,
            max_reconnect_leases: self.max_reconnect_leases,
            max_login_in_flight: self.max_login_in_flight,
            max_new_character_in_flight: self.max_new_character_in_flight,
            max_start_game_in_flight: self.max_start_game_in_flight,
            current_ws_connections: self.current_ws_connections.load(Ordering::Acquire),
            current_active_sessions: self.current_active_sessions.load(Ordering::Acquire),
            current_reconnect_leases: self.current_reconnect_leases.load(Ordering::Acquire),
            current_login_in_flight: self.current_login_in_flight.load(Ordering::Acquire),
            current_new_character_in_flight: self
                .current_new_character_in_flight
                .load(Ordering::Acquire),
            current_start_game_in_flight: self.current_start_game_in_flight.load(Ordering::Acquire),
        }
    }

    fn try_acquire_ws_connection(self: &Arc<Self>) -> Result<GatewayCapacityPermit, String> {
        self.try_acquire(GatewayCapacityKind::WebSocketConnection)
    }

    fn try_acquire_active_session(self: &Arc<Self>) -> Result<GatewayCapacityPermit, String> {
        self.try_acquire(GatewayCapacityKind::ActiveSession)
    }

    fn try_acquire_reconnect_lease(self: &Arc<Self>) -> Result<GatewayCapacityPermit, String> {
        self.try_acquire(GatewayCapacityKind::ReconnectLease)
    }

    fn try_acquire_action(
        self: &Arc<Self>,
        kind: GatewayCapacityKind,
    ) -> Result<GatewayCapacityPermit, String> {
        debug_assert!(matches!(
            kind,
            GatewayCapacityKind::Login
                | GatewayCapacityKind::NewCharacter
                | GatewayCapacityKind::StartGame
        ));
        self.try_acquire(kind)
    }

    fn try_acquire(
        self: &Arc<Self>,
        kind: GatewayCapacityKind,
    ) -> Result<GatewayCapacityPermit, String> {
        let counter = self.counter(kind);
        let previous = counter.fetch_add(1, Ordering::AcqRel);
        if let Some(limit) = self.limit(kind) {
            if previous >= limit {
                counter.fetch_sub(1, Ordering::AcqRel);
                return Err(format!(
                    "gateway {} capacity reached (limit {limit})",
                    kind.label()
                ));
            }
        }
        Ok(GatewayCapacityPermit {
            state: Arc::clone(self),
            kind,
        })
    }

    fn counter(&self, kind: GatewayCapacityKind) -> &AtomicUsize {
        match kind {
            GatewayCapacityKind::WebSocketConnection => &self.current_ws_connections,
            GatewayCapacityKind::ActiveSession => &self.current_active_sessions,
            GatewayCapacityKind::ReconnectLease => &self.current_reconnect_leases,
            GatewayCapacityKind::Login => &self.current_login_in_flight,
            GatewayCapacityKind::NewCharacter => &self.current_new_character_in_flight,
            GatewayCapacityKind::StartGame => &self.current_start_game_in_flight,
        }
    }

    fn limit(&self, kind: GatewayCapacityKind) -> Option<usize> {
        match kind {
            GatewayCapacityKind::WebSocketConnection => self.max_ws_connections,
            GatewayCapacityKind::ActiveSession => self.max_active_sessions,
            GatewayCapacityKind::ReconnectLease => self.max_reconnect_leases,
            GatewayCapacityKind::Login => self.max_login_in_flight,
            GatewayCapacityKind::NewCharacter => self.max_new_character_in_flight,
            GatewayCapacityKind::StartGame => self.max_start_game_in_flight,
        }
    }
}

impl GatewayCapacityKind {
    fn label(self) -> &'static str {
        match self {
            GatewayCapacityKind::WebSocketConnection => "WebSocket connection",
            GatewayCapacityKind::ActiveSession => "active session",
            GatewayCapacityKind::ReconnectLease => "reconnect lease",
            GatewayCapacityKind::Login => "login in-flight",
            GatewayCapacityKind::NewCharacter => "new-character in-flight",
            GatewayCapacityKind::StartGame => "StartGame in-flight",
        }
    }
}

impl Drop for GatewayCapacityPermit {
    fn drop(&mut self) {
        let previous = self.state.counter(self.kind).fetch_sub(1, Ordering::AcqRel);
        debug_assert!(
            previous > 0,
            "gateway capacity permit dropped with no matching acquisition"
        );
    }
}

#[derive(Debug, Default)]
struct ReconnectSessionStore {
    sessions: Mutex<HashMap<GatewaySessionCacheKey, ReconnectSessionLease>>,
}

#[derive(Debug)]
struct ReconnectSessionLease {
    session: GatewaySession,
    active_session_permit: Option<GatewayCapacityPermit>,
    _reconnect_lease_permit: GatewayCapacityPermit,
    expires_at: Instant,
}

#[derive(Debug)]
struct ReconnectSessionRestore {
    session: GatewaySession,
    active_session_permit: Option<GatewayCapacityPermit>,
}

#[derive(Debug, Clone, Copy)]
struct GatewaySaveQueueConfig {
    debounce: Duration,
    checkpoint: Duration,
    queue_limit: usize,
}

#[derive(Debug)]
struct WebSessionSaveQueue {
    config: GatewaySaveQueueConfig,
    dirty: bool,
    queued_requests: usize,
    last_saved_at: Instant,
}

#[derive(Debug, Clone, Copy)]
struct GatewayRouteRefreshConfig {
    interval: Duration,
}

#[derive(Debug)]
struct WebSessionRouteRefresh {
    config: GatewayRouteRefreshConfig,
    last_refreshed_at: Option<Instant>,
}

type SharedBackgroundRouteRefreshRecord = Arc<Mutex<Option<(GatewaySessionCacheRecord, String)>>>;

struct BackgroundRouteRefreshTask {
    handle: JoinHandle<()>,
}

impl GatewaySaveQueueConfig {
    fn from_env() -> Self {
        Self {
            debounce: Duration::from_millis(
                env::var("MIR2_GATEWAY_SAVE_DEBOUNCE_MS")
                    .ok()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .unwrap_or(1_500)
                    .max(1),
            ),
            checkpoint: Duration::from_secs(
                env::var("MIR2_GATEWAY_SAVE_CHECKPOINT_SECONDS")
                    .ok()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .unwrap_or(15)
                    .max(1),
            ),
            queue_limit: positive_usize_env("MIR2_GATEWAY_SAVE_QUEUE_LIMIT").unwrap_or(64),
        }
    }

    #[cfg(test)]
    fn new(debounce: Duration, checkpoint: Duration, queue_limit: usize) -> Self {
        Self {
            debounce,
            checkpoint,
            queue_limit: queue_limit.max(1),
        }
    }
}

impl WebSessionSaveQueue {
    fn new(config: GatewaySaveQueueConfig) -> Self {
        Self {
            config,
            dirty: false,
            queued_requests: 0,
            last_saved_at: Instant::now(),
        }
    }

    fn request_save(
        &mut self,
        now: Instant,
        save: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        self.dirty = true;
        self.queued_requests = self.queued_requests.saturating_add(1);
        if self.queued_requests >= self.config.queue_limit
            || now.duration_since(self.last_saved_at) >= self.config.debounce
        {
            return self.flush_now(now, save);
        }
        Ok(())
    }

    fn checkpoint(
        &mut self,
        now: Instant,
        save: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        if self.dirty && now.duration_since(self.last_saved_at) >= self.config.checkpoint {
            return self.flush_now(now, save);
        }
        Ok(())
    }

    fn flush_now(
        &mut self,
        now: Instant,
        save: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        save()?;
        self.dirty = false;
        self.queued_requests = 0;
        self.last_saved_at = now;
        Ok(())
    }

    fn force_save_now(
        &mut self,
        now: Instant,
        save: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        save()?;
        self.dirty = false;
        self.queued_requests = 0;
        self.last_saved_at = now;
        Ok(())
    }

    #[cfg(test)]
    fn has_pending_save(&self) -> bool {
        self.dirty
    }
}

impl GatewayRouteRefreshConfig {
    fn from_env() -> Self {
        Self {
            interval: Duration::from_millis(
                env::var("MIR2_GATEWAY_ROUTE_REFRESH_INTERVAL_MS")
                    .ok()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .unwrap_or(5_000)
                    .clamp(250, 30_000),
            ),
        }
    }

    #[cfg(test)]
    fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

impl WebSessionRouteRefresh {
    fn new(config: GatewayRouteRefreshConfig) -> Self {
        Self {
            config,
            last_refreshed_at: None,
        }
    }

    fn claim_refresh_due(&mut self, session: &GatewaySession, now: Instant, force: bool) -> bool {
        if session.active_identity().is_none() {
            return false;
        }
        if !force
            && self
                .last_refreshed_at
                .is_some_and(|last| now.duration_since(last) < self.config.interval)
        {
            return false;
        }
        self.last_refreshed_at = Some(now);
        true
    }

    fn maybe_refresh(
        &mut self,
        session_cache: &dyn crate::cache::GatewaySessionCache,
        session: &GatewaySession,
        now: Instant,
        force: bool,
    ) -> Result<bool, String> {
        if !self.claim_refresh_due(session, now, force) {
            return Ok(false);
        }
        refresh_session_cache_with_route_lease(session_cache, session, route_lease_ttl_seconds())?;
        Ok(true)
    }
}

impl Drop for BackgroundRouteRefreshTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl ReconnectSessionStore {
    fn store(
        &self,
        key: GatewaySessionCacheKey,
        session: GatewaySession,
        active_session_permit: Option<GatewayCapacityPermit>,
        reconnect_lease_permit: GatewayCapacityPermit,
        ttl: Duration,
    ) {
        let expires_at = Instant::now() + ttl.max(Duration::from_millis(1));
        let mut sessions = self
            .sessions
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut sessions);
        sessions.insert(
            key,
            ReconnectSessionLease {
                session,
                active_session_permit,
                _reconnect_lease_permit: reconnect_lease_permit,
                expires_at,
            },
        );
    }

    fn take(&self, key: &GatewaySessionCacheKey) -> Option<ReconnectSessionRestore> {
        let mut sessions = self
            .sessions
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut sessions);
        let lease = sessions.remove(key)?;
        if lease.expires_at <= Instant::now() {
            return None;
        }
        Some(ReconnectSessionRestore {
            session: lease.session,
            active_session_permit: lease.active_session_permit,
        })
    }

    fn purge_expired(&self) {
        let mut sessions = self
            .sessions
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut sessions);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        let mut sessions = self
            .sessions
            .lock()
            .expect("gateway reconnect session store mutex should not be poisoned");
        Self::purge_expired_locked(&mut sessions);
        sessions.len()
    }

    fn purge_expired_locked(sessions: &mut HashMap<GatewaySessionCacheKey, ReconnectSessionLease>) {
        let now = Instant::now();
        sessions.retain(|_, lease| lease.expires_at > now);
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum BrowserCommand {
    ClientVersion,
    Disconnect,
    Login {
        #[serde(alias = "accountId")]
        account_id: String,
        password: String,
    },
    PasskeyLogin {
        #[serde(alias = "accountId")]
        account_id: String,
        token: String,
    },
    NewAccount {
        #[serde(alias = "accountId")]
        account_id: String,
        password: String,
        #[serde(alias = "birthDateBinary", default)]
        birth_date_binary: i64,
        #[serde(alias = "userName", default)]
        user_name: String,
        #[serde(alias = "secretQuestion", default)]
        secret_question: String,
        #[serde(alias = "secretAnswer", default)]
        secret_answer: String,
        #[serde(alias = "emailAddress", default)]
        email_address: String,
    },
    ChangePassword {
        #[serde(alias = "accountId")]
        account_id: String,
        #[serde(alias = "currentPassword")]
        current_password: String,
        #[serde(alias = "newPassword")]
        new_password: String,
    },
    UnlockStorage {
        password: String,
    },
    SetStoragePassword {
        #[serde(alias = "currentPassword")]
        current_password: String,
        #[serde(alias = "newPassword")]
        new_password: String,
    },
    RemoveStoragePassword {
        #[serde(alias = "currentPassword")]
        current_password: String,
    },
    NewCharacter {
        name: String,
        gender: String,
        class: String,
    },
    NewHero {
        name: String,
        gender: String,
        class: String,
    },
    StartGame {
        #[serde(alias = "characterIndex")]
        character_index: i32,
    },
    Turn {
        direction: String,
    },
    Walk {
        direction: String,
    },
    Run {
        direction: String,
    },
    Chat {
        message: String,
    },
    KeepAlive {
        #[serde(alias = "time")]
        time: i64,
    },
    MoveTo {
        x: i32,
        y: i32,
        mode: Option<String>,
    },
    Attack {
        #[serde(alias = "objectId")]
        object_id: u32,
    },
    AttackDirection {
        direction: String,
        #[serde(default)]
        spell: Option<u8>,
    },
    RangeAttack {
        direction: String,
        x: i32,
        y: i32,
        #[serde(alias = "targetId")]
        target_id: u32,
        #[serde(alias = "targetX")]
        target_x: i32,
        #[serde(alias = "targetY")]
        target_y: i32,
    },
    Harvest {
        direction: String,
    },
    Interact {
        #[serde(alias = "objectId")]
        object_id: u32,
    },
    SelectNpcDialog {
        target: String,
    },
    SubmitNpcInput {
        value: String,
    },
    PickUp {
        #[serde(alias = "objectId")]
        object_id: u32,
    },
    PickUpTile,
    UseItem {
        #[serde(default)]
        key: Option<String>,
        #[serde(alias = "uniqueId", default)]
        unique_id: Option<u64>,
        #[serde(default)]
        slot: Option<u8>,
        #[serde(default)]
        grid: Option<String>,
    },
    MoveItem {
        grid: String,
        from: i32,
        to: i32,
    },
    MergeItem {
        #[serde(alias = "gridFrom")]
        grid_from: String,
        #[serde(alias = "gridTo")]
        grid_to: String,
        #[serde(alias = "idFrom")]
        id_from: u64,
        #[serde(alias = "idTo")]
        id_to: u64,
    },
    EquipItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        to: i32,
    },
    RemoveItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        to: i32,
    },
    RemoveSlotItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        #[serde(alias = "gridTo")]
        grid_to: String,
        #[serde(alias = "fromUniqueId")]
        from_unique_id: u64,
        to: i32,
    },
    SplitItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        grid: String,
        count: u16,
    },
    StoreItem {
        from: i32,
        to: i32,
    },
    TakeBackItem {
        from: i32,
        to: i32,
    },
    TakeBackHeroItem {
        from: i32,
        to: i32,
    },
    TransferHeroItem {
        from: i32,
        to: i32,
    },
    DropItem {
        key: String,
        #[serde(alias = "uniqueId", default)]
        unique_id: Option<u64>,
        #[serde(default = "default_drop_count")]
        count: u16,
        #[serde(alias = "heroInventory", default)]
        hero_inventory: bool,
    },
    DeleteItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        #[serde(default = "default_drop_count")]
        count: u16,
        #[serde(alias = "heroInventory", default)]
        hero_inventory: bool,
    },
    DropGold {
        amount: u32,
    },
    RequestItemInfo {
        #[serde(alias = "itemIndex")]
        item_index: i32,
    },
    SellItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        #[serde(default = "default_drop_count")]
        count: u16,
    },
    BuyItem {
        #[serde(alias = "itemIndex")]
        item_index: u64,
        #[serde(default = "default_drop_count")]
        count: u16,
        #[serde(alias = "panelType", default)]
        panel_type: u8,
    },
    RepairItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
    },
    SpecialRepairItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
    },
    DeleteCharacter {
        #[serde(alias = "characterIndex")]
        character_index: i32,
    },
    MagicKey {
        spell: String,
        key: u8,
        #[serde(alias = "oldKey", default)]
        old_key: u8,
    },
    Magic {
        #[serde(alias = "objectId", default)]
        object_id: u32,
        spell: String,
        direction: String,
        #[serde(alias = "targetId", default)]
        target_id: u32,
        #[serde(default)]
        x: i32,
        #[serde(default)]
        y: i32,
        #[serde(alias = "spellTargetLock", default)]
        spell_target_lock: bool,
    },
    SpellToggle {
        spell: String,
        #[serde(alias = "toggleState", default)]
        toggle_state: Option<i8>,
        #[serde(alias = "canUse", default)]
        can_use: Option<bool>,
    },
    SetHeroBehaviour {
        behaviour: u8,
    },
    ChangeHero {
        #[serde(alias = "listIndex")]
        list_index: i32,
    },
    // Crystal `C.SetAutoPotValue` (Shared/ClientPackets.cs:1221). Sets the
    // hero auto-potion HP/MP trigger percentage. `stat` is the Crystal `Stat`
    // byte (0 = HP, otherwise MP), matching MirConnection.SetAutoPotValue.
    SetAutoPotValue {
        stat: u8,
        value: u32,
    },
    // Crystal `C.SetAutoPotItem` (Shared/ClientPackets.cs:1240). Selects which
    // inventory item the hero auto-pots from. `grid` is the Crystal
    // `MirGridType` (HeroHpItem = 23, HeroMpItem = 24).
    SetAutoPotItem {
        grid: String,
        #[serde(alias = "itemIndex")]
        item_index: i32,
    },
    // Crystal `C.ChangeAMode` (Shared/ClientPackets.cs:756). Cycles the
    // player's attack mode (Peace/Group/Guild/etc.); server echoes
    // `S.ChangeAMode` (MirConnection.ChangeAMode, MirConnection.cs:1432).
    ChangeAMode {
        mode: u8,
    },
    // Crystal `C.ChangePMode` (Shared/ClientPackets.cs:771). Cycles the
    // player's pet command mode; server echoes `S.ChangePMode`
    // (MirConnection.ChangePMode, MirConnection.cs:1440).
    ChangePMode {
        mode: u8,
    },
    // Crystal `C.Opendoor` (Shared/ClientPackets.cs:2524). Opens a map door /
    // conquest siege gate by index; server runs Map.OpenDoor (auto-close after
    // 5s) and toggles conquest gate bookkeeping (PlayerObject @GATES, Gate.cs).
    OpenDoor {
        #[serde(alias = "doorIndex")]
        door_index: u8,
    },
    ConsignItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        price: u32,
        #[serde(alias = "marketType", default)]
        market_type: u8,
    },
    MarketSearch {
        #[serde(alias = "matchText")]
        match_text: String,
        #[serde(alias = "itemType", default)]
        item_type: u8,
        #[serde(alias = "userMode", default)]
        user_mode: bool,
        #[serde(alias = "minShape", default)]
        min_shape: i16,
        #[serde(alias = "maxShape", default = "default_market_max_shape")]
        max_shape: i16,
        #[serde(alias = "marketType", default)]
        market_type: u8,
    },
    MarketRefresh,
    MarketPage {
        page: i32,
    },
    MarketBuy {
        #[serde(alias = "auctionId")]
        auction_id: u64,
        #[serde(alias = "bidPrice", default)]
        bid_price: u32,
    },
    MarketGetBack {
        mode: u8,
        #[serde(alias = "auctionId")]
        auction_id: u64,
    },
    MarketSellNow {
        #[serde(alias = "auctionId")]
        auction_id: u64,
    },
    MarriageRequest,
    MarriageReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    ChangeMarriage,
    DivorceRequest,
    DivorceReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    AddMentor {
        name: String,
    },
    MentorReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    AllowMentor,
    CancelMentor,
    DepositTradeItem {
        from: i32,
        to: i32,
    },
    RetrieveTradeItem {
        from: i32,
        to: i32,
    },
    TradeRequest,
    TradeReply {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    TradeGold {
        amount: u32,
    },
    TradeConfirm {
        locked: bool,
    },
    TradeCancel,
    FishingCast {
        #[serde(alias = "castOut")]
        cast_out: bool,
    },
    FishingChangeAutocast {
        #[serde(alias = "autoCast")]
        auto_cast: bool,
    },
    SendMail {
        name: String,
        message: String,
        gold: u32,
        #[serde(alias = "itemsIdx")]
        items_idx: [u64; 5],
        stamped: bool,
    },
    ReadMail {
        #[serde(alias = "mailId")]
        mail_id: u64,
    },
    CollectParcel {
        #[serde(alias = "mailId")]
        mail_id: u64,
    },
    DeleteMail {
        #[serde(alias = "mailId")]
        mail_id: u64,
    },
    LockMail {
        #[serde(alias = "mailId")]
        mail_id: u64,
        lock: bool,
    },
    MailLockedItem {
        #[serde(alias = "uniqueId")]
        unique_id: u64,
        locked: bool,
    },
    MailCost {
        gold: u32,
        #[serde(alias = "itemsIdx")]
        items_idx: [u64; 5],
        stamped: bool,
    },
    UpdateIntelligentCreature {
        creature: ClientIntelligentCreature,
        #[serde(alias = "summonMe")]
        summon_me: bool,
        #[serde(alias = "unsummonMe")]
        unsummon_me: bool,
        #[serde(alias = "releaseMe")]
        release_me: bool,
    },
    IntelligentCreaturePickup {
        #[serde(alias = "mouseMode")]
        mouse_mode: bool,
        location: Point,
    },
    RequestIntelligentCreatureUpdates {
        update: bool,
    },
    AddFriend {
        name: String,
        blocked: bool,
    },
    RemoveFriend {
        #[serde(alias = "characterIndex")]
        character_index: i32,
    },
    RefreshFriends,
    AddMemo {
        #[serde(alias = "characterIndex")]
        character_index: i32,
        memo: String,
    },
    GetRanking {
        #[serde(alias = "rankType")]
        rank_type: u8,
        #[serde(alias = "rankIndex", default)]
        rank_index: i32,
        #[serde(alias = "onlineOnly", default)]
        online_only: bool,
    },
    GetRentedItems,
    ItemRentalRequest,
    ItemRentalFee {
        amount: u32,
    },
    ItemRentalPeriod {
        days: u32,
    },
    DepositRentalItem {
        from: i32,
        to: i32,
    },
    RetrieveRentalItem {
        from: i32,
        to: i32,
    },
    CancelItemRental,
    ItemRentalLockFee,
    ItemRentalLockItem,
    ConfirmItemRental,
    AcceptQuest {
        #[serde(alias = "npcIndex", default)]
        npc_index: u32,
        #[serde(alias = "questIndex")]
        quest_index: i32,
    },
    FinishQuest {
        #[serde(alias = "questIndex")]
        quest_index: i32,
        #[serde(alias = "selectedItemIndex", default = "default_selected_item_index")]
        selected_item_index: i32,
    },
    AbandonQuest {
        #[serde(alias = "questIndex")]
        quest_index: i32,
    },
    ShareQuest {
        #[serde(alias = "questIndex")]
        quest_index: i32,
    },
    SwitchGroup {
        #[serde(alias = "allowGroup")]
        allow_group: bool,
    },
    AddMember {
        name: String,
    },
    DelMember {
        name: String,
    },
    GroupInvite {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    EditGuildMember {
        #[serde(alias = "changeType")]
        change_type: u8,
        #[serde(alias = "rankIndex", default)]
        rank_index: u8,
        #[serde(default)]
        name: String,
        #[serde(alias = "rankName", default)]
        rank_name: String,
    },
    EditGuildNotice {
        #[serde(default)]
        notice: Vec<String>,
    },
    GuildInvite {
        #[serde(alias = "acceptInvite")]
        accept_invite: bool,
    },
    GuildNameReturn {
        name: String,
    },
    RequestGuildInfo {
        #[serde(alias = "infoType", default)]
        info_type: u8,
    },
    GuildStorageGoldChange {
        #[serde(alias = "changeType")]
        change_type: u8,
        amount: u32,
    },
    GuildStorageItemChange {
        #[serde(alias = "changeType")]
        change_type: u8,
        from: i32,
        to: i32,
    },
    CastSkill {
        key: String,
    },
    TransferMap {
        key: String,
    },
    Stage5Command {
        action: String,
        #[serde(default)]
        args: Vec<String>,
    },
    SetLanguage {
        language: String,
    },
    Tick,
    LogOut,
}

#[derive(Debug)]
enum SessionAction {
    Packet(ClientPacket),
    PasskeyLogin { account_id: String, token: String },
    MoveTo { x: i32, y: i32, running: bool },
    Attack { object_id: u32 },
    Interact { object_id: u32 },
    SelectNpcDialog { target: String },
    SubmitNpcInput { value: String },
    PickUp { object_id: u32 },
    UseItem { key: String },
    DropItem { key: String },
    CastSkill { key: String },
    TransferMap { key: String },
    Stage5Command { action: String, args: Vec<String> },
    SetLanguage { language: String },
    Tick,
}

/// Default `selected_item_index` for `FinishQuest`: `-1` means "no reward
/// choice", which the simulation maps to `None` (see `stage5_finish_quest_packet`).
fn default_selected_item_index() -> i32 {
    -1
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    http: &'static str,
    ws: &'static str,
    tcp_stub: &'static str,
    session_cache: GatewaySessionCacheStatus,
    capacity: GatewayCapacityStatus,
    gameplay_events: GameplayEventSinkStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminSystemMailRequest {
    target_kind: Stage5MailTargetKind,
    target_id: String,
    from: String,
    subject: String,
    body: String,
    #[serde(default)]
    gold: u32,
    #[serde(default)]
    items: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminKickPlayerRequest {
    character_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminKickPlayerReceipt {
    character_id: String,
    removed: bool,
    account_id: Option<String>,
    character_index: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminSessionsResponse {
    source: String,
    sessions: Vec<GatewaySessionCacheRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminControlRequest {
    action: String,
    target: Option<String>,
    operator_id: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminControlReceipt {
    action: String,
    target: Option<String>,
    accepted: bool,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminErrorResponse {
    error: String,
}

pub async fn run_web_gateway(addr: &str, config: GatewayConfig) -> io::Result<()> {
    // Activated Crystal world: host every map full-size in the shared zone so
    // players roam all of Bichon (and reach every transfer), not just the
    // starter slice. Empty maps stay dormant regardless.
    if config.monster_spawn_source == mir2_simulation::MonsterSpawnSource::CrystalWorld {
        mir2_simulation::set_crystal_full_world_zone_collision(true);
    }
    let state = WebState {
        config: Arc::new(config),
        zone_registry: Arc::new(ZoneRegistry::in_process_with_owner_lease_authority(
            crate::zone_lease::default_zone_owner_lease_authority_from_env(),
        )),
        session_cache: gateway_session_cache_from_env()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?,
        reconnect_sessions: Arc::new(ReconnectSessionStore::default()),
        capacity: Arc::new(GatewayCapacityState::from_env()),
        gameplay_event_sink: default_gameplay_event_sink_from_env(),
        injector: crate::inject::LiveSessionInjector::default(),
    };

    probe_mark_started();

    let app = Router::new()
        .route("/", get(manual_ui))
        .route("/health", get(health))
        .route("/metrics", get(metrics_handler))
        .route("/admin/system-mail", post(admin_system_mail))
        .route("/admin/sessions", get(admin_sessions))
        .route("/admin/kick-player", post(admin_kick_player))
        .route("/admin/control", post(admin_control))
        .route("/onchain/inject", post(onchain_inject))
        .route("/ws", get(ws_upgrade))
        .with_state(state);

    let listener = TcpListener::bind(addr).await?;
    eprintln!("mir2-gateway web listening on http://{addr}");

    axum::serve(listener, app)
        .await
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
}

async fn manual_ui() -> Html<&'static str> {
    Html(include_str!("../static/manual.html"))
}

async fn health(State(state): State<WebState>) -> Json<HealthResponse> {
    state.reconnect_sessions.purge_expired();
    let session_cache = Arc::clone(&state.session_cache);
    let session_cache_status =
        tokio::task::spawn_blocking(move || gateway_session_cache_status(session_cache.as_ref()))
            .await
            .unwrap_or_else(|error| GatewaySessionCacheStatus {
                configured: true,
                backend: "unknown".to_string(),
                ttl_seconds: None,
                record_count: 0,
                stale_record_count: 0,
                route_lease_count: 0,
                healthy: false,
                last_error: Some(format!("session-cache health task failed: {error}")),
            });
    Json(HealthResponse {
        ok: true,
        http: "ready",
        ws: "ready",
        tcp_stub: "ready",
        session_cache: session_cache_status,
        capacity: state.capacity.status(),
        gameplay_events: gameplay_event_sink_status(state.gameplay_event_sink.as_ref()),
    })
}

/// Gateway probe endpoint — `GET /metrics`. Returns cumulative tick/save
/// timing and a world-snapshot push count since process startup. The probe
/// on the web side polls this endpoint (via the L5 layer reader) to surface
/// gateway-side latency in the dev overlay; it is intentionally **separate**
/// from `/health` so health-checks stay cheap and unchanged.
async fn metrics_handler() -> impl IntoResponse {
    let tick_count = PROBE_TICK_COUNT.load(Ordering::Relaxed);
    let tick_total = PROBE_TICK_MS_TOTAL.load(Ordering::Relaxed);
    let tick_last = PROBE_TICK_MS_LAST.load(Ordering::Relaxed);
    let tick_max = PROBE_TICK_MS_MAX.load(Ordering::Relaxed);
    let save_count = PROBE_SAVE_COUNT.load(Ordering::Relaxed);
    let save_total = PROBE_SAVE_MS_TOTAL.load(Ordering::Relaxed);
    let save_last = PROBE_SAVE_MS_LAST.load(Ordering::Relaxed);
    let save_max = PROBE_SAVE_MS_MAX.load(Ordering::Relaxed);
    let world_snapshot_count = PROBE_WORLD_SNAPSHOT_COUNT.load(Ordering::Relaxed);
    let started_at_ms = PROBE_STARTED_AT_MS.load(Ordering::Relaxed);
    let sampled_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    (
        [
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
            (header::ACCESS_CONTROL_ALLOW_METHODS, "GET"),
            (header::ACCESS_CONTROL_ALLOW_HEADERS, "accept"),
        ],
        Json(GatewayProbeMetrics {
            started_at_ms,
            sampled_at_ms,
            tick: probe_metric_summary(tick_count, tick_total, tick_last, tick_max),
            shared_tick: shared_tick_probe_metrics(),
            save: probe_metric_summary(save_count, save_total, save_last, save_max),
            world_snapshot_count,
        }),
    )
}

async fn admin_system_mail(
    State(state): State<WebState>,
    Json(request): Json<AdminSystemMailRequest>,
) -> Result<Json<mir2_simulation::Stage5MailDeliveryReceipt>, (StatusCode, Json<AdminErrorResponse>)>
{
    let receipt = deliver_stage5_system_mail(
        &state.config,
        Stage5MailDelivery {
            target_kind: request.target_kind,
            target_id: request.target_id,
            from: request.from,
            subject: request.subject,
            body: request.body,
            gold: request.gold,
            items: request.items,
        },
    )
    .map_err(|error| (StatusCode::BAD_REQUEST, Json(AdminErrorResponse { error })))?;
    Ok(Json(receipt))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OnchainInjectReceipt {
    accepted: bool,
    connected: bool,
    packet_count: usize,
    idempotency_key: String,
}

/// Extract a `Bearer <token>` from the Authorization header.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// M4 WF-5: the trusted Relayer POSTs chain-confirmed commands here. Operator-token
/// authenticated; routed to the target account's LIVE session via the injector. Idempotency
/// place #3 lives in the sim (duplicate `idempotency_key` is a no-op there).
async fn onchain_inject(
    State(state): State<WebState>,
    headers: HeaderMap,
    Json(command): Json<crate::inject::OnchainInjectCommand>,
) -> Result<Json<OnchainInjectReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    let token = bearer_token(&headers).ok_or((
        StatusCode::UNAUTHORIZED,
        Json(AdminErrorResponse {
            error: "missing operator bearer token".to_string(),
        }),
    ))?;
    crate::auth::verify_operator_token(token)
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(AdminErrorResponse { error })))?;

    let idempotency_key = command.idempotency_key().to_string();
    let world_command = command
        .to_world_command()
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(AdminErrorResponse { error })))?;
    // Render-only (MineDepleted) has no sim WorldCommand — accept + ignore (200) so the
    // relayer's retry loop stops.
    let Some(world_command) = world_command else {
        return Ok(Json(OnchainInjectReceipt {
            accepted: true,
            connected: false,
            packet_count: 0,
            idempotency_key,
        }));
    };
    let Some(account) = command.account().map(str::to_string) else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AdminErrorResponse {
                error: "command has no target account".to_string(),
            }),
        ));
    };
    match state.injector.dispatch(&account, world_command).await {
        Ok(outcome) => Ok(Json(OnchainInjectReceipt {
            accepted: true,
            connected: true,
            packet_count: outcome.packet_count,
            idempotency_key,
        })),
        // Player offline: accept (200) so the relayer stops retrying. NOTE: the grant is NOT
        // persisted for offline players in M4 — offline delivery (persist/mail) is M6.
        Err(crate::inject::InjectionError::NotConnected) => Ok(Json(OnchainInjectReceipt {
            accepted: true,
            connected: false,
            packet_count: 0,
            idempotency_key,
        })),
        Err(crate::inject::InjectionError::SessionGone) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AdminErrorResponse {
                error: "target session became unavailable".to_string(),
            }),
        )),
    }
}

async fn admin_kick_player(
    State(state): State<WebState>,
    Json(request): Json<AdminKickPlayerRequest>,
) -> Result<Json<AdminKickPlayerReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    if request.character_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AdminErrorResponse {
                error: "character_id is required".to_string(),
            }),
        ));
    }
    let removed = state
        .session_cache
        .remove_character(request.character_id.trim());
    Ok(Json(AdminKickPlayerReceipt {
        character_id: request.character_id,
        removed: removed.is_some(),
        account_id: removed.as_ref().map(|record| record.key.account_id.clone()),
        character_index: removed.as_ref().map(|record| record.key.character_index),
    }))
}

async fn admin_sessions(State(state): State<WebState>) -> Json<AdminSessionsResponse> {
    let mut sessions = state.session_cache.list();
    sessions.sort_by(|left, right| {
        left.character_name
            .cmp(&right.character_name)
            .then_with(|| left.key.account_id.cmp(&right.key.account_id))
            .then(left.key.character_index.cmp(&right.key.character_index))
    });
    Json(AdminSessionsResponse {
        source: "gateway_session_cache".to_string(),
        sessions,
    })
}

async fn admin_control(
    Json(request): Json<AdminControlRequest>,
) -> Result<Json<AdminControlReceipt>, (StatusCode, Json<AdminErrorResponse>)> {
    let action = canonical_admin_control_action(&request.action)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(AdminErrorResponse { error })))?;
    let message = match action.as_str() {
        "status" => "gateway is ready".to_string(),
        "reload_npcs" => {
            "NPC reload request accepted; generated manifests are active for new sessions"
                .to_string()
        }
        "reload_drops" => {
            "drop reload request accepted; generated manifests are active for new sessions"
                .to_string()
        }
        "reload_line_message" => "line-message reload request accepted".to_string(),
        "clear_blocked_ips" => "blocked IP cache cleared".to_string(),
        "start" => "gateway process is already running".to_string(),
        "stop" | "reboot" | "close" => {
            return Err((
                StatusCode::CONFLICT,
                Json(AdminErrorResponse {
                    error: format!(
                        "{action} is recorded by Admin API but not executed by the in-process dev gateway"
                    ),
                }),
            ));
        }
        _ => unreachable!("canonical action should be validated"),
    };
    Ok(Json(AdminControlReceipt {
        action,
        target: request.target,
        accepted: true,
        message: match (request.operator_id, request.reason) {
            (Some(operator_id), Some(reason)) if !reason.trim().is_empty() => {
                format!("{message}; operator={operator_id}; reason={reason}")
            }
            (Some(operator_id), _) => format!("{message}; operator={operator_id}"),
            _ => message,
        },
    }))
}

fn canonical_admin_control_action(action: &str) -> Result<String, String> {
    let action = action
        .trim()
        .replace('-', "_")
        .replace(' ', "_")
        .to_ascii_lowercase();
    let canonical = match action.as_str() {
        "status" | "health" => "status",
        "reload_npcs" | "reload_npc" | "reloadnpc" => "reload_npcs",
        "reload_drops" | "reload_drop" | "reloaddrops" => "reload_drops",
        "reload_line_message" | "reload_line_messages" | "reloadlinemessage" => {
            "reload_line_message"
        }
        "clear_blocked_ips" | "clear_blocked_ip" | "clearblockedips" => "clear_blocked_ips",
        "start" | "start_server" => "start",
        "stop" | "stop_server" => "stop",
        "reboot" | "restart" => "reboot",
        "close" | "shutdown" => "close",
        "" => return Err("action is required".to_string()),
        other => return Err(format!("unsupported admin control action: {other}")),
    };
    Ok(canonical.to_string())
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<WebState>) -> Response {
    let ws_connection_permit = match state.capacity.try_acquire_ws_connection() {
        Ok(permit) => permit,
        Err(error) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AdminErrorResponse { error }),
            )
                .into_response();
        }
    };
    ws.on_upgrade(move |socket| handle_socket(socket, state, ws_connection_permit))
}

async fn handle_socket(
    socket: WebSocket,
    state: WebState,
    _ws_connection_permit: GatewayCapacityPermit,
) {
    let mut session = new_gateway_session_for_web(&state);
    let mut active_session_permit: Option<GatewayCapacityPermit> = None;
    let mut save_queue = WebSessionSaveQueue::new(GatewaySaveQueueConfig::from_env());
    let mut route_refresh = WebSessionRouteRefresh::new(GatewayRouteRefreshConfig::from_env());
    handle_socket_inner(
        socket,
        &mut session,
        Arc::clone(&state.session_cache),
        Arc::clone(&state.reconnect_sessions),
        Arc::clone(&state.capacity),
        &mut active_session_permit,
        &mut save_queue,
        &mut route_refresh,
        state.injector.clone(),
    )
    .await;
    let _ = tokio::task::block_in_place(|| {
        catch_gateway_panic("web refresh_active_external_mail", || {
            session.refresh_active_external_mail()
        })
    });
    let _ = save_queue.force_save_now(Instant::now(), || {
        tokio::task::block_in_place(|| {
            catch_gateway_panic("web save_active_character", || {
                session.save_active_character()
            })
        })
    });
    if let Some(key) = session_cache_key(&session) {
        let grace_seconds = reconnect_grace_ttl_seconds();
        if let Err(error) = refresh_session_cache_with_route_lease(
            state.session_cache.as_ref(),
            &session,
            grace_seconds,
        ) {
            eprintln!("web reconnect route lease refresh skipped: {error}");
        }
        if active_session_permit.is_none() {
            match state.capacity.try_acquire_active_session() {
                Ok(permit) => active_session_permit = Some(permit),
                Err(error) => {
                    eprintln!("web reconnect grace skipped: {error}");
                    let _ = remove_owned_session_cache(state.session_cache.as_ref(), &session);
                    return;
                }
            }
        }
        state.reconnect_sessions.purge_expired();
        let reconnect_lease_permit = match state.capacity.try_acquire_reconnect_lease() {
            Ok(permit) => permit,
            Err(error) => {
                eprintln!("web reconnect grace skipped: {error}");
                let _ = remove_owned_session_cache(state.session_cache.as_ref(), &session);
                return;
            }
        };
        let log_account_id = key.account_id.clone();
        let log_character_index = key.character_index;
        state.reconnect_sessions.store(
            key,
            session,
            active_session_permit.take(),
            reconnect_lease_permit,
            Duration::from_secs(grace_seconds),
        );
        schedule_reconnect_session_purge(
            Arc::clone(&state.reconnect_sessions),
            Duration::from_secs(grace_seconds),
        );
        eprintln!(
            "web reconnect grace retained session for {log_account_id}/{log_character_index} for {grace_seconds}s"
        );
        return;
    }
    let _ = remove_owned_session_cache(state.session_cache.as_ref(), &session);
}

fn new_gateway_session_for_web(state: &WebState) -> GatewaySession {
    let mut session = match &state.gameplay_event_sink {
        Some(sink) => GatewaySession::new_with_zone_registry_and_event_sink(
            (*state.config).clone(),
            &state.zone_registry,
            Arc::clone(sink),
        ),
        None => {
            GatewaySession::new_with_zone_registry((*state.config).clone(), &state.zone_registry)
        }
    };
    session.configure_zone_owner_heartbeat(gateway_zone_owner_heartbeat_interval_ms(), 0);
    session
}

// `_injection_registration` is an RAII Drop-guard; reassigning it (to re-register on
// re-login or unregister on logout) drops the prior guard intentionally.
#[allow(unused_assignments)]
async fn handle_socket_inner(
    socket: WebSocket,
    session: &mut GatewaySession,
    session_cache: SharedGatewaySessionCache,
    reconnect_sessions: Arc<ReconnectSessionStore>,
    capacity: Arc<GatewayCapacityState>,
    active_session_permit: &mut Option<GatewayCapacityPermit>,
    save_queue: &mut WebSessionSaveQueue,
    route_refresh: &mut WebSessionRouteRefresh,
    injector: crate::inject::LiveSessionInjector,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut runtime_tick = tokio::time::interval(gateway_runtime_tick_interval());
    runtime_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut runtime_tick_deferred_until = Instant::now();
    let enforce_player_command_safety = production_player_command_safety_enabled();
    let mut authenticated = false;
    let mut authenticated_account_id: Option<String> = None;
    // M4 WF-5: this socket task registers an injection channel keyed by its account so the
    // /onchain/inject HTTP handler can hand chain-confirmed commands to it. The RAII guard
    // unregisters on every exit path (the loop has many early returns).
    let (inject_tx, mut inject_rx) =
        tokio::sync::mpsc::unbounded_channel::<crate::inject::InjectionMessage>();
    let mut _injection_registration: Option<crate::inject::InjectionRegistration> = None;
    let background_route_refresh_record: SharedBackgroundRouteRefreshRecord =
        Arc::new(Mutex::new(None));
    let _background_route_refresh = spawn_background_route_lease_refresh(
        Arc::clone(&session_cache),
        route_refresh.config,
        Arc::clone(&background_route_refresh_record),
    );

    let connect_packets = match catch_gateway_panic("web on_connect", || session.on_connect()) {
        Ok(packets) => packets,
        Err(error) => {
            let _ = send_error_message(&mut sender, &error).await;
            return;
        }
    };

    for packet in connect_packets {
        if send_server_packet(&mut sender, &packet).await.is_err() {
            return;
        }
    }
    if let Err(error) = tokio::task::block_in_place(|| refresh_external_session_state(session)) {
        let _ = send_error_message(&mut sender, &error).await;
        return;
    }
    if send_world_snapshot(&mut sender, &session).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            biased;

            message_result = receiver.next() => {
                let message = match message_result {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Close(_))) | None => return,
                    Some(Ok(_)) => continue,
                    Some(Err(_)) => return,
                };

                let command = match serde_json::from_str::<BrowserCommand>(&message) {
                    Ok(command) => command,
                    Err(error) => {
                        let _ = send_error_message(&mut sender, &format!("invalid command: {error}")).await;
                        continue;
                    }
                };

                let action = match browser_command_to_action(command) {
                    Ok(action) => action,
                    Err(error) => {
                        let _ = send_error_message(&mut sender, &error).await;
                        continue;
                    }
                };

                let should_send_snapshot_by_action = should_send_world_snapshot_for_action(&action);
                let low_latency_action = is_low_latency_action(&action);
                let should_queue_save_by_action = should_queue_save_for_action(&action);
                let runtime_tick_defer_duration = runtime_tick_defer_duration_for_action(&action);
                let login_account_id = login_account_id_for_action(&action).map(str::to_string);
                let keep_alive_time = keep_alive_time_for_action(&action);
                if let Some(time) = keep_alive_time {
                    if send_server_packet(&mut sender, &ServerPacket::KeepAlive { time })
                        .await
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
                let start_game_character_index = start_game_character_index_for_action(&action);
                if let (true, Some(account_id), Some(character_index)) = (
                    authenticated,
                    authenticated_account_id.as_deref(),
                    start_game_character_index,
                ) {
                    let key = GatewaySessionCacheKey {
                        account_id: account_id.to_string(),
                        character_index,
                    };
                    if let Some(restored) = reconnect_sessions.take(&key) {
                        let restored_session_id = restored.session.session_id().to_string();
                        *session = restored.session;
                        *active_session_permit = restored.active_session_permit;
                        eprintln!(
                            "web reconnect grace restored session {restored_session_id} for {}/{}",
                            key.account_id, key.character_index
                        );
                    }
                }
                let pending_start_game_route_lease = match try_acquire_start_game_route_lease(
                    session_cache.as_ref(),
                    session,
                    authenticated,
                    authenticated_account_id.as_deref(),
                    start_game_character_index,
                ) {
                    Ok(key) => key,
                    Err(error) => {
                        let _ = send_error_message(&mut sender, &error).await;
                        continue;
                    }
                };
                let action_capacity_permit = match inflight_capacity_kind_for_action(&action) {
                    Some(kind) => match capacity.try_acquire_action(kind) {
                        Ok(permit) => Some(permit),
                        Err(error) => {
                            release_pending_start_game_route_lease(
                                session_cache.as_ref(),
                                session,
                                pending_start_game_route_lease.as_ref(),
                            );
                            let _ = send_error_message(&mut sender, &error).await;
                            continue;
                        }
                    },
                    None => None,
                };
                let mut pending_active_session_permit = None;
                if start_game_character_index.is_some()
                    && session.active_identity().is_none()
                    && active_session_permit.is_none()
                {
                    match capacity.try_acquire_active_session() {
                        Ok(permit) => pending_active_session_permit = Some(permit),
                        Err(error) => {
                            release_pending_start_game_route_lease(
                                session_cache.as_ref(),
                                session,
                                pending_start_game_route_lease.as_ref(),
                            );
                            let _ = send_error_message(&mut sender, &error).await;
                            continue;
                        }
                    }
                }
                let responses = match catch_gateway_panic("web session action", || {
                    tokio::task::block_in_place(|| {
                        execute_session_action(
                            session,
                            action,
                            authenticated,
                            enforce_player_command_safety,
                        )
                    })
                }) {
                    Ok(Ok(responses)) => responses,
                    Ok(Err(error)) => {
                        release_pending_start_game_route_lease(
                            session_cache.as_ref(),
                            session,
                            pending_start_game_route_lease.as_ref(),
                        );
                        let _ = send_error_message(&mut sender, &error).await;
                        continue;
                    }
                    Err(error) => {
                        release_pending_start_game_route_lease(
                            session_cache.as_ref(),
                            session,
                            pending_start_game_route_lease.as_ref(),
                        );
                        let _ = send_error_message(&mut sender, &error).await;
                        return;
                    }
                };
                drop(action_capacity_permit);
                release_unclaimed_start_game_route_lease(
                    session_cache.as_ref(),
                    session,
                    pending_start_game_route_lease.as_ref(),
                );
                if pending_active_session_permit.is_some() && session.active_identity().is_some() {
                    *active_session_permit = pending_active_session_permit.take();
                }
                let force_route_refresh = start_game_character_index.is_some();
                let next_authenticated = update_authenticated_state(authenticated, &responses);
                if next_authenticated {
                    if let Some(account_id) = login_account_id {
                        _injection_registration = Some(crate::inject::InjectionRegistration::new(
                            injector.clone(),
                            &account_id,
                            inject_tx.clone(),
                        ));
                        authenticated_account_id = Some(account_id);
                    }
                } else if authenticated {
                    authenticated_account_id = None;
                    _injection_registration = None;
                }
                authenticated = next_authenticated;

                if let Err(error) = flush_session_updates(
                    &mut sender,
                    session,
                    session_cache.as_ref(),
                    save_queue,
                    route_refresh,
                    responses,
                    should_send_snapshot_by_action,
                    low_latency_action,
                    should_queue_save_by_action,
                    force_route_refresh,
                )
                .await
                {
                    let _ = send_error_message(&mut sender, &error).await;
                    return;
                }
                if force_route_refresh || !low_latency_action {
                    update_background_route_refresh_record(
                        &background_route_refresh_record,
                        session,
                    );
                }
                if let Some(duration) = runtime_tick_defer_duration {
                    runtime_tick_deferred_until = Instant::now() + duration;
                }
            }
            maybe_injection = inject_rx.recv() => {
                let Some(crate::inject::InjectionMessage { command, reply }) = maybe_injection else {
                    continue;
                };
                // Apply the chain-confirmed command authoritatively (Direct mode), then push
                // the resulting packets to this player's socket.
                let outcome = catch_gateway_panic("web onchain injection", || {
                    tokio::task::block_in_place(|| session.execute_with_outcome(command))
                });
                let responses = match outcome {
                    Ok(Ok(execution)) => execution.packets,
                    Ok(Err(error)) => {
                        eprintln!("web onchain injection rejected: {error}");
                        let _ = reply.send(crate::inject::InjectionOutcome { packet_count: 0 });
                        continue;
                    }
                    Err(error) => {
                        let _ = send_error_message(&mut sender, &error).await;
                        return;
                    }
                };
                let packet_count = responses.len();
                if let Err(error) = flush_session_updates(
                    &mut sender,
                    session,
                    session_cache.as_ref(),
                    save_queue,
                    route_refresh,
                    responses,
                    true,
                    false,
                    true,
                    false,
                )
                .await
                {
                    let _ = send_error_message(&mut sender, &error).await;
                    return;
                }
                let _ = reply.send(crate::inject::InjectionOutcome { packet_count });
            }
            _ = runtime_tick.tick() => {
                let now = Instant::now();
                if let Err(error) = catch_gateway_panic("web zone owner heartbeat", || {
                    tokio::task::block_in_place(|| session.renew_zone_owner_lease_if_due())
                }).and_then(|result| result.map(|_| ())) {
                    let _ = send_error_message(&mut sender, &error).await;
                    return;
                }
                if now < runtime_tick_deferred_until {
                    continue;
                }
                let tick_start = Instant::now();
                let responses = match catch_gateway_panic("web session tick", || {
                    tokio::task::block_in_place(|| session.tick())
                }) {
                    Ok(responses) => responses,
                    Err(error) => {
                        let _ = send_error_message(&mut sender, &error).await;
                        return;
                    }
                };
                probe_record_tick(tick_start);
                if responses.is_empty() {
                    // Save timing: we time only when SaveQueue.flush_now actually
                    // invokes the closure (so dirty-mark-only checkpoints do not
                    // pollute the metric). We use a Cell<bool> sentinel that the
                    // closure sets on entry; flush_now's `if !self.dirty` early
                    // return skips the closure when no flush is needed.
                    let save_start = Instant::now();
                    let save_invoked = std::cell::Cell::new(false);
                    let invoked_ref = &save_invoked;
                    let session_ref: &mut GatewaySession = session;
                    let save_closure = move || {
                        invoked_ref.set(true);
                        tokio::task::block_in_place(|| {
                            catch_gateway_panic("web save_active_character", || {
                                session_ref.save_active_character()
                            })
                        })?;
                        Ok::<(), String>(())
                    };
                    if let Err(error) = save_queue.checkpoint(now, save_closure) {
                        let _ = send_error_message(&mut sender, &error).await;
                        return;
                    }
                    if save_invoked.get() {
                        probe_record_save(save_start);
                    }
                    continue;
                }
                if let Err(error) = flush_session_updates(
                    &mut sender,
                    session,
                    session_cache.as_ref(),
                    save_queue,
                    route_refresh,
                    responses,
                    false,
                    true,
                    false,
                    false,
                )
                .await
                {
                    let _ = send_error_message(&mut sender, &error).await;
                    return;
                }
            }
        }
    }
}

fn spawn_background_route_lease_refresh(
    session_cache: SharedGatewaySessionCache,
    config: GatewayRouteRefreshConfig,
    record: SharedBackgroundRouteRefreshRecord,
) -> BackgroundRouteRefreshTask {
    let handle = tokio::spawn(async move {
        let mut tick = tokio::time::interval(config.interval);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let Some((record, owner)) = record
                .lock()
                .expect("background route refresh record mutex should not be poisoned")
                .clone()
            else {
                continue;
            };
            let session_cache = Arc::clone(&session_cache);
            let result = tokio::task::spawn_blocking(move || {
                session_cache.refresh_owned_route_lease_record(
                    record,
                    &owner,
                    route_lease_ttl_seconds(),
                )
            })
            .await;
            match result {
                Ok(Ok(true)) | Ok(Ok(false)) => {}
                Ok(Err(error)) => eprintln!("web background route lease refresh skipped: {error}"),
                Err(error) => eprintln!("web background route lease refresh task failed: {error}"),
            }
        }
    });
    BackgroundRouteRefreshTask { handle }
}

fn update_background_route_refresh_record(
    record: &SharedBackgroundRouteRefreshRecord,
    session: &GatewaySession,
) {
    let next =
        session_cache_record(session).map(|record| (record, session.session_id().to_string()));
    *record
        .lock()
        .expect("background route refresh record mutex should not be poisoned") = next;
}

fn schedule_reconnect_session_purge(
    reconnect_sessions: Arc<ReconnectSessionStore>,
    grace: Duration,
) {
    tokio::spawn(async move {
        tokio::time::sleep(grace.max(Duration::from_millis(1)) + Duration::from_millis(50)).await;
        reconnect_sessions.purge_expired();
    });
}

const DEFAULT_GATEWAY_RUNTIME_TICK_MS: u64 = 300;
const DEFAULT_GATEWAY_RUNTIME_TICK_BOOTSTRAP_GRACE_MS: u64 = 15_000;
const DEFAULT_GATEWAY_ZONE_OWNER_HEARTBEAT_MS: u64 = 10_000;

async fn flush_session_updates(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    session: &mut GatewaySession,
    session_cache: &dyn crate::cache::GatewaySessionCache,
    save_queue: &mut WebSessionSaveQueue,
    route_refresh: &mut WebSessionRouteRefresh,
    responses: Vec<ServerPacket>,
    should_send_snapshot_by_action: bool,
    low_latency_action: bool,
    should_queue_save_by_action: bool,
    force_route_refresh: bool,
) -> Result<(), String> {
    let response_requires_snapshot = responses_require_world_snapshot(&responses);

    for response in responses {
        send_server_packet(sender, &response)
            .await
            .map_err(|error| error.to_string())?;
    }

    if low_latency_action {
        return Ok(());
    }

    let external_state_changed =
        tokio::task::block_in_place(|| refresh_external_session_state(session))?;
    let should_send_snapshot =
        should_send_snapshot_by_action || response_requires_snapshot || external_state_changed;

    if should_send_snapshot {
        send_world_snapshot(sender, session).await?;
        probe_record_world_snapshot();
    }

    // Save timing: same Cell<bool> pattern as in the tick loop — record only
    // when flush_now actually invokes the closure. Route-lease refresh runs
    // BEFORE the save closure is built so the borrow of `session` is released
    // before closure construction moves it.
    if let Err(error) =
        route_refresh.maybe_refresh(session_cache, session, Instant::now(), force_route_refresh)
    {
        eprintln!("web session route lease refresh skipped: {error}");
    }

    let has_active_identity = session.active_identity().is_some();
    let use_request_save =
        !low_latency_action && should_queue_save_by_action && has_active_identity;
    let save_start = Instant::now();
    let save_invoked = std::cell::Cell::new(false);
    let invoked_ref = &save_invoked;
    let session_ref: &mut GatewaySession = session;
    let save_closure = move || {
        invoked_ref.set(true);
        tokio::task::block_in_place(|| {
            catch_gateway_panic("web save_active_character", || {
                session_ref.save_active_character()
            })
        })?;
        Ok::<(), String>(())
    };
    if use_request_save {
        save_queue.request_save(Instant::now(), save_closure)?;
    } else {
        save_queue.checkpoint(Instant::now(), save_closure)?;
    }
    if save_invoked.get() {
        probe_record_save(save_start);
    }

    Ok(())
}

fn route_lease_ttl_seconds() -> u64 {
    std::env::var("MIR2_GATEWAY_ROUTE_LEASE_TTL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or_else(|| {
            std::env::var("MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
        })
        .unwrap_or(30)
        .max(1)
}

fn reconnect_grace_ttl_seconds() -> u64 {
    std::env::var("MIR2_GATEWAY_RECONNECT_GRACE_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(15)
        .clamp(1, 120)
}

fn gateway_runtime_tick_interval() -> Duration {
    duration_from_millis_env(
        "MIR2_GATEWAY_RUNTIME_TICK_MS",
        DEFAULT_GATEWAY_RUNTIME_TICK_MS,
        100,
        5_000,
    )
}

fn gateway_runtime_tick_bootstrap_grace() -> Duration {
    duration_from_millis_env(
        "MIR2_GATEWAY_RUNTIME_TICK_BOOTSTRAP_GRACE_MS",
        DEFAULT_GATEWAY_RUNTIME_TICK_BOOTSTRAP_GRACE_MS,
        0,
        30_000,
    )
}

fn gateway_zone_owner_heartbeat_interval_ms() -> u64 {
    duration_from_millis_env(
        "MIR2_GATEWAY_ZONE_OWNER_HEARTBEAT_MS",
        DEFAULT_GATEWAY_ZONE_OWNER_HEARTBEAT_MS,
        100,
        60_000,
    )
    .as_millis()
    .min(u128::from(u64::MAX)) as u64
}

fn duration_from_millis_env(name: &str, default_ms: u64, min_ms: u64, max_ms: u64) -> Duration {
    Duration::from_millis(
        std::env::var(name)
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(default_ms)
            .clamp(min_ms, max_ms),
    )
}

fn runtime_tick_defer_duration_for_action(action: &SessionAction) -> Option<Duration> {
    match action {
        SessionAction::Packet(ClientPacket::StartGame { .. }) => {
            Some(gateway_runtime_tick_bootstrap_grace())
        }
        SessionAction::MoveTo { .. }
        | SessionAction::Packet(
            ClientPacket::Walk { .. } | ClientPacket::Run { .. } | ClientPacket::Turn { .. },
        ) => Some(Duration::ZERO),
        _ => None,
    }
}

fn positive_usize_env(name: &str) -> Option<usize> {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn refresh_external_session_state(session: &mut GatewaySession) -> Result<bool, String> {
    catch_gateway_panic("web refresh_active_external_mail", || {
        session.refresh_active_external_mail()
    })
}

fn login_account_id_for_action(action: &SessionAction) -> Option<&str> {
    match action {
        SessionAction::Packet(ClientPacket::Login { account_id, .. }) => Some(account_id),
        SessionAction::PasskeyLogin { account_id, .. } => Some(account_id),
        _ => None,
    }
}

fn start_game_character_index_for_action(action: &SessionAction) -> Option<i32> {
    match action {
        SessionAction::Packet(ClientPacket::StartGame { character_index }) => {
            Some(*character_index)
        }
        _ => None,
    }
}

fn keep_alive_time_for_action(action: &SessionAction) -> Option<i64> {
    match action {
        SessionAction::Packet(ClientPacket::KeepAlive { time }) => Some(*time),
        _ => None,
    }
}

fn try_acquire_start_game_route_lease(
    session_cache: &dyn crate::cache::GatewaySessionCache,
    session: &GatewaySession,
    authenticated: bool,
    authenticated_account_id: Option<&str>,
    character_index: Option<i32>,
) -> Result<Option<GatewaySessionCacheKey>, String> {
    let Some(character_index) = character_index else {
        return Ok(None);
    };
    if !authenticated || session.active_identity().is_some() {
        return Ok(None);
    }
    let account_id = authenticated_account_id
        .ok_or_else(|| "authenticated account is required before StartGame".to_string())?;
    let key = GatewaySessionCacheKey {
        account_id: account_id.to_string(),
        character_index,
    };
    match session_cache.acquire_route_lease(&key, session.session_id(), route_lease_ttl_seconds()) {
        Ok(_) => Ok(Some(key)),
        Err(error) => {
            eprintln!(
                "web StartGame route lease rejected for {}/{}: {error}",
                key.account_id, key.character_index
            );
            Err("character is already online or route lease is unavailable".to_string())
        }
    }
}

fn release_pending_start_game_route_lease(
    session_cache: &dyn crate::cache::GatewaySessionCache,
    session: &GatewaySession,
    key: Option<&GatewaySessionCacheKey>,
) {
    if let Some(key) = key {
        if let Err(error) = session_cache.release_route_lease(key, session.session_id()) {
            eprintln!(
                "web StartGame route lease release skipped for {}/{}: {error}",
                key.account_id, key.character_index
            );
        }
    }
}

fn release_unclaimed_start_game_route_lease(
    session_cache: &dyn crate::cache::GatewaySessionCache,
    session: &GatewaySession,
    key: Option<&GatewaySessionCacheKey>,
) {
    let Some(key) = key else {
        return;
    };
    let claimed = session.active_identity().is_some_and(|identity| {
        identity.account_id == key.account_id && identity.character_index == key.character_index
    });
    if !claimed {
        release_pending_start_game_route_lease(session_cache, session, Some(key));
    }
}

fn inflight_capacity_kind_for_action(action: &SessionAction) -> Option<GatewayCapacityKind> {
    match action {
        SessionAction::Packet(ClientPacket::Login { .. }) | SessionAction::PasskeyLogin { .. } => {
            Some(GatewayCapacityKind::Login)
        }
        SessionAction::Packet(ClientPacket::NewCharacter { .. }) => {
            Some(GatewayCapacityKind::NewCharacter)
        }
        SessionAction::Packet(ClientPacket::StartGame { .. }) => {
            Some(GatewayCapacityKind::StartGame)
        }
        _ => None,
    }
}

fn should_queue_save_for_action(action: &SessionAction) -> bool {
    !matches!(
        action,
        SessionAction::Packet(
            ClientPacket::ClientVersion { .. }
                | ClientPacket::Login { .. }
                | ClientPacket::KeepAlive { .. }
                | ClientPacket::Turn { .. }
                | ClientPacket::Walk { .. }
                | ClientPacket::Run { .. }
                | ClientPacket::Chat { .. }
        ) | SessionAction::PasskeyLogin { .. }
            | SessionAction::Tick
    )
}

fn execute_session_action(
    session: &mut GatewaySession,
    action: SessionAction,
    authenticated: bool,
    enforce_player_command_safety: bool,
) -> Result<Vec<ServerPacket>, String> {
    let move_log = move_log_for_action(&action);
    if enforce_player_command_safety {
        return execute_production_session_action(session, action, authenticated, move_log);
    }
    match action {
        SessionAction::Packet(packet) => {
            let responses = session.handle_packet(packet);
            log_move_action(move_log, &responses);
            Ok(responses)
        }
        SessionAction::PasskeyLogin { account_id, token } => {
            verify_passkey_gateway_token(&account_id, &token)?;
            Ok(session.passkey_login(&account_id))
        }
        SessionAction::MoveTo { x, y, running } => {
            let responses = session.move_to(x, y, running);
            log_move_action(move_log, &responses);
            Ok(responses)
        }
        SessionAction::Attack { object_id } => Ok(session.attack(object_id)),
        SessionAction::Interact { object_id } => Ok(session.interact(object_id)),
        SessionAction::SelectNpcDialog { target } => Ok(session.select_npc_dialog_target(&target)),
        SessionAction::SubmitNpcInput { value } => Ok(session.submit_npc_input(&value)),
        SessionAction::PickUp { object_id } => Ok(session.pick_up(object_id)),
        SessionAction::UseItem { key } => Ok(session.use_item(&key)),
        SessionAction::DropItem { key } => Ok(session.drop_item(&key)),
        SessionAction::CastSkill { key } => Ok(session.cast_skill(&key)),
        SessionAction::TransferMap { key } => Ok(session.transfer_map(&key)),
        SessionAction::Stage5Command { action, args } => Ok(session.stage5_command(&action, args)),
        SessionAction::SetLanguage { language } => session.set_language(&language).map(|_| vec![]),
        SessionAction::Tick => Ok(session.tick()),
    }
}

fn execute_production_session_action(
    session: &mut GatewaySession,
    action: SessionAction,
    authenticated: bool,
    move_log: Option<String>,
) -> Result<Vec<ServerPacket>, String> {
    let zone_owner_lease = session.zone_owner_lease().clone();
    let execution = match action {
        SessionAction::Packet(packet) => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::ClientPacket(packet),
            )?,
        SessionAction::PasskeyLogin { account_id, token } => {
            verify_passkey_gateway_token(&account_id, &token)?;
            return Ok(session.passkey_login(&account_id));
        }
        SessionAction::MoveTo { x, y, running } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::MoveTo {
                    position: Point { x, y },
                    running,
                },
            )?,
        SessionAction::Attack { object_id } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::Attack { object_id },
            )?,
        SessionAction::Interact { object_id } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::Interact { object_id },
            )?,
        SessionAction::SelectNpcDialog { target } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::SelectNpcDialog { target },
            )?,
        SessionAction::SubmitNpcInput { value } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::SubmitNpcInput { value },
            )?,
        SessionAction::PickUp { object_id } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::PickUp { object_id },
            )?,
        SessionAction::UseItem { key } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::UseItem { key },
            )?,
        SessionAction::DropItem { key } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::DropItem { key },
            )?,
        SessionAction::CastSkill { key } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::CastSkill { key },
            )?,
        SessionAction::TransferMap { key } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::TransferMap { key },
            )?,
        SessionAction::Stage5Command { action, args } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::Stage5Command { action, args },
            )?,
        SessionAction::SetLanguage { language } => session
            .execute_production_player_command_with_zone_owner_lease(
                &zone_owner_lease,
                authenticated,
                WorldCommand::SetLanguage { language },
            )?,
        SessionAction::Tick => session.execute_production_player_command_with_zone_owner_lease(
            &zone_owner_lease,
            authenticated,
            WorldCommand::Tick,
        )?,
    };
    log_move_action(move_log, &execution.packets);
    Ok(execution.packets)
}

fn update_authenticated_state(current: bool, responses: &[ServerPacket]) -> bool {
    if responses
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. }))
    {
        return true;
    }
    if responses.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::Login { .. }
                | ServerPacket::LoginBanned { .. }
                | ServerPacket::ReturnToLogin
        )
    }) {
        return false;
    }
    current
}

fn production_player_command_safety_enabled() -> bool {
    env_flag_enabled("MIR2_GATEWAY_ENFORCE_PLAYER_COMMAND_SAFETY")
        || ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV"]
            .into_iter()
            .filter_map(|name| env::var(name).ok())
            .any(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "production" | "prod" | "staging"
                )
            })
}

fn env_flag_enabled(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn move_log_for_action(action: &SessionAction) -> Option<String> {
    if !move_logging_enabled() {
        return None;
    }

    match action {
        SessionAction::MoveTo { x, y, running } => Some(format!(
            "MoveTo target=({x},{y}) mode={}",
            if *running { "run" } else { "walk" }
        )),
        SessionAction::Packet(ClientPacket::Walk { direction }) => {
            Some(format!("Walk direction={direction:?}"))
        }
        SessionAction::Packet(ClientPacket::Run { direction }) => {
            Some(format!("Run direction={direction:?}"))
        }
        SessionAction::Packet(ClientPacket::Turn { direction }) => {
            Some(format!("Turn direction={direction:?}"))
        }
        _ => None,
    }
}

fn move_logging_enabled() -> bool {
    env_flag_enabled("MIR2_GATEWAY_MOVE_LOG")
}

fn log_move_action(action: Option<String>, responses: &[ServerPacket]) {
    let Some(action) = action else {
        return;
    };

    let movement = responses.iter().find_map(|packet| match packet {
        ServerPacket::UserLocation { location } => Some(format!(
            "UserLocation=({}, {}) {:?}",
            location.position.x, location.position.y, location.direction
        )),
        ServerPacket::ObjectWalk { movement } => Some(format!(
            "ObjectWalk=({}, {}) {:?}",
            movement.position.x, movement.position.y, movement.direction
        )),
        ServerPacket::ObjectRun { movement } => Some(format!(
            "ObjectRun=({}, {}) {:?}",
            movement.position.x, movement.position.y, movement.direction
        )),
        _ => None,
    });

    eprintln!(
        "mir2-gateway movement {action} -> {} packets={} ",
        movement.unwrap_or_else(|| "no movement packet".to_string()),
        responses.len()
    );
}

fn browser_command_to_action(command: BrowserCommand) -> Result<SessionAction, String> {
    match command {
        BrowserCommand::ClientVersion => Ok(SessionAction::Packet(ClientPacket::ClientVersion {
            version_hash: Vec::new(),
        })),
        BrowserCommand::Disconnect => Ok(SessionAction::Packet(ClientPacket::Disconnect)),
        BrowserCommand::Login {
            account_id,
            password,
        } => Ok(SessionAction::Packet(ClientPacket::Login {
            account_id,
            password,
        })),
        BrowserCommand::PasskeyLogin { account_id, token } => {
            Ok(SessionAction::PasskeyLogin { account_id, token })
        }
        BrowserCommand::NewAccount {
            account_id,
            password,
            birth_date_binary,
            user_name,
            secret_question,
            secret_answer,
            email_address,
        } => Ok(SessionAction::Packet(ClientPacket::NewAccount {
            account_id,
            password,
            birth_date_binary,
            user_name,
            secret_question,
            secret_answer,
            email_address,
        })),
        BrowserCommand::ChangePassword {
            account_id,
            current_password,
            new_password,
        } => Ok(SessionAction::Packet(ClientPacket::ChangePassword {
            account_id,
            current_password,
            new_password,
        })),
        BrowserCommand::UnlockStorage { password } => {
            Ok(SessionAction::Packet(ClientPacket::UnlockStorage {
                password,
            }))
        }
        BrowserCommand::SetStoragePassword {
            current_password,
            new_password,
        } => Ok(SessionAction::Packet(ClientPacket::SetStoragePassword {
            current_password,
            new_password,
        })),
        BrowserCommand::RemoveStoragePassword { current_password } => {
            Ok(SessionAction::Packet(ClientPacket::RemoveStoragePassword {
                current_password,
            }))
        }
        BrowserCommand::NewCharacter {
            name,
            gender,
            class,
        } => Ok(SessionAction::Packet(ClientPacket::NewCharacter {
            name,
            gender: parse_gender(&gender)?,
            class: parse_class(&class)?,
        })),
        BrowserCommand::NewHero {
            name,
            gender,
            class,
        } => Ok(SessionAction::Packet(ClientPacket::NewHero {
            name,
            gender: parse_gender(&gender)?,
            class: parse_class(&class)?,
        })),
        BrowserCommand::StartGame { character_index } => {
            Ok(SessionAction::Packet(ClientPacket::StartGame {
                character_index,
            }))
        }
        BrowserCommand::Turn { direction } => Ok(SessionAction::Packet(ClientPacket::Turn {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Walk { direction } => Ok(SessionAction::Packet(ClientPacket::Walk {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Run { direction } => Ok(SessionAction::Packet(ClientPacket::Run {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Chat { message } => Ok(SessionAction::Packet(ClientPacket::Chat {
            message,
            linked_items: Vec::new(),
        })),
        BrowserCommand::KeepAlive { time } => {
            Ok(SessionAction::Packet(ClientPacket::KeepAlive { time }))
        }
        BrowserCommand::MoveTo { x, y, mode } => Ok(SessionAction::MoveTo {
            x,
            y,
            running: parse_move_mode(mode.as_deref())?,
        }),
        BrowserCommand::Attack { object_id } => Ok(SessionAction::Attack { object_id }),
        BrowserCommand::AttackDirection { direction, spell } => {
            Ok(SessionAction::Packet(ClientPacket::Attack {
                direction: parse_direction(&direction)?,
                spell: parse_spell(spell.unwrap_or(0))?,
            }))
        }
        BrowserCommand::RangeAttack {
            direction,
            x,
            y,
            target_id,
            target_x,
            target_y,
        } => Ok(SessionAction::Packet(ClientPacket::RangeAttack {
            direction: parse_direction(&direction)?,
            location: Point { x, y },
            target_id,
            target_location: Point {
                x: target_x,
                y: target_y,
            },
        })),
        BrowserCommand::Harvest { direction } => Ok(SessionAction::Packet(ClientPacket::Harvest {
            direction: parse_direction(&direction)?,
        })),
        BrowserCommand::Interact { object_id } => Ok(SessionAction::Interact { object_id }),
        BrowserCommand::SelectNpcDialog { target } => Ok(SessionAction::SelectNpcDialog { target }),
        BrowserCommand::SubmitNpcInput { value } => Ok(SessionAction::SubmitNpcInput { value }),
        BrowserCommand::PickUp { object_id } => Ok(SessionAction::PickUp { object_id }),
        BrowserCommand::PickUpTile => Ok(SessionAction::Packet(ClientPacket::PickUp)),
        BrowserCommand::UseItem {
            key,
            unique_id,
            slot,
            grid,
        } => {
            if let Some(unique_id) = unique_id {
                Ok(SessionAction::Packet(ClientPacket::UseItem {
                    unique_id,
                    grid: parse_grid(grid.as_deref().unwrap_or("inventory"))?,
                }))
            } else if let Some(slot) = slot {
                Ok(SessionAction::Packet(ClientPacket::UseItem {
                    unique_id: u64::from(slot),
                    grid: parse_grid(grid.as_deref().unwrap_or("inventory"))?,
                }))
            } else if let Some(key) = key {
                Ok(SessionAction::UseItem { key })
            } else {
                Err("useItem requires key, uniqueId, or slot".to_string())
            }
        }
        BrowserCommand::MoveItem { grid, from, to } => {
            Ok(SessionAction::Packet(ClientPacket::MoveItem {
                grid: parse_grid(&grid)?,
                from,
                to,
            }))
        }
        BrowserCommand::MergeItem {
            grid_from,
            grid_to,
            id_from,
            id_to,
        } => Ok(SessionAction::Packet(ClientPacket::MergeItem {
            grid_from: parse_grid(&grid_from)?,
            grid_to: parse_grid(&grid_to)?,
            id_from,
            id_to,
        })),
        BrowserCommand::EquipItem {
            unique_id,
            grid,
            to,
        } => Ok(SessionAction::Packet(ClientPacket::EquipItem {
            grid: parse_grid(&grid)?,
            unique_id,
            to,
        })),
        BrowserCommand::RemoveItem {
            unique_id,
            grid,
            to,
        } => Ok(SessionAction::Packet(ClientPacket::RemoveItem {
            grid: parse_grid(&grid)?,
            unique_id,
            to,
        })),
        BrowserCommand::RemoveSlotItem {
            unique_id,
            grid,
            grid_to,
            from_unique_id,
            to,
        } => Ok(SessionAction::Packet(ClientPacket::RemoveSlotItem {
            grid: parse_grid(&grid)?,
            grid_to: parse_grid(&grid_to)?,
            unique_id,
            to,
            from_unique_id,
        })),
        BrowserCommand::SplitItem {
            unique_id,
            grid,
            count,
        } => Ok(SessionAction::Packet(ClientPacket::SplitItem {
            grid: parse_grid(&grid)?,
            unique_id,
            count,
        })),
        BrowserCommand::StoreItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::StoreItem { from, to }))
        }
        BrowserCommand::TakeBackItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::TakeBackItem {
                from,
                to,
            }))
        }
        BrowserCommand::TakeBackHeroItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::TakeBackHeroItem {
                from,
                to,
            }))
        }
        BrowserCommand::TransferHeroItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::TransferHeroItem {
                from,
                to,
            }))
        }
        BrowserCommand::DropItem {
            key,
            unique_id,
            count,
            hero_inventory,
        } => {
            if let Some(unique_id) = unique_id {
                Ok(SessionAction::Packet(ClientPacket::DropItem {
                    unique_id,
                    count,
                    hero_inventory,
                }))
            } else {
                Ok(SessionAction::DropItem { key })
            }
        }
        BrowserCommand::DeleteItem {
            unique_id,
            count,
            hero_inventory,
        } => Ok(SessionAction::Packet(ClientPacket::DeleteItem {
            unique_id,
            count,
            hero_inventory,
        })),
        BrowserCommand::DropGold { amount } => {
            Ok(SessionAction::Packet(ClientPacket::DropGold { amount }))
        }
        BrowserCommand::RequestItemInfo { item_index } => {
            Ok(SessionAction::Packet(ClientPacket::RequestItemInfo {
                item_index,
            }))
        }
        BrowserCommand::SellItem { unique_id, count } => {
            Ok(SessionAction::Packet(ClientPacket::SellItem {
                unique_id,
                count,
            }))
        }
        BrowserCommand::BuyItem {
            item_index,
            count,
            panel_type,
        } => Ok(SessionAction::Packet(ClientPacket::BuyItem {
            item_index,
            count,
            panel_type,
        })),
        BrowserCommand::RepairItem { unique_id } => {
            Ok(SessionAction::Packet(ClientPacket::RepairItem {
                unique_id,
            }))
        }
        BrowserCommand::SpecialRepairItem { unique_id } => {
            Ok(SessionAction::Packet(ClientPacket::SRepairItem {
                unique_id,
            }))
        }
        BrowserCommand::DeleteCharacter { character_index } => {
            Ok(SessionAction::Packet(ClientPacket::DeleteCharacter {
                character_index,
            }))
        }
        BrowserCommand::MagicKey {
            spell,
            key,
            old_key,
        } => Ok(SessionAction::Packet(ClientPacket::MagicKey {
            spell: parse_spell_name(&spell)?,
            key,
            old_key,
        })),
        BrowserCommand::Magic {
            object_id,
            spell,
            direction,
            target_id,
            x,
            y,
            spell_target_lock,
        } => Ok(SessionAction::Packet(ClientPacket::Magic {
            object_id,
            spell: parse_spell_name(&spell)?,
            direction: parse_direction(&direction)?,
            target_id,
            location: Point { x, y },
            spell_target_lock,
        })),
        BrowserCommand::SpellToggle {
            spell,
            toggle_state,
            can_use,
        } => Ok(SessionAction::Packet(ClientPacket::SpellToggle {
            spell: parse_spell_name(&spell)?,
            toggle_state: toggle_state.unwrap_or_else(|| match can_use {
                Some(true) => 1,
                Some(false) => 0,
                None => -1,
            }),
        })),
        BrowserCommand::SetHeroBehaviour { behaviour } => {
            Ok(SessionAction::Packet(ClientPacket::SetHeroBehaviour {
                behaviour,
            }))
        }
        BrowserCommand::ChangeHero { list_index } => {
            Ok(SessionAction::Packet(ClientPacket::ChangeHero {
                list_index,
            }))
        }
        BrowserCommand::SetAutoPotValue { stat, value } => {
            Ok(SessionAction::Packet(ClientPacket::SetAutoPotValue {
                stat,
                value,
            }))
        }
        BrowserCommand::SetAutoPotItem { grid, item_index } => {
            Ok(SessionAction::Packet(ClientPacket::SetAutoPotItem {
                grid: parse_hero_autopot_grid(&grid)?,
                item_index,
            }))
        }
        BrowserCommand::ChangeAMode { mode } => {
            Ok(SessionAction::Packet(ClientPacket::ChangeAMode { mode }))
        }
        BrowserCommand::ChangePMode { mode } => {
            Ok(SessionAction::Packet(ClientPacket::ChangePMode { mode }))
        }
        BrowserCommand::OpenDoor { door_index } => {
            Ok(SessionAction::Packet(ClientPacket::OpenDoor { door_index }))
        }
        BrowserCommand::ConsignItem {
            unique_id,
            price,
            market_type,
        } => Ok(SessionAction::Packet(ClientPacket::ConsignItem {
            unique_id,
            price,
            market_type,
        })),
        BrowserCommand::MarketSearch {
            match_text,
            item_type,
            user_mode,
            min_shape,
            max_shape,
            market_type,
        } => Ok(SessionAction::Packet(ClientPacket::MarketSearch {
            match_text,
            item_type,
            user_mode,
            min_shape,
            max_shape,
            market_type,
        })),
        BrowserCommand::MarketRefresh => Ok(SessionAction::Packet(ClientPacket::MarketRefresh)),
        BrowserCommand::MarketPage { page } => {
            Ok(SessionAction::Packet(ClientPacket::MarketPage { page }))
        }
        BrowserCommand::MarketBuy {
            auction_id,
            bid_price,
        } => Ok(SessionAction::Packet(ClientPacket::MarketBuy {
            auction_id,
            bid_price,
        })),
        BrowserCommand::MarketGetBack { mode, auction_id } => {
            Ok(SessionAction::Packet(ClientPacket::MarketGetBack {
                mode,
                auction_id,
            }))
        }
        BrowserCommand::MarketSellNow { auction_id } => {
            Ok(SessionAction::Packet(ClientPacket::MarketSellNow {
                auction_id,
            }))
        }
        BrowserCommand::MarriageRequest => Ok(SessionAction::Packet(ClientPacket::MarriageRequest)),
        BrowserCommand::MarriageReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::MarriageReply {
                accept_invite,
            }))
        }
        BrowserCommand::ChangeMarriage => Ok(SessionAction::Packet(ClientPacket::ChangeMarriage)),
        BrowserCommand::DivorceRequest => Ok(SessionAction::Packet(ClientPacket::DivorceRequest)),
        BrowserCommand::DivorceReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::DivorceReply {
                accept_invite,
            }))
        }
        BrowserCommand::AddMentor { name } => {
            Ok(SessionAction::Packet(ClientPacket::AddMentor { name }))
        }
        BrowserCommand::MentorReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::MentorReply {
                accept_invite,
            }))
        }
        BrowserCommand::AllowMentor => Ok(SessionAction::Packet(ClientPacket::AllowMentor)),
        BrowserCommand::CancelMentor => Ok(SessionAction::Packet(ClientPacket::CancelMentor)),
        BrowserCommand::DepositTradeItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::DepositTradeItem {
                from,
                to,
            }))
        }
        BrowserCommand::RetrieveTradeItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::RetrieveTradeItem {
                from,
                to,
            }))
        }
        BrowserCommand::TradeRequest => Ok(SessionAction::Packet(ClientPacket::TradeRequest)),
        BrowserCommand::TradeReply { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::TradeReply {
                accept_invite,
            }))
        }
        BrowserCommand::TradeGold { amount } => {
            Ok(SessionAction::Packet(ClientPacket::TradeGold { amount }))
        }
        BrowserCommand::TradeConfirm { locked } => {
            Ok(SessionAction::Packet(ClientPacket::TradeConfirm { locked }))
        }
        BrowserCommand::TradeCancel => Ok(SessionAction::Packet(ClientPacket::TradeCancel)),
        BrowserCommand::FishingCast { cast_out } => {
            Ok(SessionAction::Packet(ClientPacket::FishingCast {
                cast_out,
            }))
        }
        BrowserCommand::FishingChangeAutocast { auto_cast } => {
            Ok(SessionAction::Packet(ClientPacket::FishingChangeAutocast {
                auto_cast,
            }))
        }
        BrowserCommand::SendMail {
            name,
            message,
            gold,
            items_idx,
            stamped,
        } => Ok(SessionAction::Packet(ClientPacket::SendMail {
            name,
            message,
            gold,
            items_idx,
            stamped,
        })),
        BrowserCommand::ReadMail { mail_id } => {
            Ok(SessionAction::Packet(ClientPacket::ReadMail { mail_id }))
        }
        BrowserCommand::CollectParcel { mail_id } => {
            Ok(SessionAction::Packet(ClientPacket::CollectParcel {
                mail_id,
            }))
        }
        BrowserCommand::DeleteMail { mail_id } => {
            Ok(SessionAction::Packet(ClientPacket::DeleteMail { mail_id }))
        }
        BrowserCommand::LockMail { mail_id, lock } => {
            Ok(SessionAction::Packet(ClientPacket::LockMail {
                mail_id,
                lock,
            }))
        }
        BrowserCommand::MailLockedItem { unique_id, locked } => {
            Ok(SessionAction::Packet(ClientPacket::MailLockedItem {
                unique_id,
                locked,
            }))
        }
        BrowserCommand::MailCost {
            gold,
            items_idx,
            stamped,
        } => Ok(SessionAction::Packet(ClientPacket::MailCost {
            gold,
            items_idx,
            stamped,
        })),
        BrowserCommand::UpdateIntelligentCreature {
            creature,
            summon_me,
            unsummon_me,
            release_me,
        } => Ok(SessionAction::Packet(
            ClientPacket::UpdateIntelligentCreature {
                creature,
                summon_me,
                unsummon_me,
                release_me,
            },
        )),
        BrowserCommand::IntelligentCreaturePickup {
            mouse_mode,
            location,
        } => Ok(SessionAction::Packet(
            ClientPacket::IntelligentCreaturePickup {
                mouse_mode,
                location,
            },
        )),
        BrowserCommand::RequestIntelligentCreatureUpdates { update } => Ok(SessionAction::Packet(
            ClientPacket::RequestIntelligentCreatureUpdates { update },
        )),
        BrowserCommand::AddFriend { name, blocked } => {
            Ok(SessionAction::Packet(ClientPacket::AddFriend {
                name,
                blocked,
            }))
        }
        BrowserCommand::RemoveFriend { character_index } => {
            Ok(SessionAction::Packet(ClientPacket::RemoveFriend {
                character_index,
            }))
        }
        BrowserCommand::RefreshFriends => Ok(SessionAction::Packet(ClientPacket::RefreshFriends)),
        BrowserCommand::AddMemo {
            character_index,
            memo,
        } => Ok(SessionAction::Packet(ClientPacket::AddMemo {
            character_index,
            memo,
        })),
        BrowserCommand::GetRanking {
            rank_type,
            rank_index,
            online_only,
        } => Ok(SessionAction::Packet(ClientPacket::GetRanking {
            rank_type,
            rank_index,
            online_only,
        })),
        BrowserCommand::GetRentedItems => Ok(SessionAction::Packet(ClientPacket::GetRentedItems)),
        BrowserCommand::ItemRentalRequest => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalRequest))
        }
        BrowserCommand::ItemRentalFee { amount } => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalFee {
                amount,
            }))
        }
        BrowserCommand::ItemRentalPeriod { days } => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalPeriod {
                days,
            }))
        }
        BrowserCommand::DepositRentalItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::DepositRentalItem {
                from,
                to,
            }))
        }
        BrowserCommand::RetrieveRentalItem { from, to } => {
            Ok(SessionAction::Packet(ClientPacket::RetrieveRentalItem {
                from,
                to,
            }))
        }
        BrowserCommand::CancelItemRental => {
            Ok(SessionAction::Packet(ClientPacket::CancelItemRental))
        }
        BrowserCommand::ItemRentalLockFee => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalLockFee))
        }
        BrowserCommand::ItemRentalLockItem => {
            Ok(SessionAction::Packet(ClientPacket::ItemRentalLockItem))
        }
        BrowserCommand::ConfirmItemRental => {
            Ok(SessionAction::Packet(ClientPacket::ConfirmItemRental))
        }
        BrowserCommand::AcceptQuest {
            npc_index,
            quest_index,
        } => Ok(SessionAction::Packet(ClientPacket::AcceptQuest {
            npc_index,
            quest_index,
        })),
        BrowserCommand::FinishQuest {
            quest_index,
            selected_item_index,
        } => Ok(SessionAction::Packet(ClientPacket::FinishQuest {
            quest_index,
            selected_item_index,
        })),
        BrowserCommand::AbandonQuest { quest_index } => {
            Ok(SessionAction::Packet(ClientPacket::AbandonQuest {
                quest_index,
            }))
        }
        BrowserCommand::ShareQuest { quest_index } => {
            Ok(SessionAction::Packet(ClientPacket::ShareQuest {
                quest_index,
            }))
        }
        BrowserCommand::SwitchGroup { allow_group } => {
            Ok(SessionAction::Packet(ClientPacket::SwitchGroup {
                allow_group,
            }))
        }
        BrowserCommand::AddMember { name } => {
            Ok(SessionAction::Packet(ClientPacket::AddMember { name }))
        }
        BrowserCommand::DelMember { name } => {
            Ok(SessionAction::Packet(ClientPacket::DelMember { name }))
        }
        BrowserCommand::GroupInvite { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::GroupInvite {
                accept_invite,
            }))
        }
        BrowserCommand::EditGuildMember {
            change_type,
            rank_index,
            name,
            rank_name,
        } => Ok(SessionAction::Packet(ClientPacket::EditGuildMember {
            change_type,
            rank_index,
            name,
            rank_name,
        })),
        BrowserCommand::EditGuildNotice { notice } => {
            Ok(SessionAction::Packet(ClientPacket::EditGuildNotice {
                notice,
            }))
        }
        BrowserCommand::GuildInvite { accept_invite } => {
            Ok(SessionAction::Packet(ClientPacket::GuildInvite {
                accept_invite,
            }))
        }
        BrowserCommand::GuildNameReturn { name } => {
            Ok(SessionAction::Packet(ClientPacket::GuildNameReturn {
                name,
            }))
        }
        BrowserCommand::RequestGuildInfo { info_type } => {
            Ok(SessionAction::Packet(ClientPacket::RequestGuildInfo {
                info_type,
            }))
        }
        BrowserCommand::GuildStorageGoldChange {
            change_type,
            amount,
        } => Ok(SessionAction::Packet(
            ClientPacket::GuildStorageGoldChange {
                change_type,
                amount,
            },
        )),
        BrowserCommand::GuildStorageItemChange {
            change_type,
            from,
            to,
        } => Ok(SessionAction::Packet(
            ClientPacket::GuildStorageItemChange {
                change_type,
                from,
                to,
            },
        )),
        BrowserCommand::CastSkill { key } => Ok(SessionAction::CastSkill { key }),
        BrowserCommand::TransferMap { key } => Ok(SessionAction::TransferMap { key }),
        BrowserCommand::Stage5Command { action, args } => {
            Ok(SessionAction::Stage5Command { action, args })
        }
        BrowserCommand::SetLanguage { language } => Ok(SessionAction::SetLanguage { language }),
        BrowserCommand::Tick => Ok(SessionAction::Tick),
        BrowserCommand::LogOut => Ok(SessionAction::Packet(ClientPacket::LogOut)),
    }
}

/// Decode the `MirGridType` for `C.SetAutoPotItem`. Crystal only ever sends
/// `HeroHPItem`/`HeroMPItem` for this packet (HeroDialogs auto-pot slots), so we
/// accept those two grids by name or by raw enum byte. `parse_grid` in
/// `browser_commands.rs` does not cover the hero auto-pot grids, hence this
/// dedicated decoder (mirrors `MirGridType` HeroHPItem = 23 / HeroMPItem = 24).
fn parse_hero_autopot_grid(value: &str) -> Result<MirGridType, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "herohpitem" | "hero_hp_item" | "hp" | "23" => Ok(MirGridType::HeroHpItem),
        "herompitem" | "hero_mp_item" | "mp" | "24" => Ok(MirGridType::HeroMpItem),
        other => Err(format!("unsupported hero auto-pot grid: {other}")),
    }
}

fn should_send_world_snapshot_for_action(action: &SessionAction) -> bool {
    !matches!(
        action,
        SessionAction::MoveTo { .. }
            | SessionAction::Packet(ClientPacket::Turn { .. })
            | SessionAction::Packet(ClientPacket::Walk { .. })
            | SessionAction::Packet(ClientPacket::Run { .. })
            | SessionAction::Packet(ClientPacket::Chat { .. })
            | SessionAction::Packet(ClientPacket::KeepAlive { .. })
            | SessionAction::Tick
    )
}

fn is_low_latency_action(action: &SessionAction) -> bool {
    matches!(
        action,
        SessionAction::Packet(
            ClientPacket::Turn { .. }
                | ClientPacket::Walk { .. }
                | ClientPacket::Run { .. }
                | ClientPacket::Chat { .. }
                | ClientPacket::KeepAlive { .. }
        ) | SessionAction::Tick
    )
}

fn responses_require_world_snapshot(responses: &[ServerPacket]) -> bool {
    responses.iter().any(|packet| {
        matches!(
            packet,
            ServerPacket::ObjectHide { .. } | ServerPacket::ObjectShow { .. }
        )
    })
}

async fn send_world_snapshot(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    session: &GatewaySession,
) -> Result<(), String> {
    let snapshot = catch_gateway_panic("web world_snapshot", || {
        tokio::task::block_in_place(|| session.world_snapshot())
    })?;
    sender
        .send(Message::Text(
            json!({
                "type": "worldSnapshot",
                "payload": snapshot
            })
            .to_string()
            .into(),
        ))
        .await
        .map_err(|error| error.to_string())
}

async fn send_server_packet(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    packet: &ServerPacket,
) -> Result<(), axum::Error> {
    sender
        .send(Message::Text(
            server_packet_to_event(packet).to_string().into(),
        ))
        .await
}

async fn send_error_message(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &str,
) -> Result<(), axum::Error> {
    sender
        .send(Message::Text(
            json!({
                "type": "error",
                "message": message
            })
            .to_string()
            .into(),
        ))
        .await
}

/// Renders a buff stat as a superset object: keeps Crystal's raw `stat`+`value`
/// (backward compatible) and adds `label` (Crystal `Stat` enum name) for the
/// browser buff window's `{label, value}` contract.
fn buff_stat_json(stat: &UserItemStat) -> Value {
    json!({
        "stat": stat.stat,
        "label": crystal_stat_label(stat.stat),
        "value": stat.value,
    })
}

/// Renders a non-blocked friend for the browser `friends` list. `level`/`mapName`
/// are intentionally absent (not carried by Crystal's `ClientFriend` nor known to
/// the simulation); an empty memo is omitted.
fn friend_entry_json(friend: &ClientFriend) -> Value {
    let mut entry = serde_json::Map::new();
    entry.insert("name".into(), json!(friend.name));
    entry.insert("online".into(), json!(friend.online));
    if !friend.memo.is_empty() {
        entry.insert("memo".into(), json!(friend.memo));
    }
    Value::Object(entry)
}

/// Renders a blocked entry for the browser `blocked` list (`{name, memo?}`).
fn blocked_entry_json(friend: &ClientFriend) -> Value {
    let mut entry = serde_json::Map::new();
    entry.insert("name".into(), json!(friend.name));
    if !friend.memo.is_empty() {
        entry.insert("memo".into(), json!(friend.memo));
    }
    Value::Object(entry)
}

/// Crystal `RequiredType.Level` (`Crystal/Shared/Enums.cs:1087-1089`). When an
/// item's `RequiredType` is `Level` the `RequiredAmount` is the level gate the
/// client renders (`Crystal/Client/MirScenes/Dialogs/NPCDialogs.cs:895-897`).
const CRYSTAL_REQUIRED_TYPE_LEVEL: u8 = 0;

/// Resolves a wire `UserItem` (which only carries `item_index`) into the
/// browser-facing `{ name, count?, grade? }` shape used by the trade / market /
/// quest contracts. The name + grade come from the Crystal item template
/// (`Crystal` mirrors `UserItem.Info.Name` / `UserItem.Info.Grade`), looked up
/// by `item_index`. When the index is unknown the name falls back to
/// `Item #<index>` so the window still renders a row.
fn user_item_summary_json(item: &UserItem) -> Value {
    let template = crystal_item_by_index(item.item_index);
    let name = template
        .as_ref()
        .map(|template| template.name.clone())
        .unwrap_or_else(|| format!("Item #{}", item.item_index));
    let mut entry = serde_json::Map::new();
    entry.insert("name".into(), json!(name));
    entry.insert("count".into(), json!(item.count));
    if let Some(template) = template.as_ref() {
        // Crystal `ItemGrade` (0 = Common). Only emit a meaningful grade.
        if template.grade != 0 {
            entry.insert("grade".into(), json!(template.grade));
        }
    }
    Value::Object(entry)
}

/// Maps a `Vec<Option<UserItem>>` trade payload into the contract item array,
/// skipping the empty trade slots (Crystal `TradeItem` sends a fixed-length
/// array with `null` holes — see `Crystal/Shared/ServerPackets.cs:1944-1972`).
fn trade_items_summary_json(items: &[Option<UserItem>]) -> Value {
    Value::Array(
        items
            .iter()
            .filter_map(|slot| slot.as_ref())
            .map(user_item_summary_json)
            .collect(),
    )
}

/// Resolves the optional `level` field for an auction/market listing: Crystal
/// only treats `RequiredAmount` as a level when `RequiredType == Level`
/// (`Crystal/Client/MirScenes/Dialogs/NPCDialogs.cs:895`).
fn crystal_item_level_requirement(item_index: i32) -> Option<u32> {
    let template = crystal_item_by_index(item_index)?;
    if template.required_type == CRYSTAL_REQUIRED_TYPE_LEVEL && template.required_amount > 0 {
        Some(u32::from(template.required_amount))
    } else {
        None
    }
}

/// Crystal `MarketItemType.Auction` (`Crystal/Shared/Enums.cs`). A `ClientAuction`
/// whose `ItemType` is `Auction` is shown as a live auction; `Consign`/`GameShop`
/// are fixed-price. Used to fill the contract `auction` flag.
const CRYSTAL_MARKET_ITEM_TYPE_AUCTION: u8 = 2;

/// Serializes a protocol `ClientAuction` into the browser-facing market listing
/// contract: `{ id, itemName, seller?, price, type?, level?, expiry?, auction?,
/// sold? }`. Mirrors `Crystal/Server/MirDatabase/AuctionInfo.cs:104-115`
/// (`CreateClientAuction`) — the wire struct collapses `CurrentBid`/`Expired`/
/// `Sold` into the `Seller` label + `Price`, so for own-listings the `Seller`
/// already encodes the status string ("Sold" / "Expired" / "For Sale" / ...).
/// `expiry` is the consignment `DateTime.ToBinary()` value Crystal ships
/// (`ClientAuction.ConsignmentDate`, `Crystal/Shared/Data/ClientData.cs:213`).
fn client_auction_listing_json(auction: &ClientAuction) -> Value {
    let item_index = auction.item.item_index;
    let item_name = crystal_item_by_index(item_index)
        .map(|template| template.name)
        .unwrap_or_else(|| format!("Item #{item_index}"));
    let mut entry = serde_json::Map::new();
    entry.insert("id".into(), json!(auction.auction_id));
    entry.insert("itemName".into(), json!(item_name));
    if !auction.seller.is_empty() {
        entry.insert("seller".into(), json!(auction.seller));
    }
    entry.insert("price".into(), json!(auction.price));
    entry.insert("type".into(), json!(auction.item_type));
    if let Some(level) = crystal_item_level_requirement(item_index) {
        entry.insert("level".into(), json!(level));
    }
    if auction.consignment_date_binary_datetime != 0 {
        entry.insert(
            "expiry".into(),
            json!(auction.consignment_date_binary_datetime),
        );
    }
    entry.insert(
        "auction".into(),
        json!(auction.item_type == CRYSTAL_MARKET_ITEM_TYPE_AUCTION),
    );
    Value::Object(entry)
}

/// Maps a list of protocol `ClientAuction`s into the contract listing array.
fn client_auction_listings_json(listings: &[ClientAuction]) -> Value {
    Value::Array(listings.iter().map(client_auction_listing_json).collect())
}

/// Parses a Crystal quest task line into a contract `objective`
/// (`{ text, current?, required? }`). The simulation formats task lines as e.g.
/// `"Kill ChickyBoo 2/5"` / `"Collect Wool 0/3"`
/// (`apps/simulation/src/runtime/quests.rs:318-356`), so the trailing
/// `current/required` fraction is parsed out when present; otherwise just the
/// text is kept.
fn quest_objective_json(line: &str) -> Value {
    let mut entry = serde_json::Map::new();
    entry.insert("text".into(), json!(line));
    if let Some((current, required)) = parse_progress_fraction(line) {
        entry.insert("current".into(), json!(current));
        entry.insert("required".into(), json!(required));
    }
    Value::Object(entry)
}

/// Extracts the last `current/required` integer fraction from a task line, if
/// one is present (e.g. `"Kill ChickyBoo 2/5"` -> `(2, 5)`).
fn parse_progress_fraction(line: &str) -> Option<(u32, u32)> {
    let token = line.split_whitespace().rev().find(|token| {
        token
            .split_once('/')
            .is_some_and(|(left, right)| !left.is_empty() && !right.is_empty())
    })?;
    let (left, right) = token.split_once('/')?;
    let current: u32 = left.parse().ok()?;
    let required: u32 = right.parse().ok()?;
    Some((current, required))
}

/// Serializes the reward bundle of a `ClientQuestInfo` into the contract
/// `rewards` object (`{ gold?, experience?, credit?, items? }`). Mirrors the
/// Crystal `ClientQuestInfo` reward fields
/// (`Crystal/Shared/Data/ClientData.cs:380-384`); fixed + selectable item
/// rewards are merged into one `items` list (each `{ name, count? }`) resolved
/// from the reward `ItemInfo.Name` (`Crystal/Shared/Data/SharedData.cs:75-93`).
/// Returns `None` when there is nothing to reward so the field can be omitted.
fn quest_rewards_json(info: &ClientQuestInfo) -> Option<Value> {
    let mut entry = serde_json::Map::new();
    if info.reward_gold != 0 {
        entry.insert("gold".into(), json!(info.reward_gold));
    }
    if info.reward_exp != 0 {
        entry.insert("experience".into(), json!(info.reward_exp));
    }
    if info.reward_credit != 0 {
        entry.insert("credit".into(), json!(info.reward_credit));
    }
    let items = quest_reward_items_json(
        info.rewards_fixed_item
            .iter()
            .chain(info.rewards_select_item.iter()),
    );
    if let Value::Array(ref array) = items {
        if !array.is_empty() {
            entry.insert("items".into(), items);
        }
    }
    if entry.is_empty() {
        None
    } else {
        Some(Value::Object(entry))
    }
}

/// Maps quest reward items into `[{ name, count? }]` using the reward item's
/// `ItemInfo.Name` (already carried on the wire, no lookup needed).
fn quest_reward_items_json<'a>(rewards: impl Iterator<Item = &'a QuestItemReward>) -> Value {
    Value::Array(
        rewards
            .map(|reward| {
                let mut item = serde_json::Map::new();
                item.insert("name".into(), json!(reward.item.name));
                if reward.count != 1 {
                    item.insert("count".into(), json!(reward.count));
                }
                Value::Object(item)
            })
            .collect(),
    )
}

/// Formats a quest time limit (Crystal `ClientQuestInfo.TimeLimitInSeconds`,
/// `Crystal/Shared/Data/ClientData.cs:378`) into a human `mm:ss` / `Hh Mm`
/// string for the contract `timeLimit` field. Returns `None` when the quest has
/// no time limit (0 seconds), so the field is omitted.
fn quest_time_limit_label(time_limit_in_seconds: i32) -> Option<String> {
    if time_limit_in_seconds <= 0 {
        return None;
    }
    let total = time_limit_in_seconds as u32;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        Some(format!("{hours}h {minutes:02}m"))
    } else {
        Some(format!("{minutes:02}:{seconds:02}"))
    }
}

fn server_packet_to_event(packet: &ServerPacket) -> Value {
    match packet {
        ServerPacket::Raw { packet_id, payload } => {
            let packet_name = server_packet_raw_display_name(*packet_id);
            json!({
                "type": "packet",
                "packet": packet_name,
                "payload": raw_payload_detail(
                    &packet_name,
                    *packet_id as i16,
                    payload
                )
            })
        }
        ServerPacket::TimeOfDay { lights } => json!({
            "type": "packet",
            "packet": "TimeOfDay",
            "payload": { "lights": lights }
        }),
        ServerPacket::ChangeAMode { mode } => json!({
            "type": "packet",
            "packet": "ChangeAMode",
            "payload": { "mode": mode }
        }),
        ServerPacket::ChangePMode { mode } => json!({
            "type": "packet",
            "packet": "ChangePMode",
            "payload": { "mode": mode }
        }),
        ServerPacket::BaseStatsInfo { stats } => json!({
            "type": "packet",
            "packet": "BaseStatsInfo",
            "payload": { "stats": stats }
        }),
        ServerPacket::HeroBaseStatsInfo { stats } => json!({
            "type": "packet",
            "packet": "HeroBaseStatsInfo",
            "payload": { "stats": stats }
        }),
        ServerPacket::HeroInformation { info } => json!({
            "type": "packet",
            "packet": "HeroInformation",
            "payload": { "info": info }
        }),
        // Crystal `GetMarket` enqueues `S.NPCMarket` with the matched
        // `ClientAuction`s, the page count, and `UserMode`
        // (`Crystal/Server/MirObjects/PlayerObject.cs:8419`). `auctions` resolves
        // each `ClientAuction` to the browser market-listing contract
        // (`{ id, itemName, seller?, price, type?, level?, expiry?, auction?,
        // sold? }`); the raw `listings` array is kept for backward compatibility.
        // `highestBid` / `sold` are omitted because the wire `ClientAuction`
        // collapses `CurrentBid`/`Sold` into `Price` + the `Seller` status label
        // (`Crystal/Server/MirDatabase/AuctionInfo.cs:104-115`), so they are not
        // separately recoverable here.
        ServerPacket::NPCMarket {
            listings,
            pages,
            user_mode,
        } => json!({
            "type": "packet",
            "packet": "NPCMarket",
            "payload": {
                "listings": listings,
                "auctions": client_auction_listings_json(listings),
                "pages": pages,
                "userMode": user_mode
            }
        }),
        ServerPacket::NPCMarketPage { listings } => json!({
            "type": "packet",
            "packet": "NPCMarketPage",
            "payload": {
                "listings": listings,
                "auctions": client_auction_listings_json(listings)
            }
        }),
        ServerPacket::Connected => json!({
            "type": "packet",
            "packet": "Connected",
            "payload": {}
        }),
        ServerPacket::ClientVersion { result } => json!({
            "type": "packet",
            "packet": "ClientVersion",
            "payload": { "result": result }
        }),
        ServerPacket::Disconnect { reason } => json!({
            "type": "packet",
            "packet": "Disconnect",
            "payload": { "reason": reason }
        }),
        ServerPacket::KeepAlive { time } => json!({
            "type": "packet",
            "packet": "KeepAlive",
            "payload": { "time": time }
        }),
        ServerPacket::NewAccount { result } => json!({
            "type": "packet",
            "packet": "NewAccount",
            "payload": { "result": result }
        }),
        ServerPacket::ChangePassword { result } => json!({
            "type": "packet",
            "packet": "ChangePassword",
            "payload": { "result": result }
        }),
        ServerPacket::ChangePasswordBanned {
            reason,
            expiry_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "ChangePasswordBanned",
            "payload": {
                "reason": reason,
                "expiryBinaryDatetime": expiry_binary_datetime
            }
        }),
        ServerPacket::Login { result } => json!({
            "type": "packet",
            "packet": "Login",
            "payload": { "result": result }
        }),
        ServerPacket::LoginBanned {
            reason,
            expiry_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "LoginBanned",
            "payload": {
                "reason": reason,
                "expiryBinaryDatetime": expiry_binary_datetime
            }
        }),
        ServerPacket::LoginSuccess { characters } => json!({
            "type": "packet",
            "packet": "LoginSuccess",
            "payload": {
                "characters": characters.iter().map(|character| {
                    json!({
                        "index": character.index,
                        "name": character.name,
                        "level": character.level,
                        "class": format!("{:?}", character.class),
                        "gender": format!("{:?}", character.gender)
                    })
                }).collect::<Vec<_>>()
            }
        }),
        ServerPacket::NewCharacter { result } => json!({
            "type": "packet",
            "packet": "NewCharacter",
            "payload": { "result": result }
        }),
        ServerPacket::NewHero { result } => json!({
            "type": "packet",
            "packet": "NewHero",
            "payload": { "result": result }
        }),
        ServerPacket::NewCharacterSuccess { char_info } => json!({
            "type": "packet",
            "packet": "NewCharacterSuccess",
            "payload": {
                "character": {
                    "index": char_info.index,
                    "name": char_info.name,
                    "level": char_info.level,
                    "class": format!("{:?}", char_info.class),
                    "gender": format!("{:?}", char_info.gender)
                }
            }
        }),
        ServerPacket::DeleteCharacter { result } => json!({
            "type": "packet",
            "packet": "DeleteCharacter",
            "payload": { "result": result }
        }),
        ServerPacket::DeleteCharacterSuccess { character_index } => json!({
            "type": "packet",
            "packet": "DeleteCharacterSuccess",
            "payload": { "characterIndex": character_index }
        }),
        ServerPacket::StartGame { result, resolution } => json!({
            "type": "packet",
            "packet": "StartGame",
            "payload": {
                "result": result,
                "resolution": resolution
            }
        }),
        ServerPacket::StartGameBanned {
            reason,
            expiry_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "StartGameBanned",
            "payload": {
                "reason": reason,
                "expiryBinaryDatetime": expiry_binary_datetime
            }
        }),
        ServerPacket::StartGameDelay { milliseconds } => json!({
            "type": "packet",
            "packet": "StartGameDelay",
            "payload": { "milliseconds": milliseconds }
        }),
        ServerPacket::MapInformation { info } => json!({
            "type": "packet",
            "packet": "MapInformation",
            "payload": {
                "mapIndex": info.map_index,
                "fileName": info.file_name,
                "title": info.title,
                "miniMapIndex": info.mini_map,
                "bigMapIndex": info.big_map,
                // Crystal SoundList music id for this map (0 = silent); the Web client loops it as
                // background music, matching MapControl.LoadMap -> SoundManager.PlayMusic(Music).
                "music": info.music,
                "spawnFlags": {
                    "lightning": info.has_lightning(),
                    "fire": info.has_fire()
                }
            }
        }),
        ServerPacket::UserInformation { info } => json!({
            "type": "packet",
            "packet": "UserInformation",
            "payload": {
                "objectId": info.object_id,
                "realId": info.real_id,
                "name": info.name,
                "guildName": info.guild_name,
                "guildRank": info.guild_rank,
                "nameColourArgb": info.name_colour_argb,
                "class": format!("{:?}", info.class),
                "gender": format!("{:?}", info.gender),
                "level": info.level,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "hair": info.hair,
                "hp": info.hp,
                "mp": info.mp,
                "maxHp": info.max_hp,
                "maxMp": info.max_mp,
                "experience": info.experience,
                "maxExperience": info.max_experience,
                "levelEffects": info.level_effects,
                "hasHero": info.has_hero,
                "heroBehaviour": info.hero_behaviour,
                "inventorySectionPresent": info.inventory_section_present,
                "equipmentSectionPresent": info.equipment_section_present,
                "questInventorySectionPresent": info.quest_inventory_section_present,
                "gold": info.gold,
                "credit": info.credit,
                "hasExpandedStorage": info.has_expanded_storage,
                "hasStoragePassword": info.has_storage_password,
                "requireStoragePassword": info.require_storage_password,
                "storagePasswordLastSetBinaryDatetime": info.storage_password_last_set_binary_datetime,
                "expandedStorageExpiryTimeBinaryDatetime": info.expanded_storage_expiry_time_binary_datetime,
                "magicCount": info.magic_count,
                "intelligentCreatureCount": info.intelligent_creature_count,
                "summonedCreatureType": info.summoned_creature_type,
                "creatureSummoned": info.creature_summoned,
                "allowObserve": info.allow_observe,
                "observer": info.observer
            }
        }),
        ServerPacket::UserLocation { location } => json!({
            "type": "packet",
            "packet": "UserLocation",
            "payload": {
                "x": location.position.x,
                "y": location.position.y,
                "direction": format!("{:?}", location.direction)
            }
        }),
        ServerPacket::ObjectPlayer { info } => json!({
            "type": "packet",
            "packet": "ObjectPlayer",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "guildName": info.guild_name,
                "guildRankName": info.guild_rank_name,
                "nameColourArgb": info.name_colour_argb,
                "class": format!("{:?}", info.class),
                "gender": format!("{:?}", info.gender),
                "level": info.level,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "hair": info.hair,
                "light": info.light,
                "weapon": info.weapon,
                "weaponEffect": info.weapon_effect,
                "armour": info.armour,
                "poison": info.poison,
                "dead": info.dead,
                "hidden": info.hidden,
                "effect": info.effect,
                "wingEffect": info.wing_effect,
                "extra": info.extra,
                "mountType": info.mount_type,
                "ridingMount": info.riding_mount,
                "fishing": info.fishing,
                "transformType": info.transform_type,
                "elementOrbEffect": info.element_orb_effect,
                "elementOrbLevel": info.element_orb_level,
                "elementOrbMax": info.element_orb_max,
                "buffs": info.buffs,
                "levelEffects": info.level_effects
            }
        }),
        ServerPacket::ObjectHero { info, owner_name } => json!({
            "type": "packet",
            "packet": "ObjectHero",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "ownerName": owner_name,
                "class": format!("{:?}", info.class),
                "gender": format!("{:?}", info.gender),
                "level": info.level,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "buffs": info.buffs,
                "levelEffects": info.level_effects
            }
        }),
        ServerPacket::ObjectRemove { object_id } => json!({
            "type": "packet",
            "packet": "ObjectRemove",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectTeleportOut {
            object_id,
            effect_type,
        } => json!({
            "type": "packet",
            "packet": "ObjectTeleportOut",
            "payload": {
                "objectId": object_id,
                "effectType": effect_type
            }
        }),
        ServerPacket::ObjectTeleportIn {
            object_id,
            effect_type,
        } => json!({
            "type": "packet",
            "packet": "ObjectTeleportIn",
            "payload": {
                "objectId": object_id,
                "effectType": effect_type
            }
        }),
        ServerPacket::TeleportIn => json!({
            "type": "packet",
            "packet": "TeleportIn",
            "payload": {}
        }),
        ServerPacket::NPCGoods {
            list,
            rate,
            panel_type,
            hide_added_stats,
        } => json!({
            "type": "packet",
            "packet": "NPCGoods",
            "payload": {
                "list": list,
                "rate": rate,
                "panelType": panel_type,
                "hideAddedStats": hide_added_stats
            }
        }),
        ServerPacket::NPCSell => json!({
            "type": "packet",
            "packet": "NPCSell",
            "payload": {}
        }),
        ServerPacket::NPCRepair { rate } => json!({
            "type": "packet",
            "packet": "NPCRepair",
            "payload": {
                "rate": rate
            }
        }),
        ServerPacket::NPCSRepair { rate } => json!({
            "type": "packet",
            "packet": "NPCSRepair",
            "payload": {
                "rate": rate
            }
        }),
        ServerPacket::NPCRefine { rate, refining } => json!({
            "type": "packet",
            "packet": "NPCRefine",
            "payload": {
                "rate": rate,
                "refining": refining
            }
        }),
        ServerPacket::NPCCheckRefine => json!({
            "type": "packet",
            "packet": "NPCCheckRefine",
            "payload": {}
        }),
        ServerPacket::NPCCollectRefine { success } => json!({
            "type": "packet",
            "packet": "NPCCollectRefine",
            "payload": {
                "success": success
            }
        }),
        ServerPacket::NPCReplaceWedRing { rate } => json!({
            "type": "packet",
            "packet": "NPCReplaceWedRing",
            "payload": {
                "rate": rate
            }
        }),
        ServerPacket::NPCStorage => json!({
            "type": "packet",
            "packet": "NPCStorage",
            "payload": {}
        }),
        ServerPacket::UserStorage { storage } => json!({
            "type": "packet",
            "packet": "UserStorage",
            "payload": {
                "storage": storage
            }
        }),
        ServerPacket::CombineItem {
            grid,
            id_from,
            id_to,
            success,
            destroy,
        } => json!({
            "type": "packet",
            "packet": "CombineItem",
            "payload": {
                "grid": grid,
                "idFrom": id_from,
                "idTo": id_to,
                "success": success,
                "destroy": destroy
            }
        }),
        ServerPacket::ItemUpgraded { item } => json!({
            "type": "packet",
            "packet": "ItemUpgraded",
            "payload": {
                "item": item
            }
        }),
        ServerPacket::ItemRepaired {
            unique_id,
            max_dura,
            current_dura,
        } => json!({
            "type": "packet",
            "packet": "ItemRepaired",
            "payload": {
                "uniqueId": unique_id,
                "maxDura": max_dura,
                "currentDura": current_dura
            }
        }),
        ServerPacket::ItemSlotSizeChanged {
            unique_id,
            slot_size,
        } => json!({
            "type": "packet",
            "packet": "ItemSlotSizeChanged",
            "payload": {
                "uniqueId": unique_id,
                "slotSize": slot_size
            }
        }),
        ServerPacket::ItemSealChanged {
            unique_id,
            expiry_date_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "ItemSealChanged",
            "payload": {
                "uniqueId": unique_id,
                "expiryDateBinaryDatetime": expiry_date_binary_datetime
            }
        }),
        ServerPacket::NewMagic { magic, hero } => json!({
            "type": "packet",
            "packet": "NewMagic",
            "payload": {
                "magic": magic,
                "hero": hero
            }
        }),
        ServerPacket::RemoveMagic { place_id } => json!({
            "type": "packet",
            "packet": "RemoveMagic",
            "payload": {
                "placeId": place_id
            }
        }),
        ServerPacket::MagicLeveled {
            object_id,
            spell,
            level,
            experience,
        } => json!({
            "type": "packet",
            "packet": "MagicLeveled",
            "payload": {
                "objectId": object_id,
                "spell": format!("{spell:?}"),
                "level": level,
                "experience": experience
            }
        }),
        ServerPacket::Magic {
            spell,
            target_id,
            target,
            cast,
            level,
            secondary_target_ids,
        } => json!({
            "type": "packet",
            "packet": "Magic",
            "payload": {
                "spell": format!("{spell:?}"),
                "targetId": target_id,
                "target": { "x": target.x, "y": target.y },
                "cast": cast,
                "level": level,
                "secondaryTargetIds": secondary_target_ids
            }
        }),
        ServerPacket::MagicDelay {
            object_id,
            spell,
            delay,
        } => json!({
            "type": "packet",
            "packet": "MagicDelay",
            "payload": {
                "objectId": object_id,
                "spell": format!("{spell:?}"),
                "delay": delay
            }
        }),
        ServerPacket::MagicCast { spell } => json!({
            "type": "packet",
            "packet": "MagicCast",
            "payload": {
                "spell": format!("{spell:?}")
            }
        }),
        ServerPacket::ObjectMagic {
            object_id,
            location,
            direction,
            spell,
            target_id,
            target,
            cast,
            level,
            self_broadcast,
            secondary_target_ids,
        } => json!({
            "type": "packet",
            "packet": "ObjectMagic",
            "payload": {
                "objectId": object_id,
                "location": { "x": location.x, "y": location.y },
                "direction": format!("{direction:?}"),
                "spell": format!("{spell:?}"),
                "targetId": target_id,
                "target": { "x": target.x, "y": target.y },
                "cast": cast,
                "level": level,
                "selfBroadcast": self_broadcast,
                "secondaryTargetIds": secondary_target_ids
            }
        }),
        ServerPacket::SpellToggle {
            object_id,
            spell,
            can_use,
        } => json!({
            "type": "packet",
            "packet": "SpellToggle",
            "payload": {
                "objectId": object_id,
                "spell": format!("{spell:?}"),
                "canUse": can_use
            }
        }),
        ServerPacket::SwitchGroup { allow_group } => json!({
            "type": "packet",
            "packet": "SwitchGroup",
            "payload": {
                "allowGroup": allow_group
            }
        }),
        ServerPacket::DeleteGroup => json!({
            "type": "packet",
            "packet": "DeleteGroup",
            "payload": {}
        }),
        ServerPacket::DeleteMember { name } => json!({
            "type": "packet",
            "packet": "DeleteMember",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::GroupInvite { name } => json!({
            "type": "packet",
            "packet": "GroupInvite",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::AddMember { name } => json!({
            "type": "packet",
            "packet": "AddMember",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::GroupMembersMap {
            player_name,
            player_map,
        } => json!({
            "type": "packet",
            "packet": "GroupMembersMap",
            "payload": {
                "playerName": player_name,
                "playerMap": player_map
            }
        }),
        // Custom roster snapshot (beyond Crystal): the full enriched party list so
        // the browser group window can render member level/class/HP/online in one
        // update. `members[0]` is the group leader (Crystal `GroupMembers[0]`,
        // e.g. PlayerObject.cs:2497); `leaderName` echoes it. Per-member fields the
        // simulation can't know (e.g. a remote player's HP) are omitted by the
        // `GroupMember` serde `skip_serializing_if` so consumers see only real data.
        ServerPacket::GroupMemberInfo {
            members,
            leader_name,
        } => json!({
            "type": "packet",
            "packet": "GroupMemberInfo",
            "payload": {
                "members": members,
                "leaderName": leader_name
            }
        }),
        ServerPacket::SendMemberLocation {
            member_name,
            member_location,
        } => json!({
            "type": "packet",
            "packet": "SendMemberLocation",
            "payload": {
                "memberName": member_name,
                "memberLocation": {
                    "x": member_location.x,
                    "y": member_location.y
                }
            }
        }),
        ServerPacket::SellItem {
            unique_id,
            count,
            success,
        } => json!({
            "type": "packet",
            "packet": "SellItem",
            "payload": {
                "uniqueId": unique_id,
                "count": count,
                "success": success
            }
        }),
        ServerPacket::RepairItem { unique_id } => json!({
            "type": "packet",
            "packet": "RepairItem",
            "payload": {
                "uniqueId": unique_id
            }
        }),
        ServerPacket::CraftItem { success } => json!({
            "type": "packet",
            "packet": "CraftItem",
            "payload": {
                "success": success
            }
        }),
        ServerPacket::ObjectTurn { movement } => json!({
            "type": "packet",
            "packet": "ObjectTurn",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectWalk { movement } => json!({
            "type": "packet",
            "packet": "ObjectWalk",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectRun { movement } => json!({
            "type": "packet",
            "packet": "ObjectRun",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectBackStep { movement, distance } => {
            let mut payload = movement_json(
                movement.object_id,
                movement.position.x,
                movement.position.y,
                movement.direction,
            );
            payload.insert("distance", json!(distance));
            json!({
                "type": "packet",
                "packet": "ObjectBackStep",
                "payload": payload
            })
        }
        ServerPacket::ObjectSitDown { movement, sitting } => {
            let mut payload = movement_json(
                movement.object_id,
                movement.position.x,
                movement.position.y,
                movement.direction,
            );
            payload.insert("sitting", json!(sitting));
            json!({
                "type": "packet",
                "packet": "ObjectSitDown",
                "payload": payload
            })
        }
        ServerPacket::ObjectHarvest { movement } => json!({
            "type": "packet",
            "packet": "ObjectHarvest",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::ObjectHarvested { movement } => json!({
            "type": "packet",
            "packet": "ObjectHarvested",
            "payload": movement_json(movement.object_id, movement.position.x, movement.position.y, movement.direction)
        }),
        ServerPacket::Chat { message, chat_type } => json!({
            "type": "packet",
            "packet": "Chat",
            "payload": {
                "message": message,
                "chatType": format!("{:?}", chat_type)
            }
        }),
        ServerPacket::ObjectChat {
            object_id,
            text,
            chat_type,
        } => json!({
            "type": "packet",
            "packet": "ObjectChat",
            "payload": {
                "objectId": object_id,
                "text": text,
                "chatType": format!("{:?}", chat_type)
            }
        }),
        ServerPacket::NewItemInfo { info } => json!({
            "type": "packet",
            "packet": "NewItemInfo",
            "payload": {
                "info": info
            }
        }),
        ServerPacket::NewHeroInfo {
            info,
            storage_index,
        } => json!({
            "type": "packet",
            "packet": "NewHeroInfo",
            "payload": {
                "info": info,
                "storageIndex": storage_index
            }
        }),
        ServerPacket::MoveItem {
            grid,
            from,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "MoveItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::EquipItem {
            grid,
            unique_id,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "EquipItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "uniqueId": unique_id,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::MergeItem {
            grid_from,
            grid_to,
            id_from,
            id_to,
            success,
        } => json!({
            "type": "packet",
            "packet": "MergeItem",
            "payload": {
                "gridFrom": format!("{:?}", grid_from),
                "gridTo": format!("{:?}", grid_to),
                "idFrom": id_from,
                "idTo": id_to,
                "success": success
            }
        }),
        ServerPacket::RemoveItem {
            grid,
            unique_id,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "RemoveItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "uniqueId": unique_id,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RemoveSlotItem {
            grid,
            grid_to,
            unique_id,
            to,
            success,
        } => json!({
            "type": "packet",
            "packet": "RemoveSlotItem",
            "payload": {
                "grid": format!("{:?}", grid),
                "gridTo": format!("{:?}", grid_to),
                "uniqueId": unique_id,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::DepositRefineItem { from, to, success } => json!({
            "type": "packet",
            "packet": "DepositRefineItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RetrieveRefineItem { from, to, success } => json!({
            "type": "packet",
            "packet": "RetrieveRefineItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RefineCancel => json!({
            "type": "packet",
            "packet": "RefineCancel",
            "payload": {}
        }),
        ServerPacket::RefineItem { unique_id } => json!({
            "type": "packet",
            "packet": "RefineItem",
            "payload": {
                "uniqueId": unique_id
            }
        }),
        ServerPacket::TakeBackItem { from, to, success } => json!({
            "type": "packet",
            "packet": "TakeBackItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::StoreItem { from, to, success } => json!({
            "type": "packet",
            "packet": "StoreItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::TakeBackHeroItem { from, to, success } => json!({
            "type": "packet",
            "packet": "TakeBackHeroItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::TransferHeroItem { from, to, success } => json!({
            "type": "packet",
            "packet": "TransferHeroItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::SplitItem { item, grid } => json!({
            "type": "packet",
            "packet": "SplitItem",
            "payload": {
                "item": item,
                "grid": format!("{:?}", grid)
            }
        }),
        ServerPacket::SplitItem1 {
            grid,
            unique_id,
            count,
            success,
        } => json!({
            "type": "packet",
            "packet": "SplitItem1",
            "payload": {
                "grid": format!("{:?}", grid),
                "uniqueId": unique_id,
                "count": count,
                "success": success
            }
        }),
        ServerPacket::UseItem {
            unique_id,
            success,
            grid,
        } => json!({
            "type": "packet",
            "packet": "UseItem",
            "payload": {
                "uniqueId": unique_id,
                "success": success,
                "grid": format!("{:?}", grid)
            }
        }),
        ServerPacket::DropItem {
            unique_id,
            count,
            hero_inventory,
            success,
        } => json!({
            "type": "packet",
            "packet": "DropItem",
            "payload": {
                "uniqueId": unique_id,
                "count": count,
                "heroInventory": hero_inventory,
                "success": success
            }
        }),
        ServerPacket::DeleteItem { unique_id, count } => json!({
            "type": "packet",
            "packet": "DeleteItem",
            "payload": {
                "uniqueId": unique_id,
                "count": count
            }
        }),
        ServerPacket::ObjectItem { info } => json!({
            "type": "packet",
            "packet": "ObjectItem",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "image": info.image,
                "grade": info.grade
            }
        }),
        ServerPacket::ObjectGold { info } => json!({
            "type": "packet",
            "packet": "ObjectGold",
            "payload": {
                "objectId": info.object_id,
                "gold": info.gold,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                }
            }
        }),
        ServerPacket::GainedItem { item } => json!({
            "type": "packet",
            "packet": "GainedItem",
            "payload": {
                "item": item
            }
        }),
        ServerPacket::GainedGold { gold } => json!({
            "type": "packet",
            "packet": "GainedGold",
            "payload": {
                "gold": gold
            }
        }),
        ServerPacket::LoseGold { gold } => json!({
            "type": "packet",
            "packet": "LoseGold",
            "payload": {
                "gold": gold
            }
        }),
        ServerPacket::GainedCredit { credit } => json!({
            "type": "packet",
            "packet": "GainedCredit",
            "payload": {
                "credit": credit
            }
        }),
        ServerPacket::LoseCredit { credit } => json!({
            "type": "packet",
            "packet": "LoseCredit",
            "payload": {
                "credit": credit
            }
        }),
        ServerPacket::NewMonsterInfo { info } => json!({
            "type": "packet",
            "packet": "NewMonsterInfo",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "image": info.image,
                "direction": format!("{:?}", info.direction),
                "effect": info.effect,
                "ai": info.ai,
                "light": info.light,
                "dead": info.dead,
                "skeleton": info.skeleton,
                "poison": info.poison,
                "hidden": info.hidden,
                "shockTime": info.shock_time,
                "bindingShotCenter": info.binding_shot_center,
                "extra": info.extra,
                "extraByte": info.extra_byte,
                "masterObjectId": info.master_object_id,
                "rarity": info.rarity,
                "buffs": info.buffs
            }
        }),
        ServerPacket::ObjectMonster { info } => json!({
            "type": "packet",
            "packet": "ObjectMonster",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "image": info.image,
                "direction": format!("{:?}", info.direction),
                "effect": info.effect,
                "ai": info.ai,
                "light": info.light,
                "dead": info.dead,
                "skeleton": info.skeleton,
                "poison": info.poison,
                "hidden": info.hidden,
                "shockTime": info.shock_time,
                "bindingShotCenter": info.binding_shot_center,
                "extra": info.extra,
                "extraByte": info.extra_byte,
                "masterObjectId": info.master_object_id,
                "rarity": info.rarity,
                "buffs": info.buffs
            }
        }),
        ServerPacket::ObjectAttack { info } => json!({
            "type": "packet",
            "packet": "ObjectAttack",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "spell": info.spell,
                "level": info.level,
                "attackType": info.attack_type
            }
        }),
        ServerPacket::Struck { info } => json!({
            "type": "packet",
            "packet": "Struck",
            "payload": {
                "attackerId": info.attacker_id
            }
        }),
        ServerPacket::ObjectStruck { info } => json!({
            "type": "packet",
            "packet": "ObjectStruck",
            "payload": {
                "objectId": info.object_id,
                "attackerId": info.attacker_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction)
            }
        }),
        ServerPacket::DuraChanged {
            unique_id,
            current_dura,
        } => json!({
            "type": "packet",
            "packet": "DuraChanged",
            "payload": {
                "uniqueId": unique_id,
                "currentDura": current_dura
            }
        }),
        ServerPacket::HeroHealthChanged { hp, mp } => json!({
            "type": "packet",
            "packet": "HeroHealthChanged",
            "payload": {
                "hp": hp,
                "mp": mp
            }
        }),
        ServerPacket::NewNpcInfo { info } => json!({
            "type": "packet",
            "packet": "NewNpcInfo",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "image": info.image,
                "colourArgb": info.colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "questIds": info.quest_ids
            }
        }),
        ServerPacket::ObjectNpc { info } => json!({
            "type": "packet",
            "packet": "ObjectNpc",
            "payload": {
                "objectId": info.object_id,
                "name": info.name,
                "nameColourArgb": info.name_colour_argb,
                "image": info.image,
                "colourArgb": info.colour_argb,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "questIds": info.quest_ids
            }
        }),
        ServerPacket::NPCResponse { page } => json!({
            "type": "packet",
            "packet": "NPCResponse",
            "payload": { "page": page }
        }),
        ServerPacket::ObjectDied { info } => json!({
            "type": "packet",
            "packet": "ObjectDied",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "kind": info.kind
            }
        }),
        ServerPacket::GainHeroExperience { amount } => json!({
            "type": "packet",
            "packet": "GainHeroExperience",
            "payload": {
                "amount": amount
            }
        }),
        ServerPacket::HeroLevelChanged {
            level,
            experience,
            max_experience,
        } => json!({
            "type": "packet",
            "packet": "HeroLevelChanged",
            "payload": {
                "level": level,
                "experience": experience,
                "maxExperience": max_experience
            }
        }),
        ServerPacket::ObjectHide { object_id } => json!({
            "type": "packet",
            "packet": "ObjectHide",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectShow { object_id } => json!({
            "type": "packet",
            "packet": "ObjectShow",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectRevived { info } => json!({
            "type": "packet",
            "packet": "ObjectRevived",
            "payload": {
                "objectId": info.object_id,
                "effect": info.effect
            }
        }),
        ServerPacket::ObjectEffect { info } => json!({
            "type": "packet",
            "packet": "ObjectEffect",
            "payload": {
                "objectId": info.object_id,
                "effect": info.effect,
                "effectType": info.effect_type,
                "delayTime": info.delay_time,
                "time": info.time
            }
        }),
        ServerPacket::ObjectHealth { info } => json!({
            "type": "packet",
            "packet": "ObjectHealth",
            "payload": {
                "objectId": info.object_id,
                "percent": info.percent,
                "expire": info.expire
            }
        }),
        ServerPacket::ObjectMana { info } => json!({
            "type": "packet",
            "packet": "ObjectMana",
            "payload": {
                "objectId": info.object_id,
                "percent": info.percent
            }
        }),
        ServerPacket::ObjectProjectile {
            spell,
            source_id,
            destination_id,
        } => json!({
            "type": "packet",
            "packet": "ObjectProjectile",
            "payload": {
                "spell": format!("{:?}", spell),
                "sourceId": source_id,
                "destinationId": destination_id
            }
        }),
        ServerPacket::RangeAttack {
            target_id,
            target,
            spell,
        } => json!({
            "type": "packet",
            "packet": "RangeAttack",
            "payload": {
                "targetId": target_id,
                "target": {"x": target.x, "y": target.y},
                "spell": format!("{:?}", spell)
            }
        }),
        ServerPacket::Pushed {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "Pushed",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::ObjectPushed {
            object_id,
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "ObjectPushed",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::MapEffect {
            location,
            effect,
            value,
        } => json!({
            "type": "packet",
            "packet": "MapEffect",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "effect": effect,
                "value": value
            }
        }),
        ServerPacket::AllowObserve { allow } => json!({
            "type": "packet",
            "packet": "AllowObserve",
            "payload": {
                "allow": allow
            }
        }),
        // Crystal `S.AddBuff` ships a `ClientBuff` (Type, Caster, Visible, ObjectID,
        // ExpireTime, Infinite, Paused, Stats, Values — Crystal/Shared/Data/
        // ClientData.cs:575). The browser contract additionally wants `type`,
        // `remainingMs`, and `stats` rendered as `{label, value}` (Crystal labels a
        // buff stat with `Stat.ToString()`, BuffDialog.cs:351). In the simulation
        // `ClientBuff.expire_time` is already milliseconds-from-now (it is built as
        // `(expires_at_tick - tick) * 1000`, runtime/buffs.rs:168), so it maps
        // straight to `remainingMs`. `caster` and `name` are omitted: the
        // simulation's buff packet does not carry them (Crystal's `Caster` is a
        // client-only field that is never serialised on the wire, ClientData.cs:595).
        // Existing keys are kept for backward compatibility.
        ServerPacket::AddBuff { buff } => json!({
            "type": "packet",
            "packet": "AddBuff",
            "payload": {
                "buffType": buff.buff_type,
                "type": buff.buff_type,
                "visible": buff.visible,
                "objectId": buff.object_id,
                "expireTime": buff.expire_time,
                "remainingMs": buff.expire_time,
                "infinite": buff.infinite,
                "paused": buff.paused,
                // Superset objects: keep `stat`+`value` (backward compatible) and add
                // `label` (Crystal `Stat` enum name) for the browser buff window.
                "stats": buff
                    .stats
                    .iter()
                    .map(buff_stat_json)
                    .collect::<Vec<_>>(),
                "values": buff.values
            }
        }),
        ServerPacket::RemoveBuff {
            buff_type,
            object_id,
        } => json!({
            "type": "packet",
            "packet": "RemoveBuff",
            "payload": {
                "buffType": buff_type,
                "objectId": object_id
            }
        }),
        ServerPacket::PauseBuff {
            buff_type,
            object_id,
            paused,
        } => json!({
            "type": "packet",
            "packet": "PauseBuff",
            "payload": {
                "buffType": buff_type,
                "objectId": object_id,
                "paused": paused
            }
        }),
        ServerPacket::ObjectHidden { object_id, hidden } => json!({
            "type": "packet",
            "packet": "ObjectHidden",
            "payload": {
                "objectId": object_id,
                "hidden": hidden
            }
        }),
        ServerPacket::ObjectRangeAttack { info } => json!({
            "type": "packet",
            "packet": "ObjectRangeAttack",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "direction": format!("{:?}", info.direction),
                "targetId": info.target_id,
                "target": {
                    "x": info.target.x,
                    "y": info.target.y
                },
                "attackType": info.attack_type,
                "spell": info.spell,
                "level": info.level
            }
        }),
        ServerPacket::UserDash {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "UserDash",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::ObjectDash {
            object_id,
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "ObjectDash",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::UserDashFail {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "UserDashFail",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::ObjectDashFail {
            object_id,
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "ObjectDashFail",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::UserDashAttack {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "UserDashAttack",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::ObjectDashAttack {
            object_id,
            location,
            direction,
            distance,
        } => json!({
            "type": "packet",
            "packet": "ObjectDashAttack",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction),
                "distance": distance
            }
        }),
        ServerPacket::UserAttackMove {
            location,
            direction,
        } => json!({
            "type": "packet",
            "packet": "UserAttackMove",
            "payload": {
                "location": {"x": location.x, "y": location.y},
                "direction": format!("{:?}", direction)
            }
        }),
        ServerPacket::RefreshItem { item } => json!({
            "type": "packet",
            "packet": "RefreshItem",
            "payload": {
                "item": item
            }
        }),
        ServerPacket::ObjectSpell { info } => json!({
            "type": "packet",
            "packet": "ObjectSpell",
            "payload": {
                "objectId": info.object_id,
                "location": {
                    "x": info.location.x,
                    "y": info.location.y
                },
                "spell": info.spell as u8,
                "direction": format!("{:?}", info.direction),
                "param": info.param
            }
        }),
        ServerPacket::ConsignItem { unique_id, success } => json!({
            "type": "packet",
            "packet": "ConsignItem",
            "payload": {
                "uniqueId": unique_id,
                "success": success
            }
        }),
        ServerPacket::MarketFail { reason } => json!({
            "type": "packet",
            "packet": "MarketFail",
            "payload": {
                "reason": reason
            }
        }),
        ServerPacket::MarketSuccess { message } => json!({
            "type": "packet",
            "packet": "MarketSuccess",
            "payload": {
                "message": message
            }
        }),
        ServerPacket::HeroCreateRequest { can_create_class } => json!({
            "type": "packet",
            "packet": "HeroCreateRequest",
            "payload": {
                "canCreateClass": can_create_class
            }
        }),
        ServerPacket::UpdateHeroSpawnState { state } => json!({
            "type": "packet",
            "packet": "UpdateHeroSpawnState",
            "payload": {
                "state": state
            }
        }),
        ServerPacket::UnlockHeroAutoPot => json!({
            "type": "packet",
            "packet": "UnlockHeroAutoPot",
            "payload": {}
        }),
        ServerPacket::SetHeroBehaviour { behaviour } => json!({
            "type": "packet",
            "packet": "SetHeroBehaviour",
            "payload": {
                "behaviour": behaviour
            }
        }),
        ServerPacket::ManageHeroes {
            maximum_count,
            current_hero,
            heroes,
        } => json!({
            "type": "packet",
            "packet": "ManageHeroes",
            "payload": {
                "maximumCount": maximum_count,
                "currentHero": current_hero,
                "heroes": heroes
            }
        }),
        ServerPacket::ChangeHero { from_index } => json!({
            "type": "packet",
            "packet": "ChangeHero",
            "payload": {
                "fromIndex": from_index
            }
        }),
        ServerPacket::DefaultNPC { object_id } => json!({
            "type": "packet",
            "packet": "DefaultNPC",
            "payload": { "objectId": object_id }
        }),
        ServerPacket::NPCUpdate { npc_id } => json!({
            "type": "packet",
            "packet": "NPCUpdate",
            "payload": { "npcId": npc_id }
        }),
        ServerPacket::MarriageRequest { name } => json!({
            "type": "packet",
            "packet": "MarriageRequest",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::DivorceRequest { name } => json!({
            "type": "packet",
            "packet": "DivorceRequest",
            "payload": {
                "name": name
            }
        }),
        ServerPacket::MentorRequest { name, level } => json!({
            "type": "packet",
            "packet": "MentorRequest",
            "payload": {
                "name": name,
                "level": level
            }
        }),
        ServerPacket::DepositTradeItem { from, to, success } => json!({
            "type": "packet",
            "packet": "DepositTradeItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RetrieveTradeItem { from, to, success } => json!({
            "type": "packet",
            "packet": "RetrieveTradeItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        // Crystal sends `TradeRequest`/`TradeAccept` to the *partner* carrying the
        // other player's name (`Crystal/Server/MirObjects/PlayerObject.cs:10705,
        // 10741-10742`), which the client stores as the guest/partner name
        // (`Crystal/Client/MirScenes/GameScene.cs:6314`). `partnerName` is the
        // contract key; `name` is kept for backward compatibility.
        ServerPacket::TradeRequest { name } => json!({
            "type": "packet",
            "packet": "TradeRequest",
            "payload": {
                "name": name,
                "partnerName": name
            }
        }),
        ServerPacket::TradeAccept { name } => json!({
            "type": "packet",
            "packet": "TradeAccept",
            "payload": {
                "name": name,
                "partnerName": name
            }
        }),
        // `TradeGold` is enqueued to the partner with the *partner's* running gold
        // offer (`Crystal/Server/MirObjects/PlayerObject.cs:10759`); the client
        // stores it as `GuestTradeDialog.GuestGold`
        // (`Crystal/Client/MirScenes/GameScene.cs:6319`). `partnerGold` is the
        // contract key; `amount` is kept for backward compatibility.
        ServerPacket::TradeGold { amount } => json!({
            "type": "packet",
            "packet": "TradeGold",
            "payload": {
                "amount": amount,
                "partnerGold": amount
            }
        }),
        // `TradeItem` is enqueued to the partner with the *partner's* offered items
        // (`Crystal/Server/MirObjects/PlayerObject.cs:10776`); the client stores it
        // as `GuestTradeDialog.GuestItems`
        // (`Crystal/Client/MirScenes/GameScene.cs:6325`). `partnerItems` resolves
        // each `UserItem.item_index` to `{ name, count?, grade? }`; the raw
        // `tradeItems` array is kept for backward compatibility. `myItems` /
        // `myGold` / `myLocked` / `partnerLocked` are intentionally omitted: the
        // Crystal trade protocol only pushes the partner's side to each client
        // (own offer + lock state are tracked client-side), so the simulation
        // cannot know them here.
        ServerPacket::TradeItem { trade_items } => json!({
            "type": "packet",
            "packet": "TradeItem",
            "payload": {
                "tradeItems": trade_items,
                "partnerItems": trade_items_summary_json(trade_items)
            }
        }),
        ServerPacket::TradeConfirm => json!({
            "type": "packet",
            "packet": "TradeConfirm",
            "payload": {}
        }),
        ServerPacket::TradeCancel { unlock } => json!({
            "type": "packet",
            "packet": "TradeCancel",
            "payload": {
                "unlock": unlock
            }
        }),
        ServerPacket::MountUpdate {
            object_id,
            mount_type,
            riding_mount,
        } => json!({
            "type": "packet",
            "packet": "MountUpdate",
            "payload": {
                "objectId": object_id,
                "mountType": mount_type,
                "ridingMount": riding_mount
            }
        }),
        ServerPacket::FishingUpdate {
            object_id,
            fishing,
            progress_percent,
            chance_percent,
            fishing_point,
            found_fish,
        } => json!({
            "type": "packet",
            "packet": "FishingUpdate",
            "payload": {
                "objectId": object_id,
                "fishing": fishing,
                "progressPercent": progress_percent,
                "chancePercent": chance_percent,
                "fishingPoint": {
                    "x": fishing_point.x,
                    "y": fishing_point.y
                },
                "foundFish": found_fish
            }
        }),
        ServerPacket::RemoveDelayedExplosion { object_id } => json!({
            "type": "packet",
            "packet": "RemoveDelayedExplosion",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::ObjectDeco {
            object_id,
            location,
            image,
        } => json!({
            "type": "packet",
            "packet": "ObjectDeco",
            "payload": {
                "objectId": object_id,
                "location": {"x": location.x, "y": location.y},
                "image": image
            }
        }),
        ServerPacket::ObjectSneaking {
            object_id,
            sneaking_active,
        } => json!({
            "type": "packet",
            "packet": "ObjectSneaking",
            "payload": {
                "objectId": object_id,
                "sneakingActive": sneaking_active
            }
        }),
        ServerPacket::ObjectLevelEffects {
            object_id,
            level_effects,
        } => json!({
            "type": "packet",
            "packet": "ObjectLevelEffects",
            "payload": {
                "objectId": object_id,
                "levelEffects": level_effects
            }
        }),
        ServerPacket::SetBindingShot {
            object_id,
            enabled,
            value,
        } => json!({
            "type": "packet",
            "packet": "SetBindingShot",
            "payload": {
                "objectId": object_id,
                "enabled": enabled,
                "value": value
            }
        }),
        ServerPacket::SendOutputMessage {
            message,
            output_type,
        } => json!({
            "type": "packet",
            "packet": "SendOutputMessage",
            "payload": {
                "message": message,
                "outputType": output_type
            }
        }),
        ServerPacket::OpenDoor { door_index, close } => json!({
            "type": "packet",
            "packet": "Opendoor",
            "payload": {
                "doorIndex": door_index,
                "close": close
            }
        }),
        ServerPacket::OpenBrowser { url } => json!({
            "type": "packet",
            "packet": "OpenBrowser",
            "payload": {
                "url": url
            }
        }),
        ServerPacket::PlaySound { sound } => json!({
            "type": "packet",
            "packet": "PlaySound",
            "payload": {
                "sound": sound
            }
        }),
        ServerPacket::SetTimer {
            key,
            timer_type,
            seconds,
        } => json!({
            "type": "packet",
            "packet": "SetTimer",
            "payload": {
                "key": key,
                "timerType": timer_type,
                "seconds": seconds
            }
        }),
        ServerPacket::ExpireTimer { key } => json!({
            "type": "packet",
            "packet": "ExpireTimer",
            "payload": {
                "key": key
            }
        }),
        ServerPacket::Roll {
            roll_type,
            page,
            result,
            auto_roll,
        } => json!({
            "type": "packet",
            "packet": "Roll",
            "payload": {
                "rollType": roll_type,
                "page": page,
                "result": result,
                "autoRoll": auto_roll
            }
        }),
        ServerPacket::SetCompass { location } => json!({
            "type": "packet",
            "packet": "SetCompass",
            "payload": {
                "location": {
                    "x": location.x,
                    "y": location.y
                }
            }
        }),
        ServerPacket::NPCAwakening => json!({
            "type": "packet",
            "packet": "NPCAwakening",
            "payload": {}
        }),
        ServerPacket::NPCDisassemble => json!({
            "type": "packet",
            "packet": "NPCDisassemble",
            "payload": {}
        }),
        ServerPacket::NPCDowngrade => json!({
            "type": "packet",
            "packet": "NPCDowngrade",
            "payload": {}
        }),
        ServerPacket::NPCReset => json!({
            "type": "packet",
            "packet": "NPCReset",
            "payload": {}
        }),
        ServerPacket::AwakeningLockedItem { unique_id, locked } => json!({
            "type": "packet",
            "packet": "AwakeningLockedItem",
            "payload": {
                "uniqueId": unique_id,
                "locked": locked
            }
        }),
        ServerPacket::Awakening { result, remove_id } => json!({
            "type": "packet",
            "packet": "Awakening",
            "payload": {
                "result": result,
                "removeId": remove_id
            }
        }),
        ServerPacket::ReceiveMail { mail } => json!({
            "type": "packet",
            "packet": "ReceiveMail",
            "payload": {
                "mail": mail
            }
        }),
        ServerPacket::MailLockedItem { unique_id, locked } => json!({
            "type": "packet",
            "packet": "MailLockedItem",
            "payload": {
                "uniqueId": unique_id,
                "locked": locked
            }
        }),
        ServerPacket::MailSendRequest => json!({
            "type": "packet",
            "packet": "MailSendRequest",
            "payload": {}
        }),
        ServerPacket::MailSent { result } => json!({
            "type": "packet",
            "packet": "MailSent",
            "payload": {
                "result": result
            }
        }),
        ServerPacket::ParcelCollected { result } => json!({
            "type": "packet",
            "packet": "ParcelCollected",
            "payload": {
                "result": result
            }
        }),
        ServerPacket::MailCost { cost } => json!({
            "type": "packet",
            "packet": "MailCost",
            "payload": {
                "cost": cost
            }
        }),
        // Crystal `S.FriendUpdate` carries one flat `List<ClientFriend>` where each
        // entry has Name/Memo/Blocked/Online (Crystal/Shared/Data/ClientData.cs:122,
        // populated by CharacterInfo.CreateClientFriend, CharacterInfo.cs:759). The
        // browser contract wants the roster pre-split into a non-blocked `friends`
        // list and a `blocked` list, with friend objects carrying name/online/memo.
        // `level`/`mapName` are omitted: neither Crystal's `ClientFriend` nor the
        // simulation's social state tracks them, so they are genuinely unknown here.
        ServerPacket::FriendUpdate { friends } => json!({
            "type": "packet",
            "packet": "FriendUpdate",
            "payload": {
                "friends": friends
                    .iter()
                    .filter(|friend| !friend.blocked)
                    .map(friend_entry_json)
                    .collect::<Vec<_>>(),
                "blocked": friends
                    .iter()
                    .filter(|friend| friend.blocked)
                    .map(blocked_entry_json)
                    .collect::<Vec<_>>()
            }
        }),
        ServerPacket::LoverUpdate {
            name,
            date_binary_datetime,
            map_name,
            married_days,
        } => json!({
            "type": "packet",
            "packet": "LoverUpdate",
            "payload": {
                "name": name,
                "dateBinaryDatetime": date_binary_datetime,
                "mapName": map_name,
                "marriedDays": married_days
            }
        }),
        ServerPacket::MentorUpdate {
            name,
            level,
            online,
            mentee_exp,
        } => json!({
            "type": "packet",
            "packet": "MentorUpdate",
            "payload": {
                "name": name,
                "level": level,
                "online": online,
                "menteeExp": mentee_exp
            }
        }),
        ServerPacket::GuildBuffList {
            remove,
            active_buffs,
            guild_buffs,
        } => json!({
            "type": "packet",
            "packet": "GuildBuffList",
            "payload": {
                "remove": remove,
                "activeBuffs": active_buffs,
                "guildBuffs": guild_buffs
            }
        }),
        ServerPacket::GameShopInfo { item, stock_level } => json!({
            "type": "packet",
            "packet": "GameShopInfo",
            "payload": {
                "item": item,
                "stockLevel": stock_level
            }
        }),
        ServerPacket::NewIntelligentCreature { creature } => json!({
            "type": "packet",
            "packet": "NewIntelligentCreature",
            "payload": {
                "creature": creature
            }
        }),
        ServerPacket::UpdateIntelligentCreatureList {
            creature_list,
            creature_summoned,
            summoned_creature_type,
            pearl_count,
        } => json!({
            "type": "packet",
            "packet": "UpdateIntelligentCreatureList",
            "payload": {
                "creatureList": creature_list,
                "creatureSummoned": creature_summoned,
                "summonedCreatureType": summoned_creature_type,
                "pearlCount": pearl_count
            }
        }),
        ServerPacket::IntelligentCreatureEnableRename => json!({
            "type": "packet",
            "packet": "IntelligentCreatureEnableRename",
            "payload": {}
        }),
        ServerPacket::IntelligentCreaturePickup { object_id } => json!({
            "type": "packet",
            "packet": "IntelligentCreaturePickup",
            "payload": {
                "objectId": object_id
            }
        }),
        ServerPacket::GetRentedItems { rented_items } => json!({
            "type": "packet",
            "packet": "GetRentedItems",
            "payload": {
                "rentedItems": rented_items
            }
        }),
        ServerPacket::ItemRentalRequest { name, renting } => json!({
            "type": "packet",
            "packet": "ItemRentalRequest",
            "payload": {
                "name": name,
                "renting": renting
            }
        }),
        ServerPacket::ItemRentalFee { amount } => json!({
            "type": "packet",
            "packet": "ItemRentalFee",
            "payload": {
                "amount": amount
            }
        }),
        ServerPacket::ItemRentalPeriod { days } => json!({
            "type": "packet",
            "packet": "ItemRentalPeriod",
            "payload": {
                "days": days
            }
        }),
        ServerPacket::DepositRentalItem { from, to, success } => json!({
            "type": "packet",
            "packet": "DepositRentalItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::RetrieveRentalItem { from, to, success } => json!({
            "type": "packet",
            "packet": "RetrieveRentalItem",
            "payload": {
                "from": from,
                "to": to,
                "success": success
            }
        }),
        ServerPacket::UpdateRentalItem { loan_item } => json!({
            "type": "packet",
            "packet": "UpdateRentalItem",
            "payload": {
                "loanItem": loan_item
            }
        }),
        ServerPacket::CancelItemRental => json!({
            "type": "packet",
            "packet": "CancelItemRental",
            "payload": {}
        }),
        ServerPacket::ItemRentalLock {
            success,
            gold_locked,
            item_locked,
        } => json!({
            "type": "packet",
            "packet": "ItemRentalLock",
            "payload": {
                "success": success,
                "goldLocked": gold_locked,
                "itemLocked": item_locked
            }
        }),
        ServerPacket::ItemRentalPartnerLock {
            gold_locked,
            item_locked,
        } => json!({
            "type": "packet",
            "packet": "ItemRentalPartnerLock",
            "payload": {
                "goldLocked": gold_locked,
                "itemLocked": item_locked
            }
        }),
        ServerPacket::CanConfirmItemRental => json!({
            "type": "packet",
            "packet": "CanConfirmItemRental",
            "payload": {}
        }),
        ServerPacket::ConfirmItemRental => json!({
            "type": "packet",
            "packet": "ConfirmItemRental",
            "payload": {}
        }),
        // Crystal `ChangeQuest` carries the dynamic `ClientQuestProgress`
        // (`Id` + `TaskList` + `Taken`/`Completed`/`New`) plus the `QuestState`
        // enum (Add=0/Update=1/Remove=2) and `TrackQuest`
        // (`Crystal/Shared/ServerPackets.cs:5214-5237`,
        // `Crystal/Shared/Data/ClientData.cs:524-573`). The static name /
        // description / rewards / npc / timeLimit live in `NewQuestInfo`, so here
        // the contract adds only the dynamic fields: `id`, `state` (the
        // `QuestState` byte), `objectives` (parsed from the `TaskList` progress
        // lines), and `descriptionLines` (the raw task lines for a fallback view).
        ServerPacket::ChangeQuest {
            quest_id,
            task_list,
            taken,
            completed,
            new,
            quest_state,
            track_quest,
        } => json!({
            "type": "packet",
            "packet": "ChangeQuest",
            "payload": {
                "questId": quest_id,
                "id": quest_id,
                "taskList": task_list,
                "state": quest_state,
                "descriptionLines": task_list,
                "objectives": Value::Array(
                    task_list.iter().map(|line| quest_objective_json(line)).collect()
                ),
                "taken": taken,
                "completed": completed,
                "new": new,
                "questState": quest_state,
                "trackQuest": track_quest
            }
        }),
        ServerPacket::CompleteQuest { completed_quests } => json!({
            "type": "packet",
            "packet": "CompleteQuest",
            "payload": {
                "completedQuests": completed_quests
            }
        }),
        ServerPacket::ShareQuest {
            quest_index,
            sharer_name,
        } => json!({
            "type": "packet",
            "packet": "ShareQuest",
            "payload": {
                "questIndex": quest_index,
                "sharerName": sharer_name
            }
        }),
        // Crystal `NewQuestInfo` carries the static `ClientQuestInfo`
        // (`Crystal/Shared/ServerPackets.cs:5285-5302`,
        // `Crystal/Shared/Data/ClientData.cs:360-440`). The raw `info` object is
        // kept for backward compatibility; the contract-shaped quest fields are
        // hoisted alongside it: `id`, `name`, `descriptionLines` (the quest
        // `Description`), `objectives` (the `TaskDescription` lines), `rewards`
        // (gold/exp/credit/items) and `timeLimit`. `npc` (a *name*) is omitted:
        // `ClientQuestInfo` only carries `NPCIndex` (a numeric index the Crystal
        // client resolves against its own NPC list), so the simulation cannot
        // supply an NPC name from this packet alone.
        ServerPacket::NewQuestInfo { info } => {
            let mut payload = serde_json::Map::new();
            payload.insert("info".into(), json!(info));
            payload.insert("id".into(), json!(info.index));
            payload.insert("name".into(), json!(info.name));
            if !info.group.is_empty() {
                payload.insert("group".into(), json!(info.group));
            }
            if !info.description.is_empty() {
                payload.insert("descriptionLines".into(), json!(info.description));
            }
            if !info.task_description.is_empty() {
                payload.insert(
                    "objectives".into(),
                    Value::Array(
                        info.task_description
                            .iter()
                            .map(|line| quest_objective_json(line))
                            .collect(),
                    ),
                );
            }
            if let Some(rewards) = quest_rewards_json(info) {
                payload.insert("rewards".into(), rewards);
            }
            if let Some(time_limit) = quest_time_limit_label(info.time_limit_in_seconds) {
                payload.insert("timeLimit".into(), json!(time_limit));
            }
            json!({
                "type": "packet",
                "packet": "NewQuestInfo",
                "payload": Value::Object(payload)
            })
        }
        ServerPacket::NewRecipeInfo { info } => json!({
            "type": "packet",
            "packet": "NewRecipeInfo",
            "payload": { "info": info }
        }),
        ServerPacket::ResizeInventory { size } => json!({
            "type": "packet",
            "packet": "ResizeInventory",
            "payload": {
                "size": size
            }
        }),
        ServerPacket::ResizeStorage {
            size,
            has_expanded_storage,
            expiry_time_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "ResizeStorage",
            "payload": {
                "size": size,
                "hasExpandedStorage": has_expanded_storage,
                "expiryTimeBinaryDatetime": expiry_time_binary_datetime
            }
        }),
        ServerPacket::StorageUnlockResult {
            result,
            has_password,
        } => json!({
            "type": "packet",
            "packet": "StorageUnlockResult",
            "payload": {
                "result": result,
                "hasPassword": has_password
            }
        }),
        ServerPacket::StoragePasswordResult {
            result,
            removing,
            has_password,
            last_set_binary_datetime,
        } => json!({
            "type": "packet",
            "packet": "StoragePasswordResult",
            "payload": {
                "result": result,
                "removing": removing,
                "hasPassword": has_password,
                "lastSetBinaryDatetime": last_set_binary_datetime
            }
        }),
        ServerPacket::LogOutSuccess { characters } => json!({
            "type": "packet",
            "packet": "LogOutSuccess",
            "payload": {
                "characters": characters.iter().map(|character| {
                    json!({
                        "index": character.index,
                        "name": character.name,
                        "level": character.level,
                        "class": format!("{:?}", character.class),
                        "gender": format!("{:?}", character.gender)
                    })
                }).collect::<Vec<_>>()
            }
        }),
        ServerPacket::LogOutFailed => json!({
            "type": "packet",
            "packet": "LogOutFailed",
            "payload": {}
        }),
        other => {
            let (packet_name, payload) = typed_packet_event_detail(other);
            json!({
                "type": "packet",
                "packet": packet_name,
                "payload": payload
            })
        }
    }
}

fn typed_packet_event_detail(packet: &ServerPacket) -> (String, Value) {
    let encoded = match serde_json::to_value(packet) {
        Ok(value) => value,
        Err(error) => {
            return (
                server_packet_display_name(packet),
                json!({
                    "typed": true,
                    "summary": format!("{:?}", packet),
                    "serializationError": error.to_string()
                }),
            );
        }
    };

    match encoded {
        Value::Object(variants) => {
            let Some((packet_name, payload)) = variants.into_iter().next() else {
                return (server_packet_display_name(packet), json!({ "typed": true }));
            };
            let payload = match payload {
                Value::Object(mut payload) => {
                    payload.insert("typed".to_string(), Value::Bool(true));
                    Value::Object(payload)
                }
                Value::Null => json!({ "typed": true }),
                value => json!({
                "typed": true,
                "value": value
                }),
            };
            (packet_name, payload)
        }
        Value::String(packet_name) => (packet_name, json!({ "typed": true })),
        Value::Null => (server_packet_display_name(packet), json!({ "typed": true })),
        value => (
            server_packet_display_name(packet),
            json!({
                "typed": true,
                "value": value
            }),
        ),
    }
}

fn raw_payload_detail(packet_name: &str, packet_id: i16, payload: &[u8]) -> Value {
    json!({
        "packetName": packet_name,
        "packetId": packet_id,
        "payloadLength": payload.len(),
        "payloadHex": packet_payload_hex(payload),
        "rawPayloadLength": payload.len()
    })
}

fn movement_json(
    object_id: u32,
    x: i32,
    y: i32,
    direction: MirDirection,
) -> HashMap<&'static str, Value> {
    HashMap::from([
        ("objectId", json!(object_id)),
        ("x", json!(x)),
        ("y", json!(y)),
        ("direction", json!(format!("{direction:?}"))),
    ])
}

#[cfg(test)]
mod tests {
    use super::{
        responses_require_world_snapshot, should_send_world_snapshot_for_action, BrowserCommand,
        SessionAction,
    };
    use axum::extract::State;
    use axum::Json;
    use mir2_protocol::{
        ClientAuction, ClientBuff, ClientFriend, ClientHeroInformation, ClientIntelligentCreature,
        ClientMail, ClientMapInfo, ClientPacket, ClientQuestInfo, GroupMember,
        IntelligentCreatureItemFilter, IntelligentCreatureRules, MirClass, MirDirection, MirGender,
        MirGridType, ObjectManaInfo, Point, RankCharacterInfo, ServerPacket, ServerPacketId, Spell,
        UserItem, UserItemStat,
    };
    use mir2_simulation::{
        AccountStore, SimulationConfig, Stage5MailTargetKind, Stage5SystemsState,
    };
    use serde_json::json;
    use std::sync::Arc;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    fn sample_user_item(unique_id: u64, count: u16) -> UserItem {
        UserItem {
            unique_id,
            item_index: 321,
            current_dura: 1000,
            max_dura: 1000,
            count,
            soul_bound_id: -1,
            identified: true,
            cursed: false,
            slots: Vec::new(),
            gem_count: 1,
            added_stats: vec![UserItemStat { stat: 5, value: 1 }],
            awake_type: 0,
            awake_values: Vec::new(),
            refined_value: 0,
            refine_added: 0,
            refine_success_chance: 0,
            wedding_ring: -1,
            expire_info: None,
            rental_information: None,
            is_shop_item: false,
            sealed_info: None,
            gm_made: false,
        }
    }

    fn sample_intelligent_creature(slot_index: i32) -> ClientIntelligentCreature {
        ClientIntelligentCreature {
            pet_type: 1,
            icon: 44,
            custom_name: "Buddy".to_string(),
            fullness: 50,
            slot_index,
            expire_binary_datetime: 638000000000000000,
            blackstone_time: 12_000,
            pet_mode: 1,
            creature_rules: IntelligentCreatureRules {
                minimal_fullness: 1,
                mouse_pickup_enabled: true,
                mouse_pickup_range: 6,
                auto_pickup_enabled: false,
                auto_pickup_range: 0,
                semi_auto_pickup_enabled: true,
                semi_auto_pickup_range: 4,
                can_produce_blackstone: true,
            },
            filter: IntelligentCreatureItemFilter {
                pet_pickup_all: false,
                pet_pickup_gold: true,
                pet_pickup_weapons: false,
                pet_pickup_armours: false,
                pet_pickup_helmets: false,
                pet_pickup_boots: false,
                pet_pickup_belts: false,
                pet_pickup_accessories: false,
                pet_pickup_others: true,
            },
            pickup_grade: 2,
            maintain_food_time: 24_000,
        }
    }

    fn sample_hero_info(index: i32) -> ClientHeroInformation {
        ClientHeroInformation {
            index,
            name: format!("Hero{index}"),
            level: 12,
            class: MirClass::Taoist,
            gender: MirGender::Female,
        }
    }

    fn demo_game_session() -> crate::GatewaySession {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());
        let login_packets = session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        assert!(login_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        let start_packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(start_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { .. })));
        session
    }

    #[tokio::test]
    async fn admin_system_mail_endpoint_writes_live_account_store() {
        let path = std::env::temp_dir().join(format!(
            "mir2-gateway-admin-mail-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let config = SimulationConfig::default().with_account_store_path(path.clone());
        let default_character = config.default_character.clone();
        let state = super::WebState {
            config: Arc::new(config),
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            reconnect_sessions: Arc::new(super::ReconnectSessionStore::default()),
            capacity: Arc::new(super::GatewayCapacityState::unlimited()),
            gameplay_event_sink: None,
            injector: crate::inject::LiveSessionInjector::default(),
        };

        let Json(receipt) = super::admin_system_mail(
            State(state),
            Json(super::AdminSystemMailRequest {
                target_kind: Stage5MailTargetKind::Character,
                target_id: "Scout".into(),
                from: "GM System".into(),
                subject: "Endpoint smoke".into(),
                body: "Delivered through the live gateway admin endpoint.".into(),
                gold: 5000,
                items: vec!["red-potion".into()],
            }),
        )
        .await
        .expect("mail delivery should succeed");

        assert_eq!(receipt.delivered_count, 1);
        assert_eq!(receipt.mail_ids, vec![1]);

        let store = AccountStore::load_or_new(&path, default_character);
        let save = store
            .accounts
            .get("demo")
            .and_then(|account| account.saves.get(&0))
            .expect("demo character save should exist");
        let systems: Stage5SystemsState = serde_json::from_str(
            save.stage5_systems_json
                .as_deref()
                .expect("stage5 systems should be persisted"),
        )
        .expect("stage5 systems should decode");
        assert_eq!(systems.mail[0].subject, "Endpoint smoke");
        assert_eq!(systems.mail[0].gold, 5000);
        assert_eq!(systems.mail[0].items, vec!["red-potion"]);

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn health_reports_cache_and_gameplay_event_boundaries() {
        let event_sink = Arc::new(crate::InMemoryGameplayEventSink::default());
        let shared_event_sink: crate::SharedGameplayEventSink = event_sink;
        let state = super::WebState {
            config: Arc::new(crate::GatewayConfig::default()),
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            session_cache: Arc::new(crate::InMemoryGatewaySessionCache::default()),
            reconnect_sessions: Arc::new(super::ReconnectSessionStore::default()),
            capacity: Arc::new(super::GatewayCapacityState::unlimited()),
            gameplay_event_sink: Some(shared_event_sink),
            injector: crate::inject::LiveSessionInjector::default(),
        };

        let Json(response) = super::health(State(state)).await;

        assert!(response.ok);
        assert_eq!(response.session_cache.backend, "in_memory");
        assert!(response.session_cache.healthy);
        assert_eq!(
            response.capacity,
            super::GatewayCapacityStatus {
                max_ws_connections: None,
                max_active_sessions: None,
                max_reconnect_leases: None,
                max_login_in_flight: None,
                max_new_character_in_flight: None,
                max_start_game_in_flight: None,
                current_ws_connections: 0,
                current_active_sessions: 0,
                current_reconnect_leases: 0,
                current_login_in_flight: 0,
                current_new_character_in_flight: 0,
                current_start_game_in_flight: 0,
            }
        );
        assert!(response.gameplay_events.configured);
        assert_eq!(
            response.gameplay_events.topic.as_deref(),
            Some(crate::events::DEFAULT_GAMEPLAY_EVENT_TOPIC)
        );
    }

    #[tokio::test]
    async fn admin_sessions_and_control_endpoints_are_queryable() {
        let cache = Arc::new(crate::InMemoryGatewaySessionCache::default());
        crate::GatewaySessionCache::put(
            cache.as_ref(),
            crate::GatewaySessionCacheRecord {
                key: crate::GatewaySessionCacheKey {
                    account_id: "demo".into(),
                    character_index: 0,
                },
                character_name: "Scout".into(),
                zone_id: Some("crystal".into()),
                zone_owner_id: Some("owner-crystal".into()),
                zone_owner_fencing_token: Some(1),
                map_file_name: Some("0".into()),
                player_object_id: Some(1001),
                player_hp: Some(18),
                player_max_hp: Some(18),
                gold: 50,
                tick: 7,
                updated_at_ms: 1_000,
                route_lease_owner: None,
                route_lease_expires_at_ms: None,
            },
        );
        let state = super::WebState {
            config: Arc::new(crate::GatewayConfig::default()),
            zone_registry: Arc::new(crate::ZoneRegistry::in_process()),
            session_cache: cache,
            reconnect_sessions: Arc::new(super::ReconnectSessionStore::default()),
            capacity: Arc::new(super::GatewayCapacityState::unlimited()),
            gameplay_event_sink: None,
            injector: crate::inject::LiveSessionInjector::default(),
        };

        let Json(sessions) = super::admin_sessions(State(state)).await;
        assert_eq!(sessions.source, "gateway_session_cache");
        assert_eq!(sessions.sessions.len(), 1);
        assert_eq!(sessions.sessions[0].character_name, "Scout");

        let Json(receipt) = super::admin_control(Json(super::AdminControlRequest {
            action: "reload-npcs".into(),
            target: Some("world".into()),
            operator_id: Some("op-1".into()),
            reason: Some("endpoint smoke".into()),
        }))
        .await
        .expect("reload should be accepted");
        assert_eq!(receipt.action, "reload_npcs");
        assert!(receipt.accepted);
        assert!(receipt.message.contains("operator=op-1"));

        let stop_error = super::admin_control(Json(super::AdminControlRequest {
            action: "stop".into(),
            target: None,
            operator_id: None,
            reason: None,
        }))
        .await
        .expect_err("dev gateway should not execute stop");
        assert_eq!(stop_error.0, axum::http::StatusCode::CONFLICT);
    }

    #[test]
    fn login_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"login","accountId":"demo","password":"demo"}"#,
        )
        .expect("login command should deserialize");

        match command {
            BrowserCommand::Login {
                account_id,
                password,
            } => {
                assert_eq!(account_id, "demo");
                assert_eq!(password, "demo");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn disconnect_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(r#"{"type":"disconnect"}"#)
            .expect("disconnect command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("disconnect command should map to a session action");

        assert!(matches!(
            action,
            SessionAction::Packet(ClientPacket::Disconnect)
        ));
    }

    #[test]
    fn keep_alive_command_maps_to_protocol_packet_without_snapshot() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"keepAlive","time":12345}"#)
                .expect("keep alive command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("keep alive command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::KeepAlive { time }) => assert_eq!(time, 12345),
            _ => panic!("unexpected action"),
        }

        assert!(!should_send_world_snapshot_for_action(
            &SessionAction::Packet(ClientPacket::KeepAlive { time: 12345 },)
        ));
        assert!(!super::should_queue_save_for_action(
            &SessionAction::Packet(ClientPacket::KeepAlive { time: 12345 },)
        ));
        assert!(
            super::inflight_capacity_kind_for_action(&SessionAction::Packet(
                ClientPacket::KeepAlive { time: 12345 },
            ))
            .is_none()
        );
        assert!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::KeepAlive { time: 12345 },
            ))
            .is_none()
        );
    }

    #[test]
    fn new_account_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"newAccount","accountId":"fresh","password":"demo","birthDateBinary":42,"userName":"Fresh User","secretQuestion":"Q","secretAnswer":"A","emailAddress":"fresh@example.test"}"#,
        )
        .expect("new account command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("new account command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::NewAccount {
                account_id,
                password,
                birth_date_binary,
                user_name,
                secret_question,
                secret_answer,
                email_address,
            }) => {
                assert_eq!(account_id, "fresh");
                assert_eq!(password, "demo");
                assert_eq!(birth_date_binary, 42);
                assert_eq!(user_name, "Fresh User");
                assert_eq!(secret_question, "Q");
                assert_eq!(secret_answer, "A");
                assert_eq!(email_address, "fresh@example.test");
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn change_password_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"changePassword","accountId":"demo","currentPassword":"old","newPassword":"new"}"#,
        )
        .expect("change password command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("change password command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::ChangePassword {
                account_id,
                current_password,
                new_password,
            }) => {
                assert_eq!(account_id, "demo");
                assert_eq!(current_password, "old");
                assert_eq!(new_password, "new");
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn storage_password_commands_map_to_protocol_packets() {
        let unlock = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"unlockStorage","password":"vault"}"#,
        )
        .expect("unlock storage command should deserialize");
        let set = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"setStoragePassword","currentPassword":"vault","newPassword":"new-vault"}"#,
        )
        .expect("set storage password command should deserialize");
        let remove = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"removeStoragePassword","currentPassword":"new-vault"}"#,
        )
        .expect("remove storage password command should deserialize");

        match super::browser_command_to_action(unlock).expect("unlock should map") {
            SessionAction::Packet(ClientPacket::UnlockStorage { password }) => {
                assert_eq!(password, "vault")
            }
            _ => panic!("unexpected action"),
        }
        match super::browser_command_to_action(set).expect("set should map") {
            SessionAction::Packet(ClientPacket::SetStoragePassword {
                current_password,
                new_password,
            }) => {
                assert_eq!(current_password, "vault");
                assert_eq!(new_password, "new-vault");
            }
            _ => panic!("unexpected action"),
        }
        match super::browser_command_to_action(remove).expect("remove should map") {
            SessionAction::Packet(ClientPacket::RemoveStoragePassword { current_password }) => {
                assert_eq!(current_password, "new-vault")
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn storage_password_server_events_expose_crystal_payload_fields() {
        let unlock = super::server_packet_to_event(&ServerPacket::StorageUnlockResult {
            result: 0,
            has_password: true,
        });
        assert_eq!(unlock["packet"], "StorageUnlockResult");
        assert_eq!(unlock["payload"]["result"], 0);
        assert_eq!(unlock["payload"]["hasPassword"], true);

        let password = super::server_packet_to_event(&ServerPacket::StoragePasswordResult {
            result: 4,
            removing: true,
            has_password: false,
            last_set_binary_datetime: 638000000000000000,
        });
        assert_eq!(password["packet"], "StoragePasswordResult");
        assert_eq!(password["payload"]["result"], 4);
        assert_eq!(password["payload"]["removing"], true);
        assert_eq!(password["payload"]["hasPassword"], false);
        assert_eq!(
            password["payload"]["lastSetBinaryDatetime"],
            638000000000000000_i64
        );

        let user_storage = super::server_packet_to_event(&ServerPacket::UserStorage {
            storage: Some(vec![Some(sample_user_item(90, 2)), None]),
        });
        assert_eq!(user_storage["packet"], "UserStorage");
        assert_eq!(user_storage["payload"]["storage"][0]["unique_id"], 90);
        assert_eq!(user_storage["payload"]["storage"][0]["count"], 2);
        assert!(user_storage["payload"]["storage"][1].is_null());
    }

    #[test]
    fn raw_server_events_expose_copyable_payload_fields() {
        let raw = super::server_packet_to_event(&ServerPacket::Raw {
            packet_id: ServerPacketId::TimeOfDay,
            payload: vec![0x00, 0x11, 0x22, 0xaa],
        });
        assert_eq!(raw["packet"], "TimeOfDay");
        assert_eq!(
            raw["payload"]["packetId"],
            json!(ServerPacketId::TimeOfDay as i16)
        );
        assert_eq!(raw["payload"]["packetName"], "TimeOfDay");
        assert_eq!(raw["payload"]["payloadLength"], 4);
        assert_eq!(raw["payload"]["payloadHex"], "001122aa");
        assert_eq!(raw["payload"]["rawPayloadLength"], 4);
    }

    #[test]
    fn newly_typed_server_events_expose_structured_payload_fields() {
        let map = super::server_packet_to_event(&ServerPacket::NewMapInfo {
            map_index: 77,
            info: ClientMapInfo {
                title: "CastleGi-Ryoong".into(),
                width: 120,
                height: 220,
                big_map: 121,
                movements: vec![],
                npcs: vec![],
            },
        });
        assert_eq!(map["packet"], "NewMapInfo");
        assert_eq!(map["payload"]["typed"], true);
        assert_eq!(map["payload"]["mapIndex"], 77);
        assert_eq!(map["payload"]["info"]["title"], "CastleGi-Ryoong");
        assert_eq!(map["payload"]["info"]["bigMap"], 121);
        assert!(!map["payload"].as_object().unwrap().contains_key("summary"));

        let rankings = super::server_packet_to_event(&ServerPacket::Rankings {
            rank_type: 2,
            my_rank: 5,
            listing_details: vec![RankCharacterInfo {
                player_id: 9001,
                name: "RankedHero".into(),
                level: 45,
                class: MirClass::Taoist,
            }],
            listings: vec![123_456],
            count: 1,
        });
        assert_eq!(rankings["packet"], "Rankings");
        assert_eq!(rankings["payload"]["typed"], true);
        assert_eq!(rankings["payload"]["rankType"], 2);
        assert_eq!(rankings["payload"]["myRank"], 5);
        assert_eq!(
            rankings["payload"]["listingDetails"][0]["name"],
            "RankedHero"
        );
        assert_eq!(rankings["payload"]["listings"][0], 123_456);

        let unit = super::server_packet_to_event(&ServerPacket::ReturnToLogin);
        assert_eq!(unit["packet"], "ReturnToLogin");
        assert_eq!(unit["payload"], json!({ "typed": true }));
    }

    #[test]
    fn resize_storage_server_event_exposes_crystal_payload_fields() {
        let resize = super::server_packet_to_event(&ServerPacket::ResizeStorage {
            size: 160,
            has_expanded_storage: true,
            expiry_time_binary_datetime: 638000000000000000,
        });
        assert_eq!(resize["packet"], "ResizeStorage");
        assert_eq!(resize["payload"]["size"], 160);
        assert_eq!(resize["payload"]["hasExpandedStorage"], true);
        assert_eq!(
            resize["payload"]["expiryTimeBinaryDatetime"],
            638000000000000000_i64
        );
    }

    #[test]
    fn credit_delta_server_events_expose_crystal_payload_fields() {
        let gained = super::server_packet_to_event(&ServerPacket::GainedCredit { credit: 45 });
        assert_eq!(gained["packet"], "GainedCredit");
        assert_eq!(gained["payload"]["credit"], 45);

        let lost = super::server_packet_to_event(&ServerPacket::LoseCredit { credit: 12 });
        assert_eq!(lost["packet"], "LoseCredit");
        assert_eq!(lost["payload"]["credit"], 12);
    }

    #[test]
    fn item_slot_and_seal_server_events_expose_crystal_payload_fields() {
        let slot = super::server_packet_to_event(&ServerPacket::ItemSlotSizeChanged {
            unique_id: 42,
            slot_size: 3,
        });
        assert_eq!(slot["packet"], "ItemSlotSizeChanged");
        assert_eq!(slot["payload"]["uniqueId"], 42);
        assert_eq!(slot["payload"]["slotSize"], 3);

        let seal = super::server_packet_to_event(&ServerPacket::ItemSealChanged {
            unique_id: 43,
            expiry_date_binary_datetime: 638000000000000000,
        });
        assert_eq!(seal["packet"], "ItemSealChanged");
        assert_eq!(seal["payload"]["uniqueId"], 43);
        assert_eq!(
            seal["payload"]["expiryDateBinaryDatetime"],
            638000000000000000_i64
        );

        let upgraded = super::server_packet_to_event(&ServerPacket::ItemUpgraded {
            item: sample_user_item(44, 1),
        });
        assert_eq!(upgraded["packet"], "ItemUpgraded");
        assert_eq!(upgraded["payload"]["item"]["unique_id"], 44);
    }

    #[test]
    fn combine_item_server_event_exposes_crystal_payload_fields() {
        let packet = super::server_packet_to_event(&ServerPacket::CombineItem {
            grid: MirGridType::Inventory,
            id_from: 31,
            id_to: 32,
            success: true,
            destroy: false,
        });
        assert_eq!(packet["packet"], "CombineItem");
        assert_eq!(packet["payload"]["grid"], "Inventory");
        assert_eq!(packet["payload"]["idFrom"], 31);
        assert_eq!(packet["payload"]["idTo"], 32);
        assert_eq!(packet["payload"]["success"], true);
        assert_eq!(packet["payload"]["destroy"], false);
    }

    #[test]
    fn npc_service_server_events_expose_crystal_payload_fields() {
        let goods = super::server_packet_to_event(&ServerPacket::NPCGoods {
            list: Vec::new(),
            rate: 1.25,
            panel_type: 3,
            hide_added_stats: true,
        });
        assert_eq!(goods["packet"], "NPCGoods");
        assert_eq!(goods["payload"]["rate"], 1.25);
        assert_eq!(goods["payload"]["panelType"], 3);
        assert_eq!(goods["payload"]["hideAddedStats"], true);

        let repair = super::server_packet_to_event(&ServerPacket::NPCRepair { rate: 1.5 });
        assert_eq!(repair["packet"], "NPCRepair");
        assert_eq!(repair["payload"]["rate"], 1.5);

        let refine = super::server_packet_to_event(&ServerPacket::NPCRefine {
            rate: 2.5,
            refining: true,
        });
        assert_eq!(refine["packet"], "NPCRefine");
        assert_eq!(refine["payload"]["rate"], 2.5);
        assert_eq!(refine["payload"]["refining"], true);

        let craft = super::server_packet_to_event(&ServerPacket::CraftItem { success: false });
        assert_eq!(craft["packet"], "CraftItem");
        assert_eq!(craft["payload"]["success"], false);
    }

    #[test]
    fn start_game_command_accepts_camel_case_fields() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"startGame","characterIndex":0}"#)
                .expect("start game command should deserialize");

        match command {
            BrowserCommand::StartGame { character_index } => {
                assert_eq!(character_index, 0);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn pickup_command_accepts_camel_case_fields() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"pickUp","objectId":5000}"#)
                .expect("pickup command should deserialize");

        match command {
            BrowserCommand::PickUp { object_id } => {
                assert_eq!(object_id, 5000);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn use_item_command_accepts_key_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"useItem","key":"red-potion"}"#)
                .expect("use item command should deserialize");

        match command {
            BrowserCommand::UseItem { key, .. } => assert_eq!(key.as_deref(), Some("red-potion")),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn use_item_with_unique_id_maps_to_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"useItem","uniqueId":1024,"grid":"equipment"}"#,
        )
        .expect("use item command should deserialize");
        match super::browser_command_to_action(command)
            .expect("use item command should map to a session action")
        {
            SessionAction::Packet(ClientPacket::UseItem { unique_id, grid }) => {
                assert_eq!(unique_id, 1024);
                assert_eq!(grid, MirGridType::Equipment);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn drop_item_command_accepts_key_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"dropItem","key":"red-potion"}"#)
                .expect("drop item command should deserialize");

        match command {
            BrowserCommand::DropItem { key, .. } => assert_eq!(key, "red-potion"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn delete_character_command_accepts_camel_case_fields() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"deleteCharacter","characterIndex":1}"#,
        )
        .expect("delete character command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("delete character command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::DeleteCharacter { character_index }) => {
                assert_eq!(character_index, 1)
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn equip_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"equipItem","uniqueId":3,"grid":"inventory","to":2}"#,
        )
        .expect("equip item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("equip item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::EquipItem {
                unique_id,
                grid,
                to,
            }) => {
                assert_eq!(unique_id, 3);
                assert_eq!(grid, mir2_protocol::MirGridType::Inventory);
                assert_eq!(to, 2);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn split_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"splitItem","uniqueId":0,"grid":"inventory","count":2}"#,
        )
        .expect("split item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("split item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::SplitItem {
                unique_id,
                grid,
                count,
            }) => {
                assert_eq!(unique_id, 0);
                assert_eq!(grid, mir2_protocol::MirGridType::Inventory);
                assert_eq!(count, 2);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn remove_slot_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"removeSlotItem","uniqueId":2,"grid":"equipment","gridTo":"inventory","to":5,"fromUniqueId":9}"#,
        )
        .expect("remove slot item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("remove slot item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::RemoveSlotItem {
                unique_id,
                grid,
                grid_to,
                to,
                from_unique_id,
            }) => {
                assert_eq!(unique_id, 2);
                assert_eq!(grid, mir2_protocol::MirGridType::Equipment);
                assert_eq!(grid_to, mir2_protocol::MirGridType::Inventory);
                assert_eq!(to, 5);
                assert_eq!(from_unique_id, 9);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn merge_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"mergeItem","gridFrom":"inventory","gridTo":"inventory","idFrom":1,"idTo":4}"#,
        )
        .expect("merge item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("merge item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::MergeItem {
                grid_from,
                grid_to,
                id_from,
                id_to,
            }) => {
                assert_eq!(grid_from, mir2_protocol::MirGridType::Inventory);
                assert_eq!(grid_to, mir2_protocol::MirGridType::Inventory);
                assert_eq!(id_from, 1);
                assert_eq!(id_to, 4);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn drop_gold_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(r#"{"type":"dropGold","amount":88}"#)
            .expect("drop gold command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("drop gold command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::DropGold { amount }) => assert_eq!(amount, 88),
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn request_item_info_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"requestItemInfo","itemIndex":658}"#)
                .expect("request item info command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("request item info command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::RequestItemInfo { item_index }) => {
                assert_eq!(item_index, 658);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn delete_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"deleteItem","uniqueId":2,"count":3,"heroInventory":false}"#,
        )
        .expect("delete item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("delete item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::DeleteItem {
                unique_id,
                count,
                hero_inventory,
            }) => {
                assert_eq!(unique_id, 2);
                assert_eq!(count, 3);
                assert!(!hero_inventory);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn sell_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"sellItem","uniqueId":2,"count":1}"#)
                .expect("sell item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("sell item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::SellItem { unique_id, count }) => {
                assert_eq!(unique_id, 2);
                assert_eq!(count, 1);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn buy_item_command_maps_to_protocol_packet() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"buyItem","itemIndex":43122688,"count":5,"panelType":0}"#,
        )
        .expect("buy item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("buy item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::BuyItem {
                item_index,
                count,
                panel_type,
            }) => {
                assert_eq!(item_index, 43_122_688);
                assert_eq!(count, 5);
                assert_eq!(panel_type, 0);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn repair_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"repairItem","uniqueId":5}"#)
                .expect("repair item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("repair item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::RepairItem { unique_id }) => {
                assert_eq!(unique_id, 5);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn special_repair_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"specialRepairItem","uniqueId":6}"#)
                .expect("special repair item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("special repair item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::SRepairItem { unique_id }) => {
                assert_eq!(unique_id, 6);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn store_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"storeItem","from":2,"to":5}"#)
                .expect("store item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("store item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::StoreItem { from, to }) => {
                assert_eq!(from, 2);
                assert_eq!(to, 5);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn take_back_item_command_maps_to_protocol_packet() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"takeBackItem","from":5,"to":3}"#)
                .expect("take back item command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("take back item command should map to a session action");

        match action {
            SessionAction::Packet(ClientPacket::TakeBackItem { from, to }) => {
                assert_eq!(from, 5);
                assert_eq!(to, 3);
            }
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn magic_commands_map_to_crystal_protocol_packets() {
        let key_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"magicKey","spell":"Fury","key":5,"oldKey":0}"#,
        )
        .expect("magic key command should deserialize");
        let action = super::browser_command_to_action(key_command)
            .expect("magic key command should map to a session action");
        match action {
            SessionAction::Packet(ClientPacket::MagicKey {
                spell,
                key,
                old_key,
            }) => {
                assert_eq!(spell, Spell::Fury);
                assert_eq!(key, 5);
                assert_eq!(old_key, 0);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let magic_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"magic","objectId":1001,"spell":"Healing","direction":"downLeft","targetId":1001,"x":333,"y":267,"spellTargetLock":true}"#,
        )
        .expect("magic command should deserialize");
        let action = super::browser_command_to_action(magic_command)
            .expect("magic command should map to a session action");
        match action {
            SessionAction::Packet(ClientPacket::Magic {
                object_id,
                spell,
                direction,
                target_id,
                location,
                spell_target_lock,
            }) => {
                assert_eq!(object_id, 1_001);
                assert_eq!(spell, Spell::Healing);
                assert_eq!(direction, MirDirection::DownLeft);
                assert_eq!(target_id, 1_001);
                assert_eq!(location, Point { x: 333, y: 267 });
                assert!(spell_target_lock);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let toggle_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"spellToggle","spell":"Slaying","canUse":true}"#,
        )
        .expect("spell toggle command should deserialize");
        let action = super::browser_command_to_action(toggle_command)
            .expect("spell toggle command should map to a session action");
        match action {
            SessionAction::Packet(ClientPacket::SpellToggle {
                spell,
                toggle_state,
            }) => {
                assert_eq!(spell, Spell::Slaying);
                assert_eq!(toggle_state, 1);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn magic_packets_are_exposed_as_browser_events() {
        let magic = super::server_packet_to_event(&ServerPacket::Magic {
            spell: Spell::Fury,
            target_id: 0,
            target: Point { x: 333, y: 267 },
            cast: true,
            level: 3,
            secondary_target_ids: vec![2],
        });
        assert_eq!(magic["packet"], "Magic");
        assert_eq!(magic["payload"]["spell"], "Fury");
        assert_eq!(magic["payload"]["secondaryTargetIds"][0], 2);

        let object_magic = super::server_packet_to_event(&ServerPacket::ObjectMagic {
            object_id: 1_001,
            location: Point { x: 332, y: 267 },
            direction: MirDirection::Right,
            spell: Spell::Fury,
            target_id: 0,
            target: Point { x: 333, y: 267 },
            cast: true,
            level: 3,
            self_broadcast: false,
            secondary_target_ids: Vec::new(),
        });
        assert_eq!(object_magic["packet"], "ObjectMagic");
        assert_eq!(object_magic["payload"]["objectId"], 1_001);
        assert_eq!(object_magic["payload"]["direction"], "Right");

        let toggle = super::server_packet_to_event(&ServerPacket::SpellToggle {
            object_id: 1_001,
            spell: Spell::Slaying,
            can_use: true,
        });
        assert_eq!(toggle["packet"], "SpellToggle");
        assert_eq!(toggle["payload"]["canUse"], true);

        let mana = super::server_packet_to_event(&ServerPacket::ObjectMana {
            info: ObjectManaInfo {
                object_id: 1_001,
                percent: 88,
            },
        });
        assert_eq!(mana["packet"], "ObjectMana");
        assert_eq!(mana["payload"]["percent"], 88);

        let add_buff = super::server_packet_to_event(&ServerPacket::AddBuff {
            buff: ClientBuff {
                buff_type: 5,
                visible: true,
                object_id: 1_001,
                expire_time: 60_000,
                infinite: false,
                paused: false,
                stats: vec![UserItemStat { stat: 14, value: 4 }],
                values: Vec::new(),
            },
        });
        assert_eq!(add_buff["packet"], "AddBuff");
        assert_eq!(add_buff["payload"]["buffType"], 5);
        assert_eq!(add_buff["payload"]["stats"][0]["stat"], 14);
        // Browser buff contract: numeric `type`, `remainingMs` (= expire_time, which
        // the sim already stores as ms-from-now), and `{label, value}` stats.
        assert_eq!(add_buff["payload"]["type"], 5);
        assert_eq!(add_buff["payload"]["remainingMs"], 60_000);
        assert_eq!(add_buff["payload"]["stats"][0]["label"], "AttackSpeed");
        assert_eq!(add_buff["payload"]["stats"][0]["value"], 4);

        let remove_buff = super::server_packet_to_event(&ServerPacket::RemoveBuff {
            buff_type: 5,
            object_id: 1_001,
        });
        assert_eq!(remove_buff["packet"], "RemoveBuff");
        assert_eq!(remove_buff["payload"]["objectId"], 1_001);
    }

    #[test]
    fn simple_bootstrap_social_packets_are_exposed_as_browser_events() {
        let time = super::server_packet_to_event(&ServerPacket::TimeOfDay { lights: 4 });
        assert_eq!(time["packet"], "TimeOfDay");
        assert_eq!(time["payload"]["lights"], 4);

        let attack = super::server_packet_to_event(&ServerPacket::ChangeAMode { mode: 2 });
        assert_eq!(attack["packet"], "ChangeAMode");
        assert_eq!(attack["payload"]["mode"], 2);

        let npc_response = super::server_packet_to_event(&ServerPacket::NPCResponse {
            page: vec!["@main".to_string(), "Welcome".to_string()],
        });
        assert_eq!(npc_response["packet"], "NPCResponse");
        assert_eq!(npc_response["payload"]["page"][1], "Welcome");

        let default_npc =
            super::server_packet_to_event(&ServerPacket::DefaultNPC { object_id: 1_001 });
        assert_eq!(default_npc["packet"], "DefaultNPC");
        assert_eq!(default_npc["payload"]["objectId"], 1_001);

        let npc_update = super::server_packet_to_event(&ServerPacket::NPCUpdate { npc_id: 1_002 });
        assert_eq!(npc_update["packet"], "NPCUpdate");
        assert_eq!(npc_update["payload"]["npcId"], 1_002);

        let lover = super::server_packet_to_event(&ServerPacket::LoverUpdate {
            name: "Partner".to_string(),
            date_binary_datetime: 42,
            map_name: "Bichon".to_string(),
            married_days: 7,
        });
        assert_eq!(lover["packet"], "LoverUpdate");
        assert_eq!(lover["payload"]["mapName"], "Bichon");

        let mentor = super::server_packet_to_event(&ServerPacket::MentorUpdate {
            name: "Mentor".to_string(),
            level: 45,
            online: true,
            mentee_exp: 1_234,
        });
        assert_eq!(mentor["packet"], "MentorUpdate");
        assert_eq!(mentor["payload"]["online"], true);
        assert_eq!(mentor["payload"]["menteeExp"], 1_234);
    }

    #[test]
    fn group_utility_packets_are_exposed_as_browser_events() {
        let switch =
            super::server_packet_to_event(&ServerPacket::SwitchGroup { allow_group: true });
        assert_eq!(switch["packet"], "SwitchGroup");
        assert_eq!(switch["payload"]["allowGroup"], true);

        let add = super::server_packet_to_event(&ServerPacket::AddMember {
            name: "Scout".to_string(),
        });
        assert_eq!(add["packet"], "AddMember");
        assert_eq!(add["payload"]["name"], "Scout");

        let member_map = super::server_packet_to_event(&ServerPacket::GroupMembersMap {
            player_name: "Scout".to_string(),
            player_map: "Bichon Province".to_string(),
        });
        assert_eq!(member_map["packet"], "GroupMembersMap");
        assert_eq!(member_map["payload"]["playerMap"], "Bichon Province");

        // Enriched roster: members widen to objects (level/class/hp/maxHp/online),
        // unknown fields are omitted, and `leaderName` mirrors members[0].
        let roster = super::server_packet_to_event(&ServerPacket::GroupMemberInfo {
            members: vec![
                GroupMember {
                    name: "Scout".to_string(),
                    level: Some(33),
                    class: Some(MirClass::Archer as u8),
                    hp: Some(210),
                    max_hp: Some(260),
                    online: Some(true),
                },
                GroupMember {
                    name: "Faraway".to_string(),
                    level: Some(40),
                    class: Some(MirClass::Wizard as u8),
                    hp: None,
                    max_hp: None,
                    online: Some(true),
                },
            ],
            leader_name: "Scout".to_string(),
        });
        assert_eq!(roster["packet"], "GroupMemberInfo");
        assert_eq!(roster["payload"]["leaderName"], "Scout");
        assert_eq!(roster["payload"]["members"][0]["name"], "Scout");
        assert_eq!(roster["payload"]["members"][0]["level"], 33);
        assert_eq!(roster["payload"]["members"][0]["class"], 4);
        assert_eq!(roster["payload"]["members"][0]["hp"], 210);
        assert_eq!(roster["payload"]["members"][0]["maxHp"], 260);
        assert_eq!(roster["payload"]["members"][0]["online"], true);
        // A member with unknown HP omits hp/maxHp entirely.
        assert!(roster["payload"]["members"][1].get("hp").is_none());
        assert!(roster["payload"]["members"][1].get("maxHp").is_none());
        assert_eq!(roster["payload"]["members"][1]["class"], 1);

        let location = super::server_packet_to_event(&ServerPacket::SendMemberLocation {
            member_name: "Scout".to_string(),
            member_location: Point { x: 330, y: 270 },
        });
        assert_eq!(location["packet"], "SendMemberLocation");
        assert_eq!(location["payload"]["memberLocation"]["x"], 330);

        let door = super::server_packet_to_event(&ServerPacket::OpenDoor {
            door_index: 4,
            close: false,
        });
        assert_eq!(door["packet"], "Opendoor");
        assert_eq!(door["payload"]["doorIndex"], 4);

        let timer = super::server_packet_to_event(&ServerPacket::SetTimer {
            key: "quest".to_string(),
            timer_type: 1,
            seconds: 60,
        });
        assert_eq!(timer["packet"], "SetTimer");
        assert_eq!(timer["payload"]["seconds"], 60);

        let compass = super::server_packet_to_event(&ServerPacket::SetCompass {
            location: Point { x: 331, y: 271 },
        });
        assert_eq!(compass["packet"], "SetCompass");
        assert_eq!(compass["payload"]["location"]["y"], 271);
    }

    #[test]
    fn quest_packets_are_exposed_as_browser_events() {
        let change = super::server_packet_to_event(&ServerPacket::ChangeQuest {
            quest_id: 1001,
            task_list: vec!["Collect Wasp Stinger 0/1".to_string()],
            taken: true,
            completed: false,
            new: true,
            quest_state: 0,
            track_quest: true,
        });
        assert_eq!(change["packet"], "ChangeQuest");
        assert_eq!(change["payload"]["questId"], 1001);
        assert_eq!(change["payload"]["taskList"][0], "Collect Wasp Stinger 0/1");
        // Contract: dynamic quest fields are hoisted from the progress payload.
        assert_eq!(change["payload"]["id"], 1001);
        assert_eq!(change["payload"]["state"], 0);
        assert_eq!(
            change["payload"]["descriptionLines"][0],
            "Collect Wasp Stinger 0/1"
        );
        // The "0/1" progress fraction is parsed into current/required.
        let objective = &change["payload"]["objectives"][0];
        assert_eq!(objective["text"], "Collect Wasp Stinger 0/1");
        assert_eq!(objective["current"], 0);
        assert_eq!(objective["required"], 1);

        let complete = super::server_packet_to_event(&ServerPacket::CompleteQuest {
            completed_quests: vec![1001],
        });
        assert_eq!(complete["packet"], "CompleteQuest");
        assert_eq!(complete["payload"]["completedQuests"][0], 1001);

        let share = super::server_packet_to_event(&ServerPacket::ShareQuest {
            quest_index: 1001,
            sharer_name: "Scout".to_string(),
        });
        assert_eq!(share["packet"], "ShareQuest");
        assert_eq!(share["payload"]["sharerName"], "Scout");
    }

    #[test]
    fn new_quest_info_exposes_contract_quest_fields() {
        let info = super::server_packet_to_event(&ServerPacket::NewQuestInfo {
            info: ClientQuestInfo {
                index: 1001,
                npc_index: 1_001,
                name: "Field Wasp".to_string(),
                group: "Starter".to_string(),
                description: vec!["Help the town guard.".to_string()],
                task_description: vec!["Defeat 3 Wasps 0/3".to_string()],
                return_description: vec!["Return to the guard.".to_string()],
                completion_description: vec!["Good work.".to_string()],
                min_level_needed: 1,
                max_level_needed: 0,
                quest_needed: 0,
                class_needed: 31,
                quest_type: 0,
                time_limit_in_seconds: 90,
                reward_gold: 500,
                reward_exp: 1_200,
                reward_credit: 0,
                rewards_fixed_item: Vec::new(),
                rewards_select_item: Vec::new(),
                finish_npc_index: 1_001,
            },
        });
        assert_eq!(info["packet"], "NewQuestInfo");
        // Raw info retained for backward compatibility.
        assert_eq!(info["payload"]["info"]["index"], 1001);
        // Contract-shaped quest fields hoisted alongside it.
        assert_eq!(info["payload"]["id"], 1001);
        assert_eq!(info["payload"]["name"], "Field Wasp");
        assert_eq!(info["payload"]["group"], "Starter");
        assert_eq!(
            info["payload"]["descriptionLines"][0],
            "Help the town guard."
        );
        assert_eq!(
            info["payload"]["objectives"][0]["text"],
            "Defeat 3 Wasps 0/3"
        );
        assert_eq!(info["payload"]["objectives"][0]["current"], 0);
        assert_eq!(info["payload"]["objectives"][0]["required"], 3);
        assert_eq!(info["payload"]["rewards"]["gold"], 500);
        assert_eq!(info["payload"]["rewards"]["experience"], 1_200);
        // No credit reward -> field omitted.
        assert!(info["payload"]["rewards"].get("credit").is_none());
        // 90 seconds -> "01:30".
        assert_eq!(info["payload"]["timeLimit"], "01:30");
        // npc (a name) is omitted because only NPCIndex is known here.
        assert!(info["payload"].get("npc").is_none());
    }

    #[test]
    fn refine_packets_are_exposed_as_browser_events() {
        let deposit = super::server_packet_to_event(&ServerPacket::DepositRefineItem {
            from: 4,
            to: 0,
            success: true,
        });
        assert_eq!(deposit["packet"], "DepositRefineItem");
        assert_eq!(deposit["payload"]["success"], true);

        let retrieve = super::server_packet_to_event(&ServerPacket::RetrieveRefineItem {
            from: 0,
            to: 4,
            success: true,
        });
        assert_eq!(retrieve["packet"], "RetrieveRefineItem");
        assert_eq!(retrieve["payload"]["to"], 4);

        let cancel = super::server_packet_to_event(&ServerPacket::RefineCancel);
        assert_eq!(cancel["packet"], "RefineCancel");

        let refine = super::server_packet_to_event(&ServerPacket::RefineItem { unique_id: 4 });
        assert_eq!(refine["packet"], "RefineItem");
        assert_eq!(refine["payload"]["uniqueId"], 4);
    }

    #[test]
    fn hero_commands_map_to_crystal_protocol_packets() {
        let new_hero = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"newHero","name":"Aide","gender":"female","class":"taoist"}"#,
        )
        .expect("new hero command should deserialize");
        match super::browser_command_to_action(new_hero).expect("new hero maps") {
            SessionAction::Packet(ClientPacket::NewHero {
                name,
                gender,
                class,
            }) => {
                assert_eq!(name, "Aide");
                assert_eq!(gender, MirGender::Female);
                assert_eq!(class, MirClass::Taoist);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let take_back = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"takeBackHeroItem","from":3,"to":1}"#,
        )
        .expect("take back hero item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(take_back).expect("take back maps"),
            SessionAction::Packet(ClientPacket::TakeBackHeroItem { from: 3, to: 1 })
        ));

        let transfer = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"transferHeroItem","from":1,"to":3}"#,
        )
        .expect("transfer hero item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(transfer).expect("transfer maps"),
            SessionAction::Packet(ClientPacket::TransferHeroItem { from: 1, to: 3 })
        ));

        let behaviour =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"setHeroBehaviour","behaviour":2}"#)
                .expect("set hero behaviour command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(behaviour).expect("behaviour maps"),
            SessionAction::Packet(ClientPacket::SetHeroBehaviour { behaviour: 2 })
        ));

        let change =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"changeHero","listIndex":1}"#)
                .expect("change hero command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(change).expect("change maps"),
            SessionAction::Packet(ClientPacket::ChangeHero { list_index: 1 })
        ));
    }

    // Newly-wired player->server actions: hero auto-pot config (Crystal
    // C.SetAutoPotValue / C.SetAutoPotItem), attack/pet mode toggles
    // (C.ChangeAMode / C.ChangePMode) and the door / conquest gate open
    // (C.Opendoor). Each previously had a simulation handler but no browser
    // command, so the frontend could not reach them.
    #[test]
    fn hero_and_world_control_commands_map_to_crystal_protocol_packets() {
        let auto_pot_value = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"setAutoPotValue","stat":0,"value":80}"#,
        )
        .expect("set auto pot value command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(auto_pot_value).expect("auto pot value maps"),
            SessionAction::Packet(ClientPacket::SetAutoPotValue { stat: 0, value: 80 })
        ));

        let auto_pot_item = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"setAutoPotItem","grid":"heroHpItem","itemIndex":0}"#,
        )
        .expect("set auto pot item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(auto_pot_item).expect("auto pot item maps"),
            SessionAction::Packet(ClientPacket::SetAutoPotItem {
                grid: MirGridType::HeroHpItem,
                item_index: 0,
            })
        ));

        let auto_pot_item_mp = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"setAutoPotItem","grid":"heroMpItem","itemIndex":12}"#,
        )
        .expect("set auto pot item (mp) command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(auto_pot_item_mp).expect("auto pot item mp maps"),
            SessionAction::Packet(ClientPacket::SetAutoPotItem {
                grid: MirGridType::HeroMpItem,
                item_index: 12,
            })
        ));

        // Unknown grids are rejected (only hero auto-pot grids are valid here).
        let bad_grid = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"setAutoPotItem","grid":"inventory","itemIndex":0}"#,
        )
        .expect("command should deserialize");
        assert!(super::browser_command_to_action(bad_grid).is_err());

        let a_mode = serde_json::from_str::<BrowserCommand>(r#"{"type":"changeAMode","mode":4}"#)
            .expect("change a mode command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(a_mode).expect("a mode maps"),
            SessionAction::Packet(ClientPacket::ChangeAMode { mode: 4 })
        ));

        let p_mode = serde_json::from_str::<BrowserCommand>(r#"{"type":"changePMode","mode":1}"#)
            .expect("change p mode command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(p_mode).expect("p mode maps"),
            SessionAction::Packet(ClientPacket::ChangePMode { mode: 1 })
        ));

        let open_door =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"openDoor","doorIndex":3}"#)
                .expect("open door command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(open_door).expect("open door maps"),
            SessionAction::Packet(ClientPacket::OpenDoor { door_index: 3 })
        ));
    }

    #[test]
    fn hero_packets_are_exposed_as_browser_events() {
        let new_hero = super::server_packet_to_event(&ServerPacket::NewHero { result: 0 });
        assert_eq!(new_hero["packet"], "NewHero");
        assert_eq!(new_hero["payload"]["result"], 0);

        let hero_info = super::server_packet_to_event(&ServerPacket::NewHeroInfo {
            info: sample_hero_info(0),
            storage_index: -1,
        });
        assert_eq!(hero_info["packet"], "NewHeroInfo");
        assert_eq!(hero_info["payload"]["info"]["name"], "Hero0");
        assert_eq!(hero_info["payload"]["storageIndex"], -1);

        let take_back = super::server_packet_to_event(&ServerPacket::TakeBackHeroItem {
            from: 3,
            to: 1,
            success: false,
        });
        assert_eq!(take_back["packet"], "TakeBackHeroItem");
        assert_eq!(take_back["payload"]["success"], false);

        let create = super::server_packet_to_event(&ServerPacket::HeroCreateRequest {
            can_create_class: vec![true, true, true, false, false],
        });
        assert_eq!(create["packet"], "HeroCreateRequest");
        assert_eq!(create["payload"]["canCreateClass"][0], true);

        let manage = super::server_packet_to_event(&ServerPacket::ManageHeroes {
            maximum_count: 3,
            current_hero: Some(sample_hero_info(0)),
            heroes: Some(vec![Some(sample_hero_info(1)), None]),
        });
        assert_eq!(manage["packet"], "ManageHeroes");
        assert_eq!(manage["payload"]["maximumCount"], 3);
        assert_eq!(manage["payload"]["currentHero"]["name"], "Hero0");

        let behaviour =
            super::server_packet_to_event(&ServerPacket::SetHeroBehaviour { behaviour: 2 });
        assert_eq!(behaviour["packet"], "SetHeroBehaviour");
        assert_eq!(behaviour["payload"]["behaviour"], 2);

        let change = super::server_packet_to_event(&ServerPacket::ChangeHero { from_index: 1 });
        assert_eq!(change["packet"], "ChangeHero");
        assert_eq!(change["payload"]["fromIndex"], 1);

        let spawn_state =
            super::server_packet_to_event(&ServerPacket::UpdateHeroSpawnState { state: 3 });
        assert_eq!(spawn_state["packet"], "UpdateHeroSpawnState");
        assert_eq!(spawn_state["payload"]["state"], 3);

        let gain =
            super::server_packet_to_event(&ServerPacket::GainHeroExperience { amount: 1_500 });
        assert_eq!(gain["packet"], "GainHeroExperience");
        assert_eq!(gain["payload"]["amount"], 1_500);

        let level = super::server_packet_to_event(&ServerPacket::HeroLevelChanged {
            level: 42,
            experience: 12_345,
            max_experience: 20_000,
        });
        assert_eq!(level["packet"], "HeroLevelChanged");
        assert_eq!(level["payload"]["level"], 42);
        assert_eq!(level["payload"]["experience"], 12_345);
        assert_eq!(level["payload"]["maxExperience"], 20_000);
    }

    #[test]
    fn market_relationship_commands_map_to_crystal_protocol_packets() {
        let consign = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"consignItem","uniqueId":77,"price":1500,"marketType":0}"#,
        )
        .expect("consign command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(consign).expect("consign maps"),
            SessionAction::Packet(ClientPacket::ConsignItem {
                unique_id: 77,
                price: 1_500,
                market_type: 0
            })
        ));

        let search = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"marketSearch","matchText":"blade","itemType":5,"userMode":true,"minShape":1,"maxShape":99,"marketType":0}"#,
        )
        .expect("market search command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(search).expect("search maps"),
            SessionAction::Packet(ClientPacket::MarketSearch {
                ref match_text,
                item_type: 5,
                user_mode: true,
                min_shape: 1,
                max_shape: 99,
                market_type: 0
            }) if match_text == "blade"
        ));

        let buy = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"marketBuy","auctionId":88,"bidPrice":2000}"#,
        )
        .expect("market buy command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(buy).expect("buy maps"),
            SessionAction::Packet(ClientPacket::MarketBuy {
                auction_id: 88,
                bid_price: 2_000
            })
        ));

        let marriage = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"marriageReply","acceptInvite":true}"#,
        )
        .expect("marriage reply command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(marriage).expect("marriage maps"),
            SessionAction::Packet(ClientPacket::MarriageReply {
                accept_invite: true
            })
        ));

        let mentor =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"addMentor","name":"Master"}"#)
                .expect("add mentor command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(mentor).expect("mentor maps"),
            SessionAction::Packet(ClientPacket::AddMentor { ref name }) if name == "Master"
        ));
    }

    #[test]
    fn market_relationship_packets_are_exposed_as_browser_events() {
        let consign = super::server_packet_to_event(&ServerPacket::ConsignItem {
            unique_id: 77,
            success: false,
        });
        assert_eq!(consign["packet"], "ConsignItem");
        assert_eq!(consign["payload"]["success"], false);

        let fail = super::server_packet_to_event(&ServerPacket::MarketFail { reason: 1 });
        assert_eq!(fail["packet"], "MarketFail");
        assert_eq!(fail["payload"]["reason"], 1);

        let success = super::server_packet_to_event(&ServerPacket::MarketSuccess {
            message: "Listed".to_string(),
        });
        assert_eq!(success["packet"], "MarketSuccess");
        assert_eq!(success["payload"]["message"], "Listed");

        let marriage = super::server_packet_to_event(&ServerPacket::MarriageRequest {
            name: "Partner".to_string(),
        });
        assert_eq!(marriage["packet"], "MarriageRequest");
        assert_eq!(marriage["payload"]["name"], "Partner");

        let mentor = super::server_packet_to_event(&ServerPacket::MentorRequest {
            name: "Master".to_string(),
            level: 45,
        });
        assert_eq!(mentor["packet"], "MentorRequest");
        assert_eq!(mentor["payload"]["level"], 45);
    }

    #[test]
    fn fishing_commands_map_to_crystal_protocol_packets() {
        let cast_command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"fishingCast","castOut":true}"#)
                .expect("fishing cast command should deserialize");
        let action = super::browser_command_to_action(cast_command)
            .expect("fishing cast command should map to a session action");
        assert!(matches!(
            action,
            SessionAction::Packet(ClientPacket::FishingCast { cast_out: true })
        ));

        let autocast_command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"fishingChangeAutocast","autoCast":true}"#,
        )
        .expect("fishing autocast command should deserialize");
        let action = super::browser_command_to_action(autocast_command)
            .expect("fishing autocast command should map to a session action");
        assert!(matches!(
            action,
            SessionAction::Packet(ClientPacket::FishingChangeAutocast { auto_cast: true })
        ));
    }

    #[test]
    fn trade_commands_map_to_crystal_protocol_packets() {
        let deposit = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"depositTradeItem","from":4,"to":0}"#,
        )
        .expect("deposit trade command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(deposit).expect("deposit trade maps"),
            SessionAction::Packet(ClientPacket::DepositTradeItem { from: 4, to: 0 })
        ));

        let reply =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeReply","acceptInvite":true}"#)
                .expect("trade reply command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(reply).expect("trade reply maps"),
            SessionAction::Packet(ClientPacket::TradeReply {
                accept_invite: true
            })
        ));

        let gold = serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeGold","amount":100}"#)
            .expect("trade gold command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(gold).expect("trade gold maps"),
            SessionAction::Packet(ClientPacket::TradeGold { amount: 100 })
        ));

        let confirm =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeConfirm","locked":true}"#)
                .expect("trade confirm command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(confirm).expect("trade confirm maps"),
            SessionAction::Packet(ClientPacket::TradeConfirm { locked: true })
        ));

        let cancel = serde_json::from_str::<BrowserCommand>(r#"{"type":"tradeCancel"}"#)
            .expect("trade cancel command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(cancel).expect("trade cancel maps"),
            SessionAction::Packet(ClientPacket::TradeCancel)
        ));
    }

    #[test]
    fn trade_packets_are_exposed_as_browser_events() {
        let request = super::server_packet_to_event(&ServerPacket::TradeRequest {
            name: "Scout".to_string(),
        });
        assert_eq!(request["packet"], "TradeRequest");
        assert_eq!(request["payload"]["name"], "Scout");
        // Contract: partner name is hoisted onto `partnerName`.
        assert_eq!(request["payload"]["partnerName"], "Scout");

        let accept = super::server_packet_to_event(&ServerPacket::TradeAccept {
            name: "Scout".to_string(),
        });
        assert_eq!(accept["packet"], "TradeAccept");
        assert_eq!(accept["payload"]["partnerName"], "Scout");

        let gold = super::server_packet_to_event(&ServerPacket::TradeGold { amount: 100 });
        assert_eq!(gold["packet"], "TradeGold");
        assert_eq!(gold["payload"]["amount"], 100);
        // Contract: gold is the partner's running offer.
        assert_eq!(gold["payload"]["partnerGold"], 100);

        // Contract: `partnerItems` resolves item_index -> { name, count, grade }.
        // `sample_user_item` uses item_index 321 = "MediumArmour(M)" (grade 2).
        let trade_item = super::server_packet_to_event(&ServerPacket::TradeItem {
            trade_items: vec![
                Some(sample_user_item(7, 3)),
                None,
                Some(sample_user_item(8, 1)),
            ],
        });
        assert_eq!(trade_item["packet"], "TradeItem");
        // Raw array retained for backward compatibility.
        assert!(trade_item["payload"]["tradeItems"].is_array());
        let partner_items = &trade_item["payload"]["partnerItems"];
        assert!(partner_items.is_array());
        // The empty slot is dropped; only the two real items remain.
        assert_eq!(partner_items.as_array().unwrap().len(), 2);
        assert_eq!(partner_items[0]["name"], "MediumArmour(M)");
        assert_eq!(partner_items[0]["count"], 3);
        assert_eq!(partner_items[0]["grade"], 2);

        let cancel = super::server_packet_to_event(&ServerPacket::TradeCancel { unlock: true });
        assert_eq!(cancel["packet"], "TradeCancel");
        assert_eq!(cancel["payload"]["unlock"], true);

        let confirm = super::server_packet_to_event(&ServerPacket::TradeConfirm);
        assert_eq!(confirm["packet"], "TradeConfirm");
        assert!(confirm["payload"].is_object());
    }

    #[test]
    fn market_packets_expose_contract_listing_fields() {
        // Build a ClientAuction whose item is index 321 = "MediumArmour(M)"
        // (grade 2, RequiredType.Level, RequiredAmount 16).
        let auction = ClientAuction {
            auction_id: 42,
            item: sample_user_item(99, 1),
            seller: "Merchant".to_string(),
            price: 1500,
            consignment_date_binary_datetime: 638_000_000_000_000_000,
            // MarketItemType.Auction = 2.
            item_type: 2,
        };
        let market = super::server_packet_to_event(&ServerPacket::NPCMarket {
            listings: vec![auction.clone()],
            pages: 3,
            user_mode: true,
        });
        assert_eq!(market["packet"], "NPCMarket");
        // Raw listings + page metadata retained for backward compatibility.
        assert!(market["payload"]["listings"].is_array());
        assert_eq!(market["payload"]["pages"], 3);
        assert_eq!(market["payload"]["userMode"], true);
        // Contract-shaped listing array.
        let listing = &market["payload"]["auctions"][0];
        assert_eq!(listing["id"], 42);
        assert_eq!(listing["itemName"], "MediumArmour(M)");
        assert_eq!(listing["seller"], "Merchant");
        assert_eq!(listing["price"], 1500);
        assert_eq!(listing["type"], 2);
        assert_eq!(listing["level"], 16);
        assert_eq!(listing["auction"], true);
        assert_eq!(listing["expiry"], 638_000_000_000_000_000i64);

        // The page variant carries the same enriched listing array.
        let page = super::server_packet_to_event(&ServerPacket::NPCMarketPage {
            listings: vec![auction],
        });
        assert_eq!(page["packet"], "NPCMarketPage");
        assert_eq!(
            page["payload"]["auctions"][0]["itemName"],
            "MediumArmour(M)"
        );
        // A non-auction (consign) listing reports auction=false.
        let consign = super::server_packet_to_event(&ServerPacket::NPCMarketPage {
            listings: vec![ClientAuction {
                auction_id: 7,
                item: sample_user_item(1, 1),
                seller: "For Sale".to_string(),
                price: 50,
                consignment_date_binary_datetime: 0,
                item_type: 1,
            }],
        });
        assert_eq!(consign["payload"]["auctions"][0]["auction"], false);
        // A zero consignment date is omitted (Crystal DateTime.MinValue).
        assert!(consign["payload"]["auctions"][0].get("expiry").is_none());
    }

    #[test]
    fn mail_friend_commands_map_to_crystal_protocol_packets() {
        let send = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"sendMail","name":"Blade","message":"Delivery","gold":2500,"itemsIdx":[7,8,0,0,0],"stamped":true}"#,
        )
        .expect("send mail command should deserialize");
        match super::browser_command_to_action(send).expect("send mail maps") {
            SessionAction::Packet(ClientPacket::SendMail {
                name,
                message,
                gold,
                items_idx,
                stamped,
            }) => {
                assert_eq!(name, "Blade");
                assert_eq!(message, "Delivery");
                assert_eq!(gold, 2_500);
                assert_eq!(items_idx, [7, 8, 0, 0, 0]);
                assert!(stamped);
            }
            other => panic!("unexpected action: {other:?}"),
        }

        let lock = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"mailLockedItem","uniqueId":7,"locked":true}"#,
        )
        .expect("mail locked item command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(lock).expect("mail lock maps"),
            SessionAction::Packet(ClientPacket::MailLockedItem {
                unique_id: 7,
                locked: true
            })
        ));

        let cost = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"mailCost","gold":2500,"itemsIdx":[7,8,0,0,0],"stamped":false}"#,
        )
        .expect("mail cost command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(cost).expect("mail cost maps"),
            SessionAction::Packet(ClientPacket::MailCost {
                gold: 2_500,
                items_idx: [7, 8, 0, 0, 0],
                stamped: false
            })
        ));

        let friend = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"addFriend","name":"Blade","blocked":false}"#,
        )
        .expect("add friend command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(friend).expect("add friend maps"),
            SessionAction::Packet(ClientPacket::AddFriend {
                ref name,
                blocked: false
            }) if name == "Blade"
        ));

        let memo = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"addMemo","characterIndex":42,"memo":"party lead"}"#,
        )
        .expect("add memo command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(memo).expect("add memo maps"),
            SessionAction::Packet(ClientPacket::AddMemo {
                character_index: 42,
                ref memo
            }) if memo == "party lead"
        ));

        let ranking = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"getRanking","rankType":3,"rankIndex":20,"onlineOnly":true}"#,
        )
        .expect("get ranking command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(ranking).expect("ranking maps"),
            SessionAction::Packet(ClientPacket::GetRanking {
                rank_type: 3,
                rank_index: 20,
                online_only: true,
            })
        ));
    }

    #[test]
    fn mail_friend_packets_are_exposed_as_browser_events() {
        let receive = super::server_packet_to_event(&ServerPacket::ReceiveMail {
            mail: vec![ClientMail {
                mail_id: 11,
                sender_name: "GM".to_string(),
                message: "Welcome".to_string(),
                opened: false,
                locked: true,
                can_reply: false,
                collected: false,
                date_sent_binary_datetime: 638000000000000000,
                gold: 2_500,
                items: vec![sample_user_item(7, 1)],
            }],
        });
        assert_eq!(receive["packet"], "ReceiveMail");
        assert_eq!(receive["payload"]["mail"][0]["senderName"], "GM");
        assert_eq!(receive["payload"]["mail"][0]["items"][0]["unique_id"], 7);

        let sent = super::server_packet_to_event(&ServerPacket::MailSent { result: -1 });
        assert_eq!(sent["packet"], "MailSent");
        assert_eq!(sent["payload"]["result"], -1);

        let cost = super::server_packet_to_event(&ServerPacket::MailCost { cost: 200 });
        assert_eq!(cost["packet"], "MailCost");
        assert_eq!(cost["payload"]["cost"], 200);

        let friends = super::server_packet_to_event(&ServerPacket::FriendUpdate {
            friends: vec![
                ClientFriend {
                    index: 42,
                    name: "Blade".to_string(),
                    memo: "party lead".to_string(),
                    blocked: false,
                    online: true,
                },
                ClientFriend {
                    index: 43,
                    name: "Griefer".to_string(),
                    memo: "ignored".to_string(),
                    blocked: true,
                    online: false,
                },
            ],
        });
        assert_eq!(friends["packet"], "FriendUpdate");
        // Roster is split: non-blocked entries in `friends`, blocked in `blocked`.
        assert_eq!(friends["payload"]["friends"][0]["name"], "Blade");
        assert_eq!(friends["payload"]["friends"][0]["online"], true);
        assert_eq!(friends["payload"]["friends"][0]["memo"], "party lead");
        assert_eq!(friends["payload"]["friends"].as_array().unwrap().len(), 1);
        assert_eq!(friends["payload"]["blocked"][0]["name"], "Griefer");
        assert_eq!(friends["payload"]["blocked"][0]["memo"], "ignored");
        assert_eq!(friends["payload"]["blocked"].as_array().unwrap().len(), 1);
        // Friends with an empty memo omit the field entirely.
        let no_memo = super::server_packet_to_event(&ServerPacket::FriendUpdate {
            friends: vec![ClientFriend {
                index: 1,
                name: "Plain".to_string(),
                memo: String::new(),
                blocked: false,
                online: true,
            }],
        });
        assert!(no_memo["payload"]["friends"][0].get("memo").is_none());
    }

    #[test]
    fn intelligent_creature_commands_map_to_crystal_protocol_packets() {
        let request = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"requestIntelligentCreatureUpdates","update":true}"#,
        )
        .expect("request intelligent creature command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(request).expect("request maps"),
            SessionAction::Packet(ClientPacket::RequestIntelligentCreatureUpdates { update: true })
        ));

        let pickup = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"intelligentCreaturePickup","mouseMode":true,"location":{"x":330,"y":270}}"#,
        )
        .expect("intelligent creature pickup command should deserialize");
        assert!(matches!(
            super::browser_command_to_action(pickup).expect("pickup maps"),
            SessionAction::Packet(ClientPacket::IntelligentCreaturePickup {
                mouse_mode: true,
                location: Point { x: 330, y: 270 }
            })
        ));

        let update = serde_json::from_value::<BrowserCommand>(serde_json::json!({
            "type": "updateIntelligentCreature",
            "creature": sample_intelligent_creature(0),
            "summonMe": true,
            "unsummonMe": false,
            "releaseMe": false
        }))
        .expect("update intelligent creature command should deserialize");
        match super::browser_command_to_action(update).expect("update maps") {
            SessionAction::Packet(ClientPacket::UpdateIntelligentCreature {
                creature,
                summon_me,
                unsummon_me,
                release_me,
            }) => {
                assert_eq!(creature.custom_name, "Buddy");
                assert!(summon_me);
                assert!(!unsummon_me);
                assert!(!release_me);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn intelligent_creature_packets_are_exposed_as_browser_events() {
        let update = super::server_packet_to_event(&ServerPacket::UpdateIntelligentCreatureList {
            creature_list: vec![sample_intelligent_creature(0)],
            creature_summoned: true,
            summoned_creature_type: 1,
            pearl_count: 3,
        });
        assert_eq!(update["packet"], "UpdateIntelligentCreatureList");
        assert_eq!(update["payload"]["creatureList"][0]["customName"], "Buddy");
        assert_eq!(update["payload"]["creatureSummoned"], true);
        assert_eq!(update["payload"]["pearlCount"], 3);

        let rename = super::server_packet_to_event(&ServerPacket::IntelligentCreatureEnableRename);
        assert_eq!(rename["packet"], "IntelligentCreatureEnableRename");

        let pickup = super::server_packet_to_event(&ServerPacket::IntelligentCreaturePickup {
            object_id: 1_001,
        });
        assert_eq!(pickup["packet"], "IntelligentCreaturePickup");
        assert_eq!(pickup["payload"]["objectId"], 1_001);
    }

    #[test]
    fn mount_and_fishing_packets_are_exposed_as_browser_events() {
        let mount = super::server_packet_to_event(&ServerPacket::MountUpdate {
            object_id: 1_001,
            mount_type: 12,
            riding_mount: true,
        });
        assert_eq!(mount["packet"], "MountUpdate");
        assert_eq!(mount["payload"]["mountType"], 12);
        assert_eq!(mount["payload"]["ridingMount"], true);

        let fishing = super::server_packet_to_event(&ServerPacket::FishingUpdate {
            object_id: 1_001,
            fishing: true,
            progress_percent: 33,
            chance_percent: 12,
            fishing_point: Point { x: 331, y: 270 },
            found_fish: false,
        });
        assert_eq!(fishing["packet"], "FishingUpdate");
        assert_eq!(fishing["payload"]["progressPercent"], 33);
        assert_eq!(fishing["payload"]["fishingPoint"]["x"], 331);
    }

    #[test]
    fn cast_skill_command_accepts_key_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"castSkill","key":"battle-focus"}"#)
                .expect("cast skill command should deserialize");

        match command {
            BrowserCommand::CastSkill { key } => assert_eq!(key, "battle-focus"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn transfer_map_command_accepts_key_field() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"transferMap","key":"starter-east-field-gate"}"#,
        )
        .expect("transfer map command should deserialize");

        match command {
            BrowserCommand::TransferMap { key } => assert_eq!(key, "starter-east-field-gate"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn stage5_command_accepts_action_and_args() {
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"stage5Command","action":"guild.create","args":["Bichon"]}"#,
        )
        .expect("stage5 command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("stage5 command should map to a session action");

        match action {
            SessionAction::Stage5Command { action, args } => {
                assert_eq!(action, "guild.create");
                assert_eq!(args, vec!["Bichon"]);
            }
            other => panic!("unexpected action: {other:?}"),
        }
    }

    #[test]
    fn stage5_damage_equipment_browser_command_returns_dura_changed_event() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let command = serde_json::from_str::<BrowserCommand>(
            r#"{"type":"stage5Command","action":"qa.damageEquipment","args":["weapon","2500"]}"#,
        )
        .expect("stage5 damage equipment command should deserialize");
        let action = super::browser_command_to_action(command)
            .expect("stage5 damage equipment command should map to a session action");

        let responses = super::execute_session_action(&mut session, action, false, false)
            .expect("stage5 damage equipment command should execute");
        let damage_packet = responses
            .iter()
            .find(|packet| matches!(packet, ServerPacket::DuraChanged { .. }))
            .expect("stage5 damage equipment should emit DuraChanged");
        let event = super::server_packet_to_event(damage_packet);

        assert_eq!(event["packet"], "DuraChanged");
        assert_eq!(event["payload"]["uniqueId"], 0);
        assert_eq!(event["payload"]["currentDura"], 0);
    }

    #[test]
    fn production_web_path_rejects_unauthenticated_start_game() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());

        let error = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::StartGame { character_index: 0 }),
            false,
            true,
        )
        .expect_err("production web path should reject unauthenticated StartGame");

        assert!(error.contains("authenticated account is required"));
    }

    #[test]
    fn runtime_tick_defers_after_bootstrap_but_not_player_movement() {
        assert!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::StartGame { character_index: 0 },
            ))
            .is_some()
        );
        assert_eq!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::Walk {
                    direction: MirDirection::Right,
                },
            )),
            Some(std::time::Duration::ZERO)
        );
        assert_eq!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::Run {
                    direction: MirDirection::Right,
                },
            )),
            Some(std::time::Duration::ZERO)
        );
        assert_eq!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::Turn {
                    direction: MirDirection::Right,
                },
            )),
            Some(std::time::Duration::ZERO)
        );
        assert!(
            super::runtime_tick_defer_duration_for_action(&SessionAction::Packet(
                ClientPacket::Chat {
                    message: "hello".to_string(),
                    linked_items: Vec::new(),
                },
            ))
            .is_none()
        );
    }

    #[test]
    fn production_web_path_allows_start_game_after_login_success() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());

        let login_packets = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::Login {
                account_id: "demo".to_string(),
                password: "demo".to_string(),
            }),
            false,
            true,
        )
        .expect("login should execute on production web path");
        let authenticated = super::update_authenticated_state(false, &login_packets);
        assert!(authenticated);

        let start_packets = super::execute_session_action(
            &mut session,
            SessionAction::Packet(ClientPacket::StartGame { character_index: 0 }),
            authenticated,
            true,
        )
        .expect("authenticated StartGame should execute");

        assert!(start_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { .. })));
        assert!(start_packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::UserInformation { .. })));
        assert!(session.active_identity().is_some());
        assert!(session
            .world_snapshot()
            .entities
            .iter()
            .any(|entity| entity.kind == mir2_simulation::WorldEntityKind::SelfPlayer));
    }

    #[test]
    fn capacity_state_rejects_websocket_connections_over_limit() {
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            Some(1),
            None,
            None,
        ));
        let permit = capacity
            .try_acquire_ws_connection()
            .expect("first websocket should fit capacity");

        let error = capacity
            .try_acquire_ws_connection()
            .expect_err("second websocket should be rejected");
        assert!(error.contains("WebSocket connection capacity reached"));
        assert_eq!(capacity.status().current_ws_connections, 1);

        drop(permit);
        assert_eq!(capacity.status().current_ws_connections, 0);
    }

    #[test]
    fn capacity_state_tracks_active_sessions_and_reconnect_leases() {
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            None,
            Some(1),
            Some(1),
        ));
        let active = capacity
            .try_acquire_active_session()
            .expect("first active session should fit capacity");
        let reconnect = capacity
            .try_acquire_reconnect_lease()
            .expect("first reconnect lease should fit capacity");

        assert!(capacity.try_acquire_active_session().is_err());
        assert!(capacity.try_acquire_reconnect_lease().is_err());
        assert_eq!(capacity.status().current_active_sessions, 1);
        assert_eq!(capacity.status().current_reconnect_leases, 1);

        drop(active);
        drop(reconnect);
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
    }

    #[test]
    fn capacity_state_tracks_account_command_inflight_limits() {
        let capacity = Arc::new(super::GatewayCapacityState::with_action_limits(
            Some(1),
            Some(1),
            Some(1),
        ));
        let login = capacity
            .try_acquire_action(super::GatewayCapacityKind::Login)
            .expect("first login should fit capacity");
        let new_character = capacity
            .try_acquire_action(super::GatewayCapacityKind::NewCharacter)
            .expect("first new-character should fit capacity");
        let start_game = capacity
            .try_acquire_action(super::GatewayCapacityKind::StartGame)
            .expect("first StartGame should fit capacity");

        assert!(capacity
            .try_acquire_action(super::GatewayCapacityKind::Login)
            .expect_err("second login should be rejected")
            .contains("login in-flight capacity reached"));
        assert!(capacity
            .try_acquire_action(super::GatewayCapacityKind::NewCharacter)
            .expect_err("second new-character should be rejected")
            .contains("new-character in-flight capacity reached"));
        assert!(capacity
            .try_acquire_action(super::GatewayCapacityKind::StartGame)
            .expect_err("second StartGame should be rejected")
            .contains("StartGame in-flight capacity reached"));
        assert_eq!(capacity.status().current_login_in_flight, 1);
        assert_eq!(capacity.status().current_new_character_in_flight, 1);
        assert_eq!(capacity.status().current_start_game_in_flight, 1);

        drop(login);
        drop(new_character);
        drop(start_game);
        assert_eq!(capacity.status().current_login_in_flight, 0);
        assert_eq!(capacity.status().current_new_character_in_flight, 0);
        assert_eq!(capacity.status().current_start_game_in_flight, 0);
    }

    #[test]
    fn web_session_save_queue_debounces_until_due_or_limit() {
        let start = std::time::Instant::now();
        let mut saves = 0;
        let mut queue = super::WebSessionSaveQueue::new(super::GatewaySaveQueueConfig::new(
            std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(30),
            3,
        ));

        queue
            .request_save(start, || {
                saves += 1;
                Ok(())
            })
            .expect("first save request should queue");
        queue
            .request_save(start + std::time::Duration::from_secs(1), || {
                saves += 1;
                Ok(())
            })
            .expect("second save request should still debounce");
        assert_eq!(saves, 0);
        assert!(queue.has_pending_save());

        queue
            .request_save(start + std::time::Duration::from_secs(2), || {
                saves += 1;
                Ok(())
            })
            .expect("queue limit should flush");
        assert_eq!(saves, 1);
        assert!(!queue.has_pending_save());
    }

    #[test]
    fn web_session_save_queue_flushes_on_checkpoint_and_close() {
        let start = std::time::Instant::now();
        let mut saves = 0;
        let mut queue = super::WebSessionSaveQueue::new(super::GatewaySaveQueueConfig::new(
            std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(20),
            8,
        ));

        queue
            .request_save(start, || {
                saves += 1;
                Ok(())
            })
            .expect("save request should queue");
        queue
            .checkpoint(start + std::time::Duration::from_secs(10), || {
                saves += 1;
                Ok(())
            })
            .expect("early checkpoint should not flush");
        assert_eq!(saves, 0);

        queue
            .checkpoint(start + std::time::Duration::from_secs(21), || {
                saves += 1;
                Ok(())
            })
            .expect("due checkpoint should flush");
        assert_eq!(saves, 1);

        queue
            .request_save(start + std::time::Duration::from_secs(21), || {
                saves += 1;
                Ok(())
            })
            .expect("save request should queue again");
        queue
            .flush_now(start + std::time::Duration::from_secs(22), || {
                saves += 1;
                Ok(())
            })
            .expect("close flush should persist dirty state");
        assert_eq!(saves, 2);
        assert!(!queue.has_pending_save());

        queue
            .force_save_now(start + std::time::Duration::from_secs(23), || {
                saves += 1;
                Ok(())
            })
            .expect("forced close save should run even without dirty state");
        assert_eq!(saves, 3);
    }

    #[test]
    fn web_session_route_refresh_throttles_low_latency_updates() {
        let start = Instant::now();
        let cache = crate::InMemoryGatewaySessionCache::default();
        let session = demo_game_session();
        let mut refresh = super::WebSessionRouteRefresh::new(
            super::GatewayRouteRefreshConfig::new(Duration::from_secs(5)),
        );

        assert!(refresh
            .maybe_refresh(&cache, &session, start, false)
            .expect("initial active session refresh should succeed"));
        assert_eq!(crate::GatewaySessionCache::route_lease_count(&cache), 1);
        assert!(!refresh
            .maybe_refresh(&cache, &session, start + Duration::from_secs(1), false)
            .expect("early refresh should be skipped"));
        assert!(refresh
            .maybe_refresh(&cache, &session, start + Duration::from_secs(1), true)
            .expect("forced refresh should bypass throttle"));
        assert!(!refresh
            .maybe_refresh(&cache, &session, start + Duration::from_secs(3), false)
            .expect("throttle should use the latest forced refresh time"));
        assert!(refresh
            .maybe_refresh(&cache, &session, start + Duration::from_secs(7), false)
            .expect("refresh should run again after the interval"));
    }

    #[test]
    fn reconnect_session_store_restores_active_session_before_grace_expires() {
        let store = super::ReconnectSessionStore::default();
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            None,
            Some(1),
            Some(1),
        ));
        let session = demo_game_session();
        let session_id = session.session_id().to_string();
        let key = super::session_cache_key(&session)
            .expect("active demo game session should have a cache key");
        let active_session_permit = capacity
            .try_acquire_active_session()
            .expect("test active session should fit capacity");
        let reconnect_lease_permit = capacity
            .try_acquire_reconnect_lease()
            .expect("test reconnect lease should fit capacity");

        store.store(
            key.clone(),
            session,
            Some(active_session_permit),
            reconnect_lease_permit,
            std::time::Duration::from_secs(30),
        );
        assert_eq!(store.len(), 1);
        assert_eq!(capacity.status().current_active_sessions, 1);
        assert_eq!(capacity.status().current_reconnect_leases, 1);

        let restored = store
            .take(&key)
            .expect("stored reconnect session should be restored within grace");
        assert_eq!(restored.session.session_id(), session_id);
        assert_eq!(
            restored
                .session
                .active_identity()
                .expect("restored session should remain active")
                .account_id,
            "demo"
        );
        assert!(restored.active_session_permit.is_some());
        assert_eq!(store.len(), 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);

        drop(restored);
        assert_eq!(capacity.status().current_active_sessions, 0);
    }

    #[test]
    fn reconnect_session_store_discards_expired_sessions() {
        let store = super::ReconnectSessionStore::default();
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            None,
            Some(1),
            Some(1),
        ));
        let session = demo_game_session();
        let key = super::session_cache_key(&session)
            .expect("active demo game session should have a cache key");
        let active_session_permit = capacity
            .try_acquire_active_session()
            .expect("test active session should fit capacity");
        let reconnect_lease_permit = capacity
            .try_acquire_reconnect_lease()
            .expect("test reconnect lease should fit capacity");
        {
            let mut sessions = store
                .sessions
                .lock()
                .expect("test reconnect store mutex should not be poisoned");
            sessions.insert(
                key.clone(),
                super::ReconnectSessionLease {
                    session,
                    active_session_permit: Some(active_session_permit),
                    _reconnect_lease_permit: reconnect_lease_permit,
                    expires_at: std::time::Instant::now() - std::time::Duration::from_secs(1),
                },
            );
        }
        assert_eq!(capacity.status().current_active_sessions, 1);
        assert_eq!(capacity.status().current_reconnect_leases, 1);

        assert!(store.take(&key).is_none());
        assert_eq!(store.len(), 0);
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
    }

    #[tokio::test]
    async fn reconnect_session_store_scheduled_purge_discards_expired_sessions() {
        let store = Arc::new(super::ReconnectSessionStore::default());
        let capacity = Arc::new(super::GatewayCapacityState::with_limits(
            None,
            Some(1),
            Some(1),
        ));
        let session = demo_game_session();
        let key = super::session_cache_key(&session)
            .expect("active demo game session should have a cache key");
        let active_session_permit = capacity
            .try_acquire_active_session()
            .expect("test active session should fit capacity");
        let reconnect_lease_permit = capacity
            .try_acquire_reconnect_lease()
            .expect("test reconnect lease should fit capacity");

        store.store(
            key,
            session,
            Some(active_session_permit),
            reconnect_lease_permit,
            std::time::Duration::from_millis(10),
        );
        super::schedule_reconnect_session_purge(
            Arc::clone(&store),
            std::time::Duration::from_millis(10),
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert_eq!(store.len(), 0);
        assert_eq!(capacity.status().current_active_sessions, 0);
        assert_eq!(capacity.status().current_reconnect_leases, 0);
    }

    #[test]
    fn reconnect_resume_key_helpers_match_login_and_start_game_actions() {
        let login = SessionAction::Packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        assert_eq!(super::login_account_id_for_action(&login), Some("demo"));

        let passkey = SessionAction::PasskeyLogin {
            account_id: "wallet-demo".to_string(),
            token: "token".to_string(),
        };
        assert_eq!(
            super::login_account_id_for_action(&passkey),
            Some("wallet-demo")
        );

        let start = SessionAction::Packet(ClientPacket::StartGame { character_index: 2 });
        assert_eq!(
            super::start_game_character_index_for_action(&start),
            Some(2)
        );
    }

    #[test]
    fn start_game_route_lease_blocks_duplicate_online_character_before_world_entry() {
        let cache = crate::InMemoryGatewaySessionCache::default();
        let first = crate::GatewaySession::new(SimulationConfig::default());
        let second = crate::GatewaySession::new(SimulationConfig::default());

        let first_key =
            super::try_acquire_start_game_route_lease(&cache, &first, true, Some("demo"), Some(0))
                .expect("first StartGame route lease should succeed")
                .expect("StartGame should acquire a pending route lease");
        let second_error =
            super::try_acquire_start_game_route_lease(&cache, &second, true, Some("demo"), Some(0))
                .expect_err("second StartGame route lease should be rejected");

        assert!(second_error.contains("already online"));
        assert_eq!(first_key.account_id, "demo");
        assert_eq!(crate::GatewaySessionCache::route_lease_count(&cache), 1);

        super::release_pending_start_game_route_lease(&cache, &first, Some(&first_key));
        assert_eq!(crate::GatewaySessionCache::route_lease_count(&cache), 0);
        assert!(super::try_acquire_start_game_route_lease(
            &cache,
            &second,
            true,
            Some("demo"),
            Some(0),
        )
        .expect("second StartGame route lease should succeed after release")
        .is_some());
    }

    #[test]
    fn start_game_route_lease_releases_when_start_game_does_not_claim_identity() {
        let cache = crate::InMemoryGatewaySessionCache::default();
        let session = crate::GatewaySession::new(SimulationConfig::default());
        let key = super::try_acquire_start_game_route_lease(
            &cache,
            &session,
            true,
            Some("demo"),
            Some(0),
        )
        .expect("pending route lease should succeed")
        .expect("StartGame should acquire a pending route lease");

        super::release_unclaimed_start_game_route_lease(&cache, &session, Some(&key));

        assert_eq!(crate::GatewaySessionCache::route_lease_count(&cache), 0);
    }

    #[test]
    fn production_web_path_rejects_debug_runtime_commands() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());

        let move_error = super::execute_session_action(
            &mut session,
            SessionAction::MoveTo {
                x: 330,
                y: 270,
                running: false,
            },
            true,
            true,
        )
        .expect_err("production web path should reject MoveTo");
        assert!(move_error.contains("debug MoveTo"));

        let stage5_error = super::execute_session_action(
            &mut session,
            SessionAction::Stage5Command {
                action: "qa.giveItem".to_string(),
                args: vec!["red-potion".to_string()],
            },
            true,
            true,
        )
        .expect_err("production web path should reject Stage5Command");
        assert!(stage5_error.contains("Stage5Command"));

        let transfer_error = super::execute_session_action(
            &mut session,
            SessionAction::TransferMap {
                key: "crystal:0:330:270".to_string(),
            },
            true,
            true,
        )
        .expect_err("production web path should reject debug crystal transfer");
        assert!(transfer_error.contains("debug crystal transfer"));
    }

    #[test]
    fn stage5_damage_equipment_still_works_after_storage_password_flow() {
        let mut session = crate::GatewaySession::new(SimulationConfig::default());
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session.handle_packet(ClientPacket::EquipItem {
            unique_id: 4,
            grid: MirGridType::Inventory,
            to: 0,
        });
        session.transfer_map("crystal:0:317:260");
        session.stage5_command("qa.openStorage", Vec::new());
        session.handle_packet(ClientPacket::SetStoragePassword {
            current_password: String::new(),
            new_password: "Safe123".to_string(),
        });
        session.handle_packet(ClientPacket::UnlockStorage {
            password: "Safe123".to_string(),
        });
        session.handle_packet(ClientPacket::SetStoragePassword {
            current_password: "Safe123".to_string(),
            new_password: "Vault123".to_string(),
        });
        session.handle_packet(ClientPacket::RemoveStoragePassword {
            current_password: "Vault123".to_string(),
        });

        let responses =
            session.stage5_command("qa.damageEquipment", vec!["weapon".into(), "2500".into()]);
        let snapshot = session.world_snapshot();
        let weapon = snapshot
            .equipment_items
            .iter()
            .find(|item| item.slot == mir2_simulation::EquipmentSlot::Weapon)
            .expect("weapon should remain equipped");

        assert_eq!(weapon.name, "Dagger");
        assert_eq!(weapon.durability_current, 0);
        assert!(responses.iter().any(|packet| matches!(
            packet,
            ServerPacket::DuraChanged {
                unique_id: 0,
                current_dura: 0
            }
        )));
    }

    #[test]
    fn select_npc_dialog_command_accepts_target_field() {
        let command =
            serde_json::from_str::<BrowserCommand>(r#"{"type":"selectNpcDialog","target":"@Buy"}"#)
                .expect("select npc dialog command should deserialize");

        match command {
            BrowserCommand::SelectNpcDialog { target } => assert_eq!(target, "@Buy"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn move_actions_skip_snapshot_without_visibility_changes() {
        let action = SessionAction::MoveTo {
            x: 10,
            y: 20,
            running: false,
        };

        assert!(!should_send_world_snapshot_for_action(&action));
    }

    #[test]
    fn movement_packets_skip_snapshot_by_default() {
        let action = SessionAction::Packet(ClientPacket::Walk {
            direction: mir2_protocol::MirDirection::Up,
        });

        assert!(!should_send_world_snapshot_for_action(&action));
    }

    #[test]
    fn visibility_packets_force_snapshot() {
        let responses = vec![ServerPacket::ObjectShow { object_id: 42 }];

        assert!(responses_require_world_snapshot(&responses));
    }
}
