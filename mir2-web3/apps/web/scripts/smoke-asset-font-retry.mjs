import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  const name = process.argv[i], value = process.argv[i + 1];
  assert(['--output', '--worker-source', '--chrome', '--playwright-package'].includes(name) && value);
  assert(!args.has(name)); args.set(name, value);
}
assert(args.has('--output'), 'use a fresh evidence directory');
const web = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const output = resolve(args.get('--output'));
await mkdir(output, { recursive: false });
const load = args.has('--playwright-package') ? createRequire(resolve(args.get('--playwright-package'))) : createRequire(import.meta.url);
const { chromium } = load('playwright');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const worker = await readFile(resolve(args.get('--worker-source') ?? join(web, 'public/mir2-asset-worker.js')));
const compile = source => ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const requestModule = compile(await readFile(join(web, 'lib/asset-prewarm-request.ts'), 'utf8'));
const fontModule = compile((await readFile(join(web, 'lib/bevy-runtime-font-prewarm.ts'), 'utf8'))
  .replace('"./asset-prewarm-request"', '"/prewarm-request.mjs"'));
const paths = [...fontModule.matchAll(/"(\/original-ui\/fonts\/[^"\n]+\.(?:ttf|otf))"/g)].map(row => row[1]);
assert.equal(new Set(paths).size, 6);
const fonts = new Map(await Promise.all(paths.map(async path => {
  const bytes = await readFile(join(web, 'public', path.slice(1)));
  return [path, { bytes, sha256: sha(bytes) }];
})));
const selected = '/original-ui/fonts/NotoSansTC.ttf';
const requests = [];
const proof = { kind: 'real-chrome-font-retry-after-mixed-source-failure', passed: false,
  publicNetworkUsed: false, gameAccountUsed: false, workerSha256: sha(worker), requests,
  fontPins: [...fonts].map(([path, row]) => ({ path, bytes: row.bytes.length, sha256: row.sha256 })) };
let browser, server, origin, mirrorHealthy = false;
try {
  server = createServer((request, response) => {
    const path = new URL(request.url, 'http://127.0.0.1').pathname;
    const row = { path, at: Date.now() }; requests.push(row);
    response.setHeader('cache-control', 'no-store');
    response.setHeader('access-control-allow-origin', '*');
    response.once('finish', () => { row.status = response.statusCode; });
    if (path === '/mir2-asset-worker.js') {
      response.writeHead(200, { 'content-type': 'application/javascript', 'service-worker-allowed': '/' }); response.end(worker);
    } else if (path === '/font-prewarm.mjs' || path === '/prewarm-request.mjs') {
      response.writeHead(200, { 'content-type': 'application/javascript' }); response.end(path === '/font-prewarm.mjs' ? fontModule : requestModule);
    } else if (path.startsWith('/mir2/v/current/original-ui/fonts/')) {
      const logical = path.slice('/mir2/v/current'.length), font = fonts.get(logical);
      if (!font) { response.writeHead(404); response.end(); }
      else if (logical === selected && !mirrorHealthy) { response.writeHead(503, { 'retry-after': '1' }); response.end('temporary fixture failure'); }
      else { response.writeHead(200, { 'content-type': logical.endsWith('.otf') ? 'font/otf' : 'font/ttf', 'content-length': font.bytes.length }); response.end(font.bytes); }
    } else if (path.startsWith('/mir2/v/older/original-ui/fonts/')) {
      // The inherited release lacks this file. Its 404 is not evidence that
      // the current-release object is absent. The next logical read recovers.
      mirrorHealthy = true; response.writeHead(404); response.end('inherited copy absent');
    } else if (path.startsWith('/original-ui/fonts/')) {
      response.writeHead(404); response.end('fixture application has no local font');
    } else if (path === '/') {
      response.writeHead(200, { 'content-type': 'text/html' });
      response.end('<!doctype html><title>Original font retry</title><canvas width="640" height="160"></canvas>');
    } else { response.writeHead(404); response.end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port; origin = 'http://127.0.0.1:' + port;
  browser = await chromium.launch({ headless: true, ...(args.has('--chrome') ? { executablePath: resolve(args.get('--chrome')) } : {}) });
  const context = await browser.newContext({ serviceWorkers: 'allow' }), page = await context.newPage();
  const unexpected = [];
  context.on('request', request => { const url = new URL(request.url()); if (!['127.0.0.1', 'localhost'].includes(url.hostname)) unexpected.push(url.origin + url.pathname); });
  await page.goto(origin, { waitUntil: 'domcontentloaded' });
  proof.prewarm = await page.evaluate(async ({ origin, port }) => {
    await navigator.serviceWorker.register('/mir2-asset-worker.js'); await navigator.serviceWorker.ready;
    if (!navigator.serviceWorker.controller) await new Promise(resolve => navigator.serviceWorker.addEventListener('controllerchange', resolve, { once: true }));
    await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(Error('configuration acknowledgement missing')), 5000);
      const onMessage = event => { if (event.data?.type !== 'MIR2_ASSET_CACHE_CONFIGURED') return;
        clearTimeout(timeout); navigator.serviceWorker.removeEventListener('message', onMessage); resolve(); };
      navigator.serviceWorker.addEventListener('message', onMessage);
      const base = 'http://localhost:' + port + '/mir2/v/';
      navigator.serviceWorker.controller.postMessage({ type: 'MIR2_ASSET_CACHE_CONFIG', manifestVersion: 'current', cacheNamespace: 'font-retry', fontMirrorFirst: true,
        manifest: { version: 'current', remoteAssets: { enabled: true, assetBaseUrl: base + 'current', browserFallbackBaseUrls: [base + 'current', base + 'older'] },
          resourcePacks: [{ cacheTier: 'critical', urls: ['/original-ui/fonts/NotoSansTC.ttf'] }] } });
    });
    const { prewarmBevyRuntimeFonts } = await import('/font-prewarm.mjs');
    const started = performance.now();
    const result = await prewarmBevyRuntimeFonts();
    return { ...result, elapsedMs: performance.now() - started };
  }, { origin, port });
  proof.selectedFont = await page.evaluate(async selected => {
    try {
      const face = new FontFace('R16Original', `url(${selected})`); await face.load(); document.fonts.add(face);
      const context = document.querySelector('canvas').getContext('2d');
      context.fillStyle = '#15110b'; context.fillRect(0, 0, 640, 160);
      context.font = '28px R16Original'; context.fillStyle = '#efcb78'; context.fillText('比奇省 · 冒險日記 · 阿洛', 20, 80);
      return { loaded: face.status === 'loaded' };
    } catch (error) { return { loaded: false, errorName: error.name }; }
  }, selected);
  assert.equal(unexpected.length, 0); proof.unexpectedNetworkOrigins = unexpected;
  assert.equal(proof.prewarm.completed.length, 6); assert.equal(proof.prewarm.failed.length, 0);
  assert.equal(proof.selectedFont.loaded, true);
  proof.cache = await page.evaluate(async pins => {
    const rows = [];
    for (const pin of pins) {
      const response = await caches.match(location.origin + pin.path); if (!response) { rows.push({ path: pin.path, present: false }); continue; }
      const bytes = await response.arrayBuffer();
      rows.push({ path: pin.path, present: true, bytes: bytes.byteLength, sha256: [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(x => x.toString(16).padStart(2, '0')).join('') });
    }
    return rows;
  }, proof.fontPins);
  for (const pin of proof.fontPins) assert.deepEqual(proof.cache.find(row => row.path === pin.path), { ...pin, present: true });
  const attempts = requests.filter(row => row.path === '/mir2/v/current' + selected);
  assert.deepEqual(attempts.map(row => row.status), [503, 200], 'one failed attempt, then the next logical request uses the recovered current source');
  assert.equal(requests.filter(row => row.path === '/mir2/v/older' + selected).length, 1);
  await page.screenshot({ path: join(output, 'original-font-recovered.png') });
  proof.passed = true;
} catch (error) {
  proof.failure = { name: error.name, message: error.message };
} finally {
  await browser?.close();
  if (server) { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
  proof.browserClosed = true; proof.serverClosed = true;
  await writeFile(join(output, 'receipt.json'), JSON.stringify(proof, null, 2) + '\n', { flag: 'wx' });
}
console.log(JSON.stringify({ passed: proof.passed, failure: proof.failure, completedFonts: proof.prewarm?.completed.length, selectedFontLoaded: proof.selectedFont?.loaded, output }));
if (!proof.passed) process.exitCode = 1;
