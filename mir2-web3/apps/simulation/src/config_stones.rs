//! Sealed-stone results are a separate server authority. Ordinary saves cannot
//! modify it; material processing has an explicit fixed identity scope.
use super::{AccountStore, AccountStoreMutationScope as Scope,
    AccountStoreTransactionScopeError as ScopeError};
use mir2_production::StoneRegistry;
use postgres::{GenericClient, Transaction};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) const COMMIT_UNKNOWN: &str = "STONE_COMMIT_OUTCOME_UNKNOWN";

#[derive(Debug, Clone)]
pub(super) struct StoneMutation {
    pub expected_version: Option<i64>,
    pub desired: StoneRegistry,
}

pub(super) fn validate_store(store: &AccountStore) -> Result<(), String> {
    store.sealed_stones.validate().map_err(|e| e.to_string())?;
    if store.schema_version < 6 && store.sealed_stones != StoneRegistry::default() {
        return Err("stone authority present in an older schema".into());
    }
    Ok(())
}

pub(super) fn validate_scope(before: &AccountStore, after: &AccountStore, scope: Scope<'_>)
    -> Result<(), ScopeError>
{
    let invalid = |s: String| ScopeError::InvalidStoneState(s);
    validate_store(after).map_err(invalid)?;
    if !matches!(scope, Scope::FullRestore) && before.source_stone_version != after.source_stone_version {
        return Err(invalid("stone source version is repository-owned".into()));
    }
    let (ids, accounts) = match scope {
        Scope::AccountsWithStones { stone_ids, account_ids }
        | Scope::AccountsWithStonesAndGuilds { stone_ids, account_ids, .. } => (stone_ids, account_ids),
        _ => {
            if before.sealed_stones != after.sealed_stones {
                return Err(invalid("stone mutation requires a fixed stone scope; restore cannot rewind results".into()));
            }
            return Ok(());
        }
    };
    before.sealed_stones.validate_successor(&after.sealed_stones).map_err(|e| invalid(e.to_string()))?;
    let allowed: BTreeSet<_> = ids.iter().copied().collect();
    if allowed.len() != ids.len() || allowed.contains(&0) || allowed.len() != 1 {
        return Err(invalid("one exact stone identity is required".into()));
    }
    for id in before.sealed_stones.records().chain(after.sealed_stones.records()).map(|r| r.serial()) {
        if before.sealed_stones.record(id) != after.sealed_stones.record(id) && !allowed.contains(&id) {
            return Err(invalid("stone changed outside the authorized scope".into()));
        }
        if before.sealed_stones.record(id).is_none()
            && after.sealed_stones.record(id).is_some_and(|r| !accounts.contains(&r.owner().account_id)) {
            return Err(invalid("stone minted outside the authorized account".into()));
        }
    }
    Ok(())
}

pub(super) fn build(before: &AccountStore, after: &AccountStore) -> Option<StoneMutation> {
    (before.sealed_stones != after.sealed_stones).then(|| StoneMutation {
        expected_version: before.source_stone_version,
        desired: after.sealed_stones.clone(),
    })
}

pub(super) fn validate_receipt(plan: &super::AccountStoreMutationPlan,
    receipt: &super::AccountStoreRepositorySave) -> Result<(), String>
{
    if let Some(change) = &plan.stones {
        let expected = change.expected_version.and_then(|v| v.checked_add(1));
        if expected.is_none() || receipt.stones != expected {
            return Err(format!("{COMMIT_UNKNOWN}: stone commit omitted or changed its exact durable version"));
        }
    } else if receipt.stones.is_some() {
        return Err(format!("{COMMIT_UNKNOWN}: stone version returned outside the authorized mutation"));
    }
    Ok(())
}

pub(super) fn load(client: &mut impl GenericClient, store: &mut AccountStore) -> Result<(), String> {
    let row = client.query_opt(
        "SELECT registry_json,store_version FROM sealed_stone_authority WHERE singleton=TRUE", &[])
        .map_err(|e| format!("stone authority load failed: {e}"))?
        .ok_or("stone authority migration is required")?;
    let version: i64 = row.get(1);
    if version <= 0 { return Err("invalid stone authority source version".into()); }
    let registry: Option<Value> = row.get(0);
    store.sealed_stones = match registry {
        Some(json) => serde_json::from_value(json).map_err(|e| format!("invalid stone authority: {e}"))?,
        None => StoneRegistry::default(),
    };
    store.source_stone_version = Some(version);
    validate_store(store)
}

pub(super) fn write(tx: &mut Transaction<'_>, change: Option<&StoneMutation>) -> Result<Option<i64>, String> {
    let Some(change) = change else { return Ok(None); };
    change.desired.validate().map_err(|e| e.to_string())?;
    let row = tx.query_opt(
        "SELECT registry_json,store_version FROM sealed_stone_authority WHERE singleton=TRUE FOR UPDATE", &[])
        .map_err(|e| e.to_string())?.ok_or("stone authority row is missing")?;
    let version: i64 = row.get(1);
    if change.expected_version != Some(version) || version <= 0 {
        return Err("stale stone authority; recover the original operation".into());
    }
    let old_json: Option<Value> = row.get(0);
    let old: StoneRegistry = match old_json {
        Some(json) => serde_json::from_value(json).map_err(|e| e.to_string())?,
        None => StoneRegistry::default(),
    };
    old.validate_successor(&change.desired).map_err(|e| e.to_string())?;
    let next = version.checked_add(1).ok_or("stone authority version exhausted")?;
    let json = serde_json::to_value(&change.desired).map_err(|e| e.to_string())?;
    if tx.execute("UPDATE sealed_stone_authority SET registry_json=$1,store_version=$2,updated_at=now() WHERE singleton=TRUE AND store_version=$3",
        &[&json, &next, &version]).map_err(|e| e.to_string())? != 1 {
        return Err("stone authority CAS failed".into());
    }
    Ok(Some(next))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stone_receipt_requires_exact_increment_and_rejects_unscoped_versions() {
        let cfg=super::super::SimulationConfig::default();
        let before=cfg.account_store.lock().unwrap().clone();
        let mut plan=super::super::build_account_store_mutation_plan(&before,&before,
            Scope::Accounts(&["demo".into()]),false);
        let mut receipt=super::super::AccountStoreRepositorySave::default();
        validate_receipt(&plan,&receipt).unwrap();
        receipt.stones=Some(2);assert!(validate_receipt(&plan,&receipt).is_err());
        plan.stones=Some(StoneMutation{expected_version:Some(7),desired:StoneRegistry::default()});
        for version in [None,Some(0),Some(7),Some(9)] {
            receipt.stones=version;assert!(validate_receipt(&plan,&receipt).is_err());
        }
        receipt.stones=Some(8);validate_receipt(&plan,&receipt).unwrap();
        plan.stones.as_mut().unwrap().expected_version=Some(i64::MAX);
        assert!(validate_receipt(&plan,&receipt).is_err());
    }
    #[test]
    fn stone_source_metadata_and_empty_or_duplicate_identity_scope_are_rejected() {
        let cfg=super::super::SimulationConfig::default();
        let before=cfg.account_store.lock().unwrap().clone();let mut after=before.clone();
        after.source_stone_version=Some(1);
        assert!(validate_scope(&before,&after,Scope::Accounts(&["demo".into()])).is_err());
        for ids in [&[][..],&[1,1][..],&[0][..],&[1,2][..]] {
            assert!(validate_scope(&before,&before,Scope::AccountsWithStones{account_ids:&["demo".into()],stone_ids:ids}).is_err());
        }
    }
}
