//! Delayed Hero impacts are released by monotonic deadlines, never by client input counts.
use bevy_ecs::prelude::{Entity,Resource,World};
use std::time::{Duration,Instant};
use super::super::{combat::{PendingCombatAction,queue_pending_combat_action}, components::{hero_entity,ObjectId},resources::{runtime_tick,Stage5SystemsResource}};
#[derive(Clone)]
struct Pending {due:Instant,target:Option<(Entity,u32)>,action:PendingCombatAction}
#[derive(Resource,Default,Clone)]
struct HeroDelayedActions {identity:String,actor_id:u32,pending:Vec<Pending>}
fn identity(world:&World)->String {
    world.resource::<Stage5SystemsResource>().stage5_systems.hero.as_ref().map(|h|format!("{}:{:?}:{:?}",h.name,h.class,h.gender)).unwrap_or_default()
}
pub(in crate::runtime) fn reset(world:&mut World){world.remove_resource::<HeroDelayedActions>();}
pub(super) fn enqueue(world:&mut World,delay_ms:u64,target:Option<(Entity,u32)>,action:PendingCombatAction) {
    enqueue_at(world,Instant::now(),delay_ms,target,action);
}
fn enqueue_at(world:&mut World,now:Instant,delay_ms:u64,target:Option<(Entity,u32)>,action:PendingCombatAction) {
    let key=identity(world);let id=action.attacker_id;
    if world.get_resource::<HeroDelayedActions>().is_none_or(|s|s.identity!=key||s.actor_id!=id) {
        world.insert_resource(HeroDelayedActions{identity:key,actor_id:id,pending:vec![]});
    }
    world.resource_mut::<HeroDelayedActions>().pending.push(Pending{due:now+Duration::from_millis(delay_ms),target,action});
}
pub(super) fn flush(world:&mut World){flush_at(world,Instant::now());}
fn flush_at(world:&mut World,now:Instant){
    let key=identity(world);let actor=hero_entity(world).and_then(|e|world.get::<ObjectId>(e)).map(|id|id.0);
    let due={
        let Some(mut state)=world.get_resource_mut::<HeroDelayedActions>() else{return;};
        if state.identity!=key||Some(state.actor_id)!=actor {state.pending.clear();return;}
        let mut future=vec![];let mut due=vec![];
        for entry in std::mem::take(&mut state.pending){if now>=entry.due{due.push(entry)}else{future.push(entry)}}
        state.pending=future;due
    };
    let tick=runtime_tick(world);
    for mut entry in due{
        if entry.target.is_some_and(|(entity,id)|world.get::<ObjectId>(entity).is_none_or(|actual|actual.0!=id)){continue;}
        entry.action.due_tick=tick;queue_pending_combat_action(world,entry.action);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::{combat::{CrystalDefence,PendingCombatTarget},components::Monster,resources::RuntimeQueueResource};
    use mir2_protocol::{ClientPacket,MirClass,MirGender};
    fn scene()->(crate::SimulationSession,Entity,u32,u32){
        let mut s=crate::SimulationSession::new(crate::SimulationConfig::default());
        s.handle_packet(ClientPacket::Login{account_id:"demo".into(),password:"demo".into()});
        s.handle_packet(ClientPacket::StartGame{character_index:0});
        s.handle_packet(ClientPacket::NewHero{name:"Aide".into(),gender:MirGender::Female,class:MirClass::Wizard});
        let w=s.app.world();let hero=w.get::<ObjectId>(hero_entity(w).unwrap()).unwrap().0;
        let target=w.iter_entities().find(|e|e.get::<Monster>().is_some()).unwrap();let entity=target.id();let id=target.get::<ObjectId>().unwrap().0;
        (s,entity,id,hero)
    }
    fn action(entity:Entity,actor:u32)->PendingCombatAction{
        PendingCombatAction{due_tick:0,attacker_id:actor,target:PendingCombatTarget::Monster(entity),damage:10,player_status_effect:None,due_packet:None,player_movement:None,on_monster_defeat:None,defence:CrystalDefence::Mac,on_hit_poison:None}
    }
    #[test]
    fn hero_delayed_impacts_wait_real_deadline_then_enter_existing_combat_chain_once(){
        let (mut s,target,id,actor)=scene();let now=Instant::now();
        let count=s.app.world().resource::<RuntimeQueueResource>().pending_combat_actions.len();
        enqueue_at(s.app.world_mut(),now,300,Some((target,id)),action(target,actor));
        for _ in 0..1000{flush_at(s.app.world_mut(),now+Duration::from_millis(299));}
        assert_eq!(s.app.world().resource::<RuntimeQueueResource>().pending_combat_actions.len(),count);
        flush_at(s.app.world_mut(),now+Duration::from_millis(300));
        flush_at(s.app.world_mut(),now+Duration::from_millis(301));
        let q=&s.app.world().resource::<RuntimeQueueResource>().pending_combat_actions;
        assert_eq!(q.len(),count+1);assert_eq!(q.last().unwrap().defence,CrystalDefence::Mac);
        assert_eq!(q.last().unwrap().attacker_id,actor);
    }
    #[test]
    fn hero_delayed_stale_target_or_changed_hero_never_hits_reused_actor(){
        let (mut s,target,id,actor)=scene();let now=Instant::now();
        let count=s.app.world().resource::<RuntimeQueueResource>().pending_combat_actions.len();
        enqueue_at(s.app.world_mut(),now,300,Some((target,id+1)),action(target,actor));
        flush_at(s.app.world_mut(),now+Duration::from_secs(1));
        assert_eq!(s.app.world().resource::<RuntimeQueueResource>().pending_combat_actions.len(),count);
        enqueue_at(s.app.world_mut(),now,300,Some((target,id)),action(target,actor));
        s.app.world_mut().resource_mut::<Stage5SystemsResource>().stage5_systems.hero.as_mut().unwrap().name="Other".into();
        flush_at(s.app.world_mut(),now+Duration::from_secs(1));
        assert_eq!(s.app.world().resource::<RuntimeQueueResource>().pending_combat_actions.len(),count);
    }
}

#[derive(Clone)]
pub(super) struct TransientSnapshot(Option<HeroDelayedActions>);
pub(super) fn capture_transient(world:&World)->TransientSnapshot {TransientSnapshot(world.get_resource::<HeroDelayedActions>().cloned())}
pub(super) fn restore_transient(world:&mut World,snapshot:&TransientSnapshot){
    world.remove_resource::<HeroDelayedActions>();
    let Some(mut state)=snapshot.0.clone() else{return;};
    let objects:std::collections::BTreeMap<u32,Entity>=world.iter_entities().filter_map(|e|e.get::<ObjectId>().map(|id|(id.0,e.id()))).collect();
    state.pending.retain_mut(|entry|{
        if let Some((_,id))=entry.target {
            let Some(entity)=objects.get(&id).copied() else{return false;};
            entry.target=Some((entity,id));entry.action.target=super::super::combat::PendingCombatTarget::Monster(entity);
        }
        true
    });
    world.insert_resource(state);
}
