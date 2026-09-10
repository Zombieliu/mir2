# Native Android / Windows sync checkpoint — 2026-09-11

## Provenance

- Android branch before sync: `codex/android-player-journey` at `14aeca90c`.
- Windows source head: `41a3101924aabd63f000f11c155191138e982dc7`.
- Windows commits after the original Android handoff base `58eab9a4a6684b2d2d11d2c533049b64995e7458`:
  - `d1007a349` — shared monster updates and Windows gameplay fixes.
  - `41a310192` — Windows UI and shared gameplay parity checkpoint.
- Local rollback branch: `codex/android-player-journey-pre-windows-sync-20260911` at `14aeca90c`.
- Integration method: a normal non-fast-forward merge, without reset, clean, stash, rebase, or force push.

The repository is a partial clone. Only blobs required by the sparse Android/shared-client checkout were recovered; the complete external resource pack was not fetched or added.

## Conflict and compatibility resolution

Five textual conflicts were resolved in `audio.rs`, `overlays.rs`, `widget.rs`, `lib.rs`, and `CRYSTAL-1TO1-ROADMAP.md`.

- Android keeps the visual-only `native-player-ui` path and its bounded UI-audio intent queue; Windows `native-ui` still adds actual audio playback and the login-door clip.
- Newly shared friend, keyboard, ranking, relationship, hero-buff, and status UI modules compile under `native-player-ui`; they no longer require the desktop audio feature.
- Guild notice serialization preserves the source/server line contract, while Android IME avoidance uses the eight visible editor rows rather than the larger protocol limit.
- The Android host now expects the shared 1.8-second login-door transition before character selection.
- Chat settings tests register the Bevy mouse-wheel message used by the pointer-scroll system, retaining same-frame tab presses across model invalidation.
- Android `Cargo.lock` records the merged client's `unicode-segmentation` dependency.

## Verification

All commands used Rust `1.95.0`.

| Gate | Result | Local log |
| --- | --- | --- |
| Android host tests | 94 passed, 0 failed | `/tmp/android-windows-sync-full.log` |
| Android `ui-preview` tests | 97 passed, 0 failed | `/tmp/android-windows-sync-preview.log` |
| Shared client `native-player-ui` suite | 788 passed, 0 failed | `/tmp/android-windows-sync-client-player-ui-rerun.log` |
| Bevy runtime, serial | 226 passed, 0 failed | `/tmp/android-windows-sync-runtime.log` |
| Android arm64 target check | API 31 passed with NDK `26.1.10909125` | `/tmp/android-windows-sync-api31.log` |

The shared-client suite used the exact merged source in a temporary manifest projection with the unused desktop-audio feature edges omitted and the Android lockfile copied in. This was necessary because the local Cargo sparse index did not yet contain the Windows-only `bevy_audio 0.19.0` and `pkg-config 0.3.34` records, and live index refresh repeatedly timed out. The exact Android manifests and production feature path were independently compiled by both Android test gates and the API 31 check.

The full desktop `native-ui` audio gate was therefore not rerun on this Mac. A repository-wide `cargo fmt --check` also reports pre-existing formatting drift in the imported Windows files; those unrelated files were not mechanically rewritten during this Android integration.

## Acceptance boundary

This checkpoint proves source integration, shared UI behavior, runtime regression tests, and Android API 31 cross-compilation only. It does **not** prove a new APK install, emulator launch, approved-Gateway login, online player loop, Android real-device behavior, signing/store readiness, or production acceptance.

The first push attempt over the configured SSH remote produced no response and was stopped without changing the remote. Push status must be re-verified against the final merge commit before this checkpoint is treated as published.
