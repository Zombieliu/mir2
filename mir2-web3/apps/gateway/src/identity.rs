//! Commercial player identity lifecycle.
//!
//! Gameplay account state remains in `mir2-simulation`; this module owns
//! revocable browser sessions, credential metadata, recovery codes and a
//! security audit trail.  Production uses Postgres.  The in-memory backend is
//! intentionally restricted to local development and unit tests.

use std::collections::BTreeMap;
use std::env;
use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use hmac::{Hmac, KeyInit, Mac};
use postgres::Client;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const SESSION_TOKEN_VERSION: &str = "mir2-identity-session-v1";
const DEFAULT_SESSION_TTL_MS: u64 = 12 * 60 * 60 * 1_000;
const MAX_SESSION_TTL_MS: u64 = 30 * 24 * 60 * 60 * 1_000;
const RECOVERY_CODE_COUNT: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IdentitySessionView {
    pub session_id: String,
    pub account_id: String,
    pub auth_method: String,
    pub credential_id: Option<String>,
    pub issued_at_ms: u64,
    pub last_seen_at_ms: u64,
    pub expires_at_ms: u64,
    pub revoked_at_ms: Option<u64>,
    pub revoked_reason: Option<String>,
    pub user_agent_summary: String,
    pub gateway_id: String,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IdentityCredentialView {
    pub credential_id: String,
    pub credential_kind: String,
    pub credential_subject: String,
    pub display_name: String,
    pub created_at_ms: u64,
    pub last_used_at_ms: Option<u64>,
    pub revoked_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IdentitySessionGrant {
    pub token: String,
    pub session: IdentitySessionView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedIdentitySession {
    pub account_id: String,
    pub session_id: String,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentitySessionTokenPayload {
    version: String,
    account_id: String,
    session_id: String,
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Debug, Clone)]
struct StoredRecoveryCode {
    code_id: String,
    hash: String,
    used_at_ms: Option<u64>,
    revoked_at_ms: Option<u64>,
}

#[derive(Debug, Default)]
struct MemoryIdentityStore {
    credentials: BTreeMap<String, Vec<IdentityCredentialView>>,
    sessions: BTreeMap<String, IdentitySessionView>,
    recovery_codes: BTreeMap<String, Vec<StoredRecoveryCode>>,
    audit_events: Vec<Value>,
}

#[derive(Clone)]
pub struct IdentityService {
    database_url: Option<String>,
    token_secret: Arc<String>,
    recovery_pepper: Arc<String>,
    gateway_id: Arc<String>,
    memory: Arc<Mutex<MemoryIdentityStore>>,
}

impl std::fmt::Debug for IdentityService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IdentityService")
            .field("backend", &self.backend_label())
            .field("gateway_id", &self.gateway_id)
            .finish_non_exhaustive()
    }
}

impl IdentityService {
    pub fn from_env(database_url: Option<String>) -> Result<Self, String> {
        let prod = production_like_env();
        if prod && database_url.is_none() {
            return Err(
                "commercial identity requires MIR2_ACCOUNT_STORE_DATABASE_URL in production"
                    .to_string(),
            );
        }
        let token_secret = secret_from_env(
            "MIR2_IDENTITY_SESSION_SECRET",
            "mir2-local-identity-session-secret-do-not-deploy",
            prod,
        )?;
        let recovery_pepper = secret_from_env(
            "MIR2_IDENTITY_RECOVERY_PEPPER",
            "mir2-local-recovery-pepper-do-not-deploy",
            prod,
        )?;
        let gateway_id = env::var("MIR2_GATEWAY_ID")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "local-gateway".to_string());
        let service = Self {
            database_url,
            token_secret: Arc::new(token_secret),
            recovery_pepper: Arc::new(recovery_pepper),
            gateway_id: Arc::new(gateway_id),
            memory: Arc::new(Mutex::new(MemoryIdentityStore::default())),
        };
        if service.database_url.is_some() {
            service.with_postgres(|client| mir2_simulation::apply_migrations(client))?;
        }
        Ok(service)
    }

    #[cfg(test)]
    pub fn local_for_tests() -> Self {
        Self {
            database_url: None,
            token_secret: Arc::new("test-identity-session-secret-at-least-32".to_string()),
            recovery_pepper: Arc::new("test-identity-recovery-pepper-at-least-32".to_string()),
            gateway_id: Arc::new("test-gateway".to_string()),
            memory: Arc::new(Mutex::new(MemoryIdentityStore::default())),
        }
    }

    pub fn backend_label(&self) -> &'static str {
        if self.database_url.is_some() {
            "postgres"
        } else {
            "in_memory_local"
        }
    }

    pub fn issue_session(
        &self,
        account_id: &str,
        auth_method: &str,
        credential_subject: &str,
        peer_address: &str,
        user_agent: &str,
    ) -> Result<IdentitySessionGrant, String> {
        validate_account_id(account_id)?;
        let now_ms = unix_now_ms();
        let expires_at_ms = now_ms.saturating_add(session_ttl_ms());
        let session_id = random_identifier(24);
        let credential_id =
            self.upsert_credential(account_id, auth_method, credential_subject, now_ms)?;
        let mut session = IdentitySessionView {
            session_id: session_id.clone(),
            account_id: account_id.to_string(),
            auth_method: canonical_auth_method(auth_method)?.to_string(),
            credential_id: Some(credential_id.clone()),
            issued_at_ms: now_ms,
            last_seen_at_ms: now_ms,
            expires_at_ms,
            revoked_at_ms: None,
            revoked_reason: None,
            user_agent_summary: sanitize_summary(user_agent, 160),
            gateway_id: self.gateway_id.as_ref().clone(),
            current: true,
        };
        let peer_fingerprint = self.peer_fingerprint(peer_address)?;
        if self.database_url.is_some() {
            self.with_postgres(|client| {
                client
                    .execute(
                        "INSERT INTO identity_sessions (session_id, account_id, auth_method, credential_id, issued_at_ms, last_seen_at_ms, expires_at_ms, peer_fingerprint, user_agent_summary, gateway_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
                        &[&session.session_id, &session.account_id, &session.auth_method, &credential_id, &(now_ms as i64), &(now_ms as i64), &(expires_at_ms as i64), &peer_fingerprint, &session.user_agent_summary, &session.gateway_id],
                    )
                    .map_err(|error| format!("identity session insert failed: {error}"))?;
                Ok(())
            })?;
        } else {
            self.memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?
                .sessions
                .insert(session_id.clone(), session.clone());
        }
        self.audit(
            Some(account_id),
            "session_issued",
            "success",
            "",
            Some(&session_id),
            Some(&credential_id),
            &peer_fingerprint,
            user_agent,
            json!({"authMethod": session.auth_method}),
        )?;
        let payload = IdentitySessionTokenPayload {
            version: SESSION_TOKEN_VERSION.to_string(),
            account_id: account_id.to_string(),
            session_id,
            issued_at_ms: now_ms,
            expires_at_ms,
        };
        let token = self.sign_session_token(&payload)?;
        session.current = true;
        Ok(IdentitySessionGrant { token, session })
    }

    pub fn verify_session_token(&self, token: &str) -> Result<VerifiedIdentitySession, String> {
        let payload = self.decode_session_token(token)?;
        let now_ms = unix_now_ms();
        if payload.expires_at_ms <= now_ms || payload.issued_at_ms > now_ms.saturating_add(30_000) {
            return Err("identity session expired".to_string());
        }
        let stored = self
            .session_by_id(&payload.session_id)?
            .ok_or_else(|| "identity session not found".to_string())?;
        if stored.account_id != payload.account_id
            || stored.expires_at_ms != payload.expires_at_ms
            || stored.revoked_at_ms.is_some()
            || stored.expires_at_ms <= now_ms
        {
            return Err("identity session is not active".to_string());
        }
        Ok(VerifiedIdentitySession {
            account_id: payload.account_id,
            session_id: payload.session_id,
            expires_at_ms: payload.expires_at_ms,
        })
    }

    pub fn touch_session(&self, verified: &VerifiedIdentitySession) -> Result<(), String> {
        let now_ms = unix_now_ms();
        if self.database_url.is_some() {
            self.with_postgres(|client| {
                client
                    .execute(
                        "UPDATE identity_sessions SET last_seen_at_ms=$1, updated_at=now() WHERE session_id=$2 AND account_id=$3 AND revoked_at_ms IS NULL AND expires_at_ms>$1",
                        &[&(now_ms as i64), &verified.session_id, &verified.account_id],
                    )
                    .map_err(|error| format!("identity session touch failed: {error}"))
                    .and_then(|count| {
                        if count == 1 {
                            Ok(())
                        } else {
                            Err("identity session is not active".to_string())
                        }
                    })
            })
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?;
            let session = memory
                .sessions
                .get_mut(&verified.session_id)
                .filter(|session| {
                    session.account_id == verified.account_id
                        && session.revoked_at_ms.is_none()
                        && session.expires_at_ms > now_ms
                })
                .ok_or_else(|| "identity session is not active".to_string())?;
            session.last_seen_at_ms = now_ms;
            Ok(())
        }
    }

    pub fn record_auth_security_event(
        &self,
        account_id: Option<&str>,
        event_type: &str,
        outcome: &str,
        reason_code: &str,
        peer: &str,
        user_agent: &str,
    ) -> Result<(), String> {
        if let Some(account_id) = account_id {
            validate_account_id(account_id)?;
        }
        if !matches!(outcome, "success" | "failure" | "blocked") {
            return Err("invalid identity audit outcome".to_string());
        }
        let peer_fingerprint = self.peer_fingerprint(peer)?;
        self.audit(
            account_id,
            event_type,
            outcome,
            reason_code,
            None,
            None,
            &peer_fingerprint,
            user_agent,
            json!({}),
        )
    }

    pub fn list_sessions(
        &self,
        verified: &VerifiedIdentitySession,
    ) -> Result<Vec<IdentitySessionView>, String> {
        let mut sessions = if self.database_url.is_some() {
            self.with_postgres(|client| {
                client
                    .query(
                        "SELECT session_id, account_id, auth_method, credential_id, issued_at_ms, last_seen_at_ms, expires_at_ms, revoked_at_ms, revoked_reason, user_agent_summary, gateway_id FROM identity_sessions WHERE account_id=$1 ORDER BY issued_at_ms DESC LIMIT 100",
                        &[&verified.account_id],
                    )
                    .map_err(|error| format!("identity session list failed: {error}"))?
                    .into_iter()
                    .map(|row| IdentitySessionView {
                        session_id: row.get(0), account_id: row.get(1), auth_method: row.get(2), credential_id: row.get(3),
                        issued_at_ms: i64_to_u64(row.get(4)), last_seen_at_ms: i64_to_u64(row.get(5)), expires_at_ms: i64_to_u64(row.get(6)),
                        revoked_at_ms: row.get::<_, Option<i64>>(7).map(i64_to_u64), revoked_reason: row.get(8), user_agent_summary: row.get(9), gateway_id: row.get(10), current: false,
                    })
                    .collect::<Vec<_>>()
                    .pipe(Ok)
            })?
        } else {
            self.memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?
                .sessions
                .values()
                .filter(|session| session.account_id == verified.account_id)
                .cloned()
                .collect()
        };
        for session in &mut sessions {
            session.current = session.session_id == verified.session_id;
        }
        sessions.sort_by(|left, right| right.issued_at_ms.cmp(&left.issued_at_ms));
        Ok(sessions)
    }

    pub fn list_credentials(
        &self,
        verified: &VerifiedIdentitySession,
    ) -> Result<Vec<IdentityCredentialView>, String> {
        if self.database_url.is_some() {
            self.with_postgres(|client| {
                let credentials = client
                    .query(
                        "SELECT credential_id, credential_kind, credential_subject, display_name, created_at_ms, last_used_at_ms, revoked_at_ms FROM identity_credentials WHERE account_id=$1 ORDER BY created_at_ms DESC LIMIT 100",
                        &[&verified.account_id],
                    )
                    .map_err(|error| format!("identity credential list failed: {error}"))?
                    .into_iter()
                    .map(|row| IdentityCredentialView {
                        credential_id: row.get(0), credential_kind: row.get(1), credential_subject: redact_credential_subject(&row.get::<_, String>(2)), display_name: row.get(3),
                        created_at_ms: i64_to_u64(row.get(4)), last_used_at_ms: row.get::<_, Option<i64>>(5).map(i64_to_u64), revoked_at_ms: row.get::<_, Option<i64>>(6).map(i64_to_u64),
                    })
                    .collect();
                Ok(credentials)
            })
        } else {
            let mut credentials = self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?
                .credentials
                .get(&verified.account_id)
                .cloned()
                .unwrap_or_default();
            for credential in &mut credentials {
                credential.credential_subject =
                    redact_credential_subject(&credential.credential_subject);
            }
            Ok(credentials)
        }
    }

    pub fn list_account_session_ids(&self, account_id: &str) -> Result<Vec<String>, String> {
        validate_account_id(account_id)?;
        if self.database_url.is_some() {
            self.with_postgres(|client| {
                client
                    .query(
                        "SELECT session_id FROM identity_sessions WHERE account_id=$1 AND revoked_at_ms IS NULL AND expires_at_ms>$2",
                        &[&account_id, &(unix_now_ms() as i64)],
                    )
                    .map(|rows| rows.into_iter().map(|row| row.get(0)).collect())
                    .map_err(|error| format!("identity account-session list failed: {error}"))
            })
        } else {
            let now_ms = unix_now_ms();
            Ok(self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?
                .sessions
                .values()
                .filter(|session| {
                    session.account_id == account_id
                        && session.revoked_at_ms.is_none()
                        && session.expires_at_ms > now_ms
                })
                .map(|session| session.session_id.clone())
                .collect())
        }
    }

    pub fn operator_account_security(
        &self,
        account_id: &str,
    ) -> Result<
        (
            Vec<IdentitySessionView>,
            Vec<IdentityCredentialView>,
            Vec<Value>,
        ),
        String,
    > {
        validate_account_id(account_id)?;
        let operator_view = VerifiedIdentitySession {
            account_id: account_id.to_string(),
            session_id: String::new(),
            expires_at_ms: u64::MAX,
        };
        let sessions = self.list_sessions(&operator_view)?;
        let credentials = self.list_credentials(&operator_view)?;
        let audit_events = if self.database_url.is_some() {
            self.with_postgres(|client| {
                client
                    .query(
                        "SELECT event_id, event_type, outcome, reason_code, session_id, credential_id, peer_fingerprint, user_agent_summary, trace_id, details_json, occurred_at_ms FROM identity_audit_events WHERE account_id=$1 ORDER BY occurred_at_ms DESC LIMIT 100",
                        &[&account_id],
                    )
                    .map_err(|error| format!("identity audit list failed: {error}"))
                    .map(|rows| {
                        rows.into_iter()
                            .map(|row| json!({
                                "eventId": row.get::<_, String>(0),
                                "eventType": row.get::<_, String>(1),
                                "outcome": row.get::<_, String>(2),
                                "reasonCode": row.get::<_, String>(3),
                                "sessionId": row.get::<_, Option<String>>(4),
                                "credentialId": row.get::<_, Option<String>>(5),
                                "peerFingerprint": row.get::<_, String>(6),
                                "userAgentSummary": row.get::<_, String>(7),
                                "traceId": row.get::<_, String>(8),
                                "details": row.get::<_, Value>(9),
                                "occurredAtMs": i64_to_u64(row.get(10)),
                            }))
                            .collect()
                    })
            })?
        } else {
            let memory = self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?;
            memory
                .audit_events
                .iter()
                .rev()
                .filter(|event| event.get("accountId").and_then(Value::as_str) == Some(account_id))
                .take(100)
                .cloned()
                .collect()
        };
        Ok((sessions, credentials, audit_events))
    }

    pub fn revoke_session(
        &self,
        verified: &VerifiedIdentitySession,
        target_session_id: &str,
        reason: &str,
    ) -> Result<bool, String> {
        if target_session_id.trim().is_empty() {
            return Err("sessionId is required".to_string());
        }
        let now_ms = unix_now_ms();
        let reason = sanitize_summary(reason, 160);
        let changed = if self.database_url.is_some() {
            self.with_postgres(|client| {
                client
                    .execute(
                        "UPDATE identity_sessions SET revoked_at_ms=$1, revoked_reason=$2, updated_at=now() WHERE session_id=$3 AND account_id=$4 AND revoked_at_ms IS NULL",
                        &[&(now_ms as i64), &reason, &target_session_id, &verified.account_id],
                    )
                    .map(|count| count > 0)
                    .map_err(|error| format!("identity session revoke failed: {error}"))
            })?
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?;
            match memory.sessions.get_mut(target_session_id) {
                Some(session)
                    if session.account_id == verified.account_id
                        && session.revoked_at_ms.is_none() =>
                {
                    session.revoked_at_ms = Some(now_ms);
                    session.revoked_reason = Some(reason.clone());
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.audit(
                Some(&verified.account_id),
                "session_revoked",
                "success",
                "",
                Some(target_session_id),
                None,
                "",
                "",
                json!({"reason": reason}),
            )?;
        }
        Ok(changed)
    }

    pub fn revoke_all_other_sessions(
        &self,
        verified: &VerifiedIdentitySession,
    ) -> Result<u64, String> {
        let now_ms = unix_now_ms();
        let count = if self.database_url.is_some() {
            self.with_postgres(|client| {
                client
                    .execute(
                        "UPDATE identity_sessions SET revoked_at_ms=$1, revoked_reason='player_all_devices_logout', updated_at=now() WHERE account_id=$2 AND session_id<>$3 AND revoked_at_ms IS NULL",
                        &[&(now_ms as i64), &verified.account_id, &verified.session_id],
                    )
                    .map_err(|error| format!("identity all-session revoke failed: {error}"))
            })?
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?;
            let mut count = 0;
            for session in memory.sessions.values_mut() {
                if session.account_id == verified.account_id
                    && session.session_id != verified.session_id
                    && session.revoked_at_ms.is_none()
                {
                    session.revoked_at_ms = Some(now_ms);
                    session.revoked_reason = Some("player_all_devices_logout".to_string());
                    count += 1;
                }
            }
            count
        };
        self.audit(
            Some(&verified.account_id),
            "sessions_revoked_all_other",
            "success",
            "",
            Some(&verified.session_id),
            None,
            "",
            "",
            json!({"count": count}),
        )?;
        Ok(count)
    }

    pub fn generate_recovery_codes(
        &self,
        verified: &VerifiedIdentitySession,
    ) -> Result<Vec<String>, String> {
        if unix_now_ms().saturating_sub(
            self.session_by_id(&verified.session_id)?
                .ok_or_else(|| "identity session not found".to_string())?
                .issued_at_ms,
        ) > 10 * 60 * 1_000
        {
            return Err("recent authentication is required to rotate recovery codes".to_string());
        }
        let now_ms = unix_now_ms();
        let codes = (0..RECOVERY_CODE_COUNT)
            .map(|_| random_recovery_code())
            .collect::<Vec<_>>();
        let stored = codes
            .iter()
            .map(|code| StoredRecoveryCode {
                code_id: random_identifier(18),
                hash: self
                    .recovery_code_hash(code)
                    .expect("configured recovery pepper must initialize HMAC"),
                used_at_ms: None,
                revoked_at_ms: None,
            })
            .collect::<Vec<_>>();
        if self.database_url.is_some() {
            self.with_postgres(|client| {
                let mut transaction = client.transaction().map_err(|error| format!("recovery code transaction failed: {error}"))?;
                transaction.execute("UPDATE identity_recovery_codes SET revoked_at_ms=$1 WHERE account_id=$2 AND used_at_ms IS NULL AND revoked_at_ms IS NULL", &[&(now_ms as i64), &verified.account_id]).map_err(|error| format!("old recovery code revoke failed: {error}"))?;
                for code in &stored {
                    transaction.execute("INSERT INTO identity_recovery_codes (recovery_code_id, account_id, code_hash, created_at_ms) VALUES ($1,$2,$3,$4)", &[&code.code_id, &verified.account_id, &code.hash, &(now_ms as i64)]).map_err(|error| format!("recovery code insert failed: {error}"))?;
                }
                transaction.commit().map_err(|error| format!("recovery code commit failed: {error}"))
            })?;
        } else {
            self.memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?
                .recovery_codes
                .insert(verified.account_id.clone(), stored);
        }
        self.audit(
            Some(&verified.account_id),
            "recovery_codes_rotated",
            "success",
            "",
            Some(&verified.session_id),
            None,
            "",
            "",
            json!({"count": codes.len()}),
        )?;
        Ok(codes)
    }

    pub fn consume_recovery_code(&self, account_id: &str, code: &str) -> Result<bool, String> {
        validate_account_id(account_id)?;
        let hash = self.recovery_code_hash(code)?;
        let now_ms = unix_now_ms();
        let consumed = if self.database_url.is_some() {
            self.with_postgres(|client| {
                client.execute("UPDATE identity_recovery_codes SET used_at_ms=$1 WHERE account_id=$2 AND code_hash=$3 AND used_at_ms IS NULL AND revoked_at_ms IS NULL", &[&(now_ms as i64), &account_id, &hash])
                    .map(|count| count == 1)
                    .map_err(|error| format!("recovery code consume failed: {error}"))
            })?
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?;
            memory
                .recovery_codes
                .get_mut(account_id)
                .and_then(|codes| {
                    codes.iter_mut().find(|stored| {
                        stored.hash == hash
                            && stored.used_at_ms.is_none()
                            && stored.revoked_at_ms.is_none()
                    })
                })
                .map(|stored| {
                    stored.used_at_ms = Some(now_ms);
                    true
                })
                .unwrap_or(false)
        };
        self.audit(
            Some(account_id),
            "recovery_code_consumed",
            if consumed { "success" } else { "failure" },
            if consumed { "" } else { "invalid_or_used" },
            None,
            None,
            "",
            "",
            json!({}),
        )?;
        Ok(consumed)
    }

    pub fn revoke_all_account_sessions(
        &self,
        account_id: &str,
        reason: &str,
    ) -> Result<u64, String> {
        validate_account_id(account_id)?;
        let now_ms = unix_now_ms();
        let reason = sanitize_summary(reason, 160);
        let count = if self.database_url.is_some() {
            self.with_postgres(|client| {
                client.execute("UPDATE identity_sessions SET revoked_at_ms=$1, revoked_reason=$2, updated_at=now() WHERE account_id=$3 AND revoked_at_ms IS NULL", &[&(now_ms as i64), &reason, &account_id])
                    .map_err(|error| format!("identity account-session revoke failed: {error}"))
            })?
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?;
            let mut count = 0;
            for session in memory.sessions.values_mut() {
                if session.account_id == account_id && session.revoked_at_ms.is_none() {
                    session.revoked_at_ms = Some(now_ms);
                    session.revoked_reason = Some(reason.clone());
                    count += 1;
                }
            }
            count
        };
        self.audit(
            Some(account_id),
            "sessions_revoked_all",
            "success",
            "",
            None,
            None,
            "",
            "",
            json!({"count": count, "reason": reason}),
        )?;
        Ok(count)
    }

    pub fn revoke_credential(
        &self,
        verified: &VerifiedIdentitySession,
        credential_id: &str,
        reason: &str,
    ) -> Result<bool, String> {
        let current = self
            .session_by_id(&verified.session_id)?
            .ok_or_else(|| "identity session not found".to_string())?;
        if unix_now_ms().saturating_sub(current.issued_at_ms) > 10 * 60 * 1_000 {
            return Err("recent authentication is required to revoke a credential".to_string());
        }
        if current.credential_id.as_deref() == Some(credential_id) {
            return Err(
                "the credential used by the current session cannot revoke itself".to_string(),
            );
        }
        let now_ms = unix_now_ms();
        let reason = sanitize_summary(reason, 160);
        let changed = if self.database_url.is_some() {
            self.with_postgres(|client| {
                let mut transaction = client.transaction().map_err(|error| format!("credential revoke transaction failed: {error}"))?;
                let live_count: i64 = transaction.query_one("SELECT count(*) FROM identity_credentials WHERE account_id=$1 AND revoked_at_ms IS NULL", &[&verified.account_id]).map_err(|error| format!("credential count failed: {error}"))?.get(0);
                if live_count <= 1 { return Err("the last active credential cannot be revoked".to_string()); }
                let changed = transaction.execute("UPDATE identity_credentials SET revoked_at_ms=$1, revoked_reason=$2, updated_at=now() WHERE credential_id=$3 AND account_id=$4 AND revoked_at_ms IS NULL", &[&(now_ms as i64), &reason, &credential_id, &verified.account_id]).map_err(|error| format!("credential revoke failed: {error}"))? == 1;
                if changed {
                    transaction.execute("UPDATE identity_sessions SET revoked_at_ms=$1, revoked_reason='credential_revoked', updated_at=now() WHERE account_id=$2 AND credential_id=$3 AND revoked_at_ms IS NULL", &[&(now_ms as i64), &verified.account_id, &credential_id]).map_err(|error| format!("credential session revoke failed: {error}"))?;
                }
                transaction.commit().map_err(|error| format!("credential revoke commit failed: {error}"))?;
                Ok(changed)
            })?
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?;
            let credentials = memory
                .credentials
                .get_mut(&verified.account_id)
                .ok_or_else(|| "account credentials were not found".to_string())?;
            if credentials
                .iter()
                .filter(|item| item.revoked_at_ms.is_none())
                .count()
                <= 1
            {
                return Err("the last active credential cannot be revoked".to_string());
            }
            let changed = credentials
                .iter_mut()
                .find(|item| item.credential_id == credential_id && item.revoked_at_ms.is_none())
                .map(|item| {
                    item.revoked_at_ms = Some(now_ms);
                    true
                })
                .unwrap_or(false);
            if changed {
                for session in memory.sessions.values_mut() {
                    if session.account_id == verified.account_id
                        && session.credential_id.as_deref() == Some(credential_id)
                        && session.revoked_at_ms.is_none()
                    {
                        session.revoked_at_ms = Some(now_ms);
                        session.revoked_reason = Some("credential_revoked".to_string());
                    }
                }
            }
            changed
        };
        if changed {
            self.audit(
                Some(&verified.account_id),
                "credential_revoked",
                "success",
                "",
                Some(&verified.session_id),
                Some(credential_id),
                "",
                "",
                json!({"reason": reason}),
            )?;
        }
        Ok(changed)
    }

    pub fn bind_sui_credential(
        &self,
        verified: &VerifiedIdentitySession,
        auth_method: &str,
        address: &str,
    ) -> Result<String, String> {
        let current = self
            .session_by_id(&verified.session_id)?
            .ok_or_else(|| "identity session not found".to_string())?;
        if unix_now_ms().saturating_sub(current.issued_at_ms) > 10 * 60 * 1_000 {
            return Err("recent authentication is required to bind a credential".to_string());
        }
        if !matches!(
            canonical_auth_method(auth_method)?,
            "sui_passkey" | "sui_wallet"
        ) {
            return Err("only Sui passkey or wallet credentials can be bound".to_string());
        }
        let credential_id =
            self.upsert_credential(&verified.account_id, auth_method, address, unix_now_ms())?;
        self.audit(
            Some(&verified.account_id),
            "credential_bound",
            "success",
            "",
            Some(&verified.session_id),
            Some(&credential_id),
            "",
            "",
            json!({"kind": credential_kind(auth_method)?}),
        )?;
        Ok(credential_id)
    }

    pub fn resolve_sui_account(
        &self,
        auth_method: &str,
        address: &str,
    ) -> Result<Option<String>, String> {
        let kind = credential_kind(auth_method)?;
        if !matches!(kind, "sui_passkey" | "sui_wallet") {
            return Err("only Sui credentials can resolve an account".to_string());
        }
        let address = sanitize_summary(address, 512);
        if self.database_url.is_some() {
            self.with_postgres(|client| {
                client
                    .query_opt(
                        "SELECT account_id FROM identity_credentials WHERE credential_kind=$1 AND credential_subject=$2 AND revoked_at_ms IS NULL",
                        &[&kind, &address],
                    )
                    .map(|row| row.map(|row| row.get(0)))
                    .map_err(|error| format!("Sui credential resolution failed: {error}"))
            })
        } else {
            let memory = self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?;
            Ok(memory
                .credentials
                .iter()
                .find_map(|(account_id, credentials)| {
                    credentials
                        .iter()
                        .any(|item| {
                            item.credential_kind == kind
                                && item.credential_subject == address
                                && item.revoked_at_ms.is_none()
                        })
                        .then(|| account_id.clone())
                }))
        }
    }

    fn upsert_credential(
        &self,
        account_id: &str,
        auth_method: &str,
        subject: &str,
        now_ms: u64,
    ) -> Result<String, String> {
        let kind = credential_kind(auth_method)?;
        let subject = sanitize_summary(subject, 512);
        if subject.is_empty() {
            return Err("credential subject is required".to_string());
        }
        if self.database_url.is_some() {
            self.with_postgres(|client| {
                if let Some(row) = client.query_opt("SELECT credential_id, account_id FROM identity_credentials WHERE credential_kind=$1 AND credential_subject=$2 AND revoked_at_ms IS NULL", &[&kind, &subject]).map_err(|error| format!("credential lookup failed: {error}"))? {
                    let credential_id: String = row.get(0);
                    let owner_account_id: String = row.get(1);
                    if owner_account_id != account_id {
                        return Err("credential is already bound to another account".to_string());
                    }
                    client.execute("UPDATE identity_credentials SET last_used_at_ms=$1, updated_at=now() WHERE credential_id=$2", &[&(now_ms as i64), &credential_id]).map_err(|error| format!("credential touch failed: {error}"))?;
                    return Ok(credential_id);
                }
                let credential_id = random_identifier(18);
                client.execute("INSERT INTO identity_credentials (credential_id, account_id, credential_kind, credential_subject, display_name, created_at_ms, last_used_at_ms) VALUES ($1,$2,$3,$4,$5,$6,$6)", &[&credential_id, &account_id, &kind, &subject, &default_credential_name(kind), &(now_ms as i64)]).map_err(|error| format!("credential insert failed: {error}"))?;
                Ok(credential_id)
            })
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?;
            if memory.credentials.iter().any(|(owner, credentials)| {
                owner != account_id
                    && credentials.iter().any(|item| {
                        item.credential_kind == kind
                            && item.credential_subject == subject
                            && item.revoked_at_ms.is_none()
                    })
            }) {
                return Err("credential is already bound to another account".to_string());
            }
            let credentials = memory
                .credentials
                .entry(account_id.to_string())
                .or_default();
            if let Some(existing) = credentials.iter_mut().find(|item| {
                item.credential_kind == kind
                    && item.credential_subject == subject
                    && item.revoked_at_ms.is_none()
            }) {
                existing.last_used_at_ms = Some(now_ms);
                return Ok(existing.credential_id.clone());
            }
            let credential_id = random_identifier(18);
            credentials.push(IdentityCredentialView {
                credential_id: credential_id.clone(),
                credential_kind: kind.to_string(),
                credential_subject: subject,
                display_name: default_credential_name(kind),
                created_at_ms: now_ms,
                last_used_at_ms: Some(now_ms),
                revoked_at_ms: None,
            });
            Ok(credential_id)
        }
    }

    fn session_by_id(&self, session_id: &str) -> Result<Option<IdentitySessionView>, String> {
        if self.database_url.is_some() {
            self.with_postgres(|client| {
                client.query_opt("SELECT session_id, account_id, auth_method, credential_id, issued_at_ms, last_seen_at_ms, expires_at_ms, revoked_at_ms, revoked_reason, user_agent_summary, gateway_id FROM identity_sessions WHERE session_id=$1", &[&session_id])
                    .map_err(|error| format!("identity session lookup failed: {error}"))
                    .map(|row| row.map(|row| IdentitySessionView { session_id: row.get(0), account_id: row.get(1), auth_method: row.get(2), credential_id: row.get(3), issued_at_ms: i64_to_u64(row.get(4)), last_seen_at_ms: i64_to_u64(row.get(5)), expires_at_ms: i64_to_u64(row.get(6)), revoked_at_ms: row.get::<_, Option<i64>>(7).map(i64_to_u64), revoked_reason: row.get(8), user_agent_summary: row.get(9), gateway_id: row.get(10), current: false }))
            })
        } else {
            Ok(self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?
                .sessions
                .get(session_id)
                .cloned())
        }
    }

    fn sign_session_token(&self, payload: &IdentitySessionTokenPayload) -> Result<String, String> {
        let encoded = URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(payload).map_err(|error| {
                format!("identity session token serialization failed: {error}")
            })?);
        let mut mac = HmacSha256::new_from_slice(self.token_secret.as_bytes())
            .map_err(|_| "identity session secret is invalid".to_string())?;
        mac.update(encoded.as_bytes());
        Ok(format!(
            "{encoded}.{}",
            URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
        ))
    }

    fn decode_session_token(&self, token: &str) -> Result<IdentitySessionTokenPayload, String> {
        let (payload, signature) = token
            .split_once('.')
            .ok_or_else(|| "invalid identity session token".to_string())?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| "invalid identity session token".to_string())?;
        let mut mac = HmacSha256::new_from_slice(self.token_secret.as_bytes())
            .map_err(|_| "identity session secret is invalid".to_string())?;
        mac.update(payload.as_bytes());
        mac.verify_slice(&signature)
            .map_err(|_| "invalid identity session token".to_string())?;
        let decoded = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| "invalid identity session token".to_string())?;
        let parsed: IdentitySessionTokenPayload = serde_json::from_slice(&decoded)
            .map_err(|_| "invalid identity session token".to_string())?;
        if parsed.version != SESSION_TOKEN_VERSION
            || parsed.account_id.is_empty()
            || parsed.session_id.is_empty()
        {
            return Err("invalid identity session token".to_string());
        }
        Ok(parsed)
    }

    fn recovery_code_hash(&self, code: &str) -> Result<String, String> {
        let canonical = code.trim().replace('-', "").to_ascii_uppercase();
        if canonical.len() != 20 || !canonical.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            return Err("invalid recovery code".to_string());
        }
        let mut mac = HmacSha256::new_from_slice(self.recovery_pepper.as_bytes())
            .map_err(|_| "identity recovery pepper is invalid".to_string())?;
        mac.update(canonical.as_bytes());
        Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
    }

    pub fn peer_fingerprint(&self, peer: &str) -> Result<String, String> {
        let mut mac = HmacSha256::new_from_slice(self.token_secret.as_bytes())
            .map_err(|_| "identity session secret is invalid".to_string())?;
        mac.update(peer.as_bytes());
        Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
    }

    #[allow(clippy::too_many_arguments)]
    fn audit(
        &self,
        account_id: Option<&str>,
        event_type: &str,
        outcome: &str,
        reason_code: &str,
        session_id: Option<&str>,
        credential_id: Option<&str>,
        peer_fingerprint: &str,
        user_agent: &str,
        details: Value,
    ) -> Result<(), String> {
        let event_id = random_identifier(18);
        let trace_id = random_identifier(12);
        let now_ms = unix_now_ms();
        if self.database_url.is_some() {
            self.with_postgres(|client| {
                client.execute("INSERT INTO identity_audit_events (event_id, account_id, event_type, outcome, reason_code, session_id, credential_id, peer_fingerprint, user_agent_summary, trace_id, details_json, occurred_at_ms) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)", &[&event_id, &account_id, &event_type, &outcome, &reason_code, &session_id, &credential_id, &peer_fingerprint, &sanitize_summary(user_agent, 160), &trace_id, &details, &(now_ms as i64)]).map_err(|error| format!("identity audit insert failed: {error}"))?;
                Ok(())
            })
        } else {
            let mut memory = self
                .memory
                .lock()
                .map_err(|_| "identity memory store lock poisoned".to_string())?;
            memory.audit_events.push(json!({"eventId": event_id, "accountId": account_id, "eventType": event_type, "outcome": outcome, "reasonCode": reason_code, "sessionId": session_id, "credentialId": credential_id, "traceId": trace_id, "details": details, "occurredAtMs": now_ms}));
            if memory.audit_events.len() > 1_000 {
                let remove = memory.audit_events.len() - 1_000;
                memory.audit_events.drain(0..remove);
            }
            Ok(())
        }
    }

    fn with_postgres<T: Send>(
        &self,
        action: impl FnOnce(&mut Client) -> Result<T, String> + Send,
    ) -> Result<T, String> {
        let url = self
            .database_url
            .as_deref()
            .ok_or_else(|| "identity Postgres backend is not configured".to_string())?;
        run_on_dedicated_thread("identity Postgres", move || {
            mir2_simulation::with_account_store_postgres_client(url, action)
        })
    }
}

/// `postgres` is synchronous and drives its own Tokio runtime. `block_in_place`
/// permits blocking work but does not leave the surrounding runtime context,
/// so calling the client there can panic while opening or driving a connection.
/// A scoped OS thread both leaves that context and lets database actions borrow
/// request data without forcing every identity method to clone its inputs.
fn run_on_dedicated_thread<T: Send>(
    operation: &'static str,
    action: impl FnOnce() -> Result<T, String> + Send,
) -> Result<T, String> {
    std::thread::scope(|scope| {
        scope
            .spawn(action)
            .join()
            .map_err(|_| format!("{operation} worker thread panicked"))?
    })
}

trait Pipe: Sized {
    fn pipe<T>(self, action: impl FnOnce(Self) -> T) -> T {
        action(self)
    }
}
impl<T> Pipe for T {}

fn secret_from_env(name: &str, local_fallback: &str, production: bool) -> Result<String, String> {
    match env::var(name).ok().map(|value| value.trim().to_string()).filter(|value| value.len() >= 32) {
        Some(secret) => Ok(secret),
        None if production => Err(format!("{name} with at least 32 characters is required in production")),
        None if env_flag("MIR2_ALLOW_DEV_IDENTITY_SECRETS") => Ok(local_fallback.to_string()),
        None => Err(format!("{name} is not set; set it, or explicitly set MIR2_ALLOW_DEV_IDENTITY_SECRETS=1 for local development")),
    }
}

fn production_like_env() -> bool {
    ["MIR2_RUNTIME_ENV", "MIR2_DEPLOYMENT_ENV", "MIR2_ENV"]
        .into_iter()
        .filter_map(|name| env::var(name).ok())
        .any(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "production" | "prod" | "staging"
            )
        })
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}
fn session_ttl_ms() -> u64 {
    env::var("MIR2_IDENTITY_SESSION_TTL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1_000))
        .unwrap_or(DEFAULT_SESSION_TTL_MS)
        .clamp(5 * 60 * 1_000, MAX_SESSION_TTL_MS)
}
fn canonical_auth_method(value: &str) -> Result<&'static str, String> {
    match value {
        "password" => Ok("password"),
        "passkey" | "sui_passkey" => Ok("sui_passkey"),
        "wallet" | "sui_wallet" => Ok("sui_wallet"),
        "recovery" => Ok("recovery"),
        _ => Err("unsupported identity authentication method".to_string()),
    }
}
fn credential_kind(value: &str) -> Result<&'static str, String> {
    match canonical_auth_method(value)? {
        "password" | "recovery" => Ok("password"),
        "sui_passkey" => Ok("sui_passkey"),
        "sui_wallet" => Ok("sui_wallet"),
        _ => unreachable!(),
    }
}
fn default_credential_name(kind: &str) -> String {
    match kind {
        "password" => "Password".to_string(),
        "sui_passkey" => "Sui Passkey".to_string(),
        "sui_wallet" => "Sui Wallet".to_string(),
        _ => "Credential".to_string(),
    }
}
fn validate_account_id(account_id: &str) -> Result<(), String> {
    if account_id.is_empty() || account_id.len() > 160 || account_id.chars().any(char::is_control) {
        Err("invalid account id".to_string())
    } else {
        Ok(())
    }
}
fn sanitize_summary(value: &str, max: usize) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_control())
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}
fn redact_credential_subject(subject: &str) -> String {
    let characters = subject.chars().collect::<Vec<_>>();
    if characters.len() <= 12 {
        "••••".to_string()
    } else {
        format!(
            "{}…{}",
            characters[..8].iter().collect::<String>(),
            characters[characters.len() - 4..]
                .iter()
                .collect::<String>()
        )
    }
}
fn i64_to_u64(value: i64) -> u64 {
    value.max(0) as u64
}
fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
fn random_identifier(bytes: usize) -> String {
    let mut value = vec![0u8; bytes];
    OsRng.fill_bytes(&mut value);
    URL_SAFE_NO_PAD.encode(value)
}
fn random_recovery_code() -> String {
    const ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut random = [0u8; 20];
    OsRng.fill_bytes(&mut random);
    let raw = random
        .into_iter()
        .map(|byte| ALPHABET[byte as usize % ALPHABET.len()] as char)
        .collect::<String>();
    format!(
        "{}-{}-{}-{}",
        &raw[0..5],
        &raw[5..10],
        &raw[10..15],
        &raw[15..20]
    )
}

#[cfg(test)]
mod tests {
    use super::{run_on_dedicated_thread, IdentityService};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn postgres_worker_leaves_the_callers_tokio_runtime_context() {
        let value = tokio::task::block_in_place(|| {
            run_on_dedicated_thread("test sync client", || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| error.to_string())?;
                Ok(runtime.block_on(async { 17_u8 }))
            })
        })
        .expect("dedicated worker must allow a synchronous client to drive its own runtime");

        assert_eq!(value, 17);
    }

    #[test]
    fn identity_sessions_are_signed_listed_and_revocable() {
        let service = IdentityService::local_for_tests();
        let grant = service
            .issue_session("alice", "password", "alice", "127.0.0.1", "test-browser")
            .expect("session should issue");
        let verified = service
            .verify_session_token(&grant.token)
            .expect("token should verify");
        let sessions = service
            .list_sessions(&verified)
            .expect("sessions should list");
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].current);
        assert!(service
            .revoke_session(&verified, &verified.session_id, "test logout")
            .expect("revoke should work"));
        assert!(service.verify_session_token(&grant.token).is_err());
    }

    #[test]
    fn recovery_codes_are_one_time_and_rotated() {
        let service = IdentityService::local_for_tests();
        let grant = service
            .issue_session("alice", "password", "alice", "127.0.0.1", "test-browser")
            .expect("session should issue");
        let verified = service
            .verify_session_token(&grant.token)
            .expect("token should verify");
        let codes = service
            .generate_recovery_codes(&verified)
            .expect("codes should generate");
        assert_eq!(codes.len(), 10);
        assert!(service
            .consume_recovery_code("alice", &codes[0])
            .expect("first use should work"));
        assert!(!service
            .consume_recovery_code("alice", &codes[0])
            .expect("second use should be rejected"));
    }

    #[test]
    fn sui_credential_resolves_owner_and_rejects_cross_account_binding() {
        let service = IdentityService::local_for_tests();
        let alice = service
            .issue_session("alice", "password", "alice", "127.0.0.1", "test-browser")
            .expect("alice session should issue");
        let alice = service
            .verify_session_token(&alice.token)
            .expect("alice session should verify");
        service
            .bind_sui_credential(&alice, "sui_wallet", "0x1234")
            .expect("wallet should bind to alice");
        assert_eq!(
            service
                .resolve_sui_account("sui_wallet", "0x1234")
                .expect("wallet should resolve"),
            Some("alice".to_string())
        );

        let bob = service
            .issue_session("bob", "password", "bob", "127.0.0.2", "test-browser")
            .expect("bob session should issue");
        let bob = service
            .verify_session_token(&bob.token)
            .expect("bob session should verify");
        let error = service
            .bind_sui_credential(&bob, "sui_wallet", "0x1234")
            .expect_err("the same wallet cannot bind to two accounts");
        assert!(error.contains("another account"));
    }
}
