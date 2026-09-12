# Native Android replacement-generation boundary evidence — 2026-09-13

## Scope

This proof covers Android's command/pending-operation boundary when a
`MapChanged` render transition or a network reconnect replaces the native
Gateway host generation. It does not exercise an approved WSS account or a
physical Android device.

## Result

- A lost host generation now drops every unsent command from that generation,
  including non-motion gameplay commands. None can be replayed against a new
  map, character, or account.
- Game Shop and Storage mutations that lose their transport generation are
  closed as unknown in both the shared UI and Android queue. Fresh requests use
  new correlation IDs after recovery.
- An in-flight change-password request is closed on connection loss; a late host
  write callback or authoritative result is unmatched and cannot complete a new
  transaction.
- The Java host policy was exercised continuously through initial snapshot,
  `MapChanged`, destination snapshot, disconnect, reconnect/login, and recovered
  snapshot. It invalidates each old live generation once and does not restart
  before a fresh authoritative snapshot is visible.

## Deterministic gates

- Rust formatting: pass.
- Android Rust `ui-preview`: **166/166 passed**.
- Java Debug unit tests: **23/23 passed**.
- Java UI Preview unit tests: **23/23 passed**.
- arm64-v8a native release build at Android API 31: pass.
- Debug and UI Preview APK assembly: pass.
- UI Preview streamed install and cold launcher start on API31: pass.
- Running Activity: `com.mir2.web3.uipreview/com.mir2.web3.MainActivity`.
- Emulator: `sdk_gphone64_arm64`, API 31, 1080x2340 at 440 dpi.
- Launch log reached `ANDROID_UI_PREVIEW_READY scene=hud`; the checked launch
  interval contained no fatal exception or Rust panic.

## APK artifacts

Artifacts remain untracked build outputs.

- Debug:
  `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
  - size: approximately 328 MiB
  - SHA-256: `9ed17b75e27203514f6a2a7047d14bcfc57c64b03d3bf63f9fa42039c97820db`
- UI Preview:
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
  - size: approximately 360 MiB
  - SHA-256: `0e928cc7b8265ea0716faf994ffb50779eca99330f4ee43bc787e5b66d252d73`

## Acceptance boundary

This is deterministic transport-contract and offline-emulator evidence. It does
not claim approved-WSS login, a live server map transition, mobile-network
recovery against the real Gateway, physical-device behavior, signing/store
readiness, or human acceptance. Those remain separate gates.
