//! Crystal MonsterObject.Attacked(HumanObject) arithmetic, with injected RNG.
//! Hero dispatch supplies its own stats; no player cast or accuracy scope is read.
use super::super::combat::CrystalDefence;
#[derive(Clone,Copy,Default)]
pub(super) struct Attacker {pub accuracy:i32,pub attack_bonus:i32,pub critical_rate:i32,pub critical_damage:i32}
#[derive(Clone,Copy)]
pub(super) struct Defender {pub min_ac:i32,pub max_ac:i32,pub min_mac:i32,pub max_mac:i32,pub agility:i32,pub magic_resist:i32,pub armour_rate:f32,pub damage_rate:f32}
#[derive(Clone,Copy)]
pub(super) struct Weights {pub magic_resist:u32,pub critical_rate:i32,pub critical_damage:i32}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) struct Hit {pub damage:i32,pub critical:bool}
fn roll_armour(min:i32,max:i32,rng:&mut impl FnMut(u32)->u32)->i32{
    let min=min.max(0);let max=max.max(min);
    min.saturating_add(rng((i64::from(max)-i64::from(min)+1).min(u32::MAX as i64) as u32) as i32)
}
pub(super) fn resolve(damage:i32,defence:CrystalDefence,a:Attacker,d:Defender,w:Weights,mut rng:impl FnMut(u32)->u32)->Hit{
    let mut hit=true;
    if matches!(defence,CrystalDefence::Mac|CrystalDefence::MacAgility){hit &= (rng(w.magic_resist) as i64)>=i64::from(d.magic_resist);}
    if matches!(defence,CrystalDefence::AcAgility|CrystalDefence::MacAgility|CrystalDefence::Agility){hit &= i64::from(rng(d.agility.max(0) as u32+1))<=i64::from(a.accuracy);}
    // The source rolls armour even for an agility/resistance miss, preserving RNG order.
    let armour=match defence{
        CrystalDefence::Ac|CrystalDefence::AcAgility=>roll_armour(d.min_ac,d.max_ac,&mut rng),
        CrystalDefence::Mac|CrystalDefence::MacAgility=>roll_armour(d.min_mac,d.max_mac,&mut rng),
        _=>0,
    };
    if !hit{return Hit{damage:0,critical:false};}
    let armour=(armour as f32*d.armour_rate) as i32;
    let damage=((damage as f32*d.damage_rate) as i32).saturating_add(a.attack_bonus);
    if armour>=damage{return Hit{damage:0,critical:false};}
    let critical=i64::from(rng(100))<i64::from(a.critical_rate)*i64::from(w.critical_rate);
    let damage=if critical{
        let bonus=(f64::from(damage)*((f64::from(a.critical_damage)/f64::from(w.critical_damage.max(1)))*10.0)).floor();
        (f64::from(damage)+bonus).clamp(i32::MIN as f64,i32::MAX as f64) as i32
    }else{damage};
    Hit{damage:damage.saturating_sub(armour).max(0),critical}
}
#[cfg(test)]
mod tests{
    use super::*;
    fn defender()->Defender{Defender{min_ac:20,max_ac:20,min_mac:7,max_mac:7,agility:10,magic_resist:0,armour_rate:1.0,damage_rate:1.0}}
    fn weights()->Weights{Weights{magic_resist:10,critical_rate:5,critical_damage:50}}
    #[test]
    fn hero_hit_accuracy_and_ac_both_apply_without_owner_stats(){
        let a=Attacker{accuracy:10,..Default::default()};
        assert_eq!(resolve(30,CrystalDefence::AcAgility,a,defender(),weights(),|_|0),Hit{damage:10,critical:false});
        let low=Attacker{accuracy:0,..a};let mut calls=0;
        let hit=resolve(30,CrystalDefence::AcAgility,low,defender(),weights(),|bound|{calls+=1;if calls==1{bound-1}else{0}});
        assert_eq!(hit.damage,0);assert_eq!(calls,2); // agility + armour, no critical roll on miss.
    }
    #[test]
    fn hero_hit_magic_resist_and_source_bonus_crit_order(){
        let mut d=defender();d.magic_resist=10;
        assert_eq!(resolve(30,CrystalDefence::Mac,Attacker::default(),d,weights(),|_|0).damage,0);
        d.magic_resist=0;d.armour_rate=0.5;d.damage_rate=1.5;
        let a=Attacker{attack_bonus:5,critical_rate:20,critical_damage:5,..Default::default()};
        // 30*1.5+5=50; critical adds 50 before subtracting truncated 7*.5=3.
        assert_eq!(resolve(30,CrystalDefence::Mac,a,d,weights(),|_|0),Hit{damage:97,critical:true});
    }
}
