# Native Android authoritative entity packets (2026-09-11)

## Scope

Implementation commit `ab43c1d28c052590a6cc2d76c77d1c03492decfd` adds a
packet-first Android projection over the last complete authoritative world
snapshot. The Java TLS host forwards only the bounded entity allowlist after
the socket reaches `IN_GAME`; Rust validates the envelope again and updates the
shared renderer-neutral entity set. Movement, turn, attack-position, harvest,
spawn/refresh and removal packets no longer have to wait for the next periodic
world snapshot.

Supported spawn aliases are `ObjectPlayer` / `ObjectHero`, `ObjectMonster` /
`NewMonsterInfo`, and `ObjectNpc` / `NewNpcInfo`. Supported transform/lifecycle
packets are `UserLocation`, `ObjectWalk`, `ObjectRun`, `ObjectBackStep`,
`ObjectTurn`, `ObjectHarvest`, `ObjectHarvested`, `ObjectAttack`,
`ObjectStruck`, `ObjectDashAttack`, `ObjectRemove`, and `ObjectTeleportOut`.
All positions remain server-authored. The client does not predict actors,
invent object IDs, or write gameplay authority.

The cached Crystal entity render state is tied to the same native world request
as the neutral models. Existing sprite layers move/remove immediately and a
later render product is aligned to any packet-first positions before it is
published. A newly observed packet-only actor enters the neutral object layer
immediately; resolving a new Crystal sprite/atlas layer still waits for the
next complete authoritative render snapshot. Direction and action animation
frame re-resolution are also still snapshot-driven.

## Automated evidence

- `cargo +1.95.0 test --manifest-path Cargo.toml --quiet`: 113 passed.
- `./gradlew --no-daemon testDebugUnitTest --rerun-tasks`: 11 passed.
- `MIR2_ANDROID_MODE=check ./build-android.sh`: API 31
  `aarch64-linux-android` target check passed.
- Normal debug APK package gate passed from the exact implementation commit,
  with the approved local UI and bounded Bichon world packs staged outside Git.

APK (ignored build output):

- Path: `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
- Size: `330102029` bytes
- SHA-256: `cc5a6d1cca28820c57ac458ff6804f86e77e266301bc9ff662d1faa61f5d9ad0`

## Emulator evidence

The APK installed successfully on the existing ARM64 API 31 Pixel 5 AVD
(`1080x2340`, density `440`). A force-stop/cold launch resumed
`com.mir2.web3/.MainActivity` with PID `7863`; filtered logcat contained no
fatal exception, panic, `UnsatisfiedLinkError`, or app ANR. The screen showed
the shared Crystal login UI and `Test server not configured.` because this APK
intentionally contains no Gateway URL.

The local ignored screenshot is
`apps/game-client/platform-android/target/evidence/android-native-entity-packets-20260911/pixel5-api31-login-ab43c1d2.png`
(SHA-256 `3d35bf737eeebc15dac18c697859192e62b286f8b8acd01ec537ac7008f407de`).
It is launch evidence only and is not committed.

## Acceptance boundary

This closes a bounded Android entity-transform/object-list ingestion leaf. It
does not prove an approved-account login, a live StartGame/map transition, new
actor sprite readiness, direction/action animation parity, combat/UI packet
completeness, or the full player journey. No approved live WSS endpoint/account
was supplied, and no physical Android device is attached, so real login and
true-device touch/IME/background/reconnect/thermal acceptance remain open.
