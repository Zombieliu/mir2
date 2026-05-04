# Architecture Implementation Status

Last updated: 2026-05-05

Purpose: track production-architecture completion separately from Crystal
observable parity. Crystal packet/UI/gameplay parity remains a compatibility
baseline; it is not the same as production architecture completion.

## Current Score

Production architecture completion: **52%**.

Previous planning baseline was **42%**. The first implemented slices add an
explicit in-process World/Zone runtime boundary plus a gateway zone registry
and shared zone player/map snapshot layers while preserving existing Crystal
compatibility behavior.

| Area | Weight | Current | Gap | Notes |
| --- | ---: | ---: | ---: | --- |
| Game authority runtime | 20% | 80% | 20% | `WorldRuntime` / `WorldCommand` now separates gateway from concrete `SimulationSession`; shared zone player presence plus shared NPC/monster/drop snapshot layers exist, and removed drops plus non-player entities are tombstoned across sessions. Combat, AI, inventory gain on remote pickup, NPC services, and ticks are not yet a true shared zone authority. |
| Gateway/session/routing | 15% | 70% | 30% | Gateway now constructs sessions through `ZoneRegistry`; `SessionRouter` provides the policy hook for map/character routing, and the default in-process zone shares player presence and map snapshot layers, but has no distributed registry/RPC handoff. |
| Persistence | 15% | 35% | 65% | JSON remains the default local store; Postgres source mode exists but gameplay repositories are not yet normalized. |
| Redis/cache/online state | 8% | 35% | 65% | Session cache exists; broader routing, reconnect, and invalidation semantics remain pending. |
| Service boundary/messaging | 10% | 25% | 75% | Admin outbox exists; gameplay command/event envelopes are not yet broad runtime infrastructure. |
| Admin/control plane | 10% | 55% | 45% | Real command/audit paths exist, but production RBAC/read models/approvals are incomplete. |
| Client architecture | 10% | 45% | 55% | Web/Bevy direction exists; runtime/UI responsibilities still need cleanup. |
| Content/data pipeline | 7% | 40% | 60% | Crystal import is strong; production content versioning/publish/rollback is incomplete. |
| Observability/load readiness | 5% | 15% | 85% | Load, trace, metrics, and alerting are not yet architecture gates. |

## Implemented Boundary Slice

- `mir2-simulation` now exposes `WorldRuntime`, `WorldCommand`, and
  `InProcessWorldRuntime`.
- `mir2-gateway` constructs `InProcessWorldRuntime` by default but stores it
  behind `ZoneRuntimeHandle`.
- Gateway session methods now execute typed world commands instead of calling
  `SimulationSession` directly.
- `mir2-gateway` now has `ZoneRegistry`, `ZoneId`, and `ZoneRuntimeFactory`;
  TCP and Web gateways open sessions through the registry.
- `SessionRouter` / `SessionRouteRequest` now define an explicit route-policy
  hook for future map, character, load, or reconnect-based zone placement. The
  default router still selects the primary zone.
- The default in-process zone factory now shares online player presence plus a
  per-map NPC/monster/drop snapshot layer across sessions. Players in the same
  zone appear as remote `Player` entities in each other's `WorldSnapshot`, and
  non-player map objects/dropped items can surface through the shared zone
  snapshot layer instead of only through per-session local state.
- Shared ground drops now have zone-level removal tombstones. If a session
  removes a shared drop, other sessions stop seeing it, and stale private
  `SimulationSession` snapshots cannot reinsert the same drop object id into
  the shared layer. This works for both Web high-level `pick_up(objectId)` and
  protocol-level `ClientPacket::PickUp` on the current cell.
- Shared non-player map entities also have zone-level removal tombstones. If a
  monster or NPC object id is removed from the shared map layer, a stale private
  session snapshot cannot reinsert that same object id into the zone layer.
- Session-cache records now include `zoneId` so online presence can become a
  routing index instead of only a presence list.
- The public `SimulationSession` API remains available for compatibility tests
  and direct simulation use.

## Next Architecture Slices

1. Promote the shared NPC/monster/drop snapshot layer into true shared zone
   authority: combat mutation, AI ticks, remote drop pickup inventory gain, NPC
   services, and AOI deltas must stop depending on each connection's private
   `SimulationSession`.
2. Back `SessionRouter` with concrete map/character routing policy and future
   cross-zone handoff state instead of the current single-zone default.
3. Introduce gameplay repository traits and move JSON/Postgres account-store
   access behind those traits.
4. Expand Redis online/routing cache semantics for reconnect, kick, and zone
   handoff.
5. Add gameplay event envelopes for authoritative actions, with Redpanda and
   ClickHouse kept non-authoritative.
6. Add architecture CI gates for runtime contract, routing, repository adapter,
   Redis miss/hit, and event schema compatibility.
