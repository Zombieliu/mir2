# mir2-web3

Crystal / Legend of Mir 2 compatible Web MMORPG implementation.

> 简体中文读者请从 [`README.zh-CN.md`](README.zh-CN.md) 开始。它用玩家、
> 公会和节点运营者都能理解的方式说明项目架构、Mir2 玩法设计、商业模式、
> 当前验收状态与生产边界。

For a new Windows checkout, start with the repository-level
[`README.md`](../README.md), then use:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\scripts\bootstrap-developer.ps1
.\scripts\start-developer.ps1 -OpenBrowser
```

The supported default is Player Web on `http://127.0.0.1:3002/`, Gateway
HTTP/WebSocket on `127.0.0.1:7110`, and Crystal TCP on `127.0.0.1:7000`.
The start script aligns these ports and uses the tracked prebuilt WebGPU/WebGL2
Bevy runtimes.

## Developer Documentation

- [Developer handoff](docs/DEVELOPER-HANDOFF.md)
- [Windows local development](docs/LOCAL-DEVELOPMENT-WINDOWS.md)
- [Asset consumer setup](docs/ASSET-CONSUMER-SETUP.md)
- [Agent orchestration](docs/AGENT-ORCHESTRATION.md)
- [Crystal 1:1 roadmap](docs/CRYSTAL-1TO1-ROADMAP.md)
- [Backend progress](docs/BACKEND-1TO1-PROGRESS.md)
- [Crystal server parity](docs/CRYSTAL-SERVER-PARITY.md)
- [Gate 14 no-single-point POC](docs/GATE14-NO-SINGLE-POINT-POC.md)
- [Gate 15 real-player failover](docs/GATE15-REAL-PLAYER-FAILOVER.md)
- [Gate 16 incremental replication](docs/GATE16-INCREMENTAL-REPLICATION.md)

## Layout

| Path | Responsibility |
| --- | --- |
| `apps/web` | Next.js Player Web, browser input, HUD, asset cache, and QA |
| `apps/game-client/runtime` | Bevy WebGPU/WebGL2 WASM renderer |
| `apps/gateway` | Rust TCP/HTTP/WebSocket gateway, auth, sessions, and Zone routing |
| `apps/simulation` | Authoritative personal sessions and shared Zone simulation |
| `apps/admin-api` | Audited administration API |
| `apps/admin-web` | Operations console |
| `packages/protocol` | Crystal-compatible packet definitions and codecs |
| `packages/game-data` | Converted game data that is safe to track |
| `packages/tooling` | Import, conversion, and migration tools |
| `scripts` | Developer bootstrap, start, verification, and asset packaging |
| `docs` | Architecture, parity status, runbooks, and generated evidence |

## Runtime Architecture

- Rust Gateway and Simulation are the authoritative gameplay core.
- A personal `SimulationSession` owns login, character, inventory, equipment,
  and save/load state.
- A shared `ZoneRuntime` owns online world position, movement validation,
  occupancy, AOI, and object broadcasts.
- Player Web projects Gateway state into the Bevy WASM renderer and Crystal-style
  DOM HUD.
- Postgres, Redis, NATS, Redpanda, ClickHouse, Meilisearch, Loki, and Grafana are
  optional local infrastructure. They are not required for the basic file-store
  Player Web flow.
- Gate 14 adds an opt-in four-validator Commonware `v2026.2.0` control network,
  dual dynamic Gateways, dual Dubhe Zone Hosts, and replayable Postgres/Redis
  projections. Its Docker fault-recovery acceptance and full architecture
  diagram are documented in
  [`docs/GATE14-NO-SINGLE-POINT-POC.md`](docs/GATE14-NO-SINGLE-POINT-POC.md).
- Gate 15 connects real WebSocket and Crystal TCP player admission to that
  finalized control state. Two players on separate Gateways survive active Zone
  Host loss and continue on the promoted checkpoint replica without reconnecting.
  See [`docs/GATE15-REAL-PLAYER-FAILOVER.md`](docs/GATE15-REAL-PLAYER-FAILOVER.md).
- Gate 16.1 adds the reproducible v4 full-checkpoint performance ruler,
  low-cardinality checkpoint/replay telemetry, and a constrained Docker
  baseline; see
  [`docs/GATE16-INCREMENTAL-REPLICATION.md`](docs/GATE16-INCREMENTAL-REPLICATION.md).
- Gate 16.2 adds a bounded per-Zone v5 replication Head with a continuous
  cursor, chained digest, build identity, and explicit coverage/readiness
  safety fields.
- Gate 16.3 adds bounded verified mutation batches plus a restart-safe,
  fsync-before-ACK receive WAL in both replication directions. The replicator
  intentionally continues installing v4 checkpoints: autonomous tick/AI
  capture, incremental standby apply, compaction, and promotion readiness remain
  later Gate 16 work.
- Gate 16.4a adds per-Zone, cursor-bound gzip base snapshots with SHA-256
  identity and crash-safe atomic persistence.
- Gate 16.4b1 makes complete Session images installable without replaying old
  commands. Installation rebuilds and verifies private character state in
  isolation, atomically adopts one Zone resource image, preserves unrelated
  Zones, reports the compacted base cursor in v5 Head, and keeps
  `promotionReady=false` until autonomous tick/AI capture and incremental apply
  are complete.

## Crystal Reference

The sibling `Crystal` Git submodule is the reference implementation for gameplay
rules, packet flow, map/asset formats, and server behavior. It also contains the
handoff parity tools needed by the verification script.

Initialize it through the repository bootstrap or manually:

```powershell
git -C .. submodule sync --recursive
git -C .. submodule update --init --recursive
```

Do not switch the submodule to an unrelated upstream commit. Any intentional
Crystal change must first be pushed to the configured handoff branch before the
root repository pointer is updated.

## Asset Modes

The project deliberately separates code from the multi-gigabyte full Crystal
pack:

| Mode | Command | Use |
| --- | --- | --- |
| Starter | `.\scripts\start-developer.ps1` | First run and ordinary gameplay/backend development |
| Private GitHub bundle | `.\scripts\install-developer-assets.ps1 -Download` | Full offline developer assets |
| R2 CDN | `.\scripts\start-developer.ps1 -AssetBaseUrl <url>` | Remote acceptance and CDN/cache testing |

The full pack belongs at
`apps/web/public/generated/crystal-packs/full` and is intentionally ignored by
Git. Do not commit Crystal client builds, original `.Lib` files, the full pack,
account stores, or asset credentials.

## Verification

```powershell
.\scripts\verify-developer-setup.ps1
```

For a faster iteration that skips only the production Web build:

```powershell
.\scripts\verify-developer-setup.ps1 -SkipBuild
```

The complete verification checks the Crystal handoff branch, tracked Starter
assets, Gateway compilation, asset-release safety tests, TypeScript, and the
production Web build.

## Optional Infrastructure

Only start the development infrastructure when the task requires Postgres,
Redis, event analytics, Admin services, or production-like policy:

```powershell
docker compose -f infra/docker-compose.dev.yml up -d postgres redis nats redpanda clickhouse
```

The default local account store remains `.mir2-data/accounts.json`.
