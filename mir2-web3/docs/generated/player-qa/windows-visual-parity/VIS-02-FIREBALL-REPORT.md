# Windows visual parity VIS-02 FireBall report

Date: 2026-08-27

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 725a2d6de6324e39b3699e6cd3d82e17be8165cc
FireBall implementation revision: d85d7368119053e6b2609316c4f5c76faaa298cb
branch: codex/windows-visual-parity
vis02Status: in_progress
fireBallAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
```

This report closes one bounded automated FireBall checkpoint inside VIS-02.
Together with Lightning, this is two of the first five spells. FlamingSword,
SoulFireBall and FireWall remain open. No exact-head packaged executable,
authenticated live-WSS playback, GPU raster capture or human animation/audio
acceptance was produced, so this is not a full VIS-02, Windows-visual or
whole-game parity claim.

## Source-bound behavior implemented

- Typed Gateway evidence fixes one `cast=true` and one `cast=false`
  `ObjectMagic` path plus the Rust simulation's adjacent compatibility
  `ObjectProjectile`. The native client creates FireBall's missile from
  `ObjectMagic` after the Spell action, as Crystal does, and consumes the
  compatibility packet instead of drawing a duplicate.
- The cast phase starts immediately with `Magic/0..9`, lasts 600 ms and emits
  exact `M31-0.wav`. `cast=false` retains that cast presentation but creates
  no projectile, impact or phase audio after it.
- At the 600 ms launch boundary, the missile locks Crystal
  `MapControl.Direction16` from the authoritative target location. It uses six
  frames at `Magic/(10 + direction * 10)..+5`; all 16 directions are exported
  and required by package/verify. Later target movement updates the
  destination and `MaxDistance * 50 ms` flight clock without changing the
  launch direction.
- The missile cycles the six visible frames during its finite movement count.
  Frame cycling is separate from lifecycle repeat: a point-target missile
  without a bound object still expires exactly at the end of its flight and
  cannot leak an active effect or light.
- A target-bound missile attaches `Magic/170..179` for the 600 ms impact and
  emits `M31-2.wav`; launch emits `M31-1.wav`. A point target without a live
  bound object gets the projectile but no invented impact. Map change,
  logout, reconnect/reset and object departure clear retained effects and
  pending audio.
- The exact audio identities are:
  - `M31-0.wav`: 364,024 bytes,
    SHA-256 `98C28FC920A35FE3C134607811760E4C49200239C2E3B9CCAE36B42EE083AA3E`;
  - `M31-1.wav`: 364,028 bytes,
    SHA-256 `FCC49A68343DB3E910A3A35F12CEA227CBEA058E199D048236A8D99831005A15`;
  - `M31-2.wav`: 128,908 bytes,
    SHA-256 `8732FD9131E228712071AABFED542618B9D1D6F269D748EC9857ECBFA4E59B05`.
- The present-sound manifest now includes these clips and the earlier
  Lightning `M40-0.wav`. This closes the prior CI resource-gate drift instead
  of weakening the offline release preflight.

## Automated evidence

| Gate | Result |
|---|---|
| Independent read-only review after lifecycle/direction remediation | PASS; no P0/P1 |
| Gateway exact typed FireBall fixture | PASS, 1/1 |
| Native effects suite | PASS, 59/59 |
| FireBall 16-direction PNG and three-audio identity closure | PASS |
| Magic-effect exporter/validator | PASS, 73 spells |
| Full `mir2-client-bevy` native-ui suite | PASS, 393/393 |
| Full Windows native suite | PASS, 340/340 |
| Web typecheck | PASS |
| Full offline Web resource/audio gate | PASS |
| Candidate package script self-test | PASS; ADS and reparse probes pass |
| Candidate verifier self-test | PASS; missing FireBall frame/audio fail closed |
| Rustfmt and diff checks | PASS |

The tests used source assets in the isolated visual-parity worktree. The
frozen playable Candidate processes were not stopped, replaced, launched or
used as evidence for this revision.

## Open VIS-02 and final gates

Crystal suppresses FireBall impact and `M31-2` when the still-present target's
action is already Dead at missile completion. The current native effect input
contains target tiles/removal but not an explicit target-dead bit, so that
fine-grained branch remains open rather than being guessed in the renderer.
It must be closed when authoritative dead state is added to the effect input.

VIS-02 also remains in progress for FlamingSword, SoulFireBall, FireWall and
the complete Struck/Die/Dead/Revive interaction chain. FireBall itself still
needs exact-head same-EXE playback through authenticated live WSS, GPU
additive/alpha pixels and human animation/audio/feel review.

The complete semantic denominator and legal asset pack, clean Crystal source
binding, real 100/125/150% DPI, full UI/live-WSS coverage, a 30-minute native
soak, formal publisher signing and whole-game human acceptance remain open.
Therefore `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false` remain mandatory.
