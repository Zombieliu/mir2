# Windows Continuation Checklist

Last updated: 2026-04-28

R302 original-client comparison is archived at `docs/generated/player-qa/r302-original-client/summary.json`. Windows launched original Crystal `Server.exe` on `127.0.0.1:7000` and visible `Client.exe`, generated retained character `R302HeroB`, captured original select/game screenshots, refreshed web Stage 5 UI smoke at `http://127.0.0.1:3002` with 88 screenshots and 0 critical console errors, and added `MIR2_PACKET_TRACE_KEEP_LIFECYCLE_CHARACTER=1` for retained live QA fixtures. The R302 fresh live matrix is diagnostic only (`stableDiffCleanCount=2/9`, `packetParityAccepted=false`) because the fresh local store and mutable Crystal fixture were not state-aligned.

R301 final automated Candidate acceptance pack is now refreshed. Evidence summary: `docs/generated/player-qa/r301-summary.json`; map API smoke 18/18 with 0 failures; minimap smoke 0 failures with known 450/451 warning; WS load 64/64 ready with 0 errors and keepalive p95 637 ms; Stage 5 UI smoke 88 screenshots with 0 critical console errors and 32 compact text nodes checked without overflow. Verification passed without Docker: packet-trace bin 15/15, web `tsc --noEmit`, web build, `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674. Temporary gateway/web services were stopped and ports 7000/7110/3002 verified closed.

R300 stable-diff packet acceptance is now landed. R298 Windows live Crystal matrix evidence (`docs/generated/packet-traces/r298-live-matrix/latest-matrix.json`) has 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, and `acceptedStableLiveComparisonCount=9`; R299 payload-hex probing showed strict exact dirtiness comes from live Crystal dynamic state. The accepted packet gate is now stable-diff mode (`docs/PACKET-PARITY-ACCEPTANCE.md`). Strict exact diff remains a diagnostic for deterministic fixture work. Final whole-project acceptance still needs human visual/feel QA.

R297 Windows frontend/player QA refresh: full client resources at `E:\mir2\Crystal\Build\Client\Debug` are available for the automated evidence path. Web build/typecheck, map API smoke 18/18, minimap smoke 0 failures with known 450/451 warning, WS load 64/64 ready with 0 errors, Stage 5 UI smoke 88 screenshots with 0 critical console errors, `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `fmt --check`, and `git diff --check` passed without Docker. Remaining whole-project acceptance gate is human visual/feel QA.

R248 Windows backend update: the `Server.MirDB` plus `Build\Server\Debug\Envir\Routes` import gate is closed for the current backend slice. The generator refreshed Crystal respawn/monster/item/NPC-info manifests and validation passed with `mir2-game-data` 22/22, focused `no_drop_monster_map_rule` 2/2, full `mir2-simulation` 670/670, and `mir2-gateway` 55/55 plus packet-trace bin tests 7/7. Remaining whole-project acceptance gates are full client visual/resource acceptance and human QA.

Purpose: exact handoff steps for continuing the Crystal / Mir2 1:1 work and post-1:1 product architecture work on Windows without confusing `100% Candidate`, backend tracked-slice packet acceptance, and real full-project accepted 1:1 `roughly 90.0%`.

Read `docs/PARITY-TRUTH-AUDIT.md` first if you are about to change any status percentage. It defines the difference between Accepted, Candidate, Fallback, Blocked, and Product evolution.

## Status To Preserve

- Automation status: `100.0% Candidate`.
- Backend/server tracked-slice parity: `100% Accepted for the tracked backend/server slice under stable-diff packet acceptance`.
- Real full-project accepted 1:1: `roughly 90.0%`.
- Active round: `2026-04-28-R302`.
- Latest product-evolution rounds completed on Mac: `R227` Admin Web/API foundation, `R228` live game-visible GM system mail, `R229` Docker-verified Postgres command/audit/outbox plus NATS dispatch, `R230` Postgres mirror for gameplay JSON account-store saves, and `R231` explicit Postgres account-store source-of-truth mode behind `MIR2_ACCOUNT_STORE_BACKEND=postgres`.
- Backend/server tracked-slice `100%` is allowed only under the R300 explicit stable-diff packet acceptance wording; strict exact remains diagnostic until deterministic Crystal fixture work controls volatile state.
- Do not mark full-project `100% Accepted` until `docs/PLAYER-QA-SCRIPT.md` passes or the user explicitly accepts remaining differences.
- Synthetic map terrain, local JSON stores, Admin Web mock read models, and local-only smoke results are Candidate evidence only. They must not be counted as final accepted Crystal 1:1.
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
71cb4de or newer
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

Run accepted stable local-vs-Crystal matrix in another terminal:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_CRYSTAL_TCP_ADDR='<crystal-host>:<crystal-port>'
$env:MIR2_PACKET_TRACE_FIXTURE_MODE='stable'
$env:MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF='1'
$env:MIR2_PACKET_TRACE_REQUIRE_LOCAL='1'
$env:MIR2_PACKET_TRACE_REQUIRE_CRYSTAL='1'
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
- `acceptanceMode` is `stable`.
- `stableDiffDirtyCount` is `0`.
- `acceptedPacketParityCount` equals `artifactCount`.
- `packetParityAccepted` is `true`.

If any of those fail, keep packet parity open and record the mismatch in `docs/CRYSTAL-1TO1-ROADMAP.md`. Use `MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN=1` only for strict exact diagnostics against a deterministic Crystal fixture.

## Final Human Acceptance

After the automated bundle and live trace gate are green, run `docs/PLAYER-QA-SCRIPT.md`.

Passing criteria:

- No blocker or high-severity issue remains.
- Medium issues are fixed or explicitly accepted.
- `docs/FRONTEND-1TO1-GAPS.md` entries are fixed, accepted, or explicitly deferred.
- The user confirms `100% Accepted`.
