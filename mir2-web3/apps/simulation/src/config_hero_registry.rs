//! Durable Crystal Hero identities. Live actor IDs and process-local cast clocks
//! deliberately do not belong to this record (HeroInfo.Load resets CastTime).
use super::{HeroVitalsState, MirClass, MirGender, Stage5HeroMagicState};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[path = "config_hero_scope.rs"]
mod scope;
pub(super) use scope::{build_mutations, build_allocator_mutation};
#[path = "config_hero_map.rs"]
mod map;
pub(super) use map::deserialize_records;
#[path = "config_hero_compensation.rs"]
mod compensation;
pub(super) use compensation::prepare as prepare_compensation;
#[path = "config_hero_authority.rs"]
mod authority;
pub(super) const HERO_COMMIT_OUTCOME_UNKNOWN:&str="HERO_COMMIT_OUTCOME_UNKNOWN";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SharedHeroCustody {
    Attached {
        #[serde(rename = "accountId")]
        account_id: String,
        #[serde(rename = "characterIndex")]
        character_index: i32,
        slot: u8,
        #[serde(rename = "attachmentRevision")]
        attachment_revision: u64,
    },
    Sealed {
        #[serde(rename = "carrierUid")]
        carrier_uid: u64,
    },
    Released,
}

/// Compatibility cache identity; it is never a live-session grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedHeroAttachmentRef { pub hero_id:i32, pub revision:u64, pub slot:u8 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedHeroState {
    pub name: String,
    pub class: MirClass,
    pub gender: MirGender,
    pub hair: u8,
    pub grade: u8,
    pub level: u16,
    pub experience: i64,
    /// Full retained ItemState carriers, not a derived UserItem projection.
    pub inventory_items_json: Vec<String>,
    pub equipment_items_json: Vec<String>,
    pub inventory_capacity: u8,
    pub inventory_legacy_40: bool,
    /// None is the source's newly-created HP=-1 initialization sentinel.
    pub vitals: Option<HeroVitalsState>,
    pub magics: Vec<Stage5HeroMagicState>,
    pub seal_count: u16,
    pub auto_pot: bool,
    pub auto_hp_percent: u8,
    pub auto_mp_percent: u8,
    pub hp_item_index: i32,
    pub mp_item_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedHeroRecord {
    pub id: i32,
    pub revision: u64,
    pub custody: SharedHeroCustody,
    pub state: SharedHeroState,
}

#[derive(Debug, Clone)]
pub(super) struct HeroMutation {
    pub expected_version: Option<i64>,
    /// Deletion is not representable: released identities remain tombstones.
    pub desired: SharedHeroRecord,
}

#[derive(Debug, Clone)]
pub(super) struct HeroAllocatorMutation {
    pub expected_version: Option<i64>,
    pub expected_high_watermark: i32,
    pub desired_high_watermark: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct HeroSourceVersions {
    pub heroes: BTreeMap<i32, i64>,
    pub allocator: Option<i64>,
}

/// Validate the identity domain before any defaults/migration can repair input.
/// Carrier semantics and attachment/physical-seal ownership are checked by the
/// AccountStore integration against the complete transaction view.
pub(super) fn validate_registry_structure(
    heroes: &BTreeMap<i32, SharedHeroRecord>,
    high_watermark: i32,
) -> Result<(), String> {
    if high_watermark < 0 { return Err("negative Hero allocator".into()); }
    let mut attachments = BTreeSet::new();
    let mut seals = BTreeSet::new();
    for (&id, hero) in heroes {
        if id <= 0 || id > high_watermark || hero.id != id || hero.revision == 0 {
            return Err(format!("invalid Hero identity/revision {id}"));
        }
        match &hero.custody {
            SharedHeroCustody::Attached { account_id, character_index, slot, attachment_revision } => {
                if account_id.is_empty() || *character_index < 0 || *attachment_revision == 0
                    || *attachment_revision > hero.revision
                    || !attachments.insert((account_id, character_index, slot)) {
                    return Err(format!("invalid or duplicate Hero attachment {id}"));
                }
            }
            SharedHeroCustody::Sealed { carrier_uid } => {
                if *carrier_uid == 0 || !seals.insert(*carrier_uid) {
                    return Err(format!("invalid or duplicate Hero seal {id}"));
                }
            }
            SharedHeroCustody::Released => {}
        }
        let state = &hero.state;
        if state.name.trim().is_empty() || state.level == 0 || state.experience < 0
            || state.vitals.is_some_and(|v| v.hp < 0 || v.mp < 0)
            || state.auto_hp_percent > 99 || state.auto_mp_percent > 99 {
            return Err(format!("invalid Hero state {id}"));
        }
        match (state.inventory_capacity, state.inventory_legacy_40) {
            (10 | 18 | 26 | 34 | 42, false) | (40, true) => {}
            _ => return Err(format!("invalid Hero capacity {id}")),
        }
        let mut spells = BTreeSet::new();
        let mut keys = BTreeSet::new();
        for magic in &state.magics {
            if !spells.insert(magic.spell as u8) || !(magic.key == 0 || (17..=24).contains(&magic.key))
                || (magic.key != 0 && !keys.insert(magic.key)) {
                return Err(format!("invalid Hero magic/key {id}"));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_complete_state(store: &super::AccountStore) -> Result<(), String> {
    validate_read_view(store)?;
    for hero in store.shared_heroes.values() {
        if let SharedHeroCustody::Attached { account_id, character_index, .. } = &hero.custody {
            if !store.accounts.get(account_id).is_some_and(|account|
                account.characters.iter().any(|character| character.index == *character_index)) {
                return Err(format!("Hero {} attached to missing character", hero.id));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_read_view(store: &super::AccountStore) -> Result<(), String> {
    if store.schema_version < 5 && (!store.shared_heroes.is_empty() || store.hero_id_high_watermark != 0) {
        return Err("legacy schema cannot contain shared Hero authority".into());
    }
    validate_registry_structure(&store.shared_heroes, store.hero_id_high_watermark)?;
    let mut all_ids = BTreeSet::new();
    for hero in store.shared_heroes.values() {
        let ids=crate::runtime::validate_shared_hero_carriers(&hero.state)?;
        if matches!(hero.custody,SharedHeroCustody::Released){continue;}
        for id in ids {
            if !all_ids.insert(id) { return Err("item UID duplicated across registry Heroes".into()); }
        }
    }
    for hero in store.shared_heroes.values() {
        if let SharedHeroCustody::Sealed { carrier_uid } = hero.custody {
            if all_ids.contains(&carrier_uid) { return Err("Hero seal is also owned by a Hero".into()); }
        }
    }
    Ok(())
}

pub(super) fn validate_scope(
    original: &super::AccountStore,
    staged: &super::AccountStore,
    scope: super::AccountStoreMutationScope<'_>,
) -> Result<(), super::AccountStoreTransactionScopeError> {
    use super::{AccountStoreMutationScope as Scope, AccountStoreTransactionScopeError as Error};
    let invalid = Error::InvalidHeroState;
    if !matches!(scope, Scope::FullRestore) && (original.source_hero_versions != staged.source_hero_versions
        || original.source_hero_allocator_version != staged.source_hero_allocator_version) {
        return Err(Error::SourceMetadataChanged { field: "Hero source versions" });
    }
    validate_read_view(staged).map_err(invalid)?;
    if matches!(scope, Scope::FullRestore) {
        validate_complete_state(staged).map_err(invalid)?;
        if staged.hero_id_high_watermark < original.hero_id_high_watermark {
            return Err(invalid("Hero allocator cannot be rewound".into()));
        }
        for (id, before) in &original.shared_heroes {
            let after = staged.shared_heroes.get(id).ok_or_else(|| invalid("Hero tombstone cannot be removed".into()))?;
            if after.revision < before.revision
                || (after != before && after.revision <= before.revision)
                || (matches!(before.custody, SharedHeroCustody::Released) && after != before) {
                return Err(invalid("Hero restore would revive stale custody".into()));
            }
        }
    } else {
        scope::validate_fixed_scope(original,staged,scope)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{AccountStoreMutationScope as Scope, SimulationConfig};
    fn state() -> SharedHeroState {
        SharedHeroState {
            name: "RegistryHero".into(), class: MirClass::Warrior, gender: MirGender::Male,
            hair: 0, grade: 0, level: 1, experience: 9_007_199_254_740_993,
            inventory_items_json: vec![], equipment_items_json: vec![],
            inventory_capacity: 10, inventory_legacy_40: false, vitals: None, magics: vec![],
            seal_count: 0, auto_pot: false, auto_hp_percent: 0, auto_mp_percent: 0,
            hp_item_index: 0, mp_item_index: 0,
        }
    }
    fn sealed(id: i32, uid: u64) -> SharedHeroRecord {
        SharedHeroRecord {id, revision: 1, custody: SharedHeroCustody::Sealed {carrier_uid:uid}, state:state()}
    }
    #[test]
    fn hero_registry_roundtrip_keeps_int64_experience_and_exhausted_id() {
        let hero=sealed(i32::MAX,u64::MAX);
        let json=serde_json::to_string(&hero).unwrap();
        let decoded:SharedHeroRecord=serde_json::from_str(&json).unwrap();
        assert_eq!(decoded,hero);
        assert_eq!(decoded.state.experience,9_007_199_254_740_993);
        validate_registry_structure(&BTreeMap::from([(hero.id,hero)]),i32::MAX).unwrap();
    }
    #[test]
    fn hero_registry_rejects_duplicate_seals_and_attachment_slots() {
        let mut heroes=BTreeMap::from([(1,sealed(1,12)),(2,sealed(2,12))]);
        assert!(validate_registry_structure(&heroes,2).is_err());
        for hero in heroes.values_mut() { hero.custody=SharedHeroCustody::Attached {
            account_id:"demo".into(),character_index:0,slot:0,attachment_revision:1,
        }; }
        assert!(validate_registry_structure(&heroes,2).is_err());
        heroes.remove(&2);validate_registry_structure(&heroes,2).unwrap();
    }
    #[test]
    fn hero_registry_old_schema_cannot_promote_authority_or_allocator() {
        let config=SimulationConfig::default();
        let store=config.account_store.lock().unwrap().clone();
        let mut invalid=store.clone();invalid.schema_version=4;
        invalid.hero_id_high_watermark=1;invalid.shared_heroes.insert(1,sealed(1,1));
        let migrated=invalid.migrate_to_current_schema();
        assert_eq!(migrated.schema_version,4);
        assert!(validate_complete_state(&migrated).is_err());
        let mut allocator_only=store;allocator_only.schema_version=4;allocator_only.hero_id_high_watermark=1;
        assert!(validate_complete_state(&allocator_only.migrate_to_current_schema()).is_err());
    }
    #[test]
    fn hero_registry_account_or_global_scope_cannot_mint_shared_identity() {
        let config=SimulationConfig::default();let before=config.account_store.lock().unwrap().clone();
        let mut after=before.clone();after.hero_id_high_watermark=1;after.shared_heroes.insert(1,sealed(1,1));
        let accounts=["demo".to_owned()];
        for scope in [Scope::Accounts(&accounts),Scope::AccountsWithGlobal(&accounts)] {
            assert!(validate_scope(&before,&after,scope).is_err());
        }
    }
    #[test]
    fn hero_registry_restore_cannot_reuse_or_revive_published_identity() {
        let config=SimulationConfig::default();let mut before=config.account_store.lock().unwrap().clone();
        before.hero_id_high_watermark=1;let mut hero=sealed(1,10);hero.custody=SharedHeroCustody::Released;
        before.shared_heroes.insert(1,hero);let mut after=before.clone();
        after.shared_heroes.clear();assert!(validate_scope(&before,&after,Scope::FullRestore).is_err());
        after=before.clone();after.hero_id_high_watermark=0;
        assert!(validate_scope(&before,&after,Scope::FullRestore).is_err());
        after=before.clone();after.shared_heroes.get_mut(&1).unwrap().custody=SharedHeroCustody::Sealed{carrier_uid:11};
        after.shared_heroes.get_mut(&1).unwrap().revision=2;
        assert!(validate_scope(&before,&after,Scope::FullRestore).is_err());
    }
}

pub(super) fn validate_receipt(plan:&super::AccountStoreMutationPlan,receipt:&super::AccountStoreRepositorySave)->Result<(),String>{
    for id in plan.heroes.keys(){
        if !receipt.heroes.heroes.get(id).is_some_and(|version|*version>0){return Err(format!("Hero {id} commit omitted its durable receipt"));}
    }
    if plan.hero_allocator.is_some() && !receipt.heroes.allocator.is_some_and(|version|version>0){return Err("Hero allocator commit omitted its durable receipt".into());}
    Ok(())
}

#[cfg(test)]
#[path = "config_hero_transaction_tests.rs"]
mod transaction_tests;
