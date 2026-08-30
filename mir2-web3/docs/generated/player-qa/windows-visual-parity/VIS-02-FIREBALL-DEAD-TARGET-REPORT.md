# Windows visual parity VIS-02 FireBall-family Dead-target report

Date: 2026-08-28

## Claim state

```text
implementation revision: 8d8c5f12f6faa4617ce87017f82738458f164bd9
branch: codex/windows-visual-parity
vis02Status: in_progress
fireBallFamilyDeadTargetAutomatedCheckpoint: complete
webEquivalentCheckpointComplete: false
skillEffectDenominatorComplete: false
semanticLeafInventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
realDpiEvidenceProduced: false
nativeThirtyMinuteSoakProduced: false
humanVisualAudioFeelAccepted: false
formalPublisherSigningComplete: false
```

This report closes only the Windows native projectile-completion action gate
for FireBall, GreatFireBall and SoulFireBall. It does not close spell damage,
hit authority, the Web renderer equivalent, other missile spells, VIS-02 or
the skill/effect denominator.

## Crystal source binding

`Crystal/Client/MirObjects/PlayerObject.cs` creates each of these missiles at
the Spell action boundary and registers a completion callback:

- FireBall checks `missile.Target.CurrentAction == MirAction.Dead` before
  creating `Magic/170..179` and playing the spell's `+2` impact sound.
- GreatFireBall uses the same gate before `Magic/570..579` and its `+2` sound.
- SoulFireBall uses the same gate before `Magic/1360..1369` and its `+2` sound.

The comparison is against the current `MirAction`, not a life-state boolean.
Therefore `dead=true` during the visible `Die` action must not suppress the
impact. Only the terminal `Dead` pose suppresses it.

## Implemented behavior

- `NativeEntityPresentation` exposes only object ids whose shared Crystal
  animation clock is currently in `AnimationAction::Dead`.
- The existing chained Bevy schedule advances entity presentation before
  native effects, so projectile completion sees the same-frame action state.
- FireBall, GreatFireBall and SoulFireBall retain their existing cast,
  projectile, target-following, distance-clock and removal behavior.
- At the exact projectile completion boundary, a bound target in `Dead`
  removes both the queued impact animation and its pending impact sound.
- `Die` retains both phases. A target that is `Dead` during flight but Revive
  completes before arrival also retains the impact, matching the completion-
  time callback instead of an early snapshot decision.
- No damage, hit, packet, Gateway, simulation or Zone authority changed.

## Automated evidence

| Gate | Result |
|---|---|
| Shared presentation clock distinguishes `dead=true + Die` from terminal `Dead` | PASS |
| FireBall/GreatFireBall/SoulFireBall `Die` impact bitmap and audio | PASS, 3/3 |
| FireBall/GreatFireBall/SoulFireBall `Dead` suppression of bitmap and audio | PASS, 3/3 |
| Revive-before-arrival completion behavior | PASS |
| Full `mir2-platform-windows` suite | PASS, 410/410 |
| Independent exact-diff review | PASS, P0=0/P1=0; P2 only additional spell-specific revive samples |

## Explicitly open gates

Crystal contains the same `CurrentAction == Dead` pattern for additional
missile spells; they are not covered by this checkpoint. The Web projectile
renderer has a separate death-phase gap and was not changed here. No EXE was
built or launched for this revision, and no same-EXE screenshot, authenticated
live-WSS transcript, real-DPI evidence, 30-minute native soak, human visual/
audio/feel acceptance, complete skill/effect denominator or formal publisher
signature was produced. Therefore `globalParityPercent=null`,
`accepted=false` and `visualAccepted=false` remain mandatory.
