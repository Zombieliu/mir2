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
export const QR_RELEASE = Object.freeze({
  source: '72c00d20aeb82a9f87c79c2a8567a3146d9a2cfc',
  sourceRef: BRANCH,
  baseSource: '1cc89f69a05dd35046f4497254a14b91128da1ed',
  assetSource: 'f2de762309768b51b39438c2437c14e819df9bf7',
  changedFiles: [
    {
      "status": "modified",
      "filename": "mir2-web3/apps/web/app/components/original-client-shell-types.ts",
      "sha": "a19c7a6e68a20f81eb32fdd8a8439e9a93e0d9c4"
    },
    {
      "status": "modified",
      "filename": "mir2-web3/apps/web/app/original-client-shell.tsx",
      "sha": "2c9144199416d1fbd654de9fc00ab83ca2c4f8dc"
    },
    {
      "status": "modified",
      "filename": "mir2-web3/apps/web/app/page.tsx",
      "sha": "b1747f8c09b222adaf1edfbd0c6dd6c0830335e1"
    },
    {
      "status": "modified",
      "filename": "mir2-web3/apps/web/lib/use-bevy-quest-ui.ts",
      "sha": "bd8987c9b3b527655d5e8709cdeeeb29d83b0c67"
    },
    {
      "status": "modified",
      "filename": "mir2-web3/apps/web/scripts/test-bevy-quest-ui.mjs",
      "sha": "615444c3cb845e9aa63084d0d8743f437e82dc97"
    },
    {
      "status": "modified",
      "filename": "mir2-web3/apps/web/scripts/test-bevy-runtime-policy.mjs",
      "sha": "cfb69e4be11e8acd090b10b48ef1365bad1df671"
    },
    {
      "status": "modified",
      "filename": "mir2-web3/apps/web/scripts/test-r16-keyboard-ownership.mjs",
      "sha": "9975b30a998078e5b2846b58fa44d5ca3d61b7d9"
    },
    {
      "status": "modified",
      "filename": "mir2-web3/docs/AGENT-CURRENT.md",
      "sha": "80bacafc3f258447b0b8b8241c6033ddc6550327"
    },
    {
      "status": "modified",
      "filename": "mir2-web3/docs/AGENT-TASK-QUEUE.md",
      "sha": "e0aaae454f92d9e7348d30b45de4e04bd744e6d6"
    },
    {
      "status": "modified",
      "filename": "mir2-web3/docs/FRONTEND-1TO1-GAPS.md",
      "sha": "750f3f0a09f9cefd0224d636864130c5715b0383"
    },
    {
      "status": "modified",
      "filename": "mir2-web3/docs/WEB-R16-PARITY-GOAL.zh-CN.md",
      "sha": "55a95b540eda43e5e2d5d020bf777ef07dbbb6c8"
    },
    {
      "status": "added",
      "filename": "mir2-web3/docs/evidence/r16-native-input-recovery-20261112/README.md",
      "sha": "29d02c5ed20a52499d749dad3e1a22cc65867761"
    },
    {
      "status": "added",
      "filename": "mir2-web3/docs/evidence/r16-native-input-recovery-20261112/baseline-keyboard-quest.log",
      "sha": "ba0b8a183cceb4303d56b77457d73fa4c8ce491a"
    },
    {
      "status": "added",
      "filename": "mir2-web3/docs/evidence/r16-native-input-recovery-20261112/candidate-adjacent-green.log",
      "sha": "572324090719093abee54c537e48bba8af583f02"
    },
    {
      "status": "added",
      "filename": "mir2-web3/docs/evidence/r16-native-input-recovery-20261112/dispatch-removal-red.log",
      "sha": "68606c201f8e28cd4155d73764719900cf8723d7"
    },
    {
      "status": "added",
      "filename": "mir2-web3/docs/evidence/r16-native-input-recovery-20261112/keyboard-red.log",
      "sha": "9cc04f4e5a363cebe5b3a16dd3f1429df57a29a0"
    },
    {
      "status": "added",
      "filename": "mir2-web3/docs/evidence/r16-native-input-recovery-20261112/production-build-r2.log",
      "sha": "83258dc972c3aa03e5307b01d60452f3e51c6f8f"
    },
    {
      "status": "added",
      "filename": "mir2-web3/docs/evidence/r16-native-input-recovery-20261112/quest-after-r7-hud.png",
      "sha": "d1ff3a64083e33defb30b65326f2eb3ac87683ba"
    },
    {
      "status": "added",
      "filename": "mir2-web3/docs/evidence/r16-native-input-recovery-20261112/quest-after-r8-hud.png",
      "sha": "bafdbf16a60e752759c17d10a4878b179a03060a"
    },
    {
      "status": "added",
      "filename": "mir2-web3/docs/evidence/r16-native-input-recovery-20261112/receipt.json",
      "sha": "047b77ef20d345f8874847cd392d82989761471a"
    },
    {
      "status": "added",
      "filename": "mir2-web3/docs/evidence/r16-native-input-recovery-20261112/shortcut-adjacent-green.log",
      "sha": "89ce3fd84a57ad84e725851ee879886cbf7d6c65"
    },
    {
      "status": "added",
      "filename": "mir2-web3/docs/evidence/r16-native-input-recovery-20261112/ui-intent-red.log",
      "sha": "71e70e6f42caf3bcaa3257a5c3be20841abe8171"
    }
  ],
  entryScene: {
    path: '/bootstrap/r16-scenes/5c08d1fd4e81d92a7be20eb98c4c6288515749d5d459978dcf78fe2b86bdf325.json',
    sha256: '5c08d1fd4e81d92a7be20eb98c4c6288515749d5d459978dcf78fe2b86bdf325',
    bytes: 458475,
  },
  entryFloor: {
    path: '/bootstrap/r16-floor/e4977afa3640bec9d8043ae32d794f92385a309fb4fa68cf407fe575016ce90d.json',
    sha256: 'e4977afa3640bec9d8043ae32d794f92385a309fb4fa68cf407fe575016ce90d',
    bytes: 439896,
  },
  entryPlayerMale: {
    path: '/bootstrap/r16-visuals/164d9bec51c291f17757e3c74d7e2d128d9747f5723165bc1a25c99f94445d08.json',
    sha256: '164d9bec51c291f17757e3c74d7e2d128d9747f5723165bc1a25c99f94445d08',
    bytes: 289851,
  },
  entryPlayerFemale: {
    path: '/bootstrap/r16-visuals/0f52c6a6632c44eea1ab3cf37eb58da52f6c786cf3573813d595fcdd8ca040d0.json',
    sha256: '0f52c6a6632c44eea1ab3cf37eb58da52f6c786cf3573813d595fcdd8ca040d0',
    bytes: 282415,
  },
  entryNpcs: {
    path: '/bootstrap/r16-visuals/9093bb29d7f91ab07a44a47e888e2f1d40151130c1253ecf9451af63c4e057d9.json',
    sha256: '9093bb29d7f91ab07a44a47e888e2f1d40151130c1253ecf9451af63c4e057d9',
    bytes: 313909,
  },
  entryObjects1: {
    path: '/bootstrap/r16-visuals/2530a00967ee0aa60d4802ab50690166d676c2c58b13567dc1fdde9951e0f5b5.json',
    sha256: '2530a00967ee0aa60d4802ab50690166d676c2c58b13567dc1fdde9951e0f5b5',
    bytes: 395412,
  },
  entryObjects2: {
    path: '/bootstrap/r16-visuals/8d4ebe2ab7644fb7abf46cebab8c04c52747a3202049630948ef7726c581d89d.json',
    sha256: '8d4ebe2ab7644fb7abf46cebab8c04c52747a3202049630948ef7726c581d89d',
    bytes: 395320,
  },
  entryObjects3: {
    path: '/bootstrap/r16-visuals/348dbb84596b95d7bfd065afb78cd4a582df808965b880e06fa5dbd1be05c0d1.json',
    sha256: '348dbb84596b95d7bfd065afb78cd4a582df808965b880e06fa5dbd1be05c0d1',
    bytes: 395216,
  },
  entryMonsterStanding: {
    path: '/bootstrap/r16-visuals/d3f04c400820d8cd66b49b8af638a86e719bcf1032082be5a7ec80757bdf4308.json',
    sha256: 'd3f04c400820d8cd66b49b8af638a86e719bcf1032082be5a7ec80757bdf4308',
    bytes: 513566,
  },
  entryMonsterWalking: {
    path: '/bootstrap/r16-visuals/41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f.json',
    sha256: '41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f',
    bytes: 764724,
  },
  entryGameplayLocale: {
    path: '/bootstrap/r16-locales/b373a5f2cd21d5d0270610e0b7d1d619d5e554e12a67a7190c768d59e01dd64c.json',
    sha256: 'b373a5f2cd21d5d0270610e0b7d1d619d5e554e12a67a7190c768d59e01dd64c',
    bytes: 3283113,
  },
  assetRunId: '42494701555',
  baseline: 'dpl_CWdmzkhCR1isr22yeXM3o5WekiwB',
  relay: 'https://relay.dubhe.obelisk.build',
});
const QR_OPERATIONS = ['numeron-qr-stage', 'numeron-qr-promote', 'numeron-qr-rollback'];
const WALLET_ORIGIN = 'https://wallet.dubhe.obelisk.build';
const QR_SESSION_PATH = '/api/wallet/dubhe/session';
const DEPLOYMENT = /^dpl_[A-Za-z0-9]+$/;
const SHA = /^[a-f0-9]{40}$/;
const ROOT = fileURLToPath(new URL('../../', import.meta.url));
const requireRelease = (condition, code) => { if (!condition) throw new ReleaseError(code); };
export class ReleaseError extends Error { constructor(code) { super(code); this.code = code; } }
export const production = project => project.targets?.production?.id ?? project.targets?.production?.deploymentId ?? null;
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

export function validateIdentity(env) {
  const operation = env.RELEASE_OPERATION;
  const qr = QR_OPERATIONS.includes(operation);
  const sha = qr ? QR_RELEASE.source : env.RELEASE_SOURCE_SHA || env.GITHUB_SHA;
  requireRelease(qr || ['numeron-context', 'numeron-stage', 'numeron-promote'].includes(operation), 'operationInvalid');
  requireRelease(env.GITHUB_REPOSITORY === REPOSITORY && env.GITHUB_REF === `refs/heads/${BRANCH}`
    && SHA.test(env.GITHUB_SHA || '') && SHA.test(sha || ''), 'sourceIdentityInvalid');
  requireRelease(operation !== 'numeron-stage' || sha === env.GITHUB_SHA, 'stageSourceMustEqualCheckout');
  if (qr) {
    requireRelease(!env.RELEASE_SOURCE_SHA && !env.RELEASE_ASSET_RUN_ID && !env.RELEASE_MAIN_DEPLOYMENT_ID,
      'qrSourceInputsArePinned');
    const id = env.RELEASE_DEPLOYMENT_ID;
    requireRelease(operation === 'numeron-qr-stage' ? !id : DEPLOYMENT.test(id || '') && id !== QR_RELEASE.baseline,
      'qrCandidateInputInvalid');
  }
  requireRelease(Boolean(env.VERCEL_TOKEN) && env.VERCEL_ORG_ID === TEAM
    && env.VERCEL_PROJECT_ID === SITES.original.project, 'originalCiIdentityInvalid');
  return { operation, sha, qr };
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
    // An explicit empty build/runtime value overrides the repository's .env.production.
    // The current public marketing baseline has no available Windows installer.
    NEXT_PUBLIC_WINDOWS_DOWNLOAD_URL: '',
    GAME_HEALTH_URL: 'https://165.154.65.136.sslip.io/playtest/health', EXPLORER_ORIGIN: 'https://numeron-explorer.vercel.app',
  } : {
    NEXT_PUBLIC_MIR2_GATEWAY_WS_URL: 'wss://165.154.65.136.sslip.io/playtest/ws',
    MIR2_GATEWAY_HTTP_URL: 'https://165.154.65.136.sslip.io/playtest', NEXT_PUBLIC_MIR2_QUEST_GUIDANCE: 'newcomer-v2',
    MIR2_RUNTIME_ENV: 'test', MIR2_MARKETING_ORIGIN: 'https://numeron-legend-of-rebirth.vercel.app',
    MIR2_PASSKEY_ALLOWED_ORIGINS: [ORIGIN, 'https://numeron-r16-web.vercel.app', 'https://numeron-r16-web-obelisk-labs.vercel.app'].join(','),
    MIR2_USE_PREBUILT_BEVY_RUNTIME: '1', MIR2_USE_PREBUILT_CLIENT_CORE: '1', MIR2_REUSE_ORIGINAL_ASSET_MANIFEST: '1',
    MIR2_PRODUCTION_ASSET_CONFIG: 'numeron-r16-web-assets.json',
    MIR2_DEPLOY_REVISION: sha, NEXT_PUBLIC_MIR2_ASSET_BASE_URL: config.assetBaseUrl, MIR2_ASSET_OBJECT_PREFIX: config.objectPrefix,
    MIR2_R2_PROXY_BASE: config.assetBaseUrl, MIR2_ASSET_BASE_URL: config.assetBaseUrl,
  };
}
export function deploymentBody(key, sha, repoId, config, secret, sourceRef = BRANCH) {
  const environment = publicEnvironment(key, sha, config);
  if (key === 'game') requireRelease(typeof secret === 'string' && secret.length >= 32, 'gatewayAuthSecretMissing');
  return { name: SITES[key].name, project: SITES[key].project, target: 'production', autoAssignCustomDomains: false,
    gitSource: { type: 'github', repoId, ref: sourceRef, sha },
    projectSettings: { rootDirectory: SITES[key].root, framework: 'nextjs', nodeVersion: '22.x',
      buildCommand: key === 'game' ? 'bash ./scripts/vercel-build.sh' : 'npm run build',
      installCommand: 'npm ci', outputDirectory: null, sourceFilesOutsideRootDirectory: true },
    env: { ...environment, ...(key === 'game' ? {
      MIR2_PASSKEY_AUTH_SECRET: secret, MIR2_DUBHE_CONNECT_RELAY_URL: QR_RELEASE.relay,
    } : {}) }, build: { env: environment },
    meta: { numeronR16SourceSha: sha, numeronR16Root: SITES[key].root, numeronR16AssetVersion: config.version,
      numeronR16OriginalBaseline: SITES.original.baseline, numeronR16MainBaseline: SITES.main.baseline,
      ...(key === 'main' ? { numeronR16WindowsDownloadPolicy: 'unavailable-baseline' } : {}) },
  };
}
export function candidateGuard(key, candidate, snapshot, sha, config, sourceRef = BRANCH) {
  const site = SITES[key];
  // Modern Git deployments may omit root from their REST snapshots. Every
  // explicit root must agree; missing roots still require our staged metaRoot,
  // exact Git source and the protected page proofs before any promotion.
  const roots = [candidate?.rootDirectory, candidate?.projectSettings?.rootDirectory,
    snapshot?.rootDirectory, snapshot?.projectSettings?.rootDirectory].filter(root => root != null);
  requireRelease(candidate?.projectId === site.project && candidate.target === 'production' && candidate.readyState === 'READY'
    && candidate.gitSource?.sha === sha && candidate.gitSource?.ref === sourceRef
    && candidate.meta?.numeronR16SourceSha === sha && candidate.meta?.numeronR16Root === site.root
    && candidate.meta?.numeronR16AssetVersion === config.version
    && candidate.meta?.numeronR16OriginalBaseline === SITES.original.baseline
    && candidate.meta?.numeronR16MainBaseline === SITES.main.baseline
    && snapshot?.projectId === site.project && roots.every(root => root === site.root), 'candidateIdentitySourceRootMismatch');
  requireRelease(/^[a-z0-9][a-z0-9-]*\.vercel\.app$/.test(candidate.url || ''), 'candidateHostnameInvalid');
}
function downloadMarkup(html) {
  requireRelease(typeof html === 'string', 'downloadPageHtmlInvalid');
  // Next's serialized data and script source do not establish rendered page identity.
  return html.replace(/<!--[\s\S]*?-->|<(script|style)\b[^>]*>[\s\S]*?<\/\1\s*>/gi, '');
}
function trackedDownloadAnchors(markup, track) {
  const marker = new RegExp(`\\sdata-track\\s*=\\s*(?:"${track}"|'${track}'|${track}(?=\\s|/?>))`, 'i');
  return [...markup.matchAll(/<a\b[^>]*>/gi)].map(match => match[0]).filter(tag => marker.test(tag));
}
export function validateDownloadPage(html) {
  const markup = downloadMarkup(html);
  const page = [...markup.matchAll(/<main\b[^>]*>/gi)].some(match => {
    const classes = /\sclass\s*=\s*(["'])(.*?)\1/i.exec(match[0]);
    return classes?.[2].split(/\s+/).includes('download-page');
  });
  requireRelease(/<html\b/i.test(markup) && page
    && ['download_web', 'navigation_promotions', 'footer_promotions'].every(track => trackedDownloadAnchors(markup, track).length > 0), 'downloadPageHtmlInvalid');
}
export function windowsDownloadLinks(html, { allowEmpty = false } = {}) {
  // Empty is a page-specific policy, never the default for arbitrary missing markup.
  if (allowEmpty === true) validateDownloadPage(html);
  const links = trackedDownloadAnchors(downloadMarkup(html), 'download_windows').map(tag => {
    if ([...tag.matchAll(/\shref\s*=/gi)].length !== 1) return undefined;
    return /\shref\s*=\s*(["'])(.*?)\1/i.exec(tag)?.[2]?.replaceAll('&amp;', '&');
  });
  requireRelease((links.length > 0 || allowEmpty === true) && links.every(link => {
    if (typeof link !== 'string' || !link || /[\u0000-\u0020\u007f<>"'\\]|&#|&[A-Za-z]+;/.test(link)) return false;
    try { const url = new URL(link); return url.protocol === 'https:' && !url.username && !url.password
      && !/^https:\/\/[^/?#]*@/i.test(link); } catch { return false; }
  }), 'windowsDownloadLinkMissing');
  return [...new Set(links)].sort();
}
export function verifyPages(read, candidates, baselineMain, sha) {
  const marketing = read(candidates.main, '/zh-CN');
  requireRelease(marketing.includes('/marketing-assets/_next/static/') && marketing.includes('navigation_promotions')
    && marketing.includes('footer_promotions'), 'marketingHtmlInvalid');
  const download = read(candidates.main, '/zh-CN/download');
  const baselineDownload = read(baselineMain, '/zh-CN/download');
  validateDownloadPage(download); validateDownloadPage(baselineDownload);
  requireRelease(download.includes(`${ORIGIN}/game`), 'marketingGameLinkInvalid');
  requireRelease(JSON.stringify(windowsDownloadLinks(download, { allowEmpty: true }))
    === JSON.stringify(windowsDownloadLinks(baselineDownload, { allowEmpty: true })), 'windowsDownloadsChanged');
  requireRelease(read(candidates.main, '/en').includes('lang="en"'), 'marketingEnglishRouteInvalid');
  verifyGamePages(read, candidates.game, sha);
}

function verifyGamePages(read, candidate, sha) {
  let version, manifest;
  try { version = JSON.parse(read(candidate, '/version')); manifest = JSON.parse(read(candidate, '/manifest.webmanifest')); }
  catch { throw new ReleaseError('candidateVersionOrManifestInvalid'); }
  requireRelease(version.revision === sha && manifest.start_url === '/game' && manifest.scope === '/', 'candidateVersionOrPwaMismatch');
  const game = read(candidate, '/game');
  requireRelease(game.includes('/_next/static/') && !game.includes('navigation_promotions'), 'gameHtmlInvalid');
}

function verifyEntryAsset(bytes, entry, code) {
  requireRelease(bytes.length === entry.bytes && digest(bytes) === entry.sha256, code);
  return entry;
}

async function proveQrRelay(read) {
  let created, approval;
  try {
    created = JSON.parse(await read(QR_SESSION_PATH));
    approval = new URL(created.approvalUrl);
  } catch { throw new ReleaseError('qrCreateFailed'); }
  const lifetime = created.expiresAt - Date.now();
  requireRelease(typeof created.intentToken === 'string' && /^[A-Za-z0-9_.-]{1,12000}$/.test(created.intentToken)
    && /^[A-Za-z0-9_-]{1,256}$/.test(created.sessionId || '') && Number.isSafeInteger(created.expiresAt)
    && lifetime > 0 && lifetime <= 125_000 && approval.origin === WALLET_ORIGIN
    && approval.pathname === '/connect/approve' && !approval.username && !approval.password && !approval.hash
    && [...approval.searchParams.keys()].sort().join(',') === 'relay,session'
    && approval.searchParams.get('relay') === QR_RELEASE.relay
    && approval.searchParams.get('session') === created.sessionId, 'qrSessionBindingMismatch');
  let pending;
  try { pending = JSON.parse(await read(`${QR_SESSION_PATH}/status`, created.intentToken)); }
  catch { throw new ReleaseError('qrPollFailed'); }
  requireRelease(pending?.status === 'pending' && pending.login === undefined, 'qrPollNotPending');
  return { relay: approval.searchParams.get('relay'), wallet: approval.origin, pending: true };
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
function protectedResponse(candidate, pathname, env, qrRequest) {
  requireRelease(/^[a-z0-9][a-z0-9-]*\.vercel\.app$/.test(candidate.url || '') && /^\/(?:[A-Za-z0-9_./-])*$/.test(pathname), 'candidateRequestInvalid');
  const requestArgs = qrRequest
    ? ['--request', 'POST', '--header', `Origin: ${ORIGIN}`,
      ...(qrRequest.token ? ['--header', `Authorization: Bearer ${qrRequest.token}`] : [])]
    : ['--location', '--max-redirs', '3'];
  try { return execFileSync('npx', ['--yes', 'vercel@56.4.1', 'curl', `https://${candidate.url}${pathname}`, '--',
    '--fail', '--silent', '--show-error', '--max-time', '30', ...requestArgs], {
    env: childEnvironment(env, 'vercel'), encoding: 'utf8', timeout: 180000, maxBuffer: 8 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'],
  }); } catch { throw new ReleaseError('protectedCandidateFetchFailed'); }
}

async function releaseQrGame({ operation, initial, projects, candidate, id, vercel,
  body, readCandidate, fetchImpl, sleep, receipt }) {
  const hostname = new URL(ORIGIN).hostname;
  const { baseline, relay, entryScene, entryFloor, entryPlayerMale, entryPlayerFemale, entryNpcs,
    entryObjects1, entryObjects2, entryObjects3,
    entryMonsterStanding, entryMonsterWalking, entryGameplayLocale } = QR_RELEASE;
  const entryAssets = Object.entries({
    Scene: entryScene, Floor: entryFloor, PlayerMale: entryPlayerMale, PlayerFemale: entryPlayerFemale, Npcs: entryNpcs,
    Objects1: entryObjects1, Objects2: entryObjects2, Objects3: entryObjects3,
    MonsterStanding: entryMonsterStanding, MonsterWalking: entryMonsterWalking, GameplayLocale: entryGameplayLocale,
  });
  const expectedOther = { original: production(initial.original), main: production(initial.main) };
  async function mapping(allowed) {
    const live = await projects(), game = production(live.game);
    guardProjects(live, { ...expectedOther, game });
    const alias = await vercel(`/v4/aliases/${hostname}`);
    requireRelease(allowed.includes(game) && allowed.includes(alias.deploymentId)
      && alias.alias === hostname && alias.projectId === SITES.game.project
      && alias.redirect == null && alias.microfrontends == null && alias.deletedAt == null, 'qrTrafficMappingChanged');
    return { target: game, alias: alias.deploymentId };
  }
  async function awaitMapping(from, to) {
    for (let attempt = 0; attempt < 30; attempt++) {
      const current = await mapping([from, to]);
      if (current.target === to && current.alias === to) return current;
      await sleep(2000);
    }
    throw new ReleaseError('qrTrafficDidNotConverge');
  }
  const gamePath = SITES.game.project;
  if (operation === 'numeron-qr-stage') {
    receipt.trafficBefore = await mapping([baseline]);
    body.meta.numeronR16ConnectRelay = relay;
    receipt.writes.push({ operation: 'stage', site: 'game' });
    let value;
    try { value = await vercel('/v13/deployments', 'POST', body); }
    finally { receipt.trafficAfter = await mapping([baseline]); }
    requireRelease(DEPLOYMENT.test(value.id || '') && value.id !== baseline && value.projectId === gamePath,
      'stagedDeploymentIdentityInvalid');
    receipt.candidates = { game: { id: value.id, url: value.url, project: value.projectId, readyState: value.readyState } };
    return;
  }
  const reviewed = await candidate('game', id);
  requireRelease(reviewed.meta?.numeronR16ConnectRelay === relay, 'qrCandidateRelayMismatch');
  receipt.candidates = { game: { id: reviewed.id, url: reviewed.url } };
  if (operation === 'numeron-qr-rollback') {
    receipt.trafficBefore = await mapping([reviewed.id]);
    receipt.writes.push({ operation: 'rollback', site: 'game', deployment: baseline });
    await vercel(`/v1/projects/${gamePath}/rollback/${baseline}`, 'POST', {});
    receipt.trafficAfter = await awaitMapping(reviewed.id, baseline);
    return;
  }
  receipt.trafficBefore = await mapping([baseline]);
  verifyGamePages(readCandidate, reviewed, QR_RELEASE.source);
  for (const [name, entry] of entryAssets) {
    const bytes = Buffer.from(readCandidate(reviewed, entry.path), 'utf8');
    receipt['protectedEntry' + name] = verifyEntryAsset(bytes, entry, 'entry' + name + 'DigestMismatch');
  }
  receipt.protectedQr = await proveQrRelay((pathname, token) => readCandidate(reviewed, pathname, { token }));
  await mapping([baseline]);
  receipt.writes.push({ operation: 'promote', site: 'game', deployment: reviewed.id });
  try {
    await vercel(`/v10/projects/${gamePath}/promote/${reviewed.id}`, 'POST', {});
    receipt.trafficAfter = await awaitMapping(baseline, reviewed.id);
    receipt.publicQr = await proveQrRelay(async (pathname, token) => {
      const response = await fetchImpl(`${ORIGIN}${pathname}`, {
        method: 'POST', redirect: 'error', signal: AbortSignal.timeout(15000),
        headers: { Origin: ORIGIN, ...(token ? { Authorization: `Bearer ${token}` } : {}) },
      });
      return (await boundedBytes(response, 32_768)).toString('utf8');
    });
    const checkedAssets = await Promise.all(entryAssets.map(async ([name, entry]) => {
      const response = await fetchImpl(ORIGIN + entry.path, {
        redirect: 'error', signal: AbortSignal.timeout(30_000),
      });
      requireRelease(response.ok, 'entry' + name + 'Unavailable');
      const bytes = await boundedBytes(response, entry.bytes);
      return [name, verifyEntryAsset(bytes, entry, 'entry' + name + 'DigestMismatch')];
    }));
    for (const [name, entry] of checkedAssets) receipt['publicEntry' + name] = entry;
    receipt.trafficAfter = await mapping([reviewed.id]);
  } catch (error) {
    const current = await mapping([baseline, reviewed.id]);
    if (current.target === reviewed.id) {
      await vercel(`/v1/projects/${gamePath}/rollback/${baseline}`, 'POST', {});
      receipt.trafficAfter = await awaitMapping(reviewed.id, baseline);
      receipt.rollbacks.push({ site: 'game', deployment: baseline });
    }
    throw error;
  }
}

export async function runNumeronRelease({ env = process.env, fetchImpl = fetch,
  readConfig = async () => JSON.parse(await readFile(path.join(ROOT, 'mir2-web3/config/numeron-r16-web-assets.json'), 'utf8')),
  readReceipt = downloadReceipt, readCandidate = (candidate, pathname, request) => protectedResponse(candidate, pathname, env, request),
  sleep = ms => new Promise(resolve => setTimeout(resolve, ms)),
  contract = FIXED_R16, onReceipt = async () => {} } = {}) {
  const receipt = { schema: 1, kind: 'numeron-r16-release', passed: false, domainMovePending: true,
    customDomainChanged: false, oldGameProjectMutatedByHelper: false, writes: [], rollbacks: [], credentialsRecorded: false };
  try {
    const { operation, sha, qr } = validateIdentity(env);
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
    const runId = qr ? QR_RELEASE.assetRunId : env.RELEASE_ASSET_RUN_ID;
    requireRelease(/^\d+$/.test(runId || ''), 'assetRunIdRequired');
    const github = pathname => api('api.github.com', `/repos/${REPOSITORY}${pathname}`);
    const assetSha = qr ? QR_RELEASE.assetSource : sha;
    const sourceRef = qr ? QR_RELEASE.sourceRef : BRANCH;
    if (qr) {
      const { baseSource, changedFiles } = QR_RELEASE;
      const comparison = await github(`/compare/${baseSource}...${sha}`);
      const files = Array.isArray(comparison.files) ? comparison.files : [];
      const byPath = new Map(files.map(file => [file?.filename, file]));
      requireRelease(comparison.status === 'ahead' && comparison.ahead_by === 1 && comparison.behind_by === 0
        && comparison.merge_base_commit?.sha === baseSource && comparison.commits?.length === 1
        && comparison.commits[0].sha === sha && files.length === changedFiles.length && byPath.size === files.length
        && changedFiles.every(expected => {
          const actual = byPath.get(expected.filename);
          return actual?.status === expected.status && actual.sha === expected.sha;
        }),
      'qrSourceClosureMismatch');
      receipt.sourceClosure = { base: baseSource, assetSource: assetSha, ref: sourceRef, changedFiles };
    }
    const run = await github(`/actions/runs/${runId}`), jobs = await github(`/actions/runs/${runId}/jobs?per_page=100`),
      artifacts = await github(`/actions/runs/${runId}/artifacts?per_page=100`);
    requireRelease(!jobs.total_count || jobs.total_count <= 100, 'assetJobsPaginationExceeded');
    requireRelease(!artifacts.total_count || artifacts.total_count <= 100, 'assetArtifactsPaginationExceeded');
    const artifact = validateAssetRun(run, jobs.jobs || [], artifacts.artifacts || [], runId, assetSha);
    validateAssetReceipt(await readReceipt(runId, artifact.name, env), assetSha, contract);
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
      candidateGuard(key, value, snapshot, sha, config, sourceRef);
      return value;
    }
    if (qr) {
      const body = operation === 'numeron-qr-stage'
        ? deploymentBody('game', sha, link.repoId, config, env.MIR2_R16_PASSKEY_AUTH_SECRET, sourceRef)
        : undefined;
      await releaseQrGame({ operation, initial, projects, candidate, id: ids.game, vercel,
        body, readCandidate, fetchImpl, sleep, receipt });
      receipt.passed = true;
      return receipt;
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
