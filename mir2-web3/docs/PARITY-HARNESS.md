# Parity Harness

Last updated: 2026-04-22

This file documents the repeatable local-vs-Crystal packet and behavior harness used by `docs/CRYSTAL-1TO1-ROADMAP.md`.

## Trace Matrix

The machine-readable parity matrix lives at:

- `docs/parity-matrix.json`

The matrix defines the representative flows, current automation mode, expected packet families, behavior checks, and Crystal acceptance requirement for each full 1:1 category.

## Packet Trace Binary

List supported TCP trace flows:

```powershell
cd E:\mir2\mir2-web3
cargo run -p mir2-gateway --bin packet_trace -- --list-flows
```

Capture the default local flow:

```powershell
cd E:\mir2\mir2-web3
cargo run -p mir2-gateway --bin packet_trace
```

Capture a specific local flow:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_PACKET_TRACE_FLOW='movement_chat_keepalive'
cargo run -p mir2-gateway --bin packet_trace
```

Capture local and live Crystal side by side:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_CRYSTAL_TCP_ADDR='<crystal-host>:<crystal-port>'
$env:MIR2_PACKET_TRACE_FLOW='core_bootstrap'
$env:MIR2_PACKET_TRACE_OUT='docs/generated/packet-traces/core-bootstrap-live.json'
cargo run -p mir2-gateway --bin packet_trace
```

The output includes endpoint status, packet ids, packet names, payload lengths, payload hashes, elapsed capture time, and diff mismatch reasons.

Diff mismatch reasons are intentionally coarse until field-level decoders exist for every payload:

- `endpoint_error`
- `missing_local_packet`
- `missing_crystal_packet`
- `direction_mismatch`
- `packet_id_mismatch`
- `packet_name_mismatch`
- `packet_order_mismatch`
- `decode_status_mismatch`
- `payload_length_mismatch`
- `payload_hash_mismatch`
- `timing_tolerance_mismatch`

Timing comparison is disabled by default because endpoint latency is not stable enough for unattended local-vs-live runs. Enable it only for controlled runs:

```powershell
$env:MIR2_PACKET_TRACE_COMPARE_TIMING='1'
$env:MIR2_PACKET_TRACE_TIMING_TOLERANCE_MS='750'
```

Known nondeterministic fields are written into each diff report: `generatedAtUnixMs`, `elapsedMs`, ephemeral fixture names, and payload hashes that include timestamps, object ids, generated account names, randomized drops, or runtime allocation ids.

Capture every TCP-traceable matrix flow into separate artifacts:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_PACKET_TRACE_MATRIX_OUT_DIR='docs/generated/packet-traces/matrix'
cargo run -p mir2-gateway --bin packet_trace -- --matrix
```

Matrix mode reads `docs/parity-matrix.json` and writes one JSON file per matrix entry that declares a `traceFlow`. Flows without `traceFlow` are intentionally left to WebSocket/UI smoke or simulation harnesses until packet-level protocol coverage exists for those systems.

Use require mode for local/CI checks:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_PACKET_TRACE_REQUIRE_LOCAL='1'
cargo run -p mir2-gateway --bin packet_trace -- --matrix
```

Use strict live mode after Crystal is reachable:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_CRYSTAL_TCP_ADDR='<crystal-host>:<crystal-port>'
$env:MIR2_PACKET_TRACE_REQUIRE_LOCAL='1'
$env:MIR2_PACKET_TRACE_REQUIRE_CRYSTAL='1'
$env:MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN='1'
cargo run -p mir2-gateway --bin packet_trace -- --matrix
```

`MIR2_PACKET_TRACE_REQUIRE_CRYSTAL=1` implies diff-clean mode. The command exits non-zero when a required endpoint is unavailable, a required diff is missing, or a comparable diff has mismatches.

## Fixture Modes

Default mode is `ephemeral`. It creates lifecycle account and character names from the current timestamp so local repeated runs do not require cleanup.

Use stable mode when comparing against a prepared Crystal environment:

```powershell
$env:MIR2_PACKET_TRACE_FIXTURE_MODE='stable'
$env:MIR2_PACKET_TRACE_ACCOUNT='demo'
$env:MIR2_PACKET_TRACE_PASSWORD='demo'
$env:MIR2_PACKET_TRACE_LIFECYCLE_ACCOUNT='trace-fixture'
$env:MIR2_PACKET_TRACE_LIFECYCLE_PASSWORD='trace-pass'
$env:MIR2_PACKET_TRACE_LIFECYCLE_NEW_PASSWORD='trace-new-pass'
$env:MIR2_PACKET_TRACE_CHARACTER='TraceOne'
```

Do not commit passwords or private hostnames into the repo. Keep private fixture values in the shell environment.

## Local Reset

For a clean local run, start the Rust gateway with a dedicated account store path:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_ACCOUNT_STORE_PATH='docs/generated/packet-traces/local-trace-accounts.json'
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_GATEWAY_WEB_ADDR='127.0.0.1:7010'
cargo run -p mir2-gateway --bin mir2-gateway
```

To reset local fixtures, stop the gateway and replace the dedicated account store with a fresh empty file or a known baseline backup. Do not delete the shared default `.mir2-data/accounts.json` during parity runs unless the current task explicitly calls for resetting shared local state.

## Crystal Reset

Before a stable live Crystal comparison:

- Ensure `MIR2_CRYSTAL_TCP_ADDR` points at the intended Crystal server.
- Ensure `MIR2_PACKET_TRACE_ACCOUNT` exists with the expected password and at least one playable character.
- Ensure the lifecycle account named by `MIR2_PACKET_TRACE_LIFECYCLE_ACCOUNT` is absent or restored to the expected pre-run state.
- Ensure the lifecycle character named by `MIR2_PACKET_TRACE_CHARACTER` is absent or restored to the expected pre-run state.
- Record the Crystal database or save snapshot used for the comparison in `docs/generated/packet-traces`.

If Crystal fixture reset is not possible, keep the flow unchecked and record the blocker in the roadmap gap register.

## Current TCP Trace Flows

- `core_bootstrap`
- `account_lifecycle`
- `movement_chat_keepalive`
- `inventory_storage`
- `combat_basic`
- `storage_password`

Flows that require gateway-only commands, UI interaction, or Stage 5 debug commands remain covered by WebSocket/UI smoke and simulation tests until packet-level protocol support exists for those actions.
