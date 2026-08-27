# Windows visual parity VIS-02 SoulFireBall report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 438747df9e47ce2af0891b3d5e1059d79e27bc3a
SoulFireBall implementation revision: 19991af6ddb289dc2fb22569849599caabf9195e
branch: codex/windows-visual-parity
vis02Status: in_progress
soulFireBallAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
productionNoAmuletCastFalseReachabilityProved: false
```

This report closes one bounded automated SoulFireBall presentation checkpoint
inside VIS-02. Lightning, FireBall and SoulFireBall now have bounded automated
checkpoints; FlamingSword and FireWall remain open. No exact-head packaged
executable, authenticated live-WSS playback, GPU raster capture or human
animation/audio acceptance was produced, so this is not a full VIS-02,
Windows-visual or whole-game parity claim.

## Source-bound behavior implemented

- Typed `ObjectMagic` starts SoulFireBall with exact `M64-0.wav` and no cast
  bitmap. When `cast=true`, the native client creates its local missile at the
  600 ms Spell-action completion boundary. When `cast=false`, the ready sound
  still plays but no missile, impact or later phase audio is fabricated.
- At launch, the client resolves whether `targetId` is still present. A live
  target supplies its then-current tile and locks Crystal Direction16 for the
  flight; later target movement retimes the bound destination without changing
  that launch direction. A target absent at launch falls back to the packet
  point and receives no invented impact.
- The missile uses three source frames at
  `Magic/(1160 + direction * 10)..+2` for all 16 directions and a finite
  `distance * 50 ms` clock. The bound-target completion path attaches
  `Magic/1360..1369` for 600 ms. Map change, logout/reconnect/reset and object
  departure clear retained presentation and pending audio.
- `ObjectProjectile(SoulFireBall)` is a Rust compatibility supplement, not the
  Crystal client trigger. The native adapter ignores it in forward, reverse
  and isolated replay order, preventing a duplicate or compatibility-only
  missile.
- Exact audio identities are:
  - `M64-0.wav`: 151,328 bytes, SHA-256
    `2736DA89BADEEA678DD17BC903D6AAC7D63595405D82E8C0E0C9F2FAF3E684C3`;
  - `M64-1.wav`: 168,768 bytes, SHA-256
    `3487AAA8B8218D68F34D9ACE7CFBD95A13667737216DED6BE16702CCE48E161E`;
  - `M64-2.wav`: 228,532 bytes, SHA-256
    `2D3F6EC560E0F11C86C95EBCE1E78907A154C127103E1B226B04203041B5689E`.
- Runtime manifests, source packaging and copied-Candidate verification require
  every SoulFireBall directional/impact frame and all three exact audio
  identities. Package and verifier self-tests remove a required frame/audio
  and fail closed.

## Packet-evidence scope

The fixture is explicitly a `server_packet_to_event` projection contract. It
locks typed serialization for successful `ObjectMagic`, the Rust compatibility
`ObjectProjectile`, and a synthesized failed `ObjectMagic` with Crystal's
`targetId=0`. It does not claim an authenticated production Gateway transcript
or production reachability. The current no-amulet Gateway branch returns no
packets when item preflight/commit fails, so a production `cast=false`
SoulFireBall route remains open.

## Automated evidence

| Gate | Result |
|---|---|
| Independent read-only runtime review | PASS; no P0/P1 |
| Independent evidence/contract review | P1 corrected: serializer-only scope and failed-cast `targetId=0` |
| Gateway packet-event projection fixture | PASS, 1/1 |
| SoulFireBall focused native effects | PASS, 6/6 |
| FireBall regression subset | PASS, 11/11 |
| Full Windows native suite | PASS, 346/346 |
| Full `mir2-client-bevy` native-ui suite | PASS, 393/393 |
| Magic-effect exporter/validator | PASS, 73 spells |
| Web typecheck and full offline resource/audio gate | PASS |
| Candidate package script self-test | PASS; missing SoulFireBall frame/audio fails closed |
| Candidate verifier self-test | PASS; missing SoulFireBall frame/audio fails closed |
| Rustfmt and diff checks | PASS |

The tests used source assets in the isolated visual-parity worktree. The frozen
playable Candidate processes were not stopped, replaced, launched or used as
evidence for this revision. No Candidate package was built from this exact
head.

## Open SoulFireBall, VIS-02 and final gates

Crystal suppresses the impact and `M64-2` when the bound target's action is
already Dead at missile completion. The current native effect input has target
tiles/removal but no explicit target-dead bit, so this branch remains open.
Exact post-launch target-removal behavior also remains unclaimed.

Shared-Zone SoulFireBall authority still has separate backend gaps: monster
damage timing does not yet match Crystal's `500 + distance * 50 ms` path, PvP
and target/item preflight revalidation are incomplete, range/flight validation
does not yet prove Crystal's authoritative target, range-10 and `CanFly`
semantics, and `ObjectMana`/`ObjectProjectile` remain Rust compatibility
surfaces rather than Crystal wire truth. Those gaps were not mixed into this
client presentation commit.

VIS-02 remains in progress for FlamingSword, FireWall and the complete
Struck/Die/Dead/Revive interaction chain. SoulFireBall still needs exact-head
same-EXE playback through authenticated live WSS, GPU additive/alpha pixels
and human animation/audio/feel review.

The complete semantic denominator and legal asset pack, clean Crystal source
binding, real 100/125/150% DPI, full UI/live-WSS coverage, a 30-minute native
soak, formal publisher signing and whole-game human acceptance remain open.
Therefore `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false` remain mandatory.
