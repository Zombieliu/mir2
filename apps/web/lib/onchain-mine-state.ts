/**
 * On-chain mine page-state machine (M4, WF-6).
 *
 * The pure state `page.tsx` threads through React for the on-chain mining HUD:
 * swing accumulation -> one in-flight `mine_batch` -> server-broadcast settlement ->
 * optimistic-vs-confirmed reconcile (DESIGN §4 "多退少补"). Everything here is a pure
 * transition (old state in, new state out) so it is headlessly unit-tested; the React
 * layer only calls these and renders the result.
 *
 * Server stays authoritative: "optimistic" is DISPLAY ONLY (a pending-ore counter /
 * VFX); real ore lands exclusively via the chain -> Relayer -> gateway inject -> Sim
 * path, arriving as ordinary `GainedItem` packets. The settlement signal is the
 * chain-driven `MineNodeState` re-broadcast for the configured vein (also from the
 * server) — the client never credits itself anything.
 */

export type OnchainMineState = {
  /** Swings accumulated toward the next batch (not yet submitted). */
  pendingSwings: number;
  /** Optimistic ore units currently shown for `pendingSwings` (display only). */
  pendingOptimisticUnits: number;
  /** Swings of the single in-flight batch (0 = nothing in flight). */
  inFlightSwings: number;
  /** Optimistic ore units carried by the in-flight batch (display only). */
  inFlightOptimisticUnits: number;
  /** Tx digest of the in-flight batch once the wallet accepted it. */
  inFlightDigest: string | null;
  /** Nonce used by the in-flight batch (for error reporting). */
  inFlightNonce: number | null;
  /** Chain-confirmed ore units accumulated this session (display only). */
  confirmedUnits: number;
  /** Batches confirmed this session. */
  settledBatches: number;
  /** Last reconcile result (confirmed - optimistic), null until the first settlement. */
  lastReconcile: { deltaUnits: number; phantom: boolean; shortfall: boolean } | null;
  /** Last submit/settle error surfaced to the player. */
  lastError: string | null;
};

export function createOnchainMineState(): OnchainMineState {
  return {
    pendingSwings: 0,
    pendingOptimisticUnits: 0,
    inFlightSwings: 0,
    inFlightOptimisticUnits: 0,
    inFlightDigest: null,
    inFlightNonce: null,
    confirmedUnits: 0,
    settledBatches: 0,
    lastReconcile: null,
    lastError: null,
  };
}

/** True while a submitted batch has not yet settled (only one batch in flight at a time). */
export function hasBatchInFlight(state: OnchainMineState): boolean {
  return state.inFlightSwings > 0;
}

/** True when enough swings are pending AND no batch is in flight. */
export function canFlushBatch(state: OnchainMineState, batchSize: number): boolean {
  return !hasBatchInFlight(state) && state.pendingSwings >= Math.max(1, Math.floor(batchSize));
}

/**
 * Record one pick swing. `expectedUnitsPerSwing` is the optimistic display rate
 * (hit_rate/100 * drop_rate/100 of the target mine; 1.0 for the 100/100 smoke mine).
 */
export function recordSwing(
  state: OnchainMineState,
  expectedUnitsPerSwing = 1,
): OnchainMineState {
  return {
    ...state,
    pendingSwings: state.pendingSwings + 1,
    pendingOptimisticUnits:
      state.pendingOptimisticUnits + Math.max(0, expectedUnitsPerSwing),
  };
}

/**
 * Move the pending swings into the (single) in-flight batch, returning the swing count
 * to submit. Returns null when nothing is pending or a batch is already in flight.
 */
export function beginBatch(
  state: OnchainMineState,
  nonce: number,
): { state: OnchainMineState; swings: number } | null {
  if (hasBatchInFlight(state) || state.pendingSwings <= 0) {
    return null;
  }
  return {
    swings: state.pendingSwings,
    state: {
      ...state,
      pendingSwings: 0,
      pendingOptimisticUnits: 0,
      inFlightSwings: state.pendingSwings,
      inFlightOptimisticUnits: state.pendingOptimisticUnits,
      inFlightDigest: null,
      inFlightNonce: nonce,
      lastError: null,
    },
  };
}

/** The wallet accepted the batch; remember its digest while we await settlement. */
export function batchSubmitted(state: OnchainMineState, digest: string): OnchainMineState {
  return { ...state, inFlightDigest: digest };
}

/**
 * The submit failed (wallet rejected / RPC error / contract abort). The in-flight swings
 * return to pending so the player's accumulated work is not lost; the error is surfaced.
 */
export function batchFailed(state: OnchainMineState, error: string): OnchainMineState {
  return {
    ...state,
    pendingSwings: state.pendingSwings + state.inFlightSwings,
    pendingOptimisticUnits: state.pendingOptimisticUnits + state.inFlightOptimisticUnits,
    inFlightSwings: 0,
    inFlightOptimisticUnits: 0,
    inFlightDigest: null,
    inFlightNonce: null,
    lastError: error,
  };
}

/**
 * The server broadcast the settlement (chain-driven `MineNodeState` for the vein, with
 * the granted ore arriving as `GainedItem`). `confirmedUnits` is the chain-granted ore
 * when the client could read it from the grant (fresh ore item), else null (settled,
 * units unknown -> no reconcile delta is claimed).
 */
export function confirmSettlement(
  state: OnchainMineState,
  confirmedUnits: number | null,
): OnchainMineState {
  if (!hasBatchInFlight(state)) {
    return state;
  }
  const optimistic = state.inFlightOptimisticUnits;
  const lastReconcile =
    confirmedUnits === null
      ? state.lastReconcile
      : {
          deltaUnits: confirmedUnits - optimistic,
          phantom: confirmedUnits < optimistic,
          shortfall: confirmedUnits > optimistic,
        };
  return {
    ...state,
    inFlightSwings: 0,
    inFlightOptimisticUnits: 0,
    inFlightDigest: null,
    inFlightNonce: null,
    confirmedUnits: state.confirmedUnits + (confirmedUnits ?? 0),
    settledBatches: state.settledBatches + 1,
    lastReconcile,
    lastError: null,
  };
}

/** Drop pending swings without submitting (player walked away / disabled the feature). */
export function resetPendingSwings(state: OnchainMineState): OnchainMineState {
  return { ...state, pendingSwings: 0, pendingOptimisticUnits: 0 };
}

/**
 * Give up waiting for the settlement signal (watchdog timeout: relayer down, or the
 * grant landed while the player was on another map). UNLIKE `batchFailed`, the swings
 * are NOT restored — the batch was accepted on-chain (its nonce is spent) and may well
 * have settled; resubmitting would double-mine. The chain ledger stays authoritative;
 * any granted ore still arrives as ordinary GainedItems.
 */
export function abandonInFlightBatch(state: OnchainMineState, error: string): OnchainMineState {
  if (!hasBatchInFlight(state)) {
    return state;
  }
  return {
    ...state,
    inFlightSwings: 0,
    inFlightOptimisticUnits: 0,
    inFlightDigest: null,
    inFlightNonce: null,
    lastError: error,
  };
}

// ---------------------------------------------------------------------------
// Settlement detection helpers (server-broadcast packets only).
// ---------------------------------------------------------------------------

/**
 * Ore units carried by a chain-granted ore item, when readable. The sim encodes the
 * granted units as `dura = units * 1000` on a FRESH item (count <= 1, dura == maxDura).
 * Incremented stacks are not readable this way -> null (settled-with-unknown-units).
 *
 * The shape alone is NOT unique to chain grants (fresh shop/craft items also have round
 * full durabilities), so the caller MUST additionally bound the result by the in-flight
 * swing count — the contract grants at most one ore per swing — via `maxPlausibleUnits`.
 * Items that read above that bound are rejected (null).
 */
export function oreUnitsFromGainedItem(
  item: {
    count?: number;
    currentDura?: number;
    maxDura?: number;
  },
  maxPlausibleUnits = Number.POSITIVE_INFINITY,
): number | null {
  const count = item.count ?? 1;
  const dura = item.currentDura ?? 0;
  const maxDura = item.maxDura ?? 0;
  if (count > 1 || dura <= 0 || dura !== maxDura || dura % 1000 !== 0) {
    return null;
  }
  const units = dura / 1000;
  return units <= maxPlausibleUnits ? units : null;
}

/** True when a MineNodeState location matches the configured on-chain vein cell. */
export function isOnchainVeinNode(
  node: { x: number; y: number },
  vein: { x: number; y: number },
): boolean {
  return node.x === vein.x && node.y === vein.y;
}

// ---------------------------------------------------------------------------
// Nonce persistence (per account + package) — survives reloads so the strictly
// increasing on-chain nonce is not reused. Storage is injectable for tests.
// ---------------------------------------------------------------------------

type StringStorage = {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
};

function nonceStorageKey(accountId: string, packageId: string): string {
  return `mir2.onchainMine.nextNonce.${accountId}.${packageId}`;
}

/** The persisted NEXT nonce for this account+package, or 1 when none/invalid. */
export function loadPersistedNextNonce(
  accountId: string,
  packageId: string,
  storage: StringStorage | null = typeof localStorage === "undefined" ? null : localStorage,
): number {
  if (!storage) return 1;
  try {
    const raw = storage.getItem(nonceStorageKey(accountId, packageId));
    const parsed = raw === null ? NaN : Number(raw);
    return Number.isInteger(parsed) && parsed >= 1 ? parsed : 1;
  } catch {
    return 1;
  }
}

export function persistNextNonce(
  accountId: string,
  packageId: string,
  nextNonce: number,
  storage: StringStorage | null = typeof localStorage === "undefined" ? null : localStorage,
): void {
  if (!storage) return;
  try {
    storage.setItem(nonceStorageKey(accountId, packageId), String(Math.max(1, Math.floor(nextNonce))));
  } catch {
    // Private-mode storage failures only cost nonce persistence, not correctness.
  }
}
