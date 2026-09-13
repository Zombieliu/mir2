# Android shared Crystal shell UI — 2026-09-08

## Scope

User correction: prioritize the shared UI, not a parallel Android login form.
Android now registers Windows' existing `Mir2NativeShellUiPlugin` and
`NativeShellModel`. The new `native-shell-ui` feature exposes the same shell
without requiring desktop gameplay/audio; Windows `native-ui` remains a
superset with unchanged feature dependencies. Shared renderer/layout bodies
were not forked. Only the root marker was exposed for host viewport fitting.

Removed Java's visible login/select panel and the custom Bevy status label.
Java retains TLS transport, lifecycle and a transparent 1-pixel IME connection.
Visible fields, focus, buttons, notices and character selection use shared UI.
The host forwards bounded events/intents; roster class/gender/level are retained.
No test characters or successful connection are invented in the application.

## Artifact and resources

- Branch `codex/android-player-journey`; base before this change `271b4076375b633bc31ad5be7fc781f97b0c62c9`. The commit containing this report identifies the UI implementation.
- Native APK: `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`, relative to `mir2-web3`.
- SHA-256: `3ee643ae4df13a6dd3b022618c12b90b90e8df078e0105e2b7383b149381f167`.
- Size: 208,858,289 bytes. Debug signed, arm64-v8a, `com.mir2.web3`, version 3 / `0.1.0-shared-ui`, minSdk 31 / targetSdk 35.
- Native UI, no remote webpage. No Gateway configured and no production access.
- Initial local Web image subset lacked `Prguse/360.png`. Re-exported the four original libraries from existing local `downloads/crystal-client-full/Data` into this worktree's generated `platform-android/target/shared-ui-assets`, using the existing `apps/web/scripts/export-crystal-ui.mjs` with `--libraries ChrSel,Prguse,Prguse2,Title --fullLibraries ChrSel,Prguse,Prguse2,Title`. Original checkout and source resources were not modified.
- 6,094 packaged PNGs. `ui-assets.sha256` manifest SHA-256: `8e199d2d7d877a31e5fad227b3630f33515bb2fca7b7799ceade0916a887d8a6`. Source library hashes are in `source-libraries.sha256`. No resource binaries, APKs, caches or keys are committed.
- Build with `MIR2_ANDROID_UI_ASSET_ROOT=<generated asset root>` and `MIR2_ANDROID_MODE=package`; optional approved `MIR2_GATEWAY_WS_URL` is build-time only. Gradle rejects missing core login/error resources rather than shipping an empty UI.

## Verification

| Check | Result |
| --- | --- |
| Android host tests | 57 passed, including shared-model event/intent/roster/reset test |
| Shared client with shell feature | 241 passed; includes shell, original geometry and asset-path tests (49 shell-focused tests) |
| Java/TLS tests | 8 passed; fixture-only, not real authentication |
| Native arm64 package | Locked/offline Rust release and Gradle debug package passed |
| Emulator install/cold launch | Final APK installed, COLD 683 ms, process 6946 |
| Background/resume | Same process, HOT 49 ms, shared Connection Lost dialog; touch OK returns to actual shared login |
| Login and keyboard | Original 1024x768 stage uniformly fitted to 2340x1080, no stretching. Field tap opens keyboard; full-screen extract mode disabled; stage pans above IME without shrinking field size. Dismissal restores stage |
| Text input | Synthetic account text `uiqa` entered then erased; no password or real account entered. Shared privacy protection returns to clear capture after emptying field |
| Asset/runtime errors | Initial missing error-panel image fixed from original source. Final exercised login/resume paths show no `Path not found`, panic or Java fatal exception |

Device: existing Pixel_5_API_31 emulator, Android 12, arm64-v8a. No wipe.
Screenshots were visually inspected. The keyboard screenshot contains empty
fields, no credentials. Empty shell capture is permitted; entered credentials,
SafeKey and password-change surfaces use FLAG_SECURE.

## Remaining acceptance

- Character selection is shared and model-tested but has **not** been visually exercised with a real server roster on the emulator. No fake login is used to present it as live evidence.
- Real login/StartGame acceptance still needs an approved Gateway and user-entered test credentials. Map/HUD/gameplay wiring is not delivered by this shell correction. Transport position acknowledgement remains on the shared transition surface, not a fabricated in-game scene.
- Registration/create/delete/change-password wire operations remain unsupported in this Android slice and report failure through the shared model. No new server rules were added.
- Physical-device keyboard/touch target ergonomics, CJK composition, navigation variants, live reconnect and full mobile player loop remain open.
- Windows full gates and production deployment were not run or changed. These local results are not new-SHA CI results.
