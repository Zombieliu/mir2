import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const compiled = ts.transpileModule(readFileSync(new URL('../lib/spectator-endpoint.ts', import.meta.url), 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText;
const module = { exports: {} };
new Function('exports', 'module', compiled)(module.exports, module);
const { spectatorWebSocketUrl, gatewayHttpBase } = module.exports;

test('dedicated spectator realm never changes the player URL', () => {
  const player = 'wss://game.example/ws';
  const watch = spectatorWebSocketUrl(player, 'wss://game.example/playtest/spectator/ws',
    'https://watch.example/spectate?spectateMap=0&spectateTarget=father&gatewayWs=wss://attacker.example/ws');
  assert.equal(watch, 'wss://game.example/playtest/spectator/ws?map=0&target=father');
  assert.equal(player, 'wss://game.example/ws');
});

test('derived endpoints retain the entire realm prefix and discard player parameters', () => {
  assert.equal(spectatorWebSocketUrl('wss://game.example/playtest/ws?resume=private', undefined,
    'https://watch.example/?spectateDelayMs=30000'),
    'wss://game.example/playtest/spectator/ws?delayMs=30000');
  assert.equal(gatewayHttpBase('wss://game.example/playtest/ws?resume=private'), 'https://game.example/playtest');
  assert.equal(gatewayHttpBase('wss://game.example/playtest/spectator/ws'), 'https://game.example/playtest');
  assert.equal(gatewayHttpBase('ws://127.0.0.1:7210/ws'), 'http://127.0.0.1:7210');
});

test('misconfigured spectator endpoints fail closed instead of falling back to a different realm', () => {
  for (const invalid of ['https://game.example/spectator/ws', 'wss://game.example/ws',
    'wss://user:secret@game.example/spectator/ws', 'wss://game.example/spectator/ws?token=secret']) {
    assert.throws(() => spectatorWebSocketUrl('wss://game.example/ws', invalid, 'https://watch.example/'));
  }
  assert.throws(() => gatewayHttpBase('wss://game.example/unsupported'));
});

test('spectator control parameters are encoded and no unrelated query data is forwarded', () => {
  const url = new URL(spectatorWebSocketUrl('wss://game.example/ws', undefined,
    'https://watch.example/?spectateTarget=A%26B&spectateMode=director&spectateToken=test-token&password=never-forward'));
  assert.equal(url.searchParams.get('target'), 'A&B');
  assert.equal(url.searchParams.get('mode'), 'director');
  assert.equal(url.searchParams.get('token'), 'test-token');
  assert.equal(url.searchParams.has('password'), false);
});

const liveCompiled = ts.transpileModule(readFileSync(new URL('../lib/spectator-live-state.ts', import.meta.url), 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText;
const liveModule = { exports: {} };
new Function('exports', 'module', liveCompiled)(liveModule.exports, liveModule);
const { spectatorLiveState } = liveModule.exports;
const clock = 1_000_000;
const status = { map: '0', delayMs: 30_000, capturedAtMs: clock - 30_000,
  matches: [{ mapFileName: '0', latestCapturedAtMs: clock - 1_000 }], replay: { active: false } };
test('normal public delay stays live and stopped sources become visibly stale', () => {
  assert.equal(spectatorLiveState(status, clock), 'live');
  assert.equal(spectatorLiveState(status, clock + 20_000), 'stale');
  assert.equal(spectatorLiveState({ ...status, matches: [] }, clock), 'live');
  assert.equal(spectatorLiveState({ ...status, matches: [] }, clock + 20_000), 'stale');
});
test('empty buffers and historical replays are not labelled current live gameplay', () => {
  assert.equal(spectatorLiveState(null, clock), 'waiting');
  assert.equal(spectatorLiveState({ ...status, capturedAtMs: null, matches: [] }, clock), 'waiting');
  assert.equal(spectatorLiveState({ ...status, capturedAtMs: null }, clock), 'buffering');
  assert.equal(spectatorLiveState({ ...status, capturedAtMs: null }, clock + 20_000), 'buffering');
  assert.equal(spectatorLiveState({ ...status, capturedAtMs: null }, clock + 45_000), 'stale');
  assert.equal(spectatorLiveState({ ...status, capturedAtMs: 1, matches: [], replay: { active: true } }, clock), 'replay');
});

test('resuming a map does not label its old delivered cache as current gameplay', () => {
  assert.equal(spectatorLiveState({ ...status, capturedAtMs: clock - 180_000 }, clock), 'buffering');
  assert.equal(spectatorLiveState({ ...status, capturedAtMs: clock - 180_000 }, clock + 20_000), 'stale');
});
