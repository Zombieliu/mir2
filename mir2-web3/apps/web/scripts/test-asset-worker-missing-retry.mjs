import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';
import ts from 'typescript';

const workerSource = readFileSync(new URL('../public/mir2-asset-worker.js', import.meta.url), 'utf8');
const origin = 'https://game.example.test';
const current = 'https://assets.example.test/mir2/v/current';
const older = 'https://assets.example.test/mir2/v/older';
const fontPath = '/original-ui/fonts/NotoSansTC.ttf';
const font = new Uint8Array([9, 7, 5, 3]);

test('the shipped registrar explicitly prefers its first-party font files', () => {
  const source = ts.createSourceFile('asset-cache-registrar.tsx', readFileSync(
    new URL('../app/components/asset-cache-registrar.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const policies = [];
  const visit = node => {
    if (ts.isPropertyAssignment(node) && node.name.getText(source) === 'fontMirrorFirst') policies.push(node.initializer.kind);
    ts.forEachChild(node, visit);
  };
  visit(source);
  assert.deepEqual(policies, [ts.SyntaxKind.FalseKeyword]);
});

function fixture(fetchImpl, { path = fontPath, mirrorFirst = true, remotes = [current, older], headers } = {}) {
  const calls = [];
  const context = vm.createContext({
    AbortController, AbortSignal, Headers, Request, Response, URL, console,
    setTimeout, clearTimeout,
    self: { location: { origin }, addEventListener() {} },
    fetch: request => { calls.push(request.url); return fetchImpl(request); },
  });
  vm.runInContext(workerSource, context);
  context.config = { version: 'current-cache', assetVersion: 'current', fontMirrorFirst: mirrorFirst,
    remoteAssetBaseUrl: '', remoteAssetBaseUrls: remotes };
  vm.runInContext('runtimeConfig = { ...runtimeConfig, ...config }', context);
  context.request = new Request(origin + path, { headers });
  return { calls, read: () => vm.runInContext('fetchStaticAsset(request)', context),
    negativeEntries: () => vm.runInContext('staticAssetNegativeCache.size', context) };
}

function prewarmRequest() {
  const module = { exports: {} };
  const js = ts.transpileModule(readFileSync(new URL('../lib/asset-prewarm-request.ts', import.meta.url), 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
  }).outputText;
  new Function('module', 'exports', js)(module, module.exports);
  return module.exports.fetchPrewarmAsset;
}

test('current font transport failure plus an older-release 404 is retryable, not confirmed missing', async () => {
  let healthy = false;
  const f = fixture(async request => {
    if (request.url.startsWith(current + '/')) {
      if (!healthy) throw new TypeError('temporary connection failure');
      return new Response(font);
    }
    return new Response(null, { status: 404 });
  });
  const failed = await f.read();
  assert.equal(failed.status, 504);
  assert.equal(f.negativeEntries(), 0);
  healthy = true;
  assert.deepEqual(new Uint8Array(await (await f.read()).arrayBuffer()), font);
});

test('the actual prewarm retries the current font after a mixed missing/transport failure', async () => {
  let logicalReads = 0;
  const f = fixture(async request => {
    if (request.url.startsWith(current + '/')) {
      if (logicalReads === 1) throw new TypeError('temporary connection failure');
      return new Response(font);
    }
    return new Response(null, { status: 404 });
  });
  const result = await prewarmRequest()(fontPath, { fetchImpl: () => { logicalReads++; return f.read(); } });
  assert.equal(result.response.status, 200);
  assert.deepEqual(result.body, font);
  assert.equal(logicalReads, 2);
});

test('a preferred font source is attempted once before all-missing negative caching', async () => {
  const f = fixture(async () => new Response(null, { status: 404 }));
  assert.equal((await f.read()).status, 404);
  assert.deepEqual(f.calls, [current + fontPath, origin + fontPath, older + fontPath]);
  const calls = f.calls.length;
  assert.equal((await f.read()).status, 404);
  assert.equal(f.calls.length, calls, 'all sources confirmed missing still has bounded negative caching');
});

test('app-first all-missing fonts retain one ordered fallback group and short negative caching', async () => {
  const f = fixture(async () => new Response(null, { status: 404 }), { mirrorFirst: false });
  assert.equal((await f.read()).status, 404);
  assert.deepEqual(f.calls, [origin + fontPath, current + fontPath, older + fontPath]);
  assert.equal((await f.read()).status, 404);
  assert.equal(f.calls.length, 3); assert.equal(f.negativeEntries(), 1);
});

test('an app-first transport error plus missing fallbacks remains retryable', async () => {
  let healthy = false;
  const f = fixture(async request => {
    if (request.url === origin + fontPath) {
      if (!healthy) throw new TypeError('temporary app connection failure');
      return new Response(font);
    }
    return new Response(null, { status: 404 });
  }, { mirrorFirst: false });
  assert.equal((await f.read()).status, 504);
  assert.equal(f.negativeEntries(), 0);
  assert.deepEqual(f.calls, [origin + fontPath, origin + fontPath, origin + fontPath, current + fontPath, older + fontPath]);
  healthy = true;
  assert.deepEqual(new Uint8Array(await (await f.read()).arrayBuffer()), font);
  assert.equal(f.calls.at(-1), origin + fontPath);
});

for (const status of [403, 429, 503]) {
  test(`an original ${status} is preserved instead of an optional mirror 404`, async () => {
    let healthy = false;
    const f = fixture(async request => request.url.startsWith(origin + '/')
      ? new Response(healthy ? 'healthy' : 'original unavailable', {
        status: healthy ? 200 : status, headers: { 'retry-after': '15' },
      }) : new Response(null, { status: 404 }), { path: '/original-ui/Prguse/1.png', mirrorFirst: false });
    const response = await f.read();
    assert.equal(response.status, status);
    assert.equal(response.headers.get('retry-after'), '15');
    assert.equal(await response.text(), 'original unavailable');
    assert.equal(f.negativeEntries(), 0);
    healthy = true;
    assert.equal(await (await f.read()).text(), 'healthy');
  });
}

test('a font mirror error keeps its response body and Retry-After if later sources are missing', async () => {
  const f = fixture(async request => request.url.startsWith(current + '/')
    ? new Response('mirror denied', { status: 403, headers: { 'retry-after': '7' } })
    : new Response(null, { status: 404 }));
  const response = await f.read();
  assert.equal(response.status, 403);
  assert.equal(response.headers.get('retry-after'), '7');
  assert.equal(await response.text(), 'mirror denied');
  assert.equal(f.negativeEntries(), 0);
});

for (const status of [404, 410]) {
  test(`a single confirmed ${status} still has short negative caching`, async () => {
    const f = fixture(async () => new Response('missing', { status }), { remotes: [] });
    assert.equal((await f.read()).status, status);
    assert.equal((await f.read()).status, status);
    assert.equal(f.calls.length, 1);
    assert.equal(f.negativeEntries(), 1);
  });
}

test('a later successful current source still wins over earlier missing/error responses', async () => {
  const f = fixture(async request => request.url.startsWith(current + '/')
    ? new Response(font) : new Response(null, { status: 403 }), { mirrorFirst: false });
  assert.deepEqual(new Uint8Array(await (await f.read()).arrayBuffer()), font);
  assert.equal(f.negativeEntries(), 0);
});

for (const status of [200, 206]) {
  test(`an invalid complete-font mirror ${status} cannot become a confirmed missing result`, async () => {
    const f = fixture(async request => request.url.startsWith(current + '/')
      ? new Response(status === 200 ? '' : new Uint8Array([9]), { status })
      : new Response(null, { status: 404 }));
    assert.equal((await f.read()).status, 504);
    assert.equal(f.negativeEntries(), 0);
    assert.equal(f.calls.filter(url => url.startsWith(current + '/')).length, 1);
  });
}

test('a valid ranged asset keeps its complete 206 response semantics', async () => {
  const f = fixture(async request => {
    assert.equal(request.headers.get('range'), 'bytes=0-1');
    return new Response(new Uint8Array([9, 7]), { status: 206, headers: { 'content-range': 'bytes 0-1/4' } });
  }, { headers: { range: 'bytes=0-1' } });
  const response = await f.read();
  assert.equal(response.status, 206);
  assert.equal(response.headers.get('content-range'), 'bytes 0-1/4');
  assert.deepEqual([...new Uint8Array(await response.arrayBuffer())], [9, 7]);
  assert.equal(f.negativeEntries(), 0);
});
