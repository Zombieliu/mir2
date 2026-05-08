# Architecture Adoption Plan

Last updated: 2026-05-05

Purpose: define what parts of the target MMORPG architecture should be added now, what should be introduced behind interfaces, and what should remain documented until scale or product need justifies it.

This plan complements:

- `docs/TECH-MODERNIZATION-RFC.md`
- `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`
- `docs/POST-1TO1-EVOLUTION-PLAN.md`
- `docs/PARITY-TRUTH-AUDIT.md`
- `docs/ARCHITECTURE-IMPLEMENTATION-STATUS.md`

## Guiding Rule

Add boundaries before adding infrastructure. The project should become cloud-ready without becoming infrastructure-heavy while the game design and persistence model are still changing.

## Add Now

| Area | Decision | Why Now | First Repo Change |
| --- | --- | --- | --- |
| Client split | Keep Bevy + NextJS | This is already the project direction and fits Web plus future desktop/mobile shells | Continue separating Bevy runtime rendering from React/Next panels and account flows |
| Gateway | Keep Rust gateway | High-concurrency long connections and protocol adaptation are core | Preserve `apps/gateway`; add routing/session abstractions before physical service split |
| Game authority | Keep Rust simulation/world | Current Rust simulation has the strongest tests and parity evidence | Keep `apps/simulation` authoritative; do not move gameplay mutation into Admin Web or Next routes |
| Postgres | Add as the first authoritative DB target | Needed for accounts, characters, inventory, mail, audit, admin commands, and future operations | Add schema/migration plan and storage traits before replacing JSON store |
| Redis | Add as non-authoritative cache/session/routing layer | Needed for online presence, session cache, rate limit, route cache, locks | Define cache contract and invalidation rules before code depends on Redis |
| NATS | Add as early internal command/notification bus candidate | Fits lightweight GM command dispatch, service notifications, online/offline fanout | Add local dev service and command-bus abstraction; avoid making it the source of truth |
| Redpanda + ClickHouse | Add local event-stream and analytics stack | Gives admin/gameplay events an append-only path and a queryable projection target without making it gameplay authority | Compose services, admin event projections, gameplay command outcome projection, architecture-gated gameplay event schema compatibility, and readiness alert thresholds are in place |
| Admin API | Keep Rust Axum for now | It already exists, shares domain types, and avoids a second backend stack too early | Continue building typed command/audit/repository layers in `apps/admin-api` |
| Admin Web | Keep NextJS | Fastest path for high-quality operations UI | Continue productionizing real control-plane data; dashboard/player/economy/activity/risk/server/operator reads are now Rust-backed, the dashboard surfaces gameplay-event readiness from ClickHouse-backed Admin API reads, server zone telemetry has a Postgres/Gateway-heartbeat path, and local operator auth can use Postgres bearer tokens |
| Docker Compose | Add local dev infra | Lets Windows/Mac run the same Postgres/Redis/NATS/Redpanda/ClickHouse baseline | Add `infra/docker-compose.dev.yml` with optional search/observability profiles |
| Observability contract | Document metrics/log/tracing now | Easy to wire later if route names, request ids, and command ids are consistent from the start | Use trace ids and structured logs in new service boundaries |

## Add Behind Interfaces, Not Full Production Yet

| Area | What To Do Now | What Not To Do Yet |
| --- | --- | --- |
| gRPC + Protobuf | Define service contracts for account, character, admin command, zone routing, and mail after storage boundaries are stable | Do not split every module into a network service before in-process boundaries are clean |
| Redpanda producers | Use app-level producers behind event-sink interfaces and keep delivery non-authoritative | Do not make event publish part of authoritative gameplay transactions yet |
| ClickHouse dashboards | Expand schemas for economy, inventory, mail, trade, auction, login, command audit after real producers exist | Gameplay command readiness is now visible in Admin Web; broader analytics dashboards should still wait for richer event quality |
| Meilisearch | Plan search indexes for player, account, item, mail, auction, order, support notes | Do not make it required until real read models exist |
| Loki + Grafana | Add optional local profile and structured log conventions | Do not block local gameplay on observability stack startup |
| BullMQ / JetStream / Temporal | Start with command outbox and NATS candidate | Do not introduce Temporal until workflows are complex enough to need it |
| KCP / QUIC | Keep protocol direction documented | Do not implement before WebSocket/TCP service routing and packet contracts settle |

## Defer

| Area | Defer Until | Reason |
| --- | --- | --- |
| Kubernetes | Multiple services and load targets exist | Docker Compose is enough before service count and ops needs justify K8s |
| Flink | Real-time risk/economy rules outgrow simple stream workers | Early anti-cheat can run as service workers over event streams |
| Spark / Data Lake | Large-scale historical analytics or 1M DAU-level data volume | ClickHouse plus object storage is enough first |
| OpenSearch | Loki/Grafana and ClickHouse cannot answer support/log-search needs | Avoid running two heavy log/search systems early |
| Sui Move | Product has a narrow asset-ownership reason | Do not put ordinary game state, NPC scripts, quests, or gold on-chain |
| Go / NestJS admin backend | Team composition demands it | Rust Axum already exists and shares domain types; avoid adding another backend stack now |

## Target Runtime Shape

```text
Client
  Web: NextJS + Bevy WASM
  Desktop later: Tauri shell around Web/Bevy, or native Bevy escape hatch
        |
        | WebSocket now; TCP compatibility; KCP/QUIC later if needed
        v
Gateway (Rust)
  auth handoff, session lifecycle, rate limit, protocol adaptation, route lookup
        |
        | in-process now, gRPC/protobuf later where service split is justified
        v
World / Zone Runtime (Rust)
  authoritative map, combat, AI, NPC, items, drops, snapshots/deltas
        |
        +--> Postgres authoritative persistence
        +--> Redis online/session/routing/cache/locks
        +--> NATS command/service notifications
        +--> Redpanda gameplay/economy/audit event stream
```

Admin side:

```text
Admin Web (NextJS)
        |
Admin API (Rust Axum)
        |
Command/Audit Repository + RBAC + Approval
        |
Command Bus / Outbox
        |
Gateway / Account / World / Mail service boundaries
```

## First Implementation Sequence

1. Keep parity baseline green and truthfully marked as Candidate where appropriate.
2. Add local dev infra compose for Postgres, Redis, NATS, Redpanda, and ClickHouse.
3. Define runtime/storage boundaries for world commands, account, character, inventory, storage, mail, and admin audit. The first runtime boundary now exists as `WorldRuntime` / `WorldCommand` with an in-process runtime adapter; `WorldCommandOutcome` / `WorldCommandExecution` expose typed command results; gateway sessions now open through `ZoneRegistry`; `SessionRouter` has both the default single-zone policy and `MapZoneSessionRouter`; and the shared in-process runtime factory isolates state per `ZoneId`. The default in-process zone shares player presence plus per-map NPC/monster/drop snapshot layers across same-zone sessions. Shared ground-drop removal is tombstoned at the zone layer for both high-level and protocol pickup paths to prevent stale per-session snapshots from resurrecting picked-up drops; removed non-player map entity ids are tombstoned the same way. This is still transitional shared state; combat, AI, remote pickup inventory gain, NPC services, and world ticks still need promotion into true shared zone authority.
4. Add Postgres schema draft and migration strategy. Done for the first core
   schema in `infra/postgres/migrations/0001_core.sql`.
5. Implement Postgres repositories behind existing local interfaces. Done for
   Admin command/audit records and the admin outbox. Gameplay account state
   still defaults to JSON, but now has both an import bridge and an optional
   Postgres mirror through `MIR2_ACCOUNT_STORE_DATABASE_URL`; it also has an
   explicit opt-in source-of-truth mode through
   `MIR2_ACCOUNT_STORE_BACKEND=postgres`, with row locks and version increments.
6. Add Redis session/online-state cache with tests proving cache miss/hit equivalence. Started with a gateway `GatewaySessionCache` contract, deterministic in-memory online-session records, and an optional Redis adapter with TTL tests. Redis now stores `zoneId` and `updatedAtMs`, exposes `route_character`, can derive a `SessionRouteRequest` for cached online-character reconnect/routing, rejects stale routes through `fresh_route_request_for_character`, and can remove stale routes outside Redis TTL behavior. Redis also stores a character-name routing index for Admin kick-player removal. Web Gateway now refreshes routes through per-character route leases and removes only owned routes on disconnect, preventing stale sockets from erasing newer online routes. Gateway can publish zone heartbeat records to the Admin API; cross-zone route-transfer semantics remain next.
7. Add NATS command bus dispatcher for admin command dispatch, while keeping command/audit state in Postgres. `dispatch-admin-outbox` now supports core NATS and JetStream publish-ack modes, plus retry/dead-letter lifecycle events.
8. Define event envelope schemas before wiring app-level Redpanda producers into runtime paths. Local Redpanda and ClickHouse now exist in Compose; Admin outbox events now use a stable envelope, publish to Redpanda through Pandaproxy when configured, track NATS/Redpanda delivery state independently, and project terminal command outcomes, approval lifecycle, and outbox lifecycle events into ClickHouse `admin_events` plus `admin_command_events`. Gameplay runtime now has a `GatewayGameplayEvent` envelope, optional `GameplayEventSink` boundary for command outcomes, Gateway startup wiring for Redpanda/Pandaproxy, `/health` sink metrics, a ClickHouse `gameplay_events` projection, Admin API `/admin/gameplay-events` read access, `/admin/gameplay-events/summary` lag/volume readiness with `maxLagSeconds` / `minEvents` alert thresholds, an Admin Web dashboard panel for that summary, and `infra/check-architecture-gates.sh` coverage that locks Gateway event JSON fields to the ClickHouse Kafka/materialized-view columns. Gameplay events remain analytics-only; they do not participate in authoritative transaction commit.
9. Move account storage behind repositories before normalizing gameplay tables. `mir2-simulation` now has `AccountStoreRepository`, `FileAccountStoreRepository`, and `PostgresAccountStoreRepository`; `SimulationConfig` load/save paths use those adapters while preserving JSON, mirror, and Postgres source-of-truth modes.
9. Add broader ClickHouse/Meilisearch projections only after real event/read-model data exists.

## Current Stack Answer

Add now:

- Postgres as target authoritative store.
- Redis as non-authoritative cache/session/routing.
- NATS as command/notification bus candidate.
- Redpanda plus ClickHouse as local event-stream and analytics infrastructure.
- Docker Compose dev infra.
- Storage, cache, command-bus, event-envelope interfaces.
- Admin API in Rust Axum, not Go/NestJS yet.

Document now, optional profiles only:

- Meilisearch.
- Loki + Grafana.

Defer:

- KCP/QUIC implementation.
- Kubernetes.
- Temporal.
- Flink.
- Spark/Data Lake.
- OpenSearch.
- Sui Move gameplay integration.
