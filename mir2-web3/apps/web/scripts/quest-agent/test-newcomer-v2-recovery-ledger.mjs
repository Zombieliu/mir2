import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  FUNCTIONAL_RECHECK_DURATION_MS,
  loadV2FunctionalRecheckLedger,
  loadV2RecoveryLedger,
  parseV2RecoveryLedger,
  persistV2FunctionalRecheckLedger,
  primeV2RunReport,
  resolveV2FunctionalRecheckLedger,
} from './newcomer-v2-recovery-ledger.mjs';

test('bootstrap rejection cannot replace a resumed V2 death and time ledger', async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'mir2-v2-bootstrap-resume-'));
  const reportPath = path.join(directory, 'Wizard.report.json');
  try {
    const prior = report([confirmedRecovery(), confirmedRecovery(), confirmedRecovery()]);
    prior.v2.status = 'paused';
    await writeFile(reportPath, JSON.stringify(prior));
    const ledger = await loadV2RecoveryLedger(reportPath);
    const next = { startedAt: new Date().toISOString() };
    await primeV2RunReport(reportPath, next, ledger);
    next.error = 'character is already online or route lease is unavailable';
    await writeFile(reportPath, JSON.stringify(next));
    const retained = await loadV2RecoveryLedger(reportPath);
    assert.equal(retained.ordinaryStartedAt, prior.v2.ordinaryStartedAt);
    assert.equal(retained.recoveries.length, 3);
    assert.equal(next.v2.status, 'paused');
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('fresh V2 run persists its clock before a login failure', async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'mir2-v2-bootstrap-fresh-'));
  const reportPath = path.join(directory, 'Wizard.report.json');
  try {
    const next = { startedAt: new Date().toISOString() };
    await primeV2RunReport(reportPath, next, null);
    assert.equal((await loadV2RecoveryLedger(reportPath)).ordinaryStartedAt, next.startedAt);
    next.error = 'login unavailable';
    await writeFile(reportPath, JSON.stringify(next));
    assert.equal((await loadV2RecoveryLedger(reportPath)).ordinaryStartedAt, next.startedAt);
    await assert.rejects(primeV2RunReport(reportPath, { startedAt: new Date().toISOString() }, null),
      /existing report has no V2 recovery ledger/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

function confirmedRecovery() {
  return {
    phase: 'quest', questId: 2110004,
    deathAt: '2026-09-17T20:41:15.152Z',
    before: { mapFileName: '0', objectId: 1000, x: 237, y: 493, hp: 0 },
    revive: {
      at: '2026-09-17T20:41:15.615Z',
      before: { map: '0', objectId: 1000, x: 237, y: 493, hp: 0, dead: true },
      after: { map: '0', objectId: 1000, x: 288, y: 616, hp: 39, dead: false },
    },
    revivedAt: '2026-09-17T20:41:15.615Z',
    after: { mapFileName: '0', objectId: 1000, x: 288, y: 616, hp: 39 },
  };
}

function report(recoveries) {
  const v2 = { profile: 'newcomer-v2', ordinaryStartedAt: '2026-09-17T20:00:00.000Z' };
  if (arguments.length === 0) v2.recoveries = [confirmedRecovery()];
  else v2.recoveries = recoveries;
  return { v2 };
}

test('loads one complete Tao-style death and town-revive receipt without extending its original clock', async () => {
  const prior = report();
  const directory = await mkdtemp(path.join(os.tmpdir(), 'mir2-v2-recovery-ledger-valid-'));
  const reportPath = path.join(directory, 'Taoist.report.json');
  try {
    await writeFile(reportPath, JSON.stringify(prior));
    const ledger = await loadV2RecoveryLedger(reportPath);
    assert.equal(ledger.ordinaryStartedAt, '2026-09-17T20:00:00.000Z');
    assert.deepEqual(ledger.recoveries, prior.v2.recoveries);
    ledger.recoveries[0].after.hp = 1;
    assert.equal(prior.v2.recoveries[0].after.hp, 39, 'resume evidence is clone-isolated');
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

for (const [label, recoveries] of [
  ['missing', undefined], ['null', null], ['string', 'one'],
  ['incomplete death checkpoint', [{ ...confirmedRecovery(), revivedAt: undefined }]],
  ['over the fixed cap', Array.from({ length: 4 }, confirmedRecovery)],
]) {
  test(`rejects an existing V2 ${label} ledger rather than resetting recovery credit`, () => {
    assert.throws(() => parseV2RecoveryLedger(report(recoveries)), /invalid existing V2 recovery ledger/);
  });
}

for (const prior of [null, 'report', [], 42]) {
  test(`rejects a non-object JSON report (${String(prior)}) rather than treating it as a first run`, () => {
    assert.throws(() => parseV2RecoveryLedger(prior), /invalid existing V2 resume ledger/);
  });
}

test('rejects a calendar-normalized ordinary start instead of accepting a different clock', () => {
  const prior = report();
  prior.v2.ordinaryStartedAt = '2026-02-30T20:00:00.000Z';
  assert.throws(() => parseV2RecoveryLedger(prior), /invalid existing V2 ordinary start time/);
});

test('a missing report is a first run with no carried recovery ledger', async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'mir2-v2-recovery-ledger-'));
  try {
    assert.equal(await loadV2RecoveryLedger(path.join(directory, 'Taoist.report.json')), null);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('a malformed existing report rejects resume rather than silently resetting recovery credit', async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'mir2-v2-recovery-ledger-malformed-'));
  const reportPath = path.join(directory, 'Taoist.report.json');
  try {
    await writeFile(reportPath, '{not json');
    await assert.rejects(loadV2RecoveryLedger(reportPath), /cannot read V2 resume ledger/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('an existing V1 report remains outside the V2 resume ledger', async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'mir2-v2-recovery-ledger-v1-'));
  const reportPath = path.join(directory, 'Taoist.report.json');
  try {
    await writeFile(reportPath, JSON.stringify({ completed: false, quests: [] }));
    assert.equal(await loadV2RecoveryLedger(reportPath), null);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

function expiredOrdinaryReport({ functionalRecheck = null } = {}) {
  const ordinaryStartedAt = new Date(Date.now() - FUNCTIONAL_RECHECK_DURATION_MS - 60_000).toISOString();
  const v2 = {
    profile: 'newcomer-v2', ordinaryStartedAt,
    deadlineAt: new Date(Date.parse(ordinaryStartedAt) + FUNCTIONAL_RECHECK_DURATION_MS).toISOString(),
    recoveries: [],
  };
  if (functionalRecheck) Object.assign(v2, functionalRecheck);
  return { ordinaryElapsedMs: FUNCTIONAL_RECHECK_DURATION_MS + 60_000, v2 };
}

test('functional recheck uses an explicit shared start while preserving the expired ordinary clock and recovery cap', () => {
  const prior = expiredOrdinaryReport();
  const startedAt = new Date(Date.now() - 1_000).toISOString();
  const ledger = resolveV2FunctionalRecheckLedger(prior, { functionalRecheckStartedAt: startedAt });
  assert.equal(ledger.ordinaryStartedAt, prior.v2.ordinaryStartedAt);
  assert.equal(ledger.ordinaryDeadlineAt, prior.v2.deadlineAt);
  assert.equal(ledger.ordinaryElapsedMs, prior.ordinaryElapsedMs);
  assert.deepEqual(ledger.recoveries, []);
  assert.deepEqual(ledger.functionalRecheck, {
    startedAt,
    deadlineAt: new Date(Date.parse(startedAt) + FUNCTIONAL_RECHECK_DURATION_MS).toISOString(),
    elapsedMs: 0,
  });
});

test('functional recheck resume requires the same persisted shared start and valid persisted deadline', async () => {
  const startedAt = new Date(Date.now() - 1_000).toISOString();
  const deadlineAt = new Date(Date.parse(startedAt) + FUNCTIONAL_RECHECK_DURATION_MS).toISOString();
  const prior = expiredOrdinaryReport({ functionalRecheck: {
    functionalRecheckStartedAt: startedAt,
    functionalRecheckDeadlineAt: deadlineAt,
    functionalRecheckElapsedMs: 900,
  } });
  const ledger = resolveV2FunctionalRecheckLedger(prior, { functionalRecheckStartedAt: startedAt });
  assert.equal(ledger.functionalRecheck.elapsedMs, 900);
  assert.throws(() => resolveV2FunctionalRecheckLedger(prior, {
    functionalRecheckStartedAt: new Date(Date.now() - 2_000).toISOString(),
  }), /does not match/);
  prior.v2.functionalRecheckDeadlineAt = new Date(Date.parse(deadlineAt) + 1).toISOString();
  assert.throws(() => resolveV2FunctionalRecheckLedger(prior, { functionalRecheckStartedAt: startedAt }), /invalid persisted functional recheck deadline/);

  const directory = await mkdtemp(path.join(os.tmpdir(), 'mir2-v2-functional-recheck-'));
  const reportPath = path.join(directory, 'Warrior.report.json');
  try {
    await writeFile(reportPath, JSON.stringify(expiredOrdinaryReport()));
    const persisted = await loadV2FunctionalRecheckLedger(reportPath, { functionalRecheckStartedAt: startedAt });
    assert.equal(persisted.functionalRecheck.startedAt, startedAt);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('functional recheck clock is atomically persisted before a later resume can select a replacement', async () => {
  const startedAt = new Date(Date.now() - 1_000).toISOString();
  const directory = await mkdtemp(path.join(os.tmpdir(), 'mir2-v2-functional-recheck-persist-'));
  const reportPath = path.join(directory, 'Wizard.report.json');
  try {
    await writeFile(reportPath, JSON.stringify(expiredOrdinaryReport()));
    await persistV2FunctionalRecheckLedger(reportPath, { functionalRecheckStartedAt: startedAt });
    const stored = JSON.parse(await readFile(reportPath, 'utf8'));
    assert.equal(stored.v2.functionalRecheckStartedAt, startedAt);
    assert.equal(stored.v2.functionalRecheckDeadlineAt,
      new Date(Date.parse(startedAt) + FUNCTIONAL_RECHECK_DURATION_MS).toISOString());
    await assert.rejects(persistV2FunctionalRecheckLedger(reportPath, {
      functionalRecheckStartedAt: new Date(Date.now() - 2_000).toISOString(),
    }), /does not match/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('functional recheck rejects fresh, active, malformed, and expired ordinary evidence', () => {
  const startedAt = new Date(Date.now() - 1_000).toISOString();
  assert.throws(() => resolveV2FunctionalRecheckLedger({ v2: {
    profile: 'newcomer-v2', ordinaryStartedAt: startedAt, recoveries: [],
  } }, { functionalRecheckStartedAt: startedAt }), /unmodified persisted ordinary V2 deadline/);
  const active = expiredOrdinaryReport();
  active.v2.ordinaryStartedAt = new Date(Date.now() - 1_000).toISOString();
  active.v2.deadlineAt = new Date(Date.parse(active.v2.ordinaryStartedAt) + FUNCTIONAL_RECHECK_DURATION_MS).toISOString();
  assert.throws(() => resolveV2FunctionalRecheckLedger(active, { functionalRecheckStartedAt: startedAt }), /requires an expired/);
  const longExpired = expiredOrdinaryReport();
  longExpired.v2.ordinaryStartedAt = new Date(Date.now() - 2 * FUNCTIONAL_RECHECK_DURATION_MS - 60_000).toISOString();
  longExpired.v2.deadlineAt = new Date(Date.parse(longExpired.v2.ordinaryStartedAt) + FUNCTIONAL_RECHECK_DURATION_MS).toISOString();
  assert.throws(() => resolveV2FunctionalRecheckLedger(longExpired, {
    functionalRecheckStartedAt: new Date(Date.parse(longExpired.v2.deadlineAt) + 1).toISOString(),
  }), /functional recheck 120-minute deadline has expired/);
  assert.throws(() => resolveV2FunctionalRecheckLedger(expiredOrdinaryReport(), {
    functionalRecheckStartedAt: new Date(Date.now() + 1_000).toISOString(),
  }), /cannot be in the future/);
  const beforeOrdinaryExpiry = expiredOrdinaryReport();
  assert.throws(() => resolveV2FunctionalRecheckLedger(beforeOrdinaryExpiry, {
    functionalRecheckStartedAt: beforeOrdinaryExpiry.v2.ordinaryStartedAt,
  }), /cannot precede/);
});
