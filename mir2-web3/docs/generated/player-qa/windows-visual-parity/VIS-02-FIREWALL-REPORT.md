# Windows visual parity VIS-02 FireWall report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: ddc7248e8b6e0b4300b1b71a9c615aa1e0a209a9
FireWall implementation revision: f6f78f3eddb813897cf4ce4c6056183130ab7f35
branch: codex/windows-visual-parity
vis02Status: in_progress
fireWallAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes one bounded automated FireWall presentation checkpoint
inside VIS-02. Lightning, FireBall, SoulFireBall and FireWall now have bounded
automated checkpoints; FlamingSword remains open in the first five-skill
slice. The wider Struck/Die/Dead/Revive chain also remains open. No exact-head
package, authenticated live-WSS playback, GPU raster capture or human
animation/audio acceptance was produced, so this is not full VIS-02, Windows
visual parity or whole-game parity.

## Source-bound behavior implemented

- Typed `ObjectMagic(FireWall)` starts the caster-attached
  `Magic/1620..1629` ten-frame action at packet arrival and queues exact
  `M39-0.wav`. The source action is 600 ms. A successful `cast=true` queues
  exact `M39-1.wav` at that completion boundary; the separately identified
  synthetic `cast=false` compatibility input retains only the start action and
  sound.
- Five typed `ObjectSpell` projections model one fully valid center plus
  Up/Right/Down/Left cross at the source 500 ms server boundary. Each object
  resolves `Magic/1630..1635`, 120 ms, `repeat=true`, `light=3` and additive
  blend. The presentation remains until its authoritative `ObjectRemove`;
  replaying the same object identity replaces instead of duplicating it.
- Map change, logout/reconnect/session reset clear the cast, all persistent
  ground objects and pending completion audio. The native path does not infer
  persistence from the transient cast packet.
- Exact audio identities are:
  - `M39-0.wav`: 246,912 bytes, SHA-256
    `464F33258DDD963A9D969AC1B439EA0FEA0A39529B84D7CC6A762FF5B712F3AF`;
  - `M39-1.wav`: 525,980 bytes, SHA-256
    `E6D5E62494DA3D2F83073D7D17FF168B251D94AED8B054B3931E7A360894E6BE`.
- Source packaging and copied-Candidate verification rules now require both
  exact audio identities and `Magic/1620..1635`. Their self-tests remove the
  completion sound and final ground frame and fail closed. No package was
  built from this exact head.

## Packet-evidence scope

The fixture is explicitly a typed `ServerPacket -> client event` projection
contract, not an authenticated production transcript. Its canonical timeline
contains one successful `ObjectMagic` followed by five eligible
`ObjectSpell` objects. The 500 ms timestamps are source contract annotations;
the projection test does not measure Gateway wall-clock delivery.

`cast=false` is stored outside that canonical timeline as
`syntheticCompatibilityCase=true` and `productionReachability=not-asserted`.
Crystal's normal FireWall branch leaves `cast=true`; the compatibility test
must not be read as a second production cast or an event expected at 2,000 ms.
Collision or existing FireWall occupancy may omit individual cells in real
server output; the five-cell fixture represents only the all-valid case.

## Automated evidence

| Gate | Result |
|---|---|
| Independent Crystal/source audit | PASS; cast/ground/audio contract identified |
| Independent final P0/P1 review | No P0; two P1 claim-boundary issues corrected |
| Gateway packet-event projection fixture | PASS, 1/1 |
| FireWall focused native effects | PASS, 5/5 |
| Full Windows native suite | PASS, 351/351 |
| Full `mir2-client-bevy` native-ui suite | PASS, 393/393 |
| Gameplay audio allowlist/queue regression | PASS, 1/1 |
| Magic-effect exporter/validator | PASS, 73 spells |
| Web typecheck and full offline resource/audio gate | PASS |
| Candidate package script self-test | PASS; missing FireWall asset fails closed |
| Candidate verifier self-test | PASS; missing FireWall asset fails closed |
| Rustfmt and diff checks | PASS |

The tests used source assets in the isolated visual-parity worktree. The
frozen playable Candidate processes were not stopped, replaced, launched or
used as evidence for this revision.

## Existing server support and unclosed boundaries

The current personal simulation and shared Zone already contain a 500 ms
FireWall ground schedule, center/cardinal geometry, persistent `ObjectSpell`
objects, 2,000 ms damage ticks and duration derived from damage. This client
checkpoint changes none of that gameplay authority and does not certify the
complete Crystal server matrix.

Still required are exact current-head negative and lifecycle evidence for each
blocked cardinal, existing-cell suppression, caster death/map transfer/
despawn cleanup, expiration, map `FireWallLimit` oldest-cast-group replacement,
and stable observer/AOI object identity. An authenticated same-EXE transcript
must also prove the successful cast, ground-spawn and removal sequence without
using this projection fixture as a substitute.

VIS-02 still needs FlamingSword's `ObjectAttack.spell=8` Attack1-bound overlay
and sound; it must not be faked as `ObjectMagic`. FireWall also still needs
same-EXE authenticated live-WSS playback, GPU additive/alpha pixels and human
animation/audio/feel review. Real 100/125/150% DPI, a 30-minute native soak,
the complete semantic denominator and legal asset pack, clean source binding,
formal publisher signing and final human acceptance remain open.

