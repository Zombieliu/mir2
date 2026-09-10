//! Canonical guild storage. This does not promote legacy personal Stage5 guild data.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedGuildMember {
    #[serde(default)]
    pub membership_epoch: u64,
    pub identity: Stage5FriendIdentity,
    pub name: String,
    pub rank_index: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedGuildRank {
    pub index: u8,
    pub name: String,
    pub options: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedGuildStoredItem {
    /// Exact serialized runtime ItemState (including socket children and UID).
    /// This is never reconstructed from a name or the client UserItem projection.
    pub item_state_json: String,
    pub depositor_character_index: i32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedGuildBuff {
    pub id: i32,
    pub active: bool,
    pub remaining_minutes: i32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedGuildRecord {
    pub id: String,
    pub name: String,
    pub revision: u64,
    pub level: u8,
    pub experience: u64,
    pub spare_points: u8,
    pub gold: u32,
    pub ranks: Vec<SharedGuildRank>,
    pub members: Vec<SharedGuildMember>,
    pub notice: Vec<String>,
    /// Crystal's fixed 112 cells, represented sparsely without losing full item state.
    pub storage: BTreeMap<u8, SharedGuildStoredItem>,
    pub buffs: BTreeMap<i32, SharedGuildBuff>,
    pub last_buff_tick_ms: u64,
    pub experience_receipts: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub experience_receipt_payloads: BTreeMap<String, String>,
}
impl SharedGuildRecord {
    /// Symmetric canonical key deliberately closes Crystal's asymmetric space lookup.
    pub fn name_key(&self) -> String {
        canonical_guild_name(&self.name)
    }
    pub fn member(&self, identity: &Stage5FriendIdentity) -> Option<&SharedGuildMember> {
        self.members
            .iter()
            .find(|member| &member.identity == identity)
    }
}
pub(super) fn canonical_guild_name(name: &str) -> String {
    // Uppercase one scalar at a time without Unicode expansion (OrdinalIgnoreCase).
    name.chars()
        .filter(|c| *c != ' ')
        .map(|c| {
            let mut upper = c.to_uppercase();
            let first = upper.next().unwrap_or(c);
            if upper.next().is_none() {
                first
            } else {
                c
            }
        })
        .collect()
}
#[derive(Debug, Clone)]
pub(super) struct GuildMutation {
    pub expected_version: Option<i64>,
    pub desired: Option<SharedGuildRecord>,
}
pub(super) fn build_guild_mutations(
    original: &AccountStore,
    desired: &AccountStore,
    scope: AccountStoreMutationScope<'_>,
) -> BTreeMap<String, GuildMutation> {
    let ids: BTreeSet<String> = match scope {
        AccountStoreMutationScope::AccountsWithGuilds { guild_ids, .. } | AccountStoreMutationScope::AccountsWithGlobalAndGuilds { guild_ids, .. } | AccountStoreMutationScope::AccountsWithHeroesAndGuilds { guild_ids, .. } => {
            guild_ids.iter().cloned().collect()
        }
        AccountStoreMutationScope::FullRestore => original
            .shared_guilds
            .keys()
            .chain(original.source_guild_versions.keys())
            .chain(desired.shared_guilds.keys())
            .cloned()
            .collect(),
        _ => BTreeSet::new(),
    };
    // Emit every explicitly scoped guild, including deletes and mirror compensation.
    // A rollback cannot infer a newly inserted/deleted row solely from old source metadata.
    ids.into_iter()
        .map(|id| {
            let mutation = GuildMutation {
                expected_version: original.source_guild_versions.get(&id).copied(),
                desired: desired.shared_guilds.get(&id).cloned(),
            };
            (id, mutation)
        })
        .collect()
}
pub(super) fn validate_guild_scope(
    original: &AccountStore,
    staged: &AccountStore,
    scope: AccountStoreMutationScope<'_>,
) -> Result<(), AccountStoreTransactionScopeError> {
    if matches!(scope, AccountStoreMutationScope::FullRestore) {
        return validate_complete_guild_state(staged)
            .map_err(AccountStoreTransactionScopeError::InvalidGuildState);
    }
    let allowed: BTreeSet<&str> = match scope {
        AccountStoreMutationScope::AccountsWithGuilds { guild_ids, .. } | AccountStoreMutationScope::AccountsWithGlobalAndGuilds { guild_ids, .. } | AccountStoreMutationScope::AccountsWithHeroesAndGuilds { guild_ids, .. } => {
            guild_ids.iter().map(String::as_str).collect()
        }
        _ => BTreeSet::new(),
    };
    for id in original
        .shared_guilds
        .keys()
        .chain(staged.shared_guilds.keys())
    {
        if !allowed.contains(id.as_str())
            && original.shared_guilds.get(id) != staged.shared_guilds.get(id)
        {
            return Err(AccountStoreTransactionScopeError::OutOfScopeGuildChanged {
                guild_id: id.clone(),
            });
        }
    }
    let authorized_accounts: BTreeSet<&str> = match scope {
        AccountStoreMutationScope::AccountsWithGuilds { account_ids, .. } | AccountStoreMutationScope::AccountsWithGlobalAndGuilds { account_ids, .. } | AccountStoreMutationScope::AccountsWithHeroesAndGuilds { account_ids, .. } => {
            account_ids.iter().map(String::as_str).collect()
        }
        _ => BTreeSet::new(),
    };
    for id in original
        .shared_guilds
        .keys()
        .chain(staged.shared_guilds.keys())
    {
        let old_members: BTreeSet<(&str, i32)> = original
            .shared_guilds
            .get(id)
            .into_iter()
            .flat_map(|guild| guild.members.iter())
            .map(|member| {
                (
                    member.identity.account_id.as_str(),
                    member.identity.character_index,
                )
            })
            .collect();
        let new_members: BTreeSet<(&str, i32)> = staged
            .shared_guilds
            .get(id)
            .into_iter()
            .flat_map(|guild| guild.members.iter())
            .map(|member| {
                (
                    member.identity.account_id.as_str(),
                    member.identity.character_index,
                )
            })
            .collect();
        if old_members
            .symmetric_difference(&new_members)
            .any(|(account_id, _)| !authorized_accounts.contains(account_id))
        {
            return Err(AccountStoreTransactionScopeError::InvalidGuildState(
                "membership changes require the affected account in scope".into(),
            ));
        }
    }
    for (id, guild) in &staged.shared_guilds {
        if original.shared_guilds.get(id) == Some(guild) {
            continue;
        }
        let expected = match original.shared_guilds.get(id) {
            Some(previous) => previous.revision.checked_add(1),
            None => Some(1),
        };
        if expected != Some(guild.revision) {
            return Err(AccountStoreTransactionScopeError::InvalidGuildState(
                "guild revision must advance exactly once".into(),
            ));
        }
        for member in &guild.members {
            let already_member = original
                .shared_guilds
                .get(id)
                .is_some_and(|old| old.member(&member.identity).is_some());
            if !already_member
                && !staged
                    .accounts
                    .get(&member.identity.account_id)
                    .is_some_and(|account| {
                        account
                            .characters
                            .iter()
                            .any(|character| character.index == member.identity.character_index)
                    })
            {
                return Err(AccountStoreTransactionScopeError::InvalidGuildState(
                    "new guild member character does not exist".into(),
                ));
            }
        }
    }
    for guild in staged.shared_guilds.values() {
        for member in &guild.members {
            let existed = original
                .accounts
                .get(&member.identity.account_id)
                .is_some_and(|account| {
                    account
                        .characters
                        .iter()
                        .any(|character| character.index == member.identity.character_index)
                });
            let remains = staged
                .accounts
                .get(&member.identity.account_id)
                .is_some_and(|account| {
                    account
                        .characters
                        .iter()
                        .any(|character| character.index == member.identity.character_index)
                });
            if existed && !remains {
                return Err(AccountStoreTransactionScopeError::InvalidGuildState(
                    "character deletion requires explicit guild membership removal".into(),
                ));
            }
        }
    }
    validate_guild_state(staged).map_err(AccountStoreTransactionScopeError::InvalidGuildState)
}
pub(super) fn validate_complete_guild_state(store: &AccountStore) -> Result<(), String> {
    guild_experience::validate_import(store)?;
    validate_guild_state(store)?;
    for guild in store.shared_guilds.values() {
        for member in &guild.members {
            if !store
                .accounts
                .get(&member.identity.account_id)
                .is_some_and(|account| {
                    account
                        .characters
                        .iter()
                        .any(|character| character.index == member.identity.character_index)
                })
            {
                return Err("guild member character does not exist".into());
            }
        }
    }
    Ok(())
}

pub(super) fn validate_guild_state(store: &AccountStore) -> Result<(), String> {
    guild_clock::validate_store(store)?;
    if store.schema_version < 3 && !store.shared_guilds.is_empty() {
        return Err("legacy account store cannot contain shared guild authority".into());
    }
    let mut names = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for (id, guild) in &store.shared_guilds {
        if id != &guild.id
            || id.len() != 32
            || !id.bytes().all(|b| b.is_ascii_hexdigit())
            || guild.revision == 0
        {
            return Err("invalid stable guild ID/revision".into());
        }
        if !(3..=20).contains(&guild.name.encode_utf16().count())
            || guild.name.contains('\\')
            || guild.name_key().is_empty()
            || !names.insert(guild.name_key())
        {
            return Err("invalid or duplicate guild name".into());
        }
        if guild.storage.keys().any(|slot| *slot >= 112) {
            return Err("guild storage slot outside 112 cells".into());
        }
        let ranks: BTreeSet<u8> = guild.ranks.iter().map(|rank| rank.index).collect();
        if ranks.len() != guild.ranks.len() || !ranks.contains(&0) {
            return Err("invalid guild ranks".into());
        }
        if guild.members.is_empty() || !guild.members.iter().any(|member| member.rank_index == 0) {
            return Err("guild has no leader".into());
        }
        let mut public_character_indexes=BTreeSet::new();
        for member in &guild.members {
            if !public_character_indexes.insert(member.identity.character_index) {
                return Err("guild members have ambiguous legacy public character indexes".into());
            }
            if !ranks.contains(&member.rank_index)
                || !identities.insert((
                    member.identity.account_id.clone(),
                    member.identity.character_index,
                ))
            {
                return Err("duplicate membership or invalid rank".into());
            }
        }
    }
    Ok(())
}
impl SimulationConfig {
    pub(crate) fn commit_account_store_transaction_with_guilds<T, F>(
        &self,
        account_ids: &[String],
        guild_ids: &[String],
        transaction: F,
    ) -> Result<T, String>
    where
        F: FnOnce(&mut AccountStore) -> Result<T, String>,
    {
        if guild_ids.is_empty() {
            return Err("guild transaction requires explicit guild IDs".into());
        }
        self.commit_account_store_transaction_inner(
            AccountStoreMutationScope::AccountsWithGuilds {
                account_ids,
                guild_ids,
            },
            transaction,
        )
    }
}

pub(super) fn load_guilds(
    client: &mut impl postgres::GenericClient,
    store: &mut AccountStore,
) -> Result<(), String> {
    for row in client
        .query(
            "SELECT guild_id, raw_json, store_version FROM shared_guilds ORDER BY guild_id",
            &[],
        )
        .map_err(|e| format!("postgres shared guild load failed: {e}"))?
    {
        let id: String = row.get("guild_id");
        let guild: SharedGuildRecord = serde_json::from_value(row.get("raw_json"))
            .map_err(|e| format!("invalid shared guild {id}: {e}"))?;
        store.shared_guilds.insert(id.clone(), guild);
        store
            .source_guild_versions
            .insert(id, row.get("store_version"));
    }
    validate_complete_guild_state(store)
}
pub(super) fn write_guild_mutations(
    transaction: &mut Transaction<'_>,
    mutations: &BTreeMap<String, GuildMutation>,
    mode: AccountStoreDatabaseMode,
) -> Result<BTreeMap<String, i64>, String> {
    let mut versions = BTreeMap::new();
    // Ordered guild locks always precede ordered account locks. Advisory locks also
    // serialize absent-row creation; unique constraints arbitrate names/membership.
    for id in mutations.keys() {
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 735891))",
                &[id],
            )
            .map_err(|e| format!("postgres guild lock failed: {e}"))?;
    }
    let mut locked_versions = BTreeMap::new();
    let mut membership_changes = BTreeSet::new();
    let identities = |guild: &SharedGuildRecord| {
        guild
            .members
            .iter()
            .map(|member| {
                (
                    member.identity.account_id.clone(),
                    member.identity.character_index,
                )
            })
            .collect::<BTreeSet<_>>()
    };
    for (id, mutation) in mutations {
        let row = transaction
            .query_opt(
                "SELECT store_version, raw_json FROM shared_guilds WHERE guild_id=$1 FOR UPDATE",
                &[id],
            )
            .map_err(|e| e.to_string())?;
        let current = row.as_ref().map(|row| row.get::<_, i64>(0));
        let before: Option<SharedGuildRecord> = row
            .as_ref()
            .map(|row| serde_json::from_value(row.get(1)))
            .transpose()
            .map_err(|e| format!("invalid stored guild membership: {e}"))?;
        if before.as_ref().map(&identities) != mutation.desired.as_ref().map(&identities) {
            membership_changes.insert(id.clone());
        }
        if mode == AccountStoreDatabaseMode::SourceOfTruth && current != mutation.expected_version {
            return Err(format!(
                "stale postgres guild {id}: expected {:?}, found {current:?}",
                mutation.expected_version
            ));
        }
        locked_versions.insert(id.clone(), current);
    }
    // Clear changed membership sets before replacements; pure guild timer/gold
    // transactions do not churn stable membership indexes or lock member accounts.
    for id in &membership_changes {
        transaction
            .execute("DELETE FROM shared_guild_members WHERE guild_id=$1", &[id])
            .map_err(|e| e.to_string())?;
    }
    for (id, mutation) in mutations {
        let current = locked_versions[id];
        if let Some(guild) = &mutation.desired {
            let version = current
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("guild version exhausted")?;
            let json = serde_json::to_value(guild).map_err(|e| e.to_string())?;
            transaction.execute("INSERT INTO shared_guilds(guild_id,name_key,raw_json,store_version) VALUES($1,$2,$3,$4) ON CONFLICT(guild_id) DO UPDATE SET name_key=EXCLUDED.name_key,raw_json=EXCLUDED.raw_json,store_version=EXCLUDED.store_version,updated_at=now()", &[id,&guild.name_key(),&json,&version]).map_err(|e| format!("postgres guild write failed: {e}"))?;
            for member in guild
                .members
                .iter()
                .filter(|_| membership_changes.contains(id))
            {
                transaction.execute("INSERT INTO shared_guild_members(guild_id,account_id,character_index) VALUES($1,$2,$3)", &[id,&member.identity.account_id,&member.identity.character_index]).map_err(|e| format!("postgres guild membership write failed: {e}"))?;
            }
            versions.insert(id.clone(), version);
        } else {
            transaction
                .execute("DELETE FROM shared_guilds WHERE guild_id=$1", &[id])
                .map_err(|e| format!("postgres guild delete failed: {e}"))?;
        }
    }
    Ok(versions)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn guild(store: &AccountStore, id: &str, name: &str) -> SharedGuildRecord {
        let character = &store.accounts["demo"].characters[0];
        SharedGuildRecord {
            id: id.into(),
            name: name.into(),
            revision: 1,
            level: 0,
            experience: 0,
            spare_points: 0,
            gold: 0,
            ranks: vec![SharedGuildRank {
                index: 0,
                name: "Leader".into(),
                options: 255,
            }],
            members: vec![SharedGuildMember { membership_epoch: 0,
                identity: Stage5FriendIdentity {
                    account_id: "demo".into(),
                    character_index: character.index,
                },
                name: character.name.clone(),
                rank_index: 0,
            }],
            notice: Vec::new(),
            storage: BTreeMap::new(),
            buffs: BTreeMap::new(),
            last_buff_tick_ms: 0,
            experience_receipts: BTreeSet::new(), experience_receipt_payloads: Default::default(),
        }
    }
    const ID: &str = "0123456789abcdef0123456789abcdef";
    const OTHER: &str = "1123456789abcdef0123456789abcdef";
    #[test]
    fn guild_legacy_public_index_collision_rejects_join_and_file_without_repairing_assets(){
        let config=SimulationConfig::default();
        {
            let mut store=config.account_store.lock().unwrap();
            let mut peer=store.accounts["demo"].clone();
            peer.characters[0].name="LegacyPeer".into();
            store.accounts.insert("peer".into(),peer);
            let value=guild(&store,ID,"Knights");store.shared_guilds.insert(ID.into(),value);
        }
        let owner=config.account_store.lock().unwrap().shared_guilds[ID].members[0].identity.clone();
        let peer=Stage5FriendIdentity{account_id:"peer".into(),character_index:owner.character_index};
        assert!(config.commit_shared_guild_join(&owner,&peer,ID).unwrap_err().contains("ambiguous legacy"));
        let mut store=config.account_store.lock().unwrap().clone();
        assert_eq!(store.shared_guilds[ID].members.len(),1);
        store.shared_guilds.get_mut(ID).unwrap().members.push(SharedGuildMember{ membership_epoch: 0,identity:peer,name:"LegacyPeer".into(),rank_index:0});
        assert!(validate_guild_scope(&store,&store,AccountStoreMutationScope::FullRestore).is_err());
        let path=std::env::temp_dir().join(format!("mir2-guild-index-collision-{}-{}.json",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        let bytes=serde_json::to_vec(&store).unwrap();fs::write(&path,&bytes).unwrap();
        assert!(FileAccountStoreRepository::new(path.clone()).load(config.default_character.clone()).is_err());
        let frozen=SimulationConfig::default().with_account_store_path(path.clone());
        assert!(frozen.save_account_store().is_err());assert_eq!(fs::read(&path).unwrap(),bytes);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn guild_scope_is_exact_and_protects_source_versions_and_shop() {
        let config = SimulationConfig::default();
        let account_ids = vec!["demo".into()];
        let guild_ids = vec![ID.into()];
        let value = guild(&config.account_store.lock().unwrap(), ID, "Knights");
        assert!(config
            .commit_account_store_transaction(&account_ids, |store| {
                store.shared_guilds.insert(ID.into(), value.clone());
                Ok(())
            })
            .is_err());
        assert!(config
            .commit_account_store_transaction_with_global(&account_ids, |store| {
                store.shared_guilds.insert(ID.into(), value.clone());
                Ok(())
            })
            .is_err());
        assert!(config
            .commit_account_store_transaction_with_guilds(&account_ids, &[OTHER.into()], |store| {
                store.shared_guilds.insert(ID.into(), value.clone());
                Ok(())
            })
            .is_err());
        assert!(config
            .commit_account_store_transaction_with_guilds(&account_ids, &guild_ids, |store| {
                store.source_guild_versions.insert(ID.into(), 99);
                Ok(())
            })
            .is_err());
        assert!(config
            .commit_account_store_transaction_with_guilds(&account_ids, &guild_ids, |store| {
                store.game_shop_global_purchases.insert(3, 1);
                Ok(())
            })
            .is_err());
        config
            .commit_account_store_transaction_with_guilds(&account_ids, &guild_ids, |store| {
                store.shared_guilds.insert(ID.into(), value);
                Ok(())
            })
            .unwrap();
        // Pure guild ticking is permitted without saving any member account.
        config
            .commit_account_store_transaction_with_guilds(&[], &guild_ids, |store| {
                let guild = store.shared_guilds.get_mut(ID).unwrap();
                guild.last_buff_tick_ms = 60_000;
                guild.revision += 1;
                Ok(())
            })
            .unwrap();
        assert!(config
            .commit_account_store_transaction_with_guilds(&[], &guild_ids, |store| {
                store.accounts.get_mut("demo").unwrap().password = "bad".into();
                Ok(())
            })
            .is_err());
    }
    #[test]
    fn guild_name_and_membership_uniqueness_reject_ambiguous_authority() {
        let config = SimulationConfig::default();
        let mut store = config.account_store.lock().unwrap().clone();
        let first = guild(&store, ID, "Knights");
        let second = guild(&store, OTHER, "K nIgHtS");
        store.shared_guilds.insert(ID.into(), first);
        store.shared_guilds.insert(OTHER.into(), second);
        assert!(validate_guild_state(&store)
            .unwrap_err()
            .contains("duplicate guild name"));
        store.shared_guilds.get_mut(OTHER).unwrap().name = "Other".into();
        assert!(validate_guild_state(&store)
            .unwrap_err()
            .contains("duplicate membership"));
    }
    #[test]
    fn guild_scoped_read_and_account_save_cannot_write_authority() {
        let config = SimulationConfig::default();
        let mut store = config.account_store.lock().unwrap().clone();
        store
            .shared_guilds
            .insert(ID.into(), guild(&store, ID, "Knights"));
        store.source_guild_versions.insert(ID.into(), 8);
        let scoped = store.scoped_to_account("demo");
        assert_eq!(scoped.shared_guilds, store.shared_guilds);
        assert_eq!(scoped.source_guild_versions, store.source_guild_versions);
        let ids = vec!["demo".into()];
        for scope in [
            AccountStoreMutationScope::Accounts(&ids),
            AccountStoreMutationScope::AccountsWithGlobal(&ids),
        ] {
            assert!(
                build_account_store_mutation_plan(&store, &store, scope, false)
                    .guilds
                    .is_empty()
            );
        }
        let guild_ids = vec![ID.into()];
        let scope = AccountStoreMutationScope::AccountsWithGuilds {
            account_ids: &ids,
            guild_ids: &guild_ids,
        };
        let mut deleted = store.clone();
        deleted.shared_guilds.remove(ID);
        let plan = build_account_store_mutation_plan(&store, &deleted, scope, false);
        assert_eq!(plan.guilds[ID].expected_version, Some(8));
        assert!(plan.guilds[ID].desired.is_none());
        let compensate = build_account_store_mutation_plan(&deleted, &store, scope, false);
        assert_eq!(
            compensate.guilds[ID].desired,
            store.shared_guilds.get(ID).cloned()
        );
        let create = build_account_store_mutation_plan(&deleted, &store, scope, false);
        let uncreate = build_account_store_mutation_plan(&store, &deleted, scope, false);
        assert!(create.guilds[ID].desired.is_some());
        assert!(uncreate.guilds[ID].desired.is_none());
    }
    #[test]
    fn guild_full_restore_and_file_load_reject_orphans_without_replacing_assets() {
        let config = SimulationConfig::default();
        let mut store = config.account_store.lock().unwrap().clone();
        let mut orphan = guild(&store, ID, "Knights");
        orphan.members[0].identity.character_index = i32::MAX;
        store.shared_guilds.insert(ID.into(), orphan);
        assert!(
            validate_guild_scope(&store, &store, AccountStoreMutationScope::FullRestore).is_err()
        );
        let path = std::env::temp_dir().join(format!(
            "mir2-orphan-guild-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let original = serde_json::to_vec(&store).unwrap();
        fs::write(&path, &original).unwrap();
        assert!(FileAccountStoreRepository::new(path.clone())
            .load(config.default_character.clone())
            .is_err());
        let frozen = SimulationConfig::default().with_account_store_path(path.clone());
        assert!(frozen.save_account_store().is_err());
        assert!(frozen.shared_guild_authority_snapshot().is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(frozen
            .account_store
            .lock()
            .unwrap()
            .shared_guilds
            .contains_key(ID));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn guild_missing_demo_owner_cannot_be_manufactured_by_fixture_loading() {
        let config = SimulationConfig::default();
        let mut store = config.account_store.lock().unwrap().clone();
        store
            .shared_guilds
            .insert(ID.into(), guild(&store, ID, "Knights"));
        store.accounts.clear();
        let path = std::env::temp_dir().join(format!(
            "mir2-missing-owner-guild-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let original = serde_json::to_vec(&store).unwrap();
        fs::write(&path, &original).unwrap();
        assert!(FileAccountStoreRepository::new(path.clone())
            .load(config.default_character.clone())
            .is_err());
        assert!(config.restore_account_store_from_backup(&path).is_err());
        assert!(config
            .account_store
            .lock()
            .unwrap()
            .shared_guilds
            .is_empty());
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn guild_malformed_file_json_is_not_demoted_to_demo_or_overwritten() {
        for malformed in [
            br#"{"schemaVersion":3,"sharedGuilds":{"broken":{"gold":100}}}"#.as_slice(),
            br#"{"schemaVersion":3,"accounts":"#.as_slice(),
        ] {
            let path = std::env::temp_dir().join(format!(
                "mir2-corrupt-guild-{}-{}.json",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::write(&path, malformed).unwrap();
            let defaults = SimulationConfig::default();
            assert!(FileAccountStoreRepository::new(path.clone())
                .load(defaults.default_character.clone())
                .is_err());
            let frozen = SimulationConfig::default().with_account_store_path(path.clone());
            assert!(frozen.account_store.lock().unwrap().accounts.is_empty());
            assert!(frozen.save_account_store().is_err());
            assert_eq!(fs::read(&path).unwrap(), malformed);
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn guild_legacy_version_cannot_smuggle_shared_authority() {
        let config = SimulationConfig::default();
        let mut store = config.account_store.lock().unwrap().clone();
        store
            .shared_guilds
            .insert(ID.into(), guild(&store, ID, "Knights"));
        store.schema_version = 2;
        let loaded = store.migrate_to_current_schema();
        assert_eq!(loaded.schema_version, 2);
        assert!(validate_complete_guild_state(&loaded).is_err());
        assert!(
            loaded.shared_guilds.contains_key(ID),
            "invalid assets retained for repair, never silently discarded"
        );
    }

    #[test]
    fn guild_legacy_decode_never_promotes_personal_assets_or_privileges() {
        let config = SimulationConfig::default();
        let store = config.account_store.lock().unwrap().clone();
        let mut json = serde_json::to_value(store).unwrap();
        json.as_object_mut().unwrap().remove("sharedGuilds");
        json["schemaVersion"] = serde_json::json!(2);
        let loaded: AccountStore = serde_json::from_value(json.clone()).unwrap();
        let migrated = loaded.migrate_to_current_schema();
        assert!(migrated.shared_guilds.is_empty());
        assert_eq!(
            serde_json::to_value(migrated.accounts).unwrap(),
            json["accounts"]
        );
    }
    #[test]
    fn guild_file_transaction_keeps_wallet_and_bank_atomic_on_failure() {
        let path = std::env::temp_dir().join(format!(
            "mir2-guild-atomic-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let config = SimulationConfig::default().with_account_store_path(path.clone());
        config.save_account_store().unwrap();
        let before = fs::read(&path).unwrap();
        let original_password = config.account_store.lock().unwrap().accounts["demo"]
            .password
            .clone();
        let mutate = |store: &mut AccountStore| {
            let mut record = guild(store, ID, "Knights");
            record.gold = 100;
            store.shared_guilds.insert(ID.into(), record);
            store.accounts.get_mut("demo").unwrap().password = "transaction-marker".into();
            Ok(())
        };
        config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
        assert!(config
            .commit_account_store_transaction_with_guilds(&["demo".into()], &[ID.into()], mutate)
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(config
            .account_store
            .lock()
            .unwrap()
            .shared_guilds
            .is_empty());
        assert_eq!(
            config.account_store.lock().unwrap().accounts["demo"].password,
            original_password
        );
        config
            .commit_account_store_transaction_with_guilds(&["demo".into()], &[ID.into()], mutate)
            .unwrap();
        let saved: AccountStore = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.shared_guilds[ID].gold, 100);
        assert_eq!(saved.accounts["demo"].password, "transaction-marker");
        fs::remove_file(path).unwrap();
    }
}
impl SimulationConfig {
    /// Explicit shared-domain refresh. Ordinary personal saves retain a read-only
    /// guild view and never send it back through account-only persistence.
    pub fn refresh_shared_guild_authority(&self) -> Result<(), String> {
        if self.account_store_database_mode != AccountStoreDatabaseMode::SourceOfTruth {
            return Ok(());
        }
        let database_url = self
            .account_store_database_url
            .clone()
            .ok_or("postgres guild source unavailable")?;
        let _guard = self
            .account_store_persist_lock
            .lock()
            .map_err(|_| "guild refresh persist mutex poisoned")?;
        self.ensure_account_store_writable()?;
        let loaded = std::thread::spawn(move || -> Result<(BTreeMap<String, SharedGuildRecord>, BTreeMap<String, i64>),String> {
            let pool = postgres_account_store_pool(&database_url);
            let mut client = pool.connection()?;
            pool.ensure_migrated(&mut client)?;
            let mut guilds = BTreeMap::new(); let mut versions = BTreeMap::new();
            for row in client.query("SELECT guild_id, raw_json, store_version FROM shared_guilds ORDER BY guild_id", &[]).map_err(|e| format!("guild refresh failed: {e}"))? {
                let id: String = row.get("guild_id");
                let value = serde_json::from_value(row.get("raw_json")).map_err(|e| format!("invalid guild {id}: {e}"))?;
                guilds.insert(id.clone(),value); versions.insert(id,row.get("store_version"));
            }
            Ok((guilds,versions))
        }).join().map_err(|_| "guild refresh thread panicked")??;
        let mut live = self
            .account_store
            .lock()
            .map_err(|_| "guild refresh store mutex poisoned")?;
        let mut staged = live.clone();
        staged.shared_guilds = loaded.0;
        staged.source_guild_versions = loaded.1;
        validate_guild_state(&staged)?;
        live.shared_guilds = staged.shared_guilds;
        live.source_guild_versions = staged.source_guild_versions;
        Ok(())
    }
}
impl SimulationConfig {
    pub(crate) fn shared_guild_authority_snapshot(
        &self,
    ) -> Result<BTreeMap<String, SharedGuildRecord>, String> {
        self.ensure_account_store_writable()?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "guild authority mutex poisoned")?;
        validate_guild_state(&store)?;
        Ok(store.shared_guilds.clone())
    }
}
#[cfg(test)]
mod postgres_tests {
    use super::*;
    #[test]
    #[ignore = "requires dedicated MIR2_GUILD_TEST_DATABASE_URL; never connects to the live store"]
    fn guild_postgres_atomic_cas_unique_membership_and_mirror_compensation() {
        let url = std::env::var("MIR2_GUILD_TEST_DATABASE_URL")
            .expect("dedicated guild test database required");
        let mut client = Client::connect(&url, NoTls).unwrap();
        crate::db_projection::apply_migrations(&mut client).unwrap();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let account_id = format!("guild-test-{}-{nonce}", std::process::id());
        let id = format!("{nonce:032x}");
        let second_id = format!("{:032x}", nonce + 1);
        let cfg = SimulationConfig::default();
        let mut original = cfg.account_store.lock().unwrap().clone();
        let mut account = original.accounts.remove("demo").unwrap();
        let index = account.characters[0].index;
        account.saves.get_mut(&index).unwrap().gold = 200;
        original.accounts.insert(account_id.clone(), account);
        let account_ids = vec![account_id.clone()];
        let first = build_account_store_mutation_plan(
            &original,
            &original,
            AccountStoreMutationScope::Accounts(&account_ids),
            false,
        );
        let versions = write_account_store_mutation_plan_to_postgres(
            &mut client,
            &first,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .unwrap();
        apply_account_store_mutation_source_versions(&mut original, &first, versions);
        let make_guild = |guild_id: &str, name: &str| SharedGuildRecord {
            id: guild_id.into(),
            name: name.into(),
            revision: 1,
            level: 0,
            experience: 0,
            spare_points: 0,
            gold: 0,
            ranks: vec![SharedGuildRank {
                index: 0,
                name: "Leader".into(),
                options: 255,
            }],
            members: vec![SharedGuildMember { membership_epoch: 0,
                identity: Stage5FriendIdentity {
                    account_id: account_id.clone(),
                    character_index: index,
                },
                name: "Tester".into(),
                rank_index: 0,
            }],
            notice: vec![],
            storage: BTreeMap::new(),
            buffs: BTreeMap::new(),
            last_buff_tick_ms: 0,
            experience_receipts: BTreeSet::new(), experience_receipt_payloads: Default::default(),
        };
        let mut staged = original.clone();
        staged.shared_guilds.insert(
            id.clone(),
            make_guild(&id, &format!("G{:x}", nonce % 0xffffffffffffffff)),
        );
        staged
            .accounts
            .get_mut(&account_id)
            .unwrap()
            .saves
            .get_mut(&index)
            .unwrap()
            .gold = 100;
        let guild_ids = vec![id.clone()];
        let scope = AccountStoreMutationScope::AccountsWithGuilds {
            account_ids: &account_ids,
            guild_ids: &guild_ids,
        };
        let create = build_account_store_mutation_plan(&original, &staged, scope, false);
        let versions = write_account_store_mutation_plan_to_postgres(
            &mut client,
            &create,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .unwrap();
        apply_account_store_mutation_source_versions(&mut staged, &create, versions);
        assert!(write_account_store_mutation_plan_to_postgres(
            &mut client,
            &create,
            AccountStoreDatabaseMode::SourceOfTruth
        )
        .unwrap_err()
        .contains("stale postgres guild"));
        // The database prevents a second guild from claiming the same stable character.
        let mut duplicate = staged.clone();
        duplicate
            .shared_guilds
            .insert(second_id.clone(), make_guild(&second_id, "OtherGuild"));
        duplicate
            .accounts
            .get_mut(&account_id)
            .unwrap()
            .saves
            .get_mut(&index)
            .unwrap()
            .gold = 0;
        let second_ids = vec![second_id.clone()];
        let duplicate_plan = build_account_store_mutation_plan(
            &staged,
            &duplicate,
            AccountStoreMutationScope::AccountsWithGuilds {
                account_ids: &account_ids,
                guild_ids: &second_ids,
            },
            false,
        );
        assert!(write_account_store_mutation_plan_to_postgres(
            &mut client,
            &duplicate_plan,
            AccountStoreDatabaseMode::SourceOfTruth
        )
        .is_err());
        let gold: i64 = client
            .query_one(
                "SELECT gold FROM character_saves WHERE account_id=$1 AND character_index=$2",
                &[&account_id, &index],
            )
            .unwrap()
            .get(0);
        assert_eq!(
            gold, 100,
            "failed membership transaction must roll back personal debit"
        );
        let count: i64 = client
            .query_one(
                "SELECT count(*) FROM shared_guilds WHERE guild_id=$1",
                &[&second_id],
            )
            .unwrap()
            .get(0);
        assert_eq!(count, 0);
        // Pure guild updates neither require nor advance account/save versions.
        let mut ticked = staged.clone();
        ticked.shared_guilds.get_mut(&id).unwrap().revision += 1;
        let tick = build_account_store_mutation_plan(
            &staged,
            &ticked,
            AccountStoreMutationScope::AccountsWithGuilds {
                account_ids: &[],
                guild_ids: &guild_ids,
            },
            false,
        );
        let tick_versions = write_account_store_mutation_plan_to_postgres(
            &mut client,
            &tick,
            AccountStoreDatabaseMode::SourceOfTruth,
        )
        .unwrap();
        assert!(tick_versions.accounts.is_empty());
        apply_account_store_mutation_source_versions(&mut ticked, &tick, tick_versions);
        // Mirror compensation includes the created guild tombstone and restores account assets.
        let rollback = build_account_store_mutation_plan(&ticked, &original, scope, false);
        write_account_store_mutation_plan_to_postgres(
            &mut client,
            &rollback,
            AccountStoreDatabaseMode::Mirror,
        )
        .unwrap();
        let count: i64 = client
            .query_one(
                "SELECT count(*) FROM shared_guilds WHERE guild_id=$1",
                &[&id],
            )
            .unwrap()
            .get(0);
        assert_eq!(count, 0);
        let gold: i64 = client
            .query_one(
                "SELECT gold FROM character_saves WHERE account_id=$1 AND character_index=$2",
                &[&account_id, &index],
            )
            .unwrap()
            .get(0);
        assert_eq!(gold, 200);
        client
            .execute("DELETE FROM accounts WHERE account_id=$1", &[&account_id])
            .unwrap();
    }
}
impl AccountStoreMutationPlan {
    /// Reserve existing accounts before inserting membership unique-index entries.
    /// Otherwise two different guilds can insert the same member, then deadlock
    /// when the first commit checks the second insertion while it waits on accounts.
    pub(super) fn lock_guild_plan(&self, transaction: &mut Transaction<'_>) -> Result<(), String> {
        if self.guilds.is_empty() && self.heroes.is_empty() && self.hero_allocator.is_none() {
            return Ok(());
        }
        // Full restore also touches the shop singleton. Ordinary shop writes take
        // this lock before accounts, so preserve that order at the mixed boundary.
        if self.global_stock.is_some() {
            transaction
                .query_opt(
                    "SELECT state_key FROM game_shop_global_stock WHERE state_key=1 FOR UPDATE",
                    &[],
                )
                .map_err(|e| e.to_string())?;
        }
        for id in self.guilds.keys() {
            transaction
                .query_one(
                    "SELECT pg_advisory_xact_lock(hashtextextended($1, 735891))",
                    &[id],
                )
                .map_err(|e| e.to_string())?;
            transaction
                .query_opt(
                    "SELECT guild_id FROM shared_guilds WHERE guild_id=$1 FOR UPDATE",
                    &[id],
                )
                .map_err(|e| e.to_string())?;
        }
        hero_postgres::lock_hero_mutations(transaction, &self.heroes, self.hero_allocator.as_ref())?;
        for id in self.accounts.keys() {
            transaction
                .query_opt(
                    "SELECT account_id FROM accounts WHERE account_id=$1 FOR UPDATE",
                    &[id],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "config_shared_guild_pg_tests.rs"]
mod postgres_race_tests;

impl SimulationConfig {
    pub(crate) fn shared_guild_active_stats(
        &self,
        identity: &Stage5FriendIdentity,
    ) -> Result<Vec<mir2_protocol::UserItemStat>, String> {
        self.ensure_account_store_writable()?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "guild authority poisoned")?;
        let Some(guild) = store
            .shared_guilds
            .values()
            .find(|guild| guild.member(identity).is_some())
        else {
            return Ok(Vec::new());
        };
        Ok(mir2_game_data::crystal_guild_buff_definitions()
            .iter()
            .filter(|definition| {
                guild
                    .buffs
                    .get(&definition.id)
                    .is_some_and(|buff| buff.active)
            })
            .flat_map(|definition| definition.stats.iter().cloned())
            .collect())
    }
    pub fn shared_guild_for_identity(
        &self,
        identity: &Stage5FriendIdentity,
    ) -> Result<Option<SharedGuildRecord>, String> {
        self.ensure_account_store_writable()?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "guild authority mutex poisoned")?;
        // Clone only the owner's record, never every guild bank on a render snapshot.
        // Structural/identity validation happens at strict load and mutation boundaries.
        Ok(store
            .shared_guilds
            .values()
            .find(|guild| guild.member(identity).is_some())
            .cloned())
    }
}
