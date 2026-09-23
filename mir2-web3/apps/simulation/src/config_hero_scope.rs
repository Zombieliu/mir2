//! Exact registry mutation authorization. This module is registered with the
//! fixed transaction scopes, never exposed as a client-controlled capability.
use super::*;
use super::super::{AccountStore, AccountStoreMutationScope as Scope, AccountStoreTransactionScopeError as Error};

pub(in crate::config) fn validate_fixed_scope(original: &AccountStore, staged: &AccountStore, scope: Scope<'_>) -> Result<(), Error> {
    let invalid = Error::InvalidHeroState;
    let (accounts, ids, allocate) = match scope {
        Scope::AccountsWithHeroes { account_ids, hero_ids, allocate }
        | Scope::AccountsWithHeroesAndGuilds { account_ids, hero_ids, allocate, .. } => (account_ids, hero_ids, allocate),
        _ => {
            if original.shared_heroes != staged.shared_heroes || original.hero_id_high_watermark != staged.hero_id_high_watermark {
                return Err(invalid("Hero mutation requires a fixed Hero scope".into()));
            }
            return validate_attachment_removals(original, staged);
        }
    };
    if accounts.is_empty() { return Err(invalid("Hero transaction requires an affected account".into())); }
    if staged.hero_id_high_watermark < original.hero_id_high_watermark
        || (!allocate && staged.hero_id_high_watermark != original.hero_id_high_watermark) {
        return Err(invalid("Hero allocator changed outside allocation scope".into()));
    }
    let authorized:BTreeSet<_>=ids.iter().copied().collect();
    let authorized_accounts:BTreeSet<_>=accounts.iter().map(String::as_str).collect();
    for id in original.shared_heroes.keys().chain(staged.shared_heroes.keys()).copied().collect::<BTreeSet<_>>() {
        let before=original.shared_heroes.get(&id);let after=staged.shared_heroes.get(&id);
        if before==after { continue; }
        if !authorized.contains(&id) { return Err(invalid(format!("Hero {id} changed outside scope"))); }
        let after=after.ok_or_else(|| invalid("Hero identities cannot be deleted".into()))?;
        let revision=before.map_or(Some(1),|old| old.revision.checked_add(1));
        if revision!=Some(after.revision) {return Err(invalid("Hero revision must advance exactly once".into()));}
        if let Some(before)=before {
            if matches!(before.custody,SharedHeroCustody::Released) { return Err(invalid("released Hero cannot be revived or changed".into())); }
        } else if !allocate || id<=original.hero_id_high_watermark || id>staged.hero_id_high_watermark {
            return Err(invalid("Hero identity was not freshly allocated".into()));
        }
        for hero in before.into_iter().chain(std::iter::once(after)) {
            if let SharedHeroCustody::Attached{account_id,character_index,..}=&hero.custody {
                if !authorized_accounts.contains(account_id.as_str()) {return Err(invalid("Hero attachment changes require affected account scope".into()));}
                if !staged.accounts.get(account_id).is_some_and(|account| account.characters.iter().any(|character|character.index==*character_index)) {
                    return Err(invalid("Hero attachment character does not exist".into()));
                }
            }
        }
        if before.map(|old| &old.custody)!=Some(&after.custody) {
            if let SharedHeroCustody::Attached{attachment_revision,..}=after.custody {
                if attachment_revision!=after.revision {return Err(invalid("new Hero attachment must use current revision".into()));}
            }
        }
    }
    let allocated_count=staged.shared_heroes.keys().filter(|id| **id>original.hero_id_high_watermark).count() as u64;
    let advanced=(staged.hero_id_high_watermark-original.hero_id_high_watermark) as u64;
    if allocated_count!=advanced {return Err(invalid("Hero allocator increment requires contiguous identity records".into()));}
    validate_attachment_removals(original,staged)
}
fn validate_attachment_removals(original:&AccountStore,staged:&AccountStore)->Result<(),Error>{
    for hero in staged.shared_heroes.values(){
        if let SharedHeroCustody::Attached{account_id,character_index,..}=&hero.custody{
            let contains=|store:&AccountStore|store.accounts.get(account_id).is_some_and(|account|account.characters.iter().any(|character|character.index==*character_index));
            if contains(original)&&!contains(staged){return Err(Error::InvalidHeroState("character deletion left an attached Hero".into()));}
        }
    }
    Ok(())
}

pub(in crate::config) fn build_mutations(original:&AccountStore,desired:&AccountStore,scope:Scope<'_>)->BTreeMap<i32,HeroMutation>{
    let ids:BTreeSet<i32>=match scope{
        Scope::AccountsWithHeroes{hero_ids,..}|Scope::AccountsWithHeroesAndGuilds{hero_ids,..}=>hero_ids.iter().copied().collect(),
        Scope::FullRestore=>original.shared_heroes.keys().chain(desired.shared_heroes.keys()).copied().collect(),
        _=>BTreeSet::new(),
    };
    ids.into_iter().filter_map(|id|desired.shared_heroes.get(&id).map(|hero|(id,HeroMutation{expected_version:original.source_hero_versions.get(&id).copied(),desired:hero.clone()}))).collect()
}
pub(in crate::config) fn build_allocator_mutation(original:&AccountStore,desired:&AccountStore,scope:Scope<'_>)->Option<HeroAllocatorMutation>{
    let restore_has_authority=matches!(scope,Scope::FullRestore) && (original.source_hero_allocator_version.is_some() || original.hero_id_high_watermark!=0 || desired.hero_id_high_watermark!=0 || !original.shared_heroes.is_empty() || !desired.shared_heroes.is_empty());
    if restore_has_authority || matches!(scope,Scope::AccountsWithHeroes{allocate:true,..}|Scope::AccountsWithHeroesAndGuilds{allocate:true,..}){
        Some(HeroAllocatorMutation{expected_version:original.source_hero_allocator_version,expected_high_watermark:original.hero_id_high_watermark,desired_high_watermark:desired.hero_id_high_watermark})
    }else{None}
}
