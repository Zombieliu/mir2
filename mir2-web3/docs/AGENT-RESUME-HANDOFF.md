# Agent Resume Handoff

> Latest product-evolution sync: 2026-04-27-R228 completed. GM system mail is now game-visible and claimable. Admin Web posts to Next `/api/admin/system-mail`, Rust Admin API writes command/audit records, then `AccountStoreSystemMailDomain` posts to gateway `POST /admin/system-mail` at `ADMIN_GATEWAY_MAIL_URL` and falls back to persistent account-store delivery if the gateway is down. Local smoke delivered mail to `Scout` with `deliveryMode: "gateway_live"`, observed it in the gateway WS world snapshot at `payload.stage5Systems.mail`, and claimed it via `stage5Command mail.claim`, raising gold from 1280 to 6280 and adding one `red-potion`.

> Latest product-evolution sync: 2026-04-27-R227 completed. Admin operations now has a working Rust API + Next Admin Web slice: `apps/admin-api` includes command/audit repository traits, in-memory command/audit stores, Axum routes, and `SendSystemMail` domain outbox execution; `apps/admin-web` includes Dashboard, Players, Player Detail, Economy, Activities, Servers, Risk, GM Tools, and Audit pages. GM mail writes are wired through Next `/api/admin/system-mail` to Rust `/admin/commands/send-system-mail`. Verification passed: `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`, `cargo +1.89.0 fmt --check`, admin-web `tsc --noEmit`, admin-web `next build`, direct Rust API curl write, Next proxy curl write, and Playwright screenshots `docs/admin-web-dashboard-smoke.png` / `docs/admin-web-gm-tools-smoke.png`.

> Latest sync: R225 completed. Mac-local Candidate regression is green again: web `tsc --noEmit`, direct `next build`, Stage 5 UI smoke (88 screenshots, manifest summary counts), map API smoke 18/18, minimap asset smoke 0 failures with known 450/451 warning, WS load 64/64, `mir2-game-data` 22/22, `mir2-gateway` 54/54, `mir2-simulation` 664/664, require-local packet trace matrix 9/9 local artifacts under `docs/generated/packet-traces/r225-matrix`, `cargo +1.89.0 fmt --check`, and `git diff --check`. Active follow-up round is R226. Truthful status split remains: automated evidence **100% Candidate**, backend/server tracked slice **99.70%**, real full-project accepted 1:1 **roughly 90.0%**.

> Latest sync: R224 completed. The local packet trace blocker is closed: `apps/gateway/src/bin/packet_trace.rs` is restored, `--list-flows` works, `mir2-gateway` passes 53/53 including packet trace bin tests 6/6, and require-local `packet_trace --matrix` wrote 9/9 TCP-traceable artifacts with `localOk=true` under `docs/generated/packet-traces/r224-matrix`. Truthful status split: automated evidence is **100% Candidate**, backend/server tracked slice remains **99.70%**, and real full-project accepted 1:1 remains **roughly 90.0%** until human Crystal visual/feel acceptance, live Crystal packet comparison, and blocked source-data decisions are closed. Active follow-up round is R225.

> Latest sync: R219-R222 completed. Frontend/global evidence advanced across login/select lifecycle, archived map API/minimap asset smoke JSON, refreshed WS load, compact multi-panel bounds, compact system-menu overflow fix, and NPC dialog link-capable rendering. Stage 5 UI smoke now captures 85 screenshots. Validation: web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke (85 screenshots), map API smoke 18/18, minimap asset smoke 0 failures with known 450/451 warning, WS load 64/64, `cargo +1.89.0 fmt --check`, and `git diff --check`. Active backend/global round is R223; backend/server parity estimate is 99.70%, whole-project 1:1 estimate is 90.0%.


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

Purpose: this file is the restart-safe handoff for continuing the autonomous Crystal / Mir2 1:1 push after the Codex session is closed, the machine is rebooted, or chat context is lost. A new session should not depend on prior chat history; read this file plus the queue/run-log docs and continue from the active round.

## Resume Order

1. Open `E:\mir2\mir2-web3`.
2. Read these files first:
   - `docs/AGENT-RESUME-HANDOFF.md`
   - `docs/AGENT-ORCHESTRATION.md`
   - `docs/AGENT-TASK-QUEUE.md`
   - `docs/AGENT-RUN-LOG.md`
   - `docs/CRYSTAL-1TO1-ROADMAP.md`
   - `docs/BACKEND-1TO1-PROGRESS.md`
   - `docs/WINDOWS-CONTINUATION.md`
   - `docs/POST-1TO1-EVOLUTION-PLAN.md`
   - `docs/TECH-MODERNIZATION-RFC.md`
   - `docs/PLATFORM-CLIENT-STRATEGY.md`
   - `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`
3. Treat `docs/AGENT-TASK-QUEUE.md` as the source of truth for the active round.
4. Continue autonomously from the active round. Do not repeat completed rounds unless tests or code inspection show a regression.
5. Use subagents only for clearly bounded parallel work. Keep one writer per high-conflict file.

## Current Checkpoint

- Active round: `2026-04-26-R226`
- Active task: Windows continuation / human acceptance / external-blocker follow-up after `R225` Mac-local regression refresh.
- Active round state: R39 manifest-backed map-flag import is still blocked because this Mac lacks `Crystal/Build/Server/Debug/Server.MirDB`; live Crystal trace comparison is blocked until `MIR2_CRYSTAL_TCP_ADDR` is configured. Automated Candidate evidence is refreshed through R225 with 88 Stage 5 screenshots, archived map/minimap JSON, WS load 64/64, web build/type checks, full Rust package regression, and local packet trace matrix evidence under `docs/generated/packet-traces/r225-matrix`.
- Last completed round: `2026-04-26-R225`
- Backend/server parity estimate: `99.70%`
- Whole-project automation status: `100.0% Candidate`
- Whole-project real accepted 1:1 estimate: `roughly 90.0%` until final human Crystal visual/feel acceptance, live Crystal trace comparison, and blocked source-data decisions are closed.
- Latest completed code work: R224 restored the `mir2-gateway` packet trace harness and local matrix artifacts. Backend gameplay code remains completed through `R82` through `R183` current item-use/equipment/pickup/harvest/map-transfer/drop-helper/storage-helper/trainer/movement/attack/interaction/casting/combat-chat/summon-chat/death-drop-chat/missing-defeated-entity/direct-pickup-invalid-target/npc-success-chat/direct-attack-invalid-target/direct-interact-invalid-target/npc-dialog-helper/stale-dialog/no-script-npc/move-item-fallback/cast-skill-failure/normal-chat/start-game-welcome/quest-required-drop/final-runtime-sim-namespace parity.
- Latest full backend verification: `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 664 tests, `cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1` passed with 22 tests, and `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` passed with 54 tests after R225.
- Latest frontend/global verification: web `tsc --noEmit`, direct `next build`, minimap smoke with `docs/generated/assets/latest-minimap-assets.json`, map API smoke with `docs/generated/map/latest-crystal-map-api.json`, Stage 5 UI smoke with 88 screenshots including login/select lifecycle, compact layout/text/multi-panel checks, inventory/storage/character/belt/minimap/chat/system-menu flows, Mail/Report/NPC panel state, compact Mail/Report panel bounds, advanced Stage 5 systems state, auction/trade/shop/conquest/hero/mining/craft/mail-delete evidence, gateway health on `127.0.0.1:7110`, and WS load 64/64 ready passed through R225.
- Latest formatting verification: `cargo +1.89.0 fmt --check` passed.
- Repository status note: `mir2` is a git repository on `main`; the known unrelated dirty item is outer-repo submodule drift at `refactor-pwa`. Do not revert or commit that drift unless explicitly asked.
- Toolchain note: this Mac environment needs `cargo +1.89.0` for Rust verification because the default `rustc 1.87.0` fails on locked `bevy_* 0.17.3`.
- Local asset note: `node packages/tooling/scripts/generate-crystal-respawn-manifest.mjs` currently fails here with `ENOENT` for `/Users/henryliu/obelisk/ai/numeron/mir2/Crystal/Build/Server/Debug/Server.MirDB`; do not mark the data-backed map-flag import complete until that asset and matching `Envir/Routes` are available.
- Windows continuation note: after pulling on Windows, read `docs/WINDOWS-CONTINUATION.md`, then continue from `docs/AGENT-TASK-QUEUE.md` active round `2026-04-26-R226`. Treat `100.0% Candidate`, backend/server tracked-slice `99.70%`, and real full-project accepted 1:1 `roughly 90.0%` as separate metrics. Do not claim backend/server 100% unless live Crystal trace acceptance, the blocked `Server.MirDB` data import, or a documented acceptance decision closes the remaining 0.30%. Do not claim full-project 100% Accepted until the human acceptance script passes or the user explicitly accepts remaining differences.
- Product evolution note: future work is expected to turn this verified Mir2-style MMORPG foundation into a custom product. Read `docs/POST-1TO1-EVOLUTION-PLAN.md`, `docs/TECH-MODERNIZATION-RFC.md`, `docs/PLATFORM-CLIENT-STRATEGY.md`, and `docs/ADMIN-OPERATIONS-ARCHITECTURE.md` before database, cache, login UI, admin backend, global zone, client distribution, or NPC script parser changes. Preserve the current Candidate baseline as a regression reference, but do not treat intentional product divergence as a Crystal parity bug.
- Admin implementation note: `apps/admin-api` is now an HTTP-capable operations-backend slice with repository traits, in-memory command/audit stores, Axum routes, and a `SendSystemMail` executor. `apps/admin-web` is a separate NextJS operations console with the first desktop UI pages and a Next proxy route wired to Rust for GM system mail. GM system mail is connected to live gateway/account-store Stage 5 mail and is visible/claimable in-game. It is still not backed by Postgres repositories, real operator auth, approvals, or broader live game commands.

## Last Completed Round: R224

R224 restored local packet trace matrix evidence:

- `apps/gateway/src/bin/packet_trace.rs` is restored and trackable despite the outer repo .NET `**/bin/` ignore.
- `packet_trace --list-flows` lists six representative TCP trace flows.
- `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` passed 53/53, including 6/6 packet trace bin tests.
- `MIR2_PACKET_TRACE_REQUIRE_LOCAL=1 cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix` wrote 9 local TCP-traceable artifacts under `docs/generated/packet-traces/r224-matrix` and skipped 17 non-TCP matrix entries by design.
- Live Crystal diff remains blocked until `MIR2_CRYSTAL_TCP_ADDR` is configured.

## Previous Completed Round: R223

R223 completed the automated **100% Candidate** gate:

- Stage 5 UI smoke now captures 88 screenshots.
- The manifest records advanced Stage 5 systems evidence for trade item/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, mining/craft, and mail delete state.
- Compact panel bounds now include Mail and Report, alongside existing inventory/storage/character/system-menu/chat-settings coverage.
- Map API smoke writes `docs/generated/map/latest-crystal-map-api.json` with 18/18 successful requests.
- Minimap asset smoke writes `docs/generated/assets/latest-minimap-assets.json` with 0 failures and the known 450/451 missing-index warning.
- WS load writes `docs/generated/load/latest-ws.json` with 64/64 ready and 0 errors.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 88 screenshots, map/minimap smokes, WS load, `mir2-game-data` 22/22, `mir2-gateway` 47/47, full `mir2-simulation` 664/664, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.
- R224 restored `cargo +1.89.0 run -p mir2-gateway --bin packet_trace -- --matrix`; require-local matrix evidence passed with 9 artifacts and 17 intentionally skipped non-TCP matrix entries. Live Crystal diff remains blocked until `MIR2_CRYSTAL_TCP_ADDR` is configured.

## Previous Completed Round: R222

R219-R222 advanced frontend/global evidence to 90.0%:

- Stage 5 UI smoke now captures 85 screenshots.
- The manifest records login/select lifecycle flows, confirmed character delete/recreate, compact multi-panel bounds, NPC dialog link-capable state, and the existing broad gameplay/system matrix.
- Map API smoke writes `docs/generated/map/latest-crystal-map-api.json` with 18/18 successful requests.
- Minimap asset smoke writes `docs/generated/assets/latest-minimap-assets.json` with 0 failures and the known 450/451 missing-index warning.
- WS load writes `docs/generated/load/latest-ws.json` with 64/64 ready and 0 errors.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 85 screenshots, map/minimap smokes, WS load, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Previous Completed Round: R218

R210-R218 advanced frontend/global evidence to 80.0%:

- Stage 5 UI smoke now captures 71 screenshots.
- The manifest records Mail/Report/NPC panel state, broad Stage 5 systems state, guild/group chat filters, Character repair/special-repair, ground item/gold pickup, combat target state, system-menu QA and transfer-list routing, Battle Focus spell casting, and compact inventory panel layout.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 71 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Previous Completed Round: R209

R209 advanced frontend/storage-password submit evidence:

- Stage 5 UI smoke fills Set Storage Password with mismatched confirmation and verifies submit remains disabled with the mismatch warning.
- The smoke fills matching `Safe123`, submits without an active storage service, and verifies `hasStoragePassword` remains false with no-service feedback.
- The smoke captures `stage5-storage-password-mismatch.png` and `stage5-storage-password-submit-no-service.png`.
- The manifest records the extended `storagePasswordFlow` with panel, mismatch, no-service submit, and closed states.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 60 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Previous Completed Round: R208

R208 advanced frontend/storage-password evidence:

- Protect is now reachable when no storage password is set.
- Stage 5 UI smoke opens Set Storage Password and closes it without submitting credentials.
- The smoke verifies title, prompt text, two password inputs, disabled submit, and debug storage password state.
- The smoke captures `stage5-storage-password-panel.png` and records `storagePasswordFlow`.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 58 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Previous Completed Round: R207

R207 advanced frontend/storage-takeback evidence:

- Stage 5 UI smoke opens Take Back for stored Red Potion, selects an inventory slot, and closes storage to expose the feedback line.
- The smoke verifies that without an active storage service, bag1 Red Potion remains quantity 3 and storage Red Potion remains quantity 10.
- The smoke captures `stage5-storage-takeback-red-potion-selected.png`, `stage5-storage-takeback-red-potion-result.png`, and `stage5-storage-takeback-red-potion-feedback.png`, and records `storageTakeBackFlow`.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 57 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Previous Completed Round: R206

R206 advanced frontend/storage-store evidence:

- Stage 5 UI smoke opens Store Item for Dagger, selects a warehouse slot, and closes storage to expose the feedback line.
- The smoke verifies that without an active storage service, Dagger remains in bag1 slot 4 and existing storage contents are unchanged.
- The smoke exposes `storageItems` in `window.__mir2Stage5.state` for manifest-backed assertions.
- The smoke captures `stage5-storage-store-dagger-selected.png`, `stage5-storage-store-dagger-result.png`, and `stage5-storage-store-dagger-feedback.png`, and records `storageStoreFlow`.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 54 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Previous Completed Round: R205

R205 advanced frontend/inventory-sell evidence:

- Stage 5 UI smoke opens Sell Item for Dagger and confirms without an active sell service.
- The smoke verifies Dagger remains in bag1 slot 4 and gold stays at 1180.
- The smoke captures `stage5-inventory-sell-dagger-panel.png` and `stage5-inventory-sell-dagger-no-service.png`, and records `inventorySellFlow`.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 51 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Previous Completed Round: R204

R204 advanced frontend/belt mouse-use evidence:

- Stage 5 UI smoke clicks Red Potion directly in the belt.
- The smoke verifies belt quantity falls from 5 to 4 before the hotkey path falls from 4 to 3.
- The smoke captures `stage5-belt-mouse-use-red-potion.png` and records `beltMouseUseFlow`.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 49 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Previous Completed Round: R203

R203 advanced frontend/character-remove evidence:

- Character RemoveItem now sends target `grid: "inventory"` and chooses the first free bag1 slot.
- Stage 5 UI smoke verifies Dagger leaves the weapon equipment slot and returns to bag1 slot 4.
- The smoke captures `stage5-character-remove-dagger.png` and records `characterRemoveFlow`.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 48 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Previous Completed Round: R202

R202 advanced frontend/inventory-drop evidence:

- Stage 5 UI smoke opens Delete Item for Blue Potion and confirms the drop.
- The smoke verifies Blue Potion quantity drops from 3 to 2 and a `Blue Potion` ground label appears.
- The smoke captures `stage5-inventory-drop-blue-potion-panel.png` and `stage5-inventory-drop-blue-potion.png`, and records `inventoryDropFlow`.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 47 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.
- The smoke captures `stage5-hud-skill-spells.png` and `stage5-hud-option-stats2.png`.
- The manifest records `hudButtonFlow` with active character tab and visible field counts.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 40 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R197

R197 advanced frontend/inventory-equip evidence:

- `window.__mir2Stage5.state` exposes `equipmentItems`.
- Stage 5 UI smoke clicks Dagger from inventory bag1.
- The smoke verifies Dagger moves into the weapon equipment slot.
- The smoke captures `stage5-inventory-equip-dagger.png` and records `inventoryEquipFlow`.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 38 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R196

R196 advanced frontend/inventory-use evidence:

- Stage 5 UI smoke clicks Red Potion from inventory bag1.
- The smoke verifies Red Potion quantity drops from 5 to 4.
- The smoke captures `stage5-inventory-use-red-potion.png` and records `inventoryUseFlow`.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 37 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R195

R195 advanced frontend/expanded-storage evidence:

- `window.__mir2Stage5.state` exposes `hasExpandedStorage`.
- Stage 5 UI smoke clicks Rent from locked storage page 2.
- The smoke verifies page 2 unlocks, expanded storage is active, capacity text shows 160 slots, and expiry copy renders.
- The smoke captures `stage5-storage-page2-rented.png` and records `page2Rented` in `storageFlow`.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 36 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R194

R194 advanced frontend/system-menu evidence:

- Stage 5 UI smoke opens the system menu and records action/transfer labels.
- The smoke routes Character, Inventory, and Quest menu actions and verifies the resulting panels.
- The smoke captures `stage5-system-menu.png`, `stage5-system-menu-character.png`, `stage5-system-menu-inventory.png`, and `stage5-system-menu-quest.png`.
- The manifest records `systemMenuFlow` with menu labels, transfer labels, meta text, panel visibility, and active inventory tab.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 35 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R193

R193 advanced frontend/chat-control evidence:

- Stage 5 UI smoke exercises chat Shout filter, All restore, Settings, collapse/restore size, and Report paths.
- The smoke captures `stage5-chat-shout-filter.png`, `stage5-chat-settings.png`, `stage5-chat-collapsed.png`, and `stage5-chat-report.png`.
- The manifest records `chatFlow` with frame, filter result, settings/report, collapsed/feed-hidden, visible line, and scroll knob state.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 31 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R192

R192 advanced frontend/storage evidence:

- Stage 5 UI smoke switches storage page 1, locked page 2, and restored page 1.
- The smoke captures `stage5-storage-page2-locked.png` and `stage5-storage-page1-restored.png`.
- The manifest records `storageFlow` with active page, locked state, storage item counts, slot count, and locked expanded-storage copy.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 27 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R191

R191 advanced frontend/character evidence:

- `window.__mir2Stage5.state` exposes `activeCharacterTab` and `knownSkills`.
- Stage 5 UI smoke switches character char, stats1, stats2, spells, and restored char tabs.
- The smoke captures `stage5-character-stats1.png`, `stage5-character-stats2.png`, `stage5-character-spells.png`, and `stage5-character-char-restored.png`.
- The manifest records `characterFlow` with active tab, equipment count, stat value count, spell value count, and known skills.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 25 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R190

R190 advanced frontend/inventory evidence:

- `window.__mir2Stage5.state` exposes `inventoryItems` and `activeInventoryTab`.
- Stage 5 UI smoke switches inventory bag1, bag2, quest, and restored bag1 tabs.
- The smoke captures `stage5-inventory-bag2.png`, `stage5-inventory-quest.png`, and `stage5-inventory-bag1-restored.png`.
- The manifest records `inventoryFlow` with active tab, visible item cards, quest entry count, and item summaries.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 21 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R189

R189 advanced frontend/belt use evidence:

- `window.__mir2Stage5.state` exposes `beltItems` for smoke assertions.
- Stage 5 UI smoke presses hotkey `1`.
- The smoke verifies Red Potion in slot 1 drops from quantity 5 to 4.
- The smoke captures `stage5-belt-hotkey-use.png` and records `beltUseFlow`.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 18 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R188

R188 advanced frontend/belt evidence:

- Stage 5 UI smoke now clicks belt rotate to vertical, rotate back to horizontal, and close.
- The smoke captures `stage5-belt-vertical.png`, `stage5-belt-horizontal.png`, and `stage5-belt-closed.png`.
- The manifest records `beltFlow` states and asserts six labels stay within the belt frame.
- Belt slot-label offsets no longer double-count the slot index.
- The vertical belt no longer overlaps the Quest tracker.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 17 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R187

R187 advanced frontend/minimap evidence:

- Stage 5 UI smoke now clicks minimap collapse, BigMap re-expand, and Mail open paths.
- The smoke captures `stage5-minimap-collapsed.png`, `stage5-minimap-expanded.png`, and `stage5-minimap-mail.png`.
- The manifest records `minimapFlow` states for expanded, collapsed, expanded-after-BigMap, and mail-open.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 14 screenshots, screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R186

R186 advanced frontend/global text-layout evidence:

- Stage 5 UI smoke now checks visible compact quest/HUD/minimap/belt/chat/entity text for overflow.
- The manifest records `compactTextLayout` with 33 checked text nodes and no overflow.
- The new check caught compact minimap title overflow.
- The minimap title now renders the map name and Safe Zone as stable two-line Crystal-style text.
- Web `tsc --noEmit`, direct `next build`, `node --check`, Stage 5 UI smoke with 11 screenshots, compact screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R185

R185 advanced frontend/global screenshot evidence:

- Stage 5 UI smoke now records named desktop 1024x768 and compact 820x640 viewport metadata.
- The route captures `docs/stage5-screenshots/stage5-compact-game.png` after completing the Stage 5 systems path.
- The smoke manifest includes compact bounds for `.client-stage-frame`, `.game-ui-scene`, `.main-hud-shell`, `.chat-frame`, and `.mini-map-panel`.
- The smoke fails if any of those core UI elements overflow the compact viewport.
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`, gateway/web health, Stage 5 UI smoke with 11 screenshots, compact screenshot visual inspection, `cargo +1.89.0 fmt --check`, and `git diff --check` passed.

## Earlier Completed Round: R184

R184 advanced frontend/global smoke parity:

- Chat now opens on the newest filtered lines, follows new messages while at the bottom, preserves scrollback, and moves the scroll knob with position.
- No-WebGL2/headless browsers stay in DOM UI mode instead of tripping Bevy WebGL surface panic.
- Crystal map API falls back to packaged starter-region data when local Crystal `Map/*.map` files are absent, avoiding recursive missing-map failure.
- Stage 5 UI smoke now detects macOS Chrome.
- Web `tsc --noEmit` and direct `next build` passed.
- Crystal minimap and map API smokes passed.
- Stage 5 UI smoke captured 10 screenshots and reported zero critical console errors.
- Gateway health on `127.0.0.1:7110` returned ready, and WS load passed 64/64 ready with 0 errors.
- `cargo +1.89.0 fmt --check` and `git diff --check` passed.

## Earlier Completed Round: R183

R183 moved the remaining runtime quest-hint localization out of the `sim.*` namespace:

- `build_interaction_hints` now uses `custom.interaction.questHint`.
- The Crystal localization importer and generated game-data/web localization bundles use the same key.
- `rg -n "sim\\." apps/simulation/src/runtime.rs` has no matches.
- `cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1` passed 22/22.
- Focused `world_snapshot_includes_scene_and_state_data` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 664/664.

## Previous Completed Round: R182

R182 removed no-script NPC idle fallback:

- No-script/no-page NPC interaction now returns silently instead of opening runtime-only idle dialog text.
- Scripted NPC and modeled quest NPC dialog behavior remain intact.
- Focused no-script NPC test passed 1/1.
- Adjacent `npc_interaction` filter passed 2/2.
- Broad `crystal_npc` filter passed 52/52.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 664/664.

## Previous Completed Round: R181

R181 localized quest-required drop feedback:

- Quest-required drop feedback now uses Crystal `server.YouFound`.
- Runtime-only `sim.youSecuredQuestItem`, `sim.questReturnForReward`, and `sim.questProgressWasps` progress chats were removed from that path.
- `GainedItem`, quest inventory gain, and quest state updates remain intact.
- Focused quest-required drop test passed 1/1.
- Adjacent `quest_required_drop` filter passed 3/3.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 664/664.

## Previous Completed Round: R180

R180 localized start-game welcome chat:

- `StartGame` welcome chat now uses Crystal `server.Welcome` with localized `server.GameName`.
- The welcome packet uses `ChatType::Hint` instead of runtime-only `sim.welcomeCharacter` System text.
- The bootstrap packet order remains intact.
- Focused simulation `start_game_emits_bootstrap_sequence` passed 1/1.
- Focused gateway `start_game_emits_bootstrap_sequence` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 664/664.
- Full `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` passed 47/47.

## Previous Completed Round: R179

R179 removed runtime-only normal chat echo:

- Normal `ClientPacket::Chat` before `StartGame` now returns no packets.
- In-game normal chat now emits only `ObjectChat` with `Name: message`.
- The runtime-only `sim.echoChat` self `Chat` echo is removed.
- `@ADDSTORAGE` remains as the modeled helper command.
- Simulation `chat_` filter passed 43/43.
- Gateway `chat_` filter passed 2/2.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 664/664.
- Full `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` passed 47/47.

## Previous Completed Round: R178

R178 removed runtime-only cast-skill failure chats:

- High-level `cast_skill` unknown-skill, cooldown, unwired-definition, missing-player, and no-MP failures no longer emit `sim.skillNotKnown`, `sim.skillCooldown`, `sim.skillLogicNotWired`, `sim.playerNotInWorld`, or `sim.notEnoughMp`.
- Unwired summon-spell and missing dynamic summon-template failures now also return no runtime-only helper chat.
- Successful buff/heal and summon behavior remains intact.
- `casting` filter passed 9/9.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 663/663.

## Previous Completed Round: R177

R177 removed runtime-only MoveItem unsupported fallback chat:

- `MoveItem` unsupported-grid/missing-source fallback no longer emits `sim.itemNotFoundInBag`.
- Unsupported grids remain failed-ack only.
- Inventory/Storage missing-source failures keep Crystal `server.ItemMoveErrorReport`.
- `move_item` filter passed 26/26.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 660/660.

## Previous Completed Round: R176

R176 removed runtime-only stale active-dialog missing-NPC/no-script chat:

- Active NPC dialog target follow-up with a missing NPC entity or NPC lacking script metadata now dismisses silently without `sim.targetNotGroundDrop` or `sim.npcNoMilestoneScript`.
- Ordinary no-script NPC idle fallback remains intact.
- Focused stale-dialog tests passed 2/2.
- Adjacent `npc_interaction` (2/2) filter passed.
- Broad `crystal_npc` (52/52) filter passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 660/660.

## Previous Completed Round: R175

R175 removed runtime-only NPC dialog helper no-active/invalid-target/no-pending-input chat:

- High-level dialog target/input helper no-active-dialog, invalid-target, and no-pending-input failures no longer emit runtime-only `sim.npcNoMilestoneScript` or `sim.itemNoActiveUse`.
- Successful dialog link, input, and service flows remain intact.
- Focused dialog-helper tests passed 3/3.
- Adjacent `npc_interaction` (2/2) filter passed.
- Broad `crystal_npc` (52/52) filter passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 658/658.

## Previous Completed Round: R174

R174 removed runtime-only direct NPC interaction invalid target/direction/range chat:

- High-level `interact(object_id)` missing-target, same-tile/no-direction, and out-of-range failures no longer emit runtime-only `sim.*` chat.
- Successful NPC dialog, script, and service flows remain intact.
- Focused direct-interact tests passed 3/3.
- Adjacent `npc_interaction` (2/2) filter passed.
- Broad `crystal_npc` (52/52) filter passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 655/655.

## Previous Completed Round: R173

R173 removed runtime-only direct attack invalid target/state/range chat:

- High-level `attack(object_id)` missing-target, non-monster, dead/hidden/stoned, no-direction, and out-of-range failures no longer emit runtime-only `sim.*` chat.
- Turn packets, normal attack packets, hidden reveal, Zuma wake, and delayed hit surfaces remain intact.
- Focused direct-attack tests passed 4/4.
- Hidden and Zuma focused tests passed 2/2.
- Adjacent `attack` (80/80) filter passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 652/652.

## Previous Completed Round: R172

R172 removed runtime-only successful NPC interaction chat:

- High-level NPC interaction no longer emits `sim.talkingToNpc`.
- NPC `ObjectChat`/dialog packet surfaces remain intact.
- Crystal NPC script and service flows remain intact.
- Focused `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), and `crystal_npc_service` (1/1) filters passed.
- Broad `crystal_npc` (52/52) filter passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 648/648.

## Previous Completed Round: R171

R171 removed runtime-only direct pickup invalid target/distance chat:

- High-level `pick_up(object_id)` missing-object, non-ground-target, and out-of-cell failures now return silently.
- Crystal owner-blocked/full-bag pickup messages remain intact.
- Current-cell packet pickup behavior remains intact.
- Focused direct-pickup tests passed 3/3.
- Adjacent `pickup` (18/18) and `drop` (42/42) filters passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 648/648.

## Previous Completed Round: R170

R170 removed runtime-only missing defeated-entity chat:

- Missing defeated-monster entity handling now silently returns without `sim.defeatedMonsterEntityMissing`.
- Normal death/drop packet surfaces remain intact.
- Focused missing-entity silent test passed 1/1.
- Visible death packet test passed 1/1.
- Adjacent `drop` (41/41) filter passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 645/645.

## Previous Completed Round: R169

R169 removed runtime-only monster death-drop success chats:

- Monster death gold/item drop paths no longer emit `sim.monsterDroppedGoldOnGround` or `sim.monsterDroppedItem`.
- Ground gold/item drops, quest-drop routing, owner windows, and pickup packet surfaces are preserved.
- Focused item-drop no-chat test passed 1/1.
- Focused gold-drop no-chat/pickup test passed 1/1.
- Adjacent `drop` (41/41), `pickup` (15/15), and `attack` (76/76) filters passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 644/644.

## Previous Completed Round: R168

R168 removed runtime-only summoned VampireSpider defeat chat:

- Summoned VampireSpider death explosion no longer emits `sim.targetDefeated`.
- Explosion damage, summon despawn timing, and delayed health packet surfaces are preserved.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation friendly_vampire_spider_death_explosion_has_no_runtime_defeat_chat_and_hits_nearby_hostile_monster -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `spider` (6/6) and `attack` (76/76) filters passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 643/643.

## Previous Completed Round: R167

R167 removed runtime-only ordinary combat damage narration:

- Ordinary player/monster hit resolution no longer emits `sim.youHitTargetForDamage`, `sim.targetDefeated`, or `sim.monsterPressuresYouForDamage`.
- Packet health/struck/death surfaces and Trainer DPS reporting are preserved.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation player_attack_hit_resolution_has_no_runtime_damage_chat -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `cargo +1.89.0 test --locked -p mir2-simulation attack -- --test-threads=1 --nocapture` passed 76/76.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 643/643.

## Previous Completed Round: R166

R166 removed runtime-only successful cast-skill helper chat:

- Buff/heal and summon success paths now preserve state mutation/spawns without generic `sim.castSkill` narration.
- Explicit casting failure messages remain unchanged.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation casting -- --test-threads=1 --nocapture` passed 6/6.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 643/643.

## Previous Completed Round: R165

R165 removed runtime-only pre-start cast-skill helper chat:

- `cast_skill` now emits no packets/chat before `StartGame`.
- Started-world buff, cooldown, MP, and summon casting behavior are preserved.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation cast_skill_before_start_game_rejects_without_runtime_chat -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `casting` (6/6) filter passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 643/643.

## Previous Completed Round: R164

R164 removed runtime-only pre-start interaction helper chats:

- `interact` and `select_npc_dialog_target` now emit no packets/chat before `StartGame`.
- Started-world NPC interaction, dialog target, and service-link behavior are preserved.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation interact_before_start_game_rejects_without_runtime_chat -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `npc_interaction` (2/2), `crystal_npc_dialog` (1/1), and `crystal_npc_service` (1/1) filters passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 642/642.

## Previous Completed Round: R163

R163 removed runtime-only pre-start harvest helper chats:

- `harvest` and `Harvest` now emit no packets/chat before `StartGame`.
- Started-world harvest packet and corpse-loot behavior are preserved.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation harvest_before_start_game_rejects_without_runtime_chat -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `harvest` (9/9) filter passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 641/641.

## Previous Completed Round: R162

R162 removed runtime-only pre-start attack helper chats:

- `attack`, `Attack`, and `RangeAttack` now emit no packets/chat before `StartGame`.
- Started-world attack packet traces and delayed combat health behavior are preserved.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation attack_before_start_game_rejects_without_runtime_chat -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `attack` (76/76) and combat trace focused (1/1) filters passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 640/640.

## Previous Completed Round: R161

R161 removed runtime-only pre-start movement helper chats:

- `move_to`, `Walk`, `Run`, and `Turn` now emit no packets/chat before `StartGame`.
- Started-world movement, turn packets, blocking, run fallback, and map-transfer behavior are preserved.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation movement_before_start_game_rejects_without_runtime_chat -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `walk` (6/6), `run_` (3/3), and `transfer_map` (2/2) filters passed.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 639/639.

## Previous Completed Round: R158

R158 localized trainer average damage reporting:

- Localization formatting now substitutes Crystal-style `{index:format}` placeholders.
- Trainer idle average damage chat now uses Crystal `server.AverageDamageOnTrainer`.
- Immediate trainer damage chat remains hardcoded because the generated bundle has no dedicated player-inflicted trainer-damage key.
- `cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1` passed 22/22.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation trainer_is_static_passive_and_does_not_die_from_damage -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R157

R157 localized benediction-oil weapon luck outcome chats:

- No-effect, luck, and curse outcomes now use Crystal `server.WeaponNoEffect`, `server.WeaponLuck`, and `server.WeaponCurse`.
- Regressions assert generated localized text instead of hardcoded English.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation benediction_oil -- --test-threads=1 --nocapture` passed 4/4.
- Adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture` passed 42/42.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R156

R156 removed the expanded-storage helper success chat:

- `@ADDSTORAGE` now emits modeled `ResizeStorage` without hardcoded `"Expanded storage activated."` chat.
- Storage expansion state, expiry persistence, and storage-family behavior are preserved.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation addstorage_chat_command -- --test-threads=1 --nocapture` passed 2/2.
- Adjacent `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture` passed 43/43.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R155

R155 localized the group pickup notice:

- `ShowGroupPickup` item notices now use Crystal `server.FriendlyPickedUpItem`.
- The regression asserts generated localized text instead of hardcoded English.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation pickup_emits_crystal_group_pickup_notice_for_marked_items -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `cargo +1.89.0 test --locked -p mir2-simulation pickup -- --test-threads=1 --nocapture` passed 14/14.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R154

R154 removed runtime-only high-level use/drop before-start chats:

- `use_item(key)` and `drop_item(key)` now emit no packets/chat before `StartGame`.
- Normal post-start use/drop behavior and packet-path failure surfaces are preserved.
- Adjacent `cargo +1.89.0 test --locked -p mir2-simulation drop_item -- --test-threads=1 --nocapture` passed 10/10.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation consumable_item_restores_hp -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture` passed 42/42.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R153

R153 removed the runtime-only high-level drop helper missing-item chat:

- `drop_item(key)` now emits no packets/chat and preserves state when the requested key is absent.
- This aligns the high-level helper with the packet `DropItem` missing-source no-chat/no-mutation surface.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation dropped_inventory_item_can_be_removed_from_bag_and_spawned_on_ground -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `cargo +1.89.0 test --locked -p mir2-simulation drop_item -- --test-threads=1 --nocapture` passed 10/10.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R152

R152 localized map-transfer not-in-world rejection:

- Public `transfer_map` now uses Crystal `server.NotFound` before start-game instead of `sim.joinWorldBeforeMoving`.
- Internal ordinary/debug transfer missing-player handling also remains aligned on `server.NotFound`.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation transfer_map_requires_player_on_transfer_bounds -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `cargo +1.89.0 test --locked -p mir2-simulation transfer_map -- --test-threads=1 --nocapture` passed 2/2.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R151

R151 localized missing-template `RequestItemInfo` failure:

- `RequestItemInfo` now uses Crystal `server.NotFound` when the requested item template is absent.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation request_item_info_packet_returns_crystal_item_info -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R150

R150 localized map-transfer bounds rejection:

- `apply_map_transfer` now uses Crystal `server.CannotPositionMoveOnMap` when the player is not standing on the configured transfer source tile.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation transfer_map_requires_player_on_transfer_bounds -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `cargo +1.89.0 test --locked -p mir2-simulation transfer_map -- --test-threads=1 --nocapture` passed 2/2.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R149

R149 removed remaining runtime-only Stage 5 event/hero helper success chats:

- `event.spawn` and `hero.behaviour` now preserve state mutations without emitting generic simulator narration.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1 --nocapture` passed 1/1.
- Broader `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` passed 26/26.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R148

R148 removed runtime-only debug Crystal transfer success chat:

- `crystal:<map>:<x>:<y>` transfer keys now retain `MapInformation` and `UserLocation` while suppressing simulator-only `"Transferred to Crystal map ..."` chat.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation debug_crystal_transfer_key_updates_map_information_and_location -- --test-threads=1 --nocapture` passed 1/1.
- Adjacent `cargo +1.89.0 test --locked -p mir2-simulation transfer_map -- --test-threads=1 --nocapture` passed 2/2.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R147

R147 removed generic runtime-only Stage 5 helper success chats:

- Group/social/mail/trade/auction/conquest/hero/profession helper successes now preserve state mutations without emitting simulator-only narration.
- Existing Crystal localized failure/success surfaces remain intact.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` passed 26/26.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R146

R146 localized Stage 5 event-spawn missing player/position rejection:

- `stage5_event_spawn` now uses Crystal `server.NotFound` when no player entity or position is available.
- Extended the conquest/event/hero/mining/crafting regression to assert localized missing-player event-spawn rejection before normal started-world event coverage.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R145

R145 localized unknown map-transfer rejection:

- `apply_map_transfer` now uses Crystal `server.NotFound` when the transfer key cannot be resolved.
- Extended `transfer_map_requires_player_on_transfer_bounds` to assert localized unknown-transfer rejection before the normal out-of-bounds transfer rejection.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation transfer_map_requires_player_on_transfer_bounds -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R144

R144 localized Stage 5 unknown-command rejection:

- Unknown Stage 5 commands now use Crystal `server.InvalidPacketReceived` with the rejected command as the packet index.
- Extended the trade/shop/auction error-path regression to assert the localized invalid-packet message.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R143

R143 localized Stage 5 inactive-trade state:

- `stage5_trade_offer_gold`, `stage5_trade_offer_item`, and `stage5_trade_accept` now use Crystal `server.NotFound` when no trade is active.
- Extended the trade/shop/auction error-path regression to assert localized inactive-trade messages and preserved gold/trade state.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R142

R142 localized Stage 5 auction invalid-packet input:

- `stage5_auction_buy` and `stage5_auction_cancel` now use Crystal `server.InvalidPacketReceived` when the auction id argument is absent/invalid.
- Extended the trade/shop/auction error-path regression to assert both localized invalid-packet messages.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R141

R141 localized Stage 5 mail invalid-packet input:

- `stage5_mail_claim` and `stage5_mail_delete` now use Crystal `server.InvalidPacketReceived` when the mail id argument is absent/invalid.
- Extended the social/group/guild/mail persistence regression to assert both localized invalid-packet messages.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_social_group_guild_mail_persist_across_reload -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R140

R140 localized Stage 5 trade invalid-packet input:

- `stage5_trade_offer_gold` now uses Crystal `server.InvalidPacketReceived` when the amount argument is absent/invalid.
- Extended the trade/shop/auction error-path regression to assert the localized invalid-packet message.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R139

R139 localized Stage 5 hero-behaviour missing-hero rejection chat:

- `stage5_hero_behaviour` now uses Crystal `server.NotFound` when no hero has been recruited.
- Extended the conquest/event/hero/mining/crafting regression to assert the localized missing-hero message before recruiting a hero.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R138

R138 localized Stage 5 event-spawn missing-template rejection chat:

- `stage5_event_spawn` now uses Crystal `server.NotFound` when the requested monster template cannot be resolved.
- Extended the conquest/event/hero/mining/crafting regression to assert the localized missing-template message before the valid event spawn.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R137

R137 localized Stage 5 guild creation success chat:

- `stage5_guild_create` now uses Crystal `server.SuccessfullyCreatedGuild`.
- Extended the social/group/guild/mail persistence regression to assert the localized guild creation message.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_social_group_guild_mail_persist_across_reload -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R136

R136 localized Stage 5 craft no-ore rejection chat:

- `stage5_craft` now uses Crystal `server.CraftingAttemptFailed` when ore is unavailable.
- Extended the mining/crafting flow regression to assert the localized failure before mining and preserve no crafted-item mutation.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R135

R135 localized Stage 5 credit-shop insufficient-credit rejection chat:

- `stage5_shop_buy_credit` now uses Crystal `server.YouDontHaveEnoughCurrency` when account credit is below the requested price.
- Extended the existing transactional error-path regression to assert the localized chat and preserve credit, mail, inventory, and `LoseCredit` no-mutation behavior.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` passed 1/1.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R134

R134 localized Stage 5 missing mail/trade item/auction listing rejection chats:

- `stage5_mail_claim` now uses Crystal `server.NotFound` when the mail id is absent.
- `stage5_trade_offer_item` now uses Crystal `server.NotFound` when the requested item is absent.
- `stage5_auction_buy` now uses Crystal `server.NotFound` when the listing is absent/cancelled/sold.
- Added a focused combined regression proving no gold, mail, trade, auction, or inventory mutation occurs.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` passed 26/26.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 638/638.

## Previous Completed Round: R133

R133 localized Stage 5 socket metadata-missing rejection chat:

- `stage5_item_add_socket` now uses Crystal `server.NotFound` when equipped item socket metadata is unavailable.
- Added a focused regression for unknown socket metadata preserving equipment state and emitting no `ItemSlotSizeChanged`.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` passed 16/16.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 636/636.

## Previous Completed Round: R132

R132 localized Stage 5 socket/seal missing-equipped-item rejection chats:

- `stage5_item_add_socket` now uses Crystal `server.NotFound` when the requested equipment slot is empty.
- `stage5_item_seal` now uses Crystal `server.NotFound` when the requested equipment slot is empty.
- Added focused regressions for missing weapon equipment in both commands.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` passed 15/15.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 635/635.

## Previous Completed Round: R131

R131 localized Stage 5 socket/seal missing-source rejection chats:

- `stage5_item_add_socket` now uses Crystal `server.NotFound` when the requested socket source item is absent.
- `stage5_item_seal` now uses Crystal `server.NotFound` when the requested seal source item is absent.
- Source lookup and no-mutation failure behavior are unchanged.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` passed 13/13.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R130

R130 removed runtime-only ordinary map-transfer success chat:

- `apply_map_transfer` now passes no success message for ordinary manifest-backed transfers.
- Ordinary transfers still emit `MapInformation` and `UserLocation` and still update safe-zone/map snapshot state.
- Debug `crystal:MAP:X:Y` transfer helper messaging is unchanged.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation transfer_map -- --test-threads=1 --nocapture` passed 2/2.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R129

R129 localized Stage 5 socket/seal invalid-source rejection chats:

- `stage5_item_add_socket` now uses Crystal `server.InvalidCombination` for invalid socket source items.
- `stage5_item_seal` now uses Crystal `server.InvalidCombination` for invalid seal source items.
- Source item retention and no-mutation failure behavior are unchanged.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` passed 13/13.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R128

R128 localized Stage 5 gold-shop purchase chat:

- `stage5_shop_buy` now uses Crystal `server.BoughtItemForGold` instead of runtime-only `"Bought {key}."`.
- Gold debit, item gain, and transactional behavior are unchanged.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_are_transactional -- --test-threads=1 --nocapture` passed 1/1.
- Broader `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` passed 22/22.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R127

R127 removed runtime-only harvest success chat:

- Successful harvest-drop transfer no longer emits `"Harvested ..."` system chat.
- The success surface remains `GainedItem` packets followed by `ObjectHarvested`.
- Focused/broader `cargo +1.89.0 test --locked -p mir2-simulation harvest -- --test-threads=1 --nocapture` passed 8/8.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R126

R126 localized expanded-storage expiry notice:

- `sync_expired_expanded_storage` now uses Crystal `server.ExpandedStorageExpired` instead of runtime-only `"Expanded storage expired."`.
- One-shot notice behavior, `ResizeStorage`, account flag persistence, and backing storage size are unchanged.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation expired_expanded_storage_tick_emits_resize_notice_once_and_persists_flag -- --test-threads=1 --nocapture` passed 1/1.
- Broader `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture` passed 43/43.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R125

R125 localized Stage 5 item socket/seal success chats:

- `stage5_item_add_socket` now uses Crystal `server.ItemSocketsIncreased` instead of runtime-only `"Item socket slots increased to {slot_size}."`.
- `stage5_item_seal` success now uses Crystal `server.ItemSealedFor` instead of runtime-only `"Item sealed for {minutes} minutes."`.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` passed 13/13.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R124

R124 localized Stage 5 item-seal reseal-delay rejection:

- `stage5_item_seal` now uses Crystal `server.ItemCannotBeResealedFor` instead of runtime-only `"Item cannot be resealed yet."`.
- The branch reuses the modeled Crystal remaining-time duration label already used by the `CombineItem` reseal path.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_seal_rejects_before_next_seal_date_after_expiry -- --test-threads=1 --nocapture` passed 1/1.
- Broader `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` passed 13/13.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R123

R123 localized Stage 5 credit-shop purchase chat:

- `stage5_shop_buy_credit` now uses Crystal `server.BoughtItemForCredit` instead of runtime-only `"Bought {key} for {price} credit. Mail {mail_id} created."`.
- Mailbox delivery, `LoseCredit`, credit debit, and claim behavior are unchanged.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_credit_shop_mails_purchase_and_claim_transfers_attachment -- --test-threads=1 --nocapture` passed 1/1.
- Broader `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` passed 22/22.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R122

R122 localized Stage 5 successful trade completion:

- `stage5_trade_accept` now uses Crystal `server.TradeSuccessful` instead of runtime-only `"Trade completed."`.
- The Stage 5 transactional regression now checks exact localized chat while preserving accepted/completed state and gold deduction.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_are_transactional -- --test-threads=1 --nocapture` passed 1/1.
- Broader `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` passed 22/22.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R121

R121 localized Stage 5 low-gold rejection keys:

- `stage5_trade_offer_gold`, `stage5_trade_accept`, `stage5_shop_buy`, and `stage5_auction_buy` now use Crystal `server.LowGold` instead of runtime-only `"Not enough gold."`.
- The Stage 5 error-path regression now checks exact localized chat for failed trade gold offer, shop buy, and auction buy while preserving gold/item/listing state.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` passed 1/1.
- Broader `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` passed 22/22.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R120

R120 localized direct ground-drop pickup full-bag rejection:

- `pick_up_ground_drop` now uses Crystal `server.YouCannotCarryAnymore` instead of runtime-only `"No free bag slot."` when a directly selected ground item cannot fit in the bag.
- Current-cell pickup behavior remains unchanged: full-bag blocked item drops are skipped so later pickable candidates, such as gold, can still be collected.
- `pickup_preserves_ground_drop_when_inventory_is_full` now locks both the current-cell no-chat skip behavior and the direct-pickup localized full-bag rejection.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation pickup -- --test-threads=1 --nocapture` passed 14/14.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R119

R119 localized Stage 5 full-bag economy/helper rejection keys:

- `stage5_mail_claim`, `stage5_shop_buy`, `stage5_auction_buy`, and `stage5_craft` now use Crystal `server.YouCannotCarryAnymore` instead of runtime-only `"No free bag slot."`.
- The full-bag Stage 5 regression now checks exact localized chat for shop, mail claim, auction buy, and craft while preserving transactional gold/credit/item/ore behavior.
- Focused `cargo +1.89.0 test --locked -p mir2-simulation stage5_shop_and_auction_full_bag_preserve_gold_and_items -- --test-threads=1 --nocapture` passed 1/1.
- Broader `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` passed 22/22.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R118

R118 localized Stage 5 item socket/seal rejection keys:

- Stage 5 socket max-capacity rejection now emits localized `server.ItemMaxSockets`.
- Stage 5 already-sealed rejection now emits localized `server.ItemAlreadySealed`.
- Source-item and reseal-delay branches were left unchanged because their current strings do not yet have fully modeled Crystal argument surfaces.
- Focused `stage5_item_` passed 13/13.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R117

R117 localized harvest no-drop/full-bag messages:

- Harvest no-drop paths now emit localized `server.NothingWasFound`.
- Harvest pending-drop full-bag retry now emits localized `server.YouCannotCarryAnymore`.
- Pending-drop retry semantics and `ObjectHarvested` timing are unchanged.
- Focused `harvest` passed 8/8.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R116

R116 localized owner-blocked pickup rejection:

- Current-cell owner-blocked pickup now emits localized `server.CannotPickupNotOwner` instead of a hardcoded runtime English string.
- Direct object-id pickup helper now uses the same localized key for the owner-blocked branch.
- Existing scan behavior remains unchanged: owner-blocked drops are skipped when a later current-cell candidate is pickable.
- Focused `pickup` passed 14/14.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R115

R115 removed runtime-only normal pickup success chat:

- Normal item pickup success no longer emits `sim.pickedUpItem`.
- Normal gold pickup success now emits `GainedGold` without generic pickup chat.
- Crystal `ShowGroupPickup` group notices remain preserved for the modeled item pickup surface.
- Focused `pickup` passed 14/14.
- `cargo +1.89.0 fmt --check` passed.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R114

R114 added Crystal NoDrug potion map-rule rejection:

- `MapDropRuleRecord` now carries `no_drug`.
- Dynamic manifest-backed `ItemType.Potion` use now rejects through `CanUseItem` eligibility when the current map has `no_drug`.
- Static starter HP/MP potion use now rejects before timed-recovery queueing on `no_drug` maps.
- Rejection emits `server.YouCannotUsePotionsHere`, returns failed `UseItem`, preserves the item, and leaves HP/MP recovery unqueued.
- Focused `no_drug` passed 2/2.
- Adjacent `use_item_packet_` passed 42/42.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 633/633.

## Previous Completed Round: R113

R113 aligned static starter HP/MP potion use with Crystal timed recovery:

- Static `red-potion` / `belt-red-potion` use now queues pending HP recovery instead of mutating `PlayerVitals` immediately.
- Successful use still consumes one item and returns the successful `UseItem` ack without chat.
- Follow-up ticks drain pending recovery and emit `ObjectHealth`, matching the existing dynamic Crystal normal-potion model.
- Focused `crystal_use_item_packet_consumes_` passed 2/2.
- Adjacent `use_item_packet_` passed 40/40.
- Legacy `consumable_item_restores_hp` passed after being updated to the timed-recovery surface.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 631/631.

## Previous Completed Round: R112

R112 removed runtime-only static repair-powder chat:

- Static `repair-powder` no-repair failure no longer emits `sim.noEquipmentNeedsRepair`.
- Static `repair-powder` repair success no longer emits `sim.repairedEquippedItems`.
- Repair mutation, item consumption on success, item preservation on failure, and `ItemRepaired` packets remain intact.
- Focused `repair_powder` passed 2/2.
- Adjacent `use_item_packet_` passed 40/40.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 631/631.

## Previous Completed Round: R108

R108 aligned static repair oils with Crystal chat surfaces:

- Static `repair-oil` / `war-god-oil` success now emits `ItemRepaired` plus localized `server.WeaponPartiallyRepaired` / `server.WeaponCompletelyRepaired` Hint chat.
- Static repair-oil failure now failed-acks without the runtime-only hardcoded no-repair chat and preserves the item.
- Focused `repair_oil` passed 3/3.
- Adjacent `use_item_packet_` passed 40/40.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 630/630.

## Previous Completed Round: R107

R107 removed the runtime-only successful `DropItem` chat:

- Successful normal and split-stack inventory drops now return success ack plus ground-object visibility without `custom.itemDropped`.
- This matches Crystal `PlayerObject.DropItem`, where normal success sets `p.Success = true` and enqueues the `S.DropItem` packet without a success chat.
- Adjacent `drop_item_packet` passed 10/10.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 629/629.

## Previous Completed Round: R106

R106 removed the runtime-only static HP/MP potion success chat:

- Static inventory/belt HP/MP consumables now heal, consume, and success-ack without `sim.usedItem`.
- This matches Crystal's normal potion success surface; `PlayerObject.UseItem` queues timed restore or changes HP/MP without a generic used-item chat.
- `crystal_use_item_packet_consumes_inventory_slot` passed.
- `crystal_use_item_packet_consumes_belt_slot` passed.
- Adjacent `use_item_packet_` passed 40/40.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 629/629.

## Previous Completed Round: R105

R105 removed the runtime-only missing-source `DropItem` chat:

- Missing inventory-id `DropItem` now returns only the failed `DropItem` ack.
- The runtime no longer emits `sim.itemNotFoundInBag` for that missing-source branch.
- `drop_item_packet_missing_inventory_item_rejects_without_runtime_chat` passed.
- Adjacent `drop_item_packet` passed 10/10.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 629/629.

## Previous Completed Round: R104

R104 aligned unmodeled hero-inventory `UseItem` with Crystal-shaped failed ack behavior:

- `UseItem(grid=HeroInventory)` now returns `ServerPacket::UseItem { success: false, grid: HeroInventory }` instead of empty packets.
- The runtime still does not fall back into matching player inventory, preserving the R44 no-mutation guard.
- `use_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item` passed.
- Adjacent `use_item_packet_` passed 40/40.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 628/628.

## Previous Completed Round: R103

R103 removed the runtime-only missing-item `UseItem` chat:

- Missing inventory-id `UseItem` packet handling now failed-acks without `sim.itemNotFoundInBag`.
- Runtime missing-location and missing-item fallbacks now failed-ack without chat.
- `use_item_packet_missing_inventory_item_rejects_without_runtime_chat` passed.
- Adjacent `use_item_packet_` passed 40/40.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 628/628.

## Previous Completed Round: R102

R102 removed the runtime-only unusable item fallback chat:

- Unusable inventory `UseItem` now failed-acks without `sim.itemNoActiveUse`.
- The item remains in inventory and no mutation occurs.
- `use_item_packet_unusable_inventory_item_rejects_without_runtime_chat` passed.
- Adjacent `use_item_packet_` passed 39/39.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 627/627.

## Previous Completed Round: R101

R101 removed the remaining runtime-only non-inventory use-equip failure chat:

- Belt-sourced equipment-like `UseItem` attempts now failed-ack without chat.
- The item remains in belt state and no equipment mutation occurs.
- `use_item_packet_belt_equipment_rejects_without_runtime_chat` passed.
- Adjacent `use_item_packet_` passed 38/38.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 626/626.

## Previous Completed Round: R100

R100 removed runtime-only chat from the successful use-equip surface:

- Successful modeled `UseItem` equipment no longer emits `sim.equippedItem` or `sim.equippedItemAndReturnedPrevious`.
- The successful surface is now ack/refresh/equipment-state only, matching Crystal's explicit `EquipItem` success packet surface for the bounded model.
- `use_item_packet_equipping_need_identify_item_emits_refresh_item` now asserts no chat is emitted.
- Adjacent `use_item_packet_` passed 37/37.
- Adjacent `equip_item_packet` passed 13/13.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 625/625.

## Previous Completed Round: R99

R99 locked the positive explicit equip path for dynamic manifest-backed equipment requirements:

- `SpiritRing` now has focused explicit `EquipItem` coverage at its required level 15.
- The item equips successfully into the right ring slot when Crystal requirements are met.
- `equip_item_packet_manifest_equipment_allows_when_requirements_are_met` passed.
- Adjacent `equip_item_packet` passed 13/13.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 625/625.

## Previous Completed Round: R98

R98 locked dynamic manifest-backed credit-token use coverage:

- `CreditToken3` created through the Crystal item catalog now uses the existing credit-token path.
- Success emits `UseItem`, `GainedCredit`, localized `server.CreditsAddedToAccount` hint chat, updates account credit, and consumes the item.
- `use_item_packet_dynamic_crystal_credit_token_emits_localized_hint_chat` passed.
- Adjacent `use_item_packet_` passed 37/37.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 624/624.

## Previous Completed Round: R97

R97 locked storage-grid coverage for the R96 explicit equip requirement rejection:

- `EquipItem(grid=Storage)` now has focused regression coverage for dynamic manifest-backed equipment with unmet Crystal requirements.
- The failure surface is ack-only, preserves the storage item, and does not equip the item.
- `equip_item_packet_storage_manifest_equipment_rejects_unmet_requirements_silently` passed.
- Adjacent `equip_item_packet` passed 12/12.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 623/623.

## Previous Completed Round: R96

R96 added explicit Crystal `CanEquipItem` requirement gating for dynamic manifest-backed equipment:

- Explicit `EquipItem` now silently rejects dynamic manifest-backed equipment when gender/class/required-type checks fail, matching Crystal's false-return surface for `CanEquipItem`.
- The shared requirement helper still drives localized `UseItem` rejection messages.
- Legacy hand-authored fixture aliases keep existing test behavior, while `crystal-item-*` catalog-backed equipment is gated.
- `equip_item_packet_manifest_equipment_rejects_unmet_requirements_silently` passed.
- Adjacent `equip_item_packet` passed 11/11.
- Full `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 622/622.

## Previous Completed Round: R95

R95 added explicit coverage for `ItemType.Amulet` targeting the right bracelet slot:

- `equip_item_packet_manifest_amulet_can_target_right_bracelet_slot` passed.
- Adjacent `equip_item_packet` suite passed 10/10.

## Previous Completed Round: R94

R94 was a wider validation pass after R89-R93:

- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture` passed 218/218.
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture` passed 42/42.
- `cargo +1.89.0 fmt --check` passed after applying `cargo +1.89.0 fmt`.
- `git -C mir2-web3 diff --check` passed.
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed 620/620.

## Previous Completed Round: R93

R93 fixed explicit `EquipItem` target compatibility for manifest-backed rings and bracelets:

- Imported item type compatibility now allows rings to target either ring slot.
- Imported item type compatibility now allows bracelets to target either bracelet slot.
- `UseItem` default slot selection remains unchanged.

Verification included:

- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_ring_and_bracelet_can_target_right_slots -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (9/9)

## Previous Completed Round: R92

R92 restored modeled MP on successful `ResurrectionScroll` revive:

- Dead-player `ResurrectionScroll` still consumes and emits existing revive/health packets.
- The revived player now has modeled MP restored to the current runtime cap, matching Crystal's `MP = Stats[Stat.MP]` surface within the current runtime model.

Verification included:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dead_player_resurrection_scroll_revives_and_consumes_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36)

## Previous Completed Round: R91

R91 added Crystal repair-bind rejection for manifest-backed repair oil use:

- Equipped weapon `DontRepair` blocks `RepairOil` and `WarGodOil`.
- Equipped weapon `NoSRepair` blocks full/special `WarGodOil`.
- Failure paths preserve the oil and leave weapon durability unchanged.

Verification included:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_repair_oils_respect_weapon_repair_binds -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36)

## Previous Completed Round: R90

R90 added Crystal map-rule rejection for manifest-backed teleport scroll use:

- `MapDropRuleRecord` now models `no_escape` and `no_random`.
- `DungeonEscape` / `TeleportHome` shape `0` use rejects on `no_escape` maps with `server.CanNotDungeon`.
- `RandomTeleport` shape `2` use rejects on `no_random` maps with `server.CanNotRandom`.
- Failure paths preserve item and player position.

Verification included:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_dungeon_escape_rejects_on_no_escape_map -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_random_teleport_rejects_on_no_random_map -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (35/35)

## Previous Completed Round: R89

R89 mapped manifest-backed Crystal equipment item types to runtime equipment slots during item creation and `UseItem` fallback, so current manifest equipment use no longer depends on manual test-only slot setup.

Verification included:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_ -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33)

## Previous Completed Round: R88

R88 added manifest-backed normal-potion `shape 0` behavior with modeled delayed timed recovery:

- `UseItem` now consumes and ACKs success for Crystal potion `shape 0` without immediate HP/MP mutation.
- It adds `pending_pot_health_amount` and `pending_pot_mana_amount` to `SimulationResources`.
- `advance_world` now drains those pending values per tick and emits `ObjectHealth` / `ObjectMana` packets until fully restored.
- No Crystal hint-chat was added for this modeled queued-recovery subset.

Verification included:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_normal_potion_queues_timed_restore -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33)

## Previous Completed Round: R87

R87 added manifest-backed current `UseItem` mount-feed parity for `ItemType.Food` (`RawMeat` / `LeanMeat`):

- Manifest-backed `ItemType.Food` now requires an equipped mount to consume and process; missing mount and full mount durability preserve item and state.
- Success now emits `server.MountFed` and `ItemRepaired` hints.
- `RawMeat` shape `0` applies Crystal-style max durability loss before mount feeding; `LeanMeat` shape `1` skips max-dura loss.

Verification included:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_food_requires_equipped_mount -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_food_feeds_equipped_mount -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (32/32)

## Previous Completed Round: R86

R86 added manifest-backed current `UseItem` support for `DungeonEscape` / `TeleportHome` and `RandomTeleport` through scroll-shape `0/2`:

- Success now consumes one item, emits `UseItem` success, refreshes `UserLocation`/`Map`, and performs same-map occupiable destination search when no remote teleport target is configured.
- Failure preserves item/state with ack failure.

Verification included:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_dungeon_escape_teleports_same_map -- --test-threads=1 --nocapture` (9/9)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_random_teleport_teleports_same_map -- --test-threads=1 --nocapture` (30/30)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_ -- --test-threads=1 --nocapture`

## Previous Completed Round: R85

R85 expanded manifest-backed `CanUseItem` parity beyond `R82` level-only checks to the modeled `RequiredType` stat gates used by Crystal.

- Added modeled requirement checks for `MaxAC`, `MaxMAC`, `MaxDC`, `MaxMC`, `MaxSC`, `MinAC`, `MinMAC`, `MinDC`, `MinMC`, `MinSC`, and `MaxLevel` for current equipment-based `UseItem` requirements.
- Crystal source cross-check: `Crystal/Server/MirObjects/HumanObject.cs::CanUseItem`.
- Added focused tests:
  - `use_item_packet_crystal_equipment_rejects_low_max_dc_requirement`
  - `use_item_packet_crystal_equipment_allows_modeled_max_mc_requirement`
- Verification included:
  - `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_rejects_low_max_dc_requirement -- --test-threads=1 --nocapture`
  - `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_allows_modeled_max_mc_requirement -- --test-threads=1 --nocapture`
  - `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_ -- --test-threads=1 --nocapture`
  - `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`
  - `cargo +1.89.0 test --locked -p mir2-simulation use_item_ -- --test-threads=1 --nocapture`
- Latest adjacent full-suite count remains `607`; full-suite rerun is still pending.

## Previous Completed Round: R83

R83 closed the remaining manifest-backed item-use small surface after the completed `R82` current-data parity pass:

- `AncientBanga[Green]` and `AncientBanga[Purple]` now route via Crystal's `UseItem` scroll shape family (`8`, `9`) in the current runtime surface.
- `free_map_shout` / `free_server_shout` flags are now set, and Crystal hint-chat surfaces are emitted in this path.
- Credit-token use hints now localize to `server.CreditsAddedToAccount`.
- Local verification included focused `use_item_packet_`, `use_item`, `equip_item_packet`, `item`, and `storage` regression groups with fmt/diff checks and full `mir2-simulation` pass (`607` tests).

## Previous Completed Round: R82

R82 completed the bounded `CanUseItem` subset required for current item-use parity:

- Added `Gender` and `Class` restriction parity.
- Added `RequiredType == Level` level-guard parity.
- Repeated valid skill-book learn attempts now remain blocked without repeated state mutation.
- Valid skill-book learn attempts now succeed and consume the book.

## Previous Completed Round: R81

R81 closed the next grouped manifest-backed current-data `UseItem` parity bite after the equipment metadata pass:

- Crystal source confirmed `PlayerObject.UseItem` routes current-data potions, buff consumables, town teleports, and repair oils off manifest/template metadata instead of the starter-only local key table, while `MapObject.AddBuff` stacks same-key duration buffs by extending time instead of resetting stats.
- Local runtime still treated most `crystal-item-*` consumables as unknown starter items, could not read template HP/MP or buff stats, and did not share the Crystal stack-duration semantics for the current buff potion family.
- Runtime now routes dynamic manifest-backed current-data items through `use_dynamic_crystal_template_item`, with shared helpers for template HP/MP restore, town teleports, current-data buff extraction, duration stacking, and repair-oil routing.
- Covered current-data `SunPotion`-style instant HP/MP heal, `ImpactDrug` / `Apple` style multi-stat buff items, `TownTeleport`, `BenedictionOil`, `RepairOil`, and the current `WarGodOil` full-repair path. The `WarGodOil` path currently uses a bounded name fallback because the generated manifest still reports `shape = 0` for that item.

R81 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
git -C mir2-web3 diff --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`: passed
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`: passed
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`: passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 599 / 599 passed

## Previous Completed Round: R80

R80 closed the remaining current equipment/item metadata parity gap after the `MysteryWater` / cursed-equipment pass:

- Crystal source confirmed current `NeedIdentify` and `SoulBoundId` are part of the real `UserItem` surface, successful equip/use-equip identifies the item before the ack-visible payload refresh, and items soul-bound to another character cannot be equipped.
- Local runtime still treated those fields as stubs, so current item/equipment round-trips could drop the metadata, identify-on-equip was incomplete, and equip checks could miss the real soul-bound rejection.
- Runtime now carries `identified` and `soul_bound_id` through `ItemState`, `EquipmentState`, and `UserItem` conversions, auto-identifies current items on equip/use-equip with the matching `RefreshItem` ordering, and rejects equipping items bound to a different character id.
- Focused regressions now lock bag equip, storage equip, use-equip identify behavior, and the soul-bound rejection path.

R80 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_equipping_need_identify_item_emits_refresh_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
git -C mir2-web3 diff --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`: passed
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`: passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: passed
- Later full-suite revalidation remained green at `599 / 599` after R81.

## Previous Completed Round: R79

R79 closed the next grouped current equipment/item-storage parity bite after the `RemoveSlotItem` guard pass:

- Crystal source confirmed `PlayerObject.UseItem` potion `shape=2` (`MysteryWater`) unlocks cursed unequip once, consumes only on the first successful use, and repeat use hint-chats without consuming or setting the ack success bit.
- The adjacent `PlayerObject.RemoveItem` and `PlayerObject.EquipItem` paths both reject cursed currently equipped items unless `UnlockCurse` is set, clear the flag again after a successful cursed removal/replacement, and `EquipItem(grid=Storage)` also rejects replacing currently equipped items that cannot be stored back into the exact source slot.
- Local runtime had no `unlock_curse` session flag, no `MysteryWater` branch, no cursed-item current-equipment guard on remove/equip, and no exact replacement reject when the replaced equipment carried `DontStore`.
- Runtime now tracks `unlock_curse` as transient session state, resets it on character/session refresh, implements Crystal `MysteryWater` ack/chat/consume behavior, enforces the cursed remove/equip guards, clears the unlock after successful cursed replacement/removal, and rejects storage-grid replacements when the replaced equipment cannot be stored.

R79 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
git -C mir2-web3 diff --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`: 10 / 10 passed
- `cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture`: 5 / 5 passed
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`: 5 / 5 passed
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`: 188 / 188 passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 41 / 41 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 590 / 590 passed

## Previous Completed Round: R78

R78 closed the next bounded current equipment/item-storage packet mismatch after the storage-grid `EquipItem` / exact-slot `RemoveItem` pass:

- Crystal source confirmed `PlayerObject.RemoveSlotItem` only accepts `grid=Mount` / `Fishing` / `Socket`, resolves the parent item via `idFrom` only for `Socket`, and searches the parent item's `Slots` collection instead of falling through into whole-equipment removal.
- Local runtime still ignored the source grid and `from_unique_id`, so `RemoveSlotItem(grid=Equipment, ...)` and `RemoveSlotItem(grid=Socket, unique_id=<parent equipment id>, from_unique_id=<same>)` could remove the entire equipped weapon.
- Runtime now keeps `from_unique_id` at dispatch, rejects non-Crystal source grids, and keeps currently unmodeled `Mount` / `Fishing` / `Socket` slot-item requests on the failed ack surface instead of mutating equipment.
- Focused regressions now lock the invalid `Equipment` source-grid path plus the `Socket` parent-equipment-id fallback mismatch, and the full simulation suite remains green.

R78 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
git -C mir2-web3 diff --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture`: 3 / 3 passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 39 / 39 passed
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`: 183 / 183 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 584 / 584 passed

## Previous Completed Round: R76

R76 closed the remaining expanded-storage expiry downgrade mismatch after the backing-length storage payload pass:

- Crystal source confirmed `BuildUserInformation` reports expanded storage as active only while `ExpandedStorageExpiryDate > Envir.Now`, and `PlayerObject` later clears `Account.HasExpandedStorage`, emits the `ExpandedStorageExpired` system chat, and enqueues `ResizeStorage` on the first process tick after expiry.
- Local runtime still trusted the stored `has_expanded_storage` flag directly during `StartGame`, so expired accounts could report expanded storage as active forever and never emit the Crystal expiry notice or persist the flag back to `false`.
- Runtime now treats expired expanded storage as inactive during account-state refresh and current `StartGame`, schedules a one-shot expiry notice for the first world tick, emits the chat plus `ResizeStorage`, and persists the account flag back to `false` while keeping the 160-slot backing storage length intact.
- Focused regressions now lock the inactive-on-start-game packet shape plus the first-tick expiry notice/persistence surface, and the full simulation suite remains green.

R76 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation expired_expanded_storage_is_inactive_on_start_game_but_keeps_backing_size -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation expired_expanded_storage_tick_emits_resize_notice_once_and_persists_flag -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
git -C mir2-web3 diff --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation expired_expanded_storage_is_inactive_on_start_game_but_keeps_backing_size -- --test-threads=1 --nocapture`: 1 / 1 passed
- `cargo +1.89.0 test --locked -p mir2-simulation expired_expanded_storage_tick_emits_resize_notice_once_and_persists_flag -- --test-threads=1 --nocapture`: 1 / 1 passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 37 / 37 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 579 / 579 passed

## Previous Completed Round: R75

R75 closed the remaining backing-length storage payload mismatch after the resend-suppression pass:

- Crystal source confirmed `PlayerObject.SendStorage()` enqueues `Account.Storage` at its full backing length, while `AccountInfo.IsValidStorageIndex()` separately gates whether higher storage slots are currently accessible when `HasExpandedStorage == false`.
- Local runtime still sized `UserStorage` from the currently accessible slot count, so accounts whose backing storage remained length `160` after expanded access ended emitted truncated `80`-slot `UserStorage` payloads instead of Crystal's full backing array.
- Runtime now sizes outgoing `UserStorage` from the normalized backing storage length while keeping storage item actions gated by the existing current accessible-capacity helper, matching Crystal's split between packet payload shape and slot validation.
- Focused regressions now lock the current `@Storage` open payload length and preserve the blocked higher-slot `StoreItem` surface when expansion is inactive, and the full simulation suite remains green.

R75 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
git -C mir2-web3 diff --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 35 / 35 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 577 / 577 passed

## Previous Completed Round: R74

R74 closed the remaining duplicate-storage-send mismatch after the `UserStorage` follow-up packet pass:

- Crystal source confirmed storage opens and successful unlocks both route through `PlayerObject.SendStorage()`, and that helper suppresses duplicate `UserStorage` resends via `Connection.StorageSent` until a locked open clears the flag back to `false`.
- Local runtime still emitted `UserStorage` on every unchanged successful `@Storage` open and on every successful unlock result `0`, so repeated opens did not match Crystal's no-resend surface.
- Runtime now keeps a session-level storage-send flag, resets it on character/session refresh, clears it on locked storage opens, and reuses one Crystal-style helper for both storage open and unlock follow-up packet decisions.
- Focused regressions now lock the first-open vs repeated-open packet split, and the full simulation suite remains green.

R74 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
git -C mir2-web3 diff --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 34 / 34 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 576 / 576 passed

## Previous Completed Round: R73

R73 closed the missing Crystal storage follow-up packet surface after the password and unlock-state cleanup rounds:

- Crystal source confirmed `NPCScript.StorageKey` resets unlock state, calls `SendStorage()`, then enqueues `NPCStorage`; `MirConnection.UnlockStorage` enqueues successful `StorageUnlockResult` and immediately follows it with `Player.SendStorage()`.
- Local protocol/gateway/runtime only exposed `NPCStorage` and `StorageUnlockResult`, so successful current storage open and unlock never emitted the Crystal `UserStorage` packet carrying the slot-indexed storage contents.
- Protocol now carries Crystal `UserStorage` as the nullable slot-array packet, gateway exposes it through JSON, runtime emits it before `NPCStorage` on successful open when storage is available, and successful `UnlockStorage(result=0)` now follows with `UserStorage`.
- Focused regressions now lock the current storage open packet order plus the successful unlock follow-up, while full `mir2-simulation` regression remains green.

R73 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-protocol --test codec
cargo +1.89.0 test --locked -p mir2-gateway
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-protocol --test codec`: 31 / 31 passed
- `cargo +1.89.0 test --locked -p mir2-gateway`: 46 / 46 passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 33 / 33 passed
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture`: 1 / 1 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 575 / 575 passed

## Previous Completed Round: R72

R72 closed the session-reset storage-open gap after the password-format pass:

- Crystal source confirmed `NPCScript.StorageKey` calls `ResetStorageUnlock()` before `SendStorage()`, so reopening storage after a successful unlock must relock the session and suppress contents until the next successful unlock.
- Local runtime previously preserved `storage_unlocked` across repeated `@Storage` opens, so a stale unlocked session could keep storage available without matching Crystal's reopen semantics.
- Reopening `@Storage` now resets the local session unlock state before deciding whether contents can be sent, matching Crystal's `ResetStorageUnlock()` behavior.

R72 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
git -C mir2-web3 diff --check
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 33 / 33 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 575 / 575 passed

## Previous Completed Round: R71

R71 closed the remaining storage password-format mismatch:

- Crystal storage password flows accept only `^[A-Za-z0-9]{5,15}$` passwords for set, unlock, and remove operations.
- Local runtime still accepted runtime-only password shapes outside that Crystal format envelope.
- Current storage password set/unlock/remove now all enforce the Crystal alphanumeric 5-15 character rule, with focused regressions for invalid and wrong-password branches.

R71 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation storage_password -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation storage_password -- --test-threads=1 --nocapture`: 5 / 5 passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 32 / 32 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 574 / 574 passed

## Previous Completed Round: R70

R70 closed the remaining current storage password service-context mismatch:

- Crystal storage password actions require the active in-range storage service context instead of trusting stale page state alone.
- Crystal also clears the storage password `LastSetTime` back to `0` after successful password removal.
- Local runtime now requires the active in-range storage service for password actions and clears the persisted last-set timestamp on successful removal.

## Previous Completed Round: R69

R69 closed the remaining current-data `CombineItem` manifest-slice backlog:

- Current inventory-grid `CombineItem` coverage now closes the remaining present-data shape-3/4 families surfaced by the current manifest.
- The same pass also locks the shape-0 source surface as Crystal's failed-ack-only path instead of leaving it unverified.
- Full `mir2-simulation` regression was green at 571 tests after the pass.

## Previous Completed Round: R68

R68 closed the next real current-data `CombineItem` gap after the R67 NPC service-context cleanup:

- Crystal source audit across `PlayerObject.CombineItem`, `GetGemType`, and `HumanObject.GetCurrentStatCount` confirmed current shape-3/4 durability gems/orbs use `Info.Durability` for the applied upgrade path and treat stat `48` / `HPDrainRatePercent` as the max-added-stats control field.
- Current Rust runtime still treated positive stat `48` as the applied upgrade stat when no earlier stat family matched, so `DurabilityGem` / `DurabilityOrb` incorrectly added stat `48` instead of increasing `MaxDura`, and the durability max-added-stats cap was never reached through the Crystal path.
- Runtime now keeps stat `48` out of the applied upgrade-stat detector for current gem/orb routing, so durability gems/orbs fall through to the Crystal `MaxDura` branch while the current-data attack-speed and magic-resist families keep using the existing stat upgrade flow.
- Focused regressions now lock successful `DurabilityOrb`, `StormOrb`, and `DisillusionGem` upgrades plus the durability-cap rejection surface.

R68 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation combine_item -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation combine_item -- --test-threads=1 --nocapture`: 22 / 22 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 565 / 565 passed

## Previous Completed Round: R67

R67 closed the remaining implemented NPC item-service live-object/range gap after R66:

- Crystal source confirmed current `BuyItem`, `SellItem`, and `RepairItem` / `SRepairItem` all re-check the recorded `NPCObjectID` and abort when the corresponding NPC object is gone or outside `Globals.DataRange`, instead of trusting sticky page/service state alone.
- Current Rust runtime still used the recorded service label/page for those item-service actions without revalidating the backing NPC object, so stale or out-of-range context could keep mutating current buy/sell/repair flows after the player moved away or the NPC disappeared.
- Runtime now reuses the shared live-NPC/range service gate across current `BuyItem`, `SellItem`, and `RepairItem` / `SRepairItem`, preserving Crystal's existing packet surfaces while blocking mutation when the recorded service NPC is gone or no longer within `CRYSTAL_DATA_RANGE`.
- Focused regressions now lock the out-of-range `BuyItem` surface, the missing-NPC `SellItem` surface, the missing-NPC `RepairItem` surface, and the out-of-range `SRepairItem` surface.

R67 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation buy_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation sell_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation repair_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation buy_item -- --test-threads=1 --nocapture`: 4 / 4 passed
- `cargo +1.89.0 test --locked -p mir2-simulation sell_item -- --test-threads=1 --nocapture`: 11 / 11 passed
- `cargo +1.89.0 test --locked -p mir2-simulation repair_item -- --test-threads=1 --nocapture`: 8 / 8 passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 29 / 29 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 561 / 561 passed

## Previous Completed Round: R66

R66 closed the next real storage-context parity gap after Crystal source audit ruled out the queued current `MergeItem` cross-grid target as unmodeled:

- Crystal source confirmed current storage-family item handlers (`StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage`) all re-check that the recorded `NPCObjectID` still resolves to a live NPC within `Globals.DataRange` before mutating storage.
- The same audit showed the queued current `MergeItem` `Inventory <-> Equipment` amulet-only and `Inventory <-> Fishing` bait-only paths are not currently expressible locally because `EquipmentState` has no stack quantity and there is still no fishing slot collection in runtime state.
- Current Rust storage gating still only checked the sticky active-service label key, so stale or out-of-range storage service context could keep mutating storage-family item actions after the player moved away or the NPC object disappeared.
- Runtime now records the service-opening NPC object id, validates that the storage NPC still exists and remains within `CRYSTAL_DATA_RANGE`, and applies that shared gate across current `StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage`; focused regressions now lock both the out-of-range and missing-NPC surfaces.

R66 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_storage_service_context_rejects_storage_actions_when_player_leaves_data_range -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage_service_context_requires_live_npc_object -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_storage_service_context_rejects_storage_actions_when_player_leaves_data_range -- --test-threads=1 --nocapture`: 1 / 1 passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage_service_context_requires_live_npc_object -- --test-threads=1 --nocapture`: 1 / 1 passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 29 / 29 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 557 / 557 passed

## Previous Completed Round: R65

R65 closed the remaining bounded `SplitItem` message-shape/support gap after R64:

- Crystal source confirmed `PlayerObject.SplitItem` only supports `Inventory` and `Storage`, requires active storage-service context for storage splits, and keeps unsupported/invalid/full/locked branches on the failed `SplitItem1` ack with no extra chat.
- Current Rust `split_item_impl` still allowed `Belt` splits, still allowed storage splits without the active storage service, and still emitted runtime-only chat for zero-count, full-stack, no-free-slot, and locked-storage failures.
- Runtime now supports only Crystal `Inventory` / `Storage` split grids, requires the active storage service for storage splits, and keeps unsupported/invalid/full/locked split failures ack-only; focused regressions now lock the belt-grid, zero-count, inactive-service, and locked-storage surfaces.

R65 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`: 8 / 8 passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 27 / 27 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 555 / 555 passed

## Previous Completed Round: R64

R64 closed the next bounded current inventory-array placement gap on `SplitItem(grid=Inventory)`:

- Crystal source confirmed `PlayerObject.SplitItem` finds the source by unique id in the single `Info.Inventory` array, prefers eligible potion/scroll/script and amulet belt slots first, then scans the full bag array instead of staying on the source page.
- Current Rust `split_item_impl` still searched only the source local container, so `Bag1` splits could fail despite free `Bag2` space, `Bag2` splits could ignore earlier `Bag1` slots, and belt-eligible inventory splits still missed the Crystal belt-first placement rule.
- Runtime now routes inventory splits through the existing Crystal-style empty-slot helper, enabling `Bag1 -> Bag2`, `Bag2 -> Bag1`, and belt-first placement for eligible inventory splits; focused regressions now lock those three placement edges.

R64 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`: 5 / 5 passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 26 / 26 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 552 / 552 passed

## Previous Completed Round: R63

R63 closed the next bounded current inventory-array packet gap on slot-based bag paths:

- Crystal source confirmed `MoveItem`, `StoreItem`, and `TakeBackItem` all index the single `Info.Inventory` array directly instead of searching separate `Bag1` / `Bag2` containers by matching slot number.
- Current Rust runtime still treated local `Bag1` / `Bag2` same-slot entries as interchangeable aliases on those packet paths and still rejected inventory slots `40+`, even though local `Bag2` items already represented the second inventory page.
- Runtime now routes slot-based current inventory selection through Crystal-style single-array indices across local `Bag1` / `Bag2`, enabling `Bag2` move/store/take-back paths on slots `40+` and preventing same-slot cross-page aliasing; focused regressions now lock all three packet families.

R63 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation crystal_inventory_index_for_bag2 -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation crystal_inventory_index_for_bag2 -- --test-threads=1 --nocapture`: 3 / 3 passed
- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`: 23 / 23 passed
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`: 26 / 26 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 549 / 549 passed

## Previous Completed Round: R62

R62 closed the remaining unsupported current `MergeItem` cross-grid message-shape gap on the modeled belt/storage surface:

- Crystal source confirmed unsupported `MergeItem` cross-grid combinations fall through to the failed ack with no extra chat.
- Current Rust `merge_item_impl` still emitted the runtime-only `Cross-grid item merge is not available yet.` chat for `Storage -> Belt` and `Belt -> Storage`.
- Runtime now keeps those remaining unsupported cross-grid requests ack-only, and focused regressions now lock both directions against extra chat or item mutation.

R62 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`: 24 / 24 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 546 / 546 passed

## Previous Completed Round: R61

R61 closed the next bounded current `MergeItem` unsupported-grid parity bite:

- Crystal source confirmed `PlayerObject.MergeItem` has no `QuestInventory` branch, so those requests fall through to the failed ack with no extra chat or quest mutation.
- Current Rust `merge_item_impl` still allowed same-grid `QuestInventory` merges and still appended runtime-only cross-grid chat when `QuestInventory` was involved.
- Runtime now short-circuits any `MergeItem` touching `QuestInventory` to the failed Crystal-shaped ack, and focused regressions now lock both the same-grid quest merge attempt and an inventory-to-quest cross-grid request against extra chat or mutation.

R61 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`: 22 / 22 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 544 / 544 passed

## Previous Completed Round: R60

R60 closed the next bounded current `MoveItem` inventory-array parity gap:

- Crystal source confirmed `PlayerObject.MoveItem` has no `Belt` or `QuestInventory` branch, checks current `Inventory` slot bounds against the real bag array, and never consults quest inventory items during an ordinary bag move.
- Current Rust `move_item_impl` still allowed `Belt` and `QuestInventory` move requests, accepted out-of-range current inventory slots, and could mutate quest items because both bag and quest items lived in the same local vector.
- Runtime now rejects `Belt` / `QuestInventory` `MoveItem` requests ack-only, enforces current inventory slot bounds, and scopes current bag moves away from quest items; focused regressions now lock those branches plus the quest-slot collision case.

R60 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`: 22 / 22 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 542 / 542 passed

## Previous Completed Round: R59

R59 closed the next bounded current `MoveItem` message-shape gap:

- Crystal source confirmed current `MoveItem` missing-source failures on `Inventory` and `Storage` first report `ServerTextKeys.ItemMoveErrorReport`, then enqueue the failed ack.
- Current Rust `move_item_impl` still emitted the generic `sim.itemNotFoundInBag` chat on those same current paths.
- Runtime now uses Crystal's localized `ItemMoveErrorReport` surface for current missing-source `Inventory` / `Storage` moves, and focused regressions now lock both the bag and storage branches.

R59 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`: 17 / 17 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 537 / 537 passed

## Previous Completed Round: R58

R58 closed the next bounded current `MoveItem` message-shape gap:

- Crystal source confirmed successful `MoveItem` swaps only enqueue the success ack; Crystal does not emit an additional success chat after a current move completes.
- Current Rust `move_item_impl` still appended the runtime-only `Item slot updated.` chat after successful current moves.
- Runtime now keeps successful current `MoveItem` Inventory/Storage paths ack-only, and focused regressions now lock both the storage reorder and the current inventory-slot reorder surfaces.

R58 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`: 15 / 15 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 535 / 535 passed

## Previous Completed Round: R57

R57 closed the next bounded current `MoveItem(grid=Storage)` gating gap:

- Crystal source confirmed `MoveItem(grid=Storage)` first requires the active Crystal `@Storage` / `NPCStorage` service context before any storage mutation is attempted.
- Current Rust `move_item_impl` still allowed storage-slot reorders with no active storage service, diverging from the other current storage item actions that already honored the Crystal service gate.
- Runtime now requires the active storage service for current `MoveItem(grid=Storage)`, and focused regressions now lock the inactive-service failed-ack surface plus the existing gated success case.

R57 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`: 14 / 14 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 534 / 534 passed

## Previous Completed Round: R56

R56 closed the next bounded current `MoveItem` message-shape gap:

- Crystal source confirmed current `MoveItem` storage-lock and invalid-slot branches enqueue only the failed ack; Crystal does not emit extra chat for those failures.
- Current Rust `move_item_impl` still appended runtime-only `Storage is locked.`, `Invalid target item slot.`, and `Invalid source item slot.` chat on those same branches.
- Runtime now keeps those current `MoveItem` failures ack-only, and focused regressions now lock the negative-source, negative-target, and invalid storage source/target surfaces alongside the locked-storage reorder case.

R56 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`: 13 / 13 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 533 / 533 passed

## Previous Completed Round: R55

R55 closed the next bounded current `MoveItem` unsupported-grid parity bite:

- Crystal source confirmed `PlayerObject.MoveItem` supports only `Inventory`, `Storage`, `Trade`, `Refine`, and `HeroInventory`; other grids, including `Equipment`, `Fishing`, and `HeroEquipment`, fall through to the failed ack with no extra chat or mutation.
- Current Rust `move_item_impl` still emitted the runtime-only `That item grid cannot be moved yet.` chat for those requests.
- Runtime now short-circuits unmodeled `MoveItem` grids to the failed Crystal-shaped ack, and focused regressions now lock `HeroEquipment`, `Equipment`, and `Fishing` against extra chat or mutation.

R55 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`: 9 / 9 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 529 / 529 passed

## Previous Completed Round: R54

R54 closed the next bounded current modeled `MergeItem` cross-grid bite after the verified `Inventory <-> Storage` surface:

- Crystal source confirmed literal `MergeItem(grid=Equipment|Fishing)` uses true equipment and fishing-slot arrays, but the local runtime audit found those paths still need new modeling because equipped gear has no stack quantity and fishing-rod slot arrays are not represented.
- The current runtime does model Crystal belt-priority stackables as `belt_items`, so the coordinator promoted the next bounded local equivalent: `Inventory <-> Belt` stack merges for Crystal belt-eligible items.
- Runtime now supports `Inventory -> Belt` and `Belt -> Inventory` stack merges for Crystal belt-eligible items, keeps non-beltable belt cross-grid requests ack-only, and adds focused regressions for both success directions plus the non-beltable `FishBait` failure.

R54 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`: 20 / 20 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 529 / 529 passed

## Previous Completed Round: R53

R53 closed the next bounded current modeled `MergeItem` feature gap:

- Crystal source confirmed `PlayerObject.MergeItem` supports cross-grid stack merges between `Inventory` and `Storage`, but still requires an active `@Storage` page and storage access checks on either side.
- Current Rust `merge_item_impl` still rejected all cross-grid merges with a runtime-only message, and same-grid storage merges did not require the storage service to be active.
- Runtime now supports current `Inventory -> Storage` and `Storage -> Inventory` stack merges for matching stackables, requires the active Crystal storage service whenever `Storage` is involved, preserves ack-only inactive/locked failures, and keeps current same-grid storage merges behind the same gate.
- Added focused regressions proving inventory-to-storage and storage-to-inventory red-potion merges succeed when the storage page is active, while inactive-service requests fail ack-only and preserve both collections.

R53 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`: 17 / 17 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 523 / 523 passed

## Previous Completed Round: R52

R52 closed the next bounded current `MergeItem` message-shape gap:

- Crystal source confirmed current Inventory/Storage `MergeItem` failures for storage lock, missing item, mismatched stacks, full targets, and other rejection branches all enqueue only the failed ack; successful merges also do not emit runtime chat.
- Current Rust `merge_item_impl` still attached runtime-only `Storage is locked`, `sim.itemNotFoundInBag`, `Only matching item stacks can be merged`, and `Item stacks merged` chat/messages on those same current paths.
- Runtime now keeps those current `MergeItem` branches ack-only, matching Crystal's packet-visible surface more closely without changing the actual stack mutation logic.
- Added focused regressions for missing-source, mismatched-stack, and full-target failures, and updated the storage lock/success merge regressions to assert no extra chat.

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`: 14 / 14 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 520 / 520 passed

## Previous Completed Round: R51

R51 closed the next bounded current `MergeItem` unsupported-grid parity bite:

- Crystal source confirmed `Trade` and `Refine` both fall through `PlayerObject.MergeItem` to the failed ack with no extra chat or player-bag mutation.
- Current Rust `merge_item_impl` still emitted runtime-only cross-grid/grid-not-supported chat for those requests.
- Runtime now short-circuits any `MergeItem` touching `Trade` or `Refine` to the failed Crystal-shaped ack with no extra chat and no matching player-stack mutation.
- Added focused regressions proving both same-grid and inventory-to-grid requests leave the player's potion stacks untouched.

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`: 11 / 11 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 517 / 517 passed

## Previous Completed Round: R50

R50 closed the next bounded current `MergeItem` unsupported-grid parity bite:

- Crystal source confirmed current `MergeItem` also routes `Equipment` and `Fishing` through the real array-selection branches, but when those surfaces are unavailable or unsupported they still collapse to the failed ack without runtime chat.
- Current Rust `merge_item_impl` still emitted runtime-only cross-grid/grid-not-supported chat for `Equipment` and `Fishing`, even though those surfaces remain unmodeled.
- Runtime now short-circuits any `MergeItem` touching `HeroInventory`, `HeroEquipment`, `Equipment`, or `Fishing` to the failed Crystal-shaped ack with no extra chat and no matching player-stack mutation.
- Added focused regressions proving inventory-to-equipment and inventory-to-fishing merge requests leave matching player potion stacks unchanged.

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`: 7 / 7 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 513 / 513 passed

## Previous Completed Round: R48

R48 locked the next bounded hero-grid item packet guard:

- Crystal source confirmed `PlayerObject.MoveItem` accepts `HeroInventory` only when a current hero exists/spawns; otherwise it immediately enqueues the failed ack with no extra chat.
- Current Rust `move_item_impl` still treated `HeroInventory` as an unsupported grid and emitted a runtime-only chat message.
- Runtime now short-circuits `MoveItem(grid=HeroInventory)` to the failed Crystal-shaped ack without extra chat or player-bag mutation.
- Added a focused regression proving the matching player `bronze-helmet` bag item stays in place under the hero-grid move request.

R48 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`: 4 / 4 passed
- `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`: 11 / 11 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 509 / 509 passed

## Previous Completed Round: R47

R47 locked the next bounded hero-grid item packet guard:

- Crystal source confirmed `PlayerObject.MergeItem` selects source/target arrays by grid and, for `HeroInventory` / `HeroEquipment`, immediately enqueues the failed ack when no hero is present/spawned.
- Current Rust `merge_item_impl` still returned runtime-only chat for unsupported or cross-grid hero requests, which diverged from Crystal even though player inventory stayed unmodified.
- Runtime now short-circuits any `MergeItem` request touching `HeroInventory` or `HeroEquipment` to the failed Crystal-shaped ack with no extra chat and no mutation.
- Added focused regressions proving both hero-to-hero and inventory-to-hero merge requests leave matching player potion stacks unchanged.

R47 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`: 5 / 5 passed
- `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`: 10 / 10 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 508 / 508 passed

## Previous Completed Round: R46

R46 locked the next bounded hero-grid item packet guards:

- Crystal source confirmed `PlayerObject.EquipItem`, `PlayerObject.RemoveItem`, and `PlayerObject.RemoveSlotItem` dispatch hero-grid requests through current hero inventory/equipment only; when no hero is present/spawned, they enqueue the failed ack and return without touching player bag/equipment.
- Current Rust runtime still let those hero-grid requests route into current player inventory/equipment helpers, so matching player items could be equipped or removed.
- Runtime now short-circuits `EquipItem(grid=HeroInventory)`, `RemoveItem(grid=HeroInventory)`, and `RemoveSlotItem(grid|grid_to=HeroEquipment|HeroInventory)` to the failed Crystal-shaped ack without extra mutation.
- Added focused regressions proving those hero-grid packets leave the matching player helmet/weapon state unchanged.

R46 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation remove_item_packet_hero_inventory_grid_does_not_mutate_matching_player_equipment -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item_packet_hero_equipment_grid_does_not_mutate_matching_player_equipment -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation equip_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`: 8 / 8 passed
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item -- --test-threads=1 --nocapture`: 2 / 2 passed
- `cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture`: 2 / 2 passed
- `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture`: 1 / 1 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 506 / 506 passed

## Previous Completed Round: R45

R45 locked the next bounded hero-inventory item packet guard:

- Crystal source confirmed `PlayerObject.SplitItem` only accepts `Inventory` and `Storage`; `HeroInventory` falls into the default failed `SplitItem1` ack path with no player bag mutation.
- Current Rust `split_item_impl` still routed unsupported grids through player inventory matching, so `SplitItem(grid=HeroInventory)` could split a matching player stack.
- Runtime now short-circuits `HeroInventory` to the failed `SplitItem1` ack without mutating player inventory.
- Added a focused regression proving `SplitItem(grid=HeroInventory)` leaves the matching player red-potion stack unchanged.

R45 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation split_item_packet_hero_inventory_grid_does_not_mutate_matching_player_stack -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`: 2 / 2 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 503 / 503 passed

## Previous Completed Round: R44

R44 locked the matching hero-inventory `UseItem` guard:

- Crystal source confirmed `MirConnection.UseItem` dispatches `MirGridType.HeroInventory` through `Player.HeroUseItem`, never through player bag lookup.
- Current Rust `UseItem` packet handling still resolved non-belt grids against player inventory, so `UseItem(grid=HeroInventory)` could consume matching player bag items.
- Runtime now short-circuits `HeroInventory` instead of falling back into player bag items.
- Added a focused regression proving `UseItem(grid=HeroInventory)` leaves the matching player potion stack unchanged.

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`: 8 / 8 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 502 / 502 passed before R45 extended the suite.

## Previous Completed Round: R43

R43 aligned Crystal current `ResurrectionScroll` map rejection:

- Crystal source confirmed shape-6 `ResurrectionScroll` first rejects `CurrentMap.Info.NoReincarnation` with `CannotUseOnMap`, then only revives if the user is actually dead.
- Runtime already handled alive-vs-dead `ResurrectionScroll`, but dead players on blocked maps still revived and consumed the item.
- Added `no_reincarnation` to the bounded map-rule config surface, wired the dead-player `ResurrectionScroll` path through it, and added a focused regression that proves the scroll is preserved and no revive packets are emitted on blocked maps.

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`: 7 / 7 passed
- `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`: 9 / 9 passed
- `cargo +1.89.0 test --locked -p mir2-simulation town_teleport -- --test-threads=1 --nocapture`: 2 / 2 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 501 / 501 passed before later R44/R45 hero-grid guards landed.

## R25 Completed State

R25 is complete and already accounted for. Do not rerun or reopen it unless current tests or code inspection show a regression.

Crystal source audit confirmed:

- `C.StoreItem` carries `from` and `to`; `S.StoreItem` returns `from`, `to`, and `success`.
- `C.TakeBackItem` carries `from` and `to`; `S.TakeBackItem` returns `from`, `to`, and `success`.
- Crystal gates both actions on active `[@STORAGE]`, NPC range, and `CanAccessStorage`.
- Store failure order is page/range/access, source/target bounds, `IsValidStorageIndex`, source item exists, `DontStore` / rental `DontStore`, then target slot empty.
- TakeBack failure order is page/range/access, source storage bounds, `IsValidStorageIndex`, target inventory bounds, source item exists, then target slot empty.
- Store target occupied fails; TakeBack target occupied fails. There is no swap.
- Store/TakeBack failures are ack-only `success=false` with no chat message.
- Store blocks base bind `DontStore` and rental `DontStore`; TakeBack has no bind/rental check.
- Current Rust simulation still models the service-context branch rather than a full NPC object/range check, but it now preserves the real `NPCStorage` activation path used by imported `@Storage` dialogs.

Implemented code/results:

- Added Crystal `DontStore` bind constant.
- Added storage active-service helper and inventory-slot validation helper.
- Reworked `store_item_impl` to require active storage service, return ack-only failures, reject storage lock, invalid slots, inaccessible storage slot, missing item, `DontStore`, and occupied target.
- Reworked `take_back_item_impl` to require active storage service, return ack-only failures, reject storage lock, invalid slots, inaccessible storage slot, missing item, and occupied target.
- Recorded `NPCStorage` in the normal service-context activation path so a real `@Storage` dialog can store/take back without the test-only helper.
- Added an end-to-end regression that opens the imported storage page and proves store/take-back succeeds through the actual NPC flow.
- Added a Unix/Mac `crystal_local_time_snapshot()` implementation using `libc`; the full suite exposed a pre-existing non-Windows test gap in current NPC time-condition coverage.
- Added direct `libc = "0.2"` in `apps/simulation/Cargo.toml` and refreshed `Cargo.lock`.

Rust Explorer audit completed:

- Packet dispatch is direct: `ClientPacket::StoreItem` / `TakeBackItem` route to `store_item_impl` / `take_back_item_impl`.
- The new active-service gate only accepts `active_npc_service.label_key == "STORAGE"`.
- `record_crystal_npc_service_context` now records `NPCStorage`, closing the real-dialog activation gap that previously only test helpers covered.

R25 verification completed:

```powershell
cd E:\mir2\mir2-web3
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-simulation crystal_npc_storage_service_context_allows_store_and_take_back_without_helper -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation crystal_npc_time_and_bag_conditions_follow_runtime_state -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`: 16 / 16 passed
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`: 72 / 72 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 458 / 458 passed

## Last Completed Round: R38

R38 aligned Crystal current monster-drop map-rule suppression:

- Crystal source confirmed both `MonsterObject.Drop()` and `MonsterObject.DropItem(UserItem item)` return immediately when `CurrentMap.Info.NoDropMonster` is set.
- Current Rust defeat flow still allowed normal monster drops, the deterministic field-wasp quest drop, and harvest pending loot on blocked maps.
- Runtime now suppresses all three current loot surfaces when the map disallows monster drops.
- Added focused regressions proving a blocked map suppresses field-wasp quest loot and makes harvest corpses end with `Nothing was found.` instead of transferring items.
- Remaining related gap is player/hero death-drop `NoDropPlayer` parity, which is larger because the current Rust runtime still lacks a full Crystal death-drop surface.

R38 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation no_drop_monster_map_rule -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation harvest -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation drop -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation no_drop_monster_map_rule -- --test-threads=1 --nocapture`: 2 / 2 passed
- `cargo +1.89.0 test --locked -p mir2-simulation harvest -- --test-threads=1 --nocapture`: 8 / 8 passed
- `cargo +1.89.0 test --locked -p mir2-simulation drop -- --test-threads=1 --nocapture`: 38 / 38 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 490 / 490 passed

## Previous Completed Round: R37

R37 aligned Crystal current `DropItem` map `NoThrowItem` rejection:

- Crystal source confirmed `PlayerObject.DropItem` checks `CurrentMap.Info.NoThrowItem` before inventory lookup, emits localized `CanNotDrop` system chat, then enqueues the failed ack.
- Runtime now mirrors that order for the current modeled player inventory path.
- Added a focused regression proving the blocked map path preserves inventory state, spawns no ground drop, and returns the localized chat plus failed ack.
- Remaining broader map-drop work after R37 was `NoDropMonster`, which R38 subsequently closed for the current monster-death and harvest surfaces.

R37 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`: 8 / 8 passed
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`: 100 / 100 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 488 / 488 passed

## Previous Completed Round: R36

R36 aligned the bounded Crystal current `DropItem` rental `DontDrop` edge:

- Crystal source confirmed `PlayerObject.DropItem` rejects both base `Info.Bind.HasFlag(BindMode.DontDrop)` and rental `RentalInformation.BindingFlags.HasFlag(BindMode.DontDrop)` before any mutation.
- Runtime already preserved rental binding flags on current inventory items, but `drop_item_packet` only rejected base Crystal `DontDrop`.
- Runtime now reuses the shared Crystal-or-rental bind helper so `DropItem` rejects rental `BindingFlags.DontDrop` ack-only like Crystal.
- Added a focused regression that proves rental `DontDrop` preserves inventory state, preserves rental metadata, and spawns no ground drop.
- Remaining bounded gaps include current map `NoThrowItem` rejection/message parity and broader hero-inventory/map-flag work.

R36 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`: 7 / 7 passed
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`: 99 / 99 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 487 / 487 passed

## Previous Completed Round: R35

R35 locked the bounded Crystal hero-inventory packet guards:

- Crystal source confirmed `DropItem(hero_inventory=true)` searches hero inventory only and, when no current hero exists, simply returns the failed ack without touching player inventory.
- Crystal source confirmed `CombineItem(grid=HeroInventory)` only uses `CurrentHero.Inventory` when `HasHero && HeroSpawned`; otherwise it returns the failed ack without mutating player inventory.
- Current Rust runtime already matched those bounded semantics, so the round stayed read-mostly and focused on regression locking rather than a larger hero-system implementation.
- Added focused regressions proving `DropItem(hero_inventory=true)` and `CombineItem(grid=HeroInventory)` do not mutate matching player inventory items when hero inventory is unavailable.
- Remaining bounded gaps moved to current `DropItem` rental and map-flag behavior rather than broader hero modeling.

R35 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`: 4 / 4 passed
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`: 98 / 98 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 486 / 486 passed

## Previous Completed Round: R32

R32 aligned current inventory unique-id lookup behavior:

- Crystal source confirmed `PlayerObject.CombineItem`, `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, and `RepairItem` all locate current inventory items by `UserItem.UniqueID`.
- Runtime now carries an optional `ItemState.unique_id` field with compatibility fallback logic instead of hardwiring client-visible ids to raw inventory slot numbers.
- Current inventory-grid `CombineItem` now resolves source and target items by unique id and emits target-side result packets with the resolved target unique id.
- Current `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, and `RepairItem` bag-item paths now also resolve inventory items by unique id instead of raw slot aliases.
- Default `Bag1` / `Bag2` fallback ids are now distinct for same-slot items, removing the starter inventory collision where two different bag-page items could surface the same client id.
- Split-stack clones now receive a fresh default unique id for the destination slot instead of reusing the source item id.
- Remaining bounded gaps include hero-inventory handling, move/merge unique-id parity, and other gem-family branches.

R32 verification commands:

```powershell
cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture
cargo +1.89.0 fmt --check
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture`: 7 / 7 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 479 / 479 passed

## Previous Completed Round: R31

R31 aligned the current player `GemRatePercent` combine-upgrade hook:

- Crystal source confirmed shape-3/4 `CombineItem` computes `successchance = CriticalRate - adjusted + Stats[Stat.GemRatePercent]`.
- Runtime now sums current non-broken equipment `UserItemStat` entries for `GemRatePercent` and passes that bonus into the existing Crystal-shaped upgrade success formula.
- Added a deterministic focused regression that finds a tick where the base chance fails but the `GemRatePercent`-boosted chance succeeds, then verifies `ItemUpgraded`, `gem_count`, and the target added stat.
- Remaining bounded combine gaps include hero-inventory handling, belt/id-collision cleanup, and other gem-family branches.

R31 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-simulation combine_item_packet_upgrade_branch_applies_player_gem_rate_percent_bonus -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test -p mir2-simulation combine_item_packet_upgrade_branch_applies_player_gem_rate_percent_bonus -- --test-threads=1 --nocapture`: 1 / 1 passed
- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`: 14 / 14 passed
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`: 17 / 17 passed
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`: 86 / 86 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 473 / 473 passed

## Previous Completed Round: R30

R30 aligned the current rental binding flag item paths:

- Added runtime persistence for `UserItemRentalInformation.BindingFlags` through item/equipment state, inventory/equipment round-trips, and `UserItem.RentalInformation` payload generation.
- `StoreItem` now rejects both base `DontStore` and rental `DontStore`, matching Crystal's storage bind checks.
- Current inventory-grid `CombineItem` shape-7 socket and shape-3/4 upgrade branches now reject rental `DontUpgrade` ack-only, preserving the source item and target state.
- The round intentionally did not add a seal rental check because the audited Crystal paths only checked rental `DontUpgrade` on socket and upgrade branches.
- Remaining bounded combine gaps include hero-inventory handling, belt/id-collision cleanup, player `GemRatePercent`, and other gem-family branches.

R30 verification commands:

```powershell
cargo +1.89.0 fmt
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`: 13 / 13 passed
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`: 17 / 17 passed
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`: 85 / 85 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 472 / 472 passed

## Previous Completed Round: R29

R29 aligned the next bounded real client `CombineItem` branch:

- Added Crystal repair-combine parity for shape `1/2/5/6` sources in packet-driven inventory-grid `CombineItem`.
- Runtime now rejects `DontRepair` and wrong hammer-vs-sewing target families ack-only, matching Crystal's no-chat failure behavior for those branches.
- Full-durability targets now emit Crystal `ItemNoRepairNeeded` hint plus failure ack instead of silently mutating or consuming the source.
- Successful repair-combine now mutates durability, emits `ItemRepaired`, consumes the source stack, and ends with a success `CombineItem` ack.
- This round remains intentionally bounded: hero-inventory handling, belt/id-collision cleanup, rental `DontUpgrade`, player `GemRatePercent`, and other remaining gem-family branches stay open.

R29 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`: 11 / 11 passed
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`: 16 / 16 passed
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`: 83 / 83 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 469 / 469 passed

## Previous Completed Round: R28

R28 aligned the shared Crystal `CombineItem` target gate:

- Added the Crystal top-level target item-type gate to packet-driven `CombineItem`, matching `PlayerObject.CombineItem` by ack-failing any target outside item types `1..=11` before socket/seal/upgrade branch-specific handling.
- This closes a real parity gap where current packet `CombineItem` could previously emit `InvalidCombination` for shape-7 on non-equipment targets or even seal non-equipment inventory items.
- Added focused regressions that prove stage-5-style socket targets such as `BengalTiger` are rejected under the Crystal item-type window and that shape-8 seal attempts against inventory consumables fail ack-only without mutation.
- The round remains intentionally bounded: hero-inventory handling, belt/id-collision cleanup, rental `DontUpgrade`, player `GemRatePercent`, and other gem-family branches remain open.

R28 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`: 8 / 8 passed
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`: 16 / 16 passed
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`: 80 / 80 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 466 / 466 passed

## Previous Completed Round: R27

R27 aligned the next bounded real client `CombineItem` branch:

- Added Crystal `ServerPacket::ItemUpgraded` / id `216` to protocol ids, codec, gateway JSON conversion, and trace output.
- Runtime `ClientPacket::CombineItem` now covers the current inventory-grid shape-3/4 gem/orb upgrade semantics instead of stopping at socket/seal-only handling.
- Persisted `gem_count` through runtime item state, inventory/equipment round-trips, and `UserItem` encoding so upgrade state survives the same flows as Crystal.
- Added focused regressions for upgrade success, max-added-stat rejection, invalid combinations, and failure-destroy behavior.
- This round is intentionally bounded: full Crystal target-type gating across combine branches, hero-inventory handling, belt/id-collision cleanup, rental `DontUpgrade`, and player `GemRatePercent` remain open.

R27 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-protocol item_slot_seal_and_upgrade_server_packets_use_crystal_ids -- --nocapture
cargo +1.89.0 test -p mir2-gateway item_slot_and_seal_server_events_expose_crystal_payload_fields -- --nocapture
cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`: 7 / 7 passed
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`: 16 / 16 passed
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`: 79 / 79 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 465 / 465 passed

## Previous Completed Round: R26

R26 aligned the current real client `CombineItem` packet path:

- Added Crystal `ClientPacket::CombineItem` / id `111` and `ServerPacket::CombineItem` / id `215` to protocol ids, codec, and trace output.
- Gateway JSON now exposes Crystal `CombineItem` payload fields (`grid`, `idFrom`, `idTo`, `success`, `destroy`).
- Runtime `ClientPacket::CombineItem` now dispatches to the current inventory-grid shape-7 socket-growth and shape-8 seal semantics instead of leaving those flows Stage-5-only.
- Successful packet-driven socket/seal changes now mutate the same persisted runtime state as the existing helpers, including `UserItem.SealedInfo`, inventory/equipment round-trips, and item-change packets.
- This round is intentionally bounded: full Crystal target-type gating, hero-inventory handling, and other gem/combine branches remain open.

R26 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-protocol item_and_combat_client_packets_use_crystal_payloads -- --nocapture
cargo +1.89.0 test -p mir2-protocol item_action_ack_server_packets_use_crystal_ids -- --nocapture
cargo +1.89.0 test -p mir2-gateway combine_item_server_event_exposes_crystal_payload_fields -- --nocapture
cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

## Active Round: R37 Selection

The active target is no longer R35/R36 cleanup. Use this round to close the next smallest current `DropItem` gap before taking on broader map-flag import, hero inventory, or frontend work.

Current selection constraints:

- Prefer the highest-value small unchecked task over a large multi-system refactor.
- Keep one writer on `apps/simulation/src/runtime.rs`.
- Do not move the backend parity estimate again until the selected R37 task is implemented, verified, and documented.

Explorer recommendations already captured in docs/run log:

- Frontend candidate: screenshot baseline pack plus stage screenshot comparison harness.
- Backend candidate selected from Crystal source: current `DropItem` should reject on `CurrentMap.Info.NoThrowItem`, emit Crystal `CanNotDrop` system chat, and still return the failed ack before any inventory mutation.
- Do not reopen the completed hero-inventory guard or `DeleteItem` rounds unless protocol/runtime regressions appear.

## Subagent Workflow After Restart

The user explicitly wants the previous multi-agent workflow to continue. Use this pattern:

1. Coordinator reads queue/log/roadmap locally and chooses the active task.
2. Spawn a Crystal Explorer for source behavior. Read-only, no file edits.
3. Spawn a Rust Explorer for local code/test map when the implementation surface is not already clear. Read-only, no file edits.
4. Only spawn a Worker when the implementation scope is bounded and its write set does not overlap another writer.
5. For `apps/simulation/src/runtime.rs`, keep the Coordinator as the only writer unless a worker has a very narrow non-overlapping patch.
6. Coordinator integrates, runs focused tests, then broader regressions if shared behavior changed.
7. Update all relevant docs before opening the next round:
   - `docs/AGENT-TASK-QUEUE.md`
   - `docs/AGENT-RUN-LOG.md`
   - `docs/CRYSTAL-1TO1-ROADMAP.md`
   - `docs/BACKEND-1TO1-PROGRESS.md`
   - `docs/CRYSTAL-SERVER-PARITY.md`

## Model And Effort Policy

Use the observed quota profile from the prior session unless the new session shows a different one:

- Prefer `gpt-5.3-codex-spark` because Spark-specific quota was abundant.
- Use `xhigh` for the Coordinator and high-risk `runtime.rs` implementation.
- Use `high` for backend/frontend workers.
- Use `medium` for read-only explorers and docs/QA work.
- Avoid multiple code-writing agents on the same file.

## R37 Suggested Subagent Prompts

Crystal Explorer prompt:

```text
In E:\mir2\mir2-web3, do a read-only Crystal/source audit of current `DropItem` map `NoThrowItem` rejection and `CanNotDrop` message behavior. Use docs/AGENT-TASK-QUEUE.md as the source of truth, summarize exact Crystal behavior, file paths, line numbers, and the smallest safe scope for the next bounded round. Do not edit files.
```

Rust Explorer prompt:

```text
In E:\mir2\mir2-web3, do a read-only audit of the current Rust code/test surface for `DropItem` map `NoThrowItem` rejection and message behavior. Recommend the smallest safe write set, likely regression risks, and focused/full verification commands for the next bounded round. Do not edit files.
```

Backend Worker prompt, only after Crystal semantics are known:

```text
Implement the selected bounded R37 parity patch in E:\mir2\mir2-web3. You are not alone in the codebase; do not revert others' edits. Own only the explicitly assigned files, keep one writer on apps/simulation/src/runtime.rs when it is in scope, add focused regressions, run cargo +1.89.0 fmt/test commands, and report changed files plus tests. Do not update docs unless explicitly assigned.
```

## Ready-To-Paste Resume Prompt

Use this when reopening Codex:

```text
Continue E:\mir2\mir2-web3 toward 100% Crystal/Mir2 1:1 Candidate. Read docs\AGENT-RESUME-HANDOFF.md, docs\AGENT-ORCHESTRATION.md, docs\AGENT-TASK-QUEUE.md, docs\AGENT-RUN-LOG.md, docs\CRYSTAL-1TO1-ROADMAP.md, and docs\BACKEND-1TO1-PROGRESS.md first. Continue from the active round using the previous subagent workflow. Do not repeat completed rounds and do not ask for routine confirmation. Use gpt-5.3-codex-spark with xhigh/high for implementation and medium/high explorers unless current quota says otherwise.
```

## Completion Accounting

- Small Crystal edge-semantics rounds may move backend parity by only `0.01%`; this is expected at the current maturity level.
- Larger cross-cutting systems can move more, but only after code, tests, and docs are all complete.
- The backend score is not the whole-project score. Backend has 45% weight in `docs/CRYSTAL-1TO1-ROADMAP.md`.
- Do not mark a checkbox complete from inspection alone; use source evidence plus tests or trace evidence.
