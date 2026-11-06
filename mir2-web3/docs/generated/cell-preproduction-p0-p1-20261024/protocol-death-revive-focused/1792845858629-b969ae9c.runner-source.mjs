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
const { loadProtocolCollisionMap, planProtocolNavigation } =
  await import(pathToFileURL(path.join(scripts, 'quest-agent/protocol-navigation.mjs')));
const { prepareOrdinaryExposure, playerDeathEvidence, townReviveEvidence } =
  await import(pathToFileURL(path.join(scripts, 'playtest-lifecycle.mjs')));

const endpoint = 'ws://127.0.0.1:17410/ws';
const sourceReport = path.join(repo, 'mir2-web3/docs/generated/cell-preproduction-p0-p1-20261024/protocol-full-lifecycle/1792844742353-ccf6fb6e.core-projection.report.json');
const output = path.join(repo, 'mir2-web3/docs/generated/cell-preproduction-p0-p1-20261024/protocol-death-revive-focused');
const corridor = [[288, 617], [286, 617], [285, 616], [284, 616], [282, 616],
  [283, 615], [283, 614], [284, 613], [284, 612], [286, 612], [287, 611],
  [289, 611], [291, 611], [292, 611], [293, 610], [294, 610], [294, 609],
  [296, 607], [298, 605], [299, 604], [299, 602], [299, 600]].map(([x, y]) => ({ x, y }));
const runId = Date.now() + '-' + crypto.randomBytes(4).toString('hex');
const { credentials } = await resumeCredentials(sourceReport, endpoint);
const secrets = credentials.map(value => value.password);
const clients = [];
const reportPath = path.join(output, runId + '.report.json');
const report = { schemaVersion: 1, runId, endpoint, sourceReport, startedAt: new Date().toISOString(),
  scope: 'one-bounded-real-ordinary-two-socket-death-and-town-revive-continuation',
  totalBudgetMs: 480000, exposureBudgetMs: 180000, checks: {}, samples: [], provocations: [],
  fullLifecycleAccepted: false, visualAccepted: false, loadAccepted: false,
  routeWitness: 'Correction after preserved blocked interpolation: visit every supplied western-corridor waypoint exactly with live collision-aware ordinary Walk; fresh live cats alone supply attack IDs.',
  predecessorFailures: ['1792845433696-23f84151.report.json', '1792845654430-cb5de189.report.json'], corridor };
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
const obstaclesFor = client => {
  const obstacles = (client.snapshot.entities ?? []).filter(entity => entity.objectId !== self(client)?.objectId && !entity.dead);
  for (const transfer of client.snapshot.mapTransfers ?? []) {
    const bounds = transfer.bounds;
    if (!bounds || String(transfer.mapFileName ?? '0') !== '0') continue;
    for (let y = bounds.minY; y <= bounds.maxY; y++) for (let x = bounds.minX; x <= bounds.maxX; x++) obstacles.push({ x, y });
  }
  return obstacles;
};
const plan = async (index, target, radius) => planProtocolNavigation({ map: await loadMap('0'),
  start: self(clients[index]), target, desiredDistance: radius, dynamicObstacles: obstaclesFor(clients[index]) });
const navigate = async (index, target, radius, until) => {
  const before = state(clients[index]), cursor = clients[index].sequence;
  target = { x: Number(target.x), y: Number(target.y) };
  let result;
  let initialPlan;
  try {
    initialPlan = await plan(index, target, radius);
    if (!initialPlan) throw new Error('Current public occupancy has no complete planned ordinary segment');
    result = await navigators[index](target, radius,
    () => Date.now() >= until || hasAuthoritativePlayerDeath(clients[index].snapshot), {
      maxSuccessfulSteps: 120, maxAttempts: 180, maxNoPathRefreshes: 2,
      beforeMovement() { if (Date.now() >= until) throw new Error('Bounded ordinary navigation deadline expired'); },
    }); }
  catch (error) {
    (report.navigation ??= []).push({ actor: index, target, radius, before, after: state(clients[index]),
      fromSequence: cursor, throughSequence: clients[index].sequence, error: error.message });
    await checkpoint(); throw error;
  }
  if (!result.reached && !hasAuthoritativePlayerDeath(clients[index].snapshot)) throw new Error('Ordinary route did not reach its target');
  const after = state(clients[index]);
  if (after.map !== '0') throw new Error('Ordinary field route unexpectedly changed map');
  (report.navigation ??= []).push({ actor: index, target, radius, before, after,
    fromSequence: cursor, throughSequence: clients[index].sequence, successfulSteps: result.successfulSteps,
    preflightPath: initialPlan.path, preflightObstacleCount: obstaclesFor(clients[index]).length });
  await checkpoint();
  return result;
};
const liveCats = client => (client.snapshot?.entities ?? []).filter(entity => entity.kind === 'monster' &&
  ['RakingCat', 'HookingCat'].includes(entity.name) && !entity.dead && Number(entity.hp ?? 1) > 0 &&
  Number.isInteger(entity.objectId) && entity.objectId > 0 && distance(entity, { x: 299, y: 600 }) <= 10);

await fs.mkdir(output, { recursive: true });
await fs.copyFile(import.meta.filename, path.join(output, runId + '.runner-source.mjs'));
const sourcePins = {};
for (const file of [import.meta.filename, path.join(scripts, 'playtest-multiplayer-smoke.mjs'),
  path.join(scripts, 'playtest-lifecycle.mjs'), path.join(scripts, 'quest-agent/protocol-play.mjs'),
  path.join(scripts, 'quest-agent/protocol-navigation.mjs'), path.join(scripts, 'quest-agent/protocol-observation.mjs')]) {
  sourcePins[file] = crypto.createHash('sha256').update(await fs.readFile(file)).digest('hex');
}
await fs.writeFile(path.join(output, runId + '.source-pins.json'), JSON.stringify({ capturedAt: new Date().toISOString(), files: sourcePins }, null, 2) + '\n');
await checkpoint();
try {
  const preflight = JSON.parse(await fs.readFile(path.join(output, 'corrected-attempt-preflight.json'), 'utf8'));
  if (!preflight.ok || Date.now() - Date.parse(preflight.checkedAt) > 120000) throw new Error('Fresh read-only login-budget and corridor preflight did not pass');
  report.preflight = preflight;
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
    const until = Math.min(deadline - 200000, Date.now() + 230000);
    // These saved actors already reached the western corridor in the prior run.
    // Waypoints guide the corridor; an occupied tile is never an exact goal.
    const nearest = corridor.map((point, index) => ({ index, distance: distance(self(clients[0]), point) }))
      .sort((a, b) => a.distance - b.distance || b.index - a.index)[0];
    if (nearest.distance > 3) throw new Error('Saved actor is no longer near the witnessed corridor');
    report.resumeCorridorAt = nearest;
    for (const target of corridor.slice(nearest.index + 1)) {
      await Promise.all(clients.map(client => client.refresh()));
      const occupied = obstaclesFor(clients[0]).some(entity => entity.x === target.x && entity.y === target.y);
      const radius = occupied ? 1 : 0;
      const planned = await plan(0, target, radius);
      if (!planned) {
        (report.unavailableWaypoints ??= []).push({ target, radius, owner: state(clients[0]),
          occupied, reason: 'No complete current public-occupancy path; try next corridor hint' });
        await checkpoint(); continue;
      }
      await navigate(0, target, radius, until);
      if (hasAuthoritativePlayerDeath(clients[0].snapshot)) break;
      await navigate(1, self(clients[0]), 5, until);
      console.log(JSON.stringify({ phase: 'witnessed-corridor', waypoint: target, owner: state(clients[0]), observer: state(clients[1]) }));
      if (liveCats(clients[0]).some(entity => distance(self(clients[0]), entity) <= 6)) break;
    }
    await Promise.all(clients.map(client => client.refresh()));
    if (!hasAuthoritativePlayerDeath(clients[0].snapshot) && !liveCats(clients[0]).length) throw new Error('No current live cat observed after available corridor hints');
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
          if (!hasAuthoritativePlayerDeath(clients[0].snapshot) && distance(self(clients[0]), self(clients[1])) > 6) {
            await navigate(1, self(clients[0]), 5, Math.min(until, Date.now() + 15000));
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
