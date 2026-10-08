//! Comparable Hero action planning inputs. Display data only; never operation custody.
use std::collections::BTreeSet;
use serde::Serialize;
use serde_json::{json, Value};
use mir2_protocol::{ClientPacket as C, MirGridType as G};
use crate::{
    inventory::{CrystalItemInfoModel, ItemModel},
    crystal_ui::hero_dialog::{cross, item_use},
    portable_hero_ui::{HeroCell, HeroGrid, HeroPotGrid, HeroSemanticAction, HeroUiReadModel, HeroWindows},
};

pub const MAX_ACTION_BASIS_BYTES: usize = 16 * 1024;
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroActionBasis {
    pub version: u8, pub actor: HeroBasisActor, pub windows: HeroWindows,
    pub cells: Vec<HeroBasisCell>, pub facts: HeroBasisFacts, pub resolved_wire: Value,
    pub confirmation_required: bool, pub cross_player: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroBasisActor {pub object_id:u32,pub name:String,pub class:String,pub gender:String,pub spawned:bool}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroBasisItem {pub uid:String,pub item_index:i32,pub count:u32}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroBasisCell {pub grid:HeroGrid,pub slot:u32,pub capacity:u32,pub item:Option<HeroBasisItem>}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroBasePolicy {pub item_index:i32,pub item_type:u8,pub shape:i16,pub stack_size:u16}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroEffectivePolicy {
    pub item_index:i32,pub item_type:u8,pub shape:i16,pub stack_size:u16,
    pub required_class:u8,pub required_gender:u8,pub required_type:u8,pub required_amount:u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroCellPolicy {pub grid:HeroGrid,pub slot:u32,pub base:HeroBasePolicy,pub effective:HeroEffectivePolicy}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroBasisRequirement {pub passed:bool,pub stat_value:Option<i32>}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroBasisTarget {pub mode:&'static str,pub slot:Option<u32>}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroBasisKey {pub spell:String,pub key:u8}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroBasisRestock {pub belt:u8,pub from:u8,pub uid:String,pub item_index:i32}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all="camelCase")]
pub struct HeroBasisFacts {
    pub hp:i32,pub level:u16,pub riding:Option<bool>,pub auto_pot:bool,
    pub policies:Vec<HeroCellPolicy>,pub requirement:Option<HeroBasisRequirement>,
    pub target:HeroBasisTarget,pub keys:Vec<HeroBasisKey>,pub restock:Option<HeroBasisRestock>,pub confirmed:bool,
}

fn rank(grid:HeroGrid)->u8 {match grid {HeroGrid::HeroInventory=>0,HeroGrid::HeroEquipment=>1,HeroGrid::Inventory=>2}}
fn capacity(read:&HeroUiReadModel,grid:HeroGrid)->Option<u32> {
    match grid {
        HeroGrid::HeroInventory=>Some(u32::from(read.hero.inventory_view.capacity)),
        HeroGrid::HeroEquipment=>Some(14),
        HeroGrid::Inventory=>Some(u32::from(read.personal.as_ref()?.bag_slot_capacity())),
    }
}
fn item_at(read:&HeroUiReadModel,cell:HeroCell)->Option<&ItemModel> {
    let (model,container)=match cell.grid {
        HeroGrid::HeroInventory=>(&read.hero.inventory_view,0),
        HeroGrid::HeroEquipment=>(&read.hero.inventory_view,2),
        HeroGrid::Inventory=>(read.personal.as_ref()?,0),
    };
    model.items.iter().find(|item|item.container==container && item.slot==cell.slot && item.quantity>0)
}
fn source_item(read:&HeroUiReadModel,cell:HeroCell)->Option<&ItemModel> {
    if cell.slot>=capacity(read,cell.grid)? {return None;}
    item_at(read,cell).filter(|item|item.unique_id.is_some_and(|uid|uid!=0))
}
fn hero_items_match(read:&HeroUiReadModel)->bool {
    let Some(hero)=read.hero.info.as_ref() else{return false;};
    let mut placements=BTreeSet::new();let mut ids=BTreeSet::new();
    for item in &read.hero.inventory_view.items {
        if item.quantity==0 || !matches!(item.container,0|2) || !placements.insert((item.container,item.slot))
            || item.unique_id.is_none() || item.unique_id.is_some_and(|uid|uid!=0&&!ids.insert(uid)){return false;}
    }
    for (which,raw) in [(HeroGrid::HeroInventory,hero.inventory.as_ref()),(HeroGrid::HeroEquipment,hero.equipment.as_ref())] {
        let Some(raw)=raw else{return false;};
        if Some(raw.len() as u32)!=capacity(read,which){return false;}
        for (slot,user) in raw.iter().enumerate() {
            let item=item_at(read,cell(which,slot as u32));
            match (user,item) {
                (None,None)=>{},
                (Some(user),Some(item)) if item.unique_id==Some(user.unique_id)
                    && item.quantity==u32::from(user.count) && item.tooltip_source.as_ref().is_some_and(|tooltip|
                        tooltip.info.item_index==user.item_index && tooltip.user_item.as_ref().is_some_and(|view|
                            view.unique_id==user.unique_id && view.item_index==user.item_index && view.count==user.count))=>{},
                _=>return false,
            }
        }
    }
    read.hero.inventory_view.items.iter().all(|item|item.slot<if item.container==0{u32::from(read.hero.inventory_view.capacity)}else{14})
}
fn effective(item:&ItemModel)->Option<&CrystalItemInfoModel> {
    let tooltip=item.tooltip_source.as_ref()?;Some(tooltip.real_info.as_ref().unwrap_or(&tooltip.info))
}
fn grid(grid:HeroGrid)->G {match grid {HeroGrid::HeroInventory=>G::HeroInventory,HeroGrid::HeroEquipment=>G::HeroEquipment,HeroGrid::Inventory=>G::Inventory}}
fn cell(grid:HeroGrid,slot:u32)->HeroCell {HeroCell{grid,slot}}
fn matching_uid_cell(read:&HeroUiReadModel,which:HeroGrid,uid:u64)->Option<HeroCell> {
    let mut found=None;
    for slot in 0..capacity(read,which)? {
        if item_at(read,cell(which,slot)).is_some_and(|item|item.unique_id==Some(uid)) {
            if found.is_some(){return None;}found=Some(cell(which,slot));
        }
    }
    found
}
fn base_policy(info:&CrystalItemInfoModel)->HeroBasePolicy {
    HeroBasePolicy{item_index:info.item_index,item_type:info.item_type,shape:info.shape,stack_size:info.stack_size}
}
fn effective_policy(info:&CrystalItemInfoModel)->HeroEffectivePolicy {
    HeroEffectivePolicy{item_index:info.item_index,item_type:info.item_type,shape:info.shape,stack_size:info.stack_size,
        required_class:info.required_class,required_gender:info.required_gender,required_type:info.required_type,required_amount:info.required_amount}
}
fn requirement(read:&HeroUiReadModel,info:&CrystalItemInfoModel)->HeroBasisRequirement {
    let stat=match info.required_type {1=>Some(1),2=>Some(3),3=>Some(5),4=>Some(7),5=>Some(9),7=>Some(0),8=>Some(2),9=>Some(4),10=>Some(6),11=>Some(8),_=>None};
    HeroBasisRequirement{passed:item_use::requirements(&read.hero,info),stat_value:stat.map(|stat|
        read.hero.stats.as_ref().and_then(|stats|stats.iter().find(|s|s.stat==stat)).map_or(0,|s|s.value))}
}
fn wire(packet:&C)->Option<Value> {
    let grid_name=|g:&G|match g {G::HeroInventory=>Some("HeroInventory"),G::HeroEquipment=>Some("HeroEquipment"),G::Inventory=>Some("Inventory"),G::HeroHpItem=>Some("HeroHpItem"),G::HeroMpItem=>Some("HeroMpItem"),_=>None};
    Some(match packet {
        C::MoveItem{grid,from,to}=>json!({"type":"moveItem","grid":grid_name(grid)?,"from":from,"to":to}),
        C::EquipItem{grid,unique_id,to}=>json!({"type":"equipItem","grid":grid_name(grid)?,"uniqueId":unique_id.to_string(),"to":to}),
        C::RemoveItem{grid,unique_id,to}=>json!({"type":"removeItem","grid":grid_name(grid)?,"uniqueId":unique_id.to_string(),"to":to}),
        C::MergeItem{grid_from,grid_to,id_from,id_to}=>json!({"type":"mergeItem","gridFrom":grid_name(grid_from)?,"gridTo":grid_name(grid_to)?,"idFrom":id_from.to_string(),"idTo":id_to.to_string()}),
        C::TransferHeroItem{from,to}=>json!({"type":"transferHeroItem","from":from,"to":to}),
        C::TakeBackHeroItem{from,to}=>json!({"type":"takeBackHeroItem","from":from,"to":to}),
        C::UseItem{grid,unique_id}=>json!({"type":"useItem","grid":grid_name(grid)?,"uniqueId":unique_id.to_string()}),
        C::SetAutoPotValue{stat,value}=>json!({"type":"setAutoPotValue","stat":stat,"value":value}),
        C::SetAutoPotItem{grid,item_index}=>json!({"type":"setAutoPotItem","grid":grid_name(grid)?,"itemIndex":item_index}),
        C::MagicKey{spell,key,old_key}=>json!({"type":"magicKey","spell":format!("{spell:?}"),"key":key,"oldKey":old_key}),
        _=>return None,
    })
}

/// Recompute original pure planners and capture every input used by their decision.
/// UIDs are comparison strings; this API does not authorize a numeric transport.
pub fn capture_action_basis(read:&HeroUiReadModel,windows:HeroWindows,action:&HeroSemanticAction,old_key:Option<u8>)->Option<HeroActionBasis> {
    let hero=read.hero.info.as_ref()?;
    if hero.object_id==0 || hero.name.is_empty() || hero.name.len()>256 || !read.hero.spawned {return None;}
    let inventory=hero.inventory.as_ref()?;
    if ![10,18,26,34,42].contains(&inventory.len()) || inventory.len()!=usize::from(read.hero.inventory_view.capacity)
        || hero.equipment.as_ref()?.len()!=14 {return None;}
    if !hero_items_match(read){return None;}
    let mut requested=Vec::<(HeroCell,bool)>::new();
    let mut need=|which:HeroGrid,slot:u32,policy:bool| {requested.push((cell(which,slot),policy));};
    let mut facts=HeroBasisFacts {hp:hero.hp,level:hero.level,riding:read.hero.riding_mount,auto_pot:hero.auto_pot,
        policies:Vec::new(),requirement:None,target:HeroBasisTarget{mode:"none",slot:None},keys:Vec::new(),restock:None,confirmed:false};
    let mut confirmation_required=false;
    let mut cross_player=false;
    let packet=match action {
        HeroSemanticAction::Move{from,to}=> {
            if from==to || source_item(read,cell(HeroGrid::HeroInventory,*from)).is_none() || *to>=capacity(read,HeroGrid::HeroInventory)? {return None;}
            if item_at(read,cell(HeroGrid::HeroInventory,*to)).is_some_and(|i|i.unique_id==Some(0)){return None;}
            need(HeroGrid::HeroInventory,*from,true);need(HeroGrid::HeroInventory,*to,true);
            facts.target=HeroBasisTarget{mode:"explicit",slot:Some(*to)};
            C::MoveItem{grid:G::HeroInventory,from:i32::try_from(*from).ok()?,to:i32::try_from(*to).ok()?}
        }
        HeroSemanticAction::Equip{from,to}=> {
            let item=source_item(read,cell(HeroGrid::HeroInventory,*from))?;let info=effective(item)?;
            let valid_target=match info.item_type {1=>*to==0,2=>*to==1,4=>*to==2,5=>*to==4,6=>matches!(*to,5|6),7=>matches!(*to,7|8),8=>*to==9,9=>*to==10,10=>*to==11,11=>*to==12,12=>*to==3,19=>*to==13,_=>false};
            let req=requirement(read,info);if !valid_target || !req.passed || hero.hp<=0 || read.hero.riding_mount==Some(true)&&info.item_type!=12 {return None;}
            if item_at(read,cell(HeroGrid::HeroEquipment,*to)).is_some_and(|i|i.unique_id==Some(0)){return None;}
            facts.requirement=Some(req);facts.target=HeroBasisTarget{mode:"explicit",slot:Some(*to)};
            need(HeroGrid::HeroInventory,*from,true);need(HeroGrid::HeroEquipment,*to,true);
            C::EquipItem{grid:G::HeroInventory,unique_id:item.unique_id?,to:i32::try_from(*to).ok()?}
        }
        HeroSemanticAction::Remove{from,to}=> {
            let source=source_item(read,cell(HeroGrid::HeroEquipment,*from))?;
            let original=item_use::remove_plan(&read.hero,u8::try_from(*from).ok()?)?;
            let C::RemoveItem{grid,unique_id,to:auto_to}=original else{return None;};
            if source.unique_id!=Some(unique_id) || unique_id==0{return None;}
            let resolved=to.map_or(Some(auto_to),|to|i32::try_from(to).ok())?;
            let resolved_slot=u32::try_from(resolved).ok()?;
            if resolved_slot>=capacity(read,HeroGrid::HeroInventory)? || item_at(read,cell(HeroGrid::HeroInventory,resolved_slot)).is_some(){return None;}
            need(HeroGrid::HeroEquipment,*from,true);
            if to.is_none(){for slot in 0..capacity(read,HeroGrid::HeroInventory)? {need(HeroGrid::HeroInventory,slot,false);}}
            else{need(HeroGrid::HeroInventory,resolved_slot,true);}
            facts.target=HeroBasisTarget{mode:if to.is_none(){"automatic"}else{"explicit"},slot:Some(resolved_slot)};
            C::RemoveItem{grid,unique_id,to:resolved}
        }
        HeroSemanticAction::Merge{from,to}=> {
            if from==to || from.grid==HeroGrid::Inventory&&to.grid==HeroGrid::Inventory
                || [from.grid,to.grid].contains(&HeroGrid::Inventory)&&[from.grid,to.grid].contains(&HeroGrid::HeroEquipment){return None;}
            let packet=item_use::merge_packet(source_item(read,*from)?,source_item(read,*to)?,grid(from.grid),grid(to.grid))?;
            need(from.grid,from.slot,true);need(to.grid,to.slot,true);cross_player=[from.grid,to.grid].contains(&HeroGrid::Inventory);
            facts.target=HeroBasisTarget{mode:"explicit",slot:Some(to.slot)};packet
        }
        HeroSemanticAction::Transfer{from,to}|HeroSemanticAction::TakeBack{from,to}=> {
            let transfer=matches!(action,HeroSemanticAction::Transfer{..});
            let (a,b)=if transfer{(cell(HeroGrid::Inventory,*from),cell(HeroGrid::HeroInventory,*to))}else{(cell(HeroGrid::HeroInventory,*from),cell(HeroGrid::Inventory,*to))};
            if b.slot>=capacity(read,b.grid)? {return None;}
            let source=source_item(read,a)?;let target=item_at(read,b);
            if target.is_some_and(|i|!i.unique_id.is_some_and(|uid|uid!=0)){return None;}
            let as_cross=|cell:HeroCell| match cell.grid {HeroGrid::Inventory=>Some(cross::Cell::Player(cell.slot)),HeroGrid::HeroInventory=>Some(cross::Cell::Hero(u8::try_from(cell.slot).ok()?)),_=>None};
            let packet=cross::transfer_packet(as_cross(a)?,as_cross(b)?,source,target)?;
            need(a.grid,a.slot,true);need(b.grid,b.slot,true);cross_player=true;
            facts.target=HeroBasisTarget{mode:"explicit",slot:Some(b.slot)};packet
        }
        HeroSemanticAction::Use{slot,confirmed}=> {
            let source=source_item(read,cell(HeroGrid::HeroInventory,*slot))?;let info=effective(source)?;
            facts.requirement=Some(requirement(read,info));facts.confirmed=*confirmed;
            let packet=match item_use::plan(&read.hero,u8::try_from(*slot).ok()?) {
                item_use::HeroUsePlan::Packet(packet)=>packet,
                item_use::HeroUsePlan::ConfirmPotion(packet)=>{confirmation_required=!*confirmed;packet},
                _=>return None,
            };
            need(HeroGrid::HeroInventory,*slot,true);
            if info.item_type==6{need(HeroGrid::HeroEquipment,6,true);}if info.item_type==7{need(HeroGrid::HeroEquipment,8,true);}
            match &packet {
                C::EquipItem{to,..}=>{let to=u32::try_from(*to).ok()?;need(HeroGrid::HeroEquipment,to,true);facts.target=HeroBasisTarget{mode:"automatic",slot:Some(to)};}
                C::MergeItem{id_to,..}=>{let target=matching_uid_cell(read,HeroGrid::HeroEquipment,*id_to)?;need(target.grid,target.slot,true);facts.target=HeroBasisTarget{mode:"automatic",slot:Some(target.slot)};}
                C::UseItem{..} if *slot<=1 && source.quantity==1=> {
                    for at in 0..capacity(read,HeroGrid::HeroInventory)? {need(HeroGrid::HeroInventory,at,false);}
                    if let Some((belt,from,uid))=item_use::restock_candidate(&read.hero,u8::try_from(*slot).ok()?) {
                        facts.restock=Some(HeroBasisRestock{belt,from,uid:uid.to_string(),item_index:source.tooltip_source.as_ref()?.info.item_index});
                    }
                }
                _=>{}
            }
            packet
        }
        HeroSemanticAction::AutoPotValue{stat,value}=> {
            if !hero.auto_pot || !matches!(*stat,12|13) || *value>99{return None;}
            C::SetAutoPotValue{stat:*stat,value:*value}
        }
        HeroSemanticAction::AutoPotItem{grid:which,slot}=> {
            if !hero.auto_pot{return None;}
            let item_index=if let Some(slot)=slot {
                let item=source_item(read,cell(HeroGrid::HeroInventory,*slot))?;let info=&item.tooltip_source.as_ref()?.info;
                if hero.hp<=0 || info.item_type!=13 || info.shape>1{return None;}
                need(HeroGrid::HeroInventory,*slot,true);facts.target=HeroBasisTarget{mode:"explicit",slot:Some(*slot)};info.item_index
            }else{0};
            C::SetAutoPotItem{grid:match which{HeroPotGrid::HeroHpItem=>G::HeroHpItem,HeroPotGrid::HeroMpItem=>G::HeroMpItem},item_index}
        }
        HeroSemanticAction::MagicKey{spell,key}=> {
            if hero.hp<=0 || !(*key==0 || (17..=24).contains(key)){return None;}
            for magic in &hero.magics{facts.keys.push(HeroBasisKey{spell:format!("{:?}",magic.spell),key:magic.key});}
            if facts.keys.len()>256{return None;}facts.keys.sort_by(|a,b|a.spell.cmp(&b.spell));
            if facts.keys.windows(2).any(|pair|pair[0].spell==pair[1].spell)
                || facts.keys.iter().any(|entry|entry.key!=0 && !(17..=24).contains(&entry.key)){return None;}
            let magic=hero.magics.iter().find(|magic|format!("{:?}",magic.spell)==*spell)?;
            if old_key!=Some(magic.key) || *key==0&&magic.key==0{return None;}
            C::MagicKey{spell:magic.spell,key:*key,old_key:magic.key}
        }
    };
    drop(need);
    // Scan-only cells establish occupancy and identity without unused item policies.
    let policy_cells:BTreeSet<_>=requested.iter().filter(|(_,policy)|*policy)
        .map(|(cell,_)|(rank(cell.grid),cell.slot)).collect();
    let mut requested:Vec<_>=requested.into_iter().map(|(cell,_)|cell).collect();
    requested.sort_by_key(|cell|(rank(cell.grid),cell.slot));requested.dedup();
    let mut cells=Vec::new();let mut seen=BTreeSet::new();
    for cell in requested {
        let cap=capacity(read,cell.grid)?;if cap>80 || cell.slot>=cap{return None;}
        let item=if let Some(item)=item_at(read,cell) {
            let tooltip=item.tooltip_source.as_ref()?;let user=tooltip.user_item.as_ref()?;
            if item.unique_id!=Some(user.unique_id) || item.quantity!=u32::from(user.count) || user.count==0
                || tooltip.info.item_index!=user.item_index || !seen.insert((rank(cell.grid),cell.slot)){return None;}
            if policy_cells.contains(&(rank(cell.grid),cell.slot)) {
                facts.policies.push(HeroCellPolicy{grid:cell.grid,slot:cell.slot,base:base_policy(&tooltip.info),
                    effective:effective_policy(tooltip.real_info.as_ref().unwrap_or(&tooltip.info))});
            }
            Some(HeroBasisItem{uid:user.unique_id.to_string(),item_index:user.item_index,count:item.quantity})
        }else{None};
        cells.push(HeroBasisCell{grid:cell.grid,slot:cell.slot,capacity:cap,item});
    }
    let basis=HeroActionBasis{version:1,actor:HeroBasisActor{object_id:hero.object_id,name:hero.name.clone(),
        class:format!("{:?}",hero.class),gender:format!("{:?}",hero.gender),spawned:read.hero.spawned},windows,cells,facts,
        resolved_wire:wire(&packet)?,confirmation_required,cross_player};
    (serde_json::to_vec(&basis).ok()?.len()<=MAX_ACTION_BASIS_BYTES).then_some(basis)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{hero_model::HeroModel,inventory::{CrystalItemTooltipSourceModel,CrystalUserItemModel,InventoryModel}};
    use mir2_protocol::{HeroUserInformation,MirClass,MirGender};
    pub(crate) fn read_model()->HeroUiReadModel {
        HeroUiReadModel{hero:HeroModel{spawned:true,inventory_view:InventoryModel{capacity:10,..Default::default()},
            info:Some(HeroUserInformation{object_id:12,name:"Hero".into(),class:MirClass::Warrior,gender:MirGender::Male,
                level:2,hair:0,hp:20,mp:5,experience:0,max_experience:100,inventory:Some(vec![None;10]),equipment:Some(vec![None;14]),
                magics:vec![],auto_pot:true,auto_hp_percent:30,auto_mp_percent:30,hp_item_index:0,mp_item_index:0}),..Default::default()},
            personal:Some(InventoryModel::default()),..Default::default()}
    }
    pub(crate) fn add(read:&mut HeroUiReadModel,which:HeroGrid,slot:u32,uid:u64,index:i32,count:u16,kind:u8) {
        let info=CrystalItemInfoModel{item_index:index,item_type:kind,stack_size:20,..Default::default()};
        let user=CrystalUserItemModel{unique_id:uid,item_index:index,count,..Default::default()};
        let item=ItemModel{container:if which==HeroGrid::HeroEquipment{2}else{0},slot,unique_id:Some(uid),quantity:u32::from(count),
            tooltip_source:Some(CrystalItemTooltipSourceModel{info,user_item:Some(user.clone()),..Default::default()}),..Default::default()};
        if which==HeroGrid::Inventory{read.personal.as_mut().unwrap().items.push(item);}
        else{read.hero.inventory_view.items.push(item);let raw=serde_json::from_value(serde_json::to_value(user).unwrap()).unwrap();
            let hero=read.hero.info.as_mut().unwrap();if which==HeroGrid::HeroEquipment{hero.equipment.as_mut().unwrap()[slot as usize]=Some(raw);}
            else{hero.inventory.as_mut().unwrap()[slot as usize]=Some(raw);}}
    }
    #[test]
    fn hero_basis_automatic_remove_keeps_null_semantic_and_exact_first_empty_raw_slot() {
        let mut read=read_model();add(&mut read,HeroGrid::HeroEquipment,1,u64::MAX,3,1,2);
        add(&mut read,HeroGrid::HeroInventory,2,8,8,1,13);
        let action=HeroSemanticAction::Remove{from:1,to:None};
        let basis=capture_action_basis(&read,HeroWindows::default(),&action,None).unwrap();
        assert_eq!(serde_json::to_value(&action).unwrap()["to"],Value::Null);
        assert_eq!(basis.resolved_wire,json!({"type":"removeItem","grid":"HeroInventory","uniqueId":u64::MAX.to_string(),"to":3}));
        assert_eq!(basis.facts.target,HeroBasisTarget{mode:"automatic",slot:Some(3)});
        assert_eq!(basis.cells.len(),11);
        let changed=capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Remove{from:1,to:Some(4)},None).unwrap();
        assert_eq!(changed.facts.target.mode,"explicit");assert_eq!(changed.resolved_wire["to"],4);
        add(&mut read,HeroGrid::HeroInventory,3,77,1,1,13);
        assert!(capture_action_basis(&read,HeroWindows::default(),&action,None).is_some_and(|basis|basis.resolved_wire["to"]==4));
    }
    #[test]
    fn hero_basis_use_keeps_actual_effective_auto_merge_and_base_explicit_merge_policy() {
        let mut read=read_model();add(&mut read,HeroGrid::HeroInventory,2,u64::MAX,1,2,8);
        add(&mut read,HeroGrid::HeroEquipment,9,9,2,4,8);
        let source=&mut read.hero.inventory_view.items[0];let mut real=source.tooltip_source.as_ref().unwrap().info.clone();
        real.item_index=2;real.stack_size=10;source.tooltip_source.as_mut().unwrap().real_info=Some(real);
        let basis=capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Use{slot:2,confirmed:false},None).unwrap();
        assert_eq!(basis.resolved_wire,json!({"type":"mergeItem","gridFrom":"HeroInventory","gridTo":"HeroEquipment","idFrom":u64::MAX.to_string(),"idTo":"9"}));
        assert_eq!(basis.facts.policies[0].base.item_index,1);assert_eq!(basis.facts.policies[0].effective.item_index,2);
        assert!(capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Merge{from:cell(HeroGrid::HeroInventory,2),to:cell(HeroGrid::HeroEquipment,9)},None).is_none());
        read=read_model();add(&mut read,HeroGrid::HeroInventory,2,7,1,1,6);add(&mut read,HeroGrid::HeroEquipment,6,8,2,1,8);
        let a=capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Use{slot:2,confirmed:false},None).unwrap();
        assert_eq!(a.resolved_wire["to"],6);assert!(a.facts.policies.iter().any(|p|p.grid==HeroGrid::HeroEquipment&&p.slot==6));
        read.hero.inventory_view.items[1].tooltip_source.as_mut().unwrap().info.item_type=6;
        let b=capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Use{slot:2,confirmed:false},None).unwrap();
        assert_eq!(b.resolved_wire["to"],5);
        assert!(b.cells.iter().any(|c|c.grid==HeroGrid::HeroEquipment&&c.slot==5&&c.item.is_none()));
        assert!(b.facts.policies.iter().any(|p|p.grid==HeroGrid::HeroEquipment&&p.slot==6));
        read=read_model();add(&mut read,HeroGrid::HeroInventory,2,7,1,1,7);add(&mut read,HeroGrid::HeroEquipment,8,8,2,1,7);
        let a=capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Use{slot:2,confirmed:false},None).unwrap();
        assert_eq!(a.resolved_wire["to"],7);assert!(a.facts.policies.iter().any(|p|p.grid==HeroGrid::HeroEquipment&&p.slot==8));
        read.hero.inventory_view.items.pop();read.hero.info.as_mut().unwrap().equipment.as_mut().unwrap()[8]=None;
        let b=capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Use{slot:2,confirmed:false},None).unwrap();
        assert_eq!(b.resolved_wire["to"],8);
        assert!(b.cells.iter().any(|c|c.grid==HeroGrid::HeroEquipment&&c.slot==8&&c.item.is_none()));
    }
    #[test]
    fn hero_basis_confirmation_restock_and_key_facts_are_exact_without_request_allocator() {
        let mut read=read_model();add(&mut read,HeroGrid::HeroInventory,0,7,1,1,13);add(&mut read,HeroGrid::HeroInventory,3,8,1,2,13);
        read.hero.inventory_view.items[0].tooltip_source.as_mut().unwrap().info.shape=4;
        let a=capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Use{slot:0,confirmed:false},None).unwrap();
        let b=capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Use{slot:0,confirmed:true},None).unwrap();
        assert!(a.confirmation_required);assert!(!b.confirmation_required);assert_eq!(a.resolved_wire,b.resolved_wire);
        assert_eq!(b.facts.restock,Some(HeroBasisRestock{belt:0,from:3,uid:"8".into(),item_index:1}));
        let magic=serde_json::from_value(json!({"name":"Fire Ball","spell":"FireBall","base_cost":1,"level_cost":0,"icon":1,"level1":1,"level2":2,"level3":3,"need1":1,"need2":2,"need3":3,"level":1,"key":17,"experience":0,"delay":100,"range":8,"cast_time":0})).unwrap();
        read.hero.info.as_mut().unwrap().magics.push(magic);
        let action=HeroSemanticAction::MagicKey{spell:"FireBall".into(),key:18};
        assert!(capture_action_basis(&read,HeroWindows::default(),&action,Some(0)).is_none());
        let basis=capture_action_basis(&read,HeroWindows::default(),&action,Some(17)).unwrap();
        assert_eq!(basis.resolved_wire,json!({"type":"magicKey","spell":"FireBall","key":18,"oldKey":17}));
        assert!(basis.resolved_wire.get("requestId").is_none());assert_eq!(basis.facts.keys[0].key,17);
    }
    #[test]
    fn hero_basis_transfer_uses_original_cross_planner_and_never_grants_anonymous_target() {
        let mut read=read_model();add(&mut read,HeroGrid::Inventory,0,7,1,2,13);
        let action=HeroSemanticAction::Transfer{from:0,to:2};
        let basis=capture_action_basis(&read,HeroWindows::default(),&action,None).unwrap();
        assert_eq!(basis.resolved_wire,json!({"type":"transferHeroItem","from":0,"to":2}));assert!(basis.cross_player);
        add(&mut read,HeroGrid::HeroInventory,2,8,1,3,13);
        assert_eq!(capture_action_basis(&read,HeroWindows::default(),&action,None).unwrap().resolved_wire["type"],"mergeItem");
        read.hero.inventory_view.items[0].unique_id=Some(0);read.hero.inventory_view.items[0].tooltip_source.as_mut().unwrap().user_item.as_mut().unwrap().unique_id=0;
        read.hero.info.as_mut().unwrap().inventory.as_mut().unwrap()[2].as_mut().unwrap().unique_id=0;
        assert!(capture_action_basis(&read,HeroWindows::default(),&action,None).is_none());
    }
    #[test]
    fn hero_basis_missing_metadata_duplicate_keys_and_oversize_fail_closed() {
        let mut read=read_model();add(&mut read,HeroGrid::HeroInventory,2,7,1,1,13);
        let action=HeroSemanticAction::Use{slot:2,confirmed:false};
        assert!(capture_action_basis(&read,HeroWindows::default(),&action,None).is_some());
        read.hero.inventory_view.items[0].tooltip_source.as_mut().unwrap().user_item=None;
        assert!(capture_action_basis(&read,HeroWindows::default(),&action,None).is_none());
        read=read_model();let magic:mir2_protocol::ClientMagic=serde_json::from_value(json!({"name":"Fire Ball","spell":"FireBall","base_cost":1,"level_cost":0,"icon":1,"level1":1,"level2":2,"level3":3,"need1":1,"need2":2,"need3":3,"level":1,"key":17,"experience":0,"delay":100,"range":8,"cast_time":0})).unwrap();
        read.hero.info.as_mut().unwrap().magics=vec![magic.clone(),magic];
        assert!(capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::MagicKey{spell:"FireBall".into(),key:18},Some(17)).is_none());
        read=read_model();read.hero.inventory_view.capacity=42;
        read.hero.info.as_mut().unwrap().inventory=Some(vec![None;42]);
        for slot in 0..42{add(&mut read,HeroGrid::HeroInventory,slot,u64::MAX-u64::from(slot),i32::MAX,1,13);}
        assert!(item_use::plan(&read.hero,0)!=item_use::HeroUsePlan::Unavailable);
        let basis=capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Use{slot:0,confirmed:false},None).unwrap();
        assert_eq!(basis.cells.len(),42);assert_eq!(basis.facts.policies.len(),1);
        assert_eq!(basis.facts.policies[0].slot,0);
        assert!(serde_json::to_vec(&basis).unwrap().len()<=MAX_ACTION_BASIS_BYTES);
        assert_eq!(basis.cells[0].item.as_ref().unwrap().uid,u64::MAX.to_string());
        assert_eq!(basis.facts.restock.as_ref().unwrap().item_index,i32::MAX);
        read.hero.inventory_view.items.pop();read.hero.info.as_mut().unwrap().inventory.as_mut().unwrap()[41]=None;
        add(&mut read,HeroGrid::HeroEquipment,1,7,1,1,2);
        let remove=capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Remove{from:1,to:None},None).unwrap();
        assert_eq!(remove.resolved_wire["to"],41);assert_eq!(remove.cells.len(),43);assert_eq!(remove.facts.policies.len(),1);
        assert_eq!(remove.facts.policies[0].grid,HeroGrid::HeroEquipment);
        assert!(serde_json::to_vec(&remove).unwrap().len()<=MAX_ACTION_BASIS_BYTES);
        read.hero.info.as_mut().unwrap().name="H".repeat(MAX_ACTION_BASIS_BYTES);
        assert!(capture_action_basis(&read,HeroWindows::default(),&HeroSemanticAction::Use{slot:0,confirmed:false},None).is_none());
    }
}
