# mir2 Web-Client Render-Performance Remediation Roadmap

> Produced 2026-06-22 by a 10-agent deep analysis (6 readers → 3 architects → 1 synthesizer)
> over the *current* `apps/web` code, then spot-verified by hand. Supersedes the stale
> "setWorld-per-packet / 4-hot-paths" framing: packet ingestion is **already** rAF-coalesced.
> All claims cite `file:line`. Every stage is additive, backward-compatible, and reversible.

## Executive Summary

The real bottleneck is **not** packet ingestion — that's already rAF-coalesced to one `setWorld`/frame (page.tsx:1447). It's two things stacked on the same main thread:

1. **Per-packet CPU on the WS message task** — combat packets each fire 2-4 full `{...world}` clones + O(N) `entities.map` walks (page.tsx:6851-6892), plus a fixed prologue tax (dead `t("log.recv")` translation + 50-element history allocs) on *every* packet.
2. **Whole-tree re-render fan-out** — the single coalesced flush re-renders the entire 12.7k-line monolith because `world` is one monolithic `useState` (page.tsx:1429) with no selector boundary, so the existing memos (GameUiScene game-ui-scene.tsx:409) are defeated by `world={world}`'s fresh identity each flush, and the 1,155-node viewport tile-grid (shell.tsx:2096) reconciles in full.

The headline fix is a **staged decoupling**: instrument first, then delete dead per-packet work and collapse combat clones (cheap, near-zero risk), then memo-gate + stabilize identities, and finally **subscribe consumers to slices of the already-existing `worldStoreRef` store via `useSyncExternalStore`** — reusing the infrastructure that already feeds Bevy, so React renders less while the canvas keeps seeing every frame.

Two charter-premise corrections confirmed across the analyses:

- **There are zero real `flushSync()` calls** (only an avoidance comment at page.tsx:1959-1967 — the predicted-self path is *already* decoupled).
- **React.memo is not 0** — 3 boundaries exist (scene-visual-layers.tsx:88, game-ui-scene.tsx:409, mobile-controls.tsx:393); they're just defeated by the monolithic `world` prop.

The fix *extends* the existing decoupling discipline rather than inventing it.

---

## Performance Targets (the bar, from Crystal)

Crystal's own render loop fixes the target explicitly: **60 FPS = a 16.67 ms frame budget** — `const int TargetUpdates = 1000 / 60; // 60 frames per second` (Crystal/Client/Forms/CMain.cs:347), VSync-locked via `PresentInterval.One` (DXManager.cs:67; `FPSCap` default `true`, Settings.cs:76). Crucially Crystal **separates update from draw** (`Application_Idle`: `UpdateEnviroment()` every iteration, `RenderEnvironment()` only when `IsDrawTime()` — CMain.cs:106-115) and mutates object state **in place** on packet receipt, so it has **no equivalent of our per-packet full-UI rebuild** — which is why the original never shows a 200 ms handler. Smooth scroll is interpolated over N sub-frames (`OffSetMove = CellHeight * i/count`, MonsterObject.cs:398-410), so **one dropped frame = a visible step** → the bar is steady-state, not average.

| Metric (our harness) | Parity bar | Acceptable floor | Today |
|---|---|---|---|
| Render FPS (qa-soak `fpsMedian`) | **60** | ≥ 50 | Bevy ~119 but React stalls starve it |
| Per-frame main-thread work | **≤ 16.7 ms** | ≤ 50 ms (no Chrome `[Violation]`) | click 197 ms, rAF 138 ms = 3–12× over |
| Frame-time p95 (qa-soak `frameTimeP95`) | **≤ 18 ms** | ≤ 33 ms | — |
| Max frame gap / hitches (qa-soak `maxFrameGapMs`, qa-load-stress `hitchCount`) | **≈1 frame, 0 hitches** | < 100 ms | 130–150 ms GC hitch every ~10–15 s |
| Per-handler ceiling (Stage-0 `?perfDiag`) | **single-digit ms** | < 50 ms | so message+click+render all fit one 16.7 ms frame |

120 Hz stretch (the user's display): 8.3 ms/frame — the Bevy canvas can already do it; affects only scroll smoothness, not parity. **Stage 0 exists to measure against this table** — it is the pass/fail bar every later stage is judged on. Measure in a **release** build on a cool host (dev's React-development build alone inflates every number).

---

## Staged Roadmap

### Stage 0 — Instrument the actual `[Violation]` symptom (prerequisite, no app behavior change)

The blocking gap: **no harness captures Chrome's `[Violation]` warnings.** They arrive at verbose level / the CDP `Log` domain, and the only level-filtering harness keeps `type==='error'|'warning'` (qa-load-stress.mjs:221), so every violation string is silently dropped. Without a counter, every later stage is unfalsifiable.

| Change | File:line |
|---|---|
| Add `await client.send('Log.enable')` + collect `Log.entryAdded` matching `/handler took\|Violation/`, bucketed by handler name (message/click/setInterval/requestAnimationFrame) | `apps/web/scripts/qa-load-stress.mjs:221` |
| Install in-page `new PerformanceObserver({type:'longtask'})` draining total long-task ms/window into the existing report row (the standards-based equivalent of the violations) | `apps/web/scripts/qa-load-stress.mjs:300` |
| Add an OPTIONAL `?perfDiag=1`-gated `performance.now()` wrap around the `handleGatewayEvent` switch body (symmetric with the existing `?movementDiag=1` gate), emitting p95 per-packet handler ms to `window.__mir2PerfDiag` — isolates the message-handler source from React render time | `apps/web/app/page.tsx:6346` (gate pattern at `page.tsx:1980`) |
| Capture BASELINE on a **cool host**, non-hog "aggressive" phase only | `apps/web/scripts/qa-load-stress.mjs:1126` |

- **Root cause addressed:** measurement gap — the literal violations the charter cites can't be trended today.
- **Violation reduced:** none directly; makes all four (message x28 / click 197ms / setInterval x11 / rAF 138ms) countable.
- **Do NOT** read jank from `__mir2CacheMetrics` — `firstPlayableMs` (asset-cache-registrar.tsx:1292) is a one-shot load milestone with no steady-state channel (benign).
- **Risk:** low (harness-only + one default-off gated wrapper).
- **Verify:** baseline `npm run qa:load-stress` (detectHitchCorrelation: approxFps/hitchCount/p95DtMs/maxDtMs at qa-load-stress.mjs:985) + `npm run qa:soak` (fpsMedian/frameTimeP95/maxFrameGapMs at qa-soak.mjs:344). Judge the hog-OFF phase; the synthetic CPU-HOG (qa-load-stress.mjs:403) is calibration, not the target.

---

### Stage 1 — Delete per-packet dead work (the prologue tax) [quick win, near-zero risk]

Every packet pays a fixed tax *before the switch even runs*, independent of which case handles it.

| Change | File:line |
|---|---|
| Remove the unconditional `appendLog(t("log.recv", [event.packet]), "network")` — `appendLog` returns immediately for `tone==='network'` (page.tsx:3991), so the full `text()` + `formatTemplate` split/join (localization.ts:82) runs per packet and the result is **discarded**. Pure dead CPU. | `apps/web/app/page.tsx:6403` |
| Gate the debug bookkeeping (`debugEvent` literal + `__mir2GatewayEventHistory = [debugEvent, ...].slice(0,50)` 50-element array alloc, and the parallel movement-packet slice at 6406-6413) behind an opt-in flag (mirror `?movementDiag`). Keep the cheap single `__mir2LastGatewayEvent` assignment. | `apps/web/app/page.tsx:6352-6364` |
| Confirm `recordDebugEvent('packet-in',...)` early-returns when its ring is disabled; gate it behind the same flag if it always pushes | `apps/web/app/page.tsx:6421` |

- **Root cause addressed:** per-packet prologue tax (wasted translation + two 50-element array allocs + ring pushes on every message).
- **Violation reduced:** "message handler took N ms" (x28) — trims a fixed constant off every invocation.
- **Risk:** low — pure removal of provably-discarded work + opt-in-gating of debug-only allocations. Trivially revertable.
- **Verify:** Stage-0 message-violation count + `?perfDiag=1` p95 handler ms drop; `npx tsc --noEmit` == 0; `npm run test:frontend-logic` green.

---

### Stage 2 — Collapse combat clones: one `updateWorld` per packet, refs for VFX [quick win, low risk]

The dominant message-handler cost. The three hottest combat packets each fire 2-4 separate `updateWorld` calls, every one a full `{...current}` WorldState clone + O(N) `entities.map`.

| Change | File:line |
|---|---|
| **ObjectAttack** — fold `updateWorldEntityFromLocationPacket(payload)` + `markWorldEntityAttack(payload)` into ONE `updateWorld` whose single `patchEntityInList` updater applies both location patch and attack-animation fields in one `entities.map` (2 clones → 1) | `apps/web/app/page.tsx:6851-6853` |
| **ObjectStruck** — same fold for `updateWorldEntityFromLocationPacket` + `markWorldEntityStruck` (was the worst: 2 clones here + a 3rd from the bus flash) | `apps/web/app/page.tsx:6859-6861` |
| **ObjectSpell/ObjectMagic** — fold `updateWorldEntityFromLocationPacket` + `markWorldEntityMagic` into one updater; leave `spawnRangeProjectile` as-is | `apps/web/app/page.tsx:6884-6892` |
| Stop the gameBus VFX subscribers (`markEntityStruckFlash`, `pushDamageFloaterFromBus`) from **re-entering** `updateWorld` for a 3rd/4th clone per hit. Drive the struck-flash + damage floater either via a ref + the existing motionNow overlay tick (overlays already read `world.damageFloaters`/`motionNow`) OR batch them into the SAME single case-handler updater. **Keep the bus emit that plays SOUND** (no clone). | `apps/web/app/page.tsx:1528-1532`, `markEntityStruckFlash` ~`page.tsx:9187/9257`, `pushDamageFloater` ~`page.tsx:9200` |
| Remove the redundant `worldRef.current = nextWorld` **inside** the movement updater — `updateWorld` already assigns at page.tsx:1442 (double-write) | `apps/web/app/page.tsx:6790` |

- **Root cause addressed:** combat packets each cause ~4 full clones + O(N) scans; `message-handler` time scales with (clones/packet × visible-entity-count × packets/tick).
- **Violation reduced:** "message handler took N ms" (x28) — the single largest reduction (≈4× fewer clones on the hottest packets) + proportional GC relief.
- **Risk:** low — self-contained handler-body edits; **fold by composing the existing helper bodies** into one `patchEntityInList` updater, do not reimplement. Preserve every field the renderer reads (struckStartedAt/struckUntil/attackAnimation — WorldEntity page.tsx:488-496).
- **Verify:** `npm run qa:vfx` (struck/attack/spell animation assets) + combat-visual-feedback path — struck flash + floaters + attack animation must still fire. Stage-0 message-violation count + `?perfDiag=1` p95 drop sharply in a combat burst. `npx tsc --noEmit` == 0.

---

### Stage 3 — Memo-gate the heavy subtrees + stabilize their inputs [structural-lite, low/medium risk]

The existing memos are inert because their inputs change identity every render. This stage makes them actually hold — and removes the ExtraWindows adapter tax.

| Change | File:line |
|---|---|
| Memoize `self` (`world.entities.find`) keyed on `(world.entities, world.playerObjectId)`, and `selectedEntity` (`displayEntities.find`) keyed on `(displayEntities, world.selectedObjectId)` — these feed 4 subtrees (shell.tsx:2140/2160/2179/2309); their fresh identity is a memo-blocker | `apps/web/app/page.tsx:1765`, `apps/web/app/page.tsx:1813` |
| Gate each ExtraWindows adapter behind its `show*` flag (e.g. `showMarket ? adaptMarketListings(world.stage5Systems.auction) : EMPTY_MARKET`) and dedupe the **double** `adaptActiveRankingPage` (page.tsx:11248) into one `useMemo`. Stops 15 adapter passes + 17 object-literal allocs from running every flush when windows are closed (the common case). | `apps/web/app/page.tsx:11242-11258` |
| Wrap `ExtraWindows` (a plain function today) in `memo` so once its prop bag stops changing every flush it skips | `apps/web/app/components/original-client-extra-windows.tsx:290` |
| `useCallback` the inline handler closures forwarded to the shell (`onApproachTarget`, `onPrimaryTargetAction`, the 4 `onViewportTile*`) so existing/new child memos can hold | `apps/web/app/page.tsx:11202-11236` |
| Wrap the three unmemoized per-frame DOM scene layers in `memo` (their props are already shell-memoized) so the 30Hz motionNow tick (shell.tsx:851-860) skips them when nothing moved | `scene-visual-layers.tsx:320`, `scene-overlays.tsx:620`, `scene-map-rendering.tsx:43` |
| Memoize `MiniMapScene` on `(world.entities, bounds)` and `return null` (not CSS-`hidden`) when collapsed — it currently reconciles one `<rect>`/entity every flush while hidden | `apps/web/app/components/original-client-map-panels.tsx:282/398` (CSS-hide at `:307`) |

- **Root cause addressed:** memo boundaries defeated by unstable inputs; unconditional adapter/allocation tax; motionNow re-rendering unmemoized scene layers.
- **Violation reduced:** "message handler took N ms" (per-flush CPU half) + the 30Hz steady-state cost behind "rAF 138ms".
- **Risk:** medium — `memo`/`useMemo`/`useCallback` are identity-only and **inert-but-safe** if a dependency is still unstable (worst case = no speedup, never a regression). Must pair each memo with stable inputs or it's inert.
- **Verify:** **render-count probe** (useRef counter or React DevTools Profiler) — a window-toggle click must increment ONLY that window's counter, not the scene; a movement burst must not re-render closed windows. Stage-0 approxFps↑ / p95DtMs↓ on the non-hog phase. `npx tsc --noEmit` == 0; `npm run test:frontend-logic` green.

---

### Stage 4 — Replace the 1,155-button tile grid with one canvas-level pointer handler [structural, medium risk]

The single largest *fixed* per-flush reconcile and a prime driver of the 197ms click.

| Change | File:line |
|---|---|
| Replace the 33×35 = 1,155-element tile-hit `<button>` grid (each with 6 inline closures) with a SINGLE absolutely-positioned overlay `<div>` carrying `onPointerDown/Move/Up/ContextMenu`, converting client coords → tile via the existing `scenePointFromMouseEvent` helper. Deletes ~1,155 host-node diffs + ~6,930 closures/flush outright. | `apps/web/app/original-client-shell.tsx:2096` (grid dims: `scene-layout.ts:4-9`, `original-ui.ts:64-65`) |

- **Root cause addressed:** the 1,155-node grid reconciling in full every flush; it exists only as a click-capture layer over the Bevy canvas (canvas at shell.tsx:2069 carries no world props and owns its pixels), so a single pointer handler + math is behaviorally equivalent.
- **Violation reduced:** "click handler took 197ms" (primary) + "message handler took N ms".
- **Risk:** medium — pointer-to-tile mapping must reproduce all four existing handler semantics exactly (walk/run/step/direction-intent at page.tsx:11208-11213). Gate behind Stage-0 harness; an off-by-one in tile conversion mis-routes clicks.
- **Verify:** `npm run qa:load-stress` drives real CDP click + arrow input — movement must not regress (in-viewport-click constraint from the combat-QA harness); Stage-0 click-violation count drops. Manual: clicking a tile still walks there; right-click context still fires.

---

### Stage 5 — Selector-store migration: subscribe consumers to `world` slices via `useSyncExternalStore` [structural, medium→high risk, do last]

The structural payoff — what memo alone *cannot* achieve, because every memoized child still receives a fresh `world` prop today. Reuses the **already-existing** `worldStoreRef` store that already feeds Bevy.

**5a — Add the React read primitive (no consumers migrated yet):**

| Change | File:line |
|---|---|
| Add `useWorldSelector<S>(store, selector, isEqual?)` built on `useSyncExternalStore`, implementing the `useSyncExternalStoreWithSelector` cache (memoize last result; recompute only on store change; apply isEqual) so **derived selectors don't loop**. Pure module, imports React only here (keeps store framework-agnostic). | NEW `apps/web/lib/world-model/use-world-selector.ts` |
| Re-export from the barrel | `apps/web/lib/world-model/index.ts` |
| Reuse the EXISTING `subscribe(selector,listener)` + `getSnapshot()` (returns live `state`); add a thin `subscribeFull(listener)` for the hook's subscribe arg if needed — additive, no signature change | `apps/web/lib/world-model/store.ts:105/227-242` |
| NEW test in the `test:frontend-logic` chain: direct-slice selector fires once per matching change; unrelated-slice change does NOT fire; derived selector with isEqual does not loop | NEW `apps/web/scripts/test-world-store-hook.mjs` + `package.json` |

**5b — Migrate low-frequency consumers first (lowest blast radius):**

| Change | File:line |
|---|---|
| **OnchainMinePanel** (reads ONLY `world.mineNodes.find`, yet re-renders every flush) → `useWorldSelector(store, s => s.mineNodes.find(...))`. Proves the pattern. | `apps/web/app/page.tsx:11280-11309` (read at `:11294`) |
| **ExtraWindows** window slices → `s.stage5Systems.X`, `s.questLog`, `s.activeBuffs`, `s.rankings` selectors (still open-gated + memoized from Stage 3), so windows track only their low-frequency slice + `show*` flag | `apps/web/app/page.tsx:11242-11257` |

**5c — Migrate the HUD/scene so `memo(GameUiScene)` finally holds:**

| Change | File:line |
|---|---|
| Stop passing `world={world}` to GameUiScene; pass the store (or narrowed slices) so it subscribes per-slice. This is the line that busts the memo every flush. | `apps/web/app/original-client-shell.tsx:2236` (memo at `game-ui-scene.tsx:409`) |
| MainHud reads `hp/mp/level/gold/weight` via one shallow-equal selector; MiniMapScene reads `s.entities` | `original-client-overlays.tsx`, `map-panels.tsx:308` |
| Keep the legacy `world={world}` path behind a `?selectorHud=1`-style flag for the first ship → instant rollback | `apps/web/app/page.tsx:11114` |

- **Root cause addressed:** monolithic `world` read across the whole tree; the one flush re-renders everything because consumers read the whole `world` identity.
- **Violation reduced:** "message handler took N ms" (render half) + "rAF 138ms" (full-tree commit landing on Bevy's paint frame) + 197ms click fan-out.
- **Risk:** medium→high. **Selector correctness is load-bearing:** a selector returning a fresh object (`s => ({hp,mp})`) loops React — Stage 5a's cache + the test guard against this; prefer direct-slice selectors (`s => s.entities`) which the store keeps identity-stable via `{...current}` (store.ts:248). **Tearing:** migrate each component *fully* in one step (never half prop / half selector). **Two parallel `WorldState` types** (page.tsx:673 vs lib/world-model/types.ts) must stay structurally assignable.
- **Verify:** render-count probe — GameUiScene renders drop from once-per-flush to once-per-stat-change; ExtraWindows ≈0 renders when closed. Stage-0 approxFps↑, maxDtMs/hitchCount↓; `npm run qa:soak` maxFrameGapMs↓ (the 138ms rAF should disappear). The new selector unit test green. The social/items/economy/magic CDP QA loops still pass.

---

## Consolidated Risk Register

| # | Risk | Mitigation |
|---|---|---|
| R1 | **Machine-load contamination** — qa-load-stress maxLead/snaps/corrections scale with host load + WS-resync. A noisy host fakes or masks improvement. | Run before/after on the SAME cool/idle host; judge the hog-OFF "aggressive" phase; corroborate with qa-soak's median-fps/p95 time-series, not a single number. |
| R2 | **Stage 5 selector loop/tearing** — `useSyncExternalStore` requires a cached `getSnapshot`; a derived selector returning a fresh object loops. | Stage 5a implements the `useSyncExternalStoreWithSelector` cache + a unit test asserting no-loop; prefer direct-slice selectors (identity-stable via store.ts:248). |
| R3 | **Stage 4 pointer-parity** — collapsing the grid must reproduce walk/run/step/direction-intent semantics (page.tsx:11208-11213) exactly. | Drive real CDP click + arrow input via qa-load-stress; off-by-one tile mapping caught by movement regression. |
| R4 | **Stage 2 dropped animation** — folding 2-4 updaters into one must not drop struck/attack/magic fields (WorldEntity page.tsx:488-496). | Compose existing helper bodies (don't reimplement); verify via qa:vfx + combat-visual-feedback. |
| R5 | **Inert memos** — Stage 3 memos defeated if `selectedEntity`/`self`/handler closures stay unstable. | Pair every memo with `useMemo`/`useCallback` for its inputs; gate adoption on the render-count probe. |
| R6 | **Two parallel WorldState types drift** (page.tsx:673 vs lib/world-model/types.ts). | `npx tsc --noEmit` == 0 after every stage; add fields additively to both when a selector needs one. |
| R7 | **Bevy regression** — never touch the emitter (page.tsx:3728-3739) or `toBevyWorldSnapshot` (page.tsx:1306-1317); keep `store.set` (page.tsx:1443) unconditional. | Verify the canvas still animates after each structural stage via qa-render-sweep. |

## Invariants Every Stage Must Preserve

- **Additive/optional WorldState↔DisplayWorld** — `world={world}` (page.tsx:11114) relies on structural compat with `DisplayWorld` (original-client-types.ts:254); never change DisplayWorld's existing fields. `npx tsc --noEmit` MUST be 0.
- **Single rAF-coalesced `setWorld`** (page.tsx:1447) + **synchronous `worldRef.current`** (page.tsx:1442) + **`worldStoreRef.set` Bevy feed** (page.tsx:1443) — React may render less, but Bevy must see every frame.
- **No `flushSync`** (the avoidance at page.tsx:1959-1967 is an existing invariant) and the `queueMicrotask` predicted-self path (page.tsx:1968-1974) stay intact.
- **Crystal 1:1 / presentation-only** — no protocol/gateway/sim change, no model identifiers, no asset-manifest/SW-namespace churn (asset-cache-registrar.tsx:717-731 reloads on namespace change).
- After every stage: `npm run test:frontend-logic` green + `npx tsc --noEmit` == 0.

## Sequencing Rationale

Stages 0-2 are **shippable today as small additive PRs** with near-zero risk and recover the bulk of the *message-handler* cost (the dominant violation cluster). Stage 3 makes the existing memos hold. Stage 4 deletes the largest fixed reconcile (the click violation). Stage 5 — the highest-leverage but highest-care change — ships **last and incrementally** (primitive → single-slice OnchainMinePanel → windows → HUD), riding entirely on infrastructure that already exists, behind a rollback flag. **An engineer can start Stage 1 immediately**: it's two edits in `page.tsx` (delete line 6403, gate 6352-6364) verified by re-running the Stage-0 baseline.

---

**Key files:** `apps/web/app/page.tsx` (handlers 6346-9000, updateWorld 1436-1453, mounts 11107-11258), `apps/web/app/original-client-shell.tsx` (scene 2086-2188, tile grid 2096), `apps/web/lib/world-model/store.ts` (the reusable store), `apps/web/scripts/qa-load-stress.mjs` + `qa-soak.mjs` (the metric harnesses).
