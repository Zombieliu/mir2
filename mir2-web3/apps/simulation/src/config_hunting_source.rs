//! Trusted one-time quest source, complete save CAS and bound wallet in one PG
//! transaction. Nothing here deserializes a client-provided reward capability.
use super::*;
use mir2_economy_service::{
    self as economy, BoundCreditReceipt, EconomyError, Idempotent, WalletOwner,
};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone)]
pub(crate) struct HuntingQuestCommit {
    pub checkpoint: CharacterSaveRecord,
    pub credit: Idempotent<BoundCreditReceipt>,
}

#[derive(Debug)]
pub(crate) enum HuntingSourceError {
    Rejected(String),
    OutcomeUnknown(String),
    PostCommit {
        commit: Box<HuntingQuestCommit>,
        detail: String,
    },
    /// The immutable economic receipt is known, but no trustworthy current
    /// personal projection is available. Never substitute the old source image.
    KnownCommitted {
        credit: Idempotent<BoundCreditReceipt>,
        detail: String,
    },
}
impl From<String> for HuntingSourceError {
    fn from(value: String) -> Self {
        Self::Rejected(value)
    }
}
impl fmt::Display for HuntingSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(detail) | Self::OutcomeUnknown(detail) => f.write_str(detail),
            Self::PostCommit { detail, .. } | Self::KnownCommitted { detail, .. } => {
                write!(f, "hunting source committed; {detail}")
            }
        }
    }
}
impl std::error::Error for HuntingSourceError {}

/// Opaque, non-Serde capability constructed only after trusted runtime planning.
pub(crate) struct PreparedHuntingQuestSource {
    binding: SharedAccountStore,
    database_url: String,
    realm: String,
    account: String,
    expected_incarnation: String,
    index: i32,
    quest_id: i32,
    source_digest: String,
    amount: u64,
    before: CharacterSaveRecord,
    original: AccountStore,
    staged: AccountStore,
    plan: AccountStoreMutationPlan,
}
impl fmt::Debug for PreparedHuntingQuestSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedHuntingQuestSource")
            .field("quest_id", &self.quest_id)
            .field("character_index", &self.index)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub(super) struct HuntingSourceFence {
    realm: String,
    database_binding: String,
    owner: WalletOwner,
    quest_id: i32,
    source_digest: String,
    amount: u64,
    guilds: Vec<String>,
    reason: String,
    epoch: u64,
}

struct LockedImage {
    owner: WalletOwner,
    account: AccountRecord,
    versions: AccountStoreSourceVersions,
    guilds: BTreeMap<String, Option<SharedGuildRecord>>,
    original_checkpoint: Option<CharacterSaveRecord>,
}

fn source_key(quest_id: i32) -> String {
    format!("quest-once-v1:{quest_id}")
}
fn database_binding(url: &str) -> String {
    Sha256::digest(url.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn validate_token(value: &str, max: usize) -> Result<(), String> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > max
        || value.chars().any(char::is_control)
    {
        return Err("invalid hunting source identity".into());
    }
    Ok(())
}
fn canonical_hash(value: &Value) -> Result<String, String> {
    fn sorted(value: Value) -> Value {
        match value {
            Value::Object(fields) => {
                let mut entries = fields.into_iter().collect::<Vec<_>>();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                Value::Object(
                    entries
                        .into_iter()
                        .map(|(key, value)| (key, sorted(value)))
                        .collect(),
                )
            }
            Value::Array(values) => Value::Array(values.into_iter().map(sorted).collect()),
            other => other,
        }
    }
    let bytes = serde_json::to_vec(&sorted(value.clone()))
        .map_err(|_| "hunting checkpoint encoding failed")?;
    Ok(format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}
fn quest_stage(save: &CharacterSaveRecord, quest: i32) -> Result<Option<QuestStage>, String> {
    #[derive(Deserialize)]
    struct Quest {
        quest_id: i32,
        stage: QuestStage,
    }
    let mut found = None;
    for encoded in &save.quest_states_json {
        let state: Quest = serde_json::from_str(encoded)
            .map_err(|_| "hunting source quest checkpoint is invalid")?;
        if state.quest_id == quest {
            if found.is_some() {
                return Err("duplicate hunting quest checkpoint".into());
            }
            found = Some(state.stage);
        }
    }
    Ok(found)
}
fn source_db(_: postgres::Error) -> EconomyError {
    EconomyError::SourceRejected
}

fn lock_and_load(
    tx: &mut Transaction<'_>,
    account: &str,
    index: i32,
    expected: Option<&CharacterRecord>,
    guilds: impl Iterator<Item = String>,
    allow_missing: bool,
) -> Result<Option<LockedImage>, EconomyError> {
    // Caller has already taken realm and any Guild locks. UPDATE immediately;
    // never take SHARE then upgrade it when the complete checkpoint is written.
    let row = tx.query_opt("SELECT raw_json,store_version,is_banned,ban_until_ms FROM accounts WHERE account_id=$1 FOR UPDATE", &[&account]).map_err(source_db)?;
    let Some(row) = row else {
        return if allow_missing {
            Ok(None)
        } else {
            Err(EconomyError::AccountUnavailable)
        };
    };
    let now: i64 = tx
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .map_err(source_db)?
        .get(0);
    let banned: bool = row.get("is_banned");
    let until: Option<i64> = row.get("ban_until_ms");
    if banned && until.is_none_or(|until| until > now) && !allow_missing {
        return Err(EconomyError::AccountUnavailable);
    }
    let character = tx.query_opt("SELECT character_name,hunting_incarnation::text AS incarnation FROM characters WHERE account_id=$1 AND character_index=$2 FOR UPDATE", &[&account,&index]).map_err(source_db)?;
    if character.is_none() && !allow_missing {
        return Err(EconomyError::CharacterNotFound);
    }
    if expected.is_some_and(|expected| {
        character
            .as_ref()
            .is_none_or(|character| expected.name != character.get::<_, String>("character_name"))
    }) {
        return Err(EconomyError::CharacterNotFound);
    }
    let account_image: AccountRecord = serde_json::from_value(row.get("raw_json"))
        .map_err(|_| EconomyError::CorruptHistory("account_checkpoint"))?;
    let mut versions = AccountStoreSourceVersions::default();
    versions
        .accounts
        .insert(account.into(), row.get("store_version"));
    let saves = tx.query("SELECT character_index,save_version FROM character_saves WHERE account_id=$1 ORDER BY character_index FOR UPDATE", &[&account]).map_err(source_db)?;
    versions.saves.insert(
        account.into(),
        saves
            .into_iter()
            .map(|row| (row.get(0), row.get(1)))
            .collect(),
    );
    let mut guild_images = BTreeMap::new();
    for id in guilds {
        let row = tx
            .query_opt(
                "SELECT raw_json,store_version FROM shared_guilds WHERE guild_id=$1",
                &[&id],
            )
            .map_err(source_db)?;
        let image = if let Some(row) = row {
            versions.guilds.insert(id.clone(), row.get("store_version"));
            Some(
                serde_json::from_value(row.get("raw_json"))
                    .map_err(|_| EconomyError::CorruptHistory("guild_checkpoint"))?,
            )
        } else {
            None
        };
        guild_images.insert(id, image);
    }
    Ok(Some(LockedImage {
        owner: WalletOwner {
            account_id: account.into(),
            character_index: index,
            incarnation: character
                .as_ref()
                .map(|character| {
                    format!(
                        "pg-character-v1:{}",
                        character.get::<_, String>("incarnation")
                    )
                })
                .unwrap_or_default(),
        },
        account: account_image,
        versions,
        guilds: guild_images,
        original_checkpoint: None,
    }))
}

fn load_original_checkpoint(
    tx: &mut Transaction<'_>,
    realm: &str,
    owner: &WalletOwner,
    quest_id: i32,
    digest: &str,
    amount: u64,
) -> Result<Option<CharacterSaveRecord>, EconomyError> {
    let row = tx.query_opt("SELECT source_key,source_digest,bound_gold,before_revision,after_revision,checkpoint_json,checkpoint_digest,operation_key FROM hunting_quest_source_checkpoints WHERE realm_id=$1 AND account_id=$2 AND character_index=$3 AND character_incarnation=$4 AND quest_id=$5", &[&realm,&owner.account_id,&owner.character_index,&owner.incarnation,&quest_id]).map_err(source_db)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let (operation_key, _) = economy::bound_credit_operation_identity(
        realm,
        owner,
        &source_key(quest_id),
        digest,
        amount,
    )?;
    if row.get::<_, String>("source_key") != source_key(quest_id)
        || row.get::<_, String>("source_digest") != digest
        || row.get::<_, i64>("bound_gold")
            != i64::try_from(amount).map_err(|_| EconomyError::Overflow("credit_amount"))?
        || row.get::<_, String>("operation_key") != operation_key
    {
        return Err(EconomyError::RequestConflict);
    }
    let data: Value = row.get("checkpoint_json");
    if canonical_hash(&data).map_err(|_| EconomyError::CorruptHistory("source_checkpoint"))?
        != row.get::<_, String>("checkpoint_digest")
    {
        return Err(EconomyError::CorruptHistory("source_checkpoint_digest"));
    }
    let checkpoint: CharacterSaveRecord = serde_json::from_value(data)
        .map_err(|_| EconomyError::CorruptHistory("source_checkpoint"))?;
    if checkpoint.character.index != owner.character_index
        || i64::try_from(checkpoint.revision).ok() != Some(row.get("after_revision"))
        || row.get::<_, i64>("after_revision").checked_sub(1) != Some(row.get("before_revision"))
        || quest_stage(&checkpoint, quest_id).ok() != Some(Some(QuestStage::Completed))
    {
        return Err(EconomyError::CorruptHistory("source_checkpoint_identity"));
    }
    Ok(Some(checkpoint))
}

impl PreparedHuntingQuestSource {
    fn checkpoint(&self) -> &CharacterSaveRecord {
        &self.staged.accounts[&self.account].saves[&self.index]
    }
}

impl SimulationConfig {
    /// Until source outcomes and their original incarnation witnesses have a
    /// resolved replication carrier, an RPC/replicated host must reject this
    /// configuration BEFORE executing active player commands. Clearing the
    /// realm on a standby and replaying the command would mint legacy gold.
    pub fn ensure_hunting_source_replication_supported(&self) -> Result<(), String> {
        if self.hunting_economy_realm.is_some() {
            return Err("hunting quest sources do not support raw command replication or promotion; use an unreplicated in-process game runtime".into());
        }
        Ok(())
    }

    pub(crate) fn freeze_hunting_quest_publication(&self, detail: String) -> String {
        self.freeze_account_store_writes(detail)
    }

    fn fence_hunting_source_operation(
        &self,
        source: &PreparedHuntingQuestSource,
        detail: String,
    ) -> String {
        let (reason, epoch, newly_frozen) = self.freeze_account_store_writes_with_epoch(detail);
        if !newly_frozen {
            // Another publication failure owns the existing freeze. Do not
            // turn its reason and a fresh epoch into this source's capability.
            return reason;
        }
        if let Ok(mut fence) = self.hunting_source_fence.lock() {
            *fence = Some(HuntingSourceFence {
                realm: source.realm.clone(),
                database_binding: database_binding(&source.database_url),
                owner: WalletOwner {
                    account_id: source.account.clone(),
                    character_index: source.index,
                    incarnation: source.expected_incarnation.clone(),
                },
                quest_id: source.quest_id,
                source_digest: source.source_digest.clone(),
                amount: source.amount,
                guilds: source.plan.guilds.keys().cloned().collect(),
                reason: reason.clone(),
                epoch,
            });
        }
        reason
    }

    pub(crate) fn bind_hunting_character_incarnation(
        &self,
        account: &str,
        save: &CharacterSaveRecord,
    ) -> Result<Option<String>, String> {
        if self.hunting_economy_realm.is_none() {
            return Ok(None);
        }
        self.ensure_account_store_writable()?;
        let (realm, url) = self.hunting_source_binding()?;
        validate_token(account, 256)?;
        with_account_store_postgres_client(url, |client| {
            let mut tx = client
                .transaction()
                .map_err(|_| "hunting identity transaction unavailable")?;
            tx.query_one("SELECT set_config('lock_timeout','2000ms',true),set_config('statement_timeout','10000ms',true)",&[]).map_err(|_| "hunting identity timeouts unavailable")?;
            economy::lock_realm_and_read_database_clock(&mut tx, realm)
                .map_err(|_| "hunting identity realm unavailable")?;
            let image = lock_and_load(
                &mut tx,
                account,
                save.character.index,
                Some(&save.character),
                std::iter::empty(),
                false,
            )
            .map_err(|_| "hunting identity account or character unavailable")?
            .ok_or("hunting identity character unavailable")?;
            let checkpoint = image
                .account
                .saves
                .get(&save.character.index)
                .ok_or("hunting identity canonical checkpoint missing")?;
            let row=tx.query_opt("SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=$2", &[&account,&save.character.index]).map_err(|_| "hunting identity canonical checkpoint unavailable")?.ok_or("hunting identity canonical checkpoint missing")?;
            if to_json(checkpoint)? != to_json(save)? || row.get::<_, Value>(0) != to_json(save)? {
                return Err(
                    "hunting StartGame checkpoint changed before incarnation binding".into(),
                );
            }
            tx.commit()
                .map_err(|_| "hunting identity binding confirmation unavailable")?;
            Ok(Some(image.owner.incarnation))
        })
    }

    pub fn with_hunting_economy_realm(mut self, realm: impl Into<String>) -> Result<Self, String> {
        let realm = realm.into();
        validate_token(&realm, 128)?;
        if self.account_store_database_url.is_none()
            || self.account_store_database_mode != AccountStoreDatabaseMode::SourceOfTruth
            || self.account_store_path.is_some()
        {
            return Err(
                "hunting quest sources require PostgreSQL SourceOfTruth without a File primary"
                    .into(),
            );
        }
        self.hunting_economy_realm = Some(realm);
        Ok(self)
    }
    fn hunting_source_binding(&self) -> Result<(&str, &str), String> {
        if self.account_store_database_mode != AccountStoreDatabaseMode::SourceOfTruth
            || self.account_store_path.is_some()
        {
            return Err("hunting source requires PostgreSQL SourceOfTruth".into());
        }
        let realm = self
            .hunting_economy_realm
            .as_deref()
            .ok_or("hunting quest sources are disabled")?;
        validate_token(realm, 128)?;
        let url = self
            .account_store_database_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .ok_or("hunting source PostgreSQL is unavailable")?;
        Ok((realm, url))
    }

    pub(crate) fn prepare_hunting_quest_source<F>(
        &self,
        account: &str,
        expected_incarnation: &str,
        quest_id: i32,
        source_digest: &str,
        amount: u64,
        runtime_before: CharacterSaveRecord,
        stage: F,
    ) -> Result<PreparedHuntingQuestSource, String>
    where
        F: FnOnce(&mut AccountStore) -> Result<(), String>,
    {
        self.ensure_account_store_writable()?;
        let (realm, url) = self.hunting_source_binding()?;
        validate_token(account, 256)?;
        validate_token(source_digest, 256)?;
        validate_token(expected_incarnation, 256)?;
        if !expected_incarnation.starts_with("pg-character-v1:") {
            return Err("hunting source requires a StartGame incarnation witness".into());
        }
        if quest_id <= 0 || amount == 0 || i64::try_from(amount).is_err() {
            return Err("invalid hunting quest amount or identity".into());
        }
        let original = self
            .account_store
            .lock()
            .map_err(|_| "hunting source account image poisoned")?
            .clone();
        let index = runtime_before.character.index;
        let durable = original
            .accounts
            .get(account)
            .and_then(|a| a.saves.get(&index))
            .ok_or("hunting source durable checkpoint missing")?;
        if durable.revision != runtime_before.revision
            || durable.character.name != runtime_before.character.name
            || quest_stage(&runtime_before, quest_id)? != Some(QuestStage::ReadyToTurnIn)
        {
            return Err("hunting source revision, character or ready quest mismatch".into());
        }
        let mut staged = original.clone();
        stage(&mut staged)?;
        let accounts = [account.to_owned()];
        validate_account_store_transaction_scope(
            &original,
            &staged,
            AccountStoreMutationScope::Accounts(&accounts),
        )
        .map_err(|e| e.to_string())?;
        let guilds =
            guild_experience::settle_authorized_sources(&original, &mut staged, &accounts, None)?
                .into_iter()
                .collect::<Vec<_>>();
        let scope = AccountStoreMutationScope::AccountsWithGuilds {
            account_ids: &accounts,
            guild_ids: &guilds,
        };
        validate_account_store_transaction_scope(&original, &staged, scope)
            .map_err(|e| e.to_string())?;
        let after = staged
            .accounts
            .get(account)
            .and_then(|a| a.saves.get(&index))
            .ok_or("hunting source staged checkpoint missing")?;
        if after.revision
            != runtime_before
                .revision
                .checked_add(1)
                .ok_or("hunting source revision exhausted")?
            || i64::try_from(after.revision).is_err()
            || after.character.index != index
            || after.character.name != runtime_before.character.name
            || after.gold != runtime_before.gold
            || quest_stage(after, quest_id)? != Some(QuestStage::Completed)
        {
            return Err(
                "hunting source must CAS one completed quest without changing legacy gold".into(),
            );
        }
        let mut expected_account = original.accounts[account].clone();
        expected_account.saves.insert(index, after.clone());
        if let Some(character) = expected_account
            .characters
            .iter_mut()
            .find(|character| character.index == index)
        {
            *character = after.character.clone();
        }
        if to_json(&expected_account)? != to_json(&staged.accounts[account])? {
            return Err("hunting source changed unrelated account state".into());
        }
        let plan = build_account_store_mutation_plan(&original, &staged, scope, false);
        if plan.global_stock.is_some()
            || plan.clock.is_some()
            || !plan.heroes.is_empty()
            || plan.hero_allocator.is_some()
        {
            return Err("hunting source changed unrelated shared state".into());
        }
        serde_json::to_vec(&staged).map_err(|_| "hunting source checkpoint encoding failed")?;
        Ok(PreparedHuntingQuestSource {
            binding: self.account_store.clone(),
            database_url: url.into(),
            realm: realm.into(),
            account: account.into(),
            expected_incarnation: expected_incarnation.into(),
            index,
            quest_id,
            source_digest: source_digest.into(),
            amount,
            before: runtime_before,
            original,
            staged,
            plan,
        })
    }

    pub(crate) fn commit_hunting_quest_source<F>(
        &self,
        account: &str,
        expected_incarnation: &str,
        quest_id: i32,
        source_digest: &str,
        amount: u64,
        runtime_before: CharacterSaveRecord,
        stage: F,
    ) -> Result<HuntingQuestCommit, HuntingSourceError>
    where
        F: FnOnce(&mut AccountStore) -> Result<(), String>,
    {
        let source = self.prepare_hunting_quest_source(
            account,
            expected_incarnation,
            quest_id,
            source_digest,
            amount,
            runtime_before,
            stage,
        )?;
        self.publish_hunting_quest_source(&source)
    }

    pub(crate) fn publish_hunting_quest_source(
        &self,
        source: &PreparedHuntingQuestSource,
    ) -> Result<HuntingQuestCommit, HuntingSourceError> {
        self.publish_hunting_quest_source_inner(source, 0)
    }

    #[cfg(test)]
    pub(crate) fn publish_hunting_quest_source_with_test_fault(
        &self,
        source: &PreparedHuntingQuestSource,
        fault: u8,
    ) -> Result<HuntingQuestCommit, HuntingSourceError> {
        self.publish_hunting_quest_source_inner(source, fault)
    }

    fn publish_hunting_quest_source_inner(
        &self,
        source: &PreparedHuntingQuestSource,
        test_fault: u8,
    ) -> Result<HuntingQuestCommit, HuntingSourceError> {
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "hunting source persistence lock poisoned".to_string())?;
        self.ensure_account_store_writable()?;
        let (realm, url) = self.hunting_source_binding()?;
        if !Arc::ptr_eq(&self.account_store, &source.binding)
            || realm != source.realm
            || url != source.database_url
        {
            return Err("hunting source authority binding changed"
                .to_string()
                .into());
        }
        let mut live = self
            .account_store
            .lock()
            .map_err(|_| "hunting source account image poisoned".to_string())?;
        let mut local_matches = to_json(&live.accounts.get(&source.account))?
            == to_json(&source.original.accounts.get(&source.account))?;
        for id in source.plan.guilds.keys() {
            local_matches &= to_json(&live.shared_guilds.get(id))?
                == to_json(&source.original.shared_guilds.get(id))?;
        }
        let locked = RefCell::new(None::<LockedImage>);
        let written = RefCell::new(None::<AccountStoreSourceVersions>);
        let source_error = RefCell::new(None::<String>);
        let key = source_key(source.quest_id);
        let delivered = with_account_store_postgres_client(url, |client| {
            Ok(economy::credit_bound_authorized_with_source_with_timeouts(
                client,
                realm,
                &key,
                &source.source_digest,
                source.amount,
                2000,
                10000,
                |tx, _clock| {
                    #[cfg(test)]
                    if test_fault == 9 {
                        return Err(EconomyError::SourceRejected);
                    }
                    source.plan.lock_guild_plan(tx).map_err(|error| {
                        *source_error.borrow_mut() = Some(error);
                        EconomyError::SourceRejected
                    })?;
                    let mut image = lock_and_load(
                        tx,
                        &source.account,
                        source.index,
                        Some(&source.before.character),
                        source.plan.guilds.keys().cloned(),
                        false,
                    )?
                    .ok_or(EconomyError::CharacterNotFound)?;
                    if image.owner.incarnation != source.expected_incarnation {
                        return Err(EconomyError::CharacterNotFound);
                    }
                    image.original_checkpoint = load_original_checkpoint(
                        tx,
                        realm,
                        &image.owner,
                        source.quest_id,
                        &source.source_digest,
                        source.amount,
                    )?;
                    let owner = image.owner.clone();
                    *locked.borrow_mut() = Some(image);
                    Ok(owner)
                },
                |tx, clock| {
                    if !local_matches {
                        *source_error.borrow_mut() = Some(
                            "hunting source account or Guild changed before publication".into(),
                        );
                        return Err(EconomyError::SourceRejected);
                    }
                    let versions = write_account_store_mutation_plan_in_transaction(
                        tx,
                        &source.plan,
                        AccountStoreDatabaseMode::SourceOfTruth,
                    )
                    .map_err(|error| {
                        *source_error.borrow_mut() = Some(error);
                        EconomyError::SourceRejected
                    })?;
                    let image = locked.borrow();
                    let image = image.as_ref().ok_or(EconomyError::SourceRejected)?;
                    let (operation, _) = economy::bound_credit_operation_identity(
                        realm,
                        &image.owner,
                        &key,
                        &source.source_digest,
                        source.amount,
                    )?;
                    let checkpoint =
                        to_json(source.checkpoint()).map_err(|_| EconomyError::SourceRejected)?;
                    let hash =
                        canonical_hash(&checkpoint).map_err(|_| EconomyError::SourceRejected)?;
                    tx.execute("INSERT INTO hunting_quest_source_checkpoints(realm_id,account_id,character_index,character_incarnation,quest_id,source_key,source_digest,bound_gold,before_revision,after_revision,checkpoint_json,checkpoint_digest,operation_key,committed_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
                        &[&realm,&source.account,&source.index,&image.owner.incarnation,&source.quest_id,&key,&source.source_digest,&(source.amount as i64),&(source.before.revision as i64),&(source.checkpoint().revision as i64),&checkpoint,&hash,&operation,&clock.database_time_ms]).map_err(source_db)?;
                    *written.borrow_mut() = Some(versions);
                    #[cfg(test)]
                    if test_fault == 1 || test_fault == 4 {
                        return Err(EconomyError::SourceRejected);
                    }
                    Ok(())
                },
            ))
        });
        // The pool/worker boundary may fail to deliver a result after its
        // transaction committed. Only the original source lookup can decide.
        #[cfg(test)]
        let delivered = if (test_fault == 5
            && delivered.as_ref().is_ok_and(|outcome| outcome.is_ok()))
            || test_fault == 9
        {
            *locked.borrow_mut() = None;
            Err("injected worker result delivery failure".into())
        } else {
            delivered
        };
        let outcome = match delivered {
            Ok(outcome) => outcome,
            Err(_) => Err(EconomyError::OutcomeUnknown),
        };
        #[cfg(test)]
        let outcome = if test_fault == 10 && outcome.is_ok() {
            // Reproduce another runtime's publication failure after our writable
            // check/real COMMIT and before this source can fence its unknown result.
            self.freeze_hunting_quest_publication(
                "injected unrelated failure before source fencing".into(),
            );
            Err(EconomyError::OutcomeUnknown)
        } else if (test_fault == 2 && outcome.is_ok()) || test_fault == 4 {
            Err(EconomyError::OutcomeUnknown)
        } else {
            outcome
        };
        #[cfg(not(test))]
        let _ = test_fault;
        let credit = match outcome {
            Ok(receipt) => receipt,
            Err(EconomyError::OutcomeUnknown) => {
                // The StartGame-bound owner is sufficient even if the worker
                // never delivered its lock-qualified image (or never started).
                let reason = self.fence_hunting_source_operation(
                    source,
                    "hunting quest COMMIT outcome unknown; recover its original source".into(),
                );
                return Err(HuntingSourceError::OutcomeUnknown(reason));
            }
            Err(error) => {
                return Err(HuntingSourceError::Rejected(
                    source_error
                        .into_inner()
                        .unwrap_or_else(|| error.to_string()),
                ))
            }
        };
        #[cfg(test)]
        if test_fault == 6 {
            *locked.borrow_mut() = None;
        }
        let Some(mut image) = locked.into_inner() else {
            let detail = self.fence_hunting_source_operation(
                source,
                "hunting committed source image missing".into(),
            );
            return Err(HuntingSourceError::KnownCommitted { credit, detail });
        };
        let checkpoint = if credit.replayed {
            #[cfg(test)]
            if test_fault == 7 {
                image.account.saves.remove(&source.index);
            }
            if image.original_checkpoint.is_none() {
                let detail = self.fence_hunting_source_operation(
                    source,
                    "hunting receipt lacks original complete source checkpoint".into(),
                );
                return Err(HuntingSourceError::KnownCommitted { credit, detail });
            }
            let Some(checkpoint) = image.account.saves.get(&source.index).cloned() else {
                let detail = self.fence_hunting_source_operation(
                    source,
                    "hunting replay current checkpoint missing".into(),
                );
                return Err(HuntingSourceError::KnownCommitted { credit, detail });
            };
            checkpoint
        } else {
            source.checkpoint().clone()
        };
        let commit = HuntingQuestCommit { checkpoint, credit };
        #[cfg(test)]
        if test_fault == 3 {
            let detail = self.fence_hunting_source_operation(
                source,
                "injected failure after known hunting COMMIT".into(),
            );
            return Err(HuntingSourceError::PostCommit {
                commit: Box::new(commit),
                detail,
            });
        }
        let versions = if commit.credit.replayed {
            image.versions
        } else {
            let Some(versions) = written.into_inner() else {
                let detail = self.fence_hunting_source_operation(
                    source,
                    "hunting committed source versions missing".into(),
                );
                return Err(HuntingSourceError::PostCommit {
                    commit: Box::new(commit),
                    detail,
                });
            };
            image.account = source.staged.accounts[&source.account].clone();
            image.guilds = source
                .plan
                .guilds
                .keys()
                .map(|id| (id.clone(), source.staged.shared_guilds.get(id).cloned()))
                .collect();
            versions
        };
        live.accounts.insert(source.account.clone(), image.account);
        for (id, guild) in image.guilds {
            if let Some(guild) = guild {
                live.shared_guilds.insert(id, guild);
            } else {
                live.shared_guilds.remove(&id);
            }
        }
        apply_account_store_mutation_source_versions(&mut live, &source.plan, versions);
        self.guild_clock_updates.fetch_add(1, Ordering::AcqRel);
        Ok(commit)
    }

    /// Packet recovery uses only the digest and amount saved by the original
    /// trusted source. A later catalog edit must not redefine an uncertain
    /// operation. This probe never grants authority to release another fence.
    pub(crate) fn recover_pending_hunting_quest_source(
        &self,
        account: &str,
        expected_incarnation: &str,
        index: i32,
        quest_id: i32,
    ) -> Result<Option<HuntingQuestCommit>, HuntingSourceError> {
        let fence = self
            .hunting_source_fence
            .lock()
            .map_err(|_| "hunting pending source fence poisoned".to_string())?
            .clone();
        let Some(fence) = fence else {
            return Ok(None);
        };
        if self.hunting_economy_realm.as_deref() != Some(fence.realm.as_str())
            || self
                .account_store_database_url
                .as_deref()
                .is_none_or(|url| database_binding(url) != fence.database_binding)
            || fence.owner.account_id != account
            || fence.owner.incarnation != expected_incarnation
            || fence.owner.character_index != index
            || fence.quest_id != quest_id
        {
            return Ok(None);
        }
        let own_fence = {
            let state = self
                .account_store_write_state
                .lock()
                .map_err(|_| "hunting pending source write state poisoned".to_string())?;
            self.account_store_freeze_epoch.load(Ordering::Acquire) == fence.epoch
                && matches!(&*state,AccountStoreWriteState::Frozen{reason} if reason==&fence.reason)
        };
        if !own_fence {
            return Ok(None);
        }
        // recover rechecks the binding, actor, digest and epoch under its
        // persistence guard. No write-state/fence guard is retained here.
        self.recover_hunting_quest_source(
            account,
            expected_incarnation,
            index,
            quest_id,
            &fence.source_digest,
            fence.amount,
        )
    }

    /// Only the exact original uncertain source may release its own fence. A
    /// second freeze changes the epoch, so this cannot clear another subsystem.
    /// None means the realm lock proved that the original transaction did not
    /// commit; no source is executed and no reward is minted by recovery.
    pub(crate) fn recover_hunting_quest_source(
        &self,
        account: &str,
        expected_incarnation: &str,
        index: i32,
        quest_id: i32,
        digest: &str,
        amount: u64,
    ) -> Result<Option<HuntingQuestCommit>, HuntingSourceError> {
        let _persist = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "hunting recovery persistence lock poisoned".to_string())?;
        let (realm, url) = self.hunting_source_binding()?;
        let fence = self
            .hunting_source_fence
            .lock()
            .map_err(|_| "hunting recovery fence poisoned".to_string())?
            .clone();
        let Some(fence) = fence else {
            self.ensure_account_store_writable()?;
            return Ok(None);
        };
        if fence.realm != realm
            || fence.database_binding != database_binding(url)
            || fence.owner.account_id != account
            || fence.owner.incarnation != expected_incarnation
            || fence.owner.character_index != index
            || fence.quest_id != quest_id
            || fence.source_digest != digest
            || fence.amount != amount
        {
            return Err("hunting recovery does not match the original source"
                .to_string()
                .into());
        }
        let mut write_state = self
            .account_store_write_state
            .lock()
            .map_err(|_| "hunting recovery write state poisoned".to_string())?;
        // Everything called while this guard is held uses only the PG pool,
        // SQL and pure decoding. Do not call ensure_account_store_writable,
        // freeze_account_store_writes or the public refresh helper here: they
        // would acquire this same non-reentrant write-state mutex.
        if self.account_store_freeze_epoch.load(Ordering::Acquire) != fence.epoch
            || !matches!(&*write_state,AccountStoreWriteState::Frozen{reason} if reason==&fence.reason)
        {
            return Err(
                "hunting recovery cannot release a different publication fence"
                    .to_string()
                    .into(),
            );
        }
        let image = RefCell::new(None::<LockedImage>);
        let original = RefCell::new(None::<CharacterSaveRecord>);
        let result = with_account_store_postgres_client(url, |client| {
            Ok(economy::lookup_bound_credit_authorized_with_timeouts(
                client,
                realm,
                &source_key(quest_id),
                digest,
                amount,
                2000,
                10000,
                |tx, _clock| {
                    for id in &fence.guilds {
                        tx.query_one(
                            "SELECT pg_advisory_xact_lock(hashtextextended($1,735891))",
                            &[id],
                        )
                        .map_err(source_db)?;
                        tx.query_opt(
                            "SELECT guild_id FROM shared_guilds WHERE guild_id=$1 FOR UPDATE",
                            &[id],
                        )
                        .map_err(source_db)?;
                    }
                    let current = lock_and_load(
                        tx,
                        account,
                        index,
                        None,
                        fence.guilds.iter().cloned(),
                        true,
                    )?;
                    *original.borrow_mut() = load_original_checkpoint(
                        tx,
                        realm,
                        &fence.owner,
                        quest_id,
                        digest,
                        amount,
                    )?;
                    *image.borrow_mut() = current;
                    Ok(fence.owner.clone())
                },
            ))
        })
        .map_err(|_| {
            HuntingSourceError::OutcomeUnknown("hunting recovery repository unavailable".into())
        })?
        .map_err(|_| {
            HuntingSourceError::OutcomeUnknown(
                "hunting original recovery remains unresolved".into(),
            )
        })?;
        let current = image.into_inner();
        let commit = match result {
            Some(receipt) => {
                let credit = Idempotent {
                    receipt,
                    replayed: true,
                };
                if current
                    .as_ref()
                    .is_none_or(|image| image.owner != fence.owner)
                {
                    return Err(HuntingSourceError::KnownCommitted {
                        credit,
                        detail: "hunting committed owner incarnation is no longer active".into(),
                    });
                }
                if original.borrow().is_none() {
                    return Err(HuntingSourceError::KnownCommitted {
                        credit,
                        detail: "hunting receipt lacks original source checkpoint".into(),
                    });
                }
                let Some(checkpoint) = current
                    .as_ref()
                    .and_then(|image| image.account.saves.get(&index))
                    .cloned()
                else {
                    return Err(HuntingSourceError::KnownCommitted {
                        credit,
                        detail: "hunting committed owner checkpoint unavailable".into(),
                    });
                };
                Some(HuntingQuestCommit { checkpoint, credit })
            }
            None => {
                if original.borrow().is_some() {
                    return Err(HuntingSourceError::OutcomeUnknown(
                        "hunting source checkpoint exists without economic receipt".into(),
                    ));
                }
                None
            }
        };
        let publication_error = |detail: &str| match &commit {
            Some(commit) => HuntingSourceError::PostCommit {
                commit: Box::new(commit.clone()),
                detail: detail.into(),
            },
            None => HuntingSourceError::OutcomeUnknown(detail.into()),
        };
        let mut live = self
            .account_store
            .lock()
            .map_err(|_| publication_error("hunting recovery account image poisoned"))?;
        let mut fence_slot = self
            .hunting_source_fence
            .lock()
            .map_err(|_| publication_error("hunting recovery fence poisoned"))?;
        let mut staged = live.clone();
        staged.source_account_versions.remove(account);
        staged.source_save_versions.remove(account);
        if let Some(image) = current {
            staged.accounts.insert(account.into(), image.account);
            for (id, guild) in image.guilds {
                if let Some(guild) = guild {
                    staged.shared_guilds.insert(id, guild);
                } else {
                    staged.shared_guilds.remove(&id);
                    staged.source_guild_versions.remove(&id);
                }
            }
            staged.merge_source_versions(image.versions);
        } else {
            staged.accounts.remove(account);
        }
        // Compare again after the DB boundary; no broad unfreeze operation exists.
        if self.account_store_freeze_epoch.load(Ordering::Acquire) != fence.epoch {
            return Err(publication_error(
                "another publication fence superseded hunting recovery",
            ));
        }
        *live = staged;
        *write_state = AccountStoreWriteState::Writable;
        *fence_slot = None;
        self.guild_clock_updates.fetch_add(1, Ordering::AcqRel);
        Ok(commit)
    }
}

#[cfg(test)]
#[path = "config_hunting_source_tests.rs"]
mod tests;
