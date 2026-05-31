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

## Remaining gap to numeric 1:1: the combat damage curve

The stat engine is semantically 1:1, but two combat numbers still diverge from
Crystal, both deliberately retained to keep the large existing combat-regression
suite green. Closing them is the scoped "combat number" phase:

1. **Melee floor.** `compute_player_stats` adds a non-Crystal `18 + level/2`
   floor to `Min/MaxDC`. Crystal has no floor — melee `MaxDC` is
   `class-base (≈0 at low level) + equipment`. Removing it requires importing
   Crystal-accurate starting-equipment stats (the seed weapons here use
   placeholder attack values) and re-baselining the damage assertions.

2. **Roll-based mitigation.** Monster→player damage currently subtracts a flat
   equipment-defence total. Crystal `HumanObject.Attacked` → `GetArmour` does:
   - physical (`ACAgility`): an agility/accuracy miss check
     (`Random(Agility+1) > attacker.Accuracy`) then `armour = Random(MinAC, MaxAC)`;
   - magic (`MACAgility`/`MAC`): the `MagicResist` miss check then
     `armour = Random(MinMAC, MaxMAC)`;
   - then `DamageReductionPercent`, `Reflect`, `EnergyShield`, `MagicShield`.

   The stat block already exposes the `Min/Max AC/MAC` and resist inputs these
   need; wiring the rolls + miss checks changes per-hit damage and so requires
   re-baselining the monster-combat tests.

Other minor items: full physical hit-roll base accuracy parity, and
proximity-gating the lover/mentor experience bonus (currently relationship-gated).
