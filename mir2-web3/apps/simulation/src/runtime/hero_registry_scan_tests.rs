use super::*;
use crate::config::{SharedHeroAttachmentRef,SharedHeroRecord,SharedHeroState,Stage5HeroState,SimulationConfig,ItemContainer};
use mir2_protocol::{MirClass,MirGender,UserItemStat};
fn seal(hero_id:i32,uid:u64,slot:u8)->String{
    let template=mir2_game_data::crystal_item_by_index(1349).unwrap();
    let mut item=super::super::items::embedded_item_state_from_template(&template,ItemContainer::Bag1,slot);
    item.unique_id=uid;item.added_stats.push(UserItemStat{stat:129,value:hero_id});
    serde_json::to_string(&item).unwrap()
}
fn hero(id:i32,uid:u64)->SharedHeroRecord{
    SharedHeroRecord{id,revision:1,custody:SharedHeroCustody::Sealed{carrier_uid:uid},state:SharedHeroState{
        name:format!("Hero{id}"),class:MirClass::Warrior,gender:MirGender::Male,hair:0,grade:0,level:1,experience:9_007_199_254_740_993,
        inventory_items_json:vec![],equipment_items_json:vec![],inventory_capacity:10,inventory_legacy_40:false,vitals:None,magics:vec![],seal_count:0,
        auto_pot:false,auto_hp_percent:0,auto_mp_percent:0,hp_item_index:0,mp_item_index:0,
    }}
}
fn store()->AccountStore{SimulationConfig::default().account_store.lock().unwrap().clone()}
fn attach(store:&mut AccountStore,mut hero:SharedHeroRecord){
    hero.custody=SharedHeroCustody::Attached{account_id:"demo".into(),character_index:0,slot:0,attachment_revision:hero.revision};
    let save=store.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap();
    save.hero_registry_attachment=Some(SharedHeroAttachmentRef{hero_id:hero.id,revision:hero.revision,slot:0});
    save.hero_inventory_items_json=hero.state.inventory_items_json.clone();save.hero_equipment_items_json=hero.state.equipment_items_json.clone();
    save.hero_inventory_capacity=Some(hero.state.inventory_capacity);save.hero_inventory_legacy_40=hero.state.inventory_legacy_40;save.hero_vitals=hero.state.vitals;
    let mut systems:Stage5SystemsState=save.stage5_systems_json.as_deref().map(|value|serde_json::from_str(value).unwrap()).unwrap_or_default();
    systems.hero=Some(Stage5HeroState{name:hero.state.name.clone(),level:hero.state.level,class:hero.state.class,gender:hero.state.gender,experience:hero.state.experience,
        behaviour:0,spawned:false,auto_pot:hero.state.auto_pot,auto_hp_percent:hero.state.auto_hp_percent,auto_mp_percent:hero.state.auto_mp_percent,hp_item_index:hero.state.hp_item_index,mp_item_index:hero.state.mp_item_index});
    systems.hero_learned_magics=hero.state.magics.clone();save.stage5_systems_json=Some(serde_json::to_string(&systems).unwrap());
    store.shared_heroes.insert(hero.id,hero);
}
#[test]
fn hero_registry_physical_census_accepts_nested_source_seals_and_exact_attachment_mirror(){
    let mut value=store();value.hero_id_high_watermark=2;
    let mut parent=hero(1,101);parent.state.inventory_items_json.push(seal(2,102,0));
    value.shared_heroes.insert(1,parent.clone());value.shared_heroes.insert(2,hero(2,102));
    value.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap().inventory_items_json.push(seal(1,101,0));
    validate_shared_hero_physical_custody(&value).unwrap();
    value.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap().inventory_items_json.clear();attach(&mut value,parent);
    validate_shared_hero_physical_custody(&value).unwrap();
    let original=serde_json::to_value(&value).unwrap();
    value.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap().hero_registry_attachment=None;
    assert!(validate_shared_hero_physical_custody(&value).is_err());
    assert_eq!(value.shared_heroes[&1].state.experience,9_007_199_254_740_993);
    assert_eq!(original["sharedHeroes"]["1"]["state"]["experience"].as_i64(),Some(9_007_199_254_740_993));
}
#[test]
fn hero_registry_physical_census_rejects_hidden_cycle_without_mutating_saved_bytes(){
    let mut value=store();value.hero_id_high_watermark=2;
    let mut a=hero(1,101);let mut b=hero(2,102);
    a.state.inventory_items_json.push(seal(2,102,0));b.state.inventory_items_json.push(seal(1,101,0));
    value.shared_heroes.insert(1,a);value.shared_heroes.insert(2,b);
    let before=serde_json::to_vec(&value).unwrap();
    assert!(validate_shared_hero_physical_custody(&value).unwrap_err().contains("cycle"));
    assert_eq!(serde_json::to_vec(&value).unwrap(),before);
}
#[test]
fn hero_registry_physical_census_rejects_default_seal_storage_and_copies(){
    let mut value=store();value.hero_id_high_watermark=1;value.shared_heroes.insert(1,hero(1,101));
    let save=value.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap();save.storage_items_json.push(seal(1,101,0));
    assert!(validate_shared_hero_physical_custody(&value).unwrap_err().contains("forbidden"));
    let save=value.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap();save.storage_items_json.clear();save.inventory_items_json=vec![seal(1,101,0),seal(1,101,1)];
    assert!(validate_shared_hero_physical_custody(&value).is_err());
}
#[test]
fn hero_registry_physical_census_preserves_unrelated_unknown_legacy_carrier(){
    let mut value=store();value.hero_id_high_watermark=1;value.shared_heroes.insert(1,hero(1,101));
    let mut unknown:ItemState=serde_json::from_str(&seal(1,777,1)).unwrap();unknown.key="legacy-unknown".into();unknown.name="Old unknown".into();unknown.added_stats.clear();unknown.user_item_metadata=None;
    let encoded=serde_json::to_string(&unknown).unwrap();
    let save=value.accounts.get_mut("demo").unwrap().saves.get_mut(&0).unwrap();save.inventory_items_json=vec![seal(1,101,0),encoded.clone()];
    validate_shared_hero_physical_custody(&value).unwrap();
    assert_eq!(value.accounts["demo"].saves[&0].inventory_items_json[1],encoded);
}
