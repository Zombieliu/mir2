use super::*;
use crate::runtime::zone::types::ZonePlayerCombatStats;
use crate::runtime::drops::{crystal_adjusted_drop_denominator, zone_ground_drop_snapshots_for_monster_at_tick_with_rate};
use mir2_protocol::{MirClass, MirGender};

fn joined(zone: &mut ZoneRuntime, id: &str, object_id: u32, bonus: i32) -> SessionId {
    let id = SessionId::new(id);
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id:id.clone(),account_id:format!("acct-{object_id}"),character_index:object_id as i32,
        object_id,name:format!("player-{object_id}"),class:MirClass::Warrior,gender:MirGender::Male,
        level:20,hp:200,max_hp:200,mp:100,map_file_name:"0".into(),position:Point{x:10,y:10},
        direction:MirDirection::Down,chat_profile:Default::default(),
        combat_stats:ZonePlayerCombatStats{item_drop_rate_percent:bonus,..Default::default()},
    })); id
}

fn kill(bonus:i32, observer_bonus:i32, source:bool)->Vec<crate::GroundDropSnapshot> {
    let mut zone=ZoneRuntime::new_with_collision(ZoneKey::for_map("0"),ZoneCollision::unbounded());
    let owner=joined(&mut zone,"owner",101,bonus);
    joined(&mut zone,"observer",102,observer_bonus);
    let name="Scarecrow";
    let drops=zone_ground_drop_snapshots_for_monster_at_tick_with_rate(9001,name,7,0);
    zone.handle(ZoneCommand::SpawnMonster{session_id:owner.clone(),now_ms:0,monster:ZoneMonsterSpawn{
        crystal_drop_seed:source.then_some(7),object_id:9001,name:name.into(),name_colour_argb:-1,image:0,
        ai:0,disposition:Some(crate::WorldEntityDisposition::Hostile),level:1,max_hp:10,hp:10,experience:1,
        move_speed_ms:600,attack_speed_ms:1200,friendly_guild:None,position:Point{x:11,y:10},
        direction:MirDirection::Down,defense:Default::default(),respawn:None,drops,
    }});
    let result=zone.apply_native_monster_damage(9001,10,Some(&owner),1000).unwrap();
    assert!(result.2); assert_eq!(result.7,Some(owner)); result.8
}

#[test]
fn reward_rate_shared_zone_uses_owner_bonus_and_preserves_explicit_loot() {
    let base=kill(0,100,true);
    let boosted=kill(100,0,true);
    assert!(boosted.len()>base.len(),"source table must expose a genuine rate effect");
    let expected=zone_ground_drop_snapshots_for_monster_at_tick_with_rate(9001,"Scarecrow",7,100);
    assert_eq!(serde_json::to_value(&boosted).unwrap(),serde_json::to_value(expected).unwrap());
    assert_eq!(serde_json::to_value(kill(100,0,false)).unwrap(),serde_json::to_value(&base).unwrap(),
        "explicit/custom loot must not be overwritten with a source table");
    assert_eq!(serde_json::to_value(kill(100,0,true)).unwrap(),serde_json::to_value(boosted).unwrap(),
        "same authoritative death seed must be replay stable");
}

#[test]
fn reward_rate_uses_original_integer_denominator_reduction() {
    assert_eq!(crystal_adjusted_drop_denominator(100,50),50);
    assert_eq!(crystal_adjusted_drop_denominator(10,20),8);
    assert_eq!(crystal_adjusted_drop_denominator(3,50),2);
    assert_eq!(crystal_adjusted_drop_denominator(10,-20),10);
    assert_eq!(crystal_adjusted_drop_denominator(10,100),1);
    assert_eq!(crystal_adjusted_drop_denominator(u32::MAX,i32::MAX),1);
}
