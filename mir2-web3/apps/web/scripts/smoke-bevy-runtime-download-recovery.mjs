// Loopback Chrome transport checks. No game account, external service or real-game timing claim.
import assert from 'node:assert/strict';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';
import ts from 'typescript';

assert(process.argv[2] && process.argv.length === 3, 'pass a fresh evidence directory');
assert(process.env.MIR2_CHROME_BIN, 'set MIR2_CHROME_BIN to the actual Chrome executable');
const output = resolve(process.argv[2]);
await mkdir(output, { recursive: false });
const { chromium } = createRequire(resolve(process.env.MIR2_PLAYWRIGHT_PACKAGE ?? 'package.json'))('playwright');
const source = await readFile(new URL('../lib/bevy-runtime-initializer.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } });
const policySource = await readFile(new URL('../lib/bevy-runtime-policy.ts', import.meta.url), 'utf8');
const compiledPolicy = ts.transpileModule(policySource, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } });
const binary = Buffer.from('0061736d010000000105016000017f03020100070a0106616e7377657200000a06010400412a0b', 'hex');
assert.equal(new WebAssembly.Instance(await WebAssembly.compile(binary)).exports.answer(), 42);
const requests = [], retiredBodies = [], counts = new Map(), timers = new Set();
let browser;
const server = createServer((req, res) => {
  if (req.url === '/initializer.js') {
    res.writeHead(200, { 'content-type': 'application/javascript', 'cache-control': 'no-store' }); res.end(compiled.outputText); return;
  }
  if (req.url === '/policy.js') {
    res.writeHead(200, { 'content-type': 'application/javascript', 'cache-control': 'no-store' }); res.end(compiledPolicy.outputText); return;
  }
  const match = req.url.match(/^\/(reset|idle|exhausted|permanent|invalid|oversized)\/(app|mirror)\.wasm$/);
  if (!match) { res.writeHead(200, { 'content-type': 'text/html' }); res.end('<!doctype html><title>Runtime download fixture</title>'); return; }
  const [, scenario, mirror] = match;
  const count = (counts.get(req.url) ?? 0) + 1; counts.set(req.url, count);
  const row = { scenario, mirror, count, at: Date.now() }; requests.push(row);
  const status = scenario === 'permanent' ? 404
    : scenario === 'exhausted' || (mirror === 'app' && scenario !== 'reset') ? 503 : 200;
  row.status = status;
  res.writeHead(status, { 'content-type': 'application/wasm', 'cache-control': 'no-store' });
  if (status !== 200) { res.end(); return; }
  if ((scenario === 'reset' && (mirror === 'app' || count === 1)) || (scenario === 'idle' && count === 1)) {
    row.partialBytes = 4; res.flushHeaders(); res.write(binary.subarray(0, 4));
    res.once('close', () => { if (!res.writableEnded) retiredBodies.push({ scenario, mirror, at: Date.now() }); });
    if (scenario === 'reset') {
      const timer = setTimeout(() => { timers.delete(timer); res.destroy(); }, 120); timers.add(timer);
    }
    return;
  }
  res.end(scenario === 'invalid' ? Buffer.from('invalid runtime') : binary);
});
const proof = { passed: false, scope: 'actual Chrome, loopback HTTP faults and actual WASM compilation; no game, account or production performance qualification',
  sourceSha256: createHash('sha256').update(source).digest('hex'), policySourceSha256: createHash('sha256').update(policySource).digest('hex'),
  closed: { browser: false, server: false } };
try {
  await new Promise((ok, fail) => { server.once('error', fail); server.listen(0, '127.0.0.1', ok); });
  const origin = 'http://127.0.0.1:' + server.address().port;
  browser = await chromium.launch({ executablePath: process.env.MIR2_CHROME_BIN, headless: true });
  proof.chromeVersion = browser.version();
  const page = await browser.newPage();
  const origins = new Set(), pageErrors = [];
  page.on('request', request => origins.add(new URL(request.url()).origin));
  page.on('pageerror', error => pageErrors.push(error.message));
  await page.goto(origin);
  proof.cases = await page.evaluate(async origin => {
    const { initializeBevyRuntime } = await import(origin + '/initializer.js');
    const policy = await import(origin + '/policy.js');
    const results = [];
    for (const scenario of ['reset', 'idle', 'exhausted', 'permanent', 'invalid', 'oversized']) {
      const events = [], signals = [], requests = [];
      let imports = 0, canvases = 0, initialized = 0, answer = null, errorName = null, priorRequestsAborted = null;
      let pageEntryRetry = null, alternateBackendRetry = null;
      const started = Date.now();
      try {
        await initializeBevyRuntime({
          wasmUrl: origin + '/' + scenario + '/app.wasm', wasmFallbackUrls: [origin + '/' + scenario + '/mirror.wasm'],
          timeoutMs: 120000, ...(scenario === 'oversized' ? { maxBodyBytes: 8 } : {}),
          loadModule: async () => { imports++; return { async default({ module_or_path }) {
            initialized++; answer = (await WebAssembly.instantiate(module_or_path)).exports.answer();
          } }; },
          waitForCanvas: async () => { canvases++; }, onDownloadAttempt: event => events.push(event),
          fetchImpl: request => {
            if (signals.length === 2) priorRequestsAborted = signals.every(signal => signal.aborted);
            signals.push(request.signal); requests.push(new URL(request.url).pathname); return fetch(request);
          },
        });
      } catch (error) {
        errorName = error.name;
        pageEntryRetry = policy.canRetryBevyRuntimeBeforeBoot(false, error);
        alternateBackendRetry = policy.shouldRetryBevyRuntimeWithWebGl2('webgpu', true, error);
      }
      results.push({ scenario, elapsedMs: Date.now() - started, imports, canvases, initialized, answer, errorName,
        priorRequestsAborted, requests, events, pageEntryRetry, alternateBackendRetry });
    }
    return results;
  }, origin);
  assert.deepEqual([...origins], [origin]); assert.deepEqual(pageErrors, []);
  for (const value of proof.cases) {
    assert.equal(value.imports, 1);
    if (value.scenario === 'reset' || value.scenario === 'idle') {
      assert.equal(value.errorName, null); assert.equal(value.initialized, 1); assert.equal(value.canvases, 1); assert.equal(value.answer, 42);
      assert.equal(value.requests.length, 3); assert.equal(value.requests[1], value.requests[2]); assert(value.priorRequestsAborted);
      assert.equal(value.events.at(-1).bytes, binary.length); assert.equal(value.events.at(-1).state, 'completed');
      if (value.scenario === 'idle') {
        const failure = value.events.find(x => x.reason === 'body-idle'); assert(failure); assert.equal(failure.bytes, 4);
        assert(value.elapsedMs >= 14900 && value.elapsedMs < 30000, 'the real 15-second idle boundary must remain bounded');
      }
    } else {
      assert.equal(value.initialized, 0); assert.equal(value.canvases, 0);
      assert.equal(value.requests.length, value.scenario === 'exhausted' ? 3 : 2);
      assert.equal(value.errorName, value.scenario === 'invalid' ? 'CompileError' : value.scenario === 'oversized' ? 'RangeError' : 'TypeError');
      if (value.scenario === 'invalid' || value.scenario === 'oversized') {
        assert.equal(value.pageEntryRetry, false); assert.equal(value.alternateBackendRetry, false);
      }
    }
  }
  assert.equal(retiredBodies.filter(x => x.scenario === 'idle').length, 1);
  proof.requests = requests; proof.retiredBodies = retiredBodies; proof.pageErrors = pageErrors; proof.passed = true;
} catch (error) { proof.failure = error.message; throw error; }
finally {
  if (browser) { await browser.close(); proof.closed.browser = true; }
  for (const timer of timers) clearTimeout(timer);
  server.closeAllConnections(); await new Promise(ok => server.close(ok)); proof.closed.server = true;
  await writeFile(join(output, 'receipt.json'), JSON.stringify(proof, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ passed: proof.passed, cases: proof.cases?.length ?? 0, closed: proof.closed, receipt: join(output, 'receipt.json') }));
}
