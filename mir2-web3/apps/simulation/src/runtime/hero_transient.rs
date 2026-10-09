//! Live-only rollback state. Never serialize Hero buffs or monotonic clocks into HeroInfo.
use bevy_ecs::prelude::World;
use super::{hero_buffs,hero_cast,hero_mount,hero_cadence};
use super::super::resources::{SessionResource,Stage5SystemsResource};
#[derive(Clone)]
pub(crate) struct HeroTransientCheckpoint {
    owner:Option<(String,i32)>,hero:String,
    buffs:hero_buffs::TransientSnapshot,cast:hero_cast::TransientSnapshot,
    mount:hero_mount::TransientSnapshot,cadence:hero_cadence::TransientSnapshot,
}
impl std::fmt::Debug for HeroTransientCheckpoint {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {f.debug_struct("HeroTransientCheckpoint").finish_non_exhaustive()}
}
fn owner(world:&World)->Option<(String,i32)> {let s=world.resource::<SessionResource>();Some((s.account_id.clone()?,s.selected_character.as_ref()?.index))}
fn hero(world:&World)->String {world.resource::<Stage5SystemsResource>().stage5_systems.hero.as_ref().map(|h|format!("{}:{:?}:{:?}",h.name,h.class,h.gender)).unwrap_or_default()}
pub(in crate::runtime) fn capture(world:&World)->HeroTransientCheckpoint {
    HeroTransientCheckpoint{owner:owner(world),hero:hero(world),buffs:hero_buffs::capture_transient(world),cast:hero_cast::capture_transient(world),mount:hero_mount::capture_transient(world),cadence:hero_cadence::capture_transient(world)}
}
pub(in crate::runtime) fn restore(world:&mut World,snapshot:&HeroTransientCheckpoint)->Result<(),String>{
    if snapshot.owner.is_none()||owner(world)!=snapshot.owner||hero(world)!=snapshot.hero{return Err("Hero transient checkpoint identity mismatch".into());}
    hero_buffs::restore_transient(world,&snapshot.buffs);hero_cast::restore_transient(world,&snapshot.cast);
    hero_mount::restore_transient(world,&snapshot.mount);hero_cadence::restore_transient(world,&snapshot.cadence);Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::{ClientPacket,MirClass,MirGender,ServerPacket,ClientBuff,UserItemStat};
    #[test]
    fn hero_transient_rollback_retains_buff_stats_and_original_remaining_duration(){
        let mut s=crate::SimulationSession::new(crate::SimulationConfig::default());
        s.handle_packet(ClientPacket::Login{account_id:"demo".into(),password:"demo".into()});
        s.handle_packet(ClientPacket::StartGame{character_index:0});
        s.handle_packet(ClientPacket::NewHero{name:"Aide".into(),gender:MirGender::Female,class:MirClass::Wizard});
        let w=s.app.world_mut();
        hero_buffs::capture(w,&[ServerPacket::AddBuff{buff:ClientBuff{buff_type:21,object_id:1001,expire_time:60000,infinite:false,visible:true,paused:false,stats:vec![UserItemStat{stat:4,value:6}],values:vec![]}}]);
        hero_cadence::cast(w);
        let before=hero_buffs::active(w)[0].expire_time;let checkpoint=capture(w);
        hero_buffs::reset(w);hero_cadence::reset(w);
        restore(w,&checkpoint).unwrap();
        let after=hero_buffs::active(w);assert_eq!(after[0].stats[0].value,6);
        assert!(after[0].expire_time<=before);assert!(after[0].expire_time>59000);
        assert!(!hero_cadence::action_ready(w));
        w.resource_mut::<Stage5SystemsResource>().stage5_systems.hero.as_mut().unwrap().name="Other".into();
        assert!(restore(w,&checkpoint).is_err());
    }
}
