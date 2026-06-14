/**
 * On-chain smart-mine — pure configuration + dependency-free helpers (M4, WF-6).
 *
 * Split from `onchain-mine.ts` so `page.tsx` can import deployment ids, ore-kind
 * names and the nonce tracker WITHOUT statically pulling `@mysten/sui` into the main
 * bundle; the PTB builders / wallet session (which need the SDK) are dynamically
 * imported only when the player actually mines. `onchain-mine.ts` re-exports all of
 * this, so existing imports keep working.
 */

/**
 * Public on-chain ids for the deployed mine package (onchain/deployments/testnet.json).
 * SK1/SK2: the Dubhe 1.2.x dappHub/UserStorage model — global mine state lives in
 * `dappStorageId`, each miner has a per-user `UserStorage` (resolved at runtime), and the
 * framework package (`frameworkPackageId`) + `dappHubId` are needed to activate sessions.
 */
export type OnchainMineDeployment = {
  packageId: string;
  /** Published Dubhe framework package on testnet (activate/deactivate session lives there). */
  frameworkPackageId: string;
  /** Shared DappHub object (framework). */
  dappHubId: string;
  /** This dapp's shared DappStorage object (global mine state). */
  dappStorageId: string;
  /** Sui system Random object (always 0x8). */
  randomId: string;
  /** Sui system Clock object (always 0x6). */
  clockId: string;
};

/**
 * testnet deployment — public ids only. Driven by NEXT_PUBLIC_* so the fresh SK1 publish
 * (new packageId + DappStorage) is wired without a code change; see onchain/deployments/testnet.json.
 */
export const TESTNET_MINE_DEPLOYMENT: OnchainMineDeployment = {
  packageId: process.env.NEXT_PUBLIC_ONCHAIN_MINE_PACKAGE_ID ?? "",
  frameworkPackageId: process.env.NEXT_PUBLIC_ONCHAIN_MINE_FRAMEWORK_PACKAGE_ID ?? "",
  dappHubId: process.env.NEXT_PUBLIC_ONCHAIN_MINE_DAPP_HUB_ID ?? "",
  dappStorageId: process.env.NEXT_PUBLIC_ONCHAIN_MINE_DAPP_STORAGE_ID ?? "",
  randomId: "0x8",
  clockId: "0x6",
};

/**
 * OreKind enum variants (`mir2_mine::ore_kind`), matching the relayer/sim strings (the
 * Move variant names). Order matches the on-chain enum.
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

/** OreKind variant -> its `mir2_mine::ore_kind::new_*` constructor function name (1.2.x). */
export const ORE_KIND_CONSTRUCTOR: Record<OreKindName, string> = {
  Amethyst: "new_amethyst",
  BlackIron: "new_blackiron",
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
 * client already understands (DESIGN §4-⑥, full / cracked / empty). EXACTLY the P0
 * `mining::mine_stage` tiers (1:1 with the sim): depleted at 0; cracked while
 * `stones * 2 < max` (strictly below half — exactly half still renders full).
 */
export function stonesLeftToVeinStage(stonesLeft: number, maxStones: number): number {
  const left = Math.max(0, Math.floor(stonesLeft));
  if (left <= 0) {
    return VEIN_STAGE_DEPLETED;
  }
  const capacity = Math.max(1, Math.floor(maxStones));
  if (left * 2 < capacity) {
    return VEIN_STAGE_CRACKED;
  }
  return VEIN_STAGE_FULL;
}
