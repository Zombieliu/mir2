# Native Android render-ready map transition evidence — 2026-09-11

Implementation commit: `32e7b9b660f147834734e5c0964c91078eab1df3`

## Scope

The Android host now treats an authoritative map boundary as a presentation
barrier. A changed snapshot map, or the live transport's `IN_GAME -> STARTING`
phase change, moves the existing shared shell to `StartingGame` before the
native scene is reset. Authentication, the selected/active character, and
personal models remain intact, while the old map/entity presentation is
cleared and unsent gameplay effects are no longer eligible to leave the
client during the transition.

The shell returns to `InGame` only after the existing exact-request
`NativeRenderReady` contract succeeds: the map and entity receipts must refer
to the same nonzero request and center, have no unresolved visible content,
include a visible self actor, and remain complete for two consecutive rendered
frames. Stale or partial receipts cannot release the barrier.

This is a code-level and offline-emulator rendering gate. There was no approved
Gateway endpoint/account in this run, so it is not evidence of a real login,
StartGame, or live map transition.

## Automated checks

- Android Rust tests: `113 passed; 0 failed`.
- Configured shared entity-atlas regression: `1 passed; 0 failed` (112 filtered
  out).
- Java MockWebServer TLS/protocol tests: `11 passed; 0 failed`.
- Rust Android target check: `aarch64-linux-android`, API 31, passed.

The MockWebServer tests exercise the bounded TLS/protocol host only. They do
not establish authentication against a real Gateway.

## Normal APK launch

- Artifact: `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
- Size: `330160237` bytes
- SHA-256: `7e45e8096ce824e34708daf8ccddde02cf912497b9d81f74a9fc9408e64242d5`
- Target: ARM64 API 31 emulator
- Package/activity: `com.mir2.web3/.MainActivity`
- Cold launch: `1342 ms`, PID `8834`

Logcat reached Bevy's Vulkan/window startup without a fatal exception, panic,
ANR, or `UnsatisfiedLinkError`. The emulator used SwiftShader software Vulkan.
The build had no configured Gateway URL and therefore proves launch only.

## Offline transition specimen

- Artifact: `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Size: `374467181` bytes
- SHA-256: `5684da40e22bc48c4e266d5e94a521367aab8094f2c1707149005ac2bb6b29be`
- Package: `com.mir2.web3.uipreview`
- Route: `--es ui_scene starting`
- Cold launch: `1218 ms`, PID `8921`
- Marker: `ANDROID_UI_PREVIEW_READY scene=starting`
- Screenshot: `api31-starting-preview.jpg`
- Screenshot SHA-256: `011201eda15390f34da92131384a61633d82dc00dd265e8215df03549f36be14`

The screenshot is labelled `OFFLINE UI PREVIEW — starting — NOT LIVE
GAMEPLAY`. It verifies that the shared loading surface is visible on the API 31
emulator; it is not a captured online map switch.

## Remaining acceptance gates

- Approved real `wss://` Gateway endpoint and test account.
- Real login -> roster -> StartGame -> authoritative scene -> map transition.
- Remaining gameplay packet/reducer and action-animation coverage.
- Physical Android device touch, IME, lifecycle, reconnect, performance, and
  soak acceptance.

No APK, credentials, signing material, asset pack, or cache is committed.
