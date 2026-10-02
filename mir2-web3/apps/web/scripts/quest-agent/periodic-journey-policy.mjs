import crypto from 'node:crypto';

export const PERIODIC_TIMEOUT_MS = 75 * 60_000;
export const PERIODIC_GATEWAY_URL = 'ws://127.0.0.1:7290/ws';
export const PERIODIC_CLASSES = Object.freeze(['Warrior', 'Wizard', 'Taoist']);
export const PERIODIC_BOUNDARIES = Object.freeze([10, 14, 15, 24, 25, 34, 35, 50]);
export const PERIODIC_COMMANDS = new Set([
  'clientVersion', 'login', 'startGame', 'townRevive', 'keepAlive', 'walk', 'run',
  'turn', 'attack', 'attackDirection', 'magic', 'spellToggle', 'harvest', 'interact',
  'selectNpcDialog', 'acceptQuest', 'finishQuest', 'pickUp', 'pickUpTile',
  'equipItem', 'moveItem', 'useItem', 'buyItem', 'sellItem', 'logOut',
]);

export const stageName = value => String(value ?? '').replace(/[^a-z]/gi, '').toLowerCase();
export const mapName = value => String(value ?? '').trim().replace(/\.map$/i, '').toLowerCase();
const nameKey = value => String(value ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase();
const safeInteger = (value, label, minimum = 0) => {
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number < minimum) throw new Error(`Invalid ${label}`);
  return number;
};
const actor = snapshot => (snapshot?.entities ?? []).find(entity =>
  String(entity?.objectId) === String(snapshot?.playerObjectId));
const distance = (left, right) => Math.max(Math.abs(Number(left.x) - Number(right.x)), Math.abs(Number(left.y) - Number(right.y)));
const possibleOwnerPet = (world, selfId, selfName) => {
  const ownerName = String(selfName ?? '').trim().toLowerCase();
  return Number(world?.petCount ?? world?.playerPetCount ?? 0) > 0 ||
    ['pets', 'ownedPets', 'playerPets'].some(key => Array.isArray(world?.[key]) && world[key].length > 0) ||
    (world?.entities ?? []).some(entity => nameKey(entity.kind).includes('pet') || entity.isPet === true ||
      nameKey(entity.kind) === 'monster' && (
        [entity.ownerObjectId, entity.owner_object_id, entity.petOwnerObjectId, entity.ownerId, entity.masterObjectId]
          .some(id => id != null && Number(id) === selfId) ||
        ownerName && String(entity.ownerName ?? '').trim().toLowerCase() === ownerName));
};

export function validatePeriodicCatalog(catalog) {
  if (catalog?.schema !== 1 || catalog?.profile !== 'daily-weekly-v1' || catalog?.timeZone !== 'UTC+8') {
    throw new Error('Unsupported periodic catalog identity');
  }
  if (!Array.isArray(catalog.quests) || catalog.quests.length !== 20 || catalog.npcs?.length !== 2) {
    throw new Error('Periodic catalog must contain twenty quests and two stewards');
  }
  const ids = new Set();
  for (const quest of catalog.quests) {
    if (ids.has(quest.id)) throw new Error('Duplicate periodic quest id');
    ids.add(safeInteger(quest.id, 'quest id', 1));
    if (!['daily', 'weekly'].includes(quest.cadence)) throw new Error('Unknown periodic cadence');
    safeInteger(quest.slot, 'quest slot');
    safeInteger(quest.gold, 'quest gold', 1);
    if (!Array.isArray(quest.kills) || quest.kills.length === 0) throw new Error('Periodic quest has no kill objective');
    for (const kill of quest.kills) {
      safeInteger(kill.monsterIndex, 'monster index', 1);
      safeInteger(kill.count, 'kill requirement', 1);
      if (!kill.monster || !kill.maps?.length || kill.maps.some(map => !/^[a-z0-9_-]+$/i.test(mapName(map)))) {
        throw new Error('Invalid periodic monster/map identity');
      }
    }
  }
  if (Array.from({ length: 20 }, (_, index) => 92001 + index).some(id => !ids.has(id))) {
    throw new Error('Periodic quest IDs must be the authored 92001..92020 catalog');
  }
  for (const [minimum, maximum] of [[10, 14], [15, 24], [25, 34], [35, 50]]) {
    const tier = catalog.quests.filter(quest => quest.minLevel === minimum && quest.maxLevel === maximum);
    for (const [cadence, slots] of [['daily', [0, 1, 2]], ['weekly', [0, 1]]]) {
      const actual = tier.filter(quest => quest.cadence === cadence).map(quest => quest.slot).sort();
      if (JSON.stringify(actual) !== JSON.stringify(slots)) throw new Error('Periodic tier slot identity mismatch');
    }
  }
  return catalog;
}

export function periodicTier(catalog, initialLevel) {
  validatePeriodicCatalog(catalog);
  const level = safeInteger(initialLevel, 'initial level', 1);
  const result = catalog.quests.filter(quest => level >= quest.minLevel && level <= quest.maxLevel);
  if (result.length !== 5) throw new Error(`No five-slot periodic tier for level ${level}`);
  return result.sort((left, right) => (left.cadence === right.cadence ? left.slot - right.slot : left.cadence === 'daily' ? -1 : 1));
}

export function validateScenario(input, overrides = {}) {
  const phase = String(overrides.phase ?? input?.phase ?? 'measure');
  if (!['smoke', 'measure'].includes(phase)) throw new Error('Phase must be smoke or measure');
  const scenarioId = String(input?.scenarioId ?? '');
  if (!/^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$/.test(scenarioId)) throw new Error('Invalid public scenario alias');
  if (!PERIODIC_CLASSES.includes(input?.className)) throw new Error('Invalid fixture class');
  const initialLevel = safeInteger(input?.initialLevel, 'fixture level', 1);
  if (!PERIODIC_BOUNDARIES.includes(initialLevel)) throw new Error('Fixture must use a tier boundary level');
  if (![input?.accountId, input?.password, input?.name].every(value => typeof value === 'string' && value.length > 0)) {
    throw new Error('Owned fixture login and character identity are required');
  }
  if (input.fixture?.kind !== 'offline-owned-level-equipment' || !input.fixture?.source ||
      !/^[a-f0-9]{64}$/i.test(input.fixture?.sha256 ?? '')) throw new Error('Owned offline fixture provenance is required');
  const gatewayUrl = String(overrides.gatewayUrl ?? input.gatewayUrl ?? PERIODIC_GATEWAY_URL);
  const endpoint = new URL(gatewayUrl);
  if (endpoint.protocol !== 'ws:' || !['127.0.0.1', 'localhost', '[::1]'].includes(endpoint.hostname) ||
      endpoint.username || endpoint.password || endpoint.search || endpoint.hash) throw new Error('Periodic journey requires a plain loopback WebSocket endpoint');
  const timeoutMs = safeInteger(overrides.timeoutMs ?? input.timeoutMs ?? PERIODIC_TIMEOUT_MS, 'scenario timeout', 1);
  const maxRevives = safeInteger(input.maxRevives ?? 3, 'revival budget');
  return { ...input, scenarioId, initialLevel, phase, gatewayUrl, timeoutMs, maxRevives };
}

export function assertFixtureIdentity(snapshot, input) {
  const self = actor(snapshot);
  if (!self || self.name !== input.name || self.class !== input.className || Number(self.level) !== input.initialLevel) {
    throw new Error('Authoritative character does not match the owned class/level fixture');
  }
  return { objectId: Number(self.objectId), className: self.class, initialLevel: Number(self.level), mapFileName: String(snapshot.mapFileName), position: { x: Number(self.x), y: Number(self.y) } };
}

/** Sources are imported ordinary respawns, never the UI's suggested marker. */
export function buildPeriodicRoutes(catalog, level, sources) {
  const quests = periodicTier(catalog, level);
  const monsters = sources?.monsterManifest?.monsters ?? [];
  const maps = sources?.respawnManifest?.maps ?? [];
  const allowedMaps = new Set((sources?.contentProfile?.mapWhitelist ?? []).map(map => mapName(map.fileName)));
  return quests.map(quest => ({
    ...quest, questId: quest.id,
    objectiveMaps: [...new Set(quest.kills.flatMap(kill => kill.maps))],
    objectives: { item: [], kill: quest.kills.map(kill => {
      const matching = monsters.filter(monster => nameKey(monster.name) === nameKey(kill.monster));
      if (matching.length !== 1 || Number(matching[0].monster_index) !== kill.monsterIndex) {
        throw new Error(`q${quest.id} monster identity is not uniquely imported`);
      }
      const spawnCandidates = maps.filter(map => kill.maps.some(allowed => mapName(allowed) === mapName(map.map_file_name)))
        .flatMap(map => {
          if (allowedMaps.size && !allowedMaps.has(mapName(map.map_file_name))) throw new Error(`q${quest.id} map is outside the runtime profile`);
          return (map.respawns ?? []).filter(spawn => Number(spawn.monster_index) === kill.monsterIndex &&
            nameKey(spawn.monster_name) === nameKey(kill.monster) && Number(spawn.count) > 0)
            .map(spawn => ({ mapFileName: String(map.map_file_name), mapTitle: map.map_title,
              position: { x: Number(spawn.location.x), y: Number(spawn.location.y) },
              count: Number(spawn.count), spread: Number(spawn.spread), delayMinutes: Number(spawn.delay_minutes),
              respawnIndex: Number(spawn.respawn_index), monsterIndex: kill.monsterIndex, monsterName: kill.monster,
              isBoss: matching[0].is_boss === true, source: 'crystal-respawn-manifest',
            }));
        });
      if (!spawnCandidates.length) throw new Error(`q${quest.id} has no ordinary ${kill.monster} source on an allowed map`);
      return { monsterName: kill.monster, monsterIndex: kill.monsterIndex, count: kill.count, allowedMaps: kill.maps.map(mapName), spawnCandidates };
    }) },
  }));
}

export function assertPeriodicOffers(snapshot, routes) {
  for (const route of routes) {
    const rows = (snapshot?.questLog ?? []).filter(quest => Number(quest.questId) === route.questId);
    if (rows.length !== 1 || stageName(rows[0].stage) !== 'available') throw new Error(`q${route.questId} is not a unique fresh server offer`);
  }
  return routes.map(route => ({ questId: route.questId, cadence: route.cadence, slot: route.slot, stage: 'Available' }));
}

export function periodicQuestStates(snapshot, routes) {
  return routes.map(route => {
    const row = (snapshot?.questLog ?? []).find(quest => Number(quest.questId) === route.questId);
    return { questId: route.questId, cadence: route.cadence, slot: route.slot, stage: String(row?.stage ?? 'Absent'),
      objectives: route.objectives.kill.map((kill, index) => {
        const objective = (row?.objectives ?? []).find(entry => nameKey(entry.label).includes(nameKey(kill.monsterName))) ?? row?.objectives?.[index];
        const explicitIndex = objective?.monsterIndex ?? objective?.monster_index;
        if (explicitIndex != null && Number(explicitIndex) !== kill.monsterIndex) throw new Error(`q${route.questId} server monster index mismatch`);
        const current = safeInteger(objective?.current ?? (route.objectives.kill.length === 1 ? row?.current : 0) ?? 0, 'server kill progress');
        const required = Number(objective?.required ?? (route.objectives.kill.length === 1 ? row?.required : kill.count) ?? kill.count);
        if (required !== kill.count || current > required) throw new Error(`q${route.questId} server objective identity/count mismatch`);
        if (['inprogress', 'readytoturnin'].includes(stageName(row?.stage)) && !objective && row?.current == null) {
          throw new Error(`q${route.questId} active server objective is absent`);
        }
        return { monsterName: kill.monsterName, monsterIndex: kill.monsterIndex, current, required };
      }) };
  });
}

export function periodicCreditDeltas(previous, current) {
  const before = new Map(previous.map(quest => [quest.questId, quest]));
  return current.flatMap(quest => quest.objectives.flatMap(objective => {
    const old = before.get(quest.questId)?.objectives.find(entry => entry.monsterIndex === objective.monsterIndex);
    const delta = objective.current - Number(old?.current ?? 0);
    if (delta < 0 && stageName(quest.stage) !== 'available') throw new Error(`q${quest.questId} server kill credit regressed`);
    return delta > 0 ? [{ questId: quest.questId, cadence: quest.cadence, monsterIndex: objective.monsterIndex,
      monsterName: objective.monsterName, before: Number(old?.current ?? 0), current: objective.current, delta }] : [];
  }));
}

export function dailyReady(snapshot, routes) {
  return periodicQuestStates(snapshot, routes).filter(quest => quest.cadence === 'daily').every(quest =>
    ['readytoturnin', 'completed'].includes(stageName(quest.stage)) && quest.objectives.every(objective => objective.current === objective.required));
}

export function monsterMatchesPeriodicKill(entity, kill, currentMap) {
  if (nameKey(entity?.kind) !== 'monster' || entity.dead === true || (entity.hp != null && Number(entity.hp) <= 0) ||
      !Number.isSafeInteger(Number(entity.objectId)) || nameKey(entity.name) !== nameKey(kill.monsterName) ||
      !kill.allowedMaps.includes(mapName(currentMap))) return false;
  // Native ObjectMonster has a name/image, not a MonsterInfo index. A route
  // may use its unique imported name, but a supplied index must agree exactly.
  const index = entity.monsterIndex ?? entity.monster_index;
  return index == null || Number(index) === kill.monsterIndex;
}

/** Re-evaluate all daily objectives after each credited engagement. */
export function selectPeriodicFarmPlan(snapshot, routes) {
  const states = new Map(periodicQuestStates(snapshot, routes).map(quest => [quest.questId, quest]));
  const self = actor(snapshot);
  if (!self || !Number.isFinite(Number(self.x)) || !Number.isFinite(Number(self.y))) throw new Error('No authoritative farm position');
  const candidates = routes.filter(route => route.cadence === 'daily' && stageName(states.get(route.questId)?.stage) === 'inprogress')
    .flatMap(route => route.objectives.kill.flatMap((kill, index) => {
      const state = states.get(route.questId).objectives[index];
      if (state.current >= state.required) return [];
      const target = (snapshot.entities ?? []).filter(entity => monsterMatchesPeriodicKill(entity, kill, snapshot.mapFileName))
        .sort((left, right) => distance(self, left) - distance(self, right) || Number(left.objectId) - Number(right.objectId))[0] ?? null;
      const currentSpawns = kill.spawnCandidates.filter(spawn => mapName(spawn.mapFileName) === mapName(snapshot.mapFileName));
      return [{ route, kill, target, currentMap: currentSpawns.length > 0,
        distance: target ? distance(self, target) : currentSpawns.length ? Math.min(...currentSpawns.map(spawn => distance(self, spawn.position))) : Infinity }];
    }));
  return candidates.sort((left, right) => Number(Boolean(right.target)) - Number(Boolean(left.target)) ||
    Number(right.currentMap) - Number(left.currentMap) || left.distance - right.distance || left.route.slot - right.route.slot)[0] ?? null;
}

/** A shared target can die while this owner is still approaching it. */
export function lostPeriodicTarget({ beforeSnapshot, afterSnapshot, target, beforeSequence, engagementStartedAtMs, events, credits }) {
  const objectId = Number(target?.objectId), selfId = Number(beforeSnapshot?.playerObjectId);
  if (!Number.isSafeInteger(objectId) || objectId <= 0 || !Number.isSafeInteger(selfId) || selfId <= 0 ||
      !Number.isFinite(engagementStartedAtMs) || !Number.isSafeInteger(beforeSequence) || credits?.length ||
      mapName(beforeSnapshot?.mapFileName) !== mapName(afterSnapshot?.mapFileName)) return null;
  const terminal = (afterSnapshot?.entities ?? []).find(entity => Number(entity.objectId) === objectId);
  if (nameKey(target.kind) !== 'monster' || target.dead === true || Number(target.hp) <= 0 ||
      !terminal || !(terminal.dead === true || terminal.hp != null && Number(terminal.hp) <= 0)) return null;
  if (beforeSnapshot.playerExperience !== afterSnapshot.playerExperience ||
      actor(beforeSnapshot)?.level !== actor(afterSnapshot)?.level) return null;
  const recent = (events ?? []).filter(event => Number(event.sequence) > beforeSequence);
  const worlds = [beforeSnapshot, afterSnapshot, ...recent.filter(event => event.direction === 'received' && event.type === 'worldSnapshot').map(event => event.payload)];
  if (worlds.some(world => possibleOwnerPet(world, selfId, actor(beforeSnapshot)?.name))) return null;
  const attempted = recent.some(event =>
    (event.direction === 'sent' && (['attack', 'attackDirection', 'magic'].includes(event.type) || /pet/i.test(event.type ?? '')) ||
     event.direction === 'received' && (
       ['ObjectAttack', 'ObjectMagic'].includes(event.packet) && Number(event.payload?.objectId) === selfId ||
       event.packet === 'Magic' && event.payload?.cast === true ||
       event.packet === 'ObjectStruck' && Number(event.payload?.attackerId) === selfId ||
       event.packet === 'GainExperience' && Number(event.payload?.amount) > 0 ||
       event.packet === 'ObjectMonster' && Number(event.payload?.masterObjectId) === selfId)));
  if (attempted) return null;
  const players = worlds.flatMap(world => world.entities ?? []).filter(entity =>
    nameKey(entity.kind) === 'player' && PERIODIC_CLASSES.includes(entity.class) && Number(entity.objectId) !== selfId);
  // A cast can already be in flight when the target is selected. Retain a
  // bounded recent observation, including the 11 ms race in the live trace.
  const foreign = (events ?? []).find(event => event.direction === 'received' &&
    ['ObjectAttack', 'ObjectMagic'].includes(event.packet) && event.payload?.cast !== false &&
    Number(event.payload?.targetId) === objectId &&
    Date.parse(event.at) >= engagementStartedAtMs - 10_000 &&
    players.some(player => Number(player.objectId) === Number(event.payload?.objectId)));
  if (foreign) return { reason: 'unattacked-target-killed-by-other-player', objectId,
    foreignPlayerObjectId: Number(foreign.payload.objectId), foreignAttackSequence: foreign.sequence,
    foreignAttackPacket: foreign.packet, foreignAttackAt: foreign.at,
    terminal: { hp: terminal.hp, dead: terminal.dead }, ownCreditGranted: false };
  // Crystal melee ObjectAttack has no targetId. Require a victim-bound strike
  // by a known foreign player, subsequent matching death and fresh corpse.
  const struck = recent.findLast(event => event.direction === 'received' && event.packet === 'ObjectStruck' &&
    Number(event.payload?.objectId) === objectId && Date.parse(event.at) >= engagementStartedAtMs &&
    players.some(player => Number(player.objectId) === Number(event.payload?.attackerId)));
  if (!struck) return null;
  const death = recent.findLast(event => event.direction === 'received' && event.packet === 'ObjectDied' &&
    Number(event.payload?.objectId) === objectId && Number(event.sequence) > Number(struck.sequence) &&
    Date.parse(event.at) >= Date.parse(struck.at));
  if (!death) return null;
  const snapshot = recent.findLast(event => event.direction === 'received' && event.type === 'worldSnapshot' &&
    Number(event.sequence) > Number(death.sequence) && Date.parse(event.at) >= Date.parse(death.at) &&
    mapName(event.payload?.mapFileName) === mapName(beforeSnapshot.mapFileName) &&
    (event.payload?.entities ?? []).some(entity => Number(entity.objectId) === objectId &&
      (entity.dead === true || entity.hp != null && Number(entity.hp) <= 0)));
  if (!snapshot) return null;
  return { reason: 'unattacked-target-killed-by-other-player', objectId,
    foreignPlayerObjectId: Number(struck.payload.attackerId), foreignAttackSequence: struck.sequence,
    foreignAttackPacket: struck.packet, foreignAttackAt: struck.at,
    deathObservationSequence: death.sequence, snapshotSequence: snapshot.sequence,
    terminal: { hp: terminal.hp, dead: terminal.dead }, ownCreditGranted: false };
}

/** A spawn search can briefly re-observe an old corpse as an alive packet. */
export function staleObservedPeriodicCorpse({ error, route, kill, beforeSnapshot, afterSnapshot, beforeSequence, events, credits }) {
  if (error?.name !== 'Error' || error?.code != null || error?.reason != null ||
      !Number.isSafeInteger(route?.questId) || !kill?.monsterName || !Array.isArray(kill.allowedMaps) ||
      error.message !== `Timeout waiting for q${route?.questId} ${kill?.monsterName} kill progress` ||
      !route?.objectives?.kill?.some(entry => entry.monsterIndex === kill?.monsterIndex && entry.monsterName === kill?.monsterName) ||
      !Number.isSafeInteger(beforeSequence) || beforeSequence < 0 || credits?.length) return null;
  const currentMap = mapName(beforeSnapshot?.mapFileName), selfId = Number(beforeSnapshot?.playerObjectId);
  if (!currentMap || currentMap !== mapName(afterSnapshot?.mapFileName) || !kill.allowedMaps.includes(currentMap) ||
      !Number.isSafeInteger(selfId) || selfId <= 0 || Number(afterSnapshot?.playerObjectId) !== selfId ||
      !actor(beforeSnapshot) || !actor(afterSnapshot) ||
      !Number.isSafeInteger(beforeSnapshot.playerExperience) || beforeSnapshot.playerExperience < 0 ||
      !Number.isSafeInteger(Number(actor(beforeSnapshot).level)) || Number(actor(beforeSnapshot).level) <= 0 ||
      beforeSnapshot.playerExperience !== afterSnapshot.playerExperience || actor(beforeSnapshot).level !== actor(afterSnapshot).level) return null;
  const recent = (events ?? []).filter(event => Number(event.sequence) > beforeSequence);
  const worlds = [beforeSnapshot, afterSnapshot, ...recent.filter(event => event.type === 'worldSnapshot' && event.direction === 'received').map(event => event.payload)];
  if (worlds.some(world => possibleOwnerPet(world, selfId, actor(beforeSnapshot)?.name))) return null;
  if (recent.some(event => event.direction === 'sent' &&
      (['attack', 'attackDirection', 'magic'].includes(event.type) || /pet/i.test(event.type ?? '')) ||
      event.direction === 'received' && (
        ['ObjectAttack', 'ObjectMagic'].includes(event.packet) && Number(event.payload?.objectId) === selfId ||
        event.packet === 'Magic' && event.payload?.cast === true ||
        event.packet === 'ObjectStruck' && Number(event.payload?.attackerId) === selfId ||
        event.packet === 'GainExperience' && Number(event.payload?.amount) > 0 ||
        event.packet === 'ObjectMonster' && Number(event.payload?.masterObjectId) === selfId))) return null;
  const observed = new Map();
  for (const event of recent) {
    const value = event.payload;
    if (event.direction === 'received' && event.packet === 'ObjectMonster' && value?.dead === false &&
        nameKey(value.name) === nameKey(kill.monsterName) && Number.isSafeInteger(Number(value.objectId)) && Number(value.objectId) > 0) {
      observed.set(Number(value.objectId), event);
    }
  }
  const proofs = [];
  for (const [objectId, alive] of observed) {
    const terminal = (afterSnapshot.entities ?? []).find(entity => Number(entity.objectId) === objectId);
    const isCorpse = entity => entity && nameKey(entity.kind) === 'monster' && nameKey(entity.name) === nameKey(kill.monsterName) &&
      (entity.monsterIndex == null && entity.monster_index == null || Number(entity.monsterIndex ?? entity.monster_index) === kill.monsterIndex) &&
      (entity.dead === true || entity.hp != null && Number(entity.hp) <= 0);
    if (!isCorpse(terminal)) continue;
    const snapshot = recent.findLast(event => event.direction === 'received' && event.type === 'worldSnapshot' &&
      Number(event.sequence) > Number(alive.sequence) && mapName(event.payload?.mapFileName) === currentMap &&
      (event.payload?.entities ?? []).some(entity => Number(entity.objectId) === objectId && isCorpse(entity)));
    if (!snapshot) continue;
    const death = recent.findLast(event => event.direction === 'received' && Number(event.payload?.objectId) === objectId &&
      (event.packet === 'ObjectDied' || event.packet === 'ObjectMonster' && event.payload?.dead === true ||
       event.packet === 'ObjectHealth' && Number(event.payload?.percent) === 0));
    const priorDeath = (events ?? []).findLast(event => Number(event.sequence) <= beforeSequence && event.direction === 'received' &&
      event.packet === 'ObjectDied' && Number(event.payload?.objectId) === objectId);
    proofs.push({ reason: 'unattacked-authoritative-stale-corpse', objectId, monsterName: kill.monsterName,
      observedAliveSequence: alive.sequence, deathObservationSequence: death?.sequence ?? snapshot.sequence,
      priorDeathSequence: priorDeath?.sequence ?? null, snapshotSequence: snapshot.sequence,
      terminal: { hp: terminal.hp, dead: terminal.dead }, ownCreditGranted: false });
  }
  // The shared helper does not export its chosen target. Require one uniquely
  // observed corpse instead of guessing among several possible targets.
  return proofs.length === 1 ? proofs[0] : null;
}

export function periodicRewardPreview(info, route) {
  if (Number(info?.index) !== route.questId || Number(info?.min_level_needed) !== route.minLevel ||
      Number(info?.max_level_needed) !== route.maxLevel || Number(info?.reward_gold) !== route.gold) {
    throw new Error(`q${route.questId} server definition does not match the periodic offer`);
  }
  const experience = safeInteger(info.reward_exp, 'server reward experience', 1);
  const tasks = info.task_description ?? [];
  for (const kill of route.objectives.kill) {
    if (!tasks.some(line => nameKey(line).includes(nameKey(kill.monsterName)) && new RegExp(`\\b${kill.count}\\b`).test(line))) {
      throw new Error(`q${route.questId} server task definition differs from the imported route`);
    }
  }
  return { questId: route.questId, gold: route.gold, experience, minLevel: route.minLevel, maxLevel: route.maxLevel };
}

export function playerBalance(snapshot) {
  const self = actor(snapshot);
  return { level: safeInteger(self?.level, 'balance level', 1), experience: safeInteger(snapshot?.playerExperience, 'balance experience'), gold: safeInteger(snapshot?.gold, 'balance gold') };
}

export function experienceGain(before, after, curve) {
  if (after.level < before.level) throw new Error('Reward lowered the authoritative level');
  const thresholds = new Map(curve.map(entry => [Number(entry.level), Number(entry.requiredExperience)]));
  let result = after.experience - before.experience;
  for (let level = before.level; level < after.level; level += 1) {
    const threshold = thresholds.get(level);
    if (!Number.isSafeInteger(threshold) || threshold <= 0) throw new Error('Reward crossed an unsupported experience threshold');
    result += threshold;
  }
  return result;
}

export function validatePeriodicFinish({ before, after, preview, operation, changeQuest, quest, curve }) {
  const ack = operation?.acknowledgement;
  if (operation?.command?.type !== 'finishQuest' || ack?.operation !== 'finishQuest' || ack?.success !== true ||
      ack.requestId !== operation.command.requestId || Number(ack.questIndex) !== preview.questId || Number(ack.selectedItemIndex) !== -1) {
    throw new Error(`q${preview.questId} lacks a request-bound successful finish acknowledgement`);
  }
  const payload = changeQuest?.payload;
  if (changeQuest?.packet !== 'ChangeQuest' || Number(payload?.questId ?? payload?.id) !== preview.questId ||
      Number(payload?.questState ?? payload?.state) !== 2 || payload?.taken !== false || payload?.completed !== true ||
      stageName(quest?.stage) !== 'completed') throw new Error(`q${preview.questId} lacks the server removal/completion receipt`);
  const goldDelta = after.gold - before.gold;
  const experienceDelta = experienceGain(before, after, curve);
  if (goldDelta !== preview.gold || experienceDelta !== preview.experience) throw new Error(`q${preview.questId} reward balance delta differs from its locked preview`);
  return { questId: preview.questId, acknowledgement: ack, removal: payload, before, after, goldDelta, experienceDelta, verified: true };
}

export function createPeriodicRedactor(secretValues = []) {
  const secrets = [...new Set(secretValues.filter(value => typeof value === 'string' && value.length > 0))].sort((left, right) => right.length - left.length);
  const sensitiveKey = /^(?:account(?:_?id)?|password|passphrase|token|accessToken|refreshToken|secret(?:Answer)?|secret_answer|credential(?:s)?|authorization)$/i;
  const redact = value => {
    if (typeof value === 'string') return secrets.reduce((text, secret) => text.split(secret).join('[redacted]'), value);
    if (Array.isArray(value)) return value.map(redact);
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, nested]) =>
      [key, sensitiveKey.test(key) ? '[redacted]' : redact(nested)]));
    return value;
  };
  return redact;
}

export function periodicTraceProjection(event) {
  if (event.type !== 'worldSnapshot') return event;
  const snapshot = event.payload ?? {};
  // Avoid multi-gigabyte passive snapshots while retaining receipt, balance,
  // task, live actor and carried-stock evidence. The in-memory client remains
  // untouched, including all item tooltip requirements used by combat.
  const itemFields = items => (items ?? []).map(item => ({ key: item.key, name: item.name, itemIndex: item.itemIndex ?? item.item_index,
    uniqueId: item.uniqueId, slot: item.slot, container: item.container, quantity: item.quantity, equipSlot: item.equipSlot }));
  return { ...event, traceProjection: 'periodic-receipts/1', payload: {
    mapFileName: snapshot.mapFileName, mapIndex: snapshot.mapIndex, mapTitle: snapshot.mapTitle,
    playerObjectId: snapshot.playerObjectId, playerHp: snapshot.playerHp, playerMaxHp: snapshot.playerMaxHp,
    playerMp: snapshot.playerMp, playerMaxMp: snapshot.playerMaxMp, playerExperience: snapshot.playerExperience,
    gold: snapshot.gold, inSafeZone: snapshot.inSafeZone, mapSnapshotPending: snapshot.mapSnapshotPending,
    entities: snapshot.entities, questLog: (snapshot.questLog ?? []).filter(quest => Number(quest.questId) >= 92001 && Number(quest.questId) <= 92020),
    questOperationAck: snapshot.questOperationAck, activeNpcDialog: snapshot.activeNpcDialog,
    inventoryItems: itemFields(snapshot.inventoryItems), beltItems: itemFields(snapshot.beltItems), equipmentItems: itemFields(snapshot.equipmentItems),
    knownSkills: snapshot.knownSkills, mapTransfers: snapshot.mapTransfers,
  } };
}

export const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
