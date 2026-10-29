// Real nativeResumeV1 control exchange against an explicitly isolated realm.
// Uses one already registered ordinary actor; credentials and tickets stay private.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { PlaytestClient, authenticate, exposesAuthenticatedGameplay, inventoryFingerprint,
  outsideRepository, redact, resumeCredentials, validateEndpoint } from './playtest-multiplayer-smoke.mjs';
import { findProtocolWalkPath, loadProtocolCollisionMap } from './quest-agent/protocol-navigation.mjs';
import { CATALOG_GZIP_CAPABILITY } from './playtest-catalog-transport.mjs';

const PROTOCOL = 'nativeResumeV1';
const REQUIRED = ['ordinaryLogin', 'stepBeforeDrop', 'uncleanTransportLoss', 'nativeResume',
  'sameState', 'rotatedTicket', 'replayRejected', 'resumedOwnerLive', 'logout'];
const sleep = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const self = snapshot => (snapshot?.entities ?? []).find(entity =>
  Number(entity.objectId) === Number(snapshot.playerObjectId));

export function validateResumeControl(command) {
  const keys = Object.keys(command).sort().join(',');
  if (command.type === 'clientCapabilities' && keys === 'capabilities,type' &&
      [JSON.stringify([PROTOCOL]), JSON.stringify([PROTOCOL, CATALOG_GZIP_CAPABILITY])]
        .includes(JSON.stringify(command.capabilities))) return;
  if (command.type === 'resumeSession' && keys === 'credential,type' &&
      typeof command.credential === 'string' && /^[A-Za-z0-9_-]{43}$/.test(command.credential) &&
      Buffer.from(command.credential, 'base64url').length === 32 &&
      Buffer.from(command.credential, 'base64url').toString('base64url') === command.credential) return;
  throw new Error('Invalid native resume control; identity, character and gameplay overrides are forbidden');
}

export function resumeFingerprint(snapshot, name) {
  const player = self(snapshot);
  const sameName = (snapshot?.entities ?? []).filter(entity => entity.name === name);
  if (!player || player.name !== name || sameName.length !== 1) {
    throw new Error('Authoritative snapshot must contain exactly one copy of the expected actor');
  }
  return { name, playerObjectId: Number(player.objectId), mapFileName: String(snapshot.mapFileName),
    x: Number(player.x), y: Number(player.y), direction: player.direction,
    inventory: inventoryFingerprint(snapshot) };
}

export function assertResumeEvidence({ before, after, characterIndex, resumed, firstGeneration,
  rotatedGeneration, ticketChanged, snapshotSequence, sentTypes }) {
  if (resumed?.type !== 'sessionResumed' || resumed.protocol !== PROTOCOL ||
      resumed.characterIndex !== characterIndex || resumed.generation !== firstGeneration + 1) {
    throw new Error('Native resume did not acknowledge the bound character and next generation');
  }
  if (!(snapshotSequence > resumed.sequence)) throw new Error('Authoritative snapshot did not follow sessionResumed');
  if (sentTypes.some(type => ['login', 'startGame', 'newAccount', 'newCharacter'].includes(type))) {
    throw new Error('Fresh login or StartGame cannot substitute for native session resume');
  }
  if (!ticketChanged || rotatedGeneration !== firstGeneration + 1) throw new Error('Native resume ticket did not rotate exactly once');
  if (JSON.stringify(before) !== JSON.stringify(after)) throw new Error('Native resume changed the saved authoritative actor state');
}

export function assertReplayRejected(events) {
  const rejection = events.find(event => event.type === 'resumeRejected');
  if (rejection?.code !== 'unavailable' || events.some(event =>
    event.type === 'sessionResumed' || exposesAuthenticatedGameplay(event))) {
    throw new Error('Used native resume ticket replay was not rejected without exposing an authenticated owner');
  }
  return { code: rejection.code, receipt: rejection.sequence };
}

export function matchesOwnerStep(event, after, target) {
  // The gateway's browser/native wire flattens UserLocation to top-level x/y.
  return event.sequence > after && event.packet === 'UserLocation' &&
    event.payload?.x === target.x && event.payload?.y === target.y;
}

export class NativeResumeClient extends PlaytestClient {
  record(direction, value) {
    // The ordinary smoke never reuses tokens. This protocol-specific client
    // additionally treats every credential field as a secret before any trace.
    const remember = item => {
      if (!item || typeof item !== 'object') return;
      for (const [key, entry] of Object.entries(item)) {
        if (key === 'credential' && typeof entry === 'string' && entry && !this.secrets.includes(entry)) this.secrets.push(entry);
        else remember(entry);
      }
    };
    remember(value);
    return super.record(direction, value);
  }
  observe(message) {
    if (message?.type === 'resumeCredential') {
      validateResumeControl({ type: 'resumeSession', credential: message.credential });
      if (message.protocol !== PROTOCOL || !Number.isInteger(message.generation) ||
          !Number.isFinite(message.expiresAtMs)) throw new Error('Invalid native resume credential event');
      this.nativeTicket = { credential: message.credential, generation: message.generation,
        expiresAtMs: message.expiresAtMs, sequence: this.sequence + 1 };
    }
    super.observe(message);
  }
  control(command) {
    validateResumeControl(command);
    this.record('sent', command);
    this.ws.send(JSON.stringify(command));
    if (command.type === 'clientCapabilities') this.catalogGzipOptedIn = command.capabilities.includes(CATALOG_GZIP_CAPABILITY);
  }
  async loseTransport() {
    if (this.ws?.readyState !== 1 || !this.inGame) throw new Error('Transport loss requires a live authenticated game');
    clearInterval(this.timer);
    this.record('local', { type: 'transportTerminated', logoutSent: false });
    this.ws.terminate(); // No LogOut or Disconnect application command.
    const deadline = Date.now() + 5000;
    while (!this.closed && Date.now() < deadline) await sleep(25);
    if (!this.closed) throw new Error('Terminated transport did not close');
    await this.writeQueue;
  }
}

async function ordinaryStep(client, mapRoot) {
  const from = { ...self(client.snapshot) }, mapName = String(client.snapshot.mapFileName);
  const map = await loadProtocolCollisionMap(mapName, mapRoot ? { mapRoot } : {});
  const occupied = client.snapshot.entities.filter(entity => entity.objectId !== from.objectId && !entity.dead);
  for (const [dx, dy, direction] of [[1, 0, 'Right'], [-1, 0, 'Left'], [0, 1, 'Down'], [0, -1, 'Up']]) {
    const target = { x: from.x + dx, y: from.y + dy };
    if ((client.snapshot.mapTransfers ?? []).some(transfer => {
      const bounds = transfer.bounds;
      return bounds && target.x >= bounds.minX && target.x <= bounds.maxX && target.y >= bounds.minY && target.y <= bounds.maxY;
    })) continue;
    const route = findProtocolWalkPath({ map, start: from, target, dynamicObstacles: occupied, maxExpanded: 100 });
    if (route?.length !== 2) continue;
    await sleep(650);
    const after = client.sequence;
    client.send({ type: 'walk', direction });
    const receipt = await client.wait(() => client.events.find(event =>
      matchesOwnerStep(event, after, target)), 'authoritative one-step movement', 5000);
    await client.refresh();
    const actual = self(client.snapshot);
    if (String(client.snapshot.mapFileName) !== mapName || actual?.x !== target.x || actual?.y !== target.y) {
      throw new Error('One-step movement disagrees with the fresh authoritative snapshot');
    }
    return { from: { mapFileName: mapName, x: from.x, y: from.y }, to: { ...target, direction }, receipt: receipt.sequence };
  }
  throw new Error('No safe ordinary one-step movement adjacent to the actor');
}

export async function runNativeResume(options) {
  if (options.realm !== 'isolated') throw new Error('--realm isolated is required');
  const endpoint = validateEndpoint(options.endpoint);
  if (!options.sourceReport || !options.output) throw new Error('--source-report and --output are required');
  const actor = Number(options.actor);
  if (![0, 1].includes(actor)) throw new Error('--actor must explicitly select 0 or 1 from the source report');
  const source = JSON.parse(await fs.readFile(options.sourceReport, 'utf8'));
  if (!source.ok) throw new Error('Native resume requires an already passed ordinary two-account report');
  const { credentials } = await resumeCredentials(options.sourceReport, endpoint);
  const credential = credentials[actor], expectedIndex = source.checks.twoAccounts.evidence[actor]?.characterIndex;
  if (!Number.isInteger(expectedIndex)) throw new Error('Source report lacks the selected character index');
  const output = outsideRepository(options.output);
  await fs.mkdir(output, { recursive: true });
  const runId = `${Date.now()}-${crypto.randomBytes(4).toString('hex')}`, clients = [];
  const report = { schemaVersion: 1, runId, endpoint, sourceRunId: source.runId, actor,
    accountId: credential.accountId, name: credential.name, characterIndex: expectedIndex,
    scope: 'real-public-nativeResumeV1-protocol-with-unclean-socket-loss', startedAt: new Date().toISOString(),
    checks: {}, nativeClientUiAccepted: false, serverRestartResumeTested: false };
  const makeClient = label => {
    const client = new NativeResumeClient(endpoint, label, path.join(output, `${runId}-${label}.trace.jsonl`), [credential.password]);
    clients.push(client); return client;
  };
  let activeCheck = REQUIRED[0];
  const check = (key, evidence) => {
    report.checks[key] = { status: 'passed', evidence };
    activeCheck = REQUIRED[REQUIRED.indexOf(key) + 1] ?? key;
    options.onProgress?.({ check: key, status: 'passed' });
  };
  try {
    const first = makeClient('original');
    await first.connect();
    first.control({ type: 'clientCapabilities', capabilities: [PROTOCOL] });
    const login = await authenticate(first, credential, false);
    if (login.characterIndex !== expectedIndex) throw new Error('Authenticated character differs from the source report');
    check('ordinaryLogin', login);
    await first.wait(() => first.nativeTicket, 'issued native resume credential');
    check('stepBeforeDrop', await ordinaryStep(first, options.mapRoot));
    const before = resumeFingerprint(first.snapshot, credential.name);
    await first.wait(() => first.nativeTicket?.expiresAtMs > Date.now() + 5000, 'live native resume credential');
    const originalTicket = { ...first.nativeTicket };
    await first.loseTransport();
    check('uncleanTransportLoss', { logoutSent: false, closed: first.closed,
      priorGeneration: originalTicket.generation, latestOwnerState: before });
    await sleep(1000); // Allow the closed transport to enter retained-session state.

    const resumed = makeClient('resumed');
    await resumed.connect();
    resumed.control({ type: 'clientCapabilities', capabilities: [PROTOCOL] });
    const resumeAfter = resumed.sequence;
    resumed.control({ type: 'resumeSession', credential: originalTicket.credential });
    const acknowledgement = await resumed.wait(() => {
      const fresh = resumed.events.filter(event => event.sequence > resumeAfter);
      if (fresh.some(event => event.type === 'resumeRejected' || event.type === 'error')) throw new Error('Native resume rejected by the real gateway');
      return fresh.find(event => event.type === 'sessionResumed');
    }, 'native sessionResumed');
    check('nativeResume', { characterIndex: acknowledgement.characterIndex,
      generation: acknowledgement.generation, receipt: acknowledgement.sequence });
    const snapshotEvent = await resumed.wait(() => resumed.events.find(event => event.sequence > acknowledgement.sequence &&
      event.type === 'worldSnapshot' && exposesAuthenticatedGameplay(event)), 'authoritative post-resume snapshot');
    await resumed.wait(() => resumed.nativeTicket, 'rotated native resume credential');
    const after = resumeFingerprint(resumed.snapshot, credential.name);
    assertResumeEvidence({ before, after, characterIndex: expectedIndex, resumed: acknowledgement,
      firstGeneration: originalTicket.generation, rotatedGeneration: resumed.nativeTicket.generation,
      ticketChanged: resumed.nativeTicket.credential !== originalTicket.credential,
      snapshotSequence: snapshotEvent.sequence,
      sentTypes: resumed.events.filter(event => event.direction === 'sent').map(event => event.type) });
    check('sameState', { state: after, snapshotReceipt: snapshotEvent.sequence, actorCopies: 1 });
    check('rotatedTicket', { fromGeneration: originalTicket.generation, toGeneration: resumed.nativeTicket.generation,
      changed: true, receipt: resumed.nativeTicket.sequence });

    const replay = makeClient('replay');
    await replay.connect();
    replay.control({ type: 'clientCapabilities', capabilities: [PROTOCOL] });
    const replayAfter = replay.sequence;
    replay.control({ type: 'resumeSession', credential: originalTicket.credential });
    await replay.wait(() => replay.events.find(event => event.sequence > replayAfter &&
      ['resumeRejected', 'sessionResumed'].includes(event.type)), 'old-ticket replay rejection');
    await replay.refresh();
    check('replayRejected', assertReplayRejected(replay.events.filter(event => event.sequence > replayAfter)));
    await replay.close();
    const direction = self(resumed.snapshot).direction === 'Up' ? 'Right' : 'Up';
    const turn = await resumed.request({ type: 'turn', direction }, 'UserLocation');
    await resumed.refresh();
    const active = resumeFingerprint(resumed.snapshot, credential.name);
    if (active.direction !== direction || active.x !== before.x || active.y !== before.y || active.mapFileName !== before.mapFileName) {
      throw new Error('Resumed owner lost movement authority after the rejected ticket replay');
    }
    check('resumedOwnerLive', { direction, receipt: turn.sequence, actorCopies: 1 });
    const logout = await resumed.logout();
    if (!logout) throw new Error('Resumed actor was not in game for normal logout');
    check('logout', { packet: logout.packet, receipt: logout.sequence });
  } catch (error) {
    report.error = error.message;
    report.checks[activeCheck] ??= { status: 'failed', error: error.message };
  }
  finally {
    for (const client of clients) {
      try { await client.close(); } catch (error) { (report.cleanupErrors ??= []).push(error.message); }
    }
    for (const key of REQUIRED) report.checks[key] ??= { status: 'skipped', reason: 'Earlier prerequisite did not complete' };
    report.ok = !report.error && !report.cleanupErrors?.length && REQUIRED.every(key => report.checks[key].status === 'passed');
    report.finishedAt = new Date().toISOString();
    const safe = redact(report, clients.flatMap(client => client.secrets));
    report.reportPath = path.join(output, `${runId}.report.json`);
    await fs.writeFile(report.reportPath, JSON.stringify(safe, null, 2) + '\n');
  }
  return redact(report, clients.flatMap(client => client.secrets));
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const keys = { '--endpoint': 'endpoint', '--realm': 'realm', '--source-report': 'sourceReport',
    '--actor': 'actor', '--output': 'output', '--map-root': 'mapRoot' };
  try {
    const options = {};
    for (let index = 2; index < process.argv.length; index += 2) {
      const key = keys[process.argv[index]];
      if (!key || !process.argv[index + 1]) throw new Error('Unknown or missing argument; credentials are read only from the source report private file');
      options[key] = process.argv[index + 1];
    }
    const report = await runNativeResume({ ...options, onProgress: status => console.log(JSON.stringify(status)) });
    console.log(JSON.stringify(report, null, 2));
    if (!report.ok) process.exitCode = 1;
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
