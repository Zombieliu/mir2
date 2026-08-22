use std::env;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, TimeZone, Timelike, Utc};
use postgres::Row;
use reqwest::blocking::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{
    AdminApiState, AdminError, ApiError, GameplayEventCommandSummary, GameplayEventSummaryQuery,
    ParsedClickHouseUrl, Permission, PostgresAdminRepository, admin_is_production_env,
    build_dashboard_read_model, clickhouse_password, fetch_clickhouse_gameplay_event_summary,
    now_ms, operator_from_headers, post_clickhouse_query, require_operator_permission,
    url_component,
};

const PROMPT_VERSION: &str = "mir2.daily-report.v1";
const DEFAULT_SCOPE: &str = "global";
const MAX_AI_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_MARKDOWN_BYTES: usize = 12_000;
const DISCORD_MAX_ATTEMPTS: u32 = 8;

#[derive(Debug, Clone)]
pub struct DailyReportService {
    repository: Option<PostgresAdminRepository>,
    config: Arc<DailyReportConfig>,
}

#[derive(Debug, Clone)]
struct DailyReportConfig {
    timezone: String,
    timezone_offset_minutes: i32,
    schedule_hour: u32,
    schedule_minute: u32,
    scheduler_enabled: bool,
    auto_publish: bool,
    ai_endpoint: Option<String>,
    ai_api_key: Option<String>,
    ai_model: String,
    discord_webhook_url: Option<String>,
    discord_destination_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReport {
    pub report_id: String,
    pub report_date: String,
    pub timezone: String,
    pub scope: String,
    pub status: String,
    pub source_window_start_ms: u64,
    pub source_window_end_ms: u64,
    pub metrics: DailyReportMetrics,
    pub evidence: DailyReportEvidence,
    pub operations_markdown: String,
    pub player_markdown: String,
    pub generation_source: String,
    pub model: Option<String>,
    pub prompt_version: String,
    pub input_sha256: String,
    pub content_sha256: String,
    pub created_by: String,
    pub reviewed_by: Option<String>,
    pub review_reason: Option<String>,
    pub published_by: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub reviewed_at_ms: Option<u64>,
    pub published_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReportMetrics {
    pub total_accounts: u64,
    pub total_characters: u64,
    pub online_at_generation: u64,
    pub daily_active_accounts: u64,
    pub gameplay_event_count: u64,
    pub active_zones: u64,
    pub last_gameplay_event_at_ms: Option<u64>,
    pub total_gold_stock: u64,
    pub total_credit_stock: u64,
    pub active_bans: u64,
    pub healthy_services: u64,
    pub configured_services: u64,
    pub map_population: Vec<DailyMapMetric>,
    pub level_distribution: Vec<DailyLevelBucket>,
    pub command_distribution: Vec<GameplayEventCommandSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyMapMetric {
    pub map_file_name: String,
    pub map_title: String,
    pub character_count: u64,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyLevelBucket {
    pub label: String,
    pub characters: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReportEvidence {
    pub generated_at_ms: u64,
    pub sources: Vec<DailyEvidenceSource>,
    pub warnings: Vec<String>,
    pub privacy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyEvidenceSource {
    pub source: String,
    pub status: String,
    pub detail: String,
    pub observed_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReportDelivery {
    pub delivery_id: String,
    pub report_id: String,
    pub channel: String,
    pub destination_label: String,
    pub status: String,
    pub attempts: u32,
    pub next_attempt_at_ms: Option<u64>,
    pub last_attempt_at_ms: Option<u64>,
    pub delivered_at_ms: Option<u64>,
    pub provider_message_id: Option<String>,
    pub last_error: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReportListResponse {
    pub configured: bool,
    pub scheduler_enabled: bool,
    pub discord_configured: bool,
    pub timezone: String,
    pub schedule: String,
    pub reports: Vec<DailyReport>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReportDetailResponse {
    pub report: DailyReport,
    pub deliveries: Vec<DailyReportDelivery>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicDailyReport {
    pub report_id: String,
    pub report_date: String,
    pub timezone: String,
    pub published_at_ms: u64,
    pub player_markdown: String,
    pub highlights: PublicDailyHighlights,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicDailyHighlights {
    pub daily_active_accounts: u64,
    pub gameplay_event_count: u64,
    pub active_zones: u64,
    pub top_maps: Vec<DailyMapMetric>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReportListQuery {
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateDailyReportRequest {
    pub report_date: Option<String>,
    #[serde(default)]
    pub force: bool,
    pub trigger: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDailyReportRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishDailyReportRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReportMetricsResponse {
    pub configured: bool,
    pub total_reports: u64,
    pub published_reports: u64,
    pub pending_deliveries: u64,
    pub failed_deliveries: u64,
}

#[derive(Debug, Clone)]
struct DailyWindow {
    report_date: String,
    start_ms: u64,
    end_ms: u64,
}

#[derive(Debug, Clone, Default)]
struct DailyEventAggregate {
    total_events: u64,
    active_accounts: u64,
    active_zones: u64,
    last_event_at_ms: Option<u64>,
    commands: Vec<GameplayEventCommandSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AiDailyNarrative {
    operations_markdown: String,
    player_markdown: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionMessage,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatCompletionMessage {
    content: String,
}

impl DailyReportService {
    pub fn from_env(repository: Option<PostgresAdminRepository>) -> Result<Self, AdminError> {
        let config = DailyReportConfig::from_env()?;
        Ok(Self {
            repository,
            config: Arc::new(config),
        })
    }

    pub fn configured(&self) -> bool {
        self.repository.is_some()
    }

    pub fn scheduler_enabled(&self) -> bool {
        self.config.scheduler_enabled && self.repository.is_some()
    }

    fn repository(&self) -> Result<PostgresAdminRepository, ApiError> {
        self.repository.clone().ok_or_else(|| ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "ADMIN_DATABASE_URL is required for AI daily reports".into(),
        })
    }

    fn list(&self, limit: usize) -> Result<Vec<DailyReport>, ApiError> {
        self.repository()?
            .list_daily_reports(limit)
            .map_err(ApiError::from)
    }

    fn get(&self, report_id: &str) -> Result<DailyReport, ApiError> {
        self.repository()?
            .get_daily_report(report_id)?
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                message: format!("daily report not found: {report_id}"),
            })
    }

    fn generate(
        &self,
        state: &AdminApiState,
        report_date: Option<&str>,
        force: bool,
        actor: &str,
        trigger: &str,
    ) -> Result<DailyReport, ApiError> {
        let repository = self.repository()?;
        let window = daily_window(report_date, self.config.timezone_offset_minutes, Utc::now())?;
        if let Some(existing) = repository.get_daily_report_by_key(
            &window.report_date,
            &self.config.timezone,
            DEFAULT_SCOPE,
        )? {
            if !force {
                return Ok(existing);
            }
            if existing.status == "published" {
                return Err(ApiError {
                    status: StatusCode::CONFLICT,
                    message: "published daily reports are immutable; generate a correction instead"
                        .into(),
                });
            }
        }

        let started_at_ms = now_ms();
        let run_id = format!("daily-run-{}-{}", window.report_date, started_at_ms);
        repository.insert_daily_report_run(
            &run_id,
            &window.report_date,
            trigger,
            self.config.ai_model.as_str(),
            started_at_ms,
        )?;

        let result = self.build_report(state, &window, actor);
        match result {
            Ok(report) => {
                repository.upsert_daily_report(&report)?;
                repository.finish_daily_report_run(
                    &run_id,
                    Some(&report.report_id),
                    "succeeded",
                    Some(&report.input_sha256),
                    None,
                    None,
                    now_ms(),
                )?;
                repository.append_daily_report_event(
                    &report.report_id,
                    "daily_report.generated",
                    actor,
                    Some(trigger),
                    json!({
                        "reportDate": report.report_date,
                        "generationSource": report.generation_source,
                        "model": report.model,
                        "inputSha256": report.input_sha256,
                        "contentSha256": report.content_sha256
                    }),
                    now_ms(),
                )?;
                Ok(report)
            }
            Err(error) => {
                let message = error.message.clone();
                let _ = repository.finish_daily_report_run(
                    &run_id,
                    None,
                    "failed",
                    None,
                    Some("generation_failed"),
                    Some(&message),
                    now_ms(),
                );
                Err(error)
            }
        }
    }

    fn build_report(
        &self,
        state: &AdminApiState,
        window: &DailyWindow,
        actor: &str,
    ) -> Result<DailyReport, ApiError> {
        let dashboard = build_dashboard_read_model(state.clone())?;
        let event_aggregate = match fetch_daily_event_aggregate(window.start_ms, window.end_ms) {
            Ok(aggregate) => aggregate,
            Err(error) => DailyEventAggregate {
                commands: Vec::new(),
                ..{
                    let _ = error;
                    DailyEventAggregate::default()
                }
            },
        };
        let clickhouse_ready = event_aggregate.last_event_at_ms.is_some();
        let mut warnings = Vec::new();
        if !clickhouse_ready {
            warnings.push(
                "ClickHouse gameplay event window unavailable or empty; activity metrics are zero."
                    .to_string(),
            );
        }
        if dashboard
            .services
            .iter()
            .any(|service| service.configured && service.status != "healthy")
        {
            warnings.push(
                "One or more configured services were not healthy at generation time.".into(),
            );
        }

        let metrics = DailyReportMetrics {
            total_accounts: dashboard.account_count as u64,
            total_characters: dashboard.character_count as u64,
            online_at_generation: dashboard.online_now as u64,
            daily_active_accounts: event_aggregate.active_accounts,
            gameplay_event_count: event_aggregate.total_events,
            active_zones: event_aggregate.active_zones,
            last_gameplay_event_at_ms: event_aggregate.last_event_at_ms,
            total_gold_stock: dashboard.total_gold,
            total_credit_stock: dashboard.total_credit,
            active_bans: dashboard.active_ban_count as u64,
            healthy_services: dashboard
                .services
                .iter()
                .filter(|service| service.configured && service.status == "healthy")
                .count() as u64,
            configured_services: dashboard
                .services
                .iter()
                .filter(|service| service.configured)
                .count() as u64,
            map_population: dashboard
                .hot_maps
                .iter()
                .take(20)
                .map(|map| DailyMapMetric {
                    map_file_name: map.map_file_name.clone(),
                    map_title: map.map_title.clone(),
                    character_count: map.character_count as u64,
                    percent: map.percent,
                })
                .collect(),
            level_distribution: collect_level_distribution(state)?,
            command_distribution: event_aggregate.commands,
        };
        let generated_at_ms = now_ms();
        let evidence = DailyReportEvidence {
            generated_at_ms,
            sources: vec![
                DailyEvidenceSource {
                    source: dashboard.online_source.clone(),
                    status: if dashboard.online_source.contains("unavailable") {
                        "degraded".into()
                    } else {
                        "ok".into()
                    },
                    detail: "Point-in-time online sessions and map placement.".into(),
                    observed_at_ms: dashboard.generated_at_ms,
                },
                DailyEvidenceSource {
                    source: "postgres_account_projection".into(),
                    status: "ok".into(),
                    detail: "Account, character, balance, level and ban stock snapshot.".into(),
                    observed_at_ms: dashboard.generated_at_ms,
                },
                DailyEvidenceSource {
                    source: "clickhouse_gameplay_events".into(),
                    status: if clickhouse_ready { "ok" } else { "degraded" }.into(),
                    detail: format!(
                        "UTC event window {}..{} derived from {}.",
                        window.start_ms, window.end_ms, self.config.timezone
                    ),
                    observed_at_ms: generated_at_ms,
                },
            ],
            warnings,
            privacy: "Only aggregate metrics are supplied to the narrative model; account IDs, character names, chat, IP addresses and inventories are excluded.".into(),
        };
        let input_value = json!({
            "reportDate": window.report_date,
            "timezone": self.config.timezone,
            "scope": DEFAULT_SCOPE,
            "metrics": metrics,
            "evidence": evidence
        });
        let input_sha256 = sha256_json(&input_value)?;
        let (narrative, generation_source, model) = match self.generate_ai_narrative(&input_value) {
            Ok(narrative) => (
                narrative,
                "ai".to_string(),
                Some(self.config.ai_model.clone()),
            ),
            Err(error) => {
                let mut deterministic =
                    deterministic_narrative(&window.report_date, &metrics, &evidence);
                deterministic.operations_markdown.push_str(&format!(
                    "\n\n> AI enrichment unavailable; deterministic report retained. Reason: {}",
                    safe_error(&error)
                ));
                (deterministic, "deterministic_fallback".to_string(), None)
            }
        };
        validate_narrative(&narrative)?;
        let content_sha256 = sha256_json(&json!({
            "operationsMarkdown": narrative.operations_markdown,
            "playerMarkdown": narrative.player_markdown
        }))?;
        let report_id = format!("daily-{}-{DEFAULT_SCOPE}", window.report_date);
        Ok(DailyReport {
            report_id,
            report_date: window.report_date.clone(),
            timezone: self.config.timezone.clone(),
            scope: DEFAULT_SCOPE.into(),
            status: "draft".into(),
            source_window_start_ms: window.start_ms,
            source_window_end_ms: window.end_ms,
            metrics,
            evidence,
            operations_markdown: narrative.operations_markdown,
            player_markdown: narrative.player_markdown,
            generation_source,
            model,
            prompt_version: PROMPT_VERSION.into(),
            input_sha256,
            content_sha256,
            created_by: actor.into(),
            reviewed_by: None,
            review_reason: None,
            published_by: None,
            created_at_ms: generated_at_ms,
            updated_at_ms: generated_at_ms,
            reviewed_at_ms: None,
            published_at_ms: None,
        })
    }

    fn generate_ai_narrative(&self, input: &Value) -> Result<AiDailyNarrative, String> {
        let endpoint = self
            .config
            .ai_endpoint
            .as_deref()
            .ok_or_else(|| "AI endpoint is not configured".to_string())?;
        let api_key = self
            .config
            .ai_api_key
            .as_deref()
            .ok_or_else(|| "AI API key is not configured".to_string())?;
        let http = daily_report_http_client()?;
        let response = http
            .post(endpoint)
            .bearer_auth(api_key)
            .json(&json!({
                "model": self.config.ai_model,
                "temperature": 0.2,
                "response_format": { "type": "json_object" },
                "messages": [
                    {
                        "role": "system",
                        "content": "You are the Mir2 live-operations editor. Return strict JSON with exactly operationsMarkdown and playerMarkdown. Do not invent metrics, players, rewards, incidents or causal claims. Mention degraded evidence. Operations markdown is concise Chinese for operators. Player markdown is exciting but factual Chinese, contains no internal service/security details, and makes no promise about future rewards."
                    },
                    {
                        "role": "user",
                        "content": serde_json::to_string(input).map_err(|error| error.to_string())?
                    }
                ]
            }))
            .send()
            .map_err(|error| format!("AI request failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("AI request returned HTTP {}", response.status()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_AI_RESPONSE_BYTES as u64)
        {
            return Err("AI response exceeds size limit".into());
        }
        let bytes = response
            .bytes()
            .map_err(|error| format!("AI response read failed: {error}"))?;
        if bytes.len() > MAX_AI_RESPONSE_BYTES {
            return Err("AI response exceeds size limit".into());
        }
        let response: ChatCompletionResponse = serde_json::from_slice(&bytes)
            .map_err(|error| format!("AI response decode failed: {error}"))?;
        let content = response
            .choices
            .first()
            .map(|choice| choice.message.content.trim())
            .filter(|content| !content.is_empty())
            .ok_or_else(|| "AI response contains no narrative".to_string())?;
        serde_json::from_str(content)
            .map_err(|error| format!("AI narrative JSON validation failed: {error}"))
    }

    fn approve(&self, report_id: &str, actor: &str, reason: &str) -> Result<DailyReport, ApiError> {
        require_reason(reason)?;
        let repository = self.repository()?;
        let report = repository.review_daily_report(report_id, actor, reason, now_ms())?;
        repository.append_daily_report_event(
            report_id,
            "daily_report.approved",
            actor,
            Some(reason),
            json!({ "contentSha256": report.content_sha256 }),
            now_ms(),
        )?;
        Ok(report)
    }

    fn publish(
        &self,
        report_id: &str,
        actor: &str,
        reason: &str,
    ) -> Result<DailyReportDetailResponse, ApiError> {
        require_reason(reason)?;
        let repository = self.repository()?;
        let report = repository.publish_daily_report(
            report_id,
            actor,
            reason,
            self.config.discord_webhook_url.is_some(),
            &self.config.discord_destination_label,
            now_ms(),
        )?;
        repository.append_daily_report_event(
            report_id,
            "daily_report.published",
            actor,
            Some(reason),
            json!({ "discordQueued": self.config.discord_webhook_url.is_some() }),
            now_ms(),
        )?;
        let _ = self.deliver_due();
        Ok(DailyReportDetailResponse {
            report,
            deliveries: repository.list_daily_report_deliveries(report_id)?,
        })
    }

    fn retry_discord(
        &self,
        report_id: &str,
        actor: &str,
        reason: &str,
    ) -> Result<DailyReportDetailResponse, ApiError> {
        require_reason(reason)?;
        if self.config.discord_webhook_url.is_none() {
            return Err(ApiError {
                status: StatusCode::SERVICE_UNAVAILABLE,
                message: "Discord webhook is not configured".into(),
            });
        }
        let repository = self.repository()?;
        repository.retry_daily_report_delivery(
            report_id,
            &self.config.discord_destination_label,
            now_ms(),
        )?;
        repository.append_daily_report_event(
            report_id,
            "daily_report.discord_retry_requested",
            actor,
            Some(reason),
            json!({ "destination": self.config.discord_destination_label }),
            now_ms(),
        )?;
        let _ = self.deliver_due();
        Ok(DailyReportDetailResponse {
            report: self.get(report_id)?,
            deliveries: repository.list_daily_report_deliveries(report_id)?,
        })
    }

    fn deliver_due(&self) -> Result<usize, ApiError> {
        let webhook = match self.config.discord_webhook_url.as_deref() {
            Some(webhook) => webhook,
            None => return Ok(0),
        };
        let repository = self.repository()?;
        let due = repository.list_due_daily_report_deliveries(now_ms(), 10)?;
        let mut delivered = 0;
        for delivery in due {
            let report = match repository.get_daily_report(&delivery.report_id)? {
                Some(report) => report,
                None => continue,
            };
            let attempted_at_ms = now_ms();
            let http = daily_report_http_client().map_err(|message| ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message,
            })?;
            match send_discord_report(&http, webhook, &report) {
                Ok(provider_message_id) => {
                    repository.mark_daily_report_delivery_succeeded(
                        &delivery.delivery_id,
                        attempted_at_ms,
                        provider_message_id.as_deref(),
                    )?;
                    let _ = repository.append_daily_report_event(
                        &report.report_id,
                        "daily_report.discord_delivered",
                        "daily-report-worker",
                        None,
                        json!({
                            "deliveryId": delivery.delivery_id,
                            "attempt": delivery.attempts + 1,
                            "destination": delivery.destination_label
                        }),
                        attempted_at_ms,
                    );
                    delivered += 1;
                }
                Err(error) => {
                    let attempts = delivery.attempts.saturating_add(1);
                    let dead_letter = attempts >= DISCORD_MAX_ATTEMPTS;
                    let next_attempt_at_ms = (!dead_letter)
                        .then(|| attempted_at_ms.saturating_add(retry_delay_ms(attempts)));
                    repository.mark_daily_report_delivery_failed(
                        &delivery.delivery_id,
                        attempts,
                        next_attempt_at_ms,
                        &safe_error(&error),
                        attempted_at_ms,
                    )?;
                }
            }
        }
        Ok(delivered)
    }

    fn latest_public(&self) -> Result<PublicDailyReport, ApiError> {
        let report = self
            .repository()?
            .latest_published_daily_report()?
            .ok_or_else(|| ApiError {
                status: StatusCode::NOT_FOUND,
                message: "no published daily report is available".into(),
            })?;
        Ok(PublicDailyReport {
            report_id: report.report_id,
            report_date: report.report_date,
            timezone: report.timezone,
            published_at_ms: report.published_at_ms.unwrap_or(report.updated_at_ms),
            player_markdown: report.player_markdown,
            highlights: PublicDailyHighlights {
                daily_active_accounts: report.metrics.daily_active_accounts,
                gameplay_event_count: report.metrics.gameplay_event_count,
                active_zones: report.metrics.active_zones,
                top_maps: report.metrics.map_population.into_iter().take(5).collect(),
            },
        })
    }

    fn metrics(&self) -> Result<DailyReportMetricsResponse, ApiError> {
        match &self.repository {
            Some(repository) => repository.daily_report_metrics().map_err(ApiError::from),
            None => Ok(DailyReportMetricsResponse {
                configured: false,
                total_reports: 0,
                published_reports: 0,
                pending_deliveries: 0,
                failed_deliveries: 0,
            }),
        }
    }
}

impl DailyReportConfig {
    fn from_env() -> Result<Self, AdminError> {
        let timezone_offset_minutes = env::var("ADMIN_DAILY_REPORT_TIMEZONE_OFFSET_MINUTES")
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(480);
        if !(-1_439..=1_439).contains(&timezone_offset_minutes) {
            return Err(AdminError::InvalidCommand(
                "ADMIN_DAILY_REPORT_TIMEZONE_OFFSET_MINUTES must be within -1439..=1439".into(),
            ));
        }
        let schedule_hour = env::var("ADMIN_DAILY_REPORT_SCHEDULE_HOUR")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(9);
        let schedule_minute = env::var("ADMIN_DAILY_REPORT_SCHEDULE_MINUTE")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        if schedule_hour > 23 || schedule_minute > 59 {
            return Err(AdminError::InvalidCommand(
                "daily report schedule hour/minute is invalid".into(),
            ));
        }
        let ai_endpoint = non_empty_env("ADMIN_DAILY_REPORT_AI_ENDPOINT");
        let ai_api_key = non_empty_env("ADMIN_DAILY_REPORT_AI_API_KEY");
        if ai_endpoint.is_some() != ai_api_key.is_some() {
            return Err(AdminError::InvalidCommand(
                "AI endpoint and API key must be configured together".into(),
            ));
        }
        if admin_is_production_env() {
            if let Some(endpoint) = ai_endpoint.as_deref() {
                require_https_url("AI endpoint", endpoint)?;
            }
        }
        let discord_webhook_url = non_empty_env("ADMIN_DAILY_REPORT_DISCORD_WEBHOOK_URL");
        if let Some(webhook) = discord_webhook_url.as_deref() {
            validate_discord_webhook(webhook, admin_is_production_env())?;
        }
        Ok(Self {
            timezone: env::var("ADMIN_DAILY_REPORT_TIMEZONE")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "Asia/Shanghai".into()),
            timezone_offset_minutes,
            schedule_hour,
            schedule_minute,
            scheduler_enabled: env_bool("ADMIN_DAILY_REPORT_SCHEDULER_ENABLED", false),
            auto_publish: env_bool("ADMIN_DAILY_REPORT_AUTO_PUBLISH", false),
            ai_endpoint,
            ai_api_key,
            ai_model: non_empty_env("ADMIN_DAILY_REPORT_AI_MODEL")
                .unwrap_or_else(|| "gpt-5-mini".into()),
            discord_webhook_url,
            discord_destination_label: non_empty_env(
                "ADMIN_DAILY_REPORT_DISCORD_DESTINATION_LABEL",
            )
            .unwrap_or_else(|| "mir2-world-news".into()),
        })
    }
}

impl PostgresAdminRepository {
    fn upsert_daily_report(&self, report: &DailyReport) -> Result<(), AdminError> {
        let metrics = serde_json::to_value(&report.metrics)
            .map_err(|error| AdminError::Repository(format!("encode report metrics: {error}")))?;
        let evidence = serde_json::to_value(&report.evidence)
            .map_err(|error| AdminError::Repository(format!("encode report evidence: {error}")))?;
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client
            .execute(
                "INSERT INTO admin_daily_reports (
                report_id, report_date, timezone, scope, status,
                source_window_start_ms, source_window_end_ms, metrics_json, evidence_json,
                operations_markdown, player_markdown, generation_source, model,
                prompt_version, input_sha256, content_sha256, created_by,
                created_at_ms, updated_at_ms
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$18)
             ON CONFLICT (report_date, timezone, scope) DO UPDATE
             SET status='draft',
                 source_window_start_ms=EXCLUDED.source_window_start_ms,
                 source_window_end_ms=EXCLUDED.source_window_end_ms,
                 metrics_json=EXCLUDED.metrics_json,
                 evidence_json=EXCLUDED.evidence_json,
                 operations_markdown=EXCLUDED.operations_markdown,
                 player_markdown=EXCLUDED.player_markdown,
                 generation_source=EXCLUDED.generation_source,
                 model=EXCLUDED.model,
                 prompt_version=EXCLUDED.prompt_version,
                 input_sha256=EXCLUDED.input_sha256,
                 content_sha256=EXCLUDED.content_sha256,
                 created_by=EXCLUDED.created_by,
                 reviewed_by=NULL, review_reason=NULL, reviewed_at_ms=NULL,
                 published_by=NULL, published_at_ms=NULL,
                 updated_at_ms=EXCLUDED.updated_at_ms",
                &[
                    &report.report_id,
                    &report.report_date,
                    &report.timezone,
                    &report.scope,
                    &report.status,
                    &(report.source_window_start_ms as i64),
                    &(report.source_window_end_ms as i64),
                    &metrics,
                    &evidence,
                    &report.operations_markdown,
                    &report.player_markdown,
                    &report.generation_source,
                    &report.model,
                    &report.prompt_version,
                    &report.input_sha256,
                    &report.content_sha256,
                    &report.created_by,
                    &(report.created_at_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres daily report upsert failed: {error}"))
            })?;
        Ok(())
    }

    fn list_daily_reports(&self, limit: usize) -> Result<Vec<DailyReport>, AdminError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client
            .query(
                &format!(
                    "{} ORDER BY report_date DESC, updated_at_ms DESC LIMIT $1",
                    daily_report_select_sql()
                ),
                &[&(limit.clamp(1, 200) as i64)],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres daily report list failed: {error}"))
            })?
            .iter()
            .map(row_to_daily_report)
            .collect()
    }

    fn get_daily_report(&self, report_id: &str) -> Result<Option<DailyReport>, AdminError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client
            .query_opt(
                &format!("{} WHERE report_id=$1", daily_report_select_sql()),
                &[&report_id],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres daily report read failed: {error}"))
            })?
            .as_ref()
            .map(row_to_daily_report)
            .transpose()
    }

    fn get_daily_report_by_key(
        &self,
        report_date: &str,
        timezone: &str,
        scope: &str,
    ) -> Result<Option<DailyReport>, AdminError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client
            .query_opt(
                &format!(
                    "{} WHERE report_date=$1 AND timezone=$2 AND scope=$3",
                    daily_report_select_sql()
                ),
                &[&report_date, &timezone, &scope],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres daily report key read failed: {error}"))
            })?
            .as_ref()
            .map(row_to_daily_report)
            .transpose()
    }

    fn latest_published_daily_report(&self) -> Result<Option<DailyReport>, AdminError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client
            .query_opt(
                &format!(
                    "{} WHERE status='published' ORDER BY report_date DESC, published_at_ms DESC LIMIT 1",
                    daily_report_select_sql()
                ),
                &[],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres latest daily report failed: {error}"))
            })?
            .as_ref()
            .map(row_to_daily_report)
            .transpose()
    }

    fn insert_daily_report_run(
        &self,
        run_id: &str,
        report_date: &str,
        trigger: &str,
        model: &str,
        started_at_ms: u64,
    ) -> Result<(), AdminError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client
            .execute(
                "INSERT INTO admin_daily_report_runs (
                run_id, report_date, trigger, status, model, started_at_ms
             ) VALUES ($1,$2,$3,'running',$4,$5)",
                &[
                    &run_id,
                    &report_date,
                    &trigger,
                    &model,
                    &(started_at_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres daily report run insert failed: {error}"))
            })?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_daily_report_run(
        &self,
        run_id: &str,
        report_id: Option<&str>,
        status: &str,
        input_sha256: Option<&str>,
        error_code: Option<&str>,
        error_message: Option<&str>,
        completed_at_ms: u64,
    ) -> Result<(), AdminError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client.execute(
            "UPDATE admin_daily_report_runs
             SET report_id=$2,status=$3,input_sha256=$4,error_code=$5,error_message=$6,completed_at_ms=$7
             WHERE run_id=$1",
            &[&run_id, &report_id, &status, &input_sha256, &error_code, &error_message, &(completed_at_ms as i64)],
        ).map_err(|error| AdminError::Repository(format!("postgres daily report run update failed: {error}")))?;
        Ok(())
    }

    fn append_daily_report_event(
        &self,
        report_id: &str,
        event_type: &str,
        actor_id: &str,
        reason: Option<&str>,
        payload: Value,
        occurred_at_ms: u64,
    ) -> Result<(), AdminError> {
        let event_id = format!(
            "{}-{}-{}",
            event_type.replace('.', "-"),
            report_id,
            occurred_at_ms
        );
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client
            .execute(
                "INSERT INTO admin_daily_report_events (
                event_id,report_id,event_type,actor_id,reason,payload_json,occurred_at_ms
             ) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (event_id) DO NOTHING",
                &[
                    &event_id,
                    &report_id,
                    &event_type,
                    &actor_id,
                    &reason,
                    &payload,
                    &(occurred_at_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!(
                    "postgres daily report event insert failed: {error}"
                ))
            })?;
        Ok(())
    }

    fn review_daily_report(
        &self,
        report_id: &str,
        actor: &str,
        reason: &str,
        reviewed_at_ms: u64,
    ) -> Result<DailyReport, ApiError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        let updated = client.execute(
            "UPDATE admin_daily_reports
             SET status='approved',reviewed_by=$2,review_reason=$3,reviewed_at_ms=$4,updated_at_ms=$4
             WHERE report_id=$1 AND status='draft'",
            &[&report_id, &actor, &reason, &(reviewed_at_ms as i64)],
        ).map_err(|error| ApiError::from(AdminError::Repository(format!("postgres daily report approve failed: {error}"))))?;
        drop(client);
        if updated == 0 {
            return Err(ApiError {
                status: StatusCode::CONFLICT,
                message: "only draft daily reports can be approved".into(),
            });
        }
        self.get_daily_report(report_id)?.ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "daily report disappeared after approval".into(),
        })
    }

    fn publish_daily_report(
        &self,
        report_id: &str,
        actor: &str,
        reason: &str,
        queue_discord: bool,
        destination_label: &str,
        published_at_ms: u64,
    ) -> Result<DailyReport, ApiError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        let mut transaction = client.transaction().map_err(|error| {
            ApiError::from(AdminError::Repository(format!(
                "daily report publish transaction failed: {error}"
            )))
        })?;
        let updated = transaction.execute(
            "UPDATE admin_daily_reports
             SET status='published',published_by=$2,published_at_ms=$3,updated_at_ms=$3,
                 review_reason=COALESCE(review_reason,'') || CASE WHEN review_reason IS NULL OR review_reason='' THEN $4 ELSE E'\\nPublish: ' || $4 END
             WHERE report_id=$1 AND status='approved'",
            &[&report_id, &actor, &(published_at_ms as i64), &reason],
        ).map_err(|error| ApiError::from(AdminError::Repository(format!("postgres daily report publish failed: {error}"))))?;
        if updated == 0 {
            return Err(ApiError {
                status: StatusCode::CONFLICT,
                message: "daily report must be approved before publication".into(),
            });
        }
        if queue_discord {
            let delivery_id = format!("discord-{report_id}");
            transaction.execute(
                "INSERT INTO admin_daily_report_deliveries (
                    delivery_id,report_id,channel,destination_label,status,attempts,
                    next_attempt_at_ms,created_at_ms,updated_at_ms
                 ) VALUES ($1,$2,'discord',$3,'pending',0,$4,$4,$4)
                 ON CONFLICT (report_id,channel,destination_label) DO UPDATE
                 SET status='pending',next_attempt_at_ms=EXCLUDED.next_attempt_at_ms,last_error=NULL,updated_at_ms=EXCLUDED.updated_at_ms",
                &[&delivery_id, &report_id, &destination_label, &(published_at_ms as i64)],
            ).map_err(|error| ApiError::from(AdminError::Repository(format!("postgres Discord delivery enqueue failed: {error}"))))?;
        }
        transaction.commit().map_err(|error| {
            ApiError::from(AdminError::Repository(format!(
                "daily report publish commit failed: {error}"
            )))
        })?;
        drop(client);
        self.get_daily_report(report_id)?.ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "daily report disappeared after publication".into(),
        })
    }

    fn list_daily_report_deliveries(
        &self,
        report_id: &str,
    ) -> Result<Vec<DailyReportDelivery>, ApiError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client
            .query(
                &format!(
                    "{} WHERE report_id=$1 ORDER BY created_at_ms DESC",
                    daily_delivery_select_sql()
                ),
                &[&report_id],
            )
            .map_err(|error| {
                ApiError::from(AdminError::Repository(format!(
                    "postgres daily report delivery list failed: {error}"
                )))
            })?
            .iter()
            .map(row_to_daily_delivery)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ApiError::from)
    }

    fn list_due_daily_report_deliveries(
        &self,
        current_ms: u64,
        limit: usize,
    ) -> Result<Vec<DailyReportDelivery>, ApiError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client.query(
            &format!("{} WHERE status IN ('pending','retry') AND next_attempt_at_ms <= $1 ORDER BY next_attempt_at_ms ASC LIMIT $2", daily_delivery_select_sql()),
            &[&(current_ms as i64), &(limit.clamp(1, 50) as i64)],
        ).map_err(|error| ApiError::from(AdminError::Repository(format!("postgres due daily deliveries failed: {error}"))))?
            .iter().map(row_to_daily_delivery).collect::<Result<Vec<_>, _>>().map_err(ApiError::from)
    }

    fn retry_daily_report_delivery(
        &self,
        report_id: &str,
        destination_label: &str,
        current_ms: u64,
    ) -> Result<(), ApiError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        let delivery_id = format!("discord-{report_id}");
        client.execute(
            "INSERT INTO admin_daily_report_deliveries (
                delivery_id,report_id,channel,destination_label,status,attempts,
                next_attempt_at_ms,created_at_ms,updated_at_ms
             ) SELECT $1,report_id,'discord',$3,'pending',0,$4,$4,$4
               FROM admin_daily_reports WHERE report_id=$2 AND status='published'
             ON CONFLICT (report_id,channel,destination_label) DO UPDATE
             SET status='pending',attempts=0,next_attempt_at_ms=EXCLUDED.next_attempt_at_ms,last_error=NULL,updated_at_ms=EXCLUDED.updated_at_ms",
            &[&delivery_id, &report_id, &destination_label, &(current_ms as i64)],
        ).map_err(|error| ApiError::from(AdminError::Repository(format!("postgres Discord retry queue failed: {error}"))))?;
        Ok(())
    }

    fn mark_daily_report_delivery_succeeded(
        &self,
        delivery_id: &str,
        current_ms: u64,
        provider_message_id: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client.execute(
            "UPDATE admin_daily_report_deliveries
             SET status='delivered',attempts=attempts+1,next_attempt_at_ms=NULL,last_attempt_at_ms=$2,
                 delivered_at_ms=$2,provider_message_id=$3,last_error=NULL,updated_at_ms=$2
             WHERE delivery_id=$1",
            &[&delivery_id, &(current_ms as i64), &provider_message_id],
        ).map_err(|error| ApiError::from(AdminError::Repository(format!("postgres Discord success update failed: {error}"))))?;
        Ok(())
    }

    fn mark_daily_report_delivery_failed(
        &self,
        delivery_id: &str,
        attempts: u32,
        next_attempt_at_ms: Option<u64>,
        error_message: &str,
        current_ms: u64,
    ) -> Result<(), ApiError> {
        let status = if next_attempt_at_ms.is_some() {
            "retry"
        } else {
            "dead_letter"
        };
        let next_attempt_at_ms = next_attempt_at_ms.map(|value| value as i64);
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        client.execute(
            "UPDATE admin_daily_report_deliveries
             SET status=$2,attempts=$3,next_attempt_at_ms=$4,last_attempt_at_ms=$5,last_error=$6,updated_at_ms=$5
             WHERE delivery_id=$1",
            &[&delivery_id, &status, &(attempts as i32), &next_attempt_at_ms, &(current_ms as i64), &error_message],
        ).map_err(|error| ApiError::from(AdminError::Repository(format!("postgres Discord failure update failed: {error}"))))?;
        Ok(())
    }

    fn daily_report_metrics(&self) -> Result<DailyReportMetricsResponse, AdminError> {
        let mut client = self.client.lock().map_err(super::repository_lock_error)?;
        let row = client.query_one(
            "SELECT
                (SELECT count(*) FROM admin_daily_reports) AS total_reports,
                (SELECT count(*) FROM admin_daily_reports WHERE status='published') AS published_reports,
                (SELECT count(*) FROM admin_daily_report_deliveries WHERE status IN ('pending','retry')) AS pending_deliveries,
                (SELECT count(*) FROM admin_daily_report_deliveries WHERE status='dead_letter') AS failed_deliveries",
            &[],
        ).map_err(|error| AdminError::Repository(format!("postgres daily report metrics failed: {error}")))?;
        Ok(DailyReportMetricsResponse {
            configured: true,
            total_reports: row.get::<_, i64>("total_reports").max(0) as u64,
            published_reports: row.get::<_, i64>("published_reports").max(0) as u64,
            pending_deliveries: row.get::<_, i64>("pending_deliveries").max(0) as u64,
            failed_deliveries: row.get::<_, i64>("failed_deliveries").max(0) as u64,
        })
    }
}

pub(super) async fn list_daily_reports(
    State(state): State<AdminApiState>,
    Query(query): Query<DailyReportListQuery>,
) -> Result<Json<DailyReportListResponse>, ApiError> {
    let service = state.daily_reports.clone();
    let reports = tokio::task::spawn_blocking(move || service.list(query.limit.unwrap_or(31)))
        .await
        .map_err(super::join_error)??;
    Ok(Json(DailyReportListResponse {
        configured: state.daily_reports.configured(),
        scheduler_enabled: state.daily_reports.scheduler_enabled(),
        discord_configured: state.daily_reports.config.discord_webhook_url.is_some(),
        timezone: state.daily_reports.config.timezone.clone(),
        schedule: format!(
            "{:02}:{:02}",
            state.daily_reports.config.schedule_hour, state.daily_reports.config.schedule_minute
        ),
        reports,
    }))
}

pub(super) async fn get_daily_report(
    State(state): State<AdminApiState>,
    Path(report_id): Path<String>,
) -> Result<Json<DailyReportDetailResponse>, ApiError> {
    let service = state.daily_reports.clone();
    tokio::task::spawn_blocking(move || {
        let report = service.get(&report_id)?;
        let deliveries = service
            .repository()?
            .list_daily_report_deliveries(&report_id)?;
        Ok(Json(DailyReportDetailResponse { report, deliveries }))
    })
    .await
    .map_err(super::join_error)?
}

pub(super) async fn generate_daily_report(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<GenerateDailyReportRequest>,
) -> Result<Json<DailyReport>, ApiError> {
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
    require_operator_permission(&operator, Permission::ContentPublish)?;
    let actor = operator.id;
    let service = state.daily_reports.clone();
    let state_for_generation = state.clone();
    let trigger = request
        .trigger
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("manual")
        .to_string();
    tokio::task::spawn_blocking(move || {
        service
            .generate(
                &state_for_generation,
                request.report_date.as_deref(),
                request.force,
                &actor,
                &trigger,
            )
            .map(Json)
    })
    .await
    .map_err(super::join_error)?
}

pub(super) async fn approve_daily_report(
    State(state): State<AdminApiState>,
    Path(report_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ReviewDailyReportRequest>,
) -> Result<Json<DailyReport>, ApiError> {
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
    require_operator_permission(&operator, Permission::ContentPublish)?;
    let service = state.daily_reports.clone();
    tokio::task::spawn_blocking(move || {
        service
            .approve(&report_id, &operator.id, &request.reason)
            .map(Json)
    })
    .await
    .map_err(super::join_error)?
}

pub(super) async fn publish_daily_report(
    State(state): State<AdminApiState>,
    Path(report_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PublishDailyReportRequest>,
) -> Result<Json<DailyReportDetailResponse>, ApiError> {
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
    require_operator_permission(&operator, Permission::ContentPublish)?;
    let service = state.daily_reports.clone();
    tokio::task::spawn_blocking(move || {
        service
            .publish(&report_id, &operator.id, &request.reason)
            .map(Json)
    })
    .await
    .map_err(super::join_error)?
}

pub(super) async fn retry_daily_report_discord(
    State(state): State<AdminApiState>,
    Path(report_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PublishDailyReportRequest>,
) -> Result<Json<DailyReportDetailResponse>, ApiError> {
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
    require_operator_permission(&operator, Permission::ContentPublish)?;
    let service = state.daily_reports.clone();
    tokio::task::spawn_blocking(move || {
        service
            .retry_discord(&report_id, &operator.id, &request.reason)
            .map(Json)
    })
    .await
    .map_err(super::join_error)?
}

pub(super) async fn latest_public_daily_report(
    State(state): State<AdminApiState>,
) -> Result<Json<PublicDailyReport>, ApiError> {
    let service = state.daily_reports.clone();
    tokio::task::spawn_blocking(move || service.latest_public().map(Json))
        .await
        .map_err(super::join_error)?
}

pub(super) async fn daily_report_prometheus(
    State(state): State<AdminApiState>,
) -> Result<String, ApiError> {
    let daily_report_service = state.daily_reports.clone();
    let world_director_service = state.world_director.clone();
    let (metrics, world_director_metrics) = tokio::task::spawn_blocking(move || {
        let metrics = daily_report_service.metrics()?;
        let world_director_metrics = world_director_service
            .prometheus_metrics(super::world_director_now_ms())
            .map_err(super::world_director_api_error)?;
        Ok::<_, ApiError>((metrics, world_director_metrics))
    })
    .await
    .map_err(super::join_error)??;
    Ok(format!(
        "# HELP mir2_daily_reports_configured Whether persistent AI daily reports are configured.\n\
         # TYPE mir2_daily_reports_configured gauge\n\
         mir2_daily_reports_configured {}\n\
         # HELP mir2_daily_reports_total Stored daily reports.\n\
         # TYPE mir2_daily_reports_total gauge\n\
         mir2_daily_reports_total {}\n\
         # HELP mir2_daily_reports_published_total Published daily reports.\n\
         # TYPE mir2_daily_reports_published_total gauge\n\
         mir2_daily_reports_published_total {}\n\
         # HELP mir2_daily_report_discord_pending Pending or retrying Discord deliveries.\n\
         # TYPE mir2_daily_report_discord_pending gauge\n\
         mir2_daily_report_discord_pending {}\n\
         # HELP mir2_daily_report_discord_dead_letter Dead-letter Discord deliveries.\n\
         # TYPE mir2_daily_report_discord_dead_letter gauge\n\
         mir2_daily_report_discord_dead_letter {}\n",
        u8::from(metrics.configured),
        metrics.total_reports,
        metrics.published_reports,
        metrics.pending_deliveries,
        metrics.failed_deliveries
    ) + &world_director_metrics)
}

pub fn spawn_daily_report_scheduler(state: AdminApiState) {
    if !state.daily_reports.scheduler_enabled() {
        return;
    }
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let state = state.clone();
            let service = state.daily_reports.clone();
            if let Err(error) =
                tokio::task::spawn_blocking(move || scheduler_tick(&state, &service))
                    .await
                    .map_err(super::join_error)
                    .and_then(|result| result)
            {
                eprintln!(
                    "AI daily report scheduler tick failed: {}",
                    safe_error(&error.message)
                );
            }
        }
    });
}

fn scheduler_tick(state: &AdminApiState, service: &DailyReportService) -> Result<(), ApiError> {
    let _ = service.deliver_due()?;
    let offset =
        FixedOffset::east_opt(service.config.timezone_offset_minutes * 60).ok_or_else(|| {
            ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "daily report timezone offset is invalid".into(),
            }
        })?;
    let local_now = Utc::now().with_timezone(&offset);
    if local_now.hour() < service.config.schedule_hour
        || (local_now.hour() == service.config.schedule_hour
            && local_now.minute() < service.config.schedule_minute)
    {
        return Ok(());
    }
    let target_date = local_now
        .date_naive()
        .pred_opt()
        .ok_or_else(|| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "daily report previous date cannot be calculated".into(),
        })?
        .format("%Y-%m-%d")
        .to_string();
    let report = service.generate(
        state,
        Some(&target_date),
        false,
        "daily-report-scheduler",
        "scheduled",
    )?;
    if service.config.auto_publish && report.status == "draft" {
        let report = service.approve(
            &report.report_id,
            "daily-report-scheduler",
            "Automated publication explicitly enabled by operator configuration.",
        )?;
        let _ = service.publish(
            &report.report_id,
            "daily-report-scheduler",
            "Automated scheduled publication.",
        )?;
    }
    Ok(())
}

fn daily_window(
    requested_date: Option<&str>,
    timezone_offset_minutes: i32,
    now: DateTime<Utc>,
) -> Result<DailyWindow, ApiError> {
    let offset = FixedOffset::east_opt(timezone_offset_minutes * 60).ok_or_else(|| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: "daily report timezone offset is invalid".into(),
    })?;
    let date = match requested_date
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "reportDate must use YYYY-MM-DD".into(),
        })?,
        None => now
            .with_timezone(&offset)
            .date_naive()
            .pred_opt()
            .ok_or_else(|| ApiError {
                status: StatusCode::BAD_REQUEST,
                message: "previous report date is unavailable".into(),
            })?,
    };
    let today = now.with_timezone(&offset).date_naive();
    if date >= today {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "daily reports can only be generated for completed local dates".into(),
        });
    }
    let start = offset
        .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
        .single()
        .ok_or_else(|| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "report date start is invalid".into(),
        })?;
    let next = date.succ_opt().ok_or_else(|| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: "report date end is invalid".into(),
    })?;
    let end = offset
        .with_ymd_and_hms(next.year(), next.month(), next.day(), 0, 0, 0)
        .single()
        .ok_or_else(|| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "report date end is invalid".into(),
        })?;
    Ok(DailyWindow {
        report_date: date.format("%Y-%m-%d").to_string(),
        start_ms: start.timestamp_millis().max(0) as u64,
        end_ms: end.timestamp_millis().max(0) as u64,
    })
}

fn fetch_daily_event_aggregate(
    start_ms: u64,
    end_ms: u64,
) -> Result<DailyEventAggregate, ApiError> {
    let base_url =
        env::var("ADMIN_CLICKHOUSE_URL").unwrap_or_else(|_| "http://127.0.0.1:8123".into());
    let user = env::var("ADMIN_CLICKHOUSE_USER").unwrap_or_else(|_| "mir2".into());
    let password = clickhouse_password()?;
    let database = env::var("ADMIN_CLICKHOUSE_DATABASE").unwrap_or_else(|_| "mir2_events".into());
    let url = ParsedClickHouseUrl::parse(base_url.trim()).map_err(|message| ApiError {
        status: StatusCode::BAD_GATEWAY,
        message,
    })?;
    let path = format!(
        "/?user={}&password={}&database={}",
        url_component(&user),
        url_component(&password),
        url_component(&database)
    );
    let query = format!(
        "SELECT count() AS totalEvents,\
                uniqExactIf(account_id, isNotNull(account_id)) AS activeAccounts,\
                uniqExact(zone_id) AS activeZones,\
                maxOrNull(occurred_at_ms) AS lastEventAtMs \
         FROM gameplay_events \
         WHERE occurred_at_ms >= {start_ms} AND occurred_at_ms < {end_ms} \
         FORMAT JSONEachRow"
    );
    let response = post_clickhouse_query(&url, &path, &query).map_err(|message| ApiError {
        status: StatusCode::BAD_GATEWAY,
        message,
    })?;
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct AggregateRow {
        total_events: u64,
        active_accounts: u64,
        active_zones: u64,
        last_event_at_ms: Option<u64>,
    }
    let row: AggregateRow =
        serde_json::from_str(response.lines().next().unwrap_or("{}")).map_err(|error| {
            ApiError {
                status: StatusCode::BAD_GATEWAY,
                message: format!("ClickHouse daily aggregate decode failed: {error}"),
            }
        })?;
    let summary = fetch_clickhouse_gameplay_event_summary(
        &GameplayEventSummaryQuery {
            window_seconds: Some((end_ms.saturating_sub(start_ms)) / 1_000),
            limit: Some(100),
            ..GameplayEventSummaryQuery::default()
        },
        end_ms,
        (end_ms.saturating_sub(start_ms)) / 1_000,
    )?;
    Ok(DailyEventAggregate {
        total_events: row.total_events,
        active_accounts: row.active_accounts,
        active_zones: row.active_zones,
        last_event_at_ms: row.last_event_at_ms,
        commands: summary.commands,
    })
}

fn collect_level_distribution(state: &AdminApiState) -> Result<Vec<DailyLevelBucket>, ApiError> {
    let snapshot = state.read_models.load_snapshot().map_err(ApiError::from)?;
    let mut buckets = [0_u64; 6];
    for account in snapshot.accounts {
        for save in account.saves.values() {
            let index = match save.save.character.level {
                0..=9 => 0,
                10..=19 => 1,
                20..=29 => 2,
                30..=39 => 3,
                40..=49 => 4,
                _ => 5,
            };
            buckets[index] = buckets[index].saturating_add(1);
        }
    }
    Ok(["0–9", "10–19", "20–29", "30–39", "40–49", "50+"]
        .into_iter()
        .zip(buckets)
        .map(|(label, characters)| DailyLevelBucket {
            label: label.into(),
            characters,
        })
        .collect())
}

fn deterministic_narrative(
    report_date: &str,
    metrics: &DailyReportMetrics,
    evidence: &DailyReportEvidence,
) -> AiDailyNarrative {
    let top_maps = if metrics.map_population.is_empty() {
        "暂无地图人口快照".to_string()
    } else {
        metrics
            .map_population
            .iter()
            .take(5)
            .map(|map| {
                format!(
                    "{}（{}）{} 人",
                    map.map_title, map.map_file_name, map.character_count
                )
            })
            .collect::<Vec<_>>()
            .join("、")
    };
    let warnings = if evidence.warnings.is_empty() {
        "无数据完整性告警".to_string()
    } else {
        evidence.warnings.join("；")
    };
    AiDailyNarrative {
        operations_markdown: format!(
            "# Mir2 运营日报 · {report_date}\n\n\
             ## 核心指标\n\n\
             - 日活跃账号：{}\n\
             - 游戏事件：{}\n\
             - 活跃 Zone：{}\n\
             - 生成时在线：{}\n\
             - 角色总数：{}\n\
             - 金币存量：{}\n\
             - 服务健康：{}/{}\n\n\
             ## 地图与内容\n\n{}\n\n\
             ## 数据质量\n\n{}",
            metrics.daily_active_accounts,
            metrics.gameplay_event_count,
            metrics.active_zones,
            metrics.online_at_generation,
            metrics.total_characters,
            metrics.total_gold_stock,
            metrics.healthy_services,
            metrics.configured_services,
            top_maps,
            warnings
        ),
        player_markdown: format!(
            "# 玛法世界日报 · {report_date}\n\n\
             昨日共有 **{}** 位冒险者留下战斗足迹，世界记录了 **{}** 次游戏行动，\
             活跃区域覆盖 **{}** 个 Zone。\n\n\
             ## 冒险者聚集地\n\n{}\n\n\
             世界仍在由玩家与社区节点共同运行。所有数字来自已完成日期的聚合记录。",
            metrics.daily_active_accounts,
            metrics.gameplay_event_count,
            metrics.active_zones,
            top_maps
        ),
    }
}

fn validate_narrative(narrative: &AiDailyNarrative) -> Result<(), ApiError> {
    for (name, value) in [
        ("operationsMarkdown", narrative.operations_markdown.trim()),
        ("playerMarkdown", narrative.player_markdown.trim()),
    ] {
        if value.is_empty() {
            return Err(ApiError {
                status: StatusCode::BAD_GATEWAY,
                message: format!("AI daily report {name} is empty"),
            });
        }
        if value.len() > MAX_MARKDOWN_BYTES {
            return Err(ApiError {
                status: StatusCode::BAD_GATEWAY,
                message: format!("AI daily report {name} exceeds size limit"),
            });
        }
    }
    Ok(())
}

fn send_discord_report(
    http: &HttpClient,
    webhook: &str,
    report: &DailyReport,
) -> Result<Option<String>, String> {
    let separator = if webhook.contains('?') { '&' } else { '?' };
    let url = format!("{webhook}{separator}wait=true");
    let description = truncate_utf8(&report.player_markdown, 3_900);
    let response = http
        .post(url)
        .json(&json!({
            "username": "Mir2 世界日报",
            "allowed_mentions": { "parse": [] },
            "embeds": [{
                "title": format!("玛法世界日报 · {}", report.report_date),
                "description": description,
                "color": 0x57F0D2,
                "footer": { "text": format!("{} · {}", report.timezone, report.content_sha256.chars().take(12).collect::<String>()) }
            }]
        }))
        .send()
        .map_err(|error| format!("Discord delivery failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Discord delivery returned HTTP {status}"));
    }
    let body = response
        .text()
        .map_err(|error| format!("Discord response read failed: {error}"))?;
    if body.trim().is_empty() {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(&body)
        .map_err(|error| format!("Discord response decode failed: {error}"))?;
    Ok(value.get("id").and_then(Value::as_str).map(str::to_string))
}

fn daily_report_select_sql() -> &'static str {
    "SELECT report_id,report_date,timezone,scope,status,source_window_start_ms,
            source_window_end_ms,metrics_json,evidence_json,operations_markdown,
            player_markdown,generation_source,model,prompt_version,input_sha256,
            content_sha256,created_by,reviewed_by,review_reason,published_by,
            created_at_ms,updated_at_ms,reviewed_at_ms,published_at_ms
     FROM admin_daily_reports"
}

fn daily_delivery_select_sql() -> &'static str {
    "SELECT delivery_id,report_id,channel,destination_label,status,attempts,
            next_attempt_at_ms,last_attempt_at_ms,delivered_at_ms,provider_message_id,
            last_error,created_at_ms,updated_at_ms
     FROM admin_daily_report_deliveries"
}

fn row_to_daily_report(row: &Row) -> Result<DailyReport, AdminError> {
    let metrics: Value = row.get("metrics_json");
    let evidence: Value = row.get("evidence_json");
    Ok(DailyReport {
        report_id: row.get("report_id"),
        report_date: row.get("report_date"),
        timezone: row.get("timezone"),
        scope: row.get("scope"),
        status: row.get("status"),
        source_window_start_ms: non_negative_i64(row, "source_window_start_ms"),
        source_window_end_ms: non_negative_i64(row, "source_window_end_ms"),
        metrics: serde_json::from_value(metrics).map_err(|error| {
            AdminError::Repository(format!("decode daily report metrics failed: {error}"))
        })?,
        evidence: serde_json::from_value(evidence).map_err(|error| {
            AdminError::Repository(format!("decode daily report evidence failed: {error}"))
        })?,
        operations_markdown: row.get("operations_markdown"),
        player_markdown: row.get("player_markdown"),
        generation_source: row.get("generation_source"),
        model: row.get("model"),
        prompt_version: row.get("prompt_version"),
        input_sha256: row.get("input_sha256"),
        content_sha256: row.get("content_sha256"),
        created_by: row.get("created_by"),
        reviewed_by: row.get("reviewed_by"),
        review_reason: row.get("review_reason"),
        published_by: row.get("published_by"),
        created_at_ms: non_negative_i64(row, "created_at_ms"),
        updated_at_ms: non_negative_i64(row, "updated_at_ms"),
        reviewed_at_ms: optional_non_negative_i64(row, "reviewed_at_ms"),
        published_at_ms: optional_non_negative_i64(row, "published_at_ms"),
    })
}

fn row_to_daily_delivery(row: &Row) -> Result<DailyReportDelivery, AdminError> {
    Ok(DailyReportDelivery {
        delivery_id: row.get("delivery_id"),
        report_id: row.get("report_id"),
        channel: row.get("channel"),
        destination_label: row.get("destination_label"),
        status: row.get("status"),
        attempts: row.get::<_, i32>("attempts").max(0) as u32,
        next_attempt_at_ms: optional_non_negative_i64(row, "next_attempt_at_ms"),
        last_attempt_at_ms: optional_non_negative_i64(row, "last_attempt_at_ms"),
        delivered_at_ms: optional_non_negative_i64(row, "delivered_at_ms"),
        provider_message_id: row.get("provider_message_id"),
        last_error: row.get("last_error"),
        created_at_ms: non_negative_i64(row, "created_at_ms"),
        updated_at_ms: non_negative_i64(row, "updated_at_ms"),
    })
}

fn non_negative_i64(row: &Row, column: &str) -> u64 {
    row.get::<_, i64>(column).max(0) as u64
}

fn optional_non_negative_i64(row: &Row, column: &str) -> Option<u64> {
    row.get::<_, Option<i64>>(column)
        .map(|value| value.max(0) as u64)
}

fn retry_delay_ms(attempts: u32) -> u64 {
    const DELAYS: [u64; 7] = [
        60_000,
        5 * 60_000,
        30 * 60_000,
        2 * 60 * 60_000,
        6 * 60 * 60_000,
        12 * 60 * 60_000,
        24 * 60 * 60_000,
    ];
    DELAYS[(attempts.saturating_sub(1) as usize).min(DELAYS.len() - 1)]
}

fn daily_report_http_client() -> Result<HttpClient, String> {
    HttpClient::builder()
        .timeout(Duration::from_secs(
            env::var("ADMIN_DAILY_REPORT_HTTP_TIMEOUT_SECONDS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(20)
                .clamp(3, 60),
        ))
        .user_agent("mir2-admin-daily-report/1")
        .build()
        .map_err(|error| format!("daily report HTTP client build failed: {error}"))
}

fn sha256_json(value: &Value) -> Result<String, ApiError> {
    let bytes = serde_json::to_vec(value).map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("daily report canonical JSON encode failed: {error}"),
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn require_reason(reason: &str) -> Result<(), ApiError> {
    if reason.trim().chars().count() < 8 {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "reason must contain at least 8 characters".into(),
        });
    }
    Ok(())
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn require_https_url(label: &str, value: &str) -> Result<(), AdminError> {
    let url = reqwest::Url::parse(value)
        .map_err(|_| AdminError::InvalidCommand(format!("{label} must be a valid URL")))?;
    if url.scheme() != "https" {
        return Err(AdminError::InvalidCommand(format!(
            "{label} must use HTTPS in production"
        )));
    }
    Ok(())
}

fn validate_discord_webhook(value: &str, production: bool) -> Result<(), AdminError> {
    let url = reqwest::Url::parse(value)
        .map_err(|_| AdminError::InvalidCommand("Discord webhook must be a valid URL".into()))?;
    if production {
        if url.scheme() != "https" {
            return Err(AdminError::InvalidCommand(
                "Discord webhook must use HTTPS in production".into(),
            ));
        }
        let host = url.host_str().unwrap_or_default();
        if !matches!(host, "discord.com" | "discordapp.com")
            || !url.path().starts_with("/api/webhooks/")
        {
            return Err(AdminError::InvalidCommand(
                "production Discord webhook must use the official Discord webhook endpoint".into(),
            ));
        }
    }
    Ok(())
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

fn safe_error(value: &str) -> String {
    let mut redacted = value.to_string();
    for secret in [
        non_empty_env("ADMIN_DAILY_REPORT_AI_API_KEY"),
        non_empty_env("ADMIN_DAILY_REPORT_DISCORD_WEBHOOK_URL"),
        non_empty_env("ADMIN_CLICKHOUSE_PASSWORD"),
    ]
    .into_iter()
    .flatten()
    {
        redacted = redacted.replace(&secret, "[REDACTED]");
    }
    truncate_utf8(&redacted.replace('\n', " "), 500)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_local_date_maps_to_exact_utc_window() {
        let now = DateTime::parse_from_rfc3339("2026-07-29T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let window = daily_window(Some("2026-07-28"), 480, now).unwrap();
        assert_eq!(window.report_date, "2026-07-28");
        assert_eq!(
            DateTime::from_timestamp_millis(window.start_ms as i64)
                .unwrap()
                .to_rfc3339(),
            "2026-07-27T16:00:00+00:00"
        );
        assert_eq!(window.end_ms - window.start_ms, 86_400_000);
    }

    #[test]
    fn current_or_future_dates_are_rejected() {
        let now = DateTime::parse_from_rfc3339("2026-07-29T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(daily_window(Some("2026-07-29"), 480, now).is_err());
        assert!(daily_window(Some("2026-07-30"), 480, now).is_err());
    }

    #[test]
    fn deterministic_fallback_contains_only_aggregate_facts() {
        let metrics = DailyReportMetrics {
            daily_active_accounts: 42,
            gameplay_event_count: 900,
            active_zones: 3,
            map_population: vec![DailyMapMetric {
                map_file_name: "0".into(),
                map_title: "比奇省".into(),
                character_count: 12,
                percent: 30,
            }],
            ..DailyReportMetrics::default()
        };
        let evidence = DailyReportEvidence {
            generated_at_ms: 1,
            sources: Vec::new(),
            warnings: Vec::new(),
            privacy: "aggregate only".into(),
        };
        let narrative = deterministic_narrative("2026-07-28", &metrics, &evidence);
        assert!(narrative.player_markdown.contains("42"));
        assert!(narrative.player_markdown.contains("比奇省"));
        validate_narrative(&narrative).unwrap();
    }

    #[test]
    fn retry_schedule_is_bounded_and_reaches_dead_letter_boundary() {
        assert_eq!(retry_delay_ms(1), 60_000);
        assert_eq!(retry_delay_ms(2), 300_000);
        assert_eq!(retry_delay_ms(99), 86_400_000);
        assert_eq!(DISCORD_MAX_ATTEMPTS, 8);
    }

    #[test]
    fn discord_content_truncation_preserves_utf8() {
        let content = "玛".repeat(2_000);
        let truncated = truncate_utf8(&content, 3_900);
        assert!(truncated.len() <= 3_903);
        assert!(truncated.ends_with('…'));
    }
}
