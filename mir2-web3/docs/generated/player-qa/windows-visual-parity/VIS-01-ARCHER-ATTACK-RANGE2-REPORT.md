# Windows visual parity VIS-01 Archer AttackRange2 report

Date: 2026-08-28

## Claim state

```text
implementation revision: 17b234911a44dd4df47d2e6d11270a5b7ca2370d
branch: codex/windows-visual-parity
archerAttackRange2AutomatedCheckpoint: complete
archerRangedActionDenominatorComplete: false
sameSceneAnimationCaptureProduced: false
humanAnimationFeelAccepted: false
globalParityPercent: null
accepted: false
visualAccepted: false
```

This report closes the bounded native ability to select and render Crystal's
second Archer ranged action for the spell branches whose source rule is
unconditional. It does not claim complete Archer or ranged-combat parity.

## Crystal source binding

`Crystal/Client/MirObjects/Frames.cs` defines:

- `AttackRange1 = Frame(96, 8, 0, 100)`
- `AttackRange2 = Frame(160, 8, 0, 100)`

`Crystal/Client/MirObjects/PlayerObject.cs` selects the Archer alternate body,
hair and weapon libraries for bow movement/ranged actions. Twelve spell
branches unconditionally select AttackRange2:

1. StraightShot
2. DoubleShot
3. DelayedExplosion
4. BindingShot
5. VampireShot
6. PoisonShot
7. CrippleShot
8. NapalmShot
9. SummonVampire
10. SummonToad
11. SummonSnakes
12. Stonetrap

ElementalShot is deliberately excluded: Crystal selects AttackRange2 only when
`HasElements` is true and `ElementCasted` is false, but those two facts are not
currently exposed on the typed native presentation boundary.

## Implemented behavior

- Runtime defines `AnimationAction::AttackRange2` with the exact
  `160, 8, 0, 100` player descriptor.
- Native parsers and the animation bridge round-trip `attackRange2` rather
  than collapsing it to AttackRange1.
- Archer AttackRange2 selects `ARArmour`, `ARHair` and `ARWeapon` layers; a
  return to Standing selects the common libraries again.
- Typed ObjectMagic spell names select AttackRange2 only for the exact
  12-member unconditional set and fail closed to the existing Spell action for
  unknown or conditional cases.
- Frame-set fallbacks retain AttackRange1/Attack1/Standing so an incomplete
  library fails visually soft without inventing a protocol fact.

## Automated evidence

| Gate | Result |
|---|---|
| Exact 12-spell action table and fail-closed cases | PASS |
| ObjectMagic native action projection | PASS |
| Runtime parser/round-trip and exact frame descriptor | PASS |
| Archer alternate layer selection and Standing reset | PASS |
| Full runtime suite | PASS, 192/192 |
| Full Windows suite | PASS, 436/436 |

## Native boot boundary

The exact implementation revision produced the 138,914,304-byte EXE with
SHA-256
`ED6C1BB4F9D5EB4F501201C361EE3437DF7CB8EB2B192B3F2F55AA63A7871037`.
It booted and connected to local `ws://127.0.0.1:7110/ws`, but no real Archer
cast, same-scene animation capture or human feel acceptance was produced.

## Explicitly open gates

ElementalShot's conditional branch, `WalkingBow`, `RunningBow`,
`AttackRange3`, projectile/hit coupling for the wider ranged-skill set, the
complete class/player animation denominator, authenticated live WSS, real DPI,
30-minute native soak, human visual/feel acceptance and formal publisher
signing remain open. `globalParityPercent=null` remains mandatory.
