# Player QA Script

Last updated: 2026-05-18

Purpose: keep final human frontend validation focused. The project can be driven to **100% Candidate** automatically, then this script is used to decide whether the build becomes **100% Accepted**.

## Acceptance States

| State | Meaning |
| --- | --- |
| 100% Candidate | Automated checks, docs, traces, screenshots, and implementation tasks are complete against current standards. |
| 100% Accepted | Human gameplay review passes, or remaining differences are explicitly accepted. |

## Human Time Budget

Recommended final human QA budget: **35-70 hours** total.

The target is to keep routine development review small and reserve most human time for the final Candidate build.

## Evidence Gate

Before starting human acceptance for a Candidate build, the Coordinator should provide fresh evidence for:

- `npm.cmd run build`
- `npm.cmd run audit:crystal-map-coverage`
- `npm.cmd run audit:crystal-map-gameplay`
- `npm.cmd run smoke:crystal-minimap-assets`
- `npm.cmd run smoke:crystal-map-api`
- `npm.cmd run smoke:stage5-ui`
- `npm.cmd run smoke:cache-metrics`
- `npm.cmd run smoke:bevy-runtime-backends`
- `npm.cmd run smoke:cache-maintenance`
- `npm.cmd run smoke:playable-metrics`
- `npm.cmd run assets:remote:build`
- `npm.cmd run assets:r2:dry-run`
- `npm.cmd run load:gateway-ws`
- backend focused/regression commands for the latest changed systems

Existing frontend evidence sources:

- `apps/web/package.json` scripts: `build`, `audit:crystal-map-coverage`, `smoke:crystal-minimap-assets`, `smoke:crystal-map-api`, `smoke:stage5-ui`, `smoke:cache-metrics`, `smoke:bevy-runtime-backends`, `smoke:cache-maintenance`, `smoke:playable-metrics`, `assets:remote:build`, `assets:r2:dry-run`, `load:gateway-ws`
- `docs/generated/player-qa/cache-metrics/latest-cache-metrics.json`
- `docs/generated/player-qa/bevy-runtime-backends/latest-bevy-runtime-backends.json`
- `docs/generated/remote-assets/latest-remote-asset-release.json`
- `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`
- `docs/generated/load/latest-ws.json`
- `docs/generated/load/latest-tcp.json`
- `docs/generated/map/latest-crystal-map-coverage.json`
- `docs/generated/map/latest-crystal-map-gameplay.json`
- `docs/generated/map/latest-crystal-map-api.json`
- `docs/generated/assets/latest-minimap-assets.json`

Latest automated frontend evidence:

- 2026-05-19: Mobile landscape controls are now covered by live strict smoke.
  Player Web uses `nipplejs` as the touch joystick sensor and a Mir2 semantic
  adapter to send Crystal 8-way `walk` / `run` intents through the existing
  packet runtime/Zone movement path. The mobile overlay includes a left joystick,
  a right-bottom circular Run/Attack/Go/Pick/Bag/Char action wheel, belt/skill
  quick actions, and a portrait rotate prompt. Evidence:
  `pnpm --dir apps/web exec tsc --noEmit
  --pretty false`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`,
  and live mobile viewport smoke
  `mobile-controls-joystick-longhold3-20260519` passed with `ok=true`,
  `strictStatus="settled"`, no visual jumps, no logical rollback, no route spam,
  no stale prediction, no command queue warnings, no console errors, and no
  non-favicon 404s. Report/screenshot:
  `docs/generated/player-qa/movement-jitter/mobile-controls-joystick-longhold3-20260519.json`
  and `docs/generated/player-qa/movement-jitter/mobile-controls-joystick-longhold3-20260519.png`.
  The circular right-bottom wheel refinement was additionally verified by
  `mobile-controls-wheel-short-20260519` with `ok=true`,
  `strictStatus="settled"`, no visual jumps, no logical rollback, no stale
  prediction, no command queue warnings, no console errors, and no non-favicon
  404s. Report/screenshot:
  `docs/generated/player-qa/movement-jitter/mobile-controls-wheel-short-20260519.json`
  and `docs/generated/player-qa/movement-jitter/mobile-controls-wheel-short-20260519.png`.
- 2026-05-19: Production R2 asset delivery is on the custom Cloudflare domain
  `https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c` instead of the
  raw `r2.dev` public URL. `infra/cloudflare/mir2-r2-asset-cache` is deployed on
  `assets.mir2.obelisk.build/*` with an R2 binding and edge Cache API; repeated
  GET probes for gameplay sprite frames return `x-mir2-edge-cache: HIT` and
  `cf-cache-status: HIT`. Production `/api/asset-manifest` reports
  `remoteAssets.assetBaseUrl="https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c"`
  and `remoteAssets.objectPrefix="mir2/v/37596e16d64fde7c"`. `/bevy-runtime`
  remains same-origin with a build-versioned JS/WASM query so R2 runtime copies
  cannot mismatch the current Vercel build. Evidence:
  `MIR2_WEB_BASE_URL=https://mir2.obelisk.build/?codexBust=domain-smoke-final-... npm run smoke:playable-metrics -- --runId codex-r2-assets-domain-prod-smoke-final --waitTimeoutMs 240000`
  passed with `ok=true`, cold first playable 3563.4ms, warm first playable
  3775.9ms, 517/517 prewarm ok, warm transfer bytes 727,992, no critical
  console errors, and no non-favicon 404s.
- 2026-05-18: Remote R2/CDN asset release chain is implemented and live R2 verified. `/api/asset-manifest` now exposes versioned `remoteAssets`, the Service Worker can fetch static game assets from a configured CDN base on cache miss while storing them under the original same-origin CacheStorage key, and `assets:remote:build` stages the manifest-declared critical packs plus Bichon scene frames for R2. Evidence: `npm run assets:remote:build -- --baseUrl http://127.0.0.1:13014 --assetBaseUrl https://assets.example.com/mir2/v/{version} --runId codex-r2-release-smoke` wrote `docs/generated/remote-assets/codex-r2-release-smoke/remote-asset-release.json` and `latest-remote-asset-release.json` with `stats.fileCount=512`, `stats.totalBytes=64626176`, `stats.missingCount=0`, and object prefix `mir2/v/37596e16d64fde7c`. Live upload then created/used bucket `mir2-web3-assets`, uploaded 513/513 objects remotely totaling 65,000,146 bytes, enabled public r2.dev access at `https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev`, applied GET/HEAD CORS, and republished the release manifest with CDN base `https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev/mir2/v/37596e16d64fde7c`. Public CORS probes for `original-ui/Prguse/4.png` returned 200 with `Access-Control-Allow-Origin: *`, and the public release manifest reports `stats.fileCount=512`, `stats.missingCount=0`.
- 2026-05-18: Game cache, cache maintenance, persistence diagnostics, budget guardrails, and first-playable evidence passed. `smoke:cache-metrics` validates cold/warm resource caching, critical prewarm, scene cache hits, populated Mir2 CacheStorage entries, storage usage/quota diagnostics, and no critical browser errors. `smoke:cache-maintenance` seeds a legacy Mir2 CacheStorage bucket, verifies manifest-version cleanup removes it, then calls the QA reset API to clear active caches and unregister the Service Worker. Latest maintenance evidence at `docs/generated/player-qa/cache-metrics/cache-metrics-codex-cache-budget-maintenance-smoke-final.json` records `ok=true`, 511/511 prewarm ok, warm 118 CacheStorage entries, 62272086 storage usage bytes, storage persistence requested but not granted in fresh headless Chrome, all default budget assertions true (`<=1000` prewarm requests, `<=2500` entries, `<=256MiB` usage), legacy cleanup true, reset API available true, 3 caches deleted, 1 SW scope unregistered, and `afterReset.cacheNames=[]`. `smoke:playable-metrics` drives `demo/demo` through Gateway login, character select, `StartGame`, `UserInformation`, Bichon scene readiness, and first playable frame before repeating a warm pass in the same Chrome profile. Latest playable storage evidence at `docs/generated/player-qa/cache-metrics/cache-metrics-codex-playable-storage-smoke-final.json` records `ok=true`, cold first playable 1659.6ms, warm first playable 2224.9ms, 3/3 scene hits in both passes, 511/511 prewarm ok in both passes, warm CacheStorage 2 caches / 555 entries / 67045268 usage bytes, no prewarm failures, no critical console errors, and no non-favicon 404s.
- 2026-05-16: All-map resource/gameplay evidence passed on the full Crystal client root. `audit:crystal-map-coverage` now records 463/463 maps present/parseable, unsupported map types 0, parse errors 0, missing minimap indices `[]`, missing sampled map libraries 0, `visualFallbackRisk.mapCount=0`, and Crystal empty/out-of-range source frame references tracked separately as no-draw behavior. `audit:crystal-map-gameplay` records 1999 movement rows checked with 1906 direct transfers, 93 Crystal-ignored/deferred/special rows, movement failures 0, 6341 respawns with 6293 walkable-candidate rows and 48 Crystal-inert no-candidate warnings, respawn failures 0, 375 NPC rows with scripts found, 7 empty placeholder warnings, unimplemented NPC commands 0, and static map semantic failures 0 across safe zones, safe-zone spell flags, doors, cell lights, fishing cells, drop rules, and light/feature flags. Web `npx tsc --noEmit`, Simulation fmt check, focused `crystal_manifest_movements` 2/2, and focused `spread_slots` 2/2 also passed. Human visual/feel acceptance is still the final gate, but the prior automated map-source blocker is closed.
- 2026-04-29 R303: All-map frontend source-resource audit was added and passed via `npm.cmd run audit:crystal-map-coverage --prefix apps\web`, with evidence archived at `docs/generated/map/r303-crystal-map-coverage.json`. This was the first all-map source baseline; its then-open source-frame/minimap warnings are superseded by the 2026-05-16 full-client map coverage and gameplay audits above.
- 2026-04-28 R302: Windows original-client comparison evidence is archived at `docs/generated/player-qa/r302-original-client/summary.json`. Original Crystal `Server.exe` listened on `127.0.0.1:7000`, visible `Client.exe` reached select/game with retained character `R302HeroB`, and screenshots were captured for login rejection, select, game welcome, and unobstructed game HUD. Matching web evidence was refreshed through Stage 5 UI smoke at `http://127.0.0.1:3002` with 88 screenshots and 0 critical console errors, archived as `docs/generated/player-qa/r302-original-client/web-stage5-ui-smoke-manifest.json`. This is visual-reference evidence only; it does not close whole-project 100% Accepted because the comparison is not yet a deterministic same-scene human visual/feel pass.
- 2026-04-28 R301: Windows final automated Candidate acceptance pack passed and is summarized in `docs/generated/player-qa/r301-summary.json`. Web `tsc --noEmit` and `npm.cmd run build` passed; map API smoke wrote `docs/generated/map/r301-crystal-map-api.json` with 18/18 requests and 0 failures; minimap smoke wrote `docs/generated/assets/r301-minimap-assets.json` with 0 failures and a historical preview-index warning later closed by the 2026-05-16 full-client map audit; WS load wrote `docs/generated/load/r301-ws.json` with 64/64 ready, 0 errors, and keepalive p95 637 ms; Stage 5 UI smoke wrote `docs/generated/player-qa/r301/stage5-ui-smoke-manifest.json` with 88 screenshots, 0 critical console errors, and 32 compact text nodes checked without overflow. Rust verification passed `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674. Temporary gateway/web services were stopped and ports 7000/7110/3002 verified closed. Human Crystal visual/feel acceptance remains required for whole-project 100% Accepted.
- 2026-04-28 R300: backend/server tracked-slice packet parity is accepted under the explicit stable-diff policy. R298 supplies the 9/9 stable-clean live Crystal matrix, R299 identifies strict exact dirtiness as Crystal dynamic state, and R300 records/enforces the stable-diff acceptance mode in `docs/PACKET-PARITY-ACCEPTANCE.md` and `docs/generated/packet-traces/r300-stable-acceptance.json`. This does not replace the human Crystal visual/feel pass required for whole-project 100% Accepted.
- 2026-04-28 R297: Windows automated player QA refresh passed with `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug`. Web `npm.cmd run build`, `tsc --noEmit`, map API smoke 18/18, minimap smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 full-client map audit, WS load 64/64 ready with 0 errors and keepalive p95 632 ms, and Stage 5 UI smoke 88 screenshots with 0 critical console errors passed. This round also exported missing original scene sprite libs (`NPC/07`, `NPC/08`, `NPC/16`, `NPC/27`, `NPC/45`, `NPC/52`, `NPC/83`, `Monster/000`, `Monster/139`), fixed map-transfer minimap state from gateway `MapInformation`, and hardened concurrent account-store file saves for Windows load. Rust validation passed `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `fmt --check`, and `git diff --check`. Human Crystal visual/feel acceptance remains required for 100% Accepted.
- 2026-04-26 R224: Local packet trace matrix evidence restored. `packet_trace --list-flows` works, `mir2-gateway` passes 53/53 including packet trace bin tests 6/6, and require-local `packet_trace --matrix` wrote 9 TCP-traceable artifacts under `docs/generated/packet-traces/r224-matrix` with `localOk=true`. Live Crystal trace comparison was later refreshed by R298 and accepted under R300 stable-diff policy.
- 2026-04-26 R223: **100% Candidate** automated gate passed. Stage 5 UI smoke captured 88 screenshots with advanced Stage 5 systems state and compact Mail/Report bounds. Direct `next build`, `tsc --noEmit`, map API smoke 18/18, minimap asset smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64, `mir2-game-data` 22/22, `mir2-gateway` 47/47, full `mir2-simulation` 664/664, `fmt --check`, and `diff --check` passed. Human Crystal visual/feel acceptance remains required for 100% Accepted.
- 2026-04-26 R184: direct `next build`, Crystal minimap smoke, Crystal map API smoke, Stage 5 UI smoke with 10 screenshots, gateway health on `127.0.0.1:7110`, and websocket load 64/64 ready passed. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R185: Stage 5 UI smoke passed with 11 screenshots, named desktop 1024x768 and compact 820x640 viewport metadata, compact layout bounds assertions, and `stage5-compact-game.png`. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R186: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with compact visible-text overflow checks for 33 core text nodes. The compact minimap title/Safe Zone header is fixed. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R187: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 14 screenshots and minimap collapse, BigMap re-expand, and Mail open flow evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R188: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 17 screenshots and belt horizontal, vertical, rotate-back, and close evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R189: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 18 screenshots and belt hotkey `1` Red Potion use evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R190: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 21 screenshots and inventory bag1/bag2/quest tab evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R191: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 25 screenshots and character char/stats1/stats2/spells tab evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R192: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 27 screenshots and storage page1/page2-locked/page1 evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R193: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 31 screenshots and chat Shout filter, All restore, Settings, collapse/restore, and Report evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R194: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 35 screenshots and system menu open plus Character/Inventory/Quest action evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R195: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 36 screenshots and expanded storage rent/unlock evidence from storage page 2. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R196: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 37 screenshots and inventory Red Potion use evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R197: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 38 screenshots and inventory Dagger equip evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R198: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 40 screenshots and HUD Skill/Option character-panel routing evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R199: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 42 screenshots and inventory Drop Gold evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R200: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 43 screenshots and inventory Wooden Sword move evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R201: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 45 screenshots and inventory Red Potion Split Item evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R202: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 47 screenshots and inventory Blue Potion item-drop evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R203: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 48 screenshots and Character Dagger remove evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R204: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 49 screenshots and belt mouse-use Red Potion evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R205: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 51 screenshots and Sell Item no-service preservation evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R206: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 54 screenshots and Store Item no-service preservation evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R207: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 57 screenshots and Take Back no-service preservation evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R208: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 58 screenshots and Set Storage Password panel evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R209: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 60 screenshots and Set Storage Password mismatch/no-service submit evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R210-R218: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 71 screenshots and Mail/Report/NPC panel state, broad Stage 5 systems state, guild/group chat filters, Character repair/special-repair, ground item/gold pickup, combat target state, system-menu QA/transfer routes, Battle Focus spell casting, and compact inventory panel bounds evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R219-R222: web `tsc --noEmit`, direct `next build`, Stage 5 UI smoke passed with 85 screenshots, map API smoke archived 18/18 successful requests, minimap asset smoke archived 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, and WS load refreshed at 64/64 ready with 0 errors. Login/select lifecycle, confirmed character delete/recreate, compact multi-panel bounds, compact system-menu overflow fix, and NPC dialog link-capable rendering are covered. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R223: Stage 5 UI smoke passed with 88 screenshots, adding advanced systems state and compact Mail/Report bounds. This is the automated Candidate gate; human Crystal visual/feel acceptance remains required.

## Phase 1: Smoke Acceptance

Estimated human time: 2-4 hours.

Run after major backend/frontend milestones.

- Login with a fresh account.
- Create/select a character.
- Enter game and verify the first viewport looks coherent.
- Walk and run in four directions.
- Open/close inventory, character, belt, NPC dialog, and storage/shop panels where available.
- Fight a representative starter monster.
- Pick up gold and item drops.
- Use a potion from inventory and belt.
- Log out and reconnect.
- Confirm the first game viewport visually reads as a Crystal-like client rather than a generic web dashboard.

Pass criteria:

- No crash or broken panel.
- No unreadable or overlapping critical text.
- Core controls respond without obvious delay or wrong target behavior.

## Phase 2: System Matrix Acceptance

Estimated human time: 12-24 hours.

Run near 85-92% project completion.

Panel matrix:

- HUD: HP/MP bars, experience, gold/credit, target state, combat feedback.
- Chat: filtering, scroll, input, system messages, size/settings/report entry points.
- Belt: slots 1-6, rotation where available, hotkey item use, empty/full slot states.
- Minimap: collapse/expand, mail/map buttons, safe-zone/map readability.
- Inventory: bag1/bag2/quest tabs, item use/drop/equip/remove/move/merge/split/sell/drop gold/store/take back.
- Character: character/stats/spells tabs, equipment slots, durability display, repair/special repair entry points.
- NPC: dialog links, input submission, branch flow, buy/sell/repair/storage/craft/refine surfaces.
- Storage: unlock/set/change/remove password flows, expanded storage confirmation where available.
- Quest/mail/report/system menus: open/close, readable state, no critical overlap.
- Scene: target selection, approach/primary action, ground drop pickup, map transfer, logout/reconnect.

Backend-facing checks:

- movement and map transfer
- PvE melee/ranged attacks
- death/revive where available
- harvest monsters
- drop ownership and pickup
- item use/drop/split/merge/sell/buy/repair
- NPC dialog branches and input pages
- storage, shop, craft, repair pages
- save/reconnect persistence

Frontend-facing checks:

- login/select/game layout
- inventory/equipment/belt drag and click behavior
- tooltips and item metadata
- NPC link selection and input flow
- combat target feedback and HP/MP display
- map/minimap readability
- responsive layout at accepted desktop/mobile sizes

Pass criteria:

- Representative flows match Crystal behavior closely enough for Candidate status.
- Any accepted visual/feel differences are recorded in `docs/FRONTEND-1TO1-GAPS.md` or this script.

## Phase 3: Crystal Comparison Acceptance

Estimated human time: 8-16 hours.

Run near 92-97% project completion.

- Compare screenshots for login/select/game panels against Crystal references.
- Compare packet trace reports for representative flows when a live Crystal endpoint is configured.
- Play the same route in Crystal and `mir2-web3`:
  - start game
  - move to a nearby combat area
  - kill and loot monsters
  - use consumables
  - interact with NPC/shop/storage
  - transfer maps
  - reconnect
- Compare the panel matrix against Crystal screenshots or direct Crystal play for every implemented panel.

Pass criteria:

- No high-impact packet-visible mismatch remains untriaged.
- No major visual/layout mismatch blocks normal play.

## Phase 4: Final Candidate Acceptance

Estimated human time: 10-20 hours.

Run only after the Coordinator marks **100% Candidate**.

- Complete a 2-4 hour continuous play session.
- Complete one fresh-account route and one existing-account reconnect route.
- Visit representative maps from the current accepted map list.
- Exercise representative monsters, items, NPCs, shop/storage, and map transfer.
- Review the final known-gap list.
- Confirm frontend gaps in `docs/FRONTEND-1TO1-GAPS.md` are fixed, accepted, or explicitly deferred.

Pass criteria:

- No blocker or high-severity issue remains.
- Medium issues are either fixed or explicitly accepted.
- The user confirms `100% Accepted`.

## Reporting Format

For each issue, record:

```text
Area:
Route/step:
Expected Crystal behavior:
Actual mir2-web3 behavior:
Screenshot/trace:
Severity: blocker | high | medium | low
Decision: fix | accept | defer
```
