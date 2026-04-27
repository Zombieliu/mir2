use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use mir2_simulation::{
    ban_account_in_store, deliver_stage5_system_mail, SimulationConfig, Stage5MailDelivery,
    Stage5MailDeliveryReceipt, Stage5MailTargetKind,
};
use postgres::error::SqlState;
use postgres::{Client, NoTls, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    AccountRead,
    AccountBan,
    CharacterRead,
    CharacterKick,
    InventoryRead,
    InventoryGrantItem,
    CurrencyGrant,
    MailSendSystem,
    ContentPublish,
    ContentRollback,
    AuditRead,
    ApprovalManage,
    PermissionManage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operator {
    pub id: String,
    pub email: String,
    pub role: String,
    pub permissions: BTreeSet<Permission>,
}

impl Operator {
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatorPolicyFile {
    pub operators: Vec<OperatorPolicyEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatorPolicyEntry {
    pub id: String,
    pub email: String,
    pub role: String,
    pub token: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetType {
    Account,
    Character,
    ContentBundle,
    World,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminTarget {
    pub target_type: TargetType,
    pub target_id: String,
    pub account_id: Option<String>,
    pub character_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailTargetKind {
    Account,
    Character,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemGrant {
    pub item_id: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdminCommand {
    SendSystemMail {
        target_kind: MailTargetKind,
        target_id: String,
        subject: String,
        body: String,
        attachments: Vec<ItemGrant>,
    },
    GrantItem {
        character_id: String,
        item_id: String,
        count: u32,
    },
    GrantCurrency {
        character_id: String,
        currency: String,
        amount: u64,
    },
    KickPlayer {
        character_id: String,
    },
    BanAccount {
        account_id: String,
        duration_seconds: Option<u64>,
    },
}

impl AdminCommand {
    pub fn required_permission(&self) -> Permission {
        match self {
            AdminCommand::SendSystemMail { .. } => Permission::MailSendSystem,
            AdminCommand::GrantItem { .. } => Permission::InventoryGrantItem,
            AdminCommand::GrantCurrency { .. } => Permission::CurrencyGrant,
            AdminCommand::KickPlayer { .. } => Permission::CharacterKick,
            AdminCommand::BanAccount { .. } => Permission::AccountBan,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminCommandEnvelope {
    pub command_id: String,
    pub operator: Operator,
    pub target: AdminTarget,
    pub reason: String,
    #[serde(default)]
    pub approval_id: Option<String>,
    pub command: AdminCommand,
    pub trace_id: String,
    pub created_at_ms: u64,
}

impl AdminCommandEnvelope {
    pub fn validate(&self) -> Result<(), AdminError> {
        if self.command_id.trim().is_empty() {
            return Err(AdminError::InvalidCommand("command_id is required".into()));
        }
        if self.trace_id.trim().is_empty() {
            return Err(AdminError::InvalidCommand("trace_id is required".into()));
        }
        if self.reason.trim().len() < 8 {
            return Err(AdminError::InvalidCommand(
                "reason must be at least 8 non-whitespace characters".into(),
            ));
        }
        if self.target.target_id.trim().is_empty() {
            return Err(AdminError::InvalidCommand("target_id is required".into()));
        }
        if command_requires_approval(&self.command)
            && self
                .approval_id
                .as_deref()
                .map(str::trim)
                .filter(|approval_id| !approval_id.is_empty())
                .is_none()
        {
            return Err(AdminError::InvalidCommand(
                "approval_id is required for high-risk admin commands".into(),
            ));
        }
        validate_command_payload(&self.command)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    Pending,
    Executing,
    Succeeded,
    Failed,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionResult {
    pub status: CommandStatus,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRecord {
    pub audit_id: String,
    pub command_id: String,
    pub operator_id: String,
    pub operator_email: String,
    pub operator_role_snapshot: String,
    pub permission: Permission,
    pub target: AdminTarget,
    pub reason: String,
    pub status: CommandStatus,
    pub error_code: Option<String>,
    pub trace_id: String,
    pub created_at_ms: u64,
    pub completed_at_ms: Option<u64>,
}

impl AuditRecord {
    fn pending(envelope: &AdminCommandEnvelope, permission: Permission) -> Self {
        Self {
            audit_id: format!("audit-{}", envelope.command_id),
            command_id: envelope.command_id.clone(),
            operator_id: envelope.operator.id.clone(),
            operator_email: envelope.operator.email.clone(),
            operator_role_snapshot: envelope.operator.role.clone(),
            permission,
            target: envelope.target.clone(),
            reason: envelope.reason.clone(),
            status: CommandStatus::Pending,
            error_code: None,
            trace_id: envelope.trace_id.clone(),
            created_at_ms: envelope.created_at_ms,
            completed_at_ms: None,
        }
    }

    fn complete(
        &mut self,
        status: CommandStatus,
        error_code: Option<String>,
        completed_at_ms: u64,
    ) {
        self.status = status;
        self.error_code = error_code;
        self.completed_at_ms = Some(completed_at_ms);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRecord {
    pub approval_id: String,
    pub command_id: String,
    pub command_type: String,
    pub status: ApprovalStatus,
    pub requested_by: String,
    pub requested_reason: String,
    pub decided_by: Option<String>,
    pub decision_reason: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub decided_at_ms: Option<u64>,
}

impl ApprovalRecord {
    pub fn pending(
        approval_id: String,
        command_id: String,
        command_type: String,
        requested_by: String,
        requested_reason: String,
        created_at_ms: u64,
    ) -> Self {
        Self {
            approval_id,
            command_id,
            command_type,
            status: ApprovalStatus::Pending,
            requested_by,
            requested_reason,
            decided_by: None,
            decision_reason: None,
            created_at_ms,
            updated_at_ms: created_at_ms,
            decided_at_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminError {
    PermissionDenied { permission: Permission },
    InvalidCommand(String),
    DuplicateCommand(String),
    ExecutionFailed(String),
    Repository(String),
    UnsupportedCommand(String),
}

impl fmt::Display for AdminError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdminError::PermissionDenied { permission } => {
                write!(f, "operator lacks required permission: {permission:?}")
            }
            AdminError::InvalidCommand(message) => write!(f, "invalid admin command: {message}"),
            AdminError::DuplicateCommand(command_id) => {
                write!(f, "admin command already exists: {command_id}")
            }
            AdminError::ExecutionFailed(message) => {
                write!(f, "admin command execution failed: {message}")
            }
            AdminError::Repository(message) => write!(f, "admin repository error: {message}"),
            AdminError::UnsupportedCommand(message) => {
                write!(f, "unsupported admin command: {message}")
            }
        }
    }
}

impl std::error::Error for AdminError {}

pub trait AdminCommandExecutor {
    fn execute(
        &mut self,
        envelope: &AdminCommandEnvelope,
    ) -> Result<CommandExecutionResult, AdminError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminCommandRecord {
    pub envelope: AdminCommandEnvelope,
    pub status: CommandStatus,
    pub result_message: Option<String>,
    pub error_code: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl AdminCommandRecord {
    fn pending(envelope: AdminCommandEnvelope) -> Self {
        let created_at_ms = envelope.created_at_ms;
        Self {
            envelope,
            status: CommandStatus::Pending,
            result_message: None,
            error_code: None,
            created_at_ms,
            updated_at_ms: created_at_ms,
        }
    }
}

pub trait AdminCommandRepository {
    fn insert_pending(&mut self, envelope: AdminCommandEnvelope) -> Result<(), AdminError>;
    fn mark_completed(
        &mut self,
        command_id: &str,
        status: CommandStatus,
        result_message: Option<String>,
        error_code: Option<String>,
        updated_at_ms: u64,
    ) -> Result<(), AdminError>;
    fn get(&self, command_id: &str) -> Option<AdminCommandRecord>;
    fn list_recent(&self, limit: usize) -> Vec<AdminCommandRecord>;
}

pub trait AuditRepository {
    fn append(&mut self, record: AuditRecord) -> Result<(), AdminError>;
    fn list_recent(&self, limit: usize) -> Vec<AuditRecord>;
}

pub trait ApprovalRepository {
    fn insert_pending(&mut self, record: ApprovalRecord) -> Result<(), AdminError>;
    fn decide(
        &mut self,
        approval_id: &str,
        status: ApprovalStatus,
        decided_by: String,
        decision_reason: Option<String>,
        decided_at_ms: u64,
    ) -> Result<ApprovalRecord, AdminError>;
    fn get(&self, approval_id: &str) -> Option<ApprovalRecord>;
    fn list_recent(&self, limit: usize) -> Vec<ApprovalRecord>;
}

pub const ADMIN_EVENT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminEventEnvelope {
    pub event_id: String,
    pub event_type: String,
    pub schema_version: u16,
    pub command_id: String,
    pub operator_id: String,
    pub status: String,
    pub occurred_at_ms: u64,
    pub payload: Value,
    pub payload_json: String,
}

impl AdminEventEnvelope {
    pub fn new(
        event_id: String,
        event_type: String,
        command_id: String,
        operator_id: String,
        status: String,
        occurred_at_ms: u64,
        payload: Value,
    ) -> Result<Self, AdminError> {
        let payload_json = serde_json::to_string(&payload).map_err(|error| {
            AdminError::Repository(format!("encode event payload failed: {error}"))
        })?;
        Ok(Self {
            event_id,
            event_type,
            schema_version: ADMIN_EVENT_SCHEMA_VERSION,
            command_id,
            operator_id,
            status,
            occurred_at_ms,
            payload,
            payload_json,
        })
    }

    pub fn into_value(self) -> Result<Value, AdminError> {
        serde_json::to_value(self).map_err(|error| {
            AdminError::Repository(format!("encode event envelope failed: {error}"))
        })
    }
}

pub fn admin_outbox_event_payload(message: &AdminOutboxMessage) -> Result<Value, AdminError> {
    if message.payload.get("eventId").is_some()
        && message.payload.get("eventType").is_some()
        && message.payload.get("schemaVersion").is_some()
    {
        return Ok(message.payload.clone());
    }
    let status = message
        .payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or(&message.status)
        .to_string();
    let occurred_at_ms = message
        .payload
        .get("updatedAtMs")
        .and_then(Value::as_u64)
        .unwrap_or(message.updated_at_ms);
    AdminEventEnvelope::new(
        message.outbox_id.clone(),
        message.topic.clone(),
        message
            .command_id
            .clone()
            .unwrap_or_else(|| message.outbox_id.clone()),
        "unknown".into(),
        status,
        occurred_at_ms,
        message.payload.clone(),
    )?
    .into_value()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminOutboxMessage {
    pub outbox_id: String,
    pub command_id: Option<String>,
    pub topic: String,
    pub payload: Value,
    pub status: String,
    pub nats_status: String,
    pub redpanda_status: String,
    pub last_error: Option<String>,
    pub attempts: u32,
    pub next_attempt_at_ms: Option<u64>,
    pub dispatched_at_ms: Option<u64>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

impl AdminOutboxMessage {
    pub fn pending(
        outbox_id: String,
        command_id: String,
        topic: String,
        payload: Value,
        created_at_ms: u64,
    ) -> Self {
        Self {
            outbox_id,
            command_id: Some(command_id),
            topic,
            payload,
            status: "pending".into(),
            nats_status: "pending".into(),
            redpanda_status: "not_configured".into(),
            last_error: None,
            attempts: 0,
            next_attempt_at_ms: None,
            dispatched_at_ms: None,
            created_at_ms,
            updated_at_ms: created_at_ms,
        }
    }
}

pub trait AdminOutboxRepository {
    fn insert_pending(&mut self, message: AdminOutboxMessage) -> Result<(), AdminError>;
    fn record_delivery_attempt(
        &mut self,
        outbox_id: &str,
        nats_status: &str,
        redpanda_status: &str,
        last_error: Option<&str>,
        updated_at_ms: u64,
    ) -> Result<(), AdminError>;
    fn mark_dispatched(&mut self, outbox_id: &str, updated_at_ms: u64) -> Result<(), AdminError>;
    fn mark_retry(
        &mut self,
        outbox_id: &str,
        next_attempt_at_ms: u64,
        updated_at_ms: u64,
    ) -> Result<(), AdminError>;
    fn mark_dead_letter(&mut self, outbox_id: &str, updated_at_ms: u64) -> Result<(), AdminError>;
    fn list_pending(&self, limit: usize) -> Vec<AdminOutboxMessage>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InMemoryCommandStore {
    records: BTreeMap<String, AdminCommandRecord>,
}

impl AdminCommandRepository for InMemoryCommandStore {
    fn insert_pending(&mut self, envelope: AdminCommandEnvelope) -> Result<(), AdminError> {
        if self.records.contains_key(&envelope.command_id) {
            return Err(AdminError::DuplicateCommand(envelope.command_id));
        }
        self.records.insert(
            envelope.command_id.clone(),
            AdminCommandRecord::pending(envelope),
        );
        Ok(())
    }

    fn mark_completed(
        &mut self,
        command_id: &str,
        status: CommandStatus,
        result_message: Option<String>,
        error_code: Option<String>,
        updated_at_ms: u64,
    ) -> Result<(), AdminError> {
        let record = self.records.get_mut(command_id).ok_or_else(|| {
            AdminError::Repository(format!("command record not found: {command_id}"))
        })?;
        record.status = status;
        record.result_message = result_message;
        record.error_code = error_code;
        record.updated_at_ms = updated_at_ms;
        Ok(())
    }

    fn get(&self, command_id: &str) -> Option<AdminCommandRecord> {
        self.records.get(command_id).cloned()
    }

    fn list_recent(&self, limit: usize) -> Vec<AdminCommandRecord> {
        self.records.values().rev().take(limit).cloned().collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InMemoryAuditStore {
    records: Vec<AuditRecord>,
}

impl AuditRepository for InMemoryAuditStore {
    fn append(&mut self, record: AuditRecord) -> Result<(), AdminError> {
        self.records.push(record);
        Ok(())
    }

    fn list_recent(&self, limit: usize) -> Vec<AuditRecord> {
        self.records.iter().rev().take(limit).cloned().collect()
    }
}

impl InMemoryAuditStore {
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InMemoryApprovalStore {
    records: BTreeMap<String, ApprovalRecord>,
}

impl ApprovalRepository for InMemoryApprovalStore {
    fn insert_pending(&mut self, record: ApprovalRecord) -> Result<(), AdminError> {
        if self.records.contains_key(&record.approval_id) {
            return Err(AdminError::Repository(format!(
                "approval record already exists: {}",
                record.approval_id
            )));
        }
        self.records.insert(record.approval_id.clone(), record);
        Ok(())
    }

    fn decide(
        &mut self,
        approval_id: &str,
        status: ApprovalStatus,
        decided_by: String,
        decision_reason: Option<String>,
        decided_at_ms: u64,
    ) -> Result<ApprovalRecord, AdminError> {
        let record = self.records.get_mut(approval_id).ok_or_else(|| {
            AdminError::Repository(format!("approval record not found: {approval_id}"))
        })?;
        record.status = status;
        record.decided_by = Some(decided_by);
        record.decision_reason = decision_reason;
        record.updated_at_ms = decided_at_ms;
        record.decided_at_ms = Some(decided_at_ms);
        Ok(record.clone())
    }

    fn get(&self, approval_id: &str) -> Option<ApprovalRecord> {
        self.records.get(approval_id).cloned()
    }

    fn list_recent(&self, limit: usize) -> Vec<ApprovalRecord> {
        self.records.values().rev().take(limit).cloned().collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InMemoryAdminOutboxStore {
    records: BTreeMap<String, AdminOutboxMessage>,
}

impl AdminOutboxRepository for InMemoryAdminOutboxStore {
    fn insert_pending(&mut self, message: AdminOutboxMessage) -> Result<(), AdminError> {
        if self.records.contains_key(&message.outbox_id) {
            return Err(AdminError::Repository(format!(
                "admin outbox message already exists: {}",
                message.outbox_id
            )));
        }
        self.records.insert(message.outbox_id.clone(), message);
        Ok(())
    }

    fn record_delivery_attempt(
        &mut self,
        outbox_id: &str,
        nats_status: &str,
        redpanda_status: &str,
        last_error: Option<&str>,
        updated_at_ms: u64,
    ) -> Result<(), AdminError> {
        let record = self.records.get_mut(outbox_id).ok_or_else(|| {
            AdminError::Repository(format!("admin outbox message not found: {outbox_id}"))
        })?;
        record.nats_status = nats_status.into();
        record.redpanda_status = redpanda_status.into();
        record.last_error = last_error.map(str::to_string);
        record.updated_at_ms = updated_at_ms;
        Ok(())
    }

    fn mark_dispatched(&mut self, outbox_id: &str, updated_at_ms: u64) -> Result<(), AdminError> {
        let record = self.records.get_mut(outbox_id).ok_or_else(|| {
            AdminError::Repository(format!("admin outbox message not found: {outbox_id}"))
        })?;
        record.status = "dispatched".into();
        record.last_error = None;
        record.dispatched_at_ms = Some(updated_at_ms);
        record.updated_at_ms = updated_at_ms;
        Ok(())
    }

    fn mark_retry(
        &mut self,
        outbox_id: &str,
        next_attempt_at_ms: u64,
        updated_at_ms: u64,
    ) -> Result<(), AdminError> {
        let record = self.records.get_mut(outbox_id).ok_or_else(|| {
            AdminError::Repository(format!("admin outbox message not found: {outbox_id}"))
        })?;
        record.status = "retry".into();
        record.attempts = record.attempts.saturating_add(1);
        record.next_attempt_at_ms = Some(next_attempt_at_ms);
        record.dispatched_at_ms = None;
        record.updated_at_ms = updated_at_ms;
        Ok(())
    }

    fn mark_dead_letter(&mut self, outbox_id: &str, updated_at_ms: u64) -> Result<(), AdminError> {
        let record = self.records.get_mut(outbox_id).ok_or_else(|| {
            AdminError::Repository(format!("admin outbox message not found: {outbox_id}"))
        })?;
        record.status = "dead_letter".into();
        record.attempts = record.attempts.saturating_add(1);
        record.next_attempt_at_ms = None;
        record.dispatched_at_ms = None;
        record.updated_at_ms = updated_at_ms;
        Ok(())
    }

    fn list_pending(&self, limit: usize) -> Vec<AdminOutboxMessage> {
        let now = now_ms();
        self.records
            .values()
            .filter(|record| {
                matches!(record.status.as_str(), "pending" | "retry")
                    && record
                        .next_attempt_at_ms
                        .map(|next| next <= now)
                        .unwrap_or(true)
            })
            .take(limit)
            .cloned()
            .collect()
    }
}

#[derive(Clone)]
pub struct PostgresAdminRepository {
    client: Arc<Mutex<Client>>,
}

impl fmt::Debug for PostgresAdminRepository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresAdminRepository")
            .finish_non_exhaustive()
    }
}

impl PostgresAdminRepository {
    pub fn connect(database_url: &str) -> Result<Self, AdminError> {
        let client = Client::connect(database_url, NoTls)
            .map_err(|error| AdminError::Repository(format!("postgres connect failed: {error}")))?;
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    pub fn ensure_schema(&self) -> Result<(), AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .batch_execute(include_str!(
                "../../../infra/postgres/migrations/0001_core.sql"
            ))
            .map_err(|error| AdminError::Repository(format!("postgres migration failed: {error}")))
    }

    pub fn from_env() -> Result<Option<Self>, AdminError> {
        match env::var("ADMIN_DATABASE_URL") {
            Ok(database_url) if !database_url.trim().is_empty() => {
                let repository = Self::connect(database_url.trim())?;
                repository.ensure_schema()?;
                Ok(Some(repository))
            }
            _ => Ok(None),
        }
    }
}

impl AdminCommandRepository for PostgresAdminRepository {
    fn insert_pending(&mut self, envelope: AdminCommandEnvelope) -> Result<(), AdminError> {
        let envelope_json = serde_json::to_value(&envelope)
            .map_err(|error| AdminError::Repository(format!("encode command failed: {error}")))?;
        let command_type = command_type(&envelope.command);
        let status = status_text(&CommandStatus::Pending);
        let target_type = target_type_text(&envelope.target.target_type);
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .execute(
                "INSERT INTO admin_commands (
                    command_id,
                    command_type,
                    status,
                    operator_id,
                    target_type,
                    target_id,
                    trace_id,
                    envelope_json,
                    created_at_ms,
                    updated_at_ms
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
                &[
                    &envelope.command_id,
                    &command_type,
                    &status,
                    &envelope.operator.id,
                    &target_type,
                    &envelope.target.target_id,
                    &envelope.trace_id,
                    &envelope_json,
                    &(envelope.created_at_ms as i64),
                    &(envelope.created_at_ms as i64),
                ],
            )
            .map_err(map_command_insert_error)?;
        Ok(())
    }

    fn mark_completed(
        &mut self,
        command_id: &str,
        status: CommandStatus,
        result_message: Option<String>,
        error_code: Option<String>,
        updated_at_ms: u64,
    ) -> Result<(), AdminError> {
        let event_type = command_event_type(&status);
        let status = status_text(&status);
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let updated_command = client
            .query_opt(
                "UPDATE admin_commands
                 SET status = $2,
                     result_message = $3,
                     error_code = $4,
                     updated_at_ms = $5,
                     updated_at = now()
                 WHERE command_id = $1
                 RETURNING envelope_json",
                &[
                    &command_id,
                    &status,
                    &result_message,
                    &error_code,
                    &(updated_at_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres command update failed: {error}"))
            })?;
        let Some(updated_command) = updated_command else {
            return Err(AdminError::Repository(format!(
                "command record not found: {command_id}"
            )));
        };
        if let Some(event_type) = event_type {
            let outbox_id = format!("outbox-{command_id}");
            let command_envelope: Value = updated_command.get("envelope_json");
            let operator_id = command_envelope
                .get("operator")
                .and_then(|operator| operator.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let payload = serde_json::json!({
                "resultMessage": result_message,
                "errorCode": error_code,
                "updatedAtMs": updated_at_ms
            });
            let event_payload = AdminEventEnvelope::new(
                outbox_id.clone(),
                event_type.into(),
                command_id.to_string(),
                operator_id,
                status.to_string(),
                updated_at_ms,
                payload,
            )?
            .into_value()?;
            client
                .execute(
                    "INSERT INTO admin_outbox (
                        outbox_id,
                        command_id,
                        topic,
                        payload_json,
                        status,
                        attempts,
                        created_at_ms,
                        updated_at_ms
                    ) VALUES ($1,$2,$3,$4,'pending',0,$5,$6)
                    ON CONFLICT (outbox_id) DO NOTHING",
                    &[
                        &outbox_id,
                        &command_id,
                        &event_type,
                        &event_payload,
                        &(updated_at_ms as i64),
                        &(updated_at_ms as i64),
                    ],
                )
                .map_err(|error| {
                    AdminError::Repository(format!("postgres outbox enqueue failed: {error}"))
                })?;
        }
        Ok(())
    }

    fn get(&self, command_id: &str) -> Option<AdminCommandRecord> {
        let mut client = self.client.lock().ok()?;
        client
            .query_opt(
                "SELECT envelope_json, status, result_message, error_code, created_at_ms, updated_at_ms
                 FROM admin_commands
                 WHERE command_id = $1",
                &[&command_id],
            )
            .ok()
            .flatten()
            .and_then(|row| row_to_command_record(&row).ok())
    }

    fn list_recent(&self, limit: usize) -> Vec<AdminCommandRecord> {
        let mut client = match self.client.lock() {
            Ok(client) => client,
            Err(_) => return Vec::new(),
        };
        client
            .query(
                "SELECT envelope_json, status, result_message, error_code, created_at_ms, updated_at_ms
                 FROM admin_commands
                 ORDER BY updated_at_ms DESC, created_at_ms DESC
                 LIMIT $1",
                &[&(limit.min(i32::MAX as usize) as i64)],
            )
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row_to_command_record(row).ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl AuditRepository for PostgresAdminRepository {
    fn append(&mut self, record: AuditRecord) -> Result<(), AdminError> {
        let target_json = serde_json::to_value(&record.target).map_err(|error| {
            AdminError::Repository(format!("encode audit target failed: {error}"))
        })?;
        let permission = permission_text(&record.permission);
        let status = status_text(&record.status);
        let target_type = target_type_text(&record.target.target_type);
        let completed_at_ms = record.completed_at_ms.map(|value| value as i64);
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .execute(
                "INSERT INTO admin_audit_records (
                    audit_id,
                    command_id,
                    operator_id,
                    operator_email,
                    operator_role_snapshot,
                    permission,
                    target_json,
                    target_type,
                    target_id,
                    reason,
                    status,
                    error_code,
                    trace_id,
                    created_at_ms,
                    completed_at_ms
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
                &[
                    &record.audit_id,
                    &record.command_id,
                    &record.operator_id,
                    &record.operator_email,
                    &record.operator_role_snapshot,
                    &permission,
                    &target_json,
                    &target_type,
                    &record.target.target_id,
                    &record.reason,
                    &status,
                    &record.error_code,
                    &record.trace_id,
                    &(record.created_at_ms as i64),
                    &completed_at_ms,
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres audit insert failed: {error}"))
            })?;
        Ok(())
    }

    fn list_recent(&self, limit: usize) -> Vec<AuditRecord> {
        let mut client = match self.client.lock() {
            Ok(client) => client,
            Err(_) => return Vec::new(),
        };
        client
            .query(
                "SELECT audit_id, command_id, operator_id, operator_email, operator_role_snapshot,
                        permission, target_json, reason, status, error_code, trace_id,
                        created_at_ms, completed_at_ms
                 FROM admin_audit_records
                 ORDER BY created_at_ms DESC
                 LIMIT $1",
                &[&(limit.min(i32::MAX as usize) as i64)],
            )
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row_to_audit_record(row).ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl ApprovalRepository for PostgresAdminRepository {
    fn insert_pending(&mut self, record: ApprovalRecord) -> Result<(), AdminError> {
        let status = approval_status_text(&record.status);
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .execute(
                "INSERT INTO admin_approvals (
                    approval_id,
                    command_id,
                    command_type,
                    status,
                    requested_by,
                    requested_reason,
                    decided_by,
                    decision_reason,
                    created_at_ms,
                    updated_at_ms,
                    decided_at_ms
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
                &[
                    &record.approval_id,
                    &record.command_id,
                    &record.command_type,
                    &status,
                    &record.requested_by,
                    &record.requested_reason,
                    &record.decided_by,
                    &record.decision_reason,
                    &(record.created_at_ms as i64),
                    &(record.updated_at_ms as i64),
                    &record.decided_at_ms.map(|value| value as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres approval insert failed: {error}"))
            })?;
        enqueue_approval_event(&mut client, &record, "admin.approval.requested")?;
        Ok(())
    }

    fn decide(
        &mut self,
        approval_id: &str,
        status: ApprovalStatus,
        decided_by: String,
        decision_reason: Option<String>,
        decided_at_ms: u64,
    ) -> Result<ApprovalRecord, AdminError> {
        let status_text = approval_status_text(&status);
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let row = client
            .query_opt(
                "UPDATE admin_approvals
                 SET status = $2,
                     decided_by = $3,
                     decision_reason = $4,
                     decided_at_ms = $5,
                     updated_at_ms = $5,
                     updated_at = now()
                 WHERE approval_id = $1
                 RETURNING approval_id, command_id, command_type, status, requested_by,
                           requested_reason, decided_by, decision_reason, created_at_ms,
                           updated_at_ms, decided_at_ms",
                &[
                    &approval_id,
                    &status_text,
                    &decided_by,
                    &decision_reason,
                    &(decided_at_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres approval decision failed: {error}"))
            })?;
        let Some(row) = row else {
            return Err(AdminError::Repository(format!(
                "approval record not found: {approval_id}"
            )));
        };
        let record = row_to_approval_record(&row)?;
        let event_type = match record.status {
            ApprovalStatus::Approved => "admin.approval.approved",
            ApprovalStatus::Rejected => "admin.approval.rejected",
            ApprovalStatus::Pending => "admin.approval.requested",
        };
        enqueue_approval_event(&mut client, &record, event_type)?;
        Ok(record)
    }

    fn get(&self, approval_id: &str) -> Option<ApprovalRecord> {
        let mut client = self.client.lock().ok()?;
        client
            .query_opt(
                "SELECT approval_id, command_id, command_type, status, requested_by,
                        requested_reason, decided_by, decision_reason, created_at_ms,
                        updated_at_ms, decided_at_ms
                 FROM admin_approvals
                 WHERE approval_id = $1",
                &[&approval_id],
            )
            .ok()
            .flatten()
            .and_then(|row| row_to_approval_record(&row).ok())
    }

    fn list_recent(&self, limit: usize) -> Vec<ApprovalRecord> {
        let mut client = match self.client.lock() {
            Ok(client) => client,
            Err(_) => return Vec::new(),
        };
        client
            .query(
                "SELECT approval_id, command_id, command_type, status, requested_by,
                        requested_reason, decided_by, decision_reason, created_at_ms,
                        updated_at_ms, decided_at_ms
                 FROM admin_approvals
                 ORDER BY updated_at_ms DESC, created_at_ms DESC
                 LIMIT $1",
                &[&(limit.min(i32::MAX as usize) as i64)],
            )
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row_to_approval_record(row).ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl AdminOutboxRepository for PostgresAdminRepository {
    fn insert_pending(&mut self, message: AdminOutboxMessage) -> Result<(), AdminError> {
        let status = message.status;
        let next_attempt_at_ms = message.next_attempt_at_ms.map(|value| value as i64);
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .execute(
                "INSERT INTO admin_outbox (
                    outbox_id,
                    command_id,
                    topic,
                    payload_json,
                    status,
                    nats_status,
                    redpanda_status,
                    last_error,
                    attempts,
                    next_attempt_at_ms,
                    dispatched_at_ms,
                    created_at_ms,
                    updated_at_ms
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
                &[
                    &message.outbox_id,
                    &message.command_id,
                    &message.topic,
                    &message.payload,
                    &status,
                    &message.nats_status,
                    &message.redpanda_status,
                    &message.last_error,
                    &(message.attempts as i32),
                    &next_attempt_at_ms,
                    &message.dispatched_at_ms.map(|value| value as i64),
                    &(message.created_at_ms as i64),
                    &(message.updated_at_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres outbox insert failed: {error}"))
            })?;
        Ok(())
    }

    fn record_delivery_attempt(
        &mut self,
        outbox_id: &str,
        nats_status: &str,
        redpanda_status: &str,
        last_error: Option<&str>,
        updated_at_ms: u64,
    ) -> Result<(), AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let updated = client
            .execute(
                "UPDATE admin_outbox
                 SET nats_status = $2,
                     redpanda_status = $3,
                     last_error = $4,
                     updated_at_ms = $5,
                     updated_at = now()
                 WHERE outbox_id = $1",
                &[
                    &outbox_id,
                    &nats_status,
                    &redpanda_status,
                    &last_error,
                    &(updated_at_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres outbox delivery update failed: {error}"))
            })?;
        if updated == 0 {
            return Err(AdminError::Repository(format!(
                "admin outbox message not found: {outbox_id}"
            )));
        }
        Ok(())
    }

    fn mark_dispatched(&mut self, outbox_id: &str, updated_at_ms: u64) -> Result<(), AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let updated = client
            .execute(
                "UPDATE admin_outbox
                 SET status = 'dispatched',
                     last_error = NULL,
                     next_attempt_at_ms = NULL,
                     dispatched_at_ms = $2,
                     updated_at_ms = $2,
                     updated_at = now()
                 WHERE outbox_id = $1",
                &[&outbox_id, &(updated_at_ms as i64)],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres outbox update failed: {error}"))
            })?;
        if updated == 0 {
            return Err(AdminError::Repository(format!(
                "admin outbox message not found: {outbox_id}"
            )));
        }
        Ok(())
    }

    fn mark_retry(
        &mut self,
        outbox_id: &str,
        next_attempt_at_ms: u64,
        updated_at_ms: u64,
    ) -> Result<(), AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let updated = client
            .execute(
                "UPDATE admin_outbox
                 SET status = 'retry',
                     attempts = attempts + 1,
                     next_attempt_at_ms = $2,
                     dispatched_at_ms = NULL,
                     updated_at_ms = $3,
                     updated_at = now()
                 WHERE outbox_id = $1",
                &[
                    &outbox_id,
                    &(next_attempt_at_ms as i64),
                    &(updated_at_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres outbox retry update failed: {error}"))
            })?;
        if updated == 0 {
            return Err(AdminError::Repository(format!(
                "admin outbox message not found: {outbox_id}"
            )));
        }
        Ok(())
    }

    fn mark_dead_letter(&mut self, outbox_id: &str, updated_at_ms: u64) -> Result<(), AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let updated = client
            .execute(
                "UPDATE admin_outbox
                 SET status = 'dead_letter',
                     attempts = attempts + 1,
                     next_attempt_at_ms = NULL,
                     dispatched_at_ms = NULL,
                     updated_at_ms = $2,
                     updated_at = now()
                 WHERE outbox_id = $1",
                &[&outbox_id, &(updated_at_ms as i64)],
            )
            .map_err(|error| {
                AdminError::Repository(format!(
                    "postgres outbox dead-letter update failed: {error}"
                ))
            })?;
        if updated == 0 {
            return Err(AdminError::Repository(format!(
                "admin outbox message not found: {outbox_id}"
            )));
        }
        Ok(())
    }

    fn list_pending(&self, limit: usize) -> Vec<AdminOutboxMessage> {
        let mut client = match self.client.lock() {
            Ok(client) => client,
            Err(_) => return Vec::new(),
        };
        let now = now_ms() as i64;
        client
            .query(
                "SELECT outbox_id, command_id, topic, payload_json, status,
                        nats_status, redpanda_status, last_error, attempts,
                        next_attempt_at_ms, dispatched_at_ms, created_at_ms, updated_at_ms
                 FROM admin_outbox
                 WHERE status IN ('pending', 'retry')
                   AND (next_attempt_at_ms IS NULL OR next_attempt_at_ms <= $2)
                 ORDER BY created_at_ms ASC
                 LIMIT $1",
                &[&(limit.min(i32::MAX as usize) as i64), &now],
            )
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row_to_outbox_message(row).ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub enum CommandRepositoryBackend {
    Memory(InMemoryCommandStore),
    Postgres(PostgresAdminRepository),
}

impl Default for CommandRepositoryBackend {
    fn default() -> Self {
        Self::Memory(InMemoryCommandStore::default())
    }
}

impl AdminCommandRepository for CommandRepositoryBackend {
    fn insert_pending(&mut self, envelope: AdminCommandEnvelope) -> Result<(), AdminError> {
        match self {
            Self::Memory(store) => store.insert_pending(envelope),
            Self::Postgres(store) => AdminCommandRepository::insert_pending(store, envelope),
        }
    }

    fn mark_completed(
        &mut self,
        command_id: &str,
        status: CommandStatus,
        result_message: Option<String>,
        error_code: Option<String>,
        updated_at_ms: u64,
    ) -> Result<(), AdminError> {
        match self {
            Self::Memory(store) => store.mark_completed(
                command_id,
                status,
                result_message,
                error_code,
                updated_at_ms,
            ),
            Self::Postgres(store) => store.mark_completed(
                command_id,
                status,
                result_message,
                error_code,
                updated_at_ms,
            ),
        }
    }

    fn get(&self, command_id: &str) -> Option<AdminCommandRecord> {
        match self {
            Self::Memory(store) => store.get(command_id),
            Self::Postgres(store) => AdminCommandRepository::get(store, command_id),
        }
    }

    fn list_recent(&self, limit: usize) -> Vec<AdminCommandRecord> {
        match self {
            Self::Memory(store) => store.list_recent(limit),
            Self::Postgres(store) => AdminCommandRepository::list_recent(store, limit),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AuditRepositoryBackend {
    Memory(InMemoryAuditStore),
    Postgres(PostgresAdminRepository),
}

impl Default for AuditRepositoryBackend {
    fn default() -> Self {
        Self::Memory(InMemoryAuditStore::default())
    }
}

impl AuditRepository for AuditRepositoryBackend {
    fn append(&mut self, record: AuditRecord) -> Result<(), AdminError> {
        match self {
            Self::Memory(store) => store.append(record),
            Self::Postgres(store) => store.append(record),
        }
    }

    fn list_recent(&self, limit: usize) -> Vec<AuditRecord> {
        match self {
            Self::Memory(store) => store.list_recent(limit),
            Self::Postgres(store) => AuditRepository::list_recent(store, limit),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ApprovalRepositoryBackend {
    Memory(InMemoryApprovalStore),
    Postgres(PostgresAdminRepository),
}

impl Default for ApprovalRepositoryBackend {
    fn default() -> Self {
        Self::Memory(InMemoryApprovalStore::default())
    }
}

impl ApprovalRepository for ApprovalRepositoryBackend {
    fn insert_pending(&mut self, record: ApprovalRecord) -> Result<(), AdminError> {
        match self {
            Self::Memory(store) => store.insert_pending(record),
            Self::Postgres(store) => ApprovalRepository::insert_pending(store, record),
        }
    }

    fn decide(
        &mut self,
        approval_id: &str,
        status: ApprovalStatus,
        decided_by: String,
        decision_reason: Option<String>,
        decided_at_ms: u64,
    ) -> Result<ApprovalRecord, AdminError> {
        match self {
            Self::Memory(store) => store.decide(
                approval_id,
                status,
                decided_by,
                decision_reason,
                decided_at_ms,
            ),
            Self::Postgres(store) => store.decide(
                approval_id,
                status,
                decided_by,
                decision_reason,
                decided_at_ms,
            ),
        }
    }

    fn get(&self, approval_id: &str) -> Option<ApprovalRecord> {
        match self {
            Self::Memory(store) => store.get(approval_id),
            Self::Postgres(store) => ApprovalRepository::get(store, approval_id),
        }
    }

    fn list_recent(&self, limit: usize) -> Vec<ApprovalRecord> {
        match self {
            Self::Memory(store) => store.list_recent(limit),
            Self::Postgres(store) => ApprovalRepository::list_recent(store, limit),
        }
    }
}

pub struct AdminControlPlane<
    E,
    C = InMemoryCommandStore,
    A = InMemoryAuditStore,
    P = InMemoryApprovalStore,
> {
    executor: E,
    command_store: C,
    audit_store: A,
    approval_store: P,
}

impl<E> AdminControlPlane<E, InMemoryCommandStore, InMemoryAuditStore, InMemoryApprovalStore>
where
    E: AdminCommandExecutor,
{
    pub fn new(executor: E) -> Self {
        Self::with_repositories(
            executor,
            InMemoryCommandStore::default(),
            InMemoryAuditStore::default(),
            InMemoryApprovalStore::default(),
        )
    }
}

impl<E, C, A, P> AdminControlPlane<E, C, A, P>
where
    E: AdminCommandExecutor,
    C: AdminCommandRepository,
    A: AuditRepository,
    P: ApprovalRepository,
{
    pub fn with_repositories(
        executor: E,
        command_store: C,
        audit_store: A,
        approval_store: P,
    ) -> Self {
        Self {
            executor,
            command_store,
            audit_store,
            approval_store,
        }
    }

    pub fn submit(
        &mut self,
        envelope: AdminCommandEnvelope,
        completed_at_ms: u64,
    ) -> Result<CommandExecutionResult, AdminError> {
        envelope.validate()?;

        let permission = envelope.command.required_permission();
        let mut audit = AuditRecord::pending(&envelope, permission.clone());
        self.command_store.insert_pending(envelope.clone())?;

        if !envelope.operator.has_permission(&permission) {
            audit.complete(
                CommandStatus::Denied,
                Some("permission_denied".into()),
                completed_at_ms,
            );
            self.command_store.mark_completed(
                &envelope.command_id,
                CommandStatus::Denied,
                Some("permission denied".into()),
                Some("permission_denied".into()),
                completed_at_ms,
            )?;
            self.audit_store.append(audit)?;
            return Err(AdminError::PermissionDenied { permission });
        }

        if command_requires_approval(&envelope.command) {
            let approval_id = envelope
                .approval_id
                .as_deref()
                .map(str::trim)
                .filter(|approval_id| !approval_id.is_empty())
                .ok_or_else(|| {
                    AdminError::InvalidCommand(
                        "approval_id is required for high-risk admin commands".into(),
                    )
                })?;
            match self.approval_store.get(approval_id) {
                Some(approval)
                    if approval.command_id == envelope.command_id
                        && approval.status == ApprovalStatus::Approved => {}
                Some(approval) => {
                    audit.complete(
                        CommandStatus::Denied,
                        Some("approval_not_approved".into()),
                        completed_at_ms,
                    );
                    self.command_store.mark_completed(
                        &envelope.command_id,
                        CommandStatus::Denied,
                        Some(format!(
                            "approval {} is {:?}",
                            approval.approval_id, approval.status
                        )),
                        Some("approval_not_approved".into()),
                        completed_at_ms,
                    )?;
                    self.audit_store.append(audit)?;
                    return Err(AdminError::InvalidCommand(
                        "approval_id is not approved for this command".into(),
                    ));
                }
                None => {
                    audit.complete(
                        CommandStatus::Denied,
                        Some("approval_not_found".into()),
                        completed_at_ms,
                    );
                    self.command_store.mark_completed(
                        &envelope.command_id,
                        CommandStatus::Denied,
                        Some("approval not found".into()),
                        Some("approval_not_found".into()),
                        completed_at_ms,
                    )?;
                    self.audit_store.append(audit)?;
                    return Err(AdminError::InvalidCommand(
                        "approval_id was not found".into(),
                    ));
                }
            }
        }

        match self.executor.execute(&envelope) {
            Ok(result) => {
                audit.complete(result.status.clone(), None, completed_at_ms);
                self.command_store.mark_completed(
                    &envelope.command_id,
                    result.status.clone(),
                    Some(result.message.clone()),
                    None,
                    completed_at_ms,
                )?;
                self.audit_store.append(audit)?;
                Ok(result)
            }
            Err(error) => {
                let code = error_code(&error).to_string();
                audit.complete(CommandStatus::Failed, Some(code.clone()), completed_at_ms);
                self.command_store.mark_completed(
                    &envelope.command_id,
                    CommandStatus::Failed,
                    Some(error.to_string()),
                    Some(code),
                    completed_at_ms,
                )?;
                self.audit_store.append(audit)?;
                Err(error)
            }
        }
    }

    pub fn command_records(&self, limit: usize) -> Vec<AdminCommandRecord> {
        self.command_store.list_recent(limit)
    }

    pub fn command_record(&self, command_id: &str) -> Option<AdminCommandRecord> {
        self.command_store.get(command_id)
    }

    pub fn audit_records(&self, limit: usize) -> Vec<AuditRecord> {
        self.audit_store.list_recent(limit)
    }

    pub fn create_approval(
        &mut self,
        record: ApprovalRecord,
    ) -> Result<ApprovalRecord, AdminError> {
        self.approval_store.insert_pending(record.clone())?;
        Ok(record)
    }

    pub fn decide_approval(
        &mut self,
        approval_id: &str,
        status: ApprovalStatus,
        decided_by: String,
        decision_reason: Option<String>,
        decided_at_ms: u64,
    ) -> Result<ApprovalRecord, AdminError> {
        self.approval_store.decide(
            approval_id,
            status,
            decided_by,
            decision_reason,
            decided_at_ms,
        )
    }

    pub fn approval_records(&self, limit: usize) -> Vec<ApprovalRecord> {
        self.approval_store.list_recent(limit)
    }

    pub fn approval_record(&self, approval_id: &str) -> Option<ApprovalRecord> {
        self.approval_store.get(approval_id)
    }

    pub fn executor(&self) -> &E {
        &self.executor
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMailRequest {
    pub command_id: String,
    pub target_kind: MailTargetKind,
    pub target_id: String,
    pub subject: String,
    pub body: String,
    pub attachments: Vec<ItemGrant>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMailReceipt {
    pub outbox_id: String,
    pub target_kind: MailTargetKind,
    pub target_id: String,
    pub attachment_count: usize,
    pub accepted_at_ms: u64,
    pub delivery_mode: String,
    pub delivered_count: usize,
    pub mail_ids: Vec<u32>,
}

pub trait SystemMailDomain {
    fn enqueue_system_mail(
        &mut self,
        request: SystemMailRequest,
        accepted_at_ms: u64,
    ) -> Result<SystemMailReceipt, AdminError>;

    fn kick_player(
        &mut self,
        character_id: &str,
        _accepted_at_ms: u64,
    ) -> Result<String, AdminError> {
        Err(AdminError::UnsupportedCommand(format!(
            "kick_player is not supported for {character_id}"
        )))
    }

    fn ban_account(
        &mut self,
        account_id: &str,
        _duration_seconds: Option<u64>,
        _reason: &str,
        _accepted_at_ms: u64,
    ) -> Result<String, AdminError> {
        Err(AdminError::UnsupportedCommand(format!(
            "ban_account is not supported for {account_id}"
        )))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InMemorySystemMailOutbox {
    receipts: Vec<SystemMailReceipt>,
}

impl InMemorySystemMailOutbox {
    pub fn receipts(&self) -> &[SystemMailReceipt] {
        &self.receipts
    }
}

impl SystemMailDomain for InMemorySystemMailOutbox {
    fn enqueue_system_mail(
        &mut self,
        request: SystemMailRequest,
        accepted_at_ms: u64,
    ) -> Result<SystemMailReceipt, AdminError> {
        let receipt = SystemMailReceipt {
            outbox_id: format!("mail-{}", request.command_id),
            target_kind: request.target_kind,
            target_id: request.target_id,
            attachment_count: request.attachments.len(),
            accepted_at_ms,
            delivery_mode: "memory_outbox".to_string(),
            delivered_count: 0,
            mail_ids: Vec::new(),
        };
        self.receipts.push(receipt.clone());
        Ok(receipt)
    }

    fn kick_player(
        &mut self,
        character_id: &str,
        _accepted_at_ms: u64,
    ) -> Result<String, AdminError> {
        Ok(format!("memory kick accepted for {character_id}"))
    }

    fn ban_account(
        &mut self,
        account_id: &str,
        duration_seconds: Option<u64>,
        reason: &str,
        _accepted_at_ms: u64,
    ) -> Result<String, AdminError> {
        Ok(format!(
            "memory ban accepted for {account_id} duration={duration_seconds:?} reason={reason}"
        ))
    }
}

#[derive(Debug, Clone)]
pub struct AccountStoreSystemMailDomain {
    gateway_mail_url: String,
    gateway_kick_url: String,
    fallback_config: SimulationConfig,
    receipts: Vec<SystemMailReceipt>,
}

impl AccountStoreSystemMailDomain {
    pub fn from_env() -> Self {
        let gateway_mail_url = env::var("ADMIN_GATEWAY_MAIL_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:7110/admin/system-mail".to_string());
        let gateway_kick_url = env::var("ADMIN_GATEWAY_KICK_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:7110/admin/kick-player".to_string());
        let database_backend = env::var("MIR2_ACCOUNT_STORE_BACKEND").unwrap_or_default();
        let fallback_config = if database_backend.eq_ignore_ascii_case("postgres") {
            let database_url = env::var("MIR2_ACCOUNT_STORE_DATABASE_URL")
                .expect("MIR2_ACCOUNT_STORE_DATABASE_URL is required for postgres account store");
            SimulationConfig::default().with_postgres_account_store(database_url)
        } else {
            let account_store_path = env::var("ADMIN_ACCOUNT_STORE_PATH")
                .or_else(|_| env::var("MIR2_ACCOUNT_STORE_PATH"))
                .unwrap_or_else(|_| ".mir2-data/accounts.json".to_string());
            let mut fallback_config = SimulationConfig::default()
                .with_account_store_path(PathBuf::from(account_store_path));
            if let Ok(database_url) = env::var("MIR2_ACCOUNT_STORE_DATABASE_URL") {
                fallback_config = fallback_config.with_account_store_database_url(database_url);
            }
            fallback_config
        };
        Self {
            gateway_mail_url,
            gateway_kick_url,
            fallback_config,
            receipts: Vec::new(),
        }
    }

    pub fn with_gateway_and_fallback(
        gateway_mail_url: String,
        fallback_config: SimulationConfig,
    ) -> Self {
        Self {
            gateway_mail_url,
            gateway_kick_url: "http://127.0.0.1:7110/admin/kick-player".to_string(),
            fallback_config,
            receipts: Vec::new(),
        }
    }

    pub fn receipts(&self) -> &[SystemMailReceipt] {
        &self.receipts
    }
}

impl SystemMailDomain for AccountStoreSystemMailDomain {
    fn enqueue_system_mail(
        &mut self,
        request: SystemMailRequest,
        accepted_at_ms: u64,
    ) -> Result<SystemMailReceipt, AdminError> {
        let delivery = delivery_from_system_mail(&request);
        let (delivery_mode, delivered) =
            match post_system_mail_to_gateway(&self.gateway_mail_url, &delivery) {
                Ok(receipt) => ("gateway_live".to_string(), receipt),
                Err(_) => (
                    "account_store_fallback".to_string(),
                    deliver_stage5_system_mail(&self.fallback_config, delivery)
                        .map_err(AdminError::ExecutionFailed)?,
                ),
            };
        let receipt = SystemMailReceipt {
            outbox_id: format!("mail-{}", request.command_id),
            target_kind: request.target_kind,
            target_id: request.target_id,
            attachment_count: request.attachments.len(),
            accepted_at_ms,
            delivery_mode,
            delivered_count: delivered.delivered_count,
            mail_ids: delivered.mail_ids,
        };
        self.receipts.push(receipt.clone());
        Ok(receipt)
    }

    fn kick_player(
        &mut self,
        character_id: &str,
        _accepted_at_ms: u64,
    ) -> Result<String, AdminError> {
        let body = serde_json::json!({ "characterId": character_id });
        let receipt = post_json_to_gateway(&self.gateway_kick_url, &body)
            .map_err(AdminError::ExecutionFailed)?;
        Ok(format!("kick request accepted by gateway: {receipt}"))
    }

    fn ban_account(
        &mut self,
        account_id: &str,
        duration_seconds: Option<u64>,
        reason: &str,
        _accepted_at_ms: u64,
    ) -> Result<String, AdminError> {
        let receipt =
            ban_account_in_store(&self.fallback_config, account_id, duration_seconds, reason)
                .map_err(AdminError::ExecutionFailed)?;
        Ok(format!(
            "account {} banned until {:?}",
            receipt.account_id, receipt.ban_until_ms
        ))
    }
}

fn delivery_from_system_mail(request: &SystemMailRequest) -> Stage5MailDelivery {
    let mut gold = 0u32;
    let mut items = Vec::new();
    for attachment in &request.attachments {
        if attachment.item_id.eq_ignore_ascii_case("gold") {
            gold = gold.saturating_add(attachment.count);
        } else {
            for _ in 0..attachment.count {
                items.push(attachment.item_id.clone());
            }
        }
    }
    Stage5MailDelivery {
        target_kind: match request.target_kind {
            MailTargetKind::Account => Stage5MailTargetKind::Account,
            MailTargetKind::Character => Stage5MailTargetKind::Character,
            MailTargetKind::Global => Stage5MailTargetKind::Global,
        },
        target_id: request.target_id.clone(),
        from: "GM System".to_string(),
        subject: request.subject.clone(),
        body: request.body.clone(),
        gold,
        items,
    }
}

fn post_system_mail_to_gateway(
    gateway_mail_url: &str,
    delivery: &Stage5MailDelivery,
) -> Result<Stage5MailDeliveryReceipt, String> {
    let response_body = post_json_to_gateway(gateway_mail_url, delivery)?;
    serde_json::from_str::<Stage5MailDeliveryReceipt>(&response_body)
        .map_err(|error| format!("gateway mail response decode failed: {error}"))
}

fn post_json_to_gateway<T>(gateway_url: &str, payload: &T) -> Result<String, String>
where
    T: Serialize,
{
    let (host, path) = parse_http_url(gateway_url)?;
    let body = serde_json::to_string(payload)
        .map_err(|error| format!("gateway request encode failed: {error}"))?;
    let mut stream = TcpStream::connect(&host)
        .map_err(|error| format!("gateway endpoint unavailable: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| format!("gateway read timeout setup failed: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| format!("gateway write timeout setup failed: {error}"))?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .map_err(|error| format!("gateway request write failed: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("gateway response read failed: {error}"))?;
    let (head, response_body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "gateway response was not valid HTTP".to_string())?;
    let status_line = head.lines().next().unwrap_or_default();
    if !status_line.contains(" 2") {
        return Err(format!(
            "gateway endpoint rejected request: {status_line} {response_body}"
        ));
    }
    Ok(response_body.to_string())
}

fn parse_http_url(url: &str) -> Result<(String, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "gateway mail url must start with http://".to_string())?;
    let (host, path) = rest
        .split_once('/')
        .map(|(host, path)| (host, format!("/{path}")))
        .unwrap_or((rest, "/".to_string()));
    if host.trim().is_empty() {
        return Err("gateway mail url host is required".to_string());
    }
    Ok((host.to_string(), path))
}

pub struct SystemMailExecutor<D> {
    domain: D,
}

impl<D> SystemMailExecutor<D> {
    pub fn new(domain: D) -> Self {
        Self { domain }
    }

    pub fn domain(&self) -> &D {
        &self.domain
    }
}

impl<D> AdminCommandExecutor for SystemMailExecutor<D>
where
    D: SystemMailDomain,
{
    fn execute(
        &mut self,
        envelope: &AdminCommandEnvelope,
    ) -> Result<CommandExecutionResult, AdminError> {
        match &envelope.command {
            AdminCommand::SendSystemMail {
                target_kind,
                target_id,
                subject,
                body,
                attachments,
            } => {
                let receipt = self.domain.enqueue_system_mail(
                    SystemMailRequest {
                        command_id: envelope.command_id.clone(),
                        target_kind: target_kind.clone(),
                        target_id: target_id.clone(),
                        subject: subject.clone(),
                        body: body.clone(),
                        attachments: attachments.clone(),
                        reason: envelope.reason.clone(),
                    },
                    envelope.created_at_ms,
                )?;
                Ok(CommandExecutionResult {
                    status: CommandStatus::Succeeded,
                    message: format!("system mail queued as {}", receipt.outbox_id),
                })
            }
            AdminCommand::GrantItem {
                character_id,
                item_id,
                count,
            } => {
                let receipt = self.domain.enqueue_system_mail(
                    SystemMailRequest {
                        command_id: envelope.command_id.clone(),
                        target_kind: MailTargetKind::Character,
                        target_id: character_id.clone(),
                        subject: "GM Item Grant".to_string(),
                        body: format!("Operator {} granted item {item_id}.", envelope.operator.id),
                        attachments: vec![ItemGrant {
                            item_id: item_id.clone(),
                            count: *count,
                        }],
                        reason: envelope.reason.clone(),
                    },
                    envelope.created_at_ms,
                )?;
                Ok(CommandExecutionResult {
                    status: CommandStatus::Succeeded,
                    message: format!("item grant queued as {}", receipt.outbox_id),
                })
            }
            AdminCommand::GrantCurrency {
                character_id,
                currency,
                amount,
            } if currency.eq_ignore_ascii_case("gold") => {
                let count = u32::try_from(*amount).map_err(|_| {
                    AdminError::InvalidCommand("gold grant amount exceeds u32".into())
                })?;
                let receipt = self.domain.enqueue_system_mail(
                    SystemMailRequest {
                        command_id: envelope.command_id.clone(),
                        target_kind: MailTargetKind::Character,
                        target_id: character_id.clone(),
                        subject: "GM Currency Grant".to_string(),
                        body: format!("Operator {} granted {amount} gold.", envelope.operator.id),
                        attachments: vec![ItemGrant {
                            item_id: "gold".to_string(),
                            count,
                        }],
                        reason: envelope.reason.clone(),
                    },
                    envelope.created_at_ms,
                )?;
                Ok(CommandExecutionResult {
                    status: CommandStatus::Succeeded,
                    message: format!("currency grant queued as {}", receipt.outbox_id),
                })
            }
            AdminCommand::KickPlayer { character_id } => {
                let message = self
                    .domain
                    .kick_player(character_id, envelope.created_at_ms)?;
                Ok(CommandExecutionResult {
                    status: CommandStatus::Succeeded,
                    message,
                })
            }
            AdminCommand::BanAccount {
                account_id,
                duration_seconds,
            } => {
                let message = self.domain.ban_account(
                    account_id,
                    *duration_seconds,
                    &envelope.reason,
                    envelope.created_at_ms,
                )?;
                Ok(CommandExecutionResult {
                    status: CommandStatus::Succeeded,
                    message,
                })
            }
            other => Err(AdminError::UnsupportedCommand(format!("{other:?}"))),
        }
    }
}

type HttpControlPlane = AdminControlPlane<
    SystemMailExecutor<AccountStoreSystemMailDomain>,
    CommandRepositoryBackend,
    AuditRepositoryBackend,
    ApprovalRepositoryBackend,
>;

#[derive(Clone)]
pub struct AdminApiState {
    control_plane: Arc<Mutex<HttpControlPlane>>,
}

impl Default for AdminApiState {
    fn default() -> Self {
        Self::from_env().expect("admin api state should initialize")
    }
}

impl AdminApiState {
    pub fn from_env() -> Result<Self, AdminError> {
        let (command_store, audit_store, approval_store) =
            match PostgresAdminRepository::from_env()? {
                Some(repository) => (
                    CommandRepositoryBackend::Postgres(repository.clone()),
                    AuditRepositoryBackend::Postgres(repository.clone()),
                    ApprovalRepositoryBackend::Postgres(repository),
                ),
                None => (
                    CommandRepositoryBackend::default(),
                    AuditRepositoryBackend::default(),
                    ApprovalRepositoryBackend::default(),
                ),
            };
        Ok(Self {
            control_plane: Arc::new(Mutex::new(AdminControlPlane::with_repositories(
                SystemMailExecutor::new(AccountStoreSystemMailDomain::from_env()),
                command_store,
                audit_store,
                approval_store,
            ))),
        })
    }
}

pub fn admin_router() -> Router {
    admin_router_with_state(AdminApiState::default())
}

pub fn admin_router_with_state(state: AdminApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/admin/commands", get(list_commands))
        .route(
            "/admin/commands/:command_id/status",
            get(get_command_status),
        )
        .route("/admin/audit", get(list_audit))
        .route(
            "/admin/approvals",
            get(list_approvals).post(create_approval),
        )
        .route(
            "/admin/approvals/:approval_id/approve",
            post(approve_approval),
        )
        .route(
            "/admin/approvals/:approval_id/reject",
            post(reject_approval),
        )
        .route("/admin/events", get(list_admin_events))
        .route("/admin/timeline", get(list_admin_timeline))
        .route("/admin/system-mail/outbox", get(list_system_mail_outbox))
        .route("/admin/commands/send-system-mail", post(submit_system_mail))
        .route("/admin/commands/grant-item", post(submit_grant_item))
        .route(
            "/admin/commands/grant-currency",
            post(submit_grant_currency),
        )
        .route("/admin/commands/kick-player", post(submit_kick_player))
        .route("/admin/commands/ban-account", post(submit_ban_account))
        .with_state(state)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub ok: bool,
    pub service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitSystemMailRequest {
    pub command_id: Option<String>,
    pub trace_id: Option<String>,
    pub approval_id: Option<String>,
    pub target_kind: MailTargetKind,
    pub target_id: String,
    pub account_id: Option<String>,
    pub character_id: Option<String>,
    pub subject: String,
    pub body: String,
    pub attachments: Vec<ItemGrant>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitGrantItemRequest {
    pub command_id: Option<String>,
    pub trace_id: Option<String>,
    pub approval_id: Option<String>,
    pub character_id: String,
    pub item_id: String,
    pub count: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitGrantCurrencyRequest {
    pub command_id: Option<String>,
    pub trace_id: Option<String>,
    pub approval_id: Option<String>,
    pub character_id: String,
    pub currency: String,
    pub amount: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitKickPlayerRequest {
    pub command_id: Option<String>,
    pub trace_id: Option<String>,
    pub character_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitBanAccountRequest {
    pub command_id: Option<String>,
    pub trace_id: Option<String>,
    pub approval_id: Option<String>,
    pub account_id: String,
    pub duration_seconds: Option<u64>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApprovalRequest {
    pub approval_id: Option<String>,
    pub command_id: String,
    pub command_type: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalDecisionRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitCommandResponse {
    pub command_id: String,
    pub result: CommandExecutionResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminEventRecord {
    pub event_id: String,
    pub event_type: String,
    pub command_id: String,
    pub operator_id: String,
    pub status: String,
    pub occurred_at_ms: u64,
    pub payload_json: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminEventQuery {
    pub limit: Option<usize>,
    pub command_id: Option<String>,
    pub event_type: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminEventsResponse {
    pub degraded: bool,
    pub error: Option<String>,
    pub records: Vec<AdminEventRecord>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminTimelineQuery {
    pub limit: Option<usize>,
    pub command_id: Option<String>,
    pub target_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminTimelineItem {
    pub source: String,
    pub record_id: String,
    pub command_id: Option<String>,
    pub target_id: Option<String>,
    pub event_type: String,
    pub status: String,
    pub actor_id: Option<String>,
    pub occurred_at_ms: u64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminTimelineResponse {
    pub degraded: bool,
    pub error: Option<String>,
    pub records: Vec<AdminTimelineItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    error: String,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "mir2-admin-api".into(),
    })
}

async fn list_commands(
    State(state): State<AdminApiState>,
) -> Result<Json<Vec<AdminCommandRecord>>, ApiError> {
    let records = tokio::task::spawn_blocking(move || {
        let control = state.control_plane.lock().map_err(lock_error)?;
        Ok::<_, ApiError>(control.command_records(100))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(records))
}

async fn get_command_status(
    State(state): State<AdminApiState>,
    Path(command_id): Path<String>,
) -> Result<Json<AdminCommandRecord>, ApiError> {
    let command_id = command_id.trim().to_string();
    if command_id.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "command_id is required".into(),
        });
    }
    let lookup_id = command_id.clone();
    let record = tokio::task::spawn_blocking(move || {
        let control = state.control_plane.lock().map_err(lock_error)?;
        Ok::<_, ApiError>(control.command_record(&lookup_id))
    })
    .await
    .map_err(join_error)??;
    let record = record.ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: format!("command record not found: {command_id}"),
    })?;
    Ok(Json(record))
}

async fn list_audit(
    State(state): State<AdminApiState>,
) -> Result<Json<Vec<AuditRecord>>, ApiError> {
    let records = tokio::task::spawn_blocking(move || {
        let control = state.control_plane.lock().map_err(lock_error)?;
        Ok::<_, ApiError>(control.audit_records(100))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(records))
}

async fn list_approvals(
    State(state): State<AdminApiState>,
) -> Result<Json<Vec<ApprovalRecord>>, ApiError> {
    let records = tokio::task::spawn_blocking(move || {
        let control = state.control_plane.lock().map_err(lock_error)?;
        Ok::<_, ApiError>(control.approval_records(100))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(records))
}

async fn create_approval(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<CreateApprovalRequest>,
) -> Result<Json<ApprovalRecord>, ApiError> {
    let now = now_ms();
    let operator = operator_from_headers(&headers)?;
    require_approval_manager(&operator)?;
    if request.command_id.trim().is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "command_id is required".into(),
        });
    }
    if request.command_type.trim().is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "command_type is required".into(),
        });
    }
    if request.reason.trim().len() < 8 {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "reason must be at least 8 non-whitespace characters".into(),
        });
    }
    let approval_id = request
        .approval_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("approval-{now}"));
    let record = ApprovalRecord::pending(
        approval_id,
        request.command_id,
        request.command_type,
        operator.id,
        request.reason,
        now,
    );
    let record = tokio::task::spawn_blocking(move || {
        let mut control = state.control_plane.lock().map_err(lock_error)?;
        control.create_approval(record).map_err(ApiError::from)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(record))
}

async fn approve_approval(
    State(state): State<AdminApiState>,
    Path(approval_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ApprovalDecisionRequest>,
) -> Result<Json<ApprovalRecord>, ApiError> {
    decide_approval_handler(
        state,
        approval_id,
        headers,
        request,
        ApprovalStatus::Approved,
    )
    .await
}

async fn reject_approval(
    State(state): State<AdminApiState>,
    Path(approval_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ApprovalDecisionRequest>,
) -> Result<Json<ApprovalRecord>, ApiError> {
    decide_approval_handler(
        state,
        approval_id,
        headers,
        request,
        ApprovalStatus::Rejected,
    )
    .await
}

async fn decide_approval_handler(
    state: AdminApiState,
    approval_id: String,
    headers: HeaderMap,
    request: ApprovalDecisionRequest,
    status: ApprovalStatus,
) -> Result<Json<ApprovalRecord>, ApiError> {
    let now = now_ms();
    let operator = operator_from_headers(&headers)?;
    require_approval_manager(&operator)?;
    let operator_id = operator.id.clone();
    let allow_self_approval = env_flag("ADMIN_APPROVAL_ALLOW_SELF");
    let record = tokio::task::spawn_blocking(move || {
        let mut control = state.control_plane.lock().map_err(lock_error)?;
        if status == ApprovalStatus::Approved && !allow_self_approval {
            let existing = control
                .approval_record(&approval_id)
                .ok_or_else(|| ApiError {
                    status: StatusCode::NOT_FOUND,
                    message: format!("approval record not found: {approval_id}"),
                })?;
            validate_approval_decision(&existing, &operator_id, &status, allow_self_approval)?;
        }
        control
            .decide_approval(&approval_id, status, operator_id, request.reason, now)
            .map_err(ApiError::from)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(record))
}

async fn list_admin_events(
    Query(query): Query<AdminEventQuery>,
) -> Result<Json<AdminEventsResponse>, ApiError> {
    let records = tokio::task::spawn_blocking(move || fetch_clickhouse_admin_events(&query))
        .await
        .map_err(join_error)?;
    match records {
        Ok(records) => Ok(Json(AdminEventsResponse {
            degraded: false,
            error: None,
            records,
        })),
        Err(error) => Ok(Json(AdminEventsResponse {
            degraded: true,
            error: Some(error.message),
            records: Vec::new(),
        })),
    }
}

async fn list_admin_timeline(
    State(state): State<AdminApiState>,
    Query(query): Query<AdminTimelineQuery>,
) -> Result<Json<AdminTimelineResponse>, ApiError> {
    let response = tokio::task::spawn_blocking(move || build_admin_timeline(state, query))
        .await
        .map_err(join_error)??;
    Ok(Json(response))
}

async fn list_system_mail_outbox(
    State(state): State<AdminApiState>,
) -> Result<Json<Vec<SystemMailReceipt>>, ApiError> {
    let receipts = tokio::task::spawn_blocking(move || {
        let control = state.control_plane.lock().map_err(lock_error)?;
        Ok::<_, ApiError>(control.executor().domain().receipts().to_vec())
    })
    .await
    .map_err(join_error)??;
    Ok(Json(receipts))
}

async fn submit_system_mail(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<SubmitSystemMailRequest>,
) -> Result<Json<SubmitCommandResponse>, ApiError> {
    let now = now_ms();
    let operator = operator_from_headers(&headers)?;
    let command_id = request
        .command_id
        .unwrap_or_else(|| format!("cmd-mail-{now}"));
    let trace_id = request
        .trace_id
        .unwrap_or_else(|| format!("trace-admin-{now}"));
    let target = AdminTarget {
        target_type: match request.target_kind {
            MailTargetKind::Account => TargetType::Account,
            MailTargetKind::Character => TargetType::Character,
            MailTargetKind::Global => TargetType::World,
        },
        target_id: request.target_id.clone(),
        account_id: request.account_id.clone(),
        character_id: request.character_id.clone(),
    };
    let envelope = AdminCommandEnvelope {
        command_id: command_id.clone(),
        operator,
        target,
        reason: request.reason,
        approval_id: request.approval_id,
        command: AdminCommand::SendSystemMail {
            target_kind: request.target_kind,
            target_id: request.target_id,
            subject: request.subject,
            body: request.body,
            attachments: request.attachments,
        },
        trace_id,
        created_at_ms: now,
    };

    let result = tokio::task::spawn_blocking(move || {
        let mut control = state.control_plane.lock().map_err(lock_error)?;
        control.submit(envelope, now_ms()).map_err(ApiError::from)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(SubmitCommandResponse { command_id, result }))
}

async fn submit_grant_item(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<SubmitGrantItemRequest>,
) -> Result<Json<SubmitCommandResponse>, ApiError> {
    let now = now_ms();
    let command_id = request
        .command_id
        .clone()
        .unwrap_or_else(|| format!("cmd-grant-item-{now}"));
    submit_admin_http_command(
        state,
        headers,
        command_id,
        request
            .trace_id
            .unwrap_or_else(|| format!("trace-admin-{now}")),
        request.approval_id,
        AdminTarget {
            target_type: TargetType::Character,
            target_id: request.character_id.clone(),
            account_id: None,
            character_id: Some(request.character_id.clone()),
        },
        request.reason,
        AdminCommand::GrantItem {
            character_id: request.character_id,
            item_id: request.item_id,
            count: request.count,
        },
        now,
    )
    .await
}

async fn submit_grant_currency(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<SubmitGrantCurrencyRequest>,
) -> Result<Json<SubmitCommandResponse>, ApiError> {
    let now = now_ms();
    let command_id = request
        .command_id
        .clone()
        .unwrap_or_else(|| format!("cmd-grant-currency-{now}"));
    submit_admin_http_command(
        state,
        headers,
        command_id,
        request
            .trace_id
            .unwrap_or_else(|| format!("trace-admin-{now}")),
        request.approval_id,
        AdminTarget {
            target_type: TargetType::Character,
            target_id: request.character_id.clone(),
            account_id: None,
            character_id: Some(request.character_id.clone()),
        },
        request.reason,
        AdminCommand::GrantCurrency {
            character_id: request.character_id,
            currency: request.currency,
            amount: request.amount,
        },
        now,
    )
    .await
}

async fn submit_kick_player(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<SubmitKickPlayerRequest>,
) -> Result<Json<SubmitCommandResponse>, ApiError> {
    let now = now_ms();
    let command_id = request
        .command_id
        .clone()
        .unwrap_or_else(|| format!("cmd-kick-player-{now}"));
    submit_admin_http_command(
        state,
        headers,
        command_id,
        request
            .trace_id
            .unwrap_or_else(|| format!("trace-admin-{now}")),
        None,
        AdminTarget {
            target_type: TargetType::Character,
            target_id: request.character_id.clone(),
            account_id: None,
            character_id: Some(request.character_id.clone()),
        },
        request.reason,
        AdminCommand::KickPlayer {
            character_id: request.character_id,
        },
        now,
    )
    .await
}

async fn submit_ban_account(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<SubmitBanAccountRequest>,
) -> Result<Json<SubmitCommandResponse>, ApiError> {
    let now = now_ms();
    let command_id = request
        .command_id
        .clone()
        .unwrap_or_else(|| format!("cmd-ban-account-{now}"));
    submit_admin_http_command(
        state,
        headers,
        command_id,
        request
            .trace_id
            .unwrap_or_else(|| format!("trace-admin-{now}")),
        request.approval_id,
        AdminTarget {
            target_type: TargetType::Account,
            target_id: request.account_id.clone(),
            account_id: Some(request.account_id.clone()),
            character_id: None,
        },
        request.reason,
        AdminCommand::BanAccount {
            account_id: request.account_id,
            duration_seconds: request.duration_seconds,
        },
        now,
    )
    .await
}

async fn submit_admin_http_command(
    state: AdminApiState,
    headers: HeaderMap,
    command_id: String,
    trace_id: String,
    approval_id: Option<String>,
    target: AdminTarget,
    reason: String,
    command: AdminCommand,
    created_at_ms: u64,
) -> Result<Json<SubmitCommandResponse>, ApiError> {
    let operator = operator_from_headers(&headers)?;
    let envelope = AdminCommandEnvelope {
        command_id: command_id.clone(),
        operator,
        target,
        reason,
        approval_id,
        command,
        trace_id,
        created_at_ms,
    };
    let result = tokio::task::spawn_blocking(move || {
        let mut control = state.control_plane.lock().map_err(lock_error)?;
        control.submit(envelope, now_ms()).map_err(ApiError::from)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(SubmitCommandResponse { command_id, result }))
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl From<AdminError> for ApiError {
    fn from(error: AdminError) -> Self {
        let status = match error {
            AdminError::PermissionDenied { .. } => StatusCode::FORBIDDEN,
            AdminError::InvalidCommand(_) => StatusCode::BAD_REQUEST,
            AdminError::DuplicateCommand(_) => StatusCode::CONFLICT,
            AdminError::UnsupportedCommand(_) => StatusCode::NOT_IMPLEMENTED,
            AdminError::ExecutionFailed(_) | AdminError::Repository(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        Self {
            status,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

fn lock_error<T>(_: std::sync::PoisonError<T>) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "admin control plane lock poisoned".into(),
    }
}

fn join_error(error: tokio::task::JoinError) -> ApiError {
    ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("admin blocking task failed: {error}"),
    }
}

fn operator_from_headers(headers: &HeaderMap) -> Result<Operator, ApiError> {
    if let Some(operator) = operator_from_policy_file(headers)? {
        return Ok(operator);
    }
    validate_operator_token(headers)?;
    let id = required_header(headers, "x-operator-id")?;
    let email = required_header(headers, "x-operator-email")?;
    let role = required_header(headers, "x-operator-role")?;
    let permissions = required_header(headers, "x-operator-permissions")?
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(parse_permission)
        .collect::<Result<BTreeSet<_>, _>>()?;

    Ok(Operator {
        id,
        email,
        role,
        permissions,
    })
}

fn operator_from_policy_file(headers: &HeaderMap) -> Result<Option<Operator>, ApiError> {
    let Ok(policy_path) = env::var("ADMIN_OPERATOR_POLICY_PATH") else {
        return Ok(None);
    };
    let policy_path = policy_path.trim();
    if policy_path.is_empty() {
        return Ok(None);
    }
    let policy_json = fs::read_to_string(policy_path).map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to read operator policy file: {error}"),
    })?;
    let token = bearer_token(headers)?;
    operator_from_policy_json(&policy_json, &token).map(Some)
}

fn operator_from_policy_json(policy_json: &str, token: &str) -> Result<Operator, ApiError> {
    let policy =
        serde_json::from_str::<OperatorPolicyFile>(policy_json).map_err(|error| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("failed to decode operator policy file: {error}"),
        })?;
    let entry = policy
        .operators
        .iter()
        .find(|entry| !entry.token.trim().is_empty() && entry.token == token)
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "operator token not found in policy".into(),
        })?;
    let permissions = entry
        .permissions
        .iter()
        .map(|permission| parse_permission(permission.trim()))
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(Operator {
        id: entry.id.trim().to_string(),
        email: entry.email.trim().to_string(),
        role: entry.role.trim().to_string(),
        permissions,
    })
}

fn validate_operator_token(headers: &HeaderMap) -> Result<(), ApiError> {
    let Ok(expected_token) = env::var("ADMIN_OPERATOR_TOKEN") else {
        return Ok(());
    };
    let expected_token = expected_token.trim();
    if expected_token.is_empty() {
        return Ok(());
    }
    let token = bearer_token(headers)?;
    if token == expected_token {
        Ok(())
    } else {
        Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "invalid operator bearer token".into(),
        })
    }
}

fn bearer_token(headers: &HeaderMap) -> Result<String, ApiError> {
    let provided = required_header(headers, "authorization")?;
    provided
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "authorization header must be a Bearer token".into(),
        })
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            let value = value.trim();
            value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

fn validate_approval_decision(
    approval: &ApprovalRecord,
    operator_id: &str,
    status: &ApprovalStatus,
    allow_self_approval: bool,
) -> Result<(), ApiError> {
    if *status == ApprovalStatus::Approved
        && !allow_self_approval
        && approval.requested_by == operator_id
    {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "approval cannot be approved by its requester".into(),
        });
    }
    Ok(())
}

fn require_approval_manager(operator: &Operator) -> Result<(), ApiError> {
    if operator.has_permission(&Permission::ApprovalManage)
        || operator.has_permission(&Permission::PermissionManage)
    {
        Ok(())
    } else {
        Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "operator lacks required permission: approval_manage".into(),
        })
    }
}

fn required_header(headers: &HeaderMap, name: &str) -> Result<String, ApiError> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: format!("missing required operator header: {name}"),
        })
}

fn parse_permission(value: &str) -> Result<Permission, ApiError> {
    match value {
        "account_read" => Ok(Permission::AccountRead),
        "account_ban" => Ok(Permission::AccountBan),
        "character_read" => Ok(Permission::CharacterRead),
        "character_kick" => Ok(Permission::CharacterKick),
        "inventory_read" => Ok(Permission::InventoryRead),
        "inventory_grant_item" => Ok(Permission::InventoryGrantItem),
        "currency_grant" => Ok(Permission::CurrencyGrant),
        "mail_send_system" => Ok(Permission::MailSendSystem),
        "content_publish" => Ok(Permission::ContentPublish),
        "content_rollback" => Ok(Permission::ContentRollback),
        "audit_read" => Ok(Permission::AuditRead),
        "approval_manage" => Ok(Permission::ApprovalManage),
        "permission_manage" => Ok(Permission::PermissionManage),
        _ => Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: format!("unknown operator permission: {value}"),
        }),
    }
}

fn fetch_clickhouse_admin_events(
    query: &AdminEventQuery,
) -> Result<Vec<AdminEventRecord>, ApiError> {
    let base_url =
        env::var("ADMIN_CLICKHOUSE_URL").unwrap_or_else(|_| "http://127.0.0.1:8123".into());
    let user = env::var("ADMIN_CLICKHOUSE_USER").unwrap_or_else(|_| "mir2".into());
    let password =
        env::var("ADMIN_CLICKHOUSE_PASSWORD").unwrap_or_else(|_| "mir2_dev_password".into());
    let database = env::var("ADMIN_CLICKHOUSE_DATABASE").unwrap_or_else(|_| "mir2_events".into());
    let url = ParsedClickHouseUrl::parse(base_url.trim()).map_err(|message| ApiError {
        status: StatusCode::BAD_GATEWAY,
        message,
    })?;
    let query_text = build_clickhouse_admin_events_query(query);
    let path = format!(
        "/?user={}&password={}&database={}",
        url_component(&user),
        url_component(&password),
        url_component(&database)
    );
    let response = post_clickhouse_query(&url, &path, &query_text).map_err(|message| ApiError {
        status: StatusCode::BAD_GATEWAY,
        message,
    })?;
    response
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(parse_clickhouse_admin_event)
        .collect()
}

fn build_clickhouse_admin_events_query(query: &AdminEventQuery) -> String {
    let mut filters = Vec::new();
    if let Some(command_id) = non_empty_query_value(query.command_id.as_deref()) {
        filters.push(format!(
            "command_id = '{}'",
            clickhouse_string_literal(command_id)
        ));
    }
    if let Some(event_type) = non_empty_query_value(query.event_type.as_deref()) {
        filters.push(format!(
            "event_type = '{}'",
            clickhouse_string_literal(event_type)
        ));
    }
    if let Some(status) = non_empty_query_value(query.status.as_deref()) {
        filters.push(format!("status = '{}'", clickhouse_string_literal(status)));
    }
    let where_clause = if filters.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", filters.join(" AND "))
    };
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    format!(
        "SELECT event_id,event_type,command_id,operator_id,status,occurred_at_ms,payload_json \
         FROM admin_events{where_clause} \
         ORDER BY occurred_at_ms DESC \
         LIMIT {limit} \
         FORMAT JSONEachRow"
    )
}

fn non_empty_query_value(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn clickhouse_string_literal(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedClickHouseUrl {
    host: String,
    port: u16,
}

impl ParsedClickHouseUrl {
    fn parse(url: &str) -> Result<Self, String> {
        let Some(rest) = url.strip_prefix("http://") else {
            return Err(format!("only http:// ClickHouse URLs are supported: {url}"));
        };
        let host_port = rest.split('/').next().unwrap_or_default();
        if host_port.is_empty() {
            return Err(format!("invalid ClickHouse URL: {url}"));
        }
        let (host, port) = match host_port.rsplit_once(':') {
            Some((host, port)) => {
                let port = port
                    .parse::<u16>()
                    .map_err(|error| format!("invalid ClickHouse port: {error}"))?;
                (host.to_string(), port)
            }
            None => (host_port.to_string(), 80),
        };
        Ok(Self { host, port })
    }

    fn host_header(&self) -> String {
        if self.port == 80 {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

fn post_clickhouse_query(
    url: &ParsedClickHouseUrl,
    path: &str,
    query: &str,
) -> Result<String, String> {
    let mut addrs = (url.host.as_str(), url.port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve ClickHouse failed: {error}"))?;
    let addr = addrs
        .next()
        .ok_or_else(|| format!("no socket address for ClickHouse {}", url.host))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))
        .map_err(|error| format!("ClickHouse connect failed at {addr}: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("ClickHouse read timeout setup failed: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("ClickHouse write timeout setup failed: {error}"))?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        url.host_header(),
        query.len(),
        query
    )
    .map_err(|error| format!("ClickHouse query write failed: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("ClickHouse query flush failed: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("ClickHouse response read failed: {error}"))?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| format!("invalid ClickHouse HTTP response: {response:?}"))?;
    let status_code = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| format!("missing ClickHouse HTTP status: {head:?}"))?;
    if !(200..300).contains(&status_code) {
        return Err(format!(
            "ClickHouse query failed with HTTP {status_code}: {}",
            body.trim()
        ));
    }
    if head
        .lines()
        .any(|line| line.eq_ignore_ascii_case("Transfer-Encoding: chunked"))
    {
        decode_chunked_body(body)
    } else {
        Ok(body.to_string())
    }
}

fn decode_chunked_body(body: &str) -> Result<String, String> {
    let mut rest = body;
    let mut decoded = String::new();
    loop {
        let Some((size_text, after_size)) = rest.split_once("\r\n") else {
            return Err("invalid chunked ClickHouse response".into());
        };
        let size_hex = size_text.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|error| format!("invalid chunk size from ClickHouse: {error}"))?;
        if size == 0 {
            return Ok(decoded);
        }
        if after_size.len() < size + 2 {
            return Err("truncated chunked ClickHouse response".into());
        }
        decoded.push_str(&after_size[..size]);
        rest = &after_size[size + 2..];
    }
}

fn parse_clickhouse_admin_event(line: &str) -> Result<AdminEventRecord, ApiError> {
    let row: Value = serde_json::from_str(line).map_err(|error| ApiError {
        status: StatusCode::BAD_GATEWAY,
        message: format!("decode ClickHouse admin event failed: {error}"),
    })?;
    Ok(AdminEventRecord {
        event_id: string_field(&row, "event_id")?,
        event_type: string_field(&row, "event_type")?,
        command_id: string_field(&row, "command_id")?,
        operator_id: string_field(&row, "operator_id")?,
        status: string_field(&row, "status")?,
        occurred_at_ms: u64_field(&row, "occurred_at_ms")?,
        payload_json: string_field(&row, "payload_json")?,
    })
}

fn string_field(row: &Value, field: &str) -> Result<String, ApiError> {
    row.get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ApiError {
            status: StatusCode::BAD_GATEWAY,
            message: format!("ClickHouse event row missing string field {field}"),
        })
}

fn u64_field(row: &Value, field: &str) -> Result<u64, ApiError> {
    let value = row.get(field).ok_or_else(|| ApiError {
        status: StatusCode::BAD_GATEWAY,
        message: format!("ClickHouse event row missing integer field {field}"),
    })?;
    if let Some(value) = value.as_u64() {
        return Ok(value);
    }
    if let Some(value) = value.as_str().and_then(|value| value.parse::<u64>().ok()) {
        return Ok(value);
    }
    Err(ApiError {
        status: StatusCode::BAD_GATEWAY,
        message: format!("ClickHouse event row missing integer field {field}"),
    })
}

fn url_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn validate_command_payload(command: &AdminCommand) -> Result<(), AdminError> {
    match command {
        AdminCommand::SendSystemMail {
            target_id,
            subject,
            body,
            attachments,
            ..
        } => {
            require_non_empty("target_id", target_id)?;
            require_non_empty("subject", subject)?;
            require_non_empty("body", body)?;
            for attachment in attachments {
                require_non_empty("attachment.item_id", &attachment.item_id)?;
                if attachment.count == 0 {
                    return Err(AdminError::InvalidCommand(
                        "attachment.count must be greater than zero".into(),
                    ));
                }
            }
        }
        AdminCommand::GrantItem {
            character_id,
            item_id,
            count,
        } => {
            require_non_empty("character_id", character_id)?;
            require_non_empty("item_id", item_id)?;
            if *count == 0 {
                return Err(AdminError::InvalidCommand(
                    "count must be greater than zero".into(),
                ));
            }
        }
        AdminCommand::GrantCurrency {
            character_id,
            currency,
            amount,
        } => {
            require_non_empty("character_id", character_id)?;
            require_non_empty("currency", currency)?;
            if *amount == 0 {
                return Err(AdminError::InvalidCommand(
                    "amount must be greater than zero".into(),
                ));
            }
        }
        AdminCommand::KickPlayer { character_id } => {
            require_non_empty("character_id", character_id)?;
        }
        AdminCommand::BanAccount { account_id, .. } => {
            require_non_empty("account_id", account_id)?;
        }
    }
    Ok(())
}

fn command_requires_approval(command: &AdminCommand) -> bool {
    match command {
        AdminCommand::SendSystemMail { target_kind, .. } => *target_kind == MailTargetKind::Global,
        AdminCommand::GrantItem { .. }
        | AdminCommand::GrantCurrency { .. }
        | AdminCommand::BanAccount { .. } => true,
        AdminCommand::KickPlayer { .. } => false,
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<(), AdminError> {
    if value.trim().is_empty() {
        return Err(AdminError::InvalidCommand(format!("{field} is required")));
    }
    Ok(())
}

fn error_code(error: &AdminError) -> &'static str {
    match error {
        AdminError::PermissionDenied { .. } => "permission_denied",
        AdminError::InvalidCommand(_) => "invalid_command",
        AdminError::DuplicateCommand(_) => "duplicate_command",
        AdminError::ExecutionFailed(_) => "execution_failed",
        AdminError::Repository(_) => "repository",
        AdminError::UnsupportedCommand(_) => "unsupported_command",
    }
}

fn repository_lock_error<T>(_: std::sync::PoisonError<T>) -> AdminError {
    AdminError::Repository("postgres repository lock poisoned".into())
}

fn map_command_insert_error(error: postgres::Error) -> AdminError {
    if error
        .as_db_error()
        .map(|db_error| db_error.code() == &SqlState::UNIQUE_VIOLATION)
        .unwrap_or(false)
    {
        return AdminError::DuplicateCommand("duplicate postgres command_id".into());
    }
    AdminError::Repository(format!("postgres command insert failed: {error}"))
}

fn enqueue_approval_event(
    client: &mut Client,
    record: &ApprovalRecord,
    event_type: &str,
) -> Result<(), AdminError> {
    let event_suffix = event_type.rsplit('.').next().unwrap_or("event");
    let outbox_id = format!("outbox-approval-{}-{event_suffix}", record.approval_id);
    let occurred_at_ms = record.updated_at_ms;
    let operator_id = record
        .decided_by
        .clone()
        .unwrap_or_else(|| record.requested_by.clone());
    let payload = serde_json::json!({
        "approvalId": record.approval_id,
        "commandId": record.command_id,
        "commandType": record.command_type,
        "requestedBy": record.requested_by,
        "requestedReason": record.requested_reason,
        "decidedBy": record.decided_by,
        "decisionReason": record.decision_reason,
        "updatedAtMs": occurred_at_ms
    });
    let event_payload = AdminEventEnvelope::new(
        outbox_id.clone(),
        event_type.to_string(),
        record.command_id.clone(),
        operator_id,
        approval_status_text(&record.status),
        occurred_at_ms,
        payload,
    )?
    .into_value()?;
    client
        .execute(
            "INSERT INTO admin_outbox (
                outbox_id,
                command_id,
                topic,
                payload_json,
                status,
                attempts,
                created_at_ms,
                updated_at_ms
            ) VALUES ($1,NULL,$2,$3,'pending',0,$4,$5)
            ON CONFLICT (outbox_id) DO NOTHING",
            &[
                &outbox_id,
                &event_type,
                &event_payload,
                &(occurred_at_ms as i64),
                &(occurred_at_ms as i64),
            ],
        )
        .map_err(|error| {
            AdminError::Repository(format!("postgres approval event enqueue failed: {error}"))
        })?;
    Ok(())
}

fn row_to_command_record(row: &Row) -> Result<AdminCommandRecord, AdminError> {
    let envelope_json: Value = row.get("envelope_json");
    let envelope = serde_json::from_value::<AdminCommandEnvelope>(envelope_json)
        .map_err(|error| AdminError::Repository(format!("decode command failed: {error}")))?;
    let status: String = row.get("status");
    let created_at_ms: i64 = row.get("created_at_ms");
    let updated_at_ms: i64 = row.get("updated_at_ms");
    Ok(AdminCommandRecord {
        envelope,
        status: parse_status(&status)?,
        result_message: row.get("result_message"),
        error_code: row.get("error_code"),
        created_at_ms: created_at_ms.max(0) as u64,
        updated_at_ms: updated_at_ms.max(0) as u64,
    })
}

fn row_to_audit_record(row: &Row) -> Result<AuditRecord, AdminError> {
    let target_json: Value = row.get("target_json");
    let target = serde_json::from_value::<AdminTarget>(target_json)
        .map_err(|error| AdminError::Repository(format!("decode audit target failed: {error}")))?;
    let permission: String = row.get("permission");
    let status: String = row.get("status");
    let created_at_ms: i64 = row.get("created_at_ms");
    let completed_at_ms: Option<i64> = row.get("completed_at_ms");
    Ok(AuditRecord {
        audit_id: row.get("audit_id"),
        command_id: row.get("command_id"),
        operator_id: row.get("operator_id"),
        operator_email: row.get("operator_email"),
        operator_role_snapshot: row.get("operator_role_snapshot"),
        permission: parse_permission_text(&permission)?,
        target,
        reason: row.get("reason"),
        status: parse_status(&status)?,
        error_code: row.get("error_code"),
        trace_id: row.get("trace_id"),
        created_at_ms: created_at_ms.max(0) as u64,
        completed_at_ms: completed_at_ms.map(|value| value.max(0) as u64),
    })
}

fn row_to_approval_record(row: &Row) -> Result<ApprovalRecord, AdminError> {
    let status: String = row.get("status");
    let created_at_ms: i64 = row.get("created_at_ms");
    let updated_at_ms: i64 = row.get("updated_at_ms");
    let decided_at_ms: Option<i64> = row.get("decided_at_ms");
    Ok(ApprovalRecord {
        approval_id: row.get("approval_id"),
        command_id: row.get("command_id"),
        command_type: row.get("command_type"),
        status: parse_approval_status(&status)?,
        requested_by: row.get("requested_by"),
        requested_reason: row.get("requested_reason"),
        decided_by: row.get("decided_by"),
        decision_reason: row.get("decision_reason"),
        created_at_ms: created_at_ms.max(0) as u64,
        updated_at_ms: updated_at_ms.max(0) as u64,
        decided_at_ms: decided_at_ms.map(|value| value.max(0) as u64),
    })
}

fn row_to_outbox_message(row: &Row) -> Result<AdminOutboxMessage, AdminError> {
    let attempts: i32 = row.get("attempts");
    let next_attempt_at_ms: Option<i64> = row.get("next_attempt_at_ms");
    let dispatched_at_ms: Option<i64> = row.get("dispatched_at_ms");
    let created_at_ms: i64 = row.get("created_at_ms");
    let updated_at_ms: i64 = row.get("updated_at_ms");
    Ok(AdminOutboxMessage {
        outbox_id: row.get("outbox_id"),
        command_id: row.get("command_id"),
        topic: row.get("topic"),
        payload: row.get("payload_json"),
        status: row.get("status"),
        nats_status: row.get("nats_status"),
        redpanda_status: row.get("redpanda_status"),
        last_error: row.get("last_error"),
        attempts: attempts.max(0) as u32,
        next_attempt_at_ms: next_attempt_at_ms.map(|value| value.max(0) as u64),
        dispatched_at_ms: dispatched_at_ms.map(|value| value.max(0) as u64),
        created_at_ms: created_at_ms.max(0) as u64,
        updated_at_ms: updated_at_ms.max(0) as u64,
    })
}

fn status_text(status: &CommandStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{status:?}").to_lowercase())
}

fn approval_status_text(status: &ApprovalStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{status:?}").to_lowercase())
}

fn parse_approval_status(value: &str) -> Result<ApprovalStatus, AdminError> {
    serde_json::from_value(Value::String(value.to_string()))
        .map_err(|error| AdminError::Repository(format!("decode approval status failed: {error}")))
}

fn command_event_type(status: &CommandStatus) -> Option<&'static str> {
    match status {
        CommandStatus::Succeeded => Some("admin.command.succeeded"),
        CommandStatus::Failed => Some("admin.command.failed"),
        CommandStatus::Denied => Some("admin.command.denied"),
        CommandStatus::Pending | CommandStatus::Executing => None,
    }
}

fn parse_status(value: &str) -> Result<CommandStatus, AdminError> {
    serde_json::from_value(Value::String(value.to_string()))
        .map_err(|error| AdminError::Repository(format!("decode status failed: {error}")))
}

fn permission_text(permission: &Permission) -> String {
    serde_json::to_value(permission)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{permission:?}").to_lowercase())
}

fn parse_permission_text(value: &str) -> Result<Permission, AdminError> {
    serde_json::from_value(Value::String(value.to_string()))
        .map_err(|error| AdminError::Repository(format!("decode permission failed: {error}")))
}

fn target_type_text(target_type: &TargetType) -> String {
    serde_json::to_value(target_type)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{target_type:?}").to_lowercase())
}

fn command_type(command: &AdminCommand) -> String {
    match command {
        AdminCommand::SendSystemMail { .. } => "send_system_mail",
        AdminCommand::GrantItem { .. } => "grant_item",
        AdminCommand::GrantCurrency { .. } => "grant_currency",
        AdminCommand::KickPlayer { .. } => "kick_player",
        AdminCommand::BanAccount { .. } => "ban_account",
    }
    .to_string()
}

fn build_admin_timeline(
    state: AdminApiState,
    query: AdminTimelineQuery,
) -> Result<AdminTimelineResponse, ApiError> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let mut records = Vec::new();
    {
        let control = state.control_plane.lock().map_err(lock_error)?;
        for command in control.command_records(limit) {
            if !timeline_matches(
                &query,
                Some(&command.envelope.command_id),
                Some(&command.envelope.target.target_id),
            ) {
                continue;
            }
            records.push(AdminTimelineItem {
                source: "command".to_string(),
                record_id: command.envelope.command_id.clone(),
                command_id: Some(command.envelope.command_id.clone()),
                target_id: Some(command.envelope.target.target_id.clone()),
                event_type: command_type(&command.envelope.command),
                status: status_text(&command.status),
                actor_id: Some(command.envelope.operator.id.clone()),
                occurred_at_ms: command.updated_at_ms,
                summary: command
                    .result_message
                    .clone()
                    .or_else(|| command.error_code.clone())
                    .unwrap_or_else(|| command_type(&command.envelope.command)),
            });
        }
        for audit in control.audit_records(limit) {
            if !timeline_matches(
                &query,
                Some(&audit.command_id),
                Some(&audit.target.target_id),
            ) {
                continue;
            }
            records.push(AdminTimelineItem {
                source: "audit".to_string(),
                record_id: audit.audit_id.clone(),
                command_id: Some(audit.command_id.clone()),
                target_id: Some(audit.target.target_id.clone()),
                event_type: permission_text(&audit.permission),
                status: status_text(&audit.status),
                actor_id: Some(audit.operator_id.clone()),
                occurred_at_ms: audit.completed_at_ms.unwrap_or(audit.created_at_ms),
                summary: audit.reason.clone(),
            });
        }
        for approval in control.approval_records(limit) {
            if !timeline_matches(&query, Some(&approval.command_id), None) {
                continue;
            }
            records.push(AdminTimelineItem {
                source: "approval".to_string(),
                record_id: approval.approval_id.clone(),
                command_id: Some(approval.command_id.clone()),
                target_id: None,
                event_type: format!("approval.{}", approval.command_type),
                status: approval_status_text(&approval.status),
                actor_id: approval
                    .decided_by
                    .clone()
                    .or_else(|| Some(approval.requested_by.clone())),
                occurred_at_ms: approval.updated_at_ms,
                summary: approval
                    .decision_reason
                    .clone()
                    .unwrap_or(approval.requested_reason.clone()),
            });
        }
    }

    let mut degraded = false;
    let mut error = None;
    if query.target_id.is_none() {
        let events_query = AdminEventQuery {
            limit: Some(limit),
            command_id: query.command_id.clone(),
            event_type: None,
            status: None,
        };
        match fetch_clickhouse_admin_events(&events_query) {
            Ok(events) => {
                for event in events {
                    if !timeline_matches(&query, Some(&event.command_id), None) {
                        continue;
                    }
                    records.push(AdminTimelineItem {
                        source: "event".to_string(),
                        record_id: event.event_id,
                        command_id: Some(event.command_id),
                        target_id: None,
                        event_type: event.event_type,
                        status: event.status,
                        actor_id: Some(event.operator_id),
                        occurred_at_ms: event.occurred_at_ms,
                        summary: event.payload_json,
                    });
                }
            }
            Err(fetch_error) => {
                degraded = true;
                error = Some(fetch_error.message);
            }
        }
    }

    records.sort_by(|left, right| {
        right
            .occurred_at_ms
            .cmp(&left.occurred_at_ms)
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.record_id.cmp(&right.record_id))
    });
    records.truncate(limit);
    Ok(AdminTimelineResponse {
        degraded,
        error,
        records,
    })
}

fn timeline_matches(
    query: &AdminTimelineQuery,
    command_id: Option<&str>,
    target_id: Option<&str>,
) -> bool {
    if let Some(expected_command_id) = query.command_id.as_deref() {
        if command_id != Some(expected_command_id) {
            return false;
        }
    }
    if let Some(expected_target_id) = query.target_id.as_deref() {
        if target_id != Some(expected_target_id) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct RecordingExecutor {
        executed_commands: Vec<String>,
        fail: bool,
    }

    impl AdminCommandExecutor for RecordingExecutor {
        fn execute(
            &mut self,
            envelope: &AdminCommandEnvelope,
        ) -> Result<CommandExecutionResult, AdminError> {
            if self.fail {
                return Err(AdminError::ExecutionFailed("executor unavailable".into()));
            }
            self.executed_commands.push(envelope.command_id.clone());
            Ok(CommandExecutionResult {
                status: CommandStatus::Succeeded,
                message: "accepted".into(),
            })
        }
    }

    #[test]
    fn send_system_mail_requires_permission_and_writes_success_audit() {
        let mut control = AdminControlPlane::new(RecordingExecutor::default());
        let envelope = sample_envelope(operator_with([Permission::MailSendSystem]));

        let result = control
            .submit(envelope, 2_000)
            .expect("command should pass");

        assert_eq!(result.status, CommandStatus::Succeeded);
        assert_eq!(control.executor().executed_commands, vec!["cmd-1"]);
        assert_eq!(control.audit_records(10).len(), 1);
        assert_eq!(control.command_records(10).len(), 1);
        let audit = &control.audit_records(10)[0];
        assert_eq!(audit.status, CommandStatus::Succeeded);
        assert_eq!(audit.permission, Permission::MailSendSystem);
        assert_eq!(audit.completed_at_ms, Some(2_000));
        assert_eq!(audit.reason, "support requested system mail");
    }

    #[test]
    fn denied_command_is_audited_and_not_executed() {
        let mut control = AdminControlPlane::new(RecordingExecutor::default());
        let envelope = sample_envelope(operator_with([]));

        let error = control.submit(envelope, 2_000).expect_err("should deny");

        assert!(matches!(
            error,
            AdminError::PermissionDenied {
                permission: Permission::MailSendSystem
            }
        ));
        assert!(control.executor().executed_commands.is_empty());
        assert_eq!(control.audit_records(10).len(), 1);
        assert_eq!(control.audit_records(10)[0].status, CommandStatus::Denied);
        assert_eq!(
            control.audit_records(10)[0].error_code.as_deref(),
            Some("permission_denied")
        );
        assert_eq!(control.command_records(10)[0].status, CommandStatus::Denied);
    }

    #[test]
    fn reason_is_required_before_command_is_recorded() {
        let mut control = AdminControlPlane::new(RecordingExecutor::default());
        let mut envelope = sample_envelope(operator_with([Permission::MailSendSystem]));
        envelope.reason = "  ".into();

        let error = control.submit(envelope, 2_000).expect_err("should reject");

        assert!(matches!(error, AdminError::InvalidCommand(_)));
        assert!(control.audit_records(10).is_empty());
        assert!(control.command_records(10).is_empty());
        assert!(control.executor().executed_commands.is_empty());
    }

    #[test]
    fn high_risk_commands_require_approval_before_recording() {
        let mut control = AdminControlPlane::new(RecordingExecutor::default());
        let mut envelope = sample_envelope(operator_with([Permission::InventoryGrantItem]));
        envelope.command = AdminCommand::GrantItem {
            character_id: "Scout".into(),
            item_id: "red-potion".into(),
            count: 1,
        };

        let error = control.submit(envelope, 2_000).expect_err("should reject");

        assert!(
            matches!(error, AdminError::InvalidCommand(message) if message.contains("approval_id"))
        );
        assert!(control.audit_records(10).is_empty());
        assert!(control.command_records(10).is_empty());
    }

    #[test]
    fn high_risk_commands_require_approved_approval_record() {
        let mut control = AdminControlPlane::new(RecordingExecutor::default());
        let mut envelope = sample_envelope(operator_with([Permission::InventoryGrantItem]));
        envelope.command_id = "cmd-grant-approval".into();
        envelope.approval_id = Some("approval-pending".into());
        envelope.command = AdminCommand::GrantItem {
            character_id: "Scout".into(),
            item_id: "red-potion".into(),
            count: 1,
        };
        control
            .create_approval(ApprovalRecord::pending(
                "approval-pending".into(),
                "cmd-grant-approval".into(),
                "grant_item".into(),
                "lead-gm".into(),
                "review high risk grant".into(),
                1_000,
            ))
            .expect("approval create");

        let error = control
            .submit(envelope.clone(), 2_000)
            .expect_err("pending approval should reject");

        assert!(
            matches!(error, AdminError::InvalidCommand(message) if message.contains("not approved"))
        );
        assert_eq!(control.command_records(10)[0].status, CommandStatus::Denied);
        assert!(control.executor().executed_commands.is_empty());

        control
            .decide_approval(
                "approval-pending",
                ApprovalStatus::Approved,
                "lead-gm".into(),
                Some("approved for smoke".into()),
                3_000,
            )
            .expect("approval approve");
        envelope.command_id = "cmd-grant-approval-approved".into();
        envelope.approval_id = Some("approval-approved".into());
        control
            .create_approval(ApprovalRecord::pending(
                "approval-approved".into(),
                "cmd-grant-approval-approved".into(),
                "grant_item".into(),
                "lead-gm".into(),
                "review high risk grant".into(),
                4_000,
            ))
            .expect("approval create");
        control
            .decide_approval(
                "approval-approved",
                ApprovalStatus::Approved,
                "lead-gm".into(),
                Some("approved for execution".into()),
                5_000,
            )
            .expect("approval approve");

        let result = control
            .submit(envelope, 6_000)
            .expect("approved command should execute");

        assert_eq!(result.status, CommandStatus::Succeeded);
        assert_eq!(
            control.executor().executed_commands,
            vec!["cmd-grant-approval-approved"]
        );
    }

    #[test]
    fn duplicate_command_id_is_rejected_before_second_execution() {
        let mut control = AdminControlPlane::new(RecordingExecutor::default());
        let envelope = sample_envelope(operator_with([Permission::MailSendSystem]));

        control
            .submit(envelope.clone(), 2_000)
            .expect("first command should pass");
        let error = control
            .submit(envelope, 3_000)
            .expect_err("duplicate should fail");

        assert_eq!(error, AdminError::DuplicateCommand("cmd-1".into()));
        assert_eq!(control.executor().executed_commands, vec!["cmd-1"]);
        assert_eq!(control.audit_records(10).len(), 1);
        assert_eq!(control.command_records(10).len(), 1);
    }

    #[test]
    fn executor_failure_is_audited() {
        let mut control = AdminControlPlane::new(RecordingExecutor {
            fail: true,
            ..RecordingExecutor::default()
        });
        let envelope = sample_envelope(operator_with([Permission::MailSendSystem]));

        let error = control.submit(envelope, 2_000).expect_err("should fail");

        assert!(matches!(error, AdminError::ExecutionFailed(_)));
        assert_eq!(control.audit_records(10).len(), 1);
        assert_eq!(control.audit_records(10)[0].status, CommandStatus::Failed);
        assert_eq!(
            control.audit_records(10)[0].error_code.as_deref(),
            Some("execution_failed")
        );
        assert_eq!(control.command_records(10)[0].status, CommandStatus::Failed);
    }

    #[test]
    fn system_mail_executor_writes_domain_outbox() {
        let executor = SystemMailExecutor::new(InMemorySystemMailOutbox::default());
        let mut control = AdminControlPlane::new(executor);
        let envelope = sample_envelope(operator_with([Permission::MailSendSystem]));

        let result = control.submit(envelope, 2_000).expect("mail should queue");

        assert_eq!(result.status, CommandStatus::Succeeded);
        let receipts = control.executor().domain().receipts();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].outbox_id, "mail-cmd-1");
        assert_eq!(receipts[0].target_id, "char-1");
        assert_eq!(receipts[0].attachment_count, 1);
    }

    #[tokio::test]
    async fn get_command_status_returns_one_command_record() {
        let path = std::env::temp_dir().join(format!(
            "mir2-admin-status-{}-{}.json",
            std::process::id(),
            now_ms()
        ));
        let config = SimulationConfig::default().with_account_store_path(path.clone());
        let executor =
            SystemMailExecutor::new(AccountStoreSystemMailDomain::with_gateway_and_fallback(
                "http://127.0.0.1:1/admin/system-mail".into(),
                config,
            ));
        let mut control = AdminControlPlane::with_repositories(
            executor,
            CommandRepositoryBackend::default(),
            AuditRepositoryBackend::default(),
            ApprovalRepositoryBackend::default(),
        );
        let mut envelope = sample_envelope(operator_with([Permission::MailSendSystem]));
        envelope.target.target_id = "Scout".into();
        envelope.target.character_id = Some("Scout".into());
        envelope.command = AdminCommand::SendSystemMail {
            target_kind: MailTargetKind::Character,
            target_id: "Scout".into(),
            subject: "Status".into(),
            body: "Command status smoke.".into(),
            attachments: vec![ItemGrant {
                item_id: "gold".into(),
                count: 1,
            }],
        };
        control.submit(envelope, 2_000).expect("mail should queue");
        let state = AdminApiState {
            control_plane: Arc::new(Mutex::new(control)),
        };

        let record = get_command_status(State(state.clone()), Path("cmd-1".into()))
            .await
            .expect("command status should load")
            .0;

        assert_eq!(record.envelope.command_id, "cmd-1");
        assert_eq!(record.status, CommandStatus::Succeeded);
        assert_eq!(
            record.result_message.as_deref(),
            Some("system mail queued as mail-cmd-1")
        );

        let error = get_command_status(State(state), Path("missing".into()))
            .await
            .expect_err("missing command should return 404");

        assert_eq!(error.status, StatusCode::NOT_FOUND);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn grant_item_and_gold_commands_queue_game_mail_with_approval() {
        let executor = SystemMailExecutor::new(InMemorySystemMailOutbox::default());
        let mut control = AdminControlPlane::new(executor);
        let mut item_envelope = sample_envelope(operator_with([Permission::InventoryGrantItem]));
        item_envelope.command_id = "cmd-grant-item".into();
        item_envelope.approval_id = Some("approval-1".into());
        item_envelope.command = AdminCommand::GrantItem {
            character_id: "Scout".into(),
            item_id: "red-potion".into(),
            count: 2,
        };
        control
            .create_approval(ApprovalRecord::pending(
                "approval-1".into(),
                "cmd-grant-item".into(),
                "grant_item".into(),
                "lead-gm".into(),
                "review item grant".into(),
                1_000,
            ))
            .expect("approval create");
        control
            .decide_approval(
                "approval-1",
                ApprovalStatus::Approved,
                "lead-gm".into(),
                Some("approved item grant".into()),
                1_500,
            )
            .expect("approval approve");

        let item_result = control
            .submit(item_envelope, 2_000)
            .expect("item grant should queue");
        assert_eq!(item_result.status, CommandStatus::Succeeded);

        let mut gold_envelope = sample_envelope(operator_with([Permission::CurrencyGrant]));
        gold_envelope.command_id = "cmd-grant-gold".into();
        gold_envelope.approval_id = Some("approval-2".into());
        gold_envelope.command = AdminCommand::GrantCurrency {
            character_id: "Scout".into(),
            currency: "gold".into(),
            amount: 250,
        };
        control
            .create_approval(ApprovalRecord::pending(
                "approval-2".into(),
                "cmd-grant-gold".into(),
                "grant_currency".into(),
                "lead-gm".into(),
                "review gold grant".into(),
                2_500,
            ))
            .expect("approval create");
        control
            .decide_approval(
                "approval-2",
                ApprovalStatus::Approved,
                "lead-gm".into(),
                Some("approved gold grant".into()),
                2_750,
            )
            .expect("approval approve");

        let gold_result = control
            .submit(gold_envelope, 3_000)
            .expect("gold grant should queue");

        assert_eq!(gold_result.status, CommandStatus::Succeeded);
        let receipts = control.executor().domain().receipts();
        assert_eq!(receipts.len(), 2);
        assert_eq!(receipts[0].attachment_count, 1);
        assert_eq!(receipts[1].attachment_count, 1);
    }

    #[test]
    fn kick_and_ban_commands_execute_with_expected_permissions() {
        let executor = SystemMailExecutor::new(InMemorySystemMailOutbox::default());
        let mut control = AdminControlPlane::new(executor);

        let mut kick_envelope = sample_envelope(operator_with([Permission::CharacterKick]));
        kick_envelope.command_id = "cmd-kick".into();
        kick_envelope.command = AdminCommand::KickPlayer {
            character_id: "Scout".into(),
        };
        let kick_result = control
            .submit(kick_envelope, 2_000)
            .expect("kick should execute");
        assert_eq!(kick_result.status, CommandStatus::Succeeded);
        assert!(kick_result.message.contains("memory kick accepted"));

        let mut ban_envelope = sample_envelope(operator_with([Permission::AccountBan]));
        ban_envelope.command_id = "cmd-ban".into();
        ban_envelope.approval_id = Some("approval-ban".into());
        ban_envelope.target = AdminTarget {
            target_type: TargetType::Account,
            target_id: "demo".into(),
            account_id: Some("demo".into()),
            character_id: None,
        };
        ban_envelope.command = AdminCommand::BanAccount {
            account_id: "demo".into(),
            duration_seconds: Some(60),
        };
        control
            .create_approval(ApprovalRecord::pending(
                "approval-ban".into(),
                "cmd-ban".into(),
                "ban_account".into(),
                "lead-gm".into(),
                "review ban".into(),
                2_500,
            ))
            .expect("approval create");
        control
            .decide_approval(
                "approval-ban",
                ApprovalStatus::Approved,
                "lead-gm".into(),
                Some("approved ban".into()),
                2_750,
            )
            .expect("approval approve");

        let ban_result = control
            .submit(ban_envelope, 3_000)
            .expect("ban should execute");
        assert_eq!(ban_result.status, CommandStatus::Succeeded);
        assert!(ban_result.message.contains("memory ban accepted"));
    }

    #[test]
    fn admin_timeline_merges_local_command_audit_and_approval_records() {
        let executor =
            SystemMailExecutor::new(AccountStoreSystemMailDomain::with_gateway_and_fallback(
                "http://127.0.0.1:1/admin/system-mail".into(),
                SimulationConfig::default(),
            ));
        let mut control = AdminControlPlane::with_repositories(
            executor,
            CommandRepositoryBackend::default(),
            AuditRepositoryBackend::default(),
            ApprovalRepositoryBackend::default(),
        );
        control
            .create_approval(ApprovalRecord::pending(
                "approval-timeline".into(),
                "cmd-timeline".into(),
                "send_system_mail".into(),
                "lead-gm".into(),
                "timeline review".into(),
                1_000,
            ))
            .expect("approval create");
        let mut envelope = sample_envelope(operator_with([Permission::MailSendSystem]));
        envelope.command_id = "cmd-timeline".into();
        envelope.target.target_id = "Scout".into();
        envelope.command = AdminCommand::SendSystemMail {
            target_kind: MailTargetKind::Character,
            target_id: "Scout".into(),
            subject: "Timeline".into(),
            body: "Timeline smoke".into(),
            attachments: Vec::new(),
        };
        control.submit(envelope, 2_000).expect("command submit");
        let state = AdminApiState {
            control_plane: Arc::new(Mutex::new(AdminControlPlane::with_repositories(
                control.executor,
                control.command_store,
                control.audit_store,
                control.approval_store,
            ))),
        };

        let timeline = build_admin_timeline(
            state,
            AdminTimelineQuery {
                limit: Some(10),
                command_id: None,
                target_id: Some("Scout".into()),
            },
        )
        .expect("timeline should build");

        assert!(!timeline.degraded);
        assert_eq!(timeline.records.len(), 2);
        assert_eq!(timeline.records[0].source, "audit");
        assert_eq!(timeline.records[1].source, "command");
        assert!(timeline
            .records
            .iter()
            .all(|record| record.command_id.as_deref() == Some("cmd-timeline")));
    }

    #[test]
    fn admin_outbox_store_tracks_pending_and_dispatched_messages() {
        let mut store = InMemoryAdminOutboxStore::default();
        store
            .insert_pending(AdminOutboxMessage::pending(
                "outbox-1".into(),
                "cmd-1".into(),
                "admin.command.send_system_mail".into(),
                serde_json::json!({"targetId":"Scout"}),
                1_000,
            ))
            .expect("outbox insert should pass");

        assert_eq!(store.list_pending(10).len(), 1);

        store
            .record_delivery_attempt("outbox-1", "succeeded", "not_configured", None, 1_500)
            .expect("delivery status should update");
        let pending = store.records.get("outbox-1").expect("outbox record");
        assert_eq!(pending.nats_status, "succeeded");
        assert_eq!(pending.redpanda_status, "not_configured");

        store
            .mark_dispatched("outbox-1", 2_000)
            .expect("outbox dispatch should pass");

        let dispatched = store.records.get("outbox-1").expect("outbox record");
        assert_eq!(dispatched.dispatched_at_ms, Some(2_000));
        assert!(store.list_pending(10).is_empty());
    }

    #[test]
    fn admin_outbox_store_tracks_retry_delay_and_dead_letter() {
        let mut store = InMemoryAdminOutboxStore::default();
        store
            .insert_pending(AdminOutboxMessage::pending(
                "outbox-retry".into(),
                "cmd-retry".into(),
                "admin.command.succeeded".into(),
                serde_json::json!({"commandId":"cmd-retry"}),
                1_000,
            ))
            .expect("outbox insert should pass");

        store
            .mark_retry("outbox-retry", now_ms().saturating_add(60_000), 2_000)
            .expect("retry mark should pass");
        assert!(store.list_pending(10).is_empty());

        store
            .mark_dead_letter("outbox-retry", 3_000)
            .expect("dead-letter mark should pass");
        let record = store.records.get("outbox-retry").expect("outbox record");
        assert_eq!(record.status, "dead_letter");
        assert_eq!(record.attempts, 2);
        assert_eq!(record.next_attempt_at_ms, None);
        assert_eq!(record.dispatched_at_ms, None);
    }

    #[test]
    fn admin_outbox_event_payload_wraps_legacy_payload_in_envelope() {
        let message = AdminOutboxMessage::pending(
            "outbox-cmd-1".into(),
            "cmd-1".into(),
            "admin.command.succeeded".into(),
            serde_json::json!({
                "resultMessage": "queued",
                "errorCode": null,
                "updatedAtMs": 123
            }),
            123,
        );

        let payload = admin_outbox_event_payload(&message).expect("event envelope");
        assert_eq!(payload["eventId"], "outbox-cmd-1");
        assert_eq!(payload["eventType"], "admin.command.succeeded");
        assert_eq!(payload["schemaVersion"], ADMIN_EVENT_SCHEMA_VERSION);
        assert_eq!(payload["commandId"], "cmd-1");
        assert_eq!(payload["operatorId"], "unknown");
        assert_eq!(payload["payload"]["resultMessage"], "queued");
        assert!(payload["payloadJson"]
            .as_str()
            .expect("payloadJson")
            .contains("resultMessage"));
    }

    #[test]
    fn clickhouse_admin_event_row_parses_json_each_row_output() {
        let record = parse_clickhouse_admin_event(
            r#"{"event_id":"outbox-1","event_type":"admin.command.succeeded","command_id":"cmd-1","operator_id":"operator-1","status":"succeeded","occurred_at_ms":"123","payload_json":"{}"}"#,
        )
        .expect("event row should parse");

        assert_eq!(record.event_id, "outbox-1");
        assert_eq!(record.event_type, "admin.command.succeeded");
        assert_eq!(record.command_id, "cmd-1");
        assert_eq!(record.operator_id, "operator-1");
        assert_eq!(record.status, "succeeded");
        assert_eq!(record.occurred_at_ms, 123);
        assert_eq!(record.payload_json, "{}");
    }

    #[test]
    fn clickhouse_url_parser_and_component_encoding_are_stable() {
        assert_eq!(
            ParsedClickHouseUrl::parse("http://127.0.0.1:8123").expect("parse"),
            ParsedClickHouseUrl {
                host: "127.0.0.1".into(),
                port: 8123
            }
        );
        assert_eq!(url_component("mir2 dev@local"), "mir2%20dev%40local");
        assert_eq!(
            decode_chunked_body("5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n").expect("chunk decode"),
            "hello world"
        );
    }

    #[test]
    fn clickhouse_admin_events_query_applies_filters_and_limit() {
        let query = build_clickhouse_admin_events_query(&AdminEventQuery {
            limit: Some(1_000),
            command_id: Some("cmd'1".into()),
            event_type: Some("admin.command.succeeded".into()),
            status: Some("succeeded".into()),
        });

        assert!(query.contains("command_id = 'cmd\\'1'"));
        assert!(query.contains("event_type = 'admin.command.succeeded'"));
        assert!(query.contains("status = 'succeeded'"));
        assert!(query.contains("LIMIT 500"));
    }

    #[test]
    fn command_event_type_covers_terminal_admin_statuses() {
        assert_eq!(
            command_event_type(&CommandStatus::Succeeded),
            Some("admin.command.succeeded")
        );
        assert_eq!(
            command_event_type(&CommandStatus::Failed),
            Some("admin.command.failed")
        );
        assert_eq!(
            command_event_type(&CommandStatus::Denied),
            Some("admin.command.denied")
        );
        assert_eq!(command_event_type(&CommandStatus::Pending), None);
        assert_eq!(command_event_type(&CommandStatus::Executing), None);
    }

    #[test]
    fn operator_policy_json_authenticates_operator_and_permissions() {
        let operator = operator_from_policy_json(
            r#"{
                "operators": [
                    {
                        "id": "ops-lead",
                        "email": "lead@mir2.dev",
                        "role": "lead",
                        "token": "secret-token",
                        "permissions": ["approval_manage", "account_ban"]
                    }
                ]
            }"#,
            "secret-token",
        )
        .expect("policy token should authenticate");

        assert_eq!(operator.id, "ops-lead");
        assert!(operator.has_permission(&Permission::ApprovalManage));
        assert!(operator.has_permission(&Permission::AccountBan));
    }

    #[test]
    fn approval_decision_blocks_self_approval_by_default() {
        let approval = ApprovalRecord::pending(
            "approval-self".into(),
            "cmd-self".into(),
            "ban_account".into(),
            "ops-lead".into(),
            "self approval smoke".into(),
            1_000,
        );

        let error =
            validate_approval_decision(&approval, "ops-lead", &ApprovalStatus::Approved, false)
                .expect_err("self approval should fail");

        assert_eq!(error.status, StatusCode::FORBIDDEN);
        validate_approval_decision(&approval, "ops-peer", &ApprovalStatus::Approved, false)
            .expect("peer approval should pass");
        validate_approval_decision(&approval, "ops-lead", &ApprovalStatus::Rejected, false)
            .expect("self rejection should pass");
    }

    #[test]
    fn account_store_system_mail_domain_falls_back_to_persistent_game_mail() {
        let path = std::env::temp_dir().join(format!(
            "mir2-admin-mail-{}-{}.json",
            std::process::id(),
            now_ms()
        ));
        let config = SimulationConfig::default().with_account_store_path(path.clone());
        let executor =
            SystemMailExecutor::new(AccountStoreSystemMailDomain::with_gateway_and_fallback(
                "http://127.0.0.1:1/admin/system-mail".to_string(),
                config,
            ));
        let mut control = AdminControlPlane::new(executor);
        let mut envelope = sample_envelope(operator_with([Permission::MailSendSystem]));
        envelope.command = AdminCommand::SendSystemMail {
            target_kind: MailTargetKind::Character,
            target_id: "Scout".into(),
            subject: "Admin mail".into(),
            body: "Persisted into game mail.".into(),
            attachments: vec![
                ItemGrant {
                    item_id: "gold".into(),
                    count: 99,
                },
                ItemGrant {
                    item_id: "red-potion".into(),
                    count: 1,
                },
            ],
        };

        let result = control
            .submit(envelope, 2_000)
            .expect("fallback delivery should succeed");

        assert_eq!(result.status, CommandStatus::Succeeded);
        let receipts = control.executor().domain().receipts();
        assert_eq!(receipts[0].delivery_mode, "account_store_fallback");
        assert_eq!(receipts[0].delivered_count, 1);
        assert_eq!(receipts[0].mail_ids, vec![1]);

        let store = mir2_simulation::AccountStore::load_or_new(
            &path,
            SimulationConfig::default().default_character,
        );
        let save = store
            .accounts
            .get("demo")
            .and_then(|account| account.saves.get(&0))
            .expect("demo save should exist");
        let systems: mir2_simulation::Stage5SystemsState = serde_json::from_str(
            save.stage5_systems_json
                .as_deref()
                .expect("stage5 systems should be persisted"),
        )
        .expect("stage5 systems should decode");
        assert_eq!(systems.mail[0].gold, 99);
        assert_eq!(systems.mail[0].items, vec!["red-potion"]);

        let _ = std::fs::remove_file(path);
    }

    fn sample_envelope(operator: Operator) -> AdminCommandEnvelope {
        AdminCommandEnvelope {
            command_id: "cmd-1".into(),
            operator,
            target: AdminTarget {
                target_type: TargetType::Character,
                target_id: "char-1".into(),
                account_id: Some("account-1".into()),
                character_id: Some("char-1".into()),
            },
            reason: "support requested system mail".into(),
            approval_id: None,
            command: AdminCommand::SendSystemMail {
                target_kind: MailTargetKind::Character,
                target_id: "char-1".into(),
                subject: "Welcome".into(),
                body: "Welcome to the test shard.".into(),
                attachments: vec![ItemGrant {
                    item_id: "red-potion".into(),
                    count: 3,
                }],
            },
            trace_id: "trace-1".into(),
            created_at_ms: 1_000,
        }
    }

    fn operator_with<const N: usize>(permissions: [Permission; N]) -> Operator {
        Operator {
            id: "op-1".into(),
            email: "operator@example.com".into(),
            role: "gm".into(),
            permissions: permissions.into_iter().collect(),
        }
    }
}
