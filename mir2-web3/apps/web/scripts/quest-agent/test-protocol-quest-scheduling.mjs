import test from 'node:test';
import assert from 'node:assert/strict';
import {
  cofarmFollowers,
  shouldDeferCofarmQuest,
} from './protocol-quest-scheduling.mjs';

test('q124 defers its CleanSkull hunt while q122 and q123 remain in the route', () => {
  assert.equal(shouldDeferCofarmQuest(124, 'InProgress', [122, 123]), true);
  assert.deepEqual(cofarmFollowers(124, [122, 123]), [122, 123]);
});

test('q124 does not defer after its shared hunting quests have passed', () => {
  assert.equal(shouldDeferCofarmQuest(124, 'InProgress', []), false);
  assert.equal(shouldDeferCofarmQuest(124, 'ReadyToTurnIn', [122, 123]), false);
});

test('ordinary quests are never deferred by the co-farm scheduler', () => {
  assert.equal(shouldDeferCofarmQuest(122, 'InProgress', [123]), false);
  assert.deepEqual(cofarmFollowers(122, [123]), []);
});
