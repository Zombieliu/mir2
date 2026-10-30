import { writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';

const TEAM = 'team_pXY7oaLtibSTlKSmKXMXzrQR';
const BRANCH = 'codex/promotion-vercel-20261029';
const WEB = { project: 'prj_Bl3ttB4PzcdCpJMYLRhzQjkdXyTz', name: 'mir2-web3-web',
  baseline: 'dpl_CYQVRcyaxmFERsYoeK9zqQ41vhbr', root: 'mir2-web3/apps/web' };
const MAIN = { project: 'prj_Q14gZuvjxkfqOCExhqOrYiR2UbxA', name: 'numeron-legend-of-rebirth',
  baseline: 'dpl_7krVTQSx5Wj9Ht8Sd5KF5o9d3F97' };
const CAMPAIGN = 'numeron-playtest-20261029';
const operation = process.env.RELEASE_OPERATION;
const sha = process.env.RELEASE_SOURCE_SHA || process.env.GITHUB_SHA;
if (!['realtime-context', 'realtime-stage', 'realtime-promote'].includes(operation)) throw Error('Unsupported realtime release operation');
if (process.env.GITHUB_REF !== `refs/heads/${BRANCH}` || !/^[a-f0-9]{40}$/.test(process.env.GITHUB_SHA || '') ||
    !/^[a-f0-9]{40}$/.test(sha || '')) throw Error('Unexpected immutable realtime release source');
if (operation === 'realtime-stage' && sha !== process.env.GITHUB_SHA) throw Error('Stage must use the checked-out workflow commit');
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
    const value = await api(`/v9/projects/${site.project}`);
    if (value.id !== site.project || value.accountId !== TEAM || value.name !== site.name) throw Error('Project identity mismatch');
    values[key] = value;
  }
  return values;
}
function baselineGuard(values, webId = WEB.baseline) {
  if (production(values.web) !== webId) throw Error('Game website production changed; inspect before continuing');
  if (production(values.main) !== MAIN.baseline) throw Error('Marketing production changed; inspect before continuing');
}
function summary(candidate, root) {
  return { id: candidate.id, url: candidate.url, readyState: candidate.readyState, target: candidate.target,
    project: candidate.projectId, root, sha: candidate.gitSource?.sha || candidate.meta?.realtimeSourceSha,
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
      value.meta?.realtimeSourceSha !== sha || value.meta?.realtimeWebBaseline !== WEB.baseline ||
      value.meta?.realtimeMainBaseline !== MAIN.baseline || value.meta?.realtimeRoot !== WEB.root ||
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
  for (const marker of ['Accrued commission (pending settlement)', 'Updates every 10 seconds', 'data-promotion-live-updates']) {
    if (!promotionJavaScript.includes(marker)) throw Error('Candidate realtime commission copy or live-update implementation is missing');
  }
  const bootstrap = JSON.parse(await read('/api/promotions/bootstrap'));
  if (typeof bootstrap.authenticated !== 'boolean' || !Array.isArray(bootstrap.characters) ||
      bootstrap.campaign?.campaignId !== CAMPAIGN || bootstrap.campaign?.environment !== 'test' ||
      bootstrap.campaign?.commissionEnabled !== true || bootstrap.campaign?.payoutEnabled !== false) {
    throw Error('Existing test campaign or commission/payout settings changed');
  }
  const game = await read('/game');
  if (!game.includes('/_next/static/') || game.includes('navigation_promotions')) throw Error('Game application routing mismatch');
  const manifest = JSON.parse(await read('/manifest.webmanifest'));
  const version = JSON.parse(await read('/version'));
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
async function waitWebProduction(id) {
  for (let attempt = 0; attempt < 30; attempt++) {
    const values = await projects();
    if (production(values.main) !== MAIN.baseline) throw Error('Marketing production changed during game website release');
    if (production(values.web) === id) return;
    await new Promise(resolve => setTimeout(resolve, 2000));
  }
  throw Error('Game website production mapping did not converge');
}

const initial = await projects();
if (operation === 'realtime-context') {
  for (const [key, value] of Object.entries(initial)) console.log(JSON.stringify({ site: key, id: value.id,
    name: value.name, production: production(value), root: value.rootDirectory, framework: value.framework,
    git: value.link ? { type: value.link.type, org: value.link.org, repo: value.link.repo, repoId: value.link.repoId } : null }));
  if (process.env.RELEASE_DEPLOYMENT_ID) {
    const reviewed = await candidate(false);
    console.log(JSON.stringify({ candidate: summary(reviewed, reviewed.verifiedRoot) }));
  }
} else if (operation === 'realtime-stage') {
  baselineGuard(initial);
  const link = initial.web.link;
  if (link?.type !== 'github' || link.org !== 'Zombieliu' || link.repo !== 'mir2' || !link.repoId) throw Error('Original GitHub source repository mismatch');
  const staged = await api('/v13/deployments', 'POST', {
    name: WEB.name, project: WEB.project, target: 'production', autoAssignCustomDomains: false,
    gitSource: { type: 'github', repoId: link.repoId, ref: BRANCH, sha },
    projectSettings: { rootDirectory: WEB.root, framework: 'nextjs',
      buildCommand: 'npm run build', installCommand: 'npm ci', outputDirectory: null },
    meta: { realtimeSourceSha: sha, realtimeWebBaseline: WEB.baseline, realtimeMainBaseline: MAIN.baseline, realtimeRoot: WEB.root },
  });
  if (!/^dpl_[a-zA-Z0-9]+$/.test(staged.id || '') || (staged.projectId && staged.projectId !== WEB.project)) throw Error('Unexpected staged candidate identity');
  baselineGuard(await projects());
  const receipt = { sourceSha: sha, gameWebsite: summary(staged, WEB.root), webBaseline: WEB.baseline, marketingBaseline: MAIN.baseline };
  await writeFile('promotion-realtime-release.json', JSON.stringify(receipt, null, 2) + '\n');
  console.log(JSON.stringify(receipt));
} else {
  baselineGuard(initial);
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
    for (let attempt = 0; ; attempt++) {
      try { await verifyRoutes(publicReader(), 'public-production'); break; }
      catch (error) {
        if (attempt === 3) throw error;
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
        await waitWebProduction(WEB.baseline);
        console.log(JSON.stringify({ restored: 'game-website', deployment: WEB.baseline, marketingUnchanged: production((await projects()).main) === MAIN.baseline }));
      } else if (production(current.web) !== WEB.baseline) {
        throw Error('Game website changed concurrently; automatic rollback will not overwrite another deployment');
      }
    }
    throw error;
  }
}
