# WN-CANDIDATE-01 R9 nonvisual recovery report

Status: **INTERNAL CANDIDATE NOT YET CUT**
Branch: `codex/wn-candidate-recovery`
Latest source revision: `b81509472d2a50db2b896b94002ff535cc8dcf90`

This report records only evidence observed on the current Windows host. It does
not certify visual parity, human play feel, public release signing, or a
same-revision R9 package.

## Passed evidence

- Clean detached checkout build wrapper completed at parent revision
  `5e7100798c7a6afa6f0b7f0de3f63c8d74428896`.
- The emitted `mir2.windows.build-attestation.v2` recorded a clean worktree and
  matched the produced executable byte-for-byte.
- Parent-revision executable SHA-256:
  `02C665C187638A96DDD631A1C9B86B62777DBADCAA3DBFD2FC7CEA23872FA5A1`.
- Native keyed map clean-checkout build passed with 7,158 references, 4,650
  emitted entries and the accepted 2,508 missing-source budget.
- `npm run test:native-map-keyed`: 1 passed, 0 failed.
- `cargo +1.95.0 test --locked --manifest-path
  apps/game-client/platform-windows/Cargo.toml --target
  x86_64-pc-windows-msvc`: 291 passed, 0 failed after running the same
  `assets:map-atlas:build` prerequisite used by CI.
- `cargo +1.95.0 test --locked --manifest-path
  apps/simulation/Cargo.toml --test vertical_slice`: 8 passed, 0 failed. The
  suite covers the Bichon starter quest/combat/drop loop, fresh-character
  starter combat, quest-four deer harvest/drop progression, class baselines,
  and shared-zone stability.
- The two release-pipeline hardening commits are pushed:
  - `5e7100798 build(native): harden attested Candidate pipeline`
  - `b81509472 ci(native): verify attested Windows artifact`
- Independent read-only review found P0=0 and P1=0 after the hardening round.

## Evidence rejected or still missing

- The attested executable above is from `5e7100798`, not latest revision
  `b81509472`. The latter changes CI/build scripts only, but a strict package
  still requires a new exact-revision attestation.
- The latest `npm run build` did not complete. It was interrupted while building
  the WebAssembly runtime; there is no `.next/BUILD_ID`, so Web production build
  is **not** green.
- No strict R9 package has been signed or verified. The host has no current
  private-key Code Signing certificate suitable for the package script.
- No current-revision original/native paired screenshots or Gemini visual score
  exist. Historical Login/Select/HUD scores must not be reused as current proof.
- Real 100%/125%/150% DPI, 30-minute native-client soak, and external human
  10-20 minute play-feel acceptance remain open.

## Host stability incident

The Windows host produced two matching blue-screen families during long-running
work:

- 2026-08-23: bugcheck `0x0000000A`, dump `082326-15078-01.dmp`.
- 2026-08-22: bugcheck `0x0000000A`, dump `082226-15312-01.dmp`.

Both are `IRQL_NOT_LESS_OR_EQUAL` kernel crashes followed by unexpected restart,
not agent-issued shutdown commands. High-load builds are paused until the dump
stack is analysed (preferably with Microsoft WinDbg) or a stable build host is
provided. The interrupted Web build lock was moved to an isolated temporary
archive; it was not deleted and the clean checkout is clean again.

## Honest acceptance decision

The gameplay and native unit gates are strong enough to continue toward an
internal playable Candidate, but they do not prove the requested 100% Candidate.
The next release attempt must, in order:

1. identify or mitigate the kernel/driver crash source;
2. build latest revision `b81509472` through the attested wrapper;
3. complete Web production build;
4. cut and verify a clearly labelled internal QA package;
5. run same-scene visual/Gemini, real DPI, soak, persistence replay, and human
   acceptance gates.
