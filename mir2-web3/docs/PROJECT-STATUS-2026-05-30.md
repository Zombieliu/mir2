# Project Status Snapshot - 2026-05-30

This document captures the current state of the `mir2-web3` project and PR #1 as of 2026-05-30. It is intentionally evidence-focused: what is already done, what is verified, what is still blocking merge, and what should happen next.

No secrets, API tokens, or private credentials are recorded here.

## Executive Summary

READY TO MERGE: NO

The current Draft PR is directionally correct and the narrow asset-release gap has been fixed and pushed. The select/audio assets that were previously only covered by production smoke are now also covered by the remote asset release required list and release doctor required assets.

However, the PR cannot be merged into `main` yet. It is still Draft, GitHub reports it as conflicting against `main`, the Vercel PR check is failing, and `/api/scene/crystal` still has a production resource-missing failure. Main was not merged and was not pushed.

## Current PR And Branch

- PR: `#1` - `Draft: harden production original asset smoke coverage and rollout gates`
- URL: `https://github.com/Zombieliu/mir2/pull/1`
- Branch: `codex/web-scene-fallback-20260521`
- Base: `main`
- Readiness evaluation code head: `7f1f338bf253875434bdff41fe40eb37ba8da845`
- Documentation-only commits may follow that code head on the PR branch.
- GitHub state:
  - `isDraft: true`
  - `mergeable: CONFLICTING`
  - `mergeStateStatus: DIRTY`
  - `mergedAt: null`
- Status checks observed for the readiness evaluation:
  - `Vercel`: failure
  - `Vercel Preview Comments`: success

## Commits Added In This Readiness Pass

- `8a183b07` - Require select assets in release gates
  - Added select/audio assets to `apps/web/scripts/build-remote-asset-release.mjs`.
  - Added the same required assets to `apps/web/scripts/release-doctor.mjs`.
- `603730e2` - Fix web asset release readiness gaps
  - Added missing support files used by the web app.
  - Added missing `public/original-ui/Sound/NewChar.wav`.
  - Kept unreferenced `Title/355..359` out of required gates because the current generated/exhaustive list and code references do not require them.
- `b876f614` - Fix asset manifest Vercel tracing
  - Added targeted Vercel tracing excludes for large static asset paths.
  - Local `npm run build` passed after this change.
- `7f1f338b` - Avoid asset manifest repo tracing
  - Added targeted `turbopackIgnore` guards around `process.cwd()` / path resolution in the asset manifest route.
  - Local `npm run build` passed at this head.

## Asset Release State

Generated remote asset release:

- Version: `603730e25e9d`
- Object prefix: `mir2/v/603730e25e9d`
- Asset base URL: `https://assets.mir2.obelisk.build/mir2/v/603730e25e9d`
- File count: `15027`
- Total bytes: `135170729`
- Missing count: `0`

R2/static release checks:

- R2 upload completed through Cloudflare tooling / Worker-assisted upload path.
- Upload count observed: `15028/15028`.
- `release:doctor` with manifest and R2/static asset checks passed.
- Required assets checked by release doctor: `64/64`.

Important note: an R2 S3 access key/secret pair was not provided. The upload and validation used Cloudflare authenticated tooling and the Worker/R2 route, not an S3-specific credential path.

## Production Worker / Static Asset Route

The `mir2-web3-domain-proxy` Worker was deployed with static assets pointing at release `603730e25e9d`.

Observed behavior:

- Same-origin `/original-ui/...` assets are served from the R2-backed release.
- Critical select/audio paths now return `200`, including:
  - `/original-ui/Sound/Login2.wav`
  - `/original-ui/Sound/100.wav`
  - `/original-ui/Prguse/44.png`
  - `/original-ui/Prguse/65.png`
  - `/original-ui/Prguse/940.png`
  - `/original-ui/Title/40.png`
  - `/original-ui/Title/340.png` through `/original-ui/Title/362.png`

This improves the static asset serving path, but it does not make the whole PR merge-ready because Vercel and scene API gates are still failing.

## Verified Passing Gates

The following gates passed during this readiness pass:

- Local web build:
  - `cd mir2-web3/apps/web`
  - `npm run build`
- Required release doctor against manifest and R2/static serving:
  - `npm run release:doctor -- --manifest ../../docs/generated/remote-assets/latest-remote-asset-release.json --checkManifest true --checkR2 true --checkWorker false --checkBevyRuntime false --assetBaseUrl https://assets.mir2.obelisk.build/mir2/v/603730e25e9d`
- Production original asset smoke:
  - `npm run smoke:production-original-assets -- --targets web --webBaseUrl https://mir2.obelisk.build`
- Production Bevy runtime smoke:
  - `npm run smoke:production-bevy-runtime -- --webBaseUrl https://mir2.obelisk.build`
- One direct scene API sample succeeded:
  - `https://mir2.obelisk.build/api/scene/crystal?map=0&x=330&y=270&width=48&height=36`
  - Result: `200`, with `originalMapRegion: true`

## Current Blocking Gates

### 1. PR Is Still Draft

The PR is still marked Draft. It should not be merged while Draft.

### 2. PR Conflicts With `main`

GitHub reports:

- `mergeable: CONFLICTING`
- `mergeStateStatus: DIRTY`

A local merge attempt from `origin/main` surfaced broad conflicts across high-risk areas, including gateway/admin/simulation/web scene areas. The merge was aborted. This matches the stop condition for this readiness pass: do not force broad conflict resolution in gameplay, gateway, admin, or simulation code as part of a narrow asset-release fix.

### 3. Vercel PR Check Is Failing

The current Vercel failure is no longer the original `/api/asset-manifest` tracing failure. It has moved to `/api/scene/crystal`.

Current failure shape:

- Vercel function: `api/scene/crystal`
- Approximate bundled size: `819 MB`
- Vercel limit: `300 MB`

Observed warning points at `apps/web/lib/crystal-map-loader.ts`, where path resolution can still cause Vercel file tracing to include too much repository/static asset content.

This likely needs a focused fix around scene API tracing and/or `crystal-map-loader.ts`. That is outside the narrow asset-release gate scope unless explicitly approved, because the pasted merge-readiness instruction said not to change scene fallback or map loader logic in this pass.

### 4. Production `/api/asset-manifest` Is Still Old

The production Vercel app was not successfully redeployed at the current PR head.

Observed production manifest state:

- Endpoint: `https://mir2.obelisk.build/api/asset-manifest`
- Status: `200`
- Reported version: `faf990abb08d6f29`
- Reported object prefix: `mir2/v/37596e16d64fde7c`

This does not match the newly uploaded static release `603730e25e9d`. Static asset serving through the Worker is updated, but the Vercel app manifest remains old until a successful production deployment lands.

### 5. `/api/scene/crystal` Is Not Fully Green

Another direct scene API sample failed:

- URL: `https://mir2.obelisk.build/api/scene/crystal?map=0&x=307&y=232&width=56&height=68`
- Result: `424`
- Error: `resource_missing`
- Missing resource: `/original-map/WemadeMir2/Objects/209.png`

This is a hard merge-readiness blocker because the requested next gate included the map=0 scene smoke after asset smokes.

## Current Merge Decision

Do not merge this PR into `main` yet.

Required before merge:

- PR is moved out of Draft.
- PR is mergeable against `main`.
- Vercel PR check is green.
- Production deployment at the intended head succeeds.
- Production asset manifest matches the intended release path or the release strategy is explicitly documented.
- `smoke:production-original-assets` remains green.
- `smoke:production-bevy-runtime` remains green.
- `/api/scene/crystal` map=0 smoke is green.
- A human explicitly approves the final merge.

## Recommended Next Steps

1. Decide whether this PR is allowed to touch `apps/web/lib/crystal-map-loader.ts` / scene API tracing.
   - If yes, make the smallest tracing-only fix to stop Vercel from bundling the large original-map/public asset tree into `api/scene/crystal`.
   - If no, split the scene tracing fix into a separate PR and keep this PR Draft.

2. Fix the missing scene resource for `/original-map/WemadeMir2/Objects/209.png`.
   - Determine whether it should be generated/exported into the release, served from R2, or handled by a documented scene fallback.
   - Do not silently hide the missing asset if it is needed for Crystal parity.

3. Re-run local and production gates:

```bash
cd mir2-web3/apps/web

npm run build

npm run smoke:production-original-assets -- \
  --targets web \
  --webBaseUrl https://mir2.obelisk.build

npm run smoke:production-bevy-runtime -- \
  --webBaseUrl https://mir2.obelisk.build

npm run smoke:crystal-map-api -- \
  https://mir2.obelisk.build
```

4. Verify GitHub PR state again:

```bash
gh pr view 1 --json isDraft,mergeable,mergeStateStatus,statusCheckRollup,headRefOid,url
```

5. Only after all gates are green, request explicit approval before merging to `main`.

## Operational Notes

- CodeGraph was consulted and the index was healthy before documenting this status.
- The current worktree was clean after removing generated local artifacts.
- Temporary upload secret material used during this pass was removed from `/tmp`.
- User-provided tokens and account IDs must not be copied into docs, PR descriptions, commits, logs, or issue comments.

## Important Paths

- Web app: `mir2-web3/apps/web`
- Asset release builder: `mir2-web3/apps/web/scripts/build-remote-asset-release.mjs`
- Release doctor: `mir2-web3/apps/web/scripts/release-doctor.mjs`
- Production original asset smoke: `mir2-web3/apps/web/scripts/smoke-production-original-assets.mjs`
- Production Bevy smoke: `mir2-web3/apps/web/scripts/smoke-production-bevy-runtime.mjs`
- Asset manifest route: `mir2-web3/apps/web/app/api/asset-manifest/route.ts`
- Scene API route: `mir2-web3/apps/web/app/api/scene/crystal/route.ts`
- Crystal map loader: `mir2-web3/apps/web/lib/crystal-map-loader.ts`
- Domain proxy Worker: `mir2-web3/infra/cloudflare/mir2-domain-proxy`
