# Native Android GameShop and BigMap phone focus — 2026-09-13

## Scope

This proof keeps the shared Crystal GameShop and BigMap implementations and
adds Android host focus to their real shared roots. Both blocking panels grow
within the safe landscape viewport and temporarily hide the desktop HUD/chat
tree so it is neither visible nor pickable through transparent artwork.
Closing either panel restores that same shared gameplay chrome.

The GameShop root no longer clips its transformed children against its
pre-transform bounds. Its original `Title/749` artwork, eight-item page,
selection, quantity controls, paging and footer therefore render together at
phone sizes. The preview provides twelve bounded synthetic products and 500
offline credit only to exercise shared local UI state. Buy was not tapped and
no transaction was sent or claimed.

The BigMap fixture uses the packaged Crystal world-map artwork and packaged
`original-ui/MMap/412.png` local map. It provides 22 synthetic NPC rows so the
shared selection, scrolling, world/local switch and search paths can be
exercised. While the landscape Android IME is open, the shared search field is
panned above the keyboard; hiding the IME restores the full panel. Go To was
not tapped and no teleport was sent or claimed.

## Verification

- Android Rust UI Preview suite: 175/175 passed.
- Focused GameShop/BigMap fixture and BigMap IME geometry tests: passed.
- Gradle `testDebugUnitTest` and `testUiPreviewUnitTest`: passed in the native
  Android project under Android Studio JBR 17 and the configured Android SDK.
- arm64-v8a native release build at Android API 31 with `ui-preview`: passed.
- Full licensed-asset package and streamed install: passed.
- Both scenes reached `ANDROID_UI_PREVIEW_READY` at 2340x1080, 1920x1080 and
  1600x720. Captured logs contained no panic, fatal exception, ANR or SIGSEGV.
- The emulator size override was reset after capture; `wm size` again reports
  its physical 1080x2340 profile.

## Visible and touch evidence

The API31 ARM64 emulator uses density 440 and fingerprint
`google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`.

- GameShop open: [2340x1080](gameshop-2340x1080-open.png),
  [1920x1080](gameshop-1920x1080-open.png), and
  [1600x720](gameshop-1600x720-open.png).
- Real ADB taps selected the second product, increased its quantity and moved
  from the first eight-item page to the remaining four-item page:
  [selected](gameshop-1600x720-selected-second.png),
  [quantity two](gameshop-1600x720-quantity2.png), and
  [page two](gameshop-1600x720-page2.png).
- The shared close button restored HUD, chat and mobile world controls:
  [GameShop closed](gameshop-1600x720-closed.png).
- BigMap open: [2340x1080](bigmap-2340x1080-open.png),
  [1920x1080](bigmap-1920x1080-open.png), and
  [1600x720](bigmap-1600x720-open.png).
- Real ADB taps selected an NPC row, scrolled the 22-row list, switched to the
  packaged world artwork and returned to the packaged local map:
  [selected](bigmap-1600x720-selected-second.png),
  [scrolled](bigmap-1600x720-scrolled.png),
  [world](bigmap-1600x720-world.png), and
  [location](bigmap-1600x720-location.png).
- The real Android IME opened on the shared search field, accepted `22` and
  filtered the shared list. Android Back restored the full panel, and the
  shared search action remained tappable: [IME](bigmap-1600x720-search-ime.png),
  [typed](bigmap-1600x720-search-typed.png),
  [restored](bigmap-1600x720-search-restored.png), and
  [filtered](bigmap-1600x720-search-filtered.png).
- The shared close button restored the gameplay UI:
  [BigMap closed](bigmap-1600x720-closed.png).

## APK artifact

The APK remains an ignored build output and is not committed:

`apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`

- bytes: 516,903,659
- SHA-256: `ca1f5b625cdb896c3d32576c13739bbf5eb822a2df517ea435de8cc9c0f8db2e`
- package/version: `com.mir2.web3.uipreview` / `0.1.0-shared-ui`
- UI source: the same local complete shared Crystal UI staging pack used by
  the preceding Android UI proofs
- world source: local keyed Bichon proof pack
- entity source: ignored `local-player-shards-20260912` proof pack

## Acceptance boundary

This is explicitly labelled offline UI Preview evidence on an API31 emulator.
The packaged map artwork is real, but the catalog, player state and NPC list
are synthetic fixtures. It proves Android geometry and local shared-UI picking;
it does not prove an approved HTTPS/WSS endpoint, real authentication/roster,
StartGame, server-confirmed GameShop or teleport operations, a live render-ready
map transition, physical-device touch/IME/network/GPU behavior, signing/store
readiness, soak, or final human acceptance.
