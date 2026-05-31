# Combat Engine Parity

Status of the Rust simulation's combat math relative to the reference Crystal
server (`Crystal/Server/MirObjects/{MapObject,HumanObject,MonsterObject}.cs`).

## What is now Crystal-faithful

The damage pipeline is ported as a pure, deterministic module,
`apps/simulation/src/runtime/combat_engine.rs`, and exercised by 21 unit tests
plus end-to-end integration tests.

- **Attack power** — `GetAttackPower(min, max)` rolls a uniform value in
  `[min, max]` with luck pulling to the maximum (`Luck > rand(MaxLuck)`) or, for
  negative luck, to the minimum. Replaces the old flat
  `18 + level/2 + equipment` melee value.
- **Armour** — `GetDefencePower(min, max)` plus `GetArmour(DefenceType, …)`,
  including the agility dodge (`rand(Agility+1) > Accuracy`) and magic resist
  (`rand(MagicResistWeight) < MagicResist`).
- **`Attacked` pipeline** — `armour*ArmourRate`, `damage*DamageRate`,
  `+ AttackBonus`, reflect (`rand(100) < Reflect`), `DamageReductionPercent`,
  the `armour >= damage` block, and critical hits
  (`rand(100) < CriticalRate*5`, bonus `floor(damage * (CriticalDamage/50) * 10)`).
- **Stats** — players resolve `MinDC/MaxDC … Accuracy/Agility/Luck/CriticalRate
  …` from per-class level-scaled base stats (`Shared/BaseStats.cs`) + equipment
  + buffs, normalised by `RefreshStatCaps`. Monsters resolve from the Crystal
  monster manifest; the agility dodge uses the imported per-monster value.
- **Poison rate modifiers** — Red poison `ArmourRate -= 0.10`, Stun
  `DamageRate += 0.20`, applied to both player and monster victims.
- **On-hit gear procs** — `ApplyNegativeEffects` rolls Freezing→Slow and
  PoisonAttack→Green against the level offset.
- **Weapon durability** — 1 point per landed swing, matching `DamageWeapon`.

Both melee and physical ranged player attacks resolve through this pipeline.
Incoming monster damage is reduced by the player's physical defence (AC), or the
magic armour (MAC) for magic/spell blows, matching Crystal's `DefenceType`
routing; it can also miss via the agility dodge (physical) or magic resist
(magic) from `GetArmour`. Vampiric gear (`HPDrainRatePercent`) heals the player a
fraction of the damage dealt.

## Deterministic rolling

The simulation is replayable, so every `Envir.Random` draw is replaced by a
splitmix64-mixed roll keyed on `(tick, salt, purpose)`. Distinct purposes keep
the draws inside one attack decorrelated, and the mixer avalanches so damage
varies hit-to-hit (the project-wide `deterministic_roll` collapses for the small
moduli combat uses).

## Known modelling limitations / follow-ups

- **Item Min stats.** Equipment exposes a single attack/defence figure per slot
  (treated as the Max stat); when an item carries no explicit Min stat the Min
  is taken to equal the Max (close to real Crystal gear, and far better than a
  `[0, Max]` spread). Wiring true per-item `MinDC/MaxDC` from the Crystal item
  manifest would restore weapon damage variance.
- **Monster accuracy.** The generated monster manifest does not export the
  `Accuracy` stat, so monster blows use a fixed accuracy floor (high enough that
  base-agility players are not falsely dodging).
- **Incoming armour is flat, not rolled.** Incoming monster damage subtracts the
  full AC/MAC rather than a `GetDefencePower(0, MaxAC)` roll. Rolling only the
  armour (while the monster damage stays a flat max instead of a
  `GetAttackPower(MinDC,MaxDC)` roll) roughly doubles effective damage taken and
  unbalances early fights, so the faithful version needs the paired monster-side
  damage roll first.
- **Incoming reflect.** The player's `Reflect` stat is honoured by the
  victim-side resolver (`resolve_attack_on_player`) but not by the monster attack
  scheduler, which lacks the attacker entity needed to bounce damage back.
- **Skill / magic damage** still uses the legacy direct-damage path (no armour
  subtraction); migrating it onto the pipeline (with the `MAC`/`MACAgility`
  defence types) is part of the same step.
- **Monster Slow/Paralysis movement.** The Slow status flag is applied and
  broadcast, but the monster AI does not yet act on it.
- **Zone (shared-world) path** resolves damage with its own simplified math and
  has not yet been moved onto the engine.
