use std::collections::{BTreeSet, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InMemoryAuditStore {
    records: Vec<AuditRecord>,
}

impl InMemoryAuditStore {
    pub fn push(&mut self, record: AuditRecord) {
        self.records.push(record);
    }

    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }
}

pub struct AdminControlPlane<E> {
    executor: E,
    audit_store: InMemoryAuditStore,
    seen_commands: HashSet<String>,
}

impl<E> AdminControlPlane<E>
where
    E: AdminCommandExecutor,
{
    pub fn new(executor: E) -> Self {
        Self {
            executor,
            audit_store: InMemoryAuditStore::default(),
            seen_commands: HashSet::new(),
        }
    }

    pub fn submit(
        &mut self,
        envelope: AdminCommandEnvelope,
        completed_at_ms: u64,
    ) -> Result<CommandExecutionResult, AdminError> {
        envelope.validate()?;

        if !self.seen_commands.insert(envelope.command_id.clone()) {
            return Err(AdminError::DuplicateCommand(envelope.command_id));
        }

        let permission = envelope.command.required_permission();
        let mut audit = AuditRecord::pending(&envelope, permission.clone());

        if !envelope.operator.has_permission(&permission) {
            audit.complete(
                CommandStatus::Denied,
                Some("permission_denied".into()),
                completed_at_ms,
            );
            self.audit_store.push(audit);
            return Err(AdminError::PermissionDenied { permission });
        }

        match self.executor.execute(&envelope) {
            Ok(result) => {
                audit.complete(result.status.clone(), None, completed_at_ms);
                self.audit_store.push(audit);
                Ok(result)
            }
            Err(error) => {
                audit.complete(
                    CommandStatus::Failed,
                    Some(error_code(&error).into()),
                    completed_at_ms,
                );
                self.audit_store.push(audit);
                Err(error)
            }
        }
    }

    pub fn audit_records(&self) -> &[AuditRecord] {
        self.audit_store.records()
    }

    pub fn executor(&self) -> &E {
        &self.executor
    }
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
    }
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
        assert_eq!(control.audit_records().len(), 1);
        let audit = &control.audit_records()[0];
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
        assert_eq!(control.audit_records().len(), 1);
        assert_eq!(control.audit_records()[0].status, CommandStatus::Denied);
        assert_eq!(
            control.audit_records()[0].error_code.as_deref(),
            Some("permission_denied")
        );
    }

    #[test]
    fn reason_is_required_before_command_is_recorded() {
        let mut control = AdminControlPlane::new(RecordingExecutor::default());
        let mut envelope = sample_envelope(operator_with([Permission::MailSendSystem]));
        envelope.reason = "  ".into();

        let error = control.submit(envelope, 2_000).expect_err("should reject");

        assert!(matches!(error, AdminError::InvalidCommand(_)));
        assert!(control.audit_records().is_empty());
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
        assert_eq!(control.audit_records().len(), 1);
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
        assert_eq!(control.audit_records().len(), 1);
        assert_eq!(control.audit_records()[0].status, CommandStatus::Failed);
        assert_eq!(
            control.audit_records()[0].error_code.as_deref(),
            Some("execution_failed")
        );
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
