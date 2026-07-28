//! Production AI live-broadcast program layer.
//!
//! This module only consumes already-sanitized [`SpectatorFrame`] values. It never receives a
//! player session or an authoritative Zone command handle, so model, TTS, Discord, and encoder
//! failures cannot affect gameplay.

use crate::ai_distribution::{
    AiDistributionChannel, AiDistributionConfig, AiDistributionHub, AiDistributionMetrics,
    AiDistributionStatus,
};
use crate::spectator::{SpectatorEvent, SpectatorFrame, SpectatorHub};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const AI_LIVE_SCHEMA: &str = "obelisk.mir2.ai-live.v1";
const MAX_COMMENTARY_CHARS: usize = 220;
const MAX_SUBTITLE_CHARS: usize = 96;
const MAX_REASON_CHARS: usize = 160;
const MAX_MODEL_RESPONSE_BYTES: usize = 32 * 1024;
const MAX_RECENT_SEGMENTS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiLiveMode {
    Shadow,
    Live,
    Paused,
}

#[derive(Debug, Clone)]
pub struct AiLiveConfig {
    pub enabled: bool,
    pub mode: AiLiveMode,
    pub operator_token: Option<String>,
    pub text_endpoint: Option<String>,
    pub text_api_key: Option<String>,
    pub text_model: String,
    pub tts_endpoint: Option<String>,
    pub tts_api_key: Option<String>,
    pub tts_model: String,
    pub tts_voice: String,
    pub poll_interval_ms: u64,
    pub minimum_score: u8,
    pub commentary_cooldown_ms: u64,
    pub data_dir: PathBuf,
    pub production: bool,
}

impl AiLiveConfig {
    pub fn from_env() -> Result<Self, String> {
        let production = bool_env("MIR2_PRODUCTION", false);
        let enabled = bool_env("MIR2_AI_LIVE_ENABLED", false);
        let mode = match env::var("MIR2_AI_LIVE_MODE")
            .unwrap_or_else(|_| "shadow".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "shadow" => AiLiveMode::Shadow,
            "live" => AiLiveMode::Live,
            "paused" | "pause" => AiLiveMode::Paused,
            other => return Err(format!("unsupported MIR2_AI_LIVE_MODE: {other}")),
        };
        let config = Self {
            enabled,
            mode,
            operator_token: optional_env("MIR2_AI_LIVE_OPERATOR_TOKEN"),
            text_endpoint: optional_env("MIR2_AI_LIVE_TEXT_ENDPOINT"),
            text_api_key: optional_env("MIR2_AI_LIVE_TEXT_API_KEY"),
            text_model: env::var("MIR2_AI_LIVE_TEXT_MODEL")
                .unwrap_or_else(|_| "gpt-5-mini".to_string()),
            tts_endpoint: optional_env("MIR2_AI_LIVE_TTS_ENDPOINT"),
            tts_api_key: optional_env("MIR2_AI_LIVE_TTS_API_KEY"),
            tts_model: env::var("MIR2_AI_LIVE_TTS_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini-tts".to_string()),
            tts_voice: env::var("MIR2_AI_LIVE_TTS_VOICE").unwrap_or_else(|_| "alloy".to_string()),
            poll_interval_ms: bounded_u64_env("MIR2_AI_LIVE_POLL_MS", 500, 100, 10_000),
            minimum_score: bounded_u64_env("MIR2_AI_LIVE_MIN_SCORE", 60, 1, 100) as u8,
            commentary_cooldown_ms: bounded_u64_env(
                "MIR2_AI_LIVE_COOLDOWN_MS",
                8_000,
                500,
                300_000,
            ),
            data_dir: env::var("MIR2_AI_LIVE_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(".mir2-data/ai-live")),
            production,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn disabled_for_tests(data_dir: PathBuf) -> Self {
        Self {
            enabled: false,
            mode: AiLiveMode::Shadow,
            operator_token: None,
            text_endpoint: None,
            text_api_key: None,
            text_model: "test-model".to_string(),
            tts_endpoint: None,
            tts_api_key: None,
            tts_model: "test-tts".to_string(),
            tts_voice: "alloy".to_string(),
            poll_interval_ms: 500,
            minimum_score: 60,
            commentary_cooldown_ms: 8_000,
            data_dir,
            production: false,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if self.production && self.operator_token.as_deref().is_none_or(str::is_empty) {
            return Err(
                "MIR2_AI_LIVE_OPERATOR_TOKEN is required when MIR2_PRODUCTION=1".to_string(),
            );
        }
        for (name, endpoint) in [
            ("MIR2_AI_LIVE_TEXT_ENDPOINT", self.text_endpoint.as_deref()),
            ("MIR2_AI_LIVE_TTS_ENDPOINT", self.tts_endpoint.as_deref()),
        ] {
            let Some(endpoint) = endpoint else { continue };
            let url =
                Url::parse(endpoint).map_err(|error| format!("{name} is invalid: {error}"))?;
            if self.production && url.scheme() != "https" {
                return Err(format!("{name} must use HTTPS in production"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiLiveSegment {
    pub schema: String,
    pub segment_id: String,
    pub created_at_ms: u64,
    pub map_file_name: String,
    pub map_title: String,
    pub target: Option<String>,
    pub score: u8,
    pub reason: String,
    pub commentary: String,
    pub subtitle: String,
    pub source: AiLiveNarrativeSource,
    pub model: Option<String>,
    pub audio_url: Option<String>,
    pub frame_digest: String,
    pub frame_sequence: u64,
    pub event_kinds: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiLiveNarrativeSource {
    Model,
    DeterministicFallback,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiLiveMetrics {
    pub enabled: bool,
    pub mode: AiLiveMode,
    pub running: bool,
    pub processed_frames_total: u64,
    pub scored_highlights_total: u64,
    pub generated_segments_total: u64,
    pub model_success_total: u64,
    pub model_failure_total: u64,
    pub tts_success_total: u64,
    pub tts_failure_total: u64,
    pub distribution_success_total: u64,
    pub distribution_failure_total: u64,
    pub distribution_dead_letters_total: u64,
    pub queued_distribution_deliveries: usize,
    // Compatibility counters retained for existing dashboards during the v1 migration.
    pub discord_success_total: u64,
    pub discord_failure_total: u64,
    pub discord_dead_letters_total: u64,
    pub persisted_segments_total: u64,
    pub persistence_errors_total: u64,
    pub queued_discord_deliveries: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiLiveStatus {
    pub schema: &'static str,
    pub enabled: bool,
    pub mode: AiLiveMode,
    pub running: bool,
    pub latest_segment: Option<AiLiveSegment>,
    pub recent_segments: Vec<AiLiveSegment>,
    pub providers: AiLiveProviderStatus,
    pub metrics: AiLiveMetrics,
    pub distribution: AiDistributionStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiLiveProviderStatus {
    pub text_configured: bool,
    pub tts_configured: bool,
    pub discord_configured: bool,
    pub broadcast_url_configured: bool,
}

#[derive(Debug)]
struct AiLiveState {
    mode: AiLiveMode,
    latest_segment: Option<AiLiveSegment>,
    recent_segments: VecDeque<AiLiveSegment>,
    last_sequence_by_map: BTreeMap<String, u64>,
    last_segment_at_ms: u64,
    processed_frames_total: u64,
    scored_highlights_total: u64,
    generated_segments_total: u64,
    model_success_total: u64,
    model_failure_total: u64,
    tts_success_total: u64,
    tts_failure_total: u64,
    persisted_segments_total: u64,
    persistence_errors_total: u64,
}

impl AiLiveState {
    fn new(mode: AiLiveMode) -> Self {
        Self {
            mode,
            latest_segment: None,
            recent_segments: VecDeque::new(),
            last_sequence_by_map: BTreeMap::new(),
            last_segment_at_ms: 0,
            processed_frames_total: 0,
            scored_highlights_total: 0,
            generated_segments_total: 0,
            model_success_total: 0,
            model_failure_total: 0,
            tts_success_total: 0,
            tts_failure_total: 0,
            persisted_segments_total: 0,
            persistence_errors_total: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AiLiveHub {
    config: Arc<AiLiveConfig>,
    distribution: AiDistributionHub,
    state: Arc<Mutex<AiLiveState>>,
    client: Client,
    running: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
struct Highlight {
    score: u8,
    reason: String,
    target: Option<String>,
    event_kinds: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModelCommentary {
    commentary: String,
    subtitle: String,
    target: Option<String>,
}

impl AiLiveHub {
    pub fn from_env() -> Result<Self, String> {
        let config = AiLiveConfig::from_env()?;
        let distribution =
            AiDistributionConfig::from_env(config.data_dir.clone(), config.production)?;
        Self::new_with_distribution(config, distribution)
    }

    pub fn new(config: AiLiveConfig) -> Result<Self, String> {
        let distribution = AiDistributionConfig::disabled_for_tests(config.data_dir.clone());
        Self::new_with_distribution(config, distribution)
    }

    pub fn new_with_distribution(
        config: AiLiveConfig,
        distribution_config: AiDistributionConfig,
    ) -> Result<Self, String> {
        config.validate()?;
        if config.enabled {
            fs::create_dir_all(config.data_dir.join("audio")).map_err(|error| {
                format!(
                    "create AI live data directory {} failed: {error}",
                    config.data_dir.display()
                )
            })?;
        }
        let mode = config.mode;
        let state = AiLiveState::new(mode);
        Ok(Self {
            config: Arc::new(config),
            distribution: AiDistributionHub::new(distribution_config)?,
            state: Arc::new(Mutex::new(state)),
            client: Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(20))
                .build()
                .map_err(|error| format!("build AI live HTTP client failed: {error}"))?,
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn config(&self) -> &AiLiveConfig {
        &self.config
    }

    pub fn spawn(&self, spectator: SpectatorHub) {
        if !self.config.enabled || self.running.swap(true, Ordering::AcqRel) {
            return;
        }
        let hub = self.clone();
        tokio::spawn(async move {
            let mut tick =
                tokio::time::interval(Duration::from_millis(hub.config.poll_interval_ms));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                hub.poll_once(&spectator).await;
            }
        });
        let hub = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(250));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                if hub.mode() == AiLiveMode::Live {
                    hub.distribution.retry_once().await;
                }
            }
        });
    }

    async fn poll_once(&self, spectator: &SpectatorHub) {
        let matches = spectator.matches(true);
        for active_match in matches {
            let after_sequence = self
                .state
                .lock()
                .ok()
                .and_then(|state| {
                    state
                        .last_sequence_by_map
                        .get(&active_match.map_file_name)
                        .copied()
                })
                .unwrap_or(0);
            let Some(frame) =
                spectator.frame_at(&active_match.map_file_name, now_ms(), after_sequence)
            else {
                continue;
            };
            if let Ok(mut state) = self.state.lock() {
                state
                    .last_sequence_by_map
                    .insert(active_match.map_file_name.clone(), frame.sequence);
                state.processed_frames_total = state.processed_frames_total.saturating_add(1);
            }
            if let Err(error) = self.process_frame(frame).await {
                eprintln!("AI live frame skipped: {error}");
            }
        }
    }

    pub async fn process_frame(
        &self,
        frame: SpectatorFrame,
    ) -> Result<Option<AiLiveSegment>, String> {
        if !self.config.enabled {
            return Ok(None);
        }
        let mode = self.mode();
        if mode == AiLiveMode::Paused {
            return Ok(None);
        }
        let highlight = score_frame(&frame);
        if highlight.score < self.config.minimum_score {
            return Ok(None);
        }
        let now = now_ms();
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "AI live state mutex poisoned".to_string())?;
            state.scored_highlights_total = state.scored_highlights_total.saturating_add(1);
            if now.saturating_sub(state.last_segment_at_ms) < self.config.commentary_cooldown_ms {
                return Ok(None);
            }
            state.last_segment_at_ms = now;
        }

        let allowed_targets = frame
            .world
            .get("entities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entity| entity.get("name").and_then(Value::as_str))
            .map(str::to_string)
            .take(256)
            .collect::<Vec<_>>();
        let (mut commentary, source, model) = match self
            .model_commentary(&frame, &highlight, &allowed_targets)
            .await
        {
            Ok(commentary) => {
                self.with_state(|state| {
                    state.model_success_total = state.model_success_total.saturating_add(1)
                });
                (
                    commentary,
                    AiLiveNarrativeSource::Model,
                    Some(self.config.text_model.clone()),
                )
            }
            Err(error) => {
                if self.config.text_endpoint.is_some() {
                    eprintln!("AI live commentary fallback: {error}");
                    self.with_state(|state| {
                        state.model_failure_total = state.model_failure_total.saturating_add(1)
                    });
                }
                (
                    fallback_commentary(&frame, &highlight),
                    AiLiveNarrativeSource::DeterministicFallback,
                    None,
                )
            }
        };
        commentary.target = commentary
            .target
            .filter(|target| allowed_targets.iter().any(|allowed| allowed == target))
            .or(highlight.target.clone());
        commentary.commentary = bounded_text(&commentary.commentary, MAX_COMMENTARY_CHARS);
        commentary.subtitle = bounded_text(&commentary.subtitle, MAX_SUBTITLE_CHARS);
        let segment_id = segment_id(&frame, now);
        let mut segment = AiLiveSegment {
            schema: AI_LIVE_SCHEMA.to_string(),
            segment_id: segment_id.clone(),
            created_at_ms: now,
            map_file_name: frame.map_file_name.clone(),
            map_title: frame.map_title.clone(),
            target: commentary.target,
            score: highlight.score,
            reason: bounded_text(&highlight.reason, MAX_REASON_CHARS),
            commentary: commentary.commentary,
            subtitle: commentary.subtitle,
            source,
            model,
            audio_url: None,
            frame_digest: frame.digest.clone(),
            frame_sequence: frame.sequence,
            event_kinds: highlight.event_kinds,
        };

        if mode == AiLiveMode::Live {
            match self
                .synthesize_speech(&segment_id, &segment.commentary)
                .await
            {
                Ok(Some(audio_url)) => {
                    segment.audio_url = Some(audio_url);
                    self.with_state(|state| {
                        state.tts_success_total = state.tts_success_total.saturating_add(1)
                    });
                }
                Ok(None) => {}
                Err(error) => {
                    eprintln!("AI live TTS skipped: {error}");
                    self.with_state(|state| {
                        state.tts_failure_total = state.tts_failure_total.saturating_add(1)
                    });
                }
            }
        }

        let persisted = append_segment(&self.config.data_dir, &segment);
        self.with_state(|state| {
            state.generated_segments_total = state.generated_segments_total.saturating_add(1);
            if persisted.is_ok() {
                state.persisted_segments_total = state.persisted_segments_total.saturating_add(1);
            } else {
                state.persistence_errors_total = state.persistence_errors_total.saturating_add(1);
            }
            state.latest_segment = Some(segment.clone());
            state.recent_segments.push_front(segment.clone());
            state.recent_segments.truncate(MAX_RECENT_SEGMENTS);
        });
        if let Err(error) = persisted {
            eprintln!("AI live segment persistence failed: {error}");
        }

        if mode == AiLiveMode::Live {
            self.distribution.publish(&segment);
        }
        Ok(Some(segment))
    }

    async fn model_commentary(
        &self,
        frame: &SpectatorFrame,
        highlight: &Highlight,
        allowed_targets: &[String],
    ) -> Result<ModelCommentary, String> {
        let endpoint = self
            .config
            .text_endpoint
            .as_deref()
            .ok_or_else(|| "text model endpoint is not configured".to_string())?;
        let api_key = self
            .config
            .text_api_key
            .as_deref()
            .ok_or_else(|| "text model API key is not configured".to_string())?;
        let event_summary = frame
            .events
            .iter()
            .take(32)
            .map(safe_event_summary)
            .collect::<Vec<_>>();
        let response = self
            .client
            .post(endpoint)
            .bearer_auth(api_key)
            .json(&json!({
                "model": self.config.text_model,
                "temperature": 0.35,
                "response_format": {"type": "json_object"},
                "messages": [
                    {
                        "role": "system",
                        "content": "你是热血传奇赛事解说。只根据给定的脱敏事件说话，不推测身份、装备价值或未发生的事。返回严格 JSON：commentary、subtitle、target。commentary 最多 90 个汉字，subtitle 最多 32 个汉字，target 必须为允许目标之一或 null。"
                    },
                    {
                        "role": "user",
                        "content": serde_json::to_string(&json!({
                            "map": frame.map_title,
                            "score": highlight.score,
                            "reason": highlight.reason,
                            "allowedTargets": allowed_targets,
                            "events": event_summary
                        })).unwrap_or_default()
                    }
                ]
            }))
            .send()
            .await
            .map_err(|error| format!("text model request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("text model returned HTTP {}", response.status()));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("read text model response failed: {error}"))?;
        if bytes.len() > MAX_MODEL_RESPONSE_BYTES {
            return Err("text model response exceeded size limit".to_string());
        }
        let envelope: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode text model response failed: {error}"))?;
        let content = envelope
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| "text model response has no message content".to_string())?;
        let commentary: ModelCommentary = serde_json::from_str(content)
            .map_err(|error| format!("decode strict commentary JSON failed: {error}"))?;
        validate_commentary(&commentary, allowed_targets)?;
        Ok(commentary)
    }

    async fn synthesize_speech(
        &self,
        segment_id: &str,
        commentary: &str,
    ) -> Result<Option<String>, String> {
        let Some(endpoint) = self.config.tts_endpoint.as_deref() else {
            return Ok(None);
        };
        let Some(api_key) = self.config.tts_api_key.as_deref() else {
            return Ok(None);
        };
        let response = self
            .client
            .post(endpoint)
            .bearer_auth(api_key)
            .json(&json!({
                "model": self.config.tts_model,
                "voice": self.config.tts_voice,
                "input": commentary,
                "response_format": "mp3"
            }))
            .send()
            .await
            .map_err(|error| format!("TTS request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("TTS returned HTTP {}", response.status()));
        }
        if !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value.starts_with("audio/") || value.starts_with("application/octet-stream")
            })
        {
            return Err("TTS returned an unexpected content type".to_string());
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("read TTS response failed: {error}"))?;
        if bytes.is_empty() || bytes.len() > 8 * 1024 * 1024 {
            return Err("TTS response size is invalid".to_string());
        }
        let path = self
            .config
            .data_dir
            .join("audio")
            .join(format!("{segment_id}.mp3"));
        fs::write(&path, bytes)
            .map_err(|error| format!("write TTS clip {} failed: {error}", path.display()))?;
        Ok(Some(format!("/ai-live/audio/{segment_id}.mp3")))
    }

    pub fn status(&self) -> AiLiveStatus {
        let state = self.state.lock().expect("AI live state mutex poisoned");
        let distribution = self.distribution.status();
        AiLiveStatus {
            schema: AI_LIVE_SCHEMA,
            enabled: self.config.enabled,
            mode: state.mode,
            running: self.running.load(Ordering::Acquire),
            latest_segment: state.latest_segment.clone(),
            recent_segments: state.recent_segments.iter().cloned().collect(),
            providers: AiLiveProviderStatus {
                text_configured: self.config.text_endpoint.is_some()
                    && self.config.text_api_key.is_some(),
                tts_configured: self.config.tts_endpoint.is_some()
                    && self.config.tts_api_key.is_some(),
                discord_configured: distribution.channels.iter().any(|channel| {
                    channel.channel == AiDistributionChannel::DiscordWebhook && channel.configured
                }),
                broadcast_url_configured: distribution.channels.iter().any(|channel| {
                    channel.channel == AiDistributionChannel::WebBroadcast && channel.configured
                }),
            },
            metrics: metrics_from_state(
                &self.config,
                &state,
                self.running.load(Ordering::Acquire),
                self.distribution.metrics(),
                self.distribution
                    .channel_status(AiDistributionChannel::DiscordWebhook),
            ),
            distribution,
        }
    }

    pub fn metrics(&self) -> AiLiveMetrics {
        let state = self.state.lock().expect("AI live state mutex poisoned");
        metrics_from_state(
            &self.config,
            &state,
            self.running.load(Ordering::Acquire),
            self.distribution.metrics(),
            self.distribution
                .channel_status(AiDistributionChannel::DiscordWebhook),
        )
    }

    pub fn mode(&self) -> AiLiveMode {
        self.state
            .lock()
            .map(|state| state.mode)
            .unwrap_or(AiLiveMode::Paused)
    }

    pub fn set_mode(&self, token: Option<&str>, mode: AiLiveMode) -> Result<AiLiveStatus, String> {
        if !self.config.enabled {
            return Err("AI live service is disabled".to_string());
        }
        self.authorize(token)?;
        self.with_state(|state| state.mode = mode);
        Ok(self.status())
    }

    pub fn distribution_status(&self) -> AiDistributionStatus {
        self.distribution.status()
    }

    pub async fn retry_distribution_once(&self) {
        self.distribution.retry_once().await;
    }

    pub fn set_distribution_channel(
        &self,
        token: Option<&str>,
        channel: AiDistributionChannel,
        enabled: bool,
    ) -> Result<AiDistributionStatus, String> {
        self.authorize(token)?;
        self.distribution.set_channel_enabled(channel, enabled)
    }

    pub fn retry_distribution_channel(
        &self,
        token: Option<&str>,
        channel: AiDistributionChannel,
    ) -> Result<AiDistributionStatus, String> {
        self.authorize(token)?;
        self.distribution.retry_channel_now(channel)
    }

    pub fn audio_path(&self, clip: &str) -> Result<PathBuf, String> {
        let id = clip.strip_suffix(".mp3").unwrap_or(clip);
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err("invalid AI live audio clip id".to_string());
        }
        Ok(self.config.data_dir.join("audio").join(format!("{id}.mp3")))
    }

    fn with_state(&self, apply: impl FnOnce(&mut AiLiveState)) {
        if let Ok(mut state) = self.state.lock() {
            apply(&mut state);
        }
    }

    fn authorize(&self, token: Option<&str>) -> Result<(), String> {
        let expected = self
            .config
            .operator_token
            .as_deref()
            .ok_or_else(|| "AI live operator token is not configured".to_string())?;
        let actual = token.ok_or_else(|| "AI live operator token is required".to_string())?;
        if !constant_time_eq(expected.as_bytes(), actual.as_bytes()) {
            return Err("invalid AI live operator token".to_string());
        }
        Ok(())
    }
}

fn metrics_from_state(
    config: &AiLiveConfig,
    state: &AiLiveState,
    running: bool,
    distribution: AiDistributionMetrics,
    discord: crate::ai_distribution::AiChannelStatus,
) -> AiLiveMetrics {
    AiLiveMetrics {
        enabled: config.enabled,
        mode: state.mode,
        running,
        processed_frames_total: state.processed_frames_total,
        scored_highlights_total: state.scored_highlights_total,
        generated_segments_total: state.generated_segments_total,
        model_success_total: state.model_success_total,
        model_failure_total: state.model_failure_total,
        tts_success_total: state.tts_success_total,
        tts_failure_total: state.tts_failure_total,
        distribution_success_total: distribution.delivered_total,
        distribution_failure_total: distribution.failure_total,
        distribution_dead_letters_total: distribution.dead_letters_total,
        queued_distribution_deliveries: distribution.queued_deliveries,
        discord_success_total: discord.delivered_total,
        discord_failure_total: discord.failure_total,
        discord_dead_letters_total: discord.dead_letters_total,
        persisted_segments_total: state.persisted_segments_total,
        persistence_errors_total: state.persistence_errors_total,
        queued_discord_deliveries: discord.queued,
    }
}

fn score_frame(frame: &SpectatorFrame) -> Highlight {
    let mut score = 0u8;
    let mut reason = "战场态势变化".to_string();
    let mut target = None;
    let mut event_kinds = Vec::new();
    for event in &frame.events {
        if !event_kinds.contains(&event.kind) {
            event_kinds.push(event.kind.clone());
        }
        let candidate = match event.kind.as_str() {
            "death" => (100, "玩家倒地"),
            "dropSpawn" => (88, "稀有战利品出现"),
            "revive" => (78, "玩家重返战场"),
            "health" => {
                let from = event.payload.get("from").and_then(Value::as_i64);
                let to = event.payload.get("to").and_then(Value::as_i64);
                let damage = from.zip(to).map_or(0, |(from, to)| from.saturating_sub(to));
                if damage >= 100 {
                    (72, "高额伤害")
                } else if damage > 0 {
                    (55, "交战伤害")
                } else {
                    (20, "生命值变化")
                }
            }
            "spawn"
                if event.payload.get("entityKind").and_then(Value::as_str) == Some("monster") =>
            {
                (35, "怪物进入战场")
            }
            _ => (0, ""),
        };
        if candidate.0 > score {
            score = candidate.0;
            reason = candidate.1.to_string();
            target = event.name.clone();
        }
    }
    let active_events = frame
        .events
        .iter()
        .filter(|event| !matches!(event.kind.as_str(), "move" | "spawn"))
        .count();
    if active_events >= 4 {
        score = score.saturating_add(8).min(100);
        reason.push_str("，多人交战升温");
    }
    Highlight {
        score,
        reason,
        target,
        event_kinds,
    }
}

fn fallback_commentary(frame: &SpectatorFrame, highlight: &Highlight) -> ModelCommentary {
    let subject = highlight.target.as_deref().unwrap_or("战场");
    ModelCommentary {
        commentary: format!(
            "{}：{}！{}，镜头已经锁定现场。",
            frame.map_title, subject, highlight.reason
        ),
        subtitle: format!("{} · {}", frame.map_title, highlight.reason),
        target: highlight.target.clone(),
    }
}

fn validate_commentary(
    commentary: &ModelCommentary,
    allowed_targets: &[String],
) -> Result<(), String> {
    if commentary.commentary.trim().is_empty()
        || commentary.commentary.chars().count() > MAX_COMMENTARY_CHARS
    {
        return Err("model commentary is empty or too long".to_string());
    }
    if commentary.subtitle.trim().is_empty()
        || commentary.subtitle.chars().count() > MAX_SUBTITLE_CHARS
    {
        return Err("model subtitle is empty or too long".to_string());
    }
    if commentary
        .target
        .as_ref()
        .is_some_and(|target| !allowed_targets.iter().any(|allowed| allowed == target))
    {
        return Err("model selected a target outside the sanitized allowlist".to_string());
    }
    Ok(())
}

fn safe_event_summary(event: &SpectatorEvent) -> Value {
    json!({
        "kind": event.kind,
        "name": event.name,
        "atMs": event.at_ms,
        "from": event.payload.get("from"),
        "to": event.payload.get("to"),
        "entityKind": event.payload.get("entityKind"),
        "quantity": event.payload.get("quantity")
    })
}

fn append_segment(data_dir: &Path, segment: &AiLiveSegment) -> Result<(), String> {
    fs::create_dir_all(data_dir)
        .map_err(|error| format!("create AI live data dir failed: {error}"))?;
    let path = data_dir.join("segments.jsonl");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open {} failed: {error}", path.display()))?;
    serde_json::to_writer(&mut file, segment)
        .map_err(|error| format!("encode AI live segment failed: {error}"))?;
    file.write_all(b"\n")
        .and_then(|()| file.flush())
        .map_err(|error| format!("flush {} failed: {error}", path.display()))
}

fn segment_id(frame: &SpectatorFrame, now_ms: u64) -> String {
    let digest = Sha256::digest(
        format!(
            "{}:{}:{}:{}",
            frame.map_file_name, frame.sequence, frame.digest, now_ms
        )
        .as_bytes(),
    );
    format!(
        "live-{}-{}",
        safe_component(&frame.map_file_name),
        digest
            .iter()
            .take(8)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    value.trim().chars().take(max_chars).collect()
}

fn safe_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn constant_time_eq(expected: &[u8], actual: &[u8]) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    expected
        .iter()
        .zip(actual)
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn bool_env(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn bounded_u64_env(name: &str, default: u64, min: u64, max: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
        .clamp(min, max)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(events: Vec<SpectatorEvent>) -> SpectatorFrame {
        SpectatorFrame {
            schema: "test".to_string(),
            recording_id: "0-1".to_string(),
            sequence: 7,
            captured_at_ms: 1_000,
            map_file_name: "0".to_string(),
            map_title: "比奇省".to_string(),
            digest: "abc".to_string(),
            events,
            world: json!({
                "entities": [{
                    "objectId": 1,
                    "kind": "player",
                    "name": "TestHero",
                    "hp": 10,
                    "maxHp": 100,
                    "x": 10,
                    "y": 20
                }]
            }),
        }
    }

    #[test]
    fn death_scores_as_maximum_highlight() {
        let highlight = score_frame(&frame(vec![SpectatorEvent {
            kind: "death".to_string(),
            at_ms: 1_000,
            object_id: Some(1),
            name: Some("TestHero".to_string()),
            payload: json!({"dead": true}),
        }]));
        assert_eq!(highlight.score, 100);
        assert_eq!(highlight.target.as_deref(), Some("TestHero"));
    }

    #[test]
    fn strict_model_commentary_rejects_unknown_fields_and_invalid_target() {
        let decoded = serde_json::from_str::<ModelCommentary>(
            r#"{"commentary":"精彩","subtitle":"高光","target":"Unknown","extra":true}"#,
        );
        assert!(decoded.is_err());
        let commentary = ModelCommentary {
            commentary: "精彩".to_string(),
            subtitle: "高光".to_string(),
            target: Some("Unknown".to_string()),
        };
        assert!(validate_commentary(&commentary, &["TestHero".to_string()]).is_err());
    }

    #[test]
    fn audio_path_rejects_traversal() {
        let hub = AiLiveHub::new(AiLiveConfig::disabled_for_tests(
            std::env::temp_dir().join("mir2-ai-live-path-test"),
        ))
        .expect("test hub");
        assert!(hub.audio_path("../../secret").is_err());
        assert!(hub.audio_path("live-0-deadbeef.mp3").is_ok());
    }

    #[test]
    fn production_rejects_non_https_provider() {
        let mut config =
            AiLiveConfig::disabled_for_tests(std::env::temp_dir().join("mir2-ai-live-config-test"));
        config.enabled = true;
        config.production = true;
        config.operator_token = Some("secret".to_string());
        config.text_endpoint = Some("http://localhost/model".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn control_mode_requires_exact_operator_token() {
        let mut config =
            AiLiveConfig::disabled_for_tests(std::env::temp_dir().join("mir2-ai-live-mode-test"));
        config.enabled = true;
        config.operator_token = Some("correct-token".to_string());
        let hub = AiLiveHub::new(config).expect("AI live hub");
        assert!(hub.set_mode(Some("wrong-token"), AiLiveMode::Live).is_err());
        assert_eq!(
            hub.set_mode(Some("correct-token"), AiLiveMode::Live)
                .expect("authorized mode switch")
                .mode,
            AiLiveMode::Live
        );
    }

    #[tokio::test]
    async fn live_pipeline_calls_model_tts_discord_and_persists_evidence() {
        use axum::http::{header, StatusCode};
        use axum::routing::post;
        use axum::{Json, Router};

        let app = Router::new()
            .route(
                "/chat",
                post(|| async {
                    Json(json!({
                        "choices": [{
                            "message": {
                                "content": "{\"commentary\":\"TestHero 在比奇省倒下，战局瞬间改变！\",\"subtitle\":\"比奇省决胜时刻\",\"target\":\"TestHero\"}"
                            }
                        }]
                    }))
                }),
            )
            .route(
                "/tts",
                post(|| async {
                    (
                        [(header::CONTENT_TYPE, "audio/mpeg")],
                        b"ID3-test-audio".to_vec(),
                    )
                }),
            )
            .route("/discord", post(|| async { StatusCode::NO_CONTENT }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock provider");
        let address = listener.local_addr().expect("mock provider address");
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve mock provider");
        });
        let data_dir = std::env::temp_dir().join(format!(
            "mir2-ai-live-pipeline-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let mut config = AiLiveConfig::disabled_for_tests(data_dir.clone());
        config.enabled = true;
        config.mode = AiLiveMode::Live;
        config.operator_token = Some("test-token".to_string());
        config.text_endpoint = Some(format!("http://{address}/chat"));
        config.text_api_key = Some("text-secret".to_string());
        config.tts_endpoint = Some(format!("http://{address}/tts"));
        config.tts_api_key = Some("tts-secret".to_string());
        config.minimum_score = 60;
        config.commentary_cooldown_ms = 500;
        let mut distribution = AiDistributionConfig::disabled_for_tests(data_dir.clone());
        distribution.discord_webhook = Some(format!("http://{address}/discord"));
        distribution.discord_minimum_score = 90;
        let hub = AiLiveHub::new_with_distribution(config, distribution).expect("AI live hub");
        let segment = hub
            .process_frame(frame(vec![SpectatorEvent {
                kind: "death".to_string(),
                at_ms: 1_000,
                object_id: Some(1),
                name: Some("TestHero".to_string()),
                payload: json!({"dead": true}),
            }]))
            .await
            .expect("pipeline")
            .expect("highlight segment");
        hub.retry_distribution_once().await;

        assert_eq!(segment.source, AiLiveNarrativeSource::Model);
        assert_eq!(segment.target.as_deref(), Some("TestHero"));
        assert!(segment.audio_url.is_some());
        assert!(data_dir.join("segments.jsonl").is_file());
        assert!(data_dir
            .join("audio")
            .join(format!("{}.mp3", segment.segment_id))
            .is_file());
        let metrics = hub.metrics();
        assert_eq!(metrics.model_success_total, 1);
        assert_eq!(metrics.tts_success_total, 1);
        assert_eq!(metrics.discord_success_total, 1);
        assert_eq!(metrics.persisted_segments_total, 1);

        server.abort();
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn discord_retry_queue_survives_hub_restart() {
        let data_dir = std::env::temp_dir().join(format!(
            "mir2-ai-live-queue-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let mut config = AiLiveConfig::disabled_for_tests(data_dir.clone());
        config.enabled = true;
        config.mode = AiLiveMode::Live;
        config.operator_token = Some("test-token".to_string());
        config.minimum_score = 60;
        config.commentary_cooldown_ms = 500;
        let mut distribution = AiDistributionConfig::disabled_for_tests(data_dir.clone());
        distribution.discord_webhook = Some("http://127.0.0.1:1/discord".to_string());
        distribution.discord_minimum_score = 90;
        let hub = AiLiveHub::new_with_distribution(config.clone(), distribution.clone())
            .expect("AI live hub");
        hub.process_frame(frame(vec![SpectatorEvent {
            kind: "death".to_string(),
            at_ms: 1_000,
            object_id: Some(1),
            name: Some("TestHero".to_string()),
            payload: json!({"dead": true}),
        }]))
        .await
        .expect("pipeline")
        .expect("highlight segment");
        assert_eq!(hub.metrics().queued_discord_deliveries, 1);
        assert!(data_dir.join("distribution-queue.json").is_file());

        let restarted =
            AiLiveHub::new_with_distribution(config, distribution).expect("restarted AI live hub");
        assert_eq!(restarted.metrics().queued_discord_deliveries, 1);

        let _ = fs::remove_dir_all(data_dir);
    }
}
