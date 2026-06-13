import 'dotenv/config';
import { Dubhe, loadMetadata, type NetworkType } from '@0xobelisk/sui-client';
import { Transaction } from '@mysten/sui/transactions';

/**
 * SK1 testnet smoke for the on-chain smart mine (Dubhe 1.2.x dappHub/UserStorage model).
 *
 * Ensures the miner has a `UserStorage`, sends one owner-signed `mine_batch`, asserts the
 * `MineSettledEvent`, and reads the UserStorage back. The session-signed (popup-free) path
 * is exercised separately — see `onchain-sk/scripts/session-spike.ts` (SK0) and SK2.
 *
 * Run (after `dubhe publish` populates the ids):
 *   1. cp onchain/.env.example onchain/.env
 *   2. set PRIVATE_KEY (miner key), MINE_PACKAGE_ID, FRAMEWORK_PACKAGE_ID, DAPP_HUB_ID,
 *      DAPP_STORAGE_ID (from deployments/testnet.json), and NONCE (on-chain miner_nonce + 1).
 *   3. ensure mine #1 is seeded (admin_system::set_mine) — done at deploy.
 *   4. pnpm tsx scripts/smoke-mine.ts
 *
 * NOTE: type-checked but intentionally NOT executed by the agent — running it needs a
 * funded private key in .env, which by policy never enters the agent transcript.
 */

const NETWORK = (process.env.SUI_NETWORK ?? 'testnet') as NetworkType;
const RANDOM_OBJECT_ID = '0x8';
const CLOCK_OBJECT_ID = '0x6';
const MINE_ID = 1n;
const SWINGS = 5n;

function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`Missing ${name} in onchain/.env`);
  return value;
}

async function main(): Promise<void> {
  const packageId = requireEnv('MINE_PACKAGE_ID');
  const frameworkPackageId = requireEnv('FRAMEWORK_PACKAGE_ID');
  const dappHubId = requireEnv('DAPP_HUB_ID');
  const dappStorageId = requireEnv('DAPP_STORAGE_ID');
  const secretKey = requireEnv('PRIVATE_KEY');
  const dappKey = `${packageId}::dapp_key::DappKey`;

  const metadata = await loadMetadata(NETWORK, packageId);
  const dubhe = new Dubhe({
    networkType: NETWORK,
    packageId,
    metadata,
    secretKey,
    frameworkPackageId,
    dappHubId,
    dappKey,
  });
  const miner = dubhe.getAddress();
  console.log(`miner:   ${miner}`);
  console.log(`package: ${packageId}`);

  // The miner needs a UserStorage before they can hold ore (one-time, idempotent).
  let userStorageId = await dubhe.getUserStorageId(miner);
  if (!userStorageId) {
    await dubhe.initUserStorage({ dappHubId, dappStorageId });
    userStorageId = await dubhe.getUserStorageId(miner);
  }
  if (!userStorageId) throw new Error('UserStorage not found after init');
  console.log(`userStorage: ${userStorageId}`);

  // nonce must be on-chain miner_nonce + 1 (default 1 for a fresh miner).
  const nonce = BigInt(process.env.NONCE ?? '1');
  console.log(`nonce:   ${nonce}`);

  // mine_batch(dapp_storage, user_storage, mine_id, swings, nonce, fee, Random=0x8, Clock=0x6).
  const tx = new Transaction();
  const [fee] = tx.splitCoins(tx.gas, [0]);
  tx.moveCall({
    target: `${packageId}::mine_system::mine_batch`,
    arguments: [
      tx.object(dappStorageId),
      tx.object(userStorageId),
      tx.pure.u64(MINE_ID),
      tx.pure.u64(SWINGS),
      tx.pure.u64(nonce),
      fee,
      tx.object(RANDOM_OBJECT_ID),
      tx.object(CLOCK_OBJECT_ID),
    ],
  });

  const res = (await dubhe.signAndSendTxn({ tx })) as {
    digest?: string;
    events?: Array<{ type: string; parsedJson?: unknown }>;
  };
  console.log(`mine_batch tx: ${res?.digest}`);

  const settled = (res?.events ?? []).find((e) => e.type.includes('MineSettledEvent'));
  if (!settled) throw new Error('MineSettledEvent not found in tx effects');
  console.log('MineSettledEvent:', JSON.stringify(settled.parsedJson));

  const us = await dubhe.getUserStorageFields(userStorageId);
  console.log('canonical_owner:', us.canonical_owner, '(== miner)');
  console.log('write_count:', us.write_count);
  console.log('✅ smoke ok');
}

main().catch((error) => {
  console.error('❌ smoke failed:', error);
  process.exit(1);
});
