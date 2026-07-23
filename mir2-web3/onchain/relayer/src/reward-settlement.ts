import { Transaction } from '@mysten/sui/transactions';

/** JSON shape emitted by `mir2_gateway::RewardSettlementBatch`. */
export interface RewardSettlementBatch {
  batchId: string;
  gameId: string;
  epoch: number | string;
  merkleRoot: string;
  totalReward: number | string;
  allocationCount: number;
  finalizedControlHeight: number | string;
  settlementCoinType: string;
}

export interface RewardSettlementObjects {
  packageId: string;
  registryId: string;
  adminCapId: string;
}

export interface VerifiedRewardClaim {
  batchId: string;
  nodeId: string;
  recipient: string;
  amount: number | string;
}

const encoder = new TextEncoder();

function requireObjectId(label: string, value: string): void {
  if (!/^0x[0-9a-fA-F]+$/.test(value)) throw new Error(`${label} must be a 0x object id`);
}

function requirePositiveInteger(label: string, value: number | string): bigint {
  const parsed = BigInt(value);
  if (parsed <= 0n) throw new Error(`${label} must be positive`);
  return parsed;
}

function hexBytes(label: string, value: string): number[] {
  if (!/^[0-9a-fA-F]{64}$/.test(value)) throw new Error(`${label} must be a 32-byte hex digest`);
  return Array.from({ length: 32 }, (_, index) => Number.parseInt(value.slice(index * 2, index * 2 + 2), 16));
}

function textBytes(label: string, value: string): number[] {
  const bytes = Array.from(encoder.encode(value));
  if (bytes.length === 0 || bytes.length > 255) throw new Error(`${label} must be 1..255 UTF-8 bytes`);
  return bytes;
}

function validateObjects(objects: RewardSettlementObjects): void {
  requireObjectId('packageId', objects.packageId);
  requireObjectId('registryId', objects.registryId);
  requireObjectId('adminCapId', objects.adminCapId);
}

/**
 * Build, but never sign, the capability-gated Sui transaction publishing a finalized game epoch.
 * Key custody stays in the operator signer/HSM surrounding this pure adapter, never on guild nodes.
 */
export function buildPublishRewardBatchTransaction(
  objects: RewardSettlementObjects,
  batch: RewardSettlementBatch,
): Transaction {
  validateObjects(objects);
  const epoch = BigInt(batch.epoch);
  if (epoch < 0n) throw new Error('epoch must not be negative');
  const totalReward = requirePositiveInteger('totalReward', batch.totalReward);
  const controlHeight = BigInt(batch.finalizedControlHeight);
  if (controlHeight < 0n) throw new Error('finalizedControlHeight must not be negative');
  if (!Number.isInteger(batch.allocationCount) || batch.allocationCount <= 0) {
    throw new Error('allocationCount must be positive');
  }
  if (batch.settlementCoinType !== '0x2::sui::SUI') {
    throw new Error('Gate 9 Move settlement currently accepts 0x2::sui::SUI only');
  }

  const tx = new Transaction();
  tx.moveCall({
    target: `${objects.packageId}::reward_settlement::publish_batch`,
    arguments: [
      tx.object(objects.registryId),
      tx.object(objects.adminCapId),
      tx.pure.vector('u8', hexBytes('batchId', batch.batchId)),
      tx.pure.vector('u8', textBytes('gameId', batch.gameId)),
      tx.pure.u64(epoch),
      tx.pure.vector('u8', hexBytes('merkleRoot', batch.merkleRoot)),
      tx.pure.u64(totalReward),
      tx.pure.u32(batch.allocationCount),
      tx.pure.u64(controlHeight),
    ],
  });
  return tx;
}

/** Build a payout after the operator has verified the Rust Merkle proof. */
export function buildPayVerifiedClaimTransaction(
  objects: RewardSettlementObjects,
  claim: VerifiedRewardClaim,
): Transaction {
  validateObjects(objects);
  requireObjectId('recipient', claim.recipient);
  const tx = new Transaction();
  tx.moveCall({
    target: `${objects.packageId}::reward_settlement::pay_verified_claim`,
    arguments: [
      tx.object(objects.registryId),
      tx.object(objects.adminCapId),
      tx.pure.vector('u8', hexBytes('batchId', claim.batchId)),
      tx.pure.vector('u8', textBytes('nodeId', claim.nodeId)),
      tx.pure.address(claim.recipient),
      tx.pure.u64(requirePositiveInteger('amount', claim.amount)),
    ],
  });
  return tx;
}
