import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const TEAM = 'team_pXY7oaLtibSTlKSmKXMXzrQR';
const BRANCH = 'codex/promotion-vercel-20261029';
const SHA = 'a'.repeat(40);
const TOKEN = 'synthetic-ci-token-never-log';
const SECRET = 'synthetic-existing-promotion-secret-never-log';
const sites = {
  main: { project: 'prj_Q14gZuvjxkfqOCExhqOrYiR2UbxA', name: 'numeron-legend-of-rebirth', baseline: 'dpl_4uHMwZobvDW1tUwszUdbq14z8eMj', root: 'mir2-web3/apps/website', candidate: 'dpl_maincandidate' },
  game: { project: 'prj_Bl3ttB4PzcdCpJMYLRhzQjkdXyTz', name: 'mir2-web3-web', baseline: 'dpl_GuricEhcXbVub8u3u3HSt2VR5wGe', root: 'mir2-web3/apps/web', candidate: 'dpl_gamecandidate' },
};
const source = readFileSync(new URL('./unified-website-release.mjs', import.meta.url), 'utf8');
const executable = source.replace(/^import\s+\{\s*writeFile\s*\}\s+from\s+['"]node:fs\/promises['"];?\r?\n/, '');
assert.notEqual(executable, source, 'only filesystem import is replaced with an in-memory writer');

function fixture(operation = 'unified-promote') {
  const projects = {}, candidates = {}, calls = [], logs = [], files = new Map();
  for (const [key, site] of Object.entries(sites)) {
    projects[key] = { id: site.project, accountId: TEAM, name: site.name, rootDirectory: site.root,
      targets: { production: { id: site.baseline } }, link: { type: 'github', repo: 'mir2', repoId: 123456 },
      env: [{ key: 'MIR2_PROMOTION_SESSION_SECRET', value: SECRET, target: ['production'] }] };
    candidates[key] = { id: site.candidate, url: `${key}-candidate.protected.vercel.app`, projectId: site.project,
      readyState: 'READY', target: 'production', projectSettings: { rootDirectory: site.root }, gitSource: { sha: SHA },
      meta: { unifiedSourceSha: SHA, unifiedMainBaseline: sites.main.baseline, unifiedGameBaseline: sites.game.baseline } };
  }
  const env = { GITHUB_REF: `refs/heads/${BRANCH}`, GITHUB_SHA: SHA, RELEASE_OPERATION: operation,
    VERCEL_TOKEN: TOKEN, VERCEL_ORG_ID: TEAM, VERCEL_PROJECT_ID: sites.game.project,
    RELEASE_MAIN_DEPLOYMENT_ID: sites.main.candidate, RELEASE_DEPLOYMENT_ID: sites.game.candidate };
  const state = { projects, candidates, env, calls, logs, files, onRequest: undefined };
  const projectKey = id => Object.keys(sites).find(key => sites[key].project === id);
  const json = value => Response.json(JSON.parse(JSON.stringify(value)));
  const fetch = async (input, init = {}) => {
    const url = new URL(input);
    const request = { origin: url.origin, path: url.pathname, query: url.searchParams, method: init.method || 'GET', headers: init.headers || {}, body: init.body ? JSON.parse(init.body) : undefined };
    calls.push(request);
    const overridden = await state.onRequest?.(request, state);
    if (overridden !== undefined) return overridden;
    if (url.origin === 'https://mir2.obelisk.build') {
      assert.equal(request.headers.Authorization, undefined, 'CI bearer never reaches the public origin');
      const pages = {
        '/': '<html data-track="navigation_promotions" data-footer="footer_promotions"><a href="/promotions?lang=zh-CN">Referral</a><script src="/marketing-assets/_next/static/chunks/site.js"></script></html>',
        '/marketing-assets/_next/static/chunks/site.js': 'window.marketingReady = true;',
        '/zh-CN/download': '<a href="https://mir2.obelisk.build/game">Play</a>',
        '/en': '<a href="/promotions?lang=en">Referral</a>',
        '/game': '<html>Game client</html>', '/promotions': '<html>Promotion application</html>',
        '/version': JSON.stringify({ revision: SHA }),
        '/manifest.webmanifest': JSON.stringify({ id: '/', start_url: '/game', scope: '/' }),
        '/api/promotions/bootstrap': JSON.stringify({ authenticated: false, campaign: { campaignId: 'numeron-playtest-20261029', environment: 'test' } }),
      };
      if (request.path in pages) return new Response(pages[request.path]);
      throw Error(`Unmocked public path ${request.path}`);
    }
    assert.equal(url.origin, 'https://api.vercel.com', 'every network destination must be explicitly mocked');
    assert.equal(url.searchParams.get('teamId'), TEAM);
    assert.equal(request.headers.Authorization, `Bearer ${TOKEN}`);
    const projectRead = request.path.match(/^\/v9\/projects\/([^/]+)$/);
    if (projectRead && request.method === 'GET') return json(projects[projectKey(projectRead[1])]);
    const candidateRead = request.path.match(/^\/v13\/deployments\/([^/]+)$/);
    if (candidateRead && request.method === 'GET') {
      const value = Object.values(candidates).find(candidate => candidate.id === candidateRead[1]);
      return value ? json(value) : Response.json({ error: { code: 'NOT_FOUND' } }, { status: 404 });
    }
    if (request.path === '/v13/deployments' && request.method === 'POST') {
      const key = projectKey(request.body.project);
      return json({ ...candidates[key], readyState: 'BUILDING' });
    }
    const promote = request.path.match(/^\/v10\/projects\/([^/]+)\/promote\/(dpl_[a-zA-Z0-9]+)$/);
    if (promote && request.method === 'POST') {
      projects[projectKey(promote[1])].targets.production.id = promote[2];
      return json({});
    }
    const rollback = request.path.match(/^\/v1\/projects\/([^/]+)\/rollback\/(dpl_[a-zA-Z0-9]+)$/);
    if (rollback && request.method === 'POST') {
      projects[projectKey(rollback[1])].targets.production.id = rollback[2];
      return json({});
    }
    throw Error(`Unmocked API request ${request.method} ${request.path}`);
  };
  async function run() {
    let error;
    try {
      await vm.runInNewContext(`(async () => {\n${executable}\n})()`, {
        process: { env }, fetch, AbortSignal, URL, JSON, console: { log: (...args) => logs.push(args.join(' ')) },
        writeFile: async (name, value) => files.set(name, value), setTimeout: callback => { callback(); return 0; },
      }, { timeout: 2000, filename: 'mocked-unified-website-release.mjs' });
    } catch (value) { error = value; }
    return { error, ...state };
  }
  return { ...state, state, run };
}
const writes = result => result.calls.filter(call => call.method !== 'GET');
const assertFailed = (result, message) => { assert.ok(result.error, 'release must fail closed'); if (message) assert.match(String(result.error), message); };

test('immutable source and original CI identity fail before any network I/O', async () => {
  for (const change of [{ GITHUB_REF: 'refs/heads/main' }, { GITHUB_SHA: 'moving-ref' }, { VERCEL_ORG_ID: 'wrong-team' }, { VERCEL_PROJECT_ID: sites.main.project }, { VERCEL_TOKEN: '' }]) {
    const f = fixture(); Object.assign(f.env, change);
    const result = await f.run(); assertFailed(result); assert.equal(result.calls.length, 0);
  }
});

test('foreign projects and changed production baselines reject before mutations', async () => {
  for (const change of [f => { f.projects.main.accountId = 'foreign-team'; }, f => { f.projects.game.targets.production.id = 'dpl_concurrentrelease'; }]) {
    const f = fixture(); change(f);
    const result = await f.run(); assertFailed(result); assert.equal(writes(result).length, 0);
  }
});

test('staging creates only two unaliased exact-SHA candidates and preserves all existing secrets', async () => {
  const f = fixture('unified-stage');
  const result = await f.run(); assert.equal(result.error, undefined);
  const mutations = writes(result); assert.equal(mutations.length, 2);
  for (const [index, key] of ['main', 'game'].entries()) {
    const request = mutations[index];
    assert.equal(request.path, '/v13/deployments'); assert.equal(request.body.project, sites[key].project);
    assert.equal(request.body.autoAssignCustomDomains, false); assert.equal(request.body.target, 'production');
    assert.equal(request.body.gitSource.sha, SHA); assert.equal(request.body.gitSource.ref, BRANCH);
    assert.equal(request.body.meta.unifiedSourceSha, SHA);
    assert.equal(f.projects[key].targets.production.id, sites[key].baseline);
    assert.equal(f.projects[key].env[0].value, SECRET);
    if (key === 'main') {
      assert.equal(request.body.projectSettings.rootDirectory, sites.main.root);
      assert.equal(request.body.env.NEXT_PUBLIC_SITE_URL, 'https://mir2.obelisk.build');
      assert.equal(request.body.env.NEXT_PUBLIC_GAME_URL, 'https://mir2.obelisk.build/game');
    } else {
      assert.equal(request.body.env, undefined, 'existing game environment and public marketing alias stay unchanged');
      assert.equal(request.body.projectSettings, undefined);
    }
    assert.ok(!JSON.stringify(request.body).includes(SECRET));
  }
  const receipt = JSON.parse(result.files.get('unified-release.json'));
  assert.equal(receipt.sourceSha, SHA); assert.equal(receipt.sites.main.id, sites.main.candidate); assert.equal(receipt.sites.game.id, sites.game.candidate);
  assert.ok(!result.logs.join('\n').includes(TOKEN)); assert.ok(!result.logs.join('\n').includes(SECRET));
});

test('staging stops before the second candidate if an external release changes production', async () => {
  const f = fixture('unified-stage');
  f.state.onRequest = request => {
    if (request.path === '/v13/deployments' && request.method === 'POST') f.projects.game.targets.production.id = 'dpl_otherpublisher';
  };
  const result = await f.run(); assertFailed(result, /production changed/); assert.equal(writes(result).length, 1);
});

test('promotion rejects mismatched source, root, project, readiness and review metadata', async () => {
  for (const change of [candidate => { candidate.gitSource.sha = 'b'.repeat(40); }, candidate => { candidate.projectSettings.rootDirectory = 'other-root'; }, candidate => { candidate.projectId = sites.game.project; }, candidate => { candidate.readyState = 'BUILDING'; }, candidate => { candidate.target = 'preview'; }, candidate => { candidate.meta.unifiedMainBaseline = 'dpl_otherbaseline'; }, candidate => { candidate.meta.unifiedSourceSha = 'b'.repeat(40); }]) {
    const f = fixture(); change(f.candidates.main);
    const result = await f.run(); assertFailed(result, /candidate identity/); assert.equal(writes(result).length, 0);
  }
});

test('promotion rejects a deployment response whose ID differs from the exact reviewed input', async () => {
  for (const key of ['main', 'game']) {
    const f = fixture();
    f.state.onRequest = request => request.path === `/v13/deployments/${sites[key].candidate}`
      ? Response.json({ ...f.candidates[key], id: 'dpl_unreviewedcandidate' }) : undefined;
    const result = await f.run(); assertFailed(result, /Candidate deployment ID mismatch/);
    assert.equal(writes(result).length, 0);
  }
});

test('promotion changes main then game, smoke-checks the public alias, and never accesses protected candidate URLs', async () => {
  const result = await fixture().run(); assert.equal(result.error, undefined);
  assert.deepEqual(writes(result).map(call => call.path), [
    `/v10/projects/${sites.main.project}/promote/${sites.main.candidate}`,
    `/v10/projects/${sites.game.project}/promote/${sites.game.candidate}`,
  ]);
  assert.ok(result.calls.some(call => call.origin === 'https://mir2.obelisk.build' && call.path === '/api/promotions/bootstrap'));
  assert.ok(result.calls.every(call => !call.origin.includes('protected.vercel.app')));
  assert.ok(result.logs.some(value => value.includes('"publicSmoke":"passed"')));
});

test('a public smoke failure restores both still-owned candidates in reverse order', async () => {
  const f = fixture(); f.state.onRequest = request => request.origin === 'https://mir2.obelisk.build' && request.path === '/' ? new Response('wrong-site') : undefined;
  const result = await f.run(); assertFailed(result, /unexpected content/);
  assert.equal(result.calls.filter(call => call.origin === 'https://mir2.obelisk.build' && call.path === '/').length, 4, 'propagation retries are bounded');
  assert.deepEqual(writes(result).map(call => call.path), [
    `/v10/projects/${sites.main.project}/promote/${sites.main.candidate}`,
    `/v10/projects/${sites.game.project}/promote/${sites.game.candidate}`,
    `/v1/projects/${sites.game.project}/rollback/${sites.game.baseline}`,
    `/v1/projects/${sites.main.project}/rollback/${sites.main.baseline}`,
  ]);
  assert.equal(f.projects.main.targets.production.id, sites.main.baseline); assert.equal(f.projects.game.targets.production.id, sites.game.baseline);
});

test('short revision propagation retries public smoke without repeating promotion mutations', async () => {
  const f = fixture(); let versionReads = 0;
  f.state.onRequest = request => request.origin === 'https://mir2.obelisk.build' && request.path === '/version' && versionReads++ < 2
    ? Response.json({ revision: 'b'.repeat(40) }) : undefined;
  const result = await f.run(); assert.equal(result.error, undefined);
  assert.equal(versionReads, 3); assert.equal(writes(result).length, 2);
  assert.ok(!writes(result).some(call => call.path.includes('/rollback/')));
});

test('a persistent public revision mismatch cannot pass smoke and restores the reviewed baselines', async () => {
  const f = fixture();
  f.state.onRequest = request => request.origin === 'https://mir2.obelisk.build' && request.path === '/version'
    ? Response.json({ revision: 'b'.repeat(40) }) : undefined;
  const result = await f.run(); assertFailed(result, /Public game revision mismatch/);
  assert.equal(result.calls.filter(call => call.origin === 'https://mir2.obelisk.build' && call.path === '/version').length, 4);
  assert.equal(writes(result).filter(call => call.path.includes('/rollback/')).length, 2);
  assert.equal(f.projects.main.targets.production.id, sites.main.baseline); assert.equal(f.projects.game.targets.production.id, sites.game.baseline);
});

test('compensation skips an externally replaced candidate rather than overwriting the other publisher', async () => {
  const f = fixture();
  f.state.onRequest = request => {
    if (request.path === `/v10/projects/${sites.game.project}/promote/${sites.game.candidate}`) {
      f.projects.main.targets.production.id = 'dpl_otherpublisher';
      return Response.json({ error: { code: 'PROMOTE_FAILED' } }, { status: 503 });
    }
  };
  const result = await f.run(); assertFailed(result, /HTTP 503/);
  assert.ok(!writes(result).some(call => call.path.includes('/rollback/')));
  assert.equal(f.projects.main.targets.production.id, 'dpl_otherpublisher');
});

test('an uncertain promote that already applied is included in guarded compensation', async () => {
  const f = fixture();
  f.state.onRequest = request => {
    if (request.path === `/v10/projects/${sites.game.project}/promote/${sites.game.candidate}`) {
      f.projects.game.targets.production.id = sites.game.candidate;
      throw Error('synthetic network result unknown');
    }
  };
  const result = await f.run(); assertFailed(result, /result unknown/);
  assert.deepEqual(writes(result).filter(call => call.path.includes('/rollback/')).map(call => call.path), [
    `/v1/projects/${sites.game.project}/rollback/${sites.game.baseline}`,
    `/v1/projects/${sites.main.project}/rollback/${sites.main.baseline}`,
  ]);
});

test('workflow serializes website releases and never supplies promotion secrets to unified operations', () => {
  const workflow = readFileSync(new URL('../workflows/web-assets-r2-release.yml', import.meta.url), 'utf8');
  assert.match(workflow, /concurrency:[\s\S]*?production-release[\s\S]*?cancel-in-progress: false/);
  assert.match(workflow, /persist-credentials: false/);
  assert.match(workflow, /website:[\s\S]*?timeout-minutes: 12/);
  const unified = workflow.split('- name: Stage or promote the two existing websites from one immutable source')[1].split('- name: Save paired immutable candidate identities')[0];
  assert.match(unified, /RELEASE_MAIN_DEPLOYMENT_ID:/);
  assert.doesNotMatch(unified, /MIR2_PROMOTION_.*SECRET/);
});
