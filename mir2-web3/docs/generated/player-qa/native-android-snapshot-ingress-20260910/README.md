# Android authoritative snapshot ingress — 2026-09-10

Source: local `af5acf6ad`, branch `codex/android-player-journey`, isolated Android
worktree. No push, Windows/backend changes, production access or save mutation.

## What changed

Previously `GatewaySession` discarded the complete world snapshot after extracting
the self player's name/map/x/y. It now retains the immutable serialized payload,
delivers it once after StartGame acceptance, and clears pending snapshots on
position/information/map/session boundaries. The existing Java observer passes
this payload to JNI, without parsing display text or weakening login gates.

Android `world_projection.rs` adapts numeric actor IDs and movement times to the
existing shared runtime wire shape. It preserves scene/actor payloads and projects
HUD values through the actual shared `PlayerStats` serde type. It rejects mismatched
owner/name/map/position, duplicate actor IDs, invalid numeric IDs and malformed
HUD values. Nullable snapshot scalars use shared defaults, matching the existing
Windows producer's partial-snapshot treatment. Zero selected-object sentinel is
retained; zero owner/actor IDs are rejected.

The host uses existing `push_native_world_state` and `push_native_ui_read_model`
entry points, plus shared data/scene resets on lifecycle/map boundaries. JNI now
accepts bounded large snapshots instead of silently discarding everything above
64KiB. Gateway wire input is at most 1MiB UTF-8; the escaped host envelope is at
most 2MiB + 64KiB. The queue keeps its 32-event cap and an 8MiB payload budget
(snapshot bytes plus 64KiB allowance per event; not a measured process RSS cap).
Invalid/overflowing host input enters the disconnected shell state.

## Verification

- `cargo +1.95.0 test --offline --manifest-path apps/game-client/platform-android/Cargo.toml`:
  92 passed. Log: `/tmp/android-snapshot-rust.log`.
- Same command with `--features ui-preview`: 95 passed.
  Log: `/tmp/android-snapshot-preview.log`.
- Android Gradle `:app:testDebugUnitTest`, with existing SDK/JDK environment:
  8 TLS MockWebServer tests passed. Log: `/tmp/android-snapshot-java.log`.
  The expanded test verifies pre-ack suppression, complete actor retention,
  one-shot delivery, map/disconnect clearing and previously published immutability.
- Android API31 arm64 target check passed before the final nullable-field adapter
  addition; the final full package build must be used for exact-source evidence.
  Log: `/tmp/android-snapshot-target.log`.
- Existing dependency warnings and Java deprecated-API note remain; no dependency
  updates or warning-suppression changes.

## Exact-source APK

Final ordinary debug package build passed for source
`af5acf6adc13fc1b2e732af7df702c694ad1d735`, using `build-android.sh` with
`MIR2_ANDROID_MODE=package`, `MIR2_ANDROID_VARIANT=debug`, existing JDK21 and
`MIR2_ANDROID_UI_ASSET_ROOT=<Android crate>/target/shared-ui-assets`.
Log: `/tmp/android-snapshot-package.log`.

- Package: `com.mir2.web3`, arm64-v8a, native Bevy (not WebView).
- File: `/Users/henryliu/obelisk/numeron-worktrees/android-player-journey/mir2-web3/apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`.
- SHA-256: `a9e253b7ed2bd34395f77b17537834b59d3ff0026b76796039bcc7548ba350b3`.
- Not installed or launched in this round. Preview APK was not rebuilt.
- Existing external staged UI resources reused; no new world-resource release or
  Gateway environment/version verified. APK/resources are not committed.

## Acceptance boundaries and next work

These are host adapter/transport-fixture tests, **not an end-to-end shared-runtime
ingest acknowledgement or rendered-world test**. Enqueue success is not runtime
schema acceptance. The shell deliberately remains loading: no fake Bootstrap
success and no forced InGame transition are emitted.

Next connect the existing native map/entity asset producers and their bootstrap
readiness contract, including runtime decode/ingest acknowledgement and incremental
packet handling. Only then enable the game surface and authoritative player actions.
Raw map names from the server must not become unchecked filesystem paths. Reuse
shared assets/protocols and keep Android-only adaptations bounded.

Approved live HTTPS/WSS test Gateway is still missing. No real login, online
movement/combat/save restoration, emulator UI run or physical-device acceptance
is claimed here. Physical-device acceptance remains deferred until a playable
flow exists. The earlier mail/native-heap growth remains open, not the main task.
