import { writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const TEAM = 'team_pXY7oaLtibSTlKSmKXMXzrQR';
const BRANCH = 'codex/promotion-vercel-20261029';
const WEB = { project: 'prj_Bl3ttB4PzcdCpJMYLRhzQjkdXyTz', name: 'mir2-web3-web',
  baseline: 'dpl_9yLef9wXkWDovACg9atHU1W1uBSx', root: 'mir2-web3/apps/web' };
const MAIN = { project: 'prj_Q14gZuvjxkfqOCExhqOrYiR2UbxA', name: 'numeron-legend-of-rebirth',
  baseline: 'dpl_7krVTQSx5Wj9Ht8Sd5KF5o9d3F97' };
const CAMPAIGN = 'numeron-windows-playtest-20261030';
const LEGACY_CAMPAIGNS = ['numeron-20261101-testnet', 'numeron-chain-acceptance-20261029-fullchain', 'numeron-playtest-20261029'];
// Windows uses original gameplay identity and financial history. Legacy profile
// secrets and all three historical campaigns remain separately addressable.
const WINDOWS_PUBLIC = Object.freeze({
  MIR2_PROMOTION_WINDOWS_CAMPAIGN_ID: CAMPAIGN,
  MIR2_PROMOTION_WINDOWS_GATEWAY_HTTP_URL: 'https://api-playtest.165.154.65.136.sslip.io',
  MIR2_PROMOTION_WINDOWS_CHECKOUT_ORIGIN: 'https://billing-playtest.165.154.65.136.sslip.io',
  MIR2_PROMOTION_WINDOWS_PUBLIC_ORIGIN: 'https://mir2.obelisk.build',
  MIR2_PROMOTION_WINDOWS_ALLOWED_ORIGINS: 'https://mir2.obelisk.build',
  MIR2_PROMOTION_WINDOWS_DUBHE_CONNECT_NETWORK: 'testnet',
  MIR2_PROMOTION_WINDOWS_DUBHE_CONNECT_RELAY_URL: 'https://dubhe-connect-relay.zombiesliu.workers.dev',
  MIR2_PROMOTION_WINDOWS_DUBHE_WALLET_ORIGIN: 'https://wallet.dubhe.obelisk.build',
  MIR2_PROMOTION_LEGACY_CAMPAIGN_IDS: LEGACY_CAMPAIGNS.join(','),
});
const WINDOWS_SECRET_KEYS = ['MIR2_PROMOTION_WINDOWS_SESSION_SECRET', 'MIR2_PROMOTION_WINDOWS_WALLET_AUTH_SECRET'];
const WINDOWS_KEYS = new Set([...Object.keys(WINDOWS_PUBLIC), ...WINDOWS_SECRET_KEYS]);
const configurationDigest = createHash('sha256').update(JSON.stringify(WINDOWS_PUBLIC)).digest('hex');
const WEB_ALIASES = ['mir2.obelisk.build', 'mir2-web3-web.vercel.app'];
const operation = process.env.RELEASE_OPERATION;
const sha = process.env.RELEASE_SOURCE_SHA || process.env.GITHUB_SHA;
if (!['windows-context', 'windows-config', 'windows-stage', 'windows-promote'].includes(operation)) throw Error('Unsupported Windows release operation');
if (process.env.GITHUB_REF !== `refs/heads/${BRANCH}` || !/^[a-f0-9]{40}$/.test(process.env.GITHUB_SHA || '') ||
    !/^[a-f0-9]{40}$/.test(sha || '')) throw Error('Unexpected immutable Windows release source');
if (['windows-stage', 'windows-config'].includes(operation) && sha !== process.env.GITHUB_SHA) throw Error('Stage/config must use the checked-out workflow commit');
if (!process.env.VERCEL_TOKEN || process.env.VERCEL_ORG_ID !== TEAM || process.env.VERCEL_PROJECT_ID !== WEB.project) {
  throw Error('Existing CI identity does not match the original game website');
}

async function api(path, method = 'GET', body) {
  const response = await fetch(`https://api.vercel.com${path}${path.includes('?') ? '&' : '?'}teamId=${TEAM}`, {
    method, headers: { Authorization: `Bearer ${process.env.VERCEL_TOKEN}`, 'Content-Type': 'application/json' },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }), signal: AbortSignal.timeout(30000),
  });
  if (!response.ok) {
    const value = await response.json().catch(() => ({}));
    // Never print API response bodies, request headers, environment values or credentials.
    const code = String(value.error?.code || 'unknown').replace(/[^a-zA-Z0-9_-]/g, '');
    throw Error(`Vercel ${method} ${path.split('?')[0]} HTTP ${response.status} (${code})`);
  }
  const text = await response.text();
  return text ? JSON.parse(text) : {};
}
function production(project) { return project.targets?.production?.id || project.targets?.production?.deploymentId; }
async function projects() {
  const values = {};
  for (const [key, site] of Object.entries({ web: WEB, main: MAIN })) {
    const value = await api(`/v9/projects/${site.project}?rollbackInfo=true`);
    if (value.id !== site.project || value.accountId !== TEAM || value.name !== site.name) throw Error('Project identity mismatch');
    values[key] = value;
  }
  return values;
}
function baselineGuard(values, webId = WEB.baseline) {
  if (production(values.web) !== webId) throw Error('Game website production changed; inspect before continuing');
  if (production(values.main) !== MAIN.baseline) throw Error('Marketing production changed; inspect before continuing');
}
async function environment() {
  const entries = [];
  let until;
  for (let page = 0; page < 10; page++) {
    const result = await api(`/v10/projects/${WEB.project}/env?decrypt=false${until ? `&until=${encodeURIComponent(until)}` : ''}`);
    if (!Array.isArray(result.envs)) throw Error('Unexpected project environment metadata');
    entries.push(...result.envs);
    if (!result.pagination?.next) return entries;
    until = result.pagination.next;
  }
  throw Error('Project environment pagination did not finish');
}
function originalEnvironment(entries) {
  // Hash retained metadata in memory; never print secret values, ciphertext,
  // API payloads or variable contents. Existing env IDs/targets stay intact.
  const retained = entries.filter(entry => !WINDOWS_KEYS.has(entry.key)).map(entry => ({
    id: entry.id, key: entry.key, type: entry.type, value: entry.value,
    target: [...(entry.target || [])].sort(), gitBranch: entry.gitBranch ?? null,
    customEnvironmentIds: [...(entry.customEnvironmentIds || [])].sort(),
  })).sort((a, b) => String(a.id).localeCompare(String(b.id)));
  return createHash('sha256').update(JSON.stringify(retained)).digest('hex');
}
function productionEntry(entries, key, required = true) {
  const found = entries.filter(entry => entry.key === key && entry.target?.includes('production'));
  if (found.length > 1 || required && found.length !== 1) throw Error('Windows production environment key is absent or duplicated');
  const entry = found[0];
  if (entry && (entry.target.length !== 1 || entry.gitBranch || entry.customEnvironmentIds?.length)) {
    throw Error('Windows configuration would change a shared or branch-specific variable');
  }
  return entry;
}
function environmentReady(entries) {
  for (const [key, expected] of Object.entries(WINDOWS_PUBLIC)) {
    const entry = productionEntry(entries, key);
    if (entry.type !== 'plain' || entry.value !== expected) throw Error('Windows public environment does not match the reviewed realm configuration');
  }
  for (const key of WINDOWS_SECRET_KEYS) {
    const entry = productionEntry(entries, key);
    if (entry.type !== 'sensitive') throw Error('Windows secrets must be unreadable production Secret variables');
  }
}
async function configureWindows() {
  const values = WINDOWS_SECRET_KEYS.map(key => process.env[key]);
  if (values.some(value => !value || value.length < 32 || /[\r\n\0]/.test(value)) ||
      values[0] === values[1] || values.some(value => value === process.env.VERCEL_TOKEN ||
        value === process.env.MIR2_PROMOTION_SESSION_SECRET || value === process.env.MIR2_PROMOTION_WALLET_AUTH_SECRET)) {
    throw Error('Two distinct Windows profile secrets must be supplied through CI environment');
  }
  const before = await environment();
  for (const key of WINDOWS_KEYS) productionEntry(before, key, false);
  const retained = originalEnvironment(before);
  const desired = [
    ...Object.entries(WINDOWS_PUBLIC).map(([key, value]) => ({ key, value, type: 'plain', target: ['production'] })),
    ...WINDOWS_SECRET_KEYS.map((key, index) => ({ key, value: values[index], type: 'sensitive', target: ['production'] })),
  ];
  baselineGuard(await projects());
  // This operation changes only the future Windows deployment profile. Never
  // change MIR2_PROMOTION_SESSION_SECRET, wallet legacy secrets, marketing env,
  // project root, branch or a current deployment's immutable environment.
  await api(`/v10/projects/${WEB.project}/env?upsert=true`, 'POST', desired);
  const after = await environment();
  if (originalEnvironment(after) !== retained) throw Error('An existing non-Windows environment variable changed; inspect before staging');
  environmentReady(after);
  baselineGuard(await projects());
  const receipt = { sourceSha: sha, operation: 'windows-config', project: WEB.project,
    configurationDigest, keys: [...WINDOWS_KEYS].sort(), secretValuesIncluded: false,
    originalEnvironmentSha256: retained, legacyCampaigns: LEGACY_CAMPAIGNS,
    marketingUnchanged: MAIN.baseline, gameWebsiteUnchanged: WEB.baseline };
  await writeFile('promotion-windows-config.json', JSON.stringify(receipt, null, 2) + '\n');
  console.log(JSON.stringify(receipt));
}
function summary(candidate, root) {
  return { id: candidate.id, url: candidate.url, readyState: candidate.readyState, target: candidate.target,
    project: candidate.projectId, root, sha: candidate.gitSource?.sha || candidate.meta?.windowsSourceSha,
    errorCode: candidate.errorCode };
}
async function deploymentSnapshot(id) {
  let until;
  for (let page = 0; page < 5; page++) {
    const list = await api(`/v7/deployments?projectId=${WEB.project}&sha=${sha}&target=production&limit=100${until ? `&until=${until}` : ''}`);
    const snapshot = list.deployments?.find(entry => entry.uid === id);
    if (snapshot) {
      if (snapshot.projectId !== WEB.project) throw Error('Candidate snapshot project mismatch');
      return snapshot;
    }
    until = list.pagination?.next;
    if (!until) break;
  }
  throw Error('Exact game website deployment snapshot not found');
}
async function candidate(requireReady = true) {
  const id = process.env.RELEASE_DEPLOYMENT_ID;
  if (!/^dpl_[a-zA-Z0-9]+$/.test(id || '')) throw Error('Exact game website candidate deployment ID required');
  const value = await api(`/v13/deployments/${id}?withGitRepoInfo=true`);
  const snapshot = await deploymentSnapshot(id);
  const root = snapshot.projectSettings?.rootDirectory ?? value.projectSettings?.rootDirectory;
  if (value.id !== id || value.projectId !== WEB.project || value.target !== 'production' ||
      (requireReady && value.readyState !== 'READY') || value.gitSource?.sha !== sha ||
      value.meta?.windowsSourceSha !== sha || value.meta?.windowsWebBaseline !== WEB.baseline ||
      value.meta?.windowsMainBaseline !== MAIN.baseline || value.meta?.windowsRoot !== WEB.root ||
      value.meta?.windowsCampaignId !== CAMPAIGN || value.meta?.windowsConfigurationDigest !== configurationDigest ||
      (root != null && root !== WEB.root)) throw Error('Candidate identity, source, readiness, root or baseline mismatch');
  return { ...value, verifiedRoot: root };
}
function protectedReader(deployment) {
  if (!/^[a-z0-9][a-z0-9-]*\.vercel\.app$/.test(deployment.url || '')) throw Error('Unexpected candidate hostname');
  const cache = new Map();
  return async path => {
    if (!cache.has(path)) {
      try {
        cache.set(path, execFileSync('npx', ['--yes', 'vercel@56.4.1', 'curl', `https://${deployment.url}${path}`, '--',
          '--fail', '--silent', '--show-error', '--location', '--max-redirs', '3', '--max-time', '30'], {
          encoding: 'utf8', timeout: 180000, maxBuffer: 8 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'],
        }));
      } catch { throw Error(`Authenticated candidate HTTP check failed for ${deployment.id} ${path}`); }
    }
    return cache.get(path);
  };
}
function publicReader() {
  const cache = new Map();
  return async path => {
    if (!cache.has(path)) {
      const response = await fetch(`https://mir2.obelisk.build${path}`, {
        cache: 'no-store', redirect: 'manual', signal: AbortSignal.timeout(30000),
      });
      if (path === '/version' || path === '/promotions?lang=zh-CN') {
        console.log(JSON.stringify({ publicResponse: path, status: response.status, cache: Object.fromEntries(
          ['age', 'cache-control', 'x-vercel-cache', 'cf-cache-status', 'x-vercel-id']
            .map(header => [header, response.headers.get(header)]),
        ) }));
      }
      if (!response.ok) throw Error(`Public HTTP check ${path} failed: ${response.status}`);
      cache.set(path, await response.text());
    }
    return cache.get(path);
  };
}
function assets(html, marketing = false) {
  const prefix = marketing ? '/marketing-assets/_next/static/' : '/_next/static/';
  return [...new Set([...html.matchAll(/(?:src|href)="([^"<>]+)"/g)]
    .map(match => match[1].replaceAll('&amp;', '&'))
    .filter(path => path.startsWith(prefix) && /\.(?:css|js)(?:\?|$)/.test(path)))];
}
async function staticResponse(read, path) {
  const content = await read(path);
  if (content.length < 20 || content.trimStart().startsWith('<')) throw Error(`Static asset ${path} returned unexpected content`);
  return content;
}
async function verifyRoutes(read, phase) {
  const version = JSON.parse(await read('/version'));
  if (version.revision !== sha) {
    const observed = /^[a-f0-9]{40}$/.test(version.revision || '') ? version.revision : 'unavailable';
    throw Error(`Website revision has not converged to the candidate (${observed})`);
  }
  const promotions = await read('/promotions?lang=zh-CN');
  if (!promotions.includes('Promotion program') || !promotions.includes('/_next/static/')) throw Error('Promotion route application mismatch');
  const promotionAssets = assets(promotions);
  if (!promotionAssets.some(path => /\.css(?:\?|$)/.test(path)) || !promotionAssets.some(path => /\.js(?:\?|$)/.test(path)) ||
      promotionAssets.length > 40) throw Error('Unexpected promotion asset inventory');
  const loadedPromotionAssets = await Promise.all(promotionAssets.map(async path => ({
    path, content: await staticResponse(read, path),
  })));
  const promotionJavaScript = loadedPromotionAssets.filter(asset => /\.js(?:\?|$)/.test(asset.path))
    .map(asset => asset.content).join('\n');
  const markers = ['Accrued commission (pending settlement)', 'Updates every 10 seconds', 'data-promotion-live-updates',
    'Create your character in the Windows game', 'promotion-migration-pending', 'promotion-migration-confirm',
    'promotion-migration-completed'];
  const missingMarkers = markers.filter(marker => !promotionJavaScript.includes(marker));
  if (missingMarkers.length) throw Error(`Promotion implementation is missing ${missingMarkers.length} expected markers (${phase}, ${promotionAssets.length} assets)`);
  const bootstrap = JSON.parse(await read('/api/promotions/bootstrap'));
  if (typeof bootstrap.authenticated !== 'boolean' || !Array.isArray(bootstrap.characters) ||
      bootstrap.gameClient !== 'windows' || bootstrap.campaign?.campaignId !== CAMPAIGN || bootstrap.campaign?.environment !== 'test' ||
      bootstrap.campaign?.commissionEnabled !== true || bootstrap.campaign?.payoutEnabled !== false ||
      bootstrap.campaign?.startsAtMs !== 1793219738449 || bootstrap.campaign?.endsAtMs !== 1793824538449) {
    throw Error('Windows campaign, original gameplay profile, inherited seven-day period or payout-off settings mismatch');
  }
  const explicitWindows = JSON.parse(await read(`/api/promotions/bootstrap?campaignId=${CAMPAIGN}`));
  if (explicitWindows.gameClient !== 'windows' || explicitWindows.campaign?.campaignId !== CAMPAIGN ||
      explicitWindows.campaign?.environment !== 'test' || explicitWindows.campaign?.payoutEnabled !== false) {
    throw Error('Explicit Windows campaign differs from the default profile');
  }
  for (const legacy of LEGACY_CAMPAIGNS) {
    const existing = JSON.parse(await read(`/api/promotions/bootstrap?campaignId=${legacy}`));
    if (existing.gameClient !== 'web' || existing.campaign?.campaignId !== legacy || existing.campaign?.environment !== 'test' ||
        typeof existing.authenticated !== 'boolean' || !Array.isArray(existing.characters)) {
      throw Error('A retained legacy campaign was routed into a different realm');
    }
    const originalSettings = {
      'numeron-20261101-testnet': ['draft', false, false, false, 1793462400000, 1801411200000],
      'numeron-chain-acceptance-20261029-fullchain': ['locked', false, true, true, 1793258649320, 1793259549320],
      'numeron-playtest-20261029': ['active', true, true, false, 1793219738449, 1793824538449],
    }[legacy];
    const observed = ['status', 'recruitmentEnabled', 'commissionEnabled', 'payoutEnabled', 'startsAtMs', 'endsAtMs']
      .map(key => existing.campaign[key]);
    if (JSON.stringify(observed) !== JSON.stringify(originalSettings)) throw Error('A legacy campaign period or financial setting changed');
  }
  const game = await read('/game');
  if (!game.includes('/_next/static/') || game.includes('navigation_promotions')) throw Error('Game application routing mismatch');
  const manifest = JSON.parse(await read('/manifest.webmanifest'));
  if (manifest.start_url !== '/game' || manifest.scope !== '/' || version.revision !== sha) throw Error('Game manifest or source revision mismatch');

  const main = await read('/');
  if (!main.includes('/marketing-assets/_next/static/') || !main.includes('navigation_promotions') ||
      !main.includes('footer_promotions') || !main.includes('promotions?lang=zh-CN')) throw Error('Existing marketing navigation or routing changed');
  const mainAssets = assets(main, true);
  await Promise.all(['css', 'js'].map(async extension => {
    const path = mainAssets.find(value => new URL(value, 'https://asset.invalid').pathname.endsWith(`.${extension}`));
    if (!path) throw Error(`Existing marketing ${extension} asset missing`);
    await staticResponse(read, path);
  }));
  const english = await read('/en');
  if (!english.includes('lang="en"') || !english.includes('promotions?lang=en')) throw Error('Existing English marketing routing changed');
  console.log(JSON.stringify({ httpChecks: 'passed', phase, sourceSha: sha, campaignId: CAMPAIGN,
    commissionEnabled: true, payoutEnabled: false, realtimeBundleChecks: 'passed', promotionAssetCount: promotionAssets.length,
    marketingBaseline: MAIN.baseline }));
}
async function waitWebProduction(id, requirePromotionJob = true) {
  // The production pointer can update before the asynchronous alias job finishes.
  const deadline = Date.now() + 180000;
  let previous;
  while (Date.now() < deadline) {
    const values = await projects();
    if (production(values.main) !== MAIN.baseline) throw Error('Marketing production changed during game website release');
    const job = values.web.lastAliasRequest;
    if (requirePromotionJob && job?.toDeploymentId === id && job.jobStatus === 'failed') throw Error('Game website alias assignment failed');
    const aliases = await Promise.all(WEB_ALIASES.map(async alias => {
      const value = await api(`/v4/aliases/${alias}?projectId=${WEB.project}`);
      if (value.projectId !== WEB.project) throw Error('Game website alias project mismatch');
      return { alias, deployment: value.deploymentId || value.deployment?.id };
    }));
    const state = JSON.stringify({ production: production(values.web), aliases,
      aliasJob: job ? { toDeploymentId: job.toDeploymentId, status: job.jobStatus } : null });
    if (state !== previous) { console.log(state); previous = state; }
    const jobReady = !requirePromotionJob || job?.toDeploymentId === id && job.jobStatus === 'succeeded';
    if (production(values.web) === id && aliases.every(alias => alias.deployment === id) && jobReady) return;
    await new Promise(resolve => setTimeout(resolve, 2000));
  }
  throw Error('Game website production, assignment job and origin aliases did not converge');
}

const initial = await projects();
if (operation === 'windows-context') {
  for (const [key, value] of Object.entries(initial)) console.log(JSON.stringify({ site: key, id: value.id,
    name: value.name, production: production(value), root: value.rootDirectory, framework: value.framework,
    aliasJob: value.lastAliasRequest ? { toDeploymentId: value.lastAliasRequest.toDeploymentId, status: value.lastAliasRequest.jobStatus } : null,
    git: value.link ? { type: value.link.type, org: value.link.org, repo: value.link.repo, repoId: value.link.repoId } : null }));
  if (process.env.RELEASE_DEPLOYMENT_ID) {
    const reviewed = await candidate(false);
    console.log(JSON.stringify({ candidate: summary(reviewed, reviewed.verifiedRoot) }));
  }
  const entries = await environment();
  console.log(JSON.stringify({ windowsConfiguration: [...WINDOWS_KEYS].map(key => ({ key,
    productionPresent: entries.some(entry => entry.key === key && entry.target?.includes('production')) })),
    configurationDigest, legacyCampaigns: LEGACY_CAMPAIGNS }));
} else if (operation === 'windows-config') {
  baselineGuard(initial);
  await configureWindows();
} else if (operation === 'windows-stage') {
  baselineGuard(initial);
  environmentReady(await environment());
  const link = initial.web.link;
  if (link?.type !== 'github' || link.org !== 'Zombieliu' || link.repo !== 'mir2' || !link.repoId) throw Error('Original GitHub source repository mismatch');
  const staged = await api('/v13/deployments', 'POST', {
    name: WEB.name, project: WEB.project, target: 'production', autoAssignCustomDomains: false,
    gitSource: { type: 'github', repoId: link.repoId, ref: BRANCH, sha },
    projectSettings: { rootDirectory: WEB.root, framework: 'nextjs',
      buildCommand: 'npm run build', installCommand: 'npm ci', outputDirectory: null },
    meta: { windowsSourceSha: sha, windowsWebBaseline: WEB.baseline, windowsMainBaseline: MAIN.baseline, windowsRoot: WEB.root,
      windowsCampaignId: CAMPAIGN, windowsConfigurationDigest: configurationDigest },
  });
  if (!/^dpl_[a-zA-Z0-9]+$/.test(staged.id || '') || (staged.projectId && staged.projectId !== WEB.project)) throw Error('Unexpected staged candidate identity');
  baselineGuard(await projects());
  const receipt = { sourceSha: sha, gameWebsite: summary(staged, WEB.root), webBaseline: WEB.baseline, marketingBaseline: MAIN.baseline };
  await writeFile('promotion-windows-release.json', JSON.stringify(receipt, null, 2) + '\n');
  console.log(JSON.stringify(receipt));
} else {
  baselineGuard(initial);
  environmentReady(await environment());
  const baseline = await api(`/v13/deployments/${WEB.baseline}`);
  if (baseline.id !== WEB.baseline || baseline.projectId !== WEB.project || baseline.target !== 'production' || baseline.readyState !== 'READY') {
    throw Error('Original game website rollback baseline is not ready');
  }
  const reviewed = await candidate();
  await verifyRoutes(protectedReader(reviewed), 'protected-candidate');
  baselineGuard(await projects());
  let promotionRequested = false;
  try {
    // Mark before the request: a timed-out API response may still have promoted it.
    promotionRequested = true;
    await api(`/v10/projects/${WEB.project}/promote/${reviewed.id}`, 'POST');
    await waitWebProduction(reviewed.id);
    const publicDeadline = Date.now() + 90000;
    for (let attempt = 0; ; attempt++) {
      try { await verifyRoutes(publicReader(), 'public-production'); break; }
      catch (error) {
        if (attempt === 29 || Date.now() >= publicDeadline) throw error;
        await new Promise(resolve => setTimeout(resolve, 3000));
      }
    }
    baselineGuard(await projects(), reviewed.id);
    console.log(JSON.stringify({ promoted: summary(reviewed, reviewed.verifiedRoot), marketingUnchanged: MAIN.baseline }));
  } catch (error) {
    if (promotionRequested) {
      const current = await projects();
      if (production(current.web) === reviewed.id) {
        await api(`/v1/projects/${WEB.project}/rollback/${WEB.baseline}`, 'POST');
        await waitWebProduction(WEB.baseline, false);
        console.log(JSON.stringify({ restored: 'game-website', deployment: WEB.baseline, marketingUnchanged: production((await projects()).main) === MAIN.baseline }));
      } else if (production(current.web) !== WEB.baseline) {
        throw Error('Game website changed concurrently; automatic rollback will not overwrite another deployment');
      }
    }
    throw error;
  }
}
