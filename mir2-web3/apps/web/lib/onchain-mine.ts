/**
 * On-chain smart-mine client core (M4, WF-6).
 *
 * Pure, browser-agnostic building blocks for the client half of the on-chain
 * mining loop (DESIGN §3/§4):
 *   - PTB builders for `mine_system::mine_batch` and `redeem_system::redeem`.
 *   - A swing batcher: accumulate N pick swings, then flush ONE `mine_batch`.
 *   - A monotonic nonce tracker (the contract enforces a strictly-increasing nonce).
 *   - Reconcile helpers: optimistic-vs-chain ore delta + `stones_left` -> vein stage.
 *
 * Everything here is side-effect-free and unit-tested headlessly. Wallet signing,
 * VFX, and `page.tsx` wiring live in the React layer (see `onchain-mine-session.ts`);
 * this module never touches the DOM or a wallet. The server stays authoritative —
 * these builders only CONSTRUCT a transaction. Ore/gold only land after the Relayer
 * injects the chain-confirmed event into the Sim (apps/gateway/src/inject.rs ->
 * apps/simulation/src/runtime/onchain.rs); before that the client shows only
 * optimistic VFX, which is reconciled against the confirmed grant.
 */

import { Transaction } from "@mysten/sui/transactions";

/** Public on-chain ids for the deployed mine package (onchain/deployments/testnet.json). */
export type OnchainMineDeployment = {
  packageId: string;
  schemaId: string;
  /** Sui system Random object (always 0x8). */
  randomId: string;
  /** Sui system Clock object (always 0x6). */
  clockId: string;
};

/** testnet deployment — public ids only (see onchain/deployments/testnet.json). */
export const TESTNET_MINE_DEPLOYMENT: OnchainMineDeployment = {
  packageId: "0xe6c3602e4055b76afd82a48745f9cd34daa4a6dce1f420747f0779732640dbe5",
  schemaId: "0x77138ceee249ef3d328296cb932cbad50916bf062d06cb6d1ee7ea0ea1acc698",
  randomId: "0x8",
  clockId: "0x6",
};

/**
 * OreKind enum variants (`mir2_mine::mir2_mine_ore_kind`), matching the relayer/sim
 * strings (the Move variant names). Order matches the on-chain enum.
 */
export const ORE_KINDS = [
  "Amethyst",
  "BlackIron",
  "Copper",
  "Gold",
  "Nephrite",
  "Platinum",
  "Ruby",
  "Silver",
] as const;
export type OreKindName = (typeof ORE_KINDS)[number];

/** OreKind variant -> its `mir2_mine_ore_kind::new_*` constructor function name. */
const ORE_KIND_CONSTRUCTOR: Record<OreKindName, string> = {
  Amethyst: "new_amethyst",
  BlackIron: "new_black_iron",
  Copper: "new_copper",
  Gold: "new_gold",
  Nephrite: "new_nephrite",
  Platinum: "new_platinum",
  Ruby: "new_ruby",
  Silver: "new_silver",
};

export function isOreKindName(value: string): value is OreKindName {
  return (ORE_KINDS as readonly string[]).includes(value);
}

/** Full move-call target for the OreKind constructor of `oreKind`. */
export function oreKindConstructorTarget(
  deployment: OnchainMineDeployment,
  oreKind: OreKindName,
): string {
  return `${deployment.packageId}::mir2_mine_ore_kind::${ORE_KIND_CONSTRUCTOR[oreKind]}`;
}

export type MineBatchParams = {
  mineId: number | bigint;
  /** Number of accumulated swings to settle in this batch (>= 1). */
  swings: number | bigint;
  /** Strictly-increasing per-miner nonce (the contract rejects replays/out-of-order). */
  nonce: number | bigint;
  /** Total fee in MIST, split from the gas coin into the `fee: Coin<SUI>` argument. */
  feeMist: number | bigint;
};

/**
 * Build a `mine_system::mine_batch` PTB. The fee `Coin<SUI>` is split from the gas coin,
 * so the caller only needs a funded wallet. Sender + gas payment are filled in by the
 * wallet at sign time. Returns the unsigned Transaction.
 *
 * Matches the on-chain signature:
 *   `entry fun mine_batch(schema, mine_id, swings, nonce, fee: Coin<SUI>, random, clock, ctx)`
 */
export function buildMineBatchTransaction(
  deployment: OnchainMineDeployment,
  params: MineBatchParams,
): Transaction {
  const swings = BigInt(params.swings);
  if (swings <= 0n) {
    throw new Error("mine_batch requires swings >= 1");
  }
  const feeMist = BigInt(params.feeMist);
  if (feeMist < 0n) {
    throw new Error("mine_batch fee must be >= 0");
  }

  const tx = new Transaction();
  const [fee] = tx.splitCoins(tx.gas, [tx.pure.u64(feeMist)]);
  tx.moveCall({
    target: `${deployment.packageId}::mine_system::mine_batch`,
    arguments: [
      tx.object(deployment.schemaId),
      tx.pure.u64(BigInt(params.mineId)),
      tx.pure.u64(swings),
      tx.pure.u64(BigInt(params.nonce)),
      fee,
      tx.object(deployment.randomId),
      tx.object(deployment.clockId),
    ],
  });
  return tx;
}

export type RedeemParams = {
  oreKind: OreKindName;
  /** Ore units to burn for gold (>= 1). */
  amount: number | bigint;
};

/**
 * Build a `redeem_system::redeem` PTB: construct the OreKind value on-chain, then burn
 * `amount` of the caller's balance. The burn emits `ore_redeemed`; the Relayer turns
 * that into a `CreditGoldFromOre` command and the Sim authoritatively credits gold
 * (DESIGN §4) — gold is NOT minted client-side. Returns the unsigned Transaction.
 *
 * Matches: `public fun redeem(schema, ore_kind: OreKind, amount, ctx)`.
 */
export function buildRedeemTransaction(
  deployment: OnchainMineDeployment,
  params: RedeemParams,
): Transaction {
  const amount = BigInt(params.amount);
  if (amount <= 0n) {
    throw new Error("redeem requires amount >= 1");
  }

  const tx = new Transaction();
  const oreKind = tx.moveCall({
    target: oreKindConstructorTarget(deployment, params.oreKind),
  });
  tx.moveCall({
    target: `${deployment.packageId}::redeem_system::redeem`,
    arguments: [tx.object(deployment.schemaId), oreKind, tx.pure.u64(amount)],
  });
  return tx;
}

// ---------------------------------------------------------------------------
// Swing batcher — accumulate N pick swings, then flush ONE mine_batch (DESIGN §4).
// ---------------------------------------------------------------------------

export type SwingBatcherOptions = {
  /** Flush threshold: signal a batch once this many swings have accumulated (>= 1). */
  batchSize: number;
};

export type SwingRecord = {
  /** Swings accumulated since the last flush. */
  pending: number;
  /** True once `pending` has reached `batchSize` and a batch should be sent. */
  shouldFlush: boolean;
};

export type SwingBatcher = {
  /** Record one swing; returns the new pending count and whether to flush now. */
  recordSwing(): SwingRecord;
  /** Current pending (unflushed) swing count. */
  pending(): number;
  /**
   * Consume the pending swings for a batch: returns the count and resets pending to 0.
   * Returns 0 when nothing is pending (the caller must not send an empty batch).
   */
  takeBatch(): number;
  /** Drop pending swings without sending (e.g. the player left the vein). */
  reset(): void;
};

export function createSwingBatcher(options: SwingBatcherOptions): SwingBatcher {
  const batchSize = Math.max(1, Math.floor(options.batchSize));
  let pending = 0;
  return {
    recordSwing() {
      pending += 1;
      return { pending, shouldFlush: pending >= batchSize };
    },
    pending() {
      return pending;
    },
    takeBatch() {
      const count = pending;
      pending = 0;
      return count;
    },
    reset() {
      pending = 0;
    },
  };
}

// ---------------------------------------------------------------------------
// Nonce tracker — the contract requires a strictly-increasing per-miner nonce.
// ---------------------------------------------------------------------------

export type NonceTracker = {
  /** The nonce the NEXT mine_batch should use (without consuming it). */
  peek(): number;
  /** Return the next nonce and advance the counter. */
  next(): number;
  /**
   * Reconcile to the authoritative on-chain `miner_nonce` (the last accepted nonce):
   * the next nonce becomes `onChainNonce + 1`, but never moves backward.
   */
  syncFromChain(onChainNonce: number): void;
};

export function createNonceTracker(startNext = 1): NonceTracker {
  let nextNonce = Math.max(1, Math.floor(startNext));
  return {
    peek() {
      return nextNonce;
    },
    next() {
      const value = nextNonce;
      nextNonce += 1;
      return value;
    },
    syncFromChain(onChainNonce) {
      const candidate = Math.floor(onChainNonce) + 1;
      if (candidate > nextNonce) {
        nextNonce = candidate;
      }
    },
  };
}

// ---------------------------------------------------------------------------
// Reconcile — optimistic VFX vs chain-confirmed truth (DESIGN §4 ⑤/⑥, "多退少补").
// ---------------------------------------------------------------------------

export type OreReconcileInput = {
  /** Ore units the client OPTIMISTICALLY showed for this batch (pre-confirmation). */
  optimisticUnits: number;
  /** Ore units the chain actually granted (from the confirmed GrantOnchainOre). */
  confirmedUnits: number;
};

export type OreReconcile = {
  /** confirmed - optimistic. >0 = under-showed (add more); <0 = phantom (remove). */
  deltaUnits: number;
  /** The optimistic display over-counted (showed ore the chain did not grant). */
  phantom: boolean;
  /** The optimistic display under-counted (chain granted more than shown). */
  shortfall: boolean;
};

export function reconcileOptimisticOre(input: OreReconcileInput): OreReconcile {
  const optimistic = Math.max(0, Math.floor(input.optimisticUnits));
  const confirmed = Math.max(0, Math.floor(input.confirmedUnits));
  const deltaUnits = confirmed - optimistic;
  return {
    deltaUnits,
    phantom: deltaUnits < 0,
    shortfall: deltaUnits > 0,
  };
}

/** Vein render stages: 2 = full, 1 = cracked, 0 = depleted (matches MineNodeState.stage). */
export const VEIN_STAGE_FULL = 2;
export const VEIN_STAGE_CRACKED = 1;
export const VEIN_STAGE_DEPLETED = 0;

/**
 * Map an on-chain `stones_left` (0..maxStones) to the 3-tier vein render stage the
 * client already understands (DESIGN §4-⑥, full / cracked / empty). Depleted at 0;
 * cracked at or below half capacity; full otherwise.
 */
export function stonesLeftToVeinStage(stonesLeft: number, maxStones: number): number {
  const left = Math.max(0, Math.floor(stonesLeft));
  if (left <= 0) {
    return VEIN_STAGE_DEPLETED;
  }
  const capacity = Math.max(1, Math.floor(maxStones));
  if (left * 2 <= capacity) {
    return VEIN_STAGE_CRACKED;
  }
  return VEIN_STAGE_FULL;
}
