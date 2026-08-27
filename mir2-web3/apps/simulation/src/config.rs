use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

use mir2_game_data::{
    content_profile_experience_required, content_profile_monster_is_boss,
    content_profile_respawn_overrides_for_map, crystal_map_respawns_by_file_name,
    crystal_respawn_manifest, platinum_176_profile, platinum_176_profile_bundle,
    starter_map_collision, starter_scene, validate_content_profile, ContentProfile,
    ContentSkillRule, CrystalRespawnTemplate, DecorObjectTemplate, MapBounds, SceneBootstrap,
    SceneView, StarterMapCollision, TerrainPatchTemplate,
};
use mir2_protocol::{
    ClientIntelligentCreature, MapInformation, MirClass, MirDirection, MirGender, ObjectPlayerInfo,
    Point, SelectInfo, Spell, UserItem, UserItemStat,
};
use postgres::{Client, Config as PostgresClientConfig, NoTls, Transaction};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ItemGrade {
    #[default]
    None,
    Common,
    Rare,
    Legendary,
    Mythical,
    Heroic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterRecord {
    pub index: i32,
    pub name: String,
    pub level: u16,
    pub class: MirClass,
    pub gender: MirGender,
}

impl CharacterRecord {
    pub fn to_select_info(&self) -> SelectInfo {
        SelectInfo {
            index: self.index,
            name: self.name.clone(),
            level: self.level,
            class: self.class,
            gender: self.gender,
            last_access_binary_datetime: 638452800000000000,
        }
    }

    pub fn to_new_character_select_info(&self) -> SelectInfo {
        SelectInfo {
            index: self.index,
            name: self.name.clone(),
            level: 0,
            class: self.class,
            gender: self.gender,
            last_access_binary_datetime: 0,
        }
    }
}

pub type SharedAccountStore = Arc<Mutex<AccountStore>>;
const ACCOUNT_STORE_SCHEMA_VERSION: u16 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountStore {
    #[serde(
        rename = "schemaVersion",
        default = "legacy_account_store_schema_version"
    )]
    pub schema_version: u16,
    #[serde(rename = "nextCharacterIndex", default)]
    pub next_character_index: i32,
    /// Crystal `Envir.GameshopLog`: cumulative purchases for finite products
    /// whose stock is shared by every account. Unlimited (`stock == 0`) rows
    /// never enter this map.
    #[serde(rename = "gameShopGlobalPurchases", default)]
    pub game_shop_global_purchases: BTreeMap<i32, u64>,
    pub accounts: BTreeMap<String, AccountRecord>,
    #[serde(skip)]
    source_account_versions: BTreeMap<String, i64>,
    #[serde(skip)]
    source_save_versions: BTreeMap<String, BTreeMap<i32, i64>>,
    #[serde(skip)]
    source_game_shop_global_version: Option<i64>,
    #[serde(skip)]
    source_game_shop_global_purchases: BTreeMap<i32, u64>,
}

impl AccountStore {
    pub fn new(default_character: CharacterRecord) -> Self {
        let mut accounts = BTreeMap::new();
        accounts.insert("demo".to_string(), AccountRecord::new(default_character));
        Self {
            schema_version: ACCOUNT_STORE_SCHEMA_VERSION,
            next_character_index: 1,
            game_shop_global_purchases: BTreeMap::new(),
            accounts,
            source_account_versions: BTreeMap::new(),
            source_save_versions: BTreeMap::new(),
            source_game_shop_global_version: None,
            source_game_shop_global_purchases: BTreeMap::new(),
        }
    }

    pub fn load_or_new(path: &Path, default_character: CharacterRecord) -> Self {
        let store = fs::read_to_string(path)
            .ok()
            .and_then(|data| serde_json::from_str::<AccountStore>(&data).ok())
            .unwrap_or_else(|| AccountStore::new(default_character.clone()));
        store
            .with_default_account(default_character)
            .migrate_to_current_schema()
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        self.save_to_path_with_fault(path, None)
            .map_err(|error| error.to_string())
    }

    fn save_to_path_with_fault(
        &self,
        path: &Path,
        fault: Option<AccountStoreFileCommitFault>,
    ) -> Result<(), AccountStoreFileCommitError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AccountStoreFileCommitError::not_committed(io::Error::new(
                    error.kind(),
                    format!(
                        "failed to create account store directory {}: {error}",
                        parent.display()
                    ),
                ))
            })?;
        }

        let data = serde_json::to_string_pretty(self).map_err(|error| {
            AccountStoreFileCommitError::not_committed(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to encode account store: {error}"),
            ))
        })?;
        write_file_atomically(path, data.as_bytes(), fault).map_err(|error| {
            error.with_context(format!(
                "failed to atomically write account store {}",
                path.display()
            ))
        })
    }

    fn with_default_account(mut self, default_character: CharacterRecord) -> Self {
        self.accounts
            .entry("demo".to_string())
            .or_insert_with(|| AccountRecord::new(default_character));
        self
    }

    fn migrate_to_current_schema(mut self) -> Self {
        if self.schema_version < ACCOUNT_STORE_SCHEMA_VERSION {
            self.schema_version = ACCOUNT_STORE_SCHEMA_VERSION;
        }
        self.normalize_next_character_index();
        self
    }

    pub(crate) fn allocate_character_index(&mut self) -> i32 {
        self.normalize_next_character_index();
        let index = self.next_character_index;
        self.next_character_index = self.next_character_index.saturating_add(1);
        index
    }

    fn normalize_next_character_index(&mut self) {
        let min_next = self
            .accounts
            .values()
            .flat_map(|account| account.characters.iter().map(|character| character.index))
            .max()
            .map(|index| index.saturating_add(1))
            .unwrap_or(0);
        if self.next_character_index < min_next {
            self.next_character_index = min_next;
        }
    }

    fn with_source_versions(mut self, versions: AccountStoreSourceVersions) -> Self {
        self.source_account_versions = versions.accounts;
        self.source_save_versions = versions.saves;
        self.source_game_shop_global_version = versions.game_shop_global_version;
        self.source_game_shop_global_purchases = self.game_shop_global_purchases.clone();
        self
    }

    fn source_account_version(&self, account_id: &str) -> Option<i64> {
        self.source_account_versions.get(account_id).copied()
    }

    fn source_save_version(&self, account_id: &str, character_index: i32) -> Option<i64> {
        self.source_save_versions
            .get(account_id)
            .and_then(|versions| versions.get(&character_index))
            .copied()
    }

    fn scoped_to_account(&self, account_id: &str) -> Self {
        self.scoped_to_accounts(std::slice::from_ref(&account_id))
    }

    fn scoped_to_accounts(&self, account_ids: &[&str]) -> Self {
        let mut accounts = BTreeMap::new();
        for account_id in account_ids {
            if let Some(account) = self.accounts.get(*account_id) {
                accounts.insert((*account_id).to_string(), account.clone());
            }
        }

        let mut source_account_versions = BTreeMap::new();
        for account_id in account_ids {
            if let Some(version) = self.source_account_versions.get(*account_id) {
                source_account_versions.insert((*account_id).to_string(), *version);
            }
        }

        let mut source_save_versions = BTreeMap::new();
        for account_id in account_ids {
            if let Some(versions) = self.source_save_versions.get(*account_id) {
                source_save_versions.insert((*account_id).to_string(), versions.clone());
            }
        }

        Self {
            schema_version: self.schema_version,
            next_character_index: self.next_character_index,
            game_shop_global_purchases: self.game_shop_global_purchases.clone(),
            accounts,
            source_account_versions,
            source_save_versions,
            source_game_shop_global_version: self.source_game_shop_global_version,
            source_game_shop_global_purchases: self.source_game_shop_global_purchases.clone(),
        }
    }

    fn merge_source_versions(&mut self, versions: AccountStoreSourceVersions) {
        for (account_id, version) in versions.accounts {
            self.source_account_versions.insert(account_id, version);
        }
        for (account_id, saves) in versions.saves {
            self.source_save_versions
                .entry(account_id)
                .or_default()
                .extend(saves);
        }
        if let Some(version) = versions.game_shop_global_version {
            self.source_game_shop_global_version = Some(version);
            self.source_game_shop_global_purchases = self.game_shop_global_purchases.clone();
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AccountStoreSourceVersions {
    accounts: BTreeMap<String, i64>,
    saves: BTreeMap<String, BTreeMap<i32, i64>>,
    game_shop_global_version: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
enum AccountStoreMutationScope<'a> {
    Accounts(&'a [String]),
    AccountsWithGlobal(&'a [String]),
    FullRestore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AccountStoreTransactionScopeError {
    OutOfScopeAccountChanged { account_id: String },
    SourceMetadataChanged { field: &'static str },
    UnauthorizedGlobalStockChanged,
    AccountFingerprintFailed { account_id: String, reason: String },
}

impl fmt::Display for AccountStoreTransactionScopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfScopeAccountChanged { account_id } => write!(
                formatter,
                "account-store transaction scope violation: account {account_id} changed outside the authorized scope"
            ),
            Self::SourceMetadataChanged { field } => write!(
                formatter,
                "account-store transaction scope violation: closure changed protected source metadata {field}"
            ),
            Self::UnauthorizedGlobalStockChanged => write!(
                formatter,
                "account-store transaction scope violation: account-only closure changed global stock"
            ),
            Self::AccountFingerprintFailed { account_id, reason } => write!(
                formatter,
                "account-store transaction scope validation could not fingerprint account {account_id}: {reason}"
            ),
        }
    }
}

impl std::error::Error for AccountStoreTransactionScopeError {}

fn validate_account_store_transaction_scope(
    original: &AccountStore,
    staged: &AccountStore,
    scope: AccountStoreMutationScope<'_>,
) -> Result<(), AccountStoreTransactionScopeError> {
    let (account_ids, include_global) = match scope {
        AccountStoreMutationScope::Accounts(account_ids) => (account_ids, false),
        AccountStoreMutationScope::AccountsWithGlobal(account_ids) => (account_ids, true),
        AccountStoreMutationScope::FullRestore => return Ok(()),
    };
    let authorized = account_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut all_account_ids = BTreeSet::new();
    all_account_ids.extend(original.accounts.keys().cloned());
    all_account_ids.extend(staged.accounts.keys().cloned());
    for account_id in all_account_ids {
        if authorized.contains(&account_id) {
            continue;
        }
        let fingerprint = |account: Option<&AccountRecord>| {
            account
                .map(serde_json::to_vec)
                .transpose()
                .map_err(
                    |error| AccountStoreTransactionScopeError::AccountFingerprintFailed {
                        account_id: account_id.clone(),
                        reason: error.to_string(),
                    },
                )
        };
        if fingerprint(original.accounts.get(&account_id))?
            != fingerprint(staged.accounts.get(&account_id))?
        {
            return Err(AccountStoreTransactionScopeError::OutOfScopeAccountChanged { account_id });
        }
    }

    for (changed, field) in [
        (
            original.source_account_versions != staged.source_account_versions,
            "source_account_versions",
        ),
        (
            original.source_save_versions != staged.source_save_versions,
            "source_save_versions",
        ),
        (
            original.source_game_shop_global_version != staged.source_game_shop_global_version,
            "source_game_shop_global_version",
        ),
        (
            original.source_game_shop_global_purchases != staged.source_game_shop_global_purchases,
            "source_game_shop_global_purchases",
        ),
    ] {
        if changed {
            return Err(AccountStoreTransactionScopeError::SourceMetadataChanged { field });
        }
    }
    if !include_global && original.game_shop_global_purchases != staged.game_shop_global_purchases {
        return Err(AccountStoreTransactionScopeError::UnauthorizedGlobalStockChanged);
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct AccountStoreMutationPlan {
    accounts: BTreeMap<String, AccountStoreAccountMutation>,
    global_stock: Option<AccountStoreGlobalStockMutation>,
}

#[derive(Debug, Clone)]
struct AccountStoreAccountMutation {
    expected_version: Option<i64>,
    desired_account: Option<AccountRecord>,
    saves: BTreeMap<i32, AccountStoreSaveMutation>,
}

#[derive(Debug, Clone)]
struct AccountStoreSaveMutation {
    expected_version: Option<i64>,
    desired_save: Option<CharacterSaveRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountStoreGlobalStockMutation {
    expected_version: Option<i64>,
    expected_purchases: BTreeMap<i32, u64>,
    desired_purchases: BTreeMap<i32, u64>,
}

enum AccountStoreMutationOperation<'a> {
    GlobalStock(&'a AccountStoreGlobalStockMutation),
    Account {
        account_id: &'a str,
        mutation: &'a AccountStoreAccountMutation,
    },
}

enum AccountStoreMutationOperationResult {
    GlobalStock(Option<i64>),
    Account {
        account_version: Option<i64>,
        save_versions: BTreeMap<i32, i64>,
    },
}

fn build_account_store_mutation_plan(
    original: &AccountStore,
    desired: &AccountStore,
    scope: AccountStoreMutationScope<'_>,
    force_global_stock: bool,
) -> AccountStoreMutationPlan {
    let mut account_ids = BTreeSet::new();
    match scope {
        AccountStoreMutationScope::Accounts(scoped_ids)
        | AccountStoreMutationScope::AccountsWithGlobal(scoped_ids) => {
            account_ids.extend(scoped_ids.iter().cloned());
        }
        AccountStoreMutationScope::FullRestore => {
            account_ids.extend(original.accounts.keys().cloned());
            account_ids.extend(original.source_account_versions.keys().cloned());
            account_ids.extend(original.source_save_versions.keys().cloned());
            account_ids.extend(desired.accounts.keys().cloned());
        }
    }

    let mut accounts = BTreeMap::new();
    for account_id in account_ids {
        let original_account = original.accounts.get(&account_id);
        let desired_account = desired.accounts.get(&account_id);
        let mut save_indices = BTreeSet::new();
        if let Some(account) = original_account {
            save_indices.extend(account.saves.keys().copied());
        }
        if let Some(account) = desired_account {
            save_indices.extend(account.saves.keys().copied());
        }
        if let Some(versions) = original.source_save_versions.get(&account_id) {
            save_indices.extend(versions.keys().copied());
        }

        let saves = save_indices
            .into_iter()
            .map(|character_index| {
                (
                    character_index,
                    AccountStoreSaveMutation {
                        expected_version: original
                            .source_save_version(&account_id, character_index),
                        desired_save: desired_account
                            .and_then(|account| account.saves.get(&character_index))
                            .cloned(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let expected_version = original.source_account_version(&account_id);
        if original_account.is_none()
            && desired_account.is_none()
            && expected_version.is_none()
            && saves.is_empty()
        {
            continue;
        }
        accounts.insert(
            account_id,
            AccountStoreAccountMutation {
                expected_version,
                desired_account: desired_account.cloned(),
                saves,
            },
        );
    }

    let include_global = matches!(
        scope,
        AccountStoreMutationScope::AccountsWithGlobal(_) | AccountStoreMutationScope::FullRestore
    );
    let global_stock = if force_global_stock
    // The migrated game_shop_global_stock row is a singleton.  Full restore
    // therefore emits an explicit replace/reset (including an empty map).
    // Account-bounded work only includes it through explicit global scope.
        || matches!(scope, AccountStoreMutationScope::FullRestore)
        || (include_global
            && desired.game_shop_global_purchases
                != original.source_game_shop_global_purchases)
    {
        Some(AccountStoreGlobalStockMutation {
            expected_version: original.source_game_shop_global_version,
            expected_purchases: original.source_game_shop_global_purchases.clone(),
            desired_purchases: desired.game_shop_global_purchases.clone(),
        })
    } else {
        None
    };

    AccountStoreMutationPlan {
        accounts,
        global_stock,
    }
}

fn execute_account_store_mutation_plan<F>(
    plan: &AccountStoreMutationPlan,
    mut apply: F,
) -> Result<AccountStoreSourceVersions, String>
where
    F: for<'a> FnMut(
        AccountStoreMutationOperation<'a>,
    ) -> Result<AccountStoreMutationOperationResult, String>,
{
    let mut versions = AccountStoreSourceVersions::default();
    if let Some(global_stock) = plan.global_stock.as_ref() {
        match apply(AccountStoreMutationOperation::GlobalStock(global_stock))? {
            AccountStoreMutationOperationResult::GlobalStock(version) => {
                versions.game_shop_global_version = version;
            }
            AccountStoreMutationOperationResult::Account { .. } => {
                return Err(
                    "account-store mutation repository returned an account result for global stock"
                        .to_string(),
                );
            }
        }
    }

    for (account_id, mutation) in &plan.accounts {
        let (account_version, save_versions) =
            match apply(AccountStoreMutationOperation::Account {
                account_id,
                mutation,
            })? {
                AccountStoreMutationOperationResult::Account {
                    account_version,
                    save_versions,
                } => (account_version, save_versions),
                AccountStoreMutationOperationResult::GlobalStock(_) => {
                    return Err(
                        "account-store mutation repository returned a global result for an account"
                            .to_string(),
                    );
                }
            };
        if mutation.desired_account.is_some() {
            let account_version = account_version.ok_or_else(|| {
                format!("account-store mutation repository omitted version for {account_id}")
            })?;
            versions
                .accounts
                .insert(account_id.clone(), account_version);
            for (character_index, save_mutation) in &mutation.saves {
                if save_mutation.desired_save.is_some() {
                    let save_version = save_versions.get(character_index).copied().ok_or_else(|| {
                        format!(
                            "account-store mutation repository omitted save version for {account_id}/{character_index}"
                        )
                    })?;
                    versions
                        .saves
                        .entry(account_id.clone())
                        .or_default()
                        .insert(*character_index, save_version);
                }
            }
        } else if account_version.is_some() || !save_versions.is_empty() {
            return Err(format!(
                "account-store mutation repository returned versions for deleted account {account_id}"
            ));
        }
    }
    Ok(versions)
}

fn apply_account_store_mutation_source_versions(
    store: &mut AccountStore,
    plan: &AccountStoreMutationPlan,
    versions: AccountStoreSourceVersions,
) {
    for (account_id, mutation) in &plan.accounts {
        if mutation.desired_account.is_none() {
            store.source_account_versions.remove(account_id);
            store.source_save_versions.remove(account_id);
            continue;
        }
        if let Some(version) = versions.accounts.get(account_id) {
            store
                .source_account_versions
                .insert(account_id.clone(), *version);
        }
        let save_versions = versions.saves.get(account_id).cloned().unwrap_or_default();
        if save_versions.is_empty() {
            store.source_save_versions.remove(account_id);
        } else {
            store
                .source_save_versions
                .insert(account_id.clone(), save_versions);
        }
    }
    if let Some(global_stock) = plan.global_stock.as_ref() {
        store.source_game_shop_global_version = versions.game_shop_global_version;
        store.source_game_shop_global_purchases = global_stock.desired_purchases.clone();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStoreRepositoryStatus {
    pub backend: String,
    pub mode: AccountStoreDatabaseMode,
    pub configured: bool,
    pub location: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountStoreRepositorySave {
    pub account_versions: BTreeMap<String, i64>,
    pub save_versions: BTreeMap<String, BTreeMap<i32, i64>>,
    pub game_shop_global_version: Option<i64>,
}

impl From<AccountStoreSourceVersions> for AccountStoreRepositorySave {
    fn from(value: AccountStoreSourceVersions) -> Self {
        Self {
            account_versions: value.accounts,
            save_versions: value.saves,
            game_shop_global_version: value.game_shop_global_version,
        }
    }
}

impl AccountStoreRepositorySave {
    fn into_source_versions(self) -> AccountStoreSourceVersions {
        AccountStoreSourceVersions {
            accounts: self.account_versions,
            saves: self.save_versions,
            game_shop_global_version: self.game_shop_global_version,
        }
    }
}

#[derive(Debug)]
enum AccountStoreFileCommitError {
    NotCommitted(io::Error),
    CommitOutcomeUnknown(io::Error),
}

impl AccountStoreFileCommitError {
    fn not_committed(error: io::Error) -> Self {
        Self::NotCommitted(error)
    }

    fn commit_outcome_unknown(error: io::Error) -> Self {
        Self::CommitOutcomeUnknown(error)
    }

    fn with_context(self, context: String) -> Self {
        match self {
            Self::NotCommitted(error) => {
                Self::NotCommitted(io::Error::new(error.kind(), format!("{context}: {error}")))
            }
            Self::CommitOutcomeUnknown(error) => Self::CommitOutcomeUnknown(io::Error::new(
                error.kind(),
                format!("{context}: {error}"),
            )),
        }
    }

    fn is_commit_outcome_unknown(&self) -> bool {
        matches!(self, Self::CommitOutcomeUnknown(_))
    }

    fn is_retryable_not_committed(&self) -> bool {
        matches!(
            self,
            Self::NotCommitted(error) if is_retryable_atomic_replace_error(error)
        )
    }
}

impl fmt::Display for AccountStoreFileCommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCommitted(error) => write!(formatter, "not committed: {error}"),
            Self::CommitOutcomeUnknown(error) => {
                write!(formatter, "commit outcome unknown: {error}")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountStoreFileCommitFault {
    BeforeRename,
    AfterRenameBeforeDirectorySync,
}

pub trait AccountStoreRepository: Send + Sync {
    fn load(&self, default_character: CharacterRecord) -> Result<AccountStore, String>;
    fn save(&self, store: &AccountStore) -> Result<AccountStoreRepositorySave, String>;
    fn status(&self) -> AccountStoreRepositoryStatus;
}

#[derive(Debug, Clone)]
pub struct FileAccountStoreRepository {
    path: PathBuf,
}

impl FileAccountStoreRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn save_with_commit_outcome(
        &self,
        store: &AccountStore,
        fault: Option<AccountStoreFileCommitFault>,
    ) -> Result<AccountStoreRepositorySave, AccountStoreFileCommitError> {
        save_account_store_snapshot_to_path(store, &self.path, fault)?;
        Ok(AccountStoreRepositorySave::default())
    }
}

impl AccountStoreRepository for FileAccountStoreRepository {
    fn load(&self, default_character: CharacterRecord) -> Result<AccountStore, String> {
        Ok(AccountStore::load_or_new(&self.path, default_character))
    }

    fn save(&self, store: &AccountStore) -> Result<AccountStoreRepositorySave, String> {
        self.save_with_commit_outcome(store, None)
            .map_err(|error| error.to_string())
    }

    fn status(&self) -> AccountStoreRepositoryStatus {
        AccountStoreRepositoryStatus {
            backend: "file".to_string(),
            mode: AccountStoreDatabaseMode::Mirror,
            configured: true,
            location: self.path.display().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PostgresAccountStoreRepository {
    database_url: String,
    mode: AccountStoreDatabaseMode,
}

impl PostgresAccountStoreRepository {
    pub fn new(database_url: impl Into<String>, mode: AccountStoreDatabaseMode) -> Self {
        Self {
            database_url: database_url.into(),
            mode,
        }
    }

    fn save_mutation_plan(
        &self,
        plan: &AccountStoreMutationPlan,
    ) -> Result<AccountStoreRepositorySave, String> {
        save_account_store_mutation_plan_to_postgres(
            self.database_url.clone(),
            plan.clone(),
            self.mode,
        )
        .map(AccountStoreRepositorySave::from)
    }
}

impl AccountStoreRepository for PostgresAccountStoreRepository {
    fn load(&self, default_character: CharacterRecord) -> Result<AccountStore, String> {
        load_account_store_from_postgres(self.database_url.clone(), default_character)
    }

    fn save(&self, store: &AccountStore) -> Result<AccountStoreRepositorySave, String> {
        save_account_store_to_postgres(self.database_url.clone(), store.clone(), self.mode)
            .map(AccountStoreRepositorySave::from)
    }

    fn status(&self) -> AccountStoreRepositoryStatus {
        AccountStoreRepositoryStatus {
            backend: "postgres".to_string(),
            mode: self.mode,
            configured: !self.database_url.trim().is_empty(),
            location: redact_database_url(&self.database_url),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PostgresAccountStorePoolConfig {
    max_size: usize,
    wait_timeout: Duration,
    connect_timeout: Duration,
    test_on_checkout: bool,
}

impl PostgresAccountStorePoolConfig {
    fn from_env() -> Self {
        Self {
            max_size: env_usize("MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE")
                .unwrap_or(8)
                .clamp(1, 64),
            wait_timeout: env_millis("MIR2_ACCOUNT_STORE_PG_POOL_WAIT_TIMEOUT_MS")
                .unwrap_or(Duration::from_secs(2))
                .max(Duration::from_millis(1)),
            connect_timeout: env_millis("MIR2_ACCOUNT_STORE_PG_CONNECT_TIMEOUT_MS")
                .unwrap_or(Duration::from_secs(3))
                .max(Duration::from_millis(1)),
            test_on_checkout: env_flag_enabled("MIR2_ACCOUNT_STORE_PG_POOL_TEST_ON_CHECKOUT"),
        }
    }

    fn cache_key(self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.max_size,
            self.wait_timeout.as_millis(),
            self.connect_timeout.as_millis(),
            self.test_on_checkout
        )
    }
}

struct PostgresAccountStoreConnectionPool {
    database_url: String,
    config: PostgresAccountStorePoolConfig,
    state: Mutex<PostgresAccountStorePoolState>,
    available: Condvar,
    migration_completed: Mutex<bool>,
}

#[derive(Default)]
struct PostgresAccountStorePoolState {
    idle: Vec<Client>,
    open: usize,
}

impl fmt::Debug for PostgresAccountStoreConnectionPool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresAccountStoreConnectionPool")
            .field("database_url", &redact_database_url(&self.database_url))
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

struct PostgresAccountStoreConnection {
    pool: Arc<PostgresAccountStoreConnectionPool>,
    client: Option<Client>,
}

static POSTGRES_ACCOUNT_STORE_POOLS: OnceLock<
    Mutex<BTreeMap<String, Arc<PostgresAccountStoreConnectionPool>>>,
> = OnceLock::new();

fn postgres_account_store_pool(database_url: &str) -> Arc<PostgresAccountStoreConnectionPool> {
    let config = PostgresAccountStorePoolConfig::from_env();
    let key = format!("{database_url}|{}", config.cache_key());
    let mut pools = POSTGRES_ACCOUNT_STORE_POOLS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .expect("postgres account-store pool registry should not be poisoned");
    pools
        .entry(key)
        .or_insert_with(|| {
            Arc::new(PostgresAccountStoreConnectionPool::new(
                database_url.to_string(),
                config,
            ))
        })
        .clone()
}

/// Run a synchronous operation on the same bounded Postgres pool used by the
/// account store. Gateway-side identity data lives in the same database, so
/// sharing this pool avoids opening a new TCP/TLS connection for every login
/// or security-center request. Callers must execute this function from a
/// blocking thread when used by an async runtime.
pub fn with_account_store_postgres_client<T>(
    database_url: &str,
    action: impl FnOnce(&mut Client) -> Result<T, String>,
) -> Result<T, String> {
    if database_url.trim().is_empty() {
        return Err("postgres account-store URL is empty".to_string());
    }
    let pool = postgres_account_store_pool(database_url);
    let mut client = pool.connection()?;
    pool.ensure_migrated(&mut client)?;
    action(&mut client)
}

impl PostgresAccountStoreConnectionPool {
    fn new(database_url: String, config: PostgresAccountStorePoolConfig) -> Self {
        Self {
            database_url,
            config,
            state: Mutex::new(PostgresAccountStorePoolState::default()),
            available: Condvar::new(),
            migration_completed: Mutex::new(false),
        }
    }

    fn connection(self: &Arc<Self>) -> Result<PostgresAccountStoreConnection, String> {
        let deadline = Instant::now() + self.config.wait_timeout;
        let mut state = self
            .state
            .lock()
            .expect("postgres account-store pool mutex should not be poisoned");

        loop {
            if let Some(client) = state.idle.pop() {
                drop(state);
                let mut connection = PostgresAccountStoreConnection {
                    pool: Arc::clone(self),
                    client: Some(client),
                };
                if self.config.test_on_checkout {
                    if connection
                        .client
                        .as_mut()
                        .expect("pooled client should exist")
                        .simple_query("SELECT 1")
                        .is_err()
                    {
                        connection.discard();
                        state = self
                            .state
                            .lock()
                            .expect("postgres account-store pool mutex should not be poisoned");
                        continue;
                    }
                }
                return Ok(connection);
            }

            if state.open < self.config.max_size {
                state.open += 1;
                drop(state);
                return match connect_postgres_account_store_client(
                    &self.database_url,
                    self.config.connect_timeout,
                ) {
                    Ok(client) => Ok(PostgresAccountStoreConnection {
                        pool: Arc::clone(self),
                        client: Some(client),
                    }),
                    Err(error) => {
                        self.decrement_open_connection();
                        Err(error)
                    }
                };
            }

            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(self.timeout_error());
            };
            let (next_state, timeout) = self
                .available
                .wait_timeout(state, remaining)
                .expect("postgres account-store pool condvar should not be poisoned");
            if timeout.timed_out() {
                return Err(self.timeout_error());
            }
            state = next_state;
        }
    }

    fn ensure_migrated(&self, client: &mut Client) -> Result<(), String> {
        let mut migrated = self
            .migration_completed
            .lock()
            .expect("postgres account-store migration mutex should not be poisoned");
        if *migrated {
            return Ok(());
        }
        crate::db_projection::apply_migrations(client)
            .map_err(|error| format!("postgres account-store migration failed: {error}"))?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS game_shop_global_stock (\
                    state_key SMALLINT PRIMARY KEY CHECK (state_key = 1), \
                    purchases_json JSONB NOT NULL DEFAULT '{}'::jsonb, \
                    store_version BIGINT NOT NULL DEFAULT 1, \
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()); \
                 INSERT INTO game_shop_global_stock (state_key) VALUES (1) \
                    ON CONFLICT (state_key) DO NOTHING",
            )
            .map_err(|error| {
                format!("postgres game-shop global-stock migration failed: {error}")
            })?;
        *migrated = true;
        Ok(())
    }

    fn decrement_open_connection(&self) {
        let mut state = self
            .state
            .lock()
            .expect("postgres account-store pool mutex should not be poisoned");
        state.open = state.open.saturating_sub(1);
        self.available.notify_one();
    }

    fn timeout_error(&self) -> String {
        format!(
            "postgres account-store pool exhausted for {} after {}ms (max_size={})",
            redact_database_url(&self.database_url),
            self.config.wait_timeout.as_millis(),
            self.config.max_size
        )
    }
}

impl PostgresAccountStoreConnection {
    fn discard(&mut self) {
        if self.client.take().is_some() {
            self.pool.decrement_open_connection();
        }
    }
}

impl Deref for PostgresAccountStoreConnection {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        self.client
            .as_ref()
            .expect("postgres account-store connection should contain a client")
    }
}

impl DerefMut for PostgresAccountStoreConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.client
            .as_mut()
            .expect("postgres account-store connection should contain a client")
    }
}

impl Drop for PostgresAccountStoreConnection {
    fn drop(&mut self) {
        let Some(client) = self.client.take() else {
            return;
        };
        let mut state = self
            .pool
            .state
            .lock()
            .expect("postgres account-store pool mutex should not be poisoned");
        state.idle.push(client);
        self.pool.available.notify_one();
    }
}

fn validate_postgres_account_store_database_url(database_url: &str) -> Result<(), String> {
    let trimmed = database_url.trim();
    if trimmed.is_empty() {
        return Err("postgres account-store URL is empty".to_string());
    }
    if trimmed != database_url {
        return Err(
            "postgres account-store URL must not contain leading or trailing whitespace"
                .to_string(),
        );
    }
    if database_url.chars().any(char::is_control) {
        return Err("postgres account-store URL must not contain control characters".to_string());
    }
    if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
        return Err(
            "postgres account-store URL must use the postgres:// or postgresql:// scheme"
                .to_string(),
        );
    }
    trimmed
        .parse::<PostgresClientConfig>()
        .map(|_| ())
        .map_err(|error| format!("invalid postgres account-store URL: {error}"))
}

fn connect_postgres_account_store_client(
    database_url: &str,
    connect_timeout: Duration,
) -> Result<Client, String> {
    let mut config = database_url
        .parse::<PostgresClientConfig>()
        .map_err(|error| format!("postgres account-store connect failed: {error}"))?;
    config.connect_timeout(connect_timeout);
    config
        .connect(NoTls)
        .map_err(|error| format!("postgres account-store connect failed: {error}"))
}

fn env_usize(name: &str) -> Option<usize> {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
}

fn env_millis(name: &str) -> Option<Duration> {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_millis)
}

const fn legacy_account_store_schema_version() -> u16 {
    1
}

fn redact_database_url(database_url: &str) -> String {
    let Some((scheme, rest)) = database_url.split_once("://") else {
        return "<configured>".to_string();
    };
    let Some(at_index) = rest.rfind('@') else {
        return format!("{scheme}://<configured>");
    };
    format!("{scheme}://<redacted>@{}", &rest[at_index + 1..])
}

fn write_file_atomically(
    path: &Path,
    data: &[u8],
    fault: Option<AccountStoreFileCommitFault>,
) -> Result<(), AccountStoreFileCommitError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(AccountStoreFileCommitError::not_committed)?;

    let temp_path = atomic_temp_path(path);
    let prepare_result = (|| -> io::Result<()> {
        let mut file = File::create(&temp_path)?;
        file.write_all(data)?;
        file.write_all(b"\n")?;
        file.sync_all()
    })();
    if let Err(error) = prepare_result {
        let _ = fs::remove_file(&temp_path);
        return Err(AccountStoreFileCommitError::not_committed(error));
    }

    if fault == Some(AccountStoreFileCommitFault::BeforeRename) {
        let _ = fs::remove_file(&temp_path);
        return Err(AccountStoreFileCommitError::not_committed(io::Error::new(
            io::ErrorKind::Other,
            "injected failure before account-store rename",
        )));
    }

    match replace_file_atomically_with_retry(&temp_path, path, fault) {
        Ok(()) => Ok(()),
        Err(error) => {
            if !error.is_commit_outcome_unknown() {
                let _ = fs::remove_file(&temp_path);
            }
            Err(error)
        }
    }
}

fn replace_file_atomically_with_retry(
    from: &Path,
    to: &Path,
    fault: Option<AccountStoreFileCommitFault>,
) -> Result<(), AccountStoreFileCommitError> {
    let mut delay = Duration::from_millis(5);
    let mut last_error = None;
    for attempt in 0..8 {
        match replace_file_atomically(from, to, fault) {
            Ok(()) => return Ok(()),
            Err(error) if attempt < 7 && error.is_retryable_not_committed() => {
                last_error = Some(error);
                std::thread::sleep(delay);
                delay = delay.saturating_mul(2);
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error
        .unwrap_or_else(|| AccountStoreFileCommitError::not_committed(io::Error::last_os_error())))
}

fn is_retryable_atomic_replace_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::AlreadyExists | io::ErrorKind::WouldBlock
    )
}

fn atomic_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("account-store.json");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    path.with_file_name(format!(".{file_name}.tmp-{}-{unique}", std::process::id()))
}

#[cfg(windows)]
fn replace_file_atomically(
    from: &Path,
    to: &Path,
    fault: Option<AccountStoreFileCommitFault>,
) -> Result<(), AccountStoreFileCommitError> {
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    let from_wide = wide_null_terminated(from.as_os_str());
    let to_wide = wide_null_terminated(to.as_os_str());
    let ok = unsafe {
        MoveFileExW(
            from_wide.as_ptr(),
            to_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        return Err(AccountStoreFileCommitError::not_committed(
            io::Error::last_os_error(),
        ));
    }
    if fault == Some(AccountStoreFileCommitFault::AfterRenameBeforeDirectorySync) {
        return Err(AccountStoreFileCommitError::commit_outcome_unknown(
            io::Error::new(
                io::ErrorKind::Other,
                "injected failure after account-store publication",
            ),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn wide_null_terminated(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(not(windows))]
fn replace_file_atomically(
    from: &Path,
    to: &Path,
    fault: Option<AccountStoreFileCommitFault>,
) -> Result<(), AccountStoreFileCommitError> {
    fs::rename(from, to).map_err(AccountStoreFileCommitError::not_committed)?;
    if fault == Some(AccountStoreFileCommitFault::AfterRenameBeforeDirectorySync) {
        return Err(AccountStoreFileCommitError::commit_outcome_unknown(
            io::Error::new(
                io::ErrorKind::Other,
                "injected failure after account-store rename before parent directory sync",
            ),
        ));
    }

    // rename only publishes the new directory entry. The snapshot is not
    // considered durable until the containing directory is synced as well.
    // Any failure after rename is therefore outcome-unknown, never retryable.
    let parent = to.parent().unwrap_or_else(|| Path::new("."));
    let directory = File::open(parent).map_err(|error| {
        AccountStoreFileCommitError::commit_outcome_unknown(io::Error::new(
            error.kind(),
            format!(
                "account-store rename succeeded but parent directory {} could not be opened: {error}",
                parent.display()
            ),
        ))
    })?;
    directory.sync_all().map_err(|error| {
        AccountStoreFileCommitError::commit_outcome_unknown(io::Error::new(
            error.kind(),
            format!(
                "account-store rename succeeded but parent directory {} could not be synced: {error}",
                parent.display()
            ),
        ))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    #[serde(default = "default_account_password")]
    pub password: String,
    #[serde(default = "default_storage_size")]
    pub storage_size: u16,
    #[serde(default)]
    pub has_expanded_storage: bool,
    #[serde(default)]
    pub expanded_storage_expiry_time_binary_datetime: i64,
    #[serde(default)]
    pub storage_password: String,
    #[serde(default)]
    pub storage_password_last_set_binary_datetime: i64,
    #[serde(default)]
    pub is_banned: bool,
    #[serde(default)]
    pub ban_reason: String,
    #[serde(default)]
    pub ban_until_ms: Option<u64>,
    #[serde(default)]
    pub banned_at_ms: Option<u64>,
    /// GM rank for in-game `@` command access (0 = normal player). Set by admin
    /// tooling / account provisioning; never granted implicitly.
    #[serde(default)]
    pub gm_level: u8,
    pub characters: Vec<CharacterRecord>,
    pub saves: BTreeMap<i32, CharacterSaveRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBanStatus {
    pub reason: String,
    pub ban_until_ms: Option<u64>,
    pub banned_at_ms: Option<u64>,
}

impl AccountRecord {
    pub fn new(default_character: CharacterRecord) -> Self {
        let mut saves = BTreeMap::new();
        saves.insert(
            default_character.index,
            CharacterSaveRecord::new(default_character.clone()),
        );
        Self {
            password: default_account_password(),
            storage_size: default_storage_size(),
            has_expanded_storage: false,
            expanded_storage_expiry_time_binary_datetime: 0,
            storage_password: String::new(),
            storage_password_last_set_binary_datetime: 0,
            is_banned: false,
            ban_reason: String::new(),
            ban_until_ms: None,
            banned_at_ms: None,
            gm_level: 0,
            characters: vec![default_character],
            saves,
        }
    }

    pub fn empty() -> Self {
        Self {
            password: default_account_password(),
            storage_size: default_storage_size(),
            has_expanded_storage: false,
            expanded_storage_expiry_time_binary_datetime: 0,
            storage_password: String::new(),
            storage_password_last_set_binary_datetime: 0,
            is_banned: false,
            ban_reason: String::new(),
            ban_until_ms: None,
            banned_at_ms: None,
            gm_level: 0,
            characters: Vec::new(),
            saves: BTreeMap::new(),
        }
    }

    pub fn active_ban(&self, now_ms: u64) -> Option<AccountBanStatus> {
        if !self.is_banned {
            return None;
        }
        if self
            .ban_until_ms
            .is_some_and(|ban_until_ms| ban_until_ms <= now_ms)
        {
            return None;
        }
        Some(AccountBanStatus {
            reason: self.ban_reason.clone(),
            ban_until_ms: self.ban_until_ms,
            banned_at_ms: self.banned_at_ms,
        })
    }
}

fn default_account_password() -> String {
    "demo".to_string()
}

const fn default_storage_size() -> u16 {
    80
}

/// Net-new (beyond Crystal) per-city reputation currencies.
///
/// Each town mints its own token, earned via accept-style bounty quests and
/// spendable in player trade + the auction house. `Gold` is folded into the
/// same enum so the trade/auction "currency selector" can name a single value.
/// City balances live in a `BTreeMap<String, u32>` wallet keyed by
/// [`CurrencyKind::city_key`] so adding a new city is a config-only change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CurrencyKind {
    #[default]
    Gold,
    Feitian,
    Bichon,
}

/// Wallet keys for the currently minted city currencies. The browser HUD and
/// the snapshot default every known city to `0` so the UI can render a stable
/// row set even before the player earns anything.
pub const CITY_CURRENCY_KEYS: [&str; 2] = ["feitian", "bichon"];

impl CurrencyKind {
    /// Parse a trade/auction currency argument (case-insensitive, tolerant of
    /// the Chinese city names). Anything unrecognised falls back to gold so the
    /// legacy gold-only commands keep working unchanged.
    pub fn from_arg(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "feitian" | "feitiancity" | "feitian_coin" | "飞天城" | "飞天城币" => {
                Self::Feitian
            }
            "bichon" | "bichoncity" | "bichon_coin" | "比奇城" | "比奇城币" => Self::Bichon,
            _ => Self::Gold,
        }
    }

    /// Wallet key for a city currency, or `None` for gold (which is tracked in
    /// the dedicated `gold` field, not the city wallet).
    pub fn city_key(self) -> Option<&'static str> {
        match self {
            Self::Gold => None,
            Self::Feitian => Some("feitian"),
            Self::Bichon => Some("bichon"),
        }
    }

    /// Build a [`CurrencyKind`] from a wallet key (`None`/unknown => gold).
    pub fn from_city_key(key: &str) -> Self {
        match key {
            "feitian" => Self::Feitian,
            "bichon" => Self::Bichon,
            _ => Self::Gold,
        }
    }

    /// Localized-ish display label (Chinese, matching the in-game city names).
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Gold => "金币",
            Self::Feitian => "飞天城币",
            Self::Bichon => "比奇城币",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSaveRecord {
    /// Optimistic revision for the complete private character snapshot.
    ///
    /// Legacy saves deserialize at revision zero. Every durable character
    /// mutation advances this value so a second session cannot overwrite a
    /// newer transaction with an older full-World snapshot.
    #[serde(default)]
    pub revision: u64,
    pub character: CharacterRecord,
    pub map_file_name: String,
    pub map_title: String,
    pub position: Point,
    pub direction: MirDirection,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    #[serde(default)]
    pub max_mp: i32,
    #[serde(default)]
    pub experience: i64,
    #[serde(default = "default_max_experience")]
    pub max_experience: i64,
    pub gold: u32,
    #[serde(default)]
    pub credit: u32,
    /// Net-new per-city reputation currency wallet, keyed by
    /// [`CurrencyKind::city_key`] (e.g. `"feitian"`, `"bichon"`).
    #[serde(default)]
    pub city_currencies: BTreeMap<String, u32>,
    #[serde(default)]
    pub pk_points: i32,
    #[serde(default)]
    pub chat_banned: bool,
    #[serde(default)]
    pub chat_ban_until_ms: Option<u64>,
    pub inventory_items_json: Vec<String>,
    pub belt_items_json: Vec<String>,
    #[serde(default)]
    pub hero_inventory_items_json: Vec<String>,
    #[serde(default)]
    pub storage_items_json: Vec<String>,
    pub equipment_items_json: Vec<String>,
    #[serde(default)]
    pub equipment_items_explicit_empty: bool,
    pub quest_states_json: Vec<String>,
    pub skill_states_json: Vec<String>,
    #[serde(default)]
    pub npc_flag_states_json: Vec<String>,
    #[serde(default)]
    pub npc_saved_values_json: Vec<String>,
    #[serde(default)]
    pub npc_buy_back_items_json: Vec<String>,
    #[serde(default)]
    pub npc_used_goods_items_json: Vec<String>,
    #[serde(default)]
    pub item_rental_records_json: Vec<String>,
    #[serde(default)]
    pub has_rented_item: bool,
    #[serde(default)]
    pub stage5_systems_json: Option<String>,
}

pub fn crystal_base_vitals(class: MirClass, level: u16) -> (i32, i32) {
    let level = f32::from(level);
    let hp = match class {
        MirClass::Warrior => 14.0 + (level / 4.0 + 4.5 + level / 20.0) * level,
        MirClass::Wizard => 14.0 + (level / 15.0 + 1.8) * level,
        MirClass::Taoist => 14.0 + (level / 6.0 + 2.5) * level,
        MirClass::Assassin | MirClass::Archer => 14.0 + (level / 4.0 + 3.25) * level,
    } as i32;
    let mp = match class {
        MirClass::Wizard => 13.0 + ((level / 5.0 + 2.0) * 2.2 * level),
        MirClass::Taoist => 13.0 + level / 8.0 * 2.2 * level,
        MirClass::Warrior => 11.0 + level * 3.5,
        MirClass::Assassin => 11.0 + level * 5.0,
        MirClass::Archer => 11.0 + level * 4.0,
    } as i32;
    (hp.max(1), mp.max(0))
}

impl CharacterSaveRecord {
    pub fn new(character: CharacterRecord) -> Self {
        let (max_hp, mp) = crystal_base_vitals(character.class, character.level);
        Self {
            revision: 0,
            character,
            map_file_name: String::new(),
            map_title: String::new(),
            position: Point { x: 0, y: 0 },
            direction: MirDirection::Down,
            hp: max_hp,
            max_hp,
            mp,
            max_mp: mp,
            experience: 0,
            max_experience: default_max_experience(),
            gold: 1280,
            credit: 0,
            city_currencies: BTreeMap::new(),
            pk_points: 0,
            chat_banned: false,
            chat_ban_until_ms: None,
            inventory_items_json: Vec::new(),
            belt_items_json: Vec::new(),
            hero_inventory_items_json: Vec::new(),
            storage_items_json: Vec::new(),
            equipment_items_json: Vec::new(),
            equipment_items_explicit_empty: false,
            quest_states_json: Vec::new(),
            skill_states_json: Vec::new(),
            npc_flag_states_json: Vec::new(),
            npc_saved_values_json: Vec::new(),
            npc_buy_back_items_json: Vec::new(),
            npc_used_goods_items_json: Vec::new(),
            item_rental_records_json: Vec::new(),
            has_rented_item: false,
            stage5_systems_json: None,
        }
    }
}

const fn default_max_experience() -> i64 {
    100
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn parses_onchain_mine_nodes_and_skips_malformed_entries() {
        let nodes = parse_onchain_mine_nodes("1:0:335:270:10, 2:3:10:-20:80");
        assert_eq!(
            nodes,
            vec![
                OnchainMineNodeRecord {
                    mine_id: 1,
                    map_file_name: "0".to_string(),
                    x: 335,
                    y: 270,
                    max_stones: 10,
                },
                OnchainMineNodeRecord {
                    mine_id: 2,
                    map_file_name: "3".to_string(),
                    x: 10,
                    y: -20,
                    max_stones: 80,
                },
            ]
        );
        // Malformed entries (missing fields, junk numbers, trailing fields) are skipped.
        assert!(parse_onchain_mine_nodes("nope").is_empty());
        assert!(parse_onchain_mine_nodes("1:0:335:270").is_empty());
        assert!(parse_onchain_mine_nodes("1:0:x:270:10").is_empty());
        assert!(parse_onchain_mine_nodes("1:0:335:270:10:extra").is_empty());
        assert_eq!(parse_onchain_mine_nodes("junk,1:0:1:2:3").len(), 1);
    }

    fn default_character() -> CharacterRecord {
        CharacterRecord {
            index: 0,
            name: "Tester".to_string(),
            level: 1,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        }
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!(
            "mir2-config-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    fn postgres_test_url() -> Option<String> {
        let url = std::env::var("MIR2_TEST_POSTGRES_URL")
            .unwrap_or_else(|_| "postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2".into());
        Client::connect(&url, NoTls).ok().map(|_| url)
    }

    fn unique_account_store(label: &str) -> (String, AccountStore) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let account_id = format!("test-{label}-{}-{unique}", std::process::id());
        let character = CharacterRecord {
            index: 0,
            name: format!("Tester{unique}"),
            level: 1,
            class: MirClass::Warrior,
            gender: MirGender::Male,
        };
        let mut accounts = BTreeMap::new();
        accounts.insert(account_id.clone(), AccountRecord::new(character));
        (
            account_id,
            AccountStore {
                schema_version: ACCOUNT_STORE_SCHEMA_VERSION,
                next_character_index: 1,
                game_shop_global_purchases: BTreeMap::new(),
                accounts,
                source_account_versions: BTreeMap::new(),
                source_save_versions: BTreeMap::new(),
                source_game_shop_global_version: None,
                source_game_shop_global_purchases: BTreeMap::new(),
            },
        )
    }

    #[test]
    fn crystal_map_runtime_drops_starter_demo_transfer() {
        let default_config = SimulationConfig::default();
        assert!(
            default_config
                .map_transfers
                .iter()
                .any(|transfer| transfer.key == "starter-east-field-gate"),
            "the starter demo scenario should keep its explicit gate transfer"
        );

        let crystal_config = SimulationConfig::default().with_crystal_map_runtime();
        assert!(
            crystal_config
                .map_transfers
                .iter()
                .all(|transfer| transfer.key != "starter-east-field-gate"),
            "Crystal runtime should use generated Crystal movement records, not starter demo transfers"
        );

        let crystal_world_config = SimulationConfig::default().with_crystal_world_runtime();
        assert!(
            crystal_world_config
                .map_transfers
                .iter()
                .all(|transfer| transfer.key != "starter-east-field-gate"),
            "Full Crystal world runtime should not retain starter demo transfers either"
        );
        assert!(crystal_world_config.visible_players.is_empty());
        assert!(crystal_world_config.visible_monsters.is_empty());
        assert!(crystal_world_config.visible_npcs.is_empty());
        assert_eq!(crystal_world_config.spawn, Point { x: 288, y: 616 });
    }

    fn cleanup_postgres_account(database_url: &str, account_id: &str) {
        if let Ok(mut client) = Client::connect(database_url, NoTls) {
            for table in [
                "character_items",
                "character_mail",
                "character_npc_state",
                "character_state",
                "character_saves",
                "characters",
            ] {
                let _ = client.execute(
                    &format!("DELETE FROM {table} WHERE account_id = $1"),
                    &[&account_id],
                );
            }
            let _ = client.execute(
                "DELETE FROM auction_listings WHERE seller_account_id = $1",
                &[&account_id],
            );
            let _ = client.execute("DELETE FROM accounts WHERE account_id = $1", &[&account_id]);
        }
    }

    fn with_isolated_account_store_env<T>(
        vars: &[(&str, Option<&str>)],
        action: impl FnOnce() -> T,
    ) -> T {
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock should not be poisoned");
        let names = [
            "MIR2_ACCOUNT_STORE_BACKEND",
            "MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES",
            "MIR2_ACCOUNT_STORE_DATABASE_URL",
            "MIR2_RUNTIME_ENV",
            "MIR2_DEPLOYMENT_ENV",
            "MIR2_ENV",
            "MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE",
            "MIR2_ACCOUNT_STORE_PG_POOL_WAIT_TIMEOUT_MS",
            "MIR2_ACCOUNT_STORE_PG_CONNECT_TIMEOUT_MS",
            "MIR2_ACCOUNT_STORE_PG_POOL_TEST_ON_CHECKOUT",
        ];
        let previous = names.map(|name| (name, std::env::var(name).ok()));
        for name in names {
            std::env::remove_var(name);
        }
        for (name, value) in vars {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }

        let result = action();

        for (name, value) in previous {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        result
    }

    #[test]
    fn legacy_character_save_without_npc_flag_states_uses_default() {
        let store = serde_json::from_str::<AccountStore>(
            r#"{
                "accounts": {
                    "demo": {
                        "characters": [
                            {
                                "index": 0,
                                "name": "Legacy",
                                "level": 7,
                                "class": "Warrior",
                                "gender": "Male"
                            }
                        ],
                        "saves": {
                            "0": {
                                "character": {
                                    "index": 0,
                                    "name": "Legacy",
                                    "level": 7,
                                    "class": "Warrior",
                                    "gender": "Male"
                                },
                                "map_file_name": "0",
                                "map_title": "Bichon",
                                "position": { "x": 330, "y": 270 },
                                "direction": "Down",
                                "hp": 120,
                                "max_hp": 120,
                                "mp": 45,
                                "gold": 1280,
                                "inventory_items_json": [],
                                "belt_items_json": [],
                                "equipment_items_json": [],
                                "quest_states_json": [],
                                "skill_states_json": []
                            }
                        }
                    }
                }
            }"#,
        )
        .expect("legacy save without npc flags should deserialize");

        let save = store
            .accounts
            .get("demo")
            .and_then(|account| account.saves.get(&0))
            .expect("legacy character save should exist");
        assert!(save.npc_flag_states_json.is_empty());
        assert!(save.npc_saved_values_json.is_empty());
        assert!(save.npc_buy_back_items_json.is_empty());
        assert!(save.npc_used_goods_items_json.is_empty());
        assert_eq!(save.experience, 0);
        assert_eq!(save.max_experience, 100);
        let account = store
            .accounts
            .get("demo")
            .expect("legacy account should exist");
        assert_eq!(account.password, "demo");
        assert_eq!(account.storage_size, 80);
        assert!(!account.has_expanded_storage);
        assert_eq!(account.expanded_storage_expiry_time_binary_datetime, 0);
        assert!(account.storage_password.is_empty());
        assert_eq!(account.storage_password_last_set_binary_datetime, 0);
    }

    #[test]
    fn account_store_new_records_current_schema_version() {
        let store = AccountStore::new(default_character());
        assert_eq!(store.schema_version, ACCOUNT_STORE_SCHEMA_VERSION);
    }

    #[test]
    fn account_store_scoping_and_source_merge_preserve_shared_game_shop_state() {
        let mut store = AccountStore::new(default_character());
        store.game_shop_global_purchases.insert(17, 4);
        store.source_game_shop_global_version = Some(8);
        store.source_game_shop_global_purchases.insert(17, 3);

        let scoped = store.scoped_to_account("demo");
        assert_eq!(scoped.game_shop_global_purchases.get(&17), Some(&4));
        assert_eq!(scoped.source_game_shop_global_version, Some(8));
        assert_eq!(scoped.source_game_shop_global_purchases.get(&17), Some(&3));

        store.merge_source_versions(AccountStoreSourceVersions {
            game_shop_global_version: Some(9),
            ..AccountStoreSourceVersions::default()
        });
        assert_eq!(store.source_game_shop_global_version, Some(9));
        assert_eq!(store.source_game_shop_global_purchases.get(&17), Some(&4));
    }

    #[test]
    fn account_store_runtime_backend_defaults_to_file_for_local_development() {
        with_isolated_account_store_env(&[], || {
            assert_eq!(
                account_store_runtime_backend_from_env(),
                Ok(AccountStoreRuntimeBackend::File)
            );
            assert!(!account_store_requires_postgres_source_from_env());
        });
    }

    #[test]
    fn postgres_account_store_pool_config_reads_env() {
        with_isolated_account_store_env(
            &[
                ("MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE", Some("12")),
                ("MIR2_ACCOUNT_STORE_PG_POOL_WAIT_TIMEOUT_MS", Some("750")),
                ("MIR2_ACCOUNT_STORE_PG_CONNECT_TIMEOUT_MS", Some("1250")),
                ("MIR2_ACCOUNT_STORE_PG_POOL_TEST_ON_CHECKOUT", Some("true")),
            ],
            || {
                let config = PostgresAccountStorePoolConfig::from_env();
                assert_eq!(config.max_size, 12);
                assert_eq!(config.wait_timeout, Duration::from_millis(750));
                assert_eq!(config.connect_timeout, Duration::from_millis(1250));
                assert!(config.test_on_checkout);
            },
        );
    }

    #[test]
    fn simulation_config_clone_shares_account_store_persist_lock() {
        let config = SimulationConfig::default();
        let cloned = config.clone();

        assert!(Arc::ptr_eq(
            &config.account_store_persist_lock,
            &cloned.account_store_persist_lock
        ));
    }

    #[test]
    fn account_store_runtime_backend_defaults_to_postgres_for_production() {
        with_isolated_account_store_env(&[("MIR2_RUNTIME_ENV", Some("production"))], || {
            assert_eq!(
                account_store_runtime_backend_from_env(),
                Ok(AccountStoreRuntimeBackend::Postgres)
            );
            assert!(account_store_requires_postgres_source_from_env());
        });
    }

    #[test]
    fn account_store_runtime_backend_rejects_file_backend_in_production() {
        with_isolated_account_store_env(
            &[
                ("MIR2_RUNTIME_ENV", Some("staging")),
                ("MIR2_ACCOUNT_STORE_BACKEND", Some("file")),
            ],
            || {
                let error = account_store_runtime_backend_from_env()
                    .expect_err("production-like file backend should be rejected");
                assert!(error.contains("requires MIR2_ACCOUNT_STORE_BACKEND=postgres"));

                let config_error = match SimulationConfig::default()
                    .with_account_store_environment(".mir2-data/accounts.json")
                {
                    Ok(_) => panic!("production-like file backend should reject config"),
                    Err(error) => error,
                };
                assert!(config_error.contains("requires MIR2_ACCOUNT_STORE_BACKEND=postgres"));
            },
        );
    }

    #[test]
    fn account_store_environment_requires_database_url_for_postgres_source() {
        with_isolated_account_store_env(
            &[("MIR2_ACCOUNT_STORE_BACKEND", Some("postgres"))],
            || {
                let error = match SimulationConfig::default()
                    .with_account_store_environment(".mir2-data/accounts.json")
                {
                    Ok(_) => panic!("postgres account store should require a database url"),
                    Err(error) => error,
                };
                assert!(error.contains("MIR2_ACCOUNT_STORE_DATABASE_URL is required"));
            },
        );
    }

    #[test]
    fn legacy_account_store_without_schema_version_migrates_on_load() {
        let dir = unique_temp_dir("legacy-schema");
        let path = dir.join("accounts.json");
        fs::write(
            &path,
            r#"{
                "accounts": {
                    "demo": {
                        "characters": [
                            {
                                "index": 0,
                                "name": "Legacy",
                                "level": 7,
                                "class": "Warrior",
                                "gender": "Male"
                            }
                        ],
                        "saves": {}
                    }
                }
            }"#,
        )
        .expect("legacy account store fixture should write");

        let store = AccountStore::load_or_new(&path, default_character());

        assert_eq!(store.schema_version, ACCOUNT_STORE_SCHEMA_VERSION);
        assert!(store.accounts.contains_key("demo"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_account_store_loads_default_without_deleting_source() {
        let dir = unique_temp_dir("corrupt-load");
        let path = dir.join("accounts.json");
        fs::write(&path, "{not-json").expect("corrupt account store fixture should write");

        let store = AccountStore::load_or_new(&path, default_character());

        assert_eq!(store.schema_version, ACCOUNT_STORE_SCHEMA_VERSION);
        assert_eq!(
            fs::read_to_string(&path).expect("corrupt source should still exist"),
            "{not-json"
        );
        assert!(store.accounts.contains_key("demo"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn account_store_save_replaces_existing_file_and_cleans_temp_file() {
        let dir = unique_temp_dir("atomic-save");
        let path = dir.join("accounts.json");
        fs::write(&path, r#"{"stale":true}"#).expect("stale file should write");

        let store = AccountStore::new(default_character());
        store
            .save_to_path(&path)
            .expect("account store should save atomically");

        let saved = fs::read_to_string(&path).expect("saved account store should exist");
        assert!(saved.contains(r#""schemaVersion": 2"#));
        assert!(saved.contains(r#""accounts""#));

        let temp_files = fs::read_dir(&dir)
            .expect("temp dir should list")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".accounts.json.tmp-")
            })
            .count();
        assert_eq!(temp_files, 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn file_account_store_repository_loads_saves_and_reports_status() {
        let dir = unique_temp_dir("repository-file");
        let path = dir.join("accounts.json");
        let repository = FileAccountStoreRepository::new(path.clone());
        let mut store = repository
            .load(default_character())
            .expect("file repository should load default store");
        store
            .accounts
            .insert("repo".to_string(), AccountRecord::empty());

        repository
            .save(&store)
            .expect("file repository should save store");
        let reloaded = repository
            .load(default_character())
            .expect("file repository should reload store");
        let status = repository.status();

        assert!(reloaded.accounts.contains_key("repo"));
        assert_eq!(status.backend, "file");
        assert_eq!(status.mode, AccountStoreDatabaseMode::Mirror);
        assert_eq!(status.location, path.display().to_string());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn simulation_config_reports_configured_account_store_repositories() {
        let dir = unique_temp_dir("repository-status");
        let path = dir.join("accounts.json");
        let config = SimulationConfig::default()
            .with_account_store_path(path)
            .with_account_store_database_url("postgres://user:secret@db:5432/mir2");

        let statuses = config.account_store_repository_statuses();

        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].backend, "file");
        assert_eq!(statuses[1].backend, "postgres");
        assert_eq!(statuses[1].mode, AccountStoreDatabaseMode::Mirror);
        assert_eq!(statuses[1].location, "postgres://<redacted>@db:5432/mir2");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn simulation_config_backup_and_restore_roundtrips_account_store() {
        let dir = unique_temp_dir("backup-restore");
        let primary_path = dir.join("accounts.json");
        let backup_path = dir.join("accounts.backup.json");
        let config = SimulationConfig::default().with_account_store_path(primary_path.clone());

        {
            let mut store = config
                .account_store
                .lock()
                .expect("account store mutex should not be poisoned");
            let character = CharacterRecord {
                index: 1,
                name: "BackupBlade".to_string(),
                level: 11,
                class: MirClass::Wizard,
                gender: MirGender::Female,
            };
            store
                .accounts
                .insert("backup".to_string(), AccountRecord::new(character));
        }
        config
            .save_account_store()
            .expect("primary account store should save");
        config
            .backup_account_store(&backup_path)
            .expect("account store backup should save");

        {
            let mut store = config
                .account_store
                .lock()
                .expect("account store mutex should not be poisoned");
            store.accounts.remove("backup");
        }
        config
            .save_account_store()
            .expect("mutated primary account store should save");

        config
            .restore_account_store_from_backup(&backup_path)
            .expect("account store backup should restore");
        let restored_config = SimulationConfig::default().with_account_store_path(primary_path);
        let restored_store = restored_config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        assert!(restored_store.accounts.contains_key("backup"));
        assert!(restored_store
            .accounts
            .get("backup")
            .expect("restored account")
            .characters
            .iter()
            .any(|character| character.name == "BackupBlade"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stage5_system_mail_delivery_persists_to_character_save() {
        let dir = unique_temp_dir("system-mail-delivery");
        let path = dir.join("accounts.json");
        let config = SimulationConfig::default().with_account_store_path(path.clone());

        let receipt = deliver_stage5_system_mail(
            &config,
            Stage5MailDelivery {
                target_kind: Stage5MailTargetKind::Character,
                target_id: "Scout".to_string(),
                from: "GM".to_string(),
                subject: "Compensation".to_string(),
                body: "Thanks for testing.".to_string(),
                gold: 250,
                items: vec!["red-potion".to_string()],
            },
        )
        .expect("mail should deliver");

        assert_eq!(receipt.delivered_count, 1);
        assert_eq!(receipt.mail_ids, vec![1]);

        let saved = AccountStore::load_or_new(&path, default_character());
        let save = saved
            .accounts
            .get("demo")
            .and_then(|account| account.saves.get(&0))
            .expect("demo character save should exist");
        let systems: Stage5SystemsState = serde_json::from_str(
            save.stage5_systems_json
                .as_deref()
                .expect("stage5 systems should be persisted"),
        )
        .expect("stage5 systems should decode");

        assert_eq!(systems.mail.len(), 1);
        assert_eq!(systems.mail[0].from, "GM");
        assert_eq!(systems.mail[0].to, "Scout");
        assert_eq!(systems.mail[0].subject, "Compensation");
        assert_eq!(systems.mail[0].gold, 250);
        assert_eq!(systems.mail[0].items, vec!["red-potion"]);
        assert!(!systems.mail[0].claimed);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn postgres_source_mode_rejects_stale_account_store_writer() {
        let Some(database_url) = postgres_test_url() else {
            eprintln!(
                "skipping postgres source-mode conflict test because Postgres is unavailable"
            );
            return;
        };
        let (account_id, store) = unique_account_store("stale");
        cleanup_postgres_account(&database_url, &account_id);

        let versions = save_account_store_to_postgres(
            database_url.clone(),
            store.clone(),
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("initial source write should succeed");
        let versioned = store.with_source_versions(versions.clone());
        let mut first = versioned.clone();
        let mut second = versioned;

        first
            .accounts
            .get_mut(&account_id)
            .expect("test account should exist")
            .password = "first".to_string();
        let next_versions = save_account_store_to_postgres(
            database_url.clone(),
            first,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("first source writer should succeed");

        second
            .accounts
            .get_mut(&account_id)
            .expect("test account should exist")
            .password = "second".to_string();
        let error = save_account_store_to_postgres(
            database_url.clone(),
            second,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect_err("stale source writer should be rejected");

        assert!(error.contains("stale postgres account-store write"));
        assert_eq!(versions.accounts.get(&account_id), Some(&1));
        assert_eq!(next_versions.accounts.get(&account_id), Some(&2));
        cleanup_postgres_account(&database_url, &account_id);
    }

    #[test]
    fn postgres_source_mode_rejects_stale_character_save_writer() {
        let Some(database_url) = postgres_test_url() else {
            eprintln!(
                "skipping postgres save-version conflict test because Postgres is unavailable"
            );
            return;
        };
        let (account_id, mut store) = unique_account_store("stale-save");
        cleanup_postgres_account(&database_url, &account_id);
        store
            .accounts
            .get_mut(&account_id)
            .and_then(|account| account.saves.get_mut(&0))
            .expect("test save should exist")
            .gold = 100;

        let versions = save_account_store_to_postgres(
            database_url.clone(),
            store.clone(),
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("initial source write should succeed");
        let versioned = store.with_source_versions(versions.clone());
        let mut first = versioned.clone();
        let mut second = versioned;

        first
            .accounts
            .get_mut(&account_id)
            .and_then(|account| account.saves.get_mut(&0))
            .expect("first save should exist")
            .gold = 200;
        let next_versions = save_account_store_to_postgres(
            database_url.clone(),
            first,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("first source writer should succeed");

        second.source_account_versions = next_versions.accounts.clone();
        second
            .accounts
            .get_mut(&account_id)
            .and_then(|account| account.saves.get_mut(&0))
            .expect("second save should exist")
            .gold = 300;
        let error = save_account_store_to_postgres(
            database_url.clone(),
            second,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect_err("stale save writer should be rejected");

        assert!(error.contains("stale postgres character-save write"));
        assert_eq!(
            versions
                .saves
                .get(&account_id)
                .and_then(|saves| saves.get(&0)),
            Some(&1)
        );
        assert_eq!(
            next_versions
                .saves
                .get(&account_id)
                .and_then(|saves| saves.get(&0)),
            Some(&2)
        );
        cleanup_postgres_account(&database_url, &account_id);
    }

    fn projection_sample_save(save: &mut CharacterSaveRecord, owner_name: &str) {
        save.gold = 7777;
        save.credit = 12;
        save.map_file_name = "3".to_string();
        save.position = Point { x: 100, y: 200 };
        save.inventory_items_json = vec![
            r#"{"key":"red-potion","name":"Red Potion","icon":1,"slot":0,"unique_id":101,"container":"Bag1","quantity":20,"description":"","durability_current":null,"durability_max":null,"weight":1,"equip_slot":null,"grade":"Common","attack":0,"defence":0,"heal_hp":50,"heal_mp":0}"#.to_string(),
            r#"{"key":"iron-sword","name":"Iron Sword","icon":2,"slot":1,"unique_id":102,"container":"Bag1","quantity":1,"description":"","durability_current":35,"durability_max":40,"weight":10,"equip_slot":null,"grade":"Rare","attack":7,"defence":0,"heal_hp":0,"heal_mp":0}"#.to_string(),
        ];
        let mut systems = Stage5SystemsState::default();
        systems.guild.name = "Crimson".to_string();
        systems.mail.push(Stage5MailMessage {
            id: 9,
            delivery_nonce: "fixture-mail-9".to_string(),
            from: "GM".to_string(),
            to: owner_name.to_string(),
            subject: "Reward".to_string(),
            body: String::new(),
            gold: 500,
            items: Vec::new(),
            item_states_json: Vec::new(),
            opened: false,
            locked: false,
            claimed: false,
            deleted: false,
        });
        systems.auction.push(Stage5AuctionListing {
            id: 3,
            seller: owner_name.to_string(),
            item_key: "iron-sword".to_string(),
            price: 999,
            currency: CurrencyKind::Gold,
            sold: false,
            cancelled: false,
            expired: false,
        });
        save.stage5_systems_json =
            Some(serde_json::to_string(&systems).expect("systems serialize"));
    }

    #[test]
    fn postgres_save_projects_normalized_rows() {
        let Some(database_url) = postgres_test_url() else {
            eprintln!("skipping postgres projection test because Postgres is unavailable");
            return;
        };
        let (account_id, mut store) = unique_account_store("projection");
        cleanup_postgres_account(&database_url, &account_id);
        let owner_name = store.accounts[&account_id].characters[0].name.clone();
        projection_sample_save(
            store
                .accounts
                .get_mut(&account_id)
                .and_then(|account| account.saves.get_mut(&0))
                .expect("test save should exist"),
            &owner_name,
        );

        save_account_store_to_postgres(
            database_url.clone(),
            store,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("source write should succeed and project rows");

        let mut client = Client::connect(&database_url, NoTls).expect("connect to assert");
        let state = client
            .query_one(
                "SELECT gold, credit, guild_name, inventory_count, mail_count, \
                 unclaimed_mail_count, active_auction_count, map_file_name, save_version \
                 FROM character_state WHERE account_id = $1 AND character_index = 0",
                &[&account_id],
            )
            .expect("character_state row should exist");
        assert_eq!(state.get::<_, i64>("gold"), 7777);
        assert_eq!(state.get::<_, i64>("credit"), 12);
        assert_eq!(state.get::<_, String>("guild_name"), "Crimson");
        assert_eq!(state.get::<_, i32>("inventory_count"), 2);
        assert_eq!(state.get::<_, i32>("mail_count"), 1);
        assert_eq!(state.get::<_, i32>("unclaimed_mail_count"), 1);
        assert_eq!(state.get::<_, i32>("active_auction_count"), 1);
        assert_eq!(state.get::<_, String>("map_file_name"), "3");
        assert_eq!(state.get::<_, i64>("save_version"), 1);

        let items: i64 = client
            .query_one(
                "SELECT count(*) FROM character_items WHERE account_id = $1",
                &[&account_id],
            )
            .expect("item count")
            .get(0);
        assert_eq!(items, 2);
        let sword = client
            .query_one(
                "SELECT quantity, unique_id, durability_current FROM character_items \
                 WHERE account_id = $1 AND item_key = 'iron-sword'",
                &[&account_id],
            )
            .expect("sword row");
        assert_eq!(sword.get::<_, i64>("quantity"), 1);
        assert_eq!(sword.get::<_, i64>("unique_id"), 102);
        assert_eq!(sword.get::<_, Option<i32>>("durability_current"), Some(35));

        let mail = client
            .query_one(
                "SELECT sender, gold, claimed FROM character_mail \
                 WHERE account_id = $1 AND mail_id = 9",
                &[&account_id],
            )
            .expect("mail row");
        assert_eq!(mail.get::<_, String>("sender"), "GM");
        assert_eq!(mail.get::<_, i64>("gold"), 500);
        assert!(!mail.get::<_, bool>("claimed"));

        let auction_active: bool = client
            .query_one(
                "SELECT active FROM auction_listings \
                 WHERE seller_account_id = $1 AND listing_id = 3",
                &[&account_id],
            )
            .expect("auction row")
            .get("active");
        assert!(auction_active);

        cleanup_postgres_account(&database_url, &account_id);
    }

    #[test]
    fn postgres_projection_reflects_item_removal() {
        let Some(database_url) = postgres_test_url() else {
            eprintln!("skipping postgres projection-shrink test because Postgres is unavailable");
            return;
        };
        let (account_id, mut store) = unique_account_store("projection-shrink");
        cleanup_postgres_account(&database_url, &account_id);
        let owner_name = store.accounts[&account_id].characters[0].name.clone();
        projection_sample_save(
            store
                .accounts
                .get_mut(&account_id)
                .and_then(|account| account.saves.get_mut(&0))
                .expect("test save should exist"),
            &owner_name,
        );

        let versions = save_account_store_to_postgres(
            database_url.clone(),
            store.clone(),
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("initial source write should succeed");

        let mut shrunk = store.with_source_versions(versions);
        {
            let save = shrunk
                .accounts
                .get_mut(&account_id)
                .and_then(|account| account.saves.get_mut(&0))
                .expect("save should exist");
            save.inventory_items_json.clear();
            let mut systems = Stage5SystemsState::default();
            // Mark the auction sold so it should no longer be "active".
            systems.auction.push(Stage5AuctionListing {
                id: 3,
                seller: owner_name.clone(),
                item_key: "iron-sword".to_string(),
                price: 999,
                currency: CurrencyKind::Gold,
                sold: true,
                cancelled: false,
                expired: false,
            });
            save.stage5_systems_json =
                Some(serde_json::to_string(&systems).expect("systems serialize"));
        }
        save_account_store_to_postgres(
            database_url.clone(),
            shrunk,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("second source write should succeed");

        let mut client = Client::connect(&database_url, NoTls).expect("connect to assert");
        let items: i64 = client
            .query_one(
                "SELECT count(*) FROM character_items WHERE account_id = $1",
                &[&account_id],
            )
            .expect("item count")
            .get(0);
        assert_eq!(
            items, 0,
            "removed items must not leave stale projection rows"
        );
        let active: i64 = client
            .query_one(
                "SELECT count(*) FROM auction_listings \
                 WHERE seller_account_id = $1 AND active",
                &[&account_id],
            )
            .expect("active auction count")
            .get(0);
        assert_eq!(active, 0, "sold auction must flip to inactive");
        let state_auctions: i32 = client
            .query_one(
                "SELECT active_auction_count FROM character_state \
                 WHERE account_id = $1 AND character_index = 0",
                &[&account_id],
            )
            .expect("state row")
            .get(0);
        assert_eq!(state_auctions, 0);

        cleanup_postgres_account(&database_url, &account_id);
    }

    #[test]
    fn postgres_projection_drops_deleted_character_rows() {
        let Some(database_url) = postgres_test_url() else {
            eprintln!("skipping postgres deleted-character projection test (Postgres unavailable)");
            return;
        };
        let (account_id, mut store) = unique_account_store("projection-delete");
        cleanup_postgres_account(&database_url, &account_id);
        let owner_name = store.accounts[&account_id].characters[0].name.clone();
        projection_sample_save(
            store
                .accounts
                .get_mut(&account_id)
                .and_then(|account| account.saves.get_mut(&0))
                .expect("test save should exist"),
            &owner_name,
        );
        // Add a second character so the account is not emptied by the delete.
        {
            let account = store.accounts.get_mut(&account_id).expect("account");
            let mut second = CharacterRecord {
                index: 1,
                name: format!("{owner_name}-2"),
                level: 1,
                class: MirClass::Taoist,
                gender: MirGender::Male,
            };
            second.index = 1;
            account.characters.push(second.clone());
            account.saves.insert(1, CharacterSaveRecord::new(second));
        }

        let versions = save_account_store_to_postgres(
            database_url.clone(),
            store.clone(),
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("initial source write should succeed");

        let mut client = Client::connect(&database_url, NoTls).expect("connect to assert");
        let before: i64 = client
            .query_one(
                "SELECT count(*) FROM character_state WHERE account_id = $1",
                &[&account_id],
            )
            .expect("state count")
            .get(0);
        assert_eq!(before, 2);

        // Delete character 0 (with all its items/mail/auctions) and re-save.
        let mut pruned = store.with_source_versions(versions);
        {
            let account = pruned.accounts.get_mut(&account_id).expect("account");
            account.characters.retain(|character| character.index != 0);
            account.saves.remove(&0);
        }
        save_account_store_to_postgres(
            database_url.clone(),
            pruned,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("prune source write should succeed");

        let after_state: i64 = client
            .query_one(
                "SELECT count(*) FROM character_state WHERE account_id = $1",
                &[&account_id],
            )
            .expect("state count")
            .get(0);
        assert_eq!(
            after_state, 1,
            "deleted character must drop its character_state row"
        );
        let after_items: i64 = client
            .query_one(
                "SELECT count(*) FROM character_items WHERE account_id = $1 AND character_index = 0",
                &[&account_id],
            )
            .expect("item count")
            .get(0);
        assert_eq!(after_items, 0, "deleted character must drop its item rows");
        let after_auctions: i64 = client
            .query_one(
                "SELECT count(*) FROM auction_listings \
                 WHERE seller_account_id = $1 AND seller_character_index = 0",
                &[&account_id],
            )
            .expect("auction count")
            .get(0);
        assert_eq!(
            after_auctions, 0,
            "deleted character must drop its auction rows"
        );
        let saves: i64 = client
            .query_one(
                "SELECT count(*) FROM character_saves WHERE account_id = $1 AND character_index = 0",
                &[&account_id],
            )
            .expect("save count")
            .get(0);
        assert_eq!(
            saves, 0,
            "deleted character must drop its character_saves mirror row"
        );

        cleanup_postgres_account(&database_url, &account_id);
    }

    #[test]
    fn postgres_source_mode_reload_can_save_after_version_refresh() {
        let Some(database_url) = postgres_test_url() else {
            eprintln!("skipping postgres source-mode reload test because Postgres is unavailable");
            return;
        };
        let (account_id, mut store) = unique_account_store("reload");
        cleanup_postgres_account(&database_url, &account_id);
        let save = store
            .accounts
            .get_mut(&account_id)
            .and_then(|account| account.saves.get_mut(&0))
            .expect("test save should exist");
        save.gold = 100;

        let versions = save_account_store_to_postgres(
            database_url.clone(),
            store,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("initial source write should succeed");
        assert_eq!(
            versions
                .saves
                .get(&account_id)
                .and_then(|saves| saves.get(&0)),
            Some(&1)
        );

        let mut reloaded =
            load_account_store_from_postgres(database_url.clone(), default_character())
                .expect("source store should reload with versions");
        reloaded
            .accounts
            .retain(|loaded_account_id, _| loaded_account_id == &account_id);
        reloaded
            .accounts
            .get_mut(&account_id)
            .and_then(|account| account.saves.get_mut(&0))
            .expect("reloaded save should exist")
            .gold = 250;

        let next_versions = save_account_store_to_postgres(
            database_url.clone(),
            reloaded,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("reloaded source writer should succeed");

        assert_eq!(
            next_versions
                .saves
                .get(&account_id)
                .and_then(|saves| saves.get(&0)),
            Some(&2)
        );
        let confirmed = load_account_store_from_postgres(database_url.clone(), default_character())
            .expect("source store should load final state");
        let confirmed_gold = confirmed
            .accounts
            .get(&account_id)
            .and_then(|account| account.saves.get(&0))
            .map(|save| save.gold);
        assert_eq!(confirmed_gold, Some(250));
        cleanup_postgres_account(&database_url, &account_id);
    }

    #[test]
    fn postgres_source_mode_account_scoped_save_does_not_rewrite_other_accounts() {
        let Some(database_url) = postgres_test_url() else {
            eprintln!("skipping postgres scoped-save test because Postgres is unavailable");
            return;
        };
        let (first_account_id, mut store) = unique_account_store("scoped-first");
        let (second_account_id, second_store) = unique_account_store("scoped-second");
        cleanup_postgres_account(&database_url, &first_account_id);
        cleanup_postgres_account(&database_url, &second_account_id);
        store.accounts.extend(second_store.accounts);

        let versions = save_account_store_to_postgres(
            database_url.clone(),
            store.clone(),
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("initial source write should succeed");
        let mut config = SimulationConfig::default();
        config.account_store = Arc::new(Mutex::new(store.with_source_versions(versions)));
        config.account_store_database_url = Some(database_url.clone());
        config.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;

        {
            let mut live_store = config
                .account_store
                .lock()
                .expect("account store mutex should not be poisoned");
            live_store
                .accounts
                .get_mut(&first_account_id)
                .expect("first account should exist")
                .password = "scoped-update".to_string();
        }

        config
            .save_account_store_account(&first_account_id)
            .expect("account-scoped source write should succeed");

        let mut client = Client::connect(&database_url, NoTls).expect("postgres should connect");
        let first_version: i64 = client
            .query_one(
                "SELECT store_version FROM accounts WHERE account_id = $1",
                &[&first_account_id],
            )
            .expect("first account version should load")
            .get("store_version");
        let second_version: i64 = client
            .query_one(
                "SELECT store_version FROM accounts WHERE account_id = $1",
                &[&second_account_id],
            )
            .expect("second account version should load")
            .get("store_version");

        assert_eq!(first_version, 2);
        assert_eq!(second_version, 1);
        cleanup_postgres_account(&database_url, &first_account_id);
        cleanup_postgres_account(&database_url, &second_account_id);
    }

    #[test]
    fn postgres_multi_account_persist_failure_rolls_back_every_account() {
        let Some(database_url) = postgres_test_url() else {
            eprintln!(
                "skipping postgres multi-account rollback test because Postgres is unavailable"
            );
            return;
        };
        let (sender_account_id, mut store) = unique_account_store("mail-atomic-a-sender");
        let (recipient_account_id, recipient_store) =
            unique_account_store("mail-atomic-z-recipient");
        cleanup_postgres_account(&database_url, &sender_account_id);
        cleanup_postgres_account(&database_url, &recipient_account_id);
        store.accounts.extend(recipient_store.accounts);

        let initial_versions = save_account_store_to_postgres(
            database_url.clone(),
            store.clone(),
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect("initial multi-account source write should succeed");
        let initial_sender_gold = store
            .accounts
            .get(&sender_account_id)
            .and_then(|account| account.saves.get(&0))
            .expect("sender save should exist")
            .gold;
        let mut staged = store.with_source_versions(initial_versions);
        staged
            .accounts
            .get_mut(&sender_account_id)
            .and_then(|account| account.saves.get_mut(&0))
            .expect("sender save should exist")
            .gold = 17;
        staged
            .accounts
            .get_mut(&recipient_account_id)
            .and_then(|account| account.saves.get_mut(&0))
            .expect("recipient save should exist")
            .stage5_systems_json = Some("{malformed-mail-state".to_string());

        let error = save_account_store_to_postgres(
            database_url.clone(),
            staged,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .expect_err("the malformed recipient save must abort the whole transaction");
        assert!(error.contains("stage5 systems json decode failed"));

        let confirmed = load_account_store_from_postgres(database_url.clone(), default_character())
            .expect("postgres account store should reload after rollback");
        assert_eq!(
            confirmed
                .accounts
                .get(&sender_account_id)
                .and_then(|account| account.saves.get(&0))
                .map(|save| save.gold),
            Some(initial_sender_gold)
        );
        assert_eq!(
            confirmed
                .accounts
                .get(&recipient_account_id)
                .and_then(|account| account.saves.get(&0))
                .and_then(|save| save.stage5_systems_json.as_deref()),
            None
        );

        let mut client = Client::connect(&database_url, NoTls).expect("postgres should connect");
        for account_id in [&sender_account_id, &recipient_account_id] {
            let version: i64 = client
                .query_one(
                    "SELECT store_version FROM accounts WHERE account_id = $1",
                    &[account_id],
                )
                .expect("account version should load")
                .get("store_version");
            assert_eq!(version, 1);
        }

        cleanup_postgres_account(&database_url, &sender_account_id);
        cleanup_postgres_account(&database_url, &recipient_account_id);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisiblePlayerRecord {
    pub object_id: u32,
    pub name: String,
    pub class: MirClass,
    pub gender: MirGender,
    pub level: u16,
    pub armour_shape: Option<u16>,
    pub weapon_shape: Option<u16>,
    pub position: Point,
    pub direction: MirDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleMonsterRecord {
    pub object_id: u32,
    pub name: String,
    pub image: u16,
    pub position: Point,
    pub direction: MirDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleNpcRecord {
    pub object_id: u32,
    pub name: String,
    pub image: u16,
    pub colour_argb: i32,
    pub position: Point,
    pub direction: MirDirection,
    pub quest_ids: Vec<i32>,
    pub script_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapTransferRecord {
    pub key: String,
    pub from_map_file_name: String,
    pub from_bounds: MapBounds,
    pub to_map_file_name: String,
    pub to_map_title: String,
    pub to_position: Point,
    pub to_direction: MirDirection,
    /// Crystal `MovementInfo.ConquestIndex`. `0` means an ordinary movement;
    /// `> 0` means the movement only fires for a player whose guild owns the
    /// conquest with that index.
    pub conquest_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeZoneRecord {
    pub map_file_name: String,
    pub bounds: MapBounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapDropRuleRecord {
    pub map_file_name: String,
    pub no_town_teleport: bool,
    pub no_escape: bool,
    pub no_random: bool,
    pub no_drug: bool,
    pub no_reincarnation: bool,
    pub no_throw_item: bool,
    pub no_drop_player: bool,
    pub no_drop_monster: bool,
    pub no_mount: bool,
    pub no_hero: bool,
    pub need_bridle: bool,
}

/// A rectangular mining zone on a map (Crystal `MapInfo.MineZones`). Every cell
/// within `size` tiles of `(x, y)` becomes a mineable spot served by the given
/// built-in mine set (`mine_set` is 1-based, matching Crystal's `MineIndex`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MineZoneRecord {
    pub map_file_name: String,
    pub mine_set: u8,
    pub x: i32,
    pub y: i32,
    pub size: u16,
}

/// A single ON-CHAIN mine vein rendered in-world (M4, WF-6 — DESIGN §4-⑥). Maps the
/// Sui contract's `mine_id` to a map cell so chain-confirmed settlements can drive the
/// vein's `MineNodeState` stage. Deliberately NOT a `MineZoneRecord`: the cell must not
/// become a P0 mineable spot, or the server would also roll ore there (double payout —
/// the chain is the only payout source for these veins).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnchainMineNodeRecord {
    /// The contract-side mine id (`mir2_mine` schema key).
    pub mine_id: u64,
    pub map_file_name: String,
    pub x: i32,
    pub y: i32,
    /// Full stones capacity (mirrors the on-chain `MineConfig.max_stones`) for staging.
    pub max_stones: u32,
}

/// Parse `MIR2_ONCHAIN_MINE_NODES` — comma-separated `mine_id:map:x:y:max_stones`
/// entries (e.g. `1:0:335:270:10`). Env-gated so deployments WITHOUT the on-chain
/// stack render no ghost veins; the M4 e2e runbook sets it on the local gateway.
/// Malformed entries are skipped.
pub(crate) fn onchain_mine_nodes_from_env() -> Vec<OnchainMineNodeRecord> {
    let Ok(raw) = std::env::var("MIR2_ONCHAIN_MINE_NODES") else {
        return Vec::new();
    };
    parse_onchain_mine_nodes(&raw)
}

fn parse_onchain_mine_nodes(raw: &str) -> Vec<OnchainMineNodeRecord> {
    raw.split(',')
        .filter_map(|entry| {
            let mut parts = entry.trim().split(':');
            let mine_id = parts.next()?.trim().parse().ok()?;
            let map_file_name = parts.next()?.trim();
            if map_file_name.is_empty() {
                return None;
            }
            let x = parts.next()?.trim().parse().ok()?;
            let y = parts.next()?.trim().parse().ok()?;
            let max_stones = parts.next()?.trim().parse().ok()?;
            if parts.next().is_some() {
                return None;
            }
            Some(OnchainMineNodeRecord {
                mine_id,
                map_file_name: map_file_name.to_string(),
                x,
                y,
                max_stones,
            })
        })
        .collect()
}

/// Environmental hazard flags for a map (Crystal `MapInfo.Lightning/Fire` and
/// their damage caps). Hazards periodically strike players on the map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapHazardRecord {
    pub map_file_name: String,
    pub lightning: bool,
    pub fire: bool,
    pub lightning_damage: i32,
    pub fire_damage: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterSpawnSource {
    StarterScenario,
    CrystalStarterRegion,
    /// Activate the full Crystal world: every map a player enters spawns its
    /// entire Crystal respawn set (not just the collision-bounded starter
    /// slice), and all maps are reachable via the manifest's movements. Maps
    /// with no player on them stay dormant — nothing spawns or ticks until a
    /// player arrives, matching Crystal's "load the world, run what's occupied"
    /// behaviour without keeping all ~76k monsters alive at once.
    CrystalWorld,
}

impl MonsterSpawnSource {
    /// Both Crystal sources drive the world from the per-map respawn manifest
    /// (titles, collision, and current-map spawns), unlike the hand-authored
    /// `StarterScenario`. `CrystalWorld` additionally spawns the whole map.
    pub(crate) fn uses_crystal_current_map(self) -> bool {
        matches!(self, Self::CrystalStarterRegion | Self::CrystalWorld)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountStoreDatabaseMode {
    Mirror,
    SourceOfTruth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStoreRuntimeBackend {
    File,
    Postgres,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountSourceRefreshOutcome {
    NotRequired,
    Refreshed,
    Missing,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStoreTransactionFault {
    BeforePersist,
    Persist,
    BeforeFileRename,
    AfterFileRenameBeforeDirectorySync,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountStoreTransactionScopeObservation {
    AccountOnly,
    WithGlobal,
    FullRestore,
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct AccountStoreRepositoryWriterProbe {
    invocations: usize,
    last_plan_includes_global: Option<bool>,
    outcome: Result<AccountStoreRepositorySave, String>,
}

const ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN: &str = "ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN";

#[derive(Debug, Clone, PartialEq, Eq)]
enum AccountStoreWriteState {
    Writable,
    Frozen { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentLevelRate {
    pub min_level: u16,
    pub max_level: u16,
    pub multiplier: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRatePolicy {
    pub label: String,
    pub monster_experience_tiers: Vec<ContentLevelRate>,
    pub gold_multiplier: u32,
    pub drop_multiplier: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentProfileRuntime {
    pub profile_id: String,
    pub version: u32,
    pub acceptance_level: u16,
    pub source: String,
    pub rate_policy: ContentRatePolicy,
    pub bundle_hash: String,
    pub bundle_built_at: String,
    pub crystal_database_version: i32,
    pub crystal_database_custom_version: i32,
    pub profile: ContentProfile,
}

impl ContentProfileRuntime {
    pub fn platinum_176() -> Self {
        let profile = platinum_176_profile();
        let bundle = platinum_176_profile_bundle();
        debug_assert!(validate_content_profile(&profile).is_ok());
        Self {
            profile_id: profile.profile_id.clone(),
            version: profile.version,
            acceptance_level: profile.acceptance_level,
            source: profile.source.clone(),
            rate_policy: ContentRatePolicy {
                label: profile.rate_policy.label.clone(),
                monster_experience_tiers: profile
                    .rate_policy
                    .monster_experience_tiers
                    .iter()
                    .map(|tier| ContentLevelRate {
                        min_level: tier.min_level,
                        max_level: tier.max_level,
                        multiplier: u32::from(tier.multiplier),
                    })
                    .collect(),
                gold_multiplier: u32::from(profile.rate_policy.gold_multiplier),
                drop_multiplier: u32::from(profile.rate_policy.drop_multiplier),
            },
            bundle_hash: bundle.content_hash,
            bundle_built_at: bundle.built_at,
            crystal_database_version: bundle.source_data.crystal_database_version,
            crystal_database_custom_version: bundle.source_data.crystal_database_custom_version,
            profile,
        }
    }

    pub fn monster_experience_multiplier(&self, level: u16) -> u32 {
        self.rate_policy
            .monster_experience_tiers
            .iter()
            .find(|tier| tier.min_level <= level && level <= tier.max_level)
            .map(|tier| tier.multiplier)
            .unwrap_or(1)
            .max(1)
    }

    pub fn experience_required_for_level(&self, level: u16) -> i64 {
        content_profile_experience_required(&self.profile, level)
            .or_else(|| {
                self.profile
                    .experience_curve
                    .last()
                    .map(|entry| entry.required_experience)
            })
            .unwrap_or(100)
            .max(1)
    }

    pub fn class_is_allowed(&self, class: MirClass) -> bool {
        self.profile.allowed_classes.contains(&class)
    }

    pub fn map_is_allowed(&self, file_name: &str) -> bool {
        let requested = normalize_content_map_file_name(file_name);
        self.profile
            .map_whitelist
            .iter()
            .any(|map| normalize_content_map_file_name(&map.file_name) == requested)
    }

    pub fn monster_is_allowed(&self, monster_name: &str) -> bool {
        self.profile
            .monster_whitelist
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(monster_name))
    }

    pub fn crystal_respawns_for_map(&self, map_file_name: &str) -> Vec<CrystalRespawnTemplate> {
        let mut respawns = crystal_map_respawns_by_file_name(map_file_name)
            .map(|map| map.respawns)
            .unwrap_or_default();
        respawns.extend(content_profile_respawn_overrides_for_map(
            &self.profile,
            map_file_name,
        ));
        respawns
    }

    pub fn monster_respawn_random_delay_minutes(
        &self,
        monster_name: &str,
        imported_random_delay_minutes: u16,
    ) -> u16 {
        if content_profile_monster_is_boss(&self.profile, monster_name) {
            imported_random_delay_minutes.max(self.profile.boss_respawn_jitter_minutes)
        } else {
            imported_random_delay_minutes
        }
    }

    pub fn item_is_allowed(&self, item_name: &str) -> bool {
        self.profile
            .item_whitelist
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(item_name))
    }

    pub fn npc_script_is_allowed(&self, script_key: &str) -> bool {
        let requested = normalize_content_npc_script_key(script_key);
        self.profile
            .npc_script_whitelist
            .iter()
            .any(|allowed| normalize_content_npc_script_key(allowed) == requested)
    }

    pub fn content_skill_rule(&self, spell: &str) -> Option<&ContentSkillRule> {
        self.profile.skills.iter().find(|rule| {
            normalize_content_identifier(&rule.spell) == normalize_content_identifier(spell)
        })
    }

    pub fn skill_is_allowed(&self, spell: &str, class: MirClass, level: u16) -> bool {
        self.content_skill_rule(spell)
            .is_some_and(|rule| rule.class == class && level >= rule.required_level)
    }

    pub fn stage5_action_is_allowed(&self, action: &str) -> bool {
        !self
            .profile
            .disabled_stage5_action_prefixes
            .iter()
            .any(|prefix| action.starts_with(prefix))
    }
}

#[cfg(test)]
mod content_profile_runtime_tests {
    use super::ContentProfileRuntime;

    #[test]
    fn platinum_176_rate_tiers_cover_the_acceptance_path() {
        let profile = ContentProfileRuntime::platinum_176();

        assert_eq!(profile.monster_experience_multiplier(1), 2);
        assert_eq!(profile.monster_experience_multiplier(21), 2);
        assert_eq!(profile.monster_experience_multiplier(22), 3);
        assert_eq!(profile.monster_experience_multiplier(35), 3);
        assert_eq!(profile.monster_experience_multiplier(36), 4);
        assert_eq!(profile.monster_experience_multiplier(50), 4);
        assert_eq!(profile.monster_experience_multiplier(51), 1);
        assert_eq!(profile.rate_policy.gold_multiplier, 1);
        assert_eq!(profile.rate_policy.drop_multiplier, 1);
    }
}

pub fn account_store_runtime_backend_from_env() -> Result<AccountStoreRuntimeBackend, String> {
    let backend = env::var("MIR2_ACCOUNT_STORE_BACKEND").unwrap_or_default();
    let backend = backend.trim().to_ascii_lowercase();
    let production_like = account_store_requires_postgres_source_from_env();

    match backend.as_str() {
        "postgres" | "source" | "source-of-truth" | "source_of_truth" => {
            Ok(AccountStoreRuntimeBackend::Postgres)
        }
        "" if production_like => Ok(AccountStoreRuntimeBackend::Postgres),
        "" | "json" | "file" | "mirror" => {
            if production_like {
                Err("MIR2_RUNTIME_ENV/MIR2_DEPLOYMENT_ENV requires MIR2_ACCOUNT_STORE_BACKEND=postgres".to_string())
            } else {
                Ok(AccountStoreRuntimeBackend::File)
            }
        }
        other => Err(format!("unsupported MIR2_ACCOUNT_STORE_BACKEND: {other}")),
    }
}

pub fn account_store_requires_postgres_source_from_env() -> bool {
    if env_flag_enabled("MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES") {
        return true;
    }
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

fn env_flag_enabled(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// `MIR2_GM_ACCOUNTS` — a comma-separated allowlist of account names granted
/// in-game GM rank for the session. Applied at login (StartGame) on top of the
/// stored `gm_level` without mutating the persisted record, so it is a safe
/// local/dev convenience: e.g. `MIR2_GM_ACCOUNTS=demo`. Production should set
/// `gm_level` on the account record instead. Matching is case-insensitive.
pub fn account_is_env_gm(account_id: &str) -> bool {
    env::var("MIR2_GM_ACCOUNTS")
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .any(|name| name.eq_ignore_ascii_case(account_id))
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone)]
pub struct SimulationConfig {
    pub map: MapInformation,
    pub spawn: Point,
    pub scene_view: SceneView,
    pub map_collision: StarterMapCollision,
    pub require_storage_password: bool,
    pub monster_spawn_source: MonsterSpawnSource,
    pub terrain_patches: Vec<TerrainPatchTemplate>,
    pub decor_objects: Vec<DecorObjectTemplate>,
    pub default_character: CharacterRecord,
    pub object_id: u32,
    pub real_id: u32,
    pub visible_players: Vec<VisiblePlayerRecord>,
    pub group_member_object_ids: Vec<u32>,
    pub visible_monsters: Vec<VisibleMonsterRecord>,
    pub visible_npcs: Vec<VisibleNpcRecord>,
    pub conquest_wars: BTreeMap<i32, bool>,
    /// Conquest index → name of the guild that currently owns it. Gates
    /// conquest movements (Crystal `MyGuild.Conquest.Info.Index`).
    pub conquest_owners: BTreeMap<i32, String>,
    pub map_transfers: Vec<MapTransferRecord>,
    pub safe_zones: Vec<SafeZoneRecord>,
    /// Crystal's optional `Settings.SafeZoneBorder` presentation switch.
    ///
    /// The imported map manifest retains the decorative TrapHexagon objects so
    /// deployments that explicitly enable the option can reproduce Crystal,
    /// but Crystal defaults this setting to false. Keeping the switch in the
    /// runtime config prevents those map decorations from being confused with
    /// real player-cast TrapHexagon `ObjectSpell` packets.
    pub safe_zone_border_effects: bool,
    pub map_drop_rules: Vec<MapDropRuleRecord>,
    pub mine_zones: Vec<MineZoneRecord>,
    /// On-chain mine veins (M4) — render-only mappings, disjoint from `mine_zones`.
    pub onchain_mine_nodes: Vec<OnchainMineNodeRecord>,
    pub map_hazards: Vec<MapHazardRecord>,
    pub account_store: SharedAccountStore,
    pub account_store_path: Option<PathBuf>,
    pub account_store_database_url: Option<String>,
    pub account_store_database_mode: AccountStoreDatabaseMode,
    /// Explicit durable directory for Gateway character-save recovery journals.
    /// PostgreSQL deployments must configure this independently because a DB
    /// URL is not a filesystem location.
    pub save_recovery_dir: Option<PathBuf>,
    save_recovery_mac_key: Option<SaveRecoveryMacKey>,
    pub content_profile: Option<ContentProfileRuntime>,
    account_store_persist_lock: Arc<Mutex<()>>,
    /// Shared by every clone/rebind that can reach the same durable store.
    /// Once publication has an unknown outcome, only process restart plus a
    /// fresh durable reload may create a writable state again.
    account_store_write_state: Arc<Mutex<AccountStoreWriteState>>,
    #[cfg(any(test, feature = "test-support"))]
    account_store_transaction_fault: Arc<Mutex<Option<AccountStoreTransactionFault>>>,
    #[cfg(test)]
    account_store_repository_writer_probe: Arc<Mutex<Option<AccountStoreRepositoryWriterProbe>>>,
    #[cfg(test)]
    account_store_transaction_scope_observations:
        Arc<Mutex<Vec<AccountStoreTransactionScopeObservation>>>,
}

#[derive(Clone)]
struct SaveRecoveryMacKey(Arc<[u8; 32]>);

impl fmt::Debug for SaveRecoveryMacKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SaveRecoveryMacKey([REDACTED])")
    }
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self::from_scene(&starter_scene())
    }
}

impl SimulationConfig {
    /// Clone the runtime configuration while giving it an independent account
    /// store and persistence lock. Zone checkpoint replay uses this to ensure
    /// every full-journal replay starts from the same baseline instead of a
    /// store mutated by a previous checkpoint installation.
    pub fn fork_with_isolated_account_store(&self) -> Result<Self, String> {
        let account_store = self
            .account_store
            .lock()
            .map_err(|_| "account store lock poisoned".to_string())?
            .clone();
        let mut fork = self.clone();
        fork.account_store = Arc::new(Mutex::new(account_store));
        fork.account_store_persist_lock = Arc::new(Mutex::new(()));
        #[cfg(test)]
        {
            fork.account_store_transaction_fault = Arc::new(Mutex::new(None));
            fork.account_store_repository_writer_probe = Arc::new(Mutex::new(None));
            fork.account_store_transaction_scope_observations = Arc::new(Mutex::new(Vec::new()));
        }
        Ok(fork)
    }

    /// Build a standby-only runtime configuration. Replicated commands mutate
    /// an isolated account image and must never mirror those shadow writes back
    /// into the active file/PostgreSQL account repository.
    pub fn fork_for_replica_apply(&self) -> Result<Self, String> {
        let mut fork = self.fork_with_isolated_account_store()?;
        fork.account_store_path = None;
        fork.account_store_database_url = None;
        fork.save_recovery_dir = None;
        fork.save_recovery_mac_key = None;
        Ok(fork)
    }

    /// Rebind an already-restored replica Session to the authoritative account
    /// store when its Zone is promoted. The Session keeps its reconstructed
    /// world state while future character saves use the live host repository.
    pub fn rebind_account_store_from(&mut self, authoritative: &Self) {
        self.account_store = Arc::clone(&authoritative.account_store);
        self.account_store_path = authoritative.account_store_path.clone();
        self.account_store_database_url = authoritative.account_store_database_url.clone();
        self.account_store_database_mode = authoritative.account_store_database_mode;
        self.save_recovery_dir = authoritative.save_recovery_dir.clone();
        self.save_recovery_mac_key = authoritative.save_recovery_mac_key.clone();
        self.account_store_persist_lock = Arc::clone(&authoritative.account_store_persist_lock);
        self.account_store_write_state = Arc::clone(&authoritative.account_store_write_state);
    }

    pub fn from_scene(scene: &SceneBootstrap) -> Self {
        Self::from_scene_with_collision(scene, starter_map_collision())
    }

    pub fn from_scene_with_collision(
        scene: &SceneBootstrap,
        map_collision: StarterMapCollision,
    ) -> Self {
        let default_character = CharacterRecord {
            index: 0,
            name: scene.default_character.name.clone(),
            level: scene.default_character.level,
            class: scene.default_character.class,
            gender: scene.default_character.gender,
        };

        Self {
            map: scene.map.clone(),
            spawn: scene.spawn.clone(),
            scene_view: scene.scene_view.clone(),
            map_collision,
            require_storage_password: true,
            monster_spawn_source: MonsterSpawnSource::StarterScenario,
            terrain_patches: scene.terrain_patches.clone(),
            decor_objects: scene.decor_objects.clone(),
            default_character: default_character.clone(),
            object_id: scene.object_id,
            real_id: scene.real_id,
            visible_players: scene
                .visible_players
                .iter()
                .map(|player| VisiblePlayerRecord {
                    object_id: player.object_id,
                    name: player.name.clone(),
                    class: player.class,
                    gender: player.gender,
                    level: player.level,
                    armour_shape: None,
                    weapon_shape: None,
                    position: player.position.clone(),
                    direction: player.direction,
                })
                .collect(),
            group_member_object_ids: Vec::new(),
            visible_monsters: scene
                .visible_monsters
                .iter()
                .map(|monster| VisibleMonsterRecord {
                    object_id: monster.object_id,
                    name: monster.name.clone(),
                    image: monster.image,
                    position: monster.position.clone(),
                    direction: monster.direction,
                })
                .collect(),
            visible_npcs: scene
                .visible_npcs
                .iter()
                .map(|npc| VisibleNpcRecord {
                    object_id: npc.object_id,
                    name: npc.name.clone(),
                    image: npc.image,
                    colour_argb: npc.colour_argb,
                    position: npc.position.clone(),
                    direction: npc.direction,
                    quest_ids: npc.quest_ids.clone(),
                    script_key: npc.script_key.clone(),
                })
                .collect(),
            conquest_wars: BTreeMap::new(),
            conquest_owners: BTreeMap::new(),
            map_transfers: starter_map_transfers(),
            safe_zones: starter_safe_zones(),
            safe_zone_border_effects: false,
            map_drop_rules: Vec::new(),
            mine_zones: Vec::new(),
            onchain_mine_nodes: Vec::new(),
            map_hazards: Vec::new(),
            account_store: Arc::new(Mutex::new(AccountStore::new(default_character))),
            account_store_path: None,
            account_store_database_url: None,
            account_store_database_mode: AccountStoreDatabaseMode::Mirror,
            save_recovery_dir: None,
            save_recovery_mac_key: None,
            content_profile: None,
            account_store_persist_lock: Arc::new(Mutex::new(())),
            account_store_write_state: Arc::new(Mutex::new(AccountStoreWriteState::Writable)),
            #[cfg(any(test, feature = "test-support"))]
            account_store_transaction_fault: Arc::new(Mutex::new(None)),
            #[cfg(test)]
            account_store_repository_writer_probe: Arc::new(Mutex::new(None)),
            #[cfg(test)]
            account_store_transaction_scope_observations: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn with_platinum_176_profile(mut self) -> Self {
        let runtime = ContentProfileRuntime::platinum_176();
        if let Some(starter_map_rule) = runtime
            .profile
            .map_whitelist
            .iter()
            .find(|map| map.tier.eq_ignore_ascii_case("starter"))
        {
            let normalized_starter = normalize_content_map_file_name(&starter_map_rule.file_name);
            if let Some(starter_map) = crystal_respawn_manifest().maps.into_iter().find(|map| {
                normalize_content_map_file_name(&map.map_file_name) == normalized_starter
            }) {
                if let Some(start_point) = starter_map
                    .safe_zones
                    .iter()
                    .find(|safe_zone| safe_zone.start_point)
                {
                    self.map.map_index = starter_map.map_index;
                    self.map.file_name = starter_map.map_file_name;
                    self.map.title = starter_map.map_title;
                    self.spawn = start_point.location.clone();
                    self.scene_view.center = start_point.location.clone();
                    apply_crystal_map_metadata(&mut self.map);
                }
            }
        }
        self.content_profile = Some(runtime);
        self
    }

    pub fn experience_required_for_level(&self, level: u16) -> i64 {
        self.content_profile
            .as_ref()
            .map(|profile| profile.experience_required_for_level(level))
            .unwrap_or(100)
    }

    pub fn monster_experience_multiplier(&self, level: u16) -> u32 {
        self.content_profile
            .as_ref()
            .map(|profile| profile.monster_experience_multiplier(level))
            .unwrap_or(1)
    }

    pub fn class_is_allowed(&self, class: MirClass) -> bool {
        self.content_profile
            .as_ref()
            .is_none_or(|profile| profile.class_is_allowed(class))
    }

    pub fn map_is_allowed(&self, file_name: &str) -> bool {
        self.content_profile
            .as_ref()
            .is_none_or(|profile| profile.map_is_allowed(file_name))
    }

    pub fn monster_is_allowed(&self, monster_name: &str) -> bool {
        self.content_profile
            .as_ref()
            .is_none_or(|profile| profile.monster_is_allowed(monster_name))
    }

    pub fn crystal_respawns_for_map(&self, map_file_name: &str) -> Vec<CrystalRespawnTemplate> {
        self.content_profile.as_ref().map_or_else(
            || {
                crystal_map_respawns_by_file_name(map_file_name)
                    .map(|map| map.respawns)
                    .unwrap_or_default()
            },
            |profile| profile.crystal_respawns_for_map(map_file_name),
        )
    }

    pub fn monster_respawn_random_delay_minutes(
        &self,
        monster_name: &str,
        imported_random_delay_minutes: u16,
    ) -> u16 {
        self.content_profile
            .as_ref()
            .map(|profile| {
                profile.monster_respawn_random_delay_minutes(
                    monster_name,
                    imported_random_delay_minutes,
                )
            })
            .unwrap_or(imported_random_delay_minutes)
    }

    pub fn item_is_allowed(&self, item_name: &str) -> bool {
        self.content_profile
            .as_ref()
            .is_none_or(|profile| profile.item_is_allowed(item_name))
    }

    pub fn npc_script_is_allowed(&self, script_key: &str) -> bool {
        self.content_profile
            .as_ref()
            .is_none_or(|profile| profile.npc_script_is_allowed(script_key))
    }

    pub fn content_skill_rule(&self, spell: &str) -> Option<&ContentSkillRule> {
        self.content_profile
            .as_ref()
            .and_then(|profile| profile.content_skill_rule(spell))
    }

    pub fn skill_is_allowed(&self, spell: &str, class: MirClass, level: u16) -> bool {
        self.content_profile
            .as_ref()
            .is_none_or(|profile| profile.skill_is_allowed(spell, class, level))
    }

    pub fn stage5_action_is_allowed(&self, action: &str) -> bool {
        self.content_profile
            .as_ref()
            .is_none_or(|profile| profile.stage5_action_is_allowed(action))
    }

    pub fn with_account_store_path(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let repository = FileAccountStoreRepository::new(path.clone());
        self.account_store = Arc::new(Mutex::new(
            repository
                .load(self.default_character.clone())
                .unwrap_or_else(|error| {
                    eprintln!("failed to load file account store, using default: {error}");
                    AccountStore::new(self.default_character.clone())
                }),
        ));
        self.account_store_path = Some(path);
        self
    }

    pub fn with_save_recovery_dir(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        if !path.as_os_str().is_empty() {
            self.save_recovery_dir = Some(path);
        }
        self
    }

    pub fn with_save_recovery_mac_key(mut self, key: impl AsRef<[u8]>) -> Result<Self, String> {
        let key = key.as_ref();
        let key: [u8; 32] = key
            .try_into()
            .map_err(|_| "save recovery MAC key must decode to exactly 32 bytes".to_string())?;
        let mut distinct = [false; 256];
        for byte in key {
            distinct[usize::from(byte)] = true;
        }
        if distinct.into_iter().filter(|present| *present).count() < 8 {
            return Err("save recovery MAC key does not meet minimum diversity".to_string());
        }
        self.save_recovery_mac_key = Some(SaveRecoveryMacKey(Arc::new(key)));
        Ok(self)
    }

    pub fn save_recovery_mac_key(&self) -> Option<&[u8; 32]> {
        self.save_recovery_mac_key
            .as_ref()
            .map(|key| key.0.as_ref())
    }

    pub fn recovery_journal_enabled(&self) -> bool {
        self.save_recovery_dir.is_some()
            || self.account_store_path.is_some()
            || self.account_store_database_url.is_some()
    }

    pub fn with_crystal_map_runtime(mut self) -> Self {
        self.monster_spawn_source = MonsterSpawnSource::CrystalStarterRegion;
        self.map_transfers.clear();
        self.map_hazards = crystal_map_hazard_records();
        apply_crystal_map_metadata(&mut self.map);
        // Starter mining zone: a small ore vein just east of the Bichon spawn
        // (330, 270) so players can try mining without leaving the starter map.
        // Expands to cells x in [331, 335), y in [268, 272).
        self.mine_zones.push(MineZoneRecord {
            map_file_name: "0".to_string(),
            mine_set: 1,
            x: 333,
            y: 270,
            size: 2,
        });
        // On-chain smart-mine veins (M4) are env-gated (off by default — no ghost veins
        // in deployments without the on-chain stack). See `onchain_mine_nodes_from_env`.
        self.onchain_mine_nodes
            .extend(onchain_mine_nodes_from_env());
        // Bichon blacksmith whose existing [Trade] list already sells a PickAxe,
        // so players can buy a mining tool without touching the starter inventory.
        self.visible_npcs.push(VisibleNpcRecord {
            object_id: 4600,
            name: "Blacksmith".to_string(),
            image: 5,
            colour_argb: -1,
            position: Point { x: 329, y: 270 },
            direction: MirDirection::Down,
            quest_ids: Vec::new(),
            script_key: Some("BichonProvince/BichonWall/Blacksmith-0103".to_string()),
        });
        self
    }

    /// Activate the full Crystal world (every map live when occupied, dormant
    /// when empty). Unlike [`with_crystal_map_runtime`], this does not fence the
    /// player into the starter slice: it keeps every manifest movement so all
    /// All 463 named maps (464 source records including one empty DB placeholder)
    /// are reachable, and entering any named map spawns that map's entire
    /// Crystal respawn set. The starter map keeps its real manifest NPCs
    /// (spawned per-map from the manifest), so no hand-authored starter
    /// blacksmith/mine overlays are injected here — NOTE that `mine_zones`
    /// stays EMPTY on this path (Crystal stores mine zones in the Map DB, not
    /// the map manifest), so P0 veins only exist where config seeds them.
    ///
    /// [`with_crystal_map_runtime`]: Self::with_crystal_map_runtime
    pub fn with_crystal_world_runtime(mut self) -> Self {
        self.monster_spawn_source = MonsterSpawnSource::CrystalWorld;
        // Drop the hand-authored starter gate; manifest movements drive Crystal travel.
        self.map_transfers.clear();
        // Full-world entities come from Crystal manifests and shared Zone state.
        self.visible_players.clear();
        self.visible_monsters.clear();
        self.visible_npcs.clear();
        self.map_hazards = crystal_map_hazard_records();
        apply_crystal_map_metadata(&mut self.map);
        if let Some(start_point) = crystal_map_respawns_by_file_name(&self.map.file_name)
            .and_then(|map| map.safe_zones.into_iter().find(|zone| zone.start_point))
        {
            self.spawn = start_point.location;
        }
        // On-chain smart-mine veins (M4) are env-gated (off by default).
        self.onchain_mine_nodes
            .extend(onchain_mine_nodes_from_env());
        self
    }

    pub fn with_account_store_database_url(mut self, database_url: impl Into<String>) -> Self {
        let database_url = database_url.into();
        if !database_url.trim().is_empty() {
            self.account_store_database_url = Some(database_url);
            self.account_store_database_mode = AccountStoreDatabaseMode::Mirror;
        }
        self
    }

    pub fn with_postgres_account_store(
        self,
        database_url: impl Into<String>,
    ) -> Result<Self, String> {
        let database_url = database_url.into();
        self.with_postgres_account_store_with_loader(database_url, |database_url, default| {
            let repository = PostgresAccountStoreRepository::new(
                database_url,
                AccountStoreDatabaseMode::SourceOfTruth,
            );
            repository.load(default)
        })
    }

    fn with_postgres_account_store_with_loader<F>(
        mut self,
        database_url: String,
        loader: F,
    ) -> Result<Self, String>
    where
        F: FnOnce(String, CharacterRecord) -> Result<AccountStore, String>,
    {
        validate_postgres_account_store_database_url(&database_url)?;
        let store = loader(database_url.clone(), self.default_character.clone())
            .map_err(|error| format!("failed to load postgres account store: {error}"))?;
        self.account_store = Arc::new(Mutex::new(store));
        self.account_store_path = None;
        self.account_store_database_url = Some(database_url);
        self.account_store_database_mode = AccountStoreDatabaseMode::SourceOfTruth;
        Ok(self)
    }

    pub fn with_account_store_environment(
        self,
        account_store_path: impl Into<PathBuf>,
    ) -> Result<Self, String> {
        let backend = account_store_runtime_backend_from_env()?;
        let database_url = env::var("MIR2_ACCOUNT_STORE_DATABASE_URL").ok();
        self.with_account_store_environment_with_loader(
            account_store_path.into(),
            backend,
            database_url,
            |database_url, default| {
                PostgresAccountStoreRepository::new(
                    database_url,
                    AccountStoreDatabaseMode::SourceOfTruth,
                )
                .load(default)
            },
        )
    }

    fn with_account_store_environment_with_loader<F>(
        self,
        account_store_path: PathBuf,
        backend: AccountStoreRuntimeBackend,
        database_url: Option<String>,
        postgres_loader: F,
    ) -> Result<Self, String>
    where
        F: FnOnce(String, CharacterRecord) -> Result<AccountStore, String>,
    {
        match backend {
            AccountStoreRuntimeBackend::Postgres => {
                let database_url = database_url.ok_or_else(|| {
                    "MIR2_ACCOUNT_STORE_DATABASE_URL is required for postgres account store"
                        .to_string()
                })?;
                self.with_postgres_account_store_with_loader(database_url, postgres_loader)
            }
            AccountStoreRuntimeBackend::File => {
                let mut config = self.with_account_store_path(account_store_path);
                if let Some(database_url) = database_url {
                    config = config.with_account_store_database_url(database_url);
                }
                Ok(config)
            }
        }
    }

    fn ensure_account_store_writable(&self) -> Result<(), String> {
        let state = self.account_store_write_state.lock().map_err(|_| {
            "account-store write-state mutex poisoned; writes are blocked".to_string()
        })?;
        match &*state {
            AccountStoreWriteState::Writable => Ok(()),
            AccountStoreWriteState::Frozen { reason } => Err(format!(
                "account-store writes are frozen until restart and durable reload: {reason}"
            )),
        }
    }

    fn freeze_account_store_writes(&self, reason: String) -> String {
        let reason = format!("{reason}; live AccountStore was not published");
        match self.account_store_write_state.lock() {
            Ok(mut state) => {
                if matches!(*state, AccountStoreWriteState::Writable) {
                    *state = AccountStoreWriteState::Frozen {
                        reason: reason.clone(),
                    };
                }
                match &*state {
                    AccountStoreWriteState::Writable => unreachable!(),
                    AccountStoreWriteState::Frozen { reason } => reason.clone(),
                }
            }
            Err(_) => format!(
                "{reason}; account-store write-state mutex poisoned, so subsequent writes remain fail-closed"
            ),
        }
    }

    fn map_file_commit_error(&self, error: AccountStoreFileCommitError) -> String {
        if error.is_commit_outcome_unknown() {
            self.freeze_account_store_writes(format!(
                "{ACCOUNT_STORE_COMMIT_OUTCOME_UNKNOWN}: {error}"
            ))
        } else {
            error.to_string()
        }
    }

    fn save_file_account_store(
        &self,
        path: &Path,
        store: &AccountStore,
    ) -> Result<AccountStoreRepositorySave, AccountStoreFileCommitError> {
        let fault = self.take_account_store_file_commit_fault();
        FileAccountStoreRepository::new(path).save_with_commit_outcome(store, fault)
    }

    fn save_account_store_mutation_plan_to_repository(
        &self,
        database_url: &str,
        mode: AccountStoreDatabaseMode,
        mutation_plan: &AccountStoreMutationPlan,
    ) -> Result<AccountStoreRepositorySave, String> {
        #[cfg(test)]
        {
            let mut probe = self
                .account_store_repository_writer_probe
                .lock()
                .expect("account-store repository writer probe mutex should not be poisoned");
            if let Some(probe) = probe.as_mut() {
                probe.invocations = probe.invocations.saturating_add(1);
                probe.last_plan_includes_global = Some(mutation_plan.global_stock.is_some());
                return probe.outcome.clone();
            }
        }
        PostgresAccountStoreRepository::new(database_url, mode).save_mutation_plan(mutation_plan)
    }

    fn take_account_store_file_commit_fault(&self) -> Option<AccountStoreFileCommitFault> {
        #[cfg(any(test, feature = "test-support"))]
        {
            let mut fault = self
                .account_store_transaction_fault
                .lock()
                .expect("account-store transaction fault mutex should not be poisoned");
            let mapped = match *fault {
                Some(AccountStoreTransactionFault::BeforeFileRename) => {
                    Some(AccountStoreFileCommitFault::BeforeRename)
                }
                Some(AccountStoreTransactionFault::AfterFileRenameBeforeDirectorySync) => {
                    Some(AccountStoreFileCommitFault::AfterRenameBeforeDirectorySync)
                }
                _ => None,
            };
            if mapped.is_some() {
                *fault = None;
            }
            mapped
        }
        #[cfg(not(any(test, feature = "test-support")))]
        {
            None
        }
    }

    pub fn save_account_store(&self) -> Result<(), String> {
        self.save_account_store_with_plan_writer(None, |database_url, mode, mutation_plan| {
            PostgresAccountStoreRepository::new(database_url, mode)
                .save_mutation_plan(mutation_plan)
                .map(AccountStoreRepositorySave::into_source_versions)
        })
    }

    fn save_account_store_with_plan_writer<F>(
        &self,
        scoped_account_id: Option<&str>,
        write_repository: F,
    ) -> Result<(), String>
    where
        F: FnOnce(
            &str,
            AccountStoreDatabaseMode,
            &AccountStoreMutationPlan,
        ) -> Result<AccountStoreSourceVersions, String>,
    {
        let _persist_guard = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "account store persist mutex poisoned".to_string())?;
        self.ensure_account_store_writable()?;
        let store = {
            let store = self
                .account_store
                .lock()
                .expect("account store mutex should not be poisoned");
            store.clone()
        };
        let account_ids = if let Some(account_id) = scoped_account_id {
            vec![account_id.to_string()]
        } else {
            let mut account_ids = BTreeSet::new();
            account_ids.extend(store.accounts.keys().cloned());
            account_ids.extend(store.source_account_versions.keys().cloned());
            account_ids.extend(store.source_save_versions.keys().cloned());
            account_ids.into_iter().collect::<Vec<_>>()
        };
        let scope = if scoped_account_id.is_some() {
            AccountStoreMutationScope::Accounts(&account_ids)
        } else {
            AccountStoreMutationScope::AccountsWithGlobal(&account_ids)
        };
        let mutation_plan = build_account_store_mutation_plan(
            &store,
            &store,
            scope,
            self.account_store_database_mode == AccountStoreDatabaseMode::Mirror
                && scoped_account_id.is_none(),
        );

        if let Some(path) = self.account_store_path.as_deref() {
            self.save_file_account_store(path, &store)
                .map_err(|error| self.map_file_commit_error(error))?;
        }
        if let Some(database_url) = self.account_store_database_url.as_deref() {
            let source_versions = write_repository(
                database_url,
                self.account_store_database_mode,
                &mutation_plan,
            )?;
            if self.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth {
                let mut live_store = self
                    .account_store
                    .lock()
                    .expect("account store mutex should not be poisoned");
                apply_account_store_mutation_source_versions(
                    &mut live_store,
                    &mutation_plan,
                    source_versions,
                );
            }
        }
        Ok(())
    }

    /// Commit a bounded multi-account mutation as one persistence operation.
    ///
    /// The shared account image is kept unchanged while `transaction` mutates
    /// an isolated snapshot.  File persistence replaces the complete snapshot
    /// atomically; PostgreSQL persists only `account_ids`, but does so inside
    /// the repository's single transaction with the existing optimistic
    /// version checks.  The in-memory image is replaced only after every
    /// authoritative write succeeds.
    ///
    /// The account-store mutex deliberately remains held while the repositories
    /// are called.  This prevents an in-process writer from being overwritten
    /// between durable commit and the shared-image swap.  The implementation
    /// calls repositories directly and must never recurse through
    /// `save_account_store*` while holding this mutex.
    pub(crate) fn commit_account_store_transaction<T, F>(
        &self,
        account_ids: &[String],
        transaction: F,
    ) -> Result<T, String>
    where
        F: FnOnce(&mut AccountStore) -> Result<T, String>,
    {
        if account_ids.is_empty() {
            return Err("account-store transaction requires at least one account".to_string());
        }
        self.commit_account_store_transaction_inner(
            AccountStoreMutationScope::Accounts(account_ids),
            transaction,
        )
    }

    pub(crate) fn commit_account_store_transaction_with_global<T, F>(
        &self,
        account_ids: &[String],
        transaction: F,
    ) -> Result<T, String>
    where
        F: FnOnce(&mut AccountStore) -> Result<T, String>,
    {
        if account_ids.is_empty() {
            return Err("account-store transaction requires at least one account".to_string());
        }
        self.commit_account_store_transaction_inner(
            AccountStoreMutationScope::AccountsWithGlobal(account_ids),
            transaction,
        )
    }

    fn commit_account_store_transaction_inner<T, F>(
        &self,
        scope: AccountStoreMutationScope<'_>,
        transaction: F,
    ) -> Result<T, String>
    where
        F: FnOnce(&mut AccountStore) -> Result<T, String>,
    {
        let empty_account_scope = match scope {
            AccountStoreMutationScope::Accounts(account_ids)
            | AccountStoreMutationScope::AccountsWithGlobal(account_ids) => account_ids.is_empty(),
            AccountStoreMutationScope::FullRestore => false,
        };
        if empty_account_scope {
            return Err("account-store transaction requires at least one account".to_string());
        }

        #[cfg(test)]
        self.record_account_store_transaction_scope(scope);

        let _persist_guard = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "account store persist mutex poisoned".to_string())?;
        self.ensure_account_store_writable()?;
        let mut live_store = self
            .account_store
            .lock()
            .map_err(|_| "account store mutex poisoned".to_string())?;
        let original_store = live_store.clone();
        let mut staged_store = original_store.clone();
        let result = transaction(&mut staged_store)?;
        validate_account_store_transaction_scope(&original_store, &staged_store, scope)
            .map_err(|error| error.to_string())?;
        let mutation_plan =
            build_account_store_mutation_plan(&original_store, &staged_store, scope, false);

        #[cfg(any(test, feature = "test-support"))]
        if self.take_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist) {
            return Err("injected account-store failure before persist".to_string());
        }

        // Fail serialization before touching either repository.  The file
        // backend repeats this encoding for its pretty snapshot, while this
        // preflight also protects PostgreSQL mirror-first ordering.
        serde_json::to_vec(&staged_store)
            .map_err(|error| format!("failed to encode account-store transaction: {error}"))?;

        #[cfg(any(test, feature = "test-support"))]
        if self.take_account_store_transaction_fault(AccountStoreTransactionFault::Persist) {
            return Err("injected account-store persistence failure".to_string());
        }

        if self.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth
            && self.account_store_path.is_some()
        {
            return Err(
                "postgres source-of-truth account store cannot also commit a file primary"
                    .to_string(),
            );
        }

        let rollback_plan = build_account_store_mutation_plan(
            &staged_store,
            &original_store,
            scope,
            mutation_plan.global_stock.is_some(),
        );
        let mut postgres_versions = None;

        match (
            self.account_store_path.as_deref(),
            self.account_store_database_url.as_deref(),
        ) {
            (Some(path), Some(database_url)) => {
                // File mode with a configured database uses the file as the
                // source and PostgreSQL as a synchronous mirror.  Write the
                // mirror first so a mirror failure cannot advance the source.
                // A known pre-publication file failure is compensated with the
                // original mirror snapshot. An outcome-unknown file publish is
                // never compensated or retried: both writes may already hold
                // the staged value, so the process freezes until durable
                // reconciliation after restart. This ordering is fail-closed;
                // it does not claim cross-database atomicity.
                self.save_account_store_mutation_plan_to_repository(
                    database_url,
                    AccountStoreDatabaseMode::Mirror,
                    &mutation_plan,
                )?;
                if let Err(file_error) = self.save_file_account_store(path, &staged_store) {
                    if file_error.is_commit_outcome_unknown() {
                        return Err(self.map_file_commit_error(file_error));
                    }
                    return match self.save_account_store_mutation_plan_to_repository(
                        database_url,
                        AccountStoreDatabaseMode::Mirror,
                        &rollback_plan,
                    ) {
                        Ok(_) => Err(file_error.to_string()),
                        Err(rollback_error) => Err(self.freeze_account_store_writes(format!(
                            "file account-store write was not committed ({file_error}), but postgres mirror compensation failed: {rollback_error}"
                        ))),
                    };
                }
            }
            (Some(path), None) => {
                self.save_file_account_store(path, &staged_store)
                    .map_err(|error| self.map_file_commit_error(error))?;
            }
            (None, Some(database_url)) => {
                postgres_versions = Some(
                    self.save_account_store_mutation_plan_to_repository(
                        database_url,
                        self.account_store_database_mode,
                        &mutation_plan,
                    )?
                    .into_source_versions(),
                );
            }
            (None, None) => {}
        }

        if self.account_store_database_mode == AccountStoreDatabaseMode::SourceOfTruth {
            if let Some(versions) = postgres_versions {
                apply_account_store_mutation_source_versions(
                    &mut staged_store,
                    &mutation_plan,
                    versions,
                );
            }
        }
        *live_store = staged_store;
        Ok(result)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn inject_account_store_transaction_fault(&self, fault: AccountStoreTransactionFault) {
        *self
            .account_store_transaction_fault
            .lock()
            .expect("account-store transaction fault mutex should not be poisoned") = Some(fault);
    }

    #[cfg(test)]
    fn record_account_store_transaction_scope(&self, scope: AccountStoreMutationScope<'_>) {
        let observation = match scope {
            AccountStoreMutationScope::Accounts(_) => {
                AccountStoreTransactionScopeObservation::AccountOnly
            }
            AccountStoreMutationScope::AccountsWithGlobal(_) => {
                AccountStoreTransactionScopeObservation::WithGlobal
            }
            AccountStoreMutationScope::FullRestore => {
                AccountStoreTransactionScopeObservation::FullRestore
            }
        };
        self.account_store_transaction_scope_observations
            .lock()
            .expect("account-store transaction scope probe mutex should not be poisoned")
            .push(observation);
    }

    #[cfg(test)]
    pub(crate) fn clear_account_store_transaction_scope_observations(&self) {
        self.account_store_transaction_scope_observations
            .lock()
            .expect("account-store transaction scope probe mutex should not be poisoned")
            .clear();
    }

    #[cfg(test)]
    pub(crate) fn account_store_transaction_scope_observations(
        &self,
    ) -> Vec<AccountStoreTransactionScopeObservation> {
        self.account_store_transaction_scope_observations
            .lock()
            .expect("account-store transaction scope probe mutex should not be poisoned")
            .clone()
    }

    #[cfg(test)]
    fn inject_account_store_repository_writer_probe(
        &self,
        outcome: Result<AccountStoreRepositorySave, String>,
    ) {
        *self
            .account_store_repository_writer_probe
            .lock()
            .expect("account-store repository writer probe mutex should not be poisoned") =
            Some(AccountStoreRepositoryWriterProbe {
                invocations: 0,
                last_plan_includes_global: None,
                outcome,
            });
    }

    #[cfg(test)]
    fn account_store_repository_writer_probe_invocations(&self) -> usize {
        self.account_store_repository_writer_probe
            .lock()
            .expect("account-store repository writer probe mutex should not be poisoned")
            .as_ref()
            .map(|probe| probe.invocations)
            .unwrap_or_default()
    }

    #[cfg(test)]
    fn account_store_repository_writer_probe_last_plan_includes_global(&self) -> Option<bool> {
        self.account_store_repository_writer_probe
            .lock()
            .expect("account-store repository writer probe mutex should not be poisoned")
            .as_ref()
            .and_then(|probe| probe.last_plan_includes_global)
    }

    #[cfg(any(test, feature = "test-support"))]
    fn take_account_store_transaction_fault(&self, expected: AccountStoreTransactionFault) -> bool {
        let mut fault = self
            .account_store_transaction_fault
            .lock()
            .expect("account-store transaction fault mutex should not be poisoned");
        if *fault == Some(expected) {
            *fault = None;
            true
        } else {
            false
        }
    }

    /// Refresh one account from the authoritative PostgreSQL repository.
    ///
    /// Zone handoff and reconnect can land on a process whose in-memory account
    /// image predates the latest character save. Loading only the requested
    /// account keeps that boundary safe without re-reading every account on
    /// every login.
    pub(crate) fn refresh_account_store_account(
        &self,
        account_id: &str,
    ) -> Result<AccountSourceRefreshOutcome, String> {
        if self.account_store_database_mode != AccountStoreDatabaseMode::SourceOfTruth {
            return Ok(AccountSourceRefreshOutcome::NotRequired);
        }
        let Some(database_url) = self.account_store_database_url.as_deref() else {
            return Err("postgres account source is unavailable".to_string());
        };
        self.refresh_account_store_account_with_loader(account_id, || {
            load_account_from_postgres(database_url.to_string(), account_id.to_string())
        })
    }

    fn refresh_account_store_account_with_loader<F>(
        &self,
        account_id: &str,
        loader: F,
    ) -> Result<AccountSourceRefreshOutcome, String>
    where
        F: FnOnce() -> Result<Option<(AccountRecord, AccountStoreSourceVersions)>, String>,
    {
        if self.account_store_database_mode != AccountStoreDatabaseMode::SourceOfTruth {
            return Ok(AccountSourceRefreshOutcome::NotRequired);
        }
        let _persist_guard = self.account_store_persist_lock.lock().map_err(|_| {
            "account store persist mutex poisoned during source refresh".to_string()
        })?;
        self.ensure_account_store_writable()?;
        let loaded = loader()?;
        let mut store = self
            .account_store
            .lock()
            .map_err(|_| "account store mutex poisoned".to_string())?;
        let Some((account, versions)) = loaded else {
            store.accounts.remove(account_id);
            store.source_account_versions.remove(account_id);
            store.source_save_versions.remove(account_id);
            return Ok(AccountSourceRefreshOutcome::Missing);
        };
        store.source_account_versions.remove(account_id);
        store.source_save_versions.remove(account_id);
        store.accounts.insert(account_id.to_string(), account);
        store.merge_source_versions(versions);
        store.normalize_next_character_index();
        Ok(AccountSourceRefreshOutcome::Refreshed)
    }

    /// Refresh only the shared finite GameShop counters from PostgreSQL.
    ///
    /// Account refresh intentionally does not replace this process-wide state:
    /// a finite global purchase retries through this explicit path after the
    /// global CAS reports a stale source version.
    pub(crate) fn refresh_game_shop_global_stock(&self) -> Result<bool, String> {
        if self.account_store_database_mode != AccountStoreDatabaseMode::SourceOfTruth {
            return Ok(false);
        }
        let Some(database_url) = self.account_store_database_url.as_deref() else {
            return Ok(false);
        };
        let _persist_guard = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "account store persist mutex poisoned".to_string())?;
        self.ensure_account_store_writable()?;
        let (purchases, version) =
            load_game_shop_global_stock_from_postgres(database_url.to_string())?;
        let mut store = self
            .account_store
            .lock()
            .map_err(|_| "account store mutex poisoned".to_string())?;
        store.game_shop_global_purchases = purchases;
        store.source_game_shop_global_version = version;
        store.source_game_shop_global_purchases = store.game_shop_global_purchases.clone();
        Ok(true)
    }

    pub fn save_account_store_account(&self, account_id: &str) -> Result<(), String> {
        self.save_account_store_with_plan_writer(
            Some(account_id),
            |database_url, mode, mutation_plan| {
                PostgresAccountStoreRepository::new(database_url, mode)
                    .save_mutation_plan(mutation_plan)
                    .map(AccountStoreRepositorySave::into_source_versions)
            },
        )
    }

    pub fn account_store_repository_statuses(&self) -> Vec<AccountStoreRepositoryStatus> {
        let mut statuses = Vec::new();
        if let Some(path) = self.account_store_path.as_ref() {
            statuses.push(FileAccountStoreRepository::new(path.clone()).status());
        }
        if let Some(database_url) = self.account_store_database_url.as_ref() {
            statuses.push(
                PostgresAccountStoreRepository::new(
                    database_url.clone(),
                    self.account_store_database_mode,
                )
                .status(),
            );
        }
        if statuses.is_empty() {
            statuses.push(AccountStoreRepositoryStatus {
                backend: "memory".to_string(),
                mode: AccountStoreDatabaseMode::Mirror,
                configured: true,
                location: "process".to_string(),
            });
        }
        statuses
    }

    pub fn backup_account_store(&self, backup_path: impl AsRef<Path>) -> Result<(), String> {
        let store = self
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        store.save_to_path(backup_path.as_ref())
    }

    fn allows_default_account_fixture(&self) -> bool {
        self.account_store_database_mode != AccountStoreDatabaseMode::SourceOfTruth
            && (self.account_store_path.is_some() || self.account_store_database_url.is_none())
    }

    pub fn restore_account_store_from_backup(
        &self,
        backup_path: impl AsRef<Path>,
    ) -> Result<(), String> {
        let backup_path = backup_path.as_ref();
        let data = fs::read_to_string(backup_path).map_err(|error| {
            format!(
                "failed to read account store backup {}: {error}",
                backup_path.display()
            )
        })?;
        let decoded = serde_json::from_str::<AccountStore>(&data).map_err(|error| {
            format!(
                "failed to decode account store backup {}: {error}",
                backup_path.display()
            )
        })?;
        let mut restored = if self.allows_default_account_fixture() {
            decoded.with_default_account(self.default_character.clone())
        } else {
            decoded
        }
        .migrate_to_current_schema();
        // Backup JSON deliberately excludes optimistic source metadata.  Full
        // restore builds every expected version from the locked live image.
        restored.source_account_versions.clear();
        restored.source_save_versions.clear();
        restored.source_game_shop_global_version = None;
        restored.source_game_shop_global_purchases.clear();
        self.commit_account_store_transaction_inner(
            AccountStoreMutationScope::FullRestore,
            move |staged_store| {
                *staged_store = restored;
                Ok(())
            },
        )
    }
}

fn normalize_content_map_file_name(file_name: &str) -> String {
    file_name
        .trim()
        .trim_end_matches(".map")
        .trim_end_matches(".MAP")
        .to_ascii_lowercase()
}

fn normalize_content_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn normalize_content_npc_script_key(value: &str) -> String {
    let normalized = value.trim().replace('\\', "/").to_ascii_lowercase();
    normalized
        .rsplit_once('-')
        .filter(|(_, suffix)| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
        .map(|(base, _)| base.to_string())
        .unwrap_or(normalized)
}

pub fn apply_crystal_map_metadata(map: &mut MapInformation) -> bool {
    let Some(crystal_map) = crystal_map_respawns_by_file_name(&map.file_name) else {
        return false;
    };
    map.map_index = crystal_map.map_index;
    map.title = crystal_map.map_title;
    map.mini_map = crystal_map.mini_map;
    map.big_map = crystal_map.big_map;
    map.lights = crystal_map.light;
    map.flags = u8::from(crystal_map.lightning) | (u8::from(crystal_map.fire) << 1);
    map.map_dark_light = crystal_map.map_dark_light;
    map.music = crystal_map.music;
    map.weather_particles = crystal_map.weather_particles;
    true
}

fn crystal_map_hazard_records() -> Vec<MapHazardRecord> {
    crystal_respawn_manifest()
        .maps
        .into_iter()
        .filter(|map| map.lightning || map.fire)
        .map(|map| MapHazardRecord {
            map_file_name: map.map_file_name,
            lightning: map.lightning,
            fire: map.fire,
            lightning_damage: map.lightning_damage,
            fire_damage: map.fire_damage,
        })
        .collect()
}

static ACCOUNT_STORE_FILE_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn save_account_store_snapshot_to_path(
    store: &AccountStore,
    path: &Path,
    fault: Option<AccountStoreFileCommitFault>,
) -> Result<(), AccountStoreFileCommitError> {
    let _guard = ACCOUNT_STORE_FILE_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| {
            AccountStoreFileCommitError::not_committed(io::Error::new(
                io::ErrorKind::Other,
                "account store file write mutex poisoned",
            ))
        })?;
    store.save_to_path_with_fault(path, fault)
}

fn load_account_store_from_postgres(
    database_url: String,
    default_character: CharacterRecord,
) -> Result<AccountStore, String> {
    load_account_store_from_postgres_with_pool(
        postgres_account_store_pool(&database_url),
        default_character,
    )
}

fn load_account_from_postgres(
    database_url: String,
    account_id: String,
) -> Result<Option<(AccountRecord, AccountStoreSourceVersions)>, String> {
    let pool = postgres_account_store_pool(&database_url);
    std::thread::spawn(move || {
        let mut client = pool.connection()?;
        pool.ensure_migrated(&mut client)?;
        let Some(row) = client
            .query_opt(
                "SELECT raw_json, store_version FROM accounts WHERE account_id = $1",
                &[&account_id],
            )
            .map_err(|error| {
                format!("postgres account refresh failed for {account_id}: {error}")
            })?
        else {
            return Ok(None);
        };
        let raw_json: Value = row.get("raw_json");
        let account = serde_json::from_value::<AccountRecord>(raw_json).map_err(|error| {
            format!("postgres account raw_json decode failed for {account_id}: {error}")
        })?;
        let mut versions = AccountStoreSourceVersions::default();
        versions
            .accounts
            .insert(account_id.clone(), row.get("store_version"));
        let save_rows = client
            .query(
                "SELECT character_index, save_version
                 FROM character_saves
                 WHERE account_id = $1
                 ORDER BY character_index",
                &[&account_id],
            )
            .map_err(|error| {
                format!("postgres character-save refresh failed for {account_id}: {error}")
            })?;
        let save_versions = versions.saves.entry(account_id).or_default();
        for save_row in save_rows {
            save_versions.insert(
                save_row.get("character_index"),
                save_row.get("save_version"),
            );
        }
        Ok(Some((account, versions)))
    })
    .join()
    .map_err(|_| "postgres account refresh thread panicked".to_string())?
}

fn load_game_shop_global_stock_from_postgres(
    database_url: String,
) -> Result<(BTreeMap<i32, u64>, Option<i64>), String> {
    let pool = postgres_account_store_pool(&database_url);
    std::thread::spawn(move || {
        let mut client = pool.connection()?;
        pool.ensure_migrated(&mut client)?;
        load_game_shop_global_stock(&mut client)
    })
    .join()
    .map_err(|_| "postgres game-shop global-stock refresh thread panicked".to_string())?
}

fn load_account_store_from_postgres_with_pool(
    pool: Arc<PostgresAccountStoreConnectionPool>,
    _default_character: CharacterRecord,
) -> Result<AccountStore, String> {
    std::thread::spawn(move || {
        let mut client = pool.connection()?;
        pool.ensure_migrated(&mut client)?;
        let (game_shop_global_purchases, game_shop_global_version) =
            load_game_shop_global_stock(&mut client)?;
        let account_rows = client
            .query(
                "SELECT account_id, raw_json, store_version FROM accounts ORDER BY account_id",
                &[],
            )
            .map_err(|error| format!("postgres account-store load failed: {error}"))?
            .into_iter()
            .map(|row| {
                (
                    row.get::<_, String>("account_id"),
                    row.get::<_, Value>("raw_json"),
                    row.get::<_, i64>("store_version"),
                )
            })
            .collect();
        let save_rows = client
            .query(
                "SELECT account_id, character_index, save_version FROM character_saves ORDER BY account_id, character_index",
                &[],
            )
            .map_err(|error| format!("postgres character-save version load failed: {error}"))?
            .into_iter()
            .map(|row| {
                (
                    row.get::<_, String>("account_id"),
                    row.get::<_, i32>("character_index"),
                    row.get::<_, i64>("save_version"),
                )
            })
            .collect();
        assemble_account_store_from_postgres_rows(
            account_rows,
            save_rows,
            game_shop_global_purchases,
            game_shop_global_version,
        )
    })
    .join()
    .map_err(|_| "postgres account-store load thread panicked".to_string())?
}

fn assemble_account_store_from_postgres_rows(
    account_rows: Vec<(String, Value, i64)>,
    save_rows: Vec<(String, i32, i64)>,
    game_shop_global_purchases: BTreeMap<i32, u64>,
    game_shop_global_version: Option<i64>,
) -> Result<AccountStore, String> {
    let mut accounts = BTreeMap::new();
    let mut versions = AccountStoreSourceVersions::default();
    for (account_id, raw_json, store_version) in account_rows {
        let account = serde_json::from_value::<AccountRecord>(raw_json).map_err(|error| {
            format!("postgres account raw_json decode failed for {account_id}: {error}")
        })?;
        versions.accounts.insert(account_id.clone(), store_version);
        accounts.insert(account_id, account);
    }
    for (account_id, character_index, save_version) in save_rows {
        versions
            .saves
            .entry(account_id)
            .or_default()
            .insert(character_index, save_version);
    }
    versions.game_shop_global_version = game_shop_global_version;
    Ok(AccountStore {
        schema_version: ACCOUNT_STORE_SCHEMA_VERSION,
        next_character_index: 0,
        game_shop_global_purchases,
        accounts,
        source_account_versions: BTreeMap::new(),
        source_save_versions: BTreeMap::new(),
        source_game_shop_global_version: None,
        source_game_shop_global_purchases: BTreeMap::new(),
    }
    .migrate_to_current_schema()
    .with_source_versions(versions))
}

fn load_game_shop_global_stock(
    client: &mut Client,
) -> Result<(BTreeMap<i32, u64>, Option<i64>), String> {
    client
        .query_opt(
            "SELECT purchases_json, store_version \
             FROM game_shop_global_stock WHERE state_key = 1",
            &[],
        )
        .map_err(|error| format!("postgres game-shop global-stock load failed: {error}"))?
        .map(|row| {
            let purchases_json: Value = row.get("purchases_json");
            let purchases =
                serde_json::from_value::<BTreeMap<i32, u64>>(purchases_json).map_err(|error| {
                    format!("postgres game-shop global-stock decode failed: {error}")
                })?;
            Ok::<_, String>((purchases, Some(row.get("store_version"))))
        })
        .transpose()
        .map(|value| value.unwrap_or_default())
}

fn save_account_store_to_postgres(
    database_url: String,
    store: AccountStore,
    mode: AccountStoreDatabaseMode,
) -> Result<AccountStoreSourceVersions, String> {
    save_account_store_to_postgres_with_pool(
        postgres_account_store_pool(&database_url),
        store,
        mode,
    )
}

fn save_account_store_to_postgres_with_pool(
    pool: Arc<PostgresAccountStoreConnectionPool>,
    store: AccountStore,
    mode: AccountStoreDatabaseMode,
) -> Result<AccountStoreSourceVersions, String> {
    std::thread::spawn(move || {
        let mut client = pool.connection()?;
        pool.ensure_migrated(&mut client)?;
        upsert_account_store_to_postgres(&mut client, &store, mode)
    })
    .join()
    .map_err(|_| "postgres account-store mirror thread panicked".to_string())?
}

fn upsert_account_store_to_postgres(
    client: &mut Client,
    store: &AccountStore,
    mode: AccountStoreDatabaseMode,
) -> Result<AccountStoreSourceVersions, String> {
    let mut account_ids = BTreeSet::new();
    account_ids.extend(store.accounts.keys().cloned());
    account_ids.extend(store.source_account_versions.keys().cloned());
    account_ids.extend(store.source_save_versions.keys().cloned());
    let account_ids = account_ids.into_iter().collect::<Vec<_>>();
    let plan = build_account_store_mutation_plan(
        store,
        store,
        AccountStoreMutationScope::AccountsWithGlobal(&account_ids),
        mode == AccountStoreDatabaseMode::Mirror,
    );
    write_account_store_mutation_plan_to_postgres(client, &plan, mode)
}

fn save_account_store_mutation_plan_to_postgres(
    database_url: String,
    plan: AccountStoreMutationPlan,
    mode: AccountStoreDatabaseMode,
) -> Result<AccountStoreSourceVersions, String> {
    save_account_store_mutation_plan_to_postgres_with_pool(
        postgres_account_store_pool(&database_url),
        plan,
        mode,
    )
}

fn save_account_store_mutation_plan_to_postgres_with_pool(
    pool: Arc<PostgresAccountStoreConnectionPool>,
    plan: AccountStoreMutationPlan,
    mode: AccountStoreDatabaseMode,
) -> Result<AccountStoreSourceVersions, String> {
    std::thread::spawn(move || {
        let mut client = pool.connection()?;
        pool.ensure_migrated(&mut client)?;
        write_account_store_mutation_plan_to_postgres(&mut client, &plan, mode)
    })
    .join()
    .map_err(|_| "postgres account-store mutation thread panicked".to_string())?
}

fn write_account_store_mutation_plan_to_postgres(
    client: &mut Client,
    plan: &AccountStoreMutationPlan,
    mode: AccountStoreDatabaseMode,
) -> Result<AccountStoreSourceVersions, String> {
    let mut transaction = client
        .transaction()
        .map_err(|error| format!("postgres account-store transaction failed: {error}"))?;
    let source_versions = execute_account_store_mutation_plan(plan, |operation| match operation {
        AccountStoreMutationOperation::GlobalStock(mutation) => {
            write_game_shop_global_stock_mutation(&mut transaction, mutation, mode)
                .map(AccountStoreMutationOperationResult::GlobalStock)
        }
        AccountStoreMutationOperation::Account {
            account_id,
            mutation,
        } => write_account_store_account_mutation(&mut transaction, account_id, mutation, mode),
    })?;
    transaction
        .commit()
        .map_err(|error| format!("postgres account-store commit failed: {error}"))?;
    Ok(source_versions)
}

fn write_account_store_account_mutation(
    transaction: &mut Transaction<'_>,
    account_id: &str,
    mutation: &AccountStoreAccountMutation,
    mode: AccountStoreDatabaseMode,
) -> Result<AccountStoreMutationOperationResult, String> {
    let current_account_version = transaction
        .query_opt(
            "SELECT store_version FROM accounts WHERE account_id = $1 FOR UPDATE",
            &[&account_id],
        )
        .map_err(|error| format!("postgres account lock failed for {account_id}: {error}"))?
        .map(|row| row.get::<_, i64>("store_version"));
    let current_save_versions = transaction
        .query(
            "SELECT character_index, save_version FROM character_saves \
             WHERE account_id = $1 ORDER BY character_index FOR UPDATE",
            &[&account_id],
        )
        .map_err(|error| format!("postgres character-save locks failed for {account_id}: {error}"))?
        .into_iter()
        .map(|row| {
            (
                row.get::<_, i32>("character_index"),
                row.get::<_, i64>("save_version"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    validate_account_store_account_mutation(
        account_id,
        mutation,
        current_account_version,
        &current_save_versions,
        mode,
    )?;

    let Some(account) = mutation.desired_account.as_ref() else {
        // infra/postgres/migrations/0001_core.sql defines both account and
        // character foreign keys with ON DELETE CASCADE.  Delete saves first
        // explicitly after their CAS validation, then delete the account; the
        // account delete cascades characters and remaining dependent rows.
        transaction
            .execute(
                "DELETE FROM character_saves WHERE account_id = $1",
                &[&account_id],
            )
            .map_err(|error| {
                format!("postgres character-save tombstone failed for {account_id}: {error}")
            })?;
        crate::db_projection::retain_character_projections(transaction, account_id, &[])?;
        let deleted = transaction
            .execute("DELETE FROM accounts WHERE account_id = $1", &[&account_id])
            .map_err(|error| {
                format!("postgres account tombstone failed for {account_id}: {error}")
            })?;
        if mode == AccountStoreDatabaseMode::SourceOfTruth
            && mutation.expected_version.is_some()
            && deleted != 1
        {
            return Err(format!(
                "stale postgres account-store delete for {account_id}: locked row disappeared"
            ));
        }
        return Ok(AccountStoreMutationOperationResult::Account {
            account_version: None,
            save_versions: BTreeMap::new(),
        });
    };

    let account_version = upsert_account_record(transaction, account_id, account, mode)?;
    let present_save_indices = account.saves.keys().copied().collect::<Vec<_>>();
    transaction
        .execute(
            "DELETE FROM character_saves WHERE account_id = $1 AND character_index <> ALL($2)",
            &[&account_id, &present_save_indices],
        )
        .map_err(|error| {
            format!("postgres character-save tombstone failed for {account_id}: {error}")
        })?;
    crate::db_projection::retain_character_projections(
        transaction,
        account_id,
        &present_save_indices,
    )?;

    let mut characters = account.characters.clone();
    for save in account.saves.values() {
        if !characters
            .iter()
            .any(|character| character.index == save.character.index)
        {
            characters.push(save.character.clone());
        }
    }
    characters.sort_by_key(|character| character.index);
    characters.dedup_by_key(|character| character.index);
    let present_character_indices = characters
        .iter()
        .map(|character| character.index)
        .collect::<Vec<_>>();
    transaction
        .execute(
            "DELETE FROM characters WHERE account_id = $1 AND character_index <> ALL($2)",
            &[&account_id, &present_character_indices],
        )
        .map_err(|error| {
            format!("postgres character tombstone failed for {account_id}: {error}")
        })?;
    for character in &characters {
        upsert_character_record(transaction, account_id, character)?;
    }

    let mut save_versions = BTreeMap::new();
    for (character_index, save) in &account.saves {
        let save_version =
            upsert_character_save_record(transaction, account_id, *character_index, save, mode)?;
        save_versions.insert(*character_index, save_version);
    }
    Ok(AccountStoreMutationOperationResult::Account {
        account_version: Some(account_version),
        save_versions,
    })
}

fn validate_account_store_account_mutation(
    account_id: &str,
    mutation: &AccountStoreAccountMutation,
    current_account_version: Option<i64>,
    current_save_versions: &BTreeMap<i32, i64>,
    mode: AccountStoreDatabaseMode,
) -> Result<(), String> {
    validate_source_account_write(mode, mutation.expected_version, current_account_version)
        .map_err(|reason| {
            format!("stale postgres account-store write for {account_id}: {reason}")
        })?;
    if mode != AccountStoreDatabaseMode::SourceOfTruth {
        return Ok(());
    }

    let mut save_indices = BTreeSet::new();
    save_indices.extend(mutation.saves.keys().copied());
    save_indices.extend(current_save_versions.keys().copied());
    for character_index in save_indices {
        let expected_version = mutation
            .saves
            .get(&character_index)
            .and_then(|save| save.expected_version);
        let current_version = current_save_versions.get(&character_index).copied();
        validate_source_save_write(expected_version, current_version).map_err(|reason| {
            format!(
                "stale postgres character-save write for {account_id}/{character_index}: {reason}"
            )
        })?;
    }
    Ok(())
}

fn validate_source_account_write(
    mode: AccountStoreDatabaseMode,
    expected_version: Option<i64>,
    current_version: Option<i64>,
) -> Result<(), String> {
    if mode != AccountStoreDatabaseMode::SourceOfTruth {
        return Ok(());
    }
    match (expected_version, current_version) {
        (Some(expected), Some(current)) if expected == current => Ok(()),
        (Some(expected), Some(current)) => Err(format!(
            "expected store_version {expected}, found {current}"
        )),
        (Some(expected), None) => Err(format!(
            "expected store_version {expected}, found no row (account deleted)"
        )),
        (None, Some(current)) => Err(format!(
            "expected no row for new account, found store_version {current}"
        )),
        (None, None) => Ok(()),
    }
}

fn validate_source_save_write(
    expected_version: Option<i64>,
    current_version: Option<i64>,
) -> Result<(), String> {
    match (expected_version, current_version) {
        (Some(expected), Some(current)) if expected == current => Ok(()),
        (Some(expected), Some(current)) => {
            Err(format!("expected save_version {expected}, found {current}"))
        }
        (Some(expected), None) => Err(format!("expected save_version {expected}, found no row")),
        (None, Some(current)) => Err(format!(
            "expected no row for new save, found save_version {current}"
        )),
        (None, None) => Ok(()),
    }
}

fn validate_source_game_shop_global_write(
    expected_version: Option<i64>,
    current_version: Option<i64>,
    expected_purchases: &BTreeMap<i32, u64>,
    current_purchases: Option<&BTreeMap<i32, u64>>,
) -> Result<(), String> {
    match (expected_version, current_version) {
        (Some(expected), Some(current)) if expected == current => {
            if current_purchases != Some(expected_purchases) {
                return Err(
                    "expected global-stock contents do not match the locked row".to_string()
                );
            }
            Ok(())
        }
        (Some(expected), Some(current)) => Err(format!(
            "expected store_version {expected}, found {current}"
        )),
        (Some(expected), None) => Err(format!("expected store_version {expected}, found no row")),
        (None, Some(current)) => Err(format!(
            "expected no row for new global stock, found store_version {current}"
        )),
        (None, None) if expected_purchases.is_empty() && current_purchases.is_none() => Ok(()),
        (None, None) => {
            Err("global-stock baseline has contents without an expected source row".to_string())
        }
    }
}

fn write_game_shop_global_stock_mutation(
    transaction: &mut Transaction<'_>,
    mutation: &AccountStoreGlobalStockMutation,
    mode: AccountStoreDatabaseMode,
) -> Result<Option<i64>, String> {
    let locked = transaction
        .query_opt(
            "SELECT purchases_json, store_version FROM game_shop_global_stock \
             WHERE state_key = 1 FOR UPDATE",
            &[],
        )
        .map_err(|error| format!("postgres game-shop global-stock lock failed: {error}"))?;
    if mode == AccountStoreDatabaseMode::SourceOfTruth {
        let current_version = locked
            .as_ref()
            .map(|row| row.get::<_, i64>("store_version"));
        let current_purchases = locked
            .as_ref()
            .map(|row| row.get::<_, Value>("purchases_json"))
            .map(serde_json::from_value::<BTreeMap<i32, u64>>)
            .transpose()
            .map_err(|error| format!("postgres game-shop global-stock decode failed: {error}"))?;
        validate_source_game_shop_global_write(
            mutation.expected_version,
            current_version,
            &mutation.expected_purchases,
            current_purchases.as_ref(),
        )
        .map_err(|reason| format!("stale postgres game-shop global-stock write: {reason}"))?;
    }

    let purchases_json = serde_json::to_value(&mutation.desired_purchases)
        .map_err(|error| format!("game-shop global-stock encode failed: {error}"))?;
    let increment_version = mode == AccountStoreDatabaseMode::SourceOfTruth;
    let row = transaction
        .query_one(
            "INSERT INTO game_shop_global_stock (state_key, purchases_json, store_version, updated_at) \
             VALUES (1, $1, 1, now()) \
             ON CONFLICT (state_key) DO UPDATE SET \
                purchases_json = EXCLUDED.purchases_json, \
                store_version = CASE WHEN $2 \
                    THEN game_shop_global_stock.store_version + 1 \
                    ELSE game_shop_global_stock.store_version END, \
                updated_at = now() \
             RETURNING store_version",
            &[&purchases_json, &increment_version],
        )
        .map_err(|error| format!("postgres game-shop global-stock upsert failed: {error}"))?;
    Ok(Some(row.get("store_version")))
}
fn upsert_account_record(
    client: &mut Transaction<'_>,
    account_id: &str,
    account: &AccountRecord,
    mode: AccountStoreDatabaseMode,
) -> Result<i64, String> {
    let raw_json = to_json(account)?;
    let account_id = account_id.to_string();
    let should_increment_version = mode == AccountStoreDatabaseMode::SourceOfTruth;
    let row = client
        .query_one(
            "INSERT INTO accounts (
                account_id,
                password_snapshot,
                storage_size,
                has_expanded_storage,
                expanded_storage_expiry_time_binary_datetime,
                storage_password_snapshot,
                storage_password_last_set_binary_datetime,
                is_banned,
                ban_reason,
                ban_until_ms,
                banned_at_ms,
                store_version,
                raw_json,
                updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,1,$12,now())
            ON CONFLICT (account_id) DO UPDATE SET
                password_snapshot = EXCLUDED.password_snapshot,
                storage_size = EXCLUDED.storage_size,
                has_expanded_storage = EXCLUDED.has_expanded_storage,
                expanded_storage_expiry_time_binary_datetime = EXCLUDED.expanded_storage_expiry_time_binary_datetime,
                storage_password_snapshot = EXCLUDED.storage_password_snapshot,
                storage_password_last_set_binary_datetime = EXCLUDED.storage_password_last_set_binary_datetime,
                is_banned = EXCLUDED.is_banned,
                ban_reason = EXCLUDED.ban_reason,
                ban_until_ms = EXCLUDED.ban_until_ms,
                banned_at_ms = EXCLUDED.banned_at_ms,
                store_version = CASE WHEN $13 THEN accounts.store_version + 1 ELSE accounts.store_version END,
                raw_json = EXCLUDED.raw_json,
                updated_at = now()
            RETURNING store_version",
            &[
                &account_id,
                &account.password,
                &(account.storage_size as i32),
                &account.has_expanded_storage,
                &account.expanded_storage_expiry_time_binary_datetime,
                &account.storage_password,
                &account.storage_password_last_set_binary_datetime,
                &account.is_banned,
                &account.ban_reason,
                &(account.ban_until_ms.map(|value| value as i64)),
                &(account.banned_at_ms.map(|value| value as i64)),
                &raw_json,
                &should_increment_version,
            ],
        )
        .map_err(|error| format!("postgres account upsert failed for {account_id}: {error}"))?;
    Ok(row.get("store_version"))
}

fn upsert_character_record(
    client: &mut Transaction<'_>,
    account_id: &str,
    character: &CharacterRecord,
) -> Result<(), String> {
    let raw_json = to_json(character)?;
    client
        .execute(
            "INSERT INTO characters (
                account_id,
                character_index,
                character_name,
                class,
                gender,
                level,
                raw_json,
                updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,now())
            ON CONFLICT (account_id, character_index) DO UPDATE SET
                character_name = EXCLUDED.character_name,
                class = EXCLUDED.class,
                gender = EXCLUDED.gender,
                level = EXCLUDED.level,
                raw_json = EXCLUDED.raw_json,
                updated_at = now()",
            &[
                &account_id,
                &character.index,
                &character.name,
                &enum_text(&character.class)?,
                &enum_text(&character.gender)?,
                &(character.level as i32),
                &raw_json,
            ],
        )
        .map_err(|error| {
            format!(
                "postgres character upsert failed for {account_id}/{}: {error}",
                character.index
            )
        })?;
    Ok(())
}

fn upsert_character_save_record(
    client: &mut Transaction<'_>,
    account_id: &str,
    character_index: i32,
    save: &CharacterSaveRecord,
    mode: AccountStoreDatabaseMode,
) -> Result<i64, String> {
    let snapshot_json = to_json(save)?;
    let should_increment_version = mode == AccountStoreDatabaseMode::SourceOfTruth;
    let stage5_systems_json = save
        .stage5_systems_json
        .as_deref()
        .map(serde_json::from_str::<Value>)
        .transpose()
        .map_err(|error| {
            format!("stage5 systems json decode failed for {account_id}/{character_index}: {error}")
        })?;
    let city_currencies_json = serde_json::to_value(&save.city_currencies).map_err(|error| {
        format!("city currencies json encode failed for {account_id}/{character_index}: {error}")
    })?;
    let row = client
        .query_one(
            "INSERT INTO character_saves (
                account_id,
                character_index,
                map_file_name,
                map_title,
                position_x,
                position_y,
                direction,
                hp,
                max_hp,
                mp,
                gold,
                credit,
                snapshot_json,
                stage5_systems_json,
                city_currencies,
                save_version,
                updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,1,now())
            ON CONFLICT (account_id, character_index) DO UPDATE SET
                map_file_name = EXCLUDED.map_file_name,
                map_title = EXCLUDED.map_title,
                position_x = EXCLUDED.position_x,
                position_y = EXCLUDED.position_y,
                direction = EXCLUDED.direction,
                hp = EXCLUDED.hp,
                max_hp = EXCLUDED.max_hp,
                mp = EXCLUDED.mp,
                gold = EXCLUDED.gold,
                credit = EXCLUDED.credit,
                snapshot_json = EXCLUDED.snapshot_json,
                stage5_systems_json = EXCLUDED.stage5_systems_json,
                city_currencies = EXCLUDED.city_currencies,
                save_version = CASE WHEN $16 THEN character_saves.save_version + 1 ELSE character_saves.save_version END,
                updated_at = now()
            RETURNING save_version",
            &[
                &account_id,
                &character_index,
                &save.map_file_name,
                &save.map_title,
                &(save.position.x as i32),
                &(save.position.y as i32),
                &enum_text(&save.direction)?,
                &save.hp,
                &save.max_hp,
                &save.mp,
                &(save.gold as i64),
                &(save.credit as i64),
                &snapshot_json,
                &stage5_systems_json,
                &city_currencies_json,
                &should_increment_version,
            ],
        )
        .map_err(|error| {
            format!(
                "postgres character save upsert failed for {account_id}/{character_index}: {error}"
            )
        })?;
    let save_version: i64 = row.get("save_version");

    // Maintain the normalized read-side projections inside the same transaction so
    // query models (admin/economy/anti-fraud) never drift from the authoritative
    // snapshot we just wrote.
    let projection = crate::db_projection::derive_character_projection(
        account_id,
        character_index,
        save,
        save_version,
        postgres_now_ms(),
    );
    crate::db_projection::write_character_projection(client, &projection)?;

    Ok(save_version)
}

fn postgres_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

fn to_json<T>(value: &T) -> Result<Value, String>
where
    T: Serialize,
{
    serde_json::to_value(value).map_err(|error| format!("json encode failed: {error}"))
}

fn enum_text<T>(value: &T) -> Result<String, String>
where
    T: Serialize,
{
    match serde_json::to_value(value).map_err(|error| format!("enum encode failed: {error}"))? {
        Value::String(value) => Ok(value),
        other => Ok(other.to_string()),
    }
}

fn starter_map_transfers() -> Vec<MapTransferRecord> {
    vec![MapTransferRecord {
        key: "starter-east-field-gate".to_string(),
        from_map_file_name: "0".to_string(),
        from_bounds: MapBounds {
            min_x: 339,
            max_x: 341,
            min_y: 268,
            max_y: 271,
        },
        to_map_file_name: "0".to_string(),
        to_map_title: "BichonProvince".to_string(),
        to_position: Point { x: 330, y: 270 },
        to_direction: MirDirection::Down,
        conquest_index: 0,
    }]
}

fn starter_safe_zones() -> Vec<SafeZoneRecord> {
    vec![SafeZoneRecord {
        map_file_name: "0".to_string(),
        bounds: MapBounds {
            min_x: 324,
            max_x: 332,
            min_y: 268,
            max_y: 273,
        },
    }]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorldEntityKind {
    SelfPlayer,
    Player,
    Monster,
    Npc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorldEntityDisposition {
    Friendly,
    Neutral,
    Hostile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemContainer {
    Bag1,
    Bag2,
    Quest,
    Belt,
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EquipmentSlot {
    Weapon,
    Armour,
    Helmet,
    Mount,
    Necklace,
    Torch,
    BraceletLeft,
    BraceletRight,
    RingLeft,
    RingRight,
    Amulet,
    Boots,
    Belt,
    Stone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuestStage {
    Available,
    InProgress,
    ReadyToTurnIn,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldEntitySpriteSnapshot {
    pub body_library: String,
    pub hair_library: Option<String>,
    pub weapon_library: Option<String>,
    pub weapon_library_secondary: Option<String>,
    pub frame_base_offset: u16,
    pub weapon_frame_offset: Option<u16>,
    pub alt_body_library: Option<String>,
    pub alt_hair_library: Option<String>,
    pub alt_weapon_library: Option<String>,
    pub alt_weapon_library_secondary: Option<String>,
    pub alt_frame_base_offset: Option<u16>,
    pub alt_weapon_frame_offset: Option<u16>,
    pub frame_count: u8,
    pub direction_stride: u16,
    /// Crystal `Libraries.Mounts[MountType]` (`Data\Mount\NN`). Present only when
    /// the entity is riding a mount (Crystal `PlayerObject.SetLibraries` line 604-607).
    pub mount_library: Option<String>,
    /// Frame offset for the mount layer, mirroring Crystal `DrawMount` which draws at
    /// `DrawFrame - 416 + MountOffset` (`PlayerObject.cs:5089`).
    pub mount_frame_offset: Option<u16>,
}

/// Convert Crystal's authoritative `ObjectPlayerInfo` appearance fields into
/// the renderer-neutral layered sprite contract shared by Gateway snapshots
/// and native clients. Keeping this conversion server-side prevents clients
/// from guessing another player's equipment or class-specific library route.
pub fn world_entity_sprite_from_object_player(
    info: &ObjectPlayerInfo,
) -> WorldEntitySpriteSnapshot {
    if info.transform_type >= 0 {
        let show_mount = !matches!(info.transform_type, 6 | 9);
        let transform_mount = info.riding_mount && info.mount_type > 6;
        return WorldEntitySpriteSnapshot {
            body_library: format!(
                "{}/{:02}",
                if transform_mount {
                    "TransformRide2"
                } else {
                    "Transform"
                },
                info.transform_type
            ),
            hair_library: None,
            weapon_library: None,
            weapon_library_secondary: None,
            frame_base_offset: 0,
            weapon_frame_offset: None,
            alt_body_library: None,
            alt_hair_library: None,
            alt_weapon_library: None,
            alt_weapon_library_secondary: None,
            alt_frame_base_offset: None,
            alt_weapon_frame_offset: None,
            frame_count: 4,
            direction_stride: 4,
            mount_library: (show_mount && info.riding_mount && info.mount_type >= 0)
                .then(|| format!("Mount/{:02}", info.mount_type)),
            mount_frame_offset: (show_mount && info.riding_mount && info.mount_type >= 0)
                .then_some(0),
        };
    }

    let armour_shape = u16::try_from(info.armour).unwrap_or(0);
    let weapon_shape = u16::try_from(info.weapon).ok();
    let hair_shape = u16::from(info.hair);
    let uses_assassin_weapon = matches!(weapon_shape, Some(shape) if (100..200).contains(&shape));
    let uses_assassin_alt_body = info.weapon < 0 || uses_assassin_weapon;
    let uses_archer_weapon = matches!(weapon_shape, Some(shape) if shape >= 200);
    let body_library = format!("CArmour/{armour_shape:02}");
    let hair_library = Some(format!("CHair/{hair_shape:02}"));
    let (alt_body_library, alt_hair_library) = match info.class {
        MirClass::Assassin if uses_assassin_alt_body => (
            Some(format!("AArmour/{armour_shape:02}")),
            Some(format!("AHair/{hair_shape:02}")),
        ),
        MirClass::Archer if uses_archer_weapon => (
            Some(format!("ARArmour/{armour_shape:02}")),
            Some(format!("ARHair/{hair_shape:02}")),
        ),
        _ => (None, None),
    };
    let (
        mut weapon_library,
        mut weapon_library_secondary,
        mut alt_weapon_library,
        mut alt_weapon_library_secondary,
    ) = match weapon_shape {
        Some(shape) if (100..200).contains(&shape) => {
            let index = shape - 100;
            (
                None,
                None,
                Some(format!("AWeapon/{index:02} R")),
                Some(format!("AWeapon/{index:02} L")),
            )
        }
        Some(shape) if shape >= 200 => {
            let index = shape - 200;
            (
                Some(format!("ARWeapon/{index:02}")),
                None,
                Some(format!("ARWeapon/{index:02} S")),
                None,
            )
        }
        Some(shape) => (Some(format!("CWeapon/{shape:02}")), None, None, None),
        None => (None, None, None, None),
    };
    let frame_base_offset = match info.gender {
        MirGender::Male => 0,
        MirGender::Female => 808,
    };
    let mut weapon_frame_offset = weapon_library.as_ref().map(|_| match info.gender {
        MirGender::Male => 0,
        MirGender::Female => 416,
    });
    let alt_frame_base_offset = match info.class {
        MirClass::Archer if uses_archer_weapon => Some(match info.gender {
            MirGender::Male => 0,
            MirGender::Female => 352,
        }),
        MirClass::Assassin if uses_assassin_alt_body => Some(match info.gender {
            MirGender::Male => 0,
            MirGender::Female => 512,
        }),
        _ => None,
    };
    let alt_weapon_frame_offset = alt_frame_base_offset;
    let (mount_library, mount_frame_offset) = if info.riding_mount && info.mount_type >= 0 {
        weapon_library = None;
        weapon_library_secondary = None;
        alt_weapon_library = None;
        alt_weapon_library_secondary = None;
        weapon_frame_offset = None;
        (
            Some(format!("Mount/{:02}", info.mount_type)),
            Some(frame_base_offset),
        )
    } else {
        (None, None)
    };

    WorldEntitySpriteSnapshot {
        body_library,
        hair_library,
        weapon_library,
        weapon_library_secondary,
        frame_base_offset,
        weapon_frame_offset,
        alt_body_library,
        alt_hair_library,
        alt_weapon_library,
        alt_weapon_library_secondary,
        alt_frame_base_offset,
        alt_weapon_frame_offset,
        frame_count: 4,
        direction_stride: 4,
        mount_library,
        mount_frame_offset,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldEntitySnapshot {
    pub object_id: u32,
    pub kind: WorldEntityKind,
    pub name: String,
    pub owner_name: Option<String>,
    /// Crystal monster AI id. The client needs AI 6 to reproduce the native
    /// green minimap radar colour; non-monster entities leave this unset.
    pub ai: Option<u8>,
    pub x: i32,
    pub y: i32,
    pub direction: MirDirection,
    pub class: Option<MirClass>,
    pub gender: Option<MirGender>,
    pub level: Option<u16>,
    pub hp: Option<i32>,
    pub max_hp: Option<i32>,
    /// Crystal object light encoding: radius is `light % 15`; players also use
    /// `light / 15` as the source-strength bucket.
    pub light: u8,
    pub name_colour_argb: i32,
    pub dead: bool,
    /// Authoritative player mount state. Remote/session-external players use
    /// the value carried by Crystal `ObjectPlayerInfo`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub riding_mount: Option<bool>,
    /// Crystal `CanRideAttack`, derived from authoritative mount type, riding
    /// state and Bells equipment. Self-only; missing means deny.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_mount_attack: Option<bool>,
    /// Crystal `HasClassWeapon`, derived from the active ECS equipment and
    /// class. Never populated from a client declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_class_weapon: Option<bool>,
    /// Whether Crystal Dazed poison is active in the authoritative buff set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dazed: Option<bool>,
    /// Whether the authoritative player is currently fishing. Remote players
    /// use the value carried by Crystal `ObjectPlayerInfo`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fishing: Option<bool>,
    pub disposition: WorldEntityDisposition,
    pub sprite: Option<WorldEntitySpriteSnapshot>,
    pub quest_ids: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldItemSnapshot {
    pub key: String,
    pub name: String,
    pub icon: u16,
    pub unique_id: u64,
    pub slot: u8,
    pub container: ItemContainer,
    pub quantity: u32,
    pub description: String,
    pub durability_current: Option<u16>,
    pub durability_max: Option<u16>,
    /// Authoritative value awarded for selling one unit to a compatible NPC.
    #[serde(default)]
    pub sell_value: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equip_slot: Option<EquipmentSlot>,
    pub grade: ItemGrade,
    pub added_attack: i32,
    pub added_defence: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentItemSnapshot {
    pub slot: EquipmentSlot,
    pub key: String,
    pub quantity: u32,
    pub name: String,
    pub icon: u16,
    pub shape: Option<u16>,
    pub description: String,
    pub durability_current: u16,
    pub durability_max: u16,
    pub grade: ItemGrade,
    pub attack: i32,
    pub defence: i32,
    pub added_attack: i32,
    pub added_defence: i32,
    pub added_luck: i32,
    pub socket_slots: u8,
    pub sealed_expiry_time_binary_datetime: i64,
    pub sealed_next_time_binary_datetime: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroundDropSnapshot {
    pub object_id: u32,
    pub name: String,
    pub name_colour_argb: i32,
    /// Crystal item `Image` index (`/original-ui/Items/{icon}.png`); 0 = no icon (e.g. gold).
    #[serde(default)]
    pub icon: u16,
    pub x: i32,
    pub y: i32,
    pub quantity: u32,
    pub source_monster: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_object_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ownership_remaining_ticks: Option<u64>,
    pub loot: GroundDropLootSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroundDropItemPayload {
    /// Complete Crystal item identity frozen when the ground object is created.
    pub item: UserItem,
    /// False means the payload still needs an authoritative UID before commit.
    #[serde(default)]
    pub uid_assigned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GroundDropLootSnapshot {
    Gold {
        amount: u32,
    },
    InventoryItem {
        key: String,
        name: String,
        description: String,
        weight: u16,
        durability_current: Option<u16>,
        durability_max: Option<u16>,
        added_attack: i32,
        added_defence: i32,
        added_stats: Vec<UserItemStat>,
        cursed: bool,
        socket_slots: u8,
        show_group_pickup: bool,
        /// Added after the legacy display/template projection. Old checkpoints
        /// decode as `None`; newly produced drops must carry `Some`.
        #[serde(
            default,
            rename = "exactItem",
            alias = "exact_item",
            skip_serializing_if = "Option::is_none"
        )]
        exact_item: Option<GroundDropItemPayload>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestObjectiveSnapshot {
    pub label: String,
    pub current: u32,
    pub required: u32,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestSnapshot {
    pub quest_id: i32,
    pub title: String,
    pub summary: String,
    pub objective: String,
    pub progress_label: String,
    pub tracker: String,
    pub stage: QuestStage,
    pub current: u32,
    pub required: u32,
    pub reward_preview: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objectives: Vec<QuestObjectiveSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcDialogSnapshot {
    pub npc_object_id: u32,
    pub npc_name: String,
    pub title: String,
    pub body: Vec<String>,
    pub footer: String,
    pub links: Vec<NpcDialogLinkSnapshot>,
    pub input: Option<NpcDialogInputSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcDialogLinkSnapshot {
    pub text: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcDialogInputSnapshot {
    pub target: String,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSnapshot {
    pub key: String,
    pub name: String,
    pub description: String,
    pub spell: Option<String>,
    pub cast_kind: String,
    pub offensive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mp_cost: Option<u32>,
    pub level: u8,
    pub experience: u16,
    pub hotkey: u8,
    pub delay_ms: i64,
    pub cast_time_ms: i64,
    pub cooldown_remaining_ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpcScriptDiagnosticSnapshot {
    pub script_key: String,
    pub label: String,
    pub line_number: usize,
    pub command: String,
    pub message: String,
}

/// A buff stat rendered for the browser buff window: keeps Crystal's raw `stat`
/// byte (backward compatible) and adds the `label` (Crystal `Stat` enum name,
/// `Crystal/Shared/Data/Stat.cs`) so the window can show `{label, value}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffStatSnapshot {
    pub stat: u8,
    pub label: String,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffSnapshot {
    pub key: String,
    pub name: String,
    pub description: String,
    pub remaining_ticks: u32,
    pub attack_bonus: i32,
    pub defence_bonus: i32,
    /// Numeric Crystal `BuffType` (mirrors `S.AddBuff` `Type`); `None` for buffs
    /// without a Crystal type mapping. Matches the browser buff contract `type?`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buff_type: Option<u8>,
    /// Milliseconds remaining (Crystal maps `Expire`→remaining client-side); this
    /// is `remaining_ticks * 1000`, the same value `S.AddBuff.expire_time` carries
    /// on the simulation's packet path.
    pub remaining_ms: u64,
    /// True for non-expiring buffs (Crystal `BuffStackType.Infinite`).
    pub infinite: bool,
    /// Per-stat effects rendered as `{stat, label, value}` for the buff window.
    #[serde(default)]
    pub stats: Vec<BuffStatSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5SystemsState {
    pub group: Stage5GroupState,
    pub guild: Stage5GuildState,
    pub social: Stage5SocialState,
    #[serde(default)]
    pub relationship: Stage5RelationshipState,
    #[serde(default)]
    pub mentor: Stage5MentorState,
    pub mail: Vec<Stage5MailMessage>,
    /// Crystal `CharacterInfo.GSpurchases`: cumulative purchase quantities for
    /// finite products with individual (`iStock`) limits. This is character
    /// save state, not an item-stack counter.
    #[serde(default)]
    pub game_shop_individual_purchases: BTreeMap<i32, u64>,
    /// Durable proof that the matching external ledger event has already been
    /// materialized into this private character snapshot. The exact event ID is
    /// recorded in the same save as the resulting gold/item mutation.
    #[serde(default)]
    pub economy_projection_event_ids: BTreeSet<String>,
    pub trade: Option<Stage5TradeState>,
    pub auction: Vec<Stage5AuctionListing>,
    #[serde(default)]
    pub refine: Stage5RefineState,
    pub conquest: Stage5ConquestState,
    #[serde(default)]
    pub guild_territory: Stage5GuildTerritoryState,
    pub hero: Option<Stage5HeroState>,
    #[serde(default)]
    pub hero_learned_magics: Vec<Stage5HeroMagicState>,
    pub profession: Stage5ProfessionState,
    #[serde(default)]
    pub appearance: Stage5AppearanceState,
    #[serde(default)]
    pub name_lists: Vec<String>,
    #[serde(default)]
    pub intelligent_creatures: Vec<ClientIntelligentCreature>,
    #[serde(default)]
    pub item_rental: Stage5ItemRentalSnapshot,
    /// Player attack mode (Crystal `PlayerObject.AMode`, set by `C.ChangeAMode`).
    /// Crystal `AttackMode`: 0 Peace, 1 Group, 2 Guild, 3 EnemyGuild, 4 RedBrown,
    /// 5 All. Persisted for snapshot fidelity; server echoes `S.ChangeAMode`.
    #[serde(default)]
    pub attack_mode: u8,
    /// Player pet command mode (Crystal `PlayerObject.PMode`, set by
    /// `C.ChangePMode`). Crystal `PetMode`: 0 Both, 1 MoveOnly, 2 AttackOnly,
    /// 3 None, 4 FocusMasterTarget. Persisted for snapshot fidelity; server
    /// echoes `S.ChangePMode`.
    #[serde(default)]
    pub pet_mode: u8,
    /// Elapsed world ticks toward the next lawful PK-point decay.
    #[serde(default)]
    pub pk_decay_elapsed_ticks: u64,
}

impl Default for Stage5SystemsState {
    fn default() -> Self {
        Self {
            group: Stage5GroupState::default(),
            guild: Stage5GuildState::default(),
            social: Stage5SocialState::default(),
            relationship: Stage5RelationshipState::default(),
            mentor: Stage5MentorState::default(),
            mail: Vec::new(),
            game_shop_individual_purchases: BTreeMap::new(),
            economy_projection_event_ids: BTreeSet::new(),
            trade: None,
            auction: Vec::new(),
            refine: Stage5RefineState::default(),
            conquest: Stage5ConquestState::default(),
            guild_territory: Stage5GuildTerritoryState::default(),
            hero: None,
            hero_learned_magics: Vec::new(),
            profession: Stage5ProfessionState::default(),
            appearance: Stage5AppearanceState::default(),
            name_lists: Vec::new(),
            intelligent_creatures: Vec::new(),
            item_rental: Stage5ItemRentalSnapshot::default(),
            attack_mode: 0,
            pet_mode: 0,
            pk_decay_elapsed_ticks: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stage5ItemRentalSnapshot {
    pub partner_name: Option<String>,
    pub fee: u32,
    pub days: u32,
    pub has_deposited_item: bool,
    pub deposited_item_name: Option<String>,
    pub gold_locked: bool,
    pub item_locked: bool,
    pub record_count: usize,
    pub rented_items: Vec<Stage5ItemRentalRecordSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stage5ItemRentalRecordSnapshot {
    pub item_id: u64,
    pub item_name: String,
    pub renting_player_name: String,
    pub item_return_date_binary_datetime: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5GroupState {
    #[serde(default = "default_stage5_allow_group")]
    pub allow_group: bool,
    pub members: Vec<String>,
    pub loot_mode: String,
}

const fn default_stage5_allow_group() -> bool {
    true
}

impl Default for Stage5GroupState {
    fn default() -> Self {
        Self {
            allow_group: true,
            members: Vec::new(),
            loot_mode: "free".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stage5GuildState {
    pub name: String,
    pub members: Vec<String>,
    pub rank: String,
    pub permissions: Vec<String>,
    pub chat_log: Vec<String>,
    #[serde(default)]
    pub known_guilds: Vec<String>,
    #[serde(default)]
    pub active_wars: Vec<String>,
    #[serde(default)]
    pub active_war_ticks_remaining: BTreeMap<String, u64>,
    // Crystal stores AllyGuilds/AllyCount only on GuildObject runtime state. Keep these
    // serializable for Web snapshots, but do not rehydrate them from saved Stage 5 JSON.
    #[serde(default, skip_deserializing)]
    pub allied_guilds: Vec<String>,
    #[serde(default, skip_deserializing)]
    pub ally_count: u32,
    #[serde(default, skip_deserializing)]
    pub alliance_broadcasts: Vec<String>,
    #[serde(default)]
    pub war_broadcasts: Vec<String>,
    #[serde(default)]
    pub notice: Vec<String>,
    #[serde(default)]
    pub storage_gold: u32,
    #[serde(default)]
    pub storage_items: BTreeMap<u8, String>,
    #[serde(default)]
    pub storage_item_states: BTreeMap<u8, String>,
    #[serde(default)]
    pub storage_item_users: BTreeMap<u8, i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stage5SocialState {
    pub friends: Vec<String>,
    pub blocked: Vec<String>,
    #[serde(default)]
    pub memos: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5RelationshipState {
    #[serde(default = "default_stage5_allow_marriage")]
    pub allow_marriage: bool,
    #[serde(default)]
    pub partner_name: String,
    #[serde(default)]
    pub married_date_binary_datetime: i64,
    #[serde(default)]
    pub map_name: String,
    #[serde(default)]
    pub married_days: i16,
    #[serde(default)]
    pub pending_request_from: Option<String>,
    #[serde(default)]
    pub pending_divorce_from: Option<String>,
}

const fn default_stage5_allow_marriage() -> bool {
    true
}

impl Default for Stage5RelationshipState {
    fn default() -> Self {
        Self {
            allow_marriage: true,
            partner_name: String::new(),
            married_date_binary_datetime: 0,
            map_name: String::new(),
            married_days: 0,
            pending_request_from: None,
            pending_divorce_from: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5MentorState {
    #[serde(default = "default_stage5_allow_mentor")]
    pub allow_mentor: bool,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub level: u16,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub mentee_exp: i64,
    #[serde(default)]
    pub pending_request_from: Option<String>,
    #[serde(default)]
    pub pending_request_level: u16,
}

const fn default_stage5_allow_mentor() -> bool {
    true
}

impl Default for Stage5MentorState {
    fn default() -> Self {
        Self {
            allow_mentor: true,
            name: String::new(),
            level: 0,
            online: false,
            mentee_exp: 0,
            pending_request_from: None,
            pending_request_level: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5MailMessage {
    pub id: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub delivery_nonce: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub gold: u32,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub item_states_json: Vec<String>,
    #[serde(default)]
    pub opened: bool,
    #[serde(default)]
    pub locked: bool,
    pub claimed: bool,
    pub deleted: bool,
}

/// Generate an opaque, persistent identity for one logical mail delivery.
/// Mailbox IDs remain the client-facing address and may be re-keyed while an
/// incoming delivery is merged. Clients address mail only by `id` and must not
/// depend on this nonce.
pub fn new_stage5_mail_delivery_nonce() -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    let mut nonce = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        nonce.push(HEX[usize::from(byte >> 4)] as char);
        nonce.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    nonce
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage5MailTargetKind {
    Account,
    Character,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5MailDelivery {
    pub target_kind: Stage5MailTargetKind,
    pub target_id: String,
    pub from: String,
    pub subject: String,
    pub body: String,
    pub gold: u32,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5MailDeliveryReceipt {
    pub delivered_count: usize,
    pub mail_ids: Vec<u32>,
}

pub fn deliver_stage5_system_mail(
    config: &SimulationConfig,
    delivery: Stage5MailDelivery,
) -> Result<Stage5MailDeliveryReceipt, String> {
    let expected_targets = {
        let store = config
            .account_store
            .lock()
            .map_err(|_| "account store mutex poisoned".to_string())?;
        resolve_stage5_mail_targets(&store, &delivery)?
    };
    if expected_targets.is_empty() {
        return Ok(Stage5MailDeliveryReceipt {
            delivered_count: 0,
            mail_ids: Vec::new(),
        });
    }

    let account_ids = expected_targets
        .iter()
        .map(|(account_id, _)| account_id.clone())
        .collect::<Vec<_>>();
    config.commit_account_store_transaction(&account_ids, move |store| {
        let current_targets = resolve_stage5_mail_targets(store, &delivery)?;
        if current_targets != expected_targets {
            return Err("stage5 mail targets changed before durable commit".to_string());
        }

        let mut mail_ids = Vec::with_capacity(current_targets.len());
        for (account_id, character_index) in current_targets {
            let account = store
                .accounts
                .get_mut(&account_id)
                .ok_or_else(|| format!("account not found: {account_id}"))?;
            let character = account
                .characters
                .iter()
                .find(|character| character.index == character_index)
                .cloned()
                .ok_or_else(|| {
                    format!("character not found: {account_id}:{character_index}")
                })?;
            // A character record is the authority for initializing its own save;
            // this never creates an account or a synthetic default character.
            let save = account
                .saves
                .entry(character_index)
                .or_insert_with(|| CharacterSaveRecord::new(character.clone()));
            let mut systems = match save.stage5_systems_json.as_deref() {
                Some(state) => serde_json::from_str::<Stage5SystemsState>(state).map_err(|error| {
                    format!(
                        "failed to decode stage5 systems for {account_id}/{character_index}: {error}"
                    )
                })?,
                None => Stage5SystemsState::default(),
            };
            let id = systems
                .mail
                .iter()
                .map(|mail| mail.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            systems.mail.push(Stage5MailMessage {
                id,
                delivery_nonce: new_stage5_mail_delivery_nonce(),
                from: delivery.from.clone(),
                to: character.name,
                subject: delivery.subject.clone(),
                body: delivery.body.clone(),
                gold: delivery.gold,
                items: delivery.items.clone(),
                item_states_json: Vec::new(),
                opened: false,
                locked: false,
                claimed: false,
                deleted: false,
            });
            save.stage5_systems_json = Some(
                serde_json::to_string(&systems)
                    .map_err(|error| format!("failed to encode stage5 systems: {error}"))?,
            );
            mail_ids.push(id);
        }

        Ok(Stage5MailDeliveryReceipt {
            delivered_count: mail_ids.len(),
            mail_ids,
        })
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountBanReceipt {
    pub account_id: String,
    pub reason: String,
    pub ban_until_ms: Option<u64>,
    pub banned_at_ms: u64,
}

pub fn ban_account_in_store(
    config: &SimulationConfig,
    account_id: &str,
    duration_seconds: Option<u64>,
    reason: &str,
) -> Result<AccountBanReceipt, String> {
    let canonical_account_id = account_id.trim();
    if canonical_account_id.is_empty() {
        return Err("account_id is required".to_string());
    }
    if canonical_account_id != account_id {
        return Err("account_id must not contain leading or trailing whitespace".to_string());
    }

    let banned_at_ms = unix_now_ms();
    let ban_until_ms =
        duration_seconds.map(|seconds| banned_at_ms.saturating_add(seconds.saturating_mul(1000)));
    let reason = if reason.trim().is_empty() {
        "Admin account ban".to_string()
    } else {
        reason.trim().to_string()
    };
    let account_id = account_id.to_string();
    let touched_accounts = vec![account_id.clone()];

    config.commit_account_store_transaction(&touched_accounts, |store| {
        let account = store
            .accounts
            .get_mut(&account_id)
            .ok_or_else(|| format!("account not found: {account_id}"))?;
        account.is_banned = true;
        account.ban_reason = reason.clone();
        account.ban_until_ms = ban_until_ms;
        account.banned_at_ms = Some(banned_at_ms);
        Ok(())
    })?;

    Ok(AccountBanReceipt {
        account_id,
        reason,
        ban_until_ms,
        banned_at_ms,
    })
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn resolve_stage5_mail_targets(
    store: &AccountStore,
    delivery: &Stage5MailDelivery,
) -> Result<Vec<(String, i32)>, String> {
    match delivery.target_kind {
        Stage5MailTargetKind::Account => {
            let account = store
                .accounts
                .get(&delivery.target_id)
                .ok_or_else(|| format!("account not found: {}", delivery.target_id))?;
            Ok(account
                .characters
                .iter()
                .map(|character| (delivery.target_id.clone(), character.index))
                .collect())
        }
        Stage5MailTargetKind::Character => {
            let mut targets = Vec::new();
            for (account_id, account) in &store.accounts {
                for character in &account.characters {
                    if stage5_mail_character_matches(&delivery.target_id, account_id, character) {
                        targets.push((account_id.clone(), character.index));
                    }
                }
            }
            if targets.is_empty() {
                Err(format!("character not found: {}", delivery.target_id))
            } else {
                Ok(targets)
            }
        }
        Stage5MailTargetKind::Global => Ok(store
            .accounts
            .iter()
            .flat_map(|(account_id, account)| {
                account
                    .characters
                    .iter()
                    .map(|character| (account_id.clone(), character.index))
                    .collect::<Vec<_>>()
            })
            .collect()),
    }
}

fn stage5_mail_character_matches(
    target_id: &str,
    account_id: &str,
    character: &CharacterRecord,
) -> bool {
    target_id.eq_ignore_ascii_case(&character.name)
        || target_id == character.index.to_string()
        || target_id == format!("{account_id}:{}", character.index)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5TradeState {
    /// Opaque identity for this one participant's confirmation attempt. The
    /// pair of participant nonces forms the durable settlement identity, so two
    /// otherwise identical legal trades remain distinct.
    #[serde(default)]
    pub settlement_nonce: String,
    pub partner: String,
    pub offered_items: Vec<String>,
    #[serde(default)]
    pub offered_slots: BTreeMap<u8, u8>,
    /// Unique id of the item deposited into each trade slot, captured at deposit
    /// time. Confirm/delivery verify the live item at the slot's inventory index
    /// still has this id, so an item swapped in after depositing cannot be
    /// handed over in place of the one the partner agreed to (F-07).
    #[serde(default)]
    pub offered_unique_ids: BTreeMap<u8, u64>,
    pub offered_gold: u32,
    /// Currency the offered amount is denominated in (gold by default; a city
    /// token when the player picks one in the trade window). Net-new field —
    /// defaults to gold so existing trade state decodes unchanged.
    #[serde(default)]
    pub offered_currency: CurrencyKind,
    pub accepted: bool,
    #[serde(default)]
    pub locked: bool,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5AuctionListing {
    pub id: u32,
    pub seller: String,
    pub item_key: String,
    pub price: u32,
    /// Currency the listing is priced in (gold by default; a city token when
    /// the seller lists for one). Net-new field — defaults to gold.
    #[serde(default)]
    pub currency: CurrencyKind,
    pub sold: bool,
    pub cancelled: bool,
    #[serde(default)]
    pub expired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Stage5RefineState {
    pub slots: BTreeMap<u8, String>,
    pub current_item: Option<String>,
    pub refining: bool,
    pub ready: bool,
    /// Unique id of the weapon currently "in the oven" (set at RefineItem).
    pub pending_unique_id: u64,
    /// Success chance (0-100) computed from the deposited ingredients at
    /// RefineItem time and rolled against at CheckRefine.
    pub pending_chance: u8,
    /// Target stat code (Crystal MaxDC/MaxMC/MaxSC) the refine will add on
    /// success, chosen from the ingredients' bias and the weapon's stats.
    pub pending_stat: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Stage5ConquestState {
    pub castle_owner: String,
    pub active_wars: Vec<String>,
    pub event_log: Vec<String>,
    pub tax_rate_percent: u8,
    pub gold: u32,
    pub guards: Vec<u8>,
    pub walls: Vec<u8>,
    pub gates: Vec<u8>,
    pub open_gates: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5GuildTerritoryState {
    pub owned: bool,
    pub map_file_name: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub leader: String,
    #[serde(default)]
    pub leader2: String,
    #[serde(default)]
    pub price: i32,
    pub rental_days_left: u32,
    #[serde(default)]
    pub begin: i32,
    pub recall_log: Vec<String>,
}

impl Default for Stage5GuildTerritoryState {
    fn default() -> Self {
        Self {
            owned: false,
            map_file_name: "GA0".to_string(),
            owner: String::new(),
            leader: String::new(),
            leader2: String::new(),
            price: 0,
            rental_days_left: 0,
            begin: 0,
            recall_log: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stage5AppearanceState {
    pub hair: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5HeroState {
    pub name: String,
    pub level: u16,
    #[serde(default = "default_stage5_hero_class")]
    pub class: MirClass,
    #[serde(default = "default_stage5_hero_gender")]
    pub gender: MirGender,
    pub behaviour: u8,
    #[serde(default)]
    pub experience: u32,
    #[serde(default)]
    pub spawned: bool,
    #[serde(default)]
    pub auto_pot: bool,
    #[serde(default)]
    pub auto_hp_percent: u8,
    #[serde(default)]
    pub auto_mp_percent: u8,
    #[serde(default)]
    pub hp_item_index: i32,
    #[serde(default)]
    pub mp_item_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage5HeroMagicState {
    pub spell: Spell,
    #[serde(default)]
    pub level: u8,
    #[serde(default)]
    pub key: u8,
    #[serde(default)]
    pub experience: u16,
}

fn default_stage5_hero_class() -> MirClass {
    MirClass::Warrior
}

fn default_stage5_hero_gender() -> MirGender {
    MirGender::Male
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stage5ProfessionState {
    pub mining_level: u8,
    pub ore: u32,
    pub crafted_items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapTransferSnapshot {
    pub key: String,
    pub map_file_name: String,
    pub bounds: MapBounds,
    pub to_map_file_name: String,
    pub to_map_title: String,
    pub to_position: Point,
    pub to_direction: MirDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldSnapshot {
    pub tick: u64,
    pub map_title: Option<String>,
    pub map_file_name: Option<String>,
    pub in_safe_zone: bool,
    pub light_setting: u8,
    pub player_object_id: Option<u32>,
    pub player_hp: Option<i32>,
    pub player_max_hp: Option<i32>,
    pub player_mp: Option<i32>,
    pub player_max_mp: Option<i32>,
    #[serde(default)]
    pub player_pk_points: i32,
    pub player_experience: i64,
    pub player_max_experience: i64,
    pub gold: u32,
    pub credit: u32,
    /// Net-new per-city reputation currency balances, keyed by city
    /// (`"feitian"`, `"bichon"`). Every known city is present (defaulting to 0)
    /// so the HUD can render a stable row set. Browser reads this off the
    /// `worldSnapshot` payload.
    #[serde(default)]
    pub city_currencies: BTreeMap<String, u32>,
    pub current_weight: u16,
    pub max_weight: u16,
    pub free_bag_slots: u16,
    pub max_bag_slots: u16,
    pub storage_size: u16,
    pub has_expanded_storage: bool,
    pub has_storage_password: bool,
    pub require_storage_password: bool,
    pub storage_password_last_set_binary_datetime: i64,
    pub expanded_storage_expiry_time_binary_datetime: i64,
    pub scene_view: Option<SceneView>,
    pub terrain_patches: Vec<TerrainPatchTemplate>,
    pub decor_objects: Vec<DecorObjectTemplate>,
    pub entities: Vec<WorldEntitySnapshot>,
    pub ground_drops: Vec<GroundDropSnapshot>,
    pub belt_items: Vec<WorldItemSnapshot>,
    pub inventory_items: Vec<WorldItemSnapshot>,
    #[serde(default)]
    pub hero_inventory_items: Vec<WorldItemSnapshot>,
    #[serde(default)]
    pub storage_items: Vec<WorldItemSnapshot>,
    pub equipment_items: Vec<EquipmentItemSnapshot>,
    pub quest_log: Vec<QuestSnapshot>,
    pub active_npc_dialog: Option<NpcDialogSnapshot>,
    pub npc_script_diagnostics: Vec<NpcScriptDiagnosticSnapshot>,
    pub known_skills: Vec<SkillSnapshot>,
    pub active_buffs: Vec<BuffSnapshot>,
    pub stage5_systems: Stage5SystemsState,
    pub map_transfers: Vec<MapTransferSnapshot>,
    pub interaction_hints: Vec<String>,
}

/// Lossless server snapshots retain exact ground-item identity. Serialize this
/// explicit view only at untrusted client boundaries.
#[derive(Debug, Clone, Copy)]
pub struct WorldSnapshotClientView<'a> {
    snapshot: &'a WorldSnapshot,
}

impl WorldSnapshot {
    pub fn client_view(&self) -> WorldSnapshotClientView<'_> {
        WorldSnapshotClientView { snapshot: self }
    }
}

impl Serialize for WorldSnapshotClientView<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut value = serde_json::to_value(self.snapshot).map_err(serde::ser::Error::custom)?;
        if let Some(drops) = value.get_mut("groundDrops").and_then(Value::as_array_mut) {
            for drop in drops {
                if let Some(loot) = drop.get_mut("loot").and_then(Value::as_object_mut) {
                    loot.remove("exactItem");
                    loot.remove("exact_item");
                }
            }
        }
        value.serialize(serializer)
    }
}

#[cfg(test)]
#[path = "config_refresh_fail_closed_tests.rs"]
mod config_refresh_fail_closed_tests;

#[cfg(test)]
#[path = "config_account_store_durability_tests.rs"]
mod config_account_store_durability_tests;
