import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildPayVerifiedClaimTransaction,
  buildPublishRewardBatchTransaction,
  type RewardSettlementBatch,
} from '../src/reward-settlement';

const objects = {
  packageId: '0xabc',
  registryId: '0xdef',
  adminCapId: '0x123',
};

const batch: RewardSettlementBatch = {
  batchId: 'ab'.repeat(32),
  gameId: 'mir2',
  epoch: 7,
  merkleRoot: 'cd'.repeat(32),
  totalReward: '1000',
  allocationCount: 3,
  finalizedControlHeight: 42,
  settlementCoinType: '0x2::sui::SUI',
};

test('builds unsigned capability-gated publish and claim transactions', () => {
  const publish = buildPublishRewardBatchTransaction(objects, batch);
  const claim = buildPayVerifiedClaimTransaction(objects, {
    batchId: batch.batchId,
    nodeId: 'guild-a',
    recipient: '0xbeef',
    amount: 250,
  });
  const publishData = JSON.stringify(publish.getData());
  const claimData = JSON.stringify(claim.getData());
  assert.match(publishData, /"module":"reward_settlement"/);
  assert.match(publishData, /"function":"publish_batch"/);
  assert.match(claimData, /"function":"pay_verified_claim"/);
});

test('rejects malformed roots, unsupported coins, and non-positive claims', () => {
  assert.throws(
    () => buildPublishRewardBatchTransaction(objects, { ...batch, merkleRoot: 'bad' }),
    /32-byte hex/,
  );
  assert.throws(
    () => buildPublishRewardBatchTransaction(objects, { ...batch, settlementCoinType: '0xcoin::x::X' }),
    /currently accepts/,
  );
  assert.throws(
    () =>
      buildPayVerifiedClaimTransaction(objects, {
        batchId: batch.batchId,
        nodeId: 'guild-a',
        recipient: '0xbeef',
        amount: 0,
      }),
    /must be positive/,
  );
});
