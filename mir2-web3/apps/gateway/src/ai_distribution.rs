//! Channel-neutral distribution fabric for AI-produced game content.
//!
//! The AI program layer publishes one canonical [`AiContentPackage`]. This module owns routing,
//! channel adapters, durable retry, idempotency and delivery status. No adapter receives a player
//! session or an authoritative Zone command handle.

use crate::ai_live::{AiLiveNarrativeSource, AiLiveSegment};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DISTRIBUTION_SCHEMA: &str = "obelisk.mir2.ai-distribution.v1";
const CONTENT_SCHEMA: &str = "obelisk.mir2.ai-content.v1";
const MAX_PENDING_DELIVERIES: usize = 256;
const MAX_RECENT_RECEIPTS: usize = 40;
const MAX_ATTEMPTS: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiDistributionChannel {
    GameOverlay,
    WebBroadcast,
    RtmpBroadcast,
    DiscordWebhook,
    DiscordGoLive,
    ClipExport,
}

impl AiDistributionChannel {
    pub const ALL: [Self; 6] = [
        Self::GameOverlay,
        Self::WebBroadcast,
        Self::RtmpBroadcast,
        Self::DiscordWebhook,
        Self::DiscordGoLive,
        Self::ClipExport,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::GameOverlay => "gameOverlay",
            Self::WebBroadcast => "webBroadcast",
            Self::RtmpBroadcast => "rtmpBroadcast",
            Self::DiscordWebhook => "discordWebhook",
            Self::DiscordGoLive => "discordGoLive",
            Self::ClipExport => "clipExport",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gameoverlay" | "game" => Some(Self::GameOverlay),
            "webbroadcast" | "web" | "hls" => Some(Self::WebBroadcast),
            "rtmpbroadcast" | "rtmp" | "rtmps" => Some(Self::RtmpBroadcast),
            "discordwebhook" | "discord" => Some(Self::DiscordWebhook),
            "discordgolive" | "golive" => Some(Self::DiscordGoLive),
            "clipexport" | "clip" | "shortvideo" => Some(Self::ClipExport),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::GameOverlay => "游戏内节目层",
            Self::WebBroadcast => "Web / HLS",
            Self::RtmpBroadcast => "RTMP / RTMPS",
            Self::DiscordWebhook => "Discord 高光",
            Self::DiscordGoLive => "Discord Go Live Relay",
            Self::ClipExport => "短视频导出",
        }
    }

    fn delivery_mode(self) -> &'static str {
        match self {
            Self::GameOverlay => "push",
            Self::WebBroadcast => "pull",
            Self::RtmpBroadcast => "relay",
            Self::DiscordWebhook => "push",
            Self::DiscordGoLive => "push",
            Self::ClipExport => "push",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AiDistributionConfig {
    pub data_dir: PathBuf,
    pub production: bool,
    pub discord_webhook: Option<String>,
    pub public_spectator_url: Option<String>,
    pub rtmp_enabled: bool,
    pub game_overlay_endpoint: Option<String>,
    pub discord_go_live_endpoint: Option<String>,
    pub clip_export_endpoint: Option<String>,
    pub adapter_token: Option<String>,
    pub discord_minimum_score: u8,
    pub discord_go_live_minimum_score: u8,
    pub clip_minimum_score: u8,
}

impl AiDistributionConfig {
    pub fn from_env(data_dir: PathBuf, production: bool) -> Result<Self, String> {
        let config = Self {
            data_dir,
            production,
            discord_webhook: optional_env("MIR2_AI_LIVE_DISCORD_WEBHOOK"),
            public_spectator_url: optional_env("MIR2_AI_LIVE_PUBLIC_URL"),
            rtmp_enabled: bool_env("MIR2_AI_DISTRIBUTION_RTMP_ENABLED", false),
            game_overlay_endpoint: optional_env("MIR2_AI_DISTRIBUTION_GAME_ENDPOINT"),
            discord_go_live_endpoint: optional_env("MIR2_AI_DISTRIBUTION_DISCORD_GO_LIVE_ENDPOINT"),
            clip_export_endpoint: optional_env("MIR2_AI_DISTRIBUTION_CLIP_ENDPOINT"),
            adapter_token: optional_env("MIR2_AI_DISTRIBUTION_ADAPTER_TOKEN"),
            discord_minimum_score: bounded_score_env("MIR2_AI_LIVE_DISCORD_MIN_SCORE", 90),
            discord_go_live_minimum_score: bounded_score_env(
                "MIR2_AI_DISTRIBUTION_DISCORD_GO_LIVE_MIN_SCORE",
                90,
            ),
            clip_minimum_score: bounded_score_env("MIR2_AI_DISTRIBUTION_CLIP_MIN_SCORE", 92),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn disabled_for_tests(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            production: false,
            discord_webhook: None,
            public_spectator_url: None,
            rtmp_enabled: false,
            game_overlay_endpoint: None,
            discord_go_live_endpoint: None,
            clip_export_endpoint: None,
            adapter_token: None,
            discord_minimum_score: 90,
            discord_go_live_minimum_score: 90,
            clip_minimum_score: 92,
        }
    }

    fn validate(&self) -> Result<(), String> {
        for (name, endpoint) in [
            (
                "MIR2_AI_LIVE_DISCORD_WEBHOOK",
                self.discord_webhook.as_deref(),
            ),
            (
                "MIR2_AI_LIVE_PUBLIC_URL",
                self.public_spectator_url.as_deref(),
            ),
            (
                "MIR2_AI_DISTRIBUTION_DISCORD_GO_LIVE_ENDPOINT",
                self.discord_go_live_endpoint.as_deref(),
            ),
            (
                "MIR2_AI_DISTRIBUTION_GAME_ENDPOINT",
                self.game_overlay_endpoint.as_deref(),
            ),
            (
                "MIR2_AI_DISTRIBUTION_CLIP_ENDPOINT",
                self.clip_export_endpoint.as_deref(),
            ),
        ] {
            let Some(endpoint) = endpoint else { continue };
            let url =
                Url::parse(endpoint).map_err(|error| format!("{name} is invalid: {error}"))?;
            if self.production && url.scheme() != "https" {
                return Err(format!("{name} must use HTTPS in production"));
            }
        }
        if self.production {
            if let Some(webhook) = self.discord_webhook.as_deref() {
                let host = Url::parse(webhook)
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_string));
                if !matches!(host.as_deref(), Some("discord.com" | "discordapp.com")) {
                    return Err(
                        "MIR2_AI_LIVE_DISCORD_WEBHOOK must use an official Discord host"
                            .to_string(),
                    );
                }
            }
            if (self.game_overlay_endpoint.is_some()
                || self.discord_go_live_endpoint.is_some()
                || self.clip_export_endpoint.is_some())
                && self.adapter_token.as_deref().is_none_or(str::is_empty)
            {
                return Err(
                    "MIR2_AI_DISTRIBUTION_ADAPTER_TOKEN is required for production push adapters"
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    fn configured(&self, channel: AiDistributionChannel) -> bool {
        match channel {
            AiDistributionChannel::GameOverlay => self.game_overlay_endpoint.is_some(),
            AiDistributionChannel::WebBroadcast => self.public_spectator_url.is_some(),
            AiDistributionChannel::RtmpBroadcast => self.rtmp_enabled,
            AiDistributionChannel::DiscordWebhook => self.discord_webhook.is_some(),
            AiDistributionChannel::DiscordGoLive => self.discord_go_live_endpoint.is_some(),
            AiDistributionChannel::ClipExport => self.clip_export_endpoint.is_some(),
        }
    }

    fn endpoint(&self, channel: AiDistributionChannel) -> Option<&str> {
        match channel {
            AiDistributionChannel::GameOverlay => self.game_overlay_endpoint.as_deref(),
            AiDistributionChannel::DiscordWebhook => self.discord_webhook.as_deref(),
            AiDistributionChannel::DiscordGoLive => self.discord_go_live_endpoint.as_deref(),
            AiDistributionChannel::ClipExport => self.clip_export_endpoint.as_deref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiContentKind {
    LiveHighlight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContentAssets {
    pub audio_url: Option<String>,
    pub watch_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContentContext {
    pub map_file_name: String,
    pub map_title: String,
    pub target: Option<String>,
    pub frame_digest: String,
    pub frame_sequence: u64,
    pub event_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContentPackage {
    pub schema: String,
    pub content_id: String,
    pub kind: AiContentKind,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub locale: String,
    pub title: String,
    pub body: String,
    pub subtitle: String,
    pub score: u8,
    pub reason: String,
    pub narrative_source: AiLiveNarrativeSource,
    pub model: Option<String>,
    pub assets: AiContentAssets,
    pub context: AiContentContext,
}

impl AiContentPackage {
    fn from_segment(segment: &AiLiveSegment, public_url: Option<&str>) -> Self {
        let watch_url = public_url.map(|base| {
            format!(
                "{}?spectate=1&aiLive=1&spectateMap={}",
                base.trim_end_matches('/'),
                percent_encode_query(&segment.map_file_name)
            )
        });
        Self {
            schema: CONTENT_SCHEMA.to_string(),
            content_id: segment.segment_id.clone(),
            kind: AiContentKind::LiveHighlight,
            created_at_ms: segment.created_at_ms,
            expires_at_ms: segment.created_at_ms.saturating_add(24 * 60 * 60 * 1_000),
            locale: "zh-CN".to_string(),
            title: segment.subtitle.clone(),
            body: segment.commentary.clone(),
            subtitle: segment.subtitle.clone(),
            score: segment.score,
            reason: segment.reason.clone(),
            narrative_source: segment.source,
            model: segment.model.clone(),
            assets: AiContentAssets {
                audio_url: segment.audio_url.clone(),
                watch_url,
            },
            context: AiContentContext {
                map_file_name: segment.map_file_name.clone(),
                map_title: segment.map_title.clone(),
                target: segment.target.clone(),
                frame_digest: segment.frame_digest.clone(),
                frame_sequence: segment.frame_sequence,
                event_kinds: segment.event_kinds.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DistributionJob {
    job_id: String,
    idempotency_key: String,
    channel: AiDistributionChannel,
    package: AiContentPackage,
    attempts: u8,
    created_at_ms: u64,
    next_attempt_at_ms: u64,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDeliveryReceipt {
    pub job_id: String,
    pub content_id: String,
    pub channel: AiDistributionChannel,
    pub delivered_at_ms: u64,
    pub attempts: u8,
}

#[derive(Debug, Clone, Default)]
struct ChannelCounters {
    delivered: u64,
    failed: u64,
    dead_letters: u64,
    last_success_at_ms: Option<u64>,
    last_failure_at_ms: Option<u64>,
    last_error: Option<String>,
}

#[derive(Debug)]
struct AiDistributionState {
    queue: VecDeque<DistributionJob>,
    enabled: BTreeMap<AiDistributionChannel, bool>,
    counters: BTreeMap<AiDistributionChannel, ChannelCounters>,
    recent_receipts: VecDeque<AiDeliveryReceipt>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChannelStatus {
    pub channel: AiDistributionChannel,
    pub label: &'static str,
    pub delivery_mode: &'static str,
    pub configured: bool,
    pub enabled: bool,
    pub state: &'static str,
    pub queued: usize,
    pub delivered_total: u64,
    pub failure_total: u64,
    pub dead_letters_total: u64,
    pub last_success_at_ms: Option<u64>,
    pub last_failure_at_ms: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDistributionMetrics {
    pub delivered_total: u64,
    pub failure_total: u64,
    pub dead_letters_total: u64,
    pub queued_deliveries: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDistributionStatus {
    pub schema: &'static str,
    pub channels: Vec<AiChannelStatus>,
    pub recent_receipts: Vec<AiDeliveryReceipt>,
    pub metrics: AiDistributionMetrics,
}

#[derive(Debug, Clone)]
pub struct AiDistributionHub {
    config: Arc<AiDistributionConfig>,
    state: Arc<Mutex<AiDistributionState>>,
    client: Client,
}

impl AiDistributionHub {
    pub fn new(config: AiDistributionConfig) -> Result<Self, String> {
        config.validate()?;
        fs::create_dir_all(&config.data_dir).map_err(|error| {
            format!(
                "create AI distribution directory {} failed: {error}",
                config.data_dir.display()
            )
        })?;
        let queue = load_queue(&config)?;
        let persisted_enabled = load_channel_preferences(&config.data_dir)?;
        let enabled = AiDistributionChannel::ALL
            .into_iter()
            .map(|channel| {
                (
                    channel,
                    persisted_enabled
                        .get(&channel)
                        .copied()
                        .unwrap_or_else(|| config.configured(channel)),
                )
            })
            .collect();
        Ok(Self {
            config: Arc::new(config),
            state: Arc::new(Mutex::new(AiDistributionState {
                queue,
                enabled,
                counters: BTreeMap::new(),
                recent_receipts: VecDeque::new(),
            })),
            client: Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(20))
                .build()
                .map_err(|error| format!("build AI distribution HTTP client failed: {error}"))?,
        })
    }

    pub fn publish(&self, segment: &AiLiveSegment) -> AiContentPackage {
        let package =
            AiContentPackage::from_segment(segment, self.config.public_spectator_url.as_deref());
        if let Err(error) = append_content_package(&self.config.data_dir, &package) {
            eprintln!("persist AI content package failed: {error}");
        }
        for channel in AiDistributionChannel::ALL {
            if !self.should_route(channel, package.score) {
                continue;
            }
            let job = DistributionJob {
                job_id: format!("{}:{}", package.content_id, channel.as_str()),
                idempotency_key: format!("{}:{}", package.content_id, channel.as_str()),
                channel,
                package: package.clone(),
                attempts: 0,
                created_at_ms: now_ms(),
                next_attempt_at_ms: now_ms(),
                last_error: None,
            };
            if matches!(
                channel,
                AiDistributionChannel::GameOverlay
                    | AiDistributionChannel::DiscordWebhook
                    | AiDistributionChannel::DiscordGoLive
                    | AiDistributionChannel::ClipExport
            ) {
                self.enqueue(job);
            } else {
                self.record_success(&job);
            }
        }
        package
    }

    pub async fn retry_once(&self) {
        let pending = self.with_state_value(|state| {
            state
                .queue
                .iter()
                .find(|job| {
                    job.next_attempt_at_ms <= now_ms()
                        && state.enabled.get(&job.channel).copied().unwrap_or(false)
                })
                .cloned()
        });
        let Some(mut job) = pending else { return };
        match self.deliver(&job).await {
            Ok(()) => {
                self.with_state(|state| {
                    if let Some(index) = state
                        .queue
                        .iter()
                        .position(|current| current.job_id == job.job_id)
                    {
                        state.queue.remove(index);
                    }
                });
                self.record_success(&job);
                self.persist_queue();
            }
            Err(error) => {
                job.attempts = job.attempts.saturating_add(1);
                self.record_failure(job.channel, &error);
                if job.attempts > MAX_ATTEMPTS {
                    self.with_state(|state| {
                        if let Some(index) = state
                            .queue
                            .iter()
                            .position(|current| current.job_id == job.job_id)
                        {
                            state.queue.remove(index);
                        }
                        state.counters.entry(job.channel).or_default().dead_letters = state
                            .counters
                            .get(&job.channel)
                            .map(|counter| counter.dead_letters)
                            .unwrap_or(0)
                            .saturating_add(1);
                    });
                    if let Err(persist_error) =
                        append_dead_letter(&self.config.data_dir, &job, &error)
                    {
                        eprintln!("persist AI distribution dead letter failed: {persist_error}");
                    }
                } else {
                    let backoff = 2_000u64.saturating_mul(1u64 << job.attempts.min(6));
                    job.next_attempt_at_ms = now_ms().saturating_add(backoff);
                    job.last_error = Some(bounded_text(&error, 500));
                    self.with_state(|state| {
                        if let Some(current) = state
                            .queue
                            .iter_mut()
                            .find(|current| current.job_id == job.job_id)
                        {
                            *current = job.clone();
                        }
                    });
                }
                self.persist_queue();
            }
        }
    }

    pub fn status(&self) -> AiDistributionStatus {
        let state = self
            .state
            .lock()
            .expect("AI distribution state mutex poisoned");
        let channels = AiDistributionChannel::ALL
            .into_iter()
            .map(|channel| channel_status(&self.config, &state, channel))
            .collect::<Vec<_>>();
        AiDistributionStatus {
            schema: DISTRIBUTION_SCHEMA,
            channels,
            recent_receipts: state.recent_receipts.iter().cloned().collect(),
            metrics: metrics_from_state(&state),
        }
    }

    pub fn metrics(&self) -> AiDistributionMetrics {
        let state = self
            .state
            .lock()
            .expect("AI distribution state mutex poisoned");
        metrics_from_state(&state)
    }

    pub fn channel_status(&self, channel: AiDistributionChannel) -> AiChannelStatus {
        let state = self
            .state
            .lock()
            .expect("AI distribution state mutex poisoned");
        channel_status(&self.config, &state, channel)
    }

    pub fn set_channel_enabled(
        &self,
        channel: AiDistributionChannel,
        enabled: bool,
    ) -> Result<AiDistributionStatus, String> {
        if enabled && !self.config.configured(channel) {
            return Err(format!("{} adapter is not configured", channel.as_str()));
        }
        self.with_state(|state| {
            state.enabled.insert(channel, enabled);
        });
        self.persist_channel_preferences()?;
        Ok(self.status())
    }

    pub fn retry_channel_now(
        &self,
        channel: AiDistributionChannel,
    ) -> Result<AiDistributionStatus, String> {
        let changed = self.with_state_value(|state| {
            let mut changed = false;
            for job in state.queue.iter_mut().filter(|job| job.channel == channel) {
                job.next_attempt_at_ms = 0;
                changed = true;
            }
            changed
        });
        if !changed {
            return Err(format!("{} has no queued delivery", channel.as_str()));
        }
        self.persist_queue();
        Ok(self.status())
    }

    fn should_route(&self, channel: AiDistributionChannel, score: u8) -> bool {
        let enabled =
            self.with_state_value(|state| state.enabled.get(&channel).copied().unwrap_or(false));
        if !enabled || !self.config.configured(channel) {
            return false;
        }
        match channel {
            AiDistributionChannel::DiscordWebhook => score >= self.config.discord_minimum_score,
            AiDistributionChannel::DiscordGoLive => {
                score >= self.config.discord_go_live_minimum_score
            }
            AiDistributionChannel::ClipExport => score >= self.config.clip_minimum_score,
            _ => true,
        }
    }

    async fn deliver(&self, job: &DistributionJob) -> Result<(), String> {
        match job.channel {
            AiDistributionChannel::WebBroadcast | AiDistributionChannel::RtmpBroadcast => Ok(()),
            AiDistributionChannel::DiscordWebhook => self.deliver_discord(job).await,
            AiDistributionChannel::GameOverlay
            | AiDistributionChannel::DiscordGoLive
            | AiDistributionChannel::ClipExport => self.deliver_adapter(job).await,
        }
    }

    async fn deliver_discord(&self, job: &DistributionJob) -> Result<(), String> {
        let webhook = self
            .config
            .endpoint(AiDistributionChannel::DiscordWebhook)
            .ok_or_else(|| "Discord webhook is not configured".to_string())?;
        let package = &job.package;
        let mut description = format!(
            "{}\n\n地图：{} · 高光分 {}",
            package.body, package.context.map_title, package.score
        );
        if let Some(url) = package.assets.watch_url.as_deref() {
            description.push_str(&format!("\n[进入只读观战]({url})"));
        }
        let response = self
            .client
            .post(webhook)
            .header("X-Idempotency-Key", &job.idempotency_key)
            .json(&json!({
                "username": "Dubhe AI Live",
                "allowed_mentions": {"parse": []},
                "embeds": [{
                    "title": package.title,
                    "description": bounded_text(&description, 1500),
                    "color": 3585905,
                    "footer": {
                        "text": format!("{} · {}", package.content_id, package.reason)
                    }
                }]
            }))
            .send()
            .await
            .map_err(|error| format!("Discord request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Discord returned HTTP {}", response.status()));
        }
        Ok(())
    }

    async fn deliver_adapter(&self, job: &DistributionJob) -> Result<(), String> {
        let endpoint = self
            .config
            .endpoint(job.channel)
            .ok_or_else(|| format!("{} endpoint is not configured", job.channel.as_str()))?;
        let mut request = self
            .client
            .post(endpoint)
            .header("X-Idempotency-Key", &job.idempotency_key)
            .json(&json!({
                "schema": DISTRIBUTION_SCHEMA,
                "jobId": job.job_id,
                "idempotencyKey": job.idempotency_key,
                "channel": job.channel,
                "content": job.package
            }));
        if let Some(token) = self.config.adapter_token.as_deref() {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("{} request failed: {error}", job.channel.as_str()))?;
        if !response.status().is_success() {
            return Err(format!(
                "{} returned HTTP {}",
                job.channel.as_str(),
                response.status()
            ));
        }
        Ok(())
    }

    fn enqueue(&self, job: DistributionJob) {
        let mut evicted = None;
        self.with_state(|state| {
            if state
                .queue
                .iter()
                .any(|current| current.idempotency_key == job.idempotency_key)
            {
                return;
            }
            if state.queue.len() >= MAX_PENDING_DELIVERIES {
                evicted = state.queue.pop_front();
            }
            state.queue.push_back(job);
        });
        if let Some(evicted) = evicted {
            self.with_state(|state| {
                state
                    .counters
                    .entry(evicted.channel)
                    .or_default()
                    .dead_letters = state
                    .counters
                    .get(&evicted.channel)
                    .map(|counter| counter.dead_letters)
                    .unwrap_or(0)
                    .saturating_add(1);
            });
            let _ = append_dead_letter(&self.config.data_dir, &evicted, "queue capacity exceeded");
        }
        self.persist_queue();
    }

    fn record_success(&self, job: &DistributionJob) {
        self.with_state(|state| {
            let counter = state.counters.entry(job.channel).or_default();
            counter.delivered = counter.delivered.saturating_add(1);
            counter.last_success_at_ms = Some(now_ms());
            counter.last_error = None;
            state.recent_receipts.push_front(AiDeliveryReceipt {
                job_id: job.job_id.clone(),
                content_id: job.package.content_id.clone(),
                channel: job.channel,
                delivered_at_ms: now_ms(),
                attempts: job.attempts,
            });
            state.recent_receipts.truncate(MAX_RECENT_RECEIPTS);
        });
    }

    fn record_failure(&self, channel: AiDistributionChannel, error: &str) {
        self.with_state(|state| {
            let counter = state.counters.entry(channel).or_default();
            counter.failed = counter.failed.saturating_add(1);
            counter.last_failure_at_ms = Some(now_ms());
            counter.last_error = Some(bounded_text(error, 500));
        });
    }

    fn with_state(&self, apply: impl FnOnce(&mut AiDistributionState)) {
        if let Ok(mut state) = self.state.lock() {
            apply(&mut state);
        }
    }

    fn with_state_value<T>(&self, apply: impl FnOnce(&mut AiDistributionState) -> T) -> T {
        let mut state = self
            .state
            .lock()
            .expect("AI distribution state mutex poisoned");
        apply(&mut state)
    }

    fn persist_queue(&self) {
        let queue = self.with_state_value(|state| state.queue.iter().cloned().collect::<Vec<_>>());
        if let Err(error) =
            save_json_atomic(&self.config.data_dir, "distribution-queue.json", &queue)
        {
            eprintln!("persist AI distribution queue failed: {error}");
        }
    }

    fn persist_channel_preferences(&self) -> Result<(), String> {
        let enabled = self.with_state_value(|state| state.enabled.clone());
        save_json_atomic(
            &self.config.data_dir,
            "distribution-channels.json",
            &enabled,
        )
    }
}

fn channel_status(
    config: &AiDistributionConfig,
    state: &AiDistributionState,
    channel: AiDistributionChannel,
) -> AiChannelStatus {
    let configured = config.configured(channel);
    let enabled = state.enabled.get(&channel).copied().unwrap_or(false);
    let counter = state.counters.get(&channel).cloned().unwrap_or_default();
    let queued = state
        .queue
        .iter()
        .filter(|job| job.channel == channel)
        .count();
    let health = if !configured {
        "unconfigured"
    } else if !enabled {
        "disabled"
    } else if counter.last_error.is_some() {
        "degraded"
    } else {
        "ready"
    };
    AiChannelStatus {
        channel,
        label: channel.label(),
        delivery_mode: channel.delivery_mode(),
        configured,
        enabled,
        state: health,
        queued,
        delivered_total: counter.delivered,
        failure_total: counter.failed,
        dead_letters_total: counter.dead_letters,
        last_success_at_ms: counter.last_success_at_ms,
        last_failure_at_ms: counter.last_failure_at_ms,
        last_error: counter.last_error,
    }
}

fn metrics_from_state(state: &AiDistributionState) -> AiDistributionMetrics {
    AiDistributionMetrics {
        delivered_total: state
            .counters
            .values()
            .map(|counter| counter.delivered)
            .sum(),
        failure_total: state.counters.values().map(|counter| counter.failed).sum(),
        dead_letters_total: state
            .counters
            .values()
            .map(|counter| counter.dead_letters)
            .sum(),
        queued_deliveries: state.queue.len(),
    }
}

fn load_queue(config: &AiDistributionConfig) -> Result<VecDeque<DistributionJob>, String> {
    let path = config.data_dir.join("distribution-queue.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return load_legacy_discord_queue(config)
        }
        Err(error) => return Err(format!("read {} failed: {error}", path.display())),
    };
    let queue: Vec<DistributionJob> = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode {} failed: {error}", path.display()))?;
    Ok(queue.into_iter().take(MAX_PENDING_DELIVERIES).collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyDiscordDelivery {
    segment: AiLiveSegment,
    attempts: u8,
    next_attempt_at_ms: u64,
}

fn load_legacy_discord_queue(
    config: &AiDistributionConfig,
) -> Result<VecDeque<DistributionJob>, String> {
    let path = config.data_dir.join("discord-queue.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(VecDeque::new()),
        Err(error) => return Err(format!("read {} failed: {error}", path.display())),
    };
    let legacy: Vec<LegacyDiscordDelivery> = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode {} failed: {error}", path.display()))?;
    Ok(legacy
        .into_iter()
        .take(MAX_PENDING_DELIVERIES)
        .map(|delivery| {
            let package = AiContentPackage::from_segment(
                &delivery.segment,
                config.public_spectator_url.as_deref(),
            );
            DistributionJob {
                job_id: format!(
                    "{}:{}",
                    package.content_id,
                    AiDistributionChannel::DiscordWebhook.as_str()
                ),
                idempotency_key: format!(
                    "{}:{}",
                    package.content_id,
                    AiDistributionChannel::DiscordWebhook.as_str()
                ),
                channel: AiDistributionChannel::DiscordWebhook,
                created_at_ms: package.created_at_ms,
                package,
                attempts: delivery.attempts,
                next_attempt_at_ms: delivery.next_attempt_at_ms,
                last_error: Some("migrated from legacy Discord queue".to_string()),
            }
        })
        .collect())
}

fn load_channel_preferences(
    data_dir: &Path,
) -> Result<BTreeMap<AiDistributionChannel, bool>, String> {
    let path = data_dir.join("distribution-channels.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(format!("read {} failed: {error}", path.display())),
    };
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode {} failed: {error}", path.display()))
}

fn save_json_atomic<T: Serialize>(
    data_dir: &Path,
    file_name: &str,
    value: &T,
) -> Result<(), String> {
    fs::create_dir_all(data_dir)
        .map_err(|error| format!("create AI distribution data dir failed: {error}"))?;
    let path = data_dir.join(file_name);
    let temporary = data_dir.join(format!("{file_name}.tmp"));
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("encode {} failed: {error}", path.display()))?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("open {} failed: {error}", temporary.display()))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("flush {} failed: {error}", temporary.display()))?;
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("remove old {} failed: {error}", path.display()))?;
    }
    fs::rename(&temporary, &path)
        .map_err(|error| format!("replace {} failed: {error}", path.display()))
}

fn append_dead_letter(
    data_dir: &Path,
    job: &DistributionJob,
    last_error: &str,
) -> Result<(), String> {
    fs::create_dir_all(data_dir)
        .map_err(|error| format!("create AI distribution data dir failed: {error}"))?;
    let path = data_dir.join("distribution-dead-letter.jsonl");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open {} failed: {error}", path.display()))?;
    serde_json::to_writer(
        &mut file,
        &json!({
            "schema": DISTRIBUTION_SCHEMA,
            "failedAtMs": now_ms(),
            "lastError": bounded_text(last_error, 500),
            "job": job
        }),
    )
    .map_err(|error| format!("encode AI distribution dead letter failed: {error}"))?;
    file.write_all(b"\n")
        .and_then(|()| file.flush())
        .map_err(|error| format!("flush {} failed: {error}", path.display()))
}

fn append_content_package(data_dir: &Path, package: &AiContentPackage) -> Result<(), String> {
    fs::create_dir_all(data_dir)
        .map_err(|error| format!("create AI distribution data dir failed: {error}"))?;
    let path = data_dir.join("content-packages.jsonl");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open {} failed: {error}", path.display()))?;
    serde_json::to_writer(&mut file, package)
        .map_err(|error| format!("encode AI content package failed: {error}"))?;
    file.write_all(b"\n")
        .and_then(|()| file.flush())
        .map_err(|error| format!("flush {} failed: {error}", path.display()))
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
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(default)
}

fn bounded_score_env(name: &str, default: u8) -> u8 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u8>().ok())
        .map(|value| value.clamp(1, 100))
        .unwrap_or(default)
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn percent_encode_query(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
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

    fn segment(score: u8) -> AiLiveSegment {
        AiLiveSegment {
            schema: "obelisk.mir2.ai-live.v1".to_string(),
            segment_id: "live-test-1".to_string(),
            created_at_ms: 1_000,
            map_file_name: "0".to_string(),
            map_title: "比奇省".to_string(),
            target: Some("TestHero".to_string()),
            score,
            reason: "玩家倒地".to_string(),
            commentary: "TestHero 在比奇省倒下。".to_string(),
            subtitle: "比奇省决胜时刻".to_string(),
            source: AiLiveNarrativeSource::DeterministicFallback,
            model: None,
            audio_url: None,
            frame_digest: "digest".to_string(),
            frame_sequence: 7,
            event_kinds: vec!["death".to_string()],
        }
    }

    #[tokio::test]
    async fn one_content_package_fans_out_to_configured_channels() {
        let data_dir = env::temp_dir().join(format!(
            "mir2-ai-distribution-fanout-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let mut config = AiDistributionConfig::disabled_for_tests(data_dir.clone());
        config.public_spectator_url = Some("https://mir2.example".to_string());
        config.rtmp_enabled = true;
        let hub = AiDistributionHub::new(config).expect("distribution hub");
        let package = hub.publish(&segment(100));
        let status = hub.status();

        assert_eq!(package.schema, CONTENT_SCHEMA);
        assert_eq!(
            package.assets.watch_url.as_deref(),
            Some("https://mir2.example?spectate=1&aiLive=1&spectateMap=0")
        );
        assert_eq!(status.metrics.delivered_total, 2);
        assert_eq!(status.metrics.queued_deliveries, 0);
        assert!(data_dir.join("content-packages.jsonl").is_file());

        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn generic_queue_survives_restart_and_deduplicates() {
        let data_dir = env::temp_dir().join(format!(
            "mir2-ai-distribution-queue-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let mut config = AiDistributionConfig::disabled_for_tests(data_dir.clone());
        config.discord_webhook = Some("http://127.0.0.1:1/discord".to_string());
        let hub = AiDistributionHub::new(config.clone()).expect("distribution hub");
        hub.publish(&segment(100));
        hub.publish(&segment(100));
        assert_eq!(hub.metrics().queued_deliveries, 1);
        assert!(data_dir.join("distribution-queue.json").is_file());

        let restarted = AiDistributionHub::new(config).expect("restarted distribution hub");
        assert_eq!(restarted.metrics().queued_deliveries, 1);

        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn push_adapter_receives_canonical_package_auth_and_idempotency() {
        use axum::http::{header, HeaderMap, StatusCode};
        use axum::routing::post;
        use axum::{Json, Router};

        let app = Router::new().route(
            "/game",
            post(
                |headers: HeaderMap, Json(body): Json<serde_json::Value>| async move {
                    assert_eq!(
                        headers
                            .get(header::AUTHORIZATION)
                            .and_then(|value| value.to_str().ok()),
                        Some("Bearer adapter-secret")
                    );
                    assert_eq!(
                        headers
                            .get("X-Idempotency-Key")
                            .and_then(|value| value.to_str().ok()),
                        Some("live-test-1:gameOverlay")
                    );
                    assert_eq!(
                        body.pointer("/content/schema")
                            .and_then(|value| value.as_str()),
                        Some(CONTENT_SCHEMA)
                    );
                    StatusCode::NO_CONTENT
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock adapter");
        let address = listener.local_addr().expect("mock adapter address");
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve mock adapter");
        });
        let data_dir = env::temp_dir().join(format!(
            "mir2-ai-distribution-adapter-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let mut config = AiDistributionConfig::disabled_for_tests(data_dir.clone());
        config.game_overlay_endpoint = Some(format!("http://{address}/game"));
        config.adapter_token = Some("adapter-secret".to_string());
        let hub = AiDistributionHub::new(config).expect("distribution hub");
        hub.publish(&segment(100));
        assert_eq!(hub.metrics().queued_deliveries, 1);

        hub.retry_once().await;

        assert_eq!(hub.metrics().queued_deliveries, 0);
        assert_eq!(
            hub.channel_status(AiDistributionChannel::GameOverlay)
                .delivered_total,
            1
        );
        server.abort();
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn channel_preferences_are_persistent() {
        let data_dir = env::temp_dir().join(format!(
            "mir2-ai-distribution-channel-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let config = AiDistributionConfig::disabled_for_tests(data_dir.clone());
        let hub = AiDistributionHub::new(config.clone()).expect("distribution hub");
        hub.set_channel_enabled(AiDistributionChannel::GameOverlay, false)
            .expect("disable channel");
        let restarted = AiDistributionHub::new(config).expect("restarted distribution hub");
        assert!(
            !restarted
                .channel_status(AiDistributionChannel::GameOverlay)
                .enabled
        );

        let _ = fs::remove_dir_all(data_dir);
    }
}
