# Native Android login host — 2026-09-08

Status: implementation and local fixture checks passed; **real Gateway login / StartGame / server-position acceptance is NOT complete**.

## Provenance and artifact

- Isolated branch: `codex/android-player-journey`, source base `50ac69dd3b9b7d94a612d7fe59fe6bf9526ddf55`; APK implementation commit `448857bd4` (the following evidence-only commit does not change the APK source).
- Original checkout was not switched, stashed, reset or cleaned. Existing changes were preserved. Windows gates are outside this round.
- Local APK: `mir2-web3/apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`, relative to repository root.
- SHA-256: `646de448164a3336999e5b91df842295a703495390e091cf2cf3a1b66c7aab82`.
- Size: 117,854,920 bytes. Debug-signed, arm64-v8a, package `com.mir2.web3`, versionCode 2, versionName `0.1.0-login`, minSdk 31 / targetSdk 35. Not a store/release signing artifact.
- Native GameActivity + Bevy, **not a remote WebView page**. Web-page and asset-release versions: not applicable to this text-only native milestone. No map resources rendered or production environment deployed.
- APK, native binaries, Gradle caches, signing keys and credentials are excluded from Git.

## Implemented scope

Android-owned OkHttp WSS host and native form send existing credential login, character selection and StartGame commands. Only LoginSuccess supplies selectable indices. IN_GAME requires StartGame result 4 and authoritative character/map/position. Packet updates and matching selfPlayer snapshots feed a bounded latest-value JNI text display in Bevy. No client simulation, invented auth, PasskeyLogin assertion or transaction replay was added.

TLS uses normal platform trust and hostname verification; no cleartext, URL credentials or redirect following. Test-only certificates are generated in JVM memory, never added to application trust. Passwords are cleared on submission/backgrounding; no credential/token persistence. Login screens use FLAG_SECURE. Endpoint preference is the only saved connection data.

Failure, timeout and backgrounding clear the socket, roster and position; foreground recovery requires manual reconnect and re-login. This is safe invalidation, **not automatic session resumption**. Gameplay queues remain disconnected from this login-only socket.

## Local checks

| Check | Result and boundary |
| --- | --- |
| Rust host tests | 56 passed, 0 failed (`cargo +1.95.0 test --locked --offline --manifest-path mir2-web3/apps/game-client/platform-android/Cargo.toml`) |
| Android native target | Locked/offline cargo check passed; release native library built with Rust 1.95.0, cargo-ndk 4.1.2, NDK 26.1.10909125 |
| Gradle native package | `MIR2_ANDROID_MODE=package` build passed; final `./gradlew --no-daemon testDebugUnitTest assembleDebug` passed with Java 21 |
| Java/TLS fixture tests | 8 passed, 0 failures/errors; see XML. Covers protocol sequence/position, rejected credentials, auth/roster gating, unsolicited positions, invalid endpoints, untrusted TLS, snapshot/StartGame ordering and malformed coordinates |
| API 31 install | Final APK installed successfully with `adb install -r`; installed version verified |
| API 31 connection failure | Dummy `wss://127.0.0.1:1/ws` only, no credentials: DISCONNECTED → CONNECTING → DISCONNECTED; login/world buttons disabled |
| API 31 background/foreground | Home then resume: same process 5959, HOT launch 54 ms; UI shows background invalidation and requires re-login. This did not test a live authenticated session |
| Real Gateway login and StartGame | **Not run**: approved test endpoint and manually entered test credentials not supplied |
| Physical Android device | **Not run**: no physical device available |

Device: existing `Pixel_5_API_31` emulator, Android 12 / API 31, arm64-v8a, landscape 2208×1080. Fingerprint: `google/sdk_gphone64_arm64/emulator64_arm64:12/SE1A.220630.001/8789670:userdebug/dev-keys`. Existing AVD was not wiped. Attached UI hierarchy was captured with empty credential fields; no login screenshot was collected because the credential screen prohibits captures. Phase-only log contains no account, password or raw packets.

Android CI now runs Java host tests in addition to its existing native cargo check. Existing CI APK packaging is Capacitor, not proof of a native Gradle APK build. CI outcomes must be checked on the new pushed SHA separately; local results above are not CI results.

## Remaining acceptance and next action

1. Supply an approved test WSS endpoint and have the user manually enter a test account/password in the native form. Do not send passwords in chat. Verify exact Gateway source/environment first.
2. Capture actual LoginSuccess, StartGame acceptance and native server-position display with credentials hidden. Compare displayed position to server evidence; do not substitute fixture success.
3. Then implement/test character creation, shared Zone movement and saved-position re-entry in a bounded follow-up. Full map rendering, real authenticated reconnect/background recovery and session persistence are not claimed here.
4. Separately record physical-device touch, keyboard, background, network-loss and rendering results. No production or real-save changes are authorized by this evidence.
