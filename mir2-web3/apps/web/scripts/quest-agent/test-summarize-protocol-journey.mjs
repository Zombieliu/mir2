import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { summarizeProtocolClass } from './summarize-protocol-journey.mjs';

function line(value) {
  return `${JSON.stringify(value)}\n`;
}

function snapshot({ completed = [], active = [], level = 7, position = { x: 10, y: 20 } } = {}) {
  return {
    mapFileName: '0',
    playerObjectId: 1000,
    playerHp: 30,
    playerMaxHp: 40,
    entities: [{ objectId: 1000, kind: 'selfPlayer', level, hp: 30, maxHp: 40, ...position }],
    questLog: [
      ...completed.map(questId => ({ questId, title: `q${questId}`, stage: 'Completed', current: 1, required: 1 })),
      ...active,
    ],
  };
}

test('streams the latest session observation and separates mandatory counts from active progress', async () => {
  const outputRoot = await mkdtemp(path.join(os.tmpdir(), 'protocol-summary-'));
  try {
    const trace = [
      { direction: 'lifecycle', type: 'connection', at: '2026-09-11T10:00:00.000Z' },
      { direction: 'received', type: 'worldSnapshot', payload: snapshot({ completed: [1, 2, 3, 2_100_015, 2_100_020, 2_100_025, 2_100_030] }), at: '2026-09-11T10:00:01.000Z' },
      { direction: 'received', type: 'packet', packet: 'Death', payload: { location: { x: 10, y: 20 } }, at: '2026-09-11T10:00:02.000Z' },
      { direction: 'sent', type: 'townRevive', at: '2026-09-11T10:00:03.000Z' },
      { direction: 'lifecycle', type: 'connection', at: '2026-09-11T11:00:00.000Z' },
      { direction: 'received', type: 'worldSnapshot', payload: snapshot({
        completed: [1, 2, 2_100_015, 2_100_025, 2_100_035],
        level: 40,
        active: [{
          questId: 5,
          title: "The Smith's 1st Test",
          stage: 'InProgress',
          current: 3,
          required: 10,
          objectives: [{ label: 'Kill Deer', current: 3, required: 5, done: false }],
        }, { questId: 154, title: 'Optional', stage: 'InProgress', current: 0, required: 1 }],
      }), at: '2026-09-11T11:00:01.000Z' },
      { direction: 'received', type: 'packet', packet: 'UserLocation', payload: { x: 25, y: 31, direction: 'Right' }, at: '2026-09-11T11:00:02.000Z' },
      { direction: 'received', type: 'packet', packet: 'Death', payload: { location: { x: 25, y: 31 } }, at: '2026-09-11T11:00:03.000Z' },
      { direction: 'sent', type: 'townRevive', at: '2026-09-11T11:00:04.000Z' },
      { direction: 'received', type: 'packet', packet: 'Revived', payload: {}, at: '2026-09-11T11:00:05.000Z' },
      { direction: 'received', type: 'packet', packet: 'HealthChanged', payload: { hp: 18, maxHp: 40, mp: 5, maxMp: 10 }, at: '2026-09-11T11:00:06.000Z' },
    ];
    await writeFile(path.join(outputRoot, 'Warrior.trace.jsonl'), trace.map(line).join('') + '{partial');
    await writeFile(path.join(outputRoot, 'Warrior.private.json'), '{this must never be read');
    await writeFile(path.join(outputRoot, 'Warrior.report.json'), JSON.stringify({
      runId: 'old',
      startedAt: '2026-09-11T10:00:00.000Z',
      finishedAt: '2026-09-11T10:00:10.000Z',
      error: 'stale failure',
    }));

    const result = await summarizeProtocolClass({ outputRoot, className: 'Warrior' });
    assert.equal(result.level, 40);
    assert.equal(result.map, '0');
    assert.deepEqual(result.hp, { current: 18, maximum: 40 });
    assert.deepEqual(result.selfPosition, { x: 25, y: 31 });
    assert.deepEqual(result.mandatory, { completed: 2, total: 55 });
    assert.deepEqual(result.growthMilestones, { completed: 2, total: 4 });
    assert.deepEqual(result.activeQuests, [{
      questId: 5,
      title: "The Smith's 1st Test",
      stage: 'InProgress',
      current: 3,
      required: 10,
      objectives: [{ label: 'Kill Deer', current: 3, required: 5, done: false }],
      mandatory: true,
    }, {
      questId: 154,
      title: 'Optional',
      stage: 'InProgress',
      current: 0,
      required: 1,
      mandatory: false,
    }]);
    assert.equal(result.deathCount, 2);
    assert.equal(result.townReviveCount, 2);
    assert.equal(result.firstTimestamp, '2026-09-11T10:00:00.000Z');
    assert.equal(result.lastTimestamp, '2026-09-11T11:00:06.000Z');
    assert.equal(result.authoritativeSnapshotAt, '2026-09-11T11:00:01.000Z');
    assert.equal(result.latestFailure, null, 'a report from the prior session must not be attached');
    assert.equal(result.malformedLineCount, 1);
    assert.equal('completed' in result, false, 'kills or level must never infer whole-journey completion');
  } finally {
    await rm(outputRoot, { recursive: true, force: true });
  }
});

test('includes only whitelisted fields from a failure report overlapping the latest trace session', async () => {
  const outputRoot = await mkdtemp(path.join(os.tmpdir(), 'protocol-summary-report-'));
  try {
    await writeFile(path.join(outputRoot, 'Wizard.trace.jsonl'), [
      { direction: 'lifecycle', type: 'connection', at: '2026-09-11T12:00:00.000Z' },
      { direction: 'received', type: 'worldSnapshot', payload: snapshot(), at: '2026-09-11T12:00:01.000Z' },
      { direction: 'sent', type: 'keepAlive', at: '2026-09-11T12:00:05.000Z' },
    ].map(line).join(''));
    await writeFile(path.join(outputRoot, 'Wizard.report.json'), JSON.stringify({
      runId: 'current',
      startedAt: '2026-09-11T12:00:00.500Z',
      finishedAt: '2026-09-11T12:00:06.000Z',
      error: 'No walk path',
      name: 'must-not-leak',
      gatewayUrl: 'must-not-leak',
      quests: [{ privateEvidence: 'must-not-leak' }],
    }));

    const result = await summarizeProtocolClass({ outputRoot, className: 'Wizard' });
    assert.deepEqual(result.latestFailure, {
      runId: 'current',
      startedAt: '2026-09-11T12:00:00.500Z',
      finishedAt: '2026-09-11T12:00:06.000Z',
      error: 'No walk path',
    });
  } finally {
    await rm(outputRoot, { recursive: true, force: true });
  }
});
