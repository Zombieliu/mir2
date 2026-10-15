import { readFile, rename, writeFile } from 'node:fs/promises';

const MAX_V2_RECOVERIES = 3;

/** Keep the original clock and death receipts even when login fails before
 * the journey runner can create its first progress checkpoint. */
export async function primeV2RunReport(reportPath, report, ledger) {
  if (ledger) {
    const prior = JSON.parse(await readFile(reportPath, 'utf8'));
    const stored = parseV2RecoveryLedger(prior);
    if (!stored || stored.ordinaryStartedAt !== ledger.ordinaryStartedAt ||
        JSON.stringify(stored.recoveries) !== JSON.stringify(ledger.recoveries)) {
      throw new Error('V2 resume report changed after the recovery ledger was loaded');
    }
    report.v2 = structuredClone(prior.v2);
    if (Object.hasOwn(prior, 'ordinaryElapsedMs')) report.ordinaryElapsedMs = prior.ordinaryElapsedMs;
    return;
  }
  try {
    await readFile(reportPath, 'utf8');
    throw new Error('existing report has no V2 recovery ledger');
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
  }
  if (!isIsoInstant(report.startedAt)) throw new Error('invalid fresh V2 ordinary start time');
  report.v2 = {
    profile: 'newcomer-v2', status: 'bootstrapping', routeQuestIds: [], attempts: [],
    recoveries: [], ordinaryStartedAt: report.startedAt,
    deadlineAt: new Date(Date.parse(report.startedAt) + FUNCTIONAL_RECHECK_DURATION_MS).toISOString(),
    elapsedMs: 0, ordinaryElapsedMs: 0,
  };
  const temporaryPath = `${reportPath}.${process.pid}.bootstrap.tmp`;
  await writeFile(temporaryPath, JSON.stringify(report, null, 2));
  await rename(temporaryPath, reportPath);
}
export const FUNCTIONAL_RECHECK_DURATION_MS = 120 * 60_000;
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

/**
 * Open the separately labelled functional recheck clock. This is deliberately
 * unavailable for a fresh or still-active ordinary run: the persisted
 * ordinary evidence remains intact and is never repurposed as a new clock.
 */
export async function loadV2FunctionalRecheckLedger(reportPath, {
  functionalRecheckStartedAt,
  nowMs = Date.now(),
} = {}) {
  let prior;
  try {
    prior = JSON.parse(await readFile(reportPath, 'utf8'));
  } catch (error) {
    if (error?.code === 'ENOENT') throw new Error('functional recheck requires an existing expired ordinary V2 ledger');
    throw new Error(`cannot read V2 functional recheck ledger: ${String(error?.message ?? error)}`);
  }
  return resolveV2FunctionalRecheckLedger(prior, { functionalRecheckStartedAt, nowMs });
}

/**
 * Durably record the first shared recheck clock before a caller opens a
 * network connection. A resume validates the same persisted clock instead of
 * accepting a replacement start after a process crash.
 */
export async function persistV2FunctionalRecheckLedger(reportPath, {
  functionalRecheckStartedAt,
  nowMs = Date.now(),
} = {}) {
  let prior;
  try {
    prior = JSON.parse(await readFile(reportPath, 'utf8'));
  } catch (error) {
    throw new Error(`cannot persist V2 functional recheck ledger: ${String(error?.message ?? error)}`);
  }
  const ledger = resolveV2FunctionalRecheckLedger(prior, { functionalRecheckStartedAt, nowMs });
  prior.v2.functionalRecheckStartedAt = ledger.functionalRecheck.startedAt;
  prior.v2.functionalRecheckDeadlineAt = ledger.functionalRecheck.deadlineAt;
  prior.v2.functionalRecheckElapsedMs = ledger.functionalRecheck.elapsedMs;
  const temporaryPath = `${reportPath}.${process.pid}.functional-recheck.tmp`;
  await writeFile(temporaryPath, JSON.stringify(prior, null, 2));
  await rename(temporaryPath, reportPath);
  return ledger;
}

/** Validate an opt-in recheck against its persisted ordinary evidence. */
export function resolveV2FunctionalRecheckLedger(prior, { functionalRecheckStartedAt, nowMs = Date.now() } = {}) {
  const ledger = parseV2RecoveryLedger(prior);
  if (!ledger) throw new Error('functional recheck requires an existing expired ordinary V2 ledger');
  if (!isIsoInstant(functionalRecheckStartedAt)) throw new Error('functional recheck requires MIR2_V2_FUNCTIONAL_RECHECK_STARTED_AT as an ISO instant');
  const ordinaryDeadlineAt = prior.v2.deadlineAt;
  if (!isIsoInstant(ordinaryDeadlineAt) || Date.parse(ordinaryDeadlineAt) !== Date.parse(ledger.ordinaryStartedAt) + FUNCTIONAL_RECHECK_DURATION_MS) {
    throw new Error('functional recheck requires an unmodified persisted ordinary V2 deadline');
  }
  if (Number(nowMs) < Date.parse(ordinaryDeadlineAt)) throw new Error('functional recheck requires an expired ordinary V2 ledger');
  const persisted = functionalRecheckFromReport(prior.v2);
  const startedAt = persisted?.startedAt ?? functionalRecheckStartedAt;
  if (persisted && persisted.startedAt !== functionalRecheckStartedAt) {
    throw new Error('functional recheck start does not match the persisted recheck ledger');
  }
  if (Date.parse(startedAt) > Number(nowMs)) throw new Error('functional recheck start cannot be in the future');
  if (Date.parse(startedAt) < Date.parse(ordinaryDeadlineAt)) throw new Error('functional recheck start cannot precede the expired ordinary V2 deadline');
  if (persisted && persisted.elapsedMs > Number(nowMs) - Date.parse(startedAt)) {
    throw new Error('invalid persisted functional recheck elapsed time');
  }
  const deadlineAt = new Date(Date.parse(startedAt) + FUNCTIONAL_RECHECK_DURATION_MS).toISOString();
  if (persisted && persisted.deadlineAt !== deadlineAt) throw new Error('invalid persisted functional recheck deadline');
  if (Number(nowMs) >= Date.parse(deadlineAt)) throw new Error('functional recheck 120-minute deadline has expired');
  return {
    ...ledger,
    ordinaryDeadlineAt,
    ordinaryElapsedMs: prior.ordinaryElapsedMs ?? prior.v2.ordinaryElapsedMs,
    functionalRecheck: {
      startedAt,
      deadlineAt,
      elapsedMs: persisted?.elapsedMs ?? 0,
    },
  };
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

function functionalRecheckFromReport(v2) {
  const fields = ['functionalRecheckStartedAt', 'functionalRecheckDeadlineAt', 'functionalRecheckElapsedMs'];
  if (!fields.some(field => Object.hasOwn(v2, field))) return null;
  if (!fields.every(field => Object.hasOwn(v2, field)) ||
      !isIsoInstant(v2.functionalRecheckStartedAt) || !isIsoInstant(v2.functionalRecheckDeadlineAt) ||
      !Number.isFinite(v2.functionalRecheckElapsedMs) || v2.functionalRecheckElapsedMs < 0) {
    throw new Error('invalid persisted functional recheck ledger');
  }
  return {
    startedAt: v2.functionalRecheckStartedAt,
    deadlineAt: v2.functionalRecheckDeadlineAt,
    elapsedMs: v2.functionalRecheckElapsedMs,
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
