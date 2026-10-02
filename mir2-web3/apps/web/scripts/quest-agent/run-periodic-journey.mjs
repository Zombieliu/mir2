import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { ProtocolClient, delay, startGameBootstrapEvidence } from './protocol-client.mjs';
import { createNavigator, selfPlayer, reviveInTown, collectNearbyGold } from './protocol-play.mjs';
import { createMapTraveler } from './protocol-travel.mjs';
import { loadProtocolCollisionMap } from './protocol-navigation.mjs';
import { completeQuestObjectives, clearTravelBlockingMonster } from './protocol-combat.mjs';
import { prepareLoadout, combatApproachRange, useSupplies, useClassRecovery } from './protocol-loadout.mjs';
import { createPeriodicTaoCombatAction } from './periodic-tao-tactics.mjs';
import { refreshCombatWorldSnapshot } from './protocol-refresh.mjs';
import { hasAuthoritativePlayerDeath } from './protocol-observation.mjs';
import { restockV2Supplies, useTownTeleport, townTeleportCount } from './protocol-supplies.mjs';
import { v2SupplyPolicy, v2SupplyStatus, v2SupplyStock } from './newcomer-v2-supply-policy.mjs';
import { recoverHealthWhileEvading, waitForPassiveHealthRecovery } from './protocol-survival.mjs';
import { interactQuest } from './protocol-quest-actions.mjs';
import { loadCrystalQuestRouteSources } from './route-manifest.mjs';
import {
  PERIODIC_COMMANDS, validateScenario, assertFixtureIdentity, buildPeriodicRoutes, assertPeriodicOffers,
  periodicQuestStates, periodicCreditDeltas, dailyReady, selectPeriodicFarmPlan, periodicRewardPreview,
  playerBalance, validatePeriodicFinish, createPeriodicRedactor, periodicTraceProjection, lostPeriodicTarget, staleObservedPeriodicCorpse, stageName, mapName, sha256,
} from './periodic-journey-policy.mjs';

const CATALOG_URL = new URL('../../../../config/quest-guidance/daily-weekly-v1.json', import.meta.url);
class FarmYield extends Error {}
class PeriodicDeadline extends Error {
  constructor() { super('Configured ordinary scenario deadline reached'); this.reason = 'deadline'; this.code = 'PERIODIC_DEADLINE'; }
}

export function deadlineBoundPeriodicClient(client, deadline, checkDeadline) {
  const timeout = requested => { checkDeadline(); return Math.max(1, Math.min(deadline - Date.now(), Number(requested) > 0 ? Number(requested) : 20_000)); };
  const allowed = command => { checkDeadline(); if (!PERIODIC_COMMANDS.has(command?.type)) throw new Error('Forbidden periodic journey command'); };
  return new Proxy(client, {
    get(target, property, receiver) {
      if (property === 'send') return command => { allowed(command); return target.send(command); };
      if (property === 'request') return (command, packet, requested) => { allowed(command); return target.request(command, packet, timeout(requested)); };
      if (property === 'wait') return (predicate, label, requested) => target.wait(() => { checkDeadline(); return predicate(); }, label, timeout(requested));
      const value = Reflect.get(target, property, receiver);
      return typeof value === 'function' ? value.bind(target) : value;
    },
  });
}

async function healthEvidence(url, declaredSource) {
  const endpoint = new URL(url);
  endpoint.protocol = 'http:'; endpoint.pathname = '/health';
  const observedAt = new Date().toISOString();
  const response = await fetch(endpoint, { signal: AbortSignal.timeout(10_000) });
  if (!response.ok) throw new Error(`QA gateway health HTTP ${response.status}`);
  const health = await response.json();
  if (health.ok !== true || health.ws !== 'ready') throw new Error('QA gateway health is not ready');
  return { url: String(endpoint), observedAt, declaredSource: declaredSource ?? null,
    revision: health.revision ?? null, ok: health.ok, ws: health.ws, http: health.http,
    identityBackend: health.identityBackend ?? health.identity_backend,
    sessionCache: health.sessionCache ?? health.session_cache, capacity: health.capacity,
    gameplayEvents: health.gameplayEvents ?? health.gameplay_events };
}

export async function runPeriodicJourney({ input, output, mapPackRoot }) {
  const scenario = validateScenario(input);
  const catalogBytes = await fs.readFile(CATALOG_URL);
  const catalog = JSON.parse(catalogBytes);
  const sources = await loadCrystalQuestRouteSources();
  const controllerHashes = Object.fromEntries(await Promise.all(['run-periodic-journey.mjs', 'periodic-journey-policy.mjs', 'periodic-tao-tactics.mjs']
    .map(async filename => [filename, sha256(await fs.readFile(new URL(filename, import.meta.url)))])));
  const routes = buildPeriodicRoutes(catalog, scenario.initialLevel, sources);
  const redact = createPeriodicRedactor([scenario.accountId, scenario.password]);
  const outputRoot = path.resolve(output);
  await fs.mkdir(outputRoot, { recursive: true });
  const reportFile = path.join(outputRoot, 'report.json');
  try { await fs.access(reportFile); throw new Error('Evidence directory already has a report; use a fresh output directory'); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
  const startedAtMs = Date.now();
  const deadline = startedAtMs + scenario.timeoutMs;
  const checkDeadline = () => { if (Date.now() >= deadline) throw new PeriodicDeadline(); };
  const sleep = async milliseconds => { checkDeadline(); await delay(Math.min(Number(milliseconds), Math.max(1, deadline - Date.now()))); checkDeadline(); };
  const report = {
    schema: 'mir2-periodic-ordinary-journey/1', scenarioId: scenario.scenarioId, phase: scenario.phase,
    className: scenario.className, fixtureInitialLevel: scenario.initialLevel, fixture: scenario.fixture,
    naturalLevelOneGrowth: false, runtimeFixtureMutation: false, transport: 'ordinary-loopback-websocket',
    endpoint: { gatewayUrl: scenario.gatewayUrl, processHost: 'same host as isolated QA gateway' },
    serverHealthSource: scenario.serverHealthSource ?? null, startedAt: new Date(startedAtMs).toISOString(),
    deadlineAt: new Date(deadline).toISOString(), timeoutMs: scenario.timeoutMs,
    catalogSha256: sha256(catalogBytes), initialQuestIds: routes.map(route => route.questId),
    controllerSourceSha256: controllerHashes,
    importedData: { runtimeProfileId: sources.contentProfile.profileId, runtimeProfileVersion: sources.contentProfile.version,
      respawnGeneratedAt: sources.respawnManifest.generated_at, monsterGeneratedAt: sources.monsterManifest.generated_at },
    dailyIds: routes.filter(route => route.cadence === 'daily').map(route => route.questId),
    weeklyIds: routes.filter(route => route.cadence === 'weekly').map(route => route.questId),
    status: 'running', completed: false, smokePassed: false, timingAccepted: false, visualAccepted: false,
    capacityAcceptance: false, phases: [], taskStages: [], killCredits: [], accepts: [], finishes: [],
    travel: [], combat: [], tactics: [], merchants: [], supplies: [], deaths: [], revivals: [], lostTargets: [], staleObservedCorpses: [], actionReceipts: {}, actionCounts: {},
  };
  const checkpoint = async () => {
    if (!report.finishedAt) report.elapsedMs = Date.now() - startedAtMs;
    await fs.writeFile(`${reportFile}.tmp`, JSON.stringify(redact(report), null, 2));
    await fs.rename(`${reportFile}.tmp`, reportFile);
  };
  let checkpointQueue = Promise.resolve();
  const queueCheckpoint = () => { checkpointQueue = checkpointQueue.then(checkpoint); return checkpointQueue; };
  const client = new ProtocolClient(scenario.gatewayUrl, path.join(outputRoot, 'trace.jsonl'), {
    appendFile: (filename, line) => fs.appendFile(filename, JSON.stringify(redact(periodicTraceProjection(JSON.parse(line)))) + '\n'),
  });
  const bounded = deadlineBoundPeriodicClient(client, deadline, checkDeadline);
  let observed = false;
  let previousStates = null;
  let deathObserved = false;
  const baseRecord = client.record.bind(client);
  client.record = (direction, value) => {
    const event = baseRecord(direction, value);
    if (direction === 'sent') report.actionCounts[value.type] = (report.actionCounts[value.type] ?? 0) + 1;
    if (direction === 'received' && event.packet) {
      report.actionReceipts[event.packet] = (report.actionReceipts[event.packet] ?? 0) + 1;
    }
    if (observed && direction === 'received' && client.snapshot) {
      const states = periodicQuestStates(client.snapshot, routes);
      if (JSON.stringify(states) !== JSON.stringify(previousStates)) {
        report.taskStages.push({ at: event.at, elapsedMs: Date.now() - startedAtMs, sequence: event.sequence,
          source: event.type === 'worldSnapshot' ? 'server-worldSnapshot' : `server-${event.packet ?? event.type}`, states });
        if (previousStates) {
          const credits = periodicCreditDeltas(previousStates, states);
          if (credits.length) report.killCredits.push({ at: event.at, sequence: event.sequence, mapFileName: client.snapshot.mapFileName, credits });
        }
        previousStates = states;
      }
      const dead = hasAuthoritativePlayerDeath(client.snapshot);
      if (dead && !deathObserved) report.deaths.push({ at: event.at, elapsedMs: Date.now() - startedAtMs, sequence: event.sequence,
        mapFileName: client.snapshot.mapFileName, position: { x: selfPlayer(client)?.x, y: selfPlayer(client)?.y }, source: 'authoritative-player-death' });
      deathObserved = dead;
    }
    return event;
  };
  let phase = null;
  const changePhase = label => {
    const now = Date.now();
    if (phase) { phase.finishedAt = new Date(now).toISOString(); phase.elapsedMs = now - Date.parse(phase.startedAt); }
    phase = { phase: label, startedAt: new Date(now).toISOString(), elapsedMs: 0 };
    report.phases.push(phase);
  };
  const stock = () => v2SupplyStock(client.snapshot);
  const operation = async (target, label, action) => {
    checkDeadline();
    const startedAt = new Date().toISOString(), beforeSequence = client.sequence;
    const before = { mapFileName: client.snapshot?.mapFileName, position: { x: selfPlayer(client)?.x, y: selfPlayer(client)?.y }, stock: stock() };
    try {
      const result = await action();
      const receipts = client.events.filter(event => event.sequence > beforeSequence && event.direction === 'received' &&
        ['ObjectAttack', 'ObjectMagic', 'DamageIndicator', 'UseItem', 'GainedItem', 'LoseGold', 'GainExperience', 'MapInformation', 'Revived'].includes(event.packet))
        .map(event => ({ sequence: event.sequence, at: event.at, packet: event.packet, payload: event.payload }));
      target.push({ label, startedAt, elapsedMs: Date.now() - Date.parse(startedAt), beforeSequence, afterSequence: client.sequence,
        before, after: { mapFileName: client.snapshot?.mapFileName, position: { x: selfPlayer(client)?.x, y: selfPlayer(client)?.y }, stock: stock() }, result,
        ...(target !== report.travel ? { actionReceipts: receipts } : {}) });
      return result;
    } catch (error) {
      target.push({ label, startedAt, elapsedMs: Date.now() - Date.parse(startedAt), beforeSequence, afterSequence: client.sequence,
        before, error: { name: error.name, message: error.message, reason: error.reason ?? null } });
      throw error;
    }
  };
  const loadCollisionMap = id => loadProtocolCollisionMap(id, mapPackRoot ? { packagedMapRoot: mapPackRoot } : {});
  const navigate = createNavigator(bounded, { loadCollisionMap, delay: sleep });
  const trackedNavigate = (target, range = 1, stopWhen, options) => operation(report.travel, 'navigate', () => navigate(target, range, stopWhen, options));
  const sustain = async owner => operation(report.supplies, 'held-restoratives', async () => {
    const consumed = await useSupplies(owner, { hpThreshold: 0.75, mpThreshold: 0.4 });
    const healing = await useClassRecovery(owner, { hpThreshold: 0.7, restoreMpIfNeeded: true });
    return { consumed, healing };
  });
  const periodicCombatAction = createPeriodicTaoCombatAction({ checkDeadline,
    onReceipt: receipt => report.tactics.push({ ...receipt, at: new Date().toISOString(), sequence: client.sequence }),
  });
  const combatOptions = {
    action: async (owner, target) => { checkDeadline(); return periodicCombatAction(owner, target); },
    approachRange: combatApproachRange, sustain, sleep, maxAttackAttempts: 120, attackCadenceMs: 650,
    maxEngagements: 200, maxSpawnSearches: 8, retrySpawnSearchTimeout: true,
    allowHistoricalSpawnHints: false, spawnSearchTimeoutMs: 10 * 60_000,
    refreshWhileWaiting: refreshCombatWorldSnapshot,
    recoverAfterUnsafeRetreat: owner => recoverHealthWhileEvading(owner, trackedNavigate, {
      requiredRatio: 0.7, timeoutMs: 45_000, sustain, sleep,
    }),
  };
  const clearBlocker = (owner, target) => operation(report.combat, 'ordinary-observed-blocker', () =>
    clearTravelBlockingMonster(owner, target, trackedNavigate, combatOptions));
  const traveler = createMapTraveler(bounded, trackedNavigate, { loadCollisionMap, resolveBlockingMonster: clearBlocker });
  const travel = (destination, options) => operation(report.travel, `travel-to-${destination}`, () => traveler(destination, options));
  const ensureSupplies = async () => {
    const policy = v2SupplyPolicy(client.snapshot, null, scenario.className);
    // The existing class-aware stock policy is reused without newcomer flags
    // or reward grants. A held/purchased town scroll remains an ordinary item.
    policy.townTeleport = { minimum: 1, target: 2, itemIndex: 719 };
    if (!v2SupplyStatus(client.snapshot, policy).ready) {
      if (mapName(client.snapshot.mapFileName) !== '0') {
        if (townTeleportCount(client.snapshot) > 0) await operation(report.supplies, 'ordinary-town-scroll-resupply', () => useTownTeleport(bounded));
        if (mapName(client.snapshot.mapFileName) !== '0') await travel('0');
      }
      const result = await operation(report.merchants, 'ordinary-minimum-first-purchases', () =>
        restockV2Supplies(bounded, trackedNavigate, { policy, travel, clearBlockingMonster: clearBlocker }));
      if (!['sufficient', 'restocked'].includes(result.status) || !v2SupplyStatus(client.snapshot, policy).ready) {
        const error = new Error(`Ordinary supplies blocked: ${result.reason ?? result.status}`); error.reason = 'suppliesBlocked'; throw error;
      }
    }
    await sustain(bounded);
    if (Number(client.snapshot.playerHp) / Math.max(1, Number(client.snapshot.playerMaxHp)) < 0.35) {
      await operation(report.supplies, 'ordinary-passive-town-recovery', () => waitForPassiveHealthRecovery(bounded, { requiredRatio: 0.65, timeoutMs: 90_000, sleep }));
    }
  };
  const recoverDeath = async () => {
    if (!hasAuthoritativePlayerDeath(client.snapshot)) return false;
    if (report.revivals.length >= scenario.maxRevives) { const error = new Error('Owned scenario revival budget exhausted'); error.reason = 'revivalBudget'; throw error; }
    const result = await operation(report.supplies, 'ordinary-town-revival', () => reviveInTown(bounded));
    if (!result || hasAuthoritativePlayerDeath(client.snapshot)) throw new Error('Town revival lacks authoritative survival proof');
    report.revivals.push(result);
    await queueCheckpoint();
    return true;
  };
  const npcRoute = npc => ({ objectId: npc.objectId, name: npc.name, mapFileName: npc.map, position: { x: npc.x, y: npc.y } });
  let checkpointTimer;
  let logoutAfterSequence = null;
  try {
    changePhase('health-and-login');
    report.serverHealthStart = await healthEvidence(scenario.gatewayUrl, scenario.serverHealthSource);
    if (scenario.expectedServerRevision && report.serverHealthStart.revision !== scenario.expectedServerRevision) throw new Error('QA gateway revision differs from the owned deployment receipt');
    await queueCheckpoint();
    await client.connect();
    const login = await bounded.request({ type: 'login', accountId: scenario.accountId, password: scenario.password }, 'LoginSuccess', 60_000);
    const character = (login.payload?.characters ?? []).find(value => value.name === scenario.name &&
      (scenario.characterIndex == null || Number(value.index) === Number(scenario.characterIndex)));
    if (!character) throw new Error('Owned fixture character is absent from LoginSuccess');
    const afterStart = client.sequence;
    bounded.send({ type: 'startGame', characterIndex: Number(character.index) });
    const bootstrap = await bounded.wait(() => startGameBootstrapEvidence(client, afterStart, scenario.name), 'owned personal bootstrap', 60_000);
    await bounded.wait(() => selfPlayer(client)?.name === scenario.name, 'owned personal worldSnapshot', 60_000);
    report.bootstrap = { source: bootstrap.source, characterIndex: Number(character.index), ...assertFixtureIdentity(client.snapshot, scenario) };
    report.initialBalance = playerBalance(client.snapshot); report.initialStock = stock();
    report.serverOffers = assertPeriodicOffers(client.snapshot, routes);
    previousStates = periodicQuestStates(client.snapshot, routes); observed = true;
    report.taskStages.push({ at: new Date().toISOString(), sequence: client.sequence, source: 'initial-server-worldSnapshot', states: previousStates });
    checkpointTimer = setInterval(() => { queueCheckpoint().catch(error => { client.failure = error; }); }, 15_000);
    const home = catalog.npcs.find(npc => mapName(npc.map) === mapName(client.snapshot.mapFileName)) ?? catalog.npcs[0];
    report.homeNpc = npcRoute(home);
    changePhase('accept-five-at-initial-level');
    if (mapName(client.snapshot.mapFileName) !== mapName(home.map)) await travel(home.map);
    await trackedNavigate(home, 8);
    for (const route of routes) {
      const actionRoute = { ...route, startNpc: npcRoute(home), finishNpc: npcRoute(home) };
      const operation = await interactQuest(bounded, actionRoute, { type: 'accept', verifyDefinition: async (owner, id) => {
        const info = await owner.wait(() => owner.questDefinition(id), `q${id} server definition`, 20_000);
        return periodicRewardPreview(info, route);
      } }, trackedNavigate);
      const preview = periodicRewardPreview(client.questDefinition(route.questId), route);
      report.accepts.push({ questId: route.questId, acceptanceLevel: Number(selfPlayer(client).level), preview, ...operation });
      if (Number(selfPlayer(client).level) !== scenario.initialLevel) throw new Error('Level changed before all five periodic slots were accepted');
      await queueCheckpoint();
    }
    if (report.accepts.length !== 5) throw new Error('Five initial offers were not accepted exactly once');
    changePhase('ordinary-merged-daily-farm');
    report.dailyClockStartedAt = new Date().toISOString();
    let creditedEngagements = 0;
    for (let step = 0; step < 1_000 && !dailyReady(client.snapshot, routes); step += 1) {
      checkDeadline(); await recoverDeath(); await ensureSupplies();
      const plan = selectPeriodicFarmPlan(client.snapshot, routes);
      if (!plan) throw new Error('Active daily objectives have no valid farm plan');
      const beforeSnapshot = { ...client.snapshot, entities: (client.snapshot.entities ?? []).map(entity => ({ ...entity })) };
      const selectedTarget = plan.target ? { ...plan.target } : null, engagementStartedAtMs = Date.now();
      const beforeStates = periodicQuestStates(beforeSnapshot, routes), beforeSequence = client.sequence;
      let engagementResult;
      try {
        engagementResult = await operation(report.combat, `farm-q${plan.route.questId}-${plan.kill.monsterName}`, async () => {
          const loadout = await prepareLoadout(bounded);
          if (loadout.equipped.length || loadout.learned.length || loadout.consumed.length) report.supplies.push({ label: 'ordinary-loadout', sequence: client.sequence, result: loadout });
          if (plan.target) {
            const result = await clearTravelBlockingMonster(bounded, plan.target, trackedNavigate, combatOptions);
            // ObjectDeath alone is not kill credit. Probe and wait for the
            // exact daily ID, which also exposes the ordinary weekly overlap.
            await refreshCombatWorldSnapshot(bounded);
            const lostTarget = lostPeriodicTarget({ beforeSnapshot, afterSnapshot: client.snapshot, target: selectedTarget,
              beforeSequence, engagementStartedAtMs, events: client.events,
              credits: periodicCreditDeltas(beforeStates, periodicQuestStates(client.snapshot, routes)) });
            if (lostTarget) return { ...result, lostTarget };
            await bounded.wait(() => periodicCreditDeltas(beforeStates, periodicQuestStates(client.snapshot, routes))
              .some(credit => credit.cadence === 'daily'), 'authoritative daily kill credit', 6_000);
            return result;
          }
          try {
            return await completeQuestObjectives(bounded, plan.route, trackedNavigate, { ...combatOptions, travel,
              afterEngagement: async () => { checkDeadline(); throw new FarmYield('re-evaluate merged daily targets'); } });
          } catch (error) {
            if (error instanceof FarmYield) return { yieldedAfterEngagement: true };
            if (error.message !== `Timeout waiting for q${plan.route.questId} ${plan.kill.monsterName} kill progress`) throw error;
            // Probe through the ordinary protocol. A probe/network/cooldown
            // failure remains a failure; it cannot become a corpse retry.
            await refreshCombatWorldSnapshot(bounded);
            const staleObservedCorpse = staleObservedPeriodicCorpse({ error, route: plan.route, kill: plan.kill,
              beforeSnapshot, afterSnapshot: client.snapshot, beforeSequence, events: client.events,
              credits: periodicCreditDeltas(beforeStates, periodicQuestStates(client.snapshot, routes)) });
            if (!staleObservedCorpse) throw error;
            return { staleObservedCorpse, originalFailure: { name: error.name, message: error.message } };
          }
        });
      } catch (error) {
        if (hasAuthoritativePlayerDeath(client.snapshot)) { await recoverDeath(); await queueCheckpoint(); continue; }
        throw error;
      }
      const credits = periodicCreditDeltas(beforeStates, periodicQuestStates(client.snapshot, routes));
      if (engagementResult?.lostTarget) {
        report.lostTargets.push({ ...engagementResult.lostTarget, questId: plan.route.questId,
          beforeSequence, afterSequence: client.sequence, at: new Date().toISOString() });
        await queueCheckpoint();
        continue;
      }
      if (engagementResult?.staleObservedCorpse) {
        report.staleObservedCorpses.push({ ...engagementResult.staleObservedCorpse, questId: plan.route.questId,
          beforeSequence, afterSequence: client.sequence, at: new Date().toISOString() });
        await queueCheckpoint();
        continue;
      }
      if (!credits.some(credit => credit.cadence === 'daily')) throw new Error('Daily engagement ended without exact server quest credit');
      creditedEngagements += 1;
      report.combat.at(-1).credits = credits; report.combat.at(-1).creditAfterSequence = beforeSequence;
      await collectNearbyGold(bounded, trackedNavigate);
      await queueCheckpoint();
      if (scenario.phase === 'smoke') {
        if (!credits.some(credit => credit.cadence === 'weekly')) throw new Error('Smoke daily kill did not overlap the accepted weekly slot');
        report.smokePassed = true; report.status = 'smokePassed'; break;
      }
    }
    report.creditedEngagements = creditedEngagements;
    if (scenario.phase === 'measure') {
      if (!dailyReady(client.snapshot, routes)) throw new Error('Daily farm budget ended before three Ready server stages');
      report.dailyReadyAt = new Date().toISOString();
      report.weeklyAtDailyReady = periodicQuestStates(client.snapshot, routes).filter(quest => quest.cadence === 'weekly');
      report.dailyReadyElapsedMs = Date.now() - Date.parse(report.dailyClockStartedAt);
      changePhase('legal-town-return-and-reward');
      await recoverDeath();
      // Finishing at the fixture's home steward preserves one measured round
      // trip; either steward is authorized by the live server dialog.
      if (mapName(client.snapshot.mapFileName) !== mapName(home.map)) await travel(home.map);
      await trackedNavigate(home, 8);
      for (const route of routes.filter(quest => quest.cadence === 'daily')) {
        await refreshCombatWorldSnapshot(bounded);
        const before = playerBalance(client.snapshot), beforeSequence = client.sequence;
        const preview = report.accepts.find(entry => entry.questId === route.questId).preview;
        const operation = await interactQuest(bounded, { ...route, finishNpc: npcRoute(home) }, 'finish', trackedNavigate);
        const changeQuest = await bounded.wait(() => client.events.find(event => event.sequence > beforeSequence &&
          event.direction === 'received' && event.packet === 'ChangeQuest' && Number(event.payload?.questId ?? event.payload?.id) === route.questId &&
          Number(event.payload?.questState ?? event.payload?.state) === 2), `q${route.questId} server removal`, 20_000);
        const after = playerBalance(client.snapshot);
        const receipt = validatePeriodicFinish({ before, after, preview, operation, changeQuest,
          quest: (client.snapshot.questLog ?? []).find(quest => Number(quest.questId) === route.questId), curve: sources.contentProfile.experienceCurve });
        report.finishes.push({ ...receipt, beforeSequence, afterSequence: client.sequence, at: new Date().toISOString() });
        await queueCheckpoint();
      }
      if (report.finishes.length !== 3) throw new Error('Three exact daily rewards were not verified');
      report.completed = true; report.status = 'completed';
    }
    report.finalStates = periodicQuestStates(client.snapshot, routes);
    report.finalBalance = playerBalance(client.snapshot); report.finalStock = stock();
    report.serverHealthEnd = await healthEvidence(scenario.gatewayUrl, scenario.serverHealthSource);
    if (report.serverHealthStart.revision && report.serverHealthEnd.revision !== report.serverHealthStart.revision) {
      throw new Error('QA gateway deployment revision changed during the measured scenario');
    }
    checkDeadline();
  } catch (error) {
    report.status = error.reason === 'deadline' || error.code === 'PERIODIC_DEADLINE' ? 'timeout' : 'blocked';
    report.completed = false; report.smokePassed = false;
    report.error = { name: error.name, code: error.code ?? null, reason: error.reason ?? null, message: error.message };
    if (client.snapshot && report.bootstrap) {
      report.finalStates = periodicQuestStates(client.snapshot, routes); report.finalBalance = playerBalance(client.snapshot); report.finalStock = stock();
    }
  } finally {
    clearInterval(checkpointTimer);
    report.gameplayElapsedMs = Date.now() - startedAtMs;
    report.gameplayDeadlineExceeded = report.gameplayElapsedMs >= scenario.timeoutMs;
    changePhase('normal-logout'); logoutAfterSequence = client.sequence;
    try { await client.close(); } catch (error) { report.traceFailure = { message: error.message }; }
    const logout = client.events.find(event => event.sequence > logoutAfterSequence && event.direction === 'received' && event.packet === 'LogOutSuccess');
    report.logout = { normalRequested: client.events.some(event => event.sequence > logoutAfterSequence && event.direction === 'sent' && event.type === 'logOut'),
      confirmed: Boolean(logout), sequence: logout?.sequence ?? null, at: logout?.at ?? null };
    if (!report.logout.confirmed || report.traceFailure) {
      report.completed = false; report.smokePassed = false;
      if (['completed', 'smokePassed'].includes(report.status)) report.status = 'logoutUnconfirmed';
    }
    const finishedAtMs = Date.now(); phase.finishedAt = new Date(finishedAtMs).toISOString(); phase.elapsedMs = finishedAtMs - Date.parse(phase.startedAt);
    report.finishedAt = new Date(finishedAtMs).toISOString(); report.elapsedMs = finishedAtMs - startedAtMs;
    report.logoutElapsedMs = phase.elapsedMs;
    report.elapsedMinutes = report.elapsedMs / 60_000;
    report.timingTargetMinutes = [30, 60];
    report.timingAccepted = scenario.phase === 'measure' && report.completed && report.elapsedMs >= 30 * 60_000 && report.elapsedMs <= 60 * 60_000;
    report.timingOutcome = !report.completed ? 'incomplete' : report.elapsedMs < 30 * 60_000 ? 'below-target' : report.elapsedMs > 60 * 60_000 ? 'above-target' : 'within-target';
    report.timePaddingMs = 0; report.clockReset = false;
    try { await checkpointQueue; } catch (error) {
      report.checkpointFailure = { message: error.message }; report.completed = false; report.smokePassed = false;
      report.timingAccepted = false; report.status = 'evidenceFailure';
    }
    await checkpoint();
  }
  return redact(report);
}

async function main() {
  const { values } = parseArgs({ options: {
    input: { type: 'string' }, output: { type: 'string' }, phase: { type: 'string' }, help: { type: 'boolean', short: 'h' },
  } });
  if (values.help) {
    console.log('node run-periodic-journey.mjs --input <owned-private-scenario.json> --output <fresh-evidence-dir> --phase smoke|measure\nEnv: MIR2_PERIODIC_INPUT_FILE, MIR2_PERIODIC_OUTPUT, MIR2_PERIODIC_PHASE, MIR2_PERIODIC_TIMEOUT_MS, MIR2_QA_MAP_PACK_ROOT; optional credential overrides MIR2_PERIODIC_ACCOUNT_ID/MIR2_PERIODIC_PASSWORD. Node >=22.18; loopback QA only.');
    return;
  }
  const inputFile = values.input ?? process.env.MIR2_PERIODIC_INPUT_FILE;
  const output = values.output ?? process.env.MIR2_PERIODIC_OUTPUT;
  if (!inputFile || !output) throw new Error('Owned private input and fresh evidence output are required');
  const source = JSON.parse(await fs.readFile(inputFile, 'utf8'));
  const input = { ...source, accountId: process.env.MIR2_PERIODIC_ACCOUNT_ID ?? source.accountId,
    password: process.env.MIR2_PERIODIC_PASSWORD ?? source.password };
  const scenario = validateScenario(input, {
    phase: values.phase ?? process.env.MIR2_PERIODIC_PHASE ?? input.phase,
    timeoutMs: process.env.MIR2_PERIODIC_TIMEOUT_MS ?? input.timeoutMs,
  });
  const report = await runPeriodicJourney({ input: scenario, output, mapPackRoot: process.env.MIR2_QA_MAP_PACK_ROOT });
  console.log(JSON.stringify({ scenarioId: report.scenarioId, phase: report.phase, status: report.status,
    completed: report.completed, smokePassed: report.smokePassed, timingAccepted: report.timingAccepted,
    elapsedMs: report.elapsedMs, dailyRewards: report.finishes.length, normalLogoutConfirmed: report.logout.confirmed }));
  if (!report.completed && !report.smokePassed) process.exitCode = 1;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(() => { console.error('Periodic QA controller failed before a durable report. Check the owned input schema and fresh output path; private values are not printed.'); process.exitCode = 1; });
}
