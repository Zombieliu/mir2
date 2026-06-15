# Client Engine Refactor Roadmap

Last updated: 2026-06-15

Purpose: recover the engine layering that Crystal already had but the web port
collapsed into `apps/web/app/page.tsx` (12,354 lines, ~347 packet cases), and
implement the frontend ownership split already written in
`docs/TECH-MODERNIZATION-RFC.md:61-95`. The backend (`protocol` / `gateway` /
`simulation`) is solid and authoritative — this roadmap is **client-side only**.

## Why (root cause)

`page.tsx` is the de-facto engine: it holds the world state, runs the "game
loop" as React reconciliation, dispatches 347 packets, and fires VFX/sound
inline. The pain (200ms movement jank, R2/atlas whack-a-mole, two competing
renderers) is the symptom of engine subsystems that have no layer to live in.

Crystal, by contrast, is cleanly layered:

- update vs draw separated, draw on a fixed 60 FPS timestep — `Crystal/Client/Forms/CMain.cs:102-123`, `:345-355`
- assets behind an interface (lazy WIL/WTL loader) — `Crystal/Client/MirGraphics/MLibrary.cs`
- network is a replayable queue — `Crystal/Client/MirNetwork/Network.cs` (`Process()` → `ProcessPacket()`)

## Target layers and current status

See `docs/ARCHITECTURE-CURRENT.md` for backend. Client target (top→bottom):

| Layer | Status | Action |
|---|---|---|
| UI overlay panels (React) | 🟢 in place | hold the line; never authoritative |
| Input (move/target/cast) | 🟠 misplaced | move toward Bevy (RFC `:79`) |
| World renderer (Bevy, single) | 🟠 misplaced | retire competing DOM WebGL2 layers |
| RHI (WebGPU/WebGL2 via wgpu) | 🟠 misplaced | stop branching on backend above the renderer |
| Client game loop (fixed step + interpolation) | 🔴 missing | build; Crystal had it, the port dropped it |
| World/scene state model (plain store) | 🟠 misplaced | extract from React `useState`/`useRef` |
| VFX / animation / sound + event bus | 🔴 missing | build; today inline in the packet switch |
| Client packet dispatch | 🟠 misplaced | split `dispatch → event → state` |
| Resource residency / streaming (R2 + IndexedDB) | 🟠 misplaced | extract from `original-client-shell.tsx` into a manager |
| Offline asset pipeline (WIL/PNG → atlas/KTX2 + manifest) | 🟠 reactive | make a build stage ("airlock"); runtime only loads the bundle |

## Phases

### Phase 0 — foundational modules (PARALLEL, additive, isolated worktrees)

Each is a NEW module that touches no existing app file, so they ship
concurrently without colliding and without risking the running client.

- **P0.1 world-model** — `apps/web/lib/world-model/`: a framework-agnostic store
  (the `WorldState` from `page.tsx:667-721` + the upsert/patch/remove helpers)
  with `subscribe(selector)` and `getSnapshot()`, plus a **steady-cadence,
  timestamped snapshot emitter** to replace the React-driven full-JSON push at
  `page.tsx:3526-3547`.
- **P0.2 game-events** — `apps/web/lib/game-events/`: a typed domain-event bus
  (`entityStruck`, `entityDied`, `magicCast`, `playSound`, …) plus VFX/sound
  subscribers, so packet handlers emit events instead of calling
  `playEntity*Sound` inline (today `page.tsx:6649-7079`).
- **P0.3 asset-pipeline** — `scripts/asset-pipeline/`: formalize the existing
  atlas generation into an offline build stage that emits `atlas pages + KTX2 +
  manifest`. The airlock: the runtime never sees a raw legacy asset.
- **P0.4 bevy-interp** — `apps/game-client/runtime/`: a snapshot buffer +
  render-time entity interpolation in the Bevy runtime, consuming timestamped
  snapshots instead of snapping to the latest full-world JSON.

### Phase 1 — integrate the game loop (SERIAL, orchestrator-owned)

Wire P0.1 + P0.4 into `page.tsx`: replace the `useState`/`useRef` world with the
store; replace the rAF full-JSON push with the steady snapshot emitter; Bevy
consumes timestamped snapshots and interpolates. Outcome: **React is no longer
the game loop** — fixes the 200ms jank.

### Phase 2 — route packets through events (SERIAL)

Replace inline VFX/sound calls in the 347-case switch with `gameEvents.emit(...)`;
P0.2 subscribers consume them. Move ephemeral visuals (damage floaters,
projectiles, chat bubbles) out of `WorldState` into the event/VFX layer.

### Phase 3 — renderer consolidation + residency (PARALLEL after Phase 1)

Make Bevy the single world renderer (retire `WebGl2EntityAtlasLayer` /
`WebGl2MapAtlasLayer`, fold DOM overlays in); extract the residency manager from
`original-client-shell.tsx`; wire P0.3 bundles; de-branch the RHI.

### Phase 4 — persistence normalization (backend lane)

Continue the RFC Phase 3 track (inventory/mail/economy → Postgres).

## Validation gates (every phase)

- web: `cd mir2-web3/apps/web && npx tsc --noEmit` (0 errors) + `npm run test:frontend-logic`
- rust: `cd mir2-web3 && cargo +1.89.0 test -p mir2-simulation -- --test-threads=1`, `cargo fmt --all --check`
- never run prettier (web has no prettier config)
