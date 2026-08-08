//! Packet-driven remote entity presentation for the packed Bevy renderer.
//!
//! This resource is presentation-only. It consumes copies of normalized
//! movement events, never sends commands, and never mutates authoritative game
//! state. The existing TypeScript motion window remains the fallback path.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;
use serde::Serialize;

use crate::motion;
use crate::movement_shadow::{normalized_move_phase_count, MovementShadowEvent};

const MAX_PENDING_EVENT_JSON: usize = 256;
const MAX_PRESENTATION_ENTRIES: usize = 256;
const MAX_DECODE_ERROR_CHARS: usize = 512;
const MOVE_PHASE_INTERVAL_MS: f64 = 100.0;
const MAX_SMOOTH_TILE_DISTANCE: f32 = 3.0;

thread_local! {
    static PENDING_EVENT_JSON: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
    static PENDING_EVENT_DROP_COUNT: Cell<u64> = const { Cell::new(0) };
    static PENDING_ENABLED: Cell<Option<bool>> = const { Cell::new(None) };
    static LATEST_DIAGNOSTICS: RefCell<Option<RemoteMotionPresentationDiagnostics>> =
        const { RefCell::new(None) };
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteMotionPresentationEntry {
    pub(crate) at_ms: f64,
    pub(crate) packet: String,
    pub(crate) object_id: String,
    pub(crate) from_x: f32,
    pub(crate) from_y: f32,
    pub(crate) to_x: f32,
    pub(crate) to_y: f32,
    pub(crate) direction: String,
    pub(crate) mode: String,
    pub(crate) phase_count: u8,
    pub(crate) started_ms: f64,
    pub(crate) started_pulse_id: u64,
    pub(crate) expires_ms: f64,
    pub(crate) phase_index: u8,
    pub(crate) next_phase_ms: f64,
    pub(crate) completed: bool,
}

impl RemoteMotionPresentationEntry {
    fn sync_to_clock(&mut self, clock: &motion::CrystalMoveClock) {
        let elapsed_pulses = clock.pulse_id().saturating_sub(self.started_pulse_id);
        self.next_phase_ms = clock.next_pulse_ms();
        if elapsed_pulses >= u64::from(self.phase_count) {
            self.completed = true;
            self.expires_ms = clock.now_ms();
            return;
        }

        self.phase_index = elapsed_pulses as u8;
        let last_phase_index = self.phase_count.saturating_sub(1);
        self.expires_ms = self.next_phase_ms
            + f64::from(last_phase_index.saturating_sub(self.phase_index)) * MOVE_PHASE_INTERVAL_MS;
    }

    fn is_active(&self) -> bool {
        !self.completed
    }

    fn current_pose(&self) -> Vec2 {
        let progress = (f32::from(self.phase_index) + 1.0) / f32::from(self.phase_count);
        let remaining = (1.0 - progress).clamp(0.0, 1.0);
        Vec2::new(
            self.to_x + (self.from_x - self.to_x) * remaining,
            self.to_y + (self.from_y - self.to_y) * remaining,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteMotionPresentationDiagnostics {
    pub(crate) enabled: bool,
    pub(crate) now_ms: f64,
    pub(crate) move_clock_pulse_id: u64,
    pub(crate) move_clock_next_pulse_ms: f64,
    pub(crate) entry_count: usize,
    pub(crate) active_entry_count: usize,
    pub(crate) processed_event_count: u64,
    pub(crate) remote_motion_event_count: u64,
    pub(crate) remote_remove_event_count: u64,
    pub(crate) ignored_event_count: u64,
    pub(crate) stale_event_count: u64,
    pub(crate) disabled_event_count: u64,
    pub(crate) pending_event_drop_count: u64,
    pub(crate) entry_eviction_count: u64,
    pub(crate) decode_error_count: u64,
    pub(crate) last_decode_error: Option<String>,
    pub(crate) offset_query_count: u64,
    pub(crate) offset_match_count: u64,
    pub(crate) target_mismatch_count: u64,
    pub(crate) inactive_fallback_count: u64,
    pub(crate) entries: Vec<RemoteMotionPresentationEntry>,
}

impl Default for RemoteMotionPresentationDiagnostics {
    fn default() -> Self {
        Self {
            enabled: false,
            now_ms: 0.0,
            move_clock_pulse_id: 0,
            move_clock_next_pulse_ms: MOVE_PHASE_INTERVAL_MS,
            entry_count: 0,
            active_entry_count: 0,
            processed_event_count: 0,
            remote_motion_event_count: 0,
            remote_remove_event_count: 0,
            ignored_event_count: 0,
            stale_event_count: 0,
            disabled_event_count: 0,
            pending_event_drop_count: 0,
            entry_eviction_count: 0,
            decode_error_count: 0,
            last_decode_error: None,
            offset_query_count: 0,
            offset_match_count: 0,
            target_mismatch_count: 0,
            inactive_fallback_count: 0,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Resource)]
pub(crate) struct RemoteMotionPresentation {
    enabled: bool,
    now_ms: f64,
    move_clock_pulse_id: u64,
    move_clock_next_pulse_ms: f64,
    entries: HashMap<String, RemoteMotionPresentationEntry>,
    processed_event_count: u64,
    remote_motion_event_count: u64,
    remote_remove_event_count: u64,
    ignored_event_count: u64,
    stale_event_count: u64,
    disabled_event_count: u64,
    pending_event_drop_count: u64,
    entry_eviction_count: u64,
    decode_error_count: u64,
    last_decode_error: Option<String>,
    offset_query_count: u64,
    offset_match_count: u64,
    target_mismatch_count: u64,
    inactive_fallback_count: u64,
}

impl RemoteMotionPresentation {
    fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled {
            return;
        }
        self.enabled = enabled;
        if !enabled {
            self.entries.clear();
        }
    }

    fn ingest(
        &mut self,
        pending_json: Vec<String>,
        dropped_events: u64,
        clock: &motion::CrystalMoveClock,
    ) {
        self.sync_to_clock(clock);
        self.pending_event_drop_count =
            self.pending_event_drop_count.saturating_add(dropped_events);

        for json in pending_json {
            match serde_json::from_str::<MovementShadowEvent>(&json) {
                Ok(event) => self.apply_event(event),
                Err(error) => self.record_decode_error(error.to_string()),
            }
        }
    }

    fn sync_to_clock(&mut self, clock: &motion::CrystalMoveClock) {
        self.now_ms = clock.now_ms();
        self.move_clock_pulse_id = clock.pulse_id();
        self.move_clock_next_pulse_ms = clock.next_pulse_ms();
        for entry in self.entries.values_mut() {
            entry.sync_to_clock(clock);
        }
    }

    fn apply_event(&mut self, event: MovementShadowEvent) {
        self.processed_event_count = self.processed_event_count.saturating_add(1);
        match event {
            MovementShadowEvent::Clear { .. } | MovementShadowEvent::Reset { .. } => {
                self.entries.clear();
            }
            MovementShadowEvent::RemoteMotion {
                at_ms,
                packet,
                object_id,
                from_x,
                from_y,
                to_x,
                to_y,
                direction,
                mode,
                phase_count,
            } => {
                self.remote_motion_event_count = self.remote_motion_event_count.saturating_add(1);
                if !self.enabled {
                    self.disabled_event_count = self.disabled_event_count.saturating_add(1);
                    return;
                }
                self.apply_remote_motion(
                    at_ms,
                    packet,
                    object_id,
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    direction,
                    mode,
                    normalized_move_phase_count(phase_count),
                );
            }
            MovementShadowEvent::RemoteRemove { at_ms, object_id } => {
                self.remote_remove_event_count = self.remote_remove_event_count.saturating_add(1);
                if self
                    .entries
                    .get(&object_id)
                    .is_some_and(|entry| entry.at_ms > at_ms)
                {
                    self.stale_event_count = self.stale_event_count.saturating_add(1);
                    return;
                }
                self.entries.remove(&object_id);
            }
            _ => {
                self.ignored_event_count = self.ignored_event_count.saturating_add(1);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_remote_motion(
        &mut self,
        at_ms: f64,
        packet: String,
        object_id: String,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
        direction: String,
        mode: String,
        phase_count: u8,
    ) {
        if self
            .entries
            .get(&object_id)
            .is_some_and(|entry| entry.at_ms > at_ms)
        {
            self.stale_event_count = self.stale_event_count.saturating_add(1);
            return;
        }

        let provided_from = Vec2::new(from_x as f32, from_y as f32);
        let target = Vec2::new(to_x as f32, to_y as f32);
        let mut effective_from = provided_from;

        if let Some(previous) = self.entries.get(&object_id) {
            let previous_target_matches_source =
                previous.to_x == provided_from.x && previous.to_y == provided_from.y;
            if previous_target_matches_source {
                let current = previous.current_pose();
                if chebyshev_distance(current, target) <= MAX_SMOOTH_TILE_DISTANCE {
                    effective_from = current;
                }
            }
        }

        let distance = chebyshev_distance(effective_from, target);
        let moving = mode != "turn" && distance > f32::EPSILON;
        if distance > MAX_SMOOTH_TILE_DISTANCE {
            effective_from = target;
        }
        let presenting = moving && effective_from != target;
        let next_pulse_ms = if self.move_clock_next_pulse_ms > self.now_ms {
            self.move_clock_next_pulse_ms
        } else {
            self.now_ms + MOVE_PHASE_INTERVAL_MS
        };

        if !self.entries.contains_key(&object_id) && self.entries.len() >= MAX_PRESENTATION_ENTRIES
        {
            if let Some(oldest_id) = self
                .entries
                .iter()
                .min_by(|left, right| left.1.at_ms.total_cmp(&right.1.at_ms))
                .map(|(id, _)| id.clone())
            {
                self.entries.remove(&oldest_id);
                self.entry_eviction_count = self.entry_eviction_count.saturating_add(1);
            }
        }

        self.entries.insert(
            object_id.clone(),
            RemoteMotionPresentationEntry {
                at_ms,
                packet,
                object_id,
                from_x: effective_from.x,
                from_y: effective_from.y,
                to_x: target.x,
                to_y: target.y,
                direction,
                mode,
                phase_count,
                started_ms: self.now_ms,
                started_pulse_id: self.move_clock_pulse_id,
                expires_ms: if presenting {
                    next_pulse_ms
                        + f64::from(phase_count.saturating_sub(1)) * MOVE_PHASE_INTERVAL_MS
                } else {
                    self.now_ms
                },
                phase_index: 0,
                next_phase_ms: next_pulse_ms,
                completed: !presenting,
            },
        );
    }

    /// Returns a screen-space offset only after the packed snapshot confirms
    /// that its base position represents this segment's authoritative target.
    pub(crate) fn presentation_offset(
        &mut self,
        object_id: &str,
        target_x: i32,
        target_y: i32,
        _now_ms: f64,
        cell_width: f32,
        cell_height: f32,
    ) -> Option<Vec2> {
        if !self.enabled {
            return None;
        }
        let Some(entry) = self.entries.get(object_id) else {
            return None;
        };

        self.offset_query_count = self.offset_query_count.saturating_add(1);
        if entry.to_x != target_x as f32 || entry.to_y != target_y as f32 {
            self.target_mismatch_count = self.target_mismatch_count.saturating_add(1);
            return None;
        }
        if !entry.is_active() {
            self.inactive_fallback_count = self.inactive_fallback_count.saturating_add(1);
            return None;
        }

        let offset = motion::compute_motion_offset_fractional_for_phase_count(
            entry.from_x,
            entry.from_y,
            entry.to_x,
            entry.to_y,
            entry.phase_index,
            entry.phase_count,
            cell_width,
            cell_height,
        );
        self.offset_match_count = self.offset_match_count.saturating_add(1);
        Some(offset)
    }

    fn diagnostics_snapshot(&self) -> RemoteMotionPresentationDiagnostics {
        let mut entries: Vec<_> = self.entries.values().cloned().collect();
        entries.sort_by(|left, right| left.object_id.cmp(&right.object_id));
        RemoteMotionPresentationDiagnostics {
            enabled: self.enabled,
            now_ms: self.now_ms,
            move_clock_pulse_id: self.move_clock_pulse_id,
            move_clock_next_pulse_ms: self.move_clock_next_pulse_ms,
            entry_count: entries.len(),
            active_entry_count: entries.iter().filter(|entry| entry.is_active()).count(),
            processed_event_count: self.processed_event_count,
            remote_motion_event_count: self.remote_motion_event_count,
            remote_remove_event_count: self.remote_remove_event_count,
            ignored_event_count: self.ignored_event_count,
            stale_event_count: self.stale_event_count,
            disabled_event_count: self.disabled_event_count,
            pending_event_drop_count: self.pending_event_drop_count,
            entry_eviction_count: self.entry_eviction_count,
            decode_error_count: self.decode_error_count,
            last_decode_error: self.last_decode_error.clone(),
            offset_query_count: self.offset_query_count,
            offset_match_count: self.offset_match_count,
            target_mismatch_count: self.target_mismatch_count,
            inactive_fallback_count: self.inactive_fallback_count,
            entries,
        }
    }

    fn record_decode_error(&mut self, error: String) {
        self.decode_error_count = self.decode_error_count.saturating_add(1);
        self.last_decode_error = Some(error.chars().take(MAX_DECODE_ERROR_CHARS).collect());
    }
}

fn chebyshev_distance(left: Vec2, right: Vec2) -> f32 {
    (left.x - right.x).abs().max((left.y - right.y).abs())
}

pub(crate) fn enqueue_remote_motion_event_json(json: String) {
    PENDING_EVENT_JSON.with(|pending| {
        let mut pending = pending.borrow_mut();
        if pending.len() >= MAX_PENDING_EVENT_JSON {
            pending.pop_front();
            PENDING_EVENT_DROP_COUNT.with(|count| count.set(count.get().saturating_add(1)));
        }
        pending.push_back(json);
    });
}

pub(crate) fn set_remote_motion_presentation_enabled(enabled: bool) {
    PENDING_ENABLED.with(|pending| pending.set(Some(enabled)));
}

pub(crate) fn get_remote_motion_presentation_diagnostics_json() -> String {
    let diagnostics = LATEST_DIAGNOSTICS
        .with(|latest| latest.borrow().clone())
        .unwrap_or_default();
    serde_json::to_string(&diagnostics).unwrap_or_else(|_| {
        r#"{"serializationError":"remote motion diagnostics were not finite"}"#.to_owned()
    })
}

fn drain_pending_event_json() -> Vec<String> {
    PENDING_EVENT_JSON.with(|pending| pending.borrow_mut().drain(..).collect())
}

fn take_pending_event_drop_count() -> u64 {
    PENDING_EVENT_DROP_COUNT.with(|count| count.replace(0))
}

fn take_pending_enabled() -> Option<bool> {
    PENDING_ENABLED.with(|pending| pending.replace(None))
}

#[derive(Default)]
struct RemoteMotionPresentationMainThread;

fn ingest_remote_motion_presentation_system(
    mut presentation: ResMut<RemoteMotionPresentation>,
    clock: Res<motion::CrystalMoveClock>,
    _main_thread: NonSend<RemoteMotionPresentationMainThread>,
) {
    if let Some(enabled) = take_pending_enabled() {
        presentation.set_enabled(enabled);
    }
    presentation.ingest(
        drain_pending_event_json(),
        take_pending_event_drop_count(),
        &clock,
    );
    let snapshot = presentation.diagnostics_snapshot();
    LATEST_DIAGNOSTICS.with(|latest| {
        *latest.borrow_mut() = Some(snapshot);
    });
}

pub(crate) struct RemoteMotionPresentationPlugin;

impl Plugin for RemoteMotionPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send(RemoteMotionPresentationMainThread)
            .init_resource::<RemoteMotionPresentation>()
            .add_systems(
                PreUpdate,
                ingest_remote_motion_presentation_system.after(motion::CrystalMoveClockSet),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sync_clock_at(
        presentation: &mut RemoteMotionPresentation,
        clock: &mut motion::CrystalMoveClock,
        now_ms: f64,
    ) {
        clock.tick_at(now_ms);
        presentation.sync_to_clock(clock);
    }

    fn remote_event(
        at_ms: f64,
        object_id: &str,
        from: (i32, i32),
        to: (i32, i32),
        mode: &str,
    ) -> MovementShadowEvent {
        MovementShadowEvent::RemoteMotion {
            at_ms,
            packet: if mode == "run" {
                "ObjectRun".to_owned()
            } else {
                "ObjectWalk".to_owned()
            },
            object_id: object_id.to_owned(),
            from_x: from.0,
            from_y: from.1,
            to_x: to.0,
            to_y: to.1,
            direction: "Right".to_owned(),
            mode: mode.to_owned(),
            phase_count: None,
        }
    }

    fn remote_event_with_phase_count(
        at_ms: f64,
        object_id: &str,
        from: (i32, i32),
        to: (i32, i32),
        mode: &str,
        value: u8,
    ) -> MovementShadowEvent {
        let mut event = remote_event(at_ms, object_id, from, to, mode);
        if let MovementShadowEvent::RemoteMotion { phase_count, .. } = &mut event {
            *phase_count = Some(value);
        }
        event
    }

    fn enabled_presentation() -> RemoteMotionPresentation {
        let mut presentation = RemoteMotionPresentation::default();
        presentation.set_enabled(true);
        presentation
    }

    fn clean_thread_locals() {
        let _ = drain_pending_event_json();
        PENDING_EVENT_DROP_COUNT.with(|count| count.set(0));
        PENDING_ENABLED.with(|enabled| enabled.set(None));
        LATEST_DIAGNOSTICS.with(|latest| *latest.borrow_mut() = None);
    }

    #[test]
    fn disabled_by_default_discards_remote_motion() {
        let mut presentation = RemoteMotionPresentation::default();
        presentation.apply_event(remote_event(100.0, "p2", (2, 3), (3, 3), "walk"));
        assert!(presentation.entries.is_empty());
        assert_eq!(presentation.disabled_event_count, 1);
    }

    #[test]
    fn enabled_walk_produces_crystal_stepped_offsets() {
        let mut presentation = enabled_presentation();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut presentation, &mut clock, 100.0);
        presentation.apply_event(remote_event(100.0, "p2", (2, 3), (3, 3), "walk"));
        let at_start = presentation
            .presentation_offset("p2", 3, 3, 100.0, 48.0, 32.0)
            .expect("offset");
        sync_clock_at(&mut presentation, &mut clock, 200.0);
        let at_200 = presentation
            .presentation_offset("p2", 3, 3, 200.0, 48.0, 32.0)
            .expect("offset");
        assert_eq!(at_start, Vec2::new(-40.0, 0.0));
        assert_eq!(at_200, Vec2::new(-32.0, 0.0));
        for at_ms in [300.0, 400.0, 500.0, 600.0, 700.0] {
            sync_clock_at(&mut presentation, &mut clock, at_ms);
        }
        assert!(presentation
            .presentation_offset("p2", 3, 3, 700.0, 48.0, 32.0)
            .is_none());
    }

    #[test]
    fn mounted_walk_remains_active_for_eight_scene_pulses() {
        let mut presentation = enabled_presentation();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut presentation, &mut clock, 0.0);
        presentation.apply_event(remote_event_with_phase_count(
            0.0,
            "mounted",
            (2, 3),
            (3, 3),
            "walk",
            8,
        ));
        let entry = presentation.entries.get("mounted").expect("entry");
        assert_eq!(entry.phase_count, 8);
        assert_eq!(entry.expires_ms, 800.0);

        for at_ms in [100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0] {
            sync_clock_at(&mut presentation, &mut clock, at_ms);
        }
        assert!(presentation
            .presentation_offset("mounted", 3, 3, 700.0, 48.0, 32.0)
            .is_some());
        assert_eq!(presentation.entries["mounted"].phase_index, 7);

        sync_clock_at(&mut presentation, &mut clock, 800.0);
        assert!(presentation
            .presentation_offset("mounted", 3, 3, 800.0, 48.0, 32.0)
            .is_none());
    }

    #[test]
    fn actors_started_between_pulses_advance_on_the_same_scene_tick() {
        let mut presentation = enabled_presentation();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut presentation, &mut clock, 0.0);
        presentation.apply_event(remote_event(0.0, "p2", (2, 3), (3, 3), "walk"));
        sync_clock_at(&mut presentation, &mut clock, 50.0);
        presentation.apply_event(remote_event(50.0, "p3", (4, 3), (5, 3), "walk"));

        assert_eq!(presentation.entries["p2"].phase_index, 0);
        assert_eq!(presentation.entries["p3"].phase_index, 0);
        sync_clock_at(&mut presentation, &mut clock, 100.0);
        assert_eq!(presentation.entries["p2"].phase_index, 1);
        assert_eq!(presentation.entries["p3"].phase_index, 1);
    }

    #[test]
    fn target_handshake_falls_back_until_packed_snapshot_catches_up() {
        let mut presentation = enabled_presentation();
        presentation.apply_event(remote_event(100.0, "p2", (2, 3), (3, 3), "walk"));
        assert!(presentation
            .presentation_offset("p2", 2, 3, 100.0, 48.0, 32.0)
            .is_none());
        assert_eq!(presentation.target_mismatch_count, 1);
    }

    #[test]
    fn consecutive_segments_start_from_previous_fractional_pose() {
        let mut presentation = enabled_presentation();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut presentation, &mut clock, 0.0);
        presentation.apply_event(remote_event(0.0, "p2", (0, 0), (1, 0), "walk"));
        for at_ms in [100.0, 200.0, 300.0] {
            sync_clock_at(&mut presentation, &mut clock, at_ms);
        }
        presentation.apply_event(remote_event(300.0, "p2", (1, 0), (2, 0), "walk"));
        let entry = presentation.entries.get("p2").expect("entry");
        assert!((entry.from_x - (2.0 / 3.0)).abs() < f32::EPSILON);
        let offset = presentation
            .presentation_offset("p2", 2, 0, 300.0, 48.0, 32.0)
            .expect("offset");
        assert_eq!(offset.x, -54.0);
    }

    #[test]
    fn disconnected_segment_uses_packet_source_instead_of_old_pose() {
        let mut presentation = enabled_presentation();
        presentation.apply_event(remote_event(0.0, "p2", (0, 0), (1, 0), "walk"));
        presentation.apply_event(remote_event(100.0, "p2", (5, 5), (6, 5), "walk"));
        let entry = presentation.entries.get("p2").expect("entry");
        assert_eq!((entry.from_x, entry.from_y), (5.0, 5.0));
    }

    #[test]
    fn turn_segment_stops_motion_and_uses_fallback() {
        let mut presentation = enabled_presentation();
        presentation.apply_event(remote_event(0.0, "p2", (2, 3), (2, 3), "turn"));
        assert!(presentation
            .presentation_offset("p2", 2, 3, 0.0, 48.0, 32.0)
            .is_none());
    }

    #[test]
    fn stale_motion_and_remove_cannot_replace_newer_segment() {
        let mut presentation = enabled_presentation();
        presentation.apply_event(remote_event(200.0, "p2", (2, 3), (3, 3), "walk"));
        presentation.apply_event(remote_event(100.0, "p2", (1, 3), (2, 3), "walk"));
        presentation.apply_event(MovementShadowEvent::RemoteRemove {
            at_ms: 150.0,
            object_id: "p2".to_owned(),
        });
        assert_eq!(presentation.entries.get("p2").expect("entry").at_ms, 200.0);
        assert_eq!(presentation.stale_event_count, 2);
    }

    #[test]
    fn current_remove_clear_and_reset_evict_entries() {
        let mut presentation = enabled_presentation();
        presentation.apply_event(remote_event(100.0, "p2", (2, 3), (3, 3), "walk"));
        presentation.apply_event(MovementShadowEvent::RemoteRemove {
            at_ms: 100.0,
            object_id: "p2".to_owned(),
        });
        assert!(presentation.entries.is_empty());
        presentation.apply_event(remote_event(200.0, "p3", (2, 3), (3, 3), "walk"));
        presentation.apply_event(MovementShadowEvent::Clear { at_ms: 201.0 });
        assert!(presentation.entries.is_empty());
        presentation.apply_event(remote_event(300.0, "p4", (2, 3), (3, 3), "walk"));
        presentation.apply_event(MovementShadowEvent::Reset {
            at_ms: 301.0,
            object_id: "self".to_owned(),
            x: 1,
            y: 1,
            direction: "Down".to_owned(),
        });
        assert!(presentation.entries.is_empty());
    }

    #[test]
    fn disabling_clears_existing_entries() {
        let mut presentation = enabled_presentation();
        presentation.apply_event(remote_event(100.0, "p2", (2, 3), (3, 3), "walk"));
        presentation.set_enabled(false);
        assert!(presentation.entries.is_empty());
    }

    #[test]
    fn presentation_registry_is_bounded_and_evicts_oldest() {
        let mut presentation = enabled_presentation();
        for index in 0..MAX_PRESENTATION_ENTRIES {
            presentation.apply_event(remote_event(
                index as f64,
                &format!("remote-{index}"),
                (0, 0),
                (1, 0),
                "walk",
            ));
        }
        presentation.apply_event(remote_event(9999.0, "remote-new", (0, 0), (1, 0), "walk"));
        assert_eq!(presentation.entries.len(), MAX_PRESENTATION_ENTRIES);
        assert!(!presentation.entries.contains_key("remote-0"));
        assert!(presentation.entries.contains_key("remote-new"));
        assert_eq!(presentation.entry_eviction_count, 1);
    }

    #[test]
    fn pending_json_queue_is_bounded() {
        clean_thread_locals();
        for index in 0..=MAX_PENDING_EVENT_JSON {
            enqueue_remote_motion_event_json(format!(r#"{{"type":"clear","atMs":{index}}}"#));
        }
        assert_eq!(drain_pending_event_json().len(), MAX_PENDING_EVENT_JSON);
        assert_eq!(take_pending_event_drop_count(), 1);
    }

    #[test]
    fn malformed_json_is_bounded_and_does_not_stop_next_event() {
        let mut presentation = enabled_presentation();
        let mut clock = motion::CrystalMoveClock::default();
        clock.tick_at(100.0);
        presentation.ingest(
            vec![
                "not-json".to_owned(),
                serde_json::to_string(&remote_event(100.0, "p2", (2, 3), (3, 3), "walk"))
                    .expect("json"),
            ],
            0,
            &clock,
        );
        assert_eq!(presentation.decode_error_count, 1);
        assert!(presentation.entries.contains_key("p2"));
        assert!(
            presentation
                .last_decode_error
                .as_ref()
                .expect("error")
                .len()
                <= 512
        );
    }

    #[test]
    fn plugin_consumes_config_and_events_on_pre_update() {
        clean_thread_locals();
        let mut app = App::new();
        app.add_plugins((
            motion::CrystalMoveClockPlugin,
            RemoteMotionPresentationPlugin,
        ));
        set_remote_motion_presentation_enabled(true);
        enqueue_remote_motion_event_json(
            serde_json::to_string(&remote_event(100.0, "p2", (2, 3), (3, 3), "walk"))
                .expect("json"),
        );
        app.world_mut().run_schedule(PreUpdate);
        let presentation = app.world().resource::<RemoteMotionPresentation>();
        assert!(presentation.enabled);
        assert!(presentation.entries.contains_key("p2"));
        let json = get_remote_motion_presentation_diagnostics_json();
        let value: serde_json::Value = serde_json::from_str(&json).expect("diagnostics json");
        assert_eq!(value["enabled"], true);
        assert_eq!(value["entryCount"], 1);
    }
}
