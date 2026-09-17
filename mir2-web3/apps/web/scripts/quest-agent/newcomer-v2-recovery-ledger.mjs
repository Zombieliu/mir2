import { readFile } from 'node:fs/promises';

const MAX_V2_RECOVERIES = 3;
const ISO_INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;

/**
 * Read the local V2 resume ledger. A missing report is a new ordinary run;
 * every existing V2 report must account for every recorded death explicitly.
 */
export async function loadV2RecoveryLedger(reportPath) {
  let prior;
  try {
    prior = JSON.parse(await readFile(reportPath, 'utf8'));
  } catch (error) {
    if (error?.code === 'ENOENT') return null;
    throw new Error(`cannot read V2 resume ledger: ${String(error?.message ?? error)}`);
  }
  return parseV2RecoveryLedger(prior);
}

/** Parse and clone a persisted V2 ledger without changing its original clock. */
export function parseV2RecoveryLedger(prior) {
  if (!isRecord(prior)) throw new Error('invalid existing V2 resume ledger');
  if (!Object.hasOwn(prior, 'v2')) return null;
  const v2 = prior.v2;
  if (!isRecord(v2) || v2.profile !== 'newcomer-v2') {
    throw new Error('invalid existing V2 resume ledger');
  }
  if (!isIsoInstant(v2.ordinaryStartedAt)) {
    throw new Error('invalid existing V2 ordinary start time');
  }
  return {
    ordinaryStartedAt: v2.ordinaryStartedAt,
    recoveries: cloneConfirmedV2Recoveries(v2.recoveries),
  };
}

/**
 * Preserve only completed public death/town-revive receipts. An incomplete
 * checkpoint is not silently reset: the operator must retain or repair it.
 */
export function cloneConfirmedV2Recoveries(recoveries) {
  if (!Array.isArray(recoveries) || recoveries.length > MAX_V2_RECOVERIES) {
    throw new Error('invalid existing V2 recovery ledger');
  }
  for (const entry of recoveries) assertConfirmedRecovery(entry);
  return structuredClone(recoveries);
}

function assertConfirmedRecovery(entry) {
  if (!isRecord(entry) || typeof entry.phase !== 'string' || entry.phase.length === 0 ||
      !(entry.questId === null || Number.isSafeInteger(entry.questId)) ||
      !isIsoInstant(entry.deathAt) || !isIsoInstant(entry.revivedAt) ||
      Date.parse(entry.revivedAt) < Date.parse(entry.deathAt) ||
      !sameDeadOwner(entry.before, entry.revive?.before) ||
      !sameLivingOwner(entry.after, entry.revive?.after) ||
      !isIsoInstant(entry.revive?.at) ||
      entry.before.objectId !== entry.after.objectId) {
    throw new Error('invalid existing V2 recovery ledger');
  }
}

function sameDeadOwner(summary, receipt) {
  return isOwnerSummary(summary, 0) && isRecord(receipt) && receipt.dead === true &&
    receipt.hp === 0 && receipt.objectId === summary.objectId && receipt.map === summary.mapFileName;
}

function sameLivingOwner(summary, receipt) {
  return isOwnerSummary(summary, 1) && isRecord(receipt) && receipt.dead === false &&
    Number.isFinite(receipt.hp) && receipt.hp > 0 && receipt.hp === summary.hp &&
    receipt.objectId === summary.objectId && receipt.map === summary.mapFileName;
}

function isOwnerSummary(value, minimumHp) {
  return isRecord(value) && typeof value.mapFileName === 'string' && value.mapFileName.length > 0 &&
    Number.isSafeInteger(value.objectId) && Number.isSafeInteger(value.x) && Number.isSafeInteger(value.y) &&
    Number.isFinite(value.hp) && value.hp >= minimumHp && (minimumHp === 0 ? value.hp === 0 : value.hp > 0);
}

function isIsoInstant(value) {
  if (typeof value !== 'string' || !ISO_INSTANT.test(value)) return false;
  const milliseconds = Date.parse(value);
  return Number.isFinite(milliseconds) && new Date(milliseconds).toISOString() === value;
}

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
