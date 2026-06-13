/**
 * On-chain smart-mine — Dubhe 1.2.x **session keys** (SK2).
 *
 * The popup-killer for continuous (hold-to-mine) mining: the owner signs ONCE to authorize
 * an ephemeral "session wallet" (`activate_session`), then every `mine_batch` is signed
 * LOCALLY by that ephemeral key — no wallet popup. The framework credits the OWNER because
 * the per-user state lives in the owner's `UserStorage` (effective miner =
 * `canonical_owner`, regardless of who signed). See `onchain-sk/SK0-FINDINGS.md`.
 *
 * Pure `@mysten/sui` v2 — no Dubhe SDK in the bundle. This whole module is dynamically
 * imported by `page.tsx` (like `onchain-mine-session.ts`), so `@mysten/sui` stays out of
 * the main chunk.
 *
 * SECURITY: the session key is a LOW-PRIVILEGE delegated key — it can only write this dapp's
 * resources for this UserStorage, bounded by `durationMs` (on-chain `session_expires_at`) and
 * revocable any time (`deactivate_session`). It is NOT the user's main wallet key. It is
 * persisted in localStorage so a reload keeps the session; clearing it / expiry / deactivate
 * all end it. Gas: the session wallet self-funds (the user sends it a little SUI once) — the
 * gasless path (DubheChannel) is SK3.
 */

import { Transaction } from "@mysten/sui/transactions";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { SuiJsonRpcClient, getJsonRpcFullnodeUrl } from "@mysten/sui/jsonRpc";
import { SuiSignAndExecuteTransaction } from "@mysten/wallet-standard";
import type {
  SuiSignAndExecuteTransactionFeature,
  WalletWithFeatures,
} from "@mysten/wallet-standard";

import { buildMineBatchTransaction, type MineBatchParams } from "./onchain-mine";
import type { OnchainMineDeployment } from "./onchain-mine-config";
import type { MineSessionContext, OnchainTxResult } from "./onchain-mine-session";

/** Default session lifetime — 1h. Framework bounds: [60s, 7d]. Env-overridable. */
export const SESSION_DEFAULT_DURATION_MS = Math.min(
  604_800_000,
  Math.max(60_000, Number(process.env.NEXT_PUBLIC_ONCHAIN_MINE_SESSION_DURATION_MS ?? "3600000")),
);

type StringStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;
const defaultStorage = (): StringStorage | null =>
  typeof localStorage === "undefined" ? null : localStorage;

// ── Persistence ────────────────────────────────────────────────────────────

/** The miner's UserStorage id is permanent per (account, package) — cached separately. */
function userStorageKey(account: string, packageId: string): string {
  return `mir2.onchainMine.userStorage.${account}.${packageId}`;
}
function sessionKey(account: string, packageId: string): string {
  return `mir2.onchainMine.session.${account}.${packageId}`;
}

type StoredSession = {
  account: string;
  packageId: string;
  userStorageId: string;
  sessionAddress: string;
  /** Ephemeral session key (bech32 suiprivkey) — low-privilege, expiry-bounded. */
  sessionSecretKey: string;
  /** Epoch ms; matches the on-chain session_expires_at we requested. */
  expiresAt: number;
};

export function loadUserStorageId(
  account: string,
  packageId: string,
  storage: StringStorage | null = defaultStorage(),
): string | null {
  try {
    return storage?.getItem(userStorageKey(account, packageId)) ?? null;
  } catch {
    return null;
  }
}
function persistUserStorageId(
  account: string,
  packageId: string,
  id: string,
  storage: StringStorage | null = defaultStorage(),
): void {
  try {
    storage?.setItem(userStorageKey(account, packageId), id);
  } catch {
    /* private-mode storage failure only costs caching, not correctness */
  }
}

function loadStoredSession(
  account: string,
  packageId: string,
  storage: StringStorage | null = defaultStorage(),
): StoredSession | null {
  try {
    const raw = storage?.getItem(sessionKey(account, packageId));
    if (!raw) return null;
    const parsed = JSON.parse(raw) as StoredSession;
    return parsed.sessionSecretKey && parsed.userStorageId ? parsed : null;
  } catch {
    return null;
  }
}
function persistStoredSession(s: StoredSession, storage: StringStorage | null = defaultStorage()): void {
  try {
    storage?.setItem(sessionKey(s.account, s.packageId), JSON.stringify(s));
  } catch {
    /* ignore */
  }
}
export function clearStoredSession(
  account: string,
  packageId: string,
  storage: StringStorage | null = defaultStorage(),
): void {
  try {
    storage?.removeItem(sessionKey(account, packageId));
  } catch {
    /* ignore */
  }
}

// ── Active session ───────────────────────────────────────────────────────────

export type ActiveSession = {
  userStorageId: string;
  sessionAddress: string;
  keypair: Ed25519Keypair;
  expiresAt: number;
};

/** The active (non-expired) session for this account+package, or null. */
export function getActiveSession(
  account: string,
  packageId: string,
  nowMs: number = Date.now(),
  storage: StringStorage | null = defaultStorage(),
): ActiveSession | null {
  const s = loadStoredSession(account, packageId, storage);
  if (!s || s.expiresAt <= nowMs) return null;
  let keypair: Ed25519Keypair;
  try {
    keypair = Ed25519Keypair.fromSecretKey(s.sessionSecretKey);
  } catch {
    return null;
  }
  return { userStorageId: s.userStorageId, sessionAddress: s.sessionAddress, keypair, expiresAt: s.expiresAt };
}

// ── Sui client / wallet plumbing ──────────────────────────────────────────────

function networkOf(chain: `sui:${string}`): "testnet" | "mainnet" | "devnet" | "localnet" {
  const net = chain.slice("sui:".length);
  return net === "mainnet" || net === "devnet" || net === "localnet" ? net : "testnet";
}
function suiClientFor(ctx: MineSessionContext): SuiJsonRpcClient {
  const net = networkOf(ctx.chain);
  return new SuiJsonRpcClient({ url: getJsonRpcFullnodeUrl(net), network: net });
}

/** Owner-signed PTB via the connected Wallet-Standard wallet. Returns the tx digest. */
async function walletSign(ctx: MineSessionContext, transaction: Transaction): Promise<string> {
  const wallet = ctx.wallet as WalletWithFeatures<SuiSignAndExecuteTransactionFeature>;
  const feature = wallet.features[SuiSignAndExecuteTransaction];
  const out = await feature.signAndExecuteTransaction({
    transaction,
    account: ctx.account,
    chain: ctx.chain,
  });
  return out.digest;
}

const USER_STORAGE_TYPE_SUFFIX = "::dapp_service::UserStorage";

/** Find the UserStorage created by `digest` (after init_user_storage). */
async function createdUserStorageId(client: SuiJsonRpcClient, digest: string): Promise<string | null> {
  await client.waitForTransaction({ digest });
  const full = await client.getTransactionBlock({ digest, options: { showObjectChanges: true } });
  for (const change of full.objectChanges ?? []) {
    if (
      change.type === "created" &&
      typeof change.objectType === "string" &&
      change.objectType.endsWith(USER_STORAGE_TYPE_SUFFIX)
    ) {
      return change.objectId;
    }
  }
  return null;
}

/** Scan the owner's history for an existing UserStorage (cross-device / no local cache). */
async function findExistingUserStorageId(
  client: SuiJsonRpcClient,
  owner: string,
): Promise<string | null> {
  const res = await client.queryTransactionBlocks({
    filter: { FromAddress: owner },
    options: { showObjectChanges: true },
    order: "ascending",
    limit: 50,
  });
  for (const tx of res.data) {
    for (const change of tx.objectChanges ?? []) {
      if (
        change.type === "created" &&
        typeof change.objectType === "string" &&
        change.objectType.endsWith(USER_STORAGE_TYPE_SUFFIX)
      ) {
        return change.objectId;
      }
    }
  }
  return null;
}

// ── Public flow ────────────────────────────────────────────────────────────

/**
 * Ensure the owner has a `UserStorage` (one per account, idempotent). Returns its id.
 * Cached in localStorage; otherwise looked up on-chain; otherwise created (one wallet popup).
 */
export async function ensureUserStorage(ctx: MineSessionContext): Promise<string> {
  const { deployment, account } = ctx;
  const cached = loadUserStorageId(account.address, deployment.packageId);
  if (cached) return cached;

  const client = suiClientFor(ctx);
  const existing = await findExistingUserStorageId(client, account.address);
  if (existing) {
    persistUserStorageId(account.address, deployment.packageId, existing);
    return existing;
  }

  const tx = new Transaction();
  tx.moveCall({
    target: `${deployment.packageId}::user_storage_init::init_user_storage`,
    arguments: [tx.object(deployment.dappHubId), tx.object(deployment.dappStorageId)],
  });
  const digest = await walletSign(ctx, tx);
  const created = await createdUserStorageId(client, digest);
  if (!created) throw new Error("init_user_storage did not create a UserStorage");
  persistUserStorageId(account.address, deployment.packageId, created);
  return created;
}

/**
 * Authorize an ephemeral session wallet for `durationMs` (one owner-signed popup), then
 * persist it so subsequent `mine_batch`es sign locally. Returns the active session.
 */
export async function activateSession(
  ctx: MineSessionContext,
  durationMs: number = SESSION_DEFAULT_DURATION_MS,
  nowMs: number = Date.now(),
): Promise<ActiveSession> {
  const { deployment, account } = ctx;
  const userStorageId = await ensureUserStorage(ctx);

  const keypair = Ed25519Keypair.generate();
  const sessionAddress = keypair.toSuiAddress();

  const tx = new Transaction();
  tx.moveCall({
    target: `${deployment.frameworkPackageId}::dapp_system::activate_session`,
    typeArguments: [`${deployment.packageId}::dapp_key::DappKey`],
    arguments: [
      tx.object(deployment.dappHubId),
      tx.object(userStorageId),
      tx.pure.address(sessionAddress),
      tx.pure.u64(BigInt(Math.floor(durationMs))),
      tx.object(deployment.clockId),
    ],
  });
  await walletSign(ctx, tx);

  const expiresAt = nowMs + Math.floor(durationMs);
  persistStoredSession({
    account: account.address,
    packageId: deployment.packageId,
    userStorageId,
    sessionAddress,
    sessionSecretKey: keypair.getSecretKey(),
    expiresAt,
  });
  return { userStorageId, sessionAddress, keypair, expiresAt };
}

/** Revoke the active session (one owner-signed popup) and clear local state. */
export async function deactivateSession(ctx: MineSessionContext): Promise<void> {
  const { deployment, account } = ctx;
  const userStorageId = loadUserStorageId(account.address, deployment.packageId);
  if (userStorageId) {
    const tx = new Transaction();
    tx.moveCall({
      target: `${deployment.frameworkPackageId}::dapp_system::deactivate_session`,
      typeArguments: [`${deployment.packageId}::dapp_key::DappKey`],
      arguments: [tx.object(deployment.dappHubId), tx.object(userStorageId)],
    });
    await walletSign(ctx, tx);
  }
  clearStoredSession(account.address, deployment.packageId);
}

/**
 * Sign + submit ONE `mine_batch` with the ephemeral SESSION key — NO wallet popup. The
 * framework credits the UserStorage's canonical owner. The session wallet pays gas (it must
 * hold a little SUI). `params` carries the batch swings/nonce/fee; the UserStorage id comes
 * from the session.
 */
export async function executeMineBatchWithSession(
  deployment: OnchainMineDeployment,
  session: ActiveSession,
  params: Omit<MineBatchParams, "userStorageId">,
  network: "testnet" | "mainnet" | "devnet" | "localnet" = "testnet",
): Promise<OnchainTxResult> {
  const tx = buildMineBatchTransaction(deployment, { ...params, userStorageId: session.userStorageId });
  const client = new SuiJsonRpcClient({ url: getJsonRpcFullnodeUrl(network), network });
  const out = await client.signAndExecuteTransaction({
    signer: session.keypair,
    transaction: tx,
    options: { showEffects: true },
  });
  return { digest: out.digest, effects: JSON.stringify(out.effects ?? null) };
}
