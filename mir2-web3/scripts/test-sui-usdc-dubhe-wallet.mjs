import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

// Execute the actual source module. Only the external SDK import is replaced;
// every SDK invocation is injected, so these tests never contact a relay.
const file = new URL('../apps/gateway/src/sui_usdc_dubhe_wallet.js', import.meta.url);
const source = await readFile(file, 'utf8');
const sdkImport = "import SignClient from '@walletconnect/sign-client';";
assert.equal(source.split(sdkImport).length, 2);
const executable = source.replace(sdkImport, 'const SignClient = { init() { throw Error("Uninjected SDK is forbidden in local tests"); } };');
const { createDubheWallet } = await import(`data:text/javascript;base64,${Buffer.from(executable).toString('base64')}`);
const CHAIN = 'sui:testnet';
const ADDRESS = `0x${'1'.repeat(64)}`;
const OTHER = `0x${'2'.repeat(64)}`;
const TOPIC = 'a'.repeat(64);
const SIGNATURE = Buffer.from('local-fixture-signature').toString('base64');
const TX = Buffer.from([1, 2, 3, 4]).toString('base64');
const DIGEST = '3'.repeat(44);
const flush = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function fixture(overrides = {}) {
  let now = 1800000000000;
  let nextTimer = 0;
  const timers = new Map();
  const approval = deferred();
  const events = { uri: [], expired: 0, disconnected: 0 };
  const client = new EventEmitter();
  const calls = { init: [], connect: [], requests: [], disconnect: [] };
  const session = { topic: TOPIC, expiry: now / 1000 + 600,
    namespaces: { sui: { accounts: [`${CHAIN}:${ADDRESS}`], chains: [CHAIN],
      methods: ['sui_signPersonalMessage', 'sui_signAndExecuteTransaction'], events: ['accountsChanged', 'chainChanged'] } } };
  client.connect = async (configuration) => {
    calls.connect.push(configuration);
    return { uri: overrides.uri ?? `wc:${'b'.repeat(64)}@2?relay-protocol=irn&symKey=${'c'.repeat(64)}&expiryTimestamp=${now / 1000 + 300}`,
      approval: () => approval.promise };
  };
  client.request = async (request) => { calls.requests.push(request); return overrides.request ? overrides.request(request) : {
    signature: SIGNATURE, bytes: request.request.method === 'sui_signPersonalMessage' ? request.request.params.message : TX, digest: DIGEST }; };
  client.disconnect = async (request) => { calls.disconnect.push(request); };
  const wallet = createDubheWallet({ projectId: 'd'.repeat(32), origin: overrides.origin ?? 'https://billing.example.test',
    onUri: (...args) => { events.uri.push(args); overrides.onUri?.(...args); },
    onExpired: () => { events.expired++; }, onDisconnect: () => { events.disconnected++; } }, {
    initSignClient: async (configuration) => { calls.init.push(configuration); return overrides.init ? overrides.init(configuration) : client; }, now: () => now,
    setTimer: (callback, milliseconds) => { const handle = ++nextTimer; timers.set(handle, { callback, due: now + milliseconds }); return handle; },
    clearTimer: (handle) => timers.delete(handle),
  });
  async function connected(approved = session) {
    const pending = wallet.connect();
    await flush(); approval.resolve(approved);
    const result = await pending;
    return result.accounts[0];
  }
  async function tick(milliseconds) {
    now += milliseconds;
    for (const [handle, timer] of [...timers]) if (timer.due <= now) { timers.delete(handle); timer.callback(); }
    await flush();
  }
  return { wallet, client, approval, calls, events, session, timers, connected, tick };
}

test('connect pins metadata, relay, namespaces and approval URI expiry; capabilities are absent', async () => {
  const f = fixture();
  const a = await f.connected();
  assert.deepEqual(a, { address: ADDRESS, chains: [CHAIN], features: ['sui:signPersonalMessage', 'sui:signAndExecuteTransaction'] });
  assert.equal(f.calls.init[0].relayUrl, 'wss://relay.walletconnect.com');
  assert.equal(f.calls.init[0].telemetryEnabled, false);
  assert.equal(f.calls.init[0].metadata.url, 'https://billing.example.test');
  assert.deepEqual(f.calls.init[0].metadata.icons, []);
  assert.deepEqual(f.calls.connect[0], { requiredNamespaces: { sui: { chains: [CHAIN],
    methods: ['sui_signPersonalMessage', 'sui_signAndExecuteTransaction'], events: ['accountsChanged', 'chainChanged'] } } });
  assert.match(f.events.uri[0][0], /^wc:/);
  assert.equal(f.events.uri[0][1].expiresAt, 1800000300000);
  assert.doesNotMatch(JSON.stringify(f.calls), /capability|authorization|usdc_|game-session/i);
  await f.wallet.disconnect();
});

test('metadata accepts canonical HTTPS and loopback HTTP only', async () => {
  for (const origin of ['http://billing.example.test', 'https://billing.example.test/', 'https://name:password@billing.example.test',
    'https://billing.example.test?capability=secret', 'https://billing.example.test#fragment', ' https://billing.example.test',
    'https://billing.example.test/path', 'https://billing.example.test\\path']) assert.throws(() => fixture({ origin }));
  for (const origin of ['http://127.0.0.1:7359', 'http://localhost:7359', 'http://[::1]:7359']) {
    const f = fixture({ origin }); await f.wallet.disconnect();
  }
});

test('multiple approved Testnet addresses remain available; wrong chains/accounts/namespace/expiry fail closed', async () => {
  const good = fixture(); good.session.namespaces.sui.accounts.push(`${CHAIN}:${OTHER}`);
  const pending = good.wallet.connect(); await flush(); good.approval.resolve(good.session);
  assert.equal((await pending).accounts.length, 2); await good.wallet.disconnect();
  const changes = [
    (s) => { s.namespaces.sui.accounts = [`sui:mainnet:${ADDRESS}`]; },
    (s) => { s.namespaces.sui.accounts.push(`sui:mainnet:${OTHER}`); },
    (s) => { s.namespaces.sui.accounts = ['sui:testnet:0x1']; },
    (s) => { s.namespaces.sui.accounts = [`${CHAIN}:0x${'0'.repeat(64)}`]; },
    (s) => { s.namespaces.sui.chains = ['sui:mainnet']; },
    (s) => { s.namespaces.sui.methods = ['sui_signPersonalMessage']; },
    (s) => { s.namespaces.sui.events = ['accountsChanged']; },
    (s) => { s.namespaces.eip155 = {}; },
    (s) => { s.expiry = 1799999999; },
    (s) => { s.expiry = Number.MAX_SAFE_INTEGER; },
  ];
  for (const change of changes) {
    const f = fixture(); change(f.session);
    await assert.rejects(f.connected());
    assert.equal(f.calls.requests.length, 0);
    assert.equal(f.client.eventNames().length, 0);
    await f.wallet.disconnect();
  }
});

test('official personal message and BCS transaction params are exact; returned bytes must match', async () => {
  const f = fixture(); const account = await f.connected();
  const message = new TextEncoder().encode('MIR2 订单确认 ✓');
  assert.deepEqual(await f.wallet.signPersonalMessage({ account, message }), { signature: SIGNATURE });
  assert.deepEqual(f.calls.requests[0], { topic: TOPIC, chainId: CHAIN,
    request: { method: 'sui_signPersonalMessage', params: { message: Buffer.from(message).toString('base64') } } });
  assert.deepEqual(await f.wallet.signAndExecuteTransaction({ account, transactionBytesB64: TX }), { digest: DIGEST, bytes: TX, signature: SIGNATURE });
  assert.deepEqual(f.calls.requests[1], { topic: TOPIC, chainId: CHAIN,
    request: { method: 'sui_signAndExecuteTransaction', params: { transactionBlock: TX } } });
  assert.equal('transaction' in f.calls.requests[1].request.params, false);
  await f.wallet.disconnect();
});

test('wrong selected account/chain, JSON transactions and noncanonical bytes never reach the wallet', async () => {
  const f = fixture(); const account = await f.connected();
  await assert.rejects(f.wallet.signPersonalMessage({ account: { ...account, address: OTHER }, message: new Uint8Array([1]) }));
  await assert.rejects(f.wallet.signPersonalMessage({ account: { ...account, chains: ['sui:mainnet'] }, message: new Uint8Array([1]) }));
  for (const bytes of ['{"version":2}', 'AQ', 'AR==', '', undefined]) await assert.rejects(f.wallet.signAndExecuteTransaction({ account, transactionBytesB64: bytes }));
  assert.equal(f.calls.requests.length, 0);
  await f.wallet.disconnect();
});

test('missing/changed bytes, invalid signature or digest cannot be reported as successful payment', async () => {
  for (const result of [{ digest: DIGEST, signature: SIGNATURE }, { bytes: 'BQ==', digest: DIGEST, signature: SIGNATURE },
    { bytes: TX, digest: 'not-a-digest', signature: SIGNATURE }, { bytes: TX, digest: DIGEST, signature: '' }]) {
    const f = fixture({ request: () => result }); const account = await f.connected();
    await assert.rejects(f.wallet.signAndExecuteTransaction({ account, transactionBytesB64: TX }));
    await f.wallet.disconnect();
  }
});

test('personal-message response must contain exactly the requested canonical message bytes', async () => {
  for (const result of [{ signature: SIGNATURE }, { signature: SIGNATURE, bytes: 'Ag==' },
    { signature: SIGNATURE, bytes: 'AR==' }, { signature: '', bytes: 'AQ==' }]) {
    const f = fixture({ request: () => result }); const account = await f.connected();
    await assert.rejects(f.wallet.signPersonalMessage({ account, message: new Uint8Array([1]) }));
    await f.wallet.disconnect();
  }
});

test('user rejection propagates and never fabricates a signature or digest', async () => {
  const f = fixture(); const connecting = f.wallet.connect(); await flush();
  f.approval.reject(Error('private wallet diagnostics'));
  await assert.rejects(connecting, /rejected or unavailable/);
  assert.equal(f.client.eventNames().length, 0);
  const g = fixture({ request: () => Promise.reject(Error('user rejected')) });
  const account = await g.connected();
  await assert.rejects(g.wallet.signPersonalMessage({ account, message: new Uint8Array([1]) }), /rejected or unavailable/);
  await g.wallet.disconnect(); await f.wallet.disconnect();
});

test('approval expiry rejects immediately, removes listeners, and retires late approved sessions', async () => {
  const f = fixture(); const waiting = f.wallet.connect(); const rejected = assert.rejects(waiting, /expired/);
  await flush(); await f.tick(300001); await rejected;
  assert.equal(f.events.expired, 1); assert.equal(f.client.eventNames().length, 0);
  f.approval.resolve(f.session); await flush();
  assert.equal(f.calls.disconnect.length, 1);
  assert.equal(f.calls.requests.length, 0);
  await f.wallet.disconnect();
});

test('disconnect during approval and during payment fences late results and clears timers/listeners', async () => {
  const f = fixture(); const waiting = f.wallet.connect(); const rejected = assert.rejects(waiting, /disconnected/);
  await flush(); await f.wallet.disconnect(); await rejected;
  f.approval.resolve(f.session); await flush();
  assert.equal(f.calls.disconnect.length, 1); assert.equal(f.timers.size, 0); assert.equal(f.client.eventNames().length, 0);
  const response = deferred(); const g = fixture({ request: () => response.promise }); const account = await g.connected();
  const signing = g.wallet.signAndExecuteTransaction({ account, transactionBytesB64: TX });
  const signingRejected = assert.rejects(signing, /disconnected/);
  await flush(); await g.wallet.disconnect(); await signingRejected;
  response.resolve({ bytes: TX, digest: DIGEST, signature: SIGNATURE }); await flush();
  assert.equal(g.timers.size, 0); assert.equal(g.client.eventNames().length, 0);
});

test('account/chain events, session updates and expiry invalidate all in-flight requests', async () => {
  for (const event of ['accountsChanged', 'chainChanged', 'session_update', 'session_expire', 'session_delete']) {
    const response = deferred(); const f = fixture({ request: () => response.promise }); const account = await f.connected();
    const signing = f.wallet.signPersonalMessage({ account, message: new Uint8Array([1]) });
    const rejected = assert.rejects(signing);
    await flush();
    if (event === 'accountsChanged' || event === 'chainChanged') f.client.emit('session_event', { topic: TOPIC, params: { event: { name: event, data: [] } } });
    else f.client.emit(event, { topic: TOPIC, params: { namespaces: {} } });
    await rejected; response.resolve({ signature: SIGNATURE }); await flush();
    assert.equal(f.client.eventNames().length, 0); assert.equal(f.timers.size, 0);
    await f.wallet.disconnect();
  }
});

test('connect bursts share one proposal; simultaneous approvals cannot submit another wallet request', async () => {
  const response = deferred(); const f = fixture({ request: () => response.promise });
  const first = f.wallet.connect(), second = f.wallet.connect();
  assert.equal(first, second); await flush(); f.approval.resolve(f.session);
  const account = (await first).accounts[0]; assert.equal(f.calls.connect.length, 1);
  const signing = f.wallet.signPersonalMessage({ account, message: new Uint8Array([1]) });
  await assert.rejects(f.wallet.signAndExecuteTransaction({ account, transactionBytesB64: TX }), /already pending/);
  response.resolve({ signature: SIGNATURE, bytes: 'AQ==' }); await signing;
  assert.equal(f.calls.requests.length, 1); await f.wallet.disconnect();
});

test('invalid or expired WC URI cannot be rendered or approved', async () => {
  for (const uri of ['https://evil.invalid', `wc:${'b'.repeat(64)}@2?relay-protocol=other&symKey=${'c'.repeat(64)}`,
    `wc:${'b'.repeat(64)}@2?relay-protocol=irn&symKey=${'c'.repeat(64)}&expiryTimestamp=1`]) {
    const f = fixture({ uri }); await assert.rejects(f.wallet.connect());
    assert.equal(f.events.uri.length, 0); assert.equal(f.calls.requests.length, 0); await f.wallet.disconnect();
  }
});

test('handshake and signing each have bounded expiry, with late SDK results fenced', async () => {
  const initial = deferred(); const f = fixture({ init: () => initial.promise });
  const opening = f.wallet.connect(); const rejected = assert.rejects(opening, /expired/);
  await flush(); await f.tick(30001); await rejected;
  initial.resolve(f.client); await flush();
  assert.equal(f.calls.connect.length, 0); assert.equal(f.events.expired, 1); assert.equal(f.timers.size, 0);
  const response = deferred(); const g = fixture({ request: () => response.promise }); const account = await g.connected();
  const signing = g.wallet.signPersonalMessage({ account, message: new Uint8Array([1]) }); const declined = assert.rejects(signing, /expired/);
  await flush(); await g.tick(120001); await declined;
  response.resolve({ signature: SIGNATURE, bytes: 'AQ==' }); await flush();
  assert.equal(g.events.expired, 1); assert.equal(g.timers.size, 0); assert.equal(g.client.eventNames().length, 0);
  await f.wallet.disconnect(); await g.wallet.disconnect();
});

test('an immediate disconnect from the QR callback cannot strand approval or accept its late result', async () => {
  const f = fixture({ onUri: () => f.wallet.disconnect() });
  const opening = f.wallet.connect(); const rejected = assert.rejects(opening, /current|disconnected/);
  await flush(); await rejected;
  f.approval.resolve(f.session); await flush();
  assert.equal(f.calls.disconnect.length, 1); assert.equal(f.timers.size, 0); assert.equal(f.client.eventNames().length, 0);
});

test('events for other sessions do not invalidate this checkout', async () => {
  const f = fixture(); const account = await f.connected();
  f.client.emit('session_delete', { topic: 'f'.repeat(64) });
  f.client.emit('session_event', { topic: 'f'.repeat(64), params: { event: { name: 'chainChanged', data: 'sui:mainnet' } } });
  assert.deepEqual(await f.wallet.signPersonalMessage({ account, message: new Uint8Array([1]) }), { signature: SIGNATURE });
  assert.equal(f.events.disconnected, 0); await f.wallet.disconnect();
});
