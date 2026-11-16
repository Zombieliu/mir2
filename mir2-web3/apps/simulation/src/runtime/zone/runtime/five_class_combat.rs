use super::*;

const MAX_ARROW_RANGE: i32 = 9; // Crystal Globals.MaxAttackRange.

pub(super) fn finish_attack_clock(player: &mut ZonePlayer, now_ms: u64) {
    if !matches!(player.class, MirClass::Assassin | MirClass::Archer) {
        return;
    }
    player.next_attack_ready_at_ms =
        now_ms.saturating_add(five_class_skills::source_attack_delay(player));
    // AttackTime and ActionTime are separate in Crystal. Haste reduces the
    // former, while movement/casting can resume after the 550ms action.
    let action_until = now_ms.saturating_add(ZONE_NATIVE_PLAYER_MELEE_ACTION_MS);
    player.movement_ready_at_ms = player.movement_ready_at_ms.max(action_until);
    player.next_spell_ready_at_ms = player.next_spell_ready_at_ms.max(action_until);
}

fn archer_state_damage(player: &ZonePlayer, damage: i32) -> i32 {
    let percent = match player.combat_stats.mental_state {
        1 => 55 + 5 * i32::from(player.combat_stats.mental_state_level.min(3)),
        2 => 80,
        _ => 100,
    };
    damage.saturating_mul(percent).saturating_div(100).max(0)
}

fn range_min(min: i32, range: i32) -> i32 {
    // MapObject.GetRangeAttackPower: min -= floor(min / 9 * (9-range)).
    let min = min.max(0);
    min.saturating_sub(
        (i64::from(min) * i64::from(MAX_ARROW_RANGE - range)).div_euclid(i64::from(MAX_ARROW_RANGE))
            as i32,
    )
}

pub(super) fn arrow_damage(
    player: &ZonePlayer,
    target: &Point,
    spell: Spell,
    level: u8,
    fallback: i32,
    now_ms: u64,
) -> i32 {
    if !player.combat_stats.has_authoritative_damage() {
        return fallback.max(0);
    }
    let stats = player.combat_stats;
    let magical = matches!(spell, Spell::StraightShot | Spell::DoubleShot);
    // Trusted projection already contains equipment, passives and mirrored buffs.
    let (min, max) = if magical {
        (stats.min_mc, stats.max_mc)
    } else {
        (stats.min_dc, stats.max_dc)
    };
    let raw = zone_roll_attack_power(
        range_min(min, zone_tile_distance(&player.position, target)),
        max,
        stats.luck,
        now_ms,
        player.object_id,
        0xA220 + u64::from(spell as u8),
    );
    let raw = if magical {
        crystal_magic_by_spell(&format!("{spell:?}"))
            .map(|magic| crate::runtime::combat::crystal_magic_damage_from_base(&magic, level, raw))
            .unwrap_or(raw)
    } else {
        raw
    };
    archer_state_damage(player, raw)
}

pub(super) fn arrow_hit_chance(distance: i32, focus: bool) -> i32 {
    // RangeAccuracyBonus defaults to zero. Focus doubles accuracy, not damage.
    (100 - (100 / MAX_ARROW_RANGE) * distance).max(0) * if focus { 2 } else { 1 }
}

pub(super) fn ordinary_arrow_hit(
    player: &ZonePlayer,
    monster: &ZoneNativeMonster,
    object_id: u32,
    focus: bool,
    now_ms: u64,
) -> bool {
    let chance = arrow_hit_chance(
        zone_tile_distance(&player.position, &monster.position),
        focus,
    );
    let roll = crate::runtime::combat::crystal_accuracy_roll(
        now_ms,
        player.object_id,
        object_id ^ 0xA220,
        100,
    );
    roll < u64::try_from(chance).unwrap_or(0) && physical_hit(player, monster, object_id, now_ms)
}

fn physical_hit(
    player: &ZonePlayer,
    monster: &ZoneNativeMonster,
    object_id: u32,
    now_ms: u64,
) -> bool {
    if !player.combat_stats.has_authoritative_damage() || monster.owner_session_id.is_some() {
        return true;
    }
    let agility = monster.defense.agility.max(0);
    agility == 0
        || crate::runtime::combat::crystal_accuracy_roll(
            now_ms,
            player.object_id,
            object_id,
            u64::try_from(agility.saturating_add(1)).unwrap_or(1),
        ) <= u64::try_from(player.combat_stats.accuracy.max(0)).unwrap_or(0)
}

pub(super) fn ordinary_arrow_damage(
    player: &ZonePlayer,
    monster: &ZoneNativeMonster,
    fallback: i32,
    now_ms: u64,
) -> i32 {
    let raw = arrow_damage(player, &monster.position, Spell::None, 0, fallback, now_ms);
    armour_damage(player, monster, raw, false, now_ms)
}

fn armour_damage(
    player: &ZonePlayer,
    monster: &ZoneNativeMonster,
    raw: i32,
    magical: bool,
    now_ms: u64,
) -> i32 {
    if monster.owner_session_id.is_some() {
        return raw.max(0);
    }
    if magical {
        return zone_magic_damage_after_monster_armour(monster, raw, player.object_id, now_ms);
    }
    let (min_ac, max_ac) = horned_stat_range(
        monster,
        HornedStat::Ac,
        monster.defense.min_ac,
        monster.defense.max_ac,
        now_ms,
    );
    let ac = zone_roll_stat_range(min_ac, max_ac, now_ms, player.object_id, 0x0AC);
    raw.saturating_sub(ac).max(0)
}

/// Source damageBase before the Fatal/MPEater/Hemorrhage branches.
pub(super) fn melee_base_power(player: &ZonePlayer, fallback: i32, bonus: i32, now_ms: u64) -> i32 {
    let stats = player.combat_stats;
    let raw = if stats.has_authoritative_damage() {
        zone_roll_attack_power(
            stats.min_dc,
            stats.max_dc,
            stats.luck,
            now_ms,
            player.object_id,
            0x5DC,
        )
    } else {
        fallback.max(0)
    };
    raw.saturating_add(bonus).max(0)
}

pub(super) fn fatal_base_power(player: &ZonePlayer, base: i32, fatal: bool) -> i32 {
    if !fatal {
        return base;
    }
    crystal_magic_by_spell("FatalSword")
        .map(|magic| {
            crate::runtime::combat::crystal_magic_damage_from_base(
                &magic,
                player.combat_stats.fatal_sword_level.unwrap_or(0),
                base,
            )
        })
        .unwrap_or(base)
}

pub(super) fn melee_final_power(
    player: &ZonePlayer,
    spell: Spell,
    level: u8,
    base: i32,
    fatal: bool,
    passive: Option<(i32, EntityDefence)>,
    _now_ms: u64,
) -> (i32, EntityDefence) {
    // HumanObject snapshots normal damageFinal before modifying Fatal's
    // damageBase. MP/Bleed overwrite damageFinal; DoubleSlash reuses the same
    // Fatal-modified damageBase, not the previous passive's damageFinal.
    let (mut raw, mut defence) = passive.unwrap_or((base, EntityDefence::ACAgility));
    if spell == Spell::DoubleSlash {
        raw = crystal_magic_by_spell("DoubleSlash")
            .map(|magic| {
                crate::runtime::combat::crystal_magic_damage_from_base(
                    &magic,
                    level,
                    fatal_base_power(player, base, fatal),
                )
            })
            .unwrap_or(base);
        if matches!(defence, EntityDefence::ACAgility) {
            defence = EntityDefence::MACAgility;
        }
    }
    if fatal {
        defence = EntityDefence::Agility;
    }
    // Attacked rolls Critical separately for each actual impact, after an
    // armour block. Both delayed DoubleSlash actions capture uncritical raw.
    (raw, defence)
}

/// Strike two is queued independently, even if the primary misses Agility.
pub(super) fn assassin_melee_damage(
    player: &ZonePlayer,
    monster: &ZoneNativeMonster,
    object_id: u32,
    spell: Spell,
    raw: i32,
    defence: EntityDefence,
    now_ms: u64,
) -> Option<(i32, i32, EntityDefence)> {
    let first_hit = physical_hit(player, monster, object_id, now_ms);
    let second_hit = spell == Spell::DoubleSlash
        && physical_hit(player, monster, object_id, now_ms.saturating_add(400));
    if !first_hit && !second_hit {
        return None;
    }
    let first = if !first_hit {
        0
    } else if matches!(defence, EntityDefence::Agility) {
        raw
    } else {
        armour_damage(
            player,
            monster,
            raw,
            matches!(defence, EntityDefence::MACAgility),
            now_ms,
        )
    };
    Some((
        first.saturating_mul(qa_natural_kill_damage_multiplier()),
        if second_hit {
            raw.saturating_mul(qa_natural_kill_damage_multiplier())
        } else {
            0
        },
        defence,
    ))
}

impl ZoneRuntime {
    pub(super) fn five_class_practice(
        &mut self,
        session: &SessionId,
        spell: Spell,
        now_ms: u64,
    ) -> Option<ZoneOutbound> {
        use super::super::five_class_receipts::{
            ZoneFiveClassPracticeReceipt, ZoneFiveClassPracticeSpell,
        };
        let spell = ZoneFiveClassPracticeSpell::from_spell(spell)?;
        let owner = self.owned_pet_owner(session)?;
        if !self.owned_pet_owner_is_current(&owner) {
            return None;
        }
        let player = self.players.get_mut(session)?;
        if player.dead
            || player.class != spell.class()
            || player.combat_stats.five_class_learned_mask & spell.learned_bit() == 0
        {
            return None;
        }
        player.five_class_practice_sequence = player
            .five_class_practice_sequence
            .checked_add(1)
            .expect("Zone five-class practice sequence exhausted");
        Some(ZoneOutbound::FiveClassMagicPractice {
            receipt: ZoneFiveClassPracticeReceipt {
                spell,
                session_id: session.clone(),
                account_id: player.account_id.clone(),
                character_index: player.character_index,
                object_id: player.object_id,
                life_generation: player.life_generation,
                zone_key: self.key.clone(),
                sequence: player.five_class_practice_sequence,
                committed_at_ms: now_ms,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_range_min_and_focus_accuracy_are_not_damage_bonuses() {
        assert_eq!(range_min(9, 1), 1);
        assert_eq!(range_min(9, 9), 9);
        assert_eq!(arrow_hit_chance(5, false), 45);
        assert_eq!(arrow_hit_chance(5, true), 90);
        assert_eq!(arrow_hit_chance(9, false), 1);
        assert_eq!(arrow_hit_chance(9, true), 2);
    }
}
