import test from 'node:test';
import assert from 'node:assert/strict';
import {
  questCombatAction,
  questCombatApproachRange,
  shouldPreserveTaoistObjectiveAmmo,
} from './protocol-quest-combat-policy.mjs';

const routeQuest = {
  questId: 98,
  objectives: {
    kill: [{ monsterName: 'Dung' }, { monsterName: 'WoomaSoldier' }],
    item: [],
  },
};

function client() {
  const sent = [];
  return {
    sent,
    snapshot: {
      playerObjectId: 1,
      playerHp: 100,
      playerMaxHp: 100,
      playerMp: 100,
      playerMaxMp: 100,
      entities: [
        { kind: 'player', self: true, objectId: 1, class: 'Taoist', x: 10, y: 10 },
        { kind: 'monster', objectId: 2, name: 'CaveBat', x: 11, y: 10, hp: 20, dead: false },
      ],
      knownSkills: [{ spell: 'SoulFireBall', cooldownRemainingTicks: 0, mpCost: 5 }],
      equipmentItems: [{ name: 'Amulet', itemIndex: 712, slot: 'amulet', count: 12, quantity: 12 }],
    },
    send(command) { sent.push(command); },
  };
}

test('Taoist Wooma expedition melees a non-objective blocker without spending an Amulet', async () => {
  const owner = client();
  const target = owner.snapshot.entities[1];
  assert.equal(shouldPreserveTaoistObjectiveAmmo(owner, routeQuest, target), true);
  assert.equal(questCombatApproachRange(owner, routeQuest, target), 1);
  assert.deepEqual(await questCombatAction(owner, routeQuest, target), {
    kind: 'attack', targetId: 2, command: { type: 'attack', objectId: 2 },
  });
  assert.deepEqual(owner.sent, [{ type: 'attack', objectId: 2 }]);
});

test('Taoist Wooma expedition still casts at an objective monster', () => {
  const owner = client();
  const target = { kind: 'monster', objectId: 3, name: 'WoomaSoldier', x: 14, y: 10, hp: 285, dead: false };
  owner.snapshot.entities.push(target);
  assert.equal(shouldPreserveTaoistObjectiveAmmo(owner, routeQuest, target), false);
  assert.equal(questCombatApproachRange(owner, routeQuest, target), 6);
});

test('q99 keeps the same melee blocker rule while preserving ranged objective combat', async () => {
  const owner = client();
  const q99 = {
    questId: 99,
    objectives: {
      kill: [{ monsterName: 'WoomaFighter' }],
      item: [],
    },
  };
  const blocker = { kind: 'monster', objectId: 4, name: 'CaveBat', x: 11, y: 10, hp: 20, dead: false };
  assert.equal(shouldPreserveTaoistObjectiveAmmo(owner, q99, blocker), true);
  assert.equal(questCombatApproachRange(owner, q99, blocker), 1);
  assert.deepEqual(await questCombatAction(owner, q99, blocker), {
    kind: 'attack', targetId: 4, command: { type: 'attack', objectId: 4 },
  });

  const objective = { kind: 'monster', objectId: 5, name: 'WoomaFighter', x: 14, y: 10, hp: 285, dead: false };
  assert.equal(shouldPreserveTaoistObjectiveAmmo(owner, q99, objective), false);
  assert.equal(questCombatApproachRange(owner, q99, objective), 6);
});
