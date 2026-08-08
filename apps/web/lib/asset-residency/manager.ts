/** Framework-agnostic hot/warm/cold asset residency with lease-aware LRU. */

import {
  estimateAtlasPagePayloadBytes,
  type AssetResidencyConfig,
  type AssetResidencyManager,
  type AssetResidencyStats,
  type AtlasPagePayload,
} from "./types";

type MemoryEntry = {
  payload: AtlasPagePayload;
  bytes: number;
};

export function createAssetResidency(config: AssetResidencyConfig): AssetResidencyManager {
  const { memoryBudget, persistentBudget, persistent, fetcher } = config;
  const memoryBudgetBytes = config.memoryBudgetBytes ?? Number.POSITIVE_INFINITY;
  const persistentBudgetBytes = config.persistentBudgetBytes ?? Number.POSITIVE_INFINITY;

  // Insertion order is LRU order. Active leases pin entries against eviction.
  const memoryCache = new Map<string, MemoryEntry>();
  const refCounts = new Map<string, number>();
  const inFlight = new Map<string, Promise<AtlasPagePayload>>();
  let memoryCacheBytes = 0;

  const currentStats: AssetResidencyStats = {
    requests: 0,
    memoryHits: 0,
    persistentHits: 0,
    fetchHits: 0,
    failures: 0,
    persistentWrites: 0,
    memoryCacheSize: 0,
    memoryCacheBytes: 0,
    pinnedEntryCount: 0,
    memoryEvictions: 0,
    lastKey: null,
    lastTier: null,
  };

  function refreshMemoryStats(): void {
    currentStats.memoryCacheSize = memoryCache.size;
    currentStats.memoryCacheBytes = memoryCacheBytes;
    currentStats.pinnedEntryCount = [...memoryCache.keys()].reduce(
      (count, key) => count + ((refCounts.get(key) ?? 0) > 0 ? 1 : 0),
      0,
    );
  }

  function pin(key: string): void {
    refCounts.set(key, (refCounts.get(key) ?? 0) + 1);
    refreshMemoryStats();
  }

  function memoryGet(key: string): AtlasPagePayload | undefined {
    const entry = memoryCache.get(key);
    if (!entry) return undefined;
    memoryCache.delete(key);
    memoryCache.set(key, entry);
    return entry.payload;
  }

  function memorySet(payload: AtlasPagePayload): void {
    const existing = memoryCache.get(payload.key);
    if (existing) {
      memoryCacheBytes -= existing.bytes;
      memoryCache.delete(payload.key);
    }
    const entry = { payload, bytes: estimateAtlasPagePayloadBytes(payload) };
    memoryCache.set(payload.key, entry);
    memoryCacheBytes += entry.bytes;
    refreshMemoryStats();
  }

  function evictMemoryToBudget(): void {
    while (memoryCache.size > memoryBudget || memoryCacheBytes > memoryBudgetBytes) {
      let evicted = false;
      for (const [key, entry] of memoryCache) {
        if ((refCounts.get(key) ?? 0) > 0) continue;
        memoryCache.delete(key);
        memoryCacheBytes -= entry.bytes;
        refCounts.delete(key);
        currentStats.memoryEvictions += 1;
        evicted = true;
        break;
      }
      // The active scene may temporarily exceed budget. release() retries as
      // soon as a lease ends instead of evicting a texture still being drawn.
      if (!evicted) break;
    }
    refreshMemoryStats();
  }

  async function trimPersistentToBudget(): Promise<void> {
    try {
      const byteEntries = persistent.listEntriesByAge
        ? await persistent.listEntriesByAge()
        : null;
      if (byteEntries) {
        let totalBytes = byteEntries.reduce((total, entry) => total + entry.bytes, 0);
        let remaining = byteEntries.length;
        for (const entry of byteEntries) {
          if (remaining <= persistentBudget && totalBytes <= persistentBudgetBytes) break;
          await persistent.delete(entry.key);
          totalBytes -= entry.bytes;
          remaining -= 1;
        }
        return;
      }

      const keys = await persistent.listByAge();
      const excess = keys.length - persistentBudget;
      for (let index = 0; index < excess; index += 1) {
        const key = keys[index];
        if (key !== undefined) await persistent.delete(key);
      }
    } catch {
      // Persistent storage is an optional optimization.
    }
  }

  async function persistAndTrim(payload: AtlasPagePayload): Promise<void> {
    try {
      await persistent.put(payload);
      currentStats.persistentWrites += 1;
      await trimPersistentToBudget();
    } catch {
      // Persistent storage must never break rendering.
    }
  }

  async function resolveCold(key: string): Promise<AtlasPagePayload> {
    let warm: AtlasPagePayload | null = null;
    try {
      warm = await persistent.get(key);
    } catch {
      // Treat persistent errors as misses.
    }
    if (warm) {
      currentStats.persistentHits += 1;
      currentStats.lastTier = "persistent";
      memorySet(warm);
      return warm;
    }

    try {
      const payload = await fetcher.fetch(key);
      currentStats.fetchHits += 1;
      currentStats.lastTier = "fetch";
      memorySet(payload);
      void persistAndTrim(payload);
      return payload;
    } catch (error) {
      currentStats.failures += 1;
      throw error;
    }
  }

  async function acquire(key: string): Promise<AtlasPagePayload> {
    currentStats.requests += 1;
    currentStats.lastKey = key;

    const hot = memoryGet(key);
    if (hot) {
      currentStats.memoryHits += 1;
      currentStats.lastTier = "memory";
      pin(key);
      return hot;
    }

    let request = inFlight.get(key);
    if (!request) {
      request = resolveCold(key);
      inFlight.set(key, request);
    }

    try {
      const payload = await request;
      pin(key);
      evictMemoryToBudget();
      return payload;
    } finally {
      if (inFlight.get(key) === request) inFlight.delete(key);
    }
  }

  function release(key: string): void {
    const count = refCounts.get(key);
    if (count === undefined || count <= 1) refCounts.delete(key);
    else refCounts.set(key, count - 1);
    refreshMemoryStats();
    evictMemoryToBudget();
  }

  function has(key: string): boolean {
    return memoryCache.has(key);
  }

  function peek(key: string): AtlasPagePayload | null {
    return memoryCache.get(key)?.payload ?? null;
  }

  async function evictToBudget(): Promise<void> {
    evictMemoryToBudget();
    await trimPersistentToBudget();
  }

  function stats(): AssetResidencyStats {
    return { ...currentStats };
  }

  return { acquire, release, has, peek, evictToBudget, stats };
}
