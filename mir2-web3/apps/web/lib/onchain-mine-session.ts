"use client";

/**
 * On-chain smart-mine wallet session (M4, WF-6).
 *
 * The browser glue between the pure PTB builders (`onchain-mine.ts`) and a connected
 * Sui wallet. It signs + submits `mine_batch` / `redeem` through the Wallet Standard
 * `sui:signAndExecuteTransaction` feature — the SAME wallet objects the login flow
 * already uses (`passkey-auth.ts`).
 *
 * Server stays authoritative: this only submits the transaction and returns its digest.
 * Ore/gold land in the bag ONLY after the chain settles, the Relayer picks up the event,
 * and the Sim applies the injected `GrantOnchainOre` / `CreditGoldFromOre` (DESIGN §4).
 * The client shows optimistic VFX meanwhile and reconciles against the confirmed grant
 * (`reconcileOptimisticOre`). This module performs no on-chain reads; the caller tracks
 * its nonce locally (`createNonceTracker`) and recovers on a BadNonce abort.
 */

import {
  SuiSignAndExecuteTransaction,
  type SuiSignAndExecuteTransactionFeature,
  type Wallet,
  type WalletAccount,
} from "@mysten/wallet-standard";

import {
  buildMineBatchTransaction,
  buildRedeemTransaction,
  type MineBatchParams,
  type OnchainMineDeployment,
  type RedeemParams,
} from "./onchain-mine";

/** A submitted transaction's digest + base64 bcs effects (from the wallet RPC submit). */
export type OnchainTxResult = {
  digest: string;
  effects: string;
};

/** True when the wallet exposes the sign-and-execute feature (else it cannot mine on-chain). */
export function walletCanSignAndExecute(wallet: Wallet): boolean {
  return SuiSignAndExecuteTransaction in wallet.features;
}

function signAndExecuteFeature(wallet: Wallet) {
  const feature = (wallet.features as Partial<SuiSignAndExecuteTransactionFeature>)[
    SuiSignAndExecuteTransaction
  ];
  if (!feature) {
    throw new Error(`${wallet.name} cannot sign and execute Sui transactions`);
  }
  return feature;
}

/** The wallet + account + chain + deployment a mining session is bound to. */
export type MineSessionContext = {
  wallet: Wallet;
  account: WalletAccount;
  /** Sui chain identifier the account is on, e.g. "sui:testnet". */
  chain: `sui:${string}`;
  deployment: OnchainMineDeployment;
};

async function signAndExecute(
  ctx: MineSessionContext,
  transaction: ReturnType<typeof buildMineBatchTransaction>,
): Promise<OnchainTxResult> {
  const feature = signAndExecuteFeature(ctx.wallet);
  const output = await feature.signAndExecuteTransaction({
    transaction,
    account: ctx.account,
    chain: ctx.chain,
  });
  return { digest: output.digest, effects: output.effects };
}

/**
 * Build + sign + submit a `mine_batch`. Returns the tx digest once the wallet's RPC has
 * accepted it; the resulting ore is granted authoritatively later via the Relayer path.
 */
export async function executeMineBatch(
  ctx: MineSessionContext,
  params: MineBatchParams,
): Promise<OnchainTxResult> {
  const tx = buildMineBatchTransaction(ctx.deployment, params);
  return signAndExecute(ctx, tx);
}

/**
 * Build + sign + submit a `redeem` (burn ore for gold). The gold is credited
 * authoritatively by the Sim after the `ore_redeemed` event is relayed (DESIGN §4).
 */
export async function executeRedeem(
  ctx: MineSessionContext,
  params: RedeemParams,
): Promise<OnchainTxResult> {
  const tx = buildRedeemTransaction(ctx.deployment, params);
  return signAndExecute(ctx, tx);
}
