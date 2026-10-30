import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.MIR2_PLAYWRIGHT_MODULE || 'playwright');
const source = new URL('../apps/gateway/src/', import.meta.url);
const gatewayRequire = createRequire(new URL('../apps/gateway/package.json', import.meta.url));
const QRCode = gatewayRequire('qrcode');
const output = fileURLToPath(new URL('../docs/evidence/usdc-dubhe-qr-20261030/', import.meta.url));
mkdirSync(output, { recursive: true });
const previewText = 'MIR2_UI_PREVIEW_NOT_A_PAYMENT';
// Both screenshot QR paths use this one harmless image. A dummy merchant must
// never become a scannable payment request in retained browser evidence.
const previewPng = await QRCode.toDataURL(previewText, { width: 320, margin: 3, errorCorrectionLevel: 'M' });
const capability = `usdc_${'a'.repeat(64)}.${'b'.repeat(43)}`;
const payer = `0x${'8'.repeat(64)}`;
const merchant = `0x${'9'.repeat(64)}`;
const coinType = '0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC';
const uri = `wc:${'c'.repeat(64)}@2?relay-protocol=irn&symKey=${'d'.repeat(64)}&expiryTimestamp=${Math.floor(Date.now() / 1000) + 180}`;
let configured = true;
let currentState;
let statusError = false;
const makeState = (mode = 'invoice', expiresAtMs = Date.now() + 600000) => {
  const state = { order: { id: `usdc_${'a'.repeat(64)}`, network: 'testnet', amountMicros: '1000000', credits: 100,
  expiresAtMs, state: 'prepared' }, chain: 'sui:testnet', coinType,
  merchant, decimals: 6, testPayment: true, gasPayer: 'wallet', payer: null,
  invoiceId: null, payExpired: false, chainConfirmed: false, gameDelivered: false,
  dubheWallet: { available: configured, projectId: configured ? '1'.repeat(32) : null, chain: 'sui:testnet' } };
  if (mode === 'direct_transfer') {
    const url = new URL('https://wallet.dubhe.obelisk.build/send');
    url.search = new URLSearchParams({ recipient: merchant, coinType, amount: '1.000000', requestId: state.order.id, request: '1' }).toString();
    Object.assign(state, { paymentMode: mode, payer,
      directTransfer: { available: true, payerVerified: true, payer, merchant, chain: state.chain,
        coinType, amountMicros: state.order.amountMicros, expiresAtMs, walletRequestUrl: url.toString() } });
  }
  return state;
};
const mock = `\n(() => {
const fixture=window.__mir2UiFixture;
if (!window.Mir2DubheQr?.toPaymentDataURL || !window.Mir2DirectTransferCard?.create) throw Error('Build the checkout bundle with the direct card before browser acceptance');
window.Mir2DubheQr=Object.freeze({
  toDataURL(value){fixture.qrCalls.push({kind:'wallet-connect',input:value});return Promise.resolve(${JSON.stringify(previewPng)})},
  toPaymentDataURL(value){fixture.qrCalls.push({kind:'direct-transfer',input:value});return Promise.resolve(${JSON.stringify(previewPng)})}
});
window.Mir2DubheWallet={create(options){let reject;return {
  connect(){options.onUri(${JSON.stringify(uri)},{expiresAt:Date.now()+120000});return new Promise((_,no)=>{reject=no})},
  disconnect(){reject?.(new Error('UI fixture cancelled'))}
}}};
})();`;
const csp = "default-src 'none'; script-src 'self'; connect-src 'self' wss://relay.walletconnect.com https://relay.walletconnect.com; img-src 'self' data:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
const serverErrors = [], operations = [];
const server = createServer(async (req, res) => {
  try {
    res.setHeader('Content-Security-Policy', csp); res.setHeader('Cache-Control', 'no-store');
    if (req.url === '/v1/billing/sui-usdc/session') {
      let body = ''; for await (const chunk of req) body += chunk;
      assert.equal(req.method, 'POST');
      assert.equal(req.headers.authorization, `Bearer ${capability}`);
      assert.deepEqual(JSON.parse(body), { operation: 'status' }, 'Browser fixture never creates orders, signs, or pays');
      operations.push('status');
      if (statusError) {
        res.statusCode = 503; res.setHeader('Content-Type', 'application/json');
        return res.end(JSON.stringify({ error: 'UI fixture status temporarily unavailable' }));
      }
      res.setHeader('Content-Type', 'application/json'); return res.end(JSON.stringify(currentState));
    }
    const assets = { '/v1/billing/sui-usdc/pay.js': 'sui_usdc_checkout.js', '/v1/billing/sui-usdc/dubhe-wallet.js': 'sui_usdc_dubhe_wallet.bundle.js' };
    if (assets[req.url]) {
      res.setHeader('Content-Type', 'text/javascript');
      return res.end(readFileSync(new URL(assets[req.url], source), 'utf8') + (req.url.endsWith('dubhe-wallet.js') ? mock : ''));
    }
    if (req.url !== '/v1/billing/sui-usdc/pay') { res.statusCode = 404; return res.end('Not found'); }
    res.setHeader('Content-Type', 'text/html; charset=utf-8');
    const watermark = '<aside id="fixture-watermark" style="background:#ffe099;color:#201700;border:3px solid #b98c24;border-radius:6px;padding:16px;font-weight:800;font-size:18px;text-align:center">本地界面验收 · 模拟订单和二维码，不能付款</aside>';
    res.end(readFileSync(new URL('sui_usdc_checkout.html', source), 'utf8').replace('<body>', `<body>${watermark}`));
  } catch (error) {
    serverErrors.push(error.message); res.statusCode = 500; res.end(JSON.stringify({ error: 'UI fixture rejected an unexpected request' }));
  }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({ headless: true, ...(process.env.MIR2_CHROME_EXECUTABLE ? { executablePath: process.env.MIR2_CHROME_EXECUTABLE } : {}) });
const results = [];

async function openFixture(width = 1120) {
  const context = await browser.newContext({ viewport: { width, height: 950 }, locale: 'zh-CN', timezoneId: 'Asia/Shanghai' });
  const page = await context.newPage(), errors = [], requests = [], sockets = [];
  await page.addInitScript(() => {
    window.__mir2UiFixture = { qrCalls: [], copied: [], clipboardReject: false };
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: {
      async writeText(value) {
        const fixture = window.__mir2UiFixture;
        fixture.copied.push(value);
        if (fixture.clipboardReject) throw Error('Local fixture clipboard denied');
      },
    } });
  });
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', request => requests.push(request.url()));
  page.on('websocket', socket => sockets.push(socket.url()));
  // Fail closed if a future fixture change accidentally tries a live service.
  await page.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort());
  await page.goto(`${origin}/v1/billing/sui-usdc/pay#${capability}`);
  await page.waitForFunction(() => document.getElementById('details').textContent.includes('100'));
  assert.equal(new URL(page.url()).hash, '');
  assert.equal(await page.locator('#fixture-watermark').textContent(), '本地界面验收 · 模拟订单和二维码，不能付款');
  return { context, page, errors, requests, sockets };
}

async function assertLocalOnly(fixture) {
  assert.deepEqual(fixture.errors, []);
  assert.ok(fixture.requests.every(url => new URL(url).origin === origin));
  assert.ok(fixture.requests.every(url => !url.includes(capability)), 'Capability must never enter a request URL');
  assert.deepEqual(fixture.sockets, [], 'UI fixtures must never pair with a live wallet relay');
  assert.deepEqual(serverErrors, []);
  assert.ok(await fixture.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
}

async function assertInactiveDirect(page) {
  assert.equal(await page.locator('#direct-transfer-qr').isVisible(), false);
  assert.equal(await page.locator('#direct-transfer-qr').getAttribute('src'), null);
  assert.equal(await page.locator('#copy-direct-address').isDisabled(), true);
  assert.equal(await page.locator('#direct-transfer-address').inputValue(), '');
  assert.equal(await page.locator('#direct-transfer-status').textContent(), '直转自动到账接入中，请勿付款');
}

try {
  for (const [name, width] of [['desktop', 1120], ['mobile', 390]]) {
    currentState = makeState();
    const fixture = await openFixture(width), { page } = fixture;
    await assertInactiveDirect(page);
    assert.equal(await page.locator('#wallets').inputValue(), 'dubhe-qr');
    await page.locator('#connect').click();
    await page.locator('#dubhe-qr').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#dubhe-qr').getAttribute('src'), previewPng);
    assert.deepEqual(await page.evaluate(() => window.__mir2UiFixture.qrCalls), [{ kind: 'wallet-connect', input: uri }]);
    await assertLocalOnly(fixture);
    await page.screenshot({ path: `${output}/checkout-${name}-mock.png`, fullPage: true });
    await page.locator('#dubhe-cancel').click();
    await page.locator('#dubhe-panel').waitFor({ state: 'hidden' });
    results.push({ mode: 'invoice-wallet-connect', viewport: name, passed: true, directTransferDisabled: true, qrPreviewText: previewText, realWalletSignature: false });
    await fixture.context.close();
  }
  configured = false;
  currentState = makeState();
  const missing = await openFixture();
  await missing.page.locator('#connect').click();
  await missing.page.waitForFunction(() => document.getElementById('result').textContent.includes('尚未配置完成'));
  assert.equal(await missing.page.locator('#dubhe-panel').isVisible(), false);
  await assertInactiveDirect(missing.page);
  assert.deepEqual(await missing.page.evaluate(() => window.__mir2UiFixture.qrCalls), []);
  await assertLocalOnly(missing);
  results.push({ configuration: 'missing', noFakeQr: true, passed: true });
  await missing.context.close();

  for (const [name, width] of [['desktop', 1120], ['mobile', 390]]) {
    currentState = makeState('direct_transfer');
    const expectedRequest = currentState.directTransfer.walletRequestUrl;
    const fixture = await openFixture(width), { page } = fixture;
    await page.locator('#direct-transfer-qr').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#direct-transfer-qr').getAttribute('src'), previewPng);
    assert.deepEqual(await page.evaluate(() => window.__mir2UiFixture.qrCalls), [{ kind: 'direct-transfer', input: expectedRequest }]);
    assert.equal(await page.locator('#direct-transfer-placeholder').isVisible(), false);
    assert.equal(await page.locator('#direct-transfer-address').inputValue(), merchant);
    assert.equal(await page.locator('#direct-transfer-address').getAttribute('readonly'), '');
    assert.equal(await page.locator('#direct-transfer-amount').textContent(), '1.000000 测试 USDC');
    assert.equal(await page.locator('#direct-transfer-payer').textContent(), payer);
    assert.equal(await page.locator('#connect').isDisabled(), true, 'Direct orders cannot use the compatibility payment flow');
    assert.equal(await page.locator('#pay').isDisabled(), true);
    await page.locator('#copy-direct-address').click();
    await page.waitForFunction(() => document.getElementById('direct-transfer-copy-status').textContent.includes('收款地址已复制'));
    assert.deepEqual(await page.evaluate(() => window.__mir2UiFixture.copied), [merchant]);
    await assertLocalOnly(fixture);
    await page.screenshot({ path: `${output}/direct-card-${name}-mock.png`, fullPage: true });

    await page.evaluate(() => { window.__mir2UiFixture.clipboardReject = true; });
    await page.locator('#copy-direct-address').click();
    await page.waitForFunction(() => document.getElementById('direct-transfer-copy-status').textContent.includes('请手动复制'));
    assert.deepEqual(await page.locator('#direct-transfer-address').evaluate(input => ({ focused: document.activeElement === input, start: input.selectionStart, end: input.selectionEnd, value: input.value })),
      { focused: true, start: 0, end: merchant.length, value: merchant });
    assert.deepEqual(await page.evaluate(() => window.__mir2UiFixture.copied), [merchant, merchant]);
    await assertLocalOnly(fixture);
    results.push({ mode: 'direct_transfer', viewport: name, passed: true, clipboardMock: true, addressCopied: true,
      manualCopyFallback: true, compatibilityPaymentDisabled: true, paymentRequestInput: expectedRequest, qrPreviewText: previewText, realPayment: false });
    await fixture.context.close();
  }

  currentState = makeState('direct_transfer');
  const unavailable = await openFixture(), { page: unavailablePage } = unavailable;
  await unavailablePage.locator('#direct-transfer-qr').waitFor({ state: 'visible' });
  statusError = true;
  await unavailablePage.locator('#refresh').click();
  await unavailablePage.waitForFunction(() => document.getElementById('result').textContent.includes('UI fixture status temporarily unavailable'));
  await assertInactiveDirect(unavailablePage);
  await assertLocalOnly(unavailable);
  results.push({ mode: 'direct_transfer', statusQueryFailed: true, stalePaymentQrRemoved: true, copyDisabled: true, passed: true });
  await unavailable.context.close();
  statusError = false;

  currentState = makeState('direct_transfer', Date.now() + 5000);
  const expiring = await openFixture(390), { page: expiryPage } = expiring;
  await expiryPage.locator('#direct-transfer-qr').waitFor({ state: 'visible' });
  assert.equal(await expiryPage.locator('#copy-direct-address').isEnabled(), true);
  await expiryPage.waitForFunction(() => document.getElementById('copy-direct-address').disabled, null, { timeout: 8000 });
  assert.equal(await expiryPage.locator('#direct-transfer-qr').isVisible(), false);
  assert.equal(await expiryPage.locator('#direct-transfer-qr').getAttribute('src'), null);
  assert.equal(await expiryPage.locator('#direct-transfer-address').inputValue(), '');
  assert.match(await expiryPage.locator('#direct-transfer-status').textContent(), /付款期限已过/);
  assert.deepEqual(await expiryPage.evaluate(() => window.__mir2UiFixture.copied), []);
  await assertLocalOnly(expiring);
  results.push({ mode: 'direct_transfer', automaticExpiryDisabled: true, qrRemoved: true, addressCleared: true, passed: true });
  await expiring.context.close();

  assert.ok(operations.length >= 6 && operations.every(operation => operation === 'status'));
  writeFileSync(`${output}/BROWSER-MOCK.json`, JSON.stringify({ results, fixturesOnly: true, realPayment: false,
    qrPreviewText: previewText, clipboardIsMock: true, serverOperations: operations }, null, 2));
  console.log(JSON.stringify({ results, output }));
} finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
