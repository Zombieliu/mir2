import assert from "node:assert/strict";
import test from "node:test";

import {
  compareRetreatCandidates,
  clearTravelBlockingMonster,
  completeQuestObjectives,
  firstReachableDestination,
  harvestDirection,
  provenAggressors,
  resolveObjectiveMapFallbackDestination,
  unsafeRetreatIsSafe,
} from "./protocol-combat.mjs";
import { createWizardKitingAction, RangedSafetyBandUnavailable } from './protocol-kiting.mjs';
import { createNavigator, NavigationStalled, NavigationStepGuarded } from "./protocol-play.mjs";
import { loadProtocolCollisionMap } from './protocol-navigation.mjs';
import { questRetreatProfile } from './protocol-survival.mjs';

test("explicit objective map preference resolves equal-length dungeon routes", async () => {
  const travel = async () => {};
  travel.routeLength = async mapFileName => ({ D422: 3, D406: 3 })[mapFileName] ?? null;
  const destination = await firstReachableDestination([
    { mapFileName: "D422", position: { x: 200, y: 200 } },
    { mapFileName: "D406", position: { x: 100, y: 100 } },
  ], travel, ["D406"]);
  assert.equal(destination.mapFileName, "D406");
});

test("armed q89 fallback accepts a profile-pruned route only with the exact live doorway", () => {
  const fallback = { fromMapFileName: "D2031", toMapFileName: "D2032", transferCandidates: [
    { key: "crystal-move:d2031:198:34:117:184:267", source: { x: 198, y: 34 } },
  ] };
  const state = {
    mapFileName: "D2031",
    mapTransfers: [{ key: "crystal-move:d2031:198:34:117:184:267", mapFileName: "D2031", toMapFileName: "D2032", bounds: { minX: 198, maxX: 198, minY: 34, maxY: 34 } }],
  };
  const destination = resolveObjectiveMapFallbackDestination(
    [{ mapFileName: "D2032" }],
    { fallback, transfer: { key: "crystal-move:d2031:198:34:117:184:267", source: { x: 198, y: 34 } } },
    state,
    89,
  );
  assert.equal(destination.mapFileName, "D2032");
  state.mapTransfers = [];
  assert.throws(
    () => resolveObjectiveMapFallbackDestination(
      [{ mapFileName: "D2032" }],
      { fallback, transfer: { key: "crystal-move:d2031:198:34:117:184:267", source: { x: 198, y: 34 } } },
      state,
      89,
    ),
    /q89 live objective-map fallback D2032 transfer is stale/,
  );
});

test("retreat route bias preserves cave-exit progress without accepting an adjacent hostile", () => {
  const exposure = (adjacent, withinThree, weighted, nearest) => ({
    adjacent, withinThree, weighted, nearest,
  });
  const towardExit = {
    visited: false,
    steps: 2,
    biasProgress: 2,
    firstExposure: exposure(0, 1, 8, 2),
    targetExposure: exposure(0, 1, 8, 2),
  };
  const towardEntrance = {
    ...towardExit,
    biasProgress: -2,
    firstExposure: exposure(0, 1, 6, 2),
    targetExposure: exposure(0, 1, 6, 2),
  };
  assert.ok(compareRetreatCandidates(towardExit, towardEntrance) < 0);
  assert.ok(compareRetreatCandidates(
    { ...towardExit, visited: true },
    { ...towardEntrance, visited: false },
  ) < 0);

  const adjacentTowardExit = {
    ...towardExit,
    firstExposure: exposure(1, 1, 1, 1),
  };
  assert.ok(compareRetreatCandidates(adjacentTowardExit, towardEntrance) > 0);
});
import { TravelBlockedByMonster, TravelInterrupted } from "./protocol-travel.mjs";

class FakeClient {
  constructor(snapshot, respond = () => {}) {
    this.snapshot = snapshot;
    this.respond = respond;
    this.sequence = 0;
    this.events = [];
    this.sent = [];
  }
  send(command) {
    this.sent.push(command);
    this.respond(this, command);
  }
  receive(packet, update = () => {}, payload = {}) {
    update(this.snapshot);
    this.events.push({ sequence: ++this.sequence, direction: "received", packet, payload });
  }
  async wait(predicate, label) {
    const result = predicate();
    if (!result) throw new Error(`Fake timeout waiting for ${label}`);
    return result;
  }
}

const self = (extra = {}) => ({ objectId: 1, kind: "player", name: "Hero", x: 10, y: 10, hp: 40, maxHp: 40, direction: "Down", ...extra });
const monster = (objectId, name, x = 11, y = 10, extra = {}) => ({ objectId, kind: "monster", name, x, y, hp: 20, maxHp: 20, dead: false, ...extra });
const objective = (label, current, required) => ({ label, current, required, done: current >= required });
const snapshot = (quest, entities = []) => ({ playerObjectId: 1, playerHp: 40, mapFileName: "0", entities: [self(), ...entities], groundDrops: [], questLog: [quest] });
const settings = { sleep: async () => {}, attackCadenceMs: 0, harvestCadenceMs: 0, attackResponseTimeout: 1, questSettleTimeout: 1, spawnObservationTimeout: 1 };
const spawn = (monsterName, x = 20, y = 20) => ({ monsterName, mapFileName: "0", position: { x, y }, spread: 10, respawnIndex: x * 100 + y });
const navigateClientNear = client => async target => Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });

test("direct ranged hit evidence identifies a q89 aggressor up to eight tiles away", () => {
  const quest = { questId: 89, stage: "InProgress", objectives: [] };
  const nearRanged = monster(61, "CursedShaman", 16, 10, { disposition: "hostile" });
  const farRanged = monster(62, "CursedShaman", 19, 10, { disposition: "hostile" });
  const facingOnly = monster(63, "CursedShaman", 17, 10, { disposition: "hostile" });
  const client = new FakeClient(snapshot(quest, [nearRanged, farRanged, facingOnly]));
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 62 });
  client.receive("ObjectAttack", () => {}, {
    objectId: 63, location: { x: 17, y: 10 }, direction: "Left",
  });
  assert.deepEqual(provenAggressors(client, null, {
    aggressorEvidenceEvents: 128,
    aggressorEvidenceWindowMs: 5_000,
    directAggressorDistance: 8,
    now: () => Date.now(),
  }).map(entity => entity.objectId), [61]);
});

test('a q89 unavailable Shaman band defers and uses ordinary retreat recovery instead of ending the objective loop', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 0, 1)] };
  const priest = monster(89, 'CursedPriest', 15, 10, { disposition: 'hostile' });
  const client = new FakeClient(snapshot(quest, [priest]));
  let recoveryCalls = 0;
  let actionCalls = 0;
  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: 'CursedPriest', spawnCandidates: [spawn('CursedPriest')] }], item: [] },
  }, async target => {
    Object.assign(client.snapshot.entities[0], { x: target.x, y: target.y });
    return { reached: true, successfulSteps: 1 };
  }, {
    ...settings,
    maxEngagements: 2,
    approachRange: () => 9,
    action: async (_owner, target) => {
      actionCalls += 1;
      throw new RangedSafetyBandUnavailable(target.objectId, target, '0');
    },
    recoverAfterUnsafeRetreat: async owner => {
      recoveryCalls += 1;
      owner.snapshot.questLog[0].stage = 'ReadyToTurnIn';
    },
  });

  assert.equal(result.stage, 'ReadyToTurnIn');
  assert.equal(actionCalls, 1);
  assert.equal(recoveryCalls, 1);
});

test('q89 spawn-stall clearing reaches a certified Shaman band before its ordinary action', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 2, 3)] };
  const owner = self({ kind: 'selfPlayer', x: 278, y: 284, hp: 100, maxHp: 100 });
  const firstShaman = monster(341100, 'CursedShaman', 263, 273, { hp: 205, maxHp: 205, disposition: 'hostile' });
  const secondShaman = monster(341105, 'CursedShaman0', 275, 270, { hp: 205, maxHp: 205, disposition: 'hostile' });
  const shiZombie = monster(340903, 'ShiZombie', 268, 268, { hp: 205, maxHp: 205, disposition: 'hostile' });
  const directionDelta = {
    Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1],
    Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1],
  };
  const travelled = [];
  const client = new FakeClient({
    playerObjectId: 1, playerHp: 100, playerMaxHp: 100, playerMp: 145, playerMaxMp: 398,
    mapFileName: 'D2031', entities: [owner, firstShaman, secondShaman, shiZombie],
    groundDrops: [], questLog: [quest],
  }, (actor, command) => {
    if (command.type !== 'walk' && command.type !== 'run') return;
    const [dx, dy] = directionDelta[command.direction];
    const steps = command.type === 'run' ? 2 : 1;
    for (let index = 0; index < steps; index += 1) {
      owner.x += dx;
      owner.y += dy;
      travelled.push({ x: owner.x, y: owner.y });
    }
    actor.receive('UserLocation', () => {}, { objectId: owner.objectId, location: { x: owner.x, y: owner.y } });
  });
  client.wait = async predicate => {
    assert.equal(predicate(), true, 'the ordinary navigator must receive its movement receipt');
  };
  const approaches = [];
  const actionPositions = [];
  const rawNavigate = createNavigator(client, {
    loadCollisionMap: loadProtocolCollisionMap,
    delay: async () => {},
    now: () => 10_000,
  });
  const navigate = async (target, desiredDistance, stopWhen, options = {}) => {
    if (target.objectId == null) {
      return rawNavigate(target, desiredDistance, stopWhen, options);
    }
    approaches.push({ targetId: target.objectId, desiredDistance, options });
    return rawNavigate(target, desiredDistance, stopWhen, options);
  };
  const action = createWizardKitingAction(
    async (actor, target) => {
      actionPositions.push({ targetId: target.objectId, x: owner.x, y: owner.y });
      if (target.objectId === firstShaman.objectId || target.objectId === secondShaman.objectId) {
        Object.assign(target, { dead: true, hp: 0 });
        actor.snapshot.entities.push(monster(341200, 'CursedPriest', owner.x, owner.y - 5, { disposition: 'hostile' }));
        actor.receive('ObjectDied', () => {}, { objectId: target.objectId });
      } else {
        Object.assign(target, { dead: true, hp: 0 });
        actor.receive('ObjectDied', state => {
          state.questLog[0].objectives[0] = objective('Kill CursedPriest', 3, 3);
          state.questLog[0].stage = 'ReadyToTurnIn';
        }, { objectId: target.objectId });
      }
      return { kind: 'magic', targetId: target.objectId };
    },
    navigate,
    {
      approachRange: () => 9,
      fightWhenBlocked: true,
      rangedSafetyBand: {
        protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
        minimumTargetDistance: 7,
        maximumTargetDistance: 9,
        unsafeShamanDistance: 6,
        maxRetreatSteps: 6,
      },
    },
  );

  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: {
      kill: [{
        monsterName: 'CursedPriest',
        spawnCandidates: [{ monsterName: 'CursedPriest', mapFileName: 'D2031', position: { x: 250, y: 270 }, spread: 10, respawnIndex: 1 }],
      }],
      item: [],
    },
  }, navigate, {
    ...settings,
    approachRange: () => 9,
    action,
    spawnSearchHostileClearance: 4,
    spawnSearchHostileClearanceFallback: 1,
    spawnSearchProtectedHostileClearance: { CursedShaman: 6, CursedShaman0: 6 },
    spawnStallProtectedBlocker: {
      monsterNames: ['CursedShaman', 'CursedShaman0'],
      minimumApproachDistance: 7,
      maximumApproachDistance: 9,
      clearance: 6,
      maxBlockers: 2,
    },
    combatHostileClearance: 1,
    combatHostileClearanceFallback: 1,
    maxTargetAdjacent: 0,
    maxTargetNearby: 2,
  });

  assert.equal(result.stage, 'ReadyToTurnIn');
  assert.equal(approaches.length, 2);
  assert.ok([firstShaman.objectId, secondShaman.objectId].includes(approaches[0].targetId));
  assert.equal(approaches[0].desiredDistance, 9);
  assert.deepEqual(approaches[0].options.allowedHostileObjectIds, []);
  assert.deepEqual(approaches[0].options.hostileAvoidanceByName, { cursedshaman: 6, cursedshaman0: 6 });
  assert.ok(travelled.length > 0, 'the certified approach must dispatch ordinary movement');
  assert.ok(travelled.every(point =>
    Math.max(Math.abs(point.x - firstShaman.x), Math.abs(point.y - firstShaman.y)) > 6 &&
    Math.max(Math.abs(point.x - secondShaman.x), Math.abs(point.y - secondShaman.y)) > 6,
  ));
  assert.ok(!travelled.some(point => point.x === shiZombie.x && point.y === shiZombie.y));
  assert.equal(actionPositions.length, 2);
  const blockerAction = actionPositions[0];
  const selectedBlocker = blockerAction.targetId === firstShaman.objectId ? firstShaman : secondShaman;
  const otherShaman = selectedBlocker === firstShaman ? secondShaman : firstShaman;
  assert.ok(Math.max(Math.abs(blockerAction.x - selectedBlocker.x), Math.abs(blockerAction.y - selectedBlocker.y)) >= 7);
  assert.ok(Math.max(Math.abs(blockerAction.x - selectedBlocker.x), Math.abs(blockerAction.y - selectedBlocker.y)) <= 9);
  assert.ok(Math.max(Math.abs(blockerAction.x - otherShaman.x), Math.abs(blockerAction.y - otherShaman.y)) > 6);
  assert.equal(actionPositions[1].targetId, 341200);
});

test('q89 Wizard guards the real D2031-to-D2032 transit before a Run crosses the entrance Shaman halo', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 2, 3)] };
  const owner = self({ kind: 'selfPlayer', x: 278, y: 284, hp: 100, maxHp: 100 });
  const firstShaman = monster(341100, 'CursedShaman', 263, 273, { hp: 205, maxHp: 205, disposition: 'hostile' });
  const secondShaman = monster(341105, 'CursedShaman0', 275, 270, { hp: 205, maxHp: 205, disposition: 'hostile' });
  const shiZombie = monster(340903, 'ShiZombie', 268, 268, { hp: 230, maxHp: 230, disposition: 'hostile' });
  const transfer = {
    key: 'crystal-move:d2031:198:34:117:184:267', mapFileName: 'D2031', toMapFileName: 'D2032',
    bounds: { minX: 198, maxX: 198, minY: 34, maxY: 34 }, toPosition: { x: 184, y: 267 },
  };
  const deltas = {
    Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1],
    Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1],
  };
  const physicalCells = [];
  const actions = [];
  const diagnostics = [];
  const client = new FakeClient({
    playerObjectId: 1, playerHp: 100, playerMaxHp: 100, playerMp: 145, playerMaxMp: 398,
    mapFileName: 'D2031', mapTransfers: [transfer], entities: [owner, firstShaman, secondShaman, shiZombie],
    groundDrops: [], questLog: [quest],
  }, (actor, command) => {
    if (command.type !== 'walk' && command.type !== 'run') return;
    const [dx, dy] = deltas[command.direction];
    for (let step = 0; step < (command.type === 'run' ? 2 : 1); step += 1) {
      owner.x += dx;
      owner.y += dy;
      physicalCells.push({
        point: { x: owner.x, y: owner.y },
        liveShamans: actor.snapshot.entities
          .filter(entity => !entity.dead && ['CursedShaman', 'CursedShaman0'].includes(entity.name))
          .map(entity => ({ objectId: entity.objectId, x: entity.x, y: entity.y })),
      });
      if (owner.x === transfer.bounds.minX && owner.y === transfer.bounds.minY) {
        Object.assign(owner, transfer.toPosition);
        const priest = monster(341200, 'CursedPriest', 184, 262, { disposition: 'hostile' });
        Object.assign(actor.snapshot, { mapFileName: 'D2032', mapTransfers: [], entities: [owner, priest] });
        actor.receive('MapChanged', () => {}, { fileName: 'D2032' });
        break;
      }
    }
    actor.receive('UserLocation', () => {}, { objectId: 1, location: { x: owner.x, y: owner.y } });
  });
  client.record = (_direction, payload) => diagnostics.push(payload);
  client.wait = async predicate => assert.equal(predicate(), true, 'navigator must observe each movement receipt');
  const rawNavigate = createNavigator(client, {
    loadCollisionMap: loadProtocolCollisionMap,
    delay: async () => {},
    now: () => 10_000,
  });
  const action = createWizardKitingAction(async (actor, target) => {
    actions.push({
      targetId: target.objectId,
      position: { x: owner.x, y: owner.y },
      liveShamans: actor.snapshot.entities.filter(entity => !entity.dead &&
        ['CursedShaman', 'CursedShaman0'].includes(entity.name)),
    });
    Object.assign(target, { dead: true, hp: 0 });
    actor.receive('ObjectDied', state => {
      if (target.name === 'CursedPriest') {
        state.questLog[0].objectives[0] = objective('Kill CursedPriest', 3, 3);
        state.questLog[0].stage = 'ReadyToTurnIn';
      }
    }, { objectId: target.objectId });
    return { kind: 'magic', targetId: target.objectId };
  }, rawNavigate, {
    approachRange: () => 9,
    fightWhenBlocked: true,
    rangedSafetyBand: {
      protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
      minimumTargetDistance: 7,
      maximumTargetDistance: 9,
      unsafeShamanDistance: 6,
      maxRetreatSteps: 6,
    },
  });
  const travel = async (mapFileName, options = {}) => {
    assert.equal(mapFileName, 'D2032');
    await rawNavigate({ x: 198, y: 34 }, 0, () => false, {
      liveTransferKey: transfer.key,
      ...(options.navigationOptions ?? {}),
    });
  };
  travel.routeLength = async () => 1;

  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: {
      kill: [{ monsterName: 'CursedPriest', spawnCandidates: [
        { monsterName: 'CursedPriest', mapFileName: 'D2032', position: { x: 184, y: 267 }, spread: 8, respawnIndex: 1 },
      ] }],
      item: [],
    },
  }, rawNavigate, {
    ...settings,
    travel,
    preferObjectiveMapOverCurrent: true,
    preferredObjectiveMaps: ['D2032'],
    approachRange: () => 9,
    action,
    maxTargetAdjacent: 0,
    maxTargetNearby: 2,
    spawnStallProtectedBlocker: {
      monsterNames: ['CursedShaman', 'CursedShaman0'],
      minimumApproachDistance: 7, maximumApproachDistance: 9, clearance: 6, maxBlockers: 2,
    },
    transitProtectedBlocker: {
      monsterNames: ['CursedShaman', 'CursedShaman0'],
      minimumApproachDistance: 7, maximumApproachDistance: 9, clearance: 6, maxBlockers: 2,
    },
  });

  assert.equal(result.stage, 'ReadyToTurnIn');
  assert.ok(diagnostics.some(entry => entry.type === 'protectedTransitBlockerIntercepted' && entry.blockerObjectId === 341105));
  assert.ok(physicalCells.length > 0);
  assert.ok(physicalCells.every(entry => entry.liveShamans.every(shaman =>
    Math.max(Math.abs(entry.point.x - shaman.x), Math.abs(entry.point.y - shaman.y)) > 6,
  )), 'no physical transit cell may enter a live protected Shaman halo');
  assert.ok(!physicalCells.some(entry => entry.point.x === shiZombie.x && entry.point.y === shiZombie.y));
  for (const actionAt of actions.filter(entry => entry.liveShamans.some(shaman => shaman.objectId === entry.targetId))) {
    const target = actionAt.liveShamans.find(shaman => shaman.objectId === actionAt.targetId);
    assert.ok(Math.max(Math.abs(actionAt.position.x - target.x), Math.abs(actionAt.position.y - target.y)) >= 7);
    assert.ok(Math.max(Math.abs(actionAt.position.x - target.x), Math.abs(actionAt.position.y - target.y)) <= 9);
    assert.ok(actionAt.liveShamans.filter(shaman => shaman.objectId !== target.objectId).every(shaman =>
      Math.max(Math.abs(actionAt.position.x - shaman.x), Math.abs(actionAt.position.y - shaman.y)) > 6,
    ));
  }
});

test('q89 transit clearance aborts when a Shaman moves into its fresh cadence cells without recursive clearing', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 2, 3)] };
  const owner = self({ kind: 'selfPlayer', x: 10, y: 10, hp: 100, maxHp: 100 });
  const blocker = monster(71, 'CursedShaman', 20, 10, { hp: 205, maxHp: 205, disposition: 'hostile' });
  const movingShaman = monster(72, 'CursedShaman0', 35, 10, { hp: 205, maxHp: 205, disposition: 'hostile' });
  let movedDuringCadence = false;
  let baseActions = 0;
  let recoveries = 0;
  const client = new FakeClient({
    playerObjectId: 1, playerHp: 100, playerMaxHp: 100, mapFileName: 'D2031',
    entities: [owner, blocker, movingShaman], groundDrops: [], questLog: [quest],
  }, (actor, command) => {
    if (command.type !== 'walk' && command.type !== 'run') return;
    assert.fail(`fresh clearance guard must block before ${command.type}`);
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  const rawNavigate = createNavigator(client, {
    loadCollisionMap: async () => ({ mapFileName: 'D2031', sourcePath: 'test', width: 40, height: 20, blocked: new Uint8Array(800) }),
    delay: async () => {
      if (!movedDuringCadence) {
        movedDuringCadence = true;
        Object.assign(movingShaman, { x: 17, y: 10 });
      }
    },
    now: () => 10_000,
  });
  const action = createWizardKitingAction(async () => {
    baseActions += 1;
    return { kind: 'magic', targetId: blocker.objectId };
  }, rawNavigate, {
    approachRange: () => 9,
    fightWhenBlocked: true,
    rangedSafetyBand: {
      protectedMonsterNames: ['CursedShaman', 'CursedShaman0'],
      minimumTargetDistance: 7, maximumTargetDistance: 9, unsafeShamanDistance: 6, maxRetreatSteps: 6,
    },
  });
  const travel = async () => {
    throw new NavigationStepGuarded({
      type: 'protectedTransitShamanHalo', blockerObjectId: blocker.objectId, blockerName: blocker.name,
      mapId: 'D2031', from: { x: 10, y: 10 }, physicalCells: [{ x: 11, y: 10 }],
    });
  };
  travel.routeLength = async () => 1;

  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: 'CursedPriest', spawnCandidates: [
      { monsterName: 'CursedPriest', mapFileName: 'D2032', position: { x: 1, y: 1 }, spread: 1, respawnIndex: 1 },
    ] }], item: [] },
  }, rawNavigate, {
    ...settings,
    travel,
    preferObjectiveMapOverCurrent: true,
    preferredObjectiveMaps: ['D2032'],
    approachRange: () => 9,
    action,
    spawnStallProtectedBlocker: {
      monsterNames: ['CursedShaman', 'CursedShaman0'],
      minimumApproachDistance: 7, maximumApproachDistance: 9, clearance: 6, maxBlockers: 2,
    },
    transitProtectedBlocker: {
      monsterNames: ['CursedShaman', 'CursedShaman0'],
      minimumApproachDistance: 7, maximumApproachDistance: 9, clearance: 6, maxBlockers: 2,
    },
    recoverAfterUnsafeRetreat: async current => {
      recoveries += 1;
      current.snapshot.questLog[0].stage = 'ReadyToTurnIn';
    },
  });

  assert.equal(result.stage, 'ReadyToTurnIn');
  assert.equal(movedDuringCadence, true);
  assert.equal(baseActions, 0, 'no cast is allowed after the fresh cell becomes unsafe');
  assert.equal(recoveries, 1);
  assert.equal(client.sent.filter(command => command.type === 'walk' || command.type === 'run').length, 0);
  assert.equal(diagnostics.filter(entry => entry.type === 'protectedTransitBlockerIntercepted').length, 1);
  assert.equal(diagnostics.filter(entry => entry.type === 'protectedTransitClearanceMovementBlocked').length, 1);
});

test('q89 transit blocker cap performs one recovery then terminates a still-live next halo', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 2, 3)] };
  const first = monster(71, 'CursedShaman', 19, 10, { disposition: 'hostile' });
  const second = monster(72, 'CursedShaman0', 20, 10, { disposition: 'hostile' });
  const client = new FakeClient(snapshot(quest, [first, second]));
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let travelCalls = 0;
  const travel = async () => {
    travelCalls += 1;
    const blocker = travelCalls === 1 ? first : second;
    throw new NavigationStepGuarded({
      type: 'protectedTransitShamanHalo', blockerObjectId: blocker.objectId, blockerName: blocker.name,
      mapId: '0', from: { x: 10, y: 10 }, physicalCells: [{ x: 11, y: 10 }],
    });
  };
  travel.routeLength = async () => 1;
  let actions = 0;
  let recoveries = 0;
  await assert.rejects(completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: 'CursedPriest', spawnCandidates: [
      { monsterName: 'CursedPriest', mapFileName: 'D2032', position: { x: 1, y: 1 }, spread: 1, respawnIndex: 1 },
    ] }], item: [] },
  }, navigateClientNear(client), {
    ...settings,
    travel,
    preferObjectiveMapOverCurrent: true,
    preferredObjectiveMaps: ['D2032'],
    maxEngagements: 4,
    approachRange: () => 9,
    action: async (owner, target) => {
      actions += 1;
      Object.assign(owner.snapshot.entities.find(entity => entity.objectId === target.objectId), { dead: true, hp: 0 });
      owner.receive('ObjectDied', () => {}, { objectId: target.objectId });
      return { kind: 'magic', targetId: target.objectId };
    },
    transitProtectedBlocker: {
      monsterNames: ['CursedShaman', 'CursedShaman0'],
      minimumApproachDistance: 7, maximumApproachDistance: 9, clearance: 6, maxBlockers: 1,
    },
    recoverAfterUnsafeRetreat: async () => { recoveries += 1; },
  }), /protected transit blocker 72 remains live without physical recovery.*blockerBudgetExhausted/i);

  // The first recovery made a real bounded retreat, so the per-stall guard
  // permits one fresh route attempt. The next unchanged live halo terminates.
  assert.equal(travelCalls, 3, 'the cap must not keep replaying the same guarded step');
  assert.equal(actions, 1);
  assert.equal(recoveries, 2);
  assert.equal(diagnostics.filter(entry => entry.type === 'protectedTransitBlockerRecoveredProgress').length, 1);
  assert.equal(diagnostics.filter(entry => entry.type === 'protectedTransitBlockerTerminated').length, 1);
});

test('q89 deferred live transit blocker does not replan the same guarded step forever', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 2, 3)] };
  const blocker = monster(71, 'CursedShaman', 19, 10, { disposition: 'hostile' });
  const client = new FakeClient(snapshot(quest, [blocker]));
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let travelCalls = 0;
  const travel = async () => {
    travelCalls += 1;
    throw new NavigationStepGuarded({
      type: 'protectedTransitShamanHalo', blockerObjectId: blocker.objectId, blockerName: blocker.name,
      mapId: '0', from: { x: 10, y: 10 }, physicalCells: [{ x: 11, y: 10 }],
    });
  };
  travel.routeLength = async () => 1;
  let actions = 0;
  let recoveries = 0;
  await assert.rejects(completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: 'CursedPriest', spawnCandidates: [
      { monsterName: 'CursedPriest', mapFileName: 'D2032', position: { x: 1, y: 1 }, spread: 1, respawnIndex: 1 },
    ] }], item: [] },
  }, navigateClientNear(client), {
    ...settings,
    travel,
    preferObjectiveMapOverCurrent: true,
    preferredObjectiveMaps: ['D2032'],
    maxEngagements: 4,
    approachRange: () => 9,
    action: async (_owner, target) => {
      actions += 1;
      throw new RangedSafetyBandUnavailable(target.objectId, target, '0');
    },
    transitProtectedBlocker: {
      monsterNames: ['CursedShaman', 'CursedShaman0'],
      minimumApproachDistance: 7, maximumApproachDistance: 9, clearance: 6, maxBlockers: 2,
    },
    recoverAfterUnsafeRetreat: async () => { recoveries += 1; },
  }), /protected transit blocker 71 remains live without physical recovery.*liveDeferredWithoutPhysicalProgress/i);

  // The resolver's own ordinary retreat changed the physical state once;
  // only the following same-live-halo intercept is terminated.
  assert.equal(travelCalls, 2);
  assert.equal(actions, 1);
  assert.equal(recoveries, 2);
  assert.equal(diagnostics.filter(entry => entry.type === 'protectedTransitBlockerRecoveredProgress').length, 1);
  assert.equal(diagnostics.filter(entry => entry.type === 'protectedTransitBlockerTerminated').length, 1);
});

test('q89 spawn-stall blocker defers when independent quest progress removes it without a death receipt', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 2, 3)] };
  const blocker = monster(341105, 'CursedShaman0', 19, 10, { disposition: 'hostile' });
  const client = new FakeClient(snapshot(quest, [blocker]));
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let recoveries = 0;
  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: {
      kill: [{ monsterName: 'CursedPriest', spawnCandidates: [spawn('CursedPriest', 20, 20)] }],
      item: [],
    },
  }, async target => {
    if (target.objectId == null) throw new Error('No walk path on D2031 from 10,10 to 20,20');
    return { reached: true, successfulSteps: 0 };
  }, {
    ...settings,
    approachRange: () => 9,
    spawnSearchHostileClearance: 4,
    spawnSearchHostileClearanceFallback: 1,
    spawnStallProtectedBlocker: {
      monsterNames: ['CursedShaman', 'CursedShaman0'],
      minimumApproachDistance: 7,
      maximumApproachDistance: 9,
      clearance: 6,
      maxBlockers: 2,
    },
    action: async (owner, target) => {
      Object.assign(owner.snapshot.entities.find(entity => entity.objectId === target.objectId), {
        // A missing exact HP value is not a corpse. This used to be coerced
        // through Number(null) and could falsely clear the corridor blocker.
        hp: null,
        dead: false,
      });
      // This independent authoritative objective update is deliberately not a
      // death packet for the still-live Shaman.
      owner.receive('ObjectHealth', state => {
        state.questLog[0].objectives[0] = objective('Kill CursedPriest', 3, 3);
        state.questLog[0].stage = 'ReadyToTurnIn';
      }, { objectId: 341200, percent: 0 });
      return { kind: 'magic', targetId: target.objectId };
    },
    recoverAfterUnsafeRetreat: async () => { recoveries += 1; },
  });

  assert.equal(result.stage, 'ReadyToTurnIn');
  assert.equal(recoveries, 1);
  assert.equal(diagnostics.filter(entry => entry.type === 'spawnStallProtectedBlockerCleared').length, 0);
  assert.equal(diagnostics.filter(entry => entry.type === 'spawnStallProtectedBlockerDeferred').length, 1);
});

test("q2 kills a live Scarecrow and trusts authoritative GingerTea progress", async () => {
  const quest = { questId: 2, stage: "InProgress", objectives: [objective("Collect GingerTea", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(20, "Scarecrow")]), (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 20), { hp: 0, dead: true });
      Object.assign(state.questLog[0], { stage: "ReadyToTurnIn", objectives: [objective("Collect GingerTea", 1, 1)] });
    });
  });
  const navigated = [];
  const result = await completeQuestObjectives(client, {
    questId: 2,
    objectives: { kill: [], item: [{ itemName: "GingerTea", sources: [{ monsterName: "Scarecrow", requiresHarvest: false, spawnCandidates: [spawn("Scarecrow")] }] }] },
  }, async (...args) => navigated.push(args), settings);
  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent, [{ type: "attack", objectId: 20 }]);
  assert.equal(navigated[0][1], 1);
});

test("q5 completes Deer and Scarecrow kill objectives without treating monster level as a gate", async () => {
  const quest = { questId: 5, stage: "InProgress", objectives: [objective("Kill Deer", 0, 1), objective("Kill Scarecrow", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(30, "Deer"), monster(39, "Scarecrow", 12, 10)]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      const target = state.entities.find(entry => entry.objectId === command.objectId);
      Object.assign(target, { hp: 0, dead: true });
      const index = target.name === "Deer" ? 0 : 1;
      state.questLog[0].objectives[index] = objective(`Kill ${target.name}`, 1, 1);
      if (state.questLog[0].objectives.every(entry => entry.done)) state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  const result = await completeQuestObjectives(client, {
    questId: 5,
    objectives: { kill: [
      { monsterName: "Deer", monsterLevel: 99, spawnCandidates: [spawn("Deer")] },
      { monsterName: "Scarecrow", monsterLevel: 99, spawnCandidates: [spawn("Scarecrow")] },
    ], item: [] },
  }, navigateClientNear(client), settings);
  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [30, 39]);
});

test("a multi-objective hunt clears a visible later target while the first target is respawning", async () => {
  const quest = {
    questId: 98,
    stage: "InProgress",
    objectives: [objective("Kill Dung", 0, 1), objective("Kill WoomaSoldier", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [monster(98, "WoomaSoldier")]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      const target = state.entities.find(entry => entry.objectId === command.objectId);
      Object.assign(target, { hp: 0, dead: true });
      if (target.name === "WoomaSoldier") {
        state.questLog[0].objectives[1] = objective("Kill WoomaSoldier", 1, 1);
        state.entities.push(monster(99, "Dung"));
      } else {
        state.questLog[0].objectives[0] = objective("Kill Dung", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    });
  });

  const result = await completeQuestObjectives(client, {
    questId: 98,
    objectives: { kill: [
      { monsterName: "Dung", spawnCandidates: [spawn("Dung")] },
      { monsterName: "WoomaSoldier", spawnCandidates: [spawn("WoomaSoldier")] },
    ], item: [] },
  }, navigateClientNear(client), settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [98, 99]);
});

test("a multi-objective spawn search stops for another safe required monster entering AOI", async () => {
  const quest = {
    questId: 89,
    stage: "InProgress",
    objectives: [objective("Kill CursedPriest", 0, 1), objective("Kill CursedZombie", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      const target = state.entities.find(entry => entry.objectId === command.objectId);
      Object.assign(target, { dead: true, hp: 0 });
      const index = target.name === "CursedPriest" ? 0 : 1;
      state.questLog[0].objectives[index] = objective(`Kill ${target.name}`, 1, 1);
      if (state.questLog[0].objectives.every(entry => entry.done)) state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searches = 0;
  const navigate = async (target, _range, stopWhen) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    searches += 1;
    client.snapshot.entities.push(searches === 1
      ? monster(90, "CursedZombie", actor.x + 1, actor.y, { disposition: "hostile" })
      : monster(91, "CursedPriest", actor.x + 1, actor.y, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };
  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [
      { monsterName: "CursedPriest", spawnCandidates: [spawn("CursedPriest")] },
      { monsterName: "CursedZombie", spawnCandidates: [spawn("CursedZombie")] },
    ], item: [] },
  }, navigate, { ...settings, maxTargetAdjacent: 0, maxTargetNearby: 0 });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [90, 91]);
  assert.equal(searches, 2);
  assert.ok(diagnostics.some(entry => entry.type === "spawnSearchObjectiveSwitched" &&
    entry.fromTarget === "CursedPriest" && entry.toTarget === "CursedZombie"));
});

test("a multi-map quest clears an incomplete same-map objective before crossing to the first listed source", async () => {
  const quest = {
    questId: 54,
    stage: "InProgress",
    objectives: [objective("Kill Zombie5", 0, 1), objective("Kill Zombie2", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [monster(54, "Zombie2")]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      state.questLog[0].objectives = [
        objective("Kill Zombie5", 1, 1),
        objective("Kill Zombie2", 1, 1),
      ];
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  client.snapshot.mapFileName = "D421";
  const travelled = [];

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: {
      kill: [
        { monsterName: "Zombie5", spawnCandidates: [{ ...spawn("Zombie5"), mapFileName: "D422" }] },
        { monsterName: "Zombie2", spawnCandidates: [{ ...spawn("Zombie2"), mapFileName: "D421" }] },
      ],
      item: [],
    },
  }, navigateClientNear(client), {
    ...settings,
    travel: async mapFileName => travelled.push(mapFileName),
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent, [{ type: "attack", objectId: 54 }]);
  assert.deepEqual(travelled, []);
});

test("a live objective omitted from the spawn manifest is fought on the current map", async () => {
  const quest = {
    questId: 122,
    stage: "InProgress",
    objectives: [objective("Kill BoneArcher", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [monster(122, "BoneArcher")]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      state.questLog[0].objectives = [objective("Kill BoneArcher", 1, 1)];
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  client.snapshot.mapFileName = "D2061";
  const travelled = [];
  const travel = async mapFileName => travelled.push(mapFileName);
  travel.canReach = async () => true;

  const result = await completeQuestObjectives(client, {
    questId: 122,
    objectives: { kill: [{
      monsterName: "BoneArcher",
      spawnCandidates: [{ ...spawn("BoneArcher"), mapFileName: "D2062" }],
    }], item: [] },
  }, navigateClientNear(client), { ...settings, travel });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(travelled, []);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [122]);
});

test("an allowed wounded objective is resumed before a closer full-health peer", async () => {
  const quest = {
    questId: 98,
    stage: "InProgress",
    objectives: [objective("Kill WoomaSoldier", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "WoomaSoldier", 11, 10),
    monster(61, "WoomaSoldier", 15, 10, { hp: 5, maxHp: 20 }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      state.questLog[0].objectives = [objective("Kill WoomaSoldier", 1, 1)];
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });

  const result = await completeQuestObjectives(client, {
    questId: 98,
    objectives: { kill: [{ monsterName: "WoomaSoldier", spawnCandidates: [spawn("WoomaSoldier")] }], item: [] },
  }, navigateClientNear(client), { ...settings, maxTargetNearby: 5 });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61]);
});

test("a healthy player keeps approaching a 25-percent objective when a pack appears", async () => {
  const quest = {
    questId: 99,
    stage: "InProgress",
    objectives: [objective("Kill WoomaFighter", 0, 1)],
  };
  const target = monster(70, "WoomaFighter", 20, 10, { hp: 5, maxHp: 20 });
  const client = new FakeClient(snapshot(quest, [target]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(target, { hp: 0, dead: true });
      state.questLog[0].objectives = [objective("Kill WoomaFighter", 1, 1)];
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  const navigate = async (destination, desiredDistance, stopWhen) => {
    client.snapshot.entities.push(
      monster(71, "WoomaWarrior", 20, 9),
      monster(72, "WoomaWarrior", 20, 11),
    );
    assert.equal(stopWhen(), false, "finishable target must survive the approach pack check");
    Object.assign(client.snapshot.entities[0], { x: destination.x - desiredDistance, y: destination.y });
    return { reached: true, successfulSteps: 1 };
  };

  const result = await completeQuestObjectives(client, {
    questId: 99,
    objectives: { kill: [{ monsterName: "WoomaFighter", spawnCandidates: [spawn("WoomaFighter")] }], item: [] },
  }, navigate, {
    ...settings,
    focusTargetThroughAggressors: true,
    finishableTargetHealthRatio: 0.25,
    finishableTargetMinimumPlayerHpRatio: 0.7,
    maxTargetAdjacent: 0,
    maxTargetNearby: 1,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [70]);
});

test("an expedition can prefer its configured objective map over a denser same-map spawn", async () => {
  const quest = {
    questId: 54,
    stage: "InProgress",
    objectives: [objective("Kill Zombie1", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [monster(54, "Zombie1")]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      state.questLog[0].objectives = [objective("Kill Zombie1", 1, 1)];
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  client.snapshot.mapFileName = "D401";
  const travelled = [];
  const travel = async mapFileName => {
    travelled.push(mapFileName);
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities = [self(), monster(64, "Zombie1")];
  };
  travel.canReach = async () => true;

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: {
      kill: [{
        monsterName: "Zombie1",
        spawnCandidates: [
          { ...spawn("Zombie1"), mapFileName: "D401" },
          { ...spawn("Zombie1"), mapFileName: "D406" },
        ],
      }],
      item: [],
    },
  }, navigateClientNear(client), {
    ...settings,
    travel,
    preferredObjectiveMaps: ["D406"],
    preferObjectiveMapOverCurrent: true,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(travelled, ["D406"]);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [64]);
});

test("a post-travel supply checkpoint can leave and replan before objective combat", async () => {
  const quest = {
    questId: 113,
    stage: "InProgress",
    objectives: [objective("Kill WedgeMoth", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      state.questLog[0].objectives = [objective("Kill WedgeMoth", 1, 1)];
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  const travelled = [];
  const travel = async mapFileName => {
    travelled.push(mapFileName);
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities = [self(), monster(113, "WedgeMoth")];
  };
  travel.canReach = async () => true;
  let checkpoints = 0;

  const result = await completeQuestObjectives(client, {
    questId: 113,
    objectives: {
      kill: [{
        monsterName: "WedgeMoth",
        spawnCandidates: [{ ...spawn("WedgeMoth"), mapFileName: "D607" }],
      }],
      item: [],
    },
  }, navigateClientNear(client), {
    ...settings,
    travel,
    afterTravel: async owner => {
      checkpoints += 1;
      if (checkpoints !== 1) return false;
      owner.snapshot.mapFileName = "0";
      owner.snapshot.entities = [self()];
      return true;
    },
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(travelled, ["D607", "D607"]);
  assert.equal(checkpoints, 2);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [113]);
});

test("multi-map travel evades a proven adjacent aggressor before retrying the ordinary route", async () => {
  const quest = {
    questId: 54,
    stage: "InProgress",
    objectives: [objective("Kill Zombie5", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [
    monster(55, "CannibalPlant", 11, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      if (command.objectId === 54) {
        state.questLog[0].objectives = [objective("Kill Zombie5", 1, 1)];
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  client.snapshot.mapFileName = "D421";
  client.events.push({
    sequence: ++client.sequence,
    direction: "received",
    packet: "ObjectStruck",
    payload: { objectId: 1, attackerId: 55 },
    at: Date.now(),
  });
  let travelAttempts = 0;
  const travel = async (mapFileName, options = {}) => {
    travelAttempts += 1;
    if (travelAttempts === 1) {
      assert.equal(options.interruptWhen(), true);
      throw new TravelInterrupted("D421", mapFileName);
    }
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities = [self(), monster(54, "Zombie5")];
  };
  travel.canReach = async () => true;

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: {
      kill: [{
        monsterName: "Zombie5",
        spawnCandidates: [{ ...spawn("Zombie5"), mapFileName: "D422" }],
      }],
      item: [],
    },
  }, navigateClientNear(client), { ...settings, travel });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [54]);
  assert.equal(travelAttempts, 2);
  assert.ok(diagnostics.some(entry => entry.type === "travelThreatEvaded"));
});

test("a repeated interruption on the same travel edge clears the proven aggressor", async () => {
  const quest = {
    questId: 54,
    stage: "InProgress",
    objectives: [objective("Kill Zombie5", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [
    monster(55, "Scarecrow", 11, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      if (command.objectId === 54) {
        state.questLog[0].objectives = [objective("Kill Zombie5", 1, 1)];
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  client.snapshot.mapFileName = "0";
  client.events.push({
    sequence: ++client.sequence,
    direction: "received",
    packet: "ObjectStruck",
    payload: { objectId: 1, attackerId: 55 },
    at: Date.now(),
  });
  let travelAttempts = 0;
  const travel = async (mapFileName, options = {}) => {
    travelAttempts += 1;
    if (travelAttempts <= 2) {
      if (travelAttempts === 2) {
        const actor = client.snapshot.entities.find(entry => entry.objectId === client.snapshot.playerObjectId);
        const aggressor = client.snapshot.entities.find(entry => entry.objectId === 55);
        Object.assign(aggressor, { x: actor.x + 1, y: actor.y });
        client.events.push({
          sequence: ++client.sequence,
          direction: "received",
          packet: "ObjectStruck",
          payload: { objectId: 1, attackerId: 55 },
          at: Date.now(),
        });
      }
      assert.equal(options.interruptWhen(), true);
      throw new TravelInterrupted("0", mapFileName);
    }
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities = [self(), monster(54, "Zombie5")];
  };
  travel.canReach = async () => true;

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: {
      kill: [{
        monsterName: "Zombie5",
        spawnCandidates: [{ ...spawn("Zombie5"), mapFileName: "D422" }],
      }],
      item: [],
    },
  }, navigateClientNear(client), { ...settings, travel });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [55, 54]);
  assert.equal(travelAttempts, 3);
  assert.ok(diagnostics.some(entry => entry.type === "travelThreatEvaded"));
  assert.ok(diagnostics.some(entry => entry.type === "travelThreatCleared"));
});

test("multi-map travel clears a hostile doorway blocker and retries the same route", async () => {
  const quest = {
    questId: 54,
    stage: "InProgress",
    objectives: [objective("Kill Zombie5", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [
    monster(55, "Zombie3", 11, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      const killed = state.entities.find(entry => entry.objectId === command.objectId);
      Object.assign(killed, { hp: 0, dead: true });
      if (command.objectId === 54) {
        state.questLog[0].objectives = [objective("Kill Zombie5", 1, 1)];
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  client.snapshot.mapFileName = "D421";
  let travelAttempts = 0;
  const travel = async (mapFileName, options = {}) => {
    travelAttempts += 1;
    if (travelAttempts === 1) {
      assert.equal(options.resolveBlockingMonster, false);
      throw new TravelBlockedByMonster("D421", mapFileName,
        client.snapshot.entities.find(entry => entry.objectId === 55));
    }
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities = [self(), monster(54, "Zombie5")];
  };
  travel.canReach = async () => true;

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: {
      kill: [{
        monsterName: "Zombie5",
        spawnCandidates: [{ ...spawn("Zombie5"), mapFileName: "D422" }],
      }],
      item: [],
    },
  }, navigateClientNear(client), { ...settings, travel });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [55, 54]);
  assert.equal(travelAttempts, 2);
  assert.ok(diagnostics.some(entry =>
    entry.type === "travelBlockerCombat" && entry.blockerObjectId === 55));
});

test("doorway blocker retreats when aggressor clearing discovers a dense pack", async () => {
  const quest = {
    questId: 54,
    stage: "InProgress",
    objectives: [objective("Kill Zombie5", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [
    monster(55, "Zombie3", 11, 10, { disposition: "hostile" }),
    monster(56, "Zombie2", 10, 11, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack" || command.objectId !== 54) return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 54), { hp: 0, dead: true });
      state.questLog[0].objectives = [objective("Kill Zombie5", 1, 1)];
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  client.events.push({
    sequence: ++client.sequence,
    direction: "received",
    packet: "ObjectStruck",
    payload: { objectId: 1, attackerId: 56 },
    at: Date.now(),
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  client.snapshot.mapFileName = "D401";
  let travelAttempts = 0;
  const travel = async mapFileName => {
    travelAttempts += 1;
    if (travelAttempts === 1) {
      throw new TravelBlockedByMonster("D401", mapFileName,
        client.snapshot.entities.find(entry => entry.objectId === 55));
    }
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities = [self(), monster(54, "Zombie5")];
  };
  travel.canReach = async () => true;
  const navigateNear = async (target, desiredDistance = 1) => {
    const actor = client.snapshot.entities.find(entry => entry.objectId === 1);
    Object.assign(actor, desiredDistance === 0
      ? { x: target.x, y: target.y }
      : { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: {
      kill: [{
        monsterName: "Zombie5",
        spawnCandidates: [{ ...spawn("Zombie5"), mapFileName: "D406" }],
      }],
      item: [],
    },
  }, navigateNear, {
    ...settings,
    travel,
    retreatAtActiveAggressorCount: 1,
    unsafeRetreatSteps: 4,
    unsafeRetreatMaxSteps: 12,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [54]);
  assert.equal(travelAttempts, 2);
  assert.ok(diagnostics.some(entry => entry.type === "travelBlockerCombat"));
  assert.ok(diagnostics.some(entry => entry.type === "unsafeTargetCluster" && entry.objectId === 56));
  assert.ok(diagnostics.some(entry => entry.type === "unsafePackRetreat"));
});

test("stocked expedition travel clears a proven aggressor before retrying the route", async () => {
  const quest = {
    questId: 54,
    stage: "InProgress",
    objectives: [objective("Kill Zombie5", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [
    monster(55, "Zombie3", 11, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      const killed = state.entities.find(entry => entry.objectId === command.objectId);
      Object.assign(killed, { hp: 0, dead: true });
      if (command.objectId === 54) {
        state.questLog[0].objectives = [objective("Kill Zombie5", 1, 1)];
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  client.snapshot.mapFileName = "D421";
  client.events.push({
    sequence: ++client.sequence,
    direction: "received",
    packet: "ObjectStruck",
    payload: { objectId: 1, attackerId: 55 },
    at: Date.now(),
  });
  let travelAttempts = 0;
  const travel = async (mapFileName, options = {}) => {
    travelAttempts += 1;
    if (travelAttempts === 1) {
      assert.equal(options.interruptWhen(), true);
      throw new TravelInterrupted("D421", mapFileName);
    }
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities = [self(), monster(54, "Zombie5")];
  };
  travel.canReach = async () => true;

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: {
      kill: [{
        monsterName: "Zombie5",
        spawnCandidates: [{ ...spawn("Zombie5"), mapFileName: "D422" }],
      }],
      item: [],
    },
  }, navigateClientNear(client), {
    ...settings,
    travel,
    preferTravelAggressorCombat: true,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [55, 54]);
  assert.equal(travelAttempts, 2);
  assert.ok(diagnostics.some(entry => entry.type === "travelThreatCleared" && entry.count === 1));
  assert.equal(diagnostics.some(entry => entry.type === "travelThreatEvaded"), false);
});

test("travel retries when the proven threat leaves before fallback combat", async () => {
  const quest = {
    questId: 54,
    stage: "InProgress",
    objectives: [objective("Kill Zombie5", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [
    monster(55, "CaveMaggot", 11, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 54), { hp: 0, dead: true });
      state.questLog[0].objectives = [objective("Kill Zombie5", 1, 1)];
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  client.events.push({
    sequence: ++client.sequence,
    direction: "received",
    packet: "ObjectStruck",
    payload: { objectId: 1, attackerId: 55 },
    at: Date.now(),
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  client.snapshot.mapFileName = "D401";
  let travelAttempts = 0;
  const travel = async (mapFileName, options = {}) => {
    travelAttempts += 1;
    if (travelAttempts === 1) {
      assert.equal(options.interruptWhen(), true);
      client.snapshot.entities = [self()];
      throw new TravelInterrupted("D401", mapFileName);
    }
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities = [self(), monster(54, "Zombie5")];
  };
  travel.canReach = async () => true;

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: {
      kill: [{
        monsterName: "Zombie5",
        spawnCandidates: [{ ...spawn("Zombie5"), mapFileName: "D411" }],
      }],
      item: [],
    },
  }, navigateClientNear(client), {
    ...settings,
    travel,
    preferTravelAggressorCombat: true,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [54]);
  assert.equal(travelAttempts, 2);
  assert.ok(diagnostics.some(entry =>
    entry.type === "travelThreatEvaded" && entry.reason === "noProvenAggressorAfterInterruption"));
});

test("healthy expedition keeps crossing after a glancing hit and interrupts once health is low", async () => {
  const quest = {
    questId: 54,
    stage: "InProgress",
    objectives: [objective("Kill Zombie5", 0, 1)],
  };
  const client = new FakeClient(snapshot(quest, [
    monster(55, "Zombie3", 11, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), {
        hp: 0,
        dead: true,
      });
      if (command.objectId === 54) {
        state.questLog[0].objectives = [objective("Kill Zombie5", 1, 1)];
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  client.snapshot.mapFileName = "D421";
  client.snapshot.playerHp = 40;
  client.snapshot.playerMaxHp = 40;
  client.events.push({
    sequence: ++client.sequence,
    direction: "received",
    packet: "ObjectStruck",
    payload: { objectId: 1, attackerId: 55 },
    at: Date.now(),
  });
  let travelAttempts = 0;
  const travel = async (mapFileName, options = {}) => {
    travelAttempts += 1;
    assert.equal(options.interruptWhen(), false);
    client.snapshot.playerHp = 20;
    assert.equal(options.interruptWhen(), true);
    client.snapshot.playerHp = 40;
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities = [self(), monster(54, "Zombie5")];
  };
  travel.canReach = async () => true;

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: {
      kill: [{
        monsterName: "Zombie5",
        spawnCandidates: [{ ...spawn("Zombie5"), mapFileName: "D422" }],
      }],
      item: [],
    },
  }, navigateClientNear(client), {
    ...settings,
    travel,
    continueTravelWhileHealthy: true,
    multiAggressorRetreatRatio: 0.55,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(travelAttempts, 1);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [54]);
});

test("chooses an isolated quest target before a closer target inside a hostile pack", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "RedSnake", 11, 10, { disposition: "hostile" }),
    monster(61, "TigerViper", 11, 11),
    monster(62, "TigerSnake", 12, 10),
    monster(70, "RedSnake", 17, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), {
        dead: true,
        hp: 0,
      });
      state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: {
      kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake")] }],
      item: [],
    },
  }, navigateClientNear(client), settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [70]);
});

test("defers a clustered target for its current life and selects another target", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "RedSnake", 20, 10, { disposition: "hostile" }),
    monster(70, "RedSnake", 30, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  let firstApproach = true;
  const optionLog = [];
  const navigate = async (target, _range, stopWhen, options) => {
    optionLog.push(options);
    if (target.objectId === 60 && firstApproach) {
      firstApproach = false;
      Object.assign(client.snapshot.entities[0], { x: 19, y: 10 });
      client.snapshot.entities.push(monster(61, "TigerViper", 20, 11, { disposition: "hostile" }));
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    if (target.objectId == null) {
      Object.assign(client.snapshot.entities[0], { x: 0, y: 0 });
      client.snapshot.entities = client.snapshot.entities.filter(entity => entity.objectId !== 61);
      return { reached: true };
    }
    Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake")] }], item: [] },
  }, navigate, {
    ...settings,
    combatHostileClearance: 2,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [70]);
  assert.deepEqual(optionLog[0], { hostileAvoidanceRadius: 2, allowedHostileObjectIds: [60] });
});

test("reconsiders the only deferred target after enough events prove its pack dispersed", async () => {
  const quest = { questId: 30, stage: "InProgress", objectives: [objective("Collect JadeRing", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Currish", 20, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Collect JadeRing", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  let firstApproach = true;
  const navigate = async (target, _range, stopWhen) => {
    if (target.objectId === 60 && firstApproach) {
      firstApproach = false;
      Object.assign(client.snapshot.entities[0], { x: 19, y: 10 });
      client.snapshot.entities.push(monster(61, "RedSnake", 20, 11, { disposition: "hostile" }));
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    if (target.objectId == null) {
      Object.assign(client.snapshot.entities[0], { x: 0, y: 0 });
      client.snapshot.entities = client.snapshot.entities.filter(entity => entity.objectId !== 61);
      for (let index = 0; index < 128; index += 1) client.receive("KeepAlive", () => {}, { time: index });
      return { reached: true };
    }
    Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 30,
    objectives: { kill: [{ monsterName: "Currish", spawnCandidates: [spawn("Currish")] }], item: [] },
  }, navigate, {
    ...settings,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeTargetRetryEventGap: 128,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
});

test("disengages when a second hostile joins an attack already in progress", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "RedSnake", 11, 10, { disposition: "hostile" }),
    monster(70, "RedSnake", 30, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    if (command.objectId === 60) {
      owner.receive("ObjectStruck", state => {
        state.entities.find(entry => entry.objectId === 60).hp = 10;
        state.entities.push(monster(61, "TigerViper", 12, 10, { disposition: "hostile" }));
      }, { objectId: 60, attackerId: 1 });
      return;
    }
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 70), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 70 });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  const navigate = async target => {
    const actor = client.snapshot.entities[0];
    if (target.objectId == null) {
      Object.assign(actor, { x: 0, y: 0 });
      client.snapshot.entities = client.snapshot.entities.filter(entity => entity.objectId !== 61);
      return;
    }
    Object.assign(actor, { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake")] }], item: [] },
  }, navigate, {
    ...settings,
    combatHostileClearance: 2,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60, 70]);
  assert.equal(diagnostics.filter(entry => entry.type === "unsafeTargetCluster").length, 1);
});

test("clears only proven adjacent monster aggressors by lowest HP before the quest target", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "HookingCat", 11, 10, { hp: 20 }),
    monster(61, "Oma", 10, 9, { hp: 3, disposition: "hostile" }),
    monster(62, "RakingCat", 9, 10, { hp: 7, disposition: "hostile" }),
    { objectId: 63, kind: "npc", disposition: "hostile", name: "Guard", x: 10, y: 11, hp: 1 },
    monster(64, "Deer", 11, 11, { hp: 1, disposition: "neutral" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      const killed = state.entities.find(entry => entry.objectId === command.objectId);
      Object.assign(killed, { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  client.receive("ObjectAttack", () => {}, { objectId: 62, location: { x: 9, y: 10 }, direction: "Right" });
  client.receive("ObjectAttack", () => {}, { objectId: 61, location: { x: 10, y: 9 }, direction: "Down" });
  client.receive("ObjectAttack", () => {}, { objectId: 63, location: { x: 10, y: 11 }, direction: "Up" });
  client.receive("ObjectAttack", () => {}, { objectId: 64, location: { x: 11, y: 11 }, direction: "UpLeft" });
  const result = await completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] },
  }, async () => {}, settings);
  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61, 62, 60]);
  assert.equal(client.snapshot.entities.find(entry => entry.objectId === 63).hp, 1);
  assert.equal(client.snapshot.entities.find(entry => entry.objectId === 64).hp, 1);
});

test("low health under two established aggressors retreats instead of standing to clear both", async () => {
  const quest = { questId: 30, stage: "InProgress", objectives: [objective("Collect JadeRing", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Currish", 20, 10, { disposition: "hostile" }),
    monster(61, "TigerSnake", 11, 10, { hp: 20, disposition: "hostile" }),
    monster(62, "RedViper", 10, 9, { hp: 20, disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    if (command.objectId === 61) {
      owner.receive("ObjectHealth", state => {
        state.entities.find(entry => entry.objectId === 61).hp = 10;
        state.playerHp = 25;
        state.entities[0].hp = 25;
      }, { objectId: 61, percent: 50 });
      return;
    }
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Collect JadeRing", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 62 });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (options.maxSuccessfulSteps) {
      const before = { x: actor.x, y: actor.y };
      Object.assign(actor, { ...target, hp: 40 });
      client.snapshot.playerHp = 40;
      Object.assign(client.snapshot.entities[2], { x: actor.x + 20, y: actor.y + 20 });
      Object.assign(client.snapshot.entities[3], { x: actor.x + 21, y: actor.y + 20 });
      assert.equal(stopWhen(), true);
      return { reached: true, successfulSteps: distanceForTest(before, actor) };
    }
    Object.assign(actor, { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 30,
    objectives: { kill: [{ monsterName: "Currish", spawnCandidates: [spawn("Currish")] }], item: [] },
  }, navigate, { ...settings, recoverAfterUnsafeRetreat: async () => {} });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61, 60]);
  assert.ok(diagnostics.some(entry => entry.type === "unsafeTargetCluster"));
});

test("a stocked mine expedition clears two aggressors above its lower retreat threshold", async () => {
  const quest = { questId: 54, stage: "InProgress", objectives: [objective("Kill Zombie5", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Zombie5", 12, 10, { hp: 20, disposition: "hostile" }),
    monster(61, "Zombie2", 11, 10, { hp: 20, disposition: "hostile" }),
    monster(62, "Zombie3", 10, 9, { hp: 20, disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill Zombie5", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  Object.assign(client.snapshot, { playerHp: 25, playerMaxHp: 40 });
  Object.assign(client.snapshot.entities[0], { hp: 25, maxHp: 40 });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 62 });

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: { kill: [{ monsterName: "Zombie5", spawnCandidates: [spawn("Zombie5")] }], item: [] },
  }, navigateClientNear(client), { ...settings, multiAggressorRetreatRatio: 0.45 });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61, 62, 60]);
});

test("a mine expedition retreats from three active aggressors before waiting for low health", async () => {
  const quest = { questId: 54, stage: "InProgress", objectives: [objective("Kill Zombie1", 0, 1)] };
  const target = monster(60, "Zombie1", 12, 10, { hp: 20, disposition: "hostile" });
  const threats = [
    monster(61, "Zombie2", 11, 10, { hp: 20, disposition: "hostile" }),
    monster(62, "Zombie3", 10, 11, { hp: 20, disposition: "hostile" }),
    monster(63, "Zombie5", 9, 10, { hp: 20, disposition: "hostile" }),
  ];
  const client = new FakeClient(snapshot(quest, [target, ...threats]), (owner, command) => {
    if (command.type !== "attack" || command.objectId !== 60) return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { hp: 0, dead: true });
      state.questLog[0].objectives[0] = objective("Kill Zombie1", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });
  for (const threat of threats) {
    client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: threat.objectId });
  }
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let retreatMoves = 0;
  const navigate = async (destination, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (options.maxSuccessfulSteps) {
      retreatMoves += 1;
      Object.assign(actor, { x: 1, y: 1 });
      for (const threat of threats) Object.assign(threat, { x: 30, y: 30 });
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 9 };
    }
    Object.assign(actor, { x: destination.x - 1, y: destination.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: { kill: [{ monsterName: "Zombie1", spawnCandidates: [spawn("Zombie1")] }], item: [] },
  }, navigate, {
    ...settings,
    multiAggressorRetreatRatio: 0.55,
    retreatAtActiveAggressorCount: 3,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(retreatMoves, 1);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
  assert.ok(diagnostics.some(entry => entry.type === "unsafeTargetCluster"));
});

test("a focused snake pull wakes its response wait on the first extra aggressor", async () => {
  const quest = { questId: 42, stage: "InProgress", objectives: [objective("Kill RedViper", 0, 1)] };
  const target = monster(60, "RedViper", 12, 10, { hp: 20, disposition: "hostile" });
  const threat = monster(61, "Spider", 30, 30, { hp: 20, disposition: "hostile" });
  const alternative = monster(62, "RedViper", 18, 10, { hp: 20, disposition: "hostile" });
  const client = new FakeClient(snapshot(quest, [target, threat]), (owner, command) => {
    if (command.type !== "attack") return;
    if (command.objectId === 60) {
      Object.assign(threat, { x: 11, y: 10 });
      owner.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      return;
    }
    assert.equal(command.objectId, 62, "the retreated target must remain quarantined");
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 62), { hp: 0, dead: true });
      state.questLog[0].objectives[0] = objective("Kill RedViper", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 62 });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let retreatMoves = 0;
  const navigate = async (destination, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (options.maxSuccessfulSteps) {
      retreatMoves += 1;
      Object.assign(actor, { x: 1, y: 1 });
      Object.assign(threat, { x: 30, y: 30 });
      client.snapshot.entities.push(alternative);
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 9 };
    }
    Object.assign(actor, { x: destination.x - 1, y: destination.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 42,
    objectives: { kill: [{ monsterName: "RedViper", spawnCandidates: [spawn("RedViper")] }], item: [] },
  }, navigate, {
    ...settings,
    maxTargetAdjacent: 1,
    maxTargetNearby: 3,
    focusTargetThroughAggressors: true,
    retreatAtActiveAggressorCount: 1,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60, 62]);
  assert.equal(retreatMoves, 1);
  assert.ok(diagnostics.some(entry => entry.type === "focusedTargetRetreat"));
});

test("a newly revived adjacent aggressor pre-empts one established aggressor without target ping-pong", async () => {
  const quest = { questId: 54, stage: "InProgress", objectives: [objective("Kill Zombie5", 0, 1)] };
  const established = monster(20, "Zombie2", 11, 10, { hp: 20, disposition: "hostile" });
  const reviver = monster(21, "Zombie3", 11, 11, { hp: 0, dead: true, disposition: "hostile" });
  const requested = monster(30, "Zombie5", 12, 10, { disposition: "hostile" });
  let establishedFirstHit = true;
  const client = new FakeClient(snapshot(quest, [established, reviver, requested]), (owner, command) => {
    if (command.type !== "attack") return;
    if (command.objectId === 20 && establishedFirstHit) {
      establishedFirstHit = false;
      owner.receive("ObjectHealth", state => {
        state.entities.find(entry => entry.objectId === 20).hp = 10;
      }, { objectId: 20, percent: 50 });
      owner.receive("ObjectRevived", state => {
        Object.assign(state.entities.find(entry => entry.objectId === 21), { dead: false, hp: 15 });
      }, { objectId: 21, effect: false });
      owner.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 21 });
      return;
    }
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      if (command.objectId === 30) {
        state.questLog[0].objectives = [objective("Kill Zombie5", 1, 1)];
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  client.events.push({
    sequence: ++client.sequence,
    direction: "received",
    packet: "ObjectStruck",
    payload: { objectId: 1, attackerId: 20 },
    at: Date.now(),
  });

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: { kill: [{ monsterName: "Zombie5", spawnCandidates: [spawn("Zombie5")] }], item: [] },
  }, navigateClientNear(client), settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(
    client.sent.map(entry => entry.objectId),
    [20, 21, 20, 30],
    "the new revived life pre-empts once, then each established target finishes without oscillation",
  );
});

test("an objective target cannot interrupt clearing the aggressor that joined its fight", async () => {
  const quest = { questId: 54, stage: "InProgress", objectives: [objective("Kill Zombie4", 0, 1)] };
  const aggressor = monster(20, "Zombie2", 11, 10, { hp: 20, disposition: "hostile" });
  const requested = monster(30, "Zombie4", 12, 10, { hp: 20, disposition: "hostile" });
  let firstAggressorHit = true;
  const client = new FakeClient(snapshot(quest, [aggressor, requested]), (owner, command) => {
    if (command.type !== "attack") return;
    if (command.objectId === 20 && firstAggressorHit) {
      firstAggressorHit = false;
      owner.receive("ObjectHealth", state => {
        state.entities.find(entry => entry.objectId === 20).hp = 10;
      }, { objectId: 20, percent: 50 });
      owner.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 30 });
      return;
    }
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      if (command.objectId === 30) {
        state.questLog[0].objectives = [objective("Kill Zombie4", 1, 1)];
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  client.events.push({
    sequence: ++client.sequence,
    direction: "received",
    packet: "ObjectStruck",
    payload: { objectId: 1, attackerId: 20 },
    at: Date.now(),
  });

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: { kill: [{ monsterName: "Zombie4", spawnCandidates: [spawn("Zombie4")] }], item: [] },
  }, navigateClientNear(client), settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [20, 20, 30]);
});

test("interrupts target navigation to clear a newly proven adjacent aggressor, then resumes the exact target", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill TigerSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "TigerSnake", 20, 10, { hp: 20, disposition: "hostile" }),
    monster(61, "TigerViper", 14, 9, { hp: 3, disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      const killed = state.entities.find(entry => entry.objectId === command.objectId);
      Object.assign(killed, { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill TigerSnake", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  let firstTargetApproach = true;
  const navigationTargets = [];
  const navigate = async (target, _range, stopWhen) => {
    navigationTargets.push(target.objectId);
    if (target.objectId === 60 && firstTargetApproach) {
      firstTargetApproach = false;
      Object.assign(client.snapshot.entities[0], { x: 14, y: 10 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return;
    }
    Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "TigerSnake", spawnCandidates: [spawn("TigerSnake")] }], item: [] },
  }, navigate, settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61, 60]);
  assert.deepEqual(navigationTargets, [60, 61, 60]);
});

test("an unreachable proven aggressor is recorded, escaped, and does not abort the objective", async () => {
  const quest = { questId: 54, stage: "InProgress", objectives: [objective("Kill Zombie1", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Zombie1", 20, 10, { disposition: "hostile" }),
    monster(61, "Zombie2", 11, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack" || command.objectId !== 60) return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill Zombie1", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let retreatMoves = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61) throw new Error("No walk path on D401 from 25,142 to 27,139");
    if (options.maxSuccessfulSteps) {
      retreatMoves += 1;
      Object.assign(actor, target);
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 61), { x: 50, y: 50 });
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 2 };
    }
    if (target.objectId === 60) Object.assign(actor, { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: { kill: [{ monsterName: "Zombie1", spawnCandidates: [spawn("Zombie1")] }], item: [] },
  }, navigate, { ...settings, maxEngagements: 6, recoverAfterUnsafeRetreat: async () => {} });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(retreatMoves, 1);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
  assert.ok(diagnostics.some(entry =>
    entry.type === "unreachableCombatTarget" && entry.objectId === 61));
});

test("a healthy focused ranged hunt finishes its quest target through one proven aggressor", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "RedSnake", 11, 10, { hp: 5, disposition: "hostile" }),
    monster(61, "Oma", 10, 9, { hp: 30, disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack" || command.objectId !== 60) return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });
  Object.assign(client.snapshot, { playerHp: 49, playerMaxHp: 52 });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake")] }], item: [] },
  }, async () => {}, {
    ...settings,
    maxTargetAdjacent: 1,
    maxTargetNearby: 4,
    focusTargetThroughAggressors: true,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
  assert.equal(client.snapshot.entities.find(entry => entry.objectId === 61).dead, false);
});

test("q89 clears one proven entrance aggressor before retaining the required target", async () => {
  const quest = { questId: 89, stage: "InProgress", objectives: [objective("Kill CursedZombie", 0, 1)] };
  const blocker = monster(71, "CursedShaman", 11, 10, { disposition: "hostile" });
  const target = monster(70, "CursedZombie", 20, 10, { disposition: "hostile" });
  const client = new FakeClient(snapshot(quest, [blocker, target]), (owner, command) => {
    if (command.type !== "attack") return;
    const attacked = owner.snapshot.entities.find(entry => entry.objectId === command.objectId);
    if (command.objectId === blocker.objectId) {
      Object.assign(attacked, { hp: 0, dead: true });
      owner.receive("ObjectDied", () => {}, { objectId: blocker.objectId });
      return;
    }
    assert.equal(command.objectId, target.objectId);
    Object.assign(attacked, { hp: 0, dead: true });
    owner.receive("ObjectDied", state => {
      state.questLog[0].objectives[0] = objective("Kill CursedZombie", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: target.objectId });
  });
  Object.assign(client.snapshot, { playerHp: 28, playerMaxHp: 40 });
  Object.assign(client.snapshot.entities[0], { hp: 28, maxHp: 40 });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: blocker.objectId });

  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: "CursedZombie", spawnCandidates: [spawn("CursedZombie")] }], item: [] },
  }, navigateClientNear(client), {
    ...settings,
    maxTargetAdjacent: 0,
    maxTargetNearby: 2,
    directAggressorDistance: 8,
    multiAggressorRetreatRatio: 0.75,
    retreatAtActiveAggressorCount: 2,
    focusTargetThroughAggressors: false,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [blocker.objectId, target.objectId]);
});

test("q89 still retreats at 75 percent HP when two aggressors are proven", async () => {
  const quest = { questId: 89, stage: "InProgress", objectives: [objective("Kill CursedZombie", 0, 1)] };
  const target = monster(70, "CursedZombie", 20, 10, { disposition: "hostile" });
  const threats = [
    monster(71, "CursedShaman", 11, 10, { disposition: "hostile" }),
    monster(72, "CursedShaman", 10, 11, { disposition: "hostile" }),
  ];
  const client = new FakeClient(snapshot(quest, [target, ...threats]), (owner, command) => {
    if (command.type !== "attack") return;
    assert.equal(command.objectId, target.objectId);
    Object.assign(target, { hp: 0, dead: true });
    owner.receive("ObjectDied", state => {
      state.questLog[0].objectives[0] = objective("Kill CursedZombie", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: target.objectId });
  });
  Object.assign(client.snapshot, { playerHp: 28, playerMaxHp: 40 });
  Object.assign(client.snapshot.entities[0], { hp: 28, maxHp: 40 });
  for (const threat of threats) {
    client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: threat.objectId });
  }
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let retreatMoves = 0;
  const navigate = async (destination, _range, stopWhen, options = {}) => {
    if (options.maxSuccessfulSteps) {
      retreatMoves += 1;
      Object.assign(client.snapshot.entities[0], { x: 1, y: 1 });
      for (const threat of threats) Object.assign(threat, { x: 30, y: 30 });
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 10 };
    }
    Object.assign(client.snapshot.entities[0], { x: destination.x - 1, y: destination.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: "CursedZombie", spawnCandidates: [spawn("CursedZombie")] }], item: [] },
  }, navigate, {
    ...settings,
    maxTargetAdjacent: 0,
    maxTargetNearby: 2,
    directAggressorDistance: 8,
    multiAggressorRetreatRatio: 0.75,
    retreatAtActiveAggressorCount: 2,
    focusTargetThroughAggressors: false,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(retreatMoves, 1);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [target.objectId]);
  assert.ok(diagnostics.some(entry => entry.type === "unsafeTargetCluster"));
});

test("q89 Wizard arms the exact live D2031 doorway after a stalled full Priest spread before respawn waiting", async () => {
  const quest = { questId: 89, stage: "InProgress", objectives: [objective("Kill CursedPriest", 2, 3)] };
  const state = snapshot(quest);
  const doorway = {
    key: "crystal-move:d2031:198:34:117:184:267",
    mapFileName: "D2031", toMapFileName: "D2032",
    bounds: { minX: 198, maxX: 198, minY: 34, maxY: 34 },
  };
  Object.assign(state, { mapFileName: "D2031", mapTransfers: [doorway] });
  const client = new FakeClient(state, (owner, command) => {
    if (command.type !== "attack" || command.objectId !== 72) return;
    owner.receive("ObjectDied", current => {
      Object.assign(current.entities.find(entry => entry.objectId === 72), { dead: true, hp: 0 });
      current.questLog[0].objectives[0] = objective("Kill CursedPriest", 3, 3);
      current.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 72 });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  const travelled = [];
  const travel = async (mapFileName, options) => {
    travelled.push({ mapFileName, options });
    assert.equal(mapFileName, "D2032");
    assert.deepEqual(options.preferredTransferSource, { x: 198, y: 34 });
    assert.equal(options.preferredTransferKey, doorway.key);
    Object.assign(client.snapshot, { mapFileName: "D2032", mapTransfers: [] });
    Object.assign(client.snapshot.entities[0], { x: 184, y: 267 });
    client.snapshot.entities = [client.snapshot.entities[0], monster(72, "CursedPriest", 185, 267)];
  };
  travel.routeLength = async () => null;
  const navigate = async target => {
    if (client.snapshot.mapFileName === "D2031") {
      throw new Error(`No walk path on D2031 to ${target.x},${target.y}`);
    }
    Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: "CursedPriest", spawnCandidates: [
      { ...spawn("CursedPriest", 250, 250), mapFileName: "D2031", spread: 0 },
      { ...spawn("CursedPriest", 185, 267), mapFileName: "D2032", spread: 0, respawnIndex: 999 },
    ] }], item: [] },
  }, navigate, {
    ...settings,
    travel,
    maxSpawnRespawnWaits: 0,
    objectiveMapFallback: {
      fromMapFileName: "D2031", toMapFileName: "D2032",
      transferCandidates: [{ key: doorway.key, source: { x: 198, y: 34 } }],
    },
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(travelled.map(entry => entry.mapFileName), ["D2032"]);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [72]);
  assert.ok(diagnostics.some(entry => entry.type === "objectiveMapFallbackArmed" &&
    entry.reason === "stalledFullSpreadWithoutPhysicalProgress"));
  assert.ok(diagnostics.some(entry => entry.type === "objectiveMapFallbackEntered"));
  assert.equal(diagnostics.some(entry => entry.type === "spawnRespawnWait"), false);
});

test("a q89 Priest full-spread pass with physical progress does not arm the D2032 fallback", async () => {
  const quest = { questId: 89, stage: "InProgress", objectives: [objective("Kill CursedPriest", 2, 3)] };
  const doorway = {
    key: "crystal-move:d2031:198:34:117:184:267",
    mapFileName: "D2031", toMapFileName: "D2032",
    bounds: { minX: 198, maxX: 198, minY: 34, maxY: 34 },
  };
  const state = snapshot(quest);
  Object.assign(state, { mapFileName: "D2031", mapTransfers: [doorway] });
  const client = new FakeClient(state);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let travelCalls = 0;
  const travel = async () => { travelCalls += 1; };
  let moved = false;
  const navigate = async () => {
    if (!moved) {
      moved = true;
      Object.assign(client.snapshot.entities[0], { x: 11, y: 10 });
    }
    throw new Error("No walk path on D2031 during covered Priest search");
  };

  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: "CursedPriest", spawnCandidates: [
      { ...spawn("CursedPriest", 250, 250), mapFileName: "D2031", spread: 0 },
      { ...spawn("CursedPriest", 185, 267), mapFileName: "D2032", spread: 0, respawnIndex: 999 },
    ] }], item: [] },
  }, navigate, {
    ...settings,
    travel,
    maxSpawnRespawnWaits: 0,
    objectiveMapFallback: {
      fromMapFileName: "D2031", toMapFileName: "D2032",
      transferCandidates: [{ key: doorway.key, source: { x: 198, y: 34 } }],
    },
  }), /bounded full-spread spawn search exhausted/);

  assert.equal(travelCalls, 0);
  assert.equal(diagnostics.some(entry => entry.type === "objectiveMapFallbackArmed"), false);
});

test("a q89 Priest full-spread pass with unknown physical coordinates does not arm the D2032 fallback", async () => {
  const quest = { questId: 89, stage: "InProgress", objectives: [objective("Kill CursedPriest", 2, 3)] };
  const doorway = {
    key: "crystal-move:d2031:198:34:117:184:267",
    mapFileName: "D2031", toMapFileName: "D2032",
    bounds: { minX: 198, maxX: 198, minY: 34, maxY: 34 },
  };
  const state = snapshot(quest);
  Object.assign(state, { mapFileName: "D2031", mapTransfers: [doorway] });
  Object.assign(state.entities[0], { x: null, y: "" });
  const client = new FakeClient(state);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let travelCalls = 0;

  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: "CursedPriest", spawnCandidates: [
      { ...spawn("CursedPriest", 250, 250), mapFileName: "D2031", spread: 0 },
      { ...spawn("CursedPriest", 185, 267), mapFileName: "D2032", spread: 0, respawnIndex: 999 },
    ] }], item: [] },
  }, async target => {
    throw new Error(`No walk path on D2031 to ${target.x},${target.y}`);
  }, {
    ...settings,
    travel: async () => { travelCalls += 1; },
    maxSpawnRespawnWaits: 0,
    objectiveMapFallback: {
      fromMapFileName: "D2031", toMapFileName: "D2032",
      transferCandidates: [{ key: doorway.key, source: { x: 198, y: 34 } }],
    },
  }), /bounded full-spread spawn search exhausted/);

  assert.equal(travelCalls, 0);
  assert.equal(diagnostics.some(entry => entry.type === "objectiveMapFallbackArmed"), false);
});

test("a catalogued D2032 Priest cannot arm a stalled q89 fallback without the exact live D2031 source and key", async () => {
  const quest = { questId: 89, stage: "InProgress", objectives: [objective("Kill CursedPriest", 2, 3)] };
  const state = snapshot(quest);
  Object.assign(state, {
    mapFileName: "D2031",
    mapTransfers: [{
      key: "crystal-move:d2031:198:35:117:184:267",
      mapFileName: "D2031", toMapFileName: "D2032",
      bounds: { minX: 198, maxX: 198, minY: 35, maxY: 35 },
    }],
  });
  const client = new FakeClient(state);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let travelCalls = 0;
  const navigate = async target => {
    throw new Error(`No walk path on D2031 to ${target.x},${target.y}`);
  };

  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: "CursedPriest", spawnCandidates: [
      { ...spawn("CursedPriest", 250, 250), mapFileName: "D2031", spread: 0 },
      { ...spawn("CursedPriest", 185, 267), mapFileName: "D2032", spread: 0, respawnIndex: 999 },
    ] }], item: [] },
  }, navigate, {
    ...settings,
    travel: async () => { travelCalls += 1; },
    maxSpawnRespawnWaits: 0,
    objectiveMapFallback: {
      fromMapFileName: "D2031", toMapFileName: "D2032",
      transferCandidates: [{
        key: "crystal-move:d2031:198:34:117:184:267", source: { x: 198, y: 34 },
      }],
    },
  }), /bounded full-spread spawn search exhausted/);

  assert.equal(travelCalls, 0);
  assert.ok(diagnostics.some(entry => entry.type === "objectiveMapFallbackUnavailable" &&
    entry.reason === "stalledFullSpreadWithoutPhysicalProgress"));
  assert.equal(diagnostics.some(entry => entry.type === "objectiveMapFallbackArmed"), false);
});

test("the stalled Priest fallback stays unavailable for other q89 objectives", async () => {
  const quest = { questId: 89, stage: "InProgress", objectives: [objective("Kill CursedZombie", 2, 3)] };
  const doorway = {
    key: "crystal-move:d2031:198:34:117:184:267",
    mapFileName: "D2031", toMapFileName: "D2032",
    bounds: { minX: 198, maxX: 198, minY: 34, maxY: 34 },
  };
  const state = snapshot(quest);
  Object.assign(state, { mapFileName: "D2031", mapTransfers: [doorway] });
  const client = new FakeClient(state);
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);

  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: "CursedZombie", spawnCandidates: [
      { ...spawn("CursedZombie", 250, 250), mapFileName: "D2031", spread: 0 },
      { ...spawn("CursedZombie", 185, 267), mapFileName: "D2032", spread: 0, respawnIndex: 999 },
    ] }], item: [] },
  }, async target => {
    throw new Error(`No walk path on D2031 to ${target.x},${target.y}`);
  }, {
    ...settings,
    maxSpawnRespawnWaits: 0,
    objectiveMapFallback: {
      fromMapFileName: "D2031", toMapFileName: "D2032",
      transferCandidates: [{ key: doorway.key, source: { x: 198, y: 34 } }],
    },
  }), /bounded full-spread spawn search exhausted/);

  assert.equal(diagnostics.some(entry => entry.type === "objectiveMapFallbackArmed"), false);
});

test("q89 Wizard returns normally to D2031 before using the live keyed doorway after a retreat", async () => {
  const quest = { questId: 89, stage: "InProgress", objectives: [objective("Kill CursedZombie", 0, 1)] };
  const required = monster(70, "CursedZombie", 20, 10, { disposition: "hostile" });
  const unrelatedAggressor = monster(71, "CursedZombie0", 11, 10, { disposition: "hostile" });
  const state = snapshot(quest, [required, unrelatedAggressor]);
  Object.assign(state, {
    mapFileName: "D2031",
    mapTransfers: [{
      key: "crystal-move:d2031:198:34:117:184:267",
      mapFileName: "D2031", toMapFileName: "D2032",
      bounds: { minX: 198, maxX: 198, minY: 34, maxY: 34 },
    }],
  });
  const client = new FakeClient(state, (owner, command) => {
    if (command.type !== "attack") return;
    assert.equal(command.objectId, 72, "the unrelated CursedZombie0 must never be attacked");
    owner.receive("ObjectDied", current => {
      Object.assign(current.entities.find(entry => entry.objectId === 72), { dead: true, hp: 0 });
      current.questLog[0].objectives[0] = objective("Kill CursedZombie", 1, 1);
      current.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 72 });
  });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: unrelatedAggressor.objectId });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  const travelled = [];
  const travel = async (mapFileName, options) => {
    travelled.push({ mapFileName, options });
    if (mapFileName === "D2031") {
      assert.equal(options.preferredTransferSource, undefined);
      assert.equal(options.preferredTransferKey, undefined);
      Object.assign(client.snapshot, {
        mapFileName: "D2031",
        mapTransfers: [{
          key: "crystal-move:d2031:198:34:117:184:267",
          mapFileName: "D2031", toMapFileName: "D2032",
          bounds: { minX: 198, maxX: 198, minY: 34, maxY: 34 },
        }],
      });
      return;
    }
    assert.equal(mapFileName, "D2032");
    assert.deepEqual(options.preferredTransferSource, { x: 198, y: 34 });
    assert.equal(options.preferredTransferKey, "crystal-move:d2031:198:34:117:184:267");
    Object.assign(client.snapshot, { mapFileName: "D2032", mapTransfers: [] });
    Object.assign(client.snapshot.entities[0], { x: 184, y: 267 });
    client.snapshot.entities = [client.snapshot.entities[0], monster(72, "CursedZombie", 185, 267)];
  };
  // The profile-pruned topology intentionally reports no route to D2032;
  // the armed live doorway must still be handed to the normal traveler.
  travel.routeLength = async () => null;
  const navigate = async (target, _range, _stopWhen, options = {}) => {
    if (target.objectId === unrelatedAggressor.objectId) {
      client.snapshot.entities = client.snapshot.entities.filter(entry => entry.objectId !== required.objectId);
      Object.assign(unrelatedAggressor, { x: 50, y: 50 });
      return;
    }
    if (options.maxSuccessfulSteps) return;
    Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: "CursedZombie", spawnCandidates: [
      { ...spawn("CursedZombie"), mapFileName: "D2031" },
      { ...spawn("CursedZombie"), mapFileName: "D2032", respawnIndex: 999 },
    ] }], item: [] },
  }, navigate, {
    ...settings,
    travel,
    maxTargetAdjacent: 0,
    maxTargetNearby: 2,
    directAggressorDistance: 8,
    recoverAfterUnsafeRetreat: async current => {
      Object.assign(current.snapshot, { mapFileName: "0", mapTransfers: [] });
    },
    objectiveMapFallback: {
      fromMapFileName: "D2031", toMapFileName: "D2032",
      transferCandidates: [{
        key: "crystal-move:d2031:198:34:117:184:267", source: { x: 198, y: 34 },
      }],
    },
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(travelled.map(entry => entry.mapFileName), ["D2031", "D2032"]);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [72]);
  assert.ok(diagnostics.some(entry => entry.type === "objectiveMapFallbackArmed"));
  assert.ok(diagnostics.some(entry => entry.type === "objectiveMapFallbackEntered"));
});

test("q89 Wizard fails closed when normal fallback-source travel lands on the wrong map", async () => {
  const quest = { questId: 89, stage: "InProgress", objectives: [objective("Kill CursedZombie", 0, 1)] };
  const required = monster(70, "CursedZombie", 20, 10, { disposition: "hostile" });
  const unrelatedAggressor = monster(71, "CursedZombie0", 11, 10, { disposition: "hostile" });
  const state = snapshot(quest, [required, unrelatedAggressor]);
  Object.assign(state, {
    mapFileName: "D2031",
    mapTransfers: [{
      key: "crystal-move:d2031:198:34:117:184:267",
      mapFileName: "D2031", toMapFileName: "D2032",
      bounds: { minX: 198, maxX: 198, minY: 34, maxY: 34 },
    }],
  });
  const client = new FakeClient(state);
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: unrelatedAggressor.objectId });
  const travelled = [];
  const travel = async (mapFileName, options) => {
    travelled.push({ mapFileName, options });
    assert.equal(mapFileName, "D2031");
    assert.equal(options.preferredTransferSource, undefined);
    assert.equal(options.preferredTransferKey, undefined);
    Object.assign(client.snapshot, { mapFileName: "0", mapTransfers: [] });
  };
  const navigate = async (target, _range, _stopWhen, options = {}) => {
    if (target.objectId === unrelatedAggressor.objectId) {
      client.snapshot.entities = client.snapshot.entities.filter(entry => entry.objectId !== required.objectId);
      Object.assign(unrelatedAggressor, { x: 50, y: 50 });
      return;
    }
    if (options.maxSuccessfulSteps) return;
    Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
  };

  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: "CursedZombie", spawnCandidates: [
      { ...spawn("CursedZombie"), mapFileName: "D2031" },
      { ...spawn("CursedZombie"), mapFileName: "D2032", respawnIndex: 999 },
    ] }], item: [] },
  }, navigate, {
    ...settings,
    travel,
    maxTargetAdjacent: 0,
    maxTargetNearby: 2,
    directAggressorDistance: 8,
    recoverAfterUnsafeRetreat: async current => {
      Object.assign(current.snapshot, { mapFileName: "0", mapTransfers: [] });
    },
    objectiveMapFallback: {
      fromMapFileName: "D2031", toMapFileName: "D2032",
      transferCandidates: [{
        key: "crystal-move:d2031:198:34:117:184:267", source: { x: 198, y: 34 },
      }],
    },
  }), /q89 objective-map fallback did not authoritatively return to D2031/);
  assert.deepEqual(travelled.map(entry => entry.mapFileName), ["D2031"]);
});

test("q89 Wizard fails closed after one live D2032 fallback pass makes no objective progress", async () => {
  const quest = { questId: 89, stage: "InProgress", objectives: [objective("Kill CursedZombie", 0, 1)] };
  const required = monster(70, "CursedZombie", 20, 10, { disposition: "hostile" });
  const unrelatedAggressor = monster(71, "CursedZombie0", 11, 10, { disposition: "hostile" });
  const state = snapshot(quest, [required, unrelatedAggressor]);
  Object.assign(state, {
    mapFileName: "D2031",
    mapTransfers: [{
      key: "crystal-move:d2031:198:34:117:184:267",
      mapFileName: "D2031", toMapFileName: "D2032",
      bounds: { minX: 198, maxX: 198, minY: 34, maxY: 34 },
    }],
  });
  const client = new FakeClient(state);
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: unrelatedAggressor.objectId });
  const travelled = [];
  const travel = async mapFileName => {
    travelled.push(mapFileName);
    Object.assign(client.snapshot, { mapFileName: "D2032", mapTransfers: [] });
    Object.assign(client.snapshot.entities[0], { x: 184, y: 267 });
    client.snapshot.entities = [client.snapshot.entities[0], monster(72, "CursedZombie", 185, 267)];
  };
  travel.routeLength = async mapFileName => mapFileName === "D2032" ? 1 : 2;
  const navigate = async target => {
    if (target.objectId === unrelatedAggressor.objectId || target.objectId === 72) {
      client.snapshot.entities = client.snapshot.entities.filter(entry => entry.objectId !== target.objectId && entry.objectId !== required.objectId);
    }
  };

  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: "CursedZombie", spawnCandidates: [
      { ...spawn("CursedZombie"), mapFileName: "D2031" },
      { ...spawn("CursedZombie"), mapFileName: "D2032", respawnIndex: 999 },
    ] }], item: [] },
  }, navigate, {
    ...settings,
    travel,
    directAggressorDistance: 8,
    objectiveMapFallback: {
      fromMapFileName: "D2031", toMapFileName: "D2032",
      transferCandidates: [{
        key: "crystal-move:d2031:198:34:117:184:267", source: { x: 198, y: 34 },
      }],
    },
  }), /q89 objective-map fallback D2032 made no objective progress/);
  assert.deepEqual(travelled, ["D2032"]);
  assert.deepEqual(client.sent, []);
});

test("a survivable Taoist lands the last bounded hit before retreating from a joining snake", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  let attacks = 0;
  const client = new FakeClient(snapshot(quest, [
    monster(60, "RedSnake", 11, 10, { hp: 12, maxHp: 60, disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack" || command.objectId !== 60) return;
    attacks += 1;
    if (attacks === 1) {
      owner.receive("ObjectHealth", state => {
        Object.assign(state.entities.find(entry => entry.objectId === 60), { hp: 3, maxHp: 60 });
        Object.assign(state, { playerHp: 46, playerMaxHp: 81 });
        Object.assign(state.entities[0], { hp: 46, maxHp: 81 });
        state.entities.push(monster(61, "TigerSnake", 10, 9, { hp: 60, maxHp: 60, disposition: "hostile" }));
      }, { objectId: 60, percent: 5 });
      owner.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      return;
    }
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake")] }], item: [] },
  }, async () => {}, {
    ...settings,
    maxTargetAdjacent: 0,
    maxTargetNearby: 1,
    lowHealthTargetRetreatRatio: 0.65,
    multiAggressorRetreatRatio: 0.7,
    focusTargetThroughAggressors: true,
    finishableTargetHealthRatio: 0.25,
    finishableTargetMinimumPlayerHpRatio: 0.4,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60, 60]);
  assert.equal(client.snapshot.entities.find(entry => entry.objectId === 61).dead, false);
});

test("a proven current target forces retreat below forty percent health before combat resumes", async () => {
  const quest = { questId: 36, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(60, "RedSnake", 11, 10, { disposition: "hostile" })]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });
  client.snapshot.playerHp = 15;
  Object.assign(client.snapshot.entities[0], { hp: 15, maxHp: 40 });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 60 });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let retreatMoves = 0;
  const retreatOrder = [];
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (options.maxSuccessfulSteps) {
      retreatOrder.push("move");
      retreatMoves += 1;
      Object.assign(actor, { x: 20, y: 20 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 60), { x: 30, y: 30 });
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 10 };
    }
    Object.assign(actor, { x: target.x - 1, y: target.y });
    return { reached: true };
  };

  const result = await completeQuestObjectives(client, {
    questId: 36,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    allowLowHealthFollowerRecovery: true,
    sustain: async (_owner, context) => {
      if (context.phase === "retreat") retreatOrder.push("sustain");
    },
    recoverAfterUnsafeRetreat: async owner => {
      owner.snapshot.playerHp = 40;
      owner.snapshot.entities[0].hp = 40;
    },
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(retreatMoves, 1);
  assert.deepEqual(retreatOrder.slice(0, 2), ["move", "sustain"]);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
  assert.ok(diagnostics.some(entry => entry.type === "lowHealthTargetRetreat" && entry.hp === 15));
});

test("interrupts an in-progress target fight when a new adjacent aggressor lands a hit", async () => {
  const quest = { questId: 30, stage: "InProgress", objectives: [objective("Kill Currish", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Currish", 11, 10, { hp: 20, disposition: "hostile" }),
    monster(61, "Oma", 10, 9, { hp: 3, disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    if (command.objectId === 60 && owner.snapshot.entities.find(entry => entry.objectId === 60).hp === 20) {
      owner.receive("ObjectStruck", state => {
        state.entities.find(entry => entry.objectId === 60).hp = 15;
      }, { objectId: 60, attackerId: 1 });
      owner.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      return;
    }
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });

      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill Currish", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });

  const result = await completeQuestObjectives(client, {
    questId: 30,
    objectives: { kill: [{ monsterName: "Currish", spawnCandidates: [spawn("Currish")] }], item: [] },
  }, async () => {}, settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60, 61, 60]);
});

test("a supplied harvest hunt focuses its unique target and carves before clearing aggressors", async () => {
  const quest = { questId: 30, stage: "InProgress", objectives: [objective("Collect JadeRing", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Currish", 11, 10, { hp: 20, disposition: "hostile" }),
    monster(61, "Oma", 10, 9, { hp: 3, disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type === "attack" && command.objectId === 60) {
      owner.receive("ObjectDied", state => {
        Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      }, { objectId: 60 });
    }
    if (command.type === "harvest") {
      owner.receive("ObjectHarvested", state => {
        state.questLog[0].objectives[0] = objective("Collect JadeRing", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }, { objectId: 60 });
    }
  });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });

  const result = await completeQuestObjectives(client, {
    questId: 30,
    objectives: { kill: [], item: [{
      itemName: "JadeRing",
      sources: [{ monsterName: "Currish", requiresHarvest: true, spawnCandidates: [spawn("Currish")] }],
    }] },
  }, async () => {}, {
    ...settings,
    maxTargetAdjacent: 1,
    maxTargetNearby: 1,
    focusTargetThroughAggressors: true,
    harvestBeforeClearingAggressors: true,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent, [
    { type: "attack", objectId: 60 },
    { type: "harvest", direction: "Right" },
  ]);
  assert.equal(client.snapshot.entities.find(entry => entry.objectId === 61).dead, false);
});

test("a focused harvest hunt retreats after two proven attackers converge, then retries the target", async () => {
  const quest = { questId: 30, stage: "InProgress", objectives: [objective("Collect JadeRing", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Currish", 20, 10, { disposition: "hostile" }),
    monster(61, "RedSnake", 30, 30, { disposition: "hostile" }),
    monster(62, "TigerSnake", 31, 30, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type === "attack" && command.objectId === 60) {
      owner.receive("ObjectDied", state => {
        Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      }, { objectId: 60 });
    }
    if (command.type === "harvest") {
      owner.receive("ObjectHarvested", state => {
        state.questLog[0].objectives[0] = objective("Collect JadeRing", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }, { objectId: 60 });
    }
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let firstApproach = true;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    if (target.objectId === 60 && firstApproach) {
      firstApproach = false;
      Object.assign(client.snapshot.entities[0], { x: 19, y: 10 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 61), { x: 19, y: 9 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 62), { x: 20, y: 9 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 62 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    if (options.maxSuccessfulSteps) {
      Object.assign(client.snapshot.entities[0], { x: 5, y: 5 });
      client.snapshot.entities = client.snapshot.entities.filter(entry => ![61, 62].includes(entry.objectId));
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 8 };
    }
    Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 30,
    objectives: { kill: [], item: [{
      itemName: "JadeRing",
      sources: [{ monsterName: "Currish", requiresHarvest: true, spawnCandidates: [spawn("Currish")] }],
    }] },
  }, navigate, {
    ...settings,
    maxTargetAdjacent: 2,
    maxTargetNearby: 4,
    focusTargetThroughAggressors: true,
    harvestBeforeClearingAggressors: true,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId ?? entry.type), [60, "harvest"]);
  assert.ok(diagnostics.some(entry => entry.type === "focusedTargetRetreat"));
});

test("trusts objective progress from a proven same-species aggressor without an extra target kill", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill TigerSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "TigerSnake", 20, 10, { disposition: "hostile" }),
    monster(61, "TigerSnake", 14, 9, { hp: 3, disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill TigerSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  let interrupted = false;
  const navigate = async (target, _range, stopWhen) => {
    if (target.objectId === 60 && !interrupted) {
      interrupted = true;
      Object.assign(client.snapshot.entities[0], { x: 14, y: 10 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return;
    }
    Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "TigerSnake", spawnCandidates: [spawn("TigerSnake")] }], item: [] },
  }, navigate, settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61]);
  assert.equal(client.snapshot.entities.find(entry => entry.objectId === 60).dead, false);
});

test("applies the same threat interrupt when sustain makes a second target approach necessary", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill TigerSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "TigerSnake", 11, 10, { disposition: "hostile" }),
    monster(61, "TigerViper", 10, 9, { hp: 3, disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill TigerSnake", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  let sustained = false;
  let secondApproachInterrupted = false;
  const navigate = async (target, range, stopWhen) => {
    if (target.objectId === 60 && Number(target.x) === 20 && !secondApproachInterrupted) {
      secondApproachInterrupted = true;
      assert.equal(stopWhen(), true);
      return;
    }
    const actor = client.snapshot.entities[0];
    if (Math.max(Math.abs(actor.x - target.x), Math.abs(actor.y - target.y)) > range) {
      Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
    }
  };
  const sustain = async owner => {
    if (sustained) return;
    sustained = true;
    owner.snapshot.entities.find(entry => entry.objectId === 60).x = 20;
    owner.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "TigerSnake", spawnCandidates: [spawn("TigerSnake")] }], item: [] },
  }, navigate, { ...settings, sustain, sustainCadenceMs: 0 });

  assert.equal(secondApproachInterrupted, true);
  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61, 60]);
});

test("does not guess that an adjacent hostile is an aggressor without attack-recipient evidence", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "HookingCat", 11, 10), monster(61, "Oma", 10, 9, { hp: 1 }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  await completeQuestObjectives(client, {
    questId: 6, objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] },
  }, async () => {}, settings);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
});

test("searches the nearest same-map spawn when no target is in AOI", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  const visits = [];
  const navigate = async (point, range) => {
    visits.push([point, range]);
    Object.assign(client.snapshot.entities[0], { x: point.x - 1, y: point.y });
    if (point.x === 14 && !client.snapshot.entities.some(entry => entry.objectId === 60)) client.snapshot.entities.push(monster(60, "HookingCat", 14, 10));
  };
  await completeQuestObjectives(client, { questId: 6, objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat", 30, 30), spawn("HookingCat", 14, 10)] }], item: [] } }, navigate, settings);
  assert.equal(visits[0][0].x, 14);
  assert.equal(client.sent[0].objectId, 60);
});

test("waits for a visible unique target pack to disperse before bounded search failure", async () => {
  const quest = { questId: 30, stage: "InProgress", objectives: [objective("Collect JadeRing", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Currish", 20, 20, { disposition: "hostile" }),
    monster(61, "RedSnake", 20, 21, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Collect JadeRing", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });
  client.wait = async (predicate, label) => {
    if (label.includes("after pack dispersal")) {
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 61), { x: 40, y: 40 });
      const result = predicate();
      assert.ok(result);
      return result;
    }
    const result = predicate();
    if (!result) throw new Error(`Fake timeout waiting for ${label}`);
    return result;
  };
  const visits = [];
  const navigate = async (target, range) => {
    visits.push({ target, range });
    if (target.objectId != null) {
      Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
    }
  };

  const result = await completeQuestObjectives(client, {
    questId: 30,
    objectives: { kill: [], item: [{
      itemName: "JadeRing",
      sources: [{ monsterName: "Currish", requiresHarvest: true, spawnCandidates: [spawn("Currish", 20, 20)] }],
    }] },
  }, navigate, {
    ...settings,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeTargetObservationTimeout: 10,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.ok(visits.some(visit => visit.target.objectId == null));
  assert.deepEqual(client.sent.map(command => command.objectId), [60]);
});

test("unsafe-target observation retreats on the first proven hit instead of waiting in place", async () => {
  const quest = { questId: 30, stage: "InProgress", objectives: [objective("Collect JadeRing", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Currish", 20, 20, { disposition: "hostile" }),
    monster(61, "RedSnake", 21, 20, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Collect JadeRing", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  let observationInterrupted = false;
  client.wait = async (predicate, label) => {
    if (label.includes("after pack dispersal")) {
      observationInterrupted = true;
      Object.assign(client.snapshot.entities[0], { x: 20, y: 21 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 60), { x: 20, y: 20 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 61), { x: 21, y: 21 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(predicate(), true);
      return true;
    }
    const result = predicate();
    if (!result) throw new Error(`Fake timeout waiting for ${label}`);
    return result;
  };
  let retreats = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (options.maxSuccessfulSteps) {
      retreats += 1;
      Object.assign(actor, { x: 30, y: 30 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 60), { x: 36, y: 30 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 61), { x: 60, y: 60 });
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 10 };
    }
    if (target.objectId != null) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    Object.assign(actor, target);
    stopWhen?.();
    return { reached: true };
  };

  const result = await completeQuestObjectives(client, {
    questId: 30,
    objectives: { kill: [], item: [{
      itemName: "JadeRing",
      sources: [{ monsterName: "Currish", requiresHarvest: false, spawnCandidates: [spawn("Currish", 20, 20)] }],
    }] },
  }, navigate, {
    ...settings,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeRetreatSteps: 10,
    unsafeRetreatSafeDistance: 10,
    unsafeTargetObservationTimeout: 1_000,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(observationInterrupted, true);
  assert.equal(retreats, 1);
  assert.deepEqual(client.sent.map(command => command.objectId), [60]);
});

test("interrupts spawn navigation for a proven aggressor, clears it within budget, and resumes search", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill TigerSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(61, "TigerViper", 100, 100, { hp: 3, disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill TigerSnake", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  let searchVisits = 0;
  const navigate = async (target, _range, stopWhen) => {
    if (target.objectId != null) {
      Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
      return;
    }
    searchVisits += 1;
    Object.assign(client.snapshot.entities[0], { x: target.x, y: target.y });
    if (searchVisits === 1) {
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 61), { x: target.x, y: target.y - 1 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return;
    }
    client.snapshot.entities.push(monster(60, "TigerSnake", target.x, target.y, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "TigerSnake", spawnCandidates: [spawn("TigerSnake", 30, 30)] }], item: [] },
  }, navigate, { ...settings, maxEngagements: 4 });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61, 60]);
  assert.equal(searchVisits, 2);
});

test("a failed spawn route sustains the player and interrupts immediately for a proven aggressor", async () => {
  const quest = { questId: 62, stage: "InProgress", objectives: [objective("Kill KekTal", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(61, "SpiderFrog", 20, 19, { hp: 3, disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill KekTal", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  let searchVisits = 0;
  let sustainCalls = 0;
  const navigate = async target => {
    if (target.objectId != null) {
      Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
      return;
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(client.snapshot.entities[0], { x: 20, y: 20 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      throw new Error("No walk path on D2042 from 20,20 to 50,50");
    }
    client.snapshot.entities.push(monster(60, "KekTal", target.x, target.y, { disposition: "hostile" }));
  };

  const result = await completeQuestObjectives(client, {
    questId: 62,
    objectives: { kill: [{ monsterName: "KekTal", spawnCandidates: [spawn("KekTal", 50, 50)] }], item: [] },
  }, navigate, {
    ...settings,
    sustain: async () => { sustainCalls += 1; },
    maxEngagements: 4,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(sustainCalls, 1);
  assert.equal(searchVisits, 2);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61, 60]);
});

test("retreats from a clustered proven aggressor and resumes the spawn search", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(61, "TigerSnake", 100, 100, { disposition: "hostile" }),
    monster(62, "TigerViper", 101, 100, { disposition: "hostile" }),
    monster(63, "TigerSnake", 100, 101, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let recoveries = 0;
  let searchVisits = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61) {
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.maxSuccessfulSteps) {
      Object.assign(actor, { x: 30, y: 30 });
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 8 };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 61), { x: 20, y: 19 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 62), { x: 21, y: 19 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 63), { x: 21, y: 20 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "RedSnake", 31, 30, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    combatHostileClearance: 2,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    recoverAfterUnsafeRetreat: async () => { recoveries += 1; },
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
  assert.equal(searchVisits, 2);
  assert.equal(recoveries, 1);
  assert.ok(diagnostics.some(entry => entry.type === "unsafeTargetCluster"));
  assert.ok(diagnostics.some(entry => entry.type === "unsafePackRetreat"));
});

test("accepts a material pack retreat that isolates one follower inside the global clearance", async () => {
  const quest = { questId: 30, stage: "InProgress", objectives: [objective("Collect JadeRing", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(61, "Currish", 100, 100, { disposition: "hostile" }),
    monster(62, "RedSnake", 101, 100, { disposition: "hostile" }),
    monster(63, "TigerSnake", 100, 101, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Collect JadeRing", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searchVisits = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61) {
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    if (options.maxSuccessfulSteps) {
      Object.assign(actor, { x: 26, y: 20 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 61), { x: 26, y: 23 });
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 6 };
    }
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 61), { x: 20, y: 19 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 62), { x: 21, y: 19 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 63), { x: 21, y: 20 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "Currish", 31, 30, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 30,
    objectives: { kill: [], item: [{
      itemName: "JadeRing",
      sources: [{ monsterName: "Currish", spawnCandidates: [spawn("Currish", 30, 30)] }],
    }] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeRetreatSteps: 8,
    unsafeRetreatSafeDistance: 10,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  const retreat = diagnostics.find(entry => entry.type === "unsafePackRetreat");
  assert.ok(retreat);
  assert.equal(retreat.currentExposure.nearest, 3);
  assert.equal(retreat.currentExposure.adjacent, 0);
  assert.equal(retreat.currentExposure.withinThree, 1);
  assert.ok(retreat.currentExposure.weighted < retreat.originExposure.weighted);
});

test("low-health pack retreat requires the full safety radius", () => {
  const origin = { x: 20, y: 20 };
  const player = { x: 26, y: 20, hp: 11, maxHp: 68 };
  const hostiles = [monster(61, "Currish", 26, 23, { disposition: "hostile" })];
  const originExposure = { adjacent: 3, withinThree: 3, weighted: 28, nearest: 1 };
  const common = {
    origin, player, hostiles, safeDistance: 10, minimumRetreatProgress: 6, originExposure,
  };

  assert.equal(unsafeRetreatIsSafe({ ...common, playerHp: 11, playerMaxHp: 68 }), false);
  assert.equal(unsafeRetreatIsSafe({
    ...common,
    playerHp: 11,
    playerMaxHp: 68,
    allowLowHealthFollowerRecovery: true,
  }), false);
  assert.equal(unsafeRetreatIsSafe({
    ...common,
    playerHp: 30,
    playerMaxHp: 68,
    allowLowHealthFollowerRecovery: true,
  }), true);
  assert.equal(unsafeRetreatIsSafe({
    ...common,
    playerHp: 30,
    playerMaxHp: 68,
    allowLowHealthFollowerRecovery: true,
    hostiles: [
      monster(61, "Currish", 26, 23, { disposition: "hostile" }),
      monster(62, "TigerSnake", 30, 20, { disposition: "hostile" }),
    ],
  }), false);
  assert.equal(unsafeRetreatIsSafe({ ...common, playerHp: 60, playerMaxHp: 68 }), true);
  assert.equal(unsafeRetreatIsSafe({
    ...common,
    playerHp: 11,
    playerMaxHp: 68,
    hostiles: [monster(61, "Currish", 40, 20, { disposition: "hostile" })],
  }), true);
});

test("severe packs use the open-cell emergency escape before conservative path retries", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(61, "TigerSnake", 100, 100, { disposition: "hostile" }),
    monster(62, "TigerViper", 101, 100, { disposition: "hostile" }),
    monster(63, "TigerSnake", 100, 101, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searchVisits = 0;
  let conservativeFailures = 0;
  const retreatRanges = [];
  const navigate = async (target, range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61) {
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    if (options.maxSuccessfulSteps && !options.allowedHostileObjectIds) {
      conservativeFailures += 1;
      throw new Error("No walk path with remembered hostile trails");
    }
    if (options.maxSuccessfulSteps && options.allowedHostileObjectIds) {
      retreatRanges.push(range);
      assert.equal(options.maxNoPathRefreshes, 0);
      Object.assign(actor, { x: 30, y: 30 });
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 8 };
    }
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 61), { x: 20, y: 19 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 62), { x: 21, y: 19 });
      Object.assign(client.snapshot.entities.find(entry => entry.objectId === 63), { x: 21, y: 20 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "RedSnake", 31, 30, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(conservativeFailures, 0);
  assert.ok(retreatRanges.length > 0);
  assert.ok(retreatRanges.every(range => range === 0));
  const fallback = diagnostics.find(entry => entry.type === "unsafePackEscapeFallback");
  assert.ok(fallback);
  assert.equal(fallback.ignoredHostileTrails, 3);
  assert.equal(fallback.iterative, true);
  assert.ok(diagnostics.some(entry => entry.type === "unsafePackRetreat" && entry.emergencyFallback));
});

test("a partial cave escape replans when the proven pursuer has left", async () => {
  const quest = { questId: 54, stage: "InProgress", objectives: [objective("Kill Zombie2", 0, 1)] };
  const attackers = [
    monster(61, "CaveMaggot", 100, 100, { disposition: "hostile" }),
    monster(62, "Zombie3", 101, 100, { disposition: "hostile" }),
    monster(63, "Zombie4", 102, 100, { disposition: "hostile" }),
    monster(64, "Zombie1", 103, 100, { disposition: "hostile" }),
  ];
  const client = new FakeClient(snapshot(quest, attackers), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill Zombie2", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searchVisits = 0;
  let retreatMoves = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.allowedHostileObjectIds) {
      retreatMoves += 1;
      if (retreatMoves > 1) throw new Error("No walk path out of the cave corner");
      Object.assign(actor, target);
      for (const hostile of attackers.slice(0, 3)) Object.assign(hostile, { x: 80, y: 80 });
      Object.assign(attackers[3], { x: target.x + 1, y: target.y });
      return { reached: false, successfulSteps: 2 };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      Object.assign(attackers[0], { x: 20, y: 19 });
      Object.assign(attackers[1], { x: 21, y: 20 });
      Object.assign(attackers[2], { x: 19, y: 20 });
      Object.assign(attackers[3], { x: 21, y: 19 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    for (const hostile of attackers) Object.assign(hostile, { x: 80, y: 80 });
    client.snapshot.entities.push(monster(60, "Zombie2", actor.x + 1, actor.y, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: { kill: [{ monsterName: "Zombie2", spawnCandidates: [spawn("Zombie2", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    retreatAtActiveAggressorCount: 1,
    unsafeRetreatSteps: 8,
    unsafeRetreatMaxSteps: 8,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.ok(retreatMoves > 1);
  assert.ok(diagnostics.some(entry =>
    entry.type === "unsafePackPartialRetreat" &&
    entry.reason === "noProvenAggressorAfterPartialEscape"));
});

test("a cave dead end replans after its only proven blocker is defeated", async () => {
  const quest = { questId: 54, stage: "InProgress", objectives: [objective("Kill Zombie2", 0, 1)] };
  const blockers = Array.from({ length: 8 }, (_, index) =>
    monster(61 + index, index === 0 ? "CaveMaggot" : "Zombie3", 100 + index, 100, {
      disposition: "hostile",
      hp: index === 0 ? 3 : 20,
    }));
  const client = new FakeClient(snapshot(quest, blockers), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill Zombie2", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searchVisits = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61 || target.objectId === 60) return { reached: true };
    if (options.allowedHostileObjectIds) throw new Error("No walk path out of the occupied cave corner");
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      const occupied = [
        [20, 19], [21, 19], [21, 20], [21, 21],
        [20, 21], [19, 21], [19, 20], [19, 19],
      ];
      blockers.forEach((blocker, index) => Object.assign(blocker, {
        x: occupied[index][0], y: occupied[index][1],
      }));
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    for (const blocker of blockers) Object.assign(blocker, { x: 80, y: 80 });
    client.snapshot.entities.push(monster(60, "Zombie2", actor.x + 1, actor.y, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 54,
    objectives: { kill: [{ monsterName: "Zombie2", spawnCandidates: [spawn("Zombie2", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    retreatAtActiveAggressorCount: 1,
    unsafeRetreatSteps: 8,
    maxRetreatBreakoutKills: 1,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61, 60]);
  assert.ok(diagnostics.some(entry =>
    entry.type === "unsafePackPartialRetreat" &&
    entry.materialProgress === 0 &&
    entry.reason === "noProvenAggressorAtLocalDeadEnd"));
});

test("a fully occupied retreat cuts down one proven adjacent attacker to open an escape tile", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const blockers = [
    monster(61, "TigerSnake", 100, 100, { disposition: "hostile", hp: 3 }),
    monster(62, "TigerViper", 101, 100, { disposition: "hostile" }),
    monster(63, "TigerSnake", 102, 100, { disposition: "hostile" }),
    monster(64, "TigerViper", 103, 100, { disposition: "hostile" }),
    monster(65, "TigerSnake", 104, 100, { disposition: "hostile" }),
    monster(66, "TigerViper", 105, 100, { disposition: "hostile" }),
    monster(67, "TigerSnake", 106, 100, { disposition: "hostile" }),
    monster(68, "TigerViper", 107, 100, { disposition: "hostile" }),
  ];
  const client = new FakeClient(snapshot(quest, blockers), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searchVisits = 0;
  let retreatMoves = 0;
  let breakerApproaches = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61) {
      assert.equal(stopWhen(), breakerApproaches++ === 0);
      return { reached: false };
    }
    if (target.objectId === 60) {
      assert.equal(stopWhen(), false);
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.allowedHostileObjectIds) {
      retreatMoves += 1;
      Object.assign(actor, target);
      for (const hostile of client.snapshot.entities.filter(entry => entry.kind === "monster" && !entry.dead)) {
        Object.assign(hostile, { x: target.x + 30, y: target.y + 30 });
      }
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 2 };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      const occupied = [
        [20, 19], [21, 19], [21, 20], [21, 21],
        [20, 21], [19, 21], [19, 20], [19, 19],
      ];
      blockers.forEach((blocker, index) => Object.assign(blocker, { x: occupied[index][0], y: occupied[index][1] }));
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "RedSnake", actor.x + 1, actor.y, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeRetreatSteps: 8,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61, 60]);
  assert.equal(retreatMoves, 1);
  assert.equal(diagnostics.filter(entry => entry.type === "unsafePackBreakoutCombat").length, 1);
});

test("a fully occupied retreat waits out paralysis before breakout combat", async () => {
  const quest = { questId: 98, stage: "InProgress", objectives: [objective("Kill WoomaSoldier", 0, 1)] };
  const blockers = Array.from({ length: 8 }, (_, index) =>
    monster(61 + index, index === 0 ? "CaveBat" : "WoomaWarrior", 100 + index, 100, {
      disposition: "hostile",
      hp: index === 0 ? 3 : 20,
    }));
  const objectiveTarget = monster(60, "WoomaSoldier", 30, 30, { disposition: "hostile" });
  const client = new FakeClient(snapshot(quest, blockers), (owner, command) => {
    assert.equal(owner.snapshot.entities[0].poison ?? 0, 0, "breakout attack must wait for poison=0");
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      const target = state.entities.find(entry => entry.objectId === command.objectId);
      Object.assign(target, { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill WoomaSoldier", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  client.snapshot.entities[0].poison = 32;
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searchVisits = 0;
  let blockedSleeps = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.allowedHostileObjectIds) return { reached: false, successfulSteps: 0 };
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      const occupied = [
        [20, 19], [21, 19], [21, 20], [21, 21],
        [20, 21], [19, 21], [19, 20], [19, 19],
      ];
      blockers.forEach((blocker, index) => Object.assign(blocker, { x: occupied[index][0], y: occupied[index][1] }));
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(objectiveTarget);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 98,
    objectives: { kill: [{ monsterName: "WoomaSoldier", spawnCandidates: [spawn("WoomaSoldier")] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeRetreatSteps: 8,
    recoverAfterUnsafeRetreat: async () => {},
    sleep: async () => {
      if (client.snapshot.entities[0].poison === 32) {
        blockedSleeps += 1;
        client.snapshot.entities[0].poison = 0;
      }
    },
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(blockedSleeps, 1);
  assert.ok(diagnostics.some(entry => entry.type === "unsafePackActionControlBlocked"));
  assert.equal(client.sent[0].objectId, 61);
});

test("low-health no-step retreat uses the optional emergency escape before breaking out", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const blockers = Array.from({ length: 8 }, (_, index) =>
    monster(61 + index, index % 2 ? "TigerViper" : "TigerSnake", 100 + index, 100, { disposition: "hostile" }));
  const client = new FakeClient(snapshot(quest, blockers), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  client.snapshot.playerHp = 10;
  client.snapshot.entities[0].hp = 10;
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searchVisits = 0;
  let emergencyCalls = 0;
  let noStepCalls = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.allowedHostileObjectIds) {
      noStepCalls += 1;
      return { reached: false, successfulSteps: 0 };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      // Keep this fixture below the separate dense-pack trigger so it proves
      // the no-authoritative-step fallback specifically.
      const pressure = [[20, 19], [16, 16], [20, 16], [24, 16], [24, 20], [24, 24], [20, 24], [16, 24]];
      blockers.forEach((blocker, index) => Object.assign(blocker, { x: pressure[index][0], y: pressure[index][1] }));
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "RedSnake", 30, 30, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeRetreatSteps: 8,
    emergencyEscapeHpRatio: 0.3,
    emergencyEscape: async (owner) => {
      emergencyCalls += 1;
      Object.assign(owner.snapshot.entities[0], { x: 50, y: 50 });
      return true;
    },
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(emergencyCalls, 1);
  assert.ok(noStepCalls > 0);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
  assert.ok(diagnostics.some(entry => entry.type === "emergencyEscapeSuccess"));
  assert.equal(diagnostics.some(entry => entry.type === "unsafePackBreakoutCombat"), false);
});

test("low-health multi-aggressor pressure uses emergency escape before a temporary walk route", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const attackers = [
    monster(61, "TigerSnake", 20, 19, { disposition: "hostile" }),
    monster(62, "TigerViper", 21, 20, { disposition: "hostile" }),
  ];
  const client = new FakeClient(snapshot(quest, attackers), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  Object.assign(client.snapshot, { playerHp: 20, playerMaxHp: 40 });
  Object.assign(client.snapshot.entities[0], { x: 20, y: 20, hp: 20, maxHp: 40 });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
  client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 62 });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let emergencyCalls = 0;
  let retreatMoves = 0;
  let searchVisits = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.allowedHostileObjectIds) {
      retreatMoves += 1;
      Object.assign(actor, target);
      return { reached: true, successfulSteps: 2 };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "RedSnake", 30, 30, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    emergencyEscapeHpRatio: 0.65,
    emergencyEscape: async owner => {
      emergencyCalls += 1;
      Object.assign(owner.snapshot.entities[0], { x: 50, y: 50 });
      attackers.forEach(entry => Object.assign(entry, { x: 10, y: 10 }));
      return true;
    },
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(emergencyCalls, 1);
  assert.equal(retreatMoves, 0);
  assert.ok(diagnostics.some(entry => entry.type === "emergencyEscapeSuccess" && entry.reason === "criticalPackPressure"));
});

test("a failed low-health emergency escape falls back to bounded hostile breakout", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const blocker = monster(61, "TigerSnake", 100, 100, { disposition: "hostile", hp: 3 });
  const client = new FakeClient(snapshot(quest, [
    blocker,
    ...Array.from({ length: 7 }, (_, index) => monster(62 + index, "TigerViper", 100 + index, 101, { disposition: "hostile" })),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  client.snapshot.playerHp = 10;
  client.snapshot.entities[0].hp = 10;
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searchVisits = 0;
  let emergencyCalls = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61) return { reached: false };
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.allowedHostileObjectIds) {
      Object.assign(actor, target);
      for (const hostile of client.snapshot.entities.filter(entry => entry.kind === "monster" && !entry.dead)) {
        Object.assign(hostile, { x: target.x + 30, y: target.y + 30 });
      }
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 2 };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      const occupied = [[20, 19], [21, 19], [21, 20], [21, 21], [20, 21], [19, 21], [19, 20], [19, 19]];
      client.snapshot.entities.slice(1).forEach((hostile, index) => Object.assign(hostile, { x: occupied[index][0], y: occupied[index][1] }));
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "RedSnake", 30, 30, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeRetreatSteps: 8,
    emergencyEscapeHpRatio: 0.3,
    emergencyEscape: async () => {
      emergencyCalls += 1;
      return false;
    },
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(emergencyCalls, 1);
  assert.ok(diagnostics.some(entry => entry.type === "emergencyEscapeAttempt"));
  assert.ok(diagnostics.some(entry => entry.type === "unsafePackBreakoutCombat"));
  assert.ok(client.sent.some(entry => entry.objectId === 61));
});

test('q89 Taoist blocks a fresh critical travel-breakout cast after sustain creates two proven attackers', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 0, 1)] };
  const breaker = monster(339001, 'HungryZombie', 11, 10, { disposition: 'hostile' });
  const second = monster(339906, 'HungryZombie', 10, 9, { disposition: 'hostile' });
  const client = new FakeClient(snapshot(quest, [breaker, second]));
  Object.assign(client.snapshot, { playerHp: 80, playerMaxHp: 180 });
  Object.assign(client.snapshot.entities[0], { hp: 80, maxHp: 180 });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let casts = 0;
  await assert.rejects(() => clearTravelBlockingMonster(client, breaker, async () => {}, {
    ...settings,
    sustainCadenceMs: 0,
    sustain: async owner => {
      Object.assign(owner.snapshot, { playerHp: 7, playerMaxHp: 180 });
      Object.assign(owner.snapshot.entities[0], { hp: 7, maxHp: 180 });
      owner.receive('ObjectStruck', () => {}, { objectId: 1, attackerId: breaker.objectId });
      owner.receive('ObjectStruck', () => {}, { objectId: 1, attackerId: second.objectId });
    },
    action: () => {
      casts += 1;
      return { kind: 'SoulFireBall' };
    },
    criticalProvenAggressorOffenseGuard: { hpRatio: 0.35, minimumProvenAggressors: 2 },
  }), /unsafe hostile pack/);
  assert.equal(casts, 0);
  assert.ok(diagnostics.some(entry => entry.type === 'criticalProvenAggressorOffenseGuard' &&
    entry.hp === 7 && entry.provenAggressorIds.length === 2));
});

test('q89 Taoist critical overlap uses the held escape before any SoulFireBall', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 0, 1)] };
  const attackers = [
    monster(338505, 'ShiZombie', 20, 19, { disposition: 'hostile' }),
    monster(339001, 'HungryZombie', 21, 20, { disposition: 'hostile' }),
    monster(339906, 'HungryZombie', 20, 21, { disposition: 'hostile' }),
    monster(339106, 'CursedZombie', 19, 20, { disposition: 'hostile' }),
  ];
  const target = monster(340210, 'CursedPriest', 30, 30, { disposition: 'hostile' });
  const client = new FakeClient(snapshot(quest, [...attackers, target]));
  Object.assign(client.snapshot, { playerHp: 7, playerMaxHp: 180 });
  Object.assign(client.snapshot.entities[0], { x: 20, y: 20, hp: 7, maxHp: 180 });
  attackers.forEach(attacker => client.receive('ObjectStruck', () => {}, {
    objectId: 1, attackerId: attacker.objectId,
  }));
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let escapeCalls = 0;
  let casts = 0;
  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: 'CursedPriest', spawnCandidates: [spawn('CursedPriest', 30, 30)] }], item: [] },
  }, async () => {}, {
    ...settings,
    action: () => { casts += 1; return { kind: 'SoulFireBall' }; },
    emergencyEscapeHpRatio: 0.35,
    emergencyEscape: async owner => {
      escapeCalls += 1;
      Object.assign(owner.snapshot.entities[0], { x: 60, y: 60 });
      attackers.forEach(attacker => Object.assign(attacker, { x: 90, y: 90 }));
      return true;
    },
    recoverAfterUnsafeRetreat: async owner => {
      owner.snapshot.questLog[0].stage = 'ReadyToTurnIn';
    },
    criticalProvenAggressorOffenseGuard: { hpRatio: 0.35, minimumProvenAggressors: 2 },
  });
  assert.equal(result.stage, 'ReadyToTurnIn');
  assert.equal(escapeCalls, 1);
  assert.equal(casts, 0);
  assert.ok(diagnostics.some(entry => entry.type === 'emergencyEscapeSuccess' && entry.reason === 'criticalPackPressure'));
});

test('q89 Taoist admits a live Priest with two nonadjacent neighbors and verifies its ordinary death receipt', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 2, 3)] };
  const target = monster(340705, 'CursedPriest', 30, 30, { disposition: 'hostile' });
  const client = new FakeClient(snapshot(quest, [target,
    monster(339001, 'HungryZombie', 32, 30, { disposition: 'hostile' }),
    monster(339106, 'CursedZombie', 30, 32, { disposition: 'hostile' }),
  ]));
  Object.assign(client.snapshot, { playerHp: 180, playerMaxHp: 180 });
  Object.assign(client.snapshot.entities[0], { class: 'Taoist', x: 24, y: 30, hp: 180, maxHp: 180 });
  let casts = 0;
  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: 'CursedPriest', spawnCandidates: [spawn('CursedPriest', 30, 30)] }], item: [] },
  }, navigateClientNear(client), {
    ...settings,
    ...questRetreatProfile(89, 'Taoist'),
    criticalProvenAggressorOffenseGuard: { hpRatio: 0.35, minimumProvenAggressors: 2 },
    action: async owner => {
      casts += 1;
      owner.receive('ObjectDied', state => {
        Object.assign(target, { hp: 0, dead: true });
        state.questLog[0].objectives[0] = objective('Kill CursedPriest', 3, 3);
        state.questLog[0].stage = 'ReadyToTurnIn';
      }, { objectId: target.objectId });
      return { kind: 'magic', spell: 'SoulFireBall', targetId: target.objectId };
    },
  });
  assert.equal(casts, 1);
  assert.equal(result.stage, 'ReadyToTurnIn');
  assert.ok(client.events.some(event => event.packet === 'ObjectDied' && event.payload.objectId === target.objectId));
});

test('q89 Taoist critical blocked pack ends its bounded retreat without breakout casts or recursive recovery', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 0, 1)] };
  const cells = [[20, 19], [21, 19], [21, 20], [21, 21], [20, 21], [19, 21], [19, 20], [19, 19]];
  const attackers = cells.map(([x, y], index) => monster(
    338505 + index,
    index === 0 ? 'ShiZombie' : 'HungryZombie',
    x,
    y,
    { disposition: 'hostile' },
  ));
  const target = monster(340210, 'CursedPriest', 30, 30, { disposition: 'hostile' });
  const client = new FakeClient(snapshot(quest, [...attackers, target]));
  Object.assign(client.snapshot, { playerHp: 7, playerMaxHp: 180 });
  Object.assign(client.snapshot.entities[0], { x: 20, y: 20, hp: 7, maxHp: 180 });
  attackers.slice(0, 4).forEach(attacker => client.receive('ObjectStruck', () => {}, {
    objectId: 1, attackerId: attacker.objectId,
  }));
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let escapeCalls = 0;
  let navigateCalls = 0;
  let casts = 0;
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: 'CursedPriest', spawnCandidates: [spawn('CursedPriest', 30, 30)] }], item: [] },
  }, async () => { navigateCalls += 1; }, {
    ...settings,
    action: () => { casts += 1; return { kind: 'SoulFireBall' }; },
    emergencyEscapeHpRatio: 0.35,
    emergencyEscape: async () => { escapeCalls += 1; return false; },
    criticalProvenAggressorOffenseGuard: { hpRatio: 0.35, minimumProvenAggressors: 2 },
  }), /unsafe hostile pack retreat failed/);
  assert.equal(escapeCalls, 1);
  assert.equal(navigateCalls, 0);
  assert.equal(casts, 0);
  assert.equal(diagnostics.filter(entry => entry.type === 'unsafePackBreakoutCriticalOffenseBlocked').length, 1);
});

test("a multi-kill breakout continues through a second proven attacker before escaping", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const blockers = [
    monster(61, "TigerSnake", 100, 100, { disposition: "hostile", hp: 3 }),
    monster(62, "TigerViper", 101, 100, { disposition: "hostile", hp: 3 }),
    monster(63, "TigerSnake", 102, 100, { disposition: "hostile" }),
    monster(64, "TigerViper", 103, 100, { disposition: "hostile" }),
    monster(65, "TigerSnake", 104, 100, { disposition: "hostile" }),
    monster(66, "TigerViper", 105, 100, { disposition: "hostile" }),
    monster(67, "TigerSnake", 106, 100, { disposition: "hostile" }),
    monster(68, "TigerViper", 107, 100, { disposition: "hostile" }),
    monster(69, "TigerSnake", 108, 100, { disposition: "hostile" }),
  ];
  const client = new FakeClient(snapshot(quest, blockers), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      if (command.objectId === 61) {
        Object.assign(state.entities.find(entry => entry.objectId === 69), { x: 20, y: 19 });
      }
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searchVisits = 0;
  let retreatMoves = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61 || target.objectId === 62) {
      return { reached: false };
    }
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.allowedHostileObjectIds) {
      retreatMoves += 1;
      Object.assign(actor, target);
      for (const hostile of client.snapshot.entities.filter(entry => entry.kind === "monster" && !entry.dead)) {
        Object.assign(hostile, { x: target.x + 30, y: target.y + 30 });
      }
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 2 };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      const occupied = [
        [20, 19], [21, 19], [21, 20], [21, 21],
        [20, 21], [19, 21], [19, 20], [19, 19],
      ];
      blockers.slice(0, 8).forEach((blocker, index) => Object.assign(blocker, { x: occupied[index][0], y: occupied[index][1] }));
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 62 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "RedSnake", actor.x + 1, actor.y, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeRetreatSteps: 8,
    maxRetreatBreakoutKills: 3,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61, 62, 60]);
  assert.equal(retreatMoves, 1);
  assert.deepEqual(
    diagnostics.filter(entry => entry.type === "unsafePackBreakoutCombat").map(entry => entry.breakoutKill),
    [1, 2],
  );
  assert.ok(diagnostics.some(entry => entry.type === "unsafePackRetreat" && entry.emergencyFallback));
});
test("an unreachable breakout attacker is skipped for the next proven blocker", async () => {
  const quest = { questId: 60, stage: "InProgress", objectives: [objective("Kill SpiderFrog", 0, 1)] };
  const blockers = [
    monster(61, "SpiderFrog", 100, 100, { disposition: "hostile", hp: 3 }),
    monster(62, "VioletKekTal0", 101, 100, { disposition: "hostile", hp: 4 }),
    monster(63, "SpiderFrog", 102, 100, { disposition: "hostile" }),
    monster(64, "VioletKekTal0", 103, 100, { disposition: "hostile" }),
    monster(65, "SpiderFrog", 104, 100, { disposition: "hostile" }),
    monster(66, "VioletKekTal0", 105, 100, { disposition: "hostile" }),
    monster(67, "SpiderFrog", 106, 100, { disposition: "hostile" }),
    monster(68, "VioletKekTal0", 107, 100, { disposition: "hostile" }),
  ];
  const client = new FakeClient(snapshot(quest, blockers), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill SpiderFrog", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searchVisits = 0;
  let retreatMoves = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61) {
      throw new Error("No walk path on D2041 to the first breakout attacker");
    }
    if (target.objectId === 62) return { reached: true };
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.allowedHostileObjectIds) {
      retreatMoves += 1;
      Object.assign(actor, target);
      for (const hostile of client.snapshot.entities.filter(entry => entry.kind === "monster" && !entry.dead)) {
        Object.assign(hostile, { x: target.x + 30, y: target.y + 30 });
      }
      assert.equal(stopWhen(), true);
      return { reached: false, successfulSteps: 2 };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      const occupied = [
        [20, 19], [21, 19], [21, 20], [21, 21],
        [20, 21], [19, 21], [19, 20], [19, 19],
      ];
      blockers.forEach((blocker, index) => Object.assign(blocker, { x: occupied[index][0], y: occupied[index][1] }));
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 62 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "SpiderFrog", actor.x + 1, actor.y, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 60,
    objectives: { kill: [{ monsterName: "SpiderFrog", spawnCandidates: [spawn("SpiderFrog", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeRetreatSteps: 8,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(retreatMoves, 1);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [62, 60]);
  assert.ok(diagnostics.some(entry => entry.type === "unsafePackBreakoutTargetUnreachable" && entry.objectId === 61));
  assert.ok(diagnostics.some(entry => entry.type === "unsafePackBreakoutCombat" && entry.objectId === 62));
});

test("iterative pack retreat replans from each live position and sustains during escape", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(61, "TigerSnake", 40, 40, { disposition: "hostile" }),
    monster(62, "TigerViper", 41, 40, { disposition: "hostile" }),
    monster(63, "TigerSnake", 40, 41, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  const retreatTargets = [];
  const sustainPhases = [];
  let searchVisits = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61) {
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.allowedHostileObjectIds) {
      const before = { x: actor.x, y: actor.y };
      retreatTargets.push({ x: target.x, y: target.y });
      assert.ok(distanceForTest(before, target) <= 2);
      Object.assign(actor, target);
      if (retreatTargets.length === 1) {
        const [a, b, c] = client.snapshot.entities.slice(1, 4);
        Object.assign(a, { x: target.x - 1, y: target.y });
        Object.assign(b, { x: target.x - 1, y: target.y - 1 });
        Object.assign(c, { x: target.x, y: target.y - 1 });
        assert.equal(stopWhen(), false);
      } else {
        for (const hostile of client.snapshot.entities.slice(1, 4)) {
          Object.assign(hostile, { x: target.x + 30, y: target.y + 30 });
        }
        assert.equal(stopWhen(), true);
      }
      return { reached: false, successfulSteps: distanceForTest(before, target) };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 10, y: 10 });
      Object.assign(client.snapshot.entities[1], { x: 11, y: 10 });
      Object.assign(client.snapshot.entities[2], { x: 10, y: 11 });
      Object.assign(client.snapshot.entities[3], { x: 11, y: 11 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "RedSnake", actor.x + 1, actor.y, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    sustainCadenceMs: 0,
    sustain: async (_owner, context) => sustainPhases.push(context.phase),
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(retreatTargets.length, 2);
  assert.notDeepEqual(retreatTargets[1], { x: 10, y: 10 });
  assert.ok(distanceForTest(retreatTargets[0], retreatTargets[1]) <= 2);
  assert.ok(sustainPhases.includes("retreat"));
});

test("a pursuing pack that preserves exposure is broken after bounded retreat progress", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(61, "TigerSnake", 40, 40, { disposition: "hostile", hp: 3 }),
    monster(62, "TigerViper", 41, 40, { disposition: "hostile" }),
    monster(63, "TigerSnake", 40, 41, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      const victim = state.entities.find(entry => entry.objectId === command.objectId);
      Object.assign(victim, { hp: 0, dead: true });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let searchVisits = 0;
  let retreatMoves = 0;
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61) return { reached: true };
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.allowedHostileObjectIds) {
      retreatMoves += 1;
      Object.assign(actor, target);
      const live = client.snapshot.entities.filter(entry => entry.kind === "monster" && !entry.dead);
      if (retreatMoves <= 3) {
        live.forEach((hostile, index) => Object.assign(hostile, {
          x: target.x - 1,
          y: target.y + index - 1,
        }));
        assert.equal(stopWhen(), false);
      } else {
        live.forEach(hostile => Object.assign(hostile, { x: target.x + 30, y: target.y + 30 }));
        assert.equal(stopWhen(), true);
      }
      return { reached: false, successfulSteps: 2 };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      Object.assign(client.snapshot.entities[1], { x: 19, y: 20 });
      Object.assign(client.snapshot.entities[2], { x: 19, y: 21 });
      Object.assign(client.snapshot.entities[3], { x: 20, y: 19 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "RedSnake", actor.x + 1, actor.y, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxEngagements: 6,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeRetreatStallSteps: 6,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.ok(retreatMoves >= 4);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [61, 60]);
  assert.ok(diagnostics.some(entry => entry.type === "unsafePackBreakoutCombat" &&
    entry.reason === "retreatExposureStalled" && entry.stalledSteps >= 6));
});

test("moving hostiles can extend retreat across bounded batches until exposure is safe", async () => {
  const quest = { questId: 36, stage: "InProgress", objectives: [objective("Kill RedSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(61, "TigerSnake", 40, 40, { disposition: "hostile" }),
    monster(62, "TigerViper", 41, 40, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { hp: 0, dead: true });
      state.questLog[0].objectives[0] = objective("Kill RedSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  let searchVisits = 0;
  const retreatTargets = [];
  const navigate = async (target, _range, stopWhen, options = {}) => {
    const actor = client.snapshot.entities[0];
    if (target.objectId === 61) return { reached: false };
    if (target.objectId === 60) {
      Object.assign(actor, { x: target.x - 1, y: target.y });
      return { reached: true };
    }
    if (options.allowedHostileObjectIds) {
      retreatTargets.push({ x: target.x, y: target.y });
      Object.assign(actor, target);
      const live = client.snapshot.entities.filter(entry => entry.kind === "monster" && !entry.dead);
      if (retreatTargets.length < 3) {
        live.forEach((hostile, index) => Object.assign(hostile, {
          x: target.x - 1,
          y: target.y + index,
        }));
        assert.equal(stopWhen(), false);
      } else {
        live.forEach(hostile => Object.assign(hostile, { x: target.x + 30, y: target.y + 30 }));
        assert.equal(stopWhen(), true);
      }
      return { reached: false, successfulSteps: 2 };
    }
    searchVisits += 1;
    if (searchVisits === 1) {
      Object.assign(actor, { x: 20, y: 20 });
      Object.assign(client.snapshot.entities[1], { x: 19, y: 20 });
      Object.assign(client.snapshot.entities[2], { x: 19, y: 21 });
      client.receive("ObjectStruck", () => {}, { objectId: 1, attackerId: 61 });
      assert.equal(stopWhen(), true);
      return { reached: false };
    }
    client.snapshot.entities.push(monster(60, "RedSnake", actor.x + 1, actor.y, { disposition: "hostile" }));
    assert.equal(stopWhen(), true);
    return { reached: false };
  };

  const result = await completeQuestObjectives(client, {
    questId: 36,
    objectives: { kill: [{ monsterName: "RedSnake", spawnCandidates: [spawn("RedSnake", 30, 30)] }], item: [] },
  }, navigate, {
    ...settings,
    maxTargetAdjacent: 0,
    maxTargetNearby: 0,
    unsafeRetreatSteps: 2,
    recoverAfterUnsafeRetreat: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(retreatTargets.length, 3);
});

test("search stays in the nearest local field instead of visiting every distant center first", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  const visits = [];
  const navigate = async point => {
    visits.push({ x: point.x, y: point.y });
    Object.assign(client.snapshot.entities[0], { x: point.x, y: point.y });
    if (visits.length === 3) client.snapshot.entities.push(monster(60, "HookingCat", point.x, point.y));
  };
  const candidates = [
    { ...spawn("HookingCat", 12, 10), spread: 16 },
    { ...spawn("HookingCat", 220, 210), spread: 16 },
  ];
  await completeQuestObjectives(client, {
    questId: 6, objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: candidates }], item: [] },
  }, navigate, { ...settings, maxSpawnSearches: 2, maxSpawnWaypoints: 20 });
  assert.deepEqual(visits[0], { x: 12, y: 10 });
  assert.ok(visits.slice(0, 3).every(point => point.x < 100 && point.y < 100));
  assert.equal(visits.length >= 3, true);
  assert.equal(client.sent[0].objectId, 60);
});

test("item search prefers an abundant source field over a nearby rare variant", async () => {
  const quest = { questId: 36, stage: "InProgress", objectives: [objective("Collect SnakeBody", 0, 1)] };
  const client = new FakeClient(snapshot(quest));
  Object.assign(client.snapshot.entities[0], { x: 300, y: 400 });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);

  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 36,
    objectives: { kill: [], item: [{
      itemName: "SnakeBody",
      sources: [
        {
          monsterName: "RedSnake0",
          requiresHarvest: false,
          spawnCandidates: [{ ...spawn("RedSnake0", 300, 400), count: 1, spread: 200, delayMinutes: 40 }],
        },
        {
          monsterName: "RedSnake",
          requiresHarvest: false,
          spawnCandidates: [{ ...spawn("RedSnake", 360, 400), count: 45, spread: 60, delayMinutes: 3 }],
        },
      ],
    }] },
  }, async () => { throw new Error("source selected"); }, settings), /source selected/);

  assert.deepEqual(diagnostics.find(entry => entry.type === "spawnSearchStart")?.targets, ["RedSnake"]);
});

test("a remembered corpse is revisited once without treating it as live, then its candidate is deprioritized", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const corpse = monster(59, "HookingCat", 12, 10, { dead: true, hp: 0 });
  const client = new FakeClient(snapshot(quest, [corpse]), (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  const visits = [];
  const navigate = async point => {
    visits.push({ x: point.x, y: point.y });
    Object.assign(client.snapshot.entities[0], { x: point.x, y: point.y });
    if (point.x === 20 && point.y === 10) client.snapshot.entities.push(monster(60, "HookingCat", 20, 10));
  };
  await completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [
      { ...spawn("HookingCat", 12, 10), spread: 5 },
      { ...spawn("HookingCat", 20, 10), spread: 5 },
    ] }], item: [] },
  }, navigate, { ...settings, maxSpawnSearches: 2 });
  assert.deepEqual(visits[0], { x: 12, y: 10 });
  assert.equal(visits.filter(point => point.x === 12 && point.y === 10).length, 1);
  assert.ok(visits.some(point => point.x === 20 && point.y === 10));
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
});

test("offline monster observations are location hints and require a fresh live AOI target", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type !== "attack") return;
    assert.equal(command.objectId, 60);
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  client.observedMonsterLocations = [
    { monsterName: "HookingCat", mapFileName: "1", x: 12, y: 12 },
    { monsterName: "Deer", mapFileName: "0", x: 13, y: 13 },
    { monsterName: "HookingCat", mapFileName: "0", x: 30, y: 30 },
  ];
  const visits = [];
  const navigate = async point => {
    visits.push({ ...point });
    Object.assign(client.snapshot.entities[0], { x: point.x, y: point.y });
    assert.deepEqual(client.sent, [], "a remembered coordinate alone must not create an attack target");
    if (point.x === 30 && point.y === 30) client.snapshot.entities.push(monster(60, "HookingCat", 30, 30));
  };
  const result = await completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [{ ...spawn("HookingCat", 200, 200), spread: 20 }] }], item: [] },
  }, navigate, settings);
  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(visits[0], { x: 30, y: 30 });
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
});

test("a same-map world refresh preserves monster locations observed earlier in the visit", async () => {
  const quest = { questId: 40, stage: "InProgress", objectives: [objective("Kill OmaFighter", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill OmaFighter", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  client.receive("MapInformation", () => {}, { fileName: "0" });
  client.receive("ObjectMonster", () => {}, {
    name: "OmaFighter",
    location: { x: 30, y: 30 },
  });
  client.events.push({
    sequence: ++client.sequence,
    direction: "received",
    type: "worldSnapshot",
    payload: { mapFileName: "0" },
  });
  const visits = [];
  const navigate = async point => {
    visits.push({ ...point });
    Object.assign(client.snapshot.entities[0], { x: point.x, y: point.y });
    if (point.x === 30 && point.y === 30) {
      client.snapshot.entities.push(monster(60, "OmaFighter", 30, 30));
    }
  };

  const result = await completeQuestObjectives(client, {
    questId: 40,
    objectives: {
      kill: [{ monsterName: "OmaFighter", spawnCandidates: [spawn("OmaFighter", 200, 200)] }],
      item: [],
    },
  }, navigate, settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(visits[0], { x: 30, y: 30 });
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
});

test("a remembered CannibalPlant location gets one close bounded reveal observation", async () => {
  const quest = { questId: 25, stage: "InProgress", objectives: [objective("Collect CannibalStem", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 70), { dead: true, hp: 0 });
    }, { objectId: 70 });
    if (command.type === "harvest") owner.receive("ObjectHarvested", state => {
      state.questLog[0].objectives[0] = objective("Collect CannibalStem", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 70 });
  });
  client.observedMonsterLocations = [
    { monsterName: "CannibalPlant", mapFileName: "0", x: 40, y: 40 },
  ];
  const visits = [];
  const timeline = [];
  let revealed = false;
  const navigate = async (point, range) => {
    visits.push({ point: { x: point.x, y: point.y }, range });
    timeline.push(`navigate:${point.x},${point.y}:${range}`);
    Object.assign(client.snapshot.entities[0], { x: point.x - Math.min(range, 1), y: point.y });
    if (!revealed) assert.deepEqual(client.sent, [], "remembered location must not create an attack target");
  };

  const result = await completeQuestObjectives(client, {
    questId: 25,
    objectives: { kill: [], item: [{ itemName: "CannibalStem", sources: [{
      monsterName: "CannibalPlant", requiresHarvest: true,
      spawnCandidates: [{ ...spawn("CannibalPlant", 200, 200), spread: 20 }],
    }] }] },
  }, navigate, {
    ...settings,
    sleep: async milliseconds => {
      timeline.push(`sleep:${milliseconds}`);
      if (!revealed && milliseconds > 2_000) {
        revealed = true;
        client.snapshot.entities.push(monster(70, "CannibalPlant", 40, 40));
      }
    },
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(visits[0], { point: { x: 40, y: 40 }, range: 2 });
  assert.equal(timeline[1], "sleep:2100");
  assert.deepEqual(client.sent.map(command => command.type), ["attack", "harvest"]);
  assert.deepEqual(client.sent[0], { type: "attack", objectId: 70 });
});

test("nearest-neighbor traversal does not zigzag between distant full-spread regions", async () => {
  const quest = { questId: 25, stage: "InProgress", objectives: [objective("Collect CannibalStem", 0, 1)] };
  const client = new FakeClient(snapshot(quest));
  const visits = [];
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 25,
    objectives: { kill: [], item: [{ itemName: "CannibalStem", sources: [{
      monsterName: "CannibalPlant", requiresHarvest: true,
      spawnCandidates: [
        { ...spawn("CannibalPlant", 100, 100), spread: 40 },
        { ...spawn("CannibalPlant", 500, 500), spread: 40 },
      ],
    }] }] },
  }, async point => visits.push({ ...point }), {
    ...settings, maxSpawnSearches: 2, maxSpawnWaypoints: 12,
  }), /waypoint budget exhausted/);
  assert.equal(visits.length, 12);
  assert.ok(visits.every(point => point.x < 200 && point.y < 200));
  assert.ok(visits.slice(1).every((point, index) => distanceForTest(point, visits[index]) <= 20));
});

test("spawn search obeys one global waypoint budget", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest));
  const visits = [];
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [
      { ...spawn("HookingCat", 12, 10), spread: 48 },
      { ...spawn("HookingCat", 20, 10), spread: 48 },
    ] }], item: [] },
  }, async point => visits.push({ ...point }), { ...settings, maxSpawnSearches: 2, maxSpawnWaypoints: 3 }), /bounded full-spread spawn search/);
  assert.equal(visits.length, 3);
  assert.deepEqual(visits.slice(0, 2), [{ x: 12, y: 10 }, { x: 20, y: 10 }]);
  assert.deepEqual(client.sent, []);
});

test("full-spread exhaustion waits for a bounded respawn and resumes the active objective", async () => {
  const quest = { questId: 40, stage: "InProgress", objectives: [objective("Kill OmaFighter", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 80), { hp: 0, dead: true });
      state.questLog[0].objectives[0] = objective("Kill OmaFighter", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 80 });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let respawnWaits = 0;
  const result = await completeQuestObjectives(client, {
    questId: 40,
    objectives: { kill: [{
      monsterName: "OmaFighter",
      spawnCandidates: [{ ...spawn("OmaFighter", 20, 20), spread: 0 }],
    }], item: [] },
  }, navigateClientNear(client), {
    ...settings,
    maxSpawnRespawnWaits: 1,
    spawnRespawnWaitMs: 123,
    sleep: async milliseconds => {
      if (milliseconds !== 123) return;
      respawnWaits += 1;
      client.snapshot.entities.push(monster(80, "OmaFighter", 20, 20));
    },
    refreshWhileWaiting: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(respawnWaits, 1);
  assert.deepEqual(client.sent, [{ type: "attack", objectId: 80 }]);
  assert.deepEqual(diagnostics.find(entry => entry.type === "spawnRespawnWait"), {
    type: "spawnRespawnWait",
    questId: 40,
    target: "OmaFighter",
    mapFileName: "0",
    wait: 1,
    limit: 1,
    waitMs: 123,
  });
});

test("profiled respawn delay extends the default bounded wait window", async () => {
  const quest = { questId: 60, stage: "InProgress", objectives: [objective("Kill SpiderFrog", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 81), { hp: 0, dead: true });
      state.questLog[0].objectives[0] = objective("Kill SpiderFrog", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 81 });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  let waitedMs = 0;
  let respawnWaits = 0;
  const result = await completeQuestObjectives(client, {
    questId: 60,
    objectives: { kill: [{
      monsterName: "SpiderFrog",
      spawnCandidates: [{ ...spawn("SpiderFrog", 20, 20), spread: 0, delayMinutes: 4 }],
    }], item: [] },
  }, navigateClientNear(client), {
    ...settings,
    spawnRespawnWaitMs: 30_000,
    sleep: async milliseconds => {
      waitedMs += milliseconds;
      const completedWaits = Math.floor(waitedMs / 30_000);
      if (completedWaits <= respawnWaits) return;
      respawnWaits = completedWaits;
      if (respawnWaits === 8) client.snapshot.entities.push(monster(81, "SpiderFrog", 20, 20));
    },
    refreshWhileWaiting: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(respawnWaits, 8);
  assert.deepEqual(client.sent, [{ type: "attack", objectId: 81 }]);
  assert.equal(diagnostics.filter(entry => entry.type === "spawnRespawnWait").at(-1)?.limit, 8);
});

test("a proven adjacent attack interrupts a respawn wait and is cleared before waiting again", async () => {
  const quest = { questId: 98, stage: "InProgress", objectives: [objective("Kill Dung", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      const target = state.entities.find(entry => entry.objectId === command.objectId);
      Object.assign(target, { hp: 0, dead: true });
      if (command.objectId === 82) {
        const actor = state.entities[0];
        state.entities.push(monster(83, "Dung", actor.x + 1, actor.y));
      } else if (command.objectId === 83) {
        state.questLog[0].objectives[0] = objective("Kill Dung", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  let sleepCalls = 0;

  const result = await completeQuestObjectives(client, {
    questId: 98,
    objectives: { kill: [{
      monsterName: "Dung",
      spawnCandidates: [{ ...spawn("Dung", 20, 20), spread: 0 }],
    }], item: [] },
  }, navigateClientNear(client), {
    ...settings,
    maxSpawnRespawnWaits: 1,
    spawnRespawnWaitMs: 30_000,
    sleep: async () => {
      sleepCalls += 1;
      if (sleepCalls !== 1) return;
      const actor = client.snapshot.entities[0];
      client.snapshot.entities.push(monster(82, "WoomaSoldier", actor.x + 1, actor.y));
      client.receive("ObjectStruck", () => {}, {
        objectId: client.snapshot.playerObjectId,
        attackerId: 82,
      });
    },
    refreshWhileWaiting: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [82, 83]);
  assert.ok(sleepCalls < 60);
});

test("full-spread grid covers the observed CannibalPlant offset beyond the old 48-tile ring", async () => {
  const quest = { questId: 25, stage: "InProgress", objectives: [objective("Collect CannibalStem", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 70), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Collect CannibalStem", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  const target = { x: 92, y: 446 };
  const visits = [];
  const navigate = async (point, _range, stopWhen) => {
    visits.push({ x: point.x, y: point.y });
    Object.assign(client.snapshot.entities[0], { x: point.x, y: point.y });
    if (distanceForTest(point, target) <= 14 && !client.snapshot.entities.some(entry => entry.objectId === 70)) {
      client.snapshot.entities.push(monster(70, "CannibalPlant", target.x, target.y));
    }
    stopWhen?.();
  };
  await completeQuestObjectives(client, {
    questId: 25,
    objectives: { kill: [], item: [{ itemName: "CannibalStem", sources: [{
      monsterName: "CannibalPlant", requiresHarvest: true,
      spawnCandidates: [{ ...spawn("CannibalPlant", 130, 510), spread: 70 }],
    }] }] },
  }, navigate, settings);
  assert.ok(visits.some(point => Math.abs(point.x - target.x) <= 18 && Math.abs(point.y - target.y) <= 14));
  assert.equal(client.sent[0].objectId, 70);
});

test("full-spread grid closes RakingCat holes while bare ObjectRevived creates no phantom target", async () => {
  const quest = { questId: 14, stage: "InProgress", objectives: [objective("Kill RakingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 71), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill RakingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  client.events.push({ sequence: ++client.sequence, direction: "received", packet: "ObjectRevived", payload: { objectId: 999 } });
  const target = { x: 304, y: 590 };
  const visits = [];
  const navigate = async point => {
    visits.push({ x: point.x, y: point.y });
    Object.assign(client.snapshot.entities[0], { x: point.x, y: point.y });
    if (Math.abs(point.x - target.x) <= 18 && Math.abs(point.y - target.y) <= 14 && !client.snapshot.entities.some(entry => entry.objectId === 71)) {
      client.snapshot.entities.push(monster(71, "RakingCat", target.x, target.y));
    }
  };
  await completeQuestObjectives(client, {
    questId: 14,
    objectives: { kill: [{ monsterName: "RakingCat", spawnCandidates: [{ ...spawn("RakingCat", 340, 550), spread: 50 }] }], item: [] },
  }, navigate, settings);
  assert.ok(visits.length > 1);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [71]);
});

test("known map bounds clip every full-spread waypoint", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const state = snapshot(quest);
  state.mapWidth = 30;
  state.mapHeight = 25;
  const client = new FakeClient(state);
  const visits = [];
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [{ ...spawn("HookingCat", 5, 5), spread: 50 }] }], item: [] },
  }, async point => visits.push({ ...point }), { ...settings, maxSpawnWaypoints: 100 }), /full-spread spawn search exhausted/);
  assert.ok(visits.length > 0);
  assert.ok(visits.every(point => point.x >= 0 && point.x < 30 && point.y >= 0 && point.y < 25));
});

test("wall-clock search timeout reports partial coverage and emits bounded diagnostics", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest));
  const diagnostics = [];
  client.record = (direction, payload) => diagnostics.push({ direction, ...payload });
  let clock = 0;
  const visits = [];
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [{ ...spawn("HookingCat", 100, 100), spread: 50 }] }], item: [] },
  }, async point => {
    visits.push({ ...point });
    clock += 4;
  }, { ...settings, now: () => clock, spawnSearchTimeoutMs: 10 }), error => {
    assert.match(error.message, /timed out after 12ms/);
    assert.match(error.message, /visited 3\/37 waypoints/);
    assert.match(error.message, /map 0 for HookingCat/);
    assert.doesNotMatch(error.message, /exhausted 37 waypoints/);
    return true;
  });
  assert.equal(visits.length, 3);
  assert.deepEqual(diagnostics, [{
    direction: "diagnostic",
    type: "spawnSearchStart",
    mapFileName: "0",
    targets: ["HookingCat"],
    candidates: 1,
    totalWaypoints: 37,
    waypointBudget: 10_000,
    timeoutMs: 10,
  }]);
});

function distanceForTest(left, right) {
  return Math.max(Math.abs(left.x - right.x), Math.abs(left.y - right.y));
}

test("harvest item source uses the corpse direction and waits for quest progress", async () => {
  const quest = { questId: 4, stage: "InProgress", objectives: [objective("Collect DeerMeat", 0, 1)] };
  let passes = 0;
  const lifecycle = [];
  const client = new FakeClient(snapshot(quest, [monster(40, "Deer", 11, 9)]), (owner, command) => {
    lifecycle.push(command.type);
    if (command.type === "attack") owner.receive("ObjectDied", state => Object.assign(state.entities.find(entry => entry.objectId === 40), { dead: true, hp: 0 }));
    if (command.type === "harvest") {
      passes += 1;
      owner.receive(passes === 2 ? "ObjectHarvested" : "ObjectHarvest", state => {
        if (passes === 2) {
          state.questLog[0].objectives[0] = objective("Collect DeerMeat", 1, 1);
          state.questLog[0].stage = "ReadyToTurnIn";
        }
      });
    }
  });
  let clock = 0;
  const sustainCalls = [];
  const result = await completeQuestObjectives(client, {
    questId: 4,
    objectives: { kill: [], item: [{ itemName: "DeerMeat", sources: [{ monsterName: "Deer", requiresHarvest: true, spawnCandidates: [spawn("Deer")] }] }] },
  }, async () => {}, {
    ...settings,
    harvestCadenceMs: 2_100,
    now: () => clock,
    sleep: async milliseconds => { clock += milliseconds; },
    sustain: async (_client, context) => sustainCalls.push({ ...context, at: clock }),
    afterEngagement: async () => lifecycle.push("afterEngagement"),
  });
  assert.equal(result.harvestPasses, 2);
  assert.deepEqual(client.sent.map(entry => entry.type), ["attack", "harvest", "harvest"]);
  assert.equal(client.sent[1].direction, "UpRight");
  assert.deepEqual(sustainCalls, [
    { phase: "attack", objectId: 40, at: 0 },
    { phase: "harvest", objectId: 40, at: 2_100 },
  ]);
  assert.deepEqual(lifecycle, ["attack", "harvest", "harvest", "afterEngagement"],
    "supply/gold hooks must run only after authoritative harvest progress");
});

test("an already-consumed shared corpse is skipped without claiming progress", async () => {
  const quest = { questId: 25, stage: "InProgress", objectives: [objective("Collect CannibalStem", 0, 1)] };
  let currentTarget = null;
  const diagnostics = [];
  const client = new FakeClient(snapshot(quest, [
    monster(40, "CannibalPlant", 11, 10),
    monster(41, "CannibalPlant", 12, 10),
  ]), (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectDied", state => {
      currentTarget = command.objectId;
      Object.assign(state.entities.find(entry => entry.objectId === currentTarget), { dead: true, hp: 0 });
    }, { objectId: command.objectId });
    if (command.type === "harvest" && currentTarget === 41) {
      owner.receive("ObjectHarvested", state => {
        state.questLog[0].objectives[0] = objective("Collect CannibalStem", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }, { objectId: 41 });
    }
  });
  client.record = (type, payload) => diagnostics.push({ type, payload });

  const result = await completeQuestObjectives(client, {
    questId: 25,
    objectives: { kill: [], item: [{
      itemName: "CannibalStem",
      sources: [{ monsterName: "CannibalPlant", requiresHarvest: true, spawnCandidates: [spawn("CannibalPlant")] }],
    }] },
  }, navigateClientNear(client), settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(
    client.sent.filter(command => command.type === "attack").map(command => command.objectId),
    [40, 41],
  );
  assert.equal(client.sent.filter(command => command.type === "harvest").length, 4);
  assert.deepEqual(diagnostics.filter(entry => entry.payload.type === "unavailableQuestCorpse").map(entry => entry.payload.objectId), [40]);
  assert.equal(quest.objectives[0].current, 1, "only authoritative progress from the second corpse counts");
});

test("three distinct unavailable corpses fail with an explicit bounded error", async () => {
  const quest = { questId: 25, stage: "InProgress", objectives: [objective("Collect CannibalStem", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(40, "CannibalPlant", 11, 10),
    monster(41, "CannibalPlant", 12, 10),
    monster(42, "CannibalPlant", 13, 10),
  ]), (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
    }, { objectId: command.objectId });
  });

  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 25,
    objectives: { kill: [], item: [{
      itemName: "CannibalStem",
      sources: [{ monsterName: "CannibalPlant", requiresHarvest: true, spawnCandidates: [spawn("CannibalPlant")] }],
    }] },
  }, navigateClientNear(client), { ...settings, maxUnavailableCorpses: 3 }),
  /q25 CannibalStem reached unavailable corpse limit 3\/3/);
  assert.deepEqual(
    client.sent.filter(command => command.type === "attack").map(command => command.objectId),
    [40, 41, 42],
  );
  assert.equal(quest.objectives[0].current, 0);
});

test("player death during unavailable-corpse probing still fails immediately", async () => {
  const quest = { questId: 25, stage: "InProgress", objectives: [objective("Collect CannibalStem", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(40, "CannibalPlant", 11, 10)]), (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 40), { dead: true, hp: 0 });
    }, { objectId: 40 });
    if (command.type === "harvest") Object.assign(owner.snapshot.entities[0], { dead: true, hp: 0 });
  });

  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 25,
    objectives: { kill: [], item: [{
      itemName: "CannibalStem",
      sources: [{ monsterName: "CannibalPlant", requiresHarvest: true, spawnCandidates: [spawn("CannibalPlant")] }],
    }] },
  }, navigateClientNear(client), settings), /Player died during quest combat/);
  assert.equal(client.sent.filter(command => command.type === "harvest").length, 1);
});

test("fails closed when an objective has no source on the current map", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest));
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 6, objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [{ ...spawn("HookingCat"), mapFileName: "1" }] }], item: [] },
  }, async () => {}, settings), /no same-map monster source/);
  assert.deepEqual(client.sent, []);
});

test("cross-map fallback skips an unreachable first source and travels only to a reachable map", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  const checks = [];
  const travelled = [];
  const travel = async mapFileName => {
    travelled.push(mapFileName);
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities.push(monster(60, "HookingCat"));
  };
  travel.canReach = async mapFileName => {
    checks.push(mapFileName);
    return mapFileName === "D2042";
  };
  const result = await completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [
      { ...spawn("HookingCat"), mapFileName: "D2044" },
      { ...spawn("HookingCat"), mapFileName: "D2044", respawnIndex: 999 },
      { ...spawn("HookingCat"), mapFileName: "D2042" },
    ] }], item: [] },
  }, async () => {}, { ...settings, travel });
  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(checks, ["D2044", "D2042"]);
  assert.deepEqual(travelled, ["D2042"]);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60]);
});

test("cross-map objective prefers its quest NPC map over a closer dangerous source", async () => {
  const quest = { questId: 33, stage: "InProgress", objectives: [objective("Kill TigerSnake", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      const target = state.entities.find(entry => entry.objectId === 60);
      Object.assign(target, { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill TigerSnake", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });
  const lengths = [];
  const travelled = [];
  const diagnostics = [];
  client.record = (direction, payload) => diagnostics.push({ direction, ...payload });
  const travel = async mapFileName => {
    travelled.push(mapFileName);
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities.push(monster(60, "TigerSnake"));
  };
  travel.routeLength = async mapFileName => {
    lengths.push(mapFileName);
    return ({ "3": 2, "2": 1, "5": 2, HELL00: 0 })[mapFileName] ?? null;
  };

  const result = await completeQuestObjectives(client, {
    questId: 33,
    startNpc: { mapFileName: "2" },
    finishNpc: { mapFileName: "2" },
    objectives: { kill: [{ monsterName: "TigerSnake", spawnCandidates: [
      { ...spawn("TigerSnake"), mapFileName: "3" },
      { ...spawn("TigerSnake"), mapFileName: "2" },
      { ...spawn("TigerSnake"), mapFileName: "2", respawnIndex: 999 },
      { ...spawn("TigerSnake"), mapFileName: "HELL00" },
      { ...spawn("TigerSnake"), mapFileName: "5" },
    ] }], item: [] },
  }, async () => {}, { ...settings, travel });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(lengths, ["3", "2", "HELL00", "5"]);
  assert.deepEqual(travelled, ["2"]);
  assert.deepEqual(diagnostics.filter(entry => entry.type === "chosenObjectiveDestination"), [{
    direction: "diagnostic", type: "chosenObjectiveDestination", questId: 33,
    target: "TigerSnake", fromMapFileName: "0", toMapFileName: "2",
  }]);
});

test("diary material objective prefers its quest group map over a shorter wasteland source", async () => {
  const quest = { questId: 36, stage: "InProgress", objectives: [objective("Collect SnakeBody", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Collect SnakeBody", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });
  const travelled = [];
  const travel = async mapFileName => {
    travelled.push(mapFileName);
    client.snapshot.mapFileName = mapFileName;
    client.snapshot.entities.push(monster(60, "RedSnake"));
  };
  travel.routeLength = async mapFileName => ({ HELL00: 0, "2": 1 })[mapFileName] ?? null;

  const result = await completeQuestObjectives(client, {
    questId: 36,
    group: "SerpentValley",
    objectives: { kill: [], item: [{
      itemName: "SnakeBody",
      sources: [{ monsterName: "RedSnake", requiresHarvest: false, spawnCandidates: [
        { ...spawn("RedSnake"), mapFileName: "HELL00", mapTitle: "WasteLands" },
        { ...spawn("RedSnake"), mapFileName: "2", mapTitle: "SerpentValley" },
      ] }],
    }] },
  }, async () => {}, { ...settings, travel });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(travelled, ["2"]);
});

test("cross-map fallback fails explicitly without travelling when all source maps are unreachable", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest));
  const checks = [];
  const travelled = [];
  const travel = async mapFileName => travelled.push(mapFileName);
  travel.canReach = async mapFileName => {
    checks.push(mapFileName);
    return false;
  };
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [
      { ...spawn("HookingCat"), mapFileName: "D2044" },
      { ...spawn("HookingCat"), mapFileName: "D2044", respawnIndex: 999 },
      { ...spawn("HookingCat"), mapFileName: "D2042" },
      { ...spawn("HookingCat"), mapFileName: "D2040", isBoss: true },
    ] }], item: [] },
  }, async () => {}, { ...settings, travel }), /q6 HookingCat has no reachable monster source among maps D2044, D2042/);
  assert.deepEqual(checks, ["D2044", "D2042"]);
  assert.deepEqual(travelled, []);
  assert.deepEqual(client.sent, []);
});

test("stops immediately when the player is dead", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const state = snapshot(quest, [monster(60, "HookingCat")]);
  state.playerHp = 0;
  state.entities[0].hp = 0;
  const client = new FakeClient(state);
  await assert.rejects(() => completeQuestObjectives(client, { questId: 6, objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] } }, async () => {}, settings), /Player died/);
  assert.deepEqual(client.sent, []);
});

test("bounds a target that never reports combat progress", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(60, "HookingCat")]));
  await assert.rejects(() => completeQuestObjectives(client, { questId: 6, objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] } }, async () => {}, { ...settings, maxAttackAttempts: 10 }), /bounded full-spread spawn search exhausted.*without live HookingCat/);
  assert.equal(client.sent.length, 3);
});

test("a non-responsive revived target is quarantined while another live target completes combat", async () => {
  const quest = { questId: 42, stage: "InProgress", objectives: [objective("Kill RedViper", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "RedViper", 11, 10),
    monster(61, "RedViper", 14, 10),
  ]), (owner, command) => {
    if (command.type !== "attack" || command.objectId !== 61) return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 61), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill RedViper", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 61 });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  const result = await completeQuestObjectives(client, {
    questId: 42,
    objectives: { kill: [{ monsterName: "RedViper", spawnCandidates: [spawn("RedViper")] }], item: [] },
  }, async target => {
    if (target.objectId === 61) {
      Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
    }
  }, { ...settings, maxAttackAttempts: 10 });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(command => command.objectId), [60, 60, 60, 61]);
  assert.ok(diagnostics.some(entry =>
    entry.type === "unresponsiveCombatTarget" && entry.objectId === 60 && entry.attempts === 3));
});

test("a fleeing Deer is re-approached instead of being quarantined as unresponsive", async () => {
  const quest = { questId: 4, stage: "InProgress", objectives: [objective("Collect Venison", 0, 1)] };
  let attacks = 0;
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Deer", 11, 10, { hp: 20, maxHp: 20 }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    attacks += 1;
    if (attacks <= 4) {
      owner.receive("ObjectWalk", state => {
        state.entities.find(entry => entry.objectId === 60).x += 1;
      }, { objectId: 60, direction: "Right" });
      return;
    }
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Collect Venison", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);

  const result = await completeQuestObjectives(client, {
    questId: 4,
    objectives: { kill: [{ monsterName: "Deer", spawnCandidates: [spawn("Deer")] }], item: [] },
  }, navigateClientNear(client), { ...settings, maxAttackAttempts: 10 });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(client.sent.length, 5);
  assert.equal(diagnostics.filter(entry => entry.type === "combatTargetMovedBeforeHit").length, 4);
  assert.equal(diagnostics.some(entry => entry.type === "unresponsiveCombatTarget"), false);
});

test("a same-speed passive target is bounded after prolonged movement without a confirmed hit", async () => {
  const quest = { questId: 4, stage: "InProgress", objectives: [objective("Collect Venison", 0, 1)] };
  let clock = 0;
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Deer", 11, 10, { hp: 20, maxHp: 20 }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    clock += 10_000;
    owner.receive("ObjectWalk", state => {
      state.entities.find(entry => entry.objectId === 60).x += 1;
    }, { objectId: 60, direction: "Right" });
  });

  await assert.rejects(
    clearTravelBlockingMonster(client, client.snapshot.entities[1], navigateClientNear(client), {
      ...settings,
      maxAttackAttempts: 10,
      maxMovingTargetNoProgressMs: 20_000,
      now: () => clock,
    }),
    /made no authoritative combat progress after 2 attacks/,
  );
});

test("travel blocker combat clears a nearer live monster sealing the target approach", async () => {
  const quest = { questId: 54, stage: "InProgress", objectives: [objective("Kill Zombie1", 2, 5)] };
  const target = monster(60, "Zombie3", 41, 118, { hp: 20, maxHp: 20 });
  const blocker = monster(61, "Zombie3", 41, 117, { hp: 20, maxHp: 20 });
  const client = new FakeClient(snapshot(quest, [target, blocker]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), {
        dead: true,
        hp: 0,
      });
    }, { objectId: command.objectId });
  });
  Object.assign(client.snapshot.entities[0], { x: 43, y: 114 });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  const navigate = async current => {
    if (Number(current.objectId) === 60 &&
        client.snapshot.entities.some(entry => entry.objectId === 61 && !entry.dead)) {
      throw new Error("No walk path from 43,114 to 41,118");
    }
    const actor = client.snapshot.entities.find(entry => entry.objectId === 1);
    Object.assign(actor, { x: Number(current.x) + 1, y: Number(current.y) });
  };

  const result = await clearTravelBlockingMonster(client, target, navigate, {
    ...settings,
    maxApproachBlockerClears: 2,
  });

  assert.equal(result.cleared, true);
  assert.deepEqual(result.approachBlockers, [61]);
  assert.deepEqual(client.sent.map(command => command.objectId), [61, 60]);
  assert.ok(diagnostics.some(entry =>
    entry.type === "travelApproachBlockerCombat" &&
    entry.objectId === 60 && entry.blockerObjectId === 61));
});

test("continues ordinary attacks across HP acknowledgements until target death", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  let hits = 0;
  const client = new FakeClient(snapshot(quest, [monster(60, "HookingCat", 11, 10, { hp: 9, maxHp: 9 })]), (owner, command) => {
    if (command.type !== "attack") return;
    hits += 1;
    owner.receive(hits === 3 ? "ObjectDied" : "ObjectAttack", state => {
      const target = state.entities.find(entry => entry.objectId === 60);
      target.hp = Math.max(0, target.hp - 3);
      if (target.hp === 0) {
        target.dead = true;
        state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    });
  });
  let clock = 0;
  const sustainCalls = [];
  await completeQuestObjectives(client, { questId: 6, objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] } }, async () => {}, {
    ...settings,
    attackCadenceMs: 2_000,
    now: () => clock,
    sleep: async milliseconds => { clock += milliseconds; },
    sustain: async (_client, context) => sustainCalls.push({ ...context, at: clock }),
  });
  assert.equal(hits, 3);
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60, 60, 60]);
  assert.deepEqual(sustainCalls, [
    { phase: "attack", objectId: 60, at: 0 },
    { phase: "attack", objectId: 60, at: 2_000 },
    { phase: "attack", objectId: 60, at: 4_000 },
  ]);
});

test("dynamic approach uses Wizard spell range, closes without MP, and waits without an attack", async () => {
  const routeQuest = {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] },
  };

  const rangedQuest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const ranged = new FakeClient(snapshot(rangedQuest, [monster(60, "HookingCat", 16, 10)]), (owner, command) => {
    if (command.type !== "magic") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { hp: 0, dead: true });
      state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });
  const rangedDistances = [];
  const rangedRangeChecks = [];
  await completeQuestObjectives(ranged, routeQuest, async (_target, range) => rangedDistances.push(range), {
    ...settings,
    approachRange: owner => {
      rangedRangeChecks.push(owner.snapshot.playerMp);
      return 6;
    },
    action: async (owner, target) => {
      owner.send({ type: "magic", targetId: target.objectId });
      return { kind: "magic", targetId: target.objectId };
    },
  });
  assert.deepEqual(rangedDistances, [6]);
  assert.equal(rangedRangeChecks.length, 2, "range is refreshed after the supply hook");
  assert.deepEqual(ranged.sent, [{ type: "magic", targetId: 60 }]);

  const meleeQuest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const melee = new FakeClient(snapshot(meleeQuest, [monster(61, "HookingCat", 16, 10)]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 61), { hp: 0, dead: true });
      state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 61 });
  });
  const meleeDistances = [];
  const meleeRangeChecks = [];
  await completeQuestObjectives(melee, routeQuest, async (target, range) => {
    meleeDistances.push(range);
    Object.assign(melee.snapshot.entities[0], { x: target.x - range, y: target.y });
  }, {
    ...settings,
    approachRange: () => {
      meleeRangeChecks.push(1);
      return 1;
    },
    action: async (owner, target) => {
      owner.send({ type: "attack", objectId: target.objectId });
      return { kind: "attack", targetId: target.objectId };
    },
  });
  assert.deepEqual(meleeDistances, [1]);
  assert.equal(meleeRangeChecks.length, 2, "the no-MP melee range is also refreshed after supplies");
  assert.deepEqual(melee.sent, [{ type: "attack", objectId: 61 }]);

  const waitingQuest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const waiting = new FakeClient(snapshot(waitingQuest, [monster(62, "HookingCat", 16, 10)]), (owner, command) => {
    if (command.type !== "magic") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 62), { hp: 0, dead: true });
      state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 62 });
  });
  let actionCalls = 0;
  const slept = [];
  await completeQuestObjectives(waiting, routeQuest, async () => {}, {
    ...settings,
    attackResponseTimeout: 1_000,
    approachRange: () => 6,
    sleep: async milliseconds => slept.push(milliseconds),
    action: async (owner, target) => {
      actionCalls += 1;
      if (actionCalls === 1) return { kind: "wait", targetId: target.objectId, delayMs: 650 };
      owner.send({ type: "magic", targetId: target.objectId });
      return { kind: "magic", targetId: target.objectId };
    },
  });
  assert.equal(actionCalls, 2);
  assert.deepEqual(waiting.sent, [{ type: "magic", targetId: 62 }]);
  assert.ok(slept.includes(650));
});

test("combat approach retries with bounded clearance when the full buffer seals the path", async () => {
  const quest = { questId: 30, stage: "InProgress", objectives: [objective("Kill Currish", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Currish", 15, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill Currish", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });
  const clearances = [];
  const navigate = async (target, _range, _stopWhen, options = {}) => {
    clearances.push(options.hostileAvoidanceRadius);
    if (options.hostileAvoidanceRadius === 3) {
      throw new Error("No walk path on 2 from 10,10 to 15,10");
    }
    Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 30,
    objectives: { kill: [{ monsterName: "Currish", spawnCandidates: [spawn("Currish", 15, 10)] }], item: [] },
  }, navigate, {
    ...settings,
    combatHostileClearance: 3,
    combatHostileClearanceFallback: 1,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(clearances, [3, 1]);
  assert.deepEqual(client.sent.map(command => command.objectId), [60]);
});

test("an unreachable live monster is quarantined while combat continues with another target", async () => {
  const quest = { questId: 40, stage: "InProgress", objectives: [objective("Kill OmaFighter", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "OmaFighter", 15, 10, { disposition: "hostile" }),
    monster(61, "OmaFighter", 20, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill OmaFighter", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  const navigationTargets = [];
  const navigate = async target => {
    navigationTargets.push(target.objectId ?? null);
    if (target.objectId === 60) throw new Error("No walk path on 1 from 10,10 to 15,10");
    Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 40,
    objectives: { kill: [{ monsterName: "OmaFighter", spawnCandidates: [spawn("OmaFighter", 15, 10)] }], item: [] },
  }, navigate, settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(navigationTargets, [60, 61]);
  assert.deepEqual(client.sent.map(command => command.objectId), [61]);
  assert.ok(diagnostics.some(entry =>
    entry.type === "unreachableCombatTarget" && entry.objectId === 60 &&
    entry.location.x === 15 && entry.location.y === 10));
});

test("a stationary target is retried after its temporary corridor blocker can move", async () => {
  const quest = { questId: 60, stage: "InProgress", objectives: [objective("Kill SpiderFrog", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "SpiderFrog", 20, 10, { disposition: "hostile" }),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill SpiderFrog", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: command.objectId });
  });
  let clock = 0;
  let targetApproaches = 0;
  const navigate = async target => {
    if (target.objectId === 60) {
      targetApproaches += 1;
      if (targetApproaches === 1) throw new Error("No walk path on D2041 from 10,10 to 20,10");
      Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
    }
  };

  const result = await completeQuestObjectives(client, {
    questId: 60,
    objectives: { kill: [{
      monsterName: "SpiderFrog",
      spawnCandidates: [{ ...spawn("SpiderFrog", 20, 10), spread: 0 }],
    }], item: [] },
  }, navigate, {
    ...settings,
    now: () => clock,
    unreachableTargetRetryMs: 30_000,
    spawnRespawnWaitMs: 30_000,
    sleep: async milliseconds => { clock += milliseconds; },
    refreshWhileWaiting: async () => {},
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(targetApproaches, 2);
  assert.deepEqual(client.sent, [{ type: "attack", objectId: 60 }]);
});

test("an opted-in objective clears one nearby monster occupying its only approach", async () => {
  const quest = { questId: 60, stage: "InProgress", objectives: [objective("Kill SpiderFrog", 0, 1)] };
  const target = monster(60, "SpiderFrog", 24, 10, { disposition: "hostile" });
  const blocker = monster(61, "KekTal", 13, 10, { disposition: "hostile" });
  const client = new FakeClient(snapshot(quest, [target, blocker]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === command.objectId), {
        dead: true,
        hp: 0,
      });
      if (command.objectId === 60) {
        state.questLog[0].objectives[0] = objective("Kill SpiderFrog", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: command.objectId });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  const navigate = async current => {
    const liveBlocker = client.snapshot.entities.some(entry => entry.objectId === 61 && !entry.dead);
    if (current.objectId === 60 && liveBlocker) {
      throw new Error("No walk path on D2041 from 10,10 to 24,10");
    }
    Object.assign(client.snapshot.entities[0], { x: current.x - 1, y: current.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 60,
    objectives: { kill: [{ monsterName: "SpiderFrog", spawnCandidates: [spawn("SpiderFrog", 24, 10)] }], item: [] },
  }, navigate, {
    ...settings,
    maxApproachBlockerClears: 1,
    approachBlockerSearchRadius: 6,
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(command => command.objectId), [61, 60]);
  assert.ok(diagnostics.some(entry =>
    entry.type === "objectiveApproachBlockerCombat" &&
    entry.objectId === 60 && entry.blockerObjectId === 61));
});

test("spawn search retries the same cave waypoint with bounded hostile clearance", async () => {
  const quest = { questId: 49, stage: "InProgress", objectives: [objective("Kill Skeleton", 0, 1)] };
  const client = new FakeClient(snapshot(quest), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 63), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill Skeleton", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 63 });
  });
  const clearances = [];
  const diagnostics = [];
  client.record = (direction, payload) => diagnostics.push({ direction, ...payload });
  const navigate = async (target, _range, _stopWhen, options = {}) => {
    clearances.push({
      clearance: options.hostileAvoidanceRadius,
      protected: options.hostileAvoidanceByName,
      refreshes: options.maxNoPathRefreshes,
    });
    if (target.objectId != null) {
      Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
      return;
    }
    if (options.hostileAvoidanceRadius === 4) {
      throw new NavigationStalled({
        reason: "position-cycle",
        mapId: "D001",
        position: { x: 164, y: 346 },
        target,
        bestDistance: 24,
        successfulSteps: 48,
      });
    }
    Object.assign(client.snapshot.entities[0], { x: target.x - 4, y: target.y });
    client.snapshot.entities.push(monster(63, "Skeleton", target.x, target.y, { disposition: "hostile" }));
  };

  const result = await completeQuestObjectives(client, {
    questId: 49,
    objectives: { kill: [{ monsterName: "Skeleton", spawnCandidates: [spawn("Skeleton", 212, 332)] }], item: [] },
  }, navigate, {
    ...settings,
    spawnSearchHostileClearance: 4,
    spawnSearchHostileClearanceFallback: 1,
    spawnSearchProtectedHostileClearance: { CursedShaman: 6, CursedShaman0: 6 },
  });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(clearances.slice(0, 2), [
    { clearance: 4, protected: { CursedShaman: 6, CursedShaman0: 6 }, refreshes: 0 },
    { clearance: 1, protected: { CursedShaman: 6, CursedShaman0: 6 }, refreshes: 0 },
  ]);
  assert.ok(diagnostics.some(entry =>
    entry.type === "spawnSearchClearanceFallback" && entry.from === 4 && entry.to === 1 &&
    entry.reason === "NAVIGATION_STALLED"));
});

test("cooldown waits preserve the attack budget and refresh from authoritative snapshots", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(60, "HookingCat", 16, 10)]), (owner, command) => {
    if (command.type !== "magic") return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { hp: 0, dead: true });
      state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });
  let cooldown = 2;
  let actionCalls = 0;
  const result = await completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] },
  }, async () => {}, {
    ...settings,
    maxAttackAttempts: 1,
    attackResponseTimeout: 1_000,
    approachRange: () => 6,
    action: async (owner, target) => {
      actionCalls += 1;
      if (cooldown > 0) return { kind: "wait", targetId: target.objectId, delayMs: 650 };
      owner.send({ type: "magic", targetId: target.objectId });
      return { kind: "magic", targetId: target.objectId };
    },
    refreshWhileWaiting: async owner => {
      owner.send({ type: "clientVersion" });
      cooldown = 0;
      owner.events.push({
        sequence: ++owner.sequence,
        direction: "received",
        type: "worldSnapshot",
        payload: owner.snapshot,
      });
    },
  });
  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(actionCalls, 3);
  assert.deepEqual(client.sent, [
    { type: "clientVersion" },
    { type: "magic", targetId: 60 },
  ]);
  assert.ok(client.events.some(event => event.type === "worldSnapshot"));
});

test("positive hits reset consecutive cooldown time during a long spell pull", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(60, "HookingCat", 16, 10)]), (owner, command) => {
    if (command.type !== "magic") return;
    owner.receive("ObjectHealth", state => {
      const target = state.entities.find(entry => entry.objectId === 60);
      target.hp -= 4;
      if (target.hp === 0) {
        target.dead = true;
        state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
        state.questLog[0].stage = "ReadyToTurnIn";
      }
    }, { objectId: 60 });
  });
  let waitsRemaining = 2;
  let waited = 0;
  const result = await completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] },
  }, async () => {}, {
    ...settings,
    maxAttackAttempts: 5,
    maxCooldownWaitMs: 2_000,
    attackResponseTimeout: 1_000,
    approachRange: () => 6,
    sleep: async ms => { waited += ms; },
    action: async (owner, target) => {
      if (waitsRemaining-- > 0) return { kind: "wait", targetId: target.objectId, delayMs: 650 };
      owner.send({ type: "magic", targetId: target.objectId });
      waitsRemaining = 2;
      return { kind: "magic", targetId: target.objectId };
    },
  });
  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(client.sent.filter(command => command.type === "magic").length, 5);
  assert.equal(waited, 6_500);
});

test("a server cooldown that never changes fails after a bounded total wait", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(60, "HookingCat", 16, 10)]));
  let waited = 0;
  let refreshes = 0;
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] },
  }, async () => {}, {
    ...settings,
    maxAttackAttempts: 1,
    maxCooldownWaitMs: 2_000,
    cooldownRefreshIntervalMs: 1_000,
    attackResponseTimeout: 1_000,
    approachRange: () => 6,
    sleep: async milliseconds => { waited += milliseconds; },
    action: async (_owner, target) => ({ kind: "wait", targetId: target.objectId, delayMs: 650 }),
    refreshWhileWaiting: async owner => {
      refreshes += 1;
      owner.events.push({
        sequence: ++owner.sequence,
        direction: "received",
        type: "worldSnapshot",
        payload: owner.snapshot,
      });
    },
  }), /cooldown stayed unavailable for 2000ms without an attack/);
  assert.equal(waited, 2_000);
  assert.equal(refreshes, 2);
  assert.deepEqual(client.sent, []);
});

test("a sustain callback cannot mask authoritative player death", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(60, "HookingCat")]));
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] },
  }, async () => {}, {
    ...settings,
    sustain: async owner => {
      owner.snapshot.playerHp = 0;
      owner.snapshot.entities[0].hp = 0;
      owner.snapshot.entities[0].dead = true;
    },
  }), /Player died during quest combat/);
  assert.deepEqual(client.sent, []);
});

test("a target that leaves during approach is deferred and another live target completes the objective", async () => {
  const quest = { questId: 30, stage: "InProgress", objectives: [objective("Collect JadeRing", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "Currish", 14, 10),
    monster(61, "Currish", 18, 10),
  ]), (owner, command) => {
    if (command.type !== "attack" || command.objectId !== 61) return;
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 61), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Collect JadeRing", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 61 });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  const navigate = async target => {
    if (target.objectId === 60) {
      client.snapshot.entities = client.snapshot.entities.filter(entry => entry.objectId !== 60);
      return;
    }
    Object.assign(client.snapshot.entities[0], { x: target.x - 1, y: target.y });
  };

  const result = await completeQuestObjectives(client, {
    questId: 30,
    objectives: { kill: [{ monsterName: "Currish", spawnCandidates: [spawn("Currish")] }], item: [] },
  }, navigate, settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent, [{ type: "attack", objectId: 61 }]);
  assert.ok(diagnostics.some(entry => entry.type === "lostCombatTarget" && entry.objectId === 60));
});

test("target removal waits three seconds so a delayed authoritative player death wins the error classification", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(60, "HookingCat")] ), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectRemove", state => {
      state.entities = state.entities.filter(entry => entry.objectId !== 60);
    }, { objectId: 60 });
  });
  const ordinaryWait = client.wait.bind(client);
  let removalSettleTimeout;
  client.wait = async (predicate, label, timeout) => {
    if (label === "target 60 removal settle") {
      removalSettleTimeout = timeout;
      assert.ok(timeout >= 1_800, "the live death arrived about 1.8 seconds after ObjectRemove");
      client.snapshot.playerHp = 0;
      Object.assign(client.snapshot.entities[0], { hp: 0, dead: true });
      return predicate();
    }
    return ordinaryWait(predicate, label, timeout);
  };
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 6, objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] },
  }, async () => {}, settings), /Player died during quest combat/);
  assert.equal(removalSettleTimeout, 3_000);
});

test("a living player defers an exact removed target but still fails without quest progress", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(60, "HookingCat")]), (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectRemove", state => {
      state.entities = state.entities.filter(entry => entry.objectId !== 60);
    }, { objectId: 60 });
  });
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 6, objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] },
  }, async () => {}, settings), /without live HookingCat/);
  assert.ok(diagnostics.some(entry => entry.type === "lostCombatTarget" && entry.objectId === 60));
});

test("authoritative quest progress accepts a target removed before its death packet settles", async () => {
  const quest = { questId: 62, stage: "InProgress", objectives: [objective("Kill VioletKekTal", 0, 2)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "VioletKekTal"), monster(61, "VioletKekTal", 12, 10),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    if (command.objectId === 60) {
      owner.receive("ObjectRemove", state => {
        state.entities = state.entities.filter(entry => entry.objectId !== 60);
        state.questLog[0].objectives[0] = objective("Kill VioletKekTal", 1, 2);
      }, { objectId: 60 });
      return;
    }
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 61), { hp: 0, dead: true });
      state.questLog[0].objectives[0] = objective("Kill VioletKekTal", 2, 2);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 61 });
  });

  const result = await completeQuestObjectives(client, {
    questId: 62,
    objectives: { kill: [{ monsterName: "VioletKekTal", spawnCandidates: [spawn("VioletKekTal")] }], item: [] },
  }, navigateClientNear(client), settings);

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60, 61]);
});

test("proven remote-player last blow is recorded and retries without claiming quest credit", async () => {
  const quest = { questId: 22, stage: "InProgress", objectives: [objective("Kill ForestYeti", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [
    monster(60, "ForestYeti", 11, 10), monster(61, "ForestYeti", 12, 10),
    { objectId: 99, kind: "player", name: "RemoteWizard", x: 12, y: 11, hp: 30, dead: false },
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    if (command.objectId === 60) {
      owner.receive("ObjectStruck", state => Object.assign(state.entities.find(entry => entry.objectId === 60), { hp: 0 }), { objectId: 60, attackerId: 99 });
      owner.receive("ObjectHealth", () => {}, { objectId: 60, percent: 0 });
      owner.receive("ObjectDied", state => Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true }), { objectId: 60 });
      return;
    }
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 61), { dead: true, hp: 0 });
      state.questLog[0].objectives[0] = objective("Kill ForestYeti", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 61 });
  });
  const diagnostics = [];
  client.record = (direction, payload) => diagnostics.push({ direction, ...payload });
  const result = await completeQuestObjectives(client, {
    questId: 22, objectives: { kill: [{ monsterName: "ForestYeti", spawnCandidates: [spawn("ForestYeti")] }], item: [] },
  }, navigateClientNear(client), settings);
  assert.equal(result.stage, "ReadyToTurnIn");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60, 61]);
  assert.deepEqual(diagnostics, [{
    direction: "diagnostic", type: "contestedQuestTarget", questId: 22,
    target: "ForestYeti", objectId: 60, attackerId: 99,
  }]);
});

test("unknown last-blow identity preserves the missing-award failure", async () => {
  const quest = { questId: 22, stage: "InProgress", objectives: [objective("Kill ForestYeti", 0, 1)] };
  const client = new FakeClient(snapshot(quest, [monster(60, "ForestYeti")]), (owner, command) => {
    if (command.type !== "attack") return;
    owner.receive("ObjectStruck", state => Object.assign(state.entities.find(entry => entry.objectId === 60), { hp: 0 }), { objectId: 60, attackerId: 777 });
    owner.receive("ObjectDied", state => Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true }), { objectId: 60 });
  });
  await assert.rejects(() => completeQuestObjectives(client, {
    questId: 22, objectives: { kill: [{ monsterName: "ForestYeti", spawnCandidates: [spawn("ForestYeti")] }], item: [] },
  }, async () => {}, settings), /Fake timeout waiting for q22 ForestYeti kill progress/);
});

test("a bystander death is not accepted as the current target death", async () => {
  const quest = { questId: 6, stage: "InProgress", objectives: [objective("Kill HookingCat", 0, 1)] };
  let attacks = 0;
  const client = new FakeClient(snapshot(quest, [
    monster(60, "HookingCat", 11, 10, { hp: undefined, healthPercent: 100 }),
    monster(61, "Deer", 12, 10),
  ]), (owner, command) => {
    if (command.type !== "attack") return;
    attacks += 1;
    if (attacks === 1) {
      owner.receive("ObjectDied", state => {
        Object.assign(state.entities.find(entry => entry.objectId === 61), { dead: true, hp: 0 });
      }, { objectId: 61 });
      return;
    }
    if (attacks < 4) {
      owner.receive("ObjectHealth", state => {
        state.entities.find(entry => entry.objectId === 60).healthPercent -= 30;
      }, { objectId: 60, percent: 100 - (attacks - 1) * 30, expire: 3 });
      return;
    }
    owner.receive("ObjectDied", state => {
      Object.assign(state.entities.find(entry => entry.objectId === 60), { dead: true, hp: 0, healthPercent: 0 });
      state.questLog[0].objectives[0] = objective("Kill HookingCat", 1, 1);
      state.questLog[0].stage = "ReadyToTurnIn";
    }, { objectId: 60 });
  });

  const result = await completeQuestObjectives(client, {
    questId: 6,
    objectives: { kill: [{ monsterName: "HookingCat", spawnCandidates: [spawn("HookingCat")] }], item: [] },
  }, async () => {}, { ...settings, maxAttackAttempts: 6 });

  assert.equal(result.stage, "ReadyToTurnIn");
  assert.equal(attacks, 4, "the unrelated ObjectDied must not end the target engagement");
  assert.deepEqual(client.sent.map(entry => entry.objectId), [60, 60, 60, 60]);
});

test("picks up a visible non-harvest quest drop and requires objective confirmation", async () => {
  const quest = { questId: 2, stage: "InProgress", objectives: [objective("Collect GingerTea", 0, 1)] };
  const state = snapshot(quest, [monster(20, "Scarecrow")]);
  const client = new FakeClient(state, (owner, command) => {
    if (command.type === "attack") owner.receive("ObjectDied", current => {
      Object.assign(current.entities.find(entry => entry.objectId === 20), { hp: 0, dead: true });
      current.groundDrops.push({ objectId: 80, kind: "item", name: "GingerTea", x: 11, y: 10 });
    });
    if (command.type === "pickUp") owner.receive("NewQuestInfo", current => {
      current.groundDrops = [];
      current.questLog[0].objectives[0] = objective("Collect GingerTea", 1, 1);
      current.questLog[0].stage = "ReadyToTurnIn";
    });
  });
  await completeQuestObjectives(client, {
    questId: 2,
    objectives: { kill: [], item: [{ itemName: "GingerTea", sources: [{ monsterName: "Scarecrow", requiresHarvest: false, spawnCandidates: [spawn("Scarecrow")] }] }] },
  }, async () => {}, settings);
  assert.deepEqual(client.sent.map(entry => entry.type), ["attack", "pickUp"]);
  assert.equal(client.sent[1].objectId, 80);
});

test("same-tile harvest preserves a valid authoritative facing", () => {
  assert.equal(harvestDirection({ x: 4, y: 4, direction: "Left" }, { x: 4, y: 4 }), "Left");
});


test('q89 ordinary Priest approach maps a fresh named Shaman halo to bounded recovery', async () => {
  const quest = { questId: 89, stage: 'InProgress', objectives: [objective('Kill CursedPriest', 0, 1)] };
  const owner = self({ kind: 'selfPlayer', x: 10, y: 10 });
  const priest = monster(89, 'CursedPriest', 18, 10, { disposition: 'hostile' });
  const shaman = monster(90, 'CursedShaman', 30, 10, { disposition: 'hostile' });
  const client = new FakeClient({
    playerObjectId: 1, playerHp: 40, playerMaxHp: 40, mapFileName: '0',
    entities: [owner, priest, shaman], groundDrops: [], questLog: [quest],
  });
  let actions = 0;
  let recoveries = 0;
  const navigate = async (_target, _distance, _stopWhen, options) => {
    assert.equal(typeof options.beforeMovement, 'function');
    Object.assign(shaman, { x: 12, y: 10 });
    const hazard = options.beforeMovement({
      mapId: '0', from: { x: 10, y: 10 },
      physicalCells: [{ x: 11, y: 10 }, { x: 12, y: 10 }], movementType: 'run',
    });
    assert.ok(hazard);
    throw new NavigationStepGuarded(hazard);
  };

  const result = await completeQuestObjectives(client, {
    questId: 89,
    objectives: { kill: [{ monsterName: 'CursedPriest', spawnCandidates: [spawn('CursedPriest')] }], item: [] },
  }, navigate, {
    ...settings,
    approachRange: () => 9,
    maxEngagements: 2,
    action: async () => { actions += 1; return { kind: 'magic' }; },
    spawnStallProtectedBlocker: {
      monsterNames: ['CursedShaman', 'CursedShaman0'],
      minimumApproachDistance: 7, maximumApproachDistance: 9, clearance: 6, maxBlockers: 2,
    },
    recoverAfterUnsafeRetreat: async current => {
      recoveries += 1;
      current.snapshot.questLog[0].stage = 'ReadyToTurnIn';
    },
  });

  assert.equal(result.stage, 'ReadyToTurnIn');
  assert.equal(actions, 0);
  assert.equal(recoveries, 1);
});
