/**
 * Browser-specific adapters for the Asset Residency Manager.
 *
 * These are thin wrappers around real browser APIs (IndexedDB, fetch) that
 * satisfy the PersistentStore and AtlasFetcher interfaces.  They are separated
 * from manager.ts so tests can inject pure in-memory fakes without touching
 * the DOM at all.
 *
 * Usage:
 *   import { createBrowserIdbStore, createBrowserAtlasFetcher } from
 *     "../lib/asset-residency/browser-adapters";
 *   import { createAssetResidency } from "../lib/asset-residency";
 *
 *   const manager = createAssetResidency({
 *     memoryBudget: 24,
 *     persistentBudget: 8,
 *     persistent: createBrowserIdbStore({ dbName: "mir2-bevy-entity-atlas-cache" }),
 *     fetcher: createBrowserAtlasFetcher({ resolveFn }),
 *   });
 */

import {
  estimateAtlasPagePayloadBytes,
  type AtlasPagePayload,
  type AtlasRect,
  type PersistentStore,
  type AtlasFetcher,
} from "./types";

// ---------------------------------------------------------------------------
// IDB helpers (extracted from original-client-shell.tsx idbRequest /
// idbTransactionDone — identical logic, no DOM coupling beyond IDBRequest).
// ---------------------------------------------------------------------------

function idbRequest<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

function idbTransactionDone(transaction: IDBTransaction): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
    transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"));
  });
}

// ---------------------------------------------------------------------------
// Persisted record shape stored in IDB
// ---------------------------------------------------------------------------

type IdbRecord = {
  namespace: string;
  key: string;
  sourceKey?: string;
  width: number;
  height: number;
  rectList: AtlasRect[];
  pixels: ArrayBuffer;
  storedAt: number;
  lastUsedAt: number;
  byteLength?: number;
};

// ---------------------------------------------------------------------------
// BrowserIdbStore
// ---------------------------------------------------------------------------

export type BrowserIdbStoreOptions = {
  /** IDB database name.  Default: "mir2-bevy-entity-atlas-cache-v2" */
  dbName?: string;
  /** IDB object store name.  Default: "atlases" */
  storeName?: string;
  /** IDB schema version.  Default: 1 */
  version?: number;
  /**
   * Namespace tag written to every record.  Allows a schema-bump to invalidate
   * existing records without a version-upgrade migration.
   * Default: "bevy-entity-atlas-v1"
   */
  namespace?: string;
};

/**
 * PersistentStore implementation backed by IndexedDB.
 *
 * Mirrors loadPersistedBevyEntityAtlas / persistBevyEntityAtlas /
 * trimPersistedBevyEntityAtlases from original-client-shell.tsx, decoupled
 * from the component and made injectable.
 */
export function createBrowserIdbStore(options: BrowserIdbStoreOptions = {}): PersistentStore {
  const dbName = options.dbName ?? "mir2-bevy-entity-atlas-cache-v2";
  const storeName = options.storeName ?? "atlases";
  const version = options.version ?? 1;
  const namespace = options.namespace ?? "bevy-entity-atlas-v1";

  let dbPromise: Promise<IDBDatabase | null> | null = null;

  function openDb(): Promise<IDBDatabase | null> {
    if (dbPromise) return dbPromise;
    dbPromise = new Promise<IDBDatabase | null>((resolve) => {
      if (typeof window === "undefined" || !("indexedDB" in window)) {
        resolve(null);
        return;
      }
      const request = window.indexedDB.open(dbName, version);
      request.onupgradeneeded = () => {
        const db = request.result;
        if (!db.objectStoreNames.contains(storeName)) {
          const store = db.createObjectStore(storeName, { keyPath: "key" });
          store.createIndex("lastUsedAt", "lastUsedAt");
        }
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => resolve(null);
      request.onblocked = () => resolve(null);
    });
    return dbPromise;
  }

  function pixelsFromRecord(pixels: ArrayBuffer | Uint8Array | unknown): Uint8Array | null {
    if (pixels instanceof Uint8Array) return pixels;
    if (pixels instanceof ArrayBuffer) return new Uint8Array(pixels);
    return null;
  }

  async function get(key: string): Promise<AtlasPagePayload | null> {
    if (typeof window === "undefined" || !("indexedDB" in window)) return null;
    const db = await openDb();
    if (!db) return null;

    try {
      const tx = db.transaction(storeName, "readwrite");
      const store = tx.objectStore(storeName);
      const record = (await idbRequest(store.get(key))) as IdbRecord | undefined;
      if (!record || record.namespace !== namespace) {
        await idbTransactionDone(tx);
        return null;
      }

      const pixels = pixelsFromRecord(record.pixels);
      if (!pixels || pixels.byteLength !== record.width * record.height * 4) {
        store.delete(key);
        await idbTransactionDone(tx);
        return null;
      }

      record.lastUsedAt = Date.now();
      store.put(record);
      await idbTransactionDone(tx);

      return {
        key: record.key,
        sourceKey: record.sourceKey,
        width: record.width,
        height: record.height,
        rectList: record.rectList,
        pixels,
      };
    } catch {
      return null;
    }
  }

  async function put(payload: AtlasPagePayload): Promise<void> {
    if (typeof window === "undefined" || !("indexedDB" in window)) return;
    const db = await openDb();
    if (!db) return;
    if (!payload.pixels?.byteLength) return;

    const now = Date.now();
    const pixels = new Uint8Array(payload.pixels.byteLength);
    pixels.set(payload.pixels);

    const tx = db.transaction(storeName, "readwrite");
    const store = tx.objectStore(storeName);
    store.put({
      namespace,
      key: payload.key,
      sourceKey: payload.sourceKey,
      width: payload.width,
      height: payload.height,
      rectList: payload.rectList,
      pixels: pixels.buffer,
      storedAt: now,
      lastUsedAt: now,
      byteLength: estimateAtlasPagePayloadBytes(payload),
    } satisfies IdbRecord);
    await idbTransactionDone(tx);
  }

  async function deleteKey(key: string): Promise<void> {
    const db = await openDb();
    if (!db) return;
    try {
      const tx = db.transaction(storeName, "readwrite");
      const store = tx.objectStore(storeName);
      store.delete(key);
      await idbTransactionDone(tx);
    } catch {
      // Best-effort.
    }
  }

  async function listByAge(): Promise<string[]> {
    const db = await openDb();
    if (!db) return [];
    try {
      const tx = db.transaction(storeName, "readonly");
      const store = tx.objectStore(storeName);
      const records = (await idbRequest(store.getAll())) as IdbRecord[];
      await idbTransactionDone(tx);
      return records
        .filter((r) => r.namespace === namespace)
        .sort((a, b) => a.lastUsedAt - b.lastUsedAt)
        .map((r) => r.key);
    } catch {
      return [];
    }
  }

  async function listEntriesByAge(): Promise<Array<{ key: string; bytes: number }>> {
    const db = await openDb();
    if (!db) return [];
    try {
      const tx = db.transaction(storeName, "readonly");
      const store = tx.objectStore(storeName);
      const records = (await idbRequest(store.getAll())) as IdbRecord[];
      await idbTransactionDone(tx);
      return records
        .filter((record) => record.namespace === namespace)
        .sort((a, b) => a.lastUsedAt - b.lastUsedAt)
        .map((record) => ({
          key: record.key,
          bytes: record.byteLength ?? record.pixels?.byteLength ?? record.width * record.height * 4,
        }));
    } catch {
      return [];
    }
  }

  return { get, put, delete: deleteKey, listByAge, listEntriesByAge };
}

// ---------------------------------------------------------------------------
// BrowserAtlasFetcher
// ---------------------------------------------------------------------------

export type BrowserAtlasFetcherOptions = {
  /**
   * A function that, given a key, fetches / builds the atlas payload.
   * In production this wraps resolveBevyEntityAtlasSnapshot from the shell —
   * callers supply it at construction time so the fetcher stays thin.
   */
  resolveFn: (key: string) => Promise<AtlasPagePayload>;
};

/**
 * AtlasFetcher backed by a caller-supplied resolve function.
 *
 * The production resolve function is the existing resolveBevyEntityAtlasSnapshot
 * path from original-client-shell.tsx (prebuilt manifest → R2 fetch → live
 * canvas pack).  Wrapping it here isolates the residency manager from that
 * complexity while keeping the two decoupled.
 */
export function createBrowserAtlasFetcher(options: BrowserAtlasFetcherOptions): AtlasFetcher {
  return {
    async fetch(key: string): Promise<AtlasPagePayload> {
      return options.resolveFn(key);
    },
  };
}
