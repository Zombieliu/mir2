# Native world-data receipts — 2026-09-10

Source `0ee203649f1a18cccd8b97c910a0bba6f706a3b5`, local Android worktree and
branch `codex/android-player-journey`. No push, Windows-specific/backend edit,
production access, account credentials or save mutation.

## Implementation

The previous snapshot ingress only confirmed queue acceptance. Shared runtime
now exposes `NativeWorldReceipt`, with an exact process-local request ID and
`Applied` or `DecodeRejected`. Android replaces any wire-provided tracking field
with its own monotonic ID, queues the projected snapshot, then observes receipts
in PostUpdate. IDs are not authentication or server-operation acknowledgements.

The runtime publishes Applied only after its existing `apply_world_snapshot`
updates world state and interpolation history. Invalid schema produces rejection
while retaining the previous valid state. Untracked legacy producers still use
the same world decoder and do not leave an old receipt associated with new data.
Only the latest consumed receipt is retained; no unbounded receipt history.

Android invalidates pending IDs at session/map boundaries and ignores mismatches.
Applied leaves the shell in StartingGame with an explicit asset-wait message;
DecodeRejected requests data reset/disconnect and clears unsent commands.
No Bootstrap success is emitted. Windows/Web producers need no new field.

Shared write set was declared before editing: `runtime/src/lib.rs` (resource and
ingest hook), new `runtime/src/native_world_receipt.rs`; Android writes are
`shared_shell.rs` and `world_projection.rs`. No Windows producer refactor.

## Tests

All commands use the Android manifest/cache and Rust 1.95.0 offline.

- Shared runtime `-p mir2-bevy-runtime -- --test-threads=1`: **215 passed**,
  `/tmp/android-receipt-runtime.log`.
- Android ordinary tests: **93 passed**, `/tmp/android-receipt-host.log`.
- Android `--features ui-preview`: **96 passed**,
  `/tmp/android-receipt-preview.log`.
- `MIR2_ANDROID_MODE=check .../build-android.sh`: API31 arm64 target check passed,
  `/tmp/android-receipt-target.log`.
- `git diff --check`: passed. Existing dependency warnings retained.

The new integration regression uses the production native queue and actual
`ingest_pending_world_state` Bevy system: enqueue has no receipt, two coalesced
snapshots acknowledge only the latest request after Update, coordinate 303 is
present in RuntimeWorldState, and a subsequent invalid coordinate is rejected
without changing it. Additional tests cover direct application, untracked legacy
input, exact/stale IDs, and Applied not unlocking the Android game surface.

Java unchanged and not rerun here. No Windows or browser build claimed.
No new APK, installation, screenshot, emulator or physical-device run. The last
ordinary APK remains the previous `af5acf6ad` build and does **not** contain this
increment; see the preceding snapshot-ingress evidence for its hash.

## Remaining milestone

This is headless world-data acceptance, not GPU/render acceptance or a real
Gateway session. Map/actor resource producers, scene MapModel/EntityModelSet,
textures, render-ready acknowledgement and incremental gameplay packet routing
still need connection. Continue those local implementation steps before opening
InGame. Preserve path validation and shared asset/protocol semantics.

Approved live test Gateway remains missing; it blocks live verification only.
Physical-device acceptance is deferred until the flow is playable. Existing
mail/native-heap growth remains open. No global completion claim.
