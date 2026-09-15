#!/usr/bin/env node
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const helper = fileURLToPath(new URL('./verify-three-class-saved-route.mjs', import.meta.url));
const configPath = process.env.MIR2_NEWCOMER_CONFIG ?? path.resolve(process.cwd(), 'config/quest-guidance/newcomer-journey-v1.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
const classes = [{ className: 'Warrior', name: 'JWa3693f66', index: 8 }, { className: 'Wizard', name: 'JWdebac6d4', index: 9 }, { className: 'Taoist', name: 'JT3f4a4e1f', index: 10 }];
const milestoneIds = [2100015, 2100020, 2100025, 2100030];
const ids = c => [...new Set(config.chapters.flatMap(ch => [...(ch.questIds ?? []), ...(ch.classQuestIds?.[c] ?? [])]))].map(Number);
const all = c => [...ids(c), ...milestoneIds];
function makeFixture(root, { wrongMap = false, unknownLevel = false, missingPrelogoutOwner = false, deadFinal = false, wrongIdentity = false } = {}) {
  fs.mkdirSync(path.join(root, 'gateway'), { recursive: true });
  const accounts = {};
  for (const c of classes) {
    const done = all(c.className);
    const states = done.map(id => JSON.stringify({ quest_id: id, stage: 'completed', current: 1 }));
    const save = { revision: 1, character: { index: c.index, name: c.name, level: unknownLevel ? null : 30, class: c.className }, map_file_name: wrongMap ? '9' : '0', position: { x: 10, y: 20 }, hp: 100, mp: 50, quest_states_json: states };
    accounts[`synthetic-${c.className.toLowerCase()}`] = { characters: [{ index: c.index, name: c.name, level: 30, class: c.className }], saves: { [c.index]: save } };
    const snapshot = { mapFileName: '0', playerObjectId: 1, playerHp: deadFinal ? 0 : 100, playerMp: 50, questLog: done.map(questId => ({ questId, stage: 'completed' })), entities: missingPrelogoutOwner ? [] : [{ objectId: 1, kind: 'selfPlayer', name: wrongIdentity ? `Wrong${c.name}` : c.name, class: c.className, level: unknownLevel ? null : 30, hp: deadFinal ? 0 : 100, x: 10, y: 20, dead: deadFinal }] };
    const trace = path.join(root, `${c.className}.trace.jsonl`);
    const postLogout = { type: 'worldSnapshot', direction: 'received', sequence: 4, at: '2026-01-01T00:00:02.000Z', payload: { mapFileName: null, playerObjectId: null, questLog: [] , entities: [] } };
    fs.writeFileSync(trace, JSON.stringify({ type: 'worldSnapshot', direction: 'received', sequence: 1, at: '2026-01-01T00:00:00.000Z', payload: snapshot }) + '\n' + JSON.stringify({ type: 'logOut', direction: 'sent', sequence: 2, at: '2026-01-01T00:00:01.000Z' }) + '\n' + JSON.stringify({ type: 'packet', direction: 'received', packet: 'LogOutSuccess', sequence: 3, at: '2026-01-01T00:00:01.500Z' }) + '\n' + JSON.stringify(postLogout) + '\n');
    fs.writeFileSync(path.join(root, `${c.className}.report.json`), JSON.stringify({ className: wrongIdentity ? 'Wizard' : c.className, name: wrongIdentity ? `Wrong${c.name}` : c.name, characterIndex: wrongIdentity ? c.index + 1 : c.index, completed: true, finishedAt: '2026-01-01T00:00:01.000Z', traceFile: trace }));
    fs.writeFileSync(path.join(root, `${c.className}.snapshot.json`), JSON.stringify(snapshot));
  }
  fs.writeFileSync(path.join(root, 'gateway/accounts.json'), JSON.stringify({ accounts }));
}
function run(root) { return spawnSync(process.execPath, [helper], { env: { ...process.env, MIR2_PROTOCOL_ROOT: root, MIR2_NEWCOMER_CONFIG: configPath }, encoding: 'utf8' }); }
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'saved-route-fixture-'));
try {
  makeFixture(root);
  const pass = run(root); if (pass.status !== 0) throw new Error(`baseline synthetic proof unexpectedly failed: ${pass.status}`);
  makeFixture(root, { wrongMap: true });
  const wrongMap = run(root); if (wrongMap.status !== 2) throw new Error(`wrong-map fixture did not fail with exit 2: ${wrongMap.status}`);
  makeFixture(root, { unknownLevel: true });
  const unknownLevel = run(root); if (unknownLevel.status !== 2) throw new Error(`unknown-level fixture did not fail with exit 2: ${unknownLevel.status}`);
  makeFixture(root, { missingPrelogoutOwner: true });
  const missingPrelogoutOwner = run(root); if (missingPrelogoutOwner.status !== 2) throw new Error(`missing-prelogout-owner fixture did not fail with exit 2: ${missingPrelogoutOwner.status}`);
  makeFixture(root, { deadFinal: true });
  const deadFinal = run(root); if (deadFinal.status !== 2) throw new Error(`dead-final fixture did not fail with exit 2: ${deadFinal.status}`);
  makeFixture(root, { wrongIdentity: true });
  const wrongIdentity = run(root); if (wrongIdentity.status !== 2) throw new Error(`wrong-identity fixture did not fail with exit 2: ${wrongIdentity.status}`);
  console.log(JSON.stringify({ result: 'PASS', baselineExit: pass.status, wrongMapExit: wrongMap.status, unknownLevelExit: unknownLevel.status, missingPrelogoutOwnerExit: missingPrelogoutOwner.status, deadFinalExit: deadFinal.status, wrongIdentityExit: wrongIdentity.status, syntheticOnly: true }));
} finally { fs.rmSync(root, { recursive: true, force: true }); }
