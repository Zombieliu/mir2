# Windows visual parity VIS-02 FrostCrunch report

Date: 2026-08-28

## Claim state

```text
implementation revision: 473a56137c7af458d5c982c90f3d4a658a9243fd
branch: codex/windows-visual-parity
vis02Status: in_progress
frostCrunchAutomatedCheckpoint: complete
skillEffectDenominatorComplete: false
semanticLeafInventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
roundHeadDebugExeLaunched: true
localWsGameplayReached: true
authenticatedLiveWssTranscriptProduced: false
realDpiEvidenceProduced: false
nativeThirtyMinuteSoakProduced: false
humanVisualAudioFeelAccepted: false
formalPublisherSigningComplete: false
```

This report closes the native Windows FrostCrunch cast/projectile/impact/audio
projection and Candidate asset closure. It changes no hit, damage, freeze,
Gateway, simulation or Zone authority and does not close VIS-02.

## Crystal source binding

`Crystal/Client/MirObjects/PlayerObject.cs` binds FrostCrunch to:

- spell cast `Magic2/400..409` and `M41-1.wav`;
- a projectile created after the 600ms Spell action from
  `Magic2/410..413`, with four 30ms source frames and flight timing;
- a completion callback that returns only when the target's current action is
  terminal `Dead`;
- otherwise target impact `Magic2/570..577` over 600ms and `M41-2.wav`.

The projectile is four rotatable frames, not a fabricated 16-direction frame
bank. The Candidate closure therefore requires exactly `400..413` and
`570..577` from `Magic2`.

## Implemented behavior

- `ObjectMagic(cast=true)` owns the cast sound and delayed local projectile.
- Adjacent compatibility `ObjectProjectile` is deduplicated.
- Projectile launch binds the current target position, tracks the target for
  distance-clock updates, and never invents an impact after target removal.
- `Die` retains impact bitmap/audio; only terminal `Dead` at completion
  suppresses both. Revive-before-completion restores both.
- Map change, logout and session reset clear all delayed phases and sounds.
- Exact Crystal source WAVs are committed with size/SHA-256 identity, exported
  as ids 20411/20412, copied by Candidate packaging and required by verifier.

## Automated evidence

| Gate | Result |
|---|---|
| Focused FrostCrunch lifecycle tests | PASS, 4/4 |
| FireBall-family regression after scheduler generalization | PASS, 24/24 |
| Sound export end-to-end | PASS |
| Package and verifier strict self-tests | PASS |
| Exact source frame closure `Magic2/400..413,570..577` | PASS |
| Full combined Windows suite | PASS, 416/416 |
| Independent review | PASS, P0=0; no semantic P1 |

Sound identities:

- `M41-1.wav`: 162330 bytes,
  `E33DDD8E7C1FFD7614BCAA6220EC5886813F5FFECDFF4B4AF486D054FCD47051`
- `M41-2.wav`: 132140 bytes,
  `417C4FB71C7883918D5CFE7AFF0B6A70A6F832BA17AAB1DE144E96B2A7D54310`

## Explicitly open gates

An exact-head debug EXE was built from a clean detached worktree and reached
gameplay through local `ws://127.0.0.1:7110/ws` on 2026-08-28. It used the
complete local generated asset root, but it was not an attested Candidate
package and produced no archived same-EXE capture or authenticated WSS
transcript. Real DPI, 30-minute native soak, physical audio, human visual/
audio/feel acceptance, the remaining spell denominator and formal publisher
signing remain open. `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false` remain mandatory.
