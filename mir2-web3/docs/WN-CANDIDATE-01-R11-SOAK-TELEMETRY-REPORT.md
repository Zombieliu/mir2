# WN-CANDIDATE-01 R11 Native Soak Telemetry Report

Date: 2026-08-23
Branch: `codex/wn-candidate-recovery`

## Scope

This round closes the nonvisual instrumentation gap for Goal 6. It adds the
renderer/effect counters and the fail-closed observer required for a future
30-minute run against a real `mir2-platform-windows.exe` process.

Because this host recorded two `0xA IRQL_NOT_LESS_OR_EQUAL` bugchecks, the
round deliberately did not launch or control either game client and did not
perform a Release build. All verification was single-job or used already-built
debug test executables.

## Delivered

### Runtime and effect-producer metrics

When, and only when, `MIR2_NATIVE_SOAK_METRICS=1`, the native runtime emits one
compact JSON line per stream at most once every 10 seconds:

- `[native-soak]` reports snapshot effects, retained primary/mask/shadow/image
  layers, retained entity layers, legacy scene entities, entity atlases, map
  tiles/entities, mine nodes, lighting layers/images, additive cache entries,
  cache entries whose handles still resolve, and additive asset count;
- `[native-soak-fx]` reports active effects and the fixed production cap of
  `96`.

The systems do not retain historical samples. WASM does not compile or register
the native runtime sampler, and the environment variable defaults to disabled.

### Fail-closed 30-minute observer

`monitor-native-candidate-soak.ps1` remains read-only and observes an already
running process. A formal PASS now requires all of the following evidence:

1. process name/path identity, unchanged PID `StartTime`, and a matching
   `processId` embedded independently in both telemetry streams;
2. at least 30 minutes of independent observer wall-clock time;
3. a client log whose canonical path, creation time and NTFS file ID remain
   unchanged for the full observation window;
4. only log bytes appended after observation began; replacement, truncation,
   malformed tagged telemetry or an unreadable log fails closed;
5. runtime and effect streams each span at least 29 minutes, use strictly
   increasing timestamps, never have a gap above 30 seconds, and begin/end
   within 30 seconds of each other at every sample rather than only at the
   first and last records;
6. active effects never exceed the observer-owned fixed cap of `96`;
7. every additive cache handle resolves to a live material asset, cache count
   does not exceed asset count, and retained scene totals do not grow
   monotonically after the 10-minute warmup;
8. at least one successful native WebSocket resume in the same client-log
   window (`[gateway-client] ... resume=true`);
9. no crash, panic, device-lost, `B0001`, or unhandled-protocol log marker;
10. Gateway health checks remain successful, Windows crash-event observation
    remains available and clean, and final RSS is no more than 125% of the
    warmup baseline.

`verify-windows-candidate.ps1` intentionally clears
`MIR2_NATIVE_SOAK_METRICS`; it verifies a normal packaged launch and is not the
soak launcher. The real soak must launch the candidate separately with the
variable set to `1`, redirect native stderr to the supplied client-log path,
and then pass that PID/log to the observer.

### Deterministic runtime tests

Tests that install the process-global native ingest queue now share a
test-only mutex. This removes the observed default-harness race without adding
any lock to production code.

## Verification evidence

- Runtime soak focused tests: `2 passed; 0 failed`.
- Native data-path focused tests: `8 passed; 0 failed`.
- Runtime full suite with default parallel test harness, repeated three times:
  `184 passed; 0 failed` on every run.
- Windows native full suite: `295 passed; 0 failed`.
- Runtime WASM check:
  `cargo +1.95.0 check --target wasm32-unknown-unknown --jobs 1 --quiet` — PASS.
- Runtime and Windows `cargo fmt --check` — PASS.
- Monitor self-test under PowerShell 7 and Windows PowerShell 5.1 — PASS. The
  self-test proves sparse telemetry is rejected, malformed tags are rejected,
  a fixed cap is enforced, stale cache handles are rejected, telemetry from a
  different PID is rejected, a successful reconnect is counted, and replacing
  the client log changes its file identity.
- Candidate verifier self-test under PowerShell 7 and Windows PowerShell 5.1 —
  PASS (`VERIFY_ADS_SELFTEST=passed`).
- Independent read-only Rust review: no P0/P1 finding.

One initial direct runtime run exposed six failures caused by concurrent tests
replacing the shared global ingest queue. After the test-only serialization
fix, the default parallel suite passed three consecutive runs. The initial
failure is recorded rather than omitted.

## Explicitly not certified by this report

- a real 30-minute native EXE run containing movement, targeting, combat,
  inventory, quest, NPC, consumable, menu, logout/login and InGame network
  recovery actions;
- latest-revision attested Release EXE and client-only package;
- full Web production build (the WASM Rust check passed, but is narrower);
- fresh-account visible login/character/task-chain/persistence replay;
- real 100%, 125% and 150% Windows DPI windows;
- original-client screenshot baseline and Gemini difference scoring;
- independent final Candidate review and external human visual/play-feel signoff.

The project remains an internal Candidate under active recovery. R11 makes a
future real soak auditable; it does not substitute instrumentation or self-tests
for the required 30-minute human-visible run.
