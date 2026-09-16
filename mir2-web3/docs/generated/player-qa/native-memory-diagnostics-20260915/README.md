# Native memory diagnostics — 2026-09-15

This scoped QA note records the bounded native soak instrumentation added on
the `codex/windows-player-journey` worktree. It is diagnostic evidence only;
it does not claim Crystal parity or frontend acceptance.

## Signals

When `MIR2_NATIVE_SOAK_METRICS=1` is set, the existing 10-second runtime
sample now records:

- Bevy `Assets<Image>` count and summed CPU `Image.data` bytes;
- native inbound message count and retained queue byte capacity;
- the existing renderer registry counters.

The Windows host emits a matching `[native-soak-atlas]` line with:

- starter atlas page and rect counts;
- decoded starter atlas RGBA bytes;
- geometry-cache library/frame counts;
- original-frame pixel-cache entries and RGBA bytes (with the 256-entry cap).

All full scans and cache locks occur only when a 10-second sample is due. The
normal frame path and queue admission limits are unchanged. The existing
read-only PowerShell observer remains the source for host process
`privateBytes`/working-set telemetry; no new Win32 process-memory API was
needed in this change.

## Verification

- `cargo +1.95.0 fmt --manifest-path apps/game-client/runtime/Cargo.toml -- --check` — pass
- `cargo +1.95.0 fmt --manifest-path apps/game-client/platform-windows/Cargo.toml -- --check` — pass
- Runtime focused tests: 4/4 pass (`native_soak_metrics_tests`)
- Windows atlas focused tests: 29/29 pass (`atlas::tests`)
- `monitor-native-candidate-soak.ps1 -SelfTest` — pass (expected short-duration result)
- `git diff --check` on changed source files — pass

## R20 build artifact

Using asset root `C:/mir2-natural-journey-20260911/hookingcat-resource-package-r2/mir2-assets`,
the Windows package now contains
`mir2-platform-windows-memory-diagnostics-r20.exe` alongside the preserved R19
EXE. Full Windows tests pass 648/648. R20 is 77,944,832 bytes with SHA-256
`E0305D023EDBBE8CFCA341BAFACCC0B9639A3D5AFE52D5440ABD838DD4946928`.
The package metadata is `QA-VERSION-memory-diagnostics-r20.json`; its source
HEAD is `b126ba4804caf8b1d15564e151066f65da7e4b6b` and its aggregate hash for
the four instrumented source files is
`DDB8CBD2B2ADBCBD2A3E0087DEB34A7D711D257C11DD41D8B41F7BAD3402BBEE`.

Raw logs are stored beside this note in `windows-full-suite-20260915.log` and
`release-build-20260915.log`. The R19 binary was not overwritten, and R20 was
not launched by the worker. The coordinator subsequently launched verified
R20 with the natural level-30 Warrior account against isolated R52 at 13:11,
then restarted with render tracing (PID 53776). Raw evidence is in
`C:/mir2-native-run-acceptance-20260915`. The visible head-only body revealed
missing equipment libraries in the active resource package; original resources
are being added and visual confirmation remains open.

`accepted=false`, `visualAccepted=false`, and `globalParityPercent=null` for
this diagnostic slice. Packaging and isolated local QA launches occurred;
no production rollout was performed. Continuous held-right acceptance and a
long enough measured memory soak are still open.
