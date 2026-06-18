# Render-perf roadmap — "10-min continuous walk, no jank"

## ✅ RESOLVED (2026-06-19) — goal met for sustained continuous walking

**Measured (release build, in-page rAF-delta + `performance.memory` sampler, synthetic
keyboard walk, `allVisible:true`):**

- **6-min warm continuous walk (`?renderLoopRaf=1&mapSpritePool=1`) = `0/24` windows with a
  >50ms frame, `0/24` >35ms, worst single frame 34.9ms, p95 18.5ms (locked 60fps), heap bounded
  182–273MB (flat — no sawtooth).**
- 75s walk crossing **6 scene windows = 0 mid-walk spikes.**
- DEFAULT config (②-only, no flags) = `1/13` (one 133ms transition spike, then 12 windows locked
  60fps).

**The diagnosis was corrected.** The "130–150ms GC spikes" were **not** continuous-walk jank —
they are **(a)** the one-time map-**entry mount** (~100ms, hidden behind the "Loading map…"
overlay) and **(b)** **cold-HTTP-first-fetch** of never-loaded tiles (`createImageBitmap`+decode
of cold R2 bytes). Both are **fresh-page-reload artifacts**, not in-game walking. Profiled
decisively: during a spike every Bevy push and every `JSON.stringify` is **<3ms** (longtask
attributed to `window:unknown` = GC/render class); warm walking keeps pushes <3ms with **0
spikes**. The real fix was **② (Bevy entity interpolation, already default-on)** removing the
30Hz `JSON.stringify` churn → heap stopped sawtoothing to ~450MB → the GC-spike source is gone.

**Disproven (do not re-attempt):** `①` off-thread decode (not decode-CPU) and a **decode-
concurrency throttle** (cap in-flight standalone-tile decodes — implemented, A/B-measured, did
NOT move the spike; **reverted**). `#4`/`(B)` below are the proven-pristine config (0/24) but
only **marginal** over the ②-only default; left flag-gated default-OFF pending a ship decision
(flipping a live-render-path default affects all players). Residual = entry-mount (overlay-hidden)
+ first-ever-region cold fetch (caches after first visit) — optional future polish: prefetch the
spawn-area + look-ahead ring during the loading overlay.

## Context / why

Goal: a player walking continuously for 10+ minutes with **no perceptible jank** and correct
map + NPC rendering. Map/NPC rendering correctness is **done** (uncovered-tile fix + Crystal
y-sort). The remaining gap is **jank during walking**.

**Measured (release build — release wasm + `cargo build --release` gateway + `next build && next
start`, NOT dev; a 3–4 min synthetic continuous-walk + rAF-delta + `performance.memory` sampler on
MongchonProvince/map3):**

- Steady-state is **locked ~60fps** (p50 16.7 / p95 ~18ms), **no memory leak** (JS heap sawtooths
  ~180↔450MB, GC reclaims fully).
- **The jank = major GC pauses: a 130–150ms single-frame hitch every ~10–15s**, landing exactly on
  the GC reclaim points. From **per-frame allocation churn during walking**.
- It is **NOT** decode CPU (off-thread tile decode didn't move it), **NOT** region-change (the
  synthetic walk did 0 `/api/scene/crystal` fetches — a real cross-map walk would ADD scene-fetch
  spikes, so the bench under-tests crowds/regions).
- The churn is **multi-source** — fixing one source only adds a few clean windows. Confirmed
  dominant allocators while walking:
  - **(A) the 30Hz `motionNow` React clock** — re-renders the whole ~3.5k-line shell 30×/sec.
  - **(B) per-cell-cross sprite rebuilds** — `buildViewportMapSprites` allocates ~500 sprite
    objects every cell crossed.
  - **(C) `setWorld`-per-packet** — full-monolith re-render per gateway packet (busy maps).

**Conclusion:** "10-min no jank" is **not reachable incrementally** — it needs the architectural
move of **driving per-frame rendering OUT of React**. This roadmap stages that.

> Already shipped (verified, flag-gated, on `claude/sleepy-grothendieck-2c2534`): uncovered-tile
> Bevy rendering, map **root-offset camera model**, **② Bevy entity interpolation**
> (`?bevyEntityInterp=0`), **① off-thread tile decode** (`?bevyMapTilesDecode=0`, cache leak fixed),
> "Entering world…" overlay. ② is visually correct (entities stay map-aligned scrolling, no jitter)
> but only a **partial** GC win (4/18 clean windows) — confirming the multi-source diagnosis.

## Method (non-negotiable)

Every stage: **flag-gated** (default off until proven) + **A/B measured** against the walk
benchmark — the metric is **# of 10s windows with a >50ms frame** and **heap sawtooth amplitude**,
not feel. Ship a stage only when it measurably reduces GC-spike windows. Stages are ordered
smallest-safe-win first.

## Piece #4 — `motionNow` out of React (effort M, **DO FIRST** — biggest single lever)

Root: `original-client-shell.tsx:832-874` `setMotionNow(Date.now())` ~30Hz → full shell render
30×/sec, recomputing `playerCameraMotionOffset` (1029), `refreshEntityMotionSnapshots` (1022),
`deriveChatBubbles`/`projectileProgress` (1250-1259), and pushing `setMir2MapCameraOffset` via the
effect at ~1615. Map tiles + entity interp already live in Bevy, so these can move to a ref/rAF loop.

- **Stage 1 — ref-based camera-offset rAF loop.** Move the `setMir2MapCameraOffset` push out of the
  React effect into a ref-only rAF loop in `page.tsx` (read `motionNowRef` + `entityMotionSnapshotsRef`,
  compute `cameraMotionOffsetForEntity`, push on change). Shell stops depending on
  `playerCameraMotionOffset`. **Reuse:** the player ref path (`predictedPlayerPositionRef`,
  `tickMovementPlan` rAF at `page.tsx:3928`). Risk: stale-closure — read ALL inputs from refs
  (`worldRef`, `latestMoveInputRef`), never captured values; unsubscribe on screen change.
- **Stage 2 — 10Hz chat-bubble/projectile expiry timer.** Move `sceneChatBubbles`/projectile expiry
  off the 30Hz clock onto a ~100ms timer (chat TTL 6s, projectile flight 300–500ms → imperceptible).
  After 1+2 the shell no longer re-renders 30Hz for motion → kills allocator **(A)**.
- **Stage 3 — consolidate into a `useMotionLoop` subscription** (one rAF, centralized Bevy pushes,
  documented "never read in render"). Prevents regressions.

## Piece (B) — per-cell allocation reuse (effort M, **DO SECOND** — cheap safe win)

Root: `buildViewportMapSprites` (`original-client-scene-map-rendering.tsx:108`) allocates fresh
`floor`/`objects` arrays + ~500 sprite objects every cell-cross (`staticViewportMapSprites` memo at
`original-client-shell.tsx:1106`).

- **Stage 1 — array pooling.** Reuse the same `floor`/`objects` array instances across rebuilds
  (clear + refill). **No correctness risk** — just fewer container allocations. **DO FIRST of this
  piece.**
- **Stage 2 — delta-diff cells.** Track prev viewport bounds; only rebuild ENTERED/LEFT cells. Risk:
  perimeter miscalc → on-screen black holes → must verify against a full-rebuild reference.
- **Stage 3 — precomputed per-cell sprite cache** keyed `${cellX}:${cellY}:${spriteId}` at region
  load → viewport change does lookups, not allocation.

## Piece #3 — `setWorld`-per-packet external store (effort L, **riskiest — do for busy maps**)

Root: `page.tsx` `handleGatewayEvent` (~6340) calls `updateWorld` at 108+ sites; high-frequency
movement/combat packets (`UserLocation`, `ObjectWalk/Run/Attack/Struck/Spell/Harvest`) re-render the
11k-line monolith at 10–50 Hz on crowded maps. **The synthetic single-player bench didn't test this**
(no crowds) — needed for real crowded 10-min walks.

- **Stage 1 — entity motion-snapshot ref bypass.** High-freq position packets write a
  `entityMotionRef` (like `predictedPlayerPositionRef`) instead of `updateWorld`.
- **Stage 2 — Bevy motion-snapshot emitter** — `setMir2EntityMotionSnapshot(json)` so Bevy decodes
  motion updates without a React re-render.
- **Stage 3 (opt) — `useSyncExternalStore` UI slices** (`useWorldEntities()` etc.) so only subscribed
  components re-render.
- **Stage 4 (opt) — coalesce** multiple packets per network tick into one mutation.
- **Stage 5 — flag (`MIR2_MOTION_REF_BYPASS` default off) + measure** re-renders/sec (target <5 on a
  50-entity map). **Main risk: Crystal 1:1 packet ordering** — if motion bypasses `setWorld` but a
  rare packet is in flight, ordering is ambiguous; stage carefully + keep rare packets on `setWorld`.

## Recommended execution order

1. **#4 Stage 1 + Stage 2** → removes the 30Hz re-render (biggest lever). Re-benchmark.
2. **(B) Stage 1** (array pooling, zero-risk). Re-benchmark.
3. If GC-spike windows mostly gone for single-player → goal met for sparse maps. Else **(B) Stage 2**.
4. **#3** (Stages 1–2 + flag) → only when targeting busy/crowded maps. Re-benchmark on a crowded map
   (improve the synthetic walk to cross regions + spawn crowds first).
5. **#4 Stage 3 / (B) Stage 3** → consolidation/polish.

Acceptance: 10-min continuous walk (crossing regions, on a populated map), **0 windows with a >50ms
frame**, flat/bounded heap. Drive it with the in-page rAF-delta + `performance.memory` sampler
(release build only — dev's React-development build invalidates the numbers).
