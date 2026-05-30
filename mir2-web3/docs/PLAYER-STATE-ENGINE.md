# Player State Engine (人物状态系统) — Crystal Parity

This document describes the authoritative player stat engine added to the
simulation runtime and how it raises the **人物状态系统 (character state
system)** toward Crystal (`Mir2`/C#) parity.

Previously the player-state layer had a faithful *representation* (HP/MP,
equipment, buffs, persistence) but a heavily simplified *engine*: AC/MAC never
participated in mitigation through a real stat block, damage had no
`Min–Max` randomisation, MP was capped at a hard-coded `100`, equipment changes
did not recompute pools, and marriage/guild/mentor state was stored but inert.
This change introduces a single authoritative stat block that mirrors Crystal's
`PlayerObject.RefreshStats()` and wires it into combat, sustain, and
progression.

## Module: `apps/simulation/src/runtime/stats.rs`

`PlayerStats` is a Crystal-style stat block keyed by the Crystal `Stat` index
space (`crystal_compat::CRYSTAL_STAT_*`). It is assembled by
`compute_player_stats(world)` from three layers, exactly like Crystal:

1. **Base class/level** — vitals from the validated `crystal_base_vitals`
   (HP/MP), plus the per-class flat profile (accuracy, agility, and the
   bag/hand/wear weight capacities) derived from the real Crystal `BaseStats`
   formula table shipped in
   `packages/game-data/.../crystal_base_stats_packet_manifest.json`.
2. **Equipment** — every non-broken equipped item contributes its primary
   scalar (weapon attack → DC, armour defence → AC) plus all granular
   `added_stats` (Min/Max AC·MAC·DC·MC·SC, accuracy, agility, HP, MP, luck,
   attack speed, crit rate/damage, resistances, recovery, HP-drain, damage
   reduction, energy shield, the `*RatePercent` multipliers, …).
3. **Buffs** — active buff stats, with the historical
   `attack_bonus`/`defence_bonus` mapping onto `MaxDC`/`MaxAC`.

`*RatePercent` multipliers (`MaxDCRate`, `MaxMCRate`, `MaxSCRate`) are then
applied to the assembled totals.

### Compatibility contract

The engine is wired so that **seed/legacy behaviour is byte-identical**:

- For gear without explicit `Min*` stats the range collapses (`min == max`), so
  `Random(Min, Max)` deterministic rolls land on the historical flat number.
- Crit only triggers when `CriticalRate > 0`; resistances/recovery/weight are
  inert at `0`.
- The flat `18 + level/2` melee floor is preserved verbatim as the base `MaxDC`
  contribution.

This let the engine land without re-baselining the large existing combat suite.

## What now works (was simplified/inert before)

| Capability | Before | After |
|---|---|---|
| Aggregated stat block | none (ad-hoc totals) | `PlayerStats` over the full Crystal `Stat` space |
| Recompute on equip / unequip / enter-world / level | not recomputed | `refresh_player_stats()` (Crystal `RefreshStats`) reconciles pools |
| `max_mp` | not tracked; MP capped at hard-coded `100` | tracked on `PlayerVitals`, persisted, drives caps + bar |
| Melee damage | flat `18 + lvl/2 + atk` | `Random(MinDC, MaxDC)` + critical hits |
| Critical hits | none | `CriticalRate` roll → `CriticalDamage` amplification |
| Poison DOT on player | fixed 5/3 per tick | reduced by `PoisonResist` |
| Magic mitigation | — | `MagicResist` reduces incoming ranged/magic monster damage |
| Passive HP/MP regen | none | cadence-based regen, **paused in combat** + `Health/SpellRecovery` |
| Marriage / guild / mentor | stored, inert | grant a Crystal-style **experience-rate** bonus |
| Bag / hand / wear weight | inventory-only | computed in the stat block (per-class capacities) |

### `max_mp`

`PlayerVitals` gained a `max_mp` field (persisted in `CharacterSaveRecord` with a
`crystal_base_vitals` fallback for legacy saves). The previously hard-coded
`.min(100)` MP caps in potion/buff restore, resurrection, the mana bar percent,
and hero auto-pot now use the real pool.

### Regeneration safety

Passive regen pauses for `CRYSTAL_PLAYER_REGEN_COMBAT_DELAY_TICKS` after any
damage (tracked via `PlayerRuntimeResource::last_damaged_tick`), matching
Crystal's "no regen in combat" rule and ensuring combat tests are unaffected.

## Test coverage (`runtime::session::tests`)

New tests assert the engine end-to-end:

- `player_stats_seed_collapses_dc_range_to_legacy_melee`
- `player_stats_seed_max_mp_tracks_class_base_not_hardcoded_hundred`
- `mana_restore_caps_at_max_mp_not_legacy_hundred`
- `equipping_dc_range_weapon_produces_damage_spread`
- `equipping_hp_gear_raises_max_hp_and_unequip_restores`
- `critical_hit_amplifies_melee_when_crit_stats_present`
- `poison_resistance_reduces_player_poison_tick_damage`
- `magic_resistance_reduces_incoming_magic_damage`
- `passive_regen_restores_pools_after_combat_delay`
- `passive_regen_is_paused_immediately_after_taking_damage`
- `social_relationships_grant_experience_rate_bonus`
- `player_stats_expose_class_weight_capacities`

## Remaining gaps / future work

- **Per-element resistances** (fire/ice/lightning/wind/holy/dark/phantom): the
  block applies aggregate `MagicResist`; per-element tables are not yet modelled.
- **`MAC` on monster magic**: ranged monster strikes now also apply
  `MagicResist`, but they still go through the flat physical-defence path first;
  routing them through `MinMAC..MaxMAC` (and distinguishing physical ranged from
  magic) is a follow-up that requires re-baselining several monster tests.
- **Level-scaling base combat stats**: base `DC/MC/SC/AC/MAC` growth from the
  class table is intentionally omitted from combat to preserve existing numbers;
  it is sent to the client via the `BaseStats` packet for display.
- **Weight enforcement**: capacities are computed but not yet hard-enforced on
  pickup/equip.
- **Proximity-gated lover/mentor**: the experience bonus currently applies on
  relationship existence rather than spouse/mentor proximity.
