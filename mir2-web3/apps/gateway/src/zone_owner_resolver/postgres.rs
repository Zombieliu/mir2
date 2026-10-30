//! Least-privilege PostgreSQL observation, separate from the lease writer.
//!
//! One shared observer owns one OS worker and one connection. Queue admission is
//! nonblocking; every caller has a deadline and expired queued work is skipped.
//! A failed request is never retried. A later explicit request can reconnect
//! once on that same worker and must pass the privilege audit again. There are
//! no background retries; requests during a short reconnect cooldown fail fast.
//! PostgreSQL statement/lock timeouts bound server work, but the synchronous
//! driver's connect timeout only bounds socket connection attempts, not every
//! authentication/network stall. A stalled worker can outlive caller deadlines
//! (and the last observer), and cannot recover until that IO returns. Its queue,
//! worker and connection counts remain bounded; no replacement worker is spawned.
//! Configure one observer at startup and share its clones.
//!
//! The worker also owns destruction of the synchronous client, so dropping an
//! observer from a Tokio task cannot drop a nested synchronous Tokio runtime.

use super::{ObservedZoneLease, ResolveError, ZoneId, ZoneLeaseObserver};
use ::postgres::{Client, Config, NoTls, Row};
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_DSN_BYTES: usize = 16 * 1024;
const MAX_ZONE_ID_BYTES: usize = 1024;
const MAX_QUEUE_CAPACITY: usize = 256;
const MAX_TIMEOUT: Duration = Duration::from_secs(30);
const RECONNECT_COOLDOWN: Duration = Duration::from_millis(100);
static NEXT_OBSERVER_ID: AtomicU64 = AtomicU64::new(1);

// All object/function names are fixed and schema-qualified. No writer authority
// is constructed here: its observation path also performs migrations.
const OBSERVE_SQL: &str = "SELECT owner_id, fencing_token, expires_at_ms, heartbeat_at_ms \
                          FROM public.zone_owner_leases WHERE zone_id = $1";

// MEMBER intentionally includes NOINHERIT memberships: SET ROLE must not offer
// a route around the reader restriction. Checking each reachable role also
// includes privileges inherited through a role that itself has NOINHERIT.
// ACL helpers include PUBLIC grants. Column grants and sequence write access
// matter too, even when table-level ACLs contain no DML grant.
// PostgreSQL's exact built-in pg_settings view has default PUBLIC UPDATE:
// updating it is the session-GUC SET interface, not persistent game-data DML.
// Exempt only its DML/column ACL checks; ownership and every other relation's
// privileges remain checked. The observer still executes only fixed SELECTs
// inside explicit read-only transactions, never caller-provided SQL or SET.
const PRIVILEGE_AUDIT_SQL: &str = r#"
WITH reachable_roles AS (
    SELECT oid, rolname, rolsuper, rolcreaterole, rolcreatedb,
           rolreplication, rolbypassrls
    FROM pg_catalog.pg_roles
    WHERE pg_catalog.pg_has_role(CURRENT_USER, oid, 'MEMBER')
), target AS (
    SELECT c.oid, c.relkind, c.relrowsecurity
    FROM pg_catalog.pg_class c
    JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = 'public' AND c.relname = 'zone_owner_leases'
)
SELECT CURRENT_USER = SESSION_USER
   AND pg_catalog.current_setting('transaction_read_only') = 'on'
   AND (SELECT count(*) = 1 FROM target WHERE relkind = 'r' AND NOT relrowsecurity)
   AND COALESCE((SELECT pg_catalog.has_table_privilege(CURRENT_USER, oid, 'SELECT')
                 FROM target), false)
   AND NOT EXISTS (
       SELECT 1 FROM reachable_roles r
       WHERE r.rolsuper OR r.rolcreaterole OR r.rolcreatedb
          OR r.rolreplication OR r.rolbypassrls
          OR r.rolname LIKE 'pg\_%' ESCAPE '\'
          OR pg_catalog.has_database_privilege(r.oid, pg_catalog.current_database(), 'CREATE,TEMP')
          OR EXISTS (SELECT 1 FROM pg_catalog.pg_database d WHERE d.datdba = r.oid)
          OR EXISTS (SELECT 1 FROM pg_catalog.pg_namespace n
                     WHERE n.nspowner = r.oid
                        OR pg_catalog.has_schema_privilege(r.oid, n.oid, 'CREATE'))
          OR EXISTS (SELECT 1 FROM pg_catalog.pg_class c
                     WHERE c.relowner = r.oid
                        OR (c.relkind IN ('r', 'p', 'v', 'm', 'f')
                            AND c.oid <> 'pg_catalog.pg_settings'::pg_catalog.regclass AND (
                            pg_catalog.has_table_privilege(r.oid, c.oid,
                                'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER')
                            OR pg_catalog.has_any_column_privilege(r.oid, c.oid,
                                'INSERT,UPDATE,REFERENCES')))
                        OR (c.relkind = 'S' AND
                            pg_catalog.has_sequence_privilege(r.oid, c.oid, 'USAGE,UPDATE')))
          OR EXISTS (SELECT 1 FROM pg_catalog.pg_proc p WHERE p.proowner = r.oid)
   ) AS allowed
"#;

/// Finite limits for this observer, including queueing in the caller budget.
/// Durations must be between one millisecond and thirty seconds. Queue capacity
/// must be 1..=256. A smaller caller deadline always takes precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostgresObserverLimits {
    pub connect_timeout: Duration,
    pub query_timeout: Duration,
    pub lock_timeout: Duration,
    pub request_timeout: Duration,
    pub queue_capacity: usize,
}

impl Default for PostgresObserverLimits {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(1),
            query_timeout: Duration::from_millis(500),
            lock_timeout: Duration::from_millis(100),
            request_timeout: Duration::from_secs(2),
            queue_capacity: 32,
        }
    }
}

impl PostgresObserverLimits {
    fn validate(self) -> Result<Self, ResolveError> {
        if [
            self.connect_timeout,
            self.query_timeout,
            self.lock_timeout,
            self.request_timeout,
        ]
        .into_iter()
        .any(|duration| duration < Duration::from_millis(1) || duration > MAX_TIMEOUT)
            || self.lock_timeout > self.query_timeout
            || self.queue_capacity == 0
            || self.queue_capacity > MAX_QUEUE_CAPACITY
        {
            return Err(ResolveError::ObserverUnavailable);
        }
        Ok(self)
    }
}

type Observation = Result<Option<ObservedZoneLease>, ResolveError>;

struct ObserveRequest {
    zone_id: String,
    deadline: Instant,
    reply: SyncSender<Observation>,
}

struct SharedObserver {
    requests: SyncSender<ObserveRequest>,
    // Worker admission state, not a cached assertion of database availability.
    available: Arc<AtomicBool>,
    limits: PostgresObserverLimits,
    #[cfg(test)]
    application_name: String,
}

/// Explicitly configured reader credentials only; no environment lookup,
/// fallback database, role mutation, schema creation, or lease mutation.
#[derive(Clone)]
pub struct PostgresZoneLeaseObserver {
    shared: Arc<SharedObserver>,
}

impl fmt::Debug for PostgresZoneLeaseObserver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresZoneLeaseObserver")
            .field("limits", &self.shared.limits)
            .field("available", &self.shared.available.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

impl PostgresZoneLeaseObserver {
    /// Connect and audit the actual server-side role before returning. All
    /// failures use a fixed code; neither DSN nor driver diagnostics are exposed.
    /// The DSN must identify a database and user explicitly. Its startup options
    /// are replaced by the observer's fixed safety settings.
    pub fn connect(
        readonly_dsn: &str,
        limits: PostgresObserverLimits,
    ) -> Result<Self, ResolveError> {
        let limits = limits.validate()?;
        let deadline = deadline_after(limits.request_timeout)?;
        let mut config = reader_config(readonly_dsn, limits)?;
        let application_name = format!(
            "mir2-zone-lease-observer-{}-{}",
            std::process::id(),
            NEXT_OBSERVER_ID.fetch_add(1, Ordering::Relaxed),
        );
        config.application_name(&application_name);
        let (requests, receiver) = mpsc::sync_channel(limits.queue_capacity);
        let (initialized, initialization) = mpsc::sync_channel(1);
        let available = Arc::new(AtomicBool::new(false));
        let worker_available = available.clone();
        // No JoinHandle is stored: last-owner drop only disconnects the queue,
        // so it cannot block an async executor while the DB worker is stalled.
        std::thread::Builder::new()
            .name("zone-lease-observer".to_owned())
            .spawn(move || worker(config, receiver, initialized, worker_available, deadline))
            .map_err(|_| ResolveError::ObserverUnavailable)?;
        receive_before(&initialization, deadline)??;
        Ok(Self {
            shared: Arc::new(SharedObserver {
                requests,
                available,
                limits,
                #[cfg(test)]
                application_name,
            }),
        })
    }

    /// The caller may tighten the configured total budget, including time spent
    /// waiting in the bounded queue. A late result is never returned as success.
    pub fn observe_until(&self, zone_id: &ZoneId, deadline: Instant) -> Observation {
        let deadline = deadline.min(deadline_after(self.shared.limits.request_timeout)?);
        if zone_id.as_str().is_empty() || zone_id.as_str().len() > MAX_ZONE_ID_BYTES {
            return Err(ResolveError::InvalidLease);
        }
        if Instant::now() >= deadline || !self.shared.available.load(Ordering::Acquire) {
            return Err(ResolveError::ObserverUnavailable);
        }
        let (reply, response) = mpsc::sync_channel(1);
        enqueue(
            &self.shared.requests,
            ObserveRequest {
                zone_id: zone_id.as_str().to_owned(),
                deadline,
                reply,
            },
        )?;
        receive_before(&response, deadline)?
    }
}

impl ZoneLeaseObserver for PostgresZoneLeaseObserver {
    fn observe(&self, zone_id: &ZoneId) -> Observation {
        self.observe_until(zone_id, deadline_after(self.shared.limits.request_timeout)?)
    }
}

fn deadline_after(timeout: Duration) -> Result<Instant, ResolveError> {
    Instant::now()
        .checked_add(timeout)
        .ok_or(ResolveError::ObserverUnavailable)
}

fn receive_before<T>(receiver: &Receiver<T>, deadline: Instant) -> Result<T, ResolveError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or(ResolveError::ObserverUnavailable)?;
    let result = receiver
        .recv_timeout(remaining)
        .map_err(|_| ResolveError::ObserverUnavailable)?;
    if Instant::now() >= deadline {
        return Err(ResolveError::ObserverUnavailable);
    }
    Ok(result)
}

fn enqueue(
    sender: &SyncSender<ObserveRequest>,
    request: ObserveRequest,
) -> Result<(), ResolveError> {
    if Instant::now() >= request.deadline {
        return Err(ResolveError::ObserverUnavailable);
    }
    sender.try_send(request).map_err(|error| match error {
        TrySendError::Full(_) | TrySendError::Disconnected(_) => ResolveError::ObserverUnavailable,
    })
}

fn reader_config(dsn: &str, limits: PostgresObserverLimits) -> Result<Config, ResolveError> {
    if dsn.is_empty() || dsn.len() > MAX_DSN_BYTES {
        return Err(ResolveError::ObserverUnavailable);
    }
    let mut config: Config = dsn.parse().map_err(|_| ResolveError::ObserverUnavailable)?;
    if config.get_user().is_none_or(str::is_empty)
        || config.get_dbname().is_none_or(str::is_empty)
        || config.get_hosts().len().max(config.get_hostaddrs().len()) != 1
    {
        return Err(ResolveError::ObserverUnavailable);
    }
    // Avoid a hostname expanding to an unbounded sequence of connection
    // attempts. Explicit hostaddr is allowed; otherwise require a literal IP.
    if config.get_hostaddrs().is_empty() {
        match config.get_hosts().first() {
            Some(::postgres::config::Host::Tcp(host))
                if host.parse::<std::net::IpAddr>().is_ok() => {}
            _ => return Err(ResolveError::ObserverUnavailable),
        }
    }
    config
        .connect_timeout(limits.connect_timeout)
        .tcp_user_timeout(limits.request_timeout)
        .keepalives(true)
        .keepalives_idle(Duration::from_secs(1))
        .keepalives_interval(Duration::from_secs(1))
        .keepalives_retries(2)
        .application_name("mir2-zone-lease-observer")
        // The default driver callback logs server-provided notice text.
        .notice_callback(|_| {})
        .options(&format!(
            "-c default_transaction_read_only=on -c search_path=pg_catalog \
             -c statement_timeout={} -c lock_timeout={} \
             -c idle_in_transaction_session_timeout={}",
            limits.query_timeout.as_millis(),
            limits.lock_timeout.as_millis(),
            limits.request_timeout.as_millis(),
        ));
    Ok(config)
}

fn worker(
    config: Config,
    requests: Receiver<ObserveRequest>,
    initialized: SyncSender<Result<(), ResolveError>>,
    available: Arc<AtomicBool>,
    initialization_deadline: Instant,
) {
    let initial_client = match audited_connection(&config, initialization_deadline) {
        Ok(client) => client,
        Err(_) => {
            let _ = initialized.send(Err(ResolveError::ObserverUnavailable));
            return;
        }
    };
    let mut client = Some(initial_client);
    let mut reconnect_not_before = Instant::now();
    available.store(true, Ordering::Release);
    if initialized.send(Ok(())).is_err() {
        available.store(false, Ordering::Release);
        return;
    }
    for request in requests {
        if Instant::now() >= request.deadline {
            let _ = request
                .reply
                .try_send(Err(ResolveError::ObserverUnavailable));
            continue;
        }
        if client.is_none() {
            if !reconnect_permitted(request.deadline, reconnect_not_before) {
                let _ = request
                    .reply
                    .try_send(Err(ResolveError::ObserverUnavailable));
                continue;
            }
            // Exactly one attempt for this explicit request. Never retry the
            // request that detected an IO error, loop in the background, or
            // start another worker if connect/authentication is stalled.
            match audited_connection(&config, request.deadline) {
                Ok(reconnected) => client = Some(reconnected),
                Err(_) => {
                    reconnect_not_before = Instant::now() + RECONNECT_COOLDOWN;
                    let _ = request
                        .reply
                        .try_send(Err(ResolveError::ObserverUnavailable));
                    continue;
                }
            }
        }
        let result = match client.as_mut() {
            Some(client) if Instant::now() < request.deadline => {
                observe_readonly(client, &request.zone_id)
            }
            _ => Err(ResolveError::ObserverUnavailable),
        };
        let connection_failed = matches!(result, Err(ResolveError::ObserverUnavailable));
        let result = if Instant::now() >= request.deadline {
            Err(ResolveError::ObserverUnavailable)
        } else {
            result
        };
        let _ = request.reply.try_send(result);
        if connection_failed {
            reconnect_not_before = Instant::now() + RECONNECT_COOLDOWN;
            // The worker remains available for later requests. Client/runtime
            // destruction, like connection creation, stays on this OS worker.
            drop(client.take());
        }
    }
    available.store(false, Ordering::Release);
    // client and its private runtime are destroyed on this OS worker.
}

fn reconnect_permitted(deadline: Instant, not_before: Instant) -> bool {
    let now = Instant::now();
    now < deadline && now >= not_before
}

fn audited_connection(config: &Config, deadline: Instant) -> Result<Client, ResolveError> {
    if Instant::now() >= deadline {
        return Err(ResolveError::ObserverUnavailable);
    }
    let mut client = config
        .connect(NoTls)
        .map_err(|_| ResolveError::ObserverUnavailable)?;
    if Instant::now() >= deadline {
        return Err(ResolveError::ObserverUnavailable);
    }
    audit_reader(&mut client)?;
    if Instant::now() >= deadline {
        return Err(ResolveError::ObserverUnavailable);
    }
    Ok(client)
}

fn audit_reader(client: &mut Client) -> Result<(), ResolveError> {
    let mut transaction = client
        .build_transaction()
        .read_only(true)
        .start()
        .map_err(|_| ResolveError::ObserverUnavailable)?;
    let row = transaction
        .query_one(PRIVILEGE_AUDIT_SQL, &[])
        .map_err(|_| ResolveError::ObserverUnavailable)?;
    if !row
        .try_get::<_, bool>("allowed")
        .map_err(|_| ResolveError::ObserverUnavailable)?
    {
        return Err(ResolveError::ObserverUnavailable);
    }
    transaction
        .commit()
        .map_err(|_| ResolveError::ObserverUnavailable)
}

fn observe_readonly(client: &mut Client, zone_id: &str) -> Observation {
    let mut transaction = client
        .build_transaction()
        .read_only(true)
        .start()
        .map_err(|_| ResolveError::ObserverUnavailable)?;
    let row = transaction
        .query_opt(OBSERVE_SQL, &[&zone_id])
        .map_err(|_| ResolveError::ObserverUnavailable)?;
    let observed = row.map(decode_lease).transpose()?;
    transaction
        .commit()
        .map_err(|_| ResolveError::ObserverUnavailable)?;
    Ok(observed)
}

fn decode_lease(row: Row) -> Result<ObservedZoneLease, ResolveError> {
    let owner_id = row
        .try_get::<_, String>("owner_id")
        .map_err(|_| ResolveError::InvalidLease)?;
    if !super::valid_owner_id(&owner_id) {
        return Err(ResolveError::InvalidLease);
    }
    let positive = |name| {
        row.try_get::<_, i64>(name)
            .ok()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(ResolveError::InvalidLease)
    };
    let fencing_token = positive("fencing_token")?;
    if fencing_token == 0 {
        return Err(ResolveError::InvalidLease);
    }
    Ok(ObservedZoneLease {
        owner_id,
        fencing_token,
        expires_at_ms: positive("expires_at_ms")?,
        heartbeat_at_ms: positive("heartbeat_at_ms")?,
    })
}

#[cfg(test)]
#[path = "postgres_tests.rs"]
mod tests;
