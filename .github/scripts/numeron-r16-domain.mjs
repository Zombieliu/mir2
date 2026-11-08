import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { TEAM, SITES, ORIGIN, ReleaseError, validateIdentity,
  validateProject, guardProjects, candidateGuard, validateAssetConfig, windowsDownloadLinks } from './numeron-r16-release.mjs';

export const DOMAIN = 'numeron.obelisk.build';
const GAME_ALIASES = ['numeron-r16-web.vercel.app', 'numeron-r16-web-obelisk-labs.vercel.app'];
const MAIN_ALIAS = 'numeron-legend-of-rebirth.vercel.app';
const ROOT = fileURLToPath(new URL('../../', import.meta.url));
const ensure = (condition, code) => { if (!condition) throw new ReleaseError(code); };
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const sha256 = value => createHash('sha256').update(value).digest('hex');

export function domainSnapshot(value) {
  if (value === null) return null;
  ensure(value?.name === DOMAIN && [SITES.main.project, SITES.game.project].includes(value.projectId)
    && typeof value.verified === 'boolean', 'domainIdentityInvalid');
  for (const field of ['gitBranch', 'customEnvironmentId', 'redirect'])
    ensure(value[field] == null || typeof value[field] === 'string', 'domainBindingInvalid');
  ensure(value.redirect == null || /^[A-Za-z0-9.-]+$/.test(value.redirect), 'domainRedirectInvalid');
  ensure(value.redirectStatusCode == null || [301, 302, 307, 308].includes(value.redirectStatusCode), 'domainRedirectStatusInvalid');
  return { name: value.name, projectId: value.projectId, verified: value.verified,
    gitBranch: value.gitBranch ?? null, customEnvironmentId: value.customEnvironmentId ?? null,
    redirect: value.redirect ?? null, redirectStatusCode: value.redirectStatusCode ?? null,
    updatedAt: Number.isFinite(value.updatedAt) ? value.updatedAt : null };
}
export function productionDomain(value, projectId) {
  return value?.projectId === projectId && value.name === DOMAIN && value.gitBranch === null
    && value.customEnvironmentId === null && value.redirect === null && value.redirectStatusCode === null;
}
export function aliasMatches(value, projectId, deploymentId, hostname) {
  return value?.alias === hostname && value.projectId === projectId && value.deploymentId === deploymentId
    && value.redirect == null && value.microfrontends == null && value.deletedAt == null;
}
function sourceAliasMatches(value, before, deploymentId) {
  if (before.redirect === null) return aliasMatches(value, SITES.main.project, deploymentId, DOMAIN);
  return value?.alias === DOMAIN && [null, SITES.main.project].includes(value.projectId)
    && value.redirect === before.redirect && value.redirectStatusCode === before.redirectStatusCode
    && value.microfrontends == null && value.deletedAt == null;
}
function restoredBinding(value, before) {
  return value !== null && value.projectId === before.projectId && value.name === before.name
    && value.verified === true && value.gitBranch === before.gitBranch && value.customEnvironmentId === before.customEnvironmentId
    && value.redirect === before.redirect && value.redirectStatusCode === before.redirectStatusCode;
}
export function forwardBody() {
  return { projectId: SITES.game.project, gitBranch: null, redirect: null, redirectStatusCode: null };
}
export function rollbackBody(before) {
  ensure(before.projectId === SITES.main.project && before.gitBranch === null
    && before.customEnvironmentId === null, 'sourceDomainNotProduction');
  return { projectId: SITES.main.project, gitBranch: before.gitBranch,
    redirect: before.redirect, redirectStatusCode: before.redirectStatusCode };
}

export async function runNumeronDomain({ env = process.env, fetchImpl = fetch,
  readConfig = async () => JSON.parse(await readFile(path.join(ROOT, 'mir2-web3/config/numeron-r16-web-assets.json'), 'utf8')),
  sleep = ms => new Promise(resolve => setTimeout(resolve, ms)), onReceipt = async () => {} } = {}) {
  const receipt = { schema: 1, kind: 'numeron-r16-project-domain-move', domain: DOMAIN,
    passed: false, phase: 'preflight', manualReconcile: false, mutations: [], rollback: null,
    serverSideCompareAndSwap: false, dnsChanged: false, registrationChanged: false,
    oldGameProjectMutatedByHelper: false, credentialsRecorded: false };
  let before, ownershipWitness, controls, attempted = false;
  try {
    ensure(env.RELEASE_OPERATION === 'numeron-domain-move', 'explicitDomainOperationRequired');
    // Domain cutover has the same immutable-source/CI identity requirements as
    // promotion, while remaining a separately selected operation.
    const { sha } = validateIdentity({ ...env, RELEASE_OPERATION: 'numeron-promote' });
    const ids = { main: env.RELEASE_MAIN_DEPLOYMENT_ID, game: env.RELEASE_DEPLOYMENT_ID };
    ensure(Object.values(ids).every(id => /^dpl_[A-Za-z0-9]+$/.test(id || '')), 'exactCandidateIdsRequired');
    Object.assign(receipt, { sourceSha: sha, implementationSha: env.GITHUB_SHA, candidates: ids,
      sourceProject: SITES.main.project, targetProject: SITES.game.project });
    const config = await readConfig(); validateAssetConfig(config);
    async function api(endpoint, method = 'GET', body, missingAllowed = false) {
      let response;
      try { response = await fetchImpl(`https://api.vercel.com${endpoint}${endpoint.includes('?') ? '&' : '?'}teamId=${TEAM}`, {
        method, headers: { Authorization: `Bearer ${env.VERCEL_TOKEN}`, 'Content-Type': 'application/json' },
        ...(body === undefined ? {} : { body: JSON.stringify(body) }), signal: AbortSignal.timeout(30000), redirect: 'error',
      }); } catch { throw new ReleaseError('domainApiTransportUnknown'); }
      if (missingAllowed && response.status === 404) return null;
      ensure(response.ok, `domainApiHttp${response.status}`);
      try { return await response.json(); } catch { throw new ReleaseError('domainApiJsonInvalid'); }
    }
    const expected = { original: SITES.original.baseline, main: ids.main, game: ids.game };
    async function projectsCurrent() {
      const result = {};
      for (const [key, site] of Object.entries(SITES)) { result[key] = await api(`/v9/projects/${site.project}`); validateProject(key, result[key]); }
      guardProjects(result, expected); return result;
    }
    await projectsCurrent();
    for (const key of ['main', 'game']) {
      const candidate = await api(`/v13/deployments/${ids[key]}?withGitRepoInfo=true`);
      ensure(candidate.id === ids[key], 'domainCandidateIdMismatch');
      let snapshot, until;
      for (let page = 0; page < 5 && !snapshot; page++) {
        const list = await api(`/v7/deployments?projectId=${SITES[key].project}&sha=${sha}&target=production&limit=100${until ? `&until=${encodeURIComponent(until)}` : ''}`);
        snapshot = list.deployments?.find(item => item.uid === ids[key]); until = list.pagination?.next; if (!until) break;
      }
      candidateGuard(key, candidate, snapshot, sha, config);
    }
    const readDomain = async key => domainSnapshot(await api(`/v9/projects/${SITES[key].project}/domains/${DOMAIN}`, 'GET', undefined, true));
    const readAlias = hostname => api(`/v4/aliases/${hostname}`, 'GET', undefined, true);
    controls = { api, projectsCurrent, readDomain, readAlias };
    const primaryAlias = await readAlias(MAIN_ALIAS);
    ensure(aliasMatches(primaryAlias, SITES.main.project, ids.main, MAIN_ALIAS), 'marketingDefaultAliasMismatch');
    let gameAlias;
    for (const hostname of GAME_ALIASES) {
      const value = await readAlias(hostname);
      if (value === null) continue;
      ensure(aliasMatches(value, SITES.game.project, ids.game, hostname), 'gameDefaultAliasMismatch'); gameAlias = hostname; break;
    }
    ensure(gameAlias, 'gameDefaultAliasMissing'); receipt.defaultGameAlias = gameAlias;

    async function publicText(origin, pathname, allowRootRedirect = false) {
      let url = new URL(pathname, origin);
      for (let count = 0; count < 4; count++) {
        let response;
        try { response = await fetchImpl(url.href, { headers: { 'Cache-Control': 'no-cache' },
          cache: 'no-store', redirect: 'manual', signal: AbortSignal.timeout(15000) }); }
        catch { throw new ReleaseError('domainPublicReadFailed'); }
        if ([301, 302, 303, 307, 308].includes(response.status)) {
          ensure(allowRootRedirect, 'domainPublicUnexpectedRedirect');
          const location = response.headers.get('location'); ensure(location, 'domainPublicRedirectMissing');
          const next = new URL(location, url);
          ensure(next.origin === origin && !next.username && !next.password, 'domainPublicForeignRedirect'); url = next; continue;
        }
        ensure(response.status === 200, 'domainPublicStatusInvalid');
        const chunks = []; let bytes = 0;
        for await (const chunk of response.body) { bytes += chunk.length; ensure(bytes <= 8 * 1024 * 1024, 'domainPublicResponseTooLarge'); chunks.push(chunk); }
        return Buffer.concat(chunks).toString('utf8');
      }
      throw new ReleaseError('domainPublicRedirectLimit');
    }
    async function pages(origin) {
      let version, manifest;
      try { version = JSON.parse(await publicText(origin, '/version')); manifest = JSON.parse(await publicText(origin, '/manifest.webmanifest')); }
      catch (error) { if (error instanceof ReleaseError) throw error; throw new ReleaseError('domainPublicJsonInvalid'); }
      ensure(version.revision === sha && manifest.start_url === '/game' && manifest.scope === '/', 'domainPublicVersionOrPwaMismatch');
      const game = await publicText(origin, '/game');
      ensure(game.includes('/_next/static/') && !game.includes('navigation_promotions'), 'domainPublicGameInvalid');
      const home = await publicText(origin, '/', true);
      ensure(home.includes('/marketing-assets/_next/static/') && home.includes('navigation_promotions') && home.includes('footer_promotions'), 'domainPublicMarketingInvalid');
      const download = await publicText(origin, '/zh-CN/download');
      ensure(download.includes(`${ORIGIN}/game`), 'domainPublicGameLinkInvalid');
      const marketingDownload = await publicText(`https://${MAIN_ALIAS}`, '/zh-CN/download');
      ensure(same(windowsDownloadLinks(download), windowsDownloadLinks(marketingDownload)), 'domainWindowsDownloadsChanged');
      return { revision: version.revision, gameHtmlSha256: sha256(game), marketingHtmlSha256: sha256(home), downloadHtmlSha256: sha256(download) };
    }
    receipt.defaultAliasProof = await pages(`https://${gameAlias}`);
    async function sourceDomains(key = 'main') {
      const result = []; let until;
      for (let page = 0; page < 20; page++) {
        const list = await api(`/v9/projects/${SITES[key].project}/domains?limit=100${until ? `&until=${encodeURIComponent(until)}` : ''}`);
        ensure(Array.isArray(list.domains), 'sourceDomainListInvalid');
        for (const item of list.domains) {
          ensure(typeof item.name === 'string' && item.projectId === SITES[key].project, 'sourceDomainListIdentityMismatch');
          result.push({ name: item.name, redirect: item.redirect ?? null });
        }
        until = list.pagination?.next;
        if (!until) { ensure(new Set(result.map(item => item.name)).size === result.length, 'sourceDomainListDuplicate'); return result.sort((a, b) => a.name.localeCompare(b.name)); }
      }
      throw new ReleaseError('sourceDomainListPaginationLimit');
    }
    controls.projectDomains = sourceDomains;
    const initialDomains = await sourceDomains();
    const inbound = initialDomains.filter(item => typeof item.redirect === 'string' && item.redirect.trim().toLowerCase().replace(/\.$/, '') === DOMAIN);
    receipt.inboundRedirects = inbound.map(item => item.name);
    ensure(inbound.length === 0, 'domainHasInboundRedirects');
    const others = initialDomains.filter(item => item.name !== DOMAIN);
    before = await readDomain('main'); const existingTarget = await readDomain('game');
    if (before === null && existingTarget !== null) {
      ensure(productionDomain(existingTarget, SITES.game.project) && existingTarget.verified
        && aliasMatches(await readAlias(DOMAIN), SITES.game.project, ids.game, DOMAIN), 'domainAlreadyMovedButBindingMismatch');
      await projectsCurrent(); receipt.publicProof = await pages(ORIGIN);
      await projectsCurrent();
      ensure(same(await readDomain('game'), existingTarget) && await readDomain('main') === null
        && aliasMatches(await readAlias(DOMAIN), SITES.game.project, ids.game, DOMAIN), 'domainConcurrentChangeAfterMove');
      receipt.alreadyAtTarget = true; receipt.passed = true; receipt.phase = 'complete'; return receipt;
    }
    ensure(before?.projectId === SITES.main.project && before.verified === true && existingTarget === null, 'sourceDomainOwnershipMismatch');
    ensure(before.gitBranch === null && before.customEnvironmentId === null, 'sourceDomainNotProduction');
    ensure(initialDomains.some(item => item.name === DOMAIN), 'sourceDomainListMissingPrimary');
    ensure(sourceAliasMatches(await readAlias(DOMAIN), before, ids.main), 'sourceDomainAliasMismatch');
    receipt.before = before;
    await projectsCurrent();
    ensure(same(await readDomain('main'), before) && await readDomain('game') === null
      && same(await sourceDomains(), initialDomains)
      && sourceAliasMatches(await readAlias(DOMAIN), before, ids.main), 'domainConcurrentChangeBeforeMove');
    const move = (source, body) => api(`/v1/projects/${source}/domains/${DOMAIN}/move`, 'POST', body);
    attempted = true; receipt.phase = 'move-requested'; receipt.mutations.push({ source: SITES.main.project, target: SITES.game.project });
    try {
      const response = domainSnapshot(await move(SITES.main.project, forwardBody()));
      ensure(productionDomain(response, SITES.game.project), 'domainMoveResponseBindingMismatch');
      receipt.moveResponseVerified = response.verified;
    } catch (error) { receipt.moveResponseUnknown = true; receipt.moveErrorCode = error instanceof ReleaseError ? error.code : 'domainMoveUnknown'; }

    // A failed/unknown POST is never retried. Only reads establish its outcome.
    let unchangedSource = false;
    for (let attempt = 0; attempt < 10; attempt++) {
      await projectsCurrent();
      const source = await readDomain('main'), target = await readDomain('game'), alias = await readAlias(DOMAIN);
      if (source === null && productionDomain(target, SITES.game.project)
        && aliasMatches(alias, SITES.game.project, ids.game, DOMAIN)) { ownershipWitness = target; break; }
      unchangedSource = same(source, before) && target === null;
      if ((source !== null && !same(source, before)) || (target !== null && !productionDomain(target, SITES.game.project))) {
        receipt.manualReconcile = true; throw new ReleaseError('domainMoveOwnershipAmbiguous');
      }
      await sleep(1000);
    }
    if (!ownershipWitness) { receipt.manualReconcile = !unchangedSource || receipt.moveResponseUnknown === true; throw new ReleaseError('domainMoveNotConverged'); }
    ensure(ownershipWitness.verified === true, 'targetDomainNotVerified');
    ensure(same((await sourceDomains()).filter(item => item.name !== DOMAIN), others), 'unrelatedSourceDomainsChanged');
    let publicError;
    for (let attempt = 0; attempt < 3; attempt++) {
      try { receipt.publicProof = await pages(ORIGIN); publicError = null; break; }
      catch (error) { publicError = error; await sleep(1000); }
    }
    if (publicError) throw publicError;
    await projectsCurrent();
    ensure(same(await readDomain('game'), ownershipWitness) && await readDomain('main') === null
      && aliasMatches(await readAlias(DOMAIN), SITES.game.project, ids.game, DOMAIN), 'domainConcurrentChangeAfterMove');
    receipt.after = ownershipWitness; receipt.passed = true; receipt.phase = 'complete'; return receipt;
  } catch (error) {
    receipt.failureCode = error instanceof ReleaseError ? error.code : 'unexpectedDomainFailure';
    if (attempted && ownershipWitness && before) {
      try {
        // Re-read both project generations and the complete domain binding just
        // before a reverse move. No delete/add, third-party overwrite or retry.
        await controls.projectsCurrent();
        ensure(same(await controls.readDomain('game'), ownershipWitness) && await controls.readDomain('main') === null
          && aliasMatches(await controls.readAlias(DOMAIN), SITES.game.project, env.RELEASE_DEPLOYMENT_ID, DOMAIN), 'rollbackDomainNoLongerOwned');
        const reverseSourceDomains = await controls.projectDomains('game');
        ensure(!reverseSourceDomains.some(item => typeof item.redirect === 'string'
          && item.redirect.trim().toLowerCase().replace(/\.$/, '') === DOMAIN), 'rollbackDomainHasInboundRedirects');
        receipt.mutations.push({ source: SITES.game.project, target: SITES.main.project });
        try {
          const value = domainSnapshot(await controls.api(`/v1/projects/${SITES.game.project}/domains/${DOMAIN}/move`, 'POST', rollbackBody(before)));
          ensure(restoredBinding(value, before), 'rollbackBindingMismatch');
        } catch (rollbackError) { receipt.rollbackResponseUnknown = true; receipt.rollbackErrorCode = rollbackError instanceof ReleaseError ? rollbackError.code : 'rollbackUnknown'; }
        let confirmed;
        for (let attempt = 0; attempt < 10; attempt++) {
          await controls.projectsCurrent();
          const value = await controls.readDomain('main');
          if (restoredBinding(value, before) && await controls.readDomain('game') === null
            && sourceAliasMatches(await controls.readAlias(DOMAIN), before, env.RELEASE_MAIN_DEPLOYMENT_ID)) { confirmed = value; break; }
          await sleep(1000);
        }
        ensure(confirmed, 'rollbackNotConfirmed');
        await controls.projectsCurrent();
        ensure(same(await controls.readDomain('main'), confirmed) && await controls.readDomain('game') === null, 'rollbackConcurrentChange');
        receipt.rollback = { confirmed: true, projectId: confirmed.projectId };
        receipt.manualReconcile = receipt.failureCode === 'unrelatedSourceDomainsChanged';
      } catch (rollbackError) { receipt.rollback = { confirmed: false, code: rollbackError instanceof ReleaseError ? rollbackError.code : 'rollbackUnknown' }; receipt.manualReconcile = true; }
    } else if (attempted) receipt.manualReconcile = true;
    throw Object.assign(error, { releaseReceipt: receipt });
  } finally { await onReceipt(receipt); }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runNumeronDomain({ onReceipt: value => writeFile('numeron-r16-domain.json', `${JSON.stringify(value, null, 2)}\n`, { flag: 'wx', mode: 0o600 }) })
    .then(value => console.log(JSON.stringify({ passed: value.passed, domain: DOMAIN, manualReconcile: value.manualReconcile })))
    .catch(error => { console.error(error instanceof ReleaseError ? error.code : 'unexpectedDomainFailure'); process.exitCode = 1; });
}
