# Native Android authenticated gameplay-socket checkpoint (2026-09-11)

## Scope and source

- Branch: `codex/android-player-journey`
- Implementation commit: `f1ee2148c`
- Host: the isolated macOS Android worktree; the original checkout was not
  switched, stashed, reset, or cleaned.
- Scope: connect the existing shared Android gameplay command queue to the
  Activity's authenticated WSS session. No Windows/backend changes,
  production deployment, account bypass, or live save mutation are included.

## Implemented

- `MainActivity` starts the native gameplay transport only after the ordinary
  login/roster/StartGame state machine reaches `IN_GAME`.
- The existing bounded Rust lease mailbox is exposed through JNI. Each
  Rust-produced BrowserCommand is copied as an immutable JSON envelope, sent
  on the current authenticated `GatewaySession`, and acknowledged once by its
  exact sequence number.
- A failed write, malformed envelope, background stop, socket disconnect or
  transport generation change closes the bridge. Sent-but-unacknowledged
  Storage/GameShop mutations become unknown and are never replayed.
- Android resume, pause, destroy, network-available and network-unavailable
  events now enter the typed `AndroidShellState` lifecycle reducer before the
  transport driver drains commands.
- The gameplay writer rejects version, heartbeat and authentication/session
  bootstrap commands, including `passkeyLogin`; those remain owned by the
  dedicated login state machine.

## Automated evidence

- `cargo +1.95.0 test --manifest-path Cargo.toml`: 110 passed, 0 failed.
- Java `GatewaySessionTest`: 9 passed, 0 skipped/failed/errors after a forced
  Gradle rerun. The added TLS MockWebServer case proves that an `attack`
  command is rejected before `IN_GAME`, a forged `login` is rejected after
  entry, and the unchanged `attack` object ID reaches the live test socket.
- `MIR2_ANDROID_MODE=check ./build-android.sh`: arm64 API 31 target check
  passed with Rust 1.95.0 and NDK 26.1.10909125.
- The packaged arm64 ELF exports all five JNI transport entry points:
  host start, host stop, connection lost, poll and exact-sequence report.
- `git diff --check` and platform-only `cargo fmt --check` passed.

The Java TLS fixture verifies the real socket state machine and exact outbound
shape, but it is not approved-account or deployed-Gateway acceptance.

## APK and API 31 launch

| Artifact | Result |
| --- | --- |
| Normal debug APK | `android/app/build/outputs/apk/debug/app-debug.apk` |
| Bytes | 330,085,565 |
| SHA-256 | `f9d369bd322fa3a3b03adda23f35dcfc40f5eaf4f42067e982eb66acdd7297dc` |
| Source | implementation commit `f1ee2148c` |

The APK contains the approved local UI/world assets outside Git and an empty
Gateway URL. It installed successfully with `adb install -r` on
`Pixel_5_API_31` (`sdk_gphone64_arm64`, arm64-v8a, API 31), stayed alive after
cold launch as PID 7355, and logged the native library load without a fatal
exception, panic or `UnsatisfiedLinkError`. The captured 2340x1080 frame shows
the shared Crystal login screen and the explicit `Test server not configured.`
configuration stop. Local screenshot:
`platform-android/target/evidence/android-native-gameplay-socket-20260911/pixel5-api31-login-f1ee2148.png`,
SHA-256 `2856fc0bf6ceabafbd7a66ad34e2ad6206bae969c9c126f3404cd97495fa356a`.

APK, assets, screenshot, logs, caches and debug signing material remain local
and ignored; none is committed to Git.

## Acceptance boundary and next work

| Gate | Status | Reason |
| --- | --- | --- |
| Shared gameplay command -> authenticated Android WSS | Implemented and automated | Exact leased write and fail-closed generation handling are covered |
| Normal APK API 31 cold launch | Passed | No endpoint/account was configured; this is a configuration-negative launch only |
| Real WSS login -> roster -> StartGame -> visible rendered world | **Not run / open** | No approved endpoint or test account was available |
| Authoritative gameplay packets/transaction receipts -> shared reducers | **Open** | Ordinary inbound gameplay projection is not yet fully routed |
| Online movement/combat/inventory/NPC/reconnect journey | **Open** | Requires inbound projection plus an approved live environment |
| Physical Android device acceptance | **Not run / open** | `adb devices -l` listed only the emulator |

This checkpoint closes the outbound authenticated-socket adapter, not the
complete online player loop, physical-device acceptance, global Candidate
completion, or human acceptance.
