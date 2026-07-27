# Vercel Player Web Deployment Design

Last updated: 2026-05-26

Status: Vercel production deployment is live for the player-facing `apps/web`
surface behind the Cloudflare Worker domain `https://mir2.obelisk.build`,
while the game Gateway and authoritative services remain on separately managed
servers. This is an internal test deployment, not final human gameplay sign-off.

Latest preview evidence:

- 2026-05-26: Production deployment `dpl_HttHWiP21hufr1d3mm6fMsHNwcmW`
  is READY at `https://mir2-web3-n283i08jm-obelisk-labs.vercel.app`,
  aliased to `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. This deployment ships the frontend half of the
  `walk -> run -> reverse` movement closeout: Shift/run edges are preserved,
  reverse-direction follow-up input is backlogged instead of overwriting the
  current queued move, same-direction queued Walk can upgrade to Run, and the
  movement QA harness now asserts the expected `walk/run` WebSocket frames were
  actually sent. It is paired with UCloud Gateway release
  `20260526T1918CST-move-input-buffer`. Verification passed Web typecheck,
  movement harness syntax, Vercel prebuilt build/prune
  (707MB / 80,640 files -> 125MB / 312 files), deploy, public Web `/health`,
  public Gateway `/health`, direct Gateway WSS smoke
  `docs/generated/load/remote-move-input-buffer-wss-smoke-20260526.json`, and
  production headed Chrome WebGL2 captures
  `docs/generated/player-qa/movement-jitter/prod-move-input-buffer-walk-run-turn-webgl2-20260526b.json`
  plus
  `docs/generated/player-qa/movement-jitter/prod-move-input-buffer-walk-run-turn-fast-webgl2-20260526a.json`.
  The captures sent ordered `walk Right -> run Right -> walk Left`, settled at
  `332,270 Left`, had ACK latencies `251/51/50ms` and `73/54/55ms`, raw WebGL2
  rendered gameplay layers, and reported zero critical console errors and zero
  non-favicon 404s.
- 2026-05-26: Production deployment `dpl_Q1k4QFSbGigw9gJ64cfBNcAehjEQ`
  is READY at `https://mir2-web3-itfgw1ms0-obelisk-labs.vercel.app`,
  aliased to `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. The production build was produced from
  `apps/web` with
  `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=wss://165.154.65.136.sslip.io/ws`, so hosted
  custom-domain sessions use the direct Gateway WSS route instead of the
  higher-jitter custom-domain `/ws` Worker path. Bundle probing on
  `https://mir2.obelisk.build` found `165.154.65.136.sslip.io` in the shipped
  JS and no hard-coded `mir2.obelisk.build/ws`. Verification passed Web
  typecheck, movement capture script syntax, Vercel prebuilt build/prune
  (707MB / 80,633 files -> 125MB / 312 files), deploy, public Web `/health`,
  public Gateway `/health`, 1-client direct Gateway WSS smoke
  `docs/generated/load/remote-webgl2-final-wss-smoke-20260526.json`, and
  headed Chrome production evidence
  `docs/generated/player-qa/movement-jitter/prod-webgl2-raw-atlas-gameplay-focused-direct-default3-20260526.json`
  with `ok=true`, actual WebSocket `wss://165.154.65.136.sslip.io/ws`, raw
  WebGL2 `renderedLayers=21`, three Walk ACKs at `93/51/46ms`, clean settle,
  zero critical console errors, and zero non-favicon 404s.
- 2026-05-24: Production deployment `dpl_FW2JQim28WxQTXsYahXjfFzv1Z7c`
  is READY at `https://mir2-web3-hb4mdtpa3-obelisk-labs.vercel.app`,
  aliased to `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. This deployment fixes the real-Chrome held
  movement renderer runaway by indexing original-map region cells and
  memoizing viewport map sprite rebuilding to tile/scene-frame/map-region
  changes. Prebuilt output pruning reduced `.vercel/output` from
  625,175,158 bytes / 80,312 files to 43,893,139 bytes / 283 files before
  deploy. Verification passed Web typecheck, movement harness syntax check,
  production `/health`, real Chrome held-`D` movement without another
  unresponsive-page dialog, and production movement capture
  `docs/generated/player-qa/movement-jitter/prod-after-map-sprite-cache-d-hold-20260524T1433.json`
  with `ok=true` and all movement feel assertions passing.
- 2026-05-21: Production deployment `dpl_Fq8FkQb2JxjEmMAHwNXJCU4v7Xdi`
  is READY at `https://mir2-web3-ezaeeogvv-obelisk-labs.vercel.app`,
  aliased to `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. This deployment splits the production
  `original-ui` metadata reader from the local Crystal exporter:
  `/api/original-ui-meta` imports `lib/original-ui-meta-server.ts`, reads
  already deployed `meta.json` from the app/player domain or configured R2/CDN
  base, and no longer imports `lib/original-ui-export-server.ts`. Missing
  metadata returns `library_not_deployed`; request-time Crystal export must be
  done through local asset scripts or focused R2 repairs before deployment.
  Vercel production build reduced broad-pattern warnings from two to one:
  the previous `original-ui-export-server.ts` / `public/original-ui` trace is
  gone, and the remaining warning is the separate
  `crystal-map-loader.ts` / `public/original-map` path. The final prune report
  `docs/generated/remote-assets/vercel-output-prune-meta-reader-split-20260521.json`
  reduced `.vercel/output` from 427,399,093 bytes / 20,516 files to
  43,657,235 bytes / 278 files, removing 383,741,858 bytes / 20,238 files.
  Direct player-domain probes returned 200 for
  `/api/original-ui-meta?library=Items`, `/api/original-ui-meta?library=NPC/94`,
  representative R2-backed assets, retained debug samples, and same-origin Bevy
  wasm; `Map/foo` returned `unsupported_library`. Verification passed Web
  typecheck, production cache-maintenance smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-meta-reader-split-prod-20260521.json`
  with `ok=true`, 387/387 prewarm ok, warm transfer 0 bytes, reset cleanup
  returning to 0 caches, and no critical console errors or non-favicon 404s,
  plus playable production smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-meta-reader-split-playable-prod-20260521.json`
  with `ok=true`, cold/warm first playable 13745.3ms / 14118.8ms, 387/387
  prewarm ok, and no non-favicon 404s.
- 2026-05-21: Production deployment `dpl_ieQqdaZMnnZYNe4wxksuoqsj7Sgg`
  is READY at `https://mir2-web3-js3ofmmod-obelisk-labs.vercel.app`,
  aliased to `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. This deployment uses the CDN-first prebuilt
  flow: `vercel:build:prod` runs `vercel build --prod --yes` and then
  `apps/web/scripts/prune-vercel-output-assets.mjs`, removing R2-backed
  `.vercel/output/static/original-ui`,
  `.vercel/output/static/original-map`, and
  `.vercel/output/static/generated/original-map-blend` before
  `vercel:deploy:prod` uploads the archive. The final prune report
  `docs/generated/remote-assets/vercel-output-prune-resource-cdn-first-20260521.json`
  reduced `.vercel/output` from 420,957,251 bytes / 18,650 files to
  43,478,680 bytes / 278 files, removing 377,478,571 bytes / 18,372 files; the
  Vercel deploy uploaded 15.7MB. `static/debug` is intentionally retained until
  the player page no longer requests `/debug/map-samples/smtile-72.png` and
  `smtile-80.png`. Direct player-domain probes returned 200 for representative
  R2-backed `/original-ui`, `/original-map`,
  `/generated/original-map-blend`, retained debug samples, and same-origin
  Bevy wasm. Verification passed Web typecheck, output-prune syntax check,
  production cache-maintenance smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-resource-cdn-first-final-prod-20260521.json`
  with `ok=true`, 387/387 prewarm ok, warm transfer 900 bytes, reset cleanup
  returning to 0 caches, no critical console errors, and no non-favicon 404s,
  plus playable production smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-resource-cdn-first-playable-final-prod-20260521.json`
  with `ok=true`, cold/warm first playable 14212.5ms / 14163.9ms, 387/387
  prewarm ok, warm transfer 600 bytes, and no non-favicon 404s.
- 2026-05-21: Production deployment `dpl_9qZP7jXVU1Q6BzUWZVyQKKkMgiaf`
  is READY at `https://mir2-web3-aefb2e729-obelisk-labs.vercel.app`,
  aliased to `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. The production `/api/asset-manifest` reports
  version `5d1ec8e93c1caa62`, remote assets pinned to
  `https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c`, and tiered
  runtime cache budgets: `staticCriticalMaxEntries=3000`,
  `staticBackgroundMaxEntries=6000`, and `staticRuntimeMaxEntries=16000`.
  Resource packs now declare explicit cache tiers: `login`,
  `character-select`, and `hud-core` are critical, while `bichon-spawn` remains
  background with a 180 sprite-frame scene cap. Verification passed
  service-worker/script syntax checks, Web typecheck, local Next production
  build, Vercel production prebuild/deploy via `npx vercel@56.4.1`, direct
  player-domain manifest probe, and production cache-maintenance smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-resource-tier-prod-20260521.json`
  with `ok=true`, 387/387 prewarm ok, warm CacheStorage 3 caches / 383
  entries / 51.1MB, after-cleanup caches for `static-critical`,
  `static-background`, `scene`, and `api`, reset deleting 4 caches and
  unregistering 1 Service Worker scope, and no critical console errors or
  non-favicon 404s.
- 2026-05-20: Production hotfix deployment `dpl_9U4QFRQHubk8vzaKXYN7FQMWhRhp`
  is READY at `https://mir2-web3-1ywu3e52h-obelisk-labs.vercel.app`,
  aliased to `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. The browser Gateway URL resolver no longer
  falls back to `ws://127.0.0.1:7110/ws` on hosted domains when
  `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` is absent; local `localhost`/`127.0.0.1`
  still use the local default, explicit `?gatewayWs=` remains the first
  override, and hosted domains fall back to same-origin `/ws`. The asset
  service worker also handles scene-blueprint stale-while-revalidate network
  failure with a controlled `503` JSON response instead of issuing an
  unhandled second `fetch()`. Verification passed Web typecheck, service worker
  syntax check, scoped diff whitespace check, Vercel production build/deploy,
  direct `https://mir2.obelisk.build` and `/api/scene/crystal` 200 probes, and
  the no-`MIR2_GATEWAY_WS_URL` playable smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-prod-ws-fallback-20260520.json`,
  which recorded two `gatewayConnectStart` milestones using
  `wss://mir2.obelisk.build/ws` and first playable frames at 11463.3ms cold /
  5893.5ms warm. That smoke is intentionally not a full green cache acceptance
  artifact because it exposed separate R2/static asset 404s under `/original-ui`
  such as `Title/31.png`, `Prguse/1932.png`, `Monster/010/17.png`, and
  character select/equipment frames; follow-up should sync those missing R2
  assets or add a Worker fallback to the Vercel origin for static-asset 404s.
- 2026-05-19: Production deployment `dpl_4YwqgqQdhA1HQQwPhFrA1KoTCpXP` is
  READY at `https://mir2-web3-7r34j61kg-obelisk-labs.vercel.app`, aliased to
  `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. The production `/api/asset-manifest` reports
  version `ecb5ff44ad1ad66b`, generated from the verified R2 prefix plus
  `asset-cache-packs` SHA256
  `ccb99631adab3fda78d4db3029e6199cb79f0c29256662cbb33691aee016d8f0`; the
  resource packs are `login` 40 critical URLs, `character-select` 47 critical
  URLs, `hud-core` 108 critical URLs, and `bichon-spawn` background prewarm.
  Production playable cache smoke
  `MIR2_GATEWAY_WS_URL=wss://mir2.obelisk.build/ws node apps/web/scripts/smoke-cache-metrics.mjs --mode playable --baseUrl https://mir2.obelisk.build --runId prod-viewport-pruned-delay20-cache-existing-20260519-221410 --waitTimeoutMs 300000 --account CodexMoveD20213130 --password Mir2test1`
  passed with `ok=true`, cold first playable 11673.5ms, warm first playable
  13549.9ms, 387/387 prewarm ok, warm CacheStorage 437 entries / 54.5MB, no
  critical console errors, and no non-favicon 404s. Production movement
  diagnostic
  `docs/generated/player-qa/movement-jitter/prod-viewport-pruned-existing-settle9-20260519-221630.json`
  passed with `ok=true`, 124/124 scene assets loaded,
  `packetRuntimeModes={"packetRefresh":58}`, no visual jumps/rollback/route
  spam/stale prediction/queue warnings, no critical console errors, and no
  non-favicon 404s.
- 2026-05-19: Local pre-deploy production verification for the movement-feel
  prewarm optimization passed against the live Gateway. The build splits
  cache packs into critical/background phases, makes Bichon scene prewarm wait
  until after the first playable frame plus a 20s idle window, lowers the
  background Bichon frame cap to 180, includes `asset-cache-packs` in the
  `/api/asset-manifest` version input, and prunes original-map object sprites
  by rendered viewport intersection. Local production Web 13015 reported
  manifest version `782ef5c0a5b58195`, `asset-cache-packs` hash
  `ccb99631adab3fda78d4db3029e6199cb79f0c29256662cbb33691aee016d8f0`,
  and packs `login` critical 40, `character-select` critical 47, `hud-core`
  critical 108, and `bichon-spawn` background limit 180. Playable cache smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-viewport-pruned-delay20-cache-local-20260519.json`
  passed with `ok=true`, cold first playable 11976.3ms, warm first playable
  6022.1ms, 387/387 prewarm ok, warm CacheStorage 439 entries / 65.9MB, no
  critical console errors, and no non-favicon 404s. Movement diagnostic
  `docs/generated/player-qa/movement-jitter/viewport-pruned-existing-settle9-local-20260519.json`
  passed with `ok=true`, 112/112 scene assets loaded,
  `packetRuntimeModes={"packetRefresh":58}`, no visual jumps/rollback/route
  spam/stale prediction/queue warnings, no critical console errors, and no
  non-favicon 404s.
- 2026-05-19: The live R2 prefix `mir2/v/37596e16d64fde7c` now includes the
  scene actor sprite set that production gameplay requests from `/original-ui`
  after first map render. The release manifest reports 7,319 asset files,
  6,807 scene sprite files, and 0 missing files. Public R2 probes returned 200
  with immutable cache headers for the previously missing `Monster/003/52.png`,
  adjacent Monster frames, `NPC/03/0.png`, `CArmour/00/12.png`, and encoded
  weapon paths such as `AWeapon/00%20L/12.png` and `ARWeapon/00%20S/12.png`.
  Production smoke
  `MIR2_WEB_BASE_URL=https://mir2.obelisk.build npm run smoke:playable-metrics -- --runId codex-r2-actor-sprites-prod-smoke --waitTimeoutMs 180000`
  passed with `ok=true`, cold first playable 4296.3ms, warm first playable
  4049.6ms, 517/517 prewarm ok, no critical console errors, and no
  non-favicon 404s. Report:
  `docs/generated/player-qa/cache-metrics/cache-metrics-codex-r2-actor-sprites-prod-smoke.json`.
- 2026-05-18: Production deployment `dpl_ckFd5WW2xjgyECWvrj6qQahMXveU` is READY
  at `https://mir2-web3-b997bvnkz-obelisk-labs.vercel.app`, aliased to
  `https://mir2-web3-web.vercel.app`, with `https://mir2.obelisk.build` routing
  through Cloudflare Worker `mir2-web3-domain-proxy`. The build was produced
  from `apps/web` with `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=wss://mir2.obelisk.build/ws`
  and the verified R2 asset prefix
  `mir2/v/37596e16d64fde7c`; `.vercel/output/static/original-map` was removed
  from the prebuilt output because original map frames are served by R2.
- 2026-05-18: Production smoke
  `npm run smoke:playable-metrics -- --baseUrl https://mir2.obelisk.build --runId prod-mir2-obelisk-final-002458 --waitTimeoutMs 300000`
  passed with `ok=true`. Evidence:
  `docs/generated/player-qa/cache-metrics/cache-metrics-prod-mir2-obelisk-final-002458.json`
  records cold first playable 4612.5ms, warm first playable 4684.3ms, prewarm
  517/517 ok with 0 failures, no critical console errors, and no non-favicon
  404s. Direct production probes returned 200 for the previously failing
  `/api/original-ui-meta` libraries `CHair/00`, `CWeapon/00`, `Monster/010`,
  `NPC/05`, `CArmour/00`, and `Monster/012`.
- 2026-05-18: Vercel preview deployment is READY at
  `https://mir2-web3-jv7m1fbai-obelisk-labs.vercel.app` with inspector
  `https://vercel.com/obelisk-labs/mir2-web3-web/4HgMDnBCwVDpyYHisiQquo6dx4FU`.
- Deployment used the live R2 asset release through deployment-level build and
  runtime env:
  `NEXT_PUBLIC_MIR2_ASSET_BASE_URL=https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c`
  and `MIR2_ASSET_OBJECT_PREFIX=mir2/v/37596e16d64fde7c`.
- Vercel single-directory upload does not include sibling Rust runtime sources,
  so `scripts/vercel-build.sh` now falls back to the prebuilt
  `public/bevy-runtime/pkg` package when `apps/game-client/runtime` is absent.
- `next.config.ts` excludes static `/original-ui` and `/original-map` PNG/WAV
  media from API Serverless Function tracing; this fixed the Vercel 250 MB
  unzipped function limit hit by `/api/original-ui-meta` and
  `/api/scene/crystal`.
- Local verification before the successful preview: `MIR2_USE_PREBUILT_BEVY_RUNTIME=1 npm run build`,
  `npx tsc --noEmit --pretty false`, and targeted `git diff --check` passed.
- 2026-05-18: Vercel project `obelisk-labs/mir2-web3-web` now has
  production env vars for `NEXT_PUBLIC_MIR2_ASSET_BASE_URL`,
  `MIR2_ASSET_OBJECT_PREFIX`, and `MIR2_ENV`. Preview env vars still need to be
  passed on `vercel deploy` because this unconnected-Git project requires a
  branch selector for Preview env storage.
- 2026-05-18: `https://mir2.obelisk.build` is live through Cloudflare Worker
  `mir2-web3-domain-proxy`, routed at `mir2.obelisk.build/*` and forwarding to
  the current Vercel preview. Vercel domain ownership for `obelisk.build` is not
  claimed by `obelisk-labs`, so the Worker injects a Vercel automation bypass
  secret server-side; public verification returned `HTTP/2 200` and
  `content-type: text/html; charset=utf-8`.
- 2026-05-19: Production asset delivery now uses
  `https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c`. The R2 custom
  domain is active, and `infra/cloudflare/mir2-r2-asset-cache` is deployed on
  `assets.mir2.obelisk.build/*` to cache immutable R2 objects at the Cloudflare
  edge before R2 origin fetches. Production `/api/asset-manifest` confirms this
  base URL and object prefix.
- 2026-05-19: The current production deployment is
  `mir2-web3-7r34j61kg-obelisk-labs.vercel.app`, aliased to
  `mir2-web3-web.vercel.app`. The production build baked
  `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=wss://mir2.obelisk.build/ws` and
  generated Bevy runtime version `bevy-44768732d7a22abb`; `/bevy-runtime`
  remains same-origin with short cache headers so the wasm-bindgen JS/WASM pair
  is not served from an older R2 asset release.

## Decision

Deploy **Player Web** to Vercel, and keep **Gateway, persistence, queues, Admin
API, and operations surfaces** off Vercel for the first internal staging pass.

Target split:

```text
Vercel
  Player Web: apps/web
    Next.js shell
    Bevy WASM runtime
    retained small static/debug assets
    passkey login token API

Cloudflare R2/CDN
  immutable Crystal UI/map/audio/blend media

Gateway host
  mir2-gateway
    public /ws only
    optional public /health
    private /admin/*

Private data/control network
  Postgres
  Redis
  NATS
  optional Redpanda + ClickHouse
  Admin API
  Admin Web, initially Tailscale/private
```

Rationale:

- Vercel is a good fit for the browser client: HTTPS, CDN, preview deploys,
  rollbacks, and fast iteration on `apps/web`.
- Vercel Functions are not a fit for `mir2-gateway` because the Gateway is a
  long-lived WebSocket/game-state service. Vercel documents that Functions do
  not act as WebSocket servers.
- The current Player Web is not a pure static export. It has Node route
  handlers and filesystem-backed resource helpers. The first Vercel pass should
  make those route handlers explicit and keep source-resource export work out of
  request-time Vercel runtime paths.

## Vercel Project

Import the Git repository as one Vercel project for Player Web only.

Project settings:

| Setting | Value |
| --- | --- |
| Root Directory | `mir2-web3/apps/web` |
| Framework Preset | Next.js |
| Install Command | `npm ci` |
| Build Command | `npm run build` after the build script is made Linux-safe |
| Output Directory | Next.js default |
| Node.js | Match `apps/web/package.json` (`>=22`) |

The current Git top-level is the parent `mir2` repo, so the Vercel Root
Directory must include the `mir2-web3/apps/web` prefix unless the repository is
split before import.

## Required Environment Variables

Set these in Vercel Project Settings for Preview and Production.

| Variable | Scope | Notes |
| --- | --- | --- |
| `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` | browser | `wss://gateway-staging.example.com/ws`; public by design |
| `NEXT_PUBLIC_MIR2_ASSET_BASE_URL` | browser/server route | Current production pins the verified R2 release prefix: `https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c` |
| `NEXT_PUBLIC_MIR2_RUNTIME_VERSION` | browser | Legacy/manual override for `/bevy-runtime` JS/WASM cache-busting; the app currently imports `lib/generated/bevy_runtime_version.json`. Current production build version: `bevy-44768732d7a22abb` |
| `MIR2_ASSET_OBJECT_PREFIX` | server route | Current preview pins `mir2/v/37596e16d64fde7c` so manifest metadata matches the fixed R2 release |
| `MIR2_PASSKEY_AUTH_SECRET` | server | Must match the Gateway secret for passkey/wallet login tokens |
| `MIR2_ENV` or `MIR2_DEPLOYMENT_ENV` | server | Use `staging` for staging so missing auth secret fails closed |

Do not put database URLs, Redis URLs, operator tokens, or Admin API secrets in
the Player Web Vercel project unless a route handler genuinely needs them. The
player app should talk to the Gateway over WebSocket, not directly to private
control-plane services.

## Gateway Exposure

Vercel should connect to a separately hosted Gateway endpoint:

```text
https://gateway-staging.example.com/health
wss://gateway-staging.example.com/ws
```

Public reverse proxy policy:

```text
/ws      -> mir2-gateway /ws
/health  -> mir2-gateway /health
/admin/* -> 404 or private-network only
*        -> 404 unless explicitly needed
```

The Gateway host may be a small VPS, home staging machine behind Cloudflare
Tunnel, or a container platform that supports long-lived WebSockets. Admin API,
Postgres, Redis, NATS, Redpanda, and ClickHouse stay private.

## Current Blockers Before First Vercel Build

### 1. Build Script Must Become Cross-Platform

`apps/web/package.json` previously ran:

```json
"runtime:build:release": "powershell -ExecutionPolicy Bypass -File ./scripts/build-bevy-runtime.ps1 release",
"build": "npm run runtime:build:release && next build"
```

The goal is **not** to drop Windows support. The old PowerShell script was useful
for local Windows work, but it hard-coded Windows-only tool paths such as
`C:\Users\Administrator\.cargo\bin\cargo.exe` and `wasm-bindgen.exe`, so it
cannot run on Vercel Linux or on another Windows account without edits.

Use the cross-platform Bevy/WASM build path before turning on automatic
deployments:

```json
"runtime:build:release": "node ./scripts/build-bevy-runtime.mjs release",
"runtime:build:dev": "node ./scripts/build-bevy-runtime.mjs dev"
```

The Node wrapper in `apps/web/scripts/build-bevy-runtime.mjs` supports Windows,
WSL/Linux, macOS, and Vercel Linux:

- resolve `cargo` and `wasm-bindgen` from `PATH`;
- allow overrides such as `CARGO_BIN` and `WASM_BINDGEN_BIN` for unusual Windows
  installs;
- use Node `path` APIs instead of hard-coded `\` or `/` path strings;
- emit clear setup errors when `cargo` or `wasm-bindgen-cli` is missing;
- add the `wasm32-unknown-unknown` Rust target through `rustup` when available;
- fail early when the installed `wasm-bindgen` CLI does not match the resolved
  Rust dependency version from `Cargo.lock`;
- preserve the existing output path:

```text
apps/web/public/bevy-runtime/pkg/
```

`build-bevy-runtime.ps1` remains as a Windows convenience shim, but
`npm run build` uses the cross-platform script so local Windows and Vercel builds
exercise the same path.

### 2. Request-Time Asset Export Is Not Vercel-Safe

`/api/original-ui-meta` returns deployed static `meta.json` files only. Missing
Crystal sprite libraries must be generated before deployment through the asset
scripts, not exported during a request:

- Vercel function bundles should stay small.
- Vercel runtime filesystem is not a durable writable `public/` asset store.
- The full Crystal client source should not be bundled into a Function.

Staging policy:

- Pre-generate required UI/audio/map assets before deployment.
- Use `npm run assets:prepare` locally before deployment when additional
  original UI libraries are needed.
- Treat `public/original-ui` and `public/original-map` as static deploy
  artifacts.
- `/api/original-ui-meta` returns already deployed metadata from the app/player
  domain or configured R2/CDN base, and otherwise fails with a clear
  `library_not_deployed` response.
- If the static asset set grows past Vercel upload/file comfort limits, move
  large generated assets to object storage/CDN and keep only indexes in the app
  deployment.
- Implemented: the manual GitHub Actions workflow
  `Mir2 Web Assets R2 Release` can rebuild the deployable remote-asset manifest,
  dry-run the R2 upload plan, and optionally publish to R2 when the checkout has
  the generated static assets available.

Current local static footprint:

| Path | Size / count |
| --- | ---: |
| `apps/web/public` | about 435 MB |
| `apps/web/public/original-ui` | about 339 MB |
| `apps/web/public/original-map` | about 57 MB |
| `apps/web/public/bevy-runtime` | about 32 MB |
| `apps/web/public` files | about 16,650 |

This is above Vercel Hobby static upload limits and should be treated as a Pro
or artifact-slimming deployment unless the asset set is reduced.

### 3. Movement Diagnostics Writes To Repo Docs

`/api/movement-diagnostics` writes to `../../docs/generated/...`. That is a
local QA convenience. For Vercel:

- Disable it in production/staging, or
- write to an external store later, such as Vercel Blob or the Admin/Gateway
  event path.

The first Vercel staging pass should disable request-time doc writes.

### 4. Scene APIs Need A Packaged-Only Mode

`/api/scene/starter` is already packaged-data friendly. `/api/scene/crystal`
uses the Crystal map loader and may touch local source maps or generated public
map assets. For Vercel:

- Prefer packaged/generated map JSON and PNGs.
- Do not require `CRYSTAL_CLIENT_ROOT` at request time.
- Keep full-client scanning and `assets:prepare` as a pre-deploy CI/local step,
  not a Vercel Function responsibility.

## Deployment Phases

### Phase 0: Design and Safety Gates

- Keep Gateway/admin/data on the current staging server.
- Decide the public Gateway hostname, for example
  `gateway-staging.example.com`.
- Confirm the proxy exposes only `/ws` and `/health`.
- Confirm Vercel project root is `mir2-web3/apps/web`.
- Confirm Player Web static footprint fits the target Vercel plan.

### Phase 1: Make Player Web Build Cross-Platform

- Implemented: the WASM build entrypoint now uses a Node script that
  works on Windows, WSL/Linux, macOS, and Vercel Linux.
- Run from `apps/web`:

  ```bash
  npm ci
  npm run build
  ```

- Verify that `public/bevy-runtime/pkg` is present after build on Windows and
  Linux before enabling automatic Vercel deployments.
- Keep `wasm-bindgen-cli` aligned with the resolved `wasm-bindgen` crate version
  from `apps/game-client/runtime/Cargo.lock`.
- Vercel uses `apps/web/vercel.json` to run `bash ./scripts/vercel-build.sh`;
  that script installs Rust `1.89.0`, adds `wasm32-unknown-unknown`, installs the
  matching `wasm-bindgen-cli`, and then runs `npm run build` when the sibling
  runtime source is present. For a Vercel project rooted directly at `apps/web`,
  it falls back to the prebuilt `public/bevy-runtime/pkg` package and sets
  `MIR2_USE_PREBUILT_BEVY_RUNTIME=1`.

### Phase 2: Make Vercel Runtime Paths Explicit

- Implemented: `MIR2_WEB_HOSTING=vercel`, `VERCEL=1`, or `VERCEL_ENV` marks the
  Player Web request runtime as Vercel-hosted.
- Implemented: guard `/api/original-ui-meta` so Vercel never attempts on-demand Crystal
  library export.
- Implemented: guard `/api/movement-diagnostics` so staging/production does not write into
  repo docs.
- Verify `/api/passkey/login` still works with `MIR2_PASSKEY_AUTH_SECRET`.
- Verify `/api/scene/starter` and `/api/scene/crystal` work without a local
  full-client root.

### Phase 3: First Preview Deployment

- Link the Vercel project.
- Confirm the Vercel Build Command is still:

  ```bash
  bash ./scripts/vercel-build.sh
  ```

- Add Vercel env vars:

  ```bash
  NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=wss://gateway-staging.example.com/ws
  MIR2_PASSKEY_AUTH_SECRET=<same-secret-as-gateway>
  MIR2_ENV=staging
  ```

- Deploy a preview.
- For the current unconnected-Git Vercel project, preview env vars are passed on
  the deploy command with `--build-env` and `--env`; `vercel env add ... preview`
  is branch-oriented and fails because the project has no connected Git repo.
- Open the preview URL and verify:
  - login screen renders with original UI assets;
  - Bevy runtime loads;
  - WebSocket connects to the external Gateway;
  - password login / quick enter reaches character select or StartGame;
  - passkey login issues a Gateway token if enabled;
  - no non-favicon static asset 404s in the first screen.

### Phase 4: Staging Domain

- Current internal staging domain is `mir2.obelisk.build`.
- Implemented: `infra/cloudflare/mir2-domain-proxy` deploys the Cloudflare
  Worker route `mir2.obelisk.build/*` and forwards to the current Vercel preview
  while keeping Vercel SSO protection in place through a Worker-only
  `VERCEL_BYPASS_SECRET`.
- Vercel direct domain ownership is still blocked until `obelisk.build` is
  claimed in the `obelisk-labs` Vercel team, so do not delete the Worker proxy
  route unless Vercel custom-domain ownership is completed.
- Keep `gateway-staging.example.com` separate from the Vercel app.
- Run the existing browser smokes against:

  ```bash
  MIR2_GATEWAY_WS_URL=wss://gateway-staging.example.com/ws
  ```

- Record the Vercel deployment URL and Gateway commit SHA in the staging smoke
  evidence.

### Phase 5: Decide Asset Scaling Strategy

The current staging direction is R2/CDN for deployable Crystal assets and a
smaller Vercel app artifact. Keep only the asset indexes and first-screen
runtime assets in the app bundle when practical.

- `NEXT_PUBLIC_MIR2_ASSET_BASE_URL` points Web to the active R2/CDN release.
- `MIR2_ASSET_OBJECT_PREFIX` records where the current release lives in R2.
- `apps/web/public/mir2-asset-worker.js` tries the remote base URL for cacheable
  `/original-ui`, `/original-map`, and `/bevy-runtime` misses.
- `apps/web/scripts/build-remote-asset-release.mjs` stages the manifest-declared
  resource packs, scene frames, and generated scene sprite roots.
- `apps/web/scripts/upload-r2-assets.mjs` uploads the staged release to R2; for
  large small-file batches, use the authenticated R2-binding Worker in
  `infra/cloudflare/mir2-r2-bulk-upload` with `MIR2_R2_UPLOAD_DRIVER=worker`.

## Verification Checklist

Before handing a Vercel Player Web URL to testers:

- Vercel preview build passes.
- `/version` returns the deployed Git SHA. `MIR2_DEPLOY_REVISION` remains the
  explicit self-hosted override; Vercel falls back to its runtime
  `VERCEL_GIT_COMMIT_SHA` and a build-captured copy of the same value.
- Vercel runtime logs show no missing `MIR2_PASSKEY_AUTH_SECRET`.
- Browser first load returns 200.
- `/api/passkey/login` rejects malformed input and succeeds for a valid wallet
  message in staging, if wallet/passkey is enabled.
- `/api/original-ui-meta` does not perform request-time export on Vercel.
- `/api/movement-diagnostics` is disabled or externalized.
- Player Web connects to `wss://gateway-staging.example.com/ws`.
- Gateway proxy blocks `/admin/*` publicly.
- `mir2-gateway /health` reports WebSocket ready.
- A two-client shared Zone smoke passes against the hosted Player Web and
  external Gateway.

## Open Questions

- Which Vercel team/project should own `play-staging`?
- Will staging use Vercel Pro from day one? The current static footprint is too
  large for Hobby-style static upload limits.
- Should Admin Web remain private-only, or become a separate Vercel project
  protected by Access after Player Web is stable?
- Should large generated Crystal assets move to object storage before first
  tester access, or after the first Vercel preview proves the runtime shape?

## References

- Vercel Next.js deployment: https://vercel.com/docs/frameworks/full-stack/nextjs
- Vercel build configuration and Root Directory:
  https://vercel.com/docs/builds/configure-a-build
- Vercel monorepo project setup: https://vercel.com/docs/monorepos
- Vercel environment variables:
  https://vercel.com/docs/projects/environment-variables
- Vercel limits, including WebSocket and static upload limits:
  https://vercel.com/docs/limits/overview
- Vercel Function filesystem/runtime limits:
  https://vercel.com/docs/functions/limitations
