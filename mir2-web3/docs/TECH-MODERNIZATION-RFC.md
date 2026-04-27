# Technical Modernization RFC

Last updated: 2026-04-26

Status: discussion draft.

Purpose: capture the technical modernization direction discussed after the Crystal / Mir2 1:1 Candidate baseline. This is not an implementation plan yet; it records architecture principles, confirmed decisions, recommended boundaries, and open questions before large code changes.

Architecture adoption plan: `docs/ARCHITECTURE-ADOPTION-PLAN.md` defines what to add now, what to add behind interfaces, and what to defer.

Platform strategy: `docs/PLATFORM-CLIENT-STRATEGY.md` captures the current Web, Tauri desktop, mobile, and console stance.

Admin operations strategy: `docs/ADMIN-OPERATIONS-ARCHITECTURE.md` captures the control-plane, RBAC, audit, and admin command architecture.

## Context

The project currently has a strong Mir2-style MMORPG foundation:

- Rust simulation/runtime has broad deterministic regression coverage.
- Gateway supports browser HTTP/WebSocket plus Crystal-framed TCP surfaces.
- Frontend has automated Stage 5 smoke screenshots and route coverage.
- Crystal parity docs and packet trace harnesses provide a golden compatibility baseline.

The next product direction is not pure Crystal 1:1. The goal is to use this foundation to build a modern custom MMORPG with cloud production, global service boundaries, stronger persistence, cache, operations tooling, and a new content authoring model.

## First Principles

The architecture should optimize for:

- many players in a persistent shared world;
- low-latency gameplay loops;
- strong consistency for valuable state;
- explicit authority boundaries;
- safe operations and auditability;
- content iteration speed;
- cloud production and horizontal growth.

Core rules:

- Rust server-side simulation remains the authority for gameplay state.
- Postgres stores long-lived authoritative state.
- Redis stores short-lived, derived, routing, cache, and queue state.
- World/zone processes run high-frequency simulation in memory.
- Frontend, admin tools, cache, and chain integrations must not directly mutate authoritative gameplay state.
- Content logic must be validated and compiled into a restricted runtime representation, not executed as arbitrary scripts.
- Crystal 1:1 evidence remains a compatibility baseline, not the future product ceiling.

## Confirmed Product Direction

- Keep the current Rust simulation/gateway foundation first; do not rewrite from scratch.
- Database target: Postgres.
- Cache/session target: Redis.
- Global architecture target: perceived global single world, implemented as global account/economy/social services plus distributed zone/channel runtime.
- Frontend target: Bevy + NextJS full-end adaptation.
- Platform target: Web first, Windows/macOS through a near-term Tauri shell, iOS/Android after validation, consoles deferred to a separate strategy.
- Admin/operations: a dedicated audited operations backend is required; see `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`.
- NPC and quest scripting: move toward a new developer-oriented DSL compiled to Rust IR.
- Sui Move: not the main NPC/quest DSL; possible future boundary for rare assets or trusted asset movement only.
- Cloud production and horizontal scaling are priority goals.

## Target Architecture

### Frontend

NextJS should own:

- login;
- account flows;
- character selection and creation;
- shell routing;
- activity/payment/account pages;
- React overlay panels;
- operations/admin frontend when appropriate.

Bevy should own:

- game rendering;
- scene/camera;
- movement and targeting input;
- animation and visual effects;
- map/entity presentation.

React overlay can own:

- inventory;
- character panels;
- storage;
- chat;
- NPC dialogs;
- mail;
- shop/auction;
- system menus.

Frontend must not be authoritative for item, gold, combat, quest, or persistence state.

### Backend

Gateway should own:

- WebSocket/TCP connection entry;
- authentication handoff;
- session lifecycle;
- routing to the correct world/zone;
- protocol adaptation;
- backpressure and disconnect handling.

World/zone service should own:

- map runtime;
- monster AI;
- combat and skill resolution;
- NPC interaction execution;
- item/drop logic;
- authoritative world snapshots and deltas;
- save checkpoints through persistence adapters.

Global services should own:

- account identity;
- character index;
- mail;
- auction/economy surfaces;
- guild/social;
- global chat/channel metadata;
- admin command intake;
- cross-zone routing.

The initial implementation can remain one process while boundaries are introduced. The code should evolve toward separable modules before physical service splitting.

### Persistence

Postgres should become the authoritative long-lived store for:

- accounts;
- credentials or external auth identity references;
- characters;
- inventory;
- equipment;
- storage;
- mail;
- guilds;
- auctions;
- NPC flags and quest state;
- world-event state;
- admin audit logs.

Current JSON/local account store should be preserved long enough to support migration tests and deterministic local fixtures.

### Cache And Messaging

Redis should be used for:

- sessions;
- online presence;
- gateway routing;
- short-lived character snapshot cache;
- map metadata cache;
- content manifest cache;
- rate limits;
- distributed locks where needed;
- job queues or stream-like workflows if the system does not introduce a separate event bus yet.

Redis must not be the source of truth for valuable gameplay state. Cache misses and cache hits must produce the same gameplay-visible state.

### Operations Backend

The admin/operations backend should support:

- account lookup;
- character lookup;
- ban, unban, and kick;
- grant item;
- grant currency;
- send mail;
- inspect inventory/storage/mail/auction state;
- inspect online users and map populations;
- publish content/config changes;
- audit every GM operation;
- role-based access control.

Admin tools must not directly mutate production tables without going through audited commands or approved migration paths.

## Global Single-World Model

The user-facing goal is global single-world behavior:

- shared account identity;
- shared economy;
- shared social/guild systems;
- shared auction/trade/mail surfaces;
- global events;
- ability to interact across regions where product rules allow.

The technical model should not force every map into one process. Recommended implementation:

- global services coordinate identity, economy, social, and routing;
- maps run in zone/channel processes;
- hot maps can split into instances or channels;
- players transfer between zones through explicit handoff;
- global systems publish events to zones rather than sharing mutable in-memory state.

## NPC And Quest DSL

### Decision

Use a small custom developer-oriented DSL or similarly constrained content format that compiles to a Rust IR. Do not use TypeScript, Lua, or Sui Move as the authoritative runtime script language.

### Rationale

NPC/quest content needs:

- static validation;
- deterministic execution;
- no arbitrary IO;
- no unbounded loops;
- explicit side effects;
- testability;
- compatibility import from Crystal scripts where useful;
- future editor/admin integration.

General-purpose languages are too permissive for this boundary. TypeScript can be useful for tooling, editor UI, validation CLI, and generated type bindings, but not as the authoritative server-side content runtime.

### Example Direction

```text
npc bichon_guide {
  state main {
    say "Welcome."

    option "Start training" when quest(starter_training).not_started {
      quest.start starter_training
      say "Defeat 3 wasps."
    }

    option "Open storage" {
      service.open storage
    }
  }
}
```

The DSL should compile into a restricted IR such as:

```text
Say(text)
Option(text, condition, target)
StartQuest(id)
GiveItem(item, count)
TakeGold(amount)
OpenService(kind)
SetFlag(key, value)
```

Runtime executes validated IR only.

### Sui Move Boundary

Sui Move should not drive NPC dialogs, ordinary quests, combat, or normal game-loop state.

Possible future Sui/chain boundaries:

- rare asset ownership;
- externally auditable player-to-player asset movement;
- special cosmetic or title ownership;
- land/territory proofs if the product needs them;
- cross-product identity or asset bridges.

Ordinary inventory, normal gold, NPC state, monster drops, and quest progression should stay in Postgres/Rust unless there is a clear product reason to move a narrow slice on-chain.

## Real-Time Protocol Direction

Short term:

- keep WebSocket for browser/game clients;
- keep Crystal TCP trace harness for compatibility and regression evidence;
- keep typed Rust commands/events internally;
- continue snapshot and smoke coverage.

Medium term:

- move from broad snapshots toward command plus delta events where useful;
- define typed event schemas;
- consider binary encoding only after behavior stabilizes;
- keep JSON/debug surfaces for tests and admin tools.

Long term:

- edge gateway routes players to zone services;
- internal event/messaging layer may use Redis Streams first, or a dedicated bus such as NATS if scale/operational complexity justifies it;
- packet trace and replay remain regression tools.

## Modernization Phases

### Phase 0: Preserve Golden Baseline

- Keep current Candidate evidence and parity docs.
- Do not erase 1:1 metrics.
- Keep Stage 5 smoke, packet traces, and Rust regression suites green.

### Phase 1: Architecture RFC And Boundaries

- Finalize this RFC.
- Decide persistence adapter boundary.
- Decide service boundary names and module ownership.
- Decide NPC DSL syntax direction.

### Phase 2: Persistence Adapter

- Introduce storage traits/interfaces without changing storage backend first.
- Keep JSON/local implementation as reference.
- Add migration test scaffolding.

### Phase 3: Postgres MVP

- Add Postgres schema for accounts, characters, inventory, equipment, storage, mail, NPC flags, and audit logs.
- Implement Postgres adapter behind the storage boundary.
- Keep deterministic tests and smoke flows green.

### Phase 4: Redis MVP

- Add session and online-state cache.
- Add routing/cache primitives.
- Prove cache invalidation for inventory/storage/mail/NPC flags.
- Keep Redis non-authoritative.

### Phase 5: Admin API And Operations Console

- Build audited Admin API before direct admin UI mutation.
- Add GM operation audit table.
- Implement account lookup, character lookup, ban/unban/kick, grant item, send mail, and inspect inventory/storage.

### Phase 6: NPC DSL And IR

- Design parser, AST, validator, compiler, and runtime IR.
- Keep Crystal compatibility tests or split them explicitly.
- Add product-script fixtures and runtime tests.

### Phase 7: Login/Select Product Redesign

- Redesign NextJS login/select UI.
- Update Stage 5 login/select smoke to the product target.
- Mark Crystal visual differences as accepted divergence where intentional.

### Phase 8: Zone Architecture

- Introduce logical zone/channel routing in-process first.
- Add transfer/handoff tests.
- Split physical services only after boundaries are stable.

### Phase 9: Cloud Production

- Containerize services.
- Add environment-specific config.
- Add observability: metrics, logs, traces, health, readiness.
- Add backup/restore and migration procedures.
- Add load tests for gateway, zone, Postgres, and Redis.

## Expected Change Size

The modernization is large, but it should not be a rewrite.

Low-to-medium impact:

- login/select UI redesign;
- admin frontend shell;
- docs and product specs;
- content tooling prototypes.

Medium impact:

- persistence adapter;
- Postgres schema and migration;
- Redis session/cache;
- Admin API;
- content compiler pipeline.

High impact:

- inventory/equipment/storage/mail persistence migration;
- NPC DSL and quest execution;
- zone/channel routing;
- cloud production deployment and operational hardening.

Highest-risk areas:

- item unique IDs;
- character IDs;
- save schema migration;
- storage/equipment consistency;
- mail/auction/trade economy flows;
- NPC saved flags;
- login/session security.

## Open Questions

- What exact Postgres schema versioning/migration tool should be used?
- Should persistence use a repository trait per aggregate or one account-store-like adapter first?
- Should Redis Streams be enough for early async operations, or should NATS be introduced later?
- Should the NPC DSL use custom `.npc` syntax, RON, or another structured format?
- How much Crystal script compatibility should remain after the product DSL exists?
- Should the admin frontend live inside the existing Next app or a separate app package?
- What is the first production deployment target?
- What is the first load target for concurrent users per zone and globally?

## Current Recommendation

Start with architecture and persistence boundaries, not UI or NPC parser implementation.

Recommended immediate sequence:

1. Finalize this RFC.
2. Follow `docs/ARCHITECTURE-ADOPTION-PLAN.md`: add local Postgres/Redis/NATS dev infra and keep Redpanda/ClickHouse/Meilisearch/Loki optional.
3. Draft persistence adapter and Postgres schema.
4. Add migration strategy from current JSON store.
5. Add Redis scope and invalidation rules.
6. Draft NPC DSL syntax and IR, but do not implement it until persistence boundaries are stable.
