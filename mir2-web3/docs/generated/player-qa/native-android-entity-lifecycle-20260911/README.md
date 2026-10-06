# Native Android authoritative entity lifecycle evidence — 2026-09-11

Implementation commit: `35f12e4d3465b189d045bfc0008cf1853403c292`

## Scope

The real Android `GatewaySession` now forwards the bounded entity-lifecycle
family only after an authenticated session has reached `IN_GAME`:

- `ObjectHealth`, `Death`, `ObjectDied`, `Revived`, and `ObjectRevived`;
- `ObjectHide`, `ObjectShow`, `ObjectTeleportOut`, and `ObjectTeleportIn`;
- `ObjectRangeAttack`, in addition to the existing movement/melee packets.

The Android-private packet cache applies health/death state, authoritative
death position/direction, and dead/live opacity to the existing shared object
and native Crystal render models. Hide and teleport-out retain the exact actor
privately while removing it from both visible models; show and teleport-in can
restore only that retained actor. `ObjectRemove` is terminal until a later
authoritative spawn packet clears its tombstone. Periodic full snapshots cannot
resurrect a hidden or removed object, and the lifecycle cache is bounded by the
same 8,192-object limit as the accepted world model.

This closes lifecycle visibility/state projection only. Action-frame playback,
damage/health-bar effects, ground items, remaining gameplay reducers, and
online player-loop acceptance remain separate work.

## Checks

- Android Rust suite: `114 passed; 0 failed`.
- Configured shared entity-atlas regression: `1 passed; 0 failed` (113
  filtered out).
- Java MockWebServer TLS/protocol suite: `11 passed; 0 failed`.
- Rust Android target check: `aarch64-linux-android`, API 31, passed.

The JVM suite verifies the post-`IN_GAME` packet allowlist over an in-memory
TLS fixture. It is not a real Gateway/account test.

## Normal APK launch

- Artifact: `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
- Size: `330166477` bytes
- SHA-256: `4b05008b022643c70f2213f5abf7938b53f43e16926ef73400c2ec7fc5f416a9`
- Target: ARM64 API 31 emulator, 2340 x 1080 window
- Package/activity: `com.mir2.web3/.MainActivity`
- Install: passed with `adb install -r`
- Cold launch: `1014 ms`, PID `9229`

Logcat reached Bevy Vulkan/window startup with no fatal exception, panic, ANR,
or `UnsatisfiedLinkError`. The emulator uses SwiftShader software Vulkan. This
APK had no approved Gateway URL/account, so the launch proves neither real
login nor live lifecycle packets.

## Remaining gates

- Packet-driven Crystal action animation and effects.
- Ground-item/gold object rendering and pickup feedback.
- Remaining inventory, NPC, chat, combat, and reconnect reducers.
- Approved real login -> roster -> StartGame -> online object lifecycle.
- Physical Android device acceptance and soak.

No APK, credentials, signing material, licensed asset pack, or cache is
committed.
