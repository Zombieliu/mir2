# Player QA Script

Last updated: 2026-04-29

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
- `npm.cmd run smoke:crystal-minimap-assets`
- `npm.cmd run smoke:crystal-map-api`
- `npm.cmd run smoke:stage5-ui`
- `npm.cmd run load:gateway-ws`
- backend focused/regression commands for the latest changed systems

Existing frontend evidence sources:

- `apps/web/package.json` scripts: `build`, `audit:crystal-map-coverage`, `smoke:crystal-minimap-assets`, `smoke:crystal-map-api`, `smoke:stage5-ui`, `load:gateway-ws`
- `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`
- `docs/generated/load/latest-ws.json`
- `docs/generated/load/latest-tcp.json`
- `docs/generated/map/latest-crystal-map-coverage.json`
- `docs/generated/map/latest-crystal-map-api.json`
- `docs/generated/assets/latest-minimap-assets.json`

Latest automated frontend evidence:

- 2026-04-29 R303: All-map frontend source-resource audit was added and passed via `npm.cmd run audit:crystal-map-coverage --prefix apps\web`, with evidence archived at `docs/generated/map/r303-crystal-map-coverage.json`. The audit confirms 463/463 Crystal manifest maps have local map files, 0 unsupported map types, 0 parse errors, 463/463 sampled viewports with source frames, and no missing sampled map libraries. It also records remaining map acceptance risks: 11 sampled maps have out-of-range frame-reference risk, minimap 450/451 are still missing for `DogYoArena2` / `DogYoHyun`, and full-map visual 1:1 still needs comparison/human acceptance.
- 2026-04-28 R302: Windows original-client comparison evidence is archived at `docs/generated/player-qa/r302-original-client/summary.json`. Original Crystal `Server.exe` listened on `127.0.0.1:7000`, visible `Client.exe` reached select/game with retained character `R302HeroB`, and screenshots were captured for login rejection, select, game welcome, and unobstructed game HUD. Matching web evidence was refreshed through Stage 5 UI smoke at `http://127.0.0.1:3002` with 88 screenshots and 0 critical console errors, archived as `docs/generated/player-qa/r302-original-client/web-stage5-ui-smoke-manifest.json`. This is visual-reference evidence only; it does not close whole-project 100% Accepted because the comparison is not yet a deterministic same-scene human visual/feel pass.
- 2026-04-28 R301: Windows final automated Candidate acceptance pack passed and is summarized in `docs/generated/player-qa/r301-summary.json`. Web `tsc --noEmit` and `npm.cmd run build` passed; map API smoke wrote `docs/generated/map/r301-crystal-map-api.json` with 18/18 requests and 0 failures; minimap smoke wrote `docs/generated/assets/r301-minimap-assets.json` with 0 failures and the known 450/451 warning; WS load wrote `docs/generated/load/r301-ws.json` with 64/64 ready, 0 errors, and keepalive p95 637 ms; Stage 5 UI smoke wrote `docs/generated/player-qa/r301/stage5-ui-smoke-manifest.json` with 88 screenshots, 0 critical console errors, and 32 compact text nodes checked without overflow. Rust verification passed `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674. Temporary gateway/web services were stopped and ports 7000/7110/3002 verified closed. Human Crystal visual/feel acceptance remains required for whole-project 100% Accepted.
- 2026-04-28 R300: backend/server tracked-slice packet parity is accepted under the explicit stable-diff policy. R298 supplies the 9/9 stable-clean live Crystal matrix, R299 identifies strict exact dirtiness as Crystal dynamic state, and R300 records/enforces the stable-diff acceptance mode in `docs/PACKET-PARITY-ACCEPTANCE.md` and `docs/generated/packet-traces/r300-stable-acceptance.json`. This does not replace the human Crystal visual/feel pass required for whole-project 100% Accepted.
- 2026-04-28 R297: Windows automated player QA refresh passed with `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug`. Web `npm.cmd run build`, `tsc --noEmit`, map API smoke 18/18, minimap smoke 0 failures with known 450/451 warning, WS load 64/64 ready with 0 errors and keepalive p95 632 ms, and Stage 5 UI smoke 88 screenshots with 0 critical console errors passed. This round also exported missing original scene sprite libs (`NPC/07`, `NPC/08`, `NPC/16`, `NPC/27`, `NPC/45`, `NPC/52`, `NPC/83`, `Monster/000`, `Monster/139`), fixed map-transfer minimap state from gateway `MapInformation`, and hardened concurrent account-store file saves for Windows load. Rust validation passed `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `fmt --check`, and `git diff --check`. Human Crystal visual/feel acceptance remains required for 100% Accepted.
- 2026-04-26 R224: Local packet trace matrix evidence restored. `packet_trace --list-flows` works, `mir2-gateway` passes 53/53 including packet trace bin tests 6/6, and require-local `packet_trace --matrix` wrote 9 TCP-traceable artifacts under `docs/generated/packet-traces/r224-matrix` with `localOk=true`. Live Crystal trace comparison was later refreshed by R298 and accepted under R300 stable-diff policy.
- 2026-04-26 R223: **100% Candidate** automated gate passed. Stage 5 UI smoke captured 88 screenshots with advanced Stage 5 systems state and compact Mail/Report bounds. Direct `next build`, `tsc --noEmit`, map API smoke 18/18, minimap asset smoke 0 failures with known 450/451 warning, WS load 64/64, `mir2-game-data` 22/22, `mir2-gateway` 47/47, full `mir2-simulation` 664/664, `fmt --check`, and `diff --check` passed. Human Crystal visual/feel acceptance remains required for 100% Accepted.
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
- 2026-04-26 R219-R222: web `tsc --noEmit`, direct `next build`, Stage 5 UI smoke passed with 85 screenshots, map API smoke archived 18/18 successful requests, minimap asset smoke archived 0 failures with the known 450/451 warning, and WS load refreshed at 64/64 ready with 0 errors. Login/select lifecycle, confirmed character delete/recreate, compact multi-panel bounds, compact system-menu overflow fix, and NPC dialog link-capable rendering are covered. Human Crystal visual/feel acceptance remains required.
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
