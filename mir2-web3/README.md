# mir2-web3

Crystal / Legend of Mir 2 compatible Web MMORPG implementation.

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
