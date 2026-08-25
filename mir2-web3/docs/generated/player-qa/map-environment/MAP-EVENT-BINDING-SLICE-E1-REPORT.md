# Map Event Binding Slice E1

Status: bounded six-binding closure; general map-event parity remains open.

## Crystal baseline

- `Crystal/Server/MirObjects/PlayerObject.cs::CheckMovement` queues the
  `_MAPCOORD(map,x,y)` default-NPC action before scanning movements. A matching
  `NeedMove` stores `NPCMoveMap` and `NPCMoveCoord` instead of transferring
  immediately.
- `Crystal/Server/MirObjects/PlayerObject.cs::CallDefaultNPC` schedules the
  map-coordinate script through the delayed NPC action list.
- `Crystal/Server/MirObjects/NPC/NPCSegment.cs` parses `ENTERMAP`; execution
  consumes the stored map/coordinate, teleports without forcing a new facing,
  and removes both temporary values.
- `Crystal/Server/MirDatabase/MovementInfo.cs` is the authoritative binary
  `NeedMove`, destination, and conquest-index layout.

## Generated evidence

- Six and only six current `_MAPCOORD` source bindings are typed.
- Every binding resolves to exactly one current `Server.MirDB` `NeedMove` row.
- Conditions are typed as `LEVEL > 49` or `CHECKPKPOINT > 199`; pass action is
  `ENTERMAP`; failure action is the exact Crystal `LocalMessage ... Hint`.
- Binding, condition, pass-action, and failure-action source file/line
  provenance is retained in the generated manifest.
- Duplicate coordinates, missing or ambiguous `NeedMove`, unsupported
  commands, unsafe paths, include cycles, and invalid phases fail generation.
- The 18 imported general event files remain explicitly
  `generalEventScripts.status = "open"` and are not executable.

## Runtime closure

- Personal-session movement admits only generated typed bindings and preserves
  the player's current direction for the `ENTERMAP` transfer.
- Shared Zone movement evaluates the same typed threshold and exact Hint
  failure for walk, run, and turn. An allowed transfer records the generated
  destination and current facing; the Gateway then commits that authoritative
  map/position/direction snapshot instead of leaving the transfer pending.
- Duplicate runtime bindings and invalid generated actions fail closed.

## Verification

- Map-event generator self-test: 7/7 passed; regenerated 6 bindings.
- `crystal_map_events` plus `map_event_binding_e1`: 3/3 passed.
- Existing `map_coordinate_events` personal/shared integration: 3/3 passed.
- Gateway allowed-turn transfer regression: 1/1 passed (`3` at `861,686`,
  PK 200, turn left -> `D1801` at `128,171`, preserving left facing).
- Exact changed-file Rustfmt, Node syntax, and `git diff --check`: passed.

## Explicitly open

- Execution semantics for the 18 general `Events/*.txt` files.
- Ordinary door open/close timing and packet order.
- Castle gate, wall, and blocking-object actions.
- Exact delayed-action ordering, AOI removal/seed order, the complete six-gate
  Gateway matrix, and live packet trace comparison against Crystal.
- Hazard RNG trace equivalence and persistent map-event state.

This report must not be used to claim full map, backend, Candidate, or Accepted
parity.
