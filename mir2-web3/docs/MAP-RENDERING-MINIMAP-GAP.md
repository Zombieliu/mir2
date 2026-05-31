# Map rendering / big-map / minimap — gap audit

Audit of the **client** map presentation (separate from the server-side map
simulation). Answers "is rendering + 大地图/小地图 complete?". Short version:
the **minimap/bigmap feature is built and asset-complete**; the remaining gaps
are **externally constrained** (original art + Crystal-client calibration).

## Minimap (小地图)
- **UI**: ✅ `MiniMapPanel` / `MiniMapScene` render the `MMap` raster centred on
  the player with a tracking dot.
- **Assets**: ✅ **294/294** maps with `mini_map > 0` have their image in
  `public/original-ui/MMap/` (0 missing; 227 MMap frames cover all indices).
- **Projection calibration**: ⚠️ only map `0` (Bichon) has a hand-calibrated
  **isometric** transform in `CRYSTAL_MINI_MAP_TRANSFORMS`. The other 462 maps
  use `createLinearMiniMapTransform` (a linear world→image fit from the map's
  real W×H and the image W×H). The dot is placed, but on maps whose MMap frame
  is an isometric render the placement is approximate rather than pixel-exact.

## Big map (大地图)
- **UI**: ✅ `BigMapDialog` renders the `MMap` frame scaled into the world-map
  window with player + NPC markers (`CRYSTAL_BIG_MAP_NPCS`).
- **Assets**: ⚠️ **227/229** present. **2 missing**: `NAMMAN` (DeadForest,
  big_map 287) and `NAMMAN2` (CastleRuins, big_map 298) — their frames were
  never exported from `MMap.Lib`.
  - **Fixed here**: `BigMapDialog` now falls back to the map's **minimap** image
    when the bigmap image is absent (both NAMMAN/NAMMAN2 have minimap frames),
    so those maps show a base image instead of a blank window.
- **Projection**: same as minimap — only map `0` calibrated.

## Rendering (渲染)
- The scene render is the web/Bevy asset pipeline (`crystal-map-loader.ts`,
  scene/atlas), **not** part of this branch. Per `RESOURCE-LOADING-COMPLETION.md`
  it is ~85–88%: real WebGL2 rendering over ~15k pre-exported PNGs with graceful
  degradation (`missingAssets` diagnostics, synthetic-map fallback behind
  `MIR2_ALLOW_SYNTHETIC_MAP_FALLBACK`). Remaining holes are **missing original
  PNGs** that must be exported from the Crystal client `.Lib` files.

## What's code (doable here) vs external input

| Gap | Type | Status |
|-----|------|--------|
| Bigmap base image for NAMMAN/NAMMAN2 | code (fallback) | ✅ fixed (use minimap frame) |
| Minimap/bigmap UI + asset wiring | code | ✅ already complete |
| Per-map **isometric** transform calibration (462 maps) | **needs Crystal client + `.map` dims** | ⛔ can't calibrate offline |
| 2 true bigmap art frames (287/298) | **needs original `MMap.Lib`** | ⛔ art-constrained |
| Rendering PNG coverage to 100% | **needs original `.Lib` exports** | ⛔ art-constrained (separate pipeline) |

The two ⛔ classes are the audit's "C class" work — they need original art /
the live Crystal client and can't be produced from code alone in this sandbox.
