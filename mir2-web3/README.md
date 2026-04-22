# mir2-web3

New cross-platform Mir2/Web3 project scaffold.

## Layout

- `apps/web`
  - Next.js app for wallet login, account portal, marketplace, admin tools.
- `apps/game-client`
  - Bevy WASM game client.
- `apps/gateway`
  - Rust gateway for auth, sessions, websocket traffic, Sui integration.
- `apps/simulation`
  - Bevy headless or `bevy_ecs` authority simulation.
- `packages/protocol`
  - Shared protocol definitions and generated artifacts.
- `packages/game-data`
  - Exported data converted from Crystal resources/configs.
- `packages/tooling`
  - Importers, converters, generators, and migration scripts.
- `docs`
  - Architecture notes, migration plan, and milestones.

## Source Of Truth

The existing Crystal project remains the reference implementation for:

- gameplay rules
- packet flow
- map and asset formats
- server-side data behavior

The new project should not modify Crystal directly. Use Crystal as a reference and migration source.

## MVP Goal

Phase 1 should only target:

1. wallet/account binding
2. character selection
3. map entry
4. movement
5. chat
6. basic entity visibility

## Next Steps

Current implemented checkpoint:

1. `packages/protocol` now has typed packet support for login/select/start-game, `MapInformation`, `UserInformation`, movement, chat, `ObjectPlayer`, `NewMonsterInfo`, and `NewNpcInfo`.
2. `apps/simulation` emits a deterministic bootstrap scene with the player, one remote player, one monster, and one NPC for local testing.
3. `apps/gateway` exposes TCP, HTTP health, WebSocket bridge, browser manual smoke UI, and a TCP smoke binary.

Immediate next steps:

1. Add normalized map/bootstrap data import so the starter scene is driven by converted Crystal data instead of hardcoded values.
2. Introduce a first visible Bevy client crate that consumes the structured gateway events.
3. Start extracting reusable game-data and tooling packages for assets, maps, and object definitions.
