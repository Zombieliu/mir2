use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use mir2_simulation::{
    ban_account_in_store, deliver_stage5_system_mail, AccountRecord, AccountStore,
    AccountStoreDatabaseMode, CharacterRecord, CharacterSaveRecord, SimulationConfig,
    Stage5MailDelivery, Stage5MailDeliveryReceipt, Stage5MailMessage, Stage5MailTargetKind,
    Stage5SystemsState,
};
use postgres::error::SqlState;
use postgres::{Client, NoTls, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    AccountRead,
    AccountWrite,
    AccountBan,
    CharacterRead,
    CharacterWrite,
    CharacterKick,
    CharacterMessage,
    InventoryRead,
    InventoryGrantItem,
    CurrencyGrant,
    MailSendSystem,
    WorldBroadcast,
    ServerControl,
    MarketModerate,
    GuildModerate,
    NameListManage,
    ContentRead,
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
    Guild,
    Market,
    NameList,
    Server,
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
    CreateAccount {
        account_id: String,
        password: Option<String>,
    },
    UpdateAccount {
        account_id: String,
        password: Option<String>,
        storage_size: Option<u16>,
        clear_storage_password: bool,
        unban: bool,
    },
    DeleteAccount {
        account_id: String,
    },
    UpdateCharacter {
        character_id: String,
        name: Option<String>,
        level: Option<u16>,
        gold: Option<u32>,
        credit: Option<u32>,
        map_file_name: Option<String>,
        map_title: Option<String>,
        position_x: Option<i32>,
        position_y: Option<i32>,
        hp: Option<i32>,
        max_hp: Option<i32>,
        mp: Option<i32>,
        pk_points: Option<i32>,
    },
    ReturnSafeZone {
        character_id: String,
    },
    KillPlayer {
        character_id: String,
    },
    KillPets {
        character_id: String,
    },
    SetCharacterFlag {
        character_id: String,
        flag_index: u32,
        active: bool,
    },
    SetChatBan {
        character_id: String,
        banned: bool,
        until_ms: Option<u64>,
    },
    MessagePlayer {
        character_id: String,
        message: String,
    },
    Broadcast {
        message: String,
    },
    MarketCancelListing {
        character_id: String,
        listing_id: u32,
        notify_seller: bool,
    },
    MarketExpireListing {
        character_id: String,
        listing_id: u32,
        notify_seller: bool,
    },
    MarketDeleteListing {
        character_id: String,
        listing_id: u32,
        reason_message: Option<String>,
    },
    GuildRemoveMember {
        guild_name: String,
        member_name: String,
    },
    GuildSendMessage {
        guild_name: String,
        message: String,
    },
    NameListAdd {
        list_name: String,
        player_name: String,
    },
    NameListRemove {
        list_name: String,
        player_name: String,
    },
    NameListCreate {
        list_name: String,
    },
    NameListDelete {
        list_name: String,
    },
    PublishContentBundle {
        bundle_id: String,
        category: String,
        record_key: String,
        payload_json: Value,
    },
    ServerControl {
        action: String,
        target: Option<String>,
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
            AdminCommand::CreateAccount { .. }
            | AdminCommand::UpdateAccount { .. }
            | AdminCommand::DeleteAccount { .. } => Permission::AccountWrite,
            AdminCommand::UpdateCharacter { .. }
            | AdminCommand::ReturnSafeZone { .. }
            | AdminCommand::KillPlayer { .. }
            | AdminCommand::KillPets { .. }
            | AdminCommand::SetCharacterFlag { .. }
            | AdminCommand::SetChatBan { .. } => Permission::CharacterWrite,
            AdminCommand::MessagePlayer { .. } => Permission::CharacterMessage,
            AdminCommand::Broadcast { .. } => Permission::WorldBroadcast,
            AdminCommand::MarketCancelListing { .. }
            | AdminCommand::MarketExpireListing { .. }
            | AdminCommand::MarketDeleteListing { .. } => Permission::MarketModerate,
            AdminCommand::GuildRemoveMember { .. } | AdminCommand::GuildSendMessage { .. } => {
                Permission::GuildModerate
            }
            AdminCommand::NameListAdd { .. }
            | AdminCommand::NameListRemove { .. }
            | AdminCommand::NameListCreate { .. }
            | AdminCommand::NameListDelete { .. } => Permission::NameListManage,
            AdminCommand::PublishContentBundle { .. } => Permission::ContentPublish,
            AdminCommand::ServerControl { .. } => Permission::ServerControl,
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

    pub fn upsert_system_mail_receipt(
        &self,
        receipt: &SystemMailReceipt,
    ) -> Result<(), AdminError> {
        let command_id = receipt
            .outbox_id
            .strip_prefix("mail-")
            .unwrap_or(&receipt.outbox_id)
            .to_string();
        let target_kind = mail_target_kind_text(&receipt.target_kind);
        let mail_ids_json = serde_json::to_value(&receipt.mail_ids).map_err(|error| {
            AdminError::Repository(format!("encode mail receipt ids failed: {error}"))
        })?;
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .execute(
                "INSERT INTO admin_system_mail_receipts (
                    outbox_id,
                    command_id,
                    target_kind,
                    target_id,
                    attachment_count,
                    accepted_at_ms,
                    delivery_mode,
                    delivered_count,
                    mail_ids_json
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                ON CONFLICT (outbox_id) DO UPDATE
                SET target_kind = EXCLUDED.target_kind,
                    target_id = EXCLUDED.target_id,
                    attachment_count = EXCLUDED.attachment_count,
                    accepted_at_ms = EXCLUDED.accepted_at_ms,
                    delivery_mode = EXCLUDED.delivery_mode,
                    delivered_count = EXCLUDED.delivered_count,
                    mail_ids_json = EXCLUDED.mail_ids_json,
                    updated_at = now()",
                &[
                    &receipt.outbox_id,
                    &command_id,
                    &target_kind,
                    &receipt.target_id,
                    &(receipt.attachment_count.min(i32::MAX as usize) as i32),
                    &(receipt.accepted_at_ms as i64),
                    &receipt.delivery_mode,
                    &(receipt.delivered_count.min(i32::MAX as usize) as i32),
                    &mail_ids_json,
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!(
                    "postgres system mail receipt upsert failed: {error}"
                ))
            })?;
        Ok(())
    }

    pub fn list_system_mail_receipts(
        &self,
        limit: usize,
    ) -> Result<Vec<SystemMailReceipt>, AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .query(
                "SELECT outbox_id, target_kind, target_id, attachment_count, accepted_at_ms,
                        delivery_mode, delivered_count, mail_ids_json
                 FROM admin_system_mail_receipts
                 ORDER BY accepted_at_ms DESC, outbox_id DESC
                 LIMIT $1",
                &[&(limit.min(i32::MAX as usize) as i64)],
            )
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row_to_system_mail_receipt(row).ok())
                    .collect()
            })
            .map_err(|error| {
                AdminError::Repository(format!("postgres system mail receipt list failed: {error}"))
            })
    }

    pub fn upsert_activity(
        &self,
        request: UpsertAdminActivityRequest,
        operator_id: &str,
        now_ms: u64,
    ) -> Result<AdminActivityRecord, AdminError> {
        let activity_id = request
            .activity_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("activity-{now_ms}"));
        let name = required_string("name", &request.name)?;
        let activity_type = required_string("activity_type", &request.activity_type)?;
        let status = optional_string(request.status).unwrap_or_else(|| "Draft".to_string());
        let signal = optional_string(request.signal).unwrap_or_default();
        let reward = optional_string(request.reward).unwrap_or_default();
        let condition = optional_string(request.condition).unwrap_or_default();
        let realms = clean_string_list(request.realms);
        let realms_json = serde_json::to_value(&realms).map_err(|error| {
            AdminError::Repository(format!("encode activity realms failed: {error}"))
        })?;
        let start_at_ms = request.start_at_ms.map(|value| value as i64);
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let row = client
            .query_one(
                "INSERT INTO admin_activities (
                    activity_id,
                    name,
                    activity_type,
                    status,
                    signal,
                    start_at_ms,
                    reward,
                    condition,
                    realms_json,
                    created_by,
                    created_at_ms,
                    updated_at_ms
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11)
                ON CONFLICT (activity_id) DO UPDATE
                SET name = EXCLUDED.name,
                    activity_type = EXCLUDED.activity_type,
                    status = EXCLUDED.status,
                    signal = EXCLUDED.signal,
                    start_at_ms = EXCLUDED.start_at_ms,
                    reward = EXCLUDED.reward,
                    condition = EXCLUDED.condition,
                    realms_json = EXCLUDED.realms_json,
                    updated_at_ms = EXCLUDED.updated_at_ms,
                    updated_at = now()
                RETURNING activity_id, name, activity_type, status, signal, start_at_ms,
                          reward, condition, realms_json, updated_at_ms",
                &[
                    &activity_id,
                    &name,
                    &activity_type,
                    &status,
                    &signal,
                    &start_at_ms,
                    &reward,
                    &condition,
                    &realms_json,
                    &operator_id,
                    &(now_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres activity upsert failed: {error}"))
            })?;
        row_to_activity_record(&row)
    }

    pub fn list_activities(&self, limit: usize) -> Result<Vec<AdminActivityRecord>, AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .query(
                "SELECT activity_id, name, activity_type, status, signal, start_at_ms,
                        reward, condition, realms_json, updated_at_ms
                 FROM admin_activities
                 ORDER BY updated_at_ms DESC, activity_id ASC
                 LIMIT $1",
                &[&(limit.min(i32::MAX as usize) as i64)],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres activity list failed: {error}"))
            })?
            .iter()
            .map(row_to_activity_record)
            .collect()
    }

    pub fn upsert_price_feed(
        &self,
        request: UpsertPriceFeedRequest,
        now_ms: u64,
    ) -> Result<AdminPriceFeedRecord, AdminError> {
        let item = required_string("item", &request.item)?;
        let source = required_string("source", &request.source)?;
        let sample_count = request.sample_count.min(i32::MAX as usize) as i32;
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let row = client
            .query_one(
                "INSERT INTO admin_market_price_feeds (
                    item,
                    latest_price,
                    sample_count,
                    source,
                    updated_at_ms
                ) VALUES ($1,$2,$3,$4,$5)
                ON CONFLICT (item) DO UPDATE
                SET latest_price = EXCLUDED.latest_price,
                    sample_count = EXCLUDED.sample_count,
                    source = EXCLUDED.source,
                    updated_at_ms = EXCLUDED.updated_at_ms,
                    updated_at = now()
                RETURNING item, latest_price, sample_count, source, updated_at_ms",
                &[
                    &item,
                    &(request.latest_price as i64),
                    &sample_count,
                    &source,
                    &(now_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres price-feed upsert failed: {error}"))
            })?;
        row_to_price_feed_record(&row)
    }

    pub fn list_price_feeds(&self, limit: usize) -> Result<Vec<AdminPriceFeedRecord>, AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .query(
                "SELECT item, latest_price, sample_count, source, updated_at_ms
                 FROM admin_market_price_feeds
                 ORDER BY updated_at_ms DESC, item ASC
                 LIMIT $1",
                &[&(limit.min(i32::MAX as usize) as i64)],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres price-feed list failed: {error}"))
            })?
            .iter()
            .map(row_to_price_feed_record)
            .collect()
    }

    pub fn upsert_trade_graph_edge(
        &self,
        request: UpsertTradeGraphEdgeRequest,
        now_ms: u64,
    ) -> Result<AdminRiskGraphEdge, AdminError> {
        let from = required_string("from", &request.from)?;
        let to = required_string("to", &request.to)?;
        let signal = required_string("signal", &request.signal)?;
        let risk = required_string("risk", &request.risk)?;
        let evidence = optional_string(request.evidence).unwrap_or_default();
        let edge_id = request
            .edge_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("trade-edge-{from}-{to}-{now_ms}"));
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let row = client
            .query_one(
                "INSERT INTO admin_trade_graph_edges (
                    edge_id,
                    from_player_id,
                    to_player_id,
                    signal,
                    risk,
                    evidence,
                    updated_at_ms
                ) VALUES ($1,$2,$3,$4,$5,$6,$7)
                ON CONFLICT (edge_id) DO UPDATE
                SET from_player_id = EXCLUDED.from_player_id,
                    to_player_id = EXCLUDED.to_player_id,
                    signal = EXCLUDED.signal,
                    risk = EXCLUDED.risk,
                    evidence = EXCLUDED.evidence,
                    updated_at_ms = EXCLUDED.updated_at_ms,
                    updated_at = now()
                RETURNING edge_id, from_player_id, to_player_id, signal, risk, evidence,
                          updated_at_ms",
                &[
                    &edge_id,
                    &from,
                    &to,
                    &signal,
                    &risk,
                    &evidence,
                    &(now_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres trade graph upsert failed: {error}"))
            })?;
        row_to_trade_graph_edge(&row)
    }

    pub fn list_trade_graph_edges(
        &self,
        limit: usize,
    ) -> Result<Vec<AdminRiskGraphEdge>, AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .query(
                "SELECT edge_id, from_player_id, to_player_id, signal, risk, evidence,
                        updated_at_ms
                 FROM admin_trade_graph_edges
                 ORDER BY updated_at_ms DESC, edge_id ASC
                 LIMIT $1",
                &[&(limit.min(i32::MAX as usize) as i64)],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres trade graph list failed: {error}"))
            })?
            .iter()
            .map(row_to_trade_graph_edge)
            .collect()
    }

    pub fn upsert_zone_runtime(
        &self,
        request: UpsertZoneRuntimeRequest,
        now_ms: u64,
    ) -> Result<AdminZoneRuntimeRecord, AdminError> {
        let name = required_string("name", &request.name)?;
        let zone_id = request
            .zone_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| normalized_id("zone", &name, now_ms));
        let status = optional_string(request.status).unwrap_or_else(|| "Healthy".to_string());
        let host = optional_string(request.host).unwrap_or_default();
        let process_id = optional_string(request.process_id).unwrap_or_default();
        let map_count = request.map_count.unwrap_or_default().min(i32::MAX as usize) as i32;
        let player_count = request
            .player_count
            .unwrap_or_default()
            .min(i32::MAX as usize) as i32;
        let tick_rate = request.tick_rate.unwrap_or_default().min(i32::MAX as u32) as i32;
        let uptime_seconds = request
            .uptime_seconds
            .unwrap_or_default()
            .min(i64::MAX as u64) as i64;
        let source =
            optional_string(request.source).unwrap_or_else(|| "operator_projection".into());
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let row = client
            .query_one(
                "INSERT INTO admin_zone_runtime_records (
                    zone_id,
                    name,
                    status,
                    host,
                    process_id,
                    map_count,
                    player_count,
                    tick_rate,
                    uptime_seconds,
                    source,
                    updated_at_ms
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
                ON CONFLICT (zone_id) DO UPDATE
                SET name = EXCLUDED.name,
                    status = EXCLUDED.status,
                    host = EXCLUDED.host,
                    process_id = EXCLUDED.process_id,
                    map_count = EXCLUDED.map_count,
                    player_count = EXCLUDED.player_count,
                    tick_rate = EXCLUDED.tick_rate,
                    uptime_seconds = EXCLUDED.uptime_seconds,
                    source = EXCLUDED.source,
                    updated_at_ms = EXCLUDED.updated_at_ms,
                    updated_at = now()
                RETURNING zone_id, name, status, host, process_id, map_count, player_count,
                          tick_rate, uptime_seconds, source, updated_at_ms",
                &[
                    &zone_id,
                    &name,
                    &status,
                    &host,
                    &process_id,
                    &map_count,
                    &player_count,
                    &tick_rate,
                    &uptime_seconds,
                    &source,
                    &(now_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres zone runtime upsert failed: {error}"))
            })?;
        row_to_zone_runtime_record(&row)
    }

    pub fn list_zone_runtime(
        &self,
        limit: usize,
    ) -> Result<Vec<AdminZoneRuntimeRecord>, AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .query(
                "SELECT zone_id, name, status, host, process_id, map_count, player_count,
                        tick_rate, uptime_seconds, source, updated_at_ms
                 FROM admin_zone_runtime_records
                 ORDER BY updated_at_ms DESC, zone_id ASC
                 LIMIT $1",
                &[&(limit.min(i32::MAX as usize) as i64)],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres zone runtime list failed: {error}"))
            })?
            .iter()
            .map(row_to_zone_runtime_record)
            .collect()
    }

    pub fn upsert_operator(
        &self,
        request: UpsertAdminOperatorRequest,
        actor_id: &str,
        now_ms: u64,
    ) -> Result<AdminOperatorRecord, AdminError> {
        let email = required_string("email", &request.email)?;
        let role = required_string("role", &request.role)?;
        let operator_id = request
            .operator_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| email.clone());
        let status = optional_string(request.status).unwrap_or_else(|| "Active".to_string());
        let permissions = canonical_permissions(request.permissions)?;
        let permissions_json = serde_json::to_value(&permissions).map_err(|error| {
            AdminError::Repository(format!("encode operator permissions failed: {error}"))
        })?;
        let token_hash = request
            .token
            .as_deref()
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(operator_token_hash)
            .unwrap_or_default();
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let row = client
            .query_one(
                "INSERT INTO admin_operators (
                    operator_id,
                    email,
                    role,
                    status,
                    permissions_json,
                    token_hash,
                    created_by,
                    created_at_ms,
                    updated_at_ms
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$8)
                ON CONFLICT (operator_id) DO UPDATE
                SET email = EXCLUDED.email,
                    role = EXCLUDED.role,
                    status = EXCLUDED.status,
                    permissions_json = EXCLUDED.permissions_json,
                    token_hash = CASE
                        WHEN EXCLUDED.token_hash <> '' THEN EXCLUDED.token_hash
                        ELSE admin_operators.token_hash
                    END,
                    updated_at_ms = EXCLUDED.updated_at_ms,
                    updated_at = now()
                RETURNING operator_id, email, role, status, permissions_json, token_hash,
                          updated_at_ms, last_authenticated_at_ms",
                &[
                    &operator_id,
                    &email,
                    &role,
                    &status,
                    &permissions_json,
                    &token_hash,
                    &actor_id,
                    &(now_ms as i64),
                ],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres operator upsert failed: {error}"))
            })?;
        row_to_operator_record(&row)
    }

    pub fn list_operators(&self, limit: usize) -> Result<Vec<AdminOperatorRecord>, AdminError> {
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        client
            .query(
                "SELECT operator_id, email, role, status, permissions_json, token_hash,
                        updated_at_ms, last_authenticated_at_ms
                 FROM admin_operators
                 ORDER BY updated_at_ms DESC, operator_id ASC
                 LIMIT $1",
                &[&(limit.min(i32::MAX as usize) as i64)],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres operator list failed: {error}"))
            })?
            .iter()
            .map(row_to_operator_record)
            .collect()
    }

    pub fn operator_by_token(
        &self,
        token: &str,
        now_ms: u64,
    ) -> Result<Option<Operator>, AdminError> {
        let token = token.trim();
        if token.is_empty() {
            return Ok(None);
        }
        let token_hash = operator_token_hash(token);
        let mut client = self.client.lock().map_err(repository_lock_error)?;
        let row = client
            .query_opt(
                "UPDATE admin_operators
                 SET last_authenticated_at_ms = $2
                 WHERE token_hash = $1
                   AND token_hash <> ''
                   AND lower(status) = 'active'
                 RETURNING operator_id, email, role, permissions_json",
                &[&token_hash, &(now_ms as i64)],
            )
            .map_err(|error| {
                AdminError::Repository(format!("postgres operator token lookup failed: {error}"))
            })?;
        row.map(|row| row_to_operator(&row)).transpose()
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
                Some(approval) if approval_matches_command(&approval, &envelope) => {}
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

    fn execute_console_command(
        &mut self,
        command: &AdminCommand,
        _operator_id: &str,
        _reason: &str,
        _accepted_at_ms: u64,
    ) -> Result<String, AdminError> {
        Err(AdminError::UnsupportedCommand(format!(
            "console command is not supported: {command:?}"
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

    fn execute_console_command(
        &mut self,
        command: &AdminCommand,
        operator_id: &str,
        reason: &str,
        _accepted_at_ms: u64,
    ) -> Result<String, AdminError> {
        Ok(format!(
            "memory console command accepted by {operator_id}: {command:?} ({reason})"
        ))
    }
}

#[derive(Debug, Clone)]
pub struct AccountStoreSystemMailDomain {
    gateway_mail_url: String,
    gateway_kick_url: String,
    gateway_control_url: String,
    fallback_config: SimulationConfig,
    name_list_root: PathBuf,
    content_bundle_dir: PathBuf,
    receipts: Vec<SystemMailReceipt>,
}

impl AccountStoreSystemMailDomain {
    pub fn from_env() -> Self {
        let gateway_mail_url = env::var("ADMIN_GATEWAY_MAIL_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:7110/admin/system-mail".to_string());
        let gateway_kick_url = env::var("ADMIN_GATEWAY_KICK_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:7110/admin/kick-player".to_string());
        let gateway_control_url = env::var("ADMIN_GATEWAY_CONTROL_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:7110/admin/control".to_string());
        let name_list_root = name_list_root_from_env();
        let content_bundle_dir = content_bundle_dir_from_env();
        let account_store_path = env::var("ADMIN_ACCOUNT_STORE_PATH")
            .or_else(|_| env::var("MIR2_ACCOUNT_STORE_PATH"))
            .unwrap_or_else(|_| ".mir2-data/accounts.json".to_string());
        let fallback_config = SimulationConfig::default()
            .with_account_store_environment(PathBuf::from(account_store_path))
            .expect("account store environment should initialize");
        Self {
            gateway_mail_url,
            gateway_kick_url,
            gateway_control_url,
            fallback_config,
            name_list_root,
            content_bundle_dir,
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
            gateway_control_url: "http://127.0.0.1:7110/admin/control".to_string(),
            fallback_config,
            name_list_root: name_list_root_from_env(),
            content_bundle_dir: content_bundle_dir_from_env(),
            receipts: Vec::new(),
        }
    }

    pub fn receipts(&self) -> &[SystemMailReceipt] {
        &self.receipts
    }

    pub fn fallback_config(&self) -> &SimulationConfig {
        &self.fallback_config
    }

    fn mutate_account_store(
        &self,
        mutate: impl FnOnce(&mut AccountStore, &CharacterRecord) -> Result<String, AdminError>,
    ) -> Result<String, AdminError> {
        let message = {
            let mut store = self
                .fallback_config
                .account_store
                .lock()
                .map_err(|_| AdminError::Repository("account store lock poisoned".into()))?;
            mutate(&mut store, &self.fallback_config.default_character)?
        };
        self.fallback_config
            .save_account_store()
            .map_err(AdminError::ExecutionFailed)?;
        Ok(message)
    }

    fn deliver_admin_message(
        &self,
        target_kind: Stage5MailTargetKind,
        target_id: String,
        subject: String,
        body: String,
        from: String,
    ) -> Result<String, AdminError> {
        let receipt = deliver_stage5_system_mail(
            &self.fallback_config,
            Stage5MailDelivery {
                target_kind,
                target_id,
                from,
                subject,
                body,
                gold: 0,
                items: Vec::new(),
            },
        )
        .map_err(AdminError::ExecutionFailed)?;
        Ok(format!(
            "admin message delivered to {} mailbox target(s); mail ids={:?}",
            receipt.delivered_count, receipt.mail_ids
        ))
    }

    fn execute_server_control(
        &self,
        action: &str,
        target: Option<&str>,
        operator_id: &str,
        reason: &str,
    ) -> Result<String, AdminError> {
        let action = canonical_control_action(action)?;
        let payload = serde_json::json!({
            "action": action,
            "target": target,
            "operatorId": operator_id,
            "reason": reason,
        });
        match post_json_to_gateway(&self.gateway_control_url, &payload) {
            Ok(receipt) => Ok(format!("gateway control accepted: {receipt}")),
            Err(error) if control_action_can_fallback(&action) => Ok(format!(
                "local control receipt accepted for {action}; gateway unavailable: {error}"
            )),
            Err(error) => Err(AdminError::ExecutionFailed(format!(
                "gateway control endpoint is required for {action}: {error}"
            ))),
        }
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
        if let Ok(database_url) = env::var("ADMIN_DATABASE_URL") {
            if !database_url.trim().is_empty() {
                match PostgresAdminRepository::connect(database_url.trim()) {
                    Ok(repository) => {
                        let _ = repository.ensure_schema();
                        let _ = repository.upsert_system_mail_receipt(&receipt);
                    }
                    Err(_) => {}
                }
            }
        }
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

    fn execute_console_command(
        &mut self,
        command: &AdminCommand,
        operator_id: &str,
        reason: &str,
        accepted_at_ms: u64,
    ) -> Result<String, AdminError> {
        match command {
            AdminCommand::CreateAccount {
                account_id,
                password,
            } => self.mutate_account_store(|store, _| {
                let account_id = required_string("account_id", account_id)?;
                if store.accounts.contains_key(&account_id) {
                    return Err(AdminError::InvalidCommand(format!(
                        "account already exists: {account_id}"
                    )));
                }
                let mut account = AccountRecord::empty();
                if let Some(password) = optional_string(password.clone()) {
                    account.password = password;
                }
                store.accounts.insert(account_id.clone(), account);
                Ok(format!("account {account_id} created"))
            }),
            AdminCommand::UpdateAccount {
                account_id,
                password,
                storage_size,
                clear_storage_password,
                unban,
            } => self.mutate_account_store(|store, _| {
                let account_id = required_string("account_id", account_id)?;
                let account = store.accounts.get_mut(&account_id).ok_or_else(|| {
                    AdminError::InvalidCommand(format!("account not found: {account_id}"))
                })?;
                let mut changes = Vec::new();
                if let Some(password) = optional_string(password.clone()) {
                    account.password = password;
                    changes.push("password");
                }
                if let Some(storage_size) = storage_size {
                    account.storage_size = *storage_size;
                    changes.push("storage_size");
                }
                if *clear_storage_password {
                    account.storage_password.clear();
                    account.storage_password_last_set_binary_datetime = 0;
                    changes.push("storage_password");
                }
                if *unban {
                    account.is_banned = false;
                    account.ban_reason.clear();
                    account.ban_until_ms = None;
                    account.banned_at_ms = None;
                    changes.push("ban_state");
                }
                if changes.is_empty() {
                    return Err(AdminError::InvalidCommand(
                        "no account changes were requested".into(),
                    ));
                }
                Ok(format!(
                    "account {account_id} updated: {}",
                    changes.join(", ")
                ))
            }),
            AdminCommand::DeleteAccount { account_id } => self.mutate_account_store(|store, _| {
                let account_id = required_string("account_id", account_id)?;
                if store.accounts.remove(&account_id).is_none() {
                    return Err(AdminError::InvalidCommand(format!(
                        "account not found: {account_id}"
                    )));
                }
                Ok(format!("account {account_id} deleted"))
            }),
            AdminCommand::UpdateCharacter {
                character_id,
                name,
                level,
                gold,
                credit,
                map_file_name,
                map_title,
                position_x,
                position_y,
                hp,
                max_hp,
                mp,
                pk_points,
            } => self.mutate_account_store(|store, _| {
                let (account_id, character_index) = resolve_character_target(store, character_id)?;
                let requested_name = optional_string(name.clone());
                if let Some(name) = requested_name.as_ref() {
                    if store.accounts.iter().any(|(existing_account_id, account)| {
                        account.characters.iter().any(|existing| {
                            (existing_account_id != &account_id
                                || existing.index != character_index)
                                && existing.name.eq_ignore_ascii_case(name)
                        })
                    }) {
                        return Err(AdminError::InvalidCommand(format!(
                            "character name already exists: {name}"
                        )));
                    }
                }
                let account = store.accounts.get_mut(&account_id).ok_or_else(|| {
                    AdminError::InvalidCommand(format!("account not found: {account_id}"))
                })?;
                let character = account
                    .characters
                    .iter_mut()
                    .find(|character| character.index == character_index)
                    .ok_or_else(|| {
                        AdminError::InvalidCommand(format!("character not found: {character_id}"))
                    })?;
                let save = account.saves.get_mut(&character_index).ok_or_else(|| {
                    AdminError::InvalidCommand(format!("character save not found: {character_id}"))
                })?;
                let mut changes = Vec::new();
                if let Some(name) = requested_name {
                    character.name = name.clone();
                    save.character.name = name;
                    changes.push("name");
                }
                if let Some(level) = level {
                    character.level = *level;
                    save.character.level = *level;
                    changes.push("level");
                }
                if let Some(gold) = gold {
                    save.gold = *gold;
                    changes.push("gold");
                }
                if let Some(credit) = credit {
                    save.credit = *credit;
                    changes.push("credit");
                }
                if let Some(map_file_name) = optional_string(map_file_name.clone()) {
                    save.map_file_name = map_file_name;
                    changes.push("map_file_name");
                }
                if let Some(map_title) = optional_string(map_title.clone()) {
                    save.map_title = map_title;
                    changes.push("map_title");
                }
                if let Some(position_x) = position_x {
                    save.position.x = *position_x;
                    changes.push("position_x");
                }
                if let Some(position_y) = position_y {
                    save.position.y = *position_y;
                    changes.push("position_y");
                }
                if let Some(max_hp) = max_hp {
                    save.max_hp = (*max_hp).max(1);
                    changes.push("max_hp");
                }
                if let Some(hp) = hp {
                    save.hp = *hp;
                    changes.push("hp");
                }
                if let Some(mp) = mp {
                    save.mp = (*mp).max(0);
                    changes.push("mp");
                }
                if let Some(pk_points) = pk_points {
                    save.pk_points = *pk_points;
                    changes.push("pk_points");
                }
                save.hp = save.hp.min(save.max_hp);
                if changes.is_empty() {
                    return Err(AdminError::InvalidCommand(
                        "no character changes were requested".into(),
                    ));
                }
                Ok(format!(
                    "character {} updated: {}",
                    character_player_id(&account_id, character_index),
                    changes.join(", ")
                ))
            }),
            AdminCommand::ReturnSafeZone { character_id } => {
                self.mutate_account_store(|store, _| {
                    let (account_id, character_index) =
                        resolve_character_target(store, character_id)?;
                    let save = store
                        .accounts
                        .get_mut(&account_id)
                        .and_then(|account| account.saves.get_mut(&character_index))
                        .ok_or_else(|| {
                            AdminError::InvalidCommand(format!(
                                "character save not found: {character_id}"
                            ))
                        })?;
                    save.map_file_name = "0".to_string();
                    save.map_title = "BichonProvince".to_string();
                    save.position.x = 284;
                    save.position.y = 607;
                    save.hp = save.hp.max(1);
                    Ok(format!(
                        "character {} returned to BichonProvince safe zone",
                        character_player_id(&account_id, character_index)
                    ))
                })
            }
            AdminCommand::KillPlayer { character_id } => self.mutate_account_store(|store, _| {
                let (account_id, character_index) = resolve_character_target(store, character_id)?;
                let save = store
                    .accounts
                    .get_mut(&account_id)
                    .and_then(|account| account.saves.get_mut(&character_index))
                    .ok_or_else(|| {
                        AdminError::InvalidCommand(format!(
                            "character save not found: {character_id}"
                        ))
                    })?;
                save.hp = 0;
                Ok(format!(
                    "character {} hp set to 0",
                    character_player_id(&account_id, character_index)
                ))
            }),
            AdminCommand::KillPets { character_id } => self.mutate_account_store(|store, _| {
                let (account_id, character_index) = resolve_character_target(store, character_id)?;
                let save = store
                    .accounts
                    .get_mut(&account_id)
                    .and_then(|account| account.saves.get_mut(&character_index))
                    .ok_or_else(|| {
                        AdminError::InvalidCommand(format!(
                            "character save not found: {character_id}"
                        ))
                    })?;
                let mut systems = stage5_systems_from_save(save);
                let removed = systems.hero.take().is_some();
                write_stage5_systems_to_save(save, systems)?;
                Ok(format!(
                    "character {} companion state cleared={removed}",
                    character_player_id(&account_id, character_index)
                ))
            }),
            AdminCommand::SetCharacterFlag {
                character_id,
                flag_index,
                active,
            } => self.mutate_account_store(|store, _| {
                let (account_id, character_index) = resolve_character_target(store, character_id)?;
                let save = store
                    .accounts
                    .get_mut(&account_id)
                    .and_then(|account| account.saves.get_mut(&character_index))
                    .ok_or_else(|| {
                        AdminError::InvalidCommand(format!(
                            "character save not found: {character_id}"
                        ))
                    })?;
                set_npc_flag(save, *flag_index, *active)?;
                Ok(format!(
                    "character {} flag {flag_index} set to {active}",
                    character_player_id(&account_id, character_index)
                ))
            }),
            AdminCommand::SetChatBan {
                character_id,
                banned,
                until_ms,
            } => self.mutate_account_store(|store, _| {
                let (account_id, character_index) = resolve_character_target(store, character_id)?;
                let save = store
                    .accounts
                    .get_mut(&account_id)
                    .and_then(|account| account.saves.get_mut(&character_index))
                    .ok_or_else(|| {
                        AdminError::InvalidCommand(format!(
                            "character save not found: {character_id}"
                        ))
                    })?;
                save.chat_banned = *banned;
                save.chat_ban_until_ms = if *banned { *until_ms } else { None };
                Ok(format!(
                    "character {} chat_banned={} until={:?}",
                    character_player_id(&account_id, character_index),
                    save.chat_banned,
                    save.chat_ban_until_ms
                ))
            }),
            AdminCommand::MessagePlayer {
                character_id,
                message,
            } => self.deliver_admin_message(
                Stage5MailTargetKind::Character,
                character_id.clone(),
                "GM Message".to_string(),
                required_string("message", message)?,
                format!("GM {operator_id}"),
            ),
            AdminCommand::Broadcast { message } => self.deliver_admin_message(
                Stage5MailTargetKind::Global,
                "global".to_string(),
                "GM Broadcast".to_string(),
                required_string("message", message)?,
                format!("GM {operator_id}"),
            ),
            AdminCommand::MarketCancelListing {
                character_id,
                listing_id,
                notify_seller,
            } => self.mutate_account_store(|store, _| {
                let (account_id, character_index) = resolve_character_target(store, character_id)?;
                let account = store.accounts.get_mut(&account_id).ok_or_else(|| {
                    AdminError::InvalidCommand(format!("account not found: {account_id}"))
                })?;
                let save = account.saves.get_mut(&character_index).ok_or_else(|| {
                    AdminError::InvalidCommand(format!("character save not found: {character_id}"))
                })?;
                let character_name = save.character.name.clone();
                let mut systems = stage5_systems_from_save(save);
                let listing = systems
                    .auction
                    .iter_mut()
                    .find(|listing| listing.id == *listing_id)
                    .ok_or_else(|| {
                        AdminError::InvalidCommand(format!(
                            "auction listing not found: {listing_id}"
                        ))
                    })?;
                listing.cancelled = true;
                if *notify_seller {
                    push_stage5_mail(
                        &mut systems,
                        character_name,
                        "GM Market Notice",
                        &format!("Listing {listing_id} was cancelled by GM moderation."),
                        0,
                        Vec::new(),
                    );
                }
                write_stage5_systems_to_save(save, systems)?;
                Ok(format!(
                    "auction listing {listing_id} cancelled for {}",
                    character_player_id(&account_id, character_index)
                ))
            }),
            AdminCommand::MarketExpireListing {
                character_id,
                listing_id,
                notify_seller,
            } => self.mutate_account_store(|store, _| {
                let (account_id, character_index) = resolve_character_target(store, character_id)?;
                let account = store.accounts.get_mut(&account_id).ok_or_else(|| {
                    AdminError::InvalidCommand(format!("account not found: {account_id}"))
                })?;
                let save = account.saves.get_mut(&character_index).ok_or_else(|| {
                    AdminError::InvalidCommand(format!("character save not found: {character_id}"))
                })?;
                let character_name = save.character.name.clone();
                let mut systems = stage5_systems_from_save(save);
                let listing = systems
                    .auction
                    .iter_mut()
                    .find(|listing| listing.id == *listing_id)
                    .ok_or_else(|| {
                        AdminError::InvalidCommand(format!(
                            "auction listing not found: {listing_id}"
                        ))
                    })?;
                listing.expired = true;
                if *notify_seller {
                    push_stage5_mail(
                        &mut systems,
                        character_name,
                        "GM Market Notice",
                        &format!("Listing {listing_id} was expired by GM moderation."),
                        0,
                        Vec::new(),
                    );
                }
                write_stage5_systems_to_save(save, systems)?;
                Ok(format!(
                    "auction listing {listing_id} expired for {}",
                    character_player_id(&account_id, character_index)
                ))
            }),
            AdminCommand::MarketDeleteListing {
                character_id,
                listing_id,
                reason_message,
            } => self.mutate_account_store(|store, _| {
                let (account_id, character_index) = resolve_character_target(store, character_id)?;
                let account = store.accounts.get_mut(&account_id).ok_or_else(|| {
                    AdminError::InvalidCommand(format!("account not found: {account_id}"))
                })?;
                let save = account.saves.get_mut(&character_index).ok_or_else(|| {
                    AdminError::InvalidCommand(format!("character save not found: {character_id}"))
                })?;
                let character_name = save.character.name.clone();
                let mut systems = stage5_systems_from_save(save);
                let index = systems
                    .auction
                    .iter()
                    .position(|listing| listing.id == *listing_id)
                    .ok_or_else(|| {
                        AdminError::InvalidCommand(format!(
                            "auction listing not found: {listing_id}"
                        ))
                    })?;
                systems.auction.remove(index);
                if let Some(reason_message) = optional_string(reason_message.clone()) {
                    push_stage5_mail(
                        &mut systems,
                        character_name,
                        "GM Market Notice",
                        &reason_message,
                        0,
                        Vec::new(),
                    );
                }
                write_stage5_systems_to_save(save, systems)?;
                Ok(format!(
                    "auction listing {listing_id} deleted for {}",
                    character_player_id(&account_id, character_index)
                ))
            }),
            AdminCommand::GuildRemoveMember {
                guild_name,
                member_name,
            } => self.mutate_account_store(|store, _| {
                let guild_name = required_string("guild_name", guild_name)?;
                let member_name = required_string("member_name", member_name)?;
                let mut updated = 0usize;
                for account in store.accounts.values_mut() {
                    for save in account.saves.values_mut() {
                        let mut systems = stage5_systems_from_save(save);
                        if systems.guild.name.eq_ignore_ascii_case(&guild_name) {
                            let before = systems.guild.members.len();
                            systems
                                .guild
                                .members
                                .retain(|member| !member.eq_ignore_ascii_case(&member_name));
                            if save.character.name.eq_ignore_ascii_case(&member_name) {
                                systems.guild = Default::default();
                            }
                            if systems.guild.members.len() != before
                                || save.character.name.eq_ignore_ascii_case(&member_name)
                            {
                                write_stage5_systems_to_save(save, systems)?;
                                updated = updated.saturating_add(1);
                            }
                        }
                    }
                }
                Ok(format!(
                    "guild {guild_name} removed member {member_name}; updated saves={updated}"
                ))
            }),
            AdminCommand::GuildSendMessage {
                guild_name,
                message,
            } => self.mutate_account_store(|store, _| {
                let guild_name = required_string("guild_name", guild_name)?;
                let message = required_string("message", message)?;
                let entry = format!("[GM {operator_id}] {message}");
                let mut updated = 0usize;
                for account in store.accounts.values_mut() {
                    for save in account.saves.values_mut() {
                        let mut systems = stage5_systems_from_save(save);
                        if systems.guild.name.eq_ignore_ascii_case(&guild_name) {
                            systems.guild.chat_log.push(entry.clone());
                            write_stage5_systems_to_save(save, systems)?;
                            updated = updated.saturating_add(1);
                        }
                    }
                }
                Ok(format!(
                    "guild {guild_name} message appended to {updated} character save(s)"
                ))
            }),
            AdminCommand::NameListAdd {
                list_name,
                player_name,
            } => update_name_list(&self.name_list_root, list_name, player_name, true),
            AdminCommand::NameListRemove {
                list_name,
                player_name,
            } => update_name_list(&self.name_list_root, list_name, player_name, false),
            AdminCommand::NameListCreate { list_name } => {
                create_name_list(&self.name_list_root, list_name)
            }
            AdminCommand::NameListDelete { list_name } => {
                delete_name_list(&self.name_list_root, list_name)
            }
            AdminCommand::PublishContentBundle {
                bundle_id,
                category,
                record_key,
                payload_json,
            } => publish_content_bundle(
                &self.content_bundle_dir,
                bundle_id,
                category,
                record_key,
                payload_json,
                operator_id,
                reason,
                accepted_at_ms,
            ),
            AdminCommand::ServerControl { action, target } => {
                self.execute_server_control(action, target.as_deref(), operator_id, reason)
            }
            _ => Err(AdminError::UnsupportedCommand(format!("{command:?}"))),
        }
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
    gateway_http_response_body(&response)
}

fn get_json_from_gateway(gateway_url: &str) -> Result<String, String> {
    let (host, path) = parse_http_url(gateway_url)?;
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
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| format!("gateway request write failed: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("gateway response read failed: {error}"))?;
    gateway_http_response_body(&response)
}

fn gateway_http_response_body(response: &str) -> Result<String, String> {
    let (head, response_body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "gateway response was not valid HTTP".to_string())?;
    let status_line = head.lines().next().unwrap_or_default();
    if !status_line.contains(" 2") {
        return Err(format!(
            "gateway endpoint rejected request: {status_line} {response_body}"
        ));
    }
    if head
        .lines()
        .any(|line| line.eq_ignore_ascii_case("Transfer-Encoding: chunked"))
    {
        decode_chunked_body(response_body)
    } else {
        Ok(response_body.to_string())
    }
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

fn resolve_character_target(
    store: &AccountStore,
    character_id: &str,
) -> Result<(String, i32), AdminError> {
    let character_id = required_string("character_id", character_id)?;
    if let Some((account_id, raw_index)) = character_id.split_once(':') {
        let account_id = required_string("account_id", account_id)?;
        let character_index = raw_index.trim().parse::<i32>().map_err(|_| {
            AdminError::InvalidCommand(format!("invalid character index: {raw_index}"))
        })?;
        let account = store.accounts.get(&account_id).ok_or_else(|| {
            AdminError::InvalidCommand(format!("account not found: {account_id}"))
        })?;
        if account
            .characters
            .iter()
            .any(|character| character.index == character_index)
            || account.saves.contains_key(&character_index)
        {
            return Ok((account_id, character_index));
        }
        return Err(AdminError::InvalidCommand(format!(
            "character not found: {character_id}"
        )));
    }
    for (account_id, account) in &store.accounts {
        for character in &account.characters {
            if character.name.eq_ignore_ascii_case(&character_id)
                || character.index.to_string() == character_id
            {
                return Ok((account_id.clone(), character.index));
            }
        }
    }
    Err(AdminError::InvalidCommand(format!(
        "character not found: {character_id}"
    )))
}

fn character_player_id(account_id: &str, character_index: i32) -> String {
    format!("{account_id}:{character_index}")
}

fn stage5_systems_from_save(save: &CharacterSaveRecord) -> Stage5SystemsState {
    save.stage5_systems_json
        .as_deref()
        .and_then(|state| serde_json::from_str::<Stage5SystemsState>(state).ok())
        .unwrap_or_default()
}

fn write_stage5_systems_to_save(
    save: &mut CharacterSaveRecord,
    systems: Stage5SystemsState,
) -> Result<(), AdminError> {
    save.stage5_systems_json = Some(serde_json::to_string(&systems).map_err(|error| {
        AdminError::Repository(format!("encode stage5 systems failed: {error}"))
    })?);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AdminNpcFlagState {
    index: u32,
    value: bool,
}

fn npc_flags_from_save(save: &CharacterSaveRecord) -> Vec<AdminNpcFlagState> {
    save.npc_flag_states_json
        .iter()
        .filter_map(|entry| serde_json::from_str::<AdminNpcFlagState>(entry).ok())
        .collect()
}

fn write_npc_flags_to_save(
    save: &mut CharacterSaveRecord,
    mut flags: Vec<AdminNpcFlagState>,
) -> Result<(), AdminError> {
    flags.sort_by_key(|flag| flag.index);
    save.npc_flag_states_json = flags
        .iter()
        .map(|flag| {
            serde_json::to_string(flag)
                .map_err(|error| AdminError::Repository(format!("encode npc flag failed: {error}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(())
}

fn set_npc_flag(
    save: &mut CharacterSaveRecord,
    flag_index: u32,
    active: bool,
) -> Result<(), AdminError> {
    if !(1..=1999).contains(&flag_index) {
        return Err(AdminError::InvalidCommand(
            "flag_index must be between 1 and 1999".into(),
        ));
    }
    let mut flags = npc_flags_from_save(save);
    if let Some(flag) = flags.iter_mut().find(|flag| flag.index == flag_index) {
        flag.value = active;
    } else {
        flags.push(AdminNpcFlagState {
            index: flag_index,
            value: active,
        });
    }
    flags.retain(|flag| flag.value);
    write_npc_flags_to_save(save, flags)
}

fn push_stage5_mail(
    systems: &mut Stage5SystemsState,
    to: String,
    subject: &str,
    body: &str,
    gold: u32,
    items: Vec<String>,
) {
    let id = systems
        .mail
        .iter()
        .map(|mail| mail.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    systems.mail.push(Stage5MailMessage {
        id,
        from: "GM System".to_string(),
        to,
        subject: subject.to_string(),
        body: body.to_string(),
        gold,
        items,
        item_states_json: Vec::new(),
        opened: false,
        locked: false,
        claimed: false,
        deleted: false,
    });
}

fn name_list_root_from_env() -> PathBuf {
    env::var("ADMIN_NAMELIST_ROOT")
        .or_else(|_| env::var("MIR2_NAMELIST_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".mir2-data/NameLists"))
}

fn content_bundle_dir_from_env() -> PathBuf {
    env::var("ADMIN_CONTENT_BUNDLE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".mir2-data/admin-content-bundles"))
}

fn sanitized_file_stem(field: &str, value: &str) -> Result<String, AdminError> {
    let value = required_string(field, value)?;
    let mut stem = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            stem.push(ch);
        } else if ch.is_whitespace() {
            stem.push('-');
        } else {
            return Err(AdminError::InvalidCommand(format!(
                "{field} contains unsupported path character: {ch}"
            )));
        }
    }
    let stem = stem.trim_matches('.').trim_matches('-').to_string();
    if stem.is_empty() || stem.contains("..") {
        return Err(AdminError::InvalidCommand(format!("{field} is not safe")));
    }
    Ok(stem)
}

fn name_list_path(root: &PathBuf, list_name: &str) -> Result<PathBuf, AdminError> {
    let mut stem = sanitized_file_stem("list_name", list_name)?;
    if !stem.ends_with(".txt") {
        stem.push_str(".txt");
    }
    Ok(root.join(stem))
}

fn update_name_list(
    root: &PathBuf,
    list_name: &str,
    player_name: &str,
    add: bool,
) -> Result<String, AdminError> {
    let player_name = required_string("player_name", player_name)?;
    let path = name_list_path(root, list_name)?;
    fs::create_dir_all(root).map_err(|error| {
        AdminError::Repository(format!(
            "create namelist root {} failed: {error}",
            root.display()
        ))
    })?;
    let mut players = fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if add {
        if !players
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&player_name))
        {
            players.push(player_name.clone());
        }
    } else {
        players.retain(|existing| !existing.eq_ignore_ascii_case(&player_name));
    }
    players.sort_by_key(|value| value.to_ascii_lowercase());
    let mut body = players.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    fs::write(&path, body).map_err(|error| {
        AdminError::Repository(format!("write namelist {} failed: {error}", path.display()))
    })?;
    Ok(format!(
        "namelist {} {} {}; players={}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown"),
        if add { "added" } else { "removed" },
        player_name,
        players.len()
    ))
}

fn create_name_list(root: &PathBuf, list_name: &str) -> Result<String, AdminError> {
    let path = name_list_path(root, list_name)?;
    fs::create_dir_all(root).map_err(|error| {
        AdminError::Repository(format!(
            "create namelist root {} failed: {error}",
            root.display()
        ))
    })?;
    if path.exists() {
        return Err(AdminError::InvalidCommand(format!(
            "namelist already exists: {}",
            path.display()
        )));
    }
    fs::write(&path, "").map_err(|error| {
        AdminError::Repository(format!(
            "create namelist {} failed: {error}",
            path.display()
        ))
    })?;
    Ok(format!(
        "namelist {} created",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
    ))
}

fn delete_name_list(root: &PathBuf, list_name: &str) -> Result<String, AdminError> {
    let path = name_list_path(root, list_name)?;
    if !path.exists() {
        return Err(AdminError::InvalidCommand(format!(
            "namelist not found: {}",
            path.display()
        )));
    }
    fs::remove_file(&path).map_err(|error| {
        AdminError::Repository(format!(
            "delete namelist {} failed: {error}",
            path.display()
        ))
    })?;
    Ok(format!(
        "namelist {} deleted",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
    ))
}

fn publish_content_bundle(
    root: &PathBuf,
    bundle_id: &str,
    category: &str,
    record_key: &str,
    payload_json: &Value,
    operator_id: &str,
    reason: &str,
    updated_at_ms: u64,
) -> Result<String, AdminError> {
    let bundle_id = sanitized_file_stem("bundle_id", bundle_id)?;
    let category = required_string("category", category)?;
    let record_key = required_string("record_key", record_key)?;
    fs::create_dir_all(root).map_err(|error| {
        AdminError::Repository(format!(
            "create content bundle dir {} failed: {error}",
            root.display()
        ))
    })?;
    let payload = serde_json::json!({
        "bundleId": bundle_id,
        "category": category,
        "recordKey": record_key,
        "payload": payload_json,
        "operatorId": operator_id,
        "reason": reason,
        "updatedAtMs": updated_at_ms,
    });
    let body = serde_json::to_vec_pretty(&payload).map_err(|error| {
        AdminError::Repository(format!("encode content bundle failed: {error}"))
    })?;
    let path = root.join(format!("{bundle_id}.json"));
    fs::write(&path, body).map_err(|error| {
        AdminError::Repository(format!(
            "write content bundle {} failed: {error}",
            path.display()
        ))
    })?;
    Ok(format!(
        "content bundle {bundle_id} published to {}",
        path.display()
    ))
}

fn canonical_control_action(action: &str) -> Result<String, AdminError> {
    let action = required_string("action", action)?
        .replace('-', "_")
        .replace(' ', "_")
        .to_ascii_lowercase();
    let canonical = match action.as_str() {
        "status" | "health" => "status",
        "reload_npcs" | "reload_npc" | "reloadnpc" => "reload_npcs",
        "reload_drops" | "reload_drop" | "reloaddrops" => "reload_drops",
        "reload_line_message" | "reload_line_messages" | "reloadlinemessage" => {
            "reload_line_message"
        }
        "clear_blocked_ips" | "clear_blocked_ip" | "clearblockedips" => "clear_blocked_ips",
        "start" | "start_server" => "start",
        "stop" | "stop_server" => "stop",
        "reboot" | "restart" => "reboot",
        "close" | "shutdown" => "close",
        other => {
            return Err(AdminError::InvalidCommand(format!(
                "unsupported server control action: {other}"
            )))
        }
    };
    Ok(canonical.to_string())
}

fn control_action_can_fallback(action: &str) -> bool {
    matches!(
        action,
        "status" | "reload_npcs" | "reload_drops" | "reload_line_message" | "clear_blocked_ips"
    )
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
            other => {
                let message = self.domain.execute_console_command(
                    other,
                    &envelope.operator.id,
                    &envelope.reason,
                    envelope.created_at_ms,
                )?;
                Ok(CommandExecutionResult {
                    status: CommandStatus::Succeeded,
                    message,
                })
            }
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
    read_models: Arc<AdminReadModelStore>,
    admin_store: Option<PostgresAdminRepository>,
}

impl Default for AdminApiState {
    fn default() -> Self {
        Self::from_env().expect("admin api state should initialize")
    }
}

impl AdminApiState {
    pub fn from_env() -> Result<Self, AdminError> {
        let domain = AccountStoreSystemMailDomain::from_env();
        let read_models = Arc::new(AdminReadModelStore::from_config(domain.fallback_config()));
        let postgres_repository = PostgresAdminRepository::from_env()?;
        let admin_store = postgres_repository.clone();
        let (command_store, audit_store, approval_store) = match postgres_repository {
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
                SystemMailExecutor::new(domain),
                command_store,
                audit_store,
                approval_store,
            ))),
            read_models,
            admin_store,
        })
    }
}

pub fn admin_router() -> Router {
    admin_router_with_state(AdminApiState::default())
}

pub fn admin_router_with_state(state: AdminApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/admin/auth/me", get(auth_me))
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
        .route(
            "/admin/gameplay-events/summary",
            get(get_gameplay_event_summary),
        )
        .route("/admin/gameplay-events", get(list_gameplay_events))
        .route("/admin/timeline", get(list_admin_timeline))
        .route("/admin/system-mail/outbox", get(list_system_mail_outbox))
        .route("/admin/read/dashboard", get(read_dashboard))
        .route("/admin/read/accounts", get(read_accounts))
        .route("/admin/read/accounts/:account_id", get(read_account_detail))
        .route("/admin/read/players", get(read_players))
        .route("/admin/read/players/:player_id", get(read_player_detail))
        .route("/admin/read/economy", get(read_economy))
        .route("/admin/read/market", get(read_market))
        .route("/admin/read/guilds", get(read_guilds))
        .route("/admin/read/namelists", get(read_namelists))
        .route("/admin/read/content", get(read_content))
        .route("/admin/read/control", get(read_control))
        .route("/admin/read/activities", get(read_activities))
        .route("/admin/read/servers", get(read_servers))
        .route("/admin/read/risk", get(read_risk))
        .route("/admin/read/operators", get(read_operators))
        .route("/admin/activities", post(upsert_activity))
        .route("/admin/economy/price-feeds", post(upsert_price_feed))
        .route("/admin/risk/trade-edges", post(upsert_trade_graph_edge))
        .route("/admin/servers/zones", post(upsert_zone_runtime))
        .route("/admin/operators", post(upsert_operator))
        .route("/admin/commands/send-system-mail", post(submit_system_mail))
        .route("/admin/commands/grant-item", post(submit_grant_item))
        .route(
            "/admin/commands/grant-currency",
            post(submit_grant_currency),
        )
        .route("/admin/commands/kick-player", post(submit_kick_player))
        .route("/admin/commands/ban-account", post(submit_ban_account))
        .route("/admin/commands/console", post(submit_console_command))
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
pub struct SubmitConsoleCommandRequest {
    pub command_id: Option<String>,
    pub trace_id: Option<String>,
    pub approval_id: Option<String>,
    pub target: AdminTarget,
    pub command: AdminCommand,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertAdminActivityRequest {
    pub activity_id: Option<String>,
    pub name: String,
    pub activity_type: String,
    pub status: Option<String>,
    pub signal: Option<String>,
    pub start_at_ms: Option<u64>,
    pub reward: Option<String>,
    pub condition: Option<String>,
    #[serde(default)]
    pub realms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertPriceFeedRequest {
    pub item: String,
    pub latest_price: u64,
    pub sample_count: usize,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertTradeGraphEdgeRequest {
    pub edge_id: Option<String>,
    pub from: String,
    pub to: String,
    pub signal: String,
    pub risk: String,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertZoneRuntimeRequest {
    pub zone_id: Option<String>,
    pub name: String,
    pub status: Option<String>,
    pub host: Option<String>,
    pub process_id: Option<String>,
    pub map_count: Option<usize>,
    pub player_count: Option<usize>,
    pub tick_rate: Option<u32>,
    pub uptime_seconds: Option<u64>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertAdminOperatorRequest {
    pub operator_id: Option<String>,
    pub email: String,
    pub role: String,
    pub status: Option<String>,
    pub token: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayEventRecord {
    pub event_id: String,
    pub event_type: String,
    pub zone_id: String,
    pub command_kind: String,
    pub account_id: Option<String>,
    pub character_index: Option<i32>,
    pub character_name: Option<String>,
    pub packet_count: u64,
    pub snapshot_tick: u64,
    pub occurred_at_ms: u64,
    pub payload_json: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayEventQuery {
    pub limit: Option<usize>,
    pub zone_id: Option<String>,
    pub command_kind: Option<String>,
    pub account_id: Option<String>,
    pub character_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayEventsResponse {
    pub degraded: bool,
    pub error: Option<String>,
    pub records: Vec<GameplayEventRecord>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayEventSummaryQuery {
    pub window_seconds: Option<u64>,
    pub limit: Option<usize>,
    pub zone_id: Option<String>,
    pub command_kind: Option<String>,
    pub max_lag_seconds: Option<u64>,
    pub min_events: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayEventCommandSummary {
    pub command_kind: String,
    pub event_count: u64,
    pub last_occurred_at_ms: u64,
    pub max_snapshot_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayEventReadinessAlert {
    pub level: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayEventSummaryResponse {
    pub degraded: bool,
    pub ready: bool,
    pub error: Option<String>,
    pub generated_at_ms: u64,
    pub window_seconds: u64,
    pub max_lag_ms: u64,
    pub min_event_count: u64,
    pub total_count: u64,
    pub last_occurred_at_ms: Option<u64>,
    pub lag_ms: Option<u64>,
    pub alerts: Vec<GameplayEventReadinessAlert>,
    pub commands: Vec<GameplayEventCommandSummary>,
}

#[derive(Debug, Clone, Default)]
struct GameplayEventSummary {
    total_count: u64,
    last_occurred_at_ms: Option<u64>,
    commands: Vec<GameplayEventCommandSummary>,
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
pub struct AdminDashboardReadModel {
    pub source: String,
    pub generated_at_ms: u64,
    pub account_count: usize,
    pub character_count: usize,
    pub online_now: usize,
    pub online_source: String,
    pub total_gold: u64,
    pub total_credit: u64,
    pub active_ban_count: usize,
    pub hot_maps: Vec<AdminMapPopulation>,
    pub services: Vec<AdminServiceStatus>,
    pub audit_record_count: usize,
    pub outbox_receipt_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminMapPopulation {
    pub map_file_name: String,
    pub map_title: String,
    pub character_count: usize,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminServiceStatus {
    pub name: String,
    pub status: String,
    pub detail: String,
    pub latency_ms: Option<u64>,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAccountsReadModel {
    pub source: String,
    pub generated_at_ms: u64,
    pub accounts: Vec<AdminAccountSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAccountSummary {
    pub account_id: String,
    pub password_configured: bool,
    pub character_count: usize,
    pub storage_size: u16,
    pub has_storage_password: bool,
    pub is_banned: bool,
    pub ban_reason: Option<String>,
    pub ban_until_ms: Option<u64>,
    pub banned_at_ms: Option<u64>,
    pub store_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAccountDetail {
    pub summary: AdminAccountSummary,
    pub characters: Vec<AdminPlayerSummary>,
    pub has_expanded_storage: bool,
    pub expanded_storage_expiry_time_binary_datetime: i64,
    pub storage_password_last_set_binary_datetime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPlayersReadModel {
    pub source: String,
    pub generated_at_ms: u64,
    pub players: Vec<AdminPlayerSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPlayerSummary {
    pub player_id: String,
    pub account_id: String,
    pub character_index: i32,
    pub character_name: String,
    pub class_name: String,
    pub gender: String,
    pub level: u16,
    pub map_file_name: String,
    pub map_title: String,
    pub position_x: i32,
    pub position_y: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub gold: u64,
    pub credit: u64,
    pub pk_points: i32,
    pub chat_banned: bool,
    pub chat_ban_until_ms: Option<u64>,
    pub status: String,
    pub online: bool,
    pub online_source: Option<String>,
    pub player_object_id: Option<u32>,
    pub runtime_tick: Option<u64>,
    pub store_version: Option<i64>,
    pub save_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPlayerDetail {
    pub summary: AdminPlayerSummary,
    pub inventory_count: usize,
    pub belt_count: usize,
    pub storage_count: usize,
    pub equipment_count: usize,
    pub quest_state_count: usize,
    pub skill_state_count: usize,
    pub npc_flag_count: usize,
    pub active_npc_flags: Vec<AdminNpcFlagRecord>,
    pub mail_count: usize,
    pub unclaimed_mail_count: usize,
    pub auction_listing_count: usize,
    pub group_member_count: usize,
    pub guild_name: Option<String>,
    pub active_ban_reason: Option<String>,
    pub ban_until_ms: Option<u64>,
    pub banned_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminNpcFlagRecord {
    pub index: u32,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminEconomyReadModel {
    pub source: String,
    pub generated_at_ms: u64,
    pub assets: Vec<AdminEconomyAsset>,
    pub gold_distribution: Vec<AdminDistributionBucket>,
    pub price_feeds: Vec<AdminPriceFeedRecord>,
    pub price_feed_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminMarketReadModel {
    pub source: String,
    pub generated_at_ms: u64,
    pub listings: Vec<AdminMarketListingRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminMarketListingRecord {
    pub owner_player_id: String,
    pub account_id: String,
    pub character_index: i32,
    pub character_name: String,
    pub listing_id: u32,
    pub seller: String,
    pub item_key: String,
    pub price: u32,
    pub sold: bool,
    pub cancelled: bool,
    pub expired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminGuildsReadModel {
    pub source: String,
    pub generated_at_ms: u64,
    pub guilds: Vec<AdminGuildRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminGuildRecord {
    pub guild_name: String,
    pub member_count: usize,
    pub members: Vec<String>,
    pub ranks: Vec<String>,
    pub chat_log: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminNameListsReadModel {
    pub source: String,
    pub generated_at_ms: u64,
    pub root: String,
    pub lists: Vec<AdminNameListRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminNameListRecord {
    pub list_name: String,
    pub path: String,
    pub player_count: usize,
    pub players: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminContentReadModel {
    pub generated_at_ms: u64,
    pub categories: Vec<AdminContentCategoryRecord>,
    pub bundles: Vec<AdminContentBundleRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminContentCategoryRecord {
    pub category: String,
    pub source: String,
    pub total_records: usize,
    pub status: String,
    pub editable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminContentBundleRecord {
    pub bundle_id: String,
    pub category: String,
    pub record_key: String,
    pub path: String,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminControlReadModel {
    pub generated_at_ms: u64,
    pub gateway_control_configured: bool,
    pub control_actions: Vec<String>,
    pub logs: Vec<AdminLogTailRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminLogTailRecord {
    pub name: String,
    pub path: String,
    pub configured: bool,
    pub lines: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminEconomyAsset {
    pub asset: String,
    pub total: u64,
    pub holders: usize,
    pub average: u64,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminDistributionBucket {
    pub key: String,
    pub label: String,
    pub value: u8,
    pub amount: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminPriceFeedRecord {
    pub item: String,
    pub latest_price: u64,
    pub sample_count: usize,
    pub source: String,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminActivitiesReadModel {
    pub source: String,
    pub generated_at_ms: u64,
    pub configured: bool,
    pub activities: Vec<AdminActivityRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminActivityRecord {
    pub activity_id: String,
    pub name: String,
    pub start_at_ms: Option<u64>,
    pub activity_type: String,
    pub status: String,
    pub signal: String,
    pub reward: String,
    pub condition: String,
    pub realms: Vec<String>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminServersReadModel {
    pub generated_at_ms: u64,
    pub account_store_source: String,
    pub account_count: usize,
    pub character_count: usize,
    pub zones_online: usize,
    pub zones_source: String,
    pub zone_runtime_configured: bool,
    pub zones: Vec<AdminZoneRuntimeRecord>,
    pub services: Vec<AdminServiceStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminZoneRuntimeRecord {
    pub zone_id: String,
    pub name: String,
    pub status: String,
    pub host: String,
    pub process_id: String,
    pub map_count: usize,
    pub player_count: usize,
    pub tick_rate: u32,
    pub uptime_seconds: u64,
    pub source: String,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminOperatorsReadModel {
    pub source: String,
    pub generated_at_ms: u64,
    pub configured: bool,
    pub operators: Vec<AdminOperatorRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminOperatorRecord {
    pub operator_id: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub permissions: Vec<String>,
    pub token_configured: bool,
    pub updated_at_ms: u64,
    pub last_authenticated_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAuthMeResponse {
    pub source: String,
    pub operator: AdminOperatorRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminRiskReadModel {
    pub source: String,
    pub generated_at_ms: u64,
    pub cases: Vec<AdminRiskCase>,
    pub graph: Vec<AdminRiskGraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminRiskCase {
    pub player_id: String,
    pub account_id: String,
    pub character_name: String,
    pub signal: String,
    pub risk: String,
    pub evidence: String,
    pub ban_until_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminRiskGraphEdge {
    pub edge_id: String,
    pub from: String,
    pub to: String,
    pub signal: String,
    pub risk: String,
    pub evidence: String,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone)]
struct AdminReadModelStore {
    source: AdminReadModelSource,
    default_character: CharacterRecord,
}

#[derive(Debug, Clone)]
enum AdminReadModelSource {
    JsonPath(PathBuf),
    Postgres(String),
    Memory(Arc<Mutex<AccountStore>>),
}

#[derive(Debug, Clone)]
struct AccountReadSnapshot {
    source: String,
    accounts: Vec<AccountReadRecord>,
}

#[derive(Debug, Clone)]
struct AccountReadRecord {
    account_id: String,
    account: AccountRecord,
    store_version: Option<i64>,
    saves: BTreeMap<i32, CharacterSaveReadRecord>,
}

#[derive(Debug, Clone)]
struct CharacterSaveReadRecord {
    save: CharacterSaveRecord,
    save_version: Option<i64>,
}

#[derive(Debug, Clone, Default)]
struct GatewayPresenceSnapshot {
    source: String,
    sessions: Vec<GatewaySessionRecord>,
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GatewaySessionsResponse {
    source: String,
    sessions: Vec<GatewaySessionRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GatewaySessionRecord {
    key: GatewaySessionKey,
    character_name: String,
    map_file_name: Option<String>,
    player_object_id: Option<u32>,
    player_hp: Option<i32>,
    player_max_hp: Option<i32>,
    gold: u32,
    tick: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
struct GatewaySessionKey {
    account_id: String,
    character_index: i32,
}

impl AdminReadModelStore {
    fn from_config(config: &SimulationConfig) -> Self {
        let source =
            if config.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth {
                env::var("MIR2_ACCOUNT_STORE_DATABASE_URL")
                    .ok()
                    .or_else(|| config.account_store_database_url.clone())
                    .filter(|value| !value.trim().is_empty())
                    .map(AdminReadModelSource::Postgres)
            } else {
                config
                    .account_store_path
                    .clone()
                    .or_else(|| env::var("ADMIN_ACCOUNT_STORE_PATH").ok().map(PathBuf::from))
                    .or_else(|| env::var("MIR2_ACCOUNT_STORE_PATH").ok().map(PathBuf::from))
                    .map(AdminReadModelSource::JsonPath)
            }
            .unwrap_or_else(|| AdminReadModelSource::Memory(config.account_store.clone()));
        Self {
            source,
            default_character: config.default_character.clone(),
        }
    }

    fn load_snapshot(&self) -> Result<AccountReadSnapshot, AdminError> {
        match &self.source {
            AdminReadModelSource::JsonPath(path) => {
                read_json_account_snapshot(path, self.default_character.clone())
            }
            AdminReadModelSource::Postgres(database_url) => {
                read_postgres_account_snapshot(database_url, self.default_character.clone())
            }
            AdminReadModelSource::Memory(store) => {
                let store = store
                    .lock()
                    .map_err(|_| AdminError::Repository("account read model lock poisoned".into()))?
                    .clone();
                Ok(snapshot_from_store("memory_account_store", store))
            }
        }
    }
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
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
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
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
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

async fn list_gameplay_events(
    Query(query): Query<GameplayEventQuery>,
) -> Result<Json<GameplayEventsResponse>, ApiError> {
    let records = tokio::task::spawn_blocking(move || fetch_clickhouse_gameplay_events(&query))
        .await
        .map_err(join_error)?;
    match records {
        Ok(records) => Ok(Json(GameplayEventsResponse {
            degraded: false,
            error: None,
            records,
        })),
        Err(error) => Ok(Json(GameplayEventsResponse {
            degraded: true,
            error: Some(error.message),
            records: Vec::new(),
        })),
    }
}

async fn get_gameplay_event_summary(
    Query(query): Query<GameplayEventSummaryQuery>,
) -> Result<Json<GameplayEventSummaryResponse>, ApiError> {
    let generated_at_ms = now_ms();
    let window_seconds = normalized_gameplay_event_summary_window_seconds(query.window_seconds);
    let max_lag_ms =
        normalized_gameplay_event_summary_max_lag_ms(query.max_lag_seconds, window_seconds);
    let min_event_count = normalized_gameplay_event_summary_min_events(query.min_events);
    let summary = tokio::task::spawn_blocking(move || {
        fetch_clickhouse_gameplay_event_summary(&query, generated_at_ms, window_seconds)
    })
    .await
    .map_err(join_error)?;
    match summary {
        Ok(summary) => {
            let lag_ms = summary
                .last_occurred_at_ms
                .map(|last_occurred_at_ms| generated_at_ms.saturating_sub(last_occurred_at_ms));
            let alerts =
                gameplay_event_readiness_alerts(&summary, lag_ms, max_lag_ms, min_event_count);
            Ok(Json(GameplayEventSummaryResponse {
                degraded: false,
                ready: alerts.is_empty(),
                error: None,
                generated_at_ms,
                window_seconds,
                max_lag_ms,
                min_event_count,
                total_count: summary.total_count,
                last_occurred_at_ms: summary.last_occurred_at_ms,
                lag_ms,
                alerts,
                commands: summary.commands,
            }))
        }
        Err(error) => Ok(Json(GameplayEventSummaryResponse {
            degraded: true,
            ready: false,
            error: Some(error.message.clone()),
            generated_at_ms,
            window_seconds,
            max_lag_ms,
            min_event_count,
            total_count: 0,
            last_occurred_at_ms: None,
            lag_ms: None,
            alerts: vec![GameplayEventReadinessAlert {
                level: "critical".into(),
                code: "clickhouse_unavailable".into(),
                message: error.message,
            }],
            commands: Vec::new(),
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
        let mut receipts = control.executor().domain().receipts().to_vec();
        drop(control);
        if let Some(admin_store) = state.admin_store.as_ref() {
            for receipt in admin_store.list_system_mail_receipts(100)? {
                if !receipts
                    .iter()
                    .any(|existing| existing.outbox_id == receipt.outbox_id)
                {
                    receipts.push(receipt);
                }
            }
        }
        receipts.sort_by(|left, right| {
            right
                .accepted_at_ms
                .cmp(&left.accepted_at_ms)
                .then_with(|| right.outbox_id.cmp(&left.outbox_id))
        });
        receipts.truncate(100);
        Ok::<_, ApiError>(receipts)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(receipts))
}

async fn auth_me(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
) -> Result<Json<AdminAuthMeResponse>, ApiError> {
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
    let now = now_ms();
    Ok(Json(AdminAuthMeResponse {
        source: if operator_auth_backend_is_postgres() {
            "postgres_admin_operators".into()
        } else {
            "operator_auth".into()
        },
        operator: AdminOperatorRecord {
            operator_id: operator.id,
            email: operator.email,
            role: operator.role,
            status: "Active".into(),
            permissions: operator
                .permissions
                .iter()
                .map(permission_text)
                .collect::<Vec<_>>(),
            token_configured: optional_bearer_token(&headers).is_some(),
            updated_at_ms: now,
            last_authenticated_at_ms: Some(now),
        },
    }))
}

async fn read_dashboard(
    State(state): State<AdminApiState>,
) -> Result<Json<AdminDashboardReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(move || build_dashboard_read_model(state))
        .await
        .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_accounts(
    State(state): State<AdminApiState>,
) -> Result<Json<AdminAccountsReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        let snapshot = state.read_models.load_snapshot().map_err(ApiError::from)?;
        Ok::<_, ApiError>(build_accounts_read_model(&snapshot))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_account_detail(
    State(state): State<AdminApiState>,
    Path(account_id): Path<String>,
) -> Result<Json<AdminAccountDetail>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        let snapshot = state.read_models.load_snapshot().map_err(ApiError::from)?;
        let presence = load_gateway_presence();
        build_account_detail(&snapshot, &account_id, Some(&presence)).ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("account not found: {account_id}"),
        })
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_players(
    State(state): State<AdminApiState>,
) -> Result<Json<AdminPlayersReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        let snapshot = state.read_models.load_snapshot().map_err(ApiError::from)?;
        let presence = load_gateway_presence();
        Ok::<_, ApiError>(AdminPlayersReadModel {
            source: snapshot.source.clone(),
            generated_at_ms: now_ms(),
            players: build_player_summaries(&snapshot, Some(&presence)),
        })
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_player_detail(
    State(state): State<AdminApiState>,
    Path(player_id): Path<String>,
) -> Result<Json<AdminPlayerDetail>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        let snapshot = state.read_models.load_snapshot().map_err(ApiError::from)?;
        let presence = load_gateway_presence();
        build_player_detail(&snapshot, &player_id, Some(&presence)).ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("player not found: {player_id}"),
        })
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_economy(
    State(state): State<AdminApiState>,
) -> Result<Json<AdminEconomyReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        let snapshot = state.read_models.load_snapshot().map_err(ApiError::from)?;
        let price_feeds = match state.admin_store.as_ref() {
            Some(repository) => repository.list_price_feeds(64).map_err(ApiError::from)?,
            None => Vec::new(),
        };
        Ok::<_, ApiError>(build_economy_read_model(
            &snapshot,
            price_feeds,
            state.admin_store.is_some(),
        ))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_market(
    State(state): State<AdminApiState>,
) -> Result<Json<AdminMarketReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        let snapshot = state.read_models.load_snapshot().map_err(ApiError::from)?;
        Ok::<_, ApiError>(build_market_read_model(&snapshot))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_guilds(
    State(state): State<AdminApiState>,
) -> Result<Json<AdminGuildsReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        let snapshot = state.read_models.load_snapshot().map_err(ApiError::from)?;
        Ok::<_, ApiError>(build_guilds_read_model(&snapshot))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_namelists() -> Result<Json<AdminNameListsReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(build_namelists_read_model)
        .await
        .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_content() -> Result<Json<AdminContentReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(build_content_read_model)
        .await
        .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_control() -> Result<Json<AdminControlReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(build_control_read_model)
        .await
        .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_activities(
    State(state): State<AdminApiState>,
) -> Result<Json<AdminActivitiesReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        build_activities_read_model(state.admin_store.as_ref()).map_err(ApiError::from)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn upsert_activity(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<UpsertAdminActivityRequest>,
) -> Result<Json<AdminActivityRecord>, ApiError> {
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
    require_operator_permission(&operator, Permission::ContentPublish)?;
    let repository = configured_admin_store(&state)?;
    let operator_id = operator.id.clone();
    let response = tokio::task::spawn_blocking(move || {
        repository
            .upsert_activity(request, &operator_id, now_ms())
            .map_err(ApiError::from)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn upsert_price_feed(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<UpsertPriceFeedRequest>,
) -> Result<Json<AdminPriceFeedRecord>, ApiError> {
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
    require_operator_permission(&operator, Permission::ContentPublish)?;
    let repository = configured_admin_store(&state)?;
    let response = tokio::task::spawn_blocking(move || {
        repository
            .upsert_price_feed(request, now_ms())
            .map_err(ApiError::from)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_servers(
    State(state): State<AdminApiState>,
) -> Result<Json<AdminServersReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        let snapshot = state.read_models.load_snapshot().map_err(ApiError::from)?;
        let presence = load_gateway_presence();
        let zones = match state.admin_store.as_ref() {
            Some(repository) => repository.list_zone_runtime(128).map_err(ApiError::from)?,
            None => Vec::new(),
        };
        Ok::<_, ApiError>(build_servers_read_model(
            &snapshot,
            &presence,
            zones,
            state.admin_store.is_some(),
        ))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn upsert_zone_runtime(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<UpsertZoneRuntimeRequest>,
) -> Result<Json<AdminZoneRuntimeRecord>, ApiError> {
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
    require_operator_permission(&operator, Permission::ContentPublish)?;
    let repository = configured_admin_store(&state)?;
    let response = tokio::task::spawn_blocking(move || {
        repository
            .upsert_zone_runtime(request, now_ms())
            .map_err(ApiError::from)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_risk(
    State(state): State<AdminApiState>,
) -> Result<Json<AdminRiskReadModel>, ApiError> {
    let response = tokio::task::spawn_blocking(move || {
        let snapshot = state.read_models.load_snapshot().map_err(ApiError::from)?;
        let graph = match state.admin_store.as_ref() {
            Some(repository) => repository
                .list_trade_graph_edges(128)
                .map_err(ApiError::from)?,
            None => Vec::new(),
        };
        Ok::<_, ApiError>(build_risk_read_model(
            &snapshot,
            graph,
            state.admin_store.is_some(),
        ))
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn upsert_trade_graph_edge(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<UpsertTradeGraphEdgeRequest>,
) -> Result<Json<AdminRiskGraphEdge>, ApiError> {
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
    require_operator_permission(&operator, Permission::ContentPublish)?;
    let repository = configured_admin_store(&state)?;
    let response = tokio::task::spawn_blocking(move || {
        repository
            .upsert_trade_graph_edge(request, now_ms())
            .map_err(ApiError::from)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn read_operators(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
) -> Result<Json<AdminOperatorsReadModel>, ApiError> {
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
    require_operator_permission(&operator, Permission::PermissionManage)?;
    let response = tokio::task::spawn_blocking(move || {
        build_operators_read_model(state.admin_store.as_ref()).map_err(ApiError::from)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn upsert_operator(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<UpsertAdminOperatorRequest>,
) -> Result<Json<AdminOperatorRecord>, ApiError> {
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
    require_operator_permission(&operator, Permission::PermissionManage)?;
    let repository = configured_admin_store(&state)?;
    let actor_id = operator.id.clone();
    let response = tokio::task::spawn_blocking(move || {
        repository
            .upsert_operator(request, &actor_id, now_ms())
            .map_err(ApiError::from)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(response))
}

async fn submit_system_mail(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<SubmitSystemMailRequest>,
) -> Result<Json<SubmitCommandResponse>, ApiError> {
    let now = now_ms();
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
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

async fn submit_console_command(
    State(state): State<AdminApiState>,
    headers: HeaderMap,
    Json(request): Json<SubmitConsoleCommandRequest>,
) -> Result<Json<SubmitCommandResponse>, ApiError> {
    let now = now_ms();
    let command_type = command_type(&request.command);
    let command_id = request
        .command_id
        .clone()
        .unwrap_or_else(|| format!("cmd-{command_type}-{now}"));
    submit_admin_http_command(
        state,
        headers,
        command_id,
        request
            .trace_id
            .unwrap_or_else(|| format!("trace-admin-{now}")),
        request.approval_id,
        request.target,
        request.reason,
        request.command,
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
    let operator = operator_from_headers(&headers, state.admin_store.as_ref())?;
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

fn read_json_account_snapshot(
    path: &PathBuf,
    default_character: CharacterRecord,
) -> Result<AccountReadSnapshot, AdminError> {
    match fs::read_to_string(path) {
        Ok(data) => {
            let store = serde_json::from_str::<AccountStore>(&data).map_err(|error| {
                AdminError::Repository(format!(
                    "decode json account store {} failed: {error}",
                    path.display()
                ))
            })?;
            Ok(snapshot_from_store("json_account_store", store))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut store = AccountStore::new(default_character);
            store.accounts.clear();
            Ok(snapshot_from_store("json_account_store_missing", store))
        }
        Err(error) => Err(AdminError::Repository(format!(
            "read json account store {} failed: {error}",
            path.display()
        ))),
    }
}

fn read_postgres_account_snapshot(
    database_url: &str,
    default_character: CharacterRecord,
) -> Result<AccountReadSnapshot, AdminError> {
    let mut client = Client::connect(database_url, NoTls).map_err(|error| {
        AdminError::Repository(format!(
            "postgres account read-model connect failed: {error}"
        ))
    })?;
    client
        .batch_execute(include_str!(
            "../../../infra/postgres/migrations/0001_core.sql"
        ))
        .map_err(|error| {
            AdminError::Repository(format!(
                "postgres account read-model migration failed: {error}"
            ))
        })?;
    let rows = client
        .query(
            "SELECT account_id, raw_json, store_version FROM accounts ORDER BY account_id",
            &[],
        )
        .map_err(|error| {
            AdminError::Repository(format!("postgres account read-model query failed: {error}"))
        })?;
    let mut accounts = BTreeMap::new();
    for row in rows {
        let account_id: String = row.get("account_id");
        let raw_json: Value = row.get("raw_json");
        let store_version: i64 = row.get("store_version");
        let account = serde_json::from_value::<AccountRecord>(raw_json).map_err(|error| {
            AdminError::Repository(format!(
                "decode postgres account raw_json failed for {account_id}: {error}"
            ))
        })?;
        let saves = account
            .saves
            .iter()
            .map(|(character_index, save)| {
                (
                    *character_index,
                    CharacterSaveReadRecord {
                        save: save.clone(),
                        save_version: None,
                    },
                )
            })
            .collect();
        accounts.insert(
            account_id.clone(),
            AccountReadRecord {
                account_id,
                account,
                store_version: Some(store_version),
                saves,
            },
        );
    }
    let save_rows = client
        .query(
            "SELECT account_id, character_index, snapshot_json, save_version
             FROM character_saves
             ORDER BY account_id, character_index",
            &[],
        )
        .map_err(|error| {
            AdminError::Repository(format!(
                "postgres character-save read-model query failed: {error}"
            ))
        })?;
    for row in save_rows {
        let account_id: String = row.get("account_id");
        let character_index: i32 = row.get("character_index");
        let snapshot_json: Value = row.get("snapshot_json");
        let save_version: i64 = row.get("save_version");
        let save =
            serde_json::from_value::<CharacterSaveRecord>(snapshot_json).map_err(|error| {
                AdminError::Repository(format!(
                "decode postgres character save failed for {account_id}/{character_index}: {error}"
            ))
            })?;
        let account = accounts.entry(account_id.clone()).or_insert_with(|| {
            let mut account = AccountRecord::new(default_character.clone());
            account.characters.clear();
            account.saves.clear();
            AccountReadRecord {
                account_id: account_id.clone(),
                account,
                store_version: None,
                saves: BTreeMap::new(),
            }
        });
        if !account
            .account
            .characters
            .iter()
            .any(|character| character.index == save.character.index)
        {
            account.account.characters.push(save.character.clone());
        }
        account.account.saves.insert(character_index, save.clone());
        account.saves.insert(
            character_index,
            CharacterSaveReadRecord {
                save,
                save_version: Some(save_version),
            },
        );
    }
    Ok(AccountReadSnapshot {
        source: "postgres_account_store".into(),
        accounts: accounts.into_values().collect(),
    })
}

fn snapshot_from_store(source: &str, store: AccountStore) -> AccountReadSnapshot {
    AccountReadSnapshot {
        source: source.to_string(),
        accounts: store
            .accounts
            .into_iter()
            .map(|(account_id, account)| {
                let saves = account
                    .saves
                    .iter()
                    .map(|(character_index, save)| {
                        (
                            *character_index,
                            CharacterSaveReadRecord {
                                save: save.clone(),
                                save_version: None,
                            },
                        )
                    })
                    .collect();
                AccountReadRecord {
                    account_id,
                    account,
                    store_version: None,
                    saves,
                }
            })
            .collect(),
    }
}

fn build_dashboard_read_model(state: AdminApiState) -> Result<AdminDashboardReadModel, ApiError> {
    let snapshot = state.read_models.load_snapshot().map_err(ApiError::from)?;
    let presence = load_gateway_presence();
    let players = build_player_summaries(&snapshot, Some(&presence));
    let (audit_record_count, outbox_receipt_count) = {
        let control = state.control_plane.lock().map_err(lock_error)?;
        (
            control.audit_records(100).len(),
            control.executor().domain().receipts().len(),
        )
    };
    Ok(AdminDashboardReadModel {
        source: snapshot.source.clone(),
        generated_at_ms: now_ms(),
        account_count: snapshot.accounts.len(),
        character_count: players.len(),
        online_now: presence.sessions.len(),
        online_source: presence.source.clone(),
        total_gold: players.iter().map(|player| player.gold).sum(),
        total_credit: players.iter().map(|player| player.credit).sum(),
        active_ban_count: players
            .iter()
            .filter(|player| player.status == "Banned")
            .count(),
        hot_maps: build_presence_hot_maps(&presence).unwrap_or_else(|| build_hot_maps(&players)),
        services: build_service_statuses(&snapshot, Some(&presence)),
        audit_record_count,
        outbox_receipt_count,
    })
}

fn build_accounts_read_model(snapshot: &AccountReadSnapshot) -> AdminAccountsReadModel {
    let now = now_ms();
    AdminAccountsReadModel {
        source: snapshot.source.clone(),
        generated_at_ms: now,
        accounts: snapshot
            .accounts
            .iter()
            .map(|account| account_summary(account, now))
            .collect(),
    }
}

fn build_account_detail(
    snapshot: &AccountReadSnapshot,
    account_id: &str,
    presence: Option<&GatewayPresenceSnapshot>,
) -> Option<AdminAccountDetail> {
    let now = now_ms();
    let account = snapshot
        .accounts
        .iter()
        .find(|account| account.account_id.eq_ignore_ascii_case(account_id))?;
    let presence_by_key = presence_records_by_key(presence);
    let characters = account
        .saves
        .values()
        .map(|save| {
            let presence_record = presence_by_key.get(&GatewaySessionKey {
                account_id: account.account_id.clone(),
                character_index: save.save.character.index,
            });
            player_summary_from_save(
                account,
                save,
                now,
                presence.map(|presence| presence.source.as_str()),
                presence_record.copied(),
            )
        })
        .collect();
    Some(AdminAccountDetail {
        summary: account_summary(account, now),
        characters,
        has_expanded_storage: account.account.has_expanded_storage,
        expanded_storage_expiry_time_binary_datetime: account
            .account
            .expanded_storage_expiry_time_binary_datetime,
        storage_password_last_set_binary_datetime: account
            .account
            .storage_password_last_set_binary_datetime,
    })
}

fn account_summary(account: &AccountReadRecord, now: u64) -> AdminAccountSummary {
    let active_ban = account.account.active_ban(now);
    AdminAccountSummary {
        account_id: account.account_id.clone(),
        password_configured: !account.account.password.trim().is_empty(),
        character_count: account.account.characters.len().max(account.saves.len()),
        storage_size: account.account.storage_size,
        has_storage_password: !account.account.storage_password.trim().is_empty(),
        is_banned: active_ban.is_some(),
        ban_reason: active_ban.as_ref().map(|ban| ban.reason.clone()),
        ban_until_ms: active_ban.as_ref().and_then(|ban| ban.ban_until_ms),
        banned_at_ms: active_ban.as_ref().and_then(|ban| ban.banned_at_ms),
        store_version: account.store_version,
    }
}

fn build_player_summaries(
    snapshot: &AccountReadSnapshot,
    presence: Option<&GatewayPresenceSnapshot>,
) -> Vec<AdminPlayerSummary> {
    let now = now_ms();
    let presence_by_key = presence_records_by_key(presence);
    let mut players = Vec::new();
    for account in &snapshot.accounts {
        for save in account.saves.values() {
            let presence_record = presence_by_key.get(&GatewaySessionKey {
                account_id: account.account_id.clone(),
                character_index: save.save.character.index,
            });
            players.push(player_summary_from_save(
                account,
                save,
                now,
                presence.map(|presence| presence.source.as_str()),
                presence_record.copied(),
            ));
        }
    }
    players.sort_by(|left, right| {
        left.character_name
            .cmp(&right.character_name)
            .then_with(|| left.account_id.cmp(&right.account_id))
            .then(left.character_index.cmp(&right.character_index))
    });
    players
}

fn build_player_detail(
    snapshot: &AccountReadSnapshot,
    requested_player_id: &str,
    presence: Option<&GatewayPresenceSnapshot>,
) -> Option<AdminPlayerDetail> {
    let now = now_ms();
    let requested_player_id = requested_player_id.trim();
    let presence_by_key = presence_records_by_key(presence);
    for account in &snapshot.accounts {
        for save in account.saves.values() {
            let presence_record = presence_by_key.get(&GatewaySessionKey {
                account_id: account.account_id.clone(),
                character_index: save.save.character.index,
            });
            let summary = player_summary_from_save(
                account,
                save,
                now,
                presence.map(|presence| presence.source.as_str()),
                presence_record.copied(),
            );
            if !player_id_matches(&summary, requested_player_id) {
                continue;
            }
            let systems = decode_stage5_systems(&save.save);
            let active_ban = account.account.active_ban(now);
            let guild_name = systems
                .as_ref()
                .map(|systems| systems.guild.name.trim().to_string())
                .filter(|name| !name.is_empty());
            let active_npc_flags = npc_flags_from_save(&save.save)
                .into_iter()
                .filter(|flag| flag.value)
                .map(|flag| AdminNpcFlagRecord {
                    index: flag.index,
                    active: true,
                })
                .collect::<Vec<_>>();
            return Some(AdminPlayerDetail {
                summary,
                inventory_count: save.save.inventory_items_json.len(),
                belt_count: save.save.belt_items_json.len(),
                storage_count: save.save.storage_items_json.len(),
                equipment_count: save.save.equipment_items_json.len(),
                quest_state_count: save.save.quest_states_json.len(),
                skill_state_count: save.save.skill_states_json.len(),
                npc_flag_count: active_npc_flags.len(),
                active_npc_flags,
                mail_count: systems
                    .as_ref()
                    .map(|systems| systems.mail.len())
                    .unwrap_or(0),
                unclaimed_mail_count: systems
                    .as_ref()
                    .map(|systems| {
                        systems
                            .mail
                            .iter()
                            .filter(|mail| !mail.claimed && !mail.deleted)
                            .count()
                    })
                    .unwrap_or(0),
                auction_listing_count: systems
                    .as_ref()
                    .map(|systems| systems.auction.len())
                    .unwrap_or(0),
                group_member_count: systems
                    .as_ref()
                    .map(|systems| systems.group.members.len())
                    .unwrap_or(0),
                guild_name,
                active_ban_reason: active_ban.as_ref().map(|ban| ban.reason.clone()),
                ban_until_ms: active_ban.as_ref().and_then(|ban| ban.ban_until_ms),
                banned_at_ms: active_ban.as_ref().and_then(|ban| ban.banned_at_ms),
            });
        }
    }
    None
}

fn player_summary_from_save(
    account: &AccountReadRecord,
    save: &CharacterSaveReadRecord,
    now_ms: u64,
    online_source: Option<&str>,
    presence: Option<&GatewaySessionRecord>,
) -> AdminPlayerSummary {
    let character = &save.save.character;
    let active_ban = account.account.active_ban(now_ms).is_some();
    let map_file_name = presence
        .and_then(|presence| presence.map_file_name.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| save.save.map_file_name.clone());
    let player_hp = presence
        .and_then(|presence| presence.player_hp)
        .unwrap_or(save.save.hp);
    let player_max_hp = presence
        .and_then(|presence| presence.player_max_hp)
        .unwrap_or(save.save.max_hp);
    AdminPlayerSummary {
        player_id: player_id(&account.account_id, character.index),
        account_id: account.account_id.clone(),
        character_index: character.index,
        character_name: character.name.clone(),
        class_name: serialized_text(&character.class),
        gender: serialized_text(&character.gender),
        level: character.level,
        map_file_name,
        map_title: save.save.map_title.clone(),
        position_x: save.save.position.x,
        position_y: save.save.position.y,
        hp: player_hp,
        max_hp: player_max_hp,
        mp: save.save.mp,
        gold: presence
            .map(|presence| u64::from(presence.gold))
            .unwrap_or_else(|| u64::from(save.save.gold)),
        credit: u64::from(save.save.credit),
        pk_points: save.save.pk_points,
        chat_banned: save.save.chat_banned,
        chat_ban_until_ms: save.save.chat_ban_until_ms,
        status: if active_ban {
            "Banned".into()
        } else if save.save.chat_banned {
            "ChatBanned".into()
        } else if presence.is_some() {
            "Online".into()
        } else {
            "Normal".into()
        },
        online: presence.is_some(),
        online_source: presence.map(|_| online_source.unwrap_or("gateway_session_cache").into()),
        player_object_id: presence.and_then(|presence| presence.player_object_id),
        runtime_tick: presence.map(|presence| presence.tick),
        store_version: account.store_version,
        save_version: save.save_version,
    }
}

fn player_id(account_id: &str, character_index: i32) -> String {
    format!("{account_id}:{character_index}")
}

fn player_id_matches(summary: &AdminPlayerSummary, requested: &str) -> bool {
    summary.player_id == requested
        || summary.character_name.eq_ignore_ascii_case(requested)
        || format!("{}:{}", summary.account_id, summary.character_index) == requested
}

fn decode_stage5_systems(save: &CharacterSaveRecord) -> Option<Stage5SystemsState> {
    save.stage5_systems_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<Stage5SystemsState>(value).ok())
}

fn build_market_read_model(snapshot: &AccountReadSnapshot) -> AdminMarketReadModel {
    let mut listings = Vec::new();
    for account in &snapshot.accounts {
        for save in account.saves.values() {
            let systems = stage5_systems_from_save(&save.save);
            for listing in systems.auction {
                listings.push(AdminMarketListingRecord {
                    owner_player_id: player_id(&account.account_id, save.save.character.index),
                    account_id: account.account_id.clone(),
                    character_index: save.save.character.index,
                    character_name: save.save.character.name.clone(),
                    listing_id: listing.id,
                    seller: listing.seller,
                    item_key: listing.item_key,
                    price: listing.price,
                    sold: listing.sold,
                    cancelled: listing.cancelled,
                    expired: listing.expired,
                });
            }
        }
    }
    listings.sort_by(|left, right| {
        left.cancelled
            .cmp(&right.cancelled)
            .then(left.expired.cmp(&right.expired))
            .then(left.sold.cmp(&right.sold))
            .then(left.listing_id.cmp(&right.listing_id))
    });
    AdminMarketReadModel {
        source: snapshot.source.clone(),
        generated_at_ms: now_ms(),
        listings,
    }
}

fn build_guilds_read_model(snapshot: &AccountReadSnapshot) -> AdminGuildsReadModel {
    #[derive(Default)]
    struct GuildAccumulator {
        members: BTreeSet<String>,
        ranks: BTreeSet<String>,
        chat_log: Vec<String>,
    }

    let mut guilds: BTreeMap<String, GuildAccumulator> = BTreeMap::new();
    for account in &snapshot.accounts {
        for save in account.saves.values() {
            let systems = stage5_systems_from_save(&save.save);
            let guild_name = systems.guild.name.trim();
            if guild_name.is_empty() {
                continue;
            }
            let accumulator = guilds.entry(guild_name.to_string()).or_default();
            for member in systems.guild.members {
                if !member.trim().is_empty() {
                    accumulator.members.insert(member);
                }
            }
            if !save.save.character.name.trim().is_empty() {
                accumulator.members.insert(save.save.character.name.clone());
            }
            if !systems.guild.rank.trim().is_empty() {
                accumulator.ranks.insert(systems.guild.rank);
            }
            for entry in systems.guild.chat_log {
                if !entry.trim().is_empty() && accumulator.chat_log.len() < 50 {
                    accumulator.chat_log.push(entry);
                }
            }
        }
    }

    AdminGuildsReadModel {
        source: snapshot.source.clone(),
        generated_at_ms: now_ms(),
        guilds: guilds
            .into_iter()
            .map(|(guild_name, accumulator)| AdminGuildRecord {
                guild_name,
                member_count: accumulator.members.len(),
                members: accumulator.members.into_iter().collect(),
                ranks: accumulator.ranks.into_iter().collect(),
                chat_log: accumulator.chat_log,
            })
            .collect(),
    }
}

fn build_namelists_read_model() -> Result<AdminNameListsReadModel, ApiError> {
    let root = name_list_root_from_env();
    let mut lists = Vec::new();
    match fs::read_dir(&root) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("txt") {
                    continue;
                }
                let players = fs::read_to_string(&path)
                    .unwrap_or_default()
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                lists.push(AdminNameListRecord {
                    list_name: path
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    path: path.display().to_string(),
                    player_count: players.len(),
                    players,
                });
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: format!("read namelist root {} failed: {error}", root.display()),
            });
        }
    }
    lists.sort_by(|left, right| left.list_name.cmp(&right.list_name));
    Ok(AdminNameListsReadModel {
        source: "file_namelists".to_string(),
        generated_at_ms: now_ms(),
        root: root.display().to_string(),
        lists,
    })
}

fn build_content_read_model() -> Result<AdminContentReadModel, ApiError> {
    let categories = vec![
        AdminContentCategoryRecord {
            category: "maps".to_string(),
            source: mir2_game_data::crystal_respawn_manifest().source_file,
            total_records: mir2_game_data::crystal_respawn_manifest().total_maps,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "respawns".to_string(),
            source: mir2_game_data::crystal_respawn_manifest().source_file,
            total_records: mir2_game_data::crystal_respawn_manifest().total_respawns,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "items".to_string(),
            source: mir2_game_data::crystal_item_manifest().source_file,
            total_records: mir2_game_data::crystal_item_manifest().total_items,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "monsters".to_string(),
            source: mir2_game_data::crystal_monster_manifest().source_file,
            total_records: mir2_game_data::crystal_monster_manifest().total_monsters,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "npcs".to_string(),
            source: mir2_game_data::crystal_npc_info_manifest().source_file,
            total_records: mir2_game_data::crystal_npc_info_manifest().total_npcs,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "npc_scripts".to_string(),
            source: mir2_game_data::crystal_npc_manifest().source_dir,
            total_records: mir2_game_data::crystal_npc_manifest().total_scripts,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "quests".to_string(),
            source: mir2_game_data::crystal_quest_packet_manifest().source_file,
            total_records: mir2_game_data::crystal_quest_packet_manifest().total_quests,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "magic".to_string(),
            source: mir2_game_data::crystal_magic_manifest().source_file,
            total_records: mir2_game_data::crystal_magic_manifest().total_magics,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "game_shop".to_string(),
            source: mir2_game_data::crystal_game_shop_packet_manifest().source_file,
            total_records: mir2_game_data::crystal_game_shop_packet_manifest().total_items,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "recipes".to_string(),
            source: mir2_game_data::crystal_recipe_packet_manifest().source_file,
            total_records: mir2_game_data::crystal_recipe_packet_manifest().total_recipes,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "base_stats".to_string(),
            source: mir2_game_data::crystal_base_stats_packet_manifest().source_configs_dir,
            total_records: mir2_game_data::crystal_base_stats_packet_manifest().total_classes,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "buffs".to_string(),
            source: mir2_game_data::crystal_buff_manifest().source_file,
            total_records: mir2_game_data::crystal_buff_manifest().total_buffs,
            status: "Generated".to_string(),
            editable: true,
        },
        AdminContentCategoryRecord {
            category: "drops".to_string(),
            source: mir2_game_data::crystal_drop_manifest().source_dir,
            total_records: mir2_game_data::crystal_drop_manifest().total_tables,
            status: "Generated".to_string(),
            editable: true,
        },
    ];

    Ok(AdminContentReadModel {
        generated_at_ms: now_ms(),
        categories,
        bundles: list_content_bundles(&content_bundle_dir_from_env())?,
    })
}

fn list_content_bundles(root: &PathBuf) -> Result<Vec<AdminContentBundleRecord>, ApiError> {
    let mut records = Vec::new();
    match fs::read_dir(root) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let data = fs::read_to_string(&path).unwrap_or_default();
                let value = serde_json::from_str::<Value>(&data).unwrap_or(Value::Null);
                let updated_at_ms = entry
                    .metadata()
                    .ok()
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis() as u64)
                    .unwrap_or_default();
                records.push(AdminContentBundleRecord {
                    bundle_id: value
                        .get("bundleId")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            path.file_stem()
                                .and_then(|name| name.to_str())
                                .unwrap_or("unknown")
                                .to_string()
                        }),
                    category: value
                        .get("category")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string(),
                    record_key: value
                        .get("recordKey")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string(),
                    path: path.display().to_string(),
                    updated_at_ms,
                });
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: format!("read content bundle dir {} failed: {error}", root.display()),
            });
        }
    }
    records.sort_by(|left, right| right.updated_at_ms.cmp(&left.updated_at_ms));
    Ok(records)
}

fn build_control_read_model() -> Result<AdminControlReadModel, ApiError> {
    Ok(AdminControlReadModel {
        generated_at_ms: now_ms(),
        gateway_control_configured: env::var("ADMIN_GATEWAY_CONTROL_URL")
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        control_actions: vec![
            "status".to_string(),
            "reload_npcs".to_string(),
            "reload_drops".to_string(),
            "reload_line_message".to_string(),
            "clear_blocked_ips".to_string(),
            "start".to_string(),
            "stop".to_string(),
            "reboot".to_string(),
            "close".to_string(),
        ],
        logs: read_admin_log_tails(),
    })
}

fn read_admin_log_tails() -> Vec<AdminLogTailRecord> {
    let configured = env::var("ADMIN_LOG_PATHS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .filter_map(|entry| {
                    let entry = entry.trim();
                    if entry.is_empty() {
                        return None;
                    }
                    let (name, path) = entry
                        .split_once('=')
                        .map(|(name, path)| (name.trim(), path.trim()))
                        .unwrap_or(("server", entry));
                    Some((name.to_string(), PathBuf::from(path)))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if configured.is_empty() {
        return vec![AdminLogTailRecord {
            name: "server".to_string(),
            path: "ADMIN_LOG_PATHS".to_string(),
            configured: false,
            lines: Vec::new(),
            error: Some("ADMIN_LOG_PATHS is not configured".to_string()),
        }];
    }
    configured
        .into_iter()
        .map(|(name, path)| match fs::read_to_string(&path) {
            Ok(data) => {
                let mut lines = data
                    .lines()
                    .rev()
                    .take(80)
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                lines.reverse();
                AdminLogTailRecord {
                    name,
                    path: path.display().to_string(),
                    configured: true,
                    lines,
                    error: None,
                }
            }
            Err(error) => AdminLogTailRecord {
                name,
                path: path.display().to_string(),
                configured: true,
                lines: Vec::new(),
                error: Some(error.to_string()),
            },
        })
        .collect()
}

fn load_gateway_presence() -> GatewayPresenceSnapshot {
    let url = env::var("ADMIN_GATEWAY_SESSIONS_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:7110/admin/sessions".to_string());
    let body = match get_json_from_gateway(&url) {
        Ok(body) => body,
        Err(error) => {
            return GatewayPresenceSnapshot {
                source: "gateway_sessions_unavailable".into(),
                sessions: Vec::new(),
                error: Some(error),
            }
        }
    };
    match serde_json::from_str::<GatewaySessionsResponse>(&body) {
        Ok(response) => {
            let mut sessions = response.sessions;
            sessions.sort_by(|left, right| {
                left.character_name
                    .cmp(&right.character_name)
                    .then_with(|| left.key.account_id.cmp(&right.key.account_id))
                    .then(left.key.character_index.cmp(&right.key.character_index))
            });
            GatewayPresenceSnapshot {
                source: response.source,
                sessions,
                error: None,
            }
        }
        Err(error) => GatewayPresenceSnapshot {
            source: "gateway_sessions_decode_failed".into(),
            sessions: Vec::new(),
            error: Some(format!("decode gateway sessions failed: {error}")),
        },
    }
}

fn presence_records_by_key(
    presence: Option<&GatewayPresenceSnapshot>,
) -> BTreeMap<GatewaySessionKey, &GatewaySessionRecord> {
    presence
        .map(|presence| {
            presence
                .sessions
                .iter()
                .map(|record| (record.key.clone(), record))
                .collect()
        })
        .unwrap_or_default()
}

fn build_hot_maps(players: &[AdminPlayerSummary]) -> Vec<AdminMapPopulation> {
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for player in players {
        let map_file_name = if player.map_file_name.trim().is_empty() {
            "unknown".to_string()
        } else {
            player.map_file_name.clone()
        };
        let map_title = if player.map_title.trim().is_empty() {
            map_file_name.clone()
        } else {
            player.map_title.clone()
        };
        *counts.entry((map_file_name, map_title)).or_default() += 1;
    }
    let max_count = counts.values().copied().max().unwrap_or(0);
    let mut rows = counts
        .into_iter()
        .map(
            |((map_file_name, map_title), character_count)| AdminMapPopulation {
                map_file_name,
                map_title,
                character_count,
                percent: percent(character_count as u64, max_count as u64),
            },
        )
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .character_count
            .cmp(&left.character_count)
            .then_with(|| left.map_title.cmp(&right.map_title))
    });
    rows.truncate(8);
    rows
}

fn build_presence_hot_maps(presence: &GatewayPresenceSnapshot) -> Option<Vec<AdminMapPopulation>> {
    if presence.sessions.is_empty() {
        return None;
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for session in &presence.sessions {
        let map_file_name = session
            .map_file_name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        *counts.entry(map_file_name).or_default() += 1;
    }
    let max_count = counts.values().copied().max().unwrap_or(0);
    let mut rows = counts
        .into_iter()
        .map(|(map_file_name, character_count)| AdminMapPopulation {
            map_title: map_file_name.clone(),
            map_file_name,
            character_count,
            percent: percent(character_count as u64, max_count as u64),
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .character_count
            .cmp(&left.character_count)
            .then_with(|| left.map_file_name.cmp(&right.map_file_name))
    });
    rows.truncate(8);
    Some(rows)
}

fn build_economy_read_model(
    snapshot: &AccountReadSnapshot,
    price_feeds: Vec<AdminPriceFeedRecord>,
    price_feed_configured: bool,
) -> AdminEconomyReadModel {
    let players = build_player_summaries(snapshot, None);
    let character_count = players.len();
    let total_gold: u64 = players.iter().map(|player| player.gold).sum();
    let total_credit: u64 = players.iter().map(|player| player.credit).sum();
    let source = if price_feed_configured {
        format!("{} + postgres_admin_market_price_feeds", snapshot.source)
    } else {
        snapshot.source.clone()
    };
    AdminEconomyReadModel {
        source,
        generated_at_ms: now_ms(),
        assets: vec![
            AdminEconomyAsset {
                asset: "Gold".into(),
                total: total_gold,
                holders: players.iter().filter(|player| player.gold > 0).count(),
                average: average(total_gold, character_count),
                state: "Tracked".into(),
            },
            AdminEconomyAsset {
                asset: "Credit".into(),
                total: total_credit,
                holders: players.iter().filter(|player| player.credit > 0).count(),
                average: average(total_credit, character_count),
                state: "Tracked".into(),
            },
        ],
        gold_distribution: build_gold_distribution(&players),
        price_feeds,
        price_feed_configured,
    }
}

fn build_activities_read_model(
    repository: Option<&PostgresAdminRepository>,
) -> Result<AdminActivitiesReadModel, AdminError> {
    match repository {
        Some(repository) => Ok(AdminActivitiesReadModel {
            source: "postgres_admin_activities".into(),
            generated_at_ms: now_ms(),
            configured: true,
            activities: repository.list_activities(128)?,
        }),
        None => Ok(AdminActivitiesReadModel {
            source: "activity_config_store_unwired".into(),
            generated_at_ms: now_ms(),
            configured: false,
            activities: Vec::new(),
        }),
    }
}

fn build_gold_distribution(players: &[AdminPlayerSummary]) -> Vec<AdminDistributionBucket> {
    let mut amounts = players.iter().map(|player| player.gold).collect::<Vec<_>>();
    amounts.sort_by(|left, right| right.cmp(left));
    let total: u64 = amounts.iter().sum();
    if total == 0 || amounts.is_empty() {
        return Vec::new();
    }
    let top_amount = amounts[0];
    let remaining = total.saturating_sub(top_amount);
    let mut buckets = vec![AdminDistributionBucket {
        key: "topCharacter".into(),
        label: "Top character".into(),
        value: percent(top_amount, total),
        amount: top_amount,
    }];
    if remaining > 0 {
        buckets.push(AdminDistributionBucket {
            key: "remainingCharacters".into(),
            label: "Remaining characters".into(),
            value: percent(remaining, total),
            amount: remaining,
        });
    }
    buckets
}

fn build_servers_read_model(
    snapshot: &AccountReadSnapshot,
    presence: &GatewayPresenceSnapshot,
    zones: Vec<AdminZoneRuntimeRecord>,
    zone_runtime_configured: bool,
) -> AdminServersReadModel {
    let players = build_player_summaries(snapshot, Some(presence));
    let session_zones_online = presence
        .sessions
        .iter()
        .filter_map(|session| session.map_file_name.as_deref())
        .filter(|map| !map.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .len();
    let runtime_zones_online = zones
        .iter()
        .filter(|zone| zone.status.eq_ignore_ascii_case("healthy"))
        .count();
    let zones_online = runtime_zones_online.max(session_zones_online);
    let zones_source = if zone_runtime_configured {
        "postgres_admin_zone_runtime + gateway_session_cache".into()
    } else {
        presence.source.clone()
    };
    AdminServersReadModel {
        generated_at_ms: now_ms(),
        account_store_source: snapshot.source.clone(),
        account_count: snapshot.accounts.len(),
        character_count: players.len(),
        zones_online,
        zones_source,
        zone_runtime_configured,
        zones,
        services: build_service_statuses(snapshot, Some(presence)),
    }
}

fn build_operators_read_model(
    repository: Option<&PostgresAdminRepository>,
) -> Result<AdminOperatorsReadModel, AdminError> {
    match repository {
        Some(repository) => Ok(AdminOperatorsReadModel {
            source: "postgres_admin_operators".into(),
            generated_at_ms: now_ms(),
            configured: true,
            operators: repository.list_operators(128)?,
        }),
        None => Ok(AdminOperatorsReadModel {
            source: "operator_store_unwired".into(),
            generated_at_ms: now_ms(),
            configured: false,
            operators: Vec::new(),
        }),
    }
}

fn build_risk_read_model(
    snapshot: &AccountReadSnapshot,
    graph: Vec<AdminRiskGraphEdge>,
    graph_configured: bool,
) -> AdminRiskReadModel {
    let now = now_ms();
    let mut cases = Vec::new();
    for account in &snapshot.accounts {
        let Some(active_ban) = account.account.active_ban(now) else {
            continue;
        };
        let reason = if active_ban.reason.trim().is_empty() {
            "active account ban".to_string()
        } else {
            active_ban.reason.clone()
        };
        let risk = if active_ban.ban_until_ms.is_none() {
            "Critical"
        } else {
            "High"
        };
        for save in account.saves.values() {
            cases.push(AdminRiskCase {
                player_id: player_id(&account.account_id, save.save.character.index),
                account_id: account.account_id.clone(),
                character_name: save.save.character.name.clone(),
                signal: "account_ban".into(),
                risk: risk.into(),
                evidence: reason.clone(),
                ban_until_ms: active_ban.ban_until_ms,
            });
        }
    }
    let source = if graph_configured {
        format!("{} + postgres_admin_trade_graph", snapshot.source)
    } else {
        snapshot.source.clone()
    };
    AdminRiskReadModel {
        source,
        generated_at_ms: now,
        cases,
        graph,
    }
}

fn build_service_statuses(
    snapshot: &AccountReadSnapshot,
    presence: Option<&GatewayPresenceSnapshot>,
) -> Vec<AdminServiceStatus> {
    let mut services = vec![
        AdminServiceStatus {
            name: "Admin API".into(),
            status: "Healthy".into(),
            detail: "current process is serving read models".into(),
            latency_ms: Some(0),
            configured: true,
        },
        AdminServiceStatus {
            name: "Account Store".into(),
            status: "Healthy".into(),
            detail: format!(
                "{}: {} accounts / {} characters",
                snapshot.source,
                snapshot.accounts.len(),
                build_player_summaries(snapshot, presence).len()
            ),
            latency_ms: None,
            configured: true,
        },
    ];
    if let Some(presence) = presence {
        services.push(AdminServiceStatus {
            name: "Gateway Sessions".into(),
            status: if presence.error.is_some() {
                "Unavailable".into()
            } else {
                "Healthy".into()
            },
            detail: presence.error.clone().unwrap_or_else(|| {
                format!(
                    "{}: {} online sessions",
                    presence.source,
                    presence.sessions.len()
                )
            }),
            latency_ms: None,
            configured: true,
        });
    }
    services.push(postgres_status());
    services.push(tcp_env_status(
        "Redis",
        "MIR2_GATEWAY_REDIS_CACHE_URL",
        "redis://127.0.0.1:6379",
        6379,
    ));
    services.push(tcp_env_status("NATS", "NATS_ADDR", "127.0.0.1:4222", 4222));
    services.push(tcp_env_status(
        "Redpanda Pandaproxy",
        "ADMIN_OUTBOX_REDPANDA_URL",
        "http://127.0.0.1:8082",
        8082,
    ));
    services.push(clickhouse_status());
    services
}

fn postgres_status() -> AdminServiceStatus {
    let configured_url = env::var("ADMIN_DATABASE_URL")
        .ok()
        .or_else(|| env::var("MIR2_ACCOUNT_STORE_DATABASE_URL").ok())
        .filter(|value| !value.trim().is_empty());
    let Some(database_url) = configured_url else {
        return AdminServiceStatus {
            name: "Postgres".into(),
            status: "NotConfigured".into(),
            detail: "ADMIN_DATABASE_URL and MIR2_ACCOUNT_STORE_DATABASE_URL are unset".into(),
            latency_ms: None,
            configured: false,
        };
    };
    let started = Instant::now();
    match Client::connect(database_url.trim(), NoTls)
        .and_then(|mut client| client.query_one("SELECT 1", &[]).map(|_| ()))
    {
        Ok(()) => AdminServiceStatus {
            name: "Postgres".into(),
            status: "Healthy".into(),
            detail: "SELECT 1 succeeded".into(),
            latency_ms: Some(started.elapsed().as_millis() as u64),
            configured: true,
        },
        Err(error) => AdminServiceStatus {
            name: "Postgres".into(),
            status: "Unavailable".into(),
            detail: error.to_string(),
            latency_ms: Some(started.elapsed().as_millis() as u64),
            configured: true,
        },
    }
}

fn clickhouse_status() -> AdminServiceStatus {
    let configured = env::var("ADMIN_CLICKHOUSE_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let started = Instant::now();
    let query = AdminEventQuery {
        limit: Some(1),
        command_id: None,
        event_type: None,
        status: None,
    };
    match fetch_clickhouse_admin_events(&query) {
        Ok(records) => AdminServiceStatus {
            name: "ClickHouse".into(),
            status: "Healthy".into(),
            detail: format!(
                "admin_events query succeeded, {} sampled rows",
                records.len()
            ),
            latency_ms: Some(started.elapsed().as_millis() as u64),
            configured: true,
        },
        Err(error) => AdminServiceStatus {
            name: "ClickHouse".into(),
            status: if configured {
                "Unavailable".into()
            } else {
                "NotConfigured".into()
            },
            detail: error.message,
            latency_ms: Some(started.elapsed().as_millis() as u64),
            configured,
        },
    }
}

fn tcp_env_status(
    name: &str,
    env_name: &str,
    default_target: &str,
    default_port: u16,
) -> AdminServiceStatus {
    let configured_target = env::var(env_name)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let configured = configured_target.is_some();
    let raw_target = configured_target.as_deref().unwrap_or(default_target);
    let target = match normalize_tcp_target(raw_target, default_port) {
        Ok(target) => target,
        Err(error) => {
            return AdminServiceStatus {
                name: name.into(),
                status: "Unavailable".into(),
                detail: error,
                latency_ms: None,
                configured,
            }
        }
    };
    let started = Instant::now();
    let result = target
        .to_socket_addrs()
        .map_err(|error| error.to_string())
        .and_then(|mut addrs| {
            let addr = addrs
                .next()
                .ok_or_else(|| format!("no socket address for {target}"))?;
            TcpStream::connect_timeout(&addr, Duration::from_secs(1))
                .map_err(|error| format!("tcp connect failed at {addr}: {error}"))
        });
    match result {
        Ok(_) => AdminServiceStatus {
            name: name.into(),
            status: "Healthy".into(),
            detail: if configured {
                format!("{env_name} reachable at {target}")
            } else {
                format!("default dev endpoint reachable at {target}; {env_name} is unset")
            },
            latency_ms: Some(started.elapsed().as_millis() as u64),
            configured,
        },
        Err(error) => AdminServiceStatus {
            name: name.into(),
            status: if configured {
                "Unavailable".into()
            } else {
                "NotConfigured".into()
            },
            detail: if configured {
                error
            } else {
                format!("{env_name} is unset and default dev endpoint {target} is not reachable")
            },
            latency_ms: Some(started.elapsed().as_millis() as u64),
            configured,
        },
    }
}

fn normalize_tcp_target(raw_target: &str, default_port: u16) -> Result<String, String> {
    let trimmed = raw_target.trim();
    if trimmed.is_empty() {
        return Err("tcp target is empty".into());
    }
    let without_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let host_port = without_scheme.split('/').next().unwrap_or_default();
    if host_port.is_empty() {
        return Err(format!("invalid tcp target: {raw_target}"));
    }
    if host_port.rsplit_once(':').is_some() {
        Ok(host_port.to_string())
    } else {
        Ok(format!("{host_port}:{default_port}"))
    }
}

fn average(total: u64, count: usize) -> u64 {
    if count == 0 {
        0
    } else {
        total / count as u64
    }
}

fn percent(amount: u64, total: u64) -> u8 {
    if total == 0 {
        0
    } else {
        amount.saturating_mul(100).div_ceil(total).min(100) as u8
    }
}

fn serialized_text<T>(value: &T) -> String
where
    T: Serialize + fmt::Debug,
{
    match serde_json::to_value(value) {
        Ok(Value::String(value)) => value,
        Ok(other) => other.to_string(),
        Err(_) => format!("{value:?}"),
    }
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

fn operator_from_headers(
    headers: &HeaderMap,
    admin_store: Option<&PostgresAdminRepository>,
) -> Result<Operator, ApiError> {
    if operator_auth_backend_is_postgres() {
        let repository = admin_store.ok_or_else(|| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "postgres operator auth requires ADMIN_DATABASE_URL".into(),
        })?;
        return operator_from_postgres_token(headers, repository);
    }
    if let Some(operator) = operator_from_policy_file(headers)? {
        return Ok(operator);
    }
    if let Some(repository) = admin_store {
        if let Some(operator) = optional_bearer_token(headers)
            .as_deref()
            .map(|token| operator_from_postgres_token_value(token, repository))
            .transpose()?
            .flatten()
        {
            return Ok(operator);
        }
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

fn operator_auth_backend_is_postgres() -> bool {
    env::var("ADMIN_OPERATOR_AUTH_BACKEND")
        .map(|value| value.trim().eq_ignore_ascii_case("postgres"))
        .unwrap_or(false)
}

fn operator_from_postgres_token(
    headers: &HeaderMap,
    repository: &PostgresAdminRepository,
) -> Result<Operator, ApiError> {
    let token = bearer_token(headers)?;
    operator_from_postgres_token_value(&token, repository)?.ok_or_else(|| ApiError {
        status: StatusCode::UNAUTHORIZED,
        message: "operator token not found in postgres operator store".into(),
    })
}

fn operator_from_postgres_token_value(
    token: &str,
    repository: &PostgresAdminRepository,
) -> Result<Option<Operator>, ApiError> {
    let lookup = || {
        repository
            .operator_by_token(token, now_ms())
            .map_err(ApiError::from)
    };
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(lookup)
    } else {
        lookup()
    }
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
    parse_bearer_token(&provided)
}

fn optional_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| parse_bearer_token(value).ok())
}

fn parse_bearer_token(provided: &str) -> Result<String, ApiError> {
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

fn require_operator_permission(
    operator: &Operator,
    permission: Permission,
) -> Result<(), ApiError> {
    if operator.has_permission(&permission)
        || operator.has_permission(&Permission::PermissionManage)
    {
        Ok(())
    } else {
        Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: format!(
                "operator lacks required permission: {}",
                permission_text(&permission)
            ),
        })
    }
}

fn configured_admin_store(state: &AdminApiState) -> Result<PostgresAdminRepository, ApiError> {
    state.admin_store.clone().ok_or_else(|| ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        message: "ADMIN_DATABASE_URL is required for admin projection writes".into(),
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
        "account_write" => Ok(Permission::AccountWrite),
        "account_ban" => Ok(Permission::AccountBan),
        "character_read" => Ok(Permission::CharacterRead),
        "character_write" => Ok(Permission::CharacterWrite),
        "character_kick" => Ok(Permission::CharacterKick),
        "character_message" => Ok(Permission::CharacterMessage),
        "inventory_read" => Ok(Permission::InventoryRead),
        "inventory_grant_item" => Ok(Permission::InventoryGrantItem),
        "currency_grant" => Ok(Permission::CurrencyGrant),
        "mail_send_system" => Ok(Permission::MailSendSystem),
        "world_broadcast" => Ok(Permission::WorldBroadcast),
        "server_control" => Ok(Permission::ServerControl),
        "market_moderate" => Ok(Permission::MarketModerate),
        "guild_moderate" => Ok(Permission::GuildModerate),
        "namelist_manage" => Ok(Permission::NameListManage),
        "content_read" => Ok(Permission::ContentRead),
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

fn fetch_clickhouse_gameplay_events(
    query: &GameplayEventQuery,
) -> Result<Vec<GameplayEventRecord>, ApiError> {
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
    let query_text = build_clickhouse_gameplay_events_query(query);
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
        .map(parse_clickhouse_gameplay_event)
        .collect()
}

fn fetch_clickhouse_gameplay_event_summary(
    query: &GameplayEventSummaryQuery,
    generated_at_ms: u64,
    window_seconds: u64,
) -> Result<GameplayEventSummary, ApiError> {
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
    let query_text =
        build_clickhouse_gameplay_event_summary_query(query, generated_at_ms, window_seconds);
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
    let commands = response
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(parse_clickhouse_gameplay_event_summary)
        .collect::<Result<Vec<_>, _>>()?;
    let total_count = commands.iter().map(|command| command.event_count).sum();
    let last_occurred_at_ms = commands
        .iter()
        .map(|command| command.last_occurred_at_ms)
        .max();
    Ok(GameplayEventSummary {
        total_count,
        last_occurred_at_ms,
        commands,
    })
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
        "SELECT event_id, event_type, command_id, operator_id, status, occurred_at_ms, payload_json \
         FROM (\
            SELECT event_id,\
                argMax(event_type, ingested_at) AS event_type,\
                argMax(command_id, ingested_at) AS command_id,\
                argMax(operator_id, ingested_at) AS operator_id,\
                argMax(status, ingested_at) AS status,\
                argMax(occurred_at_ms, ingested_at) AS occurred_at_ms,\
                argMax(payload_json, ingested_at) AS payload_json \
            FROM admin_events \
            GROUP BY event_id\
         ){where_clause} \
         ORDER BY occurred_at_ms DESC, event_id DESC \
         LIMIT {limit} \
         FORMAT JSONEachRow"
    )
}

fn build_clickhouse_gameplay_events_query(query: &GameplayEventQuery) -> String {
    let mut filters = Vec::new();
    if let Some(zone_id) = non_empty_query_value(query.zone_id.as_deref()) {
        filters.push(format!(
            "zone_id = '{}'",
            clickhouse_string_literal(zone_id)
        ));
    }
    if let Some(command_kind) = non_empty_query_value(query.command_kind.as_deref()) {
        filters.push(format!(
            "command_kind = '{}'",
            clickhouse_string_literal(command_kind)
        ));
    }
    if let Some(account_id) = non_empty_query_value(query.account_id.as_deref()) {
        filters.push(format!(
            "account_id = '{}'",
            clickhouse_string_literal(account_id)
        ));
    }
    if let Some(character_name) = non_empty_query_value(query.character_name.as_deref()) {
        filters.push(format!(
            "character_name = '{}'",
            clickhouse_string_literal(character_name)
        ));
    }
    let where_clause = if filters.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", filters.join(" AND "))
    };
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    format!(
        "SELECT event_id, event_type, zone_id, command_kind, account_id, character_index, character_name, packet_count, snapshot_tick, occurred_at_ms, payload_json \
         FROM (\
            SELECT event_id,\
                argMax(event_type, ingested_at) AS event_type,\
                argMax(zone_id, ingested_at) AS zone_id,\
                argMax(command_kind, ingested_at) AS command_kind,\
                argMax(account_id, ingested_at) AS account_id,\
                argMax(character_index, ingested_at) AS character_index,\
                argMax(character_name, ingested_at) AS character_name,\
                argMax(packet_count, ingested_at) AS packet_count,\
                argMax(snapshot_tick, ingested_at) AS snapshot_tick,\
                argMax(occurred_at_ms, ingested_at) AS occurred_at_ms,\
                argMax(payload_json, ingested_at) AS payload_json \
            FROM gameplay_events \
            GROUP BY event_id\
         ){where_clause} \
         ORDER BY occurred_at_ms DESC, event_id DESC \
         LIMIT {limit} \
         FORMAT JSONEachRow"
    )
}

fn build_clickhouse_gameplay_event_summary_query(
    query: &GameplayEventSummaryQuery,
    generated_at_ms: u64,
    window_seconds: u64,
) -> String {
    let mut filters = vec![format!(
        "occurred_at_ms >= {}",
        generated_at_ms.saturating_sub(window_seconds.saturating_mul(1_000))
    )];
    if let Some(zone_id) = non_empty_query_value(query.zone_id.as_deref()) {
        filters.push(format!(
            "zone_id = '{}'",
            clickhouse_string_literal(zone_id)
        ));
    }
    if let Some(command_kind) = non_empty_query_value(query.command_kind.as_deref()) {
        filters.push(format!(
            "command_kind = '{}'",
            clickhouse_string_literal(command_kind)
        ));
    }
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    format!(
        "SELECT command_kind, count() AS event_count, max(occurred_at_ms) AS last_occurred_at_ms, max(snapshot_tick) AS max_snapshot_tick \
         FROM (\
            SELECT event_id,\
                argMax(zone_id, ingested_at) AS zone_id,\
                argMax(command_kind, ingested_at) AS command_kind,\
                argMax(snapshot_tick, ingested_at) AS snapshot_tick,\
                argMax(occurred_at_ms, ingested_at) AS occurred_at_ms \
            FROM gameplay_events \
            GROUP BY event_id\
         ) \
         WHERE {} \
         GROUP BY command_kind \
         ORDER BY event_count DESC, last_occurred_at_ms DESC, command_kind ASC \
         LIMIT {limit} \
         FORMAT JSONEachRow",
        filters.join(" AND ")
    )
}

fn normalized_gameplay_event_summary_window_seconds(window_seconds: Option<u64>) -> u64 {
    window_seconds.unwrap_or(300).clamp(1, 86_400)
}

fn normalized_gameplay_event_summary_max_lag_ms(
    max_lag_seconds: Option<u64>,
    window_seconds: u64,
) -> u64 {
    max_lag_seconds
        .unwrap_or(window_seconds)
        .clamp(1, 86_400)
        .saturating_mul(1_000)
}

fn normalized_gameplay_event_summary_min_events(min_events: Option<u64>) -> u64 {
    min_events.unwrap_or(1).min(1_000_000)
}

fn gameplay_event_readiness_alerts(
    summary: &GameplayEventSummary,
    lag_ms: Option<u64>,
    max_lag_ms: u64,
    min_event_count: u64,
) -> Vec<GameplayEventReadinessAlert> {
    let mut alerts = Vec::new();
    if summary.total_count < min_event_count {
        alerts.push(GameplayEventReadinessAlert {
            level: "warning".into(),
            code: "event_volume_below_minimum".into(),
            message: format!(
                "Expected at least {min_event_count} gameplay event(s) in the current window; observed {}.",
                summary.total_count
            ),
        });
    }
    if let Some(lag_ms) = lag_ms {
        if lag_ms > max_lag_ms {
            alerts.push(GameplayEventReadinessAlert {
                level: "critical".into(),
                code: "event_lag_above_threshold".into(),
                message: format!(
                    "Latest gameplay event lag is {lag_ms} ms, above the configured {max_lag_ms} ms threshold."
                ),
            });
        }
    }
    alerts
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

fn parse_clickhouse_gameplay_event(line: &str) -> Result<GameplayEventRecord, ApiError> {
    let row: Value = serde_json::from_str(line).map_err(|error| ApiError {
        status: StatusCode::BAD_GATEWAY,
        message: format!("decode ClickHouse gameplay event failed: {error}"),
    })?;
    Ok(GameplayEventRecord {
        event_id: string_field(&row, "event_id")?,
        event_type: string_field(&row, "event_type")?,
        zone_id: string_field(&row, "zone_id")?,
        command_kind: string_field(&row, "command_kind")?,
        account_id: optional_string_field(&row, "account_id")?,
        character_index: optional_i32_field(&row, "character_index")?,
        character_name: optional_string_field(&row, "character_name")?,
        packet_count: u64_field(&row, "packet_count")?,
        snapshot_tick: u64_field(&row, "snapshot_tick")?,
        occurred_at_ms: u64_field(&row, "occurred_at_ms")?,
        payload_json: string_field(&row, "payload_json")?,
    })
}

fn parse_clickhouse_gameplay_event_summary(
    line: &str,
) -> Result<GameplayEventCommandSummary, ApiError> {
    let row: Value = serde_json::from_str(line).map_err(|error| ApiError {
        status: StatusCode::BAD_GATEWAY,
        message: format!("decode ClickHouse gameplay event summary failed: {error}"),
    })?;
    Ok(GameplayEventCommandSummary {
        command_kind: string_field(&row, "command_kind")?,
        event_count: u64_field(&row, "event_count")?,
        last_occurred_at_ms: u64_field(&row, "last_occurred_at_ms")?,
        max_snapshot_tick: u64_field(&row, "max_snapshot_tick")?,
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

fn optional_string_field(row: &Value, field: &str) -> Result<Option<String>, ApiError> {
    match row.get(field) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(other) => Err(ApiError {
            status: StatusCode::BAD_GATEWAY,
            message: format!("ClickHouse event row field {field} is not nullable string: {other}"),
        }),
    }
}

fn optional_i32_field(row: &Value, field: &str) -> Result<Option<i32>, ApiError> {
    let Some(value) = row.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    if let Some(value) = value.as_i64().and_then(|value| i32::try_from(value).ok()) {
        return Ok(Some(value));
    }
    if let Some(value) = value.as_str().and_then(|value| value.parse::<i32>().ok()) {
        return Ok(Some(value));
    }
    Err(ApiError {
        status: StatusCode::BAD_GATEWAY,
        message: format!("ClickHouse event row missing integer field {field}"),
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
        AdminCommand::CreateAccount { account_id, .. }
        | AdminCommand::UpdateAccount { account_id, .. }
        | AdminCommand::DeleteAccount { account_id } => {
            require_non_empty("account_id", account_id)?;
        }
        AdminCommand::UpdateCharacter { character_id, .. }
        | AdminCommand::ReturnSafeZone { character_id }
        | AdminCommand::KillPlayer { character_id }
        | AdminCommand::KillPets { character_id } => {
            require_non_empty("character_id", character_id)?;
        }
        AdminCommand::SetCharacterFlag {
            character_id,
            flag_index,
            ..
        } => {
            require_non_empty("character_id", character_id)?;
            if !(1..=1999).contains(flag_index) {
                return Err(AdminError::InvalidCommand(
                    "flag_index must be between 1 and 1999".into(),
                ));
            }
        }
        AdminCommand::SetChatBan {
            character_id,
            banned,
            until_ms,
        } => {
            require_non_empty("character_id", character_id)?;
            if *banned && until_ms == &Some(0) {
                return Err(AdminError::InvalidCommand(
                    "until_ms must be greater than zero when provided".into(),
                ));
            }
        }
        AdminCommand::MessagePlayer {
            character_id,
            message,
        } => {
            require_non_empty("character_id", character_id)?;
            require_non_empty("message", message)?;
        }
        AdminCommand::Broadcast { message } => {
            require_non_empty("message", message)?;
        }
        AdminCommand::MarketCancelListing {
            character_id,
            listing_id,
            ..
        }
        | AdminCommand::MarketExpireListing {
            character_id,
            listing_id,
            ..
        }
        | AdminCommand::MarketDeleteListing {
            character_id,
            listing_id,
            ..
        } => {
            require_non_empty("character_id", character_id)?;
            if *listing_id == 0 {
                return Err(AdminError::InvalidCommand(
                    "listing_id must be greater than zero".into(),
                ));
            }
        }
        AdminCommand::GuildRemoveMember {
            guild_name,
            member_name,
        } => {
            require_non_empty("guild_name", guild_name)?;
            require_non_empty("member_name", member_name)?;
        }
        AdminCommand::GuildSendMessage {
            guild_name,
            message,
        } => {
            require_non_empty("guild_name", guild_name)?;
            require_non_empty("message", message)?;
        }
        AdminCommand::NameListAdd {
            list_name,
            player_name,
        }
        | AdminCommand::NameListRemove {
            list_name,
            player_name,
        } => {
            require_non_empty("list_name", list_name)?;
            require_non_empty("player_name", player_name)?;
        }
        AdminCommand::NameListCreate { list_name } | AdminCommand::NameListDelete { list_name } => {
            require_non_empty("list_name", list_name)?;
        }
        AdminCommand::PublishContentBundle {
            bundle_id,
            category,
            record_key,
            ..
        } => {
            require_non_empty("bundle_id", bundle_id)?;
            require_non_empty("category", category)?;
            require_non_empty("record_key", record_key)?;
        }
        AdminCommand::ServerControl { action, .. } => {
            require_non_empty("action", action)?;
        }
    }
    Ok(())
}

fn command_requires_approval(command: &AdminCommand) -> bool {
    match command {
        AdminCommand::SendSystemMail { target_kind, .. } => *target_kind == MailTargetKind::Global,
        AdminCommand::GrantItem { .. }
        | AdminCommand::GrantCurrency { .. }
        | AdminCommand::BanAccount { .. }
        | AdminCommand::DeleteAccount { .. }
        | AdminCommand::UpdateCharacter { .. }
        | AdminCommand::SetCharacterFlag { .. }
        | AdminCommand::KillPlayer { .. }
        | AdminCommand::MarketDeleteListing { .. }
        | AdminCommand::NameListDelete { .. }
        | AdminCommand::PublishContentBundle { .. } => true,
        AdminCommand::ServerControl { action, .. } => matches!(
            canonical_control_action(action).as_deref(),
            Ok("stop" | "reboot" | "close")
        ),
        AdminCommand::KickPlayer { .. } => false,
        AdminCommand::CreateAccount { .. }
        | AdminCommand::UpdateAccount { .. }
        | AdminCommand::ReturnSafeZone { .. }
        | AdminCommand::KillPets { .. }
        | AdminCommand::SetChatBan { .. }
        | AdminCommand::MessagePlayer { .. }
        | AdminCommand::Broadcast { .. }
        | AdminCommand::MarketCancelListing { .. }
        | AdminCommand::MarketExpireListing { .. }
        | AdminCommand::GuildRemoveMember { .. }
        | AdminCommand::GuildSendMessage { .. }
        | AdminCommand::NameListAdd { .. }
        | AdminCommand::NameListRemove { .. }
        | AdminCommand::NameListCreate { .. } => false,
    }
}

fn approval_matches_command(approval: &ApprovalRecord, envelope: &AdminCommandEnvelope) -> bool {
    approval.command_id == envelope.command_id
        && approval.command_type == command_type(&envelope.command)
        && approval.requested_by == envelope.operator.id
        && approval.status == ApprovalStatus::Approved
}

fn require_non_empty(field: &str, value: &str) -> Result<(), AdminError> {
    if value.trim().is_empty() {
        return Err(AdminError::InvalidCommand(format!("{field} is required")));
    }
    Ok(())
}

fn required_string(field: &str, value: &str) -> Result<String, AdminError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AdminError::InvalidCommand(format!("{field} is required")));
    }
    Ok(value.to_string())
}

fn optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn clean_string_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn canonical_permissions(values: Vec<String>) -> Result<Vec<String>, AdminError> {
    let mut permissions = BTreeSet::new();
    for value in clean_string_list(values) {
        let permission = serde_json::from_value::<Permission>(Value::String(value.clone()))
            .map_err(|_| AdminError::InvalidCommand(format!("unknown permission: {value}")))?;
        permissions.insert(permission_text(&permission));
    }
    Ok(permissions.into_iter().collect())
}

fn operator_token_hash(token: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in b"mir2-admin-operator-token-v1"
        .iter()
        .copied()
        .chain([0])
        .chain(token.trim().as_bytes().iter().copied())
    {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn normalized_id(prefix: &str, name: &str, now_ms: u64) -> String {
    let mut slug = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        format!("{prefix}-{now_ms}")
    } else {
        format!("{prefix}-{slug}")
    }
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

fn row_to_system_mail_receipt(row: &Row) -> Result<SystemMailReceipt, AdminError> {
    let target_kind: String = row.get("target_kind");
    let attachment_count: i32 = row.get("attachment_count");
    let accepted_at_ms: i64 = row.get("accepted_at_ms");
    let delivered_count: i32 = row.get("delivered_count");
    let mail_ids_json: Value = row.get("mail_ids_json");
    let mail_ids = serde_json::from_value::<Vec<u32>>(mail_ids_json).map_err(|error| {
        AdminError::Repository(format!("decode system mail receipt ids failed: {error}"))
    })?;
    Ok(SystemMailReceipt {
        outbox_id: row.get("outbox_id"),
        target_kind: parse_mail_target_kind_text(&target_kind)?,
        target_id: row.get("target_id"),
        attachment_count: attachment_count.max(0) as usize,
        accepted_at_ms: accepted_at_ms.max(0) as u64,
        delivery_mode: row.get("delivery_mode"),
        delivered_count: delivered_count.max(0) as usize,
        mail_ids,
    })
}

fn row_to_activity_record(row: &Row) -> Result<AdminActivityRecord, AdminError> {
    let start_at_ms: Option<i64> = row.get("start_at_ms");
    let updated_at_ms: i64 = row.get("updated_at_ms");
    let realms_json: Value = row.get("realms_json");
    let realms = serde_json::from_value::<Vec<String>>(realms_json).map_err(|error| {
        AdminError::Repository(format!("decode activity realms failed: {error}"))
    })?;
    Ok(AdminActivityRecord {
        activity_id: row.get("activity_id"),
        name: row.get("name"),
        start_at_ms: start_at_ms.map(|value| value.max(0) as u64),
        activity_type: row.get("activity_type"),
        status: row.get("status"),
        signal: row.get("signal"),
        reward: row.get("reward"),
        condition: row.get("condition"),
        realms,
        updated_at_ms: updated_at_ms.max(0) as u64,
    })
}

fn row_to_price_feed_record(row: &Row) -> Result<AdminPriceFeedRecord, AdminError> {
    let latest_price: i64 = row.get("latest_price");
    let sample_count: i32 = row.get("sample_count");
    let updated_at_ms: i64 = row.get("updated_at_ms");
    Ok(AdminPriceFeedRecord {
        item: row.get("item"),
        latest_price: latest_price.max(0) as u64,
        sample_count: sample_count.max(0) as usize,
        source: row.get("source"),
        updated_at_ms: updated_at_ms.max(0) as u64,
    })
}

fn row_to_trade_graph_edge(row: &Row) -> Result<AdminRiskGraphEdge, AdminError> {
    let updated_at_ms: i64 = row.get("updated_at_ms");
    Ok(AdminRiskGraphEdge {
        edge_id: row.get("edge_id"),
        from: row.get("from_player_id"),
        to: row.get("to_player_id"),
        signal: row.get("signal"),
        risk: row.get("risk"),
        evidence: row.get("evidence"),
        updated_at_ms: updated_at_ms.max(0) as u64,
    })
}

fn row_to_zone_runtime_record(row: &Row) -> Result<AdminZoneRuntimeRecord, AdminError> {
    let map_count: i32 = row.get("map_count");
    let player_count: i32 = row.get("player_count");
    let tick_rate: i32 = row.get("tick_rate");
    let uptime_seconds: i64 = row.get("uptime_seconds");
    let updated_at_ms: i64 = row.get("updated_at_ms");
    Ok(AdminZoneRuntimeRecord {
        zone_id: row.get("zone_id"),
        name: row.get("name"),
        status: row.get("status"),
        host: row.get("host"),
        process_id: row.get("process_id"),
        map_count: map_count.max(0) as usize,
        player_count: player_count.max(0) as usize,
        tick_rate: tick_rate.max(0) as u32,
        uptime_seconds: uptime_seconds.max(0) as u64,
        source: row.get("source"),
        updated_at_ms: updated_at_ms.max(0) as u64,
    })
}

fn row_to_operator_record(row: &Row) -> Result<AdminOperatorRecord, AdminError> {
    let permissions_json: Value = row.get("permissions_json");
    let permissions = serde_json::from_value::<Vec<String>>(permissions_json).map_err(|error| {
        AdminError::Repository(format!("decode operator permissions failed: {error}"))
    })?;
    let token_hash: String = row.get("token_hash");
    let updated_at_ms: i64 = row.get("updated_at_ms");
    let last_authenticated_at_ms: Option<i64> = row.get("last_authenticated_at_ms");
    Ok(AdminOperatorRecord {
        operator_id: row.get("operator_id"),
        email: row.get("email"),
        role: row.get("role"),
        status: row.get("status"),
        permissions,
        token_configured: !token_hash.trim().is_empty(),
        updated_at_ms: updated_at_ms.max(0) as u64,
        last_authenticated_at_ms: last_authenticated_at_ms.map(|value| value.max(0) as u64),
    })
}

fn row_to_operator(row: &Row) -> Result<Operator, AdminError> {
    let permissions_json: Value = row.get("permissions_json");
    let permissions = serde_json::from_value::<Vec<String>>(permissions_json)
        .map_err(|error| AdminError::Repository(format!("decode operator failed: {error}")))?
        .into_iter()
        .map(|permission| parse_permission_text(&permission))
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(Operator {
        id: row.get("operator_id"),
        email: row.get("email"),
        role: row.get("role"),
        permissions,
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

fn mail_target_kind_text(target_kind: &MailTargetKind) -> String {
    serde_json::to_value(target_kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{target_kind:?}").to_lowercase())
}

fn parse_mail_target_kind_text(value: &str) -> Result<MailTargetKind, AdminError> {
    serde_json::from_value(Value::String(value.to_string()))
        .map_err(|error| AdminError::Repository(format!("decode mail target kind failed: {error}")))
}

fn command_type(command: &AdminCommand) -> String {
    match command {
        AdminCommand::SendSystemMail { .. } => "send_system_mail",
        AdminCommand::GrantItem { .. } => "grant_item",
        AdminCommand::GrantCurrency { .. } => "grant_currency",
        AdminCommand::KickPlayer { .. } => "kick_player",
        AdminCommand::BanAccount { .. } => "ban_account",
        AdminCommand::CreateAccount { .. } => "create_account",
        AdminCommand::UpdateAccount { .. } => "update_account",
        AdminCommand::DeleteAccount { .. } => "delete_account",
        AdminCommand::UpdateCharacter { .. } => "update_character",
        AdminCommand::ReturnSafeZone { .. } => "return_safe_zone",
        AdminCommand::KillPlayer { .. } => "kill_player",
        AdminCommand::KillPets { .. } => "kill_pets",
        AdminCommand::SetCharacterFlag { .. } => "set_character_flag",
        AdminCommand::SetChatBan { .. } => "set_chat_ban",
        AdminCommand::MessagePlayer { .. } => "message_player",
        AdminCommand::Broadcast { .. } => "broadcast",
        AdminCommand::MarketCancelListing { .. } => "market_cancel_listing",
        AdminCommand::MarketExpireListing { .. } => "market_expire_listing",
        AdminCommand::MarketDeleteListing { .. } => "market_delete_listing",
        AdminCommand::GuildRemoveMember { .. } => "guild_remove_member",
        AdminCommand::GuildSendMessage { .. } => "guild_send_message",
        AdminCommand::NameListAdd { .. } => "namelist_add",
        AdminCommand::NameListRemove { .. } => "namelist_remove",
        AdminCommand::NameListCreate { .. } => "namelist_create",
        AdminCommand::NameListDelete { .. } => "namelist_delete",
        AdminCommand::PublishContentBundle { .. } => "publish_content_bundle",
        AdminCommand::ServerControl { .. } => "server_control",
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
                "op-1".into(),
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
                "op-1".into(),
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
    fn approved_high_risk_command_requires_matching_requester_and_type() {
        let mut control = AdminControlPlane::new(RecordingExecutor::default());
        let mut envelope = sample_envelope(operator_with([Permission::InventoryGrantItem]));
        envelope.command_id = "cmd-grant-requester".into();
        envelope.approval_id = Some("approval-requester-mismatch".into());
        envelope.command = AdminCommand::GrantItem {
            character_id: "Scout".into(),
            item_id: "red-potion".into(),
            count: 1,
        };
        control
            .create_approval(ApprovalRecord::pending(
                "approval-requester-mismatch".into(),
                "cmd-grant-requester".into(),
                "grant_item".into(),
                "other-op".into(),
                "review different requester".into(),
                1_000,
            ))
            .expect("approval create");
        control
            .decide_approval(
                "approval-requester-mismatch",
                ApprovalStatus::Approved,
                "lead-gm".into(),
                Some("approved by peer".into()),
                1_500,
            )
            .expect("approval approve");

        let error = control
            .submit(envelope.clone(), 2_000)
            .expect_err("approval requested by another operator should reject");
        assert!(matches!(error, AdminError::InvalidCommand(_)));

        envelope.command_id = "cmd-grant-type".into();
        envelope.approval_id = Some("approval-type-mismatch".into());
        control
            .create_approval(ApprovalRecord::pending(
                "approval-type-mismatch".into(),
                "cmd-grant-type".into(),
                "grant_currency".into(),
                "op-1".into(),
                "review wrong command type".into(),
                3_000,
            ))
            .expect("approval create");
        control
            .decide_approval(
                "approval-type-mismatch",
                ApprovalStatus::Approved,
                "lead-gm".into(),
                Some("approved wrong type".into()),
                3_500,
            )
            .expect("approval approve");
        let error = control
            .submit(envelope, 4_000)
            .expect_err("approval with wrong command type should reject");
        assert!(matches!(error, AdminError::InvalidCommand(_)));
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
    fn admin_read_models_derive_from_json_account_store() {
        let path = std::env::temp_dir().join(format!(
            "mir2-admin-read-{}-{}.json",
            std::process::id(),
            now_ms()
        ));
        let default_character = SimulationConfig::default().default_character;
        let mut character = default_character.clone();
        character.index = 2;
        character.name = "LiveScout".into();
        character.level = 18;
        let mut account = AccountRecord::new(character.clone());
        account.is_banned = true;
        account.ban_reason = "manual operator ban".into();
        account.ban_until_ms = Some(now_ms().saturating_add(60_000));
        let save = account.saves.get_mut(&2).expect("save should exist");
        save.map_file_name = "0103".into();
        save.map_title = "WeaponStore".into();
        save.position.x = 6;
        save.position.y = 12;
        save.gold = 777;
        save.credit = 55;
        let mut systems = Stage5SystemsState::default();
        systems.mail.push(mir2_simulation::Stage5MailMessage {
            id: 1,
            from: "GM".into(),
            to: "LiveScout".into(),
            subject: "Real mail".into(),
            body: "Persisted read-model smoke".into(),
            gold: 10,
            items: Vec::new(),
            item_states_json: Vec::new(),
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        });
        save.stage5_systems_json =
            Some(serde_json::to_string(&systems).expect("systems should encode"));
        let mut store = AccountStore::new(default_character.clone());
        store.accounts.clear();
        store.accounts.insert("real-account".into(), account);
        store.save_to_path(&path).expect("store should save");

        let snapshot =
            read_json_account_snapshot(&path, default_character).expect("snapshot should load");
        let players = build_player_summaries(&snapshot, None);

        assert_eq!(snapshot.source, "json_account_store");
        assert_eq!(players.len(), 1);
        assert_eq!(players[0].player_id, "real-account:2");
        assert_eq!(players[0].character_name, "LiveScout");
        assert_eq!(players[0].gold, 777);
        assert_eq!(players[0].credit, 55);
        assert_eq!(players[0].status, "Banned");

        let detail =
            build_player_detail(&snapshot, "real-account:2", None).expect("detail should exist");
        assert_eq!(detail.mail_count, 1);
        assert_eq!(detail.unclaimed_mail_count, 1);
        assert_eq!(
            detail.active_ban_reason.as_deref(),
            Some("manual operator ban")
        );

        let economy = build_economy_read_model(&snapshot, Vec::new(), false);
        assert_eq!(economy.assets[0].asset, "Gold");
        assert_eq!(economy.assets[0].total, 777);
        assert_eq!(economy.assets[1].total, 55);
        let risk = build_risk_read_model(&snapshot, Vec::new(), false);
        assert_eq!(risk.cases.len(), 1);
        assert_eq!(risk.cases[0].evidence, "manual operator ban");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn admin_read_models_overlay_gateway_presence() {
        let default_character = SimulationConfig::default().default_character;
        let snapshot =
            snapshot_from_store("memory_account_store", AccountStore::new(default_character));
        let presence = GatewayPresenceSnapshot {
            source: "gateway_session_cache".into(),
            sessions: vec![GatewaySessionRecord {
                key: GatewaySessionKey {
                    account_id: "demo".into(),
                    character_index: 0,
                },
                character_name: "Scout".into(),
                map_file_name: Some("0103".into()),
                player_object_id: Some(7),
                player_hp: Some(88),
                player_max_hp: Some(99),
                gold: 9_999,
                tick: 42,
            }],
            error: None,
        };

        let players = build_player_summaries(&snapshot, Some(&presence));

        assert_eq!(players.len(), 1);
        assert_eq!(players[0].status, "Online");
        assert!(players[0].online);
        assert_eq!(
            players[0].online_source.as_deref(),
            Some("gateway_session_cache")
        );
        assert_eq!(players[0].map_file_name, "0103");
        assert_eq!(players[0].hp, 88);
        assert_eq!(players[0].max_hp, 99);
        assert_eq!(players[0].gold, 9_999);
        assert_eq!(players[0].runtime_tick, Some(42));

        let servers = build_servers_read_model(&snapshot, &presence, Vec::new(), false);
        assert_eq!(servers.zones_online, 1);
        assert_eq!(servers.zones_source, "gateway_session_cache");
        assert!(servers
            .services
            .iter()
            .any(|service| service.name == "Gateway Sessions" && service.status == "Healthy"));
    }

    #[tokio::test]
    async fn get_command_status_returns_one_command_record() {
        let path = std::env::temp_dir().join(format!(
            "mir2-admin-status-{}-{}.json",
            std::process::id(),
            now_ms()
        ));
        let config = SimulationConfig::default().with_account_store_path(path.clone());
        let read_models = Arc::new(AdminReadModelStore::from_config(&config));
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
            read_models,
            admin_store: None,
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
                "op-1".into(),
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
                "op-1".into(),
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
                "op-1".into(),
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
        let config = SimulationConfig::default();
        let read_models = Arc::new(AdminReadModelStore::from_config(&config));
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
            read_models,
            admin_store: None,
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
        assert!(query.contains("argMax(event_type, ingested_at)"));
        assert!(query.contains("GROUP BY event_id"));
        assert!(
            query.find("GROUP BY event_id").expect("group by clause")
                < query.find(" WHERE command_id").expect("outer where clause")
        );
        assert!(query.contains("LIMIT 500"));
    }

    #[test]
    fn clickhouse_gameplay_event_row_parses_json_each_row_output() {
        let record = parse_clickhouse_gameplay_event(
            r#"{"event_id":"primary:0:1","event_type":"gameplay.command.executed","zone_id":"primary","command_kind":"client.StartGame","account_id":"demo","character_index":"0","character_name":"Scout","packet_count":"7","snapshot_tick":"3","occurred_at_ms":"123","payload_json":"{}"}"#,
        )
        .expect("gameplay event row should parse");

        assert_eq!(record.event_id, "primary:0:1");
        assert_eq!(record.zone_id, "primary");
        assert_eq!(record.command_kind, "client.StartGame");
        assert_eq!(record.account_id.as_deref(), Some("demo"));
        assert_eq!(record.character_index, Some(0));
        assert_eq!(record.character_name.as_deref(), Some("Scout"));
        assert_eq!(record.packet_count, 7);
        assert_eq!(record.snapshot_tick, 3);
        assert_eq!(record.occurred_at_ms, 123);
    }

    #[test]
    fn clickhouse_gameplay_events_query_applies_filters_and_limit() {
        let query = build_clickhouse_gameplay_events_query(&GameplayEventQuery {
            limit: Some(1_000),
            zone_id: Some("primary".into()),
            command_kind: Some("client.StartGame".into()),
            account_id: Some("demo'1".into()),
            character_name: Some("Scout".into()),
        });

        assert!(query.contains("zone_id = 'primary'"));
        assert!(query.contains("command_kind = 'client.StartGame'"));
        assert!(query.contains("account_id = 'demo\\'1'"));
        assert!(query.contains("character_name = 'Scout'"));
        assert!(query.contains("FROM gameplay_events"));
        assert!(query.contains("GROUP BY event_id"));
        assert!(query.contains("LIMIT 500"));
    }

    #[test]
    fn clickhouse_gameplay_event_summary_row_parses_json_each_row_output() {
        let summary = parse_clickhouse_gameplay_event_summary(
            r#"{"command_kind":"client.PickUp","event_count":"12","last_occurred_at_ms":"456","max_snapshot_tick":"99"}"#,
        )
        .expect("gameplay summary row should parse");

        assert_eq!(summary.command_kind, "client.PickUp");
        assert_eq!(summary.event_count, 12);
        assert_eq!(summary.last_occurred_at_ms, 456);
        assert_eq!(summary.max_snapshot_tick, 99);
    }

    #[test]
    fn clickhouse_gameplay_event_summary_query_applies_window_filters_and_limit() {
        let query = build_clickhouse_gameplay_event_summary_query(
            &GameplayEventSummaryQuery {
                window_seconds: Some(3_600),
                limit: Some(1_000),
                zone_id: Some("primary".into()),
                command_kind: Some("client.PickUp".into()),
                max_lag_seconds: Some(120),
                min_events: Some(2),
            },
            10_000_000,
            normalized_gameplay_event_summary_window_seconds(Some(3_600)),
        );

        assert!(query.contains("occurred_at_ms >= 6400000"));
        assert!(query.contains("zone_id = 'primary'"));
        assert!(query.contains("command_kind = 'client.PickUp'"));
        assert!(query.contains("FROM gameplay_events"));
        assert!(query.contains("GROUP BY event_id"));
        assert!(query.contains("GROUP BY command_kind"));
        assert!(query.contains("ORDER BY event_count DESC"));
        assert!(query.contains("LIMIT 100"));
    }

    #[test]
    fn gameplay_event_summary_readiness_alerts_detect_missing_volume_and_lag() {
        let summary = GameplayEventSummary {
            total_count: 0,
            last_occurred_at_ms: None,
            commands: Vec::new(),
        };
        let alerts = gameplay_event_readiness_alerts(&summary, None, 1_000, 1);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].level, "warning");
        assert_eq!(alerts[0].code, "event_volume_below_minimum");

        let summary = GameplayEventSummary {
            total_count: 3,
            last_occurred_at_ms: Some(100),
            commands: Vec::new(),
        };
        let alerts = gameplay_event_readiness_alerts(&summary, Some(5_001), 5_000, 1);

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].level, "critical");
        assert_eq!(alerts[0].code, "event_lag_above_threshold");
    }

    #[test]
    fn gameplay_event_summary_thresholds_are_normalized() {
        assert_eq!(
            normalized_gameplay_event_summary_max_lag_ms(None, 300),
            300_000
        );
        assert_eq!(
            normalized_gameplay_event_summary_max_lag_ms(Some(0), 300),
            1_000
        );
        assert_eq!(
            normalized_gameplay_event_summary_max_lag_ms(Some(100_000), 300),
            86_400_000
        );
        assert_eq!(normalized_gameplay_event_summary_min_events(None), 1);
        assert_eq!(normalized_gameplay_event_summary_min_events(Some(0)), 0);
        assert_eq!(
            normalized_gameplay_event_summary_min_events(Some(2_000_000)),
            1_000_000
        );
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

    #[test]
    fn crystal_console_commands_mutate_accounts_market_guild_lists_and_content() {
        let unique = format!("{}-{}", std::process::id(), now_ms());
        let root = std::env::temp_dir().join(format!("mir2-admin-console-{unique}"));
        let account_path = root.join("accounts.json");
        let namelist_root = root.join("NameLists");
        let content_root = root.join("content");
        std::fs::create_dir_all(&root).expect("temp root");
        let config = SimulationConfig::default().with_account_store_path(account_path.clone());
        {
            let mut store = config.account_store.lock().expect("store lock");
            let account = store.accounts.get_mut("demo").expect("demo account");
            account.storage_password = "1234".into();
            account.storage_password_last_set_binary_datetime = 42;
            let save = account.saves.get_mut(&0).expect("demo save");
            let mut systems = Stage5SystemsState::default();
            systems.auction.push(mir2_simulation::Stage5AuctionListing {
                id: 1,
                seller: "Scout".into(),
                item_key: "auction-sword".into(),
                price: 50,
                sold: false,
                cancelled: false,
                expired: false,
            });
            systems.auction.push(mir2_simulation::Stage5AuctionListing {
                id: 2,
                seller: "Scout".into(),
                item_key: "auction-shield".into(),
                price: 75,
                sold: false,
                cancelled: false,
                expired: false,
            });
            systems.auction.push(mir2_simulation::Stage5AuctionListing {
                id: 3,
                seller: "Scout".into(),
                item_key: "auction-ring".into(),
                price: 90,
                sold: false,
                cancelled: false,
                expired: false,
            });
            systems.guild.name = "BichonGuard".into();
            systems.guild.members = vec!["Scout".into(), "Peer".into()];
            systems.guild.rank = "Guild Chief".into();
            save.stage5_systems_json = Some(serde_json::to_string(&systems).expect("systems"));
        }
        config.save_account_store().expect("seed store");
        std::fs::create_dir_all(&namelist_root).expect("namelist root");
        std::fs::write(namelist_root.join("OldList.txt"), "Retired\n").expect("old namelist");

        let executor = SystemMailExecutor::new(AccountStoreSystemMailDomain {
            gateway_mail_url: "http://127.0.0.1:1/admin/system-mail".into(),
            gateway_kick_url: "http://127.0.0.1:1/admin/kick-player".into(),
            gateway_control_url: "http://127.0.0.1:1/admin/control".into(),
            fallback_config: config,
            name_list_root: namelist_root.clone(),
            content_bundle_dir: content_root.clone(),
            receipts: Vec::new(),
        });
        let mut control = AdminControlPlane::new(executor);

        submit_console_for_test(
            &mut control,
            Permission::AccountWrite,
            AdminCommand::CreateAccount {
                account_id: "ops".into(),
                password: Some("secret".into()),
            },
        )
        .expect("create account");
        submit_console_for_test(
            &mut control,
            Permission::AccountWrite,
            AdminCommand::UpdateAccount {
                account_id: "demo".into(),
                password: None,
                storage_size: Some(120),
                clear_storage_password: true,
                unban: false,
            },
        )
        .expect("update account");
        let mut character_envelope = test_console_envelope(
            operator_with([Permission::CharacterWrite]),
            AdminCommand::UpdateCharacter {
                character_id: "demo:0".into(),
                name: Some("ScoutGM".into()),
                level: Some(11),
                gold: Some(333),
                credit: Some(7),
                map_file_name: None,
                map_title: None,
                position_x: None,
                position_y: None,
                hp: None,
                max_hp: None,
                mp: None,
                pk_points: Some(22),
            },
        );
        character_envelope.command_id = "cmd-character".into();
        character_envelope.approval_id = Some("approval-character".into());
        control
            .create_approval(ApprovalRecord::pending(
                "approval-character".into(),
                "cmd-character".into(),
                "update_character".into(),
                "op-1".into(),
                "character update review".into(),
                1_000,
            ))
            .expect("character approval");
        control
            .decide_approval(
                "approval-character",
                ApprovalStatus::Approved,
                "lead-gm".into(),
                Some("approved".into()),
                1_500,
            )
            .expect("approve character");
        control
            .submit(character_envelope, 2_000)
            .expect("update character");
        let mut flag_envelope = test_console_envelope(
            operator_with([Permission::CharacterWrite]),
            AdminCommand::SetCharacterFlag {
                character_id: "demo:0".into(),
                flag_index: 7,
                active: true,
            },
        );
        flag_envelope.command_id = "cmd-flag".into();
        flag_envelope.approval_id = Some("approval-flag".into());
        control
            .create_approval(ApprovalRecord::pending(
                "approval-flag".into(),
                "cmd-flag".into(),
                "set_character_flag".into(),
                "op-1".into(),
                "flag update review".into(),
                1_000,
            ))
            .expect("flag approval");
        control
            .decide_approval(
                "approval-flag",
                ApprovalStatus::Approved,
                "lead-gm".into(),
                Some("approved".into()),
                1_500,
            )
            .expect("approve flag");
        control.submit(flag_envelope, 2_000).expect("set flag");
        submit_console_for_test(
            &mut control,
            Permission::CharacterWrite,
            AdminCommand::SetChatBan {
                character_id: "demo:0".into(),
                banned: true,
                until_ms: Some(now_ms() + 120_000),
            },
        )
        .expect("set chat ban");
        submit_console_for_test(
            &mut control,
            Permission::CharacterWrite,
            AdminCommand::ReturnSafeZone {
                character_id: "demo:0".into(),
            },
        )
        .expect("return safe zone");
        submit_console_for_test(
            &mut control,
            Permission::MarketModerate,
            AdminCommand::MarketCancelListing {
                character_id: "demo:0".into(),
                listing_id: 1,
                notify_seller: true,
            },
        )
        .expect("cancel listing");
        submit_console_for_test(
            &mut control,
            Permission::MarketModerate,
            AdminCommand::MarketExpireListing {
                character_id: "demo:0".into(),
                listing_id: 2,
                notify_seller: true,
            },
        )
        .expect("expire listing");
        submit_approved_console_for_test(
            &mut control,
            Permission::MarketModerate,
            AdminCommand::MarketDeleteListing {
                character_id: "demo:0".into(),
                listing_id: 3,
                reason_message: Some("Listing removed by GM review.".into()),
            },
            "cmd-market-delete",
            "approval-market-delete",
        )
        .expect("delete listing");
        submit_console_for_test(
            &mut control,
            Permission::GuildModerate,
            AdminCommand::GuildSendMessage {
                guild_name: "BichonGuard".into(),
                message: "Ready".into(),
            },
        )
        .expect("guild message");
        submit_console_for_test(
            &mut control,
            Permission::GuildModerate,
            AdminCommand::GuildRemoveMember {
                guild_name: "BichonGuard".into(),
                member_name: "Peer".into(),
            },
        )
        .expect("guild remove");
        submit_console_for_test(
            &mut control,
            Permission::NameListManage,
            AdminCommand::NameListAdd {
                list_name: "Rebirth1".into(),
                player_name: "Scout".into(),
            },
        )
        .expect("namelist add");
        submit_console_for_test(
            &mut control,
            Permission::NameListManage,
            AdminCommand::NameListCreate {
                list_name: "NewList".into(),
            },
        )
        .expect("namelist create");
        submit_approved_console_for_test(
            &mut control,
            Permission::NameListManage,
            AdminCommand::NameListDelete {
                list_name: "OldList".into(),
            },
            "cmd-namelist-delete",
            "approval-namelist-delete",
        )
        .expect("namelist delete");
        submit_console_for_test(
            &mut control,
            Permission::ServerControl,
            AdminCommand::ServerControl {
                action: "reload_npcs".into(),
                target: None,
            },
        )
        .expect("server control fallback");

        let mut content_envelope = test_console_envelope(
            operator_with([Permission::ContentPublish]),
            AdminCommand::PublishContentBundle {
                bundle_id: "bundle-one".into(),
                category: "items".into(),
                record_key: "WoodenSword".into(),
                payload_json: serde_json::json!({"price": 1}),
            },
        );
        content_envelope.command_id = "cmd-content".into();
        content_envelope.approval_id = Some("approval-content".into());
        control
            .create_approval(ApprovalRecord::pending(
                "approval-content".into(),
                "cmd-content".into(),
                "publish_content_bundle".into(),
                "op-1".into(),
                "content publish review".into(),
                1_000,
            ))
            .expect("approval");
        control
            .decide_approval(
                "approval-content",
                ApprovalStatus::Approved,
                "lead-gm".into(),
                Some("approved".into()),
                1_500,
            )
            .expect("approve");
        control
            .submit(content_envelope, 2_000)
            .expect("content publish");

        let store =
            AccountStore::load_or_new(&account_path, SimulationConfig::default().default_character);
        assert!(store.accounts.contains_key("ops"));
        let demo = store.accounts.get("demo").expect("demo");
        assert_eq!(demo.storage_size, 120);
        assert!(demo.storage_password.is_empty());
        let save = demo.saves.get(&0).expect("demo save");
        assert_eq!(demo.characters[0].name, "ScoutGM");
        assert_eq!(save.character.name, "ScoutGM");
        assert_eq!(save.character.level, 11);
        assert_eq!(save.gold, 333);
        assert_eq!(save.credit, 7);
        assert_eq!(save.pk_points, 22);
        assert!(save.chat_banned);
        assert!(save.chat_ban_until_ms.is_some());
        assert_eq!(save.map_file_name, "0");
        assert_eq!((save.position.x, save.position.y), (284, 607));
        assert!(save
            .npc_flag_states_json
            .iter()
            .any(|entry| entry.contains("\"index\":7") && entry.contains("\"value\":true")));
        let systems: Stage5SystemsState =
            serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
        assert!(systems
            .auction
            .iter()
            .any(|listing| listing.id == 1 && listing.cancelled));
        assert!(systems
            .auction
            .iter()
            .any(|listing| listing.id == 2 && listing.expired));
        assert!(!systems.auction.iter().any(|listing| listing.id == 3));
        assert!(systems
            .mail
            .iter()
            .any(|mail| mail.subject == "GM Market Notice"));
        assert!(systems
            .guild
            .chat_log
            .iter()
            .any(|entry| entry.contains("Ready")));
        assert!(!systems.guild.members.iter().any(|member| member == "Peer"));
        let namelist = std::fs::read_to_string(namelist_root.join("Rebirth1.txt"))
            .expect("namelist should exist");
        assert!(namelist.lines().any(|line| line == "Scout"));
        assert!(namelist_root.join("NewList.txt").exists());
        assert!(!namelist_root.join("OldList.txt").exists());
        let bundle =
            std::fs::read_to_string(content_root.join("bundle-one.json")).expect("bundle exists");
        assert!(bundle.contains("\"recordKey\": \"WoodenSword\""));

        let snapshot = snapshot_from_store("test", store);
        assert_eq!(build_accounts_read_model(&snapshot).accounts.len(), 2);
        let market = build_market_read_model(&snapshot);
        assert!(market
            .listings
            .iter()
            .any(|listing| listing.listing_id == 2 && listing.expired));
        assert_eq!(
            build_guilds_read_model(&snapshot).guilds[0].guild_name,
            "BichonGuard"
        );
        let detail = build_player_detail(&snapshot, "demo:0", None).expect("detail");
        assert_eq!(detail.npc_flag_count, 1);
        assert_eq!(detail.active_npc_flags[0].index, 7);
        assert_eq!(detail.summary.pk_points, 22);
        assert!(detail.summary.chat_banned);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn console_command_request_accepts_frontend_payload_shape() {
        let request = serde_json::from_value::<SubmitConsoleCommandRequest>(serde_json::json!({
            "commandId": "cmd-console",
            "traceId": "trace-console",
            "target": {
                "targetType": "name_list",
                "targetId": "Rebirth1",
                "accountId": null,
                "characterId": null
            },
            "reason": "frontend generic console command",
            "command": {
                "type": "name_list_add",
                "list_name": "Rebirth1",
                "player_name": "Scout"
            }
        }))
        .expect("frontend command should deserialize");

        assert_eq!(request.command_id.as_deref(), Some("cmd-console"));
        assert!(matches!(request.target.target_type, TargetType::NameList));
        assert!(matches!(request.command, AdminCommand::NameListAdd { .. }));
    }

    fn submit_approved_console_for_test(
        control: &mut AdminControlPlane<SystemMailExecutor<AccountStoreSystemMailDomain>>,
        permission: Permission,
        command: AdminCommand,
        command_id: &str,
        approval_id: &str,
    ) -> Result<CommandExecutionResult, AdminError> {
        let command_type = command_type(&command);
        let mut envelope = test_console_envelope(operator_with([permission]), command);
        envelope.command_id = command_id.to_string();
        envelope.approval_id = Some(approval_id.to_string());
        control.create_approval(ApprovalRecord::pending(
            approval_id.to_string(),
            command_id.to_string(),
            command_type,
            "op-1".into(),
            "approved console command test".into(),
            1_000,
        ))?;
        control.decide_approval(
            approval_id,
            ApprovalStatus::Approved,
            "lead-gm".into(),
            Some("approved".into()),
            1_500,
        )?;
        control.submit(envelope, 2_000)
    }

    fn submit_console_for_test(
        control: &mut AdminControlPlane<SystemMailExecutor<AccountStoreSystemMailDomain>>,
        permission: Permission,
        command: AdminCommand,
    ) -> Result<CommandExecutionResult, AdminError> {
        let mut envelope = test_console_envelope(operator_with([permission]), command);
        envelope.command_id = format!(
            "cmd-test-{}-{}",
            command_type(&envelope.command),
            control.command_records(usize::MAX).len()
        );
        control.submit(envelope, 2_000)
    }

    fn test_console_envelope(operator: Operator, command: AdminCommand) -> AdminCommandEnvelope {
        let target_id = match &command {
            AdminCommand::CreateAccount { account_id, .. }
            | AdminCommand::UpdateAccount { account_id, .. }
            | AdminCommand::DeleteAccount { account_id }
            | AdminCommand::BanAccount { account_id, .. } => account_id.clone(),
            AdminCommand::UpdateCharacter { character_id, .. }
            | AdminCommand::ReturnSafeZone { character_id }
            | AdminCommand::KillPlayer { character_id }
            | AdminCommand::KillPets { character_id }
            | AdminCommand::SetCharacterFlag { character_id, .. }
            | AdminCommand::SetChatBan { character_id, .. }
            | AdminCommand::MessagePlayer { character_id, .. }
            | AdminCommand::MarketCancelListing { character_id, .. }
            | AdminCommand::MarketExpireListing { character_id, .. }
            | AdminCommand::MarketDeleteListing { character_id, .. } => character_id.clone(),
            AdminCommand::GuildRemoveMember { guild_name, .. }
            | AdminCommand::GuildSendMessage { guild_name, .. } => guild_name.clone(),
            AdminCommand::NameListAdd { list_name, .. }
            | AdminCommand::NameListRemove { list_name, .. }
            | AdminCommand::NameListCreate { list_name }
            | AdminCommand::NameListDelete { list_name } => list_name.clone(),
            AdminCommand::PublishContentBundle { bundle_id, .. } => bundle_id.clone(),
            AdminCommand::ServerControl { action, .. } => action.clone(),
            AdminCommand::Broadcast { .. } => "global".into(),
            AdminCommand::SendSystemMail { target_id, .. } => target_id.clone(),
            AdminCommand::GrantItem { character_id, .. }
            | AdminCommand::GrantCurrency { character_id, .. }
            | AdminCommand::KickPlayer { character_id } => character_id.clone(),
        };
        AdminCommandEnvelope {
            command_id: format!("cmd-test-{}", now_ms()),
            operator,
            target: AdminTarget {
                target_type: TargetType::World,
                target_id,
                account_id: None,
                character_id: None,
            },
            reason: "console command test reason".into(),
            approval_id: None,
            command,
            trace_id: format!("trace-test-{}", now_ms()),
            created_at_ms: 1_000,
        }
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
