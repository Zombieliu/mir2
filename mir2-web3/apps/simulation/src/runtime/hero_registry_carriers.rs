//! Physical Hero-seal census. Historical receipts and compatibility mirrors are
//! excluded by the caller; every supplied carrier is visited exactly once.
use std::collections::{BTreeMap,BTreeSet};
use mir2_protocol::UserItem;
use crate::config::{SharedHeroCustody,SharedHeroRecord};

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub(super) enum CarrierOrigin { External, Hero(i32) }
#[derive(Default)]
pub(super) struct HeroCarrierCensus {
    pub counts:BTreeMap<u64,usize>,
    seals:BTreeMap<i32,Vec<(u64,CarrierOrigin)>>,
}
impl HeroCarrierCensus {
    fn node(&mut self,item:&UserItem,origin:CarrierOrigin)->Result<(),String>{
        *self.counts.entry(item.unique_id).or_default()+=1;
        let template=mir2_game_data::crystal_item_by_index(item.item_index);
        let hero_stats:Vec<_>=item.added_stats.iter().filter(|stat|stat.stat==129).collect();
        if template.as_ref().is_some_and(|template|template.item_type==42) || !hero_stats.is_empty(){
            if !template.is_some_and(|template|template.item_type==42) || item.count!=1 || item.unique_id==0 || hero_stats.len()!=1 || hero_stats[0].value<=0 {
                return Err("invalid sealed Hero item identity".into());
            }
            self.seals.entry(hero_stats[0].value).or_default().push((item.unique_id,origin));
        }
        Ok(())
    }
    pub fn record(&mut self,item:&UserItem,origin:CarrierOrigin)->Result<(),String>{
        let budget=super::items::UserItemCarrierBudget::default();
        let mut pending=vec![(item,0usize)];let mut nodes=0;
        while let Some((item,depth))=pending.pop(){
            nodes+=1;
            if depth>budget.max_depth || nodes>budget.max_total_nodes || item.slots.len()>budget.max_slots_per_item || item.added_stats.len()>budget.max_added_stats_per_item {
                return Err("Hero carrier census exceeds the source carrier budget".into());
            }
            self.node(item,origin)?;
            pending.extend(item.slots.iter().flatten().map(|child|(child,depth+1)));
        }
        Ok(())
    }
    /// An unrelated pre-catalog item cannot mint a seal, but its raw identities
    /// still participate in overlap checks. Never hydrate it or rewrite bytes.
    pub fn record_unresolved_legacy(&mut self,root:&super::items::ItemState)->Result<(),String>{
        enum Node<'a>{State(&'a super::items::ItemState),Wire(&'a UserItem)}
        let budget=super::items::UserItemCarrierBudget::default();let mut nodes=0;
        let mut pending=vec![(Node::State(root),0usize)];
        while let Some((node,depth))=pending.pop(){
            nodes+=1;if depth>budget.max_depth || nodes>budget.max_total_nodes{return Err("legacy carrier census exceeds its budget".into());}
            match node {
                Node::State(item)=>{
                    if item.added_stats.iter().any(|stat|stat.stat==129)
                        || item.user_item_metadata.as_ref().and_then(|meta|meta.item_index).and_then(mir2_game_data::crystal_item_by_index).is_some_and(|template|template.item_type==42){return Err("unresolved legacy carrier cannot contain a Hero seal".into());}
                    if item.socketed.len()>budget.max_slots_per_item{return Err("legacy socket census exceeds its budget".into());}
                    *self.counts.entry(item.unique_id).or_default()+=1;
                    pending.extend(item.socketed.iter().map(|child|(Node::State(child),depth+1)));
                    if let Some(meta)=item.user_item_metadata.as_ref(){
                        if meta.slots.len()>budget.max_slots_per_item{return Err("legacy retained slots exceed census budget".into());}
                        pending.extend(meta.slots.iter().flatten().map(|child|(Node::Wire(child),depth+1)));
                    }
                }
                Node::Wire(item)=>{
                    if item.added_stats.iter().any(|stat|stat.stat==129) || mir2_game_data::crystal_item_by_index(item.item_index).is_some_and(|template|template.item_type==42){return Err("unresolved legacy container cannot own a Hero seal".into());}
                    if item.slots.len()>budget.max_slots_per_item{return Err("legacy wire slots exceed census budget".into());}
                    *self.counts.entry(item.unique_id).or_default()+=1;
                    pending.extend(item.slots.iter().flatten().map(|child|(Node::Wire(child),depth+1)));
                }
            }
        }
        Ok(())
    }
    pub fn validate(&self,heroes:&BTreeMap<i32,SharedHeroRecord>,owned_ids:&BTreeSet<u64>)->Result<(),String>{
        for id in owned_ids {
            if self.counts.get(id)!=Some(&1){return Err(format!("registry Hero item UID {id} has ambiguous physical custody"));}
        }
        let mut parents=BTreeMap::new();
        for (&id,entries) in &self.seals {
            let hero=heroes.get(&id).ok_or("seal references an unregistered Hero")?;
            let SharedHeroCustody::Sealed{carrier_uid}=hero.custody else {return Err("seal references an attached or released Hero".into());};
            if entries.len()!=1 || entries[0].0!=carrier_uid || self.counts.get(&carrier_uid)!=Some(&1){return Err("duplicate or mismatched physical Hero seal".into());}
            parents.insert(id,entries[0].1);
        }
        for (&id,hero) in heroes {
            if !matches!(hero.custody,SharedHeroCustody::Sealed{..}){continue;}
            let mut seen=BTreeSet::new();let mut cursor=id;
            loop {
                if !seen.insert(cursor){return Err("sealed Hero containment cycle has no external owner".into());}
                let current=heroes.get(&cursor).ok_or("Hero seal container missing")?;
                match &current.custody {
                    SharedHeroCustody::Attached{..}=>break,
                    SharedHeroCustody::Released=>return Err("released Hero cannot own a seal".into()),
                    SharedHeroCustody::Sealed{..}=>match parents.get(&cursor).ok_or("sealed Hero has no physical carrier")? {
                        CarrierOrigin::External=>break,
                        CarrierOrigin::Hero(parent)=>cursor=*parent,
                    },
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SharedHeroState,ItemContainer};
    use mir2_protocol::{MirClass,MirGender,UserItemStat};
    fn hero(id:i32,uid:u64)->SharedHeroRecord{
        SharedHeroRecord{id,revision:1,custody:SharedHeroCustody::Sealed{carrier_uid:uid},state:SharedHeroState{
            name:format!("Hero{id}"),class:MirClass::Warrior,gender:MirGender::Male,hair:0,grade:0,level:1,experience:0,
            inventory_items_json:vec![],equipment_items_json:vec![],inventory_capacity:10,inventory_legacy_40:false,vitals:None,magics:vec![],seal_count:0,
            auto_pot:false,auto_hp_percent:0,auto_mp_percent:0,hp_item_index:0,mp_item_index:0,
        }}
    }
    fn seal(id:i32,uid:u64)->UserItem{
        let template=mir2_game_data::crystal_item_by_index(1349).unwrap();assert_eq!(template.bind as u16,0x701f);
        let mut item=super::super::items::embedded_item_state_from_template(&template,ItemContainer::Bag1,0);
        item.unique_id=uid;item.added_stats.push(UserItemStat{stat:129,value:id});
        super::super::items::try_user_item_from_item_state(&item).unwrap()
    }
    #[test]
    fn hero_registry_containment_allows_seal_inside_another_externally_owned_hero(){
        let heroes=BTreeMap::from([(1,hero(1,101)),(2,hero(2,102))]);let mut census=HeroCarrierCensus::default();
        census.record(&seal(1,101),CarrierOrigin::External).unwrap();census.record(&seal(2,102),CarrierOrigin::Hero(1)).unwrap();
        census.validate(&heroes,&BTreeSet::from([102])).unwrap();
    }
    #[test]
    fn hero_registry_containment_rejects_self_and_mutual_cycles(){
        let heroes=BTreeMap::from([(1,hero(1,101))]);let mut census=HeroCarrierCensus::default();
        census.record(&seal(1,101),CarrierOrigin::Hero(1)).unwrap();assert!(census.validate(&heroes,&BTreeSet::from([101])).unwrap_err().contains("cycle"));
        let heroes=BTreeMap::from([(1,hero(1,101)),(2,hero(2,102))]);let mut census=HeroCarrierCensus::default();
        census.record(&seal(2,102),CarrierOrigin::Hero(1)).unwrap();census.record(&seal(1,101),CarrierOrigin::Hero(2)).unwrap();
        assert!(census.validate(&heroes,&BTreeSet::from([101,102])).unwrap_err().contains("cycle"));
    }
    #[test]
    fn hero_registry_containment_rejects_copied_and_attached_seals(){
        let mut heroes=BTreeMap::from([(1,hero(1,101))]);let mut census=HeroCarrierCensus::default();
        census.record(&seal(1,101),CarrierOrigin::External).unwrap();census.record(&seal(1,101),CarrierOrigin::External).unwrap();
        assert!(census.validate(&heroes,&BTreeSet::new()).is_err());
        let mut census=HeroCarrierCensus::default();census.record(&seal(1,101),CarrierOrigin::External).unwrap();
        heroes.get_mut(&1).unwrap().custody=SharedHeroCustody::Attached{account_id:"demo".into(),character_index:0,slot:0,attachment_revision:1};
        assert!(census.validate(&heroes,&BTreeSet::new()).is_err());
    }
    #[test]
    fn hero_registry_containment_bounds_depth_and_does_not_hydrate_unknown_items(){
        let mut item=seal(1,101);item.item_index=-999;item.added_stats.clear();
        let mut census=HeroCarrierCensus::default();census.record(&item,CarrierOrigin::External).unwrap();
        assert_eq!(census.counts.get(&101),Some(&1));
        item.added_stats.push(UserItemStat{stat:129,value:1});assert!(HeroCarrierCensus::default().record(&item,CarrierOrigin::External).is_err());
        let mut deep=seal(1,101);deep.added_stats.clear();deep.item_index=658;
        for _ in 0..10{let mut parent=deep.clone();parent.slots=vec![Some(deep)];deep=parent;}
        assert!(HeroCarrierCensus::default().record(&deep,CarrierOrigin::External).unwrap_err().contains("budget"));
    }
}
