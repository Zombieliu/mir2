# Windows visual parity VIS-02 Lightning report

Date: 2026-08-27

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 82d55b39b4ac5ba47cf80e92128c032cea57edb3
Lightning implementation revision: 53483ccf4a8f63b9fe2ebc6d4b415105fa9f9e1a
evidence/gate revision: e2235503b2ca235e02e8174bda8b95f0130d71b3
branch: codex/windows-visual-parity
vis02Status: in_progress
lightningAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
```

This report closes one bounded automated checkpoint inside VIS-02. Lightning
is only one of the first five spells; FlamingSword, FireBall, SoulFireBall and
FireWall are not closed by this report. No packaged executable, live-WSS
playback, GPU raster capture or human audio acceptance was produced, so this
is not a full VIS-02, skill-VFX, Windows-visual or whole-game parity claim.

## Source-bound behavior implemented

- Typed Gateway `ObjectMagic` events preserve Lightning's caster, location,
  direction, target, `cast`, level and broadcast fields. A fixed fixture checks
  the exact `cast=true` and `cast=false` packet projections.
- Crystal finishes the caster's six-frame `Spell` actor action before creating
  Lightning. Native Lightning therefore begins at the exact 600 ms completion
  boundary. `cast=false` retains the actor-action packet path but creates no
  Lightning effect or audio.
- The owner-attached effect uses `Magic` frames
  `970 + direction * 20 .. +5`: eight directions, six frames, 100 ms per
  frame, 600 ms total. It follows the caster's latest authoritative Zone tile;
  a departed caster cancels the pending presentation and sound.
- Lightning has no fabricated projectile or impact phase. The existing shared
  Zone authority continues to schedule its six-tile gameplay scan; this client
  checkpoint does not alter or broaden server combat claims.
- The exact Crystal clip `M40-0.wav` is queued once when the actor action
  completes. It is fail-closed behind an internal file allowlist and
  generation/sequence/cue dedupe. Disabling sound or setting volume to zero
  drops pending playback and cannot replay it later.
- Map change, logout, reconnect/reset and object departure clear pending
  anchors/audio. Toggling the local Effect option hides only presentation: the
  deterministic clock continues and audio is not replayed when the option is
  restored.
- Packaging and copied-Candidate verification now require the exact 247,772
  byte `M40-0.wav` with SHA-256
  `05E08C3AA3ADF166A3FDF9462279024898217F4F936BBD28A1FB6EA75BF92A4E`.
  Arbitrary WAV paths remain rejected.
- The Windows functional gate now generates both the ordinary map atlas and
  the native keyed/additive map pack. This preserves the VIS-01 real `0.map`
  front-cell binding instead of weakening its assertion when a clean runner
  lacks ignored generated assets.

## Automated evidence

| Gate | Result |
|---|---|
| Independent read-only Lightning review | PASS; no P0/P1 |
| Gateway exact typed VIS-02 fixture | PASS, 1/1 |
| Native Lightning lifecycle/frame/audio tests | PASS, 5/5 |
| Bevy gameplay-audio focused tests | PASS, 2/2 |
| Full `mir2-client-bevy` native-ui suite | PASS, 389/389 |
| Full Windows native suite with fresh source map + keyed packs | PASS, 333/333 |
| VIS-01 real-front-cell focused reproduction after keyed-pack generation | PASS, 1/1 |
| Windows vertical-slice gate contract self-test | PASS, 9 fixed controls |
| Candidate package script self-test | PASS; ADS and reparse probes pass |
| Candidate verifier self-test | PASS; ADS probe passes |
| Candidate supply-chain static test | PASS; 15 immutable actions |
| Rustfmt and diff checks | PASS |

The 333-pass Windows run used freshly generated ignored assets from the current
source tree. It is automated source-root evidence, not an exact-head Candidate
package or a same-EXE acceptance run. The frozen playable Candidate processes
were not stopped, replaced, launched or used as evidence for this revision.

## Open VIS-02 and final gates

VIS-02 remains in progress until FlamingSword, FireBall, SoulFireBall and
FireWall have equally source-bound cast/projectile/impact/persistence/audio
semantics and the complete first-slice actor Struck/Die/Dead/Revive chain is
covered. Lightning itself still needs exact-head same-EXE playback through an
authenticated live WebSocket, GPU additive/alpha raster evidence and human
animation/audio/feel review.

The complete semantic denominator and legal asset pack, clean Crystal source
binding, real 100/125/150% DPI, the full UI/live-WSS route, a 30-minute native
soak, formal publisher signing and whole-game human acceptance remain open.
Therefore `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false` remain mandatory.
