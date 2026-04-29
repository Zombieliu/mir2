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
