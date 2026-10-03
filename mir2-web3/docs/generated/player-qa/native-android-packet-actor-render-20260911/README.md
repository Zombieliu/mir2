# Native Android packet-only actor materialization (2026-09-11)

## Scope

Implementation commit `ed045fcfb1db3ba8e597f1c4f4abafcd3ea50a2a`
extends the Android entity-render producer and packet-first cache. Every
render-ready snapshot actor retains a bounded Android-private descriptor of its
canonical kind, class, dead state and complete sprite contract. When a later
authoritative `ObjectPlayer`, `ObjectHero`, `ObjectMonster`, `NewMonsterInfo`,
`ObjectNpc`, or `NewNpcInfo` packet introduces another actor with the exact same
descriptor, the cache can clone the already decoded standing render state.

The fast path is limited to the current render viewport (`dx <= 24`,
`dy <= 32`), rewrites every active and cached-facing layer key to the new
server-authored object id, forces `isSelf=false`, applies the authoritative
position/facing and Crystal tile depth, and preserves the 8,192-entity and 64
MiB JSON bounds. Prototype descriptors and alternative facings are stripped
before the runtime payload is serialized. No image file or atlas page is
decoded on the packet path.

An unmatched or malformed sprite descriptor cannot borrow a visually similar
actor. Its renderer-neutral object model still updates, but its Crystal sprite
waits for the next complete authoritative render snapshot.

## Verification

- Android Rust tests: 112/112 passed, including an exact-contract
  `NewMonsterInfo` spawn which verifies authoritative id/position/facing,
  rewritten layer identity, runtime sidecar stripping, and removal.
- Configured real-atlas direction test: 1/1 passed.
- Java MockWebServer TLS `GatewaySessionTest`: 11 passed, 0 skipped/failures/
  errors.
- API 31 `aarch64-linux-android` target check passed.
- Normal debug APK package gate passed from the exact implementation commit.
- APK path (not tracked):
  `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`.
- APK size: `330159581` bytes.
- APK SHA-256:
  `cc98939cdb676e56ec86f0d3edfc58f84265819606d859d68dc2168ee3166ca9`.
- The APK installed on the existing ARM64 API 31 emulator and cold-launched
  `com.mir2.web3/.MainActivity` successfully in 834 ms with PID `8556`.
  Filtered logcat showed the Bevy Vulkan renderer and window starting without a
  fatal exception, panic, app ANR, or `UnsatisfiedLinkError`.
- [`api31-cold-launch.jpg`](api31-cold-launch.jpg) is a 1170x540 copy of the
  emulator screenshot (SHA-256
  `e44b65e937469b2ad31bee42e5ebc19eb6566740665bdc207200db4ea627ee20`).
  It shows the normal APK's visible shared Crystal login screen and its explicit
  `Test server not configured` state.

## Acceptance boundary

This closes only exact-contract packet actor materialization for already loaded
standing sprite assets. It does not resolve a new sprite contract, implement
walk/run/attack frame animation, or prove a render-ready map transition. The
normal APK contains no approved Gateway URL, account, or credentials, so the
emulator screenshot is visible-frame evidence only—not real login, StartGame,
online object-layer, reconnect, persistence, or full player-loop acceptance.
No physical Android device was connected; touch, IME, background/resume,
network handover, thermal/performance, and real-device rendering remain open.
