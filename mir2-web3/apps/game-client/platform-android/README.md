# Android native client

## Shared Crystal UI (2026-09-08)

Android now installs the same `Mir2NativeShellUiPlugin` / `NativeShellModel`
used by Windows, now enabled through the additive `native-player-ui` feature.
The HUD, overlays, minimap, chat, notices and quest/NPC plugins are also mounted.
Windows `native-ui` still enables this shell plus its existing gameplay/audio
features. No fork of the login/select layout was made. Java's visible login
form and the Bevy debug-text screen were removed. The only Java editor is a
transparent 1-pixel OS IME input connection; visible fields and actions belong
to the shared Crystal shell. The 1024x768 stage fits uniformly with letterboxing;
while typing it pans upward to keep the login panel readable above the IME.

Package with `MIR2_ANDROID_UI_ASSET_ROOT` pointing to a local approved asset
root containing `original-ui/{ChrSel,Prguse,Prguse2,Title,Items,Help,MMap,StateItem}/*.png`. Gradle stages
only those images in generated build output. The files are not committed.
Optionally set `MIR2_ANDROID_WORLD_ASSET_ROOT` to an approved generated asset
root containing `generated/map-atlas/manifest.json`, its PNG pages, and the
bounded raw map `generated/crystal-map-pack/0.map`, plus
`generated/native-map-keyed/manifest.json` and its content-addressed pages.
Gradle also stages the tracked shared `bevy-entity-atlases` manifest/pages. On
each accepted Bichon world snapshot, the Android host validates schema, counts,
geometry, paths and memory limits; decodes the packaged map atlas and the
entity/keyed pages used by that view off the render thread; parses the Crystal
type-100 map; and derives the matching
shared `MapRenderState` and `EntityRenderState`. It publishes state before
bounded raw-RGBA batches through the native runtime queue. Disconnect, map
change or a newer world generation cancels stale background work. A missing or
rejected pack remains a visible loading error; there is no synthetic fallback.
Set `MIR2_GATEWAY_WS_URL` explicitly at build time for approved online tests.
Neither an exported Activity intent nor old endpoint preferences override it.
With no endpoint the actual shared login screen shows a configuration notice;
it does not fabricate a connection or a selectable test character.

## Login transport

For explicitly offline UI specimens, package with
`MIR2_ANDROID_VARIANT=uiPreview`; this produces the separate
`com.mir2.web3.uipreview` APK. Its network entrypoints are disabled and its
fixtures are labelled. Use `capture-ui-preview.sh OUTPUT_DIRECTORY` to collect
the specimen routes. The `world-render` specimen is an offline rendering
fixture, not a login result. See `docs/ANDROID-UI-COVERAGE.md` for implemented surfaces
and remaining interaction/live-device gates; registration is not acceptance.

`android/app/src/main/java/com/mir2/web3/GatewaySession.java` supplies a bounded
login/character-selection WSS host using OkHttp 4.12.0 and Android's normal TLS
trust and hostname verification. Configure an approved `wss://.../ws` endpoint
at build time; there is no default production endpoint. Cleartext, embedded
credentials, query credentials and redirects are rejected.

The shared shell's intents send the existing BrowserCommand `clientVersion`,
`login {accountId,password}`, `startGame {characterIndex}` and `keepAlive`
shapes (see Windows `native_protocol.rs` and Android `gateway_bridge.rs`).
An account ID is only a username paired with a password, never an authenticated
identity assertion. It does not send PasskeyLogin or invent authentication.
LoginSuccess supplies the shared selectable roster including class/gender/level.
JNI delivers host events to the shared model and shared intents back to the
host. An accepted complete world snapshot is projected into the shared world,
HUD, map and entity read models. Android then parses the packaged raw Crystal
map, resolves ordinary atlas draws plus keyed/additive standalone objects, and
resolves every visible server entity into the shared entity-atlas contract.
Map/entity state is published before its bounded image batches so queue pressure
cannot evict the state that owns those images. Only the same exact request
remaining complete across two consecutive rendered frames, with a non-empty
map, a visible self actor and zero unresolved visible map or entity entries,
releases the StartGame transition. New snapshots received while
an asset load is active replace one deferred frame; stale receipts cannot
release the latest transition or scene update.

The same barrier now covers authoritative map changes. When the transport
leaves `IN_GAME` for `STARTING`, or a complete snapshot changes the map name,
the host returns the shared shell to `StartingGame` before clearing native
scene presentation. It preserves authenticated character/personal models but
drops unsent gameplay effects while the shell is non-interactive. Only the
matching two-frame `NativeRenderReady` receipt restores `InGame`; a partial or
stale map/entity load cannot expose the HUD over an incomplete scene.

The checked local Bichon `(302,634)` viewport renders 607 atlas draws and 242
keyed/additive standalone draws (849 total) with zero unresolved visible draws,
plus the fixture self player and monster. The available Mac source export is
not a complete whole-map asset pack: 2,969 of 7,672 Bichon standalone references
are absent outside that viewport. Missing assets remain explicit and block a
strict render receipt when encountered. They are not replaced with synthetic
terrain or silently called complete.

Passwords, account names, session tokens and character state are not persisted.
Editor state saving and autofill are disabled. Passwords are cleared after
submission/backgrounding. Logs omit credentials and raw messages. Empty login
screens can be captured; entered credentials and the SafeKey/password-change
surfaces enable FLAG_SECURE. No genuine credentials were entered during UI QA.

On background, disconnect, timeout or transport failure, the socket and old
roster/position are discarded. Reconnect requires an explicit button and fresh
login. No command is replayed and nativeResumeV1 is not advertised. Automatic
credential-based resume, process-death session persistence, character creation,
walk/run and generated per-library action animation, and complete gameplay
remain subsequent work. The
existing reducer command queue is connected to the authenticated in-game
socket through the bounded JNI lease mailbox described below. Authoritative
transaction receipts and the entity packet families described below return to
shared read models, but the remaining gameplay packet/reducer surface is still
incomplete, so this is not an online player-loop acceptance. MockWebServer
verifies the TLS/protocol state machine but is not real account acceptance; an
approved WSS endpoint and test account are required for that gate.

Host TLS/protocol tests (test certificates are generated only in JVM memory):

```bash
cd android
./gradlew testDebugUnitTest
```

These MockWebServer tests do not establish real Gateway authentication. See
`docs/generated/player-qa/native-android-shared-ui-20260908/README.md` in the main
project for current UI evidence; the earlier `native-android-login-20260908`
pack describes the superseded debug form, not the current player interface.

This crate is the native Android shell for the shared Bevy client. UI actions
are routed through `mir2-ui-core`; the Android layer only owns lifecycle,
safe-area, keyboard, back-button, and joystick translation.

## Reducer-to-gateway bridge

`src/gateway_bridge.rs` consumes `UiEffect::GatewayCommand`, converts the
supported commands to the same camelCase JSON shapes accepted by the Web and
Windows BrowserCommand path, and retains them in a bounded FIFO
`AndroidGatewayOutboundQueue`. The Bevy update drains only while Android is in
the foreground, the network is available, and an authenticated in-game socket
generation is active. `MainActivity` polls a bounded JNI envelope, writes the
unchanged Rust-produced BrowserCommand through `GatewaySession`, and reports
the exact lease sequence once. Background/socket loss closes the generation;
sent-but-unacknowledged transaction mutations become unknown and are never
replayed. Queue overflow rejects the new command and exposes a counter/status
instead of silently dropping it. Session-bootstrap/auth commands remain on
the dedicated login state machine; the gameplay writer rejects them,
including `passkeyLogin`.

Inbound `gameShopReceipt`, Storage V2 and password-result text now enters a
separate bounded `AndroidGatewayInboundQueue` from the live
`GatewaySession`. Java forwards only those recognized authoritative result
shapes, with a 16 KiB UTF-8 limit; the Bevy update thread classifies them
again before using the existing exact-request adapters. Unrelated gameplay
packets do not enter this transaction queue. `AndroidShellPlugin` registers
that resource and drains it on the real Bevy `Update` chain, applying an exact receipt to both
`UiState` and outbound correlation state atomically. The public enqueue API is
only a transport/JNI host handoff; it does not own the WebSocket. The three
public raw inbound transaction entrypoints always pass through the fixed
default queue limits and frozen qualification step. Raw message construction,
custom queue limits, raw enqueue, drain, pending binding, the eligibility bit,
and queued-message consumption are private to the `gateway_bridge` module—not
merely `pub(crate)`. The Bevy system can only invoke an owner-level
crate-private drain function with the bounded queue, `UiState`, and outbound
model; it cannot see or construct messages or eligibility. The public receipt
parsers are pure validators and cannot mutate or release a transaction.
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

The same live host now forwards a bounded allowlist of authoritative entity
packets after `IN_GAME`. `src/live_entity.rs` validates and folds spawn/refresh,
movement/turn/action positions and removals over the last complete world model.
Existing same-request Crystal sprite layers move/remove immediately and a later
render product aligns to packet-fresh positions. Packet-only new actors enter
the neutral object layer immediately. Render-ready actors retain a bounded
Android-private set of all available standing facings; movement/turn packets
select the authoritative direction and update Crystal tile depth without
decoding atlas pages again. The runtime payload contains only the active pose.
When a packet-only actor has the exact same canonical kind/class/dead/sprite
contract as an already decoded render-ready actor, the host now clones that
bounded prototype, rewrites every layer key to the server object id, places it
inside the current viewport, and selects its authoritative standing direction
without another atlas decode. Unmatched actors still wait for a complete render
snapshot.

For generic unmounted actors whose default Crystal catalog exactly resolves
inside the immutable packaged atlas, the render producer now also retains an
Android-private bounded action-frame sidecar. `ObjectHarvest`, `ObjectAttack`,
`ObjectRangeAttack`, `ObjectStruck`, and `ObjectDashAttack` select the matching
harvest/melee/range/struck/dash frame sequence at the packet's authoritative
position and facing. If the packet omits direction, the last authoritative
object facing is used. A monotonic presentation clock advances the exact
pre-resolved frames and restores the standing facing after completion without
waiting for another snapshot; movement, lifecycle replacement, removal and a
new world request cancel stale action state. The sidecar is validated and
removed from runtime JSON, which still carries only the active layers.

This action leaf deliberately does not claim the generated class/library
catalogs needed by Archer, Assassin, mounted and other alternate actors.
Walk/run, `ObjectHarvested` skeleton motion, die/revive sequences, spell
effects, health feedback and ground drops also remain separate work. No atlas
name from a packet is resolved or decoded on the live packet path.

The post-`IN_GAME` allowlist also carries health/death/revive and
hide/show/teleport lifecycle packets. The private cache applies death position,
life state and dead/live opacity immediately. Hide and teleport-out remove an
actor from both visible models while retaining its exact bounded record;
show/teleport-in restores only that record. Remove creates a bounded tombstone
that later periodic snapshots cannot resurrect, while a new authoritative
spawn clears it. These lifecycle rules do not synthesize health-bar effects or
missing actors; death/revive still use the lifecycle opacity path rather than
the new action sequence clock.

This closes the reducer-to-live-socket outbound adaptation, existing
transaction-result return adapters, and the bounded entity object-list/
transform ingestion leaf only. Other ordinary server gameplay packets are not
yet fed into every shared reducer, and no approved live account has exercised
the path. This crate must not yet be described as a complete online-playable
Android client.

## Native M0 host

`android/` is a Gradle GameActivity host for the Rust `cdylib`. The build path
is deliberately explicit:

1. `cargo-ndk` cross-compiles `libmir2_platform_android.so` for
   `arm64-v8a`.
2. The library is copied into Gradle's generated `jniLibs` tree.
3. `MainActivity` loads that library and GameActivity invokes Bevy's
   `android_main`, which is emitted by `#[bevy_main]`.
4. The shared Bevy runtime renders the Crystal native shell. Both the earlier
   M0 teal/gold marker and the later login debug text have been removed.

The historical M0 marker proved only that the native Activity, Bevy/Winit renderer, and
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
MIR2_ANDROID_MODE=package \
MIR2_ANDROID_UI_ASSET_ROOT=/approved/ui-root \
MIR2_ANDROID_WORLD_ASSET_ROOT=/approved/generated-pack-root \
./build-android.sh
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
capture the rendered frame, the `Mir2NativeSession` phase log entries, and a
background/resume check. An `adb` listing alone is not evidence that the game
launched or that a physical device was used; emulator evidence must not be
reported as physical-device acceptance.
