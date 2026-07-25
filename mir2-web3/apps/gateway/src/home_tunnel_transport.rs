use std::collections::BTreeMap;
use std::fmt;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{Connection, Endpoint, RecvStream, SendStream};
use rand::rngs::OsRng;
use rand::RngCore;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ClientConfig as RustlsClientConfig, RootCertStore};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, RwLock};

use crate::{
    decode_zone_rpc_routing_hint, HomeTunnelChallenge, HomeTunnelPlacement, HomeTunnelRegistration,
    HomeTunnelReplayGuard, HomeTunnelStreamEnvelope, HomeTunnelStreamOpen, NodeCapacityCertificate,
    NodeSigningIdentity,
};

const HOME_TUNNEL_ALPN: &[u8] = b"obelisk-home-tunnel/1";
const DEFAULT_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_CONTROL_BYTES: usize = 1024 * 1024;
const DEFAULT_CHALLENGE_TTL: Duration = Duration::from_secs(15);
const DEFAULT_STREAM_TTL: Duration = Duration::from_secs(10);
const DEFAULT_IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct HomeTunnelTlsMaterial {
    pub ca_certificate_der: Vec<u8>,
    pub certificate_chain_der: Vec<Vec<u8>>,
    pub private_key_pkcs8_der: Vec<u8>,
}

impl fmt::Debug for HomeTunnelTlsMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HomeTunnelTlsMaterial")
            .field("ca_certificate_bytes", &self.ca_certificate_der.len())
            .field("certificate_chain_len", &self.certificate_chain_der.len())
            .field("private_key", &"[REDACTED]")
            .finish()
    }
}

impl HomeTunnelTlsMaterial {
    pub fn from_der_files(
        ca_certificate: impl AsRef<std::path::Path>,
        certificate_chain: &[impl AsRef<std::path::Path>],
        private_key: impl AsRef<std::path::Path>,
    ) -> Result<Self, String> {
        let ca_certificate_der = std::fs::read(ca_certificate.as_ref()).map_err(|error| {
            format!(
                "read Home Tunnel CA certificate {}: {error}",
                ca_certificate.as_ref().display()
            )
        })?;
        let certificate_chain_der = certificate_chain
            .iter()
            .map(|path| {
                std::fs::read(path.as_ref()).map_err(|error| {
                    format!(
                        "read Home Tunnel certificate {}: {error}",
                        path.as_ref().display()
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let private_key_pkcs8_der = std::fs::read(private_key.as_ref()).map_err(|error| {
            format!(
                "read Home Tunnel private key {}: {error}",
                private_key.as_ref().display()
            )
        })?;
        let material = Self {
            ca_certificate_der,
            certificate_chain_der,
            private_key_pkcs8_der,
        };
        material.validate()?;
        Ok(material)
    }

    pub fn leaf_certificate_sha256(&self) -> Result<String, String> {
        let leaf = self
            .certificate_chain_der
            .first()
            .ok_or_else(|| "Home Tunnel TLS certificate chain is empty".to_string())?;
        Ok(hex_digest(&Sha256::digest(leaf)))
    }

    fn validate(&self) -> Result<(), String> {
        if self.ca_certificate_der.is_empty()
            || self.certificate_chain_der.is_empty()
            || self
                .certificate_chain_der
                .iter()
                .any(|certificate| certificate.is_empty())
            || self.private_key_pkcs8_der.is_empty()
        {
            return Err("Home Tunnel TLS material contains an empty DER value".to_string());
        }
        Ok(())
    }

    fn certificate_chain(&self) -> Vec<CertificateDer<'static>> {
        self.certificate_chain_der
            .iter()
            .cloned()
            .map(CertificateDer::from)
            .collect()
    }

    fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivatePkcs8KeyDer::from(self.private_key_pkcs8_der.clone()).into()
    }

    fn roots(&self) -> Result<RootCertStore, String> {
        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from(self.ca_certificate_der.clone()))
            .map_err(|error| format!("add Home Tunnel CA certificate: {error}"))?;
        Ok(roots)
    }
}

#[derive(Debug, Clone)]
pub struct HomeTunnelRelayConfig {
    pub relay_id: String,
    pub quic_bind: SocketAddr,
    pub gateway_bind: SocketAddr,
    pub tls: HomeTunnelTlsMaterial,
    pub relay_identity: NodeSigningIdentity,
    pub trusted_capacity_issuer: String,
    pub trusted_control_issuer: String,
    pub placements: Vec<HomeTunnelPlacement>,
    pub max_frame_bytes: usize,
    pub max_control_bytes: usize,
    pub challenge_ttl: Duration,
    pub stream_ttl: Duration,
    pub io_timeout: Duration,
}

impl HomeTunnelRelayConfig {
    pub fn with_defaults(
        relay_id: impl Into<String>,
        quic_bind: SocketAddr,
        gateway_bind: SocketAddr,
        tls: HomeTunnelTlsMaterial,
        relay_identity: NodeSigningIdentity,
        trusted_capacity_issuer: impl Into<String>,
        trusted_control_issuer: impl Into<String>,
        placements: Vec<HomeTunnelPlacement>,
    ) -> Self {
        Self {
            relay_id: relay_id.into(),
            quic_bind,
            gateway_bind,
            tls,
            relay_identity,
            trusted_capacity_issuer: trusted_capacity_issuer.into(),
            trusted_control_issuer: trusted_control_issuer.into(),
            placements,
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            max_control_bytes: DEFAULT_MAX_CONTROL_BYTES,
            challenge_ttl: DEFAULT_CHALLENGE_TTL,
            stream_ttl: DEFAULT_STREAM_TTL,
            io_timeout: DEFAULT_IO_TIMEOUT,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.relay_id.trim().is_empty()
            || self.trusted_capacity_issuer.trim().is_empty()
            || self.trusted_control_issuer.trim().is_empty()
            || self.max_frame_bytes == 0
            || self.max_control_bytes == 0
            || self.challenge_ttl.is_zero()
            || self.stream_ttl.is_zero()
            || self.io_timeout.is_zero()
        {
            return Err(
                "Home Tunnel Relay configuration contains an invalid zero/empty field".to_string(),
            );
        }
        self.tls.validate()?;
        if self.placements.is_empty() {
            return Err("Home Tunnel Relay requires at least one placement".to_string());
        }
        let mut zones = BTreeMap::new();
        for placement in &self.placements {
            if placement.relay_id != self.relay_id {
                return Err(format!(
                    "Home Tunnel placement {} targets relay {}, expected {}",
                    placement.placement_id, placement.relay_id, self.relay_id
                ));
            }
            if zones
                .insert(placement.zone_id.clone(), placement.placement_id.clone())
                .is_some()
            {
                return Err(format!(
                    "Home Tunnel Relay has duplicate placement for Zone {}",
                    placement.zone_id
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct HomeTunnelAgentConfig {
    pub relay_id: String,
    pub relay_addr: SocketAddr,
    pub relay_server_name: String,
    pub local_zone_rpc_addr: SocketAddr,
    pub tls: HomeTunnelTlsMaterial,
    pub node_identity: NodeSigningIdentity,
    pub key_generation: u64,
    pub agent_instance_id: String,
    pub registration_sequence: u64,
    pub capacity_certificate: NodeCapacityCertificate,
    pub trusted_relay_issuer: String,
    pub trusted_control_issuer: String,
    pub max_frame_bytes: usize,
    pub max_control_bytes: usize,
    pub io_timeout: Duration,
}

impl HomeTunnelAgentConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn with_defaults(
        relay_id: impl Into<String>,
        relay_addr: SocketAddr,
        relay_server_name: impl Into<String>,
        local_zone_rpc_addr: SocketAddr,
        tls: HomeTunnelTlsMaterial,
        node_identity: NodeSigningIdentity,
        key_generation: u64,
        agent_instance_id: impl Into<String>,
        registration_sequence: u64,
        capacity_certificate: NodeCapacityCertificate,
        trusted_relay_issuer: impl Into<String>,
        trusted_control_issuer: impl Into<String>,
    ) -> Self {
        Self {
            relay_id: relay_id.into(),
            relay_addr,
            relay_server_name: relay_server_name.into(),
            local_zone_rpc_addr,
            tls,
            node_identity,
            key_generation,
            agent_instance_id: agent_instance_id.into(),
            registration_sequence,
            capacity_certificate,
            trusted_relay_issuer: trusted_relay_issuer.into(),
            trusted_control_issuer: trusted_control_issuer.into(),
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            max_control_bytes: DEFAULT_MAX_CONTROL_BYTES,
            io_timeout: DEFAULT_IO_TIMEOUT,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.relay_id.trim().is_empty()
            || self.relay_server_name.trim().is_empty()
            || self.agent_instance_id.trim().is_empty()
            || self.key_generation == 0
            || self.registration_sequence == 0
            || self.trusted_relay_issuer.trim().is_empty()
            || self.trusted_control_issuer.trim().is_empty()
            || self.max_frame_bytes == 0
            || self.max_control_bytes == 0
            || self.io_timeout.is_zero()
        {
            return Err(
                "Home Tunnel Agent configuration contains an invalid zero/empty field".to_string(),
            );
        }
        if self.capacity_certificate.node_id != self.node_identity.node_id()
            || self.capacity_certificate.public_key != self.node_identity.public_key()
            || self.capacity_certificate.key_generation != self.key_generation
        {
            return Err(
                "Home Tunnel Agent identity does not match capacity certificate".to_string(),
            );
        }
        self.tls.validate()
    }
}

#[derive(Clone)]
struct RegisteredHomeNode {
    connection: Connection,
    registration: HomeTunnelRegistration,
}

struct HomeTunnelRelayShared {
    config: Arc<HomeTunnelRelayConfig>,
    placements: BTreeMap<String, HomeTunnelPlacement>,
    nodes: RwLock<BTreeMap<String, RegisteredHomeNode>>,
    replay_guard: HomeTunnelReplayGuard,
    stream_sequence: AtomicU64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HomeTunnelRegistrationAck {
    relay_id: String,
    node_id: String,
    accepted_at_ms: u64,
}

pub struct HomeTunnelRelay {
    endpoint: Endpoint,
    gateway_listener: TcpListener,
    shared: Arc<HomeTunnelRelayShared>,
}

impl fmt::Debug for HomeTunnelRelay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HomeTunnelRelay")
            .field("quic_addr", &self.quic_addr())
            .field("gateway_addr", &self.gateway_addr())
            .finish_non_exhaustive()
    }
}

impl HomeTunnelRelay {
    pub async fn bind(config: HomeTunnelRelayConfig) -> Result<Self, String> {
        config.validate()?;
        let server_config = quic_server_config(&config.tls)?;
        let endpoint = Endpoint::server(server_config, config.quic_bind)
            .map_err(|error| format!("bind Home Tunnel QUIC Relay: {error}"))?;
        let gateway_listener = TcpListener::bind(config.gateway_bind)
            .await
            .map_err(|error| format!("bind Home Tunnel gateway listener: {error}"))?;
        let placements = config
            .placements
            .iter()
            .cloned()
            .map(|placement| (placement.zone_id.clone(), placement))
            .collect();
        Ok(Self {
            endpoint,
            gateway_listener,
            shared: Arc::new(HomeTunnelRelayShared {
                config: Arc::new(config),
                placements,
                nodes: RwLock::new(BTreeMap::new()),
                replay_guard: HomeTunnelReplayGuard::default(),
                stream_sequence: AtomicU64::new(1),
            }),
        })
    }

    pub fn quic_addr(&self) -> Result<SocketAddr, String> {
        self.endpoint
            .local_addr()
            .map_err(|error| format!("read Home Tunnel QUIC address: {error}"))
    }

    pub fn gateway_addr(&self) -> Result<SocketAddr, String> {
        self.gateway_listener
            .local_addr()
            .map_err(|error| format!("read Home Tunnel gateway address: {error}"))
    }

    pub async fn registered_nodes(&self) -> usize {
        self.shared.nodes.read().await.len()
    }

    pub async fn serve(self, mut shutdown: watch::Receiver<bool>) -> Result<(), String> {
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        self.endpoint.close(0_u32.into(), b"relay shutdown");
                        return Ok(());
                    }
                }
                incoming = self.endpoint.accept() => {
                    let Some(incoming) = incoming else {
                        return Ok(());
                    };
                    let shared = Arc::clone(&self.shared);
                    tokio::spawn(async move {
                        if let Err(error) = handle_agent_connection(incoming, shared).await {
                            eprintln!("Home Tunnel agent connection rejected: {error}");
                        }
                    });
                }
                accepted = self.gateway_listener.accept() => {
                    let (stream, _) = accepted
                        .map_err(|error| format!("accept Home Tunnel gateway connection: {error}"))?;
                    let shared = Arc::clone(&self.shared);
                    tokio::spawn(async move {
                        if let Err(error) = handle_gateway_connection(stream, shared).await {
                            eprintln!("Home Tunnel gateway connection closed: {error}");
                        }
                    });
                }
            }
        }
    }
}

pub struct HomeTunnelAgent {
    endpoint: Endpoint,
    connection: Connection,
    shared: Arc<HomeTunnelAgentShared>,
}

#[derive(Clone)]
pub struct HomeTunnelAgentNetworkHandle {
    endpoint: Endpoint,
}

impl fmt::Debug for HomeTunnelAgentNetworkHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HomeTunnelAgentNetworkHandle")
            .finish_non_exhaustive()
    }
}

impl HomeTunnelAgentNetworkHandle {
    pub fn rebind(&self, socket: UdpSocket) -> Result<(), String> {
        self.endpoint
            .rebind(socket)
            .map_err(|error| format!("rebind Home Tunnel Agent UDP socket: {error}"))
    }
}

impl fmt::Debug for HomeTunnelAgent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HomeTunnelAgent")
            .field("node_id", &self.shared.config.node_identity.node_id())
            .field("relay_addr", &self.shared.config.relay_addr)
            .finish_non_exhaustive()
    }
}

struct HomeTunnelAgentShared {
    config: Arc<HomeTunnelAgentConfig>,
    replay_guard: HomeTunnelReplayGuard,
}

impl HomeTunnelAgent {
    pub async fn connect(config: HomeTunnelAgentConfig) -> Result<Self, String> {
        config.validate()?;
        let mut endpoint = Endpoint::client(
            "0.0.0.0:0"
                .parse()
                .expect("static Home Tunnel client bind must parse"),
        )
        .map_err(|error| format!("bind Home Tunnel Agent UDP socket: {error}"))?;
        endpoint.set_default_client_config(quic_client_config(&config.tls)?);
        let connection = endpoint
            .connect(config.relay_addr, &config.relay_server_name)
            .map_err(|error| format!("start Home Tunnel QUIC connection: {error}"))?
            .await
            .map_err(|error| format!("establish Home Tunnel QUIC connection: {error}"))?;
        let (mut send, mut receive) =
            tokio::time::timeout(config.io_timeout, connection.accept_bi())
                .await
                .map_err(|_| "Home Tunnel registration challenge timed out".to_string())?
                .map_err(|error| format!("accept Home Tunnel registration stream: {error}"))?;
        let challenge: HomeTunnelChallenge =
            read_json_message(&mut receive, config.max_control_bytes).await?;
        challenge.verify(&config.trusted_relay_issuer, now_ms())?;
        if challenge.relay_id != config.relay_id {
            return Err("Home Tunnel challenge targets a different relay".to_string());
        }
        let registration = HomeTunnelRegistration::sign(
            challenge,
            &config.node_identity,
            config.key_generation,
            config.agent_instance_id.clone(),
            std::process::id(),
            config.registration_sequence,
            now_ms(),
            config.tls.leaf_certificate_sha256()?,
            config.capacity_certificate.clone(),
        )?;
        write_json_message(&mut send, &registration, config.max_control_bytes).await?;
        send.finish()
            .map_err(|error| format!("finish Home Tunnel registration stream: {error}"))?;
        let ack: HomeTunnelRegistrationAck =
            read_json_message(&mut receive, config.max_control_bytes).await?;
        if ack.relay_id != config.relay_id
            || ack.node_id != config.node_identity.node_id()
            || ack.accepted_at_ms == 0
        {
            return Err(
                "Home Tunnel Relay returned an invalid registration acknowledgement".to_string(),
            );
        }
        Ok(Self {
            endpoint,
            connection,
            shared: Arc::new(HomeTunnelAgentShared {
                config: Arc::new(config),
                replay_guard: HomeTunnelReplayGuard::default(),
            }),
        })
    }

    pub fn rebind(&self, socket: UdpSocket) -> Result<(), String> {
        self.endpoint
            .rebind(socket)
            .map_err(|error| format!("rebind Home Tunnel Agent UDP socket: {error}"))
    }

    pub fn network_handle(&self) -> HomeTunnelAgentNetworkHandle {
        HomeTunnelAgentNetworkHandle {
            endpoint: self.endpoint.clone(),
        }
    }

    pub async fn serve(self, mut shutdown: watch::Receiver<bool>) -> Result<(), String> {
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        self.connection.close(0_u32.into(), b"agent shutdown");
                        self.endpoint.close(0_u32.into(), b"agent shutdown");
                        return Ok(());
                    }
                }
                stream = self.connection.accept_bi() => {
                    let (send, receive) = stream
                        .map_err(|error| format!("accept Home Tunnel data stream: {error}"))?;
                    let shared = Arc::clone(&self.shared);
                    tokio::spawn(async move {
                        if let Err(error) = handle_agent_stream(send, receive, shared).await {
                            eprintln!("Home Tunnel data stream rejected: {error}");
                        }
                    });
                }
            }
        }
    }
}

async fn handle_agent_connection(
    incoming: quinn::Incoming,
    shared: Arc<HomeTunnelRelayShared>,
) -> Result<(), String> {
    let connection = incoming
        .await
        .map_err(|error| format!("accept Home Tunnel QUIC handshake: {error}"))?;
    let peer_fingerprint = peer_certificate_sha256(&connection)?;
    let (mut send, mut receive) = connection
        .open_bi()
        .await
        .map_err(|error| format!("open Home Tunnel registration stream: {error}"))?;
    let issued_at_ms = now_ms();
    let expires_at_ms = issued_at_ms
        .saturating_add(u64::try_from(shared.config.challenge_ttl.as_millis()).unwrap_or(u64::MAX));
    let nonce = random_nonce();
    let challenge = HomeTunnelChallenge::issue(
        shared.config.relay_id.clone(),
        format!("challenge-{nonce}"),
        nonce,
        issued_at_ms,
        expires_at_ms,
        &shared.config.relay_identity,
    )?;
    write_json_message(&mut send, &challenge, shared.config.max_control_bytes).await?;
    let registration: HomeTunnelRegistration =
        read_json_message(&mut receive, shared.config.max_control_bytes).await?;
    if registration.tls_certificate_sha256 != peer_fingerprint {
        return Err(
            "Home Tunnel node signature is not bound to the presented mTLS certificate".to_string(),
        );
    }
    shared.replay_guard.accept_registration(
        &registration,
        shared.config.relay_identity.public_key(),
        &shared.config.trusted_capacity_issuer,
        &shared.config.relay_id,
        now_ms(),
    )?;
    let assigned = shared
        .placements
        .values()
        .filter(|placement| placement.node_id == registration.node_id)
        .collect::<Vec<_>>();
    if assigned.is_empty() {
        return Err("Home Tunnel node has no finalized placement on this Relay".to_string());
    }
    for placement in assigned {
        placement.verify(
            &shared.config.trusted_control_issuer,
            &shared.config.relay_id,
            &registration.capacity_certificate,
            now_ms(),
        )?;
    }
    let node_id = registration.node_id.clone();
    let stable_id = connection.stable_id();
    let previous = shared.nodes.write().await.insert(
        node_id.clone(),
        RegisteredHomeNode {
            connection: connection.clone(),
            registration,
        },
    );
    if let Some(previous) = previous {
        previous
            .connection
            .close(0_u32.into(), b"newer Home Tunnel connection registered");
    }
    write_json_message(
        &mut send,
        &HomeTunnelRegistrationAck {
            relay_id: shared.config.relay_id.clone(),
            node_id: node_id.clone(),
            accepted_at_ms: now_ms(),
        },
        shared.config.max_control_bytes,
    )
    .await?;
    send.finish()
        .map_err(|error| format!("finish Home Tunnel registration acknowledgement: {error}"))?;
    let _ = connection.closed().await;
    let mut nodes = shared.nodes.write().await;
    if nodes
        .get(&node_id)
        .is_some_and(|registered| registered.connection.stable_id() == stable_id)
    {
        nodes.remove(&node_id);
    }
    Ok(())
}

async fn handle_gateway_connection(
    mut gateway: TcpStream,
    shared: Arc<HomeTunnelRelayShared>,
) -> Result<(), String> {
    loop {
        let request = match read_frame(&mut gateway, shared.config.max_frame_bytes).await {
            Ok(frame) => frame,
            Err(error)
                if error.contains("unexpected end of file") || error.contains("early eof") =>
            {
                return Ok(())
            }
            Err(error) => return Err(error),
        };
        let hint = decode_zone_rpc_routing_hint(&request)?;
        let placement = shared
            .placements
            .get(&hint.zone_id)
            .cloned()
            .ok_or_else(|| format!("no Home Tunnel placement for Zone {}", hint.zone_id))?;
        let registered = shared
            .nodes
            .read()
            .await
            .get(&placement.node_id)
            .cloned()
            .ok_or_else(|| format!("Home Tunnel node {} is offline", placement.node_id))?;
        let now = now_ms();
        placement.verify(
            &shared.config.trusted_control_issuer,
            &shared.config.relay_id,
            &registered.registration.capacity_certificate,
            now,
        )?;
        let ttl_ms = u64::try_from(shared.config.stream_ttl.as_millis()).unwrap_or(u64::MAX);
        let expires_at_ms = now.saturating_add(ttl_ms).min(placement.expires_at_ms);
        if expires_at_ms <= now {
            return Err("Home Tunnel placement expired before stream open".to_string());
        }
        let open = HomeTunnelStreamOpen::sign(
            &placement,
            hint.session_id,
            shared.stream_sequence.fetch_add(1, Ordering::Relaxed),
            random_nonce(),
            now,
            expires_at_ms,
            &shared.config.relay_identity,
        )?;
        let envelope = HomeTunnelStreamEnvelope { placement, open };
        let (mut send, mut receive) =
            tokio::time::timeout(shared.config.io_timeout, registered.connection.open_bi())
                .await
                .map_err(|_| "open Home Tunnel QUIC stream timed out".to_string())?
                .map_err(|error| format!("open Home Tunnel QUIC stream: {error}"))?;
        write_json_message(&mut send, &envelope, shared.config.max_control_bytes).await?;
        write_frame(&mut send, &request, shared.config.max_frame_bytes).await?;
        send.finish()
            .map_err(|error| format!("finish Home Tunnel request stream: {error}"))?;
        let response = tokio::time::timeout(
            shared.config.io_timeout,
            read_frame(&mut receive, shared.config.max_frame_bytes),
        )
        .await
        .map_err(|_| "Home Tunnel Zone RPC response timed out".to_string())??;
        write_frame(&mut gateway, &response, shared.config.max_frame_bytes).await?;
    }
}

async fn handle_agent_stream(
    mut send: SendStream,
    mut receive: RecvStream,
    shared: Arc<HomeTunnelAgentShared>,
) -> Result<(), String> {
    let envelope: HomeTunnelStreamEnvelope =
        read_json_message(&mut receive, shared.config.max_control_bytes).await?;
    let now = now_ms();
    envelope.verify(
        &shared.config.trusted_control_issuer,
        &shared.config.trusted_relay_issuer,
        &shared.config.relay_id,
        &shared.config.capacity_certificate,
        now,
    )?;
    shared.replay_guard.accept_stream(
        &envelope.open,
        &envelope.placement,
        &shared.config.trusted_relay_issuer,
        now,
    )?;
    let placement_id = envelope.placement.placement_id.clone();
    let session_id = envelope.open.session_id.clone();
    let result = async {
        let request = read_frame(&mut receive, shared.config.max_frame_bytes).await?;
        let hint = decode_zone_rpc_routing_hint(&request)?;
        if hint.zone_id != envelope.open.zone_id || hint.session_id != envelope.open.session_id {
            return Err(
                "Home Tunnel signed stream does not match the enclosed Zone RPC frame".to_string(),
            );
        }
        let mut local = tokio::time::timeout(
            shared.config.io_timeout,
            TcpStream::connect(shared.config.local_zone_rpc_addr),
        )
        .await
        .map_err(|_| "connect local Zone Host timed out".to_string())?
        .map_err(|error| format!("connect local Zone Host: {error}"))?;
        write_frame(&mut local, &request, shared.config.max_frame_bytes).await?;
        let response = tokio::time::timeout(
            shared.config.io_timeout,
            read_frame(&mut local, shared.config.max_frame_bytes),
        )
        .await
        .map_err(|_| "local Zone Host response timed out".to_string())??;
        write_frame(&mut send, &response, shared.config.max_frame_bytes).await?;
        send.finish()
            .map_err(|error| format!("finish Home Tunnel response stream: {error}"))
    }
    .await;
    shared.replay_guard.close_stream(&placement_id, &session_id);
    result
}

fn quic_server_config(tls: &HomeTunnelTlsMaterial) -> Result<quinn::ServerConfig, String> {
    let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(tls.roots()?))
        .build()
        .map_err(|error| format!("build Home Tunnel mTLS client verifier: {error}"))?;
    let mut crypto = rustls::ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(tls.certificate_chain(), tls.private_key())
        .map_err(|error| format!("build Home Tunnel TLS server config: {error}"))?;
    crypto.alpn_protocols = vec![HOME_TUNNEL_ALPN.to_vec()];
    let mut config = quinn::ServerConfig::with_crypto(Arc::new(
        QuicServerConfig::try_from(crypto)
            .map_err(|error| format!("build Home Tunnel QUIC server crypto: {error}"))?,
    ));
    Arc::get_mut(&mut config.transport)
        .expect("new QUIC server transport must be uniquely owned")
        .keep_alive_interval(Some(Duration::from_secs(5)));
    Ok(config)
}

fn quic_client_config(tls: &HomeTunnelTlsMaterial) -> Result<quinn::ClientConfig, String> {
    let mut crypto = RustlsClientConfig::builder()
        .with_root_certificates(tls.roots()?)
        .with_client_auth_cert(tls.certificate_chain(), tls.private_key())
        .map_err(|error| format!("build Home Tunnel TLS client config: {error}"))?;
    crypto.alpn_protocols = vec![HOME_TUNNEL_ALPN.to_vec()];
    Ok(quinn::ClientConfig::new(Arc::new(
        QuicClientConfig::try_from(crypto)
            .map_err(|error| format!("build Home Tunnel QUIC client crypto: {error}"))?,
    )))
}

fn peer_certificate_sha256(connection: &Connection) -> Result<String, String> {
    let identity = connection
        .peer_identity()
        .ok_or_else(|| "Home Tunnel mTLS peer did not present an identity".to_string())?;
    let certificates = identity
        .downcast::<Vec<CertificateDer<'static>>>()
        .map_err(|_| "Home Tunnel mTLS peer identity has an unexpected type".to_string())?;
    let leaf = certificates
        .first()
        .ok_or_else(|| "Home Tunnel mTLS peer certificate chain is empty".to_string())?;
    Ok(hex_digest(&Sha256::digest(leaf.as_ref())))
}

async fn read_json_message<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
    max_bytes: usize,
) -> Result<T, String> {
    let bytes = read_frame(reader, max_bytes).await?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode Home Tunnel control message: {error}"))
}

async fn write_json_message<T: Serialize>(
    writer: &mut (impl AsyncWrite + Unpin),
    value: &T,
    max_bytes: usize,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("encode Home Tunnel control message: {error}"))?;
    write_frame(writer, &bytes, max_bytes).await
}

async fn read_frame(
    reader: &mut (impl AsyncRead + Unpin),
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let mut header = [0_u8; 4];
    reader
        .read_exact(&mut header)
        .await
        .map_err(|error| format!("read Home Tunnel frame header: {error}"))?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > max_bytes {
        return Err(format!("invalid Home Tunnel frame length {length}"));
    }
    let mut bytes = vec![0_u8; length];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|error| format!("read Home Tunnel frame body: {error}"))?;
    Ok(bytes)
}

async fn write_frame(
    writer: &mut (impl AsyncWrite + Unpin),
    bytes: &[u8],
    max_bytes: usize,
) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > max_bytes || bytes.len() > u32::MAX as usize {
        return Err(format!(
            "invalid Home Tunnel outbound frame size {}",
            bytes.len()
        ));
    }
    writer
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .await
        .map_err(|error| format!("write Home Tunnel frame header: {error}"))?;
    writer
        .write_all(bytes)
        .await
        .map_err(|error| format!("write Home Tunnel frame body: {error}"))?;
    writer
        .flush()
        .await
        .map_err(|error| format!("flush Home Tunnel frame: {error}"))
}

fn random_nonce() -> String {
    let mut nonce = [0_u8; 32];
    OsRng.fill_bytes(&mut nonce);
    URL_SAFE_NO_PAD.encode(nonce)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
