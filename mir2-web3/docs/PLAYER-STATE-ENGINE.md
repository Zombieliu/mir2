# Player State Engine (人物状态系统) — Crystal Parity

This document describes the authoritative player stat engine added to the
simulation runtime and how it tracks Crystal (`Mir2`/C#) parity.

The engine has been **verified directly against the Crystal source** (the
`Crystal/` submodule), specifically:

- `Shared/Data/Stat.cs` — the `Stat` index space and the `Stats` container.
- `Shared/BaseStats.cs` — `BaseStat.Calculate(job, level)` (the level-scaling formula).
- `Server/MirObjects/HumanObject.cs` — `RefreshStats`, `RefreshStatCaps`,
  `ProcessRegen`, `Attacked`, and `GetArmour` (`Server/MirObjects/MapObject.cs`).

## Module: `apps/simulation/src/runtime/stats.rs`

`PlayerStats` is a Crystal-style stat block keyed by the Crystal `Stat` index
space (`crystal_compat::CRYSTAL_STAT_*`, confirmed 1:1 with `Stat.cs`). It is
assembled by `compute_player_stats(world)` following Crystal `RefreshStats`:

1. **Base class/level** via an exact port of `BaseStat.Calculate`:
   - Combat stats (`StatFormula.Stat`): `Base + level/Gain` (`Gain==0` → flat).
   - Weights (`StatFormula.Weight`): `Base + (level/Gain)*level`.
   - HP/MP (`Health`/`Mana`): identical to the existing validated `crystal_base_vitals`.
2. **Equipment** — scalar attack/defence + all granular `added_stats`.
3. **Buffs** — buff stats with `attack/defence_bonus` → `MaxDC`/`MaxAC`.
4. **`*RatePercent` multipliers** — applied to `HP, MP, MaxAC, MaxMAC, MaxDC,
   MaxMC, MaxSC, AttackSpeed` (Max/pool stats only), exactly as `RefreshStats`.
5. **`RefreshStatCaps`** — the per-class `Caps` (`MagicResist≤2`, `PoisonResist≤6`,
   `CriticalRate≤18`, `CriticalDamage≤10`, recovery `≤8/8/6`), `Max(0)` floors,
   and `Min* ≤ Max*` clamps.

`refresh_player_stats()` recomputes the block and reconciles `MaxHP`/`MaxMP`
onto the player's vitals on equip/unequip/enter-world (`RefreshStats` behaviour).

### Compatibility contract

New dimensions are inert at their zero/seed values, so the existing combat suite
is preserved: `Min/Max` ranges collapse for seed gear, crit/resist/recovery are
`0`, and combat base-stat scaling is `0` at level 1.

## Verified-1:1 behaviours

| Capability | Source verified | Notes |
|---|---|---|
| `Stat` index space | `Stat.cs` | exact |
| HP/MP per class/level | `BaseStats.cs` | exact (incl. Warrior `+level/20`, Wizard/Taoist quadratic MP) |
| Combat/weight base scaling | `BaseStat.Calculate` | exact port |
| Stat caps | `BaseStats.Caps` + `RefreshStatCaps` | exact |
| `*RatePercent` multipliers | `RefreshStats` | exact set, Max-only |
| Skill bonuses (Fencing/Slaying/SpiritSword) | `RefreshSkills` | applied in combat |
| `MagicResist` | `GetArmour` (MAC) | miss-chance `Random(MagicResistWeight=10) < MagicResist` |
| Critical hits | — | `CriticalRate` (cap 18) roll → `CriticalDamage` (cap 10) amplification |
| Passive HP/MP regen | `ProcessRegen` | `pool*3% + 1`, boosted by recovery; pauses in combat |
| `max_mp` pool | `MaxMana`/`SetMP` | tracked + persisted; removed the hard-coded MP=100 caps |
| Marriage/guild/mentor | lover/mentee/guild | Crystal-style experience-rate bonus |

## Test coverage (`runtime::session::tests`)

`player_stats_seed_collapses_dc_range_to_legacy_melee`,
`player_stats_seed_max_mp_tracks_class_base_not_hardcoded_hundred`,
`mana_restore_caps_at_max_mp_not_legacy_hundred`,
`equipping_dc_range_weapon_produces_damage_spread`,
`equipping_hp_gear_raises_max_hp_and_unequip_restores`,
`critical_hit_amplifies_melee_when_crit_stats_present`,
`poison_resistance_reduces_player_poison_tick_damage`,
`magic_resistance_grants_miss_chance_against_incoming_magic`,
`passive_regen_restores_pools_after_combat_delay`,
`passive_regen_is_paused_immediately_after_taking_damage`,
`social_relationships_grant_experience_rate_bonus`,
`player_stats_expose_class_weight_capacities`,
`class_base_stats_scale_with_level_per_crystal_formula`.

## Combat damage curve — now Crystal-numeric

The previously-retained numeric deviations have been closed:

1. **Melee floor removed.** `compute_player_stats` no longer adds the non-Crystal
   `18 + level/2` floor. Melee `Min/MaxDC` come entirely from the class table
   (`BaseStat.Calculate`, ≈0 at low level) plus equipment, exactly as Crystal.
   A level-1 warrior now hits for single digits, not ~24.

2. **Real Crystal starter gear.** `seed_equipment_items` carries the actual item
   manifest stats: WoodenSword `MinDC 2 / MaxDC 4`, LightLeatherArmour
   `MinAC 3 / MaxAC 5 / MinMAC 3 / MaxMAC 4`. Player melee is `Random(MinDC, MaxDC)`
   over the real range.

3. **Roll-based mitigation.** Monster→player physical damage now subtracts
   `crystal_player_rolled_armour` = `Random(MinAC, MaxAC)` (Crystal `GetArmour`),
   reading the stat block's real AC range, instead of a flat defence total. Magic
   blows additionally apply the `MagicResist` miss-chance.

The combat-regression suite was re-baselined for these (kill tests use a lethal
test weapon to keep their defeat-outcome intent; per-hit damage assertions became
armour-roll ranges).

## Shared-zone combat parity

The shared zone (the emerging world-combat authority) resolves combat from the
same engine: `crystal_zone_player_combat_stats` populates `ZonePlayerCombatStats`
from `player_stats()`, so the zone rolls the real `Random(MinDC,MaxDC)` melee,
`Random(MinAC,MaxAC)`/`Random(MinMAC,MaxMAC)` armour, **and** the `CriticalRate`/
`CriticalDamage` crit (`zone_apply_player_critical`) — identical to the session
path. `crystal_player_zone_base_melee_damage` (the non-authoritative fallback +
range/magic base) is unified with the engine. This closes the session↔zone
combat divergence.

On the defensive side, player attack-magic against a monster now subtracts the
target's `Random(MinMAC,MaxMAC)` (`zone_magic_damage_after_monster_armour`),
mirroring the AC subtraction the physical path applies — so the ~404/555 monsters
with non-zero template MAC mitigate spell damage authoritatively. This covers the
single-target cast, secondary AoE targets, the FireBounce chain, and the
ground/AoE attack spells (FireWall/Blizzard/MeteorStrike/PoisonCloud/ExplosiveTrap)
per damage tick. The PoisonCloud poison DoT is applied separately and stays
unmitigated (poison is not reduced by MAC).

Remaining minor item: the physical `Random(Agility+1) > attacker.Accuracy` miss
check on monster melee. Monster accuracy is **not** present in the extracted data —
the `generate-crystal-respawn-manifest.mjs` extractor reads `Agility` but drops it
from the monster output and never reads `Accuracy`, so this needs a data-pipeline
change before it can fire. (Proximity-gating the lover/mentor experience bonus also
remains.)
