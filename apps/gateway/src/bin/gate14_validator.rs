#![cfg(feature = "commonware-2026-2")]

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use commonware_codec::{Decode, Encode};
use commonware_consensus::{
    simplex::{self, elector::RoundRobin, types::Activity},
    types::{Epoch, ViewDelta},
    Automaton, CertifiableAutomaton, Relay, Reporter as ConsensusReporter, Viewable,
};
use commonware_cryptography::{ed25519, Hasher, Sha256, Signer as _};
use commonware_p2p::{authenticated::discovery, Ingress, Manager};
use commonware_parallel::Sequential;
use commonware_runtime::{
    buffer::paged::CacheRef, spawn_cell, tokio as commonware_tokio, Clock, ContextCell, Handle,
    Metrics, Quota, Runner, Spawner,
};
use commonware_utils::{
    channel::{mpsc, oneshot},
    ordered::Set,
    union, NZUsize, TryCollect, NZU16, NZU32,
};
use mir2_gateway::{
    replay_gate14_records, Gate14AuthoritativeState, Gate14CommandEnvelope, Gate14FinalizedRecord,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

type Scheme = commonware_consensus::simplex::scheme::ed25519::Scheme;
type ConsensusDigest = <Sha256 as Hasher>::Digest;

const APPLICATION_NAMESPACE: &[u8] = b"_OBELISK_GATE14_CONTROL_V1";
const GENESIS: &[u8] = b"obelisk gate14 commonware v2026.2.0 genesis";
const COMMAND_ACTIVATION_DELAY_MS: u64 = 750;

#[derive(Debug)]
struct ValidatorStore {
    node_id: String,
    data_dir: PathBuf,
    commands: BTreeMap<u64, Gate14CommandEnvelope>,
    finalized: Vec<Gate14FinalizedRecord>,
    state: Gate14AuthoritativeState,
    started_at_ms: u64,
}

impl ValidatorStore {
    fn open(node_id: String, data_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&data_dir)
            .map_err(|error| format!("create {} failed: {error}", data_dir.display()))?;
        let finalized_path = data_dir.join("finalized.jsonl");
        let mut finalized = Vec::new();
        if finalized_path.exists() {
            let file = fs::File::open(&finalized_path)
                .map_err(|error| format!("open {} failed: {error}", finalized_path.display()))?;
            for (index, line) in BufReader::new(file).lines().enumerate() {
                let line = line.map_err(|error| {
                    format!(
                        "read {} line {} failed: {error}",
                        finalized_path.display(),
                        index + 1
                    )
                })?;
                if line.trim().is_empty() {
                    continue;
                }
                finalized.push(
                    serde_json::from_str::<Gate14FinalizedRecord>(&line).map_err(|error| {
                        format!(
                            "decode {} line {} failed: {error}",
                            finalized_path.display(),
                            index + 1
                        )
                    })?,
                );
            }
        }
        let state = replay_gate14_records(&finalized)?;
        let commands = finalized
            .iter()
            .map(|record| (record.command.sequence, record.command.clone()))
            .collect();
        Ok(Self {
            node_id,
            data_dir,
            commands,
            finalized,
            state,
            started_at_ms: now_ms(),
        })
    }

    fn submit(&mut self, command: Gate14CommandEnvelope) -> Result<String, String> {
        command.validate()?;
        let digest = command.digest()?;
        if command.sequence <= self.state.last_sequence {
            let existing = self
                .finalized
                .iter()
                .find(|record| record.command.sequence == command.sequence)
                .ok_or_else(|| "finalized command sequence is missing locally".to_string())?;
            if existing.command_digest == digest {
                return Ok(digest);
            }
            return Err(format!(
                "sequence {} is already finalized with another digest",
                command.sequence
            ));
        }
        if let Some(existing) = self.commands.get(&command.sequence) {
            if existing.digest()? == digest {
                return Ok(digest);
            }
            return Err(format!(
                "sequence {} is already prepared with another digest",
                command.sequence
            ));
        }
        self.commands.insert(command.sequence, command);
        self.persist_prepared()?;
        Ok(digest)
    }

    fn next_proposal(&self, now_ms: u64) -> Option<(ConsensusDigest, String)> {
        let sequence = self.state.last_sequence.saturating_add(1);
        let command = self.commands.get(&sequence)?;
        if now_ms
            < command
                .submitted_at_ms
                .saturating_add(COMMAND_ACTIVATION_DELAY_MS)
        {
            return None;
        }
        let (digest, key) = consensus_digest(command).ok()?;
        Some((digest, key))
    }

    fn knows_digest(&self, digest_key: &str) -> bool {
        self.commands
            .values()
            .any(|command| consensus_digest(command).is_ok_and(|(_, key)| key == digest_key))
    }

    fn finalize(
        &mut self,
        epoch: u64,
        view: u64,
        digest_key: &str,
        signer_count: usize,
        certificate_base64: String,
    ) -> Result<Option<Gate14FinalizedRecord>, String> {
        if self
            .finalized
            .iter()
            .any(|record| record.commonware_digest == digest_key)
        {
            return Ok(None);
        }
        let sequence = self.state.last_sequence.saturating_add(1);
        let command = self
            .commands
            .get(&sequence)
            .filter(|command| {
                consensus_digest(command).is_ok_and(|(_, key)| key == digest_key)
            })
            .cloned()
            .ok_or_else(|| {
                format!(
                    "finalized Commonware digest {digest_key} has no prepared command at sequence {sequence}"
                )
            })?;
        let height = self.state.finalized_height.saturating_add(1);
        let outcome = self.state.apply_finalized(height, &command)?;
        let record = Gate14FinalizedRecord {
            height,
            epoch,
            view,
            command_digest: command.digest()?,
            commonware_digest: digest_key.to_string(),
            signer_count,
            certificate_base64,
            command,
            state_root: outcome.state_root,
            finalized_at_ms: now_ms(),
        };
        append_json_line(&self.data_dir.join("finalized.jsonl"), &record)?;
        self.finalized.push(record.clone());
        self.persist_snapshot()?;
        Ok(Some(record))
    }

    fn persist_prepared(&self) -> Result<(), String> {
        atomic_json(
            &self.data_dir.join("prepared.json"),
            &self.commands.values().collect::<Vec<_>>(),
        )
    }

    fn persist_snapshot(&self) -> Result<(), String> {
        atomic_json(&self.data_dir.join("state.json"), &self.state)
    }

    fn import_verified(
        &mut self,
        records: Vec<Gate14FinalizedRecord>,
        verifier: &Scheme,
        committee_size: usize,
    ) -> Result<usize, String> {
        let quorum = committee_size - (committee_size.saturating_sub(1) / 3);
        let mut imported = 0;
        for record in records {
            if record.height <= self.state.finalized_height {
                let existing = self
                    .finalized
                    .iter()
                    .find(|existing| existing.height == record.height)
                    .ok_or_else(|| {
                        format!("local finalized height {} is missing", record.height)
                    })?;
                if existing.state_root != record.state_root {
                    return Err(format!(
                        "conflicting finalized state root at height {}",
                        record.height
                    ));
                }
                continue;
            }
            if record.height != self.state.finalized_height.saturating_add(1) {
                return Err(format!(
                    "catch-up height gap: local {}, imported {}",
                    self.state.finalized_height, record.height
                ));
            }
            if record.signer_count < quorum {
                return Err(format!(
                    "catch-up record {} has {} signers below quorum {quorum}",
                    record.height, record.signer_count
                ));
            }
            if record.command.digest()? != record.command_digest {
                return Err(format!(
                    "catch-up record {} command digest mismatch",
                    record.height
                ));
            }
            let certificate = BASE64
                .decode(&record.certificate_base64)
                .map_err(|error| format!("decode finalization certificate failed: {error}"))?;
            let finalization = simplex::types::Finalization::<Scheme, ConsensusDigest>::decode_cfg(
                certificate.as_slice(),
                &committee_size,
            )
            .map_err(|error| format!("decode Commonware finalization failed: {error}"))?;
            if digest_key(&finalization.proposal.payload) != record.commonware_digest
                || finalization.view().get() != record.view
                || finalization.proposal.round.epoch().get() != record.epoch
            {
                return Err(format!(
                    "catch-up record {} does not match Commonware certificate coordinates",
                    record.height
                ));
            }
            if !finalization.verify(&mut rand::thread_rng(), verifier, &Sequential) {
                return Err(format!(
                    "catch-up record {} has an invalid Commonware certificate",
                    record.height
                ));
            }
            let outcome = self.state.apply_finalized(record.height, &record.command)?;
            if outcome.state_root != record.state_root {
                return Err(format!(
                    "catch-up record {} state root mismatch",
                    record.height
                ));
            }
            append_json_line(&self.data_dir.join("finalized.jsonl"), &record)?;
            self.commands
                .insert(record.command.sequence, record.command.clone());
            self.finalized.push(record);
            imported += 1;
        }
        if imported > 0 {
            self.persist_prepared()?;
            self.persist_snapshot()?;
        }
        Ok(imported)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidatorStatus {
    role: &'static str,
    node_id: String,
    commonware_release: &'static str,
    committee_size: usize,
    quorum: usize,
    finalized_height: u64,
    last_sequence: u64,
    state_root: String,
    prepared_count: usize,
    pending_count: usize,
    started_at_ms: u64,
}

#[derive(Debug, Deserialize)]
struct FinalityQuery {
    #[serde(default)]
    after: u64,
}

#[derive(Clone)]
struct HttpState {
    store: Arc<Mutex<ValidatorStore>>,
    committee_size: usize,
    verifier: Scheme,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitResponse {
    accepted: bool,
    command_digest: String,
    activation_delay_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportResponse {
    accepted: bool,
    imported: usize,
    finalized_height: u64,
    state_root: String,
}

#[derive(Debug)]
struct ApiError(String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"accepted": false, "error": self.0})),
        )
            .into_response()
    }
}

enum ApplicationMessage {
    Genesis {
        epoch: Epoch,
        response: oneshot::Sender<ConsensusDigest>,
    },
    Propose {
        response: oneshot::Sender<ConsensusDigest>,
    },
    Verify {
        digest: ConsensusDigest,
        response: oneshot::Sender<bool>,
    },
}

#[derive(Clone)]
struct ApplicationMailbox {
    sender: mpsc::Sender<ApplicationMessage>,
}

impl Automaton for ApplicationMailbox {
    type Digest = ConsensusDigest;
    type Context = simplex::types::Context<Self::Digest, ed25519::PublicKey>;

    async fn genesis(&mut self, epoch: Epoch) -> Self::Digest {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(ApplicationMessage::Genesis { epoch, response })
            .await
            .expect("Gate 14 application actor stopped");
        receiver.await.expect("Gate 14 genesis response dropped")
    }

    async fn propose(&mut self, _context: Self::Context) -> oneshot::Receiver<Self::Digest> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(ApplicationMessage::Propose { response })
            .await
            .expect("Gate 14 application actor stopped");
        receiver
    }

    async fn verify(
        &mut self,
        _context: Self::Context,
        digest: Self::Digest,
    ) -> oneshot::Receiver<bool> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(ApplicationMessage::Verify { digest, response })
            .await
            .expect("Gate 14 application actor stopped");
        receiver
    }
}

impl CertifiableAutomaton for ApplicationMailbox {}

impl Relay for ApplicationMailbox {
    type Digest = ConsensusDigest;

    async fn broadcast(&mut self, _digest: Self::Digest) {
        // Gate 14 ingress fans the canonical command to every validator before
        // the activation delay. Simplex carries authenticated votes and
        // certificates; production data availability can replace this ingress
        // fanout without changing the state machine.
    }
}

struct ApplicationActor<R: Clock + Rng + Spawner> {
    context: ContextCell<R>,
    store: Arc<Mutex<ValidatorStore>>,
    receiver: mpsc::Receiver<ApplicationMessage>,
}

impl<R: Clock + Rng + Spawner> ApplicationActor<R> {
    fn new(context: R, store: Arc<Mutex<ValidatorStore>>) -> (Self, ApplicationMailbox) {
        let (sender, receiver) = mpsc::channel(1024);
        (
            Self {
                context: ContextCell::new(context),
                store,
                receiver,
            },
            ApplicationMailbox { sender },
        )
    }

    fn start(mut self) -> Handle<()> {
        spawn_cell!(self.context, self.run().await)
    }

    async fn run(mut self) {
        while let Some(message) = self.receiver.recv().await {
            match message {
                ApplicationMessage::Genesis { epoch, response } => {
                    if epoch != Epoch::zero() {
                        error!(%epoch, "Gate 14 only supports epoch zero");
                    }
                    let mut hasher = Sha256::default();
                    hasher.update(GENESIS);
                    let _ = response.send(hasher.finalize());
                }
                ApplicationMessage::Propose { response } => loop {
                    let proposal = self
                        .store
                        .lock()
                        .ok()
                        .and_then(|store| store.next_proposal(now_ms()));
                    if let Some((digest, key)) = proposal {
                        info!(digest = %key, "proposing prepared Gate 14 command");
                        let _ = response.send(digest);
                        break;
                    }
                    self.context.sleep(Duration::from_millis(50)).await;
                },
                ApplicationMessage::Verify { digest, response } => {
                    let key = digest_key(&digest);
                    let valid = self
                        .store
                        .lock()
                        .is_ok_and(|store| store.knows_digest(&key));
                    let _ = response.send(valid);
                }
            }
        }
    }
}

#[derive(Clone)]
struct FinalityReporter {
    store: Arc<Mutex<ValidatorStore>>,
}

impl ConsensusReporter for FinalityReporter {
    type Activity = Activity<Scheme, ConsensusDigest>;

    async fn report(&mut self, activity: Self::Activity) {
        let Activity::Finalization(finalization) = activity else {
            return;
        };
        let key = digest_key(&finalization.proposal.payload);
        let signer_count = finalization.certificate.signers.count();
        let certificate_base64 = BASE64.encode(finalization.encode());
        let epoch = finalization.proposal.round.epoch().get();
        let view = finalization.view().get();
        match self.store.lock() {
            Ok(mut store) => {
                match store.finalize(epoch, view, &key, signer_count, certificate_base64) {
                    Ok(Some(record)) => info!(
                        height = record.height,
                        sequence = record.command.sequence,
                        state_root = %record.state_root,
                        signers = record.signer_count,
                        "finalized Gate 14 control command"
                    ),
                    Ok(None) => {}
                    Err(error) => error!(%error, %key, "failed to project finalized command"),
                }
            }
            Err(_) => error!("Gate 14 validator store mutex poisoned"),
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("gate14 validator failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .json()
        .try_init()
        .ok();

    let validator_seed = required_env("GATE14_VALIDATOR_SEED")?
        .parse::<u64>()
        .map_err(|error| format!("invalid GATE14_VALIDATOR_SEED: {error}"))?;
    let signer = ed25519::PrivateKey::from_seed(validator_seed);
    let node_id =
        env::var("GATE14_VALIDATOR_ID").unwrap_or_else(|_| format!("validator-{validator_seed}"));
    let participants = env::var("GATE14_PARTICIPANTS")
        .unwrap_or_else(|_| "0,1,2,3".to_string())
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|error| format!("invalid participant seed {value}: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if participants.len() != 4 {
        return Err("Gate 14 requires exactly four Commonware validators".to_string());
    }
    let validators: Set<_> = participants
        .iter()
        .map(|seed| ed25519::PrivateKey::from_seed(*seed).public_key())
        .try_collect()
        .map_err(|_| "Gate 14 validator public keys must be unique".to_string())?;
    let p2p_bind = socket_env("GATE14_P2P_BIND", "0.0.0.0:9300")?;
    let p2p_advertise = socket_env("GATE14_P2P_ADVERTISE", "127.0.0.1:9300")?;
    let bootstrappers = parse_bootstrappers(&env::var("GATE14_BOOTSTRAPPERS").unwrap_or_default())?;
    let api_bind = socket_env("GATE14_API_BIND", "0.0.0.0:9400")?;
    let data_dir = PathBuf::from(
        env::var("GATE14_DATA_DIR").unwrap_or_else(|_| "/var/lib/obelisk/gate14".to_string()),
    );
    let store = Arc::new(Mutex::new(ValidatorStore::open(
        node_id.clone(),
        data_dir.clone(),
    )?));
    let namespace = union(APPLICATION_NAMESPACE, b"_CONSENSUS");
    let verifier = Scheme::verifier(&namespace, validators.clone());
    spawn_http(
        api_bind,
        HttpState {
            store: store.clone(),
            committee_size: validators.len(),
            verifier,
        },
    )?;

    let runtime_cfg = commonware_tokio::Config::new().with_storage_directory(&data_dir);
    let executor = commonware_tokio::Runner::new(runtime_cfg);
    let p2p_cfg = discovery::Config::local(
        signer.clone(),
        &union(APPLICATION_NAMESPACE, b"_P2P"),
        p2p_bind,
        p2p_advertise,
        bootstrappers,
        1024 * 1024,
    );
    info!(
        %node_id,
        public_key = ?signer.public_key(),
        %p2p_bind,
        %p2p_advertise,
        %api_bind,
        commonware_release = "v2026.2.0",
        "starting Gate 14 Commonware validator"
    );
    executor.start(async move |context| {
        let (mut network, mut oracle) =
            discovery::Network::new(context.with_label("network"), p2p_cfg);
        oracle.track(0, validators.clone()).await;
        let (vote_sender, vote_receiver) = network.register(0, Quota::per_second(NZU32!(50)), 1024);
        let (certificate_sender, certificate_receiver) =
            network.register(1, Quota::per_second(NZU32!(50)), 1024);
        let (resolver_sender, resolver_receiver) =
            network.register(2, Quota::per_second(NZU32!(50)), 1024);
        let scheme = Scheme::signer(&namespace, validators, signer)
            .expect("validator private key is in committee");
        let (application, mailbox) =
            ApplicationActor::new(context.with_label("application"), store.clone());
        let reporter = FinalityReporter { store };
        let cfg = simplex::Config {
            scheme,
            elector: RoundRobin::<Sha256>::default(),
            blocker: oracle,
            automaton: mailbox.clone(),
            relay: mailbox,
            reporter,
            partition: "gate14-control".to_string(),
            mailbox_size: 1024,
            epoch: Epoch::zero(),
            replay_buffer: NZUsize!(1024 * 1024),
            write_buffer: NZUsize!(1024 * 1024),
            leader_timeout: Duration::from_millis(500),
            notarization_timeout: Duration::from_secs(1),
            nullify_retry: Duration::from_secs(3),
            fetch_timeout: Duration::from_secs(1),
            activity_timeout: ViewDelta::new(20),
            skip_timeout: ViewDelta::new(10),
            fetch_concurrent: 32,
            page_cache: CacheRef::new(NZU16!(4_096), NZUsize!(2_048)),
            strategy: Sequential,
        };
        let engine = simplex::Engine::new(context.with_label("engine"), cfg);
        application.start();
        network.start();
        engine.start(
            (vote_sender, vote_receiver),
            (certificate_sender, certificate_receiver),
            (resolver_sender, resolver_receiver),
        );
        std::future::pending::<()>().await;
    });
    Ok(())
}

fn spawn_http(bind: SocketAddr, state: HttpState) -> Result<(), String> {
    thread::Builder::new()
        .name("gate14-validator-http".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("build Gate 14 HTTP runtime");
            runtime.block_on(async move {
                let app = Router::new()
                    .route("/healthz", get(health))
                    .route("/metrics", get(metrics))
                    .route("/v1/status", get(status))
                    .route("/v1/state", get(authoritative_state))
                    .route("/v1/finality", get(finality))
                    .route("/v1/commands", post(submit))
                    .route("/v1/import", post(import_finality))
                    .with_state(state);
                let listener = tokio::net::TcpListener::bind(bind)
                    .await
                    .expect("bind Gate 14 validator HTTP");
                axum::serve(listener, app)
                    .await
                    .expect("serve Gate 14 validator HTTP");
            });
        })
        .map_err(|error| format!("spawn Gate 14 HTTP thread failed: {error}"))?;
    Ok(())
}

async fn health() -> &'static str {
    "ok\n"
}

async fn submit(
    State(state): State<HttpState>,
    Json(command): Json<Gate14CommandEnvelope>,
) -> Result<Json<SubmitResponse>, ApiError> {
    let digest = state
        .store
        .lock()
        .map_err(|_| ApiError("validator store mutex poisoned".to_string()))?
        .submit(command)
        .map_err(ApiError)?;
    Ok(Json(SubmitResponse {
        accepted: true,
        command_digest: digest,
        activation_delay_ms: COMMAND_ACTIVATION_DELAY_MS,
    }))
}

async fn status(State(state): State<HttpState>) -> Result<Json<ValidatorStatus>, ApiError> {
    let store = state
        .store
        .lock()
        .map_err(|_| ApiError("validator store mutex poisoned".to_string()))?;
    let prepared_count = store.commands.len();
    Ok(Json(ValidatorStatus {
        role: "commonware-validator",
        node_id: store.node_id.clone(),
        commonware_release: "v2026.2.0",
        committee_size: state.committee_size,
        quorum: state.committee_size - (state.committee_size.saturating_sub(1) / 3),
        finalized_height: store.state.finalized_height,
        last_sequence: store.state.last_sequence,
        state_root: store.state.state_root().map_err(ApiError)?,
        prepared_count,
        pending_count: prepared_count.saturating_sub(store.finalized.len()),
        started_at_ms: store.started_at_ms,
    }))
}

async fn authoritative_state(
    State(state): State<HttpState>,
) -> Result<Json<Gate14AuthoritativeState>, ApiError> {
    Ok(Json(
        state
            .store
            .lock()
            .map_err(|_| ApiError("validator store mutex poisoned".to_string()))?
            .state
            .clone(),
    ))
}

async fn finality(
    State(state): State<HttpState>,
    Query(query): Query<FinalityQuery>,
) -> Result<Json<Vec<Gate14FinalizedRecord>>, ApiError> {
    Ok(Json(
        state
            .store
            .lock()
            .map_err(|_| ApiError("validator store mutex poisoned".to_string()))?
            .finalized
            .iter()
            .filter(|record| record.height > query.after)
            .cloned()
            .collect(),
    ))
}

async fn import_finality(
    State(state): State<HttpState>,
    Json(records): Json<Vec<Gate14FinalizedRecord>>,
) -> Result<Json<ImportResponse>, ApiError> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| ApiError("validator store mutex poisoned".to_string()))?;
    let imported = store
        .import_verified(records, &state.verifier, state.committee_size)
        .map_err(ApiError)?;
    Ok(Json(ImportResponse {
        accepted: true,
        imported,
        finalized_height: store.state.finalized_height,
        state_root: store.state.state_root().map_err(ApiError)?,
    }))
}

async fn metrics(State(state): State<HttpState>) -> Result<String, ApiError> {
    let store = state
        .store
        .lock()
        .map_err(|_| ApiError("validator store mutex poisoned".to_string()))?;
    Ok(format!(
        "# HELP obelisk_gate14_finalized_height Finalized Commonware control height.\n\
         # TYPE obelisk_gate14_finalized_height gauge\n\
         obelisk_gate14_finalized_height{{validator=\"{}\"}} {}\n\
         # HELP obelisk_gate14_pending_commands Prepared commands waiting for finality.\n\
         # TYPE obelisk_gate14_pending_commands gauge\n\
         obelisk_gate14_pending_commands{{validator=\"{}\"}} {}\n",
        metric_label(&store.node_id),
        store.state.finalized_height,
        metric_label(&store.node_id),
        store.commands.len().saturating_sub(store.finalized.len())
    ))
}

fn consensus_digest(command: &Gate14CommandEnvelope) -> Result<(ConsensusDigest, String), String> {
    let bytes = serde_json::to_vec(command)
        .map_err(|error| format!("encode command for Commonware failed: {error}"))?;
    let mut hasher = Sha256::default();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let key = digest_key(&digest);
    Ok((digest, key))
}

fn digest_key(digest: &ConsensusDigest) -> String {
    commonware_utils::hex(digest)
}

fn append_json_line(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("open {} for append failed: {error}", path.display()))?;
    serde_json::to_writer(&mut file, value)
        .map_err(|error| format!("encode {} failed: {error}", path.display()))?;
    file.write_all(b"\n")
        .and_then(|_| file.sync_data())
        .map_err(|error| format!("flush {} failed: {error}", path.display()))
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("encode {} failed: {error}", path.display()))?;
    bytes.push(b'\n');
    fs::write(&temporary, bytes)
        .map_err(|error| format!("write {} failed: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("replace {} failed: {error}", path.display()))
}

fn parse_bootstrappers(value: &str) -> Result<Vec<(ed25519::PublicKey, Ingress)>, String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|entry| {
            let (seed, address) = entry
                .split_once('@')
                .ok_or_else(|| format!("invalid bootstrapper {entry}; expected seed@ip:port"))?;
            let seed = seed
                .parse::<u64>()
                .map_err(|error| format!("invalid bootstrapper seed {seed}: {error}"))?;
            let address = SocketAddr::from_str(address)
                .map_err(|error| format!("invalid bootstrapper address {address}: {error}"))?;
            Ok((
                ed25519::PrivateKey::from_seed(seed).public_key(),
                address.into(),
            ))
        })
        .collect()
}

fn socket_env(name: &str, default: &str) -> Result<SocketAddr, String> {
    let value = env::var(name).unwrap_or_else(|_| default.to_string());
    SocketAddr::from_str(&value).map_err(|error| format!("invalid {name}={value}: {error}"))
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))
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
