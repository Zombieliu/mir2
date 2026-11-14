// Server/protocol timing only: this does not establish visual smoothness or
// newcomer/120-minute acceptance. No QA commands, transfers or save mutations.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { ProtocolClient, delay, startGameBootstrapEvidence } from './protocol-client.mjs';
import { loadProtocolCollisionMap, protocolMapCellIsWalkable } from './protocol-navigation.mjs';

const args = {};
for (let i = 2; i < process.argv.length; i += 2) {
  const key = process.argv[i];
  if (key === '--help') {
    console.log('node probe-movement-cadence.mjs --endpoint ws://127.0.0.1:PORT/ws --output PATH [--count 64] [--map-root PATH]');
    process.exit(0);
  }
  if (!['--endpoint', '--output', '--count', '--map-root'].includes(key) || !process.argv[i + 1]) throw new Error(`Invalid argument ${key}`);
  args[key.slice(2)] = process.argv[i + 1];
}
if (!args.endpoint || !args.output) throw new Error('--endpoint and --output are required');
const endpoint = new URL(args.endpoint);
if (endpoint.protocol !== 'ws:' || !['localhost', '127.0.0.1', '[::1]'].includes(endpoint.hostname)) throw new Error('Only localhost ws endpoints are allowed');
const count = Number(args.count ?? 64);
if (!Number.isInteger(count) || count < 64 || count > 1024) throw new Error('--count must be 64..1024');
// A normal Windows setTimeout can overshoot by several milliseconds. That
// hides the exact 600 ms ready-boundary case: every packet then arrives after
// the server cooldown. Use a bounded final spin in this isolated timing probe
// only, and report the actual send intervals; production clients never spin.
async function waitUntil(due) {
  let remaining = due - performance.now();
  while (remaining > 20) {
    await delay(remaining - 20);
    remaining = due - performance.now();
  }
  while (performance.now() < due) { /* <=20 ms timing-probe-only tail */ }
}
const output = path.resolve(args.output);
await fs.mkdir(output, { recursive: true });
const runId = `${Date.now()}-${crypto.randomBytes(4).toString('hex')}`;
const suffix = crypto.randomBytes(5).toString('hex');
const credentials = { accountId: `mc${suffix}`, password: crypto.randomBytes(12).toString('hex'), name: `M${suffix}` };
const privateDirectory = path.join(output, 'private');
await fs.mkdir(privateDirectory, { recursive: true, mode: 0o700 });
await fs.writeFile(path.join(privateDirectory, `${runId}.credentials.json`), JSON.stringify(credentials, null, 2), { flag: 'wx', mode: 0o600 });

class TimingClient extends ProtocolClient {
  listeners = new Set();
  observeGatewayMessage(message) {
    const receivedPerformanceMs = performance.now();
    super.observeGatewayMessage({ ...message, receivedPerformanceMs });
    const event = this.events.at(-1);
    for (const listener of [...this.listeners]) listener(event);
  }
  // Register before sending; resolve directly from the receive callback.
  packetRequest(command, packet, timeout = 20_000) {
    return new Promise((resolve, reject) => {
      const finish = (error, event) => {
        clearTimeout(timer);
        this.listeners.delete(listener);
        error ? reject(error) : resolve(event);
      };
      const listener = event => {
        if (event.packet === packet) finish(null, event);
        else if (event.type === 'error') finish(new Error(event.message ?? 'Gateway error'));
      };
      const timer = setTimeout(() => finish(new Error(`Timeout waiting for ${packet}`)), timeout);
      this.listeners.add(listener);
      try { this.send(command); } catch (error) { finish(error); }
    });
  }
}
const client = new TimingClient(args.endpoint, path.join(output, `${runId}.trace.jsonl`));
const report = { runId, endpoint: args.endpoint, measurement: 'ordinary-public-WS-server-protocol-only',
  visualAcceptance: false, duration120MinuteAcceptance: false, nominalAnimationMs: 600, requestedCommands: count,
  scheduler: '600ms-monotonic-deadline-with-at-most-20ms-probe-only-spin-tail',
  samples: [], status: 'running', logoutSuccess: false };
const self = () => client.snapshot?.entities?.find(e => String(e.objectId) === String(client.snapshot.playerObjectId));
const mapName = () => client.snapshot?.mapFileName;
const occupied = () => (client.snapshot?.entities ?? []).filter(e =>
  String(e.objectId) !== String(client.snapshot?.playerObjectId) && !e.dead &&
  ['Npc', 'Monster', 'RemotePlayer', 'Player', 'npc', 'monster', 'remotePlayer'].includes(e.kind) &&
  Number.isFinite(e.x) && Number.isFinite(e.y));
const point = e => ({ x: Number(e.x), y: Number(e.y) });
let baselineMap;
function assertPlayable() {
  if (client.failure) throw client.failure;
  if (client.closed) throw new Error('Connection closed');
  if (!self() || self().dead || self().hp === 0) throw new Error('Self absent/dead');
  if (mapName() !== baselineMap) throw new Error('Map changed; stopping');
}
function stats(values) {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return { count: values.length, minMs: sorted[0], meanMs: values.reduce((a, b) => a + b, 0) / values.length,
    p50Ms: sorted[Math.floor((sorted.length - 1) * 0.5)], p95Ms: sorted[Math.floor((sorted.length - 1) * 0.95)], maxMs: sorted.at(-1) };
}
try {
  await client.connect();
  const registered = await client.packetRequest({ type: 'newAccount', ...credentials }, 'NewAccount');
  if (![7, 8].includes(registered.payload?.result)) throw new Error(`Registration rejected ${registered.payload?.result}`);
  await delay(200);
  await client.packetRequest({ type: 'login', accountId: credentials.accountId, password: credentials.password }, 'LoginSuccess');
  const created = await client.packetRequest({ type: 'newCharacter', name: credentials.name, class: 'Warrior', gender: 'Male' }, 'NewCharacterSuccess');
  const before = client.sequence;
  client.send({ type: 'startGame', characterIndex: created.payload.character.index });
  await client.wait(() => startGameBootstrapEvidence(client, before, credentials.name), 'bootstrap');
  await client.wait(() => self()?.name === credentials.name && mapName(), 'self snapshot');
  baselineMap = mapName();
  assertPlayable();
  const spawn = point(self());
  const map = await loadProtocolCollisionMap(baselineMap, args['map-root'] ? { mapRoot: args['map-root'] } : {});
  const axes = [{ x: 1, y: 0, forward: 'right', reverse: 'left' }, { x: 0, y: 1, forward: 'down', reverse: 'up' }];
  const candidates = axes.map(axis => {
    let low = 0, high = 0;
    for (const sign of [-1, 1]) for (let n = 1; n <= 10; n++) {
      const p = { x: spawn.x + axis.x * sign * n, y: spawn.y + axis.y * sign * n };
      if (!protocolMapCellIsWalkable(map, p, occupied())) break;
      if (sign < 0) low = -n; else high = n;
    }
    return { ...axis, low, high };
  }).filter(c => c.high - c.low >= 6).sort((a, b) => (b.high - b.low) - (a.high - a.low));
  if (!candidates.length) throw new Error('No collision-free 6+ tile corridor through actual spawn; no movement sent');
  const corridor = candidates[0];
  report.mapFileName = baselineMap; report.spawn = spawn; report.spawnInSafeZone = client.snapshot?.inSafeZone ?? null;
  report.collisionSource = map.sourcePath; report.corridor = corridor;
  let sign = corridor.high >= 2 ? 1 : -1;
  let previousSend = -Infinity, previousAck = -Infinity, rejections = 0;
  for (let i = 0; i < count; i++) {
    const due = Math.max(previousSend + 600, previousAck);
    await waitUntil(due);
    assertPlayable();
    const from = point(self());
    const offset = corridor.x ? from.x - spawn.x : from.y - spawn.y;
    if (offset + sign * 2 < corridor.low || offset + sign * 2 > corridor.high) sign *= -1;
    const direction = sign > 0 ? corridor.forward : corridor.reverse;
    const intermediate = { x: from.x + corridor.x * sign, y: from.y + corridor.y * sign };
    const target = { x: from.x + corridor.x * sign * 2, y: from.y + corridor.y * sign * 2 };
    if (![intermediate, target].every(p => protocolMapCellIsWalkable(map, p, occupied()))) throw new Error('Corridor became occupied/blocked; stopping');
    const sentPerformanceMs = performance.now();
    const receipt = await client.packetRequest({ type: 'run', direction }, 'UserLocation', 5000);
    assertPlayable();
    const to = point(self());
    const moved = to.x === target.x && to.y === target.y;
    const degradedWalk = to.x === intermediate.x && to.y === intermediate.y;
    const rejected = !moved && !degradedWalk;
    rejections = rejected ? rejections + 1 : 0;
    report.samples.push({ index: i, from, target, to, direction, sentPerformanceMs,
      receivedPerformanceMs: receipt.receivedPerformanceMs, ackMs: receipt.receivedPerformanceMs - sentPerformanceMs,
      sendIntervalMs: Number.isFinite(previousSend) ? sentPerformanceMs - previousSend : null,
      degradedWalk, correction: rejected, packetSequence: receipt.sequence });
    previousSend = sentPerformanceMs; previousAck = receipt.receivedPerformanceMs;
    if (rejections >= 2) throw new Error('Two consecutive rejected/corrected movement commands');
    if (rejected && (to.x !== from.x || to.y !== from.y)) throw new Error('Unexpected position correction');
  }
  report.status = 'completed';
} catch (error) {
  report.status = 'stopped'; report.error = String(error?.stack ?? error); process.exitCode = 1;
} finally {
  // ProtocolClient.close uses ordinary logOut and waits for LogOutSuccess.
  const beforeLogout = client.sequence;
  try { await client.close(); } catch (error) { report.closeError = String(error); }
  report.logoutSuccess = client.events.some(e => e.sequence > beforeLogout && e.packet === 'LogOutSuccess');
  if (!report.logoutSuccess) { report.status = 'stopped'; process.exitCode = 1; }
  const quarter = Math.floor(report.samples.length / 4);
  report.firstQuarterAck = stats(report.samples.slice(0, quarter).map(s => s.ackMs));
  report.lastQuarterAck = stats(quarter ? report.samples.slice(-quarter).map(s => s.ackMs) : []);
  report.firstQuarterSendIntervals = stats(report.samples.slice(0, quarter).map(s => s.sendIntervalMs).filter(Number.isFinite));
  report.lastQuarterSendIntervals = stats((quarter ? report.samples.slice(-quarter) : []).map(s => s.sendIntervalMs).filter(Number.isFinite));
  report.sendIntervals = stats(report.samples.map(s => s.sendIntervalMs).filter(Number.isFinite));
  report.corrections = report.samples.filter(s => s.correction).length;
  report.noCorrectionObserved = report.samples.length > 0 && report.corrections === 0;
  report.degradedWalks = report.samples.filter(s => s.degradedWalk).length;
  report.completedCommands = report.samples.length;
  await fs.writeFile(path.join(output, `${runId}.report.json`), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ report: path.join(output, `${runId}.report.json`), status: report.status,
    completedCommands: report.completedCommands, logoutSuccess: report.logoutSuccess, corrections: report.corrections,
    firstQuarterAck: report.firstQuarterAck, lastQuarterAck: report.lastQuarterAck, sendIntervals: report.sendIntervals }, null, 2));
}
