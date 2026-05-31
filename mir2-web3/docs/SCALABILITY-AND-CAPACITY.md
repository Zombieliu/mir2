# Scalability & Capacity (Architect)

> Owner: architect/review session. Updated 2026-05-31.
> Companion to `PRODUCTION-GAP-ASSESSMENT.md` (parity) and
> `SYSTEM-OWNERSHIP-AND-INTERFACES.md` (ownership). This doc is about **how many
> players a zone/server can carry** and the staged path to global-server +
> Sabuk-siege scale.

## The one idea that reframes everything

**"1000-player same-screen" is mostly an illusion, and that's by design.** No
client ever receives the full state of 1000 players — that is the O(N²)
disaster nobody survives. "Thousand-player siege" = **thousands on the same
map, each client receiving only the few dozen entities inside its
area-of-interest (AOI)**. Capacity is therefore governed by *local density*
(how many share one screen) and *total concurrency per zone*, not by a single
"max players" number.

How shipped MMOs actually do it:

- **Sharding/【分区】**: split total population across many independent servers.
  Legend of Mir / 传奇 ran hundreds of 区; a Sabuk siege is a *single-区* event.
- **AOI / interest management (九宫格)**: broadcast only to nearby cells, so
  cost scales with on-screen density, not server population.
- **On-screen caps + culling**: above a threshold, distant players/effects are
  dropped or simplified. (This is *why* siege screens look chaotic — it's a
  feature, not a bug.)
- **Time Dilation (TiDi)**: when a node is overloaded (siege), it slows the
  tick instead of crashing — EVE's signature trick. Players tolerate slow-mo
  far better than a crash.
- **Map = node**: one map/region per process/machine; crossing maps = handoff
  between nodes (EVE's per-system node model).

Honest caveat: **true thousand-on-one-screen at 60fps is something essentially
no commercial MMO achieves** — everyone trades it away via the techniques
above. The original 传奇 siege was itself laggy. The goal is "playable under
graceful degradation," not "lossless."

## Current architecture: verified facts

Measured from the code (`apps/simulation/src/runtime/zone/`, `apps/gateway/src/`):

| Property | State | Evidence |
| --- | --- | --- |
| Zone concurrency | **Single global lock, serial** | `routing.rs:335` `runtime: Mutex<Option<ZoneRuntimeHandle>>` — all commands for a zone run one-at-a-time; multi-core unused. |
| Visibility (players) | **was O(N²), now grid O(local)** ✅ | `diff_visibility_for` previously scanned all players each move/tick; now scans an AOI-grid neighborhood (this branch). |
| Visibility (objects/monsters) | **still O(N) full scan** | `diff_zone_object_visibility_for` iterates `self.objects.values()` — next target for the same grid. |
| Combat / AI / skills authority | **per-session, broadcast** | Not a single authoritative tick; two players each compute their own copy of a monster. |
| Multi-zone | **isolated zones exist** ✅ | `zone/manager.rs` `zones: BTreeMap<ZoneKey, ZoneRuntime>` + `tick_all` — the seam for map=node. |
| Cross-process | **none (in-process loopback)** | `ZoneOwner` is loopback; no real network/process handoff. |
| ECS (Bevy) | **present but not driving** | `bevy_ecs 0.17` used as a KV store in the per-session path; the shared `ZoneRuntime` is plain `BTreeMap`s with hand-written ticks. No `Schedule`/`par_iter` anywhere. |
| Load evidence | **64 WS connections ready only** | `load-gateway-ws.mjs` default 64 — connection readiness, **not** 64 in one screen fighting. No CPU/tick capacity curve measured. |

### Honest capacity estimate today

- Dozens of players spread across maps: **fine**.
- ~30 in one map fighting: **the stress edge** (global lock + per-session combat).
- Single zone same-screen: **~50–100 before it strains**, **unverified**.
- **Sabuk siege (hundreds–thousands same map): will not hold today.** Missing
  every required technique (grid object AOI, lock-free parallel tick, single
  authority, TiDi, on-screen culling).

## L1–L5 roadmap (difficulty, effort, capacity target)

Effort = continuous wall-clock with Opus driving + Sonnet workers; the real
risk is integration/debugging, not code generation.

| Lvl | Work | Difficulty | Effort | Unlocks |
| --- | --- | --- | --- | --- |
| **L1** | Grid AOI for players ✅ + objects (next) | 🟢 easy, local | ~3–7 days | same-screen tens → ~1–2 hundred |
| **L2** | Move `ZoneRuntime` onto a real Bevy ECS `World` + `Schedule`; drop the global `Mutex`; per-zone parallel tick | 🟡 hard, core rewrite (~8k lines) | ~2–4 wks | hundreds/zone, multi-core |
| **L3** | Unify world authority: combat/AI/skills/pickup run **once** in the zone tick (not per-session) | 🟡🔴 hard, cross-session, emergent bugs | ~4–8 wks | consistent same-map combat |
| **L4** | Cross-process zone split: loopback `ZoneOwner` → real RPC/handoff; map=node, walk-between-maps = node handoff | 🔴 distributed systems | ~6–12 wks | global server, horizontal scale |
| **L5** | Siege specials: Time Dilation, on-screen culling, AoE batch resolution, queue/instancing | 🔴 hardest in the field | ~4–10 wks + tuning | Sabuk-scale same-map |

L1 is pure win and independent of assets/multiplayer rework. L2–L3 are the
heavy "world authority" line from the gap assessment. L4–L5 are months of
distributed + capacity engineering, and L5 means "playable degraded," not
"flawless."

## Per-map server requirements (how to size, not guess)

Map cost is **not** map size — it's `concurrency × on-screen density × combat
frequency`.

| Map class | Concurrency | Density | Bottleneck | Node plan |
| --- | --- | --- | --- | --- |
| Leveling fields (most maps) | tens | low | monster AI tick (mobs > players) | **many maps per process** |
| Capital (Bichon) | hundreds | medium (NPCs/portals) | visibility broadcast + foot traffic | **one map per process** (after L2); L1 grid is the prerequisite |
| Boss/event maps | hundreds burst | high (one mob) | combat/AoE resolution | one map/process + **L3 authority** |
| **Sabuk siege** | hundreds–thousands | **extreme** | everything compounds | **dedicated high-spec node + TiDi + culling (L5)** |

### You cannot get real per-map numbers without a load test

Today the only evidence is 64 idle connections — **not** a same-screen combat
load. Before sizing machines, build a **same-map-N-players-fighting** harness
(separate from the connection load test):

1. Step-load 50 → 100 → 200 → … real sessions in one zone, all moving + casting.
2. Measure per tick: tick duration, p95 command latency, single-core CPU, RSS.
3. Find the knee where tick time exceeds budget (e.g. 100 ms) = **single-zone
   single-core capacity**.
4. Size: `cores_per_map ≈ peak_on-screen_load / single_core_capacity`.

This harness is ~1–2 days and is the acceptance ruler for every level above —
without it, optimization is blind.

## Status on this branch

- **L1 (players): shipped & verified.** `AoiGrid` primitive (5 unit tests incl.
  a 400-member brute-force superset proof) + wired into `diff_visibility_for`;
  `shared_zone` suite 131/131 (+2 new AOI regression tests). Visibility output
  is provably identical to the full scan; work is now bounded by local density.
- **Next increments**: (a) apply the grid to `diff_zone_object_visibility_for`
  (objects/monsters), (b) build the same-map combat load harness, (c) L2 ECS
  design doc. L3–L5 are multi-week and tracked here for planning.
