//! Hero buffs are independent from the owner's BuffResource. HeroInfo.Save
//! does not persist Buffs; only the live Hero instance retains them on recall.
use super::super::{buffs::RealTimeBuffDuration,components::{hero_entity,ObjectId,PlayerVitals},resources::Stage5SystemsResource};
use bevy_ecs::prelude::{Resource,World};
use mir2_protocol::{ClientBuff,ServerPacket,UserItemStat};
#[derive(Clone)]
struct Entry { wire:ClientBuff, duration:RealTimeBuffDuration }
#[derive(Resource,Default,Clone)]
struct HeroBuffs { identity:String, entries:Vec<Entry> }
fn identity(world:&World)->String {
    world.resource::<Stage5SystemsResource>().stage5_systems.hero.as_ref()
        .map(|hero|format!("{}:{:?}:{:?}",hero.name,hero.class,hero.gender)).unwrap_or_default()
}
fn definition(kind:u8)->Option<&'static mir2_game_data::CrystalBuffTemplate> {
    static DEFINITIONS:std::sync::OnceLock<std::collections::BTreeMap<u8,mir2_game_data::CrystalBuffTemplate>>=std::sync::OnceLock::new();
    DEFINITIONS.get_or_init(|| {
        let source=mir2_game_data::crystal_buff_manifest();
        (0..=u8::MAX).filter_map(|kind| {
            let key=super::super::buffs::crystal_buff_key_for_type(kind)?;
            let normalized=if key=="battle-focus" {"fury".to_owned()} else {key.replace('-',"")};
            source.buffs.iter().find(|info|info.buff_type.to_ascii_lowercase()==normalized).cloned().map(|info|(kind,info))
        }).collect()
    }).get(&kind)
}
pub(in crate::runtime) fn reset(world:&mut World) {world.remove_resource::<HeroBuffs>();}
pub(in crate::runtime) fn active(world:&World)->Vec<ClientBuff> {
    let key=identity(world);
    let Some(state)=world.get_resource::<HeroBuffs>().filter(|state|state.identity==key) else{return vec![];};
    let id=hero_entity(world).and_then(|entity|world.get::<ObjectId>(entity)).map(|id|id.0);
    let dead=hero_entity(world).and_then(|entity|world.get::<PlayerVitals>(entity)).is_some_and(|v|v.hp<=0);
    state.entries.iter().filter_map(|entry| {
        if dead && (matches!(entry.wire.buff_type,24|25)||definition(entry.wire.buff_type).is_some_and(|info|info.properties.iter().any(|p|p=="RemoveOnDeath"))) {return None;}
        let remaining=entry.duration.remaining_ms();
        if !entry.wire.infinite&&remaining==0 {return None;}
        let mut buff=entry.wire.clone();buff.object_id=id.unwrap_or(0);
        buff.expire_time=remaining.min(i64::MAX as u64) as i64;Some(buff)
    }).collect()
}
pub(in crate::runtime) fn stats(world:&World)->Vec<UserItemStat> {active(world).into_iter().flat_map(|buff|buff.stats).collect()}
pub(in crate::runtime) fn has(world:&World,buff_type:u8)->bool {active(world).iter().any(|buff|buff.buff_type==buff_type)}
pub(in crate::runtime) fn capture(world:&mut World,packets:&[ServerPacket]) {
    let Some(id)=hero_entity(world).and_then(|entity|world.get::<ObjectId>(entity)).map(|id|id.0) else{return;};
    let key=identity(world);
    if world.get_resource::<HeroBuffs>().is_none_or(|state|state.identity!=key) {world.insert_resource(HeroBuffs{identity:key,..Default::default()});}
    let mut state=world.resource_mut::<HeroBuffs>();
    for packet in packets {
        match packet {
            ServerPacket::AddBuff{buff} if buff.object_id==id => {
                let duration=RealTimeBuffDuration::new(buff.expire_time.max(0) as u64);
                if let Some(entry)=state.entries.iter_mut().find(|entry|entry.wire.buff_type==buff.buff_type) {
                    match definition(buff.buff_type).map(|info|info.stack_type.as_str()) {
                        Some("StackDuration")=>entry.duration=RealTimeBuffDuration::new(entry.duration.remaining_ms().saturating_add(buff.expire_time.max(0) as u64)),
                        Some("ResetStatAndDuration")=>{entry.wire=buff.clone();entry.duration=duration;},
                        Some("None"|"Infinite")=>{},
                        _=>entry.duration=duration,
                    }
                } else {state.entries.push(Entry{wire:buff.clone(),duration});}
            }
            ServerPacket::RemoveBuff{object_id,buff_type} if *object_id==id =>state.entries.retain(|entry|entry.wire.buff_type!=*buff_type),
            _=>{}
        }
    }
}
pub(in crate::runtime) fn tick(world:&mut World,packets:&mut Vec<ServerPacket>) {
    let id=hero_entity(world).and_then(|entity|world.get::<ObjectId>(entity)).map(|id|id.0).unwrap_or(0);
    let dead=hero_entity(world).and_then(|entity|world.get::<PlayerVitals>(entity)).is_some_and(|v|v.hp<=0);
    let key=identity(world);
    let Some(mut state)=world.get_resource_mut::<HeroBuffs>() else{return;};
    if state.identity!=key {state.entries.clear();state.identity=key;return;}
    state.entries.retain(|entry| {
        let expired=!entry.wire.infinite&&entry.duration.remaining_ms()==0;
        // HeroObject.Die explicitly removes these two shields.
        let properties=definition(entry.wire.buff_type).map(|info|info.properties.as_slice()).unwrap_or_default();
        let remove=expired||(dead&&(matches!(entry.wire.buff_type,24|25)||properties.iter().any(|p|p=="RemoveOnDeath")))
            ||(id==0&&properties.iter().any(|p|p=="RemoveOnExit"));
        if remove&&id!=0 {packets.push(ServerPacket::RemoveBuff{object_id:id,buff_type:entry.wire.buff_type});}
        !remove
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::{ClientPacket,MirClass,MirGender};
    fn scene()->(crate::SimulationSession,u32) {
        let mut s=crate::SimulationSession::new(crate::SimulationConfig::default());
        s.handle_packet(ClientPacket::Login{account_id:"demo".into(),password:"demo".into()});
        s.handle_packet(ClientPacket::StartGame{character_index:0});
        s.handle_packet(ClientPacket::NewHero{name:"Aide".into(),gender:MirGender::Female,class:MirClass::Wizard});
        let id=s.app.world().get::<ObjectId>(hero_entity(s.app.world()).unwrap()).unwrap().0;
        (s,id)
    }
    fn buff(id:u32,kind:u8,value:i32)->ServerPacket {
        ServerPacket::AddBuff{buff:ClientBuff{buff_type:kind,visible:true,object_id:id,expire_time:60_000,infinite:false,paused:false,stats:vec![UserItemStat{stat:6,value},UserItemStat{stat:7,value}],values:vec![]}}
    }
    #[test]
    fn hero_buffs_are_owner_isolated_real_time_and_reset_duration_keeps_original_stats() {
        let (mut s,id)=scene();
        capture(s.app.world_mut(),&[buff(id+10,21,100)]);assert!(stats(s.app.world()).is_empty());
        capture(s.app.world_mut(),&[buff(id,21,6)]);
        let base=super::super::super::hero_stats::compute_with_status(s.app.world(),false,&[]).get(6);
        assert_eq!(super::super::super::hero_stats::compute(s.app.world()).get(6),base+6);
        for _ in 0..1000 {tick(s.app.world_mut(),&mut vec![]);}
        assert!(has(s.app.world(),21));
        capture(s.app.world_mut(),&[buff(id,21,100)]);assert_eq!(stats(s.app.world())[0].value,6);
        s.app.world_mut().resource_mut::<HeroBuffs>().entries[0].duration.elapse_for_test(60_001);
        assert!(!has(s.app.world(),21));
        assert_eq!(super::super::super::hero_stats::compute(s.app.world()).get(6),base);
        let mut packets=vec![];tick(s.app.world_mut(),&mut packets);
        assert!(packets.contains(&ServerPacket::RemoveBuff{object_id:id,buff_type:21}));
    }
    #[test]
    fn hero_buffs_death_removes_shields_and_relogin_reset_does_not_copy_owner_buffs() {
        let (mut s,id)=scene();capture(s.app.world_mut(),&[buff(id,24,20),buff(id,21,6)]);
        let hero=hero_entity(s.app.world()).unwrap();s.app.world_mut().get_mut::<PlayerVitals>(hero).unwrap().hp=0;
        assert!(!has(s.app.world(),24));assert!(has(s.app.world(),21));
        tick(s.app.world_mut(),&mut vec![]);assert_eq!(s.app.world().resource::<HeroBuffs>().entries.len(),1);
        reset(s.app.world_mut());assert!(stats(s.app.world()).is_empty());
    }
}

#[derive(Clone)]
pub(super) struct TransientSnapshot(Option<HeroBuffs>);
pub(super) fn capture_transient(world:&World)->TransientSnapshot {TransientSnapshot(world.get_resource::<HeroBuffs>().cloned())}
pub(super) fn restore_transient(world:&mut World,snapshot:&TransientSnapshot){world.remove_resource::<HeroBuffs>();if let Some(state)=&snapshot.0 {world.insert_resource(state.clone());}}
