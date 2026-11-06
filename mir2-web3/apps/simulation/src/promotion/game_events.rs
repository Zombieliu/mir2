//! Durable gameplay adapter. Source records are written through the caller's
//! authoritative character-save transaction, never from client telemetry.
//!
//! Activity retains the UTC day of its authority-issued event timestamp, even
//! when a save or worker crosses midnight. Historical snapshots never
//! manufacture old quest completions, level times or monster kills.

use postgres::Transaction;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{PromotionStore, TrustedGameFact};
use crate::{with_account_store_postgres_client, AccountStoreDatabaseMode, CharacterSaveRecord};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapturedGameplayFact {
    pub source: String,
    pub kind: String,
    pub occurred_at_ms: u64,
    pub level: Option<u16>,
    pub receipt_key: Option<String>,
    pub receipt_sha256: Option<String>,
    pub quest_id: Option<i32>,
}
impl CapturedGameplayFact {
    pub fn combat_kill(occurred_at_ms: u64, receipt_key: &str, receipt_sha256: &str) -> Self {
        Self {
            source: "combat_commit".into(),
            kind: "combatKill".into(),
            occurred_at_ms,
            level: None,
            receipt_key: Some(receipt_key.into()),
            receipt_sha256: Some(receipt_sha256.into()),
            quest_id: None,
        }
    }
    pub fn quest_completed(occurred_at_ms: u64, quest_id: i32) -> Self {
        Self {
            source: "quest_commit".into(),
            kind: "questCompleted".into(),
            occurred_at_ms,
            level: None,
            receipt_key: None,
            receipt_sha256: None,
            quest_id: Some(quest_id),
        }
    }
    pub fn level_reached(occurred_at_ms: u64, level: u16) -> Self {
        Self {
            source: "level_commit".into(),
            kind: "levelCommitted".into(),
            occurred_at_ms,
            level: Some(level),
            receipt_key: None,
            receipt_sha256: None,
            quest_id: None,
        }
    }
    pub(crate) fn journal_key(&self) -> Result<String, String> {
        if self.occurred_at_ms == 0 {
            return Err("promotion source event time invalid".into());
        }
        match (self.source.as_str(), self.kind.as_str()) {
            ("level_commit", "levelCommitted")
                if self.level.is_some_and(|v| v > 0)
                    && self.receipt_key.is_none()
                    && self.receipt_sha256.is_none()
                    && self.quest_id.is_none() =>
            {
                Ok(format!("level:{}", self.level.unwrap()))
            }
            ("combat_commit", "combatKill")
                if self.level.is_none()
                    && self.quest_id.is_none()
                    && self
                        .receipt_key
                        .as_ref()
                        .is_some_and(|v| !v.is_empty() && v.len() <= 4096)
                    && self.receipt_sha256.as_ref().is_some_and(|v| {
                        v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())
                    }) =>
            {
                Ok(format!("combat:{}", self.occurred_at_ms / 86_400_000))
            }
            ("quest_commit", "questCompleted")
                if self.level.is_none()
                    && self.quest_id.is_some()
                    && self.receipt_key.is_none()
                    && self.receipt_sha256.is_none() =>
            {
                Ok(format!("quest:{}", self.occurred_at_ms / 86_400_000))
            }
            _ => Err("promotion source event encoding invalid".into()),
        }
    }
}

pub(crate) struct GameSourceCapture {
    environment: String,
    before: Option<CharacterSaveRecord>,
}

/// Capture under the same row lock used by the subsequent CAS update. Disabled
/// configuration and Mirror mode do not inspect or produce promotion sources.
pub(crate) fn capture_character_source(
    tx: &mut Transaction<'_>,
    account_id: &str,
    character_index: i32,
    mode: AccountStoreDatabaseMode,
) -> Result<Option<GameSourceCapture>, String> {
    if mode != AccountStoreDatabaseMode::SourceOfTruth {
        return Ok(None);
    }
    let environment = match std::env::var("MIR2_PROMOTION_ENVIRONMENT") {
        Ok(value) if matches!(value.as_str(), "development" | "test" | "production") => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        _ => return Err("promotion game source environment is invalid".into()),
    };
    let before = tx
        .query_opt(
            "SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=$2 FOR UPDATE",
            &[&account_id, &character_index],
        )
        .map_err(|error| format!("promotion source checkpoint read failed: {error}"))?
        .map(|row| serde_json::from_value::<CharacterSaveRecord>(row.get(0)))
        .transpose()
        .map_err(|error| format!("promotion source checkpoint decode failed: {error}"))?;
    Ok(Some(GameSourceCapture {
        environment,
        before,
    }))
}

/// Invoke only after the character CAS succeeds, before the caller COMMITs.
/// Failure rolls back both the source checkpoint and every generated fact.
pub(crate) fn append_character_source(
    tx: &mut Transaction<'_>,
    captured: Option<GameSourceCapture>,
    account_id: &str,
    character_index: i32,
    after: &CharacterSaveRecord,
    save_version: i64,
    committed_at_ms: u64,
) -> Result<(), String> {
    let Some(captured) = captured else {
        return Ok(());
    };
    let revision =
        i64::try_from(after.revision).map_err(|_| "promotion source revision exhausted")?;
    let timestamp =
        i64::try_from(committed_at_ms).map_err(|_| "promotion source clock exhausted")?;
    for fact in derive_character_game_facts(
        account_id,
        character_index,
        captured.before.as_ref(),
        after,
        save_version,
        committed_at_ms,
    )? {
        let value = serde_json::to_value(&fact).map_err(|error| error.to_string())?;
        let inserted = tx.execute(
            "INSERT INTO promotion_game_source_events(environment,source_event_id,source,account_id,character_index,source_save_version,source_revision,committed_at_ms,fact_json) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT DO NOTHING",
            &[&captured.environment, &fact.source_event_id, &fact.source, &account_id, &character_index, &save_version, &revision, &timestamp, &value],
        ).map_err(|error| format!("promotion source append failed: {error}"))?;
        if inserted == 0 {
            let previous: Value = tx.query_one(
                "SELECT fact_json FROM promotion_game_source_events WHERE environment=$1 AND source_event_id=$2",
                &[&captured.environment, &fact.source_event_id],
            ).map_err(|error| format!("promotion source replay read failed: {error}"))?.get(0);
            if previous != value {
                return Err("promotion source event payload conflict".into());
            }
        }
    }
    Ok(())
}

fn event_id(value: &impl Serialize) -> Result<String, String> {
    let digest = Sha256::digest(serde_json::to_vec(value).map_err(|error| error.to_string())?);
    Ok(format!(
        "game-source-v1-{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

/// Diff authority-issued private journal entries. Snapshot level, quest stage,
/// XP balance and old kill receipts alone supply no occurrence timestamp and
/// never become promotion facts.
pub fn derive_character_game_facts(
    account_id: &str,
    character_index: i32,
    before: Option<&CharacterSaveRecord>,
    after: &CharacterSaveRecord,
    save_version: i64,
    committed_at_ms: u64,
) -> Result<Vec<TrustedGameFact>, String> {
    if account_id.is_empty()
        || after.character.index != character_index
        || save_version <= 0
        || committed_at_ms == 0
    {
        return Err("promotion source checkpoint identity invalid".into());
    }
    let mut facts = Vec::new();
    let Some(before) = before else {
        return Ok(facts);
    };
    if before.character.index != character_index || before.character.name != after.character.name {
        return Ok(facts);
    }
    for (key, captured) in &after.guild_experience_journal.promotion_game_facts {
        if captured.journal_key()? != *key || captured.occurred_at_ms > committed_at_ms {
            return Err("promotion source event time or key invalid".into());
        }
        match before
            .guild_experience_journal
            .promotion_game_facts
            .get(key)
        {
            Some(previous) if previous != captured => {
                return Err("promotion source event payload changed".into())
            }
            Some(_) => continue,
            None => {}
        }
        if let (Some(receipt), Some(hash)) = (&captured.receipt_key, &captured.receipt_sha256) {
            if after
                .guild_experience_journal
                .applied_kill_receipts
                .get(receipt)
                != Some(hash)
            {
                return Err("promotion source kill lacks its committed receipt".into());
            }
        }
        facts.push(TrustedGameFact {
            source: captured.source.clone(),
            kind: captured.kind.clone(),
            source_event_id: event_id(&(
                account_id,
                character_index,
                &after.character.name,
                key,
                captured,
            ))?,
            account_id: account_id.into(),
            character_index,
            occurred_at_ms: captured.occurred_at_ms,
            level: captured.level,
            evidence_ref: format!("character-save:{account_id}:{character_index}:{save_version}"),
        });
    }
    Ok(facts)
}

/// At-least-once delivery from immutable committed evidence. No process-wide
/// cursor can skip an uncommitted transaction. Concurrent workers may replay a
/// fact; its immutable source ID and exact payload make that harmless.
pub fn drain_committed_game_events(
    database_url: &str,
    environment: &str,
    limit: u32,
    now_ms: u64,
) -> Result<Value, String> {
    if !(1..=100).contains(&limit) || now_ms == 0 {
        return Err("promotionGameSourceWorkerArgumentsInvalid".into());
    }
    let store = PromotionStore::new(database_url, environment)?;
    let rows = with_account_store_postgres_client(database_url, |client| {
        client.query(
            "SELECT s.source_event_id,s.fact_json FROM promotion_game_source_events s LEFT JOIN promotion_game_source_deliveries d USING(environment,source_event_id) WHERE s.environment=$1 AND d.source_event_id IS NULL ORDER BY s.committed_at_ms,s.source_event_id LIMIT $2",
            &[&environment, &i64::from(limit)],
        ).map_err(|error| format!("promotion source backlog query failed: {error}"))
    })?;
    let mut delivered = 0u64;
    let mut failed = Vec::new();
    for row in &rows {
        let id: String = row.get(0);
        let fact = serde_json::from_value::<TrustedGameFact>(row.get(1))
            .map_err(|_| "promotionGameSourceCorrupt")?;
        match store.record_committed_game_fact(&fact, now_ms) {
            Ok(_) => {
                let newly_acknowledged = with_account_store_postgres_client(database_url, |client| {
                    let timestamp = i64::try_from(now_ms).map_err(|_| "promotionGameSourceClockInvalid")?;
                    client.execute("INSERT INTO promotion_game_source_deliveries(environment,source_event_id,delivered_at_ms) VALUES($1,$2,$3) ON CONFLICT DO NOTHING", &[&environment, &id, &timestamp]).map_err(|error| format!("promotion source delivery receipt failed: {error}"))
                })?;
                delivered += newly_acknowledged;
            }
            Err(error) => failed.push(json!({"sourceEventId":id,"errorCode":if error.starts_with("promotion") && error.bytes().all(|byte| byte.is_ascii_alphanumeric()) {error.as_str()} else {"promotionGameSourceDeliveryFailed"}})),
        }
    }
    Ok(
        json!({"selected": rows.len(), "delivered": delivered, "failures": failed, "timestampBasis":"authoritative_event"}),
    )
}

#[cfg(test)]
#[path = "game_events_tests.rs"]
mod tests;
