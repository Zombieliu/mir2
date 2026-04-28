# Agent Run Log

> Latest product-evolution sync: 2026-04-29-R259 completed. Added `docs/WINDOWS-HOME-STAGING-SERVER.md` to define the home Windows desktop staging-server design: WSL2/Docker host layout, Tailscale-first network access, optional Cloudflare Tunnel path with public Gateway `/admin/*` blocked, service ports, env plan, startup order, backups, rollback, security rules, and acceptance checklist.

> Latest product-evolution sync: 2026-04-29-R258 completed. Added provider-neutral Admin staging rollout assets: `infra/staging.env.example`, `docs/ADMIN-STAGING-RUNBOOK.md`, staging websocket configuration for Player Web through `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` with same-origin `/ws` fallback outside localhost, and README/infra/admin doc links. This prepares staging/internal controlled deployment and keeps production-grade blockers explicit.

> Latest product-evolution sync: 2026-04-29-R257 completed and human-accepted locally. Full local live acceptance is green across Postgres, Redis, NATS, Redpanda, ClickHouse, Gateway, Admin API, Admin Web, and Player Web. Follow-up fixes landed for ClickHouse event/timeline command filters, Postgres-backed GM mail/grant receipt persistence, and local `ai/` artifact ignoring. Verification passed: live command `cmd-live-grant-persist-20260428221245` through peer approval, GM grant, Audit, Timeline, and persisted outbox receipt; admin-api 25+6 tests; admin-web `tsc --noEmit`; `cargo +1.89.0 fmt --check`; `git diff --check`; user confirmed the Admin/Player live surfaces are usable.

> Latest product-evolution sync: 2026-04-28-R254-R256 completed. Admin operator auth now supports Postgres-backed bearer tokens and `/admin/auth/me`, Admin Web has token login/logout and resolved operator display, high-risk command approval now requires a matching peer-approved request, and Gateway automatically posts zone runtime heartbeat records. Verification passed: admin-api 25+6 tests, gateway 57+7 tests, admin-web `tsc --noEmit`, live auth/operators smoke, live cross-operator approval/grant smoke, live Gateway heartbeat readback, Admin Web page smoke, fmt, and diff checks.

> Latest product-evolution sync: 2026-04-28-R251-R253 completed. Admin Servers now has real Postgres zone runtime telemetry, Admin Operators/RBAC has real Postgres operator records and a new Operators page, and the console-wide HTTP smoke is green across 11 pages. Verification passed: admin-api 24+6 tests, admin-web `tsc --noEmit`, live API write/read smoke for zone telemetry and operator RBAC, all Admin Web pages HTTP 200, fmt/diff checks.

> Latest product-evolution sync: 2026-04-28-R250 completed. Admin Activities, Economy price feeds, and Risk trade graph are now real Postgres-backed projections instead of unwired empty states. Admin API has write routes for `/admin/activities`, `/admin/economy/price-feeds`, and `/admin/risk/trade-edges`; Admin Web has server-action forms on the corresponding pages. Verification passed: admin-api 24+6 tests, admin-web `tsc --noEmit`, live Postgres API write/read smoke, Admin Web page HTTP smoke 200s, fmt/diff checks.

> Latest product-evolution sync: 2026-04-28-R249 completed. Gateway now exposes `GET /admin/sessions` from the real session cache, including Redis SCAN/list support with TTL/remove coverage. Admin API overlays Gateway presence onto `/admin/read/dashboard`, `/admin/read/players`, `/admin/read/players/:player_id`, and `/admin/read/servers`, so online totals, player online status, runtime HP/gold/map, and zones-online source are true Gateway/Redis data. Verification passed: focused gateway session-cache tests 8/8, gateway `/admin/sessions` endpoint test, admin-api presence overlay test, admin-web `tsc --noEmit`, and `git diff --check`.

> Latest product-evolution sync: 2026-04-28-R248 completed. Admin Web no longer uses mock read data for Dashboard, Players, Player Detail, Economy, Activities, Servers, or Risk. Rust Admin API now exposes `/admin/read/dashboard`, `/admin/read/players`, `/admin/read/players/:player_id`, `/admin/read/economy`, `/admin/read/activities`, `/admin/read/servers`, and `/admin/read/risk`; these derive player/economy/hot-map/risk data from JSON account-store or explicit Postgres source mode and return empty/unwired states for activity config, market prices, trade graph, and deeper zone telemetry until authoritative projections exist. Verification passed: `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`, admin-web `tsc --noEmit`, and focused admin read-model test coverage.

> Latest product-evolution sync: 2026-04-27-R238 completed. Admin command analytics now include `admin.command.succeeded`, `admin.command.failed`, and `admin.command.denied`; ClickHouse consumes all three Redpanda topics through `clickhouse-admin-events-v2`, and `/admin/events` filter smoke passed for denied and failed events.

> Latest product-evolution sync: 2026-04-27-R237 completed. Admin outbox now records per-publisher delivery state (`nats_status`, `redpanda_status`, `last_error`, `dispatched_at_ms`), `dispatch-admin-outbox` retries instead of marking dispatched when either configured publisher fails, and Admin API/Admin Web now expose filtered/degraded ClickHouse event reads.

> Latest product-evolution sync: 2026-04-27-R236 completed. Admin outbox now has a stable event envelope, `dispatch-admin-outbox` can publish to Redpanda Pandaproxy through `ADMIN_OUTBOX_REDPANDA_URL` while preserving NATS, ClickHouse projects the events into `admin_events` plus `admin_command_events`, Admin API exposes `/admin/events`, and Admin Web Audit shows the event stream.

> Latest product-evolution sync: 2026-04-27-R235 completed. Local event analytics infrastructure now includes Redpanda and ClickHouse in the default dev Compose stack. Redpanda exposes internal/external Kafka listeners; ClickHouse initializes a Kafka-engine table plus materialized view for `admin.command.succeeded` into `mir2_events.admin_command_events`. NATS remains the current admin outbox command/notification dispatcher; Redpanda/ClickHouse are non-authoritative analytics infrastructure.

> Latest product-evolution sync: 2026-04-27-R234 completed. Admin production boundary hardening now includes optional `ADMIN_OPERATOR_TOKEN` Bearer validation, `approvalId` validation for high-risk commands, `GrantItem` and gold `GrantCurrency` execution through audited system-mail delivery, plus admin outbox retry/dead-letter state. Verification passed: `mir2-admin-api` 11/11.

> Latest product-evolution sync: 2026-04-27-R233 completed. Postgres source-of-truth account-store mode now tracks loaded account/save versions and rejects stale source writers instead of overwriting newer DB state. Successful saves refresh in-memory source-version metadata. Docker Postgres tests cover stale writer rejection and reload-then-save success.

> Latest product-evolution sync: 2026-04-27-R232 expanded. Gateway session cache now has an optional Redis adapter selected by `MIR2_GATEWAY_REDIS_CACHE_URL`, TTL via `MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS`, and Redis roundtrip/remove/expiry coverage. The default path remains in-memory when Redis env is unset.

> Latest product-evolution sync: 2026-04-27-R232 completed. Added the first non-authoritative gateway session-cache boundary: `ActiveSessionIdentity` in simulation, `GatewaySessionCache` / `InMemoryGatewaySessionCache` in gateway, web-gateway cache refresh after authoritative saves, and disconnect cleanup. Focused verification passed with gateway cache tests 4/4 and `cargo +1.89.0 fmt --check`. Redis remains the external cache/session/routing target and needs a real adapter in a later slice.

> Latest product-evolution sync: 2026-04-27-R229 completed. Admin API now has the first live-verified Postgres/NATS persistence slice: `infra/postgres/migrations/0001_core.sql`, Postgres-backed `AdminCommandRepository` / `AuditRepository` adapters selected by `ADMIN_DATABASE_URL`, an `AdminOutboxRepository` boundary with in-memory and Postgres implementations, `import-account-store` for importing `.mir2-data/accounts.json` into `accounts`, `characters`, and `character_saves`, and `dispatch-admin-outbox` for publishing pending outbox rows to NATS. Docker integration smoke passed after Docker Desktop started: core compose services are healthy, `.mir2-data/admin-live-smoke.json` imported demo/Scout at `0103 WeaponStore (6,12)`, Admin API on `127.0.0.1:7421` wrote command/audit/outbox rows to Postgres, and the outbox worker published `admin.command.succeeded` to NATS before marking the row `dispatched`. Verification also passed: `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (8/8), `cargo +1.89.0 fmt --check`, compose config, and `git diff --check`.

> Latest product-evolution sync: 2026-04-27-R230 completed. Gameplay account-store persistence now has an optional Postgres mirror: `SimulationConfig::with_account_store_database_url`, `MIR2_ACCOUNT_STORE_DATABASE_URL` wiring in gateway and Admin API fallback mail, and blocking-thread Postgres upserts for accounts/characters/character_saves after JSON saves. Docker smoke verified fallback GM mail wrote `stage5_systems_json` into Postgres, then the DB was restored to the `admin-live-smoke` demo/Scout `0103 WeaponStore (6,12)` state. Verification passed: `mir2-simulation config` 11/11, `mir2-admin-api` 8/8, `mir2-gateway` 55/55, `cargo +1.89.0 fmt --check`, `git diff --check`, and Docker core services healthy.

> Latest product-evolution sync: 2026-04-27-R231 completed. Gameplay account-store Postgres source-of-truth mode is now explicit opt-in through `MIR2_ACCOUNT_STORE_BACKEND=postgres`. `SimulationConfig::with_postgres_account_store` loads from Postgres `accounts.raw_json`, initializes a default account if empty, and saves through transactions with account row locks plus `store_version` / `save_version` increments. Gateway and Admin API fallback mail both support the mode. Docker smoke verified a Postgres-source fallback GM mail kept Scout at `0103 WeaponStore`, added mail in DB, and incremented versions from 0 to 1. Verification passed: `mir2-simulation config` 11/11, `mir2-admin-api` 8/8, `mir2-gateway` 55/55, `cargo +1.89.0 fmt --check`, compose config/healthy services, and `git diff --check`.

> Latest architecture sync: 2026-04-27. Added `docs/ARCHITECTURE-ADOPTION-PLAN.md` and `infra/docker-compose.dev.yml`. Immediate architecture additions are Postgres, Redis, NATS, local Docker Compose, storage/cache/command-bus/event-envelope boundaries, and continuing Rust Axum for Admin API. Redpanda, ClickHouse, Meilisearch, Loki, and Grafana are optional Compose profiles. KCP/QUIC, Kubernetes, Temporal, Flink, Spark/Data Lake, OpenSearch, and Sui Move gameplay integration are deferred.

> Latest truth-audit sync: 2026-04-27. Added `docs/PARITY-TRUTH-AUDIT.md` and linked it from the handoff/queue/roadmap/Windows docs. Authoritative wording is now: automated parity evidence **100% Candidate**, backend/server tracked slice **99.70% Candidate**, whole-project accepted Crystal 1:1 **roughly 90%**. Fallbacks and blockers such as synthetic map terrain, missing `Server.MirDB`, missing `MIR2_CRYSTAL_TCP_ADDR`, Admin read-model gaps/unwired projections, in-memory/local JSON persistence, and human visual/feel acceptance must not be counted as final Accepted 1:1.

> Latest product-evolution sync: R228 completed. Admin `SendSystemMail` now reaches live game-visible Stage 5 mail through Admin Web -> Admin API -> gateway, with account-store fallback. Runtime smoke delivered mail to `Scout` using `deliveryMode: "gateway_live"` and claimed it through gateway WS `stage5Command mail.claim`.

> Latest product-evolution sync: 2026-04-27 admin-web i18n completed for the current operations console. Added cookie-based `en` / `zh-CN` server-rendered dictionaries, a top-bar language switcher, and translations across navigation, page headers, tables, primary controls, GM mail form copy, status labels, and empty states. Verification: admin-web `tsc --noEmit`, admin-web `next build`, and curl smoke with `Cookie: admin_locale=zh-CN` on `/` and `/gm-tools`.

> Latest sync: R225 completed. Mac-local Candidate regression is green again: web `tsc --noEmit`, direct `next build`, Stage 5 UI smoke (88 screenshots), map API smoke 18/18, minimap asset smoke 0 failures with known 450/451 warning, WS load 64/64, `mir2-game-data` 22/22, `mir2-gateway` 54/54, `mir2-simulation` 664/664, require-local `packet_trace --matrix` wrote 9 artifacts and 17 intended skips under `docs/generated/packet-traces/r225-matrix`, `cargo +1.89.0 fmt --check`, and `git diff --check`. Active follow-up round is R226 for Windows continuation / external blockers.

> Latest sync: R224 completed. Restored the `mir2-gateway` `packet_trace` bin target and refreshed local matrix evidence: `--list-flows` works, `mir2-gateway` passes 53/53 including packet trace bin tests 6/6, and require-local `packet_trace --matrix` wrote 9 artifacts with `localOk=true` plus 17 intentionally skipped non-TCP entries. The automated gate remains **100% Candidate** (not 100% Accepted). Active follow-up round is R225 for human acceptance / external blockers.

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

Purpose: record autonomous multi-agent rounds, assignments, outputs, verification, and progress updates.

## 2026-04-29-R258

Scope:

- Added `infra/staging.env.example` with placeholder-only staging variables for Admin API, Gateway, Admin Web, Player Web, Postgres, Redis, NATS, Redpanda, and ClickHouse.
- Added `docs/ADMIN-STAGING-RUNBOOK.md` covering staging topology, required services, env matrix, bootstrap/import, operator seeding, Redpanda topics, outbox worker startup, smoke checklist, rollback/recovery, and remaining production blockers.
- Made Player Web Gateway routing staging-friendly by reading `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL`, falling back to same-origin `/ws` outside localhost, and preserving the local `ws://127.0.0.1:7110/ws` default.
- Updated `README.md`, `infra/README.md`, `apps/web/README.md`, `apps/admin-api/README.md`, `apps/admin-web/README.md`, and `docs/ADMIN-OPERATIONS-ARCHITECTURE.md` to link the staging runbook and keep production readiness gates explicit.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `docker compose -f infra/docker-compose.dev.yml config`
- duplicate-key scan for `infra/staging.env.example`
- `git diff --check`

Result:

- The Admin operations stack now has a concrete staging handoff path after local live acceptance.
- This does not mark the operations center production-grade; production blockers remain external IdP/session auth, TLS/network isolation, secret rotation, observability, launch approval policy, backup/restore rehearsal, rate limits, and deployed soak/reconnect evidence.

## 2026-04-29-R259

Scope:

- Added `docs/WINDOWS-HOME-STAGING-SERVER.md` as the concrete design for using the user's home Windows desktop as the first internal staging server.
- Chose WSL2 Ubuntu plus Docker Desktop WSL2 backend, with the repo in the WSL filesystem and Docker volumes for Postgres/Redis/NATS/Redpanda/ClickHouse.
- Recommended Tailscale-only access first, then optional Cloudflare Tunnel with Admin Web protected and Gateway public exposure limited to `/ws` through a local reverse proxy.
- Defined port exposure, home staging env values, startup order, operator bootstrap, backup/restore, update/rollback, firewall rules, and acceptance checklist.
- Linked the new design from README, infra README, and the Admin staging runbook.

Validation:

- `git diff --check`
- docs link/reference review for README, infra README, and Admin staging runbook

Result:

- The staging plan now supports the user's preferred deployment model: a home Windows desktop acting as `home-staging-1` for closed internal testing.

## 2026-04-29-R257

Scope:

- Ran the complete local Admin operations stack for live acceptance: Docker Postgres, Redis, NATS, Redpanda, and ClickHouse; Gateway on `127.0.0.1:7110`; Admin API on `127.0.0.1:7420`; Admin Web on `127.0.0.1:3020`; Player Web on `127.0.0.1:3010`.
- Verified Postgres-backed operator-token auth with `r254-lead-token` resolving to `ops-r254-lead` and browser login through Admin Web.
- Exercised Operators, Approvals, GM Tools, Servers, Audit, and Timeline against live data.
- Fixed ClickHouse event reads so command/event/status filters are applied after per-`event_id` aggregation. This prevents `/admin/events?commandId=...` and `/admin/timeline?commandId=...` from degrading on filtered reads.
- Added `admin_system_mail_receipts` to Postgres and merged persisted receipts into `GET /admin/system-mail/outbox`, so GM Tools can read gateway/account-store delivery receipts after Admin API restart.
- Ignored transient local `ai/*.png` reference/generated images through `.gitignore`.

Validation:

- `GET /health` passed for Gateway `:7110` and Admin API `:7420`.
- `GET /login` on Admin Web `:3020` and `/` on Player Web `:3010` returned HTTP 200.
- Live peer-approved grant command `cmd-live-grant-persist-20260428221245` succeeded with result `currency grant queued as mail-cmd-live-grant-persist-20260428221245`.
- `/admin/events?commandId=cmd-live-grant-persist-20260428221245` returned `degraded: false` and projected command/approval events from Redpanda -> ClickHouse.
- `/admin/timeline?commandId=cmd-live-grant-persist-20260428221245` returned merged audit, command, approval, and event records with `degraded: false`.
- `/admin/system-mail/outbox` returned persisted receipt `mail-cmd-live-grant-persist-20260428221245`, `deliveryMode: "gateway_live"`, `deliveredCount: 2`, and mail ids `[8,5]`.
- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`
- `./node_modules/.bin/tsc --noEmit` in `apps/admin-web`
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- The local live Admin operations stack passed human inspection for the checked Admin Web and Player Web surfaces.
- The remaining step is deployment preparation or broader QA against a shared/staging environment.

## 2026-04-27-R238

Scope:

- Changed Postgres-backed command completion to emit terminal command analytics events for success, failure, and permission denial.
- Added the `command_event_type` mapping for `admin.command.succeeded`, `admin.command.failed`, and `admin.command.denied`.
- Updated ClickHouse init SQL to rebuild the Kafka source and materialized views while preserving MergeTree target tables, subscribe to all three command topics, and use the `clickhouse-admin-events-v2` group.
- Added Admin Web Audit denied status filtering.

Validation:

- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (16 lib tests + 4 dispatcher tests)
- `./node_modules/.bin/tsc --noEmit` in `apps/admin-web`
- Applied `infra/clickhouse/initdb/001_admin_events.sql` to the running ClickHouse container.
- Created Redpanda topics `admin.command.failed` and `admin.command.denied`.
- Submitted real Admin API command `r238-denied` without `mail_send_system`; API returned 403 and Postgres outbox contained `admin.command.denied`.
- Published and projected `r238-failed` as `admin.command.failed`.
- `dispatch-admin-outbox -- --once` delivered both denied and failed events through NATS and Redpanda.
- ClickHouse returned `outbox-r238-denied admin.command.denied r238-denied denied` and `outbox-r238-failed admin.command.failed r238-failed failed`.
- Admin API `/admin/events?eventType=admin.command.denied&status=denied&limit=5` and `/admin/events?eventType=admin.command.failed&status=failed&limit=5` returned `degraded: false` with the expected records.

Result:

- R238 makes the Admin event stream useful for negative operational outcomes, not just successful commands.
- Retry/dead-letter remain outbox delivery state for now; they should become first-class lifecycle analytics in a later slice without recursively depending on the same outbox path.

## 2026-04-27-R237

Scope:

- Added `nats_status`, `redpanda_status`, `last_error`, and `dispatched_at_ms` to `admin_outbox`.
- Updated in-memory and Postgres outbox repositories to record delivery attempts, retry with backoff, dead-letter exhausted rows, and keep `dispatched_at_ms` only for complete success.
- Changed `dispatch-admin-outbox` to attempt NATS and Redpanda independently, record each result, and avoid `dispatched` when any configured publisher fails.
- Hardened `GET /admin/events` with `limit`, `commandId`, `eventType`, and `status` filters plus a degraded response shape for ClickHouse outages.
- Added Admin Web Audit filters and a separate event-stream status badge so ClickHouse read-side degradation does not collapse command/audit visibility.

Validation:

- Docker services Postgres, Redis, NATS, Redpanda, and ClickHouse were healthy.
- Applied `infra/postgres/migrations/0001_core.sql` to the local Postgres container.
- Redpanda success plus NATS failure left `outbox-r237-nats-fail` at `retry|failed|succeeded|1|last_error set|dispatched_at_ms null`.
- NATS success plus Redpanda failure left `outbox-r237-redpanda-fail` at `retry|succeeded|failed|1|last_error set|dispatched_at_ms null`.
- NATS plus Redpanda success dispatched `outbox-r237-success` with both publisher statuses `succeeded` and `dispatched_at_ms` set; ClickHouse `mir2_events.admin_events` returned `outbox-r237-success admin.command.succeeded r237-success succeeded`.
- Admin API `/admin/events?commandId=r237-success&eventType=admin.command.succeeded&status=succeeded&limit=5` returned the filtered record with `degraded: false`.
- Admin API with an unreachable ClickHouse URL returned HTTP 200 with `degraded: true`, an error string, and empty records.
- `cargo +1.89.0 test --locked -p mir2-simulation config -- --test-threads=1` (13/13)
- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (15 lib tests + 4 dispatcher tests)
- `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (53 gateway tests + 7 packet trace tests)
- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/tsc --noEmit` in `apps/admin-web`
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- R237 closes the first multi-publisher delivery correctness gap for the Admin outbox and makes ClickHouse event reads operationally degradable.
- NATS remains the lightweight command/notification path; Redpanda/ClickHouse remain non-authoritative analytics/read-side infrastructure.

## 2026-04-27-R236

Scope:

- Added stable admin event envelopes with `eventId`, `eventType`, `schemaVersion`, `commandId`, `operatorId`, `status`, `occurredAtMs`, `payload`, and `payloadJson`.
- Updated Postgres-backed Admin command completion to enqueue envelope-shaped `admin.command.succeeded` outbox payloads.
- Extended `dispatch-admin-outbox` to publish configured Redpanda events through Pandaproxy via `ADMIN_OUTBOX_REDPANDA_URL`, while preserving NATS publication and only marking outbox rows dispatched after configured publishers succeed.
- Expanded ClickHouse init SQL with `admin_events`, `admin_events_kafka`, and materialized views into both `admin_events` and the compatibility `admin_command_events` projection.
- Added Admin API `GET /admin/events` to read recent ClickHouse-projected admin events and added the event stream to the Admin Web Audit page.

Validation:

- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (14 lib tests + 2 dispatcher tests)
- Redpanda restarted with Pandaproxy exposed at `127.0.0.1:8082`; Redpanda and ClickHouse healthy.
- Applied `infra/clickhouse/initdb/001_admin_events.sql` to the local ClickHouse volume.
- Submitted real Admin API `SendSystemMail` command `r236-redpanda-clickhouse-smoke`, which wrote a pending Postgres `admin_outbox` row with envelope fields.
- Ran `dispatch-admin-outbox -- --once` with `ADMIN_OUTBOX_REDPANDA_URL=http://127.0.0.1:8082`; the outbox row became `dispatched`.
- ClickHouse `admin_events` returned `outbox-r236-redpanda-clickhouse-smoke admin.command.succeeded r236-redpanda-clickhouse-smoke r236-operator succeeded`.
- ClickHouse `admin_command_events` returned `r236-redpanda-clickhouse-smoke succeeded system mail queued as mail-r236-redpanda-clickhouse-smoke`.
- Admin API `GET /admin/events` returned the projected event stream from ClickHouse.

Result:

- R236 completes the first real Admin outbox -> Redpanda -> ClickHouse read-side pipeline.
- NATS remains the lightweight command/notification bus; Redpanda/ClickHouse remain non-authoritative analytics/read-side infrastructure.

## 2026-04-27-R235

Scope:

- Added Redpanda and ClickHouse to the default local Compose event/analytics stack.
- Configured Redpanda with separate internal and external Kafka listeners so ClickHouse can use `redpanda:9092` while host tools use `127.0.0.1:9092`.
- Added `infra/clickhouse/initdb/001_admin_events.sql` to create `mir2_events.admin_command_events`, a Kafka-engine source table for `admin.command.succeeded`, and a materialized view into the MergeTree table.
- Updated infra/architecture/admin docs to keep NATS as the current command/notification dispatcher and mark Redpanda/ClickHouse as non-authoritative analytics infrastructure.

Validation:

- `docker compose -f infra/docker-compose.dev.yml config`
- `docker compose -f infra/docker-compose.dev.yml up -d postgres redis nats redpanda clickhouse`
- `docker compose -f infra/docker-compose.dev.yml ps` showed Postgres, Redis, NATS, Redpanda, and ClickHouse healthy.
- `docker exec mir2-redpanda rpk topic create admin.command.succeeded`
- Produced one JSONEachRow event to Redpanda topic `admin.command.succeeded`.
- ClickHouse query returned `smoke-redpanda succeeded ok 123` from `mir2_events.admin_command_events`.

Result:

- R235 introduces the local event analytics baseline without making Redpanda or ClickHouse part of gameplay authority.

## 2026-04-27-R228

Scope:

- Connected audited `SendSystemMail` execution to live game-visible Stage 5 mail.
- Added `apps/gateway` `POST /admin/system-mail` to write into the running gateway `SimulationConfig.account_store`.
- Added `apps/admin-api` gateway delivery through `ADMIN_GATEWAY_MAIL_URL`, plus persistent account-store fallback through `ADMIN_ACCOUNT_STORE_PATH` / `MIR2_ACCOUNT_STORE_PATH`.
- Added `apps/simulation` mail delivery helper that persists Stage 5 mail into `CharacterSaveRecord.stage5_systems_json`.
- Added player web Mail panel claim/delete actions for the delivered messages.
- Updated admin architecture docs to describe the live gateway mail flow and local env.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_system_mail_delivery_persists_to_character_save -- --nocapture`
- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (7/7)
- `cargo +1.89.0 test --locked -p mir2-gateway admin_system_mail_endpoint_writes_live_account_store -- --nocapture`
- `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (54/54)
- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `./node_modules/.bin/tsc --noEmit` in `apps/admin-web`
- `./node_modules/.bin/next build` in `apps/admin-web`
- Runtime smoke with gateway `127.0.0.1:7110`, Admin API `127.0.0.1:7420`, and Admin Web `127.0.0.1:3020`: Admin Web POST returned a Rust command id; Admin API outbox showed `deliveryMode: "gateway_live"`, `deliveredCount: 1`, `mailIds: [1]`; account-store inspection showed the mail under `stage5_systems_json`; gateway WS world snapshot exposed it at `payload.stage5Systems.mail`; WS `stage5Command mail.claim` marked it claimed, raised gold from 1280 to 6280, and added one `red-potion`.

Result:

- R228 complete.
- Admin system mail is now a real game-visible API path for the local gateway/account-store runtime.
- Remaining admin production gaps: Postgres command/audit repositories, real operator auth/session, approval workflows, gateway admin endpoint hardening, and broader live command executors.

## 2026-04-27-R229

Scope:

- Added Postgres command/audit repositories behind `ADMIN_DATABASE_URL`.
- Added `infra/postgres/migrations/0001_core.sql` for accounts, characters, character saves, admin commands, admin audit records, and admin outbox.
- Added `AdminOutboxRepository`, Postgres outbox persistence, and automatic `admin.command.succeeded` outbox enqueue for successful Postgres-backed commands.
- Added `import-account-store` to import the JSON account store into Postgres-shaped account/character/save tables.
- Added `dispatch-admin-outbox` to publish pending outbox rows to NATS and mark them `dispatched`.
- Fixed Postgres-backed Admin API runtime startup and request handling by initializing sync Postgres state before Tokio runtime start and running repository calls inside `spawn_blocking`.
- Fixed the NATS compose healthcheck to use the NATS TCP protocol through `nc`, making `mir2-nats` report healthy.

Validation:

- `docker compose -f infra/docker-compose.dev.yml up -d postgres redis nats`
- `ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 cargo +1.89.0 run --locked -p mir2-admin-api --bin import-account-store -- .mir2-data/admin-live-smoke.json`
- Postgres query confirmed `1` account, `1` character, `1` save, and `demo / Scout / 0103 / WeaponStore / 6 / 12`.
- Postgres-backed Admin API on `127.0.0.1:7421` accepted `POST /admin/commands/send-system-mail` and wrote matching `admin_commands`, `admin_audit_records`, and pending `admin_outbox` rows.
- Raw NATS subscriber received `MSG admin.command.succeeded`; `dispatch-admin-outbox -- --once` marked the row `dispatched`.
- `docker compose -f infra/docker-compose.dev.yml ps nats` now reports `healthy`.
- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (8/8)
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- R229 complete.
- Admin command/audit/outbox persistence is now live-verified against local Docker Postgres and NATS.
- Gameplay runtime persistence still defaults to JSON account store; moving authoritative gameplay saves to Postgres is the next storage slice.

## 2026-04-27-R230

Scope:

- Added optional Postgres mirror persistence for gameplay account-store saves through `SimulationConfig::with_account_store_database_url`.
- Wired `MIR2_ACCOUNT_STORE_DATABASE_URL` into gateway startup and Admin API account-store fallback mail.
- Kept JSON as the runtime source of truth; Postgres receives mirrored `accounts`, `characters`, and `character_saves` rows after JSON saves.
- Runs the Postgres mirror on a blocking thread so gateway/Admin API Tokio workers do not hit nested-runtime panics.

Validation:

- Docker smoke with `ADMIN_API_ADDR=127.0.0.1:7422`, invalid `ADMIN_GATEWAY_MAIL_URL`, `ADMIN_ACCOUNT_STORE_PATH=.mir2-data/postgres-mirror-smoke.json`, and `MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2`.
- `POST /admin/commands/send-system-mail` succeeded through account-store fallback.
- Postgres query confirmed `character_saves.stage5_systems_json` contained latest mail subject `Gameplay Mirror Smoke`.
- Re-imported `.mir2-data/admin-live-smoke.json` afterward so Docker DB demo/Scout returned to `0103 WeaponStore (6,12)`.
- `cargo +1.89.0 test --locked -p mir2-simulation config -- --test-threads=1` (11/11)
- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (8/8)
- `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (55/55)
- `cargo +1.89.0 fmt --check`
- `git diff --check`
- `docker compose -f infra/docker-compose.dev.yml ps` shows Postgres, Redis, and NATS healthy.

Result:

- R230 complete.
- The project has a verified bridge from JSON gameplay persistence to Postgres.
- Next storage slice is a true Postgres source-of-truth account-store backend with locking/versioning, not just mirroring.

## 2026-04-27-R231

Scope:

- Added explicit Postgres account-store source-of-truth mode through `MIR2_ACCOUNT_STORE_BACKEND=postgres`.
- `SimulationConfig::with_postgres_account_store` now loads account state from Postgres `accounts.raw_json`, initializes a default account when the DB is empty, and disables JSON file writes for that config.
- Account-store source writes run in a transaction, lock each account row with `FOR UPDATE`, and increment `accounts.store_version` / `character_saves.save_version`.
- Gateway and Admin API fallback mail now honor `MIR2_ACCOUNT_STORE_BACKEND=postgres`.

Validation:

- Re-imported `.mir2-data/admin-live-smoke.json` into Docker Postgres before smoke.
- Ran Admin API on `127.0.0.1:7423` with `MIR2_ACCOUNT_STORE_BACKEND=postgres` and fallback gateway URL disabled.
- `POST /admin/commands/send-system-mail` succeeded through Postgres source mode.
- Postgres query confirmed `store_version = 1`, `save_version = 1`, `map_file_name = 0103`, `map_title = WeaponStore`, latest mail subject `Postgres Source Smoke`, and matching admin command status `succeeded`.
- Re-imported `.mir2-data/admin-live-smoke.json` after the smoke to restore demo/Scout map/position state.

Validation continued:

- `cargo +1.89.0 test --locked -p mir2-simulation config -- --test-threads=1` (11/11)
- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (8/8)
- `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (55/55)
- `cargo +1.89.0 fmt --check`
- `docker compose -f infra/docker-compose.dev.yml config`
- `docker compose -f infra/docker-compose.dev.yml ps` shows Postgres, Redis, and NATS healthy.
- `git diff --check`

Result:

- R231 complete.
- Postgres source-of-truth account-store mode is available but explicit opt-in; JSON remains the default backend.
- Remaining work: add automated integration tests for conflict handling and decide whether to make Postgres backend the default for non-parity development.

## 2026-04-27-R232

Scope:

- Added `ActiveSessionIdentity` to `apps/simulation` so gateway/cache boundaries can identify authenticated active characters without reading simulation internals.
- Added `apps/gateway/src/cache.rs` with `GatewaySessionCacheKey`, `GatewaySessionCacheRecord`, `GatewaySessionCache`, `InMemoryGatewaySessionCache`, `RedisGatewaySessionCache`, and refresh/read/remove helpers.
- Wired the web gateway to refresh the cache after each authoritative character save and remove the online session record on disconnect.
- Kept the cache non-authoritative; gameplay state still comes from simulation/account-store persistence.

Validation:

- `cargo +1.89.0 test --locked -p mir2-gateway session_cache -- --test-threads=1` (5/5)
- `cargo +1.89.0 fmt --check`

Result:

- R232 complete.
- The first Redis-boundary contract is in place with deterministic in-memory tests and a local Redis adapter smoke.
- Remaining cache work: broader reconnect/route-cache semantics and cache-hit/cache-miss equivalence at gateway load-test scale.

## 2026-04-27-R233

Scope:

- Added source-version metadata to `AccountStore` for Postgres source-of-truth loads.
- Source-mode saves now compare expected loaded `accounts.store_version` and `character_saves.save_version` against the locked DB rows before writing.
- Successful source-mode saves return and refresh the in-memory version metadata so the same process can continue saving.
- Added Docker Postgres tests for stale writer rejection and reload-save version refresh.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation postgres_source_mode -- --test-threads=1` (2/2)

Result:

- R233 complete.
- Postgres source mode no longer silently permits stale local writers to overwrite newer account/save rows.

## 2026-04-27-R234

Scope:

- Added optional `ADMIN_OPERATOR_TOKEN` Bearer validation to Admin API operator header parsing.
- Added `approvalId` validation for high-risk commands: global system mail, item/currency grants, and account bans.
- Added `GrantItem` and gold `GrantCurrency` execution through the existing audited system-mail delivery boundary.
- Added admin outbox retry/dead-letter transitions and dispatcher retry/dead-letter handling.

Validation:

- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1` (11/11)

Result:

- R234 complete.
- Admin writes remain audited command executions; the new grant paths still avoid direct Admin UI/table mutation of gameplay state.

## 2026-04-26-R225

Scope:

- Added Stage 5 UI smoke manifest summary counts for screenshots, compact panel bounds, compact text nodes, critical console errors, and major flow counts.
- Added packet trace matrix summary counts to `latest-matrix.json`, including local/Crystal/diff status buckets and accepted live comparison count.
- Refreshed Stage 5 screenshots/manifest, map API evidence, minimap asset evidence, WS load evidence, and local packet trace matrix artifacts.
- Added `docs/WINDOWS-CONTINUATION.md` and rewrote stale `apps/gateway/README.md` status so Windows continuation does not inherit old stub language.

Validation:

- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `npm run smoke:stage5-ui` in `apps/web` (88 screenshots)
- `npm run smoke:crystal-map-api` in `apps/web` (18/18 requests)
- `npm run smoke:crystal-minimap-assets` in `apps/web` (0 failures; known 450/451 warning)
- `npm run load:gateway-ws` in `apps/web` (64/64 ready, 0 errors, 1320 messages)
- `cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1` (22/22)
- `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (54/54)
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (664/664)
- `MIR2_GATEWAY_TCP_ADDR=127.0.0.1:7310 MIR2_PACKET_TRACE_MATRIX_OUT_DIR=docs/generated/packet-traces/r225-matrix MIR2_PACKET_TRACE_REQUIRE_LOCAL=1 cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix` (9 artifacts, 17 skipped)
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- R225 complete.
- Automated evidence remains **100% Candidate**.
- Backend/server tracked-slice parity remains **99.70%**.
- Real full-project accepted 1:1 remains **roughly 90.0%** until human Crystal visual/feel acceptance, live Crystal trace comparison, and blocked source-data decisions are closed.
- R226 opened for Windows continuation / external blocker tracking.

## 2026-04-26-R224

Scope:

- Restored `apps/gateway/src/bin/packet_trace.rs` with `--list-flows`, single-flow capture, `--matrix`, local/Crystal endpoint capture, packet diff summaries, fixture metadata, artifact writing, and require-mode enforcement.
- Added an outer `.gitignore` exception so Rust source under `mir2-web3/apps/gateway/src/bin` is trackable despite the repo-wide .NET `**/bin/` ignore.
- Refreshed local packet trace matrix evidence under `docs/generated/packet-traces/r224-matrix` using a dedicated local gateway on `127.0.0.1:7310`.

Validation:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-gateway --bin packet_trace -- --test-threads=1 --nocapture` (6/6)
- `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (53/53)
- `cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --list-flows`
- `MIR2_GATEWAY_TCP_ADDR=127.0.0.1:7310 MIR2_PACKET_TRACE_MATRIX_OUT_DIR=docs/generated/packet-traces/r224-matrix MIR2_PACKET_TRACE_REQUIRE_LOCAL=1 cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix` (9 artifacts, 17 skipped)

Result:

- R224 complete.
- Local packet trace matrix blocker closed.
- Live Crystal trace comparison remains blocked until `MIR2_CRYSTAL_TCP_ADDR` is provided.
- R225 opened for human acceptance / external blocker tracking.

## 2026-04-26-R223

Scope:

- Added Stage 5 UI smoke/manifest coverage for trade item offer/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, second mining/craft path, and mail delete state.
- Added compact viewport panel bounds for Mail and Report.
- Refreshed `docs/stage5-screenshots/stage5-ui-smoke-manifest.json` with 88 screenshots, including `stage5-systems-advanced.png`, `stage5-compact-mail.png`, and `stage5-compact-report.png`.
- Refreshed map API, minimap asset, and WS load JSON evidence.
- Attempted local matrix packet trace refresh, but `cargo +1.89.0 run -p mir2-gateway --bin packet_trace -- --matrix` failed because this tree then had no `packet_trace` bin target. R224 restored the target and refreshed local matrix artifacts.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (88 screenshots)
- `npm --prefix apps/web run smoke:crystal-map-api` (18/18 requests)
- `npm --prefix apps/web run smoke:crystal-minimap-assets` (0 failures; known 450/451 warning)
- `npm --prefix apps/web run load:gateway-ws` (64/64 ready, 0 errors)
- `cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1` (22/22)
- `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (47/47)
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (664/664)
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- R223 complete.
- Whole-project 1:1 estimate advanced to **100.0% Candidate**.
- R224 opened for human acceptance / external blocker tracking.

## 2026-04-26-R219-through-R222

Scope:

- Added Stage 5 UI smoke/manifest coverage for login language switching, View Key, credential fill, Enter submit, select language switching, Credits, Delete cancel, New Character, confirmed Delete Character, recreate, slot selection, and Start.
- Added compact viewport panel bounds for inventory, storage, character, system menu, and chat settings; fixed compact system-menu overflow with a max-height scroll boundary.
- Added NPC dialog link-capable rendering in the client and manifest recording of dialog links when present.
- Added persistent JSON smoke evidence for Crystal map API and minimap assets under `docs/generated/map` and `docs/generated/assets`.
- Refreshed `docs/generated/load/latest-ws.json` with 64/64 WebSocket clients ready and 0 errors.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (85 screenshots)
- `npm --prefix apps/web run smoke:crystal-map-api` (18/18 requests)
- `npm --prefix apps/web run smoke:crystal-minimap-assets` (0 failures; known 450/451 warning)
- `npm --prefix apps/web run load:gateway-ws` (64/64 ready, 0 errors)
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- R219-R222 complete.
- Whole-project 1:1 estimate advanced to 90.0%.
- R223 reopened at global queue-selection stage.

## 2026-04-26-R210-through-R218

Scope:

- Added Stage 5 UI smoke/manifest coverage for Mail, Report, NPC dialog, system-menu QA transfer, and system-menu transfer-list routing.
- Added broad systems state coverage for group loot mode, guild rank/chat, social friend/block/unblock, mail, trade, conquest, hero, mining, and craft state.
- Added guild chat filter and empty group filter evidence.
- Added Character repair and special-repair entry buttons, with smoke evidence that selecting Dagger submits each mode and preserves equipment without an active repair service.
- Added ground Blue Potion pickup and ground gold pickup evidence using Crystal current-cell pickup semantics.
- Added combat target state evidence, Battle Focus spell-cast/buff/cooldown evidence, and compact inventory panel bounds evidence.
- Added screenshots including `stage5-system-menu-qa-transfer.png`, `stage5-system-menu-qa-transfer-result.png`, `stage5-chat-guild-filter.png`, `stage5-chat-group-filter-empty.png`, `stage5-character-repair-mode.png`, `stage5-character-special-repair-mode.png`, `stage5-ground-pickup-blue-potion.png`, `stage5-ground-pickup-gold.png`, `stage5-system-menu-transfer-list-result.png`, `stage5-character-cast-battle-focus.png`, and `stage5-compact-inventory.png`.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (71 screenshots)
- visual inspection of the new panel, repair, pickup, transfer, spell, and compact screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- R210-R218 complete.
- Whole-project 1:1 estimate advanced to 80.0%.
- R219 reopened at global queue-selection stage.

## 2026-04-26-R209

Scope:

- Added Stage 5 UI smoke coverage for Set Storage Password input handling.
- Verified mismatched confirmation keeps submit disabled and renders the password mismatch warning.
- Verified matching `Safe123` submit without an active storage service leaves `hasStoragePassword=false` and surfaces no-service feedback.
- Added `stage5-storage-password-mismatch.png`, `stage5-storage-password-submit-no-service.png`, and extended `storagePasswordFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (60 screenshots)
- visual inspection of the storage password mismatch/no-service screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R209` complete.
- Whole-project 1:1 estimate advanced to 74.4%.
- R210 reopened at global queue-selection stage.

## 2026-04-26-R208

Scope:

- Enabled the storage Protect button when no storage password is set.
- Exposed storage password state in `window.__mir2Stage5.state`.
- Added Stage 5 UI smoke coverage that opens/closes Set Storage Password without submitting credentials.
- Added `stage5-storage-password-panel.png` and `storagePasswordFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (58 screenshots)
- visual inspection of the storage password screenshot
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R208` complete.
- Whole-project 1:1 estimate advanced to 74.2%.
- R209 reopened at global queue-selection stage.

## 2026-04-26-R207

Scope:

- Added storage Take Back no-service smoke coverage.
- Verified selecting an inventory slot for stored Red Potion without an active storage service preserves bag1 Red Potion quantity 3 and storage Red Potion quantity 10.
- Added `stage5-storage-takeback-red-potion-selected.png`, `stage5-storage-takeback-red-potion-result.png`, `stage5-storage-takeback-red-potion-feedback.png`, and `storageTakeBackFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (57 screenshots)
- visual inspection of Take Back screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R207` complete.
- Whole-project 1:1 estimate advanced to 74.0%.
- R208 reopened at global queue-selection stage.

## 2026-04-26-R206

Scope:

- Added storage Store Item no-service smoke coverage.
- Exposed `storageItems` in the Stage 5 debug state so the smoke can compare existing warehouse contents.
- Verified selecting a warehouse slot for Dagger without an active storage service preserves Dagger in bag1 slot 4 and does not add a storage Dagger.
- Added `stage5-storage-store-dagger-selected.png`, `stage5-storage-store-dagger-result.png`, `stage5-storage-store-dagger-feedback.png`, and `storageStoreFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (54 screenshots)
- visual inspection of Store Item screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R206` complete.
- Whole-project 1:1 estimate advanced to 73.8%.
- R207 reopened at global queue-selection stage.

## 2026-04-26-R205

Scope:

- Added inventory Sell Item no-service smoke coverage.
- Verified confirming Dagger sell without an active sell service preserves Dagger and gold.
- Added `stage5-inventory-sell-dagger-panel.png`, `stage5-inventory-sell-dagger-no-service.png`, and `inventorySellFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (51 screenshots)
- visual inspection of Sell Item screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R205` complete.
- Whole-project 1:1 estimate advanced to 73.6%.
- R206 reopened at global queue-selection stage.

## 2026-04-26-R204

Scope:

- Added belt mouse-use smoke coverage.
- Verified clicking Red Potion directly in the belt drops the belt stack from 5 to 4.
- Kept the existing hotkey `1` path verifying the same stack then drops from 4 to 3.
- Added `stage5-belt-mouse-use-red-potion.png` and `beltMouseUseFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (49 screenshots)
- visual inspection of belt mouse-use screenshot
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R204` complete.
- Whole-project 1:1 estimate advanced to 73.4%.
- R205 reopened at global queue-selection stage.

## 2026-04-26-R203

Scope:

- Fixed Character equipment removal frontend command wiring.
- Changed `RemoveItem` to target `inventory` with the first free bag1 slot instead of invalid `equipment` grid / occupied slot 0.
- Added Character Dagger remove smoke coverage.
- Verified Dagger leaves the weapon equipment slot and returns to bag1 slot 4.
- Added `stage5-character-remove-dagger.png` and `characterRemoveFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (48 screenshots)
- visual inspection of Character remove screenshot
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R203` complete.
- Whole-project 1:1 estimate advanced to 73.2%.
- R204 reopened at global queue-selection stage.

## 2026-04-26-R202

Scope:

- Added inventory Delete Item smoke coverage.
- Verified Blue Potion quantity drops from 3 to 2 after confirming item drop.
- Verified a `Blue Potion` ground-drop label appears.
- Added `stage5-inventory-drop-blue-potion-panel.png`, `stage5-inventory-drop-blue-potion.png`, and `inventoryDropFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (47 screenshots)
- visual inspection of Delete Item screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R202` complete.
- Whole-project 1:1 estimate advanced to 73.0%.
- R203 reopened at global queue-selection stage.

## 2026-04-26-R201

Scope:

- Added inventory Split Item smoke coverage.
- Verified Red Potion split count 1 follows Crystal-style belt placement for beltable items.
- Verified total Red Potion quantity is preserved across inventory plus belt.
- Added `stage5-inventory-split-red-potion-panel.png`, `stage5-inventory-split-red-potion.png`, and `inventorySplitFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (45 screenshots)
- visual inspection of Split Item screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R201` complete.
- Whole-project 1:1 estimate advanced to 72.8%.
- R202 reopened at global queue-selection stage.

## 2026-04-26-R200

Scope:

- Added inventory item-move smoke coverage.
- Verified Wooden Sword moves from bag1 slot 4 to slot 10 through the Stage 5 UI route.
- Added `stage5-inventory-move-wooden-sword.png` and `inventoryMoveFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (43 screenshots)
- visual inspection of item-move screenshot
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R200` complete.
- Whole-project 1:1 estimate advanced to 72.6%.
- R201 reopened at global queue-selection stage.

## 2026-04-26-R199

Scope:

- Exposed `gold` through `window.__mir2Stage5.state`.
- Added Drop Gold smoke coverage.
- Verified confirming 100 gold lowers gold from 1280 to 1180 and renders a `100 Gold x100` ground label.
- Fixed missing `ui.confirm` fallback text on confirmation buttons.
- Added `stage5-inventory-drop-gold-panel.png`, `stage5-inventory-drop-gold.png`, and `inventoryGoldFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (42 screenshots)
- visual inspection of Drop Gold screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R199` complete.
- Whole-project 1:1 estimate advanced to 72.4%.
- R200 reopened at global queue-selection stage.

## 2026-04-26-R198

Scope:

- Added HUD Skill button smoke coverage for opening Character Spells.
- Added HUD Option button smoke coverage for opening Character Stats II.
- Added `stage5-hud-skill-spells.png`, `stage5-hud-option-stats2.png`, and `hudButtonFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (40 screenshots)
- visual inspection of HUD-button screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R198` complete.
- Whole-project 1:1 estimate advanced to 72.2%.
- R199 reopened at global queue-selection stage.

## 2026-04-26-R197

Scope:

- Exposed `equipmentItems` through `window.__mir2Stage5.state`.
- Added inventory Dagger equip smoke coverage.
- Verified Dagger moves from bag1 into the weapon equipment slot.
- Added `stage5-inventory-equip-dagger.png` and `inventoryEquipFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (38 screenshots)
- visual inspection of inventory-equip screenshot
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R197` complete.
- Whole-project 1:1 estimate advanced to 72.0%.
- R198 reopened at global queue-selection stage.

## 2026-04-26-R196

Scope:

- Added inventory Red Potion use smoke coverage.
- Verified the bag1 Red Potion quantity drops from 5 to 4 after clicking the inventory item.
- Added `stage5-inventory-use-red-potion.png` and `inventoryUseFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (37 screenshots)
- visual inspection of inventory-use screenshot
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R196` complete.
- Whole-project 1:1 estimate advanced to 71.8%.
- R197 reopened at global queue-selection stage.

## 2026-04-26-R195

Scope:

- Exposed `hasExpandedStorage` through `window.__mir2Stage5.state`.
- Added expanded-storage rent smoke coverage from locked storage page 2.
- Verified page 2 becomes unlocked, expanded storage is active, capacity text moves to 2/160, and expiry copy renders.
- Added `stage5-storage-page2-rented.png` and `page2Rented` storageFlow evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (36 screenshots)
- visual inspection of expanded-storage screenshot
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R195` complete.
- Whole-project 1:1 estimate advanced to 71.6%.
- R196 reopened at global queue-selection stage.

## 2026-04-26-R194

Scope:

- Added system menu smoke coverage for menu open and Character, Inventory, and Quest actions.
- Added `stage5-system-menu.png`, `stage5-system-menu-character.png`, `stage5-system-menu-inventory.png`, and `stage5-system-menu-quest.png`.
- Recorded `systemMenuFlow` manifest evidence with action labels, transfer labels, meta text, resulting panel visibility, and active inventory tab.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (35 screenshots)
- visual inspection of system-menu screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R194` complete.
- Whole-project 1:1 estimate advanced to 71.4%.
- R195 reopened at global queue-selection stage.

## 2026-04-26-R193

Scope:

- Added chat-control smoke coverage for Shout filter, All restore, Settings, collapse/restore size, and Report.
- Added `stage5-chat-shout-filter.png`, `stage5-chat-settings.png`, `stage5-chat-collapsed.png`, and `stage5-chat-report.png`.
- Recorded `chatFlow` manifest evidence with frame visibility, collapsed/feed-hidden state, settings/report state, visible line text/classes, and scroll knob top.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (31 screenshots)
- visual inspection of chat-control screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R193` complete.
- Whole-project 1:1 estimate advanced to 71.2%.
- R194 reopened at global queue-selection stage.

## 2026-04-26-R192

Scope:

- Added storage page smoke coverage for page 1, locked page 2, and restored page 1.
- Added `stage5-storage-page2-locked.png` and `stage5-storage-page1-restored.png`.
- Recorded `storageFlow` manifest evidence with active page, locked state, visible storage cards, storage slot count, and locked expanded-storage text.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (27 screenshots)
- visual inspection of storage screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R192` complete.
- Whole-project 1:1 estimate advanced to 71.0%.
- R193 reopened at global queue-selection stage.

## 2026-04-26-R191

Scope:

- Exposed `activeCharacterTab` and `knownSkills` through `window.__mir2Stage5.state`.
- Added character tab smoke coverage for char, stats1, stats2, spells, and restored char.
- Added `stage5-character-stats1.png`, `stage5-character-stats2.png`, `stage5-character-spells.png`, and `stage5-character-char-restored.png`.
- Recorded `characterFlow` manifest evidence with active tab, equipment count, stat value count, spell value count, and known skills.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (25 screenshots)
- visual inspection of character screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R191` complete.
- Whole-project 1:1 estimate advanced to 70.8%.
- R192 reopened at global queue-selection stage.

## 2026-04-26-R190

Scope:

- Exposed `inventoryItems` and `activeInventoryTab` through `window.__mir2Stage5.state`.
- Added inventory tab smoke coverage for bag1, bag2, quest, and restored bag1.
- Added `stage5-inventory-bag2.png`, `stage5-inventory-quest.png`, and `stage5-inventory-bag1-restored.png`.
- Recorded `inventoryFlow` manifest evidence with active tab, visible cards, quest entry count, and item summaries.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (21 screenshots)
- visual inspection of inventory screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R190` complete.
- Whole-project 1:1 estimate advanced to 70.6%.
- R191 reopened at global queue-selection stage.

## 2026-04-26-R189

Scope:

- Exposed `beltItems` through `window.__mir2Stage5.state` for UI smoke verification.
- Added belt hotkey `1` smoke coverage.
- Verified Red Potion in belt slot 1 drops from quantity 5 to 4 after the keypress.
- Added `stage5-belt-hotkey-use.png` and `beltUseFlow` manifest evidence.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (18 screenshots)
- visual inspection of `stage5-belt-hotkey-use.png`
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R189` complete.
- Whole-project 1:1 estimate advanced to 70.4%.
- R190 reopened at global queue-selection stage.

## 2026-04-26-R188

Scope:

- Extended Stage 5 UI smoke with belt horizontal, vertical, rotate-back, and closed states.
- Fixed doubled belt slot-label offsets so labels 1-6 stay inside both horizontal and vertical belt frames.
- Moved the vertical belt clear of the Quest tracker.
- Added `beltFlow` manifest state plus label-in-bounds and no-Quest-overlap assertions.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (17 screenshots)
- visual inspection of belt screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R188` complete.
- Whole-project 1:1 estimate advanced to 70.2%.
- R189 reopened at global queue-selection stage.

## 2026-04-26-R187

Scope:

- Extended Stage 5 UI smoke with minimap collapse, BigMap re-expand, and Mail open interactions.
- Added `stage5-minimap-collapsed.png`, `stage5-minimap-expanded.png`, and `stage5-minimap-mail.png`.
- Wrote `minimapFlow` state to the smoke manifest so the evidence records expanded, collapsed, expanded-after-BigMap, and mail-open states.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (14 screenshots)
- visual inspection of minimap screenshots
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R187` complete.
- Whole-project 1:1 estimate advanced to 70.0%.
- R188 reopened at global queue-selection stage.

## 2026-04-26-R186

Scope:

- Added a compact visible-text overflow assertion to the Stage 5 UI smoke for core quest, HUD, minimap, belt, chat, drop-label, and entity-nameplate text.
- Wrote `compactTextLayout` into `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`.
- Fixed the compact minimap title overflow caught by the new assertion by rendering map title and Safe Zone as stable two-line text.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- `npm --prefix apps/web run smoke:stage5-ui` (11 screenshots; 33 compact text nodes checked; zero overflow)
- visual inspection of `docs/stage5-screenshots/stage5-compact-game.png`
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R186` complete.
- Whole-project 1:1 estimate advanced to 69.8%.
- R187 reopened at global queue-selection stage.

## 2026-04-26-R185

Scope:

- Extended Stage 5 UI smoke from one implicit viewport to named desktop 1024x768 and compact 820x640 viewports.
- Added a compact game screenshot, `docs/stage5-screenshots/stage5-compact-game.png`.
- Wrote compact layout bounds for `.client-stage-frame`, `.game-ui-scene`, `.main-hud-shell`, `.chat-frame`, and `.mini-map-panel` into the smoke manifest.
- Added a compact viewport overflow assertion so the route fails if core UI leaves the viewport.

Validation:

- `node --check apps/web/scripts/smoke-stage5-ui.mjs`
- gateway health on `http://127.0.0.1:7110/health`
- web health on `http://127.0.0.1:3002/`
- `npm --prefix apps/web run smoke:stage5-ui` (11 screenshots)
- visual inspection of `docs/stage5-screenshots/stage5-compact-game.png`
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R185` complete.
- Whole-project 1:1 estimate advanced to 69.6%.
- R186 reopened at global queue-selection stage.

## 2026-04-26-R184

Scope:

- Updated the Crystal-style chat frame so filtered chat starts at the newest lines, follows new messages while at the bottom, preserves user scrollback when scrolled up, and moves the scroll knob with position.
- Added a no-WebGL2 DOM-only runtime fallback so headless UI smoke does not trip Bevy's WebGL surface panic.
- Fixed Crystal map API local fallback: when this Mac lacks Crystal `Map/*.map` files, the route uses the packaged starter map region instead of recursively loading missing map `0`.
- Added macOS Chrome path detection to the Stage 5 UI smoke.

Validation:

- `./node_modules/.bin/tsc --noEmit` in `apps/web`
- `./node_modules/.bin/next build` in `apps/web`
- `npm --prefix apps/web run smoke:crystal-minimap-assets`
- `npm --prefix apps/web run smoke:crystal-map-api`
- `curl http://127.0.0.1:7110/health`
- `npm --prefix apps/web run smoke:stage5-ui` (10 screenshots)
- `npm --prefix apps/web run load:gateway-ws` (64/64 ready, 0 errors)
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Result:

- Round `2026-04-26-R184` complete.
- Whole-project 1:1 estimate advanced to 69.4%.
- R185 reopened at global queue-selection stage.

## 2026-04-26-R183

Scope:

- Moved the remaining runtime interaction quest-hint localization key out of the backend `sim.*` namespace.
- `build_interaction_hints` now emits `custom.interaction.questHint`.
- Kept the Crystal localization importer and generated game-data/web localization bundles in sync.

Validation:

- `rg -n "sim\\." apps/simulation/src/runtime.rs` (no matches)
- `cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1` (22/22)
- `cargo +1.89.0 test --locked -p mir2-simulation world_snapshot_includes_scene_and_state_data -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (664/664)

Result:

- Round `2026-04-26-R183` complete.
- Runtime has no `sim.*` references left.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 664 tests.
- R184 reopened at backend/global queue-selection stage.

## 2026-04-26-R182

Scope:

- Removed the runtime-only idle dialog fallback for NPCs without a script or matching local quest dialog.
- No-script/no-page NPC interaction now preserves any pre-existing packets and otherwise emits no `ObjectChat` or active dialog, matching Crystal `NPCScript.Call` when no page is found.
- Preserved scripted NPC and modeled quest NPC dialog behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation npc_without_script_rejects_without_runtime_idle_dialog -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation npc_interaction -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc -- --test-threads=1 --nocapture` (52/52)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (664/664)

Result:

- Round `2026-04-26-R182` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 664 tests.
- R183 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R181

Scope:

- Routed quest-required drop feedback through Crystal `server.YouFound`.
- Removed runtime-only quest-required drop progress chats: `sim.youSecuredQuestItem`, `sim.questReturnForReward`, and `sim.questProgressWasps`.
- Preserved `GainedItem`, quest inventory gain, and quest stage/current updates.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation crystal_quest_required_drop_routes_to_active_quest_inventory -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation quest_required_drop -- --test-threads=1 --nocapture` (3/3)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (664/664)

Result:

- Round `2026-04-26-R181` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 664 tests.
- Latest full `mir2-gateway` regression remains R180 green with 47 tests.
- R182 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R180

Scope:

- Localized the `StartGame` welcome chat to Crystal's `server.Welcome` text with localized `server.GameName`.
- Changed the modeled welcome packet from runtime-only `sim.welcomeCharacter` System chat to Crystal-shaped `ChatType::Hint`.
- Preserved the existing `StartGame` bootstrap packet order and gateway conversion expectations.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation start_game_emits_bootstrap_sequence -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-gateway start_game_emits_bootstrap_sequence -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (664/664)
- `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (47/47)

Result:

- Round `2026-04-26-R180` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 664 tests.
- Full `mir2-gateway` regression is green with 47 tests.
- R181 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R179

Scope:

- Removed runtime-only normal chat self echo from `ClientPacket::Chat`.
- Chat before `StartGame` now returns no packets, matching Crystal's non-game-stage chat guard for ordinary chat.
- In-game normal chat now emits only `ObjectChat` with `Name: message`; `@ADDSTORAGE` remains as the modeled helper command path.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation chat_ -- --test-threads=1 --nocapture` (43/43)
- `cargo +1.89.0 test --locked -p mir2-gateway chat_ -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (664/664)
- `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1` (47/47)

Result:

- Round `2026-04-26-R179` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 664 tests.
- Full `mir2-gateway` regression is green with 47 tests.
- R180 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R178

Scope:

- Removed runtime-only cast-skill failure chats from high-level casting helper paths.
- Unknown skill, cooldown, unwired skill definition, missing player, insufficient MP, unwired summon spell, and missing dynamic summon template failures no longer emit `sim.skillNotKnown`, `sim.skillCooldown`, `sim.skillLogicNotWired`, `sim.playerNotInWorld`, or `sim.notEnoughMp`.
- Preserved successful buff/heal and summon behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation casting -- --test-threads=1 --nocapture` (9/9)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (663/663)

Result:

- Round `2026-04-26-R178` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 663 tests.
- R179 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R177

Scope:

- Removed the runtime-only `sim.itemNotFoundInBag` fallback from `MoveItem` unsupported-grid/missing-source handling.
- Preserved failed-ack-only behavior for unsupported grids.
- Preserved Crystal `server.ItemMoveErrorReport` for Inventory/Storage missing-source failures.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture` (26/26)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (660/660)

Result:

- Round `2026-04-26-R177` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 660 tests.
- R178 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R176

Scope:

- Removed runtime-only stale active-dialog missing-NPC/no-script chats.
- Active NPC dialog follow-up now dismisses silently when the recorded NPC entity is gone or lacks script metadata.
- Preserved ordinary no-script NPC idle fallback behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation npc_dialog_target_missing_npc_rejects_without_runtime_chat_and_dismisses_dialog -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation npc_dialog_target_npc_without_script_rejects_without_runtime_chat_and_dismisses_dialog -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation npc_interaction -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc -- --test-threads=1 --nocapture` (52/52)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (660/660)

Result:

- Round `2026-04-26-R176` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 660 tests.
- R177 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R175

Scope:

- Removed runtime-only NPC dialog helper no-active/invalid-target/no-pending-input chats.
- High-level dialog target/input helper failures now avoid `sim.npcNoMilestoneScript` and `sim.itemNoActiveUse` for those direct helper surfaces.
- Preserved successful dialog link, input, and service flows.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation npc_dialog_target_without_active_dialog_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation npc_dialog_invalid_target_rejects_without_runtime_chat_and_preserves_dialog -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation npc_input_submit_without_input_rejects_without_runtime_chat_and_preserves_dialog -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation npc_interaction -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc -- --test-threads=1 --nocapture` (52/52)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (658/658)

Result:

- Round `2026-04-26-R175` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 658 tests.
- R176 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R174

Scope:

- Removed runtime-only direct NPC interaction invalid target/direction/range chats.
- High-level `interact(object_id)` missing-target, same-tile/no-direction, and out-of-range failures now avoid generic `sim.*` chat.
- Preserved successful NPC dialog, script, and service flows.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation direct_interact_missing_target_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation direct_interact_same_tile_rejects_without_runtime_chat_or_dialog -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation direct_interact_out_of_range_rejects_without_runtime_chat_or_dialog -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation npc_interaction -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc -- --test-threads=1 --nocapture` (52/52)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (655/655)

Result:

- Round `2026-04-26-R174` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 655 tests.
- R175 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R173

Scope:

- Removed runtime-only direct attack invalid target/state/range chats.
- High-level `attack(object_id)` missing-target, non-monster, dead/hidden/stoned, no-direction, and out-of-range failures now avoid generic `sim.*` chat.
- Preserved turn packets, normal attack packets, hidden reveal, Zuma wake, and delayed hit surfaces.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation direct_attack_missing_target_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation direct_attack_non_monster_target_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation direct_attack_out_of_range_rejects_without_runtime_chat_or_attack_packet -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation direct_attack_dead_monster_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation cannibal_plant_hidden_state_blocks_attack_until_revealed -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation zuma_monster_requires_wake_before_it_can_be_attacked -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation attack -- --test-threads=1 --nocapture` (80/80)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (652/652)

Result:

- Round `2026-04-26-R173` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 652 tests.
- R174 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R172

Scope:

- Removed runtime-only successful NPC interaction chat.
- High-level NPC interaction no longer emits `sim.talkingToNpc`.
- Preserved NPC `ObjectChat`/dialog packet surfaces and Crystal NPC script/service flows.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation npc_interaction -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_dialog -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_service -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc -- --test-threads=1 --nocapture` (52/52)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (648/648)

Result:

- Round `2026-04-26-R172` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 648 tests.
- R173 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R171

Scope:

- Removed runtime-only direct pickup invalid target/distance chats.
- High-level `pick_up(object_id)` missing-object, non-ground-target, and out-of-cell failures now return silently.
- Preserved Crystal owner-blocked/full-bag pickup messages and current-cell packet pickup behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation direct_pickup_missing_drop_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation direct_pickup_non_ground_target_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation direct_pickup_out_of_cell_rejects_without_runtime_chat_or_mutation -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation pickup -- --test-threads=1 --nocapture` (18/18)
- `cargo +1.89.0 test --locked -p mir2-simulation drop -- --test-threads=1 --nocapture` (42/42)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (648/648)

Result:

- Round `2026-04-26-R171` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 648 tests.
- R172 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R170

Scope:

- Removed runtime-only `sim.defeatedMonsterEntityMissing` from missing defeated-monster entity handling.
- Missing internal defeat state now returns silently.
- Preserved normal visible monster death and drop packet behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation missing_defeated_monster_entity_is_silent -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation defeating_visible_monster_emits_death_packets -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation drop -- --test-threads=1 --nocapture` (41/41)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (645/645)

Result:

- Round `2026-04-26-R170` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 645 tests.
- R171 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R169

Scope:

- Removed runtime-only monster death-drop success chats.
- Gold and item drop paths no longer emit `sim.monsterDroppedGoldOnGround` or `sim.monsterDroppedItem`.
- Preserved ground drop creation, quest-drop routing, owner windows, and pickup packet surfaces.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation starter_monster_item_drop_has_no_runtime_success_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation drop -- --test-threads=1 --nocapture` (41/41)
- `cargo +1.89.0 test --locked -p mir2-simulation pickup -- --test-threads=1 --nocapture` (15/15)
- `cargo +1.89.0 test --locked -p mir2-simulation attack -- --test-threads=1 --nocapture` (76/76)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (644/644)

Result:

- Round `2026-04-26-R169` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 644 tests.
- R170 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R168

Scope:

- Removed runtime-only `sim.targetDefeated` from summoned VampireSpider death explosion.
- Kept explosion damage, summon despawn timing, and delayed health packet surfaces intact.
- Left BombSpider/CharmedSnake explosion damage behavior unchanged.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation friendly_vampire_spider_death_explosion_has_no_runtime_defeat_chat_and_hits_nearby_hostile_monster -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation spider -- --test-threads=1 --nocapture` (6/6)
- `cargo +1.89.0 test --locked -p mir2-simulation attack -- --test-threads=1 --nocapture` (76/76)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (643/643)

Result:

- Round `2026-04-26-R168` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 643 tests.
- R169 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R167

Scope:

- Removed runtime-only ordinary combat damage narration from pending hit resolution.
- Player-hit and monster-hit paths keep packet health, struck, and death surfaces without generic chat.
- Trainer-specific DPS and average trainer reporting remain intact.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation player_attack_hit_resolution_has_no_runtime_damage_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation attack -- --test-threads=1 --nocapture` (76/76)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (643/643)

Result:

- Round `2026-04-26-R167` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 643 tests.
- R168 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R166

Scope:

- Removed runtime-only successful cast-skill helper chat.
- Buff/heal and summon success paths now preserve state mutation and spawns without generic `sim.castSkill` narration.
- Kept explicit casting failure messages unchanged.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation casting -- --test-threads=1 --nocapture` (6/6)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (643/643)

Result:

- Round `2026-04-26-R166` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 643 tests.
- R167 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R165

Scope:

- Removed runtime-only pre-start cast-skill helper chat.
- `cast_skill` now emits no packets/chat before `StartGame`.
- Preserved started-world buff, cooldown, MP, and summon casting behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation cast_skill_before_start_game_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation casting -- --test-threads=1 --nocapture` (6/6)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (643/643)

Result:

- Round `2026-04-26-R165` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 643 tests.
- R166 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R164

Scope:

- Removed runtime-only pre-start interaction helper chats from high-level and dialog entrypoints.
- `interact` and `select_npc_dialog_target` now emit no packets/chat before `StartGame`.
- Preserved started-world NPC interaction, dialog target, and service-link behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation interact_before_start_game_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation npc_interaction -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_dialog -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_service -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (642/642)

Result:

- Round `2026-04-26-R164` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 642 tests.
- R165 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R163

Scope:

- Removed runtime-only pre-start harvest helper chats from high-level and packet entrypoints.
- `harvest` and `Harvest` now emit no packets/chat before `StartGame`.
- Preserved started-world harvest packet and corpse-loot behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation harvest_before_start_game_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation harvest -- --test-threads=1 --nocapture` (9/9)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (641/641)

Result:

- Round `2026-04-26-R163` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 641 tests.
- R164 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R162

Scope:

- Removed runtime-only pre-start attack helper chats from high-level and packet entrypoints.
- `attack`, `Attack`, and `RangeAttack` now emit no packets/chat before `StartGame`.
- Preserved started-world attack packet traces and delayed combat health behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation attack_before_start_game_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation attack -- --test-threads=1 --nocapture` (76/76)
- `cargo +1.89.0 test --locked -p mir2-simulation combat_packet_trace_orders_attack_before_delayed_health -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (640/640)

Result:

- Round `2026-04-26-R162` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 640 tests.
- R163 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R161

Scope:

- Removed runtime-only pre-start movement helper chats from high-level and packet entrypoints.
- `move_to`, `Walk`, `Run`, and `Turn` now emit no packets/chat before `StartGame`.
- Preserved started-world movement, wall blocking, run fallback, turn packets, and map-transfer behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation movement_before_start_game_rejects_without_runtime_chat -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation walk -- --test-threads=1 --nocapture` (6/6)
- `cargo +1.89.0 test --locked -p mir2-simulation run_ -- --test-threads=1 --nocapture` (3/3)
- `cargo +1.89.0 test --locked -p mir2-simulation transfer_map -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (639/639)

Result:

- Round `2026-04-26-R161` complete.
- Backend/server parity estimate remains 99.70%.
- Full `mir2-simulation` regression is green with 639 tests.
- R162 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R158

Scope:

- Added localization formatter support for Crystal-style `{index:format}` placeholders.
- Routed trainer average damage chat through Crystal `server.AverageDamageOnTrainer`.
- Kept immediate trainer damage chat unchanged because the generated bundle has no dedicated player-inflicted trainer-damage key.

Validation:

- `cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1` (22/22)
- `cargo +1.89.0 test --locked -p mir2-simulation trainer_is_static_passive_and_does_not_die_from_damage -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R158` complete.
- Backend/server parity estimate advanced to 99.70%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R159 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R157

Scope:

- Localized benediction-oil weapon luck outcome chats.
- Replaced hardcoded `"Curse dwells within your weapon."`, `"Luck dwells within your weapon."`, and `"No effect."` with Crystal `server.WeaponCurse`, `server.WeaponLuck`, and `server.WeaponNoEffect`.
- Updated regressions to assert generated localized text instead of hardcoded English.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation benediction_oil -- --test-threads=1 --nocapture` (4/4)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture` (42/42)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R157` complete.
- Backend/server parity estimate advanced to 99.40%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R158 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R156

Scope:

- Removed the runtime-only expanded-storage helper success chat.
- `@ADDSTORAGE` now emits the modeled `ResizeStorage` packet without hardcoded `"Expanded storage activated."` narration.
- Preserved storage expansion state, expiry persistence, and storage-family behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation addstorage_chat_command -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture` (43/43)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R156` complete.
- Backend/server parity estimate advanced to 99.30%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R157 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R155

Scope:

- Localized the group pickup notice for `ShowGroupPickup` drops.
- Replaced hardcoded `"{player} Picked up: {{item}}"` formatting with Crystal `server.FriendlyPickedUpItem`.
- Updated the regression to assert the generated localized text instead of a hardcoded English fragment.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation pickup_emits_crystal_group_pickup_notice_for_marked_items -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation pickup -- --test-threads=1 --nocapture` (14/14)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R155` complete.
- Backend/server parity estimate advanced to 99.20%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R156 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R154

Scope:

- Removed runtime-only high-level helper no-world item-use/drop chats.
- `use_item(key)` and `drop_item(key)` now emit no packets/chat before `StartGame`.
- Preserved normal post-start use/drop behavior and packet-path failure surfaces.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation drop_item -- --test-threads=1 --nocapture` (10/10)
- `cargo +1.89.0 test --locked -p mir2-simulation consumable_item_restores_hp -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture` (42/42)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R154` complete.
- Backend/server parity estimate advanced to 99.10%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R155 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R153

Scope:

- Removed the runtime-only high-level drop helper missing-item chat.
- `drop_item(key)` now returns no packets, emits no chat, and preserves state when the requested item key is absent.
- Kept the high-level helper aligned with the packet `DropItem` missing-source no-chat/no-mutation surface.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation dropped_inventory_item_can_be_removed_from_bag_and_spawned_on_ground -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation drop_item -- --test-threads=1 --nocapture` (10/10)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R153` complete.
- Backend/server parity estimate advanced to 99.00%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R154 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R152

Scope:

- Localized map-transfer not-in-world rejection.
- Replaced `sim.joinWorldBeforeMoving` on public `transfer_map` before-start failure with Crystal `server.NotFound`.
- Kept internal ordinary/debug transfer missing-player handling aligned on `server.NotFound`.
- Extended transfer-bound regression to cover unstarted ordinary and debug transfer attempts.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation transfer_map_requires_player_on_transfer_bounds -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation transfer_map -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R152` complete.
- Backend/server parity estimate advanced to 98.90%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R153 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R151

Scope:

- Localized missing-template `RequestItemInfo` failure.
- Replaced runtime-only `"Crystal item info ... was not found."` with Crystal `server.NotFound`.
- Extended the existing request-item-info regression to cover the missing item-info branch.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation request_item_info_packet_returns_crystal_item_info -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R151` complete.
- Backend/server parity estimate advanced to 98.80%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R152 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R150

Scope:

- Localized the map-transfer bounds rejection.
- Replaced runtime-only `"You are not standing on this map transfer."` with Crystal `server.CannotPositionMoveOnMap`.
- Preserved the existing no-transfer/no-position-mutation behavior when the player is not on the configured source tile.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation transfer_map_requires_player_on_transfer_bounds -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation transfer_map -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R150` complete.
- Backend/server parity estimate advanced to 98.70%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R151 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R149

Scope:

- Removed remaining runtime-only Stage 5 helper success chats from `event.spawn` and `hero.behaviour`.
- Preserved event monster spawning, conquest event-log mutation, hero recruitment, and hero behaviour state mutation.
- Extended the conquest/event/hero/mining/crafting regression to assert successful helper commands no longer emit generic simulator chat.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` (26/26)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R149` complete.
- Backend/server parity estimate advanced to 98.60%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R150 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R148

Scope:

- Removed the runtime-only debug Crystal map-transfer success chat.
- `crystal:<map>:<x>:<y>` transfer keys now emit the modeled Crystal map/location packet surface without `"Transferred to Crystal map ..."` narration.
- Updated the debug transfer regression to assert `MapInformation` and `UserLocation` remain present while the runtime-only chat is absent.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation debug_crystal_transfer_key_updates_map_information_and_location -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation transfer_map -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R148` complete.
- Backend/server parity estimate advanced to 98.50%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R149 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R147

Scope:

- Removed generic runtime-only Stage 5 helper success chats across group, social, mail, trade, auction, conquest, hero, and profession helpers.
- Preserved the underlying state mutations while leaving Crystal-backed localized failure/success surfaces intact.
- Kept the packet-visible success surface focused on modeled state/packet changes instead of simulator-only helper narration.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` (26/26)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R147` complete.
- Backend/server parity estimate advanced to 98.40%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R148 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R146

Scope:

- Localized Stage 5 event-spawn missing player/position rejections.
- Replaced runtime-only `"Join the world before spawning event monsters."` and `"Player position was not found."` with Crystal `server.NotFound`.
- Extended the existing conquest/event/hero/mining/crafting regression to assert the localized missing-player event-spawn failure before normal started-world event coverage.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R146` complete.
- Backend/server parity estimate advanced to 98.30%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R147 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R145

Scope:

- Localized the unknown map-transfer rejection.
- Replaced runtime-only `"Unknown map transfer: ..."` with Crystal `server.NotFound`.
- Extended the existing transfer-bound regression to assert the localized unknown-transfer message before the normal out-of-bounds transfer rejection.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation transfer_map_requires_player_on_transfer_bounds -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R145` complete.
- Backend/server parity estimate advanced to 98.20%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R146 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R144

Scope:

- Localized the Stage 5 unknown-command rejection.
- Replaced runtime-only `"Unknown Stage 5 command: ..."` with Crystal `server.InvalidPacketReceived`.
- Extended the existing trade/shop/auction error-path regression to assert the localized invalid-packet message for an unknown Stage 5 command.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R144` complete.
- Backend/server parity estimate advanced to 98.10%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R145 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R143

Scope:

- Localized Stage 5 inactive-trade rejections.
- Replaced runtime-only `"No active trade."` in `trade.offerGold`, `trade.offerItem`, and `trade.accept` with Crystal `server.NotFound`.
- Extended the existing trade/shop/auction error-path regression to assert the localized inactive-trade messages and no-mutation state.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R143` complete.
- Backend/server parity estimate advanced to 98.00%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R144 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R142

Scope:

- Localized the Stage 5 auction missing-id rejections.
- Replaced runtime-only `"Auction id is required."` in `auction.buy` and `auction.cancel` with Crystal `server.InvalidPacketReceived`.
- Extended the existing trade/shop/auction error-path regression to assert localized invalid-packet messages for both auction commands.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R142` complete.
- Backend/server parity estimate advanced to 97.90%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R143 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R141

Scope:

- Localized the Stage 5 mail missing-id rejections.
- Replaced runtime-only `"Mail id is required."` in `mail.claim` and `mail.delete` with Crystal `server.InvalidPacketReceived`.
- Extended the existing social/group/guild/mail persistence regression to assert the localized invalid-packet messages while preserving normal mail claim persistence.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_social_group_guild_mail_persist_across_reload -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R141` complete.
- Backend/server parity estimate advanced to 97.80%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R142 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R140

Scope:

- Localized the Stage 5 `trade.offerGold` missing-amount rejection.
- Replaced runtime-only `"Trade gold amount is required."` with Crystal `server.InvalidPacketReceived`.
- Extended the existing trade/shop/auction error-path regression to assert the localized invalid-packet message.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R140` complete.
- Backend/server parity estimate advanced to 97.70%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R141 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R139

Scope:

- Localized the Stage 5 hero-behaviour missing-hero rejection.
- Replaced runtime-only `"No hero has been recruited."` with Crystal `server.NotFound`.
- Extended the existing conquest/event/hero/mining/crafting regression to assert the localized missing-hero failure before recruiting the hero.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R139` complete.
- Backend/server parity estimate advanced to 97.60%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R140 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R138

Scope:

- Localized the Stage 5 event-spawn missing monster template rejection.
- Replaced runtime-only `"Event monster template was not found."` with Crystal `server.NotFound`.
- Extended the existing conquest/event/hero/mining/crafting regression to assert the localized missing-template failure before the valid event spawn.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R138` complete.
- Backend/server parity estimate advanced to 97.50%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R139 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R137

Scope:

- Localized the Stage 5 guild creation success chat.
- Replaced runtime-only `"Guild created: {name}."` in `guild.create` with Crystal `server.SuccessfullyCreatedGuild`.
- Extended the existing social/group/guild/mail persistence regression to assert the localized guild creation chat.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_social_group_guild_mail_persist_across_reload -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R137` complete.
- Backend/server parity estimate advanced to 97.40%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R138 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R136

Scope:

- Localized the Stage 5 craft no-ore rejection.
- Replaced runtime-only `"Not enough ore."` in `craft` with Crystal `server.CraftingAttemptFailed`.
- Extended the existing mining/crafting flow regression to assert the localized failure before mining and prove no crafted item is produced without ore.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_conquest_event_hero_mining_and_crafting_flow -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R136` complete.
- Backend/server parity estimate advanced to 97.30%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R137 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R135

Scope:

- Localized the Stage 5 credit-shop insufficient-credit rejection.
- Replaced runtime-only `"Not enough credit."` in `shop.buyCredit` with Crystal `server.YouDontHaveEnoughCurrency`.
- Extended the existing transactional error-path regression to assert the localized chat while preserving credit, mail, item, and `LoseCredit` no-mutation behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R135` complete.
- Backend/server parity estimate advanced to 97.20%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R136 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R134

Scope:

- Localized Stage 5 missing mail/trade item/auction listing rejection chats.
- Replaced runtime-only missing-entity messages in `mail.claim`, `trade.offerItem`, and `auction.buy` with Crystal `server.NotFound`.
- Added a combined regression proving no gold, mail, trade, auction, or inventory mutation occurs on those missing-entity paths.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` (26/26)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (638/638)

Result:

- Round `2026-04-26-R134` complete.
- Backend/server parity estimate advanced to 97.10%.
- Full `mir2-simulation` regression remains green with 637 tests.
- R135 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R133

Scope:

- Localized Stage 5 socket metadata-missing rejection chat.
- Replaced runtime-only `"Item socket metadata was not found."` with Crystal `server.NotFound`.
- Added a regression for unknown socket metadata preserving equipment state and emitting no `ItemSlotSizeChanged`.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` (16/16)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (636/636)

Result:

- Round `2026-04-26-R133` complete.
- Backend/server parity estimate advanced to 97.00%.
- Full `mir2-simulation` regression remains green with 636 tests.
- R134 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R132

Scope:

- Localized Stage 5 socket/seal missing-equipped-item rejection chats.
- Replaced runtime-only `"Equipped item was not found."` in the socket/seal entry guards with Crystal `server.NotFound`.
- Added regressions for missing weapon equipment in both Stage 5 socket and seal commands.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` (15/15)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (635/635)

Result:

- Round `2026-04-26-R132` complete.
- Backend/server parity estimate advanced to 96.90%.
- Full `mir2-simulation` regression remains green with 635 tests.
- R133 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R131

Scope:

- Localized Stage 5 socket/seal missing-source rejection chats.
- Replaced runtime-only `"Socket source item was not found."` and `"Seal source item was not found."` with Crystal `server.NotFound`.
- Preserved source item lookup semantics, source retention, and no-mutation failure behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` (13/13)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-26-R131` complete.
- Backend/server parity estimate advanced to 96.80%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R132 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R130

Scope:

- Removed runtime-only success chat from ordinary map transfers.
- `apply_map_transfer` now relocates manifest-backed transfer targets with `MapInformation` and `UserLocation` packets only.
- Preserved debug `crystal:MAP:X:Y` helper messaging and existing transfer-bound rejection behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation transfer_map -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-26-R130` complete.
- Backend/server parity estimate advanced to 96.70%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R131 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R129

Scope:

- Localized Stage 5 socket/seal invalid-source rejection chats.
- Replaced runtime-only `"Invalid socket source item."` and `"Invalid seal source item."` with Crystal `server.InvalidCombination`.
- Preserved source item retention, socket/seal no-mutation failure behavior, and existing success paths.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` (13/13)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-26-R129` complete.
- Backend/server parity estimate advanced to 96.60%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R130 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-26-R128

Scope:

- Localized Stage 5 gold-shop purchase chat.
- Replaced runtime-only `"Bought {key}."` in `stage5_shop_buy` with Crystal `server.BoughtItemForGold`.
- Preserved gold debit, item gain, and transactional behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_are_transactional -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` (22/22)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-26-R128` complete.
- Backend/server parity estimate advanced to 96.50%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R129 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R127

Scope:

- Removed runtime-only harvest success chat.
- Successful harvest-drop transfer now emits `GainedItem` packets and `ObjectHarvested` without the extra `"Harvested ..."` system message.
- Removed the now-unused localized harvest item-name accumulation from `HarvestDropTransfer`.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation harvest -- --test-threads=1 --nocapture` (8/8)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-25-R127` complete.
- Backend/server parity estimate advanced to 96.40%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R128 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R126

Scope:

- Localized expanded-storage expiry notice.
- Replaced runtime-only `"Expanded storage expired."` with Crystal `server.ExpandedStorageExpired`.
- Preserved the one-shot notice, `ResizeStorage`, account flag persistence, and 160-slot backing storage behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation expired_expanded_storage_tick_emits_resize_notice_once_and_persists_flag -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture` (43/43)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-25-R126` complete.
- Backend/server parity estimate advanced to 96.30%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R127 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R125

Scope:

- Localized Stage 5 item socket/seal success chats.
- Replaced runtime-only `"Item socket slots increased to {slot_size}."` with Crystal `server.ItemSocketsIncreased`.
- Replaced runtime-only `"Item sealed for {minutes} minutes."` with Crystal `server.ItemSealedFor` using the modeled duration label.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` (13/13)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-25-R125` complete.
- Backend/server parity estimate advanced to 96.20%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R126 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R124

Scope:

- Localized Stage 5 item-seal reseal-delay rejection.
- Replaced runtime-only `"Item cannot be resealed yet."` in `stage5_item_seal` with Crystal `server.ItemCannotBeResealedFor`.
- Reused the existing Crystal-style remaining-time label calculation from the modeled `CombineItem` reseal branch.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_seal_rejects_before_next_seal_date_after_expiry -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` (13/13)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-25-R124` complete.
- Backend/server parity estimate advanced to 96.10%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R125 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R123

Scope:

- Localized Stage 5 credit-shop purchase chat.
- Replaced runtime-only `"Bought {key} for {price} credit. Mail {mail_id} created."` in `stage5_shop_buy_credit` with Crystal `server.BoughtItemForCredit`.
- Preserved `LoseCredit`, credit debit, mailbox delivery, and later mail claim transfer behavior.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_credit_shop_mails_purchase_and_claim_transfers_attachment -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` (22/22)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-25-R123` complete.
- Backend/server parity estimate advanced to 96.00%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R124 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R122

Scope:

- Localized Stage 5 successful trade completion.
- Replaced runtime-only `"Trade completed."` in `stage5_trade_accept` with Crystal `server.TradeSuccessful`.
- Extended the Stage 5 transactional regression to assert exact localized chat while preserving trade completion state and gold deduction.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_are_transactional -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` (22/22)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-25-R122` complete.
- Backend/server parity estimate advanced to 95.90%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R123 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R121

Scope:

- Localized Stage 5 low-gold rejection surfaces.
- Replaced runtime-only `"Not enough gold."` in Stage 5 trade gold offer, trade accept, shop buy, and auction buy paths with Crystal `server.LowGold`.
- Extended the Stage 5 error-path regression to assert exact localized chat for failed trade, shop, and auction low-gold attempts while preserving transactional state.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_trade_shop_and_auction_cancel_error_paths_preserve_gold -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` (22/22)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-25-R121` complete.
- Backend/server parity estimate advanced to 95.80%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R122 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R120

Scope:

- Localized direct ground-drop pickup full-bag rejection.
- Replaced runtime-only `"No free bag slot."` in `pick_up_ground_drop` with Crystal `server.YouCannotCarryAnymore`.
- Preserved current-cell pickup scan behavior: full-bag blocked item drops remain skipped so later pickable drops can still be collected.
- Extended the pickup full-bag regression to assert exact localized chat only for direct object-id pickup.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation pickup -- --test-threads=1 --nocapture` (14/14)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-25-R120` complete.
- Backend/server parity estimate advanced to 95.70%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R121 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R119

Scope:

- Localized the remaining Stage 5 full-bag economy/helper rejection surfaces that were still emitting runtime-only `"No free bag slot."`.
- Updated `stage5_mail_claim`, `stage5_shop_buy`, `stage5_auction_buy`, and `stage5_craft` to return Crystal `server.YouCannotCarryAnymore`.
- Tightened the full-bag Stage 5 regression to assert exact localized chat for shop, mail claim, auction buy, and craft while preserving transactional state.

Validation:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_shop_and_auction_full_bag_preserve_gold_and_items -- --test-threads=1 --nocapture` (1/1)
- `cargo +1.89.0 test --locked -p mir2-simulation stage5_ -- --test-threads=1 --nocapture` (22/22)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Result:

- Round `2026-04-25-R119` complete.
- Backend/server parity estimate advanced to 95.60%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R120 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R118

Goal: localize bounded Stage 5 item socket/seal rejection messages to existing Crystal server text keys.

Coordinator local work:

- Confirmed the generated localization bundle contains `server.ItemMaxSockets` and `server.ItemAlreadySealed`.
- Routed Stage 5 socket max-capacity rejection through `server.ItemMaxSockets`.
- Routed Stage 5 already-sealed rejection through `server.ItemAlreadySealed`.
- Left source-item and reseal-delay branches unchanged because their current strings do not yet have fully modeled Crystal argument surfaces.
- Updated focused Stage 5 item assertions to verify exact localized English strings.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation stage5_item_ -- --test-threads=1 --nocapture` (13/13)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Outcome:

- Round `2026-04-25-R118` complete.
- Backend/server parity estimate advanced to 95.50%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R119 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R117

Goal: localize harvest no-drop and full-bag messages to Crystal server text keys.

Coordinator local work:

- Confirmed the generated localization bundle contains `server.NothingWasFound` and `server.YouCannotCarryAnymore`.
- Routed no-drop harvest branches through `server.NothingWasFound`.
- Routed pending-drop full-bag retry through `server.YouCannotCarryAnymore`.
- Preserved pending-drop retry semantics, success transfer behavior, and `ObjectHarvested` timing.
- Updated focused harvest assertions to verify the exact localized English strings.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation harvest -- --test-threads=1 --nocapture` (8/8)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Outcome:

- Round `2026-04-25-R117` complete.
- Backend/server parity estimate advanced to 95.40%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R118 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R116

Goal: localize owner-blocked pickup rejection to Crystal's server text key.

Coordinator local work:

- Rejected the initially explored `NoDropPlayer` death-drop candidate for this round because the current runtime does not yet expose a complete player death-drop surface; combat damage still clamps the player above death for normal monster attacks.
- Confirmed the generated localization bundle contains `server.CannotPickupNotOwner`.
- Replaced the hardcoded owner-blocked pickup English string in current-cell pickup with `server.CannotPickupNotOwner`.
- Replaced the same hardcoded string in the direct object-id pickup helper with the same localized key.
- Updated the owner-window pickup regression to assert the exact localized English text.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation pickup -- --test-threads=1 --nocapture` (14/14)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Outcome:

- Round `2026-04-25-R116` complete.
- Backend/server parity estimate advanced to 95.30%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R117 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R115

Goal: remove the remaining runtime-only normal pickup success chat from modeled item/gold pickup.

Coordinator local work:

- Confirmed Crystal `PlayerObject.PickUp` gains current-cell item and gold drops without emitting normal success chat.
- Kept Crystal `ShowGroupPickup` group notices intact for the modeled item pickup branch.
- Removed `sim.pickedUpItem` from normal item pickup success.
- Removed the same generic pickup chat from normal gold pickup success, leaving `GainedGold` as the visible success packet.
- Updated pickup regressions so item and gold pickup success assert no generic chat.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation pickup -- --test-threads=1 --nocapture` (14/14)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Outcome:

- Round `2026-04-25-R115` complete.
- Backend/server parity estimate advanced to 95.20%.
- Full `mir2-simulation` regression remains green with 633 tests.
- R116 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-25-R114

Goal: add Crystal `NoDrug` map-rule rejection for potion `UseItem`.

Coordinator local work:

- Used read-only explorer evidence from `HumanObject.CanUseItem`: `ItemType.Potion` rejects on `CurrentMap.Info.NoDrug` with `ServerTextKeys.YouCannotUsePotionsHere`.
- Added `no_drug` to `MapDropRuleRecord` and a current-map helper.
- Routed dynamic manifest-backed potion eligibility through the `NoDrug` rejection before consumption or mutation.
- Added the same rejection to static starter HP/MP potion use before timed-recovery queueing.
- Added static and dynamic potion regressions that assert failed ack, system chat, item preservation, no immediate HP mutation, and no follow-up `ObjectHealth`.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation no_drug -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (42/42)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (633/633)

Outcome:

- Round `2026-04-25-R114` complete.
- Potion use now honors Crystal `NoDrug` map rules for both static starter and manifest-backed dynamic potion paths.
- Backend/server parity estimate advanced to 95.10%.

## 2026-04-25-R113

Goal: align static starter HP/MP potion use with Crystal normal-potion timed recovery.

Coordinator local work:

- Used read-only explorer evidence from `PlayerObject.UseItem`: normal potion shape `0` queues `PotHealthAmount` / `PotManaAmount`, while shape `1` is the immediate `SunPotion` branch.
- Reused the existing dynamic Crystal normal-potion pending recovery model for static `heal_hp` / `heal_mp` starter items.
- Updated inventory, belt, and legacy helper coverage to assert no immediate HP mutation or `ObjectHealth` packet on `UseItem`; recovery now arrives on the follow-up tick.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation crystal_use_item_packet_consumes_ -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40)
- `cargo +1.89.0 test --locked -p mir2-simulation consumable_item_restores_hp -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631)

Outcome:

- Round `2026-04-25-R113` complete.
- Static starter potions now match the Crystal normal-potion timed recovery surface.
- Backend/server parity estimate advanced to 95.00%.

## 2026-04-25-R112

Goal: remove runtime-only static repair-powder success/failure chat.

Coordinator local work:

- Used a read-only explorer recommendation and local packet-surface audit to target the starter `repair-powder` branch.
- Removed `sim.noEquipmentNeedsRepair` from no-repair failure.
- Removed `sim.repairedEquippedItems` from repair success while preserving item consumption, durability restoration, and `ItemRepaired` packets.
- Updated repair-powder coverage to assert no `ServerPacket::Chat` on both success and failure.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation repair_powder -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40)
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631)

Outcome:

- Round `2026-04-25-R112` complete.
- Static repair powder no longer emits runtime-only generic repair chat.
- Backend/server parity estimate advanced to 94.90%.

## 2026-04-25-R111

Goal: remove runtime-only static town-teleport success chat.

Coordinator local work:

- Compared the static `town-teleport` branch against the dynamic Crystal template town-teleport path and prior `NoTownTeleport` source audit.
- Removed `sim.townTeleportReturnedToSpawn` from static town-teleport success.
- Updated `town_teleport_returns_player_to_spawn` to assert no chat on success.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation town_teleport -- --test-threads=1 --nocapture` (3/3)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631)

Outcome:

- Round `2026-04-25-R111` complete.
- Static town teleport now matches the chat-free success surface used by the dynamic Crystal teleport path.
- Backend/server parity estimate advanced to 94.80%.

## 2026-04-25-R110

Goal: remove runtime-only static `benediction-oil` no-weapon failure chat.

Coordinator local work:

- Cross-checked Crystal `PlayerObject.UseItem` case 3 and `HumanObject.TryLuckWeapon`; invalid/no-weapon luck attempts failed-ack without chat, while valid outcomes emit localized luck/no-effect/curse messages.
- Removed the hardcoded `"No equipped weapon can be blessed."` static failure chat.
- Added `benediction_oil_no_weapon_failure_has_no_runtime_chat_or_consume`.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation benediction_oil -- --test-threads=1 --nocapture` (4/4)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (631/631)

Outcome:

- Round `2026-04-25-R110` complete.
- Invalid static benediction-oil use now preserves the item and fails without runtime-only chat.
- Backend/server parity estimate advanced to 94.70%.

## 2026-04-25-R109

Goal: remove runtime-only split success chat from `SplitItem`.

Coordinator local work:

- Cross-checked Crystal `PlayerObject.SplitItem`; success enqueues `S.SplitItem1` and `S.SplitItem` only.
- Removed the hardcoded `"Item stack split."` system chat from runtime split success.
- Updated inventory and storage split success coverage to assert no extra chat.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation split_item_packet -- --test-threads=1 --nocapture` (7/7)
- `cargo +1.89.0 test --locked -p mir2-simulation storage_split_item_stack_creates_new_storage_slot -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture` (43/43)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (630/630)

Outcome:

- Round `2026-04-25-R109` complete.
- Successful inventory/storage splits now match Crystal's chat-free packet surface.
- Backend/server parity estimate advanced to 94.60%.

## 2026-04-25-R108

Goal: align static repair-oil chat surfaces with Crystal.

Coordinator local work:

- Cross-checked Crystal `PlayerObject.UseItem` scroll shapes `4` and `5`; no-repair failure enqueues the failed `UseItem` ack without chat, while success emits localized weapon repair hints and `ItemRepaired`.
- Changed static `repair-oil` / `war-god-oil` to emit localized `server.WeaponPartiallyRepaired` / `server.WeaponCompletelyRepaired` Hint chat on success.
- Removed the static runtime-only no-repair failure chat and added a no-consume/no-chat regression.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation repair_oil -- --test-threads=1 --nocapture` (3/3)
- `cargo +1.89.0 test --locked -p mir2-simulation repair_and_war_god_oil_emit_item_repaired_for_weapon -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (630/630)

Outcome:

- Round `2026-04-25-R108` complete.
- Static repair oils now match the modeled dynamic Crystal repair-oil chat/failure surface.
- Backend/server parity estimate advanced to 94.50%.

## 2026-04-25-R107

Goal: remove runtime-only success chat from `DropItem`.

Coordinator local work:

- Cross-checked Crystal `PlayerObject.DropItem`; normal success only sets `p.Success = true` and enqueues the drop ack, while `NoThrowItem` remains the explicit chat failure.
- Removed `custom.itemDropped` from the successful `DropItem` packet path.
- Updated normal and split-stack drop tests to assert success ack and ground-object behavior without chat.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture` (10/10)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629)

Outcome:

- Round `2026-04-25-R107` complete.
- Successful `DropItem` now matches Crystal's chat-free success surface.
- Backend/server parity estimate advanced to 94.40%.

## 2026-04-25-R106

Goal: remove runtime-only static HP/MP potion success chat from `UseItem`.

Coordinator local work:

- Cross-checked Crystal `PlayerObject.UseItem`; normal potion shape `0`/`1` paths queue timed restore or change HP/MP without a generic success chat.
- Removed `sim.usedItem` from the static HP/MP consumable success path.
- Updated inventory and belt starter-potion packet tests to assert heal/consume/success-ack without chat.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation crystal_use_item_packet_consumes_inventory_slot -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_use_item_packet_consumes_belt_slot -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629)

Outcome:

- Round `2026-04-25-R106` complete.
- Static HP/MP consumable use now matches the chat-free Crystal success surface.
- Backend/server parity estimate advanced to 94.30%.

## 2026-04-25-R105

Goal: remove runtime-only missing-source chat from `DropItem`.

Coordinator local work:

- Cross-checked Crystal `PlayerObject.DropItem`; missing item, invalid count, and bind failures enqueue the failed `S.DropItem` without chat.
- Removed `sim.itemNotFoundInBag` from the runtime missing-source `DropItem` branch.
- Added `drop_item_packet_missing_inventory_item_rejects_without_runtime_chat`.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet_missing_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture` (10/10)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (629/629)

Outcome:

- Round `2026-04-25-R105` complete.
- Missing-source `DropItem` is now failed-ack/no-chat/no-mutation.
- Backend/server parity estimate advanced to 94.20%.

## 2026-04-25-R104

Goal: align unmodeled hero-inventory `UseItem` with Crystal-shaped failed ack behavior.

Coordinator local work:

- Cross-checked Crystal `MirConnection.UseItem`, `PlayerObject.HeroUseItem`, and `HeroObject.UseItem`; the hero path builds a `UseItem` response with `Grid = HeroInventory` and `Success = false` before validation.
- Changed the runtime's unmodeled `UseItem(grid=HeroInventory)` branch from `Vec::new()` to `ServerPacket::UseItem { success: false, grid: HeroInventory }`.
- Updated `use_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item` to assert the failed ack while preserving no fallback into matching player inventory.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (628/628)

Outcome:

- Round `2026-04-25-R104` complete.
- Hero-inventory `UseItem` no longer drops the packet response in the current unmodeled hero-inventory runtime.
- Backend/server parity estimate advanced to 94.10%.

## 2026-04-25-R103

Goal: remove runtime-only missing-item chat from `UseItem` failure surfaces.

Coordinator local work:

- Removed `sim.itemNotFoundInBag` emission from runtime missing-location and missing-item `UseItem` fallbacks.
- Removed the same chat emission from the `ClientPacket::UseItem` pre-check when the packet unique id cannot be resolved.
- Added `use_item_packet_missing_inventory_item_rejects_without_runtime_chat`.
- Verified missing inventory ids now return only the failed `UseItem` ack without chat or mutation.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_missing_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (40/40)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (628/628)

Outcome:

- Round `2026-04-25-R103` complete.
- Missing-item `UseItem` failures are now failed-ack/no-chat/no-mutation.
- Backend/server parity estimate advanced to 94.00%.

## 2026-04-25-R102

Goal: remove runtime-only fallback chat from unusable inventory `UseItem`.

Coordinator local work:

- Removed the final `sim.itemNoActiveUse` system chat from the unusable item fallback.
- Added `use_item_packet_unusable_inventory_item_rejects_without_runtime_chat`.
- Verified failed ack, no chat, no mutation, and item preservation for an otherwise unusable inventory item.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_unusable_inventory_item_rejects_without_runtime_chat -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (39/39)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (627/627)

Outcome:

- Round `2026-04-25-R102` complete.
- Unusable inventory item use is now failed-ack/no-chat/no-mutation.
- Backend/server parity estimate advanced to 93.75%.

## 2026-04-25-R101

Goal: remove the remaining runtime-only chat from non-inventory equipment `UseItem` failure.

Coordinator local work:

- Removed the literal `"That item cannot be equipped from this grid."` system chat from the non-inventory equipment `UseItem` rejection path.
- Added `use_item_packet_belt_equipment_rejects_without_runtime_chat`.
- Covered a belt-sourced equipment-like item attempt and verified failed ack, no chat, no mutation, and no equipment state change.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_belt_equipment_rejects_without_runtime_chat -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (38/38)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (626/626)

Outcome:

- Round `2026-04-25-R101` complete.
- Non-inventory use-equip failure is now failed-ack/no-chat/no-mutation.
- Backend/server parity estimate advanced to 93.50%.

## 2026-04-25-R100

Goal: remove non-Crystal runtime-only chat from the successful `UseItem` equipment surface.

Coordinator local work:

- Confirmed Crystal's explicit `EquipItem` success path enqueues refresh/ack/state updates without an "equipped" chat message.
- Removed `sim.equippedItem` and `sim.equippedItemAndReturnedPrevious` emission from the modeled successful use-equip branch.
- Removed the now-dead `replaced_existing` mutation result field.
- Strengthened successful use-equip regressions to assert the packet surface is chat-free.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_equipping_need_identify_item_emits_refresh_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (37/37)
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (13/13)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (625/625)

Outcome:

- Round `2026-04-25-R100` complete.
- Successful use-equip no longer leaks runtime-only `sim.equippedItem*` chat packets.
- Backend/server parity estimate advanced to 93.25%.

## 2026-04-25-R99

Goal: lock the positive explicit equip path for dynamic manifest-backed equipment when Crystal requirements are met.

Coordinator local work:

- Added `equip_item_packet_manifest_equipment_allows_when_requirements_are_met`.
- Used dynamic catalog-backed `SpiritRing`, set the active character to level 15, and equipped to the right ring slot.
- Verified success ack, equipment mutation, and source inventory removal.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_equipment_allows_when_requirements_are_met -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (13/13)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (625/625)

Outcome:

- Round `2026-04-25-R99` complete.
- Dynamic manifest-backed explicit equipment use is covered for both unmet and met requirement paths.

## 2026-04-25-R98

Goal: lock dynamic manifest-backed credit-token `UseItem` coverage.

Coordinator local work:

- Added `use_item_packet_dynamic_crystal_credit_token_emits_localized_hint_chat`.
- Covered `CreditToken3` created through the Crystal item catalog rather than the legacy static alias.
- Verified success ack, `GainedCredit`, localized `server.CreditsAddedToAccount` hint chat, account credit mutation, and item consumption.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_credit_token_emits_localized_hint_chat -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (37/37)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (624/624)

Outcome:

- Round `2026-04-25-R98` complete.
- Dynamic catalog-backed credit tokens now have direct packet-level regression coverage.

## 2026-04-25-R97

Goal: lock storage-grid explicit equip requirement rejection coverage for dynamic manifest-backed equipment.

Coordinator local work:

- Added `equip_item_packet_storage_manifest_equipment_rejects_unmet_requirements_silently`.
- Covered `EquipItem(grid=Storage)` with a dynamic `SpiritRing` whose Crystal requirements are unmet.
- Verified the failure remains ack-only, preserves the storage item, and does not equip into the target ring slot.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_storage_manifest_equipment_rejects_unmet_requirements_silently -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (12/12)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (623/623)

Outcome:

- Round `2026-04-25-R97` complete.
- Storage-sourced explicit equipment use is covered for the R96 Crystal requirement rejection surface.

## 2026-04-25-R96

Goal: close explicit `EquipItem` requirement gating for dynamic manifest-backed equipment.

Coordinator local work:

- Shared the Crystal gender/class/required-type requirement helper between `UseItem` and explicit equip checks.
- Added silent `EquipItem` rejection for dynamic `crystal-item-*` equipment that fails Crystal requirements.
- Kept localized rejection messages on `UseItem` only, matching Crystal's `CanUseItem` message surface versus `CanEquipItem` false-return surface.
- Preserved legacy hand-authored fixture alias behavior while gating catalog-backed items.
- Adjusted the amulet compatibility regression to satisfy the item's Crystal class/level requirements before checking right-bracelet compatibility.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_equipment_rejects_unmet_requirements_silently -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (11/11)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_ -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (622/622)

Outcome:

- Round `2026-04-25-R96` complete.
- Dynamic manifest-backed explicit equipment use now honors Crystal requirement failures without mutating state or emitting chat.
- Backend/server parity estimate advanced to 93.00%.

## 2026-04-25-R95

Goal: add explicit regression coverage for Crystal amulet compatibility with the right bracelet slot.

Coordinator local work:

- Added `equip_item_packet_manifest_amulet_can_target_right_bracelet_slot`.
- Confirmed the R93 item-type compatibility path covers `ItemType.Amulet` targeting `EquipmentSlot.BraceletRight`.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_amulet_can_target_right_bracelet_slot -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (10/10)

Outcome:

- Round `2026-04-25-R95` complete.
- Right-bracelet compatibility is now locked for ring, bracelet, and amulet manifest item types.

## 2026-04-25-R94

Goal: run a wider validation pass after R89-R93 item/equipment parity changes.

Coordinator local work:

- Applied `cargo +1.89.0 fmt` for rustfmt-only formatting changes.
- Revalidated adjacent item/storage suites and full simulation.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture` (218/218)
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture` (42/42)
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` (620/620)

Outcome:

- Round `2026-04-25-R94` complete.
- Full backend simulation regression is green at 620/620.

## 2026-04-25-R93

Goal: close explicit right-slot equipment compatibility for manifest-backed rings and bracelets.

Coordinator local work:

- Added Crystal item-type target-slot compatibility for `EquipItem`.
- Allowed manifest-backed rings to target either ring slot.
- Allowed manifest-backed bracelets to target either bracelet slot.
- Preserved default `UseItem` slot mapping.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_manifest_ring_and_bracelet_can_target_right_slots -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture` (9/9)

Outcome:

- Round `2026-04-25-R93` complete.
- Explicit `EquipItem` target-slot compatibility now follows Crystal item-type rules for current manifest ring/bracelet equipment.

## 2026-04-25-R92

Goal: close the bounded Crystal revive-vitals gap for `ResurrectionScroll`.

Coordinator local work:

- Updated successful dead-player `ResurrectionScroll` revive to restore modeled MP as well as full HP.
- Extended the existing revive-and-consume packet test to start with zero MP and assert restored MP after use.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dead_player_resurrection_scroll_revives_and_consumes_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36)

Outcome:

- Round `2026-04-25-R92` complete.
- `ResurrectionScroll` revive now restores modeled MP within the current runtime vitals model.

## 2026-04-25-R91

Goal: close Crystal repair-bind rejection for manifest-backed repair oil item use.

Coordinator local work:

- Added repair-bind checks to `RepairOil` / `WarGodOil` use against the equipped weapon.
- `DontRepair` now blocks both partial and full repair oil paths.
- `NoSRepair` now blocks the full/special `WarGodOil` path.
- Failure paths preserve the oil and leave weapon durability unchanged.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_repair_oils_respect_weapon_repair_binds -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (36/36)

Outcome:

- Round `2026-04-25-R91` complete.
- Manifest-backed repair oils now honor Crystal/rental repair bind flags on the equipped weapon.

## 2026-04-25-R90

Goal: close Crystal `CanUseItem` map-rule rejection for manifest-backed escape/random teleport scrolls.

Coordinator local work:

- Added `no_escape` and `no_random` to configured map rules.
- Wired manifest-backed scroll shape `0` (`DungeonEscape` / `TeleportHome`, excluding `WarGodOil`) to reject on `no_escape` maps with localized `server.CanNotDungeon`.
- Wired manifest-backed scroll shape `2` (`RandomTeleport`) to reject on `no_random` maps with localized `server.CanNotRandom`.
- Preserved item and player position on both blocked-map failure paths.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_dungeon_escape_rejects_on_no_escape_map -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_random_teleport_rejects_on_no_random_map -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (35/35)

Outcome:

- Round `2026-04-25-R90` complete.
- Manifest-backed escape/random teleport item use now honors the bounded configured Crystal map-rule surface.

## 2026-04-25-R89

Goal: remove manual slot setup from manifest-backed Crystal equipment `UseItem` paths.

Coordinator local work:

- Added Crystal item-type to runtime `EquipmentSlot` mapping for Weapon, Armour, Helmet, Necklace, Bracelet, Ring, Amulet, Belt, Boots, Stone, Torch, and Mount.
- Applied the mapping when gaining manifest-backed items and in the Crystal inventory test helper.
- Added a `UseItem` equipment fallback from the item template so existing runtime items without `equip_slot` can still use the manifest slot.
- Existing equipment requirement tests now pass without manually setting `equip_slot`.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_ -- --test-threads=1 --nocapture` (2/2)
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33)

Outcome:

- Round `2026-04-25-R89` complete.
- Current manifest-backed equipment use now depends on imported item type metadata instead of hand-coded slot mutation.

## 2026-04-25-R87

Goal: expand manifest-backed current `UseItem` parity for mount-fed `ItemType.Food`.

Coordinator local work:

- Added manifest-backed mount-feed parity for `ItemType.Food`, including `RawMeat` and `LeanMeat`.
- Enforced equipped-mount and mount-durability preconditions for food feeding.
- Added Crystal-style `ItemRepaired` and `server.MountFed` feedback on successful feed.
- Applied `MaxDura` loss before feeding for shape `0` (`RawMeat`), while shape `1` (`LeanMeat`) remains no-max-loss.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_food_requires_equipped_mount -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_food_feeds_equipped_mount -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-25-R87` complete.
- Current `UseItem` now supports manifest-backed `ItemType.Food` mount feeding for equipped mount only, preserves item/state on missing-mount/full-dura failure, and emits Crystal-style mount-feed packet hints and durability repair.

## 2026-04-25-R88

Goal: implement and verify the normal potion shape `0` pending/timed recovery subset for manifest-backed `UseItem`.

Coordinator local work:

- Added `SimulationResources` pending fields for potion health/mana recovery.
- Implemented shape-0 branch behavior to queue restoration without immediate HP/MP mutation and without hint chat.
- Kept consume + success ack in `UseItem`.
- Updated world advancement to emit `ObjectHealth`/`ObjectMana` packets as `pending_pot_health_amount` / `pending_pot_mana_amount` drain over ticks.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_normal_potion_queues_timed_restore -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture` (33/33)

Outcome:

- Round `2026-04-25-R88` complete.
- Crystal normal-potion shape `0` now uses a modeled pending/timed recovery flow (not full immediate heal parity).

## 2026-04-25-R86

Goal: expand manifest-backed current `UseItem` support for `DungeonEscape`/`TeleportHome` and `RandomTeleport` through scroll-shape `0/2`.

Coordinator local work:

- Implemented manifest-backed `UseItem` scroll-shape `0/2` handling for `DungeonEscape` / `TeleportHome` and `RandomTeleport`.
- Added same-map occupiable destination search and location/map refresh on success.
- Kept no-chat/ack-failure behavior on bounded destination failures while preserving consumed/unchanged inventory state correctly.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_dungeon_escape_teleports_same_map -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_random_teleport_teleports_same_map -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_ -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-25-R86` complete.
- `UseItem` now supports manifest-backed `DungeonEscape` / `TeleportHome` / `RandomTeleport` behavior with success consume + refresh and failure preserve semantics.

## 2026-04-25-R85

Goal: expand bounded `UseItem` `CanUseItem` parity beyond level-only checks.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/HumanObject.cs::CanUseItem` to confirm modeled `RequiredType` stat/level behavior for current equipment-based item use.
- Added modeled `CanUseItem` checks for `MaxAC`, `MaxMAC`, `MaxDC`, `MaxMC`, `MaxSC`, `MinAC`, `MinMAC`, `MinDC`, `MinMC`, `MinSC`, and `MaxLevel`.
- Added focused regressions for modeled low-requirement rejection and modeled high-requirement allow-through behavior.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_rejects_low_max_dc_requirement -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_equipment_allows_modeled_max_mc_requirement -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_crystal_ -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_ -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-25-R85` complete.
- Manifest-backed current-equipment `CanUseItem` now enforces modeled stat and `MaxLevel` gates for required-type checks using existing equipment/buff totals.
- Adjacent suite and targeted full-surface checks remain green through existing `607`-count local run context.

## 2026-04-25-R84

Goal: close the manifest-backed current-data `UseItem` special-case `scroll.shape` parity for `GtInvite` and `GTTeleport`.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` for `UseItem` scroll-shape `26/27` behavior and confirmed Crystal has no active effect branch for these shapes.
- Removed the previous Rust `GTTeleport` behavior that triggered guild-territory teleport and chat failure surface from the item-use path; kept NPC guild-territory teleport helpers in NPC script flows only.
- Added focused regressions for the bounded current-path behavior where a successful `CanUseItem` run for `GtInvite` and `GTTeleport` consumes one item, emits success `UseItem` ack, and does not emit chat, `UserLocation`, or teleport side effects.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_gt_invite_consumes_without_active_effect -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal_gt_teleport_consumes_without_teleporting -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-25-R84` complete.
- Manifest-backed `GtInvite` and `GTTeleport` now match the bounded Crystal item-use surface for scroll shapes `26/27` in `UseItem`: consume-on-success only, `UseItem` success ack only, and no active-effect chat/teleport side effects.

## 2026-04-25-R83

Goal: close the remaining bounded manifest-backed current-data `UseItem` surfaces that were still starter-only after R82.

Coordinator local work:

- Routed `AncientBanga[Green]` and `AncientBanga[Purple]` through the Crystal scroll-shape 8/9 paths.
- Set the corresponding `free_map_shout` / `free_server_shout` runtime flags on successful use.
- Emitted the Crystal hint-chat surface for the granted shout-token behavior.
- Updated credit-token use messaging to the localized `server.CreditsAddedToAccount` hint instead of the previous local-only text.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R83` complete.
- Remaining manifest-backed item-use small surfaces now match the modeled Crystal behavior for AncientBanga shout tokens and credit-token hints.
- Full `mir2-simulation` regression is green at `607 / 607`.
- Backend parity tracker moved to `90.00%`.

## 2026-04-25-R82

Goal: connect current `UseItem` to the safe Crystal `CanUseItem` subset before broadening the manifest-backed item-use surface.

Coordinator local work:

- Added Crystal gender restriction handling for current item use.
- Added Crystal class restriction handling for current item use.
- Enforced `RequiredType == Level` level requirements before successful use mutation.
- Blocked repeated skill-book learning when the character already knows the skill.
- Allowed valid skill-book learning to succeed and consume the book through the current-data `UseItem` path.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_ -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R82` complete.
- Current `UseItem` now applies the bounded Crystal `CanUseItem` subset for gender, class, level, and skill-book repeat-learning behavior.
- Full `mir2-simulation` regression is green at `607 / 607` after R83.

## 2026-04-25-R81

Goal: close the next grouped manifest-backed current-data `UseItem` parity slice instead of leaving `crystal-item-*` consumables on starter-only behavior.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and `Crystal/Server/MirObjects/MapObject.cs` to confirm the current-data `UseItem` routing for potions, duration buffs, town teleports, and repair oils, plus Crystal `BuffStackType.StackDuration` behavior for same-key buffs.
- Added shared helpers in `runtime.rs` for template-driven HP/MP restore, town-teleport packet flow, current-data buff extraction, and stack-duration application, then routed dynamic manifest-backed `crystal-item-*` use through a dedicated template path.
- Extended `BuffState` with stat payloads so current buff totals can be derived from Crystal template stats instead of only the older hardcoded attack/defence fields.
- Covered current-data `SunPotion`-style HP/MP restore, `ImpactDrug` / `Apple` style multi-stat buff consumables, `TownTeleport`, `BenedictionOil`, `RepairOil`, and the current `WarGodOil` full-repair path. Kept a bounded `template.name.ends_with("WarGodOil")` fallback because the generated manifest still reports `shape = 0` for `WarGodOil`.
- Added focused regressions for dynamic current-data HP/MP restore, stack-duration buff behavior, multi-buff consumables, current-data town teleport, and current-data repair oils.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dynamic_crystal -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R81` complete.
- Dynamic manifest-backed current-data `UseItem` now routes Crystal `SunPotion`, duration buffs, `TownTeleport`, `BenedictionOil`, `RepairOil`, and `WarGodOil` through template stats and scroll shapes, including same-key buff duration stacking and the current `WarGodOil` shape-0 fallback.
- Full `mir2-simulation` regression is green at `599 / 599`.
- Backend parity tracker moved from `82.50%` to `85.00%`.

## 2026-04-25-R80

Goal: close the next current equipment/item metadata parity cluster by wiring real Crystal `NeedIdentify` and `SoulBoundId` behavior instead of keeping local stubs.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` to confirm the current equip/use-equip metadata behavior: real `UserItem` payloads preserve `NeedIdentify` / `SoulBoundId`, successful equip identifies the item before the ack-visible refresh, and soul-bound items owned by another character are rejected.
- Extended `ItemState` and `EquipmentState` with real `identified` plus `soul_bound_id` fields and preserved them through current `UserItem` conversion helpers and round-trips.
- Updated current `EquipItem` plus equip-via-`UseItem` handling so identified-on-equip items emit the matching `RefreshItem` and stay identified after mutation.
- Enforced the current soul-bound equip guard so items bound to another character id fail without being equipped.
- Added focused regressions for bag equip identify, storage equip identify, use-equip identify, and soul-bound rejection.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_equipping_need_identify_item_emits_refresh_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R80` complete.
- Current equipment/item metadata now preserves Crystal `NeedIdentify` and `SoulBoundId` through runtime/item payload round-trips, auto-identifies items on equip/use-equip, and rejects equipping items soul-bound to another character.
- Later full-suite revalidation remained green at `599 / 599` after R81.
- Backend parity tracker moved from `80.00%` to `82.50%`.

## 2026-04-25-R79

Goal: close the current `MysteryWater` / cursed current-equipment parity cluster instead of leaving `UnlockCurse` as a stub.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed the exact bounded behavior: `UseItem` potion `shape=2` (`MysteryWater`) unlocks cursed unequip once, consumes only on the first successful use, repeat use hint-chats without consuming or setting the ack success bit, and both `RemoveItem` plus replacement `EquipItem` reject cursed current equipment unless `UnlockCurse` is set.
- Confirmed the adjacent current `EquipItem(grid=Storage)` path also rejects replacing currently equipped items that cannot be stored back into the exact source slot, while successful cursed replacement/removal clears `UnlockCurse` again.
- Added a transient `unlock_curse` session flag to `runtime.rs`, reset it on character/session refresh, implemented Crystal `MysteryWater` ack/chat/consume behavior, enforced cursed current `RemoveItem` / replacement `EquipItem` guards, cleared the unlock after successful cursed replacement/removal, and rejected storage-grid replacements when the replaced equipment carried `DontStore`.
- Added focused regressions for first-use consume/unlock, repeat-use no-consume ack-fail, logout reset, cursed remove clearing the unlock, cursed replacement requiring the unlock, and storage-grid replacement rejection for replaced equipment with `DontStore`.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation mystery_water_unlock_does_not_survive_logout -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R79` complete.
- Current `MysteryWater` plus cursed current-equipment semantics now match Crystal's bounded runtime surface: first use unlocks and consumes, repeat use hint-chats without consuming, cursed current `RemoveItem` and replacement `EquipItem` require the unlock, successful cursed removal/replacement clears it again, and storage-grid replacement rejects replaced equipment that cannot be stored.
- Focused adjacent regressions are green at `10 / 10` use-item tests, `5 / 5` remove-item tests, `5 / 5` equip-item tests, `188 / 188` item tests, and `41 / 41` storage tests.
- Full `mir2-simulation` regression is green at `590 / 590`.
- Backend parity tracker moved from `77.78%` to `80.00%`.

## 2026-04-25-R78

Goal: stop current `RemoveSlotItem` requests from falling through into whole-equipment removal when Crystal only accepts slot-item sources.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `RemoveSlotItem` only accepts `grid=Mount` / `Fishing` / `Socket`, uses `idFrom` only for `Socket` to resolve the parent item, and searches the parent item's `Slots` collection before any destination move.
- Confirmed local runtime still ignored the packet source-grid and `from_unique_id`, delegating directly to whole-equipment `RemoveItem` semantics so `grid=Equipment` misuse or `grid=Socket` requests that only matched the parent equipment id could remove the equipped weapon outright.
- Updated the packet dispatch to keep `from_unique_id`, then bounded current runtime `RemoveSlotItem` to Crystal's source-grid envelope and current modeled capabilities: invalid source grids and unmodeled `Mount` / `Fishing` / `Socket` slot-item requests now stay on the failed ack instead of mutating equipment.
- Added focused regressions for the invalid `Equipment` source-grid surface and the `Socket` parent-equipment-id fallback bug.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R78` complete.
- Current `RemoveSlotItem` no longer falls through invalid source grids or unmodeled slot-item requests into whole-equipment `RemoveItem` semantics; `grid=Equipment` misuse and `grid=Socket` parent-id matches now ack-fail without mutating equipment.
- Focused adjacent regressions are green at `39 / 39` storage tests and `183 / 183` item tests.
- Full `mir2-simulation` regression is green at `584 / 584`.
- Backend parity tracker moved from `77.77%` to `77.78%`.

## 2026-04-25-R77

Goal: match Crystal's current storage-grid `EquipItem` plus exact-slot `RemoveItem` packet semantics.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` to confirm the adjacent current equipment/storage rules: `EquipItem(grid=Storage)` resolves the source from `Account.Storage` through the active storage service, swaps any replaced equipment back into the exact source slot, `RemoveItem(grid)` only accepts `Inventory` / `Storage` / `HeroInventory` as destination grids, and successful/failing current `EquipItem` / `RemoveItem` paths stay on the ack packet without runtime-only chat.
- Confirmed local runtime still resolved packet `EquipItem` only against `inventory_items`, allowed current `RemoveItem(grid=Equipment)` to succeed even though Crystal does not, and quietly fell back into another bag slot when the requested current inventory removal target was occupied.
- Added explicit current equipment/item conversion helpers, routed packet `EquipItem` through the correct Inventory/Storage source collection with exact-source-slot swap behavior, and restricted current `RemoveItem` to Crystal destination grids with exact destination-slot occupancy checks instead of bag fallback.
- Updated the focused equipment/current item regressions to lock the chat-free ack shape, current storage-grid equip success, current storage-grid remove success, and the occupied-target inventory remove failure surface.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_ -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation remove_item_packet_ -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R77` complete.
- Current `EquipItem(grid=Storage)` now resolves the exact storage item through the active `@Storage` service, and current `RemoveItem(grid=Inventory|Storage)` now follows Crystal's exact destination-slot semantics with ack-only packet shape instead of accepting `grid=Equipment` or falling back into another bag slot.
- Focused adjacent regressions are green at `39 / 39` storage tests and `181 / 181` item tests.
- Full `mir2-simulation` regression is green at `582 / 582`.
- Backend parity tracker moved from `77.76%` to `77.77%`.

## 2026-04-25-R76

Goal: match Crystal's expired expanded-storage downgrade semantics across `StartGame` and the first world tick.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` to confirm the split between immediate state presentation and deferred expiry handling: `BuildUserInformation` reports expanded storage active only while `ExpandedStorageExpiryDate > Envir.Now`, and the runtime later clears `Account.HasExpandedStorage`, emits the `ExpandedStorageExpired` system chat, and enqueues `ResizeStorage` on the first process tick after expiry.
- Confirmed local runtime still trusted the stored `has_expanded_storage` flag directly during `StartGame`, so expired accounts could keep reporting expanded storage as active indefinitely and never emit the Crystal expiry notice or persist the account flag back to `false`.
- Added an effective expanded-storage-active helper, routed account-state refresh through it, and introduced a one-shot session flag so expired accounts appear inactive immediately on `StartGame` but still emit the Crystal expiry chat plus `ResizeStorage` on the first world tick.
- Persisted the expired account flag back to `false` after the first-tick notice while preserving the 160-slot backing storage size, and updated the existing expanded-storage tests to use explicit future expiry when they intend the feature to remain active.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation expired_expanded_storage_is_inactive_on_start_game_but_keeps_backing_size -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation expired_expanded_storage_tick_emits_resize_notice_once_and_persists_flag -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R76` complete.
- Expired expanded storage now downgrades to inactive on current `StartGame`, then emits Crystal-style expiry chat plus `ResizeStorage` on the first world tick and persists the account flag back to `false` while preserving the 160-slot backing array.
- Full `mir2-simulation` regression is green at `579 / 579`.
- Backend parity tracker moved from `77.75%` to `77.76%`.

## 2026-04-25-R75

Goal: match Crystal's full backing-length `UserStorage` payload semantics when expanded storage is no longer currently active.

Coordinator local work:

- Re-read `Crystal/Server/MirDatabase/AccountInfo.cs` and `Crystal/Server/MirObjects/PlayerObject.cs` to confirm the split between packet payload size and storage-slot access: `SendStorage()` enqueues `Account.Storage` at its full backing length, while `IsValidStorageIndex()` separately rejects higher-slot actions when `HasExpandedStorage == false`.
- Confirmed local runtime still sized `UserStorage` from `accessible_storage_size`, which truncated storage-open payloads to `80` slots even when the backing storage length remained `160`.
- Added a backing-length storage helper in `runtime.rs`, switched `current_player_storage_packet()` to use it, and kept storage action validation on the existing accessible-capacity helper.
- Added a focused regression that opens current `@Storage` with inactive expanded access, verifies the `UserStorage` payload stays length `160` and includes a high-slot stored item, and then confirms `StoreItem -> slot 80` still ack-fails.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_storage_open_sends_full_backing_storage_even_when_expansion_inactive -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R75` complete.
- Current `@Storage` open now sends Crystal `UserStorage` with the full backing storage length even when expanded storage is inactive, while higher-slot storage actions remain gated by current accessible capacity.
- Full `mir2-simulation` regression is green at `577 / 577`.
- Backend parity tracker moved from `77.74%` to `77.75%`.

## 2026-04-25-R74

Goal: match Crystal `Connection.StorageSent` resend suppression so repeated unchanged storage opens do not keep resending `UserStorage`.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/NPC/NPCScript.cs`, `Crystal/Server/MirObjects/PlayerObject.cs`, and `Crystal/Server/MirNetwork/MirConnection.cs` to confirm the real storage send state machine: `StorageKey` calls `SendStorage()`, `SendStorage()` suppresses repeats via `Connection.StorageSent`, locked opens clear the flag back to `false`, and successful unlocks resend only through that same helper.
- Added a session-level storage-send flag to the runtime, reset it on character/session refresh, and routed both storage-open and unlock follow-ups through a shared Crystal-style send helper instead of unconditionally emitting `UserStorage`.
- Added a focused regression that locks the packet-visible repeated-open surface: the first unchanged `@Storage` open emits `UserStorage` plus `NPCStorage`, while the second unchanged open emits only `NPCStorage`.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `git -C mir2-web3 diff --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R74` complete.
- Repeated unchanged current `@Storage` opens now suppress duplicate `UserStorage` after the first send, matching Crystal `Connection.StorageSent` resend behavior while preserving the locked reopen/unlock resend path.
- Full `mir2-simulation` regression is green at `576 / 576`.
- Backend parity tracker moved from `77.73%` to `77.74%`.

## 2026-04-25-R73

Goal: close the missing Crystal storage follow-up packet surface by sending `UserStorage` after successful storage open/unlock instead of only `NPCStorage` / `StorageUnlockResult`.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/NPC/NPCScript.cs`, `Crystal/Server/MirObjects/PlayerObject.cs`, and `Crystal/Server/MirNetwork/MirConnection.cs` to confirm the exact packet order: `StorageKey` resets unlock state, calls `SendStorage()`, then enqueues `NPCStorage`; successful `UnlockStorage` enqueues `StorageUnlockResult` and then `Player.SendStorage()`.
- Added Crystal `UserStorage` packet support through protocol ids/codecs/trace names, gateway JSON conversion, and the current storage-open/unlock runtime paths.
- Reworked runtime storage-open packet generation so the storage service resets unlock state first, then conditionally emits `UserStorage` before `NPCStorage` only when storage contents are currently available.
- Tightened focused regressions so the visible `@Storage` link path and successful unlock path both lock the Crystal `UserStorage` follow-up behavior.

Verification:

- `cargo +1.89.0 test --locked -p mir2-protocol --test codec`
- `cargo +1.89.0 test --locked -p mir2-gateway`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_service_links_emit_packets_and_close_dialog -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R73` complete.
- Successful current `@Storage` open now emits Crystal `UserStorage` before `NPCStorage` when storage is available, and successful `UnlockStorage` now emits `StorageUnlockResult` followed by `UserStorage`, through protocol/gateway/runtime with focused regressions.
- Full `mir2-simulation` regression is green at `575 / 575`.
- Backend parity tracker moved from `77.72%` to `77.73%`.

## 2026-04-25-R72

Goal: match Crystal `ResetStorageUnlock()` semantics on repeated `@Storage` opens.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/NPC/NPCScript.cs` and confirmed the storage service always resets session unlock state before attempting to send storage contents.
- Confirmed local runtime still preserved `storage_unlocked` across repeated `@Storage` service opens, which kept storage visible after a prior unlock without matching Crystal's reset path.
- Updated the storage service-context recording path so reopening `@Storage` resets the session unlock state before any storage follow-up packet decision.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`
- `git -C mir2-web3 diff --check`

Outcome:

- Round `2026-04-25-R72` complete.
- Reopening Crystal `@Storage` now resets the session unlock state before deciding whether storage contents can be sent, matching `ResetStorageUnlock()`.
- Full `mir2-simulation` regression is green at `575 / 575`.
- Backend parity tracker moved from `77.71%` to `77.72%`.

## 2026-04-25-R71

Goal: enforce Crystal storage password format semantics across set/unlock/remove actions.

Coordinator local work:

- Source-audited the Crystal storage password validation path and confirmed the accepted format is alphanumeric `5..=15` characters.
- Replaced the runtime TODO stub with a shared Crystal password validator reused by set, unlock, and remove flows.
- Added focused regressions for invalid-format set/unlock/remove requests alongside the existing wrong-password branches.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation storage_password -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R71` complete.
- Current storage password set/unlock/remove now enforce Crystal's `^[A-Za-z0-9]{5,15}$` format semantics.
- Full `mir2-simulation` regression is green at `574 / 574`.
- Backend parity tracker moved from `77.70%` to `77.71%`.

## 2026-04-25-R70

Goal: align storage password actions with Crystal's active-service/range gate and last-set-time removal behavior.

Coordinator local work:

- Confirmed the current storage password packet family should require the active in-range storage service context instead of trusting stale state.
- Updated successful password removal to clear the persisted `LastSetTime` back to `0` like Crystal.
- Locked the surrounding storage password behavior with the existing storage-focused regression surface.

Verification:

- Focused storage password and storage regressions
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R70` complete.
- Current storage password actions now require the active in-range Crystal storage service context, and successful password removal clears `LastSetTime` back to `0`.
- Full `mir2-simulation` regression is green at `572 / 572`.
- Backend parity tracker moved from `77.69%` to `77.70%`.

## 2026-04-25-R69

Goal: close the remaining current-data `CombineItem` manifest-slice backlog after the durability-route fix was identified as the next real gap.

Coordinator local work:

- Re-audited current `PlayerObject.CombineItem` manifest-backed shape-3/4 families plus the shape-0 source surface still present in local data.
- Closed the remaining present-data shape-3/4 coverage bite and locked the shape-0 source as Crystal's failed-ack-only surface for the current manifest slice.
- Re-ran focused `CombineItem` coverage plus the full simulation suite to keep the present-data backlog closure explicit.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation combine_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R69` complete.
- Current inventory-grid `CombineItem` current-data coverage now closes the remaining present-data shape-3/4 families and the shape-0 ack-only source surface for the current manifest slice.
- Full `mir2-simulation` regression is green at `571 / 571`.
- Backend parity tracker moved from `77.68%` to `77.69%`.

## 2026-04-25-R68

Goal: close the next real current-data `CombineItem` gap by source-auditing the remaining shape-3/4 gem/orb families and fixing the smallest runtime mismatch instead of only adding coverage.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and `Crystal/Server/MirObjects/HumanObject.cs`, then checked the current generated shape-3/4 gem/orb manifest to confirm the current data families still in play after the earlier `CombineItem` passes.
- Confirmed the current Rust helper was still treating stat `48` / `HPDrainRatePercent` as the applied upgrade stat when no earlier stat family matched, even though current Crystal durability gems/orbs use `Info.Durability` for the actual upgrade path and reserve stat `48` for the max-added-stats cap.
- Reworked the runtime upgrade-stat detector so current-data durability gems/orbs fall through to the Crystal `MaxDura` path instead of adding a fake stat `48`, without disturbing the already-verified shape-3/4 stat families.
- Added focused regressions for successful `DurabilityOrb`, `StormOrb`, and `DisillusionGem` upgrades plus the durability-cap rejection surface, then reran the full `CombineItem` family and the full `mir2-simulation` regression suite.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation combine_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R68` complete.
- Current inventory-grid `CombineItem` no longer misroutes current-data `DurabilityGem` / `DurabilityOrb` stat-48 control metadata into a fake added stat, so durability upgrades now follow Crystal's `MaxDura` branch and focused regressions lock the current-data durability, attack-speed, magic-resist, and durability-cap surfaces.
- Full `mir2-simulation` regression is green at `565 / 565`.
- Backend parity tracker moved from `77.67%` to `77.68%`.

## 2026-04-25-R67

Goal: close the remaining implemented NPC item-service live-object/range gap after R66 by aligning current buy/sell/repair item actions with Crystal's recorded `NPCObjectID` + `Globals.DataRange` gate.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed current `BuyItem`, `SellItem`, and `RepairItem` / `SRepairItem` all re-check the recorded `NPCObjectID` and abort when the corresponding NPC object is gone or outside `Globals.DataRange`.
- Confirmed current Rust `buy_item_impl`, `sell_item_impl`, and `repair_item_impl` still trusted sticky active-service label state after the service-opening dialog, so stale or out-of-range NPC context could keep mutating those item-service flows.
- Added a shared current-NPC service helper in `runtime.rs`, reused it for storage, and wired the buy/sell/repair item-service paths through the same live-NPC/range gate without changing their established packet semantics.
- Added focused regressions for out-of-range `BuyItem`, missing-NPC `SellItem`, missing-NPC `RepairItem`, and out-of-range `SRepairItem`, then re-ran adjacent storage coverage because the shared helper also backs storage service context.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation buy_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation sell_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation repair_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-25-R67` complete.
- Current `BuyItem`, `SellItem`, and `RepairItem` / `SRepairItem` now require the recorded Crystal NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range service context no longer mutates the implemented current NPC buy/sell/repair item surfaces.
- Full `mir2-simulation` regression is green at `561 / 561`.
- Backend parity tracker moved from `77.66%` to `77.67%`.

## 2026-04-24-R66

Goal: close the next real storage-context parity gap after source audit showed the queued current `MergeItem` cross-grid target is not expressible in the current local model.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed current storage-family item handlers (`StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage`) all re-check that the recorded `NPCObjectID` still resolves to a live NPC within `Globals.DataRange` before mutating storage.
- Confirmed the queued current `MergeItem` `Inventory <-> Equipment` amulet-only and `Inventory <-> Fishing` bait-only surfaces are not currently expressible locally because `EquipmentState` has no stack quantity and there is still no fishing slot collection in runtime state.
- Reworked `ActiveNpcServiceState` to retain the opening NPC object id, added a shared live-NPC/range validation helper for storage service context, and applied it across current `StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage`.
- Upgraded the storage test helper so it creates a visible storage NPC without polluting packet-shape assertions, then added focused regressions for the real `@Storage` out-of-range surface plus the missing-NPC surface.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_npc_storage_service_context_rejects_storage_actions_when_player_leaves_data_range -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage_service_context_requires_live_npc_object -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R66` complete.
- Current storage-family item actions now require the recorded Crystal storage NPC object to still exist and remain within `CRYSTAL_DATA_RANGE`, so stale/out-of-range storage service context now ack-fails across `StoreItem`, `TakeBackItem`, `MoveItem(grid=Storage)`, `SplitItem(grid=Storage)`, and any `MergeItem` touching `Storage`.
- The previously queued current `MergeItem` `Inventory <-> Equipment` amulet-only and `Inventory <-> Fishing` bait-only target remains explicitly blocked on missing local equipment/fishing state and is no longer treated as the next bounded runtime bite.
- Full `mir2-simulation` regression is green at `557 / 557`.
- Backend parity tracker moved from `77.65%` to `77.66%`.

## 2026-04-24-R65

Goal: close the remaining bounded `SplitItem` parity gap after R64 by matching Crystal's supported-grid and failed-ack surface.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `SplitItem` only supports `Inventory` and `Storage`, requires active storage-service context for storage splits, and keeps unsupported/invalid/full/locked branches on the failed `SplitItem1` ack with no extra chat.
- Confirmed current Rust `split_item_impl` still allowed `Belt` splits, still allowed storage splits without the active storage service, and still emitted runtime-only chat for zero-count, full-stack, no-free-slot, and locked-storage failures.
- Reworked `split_item_impl` so unsupported grids short-circuit to the failed ack, storage splits require the active Crystal storage service, and the remaining invalid/full/locked branches now stay ack-only.
- Added focused regressions for `Belt` failed-ack behavior, zero-count inventory failure, and inactive-storage-service storage failure, and updated the existing storage-lock regression to the new ack-only surface.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R65` complete.
- Current `SplitItem` now matches Crystal's supported-grid and failed-ack surface: only `Inventory` / `Storage` are live, storage splits require active Crystal storage service, and unsupported/invalid/full/locked failures stay ack-only.
- Full `mir2-simulation` regression is green at `555 / 555`.
- Backend parity tracker moved from `77.64%` to `77.65%`.

## 2026-04-24-R64

Goal: close the next bounded current inventory-array parity gap by aligning `SplitItem(grid=Inventory)` placement with Crystal's single `Info.Inventory` array.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `SplitItem(grid=Inventory)` finds the source by unique id in the single `Info.Inventory` array, prefers eligible potion/scroll/script and amulet belt ranges first, then scans bag slots across the full inventory array instead of staying on the source page.
- Confirmed current Rust `split_item_impl` still searched only the source local container, so `Bag1` splits could fail despite free `Bag2` space, `Bag2` splits could ignore earlier `Bag1` slots, and belt-eligible inventory splits still missed the Crystal belt-first placement rule.
- Reworked only the inventory split path to use the existing Crystal-style empty-slot helper, letting non-beltable splits cross between local `Bag1` / `Bag2` pages and belt-eligible items land in belt slots first.
- Added focused regressions for `Bag1 -> Bag2`, `Bag2 -> Bag1`, and belt-first inventory split placement.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R64` complete.
- Current `SplitItem(grid=Inventory)` now follows Crystal single-array placement across local `Bag1` / `Bag2`, including belt-first placement for belt-eligible items instead of source-container page scoping.
- Full `mir2-simulation` regression is green at `552 / 552`.
- Backend parity tracker moved from `77.63%` to `77.64%`.

## 2026-04-24-R63

Goal: close the next bounded current inventory-array parity gap by routing slot-based `MoveItem` / `StoreItem` / `TakeBackItem` bag paths through Crystal's single `Info.Inventory` indexing instead of local `Bag1` / `Bag2` slot aliases.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `MoveItem`, `StoreItem`, and `TakeBackItem` all address the single `Info.Inventory` array directly, so packet slots `40+` address the second page instead of aliasing same-number `Bag1` slots.
- Confirmed current Rust `move_item_impl`, `store_item_impl`, and `take_back_item_impl` still searched local bag items by matching raw slot numbers across `Bag1` / `Bag2`, which let same-slot cross-page aliases win incorrectly and rejected `Bag2` packet slots outright.
- Added a shared current-inventory index helper for local `Bag1` / `Bag2` items, then reworked those three packet paths to swap, store, and take back through Crystal-style single-array indices.
- Added focused regressions for `MoveItem` into slot `40`, `StoreItem(from=40)`, and `TakeBackItem(to=40)` so `Bag2` items are now reachable on current packet paths without mutating the wrong same-slot `Bag1` item.
- Updated the older `MoveItem` invalid-slot regressions so `80` remains the out-of-range current inventory index after this parity fix.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation crystal_inventory_index_for_bag2 -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R63` complete.
- Slot-based current `MoveItem`, `StoreItem`, and `TakeBackItem` inventory paths now resolve Crystal single-array indices across local `Bag1` / `Bag2`, including `Bag2` swaps and storage transfers on slots `40+`.
- Full `mir2-simulation` regression is green at `549 / 549`.
- Backend parity tracker moved from `77.62%` to `77.63%`.

## 2026-04-24-R62

Goal: close the remaining unsupported current `MergeItem` cross-grid message-shape gap by removing runtime-only chat from the leftover `Storage <-> Belt` requests.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed unsupported `MergeItem` cross-grid combinations fall through to the failed ack with no extra chat.
- Confirmed current Rust `merge_item_impl` still emitted the runtime-only `Cross-grid item merge is not available yet.` chat for `Storage -> Belt` and `Belt -> Storage`.
- Reworked the remaining unsupported cross-grid fallback to stay ack-only and added focused regressions for both directions with active storage service context so the request reaches the real fallback branch.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R62` complete.
- Remaining unsupported `MergeItem` `Storage <-> Belt` cross-grid requests now follow Crystal's ack-only surface without runtime-only chat.
- Full `mir2-simulation` regression is green at `546 / 546`.
- Backend parity tracker moved from `77.61%` to `77.62%`.

## 2026-04-24-R61

Goal: close the next bounded current `MergeItem` unsupported-grid parity bite by aligning `QuestInventory` with Crystal's failed-ack-only behavior.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `PlayerObject.MergeItem` has no `QuestInventory` branch, so same-grid and cross-grid quest requests fall through to the failed ack with no extra chat or mutation.
- Confirmed current Rust `merge_item_impl` still allowed same-grid `QuestInventory` merges and emitted runtime-only cross-grid chat when `QuestInventory` was involved.
- Reworked `merge_item_impl` so any `MergeItem` touching `QuestInventory` now returns only the failed Crystal-shaped ack, and added focused regressions for the same-grid quest merge plus inventory-to-quest cross-grid surface.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R61` complete.
- Current `MergeItem` now rejects `QuestInventory` requests ack-only without extra chat or quest-item mutation.
- Full `mir2-simulation` regression is green at `544 / 544`.
- Backend parity tracker moved from `77.60%` to `77.61%`.

## 2026-04-24-R60

Goal: close the next bounded current `MoveItem` inventory-array parity gap by aligning unsupported grids, slot bounds, and bag-vs-quest selection with Crystal.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `MoveItem` has no `Belt` or `QuestInventory` branch, checks inventory slot bounds against the real bag array, and never consults quest inventory items during ordinary bag moves.
- Confirmed current Rust `move_item_impl` still allowed `Belt` and `QuestInventory` requests, accepted out-of-range current inventory slots, and could move quest items because bag and quest items shared the same local vector.
- Reworked `move_item_impl` so unsupported `Belt` / `QuestInventory` requests now ack-fail, current inventory slots are bounds-checked, and ordinary bag moves only consider bag items; added focused regressions for invalid source/target slots, quest-slot collision, and both unsupported grids.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R60` complete.
- Current `MoveItem` now rejects `Belt` / `QuestInventory` requests ack-only, enforces current inventory slot bounds, and keeps bag moves from mutating quest items.
- Full `mir2-simulation` regression is green at `542 / 542`.
- Backend parity tracker moved from `77.59%` to `77.60%`.

## 2026-04-24-R59

Goal: close the next bounded current `MoveItem` message-shape gap by aligning missing-source Inventory/Storage failures with Crystal's `ItemMoveErrorReport` surface.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed current missing-source `MoveItem` failures on `Inventory` and `Storage` first report `ServerTextKeys.ItemMoveErrorReport`, then enqueue the failed ack.
- Confirmed current Rust `move_item_impl` still emitted the generic `sim.itemNotFoundInBag` chat on those same paths.
- Reworked current missing-source `MoveItem` Inventory/Storage failures to use Crystal's localized `ItemMoveErrorReport` surface and added focused regressions for both branches.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R59` complete.
- Current missing-source `MoveItem` Inventory/Storage failures now use Crystal's `ItemMoveErrorReport` chat surface before the failed ack instead of `sim.itemNotFoundInBag`.
- Full `mir2-simulation` regression is green at `537 / 537`.
- Backend parity tracker moved from `77.58%` to `77.59%`.

## 2026-04-24-R58

Goal: close the next bounded current `MoveItem` success message-shape gap by removing the runtime-only success chat from current successful move paths.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed successful `MoveItem` swaps only enqueue the success ack; Crystal does not emit an extra success chat after the move completes.
- Confirmed current Rust `move_item_impl` still appended the runtime-only `Item slot updated.` chat after successful current moves.
- Reworked current successful `MoveItem` paths to return only the success ack and added focused regressions for both current `Inventory` and gated `Storage` success cases.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R58` complete.
- Current successful `MoveItem` current `Inventory` and `Storage` paths now follow Crystal's ack-only surface and no longer emit runtime-only `Item slot updated.` chat.
- Full `mir2-simulation` regression is green at `535 / 535`.
- Backend parity tracker moved from `77.57%` to `77.58%`.

## 2026-04-24-R57

Goal: close the next bounded current `MoveItem(grid=Storage)` gating gap by requiring the active Crystal storage service before allowing storage-slot reorders.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `MoveItem(grid=Storage)` first requires the active `@Storage` / `NPCStorage` service context before any storage mutation is attempted.
- Confirmed current Rust `move_item_impl` still allowed storage-slot reorders without the active storage service even though `StoreItem`, `TakeBackItem`, and `MergeItem` already used the Crystal storage-service gate.
- Reworked `MoveItem(grid=Storage)` to require the active Crystal storage service and added a focused regression proving inactive-service requests now failed-ack and preserve storage state.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R57` complete.
- Current `MoveItem(grid=Storage)` now requires the active Crystal storage service, and inactive-service requests fail ack-only without mutating storage items.
- Full `mir2-simulation` regression is green at `534 / 534`.
- Backend parity tracker moved from `77.56%` to `77.57%`.

## 2026-04-24-R56

Goal: close the next bounded current `MoveItem` message-shape gap by aligning storage-lock and invalid-slot failures with Crystal's failed-ack-only surface.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed current `MoveItem` storage-lock and invalid-slot branches enqueue only the failed ack; Crystal does not emit extra chat for those failures.
- Confirmed current Rust `move_item_impl` still appended runtime-only `Storage is locked.`, `Invalid target item slot.`, and `Invalid source item slot.` chat on those same branches.
- Reworked current `MoveItem` storage-lock, negative-slot, and invalid storage-slot failures to stay ack-only and added focused regressions covering the negative-source, negative-target, and invalid storage source/target surfaces.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R56` complete.
- Current `MoveItem` storage-lock and invalid-slot failures now follow Crystal's ack-only surface without extra chat.
- Full `mir2-simulation` regression is green at `533 / 533`.
- Backend parity tracker moved from `77.55%` to `77.56%`.

## 2026-04-24-R55

Goal: close the next bounded current `MoveItem` unsupported-grid ack/message-shape gap by aligning `HeroEquipment`, `Equipment`, and `Fishing` with Crystal's failed-ack-only default.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `PlayerObject.MoveItem` only supports `Inventory`, `Storage`, `Trade`, `Refine`, and `HeroInventory`; `HeroEquipment`, `Equipment`, and `Fishing` all fall through to the failed ack with no extra chat or mutation.
- Confirmed current Rust `move_item_impl` still emitted the runtime-only `That item grid cannot be moved yet.` chat for those same unmodeled grids.
- Reworked `move_item_impl` so unmodeled grids now return only the failed Crystal-shaped ack, and added focused regressions for `HeroEquipment`, `Equipment`, and `Fishing`.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R55` complete.
- Current `MoveItem` unsupported-grid parity now also covers `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player/equipment mutation.
- Full `mir2-simulation` regression is green at `529 / 529`.
- Backend parity tracker moved from `77.54%` to `77.55%`.

## 2026-04-24-R54

Goal: close the next bounded current modeled `MergeItem` cross-grid gap after `Inventory <-> Storage` by landing the local belt-equivalent surface confirmed by the Crystal audit.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed literal `MergeItem(grid=Equipment|Fishing)` uses true equipment and fishing-slot arrays, but the local runtime audit showed those paths still need new state modeling because equipped gear has no stack quantity and fishing-rod slot arrays are not represented.
- Confirmed the current runtime does model Crystal belt-priority stackables separately as `belt_items`, making `Inventory <-> Belt` the next bounded local surface that corresponds to Crystal's split bag/belt behavior.
- Reworked `merge_item_impl` so `Inventory -> Belt` and `Belt -> Inventory` now merge matching Crystal belt-eligible stacks, while non-beltable cross-grid belt requests fail ack-only without reviving the old runtime-only cross-grid chat.
- Added focused regressions for both success directions plus the non-beltable `FishBait` failure branch.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R54` complete.
- Current `MergeItem` now supports the next bounded modeled cross-grid surface via `Inventory <-> Belt` stack merges for Crystal belt-eligible items, with ack-only non-beltable failures.
- Full `mir2-simulation` regression is green at `529 / 529`.
- Backend parity tracker moved from `77.53%` to `77.54%`.

## 2026-04-24-R53

Goal: close the next bounded current `MergeItem` modeled cross-grid feature gap by matching Crystal `Inventory <-> Storage` stack-merge semantics behind the active storage-service gate.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `MergeItem` supports cross-grid merges between `Inventory` and `Storage`, while still requiring the active `@Storage` page and storage access checks on either side.
- Confirmed current Rust `merge_item_impl` still rejected all cross-grid merges with the runtime-only `Cross-grid item merge is not available yet.` message and let same-grid storage merges proceed without the storage service gate.
- Reworked `merge_item_impl` so any path touching `Storage` now requires the active Crystal storage service, preserves ack-only inactive/locked failures, and supports current `Inventory -> Storage` plus `Storage -> Inventory` stack merges for matching stackables.
- Updated the existing storage merge regressions to activate the storage service explicitly and added focused regressions for both cross-grid success directions plus the inactive-service ack-only failure.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R53` complete.
- Current `MergeItem` now supports Crystal-style `Inventory <-> Storage` stack merges through the active storage-service gate, with ack-only inactive/locked failures.
- Full `mir2-simulation` regression is green at `523 / 523`.
- Backend parity tracker moved from `77.52%` to `77.53%`.

## 2026-04-24-R52

Goal: close the next bounded current `MergeItem` message-shape gap by removing runtime-only chat from the current Inventory/Storage same-grid failure/success surface.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed current `MergeItem` failures for storage-lock, missing-item, mismatched/full-stack, and related same-grid branches all enqueue only the failed ack, while successful merges do not emit runtime chat.
- Confirmed current Rust `merge_item_impl` still emitted `Storage is locked.`, `sim.itemNotFoundInBag`, `Only matching item stacks can be merged.`, and `Item stacks merged.` on those same current paths.
- Reworked current same-grid `MergeItem` paths to remain ack-only for those failure branches and to return only the success ack on successful merges.
- Added focused regressions for missing-source, mismatched-stack, and full-target failures, and tightened the existing storage lock/success regressions to assert there is no extra chat.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R52` complete.
- Current `MergeItem` same-grid failure/success message shape now follows Crystal's ack-only surface for current Inventory/Storage paths.
- Full `mir2-simulation` regression is green at `520 / 520`.
- Backend parity tracker moved from `77.51%` to `77.52%`.

## 2026-04-24-R51

Goal: close the next bounded current `MergeItem` unsupported-grid failed-ack/message-shape gap by aligning `Trade` and `Refine` with Crystal's ack-only behavior.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `Trade` and `Refine` fall through `PlayerObject.MergeItem` to the failed ack with no extra chat or matching player-bag mutation.
- Confirmed current Rust `merge_item_impl` still emitted runtime-only cross-grid/grid-not-supported chat when those grids were involved.
- Extended the bounded early failed-ack guard so `MergeItem` now returns only the failed ack for `Trade` and `Refine`, matching Crystal's packet/message surface better while those panes remain unmodeled.
- Added focused regressions proving same-grid and inventory-to-grid requests leave matching player potion stacks in place for both grids.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R51` complete.
- Current `MergeItem` unsupported-grid parity now also covers `Trade` and `Refine` ack-only failures without extra chat or player-bag mutation.
- Full `mir2-simulation` regression is green at `517 / 517`.
- Backend parity tracker moved from `77.50%` to `77.51%`.

## 2026-04-24-R50

Goal: close the next bounded current `MergeItem` unsupported-grid failed-ack/message-shape gap after the hero-grid fix, starting with `Equipment` and `Fishing`.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed current `MergeItem` array selection also treats `Equipment` and `Fishing` as real grids, but unavailable/unmodeled cases still collapse to the failed ack with no extra chat.
- Confirmed current Rust `merge_item_impl` still emitted runtime-only cross-grid/grid-not-supported chat for `Equipment` and `Fishing`, even though those current surfaces remain unmodeled.
- Extended the bounded early failed-ack guard so `MergeItem` now returns only the failed ack for `HeroInventory`, `HeroEquipment`, `Equipment`, and `Fishing`.
- Added focused regressions proving inventory-to-equipment and inventory-to-fishing requests leave matching player potion stacks unchanged and emit no extra chat.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R50` complete.
- Current `MergeItem` unsupported-grid parity now also covers `HeroInventory`, `HeroEquipment`, `Equipment`, and `Fishing` ack-only failures without extra chat or player-bag mutation.
- Full `mir2-simulation` regression is green at `513 / 513`.
- Backend parity tracker moved from `77.49%` to `77.50%`.

## 2026-04-24-R49

Goal: close the remaining bounded current `MoveItem` unsupported-grid failed-ack/message-shape gap by aligning `Trade` and `Refine` with the Crystal-style ack-only behavior already locked for `HeroInventory`.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `MoveItem` has explicit `Trade` and `Refine` branches instead of the generic unsupported-grid message path.
- Confirmed current Rust `move_item_impl` still emitted runtime-only chat for those unmodeled grids.
- Extended the bounded early failed-ack guard so `MoveItem` now returns only the failed ack for `Trade` and `Refine`, matching the Crystal message shape better while those panes remain unmodeled.
- Added focused regressions proving matching player bag items stay in place and no extra chat is emitted for both grids.

Verification:

- `cargo +1.89.0 fmt`
- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R49` complete.
- Current `MoveItem` unsupported-grid parity now covers `HeroInventory`, `Trade`, and `Refine` ack-only failures without extra chat or player-bag mutation.
- Full `mir2-simulation` regression is green at `511 / 511`.
- Backend parity tracker moved from `77.49%` to `77.50%`.

## 2026-04-24-R48

Goal: close the next bounded hero-grid current item packet gap by matching Crystal `MoveItem(grid=HeroInventory)` failed-ack behavior while hero inventory remains unmodeled.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `MoveItem` accepts `HeroInventory` only when a current hero exists/spawns and otherwise enqueues only the failed ack.
- Confirmed current Rust `move_item_impl` still treated `HeroInventory` as an unsupported grid and emitted a runtime-only chat message.
- Added a bounded early failed-ack guard for `MoveItem(grid=HeroInventory)`.
- Added a focused regression proving the matching player `bronze-helmet` bag item remains in place and no extra chat is emitted.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation move_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R48` complete.
- Current `MoveItem(grid=HeroInventory)` now failed-ack without extra chat or player-bag mutation while hero inventory is unmodeled.
- Full `mir2-simulation` regression is green at `509 / 509`.
- Backend parity tracker moved from `77.48%` to `77.49%`.

## 2026-04-24-R47

Goal: close the next bounded hero-grid current item packet gap by matching Crystal `MergeItem` failed-ack behavior when hero inventory/equipment remain unmodeled.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `MergeItem` selects its source/target arrays by grid and, for `HeroInventory` / `HeroEquipment`, enqueues only the failed ack when no hero is present/spawned.
- Confirmed current Rust `merge_item_impl` still emitted runtime-only system chat for unsupported or cross-grid hero requests, diverging from Crystal even though player bag state stayed unchanged.
- Added a bounded early failed-ack guard for any `MergeItem` request touching `HeroInventory` or `HeroEquipment`.
- Added focused regressions proving both hero-to-hero and inventory-to-hero merge requests produce only the failed ack and preserve matching player potion stacks.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation merge_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R47` complete.
- Current `MergeItem` hero-grid requests now failed-ack without extra chat or player-bag mutation while hero inventory/equipment are unmodeled.
- Full `mir2-simulation` regression is green at `508 / 508`.
- Backend parity tracker moved from `77.47%` to `77.48%`.

## 2026-04-24-R46

Goal: close the next bounded hero-grid current item packet guards by matching Crystal failed-ack behavior for `EquipItem`, `RemoveItem`, and `RemoveSlotItem` while hero inventory/equipment remain unmodeled.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `EquipItem`, `RemoveItem`, and `RemoveSlotItem` route hero-grid requests through current hero inventory/equipment only and, when no hero is present/spawned, simply enqueue the failed ack without touching player bag/equipment.
- Confirmed current Rust `EquipItem`, `RemoveItem`, and `RemoveSlotItem` paths still reused player inventory/equipment helpers for those hero-grid requests, so matching player items could be equipped or removed accidentally.
- Added bounded early failed-ack guards for `EquipItem(grid=HeroInventory)`, `RemoveItem(grid=HeroInventory)`, and `RemoveSlotItem(grid|grid_to=HeroEquipment|HeroInventory)`.
- Added focused regressions proving the matching player helmet/weapon state remains unchanged under those hero-grid requests.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation remove_item_packet_hero_inventory_grid_does_not_mutate_matching_player_equipment -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item_packet_hero_equipment_grid_does_not_mutate_matching_player_equipment -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation equip_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation remove_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation remove_slot_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R46` complete.
- Current `EquipItem(grid=HeroInventory)`, `RemoveItem(grid=HeroInventory)`, and `RemoveSlotItem(grid=HeroEquipment|HeroInventory)` now failed-ack without mutating matching player inventory/equipment.
- Full `mir2-simulation` regression is green at `506 / 506`.
- Backend parity tracker moved from `77.46%` to `77.47%`.

## 2026-04-24-R45

Goal: close the next bounded hero-inventory current item packet gap by matching Crystal `SplitItem(grid=HeroInventory)` failed-ack behavior without falling back into player inventory.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `SplitItem` only supports `Inventory` and `Storage`; `HeroInventory` falls into the default failed `SplitItem1` ack path.
- Confirmed current Rust `split_item_impl` still treated unsupported grids as player inventory, so `HeroInventory` could split a matching player stack.
- Added a bounded early `HeroInventory` failed-ack guard in `split_item_impl`.
- Added a focused regression proving `SplitItem(grid=HeroInventory)` leaves the matching player red-potion stack unchanged and emits only the failed `SplitItem1` ack.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation split_item_packet_hero_inventory_grid_does_not_mutate_matching_player_stack -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation split_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R45` complete.
- Current `SplitItem(grid=HeroInventory)` now failed-acks without mutating matching player inventory.
- Full `mir2-simulation` regression is green at `503 / 503`.
- Backend parity tracker moved from `77.45%` to `77.46%`.

## 2026-04-24-R44

Goal: close the matching hero-inventory current item packet gap by preventing `UseItem(grid=HeroInventory)` from falling back into player inventory.

Coordinator local work:

- Re-read `Crystal/Server/MirNetwork/MirConnection.cs`, `PlayerObject.HeroUseItem`, and `HeroObject.UseItem` and confirmed hero-grid `UseItem` dispatch never routes through player bag lookup.
- Confirmed current Rust `ClientPacket::UseItem` still resolved non-belt grids against player inventory, so `HeroInventory` could consume matching player bag items.
- Added a bounded `HeroInventory` short-circuit in the packet dispatch path so current runtime no longer falls back into player inventory while hero inventory remains unmodeled.
- Added a focused regression proving `UseItem(grid=HeroInventory)` leaves the matching player red-potion stack unchanged.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_hero_inventory_grid_does_not_mutate_matching_player_item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R44` complete.
- Current `UseItem(grid=HeroInventory)` no longer falls back into player bag items when hero inventory is unmodeled.
- Full `mir2-simulation` regression is green at `502 / 502` before R45 extended the suite.
- Backend parity tracker moved from `77.44%` to `77.45%`.

## 2026-04-24-R43

Goal: close the next bounded current `UseItem` map-rule gap by matching Crystal `ResurrectionScroll` rejection on maps flagged `NoReincarnation`.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and `HumanObject.cs` and confirmed `ResurrectionScroll` first rejects `CurrentMap.Info.NoReincarnation` with `CannotUseOnMap`, while alive users still fail earlier with `CannotResurrection`.
- Added a bounded `no_reincarnation` field to the existing current map-rule override record.
- Wired the dead-player `ResurrectionScroll` path through the new map-rule helper so blocked maps now preserve the item, suppress revive packets, and emit the localized system message.
- Added a focused regression proving a dead player on a `NoReincarnation` map receives the failed ack plus `CannotUseOnMap`, keeps the scroll, and remains dead.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet_dead_player_resurrection_scroll_rejects_on_no_reincarnation_map -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation town_teleport -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R43` complete.
- Current `ResurrectionScroll` now respects map `NoReincarnation` for dead players.
- Full `mir2-simulation` regression is green at `501 / 501` before later hero-grid guards landed.
- Backend parity tracker moved from `77.43%` to `77.44%`.

## 2026-04-24-R42

Goal: close the next bounded current `UseItem` map-rule gap by matching Crystal `TownTeleport` rejection on maps flagged `NoTownTeleport`.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/HumanObject.cs` and confirmed `CanUseItem` blocks Town Teleports on `CurrentMap.Info.NoTownTeleport` with the localized system message before any mutation.
- Extended the bounded map-rule override record so current runtime can express `NoTownTeleport`.
- Wired `town-teleport` use through the new map gate so blocked maps emit `server.NoTownTeleport`, preserve the item, and suppress `UserLocation`.
- Added a focused regression proving the current player position and inventory remain unchanged on a blocked map.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation town_teleport -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R42` complete.
- Current `TownTeleport` now respects map `NoTownTeleport`.
- Full `mir2-simulation` regression is green at `500 / 500`.
- Backend parity tracker moved from `77.42%` to `77.43%`.

## 2026-04-24-R41

Goal: align current `UseItem` dead-state behavior, including alive/dead `ResurrectionScroll`, without reopening broader map-rule or hero-inventory work.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and `HumanObject.cs` and confirmed ordinary items fail while dead, `ResurrectionScroll` only works while dead, and alive use emits `CannotResurrection`.
- Added the dead-player short-circuit for ordinary current `UseItem` actions.
- Wired `ResurrectionScroll` so alive use failed-hints without consumption, while dead use revives the current player and consumes the item.
- Added focused regressions for the ordinary-item dead gate, alive `ResurrectionScroll`, and dead `ResurrectionScroll` revive path.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation use_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R41` complete.
- Current `UseItem` now matches the bounded Crystal dead-state / `ResurrectionScroll` behavior.
- Full `mir2-simulation` regression is green at `499 / 499`.
- Backend parity tracker moved from `77.41%` to `77.42%`.

## 2026-04-24-R40

Goal: align the next bounded dead-state current item mutation family without reopening larger death-drop or hero-system work.

Coordinator local work:

- Re-read the relevant Crystal current dead-player item/service branches and confirmed `BuyItem`, `DeleteItem`, `SellItem`, `RepairItem`, `DropItem`, and `CombineItem` all short-circuit before mutation while dead.
- Added bounded dead-state guards across those current runtime paths so they now acknowledge/fail without mutating inventory, gold, durability, or ground state.
- Added focused regressions covering the dead-player `BuyItem`, `DeleteItem`, `SellItem`, `RepairItem`, `DropItem`, and `CombineItem` branches.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation dead_player -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R40` complete.
- Current dead-state item mutation family now short-circuits without mutation across the bounded packet set.
- Full `mir2-simulation` regression is green at `496 / 496`.
- Backend parity tracker moved from `77.40%` to `77.41%`.

## 2026-04-24-R39

Goal: promote current Crystal map drop flags from runtime/config-only overrides into generated respawn/map data without reopening the now-verified R37/R38 runtime behavior.

Coordinator local work:

- Audited `Crystal/Server/MirDatabase/MapInfo.cs` and confirmed the saved boolean order after movement data includes `NoThrowItem`, `NoDropPlayer`, and `NoDropMonster`.
- Confirmed `packages/tooling/scripts/generate-crystal-respawn-manifest.mjs` was already reading those map booleans in order but discarding them, so the next data-backed step is a bounded generator/manifest update rather than a new parser.
- Added forward-compatible `mir2-game-data` map fields plus generator plumbing so manifest-backed map flags can flow into runtime once the Crystal DB is regenerated.
- Attempted to regenerate `packages/game-data/data/generated/crystal_respawn_manifest.json`, but this Mac did not have the expected local asset path `Crystal/Build/Server/Debug/Server.MirDB` or matching `Envir/Routes`, so the manifest-backed import remains blocked and is not counted as a verified parity move yet.

Blocked state:

- `node packages/tooling/scripts/generate-crystal-respawn-manifest.mjs` failed with `ENOENT` for `/Users/henryliu/obelisk/ai/numeron/mir2/Crystal/Build/Server/Debug/Server.MirDB`.
- Keep the runtime/game-data/tooling scaffolding, but do not mark the map-data import complete until the Crystal build assets are available and the manifest is regenerated and reverified.

## 2026-04-24-R38

Goal: close the next bounded map-drop-rule parity gap after R37 by matching Crystal `CurrentMap.Info.NoDropMonster` suppression across normal monster drops and harvest loot.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/MonsterObject.cs` and confirmed both `Drop()` and `DropItem(UserItem item)` early-return when `CurrentMap.Info.NoDropMonster` is set.
- Re-read the current Rust defeat/harvest paths and confirmed normal monster drops, current field-wasp quest drop, and harvest-corpse pending loot still ignored the map rule.
- Added a shared current-map `NoDropMonster` helper and wired it into the current monster-death drop path, the current field-wasp quest-drop special case, and harvest pending-drop preparation so blocked maps now suppress all three loot surfaces.
- Added a focused regression proving a no-drop map suppresses the deterministic field-wasp quest drop while leaving the quest in progress.
- Added a second focused regression proving a harvest corpse on a no-drop map ends with `Nothing was found.` and `ObjectHarvested` instead of transferring loot.
- Kept the runtime change intentionally bounded: current player/hero death-drop `NoDropPlayer` parity remains open because the Rust runtime still lacks a full Crystal death-drop surface.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation no_drop_monster_map_rule -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation harvest -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R38` complete.
- Current monster-drop behavior now respects Crystal map `NoDropMonster` for normal defeat drops, field-wasp quest drop, and harvest-corpse loot.
- Full `mir2-simulation` regression is green at `490 / 490`.
- Backend parity tracker moved from `77.39%` to `77.40%`.

## 2026-04-24-R37

Goal: close the next smallest current `DropItem` gap by matching Crystal `CurrentMap.Info.NoThrowItem` rejection and localized `CanNotDrop` message behavior.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `DropItem(ulong id, ushort count, bool isHeroItem)` rejects `CurrentMap.Info.NoThrowItem` before inventory lookup or hero-inventory routing, emits `CanNotDrop` system chat, then enqueues the failed `DropItem` ack.
- Added a bounded current-map drop-rule container in simulation config so the runtime can express map-level item-drop restrictions without reopening protocol `MapInformation` or a broader map import round.
- Wired `drop_item_packet` to emit the localized `server.CanNotDrop` system chat before the failed `DropItem` ack and to preserve inventory plus ground state on blocked maps.
- Added a focused regression that proves a blocked map returns the Crystal-shaped chat-plus-failed-ack sequence, preserves inventory counts, and spawns no ground drop.
- Kept the round intentionally bounded: broader map-drop rules remained queued for the next bite after `NoThrowItem`.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-24-R37` complete.
- Current `DropItem` now respects Crystal map `NoThrowItem`, emits `CanNotDrop` before the failed ack, and preserves inventory plus ground state.
- Full `mir2-simulation` regression is green at `488 / 488`.
- Backend parity tracker moved from `77.38%` to `77.39%`.

## 2026-04-23-R36

Goal: align Crystal current `DropItem` rental `DontDrop` rejection without taking on broader map-flag or hero-inventory work.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `PlayerObject.DropItem` rejects both base `Info.Bind.HasFlag(BindMode.DontDrop)` and rental `RentalInformation.BindingFlags.HasFlag(BindMode.DontDrop)` before any mutation.
- Confirmed current Rust `drop_item_packet` already carried rental binding flags in `ItemState`, but only rejected base Crystal `DontDrop`, so rental `DontDrop` still slipped through.
- Reused the existing shared runtime helper that checks Crystal-or-rental bind flags instead of duplicating another branch in `drop_item_packet`.
- Added a focused regression that proves a current inventory item with rental `BindingFlags.DontDrop` returns the Crystal-shaped failed `DropItem` ack, preserves the item in inventory, preserves rental metadata, and spawns no ground drop.
- Kept the round intentionally bounded: Crystal `CurrentMap.Info.NoThrowItem` message parity and broader map flag import remain queued as the next smallest current `DropItem` surface.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation drop_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R36` complete.
- Current `DropItem` now rejects rental `BindingFlags.DontDrop` ack-only like Crystal, preserving inventory state and rental metadata.
- Full `mir2-simulation` regression is green at `487 / 487`.
- Backend parity tracker moved from `77.37%` to `77.38%`.

## 2026-04-23-R35

Goal: lock the next smallest Crystal hero-inventory packet guards for current `DropItem` and `CombineItem` while hero inventory remains unmodeled.

Coordinator local work:

- Re-read `Crystal/Server/MirObjects/PlayerObject.cs` and confirmed `DropItem(ulong id, ushort count, bool isHeroItem)` searches hero inventory only when `isHeroItem=true`, and if no current hero exists it simply enqueues the failed ack without touching player inventory.
- Confirmed `CombineItem(MirGridType grid, ulong fromID, ulong toID)` only switches to `CurrentHero.Inventory` for `MirGridType.HeroInventory` when `HasHero && HeroSpawned`; otherwise it returns the failed ack without mutating player inventory.
- Verified current Rust runtime already matched that bounded behavior because `drop_item_packet` early-returned on `hero_inventory=true` and `combine_item_impl` already rejected non-`Inventory` grids.
- Added a focused `DropItem` regression that proves `hero_inventory=true` does not mutate a matching player bag item or spawn a ground drop when hero inventory is unavailable.
- Added a focused `CombineItem` regression that proves `grid=MirGridType::HeroInventory` does not consume or mutate matching player inventory items when hero inventory is unavailable.
- Kept the round intentionally bounded: full hero inventory modeling remains open, and the next smallest current item gap moved to rental `DontDrop`.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation hero_inventory -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R35` complete.
- Current bounded hero-inventory packet guards are now regression-locked for `DropItem(hero_inventory=true)` and `CombineItem(grid=HeroInventory)`: with no modeled/available hero inventory, both ack-fail without mutating matching player inventory.
- Full `mir2-simulation` regression is green at `486 / 486`.
- Backend parity tracker moved from `77.36%` to `77.37%`.

## 2026-04-23-R34

Goal: align the bounded Crystal `DeleteItem` hero-flag edge without taking on full hero inventory support.

Coordinator local work:

- Re-read Crystal server dispatch and confirmed `MirConnection.DeleteItem` drops the packet `HeroInventory` flag entirely before calling `PlayerObject.DeleteItem`.
- Confirmed `PlayerObject.DeleteItem` searches only `Info.Inventory` by `UserItem.UniqueID`, so matching player bag items are still deleted even when the client set `HeroInventory=true`, while missing hero/player ids remain ack-only.
- Removed the temporary runtime short-circuit that treated `hero_inventory=true` as "do not touch player inventory" because that diverged from Crystal for matching player bag ids.
- Replaced the incorrect hero-flag regression with one that proves a matching player bag item is still deleted under `hero_inventory=true`.
- Added a second focused regression that proves missing hero/player ids still return the Crystal-shaped `DeleteItem` ack without mutating current player inventory.
- Kept the round intentionally bounded: full hero-inventory item handling is still open, and current `DropItem` / `CombineItem` hero-inventory paths remain queued as the next smallest guard surface.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation delete_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R34` complete.
- Current `DeleteItem` now matches Crystal's server-side `HeroInventory` quirk: the flag is ignored and deletion still targets only current player inventory by unique id.
- Missing hero/player ids remain ack-only with no player inventory mutation.
- Full `mir2-simulation` regression is green at `484 / 484`.
- Backend parity tracker moved from `77.35%` to `77.36%`.

## 2026-04-23-R33

Goal: close the current item packet unique-id gap that still let packet `UseItem`, packet `EquipItem`, and `MergeItem` fall back to duplicate keys or raw slot aliases.

Coordinator local work:

- Confirmed from Crystal packet/runtime behavior that current item packet actions use `UserItem.UniqueID` as the item identity, not "first matching key" lookup or bag slot aliasing.
- Added a shared current item client-reference index helper so bag-page-aware unique-id resolution can be reused across packet paths without duplicating container logic.
- Updated current packet `UseItem` lookup to resolve the exact referenced current item by unique id, so duplicate-key consumables on different bag pages no longer consume the wrong stack.
- Updated current packet `EquipItem` lookup to resolve the exact referenced current item by unique id before mutating equipment, so duplicate-key equippables no longer equip the first matching item.
- Updated current `MergeItem` lookup to resolve both source and target items by unique id instead of treating packet ids as raw slot numbers, closing the remaining `Bag1` / `Bag2` merge aliasing gap left after R32.
- Added focused regressions for duplicate-key packet `UseItem`, duplicate-key packet `EquipItem`, and inventory `MergeItem` with `Bag2` unique ids.
- Kept the round intentionally bounded to current item packet unique-id cleanup; hero inventory and remaining combine-family gaps stay queued.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R33` complete.
- Current packet `UseItem`, packet `EquipItem`, and `MergeItem` now follow Crystal unique-id lookup semantics for current bag items.
- Duplicate-key items on different bag pages no longer consume, equip, or merge against the wrong candidate.
- Full `mir2-simulation` regression is green at `482 / 482`.
- Backend parity tracker moved from `77.27%` to `77.35%`.

## 2026-04-23-R32

Goal: close the current inventory unique-id lookup gap that still treated `CombineItem` and several bag-item packet paths as raw slot references.

Coordinator local work:

- Confirmed from Crystal `PlayerObject.CombineItem`, `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, and `RepairItem` that the current server scans inventory/storage arrays by `UserItem.UniqueID`, not by slot index.
- Added runtime `ItemState.unique_id` compatibility plumbing plus `item_unique_id()` so legacy slot-backed items still deserialize safely while current logic can distinguish real unique ids.
- Updated the current inventory-grid `CombineItem` path to resolve source and target items by unique id and to emit target-side `ItemRepaired`, `ItemSlotSizeChanged`, and `ItemSealChanged` packets with the resolved target unique id.
- Extended current bag-item lookup paths to use unique ids instead of raw slots for `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, `RepairItem`, and the local `drop_item` helper path that first resolves an inventory item reference.
- Removed the default `Bag1` / `Bag2` collision on slot-shared inventory items by deriving distinct fallback ids per bag page, so the starter `Bag2` slot `0` item no longer aliases `Bag1` slot `0`.
- Ensured split-stack clones receive a fresh default unique id for their destination slot instead of duplicating the source item id.
- Added focused regressions for `CombineItem`, `DropItem`, `SellItem`, and `RepairItem` unique-id lookup plus a direct default-id collision check for `Bag1` / `Bag2` slot `0`.
- Kept the round intentionally bounded to the currently modeled inventory-grid and bag-item packet surfaces; hero inventory, move/merge parity, and other gem-family branches remain queued.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation unique_id -- --test-threads=1 --nocapture`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R32` complete.
- Current inventory unique-id lookup now matches Crystal across `CombineItem`, `SplitItem`, `DeleteItem`, `DropItem`, `SellItem`, and `RepairItem`.
- Default runtime `Bag1` / `Bag2` ids no longer collide for same-slot inventory items.
- Full `mir2-simulation` regression is green at `479 / 479`.
- Backend parity tracker moved from `77.19%` to `77.27%`.

## 2026-04-23-R31

Goal: close the bounded player `GemRatePercent` success-rate gap for current inventory-grid `CombineItem` shape-3/4 upgrade branches.

Coordinator local work:

- Selected player `GemRatePercent` as the smallest remaining backend bite after R30 because Crystal `PlayerObject.CombineItem` directly adds `Stats[Stat.GemRatePercent]` to shape-3/4 gem/orb upgrade success chance.
- Preserved the existing zero-bonus success-rate helper for tests and added a runtime path that sums current non-broken equipment `UserItemStat` entries for `GemRatePercent`.
- Wired the current inventory-grid upgrade branch to pass the player gem-rate bonus into the Crystal-shaped success-chance formula.
- Added a focused regression that chooses a deterministic tick where the base chance fails but the equipment-backed `GemRatePercent` bonus succeeds, proving the upgraded item emits `ItemUpgraded`, increments `gem_count`, and applies the added stat.
- Kept the round intentionally bounded: hero-inventory handling, belt/id-collision cleanup, and other gem-family branches remain queued.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test -p mir2-simulation combine_item_packet_upgrade_branch_applies_player_gem_rate_percent_bonus -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R31` complete.
- `CombineItem` packet upgrade parity now applies current player equipment `GemRatePercent` to shape-3/4 success chance.
- Full `mir2-simulation` regression is green at `473 / 473`.
- Backend parity tracker moved from `77.18%` to `77.19%`.

## 2026-04-23-R30

Goal: close the bounded rental binding flag gap for current storage and inventory-grid combine item paths.

Coordinator local work:

- Selected rental `BindingFlags` as the next safe bounded backend bite because Crystal storage checks rental `DontStore`, and Crystal socket/upgrade combine branches check rental `DontUpgrade`.
- Added runtime item/equipment state persistence for rental binding flags and surfaced nonzero values through `UserItem.RentalInformation`.
- Preserved rental flags across inventory/equipment round-trips so equipped or unequipped items do not lose binding metadata.
- Updated `StoreItem` to reject rental `DontStore` the same way it already rejects base `DontStore`.
- Updated current shape-7 socket and shape-3/4 upgrade `CombineItem` branches to reject rental `DontUpgrade` ack-only while preserving source and target state.
- Added focused regressions for rental `DontStore`, rental socket `DontUpgrade`, and rental upgrade `DontUpgrade`.
- Kept the round intentionally bounded: seal rental handling was not added because the audited Crystal source only checked rental `DontUpgrade` on socket and upgrade branches; hero-inventory handling, belt/id-collision cleanup, player `GemRatePercent`, and other gem-family branches remain queued.

Verification:

- `cargo +1.89.0 fmt`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R30` complete.
- `CombineItem` packet parity now blocks rental `DontUpgrade` for the current socket and upgrade branches; storage now blocks rental `DontStore`.
- Full `mir2-simulation` regression is green at `472 / 472`.
- Backend parity tracker moved from `77.17%` to `77.18%`.

## 2026-04-23-R29

Goal: close the next bounded inventory-grid `CombineItem` parity gap by implementing Crystal repair-hammer/sewing packet behavior.

Coordinator local work:

- Re-read the resume/task-queue checkpoint and narrowed the next safe round to Crystal `PlayerObject.CombineItem` shape `1/2/5/6` repair branches instead of larger remaining gaps such as hero inventory or player `GemRatePercent`.
- Confirmed from Crystal `PlayerObject.CombineItem` that repair combine uses the shared target item-type gate, rejects `DontRepair` and wrong target families ack-only, emits `ItemNoRepairNeeded` for full-durability targets, and emits `ItemRepaired` plus success ack after durability mutation.
- Implemented shape `1/2/5/6` source-shape recognition in runtime packet `CombineItem`.
- Added Crystal-style repair target-family gating for hammer vs sewing sources, `DontRepair` ack-only rejection, and full-durability hint rejection.
- Added deterministic repair max-durability loss for repair-combine success so packet tests remain reproducible while preserving Crystal's random-loss branch on target shapes `1/2`.
- Added focused packet regressions for repair success, full-durability rejection with hint, and wrong target-family ack-only failure.

Verification:

- `cargo +1.89.0 fmt`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R29` complete.
- `CombineItem` packet parity now covers Crystal repair-hammer/sewing shapes `1/2/5/6` in addition to the existing socket/seal/upgrade branches.
- Full `mir2-simulation` regression is green at `469 / 469`.
- Backend parity tracker moved from `77.16%` to `77.17%`.

## 2026-04-22-R1

Goal: start 100% Candidate workflow and complete the first backend small parity item under multi-agent coordination.

Coordinator local work:

- Created `docs/AGENT-ORCHESTRATION.md`.
- Created `docs/AGENT-TASK-QUEUE.md`.
- Created `docs/PLAYER-QA-SCRIPT.md`.
- Created this run log.
- Created workspace-level `E:\mir2\AGENTS.md` so future Codex sessions start from the same Candidate workflow.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Hilbert | Crystal Explorer | Crystal `AddItem` belt-priority source behavior | none |
| Gauss | Rust Explorer | current Rust inventory/belt implementation map | none |
| Euclid | Frontend Explorer | frontend 1:1 acceptance matrix | none |
| Carver | Backend Worker | implement Crystal `AddItem` belt-priority | `apps/simulation/src/runtime.rs` |
| Hegel | Backend Worker | replacement for Carver using supported `gpt-5.3-codex-spark` xhigh | `apps/simulation/src/runtime.rs` |

Model allocation note:

- Carver failed before work started because `gpt-5.2-codex` is not supported for this ChatGPT account.
- Hegel replaced Carver with `gpt-5.3-codex-spark` at `xhigh`, matching the current quota profile.
- Hegel did not return within two 180-second waits; the Coordinator closed that worker and implemented the bounded `runtime.rs` change locally to keep the critical path moving.

Expected verification:

- `cargo fmt --check`
- focused `mir2-simulation` item gain/pickup/belt tests from worker
- coordinator review of changed code and docs

Result:

- Crystal Explorer complete:
  - `HumanObject.AddItem` first merges stackables across inventory.
  - Player belt ranges are potion slots `0..4`, amulet slots `4..6`, normal bag start `6`.
  - Potion belt types are `Potion=13`, `Scroll=17`, and `Script=21` when `Effect == 1`.
  - Amulet belt type is `Amulet=8`.
  - Belt placement is priority, not mandatory; fallback scans normal bag then the full inventory array.
- Rust Explorer complete:
  - `SimulationResources` stores `inventory_items` and `belt_items` as separate vectors.
  - `add_or_increment_item_with_durability` currently writes only `inventory_items`; passing `ItemContainer::Belt` alone is insufficient.
  - `can_gain_item_quantity` and slot helpers currently inspect `inventory_items`, so belt-aware placement needs explicit handling.
  - Existing useful tests/helpers include `fill_all_bag_slots`, `add_inventory_test_item`, `set_player_position`, `spawn_ground_drop`, and stack-size regressions.
- Frontend Explorer complete:
  - Existing commands include `npm.cmd run build`, `smoke:crystal-minimap-assets`, `smoke:crystal-map-api`, `smoke:stage5-ui`, and `load:gateway-ws`.
  - Added Evidence Gate and panel matrix to `docs/PLAYER-QA-SCRIPT.md`.
  - Added `docs/FRONTEND-1TO1-GAPS.md`.
- Backend implementation complete:
  - `add_or_increment_item_with_durability` now merges same-key belt stacks before inventory stacks for Bag1/Bag2 gains.
  - Crystal belt-priority gains now choose potion/scroll/script effect 1 slots `0..3`, amulet slots `4..5`, then normal bag fallback.
  - `can_gain_item_quantity` now counts eligible belt slots for Crystal belt-priority gains.
  - `UseItem` now resolves and consumes the referenced belt item for `MirGridType::Belt` packets instead of consuming a same-key inventory item.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_add_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation use_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation add_or_increment_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_npc_giveitem -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation quest_turn_in_full_bag_preserves_quest_state_and_rewards -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_shop_and_auction_full_bag_preserve_gold_and_items -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_npc_buy_item_packet_purchases_trade_goods -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R1` complete.
- Backend parity tracker moved from `76.90%` to `76.91%`.

## 2026-04-22-R2

Goal: complete the next backend parity item: Crystal `DropStackSize` and ground-drop position search.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Arendt | Crystal Explorer | Crystal `DropStackSize` / `ItemObject.Drop(range)` source behavior | none |
| Nietzsche | Rust Explorer | current Rust ground-drop placement and tests | none |

Coordinator local work:

- Marked R2 active in `docs/AGENT-TASK-QUEUE.md`.
- Begins local code/source inspection while explorers run.

Result:

- Crystal Explorer complete:
  - `ItemObject.Drop(int distance)` scans rings from `d=0..distance`, skips invalid points, skips `MovementInfo.Source` transfer tiles, rejects blocking objects, caps per-cell item objects by `Settings.DropStackSize=5`, and chooses the first empty cell or least-populated fallback cell.
  - Manual player item drop range is `1`; manual player gold range is hardcoded `5`; monster ground drops use `Settings.DropRange=4`.
  - Monster item drop failure stops later item drop processing; monster gold chunk placement failures are silent.
- Rust Explorer complete:
  - Confirmed the implementation seam around `spawn_ground_drop`, `drop_gold_impl`, `drop_item_packet`, `spawn_configured_monster_drops`, and current pickup tests.
- Backend implementation complete:
  - Added Crystal constants for drop range, player item/gold ranges, and `DropStackSize`.
  - Added `crystal_ground_drop_position` ring search with blocked-cell, blocking-object, transfer-source, and object-count checks.
  - Added placement-return helpers so drop failure can preserve gold/items.
  - Routed player item drops, player gold drops, and monster ground drops through the Crystal placement path while keeping the exact-position test helper available.
  - Updated the stale adjacent-pickup regression to current Crystal same-cell pickup semantics.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_drop_search -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_drop_table_gold_entry_spawns_pickup_gold_for_monster_death -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R2` complete.
- Backend parity tracker moved from `76.91%` to `76.92%`.

## 2026-04-22-R3

Goal: implement Crystal quest-drop `Q` gating while starting the first frontend shell parity investigation in parallel.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Pascal | Crystal Explorer | Crystal quest-drop `Q` gating source behavior | none |
| Wegener | Rust Explorer | current Rust quest/drop implementation map | none |
| Locke | Frontend Explorer | smallest safe frontend shell parity patch | none |

Coordinator local work:

- Marked R3 active in `docs/AGENT-TASK-QUEUE.md`.
- Inspected Crystal `DropInfo.QuestRequired`, `MonsterObject.Drop`, `HarvestMonster.Harvest`, and Rust drop/quest runtime paths.
- Implemented `ResolvedDropTemplate::Item.quest_required`, removed the old pre-roll `Q` suppression, added active quest-inventory routing for death and harvest drop paths, and moved the Field Wasp quest item path onto the shared gate.
- Added focused tests for `Q` marker preservation, active quest gain, no-active-quest suppression, and full quest-inventory suppression.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_q_drop_marker -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_quest_required_drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation quest_turn_in_full_bag_preserves_quest_state_and_rewards -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation quest -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R3` backend item complete.
- Backend parity tracker moved from `76.92%` to `76.93%`.
- Frontend R4 worker started for the login/select/game shell first patch.

## 2026-04-22-R4

Goal: land the smallest safe frontend shell interaction parity patch while preserving R3 backend changes.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Rawls | Frontend Worker | login Enter-submit and scene pointer double-dispatch guard | `apps/web/app/original-client-shell.tsx`, optional `apps/web/app/globals.css` |

Coordinator local work:

- Marked R4 active in `docs/AGENT-TASK-QUEUE.md`.
- Reviewed worker changes in `apps/web/app/original-client-shell.tsx`.
- Re-ran `npm.cmd run build --prefix E:\mir2\mir2-web3\apps\web` locally; build and TypeScript checks passed.

Outcome:

- Round `2026-04-22-R4` complete.
- Full-project estimate moved from roughly `61.5%` to `61.6%`.
- R5 opened for Crystal random-stat roll generation.

## 2026-04-22-R5

Goal: implement the next backend parity item: current random-stat roll generation for imported Crystal item drops.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Linnaeus | Crystal Explorer | Crystal random-stat source behavior | none |
| Helmholtz | Rust Explorer | current item stat/import/payload implementation map | none |

Coordinator local work:

- Marked R5 active in `docs/AGENT-TASK-QUEUE.md`.
- Inspected Crystal `Settings.LoadRandomItemStats`, `RandomItemStat`, `Envir.CreateDropItem`, and `Envir.UpgradeItem`.
- Implemented the current Rust baseline for Crystal `UpgradeItem`: deterministic `RandomomRange`-style MaxDura, MaxAC, and MaxDC rolls keyed by existing `random_stats_id` profiles.
- Threaded added attack/defence and random durability through resolved drop templates, ground-drop payloads, pickup, harvest transfer, and player item drop preservation.
- Added tests for random profile rolls, resolved drop durability/stat payloads, and pickup `GainedItem.added_stats` preservation.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_resolved_drop_applies_random_attack_defence_and_durability -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup_preserves_random_added_stats -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R5` current random-stat baseline complete.
- Backend parity tracker moved from `76.93%` to `76.94%`.
- R6 opened for added-stat ground item display investigation.

## 2026-04-22-R6

Goal: close the display side of the current random-stat baseline so added-stat ground items appear with Crystal's Cyan item-name rule.

Coordinator local work:

- Verified Crystal `ItemObject` uses `Color.Cyan` whenever `UserItem.IsAdded` is true.
- Added `GroundDropSnapshot.name_colour_argb`.
- Routed current added attack/defence ground drops through the same Cyan name-colour calculation for both `ObjectItem` packets and snapshot-driven web labels.
- Updated the web client to preserve `nameColourArgb` from snapshots/packets and render ground-drop labels using that ARGB value.
- Added a focused regression for added-stat ground items and preserved the existing Crystal grade colour test.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation ground_item_object_uses_cyan_name_colour_for_added_stats -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation ground_item_object_uses_crystal_grade_and_name_colour -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `npm.cmd run build --prefix apps\web`

Outcome:

- Round `2026-04-22-R6` complete.
- Backend parity tracker moved from `76.94%` to `76.95%`.
- Frontend shell parity estimate moved from `42.2%` to `42.3%`.
- R7 opened for the next bounded parity bite selected from explorer findings.

## 2026-04-22-R7

Goal: close the NPC buy-back / used-goods persistence gap from the backend queue.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Sagan | Backend Explorer | identify next R6+ backend parity bites | none |
| Hypatia | Frontend Explorer | ground-drop display data-flow check | none |

Coordinator local work:

- Confirmed Crystal source: `NPCObject.ProcessGoods` expires player buy-back entries after `Settings.GoodsBuyBackTime=60` minutes, moves eligible items into NPC `UsedGoods`, caps same-item stored goods, and saves `UsedGoods`.
- Added persisted `npc_buy_back_items_json` and `npc_used_goods_items_json` save fields with legacy defaults.
- Made buy-back entries player-scoped, save/reload-safe, and expiry-stamped.
- Added used-goods state, expiry processing, Buy/BuyBack/BuyUsed source selection, and removal after resale purchase.
- Preserved current used/buy-back item durability and added attack/defence when buying from those resale lists.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_npc_buy_back -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_npc_buy_item_packet_purchases_trade_goods -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation sell -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation npc -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation legacy_character_save_without_npc_flag_states_uses_default -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R7` complete.
- Backend parity tracker moved from `76.95%` to `76.96%`.
- R8 opened for the next bounded backend/frontend parity bite.

## 2026-04-22-R8

Goal: start full gem/socket validation by adding a bounded socket slot-capacity check to the existing Stage 5 socket-growth path.

Coordinator local work:

- Confirmed Crystal rejects socket growth when item socket metadata is missing or the current slot length is already at the configured cap.
- Added a runtime socket capacity helper backed by imported Crystal item `slots`.
- Updated `item.addSocket` so items with no capacity, such as the default Wooden Sword, do not mutate state and do not emit `ItemSlotSizeChanged`.
- Kept the successful packet path covered by using a manifest item with imported socket capacity.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation stage5_item_add_socket -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_item_seal_emits_item_seal_changed -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R8` complete.
- Backend parity tracker moved from `76.96%` to `76.97%`.
- Full gem/socket validation moved from not-started to in-progress; source gem item validation remains open.

## 2026-04-22-R9

Goal: start full seal-source validation by adding the first Crystal rejection path for already-sealed equipment.

Coordinator local work:

- Confirmed Crystal rejects seal attempts when an item has active `SealedInfo.ExpiryDate`.
- Updated `item.seal` so an already-sealed equipped item does not overwrite expiry and does not emit `ItemSealChanged`.
- Added a regression covering first seal success followed by rejected reseal while preserving the original expiry.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation stage5_item_seal -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R9` complete.
- Backend parity tracker moved from `76.97%` to `76.98%`.
- Full seal-source validation moved from not-started to in-progress; source item validation and reseal-delay metadata remain open.

## 2026-04-22-R10

Goal: close the remaining current BenedictionOil branches beyond guaranteed Luck gain.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Russell | Frontend Explorer | next smallest frontend 1:1 patch | none |
| Laplace | Backend Explorer | next smallest backend parity task | none |

Coordinator local work:

- Confirmed Crystal `TryLuckWeapon` can curse, add Luck, or have no effect, and consumes BenedictionOil for all true outcomes.
- Updated current BenedictionOil handling to use deterministic Crystal-shaped branch rolls.
- Added curse and no-effect paths: curse decrements weapon Luck and emits `RefreshItem`; no-effect consumes the oil without `RefreshItem`.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation benediction_oil -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R10` complete.
- Backend parity tracker moved from `76.98%` to `76.99%`.
- R11 opened for the frontend scene target keyboard action chain recommended by the frontend explorer.

## 2026-04-22-R11

Goal: land the smallest frontend scene-target action chain recommended by the frontend explorer.

Coordinator local work:

- Reused the existing selected target data flow from `page.tsx` into `OriginalClientShell`.
- Added selected-target keyboard routing: `Enter`/space invokes the primary target action and `A` invokes approach, while preserving input-field guards and belt number hotkeys.
- Added localized selected-target nameplate feedback for action type and distance.

Verification:

- `npm.cmd run build --prefix apps\web`

Outcome:

- Round `2026-04-22-R11` complete.
- Frontend shell parity estimate moved from `42.3%` to `42.4%`.
- R12 opened for the seal source item validation baseline recommended by the backend explorer.

## 2026-04-22-R12

Goal: deepen the current seal flow with source item validation while keeping the legacy Stage 5 command signature compatible.

Coordinator local work:

- Confirmed Crystal `CombineItem` seal uses a source `Gem` with `Shape == 8`, derives seal duration from source durability, rejects active already-sealed targets, then consumes the source on success.
- Added `item.seal <slot> <minutes> <source_key>` validation for inventory source presence and seal-source eligibility while preserving the old `item.seal <slot> <minutes>` path.
- Added a Stage 5 test seal source for the currently missing Jev shape-8 seal-gem data, without weakening the manifest-backed Crystal rule.
- Added regressions for missing source, wrong source, successful source consumption, legacy success, and already-sealed rejection.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation stage5_item_seal -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R12` complete.
- Backend parity tracker moved from `76.99%` to `77.00%`.
- R13 opened for socket source gem validation; Galileo is running a read-only source/Rust pass in parallel.

## 2026-04-22-R13

Goal: deepen the current socket-slot growth path with optional source gem validation and source consumption.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Galileo | Backend Explorer | Crystal socket source / `ValidGemForItem` read-only pass | none |

Coordinator local work:

- Confirmed Crystal socket growth is the `CombineItem` shape-7 branch: source must be a `Gem`, target must have capacity, `ValidGemForItem` matches the source unique flags to the target item type, and the source is consumed after success.
- Added `item.addSocket <slot> <source_key>` validation for inventory source presence and socket-source eligibility while preserving the old `item.addSocket <slot>` Stage 5 path.
- Added a Stage 5 socket-source test item because the current Jev manifest has no real shape-7 socket source gems, while keeping the manifest-backed Crystal rule in place for future data.
- Added regressions for missing source, wrong source, source consumption on success, legacy success, and capacity rejection.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation stage5_item_add_socket -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R13` complete.
- Backend parity tracker moved from `77.00%` to `77.01%`.
- R14 opened for seal reseal-delay metadata.

## 2026-04-22-R14

Goal: align current item sealing with Crystal `SealedInfo.NextSealDate` / `Settings.ItemSealDelay` metadata.

Coordinator local work:

- Confirmed Crystal stores both `ExpiryDate` and `NextSealDate`, rejects reseal while `NextSealDate > Envir.Now`, and defaults `SealDelay=60` minutes.
- Added persisted `sealed_next_time_binary_datetime` to equipped-item state and world snapshots.
- Updated current `item.seal` to set `NextSealDate = ExpiryDate + 60 minutes`, reject reseal after expiry but before that next-seal date, and expose the field through the Crystal `UserItem.SealedInfo` payload.
- Added save/reload and legacy missing-field coverage for the new reseal-delay metadata.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation stage5_item_seal -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`

Outcome:

- Round `2026-04-22-R14` complete.
- Backend parity tracker moved from `77.01%` to `77.02%`.
- R15 opened for full random-stat family source mapping.

## 2026-04-22-R15

Goal: widen current Crystal random-stat drops from the MaxDura/MaxAC/MaxDC baseline to the full Jev random-stat family payload that can safely fit the current runtime.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Herschel | Backend Explorer | Crystal `RandomItemStats.ini`, `RandomItemStat`, `UpgradeItem`, and drop-group source audit | none |
| Curie | Rust Explorer | Current Rust random-stat/drop/pickup/persistence implementation map | none |

Coordinator local work:

- Mapped Crystal stat ids and Jev `RandomItemStats.ini` profiles for current `random_stats_id` values 1 through 10.
- Added generic `added_stats`, `cursed`, and `socket_slots` metadata through resolved drops, ground drops, pickup, harvest reward transfer, inventory state, equipment state, `UserItem` payloads, and JSON save/reload.
- Preserved existing `added_attack` / `added_defence` compatibility while carrying the full added-stat vector for non-legacy families such as MC, accuracy, strong, attack speed, Luck, resistances, HP/MP, criticals, freezing, and poison attack.
- Extended ground item Cyan detection to consider generic added stats and socket slots.
- Fixed three full-suite regressions surfaced by the broader verification pass: guard attacks now preserve the Crystal target-back packet plus follow-up facing turn, ThunderElement reposition coverage uses an in-bounds map fixture, and the Stage 3 pickup flow stands on the drop cell required by Crystal current-cell pickup semantics.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation random -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item_roll_fields_persist_through_save_and_reload -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-22-R15` complete.
- Backend parity tracker moved from `77.02%` to `77.03%`.
- R16 opened for data-driven `RandomItemStats.ini` manifest import and removal of the remaining hardcoded profile table.

## 2026-04-22-R16

Goal: replace the remaining hardcoded random-stat profile table with generated `RandomItemStats.ini` manifest data while preserving the current full random-stat payload behavior.

Coordinator local work:

- Extended `generate-crystal-runtime-manifests.mjs` to parse `Crystal/Build/Server/Debug/Configs/RandomItemStats.ini`, emit complete `[ItemN]` profiles, and skip the incomplete sentinel section.
- Added `crystal_random_item_stats_manifest.json` plus typed `mir2-game-data` accessors for `CrystalRandomItemStatProfile` and `CrystalRandomStatRoll`.
- Swapped the simulation runtime from its local hardcoded random-stat profile table to the generated game-data lookup, while keeping `random_stats_id == 0` as the no-profile path.
- Verified the generated manifest still drives the same current Jev random-stat family payloads through drop resolution, pickup, item state, and persistence coverage.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo test -p mir2-game-data crystal_random_item_stats_manifest_loads -- --nocapture`
- `cargo test -p mir2-game-data -- --nocapture`
- `cargo test -p mir2-simulation random -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo check -p mir2-simulation`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-22-R16` complete.
- Backend parity tracker moved from `77.03%` to `77.04%`.
- R17 opened for exact Crystal `GROUP` drop semantics.

## 2026-04-22-R17

Goal: add Crystal `GROUP`, `GROUP*`, `GROUP^`, and nested drop-block semantics to the generated drop manifest and runtime evaluator.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Pasteur | Crystal Explorer | Crystal `DropInfo.Load`, `ParseGroup`, and `AttemptDrop` source semantics | none |
| Huygens | Rust Explorer | Current drop manifest/runtime gap map and smallest write set | none |

Coordinator local work:

- Confirmed Crystal group behavior: group parents roll their own chance first; child entries roll independently; `GROUP*` keeps one successful item after child rolls while preserving successful child gold; `GROUP^` stops after the first successful child; nested groups recurse through the same rules.
- Extended the runtime manifest generator to preserve group trees instead of flattening all entries, including nested group blocks and Crystal-style `#INSERT` append handling.
- Added `CrystalDropGroup` to `mir2-game-data` and a group-shape deserialization regression.
- Replaced the simulation drop-table flat map with a recursive group evaluator while preserving existing item/gold resolution, quest markers, random-stat generation, and ground-drop placement.
- Added focused regressions for `GROUP*`, `GROUP^`, and nested group composition.

Verification:

- `node packages\tooling\scripts\generate-crystal-runtime-manifests.mjs`
- `cargo fmt`
- `cargo test -p mir2-game-data crystal_drop -- --nocapture`
- `cargo test -p mir2-game-data -- --nocapture`
- `cargo test -p mir2-simulation crystal_group -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_nested_group -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo check -p mir2-simulation`
- `cargo test -p mir2-simulation -- --test-threads=1`
- `cargo fmt --check`

Outcome:

- Round `2026-04-22-R17` complete.
- Backend parity tracker moved from `77.04%` to `77.05%`.
- R18 opened for Crystal delayed drop visibility and remaining inventory rejection edges.

## 2026-04-22-R18

Goal: close the current ground-drop visibility and pickup rejection edge cases against Crystal source instead of the prior delayed-visibility/weight assumptions.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Kuhn | Crystal Explorer | `ItemObject`, `PlayerObject.PickUp`, `CanGainItem`, `CanGainGold`, and owner-window source audit | none |
| Dirac | Rust Explorer | Current Rust ground-drop visibility, pickup, owner, gold-cap, full-bag, and weight handling map | none |

Coordinator local work:

- Confirmed Crystal `ItemObject.Drop()` / `Spawned()` broadcasts `ObjectItem` / `ObjectGold` immediately; there is no normal delayed-visibility field for owned drops.
- Corrected the earlier bag-weight assumption: Crystal `CanGainItem` gates by free slots/stacking only, while bag weight refreshes after gain and affects movement rather than pickup/harvest acceptance.
- Updated `ClientPacket::PickUp` to scan only the player's current cell in deterministic Crystal insertion order, skip owner-blocked/full-bag/gold-cap candidates, collect a later pickable drop when present, and emit the owner warning only when no later pickable candidate exists.
- Removed the runtime pickup/harvest weight hard gate so overweight item gains are allowed and reflected in subsequent weight state.
- Added regressions for immediate visibility under owner lock, owner-blocked-first then later gold pickup, full-bag item then later gold pickup, and overweight pickup allowed like Crystal.

Verification:

- `cargo fmt`
- `cargo test -p mir2-simulation pickup_packet_skips -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup_respects_crystal_drop_owner_window -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup_allows_overweight_item_like_crystal -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation pickup -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`
- `cargo fmt --check`

Outcome:

- Round `2026-04-22-R18` complete.
- Backend parity tracker moved from `77.05%` to `77.06%`.
- R19 opened for Crystal `HarvestMonster` transfer timing and leftover inventory semantics.

## 2026-04-22-R19

Goal: align current `HarvestMonster` transfer timing and leftover-drop behavior with Crystal's `_drops` list semantics.

Agents launched:

| Agent | Role | Task | Write Set |
| --- | --- | --- | --- |
| Noether | Crystal Explorer | Crystal `HarvestMonster`, `Deer`, `PlayerObject.Harvest`, quest item, partial-transfer, and owner/source audit | none |
| Dalton | Rust Explorer | Current Rust harvest state, transfer, partial, and test map audit | none |

Coordinator local work:

- Confirmed Crystal default `HarvestMonster` needs two skin passes to generate `_drops`, then a follow-up harvest call transfers items and emits `ObjectHarvested`; Deer uses five skin passes, then a follow-up transfer.
- Added persisted `PendingHarvestDrops` so harvest rewards are rolled and materialized once when the skin count reaches zero, instead of being re-rolled on the later transfer call.
- Changed current Crystal-backed Hen/Deer/CaveMaggot/ToxicGhoul harvest timing so the final skin pass prepares pending drops but does not transfer them until the next harvest call.
- Implemented Crystal-style partial transfer: items that fit are gained, untransferable leftovers remain pending, the corpse is not marked harvested, and a later harvest retries the remaining drops.
- Kept quest-required harvest drops gated at pending-drop preparation time when no active matching quest can accept them.

Verification:

- `cargo fmt`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation hen_is_passive -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`
- `cargo fmt --check`

Outcome:

- Round `2026-04-22-R19` complete.
- Backend parity tracker moved from `77.06%` to `77.07%`.
- R20 opened for Crystal harvest owner/EXPOwner scan rejection semantics.

## 2026-04-22-R20

Goal: align harvest target scanning with Crystal `EXPOwner`/group rejection behavior for dead harvestable corpses.

Coordinator local work:

- Added `HarvestOwnership` for harvestable corpses and attaches current-player ownership when a harvest monster is defeated through the normal runtime defeat path.
- Changed harvest target selection to scan the Crystal front-centered 9-cell search area, skip corpses owned by another player, and continue to later eligible corpses.
- Added group-owner bypass using the existing configured group member object-id set.
- Emitted Crystal localization key `server.NoNearbyOwnedCarcasses` only when at least one owner-blocked corpse exists and no eligible harvest target is found.
- Added focused coverage for owner-blocked-only, owner-blocked-then-later-candidate, and owner-group-member harvest paths.

Verification:

- `cargo fmt`
- `cargo test -p mir2-simulation harvest_owner -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest_skips_owner -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest_allows_owner_group -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation harvest -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation drop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`
- `cargo fmt --check`

Outcome:

- Round `2026-04-22-R20` complete.
- Backend parity tracker moved from `77.07%` to `77.08%`.
- R21 opened for broader Crystal inventory/economy rejection edge audit.

## 2026-04-22-R21

Goal: align high-impact Crystal inventory/economy rejection edges around NPC selling, game-shop credit purchases, and mail attachment claiming.

Agents:

| Agent | Role | Scope | Write Set |
| --- | --- | --- | --- |
| Peirce | Crystal Explorer | Crystal buy/sell/repair/game-shop/trade/mail/auction rejection audit | none |
| Banach | Rust Explorer | Current Rust Stage 5 economy rejection/test coverage audit | none |

Coordinator local work:

- Required an active Crystal sell service (`@Sell` / `@BuySell`) before `SellItem` can remove inventory or grant gold.
- Added Crystal partial-stack sale overflow protection: partial stack sales are rejected when the resulting gold would exceed `uint.MaxValue`, preserving inventory and gold.
- Changed current Stage 5 credit-shop purchases toward Crystal game-shop behavior: credit is debited with `LoseCredit`, the item is mailed as an attachment, and full bags no longer block the purchase.
- Extended Stage 5 mail with item attachments and claim-time bag capacity checks so full bags preserve unclaimed mail and do not grant the attached item.
- Added focused tests for inactive-service sell rejection, partial-stack sell gold-cap rejection, credit-shop mail delivery, and full-bag mail claim preservation.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo test -p mir2-simulation sell_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_credit_shop -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_trade_shop_and_auction -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_social_group_guild_mail -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation stage5_shop_and_auction_full_bag -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-22-R21` complete.
- Backend parity tracker moved from `77.08%` to `77.09%`.
- R22 opened for Crystal repair/NPC-buy rejection edge semantics.

## 2026-04-22-R22

Goal: align current Crystal `BuyItem` rejection edges with the server's silent-return behavior before continuing into repair-specific semantics.

Coordinator local work:

- Added an active Crystal buy-service helper covering buy-capable service pages (`@Buy`, `@BuySell`, buy-back, used-goods, pearl/new-buy variants).
- Changed `BuyItem` handling to return no packets and preserve state for invalid panel type, zero count, missing active NPC service, and active non-buy pages such as `@Repair`.
- Kept the same silent no-mutation behavior for missing goods, missing item metadata, invalid requested counts, insufficient gold, and full bags.
- Added focused coverage that opens a valid `@BuySell` page, proves invalid panel/count requests are silent, then opens `@Repair` and proves valid trade goods cannot be purchased from a repair page.

Verification:

- `cargo fmt --check`
- `cargo test -p mir2-simulation crystal_npc_buy_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-22-R22` complete.
- Backend parity tracker moved from `77.09%` to `77.10%`.
- R23 opened for Crystal repair service rejection/cost semantics.

## 2026-04-22-R23 prework / restart handoff

Goal: make the active R23 repair task restart-safe before a possible machine shutdown or Codex context loss.

Agents:

| Agent | Role | Scope | Write Set |
| --- | --- | --- | --- |
| Fermat | Crystal Explorer | Crystal `RepairItem` / `SRepairItem`, NPC page gating, ack/success packets, cost formula, and rejection order | none |
| Beauvoir | Rust Explorer | Current Rust `RepairItem` dispatch, lookup semantics, repair helpers, and tests | none |

Captured findings:

- Crystal `RepairItem` / `SRepairItem` packets carry only `UniqueID`; the server matches it against backpack inventory item `UniqueID`, not an equipment slot reference.
- Crystal sends `S.RepairItem` at repair-entry time as a client grid unlock ack, then applies dead/page/range/item/repairability/cost checks; success is the later `S.ItemRepaired`.
- Active NPC page must match `[@REPAIR]` or `[@SREPAIR]`; page mismatch returns after the entry ack with no success mutation.
- Normal repair costs `ItemData.RepairPrice() * PriceRate(this)`; special repair costs `ItemData.RepairPrice() * 3 * PriceRate(this)`.
- `DontRepair` / `NoSRepair` and script-type mismatch emit repair-specific system messages; insufficient gold returns silently after cost calculation.
- Current Rust repair still treats `unique_id` as an equipment reference, has no NPC service gating, and has no gold cost.

Coordinator local work:

- Added `docs/AGENT-RESUME-HANDOFF.md` with the active R23 checkpoint, resume prompt, model/effort policy, subagent workflow, R22 verification commands, and R23 source findings.
- Updated `docs/AGENT-ORCHESTRATION.md` so the current round status points to R23 instead of stale R8 context.
- Added a restart handoff note to `docs/AGENT-TASK-QUEUE.md`.

Next action:

- Continue R23 implementation from `docs/AGENT-RESUME-HANDOFF.md`: preserve item-use powder/oil repair, but align NPC `RepairItem` / `SRepairItem` around inventory `UniqueID`, active repair-service context, Crystal cost/rejection order, `LoseGold`, and `ItemRepaired`.

## 2026-04-23-R23

Goal: finish Crystal NPC `RepairItem` / `SRepairItem` semantics for current repair service pages.

Coordinator local work:

- Recorded active repair service context when imported NPC scripts emit `NPCRepair` or `NPCSRepair`, so repair packets can require the matching `@Repair` / `@SRepair` page.
- Changed `RepairItem` / `SRepairItem` handling to preserve Crystal's entry `RepairItem` ack while applying the later mutation only after Crystal-style rejection checks.
- Switched NPC repair lookup from equipped-slot references to current backpack item unique ids.
- Implemented Crystal repairability checks for `DontRepair` and special-repair `NoSRepair`, plus NPC script `[Types]` filtering with `CannotRepairItem` / `CannotRepairItemHere` messages.
- Implemented Crystal cost behavior: normal repair uses `RepairPrice * PriceRate`, special repair uses triple cost, insufficient gold silently returns after the entry ack, success emits `LoseGold` plus `ItemRepaired`.
- Implemented normal-repair max-durability loss while keeping special repair from reducing max durability.
- Preserved separate item-use repair powder, `RepairOil`, and `WarGodOil` flows.
- Removed the obsolete equipment-slot NPC repair helper.

Verification:

- `cargo fmt`
- `cargo test -p mir2-simulation repair_item -- --test-threads=1 --nocapture`
- `cargo fmt --check`
- `cargo test -p mir2-simulation repair -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation crystal_npc_service_links -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-22-R23` complete.
- Backend parity tracker moved from `77.10%` to `77.11%`.
- Full `mir2-simulation` regression passed with 453 tests.
- R24 opened for Crystal `SellItem` item flag/type rejection semantics.

## 2026-04-23-R24

Goal: align current NPC `SellItem` with Crystal item flag, script type, price, ack, and gold-cap edge semantics.

Agents:

| Agent | Role | Scope | Write Set |
| --- | --- | --- | --- |
| Turing | Crystal Explorer | Crystal `PlayerObject.SellItem`, packet fields, page gating, item flags, script types, price, gold-cap, and failure order | none |
| Avicenna | Rust Explorer | Current Rust `SellItem` dispatch, service context, implementation, buy-back interactions, and focused tests | none |

Coordinator local work:

- Added the Crystal `DontSell` bind flag and enforced it as an ack-only failure.
- Changed `SellItem` failure semantics to match Crystal: zero count, inactive service, missing item, oversized count, `DontSell`, and partial-stack gold overflow now return only `SellItem(success=false)`; script `[Types]` mismatch emits `CannotSellItemHere` plus the failure ack.
- Kept `SellItem` active-page gating to `@SELL` / `@BUYSELL`; Crystal source showed `@BUYSELLNEW` can open a sell packet surface but `PlayerObject.SellItem` itself does not accept it.
- Changed sale value to follow Crystal `UserItem.Price() / 2`, including durability and added-stat price factors for mapped Crystal items.
- Preserved Crystal's asymmetrical gold-cap behavior: partial-stack overflow rejects before mutation, while full-stack sale succeeds and clamps gained gold, including a zero-gold `GainedGold` packet when already at cap.
- Updated sell/buy-back tests to sell allowed WickedTrader item types instead of potions rejected by the script `[Types]` section.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo test -p mir2-simulation sell_item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation sell -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo test -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R24` complete.
- Backend parity tracker moved from `77.11%` to `77.12%`.
- Full `mir2-simulation` regression passed with 457 tests.
- R25 opened for Crystal storage item flag/rejection semantics.

## 2026-04-23-R25

Goal: align Crystal storage `StoreItem` / `TakeBackItem` item flag and rejection semantics.

Agents:

| Agent | Role | Scope | Write Set |
| --- | --- | --- | --- |
| Tesla | Crystal Explorer | Crystal `StoreItem` / `TakeBackItem`, packet fields, page/range/access gating, bind flags, storage indexes, and ack behavior | none |
| Einstein | Rust Explorer | Current Rust storage implementation and tests | none |

Captured Crystal findings:

- `C.StoreItem` carries only `from` and `to`; `S.StoreItem` returns `from`, `to`, and `success`.
- `C.TakeBackItem` carries only `from` and `to`; `S.TakeBackItem` returns `from`, `to`, and `success`.
- Both actions require active `[@STORAGE]`, NPC range, and `CanAccessStorage`.
- Store rejects invalid source/target indexes, invalid storage capacity, missing inventory item, `DontStore` / rental `DontStore`, and occupied storage target.
- TakeBack rejects invalid source/target indexes, invalid storage capacity, missing storage item, and occupied inventory target.
- Store target occupied fails; TakeBack target occupied fails. Crystal does not swap in these packet handlers.
- Rejections covered by this round are ack-only failures with no chat message.

Coordinator local work:

- Finished the partial storage parity patch by recording `NPCStorage` as an active Crystal storage service so real `@Storage` NPC flows preserve `active_npc_service = STORAGE`.
- Kept Crystal ack-only `StoreItem` / `TakeBackItem` failure semantics for inactive service, password lock, invalid slots/capacity, missing items, `DontStore`, and occupied targets, and added an end-to-end regression that opens `@Storage` and stores/takes back without the test helper.
- Added a Unix `crystal_local_time_snapshot()` implementation plus the direct `libc` dependency so the existing `DAYOFWEEK` / `HOUR` / `MIN` NPC-condition regression also passes on the Mac verification environment; this was a pre-existing non-Windows test gap surfaced by the full suite.
- Refreshed `Cargo.lock` after adding the direct `libc` dependency.

Rust Explorer findings:

- Current packet dispatch is direct from `ClientPacket::StoreItem` / `TakeBackItem` to the storage handlers.
- The new storage gate requires `active_npc_service.label_key == "STORAGE"`.
- `record_crystal_npc_service_context` does not yet record `NPCStorage`, even though the imported `@Storage` flow emits `NPCStorage`.
- Because normal dialogs clear `active_npc_service`, end-to-end NPC storage may fail unless `NPCStorage` is added to the recorded service labels.
- Recommended first patch after restart: add `NPCStorage` service activation and a regression that opens `@Storage`, then performs store/takeback without using the test helper.

Verification:

- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test -p mir2-simulation crystal_npc_storage_service_context_allows_store_and_take_back_without_helper -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation crystal_npc_time_and_bag_conditions_follow_runtime_state -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R25` complete.
- Backend parity tracker moved from `77.12%` to `77.13%`.
- Full `mir2-simulation` regression passed with 458 tests.
- Mac verification note: default `rustc 1.87.0` does not compile locked `bevy_* 0.17.3`; verification used `cargo +1.89.0`.
- R26 remains at queue-selection stage for the next bounded parity bite.

## 2026-04-23-R26

Goal: close the bounded real-client `CombineItem` packet parity gap by wiring Crystal packet ids/payloads into the already-modeled shape-7 socket-growth and shape-8 seal semantics.

Coordinator local work:

- Confirmed Crystal `C.CombineItem` / `S.CombineItem` payload order, dispatch path, ack shape, and client-side source-stack handling against the Crystal client/server code.
- Added protocol support for Crystal `CombineItem`: ids, client/server enum variants, encode/decode, trace names, and codec coverage.
- Added gateway JSON conversion for `ServerPacket::CombineItem` plus a focused event test.
- Wired `ClientPacket::CombineItem` into runtime `combine_item_impl` for the current inventory-grid socket/seal branches, preserving Crystal-style success ack ordering after hint plus `ItemSlotSizeChanged` / `ItemSealChanged`.
- Threaded seal metadata through `ItemState`, `UserItem.SealedInfo`, and inventory/equipment round-trips so the packet path mutates the same saved/runtime state as the existing Stage 5 helpers.
- Kept the round intentionally bounded: the strict Crystal target-type gate was left out of the packet path because the imported manifest currently lacks meaningful real socket-capacity targets for that rule; full target-type, hero-inventory, and other combine-branch parity remain open.
- Re-ran the storage regression because the shared item/runtime state changed in the same file.

Verification:

- `cargo +1.89.0 fmt`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test -p mir2-protocol item_and_combat_client_packets_use_crystal_payloads -- --nocapture`
- `cargo +1.89.0 test -p mir2-protocol item_action_ack_server_packets_use_crystal_ids -- --nocapture`
- `cargo +1.89.0 test -p mir2-gateway combine_item_server_event_exposes_crystal_payload_fields -- --nocapture`
- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R26` complete.
- Backend parity tracker moved from `77.13%` to `77.14%`.
- Full `mir2-simulation` regression passed with 461 tests.
- R27 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-23-R27

Goal: close the next bounded `CombineItem` parity bite by implementing Crystal inventory-grid shape-3/4 gem/orb upgrade semantics.

Coordinator local work:

- Confirmed Crystal shape-3/4 `CombineItem` behavior and `ItemUpgraded` enum ordering against `PlayerObject.CombineItem`, `ValidGemForItem`, `GetGemType`, `HumanObject.GetCurrentStatCount`, and the shared packet enums.
- Added protocol support for Crystal `ServerPacket::ItemUpgraded` / id `216`, including ids, codec coverage, trace names, and gateway JSON conversion.
- Extended runtime `ClientPacket::CombineItem` handling to cover the current inventory-grid shape-3/4 gem/orb upgrade branch with Crystal-shaped success, reject, consume, and destroy behavior.
- Persisted `gem_count` through runtime item/equipment state plus `UserItem` round-trips so upgrade state survives inventory/equipment/save flows.
- Added focused regressions for upgrade success emitting `ItemUpgraded` plus `CombineItem(success=true)`, max-added-stat rejection, invalid gem/item combinations, and failure destroy branches.
- Kept the round intentionally bounded: full Crystal target-type gating across combine branches, hero-inventory handling, belt/id-collision cleanup, rental `DontUpgrade`, and player `GemRatePercent` remain open.

Verification:

- `cargo +1.89.0 fmt`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test -p mir2-protocol item_slot_seal_and_upgrade_server_packets_use_crystal_ids -- --nocapture`
- `cargo +1.89.0 test -p mir2-gateway item_slot_and_seal_server_events_expose_crystal_payload_fields -- --nocapture`
- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R27` complete.
- Backend parity tracker moved from `77.14%` to `77.15%`.
- Full `mir2-simulation` regression passed with 465 tests.
- R28 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-23-R28

Goal: align the shared Crystal `CombineItem` target item-type gate across packet socket/seal/upgrade branches.

Coordinator local work:

- Confirmed from `Crystal/Server/MirObjects/PlayerObject.cs` that `PlayerObject.CombineItem` rejects any target whose `ItemType` is outside `1..=11` before branch-specific socket/seal/upgrade handling.
- Added the same top-level target gate to current runtime `combine_item_impl`, so packet-driven shape-7 and shape-8 flows now ack-fail immediately on out-of-window targets instead of falling through into `InvalidCombination` or mutating non-equipment inventory items.
- Updated focused regressions to cover the new Crystal behavior: a slotted-but-type-19 `BengalTiger` target is now rejected ack-only, and a shape-8 seal attempt against a `red-potion` target stays ack-only with no seal metadata mutation.
- Kept the round intentionally bounded: hero-inventory handling, belt/id-collision cleanup, rental `DontUpgrade`, player `GemRatePercent`, and other gem-family branches remain open.

Verification:

- `cargo +1.89.0 fmt`
- `cargo +1.89.0 fmt --check`
- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`

Outcome:

- Round `2026-04-23-R28` complete.
- Backend parity tracker moved from `77.15%` to `77.16%`.
- Full `mir2-simulation` regression passed with 466 tests.
- R29 reopened at queue-selection stage for the next bounded parity bite.

## 2026-04-27-R227

Goal: begin post-1:1 product evolution by landing the first production-shaped Admin API and Admin Web foundation.

Coordinator local work:

- Added persistent-storage-ready `AdminCommandRepository` and `AuditRepository` traits to `apps/admin-api`.
- Replaced the earlier in-memory-only command dedupe with command records, audit records, status updates, and repository-backed idempotency.
- Added `SystemMailDomain`, `SystemMailExecutor`, and `InMemorySystemMailOutbox` so `SendSystemMail` has a real domain boundary without mutating live game state yet.
- Added Axum routes for health, command records, audit records, system-mail outbox, and `SendSystemMail` writes.
- Added `apps/admin-web` as a separate NextJS operations console with Dashboard, Player Management, Player Detail, Economy, Activities, World Monitor, Anti-Cheat, Mail/GM Tools, and Audit Log pages.
- Wired `apps/admin-web/app/api/admin/system-mail/route.ts` to forward GM mail commands to the Rust Admin API with server-side operator headers.
- Upgraded the new admin web app to `next@16.2.4` after `npm audit --audit-level=high` flagged the initial Next 16.2.1 high-severity advisory.
- Captured admin UI smoke screenshots under `docs/admin-web-dashboard-smoke.png` and `docs/admin-web-gm-tools-smoke.png`.
- Updated `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`, `docs/AGENT-TASK-QUEUE.md`, `apps/admin-api/README.md`, and `apps/admin-web/README.md`.

Verification:

- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`
- `cargo +1.89.0 fmt --check`
- `apps/admin-web ./node_modules/.bin/tsc --noEmit`
- `apps/admin-web ./node_modules/.bin/next build`
- `apps/admin-web npm audit --audit-level=high`
- `curl http://127.0.0.1:7420/health`
- direct `POST /admin/commands/send-system-mail` to Rust Admin API
- `POST /api/admin/system-mail` through Admin Web proxy
- Playwright screenshots for Dashboard and GM Tools
- `cargo +1.89.0 test --locked --workspace -- --test-threads=1`

Outcome:

- Admin API and Admin Web are now connected for the first safe write path.
- `SendSystemMail` is command/audit/outbox-complete, but not connected to live account/world/mail delivery yet.
- `npm audit --audit-level=high` is green after upgrading admin-web to Next 16.2.4. A remaining PostCSS moderate advisory is still reported by full audit; `npm audit fix --force` proposes a breaking downgrade to Next 9.3.3, so it was not applied.
- Next implementation targets: Postgres command/audit repositories, real operator auth, and live mail-service delivery from the outbox boundary.

## 2026-04-27-R239-R244

Goal: complete the seven-phase production-control-plane route from approval workflow through outbox lifecycle, GM executors, Postgres source hardening, Redis routing, read models, and runbook updates.

Coordinator local work:

- Added persistent `admin_approvals`, approval API routes, approval gates for high-risk commands, Admin Web Approvals, and approval requested/approved/rejected events.
- Added `dispatch-admin-outbox` JetStream mode plus Redpanda lifecycle events for retry and dead-letter transitions.
- Added Admin API routes and executors for grant item, grant gold, kick player, and ban account. Kick removes gateway session cache records by character routing; ban persists on account records and simulation rejects login/start-game.
- Added Postgres account ban columns and a focused source-mode stale `save_version` writer test after account-version refresh.
- Extended Redis `GatewaySessionCache` with a character-name routing index using the same TTL as the session record.
- Added Admin API `/admin/timeline` merging command, audit, approval, and ClickHouse event records, plus Admin Web Timeline.
- Updated infra/admin/orchestration handoff docs for Redpanda/ClickHouse topics, JetStream mode, approvals, Redis routing, and timeline read models.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation banned_account_is_rejected_on_login_and_start_game -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-simulation config::tests::postgres_source_mode_rejects_stale_character_save_writer -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-gateway session_cache -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-admin-api kick_and_ban_commands_execute_with_expected_permissions -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-admin-api admin_timeline_merges_local_command_audit_and_approval_records -- --test-threads=1`
- `cargo +1.89.0 fmt --check`
- `apps/admin-web ./node_modules/.bin/tsc --noEmit`

Outcome:

- Phase 1-7 implementation is code-complete locally and ready for full requested baseline verification.

## 2026-04-28-R245

Goal: make the local admin backend immediately browser-testable while tightening the operator and approval boundaries for the next production-control-plane slice.

Coordinator local work:

- Added optional `ADMIN_OPERATOR_POLICY_PATH` auth to Admin API. When configured, the Bearer token maps to a fixed operator identity and permission set from JSON policy instead of trusting spoofable operator headers.
- Added default requester self-approval blocking for approval decisions, with `ADMIN_APPROVAL_ALLOW_SELF=true` reserved for local single-operator smoke runs.
- Added Admin Web GM Tools server-action forms for `GrantItem`, `GrantCurrency`, `KickPlayer`, and `BanAccount`, including optional command/trace/approval IDs and command result notices.
- Started the local browser-testable stack: Docker Postgres/Redis/NATS/Redpanda/ClickHouse, Gateway in explicit Postgres source mode with Redis routing cache, Admin API with Postgres/ClickHouse/gateway integrations, and Admin Web on port 3020.
- Updated admin architecture, infra, app README, orchestration, task queue, and handoff docs with the local testing path and auth/approval notes.

Verification:

- `cargo +1.89.0 test --locked -p mir2-admin-api operator_policy -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-admin-api approval_decision_blocks_self_approval_by_default -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`
- `cargo +1.89.0 fmt --check`
- `apps/admin-web ./node_modules/.bin/tsc --noEmit`
- `apps/web ./node_modules/.bin/tsc --noEmit`
- Docker services healthy: Postgres, Redis, NATS, Redpanda, ClickHouse
- `curl -fsS http://127.0.0.1:7420/health`
- `curl -fsS http://127.0.0.1:7110/health`
- `curl -fsS http://127.0.0.1:3020/gm-tools`
- `curl -fsS http://127.0.0.1:3010`

Outcome:

- Local admin testing is ready at `http://127.0.0.1:3020`.
- Admin API is ready at `http://127.0.0.1:7420`.
- Gateway web/admin endpoints are ready at `http://127.0.0.1:7110`.
- Player Web is ready at `http://127.0.0.1:3010` for end-to-end mail/grant visibility checks.
- Production gaps remain: real OIDC/session auth, real multi-operator approval policy, rate limits, support-case workflows, and broader GM executor coverage.

## 2026-04-28-R246

Goal: fix the reported local backend smoke gap where a successful Admin gold grant did not light up mail in an already-online player frontend.

Coordinator local work:

- Reproduced the direct Admin API approval -> grant gold path and confirmed `gateway_live` delivery into the account store.
- Identified the missing online-session bridge: `POST /admin/system-mail` updated the shared account store, but an already-running `GatewaySession` kept its stale Stage 5 mail state and could overwrite the delivered mail on keepalive/save before the player reloaded.
- Added `SimulationSession::refresh_active_external_mail()` and `GatewaySession::refresh_active_external_mail()` to merge externally delivered Stage 5 mail into the active session.
- Gateway Web now refreshes external mail before initial snapshots, before each action save, and before disconnect save. If the refresh changed state, it sends a world snapshot, so the player UI sees the mail on the next keepalive/tick.
- Restarted the local Gateway on `127.0.0.1:7110` / `127.0.0.1:7000` with the fix.

Verification:

- `cargo +1.89.0 test --locked -p mir2-simulation online_session_refreshes_admin_delivered_mail_before_save -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-gateway session_cache -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-gateway admin_system_mail_endpoint_writes_live_account_store -- --test-threads=1`
- Playwright browser smoke: Player Web Quick Enter -> Admin API approval/grant `888` gold -> Player Web Mail panel shows `GM Currency Grant`, `Operator local-gm granted 888 gold.`, and `888 Gold · Unclaimed`.

Outcome:

- Backend gold/mail grants now become visible to an already-online local player session without requiring logout/relogin.

## 2026-04-28-R247

Goal: fix the reported Admin Web system-mail submit dead path and add complete post-submit status loading for the local backend smoke flow.

Coordinator local work:

- Identified the Admin Web submit issue as an unstable client-only/hydration path in local dev, then moved the system-mail form to a Next server action with `useFormStatus` pending UI.
- Added `GET /admin/commands/:command_id/status` to the Rust Admin API so the UI can load one command's status directly instead of inferring from a broad command list.
- Updated GM Tools to render a post-submit command status card for `commandId`, including status, result, trace id, operator, and matching `mail-{commandId}` outbox delivery receipt.
- Restarted Admin API on `127.0.0.1:7420` and Admin Web on `http://127.0.0.1:3020` with the new code.

Verification:

- `apps/admin-web ./node_modules/.bin/tsc --noEmit`
- `cargo +1.89.0 test --locked -p mir2-admin-api get_command_status_returns_one_command_record -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-simulation online_session_refreshes_admin_delivered_mail_before_save -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-gateway admin_system_mail_endpoint_writes_live_account_store -- --test-threads=1`
- Playwright browser smoke: Admin Web `/gm-tools` -> `Queue System Mail` -> `/gm-tools?commandId=cmd-mail-1777313734223` shows `succeeded`, `system mail queued as mail-cmd-mail-1777313734223`, `gateway_live / 1`, and mail id `4`; Player Web Quick Enter -> Mail shows `Compensation Package`, `5000 Gold · Unclaimed`, and Claim/Delete controls.

Outcome:

- Local backend testing now has visible submit, result, and delivery states for system mail. The player-facing mail panel updates with the delivered gold mail while the player remains online.

## 2026-04-28-R250

Goal: replace the remaining Admin Web real-data gaps for activity config, market prices, and trade graph with Postgres-backed projections.

Coordinator local work:

- Added `admin_activities`, `admin_market_price_feeds`, and `admin_trade_graph_edges` to the core Postgres migration.
- Added Admin API write routes for `/admin/activities`, `/admin/economy/price-feeds`, and `/admin/risk/trade-edges`, all gated by `content_publish`.
- Changed `/admin/read/activities`, `/admin/read/economy`, and `/admin/read/risk` to read those Postgres projections when `ADMIN_DATABASE_URL` is configured.
- Added Admin Web server-action forms on Activities, Economy, and Risk so operators can write and then immediately read real records.

Verification:

- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`
- `apps/admin-web ./node_modules/.bin/tsc --noEmit`
- Live API smoke wrote and read back `activity-r250-smoke`, `GoldBar`, and `trade-r250-smoke`.
- Admin Web `/activities`, `/economy`, and `/risk` returned HTTP 200.

Outcome:

- Activity config, market price feeds, and trade graph no longer rely on mock or unwired empty states in the local Postgres-backed admin system. Deeper zone process telemetry remains the main Admin read-model gap.

## 2026-04-28-R251-R253

Goal: finish the remaining local Admin real-data route from zone telemetry through operator/RBAC records and page-level QA.

Coordinator local work:

- Added `admin_zone_runtime_records` to the Postgres migration.
- Added `POST /admin/servers/zones` and changed `/admin/read/servers` to include Postgres zone runtime records alongside Gateway session presence.
- Updated Admin Web Servers with a zone runtime table and server-action form.
- Added `admin_operators` to the Postgres migration.
- Added `/admin/read/operators`, `POST /admin/operators`, and a new Admin Web `/operators` page with RBAC record creation.
- Added Operators to the Admin Web navigation and local dev `permission_manage` default for RBAC smoke.

Verification:

- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`
- `apps/admin-web ./node_modules/.bin/tsc --noEmit`
- Live API smoke wrote and read back `zone-r251-smoke` and `ops-r252-smoke`.
- Admin Web HTTP smoke returned 200 for `/`, `/players`, `/economy`, `/activities`, `/servers`, `/risk`, `/gm-tools`, `/approvals`, `/operators`, `/timeline`, and `/audit`.
- `cargo +1.89.0 fmt --check`
- `git diff --check`

Outcome:

- The local Admin backend now has real Postgres data for dashboard/player/economy/activity/server-zone/risk/operator/audit/timeline surfaces. Remaining production gaps are external identity provider/session auth, production multi-approver policy, broader support workflows, and additional GM executors.

## 2026-04-28-R254-R256

Goal: replace the remaining local operator-header/auth scaffolding with a real Postgres-backed operator-token path, tighten high-risk approval semantics, and make Gateway runtime telemetry write itself.

Coordinator local work:

- Added `ADMIN_OPERATOR_AUTH_BACKEND=postgres` to Admin API. In this mode `Authorization: Bearer <token>` is resolved from `admin_operators.token_hash`, `last_authenticated_at_ms` is updated, and caller-supplied identity headers are ignored.
- Added `GET /admin/auth/me` and wired Admin Web shell/login/logout around an `admin_operator_token` httpOnly cookie with `ADMIN_OPERATOR_TOKEN` as the local env fallback.
- Extended operator writes so `POST /admin/operators` can create or rotate local operator tokens without returning the secret.
- Hardened high-risk command submission so `approvalId` must point at an approved record for the same command id, command type, and requesting operator. A different deciding operator is required unless the local self-approval override is explicitly set.
- Updated Admin Web Approvals to show requester/decider fields and hide approve/reject actions for the current requesting operator.
- Added Gateway zone runtime heartbeat configuration. When `ADMIN_API_BASE_URL` and `MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN` are set, Gateway periodically posts a `gateway_heartbeat` record to `/admin/servers/zones`.
- Seeded local Postgres operators for smoke coverage: a lead operator token, a peer approver token, and a runtime service token for Gateway heartbeat.

Verification:

- `cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1`
- `cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1`
- `apps/admin-web ./node_modules/.bin/tsc --noEmit`
- Live auth smoke: unauthenticated `/admin/auth/me` returned 401, and bearer `r254-lead-token` resolved to `ops-r254-lead` from `postgres_admin_operators`.
- Live operators smoke: `/admin/read/operators` returned real Postgres operators with `tokenConfigured: true`.
- Live approval smoke: `ops-r254-lead` requested approval, `ops-r254-peer` approved it, and `ops-r254-lead` successfully submitted `grant_currency` with the matching approval id.
- Live heartbeat smoke: Gateway wrote `gateway-r254-live` with source `gateway_heartbeat`, and `/admin/read/servers` returned it.
- Admin Web HTTP smoke returned 200 for `/login`, `/operators`, `/approvals`, and `/servers`, with resolved operator identity visible in the top bar.

Outcome:

- The local Admin stack now uses real Postgres operator tokens, peer approval semantics, and automatic Gateway telemetry for the browser-testable backend. Remaining production work is external IdP/session auth, richer approval workflows, support-case tooling, rate limits, deployment hardening, and broader GM executor coverage.
