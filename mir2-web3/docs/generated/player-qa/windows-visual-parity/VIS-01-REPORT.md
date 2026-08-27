# Windows visual parity VIS-01 report

Date: 2026-08-27

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 4eefa6019251110f24f5f1aa203d51dc59bc3131
implementation revision: ef619b55158539dacae19deb2a428a19c02becab
branch: codex/windows-visual-parity
phaseStatus: in_progress
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
```

This report binds the current bounded VIS-01 automated actor-transcript and
render-state checkpoints. It does not contain a packaged-EXE screenshot, GPU
raster comparison, live-WSS trace or human acceptance and does not claim
Crystal visual acceptance or a percentage.

## Implemented source/test checkpoint

- A fixed, source-revision-bound `0.map` Bichon transcript now covers the male
  Warrior self, female Warrior remote player, Hen, Deer, Scarecrow and
  CannibalPlant. Seventeen strong packet events produce 15 exact actor render
  checkpoints plus one damage-text checkpoint across live, walk, attack,
  struck, death, harvest, harvested, Show, Hide and restore phases.
- The Gateway fixture test constructs the exact typed `ServerPacket` sequence
  and equality-checks every projected native event. Incremental
  `ObjectMonster` events now carry the authoritative monster sprite contract,
  so a packet-first monster cannot silently route through `Monster/000`.
- Monster disposition fails closed to neutral when neither a snapshot nor an
  authoritative relationship provides it. Incremental monster packets retain
  an earlier snapshot disposition instead of inventing hostility.
- `ObjectDied` now retains the packet location, direction and numeric death
  kind. The fixture deliberately changes Deer and Scarecrow death transforms,
  preventing a state-only test from masking a stale pre-death pose.
- Every checkpoint checks the exact ordered layer set, layer count, unique
  paths and z ordering. Player/monster libraries resolve through generated
  Crystal frame sets, and the production test also locks their aggregate
  source hash to
  `712e5fdc080c66916e89eded26302778623ad3c9ee01013dd9eb37f28529889d`.
- The Candidate entity atlas now includes the Harvest-only `CWeapon/01`
  library. The regenerated seven-page pack has 10,482 sprites, and the fixed
  transcript proves the male Warrior harvest layer uses that packed path.

- `Show` and `Hide` are explicit shared animation actions. Generated Crystal
  frame sets are used only when the actor library defines those actions; no
  universal fallback is fabricated.
- Real Gateway `ObjectMonster` identity (`kind=monster`, `image=10`) routes
  CannibalPlant through `Monster/010`: Show displays source frames `4..11`,
  Hide displays `12..5`, and visual suppression starts only at the 1600 ms
  Hide completion boundary.
- A later `ObjectMonster -> ObjectShow` restores the retained actor. Show can
  interrupt an in-progress Hide, preventing a replay/recovery packet from
  stalling behind the Hide action queue.
- ObjectRemove clears retained hide state so a later actor may safely reuse an
  object ID. Unknown and non-Cannibal ObjectHide retain the former
  remove/tombstone behavior; unknown and non-Cannibal ObjectShow are no-ops.
  Zuma stoning, Shinsu body swap and other distinct Crystal Hide policies are
  deliberately not claimed by this checkpoint.
- Scarecrow `Monster/005` Die now adds Crystal source frames `224..233` at the
  exact Die phase. The layer uses the packed atlas sub-rect through the native
  additive material (`SourceAlpha + One` RGB equivalent), rather than drawing
  a normal-alpha sprite or requiring an unpacked per-frame PNG.
- Scarecrow `DrawEffects` ordering uses the same map viewport guard-band and
  front-depth constants as the real map producer. The self-contained test
  resolves a synthetic type-100 map through `resolve_map_tile_draws`; the
  production-assets test additionally builds the real `0.map` render state,
  requires the declared front cell to resolve to a texture/atlas binding, and
  proves its geometry intersects the CannibalPlant body while retaining the
  expected z order.
- The Scarecrow effect follows Crystal's Effect option. An option-only
  enabled -> disabled -> enabled transition republishes the same authoritative
  pose without another Gateway packet; runtime stale cleanup removes the old
  Mesh/material and mode/reset regressions prove bounded recycling.
- No client-controlled spawn, teleport, QA/admin or debug-world path was
  introduced.

## Automated evidence

| Gate | Result |
|---|---|
| `mir2-client-runtime --lib` | PASS, 191/191 |
| Gateway exact typed VIS-01 fixture projection | PASS, 1/1 on `ef619b551` |
| Native adapter sprite/disposition/death regressions | PASS, 2/2 |
| Self-contained ordered transcript/render-state test | PASS, 1/1 |
| Production frame-set/atlas/real-map transcript test | PASS, 1/1 |
| Player-frame generator and entity-pack tests | PASS |
| Release asset preflight | PASS for atlas/render coverage; original-asset integrity unavailable in this isolated checkout |
| Rustfmt and diff checks | PASS |
| Phase-A ledger integrity verifier | PASS; integrity only |
| Windows full suite using the composite production asset root | FAIL, 327/329 passed |

The production-root test combined the newly generated entity atlas with the
frozen Candidate map/effect/frame-set assets. It is not a new exact-head
packaged Candidate. The two Windows failures remain the existing asset-closure
failures:

- `ARArmour/00/24.png` is absent.
- `Mount/00/32.png` is absent.

They remain failures and are not converted to accepted skips. The frozen
playable Candidate process was not stopped, replaced or used as evidence for
this new implementation. The Gateway's complete 655-pass suite was run at the
preceding checkpoint; after the latest sprite-projection change, the exact
focused typed fixture passed and the fresh branch CI remains the complete-suite
authority.

## Open VIS-01 and final gates

VIS-01 remains in progress. The fixed packet transcript, production asset
routing and real-map render-state binding are now automated. It still requires
the same exact sequence through real Gateway routing/WebSocket encoding and an
authenticated same-EXE session, plus captures for the male Warrior self,
female remote player, Hen, Deer, Scarecrow and CannibalPlant across live,
combat, harvest and occlusion phases. Scarecrow's additive death pixels and the
real `0.map` same-row occlusion cell still require GPU raster evidence; binding
a tile texture and intersecting geometry does not prove pixel opacity or blend
output. Walk/Attack/Struck/Harvest currently prove their deterministic start
frames, while death/hide have completion-boundary coverage. Automated
state/ECS tests do not substitute for packaged-executable captures.

Clean Crystal source binding, complete legal assets, same-EXE authenticated
live WSS, real 100/125/150% DPI, a 30-minute native soak, human visual,
animation, audio and feel acceptance, the complete semantic denominator and
formal publisher signing also remain open.
