# Combat Numerics Engine — Crystal parity

Tracks bringing player damage resolution to 1:1 with Crystal
(`MapObject.GetAttackPower`/`GetArmour`/`GetDefencePower`, `MonsterObject.Attacked`).

## Crystal melee resolution (reference)
```
attack  = GetAttackPower(MinDC, MaxDC)      // Random(MinDC..=MaxDC), Luck biases to max/min
hit     = Random(Agility + 1) <= Accuracy   // ACAgility defence type (miss otherwise)
armour  = GetDefencePower(MinAC, MaxAC)      // Random(MinAC..=MaxAC)
attack += AttackBonus
if armour >= attack -> Miss (0)
if Random(100) < CriticalRate*5 -> attack += floor(attack * CriticalDamage/5)   // crit
net     = attack - armour                    // HP -= net
```
Weights (`Settings.cs`): MaxLuck 10, CriticalRateWeight 5, CriticalDamageWeight 50,
MagicResistWeight 10. Magic uses MAC/MinMAC..MaxMAC with a MagicResist check.

## Status

| Piece | Crystal | State |
|-------|---------|-------|
| Accuracy-vs-Agility hit roll | `GetArmour` ACAgility | ✅ present (`crystal_player_hit_roll_succeeds`) |
| **Critical hits** | `MonsterObject.Attacked` crit | ✅ **this work** — `crystal_apply_player_critical`: `CriticalRate*5`% chance, `floor(dmg*CriticalDamage/5)` bonus, emits `ObjectEffect{Critical}`. Gated by `CriticalRate` (0 on basic gear ⇒ inert), so zero regression; active with crit gear. |
| AC/MAC armour reduction | `GetArmour`/`GetDefencePower` | ❌ net damage does not yet subtract `Random(MinAC,MaxAC)` |
| MinDC–MaxDC attack range | `GetAttackPower` | ❌ player attack power is a single value, not a `Random(MinDC,MaxDC)` roll |
| Luck max/min bias | `GetAttackPower` | ❌ (needs the range above) |
| AttackBonus flat add | `Attacked` | ❌ (0 by default) |

## Remaining engine work (dedicated effort)
AC/MAC reduction + MinDC–MaxDC variance + Luck change the **damage curve**, so they
require:
1. A player base-stat derivation (MinDC/MaxDC/MinAC/MaxAC by class+level, like
   Crystal's `BaseStats`, on top of equipment stats) — today the melee value is a
   flat `18 + level/2 + equipment`.
2. Monster AC plumbed into damage resolution (the monster manifest already carries
   `min_ac`/`max_ac`/`min_mac`/`max_mac`).
3. Recalibrating the ~hundreds of exact-HP combat test assertions to the new curve
   (loop-to-death tests are tolerant; exact-HP ones are not).

This is a focused, high-churn project best done on its own branch with careful
test recalibration, rather than alongside other work — the crit slice here is the
zero-regression first step.
