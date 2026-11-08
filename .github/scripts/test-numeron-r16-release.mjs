import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { FIXED_R16 } from '../../mir2-web3/apps/web/scripts/lib/fixed-r16-assets.mjs';
import { TEAM, BRANCH, REPOSITORY, SITES, ORIGIN, runNumeronRelease, deploymentBody,
  validateAssetReceipt, validatePublicManifest, childEnvironment } from './numeron-r16-release.mjs';

const SHA = 'a'.repeat(40), TOKEN = 'synthetic-vercel-token', GH = 'synthetic-github-token';
const SECRET = 'synthetic-gateway-secret-not-a-real-credential';
const IDS = { main: 'dpl_maincandidate', game: 'dpl_gamecandidate' };
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const clone = value => structuredClone(value);
const json = value => Response.json(value);

function fixture(operation = 'numeron-promote') {
  const manifest = Buffer.from(JSON.stringify({ schemaVersion: 1, kind: 'mir2-remote-asset-release',
    version: FIXED_R16.version, objectPrefix: FIXED_R16.prefix,
    candidateCoverage: { payloadSha256: FIXED_R16.payloadSha256, localCandidateFilesVerified: FIXED_R16.fileCount },
    bevyRuntime: { version: FIXED_R16.runtimeVersion } }));
  const contract = { ...FIXED_R16, manifestBytes: manifest.length, manifestSha256: digest(manifest) };
  const config = { schemaVersion: 3, version: contract.version, objectPrefix: contract.prefix,
    assetBaseUrl: `${contract.publicOrigin}/${contract.prefix}`,
    remoteReleaseUrl: `${contract.publicOrigin}/${contract.prefix}/remote-asset-release.json` };
  const projects = Object.fromEntries(Object.entries(SITES).map(([key, site]) => [key, {
    id: site.project, name: site.name, accountId: TEAM, targets: { production: { id: site.baseline } },
    link: { type: 'github', repo: 'mir2', org: 'Zombieliu', repoId: 12345 },
  }]));
  const candidates = Object.fromEntries(['main', 'game'].map(key => [key, {
    id: IDS[key], projectId: SITES[key].project, url: `${key}-candidate.vercel.app`, readyState: 'READY', target: 'production',
    gitSource: { type: 'github', ref: BRANCH, sha: SHA }, meta: deploymentBody(key, SHA, 12345, config, SECRET).meta,
  }]));
  const snapshots = Object.fromEntries(['main', 'game'].map(key => [key, { uid: IDS[key], projectId: SITES[key].project,
    projectSettings: { rootDirectory: SITES[key].root } }]));
  const assetReceipt = { schemaVersion: 1, kind: 'mir2-fixed-r16-publication-receipt', repository: REPOSITORY,
    sourceSha: SHA, materialSourceSha: contract.materialSourceSha, sourceClean: true, passed: true, dryRun: false,
    manifestPublished: true, manifestVerified: true, phase: 'complete', verified: contract.fileCount,
    uploaded: contract.fileCount, existingVerified: 0, remoteWrites: contract.fileCount + 1,
    putAttempts: contract.fileCount + 1, publicGzipWireVerified: contract.gzipFiles, publicGzipDecodedVerified: 0,
    unknownWriteOutcomes: 0, finishedAt: '2026-11-08T12:00:00Z', bundleSha256: contract.bundleSha256,
    payloadSha256: contract.payloadSha256, manifestSha256: contract.manifestSha256,
    review: { prefix: contract.prefix, files: contract.fileCount, bytes: contract.payloadBytes, wireBytes: contract.wireBytes,
      gzipFiles: contract.gzipFiles, fonts: contract.fontFiles, payloadSha256: contract.payloadSha256,
      manifestSha256: contract.manifestSha256, manifestLast: true, overwriteExisting: false,
      allMapsVerified: false, inheritedObjectsReverified: false } };
  const run = { id: 123, repository: { full_name: REPOSITORY }, path: '.github/workflows/web-assets-r2-release.yml',
    head_sha: SHA, status: 'completed', conclusion: 'success', event: 'workflow_dispatch' };
  const jobs = { total_count: 1, jobs: [{ name: 'publish-fixed-r16-assets', status: 'completed', conclusion: 'success' }] };
  const artifacts = { total_count: 1, artifacts: [{ id: 456, name: 'fixed-r16-assets-123', expired: false }] };
  const env = { GITHUB_REPOSITORY: REPOSITORY, GITHUB_REF: `refs/heads/${BRANCH}`, GITHUB_SHA: SHA,
    RELEASE_OPERATION: operation, RELEASE_SOURCE_SHA: SHA, VERCEL_ORG_ID: TEAM,
    VERCEL_PROJECT_ID: SITES.original.project, VERCEL_TOKEN: TOKEN, GH_TOKEN: GH,
    MIR2_R16_PASSKEY_AUTH_SECRET: SECRET, RELEASE_ASSET_RUN_ID: '123',
    RELEASE_MAIN_DEPLOYMENT_ID: IDS.main, RELEASE_DEPLOYMENT_ID: IDS.game };
  const state = { env, projects, candidates, snapshots, config, contract, assetReceipt, run, jobs, artifacts,
    calls: [], reads: [], receipts: [], onRequest: null, onRead: null };
  function keyFor(id) { return Object.keys(SITES).find(key => SITES[key].project === id); }
  async function fetchImpl(url, options = {}) {
    const parsed = new URL(url), request = { url, pathname: parsed.pathname, method: options.method || 'GET',
      headers: options.headers || {}, body: options.body ? JSON.parse(options.body) : undefined };
    state.calls.push(request);
    const override = await state.onRequest?.(request, state);
    if (override !== undefined) return override;
    if (parsed.origin === contract.publicOrigin) {
      assert.equal(request.headers.Authorization, undefined, 'no CI credential reaches public assets');
      assert.equal(url, config.remoteReleaseUrl); return new Response(manifest);
    }
    if (parsed.origin === 'https://api.github.com') {
      assert.equal(request.headers.Authorization, `Bearer ${GH}`);
      if (request.pathname.endsWith('/jobs')) return json(jobs);
      if (request.pathname.endsWith('/artifacts')) return json(artifacts);
      assert.equal(request.pathname, `/repos/${REPOSITORY}/actions/runs/123`); return json(run);
    }
    assert.equal(parsed.origin, 'https://api.vercel.com');
    assert.equal(parsed.searchParams.get('teamId'), TEAM);
    assert.equal(request.headers.Authorization, `Bearer ${TOKEN}`);
    let match = request.pathname.match(/^\/v9\/projects\/([^/]+)$/);
    if (match) return json(projects[keyFor(match[1])]);
    match = request.pathname.match(/^\/v13\/deployments\/([^/]+)$/);
    if (match) {
      if (match[1] === SITES.main.baseline) return json({ id: match[1], projectId: SITES.main.project, url: 'main-baseline.vercel.app' });
      return json(Object.values(candidates).find(value => value.id === match[1]));
    }
    if (request.pathname === '/v7/deployments') return json({ deployments: [snapshots[keyFor(parsed.searchParams.get('projectId'))]], pagination: {} });
    if (request.pathname === '/v13/deployments' && request.method === 'POST') {
      return json({ ...candidates[keyFor(request.body.project)], readyState: 'BUILDING' });
    }
    match = request.pathname.match(/^\/v10\/projects\/([^/]+)\/promote\/(dpl_[A-Za-z0-9]+)$/);
    if (match) { projects[keyFor(match[1])].targets.production.id = match[2]; return json({}); }
    match = request.pathname.match(/^\/v1\/projects\/([^/]+)\/rollback\/(dpl_[A-Za-z0-9]+)$/);
    if (match) { projects[keyFor(match[1])].targets.production.id = match[2]; return json({}); }
    throw Error('unmocked request');
  }
  function readCandidate(candidate, pathname) {
    state.reads.push({ id: candidate.id, pathname });
    const overridden = state.onRead?.(candidate, pathname, state);
    if (overridden !== undefined) return overridden;
    if (pathname === '/zh-CN') return '<html data-track="navigation_promotions" data-footer="footer_promotions"><script src="/marketing-assets/_next/static/site.js"></script></html>';
    if (pathname === '/zh-CN/download') return `<a href="${ORIGIN}/game">Play</a><a data-track="download_windows" href="https://assets.mir2.obelisk.build/client-updates/installers/fixture/Mir2Setup.exe">Windows</a>`;
    if (pathname === '/en') return '<html lang="en">';
    if (pathname === '/version') return JSON.stringify({ revision: SHA });
    if (pathname === '/manifest.webmanifest') return JSON.stringify({ start_url: '/game', scope: '/' });
    if (pathname === '/game') return '<html><script src="/_next/static/game.js"></script></html>';
    throw Error('unmocked candidate path');
  }
  async function execute() {
    let result, error;
    try { result = await runNumeronRelease({ env, fetchImpl, contract, readConfig: async () => config,
      readReceipt: async (id, name) => { assert.equal(id, '123'); assert.equal(name, 'fixed-r16-assets-123'); return assetReceipt; },
      readCandidate, sleep: async () => {}, onReceipt: async receipt => state.receipts.push(clone(receipt)) }); }
    catch (caught) { error = caught; }
    return { ...state, result, error };
  }
  return { ...state, state, execute };
}
const writes = result => result.calls.filter(call => call.method !== 'GET');
const failed = (result, pattern) => { assert(result.error); if (pattern) assert.match(result.error.code || '', pattern); };

test('import is inert; source, branch, team and original CI identity fail before I/O', async () => {
  for (const change of [{ GITHUB_REPOSITORY: 'other/repo' }, { GITHUB_REF: 'refs/heads/main' }, { GITHUB_SHA: 'moving' },
    { VERCEL_ORG_ID: 'wrong' }, { VERCEL_PROJECT_ID: SITES.game.project }, { VERCEL_TOKEN: '' },
    { RELEASE_OPERATION: 'numeron-move-domain' }]) {
    const f = fixture('numeron-stage'); Object.assign(f.env, change); const result = await f.execute(); failed(result); assert.equal(result.calls.length, 0);
  }
  const f = fixture('numeron-stage'); f.env.RELEASE_SOURCE_SHA = 'b'.repeat(40); const result = await f.execute(); failed(result); assert.equal(result.calls.length, 0);
});
test('context reads only safe metadata and never requires Gateway/GitHub secrets', async () => {
  const f = fixture('numeron-context'); delete f.env.GH_TOKEN; delete f.env.MIR2_R16_PASSKEY_AUTH_SECRET;
  f.projects.game.rootDirectory = SITES.game.root; f.projects.game.nodeVersion = '22.x';
  f.projects.main.env = [{ key: 'EXISTING_PRIVATE_SETTING', value: SECRET }];
  const r = await f.execute(); assert.ifError(r.error); assert.equal(writes(r).length, 0); assert.equal(r.calls.length, 3);
  assert.equal(r.result.projects.game.rootDirectory, SITES.game.root); assert.equal(r.result.projects.game.nodeVersion, '22.x');
  assert.equal(JSON.stringify(r.result).includes(SECRET), false); assert.equal(r.result.domainMovePending, true);
});
test('stage creates two same-source production candidates with explicit roots, Node22 and no automatic domains', async () => {
  const f = fixture('numeron-stage'), r = await f.execute(); assert.ifError(r.error);
  assert.equal(writes(r).length, 2);
  for (const [index, key] of ['main', 'game'].entries()) {
    const body = writes(r)[index].body;
    assert.equal(body.project, SITES[key].project); assert.equal(body.target, 'production'); assert.equal(body.autoAssignCustomDomains, false);
    assert.deepEqual(body.gitSource, { type: 'github', ref: BRANCH, repoId: 12345, sha: SHA });
    assert.equal(body.projectSettings.rootDirectory, SITES[key].root); assert.equal(body.projectSettings.nodeVersion, '22.x');
    assert.equal(body.projectSettings.buildCommand, key === 'game' ? 'bash ./scripts/vercel-build.sh' : 'npm run build');
    assert.equal(body.build.env.MIR2_PASSKEY_AUTH_SECRET, undefined);
  }
  const main = writes(r)[0].body, game = writes(r)[1].body;
  assert.deepEqual(Object.keys(main.env).sort(), ['EXPLORER_ORIGIN', 'GAME_HEALTH_URL', 'NEXT_PUBLIC_GAME_URL', 'NEXT_PUBLIC_SITE_URL']);
  assert.equal(game.env.MIR2_PASSKEY_AUTH_SECRET, SECRET); assert.equal(game.env.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL, 'wss://165.154.65.136.sslip.io/playtest/ws');
  assert.equal(game.env.MIR2_RUNTIME_ENV, 'test'); assert.equal(game.env.NEXT_PUBLIC_MIR2_QUEST_GUIDANCE, 'newcomer-v2');
  assert.equal(game.env.MIR2_PRODUCTION_ASSET_CONFIG, 'numeron-r16-web-assets.json');
  assert.equal(game.build.env.MIR2_PRODUCTION_ASSET_CONFIG, 'numeron-r16-web-assets.json');
  assert.equal(r.projects.original.targets.production.id, SITES.original.baseline);
  assert.equal(r.projects.main.targets.production.id, SITES.main.baseline); assert.equal(r.projects.game.targets.production.id, null);
  assert(!JSON.stringify(r.receipts).includes(SECRET)); assert(!JSON.stringify(r.receipts).includes(TOKEN));
});
test('missing secret, old prefix, foreign asset run or skipped job cannot stage anything', async () => {
  const changes = [f => { f.env.MIR2_R16_PASSKEY_AUTH_SECRET = ''; }, f => { f.config.objectPrefix = 'mir2/v/old'; },
    f => { f.run.head_sha = 'b'.repeat(40); }, f => { f.run.path = '.github/workflows/other.yml'; },
    f => { f.jobs.jobs[0].conclusion = 'skipped'; }, f => { f.artifacts.artifacts[0].expired = true; },
    f => { f.env.RELEASE_ASSET_RUN_ID = ''; }, f => { f.assetReceipt.dryRun = true; }];
  for (const change of changes) { const f = fixture('numeron-stage'); change(f); const r = await f.execute(); failed(r); assert.equal(writes(r).length, 0); }
});
test('all fixed asset receipt safety flags and hashes are mandatory', () => {
  const f = fixture(); validateAssetReceipt(f.assetReceipt, SHA, f.contract);
  for (const [key, value] of Object.entries({ passed: false, sourceClean: false,
    manifestVerified: false, unknownWriteOutcomes: 1, verified: 0, phase: 'partial', payloadSha256: '0'.repeat(64), sourceSha: 'b'.repeat(40) })) {
    assert.throws(() => validateAssetReceipt({ ...f.assetReceipt, [key]: value }, SHA, f.contract));
  }
  assert.throws(() => validateAssetReceipt({ ...f.assetReceipt, review: { ...f.assetReceipt.review, allMapsVerified: true } }, SHA, f.contract));
});
test('existing manifest revalidation requires all objects, gzip bodies and successful readback counts', async () => {
  const f = fixture('numeron-stage');
  Object.assign(f.assetReceipt, { manifestPublished: false, uploaded: 0, existingVerified: f.contract.fileCount, remoteWrites: 0, putAttempts: 0 });
  const r = await f.execute(); assert.ifError(r.error);
  for (const change of [{ existingVerified: f.contract.fileCount - 1 }, { publicGzipWireVerified: 0 },
    { verified: f.contract.fileCount - 1 }, { manifestVerified: false }, { unknownWriteOutcomes: 1 }, { remoteWrites: 1 }]) {
    assert.throws(() => validateAssetReceipt({ ...f.assetReceipt, ...change }, SHA, f.contract));
  }
});
test('public manifest missing, changed, old or oversized prevents writes', async () => {
  for (const response of [new Response('missing', { status: 404 }), new Response('wrong'), new Response(Buffer.alloc(2048))]) {
    const f = fixture('numeron-stage'); f.state.onRequest = req => req.url === f.config.remoteReleaseUrl ? response : undefined;
    const r = await f.execute(); failed(r); assert.equal(writes(r).length, 0);
  }
  assert.throws(() => validatePublicManifest(Buffer.from('wrong')));
});
test('project swaps and concurrent old/main/new production changes are rejected before staging', async () => {
  for (const key of Object.keys(SITES)) {
    const f = fixture('numeron-stage'); f.projects[key].targets.production.id = 'dpl_foreign';
    const r = await f.execute(); failed(r); assert.equal(writes(r).length, 0);
  }
  const f = fixture('numeron-stage'); f.projects.game.accountId = 'other'; const r = await f.execute(); failed(r); assert.equal(writes(r).length, 0);
});
test('concurrent change after first stage prevents second write and never changes original site', async () => {
  const f = fixture('numeron-stage'); f.state.onRequest = req => {
    if (req.method === 'POST') f.projects.main.targets.production.id = 'dpl_external';
  };
  const r = await f.execute(); failed(r, /concurrent/); assert.equal(writes(r).length, 1); assert.equal(r.projects.main.targets.production.id, 'dpl_external');
});
test('promote verifies protected pages and promotes marketing before new game, without domain API', async () => {
  const f = fixture(), r = await f.execute(); assert.ifError(r.error);
  assert(writes(r)[0].pathname.includes(SITES.main.project)); assert(writes(r)[1].pathname.includes(SITES.game.project));
  assert.equal(writes(r).length, 2); assert(r.reads.some(x => x.pathname === '/version')); assert(r.reads.some(x => x.id === SITES.main.baseline));
  assert.equal(r.result.customDomainChanged, false); assert.equal(r.result.domainMovePending, true);
});
test('already QA-promoted own game candidate is accepted without another promotion', async () => {
  const f = fixture(); f.projects.game.targets.production.id = IDS.game; const r = await f.execute(); assert.ifError(r.error);
  assert.equal(writes(r).length, 1); assert(writes(r)[0].pathname.includes(SITES.main.project));
});
test('promote permits an older immutable candidate source while keeping the current implementation identity', async () => {
  const f = fixture(); f.env.GITHUB_SHA = 'b'.repeat(40); const r = await f.execute(); assert.ifError(r.error);
  assert.equal(r.result.sourceSha, SHA); assert.equal(r.result.implementationSha, 'b'.repeat(40));
});
test('candidate source, ref, root, metadata, target and readiness mismatches prevent all writes', async () => {
  const changes = [f => { f.candidates.game.gitSource.sha = 'b'.repeat(40); }, f => { f.candidates.game.gitSource.ref = 'main'; },
    f => { f.snapshots.game.projectSettings.rootDirectory = SITES.main.root; }, f => { f.candidates.main.meta.numeronR16Root = SITES.game.root; },
    f => { f.candidates.game.target = 'preview'; }, f => { f.candidates.game.readyState = 'BUILDING'; },
    f => { f.candidates.game.projectId = SITES.original.project; }, f => { delete f.env.RELEASE_MAIN_DEPLOYMENT_ID; }];
  for (const change of changes) { const f = fixture(); change(f); const r = await f.execute(); failed(r); assert.equal(writes(r).length, 0); }
});
test('modern Git snapshots without root still require exact staged metadata and every protected application proof', async () => {
  for (const absent of [undefined, null]) {
    const f = fixture();
    for (const key of ['main', 'game']) f.snapshots[key].projectSettings.rootDirectory = absent;
    const r = await f.execute(); assert.ifError(r.error); assert.equal(writes(r).length, 2);
    for (const pathname of ['/zh-CN', '/zh-CN/download', '/en', '/game', '/version', '/manifest.webmanifest'])
      assert(r.reads.some(read => read.pathname === pathname), pathname);
  }
});
test('any explicitly returned incorrect deployment root rejects even when the other root field is absent', async () => {
  for (const where of ['candidate', 'candidateSettings', 'snapshot', 'snapshotSettings']) {
    const f = fixture(); delete f.snapshots.game.projectSettings.rootDirectory;
    if (where === 'candidate') f.candidates.game.rootDirectory = SITES.main.root;
    if (where === 'candidateSettings') f.candidates.game.projectSettings = { rootDirectory: SITES.main.root };
    if (where === 'snapshot') f.snapshots.game.rootDirectory = SITES.main.root;
    if (where === 'snapshotSettings') f.snapshots.game.projectSettings.rootDirectory = SITES.main.root;
    const r = await f.execute(); failed(r, /candidateIdentitySourceRootMismatch/); assert.equal(writes(r).length, 0);
  }
});
test('missing API root never bypasses metaRoot or HTML/version/PWA/download failure', async () => {
  for (const [pathname, content] of [
    ['/game', '<html>No application</html>'], ['/version', JSON.stringify({ revision: 'b'.repeat(40) })],
    ['/manifest.webmanifest', JSON.stringify({ start_url: '/', scope: '/' })],
    ['/zh-CN/download', `<a href="${ORIGIN}/game">Play</a><a data-track="download_windows" href="https://assets.mir2.obelisk.build/changed.exe">Windows</a>`],
    ['metaRoot', null],
  ]) {
    const f = fixture(); for (const key of ['main', 'game']) delete f.snapshots[key].projectSettings;
    if (pathname === 'metaRoot') delete f.candidates.game.meta.numeronR16Root;
    else f.state.onRead = (candidate, p) => p === pathname && candidate.id !== SITES.main.baseline ? content : undefined;
    const r = await f.execute(); failed(r); assert.equal(writes(r).length, 0);
  }
});
test('failed HTML/version/PWA or changed Windows downloads cannot promote', async () => {
  for (const [pathname, content] of [
    ['/game', '<html>Authentication required</html>'], ['/version', JSON.stringify({ revision: 'b'.repeat(40) })],
    ['/manifest.webmanifest', JSON.stringify({ start_url: '/', scope: '/' })],
    ['/zh-CN/download', `<a href="${ORIGIN}/game">Play</a><a data-track="download_windows" href="https://assets.mir2.obelisk.build/changed.exe">Windows</a>`],
    ['/zh-CN', '<html><script src="/marketing-assets/_next/static/site.js"></script></html>'],
  ]) {
    const f = fixture();
    f.state.onRead = (candidate, p) => p === pathname && candidate.id !== SITES.main.baseline ? content : undefined;
    const r = await f.execute(); failed(r); assert.equal(writes(r).length, 0);
  }
});
test('concurrent change during protected page reads is caught before first promotion', async () => {
  const f = fixture(); f.state.onRead = () => { f.projects.main.targets.production.id = 'dpl_external'; };
  const r = await f.execute(); failed(r, /concurrent/); assert.equal(writes(r).length, 0); assert.equal(r.projects.main.targets.production.id, 'dpl_external');
});
test('failed second promotion rolls back only the mapping still owned by this release', async () => {
  const f = fixture(); f.state.onRequest = req => req.pathname.includes(`/projects/${SITES.game.project}/promote/`) ? new Response('', { status: 503 }) : undefined;
  const r = await f.execute(); failed(r); assert.equal(r.projects.main.targets.production.id, SITES.main.baseline);
  assert.equal(writes(r).filter(x => x.pathname.includes('/rollback/')).length, 1);
  assert.equal(r.projects.original.targets.production.id, SITES.original.baseline);
});
test('a third-party production change during failure is never rolled back', async () => {
  const f = fixture(); f.state.onRequest = req => {
    if (req.pathname.includes(`/projects/${SITES.game.project}/promote/`)) {
      f.projects.main.targets.production.id = 'dpl_external'; return new Response('', { status: 503 });
    }
  };
  const r = await f.execute(); failed(r); assert.equal(r.projects.main.targets.production.id, 'dpl_external');
  assert.equal(writes(r).filter(x => x.pathname.includes('/rollback/')).length, 0);
});
test('an applied promotion with a lost response is rolled back only while still owned', async () => {
  const f = fixture(); f.state.onRequest = req => {
    if (req.pathname.includes(`/projects/${SITES.main.project}/promote/`)) {
      f.projects.main.targets.production.id = IDS.main; throw Error('synthetic lost response');
    }
  };
  const r = await f.execute(); failed(r, /apiTransportFailed/);
  assert.equal(r.projects.main.targets.production.id, SITES.main.baseline);
  assert.equal(writes(r).filter(x => x.pathname.includes('/rollback/')).length, 1);
});
test('failed uncertain staging still reads all baselines and never retries the create', async () => {
  const f = fixture('numeron-stage'); f.state.onRequest = req => {
    if (req.pathname === '/v13/deployments' && req.method === 'POST') throw Error('synthetic lost create response');
  };
  const r = await f.execute(); failed(r, /apiTransportFailed/); assert.equal(writes(r).length, 1);
  const at = r.calls.findIndex(req => req.method === 'POST');
  assert.equal(r.calls.slice(at + 1).filter(req => req.pathname.startsWith('/v9/projects/')).length, 3);
  assert.equal(r.receipts[0].writes[0].responseKnown, false);
});
test('CLI transports receive only the relevant credential and never the Gateway signing secret', () => {
  const env = { PATH: 'fixture', VERCEL_TOKEN: TOKEN, GH_TOKEN: GH, MIR2_R16_PASSKEY_AUTH_SECRET: SECRET, OPENAI_API_KEY: 'synthetic-model-key' };
  for (const kind of ['github', 'vercel']) {
    const child = childEnvironment(env, kind); assert.equal(child.MIR2_R16_PASSKEY_AUTH_SECRET, undefined); assert.equal(child.OPENAI_API_KEY, undefined);
    assert.equal(child[kind === 'github' ? 'VERCEL_TOKEN' : 'GH_TOKEN'], undefined);
  }
});
test('workflow isolates Numeron operations, pins Node22, grants read-only artifacts and never invokes old release scripts for them', () => {
  const workflow = readFileSync(new URL('../workflows/promotion-website-release.yml', import.meta.url), 'utf8');
  assert(workflow.includes('asset_run_id:')); assert(workflow.includes('RELEASE_ASSET_RUN_ID: ${{ inputs.asset_run_id }}'));
  assert(workflow.includes("node-version: '22'")); assert(workflow.includes('actions: read'));
  assert(workflow.includes("&& !startsWith(inputs.operation, 'numeron-')"));
  assert(workflow.includes('MIR2_R16_PASSKEY_AUTH_SECRET: ${{ secrets.MIR2_R16_PASSKEY_AUTH_SECRET }}'));
});
