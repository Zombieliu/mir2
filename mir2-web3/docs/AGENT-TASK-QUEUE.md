# Agent Task Queue

> Latest product-evolution sync: 2026-04-27-R228 completed. Admin `SendSystemMail` now reaches live game-visible state: `apps/admin-api` tries `ADMIN_GATEWAY_MAIL_URL` via a reqwest-free plain TCP HTTP POST helper and falls back to the persistent account store; `apps/gateway` exposes `POST /admin/system-mail` to deliver into the running gateway `SimulationConfig.account_store`; `apps/simulation` persists Stage 5 mail into `CharacterSaveRecord.stage5_systems_json`; and the player web Mail panel can display, claim, and delete those messages. Runtime smoke proved Admin Web `:3020` -> Admin API `:7420` -> gateway `:7110` delivered `deliveryMode: "gateway_live"` to `Scout`, then a gateway WS `stage5Command mail.claim` marked it claimed, raised gold from 1280 to 6280, and delivered one `red-potion`.

> Latest product-evolution sync: 2026-04-27 Admin operations foundation advanced. `apps/admin-api` now has persistent-storage-ready command/audit repository traits, in-memory repositories, Axum HTTP routes, and a `SendSystemMail` domain outbox executor. `apps/admin-web` now has a production-shaped desktop operations UI across Dashboard, Players, Player Detail, Economy, Activities, Servers, Risk, GM Tools, and Audit, with the GM mail form wired through Next to the Rust Admin API. Verification: `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`, `cargo +1.89.0 fmt --check`, admin-web `tsc --noEmit`, admin-web `next build`, direct Rust API curl write, Next route proxy curl write, and Playwright screenshots `docs/admin-web-dashboard-smoke.png` / `docs/admin-web-gm-tools-smoke.png`.

> Latest sync: R225 completed. Mac-local Candidate regression is green: web `tsc --noEmit`, direct `next build`, Stage 5 UI smoke (88 screenshots, summary counts in manifest), map API smoke 18/18, minimap asset smoke 0 failures with known 450/451 warning, WS load 64/64, `mir2-game-data` 22/22, `mir2-gateway` 54/54 including packet trace bin tests 7/7, `mir2-simulation` 664/664, require-local `packet_trace --matrix` wrote 9 local artifacts with 17 intended skips under `docs/generated/packet-traces/r225-matrix`, `cargo +1.89.0 fmt --check`, and `git diff --check`. R225 also added the Windows continuation checklist and cleaned the stale gateway README. Active follow-up round is R226 for Windows/live Crystal/human acceptance blockers; status remains **100% Candidate**, backend/server tracked slice **99.70%**, real full-project accepted 1:1 **roughly 90.0%**.

> Latest sync: R224 completed. The `mir2-gateway` `packet_trace` bin target is restored, `--list-flows` works, `mir2-gateway` now passes 53/53 including packet trace bin tests 6/6, and local require-mode `packet_trace --matrix` wrote 9/9 TCP-traceable matrix artifacts with `localOk=true` under `docs/generated/packet-traces/r224-matrix`. Truthful status split: automated evidence is **100% Candidate**, backend/server tracked slice remains **99.70%**, and real full-project accepted 1:1 remains **roughly 90.0%**. Active follow-up round is R225 for final human acceptance / external blockers; remaining non-routine gates are final human Crystal visual/feel acceptance, missing local `Crystal/Build/Server/Debug/Server.MirDB`, and missing live `MIR2_CRYSTAL_TCP_ADDR`.

> Latest sync: R219-R222 completed. Frontend/global evidence advanced across login/select lifecycle, archived map API/minimap asset smoke JSON, refreshed WS load, compact multi-panel bounds, compact system-menu overflow fix, and NPC dialog link-capable rendering. Stage 5 UI smoke now captures 85 screenshots and records `loginFlow`, `selectFlow`, expanded `compactPanelLayout`, and existing broad gameplay/system flows. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke (85 screenshots), map API smoke 18/18, minimap asset smoke 0 failures with known 450/451 warning, WS load 64/64, `cargo +1.89.0 fmt --check`, and `git diff --check`. Active backend/global round is R223; backend/server parity estimate is 99.70%, whole-project 1:1 estimate is 90.0%.


> Latest sync: R172 completed. Successful high-level NPC interaction no longer emits runtime-only `sim.talkingToNpc`; NPC `ObjectChat`/dialog packet surfaces and Crystal NPC script/service flows are preserved. Validation: focused `npc_interaction` 2/2, `crystal_npc_dialog` 1/1, `crystal_npc_service` 1/1, broad `crystal_npc` 52/52, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 648/648. Active backend round is R173; backend/server parity estimate is 99.70%.


> Latest sync: R171 completed. Direct high-level ground-drop pickup invalid target/distance handling no longer emits runtime-only `sim.itemNoLongerOnGround`, `sim.targetNotGroundDrop`, or `sim.moveCloserToPickItem`; Crystal owner/full-bag pickup messages and current-cell packet pickup behavior are preserved. Validation: focused direct-pickup tests 3/3, `pickup` 18/18, adjacent `drop` 42/42, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 648/648. Active backend round is R172; backend/server parity estimate is 99.70%.


> Latest sync: R170 completed. Missing defeated-monster entity handling no longer emits runtime-only `sim.defeatedMonsterEntityMissing`; normal death/drop packet surfaces are preserved. Validation: focused missing-entity silent test 1/1, visible death packet test 1/1, adjacent `drop` 41/41, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 645/645. Active backend round is R171; backend/server parity estimate is 99.70%.


> Latest sync: R169 completed. Monster death drop success paths no longer emit runtime-only `sim.monsterDroppedGoldOnGround` / `sim.monsterDroppedItem` chats; ground gold/item drops, quest-drop routing, and pickup packet surfaces are preserved. Validation: focused item-drop no-chat 1/1, gold-drop no-chat/pickup 1/1, adjacent `drop` 41/41, `pickup` 15/15, `attack` 76/76, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 644/644. Active backend round is R170; backend/server parity estimate is 99.70%.


> Latest sync: R168 completed. VampireSpider summoned death explosion no longer emits runtime-only `sim.targetDefeated` defeat chat; explosion damage, summon despawn timing, and packet health surfaces are preserved. Validation: focused vampire-spider no-chat explosion test 1/1, adjacent `spider` 6/6, `attack` 76/76, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R169; backend/server parity estimate is 99.70%.


> Latest sync: R167 completed. Ordinary combat hit resolution no longer emits local runtime damage narration (`sim.youHitTargetForDamage`, `sim.targetDefeated`, `sim.monsterPressuresYouForDamage`); packet health/struck/death surfaces and Trainer DPS reporting are preserved. Validation: focused player-hit no-chat test 1/1, adjacent `attack` 76/76, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R168; backend/server parity estimate is 99.70%.


> Latest sync: R166 completed. Successful cast-skill paths no longer emit local `sim.castSkill` helper chat; buff/heal and summon success now preserve state mutation/spawn behavior without generic success narration. Validation: focused `casting` suite 6/6, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R167; backend/server parity estimate is 99.70%.


> Latest sync: R165 completed. Cast-skill high-level entrypoint (`cast_skill`) now silently rejects before `StartGame` instead of emitting local `sim.joinWorldBeforeCastingSkills` helper chat. Validation: focused pre-start cast-skill test 1/1, adjacent `casting` 6/6, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 643/643. Active backend round is R166; backend/server parity estimate is 99.70%.


> Latest sync: R164 completed. Interaction high-level/dialog entrypoints (`interact`, `select_npc_dialog_target`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeInteracting` helper chat. Validation: focused pre-start interaction test 1/1, adjacent `npc_interaction` 2/2, `crystal_npc_dialog` 1/1, `crystal_npc_service` 1/1, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 642/642. Active backend round is R165; backend/server parity estimate is 99.70%.


> Latest sync: R163 completed. Harvest high-level and packet entrypoints (`harvest`, `Harvest`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeHarvesting` helper chat. Validation: focused pre-start harvest test 1/1, adjacent `harvest` 9/9, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 641/641. Active backend round is R164; backend/server parity estimate is 99.70%.


> Latest sync: R162 completed. Attack high-level and packet entrypoints (`attack`, `Attack`, `RangeAttack`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeAttacking` helper chat. Validation: focused pre-start attack test 1/1, adjacent `attack` 76/76, combat trace focused test 1/1, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 640/640. Active backend round is R163; backend/server parity estimate is 99.70%.


> Latest sync: R161 completed. Movement high-level and packet entrypoints (`move_to`, `Walk`, `Run`, `Turn`) now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforeMoving` / `sim.joinWorldBeforeTurning` helper chat. Validation: focused pre-start movement test 1/1, adjacent `walk` 6/6, `run_` 3/3, `transfer_map` 2/2, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 639/639. Active backend round is R162; backend/server parity estimate is 99.70%.


> Latest sync: R160 completed. Pickup high-level and packet entrypoints now silently reject before `StartGame` instead of emitting local `sim.joinWorldBeforePickingUpItems` helper chat. Validation: focused pre-start pickup test 1/1, pickup suite 15/15, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 638/638. Active backend round is R161; backend/server parity estimate is 99.70%.


> Latest sync: R159 completed. Trainer immediate damage reporting now routes through Crystal `server.PetInflictedDamageDps` with localized `server.You` actor; modeled `Physical Agility` damage type and DPS value are preserved. Validation: focused trainer test 1/1, `cargo +1.89.0 fmt --check`, and full `mir2-simulation` 638/638. Active backend round is R160; backend/server parity estimate is 99.70%.


Last updated: 2026-04-26

Purpose: queue autonomous tasks for reaching **100% Candidate**. The Coordinator should keep this file current as rounds complete.

Restart handoff: if the Codex session is reopened after shutdown or context loss, read `docs/AGENT-RESUME-HANDOFF.md` before continuing the active round. The user wants the previous subagent workflow to continue without routine confirmations.

Product evolution handoff: after the 1:1 Candidate baseline, future product work should also read `docs/POST-1TO1-EVOLUTION-PLAN.md`, `docs/TECH-MODERNIZATION-RFC.md`, `docs/PLATFORM-CLIENT-STRATEGY.md`, and `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`. Database, cache, login UI, admin backend, global zone, client distribution, and NPC script parser changes are expected product-evolution areas, not automatic Crystal parity regressions.

Status values:

- `[ ]` queued
- `[~]` active
- `[x]` complete and verified
- `[!]` blocked

## Active Round: 2026-04-26-R226

Restart note: R225 refreshed the Mac-local Candidate regression bundle and local packet trace matrix evidence. Backend gameplay code remains unchanged from R183 at 99.70% and is green with package regressions. Real full-project accepted 1:1 remains roughly 90.0% until human Crystal visual/feel acceptance, live Crystal trace comparison, and blocked source-data decisions are closed. Remaining items are not routine Mac implementation work: final human Crystal visual/feel acceptance, missing local `Crystal/Build/Server/Debug/Server.MirDB` for blocked map-data import, and missing `MIR2_CRYSTAL_TCP_ADDR` for live Crystal comparison.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [~] | Track Windows/live Crystal/human acceptance blockers after `R225` Mac-local refresh | Coordinator | docs only unless new assets/endpoints are provided | R225 passed 88 Stage 5 screenshots, archived map/minimap evidence, WS load 64/64, web build/type checks, `mir2-game-data` 22/22, `mir2-gateway` 54/54, full `mir2-simulation` 664/664, require-local matrix 9/9 local artifacts, `fmt --check`, and `diff --check`; `docs/WINDOWS-CONTINUATION.md` defines the Windows handoff; automation status is 100.0% Candidate, but real full-project accepted 1:1 remains roughly 90.0%. |
| [~] | Plan post-1:1 product evolution boundaries | Coordinator | docs/product specs first | `docs/POST-1TO1-EVOLUTION-PLAN.md` defines the first boundary for database/cache, login UI, NPC script parser, and product gameplay changes while preserving the current Candidate baseline as a regression reference. |
| [~] | Finalize technical modernization RFC | Coordinator | docs only until approved | `docs/TECH-MODERNIZATION-RFC.md` captures the current first-principles direction: Rust simulation authority, Postgres authoritative persistence, Redis non-authoritative cache/session/routing, global services plus zone/channel runtime, Bevy + NextJS frontend split, audited admin backend, and developer-oriented NPC DSL compiled to Rust IR. |
| [~] | Validate platform/client distribution strategy | Coordinator | docs and prototypes only until approved | `docs/PLATFORM-CLIENT-STRATEGY.md` records Web as first-class, Tauri shell for near-term Windows/macOS, mobile after validation, Bevy native desktop as a performance escape hatch, and consoles as a deferred separate platform project. |
| [~] | Finalize admin operations architecture | Coordinator | docs first, then admin command/audit model | `docs/ADMIN-OPERATIONS-ARCHITECTURE.md` defines Admin Web, Admin API/control plane, RBAC, audit records, typed admin commands, command execution, online/offline target handling, content publishing, and MVP scope. |
| [~] | Build admin command/audit foundation | Coordinator | `apps/admin-api` | `apps/admin-api` now has typed permissions, operators, targets, admin commands, command envelopes, audit records, idempotency guard, executor trait, and in-memory control-plane tests. First verification: `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (5/5). |
| [~] | Build admin HTTP and web console foundation | Coordinator | `apps/admin-api`, `apps/admin-web`, docs | `apps/admin-api` now exposes Axum routes and repository traits; `SendSystemMail` is wired to a domain outbox executor. `apps/admin-web` implements the first desktop operations UI and forwards GM mail commands to Rust through `/api/admin/system-mail`. Live game-state mail delivery, Postgres repositories, and real operator auth remain next-step work. |

## Product Evolution Round: 2026-04-27-R227

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Land Admin API repository/HTTP foundation and Admin Web UI | Coordinator | `apps/admin-api`, `apps/admin-web`, `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`, docs/screenshots | Added `AdminCommandRepository` and `AuditRepository` traits, in-memory command/audit stores, Axum HTTP routes, `SendSystemMail` domain executor/outbox, standalone Next admin console pages, Next proxy route for GM mail, docs, and smoke screenshots. Verified by Rust locked tests/fmt, admin-web typecheck/build, direct Rust API curl write, Next proxy curl write, and Playwright screenshots. |

## Product Evolution Round: 2026-04-27-R228

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Connect audited GM system mail to live game-visible Stage 5 mail | Coordinator | `apps/admin-api`, `apps/gateway`, `apps/simulation`, `apps/web`, `apps/admin-web`, `docs/ADMIN-OPERATIONS-ARCHITECTURE.md` | Added live gateway delivery for `SendSystemMail`, persistent account-store fallback, a gateway admin mail endpoint, in-game Mail panel claim/delete actions, and a gateway endpoint unit test. Verified by focused simulation/admin-api/gateway tests, web/admin-web typecheck/build, Admin Web curl through Rust API, outbox `deliveryMode: "gateway_live"`, account-store inspection, gateway WS snapshot mail visibility, and WS `mail.claim` state mutation. |

## Completed Round: 2026-04-26-R225

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Refreshed Mac-local Candidate regression and Windows handoff | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, `apps/web/scripts/smoke-stage5-ui.mjs`, `apps/gateway/README.md`, `docs/WINDOWS-CONTINUATION.md`, `docs/generated/packet-traces/r225-matrix/*`, `docs/stage5-screenshots/*`, docs | Added manifest summary counts to Stage 5 UI smoke and packet trace matrix summary counts to `latest-matrix.json`; fixed the summary field to use `compactTextLayout.checked`; refreshed Stage 5/map/minimap/WS evidence; wrote R225 packet trace matrix artifacts; cleaned stale gateway README status; and added the Windows continuation checklist. Verified by web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, map/minimap smokes, WS load, Rust package tests, `fmt --check`, and `diff --check`. |

## Completed Round: 2026-04-26-R224

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Restored local packet trace matrix harness | Coordinator | `apps/gateway/src/bin/packet_trace.rs`, `docs/generated/packet-traces/r224-matrix/*`, docs | Reintroduced `packet_trace` with `--list-flows`, single-flow capture, matrix artifact writing, local/Crystal endpoint capture, diff summaries, fixture metadata, and require-mode enforcement. Local gateway on `127.0.0.1:7310` passed `MIR2_PACKET_TRACE_REQUIRE_LOCAL=1 cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix` with 9 artifacts and 17 intentionally skipped non-TCP matrix entries. `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` passed 53/53. Live Crystal diff remains blocked until `MIR2_CRYSTAL_TCP_ADDR` is provided. |

## Completed Round: 2026-04-26-R223

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Completed the 100% Candidate automated evidence gate | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, `docs/generated/*`, docs | R223 added advanced Stage 5 systems smoke evidence for trade item/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, mining/craft, and mail delete state; added compact Mail/Report panel bounds screenshots; refreshed map/minimap/WS evidence; and reran full web/Rust validation. The then-missing `packet_trace` bin target was closed in R224. |

## Completed Round: 2026-04-26-R222

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Completed the 90% frontend/global evidence batch | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/page.tsx`, `apps/web/app/globals.css`, `apps/web/scripts/smoke-stage5-ui.mjs`, `apps/web/scripts/smoke-crystal-map-api.mjs`, `apps/web/scripts/smoke-crystal-minimap-assets.mjs`, `docs/stage5-screenshots/*`, `docs/generated/*`, docs | R219-R222 added login/select lifecycle smoke evidence, character delete/recreate evidence, archived map API/minimap smoke JSON, refreshed WS load, compact multi-panel bounds, compact system-menu overflow fix, and NPC dialog link-capable rendering. Stage 5 UI smoke now captures 85 screenshots. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke, map/minimap smokes, WS load, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R218

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added compact inventory panel layout evidence and completed the 80% target batch | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | R210-R218 added Mail/Report/NPC/system menu panel state, broad systems state, guild/group chat filters, Character repair/special-repair UI, ground item/gold pickup, combat target state, system menu transfer-list routing, Battle Focus casting, and compact inventory bounds evidence. Stage 5 UI smoke now captures 71 screenshots and writes the extended manifest. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 71 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R209

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage password submit/no-service smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now fills Set Storage Password, verifies mismatched confirmation keeps submit disabled and shows the mismatch warning, submits matching `Safe123` without an active storage service, verifies `hasStoragePassword` remains false with no-service chat feedback, captures `stage5-storage-password-mismatch.png` and `stage5-storage-password-submit-no-service.png`, and records the extended `storagePasswordFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 60 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R208

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Enabled and smoke-verified storage password panel entry | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Protect is now reachable when no storage password is set. Stage 5 UI smoke opens Set Storage Password, verifies title/prompt/input count/disabled submit/debug storage password state, closes the panel without submitting credentials, captures `stage5-storage-password-panel.png`, and records `storagePasswordFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 58 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R207

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage Take Back no-service smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Take Back for stored Red Potion, selects an inventory slot without an active storage service, verifies bag1 Red Potion remains quantity 3 and storage Red Potion remains quantity 10, captures `stage5-storage-takeback-red-potion-selected.png`, `stage5-storage-takeback-red-potion-result.png`, and `stage5-storage-takeback-red-potion-feedback.png`, and records `storageTakeBackFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 57 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R206

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage Store Item no-service smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Store Item for Dagger, selects a warehouse slot without an active storage service, verifies Dagger remains in bag1 slot 4 and existing storage items are preserved, exposes `storageItems` in Stage 5 debug state, captures `stage5-storage-store-dagger-selected.png`, `stage5-storage-store-dagger-result.png`, and `stage5-storage-store-dagger-feedback.png`, and records `storageStoreFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 54 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R205

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory Sell Item no-service smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Sell Item for Dagger, confirms without an active sell service, verifies Dagger remains in bag1 slot 4 and gold stays at 1180, captures `stage5-inventory-sell-dagger-panel.png` and `stage5-inventory-sell-dagger-no-service.png`, and records `inventorySellFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 51 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R204

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added belt mouse-use smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks Red Potion directly in the belt, verifies belt quantity drops from 5 to 4, keeps the existing hotkey path verifying 4 to 3, captures `stage5-belt-mouse-use-red-potion.png`, and records `beltMouseUseFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 49 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R203

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Fixed and verified Character equipment remove | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Character RemoveItem now targets the `inventory` grid and chooses the first free bag1 slot instead of hardcoding occupied slot 0 / invalid `equipment` grid. Stage 5 UI smoke verifies Dagger leaves the weapon slot and returns to bag1 slot 4, captures `stage5-character-remove-dagger.png`, and records `characterRemoveFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 48 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R202

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory item-drop smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Delete Item for Blue Potion, confirms the drop, verifies quantity drops from 3 to 2 and a `Blue Potion` ground label appears, captures `stage5-inventory-drop-blue-potion-panel.png` and `stage5-inventory-drop-blue-potion.png`, and records `inventoryDropFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 47 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R201

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory Split Item smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now opens Split Item for Red Potion, confirms count 1, verifies inventory quantity drops from 4 to 3 while belt quantity rises from 5 to 6 and total Red Potion quantity stays 9, captures `stage5-inventory-split-red-potion-panel.png` and `stage5-inventory-split-red-potion.png`, and records `inventorySplitFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 45 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R200

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory item-move smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now context-clicks Wooden Sword in bag1, moves it from slot 4 to slot 10, captures `stage5-inventory-move-wooden-sword.png`, and records `inventoryMoveFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 43 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R199

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory Drop Gold smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/app/original-client-shell.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `gold`; UI smoke opens Drop Gold, confirms 100 gold, verifies gold drops from 1280 to 1180 and a `100 Gold x100` ground label appears, captures `stage5-inventory-drop-gold-panel.png` and `stage5-inventory-drop-gold.png`, and records `inventoryGoldFlow`. Missing `ui.confirm` fallback text is fixed. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 42 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R198

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added HUD Skill/Option button smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks HUD Skill to open Character Spells and HUD Option to open Stats II, captures `stage5-hud-skill-spells.png` and `stage5-hud-option-stats2.png`, and records `hudButtonFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 40 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R197

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory equipment smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `equipmentItems`; UI smoke clicks Dagger in bag1, verifies Dagger moves into the weapon equipment slot, captures `stage5-inventory-equip-dagger.png`, and records `inventoryEquipFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 38 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R196

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory item-use smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks Red Potion in bag1, verifies the quantity drops from 5 to 4, captures `stage5-inventory-use-red-potion.png`, and records `inventoryUseFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 37 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R195

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added expanded storage rent smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `hasExpandedStorage`; UI smoke clicks Rent from locked storage page 2, verifies page 2 becomes unlocked with expanded storage active and 160-slot capacity text, captures `stage5-storage-page2-rented.png`, and records the rented state in `storageFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 36 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R194

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added system menu action smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records `systemMenuFlow` for menu open and Character, Inventory, and Quest menu actions; captures `stage5-system-menu.png`, `stage5-system-menu-character.png`, `stage5-system-menu-inventory.png`, and `stage5-system-menu-quest.png`; and verifies transfer/action labels plus resulting panels. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 35 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R193

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added chat control smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records `chatFlow` for All, Shout filter, All restored, Settings open, collapsed, expanded restored, and Report open; captures `stage5-chat-shout-filter.png`, `stage5-chat-settings.png`, `stage5-chat-collapsed.png`, and `stage5-chat-report.png`; and verifies DOM state transitions. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 31 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R192

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added storage page navigation smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records storage page 1, locked page 2, and restored page 1 states in `storageFlow`; captures `stage5-storage-page2-locked.png` and `stage5-storage-page1-restored.png`; and verifies locked expanded-storage text plus restored item counts. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 27 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R191

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added character tab smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `activeCharacterTab` and `knownSkills`; UI smoke switches char -> stats1 -> stats2 -> spells -> char, captures `stage5-character-stats1.png`, `stage5-character-stats2.png`, `stage5-character-spells.png`, and `stage5-character-char-restored.png`, and records `characterFlow` with equipment/stat/spell counts. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 25 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R190

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added inventory tab smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `inventoryItems` and `activeInventoryTab`; UI smoke switches bag1 -> bag2 -> quest -> bag1, captures `stage5-inventory-bag2.png`, `stage5-inventory-quest.png`, and `stage5-inventory-bag1-restored.png`, and records `inventoryFlow` with item counts and quest entry count. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 21 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R189

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added belt hotkey-use smoke evidence | Coordinator | `apps/web/app/page.tsx`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 debug state now exposes `beltItems`; UI smoke presses hotkey `1`, waits for slot-1 Red Potion quantity to fall from 5 to 4, captures `stage5-belt-hotkey-use.png`, and records `beltUseFlow`. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 18 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R188

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added belt interaction smoke evidence | Coordinator | `apps/web/app/globals.css`, `apps/web/lib/original-ui.ts`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records horizontal, vertical, rotate-back, and closed belt states in `beltFlow`; captures `stage5-belt-vertical.png`, `stage5-belt-horizontal.png`, and `stage5-belt-closed.png`; fixes doubled belt slot-label offsets; moves the vertical belt clear of Quest; and asserts labels stay inside the belt with no Quest overlap. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 17 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R187

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added minimap interaction smoke evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now clicks minimap collapse, BigMap re-expand, and Mail open paths; captures `stage5-minimap-collapsed.png`, `stage5-minimap-expanded.png`, and `stage5-minimap-mail.png`; and writes `minimapFlow` state to the manifest. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 14 screenshots, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R186

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added compact visible-text overflow checks | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/globals.css`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now checks visible core quest/HUD/minimap/belt/chat/entity text at compact viewport and writes `compactTextLayout`; the check caught minimap title overflow, fixed by splitting map title and Safe Zone into stable two-line text. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 11 screenshots and 33 compact text nodes checked, visual screenshot inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R185

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Added desktop/compact Stage 5 screenshot evidence | Coordinator | `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, docs | Stage 5 UI smoke now records desktop 1024x768 and compact 820x640 viewports, captures `stage5-compact-game.png`, writes compact layout bounds into the manifest, and fails on core stage/HUD/chat/minimap overflow. Validation: `node --check`, gateway/web health, Stage 5 UI smoke with 11 screenshots, compact screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check`. |

## Completed Round: 2026-04-26-R184

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Advanced frontend/global smoke parity | Coordinator | `apps/web/app/original-client-shell.tsx`, `apps/web/app/page.tsx`, `apps/web/lib/crystal-map-loader.ts`, `apps/web/scripts/smoke-stage5-ui.mjs`, `docs/stage5-screenshots/*`, `docs/generated/load/latest-ws.json`, docs | Chat panel now defaults/follows latest filtered lines with a live scroll knob; no-WebGL headless browsers stay on DOM UI instead of Bevy panic; Crystal map API uses packaged starter-region fallback when local Crystal map files are missing; Stage 5 UI smoke detects macOS Chrome. Validation: web `tsc --noEmit`, direct `next build`, minimap smoke, map API smoke, Stage 5 UI smoke (10 screenshots), gateway health 7110, WS load 64/64, `cargo +1.89.0 fmt --check`, `git diff --check`. |

## Completed Round: 2026-04-26-R183

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Moved quest interaction hint out of runtime `sim` namespace | Coordinator | `apps/simulation/src/runtime.rs`, `packages/tooling/scripts/import-crystal-localization.mjs`, `packages/game-data/data/generated/localization_bundle.json`, `apps/web/lib/generated/localization_bundle.json`, docs | UI/localization namespace cleanup: `build_interaction_hints` now uses `custom.interaction.questHint`, generated bundles and importer are in sync, and runtime has no `sim.*` references; `mir2-game-data` (22/22); focused snapshot test (1/1); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664). |

## Completed Round: 2026-04-26-R182

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed no-script NPC idle fallback dialog | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: no-script/no-page NPC interaction now silently returns existing packets like Crystal `NPCScript.Call` with no matching page, instead of opening runtime-only idle dialog text; focused no-script NPC (1/1); adjacent `npc_interaction` (2/2); broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664). |

## Completed Round: 2026-04-26-R181

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized quest-required drop feedback | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal localization/packet-surface parity: quest-required drop feedback now uses Crystal `server.YouFound` and no longer emits runtime-only `sim.youSecuredQuestItem`, `sim.questReturnForReward`, or `sim.questProgressWasps` progress chats; `GainedItem` and quest state updates remain intact; focused quest-required drop (1/1); adjacent `quest_required_drop` (3/3); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664). |

## Completed Round: 2026-04-26-R180

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized start-game welcome chat | Coordinator | `apps/simulation/src/runtime.rs`, `apps/gateway/src/session.rs`, docs | Crystal localization/packet-surface parity: `StartGame` welcome chat now uses `server.Welcome` with localized `server.GameName` and `ChatType::Hint` instead of runtime-only `sim.welcomeCharacter` System text; focused simulation/gateway `start_game_emits_bootstrap_sequence` (1/1 each); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664); full `mir2-gateway` (47/47). |

## Completed Round: 2026-04-26-R179

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed normal chat runtime echo | Coordinator | `apps/simulation/src/runtime.rs`, `apps/gateway/src/session.rs`, docs | Crystal packet-surface parity: normal `ClientPacket::Chat` before `StartGame` now returns no packets, and in-game normal chat emits only `ObjectChat` with `Name: message` instead of a runtime-only `sim.echoChat` self `Chat` echo; `@ADDSTORAGE` remains as the modeled helper command; simulation `chat_` (43/43); gateway `chat_` (2/2); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (664/664); full `mir2-gateway` (47/47). |

## Completed Round: 2026-04-26-R178

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed cast-skill failure runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `cast_skill` unknown-skill, cooldown, unwired-definition, missing-player, no-MP, unwired summon-spell, and missing summon-template failures no longer emit runtime-only `sim.skillNotKnown`, `sim.skillCooldown`, `sim.skillLogicNotWired`, `sim.playerNotInWorld`, or `sim.notEnoughMp`; successful buff/summon behavior remains intact; `casting` (9/9); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (663/663). |

## Completed Round: 2026-04-26-R177

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed MoveItem unsupported fallback runtime chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: unreachable/unsupported `MoveItem` missing-source fallback no longer emits `sim.itemNotFoundInBag`; unsupported grids remain failed-ack only, while Inventory/Storage missing-source keeps Crystal `server.ItemMoveErrorReport`; `move_item` (26/26); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (660/660). |

## Completed Round: 2026-04-26-R176

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed stale active-dialog missing-NPC/no-script runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: active NPC dialog target follow-up with a missing NPC entity or an NPC lacking script metadata now dismisses silently without `sim.targetNotGroundDrop` or `sim.npcNoMilestoneScript`; ordinary no-script NPC idle fallback remains intact; focused stale-dialog tests (2/2), adjacent `npc_interaction` (2/2), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (660/660). |

## Completed Round: 2026-04-26-R175

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed NPC dialog helper no-active/invalid-target/no-input runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level dialog target/input helper no-active-dialog, invalid-target, and no-pending-input failures no longer emit `sim.npcNoMilestoneScript` or `sim.itemNoActiveUse`; successful dialog link/input/service flows remain intact; focused dialog-helper tests (3/3), adjacent `npc_interaction` (2/2), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (658/658). |

## Completed Round: 2026-04-26-R174

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed direct NPC interaction invalid target/direction/range runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `interact(object_id)` missing-target, same-tile/no-direction, and out-of-range failures no longer emit `sim.targetNoScriptedInteraction`, `sim.noValidInteractionDirection`, or `sim.moveCloserToTalkToNpc`; successful NPC dialog/script/service flows remain intact; focused direct-interact tests (3/3), adjacent `npc_interaction` (2/2), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (655/655). |

## Completed Round: 2026-04-26-R173

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed direct attack invalid target/state/range runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `attack(object_id)` missing-target, non-monster, dead/hidden/stoned, no-direction, and out-of-range failures no longer emit runtime-only `sim.*` chats while preserving turn packets, normal attacks, hidden reveal, Zuma wake, and delayed hit behavior; focused direct-attack tests (4/4), hidden/Zuma focused tests (2/2), adjacent `attack` (80/80); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (652/652). |

## Completed Round: 2026-04-26-R172

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed successful NPC interaction runtime chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level NPC interaction no longer emits `sim.talkingToNpc`; NPC `ObjectChat`/dialog surfaces and Crystal script/service flows remain intact; focused `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), `crystal_npc_service` (1/1), broad `crystal_npc` (52/52); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (648/648). |

## Completed Round: 2026-04-26-R171

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed direct pickup invalid target/distance runtime chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `pick_up(object_id)` missing-object, non-ground-target, and out-of-cell failures now return silently instead of emitting `sim.itemNoLongerOnGround`, `sim.targetNotGroundDrop`, or `sim.moveCloserToPickItem`; Crystal owner/full-bag pickup messages and current-cell packet pickup behavior remain intact; focused direct-pickup tests (3/3); adjacent `pickup` (18/18), `drop` (42/42); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (648/648). |

## Completed Round: 2026-04-26-R170

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only missing defeated-entity chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: missing defeated-monster entity handling now silently returns without `sim.defeatedMonsterEntityMissing`, while normal death/drop packet surfaces remain intact; focused missing-entity silent test (1/1), visible death packet test (1/1); adjacent `drop` (41/41); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (645/645). |

## Completed Round: 2026-04-26-R169

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only monster death-drop success chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: monster death drop success paths no longer emit `sim.monsterDroppedGoldOnGround` / `sim.monsterDroppedItem` while preserving ground gold/item drops, quest-drop routing, and pickup packets; focused item-drop no-chat (1/1), focused gold-drop no-chat/pickup (1/1); adjacent `drop` (41/41), `pickup` (15/15), `attack` (76/76); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (644/644). |

## Completed Round: 2026-04-26-R168

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only summoned VampireSpider defeat chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: summoned VampireSpider death explosion no longer emits `sim.targetDefeated` while preserving explosion damage and summon despawn behavior; focused vampire-spider no-chat explosion test (1/1); adjacent `spider` (6/6), `attack` (76/76); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R167

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only ordinary combat damage narration | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: ordinary player/monster hit resolution no longer emits `sim.youHitTargetForDamage`, `sim.targetDefeated`, or `sim.monsterPressuresYouForDamage`; focused player-hit no-chat test (1/1); adjacent `attack` (76/76); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R166

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only cast-skill success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: successful buff/heal and summon `cast_skill` paths no longer emit generic `sim.castSkill` chat while preserving state mutation/spawns; focused `casting` (6/6); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R165

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only cast-skill helper chat before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `cast_skill` now silently rejects before `StartGame`; focused pre-start cast-skill test (1/1); adjacent `casting` (6/6); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (643/643). |

## Completed Round: 2026-04-26-R164

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only interaction helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `interact` plus dialog target follow-up now silently reject before `StartGame`; focused pre-start interaction test (1/1); adjacent `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), `crystal_npc_service` (1/1); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (642/642). |

## Completed Round: 2026-04-26-R163

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only harvest helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `harvest` plus packet `Harvest` now silently reject before `StartGame`; focused pre-start harvest test (1/1); adjacent `harvest` (9/9); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (641/641). |

## Completed Round: 2026-04-26-R162

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only attack helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `attack` plus packet `Attack` and `RangeAttack` now silently reject before `StartGame`; focused pre-start attack test (1/1); adjacent `attack` (76/76); combat trace focused test (1/1); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (640/640). |

## Completed Round: 2026-04-26-R161

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only movement/turning helper chats before `StartGame` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `move_to` plus packet `Walk`, `Run`, and `Turn` now silently reject before `StartGame`; focused pre-start movement test (1/1); adjacent `walk` (6/6), `run_` (3/3), `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `mir2-simulation` (639/639). |

## Completed Round: 2026-04-26-R158

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized trainer average damage reporting and Crystal format placeholders | Coordinator | `packages/game-data/src/lib.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal localization parity: `{index:format}` placeholders now substitute in localization templates and trainer idle average damage uses `server.AverageDamageOnTrainer`; `mir2-game-data` (22/22); focused trainer test (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R157

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized benediction-oil weapon luck outcome chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal localization parity: benediction-oil no-effect/luck/curse outcomes now use `server.WeaponNoEffect`, `server.WeaponLuck`, and `server.WeaponCurse`; focused `benediction_oil` (4/4); adjacent `use_item` (42/42); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R156

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only expanded-storage helper success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: `@ADDSTORAGE` now emits modeled `ResizeStorage` without hardcoded `"Expanded storage activated."` chat; focused `addstorage` (2/2); adjacent `storage` (43/43); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R155

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized group pickup notice through Crystal `server.FriendlyPickedUpItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal localization parity: `ShowGroupPickup` item notices now use the generated localization bundle instead of hardcoded English formatting; focused group pickup test (1/1); adjacent `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R154

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only high-level use/drop before-start chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: high-level `use_item(key)` and `drop_item(key)` before `StartGame` now emit no packets/chat while preserving post-start behavior; adjacent `drop_item` (10/10); focused consumable helper (1/1); adjacent `use_item` (42/42); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R153

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only high-level drop helper missing-item chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: missing high-level `drop_item(key)` requests now emit no packets/chat and preserve state, aligned with packet `DropItem` missing-source behavior; focused drop helper test (1/1); adjacent `drop_item` (10/10); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R152

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized map-transfer not-in-world rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused transfer-bound test (1/1); adjacent `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R151

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized missing-template `RequestItemInfo` failure through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused request-item-info test (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R150

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized map-transfer bounds rejection through Crystal `server.CannotPositionMoveOnMap` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.CannotPositionMoveOnMap`; focused transfer-bounds test (1/1); adjacent `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R149

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed remaining runtime-only Stage 5 event/hero helper success chats | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: `event.spawn` and `hero.behaviour` successes now mutate state without simulator-only narration; focused conquest/event/hero test (1/1); broader `stage5_` (26/26); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R148

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only debug Crystal transfer success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: debug `crystal:<map>:<x>:<y>` transfers now emit map/location packets without simulator-only `"Transferred to Crystal map ..."` chat; focused debug transfer test (1/1); adjacent `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R147

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed generic runtime-only Stage 5 helper success chats while preserving helper state mutations | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet-surface parity: group/social/mail/trade/auction/conquest/hero/profession helper successes no longer emit simulator-only narration; focused `stage5_` (26/26); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R146

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 event-spawn missing-player/position rejections through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R145

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized unknown map-transfer rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `transfer_map_requires_player_on_transfer_bounds` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R144

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 unknown-command rejection through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R143

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 inactive-trade rejections through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R142

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 `auction.buy` / `auction.cancel` missing-id rejections through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R141

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 `mail.claim` / `mail.delete` missing-id rejections through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_social_group_guild_mail_persist_across_reload` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R140

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 `trade.offerGold` missing-amount rejection through Crystal `server.InvalidPacketReceived` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidPacketReceived`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R139

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 hero-behaviour missing-hero rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R138

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 event-spawn missing-template rejection through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R137

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 guild creation success chat through Crystal `server.SuccessfullyCreatedGuild` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.SuccessfullyCreatedGuild`; focused `stage5_social_group_guild_mail_persist_across_reload` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R136

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 craft no-ore rejection through Crystal `server.CraftingAttemptFailed` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.CraftingAttemptFailed`; focused `stage5_conquest_event_hero_mining_and_crafting_flow` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R135

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 credit-shop insufficient-credit rejection through Crystal `server.YouDontHaveEnoughCurrency` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.YouDontHaveEnoughCurrency`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R134

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 mail/trade/auction missing-entity rejection chats through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_` (26/26); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638). |

## Completed Round: 2026-04-26-R133

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket metadata-missing rejection chat through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_item_` (16/16); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (636/636). |

## Completed Round: 2026-04-26-R132

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket/seal missing-equipped-item rejection chats through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_item_` (15/15); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (635/635). |

## Completed Round: 2026-04-26-R131

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket/seal missing-source rejection chats through Crystal `server.NotFound` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.NotFound`; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-26-R130

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only ordinary map-transfer success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal packet surface: ordinary map transfers now emit `MapInformation` and `UserLocation` without generic `"Transferred to ..."` chat; focused `transfer_map` (2/2); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-26-R129

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 socket/seal invalid-source rejection chats through Crystal `server.InvalidCombination` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.InvalidCombination`; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-26-R128

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 gold-shop purchase chat through Crystal `server.BoughtItemForGold` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.BoughtItemForGold`; focused `stage5_trade_shop_and_auction_are_transactional` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R127

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only harvest success chat from transferred harvest-drop success | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal surface: successful harvest transfer now emits `GainedItem` plus `ObjectHarvested` without generic `"Harvested ..."` chat; focused/broader `harvest` (8/8); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R126

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized expanded-storage expiry notice through Crystal `server.ExpandedStorageExpired` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.ExpandedStorageExpired`; focused `expired_expanded_storage_tick_emits_resize_notice_once_and_persists_flag` (1/1); broader `storage` (43/43); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R125

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 item socket/seal success chats through Crystal `server.ItemSocketsIncreased` and `server.ItemSealedFor` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains both keys; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R124

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 item-seal reseal-delay rejection through Crystal `server.ItemCannotBeResealedFor` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.ItemCannotBeResealedFor`; focused `stage5_item_seal_rejects_before_next_seal_date_after_expiry` (1/1); broader `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R123

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 credit-shop purchase chat through Crystal `server.BoughtItemForCredit` while preserving mailbox delivery | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.BoughtItemForCredit`; focused `stage5_credit_shop_mails_purchase_and_claim_transfers_attachment` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R122

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 successful trade completion through Crystal `server.TradeSuccessful` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.TradeSuccessful`; focused `stage5_trade_shop_and_auction_are_transactional` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R121

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 trade/shop/auction low-gold rejection messages through Crystal `server.LowGold` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.LowGold`; focused `stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R120

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized direct ground-drop pickup full-bag rejection through Crystal `server.YouCannotCarryAnymore` while preserving current-cell skip semantics | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.YouCannotCarryAnymore`; focused `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R119

| Status | Task | Owner | Files | Notes |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 mail, shop, auction, and craft full-bag rejection messages through Crystal `server.YouCannotCarryAnymore` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains `server.YouCannotCarryAnymore`; focused `stage5_shop_and_auction_full_bag_preserve_gold_and_items` (1/1); broader `stage5_` (22/22); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R118

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Localized Stage 5 item socket max-capacity and already-sealed rejection messages through Crystal `server.ItemMaxSockets` and `server.ItemAlreadySealed` keys | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains both server text keys; focused `stage5_item_` (13/13); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R117

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Localized harvest no-drop and full-bag messages through Crystal `server.NothingWasFound` and `server.YouCannotCarryAnymore` while preserving pending-drop retry and `ObjectHarvested` timing | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: generated localization bundle contains both server text keys; focused `harvest` (8/8); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R116

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Localized owner-blocked pickup rejection through Crystal `server.CannotPickupNotOwner` while preserving owner window, group-owner bypass, and scan-skip behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `PlayerObject.PickUp` emits `ServerTextKeys.CannotPickupNotOwner` only when no later pickable current-cell candidate exists; focused `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R115

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only normal pickup success chat so item and gold pickup success follows Crystal packet/chat surface while preserving `ShowGroupPickup` group notices | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `PlayerObject.PickUp` gains items/gold and returns without normal success chat; focused `pickup` (14/14); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R114

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal `NoDrug` map-rule rejection for static starter and dynamic manifest-backed potion `UseItem` so blocked maps emit `server.YouCannotUsePotionsHere`, fail ack, preserve items, and avoid HP/MP queueing | Coordinator | `apps/simulation/src/config.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `HumanObject.CanUseItem` rejects `ItemType.Potion` on `CurrentMap.Info.NoDrug` with `ServerTextKeys.YouCannotUsePotionsHere`; focused `no_drug` (2/2); adjacent `use_item_packet_` (42/42); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633). |

## Completed Round: 2026-04-25-R113

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Aligned static starter HP/MP potion use with Crystal normal-potion timed recovery so successful use consumes and acks immediately but restores HP/MP on follow-up ticks via `ObjectHealth` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: `PlayerObject.UseItem` `ItemType.Potion` shape `0` queues `PotHealthAmount` / `PotManaAmount`, while shape `1` is the immediate `SunPotion` branch; focused `crystal_use_item_packet_consumes_` (2/2); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R112

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only static `repair-powder` success/failure chat so starter equipment repair use preserves repair mutation and `ItemRepaired` packets without extra generic chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: no Crystal `UseItem` branch emits the starter `sim.noEquipmentNeedsRepair` / `sim.repairedEquippedItems` messages; focused `repair_powder` (2/2); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R111

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only static `town-teleport` success chat so successful teleport use emits movement/location packets without generic success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal evidence: existing dynamic Crystal town-teleport path and source-audited `NoTownTeleport` gating have no success-side chat; focused `town_teleport` (3/3); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R110

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed hardcoded static `benediction-oil` no-weapon failure chat so invalid weapon-luck attempts fail without runtime-only chat or item consumption | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.UseItem` case 3 enqueues failed `UseItem` when `TryLuckWeapon()` returns false; `HumanObject.TryLuckWeapon` only chats after a valid outcome; focused `benediction_oil` (4/4); adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631). |

## Completed Round: 2026-04-25-R109

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `SplitItem` success chat so inventory/storage splits emit Crystal-shaped `SplitItem1` plus `SplitItem` packets without extra chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.SplitItem` success enqueues `S.SplitItem1` and `S.SplitItem` only; focused `split_item_packet` (7/7); focused `storage_split_item_stack_creates_new_storage_slot`; adjacent `storage` (43/43); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (630/630). |

## Completed Round: 2026-04-25-R108

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Aligned static `repair-oil` / `war-god-oil` with Crystal's localized weapon-repair hint surface and removed the runtime-only failure chat/no-repair message | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.UseItem` scroll shape `4`/`5` silently failed-acks when no weapon repair is possible and emits `WeaponPartiallyRepaired` / `WeaponCompletelyRepaired` hint plus `ItemRepaired` on success; focused `cargo +1.89.0 test --locked -p mir2-simulation repair_oil -- --test-threads=1 --nocapture` (3/3); focused `repair_and_war_god_oil_emit_item_repaired_for_weapon`; adjacent `use_item_packet_` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (630/630). |

## Completed Round: 2026-04-25-R107

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `custom.itemDropped` from successful `DropItem` so normal and split-stack inventory drops return success ack plus ground-object visibility without generic success chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.DropItem` only chats for `NoThrowItem` and success ends with `p.Success = true; Enqueue(p);` without success chat; adjacent `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture` (10/10); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629). |

## Completed Round: 2026-04-25-R106

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.usedItem` from the static HP/MP consumable `UseItem` success path so inventory/belt starter potions heal, consume, and ack success without chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.UseItem` potion shape `0`/`1` queues restore or changes HP/MP without normal success chat; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_use_item_packet_consumes_inventory_slot -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_use_item_packet_consumes_belt_slot -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629). |

## Completed Round: 2026-04-25-R105

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.itemNotFoundInBag` from missing-source `DropItem` so absent inventory ids now return only the failed `DropItem` ack | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `PlayerObject.DropItem` enqueues the failed `S.DropItem` for missing item/count failures without chat; focused `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet_missing_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture` (10/10); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629). |

## Completed Round: 2026-04-25-R104

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Changed unmodeled `UseItem(grid=HeroInventory)` from an empty response to a Crystal-shaped failed `UseItem` ack while preserving the existing no-fallback/no-mutation behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source cross-check: `MirConnection.UseItem` routes `HeroInventory` to `HeroObject.UseItem`, which starts with `S.UseItem { Grid = HeroInventory, Success = false }`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (628/628). |

## Completed Round: 2026-04-25-R103

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.itemNotFoundInBag` from missing-item and invalid-source `UseItem` failures so missing inventory ids now failed-ack without chat or mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_missing_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (628/628). |

## Completed Round: 2026-04-25-R102

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.itemNoActiveUse` from the final unusable inventory `UseItem` fallback so unknown/unusable items now failed-ack without chat or mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_unusable_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (39/39); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (627/627). |

## Completed Round: 2026-04-25-R101

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed the literal runtime-only non-inventory equipment `UseItem` failure chat so belt-sourced equipment attempts now failed-ack without chat or mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_belt_equipment_rejects_without_runtime_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (38/38); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (626/626). |

## Completed Round: 2026-04-25-R100

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Removed runtime-only `sim.equippedItem*` chat from the successful `UseItem` equipment path so the modeled success surface stays ack/refresh/equipment-state only, matching Crystal's explicit equip packet surface | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_equipping_need_identify_item_emits_refresh_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (37/37); adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (13/13); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (625/625). |

## Completed Round: 2026-04-25-R99

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Locked the positive explicit `EquipItem` path for dynamic manifest-backed equipment when Crystal requirements are met, using `SpiritRing` at required level 15 into the right ring slot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_equipment_allows_when_requirements_are_met -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (13/13); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (625/625). |

## Completed Round: 2026-04-25-R98

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Locked dynamic manifest-backed `CreditToken3` `UseItem` coverage for credit gain, localized `server.CreditsAddedToAccount` hint, success ack, and item consumption | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_credit_token_emits_localized_hint_chat -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (37/37); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (624/624). |

## Completed Round: 2026-04-25-R97

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Locked `EquipItem(grid=Storage)` coverage for dynamic manifest-backed equipment requirement rejection so storage-sourced items fail ack-only, preserve storage state, and do not equip when Crystal requirements are unmet | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_storage_manifest_equipment_rejects_unmet_requirements_silently -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (12/12); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (623/623). |

## Completed Round: 2026-04-25-R96

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal `CanEquipItem` requirement gating for explicit `EquipItem` on dynamic manifest-backed equipment: gender/class/required-type failures now silently fail before mutation like Crystal, while legacy fixture aliases keep existing test behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_equipment_rejects_unmet_requirements_silently -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (11/11); adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_ -- --test-threads=1 --nocapture` (2/2); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (622/622). |

## Completed Round: 2026-04-25-R95

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added explicit regression coverage for Crystal `CanEquip` compatibility where manifest-backed `ItemType.Amulet` can target the right bracelet slot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_amulet_can_target_right_bracelet_slot -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (10/10). |

## Completed Round: 2026-04-25-R94

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Wider validation pass after R89-R93 item/equipment parity changes | Coordinator | docs | `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture` (218/218); `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture` (42/42); `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (620/620). |

## Completed Round: 2026-04-25-R93

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Fixed explicit `EquipItem` target-slot compatibility for manifest-backed ring/bracelet equipment: imported item type compatibility now allows rings in either ring slot and bracelets in either bracelet slot while preserving `UseItem` default slot behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_ring_and_bracelet_can_target_right_slots -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (9/9). |

## Completed Round: 2026-04-25-R92

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Matched Crystal `ResurrectionScroll` revive vitals by restoring modeled MP to the current runtime cap when a dead player revives, alongside existing full HP revive and consume behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dead_player_resurrection_scroll_revives_and_consumes_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36). |

## Completed Round: 2026-04-25-R91

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal repair-bind rejection to manifest-backed `RepairOil` / `WarGodOil`: equipped weapon `DontRepair` blocks repair oils and `NoSRepair` also blocks full/special `WarGodOil`, preserving item and weapon durability on failure | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_repair_oils_respect_weapon_repair_binds -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36). |

## Completed Round: 2026-04-25-R90

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added Crystal `CanUseItem` map-rule rejection for manifest-backed scroll shape `0/2`: `NoEscape` blocks `DungeonEscape` / `TeleportHome` with `server.CanNotDungeon`, and `NoRandom` blocks `RandomTeleport` with `server.CanNotRandom`, preserving item and position on failure | Coordinator | `apps/simulation/src/config.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_dungeon_escape_rejects_on_no_escape_map -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_random_teleport_rejects_on_no_random_map -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (35/35). |

## Completed Round: 2026-04-25-R89

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Mapped manifest-backed Crystal equipment item types to runtime `EquipmentSlot` for item gain, test helpers, and `UseItem` fallback, removing test-only manual slot setup for current manifest equipment use | Coordinator | `apps/simulation/src/runtime.rs`, docs | Focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_ -- --test-threads=1 --nocapture` (2/2); adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33). |

## Completed Round: 2026-04-25-R88

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implemented manifest-backed `UseItem` pending timed-recovery behavior for normal potion `shape 0`, using modeled `pending_pot_health_amount` / `pending_pot_mana_amount` fields and world-tick drain emissions without immediate HP/MP mutation or hint chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_normal_potion_queues_timed_restore -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33). |

## Completed Round: 2026-04-25-R87

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expand manifest-backed `UseItem` `ItemType.Food` mount-feed branch for `RawMeat`/`LeanMeat`, including equipped-mount requirement, full-dura guard, success consume/emit behavior, and Crystal-style `ItemRepaired` / `server.MountFed` hints | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_food_requires_equipped_mount -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_food_feeds_equipped_mount -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (32/32) |

## Completed Round: 2026-04-25-R86

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expand manifest-backed current `UseItem` for `DungeonEscape`/`TeleportHome` and `RandomTeleport` scroll-shape `0/2` with same-map occupiable destination search and bounded success/failure behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_dungeon_escape_teleports_same_map -- --test-threads=1 --nocapture` (9/9); focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_random_teleport_teleports_same_map -- --test-threads=1 --nocapture` (30/30); adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_ -- --test-threads=1 --nocapture` |

## Completed Round: 2026-04-25-R85

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expanded `UseItem` `CanUseItem` parity beyond the R82 level-only requirement by adding modeled stat gates for `MaxAC` / `MaxMAC` / `MaxDC` / `MaxMC` / `MaxSC`, `MinAC` / `MinMAC` / `MinDC` / `MinMC` / `MinSC`, and `MaxLevel` from existing modeled equipment/buff totals | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `Crystal/Server/MirObjects/HumanObject.cs::CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_rejects_low_max_dc_requirement -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_allows_modeled_max_mc_requirement -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_ -- --test-threads=1 --nocapture` |

## Completed Round: 2026-04-25-R84

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Corrected manifest-backed `UseItem` shape-26/27 branch for `GtInvite` and `GTTeleport` so `CanUseItem` pass now consumes once with `UseItem` success ack only, no chat, and no `UserLocation`/teleport side effect while leaving `GTTeleport` guild-territory behavior to NPC script paths | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_gt_invite_consumes_without_active_effect -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_gt_teleport_consumes_without_teleporting -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check` |

## Completed Round: 2026-04-25-R83

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Remaining manifest-backed item-use small surface completed for `AncientBanga[Green]` / `AncientBanga[Purple]`, map/server shout flags, Crystal hint chat, and credit-token usage hint localization | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R82

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `CanUseItem` parity for current subset (`Gender`, `Class`, `RequiredType==Level`, repeated skill-book learn block, and successful skill-book learn consume) | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R81

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Dynamic manifest-backed current-data `UseItem` now routes Crystal `SunPotion`, duration buffs, `TownTeleport`, `BenedictionOil`, `RepairOil`, and `WarGodOil` through template stats and scroll shapes, including Crystal-style same-key buff duration stacking and the current `WarGodOil` shape-0 name fallback | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem` plus `MapObject.AddBuff`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R80

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current equipment/item metadata now preserves Crystal `NeedIdentify` and `SoulBoundId` through runtime/item payload round-trips, auto-identifies items on equip/use-equip, and rejects equipping items soul-bound to another character | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.EquipItem` / `PlayerObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_equipping_need_identify_item_emits_refresh_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R79

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MysteryWater` plus cursed current-equipment semantics now match Crystal's bounded runtime surface: first use unlocks and consumes, repeat use hint-chats without consuming, cursed current `RemoveItem` and replacement `EquipItem` require the unlock, successful cursed removal/replacement clears it again, and storage-grid replacement rejects replaced equipment that cannot be stored | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem`, `PlayerObject.EquipItem`, and `PlayerObject.RemoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R78

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `RemoveSlotItem` now follows Crystal's bounded source-grid envelope for the modeled runtime: invalid `grid=Equipment` requests and unmodeled `Mount` / `Fishing` / `Socket` slot-item requests ack-fail without falling through into whole-equipment removal, including socket requests that only match the parent equipment id | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.RemoveSlotItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R77

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `EquipItem(grid=Storage)` now resolves the exact storage item through the active `@Storage` service, and current `RemoveItem(grid=Inventory|Storage)` now follows Crystal's exact destination-slot semantics with ack-only packet shape instead of accepting `grid=Equipment` or falling back into another bag slot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.EquipItem` / `PlayerObject.RemoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_ -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_item_packet_ -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R76

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Expired expanded storage now downgrades to inactive on current `StartGame`, then emits Crystal-style expiry chat plus `ResizeStorage` on the first world tick and persists the account flag back to `false` while preserving the 160-slot backing array | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject` expanded-storage expiry / `BuildUserInformation`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R75

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `@Storage` open now sends Crystal `UserStorage` with the full backing storage length even when expanded storage is no longer active, while higher-slot storage actions remain gated by current accessible capacity | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SendStorage` / `AccountInfo.IsValidStorageIndex`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R74

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Repeated unchanged current `@Storage` opens now suppress duplicate `UserStorage` after the first send, matching Crystal `Connection.StorageSent` resend behavior while preserving the locked reopen/unlock resend path | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `NPCScript.StorageKey` / `PlayerObject.SendStorage` / `MirConnection.UnlockStorage`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; `git -C mir2-web3 diff --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R73

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Successful current `@Storage` open now emits Crystal `UserStorage` before `NPCStorage` when storage is available, and successful `UnlockStorage` now emits `StorageUnlockResult` followed by `UserStorage`, through protocol/gateway/runtime with focused regressions | Coordinator | `packages/protocol/src/ids.rs`, `packages/protocol/src/packets.rs`, `packages/protocol/src/trace.rs`, `packages/protocol/tests/codec.rs`, `apps/gateway/src/web.rs`, `apps/web/app/page.tsx`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `NPCScript.StorageKey` / `PlayerObject.SendStorage` / `MirConnection.UnlockStorage`; focused `cargo +1.89.0 test --locked -p mir2-protocol --test codec`; focused `cargo +1.89.0 test --locked -p mir2-gateway`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R72

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Reopening Crystal `@Storage` now resets the session unlock state before deciding whether storage contents can be sent, matching `ResetStorageUnlock()` and blocking stale unlocked sessions | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `NPCScript.StorageKey`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`; `git -C mir2-web3 diff --check` |

## Completed Round: 2026-04-25-R71

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current storage password set/unlock/remove now enforce Crystal's `^[A-Za-z0-9]{5,15}$` password format semantics instead of accepting runtime-only values | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for storage password validation; focused `cargo +1.89.0 test --locked -p mir2-simulation storage_password -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R70

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current storage password actions now require the active in-range Crystal storage service context, and successful password removal clears `LastSetTime` back to `0` like Crystal | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current storage password handlers; focused `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R69

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current inventory-grid `CombineItem` current-data coverage now closes the remaining present-data shape-3/4 families and the shape-0 ack-only source surface for the current manifest slice | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.CombineItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation combine_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R68

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current inventory-grid `CombineItem` now routes current-data `DurabilityGem` / `DurabilityOrb` through Crystal's `MaxDura` branch instead of misusing stat `48` as the applied upgrade stat, and focused regressions now lock the current-data durability, attack-speed, magic-resist, and durability-cap surfaces | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.CombineItem` / `GetGemType` / `GetCurrentStatCount`; focused `cargo +1.89.0 test --locked -p mir2-simulation combine_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-25-R67

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `BuyItem`, `SellItem`, and `RepairItem`/`SRepairItem` now require the recorded Crystal NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range service context no longer mutates the implemented current NPC buy/sell/repair item surfaces | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current NPC item-service handlers; focused `cargo +1.89.0 test --locked -p mir2-simulation buy_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation sell_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation repair_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R66

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current storage-family item actions now require the recorded Crystal storage NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range storage service context now ack-fails across `StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.StoreItem` / `TakeBackItem` / `MoveItem` / `SplitItem` / `MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_storage_service_context_rejects_storage_actions_when_player_leaves_data_range -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation storage_service_context_requires_live_npc_object -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R65

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `SplitItem` now matches Crystal's supported-grid and failed-ack surface: only `Inventory` / `Storage` are live, `Storage` requires active Crystal storage service, and unsupported/invalid/full/locked failures stay ack-only | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SplitItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R64

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `SplitItem(grid=Inventory)` now follows Crystal single-array placement across local `Bag1` / `Bag2`, including belt-first placement for belt-eligible items instead of source-container page scoping | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SplitItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R63

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Slot-based current `MoveItem` / `StoreItem` / `TakeBackItem` inventory paths now resolve Crystal single-array indices across local `Bag1` / `Bag2`, including `Bag2` swaps and storage transfers on slots `40+` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem` / `PlayerObject.StoreItem` / `PlayerObject.TakeBackItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation crystal_inventory_index_for_bag2 -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R62

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Remaining unsupported `MergeItem` `Storage <-> Belt` cross-grid requests now follow Crystal's ack-only surface without runtime-only chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R61

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` now rejects `QuestInventory` requests ack-only without extra chat or quest-item mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R60

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` now rejects `Belt` / `QuestInventory` requests ack-only, enforces current inventory slot bounds, and keeps bag moves from mutating quest items | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R59

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current missing-source `MoveItem` Inventory/Storage failures now use Crystal's `ItemMoveErrorReport` chat surface before the failed ack instead of `sim.itemNotFoundInBag` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture` plus new missing-source move regressions; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R58

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current successful `MoveItem` current `Inventory` and `Storage` paths now follow Crystal's ack-only surface, removing the runtime-only `Item slot updated.` chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R57

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem(grid=Storage)` now requires active Crystal `@Storage` / `NPCStorage` service context, with ack-only inactive-service failures | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R56

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` storage-lock and invalid-slot failures now follow Crystal's ack-only surface without extra chat | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture` plus new storage-lock/invalid-slot move regressions; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R55

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` unsupported-grid parity now also covers `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player/equipment mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R54

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` now supports the next bounded modeled cross-grid surface via `Inventory <-> Belt` stack merges for Crystal belt-eligible items, with ack-only non-beltable failures | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem` plus local belt-model audit; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R53

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` now supports Crystal-style `Inventory <-> Storage` stack merges through the active storage-service gate, with ack-only inactive/locked failures | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R52

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` same-grid failure/success message shape now follows Crystal's ack-only surface for current Inventory/Storage paths | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R51

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` unsupported-grid parity now also covers `Trade` and `Refine` ack-only failures without extra chat or player-bag mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R50

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MergeItem` unsupported-grid parity now also covers `HeroInventory`, `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player-bag mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R49

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Current `MoveItem` unsupported-grid parity now covers `HeroInventory`, `Trade`, and `Refine` ack-only failures without extra chat or player-bag mutation | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R48

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `MoveItem(grid=HeroInventory)` failed-ack without extra chat or player-bag mutation while hero inventory is unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MoveItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R47

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `MergeItem` hero-grid requests failed-ack without extra chat or player-bag mutation while hero inventory/equipment are unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.MergeItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R46

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `EquipItem(grid=HeroInventory)`, `RemoveItem(grid=HeroInventory)`, and `RemoveSlotItem(grid=HeroEquipment|HeroInventory)` failed-ack without mutating matching player inventory/equipment while hero grids are unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.EquipItem`, `PlayerObject.RemoveItem`, and `PlayerObject.RemoveSlotItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_item_packet_hero_inventory_grid_does_not_mutate_matching_player_equipment -- --test-threads=1 --nocapture`; focused `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item_packet_hero_equipment_grid_does_not_mutate_matching_player_equipment -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation equip_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Blocked Follow-up: 2026-04-24-R39

Restart note: R39 remains blocked and uncounted. Keep the runtime/game-data/tooling scaffolding, but do not count the data import until the real Crystal DB and routes are available locally.

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [!] | Promote Crystal map `NoThrowItem` / `NoDropPlayer` / `NoDropMonster` flags into generated respawn/map data and switch runtime off config-only overrides | Coordinator | `packages/tooling/scripts/generate-crystal-respawn-manifest.mjs`, `packages/game-data/src/lib.rs`, generated respawn manifest, `apps/simulation/src/runtime.rs`, docs | Crystal `MapInfo` save-layout audit; regenerate `packages/game-data/data/generated/crystal_respawn_manifest.json`; `cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1`; focused `cargo +1.89.0 test --locked -p mir2-simulation no_drop_monster_map_rule -- --test-threads=1 --nocapture`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

Blocked note:

- The runtime/game-data/tooling scaffolding is in place, but this Mac did not have the expected local Crystal build asset path `Crystal/Build/Server/Debug/Server.MirDB` (and corresponding `Envir/Routes`) when the generator was invoked.
- Do not mark R39 complete until the manifest is regenerated from real Crystal data and the runtime is reverified.

## Completed Round: 2026-04-24-R45

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `SplitItem(grid=HeroInventory)` no longer falls back into player inventory when hero inventory is unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.SplitItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation split_item_packet_hero_inventory_grid_does_not_mutate_matching_player_stack -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R44

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `UseItem(grid=HeroInventory)` no longer falls back into player inventory when hero inventory is unmodeled | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `MirConnection.UseItem`, `PlayerObject.HeroUseItem`, and `HeroObject.UseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R43

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `ResurrectionScroll` map `NoReincarnation` rejection for dead current players | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | Crystal source audit for `PlayerObject.UseItem` shape-6 and `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dead_player_resurrection_scroll_rejects_on_no_reincarnation_map -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation town_teleport -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R42

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `TownTeleport` map `NoTownTeleport` rejection for current `UseItem` | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | Crystal source audit for `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation town_teleport -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R41

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal dead-state `UseItem` parity for ordinary items plus alive/dead `ResurrectionScroll` behavior | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.UseItem` shape-6 and `HumanObject.CanUseItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R40

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal dead-state current item mutation family for `BuyItem` / `DeleteItem` / `SellItem` / `RepairItem` / `DropItem` / `CombineItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current dead-player item/service branches; focused `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`; adjacent current item/service packet tests; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R38

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal monster-drop map `NoDropMonster` suppression for normal kills, field-wasp quest drop, and harvest loot | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `MonsterObject.Drop` / `DropItem` and harvest paths; focused `cargo +1.89.0 test --locked -p mir2-simulation no_drop_monster_map_rule -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation harvest -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation drop -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-24-R37

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DropItem` map `NoThrowItem` rejection and `CanNotDrop` message parity | Coordinator | `apps/simulation/src/runtime.rs`, map metadata/config if needed, docs | Crystal source audit for `PlayerObject.DropItem` map-flag branch; focused `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R36

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DropItem` rejects rental `BindingFlags.DontDrop` ack-only | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `PlayerObject.DropItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R35

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal bounded hero-inventory packet guard audit for current `DropItem` / `CombineItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for hero-inventory packet routing; focused `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R34

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DeleteItem` ignores packet `HeroInventory` and still deletes matching player inventory by unique id | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `MirConnection.DeleteItem` / `PlayerObject.DeleteItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation delete_item_packet -- --test-threads=1 --nocapture`; adjacent `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R33

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal current item packet unique-id cleanup for `UseItem`, `EquipItem`, and `MergeItem` | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for current item packet unique-id usage; focused `cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture`; `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R32

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal current inventory unique-id cleanup for `CombineItem` and current bag item packet lookups | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit for `CombineItem`, `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, `RepairItem`; focused `cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture`; `cargo +1.89.0 fmt --check`; full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R31

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal player `GemRatePercent` for current inventory-grid `CombineItem` upgrade chance | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused `GemRatePercent` upgrade regression, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet_upgrade_branch_applies_player_gem_rate_percent_bonus -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R30

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal rental binding flags for current storage and combine item paths | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused rental `DontStore` / `DontUpgrade` regressions, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R29

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal inventory-grid `CombineItem` repair-hammer and sewing parity | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused repair packet regressions, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R28

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `CombineItem` target item-type gating across packet branches | Coordinator | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused socket/seal packet rejection regressions, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R27

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal inventory-grid `CombineItem` shape-3/4 gem/orb upgrade parity | Coordinator | `packages/protocol/src/ids.rs`, `packages/protocol/src/packets.rs`, `packages/protocol/src/trace.rs`, `packages/protocol/tests/codec.rs`, `apps/gateway/src/web.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit, protocol/gateway/runtime `ItemUpgraded` coverage, persisted `gem_count` flow-through, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-protocol item_slot_seal_and_upgrade_server_packets_use_crystal_ids -- --nocapture`, `cargo +1.89.0 test -p mir2-gateway item_slot_and_seal_server_events_expose_crystal_payload_fields -- --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R26

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `CombineItem` packet parity for current socket/seal branches | Coordinator | `packages/protocol/src/ids.rs`, `packages/protocol/src/packets.rs`, `packages/protocol/src/trace.rs`, `packages/protocol/tests/codec.rs`, `apps/gateway/src/web.rs`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit, protocol/gateway/runtime `CombineItem` coverage, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-protocol item_and_combat_client_packets_use_crystal_payloads -- --nocapture`, `cargo +1.89.0 test -p mir2-protocol item_action_ack_server_packets_use_crystal_ids -- --nocapture`, `cargo +1.89.0 test -p mir2-gateway combine_item_server_event_exposes_crystal_payload_fields -- --nocapture`, `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R25

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal storage item flag/rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, `apps/simulation/Cargo.toml`, docs | Crystal source audit, `NPCStorage` service-context activation, end-to-end `@Storage` store/take-back regression, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`, `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`, full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-23-R24

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `SellItem` item flag/type rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused sell rejection tests, `cargo test -p mir2-simulation sell`, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R23

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal repair service rejection/cost semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused repair rejection tests, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R22

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal NPC BuyItem rejection edge semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused buy rejection tests, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R21

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal sell/game-shop/mail rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | Crystal source audit, focused sell/credit-shop/mail tests, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R20

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal harvest owner/EXPOwner scan rejection semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused owner-rejected/group-member corpse tests, `cargo test -p mir2-simulation harvest`, `cargo test -p mir2-simulation drop`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R19

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal HarvestMonster transfer timing and leftover inventory semantics | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused Hen/Deer/pass-count/pending-drop tests, `cargo test -p mir2-simulation harvest`, `cargo test -p mir2-simulation drop`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R18

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal drop visibility and pickup rejection edges | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, focused owner/full-bag/overweight pickup tests, `cargo test -p mir2-simulation pickup`, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation harvest`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R17

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `GROUP` drop semantics | Coordinator + Explorers | `packages/tooling`, `packages/game-data`, `apps/simulation/src/runtime.rs`, docs | Crystal source audit, generated drop parser tests, focused group-drop tests, `cargo test -p mir2-game-data`, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R16

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Data-driven `RandomItemStats.ini` manifest import | Coordinator + Worker | `packages/tooling`, `packages/game-data`, `apps/simulation/src/runtime.rs`, docs | generated manifest tests, focused random-stat tests, `cargo test -p mir2-game-data`, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation item`, full `mir2-simulation` regression |

## Completed Round: 2026-04-22-R15

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Full random-stat family source mapping and runtime payload baseline | Coordinator + Explorers | `apps/simulation/src/runtime.rs`, docs | Crystal source audit, `cargo fmt --check`, focused random-stat/persistence tests, `cargo test -p mir2-simulation drop`, `cargo test -p mir2-simulation item`, `cargo test -p mir2-simulation -- --test-threads=1` |

## Completed Round: 2026-04-22-R14

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Seal reseal-delay metadata baseline | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs`, docs | `cargo fmt --check`, focused seal tests, `cargo test -p mir2-simulation item`, legacy save test |

## Completed Round: 2026-04-22-R13

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Socket source gem validation baseline | Coordinator + Explorer | `apps/simulation/src/runtime.rs`, docs | `cargo fmt --check`, focused socket tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R12

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Seal source item validation baseline | Coordinator | `apps/simulation/src/runtime.rs`, docs | `cargo fmt --check`, focused seal tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R11

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Frontend scene target keyboard action chain | Coordinator | `apps/web/app/original-client-shell.tsx`, docs | `npm.cmd run build --prefix apps\web` |

## Completed Round: 2026-04-22-R10

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement BenedictionOil curse/no-effect branches | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused BenedictionOil tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R9

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement seal already-sealed validation first stage | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused seal tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R8

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement socket slot-capacity validation first stage | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused socket tests, `cargo test -p mir2-simulation item` |

## Completed Round: 2026-04-22-R7

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Select next backend/frontend parity bite from explorer findings | Coordinator + Explorers | docs | R7 selected NPC buy-back / used-goods parity |
| [x] | Implement NPC buy-back persistence, expiry, and used-goods baseline | Coordinator | `apps/simulation/src/runtime.rs`, `apps/simulation/src/config.rs` | `cargo fmt --check`, focused buy-back tests, `cargo test -p mir2-simulation sell`, `cargo test -p mir2-simulation npc` |

## Completed Round: 2026-04-22-R6

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Added-stat ground item display investigation | Coordinator | none | Crystal `ItemObject` / Rust packet/render map |
| [x] | Implement added-stat cyan ground item display baseline | Coordinator | `apps/simulation/src/config.rs`, `apps/simulation/src/runtime.rs`, `apps/web/app/page.tsx`, `apps/web/app/original-client-shell.tsx` | `cargo fmt --check`, focused colour tests, `cargo test -p mir2-simulation drop`, `npm.cmd run build --prefix apps\web` |

## Completed Round: 2026-04-22-R5

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal random-stat source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust item-stat/import implementation investigation | Rust Explorer | none | bounded implementation map |
| [x] | Implement current random-stat roll baseline | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused random/drop/harvest tests |

## Completed Round: 2026-04-22-R4

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Implement frontend login/select/game shell first patch | Frontend Worker | `apps/web/app/original-client-shell.tsx` | `npm.cmd run build --prefix E:\mir2\mir2-web3\apps\web` |
| [x] | Review and integrate frontend shell patch | Coordinator | docs and frontend queue | build verified locally |

## Completed Round: 2026-04-22-R3

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal quest-drop `Q` gating source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust quest/drop implementation investigation | Rust Explorer | none | function/test map |
| [x] | Frontend shell first-patch investigation | Frontend Explorer | none | bounded write-set recommendation |
| [x] | Implement backend Crystal quest-drop gating | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused drop/quest/harvest tests |

## Completed Round: 2026-04-22-R2

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `DropStackSize` / ground-drop position source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust ground-drop placement implementation investigation | Rust Explorer | none | function/test map |
| [x] | Implement backend Crystal `DropStackSize` and drop-position search | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused and broad drop tests |

## Completed Round: 2026-04-22-R1

| Status | Task | Owner | Write Set | Verification |
| --- | --- | --- | --- | --- |
| [x] | Crystal `AddItem` belt-priority source investigation | Crystal Explorer | none | source paths and behavior notes |
| [x] | Rust inventory/belt implementation investigation | Rust Explorer | none | function/test map |
| [x] | Frontend 1:1 acceptance matrix investigation | Frontend Explorer | none | QA matrix proposal |
| [x] | Implement backend Crystal `AddItem` belt-priority | Coordinator | `apps/simulation/src/runtime.rs` | `cargo fmt --check`, focused item gain/use/pickup tests |
| [x] | Create orchestration docs and Candidate workflow | Coordinator | `docs/AGENT-ORCHESTRATION.md`, `docs/AGENT-TASK-QUEUE.md`, `docs/AGENT-RUN-LOG.md`, `docs/PLAYER-QA-SCRIPT.md` | docs created |

## Backend Queue

| Status | Task | Notes |
| --- | --- | --- |
| [x] | Crystal `AddItem` belt-priority placement | Potion/Scroll/Script effect 1 -> belt 0..3, Amulet -> belt 4..5, fallback to bag, belt `UseItem` consumes belt slot. |
| [x] | Crystal ground-drop position search and `DropStackSize` | Current player item drops, player gold drops, and monster ground drops use Crystal `ItemObject.Drop(distance)` placement semantics. |
| [x] | Crystal quest-drop `Q` gating | `Q` entries now roll normally, route to active matching quest inventory, suppress ground fallback, and preserve full quest-inventory failures. |
| [x] | Random item stat generation | Current runtime rolls the full Jev profile family baseline for imported Crystal drop items from generated `RandomItemStats.ini` manifest data, including `MaxDura`, all supported `UserItemStat` families, curse flag, and socket slots; metadata survives pickup, harvest, equipment/inventory state, and save/reload. |
| [x] | Crystal `GROUP` drop semantics | Drop manifest entries can now preserve nested `GROUP`, `GROUP*`, and `GROUP^` trees, and runtime recursively applies Crystal group behavior: successful child gold accumulates, `GROUP*` keeps one successful item, `GROUP^` short-circuits after the first successful child, and nested group rules compose. |
| [x] | Crystal drop visibility and pickup rejection edges | Crystal source shows owned item/gold drops are broadcast immediately; owner windows restrict pickup only. Current `PickUp` scans the current cell, skips owner-blocked/full-bag/gold-cap candidates when later pickable drops exist, and treats bag weight as post-gain state instead of a pickup/harvest rejection gate. |
| [x] | Crystal HarvestMonster pending transfer semantics | Harvest monsters now generate and persist pending `_drops` after the configured skin count, transfer them on the next harvest call, preserve leftover drops when the bag cannot accept every item, and avoid re-rolling pending harvest rewards. |
| [x] | Crystal harvest owner/EXPOwner rejection | Harvest target scanning now skips corpses owned by another player unless the owner is in the configured group set, emits Crystal `NoNearbyOwnedCarcasses` only when no eligible corpse is found, and attaches current-player harvest ownership when a harvest monster is defeated. |
| [x] | Crystal NPC `BuyItem` rejection edges | `BuyItem` now silently rejects invalid panel/count, missing active NPC service, non-buy service pages such as `@Repair`, missing goods/metadata, insufficient gold, and full-bag purchases without mutating gold or inventory. |
| [x] | Crystal NPC `RepairItem` / `SRepairItem` rejection and cost edges | NPC repair now uses current backpack item unique ids, requires the matching active `@Repair` / `@SRepair` service page, applies Crystal repair/special-repair cost and normal max-dura loss semantics, emits `LoseGold` / `ItemRepaired` on success, and preserves Crystal message/silent rejection edges for non-repairable items, type mismatch, and insufficient gold. |
| [x] | Crystal NPC `SellItem` remaining rejection edges | `SellItem` now follows Crystal ack-only failures for zero count, missing service/item/count, `DontSell`, and partial-stack gold overflow; emits `CannotSellItemHere` only for script type mismatch; uses `UserItem.Price() / 2` style sale value; and preserves full-stack gold-cap clamping. |
| [x] | Crystal storage item flag/rejection edges | R25 now aligns `StoreItem` / `TakeBackItem` active `@Storage` / `NPCStorage` service context, `DontStore`/rental flags, password lock, accessible capacity, occupied-target no-swap behavior, and ack-only failure semantics. |
| [x] | Added-stat cyan ground item display | Current added-stat ground drops now surface Crystal Cyan through `ObjectItem.name_colour_argb`, world snapshots, and the web ground-drop label. |
| [x] | NPC buy-back expiry / used-goods persistence | Buy-back entries now persist across save/reload, carry Crystal 60-minute expiry, expire into NPC used goods, and used goods can be bought back through Buy/BuyUsed flows. |
| [~] | Full gem/socket validation | Socket slot-capacity validation, source gem validation, the real inventory-grid `CombineItem` packet path, shape-1/2/5/6 repair-hammer/sewing parity, bounded shape-3/4 gem/orb upgrade parity with `ItemUpgraded` / persisted `gem_count`, shared Crystal target-type gating, rental `DontUpgrade` rejection for current socket/upgrade combine branches, equipment-backed player `GemRatePercent` success bonus, current bag-item unique-id lookup cleanup, current item packet `UseItem` / `EquipItem` / `MergeItem` unique-id cleanup, Crystal `DeleteItem` hero-flag ignore semantics, and bounded current `DropItem` / `CombineItem` hero-inventory no-player-mutation guards are in. Broader hero-inventory handling and other gem-family branches remain. |
| [~] | Full seal-source validation | Already-sealed rejection, source item validation, reseal-delay metadata, save/reload, the real inventory-grid `CombineItem` packet path, and shared Crystal target-type gating are in. Hero-inventory handling and remaining shared combine-branch gaps remain. |
| [ ] | Map event script bindings | Import map event scripts, weather/lightning/fire/door/wall/gate behavior. |
| [ ] | Broader combat/skill parity | Spell tables, projectile objects, buff edge cases, live packet comparison. |

## Frontend Queue

| Status | Task | Notes |
| --- | --- | --- |
| [x] | Build frontend 1:1 acceptance matrix | Evidence Gate, panel matrix, and `docs/FRONTEND-1TO1-GAPS.md` are in place. |
| [~] | Login/select/game shell Crystal visual pass | First bounded patch landed: tile pointer double-dispatch guard and Enter-key login submit. Pixel/human comparison remains open. |
| [~] | Inventory/equipment/belt interaction parity | Belt slots 1-6, rotate, close, basic occupied/empty visual states, and hotkey `1` item use are smoke-verified; item drag/split/merge/drop/tooltips and inventory/equipment panel interactions remain. |
| [ ] | NPC dialog/shop/storage UI parity | Link flow, input pages, shop goods, repair/storage panels. |
| [~] | Combat HUD and target feedback parity | Selected-target keyboard approach/primary actions and localized action-distance feedback are in; HP/MP, attack feedback, object packets, and damage/struck display remain. |
| [ ] | Map/minimap interaction parity | Map switcher/debug isolation, minimap, safe-zone transfer flow. |
| [~] | Screenshot baseline pack | Desktop 1024x768 and compact 820x640 Stage 5 route screenshots are captured with manifest bounds; broader mobile/route coverage and Crystal comparison remain open. |

## Assets/Data Queue

| Status | Task | Notes |
| --- | --- | --- |
| [ ] | Event binding manifest | Map event scripts and referenced script validation. |
| [ ] | Full visual asset coverage audit | Missing sprites, effects, sounds, icons, UI source resources. |
| [ ] | Economy table import audit | Credit products, shop tables, refine/gem/seal probabilities. |
| [ ] | Full map metadata audit | Weather, fire, light, door/wall/gate/object state. |

## QA/Integration Queue

| Status | Task | Notes |
| --- | --- | --- |
| [ ] | Packet trace live Crystal fixture setup | Needs `MIR2_CRYSTAL_TCP_ADDR` and stable account fixtures. |
| [ ] | Representative local-vs-Crystal trace matrix | Login, start, move, combat, pickup, NPC, item, map transfer. |
| [~] | Stage screenshot comparison harness | Stage 5 smoke archives route screenshots plus named desktop/compact viewport metadata; true baseline diffing against Crystal/reference images remains open. |
| [ ] | 100% Candidate gate command bundle | Single local command list for backend, frontend, data, trace, load. |
| [ ] | Final human QA route | Keep under 40 hours by batching checks and evidence. |
