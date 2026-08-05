Original prompt: Continue autonomous Crystal/Mir2 1:1 parity work until the current frontend input and NPC marker issues are landed and verified.

## 2026-07-30 — Thin player client / R2 asset boundary

- Goal: separate the complete local Crystal source corpus from the player distribution, verify the real R2 boundary, remove duplicate runtime bytes, and produce a production-verifiable standalone thin client.
- Root cause: `apps/web/public` is a deterministic source/generation corpus (429,070,362 logical bytes), while `.next/cache` and `.next/dev` contribute another ~1.1 GB of compiler state. Neither is the player download.
- R2 audit: the immutable prefix contains 86,447 objects / 443,736,598 bytes, but its online release manifest contains only 188 historical entries and several current starter-map/HUD paths are absent. The build now preserves the 39,409-entry filesystem manifest instead of allowing the stale remote manifest to shrink it to 184 entries.
- Implemented: conditional Next standalone output, a 360 MiB thin-package budget/report, R2/local asset smoke, runtime same-origin fallback, local map/entity atlases, explicit tiny compatibility assets, removal of the 37 MiB legacy WebGL2 mirror, and tier-aware prewarming that no longer downloads raw map frames on packed-atlas tiers.
- Final artifact: `.mir2-thin-client` is 348,608,686 logical bytes (332.46 MiB), contains both selectable Bevy backends, and passes 13 local package checks plus 12 live R2 checks.
- Browser acceptance: `demo/demo` logged in through the real local Gateway, entered BichonProvince, rendered via WebGPU Bevy with scene interaction ready, completed cache prewarm 215/215 with 0 failures, and moved authoritatively from `(288,616)` to `(289,616)`.
- Known remote-release advisory: optional `Monster/006` metadata and the private full Crystal pack index remain unavailable in the historical R2 prefix. They are graceful optional fallbacks and do not block the verified starter gameplay path; a future R2 republish must use a new immutable version rather than overwriting the current prefix.

## 2026-07-29 — Launch Channel Pack v1

- Goal: land the first production-oriented AI distribution loop across Web/HLS, Discord Webhook, YouTube RTMPS, and a lightweight in-game AI event entry.
- Acceptance boundary: channel health must reflect real runtime evidence; missing YouTube/Discord credentials remain visibly waiting rather than reporting false readiness.
- Deferred by design: Discord Go Live, clip export, Twitch, and Bilibili remain disabled for this launch pack.
- Verification will cover the player-facing game entry, operator controls, gateway tests, production web build, and the web-game Playwright client.
- Implemented the in-process player WebSocket event entry with a 60-second freshness boundary and a dismissible “立即观战” card.
- Added launch-v1 readiness for Game, Web/HLS, Discord, and YouTube RTMPS. YouTube now requires an authenticated live encoder heartbeat; stale or failed heartbeats degrade instead of reporting false readiness.
- Added the YouTube encoder secret template, runtime heartbeat loop, acceptance probe, non-technical launch readiness UI, and Chinese production/manual acceptance guide. Discord Go Live, Clip, Twitch, and Bilibili remain explicitly deferred.
- Verified: AI distribution tests 6/6, AI Live tests 7/7, Web `typecheck`, production Next build, shell syntax, Docker Compose config, `git diff --check`, authenticated 3/4 local launch probe, and the required web-game Playwright client with no browser console errors.

2026-07-29 production AI live goal:

- New user goal: land a production-grade AI live broadcast system on top of the completed
  read-only spectator transport.
- Scope: deterministic highlight scoring, bounded AI commentary, TTS clips, AI camera target
  decisions, durable segment evidence, live/shadow/pause controls, a player-safe broadcast
  overlay, an isolated Chromium/FFmpeg encoder container, Discord highlight delivery,
  observability, tests, Chinese operations documentation, browser acceptance, commit, and push.
- Architectural decision: the Gateway only produces a sanitized broadcast program feed. The
  encoder runs as a separate disposable service, and no model, TTS, webhook, or streaming
  failure is allowed to block Zone ticks or player WebSocket sessions.
- Backend landed: deterministic highlight scoring, strict model JSON and target allowlist,
  deterministic fallback, OpenAI-compatible TTS, AI director target, JSONL segment evidence,
  restart-durable Discord retry/dead-letter state, live/shadow/pause controls, redacted status,
  audio serving, JSON metrics, and Prometheus metrics.
- Frontend and broadcast landed: responsive `/ai-live` operations console, same-origin
  server-side control proxy, WebSocket `aiLiveStatus`, clean `/spectate?aiLive=1` lower thirds,
  optional AI audio, and an isolated Chromium/PulseAudio/FFmpeg HLS/RTMP container profile.
- Verification: Gateway 400/400 library tests, AI Live 7/7 focused tests including mock
  model/TTS/Discord, Web TypeScript and production build, real spectator smoke, desktop
  operations/broadcast browser passes, 390x844 mobile pass, Compose/shell validation, and local
  H.264/AAC HLS encoding passed. Docker Desktop itself returned HTTP 500 before image build;
  third-party RTMP was not attempted because no platform ingest key was provided.

2026-07-29 production AI daily report goal:

- New user goal: land a production-grade AI daily reporting system and publish approved player-facing reports to Discord.
- Scope: real gameplay/account/infrastructure aggregation, deterministic metrics before AI prose, PostgreSQL persistence, idempotent scheduling/backfill, evidence and model audit, operator review/publish workflow, durable Discord retries/dead-letter handling, admin UI, player world-report page, metrics, tests, Chinese runbook, commit, and push.
- Architectural decision: daily metrics remain authoritative and deterministic; the model only writes bounded narrative JSON. Discord is an outbound publication channel, never the source of truth, and its webhook secret remains environment-only.
- Backend landed: migration `0008_ai_daily_reports`, complete date-window aggregation, OpenAI-compatible strict narrative adapter, deterministic fallback, immutable published reports, draft/approve/publish state machine, scheduler, public player edition, Prometheus metrics, and durable Discord retry/dead-letter state.
- Frontend landed: Admin Web `/daily-reports` and Player Web `/world-report`, both responsive and backed by the published report API.
- Real acceptance passed against an isolated Homebrew PostgreSQL cluster plus mock model and mock Discord: `ok=true`, AI request 1, Discord delivery 1, delivery `delivered`, public response redaction and Prometheus metrics verified.
- Browser acceptance passed for the Chinese Admin report page and player newspaper at 1280px with no horizontal overflow and no browser console warnings/errors. The skill-local generic Playwright client remains unavailable because its own install cannot resolve `playwright`; the in-app browser Playwright surface covered the live pages instead.

2026-07-28 production spectator goal:

- New user goal: fully land a production-verifiable spectator system instead of leaving the
  existing `Observe` packet and GM flags as protocol-only shells.
- Scope: a mutation-free spectator transport, live map/target following, director/free-camera
  controls, configurable public delay, durable recordings and timeline playback, metrics/audit
  surfaces, browser UI, automated tests, human acceptance documentation, commit, and push.
- Architectural decision: spectator sockets are separate from player `/ws` sessions. They consume
  sanitized Zone-derived frames and never own a `GatewaySession`, so movement/combat/economy
  commands cannot cross the spectator boundary.

2026-05-25 Bevy entity atlas prebuild/cache goal:

- New user goal: move the expensive Bevy entity atlas cold path out of page live packing where possible, using prebuilt atlas packs and persistent browser cache so mobile/input feel is not blocked by hundreds of image decodes and canvas packing.
- Plan: add a public manifest-driven prebuilt atlas loader, add IndexedDB persistence for built atlases, keep the existing in-memory cache as LRU, and retain live page packing only as the fallback path.

2026-05-18 character creation class-picker follow-up:

- Replaced the select-screen `NEW` shortcut's hardcoded random male Warrior creation with an original-style creation panel for name, gender, and class.
- The panel now exposes Warrior, Wizard, Taoist, Assassin, and Archer, updates the select portrait preview from the draft class/gender, localizes the Chinese labels, validates empty/duplicate/full-slot cases, and sends the selected draft through the Gateway `newCharacter` command.
- New character success now selects the newly created visible slot instead of always falling back to the first slot.
- Evidence: Browser on `127.0.0.1:13010` opened the localized create panel, created a female Archer on `demo/demo`, saw it as `QAPAPAGKA 1 弓箭手`, then cleaned the demo QA character; protocol smoke created all five classes across temporary accounts with `ok=true` in `docs/generated/player-qa/create-character-classes-20260518/class-protocol-smoke.json`.
- Verification: Web `npx tsc --noEmit --pretty false` passed in both the main checkout and the served `/private/tmp/mir2-main-human` web directory; targeted `git diff --check` passed.

2026-05-11:

- Fixed/verified the Crystal input loop follow-up for held-run plus repeated target-click movement.
- Key evidence: `docs/generated/player-qa/movement-jitter/r-input-queue-held-run-spam-click-crystal-input-final-090527.json` is green with no visual jumps, no logical rollback, no direction lag, no stale prediction, no command queue warnings, and no residual movement plan.
- Re-smoked click target, route spam obstacle, blocked target, and NPC click paths against local Web/Gateway.
- Verified quest marker placement with an isolated temporary Gateway/account-store fixture so the main `.mir2-data/accounts.json` was not modified.
- Next useful follow-up: run one manual browser feel pass on the user's current page, then continue the queued deeper skill-system and late-gameplay packet-perfect parity slices.

2026-05-11 backend continuation:

- Reconciled two 5.5 xhigh worker slices plus local skill-system work.
- Hero learned magic now gains and levels from successful keyed Hero AI casts with Crystal `MagicLeveled` / `MagicDelay` packet evidence.
- `BackStep`, `ShoulderDash`, and `FlashDash` now advance practice only on Crystal success gates instead of generic cast completion.
- Mail exact parcel claim now preflights all serialized attachments and consumes payload only after successful claim.
- Verification: `magic_packet_crystal_` 73/73, Hero AI 28/28, focused Hero progression 2/2, Mail 9+2, Simulation fmt/check, and targeted diff checks passed.

2026-05-17 login UI follow-up:

- Moved the Passkey/Wallet alternate login actions into the original login dialog's dark credential well so they no longer read as detached buttons floating under the panel.
- Synced the change into the currently served `/private/tmp/mir2-main-human` web directory and restarted the 13010 Next dev process so the updated CSS is live.
- Verification: Web `npx tsc --noEmit --pretty false` passed in both main and temp web directories; Browser screenshot inspection confirmed the buttons sit inside the dialog without overlap; Browser console error check returned `[]`.

2026-05-17 item tooltip follow-up:

- Added shared in-game item/equipment tooltips for inventory, storage, belt, and character equipment slots, showing live name, description, quantity, durability, attack, and defence fields where present.
- Synced the component/CSS changes into `/private/tmp/mir2-main-human` and restarted the 13010 Next dev process so the current browser session serves the tooltip rules.
- Verification: Web `npx tsc --noEmit --pretty false` passed in both main and temp web directories; Browser DOM inspection after `demo/demo` game entry confirmed tooltip content is mounted for visible belt items; Browser console error check returned `[]`. The in-app automation mouse move did not synthesize CSS `:hover`, so final visual hover feel still needs a human glance.

2026-05-18 playable cache metrics follow-up:

- Added first-playable cache milestones across Player Web startup, runtime decision, scene blueprint/sprites, Gateway login/select, StartGame, UserInformation, game screen readiness, and first playable frame.
- Added `npm run smoke:playable-metrics`, which drives `demo/demo` through real Gateway login and game entry, then records cold/warm first-playable and cache/prewarm metrics.
- Evidence: `docs/generated/player-qa/cache-metrics/cache-metrics-codex-playable-smoke-final.json` passed with `ok=true`, cold first playable 1503.7ms, warm first playable 1193.8ms, 511/511 prewarm ok in both passes, no prewarm failures, no critical console errors, and no non-favicon 404s.
- Verification so far: Web `npx tsc --noEmit --pretty false`, `node --check apps/web/scripts/smoke-cache-metrics.mjs`, and the live playable smoke against Web `127.0.0.1:13011` plus Gateway `127.0.0.1:7210` passed. The generic `develop-web-game` Playwright client was not runnable because the local skill script could not resolve a `playwright` package from its install path; the project-specific CDP smoke covered the real flow instead.

2026-05-18 cache storage diagnostics follow-up:

- Added CacheStorage/quota diagnostics to `window.__mir2CacheMetrics.snapshot()` and the QA cache overlay: Mir2 cache count, entry count, usage bytes, and quota bytes.
- Updated cache smoke assertions so the warm pass must prove populated Mir2 CacheStorage entries, not only resource timing cache-like hits.
- Evidence: `cache-metrics-codex-cache-storage-smoke-final.json` passed with warm 2 Mir2 caches, 510 entries, 65,338,772 usage bytes, 0 transfer bytes, and 511/511 prewarm ok. `cache-metrics-codex-playable-storage-smoke-final.json` passed with cold first playable 1659.6ms, warm first playable 2224.9ms, warm 2 Mir2 caches, 555 entries, 67,045,268 usage bytes, 0 transfer bytes, and 511/511 prewarm ok.

2026-05-18 cache maintenance/reset follow-up:

- Added QA-only `window.__mir2AssetCacheReset({ reload?: false })` to clear all Mir2 CacheStorage buckets, unregister the asset Service Worker, refresh metrics, and optionally reload.
- Added Service Worker reset/status/configured maintenance messages and stale-version cleanup reporting.
- Added `npm run smoke:cache-maintenance`, which seeds `mir2-asset-cache-static-legacy-smoke`, reloads to prove manifest-version cleanup removes it, then invokes the reset API with `reload:false`.
- Evidence: `cache-metrics-codex-cache-maintenance-smoke-final.json` passed with warm 2 caches / 510 entries / 65,335,069 usage bytes / 0 transfer bytes / 511/511 prewarm ok; maintenance seeded the legacy cache, cleaned it by version, deleted 3 active caches, unregistered 1 Service Worker scope, and ended with 0 Mir2 caches.

2026-05-18 cache persistence/budget guard follow-up:

- Added browser storage persistence diagnostics to cache metrics: `storagePersisted` and `storagePersistGranted`.
- Prewarm now requests persistent storage where supported; fresh headless Chrome can return false, so this is observability rather than a hard gate.
- The Service Worker no longer caches static/game API requests under the unversioned `bootstrap` name before it receives the versioned asset manifest, and the frontend waits for `MIR2_ASSET_CACHE_CONFIGURED` before refreshing storage metrics.
- Added smoke budget assertions to prevent accidental full-client prewarm: default limits are 1000 prewarm requests, 2500 warm CacheStorage entries, and 256 MiB warm storage usage.
- Evidence: `cache-metrics-codex-cache-budget-maintenance-smoke-final.json` passed with 511/511 prewarm ok, 118 warm CacheStorage entries, 62,272,086 usage bytes, `storagePersisted=false`, `storagePersistGranted=false`, all budget assertions true, legacy cache cleanup true, reset deleted 3 caches/unregistered 1 SW scope, and after-reset cache count 0.

2026-05-18 Bichon click-route air-wall follow-up:

- Added bounded local route search for target-click movement when the direct Crystal step is blocked by static map cells, visible live objects, or recent correction memory.
- Directional/WASD movement still uses the direct Crystal step rules; only click-to-target movement gets the obstacle route, and blocked target tiles now settle at the nearest reachable tile instead of leaving a stale pending plan.
- Synced the change into `/private/tmp/mir2-main-human`, restarted the 13010 Next dev server, and verified the current browser path around the Bichon shop/bridge area with no console warnings/errors.
- Verification: Web `npx tsc --noEmit --pretty false` passed in both main and temp web directories, targeted `git diff --check` passed, and evidence was written to `docs/generated/player-qa/airwall-route-20260518/airwall-route-summary.json` plus `airwall-route-after.png`.

2026-05-18 reconnect grace follow-up:

- Added Gateway in-memory reconnect grace retention for active WebSocket sessions, keyed by account/character, with `MIR2_GATEWAY_RECONNECT_GRACE_SECONDS` defaulting to 15 seconds and refreshing the existing route lease for the same grace window.
- Added `npm run smoke:reconnect-resume`, which enters `demo/demo`, calls `window.__mir2Stage5.closeGatewayForReconnectSmoke()`, waits for the reconnect status/overlay, then verifies the game returns to `screen=game`, `wsState=open`, `reconnectStatus=idle`, same map, and player still present.
- Evidence: `docs/generated/player-qa/reconnect/reconnect-resume-codex-reconnect-grace-smoke-final.json` passed with `ok=true` against Web `127.0.0.1:13011` and a freshly built Gateway on `127.0.0.1:7211`; Gateway logs showed session retain and restore for `demo/0`.
- Verification: Gateway reconnect store tests 2/2, reconnect helper test, production Web path safety 3/3, route-lease stale-owner regression, `cargo +1.89.0 fmt --check -p mir2-gateway`, Web `npx tsc --noEmit --pretty false`, and `node --check apps/web/scripts/smoke-reconnect-resume.mjs` passed.

2026-05-18 R2/CDN remote asset release follow-up:

- Added versioned `remoteAssets` to `/api/asset-manifest`; `NEXT_PUBLIC_MIR2_ASSET_BASE_URL` / `MIR2_ASSET_BASE_URL` and `MIR2_ASSET_OBJECT_PREFIX` support `{version}` expansion.
- Updated `mir2-asset-worker.js` so same-origin static game asset cache misses can fetch from the configured CDN/R2 base first, cache the response under the original local request key, and fall back to the app origin if the remote fetch fails.
- Added `npm run assets:remote:build`, which reads the live asset manifest, expands scene prewarm frames, stages only critical manifest-declared assets under `.mir2-remote-assets/<version>`, and writes `docs/generated/remote-assets/latest-remote-asset-release.json`.
- Added `npm run assets:r2:dry-run` and `npm run assets:r2:upload` for Wrangler R2 publishing, plus `apps/web/cloudflare/r2-cors.public.json`; the uploader now defaults to remote R2 writes and retries transient upload failures.
- Evidence: `assets:remote:build` against current-code Web `127.0.0.1:13014` with `--assetBaseUrl https://assets.example.com/mir2/v/{version}` produced version `37596e16d64fde7c`, 512 files, 64,626,176 bytes, 0 missing files, and object prefix `mir2/v/37596e16d64fde7c`. `assets:r2:dry-run` reported 513 upload objects totaling 65,000,146 bytes including `remote-asset-release.json`.
- Live R2 is now verified. Created bucket `mir2-web3-assets`, uploaded 513/513 objects remotely under `mir2/v/37596e16d64fde7c`, enabled public access at `https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev`, applied GET/HEAD CORS, and republished `remote-asset-release.json` with `assetBaseUrl=https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev/mir2/v/37596e16d64fde7c`. Public `curl` with an Origin header returned 200, immutable cache headers, and `Access-Control-Allow-Origin: *`.

2026-05-19 R2 scene sprite release closure:

- Root cause for production mixed R2 success/failure was the first 513-object release only covering critical packs and original-map scene frames; live gameplay also requested generated `/original-ui` actor, NPC, and Monster sprite frames, so the Service Worker hit R2 404s before falling back to the app origin.
- `apps/web/scripts/build-remote-asset-release.mjs` now includes the deployed scene sprite roots (`CArmour`, `CHair`, `CWeapon`, `AArmour`, `AHair`, `AWeapon`, `ARArmour`, `ARHair`, `ARWeapon`, `NPC`, and `Monster`) by default and decodes object keys so browser paths like `AWeapon/00%20L/12.png` map to the R2 key `AWeapon/00 L/12.png`.
- Added `infra/cloudflare/mir2-r2-bulk-upload` plus `MIR2_R2_UPLOAD_DRIVER=worker` in `apps/web/scripts/upload-r2-assets.mjs`, so large batches of small generated assets can upload through an authenticated R2-binding Worker instead of spawning one Wrangler process per object.
- Published the updated release under the live prefix `mir2/v/37596e16d64fde7c`: 7,319 asset files, 6,807 scene sprite files, 76,556,530 asset bytes, 0 missing files, plus `remote-asset-release.json`. Public R2 probes returned 200 with immutable cache headers for `original-ui/Monster/003/52.png`, `53.png`, `57.png`, `NPC/03/0.png`, `CArmour/00/12.png`, `AWeapon/00%20L/12.png`, `AWeapon/01%20R/95.png`, and `ARWeapon/00%20S/12.png`.
- Evidence: `node --check apps/web/scripts/build-remote-asset-release.mjs`, `node --check apps/web/scripts/upload-r2-assets.mjs`, `npx wrangler deploy --dry-run --config infra/cloudflare/mir2-r2-bulk-upload/wrangler.jsonc`, Worker-backed `assets:r2:upload` 7,320/7,320, and production `MIR2_WEB_BASE_URL=https://mir2.obelisk.build npm run smoke:playable-metrics -- --runId codex-r2-actor-sprites-prod-smoke --waitTimeoutMs 180000` passed with `ok=true`, cold first playable 4,296.3ms, warm first playable 4,049.6ms, 517/517 prewarm ok, 0 prewarm failures, no critical console errors, and no non-favicon 404s.

2026-05-19 R2 custom asset domain and edge-cache closure:

- Connected R2 bucket `mir2-web3-assets` to `assets.mir2.obelisk.build` with minimum TLS 1.2; Cloudflare reports ownership and SSL active.
- Added and deployed `infra/cloudflare/mir2-r2-asset-cache` on route `assets.mir2.obelisk.build/*`. It serves R2 objects through an R2 binding, normalizes URL-encoded object keys such as `00%20L`, returns public CORS headers, and stores immutable GET responses in Cloudflare edge cache before falling back to R2 only on miss/range requests.
- Updated `infra/cloudflare/mir2-domain-proxy` to use `https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c` as the default asset origin, updated Vercel production `NEXT_PUBLIC_MIR2_ASSET_BASE_URL` / `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL`, and deployed Vercel production `mir2-web3-bmodyplg7-obelisk-labs.vercel.app` aliased to `mir2-web3-web.vercel.app`.
- Kept `/bevy-runtime/...` same-origin instead of remote-backed R2 and added a build-version query to both the wasm-bindgen JS and WASM URLs (`NEXT_PUBLIC_MIR2_RUNTIME_VERSION=bevy-3b1c843ac5124ec1`). This prevents stale edge/browser runtime files from mixing JS and WASM from different builds.
- Republished `remote-asset-release.json` under prefix `mir2/v/37596e16d64fde7c` with `assetBaseUrl=https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c`, `objectPrefix=mir2/v/37596e16d64fde7c`, 7,319 files, and 0 missing files.
- Evidence: `npx wrangler r2 bucket domain get mir2-web3-assets --domain assets.mir2.obelisk.build` reports `ownership_status=active` and `ssl_status=active`; `npx wrangler deploy --dry-run` and live deploys passed for both Cloudflare Workers; repeated GET probes for `original-ui/Monster/003/52.png` and `original-ui/AWeapon/00%20L/12.png` returned `x-mir2-edge-cache: HIT` and `cf-cache-status: HIT`; production `/api/asset-manifest` now returns `remoteAssets.assetBaseUrl="https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c"`.
- Production smoke `MIR2_WEB_BASE_URL="https://mir2.obelisk.build/?codexBust=domain-smoke-final-..." npm run smoke:playable-metrics -- --runId codex-r2-assets-domain-prod-smoke-final --waitTimeoutMs 240000` passed with `ok=true`, cold first playable 3,563.4ms, warm first playable 3,775.9ms, 517/517 prewarm ok, 0 prewarm failures, warm transfer bytes 727,992, warm CacheStorage 596 entries, no critical console errors, and no non-favicon 404s. Report: `docs/generated/player-qa/cache-metrics/cache-metrics-codex-r2-assets-domain-prod-smoke-final.json`.

2026-05-19 player-visible resource progress and generated blend CDN follow-up:

- Added a compact production resource status strip while critical prewarm packs are still running. It aggregates logical pack progress, local cache count, transferred bytes, and failures, then disappears once the prewarm finishes cleanly. `?cacheDebug=1` still shows the fuller QA overlay.
- Increased frontend prewarm concurrency from 4 to 8 and exposed `window.__mir2AssetCache.prewarmProgress` for live browser inspection.
- Added `/generated/original-map-blend/` to the asset manifest, Service Worker static classification, Service Worker remote-backed asset mapping, remote release builder, and Cloudflare player-domain proxy. Scene prewarm now includes generated blend render paths without replacing the original map-frame limit.
- Published generated Bichon torch blend frames `2723-2732.png` plus the updated `remote-asset-release.json` to R2 under `mir2/v/37596e16d64fde7c`; the final remote release manifest reports 7,329 files, 76,612,833 bytes, and 0 missing files.
- Deployed Player Web production `mir2-web3-l6htheoka-obelisk-labs.vercel.app`, aliased to `mir2-web3-web.vercel.app`, and deployed Cloudflare Worker `mir2-web3-domain-proxy` version `4f79190e-28f9-4d78-a99e-ea031634a6b8`.
- Evidence: Web `npx tsc --noEmit --pretty false`, `node --check apps/web/public/mir2-asset-worker.js`, `node --check apps/web/scripts/build-remote-asset-release.mjs`, Cloudflare Worker dry-run/deploy, direct CDN GET for generated `2724.png` returning edge HIT on repeat, and production `npm run smoke:playable-metrics -- --runId codex-cache-progress-prod-smoke-final --waitTimeoutMs 240000` passed with `ok=true`, cold first playable 3,859.0ms, warm first playable 3,720.9ms, 527/527 prewarm ok, 0 prewarm failures, warm transfer bytes 3,600, warm CacheStorage 608 entries, no critical console errors, and no non-favicon 404s. Report: `docs/generated/player-qa/cache-metrics/cache-metrics-codex-cache-progress-prod-smoke-final.json`.
- The generic `develop-web-game` Playwright client remains unavailable in this environment because the skill-local script cannot resolve a `playwright` package; project-specific CDP smoke covered the actual production gameplay/cache flow instead.

2026-05-19 cache console logging follow-up:

- Added `?cacheLog=1` as a frontend test switch for structured console telemetry. It prints `[mir2-cache]` entries for manifest and Service Worker setup plus `[mir2-cache-progress]` entries for active pack, percent, requested/completed/ok/failed counts, resource timing totals, transfer bytes, CacheStorage entries, storage usage/quota, remote asset base, and sample failed URLs. `?cacheDebug=1` enables the same console stream with the detailed overlay.
- Updated `npm run smoke:cache-metrics` to capture cache console log/info entries into `consoleMessages` and assert `cacheConsoleLogsPresent` when the URL includes `cacheLog=1` or `cacheDebug=1`.
- Deployed Player Web production `mir2-web3-669y2nnb1-obelisk-labs.vercel.app`, aliased to `mir2-web3-web.vercel.app`.
- Evidence: Web `npx tsc --noEmit --pretty false`, `node --check apps/web/scripts/smoke-cache-metrics.mjs`, targeted `git diff --check`, production `npm run smoke:playable-metrics -- --runId codex-cache-log-prod-smoke --waitTimeoutMs 240000` passed with `ok=true`, 527/527 prewarm ok, no critical console errors, and no non-favicon 404s. Follow-up resource smoke `npm run smoke:cache-metrics -- --runId codex-cache-log-report-smoke --waitTimeoutMs 180000` passed with `cacheConsoleLogsPresent=true`, 77 captured cache console messages, 527/527 prewarm ok, warm transfer 300 bytes, warm CacheStorage 523 entries, no critical console errors, and no non-favicon 404s.

2026-05-19 Bevy runtime cache mismatch fix:

- Root cause for the visible `Runtime boot failed: WebAssembly.instantiate(): Import ... ./mir2_bevy_runtime_bg.js ... __wbg_width...` error was a stale/mixed wasm-bindgen JS/WASM runtime cache path. The runtime was being treated too much like static game media, so old browser/edge/Service Worker state could mix a JS glue file with a WASM file from another build.
- Added `lib/generated/bevy_runtime_version.json`, generated by `scripts/build-bevy-runtime.mjs`, from the runtime JS and WASM SHA-256 values. `app/page.tsx` now appends that content hash to both `/bevy-runtime/pkg/mir2_bevy_runtime.js` and `/bevy-runtime/pkg/mir2_bevy_runtime_bg.wasm`, independent of stale `NEXT_PUBLIC_MIR2_RUNTIME_VERSION` env values.
- Removed `/bevy-runtime/` from the Service Worker static asset classification, `/api/asset-manifest.staticPrefixes`, resource pack collection, and remote-release path normalization. Runtime files stay same-origin; R2/CDN and long-lived CacheStorage are only for game media and scene assets.
- Added one-shot runtime recovery: if boot fails with the wasm-bindgen import-mismatch signature, the page clears Mir2 CacheStorage/Service Worker state and reloads once with `runtimeRecovery=1`.
- Added `apps/web/.vercelignore` to keep `.next`, `node_modules`, debug assets, and R2-backed PNG/WAV media out of Vercel source uploads; this reduced production deploy upload size enough to avoid repeated Vercel file API failures.
- Deployed Player Web production `mir2-web3-84pxlq1eg-obelisk-labs.vercel.app`, aliased to `mir2-web3-web.vercel.app`.
- Evidence: Web `npx tsc --noEmit --pretty false`, `node --check apps/web/scripts/build-bevy-runtime.mjs`, `node --check apps/web/scripts/build-remote-asset-release.mjs`, targeted `git diff --check`, production bundle grep shows only `bevy-2fa72846ccbe8964` plus recovery markers, production `/api/asset-manifest` no longer lists `/bevy-runtime/` in `staticPrefixes`, and production `npm run smoke:playable-metrics -- --runId codex-runtime-fix-smoke --waitTimeoutMs 240000` passed with `ok=true`, cold first playable 10,107.8ms, warm first playable 10,804.2ms, 527/527 prewarm ok, 0 prewarm failures, warm transfer bytes 3,900, warm CacheStorage 589 entries, no critical console errors, and no non-favicon 404s. Report: `docs/generated/player-qa/cache-metrics/cache-metrics-codex-runtime-fix-smoke.json`.

2026-05-18 Vercel Player Web preview deployment:

- Deployed `apps/web` to Vercel preview with deployment-level R2 env: `NEXT_PUBLIC_MIR2_ASSET_BASE_URL=https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev/mir2/v/37596e16d64fde7c`, `MIR2_ASSET_OBJECT_PREFIX=mir2/v/37596e16d64fde7c`, and `MIR2_DISABLE_REQUEST_FILE_WRITES=1`.
- Preview is READY at `https://mir2-web3-jv7m1fbai-obelisk-labs.vercel.app`; inspector URL is `https://vercel.com/obelisk-labs/mir2-web3-web/4HgMDnBCwVDpyYHisiQquo6dx4FU`.
- Fixed the Vercel single-directory build path by allowing `scripts/vercel-build.sh` / `build-bevy-runtime.mjs` to reuse prebuilt `public/bevy-runtime/pkg` when sibling Rust runtime sources are absent from the deployment archive.
- Fixed Vercel's 250 MB unzipped Serverless Function limit by excluding static `/original-ui` and `/original-map` PNG/WAV media from `/api/original-ui-meta` and `/api/scene/crystal` output file tracing.
- Evidence: first remote build failed on missing `/vercel/game-client/runtime/Cargo.lock`; second remote build passed Next build but failed the 250 MB function limit; third remote build completed and deployed. Local verification passed `MIR2_USE_PREBUILT_BEVY_RUNTIME=1 npm run build`, `npx tsc --noEmit --pretty false`, and targeted `git diff --check`.

2026-05-18 Cloudflare player domain:

- Created Cloudflare DNS record `mir2.obelisk.build` as a proxied CNAME to `cname.vercel-dns-0.com`, then deployed Worker `mir2-web3-domain-proxy` with route `mir2.obelisk.build/*`.
- Added `infra/cloudflare/mir2-domain-proxy` as source-controlled Worker config/code. It forwards requests to `https://mir2-web3-jv7m1fbai-obelisk-labs.vercel.app`, rewrites same-origin redirects, and injects a Worker-only `VERCEL_BYPASS_SECRET` for Vercel deployment protection.
- Vercel direct domain add/alias remains blocked because `obelisk.build` is not owned by the `obelisk-labs` Vercel team, but the Cloudflare domain path is live.
- Evidence: `npx wrangler deploy --dry-run --config wrangler.jsonc` passed, deploy version `90c06892-f2ea-4fa6-b565-acf5e5ae14b2` was published, authoritative DNS returns `mir2.obelisk.build. CNAME cname.vercel-dns-0.com.`, and public `curl -I https://mir2.obelisk.build` returned `HTTP/2 200` with `content-type: text/html; charset=utf-8`.
2026-05-23 Crystal action-queue movement follow-up:

- Kafka/Redpanda-style external queues are not part of the movement fix: Walk/Run/Turn ordering is now handled by a small per-player in-memory Zone action queue so movement remains single-writer, low-latency, and deterministic.
- Replaced the shared Zone `latest_intent` movement approximation with bounded ordered `ZoneMovementAction` entries. Zone consumes ready Walk/Run/Turn actions on Crystal `ActionTime`, uses Turn 350ms and Walk/Run 600ms timing, rejects raw standstill Run with `UserLocation` correction, and keeps Crystal's failed-Walk versus failed-Run direction behavior.
- Web self movement now treats `UserLocation` as confirmation/correction for the local ActionFeed instead of starting a new animation from the server echo. Packet Walk/Run animation timing is one Crystal 600ms action, including two-tile Run.
- Evidence: `pnpm --dir apps/web exec tsc --noEmit --pretty false`, `pnpm --dir apps/web exec next build`, `cargo +1.89.0 fmt --check --package mir2-simulation --package mir2-gateway`, Simulation `shared_zone` 78/78, focused Gateway Walk+Run/Turn routing regressions, and local movement captures `crystal-action-queue-local-shiftd-20260523` plus `crystal-action-queue-local-da2-20260523`, both `ok=true` with no visual jumps, logical rollback, scene blackouts, console errors, or non-favicon 404s.
- Production closeout: remote Gateway release `20260523T071900Z-actionqueue` is live, and Player Web production deployment `dpl_HmHQ4CXfy7d895kHFMfiNLHWespN` was the action-queue verification build, with custom-domain `https://mir2.obelisk.build/health` passing.
- The first production Web pass exposed two frontend ACK/prediction issues: locally rendered self state was pruning outstanding actions before real ACKs, and a four-tile local lead plus `predictedStillAhead` treated real corrections as stale echoes. The fix now prunes outstanding movement only from real ACK/correction evidence, caps local ActionFeed lead to two tiles, and treats non-matching `UserLocation` as correction instead of a stale echo.
- Final production evidence: `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-walk-fix2-20260523T1331.json` and `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-run-fix2-20260523T1332.json` both report `ok=true`, zero visual jumps, zero logical rollback, zero scene-layer blackouts, responsive movement queue, clean settle, no critical console errors, and no non-favicon 404s. Screenshots are the adjacent `.png` files.

2026-05-25 Bevy WebGL2 packed entity-atlas renderer follow-up:

- Started the renderer migration as a concrete goal rather than a planning-only thread: visible entity body/hair/weapon sprite layers can now render through the Bevy WebGL2 canvas while React keeps the map, HUD, hit boxes, nameplates, health bars, quest markers, and gameplay routing.
- Added the Web-to-wasm atlas path. Player Web collects current visible entity frame sources, packs them into an RGBA atlas, uploads pixels through `setMir2EntityRenderAtlas`, sends layer state through `setMir2EntityRenderState`, and caches recent atlas snapshots. The runtime ingests the atlas as a Bevy `Image` and renders sprite layers with `TextureAtlas` indices; per-PNG sprite loading remains as a fallback/debug path.
- Added toggles and capture support: `?bevyEntities=1/0`, `?bevyAtlas=1/0`, localStorage overrides, Bevy entity debug state (`ready`, `enabled`, entity/layer counts, atlas mode/key/count), and movement harness options for GPU-enabled canvas capture.
- Verification: `pnpm --dir apps/web runtime:build:release`, `cargo +1.89.0 check --manifest-path apps/game-client/runtime/Cargo.toml`, `cargo +1.89.0 fmt --manifest-path apps/game-client/runtime/Cargo.toml --check`, `pnpm --dir apps/web exec tsc --noEmit --pretty false`, and `node --check apps/web/scripts/capture-web-movement-jitter.mjs` passed.
- Evidence: `docs/generated/player-qa/movement-jitter/local-bevy-atlas-chain-20260525.json` / `.png` passed locally with `ok=true`, Bevy entity renderer `ready=true`, `enabled=true`, `entityCount=19`, `layerCount=21`, `atlasMode=packed`, `atlasCount=1`, scene assets `185/185`, no critical console errors, and no non-favicon 404s.
- Caveat: this is the first local visible-entity atlas slice, not a production rollout or a full offline all-sprite atlas system. Next renderer slice is headed Chrome/live-feel verification and broader atlas cache/performance hardening.

2026-05-25 Bevy WebGL2 atlas hardening and production closeout:

- Completed the follow-up goal: real Chrome production hand-feel verification, atlas cache hardening, animation-frame source stabilization, and live deployment.
- Hardened atlas sources so visible entity sprite layers preload locomotion frames instead of only the currently displayed frame. The final production path includes standing/walking/running frames and all eight player movement directions, which keeps the atlas key stable when keyboard movement changes facing direction.
- Added renderer resilience for production cold starts: DOM entity sprites stay visible while the first atlas is warming, then are hidden once Bevy has an active packed atlas. Debug now reports current/pending/latest atlas keys, cache status, and fallback state.
- Production deployment `dpl_4PXPyp3VuAT7vHRQr4ueKBTikbtU` is live behind `https://mir2.obelisk.build`; public `/health` returned 200.
- Evidence: `docs/generated/player-qa/movement-jitter/prod-bevy-atlas-dir-20260525T043729.json` / `.png` passed with `ok=true`, `atlasMode=packed`, `atlasCurrentKey=entity-atlas-1iogxdg`, `atlasPendingKey=null`, `atlasCachedCurrent=true`, `atlasLatestCurrent=true`, `domEntityFallback=false`, 584 atlas sources, two keyboard Walk sends, two `UserLocation` ACKs, no critical console errors, and no non-favicon 404s.
- Headed Chrome evidence: live `https://mir2.obelisk.build` entered game with `demo/demo`, moved `Scout` by keyboard to `312:249`, and saved `docs/generated/player-qa/movement-jitter/headed-chrome-prod-bevy-atlas-final-20260525T0439.png`.
- Remaining performance debt: the first cold production atlas build is still heavy (`lastBuildMs=54672` for 584 sources). Correctness is closed for this slice; next optimization should reduce cold atlas cost via prebuilt/offline or tighter warmed packs.

2026-05-25 Bevy entity atlas prebuild/cache follow-up:

- Moved the entity-atlas cold path toward prebuilt/cache-first loading. Player Web now checks a persistent IndexedDB atlas cache, then a prebuilt `/bevy-entity-atlases/manifest.json` atlas pack, and only falls back to live page-side packing if neither path covers the visible source set.
- Added an in-page prebuilt-pixel cache so new visible atlas keys that are covered by the same prebuilt pack do not repeatedly decode/read back the 4096x4096 PNG.
- Added `apps/web/scripts/build-bevy-entity-atlas-pack.mjs` and `npm run assets:bevy-entity-atlas:build`. The first generated starter pack covers player/NPC plus common Bichon monster roots, emits `public/bevy-entity-atlases/starter-bichon-base.png`, and records 2,631 source rects in a 4096x4096 atlas.
- Local WebGPU evidence: `docs/generated/player-qa/movement-jitter/local-atlas-prebuilt-postcache-order-a-20260525.json` / `.png` passed with `ok=true`, `sceneInteractionReady=true`, selected/compiled backend `webgpu`, `atlasMode=packed`, 700 active atlas sources, `builds=0`, `prebuiltHits=2`, `lastSource=prebuilt`, one Walk send, one UserLocation ACK, no critical console errors, and no non-favicon 404s.
- Local WebGL2 fallback evidence: `docs/generated/player-qa/movement-jitter/local-atlas-prebuilt-webgl2-20260525.json` / `.png` passed with forced `bevyBackend=webgl2`, `builds=0`, `prebuiltHits=2`, `lastSource=prebuilt`, and `lastPrebuiltKey=starter-bichon-base`.
- Production deployment `dpl_C8sriwUxAeuCyzoY9rAnd24QTw6D` is live behind `https://mir2.obelisk.build`. Public probes passed for `/health`, `/bevy-entity-atlases/manifest.json`, and `/bevy-entity-atlases/starter-bichon-base.png`; the PNG returns `content-length: 4272109`.
- Production keyboard evidence: `docs/generated/player-qa/movement-jitter/prod-atlas-prebuilt-keyboard-final-20260525.json` / `.png` passed with `ok=true`, selected/compiled backend `webgpu`, `atlasMode=packed`, `atlasPendingKey=null`, 388 active atlas sources, `builds=0`, `prebuiltHits=1`, `lastSource=prebuilt`, one Walk send, one UserLocation ACK, no visual jumps, rollback, route spam, critical console errors, or non-favicon 404s.
- Production mobile evidence: `docs/generated/player-qa/movement-jitter/prod-atlas-prebuilt-mobile-pixelcache-20260525.json` / `.png` passed with `mobileControls=1`, phone viewport, `atlasMode=packed`, `atlasPendingKey=null`, `builds=0`, `prebuiltHits=1`, `lastPrebuiltKey=starter-bichon-base`, one mobile joystick Walk send, one UserLocation ACK, and the same clean movement assertion set.

2026-05-25 black map / transparent Bevy canvas follow-up:

- Root cause: the original map ground is still a DOM backdrop layer below the Bevy canvas, but the Bevy web surface was being composited as opaque black. Foreground map objects, NPC/player overlays, nameplates, and HUD live on higher DOM layers, so they remained visible while only the ground looked black.
- Fix: WebGPU builds now use a transparent Bevy window with `CompositeAlphaMode::PreMultiplied`; non-WebGPU builds stay `Auto`/opaque because the local forced-WebGL2 surface only advertised opaque alpha support. Player Web tracks the selected Bevy backend and, for forced-WebGL2 original-map gameplay, hides the Bevy canvas and keeps DOM entity fallback active to avoid both black-map coverage and WebGL2 alpha-mode panics.
- Verification: `pnpm --dir apps/web run runtime:build:release` produced `bevy-6732ca9f6ab18f6d`, `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed, and local captures `apps/web/docs/generated/player-qa/movement-jitter/local-transparent-canvas-webgpu-release-20260525.json` plus `apps/web/docs/generated/player-qa/movement-jitter/local-transparent-canvas-webgl2-release-20260525.json` both passed with visible ground, one movement send/ACK, no critical console errors, and no non-favicon 404s.
- Deployment: local `pnpm --dir apps/web run vercel:build:prod` reached a complete Next build and wrote `.vercel/output/static`, but the Vercel CLI stalled before `.vercel/output/config.json`; the stuck process was interrupted. Source production deploy then succeeded as `dpl_4i4fFrooS8Esuyjh1b1oSb1NCTMb`, using the uploaded prebuilt runtime packages. `https://mir2.obelisk.build/health` returned 200, runtime package probes returned 200 with `x-mir2-asset-cache: bevy-runtime`, and production captures `docs/generated/player-qa/movement-jitter/prod-transparent-canvas-webgpu-readywait-20260525.json` plus `docs/generated/player-qa/movement-jitter/prod-transparent-canvas-webgl2-readywait-20260525.json` passed with visible ground, one movement send/ACK, no critical console errors, and no non-favicon 404s.

2026-05-25 mobile/touch Bevy canvas fallback follow-up:

- User follow-up screenshot still showed the classic failure shape: entity sprites, nameplates, and lamps visible above a black ground plane. Spot-checking source PNG and atlas alpha showed transparent channels were present, so the remaining risk is still the Bevy canvas surface covering the DOM map in some browser/device path.
- Player Web now treats mobile/touch and explicit `?bevyCanvas=0` / `?bevyCanvasHidden=1` as a DOM-entity fallback path: the Bevy canvas is hidden in-game and Bevy entity rendering is disabled, while `?bevyCanvas=1` / `?bevyEntities=1` can force the experimental Bevy sprite path back on. Desktop WebGPU remains enabled by default.
- Local evidence: `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed; local mobile/touch capture `docs/generated/player-qa/movement-jitter/local-mobile-dom-fallback-canvas-hidden-fresh-20260525.json` / `.png` passed with selected/compiled backend `webgpu`, Bevy renderer `enabled=false`, `canvasHidden=true`, one Walk send, one UserLocation ACK, visible ground, no critical console errors, and no non-favicon 404s. Local desktop guard capture `docs/generated/player-qa/movement-jitter/local-desktop-webgpu-transparent-guard-20260525.json` / `.png` passed with Bevy renderer `enabled=true`, `canvasHidden=false`, prebuilt atlas hit, one Walk send, one UserLocation ACK, and visible ground.
- Production deployment `dpl_8hgZxTUoDTUokZ1tkTkpVQeU2uwf` is live behind `https://mir2.obelisk.build`. Public probes passed for `/health`, the WebGPU/WebGL2 `bevy-6732ca9f6ab18f6d` runtime JS files, and `/bevy-entity-atlases/manifest.json`.
- Production evidence: `docs/generated/player-qa/movement-jitter/prod-mobile-dom-fallback-canvas-hidden-finalready-20260525.json` / `.png` passed with mobile controls, selected/compiled backend `webgpu`, Bevy entity renderer `enabled=false`, `canvasHidden=true`, one Walk send, one UserLocation ACK, visible ground/entities, no critical console errors, and no non-favicon 404s. `docs/generated/player-qa/movement-jitter/prod-desktop-webgpu-transparent-guard-finalready-20260525.json` / `.png` passed with Bevy entity renderer `enabled=true`, `canvasHidden=false`, prebuilt atlas source, one Walk send, one UserLocation ACK, and visible ground. `docs/generated/player-qa/movement-jitter/prod-bevy-canvas-off-dom-fallback-finalready-20260525.json` / `.png` passed as the explicit `?bevyCanvas=0` escape hatch.
- QA harness note: `apps/web/scripts/capture-web-movement-jitter.mjs` now supports `--finalSceneReadyTimeoutMs` so post-movement screenshots wait for the refreshed scene asset key before capture. This avoids treating the transient resource-cache overlay frame as final visual evidence.

2026-07-28 production spectator goal:

- Added a structurally read-only `SpectatorHub` and `/spectator/ws`; spectators never receive a `GatewaySession`, and gameplay commands cannot enter the authoritative command path.
- Added server-enforced public delay, public-map allowlisting, constant-time director-token authorization, follow target, auto-director, free camera, bounded buffers, WebSocket capacity accounting, active-viewer metrics, and audit logs.
- Added sanitized hourly JSONL recording and replay. Inventory, storage, quests, skills, buffs, equipment, NPC dialog, and other private character state are stripped.
- Added a bounded all-map event timeline for spawn/despawn, movement, health, death/revive, and ground-drop changes, plus stale entity pruning for merged AOI snapshots.
- Added `/spectator/matches`, `/spectator/recordings`, `/spectator/replay`, `/spectator/metrics`, and spectator metrics in `/health`.
- Added `/spectate` Player Web entry, read-only spectator overlay, map/target selection, director controls, replay timeline/speed, event feed, reconnect, `render_game_to_text`, and `advanceTime`.
- Added backend smoke `npm run smoke:spectator`, browser CDP smoke `npm run smoke:spectator-ui`, and Chinese production/operations/manual acceptance guide `docs/SPECTATOR-MODE.zh-CN.md`.
- Local evidence before the final event-timeline pass: backend smoke verified read-only rejection, privacy redaction, one active viewer, persistence, and replay; browser smoke rendered the real Bichon scene with `Scout`, director and replay controls, no critical browser errors, and captured `apps/web/artifacts/spectator/spectator-ui.png`.
- The generic `develop-web-game` Playwright client was attempted but its skill-local script could not resolve `playwright`; the project CDP smoke exercised the actual browser path instead.

2026-07-30 distribution-channel identity goal:

- Chose a stable internal `obl_<128-bit>` Obelisk Player ID as the owner of characters. Sui Passkey is the recommended primary credential; Dubhe/Sui Wallet and CrazyGames identities are optional credentials bound to the same player. A wallet address is no longer required to be the game-account primary key.
- Added Gateway channel session exchange and guarded identity-link endpoints. Sui proofs retain the distinction between Passkey and Wallet, and the server rejects a normal wallet signature that claims to be a Passkey. CrazyGames JWTs are checked with RS256, official game ID, expiry, and a bounded public-key cache.
- Added signed HttpOnly-cookie guest identity for itch and CrazyGames guests, plus a Channel Bridge for direct/itch/CrazyGames loading, gameplay, user-token, and rewarded-ad lifecycle hooks.
- Added durable PostgreSQL identity tables, advisory-lock concurrency control, an r2d2 connection pool, raw-provider-ID hashing, and an operator-only player identity lookup. JSON and memory backends remain development fallbacks.
- Added the character-select identity panel so a channel guest can bind Sui Passkey or Dubhe/Sui Wallet without losing the existing Obelisk Player ID and characters. Passkey binding promotes the account's primary recovery provider while preserving its channel binding.
- Added an uploadable itch HTML5 launcher with a new-tab WebAuthn fallback, plus a Chinese production configuration, API, security, rollout, and manual-acceptance guide.
- Browser evidence: itch guest login remained stable across reload; CrazyGames SDK v3 automatically entered the verified channel account with no external-login controls and no console errors; a Chromium virtual platform authenticator completed a real WebAuthn/Sui Passkey registration and linked it to the existing itch Player ID.
- Data evidence: operator lookup returned `primaryProvider=suiPasskey`, `lastAuthenticatedProvider=suiPasskey`, and both `itch`/`suiPasskey` bindings for the same Player ID; `/health` returned `ok=true`, `backend=postgres`, and `durable=true`.
- Verification: Gateway full library suite `413 passed / 0 failed / 1 ignored`; focused channel suite `4 passed / 0 failed / 1 ignored`; ignored two-independent-Gateway PostgreSQL test passed separately; Web typecheck and optimized Next production build passed; the negative authentication probe returned HTTP 401 when an Ed25519 wallet signature claimed `suiPasskey`.
2026-07-30 thin player client / R2 asset boundary goal:

- New user goal: explain and eliminate the ~700 MB Mir2 client build while keeping the full local
  source corpus available for development and offline asset generation.
- Cloudflare read-only inventory verified that `mir2/v/37596e16d64fde7c/` still contains 86,447
  objects / 443,736,598 bytes (69,885 map objects and 16,551 UI objects), while the currently
  published `remote-asset-release.json` has drifted down to a 188-file runtime sample.
- Local size diagnosis: `apps/web/public` is ~563 MB of source/runtime assets; `.next` is ~877 MB
  only because it combines ~456 MB production webpack cache and ~405 MB development cache with
  the real ~15 MB production server/static output. Neither cache is a player distribution.
- Architectural decision: keep source assets in git for deterministic generation and local/offline
  development, but make production packaging a measured thin-client boundary: only the two
  same-origin Bevy runtimes plus explicit bootstrap/generated packs may ship; R2-backed original
  UI/map media must be excluded and fetched through the existing versioned Service Worker cache.

## 2026-08-04 Asset Delivery v2 implementation

- Goal: replace screen-name-only prewarming with a single lifecycle boundary that protects login
  and first playable, then make runtime/full-pack delivery content-versioned and release-driven.
- Created isolated branch `feat/asset-delivery-v2` from `origin/main@162d69edb`; the pre-existing
  generated runtime/atlas changes in `mir2-mainline-dev-environment` remain untouched.
- Added `AssetPrewarmOrchestrator`: critical/background lanes, a real first-playable gate,
  latest-screen replacement, abortable stale background fetches, deduplication, and debug state.
- Moved Crystal scene API loading and Bevy WASM boot out of the login screen. First playable is now
  emitted only on a browser animation frame after scene interaction readiness.
- Bevy module/WASM URLs now use `/bevy-runtime/v/<runtime-version>/...` with a rewrite-backed
  immutable cache contract instead of stable paths plus query strings.
- The verified production full Crystal pack is declared in `production-web-assets.json` and
  surfaced by `/api/asset-manifest`; runtime selection reads that release capability instead of a
  browser build-time feature flag.
- Added verified map-atlas release capabilities, a compact schema-v2 atlas builder, immutable
  page names, a release-manifest builder, and a remote verifier. The production atlas now contains
  13 floor libraries / 2,305 frames / 57 pages; the coordinate manifest fell from 1,857,185 to
  52,997 bytes and the largest page from 7,650,614 to 468,963 bytes.
- Uploaded the 58 immutable atlas objects (hashed manifest plus 57 pages, 17,463,398 bytes total)
  under `mir2/v/20260730-fullcrystal-f71b89aa-gzip1/`. Full SHA-256 verification passed through
  the hotlink-safe Cloudflare alias; the public R2 fallback passed all-object HEAD verification and
  manifest SHA-256 verification. The temporary upload Worker and route were removed afterward.
- Updated the read Worker so `/hotlink-ok/` and canonical requests share a single Cache API key,
  and updated the Service Worker to try browser-safe R2 origins before the referer-sensitive
  canonical origin while streaming cold responses before asynchronous CacheStorage completion.
- Removed the background request for the legacy mutable map-atlas manifest. Runtime now refuses an
  unverified/non-content-addressed atlas capability and falls back to the established DOM path.
- Fixed the playable cache harness itself: GPU acceptance no longer starts Chrome with
  `--disable-gpu`, reports Bevy/WebGL2 renderer evidence, and rejects legacy manifest/page traffic.
- Pre-ship review hardened the temporary R2 uploader path so it cannot switch origins while carrying
  credentials, and made repeated atlas builds remove only stale hashed manifests/pages. Both cases
  now have regression coverage.
- Final production build passed. With the local atlas directory deliberately removed, real Gateway
  cold/warm acceptance passed `ok=true`: first playable 15,565/15,415 ms, prewarm 146/146 with zero
  failures, verified hashed manifest/pages present, legacy manifest/pages zero, no critical console
  errors, and no non-favicon 404s. Direct final browser acceptance entered BichonProvince as Scout,
  reported `sceneInteractionReady=true`, Bevy map pipeline active with 675 packed tiles and seven
  atlas pages, rendered a visually complete map, and produced no console errors.
- Delivery boundary at commit time: Cloudflare/R2 asset infrastructure is live and the Player Web
  production build passes; PR merge and the corresponding Vercel production deployment remain the
  release gates. Real phone/network acceptance remains separate from automated desktop Chromium proof.

## 2026-08-05 Mobile startup and runtime delivery follow-up

- Reproduced the reported red `Runtime boot failed: Load failed` path: production served each Bevy
  WASM uncompressed at 27.6/29.0 MB, while a China-to-Singapore range probe sustained only about
  120 KB/s. The same cold run also spent tens of seconds awaiting Service Worker update/readiness.
- Touch-first phones/tablets now enter the already-supported DOM/WebGL2 compatibility renderer by
  default and do not download Bevy WASM. `?bevyRuntime=1` remains the explicit QA override. Runtime
  network failures degrade silently to the playable renderer and never trigger a second full backend
  download or add a red failure line to player chat.
- Service Worker registration/configuration/update are now background lifecycle work. Critical
  prewarm is exposed immediately after the manifest arrives, cache hints never await
  `navigator.serviceWorker.ready`, configuration ACK is bounded to 750 ms, and each stage has its
  own cache milestone for production diagnosis.
- Replaced the login critical-path 2,497,741-byte PNG with committed same-origin bootstrap WebP
  variants: 176,866 bytes on coarse-pointer/mobile and 305,924 bytes on desktop. A deterministic
  Sharp generator and byte/dimension budget test protect this boundary.
- Added a compressed Bevy runtime R2 release path and Cloudflare route. The two logical WASM files
  total 56.6 MB but gzip to 12.2 MB combined (5.86/6.34 MB). Runtime URLs are version-checked before
  mapping to the release object prefix, preventing stale immutable URLs from receiving current bytes.
  Runtime-only uploads explicitly suppress `remote-asset-release.json`, so an incremental runtime
  publication cannot replace the current full Crystal release manifest.
- Current automated evidence: startup budget (3/3), runtime policy (5/5), runtime byte budget,
  domain-proxy routing, asset prewarm policy (9/9), full asset-delivery regression, Web TypeScript,
  and a full production build all pass. Local touch/mobile Chromium at 844x390 fetched only the
  176,866-byte bootstrap image, made zero Bevy requests, emitted no console errors, and completed
  login-critical prewarm at about 224 ms. The cold/warm cache smoke also passed every assertion;
  production publication and real-phone verification remain separate release gates.

## 2026-08-05 First-playable asset scheduling repair

- Original prompt: `OK开始修复吧`. Goal: fix the production report that map resources remain slow
  after the Asset Delivery v2/mobile startup work, while preserving the complete Crystal/R2 corpus.
- Production diagnosis isolated two serialized blockers: the 5.2 MB encoded Bevy WASM occupied the
  slow link for about 45 seconds, then two scene requests (one incorrect default coordinate and one
  authoritative player coordinate) waited behind a cold scene function. The browser later launched
  a large unbounded tile-preload tail that competed with visible resources.
- Scene requests now require an authoritative self entity, use one shared canonical chunk/bucket
  identity in client and server, carry a schema-versioned URL, abort superseded requests, and refuse
  to apply stale responses. Production scene responses keep a five-minute browser TTL but a one-day
  shared CDN TTL plus one-week stale revalidation.
- Visible scene preloading is bounded to eight concurrent images; idle outer-ring prefetch is bounded
  to four. Once 24 visible resources are ready, queued speculative work is abandoned instead of
  flooding the connection. A timeout with zero loaded images no longer reports interaction-ready.
- Bevy boot moved behind the first browser-presented playable map frame. The DOM/WebGL2 compatibility
  renderer therefore owns the critical path; unchanged runtime bytes reuse the manifest content hash
  and no longer append the Vercel commit SHA.
- Bevy R2 objects are now immutable at
  `bevy-runtime/v/<runtime-version>/...`. The release workflow builds, dry-runs, uploads, and verifies
  the four runtime objects before deploying the Worker, and injects the same runtime version into the
  Worker config. Runtime-only publication still cannot replace `remote-asset-release.json`.
- Added a deploy warmup for three canonical Bichon first-playable chunks. Local production cold
  assembly measured 381/68/61 ms and the immediate warm pass 29/12/9 ms with complete cell/sprite
  data. The full thin production build passed at 265,239,558 bytes.
- Automated evidence passed: scene request/preloader tests, asset-delivery suite, resource loading,
  map routing, domain-proxy routing, R2 release safety, TypeScript, YAML parse, runtime R2 build/dry
  run, and optimized Next/thin-client production build. The generic web-game Playwright client
  rendered the login UI and exported `render_game_to_text`; local gameplay login is blocked only by
  the production Gateway's localhost Origin policy, so final gameplay timing remains a post-deploy
  same-origin acceptance gate.

## 2026-08-05 full-release probe hotfix

- The first production release attempt stopped before deployment because two legacy representative
  paths in the workflow were not members of the verified `20260730-fullcrystal-f71b89aa-gzip1`
  manifest. The release itself reports 46,003 files, `missingCount=0`, and a verified 5,887-file full
  Crystal pack. Replaced the stale probes with manifest-backed WemadeMir2 Objects and Tiles paths;
  no R2 object or release manifest was deleted or replaced.

## 2026-08-05 Cloudflare R2 API rate-limit hardening

- The API-backed runtime upload reached Cloudflare but four concurrent PUTs received HTTP 429.
  Runtime publication is now serialized with six attempts, and the uploader honors `Retry-After`
  before falling back to capped exponential delay. Added a deterministic 429-then-success regression
  test; immutable object keys keep partial retries idempotent and the full release manifest remains
  untouched.
