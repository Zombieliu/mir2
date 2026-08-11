# mir2 game client

The client is being migrated from a browser-coupled Bevy runtime to one shared
Rust/Bevy game client with thin platform hosts.

Current layout:

- `client-core`: platform-neutral snapshot buffering and presentation math;
  no Bevy, browser, windowing, billing, or server-authoritative gameplay.
- `runtime`: the existing Bevy renderer and its WASM/browser bridge. It now
  consumes `client-core` and remains the production Web runtime during the
  migration.

Target layout (created only as each vertical slice becomes executable):

- `client-bevy`: shared map, entity, animation, effects, camera and in-game UI.
- `platform-web`: WASM/PWA adapter.
- `platform-windows`: native window/input/lifecycle adapter.
- `platform-android`: Android Activity, touch and lifecycle adapter.
- `platform-ios`: Swift/Xcode, touch and lifecycle adapter.
- `launcher-tauri`: Windows login, update, announcement and native-game launch.
- `platform-xbox`: deferred until GDKX partner access and Windows Native are
  stable.

The server remains authoritative. Client code can emit intents, keep a local
replica, interpolate and reconcile presentation, but cannot award XP, decide
damage, mutate inventory, settle trades or determine guild/Sabuk results.

Architecture and execution contracts:

- [`ADR-0001`](../../docs/architecture/ADR-0001-cross-platform-bevy-client.md):
  dependency rules, authority ownership and migration gates.
- [`M1 client model contract`](../../docs/architecture/M1-CLIENT-MODEL-CONTRACT.md):
  frozen deterministic primitives and the M1-A edit boundary.
- [`M1 Flash handoff`](./M1-FLASH-HANDOFF.md): focused prompt and acceptance
  commands for the next implementation slice.

## Verification

```bash
cargo test --manifest-path apps/game-client/client-core/Cargo.toml
cargo test --manifest-path apps/game-client/runtime/Cargo.toml
cd apps/web && npm run runtime:build:dev
```
