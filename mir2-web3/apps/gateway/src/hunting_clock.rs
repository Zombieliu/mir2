//! Process-lifetime hunting-permit settlement. Never driven by a player or BI.
use crate::GatewayConfig;
use mir2_economy_policy::Policy;
use mir2_economy_service::{
    export_daily_metrics_with_timeouts, initialize_realm, read_realm, settle_due_with_timeouts,
};
use mir2_simulation::AccountStoreDatabaseMode;
use postgres::{Client, NoTls};
use std::{
    env, fs, io,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct HuntingRuntimeConfig {
    pub realm_id: String,
    pub database_url: String,
}
impl std::fmt::Debug for HuntingRuntimeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HuntingRuntimeConfig")
            .field("realm_id", &self.realm_id)
            .field("database_configured", &true)
            .finish()
    }
}

fn runtime_from_values(
    enabled: Option<&str>,
    realm: Option<&str>,
    database_url: Option<&str>,
    mode: AccountStoreDatabaseMode,
) -> Result<Option<HuntingRuntimeConfig>, String> {
    let enabled = match enabled
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        None | Some("0" | "false") => false,
        Some("1" | "true") => true,
        _ => return Err("MIR2_HUNTING_ECONOMY_ENABLED must be true, false, 1 or 0".into()),
    };
    if !enabled {
        return Ok(None);
    }
    if mode != AccountStoreDatabaseMode::SourceOfTruth {
        return Err("hunting economy requires PostgreSQL SourceOfTruth".into());
    }
    let realm_id = realm
        .filter(|s| {
            !s.trim().is_empty()
                && s.trim() == *s
                && s.len() <= 128
                && !s.chars().any(char::is_control)
        })
        .ok_or("MIR2_REALM_ID is required for hunting economy")?
        .to_owned();
    let database_url = database_url
        .filter(|s| !s.trim().is_empty())
        .ok_or("hunting economy requires the authoritative account PostgreSQL URL")?
        .to_owned();
    Ok(Some(HuntingRuntimeConfig {
        realm_id,
        database_url,
    }))
}

/// Default is disabled. Neither realm nor database authority comes from a client.
pub fn hunting_http_runtime(
    config: &GatewayConfig,
) -> Result<Option<HuntingRuntimeConfig>, String> {
    runtime_from_values(
        env::var("MIR2_HUNTING_ECONOMY_ENABLED").ok().as_deref(),
        env::var("MIR2_REALM_ID").ok().as_deref(),
        config.account_store_database_url.as_deref(),
        config.account_store_database_mode,
    )
}

/// The main game runtime and authenticated conversion adapter share one explicit
/// realm/database binding. Raw command replication cannot carry these source
/// commits yet, so an enabled source must stay on the local authority path.
pub fn configure_hunting_game_sources(config: GatewayConfig) -> Result<GatewayConfig, String> {
    let runtime = hunting_http_runtime(&config)?;
    let config = bind_hunting_game_sources(config, runtime)?;
    ensure_hunting_game_source_topology(&config)?;
    Ok(config)
}

/// Shared with public/programmatic Gateway entry points. Trust the actual
/// runtime capability; reading a disabled environment flag must never erase a
/// source binding supplied by a server caller.
pub(crate) fn ensure_hunting_game_source_topology(config: &GatewayConfig) -> Result<(), String> {
    validate_hunting_source_topology(
        config.hunting_economy_realm.is_some(),
        crate::gate15::health().is_some(),
        env::var("MIR2_GATE15_VALIDATOR_URLS").ok().as_deref(),
        crate::TcpZoneOwnerRpcTransport::configured_endpoints_from_env(
            &crate::routing::ZoneId::primary(),
        )
        .is_some(),
    )
}

fn validate_hunting_source_topology(
    enabled: bool,
    gate15_initialized: bool,
    validator_urls: Option<&str>,
    remote_zone_owner: bool,
) -> Result<(), String> {
    if enabled
        && (gate15_initialized
            || validator_urls.is_some_and(|urls| !urls.trim().is_empty())
            || remote_zone_owner)
    {
        return Err("hunting game sources require local authority; replicated Zone owners need trusted source checkpoints and character incarnation replication before activation".into());
    }
    Ok(())
}

fn bind_hunting_game_sources(
    config: GatewayConfig,
    runtime: Option<HuntingRuntimeConfig>,
) -> Result<GatewayConfig, String> {
    match runtime {
        None => {
            if config.hunting_economy_realm.is_some() {
                return Err(
                    "hunting game source binding cannot be silently disabled by the environment"
                        .into(),
                );
            }
            Ok(config)
        }
        Some(runtime) => {
            if config.account_store_database_url.as_deref() != Some(runtime.database_url.as_str()) {
                return Err(
                    "hunting game source database differs from conversion authority".into(),
                );
            }
            if config
                .hunting_economy_realm
                .as_ref()
                .is_some_and(|realm| realm != &runtime.realm_id)
            {
                return Err("hunting game source realm differs from conversion authority".into());
            }
            config.with_hunting_economy_realm(runtime.realm_id)
        }
    }
}

fn initialize(runtime: &HuntingRuntimeConfig, policy_path: Option<PathBuf>) -> Result<(), String> {
    let policy = policy_path
        .map(|path| {
            let bytes = fs::read(path).map_err(|_| "hunting policy file is unreadable")?;
            if bytes.len() > 65_536 {
                return Err("hunting policy exceeds 64 KiB");
            }
            serde_json::from_slice::<Policy>(&bytes).map_err(|_| "invalid hunting policy JSON")
        })
        .transpose()?;
    let mut client = dedicated_client(runtime)?;
    {
        let state = if let Some(policy) = &policy {
            initialize_realm(&mut client, &runtime.realm_id, policy)
        } else {
            read_realm(&mut client, &runtime.realm_id)
        }
        .map_err(|error| error.to_string())?;
        if policy
            .as_ref()
            .is_some_and(|policy| policy.policy_version != state.policy_version)
        {
            return Err("configured hunting policy differs from active database policy; register and activate a reviewed version explicitly".into());
        }
        Ok(())
    }
}

// Maintenance owns its backend for its entire lifetime. It must never cancel a
// token from the shared login/save pool after that connection has been returned.
fn dedicated_client(runtime: &HuntingRuntimeConfig) -> Result<Client, String> {
    let mut config: postgres::Config = runtime
        .database_url
        .parse()
        .map_err(|_| "invalid hunting database configuration")?;
    config.connect_timeout(Duration::from_secs(5));
    config.application_name("mir2-hunting-clock");
    let mut client = config
        .connect(NoTls)
        .map_err(|_| "hunting maintenance database connection unavailable")?;
    client
        .batch_execute("SET lock_timeout = '2s'; SET statement_timeout = '10s'")
        .map_err(|_| "hunting maintenance database timeout configuration failed")?;
    Ok(client)
}

/// One bounded batch. Database time and the durable cursor decide which days are
/// closed. Repeating it before 04:00 is a no-op; outage recovery needs no player.
pub fn run_hunting_settlement_once(runtime: &HuntingRuntimeConfig) -> Result<usize, String> {
    let mut client = dedicated_client(runtime)?;
    settle_due_with_timeouts(&mut client, &runtime.realm_id, 31, 2_000, 10_000)
        .map(|receipts| receipts.len())
        .map_err(|error| error.to_string())
}

/// Observation delivery is separate from settlement. A failed metric export
/// cannot roll back game assets or prevent the next closed day from settling.
pub fn export_hunting_metrics_once(runtime: &HuntingRuntimeConfig) -> Result<usize, String> {
    let mut client = dedicated_client(runtime)?;
    export_daily_metrics_with_timeouts(&mut client, &runtime.realm_id, 128, 2_000, 10_000)
        .map(|batch| batch.receipts.len())
        .map_err(|error| error.to_string())
}

pub struct HuntingClockService {
    stop: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}
impl HuntingClockService {
    /// Call from a blocking context. The optional policy file initializes a new
    /// zero-stock realm only; restart never injects permits or silently changes K.
    pub fn start(config: &GatewayConfig) -> io::Result<Self> {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let Some(runtime) = hunting_http_runtime(config).map_err(io::Error::other)? else {
            return Ok(Self { stop, worker: None });
        };
        let policy_path = env::var_os("MIR2_HUNTING_ECONOMY_POLICY_PATH").map(PathBuf::from);
        Self::start_runtime(runtime, policy_path, Duration::from_secs(15))
    }

    fn start_runtime(
        runtime: HuntingRuntimeConfig,
        policy_path: Option<PathBuf>,
        period: Duration,
    ) -> io::Result<Self> {
        initialize(&runtime, policy_path).map_err(io::Error::other)?;
        run_hunting_settlement_once(&runtime).map_err(io::Error::other)?;
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let signal = stop.clone();
        let worker = std::thread::Builder::new()
            .name("hunting-economy-clock".into())
            .spawn(move || {
                let mut previous_error = None;
                let mut previous_export_error = None;
                loop {
                    if signal.0.lock().map_or(true, |stop| *stop) {
                        break;
                    }
                    let current_error = run_hunting_settlement_once(&runtime).err();
                    if current_error != previous_error {
                        if let Some(error) = &current_error {
                            eprintln!("[hunting-economy-clock] {error}");
                        } else if previous_error.is_some() {
                            eprintln!("[hunting-economy-clock] settlement recovered");
                        }
                        previous_error = current_error;
                    }
                    if signal.0.lock().map_or(true, |stop| *stop) {
                        break;
                    }
                    let export_error = export_hunting_metrics_once(&runtime).err();
                    if export_error != previous_export_error {
                        if let Some(error) = &export_error {
                            eprintln!("[hunting-economy-metrics] {error}");
                        } else if previous_export_error.is_some() {
                            eprintln!("[hunting-economy-metrics] export recovered");
                        }
                        previous_export_error = export_error;
                    }
                    let (lock, wake) = &*signal;
                    let Ok(stopped) = lock.lock() else { break };
                    let Ok((stopped, _)) = wake.wait_timeout_while(stopped, period, |stop| !*stop)
                    else {
                        break;
                    };
                    if *stopped {
                        break;
                    }
                }
            })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}
impl Drop for HuntingClockService {
    fn drop(&mut self) {
        if let Ok(mut stopped) = self.stop.0.lock() {
            *stopped = true;
            self.stop.1.notify_all();
        }
        if let Some(worker) = self.worker.take() {
            let deadline = Instant::now() + Duration::from_secs(12);
            while !worker.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            if worker.is_finished() {
                let _ = worker.join();
            } else {
                // Do not freeze Gateway shutdown behind a broken DB transport.
                // The transaction stays atomic; any unknown result is recovered
                // from the durable cursor on the next process start.
                eprintln!("[hunting-economy-clock] database worker did not stop within the shutdown budget");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_needs_no_database_realm_or_policy() {
        for value in [None, Some("0"), Some("false")] {
            assert!(
                runtime_from_values(value, None, None, AccountStoreDatabaseMode::Mirror)
                    .unwrap()
                    .is_none()
            );
        }
    }
    #[test]
    fn enabled_requires_explicit_realm_and_postgres_authority() {
        assert!(
            runtime_from_values(Some("yes"), None, None, AccountStoreDatabaseMode::Mirror).is_err()
        );
        assert!(runtime_from_values(
            Some("1"),
            Some("test-realm"),
            Some("private"),
            AccountStoreDatabaseMode::Mirror
        )
        .is_err());
        assert!(runtime_from_values(
            Some("1"),
            None,
            Some("private"),
            AccountStoreDatabaseMode::SourceOfTruth
        )
        .is_err());
        assert!(runtime_from_values(
            Some("1"),
            Some("test-realm"),
            None,
            AccountStoreDatabaseMode::SourceOfTruth
        )
        .is_err());
        let runtime = runtime_from_values(
            Some("1"),
            Some("test-realm"),
            Some("private-value"),
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .unwrap()
        .unwrap();
        assert_eq!(runtime.realm_id, "test-realm");
        assert!(!format!("{runtime:?}").contains("private-value"));
    }

    #[test]
    fn hunting_game_source_binding_is_disabled_by_default_and_requires_one_authority() {
        let disabled = bind_hunting_game_sources(GatewayConfig::default(), None).unwrap();
        assert!(disabled.hunting_economy_realm.is_none());
        let mut config = GatewayConfig::default();
        config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
        config.account_store_database_url = Some("postgresql://isolated-config-only".into());
        let runtime = HuntingRuntimeConfig {
            realm_id: "game-source-binding".into(),
            database_url: "postgresql://isolated-config-only".into(),
        };
        let enabled = bind_hunting_game_sources(config.clone(), Some(runtime.clone())).unwrap();
        assert_eq!(
            enabled.hunting_economy_realm.as_deref(),
            Some("game-source-binding")
        );
        assert!(enabled.fork_for_replica_apply().is_err());
        assert!(bind_hunting_game_sources(enabled.clone(), None).is_err());
        let mut wrong_realm = runtime.clone();
        wrong_realm.realm_id = "other-realm".into();
        assert!(bind_hunting_game_sources(enabled, Some(wrong_realm)).is_err());
        let mut wrong = runtime;
        wrong.database_url = "postgresql://other-authority".into();
        assert!(bind_hunting_game_sources(config, Some(wrong)).is_err());
    }

    #[test]
    fn hunting_source_topology_rejects_replication_before_source_activation() {
        assert!(validate_hunting_source_topology(true, false, None, false).is_ok());
        assert!(validate_hunting_source_topology(true, false, Some("  "), false).is_ok());
        assert!(validate_hunting_source_topology(true, true, None, false).is_err());
        assert!(
            validate_hunting_source_topology(true, false, Some("http://validator"), false).is_err()
        );
        assert!(validate_hunting_source_topology(true, false, None, true).is_err());
        assert!(
            validate_hunting_source_topology(false, true, Some("http://validator"), true).is_ok()
        );
    }

    fn postgres_fixture(label: &str) -> (Client, HuntingRuntimeConfig) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let url = env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
            .expect("ignored acceptance requires an explicitly isolated PostgreSQL URL");
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
        let mut client = config.connect(NoTls).expect("isolated test connection");
        client
            .batch_execute(mir2_economy_service::MIGRATION_SQL)
            .unwrap();
        client
            .batch_execute(mir2_economy_service::OBSERVATION_MIGRATION_SQL)
            .unwrap();
        client
            .batch_execute(mir2_economy_service::METRIC_EXPORT_MIGRATION_SQL)
            .unwrap();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let runtime = HuntingRuntimeConfig {
            realm_id: format!("clock-{label}-{}-{nonce}", std::process::id()),
            database_url: url,
        };
        let policy: Policy =
            serde_json::from_str(include_str!("../../../config/hunting-economy.json")).unwrap();
        initialize_realm(&mut client, &runtime.realm_id, &policy).unwrap();
        (client, runtime)
    }

    #[test]
    #[ignore = "requires explicitly isolated PostgreSQL"]
    fn hunting_clock_postgres_catches_up_without_players_or_duplicate_days() {
        let (mut client, runtime) = postgres_fixture("empty-recovery");
        let state = read_realm(&mut client, &runtime.realm_id).unwrap();
        let first_day = state.genesis_day - 80;
        client.execute(
            "UPDATE hunting_economy_pools SET genesis_day=$2,settled_through_day=$3 WHERE realm_id=$1",
            &[&runtime.realm_id, &first_day, &(first_day - 1)],
        ).unwrap();
        let clock =
            HuntingClockService::start_runtime(runtime.clone(), None, Duration::from_millis(20))
                .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let exported: i64 = client
                .query_one(
                    "SELECT count(*)::bigint FROM hunting_economy_metric_exports WHERE realm_id=$1",
                    &[&runtime.realm_id],
                )
                .unwrap()
                .get(0);
            if exported == 80
                && read_realm(&mut client, &runtime.realm_id)
                    .unwrap()
                    .settled_through_day
                    == state.genesis_day - 1
            {
                break;
            }
            assert!(Instant::now() < deadline, "empty realm did not catch up");
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(clock);
        assert_eq!(run_hunting_settlement_once(&runtime).unwrap(), 0);
        let row = client.query_one("SELECT count(*)::bigint AS n,count(DISTINCT business_day)::bigint AS distinct_n FROM hunting_economy_settlements WHERE realm_id=$1", &[&runtime.realm_id]).unwrap();
        assert_eq!(row.get::<_, i64>("n"), 80);
        assert_eq!(row.get::<_, i64>("distinct_n"), 80);
        let row = client.query_one("SELECT count(*)::bigint AS n,count(*) FILTER (WHERE NOT coverage_complete AND coverage_scope='new_wallets_only' AND bound_gold_stock IS NULL AND unbound_gold_stock IS NULL)::bigint AS partial_n FROM hunting_economy_daily_metric_versions WHERE realm_id=$1", &[&runtime.realm_id]).unwrap();
        assert_eq!(row.get::<_, i64>("n"), 80);
        assert_eq!(row.get::<_, i64>("partial_n"), 80);
        assert_eq!(
            read_realm(&mut client, &runtime.realm_id)
                .unwrap()
                .pool
                .stock_atoms,
            0
        );
    }

    #[test]
    #[ignore = "requires explicitly isolated PostgreSQL"]
    fn hunting_clock_postgres_lock_wait_and_shutdown_are_bounded() {
        let (mut client, runtime) = postgres_fixture("lock-stop");
        let clock =
            HuntingClockService::start_runtime(runtime.clone(), None, Duration::from_millis(10))
                .unwrap();
        let mut held = client.transaction().unwrap();
        mir2_economy_service::lock_realm_and_read_database_clock(&mut held, &runtime.realm_id)
            .unwrap();
        let began = Instant::now();
        assert!(run_hunting_settlement_once(&runtime).is_err());
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "realm lock did not time out"
        );
        // Stop while another maintenance attempt is waiting. The clock owns its
        // backend and never sends cancellation against the login/save pool.
        std::thread::sleep(Duration::from_millis(30));
        let began = Instant::now();
        drop(clock);
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "clock shutdown waited indefinitely"
        );
        held.rollback().unwrap();
        assert_eq!(run_hunting_settlement_once(&runtime).unwrap(), 0);
    }
}
