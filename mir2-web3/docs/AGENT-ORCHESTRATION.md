# Agent Orchestration

> Latest runtime/frontend comparison sync: 2026-04-29-R310 completed/monitoring. R310 fixes two visible comparison blockers: the login transition overlay is cleared before game screenshots (`transitionOverlayVisible=false`), and NPC quest icons now require matching server `questIds` instead of rendering on every NPC. Added `apps/web/scripts/capture-crystal-parity.mjs` for repeatable Web same-scene capture and `apps/web/scripts/r310-visual-watch.ps1` for original/Web long-run sampling. Evidence: `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene-state.json`, `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene.png`, and the one-sample watch log under `docs/generated/player-qa/r310-visual-watch/r310-visual-watch-log.jsonl`.

> Latest runtime/frontend comparison sync: 2026-04-29-R309 completed. The aligned Bichon minimap now stays within the exact 1024x768 stage: `.mini-map-panel` uses `right=0`, and evidence records desktop `right=1024`, compact overflow-free bounds, zero non-favicon 404s, and zero console errors. Evidence: `docs/generated/player-qa/r309-minimap-bounds-web-page-state.json`, `docs/generated/player-qa/r309-minimap-bounds-web-page.png`, and `docs/generated/player-qa/r309-minimap-bounds-compact-web-page.png`.

> Latest runtime/frontend comparison sync: 2026-04-29-R308 completed. The aligned Bichon web comparison now renders the original-size browser stage at exact 1024x768 instead of the previous web-only 0.9 downscale, keeps compact 820x640 bounded with 0.78 scaling, and uses a plain black outer frame with no decorative shadow. R308 also exported missing visible-object sprite libs (`NPC/00`, `NPC/01`, `NPC/03`, `NPC/11`, `NPC/15`, `Monster/003`, `Monster/004`, `Monster/005`) from Crystal client data, eliminating non-favicon sprite 404s in the comparison view. Evidence: `docs/generated/player-qa/r308-stage-scale-web-page-state.json`, `docs/generated/player-qa/r308-stage-scale-web-page.png`, and `docs/generated/player-qa/r308-stage-scale-compact-web-page.png`.

> Latest map-resource audit sync: 2026-04-29-R303 completed. Added `audit:crystal-map-coverage` and archived all-map source-resource evidence under `docs/generated/map/r303-crystal-map-coverage.json`. The frontend map source baseline is now explicit: 463/463 Crystal manifest maps have local source map files, 0 unsupported map types, 0 parse errors, and all sampled viewports reference source frames. Remaining map/frontend acceptance risks are also explicit: 11 sampled maps have out-of-range frame-reference risk, minimap 450/451 remain missing for `DogYoArena2` / `DogYoHyun`, and full-map visual 1:1 still requires screenshot/human acceptance.

> Latest frontend/player QA sync: 2026-04-28-R301 completed. Windows refreshed the final automated Candidate acceptance pack after the R300 stable-diff packet acceptance decision. Evidence summary: `docs/generated/player-qa/r301-summary.json`; map API smoke 18/18 with 0 failures; minimap smoke 0 failures with known 450/451 warning; WS load 64/64 ready with 0 errors and keepalive p95 637 ms; Stage 5 UI smoke 88 screenshots with 0 critical console errors and 32 compact text nodes checked without overflow. Verification passed without Docker: packet-trace bin 15/15, web `tsc --noEmit`, web build, `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674. Whole-project accepted Crystal 1:1 remains roughly 90% until human visual/feel acceptance closes.

> Latest backend parity sync: 2026-04-28-R300 completed. The tracked backend/server packet matrix is now **100% Accepted under explicit stable-diff packet acceptance**. R298 provides 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9`; R299 identifies strict exact dirtiness as Crystal dynamic state; and R300 wires `packet_trace` acceptance mode so `MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF=1` plus `MIR2_PACKET_TRACE_REQUIRE_CRYSTAL=1` enforces accepted packet parity. Strict exact remains diagnostic until deterministic Crystal volatile-state fixtures exist. Verification: packet-trace bin 15/15 and `cargo +1.89.0 fmt --check`; acceptance decision: `docs/PACKET-PARITY-ACCEPTANCE.md` and `docs/generated/packet-traces/r300-stable-acceptance.json`.

> Latest frontend/player QA sync: 2026-04-28-R297 completed. Windows refreshed automated player evidence with full client resources at `E:\mir2\Crystal\Build\Client\Debug`: web build/typecheck passed, map API smoke served 18/18 requests, minimap smoke had 0 failures with known 450/451 warning, WS load passed 64/64 ready with 0 errors, and Stage 5 UI smoke captured 88 screenshots with `criticalConsoleErrorCount=0`. Verification also passed `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `cargo +1.89.0 fmt --check`, `git diff --check`, and web `tsc --noEmit`. This is automated Candidate evidence only; R300 closes the backend/server packet gate through explicit stable-diff acceptance, while whole-project accepted Crystal 1:1 remains **roughly 90%** until human visual/feel acceptance closes.

> Latest backend parity sync: 2026-04-28-R248 completed. Windows closed the previously blocked `Server.MirDB` / `Envir\Routes` data-import gate: `generate-crystal-respawn-manifest.mjs` regenerated Crystal respawn/monster/item/NPC-info manifests from `E:\mir2\Crystal\Build\Server\Debug\Server.MirDB` and `E:\mir2\Crystal\Build\Server\Debug\Envir\Routes`, including map `no_throw_item`, `no_drop_player`, and `no_drop_monster` flags. Verification passed: `mir2-game-data` 22/22, focused `mir2-simulation no_drop_monster_map_rule` 2/2, full `mir2-simulation` 670/670, and `mir2-gateway` 55/55 plus packet-trace bin tests 7/7. R300 later closed the remaining backend packet gate through explicit stable-diff acceptance.

> Latest product-evolution sync: 2026-04-28-R246 completed. Admin-delivered Stage 5 mail/gold now refreshes into already-online Gateway sessions before snapshots and saves, preventing keepalive from overwriting live admin mail and making the Player Web Mail panel update without logout/relogin.

> Latest product-evolution sync: 2026-04-28-R245 completed. The local admin backend is now browser-testable on `http://127.0.0.1:3020` with Player Web on `http://127.0.0.1:3010`, Gateway `:7110`, Admin API `:7420`, Postgres source mode, Redis routing cache, ClickHouse event reads, bearer auth, and GM forms for mail/grants/kick/ban. Admin API now supports policy-file operator auth via `ADMIN_OPERATOR_POLICY_PATH` and blocks requester self-approval by default.

> Latest product-evolution sync: 2026-04-27-R244 completed. Phase 1-7 production-control-plane route landed: persistent approvals, approval/outbox lifecycle events, JetStream outbox mode, grant/kick/ban GM routes, account ban enforcement, Postgres `save_version` stale-writer coverage, Redis character routing cache, Admin timeline read model, and Admin Web operator-token forwarding.

> Latest product-evolution sync: 2026-04-27-R238 completed. Admin command analytics now emit terminal outcome events for succeeded, failed, and denied commands. ClickHouse consumes `admin.command.succeeded`, `admin.command.failed`, and `admin.command.denied` through the v2 admin event consumer group, and Admin Web Audit can filter denied event status.

> Latest product-evolution sync: 2026-04-27-R237 completed. Admin outbox delivery state is split across NATS and Redpanda with `last_error` and `dispatched_at_ms`; partial publisher failure now retries/dead-letters instead of being marked dispatched. Admin API `/admin/events` supports filters and degraded ClickHouse responses, and Admin Web Audit exposes event filters plus independent event-stream health.

> Latest product-evolution sync: 2026-04-27-R236 completed. Admin outbox now publishes stable event envelopes to Redpanda through Pandaproxy when `ADMIN_OUTBOX_REDPANDA_URL` is set, while keeping NATS dispatch. ClickHouse projects the envelopes into `admin_events` and `admin_command_events`; Admin API exposes `/admin/events`; Admin Web Audit displays the event stream. End-to-end smoke passed from real Admin API command through Postgres outbox, dispatcher, Redpanda, ClickHouse, and Admin API event readback.

> Latest product-evolution sync: 2026-04-27-R235 completed. Redpanda and ClickHouse are now in the default local dev Compose stack for non-authoritative event analytics. Redpanda has separate internal/external Kafka listeners; ClickHouse initializes a Kafka-engine projection from `admin.command.succeeded` into `mir2_events.admin_command_events`. Existing NATS admin outbox dispatch remains the lightweight command/notification path.

> Latest product-evolution sync: 2026-04-27-R234 completed. Admin API production boundary advanced with optional bearer operator token validation, high-risk command approval IDs, item/gold grant command execution through audited system-mail delivery, and outbox retry/dead-letter state. Admin tests pass 11/11.

> Latest product-evolution sync: 2026-04-27-R233 completed. Postgres source-of-truth account store now tracks loaded source versions, rejects stale writers, and has Docker Postgres tests for stale writer rejection plus reload-save success.

> Latest product-evolution sync: 2026-04-27-R232 expanded. Gateway session cache now includes optional Redis support with TTL, while default startup remains in-memory unless `MIR2_GATEWAY_REDIS_CACHE_URL` is set.

> Latest product-evolution sync: 2026-04-27-R232 completed. Gateway online-session caching now has a non-authoritative boundary: simulation exposes active account/character identity, gateway has a `GatewaySessionCache` contract plus in-memory implementation, and the web gateway refreshes after authoritative saves and removes the record on disconnect. Focused gateway cache tests passed 4/4 with `cargo +1.89.0 fmt --check`; a real Redis adapter remains the next cache slice.

> Latest product-evolution sync: 2026-04-27-R228 completed. Audited GM system mail now mutates game-visible state. `apps/admin-api` posts `SendSystemMail` to the live gateway at `ADMIN_GATEWAY_MAIL_URL` and falls back to persistent account-store delivery; `apps/gateway` exposes `POST /admin/system-mail`; `apps/simulation` persists the mail into Stage 5 character systems; and the player Mail panel can claim/delete it. Verification included Rust focused/package tests, web/admin-web typecheck/build, Admin Web -> Admin API -> gateway curl smoke, outbox `deliveryMode: "gateway_live"`, account-store inspection, gateway WS mail visibility, and WS `mail.claim` attachment transfer.

> Latest product-evolution sync: 2026-04-27-R227 completed. Admin operations now has a working Rust API + Next Admin Web slice: `apps/admin-api` command/audit repository traits, in-memory repositories, Axum routes, `SendSystemMail` domain outbox executor, and `apps/admin-web` desktop UI pages wired through Next `/api/admin/system-mail` to Rust `/admin/commands/send-system-mail`. Verification passed: admin-api locked tests/fmt, admin-web typecheck/build, direct Rust API curl write, Next proxy curl write, and Playwright admin UI screenshots.

> Previous sync: R225 completed. Mac-local Candidate regression was green: web `tsc --noEmit`, direct `next build`, Stage 5 UI smoke (88 screenshots, 8 compact panel bounds, 34 compact text nodes, 0 critical console errors), map API smoke 18/18, minimap asset smoke 0 failures with known 450/451 warning, WS load 64/64, `mir2-game-data` 22/22, `mir2-gateway` 54/54, `mir2-simulation` 664/664, require-local `packet_trace --matrix` wrote 9/9 local TCP artifacts under `docs/generated/packet-traces/r225-matrix`, `cargo +1.89.0 fmt --check`, and `git diff --check`. At R225 time the backend tracked slice remained 99.70%; R300 later closed the packet gate under explicit stable-diff acceptance.

> Latest sync: R224 completed. Automated evidence remains **100% Candidate** and the local packet trace blocker is closed. `packet_trace --list-flows` works, `mir2-gateway` passes 53/53 including packet trace bin tests 6/6, and require-local `packet_trace --matrix` wrote 9/9 TCP-traceable artifacts with `localOk=true` under `docs/generated/packet-traces/r224-matrix`. R225 is open for human acceptance / external blockers. Remaining non-routine gates: final human visual/feel acceptance, missing local `Crystal/Build/Server/Debug/Server.MirDB`, and missing live `MIR2_CRYSTAL_TCP_ADDR`.

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


Last updated: 2026-04-29

Purpose: define the autonomous multi-agent workflow for driving `mir2-web3` to a full Crystal / Mir2 1:1 Candidate build without requiring routine human confirmation.

## Target State

The automation target is **100% Candidate**:

- Code, data, docs, tests, traces, and screenshots are complete against the current acceptance standard.
- Known gaps are either fixed, explicitly documented as blocked, or moved to a user acceptance decision.
- The user only needs final gameplay validation before the project is marked **100% Accepted**.

`100% Candidate` is not the same as `100% Accepted`. The final accepted state requires either human frontend gameplay acceptance or an explicit decision to accept remaining human-only visual/feel differences.

## Progress Tracks

| Track | Owner | Primary Evidence |
| --- | --- | --- |
| Backend/server parity | Backend agents | Rust tests, protocol tests, Crystal source references, parity docs |
| Frontend/client parity | Frontend agents | Playwright/CDP screenshots, UI smoke, manual QA script |
| Crystal assets and data | Data agents | generated manifests, asset smoke tests, map/API checks |
| Integration/live parity | QA agents | packet traces, local-vs-Crystal diffs, gateway smokes |
| Playability/operations | QA agents | soak/load tests, reconnect tests, player QA route |

## Roles

| Role | Typical Model | Effort | Responsibilities |
| --- | --- | --- | --- |
| Coordinator | current main Codex session | xhigh | select tasks, prevent conflicts, integrate patches, run final tests, update docs |
| Crystal Explorer | `gpt-5.3-codex-spark` or mini | medium/high | inspect `E:\mir2\Crystal`, extract exact source behavior and edge cases |
| Rust Explorer | `gpt-5.3-codex-spark` or mini | medium/high | inspect current Rust code/tests, locate minimal change points and risks |
| Backend Worker | `gpt-5.3-codex-spark` | high/xhigh | implement assigned backend behavior in a bounded write set |
| Frontend Worker | `gpt-5.3-codex-spark` | high | implement assigned frontend/UI parity in bounded files |
| Data Worker | `gpt-5.3-codex-spark` or mini | medium/high | update generators/manifests/assets in bounded files |
| QA/Docs Worker | mini or `gpt-5.3-codex-spark` | medium | prepare test matrix, screenshots, trace evidence, and docs updates |

## Current Quota Policy

Current observed account state on 2026-04-22:

- active model: `gpt-5.3-codex-spark`
- general 5h limit: 80% remaining
- general weekly limit: 58% remaining
- `GPT-5.3-Codex-Spark` 5h limit: 97% remaining
- `GPT-5.3-Codex-Spark` weekly limit: 78% remaining

Scheduling policy while this quota profile holds:

- Use `gpt-5.3-codex-spark` for backend/frontend workers and high-value explorers because Spark-specific quota is abundant.
- Use `xhigh` only for bounded implementation in high-risk files such as `apps/simulation/src/runtime.rs`, protocol serialization, or complex UI state.
- Use `high` for normal code workers.
- Use `medium` for read-only exploration and docs/QA planning.
- Avoid unsupported account models such as `gpt-5.2-codex` in this environment unless a later settings check proves availability.
- Keep concurrent workers to one code writer per high-conflict file; spend extra quota on explorers and QA instead of conflicting writers.

## Coordination Rules

- The Coordinator owns final integration and decides whether a task is complete.
- Explorers are read-only unless explicitly reassigned.
- A worker must receive a bounded write set before editing files.
- Do not assign two workers to edit the same file or tightly coupled module at the same time.
- `apps/simulation/src/runtime.rs` is high-conflict. Only one worker may edit it per round.
- Docs can be edited in parallel only when the code worker is not also editing docs.
- Every completed behavior change must update:
  - `docs/CRYSTAL-1TO1-ROADMAP.md`
  - `docs/BACKEND-1TO1-PROGRESS.md` when backend parity changes
  - `docs/CRYSTAL-SERVER-PARITY.md` when server parity changes
- A checkbox is marked only after a command, screenshot, packet trace, or source comparison supports it.

## Round Template

Each autonomous round should target one verified completion item.

1. Select the highest-value small unchecked task.
2. Start read-only explorers for Crystal behavior and local implementation context.
3. Start one bounded worker if the implementation scope is clear.
4. Coordinator performs non-overlapping docs/task-queue work while agents run.
5. Review worker changes and explorer findings.
6. Run focused tests first, then broader regression if the change touches shared behavior.
7. Update docs/checklists/run log.
8. Start the next round without asking for confirmation unless a stop condition is hit.

## Stop Conditions

Do not stop for normal implementation decisions, local test failures, local refactors needed to finish an assigned item, generated data refreshes, or documentation updates.

Stop and ask only when:

- destructive filesystem operations are required;
- credentials, private endpoints, or a live Crystal server address are required;
- required Crystal source/assets are unavailable;
- two acceptance standards conflict and cannot be inferred from Crystal behavior;
- human-only frontend acceptance is needed to move from Candidate to Accepted.

## Standard Verification Tiers

| Tier | When Used | Examples |
| --- | --- | --- |
| Focused | Every small task | `cargo test -p mir2-simulation drop_item_packet` |
| Adjacent | Shared behavior changed | `pickup`, `harvest`, `storage`, `packet_trace` tests |
| Workspace | Stage gates | `cargo test --workspace`, `npm.cmd run build` |
| UI/API | Frontend/data changes | Playwright/CDP smoke, map API smoke, screenshots |
| Live parity | Acceptance gates | local-vs-Crystal packet trace diff |

## Current Round Status

The authoritative current round is in `docs/AGENT-TASK-QUEUE.md`. If this file and the queue disagree, trust the queue and update this section.

Current checkpoint:

- Active round: `2026-04-29-R309`.
- Active task: keep accepted stable-diff packet parity green while closing remaining same-scene frontend visual/resource gaps before human visual/feel acceptance.
- Active round state: R225 refreshed Mac-local Candidate evidence, Stage 5 manifest summary, map/minimap/load outputs, local packet trace matrix artifacts, Windows continuation docs, and stale gateway README status. R248 closed the R39 manifest-backed map-flag import on Windows with local `Server.MirDB` and matching route files. R298 refreshed live Crystal stable packet-matrix evidence with 9/9 stable diffs clean, R299 proved strict exact dirtiness is dynamic Crystal state, R300 accepted/enforced stable-diff packet parity for the tracked backend/server matrix, R301 refreshed the final automated Candidate acceptance pack, R304/R305 restored aligned Bichon NPC/visible-respawn population, R306 removed the web-only quest overlay/nameplate underscore gap, R307 locked ordinary Guard/ArcherGuard evidence, R308 removed browser-only original-size stage scaling plus missing visible-object sprite 404s, and R309 closed the measured minimap 2px overflow.
- Last completed round: `2026-04-29-R309`; last completed backend package evidence round is `2026-04-29-R307`.
- Backend/server tracked-slice parity status: `100% Accepted under explicit stable-diff packet acceptance`.
- Whole-project automation status: `100.0% Candidate`.
- Whole-project real accepted 1:1 estimate: `roughly 90.0%`.
- Restart handoff file: `docs/AGENT-RESUME-HANDOFF.md`.
- Windows continuation checklist: `docs/WINDOWS-CONTINUATION.md`.

Latest completed rounds:

| Round | Result |
| --- | --- |
| R309 | Aligned Bichon minimap/HUD bounds no longer overflow the 1024x768 stage; desktop and compact evidence record empty overflow arrays, zero non-favicon 404s, and zero console errors. |
| R308 | Aligned Bichon comparison no longer applies original-size web stage downscaling/frame decoration, compact scaling remains bounded, and missing visible-object NPC/monster sprite libraries are exported with zero non-favicon 404s in the R308 browser evidence. |
| R307 | Second aligned Bichon comparison point has focused Guard/ArcherGuard simulation coverage and browser evidence at `0:287,618`. |
| R301 | Final automated Candidate acceptance pack refreshed: web typecheck/build, map/minimap smokes, WS load, Stage 5 UI smoke, packet-trace bin, and Rust package regressions are green; evidence summary is `docs/generated/player-qa/r301-summary.json`. |
| R300 | Stable-diff packet acceptance landed: `packet_trace` now distinguishes strict exact diagnostics from accepted packet parity, and the R298/R299 evidence closes the tracked backend/server packet gate as 100% Accepted under explicit stable-diff acceptance. |
| R298 | Live Crystal stable packet matrix passed 9/9 with no missing Crystal endpoint under `docs/generated/packet-traces/r298-live-matrix`; strict exact diff remained dirty and is diagnostic after R300. |
| R224 | `mir2-gateway` packet trace harness restored with `--list-flows`, single-flow capture, matrix artifacts, endpoint diff summaries, and require-mode checks; require-local matrix wrote 9/9 TCP-traceable artifacts under `docs/generated/packet-traces/r224-matrix`. |
| R204 | Stage 5 UI smoke now clicks Red Potion directly in the belt, verifies quantity decreases before hotkey use, captures a screenshot, and records `beltMouseUseFlow`. |
| R203 | Character RemoveItem now targets inventory with a free bag slot, and Stage 5 UI smoke verifies Dagger leaves weapon equipment and returns to bag1 slot 4 with `characterRemoveFlow`. |
| R202 | Stage 5 UI smoke now drops Blue Potion through Delete Item, verifies quantity decreases and a ground label appears, captures two screenshots, and records `inventoryDropFlow`. |
| R201 | Stage 5 UI smoke now splits Red Potion through the inventory UI, verifies the split stack lands in the belt and total quantity is preserved, captures two screenshots, and records `inventorySplitFlow`. |
| R200 | Stage 5 UI smoke now moves Wooden Sword from bag1 slot 4 to slot 10, captures the moved-item screenshot, and records `inventoryMoveFlow`. |
| R199 | Stage 5 UI smoke now drops 100 gold through inventory UI, verifies gold decreases and a ground label appears, fixes missing confirm fallback text, captures two screenshots, and records `inventoryGoldFlow`. |
| R198 | Stage 5 UI smoke now opens Character Spells from HUD Skill and Stats II from HUD Option, captures two HUD-button screenshots, and records `hudButtonFlow`. |
| R197 | Stage 5 UI smoke now clicks Dagger from inventory bag1, verifies Dagger moves into the weapon equipment slot, captures the inventory-equip screenshot, and records `inventoryEquipFlow`. |
| R196 | Stage 5 UI smoke now clicks Red Potion from inventory bag1, verifies quantity drops from 5 to 4, captures the inventory-use screenshot, and records `inventoryUseFlow`. |
| R195 | Stage 5 UI smoke now rents expanded storage from locked page 2, verifies active expanded storage/unlocked page 2/160-slot capacity/expiry copy, captures the rented page screenshot, and records the rented state in `storageFlow`. |
| R194 | Stage 5 UI smoke now opens the system menu, verifies transfer/action labels, routes Character, Inventory, and Quest actions; captures four system-menu screenshots; and records `systemMenuFlow`. |
| R193 | Stage 5 UI smoke now exercises chat Shout filter, All restore, Settings, collapse/restore size, and Report paths; captures four chat-control screenshots; and records `chatFlow`. |
| R192 | Stage 5 UI smoke now switches storage page 1, locked page 2, and restored page 1; captures two storage screenshots; and records `storageFlow`. |
| R191 | Stage 5 UI smoke now switches character char, stats1, stats2, spells, and restored char tabs; captures four character screenshots; and records `characterFlow`. |
| R190 | Stage 5 UI smoke now switches inventory bag1, bag2, quest, and restored bag1 tabs; captures three inventory screenshots; and records `inventoryFlow`. |
| R189 | Stage 5 UI smoke now presses belt hotkey `1`, verifies Red Potion quantity drops from 5 to 4, captures `stage5-belt-hotkey-use.png`, and records `beltUseFlow`. |
| R188 | Stage 5 UI smoke now exercises belt horizontal, vertical, rotate-back, and close states; fixes belt label offsets and Quest overlap; captures three belt screenshots; and records `beltFlow`. |
| R187 | Stage 5 UI smoke now exercises minimap collapse, BigMap re-expand, and Mail open paths; captures three minimap screenshots; and records `minimapFlow` state. |
| R186 | Stage 5 UI smoke now checks compact visible core text for overflow, records `compactTextLayout`, and the minimap title/Safe Zone label is fixed as a stable two-line header. |
| R185 | Stage 5 UI smoke now captures desktop 1024x768 and compact 820x640 evidence, writes compact layout bounds to the manifest, adds `stage5-compact-game.png`, and passed with 11 screenshots. |
| R184 | Frontend/global parity advanced: chat follows latest filtered lines with a live scroll knob, no-WebGL headless UI uses DOM fallback, map API has packaged fallback without recursive failure, macOS Chrome smoke detection works, Stage 5 UI smoke captured 10 screenshots, and WS load passed 64/64. |
| R183 | Runtime interaction quest hints now use `custom.interaction.questHint`; generated localization bundles/importer are synchronized, runtime has no `sim.*` references, and full `mir2-simulation` is green at 664/664. |
| R182 | No-script/no-page NPC interaction no longer opens runtime-only idle dialog text; full `mir2-simulation` is green at 664/664. |
| R181 | Quest-required drop feedback now uses Crystal `server.YouFound` and removes runtime-only quest progress chats; full `mir2-simulation` is green at 664/664. |
| R180 | Start-game welcome chat now uses Crystal `server.Welcome` with localized `server.GameName` and `Hint` chat type; full `mir2-simulation` is green at 664/664 and `mir2-gateway` is green at 47/47. |
| R179 | Normal chat now emits only Crystal-shaped `ObjectChat` and pre-start chat is silent; full `mir2-simulation` is green at 664/664 and `mir2-gateway` is green at 47/47. |
| R178 | High-level cast-skill failure paths no longer emit runtime-only helper chats; full `mir2-simulation` is green at 663/663. |
| R177 | `MoveItem` unsupported-grid/missing-source fallback no longer emits runtime-only helper chat; full `mir2-simulation` is green at 660/660. |
| R176 | Stale active NPC dialog missing-NPC/no-script handling no longer emits runtime-only helper chat; full `mir2-simulation` is green at 660/660. |
| R175 | NPC dialog helper no-active/invalid-target/no-pending-input handling no longer emits runtime-only helper chat; full `mir2-simulation` is green at 658/658. |
| R174 | Direct NPC interaction invalid target/direction/range handling no longer emits runtime-only helper chat; full `mir2-simulation` is green at 655/655. |
| R173 | Direct attack invalid target/state/range handling no longer emits runtime-only helper chat; full `mir2-simulation` is green at 652/652. |
| R172 | Successful high-level NPC interaction no longer emits runtime-only `sim.talkingToNpc`; full `mir2-simulation` is green at 648/648. |
| R171 | Direct high-level pickup invalid target/distance handling no longer emits runtime-only helper chat; full `mir2-simulation` is green at 648/648. |
| R170 | Missing defeated-monster entity handling no longer emits runtime-only internal chat; full `mir2-simulation` is green at 645/645. |
| R169 | Monster death drop success paths no longer emit runtime-only gold/item drop success chats; full `mir2-simulation` is green at 644/644. |
| R168 | Summoned VampireSpider death explosion no longer emits runtime-only `sim.targetDefeated` defeat chat; full `mir2-simulation` is green at 643/643. |
| R167 | Ordinary combat hit resolution no longer emits runtime-only damage narration; full `mir2-simulation` is green at 643/643. |
| R166 | Successful cast-skill paths no longer emit generic `sim.castSkill` helper chat; full `mir2-simulation` is green at 643/643. |
| R165 | Cast-skill high-level entrypoint now silently rejects before `StartGame` instead of emitting runtime-only cast helper chat; full `mir2-simulation` is green at 643/643. |
| R164 | Interaction high-level and dialog target entrypoints now silently reject before `StartGame` instead of emitting runtime-only interaction helper chat; full `mir2-simulation` is green at 642/642. |
| R163 | Harvest high-level and packet entrypoints now silently reject before `StartGame` instead of emitting runtime-only harvest helper chat; full `mir2-simulation` is green at 641/641. |
| R162 | Attack high-level and packet entrypoints now silently reject before `StartGame` instead of emitting runtime-only attack helper chat; full `mir2-simulation` is green at 640/640. |
| R161 | Movement high-level and packet entrypoints now silently reject before `StartGame` instead of emitting runtime-only movement/turning helper chats; full `mir2-simulation` is green at 639/639. |
| R160 | Pickup high-level and packet entrypoints now silently reject before `StartGame` instead of emitting runtime-only pickup helper chat; full `mir2-simulation` is green at 638/638. |
| R159 | Trainer immediate damage reporting now uses Crystal `server.PetInflictedDamageDps` with localized `server.You`; full `mir2-simulation` is green at 638/638. |
| R158 | Trainer average damage reporting now uses Crystal `server.AverageDamageOnTrainer` and localization formatting supports `{index:format}` placeholders; full `mir2-simulation` is green at 638/638. |
| R157 | Benediction-oil no-effect/luck/curse outcomes now use Crystal weapon luck localization keys; full `mir2-simulation` is green at 638/638. |
| R156 | `@ADDSTORAGE` no longer emits hardcoded expanded-storage helper success chat; full `mir2-simulation` is green at 638/638. |
| R155 | `ShowGroupPickup` item notices now use Crystal `server.FriendlyPickedUpItem`; full `mir2-simulation` is green at 638/638. |
| R154 | High-level `use_item(key)` and `drop_item(key)` before `StartGame` no longer emit runtime-only helper chats; full `mir2-simulation` is green at 638/638. |
| R153 | High-level `drop_item(key)` missing-item helper now emits no packets/chat and preserves state; full `mir2-simulation` is green at 638/638. |
| R152 | Map-transfer not-in-world rejection now uses Crystal `server.NotFound`; full `mir2-simulation` is green at 638/638. |
| R151 | Missing-template `RequestItemInfo` failure now uses Crystal `server.NotFound`; full `mir2-simulation` is green at 638/638. |
| R150 | Map-transfer bounds rejection now uses Crystal `server.CannotPositionMoveOnMap`; full `mir2-simulation` is green at 638/638. |
| R149 | Stage 5 `event.spawn` and `hero.behaviour` successes no longer emit runtime-only helper narration; full `mir2-simulation` is green at 638/638. |
| R148 | Debug Crystal transfer keys no longer emit runtime-only `"Transferred to Crystal map ..."` success chat; full `mir2-simulation` is green at 638/638. |
| R147 | Generic runtime-only Stage 5 helper success chats were removed across group/social/mail/trade/auction/conquest/hero/profession helpers; full `mir2-simulation` is green at 638/638. |
| R131 | Stage 5 socket/seal missing-source rejection chats now use Crystal `server.NotFound`; full `mir2-simulation` is green at 633/633. |
| R132 | Stage 5 socket/seal missing-equipped-item rejection chats now use Crystal `server.NotFound`; full `mir2-simulation` is green at 635/635. |
| R133 | Stage 5 socket metadata-missing rejection chat now uses Crystal `server.NotFound`; full `mir2-simulation` is green at 636/636. |
| R146 | Stage 5 event-spawn missing-player/position rejections now use Crystal `server.NotFound`; full `mir2-simulation` is green at 638/638. |
| R145 | Unknown map-transfer rejection now uses Crystal `server.NotFound`; full `mir2-simulation` is green at 638/638. |
| R144 | Stage 5 unknown-command rejection now uses Crystal `server.InvalidPacketReceived`; full `mir2-simulation` is green at 638/638. |
| R143 | Stage 5 inactive-trade rejections now use Crystal `server.NotFound`; full `mir2-simulation` is green at 638/638. |
| R142 | Stage 5 `auction.buy` / `auction.cancel` missing-id rejections now use Crystal `server.InvalidPacketReceived`; full `mir2-simulation` is green at 638/638. |
| R141 | Stage 5 `mail.claim` / `mail.delete` missing-id rejections now use Crystal `server.InvalidPacketReceived`; full `mir2-simulation` is green at 638/638. |
| R140 | Stage 5 `trade.offerGold` missing-amount rejection now uses Crystal `server.InvalidPacketReceived`; full `mir2-simulation` is green at 638/638. |
| R139 | Stage 5 hero-behaviour missing-hero rejection now uses Crystal `server.NotFound`; full `mir2-simulation` is green at 638/638. |
| R138 | Stage 5 event-spawn missing-template rejection now uses Crystal `server.NotFound`; full `mir2-simulation` is green at 638/638. |
| R137 | Stage 5 guild creation success chat now uses Crystal `server.SuccessfullyCreatedGuild`; full `mir2-simulation` is green at 638/638. |
| R136 | Stage 5 craft no-ore rejection chat now uses Crystal `server.CraftingAttemptFailed`; full `mir2-simulation` is green at 638/638. |
| R135 | Stage 5 credit-shop insufficient-credit rejection chat now uses Crystal `server.YouDontHaveEnoughCurrency`; full `mir2-simulation` is green at 638/638. |
| R134 | Stage 5 mail/trade/auction missing-entity rejection chats now use Crystal `server.NotFound`; full `mir2-simulation` is green at 638/638. |
| R130 | Ordinary map transfers no longer emit runtime-only `"Transferred to ..."` success chat; full `mir2-simulation` is green at 633/633. |
| R129 | Stage 5 socket/seal invalid-source rejection chats now use Crystal `server.InvalidCombination`; full `mir2-simulation` is green at 633/633. |
| R128 | Stage 5 gold-shop purchase chat now uses Crystal `server.BoughtItemForGold`; full `mir2-simulation` is green at 633/633. |
| R127 | Successful harvest-drop transfer no longer emits runtime-only `"Harvested ..."` chat; full `mir2-simulation` is green at 633/633. |
| R126 | Expanded-storage expiry notice now uses Crystal `server.ExpandedStorageExpired` while preserving one-shot resize and persistence behavior; full `mir2-simulation` is green at 633/633. |
| R125 | Stage 5 item socket/seal success chats now use Crystal `server.ItemSocketsIncreased` and `server.ItemSealedFor`; full `mir2-simulation` is green at 633/633. |
| R124 | Stage 5 item-seal reseal-delay rejection now uses Crystal `server.ItemCannotBeResealedFor` with the modeled remaining-duration label; full `mir2-simulation` is green at 633/633. |
| R123 | Stage 5 credit-shop purchase chat now uses Crystal `server.BoughtItemForCredit` while mailbox delivery remains stateful; full `mir2-simulation` is green at 633/633. |
| R122 | Stage 5 successful trade completion now uses Crystal `server.TradeSuccessful`; full `mir2-simulation` is green at 633/633. |
| R121 | Stage 5 trade/shop/auction low-gold rejections now use Crystal `server.LowGold`; full `mir2-simulation` is green at 633/633. |
| R120 | Direct ground-drop pickup full-bag rejection now uses Crystal `server.YouCannotCarryAnymore` while current-cell pickup still skips blocked drops; full `mir2-simulation` is green at 633/633. |
| R119 | Stage 5 mail/shop/auction/craft full-bag rejections now use Crystal `server.YouCannotCarryAnymore`; full `mir2-simulation` is green at 633/633. |
| R118 | Stage 5 item socket max-capacity and already-sealed rejections now use Crystal server text keys; full `mir2-simulation` is green at 633/633. |
| R117 | Harvest no-drop and full-bag messages now use Crystal `server.NothingWasFound` / `server.YouCannotCarryAnymore`; full `mir2-simulation` is green at 633/633. |
| R116 | Owner-blocked pickup rejection now uses Crystal `server.CannotPickupNotOwner` localization while preserving owner-window scan behavior; full `mir2-simulation` is green at 633/633. |
| R115 | Normal item/gold pickup success no longer emits runtime-only success chat while preserving `ShowGroupPickup` group notices; full `mir2-simulation` is green at 633/633. |
| R114 | Static starter and dynamic manifest-backed potion `UseItem` now honor Crystal `NoDrug` map-rule rejection; full `mir2-simulation` is green at 633/633. |
| R113 | Static starter HP/MP potion use now queues Crystal-style timed recovery instead of immediate HP/MP mutation; full `mir2-simulation` is green at 631/631. |
| R112 | Static `repair-powder` success/failure no longer emits runtime-only `sim.noEquipmentNeedsRepair` / `sim.repairedEquippedItems`; full `mir2-simulation` is green at 631/631. |
| R111 | Static `town-teleport` success no longer emits runtime-only `sim.townTeleportReturnedToSpawn`; full `mir2-simulation` is green at 631/631. |
| R110 | Static `benediction-oil` no-weapon failure no longer emits hardcoded runtime-only chat; full `mir2-simulation` is green at 631/631. |
| R109 | Successful `SplitItem` no longer emits runtime-only `"Item stack split."`; full `mir2-simulation` is green at 630/630. |
| R108 | Static `repair-oil` / `war-god-oil` now use Crystal localized weapon-repair hints and no failure chat; full `mir2-simulation` is green at 630/630. |
| R107 | Successful `DropItem` no longer emits runtime-only `custom.itemDropped`; full `mir2-simulation` is green at 629/629. |
| R106 | Static HP/MP consumable `UseItem` success no longer emits runtime-only `sim.usedItem`; full `mir2-simulation` is green at 629/629. |
| R105 | Missing-source `DropItem` no longer emits `sim.itemNotFoundInBag`; full `mir2-simulation` is green at 629/629. |
| R104 | Unmodeled `UseItem(grid=HeroInventory)` now emits a Crystal-shaped failed ack instead of empty packets; full `mir2-simulation` is green at 628/628. |
| R103 | Missing-item and invalid-source `UseItem` failures no longer emit `sim.itemNotFoundInBag`; full `mir2-simulation` is green at 628/628. |
| R102 | Unusable inventory `UseItem` fallback no longer emits `sim.itemNoActiveUse`; full `mir2-simulation` is green at 627/627. |
| R101 | Non-inventory use-equip failure no longer emits literal runtime-only chat; full `mir2-simulation` is green at 626/626. |
| R100 | Successful use-equip no longer emits runtime-only `sim.equippedItem*` chat; full `mir2-simulation` is green at 625/625. |
| R99 | Dynamic manifest-backed explicit `EquipItem` positive path is covered when requirements are met; full `mir2-simulation` is green at 625/625. |
| R98 | Dynamic manifest-backed `CreditToken3` use is covered for success ack, `GainedCredit`, localized hint chat, credit state update, and item consumption; full `mir2-simulation` is green at 624/624. |
| R97 | Storage-sourced explicit `EquipItem` now has regression coverage for dynamic manifest-backed requirement rejection; full `mir2-simulation` is green at 623/623. |
| R96 | Explicit `EquipItem` now silently rejects dynamic manifest-backed equipment when Crystal gender/class/required-type checks fail, while preserving legacy fixture alias behavior; full `mir2-simulation` is green at 622/622. |
| R95 | Added explicit `ItemType.Amulet` to right-bracelet slot coverage; `equip_item_packet` is green at 10/10. |
| R94 | Wider validation passed: `item` 218/218, `storage` 42/42, `fmt --check`, `diff --check`, and full `mir2-simulation` 620/620. |
| R93 | Explicit `EquipItem` target compatibility now allows manifest-backed rings/bracelets into right-side slots by Crystal item type instead of rejecting due to default left-slot metadata. |
| R92 | Successful dead-player `ResurrectionScroll` use now restores modeled MP along with full HP before consuming the scroll. |
| R91 | `RepairOil` / `WarGodOil` now honor Crystal/rental repair bind flags: `DontRepair` blocks both, `NoSRepair` blocks full/special repair, and failures preserve item plus weapon durability. |
| R90 | `UseItem` scroll-shape `0/2` now honors configured Crystal `NoEscape` / `NoRandom` map rules, emitting `server.CanNotDungeon` / `server.CanNotRandom` and preserving item/position on blocked maps. |
| R89 | Manifest-backed Crystal equipment item types now map to runtime `EquipmentSlot` during item creation and `UseItem` fallback, so current manifest equipment use no longer depends on manual slot setup. |
| R88 | Normal-potion shape `0` now uses a modeled pending/timed recovery surface (`pending_pot_health_amount` / `pending_pot_mana_amount`), with world-tick-based packetized HP/MP restoration and no immediate HP/MP mutation. |
| R87 | Mount-fed `ItemType.Food` use now follows Crystal `UseItem` surface: requires equipped mount, preserves on mount-missing/full-dura failure, consumes on success, emits mount-fed/repair hints, and applies `RawMeat` max-dura pre-loss before feeding. |
| R86 | `UseItem` scroll-shape `0/2` for `DungeonEscape` / `TeleportHome` / `RandomTeleport` now aligns with Crystal: success consumes/map refresh with ack, failure preserves item/state with failure ack, and same-map destination validation is in place. |
| R85 | `CanUseItem` parity for manifest-backed current item requirements expanded beyond level-only to covered stat surfaces (`MaxAC`, `MaxMAC`, `MaxDC`, `MaxMC`, `MaxSC`, `MinAC`, `MinMAC`, `MinDC`, `MinMC`, `MinSC`, `MaxLevel`) using modeled equipment/buff totals, with focused regressions for low requirement rejection and modeled requirement pass-through. |
| R84 | `UseItem` scroll-shape 26/27 for `GtInvite` / `GTTeleport` now follows Crystal `PlayerObject.UseItem` parity: after `CanUseItem` pass, the item is consumed once, `UseItem` success ack is emitted, no chat is sent, and no `UserLocation` teleport is performed for these two shapes; focused/adjacent runtime packet regressions have passed. |
| R83 | Remaining manifest-backed current `UseItem` small surfaces now handle `AncientBanga[Green]` / `AncientBanga[Purple]` via `scroll shape 8/9`, emit `free_map_shout` / `free_server_shout`, emit Crystal hint chat, and localize credit-token hints to `server.CreditsAddedToAccount`, with full `mir2-simulation` regression green at 607 tests. |
| R82 | Crystal `CanUseItem` parity now matches `Gender`, `Class`, `RequiredType == Level`, repeated-skill-book learn rejection, and valid skill-book learning consume behavior, with full `mir2-simulation` regression green at 607 tests. |
| R81 | Dynamic manifest-backed current-data `UseItem` now routes Crystal `SunPotion`, duration buffs, `TownTeleport`, `BenedictionOil`, `RepairOil`, and `WarGodOil` through template stats and scroll shapes, including same-key buff duration stacking and the current `WarGodOil` shape-0 name fallback, with full `mir2-simulation` regression green at 599 tests. |
| R80 | Current equipment/item metadata now preserves Crystal `NeedIdentify` and `SoulBoundId` through runtime/item payload round-trips, auto-identifies items on equip/use-equip, and rejects equipping items soul-bound to another character, with later full-suite revalidation green at 599 tests. |
| R79 | Current `MysteryWater` plus cursed current-equipment semantics now match Crystal's bounded runtime surface: first use unlocks and consumes, repeat use hint-chats without consuming, cursed current `RemoveItem` and replacement `EquipItem` require the unlock, successful cursed removal/replacement clears it again, and storage-grid replacement rejects replaced equipment that cannot be stored, with full `mir2-simulation` regression green at 590 tests. |
| R78 | Current `RemoveSlotItem` now follows Crystal's bounded source-grid envelope for the modeled runtime: invalid `grid=Equipment` requests and unmodeled `Mount` / `Fishing` / `Socket` slot-item requests ack-fail without falling through into whole-equipment removal, including socket requests that only match the parent equipment id, with full `mir2-simulation` regression green at 584 tests. |
| R77 | Current `EquipItem(grid=Storage)` now resolves the exact storage item through the active `@Storage` service, and current `RemoveItem(grid=Inventory|Storage)` now follows Crystal's exact destination-slot semantics with ack-only packet shape instead of accepting `grid=Equipment` or falling back into another bag slot, with full `mir2-simulation` regression green at 582 tests. |
| R76 | Expired expanded storage now downgrades to inactive on current `StartGame`, then emits Crystal-style expiry chat plus `ResizeStorage` on the first world tick and persists the account flag back to `false` while preserving the 160-slot backing array, with full `mir2-simulation` regression green at 579 tests. |
| R75 | Current `@Storage` open now sends Crystal `UserStorage` with the full backing storage length even when expanded storage is inactive, while higher-slot storage actions remain gated by current accessible capacity, with full `mir2-simulation` regression green at 577 tests. |
| R74 | Repeated unchanged current `@Storage` opens now suppress duplicate `UserStorage` after the first send, matching Crystal `Connection.StorageSent` resend behavior while preserving the locked reopen/unlock resend path, with full `mir2-simulation` regression green at 576 tests. |
| R73 | Successful current `@Storage` open now emits Crystal `UserStorage` before `NPCStorage` when storage is available, and successful `UnlockStorage` now emits `StorageUnlockResult` followed by `UserStorage`, with protocol/gateway/runtime coverage and full `mir2-simulation` regression green at 575 tests. |
| R72 | Reopening Crystal `@Storage` now resets the session unlock state before deciding whether storage contents can be sent, matching `ResetStorageUnlock()`, with full `mir2-simulation` regression green at 575 tests. |
| R71 | Current storage password set/unlock/remove now enforce Crystal's `^[A-Za-z0-9]{5,15}$` password format semantics, with focused storage-password regressions and full `mir2-simulation` regression green at 574 tests. |
| R70 | Current storage password actions now require the active in-range Crystal storage service context, and successful password removal clears `LastSetTime` back to `0`, with full `mir2-simulation` regression green at 572 tests. |
| R69 | Current inventory-grid `CombineItem` current-data coverage now closes the remaining present-data shape-3/4 families and the shape-0 ack-only source surface, with full `mir2-simulation` regression green at 571 tests. |
| R68 | Current inventory-grid `CombineItem` no longer misroutes current-data `DurabilityGem` / `DurabilityOrb` stat-48 control metadata into a fake added stat, so durability upgrades now follow Crystal's `MaxDura` branch and focused regressions lock the current-data durability, attack-speed, magic-resist, and durability-cap surfaces, with full `mir2-simulation` regression green at 565 tests. |
| R67 | Current buy/sell/repair service actions now require the recorded Crystal NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range NPC service context no longer mutates `BuyItem`, `SellItem`, `RepairItem`, or `SRepairItem`, with full `mir2-simulation` regression green at 561 tests. |
| R66 | Current storage-family item actions now require the recorded Crystal storage NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range storage service context now ack-fails across `StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage`, with full `mir2-simulation` regression green at 557 tests. |
| R65 | Current `SplitItem` now matches Crystal's supported-grid and failed-ack surface: only `Inventory` / `Storage` are live, storage splits require active Crystal storage service, and unsupported/invalid/full/locked failures stay ack-only, with full `mir2-simulation` regression green at 555 tests. |
| R64 | Current `SplitItem(grid=Inventory)` now follows Crystal single-array placement across local `Bag1` / `Bag2`, including belt-first placement for belt-eligible items, with full `mir2-simulation` regression green at 552 tests. |
| R63 | Slot-based current `MoveItem`, `StoreItem`, and `TakeBackItem` inventory paths now resolve Crystal single-array indices across local `Bag1` / `Bag2`, including `Bag2` swaps and storage transfers on slots `40+`, with full `mir2-simulation` regression green at 549 tests. |
| R62 | Remaining unsupported `MergeItem` `Storage <-> Belt` cross-grid requests now follow Crystal's ack-only surface without runtime-only `Cross-grid item merge is not available yet.` chat, with full `mir2-simulation` regression green at 546 tests. |
| R61 | Current `MergeItem` now rejects `QuestInventory` requests ack-only without extra chat or quest-item mutation, with full `mir2-simulation` regression green at 544 tests. |
| R60 | Current `MoveItem` now rejects `Belt` / `QuestInventory` requests ack-only, enforces Crystal current inventory slot bounds, and keeps current bag moves from mutating quest items, with full `mir2-simulation` regression green at 542 tests. |
| R59 | Current missing-source `MoveItem` Inventory/Storage failures now use Crystal's `ItemMoveErrorReport` chat surface before the failed ack instead of `sim.itemNotFoundInBag`, with full `mir2-simulation` regression green at 537 tests. |
| R58 | Current successful `MoveItem` current `Inventory` and `Storage` paths now follow Crystal's ack-only surface and no longer emit runtime-only `Item slot updated.` chat, with full `mir2-simulation` regression green at 535 tests. |
| R57 | Current `MoveItem(grid=Storage)` now requires the active Crystal storage service, and inactive-service requests fail ack-only without mutating storage items, with full `mir2-simulation` regression green at 534 tests. |
| R56 | Current `MoveItem` storage-lock and invalid-slot failures now follow Crystal's ack-only surface without extra chat, with full `mir2-simulation` regression green at 533 tests. |
| R55 | Current `MoveItem` unsupported-grid parity now also covers `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player/equipment mutation, and full `mir2-simulation` regression is green at 529 tests. |
| R54 | Current `MergeItem` now supports the next bounded modeled cross-grid surface via `Inventory <-> Belt` stack merges for Crystal belt-eligible items, keeps non-beltable belt cross-grid requests ack-only, and full `mir2-simulation` regression is green at 529 tests. |
| R53 | Current `MergeItem` now supports Crystal-style `Inventory <-> Storage` stack merges through the active storage-service gate, keeps storage-lock/inactive-service failures ack-only, and full `mir2-simulation` regression is green at 523 tests. |
| R52 | Current `MergeItem` same-grid failure/success message shape now follows Crystal's ack-only surface for storage-lock, missing-item, mismatched/full-stack, and success paths, with full `mir2-simulation` regression green at 520 tests. |
| R51 | Current `MergeItem` unsupported-grid parity now also covers `Trade` and `Refine` ack-only failures without extra chat or player-bag mutation, and full `mir2-simulation` regression is green at 517 tests. |
| R50 | Current `MergeItem` unsupported-grid parity now also covers `HeroInventory`, `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player-bag mutation, and full `mir2-simulation` regression is green at 513 tests. |
| R49 | Current `MoveItem` unsupported-grid parity now covers `HeroInventory`, `Trade`, and `Refine` ack-only failures without extra chat or player-bag mutation, and full `mir2-simulation` regression is green at 511 tests. |
| R48 | Crystal current `MoveItem(grid=HeroInventory)` now failed-ack without extra chat or player-bag mutation when hero inventory is unmodeled, and full `mir2-simulation` regression is green at 509 tests. |
| R47 | Crystal current `MergeItem` hero-grid requests now failed-ack without extra chat or player-bag mutation when hero inventory/equipment are unmodeled, and full `mir2-simulation` regression is green at 508 tests. |
| R46 | Crystal current `EquipItem(grid=HeroInventory)`, `RemoveItem(grid=HeroInventory)`, and `RemoveSlotItem(grid=HeroEquipment|HeroInventory)` now failed-ack without mutating matching player inventory/equipment, and full `mir2-simulation` regression is green at 506 tests. |
| R45 | Crystal current `SplitItem(grid=HeroInventory)` now failed-acks without mutating matching player inventory stacks, and full `mir2-simulation` regression is green at 503 tests. |
| R44 | Crystal current `UseItem(grid=HeroInventory)` no longer falls back into player bag items when hero inventory is unmodeled, and full `mir2-simulation` regression is green at 502 tests. |
| R43 | Crystal current `ResurrectionScroll` now respects map `CurrentMap.Info.NoReincarnation`: dead players receive `CannotUseOnMap`, the scroll is preserved, and revive packets are suppressed; full `mir2-simulation` regression is green at 501 tests. |
| R42 | Crystal current `TownTeleport` now respects map `CurrentMap.Info.NoTownTeleport`, emits `NoTownTeleport`, preserves the item, and does not teleport; full `mir2-simulation` regression is green at 500 tests. |
| R41 | Crystal current `UseItem` dead-state parity now rejects ordinary items ack-only while allowing `ResurrectionScroll` to revive only dead players and emit `CannotResurrection` while alive; full `mir2-simulation` regression is green at 499 tests. |
| R40 | Crystal current dead-state item mutation parity now short-circuits `BuyItem`, `DeleteItem`, `SellItem`, `RepairItem`, `DropItem`, and `CombineItem` without mutation, and full `mir2-simulation` regression is green at 496 tests. |
| R38 | Crystal current monster-drop map-rule parity now respects `CurrentMap.Info.NoDropMonster`: normal monster drops, current field-wasp quest drop, and harvest-corpse loot are all suppressed on blocked maps, with full `mir2-simulation` regression green at 490 tests. |
| R37 | Crystal current `DropItem` now respects map `CurrentMap.Info.NoThrowItem`, emits the localized `CanNotDrop` system chat before the failed ack, and preserves inventory/ground state; full `mir2-simulation` regression is green at 488 tests. |
| R36 | Crystal current `DropItem` now also rejects rental `BindingFlags.DontDrop` ack-only, preserving inventory state and rental metadata; full `mir2-simulation` regression is green at 487 tests. |
| R35 | Crystal bounded hero-inventory packet guards are now regression-locked for `DropItem(hero_inventory=true)` and `CombineItem(grid=HeroInventory)`: with no modeled/available hero inventory, both ack-fail without mutating matching player inventory; full `mir2-simulation` regression is green at 486 tests. |
| R34 | Crystal `DeleteItem` now ignores the packet `HeroInventory` flag like the real server and still searches only current player inventory by unique id; full `mir2-simulation` regression is green at 484 tests. |
| R33 | Crystal current item packet unique-id cleanup now resolves packet `UseItem`, packet `EquipItem`, and `MergeItem` by the referenced item unique id instead of duplicate-key fallback or slot aliases; full `mir2-simulation` regression is green at 482 tests. |
| R32 | Crystal current inventory unique-id cleanup now resolves `CombineItem`, `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, and `RepairItem` by item unique id instead of raw slot aliases, and default `Bag1` / `Bag2` ids no longer collide; full `mir2-simulation` regression is green at 479 tests. |
| R31 | Crystal player `GemRatePercent` now contributes to current inventory-grid `CombineItem` shape-3/4 upgrade success chance from non-broken equipped item stats, with 473-test `mir2-simulation` regression green. |
| R30 | Crystal rental `BindingFlags` now persist through runtime item/equipment state, surface in `UserItem.RentalInformation`, block storage `DontStore`, and block current socket/upgrade `CombineItem` `DontUpgrade` ack-only with 472-test `mir2-simulation` regression green. |
| R29 | Crystal inventory-grid `CombineItem` repair-hammer/sewing parity, including `ItemRepaired`, `ItemNoRepairNeeded`, and 469-test `mir2-simulation` regression green. |
| R28 | Crystal `CombineItem` top-level target item-type gating across socket/seal/upgrade packet branches, including 466-test `mir2-simulation` regression green. |
| R27 | Crystal inventory-grid `CombineItem` shape-3/4 gem/orb upgrade parity, including `ItemUpgraded`, persisted `gem_count`, and 465-test `mir2-simulation` regression green. |
| R26 | Crystal inventory-grid `CombineItem` packet parity for current socket-growth and seal branches, including protocol ids/codecs, gateway JSON, runtime dispatch, and 461-test `mir2-simulation` regression green. |
| R25 | Crystal `StoreItem` / `TakeBackItem` active `@Storage` / `NPCStorage` gating, `DontStore`, password-lock/capacity/occupied-target no-swap, and ack-only failure semantics. |
| R18 | Crystal drop visibility and pickup rejection edges. |
| R19 | Crystal `HarvestMonster` pending drop transfer and full-bag retry semantics. |
| R20 | Crystal harvest owner / `EXPOwner` corpse scan rejection. |
| R21 | Crystal sell service gating, partial-stack gold-cap rejection, credit-shop mail delivery, and mail attachment capacity checks. |
| R22 | Crystal `BuyItem` silent no-mutation rejection for invalid panel/count, missing service, non-buy service pages, missing goods/metadata, insufficient gold, and full bags. |
| R23 | Crystal NPC `RepairItem` / `SRepairItem` active-page gating, backpack unique-id lookup, cost, max-dura, and rejection semantics. |
| R24 | Crystal NPC `SellItem` `DontSell`, script type, price, ack-only failure, and gold-cap semantics. |

Restart rule:

- Read `docs/AGENT-RESUME-HANDOFF.md` before continuing after a reboot or context loss.
- Relaunch read-only explorers for any subagent findings that were not written to docs.
- Continue from the R39 map-drop-flag data follow-up only if the local Crystal build assets needed by `packages/tooling/scripts/generate-crystal-respawn-manifest.mjs` are available; otherwise keep the one-writer discipline on `runtime.rs`, leave the unverified manifest-import prep uncounted, and choose the next bounded non-data current-item parity bite from the queue without reopening verified R40-R63 work.
- On this Mac verification environment, use `cargo +1.89.0` for Rust checks/tests unless the toolchain is explicitly pinned later; default `rustc 1.87.0` does not compile locked `bevy_* 0.17.3`.
