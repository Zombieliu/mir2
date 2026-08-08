/**
 * On-chain smart-mine client core (M4, WF-6).
 *
 * Pure, browser-agnostic building blocks for the client half of the on-chain
 * mining loop (DESIGN §3/§4):
 *   - PTB builders for `mine_system::mine_batch` and `redeem_system::redeem`.
 *   - (re-exported from `onchain-mine-config.ts`): deployment ids, ore kinds, the
 *     swing-nonce tracker and the optimistic-vs-chain reconcile/vein-stage helpers.
 *
 * Everything here is side-effect-free and unit-tested headlessly. Wallet signing,
 * VFX, and `page.tsx` wiring live in the React layer (see `onchain-mine-session.ts`);
 * this module never touches the DOM or a wallet. The server stays authoritative —
 * these builders only CONSTRUCT a transaction. Ore/gold only land after the Relayer
 * injects the chain-confirmed event into the Sim (apps/gateway/src/inject.rs ->
 * apps/simulation/src/runtime/onchain.rs); before that the client shows only
 * optimistic VFX, which is reconciled against the confirmed grant.
 *
 * NOTE: this module statically imports `@mysten/sui` — `page.tsx` must only import it
 * DYNAMICALLY (the pure bits it needs at render time live in `onchain-mine-config.ts`).
 */

import { Transaction } from "@mysten/sui/transactions";

import {
  ORE_KIND_CONSTRUCTOR,
  type OnchainMineDeployment,
  type OreKindName,
} from "./onchain-mine-config";

export {
  createNonceTracker,
  isOreKindName,
  ORE_KINDS,
  reconcileOptimisticOre,
  stonesLeftToVeinStage,
  TESTNET_MINE_DEPLOYMENT,
  VEIN_STAGE_CRACKED,
  VEIN_STAGE_DEPLETED,
  VEIN_STAGE_FULL,
} from "./onchain-mine-config";
export type {
  NonceTracker,
  OnchainMineDeployment,
  OreKindName,
  OreReconcile,
  OreReconcileInput,
} from "./onchain-mine-config";

/** Full move-call target for the OreKind constructor of `oreKind` (1.2.x `ore_kind` module). */
export function oreKindConstructorTarget(
  deployment: OnchainMineDeployment,
  oreKind: OreKindName,
): string {
  return `${deployment.packageId}::ore_kind::${ORE_KIND_CONSTRUCTOR[oreKind]}`;
}

export type MineBatchParams = {
  mineId: number | bigint;
  /** Number of accumulated swings to settle in this batch (>= 1). */
  swings: number | bigint;
  /** Strictly-increasing per-miner nonce (the contract rejects replays/out-of-order). */
  nonce: number | bigint;
  /** Total fee in MIST, split from the gas coin into the `fee: Coin<SUI>` argument. */
  feeMist: number | bigint;
  /** The miner's `UserStorage` object id (per-user state; the session writes it). */
  userStorageId: string;
};

/**
 * Build a `mine_system::mine_batch` PTB. The fee `Coin<SUI>` is split from the gas coin,
 * so the caller only needs a funded signer (the wallet OR an ephemeral session key).
 * Returns the unsigned Transaction.
 *
 * Matches the SK1 (Dubhe 1.2.x) signature:
 *   `entry fun mine_batch(dapp_storage, user_storage, mine_id, swings, nonce,
 *                          fee: Coin<SUI>, random, clock, ctx)`
 * The effective miner is the UserStorage's canonical owner, so a session wallet signing
 * this credits the owner (not the signer).
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
      tx.object(deployment.dappStorageId),
      tx.object(params.userStorageId),
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
  /** The miner's `UserStorage` object id (where the ore balance lives). */
  userStorageId: string;
};

/**
 * Build a `redeem_system::redeem` PTB: construct the OreKind value on-chain, then burn
 * `amount` of the owner's balance. The burn emits `OreRedeemedEvent`; the Relayer turns
 * that into a `CreditGoldFromOre` command and the Sim authoritatively credits gold
 * (DESIGN §4) — gold is NOT minted client-side. Returns the unsigned Transaction.
 *
 * Matches SK1: `public fun redeem(dapp_storage, user_storage, ore_kind: OreKind, amount, ctx)`.
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
    arguments: [tx.object(deployment.dappStorageId), tx.object(params.userStorageId), oreKind, tx.pure.u64(amount)],
  });
  return tx;
}
