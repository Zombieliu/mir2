import assert from 'node:assert/strict';
import test from 'node:test';
import { applyProtocolObservation, observedPlayerHp, observedEntityHealthRatio } from './protocol-observation.mjs';

const packet = (name, payload = {}) => ({ type: 'packet', packet: name, payload });

function observedWorld() {
  return {
    mapFileName: '0',
    mapTitle: 'Bichon Province',
    playerObjectId: 1001,
    playerHp: 224,
    playerMaxHp: 224,
    entities: [
      { objectId: 1001, kind: 'player', name: 'Runner', x: 288, y: 616, direction: 'Down', hp: 224, maxHp: 224, dead: false },
      { objectId: 2001, kind: 'monster', name: 'Deer', x: 290, y: 616, direction: 'Left', hp: 137, maxHp: 224, dead: false },
    ],
    groundDrops: [{ objectId: 3001 }],
    activeNpcDialog: { npcObjectId: 24 },
  };
}

test('worldSnapshot replaces rather than merges the observed cache', () => {
  const old = observedWorld();
  const authoritative = { playerObjectId: 7, entities: [{ objectId: 7, x: 1, y: 2 }] };
  const observed = applyProtocolObservation(old, { type: 'worldSnapshot', payload: authoritative });
  assert.deepEqual(observed, { ...authoritative, mapSnapshotPending: false });
  assert.notEqual(observed, authoritative);
  assert.notEqual(observed.entities[0], authoritative.entities[0]);
  observed.entities[0].x = 99;
  assert.equal(authoritative.entities[0].x, 1);
  assert.equal(old.playerObjectId, 1001);
});

test('real movement packet fields update self and remote entities', () => {
  const state = observedWorld();
  applyProtocolObservation(state, packet('UserLocation', { x: 289, y: 615, direction: 'UpRight' }));
  applyProtocolObservation(state, packet('ObjectWalk', { objectId: 2001, x: 291, y: 615, direction: 'Right' }));
  applyProtocolObservation(state, packet('ObjectPushed', { objectId: 2001, location: { x: 292, y: 615 }, direction: 'Right' }));
  assert.deepEqual(
    state.entities.map(({ objectId, x, y, direction }) => ({ objectId, x, y, direction })),
    [
      { objectId: 1001, x: 289, y: 615, direction: 'UpRight' },
      { objectId: 2001, x: 292, y: 615, direction: 'Right' },
    ],
  );
});

test('ObjectMonster and NewMonsterInfo use their location payload to upsert monsters', () => {
  const state = observedWorld();
  const message = packet('ObjectMonster', {
    objectId: 2002, name: 'HookingCat', location: { x: 294, y: 610 }, image: 6,
    direction: 'DownLeft', ai: 0, dead: false, hidden: false, buffs: ['server'],
  });
  applyProtocolObservation(state, message);
  state.entities.find(entity => entity.objectId === 2002).buffs.push('observed-only');
  assert.deepEqual(message.payload.buffs, ['server']);
  applyProtocolObservation(state, packet('NewMonsterInfo', {
    objectId: 2002, name: 'HookingCat', location: { x: 295, y: 611 }, image: 6,
    direction: 'Down', ai: 0, dead: false, hidden: false,
  }));
  const monster = state.entities.find(entity => entity.objectId === 2002);
  assert.equal(state.entities.filter(entity => entity.objectId === 2002).length, 1);
  assert.deepEqual(
    { kind: monster.kind, name: monster.name, x: monster.x, y: monster.y, direction: monster.direction, ai: monster.ai },
    { kind: 'monster', name: 'HookingCat', x: 295, y: 611, direction: 'Down', ai: 0 },
  );
  assert.equal('location' in monster, false);
});

test('incremental packets retain Crystal neutral disposition for passive and guard AI families', () => {
  const state = observedWorld();
  for (const [objectId, name, ai] of [
    [2101, 'Hen', 1],
    [2102, 'Deer', 2],
    [2103, 'ChestnutTree', 3],
    [2104, 'Royal_Guard', 6],
    [2105, 'Trainer', 56],
    [2106, 'Royal_Archer', 57],
    [2107, 'Guard1', 58],
  ]) {
    applyProtocolObservation(state, packet('ObjectMonster', {
      objectId, name, ai, location: { x: objectId - 2000, y: 1 }, dead: false,
    }));
  }
  applyProtocolObservation(state, packet('ObjectMonster', {
    objectId: 2200, name: 'CannibalPlant', ai: 5, location: { x: 200, y: 1 }, dead: false,
  }));
  applyProtocolObservation(state, packet('ObjectMonster', {
    objectId: 2201, name: 'HostileOverride', ai: 6, disposition: 'hostile',
    location: { x: 201, y: 1 }, dead: false,
  }));

  for (const objectId of [2101, 2102, 2103, 2104, 2105, 2106, 2107]) {
    assert.equal(state.entities.find(entity => entity.objectId === objectId)?.disposition, 'neutral');
  }
  assert.equal(state.entities.find(entity => entity.objectId === 2200)?.disposition, undefined);
  assert.equal(state.entities.find(entity => entity.objectId === 2201)?.disposition, 'hostile');

  applyProtocolObservation(state, packet('ObjectMonster', {
    objectId: 2201, name: 'HostileOverride', ai: 6,
    location: { x: 202, y: 1 }, dead: false,
  }));
  assert.equal(
    state.entities.find(entity => entity.objectId === 2201)?.disposition,
    'hostile',
    'a later relationship-less packet must not weaken an explicit authoritative relation',
  );
});

test('ObjectHealth keeps percent separate from absolute hp', () => {
  const state = observedWorld();
  applyProtocolObservation(state, packet('ObjectHealth', { objectId: 2001, percent: 47, expire: 3_000 }));
  const deer = state.entities.find(entity => entity.objectId === 2001);
  assert.equal(deer.hp, 137);
  assert.equal(deer.maxHp, 224);
  assert.equal(deer.healthPercent, 47);
  assert.equal(deer.healthExpire, 3_000);
  assert.equal(observedEntityHealthRatio(deer), 0.47);
  applyProtocolObservation(state, packet('ObjectHealth', { objectId: 2001, percent: 19 }));
  assert.equal(observedEntityHealthRatio(deer), 0.19, 'fresh wounds supersede cached absolute target HP');
  assert.equal(deer.hp, 137);
});

test('fresh self percentages drive control without altering exact HP evidence', () => {
  let state = observedWorld();
  state.playerHp = 65;
  state.playerMaxHp = 100;
  state.entities[0].hp = 65;
  for (const percent of [69, 73, 77, 81, 85, 89, 93, 97, 100]) {
    applyProtocolObservation(state, packet('ObjectHealth', { objectId: 1001, percent, expire: 0 }));
    assert.equal(observedPlayerHp(state), percent);
  }
  assert.equal(state.playerHp, 65, 'raw exact snapshot remains available for evidence');
  assert.equal(state.entities[0].hp, 65);
  applyProtocolObservation(state, packet('HealthChanged', { hp: 74, mp: 37 }));
  assert.equal(observedPlayerHp(state), 74, 'a newer exact packet supersedes the percentage');
  applyProtocolObservation(state, packet('ObjectHealth', { objectId: 1001, percent: 20 }));
  assert.equal(observedPlayerHp(state), 20, 'new damage also supersedes exact HP');
  state = applyProtocolObservation(state, { type: 'worldSnapshot', payload: observedWorld() });
  assert.equal(observedPlayerHp(state), 224, 'a new full snapshot replaces the observation marker');
});

test('percentage HP estimates stay conservative and do not revive a dead self', () => {
  const state = observedWorld();
  applyProtocolObservation(state, packet('ObjectHealth', { objectId: 1001, percent: 47 }));
  assert.equal(observedPlayerHp(state), 105);
  applyProtocolObservation(state, packet('Death'));
  applyProtocolObservation(state, packet('ObjectHealth', { objectId: 1001, percent: 100 }));
  assert.equal(observedPlayerHp(state), 0, 'late health cannot override Death');
  applyProtocolObservation(state, packet('Revived'));
  assert.equal(observedPlayerHp(state), 0, 'revival alone carries no health replacement');
  applyProtocolObservation(state, packet('ObjectHealth', { objectId: 1001, percent: 100 }));
  assert.equal(observedPlayerHp(state), 224);
  assert.equal(state.playerHp, undefined, 'percentage estimate is kept separate from exact HP');
});

test('poison packets keep the current self movement-control mask observable', () => {
  const state = observedWorld();
  applyProtocolObservation(state, packet('ObjectPoisoned', { objectId: 1001, poison: 32 }));
  assert.equal(state.playerPoison, 32);
  assert.equal(state.entities[0].poison, 32);
  applyProtocolObservation(state, packet('Poisoned', { poison: 0 }));
  assert.equal(state.playerPoison, 0);
  assert.equal(state.entities[0].poison, 0);
});

test('death retains a harvestable corpse and revive restores its lifecycle flag', () => {
  const state = observedWorld();
  applyProtocolObservation(state, packet('ObjectDied', {
    objectId: 2001, location: { x: 291, y: 616 }, direction: 'Down', kind: 0,
  }));
  const corpse = state.entities.find(entity => entity.objectId === 2001);
  assert.ok(corpse);
  assert.equal(corpse.dead, true);
  assert.equal(corpse.hp, 0);
  assert.deepEqual({ x: corpse.x, y: corpse.y }, { x: 291, y: 616 });

  applyProtocolObservation(state, packet('ObjectRevived', { objectId: 2001, effect: 2 }));
  assert.equal(corpse.dead, false);
  assert.equal(corpse.effect, 2);
  assert.equal(corpse.hp, undefined, 'revive clears the stale death HP until an exact value arrives');
  applyProtocolObservation(state, packet('ObjectHealth', { objectId: 2001, percent: 100, expire: 0 }));
  assert.equal(corpse.healthPercent, 100);
  assert.equal(corpse.hp, undefined, 'a percentage packet must not masquerade as absolute HP');
});

test('remove and teleport-out retire entities from the visible observation', () => {
  const state = observedWorld();
  state.entities.push({ objectId: 2002, kind: 'monster', name: 'Oma', x: 1, y: 1 });
  applyProtocolObservation(state, packet('ObjectRemove', { objectId: 2001 }));
  applyProtocolObservation(state, packet('ObjectTeleportOut', { objectId: 2002, effectType: 1 }));
  assert.deepEqual(state.entities.map(entity => entity.objectId), [1001]);
});

test('MapChanged clears old-map observations while retaining the relocated self', () => {
  const state = observedWorld();
  applyProtocolObservation(state, packet('MapChanged', {
    mapIndex: 1,
    fileName: '0122',
    title: 'Mongchon Province',
    location: { x: 330, y: 270 },
    direction: 'Up',
  }));
  assert.equal(state.mapFileName, '0122');
  assert.equal(state.mapTitle, 'Mongchon Province');
  assert.equal(state.mapSnapshotPending, true);
  assert.deepEqual(state.mapTransfers, []);
  assert.deepEqual(state.entities.map(entity => entity.objectId), [1001]);
  assert.deepEqual({ x: state.entities[0].x, y: state.entities[0].y, direction: state.entities[0].direction }, { x: 330, y: 270, direction: 'Up' });
  assert.deepEqual(state.groundDrops, []);
  assert.equal(state.activeNpcDialog, null);
});

test('actual MapInformation clears stale AOI and waits for subsequent UserLocation landing', () => {
  const state = observedWorld();
  state.mapTransfers = [{ key: 'old-transfer', toMapFileName: '0115' }];
  applyProtocolObservation(state, packet('MapInformation', {
    bigMapIndex: 0,
    fileName: '0',
    lights: 2,
    mapDarkLight: 0,
    mapIndex: 33,
    miniMapIndex: 0,
    music: 0,
    spawnFlags: { fire: false, lightning: false },
    title: 'BichonProvince',
    weatherParticles: 0,
  }));

  assert.deepEqual(
    { fileName: state.mapFileName, title: state.mapTitle, mapIndex: state.mapIndex, pending: state.mapSnapshotPending },
    { fileName: '0', title: 'BichonProvince', mapIndex: 33, pending: true },
  );
  assert.deepEqual(state.entities.map(entity => entity.objectId), [1001]);
  assert.equal(state.entities[0].x, undefined, 'old-map x must not become the new-map landing');
  assert.equal(state.entities[0].y, undefined, 'old-map y must not become the new-map landing');
  assert.deepEqual(state.mapTransfers, []);
  assert.deepEqual(state.groundDrops, []);
  assert.equal(state.activeNpcDialog, null);

  applyProtocolObservation(state, packet('UserLocation', { direction: 'Down', x: 315, y: 476 }));
  assert.deepEqual(
    { x: state.entities[0].x, y: state.entities[0].y, direction: state.entities[0].direction },
    { x: 315, y: 476, direction: 'Down' },
  );
});

test('visibility and self death packets update observed lifecycle without inventing hp', () => {
  const state = observedWorld();
  applyProtocolObservation(state, packet('ObjectHide', { objectId: 2001 }));
  assert.equal(state.entities[1].hidden, true);
  applyProtocolObservation(state, packet('ObjectHidden', { objectId: 2001, hidden: false }));
  assert.equal(state.entities[1].hidden, false);
  applyProtocolObservation(state, packet('Death', { location: { x: 287, y: 615 }, direction: 'Left' }));
  assert.equal(state.entities[0].dead, true);
  assert.equal(state.playerHp, 0);
  applyProtocolObservation(state, packet('Revived'));
  assert.equal(state.entities[0].dead, false);
  assert.equal(state.playerHp, undefined, 'Revived clears stale death HP but carries no replacement value');
  assert.equal(state.entities[0].hp, undefined);
});

test('HealthChanged restores exact self vitals after Revived', () => {
  const state = observedWorld();
  applyProtocolObservation(state, packet('Death', { location: { x: 287, y: 615 }, direction: 'Left' }));
  applyProtocolObservation(state, packet('Revived'));
  applyProtocolObservation(state, packet('ObjectHealth', { objectId: 1001, percent: 100, expire: 0 }));
  assert.equal(state.playerHp, undefined);
  assert.equal(state.entities[0].hp, undefined);

  applyProtocolObservation(state, packet('HealthChanged', { hp: 112, mp: 37, typed: true }));
  assert.equal(state.playerHp, 112);
  assert.equal(state.playerMp, 37);
  assert.equal(state.playerHealthPercent, 50);
  assert.deepEqual(
    { hp: state.entities[0].hp, dead: state.entities[0].dead, healthPercent: state.entities[0].healthPercent },
    { hp: 112, dead: false, healthPercent: 50 },
  );
});

test('UserInformation restores exact self and maximum vitals from its public fields', () => {
  const state = observedWorld();
  applyProtocolObservation(state, packet('Death', { location: { x: 287, y: 615 }, direction: 'Left' }));
  applyProtocolObservation(state, packet('Revived'));
  applyProtocolObservation(state, packet('UserInformation', {
    objectId: 1001, name: 'Runner', location: { x: 288, y: 616 }, direction: 'Down',
    hp: 180, mp: 40, maxHp: 240, maxMp: 80, level: 8,
  }));
  assert.deepEqual(
    { hp: state.playerHp, mp: state.playerMp, maxHp: state.playerMaxHp, maxMp: state.playerMaxMp },
    { hp: 180, mp: 40, maxHp: 240, maxMp: 80 },
  );
  assert.deepEqual(
    { hp: state.entities[0].hp, maxHp: state.entities[0].maxHp, dead: state.entities[0].dead, x: state.entities[0].x },
    { hp: 180, maxHp: 240, dead: false, x: 288 },
  );
});
