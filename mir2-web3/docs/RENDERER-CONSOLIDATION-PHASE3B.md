# Phase 3b — Renderer consolidation

Goal: make **Bevy the sole renderer** and retire the competing DOM WebGL2 layers
(`WebGl2EntityAtlasLayer`, `WebGl2MapAtlasLayer`).

> **Status:**
> - **Entity renderer — DONE.** Bevy is now the sole entity renderer on **both**
>   backends. The webgl2 Bevy build composites transparently; the fold is **default-ON**
>   with a `?bevyFoldWebgl2=0` escape hatch. Verified in a production build on Chrome
>   (webgpu + webgl2). The DOM `WebGl2EntityAtlasLayer` is now inert by default
>   (kept as a flag-gated fallback, not deleted).
> - **Map renderer — DEFERRED.** `WebGl2MapAtlasLayer` is **not** a competing renderer
>   (Bevy draws no map tiles); retiring it means *porting map-tile rendering into Bevy*,
>   a separate feature. Kept as-is.

## How entity rendering works now

- **WebGPU browsers** (default where `navigator.gpu` exists): Bevy renders entities;
  the DOM `WebGl2EntityAtlasLayer` is inert. (Unchanged.)
- **webgl2 browsers** (no WebGPU): the Bevy canvas now stays **visible + transparent**
  and renders entities; the DOM `WebGl2EntityAtlasLayer` self-disables; the DOM map
  layer (z1) shows through the transparent Bevy canvas (z2). (Was: Bevy canvas hidden,
  DOM layer drew entities — because the webgl2 build used to be opaque.)

The **map** is still DOM (`WebGl2MapAtlasLayer`, or per-tile `<img>`) on both backends —
Bevy draws no map tiles.

## The fix (the crux)

Making the webgl2 build transparent was necessary but not sufficient. With
`CompositeAlphaMode::PreMultiplied` (the webgpu setting) the wgpu **GL** backend
composited the drawn entity sprites to **fully transparent** — the DOM map showed
through but the sprites were invisible. The webgl2 surface needs **`CompositeAlphaMode::Auto`**
(`runtime/src/lib.rs`); with Auto, sprites render opaquely and the map shows through.

- `runtime/src/lib.rs`: `WINDOW_TRANSPARENT = true` for both wasm backends;
  `WINDOW_COMPOSITE_ALPHA_MODE` = `PreMultiplied` for webgpu, **`Auto` for webgl2**.
- `original-client-shell.tsx`: `foldWebgl2ToBevy` **default ON** (`?bevyFoldWebgl2=0` /
  `localStorage mir2-bevy-fold-webgl2=0` to fall back to the DOM layer). On webgl2 it
  shows the Bevy canvas and disables the DOM `WebGl2EntityAtlasLayer`.

## Verification (production build, Chrome, release gateway)

| Case | Result |
|---|---|
| WebGPU (default) | ✅ unchanged — Bevy draws entities, `canvasHidden:false`, `atlasMode:packed` |
| webgl2, fold ON (now default) | ✅ Bevy draws entities (`enabled:true`, `canvasHidden:false`, DOM layer `reason:"disabled"`), the **player sprite renders** and the DOM map shows through — visuals match the DOM-layer path |
| webgl2, `?bevyFoldWebgl2=0` | ✅ falls back to the DOM `WebGl2EntityAtlasLayer` (`reason:"rendered"`) — the escape hatch works |

## Residual / follow-ups

1. **Cross-browser spot-check.** Verified on Chrome for both backends; recommend a
   quick check on a genuinely non-WebGPU browser (Firefox / older Safari) since
   `?bevyBackend=webgl2` only forces module selection. The `?bevyFoldWebgl2=0` escape
   hatch + the retained DOM layer cover the unlikely case of a browser whose
   transparent webgl2 compositing differs.
2. **Delete the DOM `WebGl2EntityAtlasLayer`** once cross-browser confidence is high
   (it is inert by default now; deletion also removes the QA route +
   `shouldUseRawWebGl2EntityRenderer` gating).
3. **Retire `WebGl2MapAtlasLayer`** — requires porting map-tile rendering into the Bevy
   runtime (Bevy draws no map tiles today). A separate, larger feature; deferred.
