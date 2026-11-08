import { readFile, writeFile, mkdtemp, lstat, readdir } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { FIXED_R16 } from '../../mir2-web3/apps/web/scripts/lib/fixed-r16-assets.mjs';

export const TEAM = 'team_pXY7oaLtibSTlKSmKXMXzrQR';
export const BRANCH = 'codex/windows-player-journey';
export const REPOSITORY = 'Zombieliu/mir2';
export const SITES = Object.freeze({
  original: { project: 'prj_Bl3ttB4PzcdCpJMYLRhzQjkdXyTz', name: 'mir2-web3-web', baseline: 'dpl_3NBhFUPKxNF4nccRnG94ZhDq1iz6' },
  main: { project: 'prj_Q14gZuvjxkfqOCExhqOrYiR2UbxA', name: 'numeron-legend-of-rebirth', baseline: 'dpl_22qaP2Cn1my46GJk6xrS9tk8m4RK', root: 'mir2-web3/apps/website' },
  game: { project: 'prj_QzS3a2wirTJCixU4tYRVY5nmvSjC', name: 'numeron-r16-web', baseline: null, root: 'mir2-web3/apps/web' },
});
export const ORIGIN = 'https://numeron.obelisk.build';
const DEPLOYMENT = /^dpl_[A-Za-z0-9]+$/;
const SHA = /^[a-f0-9]{40}$/;
const ROOT = fileURLToPath(new URL('../../', import.meta.url));
const requireRelease = (condition, code) => { if (!condition) throw new ReleaseError(code); };
export class ReleaseError extends Error { constructor(code) { super(code); this.code = code; } }
export const production = project => project.targets?.production?.id ?? project.targets?.production?.deploymentId ?? null;
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

export function validateIdentity(env) {
  const operation = env.RELEASE_OPERATION, sha = env.RELEASE_SOURCE_SHA || env.GITHUB_SHA;
  requireRelease(['numeron-context', 'numeron-stage', 'numeron-promote'].includes(operation), 'operationInvalid');
  requireRelease(env.GITHUB_REPOSITORY === REPOSITORY && env.GITHUB_REF === `refs/heads/${BRANCH}`
    && SHA.test(env.GITHUB_SHA || '') && SHA.test(sha || ''), 'sourceIdentityInvalid');
  requireRelease(operation !== 'numeron-stage' || sha === env.GITHUB_SHA, 'stageSourceMustEqualCheckout');
  requireRelease(Boolean(env.VERCEL_TOKEN) && env.VERCEL_ORG_ID === TEAM
    && env.VERCEL_PROJECT_ID === SITES.original.project, 'originalCiIdentityInvalid');
  return { operation, sha };
}

export function validateProject(key, value) {
  const site = SITES[key];
  requireRelease(value?.id === site.project && value.accountId === TEAM && value.name === site.name, 'projectIdentityChanged');
}
export function guardProjects(projects, expected) {
  for (const key of Object.keys(SITES)) {
    validateProject(key, projects[key]);
    requireRelease(production(projects[key]) === expected[key], 'concurrentProductionChange');
  }
}
export function validateAssetConfig(config, contract = FIXED_R16) {
  requireRelease(config?.schemaVersion === 3 && config.version === contract.version
    && config.objectPrefix === contract.prefix && config.assetBaseUrl === `${contract.publicOrigin}/${contract.prefix}`
    && config.remoteReleaseUrl === `${contract.publicOrigin}/${contract.prefix}/remote-asset-release.json`, 'assetConfigNotFixedR16');
}
export function validateAssetReceipt(receipt, sha, contract = FIXED_R16) {
  requireRelease(receipt?.schemaVersion === 1 && receipt.kind === 'mir2-fixed-r16-publication-receipt'
    && receipt.repository === REPOSITORY && receipt.sourceSha === sha && receipt.sourceClean === true
    && receipt.materialSourceSha === contract.materialSourceSha && receipt.passed === true && receipt.dryRun === false
    && typeof receipt.manifestPublished === 'boolean' && receipt.manifestVerified === true && receipt.phase === 'complete'
    && receipt.verified === contract.fileCount && receipt.unknownWriteOutcomes === 0
    && typeof receipt.finishedAt === 'string' && Number.isFinite(Date.parse(receipt.finishedAt)), 'assetReceiptNotSuccessfulPublication');
  for (const field of ['uploaded', 'existingVerified', 'remoteWrites', 'putAttempts', 'publicGzipWireVerified', 'publicGzipDecodedVerified'])
    requireRelease(Number.isSafeInteger(receipt[field]) && receipt[field] >= 0, 'assetReceiptReadbackCountInvalid');
  requireRelease(receipt.uploaded + receipt.existingVerified === contract.fileCount
    && receipt.remoteWrites === receipt.uploaded + Number(receipt.manifestPublished)
    && receipt.putAttempts === receipt.remoteWrites
    && receipt.publicGzipWireVerified + receipt.publicGzipDecodedVerified === contract.gzipFiles, 'assetReceiptReadbackIncomplete');
  for (const field of ['bundleSha256', 'payloadSha256', 'manifestSha256'])
    requireRelease(receipt[field] === contract[field], 'assetReceiptPinMismatch');
  const review = receipt.review;
  for (const [field, expected] of Object.entries({ prefix: contract.prefix, files: contract.fileCount,
    bytes: contract.payloadBytes, wireBytes: contract.wireBytes, gzipFiles: contract.gzipFiles, fonts: contract.fontFiles,
    payloadSha256: contract.payloadSha256, manifestSha256: contract.manifestSha256, manifestLast: true,
    overwriteExisting: false, allMapsVerified: false, inheritedObjectsReverified: false }))
    requireRelease(review?.[field] === expected, 'assetReceiptReviewMismatch');
}
export function validateAssetRun(run, jobs, artifacts, runId, sha) {
  requireRelease(String(run?.id) === runId && run.repository?.full_name === REPOSITORY
    && run.path === '.github/workflows/web-assets-r2-release.yml' && run.head_sha === sha
    && run.status === 'completed' && run.conclusion === 'success' && run.event === 'workflow_dispatch', 'assetRunIdentityMismatch');
  const matches = jobs.filter(job => job.name === 'publish-fixed-r16-assets');
  requireRelease(matches.length === 1 && matches[0].status === 'completed' && matches[0].conclusion === 'success', 'assetPublicationJobNotSuccessful');
  const named = artifacts.filter(artifact => artifact.name === `fixed-r16-assets-${runId}`);
  requireRelease(named.length === 1 && named[0].expired === false && Number.isSafeInteger(named[0].id), 'assetReceiptArtifactInvalid');
  return named[0];
}
export function validatePublicManifest(bytes, contract = FIXED_R16) {
  requireRelease(bytes.length === contract.manifestBytes && digest(bytes) === contract.manifestSha256, 'publicAssetManifestPinMismatch');
  let manifest;
  try { manifest = JSON.parse(bytes.toString('utf8')); } catch { throw new ReleaseError('publicAssetManifestInvalid'); }
  requireRelease(manifest.schemaVersion === 1 && manifest.kind === 'mir2-remote-asset-release'
    && manifest.version === contract.version && manifest.objectPrefix === contract.prefix
    && manifest.candidateCoverage?.payloadSha256 === contract.payloadSha256
    && manifest.candidateCoverage?.localCandidateFilesVerified === contract.fileCount
    && manifest.bevyRuntime?.version === contract.runtimeVersion, 'publicAssetManifestIdentityMismatch');
}
export function publicEnvironment(key, sha, config) {
  return key === 'main' ? {
    NEXT_PUBLIC_SITE_URL: ORIGIN, NEXT_PUBLIC_GAME_URL: `${ORIGIN}/game`,
    GAME_HEALTH_URL: 'https://165.154.65.136.sslip.io/playtest/health', EXPLORER_ORIGIN: 'https://numeron-explorer.vercel.app',
  } : {
    NEXT_PUBLIC_MIR2_GATEWAY_WS_URL: 'wss://165.154.65.136.sslip.io/playtest/ws',
    MIR2_GATEWAY_HTTP_URL: 'https://165.154.65.136.sslip.io/playtest', NEXT_PUBLIC_MIR2_QUEST_GUIDANCE: 'newcomer-v2',
    MIR2_RUNTIME_ENV: 'test', MIR2_MARKETING_ORIGIN: 'https://numeron-legend-of-rebirth.vercel.app',
    MIR2_PASSKEY_ALLOWED_ORIGINS: [ORIGIN, 'https://numeron-r16-web.vercel.app', 'https://numeron-r16-web-obelisk-labs.vercel.app'].join(','),
    MIR2_USE_PREBUILT_BEVY_RUNTIME: '1', MIR2_USE_PREBUILT_CLIENT_CORE: '1', MIR2_REUSE_ORIGINAL_ASSET_MANIFEST: '1',
    MIR2_PRODUCTION_ASSET_CONFIG: 'numeron-r16-web-assets.json',
    MIR2_DEPLOY_REVISION: sha, NEXT_PUBLIC_MIR2_ASSET_BASE_URL: config.assetBaseUrl, MIR2_ASSET_OBJECT_PREFIX: config.objectPrefix,
  };
}
export function deploymentBody(key, sha, repoId, config, secret) {
  const environment = publicEnvironment(key, sha, config);
  if (key === 'game') requireRelease(typeof secret === 'string' && secret.length >= 32, 'gatewayAuthSecretMissing');
  return { name: SITES[key].name, project: SITES[key].project, target: 'production', autoAssignCustomDomains: false,
    gitSource: { type: 'github', repoId, ref: BRANCH, sha },
    projectSettings: { rootDirectory: SITES[key].root, framework: 'nextjs', nodeVersion: '22.x',
      buildCommand: key === 'game' ? 'bash ./scripts/vercel-build.sh' : 'npm run build',
      installCommand: 'npm ci', outputDirectory: null, sourceFilesOutsideRootDirectory: true },
    env: { ...environment, ...(key === 'game' ? { MIR2_PASSKEY_AUTH_SECRET: secret } : {}) }, build: { env: environment },
    meta: { numeronR16SourceSha: sha, numeronR16Root: SITES[key].root, numeronR16AssetVersion: config.version,
      numeronR16OriginalBaseline: SITES.original.baseline, numeronR16MainBaseline: SITES.main.baseline },
  };
}
export function candidateGuard(key, candidate, snapshot, sha, config) {
  const site = SITES[key];
  // Modern Git deployments may omit root from their REST snapshots. Every
  // explicit root must agree; missing roots still require our staged metaRoot,
  // exact Git source and the protected page proofs before any promotion.
  const roots = [candidate?.rootDirectory, candidate?.projectSettings?.rootDirectory,
    snapshot?.rootDirectory, snapshot?.projectSettings?.rootDirectory].filter(root => root != null);
  requireRelease(candidate?.projectId === site.project && candidate.target === 'production' && candidate.readyState === 'READY'
    && candidate.gitSource?.sha === sha && candidate.gitSource?.ref === BRANCH
    && candidate.meta?.numeronR16SourceSha === sha && candidate.meta?.numeronR16Root === site.root
    && candidate.meta?.numeronR16AssetVersion === config.version
    && candidate.meta?.numeronR16OriginalBaseline === SITES.original.baseline
    && candidate.meta?.numeronR16MainBaseline === SITES.main.baseline
    && snapshot?.projectId === site.project && roots.every(root => root === site.root), 'candidateIdentitySourceRootMismatch');
  requireRelease(/^[a-z0-9][a-z0-9-]*\.vercel\.app$/.test(candidate.url || ''), 'candidateHostnameInvalid');
}
export function windowsDownloadLinks(html) {
  const links = [...html.matchAll(/<a\b[^>]*>/g)].filter(match => /data-track=["']download_windows["']/.test(match[0]))
    .map(match => /\bhref=["']([^"']+)["']/.exec(match[0])?.[1]?.replaceAll('&amp;', '&'));
  requireRelease(links.length > 0 && links.every(link => {
    try { const url = new URL(link); return url.protocol === 'https:' && !url.username && !url.password; } catch { return false; }
  }), 'windowsDownloadLinkMissing');
  return [...new Set(links)].sort();
}
export function verifyPages(read, candidates, baselineMain, sha) {
  const marketing = read(candidates.main, '/zh-CN');
  requireRelease(marketing.includes('/marketing-assets/_next/static/') && marketing.includes('navigation_promotions')
    && marketing.includes('footer_promotions'), 'marketingHtmlInvalid');
  const download = read(candidates.main, '/zh-CN/download');
  requireRelease(download.includes(`${ORIGIN}/game`), 'marketingGameLinkInvalid');
  requireRelease(JSON.stringify(windowsDownloadLinks(download)) === JSON.stringify(windowsDownloadLinks(read(baselineMain, '/zh-CN/download'))), 'windowsDownloadsChanged');
  requireRelease(read(candidates.main, '/en').includes('lang="en"'), 'marketingEnglishRouteInvalid');
  let version, manifest;
  try { version = JSON.parse(read(candidates.game, '/version')); manifest = JSON.parse(read(candidates.game, '/manifest.webmanifest')); }
  catch { throw new ReleaseError('candidateVersionOrManifestInvalid'); }
  requireRelease(version.revision === sha && manifest.start_url === '/game' && manifest.scope === '/', 'candidateVersionOrPwaMismatch');
  const game = read(candidates.game, '/game');
  requireRelease(game.includes('/_next/static/') && !game.includes('navigation_promotions'), 'gameHtmlInvalid');
}

async function boundedBytes(response, maximum) {
  requireRelease(response.ok, 'publicManifestUnavailable');
  const chunks = []; let size = 0;
  for await (const chunk of response.body) { size += chunk.length; requireRelease(size <= maximum, 'responseTooLarge'); chunks.push(chunk); }
  return Buffer.concat(chunks);
}
async function downloadReceipt(runId, name, env) {
  const directory = await mkdtemp(path.join(env.RUNNER_TEMP || tmpdir(), 'numeron-r16-asset-receipt-'));
  try { execFileSync('gh', ['run', 'download', runId, '--repo', REPOSITORY, '--name', name, '--dir', directory], {
    env: childEnvironment(env, 'github'), encoding: 'utf8', timeout: 120000, maxBuffer: 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'],
  }); } catch { throw new ReleaseError('assetReceiptDownloadFailed'); }
  requireRelease(JSON.stringify(await readdir(directory)) === JSON.stringify(['receipt.json']), 'assetReceiptMembersInvalid');
  const filename = path.join(directory, 'receipt.json'), stat = await lstat(filename);
  requireRelease(stat.isFile() && !stat.isSymbolicLink() && stat.nlink === 1 && stat.size <= 1024 * 1024, 'assetReceiptFileInvalid');
  try { return JSON.parse(await readFile(filename, 'utf8')); } catch { throw new ReleaseError('assetReceiptJsonInvalid'); }
}
export function childEnvironment(env, purpose) {
  const keys = ['PATH', 'HOME', 'USERPROFILE', 'TMPDIR', 'TEMP', 'TMP', 'XDG_CACHE_HOME', 'CI'];
  keys.push(...(purpose === 'github' ? ['GH_TOKEN'] : ['VERCEL_TOKEN', 'VERCEL_ORG_ID', 'VERCEL_PROJECT_ID']));
  return Object.fromEntries(keys.filter(key => env[key] !== undefined).map(key => [key, env[key]]));
}
function protectedResponse(candidate, pathname, env) {
  requireRelease(/^[a-z0-9][a-z0-9-]*\.vercel\.app$/.test(candidate.url || '') && /^\/(?:[A-Za-z0-9_./-])*$/.test(pathname), 'candidateRequestInvalid');
  try { return execFileSync('npx', ['--yes', 'vercel@56.4.1', 'curl', `https://${candidate.url}${pathname}`, '--',
    '--fail', '--silent', '--show-error', '--location', '--max-redirs', '3', '--max-time', '30'], {
    env: childEnvironment(env, 'vercel'), encoding: 'utf8', timeout: 180000, maxBuffer: 8 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'],
  }); } catch { throw new ReleaseError('protectedCandidateFetchFailed'); }
}

export async function runNumeronRelease({ env = process.env, fetchImpl = fetch,
  readConfig = async () => JSON.parse(await readFile(path.join(ROOT, 'mir2-web3/config/numeron-r16-web-assets.json'), 'utf8')),
  readReceipt = downloadReceipt, readCandidate = (candidate, pathname) => protectedResponse(candidate, pathname, env), sleep = ms => new Promise(resolve => setTimeout(resolve, ms)),
  contract = FIXED_R16, onReceipt = async () => {} } = {}) {
  const receipt = { schema: 1, kind: 'numeron-r16-release', passed: false, domainMovePending: true,
    customDomainChanged: false, oldGameProjectMutatedByHelper: false, writes: [], rollbacks: [], credentialsRecorded: false };
  try {
    const { operation, sha } = validateIdentity(env);
    Object.assign(receipt, { operation, sourceSha: sha, implementationSha: env.GITHUB_SHA });
    async function api(host, pathname, method = 'GET', body) {
      const token = host === 'api.vercel.com' ? env.VERCEL_TOKEN : env.GH_TOKEN;
      requireRelease(Boolean(token), 'apiCredentialMissing');
      let response;
      try { response = await fetchImpl(`https://${host}${pathname}`, { method, headers: {
        Authorization: `Bearer ${token}`, 'Content-Type': 'application/json', Accept: 'application/json',
      }, ...(body === undefined ? {} : { body: JSON.stringify(body) }), signal: AbortSignal.timeout(30000) }); }
      catch { throw new ReleaseError('apiTransportFailed'); }
      requireRelease(response.ok, `apiHttp${response.status}`);
      try { const text = await response.text(); return text ? JSON.parse(text) : {}; } catch { throw new ReleaseError('apiJsonInvalid'); }
    }
    const vercel = (pathname, method, body) => api('api.vercel.com', `${pathname}${pathname.includes('?') ? '&' : '?'}teamId=${TEAM}`, method, body);
    async function projects() {
      const result = {};
      for (const [key, site] of Object.entries(SITES)) { result[key] = await vercel(`/v9/projects/${site.project}`); validateProject(key, result[key]); }
      return result;
    }
    const initial = await projects(), config = await readConfig();
    receipt.projects = Object.fromEntries(Object.entries(initial).map(([key, value]) => [key, {
      project: value.id, production: production(value),
      rootDirectory: typeof value.rootDirectory === 'string' ? value.rootDirectory : null,
      nodeVersion: typeof value.nodeVersion === 'string' ? value.nodeVersion : null,
    }]));
    if (operation === 'numeron-context') { receipt.assetConfigMatches = config.version === contract.version && config.objectPrefix === contract.prefix; receipt.passed = true; return receipt; }
    validateAssetConfig(config, contract);
    requireRelease(/^\d+$/.test(env.RELEASE_ASSET_RUN_ID || ''), 'assetRunIdRequired');
    const runId = env.RELEASE_ASSET_RUN_ID, github = pathname => api('api.github.com', `/repos/${REPOSITORY}${pathname}`);
    const run = await github(`/actions/runs/${runId}`), jobs = await github(`/actions/runs/${runId}/jobs?per_page=100`),
      artifacts = await github(`/actions/runs/${runId}/artifacts?per_page=100`);
    requireRelease(!jobs.total_count || jobs.total_count <= 100, 'assetJobsPaginationExceeded');
    requireRelease(!artifacts.total_count || artifacts.total_count <= 100, 'assetArtifactsPaginationExceeded');
    const artifact = validateAssetRun(run, jobs.jobs || [], artifacts.artifacts || [], runId, sha);
    validateAssetReceipt(await readReceipt(runId, artifact.name, env), sha, contract);
    const publicResponse = await fetchImpl(config.remoteReleaseUrl, { redirect: 'error', signal: AbortSignal.timeout(60000) });
    validatePublicManifest(await boundedBytes(publicResponse, contract.manifestBytes), contract);
    receipt.assetPublication = { runId, artifactId: artifact.id, manifestSha256: contract.manifestSha256, manifestBytes: contract.manifestBytes };
    const link = initial.original.link;
    requireRelease(link?.type === 'github' && link.repo === 'mir2' && /^\d+$/.test(String(link.repoId || ''))
      && String(link.org || '').toLowerCase() === 'zombieliu', 'originalGitRepositoryMismatch');
    const ids = { main: env.RELEASE_MAIN_DEPLOYMENT_ID, game: env.RELEASE_DEPLOYMENT_ID };
    async function candidate(key, id) {
      requireRelease(DEPLOYMENT.test(id || ''), 'exactCandidateIdRequired');
      const value = await vercel(`/v13/deployments/${id}?withGitRepoInfo=true`);
      requireRelease(value.id === id, 'candidateIdMismatch');
      let snapshot, until;
      for (let page = 0; page < 5 && !snapshot; page++) {
        const list = await vercel(`/v7/deployments?projectId=${SITES[key].project}&sha=${sha}&target=production&limit=100${until ? `&until=${encodeURIComponent(until)}` : ''}`);
        snapshot = list.deployments?.find(item => item.uid === id); until = list.pagination?.next; if (!until) break;
      }
      candidateGuard(key, value, snapshot, sha, config);
      return value;
    }
    const expected = { original: SITES.original.baseline, main: SITES.main.baseline, game: null };
    if (production(initial.game) !== null) { const own = await candidate('game', ids.game); requireRelease(production(initial.game) === own.id, 'newAppProductionNotOwnedCandidate'); expected.game = own.id; }
    if (operation === 'numeron-stage') {
      guardProjects(initial, expected);
      const staged = {};
      // Validate secret before either candidate is created.
      deploymentBody('game', sha, link.repoId, config, env.MIR2_R16_PASSKEY_AUTH_SECRET);
      for (const key of ['main', 'game']) {
        guardProjects(await projects(), expected);
        receipt.writes.push({ operation: 'stage', site: key, responseKnown: false });
        let value;
        try { value = await vercel('/v13/deployments', 'POST', deploymentBody(key, sha, link.repoId, config, env.MIR2_R16_PASSKEY_AUTH_SECRET)); }
        finally { guardProjects(await projects(), expected); }
        requireRelease(DEPLOYMENT.test(value.id || '') && value.projectId === SITES[key].project, 'stagedDeploymentIdentityInvalid');
        staged[key] = { id: value.id, url: value.url, project: value.projectId, readyState: value.readyState };
        Object.assign(receipt.writes.at(-1), { responseKnown: true, deployment: value.id });
        guardProjects(await projects(), expected);
      }
      receipt.candidates = staged;
    } else {
      const reviewed = { main: await candidate('main', ids.main), game: await candidate('game', ids.game) };
      if (production(initial.main) === reviewed.main.id) expected.main = reviewed.main.id;
      guardProjects(initial, expected);
      const baselineMain = await vercel(`/v13/deployments/${SITES.main.baseline}`);
      requireRelease(baselineMain.id === SITES.main.baseline && baselineMain.projectId === SITES.main.project, 'marketingBaselineIdentityMismatch');
      verifyPages(readCandidate, reviewed, baselineMain, sha);
      receipt.candidates = Object.fromEntries(Object.entries(reviewed).map(([key, value]) => [key, { id: value.id, url: value.url, root: SITES[key].root }]));
      const touched = [];
      try {
        for (const key of ['main', 'game']) {
          guardProjects(await projects(), expected);
          if (expected[key] === reviewed[key].id) continue;
          touched.push({ key, baseline: expected[key] });
          receipt.writes.push({ operation: 'promote', site: key, deployment: reviewed[key].id });
          await vercel(`/v10/projects/${SITES[key].project}/promote/${reviewed[key].id}`, 'POST', {});
          const before = expected[key]; expected[key] = reviewed[key].id;
          for (let attempt = 0; ; attempt++) {
            const current = await projects();
            requireRelease([before, reviewed[key].id].includes(production(current[key])), 'concurrentProductionChange');
            const allowed = { ...expected, [key]: production(current[key]) }; guardProjects(current, allowed);
            if (production(current[key]) === reviewed[key].id) break;
            requireRelease(attempt < 29, 'promotionDidNotConverge'); await sleep(2000);
          }
          guardProjects(await projects(), expected);
        }
      } catch (error) {
        for (const { key, baseline } of touched.reverse()) {
          const current = await projects();
          if (production(current[key]) !== reviewed[key].id) { receipt.rollbacks.push({ site: key, skipped: 'mappingNoLongerOwned' }); continue; }
          if (!baseline) { receipt.rollbacks.push({ site: key, skipped: 'noPriorDeploymentManualReconcile' }); continue; }
          // Recheck all untouched/immutable identities; never roll back a third-party version.
          requireRelease(production(current.original) === SITES.original.baseline, 'originalProductionChangedDuringRollback');
          await vercel(`/v1/projects/${SITES[key].project}/rollback/${baseline}`, 'POST', {});
          const after = await projects(); requireRelease(production(after[key]) === baseline, 'rollbackDidNotConverge');
          receipt.rollbacks.push({ site: key, deployment: baseline });
        }
        throw error;
      }
    }
    receipt.passed = true;
    return receipt;
  } catch (error) { receipt.failureCode = error instanceof ReleaseError ? error.code : 'unexpectedReleaseFailure'; throw Object.assign(error, { releaseReceipt: receipt }); }
  finally { await onReceipt(receipt); }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runNumeronRelease({ onReceipt: receipt => writeFile('numeron-r16-release.json', `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx', mode: 0o600 }) })
    .then(receipt => console.log(JSON.stringify({ operation: receipt.operation, passed: receipt.passed, domainMovePending: true })))
    .catch(error => { console.error(error instanceof ReleaseError ? error.code : 'unexpectedReleaseFailure'); process.exitCode = 1; });
}
