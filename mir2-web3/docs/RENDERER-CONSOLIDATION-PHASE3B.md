# Phase 3b — Renderer consolidation (status: GROUNDWORK; full retirement GATED)

Goal: make **Bevy the sole renderer** and retire the competing DOM WebGL2 layers
(`WebGl2EntityAtlasLayer`, `WebGl2MapAtlasLayer`).

> **Status:** the prerequisite (transparent webgl2 Bevy build) is implemented and
> the transparent **map passthrough** is verified — but the fold cannot be turned
> on by default yet because Bevy's **entity sprites do not composite visibly on
> the wgpu GL backend**. This PR lands the groundwork behind a **default-OFF**
> flag (zero regression) and documents the remaining gates. **Do not flip the
> flag on by default or delete the DOM layers until the gates below are cleared.**

## How rendering works today

Two entity-render paths, selected at runtime by the Bevy backend:

- **WebGPU browsers** (the default where `navigator.gpu` exists): Bevy renders
  entities; the DOM `WebGl2EntityAtlasLayer` is inert. Already effectively
  "Bevy-only" for entities.
- **Non-WebGPU browsers** (Firefox, older Safari, locked-down Chrome): the Bevy
  canvas is **hidden** (`bevy-canvas-hidden`, opacity:0) and the DOM
  `WebGl2EntityAtlasLayer` draws entities. This exists because the webgl2 Bevy
  build was **opaque** (`WINDOW_TRANSPARENT=false`), so a visible Bevy canvas
  would paint over the DOM map/floor.

The **map** is *always* DOM (`WebGl2MapAtlasLayer`, or per-tile `<img>`). Bevy
draws **no** map tiles — so `WebGl2MapAtlasLayer` is **not a competing renderer**;
retiring it means *porting map-tile rendering into Bevy* (new feature), not
deleting a duplicate.

## What this PR does (groundwork, default-off)

1. **`runtime/src/lib.rs`** — make the webgl2 wasm build composite transparently
   like the webgpu build (`WINDOW_TRANSPARENT` / `WINDOW_COMPOSITE_ALPHA_MODE`
   keyed on `target_arch = "wasm32"` rather than the `webgpu` feature).
2. **`original-client-shell.tsx`** — a **default-OFF** `foldWebgl2ToBevy` flag
   (`?bevyFoldWebgl2=1` / `localStorage mir2-bevy-fold-webgl2=1`). When on (webgl2
   backend): the Bevy canvas stays visible+transparent and draws entities, the DOM
   `WebGl2EntityAtlasLayer` self-disables, and the DOM map (z1) shows through the
   transparent Bevy canvas (z2).

Rebuild the runtime (`npm run runtime:build:release`) to regenerate the wasm with
the transparent webgl2 build; this PR keeps the diff to source + this doc.

## Verification (production build, Chrome, release gateway)

| Case | Result |
|---|---|
| Default (WebGPU, flag off) | ✅ unchanged — Bevy draws entities, `canvasHidden:false`, `atlasMode:packed` |
| webgl2, flag **off** | ✅ unchanged — `canvasHidden:true`, DOM `WebGl2EntityAtlasLayer` draws (`reason:"rendered"`, 8 layers); the transparent lib.rs change did **not** break the webgl2 fallback or Bevy webgl2 boot |
| webgl2, fold **on** | ⚠️ **map shows through (transparency works)** but the Bevy-drawn **entity sprites are INVISIBLE** — only DOM name labels render. No console error: the sprites are silently composited to transparent on the wgpu GL surface. |

So: transparent **map passthrough** works on Chrome-webgl2, but Bevy's **entity
sprite compositing** does not — which is precisely **why the DOM
`WebGl2EntityAtlasLayer` was built**.

## Gates for full retirement (each a dedicated, verified effort)

1. **Fix webgl2 Bevy transparent sprite compositing.** Investigate the wgpu GL
   surface alpha path (`CompositeAlphaMode` support on GL, premultiplied-alpha vs
   the canvas `alpha`/`premultipliedAlpha` context attributes, sprite blend
   state). May be a wgpu GL-backend limitation. Until sprites composite visibly,
   the fold produces invisible entities.
2. **Cross-browser verification.** `?bevyBackend=webgl2` only forces module
   selection; the real target is genuinely non-WebGPU browsers (Firefox, Safari).
   Transparent GL compositing must be confirmed there before flipping default-on.
3. **Map-tile rendering in Bevy.** To retire `WebGl2MapAtlasLayer`, port the
   map-tile draw (`buildMapTileDrawList`) into the Bevy runtime — Bevy draws no map
   tiles today. Recommended to defer; it is orthogonal and lower value.

Once (1)+(2) clear: flip `foldWebgl2ToBevy` default-on, remove
`WebGl2EntityAtlasLayer` + its mount + the QA route + the `shouldUseRawWebGl2…`
gating. (3) is separate.

## Why default-off is safe

Flag off = today's behavior exactly (verified: WebGPU and webgl2-flag-off paths
unchanged). The transparent webgl2 build only matters when the canvas is shown,
which only the (default-off) flag does.
