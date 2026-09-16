//! Atomic, exact-instance merging across Hero and personal inventory grids.
use super::super::{crystal_compat::*, equipment::*, inventory::*, items::*, resources::*};
use super::{ready, validate_hero_custody, refresh_stats_and_packets};
use bevy_ecs::world::World;
use crate::ItemContainer;
use mir2_protocol::{MirGridType, ServerPacket};

fn is_hero(grid: MirGridType) -> bool { matches!(grid, MirGridType::HeroInventory | MirGridType::HeroEquipment) }

// Equipment and rod sockets are projected to full carriers before staging.
// Rebuilding a changed carrier retains every field, including nested UIDs.
fn collection(inv: &InventoryResource, hero: &HeroInventoryResource, grid: MirGridType) -> Option<Vec<ItemState>> {
    Some(match grid {
        MirGridType::HeroInventory => hero.items.clone(),
        MirGridType::HeroEquipment => hero.equipment.clone(),
        MirGridType::Inventory => inv.inventory_items.clone(),
        MirGridType::Belt => inv.belt_items.clone(),
        MirGridType::Storage => inv.storage_items.clone(),
        MirGridType::Equipment => inv.equipment_items.iter().map(|item| {
            Some(item_state_from_equipment_state(item.clone(), ItemContainer::Bag1, equipment_slot_index(item.slot)? as u8))
        }).collect::<Option<Vec<_>>>()?,
        MirGridType::Fishing => {
            let weapon = inv.equipment_items.iter().find(|item| item.slot == crate::EquipmentSlot::Weapon)?;
            let host = item_state_from_equipment_state(weapon.clone(), ItemContainer::Bag1, 0);
            let base = crystal_item_template_for_item_key(&host.key)?;
            if !matches!(base.shape, 49 | 50) { return None; }
            let wire = try_user_item_from_item_state(&host).ok()?;
            wire.slots.iter().enumerate().filter_map(|(slot,item)| item.as_ref().map(|item|(slot,item))).map(|(slot,item)| {
                let base = mir2_game_data::crystal_item_by_index(item.item_index)?;
                let state = embedded_item_state_from_template(&base, ItemContainer::Bag1, u8::try_from(slot).ok()?);
                try_item_state_from_user_item(state,item).ok()
            }).collect::<Option<Vec<_>>>()?
        }
        _ => return None,
    })
}
fn replace(inv: &mut InventoryResource, hero: &mut HeroInventoryResource, grid: MirGridType, items: Vec<ItemState>) -> Option<()> {
    match grid {
        MirGridType::HeroInventory => hero.items=items,
        MirGridType::HeroEquipment => hero.equipment=items,
        MirGridType::Inventory => inv.inventory_items=items,
        MirGridType::Belt => inv.belt_items=items,
        MirGridType::Storage => inv.storage_items=items,
        MirGridType::Equipment => {
            let weapon = inv.equipment_items.iter().find(|item| item.slot == crate::EquipmentSlot::Weapon).cloned();
            inv.equipment_items=items.iter().map(|item| {
                let slot=equipment_slot_from_index(i32::from(item.slot))?;
                if slot == crate::EquipmentSlot::Weapon { weapon.clone() }
                else { Some(equipment_state_from_item_state(item,slot)) }
            }).collect::<Option<Vec<_>>>()?;
        },
        MirGridType::Fishing => {
            let weapon=inv.equipment_items.iter_mut().find(|item| item.slot==crate::EquipmentSlot::Weapon)?;
            let host=item_state_from_equipment_state(weapon.clone(),ItemContainer::Bag1,0);
            let mut wire=try_user_item_from_item_state(&host).ok()?;
            wire.slots.fill(None);
            for item in items {
                *wire.slots.get_mut(usize::from(item.slot))?=Some(try_user_item_from_item_state(&item).ok()?);
            }
            let next=try_item_state_from_user_item(host,&wire).ok()?;
            *weapon=equipment_state_from_item_state(&next,crate::EquipmentSlot::Weapon);
        }
        _ => return None,
    }
    Some(())
}

pub(in crate::runtime) fn merge_item(world: &mut World, from: MirGridType, to: MirGridType, source: u64, target: u64) -> Vec<ServerPacket> {
    let ack=|success| ServerPacket::MergeItem{grid_from:from,grid_to:to,id_from:source,id_to:target,success};
    if source==0 || target==0 || source==target || !ready(world,false) { return vec![ack(false)]; }
    if matches!(from,MirGridType::Storage)||matches!(to,MirGridType::Storage) {
        if !super::super::npc::active_crystal_storage_service(world)||storage_locked(world) {return vec![ack(false)];}
    }
    if super::super::item_custody::refresh(world).is_err() {return vec![ack(false)];}
    let mut inv=world.resource::<InventoryResource>().clone();
    let mut hero=world.resource::<HeroInventoryResource>().clone();
    let Some(mut a)=collection(&inv,&hero,from) else{return vec![ack(false)];};
    let Some(mut b)=collection(&inv,&hero,to) else{return vec![ack(false)];};
    let Some(ai)=item_index_for_client_reference(&a,from,source) else{return vec![ack(false)];};
    let Some(bi)=item_index_for_client_reference(&b,to,target) else{return vec![ack(false)];};
    let left=&a[ai]; let right=&b[bi];
    for (grid,item) in [(from,left),(to,right)] {
        if grid==MirGridType::Storage && u16::from(item.slot)>=accessible_storage_size(&inv) {return vec![ack(false)];}
        if matches!(grid,MirGridType::Inventory|MirGridType::Belt) && super::super::packets::stage5_trade_reserves_item(world,item) {return vec![ack(false)];}
        if grid==MirGridType::Equipment && crystal_item_template_for_item_key(&item.key).is_none_or(|info|info.item_type!=CRYSTAL_ITEM_TYPE_AMULET) {return vec![ack(false)];}
        if grid==MirGridType::Fishing && crystal_item_template_for_item_key(&item.key).is_none_or(|info|info.item_type!=CRYSTAL_ITEM_TYPE_BAIT) {return vec![ack(false)];}
    }
    if is_hero(to)&&!is_hero(from)&&item_has_crystal_or_rental_bind_flag(left,CRYSTAL_BIND_NO_HERO) {return vec![ack(false)];}
    if to==MirGridType::Storage&&item_has_crystal_or_rental_bind_flag(left,CRYSTAL_BIND_DONT_STORE) {return vec![ack(false)];}
    if !item_stack_identity_compatible(left,right)||left.quantity==0 {return vec![ack(false)];}
    let max=crystal_stack_size_for_item_key(&right.key);
    if max<=1||right.quantity>=max {return vec![ack(false)];}
    let moved=left.quantity.min(max-right.quantity);
    b[bi].quantity+=moved;
    a[ai].quantity-=moved;
    if from==to {
        a[bi].quantity=b[bi].quantity;
        if a[ai].quantity==0 {a.remove(ai);}
        if replace(&mut inv,&mut hero,from,a).is_none(){return vec![ack(false)];}
    } else {
        if a[ai].quantity==0 {a.remove(ai);}
        if replace(&mut inv,&mut hero,from,a).is_none()||replace(&mut inv,&mut hero,to,b).is_none(){return vec![ack(false)];}
    }
    let Ok(hero_ids)=validate_hero_custody(&hero) else{return vec![ack(false)];};
    let Ok(mut reserved)=super::super::item_custody::reserved_ids(&world.resource::<Stage5SystemsResource>().stage5_systems) else{return vec![ack(false)];};
    if hero_ids.iter().any(|id|reserved.contains(id)){return vec![ack(false)];}
    reserved.extend(hero_ids);
    // Personal legacy roots retain grid-local fallback IDs. This operation
    // changes counts only; do not migrate those unrelated roots here. Every
    // retained Hero/escrow UID must nevertheless remain absent from every
    // personal carrier, including nested sockets.
    for grid in [MirGridType::Inventory,MirGridType::Belt,MirGridType::Storage,MirGridType::Equipment] {
        let Some(items)=collection(&inv,&hero,grid) else{return vec![ack(false)];};
        for item in items {
            let Ok(ids)=super::super::item_custody::item_ids(&item) else{return vec![ack(false)];};
            if ids.iter().any(|id|reserved.contains(id)) {return vec![ack(false)];}
        }
    }
    inv.reserved_item_unique_ids=reserved;
    *world.resource_mut::<InventoryResource>()=inv;
    *world.resource_mut::<HeroInventoryResource>()=hero;
    super::super::stats::refresh_player_stats(world);
    let mut packets=vec![ack(true)];
    packets.extend(refresh_stats_and_packets(world));
    packets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SimulationConfig,SimulationSession};
    use mir2_protocol::{ClientPacket,MirClass,MirGender};
    fn session() -> SimulationSession {
        let mut s=SimulationSession::new(SimulationConfig::default());
        s.handle_packet(ClientPacket::Login{account_id:"demo".into(),password:"demo".into()});
        s.handle_packet(ClientPacket::StartGame{character_index:0});
        s.handle_packet(ClientPacket::NewHero{name:"Aide".into(),gender:MirGender::Female,class:MirClass::Taoist});
        s
    }
    fn stack(uid:u64,slot:u8,count:u32) -> ItemState {
        let info=mir2_game_data::crystal_item_manifest().items.into_iter().find(|item|item.item_type==CRYSTAL_ITEM_TYPE_POTION&&item.shape==0&&item.stack_size>5).unwrap();
        let mut item=embedded_item_state_from_template(&info,ItemContainer::Bag1,slot);
        item.unique_id=uid;item.quantity=count;item
    }
    fn request(s:&mut SimulationSession,from:MirGridType,to:MirGridType)->Vec<ServerPacket> {
        s.handle_packet(ClientPacket::MergeItem{grid_from:from,grid_to:to,id_from:88001,id_to:88002})
    }
    fn passed(p:&[ServerPacket])->bool { p.iter().any(|p|matches!(p,ServerPacket::MergeItem{success:true,..})) }
    #[test]
    fn hero_custody_merge_cross_owner_grid_retains_target_uid_and_releases_consumed_uid() {
        let mut s=session();
        s.app.world_mut().resource_mut::<InventoryResource>().inventory_items=vec![stack(88001,2,2)];
        s.app.world_mut().resource_mut::<HeroInventoryResource>().items=vec![stack(88002,0,3)];
        let packets=request(&mut s,MirGridType::Inventory,MirGridType::HeroInventory);
        assert!(passed(&packets),"{packets:?}");
        let hero=s.app.world().resource::<HeroInventoryResource>();
        assert_eq!((hero.items[0].unique_id,hero.items[0].quantity,hero.items[0].slot),(88002,5,0));
        let inv=s.app.world().resource::<InventoryResource>();
        assert!(inv.inventory_items.is_empty());
        assert!(inv.reserved_item_unique_ids.contains(&88002));
        assert!(!inv.reserved_item_unique_ids.contains(&88001));
        assert!(packets.iter().any(|p|matches!(p,ServerPacket::HeroInformation{..})));
    }
    #[test]
    fn hero_custody_merge_partial_and_failure_preserve_both_exact_carriers() {
        let mut s=session();
        let a=stack(88001,0,5);let mut b=stack(88002,9,1);
        let limit=crystal_stack_size_for_item_key(&b.key);b.quantity=limit-2;
        s.app.world_mut().resource_mut::<HeroInventoryResource>().items=vec![a,b];
        assert!(passed(&request(&mut s,MirGridType::HeroInventory,MirGridType::HeroInventory)));
        let items=&s.app.world().resource::<HeroInventoryResource>().items;
        assert_eq!((items[0].unique_id,items[0].quantity,items[1].unique_id,items[1].quantity),(88001,3,88002,limit));
        let before=serde_json::to_value(items).unwrap();
        assert!(!passed(&request(&mut s,MirGridType::HeroInventory,MirGridType::HeroInventory)));
        assert_eq!(serde_json::to_value(&s.app.world().resource::<HeroInventoryResource>().items).unwrap(),before);
    }
    #[test]
    fn hero_custody_merge_rejects_instance_mismatch_and_unopened_storage_atomically() {
        let mut s=session();let a=stack(88001,0,2);let mut b=stack(88002,1,2);
        b.added_stats.push(mir2_protocol::UserItemStat{stat:12,value:1});
        s.app.world_mut().resource_mut::<HeroInventoryResource>().items=vec![a,b];
        let before=serde_json::to_value(&s.app.world().resource::<HeroInventoryResource>().items).unwrap();
        assert!(!passed(&request(&mut s,MirGridType::HeroInventory,MirGridType::HeroInventory)));
        assert!(!passed(&request(&mut s,MirGridType::HeroInventory,MirGridType::Storage)));
        assert_eq!(serde_json::to_value(&s.app.world().resource::<HeroInventoryResource>().items).unwrap(),before);
    }
}
