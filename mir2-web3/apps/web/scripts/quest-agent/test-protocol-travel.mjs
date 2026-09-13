import assert from 'node:assert/strict';
import test from 'node:test';
import {
  buildMapTravelGraph,
  loadCrystalQuestRouteSources,
} from './route-manifest.mjs';
import { applyProtocolObservation } from './protocol-observation.mjs';
import {
  createMapTraveler,
  findPathBlockingMonster,
  TravelBlockedByMonster,
  TravelInterrupted,
} from './protocol-travel.mjs';

const graph = buildMapTravelGraph(await loadCrystalQuestRouteSources());
const ordinaryEdge = graph.edges.find(edge => edge.kind === 'map-movement');
const mageHouseExit = graph.edges.find(edge =>
  edge.kind === 'map-movement' && edge.fromMapFileName === '0115' && edge.toMapFileName === '0'
);
const rupertPrajna = graph.edges.find(edge =>
  edge.kind === 'npc-script' && edge.fromMapFileName === '0' && edge.toMapFileName === '5'
);
const raymondWhiteValley = graph.edges.find(edge =>
  edge.kind === 'npc-script' && edge.fromMapFileName === '5' && edge.toMapFileName === 'WhiteVillage'
);
const stoneHeartPassage = graph.edges.find(edge =>
  edge.kind === 'npc-script' && edge.fromMapFileName === 'D715' && edge.toMapFileName === 'D710A'
);
if (!ordinaryEdge || !mageHouseExit || !rupertPrajna || !raymondWhiteValley || !stoneHeartPassage) {
  throw new Error('Crystal route manifest lacks a required travel test edge');
}

function selfPlayer(position = { x: 5, y: 5 }) {
  return { objectId: 1, kind: 'selfPlayer', ...position };
}

function pointInside(point, bounds) {
  return point.x >= bounds.minX && point.x <= bounds.maxX &&
    point.y >= bounds.minY && point.y <= bounds.maxY;
}

test('finds a hostile sealing a distant one-cell corridor', () => {
  const width = 5;
  const height = 5;
  const blocked = new Uint8Array(width * height).fill(1);
  for (let x = 0; x < width; x += 1) blocked[2 * width + x] = 0;
  const snapshot = {
    playerObjectId: 1,
    entities: [
      selfPlayer({ x: 0, y: 2 }),
      { objectId: 77, kind: 'monster', disposition: 'hostile', x: 2, y: 2, hp: 20 },
      { objectId: 78, kind: 'monster', disposition: 'hostile', x: 4, y: 1, hp: 20 },
    ],
  };

  const blocker = findPathBlockingMonster({
    map: { width, height, blocked },
    snapshot,
    target: { x: 4, y: 2 },
  });

  assert.equal(blocker?.objectId, 77);
});

function npcEntity(edge, overrides = {}) {
  return {
    objectId: edge.npc.objectId,
    kind: 'npc',
    name: edge.npc.name,
    ...edge.npc.position,
    ...overrides,
  };
}

function walkingSnapshot(edge, includeTransfer = true) {
  return {
    mapFileName: edge.fromMapFileName,
    mapSnapshotPending: false,
    playerObjectId: 1,
    entities: [selfPlayer()],
    mapTransfers: includeTransfer ? [{
      key: 'live-transfer',
      mapFileName: edge.fromMapFileName,
      toMapFileName: edge.toMapFileName,
      bounds: { minX: 8, maxX: 10, minY: 2, maxY: 4 },
    }] : [],
  };
}

function scriptedSnapshot(edge, gold, npc = npcEntity(edge)) {
  return {
    mapFileName: edge.fromMapFileName,
    mapSnapshotPending: false,
    gold,
    playerObjectId: 1,
    entities: [selfPlayer(), npc],
    mapTransfers: [],
    activeNpcDialog: null,
  };
}

class FakeClient {
  constructor(snapshot, onSend = null) {
    this.snapshot = structuredClone(snapshot);
    this.sequence = 0;
    this.events = [];
    this.commands = [];
    this.onSend = onSend;
  }

  send(command) {
    const copied = structuredClone(command);
    this.commands.push(copied);
    this.events.push({ sequence: ++this.sequence, direction: 'sent', ...copied });
    this.onSend?.(copied, this);
  }

  receive(value) {
    const copied = structuredClone(value);
    this.snapshot = applyProtocolObservation(this.snapshot, copied);
    this.events.push({ sequence: ++this.sequence, direction: 'received', ...copied });
  }

  async wait(predicate, label) {
    const result = predicate();
    if (!result) throw new Error(`Fake timeout waiting for ${label}`);
    return result;
  }
}

function destinationSnapshot(edge, beforeGold, overrides = {}) {
  return {
    mapFileName: edge.toMapFileName,
    mapSnapshotPending: false,
    gold: beforeGold - edge.goldCost,
    playerObjectId: 1,
    entities: [selfPlayer(edge.destination)],
    mapTransfers: [],
    activeNpcDialog: null,
    ...overrides,
  };
}

function successfulScriptClient(edge, beforeGold, {
  refreshOnClientVersion = false,
  mapPacket = 'MapChanged',
} = {}) {
  return new FakeClient(scriptedSnapshot(edge, beforeGold), (command, client) => {
    if (command.type === 'interact') {
      client.receive({
        type: 'worldSnapshot',
        payload: {
          ...client.snapshot,
          activeNpcDialog: {
            npcObjectId: edge.npc.objectId,
            links: edge.targetSequence.map(target => ({ label: target, target })),
          },
        },
      });
      return;
    }
    if (command.type === 'selectNpcDialog') {
      if (mapPacket === 'MapInformation') {
        client.receive({
          type: 'packet',
          packet: 'MapInformation',
          payload: { fileName: edge.toMapFileName, mapIndex: 34, title: 'PrajnaIsland' },
        });
        client.receive({ type: 'packet', packet: 'UserLocation', payload: edge.destination });
      } else {
        client.receive({
          type: 'packet',
          packet: 'MapChanged',
          payload: { fileName: edge.toMapFileName, location: edge.destination },
        });
      }
      if (!refreshOnClientVersion) {
        client.receive({ type: 'worldSnapshot', payload: destinationSnapshot(edge, beforeGold) });
      }
      return;
    }
    if (command.type === 'clientVersion' && refreshOnClientVersion) {
      client.receive({ type: 'worldSnapshot', payload: destinationSnapshot(edge, beforeGold) });
    }
  });
}

test('Crystal scripted edges retain exact Sailor targets, strict balances, costs, and destinations', () => {
  assert.deepEqual({
    npc: [rupertPrajna.npc.objectId, rupertPrajna.npc.name, rupertPrajna.npc.position],
    targets: rupertPrajna.targetSequence,
    minimum: rupertPrajna.minimumGoldExclusive,
    cost: rupertPrajna.goldCost,
    destination: [rupertPrajna.toMapFileName, rupertPrajna.destination],
  }, {
    npc: [9, 'Sailor_Rupert', { x: 251, y: 677 }],
    targets: ['@brdmove'],
    minimum: 2000,
    cost: 2000,
    destination: ['5', { x: 124, y: 353 }],
  });
  assert.deepEqual({
    npc: [raymondWhiteValley.npc.objectId, raymondWhiteValley.npc.name, raymondWhiteValley.npc.position],
    targets: raymondWhiteValley.targetSequence,
    minimum: raymondWhiteValley.minimumGoldExclusive,
    cost: raymondWhiteValley.goldCost,
    destination: [raymondWhiteValley.toMapFileName, raymondWhiteValley.destination],
  }, {
    npc: [1169, 'Sailor_Raymond', { x: 122, y: 353 }],
    targets: ['@brdmove1'],
    minimum: 12000,
    cost: 12000,
    destination: ['WhiteVillage', { x: 67, y: 93 }],
  });
});

test('canReach filters the real q60 and q62 unreachable cave candidates without travelling', async () => {
  const client = new FakeClient({ mapFileName: '0', entities: [], mapTransfers: [] });
  const travel = createMapTraveler(client, async () => assert.fail('reachability must not navigate'));

  assert.equal(await travel.canReach('D2044'), false, 'q60 first candidate has no route from Bichon');
  assert.equal(await travel.canReach('D2042'), true, 'q60 has a reachable Insect Cave candidate');

  client.snapshot.mapFileName = '1';
  assert.equal(await travel.canReach('D2043'), false, 'q62 first candidate has no route from Woomyon Woods');
  assert.equal(await travel.canReach({ mapFileName: 'D2042' }), true, 'q62 has a reachable 2F candidate');
  assert.deepEqual(client.commands, []);
  assert.equal(client.sequence, 0);
});

test('routeLength reports map 2 as the nearer real q33 source from Bichon than map 3', async () => {
  const client = new FakeClient({ mapFileName: '0', entities: [], mapTransfers: [] });
  const travel = createMapTraveler(client, async () => assert.fail('route length must not navigate'));

  assert.equal(await travel.routeLength('2'), 1);
  assert.equal(await travel.routeLength('3'), 2);
  assert.equal(await travel.routeLength('D2044'), null);
  assert.deepEqual(client.commands, []);
  assert.equal(client.sequence, 0);
});

test('travels through an authoritative live walking transfer at distance zero', async () => {
  const client = new FakeClient(walkingSnapshot(ordinaryEdge));
  const calls = [];
  const travel = createMapTraveler(client, async (target, distance, _stopWhen, options) => {
    calls.push([target, distance, options]);
    client.receive({
      type: 'worldSnapshot',
      payload: { ...client.snapshot, mapFileName: ordinaryEdge.toMapFileName, mapTransfers: [] },
    });
  });
  const traversed = await travel(ordinaryEdge.toMapFileName);
  assert.deepEqual(calls, [[{ x: 8, y: 4 }, 0, { liveTransferKey: 'live-transfer' }]]);
  assert.deepEqual(traversed, [{
    fromMapFileName: ordinaryEdge.fromMapFileName,
    toMapFileName: ordinaryEdge.toMapFileName,
    transferKey: 'live-transfer',
    position: { x: 8, y: 4 },
  }]);
});

test('a proven threat can interrupt ordinary transfer travel before the client enters the portal', async () => {
  const client = new FakeClient(walkingSnapshot(ordinaryEdge));
  let navigationCalls = 0;
  const travel = createMapTraveler(client, async (_target, _distance, stopWhen) => {
    navigationCalls += 1;
    assert.equal(stopWhen(), true);
    return { reached: false, successfulSteps: 0 };
  });

  await assert.rejects(
    () => travel(ordinaryEdge.toMapFileName, { interruptWhen: () => true }),
    error => error instanceof TravelInterrupted &&
      error.fromMapFileName === String(ordinaryEdge.fromMapFileName) &&
      error.toMapFileName === String(ordinaryEdge.toMapFileName),
  );
  assert.equal(navigationCalls, 1);
  assert.equal(client.snapshot.mapFileName, ordinaryEdge.fromMapFileName);
});

test('a proven threat converts a sealed local path into a travel interruption', async () => {
  const client = new FakeClient(walkingSnapshot(ordinaryEdge));
  const travel = createMapTraveler(client, async (_target, _distance, stopWhen) => {
    assert.equal(stopWhen(), true);
    throw new Error(`No walk path on ${ordinaryEdge.fromMapFileName} from 43,114 to 76,15`);
  });

  await assert.rejects(
    () => travel(ordinaryEdge.toMapFileName, { interruptWhen: () => true }),
    error => error instanceof TravelInterrupted &&
      error.fromMapFileName === String(ordinaryEdge.fromMapFileName) &&
      error.toMapFileName === String(ordinaryEdge.toMapFileName),
  );
});

test('reports the hostile monster that blocks a narrow live transfer approach', async () => {
  const source = walkingSnapshot(ordinaryEdge);
  source.entities.push({
    objectId: 77,
    kind: 'monster',
    name: 'Zombie3',
    disposition: 'hostile',
    x: 8,
    y: 5,
    hp: 20,
    dead: false,
  });
  const client = new FakeClient(source);
  const travel = createMapTraveler(client, async () => {
    throw new Error(`No walk path on ${ordinaryEdge.fromMapFileName} from 5,5 to 8,4`);
  });

  await assert.rejects(
    () => travel(ordinaryEdge.toMapFileName),
    error => error instanceof TravelBlockedByMonster &&
      error.objectId === 77 && error.target.x === 8 && error.target.y === 5,
  );
});

test('a configured resolver clears a hostile transfer blocker and retries the portal', async () => {
  const source = walkingSnapshot(ordinaryEdge);
  const blocker = {
    objectId: 77,
    kind: 'monster',
    name: 'Zombie3',
    disposition: 'hostile',
    x: 8,
    y: 5,
    hp: 20,
    dead: false,
  };
  source.entities.push(blocker);
  const client = new FakeClient(source);
  let navigationCalls = 0;
  let resolverCalls = 0;
  const travel = createMapTraveler(client, async () => {
    navigationCalls += 1;
    if (navigationCalls === 1) throw new Error('No walk path on test map');
    client.receive({
      type: 'worldSnapshot',
      payload: { ...client.snapshot, mapFileName: ordinaryEdge.toMapFileName, mapTransfers: [] },
    });
  }, {
    resolveBlockingMonster: async (owner, target, error) => {
      resolverCalls += 1;
      assert.equal(target.objectId, 77);
      assert.ok(error instanceof TravelBlockedByMonster);
      owner.snapshot.entities = owner.snapshot.entities.filter(entry => entry.objectId !== 77);
    },
  });

  await travel(ordinaryEdge.toMapFileName);
  assert.equal(resolverCalls, 1);
  assert.equal(navigationCalls, 2);
});

test('a travel call can delegate a blocker to its quest policy instead of the global resolver', async () => {
  const source = walkingSnapshot(ordinaryEdge);
  source.entities.push({
    objectId: 77,
    kind: 'monster',
    name: 'Zombie3',
    disposition: 'hostile',
    x: 8,
    y: 5,
    hp: 20,
    dead: false,
  });
  const client = new FakeClient(source);
  let resolverCalls = 0;
  const travel = createMapTraveler(client, async () => {
    throw new Error('No walk path on test map');
  }, {
    resolveBlockingMonster: async () => { resolverCalls += 1; },
  });

  await assert.rejects(
    () => travel(ordinaryEdge.toMapFileName, { resolveBlockingMonster: false }),
    error => error instanceof TravelBlockedByMonster && error.objectId === 77,
  );
  assert.equal(resolverCalls, 0);
});

test('a configured resolver clears a nearby cave blocker before a distant transfer is visible', async () => {
  const source = walkingSnapshot(ordinaryEdge);
  Object.assign(source.entities[0], { x: 5, y: 5 });
  source.mapTransfers[0].bounds = { minX: 30, maxX: 30, minY: 30, maxY: 30 };
  source.entities.push({
    objectId: 78,
    kind: 'monster',
    name: 'Zombie3',
    disposition: 'hostile',
    x: 5,
    y: 4,
    hp: 20,
    dead: false,
  });
  const client = new FakeClient(source);
  let navigationCalls = 0;
  const cleared = [];
  const travel = createMapTraveler(client, async () => {
    navigationCalls += 1;
    if (navigationCalls === 1) throw new Error('No walk path on D401 from 5,5 to 30,30');
    client.receive({
      type: 'worldSnapshot',
      payload: { ...client.snapshot, mapFileName: ordinaryEdge.toMapFileName, mapTransfers: [] },
    });
  }, {
    resolveBlockingMonster: async (owner, target) => {
      cleared.push(target.objectId);
      owner.snapshot.entities = owner.snapshot.entities.filter(entry => entry.objectId !== target.objectId);
    },
  });

  await travel(ordinaryEdge.toMapFileName);
  assert.deepEqual(cleared, [78]);
  assert.equal(navigationCalls, 2);
});

test('walking travel rotates to another live transfer cell after an unblocked no-path result', async () => {
  const source = walkingSnapshot(ordinaryEdge);
  source.entities = [selfPlayer({ x: 5, y: 4 })];
  source.mapTransfers = [
    {
      key: 'blocked-transfer-cell',
      mapFileName: ordinaryEdge.fromMapFileName,
      toMapFileName: ordinaryEdge.toMapFileName,
      bounds: { minX: 8, maxX: 8, minY: 4, maxY: 4 },
    },
    {
      key: 'open-transfer-cell',
      mapFileName: ordinaryEdge.fromMapFileName,
      toMapFileName: ordinaryEdge.toMapFileName,
      bounds: { minX: 8, maxX: 8, minY: 5, maxY: 5 },
    },
  ];
  const client = new FakeClient(source);
  const calls = [];
  client.record = (_direction, payload) => client.events.push({
    sequence: ++client.sequence,
    direction: 'diagnostic',
    payload,
  });
  const travel = createMapTraveler(client, async (target, _distance, _stopWhen, options) => {
    calls.push({ target: { ...target }, transferKey: options.liveTransferKey });
    if (options.liveTransferKey === 'blocked-transfer-cell') {
      throw new Error('No walk path on test map');
    }
    client.receive({
      type: 'worldSnapshot',
      payload: { ...client.snapshot, mapFileName: ordinaryEdge.toMapFileName, mapTransfers: [] },
    });
  });

  const [result] = await travel(ordinaryEdge.toMapFileName);
  assert.deepEqual(calls, [
    { target: { x: 8, y: 4 }, transferKey: 'blocked-transfer-cell' },
    { target: { x: 8, y: 5 }, transferKey: 'open-transfer-cell' },
  ]);
  assert.equal(result.transferKey, 'open-transfer-cell');
  assert.ok(client.events.some(event =>
    event.direction === 'diagnostic' && event.payload.type === 'alternateLiveTransfer'));
});

test('steps off and re-enters the real MageHouse exit when already standing on its source', async () => {
  const transfer = {
    key: 'crystal-move:0115:17:21:1:315:476',
    mapFileName: '0115',
    toMapFileName: '0',
    bounds: { minX: 17, maxX: 17, minY: 21, maxY: 21 },
  };
  const client = new FakeClient({
    mapFileName: '0115',
    mapSnapshotPending: false,
    playerObjectId: 1,
    entities: [selfPlayer({ x: 17, y: 21 })],
    mapTransfers: [transfer],
  });
  const calls = [];
  const travel = createMapTraveler(client, async (target, distance, _stopWhen, options) => {
    calls.push([target, distance, options]);
    if (calls.length === 1) {
      assert.equal(pointInside(target, transfer.bounds), false);
      client.receive({
        type: 'worldSnapshot',
        payload: { ...client.snapshot, entities: [selfPlayer(target)] },
      });
      return;
    }
    client.receive({
      type: 'packet',
      packet: 'MapChanged',
      payload: { fileName: '0', location: { x: 315, y: 476 } },
    });
    client.receive({
      type: 'worldSnapshot',
      payload: {
        mapFileName: '0',
        mapSnapshotPending: false,
        playerObjectId: 1,
        entities: [selfPlayer({ x: 315, y: 476 })],
        mapTransfers: [],
      },
    });
  });

  const traversed = await travel('0');
  assert.deepEqual(calls, [
    [{ x: 17, y: 20 }, 0, undefined],
    [{ x: 17, y: 21 }, 0, { liveTransferKey: transfer.key }],
  ]);
  assert.equal(traversed[0].transferKey, transfer.key);
  assert.deepEqual(traversed[0].position, { x: 17, y: 21 });
});

test('refreshes a pending source snapshot before selecting and entering a live walking transfer', async () => {
  const pending = walkingSnapshot(ordinaryEdge, false);
  pending.mapSnapshotPending = true;
  const client = new FakeClient(pending, (command, current) => {
    if (command.type === 'clientVersion') {
      current.receive({ type: 'worldSnapshot', payload: walkingSnapshot(ordinaryEdge) });
    }
  });
  const calls = [];
  const travel = createMapTraveler(client, async (target, distance, _stopWhen, options) => {
    calls.push([target, distance, options]);
    client.receive({
      type: 'packet',
      packet: 'MapChanged',
      payload: { fileName: ordinaryEdge.toMapFileName, location: { x: 17, y: 21 } },
    });
    client.receive({
      type: 'worldSnapshot',
      payload: {
        ...walkingSnapshot(ordinaryEdge, false),
        mapFileName: ordinaryEdge.toMapFileName,
        mapSnapshotPending: false,
      },
    });
  });

  await travel(ordinaryEdge.toMapFileName);
  assert.deepEqual(client.commands, [{ type: 'clientVersion' }]);
  assert.deepEqual(calls, [[{ x: 8, y: 4 }, 0, { liveTransferKey: 'live-transfer' }]]);
});

test('actual MapInformation plus UserLocation completes walking transfer after one fresh snapshot request', async () => {
  const transfer = {
    key: 'crystal-move:0115:17:21:1:315:476',
    mapFileName: '0115',
    toMapFileName: '0',
    bounds: { minX: 17, maxX: 17, minY: 21, maxY: 21 },
  };
  const client = new FakeClient({
    mapFileName: '0115',
    mapSnapshotPending: false,
    playerObjectId: 1,
    entities: [selfPlayer({ x: 17, y: 20 })],
    mapTransfers: [transfer],
  }, (command, current) => {
    if (command.type === 'clientVersion') {
      current.receive({
        type: 'worldSnapshot',
        payload: {
          mapFileName: '0',
          playerObjectId: 1,
          entities: [selfPlayer({ x: 315, y: 476 })],
          mapTransfers: [],
        },
      });
    }
  });
  const travel = createMapTraveler(client, async () => {
    client.receive({
      type: 'packet',
      packet: 'MapInformation',
      payload: {
        bigMapIndex: 0, fileName: '0', lights: 2, mapDarkLight: 0, mapIndex: 33,
        miniMapIndex: 0, music: 0, spawnFlags: { fire: false, lightning: false },
        title: 'BichonProvince', weatherParticles: 0,
      },
    });
    client.receive({ type: 'packet', packet: 'UserLocation', payload: { direction: 'Down', x: 315, y: 476 } });
  });

  await travel('0');
  assert.deepEqual(client.commands, [{ type: 'clientVersion' }]);
  assert.equal(client.snapshot.mapSnapshotPending, false);
  assert.deepEqual(
    { x: client.snapshot.entities[0].x, y: client.snapshot.entities[0].y },
    { x: 315, y: 476 },
  );
});

test('refuses a live walking transfer without an authoritative key before navigation', async () => {
  const snapshot = walkingSnapshot(ordinaryEdge);
  snapshot.mapTransfers[0].key = '';
  const client = new FakeClient(snapshot);
  const travel = createMapTraveler(client, async () => assert.fail('navigation must not start'));
  await assert.rejects(() => travel(ordinaryEdge.toMapFileName), /has no authoritative key/);
});

test('Rupert travel uses a live dialog link and refreshes a pending destination snapshot', async () => {
  const client = successfulScriptClient(rupertPrajna, 5000, { refreshOnClientVersion: true });
  const navigation = [];
  const travel = createMapTraveler(client, async (target, distance) => navigation.push([target, distance]));

  assert.deepEqual(await travel('5'), [{
    fromMapFileName: '0',
    toMapFileName: '5',
    scriptKey: 'BichonProvince/Sailor',
    npcObjectId: 9,
    targetSequence: ['@brdmove'],
    position: { x: 124, y: 353 },
    goldCost: 2000,
    itemCosts: [],
  }]);
  assert.deepEqual(navigation, [[{ x: 251, y: 677 }, 1]]);
  assert.deepEqual(client.commands, [
    { type: 'interact', objectId: 9 },
    { type: 'selectNpcDialog', target: '@brdmove' },
    { type: 'clientVersion' },
  ]);
  assert.equal(client.snapshot.gold, 3000);
});

test('scripted travel accepts MapInformation only with its following exact UserLocation and fresh snapshot', async () => {
  const client = successfulScriptClient(rupertPrajna, 5000, {
    refreshOnClientVersion: true,
    mapPacket: 'MapInformation',
  });
  const travel = createMapTraveler(client, async () => {});

  const [result] = await travel('5');
  assert.deepEqual(result.position, rupertPrajna.destination);
  assert.deepEqual(client.commands, [
    { type: 'interact', objectId: 9 },
    { type: 'selectNpcDialog', target: '@brdmove' },
    { type: 'clientVersion' },
  ]);
});

test('Raymond travel verifies the live White Valley link, landing, and 12000 gold debit', async () => {
  const client = successfulScriptClient(raymondWhiteValley, 15000);
  const travel = createMapTraveler(client, async () => {});
  const [result] = await travel('WhiteVillage');

  assert.equal(result.npcObjectId, 1169);
  assert.deepEqual(result.targetSequence, ['@brdmove1']);
  assert.deepEqual(result.position, { x: 67, y: 93 });
  assert.equal(result.goldCost, 12000);
  assert.equal(client.snapshot.gold, 3000);
  assert.deepEqual(client.commands, [
    { type: 'interact', objectId: 1169 },
    { type: 'selectNpcDialog', target: '@brdmove1' },
  ]);
});

test('an item-gated scripted edge verifies and consumes its exact inventory cost', async () => {
  const starting = scriptedSnapshot(stoneHeartPassage, 0);
  starting.inventoryItems = [{ key: 'stone-heart', name: 'StoneHeart', quantity: 1 }];
  const client = new FakeClient(starting, (command, current) => {
    if (command.type === 'interact') {
      current.receive({
        type: 'worldSnapshot',
        payload: {
          ...current.snapshot,
          activeNpcDialog: {
            npcObjectId: stoneHeartPassage.npc.objectId,
            links: [{ target: '@stonetomba' }],
          },
        },
      });
    } else if (command.type === 'selectNpcDialog') {
      current.receive({
        type: 'packet',
        packet: 'MapChanged',
        payload: { fileName: 'D710A', location: stoneHeartPassage.destination },
      });
      current.receive({
        type: 'worldSnapshot',
        payload: {
          ...destinationSnapshot(stoneHeartPassage, 0),
          inventoryItems: [],
        },
      });
    }
  });
  const travel = createMapTraveler(client, async () => {});
  const [result] = await travel('D710A');
  assert.deepEqual(result.itemCosts, [{ item: 'StoneHeart', count: 1 }]);
  assert.equal(client.snapshot.inventoryItems.length, 0);
});

test('refuses a walking edge when no matching live transfer exists', async () => {
  const client = new FakeClient(walkingSnapshot(ordinaryEdge, false));
  const travel = createMapTraveler(client, async () => assert.fail('navigation must not start'));
  await assert.rejects(() => travel(ordinaryEdge.toMapFileName), /No live walking transfer/);
});

test('refuses scripted travel when gold only equals the strict threshold', async () => {
  const client = new FakeClient(scriptedSnapshot(rupertPrajna, 2000));
  const travel = createMapTraveler(client, async () => assert.fail('navigation must not start'));
  await assert.rejects(() => travel('5'), /requires more than 2000 gold/);
  assert.deepEqual(client.commands, []);
});

test('refuses an item-gated scripted edge before navigation when the required item is absent', async () => {
  const client = new FakeClient(scriptedSnapshot(stoneHeartPassage, 0));
  const travel = createMapTraveler(client, async () => assert.fail('navigation must not start'));
  await assert.rejects(() => travel('D710A'), /requires 1 StoneHeart/);
  assert.deepEqual(client.commands, []);
});

test('requires the exact live NPC binding before opening a scripted dialog', async () => {
  const client = new FakeClient(scriptedSnapshot(rupertPrajna, 5000, npcEntity(rupertPrajna, { objectId: 99 })));
  const travel = createMapTraveler(client, async () => {});
  await assert.rejects(() => travel('5'), /live scripted-travel NPC Sailor_Rupert/);
  assert.deepEqual(client.commands, []);
});

test('never sends a static scripted target absent from the live dialog', async () => {
  const client = new FakeClient(scriptedSnapshot(rupertPrajna, 5000), (command, current) => {
    if (command.type !== 'interact') return;
    current.receive({
      type: 'worldSnapshot',
      payload: {
        ...current.snapshot,
        activeNpcDialog: {
          npcObjectId: rupertPrajna.npc.objectId,
          links: [{ label: 'Talk', target: '@talk' }],
        },
      },
    });
  });
  const travel = createMapTraveler(client, async () => {});
  await assert.rejects(() => travel('5'), /target @brdmove is absent/);
  assert.deepEqual(client.commands, [{ type: 'interact', objectId: 9 }]);
});

test('an old active dialog does not replace fresh post-interact snapshot proof', async () => {
  const snapshot = scriptedSnapshot(rupertPrajna, 5000);
  snapshot.activeNpcDialog = {
    npcObjectId: 9,
    links: [{ label: 'old', target: '@brdmove' }],
  };
  const client = new FakeClient(snapshot);
  const travel = createMapTraveler(client, async () => {});
  await assert.rejects(() => travel('5'), /scripted-travel dialog for Sailor_Rupert/);
  assert.deepEqual(client.commands, [{ type: 'interact', objectId: 9 }]);
});

test('rejects MapChanged whose landing does not match the scripted edge', async () => {
  const client = new FakeClient(scriptedSnapshot(rupertPrajna, 5000), (command, current) => {
    if (command.type === 'interact') {
      current.receive({
        type: 'worldSnapshot',
        payload: {
          ...current.snapshot,
          activeNpcDialog: { npcObjectId: 9, links: [{ target: '@brdmove' }] },
        },
      });
    } else if (command.type === 'selectNpcDialog') {
      current.receive({
        type: 'packet',
        packet: 'MapChanged',
        payload: { fileName: '5', location: { x: 125, y: 353 } },
      });
    }
  });
  const travel = createMapTraveler(client, async () => {});
  await assert.rejects(() => travel('5'), /scripted map transfer 0 -> 5/);
});

test('rejects MapInformation without a following exact UserLocation landing', async () => {
  const client = new FakeClient(scriptedSnapshot(rupertPrajna, 5000), (command, current) => {
    if (command.type === 'interact') {
      current.receive({
        type: 'worldSnapshot',
        payload: {
          ...current.snapshot,
          activeNpcDialog: { npcObjectId: 9, links: [{ target: '@brdmove' }] },
        },
      });
    } else if (command.type === 'selectNpcDialog') {
      current.receive({
        type: 'packet',
        packet: 'MapInformation',
        payload: { fileName: '5', mapIndex: 34, title: 'PrajnaIsland' },
      });
      current.receive({
        type: 'worldSnapshot',
        payload: destinationSnapshot(rupertPrajna, 5000),
      });
    }
  });
  const travel = createMapTraveler(client, async () => {});
  await assert.rejects(() => travel('5'), /scripted map transfer 0 -> 5/);
});

test('rejects a fresh destination snapshot with the wrong charged balance', async () => {
  const client = new FakeClient(scriptedSnapshot(rupertPrajna, 5000), (command, current) => {
    if (command.type === 'interact') {
      current.receive({
        type: 'worldSnapshot',
        payload: {
          ...current.snapshot,
          activeNpcDialog: { npcObjectId: 9, links: [{ target: '@brdmove' }] },
        },
      });
    } else if (command.type === 'selectNpcDialog') {
      current.receive({
        type: 'packet',
        packet: 'MapChanged',
        payload: { fileName: '5', location: rupertPrajna.destination },
      });
      current.receive({
        type: 'worldSnapshot',
        payload: destinationSnapshot(rupertPrajna, 5000, { gold: 2999 }),
      });
    }
  });
  const travel = createMapTraveler(client, async () => {});
  await assert.rejects(() => travel('5'), /authoritative scripted-travel snapshot for map 5/);
});

test('already being on the target map is a no-op', async () => {
  const client = new FakeClient({ mapFileName: '0', entities: [], mapTransfers: [] });
  const travel = createMapTraveler(client, async () => assert.fail('navigation must not start'));
  assert.deepEqual(await travel({ mapFileName: '0' }), []);
});
