import { FinalizedNodeRegistrySource } from './node-registry';
import type { SuiNetwork } from './config';

async function main(): Promise<void> {
  const packageId = process.env.NODE_REGISTRY_PACKAGE_ID;
  if (!packageId) {
    throw new Error('NODE_REGISTRY_PACKAGE_ID is required');
  }
  const network = (process.env.SUI_NETWORK ?? 'testnet') as SuiNetwork;
  const source = new FinalizedNodeRegistrySource(network, packageId);
  const snapshot = await source.snapshot();
  const expectedActiveNode = process.env.EXPECT_ACTIVE_NODE_ID;
  if (
    expectedActiveNode &&
    !snapshot.some((node) => node.nodeId === expectedActiveNode && node.status === 'active')
  ) {
    throw new Error(`expected active node ${expectedActiveNode} was not finalized`);
  }
  console.log(JSON.stringify({ network, packageId, snapshot }, null, 2));
}

main().catch((error) => {
  console.error(
    '[node-registry-snapshot] fatal:',
    error instanceof Error ? error.message : error,
  );
  process.exit(1);
});
