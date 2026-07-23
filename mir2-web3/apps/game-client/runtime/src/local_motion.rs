//! Guarded local movement presentation shadow and takeover source.
//!
//! This resource predicts the self-player presentation segment from copies of
//! commands and ACKs, then compares it with the TypeScript pose actually used by
//! the renderer. A rollback-gated path may select its pose for rendering, but it
//! never mutates authoritative movement state or sends a gameplay command.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use bevy::prelude::*;
use serde::Serialize;

use crate::motion;
use crate::movement_shadow::{normalized_move_phase_count, MovementShadowEvent};

const MAX_PENDING_EVENT_JSON: usize = 256;
const MAX_PENDING_COMMANDS: usize = 16;
const MAX_DECODE_ERROR_CHARS: usize = 512;
const MOVE_PHASE_INTERVAL_MS: f64 = 100.0;
const MAX_SMOOTH_TILE_DISTANCE: f32 = 3.0;
const PIXEL_MATCH_EPSILON: f32 = 0.01;
const PATH_MATCH_EPSILON_TILES: f32 = 0.001;

thread_local! {
    static PENDING_EVENT_JSON: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
    static PENDING_EVENT_DROP_COUNT: Cell<u64> = const { Cell::new(0) };
    static PENDING_PRESENTATION_ENABLED: Cell<Option<bool>> = const { Cell::new(None) };
    static LATEST_DIAGNOSTICS: RefCell<Option<LocalMotionDiagnostics>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalMotionCommand {
    pub(crate) at_ms: f64,
    pub(crate) direction: String,
    pub(crate) mode: String,
    pub(crate) from_x: i32,
    pub(crate) from_y: i32,
    pub(crate) to_x: i32,
    pub(crate) to_y: i32,
    pub(crate) phase_count: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalMotionSegment {
    pub(crate) command_at_ms: f64,
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
    pub(crate) authoritative_confirmed: bool,
    pub(crate) presentation_committed: bool,
    pub(crate) completed: bool,
}

impl LocalMotionSegment {
    fn sync_to_clock(&mut self, clock: &motion::CrystalMoveClock) {
        if self.completed || clock.now_ms() < self.next_phase_ms {
            return;
        }

        let last_phase_index = self.phase_count.saturating_sub(1);
        if self.phase_index >= last_phase_index {
            self.completed = true;
            self.expires_ms = clock.now_ms();
            return;
        }

        // Crystal advances at most one phase per display iteration. Anchor the
        // next pulse to this action, rather than the nearest global pulse, so a
        // command arriving just before that pulse still receives a full 100 ms
        // phase zero instead of a timing-dependent shortened first frame.
        self.phase_index = self.phase_index.saturating_add(1);
        self.next_phase_ms = clock.now_ms() + MOVE_PHASE_INTERVAL_MS;
        self.expires_ms = self.next_phase_ms
            + f64::from(last_phase_index.saturating_sub(self.phase_index)) * MOVE_PHASE_INTERVAL_MS;
    }

    fn is_presenting(&self) -> bool {
        !self.completed
    }

    fn remaining_ratio(&self) -> f32 {
        let progress = (f32::from(self.phase_index) + 1.0) / f32::from(self.phase_count);
        (1.0 - progress).clamp(0.0, 1.0)
    }

    fn current_pose(&self) -> Vec2 {
        let remaining = self.remaining_ratio();
        Vec2::new(
            self.to_x + (self.from_x - self.to_x) * remaining,
            self.to_y + (self.from_y - self.to_y) * remaining,
        )
    }

    fn current_offset(&self, cell_width: f32, cell_height: f32) -> Vec2 {
        motion::compute_motion_offset_fractional_for_phase_count(
            self.from_x,
            self.from_y,
            self.to_x,
            self.to_y,
            self.phase_index,
            self.phase_count,
            cell_width,
            cell_height,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalMotionPresentationPhase {
    pub(crate) frame_index: u8,
    pub(crate) phase_count: u8,
    pub(crate) mode: String,
    pub(crate) direction: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalMotionComparison {
    pub(crate) at_ms: f64,
    pub(crate) object_id: String,
    pub(crate) candidate_x: f32,
    pub(crate) candidate_y: f32,
    pub(crate) actual_x: f32,
    pub(crate) actual_y: f32,
    pub(crate) delta_x: f32,
    pub(crate) delta_y: f32,
    pub(crate) matches: bool,
    pub(crate) command_candidate_x: f32,
    pub(crate) command_candidate_y: f32,
    pub(crate) command_delta_x: f32,
    pub(crate) command_delta_y: f32,
    pub(crate) start_delta_ms: f64,
    pub(crate) local_started_ms: Option<f64>,
    pub(crate) local_expires_ms: Option<f64>,
    pub(crate) ts_window: LocalTsMotionWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalTsMotionWindow {
    pub(crate) from_x: f32,
    pub(crate) from_y: f32,
    pub(crate) to_x: f32,
    pub(crate) to_y: f32,
    pub(crate) started_ms: f64,
    pub(crate) expires_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalMotionDiagnostics {
    pub(crate) now_ms: f64,
    pub(crate) move_clock_pulse_id: u64,
    pub(crate) move_clock_next_pulse_ms: f64,
    pub(crate) presentation_enabled: bool,
    pub(crate) self_object_id: Option<String>,
    pub(crate) processed_event_count: u64,
    pub(crate) command_event_count: u64,
    pub(crate) authoritative_event_count: u64,
    pub(crate) ignored_event_count: u64,
    pub(crate) stale_event_count: u64,
    pub(crate) pending_event_drop_count: u64,
    pub(crate) pending_command_count: usize,
    pub(crate) pending_command_drop_count: u64,
    pub(crate) decode_error_count: u64,
    pub(crate) last_decode_error: Option<String>,
    pub(crate) candidate_query_count: u64,
    pub(crate) candidate_match_count: u64,
    pub(crate) target_mismatch_count: u64,
    pub(crate) inactive_candidate_count: u64,
    pub(crate) ts_window_target_mismatch_count: u64,
    pub(crate) ts_window_path_mismatch_count: u64,
    pub(crate) comparison_sample_count: u64,
    pub(crate) comparison_match_count: u64,
    pub(crate) comparison_mismatch_count: u64,
    pub(crate) command_phase_mismatch_count: u64,
    pub(crate) max_abs_delta_x: f32,
    pub(crate) max_abs_delta_y: f32,
    pub(crate) max_command_phase_abs_delta_x: f32,
    pub(crate) max_command_phase_abs_delta_y: f32,
    pub(crate) max_abs_start_delta_ms: f64,
    pub(crate) segment: Option<LocalMotionSegment>,
    pub(crate) last_comparison: Option<LocalMotionComparison>,
    pub(crate) first_mismatch: Option<LocalMotionComparison>,
    pub(crate) last_mismatch: Option<LocalMotionComparison>,
}

impl Default for LocalMotionDiagnostics {
    fn default() -> Self {
        Self {
            now_ms: 0.0,
            move_clock_pulse_id: 0,
            move_clock_next_pulse_ms: MOVE_PHASE_INTERVAL_MS,
            presentation_enabled: false,
            self_object_id: None,
            processed_event_count: 0,
            command_event_count: 0,
            authoritative_event_count: 0,
            ignored_event_count: 0,
            stale_event_count: 0,
            pending_event_drop_count: 0,
            pending_command_count: 0,
            pending_command_drop_count: 0,
            decode_error_count: 0,
            last_decode_error: None,
            candidate_query_count: 0,
            candidate_match_count: 0,
            target_mismatch_count: 0,
            inactive_candidate_count: 0,
            ts_window_target_mismatch_count: 0,
            ts_window_path_mismatch_count: 0,
            comparison_sample_count: 0,
            comparison_match_count: 0,
            comparison_mismatch_count: 0,
            command_phase_mismatch_count: 0,
            max_abs_delta_x: 0.0,
            max_abs_delta_y: 0.0,
            max_command_phase_abs_delta_x: 0.0,
            max_command_phase_abs_delta_y: 0.0,
            max_abs_start_delta_ms: 0.0,
            segment: None,
            last_comparison: None,
            first_mismatch: None,
            last_mismatch: None,
        }
    }
}

#[derive(Debug, Default, Resource)]
pub(crate) struct LocalMotionPresentationShadow {
    now_ms: f64,
    move_clock_pulse_id: u64,
    move_clock_next_pulse_ms: f64,
    presentation_enabled: bool,
    self_object_id: Option<String>,
    segment: Option<LocalMotionSegment>,
    pending_commands: VecDeque<LocalMotionCommand>,
    processed_event_count: u64,
    command_event_count: u64,
    authoritative_event_count: u64,
    ignored_event_count: u64,
    stale_event_count: u64,
    pending_event_drop_count: u64,
    pending_command_drop_count: u64,
    decode_error_count: u64,
    last_decode_error: Option<String>,
    candidate_query_count: u64,
    candidate_match_count: u64,
    target_mismatch_count: u64,
    inactive_candidate_count: u64,
    ts_window_target_mismatch_count: u64,
    ts_window_path_mismatch_count: u64,
    comparison_sample_count: u64,
    comparison_match_count: u64,
    comparison_mismatch_count: u64,
    command_phase_mismatch_count: u64,
    max_abs_delta_x: f32,
    max_abs_delta_y: f32,
    max_command_phase_abs_delta_x: f32,
    max_command_phase_abs_delta_y: f32,
    max_abs_start_delta_ms: f64,
    last_comparison: Option<LocalMotionComparison>,
    first_mismatch: Option<LocalMotionComparison>,
    last_mismatch: Option<LocalMotionComparison>,
}

impl LocalMotionPresentationShadow {
    fn set_presentation_enabled(&mut self, enabled: bool) {
        self.presentation_enabled = enabled;
    }

    pub(crate) fn presentation_enabled(&self) -> bool {
        self.presentation_enabled
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
        if let Some(segment) = &mut self.segment {
            segment.sync_to_clock(clock);
        }
    }

    fn apply_event(&mut self, event: MovementShadowEvent) {
        self.processed_event_count = self.processed_event_count.saturating_add(1);
        match event {
            MovementShadowEvent::Clear { .. } => self.clear_state(),
            MovementShadowEvent::Reset {
                object_id, x, y, ..
            } => self.reset(object_id, x, y),
            MovementShadowEvent::CommandSent {
                at_ms,
                direction,
                mode,
                from_x,
                from_y,
                to_x,
                to_y,
                phase_count,
            } => self.observe_command(LocalMotionCommand {
                at_ms,
                direction,
                mode,
                from_x,
                from_y,
                to_x,
                to_y,
                phase_count: normalized_move_phase_count(phase_count),
            }),
            MovementShadowEvent::Authoritative {
                at_ms,
                object_id,
                is_self,
                x,
                y,
                ts_disposition,
                ..
            } if is_self => {
                self.observe_authoritative(at_ms, object_id, x, y, ts_disposition.as_deref())
            }
            _ => {
                self.ignored_event_count = self.ignored_event_count.saturating_add(1);
            }
        }
    }

    fn clear_state(&mut self) {
        self.self_object_id = None;
        self.segment = None;
        self.pending_commands.clear();
    }

    fn reset(&mut self, object_id: String, _x: i32, _y: i32) {
        self.self_object_id = Some(object_id);
        self.segment = None;
        self.pending_commands.clear();
        self.last_comparison = None;
    }

    fn observe_command(&mut self, command: LocalMotionCommand) {
        self.command_event_count = self.command_event_count.saturating_add(1);
        if self
            .segment
            .as_ref()
            .is_some_and(|segment| segment.command_at_ms > command.at_ms)
        {
            self.stale_event_count = self.stale_event_count.saturating_add(1);
            return;
        }

        if self.pending_commands.len() >= MAX_PENDING_COMMANDS {
            self.pending_commands.pop_front();
            self.pending_command_drop_count = self.pending_command_drop_count.saturating_add(1);
        }
        self.pending_commands.push_back(command.clone());

        let target = Vec2::new(command.to_x as f32, command.to_y as f32);
        let provided_from = Vec2::new(command.from_x as f32, command.from_y as f32);
        let mut effective_from = provided_from;
        let mut presentation_committed = false;
        if let Some(previous) = &self.segment {
            if previous.to_x == provided_from.x && previous.to_y == provided_from.y {
                presentation_committed = previous.presentation_committed;
                let current = previous.current_pose();
                if chebyshev_distance(current, target) <= MAX_SMOOTH_TILE_DISTANCE {
                    effective_from = current;
                }
            }
        }

        let moving = command.mode != "turn" && effective_from != target;
        let phase_count = command.phase_count;
        let next_pulse_ms = self.now_ms + MOVE_PHASE_INTERVAL_MS;
        self.segment = moving.then(|| LocalMotionSegment {
            command_at_ms: command.at_ms,
            from_x: effective_from.x,
            from_y: effective_from.y,
            to_x: target.x,
            to_y: target.y,
            direction: command.direction,
            mode: command.mode,
            phase_count,
            started_ms: self.now_ms,
            started_pulse_id: self.move_clock_pulse_id,
            expires_ms: next_pulse_ms
                + f64::from(phase_count.saturating_sub(1)) * MOVE_PHASE_INTERVAL_MS,
            phase_index: 0,
            next_phase_ms: next_pulse_ms,
            authoritative_confirmed: false,
            presentation_committed,
            completed: false,
        });
    }

    fn observe_authoritative(
        &mut self,
        at_ms: f64,
        object_id: String,
        x: i32,
        y: i32,
        ts_disposition: Option<&str>,
    ) {
        self.authoritative_event_count = self.authoritative_event_count.saturating_add(1);
        if self.self_object_id.is_none() {
            self.self_object_id = Some(object_id);
        }
        let command = self.pending_commands.pop_front();
        let matching_target = command
            .as_ref()
            .is_some_and(|pending| pending.to_x == x && pending.to_y == y);
        let confirmed = ts_disposition.is_some_and(|value| {
            matches!(value.to_ascii_lowercase().as_str(), "confirmed" | "match")
        });

        // A clean ACK keeps the original display window alive. Any correction,
        // stale echo, or degraded landing remains TS-owned in the guarded path.
        if !confirmed || !matching_target {
            self.segment = None;
        } else if let Some(segment) = &mut self.segment {
            if segment.command_at_ms > at_ms {
                self.stale_event_count = self.stale_event_count.saturating_add(1);
            } else {
                segment.authoritative_confirmed = true;
            }
        }
    }

    pub(crate) fn candidate_offset(
        &mut self,
        object_id: &str,
        target_x: i32,
        target_y: i32,
        _now_ms: f64,
        cell_width: f32,
        cell_height: f32,
    ) -> Option<Vec2> {
        if self
            .self_object_id
            .as_deref()
            .is_some_and(|id| id != object_id)
        {
            return None;
        }
        let segment = self.segment.as_mut()?;
        self.candidate_query_count = self.candidate_query_count.saturating_add(1);
        if !coordinates_match(segment.to_x, target_x as f32)
            || !coordinates_match(segment.to_y, target_y as f32)
        {
            self.target_mismatch_count = self.target_mismatch_count.saturating_add(1);
            return None;
        }
        if !segment.is_presenting() {
            self.inactive_candidate_count = self.inactive_candidate_count.saturating_add(1);
            return None;
        }
        self.candidate_match_count = self.candidate_match_count.saturating_add(1);
        Some(segment.current_offset(cell_width, cell_height))
    }

    pub(crate) fn has_matching_segment_target(
        &self,
        object_id: &str,
        target_x: i32,
        target_y: i32,
    ) -> bool {
        if self
            .self_object_id
            .as_deref()
            .is_some_and(|id| id != object_id)
        {
            return false;
        }
        self.segment.as_ref().is_some_and(|segment| {
            coordinates_match(segment.to_x, target_x as f32)
                && coordinates_match(segment.to_y, target_y as f32)
        })
    }

    pub(crate) fn segment_matches_ts_window(&self, window: LocalTsMotionWindow) -> bool {
        self.segment.as_ref().is_some_and(|segment| {
            coordinates_match(segment.from_x, window.from_x)
                && coordinates_match(segment.from_y, window.from_y)
                && coordinates_match(segment.to_x, window.to_x)
                && coordinates_match(segment.to_y, window.to_y)
        })
    }

    pub(crate) fn committed_segment_matches_ts_target(&self, window: LocalTsMotionWindow) -> bool {
        self.segment.as_ref().is_some_and(|segment| {
            segment.presentation_committed
                && coordinates_match(segment.to_x, window.to_x)
                && coordinates_match(segment.to_y, window.to_y)
        })
    }

    pub(crate) fn mark_presentation_committed(&mut self) {
        if let Some(segment) = &mut self.segment {
            if segment.is_presenting() {
                segment.presentation_committed = true;
            }
        }
    }

    pub(crate) fn presentation_phase(&self) -> Option<LocalMotionPresentationPhase> {
        self.segment.as_ref().and_then(|segment| {
            segment
                .is_presenting()
                .then(|| LocalMotionPresentationPhase {
                    frame_index: segment.phase_index,
                    phase_count: segment.phase_count,
                    mode: segment.mode.clone(),
                    direction: segment.direction.clone(),
                })
        })
    }

    pub(crate) fn candidate_offset_for_applied_center(
        &mut self,
        object_id: &str,
        center_x: i32,
        center_y: i32,
        _now_ms: f64,
        cell_width: f32,
        cell_height: f32,
    ) -> Option<Vec2> {
        if self
            .self_object_id
            .as_deref()
            .is_some_and(|id| id != object_id)
        {
            return None;
        }
        let segment = self.segment.as_mut()?;
        self.candidate_query_count = self.candidate_query_count.saturating_add(1);
        if !segment.is_presenting() {
            self.inactive_candidate_count = self.inactive_candidate_count.saturating_add(1);
            return None;
        }

        let center = Vec2::new(center_x as f32, center_y as f32);
        let from = Vec2::new(segment.from_x, segment.from_y);
        let target = Vec2::new(segment.to_x, segment.to_y);
        let inside_path_bounds = center.x >= from.x.min(target.x).floor()
            && center.x <= from.x.max(target.x).ceil()
            && center.y >= from.y.min(target.y).floor()
            && center.y <= from.y.max(target.y).ceil();
        if !inside_path_bounds || chebyshev_distance(center, target) > MAX_SMOOTH_TILE_DISTANCE {
            self.target_mismatch_count = self.target_mismatch_count.saturating_add(1);
            return None;
        }

        self.candidate_match_count = self.candidate_match_count.saturating_add(1);
        let target_relative = segment.current_offset(cell_width, cell_height);
        Some(
            target_relative
                + Vec2::new(
                    (segment.to_x - center.x) * cell_width,
                    (segment.to_y - center.y) * cell_height,
                ),
        )
    }

    pub(crate) fn compare_with_actual(
        &mut self,
        at_ms: f64,
        object_id: &str,
        candidate: Vec2,
        actual: Vec2,
        ts_window: LocalTsMotionWindow,
    ) {
        if !candidate.is_finite() || !actual.is_finite() {
            return;
        }
        let Some(segment) = self.segment.as_ref() else {
            return;
        };
        let phase_aligned_candidate = motion::compute_motion_offset_fractional_with_phase_count(
            segment.from_x,
            segment.from_y,
            segment.to_x,
            segment.to_y,
            ts_window.started_ms,
            ts_window.expires_ms,
            at_ms,
            segment.phase_count,
            48.0,
            32.0,
        );
        let command_delta = candidate - actual;
        let delta = phase_aligned_candidate - actual;
        let matches = delta.x.abs() <= PIXEL_MATCH_EPSILON && delta.y.abs() <= PIXEL_MATCH_EPSILON;
        if command_delta.x.abs() > PIXEL_MATCH_EPSILON
            || command_delta.y.abs() > PIXEL_MATCH_EPSILON
        {
            self.command_phase_mismatch_count = self.command_phase_mismatch_count.saturating_add(1);
        }
        self.comparison_sample_count = self.comparison_sample_count.saturating_add(1);
        if matches {
            self.comparison_match_count = self.comparison_match_count.saturating_add(1);
        } else {
            self.comparison_mismatch_count = self.comparison_mismatch_count.saturating_add(1);
        }
        self.max_abs_delta_x = self.max_abs_delta_x.max(delta.x.abs());
        self.max_abs_delta_y = self.max_abs_delta_y.max(delta.y.abs());
        self.max_command_phase_abs_delta_x = self
            .max_command_phase_abs_delta_x
            .max(command_delta.x.abs());
        self.max_command_phase_abs_delta_y = self
            .max_command_phase_abs_delta_y
            .max(command_delta.y.abs());
        let start_delta_ms = segment.started_ms - ts_window.started_ms;
        self.max_abs_start_delta_ms = self.max_abs_start_delta_ms.max(start_delta_ms.abs());
        let comparison = LocalMotionComparison {
            at_ms,
            object_id: object_id.to_owned(),
            candidate_x: phase_aligned_candidate.x,
            candidate_y: phase_aligned_candidate.y,
            actual_x: actual.x,
            actual_y: actual.y,
            delta_x: delta.x,
            delta_y: delta.y,
            matches,
            command_candidate_x: candidate.x,
            command_candidate_y: candidate.y,
            command_delta_x: command_delta.x,
            command_delta_y: command_delta.y,
            start_delta_ms,
            local_started_ms: self.segment.as_ref().map(|segment| segment.started_ms),
            local_expires_ms: self.segment.as_ref().map(|segment| segment.expires_ms),
            ts_window,
        };
        if !matches {
            if self.first_mismatch.is_none() {
                self.first_mismatch = Some(comparison.clone());
            }
            self.last_mismatch = Some(comparison.clone());
        }
        self.last_comparison = Some(comparison);
    }

    pub(crate) fn record_ts_window_target_mismatch(&mut self) {
        self.ts_window_target_mismatch_count =
            self.ts_window_target_mismatch_count.saturating_add(1);
    }

    pub(crate) fn record_ts_window_path_mismatch(&mut self) {
        self.ts_window_path_mismatch_count = self.ts_window_path_mismatch_count.saturating_add(1);
    }

    fn diagnostics_snapshot(&self) -> LocalMotionDiagnostics {
        LocalMotionDiagnostics {
            now_ms: self.now_ms,
            move_clock_pulse_id: self.move_clock_pulse_id,
            move_clock_next_pulse_ms: self.move_clock_next_pulse_ms,
            presentation_enabled: self.presentation_enabled,
            self_object_id: self.self_object_id.clone(),
            processed_event_count: self.processed_event_count,
            command_event_count: self.command_event_count,
            authoritative_event_count: self.authoritative_event_count,
            ignored_event_count: self.ignored_event_count,
            stale_event_count: self.stale_event_count,
            pending_event_drop_count: self.pending_event_drop_count,
            pending_command_count: self.pending_commands.len(),
            pending_command_drop_count: self.pending_command_drop_count,
            decode_error_count: self.decode_error_count,
            last_decode_error: self.last_decode_error.clone(),
            candidate_query_count: self.candidate_query_count,
            candidate_match_count: self.candidate_match_count,
            target_mismatch_count: self.target_mismatch_count,
            inactive_candidate_count: self.inactive_candidate_count,
            ts_window_target_mismatch_count: self.ts_window_target_mismatch_count,
            ts_window_path_mismatch_count: self.ts_window_path_mismatch_count,
            comparison_sample_count: self.comparison_sample_count,
            comparison_match_count: self.comparison_match_count,
            comparison_mismatch_count: self.comparison_mismatch_count,
            command_phase_mismatch_count: self.command_phase_mismatch_count,
            max_abs_delta_x: self.max_abs_delta_x,
            max_abs_delta_y: self.max_abs_delta_y,
            max_command_phase_abs_delta_x: self.max_command_phase_abs_delta_x,
            max_command_phase_abs_delta_y: self.max_command_phase_abs_delta_y,
            max_abs_start_delta_ms: self.max_abs_start_delta_ms,
            segment: self.segment.clone(),
            last_comparison: self.last_comparison.clone(),
            first_mismatch: self.first_mismatch.clone(),
            last_mismatch: self.last_mismatch.clone(),
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

fn coordinates_match(left: f32, right: f32) -> bool {
    (left - right).abs() <= PATH_MATCH_EPSILON_TILES
}

pub(crate) fn enqueue_local_motion_event_json(json: String) {
    PENDING_EVENT_JSON.with(|pending| {
        let mut pending = pending.borrow_mut();
        if pending.len() >= MAX_PENDING_EVENT_JSON {
            pending.pop_front();
            PENDING_EVENT_DROP_COUNT.with(|count| count.set(count.get().saturating_add(1)));
        }
        pending.push_back(json);
    });
}

pub(crate) fn get_local_motion_diagnostics_json() -> String {
    let diagnostics = LATEST_DIAGNOSTICS
        .with(|latest| latest.borrow().clone())
        .unwrap_or_default();
    serde_json::to_string(&diagnostics).unwrap_or_else(|_| {
        r#"{"serializationError":"local motion diagnostics were not finite"}"#.to_owned()
    })
}

pub(crate) fn set_local_motion_presentation_enabled(enabled: bool) {
    PENDING_PRESENTATION_ENABLED.with(|pending| pending.set(Some(enabled)));
}

fn take_pending_presentation_enabled() -> Option<bool> {
    PENDING_PRESENTATION_ENABLED.with(|pending| pending.replace(None))
}

fn drain_pending_event_json() -> Vec<String> {
    PENDING_EVENT_JSON.with(|pending| pending.borrow_mut().drain(..).collect())
}

fn take_pending_event_drop_count() -> u64 {
    PENDING_EVENT_DROP_COUNT.with(|count| count.replace(0))
}

#[derive(Default)]
struct LocalMotionMainThread;

fn ingest_local_motion_system(
    mut shadow: ResMut<LocalMotionPresentationShadow>,
    clock: Res<motion::CrystalMoveClock>,
    _main_thread: NonSend<LocalMotionMainThread>,
) {
    if let Some(enabled) = take_pending_presentation_enabled() {
        shadow.set_presentation_enabled(enabled);
    }
    shadow.ingest(
        drain_pending_event_json(),
        take_pending_event_drop_count(),
        &clock,
    );
}

fn publish_local_motion_diagnostics_system(
    shadow: Res<LocalMotionPresentationShadow>,
    _main_thread: NonSend<LocalMotionMainThread>,
) {
    let snapshot = shadow.diagnostics_snapshot();
    LATEST_DIAGNOSTICS.with(|latest| {
        *latest.borrow_mut() = Some(snapshot);
    });
}

pub(crate) struct LocalMotionPresentationShadowPlugin;

impl Plugin for LocalMotionPresentationShadowPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send_resource(LocalMotionMainThread)
            .init_resource::<LocalMotionPresentationShadow>()
            .add_systems(
                PreUpdate,
                ingest_local_motion_system.after(motion::CrystalMoveClockSet),
            )
            .add_systems(PostUpdate, publish_local_motion_diagnostics_system);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sync_clock_at(
        shadow: &mut LocalMotionPresentationShadow,
        clock: &mut motion::CrystalMoveClock,
        now_ms: f64,
    ) {
        clock.tick_at(now_ms);
        shadow.sync_to_clock(clock);
    }

    fn reset_event() -> MovementShadowEvent {
        MovementShadowEvent::Reset {
            at_ms: 0.0,
            object_id: "self".to_owned(),
            x: 10,
            y: 10,
            direction: "Right".to_owned(),
        }
    }

    fn walk_command(at_ms: f64) -> MovementShadowEvent {
        MovementShadowEvent::CommandSent {
            at_ms,
            direction: "Right".to_owned(),
            mode: "walk".to_owned(),
            from_x: 10,
            from_y: 10,
            to_x: 11,
            to_y: 10,
            phase_count: None,
        }
    }

    fn mounted_walk_command(at_ms: f64) -> MovementShadowEvent {
        MovementShadowEvent::CommandSent {
            at_ms,
            direction: "Right".to_owned(),
            mode: "walk".to_owned(),
            from_x: 10,
            from_y: 10,
            to_x: 11,
            to_y: 10,
            phase_count: Some(8),
        }
    }

    fn ack(at_ms: f64, x: i32, disposition: &str) -> MovementShadowEvent {
        MovementShadowEvent::Authoritative {
            at_ms,
            packet: "UserLocation".to_owned(),
            object_id: "self".to_owned(),
            is_self: true,
            x,
            y: 10,
            direction: "Right".to_owned(),
            ts_predicted_x: Some(11),
            ts_predicted_y: Some(10),
            ts_disposition: Some(disposition.to_owned()),
        }
    }

    #[test]
    fn command_produces_crystal_stepped_candidate() {
        let mut shadow = LocalMotionPresentationShadow::default();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut shadow, &mut clock, 0.0);
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(0.0));
        assert_eq!(
            shadow.candidate_offset("self", 11, 10, 0.0, 48.0, 32.0),
            Some(Vec2::new(-40.0, 0.0))
        );
        sync_clock_at(&mut shadow, &mut clock, 100.0);
        assert_eq!(
            shadow.candidate_offset("self", 11, 10, 100.0, 48.0, 32.0),
            Some(Vec2::new(-32.0, 0.0))
        );
    }

    #[test]
    fn delayed_tick_advances_only_one_crystal_phase() {
        let mut shadow = LocalMotionPresentationShadow::default();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut shadow, &mut clock, 0.0);
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(0.0));

        assert_eq!(
            shadow.candidate_offset("self", 11, 10, 0.0, 48.0, 32.0),
            Some(Vec2::new(-40.0, 0.0))
        );
        sync_clock_at(&mut shadow, &mut clock, 350.0);
        assert_eq!(
            shadow.candidate_offset("self", 11, 10, 350.0, 48.0, 32.0),
            Some(Vec2::new(-32.0, 0.0))
        );
        sync_clock_at(&mut shadow, &mut clock, 351.0);
        assert_eq!(
            shadow.candidate_offset("self", 11, 10, 351.0, 48.0, 32.0),
            Some(Vec2::new(-32.0, 0.0))
        );
        sync_clock_at(&mut shadow, &mut clock, 450.0);
        assert_eq!(
            shadow.candidate_offset("self", 11, 10, 450.0, 48.0, 32.0),
            Some(Vec2::new(-24.0, 0.0))
        );
    }

    #[test]
    fn first_phase_latches_when_bevy_consumes_the_command() {
        let mut shadow = LocalMotionPresentationShadow::default();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut shadow, &mut clock, 250.0);
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(100.0));

        let segment = shadow.segment.as_ref().expect("latched segment");
        assert_eq!(segment.command_at_ms, 100.0);
        assert_eq!(segment.started_ms, 250.0);
        assert_eq!(segment.phase_index, 0);
        assert_eq!(
            shadow.candidate_offset("self", 11, 10, 250.0, 48.0, 32.0),
            Some(Vec2::new(-40.0, 0.0))
        );
    }

    #[test]
    fn first_phase_gets_a_full_hundred_milliseconds_near_a_shared_pulse() {
        let mut shadow = LocalMotionPresentationShadow::default();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut shadow, &mut clock, 0.0);
        sync_clock_at(&mut shadow, &mut clock, 50.0);
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(50.0));

        let segment = shadow.segment.as_ref().expect("segment");
        assert_eq!(segment.next_phase_ms, 150.0);
        assert_eq!(segment.expires_ms, 650.0);
        assert_eq!(segment.expires_ms - segment.started_ms, 600.0);

        sync_clock_at(&mut shadow, &mut clock, 149.0);
        assert_eq!(shadow.presentation_phase().expect("phase").frame_index, 0);
        sync_clock_at(&mut shadow, &mut clock, 150.0);
        assert_eq!(shadow.presentation_phase().expect("phase").frame_index, 1);
    }

    #[test]
    fn packed_target_mismatch_falls_back() {
        let mut shadow = LocalMotionPresentationShadow::default();
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(0.0));
        assert_eq!(
            shadow.candidate_offset("self", 10, 10, 100.0, 48.0, 32.0),
            None
        );
        assert_eq!(shadow.target_mismatch_count, 1);
    }

    #[test]
    fn clean_ack_keeps_original_segment_alive() {
        let mut shadow = LocalMotionPresentationShadow::default();
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(0.0));
        shadow.apply_event(ack(50.0, 11, "confirmed"));
        assert!(shadow.segment.is_some());
        assert_eq!(shadow.pending_commands.len(), 0);
    }

    #[test]
    fn presentation_takeover_is_disabled_by_default_and_applies_pending_toggle() {
        PENDING_PRESENTATION_ENABLED.with(|pending| pending.set(None));
        let mut app = App::new();
        app.add_plugins((
            motion::CrystalMoveClockPlugin,
            LocalMotionPresentationShadowPlugin,
        ));
        assert!(!app
            .world()
            .resource::<LocalMotionPresentationShadow>()
            .presentation_enabled());

        set_local_motion_presentation_enabled(true);
        app.world_mut().run_schedule(PreUpdate);
        assert!(app
            .world()
            .resource::<LocalMotionPresentationShadow>()
            .presentation_enabled());
    }

    #[test]
    fn matching_segment_target_survives_settle_but_correction_falls_back() {
        let mut shadow = LocalMotionPresentationShadow::default();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut shadow, &mut clock, 0.0);
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(0.0));
        shadow.apply_event(ack(50.0, 11, "confirmed"));
        assert!(shadow.has_matching_segment_target("self", 11, 10));
        for (at_ms, expected_x) in [
            (0.0, -40.0),
            (100.0, -32.0),
            (200.0, -24.0),
            (300.0, -16.0),
            (400.0, -8.0),
            (500.0, 0.0),
        ] {
            sync_clock_at(&mut shadow, &mut clock, at_ms);
            assert_eq!(
                shadow.candidate_offset("self", 11, 10, at_ms, 48.0, 32.0),
                Some(Vec2::new(expected_x, 0.0))
            );
        }
        sync_clock_at(&mut shadow, &mut clock, 600.0);
        assert_eq!(
            shadow.candidate_offset("self", 11, 10, 600.0, 48.0, 32.0),
            None
        );
        assert!(shadow.has_matching_segment_target("self", 11, 10));

        shadow.apply_event(ack(601.0, 10, "correction"));
        assert!(!shadow.has_matching_segment_target("self", 11, 10));
    }

    #[test]
    fn final_phase_finishes_on_the_sixth_scene_pulse_without_an_ack() {
        let mut shadow = LocalMotionPresentationShadow::default();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut shadow, &mut clock, 0.0);
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(0.0));
        for at_ms in [0.0, 100.0, 200.0, 300.0, 400.0, 500.0] {
            sync_clock_at(&mut shadow, &mut clock, at_ms);
            assert!(shadow
                .candidate_offset("self", 11, 10, at_ms, 48.0, 32.0)
                .is_some());
        }
        assert_eq!(
            shadow.presentation_phase(),
            Some(LocalMotionPresentationPhase {
                frame_index: 5,
                phase_count: 6,
                mode: "walk".to_owned(),
                direction: "Right".to_owned(),
            })
        );

        sync_clock_at(&mut shadow, &mut clock, 600.0);
        assert_eq!(
            shadow.candidate_offset("self", 11, 10, 600.0, 48.0, 32.0),
            None
        );
        assert_eq!(shadow.presentation_phase(), None);
    }

    #[test]
    fn mounted_walk_finishes_on_the_eighth_scene_pulse() {
        let mut shadow = LocalMotionPresentationShadow::default();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut shadow, &mut clock, 0.0);
        shadow.apply_event(reset_event());
        shadow.apply_event(mounted_walk_command(0.0));

        assert_eq!(shadow.segment.as_ref().expect("segment").expires_ms, 800.0);
        for at_ms in [0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0] {
            sync_clock_at(&mut shadow, &mut clock, at_ms);
            assert!(shadow
                .candidate_offset("self", 11, 10, at_ms, 48.0, 32.0)
                .is_some());
        }
        assert_eq!(
            shadow.presentation_phase(),
            Some(LocalMotionPresentationPhase {
                frame_index: 7,
                phase_count: 8,
                mode: "walk".to_owned(),
                direction: "Right".to_owned(),
            })
        );

        sync_clock_at(&mut shadow, &mut clock, 800.0);
        assert_eq!(
            shadow.candidate_offset("self", 11, 10, 800.0, 48.0, 32.0),
            None
        );
    }

    #[test]
    fn correction_or_degraded_target_stays_typescript_owned() {
        let mut corrected = LocalMotionPresentationShadow::default();
        corrected.apply_event(reset_event());
        corrected.apply_event(walk_command(0.0));
        corrected.apply_event(ack(50.0, 10, "correction"));
        assert!(corrected.segment.is_none());

        let mut degraded = LocalMotionPresentationShadow::default();
        degraded.apply_event(reset_event());
        degraded.apply_event(MovementShadowEvent::CommandSent {
            at_ms: 0.0,
            direction: "Right".to_owned(),
            mode: "run".to_owned(),
            from_x: 10,
            from_y: 10,
            to_x: 12,
            to_y: 10,
            phase_count: None,
        });
        degraded.apply_event(ack(50.0, 11, "confirmed"));
        assert!(degraded.segment.is_none());
    }

    #[test]
    fn pixel_comparison_tracks_matches_and_max_delta() {
        let mut shadow = LocalMotionPresentationShadow::default();
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(0.0));
        let ts_window = LocalTsMotionWindow {
            from_x: 10.0,
            from_y: 10.0,
            to_x: 11.0,
            to_y: 10.0,
            started_ms: 50.0,
            expires_ms: 650.0,
        };
        shadow.compare_with_actual(
            100.0,
            "self",
            Vec2::new(-40.0, 0.0),
            Vec2::new(-48.0, 0.0),
            ts_window,
        );
        shadow.compare_with_actual(
            200.0,
            "self",
            Vec2::new(-32.0, 0.0),
            Vec2::new(-32.0, 0.0),
            ts_window,
        );
        assert_eq!(shadow.comparison_sample_count, 2);
        assert_eq!(shadow.comparison_match_count, 1);
        assert_eq!(shadow.comparison_mismatch_count, 1);
        assert_eq!(shadow.max_abs_delta_x, 8.0);
        assert_eq!(shadow.command_phase_mismatch_count, 1);
        assert_eq!(shadow.max_command_phase_abs_delta_x, 8.0);
        assert_eq!(shadow.max_abs_start_delta_ms, 50.0);
        assert_eq!(
            shadow.first_mismatch.as_ref().map(|entry| entry.delta_x),
            Some(8.0)
        );
    }

    #[test]
    fn stale_typescript_window_is_counted_separately() {
        let mut shadow = LocalMotionPresentationShadow::default();
        shadow.record_ts_window_target_mismatch();
        shadow.record_ts_window_path_mismatch();
        assert_eq!(shadow.ts_window_target_mismatch_count, 1);
        assert_eq!(shadow.ts_window_path_mismatch_count, 1);
        assert_eq!(shadow.comparison_sample_count, 0);
    }

    #[test]
    fn typescript_window_path_must_match_local_command_geometry() {
        let mut shadow = LocalMotionPresentationShadow::default();
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(0.0));
        assert!(shadow.segment_matches_ts_window(LocalTsMotionWindow {
            from_x: 10.0,
            from_y: 10.0,
            to_x: 11.0,
            to_y: 10.0,
            started_ms: 5.0,
            expires_ms: 605.0,
        }));
        assert!(!shadow.segment_matches_ts_window(LocalTsMotionWindow {
            from_x: 9.0,
            from_y: 10.0,
            to_x: 11.0,
            to_y: 10.0,
            started_ms: 5.0,
            expires_ms: 605.0,
        }));
    }

    #[test]
    fn overlapping_command_matches_fractional_typescript_window() {
        let mut shadow = LocalMotionPresentationShadow::default();
        let mut clock = motion::CrystalMoveClock::default();
        sync_clock_at(&mut shadow, &mut clock, 0.0);
        shadow.apply_event(reset_event());
        shadow.apply_event(MovementShadowEvent::CommandSent {
            at_ms: 0.0,
            direction: "Right".to_owned(),
            mode: "run".to_owned(),
            from_x: 10,
            from_y: 10,
            to_x: 12,
            to_y: 10,
            phase_count: None,
        });
        for at_ms in [100.0, 200.0, 300.0, 400.0] {
            sync_clock_at(&mut shadow, &mut clock, at_ms);
        }
        let fractional_from = shadow
            .segment
            .as_ref()
            .expect("first segment")
            .current_pose();

        shadow.apply_event(MovementShadowEvent::CommandSent {
            at_ms: 400.0,
            direction: "Right".to_owned(),
            mode: "run".to_owned(),
            from_x: 12,
            from_y: 10,
            to_x: 14,
            to_y: 10,
            phase_count: None,
        });

        assert!(fractional_from.x.fract().abs() > f32::EPSILON);
        assert!(shadow.segment_matches_ts_window(LocalTsMotionWindow {
            from_x: fractional_from.x,
            from_y: fractional_from.y,
            to_x: 14.0,
            to_y: 10.0,
            started_ms: 400.0,
            expires_ms: 1_000.0,
        }));
        assert!(!shadow.segment_matches_ts_window(LocalTsMotionWindow {
            from_x: fractional_from.x.trunc(),
            from_y: fractional_from.y,
            to_x: 14.0,
            to_y: 10.0,
            started_ms: 400.0,
            expires_ms: 1_000.0,
        }));
    }

    #[test]
    fn connected_commands_retain_committed_local_presentation_ownership() {
        let mut shadow = LocalMotionPresentationShadow::default();
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(0.0));
        shadow.mark_presentation_committed();

        shadow.apply_event(MovementShadowEvent::CommandSent {
            at_ms: 100.0,
            direction: "Right".to_owned(),
            mode: "run".to_owned(),
            from_x: 11,
            from_y: 10,
            to_x: 13,
            to_y: 10,
            phase_count: None,
        });
        let rebased_window = LocalTsMotionWindow {
            from_x: 11.0,
            from_y: 10.0,
            to_x: 13.0,
            to_y: 10.0,
            started_ms: 100.0,
            expires_ms: 700.0,
        };

        assert!(
            shadow
                .segment
                .as_ref()
                .expect("connected segment")
                .presentation_committed
        );
        assert!(shadow.committed_segment_matches_ts_target(rebased_window));
    }

    #[test]
    fn applied_center_candidate_starts_on_the_command_frame_without_a_ts_window() {
        let mut shadow = LocalMotionPresentationShadow::default();
        shadow.apply_event(reset_event());
        shadow.apply_event(walk_command(0.0));

        let source_center = shadow
            .candidate_offset_for_applied_center("self", 10, 10, 0.0, 48.0, 32.0)
            .expect("source center candidate");
        let target_center = shadow
            .candidate_offset_for_applied_center("self", 11, 10, 0.0, 48.0, 32.0)
            .expect("target center candidate");
        assert_eq!(source_center, Vec2::new(8.0, 0.0));
        assert_eq!(target_center, Vec2::new(-40.0, 0.0));

        // Changing the base center by one cell and compensating the camera by
        // one cell preserves the exact same composed screen position.
        let source_composed = -source_center.x;
        let target_composed = -48.0 - target_center.x;
        assert_eq!(source_composed, target_composed);
        assert!(shadow
            .candidate_offset_for_applied_center("self", 12, 10, 0.0, 48.0, 32.0)
            .is_none());
    }

    #[test]
    fn left_command_source_center_exposes_crystals_first_eight_pixels() {
        let mut shadow = LocalMotionPresentationShadow::default();
        shadow.apply_event(reset_event());
        shadow.apply_event(MovementShadowEvent::CommandSent {
            at_ms: 0.0,
            direction: "Left".to_owned(),
            mode: "walk".to_owned(),
            from_x: 10,
            from_y: 10,
            to_x: 9,
            to_y: 10,
            phase_count: None,
        });
        let candidate = shadow
            .candidate_offset_for_applied_center("self", 10, 10, 0.0, 48.0, 32.0)
            .expect("left source-center candidate");
        assert_eq!(candidate, Vec2::new(-8.0, 0.0));
    }

    #[test]
    fn command_fifo_is_bounded() {
        let mut shadow = LocalMotionPresentationShadow::default();
        for index in 0..(MAX_PENDING_COMMANDS + 3) {
            shadow.apply_event(walk_command(index as f64));
        }
        assert_eq!(shadow.pending_commands.len(), MAX_PENDING_COMMANDS);
        assert_eq!(shadow.pending_command_drop_count, 3);
    }

    #[test]
    fn plugin_ingests_and_publishes_serializable_diagnostics() {
        PENDING_EVENT_JSON.with(|pending| pending.borrow_mut().clear());
        PENDING_EVENT_DROP_COUNT.with(|count| count.set(0));
        PENDING_PRESENTATION_ENABLED.with(|pending| pending.set(None));
        LATEST_DIAGNOSTICS.with(|latest| latest.borrow_mut().take());
        enqueue_local_motion_event_json(serde_json::to_string(&reset_event()).expect("reset json"));
        enqueue_local_motion_event_json(
            serde_json::to_string(&walk_command(100.0)).expect("command json"),
        );
        let mut app = App::new();
        app.add_plugins((
            motion::CrystalMoveClockPlugin,
            LocalMotionPresentationShadowPlugin,
        ));
        app.world_mut().run_schedule(PreUpdate);
        assert_eq!(
            app.world()
                .resource::<LocalMotionPresentationShadow>()
                .command_event_count,
            1
        );
        app.world_mut().run_schedule(PostUpdate);
        let value: serde_json::Value =
            serde_json::from_str(&get_local_motion_diagnostics_json()).expect("diagnostics json");
        assert_eq!(value["commandEventCount"], 1);
        assert_eq!(value["pendingCommandCount"], 1);
    }
}
