# mir2-web3

Mir2/Crystal-compatible MMORPG foundation evolving into a modern custom MMORPG stack.

## Layout

- `apps/web`
  - Next.js app for wallet login, account portal, marketplace, admin tools.
- `apps/game-client`
  - Bevy WASM game client.
- `apps/gateway`
  - Rust gateway for auth, sessions, websocket traffic, Sui integration.
- `apps/simulation`
  - Bevy headless or `bevy_ecs` authority simulation.
- `packages/protocol`
  - Shared protocol definitions and generated artifacts.
- `packages/game-data`
  - Exported data converted from Crystal resources/configs.
- `packages/tooling`
  - Importers, converters, generators, and migration scripts.
- `docs`
  - Architecture notes, migration plan, and milestones.
- `infra`
  - Optional local Postgres, Redis, NATS, Redpanda, ClickHouse, Meilisearch, Loki, and Grafana development services.

## Source Of Truth

The existing Crystal project remains the reference implementation for:

- gameplay rules
- packet flow
- map and asset formats
- server-side data behavior

The new project should not modify Crystal directly. Use Crystal as a reference and migration source.

## Current Architecture Direction

Primary references:

- `docs/ARCHITECTURE-CURRENT.md`
- `docs/PARITY-TRUTH-AUDIT.md`
- `docs/TECH-MODERNIZATION-RFC.md`
- `docs/ARCHITECTURE-ADOPTION-PLAN.md`
- `docs/POST-1TO1-EVOLUTION-PLAN.md`
- `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`
- `docs/ADMIN-STAGING-RUNBOOK.md`
- `docs/WINDOWS-HOME-STAGING-SERVER.md`

Immediate product architecture additions:

- Rust gateway and Rust authoritative simulation stay as the core.
- NextJS + Bevy remain the client direction.
- Postgres is the authoritative database target.
- Redis is the non-authoritative cache/session/routing target.
- NATS is the early internal command/notification bus candidate.
- Redpanda and ClickHouse back the current Admin event analytics projection.
- Meilisearch, Loki, and Grafana remain optional local profiles.

Start core local infrastructure:

```bash
docker compose -f infra/docker-compose.dev.yml up -d postgres redis nats redpanda clickhouse
```

## Legacy MVP Goal

Phase 1 should only target:

1. wallet/account binding
2. character selection
3. map entry
4. movement
5. chat
6. basic entity visibility

## Next Steps

Current implemented checkpoint:

1. `packages/protocol` now has typed packet support for login/select/start-game, `MapInformation`, `UserInformation`, movement, chat, `ObjectPlayer`, `NewMonsterInfo`, and `NewNpcInfo`.
2. `apps/simulation` emits a deterministic bootstrap scene with the player, one remote player, one monster, and one NPC for local testing.
3. `apps/gateway` exposes TCP, HTTP health, WebSocket bridge, browser manual smoke UI, and a TCP smoke binary.
4. `apps/admin-api` exposes an Axum Admin API with Postgres-backed operator auth, audited GM commands, peer approval, live gateway delivery, persistent receipts, Postgres command/audit/outbox adapters, and Redpanda/ClickHouse event reads.
5. `apps/admin-web` is a separate NextJS operations console with login, dashboard, players, economy, activities, servers, risk, GM tools, approvals, operators, audit, timeline, and English/Simplified Chinese UI copy.
6. `apps/simulation`, `apps/gateway`, and `apps/admin-api` share the same account-store environment policy: local file store by default, Postgres source of truth when explicitly selected or when the runtime is production/staging.
7. `apps/web` supports password, Sui Passkey, and Sui wallet login through the Gateway WebSocket flow, and can be pointed at a staging Gateway through `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` while preserving the local default gateway websocket.
8. `scripts/quality-gate.sh` provides a lightweight repo gate for the current Rust/Web engineering boundary.
9. `apps/web` has game-grade cache instrumentation for static Crystal assets,
   scene blueprints, critical prewarm packs, cold/warm cache smoke, and real
   first-playable timing through `smoke:cache-metrics` and
   `smoke:playable-metrics`.

Immediate next steps:

1. Deploy a shared staging stack using `infra/staging.env.example`.
2. Run `docs/ADMIN-STAGING-RUNBOOK.md` smoke checks.
3. Close production blockers before marking the operations center production-grade.
