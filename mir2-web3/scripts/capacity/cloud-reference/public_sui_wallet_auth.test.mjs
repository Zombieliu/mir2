import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import { exchangeSuiWallet, migrateGuestPool } from './public_sui_wallet_auth.mjs';
const requireWeb = createRequire(new URL('../../../apps/web/package.json', import.meta.url));
const { verifyPersonalMessageSignature } = await import(pathToFileURL(requireWeb.resolve('@mysten/sui/verify')).href);
const origin = 'https://example.invalid';
const json = (body, status = 200) => new Response(JSON.stringify(body), { status });

test('official challenge, real Ed25519 PersonalMessage verify, channel exchange; no transaction or RPC', async () => {
  const actor = { ordinal: 0, name: 'TestOnly' }, routes = []; let saved = 0;
  const fetchImpl = async (url, request) => {
    routes.push(new URL(url).pathname); const body = JSON.parse(request.body);
    assert.equal(request.headers.origin, origin); assert.equal(request.redirect, 'error');
    if (routes.length === 1) {
      assert.equal(saved, 1); assert.equal(body.authMethod, 'wallet'); assert.equal(body.address, actor.walletIdentity.address);
      return json({ challenge: 'server.challenge', challengeId: 'nonce-1', issuedAt: Date.now(), expiresAt: Date.now() + 60000 });
    }
    if (routes.length === 2) {
      const message = JSON.parse(body.message);
      assert.deepEqual(Object.keys(message), ['purpose', 'accountId', 'address', 'origin', 'issuedAt', 'expiresAt', 'nonce', 'challenge', 'authMethod']);
      assert.equal(message.purpose, 'mir2-sui-login'); assert.equal(message.accountId, 'sui:' + body.address);
      assert.equal(message.authMethod, 'wallet'); assert.equal(message.nonce, 'nonce-1'); assert.equal(message.origin, origin);
      const verified = await verifyPersonalMessageSignature(new TextEncoder().encode(body.message), body.signature, { address: body.address });
      assert.equal(verified.toSuiAddress(), body.address);
      return json({ provider: 'suiWallet', accountId: 'sui:' + body.address, token: 'official.identity' });
    }
    assert.deepEqual(body, { provider: 'suiWallet', credential: 'official.identity' });
    return json({ provider: 'suiWallet', accountId: 'synthetic-player-1', token: 'official.gateway', expiresAt: Date.now() + 60000 });
  };
  const proof = await exchangeSuiWallet(origin, actor, { fetchImpl, persist: async () => { saved++; } });
  assert.equal(proof.token, 'official.gateway'); assert.equal(saved, 2);
  assert.deepEqual(routes, ['/api/passkey/challenge', '/api/passkey/login', '/api/channels/session/exchange']);
  const firstSecret = actor.walletIdentity.secretKey, firstAddress = actor.walletIdentity.address; routes.length = 0; saved = 1;
  await exchangeSuiWallet(origin, actor, { fetchImpl });
  assert.equal(actor.walletIdentity.secretKey, firstSecret); assert.equal(actor.walletIdentity.address, firstAddress);
});

test('schema migration preserves old guest identity and credential metadata without reusing its character', () => {
  const old = { schema: 'mir2.cloud-reference-private-guests.v1', actors: [{ ordinal: 0, name: 'GuestTest', accountId: 'old-guest', cookie: 'private-cookie', characterIndex: 7 }] };
  const snapshot = JSON.stringify(old), next = migrateGuestPool(old);
  assert.equal(JSON.stringify(old), snapshot); assert.equal(next.schema, 'mir2.cloud-reference-private-actors.v2');
  assert.equal(next.actors[0].cookie, 'private-cookie'); assert.equal(next.actors[0].accountId, 'old-guest');
  assert.equal(next.actors[0].identityHistory[0].characterIndex, 7); assert.equal(next.actors[0].walletIdentity, undefined);
  assert.equal(migrateGuestPool(next), next);
});

test('public wallet HTTP failure does not echo credentials and performs no fallback', async () => {
  let calls = 0;
  await assert.rejects(exchangeSuiWallet(origin, {}, { fetchImpl: async () => { calls++; return json({ token: 'never-public' }, 403); } }),
    error => error.message.includes('403') && !error.message.includes('never-public'));
  assert.equal(calls, 1);
});
