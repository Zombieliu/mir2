import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { TEAM, BRANCH, REPOSITORY, SITES, ORIGIN, deploymentBody } from './numeron-r16-release.mjs';
import { DOMAIN, runNumeronDomain, forwardBody } from './numeron-r16-domain.mjs';

const SHA = 'a'.repeat(40), TOOL_SHA = 'b'.repeat(40), TOKEN = 'synthetic-domain-ci-token';
const IDS = { main: 'dpl_domainmain', game: 'dpl_domaingame' };
const GAME = 'numeron-r16-web.vercel.app', MAIN = 'numeron-legend-of-rebirth.vercel.app';
const json = value => Response.json(value);

function fixture() {
  const config = JSON.parse(readFileSync(new URL('../../mir2-web3/config/numeron-r16-web-assets.json', import.meta.url), 'utf8'));
  const env = { RELEASE_OPERATION: 'numeron-domain-move', RELEASE_SOURCE_SHA: SHA, GITHUB_SHA: TOOL_SHA,
    GITHUB_REF: `refs/heads/${BRANCH}`, GITHUB_REPOSITORY: REPOSITORY, VERCEL_ORG_ID: TEAM,
    VERCEL_PROJECT_ID: SITES.original.project, VERCEL_TOKEN: TOKEN,
    RELEASE_MAIN_DEPLOYMENT_ID: IDS.main, RELEASE_DEPLOYMENT_ID: IDS.game };
  const projects = Object.fromEntries(Object.entries(SITES).map(([key, site]) => [key, {
    id: site.project, name: site.name, accountId: TEAM,
    targets: { production: { id: key === 'original' ? site.baseline : IDS[key] } },
  }]));
  const candidates = Object.fromEntries(['main', 'game'].map(key => [key, {
    id: IDS[key], projectId: SITES[key].project, url: `${key}-domain-candidate.vercel.app`, readyState: 'READY', target: 'production',
    gitSource: { ref: BRANCH, sha: SHA },
    meta: deploymentBody(key, SHA, 123, config, 'synthetic-gateway-secret-long-enough-for-fixtures').meta,
  }]));
  const snapshots = Object.fromEntries(['main', 'game'].map(key => [key, { uid: IDS[key], projectId: SITES[key].project }]));
  const alias = (hostname, key) => ({ alias: hostname, projectId: SITES[key].project, deploymentId: IDS[key], redirect: null });
  const state = { env, config, projects, candidates, snapshots,
    domains: { main: { name: DOMAIN, projectId: SITES.main.project, verified: true, updatedAt: 100,
      gitBranch: null, customEnvironmentId: null, redirect: null, redirectStatusCode: null }, game: null },
    aliases: { [GAME]: alias(GAME, 'game'), [MAIN]: alias(MAIN, 'main'), [DOMAIN]: alias(DOMAIN, 'main') },
    others: [], gameOthers: [], calls: [], receipts: [], onRequest: null, onPublic: null };
  function keyFor(project) { return Object.keys(SITES).find(key => SITES[key].project === project); }
  state.applyMove = (source, body) => {
    const from = keyFor(source), to = keyFor(body.projectId), previous = state.domains[from];
    assert(previous, 'move source must own the domain');
    const next = { ...previous, projectId: body.projectId, gitBranch: body.gitBranch,
      redirect: body.redirect, redirectStatusCode: body.redirectStatusCode, updatedAt: previous.updatedAt + 1 };
    state.domains[from] = null; state.domains[to] = next;
    state.aliases[DOMAIN] = body.redirect === null ? alias(DOMAIN, to) : {
      alias: DOMAIN, projectId: body.projectId, deploymentId: null, redirect: body.redirect, redirectStatusCode: body.redirectStatusCode,
    };
    return next;
  };
  async function fetchImpl(url, options = {}) {
    const u = new URL(url), req = { url, pathname: u.pathname, method: options.method || 'GET',
      headers: options.headers || {}, body: options.body ? JSON.parse(options.body) : undefined };
    state.calls.push(req);
    const override = await state.onRequest?.(req, state);
    if (override !== undefined) return override;
    if (u.origin !== 'https://api.vercel.com') {
      assert.equal(req.method, 'GET'); assert.equal(req.headers.Authorization, undefined, 'CI bearer never reaches public pages');
      assert(['https://' + GAME, 'https://' + MAIN, ORIGIN].includes(u.origin));
      const overridden = await state.onPublic?.(req, state); if (overridden !== undefined) return overridden;
      if (u.pathname === '/version') return json({ revision: SHA });
      if (u.pathname === '/manifest.webmanifest') return json({ start_url: '/game', scope: '/' });
      if (u.pathname === '/game') return new Response('<html><script src="/_next/static/game.js"></script></html>');
      if (['/', '/en', '/zh-CN'].includes(u.pathname)) return new Response('<html data-track="navigation_promotions" data-footer="footer_promotions"><script src="/marketing-assets/_next/static/site.js"></script></html>');
      if (u.pathname === '/zh-CN/download') return new Response(`<a href="${ORIGIN}/game">Play</a><a href="https://assets.mir2.obelisk.build/fixture/Mir2Setup.exe" data-track="download_windows">Windows</a>`);
      throw Error('unmocked public path');
    }
    assert.equal(u.searchParams.get('teamId'), TEAM); assert.equal(req.headers.Authorization, `Bearer ${TOKEN}`);
    let match = u.pathname.match(/^\/v9\/projects\/([^/]+)$/);
    if (match) return json(projects[keyFor(match[1])]);
    match = u.pathname.match(/^\/v13\/deployments\/([^/]+)$/);
    if (match) return json(Object.values(candidates).find(value => value.id === match[1]));
    if (u.pathname === '/v7/deployments') return json({ deployments: [snapshots[keyFor(u.searchParams.get('projectId'))]], pagination: {} });
    match = u.pathname.match(/^\/v9\/projects\/([^/]+)\/domains\/numeron\.obelisk\.build$/);
    if (match) { const value = state.domains[keyFor(match[1])]; return value ? json(value) : new Response('', { status: 404 }); }
    match = u.pathname.match(/^\/v9\/projects\/([^/]+)\/domains$/);
    if (match) {
      const key = keyFor(match[1]); assert(['main', 'game'].includes(key));
      return json({ domains: [...(key === 'main' ? state.others : state.gameOthers), ...(state.domains[key] ? [state.domains[key]] : [])], pagination: {} });
    }
    match = u.pathname.match(/^\/v4\/aliases\/([^/]+)$/);
    if (match) return state.aliases[match[1]] ? json(state.aliases[match[1]]) : new Response('', { status: 404 });
    match = u.pathname.match(/^\/v1\/projects\/([^/]+)\/domains\/numeron\.obelisk\.build\/move$/);
    if (match) { assert.equal(req.method, 'POST'); return json(state.applyMove(match[1], req.body)); }
    throw Error('unmocked API path');
  }
  async function execute() {
    let result, error;
    try { result = await runNumeronDomain({ env, fetchImpl, readConfig: async () => config,
      sleep: async () => {}, onReceipt: async receipt => state.receipts.push(structuredClone(receipt)) }); }
    catch (caught) { error = caught; }
    return { ...state, result, error };
  }
  return { ...state, state, execute };
}
const writes = value => value.calls.filter(call => call.method !== 'GET');
const failed = (value, pattern) => { assert(value.error, 'must fail closed'); if (pattern) assert.match(value.error.code || '', pattern); };

test('explicit domain operation, CI identity and exact source fail before any network', async () => {
  for (const change of [{ RELEASE_OPERATION: 'numeron-promote' }, { VERCEL_ORG_ID: 'wrong' },
    { VERCEL_PROJECT_ID: SITES.game.project }, { GITHUB_REF: 'refs/heads/main' },
    { RELEASE_SOURCE_SHA: 'moving-ref' }, { RELEASE_DEPLOYMENT_ID: '' }]) {
    const f = fixture(); Object.assign(f.env, change); const r = await f.execute(); failed(r); assert.equal(r.calls.length, 0);
  }
});
test('one exact move follows default alias body proofs and preserves all three production mappings', async () => {
  const f = fixture(), r = await f.execute(); assert.ifError(r.error);
  assert.equal(writes(r).length, 1); assert(writes(r)[0].pathname.includes(SITES.main.project)); assert.deepEqual(writes(r)[0].body, forwardBody());
  assert.equal(r.domains.main, null); assert.equal(r.domains.game.projectId, SITES.game.project);
  assert.equal(r.result.serverSideCompareAndSwap, false); assert.equal(r.result.manualReconcile, false);
  for (const key of Object.keys(SITES)) assert.equal(r.projects[key].targets.production.id, key === 'original' ? SITES.original.baseline : IDS[key]);
  assert(!JSON.stringify(r.receipts).includes(TOKEN)); assert.equal(r.result.implementationSha, TOOL_SHA); assert.equal(r.result.sourceSha, SHA);
  const at = r.calls.findIndex(call => call.method === 'POST');
  for (const pathname of ['/version', '/game', '/', '/zh-CN/download']) assert(r.calls.slice(0, at).some(call => call.url === `https://${GAME}${pathname}`));
  assert(writes(r).every(call => /\/v1\/projects\/[^/]+\/domains\/numeron\.obelisk\.build\/move$/.test(call.pathname)));
});
test('foreign/current project generations, candidate metadata and explicit wrong roots reject before mutation', async () => {
  for (const change of [f => { f.projects.original.targets.production.id = 'dpl_other'; },
    f => { f.projects.main.targets.production.id = 'dpl_other'; }, f => { f.projects.game.accountId = 'other'; },
    f => { f.candidates.game.gitSource.sha = TOOL_SHA; }, f => { delete f.candidates.main.meta.numeronR16Root; },
    f => { f.snapshots.game.projectSettings = { rootDirectory: SITES.main.root }; }]) {
    const f = fixture(); change(f); const r = await f.execute(); failed(r); assert.equal(writes(r).length, 0);
  }
});
test('wrong default alias and failed real public version/game/home/download proofs cannot move', async () => {
  const changes = [f => { f.aliases[GAME].deploymentId = 'dpl_other'; },
    f => { f.state.onPublic = req => req.pathname === '/version' ? json({ revision: TOOL_SHA }) : undefined; },
    f => { f.state.onPublic = req => req.pathname === '/game' ? new Response('<html>Wrong app</html>') : undefined; },
    f => { f.state.onPublic = req => req.pathname === '/' ? new Response('<html>Wrong home</html>') : undefined; },
    f => { f.state.onPublic = req => req.url === `https://${GAME}/zh-CN/download` ? new Response(`<a href="${ORIGIN}/game">Play</a><a data-track="download_windows" href="https://assets.mir2.obelisk.build/changed.exe">Windows</a>`) : undefined; }];
  for (const change of changes) { const f = fixture(); change(f); const r = await f.execute(); failed(r); assert.equal(writes(r).length, 0); }
});
test('unverified, branch-bound, custom-environment or mismatched source ownership cannot move', async () => {
  for (const change of [f => { f.domains.main.verified = false; }, f => { f.domains.main.gitBranch = BRANCH; },
    f => { f.domains.main.customEnvironmentId = 'env_other'; }, f => { f.aliases[DOMAIN].deploymentId = 'dpl_other'; },
    f => { f.domains.main.projectId = SITES.game.project; }]) {
    const f = fixture(); change(f); const r = await f.execute(); failed(r); assert.equal(writes(r).length, 0);
  }
});
test('inbound redirects are enumerated and refuse the possible implicit multi-domain move', async () => {
  const f = fixture(); f.state.others.push({ name: 'www.numeron.obelisk.build', projectId: SITES.main.project, redirect: DOMAIN });
  const r = await f.execute(); failed(r, /domainHasInboundRedirects/); assert.equal(writes(r).length, 0);
  assert.deepEqual(r.receipts[0].inboundRedirects, ['www.numeron.obelisk.build']);
});
test('a changed source binding during final preflight is not overwritten', async () => {
  const f = fixture(); let reads = 0;
  f.state.onRequest = req => { if (req.pathname.endsWith(`/domains/${DOMAIN}`) && req.pathname.includes(SITES.main.project) && ++reads === 2) f.domains.main.updatedAt++; };
  const r = await f.execute(); failed(r, /domainConcurrentChangeBeforeMove/); assert.equal(writes(r).length, 0);
});
test('lost forward response is reconciled through reads without a second POST', async () => {
  const f = fixture(); f.state.onRequest = req => {
    if (req.method === 'POST') { f.state.applyMove(SITES.main.project, req.body); throw Error('synthetic lost acknowledgement'); }
  };
  const r = await f.execute(); assert.ifError(r.error); assert.equal(writes(r).length, 1);
  assert.equal(r.result.moveResponseUnknown, true); assert.equal(r.result.manualReconcile, false);
});
test('delayed alias convergence after a successful move is observed through reads', async () => {
  const f = fixture(); let aliasReads = 0;
  f.state.onRequest = req => {
    if (req.method === 'POST') {
      const result = f.state.applyMove(SITES.main.project, req.body);
      f.aliases[DOMAIN] = { alias: DOMAIN, projectId: SITES.main.project, deploymentId: IDS.main, redirect: null };
      return json(result);
    }
    if (f.domains.game && req.pathname === `/v4/aliases/${DOMAIN}` && ++aliasReads === 2)
      f.aliases[DOMAIN] = { alias: DOMAIN, projectId: SITES.game.project, deploymentId: IDS.game, redirect: null };
  };
  const r = await f.execute(); assert.ifError(r.error); assert.equal(writes(r).length, 1); assert(aliasReads >= 2);
});
test('unknown unapplied move requires manual reconciliation and never blindly retries', async () => {
  const f = fixture(); f.state.onRequest = req => { if (req.method === 'POST') throw Error('synthetic unknown'); };
  const r = await f.execute(); failed(r, /domainMoveNotConverged/); assert.equal(writes(r).length, 1);
  assert.equal(r.receipts[0].manualReconcile, true); assert.equal(r.domains.main.projectId, SITES.main.project);
});
test('public failure reverses one owned domain move and restores source redirect configuration exactly', async () => {
  const f = fixture(); Object.assign(f.domains.main, { redirect: 'mir2.obelisk.build', redirectStatusCode: 307 });
  f.aliases[DOMAIN] = { alias: DOMAIN, projectId: SITES.main.project, deploymentId: null, redirect: 'mir2.obelisk.build', redirectStatusCode: 307 };
  f.state.onPublic = req => req.url.startsWith(ORIGIN) ? json({ revision: TOOL_SHA }) : undefined;
  const r = await f.execute(); failed(r); assert.equal(writes(r).length, 2);
  assert.deepEqual(writes(r)[1].body, { projectId: SITES.main.project, gitBranch: null, redirect: 'mir2.obelisk.build', redirectStatusCode: 307 });
  assert.equal(r.domains.main.redirectStatusCode, 307); assert.equal(r.receipts[0].rollback.confirmed, true);
});
test('changed game generation, domain attributes or alias after move prevent rollback over another operator', async () => {
  for (const change of [f => { f.projects.game.targets.production.id = 'dpl_external'; },
    f => { f.domains.game.updatedAt++; }, f => { f.aliases[DOMAIN].deploymentId = 'dpl_external'; }]) {
    const f = fixture(); let changed = false;
    f.state.onPublic = req => { if (req.url.startsWith(ORIGIN)) { if (!changed) { change(f); changed = true; } return json({ revision: TOOL_SHA }); } };
    const r = await f.execute(); failed(r); assert.equal(writes(r).length, 1); assert.equal(r.receipts[0].manualReconcile, true);
  }
});
test('reverse move refuses newly added inbound redirects instead of carrying another domain back', async () => {
  const f = fixture(); f.state.onPublic = req => {
    if (req.url.startsWith(ORIGIN)) {
      if (!f.state.gameOthers.length) f.state.gameOthers.push({ name: 'external.example.com', projectId: SITES.game.project, redirect: DOMAIN });
      return json({ revision: TOOL_SHA });
    }
  };
  const r = await f.execute(); failed(r); assert.equal(writes(r).length, 1);
  assert.equal(r.receipts[0].rollback.code, 'rollbackDomainHasInboundRedirects'); assert.equal(r.receipts[0].manualReconcile, true);
});
test('lost reverse response uses GET reconciliation, not a repeated reverse mutation', async () => {
  const f = fixture(); f.state.onPublic = req => req.url.startsWith(ORIGIN) ? json({ revision: TOOL_SHA }) : undefined;
  f.state.onRequest = req => {
    if (req.method === 'POST' && req.pathname.includes(SITES.game.project)) { f.state.applyMove(SITES.game.project, req.body); throw Error('synthetic lost rollback acknowledgement'); }
  };
  const r = await f.execute(); failed(r); assert.equal(writes(r).length, 2);
  assert.equal(r.receipts[0].rollbackResponseUnknown, true); assert.equal(r.receipts[0].rollback.confirmed, true);
});
test('an already-correct target is read back without any mutation', async () => {
  const f = fixture(); f.state.applyMove(SITES.main.project, forwardBody());
  const r = await f.execute(); assert.ifError(r.error); assert.equal(writes(r).length, 0); assert.equal(r.result.alreadyAtTarget, true);
});
test('public redirects never carry the CI token or allow a different origin', async () => {
  const f = fixture(); f.state.onPublic = req => req.pathname === '/' ? new Response('', { status: 307, headers: { location: 'https://unrelated.invalid/' } }) : undefined;
  const r = await f.execute(); failed(r, /domainPublicForeignRedirect/); assert.equal(writes(r).length, 0);
  assert.equal(r.calls.some(call => call.url.startsWith('https://unrelated.invalid')), false);
});
test('paginated source redirects and denied target reads are never mistaken for an empty ownership scope', async () => {
  const f = fixture(); f.state.onRequest = req => {
    if (req.pathname === `/v9/projects/${SITES.main.project}/domains`) {
      if (!new URL(req.url).searchParams.has('until')) return json({ domains: [f.domains.main], pagination: { next: 123 } });
      return json({ domains: [{ name: 'late.example.com', projectId: SITES.main.project, redirect: DOMAIN }], pagination: {} });
    }
  };
  const r = await f.execute(); failed(r, /domainHasInboundRedirects/); assert.equal(writes(r).length, 0);
  const denied = fixture(); denied.state.onRequest = req => req.pathname === `/v9/projects/${SITES.game.project}/domains/${DOMAIN}` ? new Response('', { status: 403 }) : undefined;
  const result = await denied.execute(); failed(result, /domainApiHttp403/); assert.equal(writes(result).length, 0);
});
test('404 is optional only for scoped domain and alias reads, never projects, candidates or domain lists', async () => {
  for (const endpoint of [`/v9/projects/${SITES.original.project}`, `/v13/deployments/${IDS.game}`,
    '/v7/deployments', `/v9/projects/${SITES.main.project}/domains`]) {
    const f = fixture(); f.state.onRequest = req => req.pathname === endpoint ? new Response('', { status: 404 }) : undefined;
    const r = await f.execute(); failed(r, /domainApiHttp404/); assert.equal(writes(r).length, 0);
  }
  const missing = fixture(); missing.domains.main = null;
  const noOwner = await missing.execute(); failed(noOwner, /sourceDomainOwnershipMismatch/); assert.equal(writes(noOwner).length, 0);
  const noAlias = fixture(); delete noAlias.aliases[MAIN];
  const aliasFailure = await noAlias.execute(); failed(aliasFailure, /marketingDefaultAliasMismatch/); assert.equal(writes(aliasFailure).length, 0);
  const fallback = fixture(); delete fallback.aliases[GAME];
  fallback.aliases['numeron-r16-web-obelisk-labs.vercel.app'] = { alias: 'numeron-r16-web-obelisk-labs.vercel.app', projectId: SITES.game.project, deploymentId: IDS.game, redirect: null };
  fallback.state.onRequest = req => {
    if (req.url.startsWith('https://numeron-r16-web-obelisk-labs.vercel.app/')) {
      // Both default hostnames are known and the alternate still proves the exact deployment.
      const path = new URL(req.url).pathname;
      if (path === '/version') return json({ revision: SHA });
      if (path === '/manifest.webmanifest') return json({ start_url: '/game', scope: '/' });
      if (path === '/game') return new Response('<html><script src="/_next/static/game.js"></script></html>');
      if (path === '/') return new Response('<html data-track="navigation_promotions" data-footer="footer_promotions"><script src="/marketing-assets/_next/static/site.js"></script></html>');
      if (path === '/zh-CN/download') return new Response(`<a href="${ORIGIN}/game">Play</a><a href="https://assets.mir2.obelisk.build/fixture/Mir2Setup.exe" data-track="download_windows">Windows</a>`);
    }
  };
  const alternate = await fallback.execute(); assert.ifError(alternate.error); assert.equal(writes(alternate).length, 1);
});
test('a concurrent third-project generation plus 404 cannot be swallowed as missing domain or trigger rollback', async () => {
  const f = fixture(); let changed = false;
  f.state.onPublic = req => {
    if (req.url.startsWith(ORIGIN)) { f.projects.original.targets.production.id = 'dpl_external'; changed = true; return json({ revision: TOOL_SHA }); }
  };
  f.state.onRequest = req => changed && req.pathname === `/v9/projects/${SITES.original.project}` ? new Response('', { status: 404 }) : undefined;
  const r = await f.execute(); failed(r); assert.equal(writes(r).length, 1);
  assert.equal(r.projects.original.targets.production.id, 'dpl_external'); assert.equal(r.receipts[0].manualReconcile, true);
  assert.equal(r.receipts[0].rollback.code, 'domainApiHttp404');
});
test('public root may redirect to same-origin en or zh-CN without altering the marketing baseline', async () => {
  for (const locale of ['/en', '/zh-CN']) {
    const f = fixture(); f.state.onPublic = req => new URL(req.url).pathname === '/' ? new Response('', { status: 307, headers: { location: locale } }) : undefined;
    const r = await f.execute(); assert.ifError(r.error); assert.equal(writes(r).length, 1);
    assert(r.calls.some(req => req.url === `https://${GAME}${locale}`)); assert(r.calls.some(req => req.url === `${ORIGIN}${locale}`));
    assert.equal(r.projects.main.targets.production.id, IDS.main);
    assert.equal(r.calls.some(req => req.pathname === `/v13/deployments/${SITES.main.baseline}`), false);
  }
});
