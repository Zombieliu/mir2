# Windows visual parity VIS-02 LeftGuard range-projectile report

Date: 2026-08-28

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: a7cac07b7c0134f4bf5715d2676c3c60eeb756a4
LeftGuard range-projectile implementation revision: d2dfff14308256c07c3b3169798afee0a051b97b
branch: codex/windows-visual-parity
vis02Status: in_progress
leftGuardRangeProjectileAutomatedCheckpoint: complete
monsterActionFeedParity: open
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
authenticatedLiveWssTranscriptProduced: false
exactHeadCandidatePackageProduced: false
```

This report closes one bounded automated Windows-native VFX checkpoint for
LeftGuard's `AttackRange1` projectile. It does not close monster ActionFeed,
VIS-02, monster presentation, Windows visual parity or whole-game parity.

## Source-bound behavior implemented

- Typed `ObjectRangeAttack` keeps the existing `attackRange1` actor hint. The
  existing native bridge supplies unmodified `objectId`, `targetId`,
  `location`, `target`, `direction`, `attackType`, `spell` and `level` fields
  plus authoritative actor context. No Gateway, protocol, simulation packet
  or combat authority was added.
- The consumer accepts only exact LeftGuard body library `Monster/100`
  (including canonical `/original-ui/Monster/100`). Other monsters, ordinary
  `ObjectAttack`, missing source/target/location, malformed IDs or incomplete
  frame data fail closed. RightGuard `Monster/099` remains isolated.
- Crystal's frame-4 branch is represented by a 400 ms action delay before the
  missile becomes visible. Direction16 is locked from the packet source
  location to the authoritative target at launch. Frames resolve as
  `Magic/10 + Direction16*10 + CurrentFrame%6`: 16 directions, six visible
  frames at 30 ms, additive blend, opacity 1, light 6, no repeat and no
  invented impact effect or shadow.
- Source position is fixed from `ObjectRangeAttack.location`, never silently
  replaced by a later Zone tile. Target position remains authoritative and is
  followed during flight. Each target move recomputes Crystal's maximum-tile
  distance clock at 50 ms per tile while preserving the locked launch
  direction.
- The missile is source-owned. Source `ObjectRemove` always clears it;
  `ObjectHide` preserves the Crystal object/effect relationship. Target Hide
  also preserves it. Target Remove before launch prevents construction; after
  launch it detaches the target and continues to the last known destination.
- Packet-first adapter snapshots tombstone most hidden actors before effects
  observe their Zone map. The implementation retains only the last tile still
  referenced by an active LeftGuard missile. Raw Zone state is never polluted,
  old event replay cannot resurrect it, and non-LeftGuard effects cannot use
  it. Same-batch Range-before-Hide is reconstructed from the prior same-map,
  same-generation raw snapshot; Hide-before-Range, MapChanged-before-Range and
  cross-generation object-ID reuse remain fail closed.
- A stable source-target key restarts on a newer attack for that pair; distinct
  targets coexist. Replayed sequences change neither start time nor
  provenance. Map change, logout, generation change, expiry and session reset
  clear the instance and all source/target/hidden-tile bookkeeping.
- The generated manifest source-binds `LeftGuardRangeProjectile` to
  `MonsterObject.cs::LeftGuard/AttackRange1/FrameIndex4/CreateProjectile` and
  declares `Magic/10..165` as 16 six-frame ranges with a ten-frame stride. All
  PNGs already existed in the tracked legal source assets; no bitmap was added.

## Explicitly open action and audio boundaries

The current native monster presentation applies the latest animation hint
immediately; it does not yet reproduce Crystal's queued monster ActionFeed.
The 400 ms frame clock and projectile lifecycle are exact for the current
immediate-action projection, but a future ActionFeed leaf must revalidate
consecutive queued attacks and interruptions.

This checkpoint adds no LeftGuard range audio claim. No new sound bytes,
allowlist entry, package requirement or audio-device evidence was introduced.

## Automated evidence

| Gate | Result |
|---|---|
| Independent Crystal/source audits | PASS; frame, asset, direction, timing, ownership and lifecycle contracts locked |
| Independent final P0/P1 reviews | PASS; packet-location, launch failure, tombstone, replay, generation and map-boundary findings remediated; final P0=0/P1=0 |
| LeftGuard focused native tests | PASS, 5/5 |
| Guard-range focused native tests | PASS, 10/10 |
| Full Windows native suite | PASS, 392/392 |
| Magic-effect exporter/validator | PASS, 74 spells |
| Exact manifest/native catalog contract | PASS; Magic 10..165, 16x6 frames, stride 10, 30 ms, blend, opacity 1, light 6 |
| Adapter/tombstone and lifecycle matrix | PASS; ordering, source/target, replay, map, generation, expiry and reset cases |
| Rust 1.95 formatting and diff checks | PASS |

The tests used source assets in the isolated visual-parity worktree. No client
process was launched, no exact-head package or executable was built, and no
authenticated WSS transcript, GPU screenshot, same-EXE capture, DPI matrix,
native soak or live audio-device evidence was produced.

## Remaining gates

LeftGuard still needs Crystal-style queued monster ActionFeed coverage,
authenticated same-EXE delivery and real GPU/light pixels. VIS-02 and the full
semantic denominator also retain the other 129 non-None spells, 34 non-None
spell effects, monster special effects, buffs/poisons, weather/environment
effects and unenumerated client-owned action branches.

Same-EXE UI/live WSS, 100/125/150% DPI, 30-minute native soak, human visual/
animation/audio/feel, complete legal assets and formal publisher signing remain
open. Therefore `globalParityPercent=null`, `accepted=false` and
`visualAccepted=false`.
