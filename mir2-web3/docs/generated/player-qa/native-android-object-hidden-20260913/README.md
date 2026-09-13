# Native Android authoritative ObjectHidden presentation — 2026-09-13

## Scope

This bounded Android slice carries the typed Crystal `ObjectHidden` packet
through the authenticated post-`IN_GAME` host allowlist and the existing
authoritative object cache. A strict boolean update changes only the exact
existing actor. Unknown, removed and ground-drop identities are ignored, while
malformed values are rejected. A temporarily lifecycle-hidden actor retains
the status and restores with the same value on `ObjectShow`; the packet never
creates or resurrects an actor.

Crystal keeps a Hidden actor in the object layer and draws ordinary actor
layers at opacity `0.5`. Android now applies that value to current, directional
and active-action sprite layers. Clearing Hidden restores normal opacity, or
the existing explicit `0.45` Android death fallback when an exact corpse pose
is unavailable. Hidden overrides that fallback rather than multiplying with
it. This is distinct from `ObjectHide` / `ObjectShow`, which control lifecycle
presence.

The offline `world-render` route injects one labelled `ObjectHidden` packet
after the existing metadata and poison packets. This fixture exercises the
same reducer and retained Bevy layers, but it is explicitly labelled offline
and does not imply a network session.

## Deterministic verification

- Android Rust default suite: **182 passed, 0 failed**.
- Android Rust `ui-preview` suite: **190 passed, 0 failed**.
- The focused reducer test covers visible state, action-layer opacity, death
  composition, duplicate suppression, lifecycle-hidden retention and restore,
  initial spawn state, unknown/removed/drop isolation and malformed payloads.
- The entity-render fixture covers initial and directional layer opacity.
- Gradle Debug and UI Preview unit suites: **25 passed, 0 failed** in each
  variant. `GatewaySessionTest` covers post-world packet forwarding.
- `aarch64-linux-android` API 31 target check passed with Rust 1.95.0 and NDK
  26.1.10909125.
- Release Rust plus `uiPreview` packaging and streamed emulator install passed
  with the existing local UI/world/entity proof packs.
- `git diff --check` and Rust formatting passed.

## Emulator-visible evidence

The package was cold-launched on `Mir2_API_31_ARM64` using Emulator 37.1.11,
API 31, `arm64-v8a` and `sdk_gphone64_arm64`. The physical display is
1080x2340; the landscape capture is 2340x1080 and the app is the focused
activity across the complete display.

`world-render-logcat.txt` records:

- `ANDROID_OBJECT_HIDDEN_PRESENTATION_APPLIED`
- `ANDROID_WORLD_RENDER_READY` with 849 map draws, seven entities and twelve
  entity layers
- the existing remote backstep active and settled markers
- no matched fatal exception, panic, `DeviceLost`, surface error, OOM, signal
  or ANR line

`world-render-object-hidden.png` is a 2340x1080 RGBA capture, 1,419,446 bytes,
SHA-256
`610b6fafb87222687357fc27329b92ca5a7d2f5fe19fa41e0ade8bf2abb0f58b`.
It visibly retains the packet-updated remote actor in the full-screen Bichon
scene. Exact `0.5` opacity is asserted by the deterministic render tests rather
than inferred from pixels.

## APK

- Path:
  `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
- Bytes: `444440200`
- SHA-256:
  `2c97bbdd21ec3d75a9af965ca7f8f99344b4bf3146d392db47132f47e406c99e`
- Entity override pack ID:
  `android-archer-mount-bow-proof-20260912`

The APK and source asset packs remain outside Git.

## Acceptance boundary

No approved WSS endpoint, test account or physical Android device was
available. This slice does not claim real login, online StartGame, a live
render-ready map transition, public Web/entity-release alignment, physical
device behavior or final human acceptance. `ObjectSneaking` and missing
resource-backed level-effect visuals remain separate object-presentation work.
