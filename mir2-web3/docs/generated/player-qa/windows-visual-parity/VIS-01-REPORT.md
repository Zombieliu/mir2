# Windows visual parity VIS-01 report

Date: 2026-08-27

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: 4eefa6019251110f24f5f1aa203d51dc59bc3131
implementation revision: 83ed26c62aab3cffde868d5923b2ad40e2b9e9d3
branch: codex/windows-visual-parity
phaseStatus: in_progress
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
```

This report binds the first bounded VIS-01 source/test checkpoint. It does not
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
- No client-controlled spawn, teleport, QA/admin or debug-world path was
  introduced.

## Automated evidence

| Gate | Result |
|---|---|
| `mir2-client-runtime --lib` | PASS, 187/187 |
| Four independent Rust `+1.95.0` manifest format gates | PASS |
| Phase-A ledger integrity verifier | PASS; integrity only |
| Read-only agent P0/P1 review after real-packet correction | PASS |
| Windows full suite using frozen Candidate assets | FAIL, 320/322 passed |

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
phases; Scarecrow's additive death pass; and a real `0.map` same-row occlusion
cell with z-order evidence.

Clean Crystal source binding, complete legal assets, same-EXE authenticated
live WSS, real 100/125/150% DPI, a 30-minute native soak, human visual,
animation, audio and feel acceptance, the complete semantic denominator and
formal publisher signing also remain open.
