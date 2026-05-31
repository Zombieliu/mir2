# Gateway Map=Zone Routing — Design

> Owner: architect/review session. Status: **design / not yet implemented.**
> Updated 2026-05-31. The in-process bridge from L2 (parallel multi-zone tick,
> shipped in PR #18) to L4 (cross-process sharding). Companion to
> `SCALABILITY-AND-CAPACITY.md` and `L2-ECS-ZONE-DESIGN.md`.
>
> Goal: make each map (or map group) run as its **own zone**, so the measured
> ~600-players/zone/core capacity multiplies across cores instead of everyone
> sharing one `"primary"` zone on one core. This is the change that makes L2's
> already-shipped parallel `tick_all` actually do something in production.

## Capacity argument

Measured (PR #18 harness): one zone saturates ~1 core at ~600 same-map players;
`ZoneManager::tick_all` ticks independent zones in parallel (1.44–1.88× on 4
cores). Today the gateway puts **all** players in a single `"primary"` zone, so
that parallelism is dormant. Map=zone routing turns "N maps" into "N independently
tickable zones" → throughput scales with cores, and a single overloaded map can
no longer starve the whole world.

## Current architecture (verified, with file:line)

It's a **hybrid**, and that matters for scoping:

- **Routing already exists, just unused.** `MapZoneSessionRouter::route_session`
  (`routing.rs:931`) maps `map_file_name → ZoneId` from a `map_routes` table,
  falling back to `default_zone_id`. Called once at `ZoneRegistry::
  open_session_for` (`routing.rs:5754,5761`). **`map_routes` is effectively
  empty**, so everyone resolves to `"primary"`.
- **Per-zone state containers already exist.** `SharedInProcessZoneRuntimeFactory.
  states: BTreeMap<ZoneId, Arc<Mutex<SharedInProcessZoneState>>>` (`routing.rs:
  2448`), created on demand by `state_for_zone(zone_id)` (`routing.rs:2483`).
  Each `SharedInProcessZoneState` owns a simulation `ZoneManager`.
- **Each session is a hybrid runtime.** A `SharedInProcessZoneSessionRuntime`
  holds **both** a *private* `inner: InProcessWorldRuntime` (`routing.rs:3196`)
  **and** a *shared* `zone_state: Arc<Mutex<SharedInProcessZoneState>>`
  (`routing.rs:3197`). The shared `ZoneManager` carries cross-session entities
  (other players' presence, monsters, ground drops); the private runtime still
  does a lot of per-session work. (This is the "world authority is transitional"
  state from the audit — see the coupling caveat below.)
- **Ticks are per-session.** `web.rs:1686` calls `session.tick()` for each
  session on a `runtime_tick` timer (`web.rs:1453`); that pushes
  `ZoneCommand::Tick` (`routing.rs:3678`) into the session's `zone_state`'s
  `ZoneManager`. So the shared zone is ticked **once per session**, not once per
  zone — redundant today (single zone), and the wrong shape for per-zone cores.
- **Map transfer does NOT change zones.** `apply_zone_current_position_map_transfer`
  (`routing.rs:3297`) runs `WorldCommand::TransferMap` on the **private** runtime
  and sends `MapInformation`; the session stays bound to its original `ZoneId`
  and `zone_state`. Map is a snapshot field, not a zone boundary.
- **ZoneOwner lease is per-zone-ish but the hosted client is 1:1 with a runtime.**
  `leases: BTreeMap<ZoneId, ZoneOwnerLeaseRecord>` (`routing.rs:544`) is keyed by
  zone, but `HostedZoneOwnerCommandClient` binds **one** runtime; in-process
  loopback only.

### What already works vs. the real gaps

| Piece | State |
| --- | --- |
| Route `map → ZoneId` | ✅ exists (`route_session`), unused (`map_routes` empty) |
| Per-zone isolated state | ✅ exists (`state_for_zone`) |
| Multi-zone sim + parallel tick | ✅ exists (`ZoneManager` + `tick_all`, PR #18) |
| **Populate routing / topology** | ❌ no map→zone table is configured |
| **Per-zone tick driver** | ❌ ticks are per-session; need once-per-zone to use cores |
| **Zone handoff on map transfer** | ❌ transfer keeps the old zone |
| **Cross-zone chat (server shout)** | ❌ assumes one zone |
| **Per-zone lease / runtime selection** | ❌ hosted client is 1:1 with a runtime |

## Design

### 1. Zone topology
- **MVP: 1 zone per map** (`ZoneId = "map:<map_file_name>"`). Simple, matches
  `ZoneKey::for_map`, and Crystal already has hundreds of maps (natural sharding).
- **Cold-map grouping (phase 2):** many empty leveling maps can share one zone
  to avoid thousands of idle zones; a config table groups low-traffic maps.
- **Hot-map dedication (phase 3):** a sieged map (Sabuk) gets its own reinforced
  zone/node — this is where L4 cross-process + L5 TiDi plug in.
- Implemented as the `map_routes`/policy behind `route_session`, plus on-demand
  zone creation in `state_for_zone` for maps not in the table.

### 2. Invert the tick loop to per-zone (the key restructure)
Today: `for each session: session.tick() → tick that session's zone`. With many
zones this both under-uses cores (ticks serialized through the per-session loop)
and over-ticks shared zones. Target:

- A **single per-frame zone-tick driver**: once per `runtime_tick`, call
  `tick_all(now_ms)` **once** across all live zones (already parallel), then
  **fan out** each zone's `ZoneOutbound`s to its sessions' send queues.
- Per-session `tick()` stops driving the shared zone; it only drains that
  session's queued outbound + any private-runtime upkeep (until L3 removes the
  private runtime entirely).
- This is what converts L2's parallel `tick_all` into real multi-core use.

### 3. Zone handoff on map transfer (the core new mechanism)
When a player crosses a transfer tile to a map in a **different** zone,
`apply_zone_current_position_map_transfer` must perform an atomic handoff:

1. Resolve `target_zone = route(target_map)`. If `== current_zone`, today's
   in-place behavior is fine (no handoff).
2. **Leave** old zone: `ZoneManager::Leave` for the session → old zone emits
   `ObjectRemove` to remaining observers (presence cleanup — the thing that's
   missing today).
3. **Rebind** the session's `zone_state` to `state_for_zone(target_zone)` and
   re-point its command path at the new zone.
4. **Join** new zone at the destination position → new zone emits the visible
   set (`ObjectPlayer`/monsters/drops) to this session and `ObjectPlayer` to the
   new zone's observers.
5. Send `MapInformation` (as today) and the fresh snapshot.

Handoff must be **atomic w.r.t. in-flight commands**: buffer/reject the session's
commands during the swap so a Walk doesn't land in the old zone after Leave.

### 4. Cross-zone chat relay
- **Local/map chat:** stays zone-local (correct by construction once zones split).
- **Server shout (global):** today every session sees it because there's one
  zone. Add a small **global chat relay** that takes server-shout outbounds and
  rebroadcasts to all zones' sessions. Group/guild/whisper that span zones need
  the same relay (or the existing social service) — out of scope for routing
  itself but must not silently break.

### 5. Per-zone ownership
- The lease map is already per-`ZoneId`; the handoff must acquire the target
  zone's lease and release the source's. For in-process this is bookkeeping;
  for L4 it becomes the real cross-process fencing handshake (already sketched
  in the ZoneOwner RPC seam).

## The hybrid-runtime coupling caveat (important)

Map=zone routing parallelizes the **shared `ZoneManager`** work. But each session
still has a **private `InProcessWorldRuntime`** doing per-session simulation
(`routing.rs:3196`). So:

- The capacity payoff is **bounded by how much simulation actually lives in the
  shared zone** vs. the private runtime. The more that's been promoted into the
  zone (the L3 "world authority" line — combat resolution already moved there in
  PR #11), the bigger the win.
- **Recommendation:** map=zone routing and L3 authority consolidation are
  complementary and should advance together. Routing without L3 still helps
  (shared monster/drop/visibility/AOI work parallelizes per map, and one map can
  no longer starve others), but full multi-core scaling needs the heavy per-tick
  work to be *in the zone*, not the per-session runtime.

## Staged, equivalence-gated plan

Oracle at every step: `shared_zone` 141/141 + the two/multi-client gateway
registry tests (`shared_in_process_registry_*`).

1. **Topology + routing config (no behavior change).** Add the map→zone policy
   and on-demand zone creation; default everything to one zone so output is
   unchanged. Add a registry test that two sessions on **different** maps land in
   **different** `ZoneId`s.
2. **Per-zone tick driver.** Invert the tick loop; prove identical outbounds for
   the single-zone case, then a multi-zone test showing each zone ticks once and
   sessions get their zone's packets. (Uses PR #18 `tick_all`.)
3. **Zone handoff on map transfer.** Implement Leave→rebind→Join with command
   buffering; test a session walking across a zone boundary (old-zone observers
   get `ObjectRemove`, new-zone observers get `ObjectPlayer`, mover gets the new
   visible set). This is the highest-risk slice.
4. **Cross-zone server-shout relay.** Test a shout reaching sessions in other
   zones.
5. **Enable real map=zone in a staging config** and re-run the multi-zone load
   harness end-to-end (per-zone tick through the gateway, not just `ZoneManager`).

## Risks & open questions

- **Handoff atomicity / in-flight commands** — the #1 correctness risk; a
  mis-ordered Walk/Attack during the swap corrupts presence. Needs a clear
  "draining" state per session.
- **Tick-loop inversion touches `web.rs`** (the live network loop) — must keep
  the per-session send path and backpressure intact.
- **Determinism of fan-out** — outbound order per session must stay stable.
- **Cold-zone lifecycle** — when does an empty zone get torn down? (idle GC.)
- **The hybrid runtime** — decide whether to do a thin map=zone (route + tick +
  handoff on the current hybrid) first, or block on more L3 promotion. Thin
  first is recommended (incremental, shippable), with the caveat above.
- **Line numbers** here will drift as `main` moves; re-confirm before editing.

## Effort

- Steps 1–2 (routing config + per-zone tick): **~1–2 weeks**, mostly in
  `routing.rs`/`web.rs`, moderate risk.
- Step 3 (handoff): **~1–2 weeks**, the hard part (atomicity, cross-zone
  presence).
- Steps 4–5 (chat relay + staging): **~1 week**.
- This is L4-adjacent gateway work; it should be serialized through the architect
  with whoever owns `routing.rs`, and interleaved with L3 authority promotion for
  full payoff.

## Integration harness — implementation-ready spec (verified API)

The oracle for Steps 2–3 is a gateway-level multi-session/multi-map test that
drives real `GatewaySession`s through the registry (no sockets). API confirmed
by reading the code (line numbers drift; symbols are stable):

- **Construct routed sessions** — mirror the existing
  `shared_in_process_factory_isolates_state_by_zone_id` test (in `routing.rs`):
  ```rust
  let registry = ZoneRegistry::with_router(
      ZoneId::primary(),
      Arc::new(SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
      Arc::new(PerMapSessionRouter::new()) as SharedSessionRouter,
  );
  let routed = registry.open_session_for(GatewayConfig::default(),
      SessionRouteRequest { account_id: Some(a.into()), character_index: Some(0),
                            map_file_name: Some(map.into()) });
  let mut sess = GatewaySession::with_routed_world_runtime(routed.zone_id, routed.runtime);
  // then start_demo_character / start_new_character(&mut sess, account, name)
  ```
- **Drive actions** — `GatewaySession` exposes (all `-> Vec<ServerPacket>`):
  `transfer_map(key)` (`session.rs:377` → `WorldCommand::TransferMap`), `tick()`
  (`:390`), plus the client-action helpers used by existing tests. `world_snapshot()`
  (`:401`) returns a `WorldSnapshot` whose visible entities are filtered by the
  session's current map — the observation surface for "who sees whom".
- **Two transfer layers to cover:** (1) `GatewaySession::transfer_map` — a direct
  command on the session's *own* runtime; (2) `apply_zone_current_position_map_transfer`
  in `routing.rs` — the *automatic* transfer fired when a player walks onto a
  transfer tile. **Confirmed gap:** neither changes the session's `zone_id` today;
  the session stays bound to its original zone after a map change.

### Tests the harness should hold

Baseline (characterize current behavior — should pass *before* any handoff work):
- `same_map_sessions_see_each_other` — two sessions on map `"0"` appear in each
  other's `world_snapshot`.
- `map_transfer_keeps_session_in_original_zone` — after `transfer_map("1")`, the
  session's `zone_id` is **unchanged** (the gap this design closes). Lock it so a
  regression is visible when Step 3 flips it.

Step 3 target (should fail until handoff lands, then pass):
- `cross_zone_transfer_moves_zone_and_cleans_old_observers` — after a transfer to
  a different-zone map: mover's `zone_id == route(new_map)`; an observer left on
  the old map/zone received an `ObjectRemove` for the mover; an observer already
  on the new map/zone received an `ObjectPlayer` for the mover; mover's snapshot
  shows the new zone's entities, not the old.

Step 2 target:
- a per-zone tick driver test: with ≥2 zones, one tick advances each zone exactly
  once and each session receives only its own zone's outbounds.

## Handoff implementation — verified structure (the hard part)

Reading the code pins down exactly why the handoff is architecturally significant,
not a one-line change:

- `SharedInProcessZoneSessionRuntime` (the per-session runtime) holds **one**
  `zone_state: Arc<Mutex<SharedInProcessZoneState>>`, bound at `create_runtime`
  (`routing.rs:3244` via `state_for_zone(zone_id)`). It has **no reference to the
  factory's `states` map or to the `SessionRouter`**, so it cannot, today, reach
  a *different* zone's state to rebind.
- `GatewaySession` stores `zone_id` + `zone_owner_lease` as **construction-time
  fields**; nothing updates them after open. So even if the runtime rebound its
  `zone_state`, `session.zone_id()` would stay stale → a *partial* handoff (the
  inconsistent half-state the harness's `..._not_zone_today` test guards against).

A correct handoff therefore needs **all** of these, atomically:

1. **Give the session-runtime the means to rebind.** Pass the factory's
   `states` handle + the `SessionRouter` into `create_runtime` so the runtime can
   `route(new_map) → new_zone_id` and `state_for_zone(new_zone_id)`.
2. **Leave → rebind → join.** On a map change whose `route(new_map) != current
   zone`: `old_zone.handle(Leave)` (emits `ObjectRemove` to old observers — the
   cleanup that's missing today), swap `self.zone_state`, `new_zone.handle(Join)`
   at the destination position (emits the new visible set + `ObjectPlayer` to new
   observers), reset the per-map caches (`cached_map_file_name`,
   `last_shared_entity_ids_by_map`, `presence_key`).
3. **Migrate character state.** The character lives in the per-session `inner`
   runtime + account store. The zone change should `save_active_character()` on
   the old binding and reload on the new (the seam already exists for persistence).
4. **Signal the new zone up to `GatewaySession`.** Thread an optional
   `zone_changed: Option<ZoneId>` through `WorldCommandExecution` (simulation
   crate) so `GatewaySession` updates `zone_id` + re-leases `zone_owner_lease`
   (and, for L4, re-acquires the cross-process fencing token).
5. **Atomicity.** Buffer/reject the session's other commands during the swap so a
   Walk/Attack can't land in the old zone after Leave.

Alternative (do it one layer up): give `GatewaySession` a `ZoneRegistry` handle
and re-home the whole session on map change (open on the new zone, migrate the
character, drop the old). Cleaner conceptually, but still touches `GatewaySession`
construction + character migration + old-zone cleanup.

Either way this is a **multi-component, cross-layer change** (~1–2 weeks) on the
most-contended file, with the `cross_zone_transfer_moves_zone_and_cleans_old_observers`
oracle (above) as the acceptance gate. It should be a single focused effort,
serialized through the architect with the `routing.rs` owner — not folded into an
unrelated change.

### KEY FINDING: visibility is already map-filtered (Steps 2+3 must pair)

Reading the sync path settles the sequencing: **within a single zone, visibility
is already filtered by map** (`SharedInProcessZoneSessionRuntime::sync_zone_snapshot`
calls `sync_map_layer(map_file_name, …)` and tracks shared entities/drops
per-map; `world_snapshot` entities are the player's current-map set). Two players
in the same zone on *different* maps already don't see each other; two on the same
map do — **regardless of whether they're in one zone or split into per-map zones.**

Consequences:

- **Map=zone routing changes performance/isolation, NOT visibility.** Its win is
  that each map's zone can tick on its own core (Step 2) and a load spike on one
  map can't starve others — not any change to who-sees-whom.
- **The handoff (Step 3 rebind) alone has no observable benefit and fixes no bug.**
  Moving a transferring player's *presence* between per-zone state objects doesn't
  change visibility (already map-filtered) and doesn't change performance until
  per-zone ticking exists. Implemented alone it is dead groundwork carrying real
  regression risk to the sync/transfer path — a bad trade.
- **Therefore Steps 2 and 3 must land together** as one focused effort: the
  per-zone tick driver (the value) plus the handoff (so a transferred player ends
  up in the zone that's actually ticking their map). The rebind itself is now
  precisely scoped + simplified by reading the code:
  - No character migration needed in-process (the character stays in the
    per-session `inner` runtime; only the shared-zone *presence* moves).
  - The rebind is contained: `remove_presence()` (leaves old zone, notifies old
    observers) → swap `self.zone_state` to `states.entry(new_zone)` (just
    `SharedInProcessZoneState::new()`, no services) → reset the per-map caches →
    the existing `sync_zone_snapshot` re-joins the new zone. Inject it at the top
    of `sync_zone_snapshot` once `map_file_name` is known.
  - It needs the per-session runtime to carry a router + `states` handle +
    `current_zone_id` (additive fields, gated by an `Option` so default
    single-zone behavior is unchanged).
  - Verification needs an internal accessor (e.g. `current_zone_id`) since there's
    no observable visibility change — a gameplay oracle can't distinguish it.

### Status of map=zone work
- **Step 1 (routing primitive): merged** — `PerMapSessionRouter` (auto 1-zone-per-map).
- **Integration harness / oracle: merged** — `map_zone_two_sessions_..._see_each_other`
  (baseline) + `map_zone_transfer_changes_map_but_not_zone_today` (gap locked).
- **Steps 2 + 3 (per-zone tick driver + handoff): designed, scoped, and proven to
  require pairing (above). Not implemented** — this is the focused next effort;
  the live-loop tick inversion (Step 2) is its riskiest part and wants live
  validation, so it should be a deliberate, coordinated change, not a session-tail
  attempt.

> Status note: this spec was written during a window where the build/test loop
> was unavailable; it is grounded in the read-verified API above, not in compiled
> tests. Implement + verify (`cargo test -p mir2-gateway`) before relying on it.

