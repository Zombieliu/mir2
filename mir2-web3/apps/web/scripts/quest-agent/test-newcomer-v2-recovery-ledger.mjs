import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { loadV2RecoveryLedger, parseV2RecoveryLedger } from './newcomer-v2-recovery-ledger.mjs';

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
