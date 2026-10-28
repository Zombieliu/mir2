//! Crystal Human.Attacked damage for the new professions. Launches retain
//! uncritical power; each current typed impact owns its own critical roll.
//!
//! Source: HumanObject.cs:7101..7177, MonsterObject.cs:2576..2599 and
//! MapObject.cs:460. In particular an armour block happens before Critical.
use super::*;

pub(super) const CRITICAL_EFFECT: u8 = 11;
pub(super) const REFLECT_EFFECT: u8 = 10;
pub(super) const MAX_REFLECT_CHAIN: u8 = 16;

#[derive(Debug, Clone, Copy)]
pub(super) struct PreparedImpact {
    /// Human Reflect passes this power to the reverse Attacked call, before
    /// the original victim's DamageReductionPercent and Critical branches.
    pub(super) reflection_power: i32,
    pub(super) armour: i32,
    pub(super) power: i32,
}

impl PreparedImpact {
    pub(super) fn blocked(self) -> bool {
        self.armour >= self.power
    }
}

/// A current admitted Attacked result and its canonical HP settlement are distinct.
/// Source returns the final defended damage even when the victim had less HP.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct HitSettlement {
    pub(super) resolved_damage: i32,
    pub(super) applied_damage: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResolvedImpact {
    pub(super) damage: i32,
    pub(super) critical: bool,
}

fn with_rate(value: i32, numerator: i32, denominator: i32) -> i32 {
    ((i64::from(value.max(0)) * i64::from(numerator)) / i64::from(denominator))
        .clamp(0, i64::from(i32::MAX)) as i32
}

fn mitigated_armour(defence: EntityDefence) -> bool {
    matches!(
        defence,
        EntityDefence::AC
            | EntityDefence::ACAgility
            | EntityDefence::MAC
            | EntityDefence::MACAgility
    )
}

pub(super) fn magical(defence: EntityDefence) -> bool {
    matches!(defence, EntityDefence::MAC | EntityDefence::MACAgility)
}

/// Trusted Human armour already includes equipment, passives and mirrored
/// buffs. Never add the same live MaxAC/MaxMAC buff a second time.
pub(super) fn prepare_player(
    source: &ZonePlayer,
    victim: &ZonePlayer,
    raw: i32,
    defence: EntityDefence,
    now: u64,
) -> PreparedImpact {
    let mut armour = if mitigated_armour(defence) {
        let stats = victim.combat_stats;
        let (min, max) = if magical(defence) {
            (stats.min_mac, stats.max_mac)
        } else {
            (stats.min_ac, stats.max_ac)
        };
        zone_roll_stat_range(
            min.max(0),
            max.max(min).max(0),
            now,
            victim.object_id,
            if magical(defence) { 0x3AC } else { 0x4AC },
        )
    } else {
        0
    };
    let poison = zone_player_active_status(victim, now);
    if poison & CRYSTAL_POISON_RED != 0 {
        armour = with_rate(armour, 9, 10);
    }
    let power = if poison & CRYSTAL_POISON_STUN != 0 {
        with_rate(raw, 6, 5)
    } else {
        raw.max(0)
    };
    let reflection_power = power
        .saturating_add(source.combat_stats.attack_bonus)
        .max(0);
    let reduction = victim.combat_stats.damage_reduction_percent.clamp(0, 100);
    // Source subtracts the truncated reduction, not a truncated remainder:
    // 101 - floor(101 * .20) == 81, whereas floor(101 * .80) == 80.
    let power = reflection_power.saturating_sub(with_rate(reflection_power, reduction, 100));
    PreparedImpact {
        reflection_power,
        armour,
        power,
    }
}

pub(super) fn prepare_monster(
    source: &ZonePlayer,
    victim: &ZoneNativeMonster,
    target_id: u32,
    raw: i32,
    defence: EntityDefence,
    now: u64,
) -> PreparedImpact {
    let mut armour = if mitigated_armour(defence) {
        let (minimum, maximum, min_stat, max_stat) = if magical(defence) {
            let (min, max) = horned_stat_range(
                victim,
                HornedStat::Mac,
                victim.defense.min_mac,
                victim.defense.max_mac,
                now,
            );
            (min, max, 2, 3)
        } else {
            let (min, max) = horned_stat_range(
                victim,
                HornedStat::Ac,
                victim.defense.min_ac,
                victim.defense.max_ac,
                now,
            );
            (min, max, 0, 1)
        };
        // Native monster templates are base stats, so monster-only buffs are
        // added once here; unlike Human stats these are not mirrored totals.
        let minimum = minimum
            .saturating_add(zone_native_monster_buff_stat_total(victim, min_stat))
            .max(0);
        let maximum = maximum
            .saturating_add(zone_native_monster_buff_stat_total(victim, max_stat))
            .max(minimum);
        zone_roll_stat_range(
            minimum,
            maximum,
            now,
            source.object_id,
            u64::from(target_id),
        )
    } else {
        0
    };
    let poison = native_monster_current_poison(victim);
    if poison & CRYSTAL_POISON_RED != 0 {
        armour /= 2;
    }
    let power = if poison & CRYSTAL_POISON_STUN != 0 {
        with_rate(raw, 3, 2)
    } else {
        raw.max(0)
    };
    let power = power
        .saturating_add(source.combat_stats.attack_bonus)
        .max(0);
    PreparedImpact {
        reflection_power: power,
        armour,
        power,
    }
}

/// The actor-local, live sequence separates repeated same-tick/same-target
/// Attacked calls, including CrippleShot. It is not a client-supplied seed or
/// a checkpoint proof, and the caster Node/life fences it in the caller.
pub(super) fn source_chance_roll(
    now: u64,
    source: u32,
    target: u32,
    sequence: u64,
    salt: u64,
    modulo: u64,
) -> u64 {
    if modulo <= 1 {
        return 0;
    }
    // Random.Next in Source admits every result. The legacy linear roll's
    // time coefficient ends in five, so mod ten yields only two outcomes for
    // a fixed actor and target. Mix the trusted impact sequence before the
    // modulus; this also separates actual same-tick Attacked calls.
    let mut value = now.wrapping_add(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(source).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ u64::from(target).wrapping_mul(0x94D0_49BB_1331_11EB)
        ^ sequence.wrapping_mul(0xD6E8_FEB8_6659_FD93)
        ^ salt.wrapping_mul(0xA076_1D64_78BD_642F);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (value ^ (value >> 31)) % modulo
}

fn impact_roll(now: u64, source: u32, target: u32, sequence: u64, salt: usize) -> u64 {
    source_chance_roll(now, source, target, sequence, salt as u64, 100)
}

#[cfg(test)]
mod tests {
    use super::source_chance_roll;
    use std::collections::BTreeSet;

    #[test]
    fn source_control_draws_cover_all_ten_outcomes_for_human_and_monster_ids() {
        for target in [202, 900] {
            let outcomes: BTreeSet<_> = (600..1600)
                .map(|now| source_chance_roll(now, 101, target, 1, 1119, 10))
                .collect();
            assert_eq!(outcomes, (0..10).collect());
            let same_tick_outcomes: BTreeSet<_> = (1..256)
                .map(|sequence| source_chance_roll(700, 101, target, sequence, 1119, 10))
                .collect();
            assert_eq!(same_tick_outcomes, (0..10).collect());
        }
    }
}

pub(super) fn critical_roll(source: &ZonePlayer, target: u32, now: u64, sequence: u64) -> bool {
    let chance = source
        .combat_stats
        .critical_rate
        .max(0)
        .saturating_mul(crate::runtime::crystal_compat::CRYSTAL_CRITICAL_RATE_WEIGHT)
        .min(100);
    chance > 0 && impact_roll(now, source.object_id, target, sequence, 0xC817) < chance as u64
}

pub(super) fn reflection_roll(
    source: &ZonePlayer,
    victim: &ZonePlayer,
    now: u64,
    sequence: u64,
) -> bool {
    let chance = victim.combat_stats.reflect.clamp(0, 100);
    chance > 0
        && impact_roll(now, source.object_id, victim.object_id, sequence, 0xDEF1) < chance as u64
}

pub(super) fn finish(
    prepared: PreparedImpact,
    source: &ZonePlayer,
    target_id: u32,
    now: u64,
    sequence: u64,
) -> ResolvedImpact {
    if prepared.blocked() {
        return ResolvedImpact {
            damage: 0,
            critical: false,
        };
    }
    let critical = critical_roll(source, target_id, now, sequence);
    let power = if critical {
        // Source's CriticalDamage=0 has a Critical effect but no amplification.
        // The legacy three-class helper's minimum-one convention is not part
        // of HumanObject.Attacked and must not enter this new typed boundary.
        let multiplier = (f64::from(source.combat_stats.critical_damage.max(0))
            / f64::from(crate::runtime::crystal_compat::CRYSTAL_CRITICAL_DAMAGE_WEIGHT))
            * 10.0;
        let bonus = (f64::from(prepared.power) * multiplier)
            .floor()
            .clamp(0.0, f64::from(i32::MAX)) as i32;
        prepared.power.saturating_add(bonus)
    } else {
        prepared.power
    };
    ResolvedImpact {
        damage: power.saturating_sub(prepared.armour).max(0),
        critical,
    }
}

impl ZoneRuntime {
    /// Source EnergyShield heals after an admitted hit clears Moon/DarkBody,
    /// before Critical and normal damage. The later HP settlement must skip
    /// its legacy buff-only trigger so equipment+buff projection heals once.
    pub(super) fn five_class_trigger_energy_shield(
        &mut self,
        session: &SessionId,
        attacker_id: u32,
        sequence: u64,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self.players.get_mut(session) else {
            return Vec::new();
        };
        let chance = player.combat_stats.energy_shield_percent.clamp(0, 100);
        let gain = player.combat_stats.energy_shield_hp_gain.max(0);
        if chance == 0
            || gain == 0
            || player.dead
            || impact_roll(now, attacker_id, player.object_id, sequence, 0xE50) >= chance as u64
        {
            return Vec::new();
        }
        let before = player.hp;
        player.set_owner_hp(player.hp.saturating_add(gain).min(player.max_hp));
        let healed = player.hp.saturating_sub(before);
        if healed == 0 {
            return Vec::new();
        }
        let settlement = player.vital_settlement(before);
        let mut out = self.take_owner_health_through(session, u64::MAX);
        out.push(ZoneOutbound::PlayerHealed {
            session_id: session.clone(),
            amount: healed,
            settlement: Some(settlement),
        });
        out
    }

    /// Buff.ExpireTime in Crystal is remaining time, while this Zone retains
    /// absolute deadlines. Reduce exactly the existing finite lease by final
    /// unbounded hit damage*60, and send its true remaining time to the mirror.
    pub(super) fn five_class_shorten_damage_shields(
        &mut self,
        session: &SessionId,
        damage: i32,
        now: u64,
    ) -> Vec<ServerPacket> {
        if damage <= 0 {
            return Vec::new();
        }
        let Some(player) = self.players.get_mut(session) else {
            return Vec::new();
        };
        let loss = (damage as u64).saturating_mul(60);
        let mut packets = Vec::new();
        for kind in [CRYSTAL_MAGIC_SHIELD_BUFF_TYPE, 25] {
            let Some(buff) = player.buffs.get_mut(&kind) else {
                continue;
            };
            if buff.buff.infinite || buff.buff.paused {
                continue;
            }
            let Some(deadline) = buff.expires_at_ms else {
                continue;
            };
            let remaining = deadline
                .saturating_sub(now)
                .saturating_sub(loss)
                .min(i32::MAX as u64);
            buff.expires_at_ms = Some(now.saturating_add(remaining));
            buff.buff.expire_time = remaining as i64;
            packets.push(ServerPacket::AddBuff {
                buff: buff.buff.clone(),
            });
        }
        packets
    }
}
