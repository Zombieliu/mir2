# Android macOS packaging baseline — 2026-09-04

## Scope and provenance

This evidence records the first bounded Android handoff milestone: macOS environment discovery and a reproducible Capacitor debug APK build. It does not claim emulator or physical-device acceptance, a working player journey, a published Web revision, or completion of the native Bevy Android client.

- Repository: `Zombieliu/mir2`
- Source branch: `codex/windows-player-journey`
- Source commit: `58eab9a4a6684b2d2d11d2c533049b64995e7458`
- Target branch: `codex/android-player-journey`
- Worktree: `/Users/henryliu/obelisk/numeron-worktrees/android-player-journey`
- Project: `mir2-web3/apps/mir2-mobile`
- Build date: 2026-09-04 (Asia/Shanghai)
- Git state before the build: clean for tracked files at the source commit
- Source changes in this milestone: none; this evidence document is the only intended commit content

The existing checkout at `/Users/henryliu/obelisk/numeron` was not reset, cleaned, stashed, or otherwise modified by this work.

## Environment baseline

| Component | Observed value |
| --- | --- |
| Host | macOS 15.7.8 (24G806), arm64 |
| Node.js | v23.11.0 |
| npm | 10.9.2 |
| Java used for Gradle | Eclipse Temurin 21.0.12 |
| Gradle wrapper | 8.11.1 |
| Android Gradle Plugin | 8.7.2 |
| Android SDK | `/Users/henryliu/Library/Android/sdk` |
| Android platform / compile SDK | 35 |
| Installed Android build tools | 29.0.2, 30.0.2, 30.0.3, 32.0.0, 34.0.0 |
| Capacitor CLI | 7.6.8 |
| Available AVD | `Pixel_5_API_31` |
| Connected Android devices | none |

The CI workflow currently selects Node.js 22 and Temurin 21. This local baseline therefore matches CI Java but has a Node.js major-version divergence (local 23 versus CI 22). The default shell Java was Zulu 8, so the Gradle invocation explicitly selected the installed Temurin 21 runtime.

The SDK command-line tools emitted a non-blocking warning that they understand repository XML schema version 3 while encountering version 4. No SDK component installation or project-file change was required for this build.

## Network and configuration boundary

The mobile shell embeds a remote Web URL. A normal build without overrides would use the repository defaults, which point at production-facing endpoints. This packaging-only baseline intentionally replaced both values with reserved, non-routable `.invalid` hosts:

```text
MIR2_MOBILE_GAME_URL=https://android-baseline.invalid
MIR2_GATEWAY_WS_URL=wss://gateway.android-baseline.invalid/ws
```

The variables were provided only to the Web build and Capacitor sync commands; they were not exported as persistent machine configuration. The resulting APK must not be treated as a functional test candidate because it cannot load a real game or Gateway. No production deployment, production login, authentication bypass, real save mutation, or production player-loop test was performed.

## Commands and results

Run from `mir2-web3/apps/mir2-mobile` unless noted otherwise:

```text
npm ci
  PASS — 93 packages added; 94 packages audited
  NOTE — npm reported one moderate vulnerability; no audit fix or lockfile change was made

npm test
  PASS — 3/3 tests

MIR2_MOBILE_GAME_URL=https://android-baseline.invalid \
MIR2_GATEWAY_WS_URL=wss://gateway.android-baseline.invalid/ws \
npm run build
  PASS — generated www/index.html

MIR2_MOBILE_GAME_URL=https://android-baseline.invalid \
MIR2_GATEWAY_WS_URL=wss://gateway.android-baseline.invalid/ws \
npx cap sync android
  PASS — Web assets and Capacitor Android configuration synchronized

cd android
JAVA_HOME=/opt/homebrew/opt/openjdk21-local/Contents/Home \
ANDROID_HOME=/Users/henryliu/Library/Android/sdk \
ANDROID_SDK_ROOT=/Users/henryliu/Library/Android/sdk \
./gradlew assembleDebug
  PASS — BUILD SUCCESSFUL; 85 actionable tasks executed
```

Gradle emitted the existing `flatDir` repository warning and Java unchecked-operation warnings. They did not fail the debug build.

## APK evidence

- Local artifact: `/Users/henryliu/obelisk/numeron-worktrees/android-player-journey/mir2-web3/apps/mir2-mobile/android/app/build/outputs/apk/debug/app-debug.apk`
- APK SHA-256: `762d897ed6b6f8fe5b2e382f564081a262ca4d06eb2128b5dacf0700f32e0d78`
- Generated `www/index.html` SHA-256: `e485985c243ca0c7e02fd306e0330d254f272c4a420f7cf7c058b2ba2ac38f98`
- APK `assets/public/index.html` SHA-256: `e485985c243ca0c7e02fd306e0330d254f272c4a420f7cf7c058b2ba2ac38f98`
- Package: `com.obelisklabs.mir2`
- Launchable activity: `com.obelisklabs.mir2.MainActivity`
- Version: `1.0` (`versionCode` 1)
- Minimum / target SDK: 23 / 35
- Debuggable: yes
- Declared network permission: `android.permission.INTERNET`
- APK signature schemes verified: v1 and v2
- Signature type: Android debug certificate, not a release/store signature
- Signer certificate SHA-256: `7e1c7a01d4e68e75c49d600132556b2bb53d5a9c497c7ce1ba609b78c40ab014`

The generated APK, `node_modules`, Gradle outputs, caches, passwords, and signing keys are not committed to Git.

## Explicitly unaccepted

- APK installation and launch
- Emulator rendering or input
- Physical Android device coverage and device details
- Login and character selection
- Bichon map entry and authoritative movement
- Joystick walk/run, target selection, combat, pickup, inventory, equipment, and NPC dialogue
- Background/resume, offline/reconnect, and exit/re-entry state checks
- Actual Web release, asset release, and Gateway version provenance
- Network behavior against an approved test environment
- Screenshots, recording, performance, thermal, and soak evidence
- Native Bevy Android transport and host integration

## Next bounded milestone

Obtain approved non-production HTTPS and WSS endpoints together with traceable Web, asset, and Gateway revisions. Rebuild the APK with those exact values, then install and exercise it on the available emulator before starting physical-device acceptance. Keep the Capacitor player-loop baseline distinct from the still-incomplete native Bevy Android implementation.
