# Bevy-Owns-Motion Design

## Overview

This document describes how entity motion authority will be transferred from the
React DOM shell to the Bevy WASM runtime.  It records the current state, explains
why the DOM loop exists today, identifies exactly what can and cannot be removed
from that loop, and gives an ordered migration plan that can be executed and
verified one step at a time.

---

## 1. Current DOM-motion-authority flow (file:line citations)

### 1.1 The 60 Hz rAF clock

`apps/web/app/original-client-shell.tsx:748-758`

```typescript
const updateMotionClock = () => {
  setMotionNow(Date.now());
  animationFrame = window.requestAnimationFrame(updateMotionClock);
};
animationFrame = window.requestAnimationFrame(updateMotionClock);
```

`motionNow` is a React `useState(0)` (line 379) that is updated every animation
frame while `screen === "game"`.  Because it is React state, every rAF tick forces
a re-render of the **entire ~3000-line shell**.

### 1.2 `refreshEntityMotionSnapshots` — the motion model

`apps/web/app/components/original-client-scene-motion.ts:107-210`
`apps/web/app/original-client-shell.tsx:911-917`

Called on every render (every rAF tick) with the current `world.entities` list and
`motionNow`.  For each entity it produces an `EntityMotionSnapshot`:

```typescript
type EntityMotionSnapshot = {
  fromX: number; fromY: number;   // grid coords at start of motion
  toX: number;   toY: number;     // grid coords at end of motion
  animationState: EntitySpriteAnimationState;
  startedAt: number;              // ms timestamp (Date.now()-based)
  expiresAt: number;              // ms timestamp
};
```

The snapshot carries a wall-clock (`Date.now()`) window: `startedAt` is either the
`movementStartedAt` stamp sent in the server packet or `now`; `expiresAt` is either
the `movementUntil` stamp from the packet or `now + lifetime`.

### 1.3 `entityMotionOffsetForEntity` — per-entity pixel offset

`apps/web/app/components/original-client-scene-motion.ts:86-105`

Returns a `{x, y}` pixel offset for an entity relative to its integer grid cell,
computed as:

```
remaining = 1 - (now - startedAt) / (expiresAt - startedAt)
offset = (fromX - toX) * VIEWPORT_CELL_WIDTH  * remaining
       = (fromY - toY) * VIEWPORT_CELL_HEIGHT * remaining
```

`VIEWPORT_CELL_WIDTH = 48px`, `VIEWPORT_CELL_HEIGHT = 32px`
(`apps/web/app/components/original-client-scene-layout.ts:4-5`)

The motion goes **from the old cell toward the new cell**: as `remaining` runs
1→0 the offset shrinks from full-cell offset to zero.

### 1.4 `cameraMotionOffsetForEntity` — camera lag offset for the player

`apps/web/app/components/original-client-scene-motion.ts:212-231`

Same math but inverted sign on both axes.  Applied only to the player's entity so
the camera "catches up" while the player sprite glides to the new position.

### 1.5 `buildBevyEntityRenderState` — the hand-off to Bevy

`apps/web/app/original-client-shell.tsx:1163-1172` (call site),
`apps/web/app/original-client-shell.tsx:2332-2443` (function body)

Every rAF tick, after `refreshEntityMotionSnapshots` updates the motion map, the
shell calls `buildBevyEntityRenderState`.  This function:

1. Calls `entityMotionOffsetForEntity` for every non-player entity in the viewport
   (line 2397-2399).
2. Uses `playerCameraMotionOffset` for all other entities' camera adjustment
   (line 2400).
3. Computes each layer's absolute `left`/`top` in CSS-px stage coordinates
   (lines 2401-2404):

   ```
   rootLeft = VIEWPORT_ENTITY_LEFT_ORIGIN
              + entity.dx * VIEWPORT_CELL_WIDTH
              + cameraOffset.x
              + entityMotionOffset.x
   rootTop  = VIEWPORT_ENTITY_TOP_ORIGIN
              + entity.dy * VIEWPORT_CELL_HEIGHT
              + cameraOffset.y
              + entityMotionOffset.y
   ```

4. Serialises the result to JSON and passes it to Bevy via
   `setMir2EntityRenderState` (`apps/web/app/original-client-shell.tsx:1421`).

**Key insight**: Bevy currently receives pre-computed, motioned, absolute CSS-px
positions.  It never sees raw grid coordinates + a timestamp; it cannot interpolate
independently.

### 1.6 DOM overlay consumers that also need `motionNow`

| Consumer | Location | Uses `motionNow` for |
|---|---|---|
| Reconnect countdown timer | shell.tsx:392 | `(nextAttemptAt - motionNow) / 1000` |
| Viewport projectiles filter + progress | shell.tsx:1068-1079 | `expiresAt > motionNow`, `projectileProgress(…, motionNow)` |
| Chat bubbles | shell.tsx:1086 | `deriveChatBubbles(…, motionNow)` bubble expiry |
| DOM entity sprite overlay | `original-client-scene-visual-layers.tsx` | `entityMotionOffsetForEntity` per entity in DOM path |
| DOM entity/camera offsets | `original-client-scene-overlays.tsx` | name-plates, damage floaters anchored to entity position |
| `buildBevyEntityRenderState` | shell.tsx:2332 | per-entity `left`/`top` passed to Bevy (see §1.5) |

---

## 2. Target: Bevy interpolates entity motion authoritatively

### 2.1 What changes

Instead of the DOM computing `left`/`top` per-frame and feeding it to Bevy,
Bevy will:

1. Receive the **raw grid position + timing metadata** (grid coords, step duration,
   step start timestamp) for each entity via the existing `WorldSnapshot` push.
2. Run its own **wall-clock motion model** inside the Bevy Update loop at the
   native rAF rate (~60 fps on all modern browsers when Bevy drives the canvas).
3. Position its own sprites from that model — no DOM involvement.

The `WorldSnapshot` already carries `x`/`y` (grid) and `client_time_ms` per entity
(Phase 0.4 added `client_time_ms` on the root snapshot).  We extend `WorldEntity`
with `movement_started_ms` and `movement_until_ms` so Bevy can reproduce exactly
the same linear interpolation Crystal drives.

The DOM side still needs a clock for the three consumers that are legitimately DOM:

- **Reconnect countdown** — 1 Hz resolution; `Date.now()` inline is fine.
- **Projectiles** — expiry filter and progress are pure math.  These can stay
  DOM-side until projectile rendering is also moved to Bevy (a later phase).
- **Chat bubbles** — expiry is checked at the 120 ms animation tick, not per-frame.
  These stay DOM-side; they do not need 60 Hz.

### 2.2 What the DOM shell still legitimately does

After the motion authority move:

| Still-DOM responsibility | Why Bevy cannot own it yet |
|---|---|
| DOM entity overlay (fallback / WebGL2 path) | `sync_entity_render_layers` is the Bevy path; DOM is the fallback when Bevy renderer is off. |
| Camera CSS transform for DOM tiles + overlays | The DOM map layer and name-plate overlay need a CSS `translate` that mirrors Bevy's camera.  Requires a new read-back channel from Bevy. |
| Projectile DOM overlay | Arrow/magic projectiles are DOM sprites (no Bevy path yet). |
| Damage floaters, chat bubbles | DOM-only overlays. |
| Reconnect countdown | Trivially DOM. |

### 2.3 The per-frame DOM loop after the motion authority move

Once Bevy owns motion, `motionNow` is only needed for:

1. Projectile `expiresAt` check and `progress` — at most 60 Hz is fine.
2. `deriveChatBubbles` — can be throttled to 30 Hz or the 120 ms tick.
3. Reconnect countdown — 1 Hz is enough.
4. Camera CSS alignment for the DOM overlay — can read from a Bevy-supplied
   `ref` or SharedArrayBuffer rather than recomputing every frame.

With those reduced, the shell can drop from a `useState`-driven rAF loop (forces
full React re-render every frame) to a `useRef`-based imperative DOM update — or
keep the rAF but stop triggering a React state update for the camera alignment,
letting it be applied directly to the DOM via `ref.current.style.transform`.

---

## 3. Rust additions: motion-authority module

The additions live in `apps/game-client/runtime/src/motion.rs` (new file).

They are **purely additive**: existing `sync_entities` snap/lerp behavior is
unchanged.  The new module adds:

### 3.1 `EntityMotionEntry` — per-entity motion state (Bevy Resource)

Stores, per `object_id`, the current linear motion window in **wall-clock
milliseconds** (mirroring the DOM `EntityMotionSnapshot` model):

```rust
pub struct EntityMotionEntry {
    pub from_x: i32,
    pub from_y: i32,
    pub to_x: i32,
    pub to_y: i32,
    pub started_ms: f64,   // wall-clock ms (from client_time_ms + delay)
    pub expires_ms: f64,
}
```

### 3.2 `EntityMotionTable` — Bevy Resource wrapping the per-entity map

A `HashMap<String, EntityMotionEntry>` + a `wall_clock_ms: f64` field updated
each Bevy frame from `js_sys::Date::now()`.

### 3.3 `compute_motion_offset` — pure math, fully unit-tested

```rust
pub fn compute_motion_offset(
    entry: &EntityMotionEntry,
    now_ms: f64,
    cell_width_px:  f32,   // 48.0
    cell_height_px: f32,   // 32.0
) -> Vec2  // (dx_px, dy_px) from the entity's grid-cell origin
```

Mirrors `entityMotionOffsetForEntity` in `scene-motion.ts`:

```
remaining = 1 - (now - started) / (expires - started)
offset.x  = (from_x - to_x) * cell_width  * remaining
offset.y  = (from_y - to_y) * cell_height * remaining
```

### 3.4 `update_entity_motion_table` — Bevy system

Runs after `ingest_pending_world_state`.  For each entity in the new snapshot it:

1. Checks whether the entity's grid position changed.
2. If yes, creates or replaces the `EntityMotionEntry`:
   - `started_ms` = `snapshot.client_time_ms` if present and reasonable, else
     `now_ms` (same fallback as the DOM).
   - `expires_ms` = `started_ms + step_duration_ms` where `step_duration_ms` is
     the new optional `WorldEntity.movement_duration_ms` field, defaulting to 600ms
     (Crystal's nominal walk step).
3. Stores the previous `to_x`/`to_y` as the new `from_x`/`from_y` so motion
   continues from wherever the entity currently is (matches the DOM
   `currentMotionCoordinate` logic).
4. Removes entries for entities that disappeared from the snapshot.

### 3.5 `world_position_with_motion` — helper used by `sync_entities`

```rust
pub fn world_position_with_motion(
    grid_x: i32,
    grid_y: i32,
    entry: &EntityMotionEntry,   // caller pre-looks up from EntityMotionTable
    now_ms: f64,                 // table.now_ms, snapshotted once per frame
    tile_size: f32,
) -> Vec3
```

Returns the Bevy world-space `Vec3` for an entity, incorporating the motion offset
so sprite transforms are placed at the smoothly-interpolated position rather than
the snapped grid cell.  The caller (`sync_entities`) does the table lookup and passes
the entry directly, so this function is pure and cheaply testable.

Note: Bevy's scene uses a symmetric flat coordinate system (32 × 32 px per tile),
unlike the DOM's isometric 48 × 32 CSS grid.  Both axes therefore use `tile_size`
in this function; `compute_motion_offset` accepts separate `cell_width_px` /
`cell_height_px` for future DOM-readback use cases.

### 3.6 `now_ms_wall_clock` — platform-local clock helper

```rust
#[cfg(target_arch = "wasm32")]
fn now_ms_wall_clock() -> f64 { js_sys::Date::now() }

#[cfg(not(target_arch = "wasm32"))]
fn now_ms_wall_clock() -> f64 { /* std::time::SystemTime */ }
```

On WASM, calls `js_sys::Date::now()` which maps to `Date.now()` in the browser —
the same clock the TypeScript producer uses for `movementStartedMs`.  On native
(unit tests, CI) falls back to `std::time::SystemTime`.  This ensures no skew
between Bevy and the TS producer on WASM.

### 3.7 `MAX_FUTURE_SKEW_MS` — sanity gate

`movement_started_ms` values more than 5000 ms in the future relative to `now_ms`
are rejected and replaced with `now_ms`.  This guards against corrupt timestamps
from a stale packet being replayed after a client reconnect.

---

## 4. Step-by-step migration plan

Each step is **independently verifiable** in the running client (Vercel preview or
`npm run dev`).  No step removes anything until the replacement is confirmed
working.

---

### Step 1 — Extend `WorldEntity` with motion timing fields (additive, gated) ✓ DONE

**Files**: `apps/game-client/runtime/src/lib.rs`

Add optional fields to `WorldEntity`:

```rust
#[serde(default)]
pub(crate) movement_started_ms: Option<f64>,
#[serde(default)]
pub(crate) movement_duration_ms: Option<f64>,
```

These fields are ignored until Step 4.  No behavior change.  `pub(crate)` visibility
was added so `motion.rs` (a sibling module) can read them directly.

**Verification**: `cargo +1.89.0 check --target wasm32-unknown-unknown` passes.  In-game: identical to before.

---

### Step 2 — Extend `WorldSnapshot` with a reliable `client_time_ms`

**Files**: `apps/game-client/runtime/src/lib.rs`

The Phase 0.4 work added `client_time_ms` but marked it `#[allow(dead_code)]` and
did not use it for interpolation.  In this step we **use it as the origin clock
offset** to convert absolute JS `Date.now()` timestamps on `movement_started_ms`
and `movement_until_ms` into Bevy-local offsets.

No changes to the TS producer yet — we just start reading the existing field.

**Verification**: no observable change; log the offset in a debug build.

---

### Step 3 — Implement `motion.rs` with unit tests ✓ DONE

**Files**: `apps/game-client/runtime/src/motion.rs` (new, 370 lines)

Implemented `EntityMotionEntry`, `EntityMotionTable`, `compute_motion_offset`,
`update_entity_motion_table`, `world_position_with_motion`, and `now_ms_wall_clock`
as described in §3.  Wired into `boot_mir2_runtime` as an inserted Resource and
system, running after `ingest_pending_world_state` in the `.chain()`.

18 unit tests cover:
- `compute_motion_offset` at t=0, t=midpoint, t=expired, t=past-expiry, degenerate
  zero-duration, asymmetric cell dims.
- `world_position_with_motion` start/end/y-axis-inversion.
- Table: empty lookup, insert, new entity, position change, removal, future-skew
  rejection, no-change-no-timing no-op.

**Verification**: `cargo +1.89.0 test` → **33 tests pass, 0 warnings**.  Native and
WASM (`--target wasm32-unknown-unknown`) both `Finished` clean.  In-game: entity
positions are identical to before (motion offsets computed but gated — no TS
producer sends timing metadata yet, so every `motion_table.get()` returns `None`
and the existing snapshot-lerp path runs unchanged).

---

### Step 4 — Apply motion offsets to Bevy entity transforms ✓ DONE (gated)

**Files**: `apps/game-client/runtime/src/lib.rs`

`sync_entities` already contains the Priority-1 / Priority-2 / Priority-3 dispatch
(written by the prior agent):

```rust
let position = if let Some(entry) = motion_table.get(&entity_data.object_id) {
    motion::world_position_with_motion(
        entity_data.x, entity_data.y,
        entry,
        motion_table.now_ms,
        TILE_SIZE,
    )
} else {
    // Priority 2: Phase 0.4 snapshot lerp (fallback)
    // Priority 3: snap to grid cell
    …
};
```

The motion path is live but **gated** — it only activates when the TypeScript
producer starts sending `movementStartedMs` / `movementDurationMs` fields on
`WorldEntity` (Step 5).  Until then the table is empty and the existing snapshot-
lerp runs unchanged.  No `#[cfg(feature)]` guard is needed because the fallback
is the default code path.

**Verification**: In-game: entities glide smoothly between cells in Bevy's
block-primitive renderer without jitter.  Compare against the DOM overlay: the
Bevy sprites and DOM name-plates should move in sync.

---

### Step 5 — Pass motion metadata from the TS producer

**Files**: `apps/web/app/original-client-shell.tsx` (read-only analysis)

At this step we read the analysis done in Step 1-4 to confirm what fields the
TS producer needs to add to the `WorldSnapshot` JSON.  The existing
`client_time_ms` is already serialised on the root snapshot.

Extend `WorldEntity` in the TS-side snapshot builder to add:

```typescript
movementStartedMs?: number;    // = entity.movementStartedAt ?? undefined
movementDurationMs?: number;   // = (entity.movementUntil ?? 0) - (entity.movementStartedAt ?? 0)
```

This is done by whoever edits `page.tsx` / the snapshot builder — the DOM agent's
lane.  This step produces the data Bevy consumes in Step 4.

**Verification**: Log `motion_table` entries in a debug build and confirm
`started_ms` / `expires_ms` match the DOM's `EntityMotionSnapshot`.

---

### Step 6 — Validate visual parity between Bevy motion and DOM motion

**Method**: With both `useBevyEntityRenderer = true` AND DOM overlay active, use
the browser's DOM inspector / `window.__mir2SceneMotionDebug` to read the DOM
motion offsets and compare them to the Bevy sprite positions.  They should be
within ±1px at all times during a walk step.

**Verification criterion**: a screen-recorded walk sequence shows no perceptible
difference between the DOM sprite overlay and the Bevy sprite underneath it.

---

### Step 7 — Remove motion offset computation from `buildBevyEntityRenderState`

**Files**: `apps/web/app/original-client-shell.tsx`

Once Step 6 passes, remove the `entityMotionOffset` addition inside
`buildBevyEntityRenderState` (lines 2397-2404 at time of writing) so Bevy
receives **static grid-relative positions** (no per-frame pixel offset).
Bevy applies the motion offset internally via the `EntityMotionTable`.

This means `buildBevyEntityRenderState` no longer needs `motionNow` or
`entityMotionSnapshots` for the Bevy path.

**Verification**: Step 6 test repeated — visual parity unchanged.

---

### Step 8 — Slim the DOM rAF loop

Once Step 7 is done, `motionNow` is only needed for:
- Projectile expiry / progress
- DOM entity fallback overlay (only active when Bevy renderer is off)
- Chat bubble expiry

Reduce `setMotionNow(Date.now())` to trigger only when one of these is actually
visible.  For the projectile path, switch to an imperative `ref`-based clock that
does not trigger React state.

**Verification**: CPU profiler (Chrome DevTools) shows the JS frame time for the
shell's rAF callback drops from ~2–4 ms to <0.5 ms when no projectiles or chat
bubbles are active.

---

### Step 9 — Remove the camera CSS offset from the DOM entity overlay

**Files**: DOM overlay components (`original-client-scene-overlays.tsx`,
`original-client-scene-visual-layers.tsx`)

The DOM name-plate / damage-floater overlay needs a camera offset that tracks the
player's sub-tile motion.  After Bevy owns motion, add a new **read-back API** so
the Bevy runtime exposes the player's current interpolated world position to the
DOM:

```typescript
// New wasm-bindgen export from motion.rs:
getMir2PlayerMotionOffset(): { x: number; y: number } | null
```

The DOM overlay reads this once per rAF for camera alignment, replacing
`playerCameraMotionOffset` (which currently requires the full motion re-render).

**Verification**: Name-plates and damage floaters remain correctly anchored over
moving entities.

---

### Step 10 — Delete the DOM motion clock gate

Once Steps 7–9 are done and verified, `motionNow` as a React `useState` can be
replaced by a `useRef<number>` read imperatively.  The rAF is retained only as a
lightweight imperative loop updating the ref — no React state update, no shell
re-render.

Projectiles and chat bubbles read the ref at render time.  Their updates are
already gated by their own expiry windows so they naturally throttle.

**Verification**: Chrome Performance panel: `setMotionNow` disappears from the
call stack; React re-renders for the shell drop from ~60/sec to event-driven only.

---

## 5. Scope, risks, and gating

| Risk | Mitigation |
|---|---|
| Bevy wall-clock (`js_sys::Date::now()`) differs from DOM `Date.now()` | Both call JS `Date.now()`; no skew possible. |
| Motion snaps on first packet (buffer not ready) | Same fallback as today: if `started_ms` is absent the motion snaps to the target instantly. |
| `movement_duration_ms` not yet provided by TS producer | Default 600ms matches Crystal's nominal walk; will not look worse than today's snap. |
| TS producer still sends legacy offsets during migration | Bevy ignores extra JSON fields; DOM ignores Bevy's internal offsets. Both run in parallel during Steps 4–6. |
| Regression in DOM entity fallback path | DOM path is untouched through Steps 1–6 so the fallback renderer is safe. |

---

## 6. Files affected

| File | Role in this migration |
|---|---|
| `apps/game-client/runtime/src/motion.rs` | **New ✓** — motion authority module (370 lines, 18 unit tests) |
| `apps/game-client/runtime/src/lib.rs` | **Done ✓** — `WorldEntity`/`WorldSnapshot`/`RuntimeWorldState` marked `pub(crate)`; `WorldEntity` timing fields added; `motion.rs` wired; priority dispatch in `sync_entities` (Steps 1, 3, 4) |
| `apps/game-client/runtime/src/interpolation.rs` | Unchanged — retained as Priority-2 fallback |
| `apps/web/app/original-client-shell.tsx` | Steps 5, 7, 8, 10 — DOM-agent lane |
| `apps/web/app/components/original-client-scene-motion.ts` | Steps 8, 9 — reduce consumers |
| `apps/web/app/components/original-client-scene-overlays.tsx` | Step 9 — camera alignment |
| `apps/web/app/components/original-client-scene-visual-layers.tsx` | Step 9 — fallback DOM sprites |
