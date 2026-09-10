//! Opaque source participant for the Gateway's single PostgreSQL kill transaction.
//! Only authenticated simulation code can prepare it. No serde write capability.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedKillPublicationFailure {
    Rejected(String),
    OutcomeUnknown(String),
}
impl From<String> for PreparedKillPublicationFailure {
    fn from(error: String) -> Self {
        Self::Rejected(error)
    }
}

pub enum PreparedKillPublication<T> {
    Written(T),
    AlreadyCommitted(T),
}

pub struct PreparedKillAccountSource {
    binding: SharedAccountStore,
    database_url: String,
    identity: Stage5FriendIdentity,
    key: String,
    hash: String,
    runtime_before: CharacterSaveRecord,
    original: AccountStore,
    staged: AccountStore,
    plan: AccountStoreMutationPlan,
    mode: AccountStoreDatabaseMode,
    published: Mutex<Option<AccountStoreSourceVersions>>,
}
impl std::fmt::Debug for PreparedKillAccountSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedKillAccountSource")
            .field("identity", &self.identity)
            .field("key", &self.key)
            .field("before_revision", &self.runtime_before.revision)
            .field("after_revision", &self.checkpoint().revision)
            .finish_non_exhaustive()
    }
}
impl PreparedKillAccountSource {
    pub fn database_url(&self) -> &str {
        &self.database_url
    }
    pub fn account_id(&self) -> &str {
        &self.identity.account_id
    }
    pub fn character_index(&self) -> i32 {
        self.identity.character_index
    }
    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn award_hash(&self) -> &str {
        &self.hash
    }
    pub fn before(&self) -> &CharacterSaveRecord {
        &self.runtime_before
    }
    pub fn checkpoint(&self) -> &CharacterSaveRecord {
        &self.staged.accounts[self.account_id()].saves[&self.character_index()]
    }
    /// Take Guild locks only. Gateway takes economy-character advisory locks
    /// next; account row locks must not be taken before those advisory locks.
    pub fn lock_postgres_guilds(&self, tx: &mut Transaction<'_>) -> Result<(), String> {
        for id in self.plan.guilds.keys() {
            tx.query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1,735891))",
                &[id],
            )
            .map_err(|e| e.to_string())?;
            tx.query_opt(
                "SELECT guild_id FROM shared_guilds WHERE guild_id=$1 FOR UPDATE",
                &[id],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    /// Writes through an existing caller-owned transaction. Success here is
    /// provisional: the caller must not claim Written until COMMIT is confirmed.
    pub fn write_postgres(&self, tx: &mut Transaction<'_>) -> Result<CharacterSaveRecord, String> {
        let mut published = self
            .published
            .lock()
            .map_err(|_| "prepared source receipt poisoned")?;
        if published.is_some() {
            return Err("prepared source participant already invoked".into());
        }
        let mut plan = self.plan.clone();
        if self.mode == AccountStoreDatabaseMode::Mirror {
            // File images do not serialize database CAS versions. Bind fresh
            // versions only after proving PG still mirrors the complete source.
            for (id, mutation) in &mut plan.accounts {
                let row = tx.query_opt("SELECT raw_json,store_version FROM accounts WHERE account_id=$1 FOR UPDATE", &[id]).map_err(|e|e.to_string())?
                    .ok_or("prepared kill mirror account has not been initialized")?;
                if row.get::<_, serde_json::Value>(0) != to_json(&self.original.accounts[id])? {
                    return Err("prepared kill mirror account diverged from File authority".into());
                }
                mutation.expected_version = Some(row.get(1));
                for (index, save) in &mut mutation.saves {
                    let row = tx.query_opt("SELECT snapshot_json,save_version FROM character_saves WHERE account_id=$1 AND character_index=$2 FOR UPDATE", &[id,index]).map_err(|e|e.to_string())?
                        .ok_or("prepared kill mirror checkpoint missing")?;
                    if row.get::<_, serde_json::Value>(0)
                        != to_json(&self.original.accounts[id].saves[index])?
                    {
                        return Err("prepared kill mirror checkpoint diverged".into());
                    }
                    save.expected_version = Some(row.get(1));
                }
            }
            for (id, mutation) in &mut plan.guilds {
                let row = tx.query_opt("SELECT raw_json,store_version FROM shared_guilds WHERE guild_id=$1 FOR UPDATE", &[id]).map_err(|e|e.to_string())?;
                match (row, self.original.shared_guilds.get(id)) {
                    (Some(row), Some(original))
                        if row.get::<_, serde_json::Value>(0) == to_json(original)? =>
                    {
                        mutation.expected_version = Some(row.get(1))
                    }
                    (None, None) => mutation.expected_version = None,
                    _ => return Err("prepared kill mirror Guild diverged".into()),
                }
            }
        }
        let versions = write_account_store_mutation_plan_in_transaction(
            tx,
            &plan,
            AccountStoreDatabaseMode::SourceOfTruth,
        )?;
        *published = Some(versions);
        Ok(self.checkpoint().clone())
    }
}

impl SimulationConfig {
    pub(crate) fn freeze_prepared_kill_source(&self, reason: String) -> String {
        self.freeze_account_store_writes(reason)
    }
    pub(crate) fn prepare_kill_account_source<F>(
        &self,
        permit: &guild_experience::GuildExperienceCommitPermit,
        runtime_before: CharacterSaveRecord,
        apply: F,
    ) -> Result<PreparedKillAccountSource, String>
    where
        F: FnOnce(&mut AccountStore) -> Result<(), String>,
    {
        self.ensure_account_store_writable()?;
        self.ensure_file_writer_binding()?;
        if self.account_store_database_url.is_none() {
            return Err("prepared PG kill requires account PostgreSQL configuration".into());
        }
        let original = self
            .account_store
            .lock()
            .map_err(|_| "account store poisoned")?
            .clone();
        let durable = original
            .accounts
            .get(&permit.identity.account_id)
            .and_then(|account| account.saves.get(&permit.identity.character_index))
            .ok_or("prepared source original checkpoint missing")?;
        if runtime_before.character.index != permit.identity.character_index
            || runtime_before.revision != durable.revision
        {
            return Err("prepared runtime source revision/identity mismatch".into());
        }
        let mut staged = original.clone();
        apply(&mut staged)?;
        let accounts = [permit.identity.account_id.clone()];
        validate_account_store_transaction_scope(
            &original,
            &staged,
            AccountStoreMutationScope::Accounts(&accounts),
        )
        .map_err(|e| e.to_string())?;
        let guilds = guild_experience::settle_authorized_sources(
            &original,
            &mut staged,
            &accounts,
            Some(permit),
        )?
        .into_iter()
        .collect::<Vec<_>>();
        let scope = AccountStoreMutationScope::AccountsWithGuilds {
            account_ids: &accounts,
            guild_ids: &guilds,
        };
        validate_account_store_transaction_scope(&original, &staged, scope)
            .map_err(|e| e.to_string())?;
        let plan = build_account_store_mutation_plan(&original, &staged, scope, false);
        let checkpoint = staged
            .accounts
            .get(&permit.identity.account_id)
            .and_then(|a| a.saves.get(&permit.identity.character_index))
            .ok_or("prepared source checkpoint missing")?;
        if checkpoint
            .guild_experience_journal
            .applied_kill_receipts
            .get(&permit.kill_key)
            != Some(&permit.kill_payload_hash)
        {
            return Err("prepared source lacks verified kill receipt".into());
        }
        serde_json::to_vec(&staged).map_err(|e| e.to_string())?;
        Ok(PreparedKillAccountSource {
            binding: self.account_store.clone(),
            database_url: self
                .account_store_database_url
                .clone()
                .expect("validated PostgreSQL URL"),
            identity: permit.identity.clone(),
            key: permit.kill_key.clone(),
            hash: permit.kill_payload_hash.clone(),
            runtime_before,
            original,
            staged,
            plan,
            mode: self.account_store_database_mode,
            published: Mutex::new(None),
        })
    }

    /// Guard every external publication with the canonical File WAL, where
    /// configured. A confirmed PG commit followed by File failure is frozen:
    /// compensating only source would contradict the already-committed ledger.
    pub fn publish_prepared_kill_source<T, F>(
        &self,
        source: &PreparedKillAccountSource,
        publish: F,
    ) -> Result<PreparedKillPublication<T>, PreparedKillPublicationFailure>
    where
        F: FnOnce(
            &PreparedKillAccountSource,
        ) -> Result<PreparedKillPublication<T>, PreparedKillPublicationFailure>,
    {
        let _guard = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "source persistence lock poisoned".to_string())?;
        self.ensure_account_store_writable()?;
        self.ensure_file_writer_binding()?;
        if !Arc::ptr_eq(&self.account_store, &source.binding) {
            return Err("prepared source Config binding mismatch".to_string().into());
        }
        if self.account_store_database_url.as_deref() != Some(source.database_url.as_str()) {
            return Err("prepared source repository binding mismatch"
                .to_string()
                .into());
        }
        if self.account_store_database_mode != source.mode {
            return Err("prepared source backend changed".to_string().into());
        }
        let mut live = self
            .account_store
            .lock()
            .map_err(|_| "source account image poisoned".to_string())?;
        for id in source.plan.accounts.keys() {
            if to_json(&live.accounts.get(id))? != to_json(&source.original.accounts.get(id))? {
                return Err("prepared source account changed before publication"
                    .to_string()
                    .into());
            }
        }
        for id in source.plan.guilds.keys() {
            if to_json(&live.shared_guilds.get(id))?
                != to_json(&source.original.shared_guilds.get(id))?
            {
                return Err("prepared source Guild changed before publication"
                    .to_string()
                    .into());
            }
        }
        let publication = self.begin_file_publication()?;
        let outcome = match publish(source) {
            Ok(outcome) => outcome,
            Err(PreparedKillPublicationFailure::Rejected(error)) => {
                if let Some(publication) = publication {
                    publication.settle()?;
                }
                return Err(PreparedKillPublicationFailure::Rejected(error));
            }
            Err(PreparedKillPublicationFailure::OutcomeUnknown(error)) => {
                return Err(PreparedKillPublicationFailure::OutcomeUnknown(
                    self.freeze_account_store_writes(error),
                ));
            }
        };
        match outcome {
            PreparedKillPublication::AlreadyCommitted(value) => {
                if source
                    .published
                    .lock()
                    .map_err(|_| "source publication receipt poisoned".to_string())?
                    .is_some()
                {
                    return Err(PreparedKillPublicationFailure::OutcomeUnknown(
                        self.freeze_account_store_writes(
                            "replayed source unexpectedly wrote a new checkpoint".into(),
                        ),
                    ));
                }
                if let Some(publication) = publication {
                    publication.settle()?;
                }
                Ok(PreparedKillPublication::AlreadyCommitted(value))
            }
            PreparedKillPublication::Written(value) => {
                let versions = source
                    .published
                    .lock()
                    .map_err(|_| {
                        PreparedKillPublicationFailure::OutcomeUnknown(
                            self.freeze_account_store_writes(
                                "source publication receipt poisoned after COMMIT".into(),
                            ),
                        )
                    })?
                    .take()
                    .ok_or_else(|| {
                        PreparedKillPublicationFailure::OutcomeUnknown(
                            self.freeze_account_store_writes(
                                "source publisher omitted committed versions".into(),
                            ),
                        )
                    })?;
                let mut staged = live.clone();
                for id in source.plan.accounts.keys() {
                    staged
                        .accounts
                        .insert(id.clone(), source.staged.accounts[id].clone());
                }
                for id in source.plan.guilds.keys() {
                    if let Some(guild) = source.staged.shared_guilds.get(id) {
                        staged.shared_guilds.insert(id.clone(), guild.clone());
                    } else {
                        staged.shared_guilds.remove(id);
                    }
                }
                apply_account_store_mutation_source_versions(&mut staged, &source.plan, versions);
                if let Some(path) = self.account_store_path.as_deref() {
                    if let Err(error) = self.save_file_account_store(path, &staged) {
                        return Err(PreparedKillPublicationFailure::OutcomeUnknown(
                            self.freeze_account_store_writes(format!(
                                "prepared kill PG committed; File requires reconciliation: {error}"
                            )),
                        ));
                    }
                }
                if let Some(publication) = publication {
                    publication.settle().map_err(|error| {
                        PreparedKillPublicationFailure::OutcomeUnknown(
                            self.freeze_account_store_writes(error),
                        )
                    })?;
                }
                *live = staged;
                self.guild_clock_updates
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                Ok(PreparedKillPublication::Written(value))
            }
        }
    }
}
