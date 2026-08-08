use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use rand::rngs::OsRng;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{NodeSigningIdentity, verify_ed25519_signature};

pub const HOME_AGENT_RELEASE_SCHEMA: &str = "obelisk.home-agent-release.v1";
pub const HOME_AGENT_RELEASE_SIGNATURE_ALGORITHM: &str = "ed25519-zip215";
pub const HOME_AGENT_KEYRING_SERVICE: &str = "com.obelisk-labs.dubhe-home-agent";
pub const HOME_AGENT_MANAGEMENT_KEYRING_SUFFIX: &str = "management-token";
pub const HOME_AGENT_BUNDLE_MAX_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeAgentArtifact {
    pub target: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeAgentReleaseManifestPayload {
    pub schema: String,
    pub channel: String,
    pub version: String,
    pub published_at_ms: u64,
    pub expires_at_ms: u64,
    pub minimum_agent_version: String,
    pub rollout_id: String,
    pub artifacts: Vec<HomeAgentArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeAgentReleaseManifest {
    pub payload: HomeAgentReleaseManifestPayload,
    pub signature_algorithm: String,
    pub signature: String,
}

impl HomeAgentReleaseManifest {
    pub fn sign(
        payload: HomeAgentReleaseManifestPayload,
        issuer: &NodeSigningIdentity,
    ) -> Result<Self, String> {
        validate_release_payload(&payload)?;
        let bytes = serde_json::to_vec(&payload)
            .map_err(|error| format!("serialize Home Agent release payload: {error}"))?;
        Ok(Self {
            payload,
            signature_algorithm: HOME_AGENT_RELEASE_SIGNATURE_ALGORITHM.to_string(),
            signature: issuer.sign(&bytes),
        })
    }

    pub fn verify(
        &self,
        trusted_issuer: &str,
        expected_channel: &str,
        expected_target: &str,
        current_version: &str,
        now_ms: u64,
    ) -> Result<HomeAgentArtifact, String> {
        let artifact =
            self.verify_signed_artifact(trusted_issuer, expected_channel, expected_target, now_ms)?;
        let current = Version::parse(current_version)
            .map_err(|error| format!("invalid current Home Agent version: {error}"))?;
        let minimum = Version::parse(&self.payload.minimum_agent_version)
            .map_err(|error| format!("invalid minimum Home Agent version: {error}"))?;
        let release = Version::parse(&self.payload.version)
            .map_err(|error| format!("invalid release Home Agent version: {error}"))?;
        if current < minimum {
            return Err(format!(
                "current Home Agent {current} is below release minimum {minimum}; bootstrap update required"
            ));
        }
        if release <= current {
            return Err(format!(
                "Home Agent release {release} is not newer than current {current}"
            ));
        }
        Ok(artifact)
    }

    pub fn verify_signed_artifact(
        &self,
        trusted_issuer: &str,
        expected_channel: &str,
        expected_target: &str,
        now_ms: u64,
    ) -> Result<HomeAgentArtifact, String> {
        validate_release_payload(&self.payload)?;
        if self.signature_algorithm != HOME_AGENT_RELEASE_SIGNATURE_ALGORITHM {
            return Err("unsupported Home Agent release signature algorithm".to_string());
        }
        let bytes = serde_json::to_vec(&self.payload)
            .map_err(|error| format!("serialize Home Agent release payload: {error}"))?;
        verify_ed25519_signature(trusted_issuer, &bytes, &self.signature)?;
        if self.payload.channel != expected_channel {
            return Err(format!(
                "release channel {} does not match expected {expected_channel}",
                self.payload.channel
            ));
        }
        if now_ms < self.payload.published_at_ms || now_ms > self.payload.expires_at_ms {
            return Err("Home Agent release manifest is not currently valid".to_string());
        }
        self.payload
            .artifacts
            .iter()
            .find(|artifact| artifact.target == expected_target)
            .cloned()
            .ok_or_else(|| format!("release has no artifact for target {expected_target}"))
    }
}

fn validate_release_payload(payload: &HomeAgentReleaseManifestPayload) -> Result<(), String> {
    if payload.schema != HOME_AGENT_RELEASE_SCHEMA {
        return Err("unsupported Home Agent release manifest schema".to_string());
    }
    if payload.channel.trim().is_empty()
        || payload.channel.len() > 32
        || payload.rollout_id.trim().is_empty()
        || payload.rollout_id.len() > 128
        || payload.expires_at_ms <= payload.published_at_ms
        || payload.artifacts.is_empty()
    {
        return Err("Home Agent release manifest contains invalid metadata".to_string());
    }
    Version::parse(&payload.version)
        .map_err(|error| format!("invalid release Home Agent version: {error}"))?;
    Version::parse(&payload.minimum_agent_version)
        .map_err(|error| format!("invalid minimum Home Agent version: {error}"))?;
    let mut targets = BTreeSet::new();
    for artifact in &payload.artifacts {
        if artifact.target.trim().is_empty()
            || artifact.url.trim().is_empty()
            || artifact.size_bytes == 0
            || !targets.insert(artifact.target.as_str())
        {
            return Err("Home Agent release contains an invalid artifact".to_string());
        }
        decode_sha256(&artifact.sha256)?;
        let url = reqwest::Url::parse(&artifact.url)
            .map_err(|error| format!("invalid Home Agent artifact URL: {error}"))?;
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.host_str().is_none()
            || url.fragment().is_some()
        {
            return Err(
                "Home Agent release artifact URL must use HTTPS without credentials or fragments"
                    .to_string(),
            );
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HomeAgentWorkMode {
    Serving,
    Draining,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeAgentResourceSample {
    pub observed_at_ms: u64,
    pub cpu_usage_percent: f32,
    pub available_memory_bytes: u64,
    pub active_sessions: usize,
    pub elapsed_since_previous_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeAgentResourcePolicy {
    pub maximum_cpu_percent: f32,
    pub minimum_available_memory_bytes: u64,
    pub overload_samples_before_drain: u32,
    pub recovery_samples_before_resume: u32,
    pub expected_sample_interval_ms: u64,
    pub sleep_gap_multiplier: u32,
}

impl Default for HomeAgentResourcePolicy {
    fn default() -> Self {
        Self {
            maximum_cpu_percent: 75.0,
            minimum_available_memory_bytes: 2 * 1024 * 1024 * 1024,
            overload_samples_before_drain: 3,
            recovery_samples_before_resume: 12,
            expected_sample_interval_ms: 5_000,
            sleep_gap_multiplier: 3,
        }
    }
}

impl HomeAgentResourcePolicy {
    pub fn validate(&self) -> Result<(), String> {
        if !(1.0..=100.0).contains(&self.maximum_cpu_percent)
            || self.minimum_available_memory_bytes == 0
            || self.overload_samples_before_drain == 0
            || self.recovery_samples_before_resume == 0
            || self.expected_sample_interval_ms < 500
            || self.sleep_gap_multiplier < 2
        {
            return Err("Home Agent resource policy contains an invalid limit".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeAgentResourceDecision {
    pub mode: HomeAgentWorkMode,
    pub accept_new_sessions: bool,
    pub request_drain: bool,
    pub request_resume: bool,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct HomeAgentResourceController {
    policy: HomeAgentResourcePolicy,
    mode: HomeAgentWorkMode,
    manual_drain: bool,
    overload_samples: u32,
    recovery_samples: u32,
}

impl HomeAgentResourceController {
    pub fn new(policy: HomeAgentResourcePolicy) -> Result<Self, String> {
        policy.validate()?;
        Ok(Self {
            policy,
            mode: HomeAgentWorkMode::Serving,
            manual_drain: false,
            overload_samples: 0,
            recovery_samples: 0,
        })
    }

    pub fn mode(&self) -> HomeAgentWorkMode {
        self.mode
    }

    pub fn set_manual_drain(&mut self, enabled: bool) {
        self.manual_drain = enabled;
        if enabled {
            self.mode = HomeAgentWorkMode::Draining;
            self.overload_samples = 0;
            self.recovery_samples = 0;
        }
    }

    pub fn observe(&mut self, sample: HomeAgentResourceSample) -> HomeAgentResourceDecision {
        let woke_from_sleep = sample.elapsed_since_previous_ms
            > self
                .policy
                .expected_sample_interval_ms
                .saturating_mul(self.policy.sleep_gap_multiplier as u64);
        let overloaded = sample.cpu_usage_percent > self.policy.maximum_cpu_percent
            || sample.available_memory_bytes < self.policy.minimum_available_memory_bytes;
        if woke_from_sleep || self.manual_drain {
            let changed = self.mode == HomeAgentWorkMode::Serving;
            self.mode = if sample.active_sessions == 0 {
                HomeAgentWorkMode::Paused
            } else {
                HomeAgentWorkMode::Draining
            };
            return decision(
                self.mode,
                changed,
                false,
                if woke_from_sleep {
                    "system_sleep_or_resume_detected"
                } else {
                    "manual_drain"
                },
            );
        }
        if overloaded {
            self.overload_samples = self.overload_samples.saturating_add(1);
            self.recovery_samples = 0;
            if self.overload_samples >= self.policy.overload_samples_before_drain {
                let changed = self.mode == HomeAgentWorkMode::Serving;
                self.mode = if sample.active_sessions == 0 {
                    HomeAgentWorkMode::Paused
                } else {
                    HomeAgentWorkMode::Draining
                };
                return decision(self.mode, changed, false, "host_resource_pressure");
            }
            return decision(self.mode, false, false, "pressure_grace_period");
        }
        self.overload_samples = 0;
        if self.mode == HomeAgentWorkMode::Draining && sample.active_sessions == 0 {
            self.mode = HomeAgentWorkMode::Paused;
        }
        if self.mode != HomeAgentWorkMode::Serving {
            self.recovery_samples = self.recovery_samples.saturating_add(1);
            if self.recovery_samples >= self.policy.recovery_samples_before_resume {
                self.mode = HomeAgentWorkMode::Serving;
                self.recovery_samples = 0;
                return decision(self.mode, false, true, "host_resources_recovered");
            }
            return decision(self.mode, false, false, "recovery_observation");
        }
        self.recovery_samples = 0;
        decision(self.mode, false, false, "healthy")
    }
}

fn decision(
    mode: HomeAgentWorkMode,
    request_drain: bool,
    request_resume: bool,
    reason: &str,
) -> HomeAgentResourceDecision {
    HomeAgentResourceDecision {
        mode,
        accept_new_sessions: mode == HomeAgentWorkMode::Serving,
        request_drain,
        request_resume,
        reason: reason.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct HomeAgentKeyring {
    account: String,
}

impl HomeAgentKeyring {
    pub fn new(account: impl Into<String>) -> Result<Self, String> {
        let account = account.into();
        if account.trim().is_empty() || account.len() > 256 {
            return Err("Home Agent keyring account is invalid".to_string());
        }
        Ok(Self { account })
    }

    pub fn load_identity(&self) -> Result<NodeSigningIdentity, String> {
        let secret = self.entry()?.get_secret().map_err(|error| {
            format!("read Home Agent identity from operating-system keyring: {error}")
        })?;
        if secret.len() != 32 {
            return Err("Home Agent keyring identity must contain exactly 32 bytes".to_string());
        }
        let seed: [u8; 32] = secret
            .try_into()
            .map_err(|_| "Home Agent keyring identity length mismatch".to_string())?;
        Ok(NodeSigningIdentity::from_seed(seed))
    }

    pub fn load_or_create_identity(&self) -> Result<(NodeSigningIdentity, bool), String> {
        let entry = self.entry()?;
        match entry.get_secret() {
            Ok(secret) => {
                let seed: [u8; 32] = secret.try_into().map_err(|_| {
                    "Home Agent keyring identity must contain exactly 32 bytes".to_string()
                })?;
                Ok((NodeSigningIdentity::from_seed(seed), false))
            }
            Err(keyring::Error::NoEntry) => {
                let mut seed = [0_u8; 32];
                OsRng.fill_bytes(&mut seed);
                entry
                    .set_secret(&seed)
                    .map_err(|error| format!("store Home Agent identity in keyring: {error}"))?;
                Ok((NodeSigningIdentity::from_seed(seed), true))
            }
            Err(error) => Err(format!(
                "read Home Agent identity from operating-system keyring: {error}"
            )),
        }
    }

    pub fn import_base64_seed(&self, value: &str) -> Result<NodeSigningIdentity, String> {
        let decoded = URL_SAFE_NO_PAD
            .decode(value.trim())
            .map_err(|_| "Home Agent seed must be URL-safe base64 without padding".to_string())?;
        let seed: [u8; 32] = decoded
            .try_into()
            .map_err(|_| "Home Agent seed must decode to exactly 32 bytes".to_string())?;
        self.entry()?
            .set_secret(&seed)
            .map_err(|error| format!("store Home Agent identity in keyring: {error}"))?;
        Ok(NodeSigningIdentity::from_seed(seed))
    }

    pub fn store_secret(&self, secret: &[u8]) -> Result<(), String> {
        if secret.is_empty() {
            return Err("refusing to store an empty Home Agent keyring secret".to_string());
        }
        self.entry()?
            .set_secret(secret)
            .map_err(|error| format!("store Home Agent keyring secret: {error}"))
    }

    pub fn load_secret(&self) -> Result<Vec<u8>, String> {
        self.entry()?
            .get_secret()
            .map_err(|error| format!("read Home Agent keyring secret: {error}"))
    }

    pub fn delete_secret(&self) -> Result<(), String> {
        self.entry()?
            .delete_credential()
            .map_err(|error| format!("delete Home Agent keyring secret: {error}"))
    }

    fn entry(&self) -> Result<keyring::Entry, String> {
        keyring::Entry::new(HOME_AGENT_KEYRING_SERVICE, &self.account)
            .map_err(|error| format!("open operating-system keyring: {error}"))
    }
}

#[derive(Debug, Clone)]
pub struct HomeAgentManagementKeyring {
    secret: HomeAgentKeyring,
}

impl HomeAgentManagementKeyring {
    pub fn new(account: impl Into<String>) -> Result<Self, String> {
        let account = account.into();
        if account.trim().is_empty() || account.len() > 220 {
            return Err("Home Agent management keyring account is invalid".to_string());
        }
        Ok(Self {
            secret: HomeAgentKeyring::new(format!(
                "{account}:{HOME_AGENT_MANAGEMENT_KEYRING_SUFFIX}"
            ))?,
        })
    }

    pub fn load_token(&self) -> Result<String, String> {
        let secret = self.secret.load_secret()?;
        validate_management_secret(&secret)?;
        Ok(URL_SAFE_NO_PAD.encode(secret))
    }

    pub fn load_or_create_token(&self) -> Result<(String, bool), String> {
        let entry = self.secret.entry()?;
        match entry.get_secret() {
            Ok(secret) => {
                validate_management_secret(&secret)?;
                Ok((URL_SAFE_NO_PAD.encode(secret), false))
            }
            Err(keyring::Error::NoEntry) => {
                let mut secret = [0_u8; 32];
                OsRng.fill_bytes(&mut secret);
                entry
                    .set_secret(&secret)
                    .map_err(|error| format!("store Home Agent management token: {error}"))?;
                Ok((URL_SAFE_NO_PAD.encode(secret), true))
            }
            Err(error) => Err(format!(
                "read Home Agent management token from operating-system keyring: {error}"
            )),
        }
    }

    pub fn delete_token(&self) -> Result<(), String> {
        self.secret.delete_secret()
    }
}

fn validate_management_secret(secret: &[u8]) -> Result<(), String> {
    if secret.len() != 32 {
        return Err(
            "Home Agent management keyring token must contain exactly 32 bytes".to_string(),
        );
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeAgentUpdateState {
    pub current_version: String,
    pub previous_version: Option<String>,
    pub staged_version: Option<String>,
    pub failed_versions: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub struct HomeAgentUpdateStore {
    root: PathBuf,
}

impl HomeAgentUpdateStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn load_state(&self, fallback_version: &str) -> Result<HomeAgentUpdateState, String> {
        Version::parse(fallback_version)
            .map_err(|error| format!("invalid fallback Home Agent version: {error}"))?;
        let path = self.root.join("state.json");
        if !path.exists() {
            return Ok(HomeAgentUpdateState {
                current_version: fallback_version.to_string(),
                previous_version: None,
                staged_version: None,
                failed_versions: BTreeSet::new(),
            });
        }
        let state: HomeAgentUpdateState =
            serde_json::from_slice(&fs::read(&path).map_err(|error| {
                format!("read Home Agent update state {}: {error}", path.display())
            })?)
            .map_err(|error| format!("decode Home Agent update state: {error}"))?;
        validate_update_state(&state)?;
        Ok(state)
    }

    pub fn stage(
        &self,
        state: &mut HomeAgentUpdateState,
        manifest: &HomeAgentReleaseManifest,
        artifact: &HomeAgentArtifact,
        bytes: &[u8],
    ) -> Result<PathBuf, String> {
        if bytes.len() as u64 != artifact.size_bytes {
            return Err(format!(
                "Home Agent artifact length {} does not match manifest {}",
                bytes.len(),
                artifact.size_bytes
            ));
        }
        let actual = hex_digest(&Sha256::digest(bytes));
        if !constant_time_equal(actual.as_bytes(), artifact.sha256.as_bytes()) {
            return Err("Home Agent artifact SHA-256 mismatch".to_string());
        }
        if state.failed_versions.contains(&manifest.payload.version) {
            return Err(
                "Home Agent release is quarantined after a failed health check".to_string(),
            );
        }
        let release_dir = self.root.join("versions").join(&manifest.payload.version);
        fs::create_dir_all(&release_dir).map_err(|error| {
            format!(
                "create Home Agent staged release directory {}: {error}",
                release_dir.display()
            )
        })?;
        let final_path = release_dir.join("dubhe-home-agent");
        let temporary = release_dir.join(format!(".artifact-{}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| {
                format!(
                    "create staged Home Agent artifact {}: {error}",
                    temporary.display()
                )
            })?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("persist staged Home Agent artifact: {error}"))?;
        set_artifact_permissions(&temporary)?;
        fs::rename(&temporary, &final_path)
            .map_err(|error| format!("activate staged Home Agent artifact path: {error}"))?;
        sync_directory(&release_dir)?;
        state.staged_version = Some(manifest.payload.version.clone());
        self.persist_state(state)?;
        Ok(final_path)
    }

    pub fn stage_bundle(
        &self,
        state: &mut HomeAgentUpdateState,
        manifest: &HomeAgentReleaseManifest,
        artifact: &HomeAgentArtifact,
        bytes: &[u8],
    ) -> Result<PathBuf, String> {
        verify_artifact_bytes(artifact, bytes)?;
        if state.failed_versions.contains(&manifest.payload.version) {
            return Err(
                "Home Agent release is quarantined after a failed health check".to_string(),
            );
        }
        let versions = self.root.join("versions");
        fs::create_dir_all(&versions)
            .map_err(|error| format!("create Home Agent versions directory: {error}"))?;
        let release_dir = versions.join(&manifest.payload.version);
        if release_dir.exists() {
            return Err(format!(
                "Home Agent release directory {} already exists",
                release_dir.display()
            ));
        }
        let temporary = versions.join(format!(
            ".{}-{}.tmp",
            manifest.payload.version,
            std::process::id()
        ));
        if temporary.exists() {
            return Err(format!(
                "Home Agent temporary release directory {} already exists",
                temporary.display()
            ));
        }
        fs::create_dir(&temporary).map_err(|error| {
            format!(
                "create Home Agent temporary release directory {}: {error}",
                temporary.display()
            )
        })?;
        let result = extract_home_agent_bundle(bytes, &temporary).and_then(|()| {
            sync_directory(&temporary)?;
            fs::rename(&temporary, &release_dir).map_err(|error| {
                format!(
                    "activate Home Agent version directory {}: {error}",
                    release_dir.display()
                )
            })?;
            sync_directory(&versions)?;
            state.staged_version = Some(manifest.payload.version.clone());
            self.persist_state(state)?;
            Ok(release_dir.clone())
        });
        if result.is_err() {
            let _ = fs::remove_dir_all(&temporary);
        }
        result
    }

    pub fn version_binary(&self, version: &str, name: &str) -> Result<PathBuf, String> {
        let version = Version::parse(version)
            .map_err(|error| format!("invalid Home Agent version {version}: {error}"))?;
        let name = platform_binary_name(name)?;
        Ok(self
            .root
            .join("versions")
            .join(version.to_string())
            .join(name))
    }

    pub fn activate(&self, state: &mut HomeAgentUpdateState) -> Result<(), String> {
        let staged = state
            .staged_version
            .take()
            .ok_or_else(|| "no staged Home Agent release".to_string())?;
        if state.failed_versions.contains(&staged) {
            return Err("staged Home Agent release is quarantined".to_string());
        }
        state.previous_version = Some(state.current_version.clone());
        state.current_version = staged;
        self.persist_state(state)
    }

    pub fn record_health_failure(&self, state: &mut HomeAgentUpdateState) -> Result<(), String> {
        let failed = state.current_version.clone();
        let previous = state
            .previous_version
            .take()
            .ok_or_else(|| "Home Agent update has no rollback version".to_string())?;
        state.failed_versions.insert(failed);
        state.current_version = previous;
        state.staged_version = None;
        self.persist_state(state)
    }

    pub fn persist_state(&self, state: &HomeAgentUpdateState) -> Result<(), String> {
        fs::create_dir_all(&self.root)
            .map_err(|error| format!("create Home Agent update root: {error}"))?;
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|error| format!("serialize Home Agent update state: {error}"))?;
        atomic_write(&self.root.join("state.json"), &bytes)
    }
}

fn validate_update_state(state: &HomeAgentUpdateState) -> Result<(), String> {
    Version::parse(&state.current_version)
        .map_err(|error| format!("invalid current Home Agent version: {error}"))?;
    if let Some(version) = state.previous_version.as_deref() {
        Version::parse(version)
            .map_err(|error| format!("invalid previous Home Agent version: {error}"))?;
    }
    if let Some(version) = state.staged_version.as_deref() {
        Version::parse(version)
            .map_err(|error| format!("invalid staged Home Agent version: {error}"))?;
    }
    for version in &state.failed_versions {
        Version::parse(version)
            .map_err(|error| format!("invalid failed Home Agent version: {error}"))?;
    }
    if state
        .previous_version
        .as_deref()
        .is_some_and(|version| version == state.current_version)
    {
        return Err("Home Agent previous and current versions must differ".to_string());
    }
    Ok(())
}

fn verify_artifact_bytes(artifact: &HomeAgentArtifact, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() as u64 != artifact.size_bytes {
        return Err(format!(
            "Home Agent artifact length {} does not match manifest {}",
            bytes.len(),
            artifact.size_bytes
        ));
    }
    let actual = hex_digest(&Sha256::digest(bytes));
    if !constant_time_equal(actual.as_bytes(), artifact.sha256.as_bytes()) {
        return Err("Home Agent artifact SHA-256 mismatch".to_string());
    }
    Ok(())
}

fn extract_home_agent_bundle(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    let required = platform_bundle_binary_names();
    let mut extracted = BTreeSet::new();
    let mut uncompressed_bytes = 0_u64;
    let entries = archive
        .entries()
        .map_err(|error| format!("open Home Agent bundle: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read Home Agent bundle entry: {error}"))?;
        if !entry.header().entry_type().is_file() {
            return Err("Home Agent bundle may contain only regular files".to_string());
        }
        let path = entry
            .path()
            .map_err(|error| format!("decode Home Agent bundle path: {error}"))?;
        if path.components().count() != 1 {
            return Err("Home Agent bundle paths must be flat and relative".to_string());
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "Home Agent bundle contains a non-UTF-8 file name".to_string())?
            .to_string();
        if !required.contains(&name) || !extracted.insert(name.clone()) {
            return Err(format!(
                "Home Agent bundle contains an unexpected or duplicate file {name}"
            ));
        }
        let declared = entry
            .header()
            .size()
            .map_err(|error| format!("read Home Agent bundle entry size: {error}"))?;
        uncompressed_bytes = uncompressed_bytes
            .checked_add(declared)
            .ok_or_else(|| "Home Agent bundle uncompressed length overflow".to_string())?;
        if uncompressed_bytes > HOME_AGENT_BUNDLE_MAX_UNCOMPRESSED_BYTES {
            return Err("Home Agent bundle exceeds the uncompressed size limit".to_string());
        }
        let path = destination.join(name);
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                format!(
                    "create Home Agent bundle binary {}: {error}",
                    path.display()
                )
            })?;
        let copied = std::io::copy(&mut entry.take(declared + 1), &mut output)
            .map_err(|error| format!("extract Home Agent bundle binary: {error}"))?;
        if copied != declared {
            return Err("Home Agent bundle entry size does not match its header".to_string());
        }
        output
            .sync_all()
            .map_err(|error| format!("fsync Home Agent bundle binary: {error}"))?;
        set_artifact_permissions(&path)?;
    }
    if extracted != required {
        return Err("Home Agent bundle is missing one or more required binaries".to_string());
    }
    Ok(())
}

fn platform_bundle_binary_names() -> BTreeSet<String> {
    ["home_agent", "home_agent_supervisor", "zone_host"]
        .into_iter()
        .map(|name| platform_binary_name(name).expect("static binary name must be valid"))
        .collect()
}

fn platform_binary_name(name: &str) -> Result<String, String> {
    if !matches!(name, "home_agent" | "home_agent_supervisor" | "zone_host") {
        return Err("unsupported Home Agent bundle binary name".to_string());
    }
    #[cfg(windows)]
    {
        Ok(format!("{name}.exe"))
    }
    #[cfg(not(windows))]
    {
        Ok(name.to_string())
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "atomic Home Agent state path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("create state directory: {error}"))?;
    let temporary = parent.join(format!(".state-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("create temporary Home Agent state: {error}"))?;
    file.write_all(bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("persist temporary Home Agent state: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("replace Home Agent state atomically: {error}"))?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("fsync directory {}: {error}", path.display()))
}

#[cfg(unix)]
fn set_artifact_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o500))
        .map_err(|error| format!("restrict Home Agent artifact permissions: {error}"))
}

#[cfg(not(unix))]
fn set_artifact_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn decode_sha256(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err("Home Agent artifact SHA-256 must contain 64 hex characters".to_string());
    }
    let mut bytes = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk)
            .map_err(|_| "Home Agent artifact SHA-256 is not UTF-8".to_string())?;
        bytes[index] = u8::from_str_radix(text, 16)
            .map_err(|_| "Home Agent artifact SHA-256 contains invalid hex".to_string())?;
    }
    Ok(bytes)
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut archive = tar::Builder::new(encoder);
        for name in platform_bundle_binary_names() {
            let bytes = format!("signed-{name}").into_bytes();
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o500);
            header.set_cksum();
            archive
                .append_data(&mut header, name, bytes.as_slice())
                .unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap()
    }

    fn release(
        issuer: &NodeSigningIdentity,
        version: &str,
        bytes: &[u8],
    ) -> HomeAgentReleaseManifest {
        HomeAgentReleaseManifest::sign(
            HomeAgentReleaseManifestPayload {
                schema: HOME_AGENT_RELEASE_SCHEMA.to_string(),
                channel: "stable".to_string(),
                version: version.to_string(),
                published_at_ms: 1_000,
                expires_at_ms: 3_000,
                minimum_agent_version: "1.0.0".to_string(),
                rollout_id: format!("rollout-{version}"),
                artifacts: vec![HomeAgentArtifact {
                    target: "aarch64-apple-darwin".to_string(),
                    url: "https://updates.example.invalid/home-agent".to_string(),
                    sha256: hex_digest(&Sha256::digest(bytes)),
                    size_bytes: bytes.len() as u64,
                }],
            },
            issuer,
        )
        .unwrap()
    }

    #[test]
    fn signed_release_rejects_tamper_expiry_and_rollback() {
        let issuer = NodeSigningIdentity::from_seed([81; 32]);
        let manifest = release(&issuer, "1.1.0", b"signed-home-agent");
        assert!(
            manifest
                .verify(
                    issuer.public_key(),
                    "stable",
                    "aarch64-apple-darwin",
                    "1.0.0",
                    2_000,
                )
                .is_ok()
        );
        let mut tampered = manifest.clone();
        tampered.payload.version = "9.9.9".to_string();
        assert!(
            tampered
                .verify(
                    issuer.public_key(),
                    "stable",
                    "aarch64-apple-darwin",
                    "1.0.0",
                    2_000,
                )
                .is_err()
        );
        assert!(
            manifest
                .verify(
                    issuer.public_key(),
                    "stable",
                    "aarch64-apple-darwin",
                    "1.0.0",
                    4_000,
                )
                .is_err()
        );
        assert!(
            manifest
                .verify(
                    issuer.public_key(),
                    "stable",
                    "aarch64-apple-darwin",
                    "1.1.0",
                    2_000,
                )
                .is_err()
        );
    }

    #[test]
    fn resource_pressure_sleep_and_recovery_are_fail_closed() {
        let mut controller = HomeAgentResourceController::new(HomeAgentResourcePolicy {
            overload_samples_before_drain: 2,
            recovery_samples_before_resume: 2,
            ..HomeAgentResourcePolicy::default()
        })
        .unwrap();
        let overloaded = |active_sessions| HomeAgentResourceSample {
            observed_at_ms: 1,
            cpu_usage_percent: 90.0,
            available_memory_bytes: 4 * 1024 * 1024 * 1024,
            active_sessions,
            elapsed_since_previous_ms: 5_000,
        };
        assert!(!controller.observe(overloaded(3)).request_drain);
        let drain = controller.observe(overloaded(3));
        assert!(drain.request_drain);
        assert_eq!(drain.mode, HomeAgentWorkMode::Draining);
        let healthy = HomeAgentResourceSample {
            observed_at_ms: 2,
            cpu_usage_percent: 10.0,
            available_memory_bytes: 4 * 1024 * 1024 * 1024,
            active_sessions: 0,
            elapsed_since_previous_ms: 5_000,
        };
        assert_eq!(controller.observe(healthy).mode, HomeAgentWorkMode::Paused);
        let resume = controller.observe(healthy);
        assert!(resume.request_resume);
        let sleep = controller.observe(HomeAgentResourceSample {
            elapsed_since_previous_ms: 60_000,
            ..healthy
        });
        assert!(!sleep.accept_new_sessions);
        assert!(sleep.request_drain);
    }

    #[test]
    fn update_store_stages_activates_and_rolls_back_atomically() {
        let issuer = NodeSigningIdentity::from_seed([82; 32]);
        let bytes = b"new-home-agent";
        let manifest = release(&issuer, "1.1.0", bytes);
        let artifact = manifest.payload.artifacts[0].clone();
        let root = std::env::temp_dir().join(format!(
            "mir2-home-agent-update-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let store = HomeAgentUpdateStore::new(&root);
        let mut state = HomeAgentUpdateState {
            current_version: "1.0.0".to_string(),
            previous_version: None,
            staged_version: None,
            failed_versions: BTreeSet::new(),
        };
        let path = store
            .stage(&mut state, &manifest, &artifact, bytes)
            .unwrap();
        assert_eq!(fs::read(path).unwrap(), bytes);
        store.activate(&mut state).unwrap();
        assert_eq!(state.current_version, "1.1.0");
        store.record_health_failure(&mut state).unwrap();
        assert_eq!(state.current_version, "1.0.0");
        assert!(state.failed_versions.contains("1.1.0"));
        assert!(
            store
                .stage(&mut state, &manifest, &artifact, bytes)
                .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn signed_bundle_extracts_only_required_flat_binaries() {
        let issuer = NodeSigningIdentity::from_seed([83; 32]);
        let bytes = bundle();
        let manifest = release(&issuer, "1.2.0", &bytes);
        let artifact = manifest.payload.artifacts[0].clone();
        let root = std::env::temp_dir().join(format!(
            "mir2-home-agent-bundle-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let store = HomeAgentUpdateStore::new(&root);
        let mut state = HomeAgentUpdateState {
            current_version: "1.1.0".to_string(),
            previous_version: None,
            staged_version: None,
            failed_versions: BTreeSet::new(),
        };
        let release_dir = store
            .stage_bundle(&mut state, &manifest, &artifact, &bytes)
            .unwrap();
        for name in platform_bundle_binary_names() {
            assert!(release_dir.join(name).is_file());
        }
        store.activate(&mut state).unwrap();
        assert_eq!(state.current_version, "1.2.0");
        assert_eq!(
            store
                .version_binary("1.2.0", "home_agent_supervisor")
                .unwrap()
                .parent(),
            Some(release_dir.as_path())
        );
        fs::remove_dir_all(root).unwrap();
    }
}
