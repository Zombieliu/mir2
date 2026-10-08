// Ordinary owned QA account only. No borrowed player, debug command or director token.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const Ws = require('next/dist/compiled/ws');
const base = 'https://165.154.65.136.sslip.io/playtest';
const playerUrl = base.replace('https:', 'wss:') + '/ws';
const accountId = process.env.MIR2_WATCH_QA_ACCOUNT;
const password = process.env.MIR2_WATCH_QA_PASSWORD;
const output = path.resolve(process.env.MIR2_WATCH_QA_OUTPUT || 'artifacts/spectator-readiness/live-transport.json');
const holdMs = Number(process.env.MIR2_WATCH_QA_HOLD_MS || 180_000);
const publicDelayMs = Number(process.env.MIR2_WATCH_QA_DELAY_MS || 30_000);
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
assert.ok(accountId && password, 'An explicitly owned QA account and password are required');
assert.ok(Number.isInteger(publicDelayMs) && publicDelayMs >= 1_000 && publicDelayMs <= 30_000);
assert.ok(Number.isFinite(holdMs) && holdMs >= publicDelayMs + 5_000 && holdMs <= 600_000);

async function connect(url, origin) {
  const socket = new Ws(url, { origin, handshakeTimeout: 10_000 });
  const client = { socket, messages: [], sequence: 0, failure: null };
  socket.on('message', (data) => {
    try {
      const value = JSON.parse(String(data));
      client.messages.push({ sequence: ++client.sequence, value });
      if (client.messages.length > 500) client.messages.shift();
    } catch { client.failure = new Error('Invalid JSON from Gateway'); }
  });
  await new Promise((resolve, reject) => {
    socket.once('open', resolve);
    socket.once('error', (error) => reject(new Error('Owned QA WebSocket connection failed: '
      + (error.code || String(error.message).replaceAll(password, '[redacted]')))));
  });
  socket.on('error', () => { client.failure = new Error('Owned QA WebSocket transport failed'); });
  client.send = (command) => socket.send(JSON.stringify(command));
  client.wait = async (predicate, after = 0, timeout = 30_000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      if (client.failure) throw client.failure;
      const item = client.messages.find((entry) => entry.sequence > after && predicate(entry.value));
      if (item) return item;
      await delay(20);
    }
    throw new Error('Owned QA packet wait timed out');
  };
  client.request = async (command, packet, timeout = 30_000) => {
    const after = client.sequence;
    client.send(command);
    return client.wait((value) => value.packet === packet, after, timeout);
  };
  return client;
}

async function main() {
  let player, viewer, keepAlive, inGame = false;
  const report = { schema: 'mir2.playtest-spectator-transport.v1', generatedAt: new Date().toISOString(),
    endpoint: base, source: 'ordinary-owned-QA-shared-zone', publishing: 'not-run', checks: {} };
  try {
    for (const [resource, status] of [
      ['/spectator/matches', 200], ['/spectator/matches?map=D001', 403],
      ['/spectator/recordings', 404], ['/ai-live/control', 404], ['/admin', 404],
    ]) {
      const response = await fetch(base + resource, { signal: AbortSignal.timeout(10_000) });
      assert.equal(response.status, status, `Unexpected public route result for ${resource}`);
      if (status === 200) {
        const directory = await response.json();
        assert.equal(directory.source, 'gateway-spectator');
        assert.equal(directory.publicDelayMs, publicDelayMs);
      }
    }
    const health = await (await fetch(base + '/health', { signal: AbortSignal.timeout(10_000) })).json();
    assert.equal(health.ok, true);
    assert.equal(health.spectator.recordingEnabled, false);
    report.revision = health.revision;
    report.checks.publicRoutes = true;
    player = await connect(playerUrl, new URL(base).origin);
    await player.wait((message) => message.packet === 'Connected');
    await player.request({ type: 'clientVersion' }, 'ClientVersion');
    if (process.env.MIR2_WATCH_QA_CREATE === '1') {
      const result = await player.request({ type: 'newAccount', accountId, password, birthDateBinary: 0,
        userName: 'Spectator QA', secretQuestion: '', secretAnswer: '', emailAddress: '' }, 'NewAccount');
      assert.equal(result.value.payload?.result, 8, 'Ordinary QA registration rejected');
      const privateFile = process.env.MIR2_WATCH_QA_PRIVATE_FILE;
      if (privateFile) {
        const saved = JSON.parse(await fs.readFile(privateFile, 'utf8'));
        assert.equal(saved.accountId, accountId);
        saved.created = true;
        await fs.writeFile(privateFile, JSON.stringify(saved, null, 2));
      }
    }
    const login = await player.request({ type: 'login', accountId, password }, 'LoginSuccess');
    let characterIndex = login.value.payload?.characters?.[0]?.index;
    if (!Number.isInteger(characterIndex)) {
      const character = await player.request({ type: 'newCharacter', name: 'Watch' + accountId.slice(-6),
        gender: 'Male', class: 'Warrior' }, 'NewCharacterSuccess');
      characterIndex = character.value.payload?.character?.index;
    }
    assert.ok(Number.isInteger(characterIndex));
    const beforeStart = player.sequence;
    const streamStartedAtMs = Date.now();
    const start = await player.request({ type: 'startGame', characterIndex }, 'StartGame', 45_000);
    assert.equal(start.value.payload?.result, 4, 'Ordinary QA StartGame rejected');
    inGame = true;
    keepAlive = setInterval(() => {
      if (player.socket.readyState === Ws.OPEN) player.send({ type: 'keepAlive', time: Date.now() });
    }, 2_000);
    const entered = await player.wait((value) => value.type === 'worldSnapshot' && value.payload?.mapFileName,
      beforeStart, 45_000);
    assert.equal(String(entered.value.payload.mapFileName), '0', 'QA must enter public Bichon naturally');
    console.log(JSON.stringify({ playerReady: true, publicMap: '0', revision: report.revision }));
    viewer = await connect(base.replace('https:', 'wss:') + '/spectator/ws?map=0&delayMs=0&mode=director',
      'http://127.0.0.1:3211');
    const first = await viewer.wait((value) => value.type === 'spectatorStatus');
    assert.equal(first.value.payload.readOnly, true);
    assert.equal(first.value.payload.directorAuthorized, false);
    assert.equal(first.value.payload.director, false);
    assert.equal(first.value.payload.delayMs, publicDelayMs);
    report.publicDelayMs = publicDelayMs;
    const invalidAfter = viewer.sequence;
    viewer.send({ type: 'walk', direction: 'Up' });
    await viewer.wait((value) => value.type === 'error' && String(value.message).includes('invalid spectator control'), invalidAfter);
    report.checks.readOnlyAndDelayEnforced = true;
    const live = await viewer.wait((value) => value.type === 'spectatorStatus' && value.payload?.sequence > 0
      && value.payload?.capturedAtMs >= streamStartedAtMs,
      first.sequence, 50_000);
    const world = player && viewer.messages.findLast((entry) => entry.sequence < live.sequence
      && entry.value.type === 'worldSnapshot' && entry.value.payload?.mapFileName === '0');
    assert.ok(world, 'Fresh public status must have its corresponding world frame');
    for (const field of ['inventoryItems', 'storageItems', 'questLog', 'knownSkills', 'activeBuffs']) {
      assert.ok(Array.isArray(world.value.payload[field]) && world.value.payload[field].length === 0,
        `Private spectator field must be empty: ${field}`);
    }
    const next = await viewer.wait((value) => value.type === 'spectatorStatus'
      && value.payload?.sequence > live.value.payload.sequence
      && value.payload?.capturedAtMs > live.value.payload.capturedAtMs, live.sequence, 15_000);
    report.checks.actualSharedFramesAndRedaction = true;
    report.frames = { first: live.value.payload.sequence, next: next.value.payload.sequence,
      capturedAtMs: next.value.payload.capturedAtMs, entityCount: world.value.payload.entities?.length };
    console.log(JSON.stringify({ liveFrames: report.frames, holdMs }));
    const holdUntil = Date.now() + Math.max(0, holdMs - publicDelayMs - 5_000);
    while (Date.now() < holdUntil) {
      const releaseFile = process.env.MIR2_WATCH_QA_RELEASE_FILE;
      if (releaseFile && await fs.access(releaseFile).then(() => true, () => false)) break;
      await delay(500);
    }
  } catch (error) {
    report.failure = String(error.message).replaceAll(password, '[redacted]');
    throw error;
  } finally {
    clearInterval(keepAlive);
    viewer?.socket.close();
    if (inGame && player?.socket.readyState === Ws.OPEN) {
      await player.request({ type: 'logOut' }, 'LogOutSuccess', 15_000);
      report.checks.normalOwnedQaLogout = true;
      inGame = false;
    }
    player?.socket.close();
    await fs.mkdir(path.dirname(output), { recursive: true });
    await fs.writeFile(output, JSON.stringify(report, null, 2));
  }
  console.log(JSON.stringify({ passed: true, output }));
}

main().catch((error) => { console.error(error.message); process.exitCode = 1; });
