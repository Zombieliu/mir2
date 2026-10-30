import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.MIR2_PLAYWRIGHT_MODULE || 'playwright');
const source = new URL('../apps/gateway/src/', import.meta.url);
const output = fileURLToPath(new URL('../docs/evidence/usdc-dubhe-qr-20261030/', import.meta.url));
mkdirSync(output, { recursive: true });
const capability = `usdc_${'a'.repeat(64)}.${'b'.repeat(43)}`;
const payer = `0x${'8'.repeat(64)}`;
const uri = `wc:${'c'.repeat(64)}@2?relay-protocol=irn&symKey=${'d'.repeat(64)}&expiryTimestamp=${Math.floor(Date.now() / 1000) + 180}`;
let configured = true;
const state = () => ({ order: { id: `usdc_${'a'.repeat(64)}`, network: 'testnet', amountMicros: '1000000', credits: 100,
  expiresAtMs: Date.now() + 600000, state: 'prepared' }, chain: 'sui:testnet',
  coinType: '0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC',
  merchant: `0x${'9'.repeat(64)}`, decimals: 6, testPayment: true, gasPayer: 'wallet', payer: null,
  invoiceId: null, payExpired: false, chainConfirmed: false, gameDelivered: false,
  dubheWallet: { available: configured, projectId: configured ? '1'.repeat(32) : null, chain: 'sui:testnet' } });
const mock = `\nwindow.Mir2DubheWallet={create(options){let reject;return {
  connect(){options.onUri(${JSON.stringify(uri)},{expiresAt:Date.now()+120000});return new Promise((_,no)=>{reject=no})},
  disconnect(){reject?.(new Error('UI fixture cancelled'))}
}}};`;
const csp = "default-src 'none'; script-src 'self'; connect-src 'self' wss://relay.walletconnect.com https://relay.walletconnect.com; img-src 'self' data:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
const server = createServer(async (req, res) => {
  res.setHeader('Content-Security-Policy', csp); res.setHeader('Cache-Control', 'no-store');
  if (req.url === '/v1/billing/sui-usdc/session') {
    let body = ''; for await (const chunk of req) body += chunk;
    assert.equal(req.headers.authorization, `Bearer ${capability}`);
    assert.equal(JSON.parse(body).operation, 'status', 'Browser fixture never creates orders, signs, or pays');
    res.setHeader('Content-Type', 'application/json'); return res.end(JSON.stringify(state()));
  }
  const assets = { '/v1/billing/sui-usdc/pay.js': 'sui_usdc_checkout.js', '/v1/billing/sui-usdc/dubhe-wallet.js': 'sui_usdc_dubhe_wallet.bundle.js' };
  if (assets[req.url]) {
    res.setHeader('Content-Type', 'text/javascript');
    return res.end(readFileSync(new URL(assets[req.url], source), 'utf8') + (req.url.endsWith('dubhe-wallet.js') ? mock : ''));
  }
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.end(readFileSync(new URL('sui_usdc_checkout.html', source), 'utf8').replace('<body>', '<body><p class="notice">本地界面验收 · 模拟订单和二维码，不能付款</p>'));
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({ headless: true, ...(process.env.MIR2_CHROME_EXECUTABLE ? { executablePath: process.env.MIR2_CHROME_EXECUTABLE } : {}) });
const results = [];
try {
  for (const [name, width] of [['desktop', 1120], ['mobile', 390]]) {
    const context = await browser.newContext({ viewport: { width, height: 950 }, locale: 'zh-CN', timezoneId: 'Asia/Shanghai' });
    const page = await context.newPage(), errors = [], requests = [], sockets = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('request', request => requests.push(new URL(request.url()).origin));
    page.on('websocket', socket => sockets.push(socket.url()));
    await page.goto(`${origin}/v1/billing/sui-usdc/pay#${capability}`);
    await page.waitForFunction(() => document.getElementById('details').textContent.includes('100'));
    assert.equal(await page.locator('#wallets').inputValue(), 'dubhe-qr');
    assert.equal(new URL(page.url()).hash, '');
    await page.locator('#connect').click();
    await page.locator('#dubhe-qr').waitFor({ state: 'visible' });
    assert.match(await page.locator('#dubhe-qr').getAttribute('src'), /^data:image\/png;base64,/);
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
    assert.deepEqual(errors, []);
    assert.ok(requests.every(requestOrigin => requestOrigin === origin));
    assert.deepEqual(sockets, [], 'UI fixtures must never pair with a live wallet relay');
    await page.screenshot({ path: `${output}/checkout-${name}-mock.png`, fullPage: true });
    await page.locator('#dubhe-cancel').click();
    await page.locator('#dubhe-panel').waitFor({ state: 'hidden' });
    results.push({ viewport: name, passed: true, realWalletSignature: false });
    await context.close();
  }
  configured = false;
  const page = await browser.newPage();
  await page.goto(`${origin}/v1/billing/sui-usdc/pay#${capability}`);
  await page.waitForFunction(() => document.getElementById('details').textContent.includes('100'));
  await page.locator('#connect').click();
  await page.waitForFunction(() => document.getElementById('result').textContent.includes('尚未配置完成'));
  assert.equal(await page.locator('#dubhe-panel').isVisible(), false);
  results.push({ configuration: 'missing', noFakeQr: true, passed: true });
  writeFileSync(`${output}/BROWSER-MOCK.json`, JSON.stringify({ results, fixturesOnly: true, realPayment: false }, null, 2));
  console.log(JSON.stringify({ results, output }));
} finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
