# Windows visual parity VIS-03 button-state report

Date: 2026-08-27

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
implementation base: f9ce31a32c3abe60d489cb484aa2e1174ee8ff43
VIS-03 implementation revision: 448db4f723b0032dac4d794f65bb1cbee1f66ae0
branch: codex/windows-visual-parity
vis03Status: in_progress
buttonStateAutomatedCheckpoint: complete
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
```

This report closes one bounded automated checkpoint inside VIS-03. It does not
close the full HUD, Inventory, BigMap or whole-game UI denominator. No packaged
exact-head executable, authenticated live-WSS playback, GPU raster capture,
real-DPI run or human visual/interaction acceptance was produced.

## Source-bound behavior implemented

- The 1024x768 HUD base remains `Prguse/1`. The Inventory button remains at
  `(928,692,20,20)` with exact normal/hover/pressed frames
  `Prguse/1903`, `1904` and `1905`.
- `CrystalButtonAssetSet` now supports an optional explicit disabled frame.
  Existing buttons remain compatible: when that frame is absent, Disabled
  continues to use the normal frame.
- BigMap Teleport at `(638,432,72,25)` uses Crystal's
  `Title/821/822/823` normal/hover/pressed frames and explicit `Title/823`
  disabled art. A disabled entity has no Bevy `Button` component and its
  actual `ImageNode` resolves to `original-ui/Title/823.png`.
- Teleport intent now requires the active target map to equal the
  authoritative current map. A search result on a cached remote map cannot
  enable Teleport; returning to a valid current-map NPC can.
- Source packaging and copied-Candidate verification require the newly bound
  HUD, Inventory and BigMap assets. The verifier self-test deletes
  `Title/823.png` and proves the required-file check fails closed.

## Automated evidence

| Gate | Result |
|---|---|
| Independent read-only VIS-03 review | PASS; no P0/P1 |
| Disabled-frame default and explicit override tests | PASS |
| BigMap remote-map Teleport rejection | PASS |
| BigMap disabled entity and exact `Title/823` image route | PASS |
| Inventory exact three-state asset/geometry contract | PASS |
| Full `mir2-client-bevy` native-ui suite | PASS, 393/393 |
| Full Windows native suite with fresh source map + keyed packs | PASS, 333/333 |
| Candidate package script self-test | PASS; ADS and reparse probes pass |
| Candidate verifier self-test | PASS; missing `Title/823` fails closed |
| Rustfmt and diff checks | PASS |

The Windows suite used freshly generated ignored source assets in the isolated
visual-parity worktree. The frozen playable Candidate processes were not
stopped, replaced, launched or used as evidence.

## Open VIS-03 and final gates

VIS-03 still needs exact-head same-EXE mouse/keyboard interaction, GPU pixels
for normal/hover/pressed/disabled states, and real 100/125/150% DPI plus human
visual/feel review. The remaining HUD, Inventory, BigMap and panel leaves must
be extracted and audited against the still-incomplete semantic denominator.

VIS-02 also remains in progress for FlamingSword, FireBall, SoulFireBall and
FireWall. Complete legal assets, clean Crystal source binding, full live-WSS,
a 30-minute native soak, formal publisher signing and whole-game human
acceptance remain open. Therefore `globalParityPercent=null`,
`accepted=false` and `visualAccepted=false` remain mandatory.
