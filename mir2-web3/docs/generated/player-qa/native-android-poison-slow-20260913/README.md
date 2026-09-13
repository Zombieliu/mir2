# Native Android Crystal Slow cadence and level-effect state — 2026-09-13

## Scope

This bounded slice completes the animation-cadence half of Android's existing
authoritative `ObjectPoisoned` presentation. Crystal calls its slow-aware frame
advance only for walking, running, mounted movement, sneaking and dash attack.
The Android action catalog currently exposes walking, running and dash attack,
so those exact actions now use twice their normal frame interval while the
server Slow flag (`4`) is present. Attack, spell, harvest, struck and lifecycle
actions remain at their catalog interval.

Applying or clearing Slow during an active movement preserves the current
frame and its fractional progress. It changes the remaining time to the next
frame without teleporting the animation forward, restarting the actor, or
changing the authoritative endpoint. New movement actions read the latest
cached poison state. Existing visible/hidden identity, tint and removal rules
remain unchanged.

The behavior is pinned to Crystal commit
`0e315fe327192afe52c3d7357ddd1f5b7e26c5b8`:

- [movement actions use the slow-aware frame update](https://github.com/Suprcode/Crystal/blob/0e315fe327192afe52c3d7357ddd1f5b7e26c5b8/Client/MirObjects/PlayerObject.cs#L2306-L2327)
- [Slow skips every other movement frame update](https://github.com/Suprcode/Crystal/blob/0e315fe327192afe52c3d7357ddd1f5b7e26c5b8/Client/MirObjects/PlayerObject.cs#L3498-L3509)
- [Slow is PoisonType bit 4](https://github.com/Suprcode/Crystal/blob/0e315fe327192afe52c3d7357ddd1f5b7e26c5b8/Shared/Enums.cs#L1009-L1024)

The same slice forwards `ObjectLevelEffects` only after `IN_GAME` and retains
its unsigned 16-bit flags on the exact existing actor. Initial spawn,
visible/hidden lifecycle, duplicate suppression and unknown/removed/ground-drop
identity isolation are covered. This creates no visual entry without the exact
Crystal assets.

## Deterministic verification

- Android Rust default suite: **181 passed, 0 failed**.
- Android Rust `ui-preview` suite: **189 passed, 0 failed**.
- The focused reducer test proves normal walking, mid-frame Slow apply,
  mid-frame clear, Slow already active at movement start, and an attack that
  remains at full cadence.
- A second focused reducer test proves bounded `ObjectLevelEffects` parsing,
  initial spawn state, visible/hidden updates, duplicate suppression and
  fail-closed identity behavior without a render placeholder.
- Gradle Debug and UI Preview unit suites: **25 passed, 0 failed** in each
  variant; the TLS fixture forwards the exact packet only after world entry.
- `aarch64-linux-android` API 31 target check passed with Rust 1.95.0 and NDK
  26.1.10909125.
- Release Rust plus `uiPreview` packaging passed with the same approved local
  UI/world/entity proof packs used by the preceding object-layer evidence.
- Streamed install and cold launch passed on `Mir2_API_31_ARM64`: API 31,
  `arm64-v8a`, `sdk_gphone64_arm64`, density 440. The app produced a 2340x1080
  PNG screen with SHA-256
  `4f059e2bf69cd045f11d722a43d8c022f6b6cb0a0f4f29689c05a21340e0e2ff`.
  Logcat reached `ANDROID_UI_PREVIEW_READY scene=world-render` and
  `ANDROID_WORLD_RENDER_READY` with 849 map draws, seven actors and twelve
  layers, with no matched missing-path, panic or fatal-exception line.
- Rust formatting and `git diff --check` passed.

## APK

- Path:
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Bytes: `444412912`
- SHA-256:
  `d0dbbcb4e46a8dc2c6b2ff260238478732b1edba809f07d49c3b344c080490aa`
- Entity override pack ID:
  `android-archer-mount-bow-proof-20260912`

The APK and source asset packs remain outside Git. The static emulator frame
proves that this package still reaches the existing offline render-ready world;
the Slow timing assertion itself is deterministic reducer evidence, not a
human-reviewed video acceptance.

## Explicit resource boundary

The `ObjectLevelEffects` transport and authoritative cache are implemented,
but its visual presentation is not. Crystal maps its nine flags to repeating
`Effect` and `Magic3` sequences. The checked-in manifests
contain zero frames from every required range: Effect 296-327, 990-1009,
1020-1051, 1210-1229 and 1240-1271; Magic3 6800-6819, 6840-6861,
6870-6888, 6906-6924, 6930-6958, 6970-6995, 7000-7020, 7040-7070,
7080-7103, 7120-7150 and 7160-7183. No marker or unrelated effect is used as
a substitute.

No approved WSS endpoint, test account or physical Android device was
available. This evidence does not claim real login, online StartGame,
public-asset alignment, physical-device behavior or final human acceptance.
