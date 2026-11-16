//! Shared siege authority participates in the fixed account-store transaction.
//! Conquest locks precede Guild, Hero and account locks. Personal Stage 5 siege
//! snapshots are compatibility projections and are never imported here.
use super::{
    AccountStore, AccountStoreDatabaseMode, AccountStoreMutationScope,
    AccountStoreTransactionScopeError,
};
use crate::conquest::SharedConquestRecord;
use postgres::{GenericClient, Transaction};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// A separate two-key advisory-lock namespace also fences absent-row creation.
const CONQUEST_LOCK_NAMESPACE: i32 = 0x4D32_4351;

#[derive(Debug, Clone)]
pub(super) struct ConquestMutation {
    pub expected_version: Option<i64>,
    /// File primaries do not retain PostgreSQL source metadata after reopening.
    /// Only this exact old business image can admit a previously mirrored row.
    pub original: Option<SharedConquestRecord>,
    pub desired: Option<SharedConquestRecord>,
}

pub(super) fn build_mutations(
    original: &AccountStore,
    desired: &AccountStore,
    scope: AccountStoreMutationScope<'_>,
) -> BTreeMap<i32, ConquestMutation> {
    let restore = matches!(scope, AccountStoreMutationScope::FullRestore);
    let indices: BTreeSet<i32> = match scope {
        AccountStoreMutationScope::AccountsWithGuildsAndConquests { conquest_ids, .. } => {
            conquest_ids.iter().copied().collect()
        }
        AccountStoreMutationScope::FullRestore => original
            .shared_conquests
            .keys()
            .chain(original.source_conquest_versions.keys())
            .chain(desired.shared_conquests.keys())
            .copied()
            .collect(),
        _ => BTreeSet::new(),
    };
    indices
        .into_iter()
        .filter(|index| {
            restore || original.shared_conquests.get(index) != desired.shared_conquests.get(index)
        })
        .map(|index| {
            (
                index,
                ConquestMutation {
                    expected_version: original.source_conquest_versions.get(&index).copied(),
                    original: original.shared_conquests.get(&index).cloned(),
                    desired: desired.shared_conquests.get(&index).cloned(),
                },
            )
        })
        .collect()
}

pub(super) fn validate_scope(
    original: &AccountStore,
    staged: &AccountStore,
    scope: AccountStoreMutationScope<'_>,
) -> Result<(), AccountStoreTransactionScopeError> {
    let restore = matches!(scope, AccountStoreMutationScope::FullRestore);
    if !restore && original.source_conquest_versions != staged.source_conquest_versions {
        return Err(AccountStoreTransactionScopeError::SourceMetadataChanged {
            field: "source_conquest_versions",
        });
    }
    if restore {
        return validate_complete_state(staged)
            .map_err(AccountStoreTransactionScopeError::InvalidConquestState);
    }
    for guild_id in original
        .shared_guilds
        .keys()
        .filter(|id| !staged.shared_guilds.contains_key(*id))
    {
        if staged.shared_conquests.values().any(|record| {
            record.owner_guild_id.as_ref() == Some(guild_id)
                || record.attacker_guild_id.as_ref() == Some(guild_id)
        }) {
            return Err(AccountStoreTransactionScopeError::InvalidConquestState(
                "a castle owner or registered attacker cannot disband its Guild".into(),
            ));
        }
    }
    let authorized: BTreeSet<i32> = match scope {
        AccountStoreMutationScope::AccountsWithGuildsAndConquests { conquest_ids, .. } => {
            conquest_ids.iter().copied().collect()
        }
        _ => BTreeSet::new(),
    };
    for index in original
        .shared_conquests
        .keys()
        .chain(staged.shared_conquests.keys())
        .copied()
        .collect::<BTreeSet<_>>()
    {
        let before = original.shared_conquests.get(&index);
        let after = staged.shared_conquests.get(&index);
        if before == after {
            continue;
        }
        if !authorized.contains(&index) {
            return Err(
                AccountStoreTransactionScopeError::OutOfScopeConquestChanged {
                    conquest_index: index,
                },
            );
        }
        let record = after.ok_or_else(|| {
            AccountStoreTransactionScopeError::InvalidConquestState(
                "shared conquest deletion requires FullRestore authority".into(),
            )
        })?;
        validate_record(index, record)
            .map_err(AccountStoreTransactionScopeError::InvalidConquestState)?;
    }
    // Business revision progression belongs to the siege state machine. A
    // fenced Mirror compensation may legitimately restore an older business
    // revision while the repository store_version still advances monotonically.
    Ok(())
}

pub(super) fn validate_complete_state(store: &AccountStore) -> Result<(), String> {
    if store.schema_version < 6 && !store.shared_conquests.is_empty() {
        return Err("pre-conquest schema contains unauthorized siege authority".into());
    }
    for (&index, record) in &store.shared_conquests {
        validate_record(index, record)?;
    }
    for (&index, &version) in &store.source_conquest_versions {
        if index <= 0 || version <= 0 {
            return Err("invalid shared conquest source version".into());
        }
    }
    validate_guild_bindings(store)
}

pub(super) fn validate_guild_bindings(store: &AccountStore) -> Result<(), String> {
    for record in store.shared_conquests.values() {
        for id in record
            .owner_guild_id
            .iter()
            .chain(record.attacker_guild_id.iter())
        {
            if !store.shared_guilds.contains_key(id) {
                return Err("castle authority references a missing Guild".into());
            }
        }
    }
    Ok(())
}

/// A restore may recover business state, never an old process clock grant.
pub(super) fn invalidate_restored_leases(
    live: &AccountStore,
    restored: &mut AccountStore,
) -> Result<(), String> {
    validate_guild_bindings(restored)?;
    for (&index, record) in &mut restored.shared_conquests {
        let previous = live.shared_conquests.get(&index);
        record.clock_generation = record
            .clock_generation
            .max(previous.map_or(0, |r| r.clock_generation))
            .checked_add(1)
            .ok_or("castle clock generation exhausted during restore")?;
        record.revision = record
            .revision
            .max(previous.map_or(0, |r| r.revision))
            .checked_add(1)
            .ok_or("castle revision exhausted during restore")?;
        record.last_observed_ms = record
            .last_observed_ms
            .max(previous.map_or(0, |r| r.last_observed_ms));
        record.lease = None;
        record.validate()?;
    }
    Ok(())
}

pub(super) fn validate_receipt(
    plan: &super::AccountStoreMutationPlan,
    receipt: &super::AccountStoreRepositorySave,
) -> Result<(), String> {
    for (&index, mutation) in &plan.conquests {
        match (
            mutation.desired.as_ref(),
            receipt.conquest_versions.get(&index),
        ) {
            (Some(_), Some(version)) if *version > 0 => {}
            (None, None) => {}
            _ => {
                return Err(format!(
                    "conquest {index} repository omitted its durable receipt"
                ))
            }
        }
    }
    Ok(())
}

fn validate_record(index: i32, record: &SharedConquestRecord) -> Result<(), String> {
    if index <= 0 || record.index != index {
        return Err("shared conquest record index does not match its stable key".into());
    }
    record.validate()
}

pub(super) fn load_conquests(
    client: &mut impl GenericClient,
    store: &mut AccountStore,
) -> Result<(), String> {
    let mut records = BTreeMap::new();
    let mut versions = BTreeMap::new();
    for row in client
        .query(
            "SELECT conquest_index,raw_json,store_version FROM shared_conquests ORDER BY conquest_index",
            &[],
        )
        .map_err(|error| format!("postgres conquest load failed: {error}"))?
    {
        let index: i32 = row.get(0);
        let record: SharedConquestRecord = serde_json::from_value(row.get(1))
            .map_err(|error| format!("invalid stored conquest {index}: {error}"))?;
        let version: i64 = row.get(2);
        validate_record(index, &record)?;
        if version <= 0 {
            return Err(format!("invalid conquest {index} source version"));
        }
        records.insert(index, record);
        versions.insert(index, version);
    }
    store.shared_conquests = records;
    store.source_conquest_versions = versions;
    Ok(())
}

/// The enclosing repository calls this before it takes any Guild locks. The
/// namespace lock protects creations; sorted row locks protect replacements.
pub(super) fn lock_mutations(
    transaction: &mut Transaction<'_>,
    mutations: &BTreeMap<i32, ConquestMutation>,
) -> Result<(), String> {
    if mutations.keys().any(|index| *index <= 0) {
        return Err("conquest locks require positive stable indices".into());
    }
    for index in mutations.keys() {
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock($1,$2)",
                &[&CONQUEST_LOCK_NAMESPACE, index],
            )
            .map_err(|error| format!("postgres conquest namespace lock failed: {error}"))?;
    }
    for index in mutations.keys() {
        transaction
            .query_opt(
                "SELECT store_version FROM shared_conquests WHERE conquest_index=$1 FOR UPDATE",
                &[index],
            )
            .map_err(|error| format!("postgres conquest row lock failed: {error}"))?;
    }
    Ok(())
}

/// Commit and uncertain-outcome handling remain with the enclosing account-store
/// transaction. Both SourceOfTruth and Mirror enforce the admitted row version.
pub(super) fn write_mutations(
    transaction: &mut Transaction<'_>,
    mutations: &BTreeMap<i32, ConquestMutation>,
    mode: AccountStoreDatabaseMode,
) -> Result<BTreeMap<i32, i64>, String> {
    for (&index, mutation) in mutations {
        if index <= 0 {
            return Err("conquest mutation requires a positive stable index".into());
        }
        for record in mutation.original.iter().chain(mutation.desired.iter()) {
            validate_record(index, record)?;
        }
    }
    // Reacquiring already-held locks is safe. This also protects direct helper
    // callers that have no other domains to lock or write in their transaction.
    lock_mutations(transaction, mutations)?;
    let mut admitted_versions = BTreeMap::new();
    for (&index, mutation) in mutations {
        let original_json = mutation
            .original
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| format!("conquest {index} serialization failed: {error}"))?;
        let observed = transaction
            .query_opt(
                "SELECT store_version,raw_json FROM shared_conquests WHERE conquest_index=$1 FOR UPDATE",
                &[&index],
            )
            .map_err(|error| format!("postgres conquest {index} lock failed: {error}"))?
            .map(|row| (row.get::<_, i64>(0), row.get::<_, Value>(1)));
        if let Some((_, json)) = &observed {
            let record: SharedConquestRecord = serde_json::from_value(json.clone())
                .map_err(|error| format!("invalid stored conquest {index}: {error}"))?;
            validate_record(index, &record)?;
        }
        let version = admit_observed_version(
            index,
            mutation.expected_version,
            original_json.as_ref(),
            observed.as_ref(),
            mode,
        )?;
        admitted_versions.insert(index, version);
    }
    let mut written_versions = BTreeMap::new();
    for (&index, mutation) in mutations {
        let current = admitted_versions[&index];
        if let Some(record) = &mutation.desired {
            let version = current
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("conquest source version exhausted")?;
            let json = serde_json::to_value(record)
                .map_err(|error| format!("conquest {index} serialization failed: {error}"))?;
            let changed = if let Some(expected) = current {
                transaction.execute(
                    "UPDATE shared_conquests SET raw_json=$2,store_version=$3,updated_at=clock_timestamp() WHERE conquest_index=$1 AND store_version=$4",
                    &[&index, &json, &version, &expected],
                )
            } else {
                transaction.execute(
                    "INSERT INTO shared_conquests(conquest_index,raw_json,store_version) VALUES($1,$2,$3)",
                    &[&index, &json, &version],
                )
            }
            .map_err(|error| format!("postgres conquest {index} write failed: {error}"))?;
            if changed != 1 {
                return Err(format!("stale postgres conquest {index} during write"));
            }
            written_versions.insert(index, version);
        } else if let Some(expected) = current {
            let changed = transaction
                .execute(
                    "DELETE FROM shared_conquests WHERE conquest_index=$1 AND store_version=$2",
                    &[&index, &expected],
                )
                .map_err(|error| format!("postgres conquest {index} deletion failed: {error}"))?;
            if changed != 1 {
                return Err(format!("stale postgres conquest {index} during deletion"));
            }
        }
    }
    Ok(written_versions)
}

fn admit_observed_version(
    index: i32,
    expected: Option<i64>,
    original_json: Option<&Value>,
    observed: Option<&(i64, Value)>,
    mode: AccountStoreDatabaseMode,
) -> Result<Option<i64>, String> {
    if expected.is_some_and(|version| version <= 0)
        || observed.is_some_and(|(version, _)| *version <= 0)
    {
        return Err(format!("invalid conquest {index} source version"));
    }
    let current = observed.map(|(version, _)| *version);
    if current == expected {
        return Ok(current);
    }
    if mode == AccountStoreDatabaseMode::Mirror
        && expected.is_none()
        && observed.is_some_and(|(_, json)| original_json == Some(json))
    {
        return Ok(current);
    }
    Err(format!(
        "stale postgres conquest {index}: expected {expected:?}, found {current:?}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> AccountStore {
        let config = crate::SimulationConfig::default();
        let snapshot = config.account_store.lock().unwrap().clone();
        snapshot
    }
    #[test]
    fn conquest_restore_invalidates_backup_and_live_clock_grants() {
        let mut live = store();
        let mut current = SharedConquestRecord::new(1);
        current.clock_generation = 8;
        current.revision = 22;
        current.last_observed_ms = 100;
        current.lease = Some(crate::conquest::ConquestLease {
            owner: "0123456789abcdef0123456789abcdef".into(),
            generation: 8,
            expires_ms: 200,
        });
        live.shared_conquests.insert(1, current);
        let mut backup = store();
        let mut old = SharedConquestRecord::new(1);
        old.clock_generation = 2;
        old.lease = Some(crate::conquest::ConquestLease {
            owner: "0123456789abcdef0123456789abcdef".into(),
            generation: 2,
            expires_ms: 400,
        });
        backup.shared_conquests.insert(1, old);
        invalidate_restored_leases(&live, &mut backup).unwrap();
        let restored = &backup.shared_conquests[&1];
        assert!(restored.lease.is_none());
        assert_eq!(restored.clock_generation, 9);
        assert_eq!(restored.revision, 23);
        assert_eq!(restored.last_observed_ms, 100);
        restored.validate().unwrap();
        let mut compensation = live.clone();
        invalidate_restored_leases(&backup, &mut compensation).unwrap();
        assert_eq!(compensation.shared_conquests[&1].clock_generation, 10);
        assert!(compensation.shared_conquests[&1].lease.is_none());
    }
    #[test]
    fn conquest_restore_rejects_missing_guild_and_exhausted_generation() {
        let live = store();
        let mut backup = live.clone();
        let mut record = SharedConquestRecord::new(1);
        record.owner_guild_id = Some("0123456789abcdef0123456789abcdef".into());
        backup.shared_conquests.insert(1, record);
        assert!(invalidate_restored_leases(&live, &mut backup).is_err());
        let record = backup.shared_conquests.get_mut(&1).unwrap();
        record.owner_guild_id = None;
        record.clock_generation = u64::MAX;
        assert!(invalidate_restored_leases(&live, &mut backup).is_err());
    }
    #[test]
    fn conquest_scope_authorizes_only_its_explicit_record_indices() {
        let before = store();
        let mut after = before.clone();
        after
            .shared_conquests
            .insert(1, SharedConquestRecord::new(1));
        for scope in [
            AccountStoreMutationScope::Accounts(&[]),
            AccountStoreMutationScope::AccountsWithGlobal(&[]),
            AccountStoreMutationScope::AccountsWithGuilds {
                account_ids: &[],
                guild_ids: &[],
            },
            AccountStoreMutationScope::AccountsWithGuildsAndConquests {
                account_ids: &[],
                guild_ids: &[],
                conquest_ids: &[2],
            },
        ] {
            assert!(matches!(
                validate_scope(&before, &after, scope),
                Err(
                    AccountStoreTransactionScopeError::OutOfScopeConquestChanged {
                        conquest_index: 1
                    }
                )
            ));
        }
        validate_scope(
            &before,
            &after,
            AccountStoreMutationScope::AccountsWithGuildsAndConquests {
                account_ids: &[],
                guild_ids: &[],
                conquest_ids: &[1],
            },
        )
        .unwrap();
    }
    #[test]
    fn conquest_source_metadata_cannot_be_supplied_by_a_scoped_closure() {
        let before = store();
        let mut after = before.clone();
        after.source_conquest_versions.insert(1, 99);
        assert!(matches!(
            validate_scope(
                &before,
                &after,
                AccountStoreMutationScope::AccountsWithGuildsAndConquests {
                    account_ids: &[],
                    guild_ids: &[],
                    conquest_ids: &[1],
                }
            ),
            Err(AccountStoreTransactionScopeError::SourceMetadataChanged {
                field: "source_conquest_versions"
            })
        ));
    }
    #[test]
    fn conquest_scope_rejects_mismatched_keys_and_invalid_business_state() {
        let before = store();
        let scope = AccountStoreMutationScope::AccountsWithGuildsAndConquests {
            account_ids: &[],
            guild_ids: &[],
            conquest_ids: &[1],
        };
        let mut after = before.clone();
        after
            .shared_conquests
            .insert(1, SharedConquestRecord::new(2));
        assert!(validate_scope(&before, &after, scope).is_err());
        let mut invalid = SharedConquestRecord::new(1);
        invalid.revision = 0;
        after.shared_conquests.insert(1, invalid);
        assert!(validate_scope(&before, &after, scope).is_err());
    }
    #[test]
    fn conquest_normal_delete_rejects_but_explicit_restore_can_delete() {
        let mut before = store();
        before
            .shared_conquests
            .insert(1, SharedConquestRecord::new(1));
        before.source_conquest_versions.insert(1, 7);
        let mut after = before.clone();
        after.shared_conquests.remove(&1);
        assert!(validate_scope(
            &before,
            &after,
            AccountStoreMutationScope::AccountsWithGuildsAndConquests {
                account_ids: &[],
                guild_ids: &[],
                conquest_ids: &[1],
            }
        )
        .is_err());
        validate_scope(&before, &after, AccountStoreMutationScope::FullRestore).unwrap();
        let mutations = build_mutations(&before, &after, AccountStoreMutationScope::FullRestore);
        assert_eq!(mutations[&1].expected_version, Some(7));
        assert!(mutations[&1].original.is_some());
        assert!(mutations[&1].desired.is_none());
    }
    #[test]
    fn conquest_unchanged_scopes_skip_writes_but_restore_carries_authority() {
        let mut before = store();
        before
            .shared_conquests
            .insert(1, SharedConquestRecord::new(1));
        before.source_conquest_versions.insert(1, 7);
        let scope = AccountStoreMutationScope::AccountsWithGuildsAndConquests {
            account_ids: &[],
            guild_ids: &[],
            conquest_ids: &[1],
        };
        assert!(build_mutations(&before, &before, scope).is_empty());
        let restored = build_mutations(&before, &before, AccountStoreMutationScope::FullRestore);
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[&1].expected_version, Some(7));
        assert_eq!(restored[&1].original, restored[&1].desired);
    }
    #[test]
    fn conquest_compensation_can_restore_older_business_revision_under_version_cas() {
        let mut committed = store();
        let mut record = SharedConquestRecord::new(1);
        record.revision = 2;
        committed.shared_conquests.insert(1, record);
        committed.source_conquest_versions.insert(1, 8);
        let mut restored = committed.clone();
        restored
            .shared_conquests
            .insert(1, SharedConquestRecord::new(1));
        validate_scope(
            &committed,
            &restored,
            AccountStoreMutationScope::FullRestore,
        )
        .unwrap();
        let mutations = build_mutations(
            &committed,
            &restored,
            AccountStoreMutationScope::FullRestore,
        );
        assert_eq!(mutations[&1].expected_version, Some(8));
        assert_eq!(mutations[&1].original.as_ref().unwrap().revision, 2);
        assert_eq!(mutations[&1].desired.as_ref().unwrap().revision, 1);
    }
    fn payload(revision: u64) -> Value {
        serde_json::json!({"index": 1, "revision": revision, "ownerGuildId": null})
    }
    #[test]
    fn conquest_new_absent_row_is_admitted_in_both_modes() {
        for mode in [
            AccountStoreDatabaseMode::Mirror,
            AccountStoreDatabaseMode::SourceOfTruth,
        ] {
            assert_eq!(
                admit_observed_version(1, None, None, None, mode).unwrap(),
                None
            );
        }
    }
    #[test]
    fn conquest_observed_version_is_strict_in_both_modes() {
        let observed = (7, payload(4));
        for mode in [
            AccountStoreDatabaseMode::Mirror,
            AccountStoreDatabaseMode::SourceOfTruth,
        ] {
            assert_eq!(
                admit_observed_version(1, Some(7), None, Some(&observed), mode).unwrap(),
                Some(7)
            );
            assert!(admit_observed_version(1, Some(6), None, Some(&observed), mode).is_err());
            assert!(admit_observed_version(1, Some(8), None, Some(&observed), mode).is_err());
            assert!(admit_observed_version(1, Some(7), None, None, mode).is_err());
        }
    }
    #[test]
    fn conquest_reopened_mirror_requires_exact_original_business_image() {
        let original = payload(4);
        let observed = (7, original.clone());
        assert_eq!(
            admit_observed_version(
                1,
                None,
                Some(&original),
                Some(&observed),
                AccountStoreDatabaseMode::Mirror
            )
            .unwrap(),
            Some(7)
        );
        assert!(admit_observed_version(
            1,
            None,
            Some(&payload(3)),
            Some(&observed),
            AccountStoreDatabaseMode::Mirror
        )
        .is_err());
        assert!(admit_observed_version(
            1,
            None,
            None,
            Some(&observed),
            AccountStoreDatabaseMode::Mirror
        )
        .is_err());
    }
    #[test]
    fn conquest_source_authority_never_adopts_an_unobserved_existing_row() {
        let original = payload(4);
        let observed = (7, original.clone());
        assert!(admit_observed_version(
            1,
            None,
            Some(&original),
            Some(&observed),
            AccountStoreDatabaseMode::SourceOfTruth
        )
        .is_err());
    }
    #[test]
    fn conquest_invalid_repository_versions_fail_before_business_comparison() {
        let original = payload(4);
        for invalid in [0, -1] {
            let observed = (invalid, original.clone());
            for mode in [
                AccountStoreDatabaseMode::Mirror,
                AccountStoreDatabaseMode::SourceOfTruth,
            ] {
                assert!(admit_observed_version(
                    1,
                    Some(invalid),
                    Some(&original),
                    Some(&observed),
                    mode
                )
                .is_err());
                assert!(
                    admit_observed_version(1, None, Some(&original), Some(&observed), mode)
                        .is_err()
                );
            }
        }
    }
}
