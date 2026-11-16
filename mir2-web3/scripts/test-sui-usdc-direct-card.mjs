import assert from 'node:assert/strict';
import test from 'node:test';
import { createDirectTransferCard } from '../apps/gateway/src/sui_usdc_direct_card.js';

// Exercise the actual controller. DOM, clipboard and QR rendering are the only
// substitutes; this suite does not claim to verify a wallet or an on-chain pay.
const PAYER = `0x${'1'.repeat(64)}`;
const MERCHANT = `0x${'2'.repeat(64)}`;
const OTHER = `0x${'3'.repeat(64)}`;
const USDC = '0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC';
const ORDER = `usdc_${'a'.repeat(64)}`;
const START = 1800000000000;
const IMAGE = 'data:image/png;base64,TE9DQUw=';
const IDS = ['direct-transfer-panel', 'direct-transfer-qr', 'direct-transfer-placeholder',
  'direct-transfer-address', 'copy-direct-address', 'direct-transfer-amount',
  'direct-transfer-payer', 'direct-transfer-copy-status', 'direct-transfer-status'];
const flush = async () => { for (let i = 0; i < 24; i++) await Promise.resolve(); };
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function paymentUrl({ recipient = MERCHANT, amount = '10.000000', requestId = ORDER } = {}) {
  const url = new URL('/send', 'https://wallet.dubhe.obelisk.build');
  for (const [key, value] of Object.entries({ recipient, coinType: USDC, amount, requestId, request: '1' })) url.searchParams.set(key, value);
  return url.toString();
}
function state() {
  return { paymentMode: 'direct_transfer', payer: PAYER, merchant: MERCHANT, chain: 'sui:testnet', coinType: USDC,
    decimals: 6, testPayment: true, gasPayer: 'wallet', chainConfirmed: false, gameDelivered: false, payExpired: false,
    order: { id: ORDER, network: 'testnet', amountMicros: '10000000', expiresAtMs: START + 60000, state: 'awaitingPayment' },
    directTransfer: { available: true, payerVerified: true, payer: PAYER, merchant: MERCHANT, chain: 'sui:testnet',
      coinType: USDC, amountMicros: '10000000', expiresAtMs: START + 60000, walletRequestUrl: paymentUrl() } };
}
class Element {
  constructor() { this.hidden = false; this.disabled = false; this.value = ''; this.src = ''; this.textContent = ''; this.readOnly = false; this.focused = 0; this.selected = 0; this.listeners = new Map(); }
  setAttribute(name, value) { if (name === 'readonly') this.readOnly = true; else this[name] = String(value); }
  removeAttribute(name) { if (name === 'src') this.src = ''; }
  addEventListener(name, listener) { this.listeners.set(name, listener); }
  removeEventListener(name, listener) { if (this.listeners.get(name) === listener) this.listeners.delete(name); }
  focus() { this.focused++; }
  select() { this.selected++; }
  setSelectionRange(start, end) { this.selection = [start, end]; this.selected++; }
}
function fixture(t, options = {}) {
  const elements = new Map(IDS.map((id) => [id, new Element()]));
  const byId = (id) => { assert.ok(elements.has(id), `Unexpected direct-card DOM id: ${id}`); return elements.get(id); };
  byId('direct-transfer-qr').hidden = true;
  byId('copy-direct-address').disabled = true;
  byId('direct-transfer-address').readOnly = true;
  const copies = [], qrCalls = [], errors = [];
  let clock = START;
  const card = createDirectTransferCard({
    document: { getElementById: byId },
    clipboard: { writeText: async (value) => { copies.push(value); if (options.copy) return options.copy(value); } },
    renderQr: async (value) => { qrCalls.push(value); return options.qr ? options.qr(value) : IMAGE; },
    now: () => clock,
  });
  const observe = (value) => { if (value?.then) value.catch((error) => errors.push(error)); };
  t.after(() => { card.dispose(); assert.deepEqual(errors, []); });
  return { card, byId, copies, qrCalls,
    setNow: (value) => { clock = value; },
    show: async (value) => { observe(card.show(value)); await flush(); },
    copy: async (force = false) => {
      const button = byId('copy-direct-address');
      if (force || !button.disabled) await (button.onclick ?? button.listeners.get('click'))?.({ preventDefault() {} });
      await flush();
    } };
}
function assertInactive(f) {
  assert.equal(f.byId('copy-direct-address').disabled, true);
  assert.equal(f.byId('direct-transfer-qr').hidden, true);
  assert.equal(f.byId('direct-transfer-qr').src, '');
}

test('only explicit verified direct-transfer readiness enables a real QR or address copying', async (t) => {
  for (const change of [s => delete s.paymentMode, s => { s.paymentMode = 'invoice'; }, s => delete s.directTransfer,
    s => { s.directTransfer.available = false; }, s => { s.directTransfer.available = 'true'; },
    s => { s.directTransfer.payerVerified = false; }, s => { s.directTransfer.payerVerified = 'true'; }]) {
    const f = fixture(t), s = state(); change(s); await f.show(s); await f.copy(true);
    assertInactive(f); assert.deepEqual(f.qrCalls, []); assert.deepEqual(f.copies, []);
    assert.equal(f.byId('direct-transfer-address').selected, 0);
  }
});

test('ready card renders only the official payment URL and copies only the raw merchant address', async (t) => {
  const f = fixture(t), s = state(); await f.show(s);
  assert.equal(f.byId('direct-transfer-panel').hidden, false);
  assert.equal(f.byId('direct-transfer-qr').hidden, false); assert.equal(f.byId('direct-transfer-qr').src, IMAGE);
  assert.equal(f.byId('direct-transfer-address').value, MERCHANT); assert.equal(f.byId('direct-transfer-address').readOnly, true);
  assert.equal(f.byId('copy-direct-address').disabled, false);
  assert.match(f.byId('direct-transfer-amount').textContent, /10\.000000/);
  assert.ok(f.byId('direct-transfer-payer').textContent.includes(PAYER));
  assert.deepEqual(f.qrCalls, [s.directTransfer.walletRequestUrl]);
  f.byId('direct-transfer-address').value = `${OTHER}?amount=999&requestId=attacker`;
  await f.copy(); assert.deepEqual(f.copies, [MERCHANT]);
  assert.ok(f.byId('direct-transfer-copy-status').textContent.length);
});

test('every direct-transfer identity, amount and expiry must match the main server order', async (t) => {
  const changes = [s => { s.payer = null; }, s => { s.directTransfer.payer = OTHER; }, s => { s.directTransfer.merchant = OTHER; },
    s => { s.directTransfer.chain = 'sui:mainnet'; }, s => { s.directTransfer.coinType = '0x2::sui::SUI'; },
    s => { s.directTransfer.amountMicros = '1000000'; }, s => { s.directTransfer.expiresAtMs++; },
    s => { s.directTransfer.payer = '0x1'; }, s => { s.directTransfer.merchant = `0X${'2'.repeat(64)}`; },
    s => { s.chain = 'sui:mainnet'; }, s => { s.coinType = '0x2::sui::SUI'; },
    s => { s.merchant = OTHER; }, s => { s.order.id = `usdc_${'b'.repeat(64)}`; },
    s => { s.payer = s.directTransfer.payer = `0x${'0'.repeat(64)}`; },
    s => { s.merchant = s.directTransfer.merchant = `0x${'0'.repeat(64)}`; s.directTransfer.walletRequestUrl = paymentUrl({ recipient: s.merchant }); },
    s => { s.payer = s.directTransfer.payer = MERCHANT; }];
  for (const change of changes) {
    const f = fixture(t), s = state(); change(s); await f.show(s); await f.copy(true);
    assertInactive(f); assert.deepEqual(f.qrCalls, []); assert.deepEqual(f.copies, []);
  }
});

test('official URL rejects foreign origins, paths, duplicate or unknown fields and non-exact values', async (t) => {
  const changes = [u => { u.hostname = 'wallet.dubhe.obelisk.build.evil.invalid'; }, u => { u.protocol = 'http:'; },
    u => { u.username = 'person'; }, u => { u.hash = 'receipt'; }, u => { u.pathname = '/send/'; },
    u => { u.searchParams.set('bytes', 'arbitrary'); }, u => { u.searchParams.set('note', 'extra'); },
    u => { u.searchParams.append('recipient', MERCHANT); }, u => { u.searchParams.append('amount', '10.000000'); },
    u => { u.searchParams.set('recipient', OTHER); }, u => { u.searchParams.set('coinType', '0x2::sui::SUI'); },
    u => { u.searchParams.set('request', '0'); }, u => { u.searchParams.delete('request'); },
    u => { u.searchParams.set('requestId', 'different-order'); }, u => { u.searchParams.delete('requestId'); },
    ...['10', '10.0', '010.000000', '1e1', '10.000001'].map(amount => u => u.searchParams.set('amount', amount))];
  for (const change of changes) {
    const f = fixture(t), s = state(), url = new URL(s.directTransfer.walletRequestUrl);
    change(url); s.directTransfer.walletRequestUrl = url.toString(); await f.show(s); await f.copy(true);
    assertInactive(f); assert.deepEqual(f.qrCalls, []); assert.deepEqual(f.copies, []);
  }
  // URL.hash is empty for a trailing '#'; test the original string directly.
  const f = fixture(t), s = state(); s.directTransfer.walletRequestUrl += '#';
  await f.show(s); await f.copy(true); assertInactive(f); assert.deepEqual(f.qrCalls, []); assert.deepEqual(f.copies, []);
});

test('micros stay canonical and precise beyond the JavaScript safe-integer range', async (t) => {
  for (const value of ['0', '-1', '01000000', '10000000.0', '1e7', 10000000]) {
    const f = fixture(t), s = state(); s.order.amountMicros = s.directTransfer.amountMicros = value;
    await f.show(s); assertInactive(f); assert.deepEqual(f.qrCalls, []);
  }
  for (const [micros, amount] of [['9007199254740993', '9007199254.740993'], ['18446744073709551615', '18446744073709.551615']]) {
    const f = fixture(t), s = state(); s.order.amountMicros = s.directTransfer.amountMicros = micros;
    s.directTransfer.walletRequestUrl = paymentUrl({ amount });
    await f.show(s); assert.equal(f.byId('copy-direct-address').disabled, false);
    assert.ok(f.byId('direct-transfer-amount').textContent.includes(amount));
    assert.deepEqual(f.qrCalls, [s.directTransfer.walletRequestUrl]);
  }
  const overflow = fixture(t), s = state(); s.order.amountMicros = s.directTransfer.amountMicros = '18446744073709551616';
  s.directTransfer.walletRequestUrl = paymentUrl({ amount: '18446744073709.551616' });
  await overflow.show(s); await overflow.copy(true); assertInactive(overflow);
  assert.deepEqual(overflow.qrCalls, []); assert.deepEqual(overflow.copies, []);
});

test('terminal, expired or unsafe-expiry orders never enable a payment QR or copying', async (t) => {
  for (const change of [s => { s.invoiceId = `0x${'4'.repeat(64)}`; }, s => { s.order.transactionDigest = 'alreadySubmitted'; },
    s => { s.payExpired = true; }, s => { s.chainConfirmed = true; }, s => { s.gameDelivered = true; },
    ...['paymentSubmitted', 'applied', 'failed', 'unknown'].map(value => s => { s.order.state = value; }),
    ...['payExpired', 'chainConfirmed', 'gameDelivered'].flatMap(key => [
      s => { delete s[key]; }, ...[null, 'false', 0].map(value => s => { s[key] = value; }),
    ]),
    s => { s.order.expiresAtMs = s.directTransfer.expiresAtMs = START; },
    ...[Number.MAX_SAFE_INTEGER + 1, START + 0.5, String(START + 60000)].map(value => s => { s.order.expiresAtMs = s.directTransfer.expiresAtMs = value; })]) {
    const f = fixture(t), s = state(); change(s); await f.show(s); await f.copy(true);
    assertInactive(f); assert.deepEqual(f.qrCalls, []); assert.deepEqual(f.copies, []);
  }
  const f = fixture(t), s = state(); await f.show(s); f.setNow(s.order.expiresAtMs); await f.copy(true);
  assert.deepEqual(f.copies, []); assertInactive(f);
});

test('clipboard failure offers manual address selection only while this order remains ready', async (t) => {
  const f = fixture(t, { copy: async () => { throw Error('clipboard unavailable'); } }); await f.show(state()); await f.copy();
  assert.deepEqual(f.copies, [MERCHANT]); assert.ok(f.byId('direct-transfer-address').selected > 0);
  assert.ok(f.byId('direct-transfer-copy-status').textContent.length);
  const s = state(); s.directTransfer.available = false; await f.show(s);
  const selected = f.byId('direct-transfer-address').selected; await f.copy(true);
  assert.equal(f.byId('direct-transfer-address').selected, selected); assert.deepEqual(f.copies, [MERCHANT]); assertInactive(f);
});

test('a delayed clipboard failure cannot select an address after readiness is revoked or the card is disposed', async (t) => {
  for (const dispose of [false, true]) {
    const pending = deferred(), f = fixture(t, { copy: () => pending.promise }); await f.show(state());
    const copying = f.copy(); await flush();
    if (dispose) f.card.dispose(); else { const s = state(); s.payExpired = true; await f.show(s); }
    pending.reject(Error('late clipboard failure')); await copying;
    assert.equal(f.byId('direct-transfer-address').selected, 0); assertInactive(f);
  }
});

test('late QR results cannot overwrite another order, expire into view or resurrect a disposed card', async (t) => {
  const old = deferred(), next = deferred(); let calls = 0;
  const f = fixture(t, { qr: () => ++calls === 1 ? old.promise : next.promise }), a = state(), b = state();
  await f.show(a); b.order.id = `usdc_${'b'.repeat(64)}`; b.directTransfer.walletRequestUrl = paymentUrl({ requestId: b.order.id });
  await f.show(b); next.resolve(IMAGE); await flush(); old.resolve('data:image/png;base64,T0xE'); await flush();
  assert.equal(f.byId('direct-transfer-qr').src, IMAGE);
  const late = deferred(), expired = fixture(t, { qr: () => late.promise }), s = state();
  await expired.show(s); expired.setNow(s.order.expiresAtMs); late.resolve(IMAGE); await flush(); assertInactive(expired);
  const pending = deferred(), disposed = fixture(t, { qr: () => pending.promise }); await disposed.show(state());
  disposed.card.dispose(); pending.resolve(IMAGE); await flush(); await disposed.copy(true); assertInactive(disposed); assert.deepEqual(disposed.copies, []);
});

test('readiness revocation fences pending QR work; render failures never produce a fake QR', async (t) => {
  const pending = deferred(), f = fixture(t, { qr: () => pending.promise }); await f.show(state());
  const s = state(); delete s.directTransfer; await f.show(s); pending.resolve(IMAGE); await flush();
  assertInactive(f); await f.copy(true); assert.deepEqual(f.copies, []);
  const failed = fixture(t, { qr: async () => { throw Error('QR render unavailable'); } }); await failed.show(state());
  assert.equal(failed.byId('direct-transfer-qr').hidden, true); assert.equal(failed.byId('direct-transfer-qr').src, '');
  for (const image of ['data:image/svg+xml;base64,PHN2Zy8+', 'https://evil.invalid/qr.png']) {
    const unsafe = fixture(t, { qr: async () => image }); await unsafe.show(state());
    assert.equal(unsafe.byId('direct-transfer-qr').hidden, true); assert.equal(unsafe.byId('direct-transfer-qr').src, '');
  }
});
