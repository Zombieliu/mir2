import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import vm from 'node:vm';

const base = new URL('../apps/gateway/src/', import.meta.url);
const source = await readFile(new URL('sui_usdc_checkout.js', base), 'utf8');
const html = await readFile(new URL('sui_usdc_checkout.html', base), 'utf8');
const PAYER = `0x${'1'.repeat(64)}`;
const OTHER = `0x${'2'.repeat(64)}`;
const MERCHANT = `0x${'3'.repeat(64)}`;
const CAPABILITY = `usdc_${'a'.repeat(64)}.${'x'.repeat(43)}`;
const BCS = Buffer.from([0, 1, 2, 3, 4]).toString('base64');
const SIGNATURE = Buffer.from('local fixture signature').toString('base64');
const DIGEST = '4'.repeat(44);
const ACCOUNT = { address: PAYER, chains: ['sui:testnet'], features: ['sui:signPersonalMessage', 'sui:signAndExecuteTransaction'] };
const clone = (value) => structuredClone(value);
const flush = async () => { for (let index = 0; index < 24; index++) await Promise.resolve(); };
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

class Element {
  constructor(tag = 'div') { this.tag = tag; this.children = []; this._value = ''; this.disabled = false; this.hidden = false; this.textContent = ''; this.src = ''; }
  get options() { return this.children; }
  get value() { return this.tag === 'select' && !this.children.some((item) => item.value === this._value) ? '' : this._value; }
  set value(value) { this._value = String(value); }
  append(child) { this.children.push(child); if (this.tag === 'select' && !this._value) this._value = child.value; }
  replaceChildren(...children) { this.children = []; this._value = ''; for (const item of children) this.append(item); }
  removeAttribute(name) { if (name === 'src') this.src = ''; }
}

function fixture(options = {}) {
  const elements = new Map();
  for (const match of html.matchAll(/<([a-z]+)\b[^>]*\bid="([^"]+)"[^>]*>/g)) {
    const element = new Element(match[1]); element.hidden = /\bhidden\b/.test(match[0]);
    element.disabled = /\bdisabled\b/.test(match[0]); elements.set(match[2], element);
  }
  const byId = (id) => { assert.ok(elements.has(id), `Missing actual checkout element ${id}`); return elements.get(id); };
  let now = 1800000000000;
  let timerId = 0;
  const intervals = new Map();
  const requests = [], phone = [], qrCalls = [], historyCalls = [], errors = [];
  let extensionChange;
  const extension = { name: 'Slush', features: {
    'standard:connect': { connect: async () => options.extensionConnect ? options.extensionConnect() : { accounts: [clone(ACCOUNT)] } },
    'standard:events': { on: (name, callback) => { extensionChange = callback; return () => { extensionChange = undefined; }; } },
    'sui:signPersonalMessage': { signPersonalMessage: async (input) => { extension.inputs.push(['message', input]); return { signature: SIGNATURE }; } },
    'sui:signAndExecuteTransaction': { signAndExecuteTransaction: async (input) => { extension.inputs.push(['pay', input]); return { digest: DIGEST }; } },
  }, inputs: [] };
  let server = {
    order: { id: `usdc_${'a'.repeat(64)}`, network: 'testnet', amountMicros: '10000000', credits: 1000,
      expiresAtMs: now + 1200000, state: 'prepared' },
    chain: 'sui:testnet', coinType: '0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC',
    merchant: MERCHANT, decimals: 6, testPayment: true, gasPayer: 'wallet', payer: null, invoiceId: null,
    payExpired: false, chainConfirmed: false, gameDelivered: false,
    dubheWallet: { available: options.configured !== false, projectId: options.configured === false ? null : 'd'.repeat(32), chain: 'sui:testnet' },
  };
  const listeners = new Map();
  const window = {
    addEventListener: (name, listener) => { if (!listeners.has(name)) listeners.set(name, []); listeners.get(name).push(listener); },
    dispatchEvent: (event) => {
      if (event.type === 'wallet-standard:app-ready' && options.injectSlush !== false) event.detail.register(extension);
      for (const listener of listeners.get(event.type) || []) listener(event);
    },
    Mir2DubheQr: { toDataURL: async (uri) => { qrCalls.push(uri); return options.qr ? options.qr(uri) : 'data:image/png;base64,TE9DQUw='; } },
    Mir2DubheWallet: { create: (callbacks) => {
      const approval = deferred();
      const item = { callbacks, approval, calls: [], disconnected: false };
      const adapter = {
        connect: async () => {
          item.calls.push(['connect']);
          Promise.resolve(callbacks.onUri(`wc:${'b'.repeat(64)}@2?relay-protocol=irn&symKey=${'c'.repeat(64)}`, { expiresAt: now + (options.qrLifetime ?? 60000) })).catch((error) => errors.push(error));
          return approval.promise;
        },
        signPersonalMessage: async (input) => {
          item.calls.push(['message', input]);
          return options.personal ? options.personal(input) : { signature: SIGNATURE };
        },
        signAndExecuteTransaction: async (input) => {
          item.calls.push(['pay', input]);
          return options.payment ? options.payment(input) : { digest: DIGEST, bytes: BCS, signature: SIGNATURE };
        },
        disconnect: async () => { item.calls.push(['disconnect']); item.disconnected = true; callbacks.onDisconnect(); },
      };
      item.adapter = adapter; phone.push(item); return adapter;
    } },
  };
  if (options.missingSdk) delete window.Mir2DubheWallet;
  class FakeDate extends Date { static now() { return options.dateNow ? options.dateNow(now) : now; } }
  const context = vm.createContext({ window, document: { getElementById: byId, createElement: (tag) => new Element(tag) },
    location: { hash: `#${options.capability ?? CAPABILITY}`, pathname: '/v1/billing/sui-usdc/pay', origin: 'https://billing.example.test' },
    history: { replaceState: (...args) => historyCalls.push(args) },
    CustomEvent: class { constructor(type, init) { this.type = type; this.detail = init.detail; } },
    Date: FakeDate, TextEncoder, Uint8Array, console,
    setInterval: (callback, milliseconds) => { const id = ++timerId; intervals.set(id, { callback, milliseconds, due: now + milliseconds }); return id; },
    clearInterval: (id) => intervals.delete(id),
    fetch: async (url, configuration) => {
      const body = JSON.parse(configuration.body); requests.push({ url, configuration, body });
      if (options.intercept) {
        const overridden = await options.intercept(body, clone(server));
        if (overridden !== undefined) return { ok: overridden.ok !== false, json: async () => clone(overridden.value ?? overridden) };
      }
      let value;
      switch (body.operation) {
        case 'status': value = clone(server); break;
        case 'challenge': value = { message: 'MIR2 order-bound wallet confirmation\nUSDC 10.000000\nmerchant fixed', state: clone(server) }; break;
        case 'bindPayer': server.payer = body.payer; server.invoiceId = `0x${'e'.repeat(64)}`; server.order.state = 'awaitingPayment'; value = clone(server); break;
        case 'buildPayBytes': value = { transactionBytesB64: BCS, state: clone(server) }; break;
        case 'buildPay': value = { transactionJson: '{"version":2,"sender":"fixed-server-payer"}', state: clone(server) }; break;
        case 'submitted': server.order.state = 'paymentSubmitted'; server.order.transactionDigest = body.digest; value = clone(server); break;
        default: throw Error(`Unexpected checkout operation ${body.operation}`);
      }
      return { ok: true, json: async () => value };
    },
  });
  vm.runInContext(source, context, { filename: 'actual-sui-usdc-checkout.js' });
  const click = (id, force = false) => byId(id).disabled && !force ? Promise.resolve() : Promise.resolve(byId(id).onclick?.());
  async function connectPhone() {
    await flush(); const pending = click('connect'); await flush();
    assert.ok(phone.length); phone.at(-1).approval.resolve({ accounts: [clone(ACCOUNT)] }); await pending; await flush();
    return phone.at(-1);
  }
  async function changeWallet(value) { byId('wallets').value = value; byId('wallets').onchange(); await flush(); }
  async function advance(milliseconds) {
    now += milliseconds;
    for (const [id, interval] of [...intervals]) if (interval.due <= now && intervals.has(id)) {
      interval.due = now + interval.milliseconds;
      Promise.resolve(interval.callback()).catch((error) => errors.push(error));
    }
    await flush();
  }
  return { byId, click, requests, phone, qrCalls, historyCalls, errors, intervals, extension,
    window, connectPhone, changeWallet, advance, get server() { return server; },
    updateServer: (values) => { server = { ...server, ...values }; },
    extensionEvent: (value) => extensionChange?.(value),
    operations: () => requests.map((request) => request.body.operation) };
}

test('Dubhe scan remains default when Slush registers, with capability restricted to same-origin header', async () => {
  const f = fixture(); await flush();
  assert.equal(f.byId('wallets').value, 'dubhe-qr');
  assert.equal(f.byId('wallets').options[1].textContent, 'Slush · 浏览器扩展');
  assert.equal(f.historyCalls[0][2], '/v1/billing/sui-usdc/pay');
  for (const { url, configuration, body } of f.requests) {
    assert.equal(url, '/v1/billing/sui-usdc/session'); assert.equal(configuration.credentials, 'omit');
    assert.equal(configuration.redirect, 'error'); assert.equal(configuration.referrerPolicy, 'no-referrer');
    assert.equal(configuration.headers.Authorization, `Bearer ${CAPABILITY}`);
    assert.deepEqual(body, { operation: 'status' });
  }
});

test('phone flow displays QR, binds proof, pays exact server BCS and waits for server credit confirmation', async () => {
  const f = fixture(); await flush();
  const connecting = f.click('connect'); await flush();
  assert.equal(f.byId('dubhe-panel').hidden, false); assert.match(f.byId('dubhe-qr').src, /^data:image\/png/);
  assert.equal(f.qrCalls.length, 1); assert.match(f.qrCalls[0], /^wc:/);
  assert.deepEqual(Object.keys(f.phone[0].callbacks).sort(), ['onDisconnect', 'onExpired', 'onUri', 'origin', 'projectId']);
  assert.equal(f.phone[0].callbacks.origin, 'https://billing.example.test');
  assert.equal(JSON.stringify(f.phone[0].callbacks).includes(CAPABILITY), false);
  f.phone[0].approval.resolve({ accounts: [clone(ACCOUNT)] }); await connecting; await flush();
  assert.equal(f.byId('dubhe-panel').hidden, true);
  assert.equal(f.byId('pay').disabled, false);
  assert.deepEqual(f.operations(), ['status', 'challenge', 'bindPayer']);
  const proof = f.phone[0].calls.find(([name]) => name === 'message')[1];
  assert.equal(new TextDecoder().decode(proof.message), 'MIR2 order-bound wallet confirmation\nUSDC 10.000000\nmerchant fixed');
  assert.deepEqual(f.requests.find(({ body }) => body.operation === 'bindPayer').body, { operation: 'bindPayer', payer: PAYER, signature: SIGNATURE });
  await f.click('pay'); await flush();
  const payment = f.phone[0].calls.find(([name]) => name === 'pay')[1];
  assert.equal(payment.transactionBytesB64, BCS); assert.equal(payment.account.address, PAYER);
  assert.equal('transaction' in payment, false);
  assert.deepEqual(f.requests.find(({ body }) => body.operation === 'submitted').body, { operation: 'submitted', digest: DIGEST });
  assert.match(f.byId('result').textContent, /待核实/); assert.doesNotMatch(f.byId('result').textContent, /^点券已到账/);
  f.updateServer({ chainConfirmed: true, gameDelivered: false }); await f.click('refresh');
  assert.match(f.byId('result').textContent, /链上付款已确认/); assert.doesNotMatch(f.byId('result').textContent, /^点券已到账/);
  f.updateServer({ gameDelivered: true }); await f.click('refresh'); assert.match(f.byId('result').textContent, /^点券已到账/);
});

test('wallet callback fields cannot replace frozen amount, payer, credits or payment state', async () => {
  const f = fixture({ payment: () => ({ digest: DIGEST, amountMicros: '1', payer: OTHER, credits: 999999, gameDelivered: true }) });
  await f.connectPhone(); await f.click('pay');
  assert.match(f.byId('details').textContent, /10\.000000/); assert.match(f.byId('details').textContent, /1000/);
  assert.doesNotMatch(f.byId('details').textContent, /999999/);
  assert.equal(f.server.payer, PAYER); assert.equal(f.server.gameDelivered, false);
  assert.deepEqual(f.requests.find(({ body }) => body.operation === 'submitted').body, { operation: 'submitted', digest: DIGEST });
});

test('missing configuration or SDK produces no fake QR or payment request; extension remains available', async () => {
  for (const options of [{ configured: false }, { missingSdk: true }]) {
    const f = fixture(options); await flush(); await f.click('connect'); await flush();
    assert.equal(f.phone.length, 0); assert.equal(f.qrCalls.length, 0); assert.equal(f.byId('dubhe-panel').hidden, true);
    assert.match(f.byId('result').textContent, /未配置完成/);
    await f.click('pay', true); assert.deepEqual(f.operations(), ['status']);
    await f.changeWallet('extension:0'); await f.click('connect'); await f.click('pay');
    assert.equal(f.operations().includes('buildPay'), true); assert.equal(f.operations().includes('buildPayBytes'), false);
    const payment = f.extension.inputs.find(([name]) => name === 'pay')[1];
    assert.equal(await payment.transaction.toJSON(), '{"version":2,"sender":"fixed-server-payer"}');
  }
});

test('cancel fences a slow QR render and prevents a late phone connection from requesting wallet proof', async () => {
  const qr = deferred(); const f = fixture({ qr: () => qr.promise }); await flush();
  const connecting = f.click('connect'); await flush();
  await f.click('dubhe-cancel'); qr.resolve('data:image/png;base64,T0xE'); await flush();
  assert.equal(f.byId('dubhe-panel').hidden, true); assert.equal(f.byId('dubhe-qr').src, '');
  f.phone[0].approval.resolve({ accounts: [clone(ACCOUNT)] }); await connecting; await flush();
  assert.equal(f.operations().includes('challenge'), false);
  assert.equal(f.byId('pay').disabled, true);
});

test('a wallet switch while payment bytes are loading prevents any wallet payment call', async () => {
  const built = deferred();
  const f = fixture({ intercept: (body) => body.operation === 'buildPayBytes' ? built.promise : undefined });
  await f.connectPhone(); const paying = f.click('pay'); await flush();
  await f.changeWallet('extension:0'); built.resolve({ transactionBytesB64: BCS, state: clone(f.server) });
  await paying; await flush();
  assert.equal(f.phone[0].calls.some(([name]) => name === 'pay'), false);
  assert.equal(f.operations().includes('submitted'), false); assert.doesNotMatch(f.byId('result').textContent, /^点券已到账/);
});

test('an expired, confirmed, delivered or already-submitted build state cannot reach wallet payment', async () => {
  const changes = [
    ['expired', (state) => { state.payExpired = true; }],
    ['confirmed', (state) => { state.chainConfirmed = true; }],
    ['delivered', (state) => { state.gameDelivered = true; }],
    ['paymentSubmitted', (state) => { state.order.state = 'paymentSubmitted'; }],
  ];
  for (const [name, change] of changes) {
    const f = fixture({ intercept: (body, state) => {
      if (body.operation !== 'buildPayBytes') return;
      change(state); return { transactionBytesB64: BCS, state };
    } });
    await f.connectPhone(); await f.click('pay'); await flush();
    assert.equal(f.phone[0].calls.filter(([operation]) => operation === 'pay').length, 0, `${name} must not reach wallet payment`);
    assert.equal(f.operations().includes('submitted'), false);
  }
});

test('an existing server paymentSubmitted hint never permits another payment attempt after refresh', async () => {
  const f = fixture(); await f.connectPhone();
  f.server.order.state = 'paymentSubmitted'; await f.click('refresh');
  assert.equal(f.byId('pay').disabled, true);
  await f.click('pay', true);
  assert.equal(f.operations().includes('buildPayBytes'), false);
  assert.equal(f.phone[0].calls.some(([name]) => name === 'pay'), false);
});

test('rejected or unknown mobile payment never reports credit or permits another wallet payment attempt', async () => {
  for (const payment of [() => Promise.reject(Error('user rejected')), () => ({})]) {
    const f = fixture({ payment }); await f.connectPhone(); await f.click('pay'); await flush();
    assert.match(f.byId('result').textContent, /待核实/); assert.doesNotMatch(f.byId('result').textContent, /^点券已到账/);
    await f.click('pay', true); await f.click('refresh'); await f.click('pay', true);
    assert.equal(f.phone[0].calls.filter(([name]) => name === 'pay').length, 1);
    assert.equal(f.operations().filter((name) => name === 'buildPayBytes').length, 1);
    assert.equal(f.operations().includes('submitted'), false);
  }
});

test('wallet proof rejection stops before binding and payment; disconnect disables payment', async () => {
  const rejected = fixture({ personal: () => Promise.reject(Error('proof rejected')) });
  await rejected.connectPhone(); assert.equal(rejected.operations().includes('bindPayer'), false);
  assert.equal(rejected.byId('pay').disabled, true);
  const f = fixture(); await f.connectPhone(); f.phone[0].callbacks.onDisconnect(); await flush();
  assert.equal(f.byId('pay').disabled, true); await f.click('pay', true);
  assert.equal(f.operations().includes('buildPayBytes'), false);
});

test('expiry invalidates pending connection and clears QR interval without resurrecting it', async () => {
  let reads = 0;
  const f = fixture({ qrLifetime: 1, dateNow: (now) => reads++ ? now + 100 : now }); await flush();
  const connecting = f.click('connect'); await flush();
  assert.equal(f.byId('dubhe-panel').hidden, true);
  assert.equal([...f.intervals.values()].filter((item) => item.milliseconds === 1000).length, 0);
  f.phone[0].approval.reject(Error('expired')); await connecting;
  assert.equal(f.operations().includes('challenge'), false);
});

test('expired adapter callback and late QR from a retired adapter cannot interfere with the next connection', async () => {
  const f = fixture(); await flush(); const oldConnecting = f.click('connect'); await flush();
  const old = f.phone[0]; await f.click('dubhe-cancel'); old.approval.reject(Error('cancelled')); await oldConnecting;
  const newConnecting = f.click('connect'); await flush(); const current = f.phone[1];
  assert.ok(current); assert.equal(f.byId('dubhe-panel').hidden, false);
  old.callbacks.onExpired(); await flush();
  assert.equal(f.byId('dubhe-panel').hidden, false);
  await old.callbacks.onUri('wc:retired-connection', { expiresAt: 1800000060000 }); await flush();
  assert.equal(f.qrCalls.includes('wc:retired-connection'), false);
  current.approval.resolve({ accounts: [clone(ACCOUNT)] }); await newConnecting;
  assert.equal(f.byId('pay').disabled, false);
});

test('invalid checkout capability never starts wallet setup or session operations', async () => {
  const f = fixture({ capability: 'game-session-token-is-not-a-checkout' }); await flush();
  await f.click('connect', true); await f.click('pay', true);
  assert.equal(f.requests.length, 0); assert.equal(f.phone.length, 0); assert.equal(f.qrCalls.length, 0);
});

test('pagehide hides the approval QR and disconnects its phone pairing', async () => {
  const f = fixture(); await flush(); const connecting = f.click('connect'); await flush();
  assert.equal(f.byId('dubhe-panel').hidden, false);
  f.window.dispatchEvent({ type: 'pagehide' }); await flush();
  assert.equal(f.byId('dubhe-panel').hidden, true);
  assert.equal(f.byId('dubhe-qr').src, '');
  assert.equal(f.phone[0].calls.filter(([name]) => name === 'disconnect').length, 1);
  f.phone[0].approval.reject(Error('page left')); await connecting;
  assert.equal(f.operations().includes('challenge'), false);
});
