# Asset Delivery: Per-Frame PNG → Atlas Migration Plan

Status: **proposed** (2026-06-18). Owner: client/runtime + asset-pipeline lanes.
Supersedes the per-tile-upload direction recorded for the Bevy map uncovered-tile gap
(see §6). KTX2/GPU-compression status updated per the step-(a) spike (see §4).

## 1. Why — the problem, with measured evidence

The current asset delivery is a **half-migration**: a tiny "starter" set is packed into one
GPU atlas, but **everything else is fetched per-frame** as individual PNGs:

- Entities: only the `starter-bichon-base` atlas (2,631 sprites, one 4096² page) goes through
  the atlas path; every other entity frame renders via per-frame PNG fallback.
- Map: tiles are drawn from a 27-lib packed atlas subset; uncovered tiles are uploaded
  **per-tile** from R2 (`runtime/src/lib.rs::sync_map_render`).
- Full Crystal corpus on R2 ≈ **1.07M loose PNG files**.

Per-frame delivery is the slow path on every axis. Measured this session
(`/tmp/bench-atlas-vs-perframe.cjs`, libvips/sharp decode of the **same 2,064 sprites**,
2,064 found of the 2,631 in the starter atlas):

| Axis | Atlas (1 page) | Per-frame (2,064 PNGs) | Ratio |
|---|---:|---:|---:|
| HTTP requests | 1 | 2,064 | **2,064×** |
| GPU uploads (texImage2D) | 1 | 2,064 | **2,064×** |
| Decode calls | 1 | 2,064 | **2,064×** |
| Decode CPU | 73.5 ms | 1,084.2 ms | **14.8×** |
| Decode throughput | 0.91 Mpx/ms | 0.03 Mpx/ms | **30×** |
| Wire bytes | 4.07 MB | ~3.4–4.4 MB | ~wash |
| RGBA / VRAM | 64.0 MB | 31.3 MB | atlas ~2× |

The only axes where atlas costs *more* — wire bytes (~wash once per-file PNG-chunk overhead is
counted) and VRAM (page occupancy) — are both minor and tunable (§3). Every expensive
op-count and CPU axis favours the atlas by 1–3 orders of magnitude. This matches the universal
industry conclusion (Factorio FFF-264: non-atlas → per-sprite texture alloc/destroy + VRAM
fragmentation; MonoGame/Unity/HTML5: draw-call batching is the #1 win).

## 2. Goal — end-state shape

Make **atlas pages the primary delivery unit; per-frame PNG becomes dev/fallback only.**

- Pack at **Crystal `.Lib`/library granularity** (a leaf dir like `original-ui/Monster/000/` =
  one logical unit). Bin-pack multiple small libraries into a page until full; spill a large
  library across multiple pages.
- **Multi-page**: each rect carries `pageIndex` (schema already supports `pages[]` +
  `pageIndex` + `contentHash` — `scripts/asset-pipeline/schema.ts`).
- Runtime primary lookup = **atlas rect → fetch/peek page → blit sub-rect**; per-frame URL only
  on a rect miss.
- Pages stream through the **asset-residency manager** (`lib/asset-residency/manager.ts`:
  hot in-memory LRU / warm IndexedDB / cold R2), **off-thread decoded**, working-set capped.
- Mirror **PixiJS Assets** bundle/evict semantics (bundle = zone/region; background-load
  adjacent; unload on leave).

## 3. Locked parameters (from the measurement + step-a)

- **Page size 2048²** (16 MB RGBA, ~18 ms decode ≈ one frame budget) — not 4096² (73 ms spike).
- **Off-thread decode** in the cold-tier fetcher (`createImageBitmap`/`ImageDecoder` in a
  worker) — confirmed standard (PixiJS decodes textures to `ImageBitmap` on a worker; this repo
  already off-threads alpha-key, PR #96).
- **Working-set budget** derived from 16 MB/page: e.g. desktop ~512 MB ≈ 32 pages, mobile
  smaller; enforced by the residency LRU `memoryBudget`.
- **No silent drop**: a rect miss falls back to per-frame PNG and is recorded (telemetry),
  never a blank sprite.

## 4. KTX2 / GPU compression verdict (step-a spike, 2026-06-18)

GPU-compressed atlases (the "new format") are **feasible on our exact stack but NOT turnkey**,
and their web path is **WebGPU-validated / WebGL2-unproven**. They solve only the VRAM axis,
which is secondary — so they are sequenced **after** the PNG-atlas work and gated behind a
real-browser spike.

`bevy_basisu_loader` findings (crate `beicause/bevy_basisu_loader`):

- ✅ **Bevy 0.18 compatible**: published `0.6.0` → `bevy ^0.18` (our 0.18.1), confirmed via
  crates.io API. (Repo HEAD has moved to `bevy 0.19.0-rc.2`; do **not** track HEAD — pin `"0.6"`.)
- ✅ **Not WebGPU-hardwired**: `loader.rs` selects the transcode target from wgpu device
  features (`TEXTURE_COMPRESSION_BC`/`ETC2`/`ASTC`), so WebGL2 is architecturally possible.
- ⚠️ **Web build requires Emscripten** (emsdk 5.0.5 + wasm-bindgen + wasm-opt + a vendored
  `basisu_c_sys_asset_files` clone; the Basis C++ transcoder is emcc-compiled to a JS/wasm
  sidecar). Not a plain `cargo build`. emcc is **not** in the current toolchain.
- ⚠️ **Upstream only ships a `--features bevy/webgpu` web demo** — WebGL2 is unverified upstream.
- ⚠️ **Hard-fails with no RGBA fallback** if the device exposes no wgpu-surfaced compressed
  format → PNG atlas must remain the fallback layer regardless.
- ⚠️ **Integration**: it is an `AssetLoader` (`asset_server.load` → compressed `Image`); our
  runtime currently does a custom raw-RGBA push (`setMir2EntityRenderAtlas`) → adopting means
  rerouting atlas pages through the asset-load path.

**Residual unknown (needs a real browser/GPU, not closeable in-sandbox):** does our actual
WebGL2 + wgpu-GL backend report `TEXTURE_COMPRESSION_BC` on target browsers/GPUs? Desktop
Chrome/Firefox on a dGPU → almost certainly yes (`WEBGL_compressed_texture_s3tc`); some
mobile/software → none → fall back to PNG. Format choice when adopted = **UASTC L3 + zstd**
(39 dB, near-lossless on Mir art); **never ETC1S** (28 dB, unusable). Stick to **4×4-block**
formats so 2048²/4096² pages stay valid; avoid 6×6 XUASTC (needs ×12 dims).

## 5. Phased PRs (each additive, shippable, backward-compatible)

### PR1 — Entity atlas-first  *(high-confidence; do first)*
The smallest end-to-end proof: entities already have the atlas + residency rails; this scales
them from "starter" to "all" and kills the measured 2,064× per-frame entity path.

- Packer: extend `scripts/asset-pipeline/pack.mjs` with **multi-page spill** (`pageIndex`) +
  **per-library bin-packing** into 2048² pages; run over **all** entity libraries.
- Runtime: entity lookup prefers atlas rect for **all** entities (per-frame → fallback);
  cold-tier fetcher does **off-thread decode**; set residency `memoryBudget`.
- **Acceptance:**
  - `pack.mjs` emits a multi-page manifest validated against `schema.ts`; re-run is
    deterministic (stable `contentHash`).
  - In-game: a scene with ≥10 distinct entity types issues **page-count** atlas fetches, not
    per-frame fetches (verify via network panel / `residency.stats()`).
  - `npx tsc --noEmit` = 0; `cargo fmt --all --check` clean; existing rendering unchanged for
    entities already in the starter atlas (no visual regression).
  - Rect-miss still falls back to per-frame PNG (no blank sprites); misses are counted.

### PR2 — Map atlas/chunk streaming  *(nuanced; see §6 — measure first)*
Upgrade the current **per-tile** upload to **regional chunk pages** streamed + capped, using the
`bevy_ecs_tilemap` chunk-streaming pattern (chunk = mesh = one draw call; spawn/despawn by
camera; web needs the `atlas` feature). Pages are **pre-packed per map region on R2** (bounded,
not in git — see §6), not statically baked into git.

- Producer `original-client-scene-map-rendering.tsx::buildMapTileDrawList` + renderer
  `runtime/src/lib.rs::sync_map_render` switch the fetch/upload unit from tile → region page.
- **Pre-req measurement:** confirm map per-tile streaming is an actual in-game bottleneck
  (movement jank is partly React-architectural per `client-render-perf`); only commit the
  regional-pack lift if the payoff is real.
- **Acceptance:** walking across a map region issues **chunk-page** fetches/uploads, not
  per-tile; uncovered-tile gap stays closed (no dropped tiles); VRAM capped by residency.

### PR3 — KTX2 / UASTC GPU compression  *(optional VRAM optimization; gated)*
- **Gate:** a real-browser WebGL2 transcode spike must pass first (does our wgpu-GL backend
  expose a usable compressed format on target devices?). If it fails on WebGL2, PR3 is deferred
  or scoped to WebGPU-capable clients only.
- Add `bevy_basisu_loader = "0.6"`; add emsdk to the asset/release build chain; offline-encode
  the atlas pages to `.basisu.ktx2` (UASTC L3 + zstd, 4×4 block); reroute the runtime atlas
  upload through `asset_server.load`. **PNG atlas remains the mandatory fallback.**
- **Acceptance:** on a WebGL2 dGPU browser, pages load as compressed textures with ≥~4:1 VRAM
  reduction and no visible fidelity loss; on a no-compressed-format context, automatic fallback
  to PNG atlas with no blank sprites.

### PR4 — Release / airlock
- Offline build emits atlas pages as the canonical bundle; R2 release publishes **hundreds of
  pages** (entities) + **regional map pages**, not 1.07M loose frames. Per-frame retained as
  fallback during transition, then deprecated.
- Optional: **archive + HTTP Range(206)** delivery (≈ re-serving Crystal's own `.Lib`) if R2
  object count itself becomes the constraint.
- Update `build-remote-asset-release.mjs`, preflight, and version inputs (schema already
  content-hashes pages).

## 6. The map-decision reversal — reconciliation with `bevy-map-uncovered-tile-gap`

A prior audit recorded: **static atlas-expansion of the map is infeasible** because full
coverage = the entire unbounded Crystal map corpus (~1.07M files), deliberately kept out of git
and stripped from Vercel; the chosen fix was **per-tile R2 upload with a residency cap**
(Option B).

PR2 does **not** contradict the infeasibility finding — it refines Option B:

- The infeasibility is about **baking atlases into git**. It is *not* about R2. R2 already holds
  the full corpus as loose files; **re-packing those same tiles into regional pages on R2** is
  bounded per map and feasible on the asset-release lane (more bytes from padding, but
  object-count and request-count drop by orders of magnitude).
- So PR2 keeps Option B's residency-cap streaming model but changes the **unit** from *one tile*
  to *one regional page*: same "only what you walk into, capped working set", far fewer
  requests/uploads/decodes (the §1 measurement, applied to map).
- This is exactly the `bevy_ecs_tilemap` chunk-streaming pattern. The reversal is "per-region
  page instead of per-tile", not "static git atlas instead of streaming".

If PR2's pre-req measurement shows per-tile map streaming is *not* a real bottleneck, Option B
(per-tile) stands and only the entity lane (PR1) ships — the two lanes are independent.

## 7. Risks & fallbacks

- **WebGL2 compressed-format support (PR3):** unproven; mitigated by PNG-atlas fallback and the
  PR3 gate spike. Never a hard dependency.
- **VRAM from page occupancy:** mitigated by 2048² pages + tight bin-packing + residency cap.
- **Decode spike:** mitigated by off-thread decode + 2048² page size.
- **Map regional-pack lift (PR2):** large asset-pipeline effort; gated behind a measurement.
- **Backward compatibility:** every step keeps per-frame PNG as fallback; manifest changes are
  additive (schemaVersion superset); no `DisplayWorld`/existing-consumer breakage.

## 8. References

- Measurement: `/tmp/bench-atlas-vs-perframe.cjs` (this session).
- Pipeline: `scripts/asset-pipeline/pack.mjs`, `scripts/asset-pipeline/schema.ts`.
- Residency: `apps/web/lib/asset-residency/manager.ts`.
- Map: `runtime/src/lib.rs::sync_map_render`,
  `apps/web/app/components/original-client-scene-map-rendering.tsx::buildMapTileDrawList`,
  `scripts/build-map-atlas-pack.mjs`.
- KTX2: `beicause/bevy_basisu_loader` (Bevy 0.18 = `"0.6"`); Don McCurdy, "Choosing texture
  formats for WebGL/WebGPU"; UASTC vs ETC1S.
- Patterns: Factorio FFF-264 (texture streaming), PixiJS Assets (bundles/background-load/unload),
  `bevy_ecs_tilemap` (chunk streaming), HTTP Range(206) archive delivery.
</content>
</invoke>
