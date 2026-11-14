import test from 'node:test';
import assert from 'node:assert/strict';
import { preparationStagingCatalogue, selectPreparationStagingPoint, PREPARATION_STAGING_CANDIDATE_POLICY } from './capacity-ordinary-staging.mjs';
import { selectPreparationStagingPoint as originalApi } from './playtest-capacity-prepare.mjs';

const catalogue = () => Array.from({ length: 335 }, (_, i) => ({ x: 278 + i % 21, y: 606 + Math.floor(i / 21) }));
const declaration = () => ({ candidatePolicy: PREPARATION_STAGING_CANDIDATE_POLICY, stagingCandidates: catalogue() });
const map = () => ({ mapFileName: '0', width: 640, height: 640, blocked: new Uint8Array(640 * 640) });

test('ordinary staging extraction preserves the preparer public selector identity', () => {
  assert.equal(originalApi, selectPreparationStagingPoint);
});

test('ordinary staging catalogue is explicit, complete, distinct and immutable while no-policy legacy remains null', () => {
  assert.equal(preparationStagingCatalogue({}, []), null);
  const p = declaration(), validated = preparationStagingCatalogue(p, [{ x: 287, y: 614 }], map());
  assert.equal(validated.length, 335); assert.ok(Object.isFrozen(validated)); assert.ok(Object.isFrozen(validated[0]));
  p.stagingCandidates[0].x = 1; assert.equal(validated[0].x, 278);
  for (const bad of [
    { stagingCandidates: catalogue() }, { candidatePolicy: PREPARATION_STAGING_CANDIDATE_POLICY },
    { ...declaration(), candidatePolicy: 'different' }, { ...declaration(), stagingCandidates: catalogue().slice(1) },
  ]) assert.throws(() => preparationStagingCatalogue(bad, []), /frozen335/);
  const duplicate = declaration(); duplicate.stagingCandidates[334] = { ...duplicate.stagingCandidates[0] };
  assert.throws(() => preparationStagingCatalogue(duplicate, []), /distinct/);
  assert.throws(() => preparationStagingCatalogue(declaration(), [{ x: 1, y: 1 }]), /contained/);
});

test('ordinary staging validates current map, collision, transfers and map0 catalogue coordinates', () => {
  const p = declaration(), m = map();
  m.blocked[606 * 640 + 278] = 1;
  assert.throws(() => preparationStagingCatalogue(p, [], m), /walkable current map0/);
  m.blocked.fill(0); m.mapFileName = '2';
  assert.throws(() => preparationStagingCatalogue(p, [], m), /current map0/);
  m.mapFileName = '0';
  assert.throws(() => preparationStagingCatalogue(p, [], m, [{ bounds: { minX: 278, maxX: 278, minY: 606, maxY: 606 } }]), /transfer/);
  for (const point of [{ x: 1.5, y: 606 }, { x: -1, y: 606 }, { x: 65536, y: 606 }, { x: 278, y: 606, mapFileName: '2' }]) {
    const bad = declaration(); bad.stagingCandidates[0] = point;
    assert.throws(() => preparationStagingCatalogue(bad, []), /map0/);
  }
});

test('ordinary staging uses actual cold-login coordinates and skips a live Deer on the declared saved point', () => {
  const owner = { objectId: 1000, name: 'ordinary-owner', kind: 'player', x: 286, y: 613 };
  const snapshot = { playerObjectId: 1000, mapFileName: '0', entities: [owner,
    { objectId: 206515, kind: 'monster', name: 'Deer', x: 287, y: 614, hp: 25, dead: false }] };
  const receipt = { ownerId: 1000, ownerName: owner.name, map: '0', x: 286, y: 613, sequence: 689, monotonicMs: 715018.65 };
  const candidates = [{ x: 287, y: 614 }, { x: 286, y: 613 }, { x: 288, y: 613 }], savedPoints = new Set();
  const args = { snapshot, receipt, defaultPoint: candidates[0], candidates, savedPoints };
  assert.deepEqual(selectPreparationStagingPoint(args), { x: 286, y: 613 });
  assert.equal(savedPoints.size, 0, 'query alone cannot reserve an assignment');
  savedPoints.add('286,613'); assert.deepEqual(selectPreparationStagingPoint(args), { x: 288, y: 613 });
  for (const changed of [{ ...receipt, x: 287 }, { ...receipt, ownerName: 'other' }, { ...receipt, map: '2' }, { ...receipt, sequence: 0 }]) {
    assert.throws(() => selectPreparationStagingPoint({ ...args, receipt: changed }), /Fresh same-owner map0/);
  }
  snapshot.mapFileName = '2'; assert.throws(() => selectPreparationStagingPoint(args), /same-owner map0/);
});
