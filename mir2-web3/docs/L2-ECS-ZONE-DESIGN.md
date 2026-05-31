# L2 Design: ECS-native Zone + lock-free parallel ticks

> Owner: architect/review session. Status: **design / not yet implemented.**
> Updated 2026-05-31. Companion to `SCALABILITY-AND-CAPACITY.md` (the L1–L5
> roadmap) and `SYSTEM-OWNERSHIP-AND-INTERFACES.md` (hot-file rules).
>
> This is the reviewed plan to be agreed **before** anyone edits the 8k-line
> hot file `apps/simulation/src/runtime/zone/runtime.rs`. L2 = "make the zone
> ECS-native and let zones tick in parallel without a global lock." It does
> **not** change observable gameplay or packet output.

## Why L2 (grounded in measured facts)

The L1 load harness (`examples/zone_load.rs`, release, single core) measured:

| players in one zone | mean ms/tick | ms/player |
| ---: | ---: | ---: |
| 100 | 4.0 | 0.040 |
| 400 | 55.7 | 0.139 |
| **600 (knee @100ms)** | 108.9 | 0.182 |
| 1000 | 262.6 | 0.263 |

Two conclusions drive L2:

1. **Single-zone work is super-linear** (ms/player 0.04 → 0.26). L1 gridded the
   visibility path, so the residual growth is now per-player combat/AI
   authority work (PR #11) plus remaining full-collection scans — addressable
   by ECS archetype iteration + finishing the spatial indexing.
2. **One core is the ceiling.** `apps/gateway/src/routing.rs:336`
   (`runtime: Mutex<Option<ZoneRuntimeHandle>>`) serializes *all* access to a
   zone, and there is **no `Schedule`/`par_iter` anywhere** — every zone runs on
   one thread behind one lock. A 4-zone server uses 1 core, not 4.

L2 raises both ceilings: ECS archetype storage cuts the per-player constant and
enables intra-tick parallelism; removing the global lock lets independent zones
(and eventually independent systems) run on separate cores.

## Current shape (what we're changing)

`ZoneRuntime` (`runtime.rs:68-95`) is a hand-rolled struct of `BTreeMap`s:

```text
players:         BTreeMap<SessionId, ZonePlayer>
objects:         BTreeMap<u32, ZoneObject>
native_monsters: BTreeMap<u32, ZoneNativeMonster>
ground_drops:    BTreeMap<u32, ZoneGroundDrop>
player_grid:     AoiGrid<SessionId>     // L1
object_grid:     AoiGrid<u32>           // L1
... (~20 fields)
```

`tick()` (`runtime.rs:621`) is a fixed sequence of `outbounds.extend(self.tick_*())`
calls. The crate already depends on `bevy_ecs 0.17` / `bevy_app 0.17`, but the
shared zone uses **none** of it (it's only a KV store in the per-session path).
The gateway talks to the zone through one seam: `ZoneRuntimeHandle` exposing
`handle(ZoneCommand) -> Vec<ZoneOutbound>`, `tick(now_ms) -> Vec<ZoneOutbound>`,
`world_snapshot()`, `save_active_character()`.

**Invariant L2 must preserve:** `handle`/`tick` produce byte-identical
`ZoneOutbound` packet streams. The `shared_zone` suite (141 tests) + the new
AOI regression tests are the equivalence oracle.

## Target shape

Each zone owns a Bevy `World` + a fixed `Schedule`:

```text
ZoneRuntime {
    world: bevy_ecs::world::World,   // entities = players, monsters, objects, drops
    schedule: Schedule,              // the tick pipeline as ECS systems
    key: ZoneKey,
    // command intake + outbound drain stay as explicit buffers (Resources)
}
```

- **Entities/Components**: `ZonePlayer`/`ZoneObject`/`ZoneNativeMonster`/
  `ZoneGroundDrop` become component bundles on entities; the `BTreeMap` lookups
  become `Entity` handles + indexes (`Resource`-held `BTreeMap<id, Entity>` for
  the by-id access the code relies on). Positions live in a `Position`
  component; the AOI grids become a `Resource` updated by a `sync_grid` system.
- **Systems**: the `tick_*` methods become systems registered in the
  `Schedule`, in the same order. Outbound packets accumulate in an
  `OutboundQueue` resource drained at end-of-tick (preserving order).
- **Commands**: `handle(ZoneCommand)` pushes into a `CommandQueue` resource; a
  `drain_commands` system applies them at the head of the tick (or `handle`
  applies immediately for the request/response calls that need a synchronous
  return — see Risks).
- **Parallelism, staged**:
  - *Stage A (cheap, safe):* zones run on a thread pool instead of behind one
    `Mutex` — N independent zones use N cores. Requires only that each
    `ZoneRuntimeHandle` own its `World` and the registry hand out per-zone
    handles without a shared lock.
  - *Stage B (later):* within a zone, Bevy's scheduler auto-parallelizes
    systems whose component accesses don't conflict (e.g. monster-AI read vs.
    drop-expiry). Opt-in per system once Stage A is proven.

## Migration strategy — incremental, equivalence-gated

Big-bang rewriting 8k lines is the wrong move (high regression risk on the
hottest, most-contended file). Instead, **strangler-fig** in reviewed slices,
each its own PR, each keeping `shared_zone` 141/141 green:

1. **Introduce the `World` alongside the maps (no behavior change).** Add
   `world: World` to `ZoneRuntime`; mirror player entities into it on
   join/leave/move. Maps remain source of truth. Proves the entity/index
   bookkeeping under the existing tests. *(Net-zero functionally; pure
   scaffolding.)*
2. **Move one read-heavy system to ECS iteration.** Port
   `diff_zone_object_visibility_for`-style scans to `Query` iteration over the
   mirrored entities; delete the corresponding map scan. Keep output identical.
3. **Flip source of truth, component by component** (positions, then vitals,
   then combat stats), deleting each `BTreeMap` only when its readers/writers
   are all on ECS. The by-id index resource stays for `object_id` lookups.
4. **Wrap the tick in a `Schedule`.** Register the `tick_*` systems in current
   order; `tick()` becomes `schedule.run(&mut world)` + drain `OutboundQueue`.
   Still single-threaded, still identical output.
5. **Stage A parallelism:** remove `Mutex<Option<ZoneRuntimeHandle>>` in
   `routing.rs`; give the registry per-zone ownership and run `tick_all` across
   a thread pool. Add a multi-zone load harness mode to prove N-core scaling.
6. **Stage B parallelism (opt-in):** mark non-conflicting systems for parallel
   scheduling; measure with the harness.

Each step is independently revertable and shippable; we stop and reassess if
any step can't hold the equivalence oracle.

## Risks & honest unknowns

- **Synchronous command returns.** Some gateway calls (`world_snapshot`,
  `save_active_character`, trade/rental commits) expect an immediate result,
  not a queued effect. The `World` must support synchronous `handle` for those;
  only the per-tick simulation systems go through the `Schedule`. This seam
  needs care — getting it wrong reorders effects.
- **Determinism / ordering.** `BTreeMap` iteration is ordered; ECS archetype
  iteration order is **not** guaranteed stable. Anywhere packet order depends on
  id order (it does in places), we must sort explicitly or keep an ordered
  index. This is the most likely source of subtle test breakage.
- **Bevy 0.17 ECS API churn.** We use `bevy_ecs` standalone (no `App`/plugins).
  Need to confirm `Schedule`/`World` standalone ergonomics and whether
  `par_iter` needs a task pool we must own.
- **Borrow conflicts vs. today's `&mut self`.** Many `tick_*` methods mutate
  several maps at once; as systems they'll need disjoint `Query`/`ResMut`
  params or explicit `Commands`-deferred mutation. Some methods will need
  restructuring, not just relocating.
- **Cross-session/world-service commits** (Account/Inventory idempotency) must
  keep their receipt semantics across the refactor.

## Effort & sequencing

- Steps 1–4 (ECS-native, still single-thread, identical output): **~2–3 weeks**,
  the bulk of the risk, on the hot file → must be serialized through the
  architect with the 多人 session (it owns `runtime.rs`).
- Step 5 (Stage A, drop the global lock): **~3–5 days** once the `World` owns
  zone state; high payoff (N-zone → N-core).
- Step 6 (Stage B intra-zone parallel): **~1 week**, opt-in, measured.

L2 does **not** by itself fix Sabuk-scale single-map (that's L3 authority
consolidation + L5 siege specials). L2's win is: more players per zone (lower
constant) **and** many zones per machine (parallelism) — the foundation L4
cross-process sharding builds on.

## Acceptance gates for every L2 PR

1. `shared_zone` suite stays **141/141** (equivalence oracle).
2. `cargo +1.89.0 fmt --check` + `check` clean (the arch-gate's first steps).
3. `examples/zone_load` shows **no regression** at 100/400 players, and
   (from step 5) demonstrates multi-zone parallel speedup.
4. No silent behavior/packet change vs. Crystal; any intentional change called
   out in the PR per the parity-bar rule.

---

## Implementation status & evidence-based re-prioritization (2026-05-31)

Built and verified (all on `main` via the architect branch; `shared_zone`
141/141 held at every step):

- **Step 1 — ECS World mirror: DONE.** `zone/ecs.rs` stands up a `bevy_ecs::World`
  in `ZoneRuntime` and mirrors player entities at the join/leave/move sites.
  `players` is still source of truth; an invariant test asserts the mirror
  matches it across join/walk/leave. Dropped the vestigial `derive(Clone)` from
  `ZoneRuntime`/`ZoneManager` (a `World` isn't `Clone`; nothing cloned them).
- **Stage A — parallel multi-zone tick: DONE.** `ZoneManager::tick_all` ticks
  independent zones in parallel on a persistent `ComputeTaskPool` above a
  measured break-even (4 zones); deterministic (parallel == sequential, proven).
  Measured 4-core speedup: 1.44×@4z, 1.57×@8z, 1.88×@16z; neutral below
  threshold. `examples/zone_load.rs` has a multi-zone mode that reproduces this.

**What the load harness (its whole purpose) revealed, and how it changes the plan:**

1. **The ECS *storage* migration (steps 2–4) is now low-value.** It was meant to
   kill the per-tick O(N²) full-collection scans — but L1's spatial grids
   already did that. The harness shows single-zone cost is now *super-linear in
   the per-player combat-authority work* (PR #11's zone-side damage), not in
   `BTreeMap` iteration. Converting the maps to ECS components would be weeks of
   high-risk edits to the 8k-line hot file for a modest constant-factor gain.
   **Deprioritized** unless a profiler shows storage iteration is a real cost.
2. **The real single-zone lever is the combat-authority code**, which is the
   combat/多人 session's domain — an algorithmic optimization, not a storage one.
3. **The real production multi-core win is gateway map=zone routing**, not
   `tick_all`. Today the gateway runs a single `"primary"` zone behind the
   ZoneOwner lock and never calls `tick_all`. Stage A is ready and proven, but
   it is *dormant groundwork* until the gateway routes sessions to per-map zones
   (an L4-adjacent change entangled with the ZoneOwner lease/RPC machinery —
   large, and best done deliberately, not bundled into L2).

**Recommendation:** treat L2 as **done for its high-value parts** (ECS World
foundation + proven parallel multi-zone tick). Do **not** grind the ECS storage
rewrite (steps 2–4) — the evidence says it isn't worth the risk post-L1. The
next real capacity step is **gateway map=zone routing** (so Stage A's
parallelism actually runs in production), which deserves its own design pass.
