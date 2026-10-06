// One prospective prepared-geometry trial. Never edits frozen input or saves.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';
import { gunzipSync } from 'node:zlib';

const [frozenArg, outputArg, url] = process.argv.slice(2);
if (!path.isAbsolute(frozenArg ?? '') || !path.isAbsolute(outputArg ?? '')) throw new Error('Explicit absolute input/output roots required');
const frozen = path.resolve(frozenArg), output = path.resolve(outputArg);
const endpoint = new URL(url);
if (endpoint.protocol !== 'ws:' || endpoint.hostname !== '127.0.0.1') throw new Error('Owned loopback only');
const sha = data => crypto.createHash('sha256').update(data).digest('hex');
const classifierFile = path.join(frozen, 'dense_observation.mjs');
if (sha(await fs.readFile(classifierFile)) !== 'f78103fb5ddb1516caac0f0b088bacd01421c5d9ede603a5c75a3c2b2c95dc0d') throw new Error('Frozen v6 classifier mismatch');
const mapBytes = await fs.readFile(path.join(frozen, 'maps/d021.map.gz'));
if (sha(mapBytes) !== '5e283984b31ee993c8d1157402f50d7e9d4984d8be92db49388c1e024e257ca2') throw new Error('Original D021 map pack mismatch');
if (sha(gunzipSync(mapBytes)) !== 'ef8d4d9499d64bc16161abebc2548a3c9a98e99a7f634205ba45a3532db44de3') throw new Error('Original D021 decoded map mismatch');
const { ProtocolClient, delay } = await import(pathToFileURL(path.join(frozen, 'vendor/protocol-client.mjs')));
const { loadProtocolCollisionMap } = await import(pathToFileURL(path.join(frozen, 'vendor/protocol-navigation.mjs')));
const { applyDenseObservation, ownerState, self, location, qualifySeven, escapeProof, driftProof, ACCEPTANCE_FALSE } = await import(pathToFileURL(classifierFile));
const Ws = createRequire(import.meta.url)(path.join(frozen, 'vendor/ws.cjs'));
const metadata = JSON.parse(await fs.readFile(path.join(frozen, 'WORLD-INPUTS.json'), 'utf8')).maps.find(m => m.map === 'D021');
const map = await loadProtocolCollisionMap('D021', { packagedMapRoot: path.join(frozen, 'maps') });
let raw = ''; for await (const chunk of process.stdin) raw += chunk;
const account = JSON.parse(raw); raw = '';
const secretKeys = /password|secret|token|authorization|cookie|pepper|private.?key/i;
function redact(value) {
  if (Array.isArray(value)) return value.map(redact);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, secretKeys.test(k) ? '[redacted]' : redact(v)]));
  return typeof value === 'string' ? value.split(account.password).join('[redacted]') : value;
}
async function append(line) {
  for (let attempt = 0; ; attempt++) try { await fs.appendFile(path.join(output, 'trace.jsonl'), line); return; }
  catch (error) {
    if (!['EBUSY', 'EACCES', 'EPERM'].includes(error.code) || attempt >= 12) throw error;
    await delay(Math.min(1000, 25 * 2 ** attempt));
  }
}
class Client extends ProtocolClient {
  constructor() { super(url, path.join(output, 'trace.jsonl')); this.inGame = false; this.connectionId = crypto.randomUUID(); }
  record(direction, value) {
    const safe = redact(value);
    const event = { ...safe, direction, movementDirection: safe.direction, connectionId: this.connectionId,
      sequence: ++this.sequence, tsMs: Date.now(), state: ownerState(this.snapshot, value?.payload?.objectId) };
    this.events.push(event); // complete finite trace; no ring truncation
    this.writeQueue = this.writeQueue.then(() => append(JSON.stringify(event) + '\n'));
    this.writeQueue.catch(e => { this.failure = e; });
    return event;
  }
  observeGatewayMessage(message) {
    this.snapshot = applyDenseObservation(this.snapshot, message, Date.now());
    this.record('received', message);
    if (message.type === 'error') this.failure = new Error(`Gateway rejected normal command: ${message.message ?? 'unknown'}`);
    if (message.packet === 'DamageIndicator' && Number(message.payload?.objectId) === this.snapshot?.playerObjectId) this.scheduleDamageVitalsRefresh();
  }
  async connect() {
    this.ws = new Ws(url, { origin: `http://${endpoint.host}`, handshakeTimeout: 10000 });
    this.ws.addEventListener('message', e => { try { this.observeGatewayMessage(JSON.parse(e.data)); } catch (error) { this.failure = error; } });
    this.ws.addEventListener('error', () => { this.failure = new Error('Owned loopback connection failed'); });
    await this.wait(() => this.ws.readyState === 1, 'owned WebSocket open', 10000);
    this.send({ type: 'clientVersion' });
    this.timer = setInterval(() => { if (this.ws.readyState === 1) this.send({ type: 'keepAlive', time: Date.now() }); }, 2000);
  }
  async refresh() {
    const sequence = this.sequence; this.send({ type: 'clientVersion' });
    await this.wait(() => this.events.find(e => e.sequence > sequence && e.type === 'worldSnapshot'), 'fresh ordinary snapshot', 5000);
  }
  async close() {
    clearInterval(this.timer);
    if (this.damageVitalsRefreshTimer != null) this.cancelTimeout(this.damageVitalsRefreshTimer);
    const failure = this.failure; this.failure = null;
    try {
      if (this.inGame && this.ws?.readyState === 1) {
        const logout = await this.request({ type: 'logOut' }, 'LogOutSuccess', 10000);
        this.logoutAckSequence = logout.sequence; this.inGame = false;
      }
      if (this.inGame) throw new Error('World connection closed without ordinary logout acknowledgement');
      this.ws?.close();
      const deadline = Date.now() + 5000;
      while (this.ws && this.ws.readyState !== 3 && Date.now() < deadline) await delay(20);
      if (this.ws && this.ws.readyState !== 3) throw new Error('Normal socket close did not finish');
      this.normalClose = true; await this.writeQueue;
    } finally { this.failure ??= failure; }
  }
}
const c = new Client();
const report = { kind: 'preparedGeometryNormalWebSocket', className: account.className,
  originalSourceMapSha256: '5e283984b31ee993c8d1157402f50d7e9d4984d8be92db49388c1e024e257ca2',
  originalDecompressedMapSha256: 'ef8d4d9499d64bc16161abebc2548a3c9a98e99a7f634205ba45a3532db44de3',
  unchangedClassifierSha256: 'f78103fb5ddb1516caac0f0b088bacd01421c5d9ede603a5c75a3c2b2c95dc0d',
  declaredFormationBudgetMs: 30000, trials: 1, productionR18ElfUsed: false, ...ACCEPTANCE_FALSE };
const after = (sequence, packet, predicate = () => true) => c.events.find(e => e.sequence > sequence && e.direction === 'received' && e.packet === packet && predicate(e.payload ?? {}));
async function equipOrdinaryStarter() {
  const receipts = [];
  for (const [slot, to] of [['weapon', 0], ['armour', 1], ['torch', 3]]) {
    if ((c.snapshot?.equipmentItems ?? []).some(i => (i.equipSlot ?? i.slot) === slot)) continue;
    const item = c.snapshot?.inventoryItems?.find(i => i.equipSlot === slot);
    if (!item) continue;
    const ack = await c.request({ type: 'equipItem', grid: 'inventory', uniqueId: item.uniqueId, to }, 'EquipItem');
    if (!ack.payload?.success) throw new Error('Ordinary starter equipment rejected');
    await c.refresh();
    receipts.push({ item: item.name, uniqueId: item.uniqueId, slot, ackSequence: ack.sequence });
  }
  return receipts;
}
try {
  await c.connect();
  await c.request({ type: 'login', accountId: account.accountId, password: account.password }, 'LoginSuccess');
  const start = await c.request({ type: 'startGame', characterIndex: account.characterIndex }, 'StartGame');
  if (start.payload?.result !== 4) throw new Error('Normal StartGame rejected');
  c.inGame = true;
  await c.wait(() => self(c.snapshot)?.name === account.name && c.snapshot?.mapFileName === 'D021', 'original map admission');
  const [expectedHp, expectedMp] = { Warrior: [419, 116], Wizard: [128, 541], Taoist: [239, 260] }[account.className];
  if (self(c.snapshot).level !== 30 || c.snapshot.playerHp !== expectedHp || c.snapshot.playerMp !== expectedMp) {
    throw new Error('Actual bootstrap differs from declared canonical level/vitals');
  }
  report.bootstrap = { source: 'ordinaryBootstrap', hp: c.snapshot.playerHp, mp: c.snapshot.playerMp,
    maxHp: c.snapshot.playerMaxHp, stats: c.snapshot.playerCrystalStats, location: location(c.snapshot) };
  // Match the previously declared ordinary starter loadout. These are real
  // EquipItem requests against existing inventory, before AI is enabled.
  report.starterEquipment = await equipOrdinaryStarter();
  report.equippedBootstrap = { stats: c.snapshot.playerCrystalStats, equipment: redact(c.snapshot.equipmentItems) };
  await fs.writeFile(path.join(output, 'admitted.json'), JSON.stringify({ ackSequence: start.sequence, firstAdmissionMs: start.tsMs }), { flag: 'wx' });
  const deadline = start.tsMs + 30000;
  const observations = []; let q;
  while (Date.now() < deadline) {
    await c.refresh();
    q = qualifySeven(c.events, c.snapshot, map, metadata, Date.now());
    observations.push(q);
    if (q.status === 'qualified' || self(c.snapshot)?.dead || c.snapshot.playerHp === 0) break;
    await delay(200);
  }
  report.formationObservations = observations;
  report.qualification = q ?? { status: 'inconclusive', reason: 'No qualified observation inside budget' };
  if (q?.status === 'qualified') {
    const phaseSequence = c.sequence;
    // One real newly observed source windup, followed by an ordinary wait.
    // No deadline writes, favourable roll retry, or fabricated damage.
    // The normal 15-second StartGame grace defers the personal combat flush.
    // Keep ordinary snapshot refreshes, as in formation, rather than assuming
    // monster ObjectAttack is on the dedicated movement live channel. The
    // source clocks and receipt/death classifier remain unchanged.
    const windupDeadline = Date.now() + 5000;
    let windup;
    while (Date.now() < windupDeadline) {
      await c.refresh();
      windup = after(phaseSequence, 'ObjectAttack', p => q.attackerIds.includes(p.objectId));
      if (windup) break;
      if (self(c.snapshot)?.dead || c.snapshot.playerHp === 0) throw new Error('Owner died before next actual bat windup');
      await delay(200);
    }
    if (!windup) throw new Error('Timeout waiting for next actual bat windup with ordinary snapshot refresh');
    await delay(200);
    const target = c.snapshot.entities.find(e => q.attackerIds.includes(e.objectId) && !e.dead);
    const seq = c.sequence;
    c.send({ type: 'attack', objectId: target.objectId });
    const committed = await c.wait(() => after(seq, 'ObjectAttack', p => p.objectId === c.snapshot.playerObjectId), 'real owner attack commitment', 2500);
    await c.refresh();
    const refreshed = qualifySeven(c.events, c.snapshot, map, metadata, Date.now());
    if (refreshed.status !== 'qualified') {
      report.escape = { status: 'inconclusive', reason: 'Pressure/exit expired after real attack', qualification: refreshed, committedAckSequence: committed.sequence };
    } else {
      const before = location(c.snapshot), sentMs = Date.now();
      c.send({ type: 'walk', direction: refreshed.exit.direction }); const sentSequence = c.sequence;
      let ackWaitError = null;
      try { await c.wait(() => after(sentSequence, 'UserLocation', p => {
        const tile = p.location ?? p; return tile.x === refreshed.exit.target.x && tile.y === refreshed.exit.target.y;
      }), 'single ordinary escape ACK', 3200); } catch (error) { ackWaitError = error.message; }
      await c.refresh();
      const proof = escapeProof(c.events, { qualification: refreshed, sentSequence, sentMs,
        endMs: Date.now(), ownerId: c.snapshot.playerObjectId, before, target: refreshed.exit.target,
        boundMs: 1500, committedKind: 'attack', committedAckSequence: committed.sequence });
      proof.windupSequence = windup.sequence; proof.ackWaitError = ackWaitError;
      const ack = c.events.find(e => e.sequence === proof.ackSequence);
      const anchor = ack?.state ?? before, anchorSequence = ack?.sequence ?? c.sequence, anchorMs = ack?.tsMs ?? Date.now();
      const quietEnd = Date.now() + 5200;
      while (Date.now() < quietEnd) { await delay(400); await c.refresh(); }
      proof.stop = driftProof(c.events, { anchorSequence, anchorMs, anchor, endMs: Date.now(), ownerId: c.snapshot.playerObjectId, durationMs: 5000 });
      if (proof.stop.status === 'failed') proof.status = 'failed';
      report.escape = proof;
    }
  } else report.escape = { status: 'inconclusive', reason: 'Formation precondition did not qualify; action trial not executed' };
  report.finalSnapshot = redact(c.snapshot);
} catch (error) { report.executionError = redact(error.message); }
finally {
  report.finalSnapshot ??= redact(c.snapshot);
  try { await c.close(); report.normalLogoutAndClose = c.normalClose === true && c.logoutAckSequence > 0; report.logoutAckSequence = c.logoutAckSequence; }
  catch (error) { report.cleanupError = redact(error.message); }
  report.rawEventCount = c.events.length;
  await fs.writeFile(path.join(output, 'report.json'), JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
}
if (report.executionError || report.cleanupError || !report.normalLogoutAndClose) process.exitCode = 1;
