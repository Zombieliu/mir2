# Packet Parity Acceptance

Last updated: 2026-04-28

Purpose: define the accepted live Crystal packet-comparison gate for the current representative TCP matrix.

## Decision

Stable live packet comparison is accepted as the packet parity gate for the current tracked backend/server slice.

Strict exact byte-for-byte diff remains a diagnostic signal, not the acceptance gate, unless a future deterministic Crystal fixture can fully control Crystal object ids, timestamps, character slot lifecycle state, AOI ordering, and dynamic NPC payloads.

This decision is scoped to packet parity only. It does not close final frontend visual/feel acceptance and does not mark product-evolution systems as Crystal 1:1.

## Evidence

- R298 live Crystal matrix: `docs/generated/packet-traces/r298-live-matrix/latest-matrix.json`
- R298 summary: 9/9 local OK, 9/9 Crystal OK, `crystalMissingCount=0`, `stableDiffCleanCount=9`, `acceptedStableLiveComparisonCount=9`
- R298 strict exact diagnostic: `diffDirtyCount=9`, `acceptedLiveComparisonCount=0`
- R299 payload-hex probe: `docs/generated/packet-traces/r299-movement-hex.json`
- R299 finding: `Turn`, `Walk`, `Run`, and `UserLocation` are aligned; remaining exact dirtiness is from live Crystal dynamic state/control surfaces, including object ids, timestamps, lifecycle character indices, AOI ordering/payloads, and dynamic `DefaultNPC` / `NPCUpdate` payloads.
- R302 original-client diagnostic pack: `docs/generated/player-qa/r302-original-client/summary.json`. This proves local original `Server.exe`/`Client.exe` launch and retained-character visual capture, but its fresh matrix is not accepted (`stableDiffCleanCount=2/9`, `packetParityAccepted=false`) because the fresh local account store and mutable Crystal fixture were not state-aligned.

## Accepted Command

Use this mode when the user or acceptance record has explicitly accepted the stable comparator for live Crystal packet parity:

```powershell
cd E:\mir2\mir2-web3
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:7310'
$env:MIR2_CRYSTAL_TCP_ADDR='127.0.0.1:7000'
$env:MIR2_PACKET_TRACE_FIXTURE_MODE='stable'
$env:MIR2_PACKET_TRACE_ACCOUNT='cdx0428030348'
$env:MIR2_PACKET_TRACE_PASSWORD='<password>'
$env:MIR2_PACKET_TRACE_CHARACTER='Cdx0428030348'
$env:MIR2_PACKET_TRACE_CHARACTER_INDEX='8'
$env:MIR2_PACKET_TRACE_ACCEPT_STABLE_DIFF='1'
$env:MIR2_PACKET_TRACE_REQUIRE_LOCAL='1'
$env:MIR2_PACKET_TRACE_REQUIRE_CRYSTAL='1'
$env:MIR2_PACKET_TRACE_MATRIX_OUT_DIR='docs/generated/packet-traces/windows-live-stable-accepted'
cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix
```

Expected `latest-matrix.json` summary:

- `acceptanceMode` is `stable`.
- `localOkCount` equals `artifactCount`.
- `crystalMissingCount` is `0`.
- `stableDiffDirtyCount` is `0`.
- `acceptedPacketParityCount` equals `artifactCount`.
- `packetParityAccepted` is `true`.

Do not set `MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN=1` for this accepted mode. That flag intentionally remains the strict exact diagnostic gate.

## Strict Exact Diagnostic

Strict exact remains available:

```powershell
$env:MIR2_PACKET_TRACE_REQUIRE_DIFF_CLEAN='1'
cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace -- --matrix
```

This command is expected to fail against ordinary live Crystal until the fixture controls the dynamic state listed above.

## Retained Character Fixture Helper

For original-client visual QA only, `account_lifecycle` can keep the created Crystal character instead of deleting it:

```powershell
$env:MIR2_PACKET_TRACE_FLOW='account_lifecycle'
$env:MIR2_PACKET_TRACE_KEEP_LIFECYCLE_CHARACTER='1'
cargo +1.89.0 run --locked -p mir2-gateway --bin packet_trace
```

This helps create a real Crystal account/character for launching `Client.exe`. It is not a packet acceptance gate by itself.

## Status Effect

This decision closes the remaining backend/server tracked-slice packet gate under the stable-diff evidence standard.

Whole-project Accepted 1:1 remains open until the player frontend visual/feel gate in `docs/PLAYER-QA-SCRIPT.md` passes or the remaining frontend differences are explicitly accepted.
