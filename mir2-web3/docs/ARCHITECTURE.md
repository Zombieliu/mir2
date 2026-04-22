# Architecture

## Core Shape

- `web`
  - user-facing web portal
  - wallet connect
  - marketplace and account pages
- `game-client`
  - Bevy WASM runtime
  - rendering, input, local UI, client prediction
- `gateway`
  - session management
  - websocket endpoints
  - auth and account binding
  - Sui-facing application service
- `simulation`
  - authoritative ECS simulation
  - map state
  - movement, combat, AI, drops

## Design Rules

- Keep chain logic out of the hot gameplay loop.
- Keep Sui access behind gateway/service boundaries.
- Treat Crystal as a migration reference, not a runtime dependency.
- Move protocol and data definitions into shared packages as early as possible.

## Phase Order

1. Protocol definition
2. Login/session flow
3. Character list and map join
4. Movement replication
5. Chat
6. Data import pipeline
