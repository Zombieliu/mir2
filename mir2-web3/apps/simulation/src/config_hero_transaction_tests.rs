use super::*;
use super::super::{AccountStore, AccountStoreMutationScope as Scope, AccountStoreRepositorySave, SimulationConfig, validate_account_store_transaction_scope, build_account_store_mutation_plan};

fn record(id:i32,uid:u64)->SharedHeroRecord{
    SharedHeroRecord{id,revision:1,custody:SharedHeroCustody::Sealed{carrier_uid:uid},state:SharedHeroState{
        name:format!("Hero{id}"),class:MirClass::Warrior,gender:MirGender::Male,hair:0,grade:0,level:1,experience:0,
        inventory_items_json:vec![],equipment_items_json:vec![],inventory_capacity:10,inventory_legacy_40:false,vitals:None,magics:vec![],seal_count:0,
        auto_pot:false,auto_hp_percent:0,auto_mp_percent:0,hp_item_index:0,mp_item_index:0,
    }}
}
fn store()->AccountStore{SimulationConfig::default().account_store.lock().unwrap().clone()}

#[test]
fn hero_registry_scope_requires_exact_ids_allocator_and_monotonic_revision(){
    let before=store();let mut after=before.clone();after.hero_id_high_watermark=1;after.shared_heroes.insert(1,record(1,12));
    let accounts=["demo".to_owned()];let ids=[1];
    let scope=Scope::AccountsWithHeroes{account_ids:&accounts,hero_ids:&ids,allocate:true};
    validate_account_store_transaction_scope(&before,&after,scope).unwrap();
    assert!(validate_account_store_transaction_scope(&before,&after,Scope::AccountsWithHeroes{account_ids:&accounts,hero_ids:&[],allocate:true}).is_err());
    assert!(validate_account_store_transaction_scope(&before,&after,Scope::AccountsWithHeroes{account_ids:&accounts,hero_ids:&ids,allocate:false}).is_err());
    let mut hole=after.clone();hole.hero_id_high_watermark=2;
    assert!(validate_account_store_transaction_scope(&before,&hole,scope).is_err());
    let mut changed=after.clone();changed.shared_heroes.get_mut(&1).unwrap().state.experience=1;
    assert!(validate_account_store_transaction_scope(&after,&changed,scope).is_err());
    changed.shared_heroes.get_mut(&1).unwrap().revision=2;
    validate_account_store_transaction_scope(&after,&changed,scope).unwrap();
}
#[test]
fn hero_registry_scope_preserves_account_global_and_source_metadata_isolation(){
    let before=store();let mut after=before.clone();after.hero_id_high_watermark=1;after.shared_heroes.insert(1,record(1,12));
    let accounts=["demo".to_owned()];let ids=[1];
    let scope=Scope::AccountsWithHeroesAndGuilds{account_ids:&accounts,hero_ids:&ids,allocate:true,guild_ids:&[]};
    after.game_shop_global_purchases.insert(1,1);
    assert!(validate_account_store_transaction_scope(&before,&after,scope).is_err());
    after.game_shop_global_purchases.clear();after.source_hero_allocator_version=Some(1);
    assert!(validate_account_store_transaction_scope(&before,&after,scope).is_err());
}
#[test]
fn hero_registry_scope_attachment_change_requires_both_owners_and_new_epoch(){
    let mut before=store();let mut peer=before.accounts["demo"].clone();peer.characters[0].index=1;before.accounts.insert("peer".into(),peer);
    before.hero_id_high_watermark=1;let mut hero=record(1,10);
    hero.custody=SharedHeroCustody::Attached{account_id:"demo".into(),character_index:0,slot:0,attachment_revision:1};before.shared_heroes.insert(1,hero);
    let mut after=before.clone();let changed=after.shared_heroes.get_mut(&1).unwrap();changed.revision=2;
    changed.custody=SharedHeroCustody::Attached{account_id:"peer".into(),character_index:1,slot:0,attachment_revision:2};
    let only_new=["peer".to_owned()];let both=["demo".to_owned(),"peer".to_owned()];let ids=[1];
    assert!(validate_account_store_transaction_scope(&before,&after,Scope::AccountsWithHeroes{account_ids:&only_new,hero_ids:&ids,allocate:false}).is_err());
    validate_account_store_transaction_scope(&before,&after,Scope::AccountsWithHeroes{account_ids:&both,hero_ids:&ids,allocate:false}).unwrap();
    if let SharedHeroCustody::Attached{attachment_revision,..}=&mut after.shared_heroes.get_mut(&1).unwrap().custody{*attachment_revision=1;}
    assert!(validate_account_store_transaction_scope(&before,&after,Scope::AccountsWithHeroes{account_ids:&both,hero_ids:&ids,allocate:false}).is_err());
}
#[test]
fn hero_registry_mirror_compensation_retires_published_new_identity_and_fences_receipt(){
    let before=store();let mut after=before.clone();after.hero_id_high_watermark=1;after.shared_heroes.insert(1,record(1,12));
    let accounts=["demo".to_owned()];let ids=[1];let scope=Scope::AccountsWithHeroes{account_ids:&accounts,hero_ids:&ids,allocate:true};
    let forward=build_account_store_mutation_plan(&before,&after,scope,false);let rollback=build_account_store_mutation_plan(&after,&before,scope,false);
    let mut receipt=AccountStoreRepositorySave::default();receipt.heroes.heroes.insert(1,9);receipt.heroes.allocator=Some(4);
    let (restored,plan)=prepare_compensation(&before,&after,&forward,&rollback,&receipt).unwrap();
    assert_eq!(restored.hero_id_high_watermark,1);assert!(matches!(restored.shared_heroes[&1].custody,SharedHeroCustody::Released));
    assert_eq!(restored.shared_heroes[&1].revision,2);assert_eq!(plan.heroes[&1].expected_version,Some(9));assert!(plan.force_source_cas);
    assert_eq!(plan.hero_allocator.as_ref().unwrap().expected_version,Some(4));assert_eq!(plan.hero_allocator.as_ref().unwrap().desired_high_watermark,1);
    assert_eq!(serde_json::to_value(&restored.accounts).unwrap(),serde_json::to_value(&before.accounts).unwrap());
    assert!(prepare_compensation(&before,&after,&forward,&rollback,&AccountStoreRepositorySave::default()).is_err());
}
#[test]
fn hero_registry_mirror_compensation_restores_business_with_fresh_attachment_revision(){
    let mut before=store();before.hero_id_high_watermark=1;let mut hero=record(1,10);
    hero.custody=SharedHeroCustody::Attached{account_id:"demo".into(),character_index:0,slot:0,attachment_revision:1};before.shared_heroes.insert(1,hero);
    let mut after=before.clone();after.shared_heroes.get_mut(&1).unwrap().revision=2;after.shared_heroes.get_mut(&1).unwrap().custody=SharedHeroCustody::Sealed{carrier_uid:99};
    let accounts=["demo".to_owned()];let ids=[1];let scope=Scope::AccountsWithHeroes{account_ids:&accounts,hero_ids:&ids,allocate:false};
    let forward=build_account_store_mutation_plan(&before,&after,scope,false);let rollback=build_account_store_mutation_plan(&after,&before,scope,false);
    let mut receipt=AccountStoreRepositorySave::default();receipt.heroes.heroes.insert(1,10);
    let (restored,plan)=prepare_compensation(&before,&after,&forward,&rollback,&receipt).unwrap();
    assert_eq!(restored.shared_heroes[&1].revision,3);assert_eq!(restored.shared_heroes[&1].state,before.shared_heroes[&1].state);
    assert!(matches!(restored.shared_heroes[&1].custody,SharedHeroCustody::Attached{attachment_revision:3,..}));
    assert_eq!(plan.heroes[&1].expected_version,Some(10));assert!(plan.hero_allocator.is_none());
}
#[test]
fn hero_registry_duplicate_json_id_is_rejected_before_record_is_discarded(){
    let value=serde_json::to_value(store()).unwrap();let mut text=serde_json::to_string(&value).unwrap();
    let hero=serde_json::to_string(&record(1,12)).unwrap();let replacement=format!("\"sharedHeroes\":{{\"1\":{hero},\"1\":{hero}}}");
    text=text.replace("\"sharedHeroes\":{}",&replacement);
    assert!(serde_json::from_str::<AccountStore>(&text).is_err());
}
