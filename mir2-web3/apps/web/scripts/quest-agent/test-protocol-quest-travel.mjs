import assert from 'node:assert/strict';
import test from 'node:test';
import { SABUK_QUEST_TRAVEL, travelToQuestNpc } from './protocol-quest-travel.mjs';

function clientAt(mapFileName, x, y) {
  return {
    snapshot: {
      mapFileName,
      playerObjectId: 1,
      entities: [{ objectId: 1, kind: 'selfPlayer', x, y }],
    },
  };
}

const q110 = {
  questId: 110,
  startNpc: { mapFileName: '2' },
  finishNpc: { mapFileName: '3' },
};

test('q110 finish uses the D701 walking-transfer detour into the Sabuk merchant quarter', async () => {
  const client = clientAt('2', 505, 483);
  const calls = [];
  const travel = async mapFileName => {
    calls.push(['travel', mapFileName]);
    client.snapshot.mapFileName = mapFileName;
    if (mapFileName === 'D701') Object.assign(client.snapshot.entities[0], { x: 28, y: 22 });
    if (mapFileName === '3') Object.assign(client.snapshot.entities[0], { x: 660, y: 276 });
  };
  const navigate = async (target, desiredDistance) => {
    calls.push(['navigate', target, desiredDistance]);
    Object.assign(client.snapshot.entities[0], target);
  };

  const result = await travelToQuestNpc(client, q110, 'finish', { travel, navigate });

  assert.equal(result.status, 'sabukSecretGate');
  assert.deepEqual(calls, [
    ['travel', 'D701'],
    ['navigate', SABUK_QUEST_TRAVEL.innerExit, 1],
    ['travel', '3'],
  ]);
});

test('q110 start and unrelated quest endpoints keep direct topology travel', async () => {
  const client = clientAt('0', 288, 616);
  const calls = [];
  const travel = async mapFileName => {
    calls.push(mapFileName);
    client.snapshot.mapFileName = mapFileName;
  };
  const navigate = async () => assert.fail('direct travel must not navigate a fixed portal');

  assert.equal((await travelToQuestNpc(client, q110, 'start', { travel, navigate })).status, 'direct');
  assert.equal((await travelToQuestNpc(client, {
    questId: 109,
    finishNpc: { mapFileName: '3' },
  }, 'finish', { travel, navigate })).status, 'direct');
  assert.deepEqual(calls, ['2', '3']);
});

test('q111 and q112 stay inside the Sabuk merchant quarter once the detour landed', async () => {
  for (const questId of [111, 112]) {
    const client = clientAt('3', 660, 276);
    const calls = [];
    const result = await travelToQuestNpc(client, {
      questId,
      startNpc: { mapFileName: '3' },
      finishNpc: { mapFileName: '3' },
    }, 'start', {
      travel: async map => calls.push(map),
      navigate: async () => assert.fail('inside-gate route must not leave the quarter'),
    });
    assert.equal(result.status, 'direct');
    assert.deepEqual(calls, ['3']);
  }
});

test('Sabuk detour fails closed when the authoritative landing is outside the wall', async () => {
  const client = clientAt('2', 505, 483);
  await assert.rejects(() => travelToQuestNpc(client, q110, 'finish', {
    travel: async mapFileName => {
      client.snapshot.mapFileName = mapFileName;
      Object.assign(client.snapshot.entities[0], mapFileName === 'D701'
        ? { x: 171, y: 132 }
        : { x: 516, y: 778 });
    },
    navigate: async target => Object.assign(client.snapshot.entities[0], target),
  }), /did not enter the Sabuk merchant quarter/);
});
