# Windows visual parity VIS-01 report

Date: 2026-08-27

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 4eefa6019251110f24f5f1aa203d51dc59bc3131
implementation revision: d97cd98aa762015d161972000a6cd3f42aa7abe1
branch: codex/windows-visual-parity
phaseStatus: in_progress
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
```

This report binds the current bounded VIS-01 source/test checkpoints. It does not
claim the fixed actor scene is complete, does not contain a packaged-EXE
screenshot and does not claim Crystal visual acceptance or a percentage.

## Implemented source/test checkpoint

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
  front-depth constants as the real map producer. A synthetic type-100 map is
  resolved through `resolve_map_tile_draws`, and the shallow Scarecrow effect
  is proven above the deepest retained front tile and a deeper actor.
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
| Four independent Rust `+1.95.0` manifest format gates | PASS |
| Phase-A ledger integrity verifier | PASS; integrity only |
| Read-only agent P0/P1 review after packet, depth and option corrections | PASS; P0/P1 zero |
| Windows full suite using frozen Candidate assets | FAIL, 323/325 passed |

The two Windows failures remain the existing asset-closure failures:

- `ARArmour/00/24.png` is absent.
- `Mount/00/32.png` is absent.

They remain failures and are not converted to accepted skips. The frozen
playable Candidate process was not stopped, replaced or used as evidence for
this new implementation.

## Open VIS-01 and final gates

VIS-01 remains in progress. It still requires the fixed Bichon packet fixture
and same-EXE captures for the male Warrior self, female remote player, Hen,
Deer, Scarecrow and CannibalPlant across live, combat, harvest and occlusion
phases, including Scarecrow's additive death pixels; and a real `0.map`
same-row occlusion cell with z-order raster evidence. Automated state/ECS
tests do not substitute for those captures.

Clean Crystal source binding, complete legal assets, same-EXE authenticated
live WSS, real 100/125/150% DPI, a 30-minute native soak, human visual,
animation, audio and feel acceptance, the complete semantic denominator and
formal publisher signing also remain open.
