# mir2 game client

The client is being migrated from a browser-coupled Bevy runtime to one shared
Rust/Bevy game client with thin platform hosts.

Current layout:

- `client-core`: platform-neutral snapshot buffering and presentation math;
  no Bevy, browser, windowing, billing, or server-authoritative gameplay.
- `runtime`: the existing Bevy renderer and its WASM/browser bridge. It now
  consumes `client-core` and remains the production Web runtime during the
  migration.
- `client-bevy`: shared renderer-neutral read models plus Bevy map/entity
  fallbacks and HUD, inventory, chat and character UI.
- `platform-windows`: native desktop window/input/Gateway adapter with packaged
  Crystal atlases and maps.
- `platform-android`: Android aarch64 compile-gate host; Activity/surface/touch
  lifecycle wiring remains a later device slice.

Remaining target layout (created only as each vertical slice becomes executable):

- `platform-web`: WASM/PWA adapter.
- `platform-ios`: Swift/Xcode, touch and lifecycle adapter.
- `platform-xbox`: deferred until GDKX partner access and Windows Native are
  stable.

The thin WebView shells live beside this tree in `apps/mir2-launcher-tauri`
(Windows/macOS/Linux) and `apps/mir2-mobile` (Android/iOS).

The server remains authoritative. Client code can emit intents, keep a local
replica, interpolate and reconcile presentation, but cannot award XP, decide
damage, mutate inventory, settle trades or determine guild/Sabuk results.

Architecture and execution contracts:

- [`ADR-0001`](../../docs/architecture/ADR-0001-cross-platform-bevy-client.md):
  dependency rules, authority ownership and migration gates.
- [`M1 client model contract`](../../docs/architecture/M1-CLIENT-MODEL-CONTRACT.md):
  frozen deterministic primitives and the M1-A edit boundary.
- [`M1 Flash handoff`](./M1-FLASH-HANDOFF.md): archived implementation prompt
  retained as M1-A history; the slice is complete.

## Verification

```bash
cargo +1.95.0 test --locked --manifest-path apps/game-client/client-core/Cargo.toml
cargo +1.95.0 test --locked --manifest-path apps/game-client/client-bevy/Cargo.toml
cargo +1.95.0 test --locked --manifest-path apps/game-client/runtime/Cargo.toml
cd apps/web && npm run runtime:build:dev
```
