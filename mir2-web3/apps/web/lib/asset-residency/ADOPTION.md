# Asset Residency Manager — Adoption Guide

This document describes how `original-client-shell.tsx` would adopt the
`asset-residency` module.  The wiring is **intentionally deferred** — the
manager is built as an additive module and the shell is not touched here.

## What the manager replaces

In `original-client-shell.tsx` the following module-level globals collectively
implement a three-tier cache for `BevyEntityAtlasSnapshot` values:

| Shell global | Manager equivalent |
|---|---|
| `bevyEntityAtlasCache` (Map, LRU limit 24) | in-memory tier |
| `bevyEntityAtlasLatestSnapshot` | no direct equivalent — caller tracks "active" |
| `bevyEntityAtlasDbPromise` + IDB open/upgrade/trim logic | `createBrowserIdbStore()` |
| `bevyEntityAtlasStats` | `manager.stats()` |

And the following functions are replaced:

| Shell function | Manager method |
|---|---|
| `getCachedBevyEntityAtlas(key)` | `manager.has(key)` + `manager.acquire(key)` |
| `cacheBevyEntityAtlas(atlas)` | internal on every `acquire` result |
| `loadPersistedBevyEntityAtlas(key)` | transparent tier-2 inside `acquire` |
| `persistBevyEntityAtlas(atlas)` | fire-and-forget inside `acquire` |
| `trimPersistedBevyEntityAtlases(db)` | automatic after every persistent write |
| `openBevyEntityAtlasDb()` | encapsulated in `createBrowserIdbStore` |
| `shouldUsePersistentBevyEntityAtlasCache()` | caller passes a no-op store |

## Adoption steps (deferred — not done in this PR)

### 1. Construct the manager at module scope

```ts
// apps/web/app/original-client-shell.tsx (top of file, near existing globals)

import { createAssetResidency } from "../lib/asset-residency";
import { createBrowserIdbStore, createBrowserAtlasFetcher } from
  "../lib/asset-residency/browser-adapters";

const bevyAtlasResidency = createAssetResidency({
  memoryBudget: BEVY_ENTITY_ATLAS_CACHE_LIMIT,        // 24
  persistentBudget: BEVY_ENTITY_ATLAS_PERSISTENT_LIMIT, // 8
  persistent: createBrowserIdbStore({
    namespace: BEVY_ENTITY_ATLAS_CACHE_NAMESPACE,       // "bevy-entity-atlas-v1"
  }),
  fetcher: createBrowserAtlasFetcher({
    resolveFn: async (key) => {
      // Wrap the existing resolver.  The fetcher returns AtlasPagePayload;
      // the shell's BevyEntityAtlasSnapshot is a superset, so a thin
      // adapter converts it:
      const sources = /* collect sources for key somehow */;
      const result = await resolveBevyEntityAtlasSnapshot(sources, key);
      return atlasSnapshotToPayload(result.atlas);
    },
  }),
});
```

### 2. Replace the acquire path in the effect

The relevant effect is at approximately line 1368 in the shell.  Replace
the `getCachedBevyEntityAtlas` + `resolveBevyEntityAtlasSnapshot` call
sequence with:

```ts
// Before:
const cachedAtlas = getCachedBevyEntityAtlas(bevyEntityAtlasKey);
if (cachedAtlas) {
  bevyEntityAtlasStats.cacheHits += 1;
  bevyEntityAtlasStats.lastSource = "memory";
  setBevyEntityAtlas(cachedAtlas);
  return;
}
// … then resolveBevyEntityAtlasSnapshot → setBevyEntityAtlas …

// After:
bevyAtlasResidency.acquire(bevyEntityAtlasKey)
  .then((payload) => {
    if (disposed || bevyEntityAtlasRequestRef.current?.requestId !== requestId) return;
    bevyEntityAtlasRequestRef.current = null;
    setBevyEntityAtlas(payloadToAtlasSnapshot(payload));
  })
  .catch(() => {
    bevyEntityAtlasRequestRef.current = null;
  });
```

### 3. Expose stats via the existing debug panel

Replace the inline `bevyEntityAtlasStats` reference in the debug panel
with `bevyAtlasResidency.stats()`.  Field names differ slightly (see table
in the types); update the panel JSX accordingly.

### 4. Delete the replaced globals

Once the effect and debug panel are updated, the following can be removed:
- `bevyEntityAtlasCache`, `bevyEntityAtlasLatestSnapshot`, `bevyEntityAtlasDbPromise`
- `bevyEntityAtlasStats`, `cacheBevyEntityAtlas`, `getCachedBevyEntityAtlas`
- `loadPersistedBevyEntityAtlas`, `persistBevyEntityAtlas`
- `trimPersistedBevyEntityAtlases`, `openBevyEntityAtlasDb`
- `shouldUsePersistentBevyEntityAtlasCache`
- `idbRequest`, `idbTransactionDone` (moved to `browser-adapters.ts`)

### 5. Persistent-cache opt-out (URL param)

The shell has `?bevyAtlasPersistent=0` support in
`shouldUsePersistentBevyEntityAtlasCache`.  Replicate by conditionally
passing a no-op store:

```ts
const usePersistent = new URLSearchParams(window.location.search)
  .get("bevyAtlasPersistent") !== "0";

const persistent = usePersistent
  ? createBrowserIdbStore({ namespace: BEVY_ENTITY_ATLAS_CACHE_NAMESPACE })
  : createNullStore(); // trivial noop store — get/put/delete/listByAge all return empty
```

`createNullStore` is a one-liner that can live inline or in a future
`null-adapters.ts` helper.

## Adapter helpers (needed at wiring time)

Two small converters will be needed at wiring time in the shell
(not created here to respect the "additive only" constraint):

```ts
// AtlasPagePayload → BevyEntityAtlasSnapshot (shallow — rects reconstituted)
function payloadToAtlasSnapshot(p: AtlasPagePayload): BevyEntityAtlasSnapshot {
  return {
    key: p.key,
    sourceKey: p.sourceKey,
    width: p.width,
    height: p.height,
    imageUrl: p.imageUrl,
    rects: Object.fromEntries(p.rectList.map((r) => [r.key, r])),
    rectList: p.rectList,
    pixels: p.pixels,
  };
}

// BevyEntityAtlasSnapshot → AtlasPagePayload (subset)
function atlasSnapshotToPayload(a: BevyEntityAtlasSnapshot): AtlasPagePayload {
  return {
    key: a.key,
    sourceKey: a.sourceKey,
    width: a.width,
    height: a.height,
    imageUrl: a.imageUrl,
    rectList: a.rectList,
    pixels: a.pixels ?? new Uint8Array(0),
  };
}
```
