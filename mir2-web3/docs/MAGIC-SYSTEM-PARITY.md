# Magic / Skill System — Crystal Parity Notes

Status snapshot for the player magic system (`apps/simulation/src/runtime/skills.rs`)
measured against the Crystal C# server (`Crystal/Server/MirObjects/HumanObject.cs`,
`Crystal/Server/MirDatabase/MagicInfo.cs`, `Crystal/Server/MirObjects/MapObject.cs`).

## Core damage formula (the main fidelity fix)

Crystal computes spell damage as:

```
GetDamage(damageBase) = (int)((damageBase + GetPower()) * GetMultiplier())
GetMultiplier()       = MultiplierBase + Level * MultiplierBonus
GetPower()            = round(MPower()/4 * (Level+1) + DefPower())
MPower()              = MPowerBonus>0 ? Random(MPowerBase, MPowerBase+MPowerBonus) : MPowerBase
DefPower()            = PowerBonus>0  ? Random(PowerBase,  PowerBase+PowerBonus)   : PowerBase
GetAttackPower(min,max) = Luck-biased Random(min, max)   // MapObject
```

`damageBase` is the player's **rolled attack power** from a class-specific stat
channel:

| Channel | Stat            | Spells                                                        |
|---------|-----------------|--------------------------------------------------------------|
| MC      | Min/MaxMC       | Wizard spells + Archer ranged shots                          |
| SC      | Min/MaxSC       | Taoist spells (Healing, SoulFireBall, Poisoning, Curse, …)   |
| DC      | Min/MaxDC       | Warrior + Assassin melee skills                              |

### What was wrong before

The legacy code added the player's stat **after** the multiplier
(`crystal_magic_damage(magic, level) + max(MinXX, MaxXX)`) and always used the
**max** stat value, so:

- High-level spells did not scale the player's gear contribution (the multiplier
  never touched it), badly understating end-game damage.
- There was no `Min..Max` randomness (Crystal rolls every cast).

### What it does now

`skills.rs` exposes the faithful primitives:

- `crystal_magic_get_power` / `crystal_magic_multiplier` / `crystal_magic_get_damage`
  — exact `GetPower` / `GetMultiplier` / `GetDamage`.
- `crystal_attack_power_roll` — deterministic `Random(min,max)` honoring Luck,
  seeded through the shared `deterministic_roll` RNG so casts stay replayable.
- `crystal_spell_damage_channel` — the MC/SC/DC table above.
- `crystal_spell_damage` — `GetDamage(GetAttackPower(channel))` for a cast.
- `crystal_spell_range_damage` — Crystal `GetRangeAttackPower` bow falloff for
  archer shots (`min -= floor(min/MaxAttackRange * (MaxAttackRange-range))`).
- `crystal_spell_damage_with_crit` — melee critical hit that doubles the rolled
  attack power before `GetDamage` (1+Luck for IceThrust/BladeAvalanche, Accuracy
  for CrescentSlash), matching Crystal's `damageBase += damageBase`.

Every damage/heal handler now routes through these instead of the old
add-after-multiplier path. Special bases preserved from Crystal:

- `ElementalShot`: `GetDamage(GetAttackPower(MC) + orbPower)`.
- `Healing`:       `GetDamage(GetAttackPower(SC) * 2) + Level`.
- `ThunderBolt` ×1.5 vs undead, `FlameDisruptor` ×1.5 vs living.
- `Vampirism` heals the caster from damage dealt.
- AoE falloff: `MeteorShower` secondary ×0.5, `BladeAvalanche` rear ×0.6,
  `IceThrust` far ×0.6.

## Coverage

- ~110 player-castable spells; all route to a dedicated handler
  (`apply_manifest_spell_effect` arms, `cast_summon_skill`, or
  `SkillEffectTemplate`). No offensive spell falls through to a bare,
  channel-less damage path — the generic fallback also uses `crystal_spell_damage`.
- Summons implemented: Skeleton, HolyDeva, Shinsu, Vampire, Toad, Snakes, Stonetrap
  (lifetimes/amulet costs per Crystal; `SummonShinsu` now consumes 5 amulets).
- `FastMove` correctly treated as a passive (Crystal has no server cast handler).

## Validation

- Pure-formula unit tests: `crystal_damage_formula_tests` (GetPower/Multiplier/
  GetDamage folding + per-spell channel table) — green.
- End-to-end magic tests that exercise the refactored damage/heal path pass
  (`flash_dash`, `storm_escape`, `one_with_nature`, `mass_healing`, …).
- The change is behavior-neutral on the existing passing magic suite; the
  pre-existing targeted-spell test failures (target-id/timing setup in the
  single-session harness) are unchanged and unrelated — see the note in
  `CRYSTAL-SERVER-PARITY.md` about the suite's existing skill-effect failures.

## Known cross-module dependency (out of magic scope)

Magic damage **values** are now Crystal-faithful, but the shared combat
damage-application path (`combat.rs::schedule_damage_to_monster`) does not yet
subtract the target's armor. Crystal applies `Attacked(value, DefenceType.MAC|AC)`,
rolling the defender's Min/Max MAC (magic) or AC (physical). Reaching 1:1
*effective* damage requires threading a `DefenceType` through the combat
resolution and rolling defender armor — a combat-engine change that affects all
damage types, tracked separately from the magic-cast system.
