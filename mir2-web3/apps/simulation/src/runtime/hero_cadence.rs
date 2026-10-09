//! Hero action deadlines use elapsed real time, independently of request/tick counts.
use bevy_ecs::prelude::{Resource,World};
use std::time::{Duration,Instant};
use super::super::{components::{hero_entity,CharacterBody},resources::Stage5SystemsResource};
#[derive(Resource,Clone)]
struct HeroCadence {identity:String, action:Instant, attack:Instant}
fn identity(world:&World)->String {
    world.resource::<Stage5SystemsResource>().stage5_systems.hero.as_ref().map(|h|format!("{}:{:?}:{:?}",h.name,h.class,h.gender)).unwrap_or_default()
}
fn state(world:&World)->Option<&HeroCadence> {world.get_resource::<HeroCadence>().filter(|s|s.identity==identity(world))}
pub(in crate::runtime) fn reset(world:&mut World) {world.remove_resource::<HeroCadence>();}
pub(in crate::runtime) fn action_ready(world:&World)->bool {action_ready_at(world,Instant::now())}
fn action_ready_at(world:&World,now:Instant)->bool {state(world).is_none_or(|s|now>=s.action)}
pub(super) fn attack_ready(world:&World)->bool {let now=Instant::now();state(world).is_none_or(|s|now>=s.action&&now>=s.attack)}
fn set(world:&mut World,now:Instant,action_ms:u64,attack_ms:Option<u64>) {
    let key=identity(world);
    if state(world).is_none(){world.insert_resource(HeroCadence{identity:key,action:now,attack:now});}
    let mut s=world.resource_mut::<HeroCadence>();s.action=now+Duration::from_millis(action_ms);
    if let Some(ms)=attack_ms{s.attack=now+Duration::from_millis(ms);}
}
pub(super) fn moved(world:&mut World) {set(world,Instant::now(),600,None);}
pub(in crate::runtime) fn cast(world:&mut World) {set(world,Instant::now(),600,Some(600));}
pub(super) fn attacked(world:&mut World) {
    let level=hero_entity(world).and_then(|e|world.get::<CharacterBody>(e)).map(|b|b.level).unwrap_or(1);
    let speed=super::super::hero_stats::compute(world).get(14);
    set(world,Instant::now(),550,Some(attack_delay_ms(level,speed)));
}
fn attack_delay_ms(level:u16,speed:i32)->u64 {
    (1400_i64-(i64::from(speed)*60+i64::from(level).saturating_mul(14).min(370))).max(550) as u64
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hero_cadence_source_attack_speed_and_floor() {
        assert_eq!(attack_delay_ms(1,0),1386);assert_eq!(attack_delay_ms(30,0),1030);
        assert_eq!(attack_delay_ms(30,8),550);assert_eq!(attack_delay_ms(30,-2),1150);
    }
    #[test]
    fn hero_cadence_requests_never_advance_deadline_and_spell_action_is_600ms() {
        let mut s=crate::SimulationSession::new(crate::SimulationConfig::default());
        let now=Instant::now();set(s.app.world_mut(),now,600,Some(600));
        for _ in 0..1000{assert!(!action_ready_at(s.app.world(),now+Duration::from_millis(599)));}
        assert!(action_ready_at(s.app.world(),now+Duration::from_millis(600)));
        reset(s.app.world_mut());assert!(action_ready_at(s.app.world(),now));
    }
}

#[derive(Clone)]
pub(super) struct TransientSnapshot(Option<HeroCadence>);
pub(super) fn capture_transient(world:&World)->TransientSnapshot {TransientSnapshot(world.get_resource::<HeroCadence>().cloned())}
pub(super) fn restore_transient(world:&mut World,snapshot:&TransientSnapshot){world.remove_resource::<HeroCadence>();if let Some(state)=&snapshot.0 {world.insert_resource(state.clone());}}
