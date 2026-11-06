import test from 'node:test';
import assert from 'node:assert/strict';
import worker from '../index.ts';

const MOCK_SECRET = 'not-a-real-credential-test-fixture';

for (const key of [
  'mir2/native/windows-invited/releases/protected.exe',
  '  ///mir2/native/windows-invited/channels/invited.json  ',
]) {
  test(`legacy PUT rejects reserved native prefix: ${key}`, async () => {
    const writes = [];
    const response = await worker.fetch(new Request(
      `https://assets.mir2.obelisk.build/upload?key=${encodeURIComponent(key)}`, {
        method: 'PUT', headers: {authorization: `Bearer ${MOCK_SECRET}`}, body: 'bad',
      }), {MIR2_R2_UPLOAD_SECRET: MOCK_SECRET,
        MIR2_ASSETS: {put: async (...args) => {writes.push(args); return {};}}});
    assert.equal(response.status, 403);
    assert.deepEqual(writes, []);
  });
}

test('unrelated legacy Web PUT retains its existing behavior', async () => {
  const writes = [];
  const response = await worker.fetch(new Request(
    'https://assets.mir2.obelisk.build/upload?key=mir2/v/web-fixture/a.txt', {
      method: 'PUT', headers: {authorization: `Bearer ${MOCK_SECRET}`, 'content-type': 'text/plain'}, body: 'ok',
    }), {MIR2_R2_UPLOAD_SECRET: MOCK_SECRET,
      MIR2_ASSETS: {put: async (...args) => {writes.push(args); return {};}}});
  assert.equal(response.status, 200);
  assert.equal(writes.length, 1);
  assert.equal(writes[0][0], 'mir2/v/web-fixture/a.txt');
  assert.equal(writes[0][2].onlyIf, undefined);
});
