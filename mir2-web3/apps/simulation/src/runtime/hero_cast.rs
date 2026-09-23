//! Hero-only cast clocks and ordinary Magic actor routing.
use super::*;
use std::time::{Duration,Instant};
#[derive(Resource,Default,Clone)]
struct HeroCastClock { identity: String, casts:BTreeMap<u8,Instant>, next_spell:Option<Instant> }
fn identity(world:&World)->String {
    world.resource::<Stage5SystemsResource>().stage5_systems.hero.as_ref()
        .map(|hero|format!("{}:{:?}:{:?}",hero.name,hero.class,hero.gender)).unwrap_or_default()
}
pub(in crate::runtime) fn reset(world:&mut World) { world.remove_resource::<HeroCastClock>(); }
fn clock(world:&mut World)->bevy_ecs::change_detection::Mut<'_,HeroCastClock> {
    let key=identity(world);
    if world.get_resource::<HeroCastClock>().is_none_or(|state|state.identity!=key) {
        world.insert_resource(HeroCastClock{identity:key,..Default::default()});
    }
    world.resource_mut::<HeroCastClock>()
}
pub(in crate::runtime) fn cast_offset(world:&World,spell:Spell,delay:i64)->i64 {
    world.get_resource::<HeroCastClock>().filter(|state|state.identity==identity(world))
        .and_then(|state|state.casts.get(&(spell as u8)))
        .map(|last|-(last.elapsed().as_millis().min(i64::MAX as u128) as i64))
        .unwrap_or_else(||-delay.max(0))
}
pub(super) fn available(world:&World,spell:Spell,delay:i64)->bool {
    let now=Instant::now();
    world.get_resource::<HeroCastClock>().filter(|state|state.identity==identity(world)).is_none_or(|state| {
        state.next_spell.is_none_or(|deadline|now>=deadline) &&
        state.casts.get(&(spell as u8)).is_none_or(|last|now.duration_since(*last).as_millis()>=delay.max(0) as u128)
    })
}
pub(in crate::runtime) fn record(world:&mut World,packets:&[ServerPacket]) {
    let Some(hero)=hero_entity(world) else{return;};
    let Some(id)=world.get::<ObjectId>(hero).map(|id|id.0) else{return;};
    for packet in packets {
        let spell=match packet {
            ServerPacket::ObjectMagic{object_id,spell,cast:true,..} if *object_id==id=>Some(*spell),
            _=>None,
        };
        if let Some(spell)=spell {
            super::hero_cadence::cast(world);
            let mut state=clock(world);let now=Instant::now();
            state.casts.insert(spell as u8,now);
            state.next_spell=Some(now+Duration::from_millis(1800));
        }
    }
}
pub(in crate::runtime) fn manual_magic(world:&mut World,actor_id:u32,spell:Spell,direction:MirDirection,target_id:u32,location:Point)->Vec<ServerPacket> {
    if !hero_attack_mode_allowed(world) || !super::hero_cadence::action_ready(world) {return vec![];}
    let Some(hero)=hero_entity(world) else{return vec![];};
    if world.get::<ObjectId>(hero).is_none_or(|id|id.0!=actor_id)
        || !world.resource::<Stage5SystemsResource>().stage5_systems.hero.as_ref().is_some_and(|hero|hero.spawned)
        || world.get::<PlayerVitals>(hero).is_none_or(|v|v.hp<=0) {return vec![];}
    let Some(position)=entity_position(world,hero) else{return vec![];};
    let Some(body)=world.get::<CharacterBody>(hero) else{return vec![];};
    let (class,level)=(body.class,body.level);
    let Some(magic)=crystal_magic_by_spell(&format!("{spell:?}")) else{return vec![];};
    let Some(skill_level)=hero_learned_magic_level(world,spell,0) else{return vec![];};
    let delay=hero_magic_delay_ms(&magic,skill_level);
    if !available(world,spell,delay) {return vec![];}
    if magic.range!=0 && location.x!=0 && location.y!=0 && tile_distance(&position,&location)>i32::from(magic.range) {return vec![];}
    let target=world.iter_entities().find_map(|entry| {
        if entry.get::<ObjectId>().is_none_or(|id|id.0!=target_id)||entry.get::<Monster>().is_none() {return None;}
        let agent=entry.get::<MonsterAgent>()?;
        if agent.dead||agent.ai==64||is_hidden_or_sleeping_target(agent,&entry.get::<MonsterAiState>().copied().unwrap_or_default()) {return None;}
        let target_position=entry.get::<Position>()?.0.clone();
        Some(HeroAiTarget{entity:entry.id(),object_id:target_id,name:entity_name(world,entry.id()).unwrap_or_default(),position:target_position})
    });
    let Some(target)=target else{return vec![];};
    let distance=tile_distance(&position,&target.position);
    if magic.range!=0 && distance>i32::from(magic.range) {return vec![];}
    let Some(choice)=hero_wizard_spell_choices(world,Some(class),Some(level),&position,&target,distance).into_iter().find(|choice|choice.spell==spell) else{return vec![];};
    let tick=super::super::resources::runtime_tick(world);
    let mut packets=Vec::new();
    if cast_hero_wizard_spell_choice(world,tick,hero,actor_id,&position,direction,Some(level),&target,&choice,&mut packets) {
        world.entity_mut(hero).insert(Facing(direction));
        record(world,&packets);
        super::hero_buffs::capture(world,&packets);
        if let Some(v)=world.get::<PlayerVitals>(hero) {packets.push(ServerPacket::HeroHealthChanged{hp:v.hp,mp:v.mp});}
    }
    packets
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::{ClientPacket,MirGender};
    fn scene()->(crate::SimulationSession,u32,u32,Point) {
        let mut s=crate::SimulationSession::new(crate::SimulationConfig::default());
        s.handle_packet(ClientPacket::Login{account_id:"demo".into(),password:"demo".into()});
        s.handle_packet(ClientPacket::StartGame{character_index:0});
        s.handle_packet(ClientPacket::NewHero{name:"Aide".into(),gender:MirGender::Female,class:MirClass::Wizard});
        let world=s.app.world_mut();
        world.resource_mut::<Stage5SystemsResource>().stage5_systems.hero_learned_magics.push(crate::config::Stage5HeroMagicState{spell:Spell::FireBall,key:17,level:3,experience:0});
        let hero=hero_entity(world).unwrap();
        world.get_mut::<CharacterBody>(hero).unwrap().level=30;
        let id=world.get::<ObjectId>(hero).unwrap().0;
        world.get_mut::<PlayerVitals>(hero).unwrap().mp=100;
        let mut position=entity_position(world,hero).unwrap();position.x+=1;
        let monster=world.iter_entities().find(|e|e.get::<Monster>().is_some()).unwrap().id();
        world.entity_mut(monster).insert(Position(position.clone()));
        world.get_mut::<MonsterAgent>(monster).unwrap().hostile_to_player=true;
        world.get_mut::<MonsterAgent>(monster).unwrap().dead=false;
        let target=world.get::<ObjectId>(monster).unwrap().0;
        (s,id,target,position)
    }
    fn packet(id:u32,target:u32,location:Point)->ClientPacket {
        ClientPacket::Magic{object_id:id,spell:Spell::FireBall,direction:MirDirection::Right,target_id:target,location,spell_target_lock:false}
    }
    #[test]
    fn hero_cast_ordinary_magic_uses_hero_mana_and_actor_and_rejects_replay() {
        let (mut s,id,target,location)=scene();
        let player=player_entity(s.app.world()).unwrap();let hero=hero_entity(s.app.world()).unwrap();
        let player_mp=s.app.world().get::<PlayerVitals>(player).unwrap().mp;
        let hero_mp=s.app.world().get::<PlayerVitals>(hero).unwrap().mp;
        let result=s.handle_packet(packet(id,target,location.clone()));
        assert!(result.iter().any(|p|matches!(p,ServerPacket::ObjectMagic{object_id,cast:true,spell:Spell::FireBall,..} if *object_id==id)),"{result:?}");
        assert_eq!(s.app.world().get::<PlayerVitals>(player).unwrap().mp,player_mp);
        assert!(s.app.world().get::<PlayerVitals>(hero).unwrap().mp<hero_mp);
        let after=s.app.world().get::<PlayerVitals>(hero).unwrap().mp;
        assert!(s.handle_packet(packet(id,target,location)).is_empty());
        assert_eq!(s.app.world().get::<PlayerVitals>(hero).unwrap().mp,after);
    }
    #[test]
    fn hero_cast_invalid_actor_target_and_unlearned_magic_do_not_spend_player_mana() {
        let (mut s,id,_,location)=scene();let player=player_entity(s.app.world()).unwrap();let hero=hero_entity(s.app.world()).unwrap();
        let player_mp=s.app.world().get::<PlayerVitals>(player).unwrap().mp;
        let hero_mp=s.app.world().get::<PlayerVitals>(hero).unwrap().mp;
        assert!(s.handle_packet(packet(id,999999,location.clone())).is_empty());
        assert!(s.handle_packet(packet(id+999,999999,location)).is_empty());
        assert_eq!(s.app.world().get::<PlayerVitals>(player).unwrap().mp,player_mp);
        assert_eq!(s.app.world().get::<PlayerVitals>(hero).unwrap().mp,hero_mp);
    }
    #[test]
    fn hero_cast_wire_offset_retains_elapsed_without_information_refresh_restart() {
        let (mut s,id,_,_)=scene();
        let p=ServerPacket::ObjectMagic{object_id:id,location:Point{x:0,y:0},direction:MirDirection::Down,spell:Spell::FireBall,target_id:0,target:Point{x:0,y:0},cast:true,level:3,self_broadcast:false,secondary_target_ids:vec![]};
        assert_eq!(cast_offset(s.app.world(),Spell::FireBall,4000),-4000);
        record(s.app.world_mut(),&[p]);
        clock(s.app.world_mut()).casts.insert(Spell::FireBall as u8,Instant::now()-Duration::from_millis(2500));
        for _ in 0..1000 { assert!(cast_offset(s.app.world(),Spell::FireBall,4000)<=-2500); }
        assert!(!available(s.app.world(),Spell::FireBall,4000));
        let mut state=clock(s.app.world_mut());state.next_spell=None;state.casts.insert(Spell::FireBall as u8,Instant::now()-Duration::from_millis(4001));
        drop(state);
        assert!(available(s.app.world(),Spell::FireBall,4000));
    }
}

#[derive(Clone)]
pub(super) struct TransientSnapshot(Option<HeroCastClock>);
pub(super) fn capture_transient(world:&World)->TransientSnapshot {TransientSnapshot(world.get_resource::<HeroCastClock>().cloned())}
pub(super) fn restore_transient(world:&mut World,snapshot:&TransientSnapshot){world.remove_resource::<HeroCastClock>();if let Some(state)=&snapshot.0 {world.insert_resource(state.clone());}}
