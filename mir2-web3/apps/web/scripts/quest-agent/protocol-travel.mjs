import {
  buildMapTravelGraph,
  findMapTravelRoute,
  loadCrystalQuestRouteSources,
} from './route-manifest.mjs';
import {
  findProtocolWalkPath,
  findProtocolTransferExitStep,
  loadProtocolCollisionMap,
} from './protocol-navigation.mjs';

const travelGraphPromise = loadCrystalQuestRouteSources().then(sources => {
  return buildMapTravelGraph(sources);
});

export class TravelInterrupted extends Error {
  constructor(fromMapFileName, toMapFileName) {
    super(`Travel from ${fromMapFileName} to ${toMapFileName} was interrupted by a proven threat`);
    this.name = 'TravelInterrupted';
    this.fromMapFileName = String(fromMapFileName);
    this.toMapFileName = String(toMapFileName);
  }
}

export class TravelBlockedByMonster extends Error {
  constructor(fromMapFileName, toMapFileName, monster, cause) {
    super(
      `Travel from ${fromMapFileName} to ${toMapFileName} is blocked by monster ${monster.objectId}`,
      { cause },
    );
    this.name = 'TravelBlockedByMonster';
    this.fromMapFileName = String(fromMapFileName);
    this.toMapFileName = String(toMapFileName);
    this.objectId = Number(monster.objectId);
    this.target = { x: Number(monster.x), y: Number(monster.y) };
  }
}

function travelShouldInterrupt(options) {
  return typeof options?.interruptWhen === 'function' && options.interruptWhen() === true;
}

function throwIfTravelInterrupted(options, fromMapFileName, toMapFileName) {
  if (travelShouldInterrupt(options)) {
    throw new TravelInterrupted(fromMapFileName, toMapFileName);
  }
}

function mapName(value) {
  const name = typeof value === 'string'
    ? value
    : value?.mapFileName ?? value?.toMapFileName;
  if (name == null || String(name).trim() === '') throw new Error('targetMap must provide mapFileName');
  return String(name);
}

function player(snapshot) {
  const objectId = snapshot?.playerObjectId;
  return (snapshot?.entities ?? []).find(entity =>
    String(entity?.objectId) === String(objectId) ||
    String(entity?.kind ?? '').toLowerCase() === 'selfplayer'
  ) ?? null;
}

function liveTransferCandidates(snapshot, nextMap) {
  return (snapshot?.mapTransfers ?? []).filter(transfer =>
    String(transfer?.mapFileName ?? snapshot?.mapFileName) === String(snapshot?.mapFileName) &&
    String(transfer?.toMapFileName) === nextMap &&
    transfer?.bounds &&
    [transfer.bounds.minX, transfer.bounds.maxX, transfer.bounds.minY, transfer.bounds.maxY]
      .every(Number.isFinite)
  );
}

function nearestPointInBounds(origin, bounds) {
  const clamp = (value, minimum, maximum) => Math.max(minimum, Math.min(maximum, value));
  return {
    x: clamp(Number(origin?.x ?? bounds.minX), Number(bounds.minX), Number(bounds.maxX)),
    y: clamp(Number(origin?.y ?? bounds.minY), Number(bounds.minY), Number(bounds.maxY)),
  };
}

function pointInBounds(point, bounds) {
  return Number(point?.x) >= Number(bounds?.minX) && Number(point?.x) <= Number(bounds?.maxX) &&
    Number(point?.y) >= Number(bounds?.minY) && Number(point?.y) <= Number(bounds?.maxY);
}

function liveDynamicObstacles(snapshot) {
  const self = player(snapshot);
  return (snapshot?.entities ?? []).filter(entity =>
    String(entity?.objectId) !== String(self?.objectId) && !entity?.dead
  );
}

const SABUK_MAP = '3';
const SABUK_SECRET_GATE = 'D701';
const SABUK_INNER_ENTRANCE = Object.freeze({ x: 661, y: 277 });
const SABUK_OUTER_EXIT = Object.freeze({ x: 27, y: 23 });
const BUG_CAVE_CROSSROADS = 'D611';
const BUG_CAVE_DEEP_ROUTE = 'D612';
const BUG_CAVE_COMPONENT_DETOUR = Object.freeze(['D603', 'D608', 'D604', 'D611']);
const BUG_CAVE_ENTRANCE = 'D601';
const BUG_CAVE_PROVINCE = '3';
const BUG_CAVE_RETURN_DETOUR = Object.freeze(['D610', 'D603', 'D611', 'D602', 'D607', 'D601']);

function isInsideSabukMerchantQuarter(snapshot) {
  if (String(snapshot?.mapFileName ?? '') !== SABUK_MAP) return false;
  const actor = player(snapshot);
  return actor != null && Number(actor.x) >= 640 && Number(actor.y) <= 330;
}

function nearestDirectScriptedEdge(graph, current, target, snapshot) {
  const origin = player(snapshot);
  const distance = edge => origin
    ? Math.max(
      Math.abs(Number(origin.x) - Number(edge.npc?.position?.x)),
      Math.abs(Number(origin.y) - Number(edge.npc?.position?.y)),
    )
    : Number.POSITIVE_INFINITY;
  return graph.edges
    .filter(edge => edge.kind === 'npc-script' &&
      String(edge.fromMapFileName) === current &&
      String(edge.toMapFileName) === target)
    .sort((left, right) => distance(left) - distance(right) ||
      String(left.scriptKey).localeCompare(String(right.scriptKey)))[0] ?? null;
}

function chooseLiveTransfer(snapshot, edge, options) {
  return liveTransferChoices(snapshot, edge, options)[0] ?? null;
}

function liveTransferChoices(snapshot, edge, options) {
  const origin = player(snapshot);
  const preferredKey = String(options?.preferredTransferKey ?? '');
  const candidates = liveTransferCandidates(snapshot, String(edge.toMapFileName))
    .filter(transfer => !preferredKey || String(transfer?.key ?? '') === preferredKey);
  const preferredSource = options?.preferredTransferSource;
  return candidates
    .map(transfer => {
      const target = nearestPointInBounds(origin, transfer.bounds);
      const distance = origin
        ? Math.max(Math.abs(Number(origin.x) - target.x), Math.abs(Number(origin.y) - target.y))
        : Number.POSITIVE_INFINITY;
      const preferred = Number.isFinite(Number(preferredSource?.x)) &&
        Number.isFinite(Number(preferredSource?.y)) &&
        pointInBounds(preferredSource, transfer.bounds);
      return { transfer, target, distance, preferred };
    })
    .sort((left, right) => Number(right.preferred) - Number(left.preferred) ||
      left.distance - right.distance ||
      String(left.transfer.key).localeCompare(String(right.transfer.key)));
}

function transferBlockingMonster(snapshot, live, maximumDistance = 6) {
  const bounds = live?.transfer?.bounds;
  if (!bounds) return null;
  const distanceToBounds = entity => Math.max(
    Number(bounds.minX) - Number(entity.x),
    Number(entity.x) - Number(bounds.maxX),
    Number(bounds.minY) - Number(entity.y),
    Number(entity.y) - Number(bounds.maxY),
    0,
  );
  return (snapshot?.entities ?? [])
    .filter(entity =>
      String(entity?.kind ?? '').toLowerCase() === 'monster' &&
      String(entity?.disposition ?? '').toLowerCase() === 'hostile' &&
      entity?.dead !== true && Number(entity?.hp ?? 1) > 0 &&
      Number.isSafeInteger(Number(entity?.objectId)) &&
      Number.isFinite(Number(entity?.x)) && Number.isFinite(Number(entity?.y)) &&
      distanceToBounds(entity) <= maximumDistance)
    .sort((left, right) =>
      distanceToBounds(left) - distanceToBounds(right) ||
      Number(left.objectId) - Number(right.objectId))[0] ?? null;
}

function localBlockingMonster(snapshot, maximumDistance = 6) {
  const origin = player(snapshot);
  if (!origin) return null;
  const distanceToPlayer = entity => Math.max(
    Math.abs(Number(entity.x) - Number(origin.x)),
    Math.abs(Number(entity.y) - Number(origin.y)),
  );
  return (snapshot?.entities ?? [])
    .filter(entity =>
      String(entity?.kind ?? '').toLowerCase() === 'monster' &&
      String(entity?.disposition ?? '').toLowerCase() === 'hostile' &&
      entity?.dead !== true && Number(entity?.hp ?? 1) > 0 &&
      Number.isSafeInteger(Number(entity?.objectId)) &&
      Number.isFinite(Number(entity?.x)) && Number.isFinite(Number(entity?.y)) &&
      distanceToPlayer(entity) <= maximumDistance)
    .sort((left, right) =>
      distanceToPlayer(left) - distanceToPlayer(right) ||
      Number(left.hp ?? 0) - Number(right.hp ?? 0) ||
      Number(left.objectId) - Number(right.objectId))[0] ?? null;
}

/**
 * Identify a rendered hostile that cuts the only currently visible route to a
 * walking transfer. Mine corridors can be sealed well outside both the
 * player's local radius and the destination AOI, so proximity alone cannot
 * explain every authoritative no-path result.
 */
export function findPathBlockingMonster({ map, snapshot, target }) {
  const origin = player(snapshot);
  if (!origin || !target) return null;
  const hostiles = (snapshot?.entities ?? []).filter(entity =>
    String(entity?.kind ?? '').toLowerCase() === 'monster' &&
    String(entity?.disposition ?? '').toLowerCase() === 'hostile' &&
    entity?.dead !== true && Number(entity?.hp ?? 1) > 0 &&
    Number.isSafeInteger(Number(entity?.objectId)) &&
    Number.isFinite(Number(entity?.x)) && Number.isFinite(Number(entity?.y))
  );
  if (hostiles.length === 0) return null;
  const hostileIds = new Set(hostiles.map(entity => Number(entity.objectId)));
  const nonHostileObstacles = liveDynamicObstacles(snapshot).filter(entity =>
    !hostileIds.has(Number(entity.objectId))
  );
  const path = findProtocolWalkPath({
    map,
    start: origin,
    target,
    dynamicObstacles: nonHostileObstacles,
    staticWalkableOverrides: [target],
  });
  if (!path?.length) return null;

  const firstRequiredIndex = hostile => {
    const hx = Number(hostile.x);
    const hy = Number(hostile.y);
    for (let index = 0; index < path.length; index += 1) {
      const point = path[index];
      if (Number(point.x) === hx && Number(point.y) === hy) return index;
      if (index === 0) continue;
      const prior = path[index - 1];
      const dx = Number(point.x) - Number(prior.x);
      const dy = Number(point.y) - Number(prior.y);
      if (dx !== 0 && dy !== 0 && (
        (Number(prior.x) + dx === hx && Number(prior.y) === hy) ||
        (Number(prior.x) === hx && Number(prior.y) + dy === hy)
      )) return index;
    }
    return Number.POSITIVE_INFINITY;
  };
  return hostiles
    .map(entity => ({ entity, pathIndex: firstRequiredIndex(entity) }))
    .filter(entry => Number.isFinite(entry.pathIndex))
    .sort((left, right) => left.pathIndex - right.pathIndex ||
      Number(left.entity.objectId) - Number(right.entity.objectId))[0]?.entity ?? null;
}

function observedMapChange(client, afterSequence, nextMap) {
  if (String(client.snapshot?.mapFileName) === nextMap) return true;
  return client.events.some(event =>
    event?.sequence > afterSequence && event?.direction === 'received' &&
    ((event?.type === 'worldSnapshot' && String(event?.payload?.mapFileName) === nextMap) ||
      (event?.type === 'packet' && ['MapChanged', 'MapInformation'].includes(event?.packet) &&
        String(event?.payload?.fileName) === nextMap))
  );
}

async function requireFreshCurrentMapSnapshot(client, mapFileName) {
  if (!client.snapshot?.mapSnapshotPending) return;
  const afterRequest = client.sequence;
  client.send({ type: 'clientVersion' });
  await client.wait(
    () => !client.snapshot?.mapSnapshotPending &&
      String(client.snapshot?.mapFileName) === String(mapFileName) &&
      client.events.some(event =>
        event?.sequence > afterRequest && event?.direction === 'received' && event?.type === 'worldSnapshot'
      ),
    `fresh map snapshot for ${mapFileName}`,
    20_000,
  );
}

function receivedAfter(client, afterSequence, predicate) {
  return client.events.find(event =>
    event?.sequence > afterSequence && event?.direction === 'received' && predicate(event)
  ) ?? null;
}

function pointEquals(actual, expected) {
  return Number(actual?.x) === Number(expected?.x) &&
    Number(actual?.y) === Number(expected?.y);
}

function normalizedNpcName(value) {
  return String(value ?? '').replaceAll('_', ' ').trim().toLowerCase();
}

function sameScriptNpc(entity, routeNpc) {
  if (String(entity?.kind ?? '').toLowerCase() !== 'npc') return false;
  if (routeNpc?.objectId == null || String(entity?.objectId) !== String(routeNpc.objectId)) return false;
  if (normalizedNpcName(entity?.name) !== normalizedNpcName(routeNpc?.name)) return false;
  return pointEquals(entity, routeNpc?.position);
}

function liveScriptNpc(snapshot, routeNpc) {
  return (snapshot?.entities ?? []).find(entity => sameScriptNpc(entity, routeNpc)) ?? null;
}

function activeDialogLink(snapshot, npcObjectId, target) {
  const dialog = snapshot?.activeNpcDialog;
  if (String(dialog?.npcObjectId) !== String(npcObjectId)) return null;
  return (dialog?.links ?? []).find(link => String(link?.target ?? '') === String(target)) ?? null;
}

function normalizedItemName(value) {
  return String(value ?? '').trim().toLowerCase().replace(/[^a-z0-9]/g, '');
}

function inventoryItemCount(snapshot, itemName) {
  const wanted = normalizedItemName(itemName);
  return (snapshot?.inventoryItems ?? [])
    .filter(item =>
      normalizedItemName(item?.name) === wanted || normalizedItemName(item?.key) === wanted
    )
    .reduce((total, item) => total + Number(item?.quantity ?? 1), 0);
}

function scriptGoldPolicy(edge, snapshot) {
  const gold = Number(snapshot?.gold);
  const cost = Number(edge?.goldCost ?? 0);
  const hasMinimum = edge?.minimumGoldExclusive != null;
  const minimumExclusive = hasMinimum ? Number(edge.minimumGoldExclusive) : null;
  if (!Number.isSafeInteger(gold) || gold < 0) {
    throw new Error('Scripted travel requires an authoritative non-negative gold balance');
  }
  if (!Number.isSafeInteger(cost) || cost < 0) {
    throw new Error('Scripted travel edge has an invalid gold cost');
  }
  if (hasMinimum && (!Number.isSafeInteger(minimumExclusive) || minimumExclusive < 0)) {
    throw new Error('Scripted travel edge has an invalid strict gold threshold');
  }
  if (hasMinimum && gold <= minimumExclusive) {
    throw new Error(`Scripted travel requires more than ${minimumExclusive} gold; current balance is ${gold}`);
  }
  if (gold < cost) {
    throw new Error(`Scripted travel costs ${cost} gold; current balance is ${gold}`);
  }
  return { beforeGold: gold, cost };
}

function scriptItemPolicy(edge, snapshot) {
  const requiredItems = Array.isArray(edge?.requiredItems) ? edge.requiredItems : [];
  const itemCosts = Array.isArray(edge?.itemCosts) ? edge.itemCosts : [];
  for (const entry of [...requiredItems, ...itemCosts]) {
    if (normalizedItemName(entry?.item) === '' || !Number.isSafeInteger(Number(entry?.count)) || Number(entry.count) <= 0) {
      throw new Error('Scripted travel edge has an invalid item requirement');
    }
  }
  for (const entry of requiredItems) {
    const current = inventoryItemCount(snapshot, entry.item);
    if (current < Number(entry.count)) {
      throw new Error(`Scripted travel requires ${entry.count} ${entry.item}; current inventory has ${current}`);
    }
  }
  return itemCosts.map(entry => ({
    item: String(entry.item),
    count: Number(entry.count),
    before: inventoryItemCount(snapshot, entry.item),
  }));
}

function validateScriptEdge(edge) {
  const npc = edge?.npc;
  const point = npc?.position;
  const destination = edge?.destination;
  const targets = edge?.targetSequence;
  if (npc?.objectId == null || normalizedNpcName(npc?.name) === '' ||
      !Number.isFinite(Number(point?.x)) || !Number.isFinite(Number(point?.y))) {
    throw new Error('Scripted travel edge has no exact NPC binding');
  }
  if (!Array.isArray(targets) || targets.length === 0 ||
      targets.some(target => typeof target !== 'string' || target === '')) {
    throw new Error('Scripted travel edge has no verified dialog target sequence');
  }
  if (!Number.isFinite(Number(destination?.x)) || !Number.isFinite(Number(destination?.y))) {
    throw new Error('Scripted travel edge has no exact destination');
  }
}

function matchingMapChanged(client, afterSequence, edge) {
  return receivedAfter(client, afterSequence, event =>
    event?.type === 'packet' && event?.packet === 'MapChanged' &&
    String(event?.payload?.fileName) === String(edge.toMapFileName) &&
    pointEquals(event?.payload?.location, edge.destination)
  );
}

function matchingMapInformationLanding(client, afterSequence, edge) {
  const mapInformation = receivedAfter(client, afterSequence, event =>
    event?.type === 'packet' && event?.packet === 'MapInformation' &&
    String(event?.payload?.fileName) === String(edge.toMapFileName)
  );
  if (!mapInformation) return null;
  return receivedAfter(client, mapInformation.sequence, event =>
    event?.type === 'packet' && event?.packet === 'UserLocation' &&
    pointEquals(event?.payload, edge.destination)
  );
}

function matchingScriptedMapTransition(client, afterSequence, edge) {
  return matchingMapChanged(client, afterSequence, edge) ??
    matchingMapInformationLanding(client, afterSequence, edge);
}

function matchingFreshDestinationSnapshot(client, afterSequence, edge, expectedGold) {
  return receivedAfter(client, afterSequence, event => {
    if (event?.type !== 'worldSnapshot' ||
        String(event?.payload?.mapFileName) !== String(edge.toMapFileName) ||
        Number(event?.payload?.gold) !== expectedGold) return false;
    return pointEquals(player(event.payload), edge.destination);
  });
}

function itemCostsMatch(snapshot, itemCosts) {
  return itemCosts.every(entry =>
    inventoryItemCount(snapshot, entry.item) === entry.before - entry.count
  );
}

async function executeScriptedTransfer(client, navigateNear, edge, current, options = {}) {
  validateScriptEdge(edge);
  scriptGoldPolicy(edge, client.snapshot);
  scriptItemPolicy(edge, client.snapshot);
  await navigateNear(edge.npc.position, 1, () => travelShouldInterrupt(options));
  throwIfTravelInterrupted(options, current, edge.toMapFileName);
  if (client.snapshot?.mapSnapshotPending || String(client.snapshot?.mapFileName) !== current) {
    throw new Error(`Scripted travel origin became stale on map ${current}`);
  }

  const npc = await client.wait(
    () => liveScriptNpc(client.snapshot, edge.npc),
    `live scripted-travel NPC ${edge.npc.name}`,
    20_000,
  );
  const beforeInteract = client.sequence;
  client.send({ type: 'interact', objectId: Number(npc.objectId) });
  await client.wait(
    () => receivedAfter(client, beforeInteract, event =>
      event?.type === 'worldSnapshot' &&
      String(event?.payload?.activeNpcDialog?.npcObjectId) === String(npc.objectId)
    ),
    `scripted-travel dialog for ${edge.npc.name}`,
    20_000,
  );

  const targets = edge.targetSequence.map(String);
  for (let index = 0; index < targets.length; index += 1) {
    const target = targets[index];
    const link = activeDialogLink(client.snapshot, npc.objectId, target);
    if (!link) {
      throw new Error(`Scripted travel target ${target} is absent from the active ${edge.npc.name} dialog`);
    }

    const isFinalTarget = index === targets.length - 1;
    const beforeTarget = client.sequence;
    let goldPolicy = null;
    let itemCosts = null;
    if (isFinalTarget) {
      goldPolicy = scriptGoldPolicy(edge, client.snapshot);
      itemCosts = scriptItemPolicy(edge, client.snapshot);
    }
    // Send the target returned by the live dialog, never an unchecked static
    // target from route metadata.
    client.send({ type: 'selectNpcDialog', target: link.target });

    if (!isFinalTarget) {
      await client.wait(
        () => receivedAfter(client, beforeTarget, event =>
          event?.type === 'worldSnapshot' &&
          String(event?.payload?.activeNpcDialog?.npcObjectId) === String(npc.objectId)
        ),
        `scripted-travel dialog target ${target}`,
        20_000,
      );
      continue;
    }

    await client.wait(
      () => matchingScriptedMapTransition(client, beforeTarget, edge),
      `scripted map transfer ${current} -> ${edge.toMapFileName}`,
      20_000,
    );
    const expectedGold = goldPolicy.beforeGold - goldPolicy.cost;
    if (client.snapshot?.mapSnapshotPending ||
        !matchingFreshDestinationSnapshot(client, beforeTarget, edge, expectedGold) ||
        !itemCostsMatch(client.snapshot, itemCosts)) {
      client.send({ type: 'clientVersion' });
    }
    await client.wait(
      () => !client.snapshot?.mapSnapshotPending &&
        matchingFreshDestinationSnapshot(client, beforeTarget, edge, expectedGold) &&
        itemCostsMatch(client.snapshot, itemCosts),
      `authoritative scripted-travel snapshot for map ${edge.toMapFileName}`,
      20_000,
    );

    return {
      fromMapFileName: current,
      toMapFileName: String(edge.toMapFileName),
      scriptKey: String(edge.scriptKey ?? ''),
      npcObjectId: Number(npc.objectId),
      targetSequence: targets,
      position: { x: Number(edge.destination.x), y: Number(edge.destination.y) },
      goldCost: goldPolicy.cost,
      itemCosts: itemCosts.map(entry => ({ item: entry.item, count: entry.count })),
    };
  }
  throw new Error(`Scripted travel edge from ${current} did not select a destination`);
}

/**
 * Build a normal-player map traveler. Each hop is selected from the static
 * Crystal topology, but is executed only through a matching transfer exposed
 * by the current authoritative world snapshot and ordinary walking supplied by
 * navigateNear.
 */
export function createMapTraveler(client, navigateNear, dependencies = {}) {
  if (!client || typeof client.wait !== 'function') throw new Error('A protocol client is required');
  if (typeof navigateNear !== 'function') throw new Error('navigateNear is required');
  const collisionMaps = new Map();
  const loadCollisionMap = typeof dependencies.loadCollisionMap === 'function'
    ? dependencies.loadCollisionMap
    : loadProtocolCollisionMap;
  const resolveBlockingMonster = typeof dependencies.resolveBlockingMonster === 'function'
    ? dependencies.resolveBlockingMonster
    : null;
  const relocateDisconnectedRegion = typeof dependencies.relocateDisconnectedRegion === 'function'
    ? dependencies.relocateDisconnectedRegion
    : null;
  const maxDisconnectedRegionRelocations = Number.isSafeInteger(Number(dependencies.maxDisconnectedRegionRelocations))
    ? Math.max(1, Number(dependencies.maxDisconnectedRegionRelocations))
    : 4;
  const maxBlockingMonsterClears = Number.isSafeInteger(Number(dependencies.maxBlockingMonsterClears))
    ? Math.max(1, Number(dependencies.maxBlockingMonsterClears))
    : 8;

  const collisionMapFor = async mapFileName => {
    const key = String(mapFileName);
    if (!collisionMaps.has(key)) collisionMaps.set(key, loadCollisionMap(key));
    return collisionMaps.get(key);
  };

  const travelToMap = async function travelToMap(targetMap, options = {}) {
    const target = mapName(targetMap);
    const activeBlockingMonsterResolver = options.resolveBlockingMonster === false
      ? null
      : typeof options.resolveBlockingMonster === 'function'
        ? options.resolveBlockingMonster
        : resolveBlockingMonster;
    let current = mapName(client.snapshot?.mapFileName);
    if (current === target) return [];

    // A threat can interrupt the long walk from Sabuk's inner D701 landing to
    // its western exit. The next objective retry therefore starts in D701,
    // outside the merchant-quarter predicate above. Keep that retry pinned to
    // the western exit; choosing the nearest D701 -> 3 transfer would send the
    // player straight back into the enclosed quarter.
    if (options._skipSabukSafeDeparture !== true &&
        current === SABUK_SECRET_GATE &&
        target !== SABUK_MAP && target !== SABUK_SECRET_GATE) {
      const outer = await travelToMap(SABUK_MAP, {
        ...options,
        _skipSabukSafeDeparture: true,
        preferredTransferSource: SABUK_OUTER_EXIT,
      });
      const onward = await travelToMap(target, {
        ...options,
        _skipSabukSafeDeparture: false,
      });
      return [...outer, ...onward];
    }

    // Sabuk's merchant quarter is enclosed by invulnerable ArcherGuards. Any
    // generic objective or supply trip that leaves it must cross D701 back to
    // the west side before following the normal topology route.
    if (options._skipSabukSafeDeparture !== true &&
        isInsideSabukMerchantQuarter(client.snapshot) &&
        target !== SABUK_MAP && target !== SABUK_SECRET_GATE) {
      const inner = await travelToMap(SABUK_SECRET_GATE, {
        ...options,
        _skipSabukSafeDeparture: true,
        preferredTransferSource: SABUK_INNER_ENTRANCE,
      });
      const outer = await travelToMap(SABUK_MAP, {
        ...options,
        _skipSabukSafeDeparture: true,
        preferredTransferSource: SABUK_OUTER_EXIT,
      });
      const onward = await travelToMap(target, {
        ...options,
        // Recheck the landing. If an incidental movement entered a different
        // D701 return portal, this must run the safe departure again instead
        // of walking through Sabuk's invulnerable guard line.
        _skipSabukSafeDeparture: false,
      });
      return [...inner, ...outer, ...onward];
    }

    const graph = await travelGraphPromise;
    // Some quest endpoints have a physically shorter topology route that is
    // unsafe for a normal player. Callers may prefer an ordinary, direct NPC
    // transporter while retaining the normal route as a fallback.
    const directScriptedEdge = options.preferDirectScriptedEdge === true
      ? nearestDirectScriptedEdge(graph, current, target, client.snapshot)
      : null;
    const preferredTransferKey = String(options.preferredTransferKey ?? '');
    const preferredLiveTransfer = preferredTransferKey
      ? liveTransferCandidates(client.snapshot, target).find(transfer =>
        String(transfer?.key ?? '') === preferredTransferKey &&
        String(transfer?.mapFileName ?? current) === current &&
        String(transfer?.toMapFileName ?? '') === target)
      : null;
    if (preferredTransferKey && !preferredLiveTransfer) {
      throw new Error(`No live walking transfer with key ${preferredTransferKey} from ${current} to ${target}`);
    }
    const preferredLiveEdge = preferredLiveTransfer ? {
      kind: 'map-movement',
      fromMapFileName: current,
      toMapFileName: target,
      fromMapTitle: String(preferredLiveTransfer.mapTitle ?? current),
      toMapTitle: String(preferredLiveTransfer.toMapTitle ?? target),
      needHole: false,
      needMove: false,
      portals: [{
        source: {
          x: Number(preferredLiveTransfer.bounds.minX),
          y: Number(preferredLiveTransfer.bounds.minY),
        },
        destination: {
          x: Number(preferredLiveTransfer.toPosition?.x),
          y: Number(preferredLiveTransfer.toPosition?.y),
        },
      }],
    } : null;
    const route = directScriptedEdge
      ? [directScriptedEdge]
      : preferredLiveEdge
        ? [preferredLiveEdge]
      : findMapTravelRoute(graph, current, target);
    if (!route?.length) {
      throw new Error(`No normal-player map route from ${current} to ${target}`);
    }

    // D611 reuses one map image for two disconnected passages. A player
    // arriving from D602 can physically reach only the D603 exit; the D612
    // exit belongs to the other component reached through D603 -> D608 ->
    // D604 -> D611. The topology graph cannot express components, so prove
    // the direct portal is unreachable before taking Crystal's physical loop.
    if (options._skipBugCaveComponentDetour !== true &&
        current === BUG_CAVE_CROSSROADS &&
        String(route[0]?.toMapFileName) === BUG_CAVE_DEEP_ROUTE) {
      const direct = chooseLiveTransfer(client.snapshot, route[0], options);
      const origin = player(client.snapshot);
      const directPath = direct && origin ? findProtocolWalkPath({
        map: await collisionMapFor(current),
        start: origin,
        target: direct.target,
        staticWalkableOverrides: [direct.target],
      }) : null;
      if (!directPath?.length) {
        const traversed = [];
        for (const waypoint of BUG_CAVE_COMPONENT_DETOUR) {
          traversed.push(...await travelToMap(waypoint, {
            ...options,
            _skipBugCaveComponentDetour: true,
          }));
        }
        traversed.push(...await travelToMap(target, {
          ...options,
          _skipBugCaveComponentDetour: true,
        }));
        return traversed;
      }
    }

    // D601 has the same reused-map shape. The D610 landing is isolated from
    // the province exit, while the D607 landing shares the walkable component
    // with that exit. Return through the real cave ring instead of requiring a
    // random teleport or failing a normal player's journey.
    if (options._skipBugCaveReturnDetour !== true &&
        current === BUG_CAVE_ENTRANCE &&
        String(route[0]?.toMapFileName) === BUG_CAVE_PROVINCE) {
      const direct = chooseLiveTransfer(client.snapshot, route[0], options);
      const origin = player(client.snapshot);
      const directPath = direct && origin ? findProtocolWalkPath({
        map: await collisionMapFor(current),
        start: origin,
        target: direct.target,
        staticWalkableOverrides: [direct.target],
      }) : null;
      if (!directPath?.length) {
        const traversed = [];
        for (const waypoint of BUG_CAVE_RETURN_DETOUR) {
          traversed.push(...await travelToMap(waypoint, {
            ...options,
            _skipBugCaveReturnDetour: true,
          }));
        }
        traversed.push(...await travelToMap(target, {
          ...options,
          _skipBugCaveReturnDetour: true,
        }));
        return traversed;
      }
    }

    const traversed = [];
    for (const edge of route) {
      current = mapName(client.snapshot?.mapFileName);
      if (current !== String(edge.fromMapFileName)) {
        throw new Error(`Map route became stale: expected ${edge.fromMapFileName}, found ${current}`);
      }
      await requireFreshCurrentMapSnapshot(client, current);
      if (edge.kind === 'npc-script') {
        traversed.push(await executeScriptedTransfer(client, navigateNear, edge, current, options));
        continue;
      }
      if (edge.kind !== 'map-movement') {
        throw new Error(`Unsupported map edge kind ${edge.kind} from ${current} to ${edge.toMapFileName}`);
      }
      let live = chooseLiveTransfer(client.snapshot, edge, options);
      if (!live) {
        throw new Error(`No live walking transfer from ${current} to ${edge.toMapFileName}`);
      }
      if (String(live.transfer.key ?? '') === '') {
        throw new Error(`Live walking transfer from ${current} to ${edge.toMapFileName} has no authoritative key`);
      }

      const beforeTransfer = client.sequence;
      const rejectedTransferKeys = new Set();
      let disconnectedRegionRelocations = 0;
      const currentPlayer = player(client.snapshot);
      if (pointInBounds(currentPlayer, live.transfer.bounds)) {
        const exit = findProtocolTransferExitStep({
          map: await collisionMapFor(current),
          start: currentPlayer,
          bounds: live.transfer.bounds,
          dynamicObstacles: liveDynamicObstacles(client.snapshot),
        });
        if (!exit) {
          throw new Error(`No safe normal step outside live transfer ${live.transfer.key}`);
        }
        await navigateNear(
          exit,
          0,
          () => observedMapChange(client, beforeTransfer, String(edge.toMapFileName)) ||
            travelShouldInterrupt(options),
        );
        if (!observedMapChange(client, beforeTransfer, String(edge.toMapFileName))) {
          throwIfTravelInterrupted(options, current, edge.toMapFileName);
        }
        if (!observedMapChange(client, beforeTransfer, String(edge.toMapFileName)) &&
            (!pointEquals(player(client.snapshot), exit) || client.snapshot?.mapSnapshotPending)) {
          throw new Error(`Failed to step outside live transfer ${live.transfer.key}`);
        }
      }
      if (!observedMapChange(client, beforeTransfer, String(edge.toMapFileName))) {
        for (let clearAttempt = 0; clearAttempt <= maxBlockingMonsterClears; clearAttempt += 1) {
          try {
            await navigateNear(live.target, 0, () => travelShouldInterrupt(options), {
              liveTransferKey: String(live.transfer.key ?? ''),
            });
            break;
          } catch (error) {
            if (!String(error?.message ?? '').startsWith('No walk path')) throw error;
            // A dense pack may seal every locally planned first step while the
            // selected transfer is still far away. Once an adjacent attacker
            // is proven, classify that no-path result as the same bounded
            // travel interruption used by stopWhen; the quest combat policy
            // can then clear or evade the actual threat instead of exiting on
            // a raw navigation error.
            throwIfTravelInterrupted(options, current, edge.toMapFileName);
            // A narrow cave can be sealed near the player long before the
            // destination transfer enters AOI. Clear the nearest bounded local
            // blocker when no transfer-adjacent monster is visible, then retry
            // the same authoritative portal route.
            const blocker = findPathBlockingMonster({
              map: await collisionMapFor(current),
              snapshot: client.snapshot,
              target: live.target,
            }) ?? transferBlockingMonster(client.snapshot, live) ??
              localBlockingMonster(client.snapshot);
            if (!blocker) {
              rejectedTransferKeys.add(String(live.transfer.key));
              const alternative = liveTransferChoices(client.snapshot, edge, options).find(candidate =>
                !rejectedTransferKeys.has(String(candidate.transfer.key)) &&
                (!pointEquals(candidate.target, live.target) ||
                  String(candidate.transfer.key) !== String(live.transfer.key)));
              if (!alternative) {
                const origin = player(client.snapshot);
                const staticPath = origin ? findProtocolWalkPath({
                  map: await collisionMapFor(current),
                  start: origin,
                  target: live.target,
                  staticWalkableOverrides: [live.target],
                }) : null;
                if (!staticPath?.length && relocateDisconnectedRegion &&
                    disconnectedRegionRelocations < maxDisconnectedRegionRelocations) {
                  const before = origin ? { x: Number(origin.x), y: Number(origin.y) } : null;
                  const relocation = await relocateDisconnectedRegion(client, {
                    fromMapFileName: current,
                    toMapFileName: String(edge.toMapFileName),
                    target: { ...live.target },
                    cause: error,
                  });
                  if (relocation && relocation.deferred !== true) {
                    disconnectedRegionRelocations += 1;
                    rejectedTransferKeys.clear();
                    live = chooseLiveTransfer(client.snapshot, edge, options) ?? live;
                    if (typeof client.record === 'function') {
                      const after = player(client.snapshot);
                      client.record('diagnostic', {
                        type: 'disconnectedRegionRelocation',
                        fromMapFileName: current,
                        toMapFileName: String(edge.toMapFileName),
                        before,
                        after: after ? { x: Number(after.x), y: Number(after.y) } : null,
                        attempt: disconnectedRegionRelocations,
                      });
                    }
                    continue;
                  }
                }
                throw error;
              }
              const previous = live;
              live = alternative;
              if (typeof client.record === 'function') {
                client.record('diagnostic', {
                  type: 'alternateLiveTransfer',
                  fromMapFileName: current,
                  toMapFileName: String(edge.toMapFileName),
                  rejectedTransferKey: String(previous.transfer.key),
                  rejectedTarget: previous.target,
                  transferKey: String(live.transfer.key),
                  target: live.target,
                });
              }
              continue;
            }
            const blocked = new TravelBlockedByMonster(current, edge.toMapFileName, blocker, error);
            if (!activeBlockingMonsterResolver || clearAttempt >= maxBlockingMonsterClears) throw blocked;
            await activeBlockingMonsterResolver(client, blocker, blocked);
          }
        }
        if (!observedMapChange(client, beforeTransfer, String(edge.toMapFileName))) {
          throwIfTravelInterrupted(options, current, edge.toMapFileName);
        }
      }
      await client.wait(
        () => observedMapChange(client, beforeTransfer, String(edge.toMapFileName)),
        `map transfer ${current} -> ${edge.toMapFileName}`,
        20_000,
      );
      if (client.snapshot?.mapSnapshotPending || String(client.snapshot?.mapFileName) !== String(edge.toMapFileName)) {
        client.send({ type: 'clientVersion' });
        await client.wait(
          () => !client.snapshot?.mapSnapshotPending && String(client.snapshot?.mapFileName) === String(edge.toMapFileName),
          `authoritative snapshot for map ${edge.toMapFileName}`,
          20_000,
        );
      }
      traversed.push({
        fromMapFileName: current,
        toMapFileName: String(edge.toMapFileName),
        transferKey: String(live.transfer.key ?? ''),
        position: live.target,
      });
    }
    return traversed;
  };

  // Topology-only query for callers choosing among equivalent destinations.
  // It never inspects live transfers, spends resources, or emits commands.
  travelToMap.routeLength = async targetMap => {
    const target = mapName(targetMap);
    const current = mapName(client.snapshot?.mapFileName);
    if (current === target) return 0;
    const graph = await travelGraphPromise;
    return findMapTravelRoute(graph, current, target)?.length ?? null;
  };
  travelToMap.canReach = async targetMap => {
    return (await travelToMap.routeLength(targetMap)) !== null;
  };

  return travelToMap;
}
