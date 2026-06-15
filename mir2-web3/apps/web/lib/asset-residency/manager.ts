/**
 * Asset Residency Manager — core implementation.
 *
 * Framework-agnostic; no DOM, no IndexedDB, no fetch calls — all IO is
 * injected through the PersistentStore and AtlasFetcher interfaces.
 *
 * Tier model (matches the three-tier cache in original-client-shell.tsx):
 *
 *   In-memory (hot)   — Map<string, AtlasPagePayload>, LRU-ordered
 *   Persistent (warm) — IDB via injected PersistentStore
 *   Fetcher (cold)    — R2 / prebuilt / live-pack via injected AtlasFetcher
 *
 * The manager adds refcounting stubs so callers can signal when they no longer
 * need an entry in memory (release()); eviction is LRU and lazy.
 */

import type {
  AssetResidencyConfig,
  AssetResidencyManager,
  AssetResidencyStats,
  AtlasPagePayload,
} from "./types";

export function createAssetResidency(config: AssetResidencyConfig): AssetResidencyManager {
  const { memoryBudget, persistentBudget, persistent, fetcher } = config;

  // In-memory tier — insertion order = LRU order (oldest first, newest last).
  // We delete-and-re-insert on every access to promote to MRU.
  const memoryCache = new Map<string, AtlasPagePayload>();

  // Refcount table (release() decrements; currently informational only — eviction
  // is purely LRU-time-based so future callers can extend this into pinning).
  const refCounts = new Map<string, number>();

  // Stats
  const _stats: AssetResidencyStats = {
    requests: 0,
    memoryHits: 0,
    persistentHits: 0,
    fetchHits: 0,
    failures: 0,
    persistentWrites: 0,
    memoryCacheSize: 0,
    lastKey: null,
    lastTier: null,
  };

  // ---------------------------------------------------------------------------
  // Memory-tier helpers
  // ---------------------------------------------------------------------------

  function memoryGet(key: string): AtlasPagePayload | undefined {
    const entry = memoryCache.get(key);
    if (!entry) return undefined;
    // Promote to MRU position.
    memoryCache.delete(key);
    memoryCache.set(key, entry);
    return entry;
  }

  function memorySet(payload: AtlasPagePayload): void {
    // Promote if already present, else insert.
    memoryCache.delete(payload.key);
    memoryCache.set(payload.key, payload);
    _stats.memoryCacheSize = memoryCache.size;
    evictMemoryToBudget();
  }

  function evictMemoryToBudget(): void {
    while (memoryCache.size > memoryBudget) {
      const oldestKey = memoryCache.keys().next().value;
      if (oldestKey === undefined) break;
      memoryCache.delete(oldestKey);
      // Leave refcount entry; it self-cleans on next release() or acquire().
    }
    _stats.memoryCacheSize = memoryCache.size;
  }

  // ---------------------------------------------------------------------------
  // Persistent write + trim (fire-and-forget — callers await when they care)
  // ---------------------------------------------------------------------------

  async function persistAndTrim(payload: AtlasPagePayload): Promise<void> {
    try {
      await persistent.put(payload);
      _stats.persistentWrites += 1;
      await trimPersistentToBudget();
    } catch {
      // Persistent tier is an optimisation; failure must not break the caller.
    }
  }

  async function trimPersistentToBudget(): Promise<void> {
    try {
      const keys = await persistent.listByAge();
      const excess = keys.length - persistentBudget;
      for (let i = 0; i < excess; i++) {
        const key = keys[i];
        if (key !== undefined) {
          await persistent.delete(key);
        }
      }
    } catch {
      // Best-effort.
    }
  }

  // ---------------------------------------------------------------------------
  // Public API
  // ---------------------------------------------------------------------------

  async function acquire(key: string): Promise<AtlasPagePayload> {
    _stats.requests += 1;
    _stats.lastKey = key;

    // --- Tier 1: in-memory ---
    const hot = memoryGet(key);
    if (hot) {
      _stats.memoryHits += 1;
      _stats.lastTier = "memory";
      return hot;
    }

    // --- Tier 2: persistent (IDB) ---
    let warm: AtlasPagePayload | null = null;
    try {
      warm = await persistent.get(key);
    } catch {
      // Treat store errors as a miss.
    }
    if (warm) {
      _stats.persistentHits += 1;
      _stats.lastTier = "persistent";
      memorySet(warm);
      return warm;
    }

    // --- Tier 3: fetcher (R2 / prebuilt / live) ---
    let payload: AtlasPagePayload;
    try {
      payload = await fetcher.fetch(key);
    } catch (err) {
      _stats.failures += 1;
      throw err;
    }

    _stats.fetchHits += 1;
    _stats.lastTier = "fetch";
    memorySet(payload);
    // Write to persistent in the background — don't await so the caller gets
    // its payload immediately (mirrors the shell's `void persistBevyEntityAtlas(…)`).
    void persistAndTrim(payload);
    return payload;
  }

  function release(key: string): void {
    const count = refCounts.get(key);
    if (count === undefined || count <= 1) {
      refCounts.delete(key);
    } else {
      refCounts.set(key, count - 1);
    }
    // LRU eviction is time-based, not refcount-based; we do not remove from
    // memoryCache here — the entry will age out on the next evictToBudget call.
  }

  function has(key: string): boolean {
    return memoryCache.has(key);
  }

  async function evictToBudget(): Promise<void> {
    evictMemoryToBudget();
    await trimPersistentToBudget();
  }

  function stats(): AssetResidencyStats {
    return { ..._stats };
  }

  return { acquire, release, has, evictToBudget, stats };
}
