# apps/gateway

Rust gateway for the Mir2 Web3 rewrite.

## Current Status

The gateway is no longer just an early bootstrap stub. It fronts `apps/simulation`
for authority logic, exposes browser HTTP/WebSocket routes, accepts Crystal-framed
TCP packets, persists account/character state through the configured account
store, and carries the local packet trace harness used by the 1:1 parity docs.

R300 accepts the stable live packet comparator for the current tracked
backend/server packet matrix. Strict exact live diff remains available as a
diagnostic for deterministic fixture work, while final whole-project acceptance
still depends on human Crystal visual/feel acceptance.

## Main Surfaces

- `src/main.rs`: HTTP/WebSocket/TCP gateway entry point.
- `src/session.rs`: Crystal-framed TCP session handling.
- `src/web.rs`: browser API, WebSocket commands, and JSON event projection.
- `src/auth.rs`: Sui Passkey / Sui wallet gateway token verification.
- `src/browser_commands.rs`: browser command parsing and protocol enum helpers.
- `src/bin/smoke.rs`: scripted local TCP smoke.
- `src/bin/packet_trace.rs`: local/live packet trace and matrix artifact harness.

## Supported Local Flows

The current gateway covers local account lifecycle, login/start-game bootstrap,
movement/chat/keepalive, inventory/storage actions, basic combat packets, and
storage password actions through the simulation runtime. Exact Crystal acceptance
is tracked in `docs/BACKEND-1TO1-PROGRESS.md`, `docs/CRYSTAL-SERVER-PARITY.md`,
and `docs/PARITY-HARNESS.md`.

## Local Run

Use non-default ports if `7000` or `7010` are already occupied.

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_GATEWAY_WEB_ADDR='127.0.0.1:7010'
cargo run -p mir2-gateway --bin mir2-gateway
```

Manual browser surface:

- `http://127.0.0.1:7010/`

Health check:

- `http://127.0.0.1:7010/health`

Account-store runtime policy:

- local default uses `MIR2_ACCOUNT_STORE_PATH` or `.mir2-data/accounts.json`.
- set `MIR2_ACCOUNT_STORE_BACKEND=postgres` and
  `MIR2_ACCOUNT_STORE_DATABASE_URL` to use Postgres as the source of truth.
- the Postgres account-store path uses an in-process connection pool; tune it
  with `MIR2_ACCOUNT_STORE_PG_POOL_MAX_SIZE` (default `8`),
  `MIR2_ACCOUNT_STORE_PG_POOL_WAIT_TIMEOUT_MS` (default `2000`), and
  `MIR2_ACCOUNT_STORE_PG_CONNECT_TIMEOUT_MS` (default `3000`).
- `MIR2_RUNTIME_ENV=production|prod|staging`,
  `MIR2_DEPLOYMENT_ENV=production|prod|staging`, or
  `MIR2_ENV=production|prod|staging` requires the Postgres source-of-truth
  account store.
- the same production/staging environment policy also requires
  `MIR2_GATEWAY_REDIS_CACHE_URL`; local development may still use the
  in-memory cache.
- set `MIR2_GATEWAY_REQUIRE_REDIS_CACHE=1` to force Redis session/routing cache
  even outside production/staging.
- active route/session cache refreshes are throttled per WebSocket with
  `MIR2_GATEWAY_ROUTE_REFRESH_INTERVAL_MS` (default `5000`, clamped
  `250..30000`) so low-latency movement/keepalive traffic does not rewrite the
  Redis route lease on every packet.
- runtime tick cadence is configurable with `MIR2_GATEWAY_RUNTIME_TICK_MS`
  (default `300`, clamped `100..5000`); raising it for soak tests reduces idle
  per-session simulation CPU at the cost of slower delayed world effects.
- Tokio worker count is configurable with `MIR2_GATEWAY_TOKIO_WORKER_THREADS`
  (default host parallelism, clamped `1..64`) so CPU-heavy synchronous
  simulation steps can be isolated from lightweight HTTP health scheduling.
- production/staging Passkey and wallet login also requires
  `MIR2_PASSKEY_AUTH_SECRET`.
- Zone placement can be loaded from `MIR2_ZONE_TOPOLOGY_FILE` or
  `MIR2_ZONE_TOPOLOGY_JSON`; see `config/zone-topology.example.json`. Explicit
  groups share quiet maps, while unlisted maps receive a dedicated Zone and
  every Zone owns its configured tick cadence.
- Active characters atomically rebind after a topology-changing map transfer.
  Remote close is owner-fenced and checkpointed, while server shouts and GM
  announcements use the bounded cross-Zone live message bus.
- `ZoneHostControlPlane` registers multiple Zone Hosts, schedules primary/replica
  placement leases by capacity and failure domain, fences rebalances by generation,
  and drains hosts without accepting new sessions. Zone RPC v4 health advertises
  host identity, load, capacity, active connections, and drain state.

Admin runtime read endpoints:

- `POST /admin/system-mail`: deliver audited Admin API mail into the configured account store.
- `POST /admin/kick-player`: remove one character from the current session/routing cache.
- `GET /admin/sessions`: list current online session-cache records from the in-memory or Redis cache.

## Smoke And Trace

Scripted TCP smoke:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
cargo run -p mir2-gateway --bin smoke
```

List packet trace flows:

```powershell
cd E:\mir2\mir2-web3
cargo run -p mir2-gateway --bin packet_trace -- --list-flows
```

Capture the local packet trace matrix:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_PACKET_TRACE_REQUIRE_LOCAL='1'
cargo run -p mir2-gateway --bin packet_trace -- --matrix
```

Capture local and live Crystal side by side with the strict exact diagnostic:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_CRYSTAL_TCP_ADDR='<crystal-host>:<crystal-port>'
$env:MIR2_PACKET_TRACE_REQUIRE_LOCAL='1'
$env:MIR2_PACKET_TRACE_REQUIRE_CRYSTAL='1'
$env:MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN='1'
cargo run -p mir2-gateway --bin packet_trace -- --matrix
```

Capture local and live Crystal side by side with the accepted stable comparator:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7310'
$env:MIR2_CRYSTAL_TCP_ADDR='<crystal-host>:<crystal-port>'
$env:MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF='1'
$env:MIR2_PACKET_TRACE_REQUIRE_LOCAL='1'
$env:MIR2_PACKET_TRACE_REQUIRE_CRYSTAL='1'
cargo run -p mir2-gateway --bin packet_trace -- --matrix
```

Matrix output is written under `docs/generated/packet-traces/matrix` unless
`MIR2_PACKET_TRACE_MATRIX_OUT_DIR` is set.

## Current Limitations

- Strict exact live diff still requires a deterministic Crystal server fixture.
- Source-data import remains blocked on machines that do not have
  `Crystal/Build/Server/Debug/Server.MirDB` and matching `Envir/Routes`.
- Some full-project systems are still covered by WebSocket/UI smoke or simulation
  baselines rather than accepted live Crystal TCP traces.
- `100% Candidate` is an automation status, not final `100% Accepted`.
