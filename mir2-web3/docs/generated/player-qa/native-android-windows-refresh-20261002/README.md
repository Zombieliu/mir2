# Android Windows-source refresh — 2026-10-02

Status: **bounded source integration and regression PASS; complete goal Active**.
No new APK, emulator capture, real login, NPC purchase, physical-device or
production deployment acceptance is supplied by this checkpoint.

## Exact source and preservation

- Android parent: `9c2a5428d77c6fd2a146b920474eba4b839fdab1`.
- Windows source: `3d735745f1117d42a7859e87604a106351dca935`.
- Common ancestor / original goal baseline:
  `3f5e61533235921369bc13a7760b4a56b0e467e5`.
- This explicitly adds the 14 Windows commits / 238 filenames recorded in
  [the original inventory](../native-android-chat-editor-20261002/windows-upstream-delta.json).
  The selected source includes NPC response/service lifetime, native aim,
  public monster visibility, canonical mana and finite map-transfer state.
- Normal two-parent merge on `codex/android-shared-sync`, not rebase/reset,
  force push, PR merge, Windows branch write or production rollout.
- The existing dirty `codex/steam-main` checkout remains at `31b2b396057d82aa80567a33a668907d5d41a432`;
  its prior dirty entries remain. Original Android checkout remains clean at
  `5d417ca75a5fe845a7d6f6d9e16c720d55e19b36`. No checkout was switched/stashed/cleaned.

Five content conflicts were resolved: Gateway routing, Zone manager/runtime,
task queue and roadmap. Rust source conflicts were formatting plus ordered test
module overlaps. `audit-merge.mjs` parses/formats the complete three server files
using Rustfmt 1.89 and compares their complete bytes with exact Windows source:
all three match. The Gateway retains the union of 29 test submodules without
duplicate declarations. The two progress files retain both Android v16/v15
history and the Windows caster checkpoint. Read-only peer review found no
merge-introduced P0/P1/P2; it did not run tests/builds or accept Android NPC wiring.

## Fresh checks

All Rust tests use `--locked --offline` and serial test threads. Client/Android
uses Rust 1.95; server scopes use 1.89. Build cache reuse does not substitute
for executing tests. These scopes overlap and are not a completion percentage.

| Executed scope | Result | Boundary |
| --- | --- | --- |
| Android normal host, all lib tests | 276 passed, 0 failed | Host contracts, not real login |
| Android actual ui-preview feature, all lib tests | 290 passed, 0 failed | Offline fixtures |
| Shared native-player-ui, all lib tests | 1216 passed, 0 failed, 10 ignored | No ignored visual fixture counted as pass |
| Shared runtime, all lib tests | 291 passed, 0 failed, 1 ignored | Includes ordered Closed/Buy signal test |
| Shared Zone unit scope | 179 passed, 0 failed, 1754 filtered | Not the complete simulation suite |
| Canonical mana metadata | 3 passed, 0 failed | Metadata precedence and fallback bounds |
| Canonical mana projection integration | 5 passed, 0 failed | In-memory authoritative packet projection |
| Shared Zone poison transfer integration | 5 passed, 0 failed | Finite transfer and identity/deadline guards |
| Gateway private monster snapshot scope | 4 passed, 0 failed | Buried/revealed and public visibility |
| Gateway map-transfer poison scope | 3 passed, 0 failed | Old viewport retirement, personal receipts retained |
| Gateway cadence-map-transfer adjacent scope | 4 passed, 0 failed | Existing cadence preserved |
| Gateway personal-monster projection adjacent scope | 2 passed, 0 failed | Existing personal metadata preserved |
| Seven affected quest-agent/evidence test files | 366 passed, 0 failed, 0 skipped | Controller fixtures, not ordinary live gameplay |
| API31 arm64 Android Debug target check | PASS | Actual native compile, no APK packaging |
| API31 arm64 Android uiPreview target check | PASS | Actual preview feature compile |
| Three resolved Rust files scoped format / staged diff check | PASS | No whole-repository rewrite |

Java was unchanged and was **not rerun** in this source-only leaf. Its v16
37+37 results remain historical. No Windows executable/suite, full Gateway or
full server suite was rerun or relabelled green.

## Retained first failures and actual remedy

The first controller run was 361/366 with five missing-local-map failures.
After checking out the tracked D022/D2031 maps, it was 365/366; the retained
stack and test source identified D2032 as the destination input. Exact Git
materialization of that third map led to the final 366/366. No tests were skipped,
weakened, or supplied with synthetic collision maps. `map-input-blobs.txt` and
`map-upstream-blobs.txt` bind all three files to Windows Git blobs; sparse changes
are scoped to this independent worktree. Both failure logs remain unchanged.
The initial conflict-format check also failed; only those three resolved Rust
files were mechanically formatted, and the after check passes.

The initial mana/transfer run is retained separately from its final rerun after
the format operation had completed. Only the final source-bound run is used in
the result table. Raw log line endings/trailing whitespace are retained by this
directory's attributes; they are not edited to make a diff or error gate green.

## Acceptance still open / next work

The latest installed diagnostic APKs remain **v16**, exact source
`8cd2e7eaeead8c71f74fbbaa9250bb2d875fed04`; they do **not** contain this Windows
refresh. The previous package hashes, screenshots and 10 startup GL0x506 entries
remain bound to that source. No renderer fix or zero-error/stability pass follows.
A fresh exact-source APK and API31 interaction pass are still needed.

NI-10/11 remains OPEN: Java's authenticated public-packet allowlist does not yet
forward NPCResponse/Goods/Sell/Repair; Android has no equivalent complete NPC/quest
producer, nor the new accepted-service-request/ordered Exit host latch. Shared UI
merge and queue tests cannot stand in for those paths. Next connect the same
Windows projectors to the Android owner/phase/FIFO/reset boundaries, add failing
regressions first, then build/install and verify the actual service UI.

Full UI/short-IME/multi-touch/resources/audio, real HTTPS/WSS player loop and
physical/human acceptance remain in the full Windows-alignment goal. Capacity
is still paused. No new server, account, save, release feed or asset source is
used or changed here.

## Reproduction / evidence binding

From repository root with the pinned toolchain available on PATH:

```sh
node mir2-web3/docs/generated/player-qa/native-android-windows-refresh-20261002/audit-merge.mjs <normal-integration-merge-sha>
```

Without an argument, this audit expects the resolved pending merge and examines
the working files. With a merge SHA it reads immutable source, so future Android
changes cannot silently rebind this checkpoint. `MIR2_AUDIT_RUSTFMT` and
`MIR2_AUDIT_TOOLCHAIN` are explicit tool overrides, not alternate acceptance data.
The curated files are exact copies of the ignored execution evidence under
`apps/game-client/platform-android/target/windows-upstream-refresh-20261002`.
The integrity manifest records byte SHA-256 for each original proof copy.
