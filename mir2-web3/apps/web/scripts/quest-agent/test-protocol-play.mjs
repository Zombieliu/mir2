import assert from 'node:assert/strict';
import test from 'node:test';

import { createNavigator, NavigationStalled } from './protocol-play.mjs';

function openMap() {
  return {
    mapFileName: 'test',
    sourcePath: 'test.map',
    width: 8,
    height: 8,
    blocked: new Uint8Array(64),
  };
}

function navigationClient() {
  return {
    snapshot: {
      mapFileName: 'test',
      playerHp: 100,
      playerMaxHp: 100,
      playerObjectId: 1,
      entities: [{ objectId: 1, kind: 'selfPlayer', x: 1, y: 1, dead: false }],
    },
    sequence: 0,
    events: [],
    sent: [],
    diagnostics: [],
    send(command) {
      this.sent.push(command);
      this.sequence += 1;
    },
    record(_direction, value) {
      this.diagnostics.push(value);
    },
  };
}

const directionDelta = {
  Up: [0, -1],
  UpRight: [1, -1],
  Right: [1, 0],
  DownRight: [1, 1],
  Down: [0, 1],
  DownLeft: [-1, 1],
  Left: [-1, 0],
  UpLeft: [-1, -1],
};

function acknowledgeUnitMovement(client, visited = []) {
  return async predicate => {
    const command = client.sent.at(-1);
    const [dx, dy] = directionDelta[command.direction];
    const self = client.snapshot.entities[0];
    self.x += dx;
    self.y += dy;
    visited.push({ x: self.x, y: self.y });
    assert.ok(predicate());
  };
}

const dependencies = {
  loadCollisionMap: async () => openMap(),
  delay: async () => {},
  now: () => 10_000,
};

test('navigator propagates a Gateway failure without rejecting a valid tile or sending another move', async () => {
  const client = navigationClient();
  const gatewayFailure = new Error('Gateway rejected command: Player state synchronization is temporarily unavailable.');
  client.wait = async () => {
    client.failure = gatewayFailure;
    throw gatewayFailure;
  };

  const navigateNear = createNavigator(client, dependencies);
  await assert.rejects(navigateNear({ x: 5, y: 1 }, 1), error => error === gatewayFailure);
  assert.equal(client.sent.length, 1);
  assert.equal(client.diagnostics.length, 0);
});

test('navigator propagates an existing failure before a tactical stop predicate can mask it', async () => {
  const gatewayFailure = new Error('Gateway disconnected');
  const client = navigationClient();
  client.failure = gatewayFailure;
  let stopChecks = 0;
  const navigate = createNavigator(client, { loadCollisionMap: async () => openMap(), delay: async () => {} });

  await assert.rejects(
    () => navigate({ x: 12, y: 10 }, 1, () => { stopChecks += 1; return true; }),
    error => error === gatewayFailure,
  );
  assert.equal(stopChecks, 0);
  assert.deepEqual(client.sent, []);
});

test('navigator waits sixty seconds for one authoritative movement response', async () => {
  const client = navigationClient();
  const timeouts = [];
  client.wait = async (predicate, _label, timeout) => {
    timeouts.push(timeout);
    Object.assign(client.snapshot.entities[0], { x: 4, y: 1 });
    assert.ok(predicate());
  };

  const navigateNear = createNavigator(client, dependencies);
  await navigateNear({ x: 5, y: 1 }, 1);
  assert.equal(client.sent.length, 1);
  assert.deepEqual(timeouts, [60_000]);
  assert.equal(client.diagnostics.length, 0);
  assert.equal(client.failure, undefined);
});

test('navigator never resends an unacknowledged movement with an unknown result', async () => {
  const client = navigationClient();
  client.wait = async () => {
    throw new Error('Timeout waiting for authoritative movement response');
  };

  const navigateNear = createNavigator(client, dependencies);
  await assert.rejects(
    navigateNear({ x: 5, y: 1 }, 1),
    /movement response/,
  );
  assert.equal(client.sent.length, 1);
  assert.equal(client.diagnostics[0].type, 'navigationMovementResponseTimeout');
  assert.equal(client.diagnostics[0].timeoutMs, 60_000);
});

test('navigator rejects a cell only after an unchanged authoritative UserLocation', async () => {
  const client = navigationClient();
  let waits = 0;
  client.wait = async predicate => {
    waits += 1;
    if (waits === 1) {
      client.events.push({
        sequence: client.sequence + 1,
        direction: 'received',
        packet: 'UserLocation',
        payload: { x: 1, y: 1 },
      });
      assert.ok(predicate());
      return;
    }
    const command = client.sent.at(-1);
    const [dx, dy] = directionDelta[command.direction];
    client.snapshot.entities[0].x += dx;
    client.snapshot.entities[0].y += dy;
    assert.ok(predicate());
  };

  const navigateNear = createNavigator(client, dependencies);
  await navigateNear({ x: 5, y: 1 }, 1);

  assert.notDeepEqual(client.sent[1], client.sent[0]);
  assert.equal(client.diagnostics.some(entry =>
    entry.type === 'navigationMovementResponseTimeout'), false);
});

test('navigator waits out paralysis without poisoning a valid route cell', async () => {
  const client = navigationClient();
  let waits = 0;
  let blockedSleeps = 0;
  client.wait = async predicate => {
    waits += 1;
    if (waits === 1) {
      client.snapshot.entities[0].poison = 32;
      client.events.push({
        sequence: client.sequence + 1,
        direction: 'received',
        packet: 'UserLocation',
        payload: { x: 1, y: 1 },
      });
      assert.ok(predicate());
      return;
    }
    Object.assign(client.snapshot.entities[0], { x: 4, y: 1 });
    assert.ok(predicate());
  };

  const navigateNear = createNavigator(client, {
    ...dependencies,
    delay: async () => {
      if (client.snapshot.entities[0].poison === 32) {
        blockedSleeps += 1;
        client.snapshot.entities[0].poison = 0;
      }
    },
  });
  await navigateNear({ x: 5, y: 1 }, 1);

  assert.equal(waits, 2);
  assert.equal(blockedSleeps, 1);
  assert.deepEqual(client.sent[1], client.sent[0], 'the corrected tile remains a valid next step');
  assert.equal(
    client.diagnostics.filter(entry => entry.type === 'navigationMovementControlBlocked').length,
    1,
  );
});

test('navigator optional successful-step cap prevents Run from exceeding the bounded route', async () => {
  const client = navigationClient();
  client.wait = async predicate => {
    client.snapshot.entities[0].x += 1;
    assert.ok(predicate());
  };

  const navigateNear = createNavigator(client, dependencies);
  await assert.rejects(
    navigateNear({ x: 6, y: 1 }, 0, () => false, { maxSuccessfulSteps: 1, maxAttempts: 4 }),
    /Navigation successful step budget exceeded \(1\)/,
  );
  assert.deepEqual(client.sent, [{ type: 'walk', direction: 'Right' }]);
  assert.equal(client.snapshot.entities[0].x, 2);
});

test('navigator detects an authoritative position cycle after the last strict distance improvement', async () => {
  const client = navigationClient();
  const positions = [{ x: 2, y: 1 }, { x: 1, y: 1 }, { x: 2, y: 1 }];
  client.wait = async predicate => {
    Object.assign(client.snapshot.entities[0], positions.shift());
    assert.ok(predicate());
  };

  const navigateNear = createNavigator(client, dependencies);
  await assert.rejects(
    navigateNear({ x: 6, y: 1 }, 0, () => false, {
      detectPositionCycles: true,
      maxNonImprovingSteps: 10,
    }),
    error => error instanceof NavigationStalled &&
      error.code === 'NAVIGATION_STALLED' &&
      error.reason === 'position-cycle' &&
      error.position.x === 2 && error.position.y === 1 &&
      error.bestDistance === 4,
  );
  assert.equal(client.sent.length, 3);
});

test('retreat navigation can disable blocking automatic supply use before its first step', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 50;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.beltItems = [{ name: '(HP)DrugSmall', uniqueId: 0, quantity: 2, container: 'belt' }];
  client.wait = acknowledgeUnitMovement(client);
  let supplyRequests = 0;
  client.request = async () => { supplyRequests += 1; throw new Error('must not block retreat movement'); };

  const navigateNear = createNavigator(client, dependencies);
  const result = await navigateNear({ x: 3, y: 1 }, 0, () => false, {
    autoUseSupplies: false,
  });

  assert.equal(result.reached, true);
  assert.equal(supplyRequests, 0);
  assert.ok(client.sent.every(command => command.type === 'walk' || command.type === 'run'));
});

test('ordinary navigation can recover HP without spending MP before combat', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 50;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.playerMp = 0;
  client.snapshot.playerMaxMp = 30;
  client.snapshot.beltItems = [
    { name: '(HP)DrugSmall', uniqueId: 20, quantity: 2, container: 'belt' },
    { name: '(MP)DrugSmall', uniqueId: 21, quantity: 2, container: 'belt' },
  ];
  client.request = async command => {
    client.sent.push(command);
    if (command.uniqueId === 20) client.snapshot.playerHp = 80;
    if (command.uniqueId === 21) client.snapshot.playerMp = 20;
    return { payload: { success: true } };
  };
  const acknowledgeMovement = acknowledgeUnitMovement(client);
  client.wait = async predicate => {
    if (client.sent.at(-1)?.type === 'useItem') {
      assert.ok(predicate());
      return true;
    }
    return acknowledgeMovement(predicate);
  };

  const navigateNear = createNavigator(client, dependencies);
  const result = await navigateNear({ x: 3, y: 1 }, 0);

  assert.equal(result.reached, true);
  assert.ok(client.sent.some(command => command.type === 'useItem' && command.uniqueId === 20));
  assert.equal(client.sent.some(command => command.type === 'useItem' && command.uniqueId === 21), false);
});

test('critical navigation escapes an adjacent pack before sending another movement intent', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 20;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.entities.push({
    objectId: 9,
    kind: 'monster',
    disposition: 'hostile',
    x: 2,
    y: 1,
    hp: 20,
    dead: false,
  });
  client.wait = acknowledgeUnitMovement(client);
  let escapeCalls = 0;

  const navigateNear = createNavigator(client, {
    ...dependencies,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 3,
    maxEmergencyEscapesPerNavigation: 2,
    emergencyEscape: async owner => {
      escapeCalls += 1;
      Object.assign(owner.snapshot.entities[0], { x: 4, y: 4 });
      owner.snapshot.entities.splice(1);
      owner.snapshot.playerHp = 80;
      return { success: true };
    },
  });
  const result = await navigateNear({ x: 6, y: 4 }, 0);

  assert.equal(result.reached, true);
  assert.equal(escapeCalls, 1);
  assert.equal(client.diagnostics[0].type, 'navigationEmergencyEscapeAttempt');
  assert.equal(client.diagnostics[1].type, 'navigationEmergencyEscapeSuccess');
  assert.ok(client.sent.every(command => command.type === 'walk' || command.type === 'run'));
});

test('a deferred emergency escape becomes eligible again inside the same navigation', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 30;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.entities.push(
    { objectId: 8, kind: 'monster', disposition: 'hostile', x: 1, y: 2, hp: 20, dead: false },
    { objectId: 9, kind: 'monster', disposition: 'hostile', x: 2, y: 1, hp: 20, dead: false },
  );
  let timestamp = 0;
  let escapeCalls = 0;
  client.wait = async predicate => {
    timestamp += 1_000;
    const command = client.sent.at(-1);
    const [dx, dy] = directionDelta[command.direction];
    client.snapshot.entities[0].x += dx;
    client.snapshot.entities[0].y += dy;
    assert.ok(predicate());
  };

  const navigateNear = createNavigator(client, {
    ...dependencies,
    now: () => timestamp,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 3,
    emergencyEscape: async owner => {
      escapeCalls += 1;
      if (escapeCalls === 1) return { deferred: true, retryAfterMs: 1_000 };
      Object.assign(owner.snapshot.entities[0], { x: 6, y: 1 });
      owner.snapshot.entities.splice(1);
      return { success: true };
    },
  });

  const result = await navigateNear({ x: 7, y: 1 }, 0);
  assert.equal(result.reached, true);
  assert.equal(escapeCalls, 2);
  assert.ok(client.diagnostics.some(entry => entry.type === 'navigationEmergencyEscapeDeferred'));
  assert.ok(client.diagnostics.some(entry => entry.type === 'navigationEmergencyEscapeSuccess'));
});

test('moderate navigation damage preserves an escape scroll against one nearby hostile', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 60;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.entities.push({
    objectId: 9,
    kind: 'monster',
    disposition: 'hostile',
    x: 2,
    y: 1,
    hp: 20,
    dead: false,
  });
  client.wait = acknowledgeUnitMovement(client);
  let escapeCalls = 0;

  const navigateNear = createNavigator(client, {
    ...dependencies,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 3,
    maxEmergencyEscapesPerNavigation: 2,
    emergencyEscape: async () => {
      escapeCalls += 1;
      return { success: true };
    },
  });
  const result = await navigateNear({ x: 3, y: 1 }, 0);

  assert.equal(result.reached, true);
  assert.equal(escapeCalls, 0);
  assert.equal(client.diagnostics.length, 0);
});

test('navigator optional attempt cap fails without an unbounded movement retry', async () => {
  const client = navigationClient();
  client.wait = async () => { throw new Error('movement timeout'); };

  const navigateNear = createNavigator(client, dependencies);
  await assert.rejects(
    navigateNear({ x: 6, y: 1 }, 0, () => false, { maxSuccessfulSteps: 3, maxAttempts: 2 }),
    /movement timeout/,
  );
  assert.equal(client.sent.length, 1);
  assert.equal(client.diagnostics.filter(entry =>
    entry.type === 'navigationMovementResponseTimeout').length, 1);
});

test('ordinary navigation detours around a live transfer source', async () => {
  const client = navigationClient();
  client.snapshot.mapTransfers = [{
    key: 'shop-door',
    mapFileName: 'test',
    toMapFileName: 'shop',
    bounds: { minX: 3, maxX: 3, minY: 1, maxY: 1 },
  }];
  const visited = [];
  client.wait = acknowledgeUnitMovement(client, visited);

  const navigateNear = createNavigator(client, dependencies);
  const result = await navigateNear({ x: 6, y: 1 }, 0);

  assert.equal(result.reached, true);
  assert.deepEqual(client.snapshot.entities[0], {
    objectId: 1,
    kind: 'selfPlayer',
    x: 6,
    y: 1,
    dead: false,
  });
  assert.ok(!visited.some(point => point.x === 3 && point.y === 1));
});

test('dispatch recheck blocks a newly observed transfer across both Run cells', async () => {
  const client = navigationClient();
  const visited = [];
  client.wait = acknowledgeUnitMovement(client, visited);
  let exposed = false;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    delay: async () => {
      if (exposed) return;
      exposed = true;
      client.snapshot.mapTransfers = [{
        key: 'late-door',
        mapFileName: 'test',
        toMapFileName: 'other',
        bounds: { minX: 3, maxX: 3, minY: 1, maxY: 1 },
      }];
    },
  });

  const result = await navigateNear({ x: 6, y: 1 }, 0);

  assert.equal(result.reached, true);
  assert.ok(!visited.some(point => point.x === 3 && point.y === 1));
  assert.ok(client.diagnostics.some(entry =>
    entry.type === 'navigationTransferDispatchGuard' &&
    entry.movementType === 'run' && entry.transferCell.x === 3));
});

test('selected live transfer remains an allowed navigation destination', async () => {
  const client = navigationClient();
  client.snapshot.mapTransfers = [{
    key: 'selected-door',
    mapFileName: 'test',
    toMapFileName: 'shop',
    bounds: { minX: 3, maxX: 3, minY: 1, maxY: 1 },
  }];
  client.wait = acknowledgeUnitMovement(client);

  const navigateNear = createNavigator(client, dependencies);
  const result = await navigateNear({ x: 3, y: 1 }, 0, () => false, {
    liveTransferKey: 'selected-door',
  });

  assert.equal(result.reached, true);
  assert.equal(client.snapshot.entities[0].x, 3);
  assert.equal(client.snapshot.entities[0].y, 1);
});

test('intentional map travel keeps an entire same-destination doorway corridor open', async () => {
  const client = navigationClient();
  Object.assign(client.snapshot.entities[0], { x: 3, y: 6 });
  client.snapshot.mapTransfers = [
    {
      key: 'selected-door',
      mapFileName: 'test',
      toMapFileName: 'next-map',
      bounds: { minX: 3, maxX: 3, minY: 1, maxY: 1 },
    },
    {
      key: 'same-door-neighbour',
      mapFileName: 'test',
      toMapFileName: 'next-map',
      bounds: { minX: 3, maxX: 3, minY: 2, maxY: 2 },
    },
  ];
  const corridorMap = openMap();
  corridorMap.blocked.fill(1);
  for (let y = 1; y <= 6; y += 1) corridorMap.blocked[y * corridorMap.width + 3] = 0;
  const visited = [];
  client.wait = acknowledgeUnitMovement(client, visited);

  const navigateNear = createNavigator(client, {
    ...dependencies,
    loadCollisionMap: async () => corridorMap,
  });
  const result = await navigateNear({ x: 3, y: 1 }, 0, () => false, {
    liveTransferKey: 'selected-door',
  });

  assert.equal(result.reached, true);
  assert.ok(visited.some(point => point.x === 3 && point.y === 2));
});

test('hostile clearance keeps spawn-search movement outside visible aggro cells', async () => {
  const client = navigationClient();
  client.snapshot.entities.push({
    objectId: 9,
    kind: 'monster',
    x: 3,
    y: 2,
    hp: 20,
    dead: false,
  });
  const visited = [];
  client.wait = acknowledgeUnitMovement(client, visited);

  const navigateNear = createNavigator(client, dependencies);
  const result = await navigateNear({ x: 6, y: 1 }, 0, () => false, {
    hostileAvoidanceRadius: 1,
  });

  assert.equal(result.reached, true);
  assert.ok(visited.every(point => Math.max(Math.abs(point.x - 3), Math.abs(point.y - 2)) > 1));
});

test('navigator replans before a newly revealed hostile intersects the remaining route', async () => {
  const client = navigationClient();
  let movements = 0;
  client.wait = async predicate => {
    const command = client.sent.at(-1);
    const [dx, dy] = directionDelta[command.direction];
    const self = client.snapshot.entities[0];
    self.x += dx;
    self.y += dy;
    movements += 1;
    if (movements === 1) {
      client.snapshot.entities.push({
        objectId: 9,
        kind: 'monster',
        x: 5,
        y: 2,
        hp: 20,
        dead: false,
      });
    }
    assert.ok(predicate());
  };

  const navigateNear = createNavigator(client, dependencies);
  const result = await navigateNear({ x: 7, y: 1 }, 0, () => false, {
    hostileAvoidanceRadius: 1,
  });

  assert.equal(result.reached, true);
  assert.equal(client.sent[0].direction, 'Right');
  assert.notEqual(client.sent[1].direction, 'Right');
});

test('navigator remembers a recently hidden hostile long enough to avoid route oscillation', async () => {
  const client = navigationClient();
  const visited = [];
  let movements = 0;
  client.wait = async predicate => {
    const command = client.sent.at(-1);
    const [dx, dy] = directionDelta[command.direction];
    const self = client.snapshot.entities[0];
    self.x += dx;
    self.y += dy;
    visited.push({ x: self.x, y: self.y });
    movements += 1;
    if (movements === 1) {
      client.snapshot.entities.push({
        objectId: 9,
        kind: 'monster',
        x: 5,
        y: 2,
        hp: 20,
        dead: false,
      });
    } else if (movements === 2) {
      client.snapshot.entities = client.snapshot.entities.filter(entity => entity.objectId !== 9);
    }
    assert.ok(predicate());
  };

  const navigateNear = createNavigator(client, {
    ...dependencies,
    hostileMemoryDurationMs: 20_000,
  });
  const result = await navigateNear({ x: 7, y: 1 }, 0, () => false, {
    hostileAvoidanceRadius: 1,
  });

  assert.equal(result.reached, true);
  assert.ok(visited.slice(1).every(point =>
    Math.max(Math.abs(point.x - 5), Math.abs(point.y - 2)) > 1));
});

test('zero-clearance travel remembers an exact hidden monster tile across a forced replan', async () => {
  const client = navigationClient();
  Object.assign(client.snapshot.entities[0], { x: 1, y: 3 });
  const visited = [];
  let movements = 0;
  client.wait = async predicate => {
    const command = client.sent.at(-1);
    const [dx, dy] = directionDelta[command.direction];
    const self = client.snapshot.entities[0];
    self.x += dx;
    self.y += dy;
    visited.push({ x: self.x, y: self.y });
    movements += 1;
    if (movements === 1) {
      client.snapshot.entities.push({
        objectId: 9, kind: 'monster', x: 4, y: 3, hp: 20, dead: false,
      });
    } else if (movements === 2) {
      client.snapshot.entities = client.snapshot.entities.filter(entity => entity.objectId !== 9);
      client.snapshot.entities.push({
        objectId: 10, kind: 'monster', x: 4, y: 2, hp: 20, dead: false,
      });
    }
    assert.ok(predicate());
  };

  const navigateNear = createNavigator(client, dependencies);
  const result = await navigateNear({ x: 7, y: 3 }, 0);

  assert.equal(result.reached, true);
  assert.ok(!visited.some(point => point.x === 4 && point.y === 3));
});

test('hostile clearance lets a player already inside the buffer move outward', async () => {
  const client = navigationClient();
  client.snapshot.entities.push({
    objectId: 9,
    kind: 'monster',
    x: 2,
    y: 1,
    hp: 20,
    dead: false,
  });
  const visited = [];
  client.wait = acknowledgeUnitMovement(client, visited);

  const navigateNear = createNavigator(client, dependencies);
  const result = await navigateNear({ x: 1, y: 6 }, 0, () => false, {
    hostileAvoidanceRadius: 2,
  });

  assert.equal(result.reached, true);
  assert.ok(visited.every(point => point.x !== 2 || point.y !== 1));
  assert.ok(visited.every((point, index) => index === 0 ||
    Math.max(Math.abs(point.x - 2), Math.abs(point.y - 1)) >=
      Math.max(Math.abs(visited[index - 1].x - 2), Math.abs(visited[index - 1].y - 1))));
});
