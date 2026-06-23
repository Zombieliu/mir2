# Movement “卡顿” Investigation & Entity-Atlas Streaming Fix — 2026-06-24

> **Handoff / knowledge-capture doc.** Origin: a user report that *“this game’s movement
> problem has been worked on for 2 months and still isn’t fixed.”* The investigation
> **resolved the movement complaint** and, in doing so, uncovered a **separate, still-open
> render-perf item** (entity-atlas per-frame streaming). This doc records both — verified
> findings, measurements, `file`/symbol references, the measurement recipe, and the exact
> next steps — so the atlas work can resume cold without re-deriving anything.
>
> Line numbers drift (this repo’s convention) — symbols below are **grep anchors**, not
> fixed lines.

---

## TL;DR

- **Movement: RESOLVED — and it was mostly NOT a movement-code bug.** Three *environmental*
  causes: (1) the per-frame-easing fix **#148 sat unmerged** for days; (2) the local gateway
  proxy on **`:7110` was dead**, so every local test silently failed to reach the backend;
  (3) **5 stale dev servers** saturated the host → frame drops (which amplify any jank).
  After merging #148 (+#156), pointing the client at the **live gateway `:7141`**, and killing
  the stale servers, `qa-load-stress` on a cool host reports **snaps=0, corrections=0,
  held-keyboard PASS** across gentle / aggressive / aggressive+hitch. The prediction model is
  healthy; the planned “Stage 2” model-alignment work is **deferred** (corrections=0 → the
  divergence it would fix isn’t manifesting).

- **Residual “走两步卡顿” (stutter every couple of steps): a SEPARATE render-perf issue —
  entity sprite-frame streaming.** The Bevy entity renderer fetches individual frame PNGs
  (`/original-ui/CArmour/00/N.png`, `Monster/.../N.png`, …) on first encounter. **0 long-tasks**
  (not React main-thread); hitches 45–63 ms; cold walk 53 fetches → cached re-walk 16.

- **Two root causes for why the prebuilt atlas wasn’t eliminating the streaming:**
  1. **`force-cache` stale-manifest bug — FIXED.** `loadBevyEntityAtlasManifest` fetched the
     manifest with `cache:"force-cache"`; after an atlas regen the browser kept serving the
     **stale** manifest, so coverage ran against old rects. Fix → `cache:"no-cache"`. With that
     + a covering atlas, the atlas **fires** and per-frame streaming **halved (~50 → 31)**.
  2. **All-or-nothing resolver.** `prebuiltBevyEntityAtlasCoversSources` requires EVERY visible
     source key in one candidate; one unpacked frame → reject → full live-build → re-stream
     everything. → **partial-cover resolver** (atlas for covered frames, live-build only the
     rest as an extra page). **Implemented + tsc-clean but NOT yet verified working** (covered
     frames still stream in the measure — a bug to chase).

---

## Part 1 — Movement (RESOLVED)

### The complaint
User reported **all four symptoms simultaneously**:
`卡顿后瞬移` (stall→jump) · `过冲/反弹` (overshoot/snap) · `发飘/掉帧` (general jank) ·
`点击走错/原地不动` (mispath / no-move).

### The real causes (environmental, not movement code)

1. **#148 was unmerged.** The residual overshoot/snap fix
   ([#148](https://github.com/Zombieliu/mir2/pull/148) *“ease self-render to ≤1 tile/frame”*)
   was an **open PR**, not on `main`. `stepMovementTowardWithinCap` / `easeSelfRenderTile` were
   absent from the running code (grep found them only in a doc). Merged **2026-06-23** (squash,
   `--admin` past the billing-blocked CI) together with
   [#156](https://github.com/Zombieliu/mir2/pull/156). *(A memory note had wrongly recorded
   #148 as landed — since corrected. **Lesson: `gh pr list --state open` before diagnosing a
   regression.**)*

2. **Dead `:7110` gateway proxy.** The web client defaults to `ws://127.0.0.1:7110/ws`
   (`LOCAL_GATEWAY_WS_URL`, `apps/web/app/page.tsx`). That port held a stale
   `node /tmp/mir2-ws-proxy.js` that **refused** WS upgrades. The real gateway (`mir2-gateway`
   binary) was on **`127.0.0.1:7141`**. So all the stale dev servers (and the qa harness) could
   log in from cache but produced **zero movement** — moves went nowhere. Pointing the client
   at `?gatewayWs=ws://127.0.0.1:7141/ws` (page.tsx honors `gatewayWs` on localhost) restored
   full play. **This is why the qa harness first reported “0 movement” — it hit the dead proxy.**

3. **Host saturation.** Four stale `next dev` servers from old worktrees ran simultaneously
   (`:3026 :3060 :3099 :3010`) → CPU saturation → frame drops, which *amplify* movement jank
   (memory: snaps cluster within 250 ms of a rAF hitch). Killed them.

### Verification — cool host, live gateway, #148 build

`qa-load-stress.mjs --baseUrl "http://localhost:3070/?gatewayWs=ws://127.0.0.1:7141/ws"`:

| phase | snaps | corrections | held-key | fps |
|---|---|---|---|---|
| gentle | 0 | 0 | — | 106 |
| aggressive (run + hard reversals) | 0 | 0 | — | 103 |
| aggressive + **hitch** (143 forced CPU stalls) | 0 | 0 | — | 39 |
| held-keyboard | — | — | **PASS** | — |

`reproduced: no`. Plus a live CDP playthrough (Scout Lv7, BichonProvince): smooth run + hard
reversals, **0** movement-console corrections, minimap advanced normally.

### Conclusion & deferred work

The movement prediction model is **healthy on a working connection**; the four faces were
dominated by the three environmental causes. **Stage 2** is **deferred**:

- **2a — soften the 400 ms correction input-freeze** (`CRYSTAL_CORRECTION_BLOCK_MS`,
  `reconcileMovementAck` in `components/original-client-movement-controller.ts`): corrections=0
  → the freeze almost never fires; not worth the risk now. **Coupling caveat:** the 400 ms
  freeze is a *damper* on prediction/server divergence — softening it **before** reducing
  divergence (2b) risks a correction storm. Order any future Stage 2 as **2b → 2a**, measure first.
- **2b — align the client collision model to the server** (client predicts against only
  *visible* entities + loaded map cells; server `is_blocked_tile` / `has_blocking_entity` also
  enforce dynamic doors, decor, terrain, and all entities): corrections=0 means the divergence
  isn’t manifesting in common play. Revisit **only** if a symptom reproduces in
  **dense-entity / door / map-edge** scenarios (the qa field was sparse, peakEntities 3–6).

---

## Part 2 — Entity-atlas streaming stutter (IN PROGRESS)

### Diagnosis: per-frame sprite streaming (not movement, not React)

Instrumented the live client (CDP `fetch` hook + rAF-gap sampler + `PerformanceObserver`
longtask). A ~9 s walk in BichonProvince:

- **0 long-tasks** → not the React main thread.
- **53 sprite-frame PNG fetches** (`/original-ui/CArmour/00/N.png`, `CHair`, `CWeapon`,
  `Monster/.../N.png`) → the Bevy entity renderer fetching individual animation frames on
  first encounter.
- **Cached re-walk: 53 → 16** → confirms cold-cache first-encounter streaming.
- Hitches 45–63 ms (mild; ~3–7 dropped frames), aligned with the fetches (decode / GPU upload),
  not main-thread JS.

### How the entity atlas works (`apps/web/app/original-client-shell.tsx`)

- `collectBevyEntityAtlasSources` → the visible set’s source keys (each entity’s current frame
  + an **animation preload superset** across directions/frames).
- `resolveBevyEntityAtlasSnapshot` → **persistent (IDB) → prebuilt → live-build**.
- `loadPrebuiltBevyEntityAtlasSnapshot` → for each manifest candidate,
  `prebuiltBevyEntityAtlasCoversSources(candidate, sourceKeys)` must contain **EVERY** source
  key (**all-or-nothing**); multi-page candidates build via `buildMultiPagePrebuiltSnapshot` (#122).
- `buildBevyEntityAtlasSnapshot` (live) → loads every source PNG, packs one canvas, returns
  RGBA pixels. **This is the streaming.**
- Producer `buildBevyEntityRenderState` → one render-atlas per page (`atlases` carry `imageUrl`,
  `atlasImages` carry `pixels`), each layer routed to its page via `rect.pageIndex`. The Rust
  runtime is already multi-atlas (no Rust change needed for multi-page or partial-cover).

### Finding 1 — `force-cache` stale-manifest bug (FIXED)

`loadBevyEntityAtlasManifest` fetched `BEVY_ENTITY_ATLAS_MANIFEST_URL` with `cache:"force-cache"`.
After regenerating the atlas (new manifest content, **same URL**), the browser served the
**stale** cached manifest → coverage ran on old rects → atlas never matched → live-build.
**This invalidated the first “covering atlas still doesn’t fire” test.** Fix → `cache:"no-cache"`.
After the fix + a covering repack the atlas **fires** (loads `starter-bichon-base.png` + `-p1/-p2`)
and per-frame streaming **halved (~50 → 31)**.

> The entity atlas on `main` is a **single 4096² page**, `schemaVersion 1`, roots = player gear
> + `NPC` + `Monster/000/010/012/139` — it does **not** cover e.g. `Monster/003`, so it never
> *fully* covers a real scene. #122 makes it multi-page; the committed #122 pack still uses the
> same partial roots. A repack covering all **local** libs (7 monsters + 16 NPCs + gear) does
> cover the local Bichon scene — but the full game has **200+** monsters, so broad coverage
> needs the **R2 corpus pack** (release lane).

### Finding 2 — all-or-nothing resolver → partial-cover (IMPLEMENTED, NOT VERIFIED)

Even with a covering atlas + cache fix, streaming only **halved** (not zero): as the viewport
shifts, the instant **any** single visible frame isn’t packed, the all-or-nothing check rejects
the whole atlas → full live-build → re-stream everything (incl. covered frames).

**Partial-cover resolver** (the keystone): use the best-covering candidate’s resident pages for
the frames it has, and live-build **only the uncovered remainder** as one appended `pixels` page
in the same multi-page snapshot. The page model already mixes `imageUrl` + `pixels` pages and
routes by `rect.pageIndex`, so it’s additive — **TS-only, no Rust.**

Implemented in `original-client-shell.tsx`:
- `buildPartialCoverBevyEntityAtlasSnapshot(candidate, sources, key)` — prebuilt pages (covered)
  + one live page (uncovered, `pageIndex = N`), merged rects.
- Hook in `loadPrebuiltBevyEntityAtlasSnapshot` after the full-cover loop: pick the candidate
  covering the most sources; if `0 < covered < all`, call the partial builder.
- `npx tsc --noEmit` = **0**.

**BUT in-browser it did NOT reduce streaming** — **73** per-frame fetches, and **COVERED** frames
(`CArmour/00`, `CWeapon/00`, `Monster/000/003/139` — all packed) **still streamed**. So the partial
path either isn’t firing or treats covered frames as uncovered. **NOT verified working — do not ship.**

**Suspected bug:** `source.key` vs candidate `rect.key` **format mismatch** (collectSources/
live-build key vs manifest rect key), or `persistBevyEntityAtlas` (keyed by `key`) caching a bad
partial snapshot. The full-cover path *did* fire earlier (pages loaded) → keys match *sometimes*
→ pin it by exposing the resolve decision.

### Measurements — BichonProvince, cold dev server, same walk

| config | atlas pages load | per-frame fetches | note |
|---|---|---|---|
| baseline (#148, single-page atlas) | no | ~50 | reject (Monster/003 unpacked) → full live-build |
| #122 multi-page (committed roots) | no | 44 | atlas still misses Monster/003 → reject |
| covering repack, **force-cache** (stale) | no | 49 | **stale manifest** → coverage on old rects |
| covering repack + **no-cache** | **yes (3 pages)** | **31** | cache fix → atlas fires; 31 = all-or-nothing on viewport shift |
| **+ partial-cover** | yes (3 pages) | **73** (covered frames still stream) | **bug — not working** |

---

## Reproduction / measurement recipe

- **Dev server from a worktree:** `npx next dev --webpack -p 3070`. The `--webpack` flag
  sidesteps the Turbopack symlink panic when `node_modules` is symlinked from the main checkout;
  the Bevy runtime is prebuilt in `public/bevy-runtime/` so `next dev` runs directly.
  **Remove the `node_modules` symlink before any commit.**
- **Find the real gateway** (the default `:7110` may be a dead proxy):
  `lsof -nP -iTCP -sTCP:LISTEN | grep mir2-gateway` → e.g. `127.0.0.1:7141`. Open
  `http://localhost:3070/?mir2Debug=1&gatewayWs=ws://127.0.0.1:7141/ws`.
- **Login** `demo`/`demo` → Scout (Lv7) → BichonProvince.
- **CDP fetch classifier** (inject, then walk, then read `__diag`):
  ```js
  window.__diag = { atlasPages: [], perFrame: [], rafGaps: [], startedAt: performance.now() };
  (function(){ var D=window.__diag, last=performance.now();
    (function loop(){ var n=performance.now(), g=n-last; last=n; if(g>45)D.rafGaps.push(Math.round(g)); requestAnimationFrame(loop); })();
    var of=window.fetch; window.fetch=function(){ try{ var u=String((arguments[0]&&arguments[0].url)||arguments[0]||'');
      if(/bevy-entity-atlases\/.*\.png/i.test(u)) D.atlasPages.push(u.slice(-40));
      else if(/original-ui\/.*\.png/i.test(u)) D.perFrame.push(u.slice(-40)); }catch(e){} return of.apply(this,arguments); }; })();
  // after walking: JSON.stringify({atlas:[...new Set(window.__diag.atlasPages)], perFrame: window.__diag.perFrame.length})
  ```
  `bevy-entity-atlases/*.png` = atlas page loads (good); `original-ui/*.png` = per-frame stream (the stutter).
- **Atlas regen** (multi-page, covers the local Bichon scene):
  ```bash
  node scripts/asset-pipeline/pack.mjs --category entities --atlasKey starter-bichon-base \
    --pageSize 2048 \
    --roots "CArmour/00,CHair/00,CWeapon/00,AArmour/00,AHair/00,AWeapon/00 L,AWeapon/00 R,NPC,Monster/000,Monster/003,Monster/004,Monster/005,Monster/010,Monster/012,Monster/139"
  ```
  Busts the manifest only if the client uses `cache:"no-cache"` (Finding 1).

---

## Next steps

1. **Debug partial-cover (P3).** Expose the resolve decision (source: `prebuilt|partial|live`
   + covered/uncovered counts + the **first uncovered key**) to `window`; re-measure; pin why
   covered frames still stream (key format / persistence). One focused pass on a clean context.
2. **Keep the cache fix.** `force-cache → no-cache` for the manifest is a real correctness fix
   regardless of partial-cover (a content-hash-versioned manifest URL is the production-correct
   alternative).
3. **Full-corpus R2 pack (P4).** Broad coverage across all maps needs the full entity corpus
   packed into a multi-page atlas served from R2 (release lane). Partial-cover makes this
   **incremental** rather than all-or-nothing.

---

## PR / branch state (2026-06-24)

- **Merged to `main`:** [#148](https://github.com/Zombieliu/mir2/pull/148) (movement easing),
  [#156](https://github.com/Zombieliu/mir2/pull/156) (map-object glow).
- **Open:** [#122](https://github.com/Zombieliu/mir2/pull/122) (multi-page entity atlas — the
  foundation; re-applied in this worktree), [#123](https://github.com/Zombieliu/mir2/pull/123)
  (uncovered tiles + *unverified* render experiments — risky), [#149](https://github.com/Zombieliu/mir2/pull/149)
  (button sound — CONFLICTING), [#152](https://github.com/Zombieliu/mir2/pull/152) (XP / skill
  bar — needs `cargo test`).
- **This worktree** (`claude/blissful-hertz-05e45e`) currently holds (UNCOMMITTED): the #122
  re-merge + a covering atlas repack + the `no-cache` fix + the **(buggy) partial-cover** code.
  The repacked atlas, the orphan `starter-bichon-base-p3.png`, and the `node_modules` symlink are
  local test artifacts.

## Related

- `docs/client/movement-prediction.md` · `docs/ASSET-ATLAS-MIGRATION-PLAN.md` (#122) ·
  `docs/RENDER-PERF-ROADMAP.md` (#123)
- memory: `bevy-entity-atlas-runtime-model`, `movement-overshoot-snap-repro`,
  `client-render-perf`, `actor-sprite-lib-truncation`
