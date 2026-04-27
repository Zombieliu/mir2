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

- `docs/PARITY-TRUTH-AUDIT.md`
- `docs/TECH-MODERNIZATION-RFC.md`
- `docs/ARCHITECTURE-ADOPTION-PLAN.md`
- `docs/POST-1TO1-EVOLUTION-PLAN.md`
- `docs/ADMIN-OPERATIONS-ARCHITECTURE.md`

Immediate product architecture additions:

- Rust gateway and Rust authoritative simulation stay as the core.
- NextJS + Bevy remain the client direction.
- Postgres is the authoritative database target.
- Redis is the non-authoritative cache/session/routing target.
- NATS is the early internal command/notification bus candidate.
- Redpanda, ClickHouse, Meilisearch, Loki, and Grafana are optional local profiles until real adapters/projections exist.

Start core local infrastructure:

```bash
docker compose -f infra/docker-compose.dev.yml up -d postgres redis nats
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
4. `apps/admin-api` exposes an Axum Admin API with audited `SendSystemMail`, live gateway delivery, account-store fallback, Postgres command/audit/outbox adapters, a JSON account-store import utility, and NATS outbox dispatch.
5. `apps/admin-web` is a separate NextJS operations console with Dashboard, players, economy, servers, risk, GM tools, audit, and English/Simplified Chinese UI copy.
6. `apps/simulation` can mirror JSON account-store saves into Postgres when `MIR2_ACCOUNT_STORE_DATABASE_URL` is configured, and can explicitly opt into Postgres source-of-truth mode with `MIR2_ACCOUNT_STORE_BACKEND=postgres`. JSON remains the default runtime source of truth.

Immediate next steps:

1. Replace the initial Admin API outbox NATS publisher with JetStream retries and dead-letter handling.
2. Add higher-level repository tests and conflict handling around the new Postgres account-store source mode.
3. Add Redis online/session/routing cache without making it authoritative.
