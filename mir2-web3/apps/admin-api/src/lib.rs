use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use mir2_simulation::{
    deliver_stage5_system_mail, SimulationConfig, Stage5MailDelivery, Stage5MailDeliveryReceipt,
    Stage5MailTargetKind,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminOutboxMessage {
    pub outbox_id: String,
    pub command_id: String,
    pub topic: String,
    pub payload: Value,
    pub status: String,
    pub attempts: u32,
    pub next_attempt_at_ms: Option<u64>,
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
            command_id,
            topic,
            payload,
            status: "pending".into(),
            attempts: 0,
            next_attempt_at_ms: None,
            created_at_ms,
            updated_at_ms: created_at_ms,
        }
    }
}

pub trait AdminOutboxRepository {
    fn insert_pending(&mut self, message: AdminOutboxMessage) -> Result<(), AdminError>;
    fn mark_dispatched(&mut self, outbox_id: &str, updated_at_ms: u64) -> Result<(), AdminError>;
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

    fn mark_dispatched(&mut self, outbox_id: &str, updated_at_ms: u64) -> Result<(), AdminError> {
        let record = self.records.get_mut(outbox_id).ok_or_else(|| {
            AdminError::Repository(format!("admin outbox message not found: {outbox_id}"))
        })?;
        record.status = "dispatched".into();
        record.updated_at_ms = updated_at_ms;
        Ok(())
    }

    fn list_pending(&self, limit: usize) -> Vec<AdminOutboxMessage> {
        self.records
            .values()
            .filter(|record| record.status == "pending")
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
        let should_enqueue_outbox = status == CommandStatus::Succeeded;
        let status = status_text(&status);
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let updated = client
            .execute(
                "UPDATE admin_commands
                 SET status = $2,
                     result_message = $3,
                     error_code = $4,
                     updated_at_ms = $5,
                     updated_at = now()
                 WHERE command_id = $1",
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
        if updated == 0 {
            return Err(AdminError::Repository(format!(
                "command record not found: {command_id}"
            )));
        }
        if should_enqueue_outbox {
            let outbox_id = format!("outbox-{command_id}");
            let payload = serde_json::json!({
                "commandId": command_id,
                "status": status,
                "resultMessage": result_message,
                "errorCode": error_code,
                "updatedAtMs": updated_at_ms
            });
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
                    ) VALUES ($1,$2,'admin.command.succeeded',$3,'pending',0,$4,$5)
                    ON CONFLICT (outbox_id) DO NOTHING",
                    &[
                        &outbox_id,
                        &command_id,
                        &payload,
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
                    attempts,
                    next_attempt_at_ms,
                    created_at_ms,
                    updated_at_ms
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
                &[
                    &message.outbox_id,
                    &message.command_id,
                    &message.topic,
                    &message.payload,
                    &status,
                    &(message.attempts as i32),
                    &next_attempt_at_ms,
                    &(message.created_at_ms as i64),
                    &(message.updated_at_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres outbox insert failed: {error}"))
            })?;
        Ok(())
    }

    fn mark_dispatched(&mut self, outbox_id: &str, updated_at_ms: u64) -> Result<(), AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let updated = client
            .execute(
                "UPDATE admin_outbox
                 SET status = 'dispatched',
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

    fn list_pending(&self, limit: usize) -> Vec<AdminOutboxMessage> {
        let mut client = match self.client.lock() {
            Ok(client) => client,
            Err(_) => return Vec::new(),
        };
        client
            .query(
                "SELECT outbox_id, command_id, topic, payload_json, status, attempts,
                        next_attempt_at_ms, created_at_ms, updated_at_ms
                 FROM admin_outbox
                 WHERE status = 'pending'
                 ORDER BY created_at_ms ASC
                 LIMIT $1",
                &[&(limit.min(i32::MAX as usize) as i64)],
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
            Self::Postgres(store) => store.get(command_id),
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

pub struct AdminControlPlane<E, C = InMemoryCommandStore, A = InMemoryAuditStore> {
    executor: E,
    command_store: C,
    audit_store: A,
}

impl<E> AdminControlPlane<E, InMemoryCommandStore, InMemoryAuditStore>
where
    E: AdminCommandExecutor,
{
    pub fn new(executor: E) -> Self {
        Self::with_repositories(
            executor,
            InMemoryCommandStore::default(),
            InMemoryAuditStore::default(),
        )
    }
}

impl<E, C, A> AdminControlPlane<E, C, A>
where
    E: AdminCommandExecutor,
    C: AdminCommandRepository,
    A: AuditRepository,
{
    pub fn with_repositories(executor: E, command_store: C, audit_store: A) -> Self {
        Self {
            executor,
            command_store,
            audit_store,
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

    pub fn audit_records(&self, limit: usize) -> Vec<AuditRecord> {
        self.audit_store.list_recent(limit)
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
}

#[derive(Debug, Clone)]
pub struct AccountStoreSystemMailDomain {
    gateway_mail_url: String,
    fallback_config: SimulationConfig,
    receipts: Vec<SystemMailReceipt>,
}

impl AccountStoreSystemMailDomain {
    pub fn from_env() -> Self {
        let gateway_mail_url = env::var("ADMIN_GATEWAY_MAIL_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:7110/admin/system-mail".to_string());
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
    let (host, path) = parse_http_url(gateway_mail_url)?;
    let body = serde_json::to_string(delivery)
        .map_err(|error| format!("gateway mail request encode failed: {error}"))?;
    let mut stream = TcpStream::connect(&host)
        .map_err(|error| format!("gateway mail endpoint unavailable: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| format!("gateway mail read timeout setup failed: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| format!("gateway mail write timeout setup failed: {error}"))?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .map_err(|error| format!("gateway mail request write failed: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("gateway mail response read failed: {error}"))?;
    let (head, response_body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "gateway mail response was not valid HTTP".to_string())?;
    let status_line = head.lines().next().unwrap_or_default();
    if !status_line.contains(" 2") {
        return Err(format!(
            "gateway mail endpoint rejected request: {status_line} {response_body}"
        ));
    }
    serde_json::from_str::<Stage5MailDeliveryReceipt>(response_body)
        .map_err(|error| format!("gateway mail response decode failed: {error}"))
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
            other => Err(AdminError::UnsupportedCommand(format!("{other:?}"))),
        }
    }
}

type HttpControlPlane = AdminControlPlane<
    SystemMailExecutor<AccountStoreSystemMailDomain>,
    CommandRepositoryBackend,
    AuditRepositoryBackend,
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
        let (command_store, audit_store) = match PostgresAdminRepository::from_env()? {
            Some(repository) => (
                CommandRepositoryBackend::Postgres(repository.clone()),
                AuditRepositoryBackend::Postgres(repository),
            ),
            None => (
                CommandRepositoryBackend::default(),
                AuditRepositoryBackend::default(),
            ),
        };
        Ok(Self {
            control_plane: Arc::new(Mutex::new(AdminControlPlane::with_repositories(
                SystemMailExecutor::new(AccountStoreSystemMailDomain::from_env()),
                command_store,
                audit_store,
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
        .route("/admin/audit", get(list_audit))
        .route("/admin/system-mail/outbox", get(list_system_mail_outbox))
        .route("/admin/commands/send-system-mail", post(submit_system_mail))
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
pub struct SubmitCommandResponse {
    pub command_id: String,
    pub result: CommandExecutionResult,
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
        "permission_manage" => Ok(Permission::PermissionManage),
        _ => Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: format!("unknown operator permission: {value}"),
        }),
    }
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

fn row_to_outbox_message(row: &Row) -> Result<AdminOutboxMessage, AdminError> {
    let attempts: i32 = row.get("attempts");
    let next_attempt_at_ms: Option<i64> = row.get("next_attempt_at_ms");
    let created_at_ms: i64 = row.get("created_at_ms");
    let updated_at_ms: i64 = row.get("updated_at_ms");
    Ok(AdminOutboxMessage {
        outbox_id: row.get("outbox_id"),
        command_id: row.get("command_id"),
        topic: row.get("topic"),
        payload: row.get("payload_json"),
        status: row.get("status"),
        attempts: attempts.max(0) as u32,
        next_attempt_at_ms: next_attempt_at_ms.map(|value| value.max(0) as u64),
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
            .mark_dispatched("outbox-1", 2_000)
            .expect("outbox dispatch should pass");

        assert!(store.list_pending(10).is_empty());
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
