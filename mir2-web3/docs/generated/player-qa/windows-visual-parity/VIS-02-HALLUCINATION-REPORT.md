# Windows visual parity VIS-02 Hallucination report

Date: 2026-08-28

## Claim state

```text
implementation revision: 60eae9561c5b18bc79456105e455d6964c14fafe
branch: codex/windows-visual-parity
vis02Status: in_progress
hallucinationAutomatedCheckpoint: complete
skillEffectDenominatorComplete: false
semanticLeafInventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
roundHeadDebugExeLaunched: false
authenticatedLiveWssTranscriptProduced: false
realDpiEvidenceProduced: false
nativeThirtyMinuteSoakProduced: false
humanVisualAudioFeelAccepted: false
formalPublisherSigningComplete: false
```

This report closes one native Windows Hallucination projectile/impact/audio
projection and its fail-closed Candidate asset closure. It changes no cast,
hit, damage, status, Gateway, simulation or Zone authority and does not close
VIS-02.

## Crystal source binding

`Crystal/Client/MirObjects/PlayerObject.cs` binds spell id 76 to:

- no cast bitmap and no cast sound;
- projectile `Magic/1160`, three frames, 48ms process cadence and direction
  stride 10, created after the 600ms Spell action;
- a completion callback only when the target exists;
- terminal `Dead` returning before impact;
- otherwise target impact `Magic2/1110..1119` over 1000ms and sound id 20760,
  resolved exactly as `M76-0.wav`.

The rotatable source range is the 16-direction bank
`Magic/1160..1312`; the impact range is `Magic2/1110..1119`.

## Implemented behavior

- `ObjectMagic(cast=true)` schedules the client-owned missile at the exact
  600ms action boundary without adding cast art or audio.
- The projectile locks its launch direction, follows the target for distance
  clock updates, uses Crystal's 50ms-per-tile duration and derives its frame
  cadence from the 48ms process interval.
- A missing target stays a point flight and never invents impact or sound.
- `Die` retains impact; only terminal `Dead` at completion suppresses impact
  and its sole `Hallucination.impact` audio cue. Revive before completion
  restores them.
- Compatibility `ObjectProjectile` packets are ignored in either order, and
  map change, logout and session reset clear all delayed state.
- The exact Crystal WAV and every projectile/impact frame are exported,
  packaged and identity-required by the Windows Candidate verifier.

## Automated evidence

| Gate | Result |
|---|---|
| Focused Hallucination lifecycle tests | PASS, 5/5 |
| Magic-effect exporter test | PASS, 74 spells |
| Sound exporter test | PASS |
| Audio-system test | PASS |
| Web asset-release preflight | PASS, sounds 342/342 |
| Package and verifier strict self-tests | PASS |
| Full combined Windows suite | PASS, 421/421 |
| Independent exact-source review | PASS, P0=0, P1=0, P2=0 |

Sound identity:

- `M76-0.wav`: 445740 bytes,
  `4EB43491B7360A8B55A5565DC19C98542DEB36EBE10F101CD2C37473DC825744`

## Explicitly open gates

No EXE from revision `60eae9561c5b18bc79456105e455d6964c14fafe` was
launched or captured. The concurrently running local debug client predates
this revision and is not evidence for this leaf. Same-EXE UI/live WSS, real
DPI, 30-minute native soak, physical audio, human visual/audio/feel acceptance,
the complete 129-spell semantic denominator, clean-source rebinding and formal
publisher signing remain open. `globalParityPercent=null`, `accepted=false`
and `visualAccepted=false` remain mandatory.
