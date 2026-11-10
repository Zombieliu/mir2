import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { FIXED_R16 } from '../../mir2-web3/apps/web/scripts/lib/fixed-r16-assets.mjs';
import { TEAM, BRANCH, REPOSITORY, SITES, ORIGIN, QR_RELEASE, runNumeronRelease, deploymentBody,
  validateAssetReceipt, validatePublicManifest, childEnvironment, windowsDownloadLinks, validateDownloadPage } from './numeron-r16-release.mjs';

const SHA = 'a'.repeat(40), TOKEN = 'synthetic-vercel-token', GH = 'synthetic-github-token';
const SECRET = 'synthetic-gateway-secret-not-a-real-credential';
const IDS = { main: 'dpl_maincandidate', game: 'dpl_gamecandidate' };
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const clone = value => structuredClone(value);
const json = value => Response.json(value);
const WINDOWS = 'https://assets.mir2.obelisk.build/client-updates/installers/fixture/Mir2Setup.exe';
// Exact public baseline element structure; a disabled Windows card is not an anchor.
const disabledWindows = '<article class="download-platform platform-windows" data-reveal="true"><span class="download-action is-disabled" aria-disabled="true"><span>尚未公开</span><i>—</i></span></article>';
const downloadHtml = (windows = '') => `<html><body><nav><a href="/zh-CN/promotions" data-track="navigation_promotions">Promotions</a></nav>
  <main class="download-page"><a class="download-action" href="${ORIGIN}/game" data-track="download_web">Play</a>${windows || disabledWindows}</main>
  <footer><a href="/zh-CN/promotions" data-track="footer_promotions">Promotions</a></footer></body></html>`;
const windowsAnchor = (href = WINDOWS) => `<a class="download-action" href="${href}" data-track="download_windows">Windows</a>`;

function fixture(operation = 'numeron-promote') {
  const qr = operation.startsWith('numeron-qr-');
  const source = qr ? QR_RELEASE.source : SHA;
  const assetSource = qr ? QR_RELEASE.assetSource : source;
  const sourceRef = qr ? QR_RELEASE.sourceRef : BRANCH;
  const runId = qr ? QR_RELEASE.assetRunId : '123';
  const qrIntent = 'synthetic.private.intent';
  const qrSession = '/api/wallet/dubhe/session';
  function qrResponse(pathname, token) {
    if (pathname === qrSession) {
      assert.equal(token, undefined);
      return { sessionId: 'synthetic-session', intentToken: qrIntent, expiresAt: Date.now() + 119_000,
        approvalUrl: `https://wallet.dubhe.obelisk.build/connect/approve?session=synthetic-session&relay=${encodeURIComponent(QR_RELEASE.relay)}` };
    }
    assert.equal(pathname, `${qrSession}/status`);
    assert.equal(token, qrIntent);
    return { status: 'pending' };
  }
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
    gitSource: { type: 'github', ref: sourceRef, sha: source }, meta: deploymentBody(key, source, 12345, config, SECRET).meta,
  }]));
  if (qr) {
    projects.game.targets.production.id = operation === 'numeron-qr-rollback' ? IDS.game : QR_RELEASE.baseline;
    candidates.game.meta.numeronR16ConnectRelay = QR_RELEASE.relay;
  }
  const snapshots = Object.fromEntries(['main', 'game'].map(key => [key, { uid: IDS[key], projectId: SITES[key].project,
    projectSettings: { rootDirectory: SITES[key].root } }]));
  const assetReceipt = { schemaVersion: 1, kind: 'mir2-fixed-r16-publication-receipt', repository: REPOSITORY,
    sourceSha: assetSource, materialSourceSha: contract.materialSourceSha, sourceClean: true, passed: true, dryRun: false,
    manifestPublished: true, manifestVerified: true, phase: 'complete', verified: contract.fileCount,
    uploaded: contract.fileCount, existingVerified: 0, remoteWrites: contract.fileCount + 1,
    putAttempts: contract.fileCount + 1, publicGzipWireVerified: contract.gzipFiles, publicGzipDecodedVerified: 0,
    unknownWriteOutcomes: 0, finishedAt: '2026-11-08T12:00:00Z', bundleSha256: contract.bundleSha256,
    payloadSha256: contract.payloadSha256, manifestSha256: contract.manifestSha256,
    review: { prefix: contract.prefix, files: contract.fileCount, bytes: contract.payloadBytes, wireBytes: contract.wireBytes,
      gzipFiles: contract.gzipFiles, fonts: contract.fontFiles, payloadSha256: contract.payloadSha256,
      manifestSha256: contract.manifestSha256, manifestLast: true, overwriteExisting: false,
      allMapsVerified: false, inheritedObjectsReverified: false } };
  const run = { id: Number(runId), repository: { full_name: REPOSITORY }, path: '.github/workflows/web-assets-r2-release.yml',
    head_sha: assetSource, status: 'completed', conclusion: 'success', event: 'workflow_dispatch' };
  const jobs = { total_count: 1, jobs: [{ name: 'publish-fixed-r16-assets', status: 'completed', conclusion: 'success' }] };
  const artifacts = { total_count: 1, artifacts: [{ id: 456, name: `fixed-r16-assets-${runId}`, expired: false }] };
  const env = { GITHUB_REPOSITORY: REPOSITORY, GITHUB_REF: `refs/heads/${BRANCH}`, GITHUB_SHA: SHA,
    RELEASE_OPERATION: operation, RELEASE_SOURCE_SHA: qr ? '' : SHA, VERCEL_ORG_ID: TEAM,
    VERCEL_PROJECT_ID: SITES.original.project, VERCEL_TOKEN: TOKEN, GH_TOKEN: GH,
    MIR2_R16_PASSKEY_AUTH_SECRET: SECRET, RELEASE_ASSET_RUN_ID: qr ? '' : runId,
    RELEASE_MAIN_DEPLOYMENT_ID: qr ? '' : IDS.main, RELEASE_DEPLOYMENT_ID: operation === 'numeron-qr-stage' ? '' : IDS.game };
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
    if (parsed.origin === ORIGIN && qr) {
      assert.equal(request.method, 'POST');
      assert.equal(request.headers.Origin, ORIGIN);
      assert.equal(projects.game.targets.production.id, IDS.game);
      return json(qrResponse(request.pathname, request.headers.Authorization?.slice(7)));
    }
    if (parsed.origin === 'https://api.github.com') {
      assert.equal(request.headers.Authorization, `Bearer ${GH}`);
      if (qr && request.pathname === `/repos/${REPOSITORY}/compare/${QR_RELEASE.baseSource}...${source}`) {
        return json({ status: 'ahead', ahead_by: 1, behind_by: 0, merge_base_commit: { sha: QR_RELEASE.baseSource },
          commits: [{ sha: source }], files: [...QR_RELEASE.changedFiles].reverse() });
      }
      if (request.pathname.endsWith('/jobs')) return json(jobs);
      if (request.pathname.endsWith('/artifacts')) return json(artifacts);
      assert.equal(request.pathname, `/repos/${REPOSITORY}/actions/runs/${runId}`); return json(run);
    }
    assert.equal(parsed.origin, 'https://api.vercel.com');
    assert.equal(parsed.searchParams.get('teamId'), TEAM);
    assert.equal(request.headers.Authorization, `Bearer ${TOKEN}`);
    let match = request.pathname.match(/^\/v9\/projects\/([^/]+)$/);
    if (match) return json(projects[keyFor(match[1])]);
    if (qr && request.pathname === `/v4/aliases/${new URL(ORIGIN).hostname}`) {
      return json({ alias: new URL(ORIGIN).hostname, projectId: SITES.game.project, deploymentId: projects.game.targets.production.id });
    }
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
  function readCandidate(candidate, pathname, request) {
    state.reads.push({ id: candidate.id, pathname });
    const overridden = state.onRead?.(candidate, pathname, state);
    if (overridden !== undefined) return overridden;
    if (pathname === '/zh-CN') return '<html data-track="navigation_promotions" data-footer="footer_promotions"><script src="/marketing-assets/_next/static/site.js"></script></html>';
    if (pathname === '/zh-CN/download') return downloadHtml();
    if (pathname === '/en') return '<html lang="en">';
    if (qr && request) return JSON.stringify(qrResponse(pathname, request.token));
    if (pathname === '/version') return JSON.stringify({ revision: source });
    if (pathname === '/manifest.webmanifest') return JSON.stringify({ start_url: '/game', scope: '/' });
    if (pathname === '/game') return '<html><script src="/_next/static/game.js"></script></html>';
    throw Error('unmocked candidate path');
  }
  async function execute() {
    let result, error;
    try { result = await runNumeronRelease({ env, fetchImpl, contract, readConfig: async () => config,
      readReceipt: async (id, name) => { assert.equal(id, runId); assert.equal(name, `fixed-r16-assets-${runId}`); return assetReceipt; },
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
  assert.deepEqual(Object.keys(main.env).sort(), ['EXPLORER_ORIGIN', 'GAME_HEALTH_URL', 'NEXT_PUBLIC_GAME_URL', 'NEXT_PUBLIC_SITE_URL', 'NEXT_PUBLIC_WINDOWS_DOWNLOAD_URL']);
  assert.equal(main.env.NEXT_PUBLIC_WINDOWS_DOWNLOAD_URL, '');
  assert.equal(main.build.env.NEXT_PUBLIC_WINDOWS_DOWNLOAD_URL, '');
  assert.equal(main.meta.numeronR16WindowsDownloadPolicy, 'unavailable-baseline');
  assert.equal(game.env.MIR2_PASSKEY_AUTH_SECRET, SECRET); assert.equal(game.env.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL, 'wss://165.154.65.136.sslip.io/playtest/ws');
  assert.equal(game.env.MIR2_RUNTIME_ENV, 'test'); assert.equal(game.env.NEXT_PUBLIC_MIR2_QUEST_GUIDANCE, 'newcomer-v2');
  assert.equal(game.env.MIR2_DUBHE_CONNECT_RELAY_URL, QR_RELEASE.relay);
  assert.equal(game.build.env.MIR2_DUBHE_CONNECT_RELAY_URL, undefined);
  assert.equal(main.env.MIR2_DUBHE_CONNECT_RELAY_URL, undefined);
  assert.equal(game.env.MIR2_PRODUCTION_ASSET_CONFIG, 'numeron-r16-web-assets.json');
  assert.equal(game.build.env.MIR2_PRODUCTION_ASSET_CONFIG, 'numeron-r16-web-assets.json');
  for (const field of ['MIR2_R2_PROXY_BASE', 'MIR2_ASSET_BASE_URL']) {
    assert.equal(game.env[field], f.config.assetBaseUrl);
    assert.equal(game.build.env[field], f.config.assetBaseUrl);
    assert.equal(main.env[field], undefined);
  }
  assert(!JSON.stringify(main.build.env).includes(SECRET));
  assert(!JSON.stringify(game.build.env).includes(SECRET));
  assert.equal(r.projects.original.targets.production.id, SITES.original.baseline);
  assert.equal(r.projects.main.targets.production.id, SITES.main.baseline); assert.equal(r.projects.game.targets.production.id, null);
  assert(!JSON.stringify(r.receipts).includes(SECRET)); assert(!JSON.stringify(r.receipts).includes(TOKEN));
});

test('QR stage creates only the pinned game source, with the live alias left on its baseline', async () => {
  const f = fixture('numeron-qr-stage'), r = await f.execute();
  assert.ifError(r.error);
  assert.equal(writes(r).length, 1);
  const body = writes(r)[0].body;
  assert.equal(body.project, SITES.game.project);
  assert.equal(body.target, 'production');
  assert.equal(body.autoAssignCustomDomains, false);
  assert.equal(body.gitSource.sha, QR_RELEASE.source);
  assert.equal(body.gitSource.ref, QR_RELEASE.sourceRef);
  assert.equal(r.result.sourceClosure.base, QR_RELEASE.baseSource);
  assert.equal(r.result.sourceClosure.assetSource, QR_RELEASE.assetSource);
  assert.deepEqual(r.result.sourceClosure.changedFiles, QR_RELEASE.changedFiles);
  assert.equal(body.env.MIR2_DUBHE_CONNECT_RELAY_URL, QR_RELEASE.relay);
  assert.equal(body.meta.numeronR16ConnectRelay, QR_RELEASE.relay);
  assert.equal(r.result.trafficAfter.alias, QR_RELEASE.baseline);
  assert(!JSON.stringify(r.receipts).includes(SECRET));
});

test('QR stage rejects a deployment source beyond the exact login patch', async () => {
  const f = fixture('numeron-qr-stage');
  f.state.onRequest = request => request.pathname.includes('/compare/') ? json({
    status: 'ahead', ahead_by: 1, behind_by: 0, merge_base_commit: { sha: QR_RELEASE.baseSource },
    commits: [{ sha: QR_RELEASE.source }],
    files: [...QR_RELEASE.changedFiles,
      { status: 'modified', filename: 'mir2-web3/apps/web/app/page.tsx' }],
  }) : undefined;
  const result = await f.execute();
  failed(result, /qrSourceClosureMismatch/);
  assert.equal(writes(result).length, 0);
});

test('QR promotion proves protected and public routes and keeps intents out of the receipt', async () => {
  const f = fixture('numeron-qr-promote'), r = await f.execute();
  assert.ifError(r.error);
  assert.deepEqual(writes(r).filter(call => new URL(call.url).origin !== ORIGIN).map(call => call.pathname),
    [`/v10/projects/${SITES.game.project}/promote/${IDS.game}`]);
  assert.equal(r.result.protectedQr.relay, QR_RELEASE.relay);
  assert.equal(r.result.publicQr.pending, true);
  assert.equal(r.result.trafficAfter.alias, IDS.game);
  assert(!JSON.stringify(r.receipts).includes('synthetic.private.intent'));
  assert(!JSON.stringify(r.receipts).includes('synthetic-session'));
});

test('QR promotion rejects a bad candidate before writing and rolls back a failed public check', async () => {
  const staged = fixture('numeron-qr-promote');
  staged.state.onRead = (_candidate, pathname) => pathname === '/api/wallet/dubhe/session' ? '{"unexpected":true}' : undefined;
  const blocked = await staged.execute();
  failed(blocked, /qrCreateFailed/);
  assert.equal(writes(blocked).length, 0);
  const promoted = fixture('numeron-qr-promote');
  promoted.state.onRequest = request => new URL(request.url).origin === ORIGIN ? new Response('', { status: 503 }) : undefined;
  const rolledBack = await promoted.execute();
  failed(rolledBack, /qrCreateFailed/);
  assert.equal(rolledBack.projects.game.targets.production.id, QR_RELEASE.baseline);
  assert.deepEqual(rolledBack.receipts[0].rollbacks, [{ site: 'game', deployment: QR_RELEASE.baseline }]);
});

test('QR rollback is limited to its promoted candidate and never touches a third-party alias', async () => {
  const f = fixture('numeron-qr-rollback'), r = await f.execute();
  assert.ifError(r.error);
  assert.equal(writes(r)[0].pathname, `/v1/projects/${SITES.game.project}/rollback/${QR_RELEASE.baseline}`);
  assert.equal(r.result.trafficAfter.alias, QR_RELEASE.baseline);
  const stale = fixture('numeron-qr-rollback');
  stale.projects.game.targets.production.id = 'dpl_external';
  const blocked = await stale.execute();
  failed(blocked, /qrTrafficMappingChanged/);
  assert.equal(writes(blocked).length, 0);
});

test('Windows links default to required; explicit empty mode accepts only a real download page', () => {
  assert.throws(() => windowsDownloadLinks(downloadHtml()), /windowsDownloadLinkMissing/);
  assert.deepEqual(windowsDownloadLinks(downloadHtml(), { allowEmpty: true }), []);
  assert.deepEqual(windowsDownloadLinks(downloadHtml(windowsAnchor()), { allowEmpty: true }), [WINDOWS]);
  for (const html of ['', '<html>Authentication required</html>', '<html><title>404</title></html>',
    `<html><script>download-page download_web navigation_promotions footer_promotions ${ORIGIN}/game</script></html>`]) {
    assert.throws(() => windowsDownloadLinks(html, { allowEmpty: true }));
  }
});

test('download identity follows the real website source markup and ignores serialized or commented markers', () => {
  const source = readFileSync(new URL('../../mir2-web3/apps/website/app/[locale]/download/page.tsx', import.meta.url), 'utf8');
  const chrome = readFileSync(new URL('../../mir2-web3/apps/website/app/components/site-chrome.tsx', import.meta.url), 'utf8');
  assert(source.includes('<main className="download-page">'));
  assert(source.includes('data-track="download_web"'));
  assert(source.includes('platform.id === "windows" && windowsUrl'));
  assert(source.includes('<span className="download-action is-disabled" aria-disabled="true">'));
  assert(chrome.includes('data-track="navigation_promotions"') && chrome.includes('data-track="footer_promotions"'));
  assert.doesNotThrow(() => validateDownloadPage(downloadHtml()));
  for (const html of [`<html><script>${downloadHtml()}</script></html>`, `<html><!--${downloadHtml()}--></html>`,
    ...['download-page', 'download_web', 'navigation_promotions', 'footer_promotions'].map(marker => downloadHtml().replaceAll(marker, 'absent'))]) {
    assert.throws(() => validateDownloadPage(html), /downloadPageHtmlInvalid/);
  }
  assert.throws(() => windowsDownloadLinks(downloadHtml(), { allowEmpty: 'true' }), /windowsDownloadLinkMissing/);
});

test('Windows unavailable baseline cannot gain a link, and a previous real link cannot disappear', async () => {
  const success = fixture(), same = await success.execute(); assert.ifError(same.error);
  for (const introduced of [true, false]) {
    const f = fixture();
    f.state.onRead = (candidate, pathname) => pathname === '/zh-CN/download'
      ? downloadHtml((candidate.id === SITES.main.baseline) !== introduced ? windowsAnchor() : '') : undefined;
    const r = await f.execute(); failed(r, /windowsDownloadsChanged/); assert.equal(writes(r).length, 0);
  }
  const retained = fixture(); retained.state.onRead = (_candidate, pathname) => pathname === '/zh-CN/download' ? downloadHtml(windowsAnchor()) : undefined;
  assert.ifError((await retained.execute()).error);
});

test('baseline and candidate download HTML must both be genuine, even with zero Windows links', async () => {
  for (const baseline of [true, false]) for (const html of ['', '<html>Authentication required</html>', '<html>404</html>',
    `<html><main class="download-page"><a href="${ORIGIN}/game" data-track="download_web">Play</a></main></html>`]) {
    const f = fixture();
    f.state.onRead = (candidate, pathname) => pathname === '/zh-CN/download' && (candidate.id === SITES.main.baseline) === baseline ? html : undefined;
    const r = await f.execute(); failed(r, /downloadPageHtmlInvalid/); assert.equal(writes(r).length, 0);
  }
});

test('empty Windows policy never tolerates missing, malformed or unsafe tracked hrefs', () => {
  for (const anchor of ['<a data-track="download_windows">Windows</a>', '<a data-track="download_windows" href=unquoted>Windows</a>',
    '<a data-track="download_windows" href="https://safe.example/a.exe" href="https://other.example/b.exe">Windows</a>',
    ...['http://example.com/setup.exe', 'javascript:alert(1)', '//example.com/setup.exe', 'https://user:pass@example.com/setup.exe',
      ' https://example.com/setup.exe', 'https://example.com/\\setup.exe', 'https://@example.com/setup.exe',
      'https://example.com/&#x0a;setup.exe'].map(windowsAnchor)]) {
    assert.throws(() => windowsDownloadLinks(downloadHtml(anchor), { allowEmpty: true }));
  }
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
