import { writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';

const TEAM = 'team_pXY7oaLtibSTlKSmKXMXzrQR';
const BRANCH = 'codex/promotion-vercel-20261029';
const SITES = {
  main: { project: 'prj_Q14gZuvjxkfqOCExhqOrYiR2UbxA', name: 'numeron-legend-of-rebirth',
    baseline: 'dpl_4uHMwZobvDW1tUwszUdbq14z8eMj', root: 'mir2-web3/apps/website' },
  game: { project: 'prj_Bl3ttB4PzcdCpJMYLRhzQjkdXyTz', name: 'mir2-web3-web',
    baseline: 'dpl_GuricEhcXbVub8u3u3HSt2VR5wGe', root: 'mir2-web3/apps/web' },
};
const operation = process.env.RELEASE_OPERATION;
const sha = process.env.RELEASE_SOURCE_SHA || process.env.GITHUB_SHA;
if (process.env.GITHUB_REF !== `refs/heads/${BRANCH}` || !/^[a-f0-9]{40}$/.test(process.env.GITHUB_SHA || '') || !/^[a-f0-9]{40}$/.test(sha || '')) throw Error('Unexpected immutable release source');
if (operation === 'unified-stage' && sha !== process.env.GITHUB_SHA) throw Error('Staging must use the checked-out workflow commit');
if (!process.env.VERCEL_TOKEN || process.env.VERCEL_ORG_ID !== TEAM || process.env.VERCEL_PROJECT_ID !== SITES.game.project) throw Error('Existing CI identity does not match the original website');

async function api(path, method = 'GET', body) {
  const response = await fetch(`https://api.vercel.com${path}${path.includes('?') ? '&' : '?'}teamId=${TEAM}`, {
    method, headers: { Authorization: `Bearer ${process.env.VERCEL_TOKEN}`, 'Content-Type': 'application/json' },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }), signal: AbortSignal.timeout(30000),
  });
  if (!response.ok) {
    const value = await response.json().catch(() => ({}));
    // The API error code is safe diagnostic metadata; never log request bodies or credentials.
    throw Error(`Vercel ${method} ${path.split('?')[0]} HTTP ${response.status} (${String(value.error?.code || 'unknown').replace(/[^a-zA-Z0-9_-]/g, '')})`);
  }
  const text = await response.text();
  return text ? JSON.parse(text) : {};
}
function production(project) { return project.targets?.production?.id || project.targets?.production?.deploymentId; }
async function projects() {
  const result = {};
  for (const [key, site] of Object.entries(SITES)) {
    const value = await api(`/v9/projects/${site.project}`);
    if (value.id !== site.project || value.accountId !== TEAM || value.name !== site.name) throw Error('Project identity mismatch');
    result[key] = value;
  }
  return result;
}
function baselineGuard(values) {
  for (const [key, site] of Object.entries(SITES)) if (production(values[key]) !== site.baseline) throw Error(`${key} production changed; inspect before release`);
}
function candidateGuard(key, candidate, snapshot) {
  const site = SITES[key];
  if (candidate.projectId !== site.project || candidate.target !== 'production' || candidate.readyState !== 'READY' ||
      candidate.meta?.unifiedSourceSha !== sha || candidate.meta?.unifiedMainBaseline !== SITES.main.baseline ||
      candidate.meta?.unifiedGameBaseline !== SITES.game.baseline ||
      (snapshot?.projectSettings?.rootDirectory != null && snapshot.projectSettings.rootDirectory !== site.root) ||
      candidate.gitSource?.sha !== sha) throw Error(`${key} candidate identity, source, root or readiness mismatch`);
}
function candidateResponse(candidate, path) {
  if (!/^[a-z0-9][a-z0-9-]*\.vercel\.app$/.test(candidate.url || '')) throw Error('Unexpected candidate hostname');
  try {
    return execFileSync('npx', ['--yes', 'vercel@56.4.1', 'curl', `https://${candidate.url}${path}`, '--',
      '--fail', '--silent', '--show-error', '--max-time', '30'], {
      encoding: 'utf8', timeout: 180000, maxBuffer: 8 * 1024 * 1024,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
  } catch { throw Error(`Authenticated candidate check failed for ${candidate.id} ${path}`); }
}
function verifyCandidatePages(reviewed) {
  const main = candidateResponse(reviewed.main, '/zh-CN');
  if (!main.includes('/marketing-assets/_next/static/') || !main.includes('navigation_promotions') ||
      !main.includes('footer_promotions') || !main.includes('promotions?lang=zh-CN')) throw Error('Marketing candidate navigation/assets mismatch');
  if (!candidateResponse(reviewed.main, '/zh-CN/download').includes('https://mir2.obelisk.build/game')) throw Error('Marketing candidate game link mismatch');
  const manifest = JSON.parse(candidateResponse(reviewed.game, '/manifest.webmanifest'));
  const version = JSON.parse(candidateResponse(reviewed.game, '/version'));
  if (manifest.start_url !== '/game' || manifest.scope !== '/' || version.revision !== sha) throw Error('Game candidate manifest or revision mismatch');
  const game = candidateResponse(reviewed.game, '/game');
  if (!game.includes('/_next/static/') || game.includes('navigation_promotions')) throw Error('Game candidate application mismatch');
  console.log(JSON.stringify({ candidatePages: 'passed', sourceSha: sha, main: reviewed.main.id, game: reviewed.game.id }));
}
async function deploymentSnapshot(key, id) {
  let until;
  for (let page = 0; page < 5; page++) {
    const list = await api(`/v7/deployments?projectId=${SITES[key].project}&sha=${sha}&target=production&limit=100${until ? `&until=${until}` : ''}`);
    const snapshot = list.deployments?.find(entry => entry.uid === id);
    if (snapshot) {
      if (snapshot.projectId !== SITES[key].project) throw Error('Deployment snapshot project mismatch');
      return snapshot;
    }
    until = list.pagination?.next;
    if (!until) break;
  }
  throw Error('Exact deployment snapshot not found');
}
async function candidates() {
  const result = {};
  for (const [key, id] of Object.entries({ main: process.env.RELEASE_MAIN_DEPLOYMENT_ID, game: process.env.RELEASE_DEPLOYMENT_ID })) {
    if (!/^dpl_[a-zA-Z0-9]+$/.test(id || '')) throw Error(`Exact ${key} candidate deployment ID required`);
    result[key] = await api(`/v13/deployments/${id}?withGitRepoInfo=true`);
    if (result[key].id !== id) throw Error('Candidate deployment ID mismatch');
    const snapshot = await deploymentSnapshot(key, id);
    candidateGuard(key, result[key], snapshot);
    result[key] = { ...result[key], verifiedRoot: snapshot.projectSettings?.rootDirectory };
  }
  return result;
}
async function waitProduction(key, id) {
  for (let attempt = 0; attempt < 30; attempt++) {
    const value = await api(`/v9/projects/${SITES[key].project}`);
    if (production(value) === id) return;
    await new Promise(resolve => setTimeout(resolve, 2000));
  }
  throw Error(`${key} production mapping did not converge`);
}
function summary(key, value) {
  return { site: key, id: value.id, url: value.url, readyState: value.readyState, target: value.target,
    root: value.verifiedRoot, sha: value.gitSource?.sha || value.meta?.githubCommitSha || value.meta?.unifiedSourceSha };
}

async function publicSmoke() {
  async function get(path, text) {
    const response = await fetch(`https://mir2.obelisk.build${path}`, { signal: AbortSignal.timeout(30000), cache: 'no-store' });
    if (!response.ok) throw Error(`Public route ${path} HTTP ${response.status}`);
    const body = await response.text();
    if (text && !body.includes(text)) throw Error(`Public route ${path} has unexpected content`);
    return body;
  }
  const home = await get('/', '/marketing-assets/_next/static/');
  if (!home.includes('navigation_promotions') || !home.includes('footer_promotions') || !home.includes('promotions?lang=zh-CN')) throw Error('Marketing navigation or footer is missing');
  const asset = home.match(/(?:src|href)="(\/marketing-assets\/_next\/static\/[^"?]+(?:\?[^" ]*)?)"/)?.[1];
  if (!asset) throw Error('Marketing static asset not found');
  await get(asset.replaceAll('&amp;', '&'));
  await get('/zh-CN/download', 'https://mir2.obelisk.build/game');
  await get('/en', 'promotions?lang=en');
  await get('/game');
  const version = JSON.parse(await get('/version'));
  if (version.revision !== sha) throw Error('Public game revision mismatch');
  await get('/promotions?lang=zh-CN');
  const manifest = JSON.parse(await get('/manifest.webmanifest'));
  if (manifest.start_url !== '/game' || manifest.scope !== '/') throw Error('Game PWA route mismatch');
  const bootstrap = JSON.parse(await get('/api/promotions/bootstrap'));
  if (typeof bootstrap.authenticated !== 'boolean' || bootstrap.campaign?.campaignId !== 'numeron-playtest-20261029' || bootstrap.campaign?.environment !== 'test') throw Error('Existing promotion backend configuration mismatch');
  console.log(JSON.stringify({ publicSmoke: 'passed', marketingAsset: asset, campaignId: bootstrap.campaign.campaignId }));
}

const initial = await projects();
if (operation === 'unified-context') {
  for (const [key, value] of Object.entries(initial)) console.log(JSON.stringify({ site: key, id: value.id,
    production: production(value), root: value.rootDirectory, framework: value.framework,
    build: value.buildCommand, install: value.installCommand, nodeVersion: value.nodeVersion,
    git: value.link ? { repo: value.link.repo, repoId: value.link.repoId } : null,
    environmentKeys: (value.env || []).map(entry => entry.key).sort() }));
  for (const [key, id] of Object.entries({ main: process.env.RELEASE_MAIN_DEPLOYMENT_ID, game: process.env.RELEASE_DEPLOYMENT_ID })) {
    if (!id) continue;
    if (!/^dpl_[a-zA-Z0-9]+$/.test(id)) throw Error('Invalid candidate ID');
    const candidate = await api(`/v13/deployments/${id}?withGitRepoInfo=true`);
    if (candidate.projectId !== SITES[key].project) throw Error('Candidate project mismatch');
    const snapshot = await deploymentSnapshot(key, id);
    console.log(JSON.stringify({ candidate: summary(key, { ...candidate, verifiedRoot: snapshot.projectSettings?.rootDirectory }), errorCode: candidate.errorCode }));
  }
} else if (operation === 'unified-stage') {
  baselineGuard(initial);
  if (initial.game.link?.type !== 'github' || initial.game.link?.repo !== 'mir2' || !initial.game.link?.repoId) throw Error('Original GitHub source repository mismatch');
  const staged = {};
  for (const [key, site] of Object.entries(SITES)) {
    const mainEnvironment = { NEXT_PUBLIC_SITE_URL: 'https://mir2.obelisk.build',
      NEXT_PUBLIC_GAME_URL: 'https://mir2.obelisk.build/game',
      GAME_HEALTH_URL: 'https://165.154.65.136.sslip.io/health',
      EXPLORER_ORIGIN: 'https://numeron-explorer.vercel.app' };
    staged[key] = await api('/v13/deployments', 'POST', {
      name: site.name, project: site.project, target: 'production', autoAssignCustomDomains: false,
      gitSource: { type: 'github', repoId: initial.game.link.repoId, ref: BRANCH, sha },
      ...(key === 'main' ? { projectSettings: { rootDirectory: site.root, framework: 'nextjs',
        buildCommand: 'npm run build', installCommand: 'npm ci', outputDirectory: null },
        env: mainEnvironment, build: { env: mainEnvironment } } : {}),
      meta: { unifiedSourceSha: sha, unifiedMainBaseline: SITES.main.baseline, unifiedGameBaseline: SITES.game.baseline },
    });
    console.log(JSON.stringify(summary(key, staged[key])));
    baselineGuard(await projects());
  }
  await writeFile('unified-release.json', JSON.stringify({ sourceSha: sha, sites: Object.fromEntries(Object.entries(staged).map(([key, value]) => [key, summary(key, value)])) }, null, 2) + '\n');
} else if (operation === 'unified-promote') {
  baselineGuard(initial);
  const reviewed = await candidates();
  // New Git deployments omit rootDirectory and downloadable source files.
  // Verify the actual protected application responses before either alias changes.
  verifyCandidatePages(reviewed);
  const touched = [];
  try {
    // Main first: the game project will proxy only to the already upgraded public marketing alias.
    for (const key of ['main', 'game']) {
      const fresh = await projects();
      for (const [other, site] of Object.entries(SITES)) {
        const expected = touched.includes(other) ? reviewed[other].id : site.baseline;
        if (production(fresh[other]) !== expected) throw Error('Concurrent production change detected');
      }
      touched.push(key); // Include requests whose result is uncertain after a network timeout.
      await api(`/v10/projects/${SITES[key].project}/promote/${reviewed[key].id}`, 'POST');
      await waitProduction(key, reviewed[key].id);
    }
    for (let attempt = 0; ; attempt++) {
      try { await publicSmoke(); break; }
      catch (error) {
        if (attempt === 3) throw error;
        await new Promise(resolve => setTimeout(resolve, 3000));
      }
    }
    for (const [key, value] of Object.entries(reviewed)) console.log(JSON.stringify({ promoted: summary(key, value) }));
  } catch (error) {
    for (const key of touched.reverse()) {
      const current = await api(`/v9/projects/${SITES[key].project}`);
      if (production(current) === reviewed[key].id) {
        await api(`/v1/projects/${SITES[key].project}/rollback/${SITES[key].baseline}`, 'POST');
        await waitProduction(key, SITES[key].baseline);
        console.log(JSON.stringify({ restored: key, deployment: SITES[key].baseline }));
      }
    }
    throw error;
  }
} else throw Error('Unsupported unified release operation');
