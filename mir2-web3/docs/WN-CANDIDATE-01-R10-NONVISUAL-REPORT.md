# WN-CANDIDATE-01 R10 Nonvisual Recovery Report

Date: 2026-08-23
Branch: `codex/wn-candidate-recovery`

## Scope

This round deliberately used low-load, nonvisual verification after two host
bugchecks. It did not launch or control either game client and did not perform
a Release or Web build.

## Delivered

1. Native `Q` is reserved for the shared Quest Log shortcut and no longer also
   emits a world-turn command. Native `E` remains the clockwise turn shortcut.
   Candidate controls and known-issues text now describe the implemented
   behavior.
2. The Windows shell no longer enters `InGame` after `UserInformation` alone.
   It waits until the render runtime accepts an opening `worldSnapshot`.
   Backpressured opening snapshots stay quarantined, including on reconnect.
3. `monitor-native-candidate-soak.ps1` now provides a read-only, fail-closed
   observer for an already-running native process. A formal PASS requires all
   of the following:
   - a verified `mir2-platform-windows.exe` PID;
   - at least 30 minutes of observation;
   - supplied and clean Gateway health evidence;
   - supplied and clean client-log evidence;
   - available Windows crash-event observation with no relevant crash event;
   - no early process exit;
   - final RSS no more than 125% of the post-warmup baseline.

The observer does not launch, stop, attach to, or configure the client,
Gateway, Windows, or a debugger.

## Verification evidence

- Prebuilt Windows native test executable, run directly without recompiling:
  `293 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
- `monitor-native-candidate-soak.ps1` PowerShell 5.1 parser: PASS.
- `package-windows-candidate.ps1` PowerShell 5.1 parser: PASS.
- Soak observer self-test: PASS. The generated evidence correctly remained
  `FAIL-short-duration`; the self-test is not a real soak.
- Candidate package self-test:
  - `ADS_SELFTEST=passed`
  - `REPARSE_SELFTEST=passed`
  - package self-test passed.
- Windows native `cargo fmt --check`: PASS.
- `git diff --check` for the delivered files: PASS (PowerShell line-ending
  conversion warnings only).

## Explicitly not certified by this report

- latest-revision attested Windows Release EXE and package;
- Web production build / Web no-regression gate;
- real 30-minute native process soak with Gateway and client-log evidence;
- 100%, 125%, and 150% physical/VM DPI runs;
- fresh-account visible login, character creation, task-chain and persistence
  replay;
- original-client screenshot baseline and Gemini visual-difference scoring;
- independent-model review;
- external human visual and play-feel acceptance.

The project therefore remains an internal Candidate under active recovery, not
100% Candidate and not Accepted.
