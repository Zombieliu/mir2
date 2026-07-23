import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { projectFinalizedNodeEvents, type FinalizedNodeEvent } from '../src/node-registry';

const domain = Buffer.from('obelisk.guild-node.ed25519.v1\0');
const firstKey = Buffer.alloc(32, 7);
const secondKey = Buffer.alloc(32, 9);
const nodeId = [...createHash('sha256').update(domain).update(firstKey).digest()];

const event = (
  name: string,
  checkpoint: number,
  eventSequence: number,
  parsedJson: Record<string, unknown>,
): FinalizedNodeEvent => ({
  type: `0xabc::node_registry::${name}`,
  parsedJson,
  transactionDigest: `tx-${checkpoint}`,
  eventSequence,
  checkpoint,
});

test('projects finalized registration, key rotation, metadata and revocation deterministically', () => {
  const snapshot = projectFinalizedNodeEvents('testnet', '0xabc', [
    event('NodeRevokedEvent', 4, 0, {
      node_id: nodeId,
      operator: '0x11',
      returned_stake_mist: '1000000',
      generation: '2',
    }),
    event('NodeRegisteredEvent', 1, 0, {
      node_id: nodeId,
      operator: '0x11',
      public_key: [...firstKey],
      endpoint: [...Buffer.from('node-a:7020')],
      failure_domain: [...Buffer.from('az-a')],
      stake_mist: '1000000',
      max_sessions: '64',
      max_zones: '8',
      generation: '1',
    }),
    event('NodeKeyRotatedEvent', 2, 0, {
      node_id: nodeId,
      operator: '0x11',
      public_key: [...secondKey],
      generation: '2',
    }),
    event('NodeMetadataUpdatedEvent', 3, 0, {
      node_id: nodeId,
      endpoint: [...Buffer.from('node-a-new:7020')],
      failure_domain: [...Buffer.from('az-b')],
      max_sessions: '128',
      max_zones: '12',
      generation: '2',
    }),
  ]);
  assert.equal(snapshot.length, 1);
  assert.equal(snapshot[0]?.status, 'revoked');
  assert.equal(snapshot[0]?.keyGeneration, 2);
  assert.equal(snapshot[0]?.publicKey, secondKey.toString('base64url'));
  assert.equal(snapshot[0]?.endpoint, 'node-a-new:7020');
  assert.equal(snapshot[0]?.stakeMist, 0);
  assert.equal(snapshot[0]?.finality.checkpoint, 4);
});

test('rejects a registration whose stable node id does not match its first public key', () => {
  assert.throws(
    () =>
      projectFinalizedNodeEvents('testnet', '0xabc', [
        event('NodeRegisteredEvent', 1, 0, {
          node_id: new Array(32).fill(0),
          operator: '0x11',
          public_key: [...firstKey],
          endpoint: [...Buffer.from('node-a:7020')],
          failure_domain: [...Buffer.from('az-a')],
          stake_mist: '1000000',
          max_sessions: '64',
          max_zones: '8',
          generation: '1',
        }),
      ]),
    /does not match public key/,
  );
});
