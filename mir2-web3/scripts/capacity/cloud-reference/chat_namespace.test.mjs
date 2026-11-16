import test from 'node:test';
import assert from 'node:assert/strict';
import { sharedChatIdentity, exactProbeChat } from './public_signed_probe.mjs';
const speaker = { label: 'SyntheticA', snapshot: { playerObjectId: 1000, entities: [{ name: 'SyntheticA', kind: 'selfPlayer', objectId: 1000 }] } };
const peer = id => ({ snapshot: { entities: [{ name: 'SyntheticA', kind: 'player', objectId: id }] } });
const nonce = 'cg0123456789';

test('owner and peers match ObjectChat by the same peer-observed shared ID, never owner snapshot1000', () => {
  const shared = sharedChatIdentity([speaker, peer(50050), peer(50050)], speaker);
  assert.equal(shared, 50050);
  for (const payload of [{ objectId: 50050, text: 'SyntheticA : ' + nonce }, { objectId: 50050, message: nonce }])
    assert.equal(exactProbeChat(payload, shared, nonce), true);
  assert.equal(exactProbeChat({ objectId: 1000, text: nonce }, shared, nonce), false);
});
test('peer disagreement or absent AOI has no owner-ID fallback', () => {
  assert.throws(() => sharedChatIdentity([speaker, peer(50050), peer(50051)], speaker), /Every peer/);
  assert.throws(() => sharedChatIdentity([speaker, peer(undefined)], speaker), /Every peer/);
  assert.throws(() => sharedChatIdentity([speaker], speaker), /Every peer/);
});
test('exact nonce rejects substring, extended nonce, wrong sender and old message', () => {
  for (const text of ['prefix' + nonce, nonce + '0', nonce + ' suffix', 'cg9999999999'])
    assert.equal(exactProbeChat({ objectId: 50050, text }, 50050, nonce), false);
  assert.equal(exactProbeChat({ objectId: 50051, text: nonce }, 50050, nonce), false);
});
