use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use mir2_gateway::{
    CapacityChallenge, CapacityChallengeResponse, HomeAgentKeyring, HomeAgentManagementKeyring,
    HomeAgentWorkMode, HomeCapacityCertificationRequest, HomeEnrollmentRequest,
    NodeSigningIdentity, SignedHomeEnrollmentBundle, SignedHomeEnrollmentChallenge,
};
use rcgen::{
    CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose,
};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};

mod desktop_runtime;

const DEFAULT_KEYRING_ACCOUNT: &str = "default";
const DEFAULT_SUPERVISOR_URL: &str = "http://127.0.0.1:17990";
const ENROLLMENT_CLIENT_CONFIG_FILE: &str = "enrollment-client.json";
const ENROLLMENT_BUNDLE_FILE: &str = "enrollment-bundle.json";
const CAPACITY_CERTIFICATE_FILE: &str = "capacity-certificate.json";
const RELAY_CA_CERTIFICATE_FILE: &str = "relay-ca.der";
const RELAY_CLIENT_CERTIFICATE_FILE: &str = "relay-client.der";
const RELAY_TLS_KEYRING_SUFFIX: &str = "relay-tls";
const AGENT_RUNTIME_STATUS_FILE: &str = "agent-runtime-status.json";
const SUPERVISOR_LOG_FILE: &str = "home-agent-supervisor.log";
const SUPERVISOR_LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;
const DEFAULT_CREDENTIAL_RENEWAL_WINDOW_MS: u64 = 6 * 60 * 60 * 1_000;
const DEFAULT_CREDENTIAL_RENEWAL_POLL_MS: u64 = 5 * 60 * 1_000;
const SUPERVISOR_GRACEFUL_SHUTDOWN_SECONDS: u64 = 40;

struct DesktopState {
    supervisor: Mutex<Option<Child>>,
    credential_maintenance: Mutex<()>,
    renewal: RwLock<RenewalRuntime>,
}

impl Default for DesktopState {
    fn default() -> Self {
        Self {
            supervisor: Mutex::new(None),
            credential_maintenance: Mutex::new(()),
            renewal: RwLock::new(RenewalRuntime::default()),
        }
    }
}

#[derive(Debug, Clone)]
struct RenewalRuntime {
    state: &'static str,
    last_renewed_at_ms: Option<u64>,
    error: Option<String>,
    renewal_draining: bool,
}

impl Default for RenewalRuntime {
    fn default() -> Self {
        Self {
            state: "idle",
            last_renewed_at_ms: None,
            error: None,
            renewal_draining: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopIdentity {
    node_id: String,
    public_key: String,
    created: bool,
    key_store: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopBootstrap {
    identity: DesktopIdentity,
    management_token_created: bool,
    supervisor_reachable: bool,
    status: Option<SupervisorStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    agent_managed: bool,
    relay_connected: bool,
    telemetry_configured: bool,
    telemetry_accepted: bool,
    telemetry_sequence: Option<u64>,
    last_telemetry_at_ms: Option<u64>,
    telemetry_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeActionReceipt {
    accepted: bool,
    serving: bool,
    status: SupervisorStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopEnrollmentStatus {
    configured: bool,
    enrolled: bool,
    capacity_ready: bool,
    relay_ready: bool,
    enrollment_id: Option<String>,
    expires_at_ms: Option<u64>,
    relay_id: Option<String>,
    telemetry_url: Option<String>,
    max_sessions: Option<usize>,
    max_zones: Option<usize>,
    renewal_state: String,
    renew_at_ms: Option<u64>,
    last_renewed_at_ms: Option<u64>,
    renewal_error: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct EnrollmentClientConfig {
    base_url: Url,
    trusted_issuer_public_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnrollmentClientConfigFile {
    base_url: String,
    trusted_issuer_public_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnrollmentResponse {
    accepted: bool,
    bundle: SignedHomeEnrollmentBundle,
}

fn keyring_account() -> String {
    env::var("MIR2_HOME_AGENT_KEYRING_ACCOUNT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_KEYRING_ACCOUNT.to_string())
}

fn supervisor_url() -> Result<Url, String> {
    let value = env::var("MIR2_HOME_SUPERVISOR_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SUPERVISOR_URL.to_string());
    validate_supervisor_url(&value)
}

fn validate_supervisor_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("invalid Supervisor URL: {error}"))?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.host_str(), Some("127.0.0.1" | "::1" | "localhost"))
    {
        return Err(
            "Dubhe Node desktop only accepts a credential-free loopback Supervisor URL".to_string(),
        );
    }
    Ok(url)
}

fn supervisor_client() -> Result<Client, String> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|error| format!("build Supervisor client: {error}"))
}

fn enrollment_client_config(app: &AppHandle) -> Result<Option<EnrollmentClientConfig>, String> {
    let base_url = env::var("MIR2_HOME_ENROLLMENT_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let trusted_issuer_public_key = env::var("MIR2_HOME_ENROLLMENT_ISSUER_PUBLIC_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (base_url, trusted_issuer_public_key) {
        (None, None) => {
            let path = app
                .path()
                .app_config_dir()
                .map_err(|error| format!("resolve Dubhe Node configuration directory: {error}"))?
                .join(ENROLLMENT_CLIENT_CONFIG_FILE);
            let file = match fs::read(&path) {
                Ok(bytes) => serde_json::from_slice::<EnrollmentClientConfigFile>(&bytes)
                    .map_err(|error| {
                        format!(
                            "decode Home enrollment client configuration {}: {error}",
                            path.display()
                        )
                    })?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => {
                    return Err(format!(
                        "read Home enrollment client configuration {}: {error}",
                        path.display()
                    ));
                }
            };
            Ok(Some(EnrollmentClientConfig {
                base_url: validate_enrollment_url(&file.base_url)?,
                trusted_issuer_public_key: file.trusted_issuer_public_key,
            }))
        }
        (Some(value), Some(trusted_issuer_public_key)) => {
            let base_url = validate_enrollment_url(&value)?;
            Ok(Some(EnrollmentClientConfig {
                base_url,
                trusted_issuer_public_key,
            }))
        }
        _ => Err(
            "MIR2_HOME_ENROLLMENT_URL and MIR2_HOME_ENROLLMENT_ISSUER_PUBLIC_KEY must be configured together"
                .to_string(),
        ),
    }
}

fn validate_enrollment_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("invalid Home enrollment URL: {error}"))?;
    let local_development =
        url.scheme() == "http" && matches!(url.host_str(), Some("127.0.0.1" | "::1" | "localhost"));
    if (url.scheme() != "https" && !local_development)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Home enrollment URL must use HTTPS outside loopback development and contain no credentials, query, or fragment"
                .to_string(),
        );
    }
    Ok(url)
}

fn enrollment_bundle_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|path| path.join(ENROLLMENT_BUNDLE_FILE))
        .map_err(|error| format!("resolve Dubhe Node configuration directory: {error}"))
}

fn load_enrollment_bundle(app: &AppHandle) -> Result<Option<SignedHomeEnrollmentBundle>, String> {
    let path = enrollment_bundle_path(app)?;
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("decode saved Home enrollment bundle: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "read saved Home enrollment bundle {}: {error}",
            path.display()
        )),
    }
}

fn save_enrollment_bundle(
    app: &AppHandle,
    bundle: &SignedHomeEnrollmentBundle,
) -> Result<(), String> {
    let path = enrollment_bundle_path(app)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Home enrollment bundle path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create Dubhe Node configuration directory: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(bundle)
        .map_err(|error| format!("encode Home enrollment bundle: {error}"))?;
    fs::write(&temporary, bytes)
        .map_err(|error| format!("write Home enrollment bundle: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("install Home enrollment bundle: {error}"))
}

#[derive(Debug)]
struct ProductionMaterialPaths {
    capacity_certificate: PathBuf,
    relay_ca_certificate: PathBuf,
    relay_client_certificates: Vec<PathBuf>,
    relay_tls_keyring_account: String,
}

fn relay_tls_keyring_account(account: &str) -> String {
    format!("{account}:{RELAY_TLS_KEYRING_SUFFIX}")
}

fn save_certified_bundle_material(
    app: &AppHandle,
    account: &str,
    bundle: &SignedHomeEnrollmentBundle,
) -> Result<ProductionMaterialPaths, String> {
    let certificate = bundle
        .payload
        .capacity_certificate
        .as_ref()
        .ok_or_else(|| {
            "certified Home enrollment bundle has no capacity certificate".to_string()
        })?;
    let relay_credential =
        bundle.payload.relay_credential.as_ref().ok_or_else(|| {
            "certified Home enrollment bundle has no Relay credential".to_string()
        })?;
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("resolve Dubhe Node configuration directory: {error}"))?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("create Dubhe Node configuration directory: {error}"))?;
    let capacity_certificate = directory.join(CAPACITY_CERTIFICATE_FILE);
    write_atomic(
        &capacity_certificate,
        &serde_json::to_vec_pretty(certificate)
            .map_err(|error| format!("encode Home capacity certificate: {error}"))?,
    )?;
    let relay_ca_certificate = directory.join(RELAY_CA_CERTIFICATE_FILE);
    let ca_der = URL_SAFE_NO_PAD
        .decode(&relay_credential.ca_certificate_der)
        .map_err(|_| "decode Home Relay CA certificate".to_string())?;
    write_atomic(&relay_ca_certificate, &ca_der)?;
    let mut relay_client_certificates =
        Vec::with_capacity(relay_credential.certificate_chain_der.len());
    for (index, encoded) in relay_credential.certificate_chain_der.iter().enumerate() {
        let certificate_der = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| "decode Home Relay client certificate".to_string())?;
        let path = if index == 0 {
            directory.join(RELAY_CLIENT_CERTIFICATE_FILE)
        } else {
            directory.join(format!("relay-client-{index}.der"))
        };
        write_atomic(&path, &certificate_der)?;
        relay_client_certificates.push(path);
    }
    Ok(ProductionMaterialPaths {
        capacity_certificate,
        relay_ca_certificate,
        relay_client_certificates,
        relay_tls_keyring_account: relay_tls_keyring_account(account),
    })
}

fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes)
        .map_err(|error| format!("write Dubhe Node credential {}: {error}", path.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("install Dubhe Node credential {}: {error}", path.display()))
}

fn enrollment_status_for(
    config: Option<&EnrollmentClientConfig>,
    bundle: Option<&SignedHomeEnrollmentBundle>,
    node_id: &str,
    public_key: &str,
    renewal: &RenewalRuntime,
) -> DesktopEnrollmentStatus {
    let renewal_fields = || {
        (
            renewal.state.to_string(),
            renewal.last_renewed_at_ms,
            renewal.error.clone(),
        )
    };
    let Some(config) = config else {
        let (renewal_state, last_renewed_at_ms, renewal_error) = renewal_fields();
        return DesktopEnrollmentStatus {
            configured: false,
            enrolled: false,
            capacity_ready: false,
            relay_ready: false,
            enrollment_id: None,
            expires_at_ms: None,
            relay_id: None,
            telemetry_url: None,
            max_sessions: None,
            max_zones: None,
            renewal_state,
            renew_at_ms: None,
            last_renewed_at_ms,
            renewal_error,
            error: Some("尚未配置官方 Enrollment Service".to_string()),
        };
    };
    let Some(bundle) = bundle else {
        let (renewal_state, last_renewed_at_ms, renewal_error) = renewal_fields();
        return DesktopEnrollmentStatus {
            configured: true,
            enrolled: false,
            capacity_ready: false,
            relay_ready: false,
            enrollment_id: None,
            expires_at_ms: None,
            relay_id: None,
            telemetry_url: None,
            max_sessions: None,
            max_zones: None,
            renewal_state,
            renew_at_ms: None,
            last_renewed_at_ms,
            renewal_error,
            error: None,
        };
    };
    let error = bundle
        .verify(
            &config.trusted_issuer_public_key,
            node_id,
            public_key,
            now_ms(),
        )
        .err();
    let (renewal_state, last_renewed_at_ms, renewal_error) = renewal_fields();
    DesktopEnrollmentStatus {
        configured: true,
        enrolled: error.is_none(),
        capacity_ready: error.is_none() && bundle.capacity_ready(),
        relay_ready: error.is_none() && bundle.relay_ready(),
        enrollment_id: Some(bundle.payload.enrollment_id.clone()),
        expires_at_ms: Some(bundle.payload.expires_at_ms),
        relay_id: Some(bundle.payload.relay.relay_id.clone()),
        telemetry_url: Some(bundle.payload.telemetry_url.clone()),
        max_sessions: Some(bundle.payload.resource_policy.max_sessions),
        max_zones: Some(bundle.payload.resource_policy.max_zones),
        renewal_state,
        renew_at_ms: Some(
            credential_expires_at_ms(bundle).saturating_sub(credential_renewal_window_ms()),
        ),
        last_renewed_at_ms,
        renewal_error,
        error,
    }
}

async fn fetch_status() -> Result<SupervisorStatus, String> {
    let endpoint = supervisor_url()?
        .join("/v1/status")
        .map_err(|error| format!("build Supervisor status URL: {error}"))?;
    supervisor_client()?
        .get(endpoint)
        .send()
        .await
        .map_err(|error| format!("connect to local Dubhe Node service: {error}"))?
        .error_for_status()
        .map_err(|error| format!("local Dubhe Node service rejected status: {error}"))?
        .json()
        .await
        .map_err(|error| format!("decode local Dubhe Node status: {error}"))
}

async fn fetch_status_after(previous_observation_ms: u64) -> Result<SupervisorStatus, String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        let status = fetch_status().await?;
        if status.last_observed_at_ms > previous_observation_ms {
            return Ok(status);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("Supervisor did not publish a fresh Zone session count within 10 seconds".to_string())
}

async fn request_supervisor_action(action: &str, management_token: &str) -> Result<(), String> {
    let endpoint = supervisor_url()?
        .join(&format!("/v1/{action}"))
        .map_err(|error| format!("build Supervisor {action} URL: {error}"))?;
    supervisor_client()?
        .post(endpoint)
        .bearer_auth(management_token)
        .send()
        .await
        .map_err(|error| format!("request local Dubhe Node {action}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("local Dubhe Node {action} rejected: {error}"))?;
    Ok(())
}

async fn local_zone_reachable() -> bool {
    let Ok(client) = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_millis(500))
        .build()
    else {
        return true;
    };
    client
        .get("http://127.0.0.1:7021/healthz")
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

async fn stop_supervisor(
    process: &mut Option<Child>,
    management_token: &str,
) -> Result<(), String> {
    let graceful_result = request_supervisor_action("shutdown", management_token).await;
    if graceful_result.is_ok() {
        if let Some(child) = process.as_mut() {
            match tokio::time::timeout(
                Duration::from_secs(SUPERVISOR_GRACEFUL_SHUTDOWN_SECONDS),
                child.wait(),
            )
            .await
            {
                Ok(Ok(_)) => {
                    *process = None;
                    return Ok(());
                }
                Ok(Err(error)) => {
                    return Err(format!("wait for local Dubhe Node shutdown: {error}"));
                }
                Err(_) => {}
            }
        } else {
            let deadline = tokio::time::Instant::now()
                + Duration::from_secs(SUPERVISOR_GRACEFUL_SHUTDOWN_SECONDS);
            while tokio::time::Instant::now() < deadline {
                if fetch_status().await.is_err() && !local_zone_reachable().await {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            return Err(
                "existing Dubhe Node Supervisor did not finish graceful shutdown within 40 seconds"
                    .to_string(),
            );
        }
    }
    let Some(child) = process.as_mut() else {
        return Err(graceful_result
            .err()
            .unwrap_or_else(|| "existing Dubhe Node Supervisor is unresponsive".to_string()));
    };
    child
        .start_kill()
        .map_err(|error| format!("stop unresponsive local Dubhe Node service: {error}"))?;
    child
        .wait()
        .await
        .map_err(|error| format!("reap unresponsive local Dubhe Node service: {error}"))?;
    *process = None;
    Ok(())
}

fn bundled_binary(name: &str) -> Result<PathBuf, String> {
    let directory = env::current_exe()
        .map_err(|error| format!("resolve Dubhe Node executable: {error}"))?
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "Dubhe Node executable has no parent directory".to_string())?;
    #[cfg(windows)]
    let filename = format!("{name}.exe");
    #[cfg(not(windows))]
    let filename = name.to_string();
    let path = directory.join(filename);
    if !path.is_file() {
        return Err(format!(
            "bundled Dubhe Node service is missing: {}",
            path.display()
        ));
    }
    Ok(path)
}

async fn ensure_supervisor(
    app: &AppHandle,
    state: &DesktopState,
    management_token: &str,
    node_id: &str,
    public_key: &str,
    keyring_account: &str,
    enrollment: Option<&SignedHomeEnrollmentBundle>,
    force_restart: bool,
) -> Result<SupervisorStatus, String> {
    let manage_zone = enrollment.is_some();
    let manage_agent = enrollment.is_some_and(SignedHomeEnrollmentBundle::relay_ready);
    let mut process = state.supervisor.lock().await;
    if let Ok(status) = fetch_status().await {
        if !force_restart
            && status.managed_processes == manage_zone
            && status.agent_managed == manage_agent
        {
            return Ok(status);
        }
        stop_supervisor(&mut process, management_token).await?;
    }
    if let Some(child) = process.as_mut() {
        if child
            .try_wait()
            .map_err(|error| format!("inspect local Dubhe Node service: {error}"))?
            .is_some()
        {
            *process = None;
        }
    }
    if process.is_none() {
        let binary = bundled_binary("home_agent_supervisor")?;
        let application_config_directory = app
            .path()
            .app_config_dir()
            .map_err(|error| format!("resolve Dubhe Node configuration directory: {error}"))?;
        fs::create_dir_all(&application_config_directory)
            .map_err(|error| format!("create Dubhe Node configuration directory: {error}"))?;
        let agent_runtime_status = application_config_directory.join(AGENT_RUNTIME_STATUS_FILE);
        let supervisor_log = application_config_directory.join(SUPERVISOR_LOG_FILE);
        if fs::metadata(&supervisor_log)
            .map(|metadata| metadata.len() >= SUPERVISOR_LOG_MAX_BYTES)
            .unwrap_or(false)
        {
            let rotated = supervisor_log.with_extension("log.1");
            let _ = fs::remove_file(&rotated);
            fs::rename(&supervisor_log, &rotated).map_err(|error| {
                format!(
                    "rotate Dubhe Node Supervisor log {}: {error}",
                    supervisor_log.display()
                )
            })?;
        }
        let stdout = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&supervisor_log)
            .map_err(|error| {
                format!(
                    "open Dubhe Node Supervisor log {}: {error}",
                    supervisor_log.display()
                )
            })?;
        let stderr = stdout
            .try_clone()
            .map_err(|error| format!("clone Dubhe Node Supervisor log handle: {error}"))?;
        let mut command = Command::new(&binary);
        command
            .arg("serve")
            .env("MIR2_HOME_MANAGE_CHILDREN", manage_zone.to_string())
            .env("MIR2_HOME_MANAGE_AGENT", manage_agent.to_string())
            .env("MIR2_ZONE_HOST_MANAGEMENT_TOKEN", management_token)
            .env("MIR2_HOME_NODE_ID", node_id)
            .env("MIR2_HOME_NODE_PUBLIC_KEY", public_key)
            .env("MIR2_HOME_AGENT_KEYRING_ACCOUNT", keyring_account)
            .env("MIR2_ZONE_HOST_KEYRING_ACCOUNT", keyring_account)
            .env("MIR2_ZONE_HOST_ID", node_id)
            .env("MIR2_ZONE_HOST_ADDR", "127.0.0.1:7020")
            .env("MIR2_ZONE_HOST_METRICS_ADDR", "127.0.0.1:7021")
            .env("MIR2_HOME_ZONE_OPERATOR_URL", "http://127.0.0.1:7021")
            .env("MIR2_HOME_AGENT_STATUS_FILE", agent_runtime_status)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(true);
        if let Some(bundle) = enrollment {
            command
                .env("MIR2_ZONE_OWNER_LEASE_BACKEND", "trusted-rpc")
                .env("MIR2_ZONE_OWNER_TRUSTED_RPC_OWNERS", node_id)
                .env("MIR2_ZONE_HOST_OWNER_ALIASES", node_id)
                .env(
                    "MIR2_HOME_MAX_CPU_PERCENT",
                    bundle.payload.resource_policy.cpu_limit_percent.to_string(),
                )
                .env(
                    "MIR2_HOME_MIN_AVAILABLE_MEMORY_MIB",
                    (bundle.payload.resource_policy.reserved_memory_bytes / 1024 / 1024)
                        .to_string(),
                )
                .env(
                    "MIR2_ZONE_HOST_MAX_SESSIONS",
                    bundle.payload.resource_policy.max_sessions.to_string(),
                )
                .env(
                    "MIR2_ZONE_HOST_MAX_SESSIONS_PER_ZONE",
                    bundle
                        .payload
                        .resource_policy
                        .max_sessions_per_zone
                        .to_string(),
                )
                .env(
                    "MIR2_ZONE_HOST_MAX_ZONES",
                    bundle.payload.resource_policy.max_zones.to_string(),
                );
            if manage_agent {
                let material = save_certified_bundle_material(app, keyring_account, bundle)?;
                let placement = bundle
                    .payload
                    .placement
                    .as_ref()
                    .ok_or_else(|| "Home Relay placement is missing".to_string())?;
                let certificate_chain = material
                    .relay_client_certificates
                    .iter()
                    .map(|path| path.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(",");
                let allow_insecure_telemetry =
                    bundle.payload.telemetry_url.starts_with("http://127.0.0.1")
                        || bundle.payload.telemetry_url.starts_with("http://localhost");
                command
                    .env("MIR2_HOME_RELAY_ID", &bundle.payload.relay.relay_id)
                    .env("MIR2_HOME_RELAY_ADDR", &bundle.payload.relay.address)
                    .env(
                        "MIR2_HOME_RELAY_SERVER_NAME",
                        &bundle.payload.relay.server_name,
                    )
                    .env("MIR2_HOME_LOCAL_ZONE_RPC_ADDR", "127.0.0.1:7020")
                    .env("MIR2_HOME_AGENT_TLS_CA_DER", &material.relay_ca_certificate)
                    .env("MIR2_HOME_AGENT_TLS_CERT_CHAIN_DER", certificate_chain)
                    .env(
                        "MIR2_HOME_AGENT_TLS_KEY_KEYRING_ACCOUNT",
                        &material.relay_tls_keyring_account,
                    )
                    .env(
                        "MIR2_HOME_AGENT_KEY_GENERATION",
                        bundle.payload.key_generation.to_string(),
                    )
                    .env(
                        "MIR2_HOME_CAPACITY_CERTIFICATE_FILE",
                        &material.capacity_certificate,
                    )
                    .env(
                        "MIR2_HOME_RELAY_PUBLIC_KEY",
                        &bundle.payload.relay.relay_public_key,
                    )
                    .env(
                        "MIR2_HOME_CONTROL_ISSUER_PUBLIC_KEY",
                        &bundle.payload.control_issuer_public_key,
                    )
                    .env("MIR2_HOME_TELEMETRY_URL", &bundle.payload.telemetry_url)
                    .env(
                        "MIR2_HOME_TELEMETRY_ALLOW_INSECURE_LOOPBACK",
                        allow_insecure_telemetry.to_string(),
                    )
                    .env(
                        "MIR2_HOME_COARSE_REGION",
                        env::var("MIR2_HOME_COARSE_REGION")
                            .unwrap_or_else(|_| "desktop-local".to_string()),
                    )
                    .env(
                        "MIR2_HOME_PROVIDER_CODE",
                        env::var("MIR2_HOME_PROVIDER_CODE").unwrap_or_else(|_| "home".to_string()),
                    )
                    .env("MIR2_HOME_RELAY_RTT_MS", "1")
                    .env("MIR2_HOME_PACKET_LOSS_BPS", "0")
                    .env("MIR2_HOME_UPSTREAM_KBPS", "10000")
                    .env("MIR2_HOME_CHECKPOINT_LAG_MS", "0")
                    .env(
                        "MIR2_HOME_PLACEMENT_GENERATION",
                        placement.generation.to_string(),
                    )
                    .env(
                        "MIR2_HOME_GAME_ID",
                        bundle
                            .payload
                            .allowed_games
                            .first()
                            .ok_or_else(|| "Home enrollment has no allowed game".to_string())?,
                    )
                    .env("MIR2_HOME_REWARD_EPOCH", "1");
            }
        }
        let child = command.spawn().map_err(|error| {
            format!(
                "start bundled Dubhe Node service {}: {error}",
                binary.display()
            )
        })?;
        *process = Some(child);
    }
    // The first OS-keyring read by the signed Zone Host can take tens of
    // seconds on macOS while Keychain evaluates the new bundled executable.
    // Keep the desktop bootstrap alive for the Supervisor's 30-second child
    // readiness window plus margin instead of reporting a false startup error.
    for _ in 0..450 {
        if let Ok(status) = fetch_status().await {
            return Ok(status);
        }
        if let Some(child) = process.as_mut() {
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("inspect local Dubhe Node service: {error}"))?
            {
                *process = None;
                return Err(format!(
                    "bundled Dubhe Node service exited during startup with {status}"
                ));
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("bundled Dubhe Node service did not become ready within 45 seconds".to_string())
}

#[tauri::command]
async fn bootstrap_node(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<DesktopBootstrap, String> {
    let account = keyring_account();
    let (identity, created) = HomeAgentKeyring::new(&account)?.load_or_create_identity()?;
    let (management_token, management_token_created) =
        HomeAgentManagementKeyring::new(&account)?.load_or_create_token()?;
    let enrollment_config = enrollment_client_config(&app)?;
    let enrollment_bundle = load_enrollment_bundle(&app)?;
    let renewal = state.renewal.read().await;
    let enrollment_status = enrollment_status_for(
        enrollment_config.as_ref(),
        enrollment_bundle.as_ref(),
        identity.node_id(),
        identity.public_key(),
        &renewal,
    );
    drop(renewal);
    let verified_enrollment = enrollment_status
        .enrolled
        .then_some(enrollment_bundle.as_ref())
        .flatten();
    let status = ensure_supervisor(
        &app,
        &state,
        &management_token,
        identity.node_id(),
        identity.public_key(),
        &account,
        verified_enrollment,
        false,
    )
    .await?;
    Ok(DesktopBootstrap {
        identity: DesktopIdentity {
            node_id: status.node_id.clone(),
            public_key: status.public_key.clone(),
            created,
            key_store: "operating-system-keyring",
        },
        supervisor_reachable: true,
        status: Some(status),
        management_token_created,
    })
}

#[tauri::command]
async fn node_status() -> Result<SupervisorStatus, String> {
    fetch_status().await
}

#[tauri::command]
async fn set_node_serving(
    serving: bool,
    state: State<'_, DesktopState>,
) -> Result<NodeActionReceipt, String> {
    if serving && matches!(state.renewal.read().await.state, "draining" | "renewing") {
        return Err(
            "credential renewal is draining the node; new Sessions remain closed until rotation completes"
                .to_string(),
        );
    }
    let account = keyring_account();
    let token = HomeAgentManagementKeyring::new(account)?.load_token()?;
    let action = if serving { "resume" } else { "drain" };
    let endpoint = supervisor_url()?
        .join(&format!("/v1/{action}"))
        .map_err(|error| format!("build Supervisor action URL: {error}"))?;
    let response = supervisor_client()?
        .post(endpoint)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| format!("request local Dubhe Node {action}: {error}"))?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(format!("local Dubhe Node {action} rejected with {status}"));
    }
    let status = fetch_status().await?;
    Ok(NodeActionReceipt {
        accepted: true,
        serving,
        status,
    })
}

#[tauri::command]
async fn enrollment_status(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<DesktopEnrollmentStatus, String> {
    let identity = HomeAgentKeyring::new(keyring_account())?.load_identity()?;
    let config = enrollment_client_config(&app)?;
    let bundle = load_enrollment_bundle(&app)?;
    let renewal = state.renewal.read().await;
    Ok(enrollment_status_for(
        config.as_ref(),
        bundle.as_ref(),
        identity.node_id(),
        identity.public_key(),
        &renewal,
    ))
}

fn enrollment_http_client(timeout: Duration) -> Result<Client, String> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(timeout)
        .build()
        .map_err(|error| format!("build Home enrollment client: {error}"))
}

async fn request_enrollment_bundle(
    config: &EnrollmentClientConfig,
    identity: &NodeSigningIdentity,
) -> Result<SignedHomeEnrollmentBundle, String> {
    let client = enrollment_http_client(Duration::from_secs(10))?;
    let challenge_url = config
        .base_url
        .join("/v1/challenges")
        .map_err(|error| format!("build Home enrollment challenge URL: {error}"))?;
    let challenge = client
        .post(challenge_url)
        .json(&serde_json::json!({
            "nodeId": identity.node_id(),
            "publicKey": identity.public_key(),
        }))
        .send()
        .await
        .map_err(|error| format!("request Home enrollment challenge: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Home enrollment challenge rejected: {error}"))?
        .json::<SignedHomeEnrollmentChallenge>()
        .await
        .map_err(|error| format!("decode Home enrollment challenge: {error}"))?;
    let request = HomeEnrollmentRequest::sign(
        challenge,
        identity,
        &config.trusted_issuer_public_key,
        now_ms(),
    )?;
    let enrollment_url = config
        .base_url
        .join("/v1/enrollments")
        .map_err(|error| format!("build Home enrollment completion URL: {error}"))?;
    let response = client
        .post(enrollment_url)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("complete Home enrollment: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Home enrollment rejected: {error}"))?
        .json::<EnrollmentResponse>()
        .await
        .map_err(|error| format!("decode Home enrollment response: {error}"))?;
    if !response.accepted {
        return Err("Home enrollment service did not accept this node".to_string());
    }
    response.bundle.verify(
        &config.trusted_issuer_public_key,
        identity.node_id(),
        identity.public_key(),
        now_ms(),
    )?;
    Ok(response.bundle)
}

#[tauri::command]
async fn enroll_node(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<DesktopEnrollmentStatus, String> {
    let _maintenance = state.credential_maintenance.lock().await;
    let account = keyring_account();
    let identity = HomeAgentKeyring::new(&account)?.load_identity()?;
    let config = enrollment_client_config(&app)?
        .ok_or_else(|| "尚未配置官方 Enrollment Service".to_string())?;
    let bundle = request_enrollment_bundle(&config, &identity).await?;
    save_enrollment_bundle(&app, &bundle)?;
    let management_token = HomeAgentManagementKeyring::new(&account)?.load_token()?;
    ensure_supervisor(
        &app,
        &state,
        &management_token,
        identity.node_id(),
        identity.public_key(),
        &account,
        Some(&bundle),
        false,
    )
    .await?;
    let renewal = state.renewal.read().await;
    Ok(enrollment_status_for(
        Some(&config),
        Some(&bundle),
        identity.node_id(),
        identity.public_key(),
        &renewal,
    ))
}

fn create_relay_certificate_request(
    node_id: &str,
    existing_key: Option<&[u8]>,
) -> Result<(Vec<u8>, String), String> {
    let key = match existing_key {
        Some(key) => KeyPair::try_from(key)
            .map_err(|error| format!("decode existing Home Relay client key: {error}"))?,
        None => KeyPair::generate()
            .map_err(|error| format!("generate Home Relay client key: {error}"))?,
    };
    let mut params = CertificateParams::new(Vec::<String>::new())
        .map_err(|error| format!("create Home Relay certificate request: {error}"))?;
    params.is_ca = IsCa::NoCa;
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::CommonName, node_id);
    let request = params
        .serialize_request(&key)
        .map_err(|error| format!("sign Home Relay certificate request: {error}"))?;
    Ok((
        key.serialize_der(),
        URL_SAFE_NO_PAD.encode(request.der().as_ref()),
    ))
}

async fn request_capacity_certification(
    config: &EnrollmentClientConfig,
    identity: &NodeSigningIdentity,
    enrollment: SignedHomeEnrollmentBundle,
    existing_relay_key: Option<&[u8]>,
) -> Result<(SignedHomeEnrollmentBundle, Vec<u8>), String> {
    enrollment.verify(
        &config.trusted_issuer_public_key,
        identity.node_id(),
        identity.public_key(),
        now_ms(),
    )?;
    if !fetch_status().await?.zone_reachable {
        return Err("本地 Zone Host 尚未就绪，无法执行容量挑战".to_string());
    }
    let client = enrollment_http_client(Duration::from_secs(45))?;
    let challenge_url = config
        .base_url
        .join("/v1/capacity/challenges")
        .map_err(|error| format!("build Home capacity challenge URL: {error}"))?;
    let challenge = client
        .post(challenge_url)
        .json(&enrollment)
        .send()
        .await
        .map_err(|error| format!("request Home capacity challenge: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Home capacity challenge rejected: {error}"))?
        .json::<CapacityChallenge>()
        .await
        .map_err(|error| format!("decode Home capacity challenge: {error}"))?;
    let response = client
        .post("http://127.0.0.1:7021/v1/capacity-challenge")
        .json(&challenge)
        .send()
        .await
        .map_err(|error| format!("run local Home capacity challenge: {error}"))?
        .error_for_status()
        .map_err(|error| format!("local Home capacity challenge rejected: {error}"))?
        .json::<CapacityChallengeResponse>()
        .await
        .map_err(|error| format!("decode signed Home capacity response: {error}"))?;
    response.verify_node_claim(now_ms())?;
    let (relay_private_key, certificate_signing_request_der) =
        create_relay_certificate_request(identity.node_id(), existing_relay_key)?;
    let certification_url = config
        .base_url
        .join("/v1/capacity/certifications")
        .map_err(|error| format!("build Home capacity certification URL: {error}"))?;
    let certification = client
        .post(certification_url)
        .json(&HomeCapacityCertificationRequest {
            enrollment,
            response,
            certificate_signing_request_der,
        })
        .send()
        .await
        .map_err(|error| format!("complete Home capacity certification: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Home capacity certification rejected: {error}"))?
        .json::<EnrollmentResponse>()
        .await
        .map_err(|error| format!("decode Home capacity certification response: {error}"))?;
    if !certification.accepted {
        return Err("Home capacity authority did not accept this node".to_string());
    }
    certification.bundle.verify(
        &config.trusted_issuer_public_key,
        identity.node_id(),
        identity.public_key(),
        now_ms(),
    )?;
    if !certification.bundle.relay_ready() {
        return Err(
            "Home capacity authority returned an incomplete production admission bundle"
                .to_string(),
        );
    }
    Ok((certification.bundle, relay_private_key))
}

#[tauri::command]
async fn certify_node(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<DesktopEnrollmentStatus, String> {
    let _maintenance = state.credential_maintenance.lock().await;
    let account = keyring_account();
    let identity = HomeAgentKeyring::new(&account)?.load_identity()?;
    let config = enrollment_client_config(&app)?
        .ok_or_else(|| "尚未配置官方 Enrollment Service".to_string())?;
    let enrollment =
        load_enrollment_bundle(&app)?.ok_or_else(|| "请先完成节点签名 Enrollment".to_string())?;
    let (bundle, relay_private_key) =
        request_capacity_certification(&config, &identity, enrollment, None).await?;
    HomeAgentKeyring::new(relay_tls_keyring_account(&account))?.store_secret(&relay_private_key)?;
    save_certified_bundle_material(&app, &account, &bundle)?;
    save_enrollment_bundle(&app, &bundle)?;
    let management_token = HomeAgentManagementKeyring::new(&account)?.load_token()?;
    ensure_supervisor(
        &app,
        &state,
        &management_token,
        identity.node_id(),
        identity.public_key(),
        &account,
        Some(&bundle),
        false,
    )
    .await?;
    let renewal = state.renewal.read().await;
    Ok(enrollment_status_for(
        Some(&config),
        Some(&bundle),
        identity.node_id(),
        identity.public_key(),
        &renewal,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenewalDecision {
    Current,
    Drain,
    Renew,
}

fn renewal_decision(
    expires_at_ms: u64,
    observed_at_ms: u64,
    active_sessions: usize,
    renewal_window_ms: u64,
    force: bool,
) -> RenewalDecision {
    if !force && expires_at_ms.saturating_sub(observed_at_ms) > renewal_window_ms {
        RenewalDecision::Current
    } else if active_sessions > 0 {
        RenewalDecision::Drain
    } else {
        RenewalDecision::Renew
    }
}

fn credential_expires_at_ms(bundle: &SignedHomeEnrollmentBundle) -> u64 {
    let mut expires_at_ms = bundle.payload.expires_at_ms;
    if let Some(certificate) = bundle.payload.capacity_certificate.as_ref() {
        expires_at_ms = expires_at_ms.min(certificate.expires_at_ms);
    }
    if let Some(placement) = bundle.payload.placement.as_ref() {
        expires_at_ms = expires_at_ms.min(placement.expires_at_ms);
    }
    if let Some(credential) = bundle.payload.relay_credential.as_ref() {
        expires_at_ms = expires_at_ms.min(credential.expires_at_ms);
    }
    expires_at_ms
}

fn preserve_drain_after_restart(status: &SupervisorStatus, renewal_draining: bool) -> bool {
    !renewal_draining
        && !status.accept_new_sessions
        && matches!(
            status.reason.as_str(),
            "manual_drain" | "host_resource_pressure" | "system_sleep_or_resume_detected"
        )
}

fn duration_env_ms(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value >= 1_000)
        .unwrap_or(default)
}

fn credential_renewal_window_ms() -> u64 {
    duration_env_ms(
        "MIR2_HOME_CREDENTIAL_RENEWAL_WINDOW_MS",
        DEFAULT_CREDENTIAL_RENEWAL_WINDOW_MS,
    )
}

fn credential_renewal_poll_ms() -> u64 {
    duration_env_ms(
        "MIR2_HOME_CREDENTIAL_RENEWAL_POLL_MS",
        DEFAULT_CREDENTIAL_RENEWAL_POLL_MS,
    )
}

async fn set_renewal_state(
    state: &DesktopState,
    renewal_state: &'static str,
    error: Option<String>,
) {
    let mut renewal = state.renewal.write().await;
    renewal.state = renewal_state;
    renewal.error = error;
}

async fn maintain_credentials(
    app: &AppHandle,
    state: &DesktopState,
    force: bool,
) -> Result<bool, String> {
    let _maintenance = state.credential_maintenance.lock().await;
    let account = keyring_account();
    let identity = HomeAgentKeyring::new(&account)?.load_identity()?;
    let Some(config) = enrollment_client_config(app)? else {
        set_renewal_state(state, "not-configured", None).await;
        return Ok(false);
    };
    let Some(bundle) = load_enrollment_bundle(app)? else {
        set_renewal_state(state, "awaiting-enrollment", None).await;
        return Ok(false);
    };
    if !bundle.relay_ready() {
        set_renewal_state(state, "awaiting-certification", None).await;
        return Ok(false);
    }
    let mut status = fetch_status().await?;
    let bundle_valid = bundle
        .verify(
            &config.trusted_issuer_public_key,
            identity.node_id(),
            identity.public_key(),
            now_ms(),
        )
        .is_ok();
    let expires_at_ms = credential_expires_at_ms(&bundle);
    match renewal_decision(
        expires_at_ms,
        now_ms(),
        status.active_sessions,
        credential_renewal_window_ms(),
        force || !bundle_valid,
    ) {
        RenewalDecision::Current => {
            set_renewal_state(state, "current", None).await;
            return Ok(false);
        }
        RenewalDecision::Drain => {
            let management_token = HomeAgentManagementKeyring::new(&account)?.load_token()?;
            request_supervisor_action("drain", &management_token).await?;
            let mut renewal = state.renewal.write().await;
            renewal.state = "draining";
            renewal.error = None;
            renewal.renewal_draining = true;
            return Ok(false);
        }
        RenewalDecision::Renew => {}
    }

    let management_token = HomeAgentManagementKeyring::new(&account)?.load_token()?;
    if status.accept_new_sessions {
        let previous_observation_ms = status.last_observed_at_ms;
        request_supervisor_action("drain", &management_token).await?;
        {
            let mut renewal = state.renewal.write().await;
            renewal.state = "draining";
            renewal.error = None;
            renewal.renewal_draining = true;
        }
        status = fetch_status_after(previous_observation_ms).await?;
        if status.active_sessions > 0 {
            return Ok(false);
        }
    }
    set_renewal_state(state, "renewing", None).await;
    let relay_keyring = HomeAgentKeyring::new(relay_tls_keyring_account(&account))?;
    let relay_private_key = relay_keyring.load_secret()?;
    let fresh_enrollment = request_enrollment_bundle(&config, &identity).await?;
    if !status.zone_reachable {
        ensure_supervisor(
            app,
            state,
            &management_token,
            identity.node_id(),
            identity.public_key(),
            &account,
            Some(&fresh_enrollment),
            true,
        )
        .await?;
    }
    let (certified_bundle, returned_relay_key) = request_capacity_certification(
        &config,
        &identity,
        fresh_enrollment,
        Some(&relay_private_key),
    )
    .await?;
    if returned_relay_key != relay_private_key {
        return Err(
            "Home Relay renewal unexpectedly replaced the existing private key".to_string(),
        );
    }
    save_certified_bundle_material(app, &account, &certified_bundle)?;
    save_enrollment_bundle(app, &certified_bundle)?;
    let renewal_draining = state.renewal.read().await.renewal_draining;
    let restore_drain = preserve_drain_after_restart(&status, renewal_draining);
    ensure_supervisor(
        app,
        state,
        &management_token,
        identity.node_id(),
        identity.public_key(),
        &account,
        Some(&certified_bundle),
        true,
    )
    .await?;
    if restore_drain {
        request_supervisor_action("drain", &management_token).await?;
    }
    let mut renewal = state.renewal.write().await;
    renewal.state = "current";
    renewal.last_renewed_at_ms = Some(now_ms());
    renewal.error = None;
    renewal.renewal_draining = false;
    Ok(true)
}

async fn credential_renewal_loop(app: AppHandle) {
    let interval = Duration::from_millis(credential_renewal_poll_ms());
    loop {
        tokio::time::sleep(interval).await;
        let state = app.state::<DesktopState>();
        if let Err(error) = maintain_credentials(&app, &state, false).await {
            let mut renewal = state.renewal.write().await;
            renewal.state = "failed";
            renewal.error = Some(error);
        }
    }
}

#[tauri::command]
async fn renew_node_credentials(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<DesktopEnrollmentStatus, String> {
    if let Err(error) = maintain_credentials(&app, &state, true).await {
        set_renewal_state(&state, "failed", Some(error.clone())).await;
        return Err(error);
    }
    let account = keyring_account();
    let identity = HomeAgentKeyring::new(&account)?.load_identity()?;
    let config = enrollment_client_config(&app)?;
    let bundle = load_enrollment_bundle(&app)?;
    let renewal = state.renewal.read().await;
    Ok(enrollment_status_for(
        config.as_ref(),
        bundle.as_ref(),
        identity.node_id(),
        identity.public_key(),
        &renewal,
    ))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(DesktopState::default())
        .manage(desktop_runtime::DesktopRuntimeState::default())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            desktop_runtime::setup(app)?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                credential_renewal_loop(handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap_node,
            node_status,
            set_node_serving,
            enrollment_status,
            enroll_node,
            certify_node,
            renew_node_credentials,
            desktop_runtime::desktop_preferences,
            desktop_runtime::set_desktop_preferences,
            desktop_runtime::check_for_desktop_update,
            desktop_runtime::install_desktop_update,
            desktop_runtime::desktop_recovery_status,
            desktop_runtime::rollback_desktop_update,
            desktop_runtime::export_diagnostics,
            desktop_runtime::prepare_uninstall
        ])
        .on_window_event(desktop_runtime::handle_window_event)
        .run(tauri::generate_context!())
        .expect("error while running Dubhe Node desktop application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_url_is_strictly_loopback() {
        assert!(validate_supervisor_url("http://127.0.0.1:17990").is_ok());
        assert!(validate_supervisor_url("http://localhost:17990").is_ok());
        assert!(validate_supervisor_url("https://127.0.0.1:17990").is_err());
        assert!(validate_supervisor_url("http://192.168.1.10:17990").is_err());
        assert!(validate_supervisor_url("http://user:secret@localhost:17990").is_err());
        assert!(validate_supervisor_url("http://localhost:17990?token=secret").is_err());
    }

    #[test]
    fn enrollment_url_requires_https_except_for_loopback() {
        assert!(validate_enrollment_url("https://enrollment.obelisk.game").is_ok());
        assert!(validate_enrollment_url("http://127.0.0.1:18080").is_ok());
        assert!(validate_enrollment_url("http://localhost:18080").is_ok());
        assert!(validate_enrollment_url("http://enrollment.obelisk.game").is_err());
        assert!(validate_enrollment_url("https://user@example.com").is_err());
    }

    #[test]
    fn renewal_waits_for_sessions_and_rotates_inside_window() {
        let expires_at_ms = 10_000;
        assert_eq!(
            renewal_decision(expires_at_ms, 1_000, 0, 2_000, false),
            RenewalDecision::Current
        );
        assert_eq!(
            renewal_decision(expires_at_ms, 9_000, 3, 2_000, false),
            RenewalDecision::Drain
        );
        assert_eq!(
            renewal_decision(expires_at_ms, 9_000, 0, 2_000, false),
            RenewalDecision::Renew
        );
        assert_eq!(
            renewal_decision(expires_at_ms, 1_000, 0, 2_000, true),
            RenewalDecision::Renew
        );
    }

    #[test]
    fn relay_certificate_renewal_reuses_private_key() {
        let (key, first_request) =
            create_relay_certificate_request("node-test", None).expect("create first request");
        let (reused, second_request) =
            create_relay_certificate_request("node-test", Some(&key)).expect("reuse client key");
        assert_eq!(reused, key);
        assert!(!first_request.is_empty());
        assert!(!second_request.is_empty());
    }
}
