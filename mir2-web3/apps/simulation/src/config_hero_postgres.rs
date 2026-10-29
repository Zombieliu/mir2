//! Hero registry writes participate in the caller's account transaction.
//! Lock order: optional clock, Guilds (caller), allocator, sorted Heroes, accounts.
//! Every mode uses CAS; Mirror must not replace an independent Hero authority.
use super::hero_registry::{
    validate_registry_structure, HeroAllocatorMutation, HeroMutation, HeroSourceVersions,
    SharedHeroRecord,
};
use super::{AccountStore, AccountStoreDatabaseMode};
use postgres::{GenericClient, Transaction};
use std::collections::BTreeMap;

pub(super) fn load_heroes(
    client: &mut impl GenericClient,
    store: &mut AccountStore,
) -> Result<(), String> {
    // The caller loads all account domains in a consistent read transaction.
    let allocator = client
        .query_opt(
            "SELECT high_watermark,store_version FROM shared_hero_allocator WHERE singleton=TRUE",
            &[],
        )
        .map_err(|e| format!("postgres Hero allocator load failed: {e}"))?
        .ok_or("postgres Hero allocator is missing; migrations are required")?;
    let high: i32 = allocator.get(0);
    let allocator_version: i64 = allocator.get(1);
    if allocator_version <= 0 {
        return Err("invalid Hero allocator source version".into());
    }
    let mut heroes = BTreeMap::new();
    let mut versions = BTreeMap::new();
    for row in client
        .query(
            "SELECT hero_id,raw_json,store_version FROM shared_heroes ORDER BY hero_id",
            &[],
        )
        .map_err(|e| format!("postgres Hero registry load failed: {e}"))?
    {
        let id: i32 = row.get(0);
        let record: SharedHeroRecord = serde_json::from_value(row.get(1))
            .map_err(|e| format!("invalid stored Hero {id}: {e}"))?;
        let version: i64 = row.get(2);
        if version <= 0 {
            return Err(format!("invalid Hero {id} source version"));
        }
        heroes.insert(id, record);
        versions.insert(id, version);
    }
    validate_registry_structure(&heroes, high)?;
    store.shared_heroes = heroes;
    store.hero_id_high_watermark = high;
    store.source_hero_versions = versions;
    store.source_hero_allocator_version = Some(allocator_version);
    Ok(())
}

/// Accepts only the fixed domain's validated plan. It neither commits nor catches
/// a commit error: the caller publishes Hero and source inventory together and
/// preserves unknown commit outcomes. There is deliberately no delete operation.
pub(super) fn write_hero_mutations(
    transaction: &mut Transaction<'_>,
    mutations: &BTreeMap<i32, HeroMutation>,
    allocator: Option<&HeroAllocatorMutation>,
    _mode: AccountStoreDatabaseMode,
) -> Result<HeroSourceVersions, String> {
    if mutations.is_empty() && allocator.is_none() {
        return Ok(HeroSourceVersions::default());
    }
    lock_hero_mutations(transaction, mutations, allocator)?;
    let allocator_sql = if allocator.is_some() {
        "SELECT high_watermark,store_version FROM shared_hero_allocator WHERE singleton=TRUE FOR UPDATE"
    } else {
        "SELECT high_watermark,store_version FROM shared_hero_allocator WHERE singleton=TRUE"
    };
    let row = transaction
        .query_opt(allocator_sql, &[])
        .map_err(|e| e.to_string())?
        .ok_or("postgres Hero allocator is missing")?;
    let high: i32 = row.get(0);
    let allocator_version: i64 = row.get(1);
    let desired_high = if let Some(change) = allocator {
        if change.expected_version != Some(allocator_version)
            || change.expected_high_watermark != high
        {
            return Err("stale postgres Hero allocator".into());
        }
        if change.desired_high_watermark < high || change.desired_high_watermark < 0 {
            return Err("Hero allocator cannot decrease".into());
        }
        change.desired_high_watermark
    } else {
        high
    };
    let desired: BTreeMap<_, _> = mutations
        .iter()
        .map(|(&id, m)| (id, m.desired.clone()))
        .collect();
    validate_registry_structure(&desired, desired_high)?;
    let mut versions = HeroSourceVersions::default();
    for (&id, mutation) in mutations {
        let row = transaction
            .query_opt(
                "SELECT store_version FROM shared_heroes WHERE hero_id=$1 FOR UPDATE",
                &[&id],
            )
            .map_err(|e| e.to_string())?;
        let current = row.map(|row| row.get::<_, i64>(0));
        if current != mutation.expected_version {
            return Err(format!(
                "stale postgres Hero {id}: expected {:?}, found {current:?}",
                mutation.expected_version
            ));
        }
        if current.is_none() && (allocator.is_none() || id <= high || id > desired_high) {
            return Err(format!(
                "new Hero {id} requires a fresh allocator reservation"
            ));
        }
        versions.heroes.insert(
            id,
            current
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("Hero source version exhausted")?,
        );
    }
    if allocator.is_some() {
        let version = allocator_version
            .checked_add(1)
            .ok_or("Hero allocator source version exhausted")?;
        let count = transaction.execute(
            "UPDATE shared_hero_allocator SET high_watermark=$1,store_version=$2,updated_at=now() WHERE singleton=TRUE AND store_version=$3 AND high_watermark=$4",
            &[&desired_high,&version,&allocator_version,&high],
        ).map_err(|e| format!("postgres Hero allocator write failed: {e}"))?;
        if count != 1 {
            return Err("stale postgres Hero allocator during write".into());
        }
        versions.allocator = Some(version);
    }
    for (&id, mutation) in mutations {
        let json = serde_json::to_value(&mutation.desired).map_err(|e| e.to_string())?;
        let version = versions.heroes[&id];
        let count = if let Some(expected) = mutation.expected_version {
            transaction.execute(
                "UPDATE shared_heroes SET raw_json=$1,store_version=$2,updated_at=now() WHERE hero_id=$3 AND store_version=$4",
                &[&json,&version,&id,&expected],
            )
        } else {
            transaction.execute("INSERT INTO shared_heroes(hero_id,raw_json,store_version) VALUES($1,$2,$3)", &[&id,&json,&version])
        }.map_err(|e| format!("postgres Hero {id} write failed: {e}"))?;
        if count != 1 {
            return Err(format!("stale postgres Hero {id} during write"));
        }
    }
    Ok(versions)
}

/// The account plan calls this after Guild locks and before any account locks.
/// Re-entering these transaction-scoped locks in the writer is harmless.
pub(super) fn lock_hero_mutations(
    transaction: &mut Transaction<'_>,
    mutations: &BTreeMap<i32, HeroMutation>,
    allocator: Option<&HeroAllocatorMutation>,
) -> Result<(), String> {
    if allocator.is_some() {
        transaction
            .query_opt(
                "SELECT singleton FROM shared_hero_allocator WHERE singleton=TRUE FOR UPDATE",
                &[],
            )
            .map_err(|e| format!("postgres Hero allocator lock failed: {e}"))?
            .ok_or("postgres Hero allocator is missing")?;
    }
    // Two-key advisory locks cover absent rows as well as existing identities.
    const HERO_LOCK_NAMESPACE: i32 = 721_403;
    for id in mutations.keys() {
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock($1,$2)",
                &[&HERO_LOCK_NAMESPACE, id],
            )
            .map_err(|e| format!("postgres Hero lock failed: {e}"))?;
        transaction
            .query_opt(
                "SELECT hero_id FROM shared_heroes WHERE hero_id=$1 FOR UPDATE",
                &[id],
            )
            .map_err(|e| format!("postgres Hero row lock failed: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "config_hero_postgres_tests.rs"]
mod tests;
