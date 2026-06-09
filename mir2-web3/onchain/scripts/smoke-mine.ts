import 'dotenv/config';
import { Dubhe, loadMetadata } from '@0xobelisk/sui-client';
import { Transaction } from '@mysten/sui/transactions';

/**
 * M1 testnet smoke for the on-chain smart mine (@0xobelisk/sui-client).
 *
 * Sends one `mine_batch`, asserts the `mine_settled` event, and reads back `ore_balance`
 * + `stones_left` via `parseState` — the M1 exit's TS smoke (ROADMAP §4). It mirrors the
 * flow already verified on testnet via the Sui CLI (mine_settled / ore credit / replay
 * guard / depletion; see `onchain/deployments/testnet.json`).
 *
 * Run:
 *   1. cp onchain/.env.example onchain/.env
 *   2. set PRIVATE_KEY (deployer/miner key — suiprivkey/base64/hex), MINE_PACKAGE_ID,
 *      MINE_SCHEMA_ID (from deployments/testnet.json)
 *   3. ensure mine #1 is seeded (admin_system::set_mine) — already done at deploy
 *   4. pnpm tsx scripts/smoke-mine.ts
 *
 * NOTE: this file is type-checked but intentionally NOT executed by the agent — running it
 * needs a funded private key in .env, which by policy never enters the agent transcript.
 * The contract is verified end-to-end on testnet via the CLI instead.
 */

type NetworkType = 'testnet' | 'mainnet' | 'devnet' | 'localnet';

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
  const schemaId = requireEnv('MINE_SCHEMA_ID');
  const secretKey = requireEnv('PRIVATE_KEY');

  const metadata = await loadMetadata(NETWORK, packageId);
  const dubhe = new Dubhe({ networkType: NETWORK, packageId, metadata, secretKey });
  const miner = dubhe.getAddress();
  console.log(`miner:   ${miner}`);
  console.log(`package: ${packageId}`);
  console.log(`schema:  ${schemaId}`);

  // nonce must be (current miner_nonce) + 1; default 0 if this miner never mined.
  const nonceState = await dubhe.parseState({
    schema: 'miner_nonce',
    objectId: schemaId,
    storageType: 'StorageMap',
    params: [miner],
  });
  const prevNonce = BigInt((nonceState?.[0] as string | number | undefined) ?? 0);
  const nonce = prevNonce + 1n;
  console.log(`nonce:   ${nonce}`);

  // Build the mine_batch PTB (mirrors the verified CLI ptb): split a 0-fee coin from gas,
  // then call mine_batch(schema, mine_id, swings, nonce, fee, Random=0x8, Clock=0x6).
  const tx = new Transaction();
  const [fee] = tx.splitCoins(tx.gas, [0]);
  tx.moveCall({
    target: `${packageId}::mine_system::mine_batch`,
    arguments: [
      tx.object(schemaId),
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

  // Assert the mine_settled event was emitted.
  const settled = (res?.events ?? []).find((e) =>
    e.type.includes('mine_settled') || e.type.includes('MineSettled'),
  );
  if (!settled) throw new Error('mine_settled event not found in tx effects');
  console.log('mine_settled:', JSON.stringify(settled.parsedJson));

  // Read back stones_left + ore_balance via parseState.
  const stones = await dubhe.parseState({
    schema: 'stones_left',
    objectId: schemaId,
    storageType: 'StorageMap',
    params: [MINE_ID],
  });
  console.log('stones_left[mine 1]:', stones?.[0]);

  // ore_balance is keyed by (miner, OreKind). Mine set 1 credits BlackIron.
  const oreBalance = await dubhe.parseState({
    schema: 'ore_balance',
    objectId: schemaId,
    storageType: 'StorageDoubleMap',
    params: [miner, { BlackIron: {} }],
  });
  console.log('ore_balance[miner, BlackIron]:', oreBalance?.[0]);

  console.log('✅ smoke ok');
}

main().catch((error) => {
  console.error('❌ smoke failed:', error);
  process.exit(1);
});
