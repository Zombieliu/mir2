import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { resolveCandidateMapScope, compareCandidateMapScope, describeCandidateMapTopology, proveCandidateResourceOnlyRooms } from './audit-candidate-map-coverage.mjs';
import { loadCrystalQuestRouteSources } from '../../../web/scripts/quest-agent/route-manifest.mjs';

const project = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const profileBytes = fs.readFileSync(path.join(project, 'packages/game-data/data/content_profiles/platinum_176.json'));
const profile = JSON.parse(profileBytes);
const sources = await loadCrystalQuestRouteSources();
const scope = resolveCandidateMapScope({ profile, profileBytes });
const legacy = ['0', '1', '2', '3', '0108', '0109', '0141', 'd001', 'd021', 'd022', 'd401', 'd601', 'd602', 'd605', 'd607'];
const changed = update => { const value = structuredClone(profile); update(value); return { profile: value, profileBytes: Buffer.from(JSON.stringify(value)) }; };

test('default covers every admitted map, retaining merchants, Sabuk and classic branches', () => {
  assert.equal(scope.expectedMaps.length, 209);
  assert.equal(scope.profileMapCount, 208);
  assert.equal(scope.profileNativeMapCount, 209);
  assert.deepEqual(scope.resourceOnlyMaps, ['d71653']);
  assert.equal(scope.profileId, 'platinum_176');
  assert.equal(scope.profileVersion, profile.version);
  assert.equal(scope.completeClassicWorld, false);
  for (const name of ['0103', '0122', '0150', '0155', 'd715', 'd515', 'd10054', 'd10062']) assert.ok(scope.expectedMaps.includes(name));
  assert.deepEqual(compareCandidateMapScope(scope, [...profile.mapWhitelist.map(map => map.fileName), ...profile.nativeResourceOnlyMaps]), { mapNames: scope.expectedMaps, omittedMaps: [], unexpectedMaps: [] });
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
  assert.deepEqual(explicit.resourceOnlyMaps, []);
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
  assert.deepEqual(compareCandidateMapScope(scope, [...scope.expectedMaps, 'd71654']).unexpectedMaps, ['d71654']);
});

test('topology records conditional entries separately from ordinary walking and live acceptance', () => {
  const topology = describeCandidateMapTopology(sources, scope.expectedMaps);
  assert.deepEqual(topology.unreachableMaps, ['d71653']);
  assert.deepEqual(topology.unreachableRuntimeMaps, []);
  assert.equal(topology.conditionalEntries.length, 16);
  assert.deepEqual(topology.excludedClassicMaps, ['d71653']);
  assert.equal(topology.runtimeGameplayAccepted, false);
  const taoTomb = topology.conditionalEntries.find(entry => entry.map === 'd10061');
  assert.deepEqual(taoTomb.conditions.map(edge => [edge.from, edge.to, edge.needMove]), [['D10051', 'D10061', true]]);
  const ancientStone = topology.conditionalEntries.find(entry => entry.map === 'd710a');
  assert.deepEqual(ancientStone.conditions[0].itemCosts, [{ item: 'StoneHeart', count: 1 }]);
});

test('source orphan D71653 is classified without inventing entry or expanding the actual profile', () => {
  const before = JSON.stringify(sources.contentProfile);
  const admitted = describeCandidateMapTopology(sources, scope.expectedMaps);
  assert.deepEqual(admitted.sourceOrphanRooms.map(room => [room.map, room.includedInExpectedScope]), [['d71653', true]]);
  assert.deepEqual(admitted.resourceOnlyRooms.map(room => [room.map, room.mapIndex, room.runtimeAdmitted]), [['d71653', 235, false]]);
  assert.deepEqual(admitted.unreachableMaps, ['d71653']);
  const runtimeOnly = describeCandidateMapTopology(sources, profile.mapWhitelist.map(map => map.fileName));
  assert.deepEqual(runtimeOnly.unreachableMaps, []);
  assert.deepEqual(runtimeOnly.resourceOnlyRooms, []);
  assert.equal(JSON.stringify(sources.contentProfile), before);
  assert.equal(sources.contentProfile.mapWhitelist.length, 208);
});

test('every added classic room is required, including the resource-only original orphan', () => {
  const added = ['d716', 'd717', ...Array.from({ length: 25 }, (_, i) => `d716${String(i + 1).padStart(2, '0')}`),
    'd71650', 'd71651', 'd71652', 'd71653', ...Array.from({ length: 6 }, (_, i) => `d506${i + 2}`), 'd5069',
    'd511', 'd512', 'd513', 'd514', 'd10012', 'd10013', 'd10032'];
  assert.equal(added.length, 45);
  for (const name of added) {
    assert.ok(scope.expectedMaps.includes(name));
    assert.deepEqual(compareCandidateMapScope(scope, scope.expectedMaps.filter(map => map !== name)).omittedMaps, [name]);
  }
});

test('another unreachable runtime branch is not excused by the named resource-only room', () => {
  const copy = { ...sources, contentProfile: structuredClone(profile) };
  copy.contentProfile.npcScriptWhitelist = copy.contentProfile.npcScriptWhitelist.filter(script => !['BichonProvince/Sailor', 'PrajnaIsland/Sailor'].includes(script));
  const topology = describeCandidateMapTopology(copy, scope.expectedMaps);
  assert.ok(topology.unreachableMaps.includes('d71653'));
  assert.ok(topology.unreachableRuntimeMaps.includes('5'));
  assert.ok(!topology.unreachableRuntimeMaps.includes('d71653'));
});

test('resource-only metadata cannot forge another room or runtime admission', () => {
  for (const rooms of [['D71652'], ['D71653', 'D71653'], ['d71653'], ['../D71653'], 'D71653', null]) {
    assert.throws(() => resolveCandidateMapScope(changed(value => { value.nativeResourceOnlyMaps = rooms; })), /canonical named/);
  }
  assert.throws(() => resolveCandidateMapScope(changed(value => { value.mapWhitelist.push({ fileName: 'D71653' }); })), /must not be runtime-admitted/);
});

test('original no-entry proof rejects new inbound moves, scripts, spawns and changed original geometry', () => {
  const mutations = [
    copy => { copy.respawnManifest.maps.find(map => map.map_file_name === '0').movements.push({ map_index: 235 }); },
    copy => { copy.npcScriptManifest = { ...copy.npcScriptManifest, fixture: 'MAPMOVE D71653 17 12' }; },
    copy => { copy.mapEventManifest = { fixture: 'MAPMOVE D71653 17 12' }; },
    copy => { copy.respawnManifest.maps.find(map => map.map_file_name === 'D71653').movements[0].destination.x = 35; },
    copy => { copy.respawnManifest.maps.find(map => map.map_file_name === 'D71653').respawn_count = 1; },
    copy => { copy.respawnManifest.maps.find(map => map.map_file_name === 'D71653').map_index = 234; },
  ];
  for (const mutate of mutations) {
    const copy = structuredClone(sources); mutate(copy);
    assert.throws(() => proveCandidateResourceOnlyRooms(profile, copy), /original no-entry source proof/);
  }
  assert.equal(proveCandidateResourceOnlyRooms(profile, sources).length, 1);
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
  assert.equal(rejected.scope.expectedMaps.length, 209);
  assert.equal(fs.existsSync(completion), false);
});
