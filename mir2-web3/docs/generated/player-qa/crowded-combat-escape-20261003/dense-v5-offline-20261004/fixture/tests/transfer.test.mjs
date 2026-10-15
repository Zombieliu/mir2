import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { selectOrdinaryTransfer, assertOrdinaryTransferCurrent } from '../ordinary_transfer.mjs';
import { loadProtocolCollisionMap, findProtocolWalkPath } from '../vendor/protocol-navigation.mjs';

const root = path.resolve(import.meta.dirname, '..');
const point = (x, y) => ({ x, y });
function canonical(x, y, dx = 2, dy = 2) {
  return { map_index: 29, source: point(x, y), destination: point(dx, dy), need_hole: false,
    need_move: false, conquest_index: 0, show_on_big_map: false, icon: 0, targetMap: '0132' };
}
function live(t, sourceMap = '0') {
  return { key: `crystal-move:${sourceMap}:${t.source.x}:${t.source.y}:${t.map_index}:${t.destination.x}:${t.destination.y}`,
    mapFileName: sourceMap, toMapFileName: t.targetMap, toPosition: { ...t.destination },
    bounds: { minX: t.source.x, maxX: t.source.x, minY: t.source.y, maxY: t.source.y } };
}
function fixture() {
  const a = canonical(4, 3), b = canonical(7, 7), transfers = [a, b];
  const map = { mapFileName: '0', width: 12, height: 12, blocked: new Uint8Array(144), doors: [] };
  // The nearest legal entrance is enclosed; only its own entrance cell may be
  // treated as a legitimate transfer. Its surrounding wall remains blocked.
  for (let x = 3; x <= 5; x++) for (let y = 2; y <= 4; y++) map.blocked[y * 12 + x] = 1;
  map.blocked[7 * 12 + 7] = 1;
  const metadata = { maps: [{ map: '0', transfers }] };
  const snapshot = { mapFileName: '0', playerObjectId: 1000, playerHp: 100, mapSnapshotPending: false,
    entities: [{ objectId: 1000, kind: 'player', x: 1, y: 3, dead: false }], mapTransfers: transfers.map(t => live(t)) };
  return { a, b, map, metadata, snapshot };
}
function choose(f) { return selectOrdinaryTransfer({ ...f, targetMap: '0132' }); }

test('nearest unreachable known entrance retains walls and selects a second legal reachable entrance', () => {
  const f = fixture(), result = choose(f);
  assert.deepEqual(result.selection.source, f.b.source);
  assert.ok(result.route.length >= 2);
  assert.equal(f.map.blocked[2 * 12 + 3], 1);
  assert.equal(f.map.blocked[7 * 12 + 7], 1);
  assert.equal(result.attempts[0].reason, 'no-legal-static-occupancy-path');
});
test('all known entrances unreachable remains blocked without fabricated routes', () => {
  const f = fixture();
  for (let x = 6; x <= 8; x++) for (let y = 6; y <= 8; y++) f.map.blocked[y * 12 + x] = 1;
  const r = choose(f); assert.equal(r.selection, null); assert.equal(r.route, null);
  assert.equal(r.attempts.length, 2); assert.ok(r.attempts.every(x => x.reason === 'no-legal-static-occupancy-path'));
});
test('forged candidate source or destination is rejected on continuation even with a forged live row', () => {
  const f = fixture(), selection = choose(f).selection;
  for (const forged of [{ ...selection, source: point(9, 9) }, { ...selection, destination: point(9, 9) }]) {
    f.snapshot.mapTransfers.push(live(forged));
    assert.throws(() => assertOrdinaryTransferCurrent(f.snapshot, f.metadata, forged, f.map), /source-verified/);
  }
});
test('wrong current map or collision map cannot reuse transfer override', () => {
  const f = fixture(), selection = choose(f).selection;
  assert.throws(() => assertOrdinaryTransferCurrent({ ...f.snapshot, mapFileName: '1' }, f.metadata, selection, f.map), /source-verified/);
  assert.throws(() => assertOrdinaryTransferCurrent(f.snapshot, f.metadata, selection, { ...f.map, mapFileName: '1' }), /source-verified/);
});
test('NeedMove and NeedHole source records are rejected even when advertised live', () => {
  for (const flag of ['need_move', 'need_hole']) {
    const f = fixture(); f.metadata.maps[0].transfers.forEach(t => t[flag] = true);
    assert.equal(choose(f).selection, null);
  }
});
test('missing or stale live candidate does not authorize a source entrance override', () => {
  const f = fixture(), selection = choose(f).selection;
  f.snapshot.mapTransfers = [];
  assert.equal(choose(f).selection, null);
  assert.throws(() => assertOrdinaryTransferCurrent(f.snapshot, f.metadata, selection, f.map), /source-verified/);
});
test('pending map snapshot and changed owner invalidate a previously selected entrance', () => {
  const f = fixture(), selection = choose(f).selection;
  assert.throws(() => assertOrdinaryTransferCurrent({ ...f.snapshot, mapSnapshotPending: true }, f.metadata, selection, f.map), /source-verified/);
  const other = { ...f.snapshot, playerObjectId: 2000, entities: [{ ...f.snapshot.entities[0], objectId: 2000 }] };
  assert.throws(() => assertOrdinaryTransferCurrent(other, f.metadata, selection, f.map), /source-verified/);
});
test('wrong live source map, key, target, bounds or destination rejects canonical candidate', () => {
  const mutations = [t => ({ ...t, mapFileName: '1' }), t => ({ ...t, key: 'forged' }),
    t => ({ ...t, toMapFileName: '1' }), t => ({ ...t, bounds: { minX: 0, maxX: 11, minY: 0, maxY: 11 } }),
    t => ({ ...t, toPosition: point(11, 11) })];
  for (const change of mutations) {
    const f = fixture(); f.snapshot.mapTransfers = f.snapshot.mapTransfers.map(change);
    assert.equal(choose(f).selection, null);
  }
});
test('live actor occupying the reachable entrance or sole approach cannot be walked through', () => {
  const f = fixture(); f.snapshot.entities.push({ objectId: 2001, kind: 'monster', x: 7, y: 7, dead: false });
  assert.equal(choose(f).selection, null);
  const g = fixture(); g.map.blocked.fill(1);
  for (const p of [point(1, 3), point(1, 4), point(1, 5), point(1, 6), point(1, 7), point(2, 7), point(3, 7), point(4, 7), point(5, 7), point(6, 7)]) g.map.blocked[p.y * 12 + p.x] = 0;
  g.snapshot.entities.push({ objectId: 2001, kind: 'monster', x: 4, y: 7, dead: false });
  assert.equal(choose(g).selection, null);
});
test('original map0 book doorway reproduces nearest single-entrance failure and finds legal alternate', async () => {
  const metadata = JSON.parse(await fs.readFile(path.join(root, 'WORLD-INPUTS.json'), 'utf8'));
  const map = await loadProtocolCollisionMap('0', { packagedMapRoot: path.join(root, 'maps') });
  const transfers = metadata.maps.find(m => m.map === '0').transfers.filter(t => t.targetMap === '0132');
  const snapshot = { mapFileName: '0', playerObjectId: 1000, playerHp: 128, mapSnapshotPending: false,
    entities: [{ objectId: 1000, kind: 'player', x: 288, y: 609, dead: false }], mapTransfers: transfers.map(t => live(t)) };
  const nearest = transfers.find(t => t.source.x === 282 && t.source.y === 635);
  assert.equal(findProtocolWalkPath({ map, start: snapshot.entities[0], target: nearest.source,
    staticWalkableOverrides: [nearest.source], maxExpanded: 120000 }), null);
  const result = selectOrdinaryTransfer({ snapshot, metadata, map, targetMap: '0132' });
  assert.deepEqual(result.selection.source, point(283, 636));
  assert.equal(result.route.length, 46);
  assert.deepEqual(result.attempts[0].source, nearest.source);
  assert.equal(result.attempts[0].reason, 'no-legal-static-occupancy-path');
});
