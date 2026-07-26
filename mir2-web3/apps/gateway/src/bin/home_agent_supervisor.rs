use std::env;
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use mir2_gateway::{
    HomeAgentKeyring, HomeAgentManagementKeyring, HomeAgentReleaseManifest,
    HomeAgentResourceController, HomeAgentResourceDecision, HomeAgentResourcePolicy,
    HomeAgentResourceSample, HomeAgentUpdateStore, HomeAgentWorkMode, ZoneHostTelemetrySnapshot,
};
use semver::Version;
use serde::Serialize;
use sysinfo::System;
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex, RwLock};

const DEFAULT_BIND: &str = "127.0.0.1:17990";
const DEFAULT_KEYRING_ACCOUNT: &str = "default";
const UPDATE_RESTART_EXIT_CODE: i32 = 75;

enum SupervisorExit {
    Clean,
    UpdateRestart,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SupervisorStatus {
    version: String,
    mode: HomeAgentWorkMode,
    accept_new_sessions: bool,
    reason: String,
    cpu_usage_percent: f32,
    available_memory_bytes: u64,
    active_sessions: usize,
    zone_reachable: bool,
    last_observed_at_ms: u64,
    node_id: String,
    public_key: String,
    key_store: String,
    managed_processes: bool,
}

#[derive(Clone)]
struct AppState {
    status: Arc<RwLock<SupervisorStatus>>,
    controller: Arc<Mutex<HomeAgentResourceController>>,
    operator_url: String,
    management_token: String,
}

struct ManagedHomeProcesses {
    zone_host: Child,
    home_agent: Child,
}

#[tokio::main]
async fn main() {
    match run().await {
        Ok(SupervisorExit::Clean) => {}
        Ok(SupervisorExit::UpdateRestart) => std::process::exit(UPDATE_RESTART_EXIT_CODE),
        Err(error) => {
            eprintln!("HOME_AGENT_SUPERVISOR_FATAL {error}");
            std::process::exit(1);
        }
    }
}

async fn run() -> Result<SupervisorExit, String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match arguments.first().map(String::as_str) {
        Some("key-init") => key_init().map(|()| SupervisorExit::Clean),
        Some("key-import") => key_import(arguments.get(1)).map(|()| SupervisorExit::Clean),
        Some("key-public") => key_public().map(|()| SupervisorExit::Clean),
        Some("key-delete") => key_delete().map(|()| SupervisorExit::Clean),
        Some("verify-manifest") => {
            verify_manifest(arguments.get(1)).map(|()| SupervisorExit::Clean)
        }
        Some("stage-update") => {
            stage_update(arguments.get(1), arguments.get(2)).map(|()| SupervisorExit::Clean)
        }
        Some("update-once") => {
            let staged = check_and_stage_signed_update().await?;
            println!("HOME_AGENT_UPDATE_CHECKED staged={staged}");
            Ok(SupervisorExit::Clean)
        }
        Some(command) if command != "serve" => Err(format!(
            "unknown command {command}; expected serve|key-init|key-import|key-public|key-delete|verify-manifest|stage-update|update-once"
        )),
        _ => serve().await,
    }
}

fn keyring() -> Result<HomeAgentKeyring, String> {
    HomeAgentKeyring::new(
        env::var("MIR2_HOME_AGENT_KEYRING_ACCOUNT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_KEYRING_ACCOUNT.to_string()),
    )
}

fn keyring_account() -> String {
    env::var("MIR2_HOME_AGENT_KEYRING_ACCOUNT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_KEYRING_ACCOUNT.to_string())
}

fn management_token() -> Result<String, String> {
    if let Some(token) = env::var("MIR2_ZONE_HOST_MANAGEMENT_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        if token.as_bytes().len() < 32 {
            return Err(
                "MIR2_ZONE_HOST_MANAGEMENT_TOKEN must contain at least 32 bytes".to_string(),
            );
        }
        return Ok(token);
    }
    HomeAgentManagementKeyring::new(keyring_account())?
        .load_or_create_token()
        .map(|(token, _)| token)
}

fn key_init() -> Result<(), String> {
    let (identity, created) = keyring()?.load_or_create_identity()?;
    println!(
        "{}",
        serde_json::json!({
            "created": created,
            "nodeId": identity.node_id(),
            "publicKey": identity.public_key(),
            "keyStore": "operating-system-keyring",
        })
    );
    Ok(())
}

fn key_import(value: Option<&String>) -> Result<(), String> {
    let value = value.ok_or_else(|| {
        "key-import requires a URL-safe base64 seed argument; the seed is never printed".to_string()
    })?;
    let identity = keyring()?.import_base64_seed(value)?;
    println!(
        "{}",
        serde_json::json!({
            "imported": true,
            "nodeId": identity.node_id(),
            "publicKey": identity.public_key(),
            "keyStore": "operating-system-keyring",
        })
    );
    Ok(())
}

fn key_public() -> Result<(), String> {
    let identity = keyring()?.load_identity()?;
    println!(
        "{}",
        serde_json::json!({
            "nodeId": identity.node_id(),
            "publicKey": identity.public_key(),
            "keyStore": "operating-system-keyring",
        })
    );
    Ok(())
}

fn key_delete() -> Result<(), String> {
    keyring()?.delete_secret()?;
    println!("{}", serde_json::json!({"deleted": true}));
    Ok(())
}

fn verify_manifest(path: Option<&String>) -> Result<(), String> {
    let path = required_path(path, "verify-manifest requires a manifest path")?;
    let manifest: HomeAgentReleaseManifest =
        serde_json::from_slice(&std::fs::read(&path).map_err(|error| {
            format!(
                "read Home Agent release manifest {}: {error}",
                path.display()
            )
        })?)
        .map_err(|error| format!("decode Home Agent release manifest: {error}"))?;
    let artifact = verify_release(&manifest)?;
    println!(
        "{}",
        serde_json::json!({
            "verified": true,
            "version": manifest.payload.version,
            "target": artifact.target,
            "sha256": artifact.sha256,
            "sizeBytes": artifact.size_bytes,
        })
    );
    Ok(())
}

fn stage_update(
    manifest_path: Option<&String>,
    artifact_path: Option<&String>,
) -> Result<(), String> {
    let manifest_path = required_path(
        manifest_path,
        "stage-update requires manifest and downloaded artifact paths",
    )?;
    let artifact_path = required_path(
        artifact_path,
        "stage-update requires manifest and downloaded artifact paths",
    )?;
    let manifest: HomeAgentReleaseManifest =
        serde_json::from_slice(&std::fs::read(&manifest_path).map_err(|error| {
            format!(
                "read Home Agent release manifest {}: {error}",
                manifest_path.display()
            )
        })?)
        .map_err(|error| format!("decode Home Agent release manifest: {error}"))?;
    let artifact = verify_release(&manifest)?;
    let bytes = std::fs::read(&artifact_path).map_err(|error| {
        format!(
            "read downloaded Home Agent artifact {}: {error}",
            artifact_path.display()
        )
    })?;
    let root = update_root()?;
    let store = HomeAgentUpdateStore::new(&root);
    let mut state = store.load_state(env!("CARGO_PKG_VERSION"))?;
    let staged = store.stage_bundle(&mut state, &manifest, &artifact, &bytes)?;
    println!(
        "{}",
        serde_json::json!({
            "staged": true,
            "version": manifest.payload.version,
            "path": staged,
            "activation": "pending-supervisor-restart",
        })
    );
    Ok(())
}

fn verify_release(
    manifest: &HomeAgentReleaseManifest,
) -> Result<mir2_gateway::HomeAgentArtifact, String> {
    let issuer = required_env("MIR2_HOME_UPDATE_ISSUER_PUBLIC_KEY")?;
    let channel = env::var("MIR2_HOME_UPDATE_CHANNEL").unwrap_or_else(|_| "stable".to_string());
    let target = required_env("MIR2_HOME_UPDATE_TARGET")?;
    manifest.verify(
        &issuer,
        &channel,
        &target,
        env!("CARGO_PKG_VERSION"),
        now_ms(),
    )
}

async fn monitor_signed_updates(sender: oneshot::Sender<()>) -> Result<(), String> {
    let interval = Duration::from_secs(
        positive_u64_env("MIR2_HOME_UPDATE_CHECK_INTERVAL_SECONDS", 3_600)?.clamp(5, 86_400),
    );
    loop {
        match check_and_stage_signed_update().await {
            Ok(true) => {
                let _ = sender.send(());
                return Ok(());
            }
            Ok(false) => {}
            Err(error) => eprintln!("HOME_AGENT_UPDATE_CHECK_REJECTED {error}"),
        }
        tokio::time::sleep(interval).await;
    }
}

async fn check_and_stage_signed_update() -> Result<bool, String> {
    let manifest_url = required_env("MIR2_HOME_UPDATE_MANIFEST_URL")?;
    require_https_url("Home Agent update manifest", &manifest_url)?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("build Home Agent update client: {error}"))?;
    let manifest_bytes =
        download_bounded(&client, &manifest_url, 1024 * 1024, "release manifest").await?;
    let manifest: HomeAgentReleaseManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("decode Home Agent release manifest: {error}"))?;
    let issuer = required_env("MIR2_HOME_UPDATE_ISSUER_PUBLIC_KEY")?;
    let channel = env::var("MIR2_HOME_UPDATE_CHANNEL").unwrap_or_else(|_| "stable".to_string());
    let target = required_env("MIR2_HOME_UPDATE_TARGET")?;
    let artifact = manifest.verify_signed_artifact(&issuer, &channel, &target, now_ms())?;

    let root = update_root()?;
    let store = HomeAgentUpdateStore::new(&root);
    let mut state = store.load_state(env!("CARGO_PKG_VERSION"))?;
    let current = Version::parse(&state.current_version)
        .map_err(|error| format!("invalid current Home Agent version: {error}"))?;
    let release = Version::parse(&manifest.payload.version)
        .map_err(|error| format!("invalid release Home Agent version: {error}"))?;
    let minimum = Version::parse(&manifest.payload.minimum_agent_version)
        .map_err(|error| format!("invalid minimum Home Agent version: {error}"))?;
    if release <= current {
        return Ok(false);
    }
    if current < minimum {
        return Err(format!(
            "current Home Agent {current} is below release minimum {minimum}; bootstrap update required"
        ));
    }
    require_https_url("Home Agent update artifact", &artifact.url)?;
    let maximum_artifact_bytes =
        positive_u64_env("MIR2_HOME_UPDATE_MAX_DOWNLOAD_BYTES", 512 * 1024 * 1024)?;
    if artifact.size_bytes > maximum_artifact_bytes {
        return Err(format!(
            "Home Agent update artifact {} exceeds configured maximum {maximum_artifact_bytes}",
            artifact.size_bytes
        ));
    }
    let bytes = download_bounded(
        &client,
        &artifact.url,
        maximum_artifact_bytes,
        "release artifact",
    )
    .await?;
    let release_dir = store.stage_bundle(&mut state, &manifest, &artifact, &bytes)?;
    println!(
        "HOME_AGENT_UPDATE_STAGED version={} release_dir={}",
        manifest.payload.version,
        release_dir.display()
    );
    Ok(true)
}

async fn download_bounded(
    client: &reqwest::Client,
    url: &str,
    maximum_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|error| format!("download Home Agent {label}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Home Agent {label} HTTP status rejected: {error}"))?;
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes)
    {
        return Err(format!("Home Agent {label} exceeds maximum download size"));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("read Home Agent {label}: {error}"))?;
        let next_length = (bytes.len() as u64)
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| format!("Home Agent {label} download size overflow"))?;
        if next_length > maximum_bytes {
            return Err(format!("Home Agent {label} exceeds maximum download size"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn require_https_url(label: &str, value: &str) -> Result<(), String> {
    let url =
        reqwest::Url::parse(value).map_err(|error| format!("invalid {label} URL: {error}"))?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
    {
        return Err(format!(
            "{label} URL must use HTTPS without embedded credentials"
        ));
    }
    Ok(())
}

fn update_root() -> Result<PathBuf, String> {
    env::var("MIR2_HOME_UPDATE_ROOT")
        .map(PathBuf::from)
        .map_err(|_| "MIR2_HOME_UPDATE_ROOT is required for stage-update".to_string())
}

async fn serve() -> Result<SupervisorExit, String> {
    let identity = keyring()?.load_identity().map_err(|error| {
        format!("initialize the Home Agent identity with `home_agent_supervisor key-init`: {error}")
    })?;
    let bind = required_socket_env("MIR2_HOME_SUPERVISOR_BIND", DEFAULT_BIND)?;
    if !bind.ip().is_loopback() {
        return Err("Home Agent supervisor must bind to a loopback address".to_string());
    }
    let operator_url = required_env("MIR2_HOME_ZONE_OPERATOR_URL")?;
    let parsed_operator = reqwest::Url::parse(&operator_url)
        .map_err(|error| format!("invalid MIR2_HOME_ZONE_OPERATOR_URL: {error}"))?;
    if !matches!(
        parsed_operator.host_str(),
        Some("127.0.0.1" | "::1" | "localhost")
    ) {
        return Err("Home Agent Zone operator URL must target loopback".to_string());
    }
    let management_token = management_token()?;
    let policy = policy_from_env()?;
    let managed_processes = boolean_env("MIR2_HOME_MANAGE_CHILDREN", false)?;
    let status = Arc::new(RwLock::new(SupervisorStatus {
        version: env!("CARGO_PKG_VERSION").to_string(),
        mode: HomeAgentWorkMode::Draining,
        accept_new_sessions: false,
        reason: "starting".to_string(),
        cpu_usage_percent: 0.0,
        available_memory_bytes: 0,
        active_sessions: 0,
        zone_reachable: false,
        last_observed_at_ms: now_ms(),
        node_id: identity.node_id().to_string(),
        public_key: identity.public_key().to_string(),
        key_store: "operating-system-keyring".to_string(),
        managed_processes,
    }));
    let controller = Arc::new(Mutex::new(HomeAgentResourceController::new(
        policy.clone(),
    )?));
    let state = AppState {
        status: Arc::clone(&status),
        controller: Arc::clone(&controller),
        operator_url,
        management_token,
    };
    let mut managed_processes = spawn_managed_processes(&state).await?;
    let monitor_state = state.clone();
    let monitor = tokio::spawn(async move { monitor_resources(monitor_state, policy).await });
    let (update_sender, mut update_receiver) = oneshot::channel();
    let mut update_sender_guard = Some(update_sender);
    let update_monitor = if env::var("MIR2_HOME_UPDATE_MANIFEST_URL")
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
    {
        let update_sender = update_sender_guard
            .take()
            .expect("Home Agent update sender must be available");
        Some(tokio::spawn(async move {
            monitor_signed_updates(update_sender).await
        }))
    } else {
        // Keep the sender alive so the update branch remains pending.
        None
    };
    let router = Router::new()
        .route("/", get(index))
        .route("/v1/status", get(api_status))
        .route("/v1/drain", post(api_drain))
        .route("/v1/resume", post(api_resume))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|error| format!("bind Home Agent supervisor {bind}: {error}"))?;
    println!(
        "HOME_AGENT_SUPERVISOR_READY http=http://{bind}/ node_id={}",
        identity.node_id()
    );
    let shutdown_state = state.clone();
    let server = axum::serve(listener, router).with_graceful_shutdown(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = request_manual_drain(&shutdown_state, true).await;
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if fetch_zone_health(&shutdown_state.operator_url)
                .await
                .is_ok_and(|health| health.health.session_count == 0)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });
    let server = server.into_future();
    tokio::pin!(server);
    let result: Result<bool, String> = if let Some(processes) = managed_processes.as_mut() {
        tokio::select! {
            server_result = &mut server => {
                server_result
                    .map(|()| false)
                    .map_err(|error| format!("Home Agent supervisor server: {error}"))
            }
            status = processes.zone_host.wait() => {
                Err(format!(
                    "managed Zone Host exited unexpectedly: {}",
                    status.map_err(|error| format!("wait for managed Zone Host: {error}"))?
                ))
            }
            status = processes.home_agent.wait() => {
                let _ = request_manual_drain(&state, true).await;
                Err(format!(
                    "managed Home Agent exited unexpectedly: {}",
                    status.map_err(|error| format!("wait for managed Home Agent: {error}"))?
                ))
            }
            update = &mut update_receiver => {
                update.map_err(|_| "Home Agent update monitor stopped before staging".to_string())?;
                drain_and_wait(&state).await?;
                Ok(true)
            }
        }
    } else {
        tokio::select! {
            server_result = &mut server => {
                server_result
                    .map(|()| false)
                    .map_err(|error| format!("Home Agent supervisor server: {error}"))
            }
            update = &mut update_receiver => {
                update.map_err(|_| "Home Agent update monitor stopped before staging".to_string())?;
                drain_and_wait(&state).await?;
                Ok(true)
            }
        }
    };
    monitor.abort();
    if let Some(update_monitor) = update_monitor {
        update_monitor.abort();
    }
    if let Some(mut processes) = managed_processes {
        let _ = processes.home_agent.start_kill();
        let _ = processes.zone_host.start_kill();
        let _ = processes.home_agent.wait().await;
        let _ = processes.zone_host.wait().await;
    }
    if result? {
        Ok(SupervisorExit::UpdateRestart)
    } else {
        Ok(SupervisorExit::Clean)
    }
}

async fn spawn_managed_processes(state: &AppState) -> Result<Option<ManagedHomeProcesses>, String> {
    if !boolean_env("MIR2_HOME_MANAGE_CHILDREN", false)? {
        return Ok(None);
    }
    let binary_dir = env::var("MIR2_HOME_BIN_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_binary_dir);
    let zone_binary = configured_managed_binary("MIR2_HOME_ZONE_BINARY", &binary_dir, "zone_host");
    let agent_binary =
        configured_managed_binary("MIR2_HOME_AGENT_BINARY", &binary_dir, "home_agent");
    ensure_executable_file(&zone_binary)?;
    ensure_executable_file(&agent_binary)?;

    let mut zone_host = managed_command(&zone_binary, &state.management_token)
        .spawn()
        .map_err(|error| format!("start managed Zone Host {}: {error}", zone_binary.display()))?;
    let startup_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = zone_host
            .try_wait()
            .map_err(|error| format!("inspect managed Zone Host: {error}"))?
        {
            return Err(format!(
                "managed Zone Host exited during startup with {status}"
            ));
        }
        if fetch_zone_health(&state.operator_url).await.is_ok() {
            break;
        }
        if Instant::now() >= startup_deadline {
            let _ = zone_host.start_kill();
            let _ = zone_host.wait().await;
            return Err("managed Zone Host did not become healthy within 30 seconds".to_string());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let home_agent = match managed_command(&agent_binary, &state.management_token).spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = zone_host.start_kill();
            let _ = zone_host.wait().await;
            return Err(format!(
                "start managed Home Agent {}: {error}",
                agent_binary.display()
            ));
        }
    };
    Ok(Some(ManagedHomeProcesses {
        zone_host,
        home_agent,
    }))
}

fn managed_command(binary: &Path, management_token: &str) -> Command {
    let mut command = Command::new(binary);
    command
        .env("MIR2_ZONE_HOST_MANAGEMENT_TOKEN", management_token)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    command
}

fn default_binary_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn managed_binary(directory: &Path, name: &str) -> PathBuf {
    #[cfg(windows)]
    {
        directory.join(format!("{name}.exe"))
    }
    #[cfg(not(windows))]
    {
        directory.join(name)
    }
}

fn configured_managed_binary(name: &str, directory: &Path, default_name: &str) -> PathBuf {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| managed_binary(directory, default_name))
}

fn ensure_executable_file(path: &Path) -> Result<(), String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("inspect managed binary {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("managed binary {} is not a file", path.display()));
    }
    Ok(())
}

fn boolean_env(name: &str, default: bool) -> Result<bool, String> {
    match env::var(name) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => Ok(true),
            "0" | "false" | "no" => Ok(false),
            _ => Err(format!("{name} must be true or false")),
        },
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(format!("read {name}: {error}")),
    }
}

async fn monitor_resources(state: AppState, policy: HomeAgentResourcePolicy) -> Result<(), String> {
    let mut system = System::new_all();
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    let mut previous = Instant::now();
    loop {
        tokio::time::sleep(Duration::from_millis(policy.expected_sample_interval_ms)).await;
        system.refresh_cpu_usage();
        system.refresh_memory();
        let now = Instant::now();
        let elapsed = now.duration_since(previous).as_millis() as u64;
        previous = now;
        let zone = fetch_zone_health(&state.operator_url).await;
        let (active_sessions, zone_reachable) = zone
            .as_ref()
            .map(|snapshot| (snapshot.health.session_count, true))
            .unwrap_or((0, false));
        let sample = HomeAgentResourceSample {
            observed_at_ms: now_ms(),
            cpu_usage_percent: system.global_cpu_usage(),
            available_memory_bytes: system.available_memory(),
            active_sessions,
            elapsed_since_previous_ms: elapsed,
        };
        let decision = state.controller.lock().await.observe(sample);
        if decision.request_drain {
            operator_action(&state, true).await?;
        } else if decision.request_resume && zone_reachable {
            operator_action(&state, false).await?;
        }
        let mut status = state.status.write().await;
        apply_status(&mut status, sample, decision, zone_reachable);
    }
}

fn apply_status(
    status: &mut SupervisorStatus,
    sample: HomeAgentResourceSample,
    decision: HomeAgentResourceDecision,
    zone_reachable: bool,
) {
    status.mode = decision.mode;
    status.accept_new_sessions = decision.accept_new_sessions && zone_reachable;
    status.reason = if zone_reachable {
        decision.reason
    } else {
        "zone_operator_unreachable".to_string()
    };
    status.cpu_usage_percent = sample.cpu_usage_percent;
    status.available_memory_bytes = sample.available_memory_bytes;
    status.active_sessions = sample.active_sessions;
    status.zone_reachable = zone_reachable;
    status.last_observed_at_ms = sample.observed_at_ms;
}

async fn fetch_zone_health(url: &str) -> Result<ZoneHostTelemetrySnapshot, String> {
    reqwest::Client::new()
        .get(format!("{}/healthz", url.trim_end_matches('/')))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map_err(|error| format!("query Zone Host operator: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Zone Host operator health rejected: {error}"))?
        .json()
        .await
        .map_err(|error| format!("decode Zone Host health: {error}"))
}

async fn operator_action(state: &AppState, drain: bool) -> Result<(), String> {
    let path = if drain { "drain" } else { "resume" };
    reqwest::Client::new()
        .post(format!(
            "{}/v1/{path}",
            state.operator_url.trim_end_matches('/')
        ))
        .bearer_auth(&state.management_token)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map_err(|error| format!("request Zone Host {path}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Zone Host {path} rejected: {error}"))?;
    Ok(())
}

async fn request_manual_drain(state: &AppState, enabled: bool) -> Result<(), String> {
    state.controller.lock().await.set_manual_drain(enabled);
    operator_action(state, enabled).await
}

async fn drain_and_wait(state: &AppState) -> Result<(), String> {
    request_manual_drain(state, true).await?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if fetch_zone_health(&state.operator_url)
            .await
            .is_ok_and(|health| health.health.session_count == 0)
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err("Home Agent update drain exceeded 30 seconds".to_string())
}

async fn index(State(state): State<AppState>) -> Html<String> {
    let status = state.status.read().await.clone();
    Html(format!(
        r#"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Dubhe Home Agent</title><style>body{{font-family:system-ui;background:#08111f;color:#e7f0ff;max-width:760px;margin:48px auto;padding:24px}}main{{background:#101d31;border:1px solid #294264;border-radius:20px;padding:28px}}h1{{margin-top:0}}dl{{display:grid;grid-template-columns:180px 1fr;gap:12px}}dt{{color:#8da8ce}}code{{color:#7ee787;word-break:break-all}}button{{padding:10px 18px;border-radius:10px;border:0;margin-right:8px}}small{{color:#8da8ce}}</style></head><body><main><h1>Dubhe Home Agent</h1><p>状态：<strong>{:?}</strong> · {}</p><dl><dt>Node ID</dt><dd><code>{}</code></dd><dt>CPU</dt><dd>{:.1}%</dd><dt>可用内存</dt><dd>{:.2} GiB</dd><dt>Mir2 Sessions</dt><dd>{}</dd><dt>Zone</dt><dd>{}</dd><dt>密钥</dt><dd>系统密钥库</dd></dl><p><small>管理操作必须通过带 Bearer token 的本地 API；网页不保存管理密钥。</small></p></main></body></html>"#,
        status.mode,
        status.reason,
        html_escape(&status.node_id),
        status.cpu_usage_percent,
        status.available_memory_bytes as f64 / 1024.0 / 1024.0 / 1024.0,
        status.active_sessions,
        if status.zone_reachable {
            "在线"
        } else {
            "不可达"
        },
    ))
}

async fn api_status(State(state): State<AppState>) -> Json<SupervisorStatus> {
    Json(state.status.read().await.clone())
}

async fn api_drain(State(state): State<AppState>, headers: HeaderMap) -> Response {
    api_manual_action(state, headers, true).await
}

async fn api_resume(State(state): State<AppState>, headers: HeaderMap) -> Response {
    api_manual_action(state, headers, false).await
}

async fn api_manual_action(state: AppState, headers: HeaderMap, drain: bool) -> Response {
    if !bearer_matches(&headers, &state.management_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "valid local management bearer token required"})),
        )
            .into_response();
    }
    match request_manual_drain(&state, drain).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"accepted": true, "draining": drain})),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

fn bearer_matches(headers: &HeaderMap, expected: &str) -> bool {
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    supplied.is_some_and(|value| constant_time_equal(value.as_bytes(), expected.as_bytes()))
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            })
            == 0
}

fn policy_from_env() -> Result<HomeAgentResourcePolicy, String> {
    let mut policy = HomeAgentResourcePolicy::default();
    policy.maximum_cpu_percent =
        env_parse("MIR2_HOME_MAX_CPU_PERCENT", policy.maximum_cpu_percent)?;
    let minimum_memory_mib: u64 = env_parse(
        "MIR2_HOME_MIN_AVAILABLE_MEMORY_MIB",
        policy.minimum_available_memory_bytes / 1024 / 1024,
    )?;
    policy.minimum_available_memory_bytes = minimum_memory_mib.saturating_mul(1024 * 1024);
    policy.overload_samples_before_drain = env_parse(
        "MIR2_HOME_OVERLOAD_SAMPLES",
        policy.overload_samples_before_drain,
    )?;
    policy.recovery_samples_before_resume = env_parse(
        "MIR2_HOME_RECOVERY_SAMPLES",
        policy.recovery_samples_before_resume,
    )?;
    policy.expected_sample_interval_ms = env_parse(
        "MIR2_HOME_SAMPLE_INTERVAL_MS",
        policy.expected_sample_interval_ms,
    )?;
    policy.validate()?;
    Ok(policy)
}

fn env_parse<T>(name: &str, default: T) -> Result<T, String>
where
    T: std::str::FromStr,
{
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<T>()
                .map_err(|_| format!("{name} contains an invalid value"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn positive_u64_env(name: &str, default: u64) -> Result<u64, String> {
    env_parse(name, default).and_then(|value| {
        (value > 0)
            .then_some(value)
            .ok_or_else(|| format!("{name} must be a positive integer"))
    })
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn required_socket_env(name: &str, default: &str) -> Result<SocketAddr, String> {
    env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .parse()
        .map_err(|error| format!("{name} must be a socket address: {error}"))
}

fn required_path(value: Option<&String>, message: &str) -> Result<PathBuf, String> {
    value.map(PathBuf::from).ok_or_else(|| message.to_string())
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
