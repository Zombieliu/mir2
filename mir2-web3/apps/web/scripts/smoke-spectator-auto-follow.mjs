// Synthetic browser camera acceptance. This never simulates a passing live
// network/Boss/PK test and never logs into or changes a player account.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const templatePath = process.env.MIR2_SPECTATOR_FIXTURE_JSON;
assert.ok(templatePath, 'An explicitly captured redacted public map template is required');
const template = JSON.parse(await fs.readFile(templatePath, 'utf8'));
for (const key of ['inventoryItems', 'storageItems', 'questLog', 'knownSkills', 'activeBuffs']) {
  assert.deepEqual(template.world?.[key], [], `Public template must redact ${key}`);
}
assert.equal(template.status?.readOnly, true);
assert.equal(template.status?.directorAuthorized, false);
const url = process.env.MIR2_SPECTATOR_UI_URL
  || 'http://127.0.0.1:3211/spectate?spectateMap=0&bevyRuntime=0';
const output = path.resolve(process.env.MIR2_SPECTATOR_UI_OUTPUT || 'artifacts/spectator-readiness/auto-follow/browser-fixture');
const chrome = process.env.MIR2_CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const port = 10_010 + process.pid % 150;

function installFixture(source) {
  const NativeWs = window.WebSocket;
  const self = source.world.entities.find((entity) => entity.objectId === source.world.playerObjectId);
  if (!self?.sprite) throw new Error('Public template has no visible player sprite');
  let socket, sequence = 0, capturedAt, lastWorld, lastStatus, selected = null, connections = 0;
  const sent = [];
  function emit(elapsed = 0, hp = 100, dead = false, events = []) {
    const first = { ...self, objectId: 710001, name: 'IdleFixture', kind: 'selfPlayer', hp: 100, maxHp: 100, dead: false };
    const second = { ...self, objectId: 710002, name: 'CombatFixture', kind: 'player',
      x: self.x + 10, hp, maxHp: 100, dead };
    lastWorld = { ...source.world, tick: ++sequence, playerObjectId: first.objectId,
      playerHp: 100, playerMaxHp: 100,
      entities: [first, second, ...source.world.entities.filter((entity) => !['player', 'selfPlayer'].includes(entity.kind))] };
    lastStatus = { ...source.status, recordingId: 'synthetic-camera-' + connections,
      sequence, capturedAtMs: capturedAt + elapsed, map: '0', target: selected,
      director: false, directorAuthorized: false, camera: null, delayMs: 3000, events,
      targets: [first, second].map(({ objectId, name, hp, maxHp, x, y }) => ({ objectId, name, hp, maxHp, x, y })),
      matches: [{ mapFileName: '0', mapTitle: 'BichonProvince', latestCapturedAtMs: Date.now(), playerCount: 2 }],
      replay: { active: false, playing: false, speed: 1 } };
    socket.dispatchEvent(new MessageEvent('message', { data: JSON.stringify({ type: 'worldSnapshot', payload: lastWorld }) }));
    socket.dispatchEvent(new MessageEvent('message', { data: JSON.stringify({ type: 'spectatorStatus', payload: lastStatus }) }));
  }
  class FixtureWs extends EventTarget {
    static CONNECTING = 0;
    static OPEN = 1;
    static CLOSING = 2;
    static CLOSED = 3;
    readyState = 0;
    constructor(endpoint, protocols) {
      super();
      if (!new URL(endpoint).pathname.endsWith('/spectator/ws')) return new NativeWs(endpoint, protocols);
      socket = this;
      connections++;
      sequence = 0;
      selected = null;
      capturedAt = Date.now() - 3000;
      setTimeout(() => {
        this.readyState = 1;
        this.dispatchEvent(new Event('open'));
        emit();
      }, 20);
    }
    send(value) {
      const command = JSON.parse(value);
      sent.push(command);
      if (command.type === 'follow') {
        selected = command.target;
        lastStatus = { ...lastStatus, target: selected };
        // A genuine control ACK has metadata alone, without another world.
        this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify({ type: 'spectatorStatus', payload: lastStatus }) }));
      }
    }
    close() { this.readyState = 3; this.dispatchEvent(new Event('close')); }
  }
  window.WebSocket = FixtureWs;
  window.__watchFixture = { sent, emit, get at() { return capturedAt; },
    close: () => socket.close(), get connections() { return connections; } };
}

async function until(check, timeout = 45_000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (await check()) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error('Synthetic spectator browser acceptance timed out');
}

await fs.mkdir(output, { recursive: true });
const profile = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-auto-camera-'));
const browser = spawn(chrome, ['--headless=new', `--remote-debugging-port=${port}`,
  `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', 'about:blank'],
  { stdio: 'ignore', windowsHide: true });
let socket, nextId = 1;
const pending = new Map(), errors = [], resourceFailures = [], checks = [];
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const id = nextId++;
  pending.set(id, { resolve, reject });
  socket.send(JSON.stringify({ id, method, params }));
});
async function evaluate(expression) {
  const result = await send('Runtime.evaluate', { expression, returnByValue: true });
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
  return result.result?.value;
}
const state = () => evaluate(`({ id: window.__mir2Stage5?.state.playerObjectId,
  caption: document.querySelector('[data-testid=spectator-auto-follow]')?.textContent,
  mode: document.querySelector('[data-testid=spectator-auto-follow]')?.dataset.mode,
  scene: window.__mir2SceneGate,
  commands: window.__watchFixture.sent.map((entry) => entry.type) })`);
async function expectPlayer(id, phase, mode = 'automatic') {
  await until(async () => {
    const current = await state();
    return current.id === String(id) && current.mode === mode;
  });
  const current = await state();
  checks.push({ phase, renderPlayerId: current.id, mode: current.mode, caption: current.caption });
}
async function choose(value) {
  await evaluate(`(() => { const select = document.querySelector('[data-testid=spectator-target]');
    select.value = ${JSON.stringify(value)}; select.dispatchEvent(new Event('change', { bubbles: true })); })()`);
}
async function emit(elapsed, from, to, dead = false) {
  await evaluate(`window.__watchFixture.emit(${elapsed}, ${to}, ${dead}, [{ kind: 'health',
    objectId: 710002, atMs: window.__watchFixture.at + ${elapsed}, payload: { from: ${from}, to: ${to} } }])`);
}
async function screenshot(name) {
  const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
  await fs.writeFile(path.join(output, name), Buffer.from(shot.data, 'base64'));
}

try {
  await until(async () => fetch(`http://127.0.0.1:${port}/json/version`).then((response) => response.ok, () => false));
  const target = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json();
  socket = new WebSocket(target.webSocketDebuggerUrl);
  socket.addEventListener('message', ({ data }) => {
    const message = JSON.parse(String(data));
    if (message.id && pending.has(message.id)) {
      const promise = pending.get(message.id); pending.delete(message.id);
      if (message.error) promise.reject(new Error(message.error.message)); else promise.resolve(message.result || {});
    }
    if (message.method === 'Runtime.exceptionThrown') errors.push(message.params.exceptionDetails.text);
    if (message.method === 'Network.responseReceived' && message.params.response.status >= 400) {
      resourceFailures.push({ url: message.params.response.url, status: message.params.response.status });
    }
  });
  await new Promise((resolve, reject) => { socket.addEventListener('open', resolve, { once: true }); socket.addEventListener('error', reject, { once: true }); });
  for (const domain of ['Runtime', 'Page', 'Network']) await send(domain + '.enable');
  await send('Page.addScriptToEvaluateOnNewDocument', { source: `(${installFixture.toString()})(${JSON.stringify(template)});` });
  await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await send('Page.navigate', { url });
  await until(async () => await evaluate("!!document.querySelector('[data-testid=spectator-controls-toggle]')"));
  assert.equal(await evaluate("!!document.querySelector('[data-testid=spectator-overlay]')"), false);
  await evaluate("document.querySelector('[data-testid=spectator-controls-toggle]').click()");
  await expectPlayer(710001, 'initial stable idle');
  await until(async () => {
    const gate = (await state()).scene;
    return gate?.sceneInteractionReady && gate.hasRenderPlayer && gate.hasMapRegion;
  }, 90_000);
  await emit(1_000, 100, 20);
  await expectPlayer(710001, 'hold despite challenger damage');
  await emit(8_000, 20, 10);
  await expectPlayer(710002, 'switch after eight seconds');
  await screenshot('automatic-combat.png');
  await choose('IdleFixture');
  await expectPlayer(710001, 'same-frame manual ACK', 'manual');
  await emit(8_100, 10, 5);
  await expectPlayer(710001, 'manual priority over continued damage', 'manual');
  await choose('');
  await expectPlayer(710001, 'return to automatic starts hold');
  await emit(17_000, 5, 80);
  await emit(17_100, 80, 20);
  await expectPlayer(710002, 'stronger combat replaces held idle');
  await emit(17_200, 20, 0, true);
  await expectPlayer(710001, 'dead target replaced immediately');
  for (const key of ['ArrowRight', 'w', '1']) {
    await send('Input.dispatchKeyEvent', { type: 'keyDown', key });
    await send('Input.dispatchKeyEvent', { type: 'keyUp', key });
  }
  assert.deepEqual((await state()).commands, ['follow', 'follow']);
  await evaluate('window.__watchFixture.close()');
  await until(async () => await evaluate('window.__watchFixture.connections >= 2'), 5_000);
  await expectPlayer(710001, 'reconnect clears history');
  const captureUrl = new URL(url); captureUrl.searchParams.set('capture', '1');
  await send('Page.navigate', { url: captureUrl.toString() });
  await until(async () => (await state()).id === '710001');
  await until(async () => {
    const gate = (await state()).scene;
    return gate?.sceneInteractionReady && gate.hasRenderPlayer && gate.hasMapRegion;
  }, 90_000);
  assert.equal(await evaluate("!!document.querySelector('[data-testid=spectator-controls-toggle]')"), false);
  assert.equal(await evaluate("!!document.querySelector('[data-testid=spectator-overlay]')"), false);
  assert.deepEqual((await state()).commands, []);
  await screenshot('capture-clear.png');
  assert.deepEqual(errors, []);
  const report = { schema: 'mir2.spectator-auto-follow-browser-fixture.v1', generatedAt: new Date().toISOString(),
    scope: 'synthetic delayed-frame camera behavior using a redacted public map/sprite template',
    actualLiveNetworkAcceptance: false, bossPkAcceptance: false, platformPublishing: 'not-run',
    checks, captureModeClear: true, playerCommandsSent: [], defaultCollapsed: true,
    resourceFailures, errors, screenshots: ['automatic-combat.png', 'capture-clear.png'] };
  await fs.writeFile(path.join(output, 'browser-fixture.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ passed: true, phases: checks.length, scope: report.scope, output }));
} catch (error) {
  if (socket?.readyState === WebSocket.OPEN) {
    await screenshot('failed.png');
    await fs.writeFile(path.join(output, 'failed.json'), JSON.stringify({ error: error.message, checks,
      state: await state(), errors, resourceFailures, scope: 'synthetic fixture, not live network acceptance' }, null, 2));
  }
  console.error(error.message); process.exitCode = 1;
} finally {
  socket?.close(); browser.kill();
  // Preserve this task's isolated profile; never operate on the user's browser.
}
