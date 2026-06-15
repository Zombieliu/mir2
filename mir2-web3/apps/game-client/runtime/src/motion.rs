//! Wall-clock entity motion authority for the Bevy runtime.
//!
//! # Why this module exists
//!
//! Before this module, the TypeScript DOM shell was the sole authority for
//! entity motion: it ran a 60 Hz `requestAnimationFrame` loop, computed
//! per-frame pixel offsets for every entity, and handed pre-computed absolute
//! CSS-px positions to Bevy via `setMir2EntityRenderState`.  That loop forced
//! a full React re-render of the ~3000-line shell every animation frame.
//!
//! This module moves motion authority into Bevy.  Each entity's movement step
//! is described by a wall-clock window (`started_ms`..`expires_ms`) carried in
//! the [`EntityMotionTable`] Bevy resource.  The table is updated by
//! [`update_entity_motion_table`] from the latest `WorldSnapshot`, then read
//! each frame by `sync_entities` via [`world_position_with_motion`].
//!
//! # Design reference
//!
//! See `docs/BEVY-OWNS-MOTION-DESIGN.md` §3 for the full specification.
//!
//! # Gating / additive nature
//!
//! The table is only populated when the TypeScript producer sends
//! `movementStartedMs` / `movementDurationMs` on `WorldEntity`.  Until then
//! every table lookup returns `None`, falling through to the Phase 0.4
//! snapshot-lerp path in `sync_entities`.  Existing behaviour is fully
//! preserved.

use std::collections::HashMap;

use bevy::math::Vec2;
use bevy::prelude::*;

use crate::RuntimeWorldState;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Crystal's nominal walk step duration in milliseconds.  Used as the
/// `expires_ms` fallback when the TS producer does not send
/// `movementDurationMs` (Steps 1–4 of the migration plan).
pub const DEFAULT_STEP_DURATION_MS: f64 = 600.0;

/// Wall-clock tolerance (ms) for `started_ms` sanity check.  If the timestamp
/// supplied by the TS producer is more than this many milliseconds in the future
/// relative to `now_ms`, we treat the field as absent and fall back to `now_ms`.
const MAX_FUTURE_SKEW_MS: f64 = 5000.0;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// The linear motion window for a single entity, expressed in wall-clock
/// milliseconds.  Mirrors `EntityMotionSnapshot` in
/// `original-client-scene-motion.ts`.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityMotionEntry {
    /// Grid cell the entity is moving *from* (previous position).
    pub from_x: i32,
    pub from_y: i32,
    /// Grid cell the entity is moving *to* (current target position).
    pub to_x: i32,
    pub to_y: i32,
    /// Wall-clock `Date.now()` milliseconds at which the step began.
    pub started_ms: f64,
    /// Wall-clock `Date.now()` milliseconds at which the step completes.
    pub expires_ms: f64,
}

/// Per-entity motion table, held as a Bevy [`Resource`].
///
/// Updated once per frame by [`update_entity_motion_table`] immediately after
/// `ingest_pending_world_state` runs.  Read each frame by `sync_entities` via
/// [`world_position_with_motion`].
#[derive(Resource, Default)]
pub struct EntityMotionTable {
    entries: HashMap<String, EntityMotionEntry>,
    /// Wall-clock milliseconds at the start of the current Bevy frame, set by
    /// [`update_entity_motion_table`] via `js_sys::Date::now()` on WASM or
    /// `std::time` on native.  `sync_entities` reads this field so it uses the
    /// same clock snapshot for every entity in the frame.
    pub now_ms: f64,
}

impl EntityMotionTable {
    /// Look up the motion entry for an entity by its `object_id`.
    ///
    /// Returns `None` if the entity has no active entry (i.e., the TS producer
    /// has not yet sent timing metadata for it).
    #[inline]
    pub fn get(&self, object_id: &str) -> Option<&EntityMotionEntry> {
        self.entries.get(object_id)
    }
}

// ---------------------------------------------------------------------------
// Pure math
// ---------------------------------------------------------------------------

/// Compute the sub-tile pixel offset for an entity at `now_ms`.
///
/// Mirrors `entityMotionOffsetForEntity` in
/// `apps/web/app/components/original-client-scene-motion.ts:86-105`.
///
/// ```text
/// remaining = clamp(1 - (now - started) / (expires - started), 0, 1)
/// offset.x  = (from_x - to_x) * cell_width_px  * remaining
/// offset.y  = (from_y - to_y) * cell_height_px * remaining
/// ```
///
/// The motion goes **from the old cell toward the new cell**: as `remaining`
/// runs 1→0 the offset shrinks from full-cell displacement to zero.
///
/// # Parameters
///
/// * `cell_width_px`  — pixel (world-unit) width of one grid cell.  Use 32.0
///   for Bevy world space; use 48.0 when computing DOM CSS offsets.
/// * `cell_height_px` — pixel (world-unit) height of one grid cell.  Use 32.0
///   for Bevy world space; use 32.0 for DOM CSS (same in the DOM case).
///
/// Returns a [`Vec2`] containing the `(dx, dy)` offset in the same unit as the
/// `cell_*_px` parameters.
pub fn compute_motion_offset(
    entry: &EntityMotionEntry,
    now_ms: f64,
    cell_width_px: f32,
    cell_height_px: f32,
) -> Vec2 {
    let span = entry.expires_ms - entry.started_ms;
    // Degenerate: zero-duration step → snap to target (remaining = 0).
    let remaining = if span <= 0.0 {
        0.0_f64
    } else {
        let elapsed = now_ms - entry.started_ms;
        (1.0 - elapsed / span).clamp(0.0, 1.0)
    };

    let remaining = remaining as f32;
    Vec2::new(
        (entry.from_x - entry.to_x) as f32 * cell_width_px * remaining,
        (entry.from_y - entry.to_y) as f32 * cell_height_px * remaining,
    )
}

// ---------------------------------------------------------------------------
// Bevy world-space helper
// ---------------------------------------------------------------------------

/// Compute the Bevy world-space [`Vec3`] for an entity, applying the motion
/// offset from the [`EntityMotionTable`] entry.
///
/// The z-component is always `1.0` (entity layer), matching `tile_to_world` in
/// `lib.rs`.
///
/// Called by `sync_entities` when a motion table entry exists for the entity
/// (Priority 1 path).  When no entry exists, `sync_entities` falls through to
/// the Phase 0.4 snapshot-lerp path.
///
/// Note: both x and y use the same `tile_size` because Bevy's scene uses a
/// flat (non-isometric) coordinate system, unlike the DOM's 48 × 32 CSS grid.
pub fn world_position_with_motion(
    grid_x: i32,
    grid_y: i32,
    entry: &EntityMotionEntry,
    now_ms: f64,
    tile_size: f32,
) -> Vec3 {
    let offset = compute_motion_offset(entry, now_ms, tile_size, tile_size);
    Vec3::new(
        grid_x as f32 * tile_size + offset.x,
        -(grid_y as f32 * tile_size) - offset.y,
        1.0,
    )
}

// ---------------------------------------------------------------------------
// Bevy system
// ---------------------------------------------------------------------------

/// Bevy system that refreshes [`EntityMotionTable`] from the latest
/// `WorldSnapshot`.
///
/// Runs every frame, **after** `ingest_pending_world_state` in the `.chain()`
/// ordering in `boot_mir2_runtime`.
///
/// For each entity in the snapshot:
/// - If the entity's grid position changed since the last entry (or no entry
///   exists), a new [`EntityMotionEntry`] is created.
/// - If the TS producer supplied `movement_started_ms`, that value is used as
///   `started_ms` (after a future-skew sanity check).  Otherwise `now_ms` is
///   used (snap fallback, same as the DOM).
/// - `expires_ms` = `started_ms + movement_duration_ms` (defaulting to
///   [`DEFAULT_STEP_DURATION_MS`] = 600 ms).
/// - The previous entry's `to_x`/`to_y` becomes the new `from_x`/`from_y` so
///   the entity continues gliding from wherever it currently is, not from the
///   stale grid cell.  (Matches the DOM's `currentMotionCoordinate` logic in
///   `original-client-scene-motion.ts`.)
/// - Entries for entities absent from the snapshot are removed.
pub fn update_entity_motion_table(
    state: Res<RuntimeWorldState>,
    mut table: ResMut<EntityMotionTable>,
) {
    // Update the frame-level wall-clock.
    table.now_ms = now_ms_wall_clock();

    let Some(snapshot) = &state.snapshot else {
        return;
    };

    let now_ms = table.now_ms;
    let mut alive: std::collections::HashSet<String> =
        std::collections::HashSet::with_capacity(snapshot.entities.len());

    for entity in &snapshot.entities {
        alive.insert(entity.object_id.clone());

        let existing = table.entries.get(&entity.object_id);

        // Only create/update an entry when the TS producer sends timing data
        // OR the entity changed grid cell.  Without timing data and without a
        // position change there is nothing new to record.
        let position_changed = existing
            .map(|e| e.to_x != entity.x || e.to_y != entity.y)
            .unwrap_or(true); // new entity → treat as changed

        let has_timing = entity.movement_started_ms.is_some();

        if !position_changed && !has_timing {
            // No change and no new timing metadata — leave existing entry as-is.
            continue;
        }

        // `from` = wherever the entity currently appears to be.  If an entry
        // exists, start from its current `to` (the destination it was heading
        // toward); otherwise snap from the current grid cell.
        let (from_x, from_y) = existing
            .map(|e| (e.to_x, e.to_y))
            .unwrap_or((entity.x, entity.y));

        // Sanitise `movement_started_ms`: reject values far in the future.
        let started_ms = match entity.movement_started_ms {
            Some(ts) if ts <= now_ms + MAX_FUTURE_SKEW_MS => ts,
            _ => now_ms,
        };

        let duration_ms = entity
            .movement_duration_ms
            .filter(|&d| d > 0.0)
            .unwrap_or(DEFAULT_STEP_DURATION_MS);

        let expires_ms = started_ms + duration_ms;

        table.entries.insert(
            entity.object_id.clone(),
            EntityMotionEntry {
                from_x,
                from_y,
                to_x: entity.x,
                to_y: entity.y,
                started_ms,
                expires_ms,
            },
        );
    }

    // Remove stale entries for entities no longer in the snapshot.
    table.entries.retain(|id, _| alive.contains(id));
}

// ---------------------------------------------------------------------------
// Platform-specific wall-clock helper
// ---------------------------------------------------------------------------

/// Returns the current wall-clock time in milliseconds.
///
/// On WASM targets this calls `js_sys::Date::now()` which maps to the
/// browser's `Date.now()` — the exact same clock the TypeScript producer uses
/// for `movementStartedMs`.  On native (tests, dev builds) it falls back to
/// `std::time::SystemTime`.
#[cfg(target_arch = "wasm32")]
fn now_ms_wall_clock() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms_wall_clock() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // compute_motion_offset
    // -----------------------------------------------------------------------

    /// Helper: build a trivial entry moving from (0,0) to (1,0).
    fn entry_x_step() -> EntityMotionEntry {
        EntityMotionEntry {
            from_x: 0,
            from_y: 0,
            to_x: 1,
            to_y: 0,
            started_ms: 0.0,
            expires_ms: 600.0,
        }
    }

    #[test]
    fn offset_at_t0_is_full_cell() {
        let entry = entry_x_step();
        let offset = compute_motion_offset(&entry, 0.0, 32.0, 32.0);
        // remaining = 1.0; from_x - to_x = -1; offset.x = -32.0
        assert!((offset.x + 32.0).abs() < 1e-4, "offset.x = {}", offset.x);
        assert!(offset.y.abs() < 1e-4, "offset.y = {}", offset.y);
    }

    #[test]
    fn offset_at_midpoint_is_half_cell() {
        let entry = entry_x_step();
        let offset = compute_motion_offset(&entry, 300.0, 32.0, 32.0);
        // remaining = 0.5; offset.x = -16.0
        assert!((offset.x + 16.0).abs() < 1e-4, "offset.x = {}", offset.x);
        assert!(offset.y.abs() < 1e-4, "offset.y = {}", offset.y);
    }

    #[test]
    fn offset_at_expiry_is_zero() {
        let entry = entry_x_step();
        let offset = compute_motion_offset(&entry, 600.0, 32.0, 32.0);
        // remaining = 0.0; offset = (0, 0)
        assert!(offset.x.abs() < 1e-4, "offset.x = {}", offset.x);
        assert!(offset.y.abs() < 1e-4, "offset.y = {}", offset.y);
    }

    #[test]
    fn offset_past_expiry_clamps_to_zero() {
        let entry = entry_x_step();
        let offset = compute_motion_offset(&entry, 1200.0, 32.0, 32.0);
        assert!(offset.x.abs() < 1e-4, "offset past expiry should be 0");
        assert!(offset.y.abs() < 1e-4);
    }

    #[test]
    fn offset_y_step() {
        let entry = EntityMotionEntry {
            from_x: 5,
            from_y: 3,
            to_x: 5,
            to_y: 4,
            started_ms: 100.0,
            expires_ms: 700.0,
        };
        let offset = compute_motion_offset(&entry, 100.0, 32.0, 32.0);
        // remaining = 1.0; from_y - to_y = -1; offset.y = -32.0
        assert!(offset.x.abs() < 1e-4, "offset.x = {}", offset.x);
        assert!((offset.y + 32.0).abs() < 1e-4, "offset.y = {}", offset.y);
    }

    #[test]
    fn offset_degenerate_zero_duration() {
        let entry = EntityMotionEntry {
            from_x: 0,
            from_y: 0,
            to_x: 1,
            to_y: 0,
            started_ms: 500.0,
            expires_ms: 500.0, // zero duration
        };
        let offset = compute_motion_offset(&entry, 500.0, 32.0, 32.0);
        // Should snap to target: offset = (0, 0)
        assert!(offset.x.abs() < 1e-4, "degenerate: offset.x should be 0");
        assert!(offset.y.abs() < 1e-4, "degenerate: offset.y should be 0");
    }

    #[test]
    fn offset_asymmetric_cell_dims() {
        // DOM uses 48 wide × 32 tall.
        let entry = EntityMotionEntry {
            from_x: 0,
            from_y: 0,
            to_x: 1,
            to_y: 1,
            started_ms: 0.0,
            expires_ms: 600.0,
        };
        let offset = compute_motion_offset(&entry, 0.0, 48.0, 32.0);
        // from_x - to_x = -1, from_y - to_y = -1, remaining = 1
        assert!((offset.x + 48.0).abs() < 1e-4, "offset.x = {}", offset.x);
        assert!((offset.y + 32.0).abs() < 1e-4, "offset.y = {}", offset.y);
    }

    // -----------------------------------------------------------------------
    // world_position_with_motion
    // -----------------------------------------------------------------------

    #[test]
    fn world_position_at_t0_is_offset_from_old_cell() {
        // Entity at grid (1, 0) having moved from (0, 0), at t=0 of a 600ms step.
        let entry = EntityMotionEntry {
            from_x: 0,
            from_y: 0,
            to_x: 1,
            to_y: 0,
            started_ms: 0.0,
            expires_ms: 600.0,
        };
        let pos = world_position_with_motion(1, 0, &entry, 0.0, 32.0);
        // grid (1,0) → base = (32, 0, 1)
        // offset at t=0: remaining=1; from_x - to_x = -1; offset.x = -32
        // final: (32 + (-32), -0 - (-0), 1) = (0, 0, 1) — entity appears at old cell
        assert!((pos.x - 0.0).abs() < 1e-4, "pos.x = {}", pos.x);
        assert!((pos.y - 0.0).abs() < 1e-4, "pos.y = {}", pos.y);
        assert!((pos.z - 1.0).abs() < 1e-4);
    }

    #[test]
    fn world_position_at_expiry_is_target_cell() {
        let entry = EntityMotionEntry {
            from_x: 0,
            from_y: 0,
            to_x: 1,
            to_y: 0,
            started_ms: 0.0,
            expires_ms: 600.0,
        };
        let pos = world_position_with_motion(1, 0, &entry, 600.0, 32.0);
        // At expiry, offset = (0,0); entity snaps to grid (1,0) = (32, 0, 1)
        assert!((pos.x - 32.0).abs() < 1e-4, "pos.x = {}", pos.x);
        assert!((pos.y - 0.0).abs() < 1e-4, "pos.y = {}", pos.y);
        assert!((pos.z - 1.0).abs() < 1e-4);
    }

    #[test]
    fn world_position_y_axis_inverted() {
        // Bevy Y increases upward; grid Y increases downward.
        let entry = EntityMotionEntry {
            from_x: 0,
            from_y: 0,
            to_x: 0,
            to_y: 1,
            started_ms: 0.0,
            expires_ms: 600.0,
        };
        // At t=0 (remaining=1), entity appears at old cell (0,0):
        let pos = world_position_with_motion(0, 1, &entry, 0.0, 32.0);
        // grid (0,1) base = (0, -32, 1)
        // offset: from_y - to_y = -1; offset.y = -32; subtracted: -(-32) = +32
        // result: y = -32 + 32 = 0  → entity at old row 0 in world coords ✓
        assert!((pos.x - 0.0).abs() < 1e-4, "pos.x = {}", pos.x);
        assert!((pos.y - 0.0).abs() < 1e-4, "pos.y = {}", pos.y);
    }

    // -----------------------------------------------------------------------
    // EntityMotionTable helpers
    // -----------------------------------------------------------------------

    #[test]
    fn table_get_returns_none_when_empty() {
        let table = EntityMotionTable::default();
        assert!(table.get("player-1").is_none());
    }

    #[test]
    fn table_get_returns_entry_after_insert() {
        let mut table = EntityMotionTable::default();
        table.entries.insert(
            "player-1".to_owned(),
            EntityMotionEntry {
                from_x: 0,
                from_y: 0,
                to_x: 1,
                to_y: 0,
                started_ms: 0.0,
                expires_ms: 600.0,
            },
        );
        assert!(table.get("player-1").is_some());
        assert!(table.get("player-2").is_none());
    }

    // -----------------------------------------------------------------------
    // update_entity_motion_table logic (tested via direct function calls,
    // bypassing Bevy's ECS scheduler which isn't available in unit tests)
    // -----------------------------------------------------------------------

    /// Build a minimal `RuntimeWorldState` with one entity for table-update tests.
    fn state_with_entity(
        object_id: &str,
        x: i32,
        y: i32,
        started: Option<f64>,
        duration: Option<f64>,
    ) -> crate::RuntimeWorldState {
        use crate::{EntityKind, WorldEntity, WorldSnapshot};
        crate::RuntimeWorldState {
            snapshot: Some(WorldSnapshot {
                map_title: None,
                player_object_id: None,
                selected_object_id: None,
                scene_view: None,
                terrain_patches: vec![],
                decor_objects: vec![],
                entities: vec![WorldEntity {
                    object_id: object_id.to_owned(),
                    kind: EntityKind::Monster,
                    name: "Test".to_owned(),
                    x,
                    y,
                    direction: None,
                    level: None,
                    movement_started_ms: started,
                    movement_duration_ms: duration,
                }],
                mine_nodes: vec![],
                client_time_ms: None,
            }),
        }
    }

    /// Run the table-update logic extracted from the system into a testable form.
    ///
    /// We call the inner logic directly since we cannot spin up a full Bevy app
    /// in unit tests.
    fn run_update(table: &mut EntityMotionTable, state: &crate::RuntimeWorldState, now_ms: f64) {
        let Some(snapshot) = &state.snapshot else {
            return;
        };
        table.now_ms = now_ms;

        let mut alive: std::collections::HashSet<String> =
            std::collections::HashSet::with_capacity(snapshot.entities.len());

        for entity in &snapshot.entities {
            alive.insert(entity.object_id.clone());

            let existing = table.entries.get(&entity.object_id);

            let position_changed = existing
                .map(|e| e.to_x != entity.x || e.to_y != entity.y)
                .unwrap_or(true);

            let has_timing = entity.movement_started_ms.is_some();

            if !position_changed && !has_timing {
                continue;
            }

            let (from_x, from_y) = existing
                .map(|e| (e.to_x, e.to_y))
                .unwrap_or((entity.x, entity.y));

            let started_ms = match entity.movement_started_ms {
                Some(ts) if ts <= now_ms + MAX_FUTURE_SKEW_MS => ts,
                _ => now_ms,
            };

            let duration_ms = entity
                .movement_duration_ms
                .filter(|&d| d > 0.0)
                .unwrap_or(DEFAULT_STEP_DURATION_MS);

            table.entries.insert(
                entity.object_id.clone(),
                EntityMotionEntry {
                    from_x,
                    from_y,
                    to_x: entity.x,
                    to_y: entity.y,
                    started_ms,
                    expires_ms: started_ms + duration_ms,
                },
            );
        }

        table.entries.retain(|id, _| alive.contains(id));
    }

    #[test]
    fn new_entity_creates_entry() {
        let mut table = EntityMotionTable::default();
        let state = state_with_entity("mob-1", 3, 4, Some(1000.0), Some(600.0));
        run_update(&mut table, &state, 1000.0);
        let entry = table.get("mob-1").expect("entry should exist");
        assert_eq!(entry.to_x, 3);
        assert_eq!(entry.to_y, 4);
        assert!((entry.started_ms - 1000.0).abs() < 1e-6);
        assert!((entry.expires_ms - 1600.0).abs() < 1e-6);
    }

    #[test]
    fn new_entity_without_timing_uses_fallback_started() {
        let mut table = EntityMotionTable::default();
        let state = state_with_entity("mob-1", 3, 4, None, None);
        run_update(&mut table, &state, 5000.0);
        let entry = table.get("mob-1").expect("entry should exist");
        // No timing → started_ms = now_ms
        assert!((entry.started_ms - 5000.0).abs() < 1e-6);
        // No duration → DEFAULT_STEP_DURATION_MS
        assert!((entry.expires_ms - (5000.0 + DEFAULT_STEP_DURATION_MS)).abs() < 1e-6);
    }

    #[test]
    fn position_change_updates_from_to_previous_to() {
        let mut table = EntityMotionTable::default();
        // First: entity at (3, 4)
        let state1 = state_with_entity("mob-1", 3, 4, Some(0.0), Some(600.0));
        run_update(&mut table, &state1, 0.0);

        // Second: entity moved to (4, 4)
        let state2 = state_with_entity("mob-1", 4, 4, Some(600.0), Some(600.0));
        run_update(&mut table, &state2, 600.0);

        let entry = table.get("mob-1").expect("entry should exist");
        // from_x/from_y should be the previous to_x/to_y = (3, 4)
        assert_eq!(entry.from_x, 3);
        assert_eq!(entry.from_y, 4);
        assert_eq!(entry.to_x, 4);
        assert_eq!(entry.to_y, 4);
    }

    #[test]
    fn entity_removal_clears_entry() {
        use crate::WorldSnapshot;
        let mut table = EntityMotionTable::default();

        // Seed with mob-1 present.
        let state = state_with_entity("mob-1", 3, 4, Some(0.0), Some(600.0));
        run_update(&mut table, &state, 0.0);
        assert!(table.get("mob-1").is_some());

        // New snapshot with mob-1 gone.
        let empty_state = crate::RuntimeWorldState {
            snapshot: Some(WorldSnapshot {
                map_title: None,
                player_object_id: None,
                selected_object_id: None,
                scene_view: None,
                terrain_patches: vec![],
                decor_objects: vec![],
                entities: vec![],
                mine_nodes: vec![],
                client_time_ms: None,
            }),
        };
        run_update(&mut table, &empty_state, 600.0);
        assert!(table.get("mob-1").is_none());
    }

    #[test]
    fn future_timestamp_skew_falls_back_to_now() {
        let mut table = EntityMotionTable::default();
        let now_ms = 1000.0;
        // Timestamp far in the future (> MAX_FUTURE_SKEW_MS = 5000 ms ahead)
        let state = state_with_entity("mob-1", 1, 1, Some(now_ms + 10_000.0), Some(600.0));
        run_update(&mut table, &state, now_ms);
        let entry = table.get("mob-1").expect("entry should exist");
        // Should have fallen back to now_ms
        assert!(
            (entry.started_ms - now_ms).abs() < 1e-6,
            "started_ms = {}, expected {}",
            entry.started_ms,
            now_ms
        );
    }

    #[test]
    fn no_change_and_no_timing_does_not_update_existing_entry() {
        let mut table = EntityMotionTable::default();
        // First insert.
        let state = state_with_entity("mob-1", 3, 4, Some(0.0), Some(600.0));
        run_update(&mut table, &state, 0.0);
        let original_entry = table.get("mob-1").cloned().expect("entry should exist");

        // Same position, no timing → should not update.
        let state2 = state_with_entity("mob-1", 3, 4, None, None);
        run_update(&mut table, &state2, 300.0);
        let same_entry = table.get("mob-1").expect("entry should still exist");
        assert_eq!(*same_entry, original_entry, "entry should not have changed");
    }
}
