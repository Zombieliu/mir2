import test from 'node:test';
import assert from 'node:assert/strict';
import { powHash } from '../scripts/faucet-v3.mjs';

test('faucet v3 Argon2d matches the official published conformance vector', () => {
  // https://faucet.sui.io/pow-spec/pow-vectors.json (public deterministic test data).
  const challenge = { chainId: '8wWZfv1HjQxjB5ncmC9CQqqRqUMBiEZXnc5FhS9zRe1v', checkpointSeq: '1713342',
    checkpointDigest: 'GzBdc9tbUe3s2PK1iVJcyQBj5TvdX7bJiAHWbhBDYiYb',
    randomBytes: 'mJ4ZqDXQasy17GTiGfp0/OYAGCR+7reLM5+Gb39YtoAIaPuh1wj+z5J7po+eSze3',
    faucetAddress: '0x949cd75a2485cc18b8258634ffd9b849a687e2e2a0b4c2444a21f007fc79adc4', recipient: '0x' + '11'.repeat(32) };
  assert.equal(powHash(challenge, 201n).toString('hex'), '0008faf113e0b071f5f9d5d6e1dc9321890ecb7e81059f07dedbd3dec5e902bb');
  assert.equal(powHash(challenge, 201n).readBigUInt64BE(), 2527713141239921n);
});
