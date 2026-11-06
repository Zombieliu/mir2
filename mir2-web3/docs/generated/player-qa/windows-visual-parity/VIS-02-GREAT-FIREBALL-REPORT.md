# Windows visual parity VIS-02 GreatFireBall report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: eb422e47b46c115ab9f1905a05470bf7b534c178
GreatFireBall implementation revision: 9457e5618449d22350baedd01e3775f5b1fe59c6
branch: codex/windows-visual-parity
vis02Status: in_progress
greatFireBallAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes one additional bounded Windows-native GreatFireBall
automation checkpoint. It is outside the original first-five spell list and
does not close VIS-02. No executable was launched, no exact-head package was
created, and no authenticated live-WSS, GPU raster or human animation/audio
evidence was produced. It is not a UI, Windows-visual or whole-game 100%
claim.

## Source-bound behavior implemented

- Typed `ObjectMagic(spell=GreatFireBall)` immediately starts the ten source
  cast frames `Magic/400..409` at 60 ms/frame and queues exact `M34-0.wav`.
- A successful `cast=true` waits for the 600 ms Crystal Spell-action boundary,
  then creates the local client-owned projectile and queues exact
  `M34-1.wav`. The Rust `ObjectProjectile` emitted as a compatibility
  supplement is ignored in every replay order so it cannot double draw.
- The projectile locks one of sixteen Crystal directions at launch and uses
  six frames from `Magic/(410 + direction*10)..+5` at the source 30 ms frame
  interval. Its movement lifetime remains finite at `distance*50 ms`; frame
  cycling does not extend the flight.
- A still-bound target promotes at arrival to ten target frames
  `Magic/570..579` at 60 ms/frame and queues exact `M34-2.wav`. Target removal
  converts the flight to a point destination and suppresses both the invented
  impact and impact sound. Map/session lifecycle clears all phases and pending
  audio.
- The fixture labels `cast=false` as `compatibilityOnly`; that input retains
  only the immediate cast and M34-0. It is not asserted as Crystal's current
  production GreatFireBall server path.
- The exact audio identities are:
  - `M34-0.wav`: 430,124 bytes, SHA-256
    `0F25BB7CD8556726C8758C48CBF0BD2D1D3D4C205BE36C6CAE39251DE9D3068B`;
  - `M34-1.wav`: 319,532 bytes, SHA-256
    `895C3855F35BB8BA543B2717F682617A85BBE5A6EA15170D1D5EB4196914429C`;
  - `M34-2.wav`: 229,420 bytes, SHA-256
    `4482367380FFF4EDB7E1CD605ADD6EAD984B45497B254ECB3941AB6D6CC0DBAB`.

## Clean-checkout asset and package closure

GreatFireBall requires 116 distinct source frames: ten cast, sixteen times six
projectile, and ten impact frames. Direction zero and the cast/impact frames
were already tracked, but directions 1 through 15 were not. This revision
therefore adds exactly 90 PNGs (`420..425`, `430..435`, through `560..565`),
their exact `Magic/meta.json` dimensions/offsets, and Direction16 ranges in
`effects.generated.json`.

The three M34 WAVs were intentionally ignored by the repository's broad Sound
rule and were force-added by exact path. Candidate packaging and verification
now both:

- allowlist and require M34-0/M34-1/M34-2;
- require every one of the 116 GreatFireBall frames;
- copy the exact source WAVs without following reparse points;
- verify each WAV byte length and SHA-256;
- fail closed in self-tests when M34-2 or direction-15 frame 565 is removed.

No exact-head Candidate was staged. These results prove source and script
closure, not a signed packaged executable.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal/source behavior audit | PASS |
| Independent final implementation and package review | PASS; P0=0, P1=0 after asset-closure remediation |
| Gateway packet-event projection fixture | PASS, 1/1 |
| Focused GreatFireBall native effects | PASS, 5/5 |
| GreatFireBall 16-direction frame and audio integrity | PASS |
| Full Windows native suite | PASS, 372/372 |
| Full `mir2-client-bevy` native-ui suite | PASS, 393/393 |
| Gameplay audio allowlist/queue regression | PASS, 1/1 |
| Magic-effect exporter/validator | PASS, 74 spells |
| Sound exporter | PASS |
| Web TypeScript typecheck and full frontend logic | PASS |
| Full offline Web resource/audio gate | PASS |
| Candidate package script self-test | PASS; ADS/reparse probes pass |
| Candidate verifier self-test | PASS; missing GreatFireBall audio/frame fail closed |
| Rustfmt and diff checks | PASS |

The Web checks validate the shared exporter/resource contract and regressions;
this checkpoint does not claim a newly certified Web GreatFireBall renderer.

## Open gates

Crystal suppresses impact when a target remains present but its current action
is already Dead. The native effect input currently exposes target presence and
position, not an explicit authoritative dead action, so that branch remains
open rather than being inferred. Authenticated live-WSS timing must also prove
the real successful cast and server damage sequence.

GreatFireBall still needs exact-head same-EXE playback, GPU blend/layer pixels,
audio channel/volume review and human animation/feel acceptance. VIS-02 still
requires its same-EXE capture and the wider skill/combat interaction matrix.
The full player, monster, effect, environment and UI denominator remains
incomplete, together with clean Crystal source binding, real 100/125/150% DPI,
a 30-minute native soak, formal publisher signing and whole-game human
acceptance. Therefore `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false` remain mandatory.
