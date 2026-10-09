import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const compiled = ts.transpileModule(readFileSync(new URL('../lib/spectator-auto-follow.ts', import.meta.url), 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText;
const module = { exports: {} };
new Function('exports', 'module', compiled)(module.exports, module);
const { SpectatorAutoFollow, SpectatorFollowStream } = module.exports;
const start = 1_000_000;
const player = (id, patch = {}) => ({ objectId: id, name: `Player${id}`, kind: 'player',
  x: id === 10 ? 10 : 50, y: 10, hp: 100, maxHp: 100, dead: false, ...patch });
const world = (entities = [player(10, { kind: 'selfPlayer' }), player(20)], patch = {}) => ({
  mapFileName: '0', playerObjectId: 10, playerHp: 100, playerMaxHp: 100, entities, ...patch,
});
const frame = (elapsed = 0, patch = {}) => ({ readOnly: true, map: '0', recordingId: 'owned-fixture',
  sequence: elapsed + 1, capturedAtMs: start + elapsed, target: null, director: false, camera: null,
  events: [], ...patch });
const hurt = (id, elapsed, from = 100, to = 20, patch = {}) => ({ kind: 'health',
  objectId: id, atMs: start + elapsed, payload: { from, to }, ...patch });
const choose = (selector, elapsed = 0, events = [], entities, patch = {}) => selector.direct(
  world(entities), frame(elapsed, { events, ...patch }));

test('recent damage wins over an idle player with persistently low HP', () => {
  const picked = choose(new SpectatorAutoFollow(), 0, [hurt(20, 0, 100, 80)],
    [player(10, { hp: 1 }), player(20, { hp: 80 })]);
  assert.equal(picked.decision.objectId, 20);
  assert.equal(picked.decision.reason, 'recentDamage');
  assert.equal(picked.world.playerHp, 80);
  assert.equal(picked.world.entities.filter((entry) => entry.kind === 'selfPlayer').length, 1);
});

test('a crowded but idle area does not outweigh actual combat elsewhere', () => {
  const monsters = Array.from({ length: 20 }, (_, index) => player(100 + index,
    { kind: 'monster', x: 10, y: 10 }));
  assert.equal(choose(new SpectatorAutoFollow(), 0, [hurt(20, 0)],
    [player(10), player(20), ...monsters]).decision.objectId, 20);
});

test('nearby monster HP loss is labelled nearby combat without claiming an attacker', () => {
  const monster = player(30, { kind: 'monster', x: 50, y: 11 });
  const picked = choose(new SpectatorAutoFollow(), 0, [hurt(30, 0)],
    [player(10), player(20), monster]);
  assert.equal(picked.decision.objectId, 20);
  assert.equal(picked.decision.reason, 'nearbyCombat');
  assert.equal('attackerId' in picked.decision, false);
  assert.equal('boss' in picked.decision, false);
});

test('merged AOI monster ordering cannot hide a candidate player damage event', () => {
  const monsters = Array.from({ length: 300 }, (_, index) => player(1000 + index,
    { kind: 'monster', x: 200, y: 200 }));
  assert.equal(choose(new SpectatorAutoFollow(), 0, [hurt(20, 0)],
    [...monsters, player(10), player(20)]).decision.objectId, 20);
});

test('healing, spawning, ordinary drops and movement alone are not combat', () => {
  const picked = choose(new SpectatorAutoFollow(), 0, [hurt(20, 0, 20, 100),
    { kind: 'spawn', objectId: 20, atMs: start, payload: {} },
    { kind: 'dropSpawn', objectId: 20, atMs: start, payload: {} },
    { kind: 'move', objectId: 20, atMs: start, payload: {} }]);
  assert.equal(picked.decision.objectId, 10);
  assert.equal(picked.decision.score, 0);
});

test('the eight-second hold uses capture time, then switches to a stronger scene', () => {
  const selector = new SpectatorAutoFollow();
  assert.equal(choose(selector).decision.objectId, 10);
  assert.equal(choose(selector, 1_000, [hurt(20, 1_000)]).decision.objectId, 10);
  assert.equal(choose(selector, 7_999).decision.reason, 'holding');
  assert.equal(choose(selector, 8_000).decision.objectId, 20);
});

test('a small score advantage does not cause a camera switch after the hold', () => {
  const selector = new SpectatorAutoFollow();
  choose(selector, 0, [hurt(10, 0, 100, 99)]);
  const picked = choose(selector, 9_000, [hurt(20, 9_000, 100, 99)]);
  assert.equal(picked.decision.objectId, 10);
  assert.equal(picked.decision.reason, 'holding');
});

test('equal idle scores keep the current player regardless of entity ordering', () => {
  const selector = new SpectatorAutoFollow();
  choose(selector, 0, [], [player(20)]);
  assert.equal(choose(selector, 10_000, [], [player(10), player(20)]).decision.objectId, 20);
});

test('manual follow takes priority and excludes a same-name NPC', () => {
  const selector = new SpectatorAutoFollow();
  const entities = [player(99, { kind: 'npc', name: 'Player20' }), player(10), player(20)];
  const picked = choose(selector, 0, [hurt(10, 0)], entities, { target: ' player20 ' });
  assert.equal(picked.decision.objectId, 20);
  assert.equal(picked.decision.mode, 'manual');
  assert.equal(picked.world.entities.find((entry) => entry.objectId === 99).kind, 'npc');
});

test('returning from manual to automatic starts a new hold without losing recent activity', () => {
  const selector = new SpectatorAutoFollow();
  choose(selector, 0, [], undefined, { target: 'Player10' });
  choose(selector, 8_000, [hurt(20, 8_000)], undefined, { target: 'Player10' });
  const returned = choose(selector, 9_000);
  assert.equal(returned.decision.objectId, 10);
  assert.equal(returned.decision.mode, 'automatic');
  assert.equal(returned.decision.heldForMs, 0);
});

test('a dead or missing target is replaced immediately instead of waiting for the hold', () => {
  for (const entities of [[player(10, { dead: true, hp: 0 }), player(20)], [player(20)]]) {
    const selector = new SpectatorAutoFollow();
    choose(selector);
    assert.equal(choose(selector, 1_000, [], entities).decision.objectId, 20);
  }
});

test('an unavailable manual target falls back, then resumes when that player returns', () => {
  const selector = new SpectatorAutoFollow();
  const fallback = choose(selector, 0, [], [player(10)], { target: 'Player20' });
  assert.equal(fallback.decision.objectId, 10);
  assert.equal(fallback.decision.mode, 'manualUnavailable');
  const restored = choose(selector, 1_000, [], undefined, { target: 'Player20' });
  assert.equal(restored.decision.objectId, 20);
  assert.equal(restored.decision.mode, 'manual');
});

test('dead entities, zero HP players and reserved camera IDs are never auto candidates', () => {
  const entities = [player(1, { dead: true }), player(2, { hp: 0 }),
    player(0xffffffff), player(0), player(10), player(3, { kind: 'monster' })];
  assert.equal(choose(new SpectatorAutoFollow(), 0, [], entities).decision.objectId, 10);
});

test('an empty or entirely dead view clears the render self player', () => {
  const picked = choose(new SpectatorAutoFollow(), 0, [], [player(10, { kind: 'selfPlayer', dead: true })]);
  assert.equal(picked.decision.objectId, null);
  assert.equal(picked.world.playerObjectId, null);
  assert.equal(picked.world.entities.some((entry) => entry.kind === 'selfPlayer'), false);
});

test('future and expired events cannot affect the delayed frame', () => {
  for (const event of [hurt(20, 1), hurt(20, -4_001)]) {
    assert.equal(choose(new SpectatorAutoFollow(), 0, [event]).decision.objectId, 10);
  }
});

test('repeated or duplicate statuses cannot multiply a frame score', () => {
  const selector = new SpectatorAutoFollow();
  const first = choose(selector, 0, [hurt(20, 0)]);
  for (let i = 0; i < 20; i++) {
    assert.equal(choose(selector, 0, [hurt(20, 0)]).decision.score, first.decision.score);
  }
});

test('recording, map, backwards replay time and long gaps clear prior heat', () => {
  const cases = [{ elapsed: 1_000, patch: { recordingId: 'another' } },
    { elapsed: -1_000, patch: {} }, { elapsed: 16_000, patch: {} }];
  for (const entry of cases) {
    const selector = new SpectatorAutoFollow();
    choose(selector, 0, [hurt(20, 0)]);
    assert.equal(choose(selector, entry.elapsed, [], undefined, entry.patch).decision.objectId, 10);
  }
  const selector = new SpectatorAutoFollow();
  choose(selector, 0, [hurt(20, 0)]);
  const switched = selector.direct(world(undefined, { mapFileName: 'D001' }), frame(1_000, { map: 'D001' }));
  assert.equal(switched.decision.objectId, 10);
});

test('heat decays so new combat can replace an old scene', () => {
  const selector = new SpectatorAutoFollow();
  choose(selector, 0, [hurt(20, 0)]);
  const before = choose(selector, 6_000).decision.score;
  assert.equal(before, 55);
  assert.equal(choose(selector, 12_000, [hurt(10, 12_000)]).decision.objectId, 10);
});

test('privileged server camera remains unchanged and the selector sends no commands', () => {
  const source = world();
  const picked = new SpectatorAutoFollow().direct(source, frame(0, { director: true }));
  assert.equal(picked.world, source);
  assert.equal(picked.decision.mode, 'serverCamera');
});

test('the stream waits for paired metadata before rendering, preserving private arrays', () => {
  const stream = new SpectatorFollowStream();
  const source = world(undefined, { inventoryItems: [], stage5Systems: {}, groundDrops: [] });
  assert.equal(stream.receiveStatus(frame()), null);
  stream.receiveWorld(source);
  const result = stream.receiveStatus(frame());
  assert.equal(result.world.inventoryItems, source.inventoryItems);
  assert.equal(result.world.stage5Systems, source.stage5Systems);
  assert.equal(result.world.groundDrops, source.groundDrops);
});

test('unpaired newer control status cannot score against an old world', () => {
  const stream = new SpectatorFollowStream();
  stream.receiveWorld(world());
  stream.receiveStatus(frame());
  assert.equal(stream.receiveStatus(frame(1_000, { events: [hurt(20, 1_000)] })), null);
  stream.receiveWorld(world());
  assert.equal(stream.receiveStatus(frame(1_000, { events: [hurt(20, 1_000)] })).decision.objectId, 10);
});

test('same-frame manual acknowledgements change only the camera, never rescore events', () => {
  const stream = new SpectatorFollowStream();
  stream.receiveWorld(world());
  const original = stream.receiveStatus(frame(0, { events: [hurt(20, 0)] }));
  const manual = stream.receiveStatus(frame(0, { events: [hurt(20, 0)], target: 'Player10' }));
  assert.equal(manual.decision.objectId, 10);
  const automatic = stream.receiveStatus(frame(0, { events: [hurt(20, 0)], target: null }));
  assert.equal(automatic.decision.objectId, 10);
  assert.ok(original.decision.score > automatic.decision.score);
});

test('wrong-map and invalid metadata frames fail closed; reset discards pending/cached frames', () => {
  const stream = new SpectatorFollowStream();
  stream.receiveWorld(world());
  assert.equal(stream.receiveStatus(frame(0, { map: 'D001' })), null);
  stream.receiveWorld(world());
  assert.equal(stream.receiveStatus(frame(0, { capturedAtMs: null })), null);
  stream.receiveWorld(world());
  stream.reset();
  assert.equal(stream.receiveStatus(frame()), null);
});

test('invalid or unrelated standalone status invalidates an already paired cache', () => {
  for (const patch of [{ readOnly: false }, { map: 'D001' }, { recordingId: 'changed' }, { sequence: 2 }]) {
    const stream = new SpectatorFollowStream();
    stream.receiveWorld(world());
    assert.ok(stream.receiveStatus(frame()));
    assert.equal(stream.receiveStatus(frame(0, patch)), null);
    assert.equal(stream.receiveStatus(frame(0, { target: 'Player20' })), null);
  }
});
