# Android native client scaffold

This crate is the native Android shell for the shared Bevy client. UI actions
are routed through `mir2-ui-core`; the Android layer only owns lifecycle,
safe-area, keyboard, back-button, and joystick translation.

## Reducer-to-gateway bridge

`src/gateway_bridge.rs` consumes `UiEffect::GatewayCommand`, converts the
supported commands to the same camelCase JSON shapes accepted by the Web and
Windows BrowserCommand path, and retains them in a bounded FIFO
`AndroidGatewayOutboundQueue`. The Activity/WebSocket host must call
`drain_ready(&AndroidShellState, max_entries)` only while the app is in the
foreground and the network is available. Background and unavailable-network
states retain entries. Queue overflow rejects the new command and exposes a
counter/status instead of silently dropping it.

Inbound `gameShopReceipt` text now has a separate bounded
`AndroidGatewayInboundQueue`. `AndroidShellPlugin` registers that resource and
drains it on the real Bevy `Update` chain, applying an exact receipt to both
`UiState` and outbound correlation state atomically. The public enqueue API is
only a transport/JNI host handoff; it is not a WebSocket implementation.
`enqueue_native_game_shop_receipt` is the sole public raw inbound mutation
entrypoint and always passes through the fixed default queue limits and frozen
qualification step. Raw message construction, custom queue limits, raw
enqueue, drain, pending binding, the eligibility bit, and queued-message
consumption are private to the `gateway_bridge` module—not merely
`pub(crate)`. The Bevy system can only invoke an owner-level crate-private drain
function with the bounded queue, `UiState`, and outbound model; it cannot see or
construct messages or eligibility. The public receipt parser is a pure
validator and cannot mutate or release a transaction.
Each inbound JSON message is limited to 16 KiB and the queue has a 128 KiB
total byte budget in addition to its 32-entry limit. Every inbound variant is
charged by its UTF-8 byte length; oversize messages and byte/count overflow are
rejected without evicting existing FIFO entries. Drain and clear release the
tracked byte budget. Malformed or unmatched receipts never release pending
state. Reserve protection is granted only to the first valid receipt that
exactly matches the currently bound `requestId`, `gIndex`, `quantity`, and
`priceType`. Wrong, duplicate, semantically invalid, and no-pending receipts
remain quarantined and cannot suppress the real pending transaction's unknown
path. Once an exact receipt is retained it is protected from later malformed or
overflow flood; overflow without that exact reserve marks an in-flight purchase
unknown and removes any replayable buy.

`SetChatChannel` is retained as a visible `LocalOnly` entry because the Web
gateway has no `BrowserCommand::SetChatChannel`; `RetryConnection` is also
`LocalOnly` because the transport host must reopen the socket rather than
fabricating a `clientVersion` packet. Neither may be sent as a websocket
command. Non-gateway effects (`ApplyAudioSettings`,
window, persistence, notices, and exit effects) remain in `AndroidUiEffects`
for platform-side handling.

The current shared `mir2_ui_core::GatewayCommand` enum has no Mail, Storage,
or Shop mutation variants; opening those panels is therefore not evidence of a
corresponding Android wire action. This bridge does not invent those commands.

This closes the reducer-to-wire-command and receipt-to-reducer adaptation
only. The repository still does not provide an Android WebSocket transport, so
this crate must not be described as an online-playable Android client.

## Native M0 host

`android/` is a Gradle GameActivity host for the Rust `cdylib`. The build path
is deliberately explicit:

1. `cargo-ndk` cross-compiles `libmir2_platform_android.so` for
   `arm64-v8a`.
2. The library is copied into Gradle's generated `jniLibs` tree.
3. `MainActivity` loads that library and GameActivity invokes Bevy's
   `android_main`, which is emitted by `#[bevy_main]`.
4. The shared Bevy runtime renders an asset-free M0 marker and logs
   `MIR2_ANDROID_M0_FRAME_READY`.

The marker proves only that the native Activity, Bevy/Winit renderer, and
shared runtime reached a rendered frame. It is not evidence of login,
WebSocket transport, authoritative world state, or a complete native client.

## Local prerequisites

The Rust build gate is intentionally offline and never installs tools. Gradle
may populate its normal dependency cache on the first package build. Provide
all of the following locally before a target build:

- Rust toolchain `1.95.0` and target `aarch64-linux-android`
- Android SDK and NDK
- Java 17 or newer for Gradle/AGP
- `cargo-ndk` 4.1.2
- `adb` for emulator or device checks

Set `ANDROID_SDK_ROOT` and `ANDROID_NDK_HOME` (or `ANDROID_NDK_ROOT`) when the
SDK/NDK are not in the standard user location. The GameActivity flavor uses
Android API 31 by default and rejects lower values. Supporting API 26 would
require a separately tested NativeActivity flavor; lowering this package's
minimum SDK does not make GameActivity compatible.

## Host checks

From this directory:

```powershell
cargo test --locked --offline
```

The local `rust-toolchain.toml` pins Rust 1.95.0 so the repository-wide Rust
1.89.0 selection cannot make Bevy 0.19 host checks fail.

## Android check and package

On Windows:

```powershell
.\build-android.ps1
$env:MIR2_ANDROID_MODE = 'package'
.\build-android.ps1
```

On a Unix-like shell:

```bash
./build-android.sh
MIR2_ANDROID_MODE=package ./build-android.sh
```

The first command runs an offline `cargo-ndk` target check. Package mode builds
an optimized Rust library inside a debug-signed APK by default and writes it
to:

```text
android/app/build/outputs/apk/debug/app-debug.apk
```

Set `MIR2_ANDROID_VARIANT=release` for an unsigned release package. Set
`MIR2_ANDROID_RUST_PROFILE=debug` only when native debug symbols are actually
needed; a Bevy debug library makes the APK impractically large. The packaging
script stages all Cargo outputs under `target/` and copies only
`libmir2_platform_android.so` into the APK. Generated native libraries, APKs,
Gradle caches, passwords, and signing keys are ignored and must not be
committed. Missing local tools or targets fail before the build is attempted.

## Emulator/device handoff

Only after an APK exists, verify the endpoint explicitly with `adb devices`.
Install with `adb install -r <apk>` and launch package `com.mir2.web3`. For M0,
capture the rendered frame, the `MIR2_ANDROID_M0_FRAME_READY` log entry, and a
background/resume check. An `adb` listing alone is not evidence that the game
launched or that a physical device was used; emulator evidence must not be
reported as physical-device acceptance.
