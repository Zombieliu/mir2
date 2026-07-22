# Gate 5.5 — Atomic Cross-Zone Handoff and Global Messages

Gate 5.5 makes the live session follow the topology introduced in Gate 5.4. `StartGame`, explicit
map transfer, movement-triggered transfer, revive, and other map-changing commands now reconcile
the committed map with the owning Zone before the next player command.

## Handoff transaction

The Gateway uses a prepare/commit protocol:

1. execute the map-changing command on the source Zone and capture its authoritative player state;
2. sync the shared-Zone transform and persist the source character;
3. open the topology-selected target Zone, authenticate it through the trusted passkey path, and
   start the same character;
4. apply the source map position/direction through the trusted `ApplyHandoffTransform` command;
5. compare normalized player-state commitments (identity, map, position, vitals, economy,
   inventory/equipment, quests, skills, buffs and Stage 5 systems);
6. close the source under its current owner fence, then atomically swap the Gateway binding and
   event publisher to the target lease.

The old runtime stays live until target preparation and commitment verification succeed. If target
prepare or source close fails, the target is closed and the source is transferred back to its
pre-command map/position. The player connection receives an error instead of continuing with a map
owned by the wrong Zone. `GatewaySession::handoff_generation()` exposes successful commits.

Route-aware WebSocket sessions deliberately use the serialized session command path for movement.
This prevents the earlier movement fast path from completing a gate transfer without giving the
Gateway a chance to rebind. A route-aware owner-side fast path can be restored after Gate 6
placement metadata is available.

## Remote lifecycle and checkpoint replay

Zone RPC protocol version 3 adds fenced `CloseSession`. A close is journaled as a tombstone, so a
replicated checkpoint replays the source commands and the later removal in the same global order.
This prevents remote handoffs and ordinary disconnects from leaving ghost players or consuming
Zone Host session capacity.

`PasskeyLogin` now clears the runtime's fixture-selected character before `StartGame`; otherwise a
fresh target could persist demo state into the authenticated account before loading its requested
slot.

## Global messages

Each `ZoneRegistry` owns a bounded cross-Zone message bus shared by all sessions opened from it.
Valid server shout (`Shout`, `Shout2`, `Shout3`) and GM announcement packets are copied only to
active sessions in other Zones; same-Zone delivery remains authoritative in the Zone runtime.

- An active TCP/WebSocket registration receives the packet immediately with the same generation id
  used by the Zone live stream.
- A disconnected or backpressured registration keeps a bounded 256-packet backlog and drains it on
  the next session command/reconnect.
- Registration activation and replacement remain fenced, so a stale socket cannot receive packets
  belonging to the current connection.

## Acceptance

```bash
cargo +1.89.0 test -p mir2-gateway \
  routing::tests::map_zone_transfer_atomically_rebinds_to_the_destination_zone \
  -- --test-threads=1
cargo +1.89.0 test -p mir2-gateway \
  routing::tests::failed_target_prepare_rolls_the_source_back_without_rebinding \
  -- --test-threads=1
cargo +1.89.0 test -p mir2-gateway \
  routing::tests::server_shout_crosses_zone_boundaries_through_the_global_bus \
  -- --test-threads=1
cargo +1.89.0 test -p mir2-gateway --test zone_rpc -- --test-threads=1
cargo +1.89.0 check -p mir2-gateway --all-targets
```

The RPC suite includes an actual remote `primary -> map:0 -> map:1` Gateway handoff, asserts that
the host always contains one live session, closes it on disconnect, and verifies close tombstones
survive checkpoint replay.
