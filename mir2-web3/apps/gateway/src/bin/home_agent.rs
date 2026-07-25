use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mir2_gateway::{
    HomeAgentKeyring, HomeAgentWorkMode, HomeNodeTelemetryPayload, HomeTunnelAgent,
    HomeTunnelAgentConfig, HomeTunnelTlsMaterial, NodeCapacityCertificate, NodeSigningIdentity,
    SignedHomeNodeTelemetry, ZoneHostTelemetrySnapshot, HOME_TELEMETRY_SCHEMA,
};
use sysinfo::System;
use tokio::sync::watch;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("HOME_AGENT_FATAL {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    if env::args().nth(1).as_deref() == Some("telemetry-once") {
        return emit_telemetry_once().await;
    }
    let relay_id = required_env("MIR2_HOME_RELAY_ID")?;
    let relay_addr = resolve_socket_env("MIR2_HOME_RELAY_ADDR").await?;
    let relay_server_name = required_env("MIR2_HOME_RELAY_SERVER_NAME")?;
    let local_zone_rpc_addr = resolve_socket_env("MIR2_HOME_LOCAL_ZONE_RPC_ADDR").await?;
    let tls = tls_from_env("MIR2_HOME_AGENT")?;
    let node_identity = signing_identity_from_env(
        "MIR2_HOME_AGENT_SIGNING_KEY",
        "MIR2_HOME_AGENT_SIGNING_KEY_FILE",
    )?;
    let key_generation = positive_u64_env("MIR2_HOME_AGENT_KEY_GENERATION")?;
    let agent_instance_id = agent_instance_id();
    let registration_sequence = env::var("MIR2_HOME_AGENT_REGISTRATION_SEQUENCE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|_| positive_u64_env("MIR2_HOME_AGENT_REGISTRATION_SEQUENCE"))
        .transpose()?
        .unwrap_or(1);
    let capacity_certificate = read_capacity_certificate()?;
    let trusted_relay_issuer = required_env("MIR2_HOME_RELAY_PUBLIC_KEY")?;
    let trusted_control_issuer = required_env("MIR2_HOME_CONTROL_ISSUER_PUBLIC_KEY")?;
    let telemetry = HomeTelemetryEmitter::from_env(
        node_identity.clone(),
        key_generation,
        agent_instance_id.clone(),
        capacity_certificate.clone(),
    )?;
    let agent = HomeTunnelAgent::connect(HomeTunnelAgentConfig::with_defaults(
        relay_id,
        relay_addr,
        relay_server_name,
        local_zone_rpc_addr,
        tls,
        node_identity,
        key_generation,
        agent_instance_id,
        registration_sequence,
        capacity_certificate,
        trusted_relay_issuer,
        trusted_control_issuer,
    ))
    .await?;
    println!(
        "HOME_AGENT_READY relay={} local_zone_rpc={local_zone_rpc_addr}",
        required_env("MIR2_HOME_RELAY_ID")?
    );
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });
    let telemetry_shutdown = shutdown_rx.clone();
    let agent_serve = agent.serve(shutdown_rx);
    tokio::pin!(agent_serve);
    if let Some(telemetry) = telemetry {
        let mut telemetry_task = tokio::spawn(telemetry.serve(telemetry_shutdown));
        tokio::select! {
            result = &mut agent_serve => {
                telemetry_task.abort();
                result
            }
            result = &mut telemetry_task => {
                result
                    .map_err(|error| format!("Home telemetry task join failed: {error}"))?
                    .and_then(|()| Err("Home telemetry task stopped while tunnel remained active".to_string()))
            }
        }
    } else {
        agent_serve.await
    }
}

async fn emit_telemetry_once() -> Result<(), String> {
    let identity = signing_identity_from_env(
        "MIR2_HOME_AGENT_SIGNING_KEY",
        "MIR2_HOME_AGENT_SIGNING_KEY_FILE",
    )?;
    let emitter = HomeTelemetryEmitter::from_env(
        identity,
        positive_u64_env("MIR2_HOME_AGENT_KEY_GENERATION")?,
        agent_instance_id(),
        read_capacity_certificate()?,
    )?
    .ok_or_else(|| "MIR2_HOME_TELEMETRY_URL is required for telemetry-once".to_string())?;
    let client = telemetry_client()?;
    let mut system = System::new_all();
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    emitter
        .emit(&client, &mut system, 1, now_ms().saturating_sub(1), 0)
        .await?;
    println!("HOME_TELEMETRY_EMITTED_ONCE");
    Ok(())
}

fn read_capacity_certificate() -> Result<NodeCapacityCertificate, String> {
    let certificate_path = PathBuf::from(required_env("MIR2_HOME_CAPACITY_CERTIFICATE_FILE")?);
    serde_json::from_slice(&std::fs::read(&certificate_path).map_err(|error| {
        format!(
            "read Home Tunnel capacity certificate {}: {error}",
            certificate_path.display()
        )
    })?)
    .map_err(|error| {
        format!(
            "decode Home Tunnel capacity certificate {}: {error}",
            certificate_path.display()
        )
    })
}

fn agent_instance_id() -> String {
    env::var("MIR2_HOME_AGENT_INSTANCE_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "agent-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            )
        })
}

struct HomeTelemetryEmitter {
    endpoint: String,
    zone_operator_url: String,
    identity: NodeSigningIdentity,
    key_generation: u64,
    agent_instance_id: String,
    capacity_certificate: NodeCapacityCertificate,
    coarse_region: String,
    provider_code: String,
    relay_rtt_ms: u32,
    packet_loss_bps: u16,
    measured_upstream_kbps: u32,
    checkpoint_lag_ms: u32,
    placement_generation: u64,
    game_id: String,
    reward_epoch: u64,
    interval: Duration,
}

impl HomeTelemetryEmitter {
    fn from_env(
        identity: NodeSigningIdentity,
        key_generation: u64,
        agent_instance_id: String,
        capacity_certificate: NodeCapacityCertificate,
    ) -> Result<Option<Self>, String> {
        let Some(endpoint) = env::var("MIR2_HOME_TELEMETRY_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(None);
        };
        validate_telemetry_url(&endpoint)?;
        if capacity_certificate.node_id != identity.node_id()
            || capacity_certificate.public_key != identity.public_key()
            || capacity_certificate.key_generation != key_generation
        {
            return Err(
                "Home telemetry capacity certificate does not match the Agent identity".to_string(),
            );
        }
        let zone_operator_url = required_env("MIR2_HOME_ZONE_OPERATOR_URL")?;
        validate_loopback_http_url("MIR2_HOME_ZONE_OPERATOR_URL", &zone_operator_url)?;
        Ok(Some(Self {
            endpoint,
            zone_operator_url,
            identity,
            key_generation,
            agent_instance_id,
            capacity_certificate,
            coarse_region: required_env("MIR2_HOME_COARSE_REGION")?,
            provider_code: required_env("MIR2_HOME_PROVIDER_CODE")?,
            relay_rtt_ms: parse_env("MIR2_HOME_RELAY_RTT_MS")?,
            packet_loss_bps: parse_env("MIR2_HOME_PACKET_LOSS_BPS")?,
            measured_upstream_kbps: parse_env("MIR2_HOME_UPSTREAM_KBPS")?,
            checkpoint_lag_ms: parse_env("MIR2_HOME_CHECKPOINT_LAG_MS")?,
            placement_generation: positive_u64_env("MIR2_HOME_PLACEMENT_GENERATION")?,
            game_id: required_env("MIR2_HOME_GAME_ID")?,
            reward_epoch: positive_u64_env("MIR2_HOME_REWARD_EPOCH")?,
            interval: Duration::from_secs(
                optional_positive_u64_env("MIR2_HOME_TELEMETRY_INTERVAL_SECONDS")?
                    .unwrap_or(30)
                    .clamp(5, 300),
            ),
        }))
    }

    async fn serve(self, mut shutdown: watch::Receiver<bool>) -> Result<(), String> {
        let client = telemetry_client()?;
        let mut system = System::new_all();
        tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
        let mut sequence = 1_u64;
        let mut window_started_at_ms = now_ms().saturating_sub(1);
        let mut session_milliseconds = 0_u64;
        loop {
            if *shutdown.borrow() {
                return Ok(());
            }
            let emitted = self
                .emit(
                    &client,
                    &mut system,
                    sequence,
                    window_started_at_ms,
                    session_milliseconds,
                )
                .await?;
            sequence = sequence.saturating_add(1);
            window_started_at_ms = emitted.0;
            session_milliseconds = emitted.1;
            tokio::select! {
                changed = shutdown.changed() => {
                    changed.map_err(|_| "Home telemetry shutdown channel closed".to_string())?;
                }
                () = tokio::time::sleep(self.interval) => {}
            }
        }
    }

    async fn emit(
        &self,
        client: &reqwest::Client,
        system: &mut System,
        sequence: u64,
        window_started_at_ms: u64,
        session_milliseconds: u64,
    ) -> Result<(u64, u64), String> {
        system.refresh_cpu_usage();
        system.refresh_memory();
        let snapshot = fetch_zone_telemetry(client, &self.zone_operator_url).await?;
        let observed_at_ms = now_ms();
        let elapsed_ms = observed_at_ms.saturating_sub(window_started_at_ms);
        let session_milliseconds = session_milliseconds
            .saturating_add(elapsed_ms.saturating_mul(snapshot.health.session_count as u64));
        let zone_ids = snapshot
            .zones
            .iter()
            .map(|zone| zone.zone_id.clone())
            .collect::<Vec<_>>();
        let memory_usage_bps = if system.total_memory() == 0 {
            0
        } else {
            system
                .used_memory()
                .saturating_mul(10_000)
                .checked_div(system.total_memory())
                .unwrap_or_default()
                .min(10_000) as u16
        };
        let payload = HomeNodeTelemetryPayload {
            schema: HOME_TELEMETRY_SCHEMA.to_string(),
            node_id: self.identity.node_id().to_string(),
            public_key: self.identity.public_key().to_string(),
            key_generation: self.key_generation,
            agent_instance_id: self.agent_instance_id.clone(),
            sequence,
            window_started_at_ms,
            observed_at_ms,
            coarse_region: self.coarse_region.clone(),
            provider_code: self.provider_code.clone(),
            relay_rtt_ms: self.relay_rtt_ms,
            packet_loss_bps: self.packet_loss_bps,
            measured_upstream_kbps: self.measured_upstream_kbps,
            active_sessions: snapshot.health.session_count.min(u32::MAX as usize) as u32,
            active_zones: zone_ids.len().min(u16::MAX as usize) as u16,
            zone_ids,
            checkpoint_lag_ms: self.checkpoint_lag_ms,
            cpu_usage_bps: (system.global_cpu_usage() * 100.0).clamp(0.0, 10_000.0) as u16,
            memory_usage_bps,
            work_mode: if snapshot.health.draining {
                HomeAgentWorkMode::Draining
            } else {
                HomeAgentWorkMode::Serving
            },
            capacity_certificate_id: self.capacity_certificate.certificate_id.clone(),
            capacity_certificate_expires_at_ms: self.capacity_certificate.expires_at_ms,
            capacity_max_sessions: self
                .capacity_certificate
                .max_sessions
                .min(u32::MAX as usize) as u32,
            capacity_max_zones: self.capacity_certificate.max_zones.min(u16::MAX as usize) as u16,
            finalized_control_height: self.capacity_certificate.finalized_control_height,
            placement_generation: self.placement_generation,
            game_id: self.game_id.clone(),
            reward_epoch: self.reward_epoch,
            // The Home Agent never promotes local counters into billable work.
            // Quorum receipts are reconciled independently by Gate 25.
            verified_work_units: 0,
            session_milliseconds,
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
        };
        let report = SignedHomeNodeTelemetry::sign(payload, &self.identity)?;
        client
            .post(&self.endpoint)
            .timeout(Duration::from_secs(10))
            .json(&report)
            .send()
            .await
            .map_err(|error| format!("submit Home telemetry: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Home telemetry collector rejected report: {error}"))?;
        Ok((observed_at_ms, session_milliseconds))
    }
}

fn telemetry_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("build Home telemetry client: {error}"))
}

async fn fetch_zone_telemetry(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<ZoneHostTelemetrySnapshot, String> {
    client
        .get(format!("{}/healthz", base_url.trim_end_matches('/')))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map_err(|error| format!("query Home Zone telemetry: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Home Zone telemetry HTTP status rejected: {error}"))?
        .json()
        .await
        .map_err(|error| format!("decode Home Zone telemetry: {error}"))
}

fn validate_telemetry_url(value: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value)
        .map_err(|error| format!("invalid MIR2_HOME_TELEMETRY_URL: {error}"))?;
    let secure = url.scheme() == "https";
    let loopback_development = url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "::1" | "localhost"))
        && boolean_env("MIR2_HOME_TELEMETRY_ALLOW_INSECURE_LOOPBACK", false)?;
    if (!secure && !loopback_development)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Home telemetry URL must use HTTPS without credentials or fragments".to_string(),
        );
    }
    Ok(())
}

fn validate_loopback_http_url(name: &str, value: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value).map_err(|error| format!("invalid {name}: {error}"))?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("127.0.0.1" | "::1" | "localhost"))
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(format!("{name} must use loopback HTTP without credentials"));
    }
    Ok(())
}

fn tls_from_env(prefix: &str) -> Result<HomeTunnelTlsMaterial, String> {
    let ca = required_env(&format!("{prefix}_TLS_CA_DER"))?;
    let chain = required_env(&format!("{prefix}_TLS_CERT_CHAIN_DER"))?
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if chain.is_empty() {
        return Err(format!("{prefix}_TLS_CERT_CHAIN_DER must not be empty"));
    }
    let key_path = env::var(format!("{prefix}_TLS_KEY_DER"))
        .ok()
        .filter(|value| !value.trim().is_empty());
    let keyring_account = env::var(format!("{prefix}_TLS_KEY_KEYRING_ACCOUNT"))
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (key_path, keyring_account) {
        (Some(_), Some(_)) => Err(format!(
            "configure only one of {prefix}_TLS_KEY_DER or {prefix}_TLS_KEY_KEYRING_ACCOUNT"
        )),
        (Some(key), None) => HomeTunnelTlsMaterial::from_der_files(ca, &chain, key),
        (None, Some(account)) => {
            let ca_certificate_der = std::fs::read(&ca)
                .map_err(|error| format!("read Home Tunnel CA certificate {ca}: {error}"))?;
            let certificate_chain_der = chain
                .iter()
                .map(|path| {
                    std::fs::read(path).map_err(|error| {
                        format!("read Home Tunnel certificate {}: {error}", path.display())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let private_key_pkcs8_der = HomeAgentKeyring::new(account)?.load_secret()?;
            Ok(HomeTunnelTlsMaterial {
                ca_certificate_der,
                certificate_chain_der,
                private_key_pkcs8_der,
            })
        }
        (None, None) => Err(format!(
            "{prefix}_TLS_KEY_DER or {prefix}_TLS_KEY_KEYRING_ACCOUNT is required"
        )),
    }
}

fn signing_identity_from_env(
    inline_name: &str,
    file_name: &str,
) -> Result<NodeSigningIdentity, String> {
    let inline = env::var(inline_name)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let file = env::var(file_name)
        .ok()
        .filter(|value| !value.trim().is_empty());
    match (inline, file) {
        (Some(_), Some(_)) => Err(format!(
            "configure only one of {inline_name} or {file_name}"
        )),
        (Some(value), None) => NodeSigningIdentity::from_base64_seed(&value),
        (None, Some(path)) => NodeSigningIdentity::from_file(path),
        (None, None) => {
            let account = env::var("MIR2_HOME_AGENT_KEYRING_ACCOUNT")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "default".to_string());
            HomeAgentKeyring::new(account)?.load_identity().map_err(|error| {
                format!(
                    "{inline_name} or {file_name} is required unless the OS keyring contains an identity: {error}"
                )
            })
        }
    }
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn positive_u64_env(name: &str) -> Result<u64, String> {
    required_env(name)?
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{name} must be a positive integer"))
}

fn optional_positive_u64_env(name: &str) -> Result<Option<u64>, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("{name} must be a positive integer"))
        })
        .transpose()
}

fn parse_env<T>(name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    required_env(name)?
        .parse::<T>()
        .map_err(|_| format!("{name} contains an invalid value"))
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

async fn resolve_socket_env(name: &str) -> Result<SocketAddr, String> {
    let value = required_env(name)?;
    let address = tokio::net::lookup_host(&value)
        .await
        .map_err(|error| format!("resolve {name}={value}: {error}"))?
        .next()
        .ok_or_else(|| format!("{name}={value} resolved to no addresses"))?;
    Ok(address)
}
