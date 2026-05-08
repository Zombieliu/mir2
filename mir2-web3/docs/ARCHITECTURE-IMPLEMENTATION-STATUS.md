# Architecture Implementation Status

Last updated: 2026-05-06

Purpose: track production-architecture completion separately from Crystal
observable parity. Crystal packet/UI/gameplay parity remains a compatibility
baseline; it is not the same as production architecture completion.

## Current Score

Production architecture completion: **93%**.

Previous planning baseline was **42%**. The first implemented slices add an
explicit in-process World/Zone runtime boundary plus a gateway zone registry
and shared zone player/map snapshot layers while preserving existing Crystal
compatibility behavior. The follow-up slices add concrete map-based routing,
per-zone shared-state isolation, Redis-backed online route lookup, typed
command outcomes, a stable gameplay event envelope, a Redpanda producer path,
a ClickHouse gameplay event projection, Redis route freshness/stale-cleanup
helpers, route leases, gameplay-event read APIs plus summary/lag readiness and
threshold alerts, an Admin Web gameplay-event readiness panel, account-store
repository adapters, an architecture gate script that exercises the key
runtime/routing/event/schema contracts, and `/health` visibility for
session-cache and gameplay-event boundaries. The
score is rounded from the weighted table below; it is still an architecture
score, not a Crystal parity score or a production launch sign-off.

| Area | Weight | Current | Gap | Notes |
| --- | ---: | ---: | ---: | --- |
| Game authority runtime | 20% | 92% | 8% | `WorldRuntime` / `WorldCommand` separates gateway from concrete `SimulationSession`; `WorldCommandOutcome` / `WorldCommandExecution` report typed command results; shared zone player presence plus shared NPC/monster/drop snapshot layers exist; removed drops plus non-player entities are tombstoned across sessions; and command outcomes now feed event/read-model boundaries. Full combat/AI/NPC mutation is still not a single authoritative zone process. |
| Gateway/session/routing | 15% | 99% | 1% | Gateway constructs sessions through `ZoneRegistry`; `SessionRouter` has `MapZoneSessionRouter`; the shared in-process factory isolates state by `ZoneId`; default Web/TCP sessions can publish command outcomes through an env-configured event sink; Web online routes acquire per-character route leases; late stale disconnects cannot erase a newer route; `/health` reports routing-cache plus gameplay-event boundary status; and the architecture gate now repeats the shared registry and route-lease regressions. Distributed RPC handoff remains future work. |
| Persistence | 15% | 88% | 12% | JSON remains supported, but account storage now has an `AccountStoreRepository` trait plus file and Postgres adapters. `SimulationConfig` load/save paths go through those adapters, Postgres source mode keeps stale-writer checks, repository statuses are inspectable, and the architecture gate covers the repository contract. Further normalization of inventory/mail/economy tables remains open. |
| Redis/cache/online state | 8% | 96% | 4% | Session cache includes `zoneId`, `updatedAtMs`, character-name route index, `route_character`, `fresh_route_request_for_character`, stale-route cleanup, Redis TTL coverage, route leases, owned route removal, and health status. The architecture gate now covers in-memory hit/miss/freshness/cleanup plus Redis adapter and lease regressions when Redis is available. Cross-zone lease transfer protocol remains pending. |
| Service boundary/messaging | 10% | 98% | 2% | Admin outbox exists; gameplay commands produce typed outcomes and `GatewayGameplayEvent` envelopes; Gateway can publish those events to Redpanda/Pandaproxy through `MIR2_GAMEPLAY_EVENT_REDPANDA_URL`; ClickHouse has a `gameplay_events` projection; Admin API exposes `/admin/gameplay-events` plus `/admin/gameplay-events/summary` for command-volume, lag, and threshold-alert readiness; and the architecture gate locks the Gateway JSON envelope to the ClickHouse Kafka/materialized-view columns. Admin Web now consumes that summary on the dashboard. Gameplay event delivery remains non-authoritative and outside transaction commit semantics. |
| Admin/control plane | 10% | 96% | 4% | Real command/audit paths, approvals, zone heartbeat, staging env, event-stream runbook coverage, admin event reads, timeline, gameplay-event reads, gameplay-event summary readiness alerts, a dashboard readiness panel, and repeatable Admin/API/Web gate coverage exist. Production IdP, policy thresholds beyond local event readiness, and incident automation remain incomplete. |
| Client architecture | 10% | 84% | 16% | Web/Bevy direction exists, the player client remains playable through Gateway, and Admin Web now surfaces control-plane/event readiness from real Rust APIs. The remaining work is mostly runtime/UI responsibility cleanup, resource streaming packaging, and frontend deployment hardening rather than core architecture discovery. |
| Content/data pipeline | 7% | 82% | 18% | Crystal import and generated manifests are strong; staging docs now call out full resource concerns and the event/persistence boundaries are ready for content publish/read models. Production content versioning, publish/rollback, CDN packaging, and resource delta strategy remain incomplete. |
| Observability/load readiness | 5% | 99% | 1% | Command outcomes, gameplay events, Redpanda/ClickHouse projection, Admin gameplay-event query and summary APIs with readiness alerts, Admin Web readiness panel, session-cache health, route-lease health, event-sink health, repository statuses, local Docker/WS load evidence, `infra/check-architecture-gates.sh`, `infra/check-candidate-gate.sh`, and the GitHub Actions local Candidate workflow create repeatable readiness gates. External alert delivery and reconnect soak remain pending. |

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
  default router still selects the primary zone, and `MapZoneSessionRouter`
  provides the first concrete exact-map route policy.
- The shared in-process zone runtime factory now keeps separate shared state per
  `ZoneId`, so sessions routed to different zones no longer share remote
  player/map snapshot state.
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
- Gateway session cache now exposes `route_character`, and
  `route_request_for_character` converts a cached online character into a
  `SessionRouteRequest` with account, character index, and map route context.
- Session-cache records also include `updatedAtMs`; routing helpers can reject
  stale presence with `fresh_route_request_for_character`, and
  `remove_stale_session_routes` can clean expired online routes outside Redis
  TTL behavior.
- Session-cache records can carry route-lease metadata. Web Gateway refreshes
  online routes through `refresh_session_cache_with_route_lease`, Redis stores
  a lease key per account/character, and disconnect cleanup uses
  `remove_owned_session_cache` so a stale socket cannot delete a newer route.
- `WorldCommandOutcome` and `WorldCommandExecution` expose command kind, packet
  count, snapshot tick, and active identity after execution.
- `GatewayGameplayEvent` is a serializable command-event envelope with schema
  version, zone id, command kind, identity, packet count, and snapshot tick.
  `GatewaySession` can publish these events through an optional
  `GameplayEventSink`; the default constructor remains unchanged and does not
  require an event bus.
- Web and TCP gateway startup can create the gameplay event sink from
  `MIR2_GAMEPLAY_EVENT_REDPANDA_URL` / `MIR2_GAMEPLAY_EVENT_TOPIC`; local
  stderr-only logging is available through `MIR2_GAMEPLAY_EVENT_LOG=true`.
- `/health` now reports session-cache backend/TTL/record/stale counts plus
  gameplay event sink accepted/published/failed/dropped counts.
- ClickHouse initialization now includes `mir2_events.gameplay_events`, a Kafka
  engine table for `gameplay.command.executed`, and a materialized view for the
  non-authoritative gameplay event read side.
- Admin API now exposes `/admin/gameplay-events` with ClickHouse filters for
  `zoneId`, `commandKind`, `accountId`, and `characterName`, plus
  `/admin/gameplay-events/summary` with `windowSeconds`, `limit`, `zoneId`,
  `commandKind`, `maxLagSeconds`, and `minEvents` filters. The summary reports
  total command volume, per-command counts, last event time, max snapshot tick,
  event lag, `ready`, and structured readiness alerts.
- Admin Web dashboard now reads `/admin/gameplay-events/summary` and exposes
  command-stream readiness, five-minute command volume, lag, latest event time,
  readiness alert messages, and top command kinds. The panel renders
  degraded/offline responses without blocking the rest of the dashboard.
- `infra/check-architecture-gates.sh` now runs the local architecture gate:
  gateway/admin/simulation `fmt` and `check`, shared zone registry regressions,
  session-cache hit/miss/freshness/Redis/lease regressions, gameplay event
  publishing/schema compatibility, Admin API ClickHouse gameplay read/readiness
  regressions, account-store repository regressions, Admin Web typecheck, Docker
  Compose config, and `git diff --check`.
- `infra/check-candidate-gate.sh` now wraps the architecture gate with
  local/full/live Candidate scopes for game-data, packet trace, Player Web,
  Admin Web, static smoke, and running Gateway/Web evidence refreshes.
- `.github/workflows/mir2-candidate-gate.yml` now runs the local Candidate gate
  on pull requests and pushes to `main`.
- `mir2-simulation` now defines `AccountStoreRepository` plus file and Postgres
  implementations. `SimulationConfig` uses those adapters for account-store
  load/save and reports configured repository status, while preserving existing
  JSON, mirror, and Postgres source-of-truth modes.
- The public `SimulationSession` API remains available for compatibility tests
  and direct simulation use.

## Next Architecture Slices

1. Promote the shared NPC/monster/drop snapshot layer into true shared zone
   authority: combat mutation, AI ticks, remote drop pickup inventory gain, NPC
   services, and AOI deltas must stop depending on each connection's private
   `SimulationSession`.
2. Promote route leases into a distributed route-transfer protocol for
   cross-zone handoff and reconnect conflict resolution.
3. Normalize gameplay persistence behind repository adapters for inventory,
   mail, economy, auction, and NPC script state.
4. Promote gameplay event readiness alerts into external notification/incident
   delivery and deeper operator-facing drilldowns.
5. Add reconnect/soak/load gates that exercise Postgres source saves, Gateway
   routing, and Redpanda/ClickHouse degradation.
6. Expand the CI-gated Candidate flow with reconnect soak and full/live
   scheduled evidence refresh coverage.
