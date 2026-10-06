# Native Android authoritative object metadata — 2026-09-13

## Scope

This slice carries the Gateway's existing authoritative actor-identity updates
through the Android host, object reducer and renderer-owned overlay.

- `ObjectName`, `ObjectColourChanged` and `ObjectGuildNameChanged` are accepted
  only after `IN_GAME` and use the same bounded entity packet queue as the
  other authoritative object mutations.
- Exact object identity is required. Names and guilds are length-bounded,
  empty guild clears the line, and the signed 32-bit server colour is retained.
- Visible and temporarily hidden cached actors receive the metadata. Unknown or
  tombstoned objects are ignored, and the reducer does not recreate sprites or
  invent identity state.
- The offline specimen sends the same three packet shapes after the initial
  render-ready frame, then reprojects the existing overlay. The visible player
  becomes `Packet renamed player`, guild `AUTHORITATIVE`, in the packet colour.
- Native state JSON and atlas pixels travel separately. An atlas-declared entity
  layer now waits for its uploaded image and retains the last complete actor
  composite instead of asking Bevy to load its diagnostic source path.

## Deterministic verification

- Rust formatting passed for the Android and shared runtime packages.
- Android Rust default suite: **177 passed, 0 failed**.
- Android Rust `ui-preview` suite: **185 passed, 0 failed**.
- Shared Bevy runtime suite: **234 passed, 0 failed**. The added regression
  proves an atlas-declared first frame remains absent until native pixels arrive
  and then binds `atlas:native:p0`, without a source-path fallback.
- Gradle Debug and UI Preview unit suites: **25 passed, 0 failed** in each
  variant. `GatewaySessionTest` includes the three exact forwarding cases.
- API 31 arm64 release packaging and ADB streamed installation passed.

## API 31 emulator evidence

The dedicated `Mir2_API_31_ARM64` AVD is Android API 31,
`sdk_gphone64_arm64`, arm64-v8a, density 440. The captured landscape frame is
2340x1080 and visibly contains the packet-updated magenta player name and guild
over the real packaged Bichon viewport.

- [world-render-metadata.png](world-render-metadata.png), SHA-256
  `aa8328415f4e7833b4a75e3b7cef56c2898c9413dbc0ff8a1f75e700a1025118`
- [world-render-metadata-logcat.txt](world-render-metadata-logcat.txt) contains
  `ANDROID_UI_PREVIEW_READY`,
  `ANDROID_OBJECT_METADATA_PRESENTATION_APPLIED` and
  `ANDROID_WORLD_RENDER_READY`.

The ready frame reports 849 map draws, seven entities and twelve entity layers.
The final 57,991-byte log has no match for `Path not found`, `PathNotFound`,
`panicked at` or `FATAL EXCEPTION`.

## APK

- Path:
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Bytes: `444430024`
- SHA-256:
  `2bc3bf6d4eef2bcab97e40cb8cbfa694ed4ada25900e353814256895da18020c`
- Entity override pack ID:
  `android-archer-mount-bow-proof-20260912`

The APK, source asset packs, decoded resources and build caches remain outside
Git.

## Acceptance boundary

This is deterministic packet-reducer and offline API31 emulator rendering
evidence. No approved WSS endpoint, test account or physical Android device was
available, so it does not prove real login, online StartGame/render-ready,
public Web/Android asset-release alignment, physical-device behavior or final
human acceptance.
