/**
 * SK0 live session-key spike (run AFTER `dubhe publish` populates the ids below).
 *
 * Proves the Dubhe 1.2.x session flow end-to-end on testnet:
 *   1. owner `initUserStorage` (once) → owner's UserStorage
 *   2. owner `activateSession(sessionWallet, durationMs)` — ONE owner signature
 *   3. the ephemeral SESSION wallet signs `counter_system::bump` locally (no owner sig)
 *   4. read back: the OWNER's counter incremented (on-behalf-of write worked)
 *   5. owner `deactivateSession`
 *
 * The "no wallet popup" UX is a browser concern (SK2). In Node both keys are local
 * secretKeys, so this validates the *contract + SDK authorization*, not the popup UX.
 *
 * Env (see .env.example): OWNER_SECRET_KEY, PACKAGE_ID, FRAMEWORK_PACKAGE_ID,
 * DAPP_HUB_ID, DAPP_STORAGE_ID. SESSION_SECRET_KEY optional (else generated; fund it).
 */
import 'dotenv/config';
import { Dubhe, loadMetadata, NetworkType } from '@0xobelisk/sui-client';
import { SuiClient, getFullnodeUrl } from '@mysten/sui/client';
import { Ed25519Keypair } from '@mysten/sui/keypairs/ed25519';
import { Transaction } from '@mysten/sui/transactions';

const NETWORK: NetworkType = 'testnet';

function required(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`missing env ${name} (fill it from the deploy record)`);
  return value;
}

async function main(): Promise<void> {
  const packageId = required('PACKAGE_ID');
  const frameworkPackageId = required('FRAMEWORK_PACKAGE_ID');
  const dappHubId = required('DAPP_HUB_ID');
  const dappStorageId = required('DAPP_STORAGE_ID');
  const ownerSecretKey = required('OWNER_SECRET_KEY');
  const dappKey = `${packageId}::dapp_key::DappKey`;

  const metadata = await loadMetadata(NETWORK, packageId);
  const owner = new Dubhe({
    networkType: NETWORK,
    packageId,
    metadata,
    secretKey: ownerSecretKey,
    frameworkPackageId,
    dappHubId,
    dappKey,
  });
  const ownerAddress = owner.getAddress();
  console.log('owner:', ownerAddress);

  // 1. UserStorage (idempotent: skip if it already exists).
  let userStorageId = await owner.getUserStorageId(ownerAddress);
  if (!userStorageId) {
    await owner.initUserStorage({ dappHubId, dappStorageId });
    userStorageId = await owner.getUserStorageId(ownerAddress);
  }
  if (!userStorageId) throw new Error('UserStorage not found after init');
  console.log('userStorage:', userStorageId);

  // 2. Ephemeral session wallet — the locally-held key that will sign bumps.
  const sessionKp = process.env.SESSION_SECRET_KEY
    ? Ed25519Keypair.fromSecretKey(process.env.SESSION_SECRET_KEY)
    : Ed25519Keypair.generate();
  const sessionAddress = sessionKp.toSuiAddress();
  console.log('sessionWallet:', sessionAddress, '(must hold a little SUI for gas)');

  // 3. Owner authorizes the session (ONE owner signature) for 1 hour.
  await owner.activateSession({
    userStorageId,
    sessionWallet: sessionAddress,
    durationMs: 60 * 60 * 1000,
  });

  // 4. Session wallet signs bump locally — NO owner signature.
  const client = new SuiClient({ url: getFullnodeUrl(NETWORK) });
  const before = await owner.getUserStorageFields(userStorageId);
  const tx = new Transaction();
  tx.moveCall({
    target: `${packageId}::counter_system::bump`,
    arguments: [tx.object(userStorageId)],
  });
  const res = await client.signAndExecuteTransaction({ signer: sessionKp, transaction: tx });
  console.log('session-signed bump digest:', res.digest);

  // 5. Verify the OWNER's counter moved + tidy up.
  const after = await owner.getUserStorageFields(userStorageId);
  console.log('owner canonical_owner:', after.canonical_owner, '(== owner above)');
  console.log('write_count before/after:', before.write_count, '/', after.write_count);
  await owner.deactivateSession({ userStorageId });
  console.log('session deactivated ✓');
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
