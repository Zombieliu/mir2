# mir2-web3 Agent Instructions

Primary project: `mir2-web3`, resolved relative to this repository root on
Windows, macOS, and Linux. Never assume a drive letter, home directory, or
absolute checkout path.

Use `docs/AGENT-ORCHESTRATION.md` as the source of truth for multi-agent coordination.

## Default Goal

Drive the project toward **100% Candidate** Crystal / Mir2 1:1 parity before requesting final human frontend acceptance.

Do not ask for routine confirmation. Proceed autonomously through the current task queue unless a stop condition in `docs/AGENT-ORCHESTRATION.md` applies.

## Required Reading

Before planning substantial work, read:

- `mir2-web3/docs/AGENT-ORCHESTRATION.md`
- `mir2-web3/docs/AGENT-TASK-QUEUE.md`
- `mir2-web3/docs/CRYSTAL-1TO1-ROADMAP.md`
- `mir2-web3/docs/BACKEND-1TO1-PROGRESS.md`
- `mir2-web3/docs/CRYSTAL-SERVER-PARITY.md`

## Coordination

- Only one code worker may edit a high-conflict file such as `apps/simulation/src/runtime.rs` per round.
- Explorers should be read-only unless explicitly reassigned.
- Backend parity changes must update roadmap/progress/parity docs after tests pass.
- Frontend parity changes must update the player QA or frontend gaps docs after screenshots/tests pass.
- Never revert unrelated user or agent work.

## Model Policy

Use `gpt-5.3-codex-spark` for bounded implementation, tests, documentation,
and mechanical repository maintenance. Cross-branch integration, auth/security
changes, schema migrations, production rollout, and destructive cleanup require
a frontier reasoning model to lead and verify the result.

- `xhigh`: bounded high-risk implementation.
- `high`: normal implementation.
- `medium`: exploration, QA planning, docs.

Avoid unsupported account models unless availability is confirmed.

## Shared Zone MVP Rules

The current Rust simulation is mostly single-session:

- `InProcessWorldRuntime`
- `SimulationSession`
- one local `HeadlessRuntime`
- one local ECS `World`

Do not pretend this is multiplayer by spawning `RemotePlayer` inside each
personal session. A real multiplayer online server must use a shared Zone
runtime.

### Core Architecture Rule

Keep these responsibilities separate:

1. Personal `SimulationSession`
   - login
   - character list
   - StartGame bootstrap
   - inventory/equipment/personal state
   - saving/loading character state

2. Shared `ZoneRuntime` / `ZoneManager`
   - online players in same map/channel/instance
   - authoritative position/direction
   - movement validation
   - occupancy collision
   - AOI visibility
   - ObjectPlayer/ObjectWalk/ObjectRun/ObjectTurn/ObjectRemove/ObjectChat broadcasts
   - JoinZone / LeaveZone lifecycle

Session is not world. Zone is world.

### Production Safety Rules

Do not expose these to normal clients:

- `WorldCommand::MoveTo`
- `WorldCommand::Stage5Command`
- debug teleport keys like `crystal:<map>:<x>:<y>`
- raw `PasskeyLogin { account_id }` from clients
- QA/admin commands such as `qa.giveItem`, `event.spawn`, `qa.openStorage`

Production paths must not fallback to `"demo"` account.
Unauthenticated StartGame/NewCharacter/DeleteCharacter must be rejected.

### Files To Avoid Touching In The First Zone MVP

Do not rewrite or deeply modify these files unless absolutely necessary for
compilation:

- `apps/simulation/src/runtime/combat.rs`
- `apps/simulation/src/runtime/monster_ai.rs`
- `apps/simulation/src/runtime/skills.rs`
- `apps/simulation/src/runtime/stage5.rs`
- `apps/simulation/src/runtime/social_economy.rs`
- `apps/simulation/src/runtime/drops.rs`
- `apps/simulation/src/runtime/inventory.rs`
- `apps/simulation/src/runtime/items.rs`
- `apps/simulation/src/runtime/equipment.rs`

The first Zone MVP is not supposed to implement full combat, auction, trade,
rental, monster AI, or economy.

### Desired Files For Zone MVP

Prefer adding:

- `apps/simulation/src/runtime/zone/mod.rs`
- `apps/simulation/src/runtime/zone/types.rs`
- `apps/simulation/src/runtime/zone/runtime.rs`
- `apps/simulation/src/runtime/zone/manager.rs`
- `apps/simulation/src/runtime/zone/movement.rs`
- `apps/simulation/src/runtime/zone/aoi.rs`
- `apps/simulation/src/runtime/zone/collision.rs`
- `apps/simulation/src/runtime/zone/packets.rs`

Modify only as needed:

- `apps/simulation/src/runtime/mod.rs`
- `apps/simulation/src/lib.rs`
- `apps/simulation/src/world_runtime.rs`
- `apps/simulation/src/runtime/session.rs`
- `apps/simulation/src/runtime/save.rs`
- `apps/simulation/src/runtime/packets.rs`
- `apps/simulation/src/runtime/map.rs`

Add tests:

- `apps/simulation/tests/shared_zone.rs`
- `apps/simulation/tests/security_lifecycle.rs`

Do not keep stuffing tests into `apps/simulation/src/runtime/tests.rs`.

### Zone Runtime Design

Do not add async/tokio inside the simulation crate unless it already exists.
Implement Zone runtime as a synchronous state machine:

```rust
zone.handle(command) -> Vec<ZoneOutbound>
zone.tick(now_ms) -> Vec<ZoneOutbound>
```

The external gateway/WebSocket layer is responsible for sending packets to
clients.

Zone state must be single-writer. Avoid designs where many threads mutate
shared world state through `Arc<Mutex<ZoneState>>`.

Use command/outbound structs:

- `ZoneCommand`
- `ZoneOutbound`
- `ZoneJoin`
- `ZoneKey`
- `ZoneRuntime`
- `ZoneManager`
- `MoveIntent`

### Movement Rules For First MVP

Client movement packets are intents, not direct state changes.

Walk/Run:

- update latest desired movement intent
- do not queue stale pending movement commands
- on zone tick, if movement is ready, consume latest intent once
- validate static collision
- validate occupancy
- run checks intermediate tile and destination tile
- run from standstill should degrade to walk for first movement instead of causing rollback
- failed movement should send correction to owner, not broadcast movement
- successful movement sends UserLocation to owner
- successful movement sends ObjectWalk or ObjectRun to AOI observers

Turn:

- update direction
- owner receives UserLocation
- AOI observers receive ObjectTurn

Chat:

- broadcast ObjectChat to AOI or same zone, depending on available packet semantics

AOI:

- use simple rectangular visibility for MVP, for example `dx <= 18 && dy <= 14`
- maintain per-player visible object set
- when object appears, send ObjectPlayer
- when object disappears, send ObjectRemove

LeaveZone:

- remove from players
- remove from occupancy
- broadcast ObjectRemove
- emit outbound event to save authoritative transform

### Session Integration Rules

Add a method to `SimulationSession` or wrapper:

```rust
active_zone_join_snapshot(session_id) -> Option<ZoneJoin>
```

It should provide:

- session_id
- account_id
- character_index
- object_id
- name
- class
- gender
- level
- map_file_name
- position
- direction

Add a method:

```rust
force_authoritative_player_transform(position, direction)
```

It must update both:

- ECS `SelfPlayer` Position / Facing
- `PlayerRuntimeResource.player_position` / `player_direction`

This is required so disconnect/save does not save stale local-session position.

### Gateway Routing Expectation

The gateway should route:

StartGame:

- run existing personal `SimulationSession` StartGame
- then call `active_zone_join_snapshot`
- then `ZoneManager.join`

Walk/Run/Turn/Chat:

- route to Zone, not to personal `SimulationSession` packet handler

LogOut/Disconnect:

- LeaveZone
- apply final authoritative transform
- save active character
