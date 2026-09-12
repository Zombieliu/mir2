# Native Android packet-first entity facing (2026-09-11)

## Scope

Implementation commit `029dc5e3f` extends the Android entity-render producer
and packet-first cache. For each visible actor whose current authoritative pose
resolves, the producer pre-resolves all available Crystal standing directions
from the packaged immutable atlas. The sidecar stays Android-private and is
stripped from every runtime payload. Later authoritative movement/turn/action
position packets can therefore swap an existing actor to the matching facing
without decoding images again or waiting for a periodic world snapshot.

Position updates also move every cached facing and update layer depth with the
same Crystal tile ordering formula. Actor identity remains server-authored; a
remote refresh cannot demote the existing self actor to a remote player. All
JSON, entity/layer counts and combined sidecar/runtime bytes remain bounded.

The checked real shared atlas resolves all eight standing directions for the
fixture player. Those directions span two existing 2048x2048 atlas pages (32
MiB decoded RGBA total), still inside the 128 MiB selected-page budget. The
runtime receives only the active direction plus the atlas descriptors; cached
alternatives are not duplicated in the Bevy runtime model.

## Verification

- Android Rust tests: 112/112 passed.
- Configured real-atlas direction test: 1/1 passed.
- Java MockWebServer TLS tests: 11/11 passed.
- API 31 `aarch64-linux-android` target check passed.
- Normal debug APK package gate passed from the exact implementation commit.
- APK size: `330133021` bytes.
- APK SHA-256: `e26b09fc38df9e2d77e6a8c7d5b1da2bcf900aae743e3a2811ba4e4eabe926d0`.
- The APK installed on the existing ARM64 API 31 Pixel 5 AVD and force-stop /
  cold-launched `com.mir2.web3/.MainActivity` with PID `8140`; filtered logcat
  showed no fatal exception, panic, `UnsatisfiedLinkError`, or app ANR.

## Acceptance boundary

This closes packet-first standing-facing and y-sort freshness for actors that
were already part of a complete render snapshot. A packet-only new actor still
waits for the next complete snapshot to obtain its Crystal sprite contract.
Walk/run/attack frame animation bands, damage/effects, other gameplay reducers,
approved-account login, live StartGame/map-change acceptance, and physical
device testing remain open. The APK contains no approved Gateway URL, so its
emulator launch is not online-play evidence.
