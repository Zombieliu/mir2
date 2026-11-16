import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';

const repo = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey';
const scripts = path.join(repo, 'mir2-web3/apps/web/scripts');
const { PlaytestClient, authenticate, resumeCredentials, observedPlayerId, redact } =
  await import(pathToFileURL(path.join(scripts, 'playtest-multiplayer-smoke.mjs')));
const { createNavigator } = await import(pathToFileURL(path.join(scripts, 'quest-agent/protocol-play.mjs')));
const { hasAuthoritativePlayerDeath, observedPlayerHp } =
  await import(pathToFileURL(path.join(scripts, 'quest-agent/protocol-observation.mjs')));
const { loadProtocolCollisionMap } =
  await import(pathToFileURL(path.join(scripts, 'quest-agent/protocol-navigation.mjs')));
const { prepareOrdinaryExposure, playerDeathEvidence, townReviveEvidence } =
  await import(pathToFileURL(path.join(scripts, 'playtest-lifecycle.mjs')));

const endpoint = 'ws://127.0.0.1:17410/ws';
const sourceReport = path.join(repo, 'mir2-web3/docs/generated/cell-preproduction-p0-p1-20261024/protocol-full-lifecycle/1792844742353-ccf6fb6e.core-projection.report.json');
const output = path.join(repo, 'mir2-web3/docs/generated/cell-preproduction-p0-p1-20261024/protocol-death-revive-focused');
const runId = Date.now() + '-' + crypto.randomBytes(4).toString('hex');
const { credentials } = await resumeCredentials(sourceReport, endpoint);
const secrets = credentials.map(value => value.password);
const clients = [];
const reportPath = path.join(output, runId + '.report.json');
const report = { schemaVersion: 1, runId, endpoint, sourceReport, startedAt: new Date().toISOString(),
  scope: 'one-bounded-real-ordinary-two-socket-death-and-town-revive-continuation',
  totalBudgetMs: 480000, exposureBudgetMs: 180000, checks: {}, samples: [], provocations: [],
  fullLifecycleAccepted: false, visualAccepted: false, loadAccepted: false,
  routeWitness: 'Earlier real ordinary map-0 UserLocation trail north toward 299,600; fresh live cats alone supply attack IDs.' };
const deadline = Date.now() + report.totalBudgetMs;
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const self = client => client.snapshot?.entities?.find(value => Number(value.objectId) === Number(client.snapshot.playerObjectId));
const distance = (a, b) => Math.max(Math.abs(Number(a?.x) - Number(b?.x)), Math.abs(Number(a?.y) - Number(b?.y)));
const state = client => ({ name: self(client)?.name, objectId: self(client)?.objectId,
  map: client.snapshot?.mapFileName, x: self(client)?.x, y: self(client)?.y,
  hp: observedPlayerHp(client.snapshot), maxHp: client.snapshot?.playerMaxHp,
  dead: hasAuthoritativePlayerDeath(client.snapshot) });
const fields = ['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases',
  'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight'];
const health = async () => {
  const response = await fetch('http://127.0.0.1:17410/health', { signal: AbortSignal.timeout(3000) });
  if (!response.ok) throw new Error('Owned realm health rejected: ' + response.status);
  return (await response.json()).capacity;
};
const checkpoint = async () => fs.writeFile(reportPath, JSON.stringify(redact(report, secrets), null, 2) + '\n');
const check = async (key, action) => {
  try { report.checks[key] = { status: 'passed', evidence: await action() }; }
  catch (error) { report.checks[key] = { status: 'failed', error: error.message, evidence: error.evidence }; }
  await checkpoint();
  console.log(JSON.stringify({ check: key, status: report.checks[key].status }));
  return report.checks[key].status === 'passed';
};
const maps = new Map();
const loadMap = async name => {
  if (!maps.has(name)) maps.set(name, await loadProtocolCollisionMap(name));
  return maps.get(name);
};
let deathContext, exposure, navigators, activePhase = 'ordinaryTwoLogin';
const navigate = async (index, target, radius, until) => {
  const before = state(clients[index]), cursor = clients[index].sequence;
  const result = await navigators[index](target, radius,
    () => Date.now() >= until || hasAuthoritativePlayerDeath(clients[index].snapshot), {
      maxSuccessfulSteps: 120, maxAttempts: 180, maxNoPathRefreshes: 2,
      beforeMovement() { if (Date.now() >= until) throw new Error('Bounded ordinary navigation deadline expired'); },
    });
  if (!result.reached && !hasAuthoritativePlayerDeath(clients[index].snapshot)) throw new Error('Ordinary route did not reach its target');
  const after = state(clients[index]);
  if (after.map !== '0') throw new Error('Ordinary field route unexpectedly changed map');
  (report.navigation ??= []).push({ actor: index, target, radius, before, after,
    fromSequence: cursor, throughSequence: clients[index].sequence, successfulSteps: result.successfulSteps });
  await checkpoint();
  return result;
};
const liveCats = client => (client.snapshot?.entities ?? []).filter(entity => entity.kind === 'monster' &&
  ['RakingCat', 'HookingCat'].includes(entity.name) && !entity.dead && Number(entity.hp ?? 1) > 0 &&
  Number.isInteger(entity.objectId) && entity.objectId > 0 && distance(entity, { x: 299, y: 600 }) <= 10);

await fs.mkdir(output, { recursive: true });
await checkpoint();
try {
  const baseline = await health();
  if (fields.some(key => baseline[key] !== 0)) throw new Error('Owned isolated realm was not drained before the one focused attempt');
  report.initialCapacity = baseline;
  if (!await check('ordinaryTwoLogin', async () => {
    const actors = [];
    for (let index = 0; index < 2; index++) {
      const client = new PlaytestClient(endpoint, index ? 'observer' : 'victim',
        path.join(output, runId + '-' + (index ? 'observer' : 'victim') + '.trace.jsonl'), secrets);
      clients.push(client); await client.connect(); actors.push(await authenticate(client, credentials[index], false));
    }
    await Promise.all(clients.map(client => client.refresh()));
    if (clients.some(client => client.snapshot.mapFileName !== '0' || hasAuthoritativePlayerDeath(client.snapshot))) {
      throw new Error('Both existing ordinary actors must start alive on witnessed map 0');
    }
    if (!observedPlayerId(clients[1], credentials[0].name) || !observedPlayerId(clients[0], credentials[1].name)) {
      throw new Error('Both ordinary participants do not currently observe one another');
    }
    navigators = clients.map(client => createNavigator(client, { loadCollisionMap: loadMap, movementResponseTimeoutMs: 5000 }));
    return { actors, before: clients.map(state) };
  })) throw new Error('Ordinary paired authentication prerequisite failed');
  activePhase = 'armourRemoval';
  if (!await check('armourRemoval', async () => {
    exposure = await prepareOrdinaryExposure(clients[0], Math.min(deadline, Date.now() + 15000));
    return { ...exposure, owner: state(clients[0]) };
  })) throw new Error('Ordinary armour removal was not confirmed');
  deathContext = { names: clients.map(client => self(client).name), cursors: clients.map(client => client.sequence),
    observerId: observedPlayerId(clients[1], credentials[0].name), mapFileName: '0', initialAlive: true };
  activePhase = 'ordinaryApproach';
  if (!await check('ordinaryApproach', async () => {
    const until = Math.min(deadline - 20000, Date.now() + 150000);
    for (const target of [{ x: 299, y: 616 }, { x: 299, y: 610 }, { x: 298, y: 605 }, { x: 299, y: 602 }, { x: 299, y: 600 }]) {
      await Promise.all(clients.map(client => client.refresh()));
      await navigate(0, target, 1, until);
      if (hasAuthoritativePlayerDeath(clients[0].snapshot)) break;
      await navigate(1, self(clients[0]), 5, until);
      console.log(JSON.stringify({ phase: 'ordinary-walk-north', owner: state(clients[0]), observer: state(clients[1]) }));
    }
    if (!hasAuthoritativePlayerDeath(clients[0].snapshot)) await navigate(1, { x: 299, y: 604 }, 1, until);
    await Promise.all(clients.map(client => client.refresh()));
    return { owners: clients.map(state), currentCats: liveCats(clients[0]).map(entity =>
      ({ objectId: entity.objectId, name: entity.name, x: entity.x, y: entity.y, hp: entity.hp })) };
  })) throw new Error('Witnessed ordinary route did not complete');
  activePhase = 'sharedDeath';
  await check('sharedDeath', async () => {
    const until = Math.min(deadline - 20000, Date.now() + report.exposureBudgetMs), provoked = new Set();
    let lastRefresh = 0;
    while (Date.now() < until) {
      if (clients.some(client => client.failure || client.closed)) throw new Error('Participant transport failed during ordinary exposure');
      const death = playerDeathEvidence(clients, deathContext);
      if (death) return death;
      if (Date.now() - lastRefresh > 2000) {
        await Promise.all(clients.map(client => client.refresh()));
        const cats = liveCats(clients[0]);
        report.samples.push({ at: new Date().toISOString(), owner: state(clients[0]), observer: state(clients[1]),
          cats: cats.map(entity => ({ objectId: entity.objectId, x: entity.x, y: entity.y, distance: distance(self(clients[0]), entity) })) });
        await checkpoint(); lastRefresh = Date.now();
      }
      if (!hasAuthoritativePlayerDeath(clients[0].snapshot)) {
        const current = liveCats(clients[0]).filter(entity => !provoked.has(entity.objectId))
          .sort((a, b) => distance(self(clients[0]), a) - distance(self(clients[0]), b))[0];
        if (current && provoked.size < 4) {
          if (distance(self(clients[0]), current) > 1) {
            try { await navigate(0, current, 1, Math.min(until, Date.now() + 15000)); }
            catch (error) { if (!playerDeathEvidence(clients, deathContext)) throw error; }
          }
          await clients[0].refresh();
          const freshTarget = liveCats(clients[0]).find(entity => entity.objectId === current.objectId &&
            distance(self(clients[0]), entity) <= 1);
          if (freshTarget && !hasAuthoritativePlayerDeath(clients[0].snapshot)) {
            clients[0].send({ type: 'attack', objectId: freshTarget.objectId });
            provoked.add(freshTarget.objectId);
            report.provocations.push({ objectId: freshTarget.objectId, name: freshTarget.name,
              target: { x: freshTarget.x, y: freshTarget.y }, owner: state(clients[0]), commandSequence: clients[0].sequence });
            await checkpoint();
          }
        }
      }
      await pause(500);
    }
    const error = new Error('No paired owner Death and observer ObjectDied within the one bounded ordinary exposure');
    error.evidence = { owners: clients.map(state), provoked: [...provoked] };
    throw error;
  });
  const incoming = clients[0].events.filter(event => event.direction === 'received' &&
    event.sequence > deathContext.cursors[0] && event.packet === 'DamageIndicator' &&
    Number(event.payload?.objectId) === Number(clients[0].snapshot.playerObjectId) && Number(event.payload?.damage) > 0);
  report.incomingDamage = { positiveReceipts: incoming.map(event => ({ sequence: event.sequence, damage: event.payload.damage })),
    total: incoming.reduce((sum, event) => sum + Number(event.payload.damage), 0),
    scope: 'Actual owner incoming damage; public Death does not identify a unique killing source.' };
  activePhase = 'townRevive';
  if (report.checks.sharedDeath.status === 'passed') {
    await check('townRevive', async () => {
      const cursors = clients.map(client => client.sequence);
      const ack = await clients[0].request({ type: 'townRevive' }, 'Revived', 10000);
      await Promise.all(clients.map(client => client.refresh()));
      if (!townReviveEvidence(clients, deathContext, cursors) && !hasAuthoritativePlayerDeath(clients[0].snapshot)) {
        await navigate(1, self(clients[0]), 8, Math.min(deadline, Date.now() + 20000));
        await Promise.all(clients.map(client => client.refresh()));
      }
      const evidence = await clients[0].wait(() => townReviveEvidence(clients, deathContext, cursors),
        'owner Revived, positive HP snapshot and the same observer actor lifecycle', 8000);
      return { ...evidence, ack: ack.sequence, owners: clients.map(state),
        observerActor: clients[1].snapshot.entities.filter(entity => entity.name === credentials[0].name) };
    });
  } else report.checks.townRevive = { status: 'not_run', reason: 'Shared death evidence did not pass' };
} catch (error) {
  report.error = error.message;
  report.checks[activePhase] ??= { status: 'failed', error: error.message };
} finally {
  if (clients[0]?.inGame && hasAuthoritativePlayerDeath(clients[0].snapshot) &&
      clients[0].events.some(event => event.direction === 'received' && event.packet === 'Death')) {
    try {
      const ack = await clients[0].request({ type: 'townRevive' }, 'Revived', 10000);
      await clients[0].refresh();
      report.cleanupTownRevive = { receipt: ack.sequence, owner: state(clients[0]), scope: 'Cleanup only; never promotes missing shared evidence.' };
    } catch (error) { (report.cleanupErrors ??= []).push(error.message); }
  }
  if (exposure?.armourRemoved && clients[0]?.inGame && !hasAuthoritativePlayerDeath(clients[0].snapshot)) {
    try {
      await clients[0].refresh();
      const item = clients[0].snapshot.inventoryItems.find(value => value.uniqueId === exposure.uniqueId);
      if (item) {
        const ack = await clients[0].request({ type: 'equipItem', grid: 'inventory', uniqueId: exposure.uniqueId, to: 1 }, 'EquipItem', 10000);
        if (!ack.payload?.success) throw new Error('Ordinary armour restoration rejected');
        await clients[0].refresh();
        report.armourRestoration = { receipt: ack.sequence, equipped: clients[0].snapshot.equipmentItems.some(value =>
          value.uniqueId === exposure.uniqueId && (value.slot === 'armour' || value.equipSlot === 'armour')) };
      } else report.armourRestoration = { restored: false, reason: 'Original UID is not in current real Bag; no grant or save edit used' };
    } catch (error) { (report.cleanupErrors ??= []).push(error.message); }
  }
  const logouts = [];
  for (const [index, client] of clients.entries()) {
    try {
      const ack = await client.logout(); await client.close();
      logouts.push({ actor: index, receipt: ack?.sequence ?? null });
    } catch (error) { (report.cleanupErrors ??= []).push(error.message); }
  }
  report.checks.logout = { status: !report.cleanupErrors?.length && logouts.length === 2 && logouts.every(value => value.receipt)
    ? 'passed' : 'failed', evidence: logouts };
  const drainUntil = Date.now() + 15000;
  while (Date.now() < drainUntil) {
    const counters = await health();
    if (fields.every(key => counters[key] === 0)) { report.finalCapacity = counters; break; }
    await pause(250);
  }
  report.checks.realmDrain = { status: report.finalCapacity ? 'passed' : 'failed' };
  const required = ['ordinaryTwoLogin', 'armourRemoval', 'ordinaryApproach', 'sharedDeath', 'townRevive', 'logout', 'realmDrain'];
  for (const key of required) report.checks[key] ??= { status: 'not_run', reason: 'An earlier prerequisite did not complete' };
  report.ok = !report.error && !report.cleanupErrors?.length && required.every(key => report.checks[key].status === 'passed');
  report.finishedAt = new Date().toISOString(); await checkpoint();
}
console.log(JSON.stringify({ ok: report.ok, reportPath, checks: Object.fromEntries(Object.entries(report.checks).map(([key, value]) => [key, value.status])) }));
if (!report.ok) process.exitCode = 1;
