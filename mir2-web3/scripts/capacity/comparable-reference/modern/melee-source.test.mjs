// Offline source-closure checks for the actual melee CLI. No socket is opened.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { hash, MAP_SHA } from './contract.mjs';

const definition = async name => fs.readFile(new URL(`./${name}`, import.meta.url)).catch(error => {
  if (error.code !== 'ENOENT') throw error; return fs.readFile(new URL(`../${name}`, import.meta.url));
});

function plan(ruleBytes, combatBytes) {
  const rules = JSON.parse(ruleBytes), contract = JSON.parse(combatBytes);
  const value = { schema: 'mir2.comparable.plan.v1', qualificationLayer: 'game-core-local', scenario: 'dispersed-melee',
    purpose: 'functional', population: 2, durationMs: 60000, capacityClaimAllowed: false, definitionSha256: hash(ruleBytes),
    generatorSourceSha256: 'a'.repeat(64), planId: '0123456789abcdefabcd', map: rules.map, navigationSha256: 'b'.repeat(64),
    actorTemplate: rules.actorTemplate, resourceContract: rules.resourceContract, causality: rules.causality, thresholds: rules.thresholds,
    combatContractSha256: hash(combatBytes), combatContractCanonicalSha256: hash(contract), combatContract: contract,
    scheduling: { version: 2, chatPhaseRule: 'same-primary-phase-with-midpoint-offset', primaryIntervalMs: 1200,
      minimumSameActorOfferGapMs: 600, chatOffsetWithinPrimaryIntervalMs: 600 },
    assessment: { windowMs: 60000, perActor: true, allCompleteWindowsRequired: true, failedActionCanBecomeCompleted: false,
      notSentIsTimeout: false, notSentIsCompleted: false, safetyOrEvidenceFailureIsHardFailure: true },
    equipmentActorOrdinals: [0, 1], equipmentRoundsPerActor: 2, actors: [], actions: [] };
  for (let ordinal = 0; ordinal < 2; ordinal++) {
    const start = { x: 10 + ordinal, y: 10 }, phase = ordinal * 600;
    value.actors.push({ ordinal, group: 0, peerOrdinals: [1 - ordinal], start, lane: [start, { ...start }], attackTargetOrdinal: 1 - ordinal });
    const actions = [];
    for (let dueMs = phase; dueMs < 60000; dueMs += 1200) actions.push({ actorOrdinal: ordinal, kind: 'attack', dueMs, deadlineMs: dueMs + 2500, targetActorOrdinal: 1 - ordinal });
    for (let dueMs = 15000 + phase; dueMs < 60000; dueMs += 30000) actions.push({ actorOrdinal: ordinal, kind: 'chat', dueMs, deadlineMs: dueMs + 2500 });
    actions.sort((a, b) => a.dueMs - b.dueMs || a.kind.localeCompare(b.kind));
    for (const [i, action] of actions.entries()) { action.ticketSequence = i + 1;
      if (action.kind === 'chat') { action.nonce = `m2cmp:${value.planId}:${ordinal}:${i + 1}`; action.nonceSha256 = hash(action.nonce); }
      value.actions.push(action);
    }
  }
  value.actions.sort((a, b) => a.dueMs - b.dueMs || a.actorOrdinal - b.actorOrdinal || a.ticketSequence - b.ticketSequence);
  return value;
}

async function fixture() {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-melee-cli-source-sdk-'));
  assert.match(root, /^c:[\\/]/i); await fs.mkdir(path.join(root, 'private'));
  const write = async (name, value) => fs.writeFile(path.join(root, name), typeof value === 'string' ? value : JSON.stringify(value));
  const files = [];
  for (const name of ['driver.mjs', 'contract.mjs', 'combat.mjs', 'melee-driver.mjs']) {
    const content = await fs.readFile(new URL(name, import.meta.url)); await fs.writeFile(path.join(root, name), content);
    files.push({ path: name, bytes: content.length, sha256: hash(content) });
  }
  const rules = await definition('workload.v2.json'), contract = await definition('combat-contract.v1.json');
  const p = plan(rules, contract), planBytes = JSON.stringify(p);
  await write('plan.json', planBytes); await write('combat-contract.json', contract.toString());
  const marker = path.join(root, 'ws-module-loaded.txt'), ws = path.join(root, 'ws.cjs');
  const wsBytes = Buffer.from(`require('node:fs').writeFileSync(${JSON.stringify(marker)},'synthetic-only');\nmodule.exports=class {constructor(){throw new Error('SDK_PROVENANCE_VALIDATED_STOP');}};\n`);
  await fs.writeFile(ws, wsBytes);
  const sourceManifest = { schema: 'mir2.comparable.modern-adapter-sources.v1', entrypoint: 'melee-driver.mjs', files,
    wsModule: { path: ws, bytes: wsBytes.length, sha256: hash(wsBytes) } };
  await write('adapter-sources.json', sourceManifest);
  const cohort = { schema: 'mir2.comparable.modern-cohort.v1', qualificationLayer: 'game-core-local', mapSha256: MAP_SHA,
    planSha256: hash(planBytes), accountStore: path.join(root, 'private/accounts.json'),
    actors: [0, 1].map(ordinal => ({ ordinal, accountId: `synthetic${ordinal}`, name: `Hero${ordinal}`, password: 'sdk-not-a-real-password', characterIndex: 0 })) };
  await write('private/cohort.json', cohort);
  const ledger = { schema: 'mir2.comparable.local-auth-ledger.v1', purpose: 'comparable-local-only', planSha256: hash(planBytes),
    endpoint: 'ws://127.0.0.1:19010/ws', attempts: [{ atMs: Date.now() - 1000000, purpose: 'retainedSyntheticHistory' }] };
  const makeRuntime = schema => {
    const build = { schema, baseRevision: '31e9b6a74661c31d6515dd7322b466285f975d71',
      changesKind: schema.includes('observer-timing') ? 'observer-timing-only' : 'spectator-scene-projection',
      buildActualExitCode: 0, binarySha256: 'b'.repeat(64), sourceInputManifestSha256: 'c'.repeat(64) };
    return { build, runtime: { schema: 'mir2.comparable.modern-runtime.v1', qualificationLayer: 'game-core-local', backend: 'file', sessionCache: 'in-memory',
      sourceRevision: build.baseRevision, baseRevision: build.baseRevision, binarySha256: build.binarySha256,
      changesKind: build.changesKind, buildActualExitCode: 0, sourceInputManifestSha256: build.sourceInputManifestSha256,
      buildManifestSha256: hash(JSON.stringify(build)), endpoint: ledger.endpoint, accountStore: cohort.accountStore, planSha256: cohort.planSha256,
      adapterSourceManifestSha256: hash(JSON.stringify(sourceManifest)), adapterEntrypointSha256: files.find(f => f.path === 'melee-driver.mjs').sha256 } };
  };
  const execute = async (name, { build, runtime }, sources = sourceManifest, omit = []) => {
    await write('private/ledger.json', ledger); await write('runtime.json', runtime); await write('build.json', build); await write('adapter-sources.json', sources);
    await fs.rm(marker, { force: true });
    const options = ['--plan', 'plan.json', '--cohort', 'private/cohort.json', '--runtime-receipt', 'runtime.json', '--ledger', 'private/ledger.json',
      '--output', name, '--ws-module', 'ws.cjs', '--combat-contract', 'combat-contract.json', '--build-manifest', 'build.json', '--adapter-sources', 'adapter-sources.json'];
    const args = [];
    for (let i = 0; i < options.length; i += 2) if (!omit.includes(options[i])) args.push(options[i], path.join(root, options[i + 1]));
    try { return { result: await promisify(execFile)(process.execPath, [path.join(root, 'melee-driver.mjs'), ...args], { timeout: 10000, maxBuffer: 1048576 }), code: 0 }; }
    catch (error) { return { result: error, code: error.code }; }
  };
  return { root, write, ledger, marker, sourceManifest, makeRuntime, execute };
}

test('actual melee CLI inherits both reviewed build/source bindings and emits metadata before controlled SDK transport failure', { skip: process.platform !== 'win32' }, async () => {
  const f = await fixture();
  for (const [i, schema] of ['mir2.comparable.observer-timing-build.v1', 'mir2.comparable.spectator-scene-build.v1'].entries()) {
    const pins = f.makeRuntime(schema), run = await f.execute(`positive${i}`, pins);
    assert.equal(run.code, 1); assert.equal(run.result.stderr, '');
    const report = JSON.parse(await fs.readFile(path.join(f.root, `positive${i}.report.json`), 'utf8'));
    assert.equal(report.failure, 'SDK_PROVENANCE_VALIDATED_STOP'); assert.equal(report.admitted, 0); assert.equal(report.ok, false);
    assert.equal(report.capacityClaimAllowed, false); assert.equal(report.measurementSloPassed, false);
    assert.equal(report.sourceRevisionIsBaseOnly, true); assert.equal(report.additionalSource.buildManifestSchema, schema);
    assert.equal(report.additionalSource.changesKind, pins.build.changesKind); assert.equal(report.additionalSource.cleanBaseBuild, false);
    assert.equal(report.adapterSourceManifestSha256, pins.runtime.adapterSourceManifestSha256);
    assert.equal(report.adapterEntrypointSha256, pins.runtime.adapterEntrypointSha256); assert.deepEqual(report.adapterSource.files, f.sourceManifest.files);
    assert.deepEqual(report.adapterSource.wsModule, f.sourceManifest.wsModule);
    const actual = JSON.parse(await fs.readFile(path.join(f.root, 'private/ledger.json'), 'utf8'));
    assert.deepEqual(actual.attempts.slice(0, 1), f.ledger.attempts); assert.equal(actual.attempts.length, 2);
    assert.equal(await fs.readFile(f.marker, 'utf8'), 'synthetic-only');
  }
});

test('actual melee CLI rejects missing, cross-labeled or mismatched source evidence before WS module load or ledger mutation', { skip: process.platform !== 'win32' }, async () => {
  const f = await fixture(), scene = 'mir2.comparable.spectator-scene-build.v1';
  const cases = [
    { omit: ['--adapter-sources'], expected: /requires explicit --adapter-sources/ },
    { omit: ['--build-manifest'], expected: /requires explicit --build-manifest/ },
    { change: p => { p.build.changesKind = 'observer-timing-only'; }, expected: /candidate build manifest invalid/ },
    { change: p => { p.build.schema = 'mir2.comparable.unreviewed-build.v1'; }, expected: /candidate build manifest invalid/ },
    { change: p => { p.runtime.buildManifestSha256 = '0'.repeat(64); }, expected: /candidate manifest binding mismatch/ },
    { change: p => { p.runtime.sourceInputManifestSha256 = '0'.repeat(64); }, expected: /candidate manifest binding mismatch/ },
    { change: p => { p.runtime.adapterSourceManifestSha256 = '0'.repeat(64); }, expected: /Runtime adapter source manifest/ },
    { change: p => { p.runtime.adapterEntrypointSha256 = '0'.repeat(64); }, expected: /Runtime adapter source manifest/ },
    { source: s => { s.entrypoint = 'driver.mjs'; }, expected: /entrypoint differs from actual/ },
    { source: s => { s.files = s.files.filter(r => r.path !== 'combat.mjs'); }, expected: /Required executed adapter/ },
    { source: s => { s.files[0].sha256 = '0'.repeat(64); }, expected: /file bytes\/hash mismatch/ },
    { source: s => { s.wsModule.sha256 = '0'.repeat(64); }, expected: /WebSocket module bytes\/hash mismatch/ },
  ];
  for (const [i, spec] of cases.entries()) {
    const pins = f.makeRuntime(scene), sources = structuredClone(f.sourceManifest); spec.change?.(pins); spec.source?.(sources);
    const run = await f.execute(`negative${i}`, pins, sources, spec.omit);
    assert.equal(run.code, 1); assert.match(run.result.stderr, spec.expected);
    assert.deepEqual(JSON.parse(await fs.readFile(path.join(f.root, 'private/ledger.json'), 'utf8')), f.ledger);
    await assert.rejects(fs.stat(f.marker), { code: 'ENOENT' });
    await assert.rejects(fs.stat(path.join(f.root, `negative${i}.events.jsonl`)), { code: 'ENOENT' });
    await assert.rejects(fs.stat(path.join(f.root, `negative${i}.report.json`)), { code: 'ENOENT' });
  }
});
