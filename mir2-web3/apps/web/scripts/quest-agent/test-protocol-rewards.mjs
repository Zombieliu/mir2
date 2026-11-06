import assert from 'node:assert/strict';
import test from 'node:test';

import {
  expectedItemRewards,
  itemCount,
  verifyItemRewards,
} from './protocol-rewards.mjs';

const item = (itemIndex, quantity, extra = {}) => ({
  key: `crystal-item-${itemIndex}`,
  name: extra.name ?? `Item ${itemIndex}`,
  quantity,
  ...extra,
});

test('expected rewards retain exact itemIndex and omit zero-count or unselected choices', () => {
  const quest = {
    rewards: {
      fixedItems: [
        { itemIndex: 101, itemName: 'Fixed Token', count: 2 },
        { itemIndex: 102, itemName: 'Zero Placeholder', count: 0 },
      ],
      selectableItems: [
        { selectionIndex: 0, itemIndex: 201, itemName: 'Chosen Blade', count: 1 },
        { selectionIndex: 1, itemIndex: 202, itemName: 'Other Blade', count: 1 },
        { selectionIndex: 0, itemIndex: 203, itemName: 'Zero Choice', count: 0 },
      ],
    },
  };

  assert.deepEqual(
    expectedItemRewards(quest, 0).map(({ itemIndex, count }) => ({ itemIndex, count })),
    [{ itemIndex: 101, count: 2 }, { itemIndex: 201, count: 1 }],
  );
});

test('item reward verification uses itemIndex rather than a matching display name', () => {
  const before = { inventoryItems: [item(700, 1, { name: 'Training Sword' })] };
  const after = {
    inventoryItems: [
      item(700, 2, { name: 'Training Sword' }),
      item(701, 1, {
        key: 'localized-key-without-index',
        name: 'Training Sword',
        tooltipSource: { info: { item_index: 701 } },
      }),
    ],
  };

  assert.equal(itemCount(after, 700), 2);
  assert.equal(itemCount(after, 701), 1);
  assert.deepEqual(
    verifyItemRewards(before, after, [{ itemIndex: 701, itemName: 'Training Sword', count: 1 }]),
    [{ itemIndex: 701, name: 'Training Sword', expected: 1, before: 0, after: 1 }],
  );
});

test('missing item quantity rejects reward verification', () => {
  const before = { inventoryItems: [item(810, 2)] };
  const after = { inventoryItems: [item(810, 2)] };
  assert.throws(
    () => verifyItemRewards(before, after, [{ itemIndex: 810, itemName: 'Quest Parcel', count: 1 }]),
    /Reward missing: Quest Parcel, expected \+1, observed 0/,
  );
});

test('duplicate rewards for one itemIndex require their combined quantity across containers', () => {
  const before = {
    inventoryItems: [item(920, 1)],
    beltItems: [item(920, 2)],
    equipmentItems: [],
  };
  const after = {
    inventoryItems: [item(920, 3)],
    beltItems: [item(920, 2)],
    equipmentItems: [item(920, 1)],
  };
  const verified = verifyItemRewards(before, after, [
    { itemIndex: 920, itemName: 'Stacked Reward', count: 1 },
    { itemIndex: 920, itemName: 'Stacked Reward', count: 2 },
  ]);

  assert.deepEqual(verified, [
    { itemIndex: 920, name: 'Stacked Reward', expected: 3, before: 3, after: 6 },
  ]);
});
