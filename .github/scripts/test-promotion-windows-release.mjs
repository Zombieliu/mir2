import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import vm from 'node:vm';

const TEAM = 'team_pXY7oaLtibSTlKSmKXMXzrQR';
const BRANCH = 'codex/promotion-vercel-20261029';
const SHA = 'a'.repeat(40);
const WEB = 'prj_Bl3ttB4PzcdCpJMYLRhzQjkdXyTz';
const MAIN = 'prj_Q14gZuvjxkfqOCExhqOrYiR2UbxA';
const BASE = 'dpl_9yLef9wXkWDovACg9atHU1W1uBSx';
const MAIN_BASE = 'dpl_7krVTQSx5Wj9Ht8Sd5KF5o9d3F97';
const ID = 'dpl_windowscandidate';
const CAMPAIGN = 'numeron-windows-playtest-20261030';
const LEGACY = ['numeron-20261101-testnet', 'numeron-chain-acceptance-20261029-fullchain', 'numeron-playtest-20261029'];
const TOKEN = 'synthetic-ci-token-never-print';
const SESSION = 'synthetic-Windows-session-secret-at-least-32-never-print';
const WALLET = 'synthetic-Windows-wallet-secret-at-least-32-never-print';
const OLD = 'synthetic-existing-legacy-secret-never-print';
const PUBLIC = {
  MIR2_PROMOTION_WINDOWS_CAMPAIGN_ID: CAMPAIGN,
  MIR2_PROMOTION_WINDOWS_GATEWAY_HTTP_URL: 'https://api-playtest.165.154.65.136.sslip.io',
  MIR2_PROMOTION_WINDOWS_CHECKOUT_ORIGIN: 'https://billing-playtest.165.154.65.136.sslip.io',
  MIR2_PROMOTION_WINDOWS_PUBLIC_ORIGIN: 'https://mir2.obelisk.build',
  MIR2_PROMOTION_WINDOWS_ALLOWED_ORIGINS: 'https://mir2.obelisk.build',
  MIR2_PROMOTION_WINDOWS_DUBHE_CONNECT_NETWORK: 'testnet',
  MIR2_PROMOTION_WINDOWS_DUBHE_CONNECT_RELAY_URL: 'https://dubhe-connect-relay.zombiesliu.workers.dev',
  MIR2_PROMOTION_WINDOWS_DUBHE_WALLET_ORIGIN: 'https://wallet.dubhe.obelisk.build',
  MIR2_PROMOTION_LEGACY_CAMPAIGN_IDS: LEGACY.join(','),
};
const SECRET_KEYS = ['MIR2_PROMOTION_WINDOWS_SESSION_SECRET', 'MIR2_PROMOTION_WINDOWS_WALLET_AUTH_SECRET'];
const source = readFileSync(new URL('./promotion-windows-release.mjs', import.meta.url), 'utf8');
const executable = source.replace(/^import .+ from 'node:[^']+';\r?\n/gm, '');
assert.equal((source.match(/^import /gm) || []).length, 3, 'all three side-effect imports use closed mocks');
assert.ok(!/^import /m.test(executable));
const digest = createHash('sha256').update(JSON.stringify(PUBLIC)).digest('hex');

function fixture(operation = 'windows-promote') {
  const project = (id, name, baseline) => ({ id, name, accountId: TEAM, rootDirectory: 'mir2-web3/apps/web',
    targets: { production: { id: baseline } }, link: { type: 'github', org: 'Zombieliu', repo: 'mir2', repoId: 12345 } });
  const projects = { web: project(WEB, 'mir2-web3-web', BASE), main: project(MAIN, 'numeron-legend-of-rebirth', MAIN_BASE) };
  const candidate = { id: ID, projectId: WEB, target: 'production', readyState: 'READY', url: 'windows-candidate.vercel.app', gitSource: { sha: SHA },
    meta: { windowsSourceSha: SHA, windowsWebBaseline: BASE, windowsMainBaseline: MAIN_BASE, windowsRoot: 'mir2-web3/apps/web',
      windowsCampaignId: CAMPAIGN, windowsConfigurationDigest: digest } };
  const snapshot = { uid: ID, projectId: WEB, projectSettings: { rootDirectory: 'mir2-web3/apps/web' } };
  const envs = [
    { id: 'legacy-session', key: 'MIR2_PROMOTION_SESSION_SECRET', type: 'sensitive', target: ['production'] },
    { id: 'legacy-wallet', key: 'MIR2_PROMOTION_WALLET_AUTH_SECRET', type: 'sensitive', target: ['production'] },
    { id: 'legacy-campaign', key: 'MIR2_PROMOTION_CAMPAIGN_ID', value: 'numeron-playtest-20261029', type: 'plain', target: ['production'] },
    { id: 'marketing-link', key: 'NEXT_PUBLIC_SITE_URL', value: 'https://mir2.obelisk.build', type: 'plain', target: ['production'] },
    ...Object.entries(PUBLIC).map(([key, value]) => ({ id: `env-${key}`, key, value, type: 'plain', target: ['production'] })),
    ...SECRET_KEYS.map(key => ({ id: `env-${key}`, key, type: 'sensitive', target: ['production'] })),
  ];
  const env = { GITHUB_REF: `refs/heads/${BRANCH}`, GITHUB_SHA: SHA, RELEASE_OPERATION: operation,
    VERCEL_TOKEN: TOKEN, VERCEL_ORG_ID: TEAM, VERCEL_PROJECT_ID: WEB, RELEASE_DEPLOYMENT_ID: ID,
    [SECRET_KEYS[0]]: SESSION, [SECRET_KEYS[1]]: WALLET };
  const campaign = { campaignId: CAMPAIGN, environment: 'test', status: 'active', recruitmentEnabled: true,
    commissionEnabled: true, payoutEnabled: false, startsAtMs: 1793219738449, endsAtMs: 1793824538449 };
  const bootstrap = (gameClient, campaign) => JSON.stringify({ authenticated: false, characters: [], gameClient, campaign });
  const periods = [
    ['draft', false, false, false, 1793462400000, 1801411200000],
    ['locked', false, true, true, 1793258649320, 1793259549320],
    ['active', true, true, false, 1793219738449, 1793824538449],
  ];
  const pages = {
    '/version': JSON.stringify({ revision: SHA }),
    '/promotions?lang=zh-CN': '<html>Promotion program<link href="/_next/static/css/promotion.css?v=one&amp;theme=dark"><script src="/_next/static/chunks/promotion.js"></script></html>',
    '/_next/static/css/promotion.css': 'body { color: #ededdd; background: #10110d; }',
    '/_next/static/chunks/promotion.js': '/* Accrued commission (pending settlement); Updates every 10 seconds; data-promotion-live-updates; Create your character in the Windows game; promotion-migration-pending; promotion-migration-confirm; promotion-migration-completed */',
    '/api/promotions/bootstrap': bootstrap('windows', campaign),
    [`/api/promotions/bootstrap?campaignId=${CAMPAIGN}`]: bootstrap('windows', campaign),
    '/game': '<html><script src="/_next/static/chunks/game.js"></script></html>',
    '/manifest.webmanifest': JSON.stringify({ start_url: '/game', scope: '/' }),
    '/': '<html navigation_promotions footer_promotions><a href="/promotions?lang=zh-CN">Referral</a><link href="/marketing-assets/_next/static/site.css"><script src="/marketing-assets/_next/static/site.js"></script></html>',
    '/marketing-assets/_next/static/site.css': 'body { color: #ededdd; background: #10110d; }',
    '/marketing-assets/_next/static/site.js': 'window.numeronMarketingReady = true;',
    '/en': '<html lang="en"><a href="/promotions?lang=en">Referral</a></html>',
  };
  LEGACY.forEach((campaignId, index) => {
    const [status, recruitmentEnabled, commissionEnabled, payoutEnabled, startsAtMs, endsAtMs] = periods[index];
    pages[`/api/promotions/bootstrap?campaignId=${campaignId}`] = bootstrap('web', {
      campaignId, environment: 'test', status, recruitmentEnabled, commissionEnabled, payoutEnabled, startsAtMs, endsAtMs,
    });
  });
  const state = { projects, candidate, snapshot, envs, env, pages, calls: [], cli: [], logs: [], files: new Map(),
    aliases: new Map(['mir2.obelisk.build', 'mir2-web3-web.vercel.app'].map(alias => [alias, BASE])),
    onRequest: undefined, onCandidate: undefined, now: 0 };
  function page(path, url) {
    const text = state.pages[path] ?? state.pages[url.pathname];
    assert.notEqual(text, undefined, `explicit fixture path required: ${path}`);
    return text;
  }
  const fetch = async (input, init = {}) => {
    const url = new URL(input);
    const call = { origin: url.origin, path: url.pathname, fullPath: url.pathname + url.search, query: url.searchParams,
      method: init.method || 'GET', body: init.body ? JSON.parse(init.body) : undefined, headers: init.headers || {} };
    state.calls.push(call);
    const overridden = await state.onRequest?.(call, state);
    if (overridden !== undefined) return overridden;
    if (url.origin === 'https://mir2.obelisk.build') {
      assert.equal(call.method, 'GET');
      assert.equal(call.headers.Authorization, undefined, 'CI bearer never reaches public origin');
      return new Response(page(call.fullPath, url));
    }
    assert.equal(url.origin, 'https://api.vercel.com', 'no unmocked network destination');
    assert.equal(url.searchParams.get('teamId'), TEAM);
    assert.equal(call.headers.Authorization, `Bearer ${TOKEN}`);
    const project = call.path.match(/^\/v9\/projects\/([^/]+)$/);
    if (project) return Response.json(project[1] === WEB ? projects.web : projects.main);
    if (call.path === `/v10/projects/${WEB}/env`) {
      if (call.method === 'GET') {
        assert.equal(url.searchParams.get('decrypt'), 'false');
        return Response.json({ envs: state.envs });
      }
      assert.equal(call.method, 'POST');
      assert.equal(url.searchParams.get('upsert'), 'true');
      for (const entry of call.body) {
        const index = state.envs.findIndex(value => value.key === entry.key);
        const stored = { ...entry, id: index < 0 ? `new-${entry.key}` : state.envs[index].id };
        if (stored.type === 'sensitive') delete stored.value;
        if (index < 0) state.envs.push(stored); else state.envs[index] = stored;
      }
      return Response.json({ created: [] });
    }
    if (call.path === '/v7/deployments') {
      assert.equal(url.searchParams.get('projectId'), WEB);
      assert.equal(url.searchParams.get('sha'), env.RELEASE_SOURCE_SHA || env.GITHUB_SHA);
      return Response.json({ deployments: [state.snapshot], pagination: { next: null } });
    }
    if (call.path === `/v13/deployments/${BASE}`) return Response.json({ id: BASE, projectId: WEB, readyState: 'READY', target: 'production' });
    if (call.path === `/v13/deployments/${ID}`) return Response.json(state.candidate);
    if (call.path === '/v13/deployments' && call.method === 'POST') return Response.json({ ...state.candidate, readyState: 'BUILDING' });
    const alias = call.path.match(/^\/v4\/aliases\/(.+)$/);
    if (alias) return Response.json({ projectId: WEB, deploymentId: state.aliases.get(alias[1]) });
    const promote = call.path === `/v10/projects/${WEB}/promote/${ID}`;
    const rollback = call.path === `/v1/projects/${WEB}/rollback/${BASE}`;
    if (promote || rollback) {
      const target = promote ? ID : BASE;
      projects.web.targets.production.id = target;
      projects.web.lastAliasRequest = { toDeploymentId: target, jobStatus: 'succeeded' };
      for (const alias of state.aliases.keys()) state.aliases.set(alias, target);
      return Response.json({});
    }
    throw Error(`Unmocked API request ${call.method} ${call.path}`);
  };
  const execFileSync = (command, args, options) => {
    assert.equal(command, 'npx');
    assert.deepEqual(Array.from(args).slice(0, 3), ['--yes', 'vercel@56.4.1', 'curl']);
    assert.ok(args.includes('--fail') && args.includes('--max-time'));
    assert.ok(!args.some(value => [TOKEN, OLD, SESSION, WALLET].some(secret => String(value).includes(secret))));
    assert.equal(options.env, undefined);
    const url = new URL(args[3]);
    assert.equal(url.origin, 'https://windows-candidate.vercel.app');
    const path = url.pathname + url.search;
    state.cli.push(path);
    const overridden = state.onCandidate?.(path, state);
    return overridden === undefined ? page(path, url) : overridden;
  };
  async function run() {
    let error;
    class Clock extends Date { static now() { return state.now; } }
    try {
      await vm.runInNewContext(`(async () => {\n${executable}\n})()`, {
        process: { env }, fetch, AbortSignal, URL, JSON, createHash, Date: Clock,
        console: { log: (...args) => state.logs.push(args.join(' ')) },
        writeFile: async (path, data) => state.files.set(path, data), execFileSync,
        setTimeout: (callback, delay) => { state.now += delay; callback(); return 0; },
      }, { timeout: 2000, filename: 'closed-mock-promotion-windows-release.mjs' });
    } catch (value) { error = value; }
    return { ...state, error };
  }
  return { ...state, state, run };
}
const writes = result => result.calls.filter(call => call.method !== 'GET');
const failed = (result, pattern) => { assert.ok(result.error, 'must fail closed'); if (pattern) assert.match(String(result.error), pattern); };
const noSecrets = result => {
  const output = result.logs.join('\n') + [...result.files.values()].join('\n') + String(result.error || '');
  for (const secret of [TOKEN, OLD, SESSION, WALLET]) assert.ok(!output.includes(secret), 'credential absent from logs/receipts/errors');
};

test('immutable source branch and original CI identity fail before network I/O', async () => {
  for (const change of [{ GITHUB_REF: 'refs/heads/main' }, { GITHUB_SHA: 'moving-ref' }, { RELEASE_SOURCE_SHA: 'main' },
    { VERCEL_ORG_ID: 'wrong' }, { VERCEL_PROJECT_ID: MAIN }, { VERCEL_TOKEN: '' }]) {
    const f = fixture(); Object.assign(f.env, change);
    const r = await f.run(); failed(r); assert.equal(r.calls.length, 0);
  }
});
test('config and stage require the exact checked-out immutable commit', async () => {
  for (const operation of ['windows-config', 'windows-stage']) {
    const f = fixture(operation); f.env.RELEASE_SOURCE_SHA = 'b'.repeat(40);
    const r = await f.run(); failed(r); assert.equal(r.calls.length, 0);
  }
});
test('Windows context only reads and lists presence without secret values', async () => {
  const r = await fixture('windows-context').run();
  assert.ifError(r.error); assert.equal(writes(r).length, 0); noSecrets(r);
  assert.ok(r.logs.some(line => line.includes('productionPresent')));
});
test('config writes only Windows production keys plus exact legacy allowlist', async () => {
  const f = fixture('windows-config');
  const old = JSON.stringify(f.envs.filter(entry => !entry.key.startsWith('MIR2_PROMOTION_WINDOWS_') && entry.key !== 'MIR2_PROMOTION_LEGACY_CAMPAIGN_IDS'));
  const r = await f.run(); assert.ifError(r.error); noSecrets(r);
  assert.equal(writes(r).length, 1);
  const request = writes(r)[0]; assert.equal(request.path, `/v10/projects/${WEB}/env`);
  assert.deepEqual(request.body.map(entry => entry.key).sort(), [...Object.keys(PUBLIC), ...SECRET_KEYS].sort());
  for (const entry of request.body) {
    assert.deepEqual(entry.target, ['production']);
    assert.equal(entry.type, SECRET_KEYS.includes(entry.key) ? 'sensitive' : 'plain');
  }
  assert.equal(JSON.stringify(r.envs.filter(entry => !entry.key.startsWith('MIR2_PROMOTION_WINDOWS_') && entry.key !== 'MIR2_PROMOTION_LEGACY_CAMPAIGN_IDS')), old);
  assert.equal(r.projects.web.targets.production.id, BASE); assert.equal(r.projects.main.targets.production.id, MAIN_BASE);
});
test('config missing colliding short and multiline secrets fail before writes', async () => {
  for (const values of [['', WALLET], [SESSION, SESSION], ['short', WALLET], [SESSION + '\n', WALLET], [TOKEN, WALLET]]) {
    const f = fixture('windows-config'); [f.env[SECRET_KEYS[0]], f.env[SECRET_KEYS[1]]] = values;
    const r = await f.run(); failed(r); assert.equal(writes(r).length, 0); noSecrets(r);
  }
});
test('configuration cannot collapse a production and preview shared key', async () => {
  const f = fixture('windows-config'); f.envs.find(entry => entry.key === SECRET_KEYS[0]).target.push('preview');
  const r = await f.run(); failed(r, /shared/); assert.equal(writes(r).length, 0);
});
test('config detects concurrent changes to an existing legacy variable', async () => {
  const f = fixture('windows-config'); let metadataReads = 0;
  f.state.onRequest = call => {
    if (call.path === `/v10/projects/${WEB}/env` && call.method === 'GET' && ++metadataReads === 2) f.envs[0].id = 'changed-legacy';
  };
  const r = await f.run(); failed(r, /non-Windows/); assert.equal(writes(r).length, 1); noSecrets(r);
});
test('stage is exact production candidate with no domain assignment and no env writes', async () => {
  const r = await fixture('windows-stage').run(); assert.ifError(r.error); noSecrets(r);
  assert.equal(writes(r).length, 1); const body = writes(r)[0].body;
  assert.equal(body.project, WEB); assert.equal(body.target, 'production'); assert.equal(body.autoAssignCustomDomains, false);
  assert.equal(body.gitSource.sha, SHA); assert.equal(body.gitSource.ref, BRANCH);
  assert.equal(body.projectSettings.rootDirectory, 'mir2-web3/apps/web');
  assert.equal(body.meta.windowsConfigurationDigest, digest); assert.equal(body.meta.windowsCampaignId, CAMPAIGN);
  assert.equal(body.env, undefined); assert.equal(r.projects.web.targets.production.id, BASE);
});
test('stage rejects missing or mismatched Windows realm configuration', async () => {
  for (const key of ['MIR2_PROMOTION_WINDOWS_GATEWAY_HTTP_URL', 'MIR2_PROMOTION_WINDOWS_CHECKOUT_ORIGIN',
    'MIR2_PROMOTION_WINDOWS_DUBHE_CONNECT_NETWORK', 'MIR2_PROMOTION_LEGACY_CAMPAIGN_IDS', ...SECRET_KEYS]) {
    const f = fixture('windows-stage'); f.envs.find(entry => entry.key === key).value = 'wrong';
    if (SECRET_KEYS.includes(key)) f.envs.find(entry => entry.key === key).type = 'encrypted';
    const r = await f.run(); failed(r); assert.equal(writes(r).length, 0);
  }
});
test('concurrent game or marketing baseline change blocks all writes', async () => {
  for (const key of ['web', 'main']) {
    const f = fixture(); f.projects[key].targets.production.id = 'dpl_other';
    const r = await f.run(); failed(r, /production changed/); assert.equal(writes(r).length, 0);
  }
});
test('wrong source root readiness baseline campaign or configuration metadata cannot promote', async () => {
  for (const mutate of [f => f.candidate.gitSource.sha = 'b'.repeat(40), f => f.snapshot.projectSettings.rootDirectory = 'wrong',
    f => f.candidate.readyState = 'BUILDING', f => f.candidate.meta.windowsWebBaseline = 'dpl_wrong',
    f => f.candidate.meta.windowsCampaignId = LEGACY[0], f => f.candidate.meta.windowsConfigurationDigest = 'wrong']) {
    const f = fixture(); mutate(f); const r = await f.run(); failed(r); assert.equal(writes(r).length, 0);
  }
});
test('protected default Windows realm period commission and payout-off checks precede promotion', async () => {
  for (const change of [{ gameClient: 'web' }, { campaign: { payoutEnabled: true } }, { campaign: { startsAtMs: 0 } },
    { campaign: { commissionEnabled: false } }]) {
    const f = fixture(); const value = JSON.parse(f.pages['/api/promotions/bootstrap']);
    if (change.gameClient) value.gameClient = change.gameClient; else Object.assign(value.campaign, change.campaign);
    f.pages['/api/promotions/bootstrap'] = JSON.stringify(value);
    const r = await f.run(); failed(r, /Windows campaign/); assert.equal(writes(r).length, 0);
  }
});
test('candidate missing migration UI assets cannot be promoted even with matching revision', async () => {
  const f = fixture();
  f.pages['/_next/static/chunks/promotion.js'] = f.pages['/_next/static/chunks/promotion.js'].replace('promotion-migration-confirm', 'absent');
  const r = await f.run(); failed(r, /Promotion implementation is missing/); assert.equal(writes(r).length, 0);
});
test('each explicit legacy ID stays web realm with unchanged historical flags and dates', async () => {
  for (const legacy of LEGACY) {
    const f = fixture(); const path = `/api/promotions/bootstrap?campaignId=${legacy}`;
    const value = JSON.parse(f.pages[path]); value.gameClient = 'windows'; f.pages[path] = JSON.stringify(value);
    const r = await f.run(); failed(r, /legacy campaign/); assert.equal(writes(r).length, 0);
  }
  const f = fixture(); const path = `/api/promotions/bootstrap?campaignId=${LEGACY[1]}`;
  const value = JSON.parse(f.pages[path]); value.campaign.payoutEnabled = false; f.pages[path] = JSON.stringify(value);
  const r = await f.run(); failed(r, /legacy campaign period/); assert.equal(writes(r).length, 0);
});
test('successful promotion checks protected and public Windows legacy main game and assets', async () => {
  const r = await fixture().run(); assert.ifError(r.error); noSecrets(r);
  assert.deepEqual(writes(r).map(call => call.path), [`/v10/projects/${WEB}/promote/${ID}`]);
  for (const campaign of [CAMPAIGN, ...LEGACY]) {
    const path = `/api/promotions/bootstrap?campaignId=${campaign}`;
    assert.ok(r.cli.includes(path)); assert.ok(r.calls.some(call => call.origin === 'https://mir2.obelisk.build' && call.fullPath === path));
  }
  assert.equal(r.projects.web.targets.production.id, ID); assert.equal(r.projects.main.targets.production.id, MAIN_BASE);
  assert.ok([...r.aliases.values()].every(value => value === ID));
});
test('production pointer alone is insufficient until alias job and both aliases converge', async () => {
  const f = fixture(); let polls = 0;
  f.state.onRequest = call => {
    if (call.path === `/v9/projects/${WEB}` && f.projects.web.targets.production.id === ID && ++polls < 3) {
      return Response.json({ ...f.projects.web, lastAliasRequest: { toDeploymentId: ID, jobStatus: 'in-progress' } });
    }
  };
  const r = await f.run(); assert.ifError(r.error); assert.ok(r.now >= 2000); assert.equal(writes(r).length, 1);
});
test('a successful alias job still waits for the second public alias assignment', async () => {
  const f = fixture(); let reads = 0;
  f.state.onRequest = call => {
    if (call.path === '/v4/aliases/mir2-web3-web.vercel.app' && f.projects.web.targets.production.id === ID && ++reads < 3) {
      return Response.json({ projectId: WEB, deploymentId: BASE });
    }
  };
  const r = await f.run(); assert.ifError(r.error); assert.ok(reads >= 3); assert.ok(r.now >= 4000);
});
test('failed assignment job triggers only guarded game website compensation', async () => {
  const f = fixture();
  f.state.onRequest = call => {
    if (call.path === `/v9/projects/${WEB}` && f.projects.web.targets.production.id === ID) {
      return Response.json({ ...f.projects.web, lastAliasRequest: { toDeploymentId: ID, jobStatus: 'failed' } });
    }
  };
  const r = await f.run(); failed(r, /alias assignment failed/);
  assert.deepEqual(writes(r).map(call => call.path), [`/v10/projects/${WEB}/promote/${ID}`, `/v1/projects/${WEB}/rollback/${BASE}`]);
  assert.equal(r.projects.web.targets.production.id, BASE);
});
test('brief public stale version is retried before declaring promotion failed', async () => {
  const f = fixture(); let reads = 0;
  f.state.onRequest = call => {
    if (call.origin === 'https://mir2.obelisk.build' && call.path === '/version' && ++reads === 1) return Response.json({ revision: 'b'.repeat(40) });
  };
  const r = await f.run(); assert.ifError(r.error); assert.ok(reads >= 2); assert.equal(writes(r).length, 1);
});
test('persistent public failure compensates only the website alias, never backend or marketing', async () => {
  const f = fixture();
  f.state.onRequest = call => call.origin === 'https://mir2.obelisk.build' && call.path === '/version' ? Response.json({ revision: 'b'.repeat(40) }) : undefined;
  const r = await f.run(); failed(r, /revision/); noSecrets(r);
  assert.deepEqual(writes(r).map(call => call.path), [`/v10/projects/${WEB}/promote/${ID}`, `/v1/projects/${WEB}/rollback/${BASE}`]);
  assert.equal(r.projects.web.targets.production.id, BASE); assert.equal(r.projects.main.targets.production.id, MAIN_BASE);
});
test('uncertain promote that already applied is included in guarded website compensation', async () => {
  const f = fixture();
  f.state.onRequest = call => {
    if (call.path === `/v10/projects/${WEB}/promote/${ID}`) { f.projects.web.targets.production.id = ID; throw Error('synthetic timeout'); }
  };
  const r = await f.run(); failed(r, /timeout/); assert.equal(r.projects.web.targets.production.id, BASE);
  assert.equal(writes(r).length, 2); noSecrets(r);
});
test('rollback never overwrites a concurrent third deployment', async () => {
  const f = fixture();
  f.state.onRequest = call => {
    if (call.origin === 'https://mir2.obelisk.build' && call.path === '/version') {
      f.projects.web.targets.production.id = 'dpl_thirdparty'; return Response.json({ revision: 'b'.repeat(40) });
    }
  };
  const r = await f.run(); failed(r, /concurrently/); assert.equal(writes(r).length, 1);
  assert.equal(r.projects.web.targets.production.id, 'dpl_thirdparty');
});
test('candidate CLI and API errors do not expose credentials or raw error response bodies', async () => {
  const f = fixture(); f.state.onCandidate = () => { throw Error(SESSION + OLD); };
  const r = await f.run(); failed(r, /Authenticated candidate HTTP check failed/); noSecrets(r); assert.equal(writes(r).length, 0);
  const g = fixture('windows-context'); g.state.onRequest = () => Response.json({ error: { code: 'FORBIDDEN', message: SESSION + TOKEN } }, { status: 403 });
  const s = await g.run(); failed(s, /403/); noSecrets(s);
});
