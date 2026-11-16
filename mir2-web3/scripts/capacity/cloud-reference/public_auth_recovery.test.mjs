import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import { exchangeSuiWallet, PublicAuthFetchTimeout, PUBLIC_AUTH_HTTP_TIMEOUT_MS } from './public_sui_wallet_auth.mjs';
import { SignedClient, atomicPrivate, preconnectWalletProof, login } from './public_signed_probe.mjs';
import { LoginBudget, LOGIN_WINDOW_MS } from '../../../apps/web/scripts/playtest-capacity-soak-core.mjs';

const requireWeb = createRequire(new URL('../../../apps/web/package.json', import.meta.url));
const { Ed25519Keypair } = await import(pathToFileURL(requireWeb.resolve('@mysten/sui/keypairs/ed25519')).href);
const origin = 'https://example.invalid';
const routes = ['/api/passkey/challenge', '/api/passkey/login', '/api/channels/session/exchange'];
const json = (body, status = 200) => new Response(JSON.stringify(body), { status });
const challenge = () => ({ challenge: 'offline-challenge', challengeId: 'offline-nonce', issuedAt: Date.now(), expiresAt: Date.now() + 60000 });
const timeout = () => new PublicAuthFetchTimeout(routes[0], 8001);
function actor() {
  const key = Ed25519Keypair.generate();
  return { name: 'OfflineSynthetic', walletIdentity: { kind: 'synthetic-ed25519-wallet', secretKey: key.getSecretKey(),
    address: key.getPublicKey().toSuiAddress(), accountId: 'offline-account', characterIndex: 7 } };
}
const gateway = () => ({ provider: 'suiWallet', accountId: 'offline-account', token: 'offline.gateway' });
function successfulRoute(url, request) {
  const route = new URL(url).pathname;
  if (route === routes[0]) return json(challenge());
  if (route === routes[1]) return json({ provider: 'suiWallet', accountId: 'sui:' + JSON.parse(request.body).address, token: 'offline.identity' });
  return json(gateway());
}
async function harness(t, attempts = []) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-auth-recovery-test-'));
  t.after(async () => {
    assert.ok(path.isAbsolute(directory) && path.basename(directory).startsWith('mir2-auth-recovery-test-'));
    for (const name of await fs.readdir(directory)) await fs.unlink(path.join(directory, name));
    await fs.rmdir(directory);
  });
  const a = actor(), events = [], ledgers = [], clock = { now: 1800000000000, waited: 0, aliveTicks: 0 };
  const context = { connection: 0, webBase: origin, endpoint: 'wss://example.invalid/ws', pool: { actors: [a] }, secrets: [],
    ledgerFile: path.join(directory, 'auth-ledger.json'), actorsFile: path.join(directory, 'actors.private.json'),
    loginBudget: new LoginBudget(attempts, 0, () => clock.now), firstAttemptAdmissionOk: true,
    assert() {}, emit(row) { events.push(structuredClone(row)); } };
  const c = new SignedClient(context.endpoint, a, 0, context);
  const options = { async persistPrivate(file, value) {
    await atomicPrivate(file, value);
    if (file === context.ledgerFile) ledgers.push(JSON.parse(await fs.readFile(file, 'utf8')));
  }, async wait(ms) { clock.now += ms; clock.waited += ms; clock.aliveTicks++; await Promise.resolve(); } };
  return { c, context, options, ledgers, events, clock };
}

for (const failingRoute of routes) test('safe fetch TimeoutError identifies only route ' + failingRoute, async () => {
  const a = actor(), calls = [], privateText = 'never-echo-body-token-cookie';
  await assert.rejects(exchangeSuiWallet(origin, a, { fetchImpl: async (url, request) => {
    const route = new URL(url).pathname; calls.push(route);
    assert.equal(request.signal.aborted, false);
    if (route === failingRoute) throw new DOMException(privateText, 'TimeoutError');
    return successfulRoute(url, request);
  } }), error => {
    assert.ok(error instanceof PublicAuthFetchTimeout); assert.equal(error.code, 'PUBLIC_AUTH_FETCH_TIMEOUT');
    assert.equal(error.route, failingRoute); assert.equal(error.timeoutMs, 8000);
    assert.ok(Number.isFinite(error.elapsedMs) && error.elapsedMs >= 0); assert.equal(error.cause, undefined);
    const publicText = JSON.stringify(error) + error.message + error.stack;
    for (const secret of [privateText, a.walletIdentity.secretKey, a.walletIdentity.address, 'offline.identity']) assert.equal(publicText.includes(secret), false);
    return true;
  });
  assert.equal(PUBLIC_AUTH_HTTP_TIMEOUT_MS, 8000); assert.deepEqual(calls, routes.slice(0, routes.indexOf(failingRoute) + 1));
});

test('HTTP denial, malformed proof and response-body timeout are not typed fetch timeouts', async () => {
  for (const status of [400, 403, 429, 500, 503]) {
    let calls = 0;
    await assert.rejects(exchangeSuiWallet(origin, actor(), { fetchImpl: async () => { calls++; return json({ token: 'never-echo' }, status); } }), error => {
      assert.equal(error instanceof PublicAuthFetchTimeout, false); assert.match(error.message, new RegExp(String(status)));
      assert.equal(error.message.includes('never-echo'), false); return true;
    });
    assert.equal(calls, 1);
  }
  await assert.rejects(exchangeSuiWallet(origin, actor(), { fetchImpl: async () => json({ challenge: 'malformed' }) }), /missing or expired/);
  await assert.rejects(exchangeSuiWallet(origin, actor(), { fetchImpl: async () => ({ ok: true, json() { throw new DOMException('body timeout', 'TimeoutError'); } }) }),
    error => error.name === 'TimeoutError' && !(error instanceof PublicAuthFetchTimeout));
});

test('three full public-proof attempts reuse the same SDK wallet and atomically count the original ledger before HTTP', async t => {
  const h = await harness(t), initial = structuredClone(h.c.actor), addresses = [];
  let calls = 0;
  h.options.exchange = (base, a, options) => exchangeSuiWallet(base, a, { ...options, fetchImpl: async (url, request) => {
    if (new URL(url).pathname === routes[0]) {
      addresses.push(JSON.parse(request.body).address);
      const disk = JSON.parse(await fs.readFile(h.context.ledgerFile, 'utf8'));
      assert.equal(disk.attempts.length, addresses.length); assert.equal(disk.purpose, 'cloud-reference-public-auth-budget');
      if (++calls <= 2) throw new DOMException('private-timeout-message', 'TimeoutError');
    }
    return successfulRoute(url, request);
  } });
  const proof = await preconnectWalletProof(h.c, h.context, 'admission', h.options);
  assert.equal(proof.token, 'offline.gateway'); assert.deepEqual(h.c.actor, initial);
  assert.equal(new Set(addresses).size, 1); assert.equal(addresses.length, 3);
  assert.deepEqual(h.ledgers.map(l => l.attempts.length), [1, 2, 3]);
  assert.ok(h.ledgers.every(l => l.endpoint === h.context.endpoint && l.webBase === origin && l.attempts.every(a => a.kind === 'admission')));
  assert.deepEqual(h.context.admissionTransportRecovery.map(r => [r.attempt, r.outcome]), [[1, 'failed'], [2, 'failed'], [3, 'recovered']]);
  assert.equal(h.context.firstAttemptAdmissionOk, false); assert.equal(h.c.ws, undefined); assert.equal(h.c.sequence, 0);
  const publicText = JSON.stringify(h.context.admissionTransportRecovery);
  for (const secret of [initial.name, initial.walletIdentity.secretKey, initial.walletIdentity.address, 'private-timeout-message', 'offline.gateway']) assert.equal(publicText.includes(secret), false);
});

test('a newly generated wallet is persisted once then pinned through a challenge transport retry', async t => {
  const h = await harness(t); delete h.c.actor.walletIdentity;
  let calls = 0, savedSecret;
  h.options.exchange = (base, a, options) => exchangeSuiWallet(base, a, { ...options, fetchImpl: async (url, request) => {
    if (new URL(url).pathname === routes[0]) {
      const disk = JSON.parse(await fs.readFile(h.context.actorsFile, 'utf8'));
      if (!savedSecret) savedSecret = disk.actors[0].walletIdentity.secretKey;
      assert.equal(disk.actors[0].walletIdentity.secretKey, savedSecret);
      if (++calls === 1) throw new DOMException('offline timeout', 'TimeoutError');
    }
    return successfulRoute(url, request);
  } });
  await preconnectWalletProof(h.c, h.context, 'admission', h.options);
  assert.equal(calls, 2); assert.equal(h.c.actor.walletIdentity.secretKey, savedSecret); assert.equal(h.ledgers.length, 2);
});

test('a timeout at any of the three routes restarts from a fresh challenge rather than replaying a partial proof', async t => {
  for (const failingRoute of routes) {
    const h = await harness(t), calls = []; let failed = false, challenges = 0;
    h.options.exchange = (base, a, options) => exchangeSuiWallet(base, a, { ...options, fetchImpl: async (url, request) => {
      const route = new URL(url).pathname; calls.push(route);
      if (route === routes[0]) challenges++;
      if (route === failingRoute && !failed) { failed = true; throw new DOMException('offline-only timeout', 'TimeoutError'); }
      if (route === routes[0]) return json({ ...challenge(), challengeId: 'fresh-nonce-' + challenges });
      if (route === routes[1]) assert.equal(JSON.parse(JSON.parse(request.body).message).nonce, 'fresh-nonce-' + challenges);
      return successfulRoute(url, request);
    } });
    await preconnectWalletProof(h.c, h.context, 'admission', h.options);
    assert.deepEqual(calls, [...routes.slice(0, routes.indexOf(failingRoute) + 1), ...routes]);
    assert.equal(challenges, 2); assert.deepEqual(h.ledgers.map(l => l.attempts.length), [1, 2]);
    assert.equal(h.context.admissionTransportRecovery[0].route, failingRoute);
    assert.equal(h.context.firstAttemptAdmissionOk, false);
    // A later actor's first-attempt success cannot reset earlier reliability.
    const next = new SignedClient(h.context.endpoint, h.c.actor, 1, h.context);
    await preconnectWalletProof(next, h.context, 'admission', { ...h.options, exchange: async () => gateway() });
    assert.equal(h.context.firstAttemptAdmissionOk, false); assert.equal(h.ledgers.length, 3);
  }
});

for (const purpose of ['admission', 'verification']) test(purpose + ' retry waits the full unchanged 901s bucket while existing actors can stay alive', async t => {
  const count = purpose === 'admission' ? 24 : 4;
  const h = await harness(t, Array.from({ length: count }, () => ({ at: 1800000000000, kind: purpose })));
  const alive = new SignedClient(h.context.endpoint, { name: 'OfflineAlreadyAdmitted' }, 1, h.context), heartbeats = [];
  alive.inGame = true;
  alive.ws = { readyState: 1, send(text) { heartbeats.push(JSON.parse(text).type); } };
  const wait = h.options.wait;
  h.options.wait = async ms => { await wait(ms); alive.heartbeat(); };
  let calls = 0;
  h.options.exchange = async () => { if (++calls === 1) throw timeout(); return gateway(); };
  await preconnectWalletProof(h.c, h.context, purpose, h.options);
  assert.equal(LOGIN_WINDOW_MS, 901000); assert.equal(h.clock.waited, 901000); assert.ok(h.clock.aliveTicks > 0);
  assert.deepEqual(h.ledgers.map(l => l.attempts.length), [count + 1, 1]);
  assert.ok(h.ledgers.every(l => l.attempts.every(a => a.kind === purpose)));
  assert.equal(h.context.loginBudget.notBefore, 0); assert.equal(calls, 2);
  assert.ok(heartbeats.includes('keepAlive')); assert.equal(alive.failure, undefined); assert.equal(alive.inGame, true);
});

test('third typed timeout exhausts exactly three normal reservations and stays a first-attempt reliability failure', async t => {
  const h = await harness(t); let calls = 0;
  h.options.exchange = async () => { calls++; throw timeout(); };
  await assert.rejects(preconnectWalletProof(h.c, h.context, 'admission', h.options), PublicAuthFetchTimeout);
  assert.equal(calls, 3); assert.deepEqual(h.ledgers.map(l => l.attempts.length), [1, 2, 3]);
  assert.equal(h.context.admissionTransportRecovery.at(-1).retryExhausted, true);
  assert.equal(h.context.admissionTransportRecovery.at(-1).retryScheduled, false);
  assert.equal(h.context.firstAttemptAdmissionOk, false); assert.equal(h.c.ws, undefined);
});

test('HTTP429, bad proof, signature failures and unwrapped TimeoutError never trigger a retry', async t => {
  for (const failure of [new Error('HTTP429'), new Error('invalid proof'), new Error('signature failed'), new DOMException('unwrapped', 'TimeoutError')]) {
    const h = await harness(t); let calls = 0;
    h.options.exchange = async () => { calls++; throw failure; };
    await assert.rejects(preconnectWalletProof(h.c, h.context, 'admission', h.options), error => error === failure);
    assert.equal(calls, 1); assert.equal(h.ledgers.length, 1);
    assert.equal(h.context.admissionTransportRecovery[0].retryScheduled, false);
  }
});

test('typed timeout after any WebSocket creation or event cannot retry', async t => {
  for (const mutate of [c => { c.webSocketCreated = true; }, c => { c.ws = {}; }, c => { c.sequence = 1; }, c => { c.inGame = true; }]) {
    const h = await harness(t); let calls = 0;
    h.options.exchange = async () => { calls++; mutate(h.c); throw timeout(); };
    await assert.rejects(preconnectWalletProof(h.c, h.context, 'admission', h.options), PublicAuthFetchTimeout);
    assert.equal(calls, 1); assert.equal(h.ledgers.length, 1);
    assert.equal(h.context.admissionTransportRecovery[0].beforeWebSocket, false);
    assert.equal(h.context.admissionTransportRecovery[0].retryScheduled, false);
  }
  const h = await harness(t); h.c.webSocketCreated = true;
  await assert.rejects(preconnectWalletProof(h.c, h.context, 'admission', h.options), /never created a WebSocket/);
  assert.equal(h.ledgers.length, 0);
});

test('wallet, account, name and character changes stop retry without exposing changed identity', async t => {
  for (const mutate of [c => { c.actor.name = 'changed-name'; }, c => { c.actor.walletIdentity.secretKey = 'changed-private-key'; },
    c => { c.actor.walletIdentity.accountId = 'changed-account'; }, c => { c.actor.walletIdentity.characterIndex = 8; }]) {
    const h = await harness(t); let calls = 0;
    h.options.exchange = async () => { calls++; mutate(h.c); throw timeout(); };
    await assert.rejects(preconnectWalletProof(h.c, h.context, 'admission', h.options), /wallet or character changed/);
    assert.equal(calls, 1); assert.equal(h.ledgers.length, 1);
    assert.equal(h.context.admissionTransportRecovery[0].failureCode, 'PUBLIC_AUTH_IDENTITY_CHANGED');
    assert.equal(h.context.admissionTransportRecovery[0].retryScheduled, false);
  }
});

test('WebSocket open, login ACK, StartGame and complete-bootstrap failures stay outside auth retry', async t => {
  for (const phase of ['connect', 'loginAck', 'startGame', 'bootstrap']) {
    const h = await harness(t); let authCalls = 0, connects = 0;
    h.options.exchange = async () => { authCalls++; return gateway(); };
    h.c.connect = async () => { connects++; h.c.webSocketCreated = true; h.c.ws = {}; if (phase === 'connect') throw timeout(); };
    h.c.request = async command => {
      if (command.type === 'passkeyLogin') {
        if (phase === 'loginAck') throw new Error('LoginSuccess ACK failed');
        return { payload: { characters: [{ name: h.c.actor.name, index: 7 }] } };
      }
      if (phase === 'startGame') throw new Error('StartGame ACK failed');
      return { monotonicMs: 1, payload: { result: 4 } };
    };
    h.c.wait = async () => { throw new Error('complete owner bootstrap failed'); };
    await assert.rejects(login(h.c, h.context, 'admission', h.options));
    assert.equal(authCalls, 1); assert.equal(connects, 1); assert.equal(h.ledgers.length, 1);
    assert.equal(h.context.admissionTransportRecovery, undefined);
  }
});

test('ledger persistence failure and finite-wall stop never spend another HTTP attempt', async t => {
  const h = await harness(t); let calls = 0;
  h.options.exchange = async () => { calls++; return gateway(); };
  h.options.persistPrivate = async () => { throw new Error('offline atomic write failed'); };
  await assert.rejects(preconnectWalletProof(h.c, h.context, 'admission', h.options), /atomic write failed/);
  assert.equal(calls, 0); assert.equal(h.context.loginBudget.attempts.length, 1);
  const full = await harness(t, Array.from({ length: 25 }, () => ({ at: 1800000000000, kind: 'admission' })));
  full.options.exchange = async () => { calls++; return gateway(); };
  full.context.assert = () => { if (full.clock.aliveTicks >= 3) throw new Error('finite wall expired'); };
  await assert.rejects(preconnectWalletProof(full.c, full.context, 'admission', full.options), /finite wall expired/);
  assert.equal(calls, 0); assert.equal(full.ledgers.length, 0); assert.equal(full.clock.aliveTicks, 3);
});
