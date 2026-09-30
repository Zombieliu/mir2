import assert from 'node:assert/strict';
import test from 'node:test';

import { createNavigator, NavigationStalled, NavigationStepGuarded, reviveInTown } from './protocol-play.mjs';
import { applyProtocolObservation } from './protocol-observation.mjs';
import { startEmergencyHpRecovery } from './protocol-loadout.mjs';
import { loadProtocolCollisionMap } from './protocol-navigation.mjs';
import { q89WizardFreshCursedShamanRecoveryOptions } from './protocol-survival.mjs';

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

test('town revive waits for authoritative death, then a self Revived and healthy snapshot', async () => {
  const client = navigationClient();
  Object.assign(client.snapshot, { playerHp: 1, playerMaxHp: 100, playerHealthObservation: 'exact' });
  Object.assign(client.snapshot.entities[0], { hp: 1, maxHp: 100, dead: false });
  applyProtocolObservation(client.snapshot, {
    type: 'packet', packet: 'ObjectHealth', payload: { objectId: 1, percent: 0 },
  });
  const requests = [];
  client.request = async (command, expectedPacket) => {
    assert.equal(expectedPacket, 'Revived');
    requests.push(command);
    client.sequence += 1;
    applyProtocolObservation(client.snapshot, { type: 'packet', packet: 'Revived', payload: {} });
    client.events.push({ sequence: client.sequence, direction: 'received', packet: 'Revived', payload: {} });
    return client.events.at(-1);
  };
  client.send = command => {
    client.sent.push(command);
    client.sequence += 1;
    if (command.type === 'clientVersion') {
      client.snapshot = {
        mapFileName: '0', playerObjectId: 1, playerHp: 30, playerMaxHp: 100,
        entities: [{ objectId: 1, kind: 'selfPlayer', x: 1, y: 1, hp: 30, maxHp: 100, dead: false }],
      };
      client.events.push({ sequence: client.sequence, direction: 'received', type: 'worldSnapshot', payload: client.snapshot });
    }
  };
  client.wait = async predicate => assert.equal(predicate(), true);

  assert.equal(await reviveInTown(client), null);
  assert.equal(requests.length, 0, 'rounded zero must not dispatch townRevive');

  applyProtocolObservation(client.snapshot, { type: 'packet', packet: 'Death', payload: {} });
  const revival = await reviveInTown(client);
  assert.equal(requests.length, 1, 'one authoritative death dispatches one townRevive');
  assert.equal(requests[0].type, 'townRevive');
  assert.equal(revival.after.map, '0');
  const revived = client.events.filter(event => event.packet === 'Revived');
  const healthySnapshots = client.events.filter(event => event.type === 'worldSnapshot');
  assert.equal(revived.length, 1);
  assert.equal(healthySnapshots.length, 1);
  assert.ok(healthySnapshots[0].sequence > revived[0].sequence, 'the healthy snapshot must follow self Revived');
});

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

test('navigator reports exact accepted movement and attempt counts when its attempt cap is exhausted', async () => {
  const client = navigationClient();
  client.wait = acknowledgeUnitMovement(client);
  const navigateNear = createNavigator(client, dependencies);

  await assert.rejects(
    navigateNear({ x: 7, y: 1 }, 0, () => false, { maxAttempts: 1, maxSuccessfulSteps: 20 }),
    error => /Navigation attempt budget exceeded \(1\)/.test(error?.message ?? '') &&
      error.successfulSteps === 1 && error.attempts === 1,
  );
  assert.equal(client.sent.length, 1);
});

test('before-movement guard rechecks fresh AOI after cadence and blocks both physical Run cells', async () => {
  const client = navigationClient();
  let guardCall = null;
  const navigate = createNavigator(client, {
    loadCollisionMap: async () => openMap(),
    // The planner has already selected the two-cell Run. The newly rendered
    // hostile arrives during cadence, so only the dispatch-time guard can
    // prevent the second physical cell from entering its halo.
    delay: async () => {
      client.snapshot.entities.push({
        objectId: 71, kind: 'monster', name: 'CursedShaman', x: 3, y: 1,
        hp: 205, dead: false, disposition: 'hostile',
      });
    },
    now: () => 10_000,
  });

  await assert.rejects(
    () => navigate({ x: 6, y: 1 }, 0, () => false, {
      beforeMovement: context => {
        guardCall = context;
        const offending = context.physicalCells.find(cell => cell.x === 3 && cell.y === 1);
        return offending ? { type: 'protectedTransitShamanHalo', blockerObjectId: 71 } : null;
      },
    }),
    error => error instanceof NavigationStepGuarded && error.hazard.blockerObjectId === 71,
  );
  assert.deepEqual(guardCall.physicalCells, [{ x: 2, y: 1 }, { x: 3, y: 1 }]);
  assert.equal(guardCall.movementType, 'run');
  assert.deepEqual(client.sent, [], 'guard fires before the Run packet is emitted');
  assert.equal(client.snapshot.entities[0].x, 1);
  assert.equal(client.snapshot.entities[0].y, 1);
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

test('death immediately interrupts an in-flight movement without reporting a response timeout', async () => {
  const client = navigationClient();
  client.wait = async predicate => {
    client.snapshot.playerHp = 0;
    client.snapshot.entities[0].dead = true;
    client.events.push({
      sequence: client.sequence + 1,
      direction: 'received',
      packet: 'Death',
      payload: { objectId: 1 },
    });
    assert.ok(predicate(), 'the Death packet must release the movement wait immediately');
  };

  const navigateNear = createNavigator(client, dependencies);
  await assert.rejects(
    navigateNear({ x: 5, y: 1 }, 1),
    /Player died during navigation/,
  );
  assert.equal(client.sent.length, 1);
  assert.equal(client.diagnostics.some(entry =>
    entry.type === 'navigationMovementResponseTimeout'), false);
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

test('paralysis waits do not spend the bounded movement-attempt allowance', async () => {
  const client = navigationClient();
  client.snapshot.entities[0].poison = 32;
  client.wait = acknowledgeUnitMovement(client);
  let blockedWaits = 0;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    delay: async () => {
      if (client.snapshot.entities[0].poison === 32 && ++blockedWaits === 10) {
        client.snapshot.entities[0].poison = 0;
      }
    },
  });
  const result = await navigateNear({ x: 2, y: 1 }, 0, () => false, { maxAttempts: 2 });
  assert.equal(result.reached, true);
  assert.equal(blockedWaits, 10);
  assert.equal(client.sent.length, 1, 'the blocked interval sends no movement');
});

test('permanent movement control still stops after a separate bounded wait', async () => {
  const client = navigationClient();
  client.snapshot.entities[0].poison = 32;
  const navigateNear = createNavigator(client, dependencies);
  await assert.rejects(
    navigateNear({ x: 2, y: 1 }, 0, () => false, { maxAttempts: 2 }),
    /Movement control remained blocked after 20 bounded waits/,
  );
  assert.deepEqual(client.sent, []);
});

test('navigator does not dispatch movement when paralysis arrives during cadence wait', async () => {
  const client = navigationClient();
  let injected = false;
  let resumed = false;
  client.wait = async predicate => {
    assert.ok(resumed, 'movement must wait for the explicit control release');
    assert.equal(client.snapshot.entities[0].poison, 0);
    const command = client.sent.at(-1);
    const [dx, dy] = directionDelta[command.direction];
    const scale = command.type === 'run' ? 2 : 1;
    client.snapshot.entities[0].x += dx * scale;
    client.snapshot.entities[0].y += dy * scale;
    assert.ok(predicate());
  };
  const navigateNear = createNavigator(client, {
    ...dependencies,
    delay: async () => {
      if (!injected) {
        injected = true;
        client.snapshot.entities[0].poison = 32;
      } else if (client.snapshot.entities[0].poison === 32) {
        assert.deepEqual(client.sent, [], 'no command may escape the cadence guard');
        client.snapshot.entities[0].poison = 0;
        resumed = true;
      }
    },
  });
  await navigateNear({ x: 5, y: 1 }, 1);
  assert.equal(client.snapshot.entities[0].x, 4);
  assert.equal(client.diagnostics.filter(entry => entry.type === 'navigationMovementControlBlocked').length, 1);
});

test('navigator stops when death arrives during cadence wait', async () => {
  const client = navigationClient();
  client.wait = async () => assert.fail('dead player must not await a movement response');
  const navigateNear = createNavigator(client, {
    ...dependencies,
    delay: async () => { client.snapshot.entities[0].dead = true; },
  });
  await assert.rejects(navigateNear({ x: 5, y: 1 }, 1), /Player died during navigation/);
  assert.deepEqual(client.sent, []);
});

test('navigator replans when an earlier authoritative response lands during cadence wait', async () => {
  const client = navigationClient();
  let cadenceCalls = 0;
  let waitCalls = 0;
  client.wait = async predicate => {
    waitCalls += 1;
    const command = client.sent.at(-1);
    const [dx, dy] = directionDelta[command.direction];
    const scale = command.type === 'run' ? 2 : 1;
    client.snapshot.entities[0].x += dx * scale;
    client.snapshot.entities[0].y += dy * scale;
    assert.ok(predicate());
  };

  const navigateNear = createNavigator(client, {
    ...dependencies,
    delay: async () => {
      cadenceCalls += 1;
      if (cadenceCalls === 1) {
        // Simulate a prior movement ACK arriving after this route was planned
        // but before its next movement is dispatched.
        client.snapshot.entities[0].x = 2;
      }
    },
  });
  await navigateNear({ x: 5, y: 1 }, 1);

  assert.equal(waitCalls, 1, 'the stale planned step must not be sent or awaited');
  assert.deepEqual(client.sent, [{ type: 'run', direction: 'Right' }]);
  assert.equal(client.snapshot.entities[0].x, 4);
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

test('authoritative safe-zone residents do not trigger emergency escape', async () => {
  const client = navigationClient();
  client.snapshot.inSafeZone = true;
  client.snapshot.playerHp = 20;
  client.snapshot.playerMaxHp = 100;
  const now = new Date().toISOString();
  client.events.push(
    { packet: 'ObjectStruck', direction: 'received', sequence: 4, payload: { objectId: 1, attackerId: 7001 }, at: now },
    { packet: 'MapInformation', direction: 'received', sequence: 5, payload: { fileName: '0' }, at: now },
  );
  client.snapshot.entities.push(
    { objectId: 8, kind: 'monster', disposition: 'neutral', name: 'Guard', x: 2, y: 1, hp: 20, dead: false },
    { objectId: 9, kind: 'monster', disposition: 'neutral', name: 'Deer', x: 1, y: 2, hp: 20, dead: false },
    { objectId: 10, kind: 'monster', disposition: 'neutral', name: 'Scarecrow', x: 2, y: 2, hp: 20, dead: false },
  );
  client.wait = acknowledgeUnitMovement(client);
  let escapeCalls = 0;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    now: Date.now,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 3,
    maxEmergencyEscapesPerNavigation: 2,
    emergencyEscape: async () => { escapeCalls += 1; return { success: true }; },
  });

  const result = await navigateNear({ x: 3, y: 1 }, 0);

  assert.equal(result.reached, true);
  assert.equal(escapeCalls, 0);
  assert.equal(client.diagnostics.length, 0);
});

test('a fresh explicit player-hit receipt still permits safe-zone emergency escape', async () => {
  const client = navigationClient();
  client.snapshot.inSafeZone = true;
  client.snapshot.playerHp = 20;
  client.snapshot.playerMaxHp = 100;
  client.events.push({
    packet: 'ObjectStruck',
    direction: 'received',
    sequence: 2,
    payload: { objectId: 1, attackerId: 7001 },
    at: new Date().toISOString(),
  });
  client.events.unshift({
    packet: 'MapInformation',
    direction: 'received',
    sequence: 1,
    payload: { fileName: '0' },
    at: new Date().toISOString(),
  });
  client.wait = acknowledgeUnitMovement(client);
  let escapeCalls = 0;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    now: Date.now,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 3,
    maxEmergencyEscapesPerNavigation: 2,
    emergencyEscape: async owner => {
      escapeCalls += 1;
      owner.snapshot.mapFileName = '0';
      owner.snapshot.entities[0].x = 4;
      owner.snapshot.entities[0].y = 4;
      return { success: true };
    },
  });

  const result = await navigateNear({ x: 6, y: 4 }, 0);

  assert.equal(result.reached, false);
  assert.equal(escapeCalls, 1);
  assert.equal(client.snapshot.mapFileName, '0');
  assert.equal(client.diagnostics[0].safeZoneAttackReceipt, true);
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

test('q89 proven-aggressor navigation continues at 53 percent through one Shaman and a passive zombie', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 53;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.entities.push(
    { objectId: 9, kind: 'monster', disposition: 'hostile', x: 5, y: 1, hp: 20, dead: false },
    { objectId: 10, kind: 'monster', disposition: 'hostile', x: 5, y: 2, hp: 20, dead: false },
  );
  client.events.push(
    { packet: 'MapInformation', direction: 'received', sequence: 1, payload: { fileName: 'test' }, at: '1970-01-01T00:00:05.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 2, payload: { objectId: 1, attackerId: 9 }, at: '1970-01-01T00:00:09.000Z' },
  );
  client.wait = acknowledgeUnitMovement(client);
  let escapeCalls = 0;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 6,
    emergencyEscapeRequiresProvenAggressors: true,
    emergencyEscape: async () => { escapeCalls += 1; return { success: true }; },
  });

  const result = await navigateNear({ x: 3, y: 1 }, 0);

  assert.equal(result.reached, true);
  assert.equal(escapeCalls, 0);
  assert.ok(client.sent.some(command => command.type === 'walk' || command.type === 'run'));
  assert.equal(client.diagnostics.some(entry => entry.type === 'navigationEmergencyEscapeAttempt'), false);
});

test('q89 Wizard drinks one Medium immediately after the first fresh CursedShaman hit', async () => {
  const client = navigationClient();
  client.snapshot.beltItems = [{ name: '(HP)DrugMedium', uniqueId: 74, quantity: 4, container: 'belt' }];
  client.snapshot.questLog = [{ questId: 89, stage: 'inProgress' }];
  client.snapshot.entities[0].class = 'Wizard';
  client.snapshot.entities.push({
    objectId: 9, kind: 'monster', name: 'CursedShaman', disposition: 'hostile',
    x: 5, y: 1, hp: 205, dead: false,
  });
  client.events.push({
    packet: 'MapInformation', direction: 'received', sequence: 1,
    payload: { fileName: 'test' }, at: '1970-01-01T00:00:05.000Z',
  });
  const damageAtUse = [];
  const acknowledgeMovement = acknowledgeUnitMovement(client);
  let hits = 0;
  client.send = function send(command) {
    this.sent.push(command);
    this.sequence += 1;
    if (command.type === 'useItem') damageAtUse.push(this.snapshot.playerHp);
  };
  client.wait = async predicate => {
    await acknowledgeMovement(predicate);
    if (hits >= 5) return true;
    hits += 1;
    client.snapshot.playerHp = 100 - hits * 15;
    client.events.push({
      packet: 'ObjectStruck', direction: 'received', sequence: hits + 1,
      payload: { objectId: 1, attackerId: 9 }, at: '1970-01-01T00:00:09.900Z',
    });
    return true;
  };
  const navigateNear = createNavigator(client, {
    loadCollisionMap: async () => ({ ...openMap(), width: 32, height: 8, blocked: new Uint8Array(256) }),
    delay: async () => {}, now: () => 10_000,
  });

  const result = await navigateNear({ x: 12, y: 1 }, 0, () => false, {
    emergencyEscapeRequiresProvenAggressors: true,
    onFreshDirectHit: (owner, { attacker }) => {
      const options = q89WizardFreshCursedShamanRecoveryOptions(owner.snapshot, attacker, 'Wizard');
      if (options) startEmergencyHpRecovery(owner, options);
    },
  });

  assert.equal(result.reached, true);
  assert.equal(hits, 5);
  assert.deepEqual(damageAtUse, [85]);
  assert.equal(client.sent.filter(command => command.type === 'useItem' && command.uniqueId === 74).length, 1);
});

test('fresh-hit recovery does not redrink a receipt revisited during navigation', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 85;
  client.snapshot.beltItems = [{ name: '(HP)DrugMedium', uniqueId: 74, quantity: 4, container: 'belt' }];
  client.snapshot.questLog = [{ questId: 89, stage: 'inProgress' }];
  client.snapshot.entities.push({
    objectId: 9, kind: 'monster', name: 'CursedShaman', disposition: 'hostile',
    x: 5, y: 1, hp: 205, dead: false,
  });
  client.events.push(
    { packet: 'MapInformation', direction: 'received', sequence: 1, payload: { fileName: 'test' }, at: '1970-01-01T00:00:05.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 2, payload: { objectId: 1, attackerId: 9 }, at: '1970-01-01T00:00:09.900Z' },
  );
  client.wait = acknowledgeUnitMovement(client);
  const navigateNear = createNavigator(client, {
    ...dependencies,
    loadCollisionMap: async () => ({ ...openMap(), width: 32, height: 8, blocked: new Uint8Array(256) }),
  });

  const result = await navigateNear({ x: 12, y: 1 }, 0, () => false, {
    onFreshDirectHit: (owner, { attacker }) => {
      const options = q89WizardFreshCursedShamanRecoveryOptions(owner.snapshot, attacker, 'Wizard');
      if (options) startEmergencyHpRecovery(owner, options);
    },
  });

  assert.equal(result.reached, true);
  assert.equal(client.sent.filter(command => command.type === 'useItem' && command.uniqueId === 74).length, 1);
});

test('fresh-hit recovery rejects prior-map and wrong-owner receipts', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 85;
  client.snapshot.entities.push({
    objectId: 9, kind: 'monster', name: 'CursedShaman', disposition: 'hostile',
    x: 5, y: 1, hp: 205, dead: false,
  });
  client.events.push(
    { packet: 'MapInformation', direction: 'received', sequence: 1, payload: { fileName: 'old-map' }, at: '1970-01-01T00:00:05.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 2, payload: { objectId: 1, attackerId: 9 }, at: '1970-01-01T00:00:09.900Z' },
    { packet: 'MapInformation', direction: 'received', sequence: 3, payload: { fileName: 'test' }, at: '1970-01-01T00:00:09.910Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 4, payload: { objectId: 99, attackerId: 9 }, at: '1970-01-01T00:00:09.920Z' },
  );
  client.wait = acknowledgeUnitMovement(client);
  let recoveryCalls = 0;
  const navigateNear = createNavigator(client, dependencies);

  const result = await navigateNear({ x: 3, y: 1 }, 0, () => false, {
    onFreshDirectHit: () => { recoveryCalls += 1; },
  });

  assert.equal(result.reached, true);
  assert.equal(recoveryCalls, 0);
});

test('q89 proven-aggressor navigation escapes at 65 percent only after two fresh direct hits', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 53;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.entities.push(
    { objectId: 9, kind: 'monster', disposition: 'hostile', x: 5, y: 1, hp: 20, dead: false },
    { objectId: 10, kind: 'monster', disposition: 'hostile', x: 5, y: 2, hp: 20, dead: false },
  );
  client.events.push(
    { packet: 'MapInformation', direction: 'received', sequence: 1, payload: { fileName: 'test' }, at: '1970-01-01T00:00:05.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 2, payload: { objectId: 1, attackerId: 9 }, at: '1970-01-01T00:00:09.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 3, payload: { objectId: 1, attackerId: 10 }, at: '1970-01-01T00:00:09.500Z' },
  );
  client.wait = acknowledgeUnitMovement(client);
  let escapeCalls = 0;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 6,
    emergencyEscapeRequiresProvenAggressors: true,
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
  assert.equal(client.diagnostics[0].provenNearby, 2);
});

test('q89 proven-aggressor navigation preserves its one-attacker critical-floor escape', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 25;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.entities.push({
    objectId: 9, kind: 'monster', disposition: 'hostile', x: 5, y: 1, hp: 20, dead: false,
  });
  client.events.push(
    { packet: 'MapInformation', direction: 'received', sequence: 1, payload: { fileName: 'test' }, at: '1970-01-01T00:00:05.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 2, payload: { objectId: 1, attackerId: 9 }, at: '1970-01-01T00:00:09.000Z' },
  );
  client.wait = acknowledgeUnitMovement(client);
  let escapeCalls = 0;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 6,
    emergencyEscapeRequiresProvenAggressors: true,
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
  assert.equal(client.diagnostics[0].provenNearby, 1);
});

test('q89 proven-aggressor navigation ignores receipts from the prior map', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 53;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.entities.push(
    { objectId: 9, kind: 'monster', disposition: 'hostile', x: 5, y: 1, hp: 20, dead: false },
    { objectId: 10, kind: 'monster', disposition: 'hostile', x: 5, y: 2, hp: 20, dead: false },
  );
  client.events.push(
    { packet: 'MapInformation', direction: 'received', sequence: 1, payload: { fileName: 'old-map' }, at: '1970-01-01T00:00:05.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 2, payload: { objectId: 1, attackerId: 9 }, at: '1970-01-01T00:00:09.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 3, payload: { objectId: 1, attackerId: 10 }, at: '1970-01-01T00:00:09.100Z' },
    { packet: 'MapInformation', direction: 'received', sequence: 4, payload: { fileName: 'test' }, at: '1970-01-01T00:00:09.200Z' },
  );
  client.wait = acknowledgeUnitMovement(client);
  let escapeCalls = 0;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 6,
    emergencyEscapeRequiresProvenAggressors: true,
    emergencyEscape: async () => { escapeCalls += 1; return { success: true }; },
  });

  const result = await navigateNear({ x: 3, y: 1 }, 0);

  assert.equal(result.reached, true);
  assert.equal(escapeCalls, 0);
  assert.equal(client.diagnostics.some(entry => entry.type === 'navigationEmergencyEscapeAttempt'), false);
});

test('q89 proven-aggressor navigation ignores fresh hits before the current player died and revived', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 53;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.entities.push(
    { objectId: 9, kind: 'monster', disposition: 'hostile', x: 5, y: 1, hp: 20, dead: false },
    { objectId: 10, kind: 'monster', disposition: 'hostile', x: 5, y: 2, hp: 20, dead: false },
  );
  client.events.push(
    { packet: 'MapInformation', direction: 'received', sequence: 1, payload: { fileName: 'test' }, at: '1970-01-01T00:00:05.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 2, payload: { objectId: 1, attackerId: 9 }, at: '1970-01-01T00:00:09.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 3, payload: { objectId: 1, attackerId: 10 }, at: '1970-01-01T00:00:09.100Z' },
    { packet: 'ObjectDied', direction: 'received', sequence: 4, payload: { objectId: 999 }, at: '1970-01-01T00:00:09.200Z' },
    { packet: 'Death', direction: 'received', sequence: 5, payload: { location: { x: 1, y: 1 } }, at: '1970-01-01T00:00:09.300Z' },
    { packet: 'ObjectDied', direction: 'received', sequence: 6, payload: { objectId: 1 }, at: '1970-01-01T00:00:09.400Z' },
    { packet: 'ObjectRevived', direction: 'received', sequence: 7, payload: { objectId: 999, effect: true }, at: '1970-01-01T00:00:09.500Z' },
    { packet: 'ObjectRevived', direction: 'received', sequence: 8, payload: { objectId: 1, effect: true }, at: '1970-01-01T00:00:09.600Z' },
    { packet: 'Revived', direction: 'received', sequence: 9, payload: {}, at: '1970-01-01T00:00:09.700Z' },
  );
  client.sequence = 9;
  client.wait = acknowledgeUnitMovement(client);
  let escapeCalls = 0;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 6,
    emergencyEscapeRequiresProvenAggressors: true,
    emergencyEscape: async () => { escapeCalls += 1; return { success: true }; },
  });

  const result = await navigateNear({ x: 3, y: 1 }, 0);

  assert.equal(result.reached, true);
  assert.equal(escapeCalls, 0);
  assert.equal(client.diagnostics.some(entry => entry.type === 'navigationEmergencyEscapeAttempt'), false);
});

test('q89 proven-aggressor navigation ignores stale, dead, and wrong-recipient receipts', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 53;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.entities.push(
    { objectId: 9, kind: 'monster', disposition: 'hostile', x: 5, y: 1, hp: 20, dead: false },
    { objectId: 10, kind: 'monster', disposition: 'hostile', x: 5, y: 2, hp: 0, dead: true },
    { objectId: 11, kind: 'monster', disposition: 'hostile', x: 5, y: 3, hp: 20, dead: false },
    { objectId: 12, kind: 'monster', disposition: 'hostile', x: 5, y: 4, hp: 20, dead: false },
  );
  client.events.push(
    { packet: 'MapInformation', direction: 'received', sequence: 1, payload: { fileName: 'test' }, at: '1970-01-01T00:00:01.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 2, payload: { objectId: 1, attackerId: 9 }, at: '1970-01-01T00:00:04.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 3, payload: { objectId: 1, attackerId: 10 }, at: '1970-01-01T00:00:09.000Z' },
    { packet: 'ObjectStruck', direction: 'received', sequence: 4, payload: { objectId: 999, attackerId: 11 }, at: '1970-01-01T00:00:09.100Z' },
  );
  client.wait = acknowledgeUnitMovement(client);
  let escapeCalls = 0;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 6,
    emergencyEscapeRequiresProvenAggressors: true,
    emergencyEscape: async () => { escapeCalls += 1; return { success: true }; },
  });

  const result = await navigateNear({ x: 3, y: 1 }, 0);

  assert.equal(result.reached, true);
  assert.equal(escapeCalls, 0);
  assert.equal(client.diagnostics.some(entry => entry.type === 'navigationEmergencyEscapeAttempt'), false);
});

test('the default navigator still escapes a passive two-hostile pack at 53 percent', async () => {
  const client = navigationClient();
  client.snapshot.playerHp = 53;
  client.snapshot.playerMaxHp = 100;
  client.snapshot.entities.push(
    { objectId: 9, kind: 'monster', disposition: 'hostile', x: 5, y: 1, hp: 20, dead: false },
    { objectId: 10, kind: 'monster', disposition: 'hostile', x: 5, y: 2, hp: 20, dead: false },
  );
  client.wait = acknowledgeUnitMovement(client);
  let escapeCalls = 0;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscapeDangerDistance: 6,
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

test('transfer dispatch no-path retains completed movement count on its original Error', async () => {
  const client = navigationClient();
  let exposed = false;
  const navigateNear = createNavigator(client, {
    ...dependencies,
    delay: async () => {
      if (exposed) return;
      exposed = true;
      client.snapshot.mapTransfers = [{
        key: 'late-door', mapFileName: 'test', toMapFileName: 'other',
        bounds: { minX: 3, maxX: 3, minY: 1, maxY: 1 },
      }];
    },
  });

  await assert.rejects(
    navigateNear({ x: 6, y: 1 }, 0, () => false, { maxNoPathRefreshes: 0 }),
    error => /No walk path on test from 1,1 to 6,1/.test(error?.message ?? '') && error.successfulSteps === 0,
  );
  assert.deepEqual(client.sent, []);
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

test('a two-cell hostile buffer detours around a reachable melee neighbor', async () => {
  const run = async hostileAvoidanceRadius => {
    const client = navigationClient();
    client.snapshot.entities.push({
      objectId: 9, kind: 'monster', name: 'BlackBoar', disposition: 'hostile',
      x: 4, y: 2, hp: 280, maxHp: 280, dead: false,
    });
    const visited = [];
    client.wait = acknowledgeUnitMovement(client, visited);
    const result = await createNavigator(client, dependencies)(
      { x: 7, y: 1 }, 0, () => false, { hostileAvoidanceRadius },
    );
    assert.equal(result.reached, true);
    return visited;
  };

  const ordinary = await run(0);
  const buffered = await run(2);
  const distanceToBlackBoar = point => Math.max(Math.abs(point.x - 4), Math.abs(point.y - 2));
  assert.ok(ordinary.some(point => distanceToBlackBoar(point) <= 1));
  assert.ok(buffered.every(point => distanceToBlackBoar(point) > 2));
});

test('the public pre-hit D713 fixture plans with radius two without calling it route acceptance', async () => {
  // Sanitized from Wizard trace sequence 10600, before BlackBoar 466601's
  // first 22-damage receipt at sequence 10614. The transfer is the live D713
  // -> D714 source observed in the same snapshot. The static fixture has no
  // proof that this route caused the later hit, so only assert the planner's
  // selected two-cell buffer. The separate synthetic detour test above
  // supplies the radius-zero counterfactual; it is not actual-map acceptance.
  const transfer = {
    key: 'crystal-move:d713:350:244:203:382:190', mapFileName: 'D713', toMapFileName: 'D714',
    bounds: { minX: 350, maxX: 350, minY: 244, maxY: 244 },
  };
  const client = navigationClient();
  Object.assign(client.snapshot, {
    mapFileName: 'D713',
    mapTransfers: [transfer],
    entities: [
      { objectId: 1, kind: 'selfPlayer', x: 247, y: 229, hp: 105, maxHp: 105, dead: false },
      { objectId: 466601, kind: 'monster', name: 'BlackBoar', disposition: 'hostile', x: 243, y: 234, hp: 280, maxHp: 280, dead: false },
    ],
  });
  const visited = [];
  client.wait = acknowledgeUnitMovement(client, visited);
  const result = await createNavigator(client, {
    ...dependencies,
    loadCollisionMap: loadProtocolCollisionMap,
  })({ x: 350, y: 244 }, 0, () => false, {
    liveTransferKey: transfer.key,
    hostileAvoidanceRadius: 2,
    maxNoPathRefreshes: 0,
  });

  assert.equal(result.reached, true);
  assert.ok(visited.every(point => Math.max(Math.abs(point.x - 243), Math.abs(point.y - 234)) > 2));
});

test('named hostile clearance protects the Shaman band without widening a zombie route', async () => {
  const client = navigationClient();
  Object.assign(client.snapshot.entities[0], { x: 1, y: 5 });
  client.snapshot.entities.push(
    { objectId: 9, kind: 'monster', name: 'CursedShaman', x: 3, y: 2, hp: 20, dead: false },
    { objectId: 10, kind: 'monster', name: 'CursedZombie', x: 3, y: 5, hp: 20, dead: false },
  );
  const visited = [];
  client.wait = acknowledgeUnitMovement(client, visited);

  const navigateNear = createNavigator(client, dependencies);
  const result = await navigateNear({ x: 7, y: 3 }, 0, () => false, {
    hostileAvoidanceRadius: 0,
    hostileAvoidanceByName: { CursedShaman: 2 },
  });

  assert.equal(result.reached, true);
  assert.ok(visited.every(point => Math.max(Math.abs(point.x - 3), Math.abs(point.y - 2)) > 2));
  assert.ok(visited.some(point => Math.max(Math.abs(point.x - 3), Math.abs(point.y - 5)) === 1));
});

test('the q89 D2031 entry navigator refuses a waypoint sealed by both Shaman footprints', async () => {
  const client = navigationClient();
  client.snapshot.mapFileName = 'D2031';
  Object.assign(client.snapshot.entities[0], { x: 278, y: 284 });
  client.snapshot.entities.push(
    {
      objectId: 341100, kind: 'monster', name: 'CursedShaman',
      x: 263, y: 273, hp: 100, dead: false, disposition: 'hostile',
    },
    {
      objectId: 341105, kind: 'monster', name: 'CursedShaman0',
      x: 275, y: 270, hp: 100, dead: false, disposition: 'hostile',
    },
  );
  const navigateNear = createNavigator(client, {
    ...dependencies,
    loadCollisionMap: loadProtocolCollisionMap,
  });

  await assert.rejects(
    navigateNear({ x: 250, y: 270 }, 4, () => false, {
      hostileAvoidanceRadius: 1,
      hostileAvoidanceByName: { CursedShaman: 6, CursedShaman0: 6 },
      maxNoPathRefreshes: 0,
    }),
    error => /No walk path on D2031 from 278,284 to 250,270/.test(error?.message ?? '') &&
      error.successfulSteps === 0,
  );
  assert.deepEqual(client.sent, []);
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

function longCorridorMap() {
  const width = 80;
  const height = 3;
  const blocked = new Uint8Array(width * height).fill(1);
  for (let x = 0; x < width; x += 1) blocked[width + x] = 0;
  return { mapFileName: 'test', sourcePath: 'corridor.map', width, height, blocked };
}

async function navigatorWithRememberedCorridorBlocker() {
  const client = navigationClient();
  Object.assign(client.snapshot.entities[0], { x: 1, y: 1 });
  client.snapshot.entities.push({
    objectId: 9, kind: 'monster', name: 'BlackMaggot', disposition: 'hostile',
    hp: 200, dead: false, x: 50, y: 1,
  });
  client.wait = acknowledgeUnitMovement(client);
  const navigateNear = createNavigator(client, {
    ...dependencies,
    loadCollisionMap: async () => longCorridorMap(),
  });
  await navigateNear({ x: 20, y: 1 }, 0, () => false, {
    hostileAvoidanceRadius: 2,
    detectPositionCycles: true,
  });
  // Combat can make one final ordinary step after navigator completion before
  // a scroll. The reset must trust the explicit owner proof, not require this
  // cached navigator coordinate to be identical to the escape origin.
  Object.assign(client.snapshot.entities[0], { x: 22, y: 1 });
  return { client, navigateNear };
}

function publishRelocationSnapshot(client) {
  client.events.push({
    sequence: ++client.sequence,
    direction: 'received',
    type: 'worldSnapshot',
    payload: structuredClone(client.snapshot),
  });
}

function sameMapEmergencyProof(before, to) {
  return {
    quantityBefore: 3,
    quantityAfter: 2,
    from: before,
    to,
    fromMapFileName: 'test',
    toMapFileName: 'test',
  };
}

test('a proven same-map emergency relocation clears only stale hostile memory before the next route', async () => {
  const { client, navigateNear } = await navigatorWithRememberedCorridorBlocker();
  const beforeSequence = client.sequence;
  const before = { mapFileName: 'test', objectId: 1, x: 22, y: 1 };
  client.snapshot.entities = [{ objectId: 1, kind: 'selfPlayer', x: 46, y: 1, dead: false }];
  publishRelocationSnapshot(client);

  assert.equal(navigateNear.resetHostileMemoryAfterVerifiedEmergencyRelocation({
    beforeSequence,
    before,
    relocation: sameMapEmergencyProof(before, { x: 46, y: 1 }),
  }), true);
  await navigateNear({ x: 70, y: 1 }, 0, () => false, {
    hostileAvoidanceRadius: 2,
    detectPositionCycles: true,
  });
  assert.equal(client.snapshot.entities[0].x, 70);
  assert.ok(client.diagnostics.some(entry =>
    entry.type === 'navigationHostileMemoryResetAfterEmergencyRelocation' && entry.distance === 24));
});

test('a proven relocation preserves the current live hostile buffer', async () => {
  const { client, navigateNear } = await navigatorWithRememberedCorridorBlocker();
  const beforeSequence = client.sequence;
  const before = { mapFileName: 'test', objectId: 1, x: 22, y: 1 };
  client.snapshot.entities = [
    { objectId: 1, kind: 'selfPlayer', x: 46, y: 1, dead: false },
    { objectId: 10, kind: 'monster', name: 'BlackMaggot', disposition: 'hostile', hp: 200, dead: false, x: 50, y: 1 },
  ];
  publishRelocationSnapshot(client);

  assert.equal(navigateNear.resetHostileMemoryAfterVerifiedEmergencyRelocation({
    beforeSequence,
    before,
    relocation: sameMapEmergencyProof(before, { x: 46, y: 1 }),
  }), true);
  await assert.rejects(
    () => navigateNear({ x: 70, y: 1 }, 0, () => false, {
      hostileAvoidanceRadius: 2,
      detectPositionCycles: true,
      maxNoPathRefreshes: 0,
    }),
    /No walk path on test from 46,1 to 70,1/,
  );
});

test('an unproved or different-owner relocation retains hostile memory', async () => {
  const cases = [
    {
      name: 'no fresh snapshot',
      update: client => {
        client.snapshot.entities = [{ objectId: 1, kind: 'selfPlayer', x: 46, y: 1, dead: false }];
      },
    },
    {
      name: 'different owner snapshot',
      update: client => {
        client.snapshot.playerObjectId = 2;
        client.snapshot.entities = [{ objectId: 2, kind: 'selfPlayer', x: 46, y: 1, dead: false }];
        publishRelocationSnapshot(client);
      },
    },
    {
      name: 'pending snapshot',
      update: client => {
        client.snapshot.entities = [{ objectId: 1, kind: 'selfPlayer', x: 46, y: 1, dead: false }];
        client.snapshot.mapSnapshotPending = true;
        publishRelocationSnapshot(client);
      },
      afterProof: client => { client.snapshot.mapSnapshotPending = false; },
    },
    {
      name: 'unknown scroll origin',
      update: client => {
        client.snapshot.entities = [{ objectId: 1, kind: 'selfPlayer', x: 46, y: 1, dead: false }];
        publishRelocationSnapshot(client);
      },
      proof: before => ({ ...sameMapEmergencyProof(before, { x: 46, y: 1 }), from: { x: null, y: 1 } }),
    },
    {
      name: 'mismatched scroll origin',
      update: client => {
        client.snapshot.entities = [{ objectId: 1, kind: 'selfPlayer', x: 46, y: 1, dead: false }];
        publishRelocationSnapshot(client);
      },
      proof: before => ({ ...sameMapEmergencyProof(before, { x: 46, y: 1 }), from: { x: 21, y: 1 } }),
    },
  ];
  for (const fixture of cases) {
    const { client, navigateNear } = await navigatorWithRememberedCorridorBlocker();
    const beforeSequence = client.sequence;
    const before = { mapFileName: 'test', objectId: 1, x: 22, y: 1 };
    fixture.update(client);
    assert.equal(navigateNear.resetHostileMemoryAfterVerifiedEmergencyRelocation({
      beforeSequence,
      before,
      relocation: fixture.proof?.(before) ?? sameMapEmergencyProof(before, { x: 46, y: 1 }),
    }), false, fixture.name);
    fixture.afterProof?.(client);
    await assert.rejects(
      () => navigateNear({ x: 70, y: 1 }, 0, () => false, {
        hostileAvoidanceRadius: 2,
        detectPositionCycles: true,
        maxNoPathRefreshes: 0,
      }),
      /No walk path on test from 46,1 to 70,1/,
      fixture.name,
    );
  }
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
