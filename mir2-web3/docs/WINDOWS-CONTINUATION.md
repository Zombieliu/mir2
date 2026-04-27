# Windows Continuation Checklist

Last updated: 2026-04-28

Purpose: exact handoff steps for continuing the Crystal / Mir2 1:1 work and post-1:1 product architecture work on Windows without confusing `100% Candidate`, backend tracked-slice `99.70%`, and real full-project accepted 1:1 `roughly 90.0%`.

Read `docs/PARITY-TRUTH-AUDIT.md` first if you are about to change any status percentage. It defines the difference between Accepted, Candidate, Fallback, Blocked, and Product evolution.

## Status To Preserve

- Automation status: `100.0% Candidate`.
- Backend/server tracked-slice parity: `99.70%`.
- Real full-project accepted 1:1: `roughly 90.0%`.
- Active round: `2026-04-26-R226`.
- Latest product-evolution rounds completed on Mac: `R227` Admin Web/API foundation, `R228` live game-visible GM system mail, `R229` Docker-verified Postgres command/audit/outbox plus NATS dispatch, `R230` Postgres mirror for gameplay JSON account-store saves, `R231` explicit Postgres account-store source-of-truth mode behind `MIR2_ACCOUNT_STORE_BACKEND=postgres`, `R249` Gateway/Redis online presence read models, `R250` Postgres-backed Admin Activity/Economy/Risk projections, and `R251-R253` Postgres zone runtime plus operator/RBAC records.
- Do not mark backend/server `100%` until live Crystal trace acceptance, blocked `Server.MirDB` import, or an explicit acceptance decision closes the remaining `0.30%`.
- Do not mark full-project `100% Accepted` until `docs/PLAYER-QA-SCRIPT.md` passes or the user explicitly accepts remaining differences.
- Synthetic map terrain, local JSON stores, local Admin control-plane smoke results, and production-only Admin gaps such as external identity/provider workflows are Candidate/product-evolution evidence only. They must not be counted as final accepted Crystal 1:1.
- If continuing product evolution instead of parity closure, read `docs/POST-1TO1-EVOLUTION-PLAN.md`, `docs/TECH-MODERNIZATION-RFC.md`, `docs/PLATFORM-CLIENT-STRATEGY.md`, and `docs/ADMIN-OPERATIONS-ARCHITECTURE.md` first. Database, cache, login UI, admin backend, global zone, client distribution, and NPC script parser changes are allowed evolution areas, but the current Candidate baseline should remain a regression reference.

## Files To Bring To Windows

Expected project root:

```powershell
E:\mir2\mir2-web3
```

Required Crystal source/data inputs:

```text
E:\mir2\Crystal\Build\Server\Debug\Server.MirDB
E:\mir2\Crystal\Build\Server\Debug\Envir\Routes
E:\mir2\Crystal\Server
```

Recommended client/resource inputs if available:

```text
E:\mir2\Crystal\Client
E:\mir2\Crystal\Build\Client
```

Keep private live server hostnames, account names, and passwords in environment variables only. Do not commit them.

## Environment Variables

Local gateway:

```powershell
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_GATEWAY_WEB_ADDR='127.0.0.1:7010'
$env:MIR2_ACCOUNT_STORE_PATH='docs/generated/packet-traces/local-trace-accounts.json'
```

Local Docker infra for product-evolution work:

```powershell
$env:ADMIN_DATABASE_URL='postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2'
$env:MIR2_ACCOUNT_STORE_DATABASE_URL='postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2'
$env:NATS_ADDR='127.0.0.1:4222'
```

Optional Postgres source-of-truth gameplay account store:

```powershell
$env:MIR2_ACCOUNT_STORE_BACKEND='postgres'
$env:MIR2_ACCOUNT_STORE_DATABASE_URL='postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2'
```

Leave `MIR2_ACCOUNT_STORE_BACKEND` unset to keep the default JSON account-store backend. Set only `MIR2_ACCOUNT_STORE_DATABASE_URL` to mirror JSON saves into Postgres while keeping JSON as source of truth.

Live Crystal comparison:

```powershell
$env:MIR2_CRYSTAL_TCP_ADDR='<crystal-host>:<crystal-port>'
$env:MIR2_PACKET_TRACE_FIXTURE_MODE='stable'
$env:MIR2_PACKET_TRACE_ACCOUNT='<existing-crystal-account>'
$env:MIR2_PACKET_TRACE_PASSWORD='<password>'
$env:MIR2_PACKET_TRACE_LIFECYCLE_ACCOUNT='trace-fixture'
$env:MIR2_PACKET_TRACE_LIFECYCLE_PASSWORD='<initial-password>'
$env:MIR2_PACKET_TRACE_LIFECYCLE_NEW_PASSWORD='<changed-password>'
$env:MIR2_PACKET_TRACE_CHARACTER='TraceOne'
```

Strict trace gate:

```powershell
$env:MIR2_PACKET_TRACE_REQUIRE_LOCAL='1'
$env:MIR2_PACKET_TRACE_REQUIRE_CRYSTAL='1'
$env:MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN='1'
$env:MIR2_PACKET_TRACE_MATRIX_OUT_DIR='docs/generated/packet-traces/windows-live'
```

## Pull And Verify Baseline

```powershell
cd E:\mir2\mir2-web3
git pull --ff-only origin main
git rev-parse --short HEAD
```

Expected latest commit after this handoff:

```text
R231 Postgres account-store source-mode handoff commit or newer
```

## Docker Infra And Admin/Postgres Smoke

Start core local infrastructure:

```powershell
cd E:\mir2\mir2-web3
docker compose -f infra\docker-compose.dev.yml up -d postgres redis nats
docker compose -f infra\docker-compose.dev.yml ps
```

Expected services:

- `mir2-postgres`: healthy
- `mir2-redis`: healthy
- `mir2-nats`: healthy

Import the current JSON account store into Postgres:

```powershell
$env:ADMIN_DATABASE_URL='postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2'
cargo +1.89.0 run --locked -p mir2-admin-api --bin import-account-store -- .mir2-data/admin-live-smoke.json
```

Run Admin API with Postgres command/audit/outbox and Postgres account-store source mode:

```powershell
$env:ADMIN_DATABASE_URL='postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2'
$env:MIR2_ACCOUNT_STORE_BACKEND='postgres'
$env:MIR2_ACCOUNT_STORE_DATABASE_URL='postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2'
$env:ADMIN_GATEWAY_MAIL_URL='http://127.0.0.1:1/admin/system-mail'
$env:ADMIN_API_ADDR='127.0.0.1:7423'
cargo +1.89.0 run --locked -p mir2-admin-api --bin mir2-admin-api
```

Expected DB behavior:

- Admin command writes `admin_commands`.
- Audit writes `admin_audit_records`.
- Successful command queues `admin_outbox` with `admin.command.succeeded`.
- Account-store source mode writes `accounts.raw_json` and `character_saves.snapshot_json`.
- `accounts.store_version` and `character_saves.save_version` increment on source-mode writes.

Dispatch pending Admin outbox to NATS:

```powershell
$env:ADMIN_DATABASE_URL='postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2'
$env:NATS_ADDR='127.0.0.1:4222'
cargo +1.89.0 run --locked -p mir2-admin-api --bin dispatch-admin-outbox -- --once
```

## Local Candidate Regression Bundle

Run these before changing code:

```powershell
cd E:\mir2\mir2-web3\apps\web
.\node_modules\.bin\tsc --noEmit
.\node_modules\.bin\next build
npm run smoke:stage5-ui
npm run smoke:crystal-map-api
npm run smoke:crystal-minimap-assets
npm run load:gateway-ws
```

Run Rust checks from the repo root:

```powershell
cd E:\mir2\mir2-web3
cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1
cargo +1.89.0 test --locked -p mir2-gateway -- --test-threads=1
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1
cargo +1.89.0 fmt --check
git diff --check
```

## Map/Data Import Gate

Only run and count the map/data generator when `Server.MirDB` and matching `Envir\Routes` exist locally:

```powershell
cd E:\mir2\mir2-web3
node packages\tooling\scripts\generate-crystal-respawn-manifest.mjs
cargo +1.89.0 test --locked -p mir2-game-data -- --test-threads=1
```

If `Server.MirDB` is missing, leave the gate blocked. Do not hard-code fallback data and do not mark the import complete.

## Live Packet Trace Gate

Start the Rust gateway in one terminal:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_GATEWAY_WEB_ADDR='127.0.0.1:7010'
$env:MIR2_ACCOUNT_STORE_PATH='docs/generated/packet-traces/local-trace-accounts.json'
cargo +1.89.0 run --locked -p mir2-gateway --bin mir2-gateway
```

Run strict local-vs-Crystal matrix in another terminal:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_CRYSTAL_TCP_ADDR='<crystal-host>:<crystal-port>'
$env:MIR2_PACKET_TRACE_FIXTURE_MODE='stable'
$env:MIR2_PACKET_TRACE_REQUIRE_LOCAL='1'
$env:MIR2_PACKET_TRACE_REQUIRE_CRYSTAL='1'
$env:MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN='1'
$env:MIR2_PACKET_TRACE_MATRIX_OUT_DIR='docs/generated/packet-traces/windows-live'
cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix
```

Expected artifacts:

```text
docs/generated/packet-traces/windows-live/latest-matrix.json
docs/generated/packet-traces/windows-live/*.json
```

The `latest-matrix.json` summary should show:

- `localOkCount` equals `artifactCount`.
- `crystalMissingCount` is `0`.
- `diffDirtyCount` is `0`.
- `acceptedLiveComparisonCount` equals `artifactCount`.

If any of those fail, keep packet parity open and record the mismatch in `docs/CRYSTAL-1TO1-ROADMAP.md`.

## Final Human Acceptance

After the automated bundle and live trace gate are green, run `docs/PLAYER-QA-SCRIPT.md`.

Passing criteria:

- No blocker or high-severity issue remains.
- Medium issues are fixed or explicitly accepted.
- `docs/FRONTEND-1TO1-GAPS.md` entries are fixed, accepted, or explicitly deferred.
- The user confirms `100% Accepted`.
