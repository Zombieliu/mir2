/**
 * Asset Residency Manager — type contracts.
 *
 * All IO is injected through interfaces so the core manager is testable with
 * in-process fakes (no DOM, no IndexedDB, no fetch required).
 */

// ---------------------------------------------------------------------------
// Payload — what callers store and retrieve.
// ---------------------------------------------------------------------------

/**
 * The raw bytes (RGBA pixel data) for a single atlas page plus enough
 * metadata to reconstruct a BevyEntityAtlasSnapshot on the way out.
 *
 * Kept intentionally lean — the manager owns storage/eviction, not assembly.
 */
export type AtlasPagePayload = {
  /** Canonical key that identifies this atlas configuration. */
  key: string;
  /**
   * Optional key of the prebuilt/source atlas from which this page derived.
   * Mirrors BevyEntityAtlasSnapshot.sourceKey.
   */
  sourceKey?: string;
  width: number;
  height: number;
  /** RGBA pixel data — the serialisable bytes the persistent tier stores. */
  pixels: Uint8Array;
  /**
   * Rect list for rect-map reconstruction. Stored alongside pixels so the
   * persistent tier is self-contained and can reconstitute a full snapshot.
   */
  rectList: AtlasRect[];
  /**
   * Optional pre-resolved image URL (prebuilt fast-path: no pixel decode
   * needed, hand the URL straight to WebGL/Bevy).
   */
  imageUrl?: string;
  /**
   * Multi-page atlases list every page here; the top-level
   * width/height/imageUrl/pixels/rectList mirror page 0 for backward
   * compatibility. Absent or length-1 ⇒ single-page. The manager treats the
   * payload opaquely, so this is purely additive.
   */
  pages?: AtlasPageData[];
};

export type AtlasRect = {
  key: string;
  x: number;
  y: number;
  width: number;
  height: number;
  /** Page index (in AtlasPagePayload.pages) this rect lives on. Absent ⇒ 0. */
  pageIndex?: number;
};

/** One texture page of a multi-page atlas payload. */
export type AtlasPageData = {
  key: string;
  width: number;
  height: number;
  /**
   * RGBA bytes for this page (live / persistent path). Absent for URL-only
   * prebuilt pages.
   */
  pixels?: Uint8Array;
  /** Pre-resolved page image URL (prebuilt fast-path). */
  imageUrl?: string;
  rectList: AtlasRect[];
};

// ---------------------------------------------------------------------------
// Tier interfaces (dependency-injected IO)
// ---------------------------------------------------------------------------

/**
 * A pluggable persistent store (the "warm" tier).
 *
 * Implementations:
 *   - BrowserIdbStore (browser-adapters.ts) — wraps IndexedDB
 *   - In-process fakes in tests
 *
 * The store is responsible only for serialisation/persistence; it does NOT
 * enforce budget limits (the manager calls `trim` after each write).
 */
export type PersistentStore = {
  /**
   * Retrieve a payload by key.  Returns null when absent, namespace-mismatch,
   * or corrupt (pixel length mismatch).
   * Implementations SHOULD bump lastUsedAt on read so LRU trim is accurate.
   */
  get(key: string): Promise<AtlasPagePayload | null>;
  /**
   * Persist a payload.  Implementations SHOULD store a lastUsedAt timestamp.
   */
  put(payload: AtlasPagePayload): Promise<void>;
  /**
   * Delete a payload by key.
   */
  delete(key: string): Promise<void>;
  /**
   * Return all stored keys sorted by lastUsedAt ascending (oldest first).
   * Used by the manager to decide which entries to evict when over budget.
   */
  listByAge(): Promise<string[]>;
};

/**
 * A pluggable source fetcher (the "cold" tier).
 *
 * Implementations:
 *   - BrowserAtlasFetcher (browser-adapters.ts) — manifest check → R2 fetch
 *     → live canvas pack (current shell logic)
 *   - In-process fakes in tests
 */
export type AtlasFetcher = {
  /**
   * Fetch or build the atlas payload for `key` from the remote/live source.
   * Throws on failure; never returns null.
   */
  fetch(key: string): Promise<AtlasPagePayload>;
};

// ---------------------------------------------------------------------------
// Manager configuration
// ---------------------------------------------------------------------------

export type AssetResidencyConfig = {
  /**
   * Maximum number of entries kept in the persistent (IDB) tier.
   * Mirrors BEVY_ENTITY_ATLAS_PERSISTENT_LIMIT = 8 in the shell.
   */
  persistentBudget: number;
  /**
   * Maximum number of entries kept in the in-memory (hot) tier.
   * Mirrors BEVY_ENTITY_ATLAS_CACHE_LIMIT = 24 in the shell.
   */
  memoryBudget: number;
  /** Pluggable persistent store. */
  persistent: PersistentStore;
  /** Pluggable source fetcher. */
  fetcher: AtlasFetcher;
};

// ---------------------------------------------------------------------------
// Manager API
// ---------------------------------------------------------------------------

export type AssetResidencyStats = {
  /** Total acquire() calls. */
  requests: number;
  /** Acquires served from in-memory tier (hot). */
  memoryHits: number;
  /** Acquires served from persistent tier (warm). */
  persistentHits: number;
  /** Acquires that fell all the way to the fetcher (cold). */
  fetchHits: number;
  /** Failed fetcher calls. */
  failures: number;
  /** Total persistent writes (put calls). */
  persistentWrites: number;
  /** Current in-memory cache size. */
  memoryCacheSize: number;
  /** Key of the most recently acquired entry. */
  lastKey: string | null;
  /** Tier that served the most recent acquire. */
  lastTier: "memory" | "persistent" | "fetch" | null;
};

export type AssetResidencyManager = {
  /**
   * Acquire the payload for `key`.
   *
   * Tier resolution order:
   *   1. In-memory (hot)   — instant Map lookup; promotes entry to MRU position.
   *   2. Persistent (warm) — IDB read; on hit, loads into memory.
   *   3. Fetcher (cold)    — manifest / R2 / live pack; on success, writes to
   *      both persistent and memory tiers (background, non-blocking).
   *
   * After any persistent write the manager trims the persistent store to budget.
   * After any memory insertion the manager trims the in-memory cache to budget.
   */
  acquire(key: string): Promise<AtlasPagePayload>;

  /**
   * Signal that a caller no longer needs `key` in memory.
   * Current implementation is a no-op refcount decrement for future use;
   * LRU eviction happens lazily on the next acquire / evictToBudget call.
   * Safe to call with an unknown key.
   */
  release(key: string): void;

  /**
   * Return true if `key` is currently in the in-memory tier.
   * Does NOT check the persistent or source tiers.
   */
  has(key: string): boolean;

  /**
   * Synchronously return the in-memory payload for `key`, or null if absent.
   * Does NOT touch the persistent/source tiers and does NOT reorder the LRU.
   * For hot-path readers that need a same-tick answer (e.g. a per-frame render
   * lookup) where awaiting `acquire()`'s microtask would drop a frame.
   */
  peek(key: string): AtlasPagePayload | null;

  /**
   * Manually trigger LRU trim of both the in-memory and persistent tiers to
   * their respective budgets.  Called automatically after each acquire, but
   * can also be called directly to shed memory before a large fetch.
   */
  evictToBudget(): Promise<void>;

  /**
   * Return a snapshot of current statistics.  Object is a value copy — safe
   * to hold without stale-read concerns.
   */
  stats(): AssetResidencyStats;
};
