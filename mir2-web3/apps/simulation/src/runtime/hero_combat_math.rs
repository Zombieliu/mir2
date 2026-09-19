//! Source MapObject.GetAttackPower and UserMagic.GetPower/GetDamage.
//! Random bounds are exclusive; power rounding is C# midpoint-to-even.
use bevy_ecs::prelude::{Resource,World};
use mir2_game_data::CrystalMagicTemplate;
#[derive(Resource)]
struct HeroRandom { state:u64 }
impl HeroRandom {
    fn below(&mut self,bound:u64)->u64 {
        if bound==0 {return 0;}
        let cutoff=bound.wrapping_neg()%bound;
        loop {
            self.state=self.state.wrapping_add(0x9e3779b97f4a7c15);
            let mut x=self.state;x=(x^(x>>30)).wrapping_mul(0xbf58476d1ce4e5b9);x=(x^(x>>27)).wrapping_mul(0x94d049bb133111eb);x^=x>>31;
            if x>=cutoff {return x%bound;}
        }
    }
    fn attack(&mut self,min:i32,max:i32,luck:i32,max_luck:u8)->i32 {
        let min=min.max(0);let max=max.max(min);
        if luck>0 && i64::from(luck)>self.below(u64::from(max_luck)) as i64 {return max;}
        if luck<0 && i64::from(luck) < -(self.below(u64::from(max_luck)) as i64) {return min;}
        (i64::from(min)+self.below((i64::from(max)-i64::from(min)+1) as u64) as i64) as i32
    }
    fn power(&mut self,magic:&CrystalMagicTemplate,level:u8,supplied:Option<i32>)->i32 {
        let power=supplied.unwrap_or_else(||i32::from(magic.mpower_base)+self.below(u64::from(magic.mpower_bonus)) as i32);
        let flat=i32::from(magic.power_base)+self.below(u64::from(magic.power_bonus)) as i32;
        ((power as f32/4.0)*(f32::from(level)+1.0)+flat as f32).round_ties_even() as i32
    }
}
fn rng(world:&mut World)->bevy_ecs::change_detection::Mut<'_,HeroRandom> {
    if world.get_resource::<HeroRandom>().is_none() {
        let seed=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
        world.insert_resource(HeroRandom{state:seed});
    }
    world.resource_mut::<HeroRandom>()
}
pub(super) fn attack(world:&mut World,min_stat:u8,max_stat:u8)->i32 {
    let stats=super::super::hero_stats::compute(world);
    rng(world).attack(stats.get(min_stat),stats.get(max_stat),stats.get(15),mir2_game_data::crystal_hero_settings().rules.max_luck)
}
pub(super) fn power(world:&mut World,magic:&CrystalMagicTemplate,level:u8,supplied:Option<i32>)->i32 {rng(world).power(magic,level,supplied)}
pub(super) fn damage(world:&mut World,magic:&CrystalMagicTemplate,level:u8,base:i32)->i32 {
    let power=power(world,magic,level,None);
    ((base.saturating_add(power) as f32)*(magic.multiplier_base+f32::from(level)*magic.multiplier_bonus)) as i32
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hero_random_damage_replays_bounds_and_source_luck_extremes() {
        let mut a=HeroRandom{state:42};let mut b=HeroRandom{state:42};
        for _ in 0..1000 {let x=a.attack(2,7,0,10);assert!((2..=7).contains(&x));assert_eq!(x,b.attack(2,7,0,10));}
        assert_eq!(a.attack(2,7,10,10),7);assert_eq!(a.attack(2,7,-10,10),2);
        assert_eq!(a.attack(-3,-5,0,10),0);
    }
    #[test]
    fn hero_random_magic_power_uses_even_rounding_and_exclusive_bonuses() {
        let mut magic=mir2_game_data::crystal_magic_by_spell("FireBall").unwrap();magic.power_base=0;magic.power_bonus=0;magic.mpower_base=0;magic.mpower_bonus=0;
        let mut rng=HeroRandom{state:9};
        assert_eq!(rng.power(&magic,1,Some(1)),0);
        assert_eq!(rng.power(&magic,1,Some(3)),2);
        magic.power_base=3;magic.power_bonus=2;magic.mpower_base=4;magic.mpower_bonus=1;
        for _ in 0..100 {assert!((4..=5).contains(&rng.power(&magic,0,None)));}
    }
}
