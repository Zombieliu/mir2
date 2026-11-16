// Real Chrome + loopback bodies. This is not a public-game performance receipt.
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import ts from 'typescript';

assert(process.argv.length === 3 && process.env.MIR2_CHROME_BIN && process.env.MIR2_PLAYWRIGHT_PACKAGE);
const root = resolve(process.argv[2]); await mkdir(root, { recursive: false });
const { chromium } = createRequire(resolve(process.env.MIR2_PLAYWRIGHT_PACKAGE))('playwright');
const sources = {}, hashes = {};
for (const name of ['initializer', 'preparation']) {
  const source = await readFile(new URL('../lib/bevy-runtime-' + name + '.ts', import.meta.url), 'utf8');
  hashes[name] = createHash('sha256').update(source).digest('hex');
  sources['/' + name + '.js'] = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
}
const binary = Buffer.from('0061736d010000000105016000017f03020100070a0106616e7377657200000a06010400412a0b', 'hex');
const requests = [], held = new Map(), aborted = [];
const server = createServer((req, res) => {
  const url = new URL(req.url, 'http://fixture');
  if (sources[url.pathname]) { res.writeHead(200, { 'content-type': 'application/javascript' }); res.end(sources[url.pathname]); return; }
  const match = url.pathname.match(/^\/(inflight|complete|cancel-selection|cancel-game|invalid|changed)\/(pinned|fresh)\.wasm$/);
  if (match) {
    const [, name, kind] = match; requests.push({ name, kind, at: Date.now() });
    res.once('close', () => { if (!res.writableEnded) aborted.push({ name, kind }); });
    res.writeHead(200, { 'content-type': 'application/wasm', 'cache-control': 'no-store' });
    if (name === 'invalid') { res.end('invalid-wasm'); return; }
    if (name === 'complete' || kind === 'fresh') { res.end(binary); return; }
    if (name !== 'cancel-selection') { res.flushHeaders(); res.write(binary.subarray(0, 8)); }
    held.set(name, () => { if (!res.destroyed) res.end(name === 'cancel-selection' ? binary : binary.subarray(8)); }); return;
  }
  if (url.pathname === '/finish') { held.get(url.searchParams.get('name'))?.(); res.end('ok'); return; }
  if (url.pathname === '/observed') { res.setHeader('content-type', 'application/json'); res.end(JSON.stringify(requests.filter(r => r.name === url.searchParams.get('name')))); return; }
  res.setHeader('content-type', 'text/html'); res.end('<!doctype html><title>Runtime preparation transport fixture</title>');
});
let context;
const proof = { passed: false, scope: 'Actual Chrome, immutable WASM compilation and selection-to-game transfer over loopback; no game account, production timing, GPU or whole-game acceptance', hashes, closed: {} };
try {
  await new Promise((ok, fail) => { server.once('error', fail); server.listen(0, '127.0.0.1', ok); });
  const origin = 'http://127.0.0.1:' + server.address().port;
  context = await chromium.launchPersistentContext(root + '/profile', { executablePath: process.env.MIR2_CHROME_BIN, headless: true });
  const page = context.pages()[0], origins = new Set(), errors = [];
  proof.browserVersion = context.browser()?.version() ?? await page.evaluate(() => navigator.userAgent);
  page.on('request', request => origins.add(new URL(request.url()).origin)); page.on('pageerror', error => errors.push(error.message));
  await page.goto(origin);
  proof.cases = await page.evaluate(async origin => {
    const { compileBevyRuntimeBinary, initializeBevyRuntime } = await import('/initializer.js');
    const { BevyRuntimePreparation } = await import('/preparation.js');
    const results = [];
    for (const name of ['inflight', 'complete', 'cancel-selection', 'cancel-game', 'invalid', 'changed']) {
      const key = origin + '/' + name + '/runtime.js', events = [], signals = [];
      let imports = 0, initialized = 0, answer = null, errorName = null, initialImports;
      const preparation = new BevyRuntimePreparation(key, (signal, onResponseHeaders) => compileBevyRuntimeBinary({
        wasmUrl: origin + '/' + name + '/pinned.wasm', timeoutMs: 120000, ownerSignal: signal, onResponseHeaders,
        fetchImpl: request => { signals.push(request.signal); return fetch(request); }, onDownloadAttempt: event => events.push(event),
      }));
      const observed = async () => (await (await fetch('/observed?name=' + name)).json()).length;
      const started = performance.now();
      while (await observed() === 0) { if (performance.now() - started > 5000) throw Error('fixture request missing'); await new Promise(r => setTimeout(r, 10)); }
      initialImports = imports;
      const owner = new AbortController();
      const consume = () => initializeBevyRuntime({
        wasmUrl: origin + '/' + name + '/fresh.wasm', timeoutMs: 120000, ownerSignal: owner.signal,
        getPreparedBinary: signal => preparation.take(name === 'changed' ? key + '?other-package' : key, signal, () => events.push('headers-adopted')),
        loadModule: async () => { imports++; return { async default({ module_or_path }) {
          initialized++; answer = (await WebAssembly.instantiate(module_or_path)).exports.answer();
        } }; }, waitForCanvas: async () => {},
      });
      try {
        if (name === 'cancel-selection') preparation.dispose();
        else {
          if (name === 'complete') await preparation.take(key, new AbortController().signal);
          const pending = consume().catch(error => { errorName = error.name; });
          if (name === 'cancel-game') owner.abort();
          if (name === 'inflight') await fetch('/finish?name=' + name);
          await pending;
        }
        results.push({ name, initialImports, imports, initialized, answer, errorName, requests: await observed(),
          sourceAborted: signals.every(signal => signal.aborted), events });
      } finally { preparation.dispose(); }
    }
    return results;
  }, origin);
  await new Promise(resolve => setTimeout(resolve, 100));
  for (const row of proof.cases) {
    assert.equal(row.initialImports, 0); assert.equal(row.requests, row.name === 'changed' ? 2 : 1);
    if (['inflight', 'complete', 'changed'].includes(row.name)) {
      assert.equal(row.answer, 42); assert.equal(row.initialized, 1); assert.equal(row.imports, 1); assert.equal(row.errorName, null);
    } else {
      assert.equal(row.initialized, 0); assert.equal(row.answer, null);
      if (row.name === 'invalid') assert.equal(row.errorName, 'CompileError');
      if (row.name === 'cancel-game') assert.equal(row.errorName, 'AbortError');
      if (row.name === 'cancel-selection') assert.equal(row.imports, 0);
    }
  }
  for (const name of ['cancel-selection', 'cancel-game', 'changed']) assert(aborted.some(row => row.name === name && row.kind === 'pinned'), name);
  assert.deepEqual(errors, []); assert.deepEqual([...origins], [origin]);
  proof.origins = [...origins]; proof.pageErrors = errors; proof.requests = requests; proof.abortedBodies = aborted; proof.passed = true;
} catch (error) { proof.failure = String(error.message); process.exitCode = 1; }
finally {
  await context?.close(); proof.closed.browser = true;
  if (server.listening) { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
  proof.closed.server = !server.listening;
  await writeFile(root + '/receipt.json', JSON.stringify(proof, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ passed: proof.passed, cases: proof.cases?.length, failure: proof.failure, closed: proof.closed, receipt: root + '/receipt.json' }));
}
