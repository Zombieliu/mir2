//! Full private-source participant for an ordinary player's durable drop.
//! The caller owns one PostgreSQL transaction for source, ledger and intent.
use super::*;
use crate::{
    PlayerDropOperationBinding, PlayerDropPreparedOperation, PlayerDropWorldIntent,
    ZoneEconomyWritePermit,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedPlayerDropPublicationFailure {
    Rejected(String),
    OutcomeUnknown(String),
}
impl From<String> for PreparedPlayerDropPublicationFailure {
    fn from(error: String) -> Self {
        Self::Rejected(error)
    }
}
pub enum PreparedPlayerDropPublication<T> {
    Written(T),
    AlreadyCommitted(T),
}

/// Opaque and intentionally non-serializable. Immutable intent/receipt JSON is
/// never sufficient to construct a source participant or replace its fence.
pub struct PreparedPlayerDropAccountSource {
    store_binding: SharedAccountStore,
    database_url: String,
    operation: PlayerDropPreparedOperation,
    original: AccountStore,
    staged: AccountStore,
    plan: AccountStoreMutationPlan,
    permit: ZoneEconomyWritePermit,
    published: Mutex<Option<AccountStoreSourceVersions>>,
}
impl std::fmt::Debug for PreparedPlayerDropAccountSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedPlayerDropAccountSource")
            .field("binding", self.binding())
            .field("operation_digest", &self.operation.operation_digest())
            .finish_non_exhaustive()
    }
}
impl PreparedPlayerDropAccountSource {
    pub fn database_url(&self) -> &str {
        &self.database_url
    }
    pub fn operation(&self) -> &PlayerDropPreparedOperation {
        &self.operation
    }
    pub fn binding(&self) -> &PlayerDropOperationBinding {
        self.operation.binding()
    }
    pub fn account_id(&self) -> &str {
        self.binding().account_id()
    }
    pub fn character_index(&self) -> i32 {
        self.binding().character_index()
    }
    pub fn character_name(&self) -> &str {
        &self.before().character.name
    }
    pub fn idempotency_key(&self) -> String {
        self.binding().operation_key()
    }
    pub fn before(&self) -> &CharacterSaveRecord {
        self.operation.before()
    }
    pub fn runtime_before(&self) -> &CharacterSaveRecord {
        self.operation.runtime_before()
    }
    pub fn checkpoint(&self) -> &CharacterSaveRecord {
        self.operation.checkpoint()
    }
    pub fn intent(&self) -> &PlayerDropWorldIntent {
        self.operation.intent()
    }
    /// Lock order: original Zone lease, Guild rows, then caller economy locks,
    /// then source account/save CAS. No lease is reacquired with a new epoch.
    pub fn lock_postgres_source(&self, tx: &mut Transaction<'_>) -> Result<(), String> {
        self.permit.lock_and_validate(
            tx,
            &self.database_url,
            self.binding().zone_id(),
            self.binding().owner_epoch(),
        )?;
        if self.permit.fence().owner_id() != self.binding().owner_id() {
            return Err(zone_save_fence::REJECTED.into());
        }
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
    /// Provisional until the caller confirms COMMIT. The receipt is single use;
    /// every failed COMMIT must return OutcomeUnknown through publication.
    pub fn write_postgres(&self, tx: &mut Transaction<'_>) -> Result<CharacterSaveRecord, String> {
        let mut published = self
            .published
            .lock()
            .map_err(|_| "player-drop source receipt poisoned")?;
        if published.is_some() {
            return Err("player-drop source already invoked".into());
        }
        self.operation.validate_integrity()?;
        // Enforce the capability even if a caller omits the pre-lock helper.
        self.permit.lock_and_validate(
            tx,
            &self.database_url,
            self.binding().zone_id(),
            self.binding().owner_epoch(),
        )?;
        let versions = write_account_store_mutation_plan_in_transaction(
            tx,
            &self.plan,
            AccountStoreDatabaseMode::SourceOfTruth,
        )?;
        *published = Some(versions);
        Ok(self.checkpoint().clone())
    }
}

impl SimulationConfig {
    pub(crate) fn read_player_drop_canonical_source(
        &self,
        operation: &PlayerDropPreparedOperation,
    ) -> Result<CharacterSaveRecord, String> {
        operation.validate_integrity()?;
        // A readonly load works while the shared write state is frozen. It
        // does not refresh/overwrite the process's mutable account-store image.
        let permit = self.zone_economy_write_permit()?;
        let database_url = self
            .account_store_database_url
            .as_deref()
            .ok_or("player-drop canonical source unavailable")?;
        permit.validate_target_database(database_url)?;
        if permit.fence().zone_id() != operation.binding().zone_id() {
            return Err("player-drop canonical source Zone mismatch".into());
        }
        let (account, _) = load_account_from_postgres(
            database_url.to_owned(),
            operation.binding().account_id().to_owned(),
            false,
        )?
        .ok_or("player-drop canonical account missing")?;
        let save = account
            .saves
            .get(&operation.binding().character_index())
            .ok_or("player-drop canonical checkpoint missing")?;
        if save.character.index != operation.before().character.index
            || save.character.name != operation.before().character.name
        {
            return Err("player-drop canonical character incarnation mismatch".into());
        }
        Ok(save.clone())
    }
    pub(crate) fn freeze_prepared_player_drop_source(&self, reason: String) -> String {
        self.freeze_account_store_writes(reason)
    }
    pub(crate) fn prepare_player_drop_account_source<F>(
        &self,
        intent: PlayerDropWorldIntent,
        runtime_before: CharacterSaveRecord,
        apply: F,
    ) -> Result<PreparedPlayerDropAccountSource, String>
    where
        F: FnOnce(&mut AccountStore) -> Result<(), String>,
    {
        self.ensure_account_store_writable()?;
        self.ensure_file_writer_binding()?;
        let permit = self.zone_economy_write_permit()?;
        let binding = intent.binding();
        intent.validate_integrity()?;
        permit.validate_context(binding.zone_id(), binding.owner_epoch())?;
        if permit.fence().owner_id() != binding.owner_id() {
            return Err(zone_save_fence::REJECTED.into());
        }
        let database_url = self
            .account_store_database_url
            .clone()
            .ok_or("player-drop requires PG SourceOfTruth")?;
        permit.validate_target_database(&database_url)?;
        let original = self
            .account_store
            .lock()
            .map_err(|_| "player-drop account image poisoned")?
            .clone();
        let durable = original
            .accounts
            .get(binding.account_id())
            .and_then(|a| a.saves.get(&binding.character_index()))
            .ok_or("player-drop original checkpoint missing")?;
        if durable.revision != runtime_before.revision
            || durable.character.index != runtime_before.character.index
            || durable.character.name != runtime_before.character.name
            || durable.character.class != runtime_before.character.class
            || durable.character.gender != runtime_before.character.gender
        {
            return Err("player-drop runtime source identity/revision mismatch".into());
        }
        let canonical_before = durable.clone();
        let accounts = [binding.account_id().to_owned()];
        let mut staged = original.clone();
        apply(&mut staged)?;
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
        let mut plan = build_account_store_mutation_plan(&original, &staged, scope, false);
        self.bind_zone_save_fence_to_plan(&mut plan);
        let checkpoint = staged
            .accounts
            .get(binding.account_id())
            .and_then(|a| a.saves.get(&binding.character_index()))
            .ok_or("player-drop staged checkpoint missing")?
            .clone();
        let operation =
            PlayerDropPreparedOperation::new(intent, canonical_before, runtime_before, checkpoint)?;
        serde_json::to_vec(&staged).map_err(|e| e.to_string())?;
        Ok(PreparedPlayerDropAccountSource {
            store_binding: self.account_store.clone(),
            database_url,
            operation,
            original,
            staged,
            plan,
            permit,
            published: Mutex::new(None),
        })
    }

    pub fn publish_prepared_player_drop_source<T, F>(
        &self,
        source: &PreparedPlayerDropAccountSource,
        publish: F,
    ) -> Result<PreparedPlayerDropPublication<T>, PreparedPlayerDropPublicationFailure>
    where
        F: FnOnce(
            &PreparedPlayerDropAccountSource,
        )
            -> Result<PreparedPlayerDropPublication<T>, PreparedPlayerDropPublicationFailure>,
    {
        let _guard = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "player-drop persistence lock poisoned".to_owned())?;
        self.ensure_account_store_writable()?;
        self.ensure_file_writer_binding()?;
        self.validate_zone_save_fence_binding()?;
        if self.account_store_database_mode != AccountStoreDatabaseMode::SourceOfTruth
            || !Arc::ptr_eq(&self.account_store, &source.store_binding)
            || self.account_store_database_url.as_deref() != Some(source.database_url())
            || self.zone_save_fence != source.plan.zone_save_fence
        {
            return Err("player-drop source authority binding changed"
                .to_owned()
                .into());
        }
        source.operation.validate_integrity()?;
        let mut live = self
            .account_store
            .lock()
            .map_err(|_| "player-drop account image poisoned".to_owned())?;
        for id in source.plan.accounts.keys() {
            if to_json(&live.accounts.get(id))? != to_json(&source.original.accounts.get(id))? {
                return Err("player-drop source changed before publication"
                    .to_owned()
                    .into());
            }
        }
        for id in source.plan.guilds.keys() {
            if to_json(&live.shared_guilds.get(id))?
                != to_json(&source.original.shared_guilds.get(id))?
            {
                return Err("player-drop Guild changed before publication"
                    .to_owned()
                    .into());
            }
        }
        let publication = self.begin_file_publication()?;
        let outcome =
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| publish(source))) {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(PreparedPlayerDropPublicationFailure::Rejected(error))) => {
                    if let Some(publication) = publication {
                        publication.settle()?;
                    }
                    return Err(PreparedPlayerDropPublicationFailure::Rejected(error));
                }
                Ok(Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(error))) => {
                    return Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(
                        self.freeze_account_store_writes(error),
                    ));
                }
                Err(_) => {
                    return Err(PreparedPlayerDropPublicationFailure::OutcomeUnknown(
                        self.freeze_account_store_writes(
                            "player-drop publisher panicked; commit outcome unavailable".into(),
                        ),
                    ))
                }
            };
        let unknown = |reason: &str| {
            PreparedPlayerDropPublicationFailure::OutcomeUnknown(
                self.freeze_account_store_writes(reason.into()),
            )
        };
        match outcome {
            PreparedPlayerDropPublication::AlreadyCommitted(value) => {
                if source
                    .published
                    .lock()
                    .map_err(|_| unknown("player-drop receipt poisoned"))?
                    .is_some()
                {
                    return Err(unknown("player-drop duplicate unexpectedly wrote source"));
                }
                if let Some(publication) = publication {
                    publication
                        .settle()
                        .map_err(|_| unknown("player-drop publication settlement unavailable"))?;
                }
                // In particular, NEVER install this operation's older checkpoint.
                Ok(PreparedPlayerDropPublication::AlreadyCommitted(value))
            }
            PreparedPlayerDropPublication::Written(value) => {
                let versions = source
                    .published
                    .lock()
                    .map_err(|_| unknown("player-drop committed source receipt poisoned"))?
                    .take()
                    .ok_or_else(|| {
                        unknown("player-drop publisher omitted committed source versions")
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
                    self.save_file_account_store(path, &staged).map_err(|_| {
                        unknown("player-drop committed; File requires reconciliation")
                    })?;
                }
                if let Some(publication) = publication {
                    publication.settle().map_err(|_| {
                        unknown("player-drop committed publication settlement unavailable")
                    })?;
                }
                *live = staged;
                self.guild_clock_updates
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                Ok(PreparedPlayerDropPublication::Written(value))
            }
        }
    }
}
