//! Complete persistent-carrier census at the registry transaction/import boundary.
use super::{hero_registry_carriers::{CarrierOrigin,HeroCarrierCensus},items::{ItemState,try_user_item_from_item_state},equipment::{EquipmentState,user_item_from_equipment_state},npc::{NpcBuyBackState,NpcUsedGoodsState}};
use crate::config::{AccountStore,CharacterSaveRecord,SharedHeroCustody,Stage5SystemsState};
use mir2_protocol::UserItem;
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;

fn decode<T:DeserializeOwned>(value:&str)->Result<T,String>{serde_json::from_str(value).map_err(|error|format!("invalid Hero custody carrier data: {error}"))}
fn add_wire(census:&mut HeroCarrierCensus,item:&UserItem,origin:CarrierOrigin,forbidden_bind:u16)->Result<(),String>{
    // Registry-owned carriers have already passed the strict carrier validator.
    // This census also covers unrelated legacy items which may predate catalog IDs.
    census.record(item,origin)?;
    fn check(item:&UserItem,forbidden:u16)->Result<(),String>{
        let template=mir2_game_data::crystal_item_by_index(item.item_index);
        if template.is_some_and(|template|template.item_type==42 && ((template.bind as u16)&forbidden)!=0) {return Err("sealed Hero item is in a source-forbidden container".into());}
        for child in item.slots.iter().flatten(){check(child,forbidden)?;}
        Ok(())
    }
    check(item,forbidden_bind)
}
fn add_items(census:&mut HeroCarrierCensus,encoded:&[String],origin:CarrierOrigin,forbidden:u16)->Result<(),String>{
    for value in encoded {
        let item:ItemState=decode(value)?;
        match try_user_item_from_item_state(&item){
            Ok(wire)=>add_wire(census,&wire,origin,forbidden)?,
            Err(_) if matches!(origin,CarrierOrigin::External)=>census.record_unresolved_legacy(&item)?,
            Err(error)=>return Err(error.to_string()),
        }
    }
    Ok(())
}
fn add_character(census:&mut HeroCarrierCensus,save:&CharacterSaveRecord,mirrored:bool)->Result<(),String>{
    let origin=CarrierOrigin::External;
    add_items(census,&save.inventory_items_json,origin,0)?;
    add_items(census,&save.belt_items_json,origin,0)?;
    add_items(census,&save.storage_items_json,origin,8)?; // DontStore
    for value in &save.equipment_items_json {
        let state:EquipmentState=decode(value)?;
        if let Some(wire)=user_item_from_equipment_state(&state){add_wire(census,&wire,origin,0)?;}
        else {let item=super::equipment::item_state_from_equipment_state(state,crate::config::ItemContainer::Bag1,0);census.record_unresolved_legacy(&item)?;}
    }
    if !mirrored {
        add_items(census,&save.hero_inventory_items_json,origin,0x8000)?;
        add_items(census,&save.hero_equipment_items_json,origin,0x8000)?;
    }
    for value in &save.npc_buy_back_items_json {
        let state:NpcBuyBackState=decode(value)?;
        for item in state.items {add_wire(census,&item.item,origin,4)?;}
    }
    for value in &save.npc_used_goods_items_json {
        let state:NpcUsedGoodsState=decode(value)?;
        for item in state.items {add_wire(census,&item,origin,4)?;}
    }
    if let Some(value)=save.stage5_systems_json.as_ref(){
        let state:Stage5SystemsState=decode(value)?;
        super::stage5::validate_stage5_systems_item_carriers(&state)?;
        for listing in state.auction.iter().filter(|listing|!listing.sold){
            if let Some(value)=listing.item_state_json.as_ref(){
                let item=super::item_custody::decode(&listing.item_key,value)?;
                let wire=try_user_item_from_item_state(&item).map_err(|error|error.to_string())?;
                add_wire(census,&wire,origin,4)?;
            }
        }
        if let Some(value)=state.refine.oven_item_state_json.as_ref(){
            let item=super::item_custody::decode(state.refine.current_item.as_deref().ok_or("refine carrier lacks key")?,value)?;
            add_wire(census,&try_user_item_from_item_state(&item).map_err(|error|error.to_string())?,origin,0)?;
        }
        for (slot,value) in &state.refine.item_states {
            let item=super::item_custody::decode(state.refine.slots.get(slot).ok_or("refine slot carrier lacks key")?,value)?;
            add_wire(census,&try_user_item_from_item_state(&item).map_err(|error|error.to_string())?,origin,0)?;
        }
        for mail in state.mail.iter().filter(|mail|!mail.claimed&&!mail.deleted){
            add_items(census,&mail.item_states_json,origin,0x4000)?;
        }
        // Trade/rental rows are references. Legacy guild storage is not the
        // shared bank authority and cannot supply a root for a registry seal.
        for value in state.guild.storage_item_states.values(){
            let item:ItemState=decode(value)?;
            let wire=try_user_item_from_item_state(&item).map_err(|error|error.to_string())?;
            if wire.added_stats.iter().any(|stat|stat.stat==129){return Err("legacy guild projection cannot own a registry Hero seal".into());}
        }
    }
    Ok(())
}

pub(crate) fn validate_shared_hero_physical_custody(store:&AccountStore)->Result<(),String>{
    let mut census=HeroCarrierCensus::default();let mut owned=BTreeSet::new();
    let mut mirrored=BTreeSet::new();
    for (account_id,account) in &store.accounts {
        for (&index,save) in &account.saves {
            if let Some(reference)=save.hero_registry_attachment {
                let hero=store.shared_heroes.get(&reference.hero_id).ok_or("Hero mirror references missing registry identity")?;
                let SharedHeroCustody::Attached{account_id:owner,character_index,slot,..}=&hero.custody else{return Err("Hero mirror references a non-attached identity".into());};
                if owner!=account_id || *character_index!=index || *slot!=reference.slot || hero.revision!=reference.revision {
                    return Err("Hero compatibility mirror owner/revision mismatch".into());
                }
                if save.hero_inventory_items_json!=hero.state.inventory_items_json || save.hero_equipment_items_json!=hero.state.equipment_items_json
                    || save.hero_inventory_capacity!=Some(hero.state.inventory_capacity) || save.hero_inventory_legacy_40!=hero.state.inventory_legacy_40
                    || save.hero_vitals!=hero.state.vitals {
                    return Err("Hero compatibility mirror differs from registry custody".into());
                }
                let systems:Stage5SystemsState=decode(save.stage5_systems_json.as_deref().ok_or("Hero mirror lacks personal state")?)?;
                let cached=systems.hero.as_ref().ok_or("Hero mirror lacks Hero identity")?;
                if cached.name!=hero.state.name || cached.class!=hero.state.class || cached.gender!=hero.state.gender
                    || cached.level!=hero.state.level || cached.experience!=hero.state.experience
                    || cached.auto_pot!=hero.state.auto_pot || cached.auto_hp_percent!=hero.state.auto_hp_percent
                    || cached.auto_mp_percent!=hero.state.auto_mp_percent || cached.hp_item_index!=hero.state.hp_item_index
                    || cached.mp_item_index!=hero.state.mp_item_index || systems.hero_learned_magics!=hero.state.magics {
                    return Err("Hero identity/magic compatibility mirror differs from registry".into());
                }
                if !mirrored.insert(hero.id){return Err("multiple character mirrors claim one Hero".into());}
            }
            add_character(&mut census,save,save.hero_registry_attachment.is_some())?;
        }
    }
    for (&id,hero) in &store.shared_heroes {
        if matches!(hero.custody,SharedHeroCustody::Released){continue;}
        if matches!(hero.custody,SharedHeroCustody::Attached{..})&&!mirrored.contains(&id){return Err("attached Hero lacks its explicit compatibility reference".into());}
        for uid in super::hero_registry_validation::validate_shared_hero_carriers(&hero.state)?{
            if !owned.insert(uid){return Err("item UID duplicated across live registry Heroes".into());}
        }
        add_items(&mut census,&hero.state.inventory_items_json,CarrierOrigin::Hero(id),0x8000)?;
        add_items(&mut census,&hero.state.equipment_items_json,CarrierOrigin::Hero(id),0x8000)?;
    }
    for guild in store.shared_guilds.values(){for item in guild.storage.values(){
        add_items(&mut census,std::slice::from_ref(&item.item_state_json),CarrierOrigin::External,8)?;
    }}
    census.validate(&store.shared_heroes,&owned)
}
