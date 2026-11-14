//! A successor's canonical PostgreSQL source read. The opaque result is never
//! deserialized from replica bytes and never writes character data to PG.
use super::*;
use crate::ZoneEconomyWritePermit;
use sha2::{Digest, Sha256};

#[derive(Clone, Serialize)]
struct SaveRead {
    snapshot: Value,
    version: i64,
}
#[derive(Clone, Serialize)]
struct AccountRead {
    raw: Value,
    version: i64,
    saves: BTreeMap<i32, SaveRead>,
    #[serde(skip)]
    account: AccountRecord,
}
#[derive(Clone, Serialize)]
struct SourceRead {
    database: String,
    namespace: String,
    accounts: BTreeMap<String, AccountRead>,
}
struct CacheBefore {
    accounts: BTreeMap<String, Option<Value>>,
    account_versions: BTreeMap<String, Option<i64>>,
    save_versions: BTreeMap<String, Option<BTreeMap<i32, i64>>>,
}

/// Canonical data plus the exact issuing source/configuration and successor
/// fence. Private fields and no serde constructor prevent snapshot authority.
pub struct PreparedZoneSourceRecovery {
    source_binding: SharedAccountStore,
    persistence_binding: Arc<Mutex<()>>,
    database_url: String,
    fence: zone_save_fence::ZoneSaveFence,
    account_ids: Vec<String>,
    character_keys: Vec<(String, i32)>,
    read: SourceRead,
    source_digest: String,
    before: CacheBefore,
    installed: Mutex<bool>,
}
impl fmt::Debug for PreparedZoneSourceRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedZoneSourceRecovery")
            .field("fence", &self.fence)
            .field("source_digest", &self.source_digest)
            .field("account_count", &self.account_ids.len())
            .field("character_count", &self.character_keys.len())
            .finish_non_exhaustive()
    }
}

fn canonical(value: Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(canonical).collect()),
        value => value,
    }
}
fn digest(value: &impl Serialize) -> Result<String, String> {
    let mut hash = Sha256::new();
    hash.update(b"mir2/zone-source-recovery/canonical-read/v1\0");
    hash.update(serde_json::to_vec(&canonical(to_json(value)?)).map_err(|e| e.to_string())?);
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
fn canonical_ids(account_ids: &[String]) -> Result<Vec<String>, String> {
    if account_ids.len() > 10_000 {
        return Err("zone source recovery account bound exceeded".into());
    }
    let mut ids = BTreeSet::new();
    for id in account_ids {
        if id.is_empty() || id.len() > 1024 || id.trim() != id || id.chars().any(char::is_control) {
            return Err("invalid zone source recovery account id".into());
        }
        ids.insert(id.clone());
    }
    Ok(ids.into_iter().collect())
}
fn cache_before(store: &AccountStore, ids: &[String]) -> Result<CacheBefore, String> {
    Ok(CacheBefore {
        accounts: ids
            .iter()
            .map(|id| Ok((id.clone(), store.accounts.get(id).map(to_json).transpose()?)))
            .collect::<Result<_, String>>()?,
        account_versions: ids
            .iter()
            .map(|id| (id.clone(), store.source_account_versions.get(id).copied()))
            .collect(),
        save_versions: ids
            .iter()
            .map(|id| (id.clone(), store.source_save_versions.get(id).cloned()))
            .collect(),
    })
}
fn same_cache(
    store: &AccountStore,
    expected: &CacheBefore,
    ids: &[String],
) -> Result<bool, String> {
    let observed = cache_before(store, ids)?;
    Ok(observed.accounts == expected.accounts
        && observed.account_versions == expected.account_versions
        && observed.save_versions == expected.save_versions)
}

/// Account locks precede all character-save locks, with stable key order.
/// The caller already holds the exact lease row in this same transaction.
fn read_source(tx: &mut Transaction<'_>, ids: &[String]) -> Result<SourceRead, String> {
    let namespace = tx
        .query_one("SELECT current_database(), current_schema()", &[])
        .map_err(|e| e.to_string())?;
    let database: String = namespace.try_get(0).map_err(|e| e.to_string())?;
    let namespace: String = namespace.try_get(1).map_err(|e| e.to_string())?;
    if database.is_empty() || namespace.is_empty() {
        return Err("zone source recovery namespace unavailable".into());
    }
    let mut accounts = BTreeMap::new();
    if !ids.is_empty() {
        let list = ids.to_vec();
        for row in tx.query("SELECT account_id,raw_json,store_version FROM accounts WHERE account_id=ANY($1) ORDER BY account_id FOR SHARE", &[&list]).map_err(|e| e.to_string())? {
            let id: String = row.try_get(0).map_err(|e| e.to_string())?;
            let raw: Value = row.try_get(1).map_err(|e| e.to_string())?;
            let version: i64 = row.try_get(2).map_err(|e| e.to_string())?;
            let account: AccountRecord = serde_json::from_value(raw.clone()).map_err(|e| e.to_string())?;
            crate::billing::validate_account_record(&account)?;
            if version < 0 { return Err("zone source recovery account CAS version invalid".into()); }
            accounts.insert(id, AccountRead { raw, version, saves: BTreeMap::new(), account });
        }
        if accounts.len() != ids.len() {
            return Err("zone source recovery canonical account missing".into());
        }
        for row in tx.query("SELECT account_id,character_index,snapshot_json,save_version FROM character_saves WHERE account_id=ANY($1) ORDER BY account_id,character_index FOR SHARE", &[&list]).map_err(|e| e.to_string())? {
            let id: String = row.try_get(0).map_err(|e| e.to_string())?;
            let index: i32 = row.try_get(1).map_err(|e| e.to_string())?;
            let snapshot: Value = row.try_get(2).map_err(|e| e.to_string())?;
            let version: i64 = row.try_get(3).map_err(|e| e.to_string())?;
            if index < 0 || version < 0 { return Err("zone source recovery save CAS key/version invalid".into()); }
            let account = accounts.get_mut(&id).ok_or("zone source recovery unexpected save account")?;
            let save: CharacterSaveRecord = serde_json::from_value(snapshot.clone()).map_err(|e| e.to_string())?;
            crate::runtime::validate_zone_source_recovery_checkpoint(&save)?;
            let canonical_save = account.account.saves.get(&index).ok_or("zone source recovery orphan save projection")?;
            if save.character.index != index || to_json(&save)? != to_json(canonical_save)?
                || !account.account.characters.iter().any(|character| character == &save.character) {
                return Err("zone source recovery account/save projection conflict".into());
            }
            account.saves.insert(index, SaveRead { snapshot, version });
        }
        for account in accounts.values() {
            if account.saves.keys().ne(account.account.saves.keys()) {
                return Err("zone source recovery canonical save projection missing".into());
            }
            let roster_indices: BTreeSet<_> = account
                .account
                .characters
                .iter()
                .map(|character| character.index)
                .collect();
            if roster_indices.len() != account.account.characters.len()
                || roster_indices
                    .iter()
                    .copied()
                    .ne(account.account.saves.keys().copied())
            {
                return Err(
                    "zone source recovery canonical roster/save projection conflict".into(),
                );
            }
        }
    }
    Ok(SourceRead {
        database,
        namespace,
        accounts,
    })
}

impl PreparedZoneSourceRecovery {
    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }
    pub fn account_ids(&self) -> &[String] {
        &self.account_ids
    }
    pub fn character_keys(&self) -> &[(String, i32)] {
        &self.character_keys
    }
    pub fn source_namespace(&self) -> &str {
        &self.read.namespace
    }
    pub fn fence(&self) -> &zone_save_fence::ZoneSaveFence {
        &self.fence
    }
    pub fn checkpoint(&self, account: &str, index: i32) -> Option<&CharacterSaveRecord> {
        self.read.accounts.get(account)?.account.saves.get(&index)
    }
    /// Audited absence is evidence only for a loaded, still-existing account.
    /// A missing account or inconsistent raw/projection state is never retirement.
    pub fn is_retired_character(&self, account: &str, index: i32) -> Result<bool, String> {
        if index < 0 {
            return Err("zone source recovery retirement index invalid".into());
        }
        let account = self
            .read
            .accounts
            .get(account)
            .ok_or("zone source recovery retirement account was not loaded")?;
        let roster = account
            .account
            .characters
            .iter()
            .any(|character| character.index == index);
        let raw = account.account.saves.contains_key(&index);
        let projection = account.saves.contains_key(&index);
        match (roster, raw, projection) {
            (false, false, false) => Ok(true),
            (true, true, true) => Ok(false),
            _ => Err("zone source recovery retirement presence is inconsistent".into()),
        }
    }
    pub(crate) fn validate_active_account(&self, account: &str) -> Result<(), String> {
        let account = self
            .read
            .accounts
            .get(account)
            .ok_or("zone source recovery active account was not loaded")?;
        if account.account.active_ban(unix_now_ms()).is_some() {
            return Err("zone source recovery active account is banned".into());
        }
        Ok(())
    }
    pub fn validate_target_database(&self, database_url: &str) -> Result<(), String> {
        if self.database_url != database_url {
            return Err("zone source recovery database binding mismatch".into());
        }
        Ok(())
    }
    pub fn validate_source_binding(&self, config: &SimulationConfig) -> Result<(), String> {
        config.validate_zone_save_fence_binding()?;
        if config.zone_save_operation.is_some()
            || config.zone_save_fence.as_ref() != Some(&self.fence)
            || config.account_store_database_url.as_deref() != Some(self.database_url.as_str())
            || !Arc::ptr_eq(&config.account_store, &self.source_binding)
            || !Arc::ptr_eq(
                &config.account_store_persist_lock,
                &self.persistence_binding,
            )
        {
            return Err("zone source recovery original configuration binding mismatch".into());
        }
        Ok(())
    }
    /// The recovery marker calls this in its OWN transaction, after locking
    /// its operation rows. No independent connection or stale CAS read is used.
    pub fn lock_and_validate_postgres(
        &self,
        tx: &mut Transaction<'_>,
        permit: &ZoneEconomyWritePermit,
        database_url: &str,
    ) -> Result<(), String> {
        self.validate_target_database(database_url)?;
        permit.validate_target_database(database_url)?;
        if permit.fence() != &self.fence {
            return Err("zone source recovery successor fence mismatch".into());
        }
        permit.lock_and_validate(
            tx,
            database_url,
            self.fence.zone_id(),
            self.fence.fencing_token(),
        )?;
        let observed = read_source(tx, &self.account_ids)?;
        if to_json(&observed)? != to_json(&self.read)? {
            return Err("zone source recovery canonical source head changed".into());
        }
        Ok(())
    }
    pub(crate) fn validate_installed_cache(&self, config: &SimulationConfig) -> Result<(), String> {
        self.validate_source_binding(config)?;
        let store = config
            .account_store
            .lock()
            .map_err(|_| "zone source recovery cache poisoned")?;
        if !self.cache_is_canonical(&store)? {
            return Err(
                "zone source recovery canonical cache is not installed or has advanced".into(),
            );
        }
        Ok(())
    }
    fn cache_is_canonical(&self, store: &AccountStore) -> Result<bool, String> {
        for (id, expected) in &self.read.accounts {
            if store.accounts.get(id).map(to_json).transpose()? != Some(to_json(&expected.account)?)
                || store.source_account_versions.get(id) != Some(&expected.version)
                || store.source_save_versions.get(id)
                    != Some(
                        &expected
                            .saves
                            .iter()
                            .map(|(index, save)| (*index, save.version))
                            .collect(),
                    )
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl SimulationConfig {
    #[cfg(test)]
    pub(crate) fn zone_source_recovery_fixture(
        &self,
        accounts: BTreeMap<String, AccountRecord>,
        version: i64,
    ) -> Result<PreparedZoneSourceRecovery, String> {
        self.validate_zone_save_fence_binding()?;
        let fence = self
            .zone_save_fence
            .clone()
            .ok_or("fixture successor fence missing")?;
        let database_url = self
            .account_store_database_url
            .clone()
            .ok_or("fixture source URL missing")?;
        let account_ids = canonical_ids(&accounts.keys().cloned().collect::<Vec<_>>())?;
        let before = {
            let store = self
                .account_store
                .lock()
                .map_err(|_| "fixture cache poisoned")?;
            cache_before(&store, &account_ids)?
        };
        let mut read_accounts = BTreeMap::new();
        for (id, account) in accounts {
            crate::billing::validate_account_record(&account)?;
            let mut saves = BTreeMap::new();
            for (index, save) in &account.saves {
                crate::runtime::validate_zone_source_recovery_checkpoint(save)?;
                saves.insert(
                    *index,
                    SaveRead {
                        snapshot: to_json(save)?,
                        version,
                    },
                );
            }
            read_accounts.insert(
                id,
                AccountRead {
                    raw: to_json(&account)?,
                    version,
                    saves,
                    account,
                },
            );
        }
        self.finish_zone_source_recovery(
            database_url,
            fence,
            account_ids,
            before,
            SourceRead {
                database: "pure-fixture".into(),
                namespace: "fixture-namespace".into(),
                accounts: read_accounts,
            },
        )
    }
    pub fn prepare_zone_source_recovery(
        &self,
        account_ids: &[String],
    ) -> Result<PreparedZoneSourceRecovery, String> {
        self.validate_zone_save_fence_binding()?;
        if self.zone_save_operation.is_some() {
            return Err("zone source recovery cannot use an operation-scoped source".into());
        }
        let fence = self
            .zone_save_fence
            .clone()
            .ok_or("zone source recovery requires an existing successor fence")?;
        let database_url = self
            .account_store_database_url
            .clone()
            .ok_or("zone source recovery database unavailable")?;
        let account_ids = canonical_ids(account_ids)?;
        let before = {
            let store = self
                .account_store
                .lock()
                .map_err(|_| "zone source recovery cache poisoned")?;
            cache_before(&store, &account_ids)?
        };
        let pool = postgres_account_store_pool(&database_url);
        let mut client = pool.connection()?;
        // SELECT FOR UPDATE on the lease requires a read/write transaction.
        // Recovery performs no INSERT/UPDATE/DELETE, DDL, or source publication.
        let mut tx = client
            .build_transaction()
            .isolation_level(postgres::IsolationLevel::RepeatableRead)
            .start()
            .map_err(|e| e.to_string())?;
        fence.lock_and_validate(&mut tx, AccountStoreDatabaseMode::SourceOfTruth)?;
        let read = read_source(&mut tx, &account_ids)?;
        tx.commit()
            .map_err(|_| "zone source recovery read transaction unavailable")?;
        self.finish_zone_source_recovery(database_url, fence, account_ids, before, read)
    }
    fn finish_zone_source_recovery(
        &self,
        database_url: String,
        fence: zone_save_fence::ZoneSaveFence,
        account_ids: Vec<String>,
        before: CacheBefore,
        read: SourceRead,
    ) -> Result<PreparedZoneSourceRecovery, String> {
        let character_keys = read
            .accounts
            .iter()
            .flat_map(|(id, account)| account.saves.keys().map(move |index| (id.clone(), *index)))
            .collect();
        // Deployment credentials are transient capability material. Rotation
        // must not turn a durable recovery head into a password verifier.
        let source_digest = digest(&(
            fence.zone_id(),
            fence.owner_id(),
            fence.fencing_token(),
            &read,
        ))?;
        Ok(PreparedZoneSourceRecovery {
            source_binding: self.account_store.clone(),
            persistence_binding: self.account_store_persist_lock.clone(),
            database_url,
            fence,
            account_ids,
            character_keys,
            read,
            source_digest,
            before,
            installed: Mutex::new(false),
        })
    }
    /// Publishes only the opaque PostgreSQL read into its original local cache.
    /// It never persists replica data or clears an Unknown publication freeze.
    pub fn install_zone_source_recovery_cache(
        &self,
        source: &PreparedZoneSourceRecovery,
    ) -> Result<(), String> {
        source.validate_source_binding(self)?;
        let _guard = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "zone source recovery persistence lock poisoned")?;
        let mut installed = source
            .installed
            .lock()
            .map_err(|_| "zone source recovery receipt poisoned")?;
        let mut store = self
            .account_store
            .lock()
            .map_err(|_| "zone source recovery cache poisoned")?;
        if *installed {
            return if source.cache_is_canonical(&store)? {
                Ok(())
            } else {
                Err("zone source recovery cache advanced after installation".into())
            };
        }
        if !same_cache(&store, &source.before, &source.account_ids)? {
            return Err("zone source recovery cache changed since canonical preparation".into());
        }
        for (id, account) in &source.read.accounts {
            if store
                .source_account_versions
                .get(id)
                .is_some_and(|version| *version > account.version)
            {
                return Err(
                    "zone source recovery would roll back a newer source CAS version".into(),
                );
            }
            if store.source_account_versions.get(id) == Some(&account.version)
                && store.accounts.get(id).map(to_json).transpose()?
                    != Some(to_json(&account.account)?)
            {
                return Err(
                    "zone source recovery conflicts with equal-version complete source".into(),
                );
            }
            for (index, save) in &account.saves {
                if store
                    .source_save_version(id, *index)
                    .is_some_and(|version| version > save.version)
                {
                    return Err(
                        "zone source recovery would roll back a newer character CAS version".into(),
                    );
                }
            }
        }
        for (id, account) in &source.read.accounts {
            store.accounts.insert(id.clone(), account.account.clone());
            store
                .source_account_versions
                .insert(id.clone(), account.version);
            store.source_save_versions.insert(
                id.clone(),
                account
                    .saves
                    .iter()
                    .map(|(index, save)| (*index, save.version))
                    .collect(),
            );
        }
        store.normalize_next_character_index();
        *installed = true;
        Ok(())
    }
}

#[cfg(test)]
#[path = "config_zone_source_recovery_tests.rs"]
mod tests;
