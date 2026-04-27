# Architecture Adoption Plan

Last updated: 2026-04-27

Purpose: define what parts of the target MMORPG architecture should be added now, what should be introduced behind interfaces, and what should remain documented until scale or product need justifies it.

This plan complements:

- `docs/TECH-MODERNIZATION-RFC.md`
- `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`
- `docs/POST-1TO1-EVOLUTION-PLAN.md`
- `docs/PARITY-TRUTH-AUDIT.md`

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
| Admin API | Keep Rust Axum for now | It already exists, shares domain types, and avoids a second backend stack too early | Continue building typed command/audit/repository layers in `apps/admin-api` |
| Admin Web | Keep NextJS | Fastest path for high-quality operations UI | Add real read models gradually; current mock data must stay marked as mock |
| Docker Compose | Add local dev infra | Lets Windows/Mac run the same Postgres/Redis/NATS baseline | Add `infra/docker-compose.dev.yml` with optional profiles |
| Observability contract | Document metrics/log/tracing now | Easy to wire later if route names, request ids, and command ids are consistent from the start | Use trace ids and structured logs in new service boundaries |

## Add Behind Interfaces, Not Full Production Yet

| Area | What To Do Now | What Not To Do Yet |
| --- | --- | --- |
| gRPC + Protobuf | Define service contracts for account, character, admin command, zone routing, and mail after storage boundaries are stable | Do not split every module into a network service before in-process boundaries are clean |
| Redpanda | Keep it as the event-stream target for gameplay/economy/audit events | Do not require Redpanda for core gameplay startup yet |
| ClickHouse | Define event schemas for economy, inventory, mail, trade, auction, login, command audit | Do not build analytics dashboards before event quality is reliable |
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
2. Add local dev infra compose for Postgres, Redis, and NATS.
3. Define storage boundaries for account, character, inventory, storage, mail, and admin audit.
4. Add Postgres schema draft and migration strategy. Done for the first core
   schema in `infra/postgres/migrations/0001_core.sql`.
5. Implement Postgres repositories behind existing local interfaces. Done for
   Admin command/audit records and the admin outbox. Gameplay account state
   still defaults to JSON, but now has both an import bridge and an optional
   Postgres mirror through `MIR2_ACCOUNT_STORE_DATABASE_URL`; it also has an
   explicit opt-in source-of-truth mode through
   `MIR2_ACCOUNT_STORE_BACKEND=postgres`, with row locks and version increments.
6. Add Redis session/online-state cache with tests proving cache miss/hit equivalence.
7. Add NATS command bus dispatcher for admin command dispatch, while keeping command/audit state in Postgres. First minimal dispatcher exists as `dispatch-admin-outbox`; production still needs JetStream retry/dead-letter semantics.
8. Define event envelope schemas before introducing Redpanda as a required runtime dependency.
9. Add ClickHouse/Meilisearch projections only after real event/read-model data exists.

## Current Stack Answer

Add now:

- Postgres as target authoritative store.
- Redis as non-authoritative cache/session/routing.
- NATS as command/notification bus candidate.
- Docker Compose dev infra.
- Storage, cache, command-bus, event-envelope interfaces.
- Admin API in Rust Axum, not Go/NestJS yet.

Document now, optional profiles only:

- Redpanda.
- ClickHouse.
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
