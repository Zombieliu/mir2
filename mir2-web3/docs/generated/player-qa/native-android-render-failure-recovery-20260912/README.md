# Native Android render-load failure recovery — 2026-09-12

## Result

This Android-only slice started from
`codex/android-player-journey@5d96c7698369e93efc6219974790f1baf1f7635b`.
Every asynchronous packaged map/entity load event now carries the request ID
that created it. The shell updates the loading notice only for the exact
pending frame. A stale completion leaves a newer authoritative transition
alone; failure of the matching frame clears pending world/render state,
presentation caches and queued client intents, requests a host disconnect and
returns through the normal reconnect path. It cannot fabricate
`NativeRenderReady` or leave the shell indefinitely on `StartingGame`.

## Verification

- Rust formatting passes and Android `ui-preview` tests pass 154/154, including
  a new current-versus-stale failure recovery test (`gates.txt`).
- The API31 ARM64 target check passes (`gates.txt`).
- The API31 `uiPreview` package passes against the same ignored 35-atlas,
  102-page local entity pack and its exact manifest. The 375,687,300-byte APK
  has SHA-256
  `956eb27b73695a78061b3d4d17b5d63a0f6e4131072ca35994299c06dd7f70fd`.
  The APK and local packs remain ignored and uncommitted (`apk.txt`).
- Streamed install succeeds on the API31 ARM64 emulator (`emulator.txt`).
- The previous head's GitHub `Android Capacitor and native Bevy` lane passed
  (`ci.txt`).

## Emulator boundary

The same-slice pre-final capture process emitted
`ANDROID_UI_PREVIEW_READY scene=world-render`, but Emulator 31.3.10 with the
SwiftShader driver raised `DeviceLost` about 31 seconds later and Bevy exited
the render path. The captured frame is black and is retained only as negative
evidence (`world-render.png`, `emulator.txt`). The raw logcat remains ignored
and uncommitted. The final APK above was separately installed successfully; a
subsequent login-scene launch did not reach its ready marker within the bounded
window and produced no replacement capture. This run therefore does not add
visible-frame, soak, live gameplay or physical-device acceptance.

## Acceptance boundary

No approved WSS endpoint, test account or physical Android device was used.
The public Web entity manifest still differs from this checkout's tracked
Android input. This slice proves deterministic failure recovery and package
integrity only; real login, role selection, `StartGame`, live render-ready map
transition, gameplay, updated-emulator rendering, physical-device and human
acceptance remain open.
