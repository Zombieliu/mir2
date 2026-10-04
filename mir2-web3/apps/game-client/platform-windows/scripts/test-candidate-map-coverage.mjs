import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { resolveCandidateMapScope, compareCandidateMapScope, describeCandidateMapTopology } from './audit-candidate-map-coverage.mjs';
import { loadCrystalQuestRouteSources } from '../../../web/scripts/quest-agent/route-manifest.mjs';

const project = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const profileBytes = fs.readFileSync(path.join(project, 'packages/game-data/data/content_profiles/platinum_176.json'));
const profile = JSON.parse(profileBytes);
const sources = await loadCrystalQuestRouteSources();
const scope = resolveCandidateMapScope({ profile, profileBytes });
const legacy = ['0', '1', '2', '3', '0108', '0109', '0141', 'd001', 'd021', 'd022', 'd401', 'd601', 'd602', 'd605', 'd607'];
const changed = update => { const value = structuredClone(profile); update(value); return { profile: value, profileBytes: Buffer.from(JSON.stringify(value)) }; };

test('default covers every admitted map, retaining merchants, Sabuk and classic branches', () => {
  assert.equal(scope.expectedMaps.length, 164);
  assert.equal(scope.profileId, 'platinum_176');
  assert.equal(scope.profileVersion, profile.version);
  assert.equal(scope.completeClassicWorld, false);
  for (const name of ['0103', '0122', '0150', '0155', 'd715', 'd515', 'd10054', 'd10062']) assert.ok(scope.expectedMaps.includes(name));
  assert.deepEqual(compareCandidateMapScope(scope, profile.mapWhitelist.map(map => map.fileName)), { mapNames: scope.expectedMaps, omittedMaps: [], unexpectedMaps: [] });
});

test('omitting one admitted Red Moon side branch cannot pass the old journey minimum', () => {
  const selection = compareCandidateMapScope(scope, scope.expectedMaps.filter(name => name !== 'd10054'));
  assert.deepEqual(selection.omittedMaps, ['d10054']);
  assert.equal(selection.mapNames.filter(name => legacy.includes(name)).length, 15);
});

test('explicit legacy maps remain exactly selected and cannot be called full profile', () => {
  const explicit = resolveCandidateMapScope({ profile, profileBytes, mode: 'explicit', expectedMaps: [...legacy, 'D001'] });
  assert.equal(explicit.expectedMaps.length, 15);
  assert.equal(explicit.mode, 'explicit');
  assert.ok(!explicit.expectedMaps.includes('d10054'));
  assert.deepEqual(compareCandidateMapScope(explicit, legacy).omittedMaps, []);
  assert.throws(() => resolveCandidateMapScope({ profile, profileBytes, expectedMaps: legacy }), /differ from the content profile/);
  assert.throws(() => resolveCandidateMapScope({ profile, profileBytes, mode: 'explicit' }), /nonempty safe map/);
});

test('scope rejects another profile, unbound bytes and duplicate or unsafe profile identities', () => {
  assert.throws(() => resolveCandidateMapScope(changed(value => { value.profileId = 'later-world'; })), /identified platinum_176/);
  assert.throws(() => resolveCandidateMapScope({ profile, profileBytes: Buffer.from('{}') }), /bytes do not match/);
  assert.throws(() => resolveCandidateMapScope(changed(value => { value.mapWhitelist.push({ fileName: 'D10054' }); })), /duplicate/);
  assert.throws(() => resolveCandidateMapScope(changed(value => { value.mapWhitelist[0].fileName = '0:debug'; })), /safe map/);
  assert.throws(() => resolveCandidateMapScope(changed(value => { value.mapWhitelist = value.mapWhitelist.filter(map => map.fileName !== '0150'); })), /previously shipped map: 0150/);
});

test('unsafe, duplicated and unexpected manifest identities are rejected or identified', () => {
  assert.throws(() => compareCandidateMapScope(scope, [...scope.expectedMaps, 'D10054']), /duplicate/);
  assert.throws(() => compareCandidateMapScope(scope, [...scope.expectedMaps, '../d10054']), /safe map/);
  assert.deepEqual(compareCandidateMapScope(scope, [...scope.expectedMaps, 'd71653']).unexpectedMaps, ['d71653']);
});

test('topology records conditional entries separately from ordinary walking and live acceptance', () => {
  const topology = describeCandidateMapTopology(sources, scope.expectedMaps);
  assert.deepEqual(topology.unreachableMaps, []);
  assert.equal(topology.conditionalEntries.length, 16);
  assert.equal(topology.excludedClassicMaps.length, 45);
  assert.equal(topology.runtimeGameplayAccepted, false);
  const taoTomb = topology.conditionalEntries.find(entry => entry.map === 'd10061');
  assert.deepEqual(taoTomb.conditions.map(edge => [edge.from, edge.to, edge.needMove]), [['D10051', 'D10061', true]]);
  const ancientStone = topology.conditionalEntries.find(entry => entry.map === 'd710a');
  assert.deepEqual(ancientStone.conditions[0].itemCosts, [{ item: 'StoneHeart', count: 1 }]);
});

test('source orphan D71653 is classified without inventing entry or expanding the actual profile', () => {
  const before = JSON.stringify(sources.contentProfile);
  const admitted = describeCandidateMapTopology(sources, scope.expectedMaps);
  assert.deepEqual(admitted.sourceOrphanRooms.map(room => [room.map, room.includedInExpectedScope]), [['d71653', false]]);
  const future = describeCandidateMapTopology(sources, [...scope.expectedMaps, ...admitted.excludedClassicMaps]);
  assert.deepEqual(future.unreachableMaps, ['d71653']);
  assert.deepEqual(future.sourceOrphanRooms.map(room => [room.map, room.includedInExpectedScope]), [['d71653', true]]);
  assert.equal(JSON.stringify(sources.contentProfile), before);
  assert.equal(sources.contentProfile.mapWhitelist.length, 164);
});

test('CLI fails omitted scope before any atlas completion or asset read', t => {
  const parent = path.resolve(process.env.MIR2_MAP_TEST_OUTPUT_ROOT ?? os.tmpdir());
  fs.mkdirSync(parent, { recursive: true });
  const fixture = fs.mkdtempSync(path.join(parent, 'classic-map-scope-'));
  if (!process.env.MIR2_MAP_TEST_OUTPUT_ROOT) t.after(() => {
    const relative = path.relative(parent, path.resolve(fixture));
    assert.ok(!relative.startsWith('..') && !path.isAbsolute(relative) && path.basename(fixture).startsWith('classic-map-scope-'));
    fs.rmSync(fixture, { recursive: true, force: true });
  });
  const manifest = path.join(fixture, 'manifest.json'), receipt = path.join(fixture, 'rejected.json');
  const completion = path.join(fixture, 'must-not-be-created');
  fs.writeFileSync(manifest, JSON.stringify({ mapFileNames: scope.expectedMaps.filter(name => name !== 'd10054') }));
  const result = spawnSync(process.execPath, [fileURLToPath(new URL('./audit-candidate-map-coverage.mjs', import.meta.url)),
    '--manifest', manifest, '--output', receipt, '--completeAtlasRoot', completion], { encoding: 'utf8' });
  assert.equal(result.status, 1, result.stderr);
  const rejected = JSON.parse(fs.readFileSync(receipt));
  assert.equal(rejected.passed, false);
  assert.deepEqual(rejected.omittedMaps, ['d10054']);
  assert.equal(rejected.scope.expectedMaps.length, 164);
  assert.equal(fs.existsSync(completion), false);
});
