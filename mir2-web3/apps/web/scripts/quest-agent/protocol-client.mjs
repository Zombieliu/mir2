import fs from 'node:fs/promises';
import { applyProtocolObservation } from './protocol-observation.mjs';

const allowed = new Set(['clientVersion', 'newAccount', 'login', 'newCharacter', 'startGame', 'townRevive', 'keepAlive', 'walk', 'run', 'turn', 'attack', 'attackDirection', 'magic', 'spellToggle', 'harvest', 'interact', 'selectNpcDialog', 'acceptQuest', 'finishQuest', 'pickUp', 'pickUpTile', 'equipItem', 'moveItem', 'useItem', 'buyItem', 'sellItem', 'logOut']);
const DAMAGE_VITALS_REFRESH_MS = 500;
export const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

async function appendTraceLine(appendFile, output, line, retryDelay = delay) {
  for (let attempt = 0; ; attempt += 1) {
    try {
      await appendFile(output, line);
      return;
    } catch (error) {
      // Defender/indexing and read-only QA tails can briefly hold a newly
      // created trace on Windows. Route evidence is serialized already, so a
      // bounded multi-second retry is safer than terminating a live character
      // for an unrelated file-system share violation.
      if (!['EBUSY', 'EACCES', 'EPERM'].includes(String(error?.code)) || attempt >= 12) throw error;
      await retryDelay(Math.min(1_000, 25 * (2 ** attempt)));
    }
  }
}

export function startGameBootstrapEvidence(client, afterSequence, characterName) {
  const after = Number(afterSequence);
  const userInformation = (client?.events ?? []).find(event =>
    Number(event?.sequence) > after && event?.direction === 'received' &&
    event?.packet === 'UserInformation');
  if (userInformation) return { source: 'UserInformation', event: userInformation };
  const snapshot = client?.snapshot;
  const player = (snapshot?.entities ?? []).find(entity =>
    String(entity?.objectId) === String(snapshot?.playerObjectId) &&
    String(entity?.name ?? '') === String(characterName ?? ''));
  return player ? { source: 'personalSnapshot', player } : null;
}

/** Normal local gateway commands only; never mutate saves or use QA commands. */
export class ProtocolClient {
  constructor(url, output, timing = {}) {
    const endpoint = new URL(url);
    if (endpoint.protocol !== 'ws:' || !['127.0.0.1', 'localhost', '[::1]'].includes(endpoint.hostname)) throw new Error('Journey runner requires a local gateway');
    this.url = url; this.output = output; this.events = []; this.snapshot = null; this.sequence = 0; this.writeQueue = Promise.resolve();
    this.questDefinitions = new Map();
    this.now = typeof timing.now === 'function' ? timing.now : Date.now;
    this.scheduleTimeout = typeof timing.setTimeout === 'function' ? timing.setTimeout : setTimeout;
    this.cancelTimeout = typeof timing.clearTimeout === 'function' ? timing.clearTimeout : clearTimeout;
    this.appendTrace = typeof timing.appendFile === 'function' ? timing.appendFile : fs.appendFile;
    this.traceRetryDelay = typeof timing.traceRetryDelay === 'function' ? timing.traceRetryDelay : delay;
    this.lastDamageVitalsRefreshAt = null;
    this.damageVitalsRefreshTimer = null;
  }
  record(direction, value) {
    if (direction === 'received' && value?.packet === 'NewQuestInfo') {
      const questId = Number(value.payload?.info?.index);
      if (Number.isSafeInteger(questId) && questId > 0) {
        this.questDefinitions.set(questId, structuredClone(value.payload.info));
      }
    }
    const safe = { ...value };
    for (const field of ['token', 'password', 'secretAnswer']) if (field in safe) safe[field] = '[redacted]';
    const event = { ...safe, movementDirection: value.direction, sequence: ++this.sequence, at: new Date().toISOString(), direction };
    this.events.push(event);
    if (this.events.length > 10000) this.events.splice(0, 1000);
    const line = JSON.stringify(event) + '\n';
    this.writeQueue = this.writeQueue.then(() => appendTraceLine(
      this.appendTrace, this.output, line, this.traceRetryDelay,
    ));
    return event;
  }
  async connect() {
    this.questDefinitions.clear();
    this.record('lifecycle', { type: 'connection', url: this.url });
    this.ws = new WebSocket(this.url);
    this.ws.addEventListener('message', event => {
      try {
        const message = JSON.parse(event.data);
        this.observeGatewayMessage(message);
      } catch (error) { this.failure = error; }
    });
    this.ws.addEventListener('close', () => { this.closed = true; });
    this.ws.addEventListener('error', () => { this.failure = new Error('Gateway connection failed'); });
    await this.wait(() => this.ws.readyState === WebSocket.OPEN, 'connection');
    this.send({ type: 'clientVersion' });
    this.timer = setInterval(() => {
      if (this.ws.readyState === WebSocket.OPEN) this.send({ type: 'keepAlive', time: Date.now() });
    }, 2000);
    this.stopHandler = () => { this.failure = new Error('Journey stopped; progress will save on disconnect'); };
    process.once('SIGINT', this.stopHandler);
    process.once('SIGTERM', this.stopHandler);
  }
  observeGatewayMessage(message) {
    this.snapshot = applyProtocolObservation(this.snapshot, message);
    this.record('received', message);
    if (message.type === 'error') this.failure = new Error(`Gateway rejected command: ${message.message ?? 'unknown error'}`);
    if (message.packet === 'DamageIndicator'
      && Number(message.payload?.objectId) === Number(this.snapshot?.playerObjectId)) {
      this.scheduleDamageVitalsRefresh();
    }
  }
  scheduleDamageVitalsRefresh() {
    if (this.ws?.readyState !== 1) return;
    const now = this.now();
    const elapsed = this.lastDamageVitalsRefreshAt == null
      ? DAMAGE_VITALS_REFRESH_MS
      : now - this.lastDamageVitalsRefreshAt;
    if (elapsed >= DAMAGE_VITALS_REFRESH_MS && this.damageVitalsRefreshTimer == null) {
      this.lastDamageVitalsRefreshAt = now;
      this.send({ type: 'clientVersion' });
      return;
    }
    if (this.damageVitalsRefreshTimer != null) return;
    const delayMs = Math.max(0, DAMAGE_VITALS_REFRESH_MS - Math.max(0, elapsed));
    this.damageVitalsRefreshTimer = this.scheduleTimeout(() => {
      this.damageVitalsRefreshTimer = null;
      if (this.ws?.readyState !== 1) return;
      this.lastDamageVitalsRefreshAt = this.now();
      this.send({ type: 'clientVersion' });
    }, delayMs);
  }
  send(command) {
    if (!allowed.has(command.type)) throw new Error(`Forbidden journey command: ${command.type}`);
    if (command.type === 'startGame') this.questDefinitions.clear();
    const safe = { ...command };
    for (const field of ['password', 'secretAnswer']) if (field in safe) safe[field] = '[redacted]';
    this.record('sent', safe);
    this.ws.send(JSON.stringify(command));
  }
  questDefinition(questId) {
    return this.questDefinitions.get(Number(questId)) ?? null;
  }
  async wait(predicate, label, timeout = 20000) {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      if (this.failure) throw this.failure;
      const result = predicate();
      if (result) return result;
      if (this.closed) throw new Error(`Connection closed waiting for ${label}`);
      await delay(40);
    }
    throw new Error(`Timeout waiting for ${label}`);
  }
  async request(command, packet, timeout) {
    const after = this.sequence;
    this.send(command);
    return this.wait(() => this.events.find(e => e.sequence > after && e.direction === 'received' && e.packet === packet), packet, timeout);
  }
  async close() {
    clearInterval(this.timer);
    if (this.damageVitalsRefreshTimer != null) {
      this.cancelTimeout(this.damageVitalsRefreshTimer);
      this.damageVitalsRefreshTimer = null;
    }
    if (this.stopHandler) { process.removeListener('SIGINT', this.stopHandler); process.removeListener('SIGTERM', this.stopHandler); }
    if (this.ws?.readyState === WebSocket.OPEN) {
      // SIGINT/SIGTERM deliberately interrupts the active journey through
      // `failure`, but that same failure must not prevent the final LogOut
      // request from waiting for its acknowledgement and authoritative save.
      const journeyFailure = this.failure;
      this.failure = null;
      // R99's save queue delayed receipts beyond the old five-second window.
      try { await this.request({ type: 'logOut' }, 'LogOutSuccess', 20000); } catch { /* Trace preserves failed logout; disconnect still saves normally. */ }
      finally { this.failure ??= journeyFailure; }
      this.ws.close();
    }
    await this.writeQueue;
  }
}
