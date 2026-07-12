//! Read-only movement shadow for comparing the Bevy and TypeScript decisions.
//!
//! This module deliberately owns no gameplay authority. It observes normalized
//! movement events, advances an isolated prediction resource on a 100 ms fixed
//! cadence, and publishes diagnostics. It has no command sender and never reads
//! or writes renderer, map, collision, or authoritative world resources.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Crystal's movement decision cadence.
pub(crate) const MOVEMENT_SHADOW_FIXED_INTERVAL_MS: u64 = 100;
pub(crate) const DEFAULT_MOVE_PHASE_COUNT: u8 = 6;
pub(crate) const MAX_MOVE_PHASE_COUNT: u8 = 8;

pub(crate) fn normalized_move_phase_count(value: Option<u8>) -> u8 {
    value
        .unwrap_or(DEFAULT_MOVE_PHASE_COUNT)
        .clamp(1, MAX_MOVE_PHASE_COUNT)
}

const MOVEMENT_SHADOW_FIXED_INTERVAL: Duration =
    Duration::from_millis(MOVEMENT_SHADOW_FIXED_INTERVAL_MS);
const MAX_DECODE_ERROR_CHARS: usize = 512;
const MAX_PENDING_EVENT_JSON: usize = 256;
const MAX_PENDING_COMMANDS: usize = 16;
const MAX_REMOTE_SEGMENTS: usize = 256;

thread_local! {
    static PENDING_MOVEMENT_SHADOW_JSON: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
    static PENDING_MOVEMENT_SHADOW_DROP_COUNT: Cell<u64> = const { Cell::new(0) };
    static LATEST_MOVEMENT_SHADOW_DIAGNOSTICS: RefCell<Option<MovementShadowDiagnostics>> =
        const { RefCell::new(None) };
}

/// Normalized movement events mirrored from the TypeScript controller.
///
/// The internally-tagged representation intentionally matches the browser
/// bridge: `{ "type": "commandSent", "atMs": ... }`, for example.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum MovementShadowEvent {
    Clear {
        at_ms: f64,
    },
    Reset {
        at_ms: f64,
        object_id: String,
        x: i32,
        y: i32,
        direction: String,
    },
    Intent {
        at_ms: f64,
        direction: String,
        mode: String,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
        #[serde(default)]
        phase_count: Option<u8>,
    },
    CommandSent {
        at_ms: f64,
        direction: String,
        mode: String,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
        #[serde(default)]
        phase_count: Option<u8>,
    },
    Authoritative {
        at_ms: f64,
        packet: String,
        object_id: String,
        is_self: bool,
        x: i32,
        y: i32,
        direction: String,
        #[serde(default)]
        ts_predicted_x: Option<i32>,
        #[serde(default)]
        ts_predicted_y: Option<i32>,
        #[serde(default)]
        ts_disposition: Option<String>,
    },
    RemoteMotion {
        at_ms: f64,
        packet: String,
        object_id: String,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
        direction: String,
        mode: String,
        #[serde(default)]
        phase_count: Option<u8>,
    },
    RemoteRemove {
        at_ms: f64,
        object_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MovementIntent {
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
pub(crate) struct ObservedMovementCommand {
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
pub(crate) struct MovementShadowTransform {
    pub(crate) at_ms: f64,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) direction: String,
}

/// Latest observed motion segment for a remote object.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MotionSegment {
    pub(crate) at_ms: f64,
    pub(crate) packet: String,
    pub(crate) object_id: String,
    pub(crate) from_x: i32,
    pub(crate) from_y: i32,
    pub(crate) to_x: i32,
    pub(crate) to_y: i32,
    pub(crate) direction: String,
    pub(crate) mode: String,
    pub(crate) phase_count: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ShadowComparison {
    Match,
    Degraded,
    Mismatch,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MovementCommandDiagnostic {
    pub(crate) command: ObservedMovementCommand,
    pub(crate) shadow_intent: Option<MovementIntent>,
    pub(crate) comparison: ShadowComparison,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MovementAckDiagnostic {
    pub(crate) at_ms: f64,
    pub(crate) packet: String,
    pub(crate) object_id: String,
    pub(crate) command: Option<ObservedMovementCommand>,
    pub(crate) authoritative: MovementShadowTransform,
    pub(crate) shadow_predicted: Option<MovementShadowTransform>,
    /// ACK matching follows the TypeScript movement controller and compares
    /// tile coordinates. Direction agreement is reported independently.
    pub(crate) comparison: ShadowComparison,
    pub(crate) direction_matches: Option<bool>,
    pub(crate) ts_predicted_x: Option<i32>,
    pub(crate) ts_predicted_y: Option<i32>,
    pub(crate) ts_disposition: Option<String>,
    pub(crate) ts_prediction_matches_authoritative: Option<bool>,
    pub(crate) shadow_prediction_matches_ts: Option<bool>,
    pub(crate) disposition_matches_ts: Option<bool>,
}

/// Serializable, bounded view of the movement shadow state.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MovementShadowDiagnostics {
    pub(crate) fixed_interval_ms: u64,
    pub(crate) fixed_tick_count: u64,
    pub(crate) logical_elapsed_ms: u64,
    pub(crate) reset_count: u64,
    pub(crate) processed_event_count: u64,
    pub(crate) pending_event_drop_count: u64,
    pub(crate) decode_error_count: u64,
    pub(crate) last_decode_error: Option<String>,
    pub(crate) self_object_id: Option<String>,
    pub(crate) predicted: Option<MovementShadowTransform>,
    pub(crate) authoritative: Option<MovementShadowTransform>,
    pub(crate) pending_intent: Option<MovementIntent>,
    pub(crate) last_applied_intent: Option<MovementIntent>,
    pub(crate) last_command: Option<ObservedMovementCommand>,
    pub(crate) last_command_diagnostic: Option<MovementCommandDiagnostic>,
    pub(crate) command_match_count: u64,
    pub(crate) command_mismatch_count: u64,
    pub(crate) command_untracked_count: u64,
    pub(crate) pending_command_count: usize,
    pub(crate) pending_command_drop_count: u64,
    pub(crate) last_ack: Option<MovementAckDiagnostic>,
    pub(crate) ack_match_count: u64,
    pub(crate) ack_degraded_count: u64,
    pub(crate) ack_mismatch_count: u64,
    pub(crate) ack_untracked_count: u64,
    pub(crate) remote_motion_event_count: u64,
    pub(crate) remote_remove_event_count: u64,
    pub(crate) remote_segments: Vec<MotionSegment>,
}

impl Default for MovementShadowDiagnostics {
    fn default() -> Self {
        Self {
            fixed_interval_ms: MOVEMENT_SHADOW_FIXED_INTERVAL_MS,
            fixed_tick_count: 0,
            logical_elapsed_ms: 0,
            reset_count: 0,
            processed_event_count: 0,
            pending_event_drop_count: 0,
            decode_error_count: 0,
            last_decode_error: None,
            self_object_id: None,
            predicted: None,
            authoritative: None,
            pending_intent: None,
            last_applied_intent: None,
            last_command: None,
            last_command_diagnostic: None,
            command_match_count: 0,
            command_mismatch_count: 0,
            command_untracked_count: 0,
            pending_command_count: 0,
            pending_command_drop_count: 0,
            last_ack: None,
            ack_match_count: 0,
            ack_degraded_count: 0,
            ack_mismatch_count: 0,
            ack_untracked_count: 0,
            remote_motion_event_count: 0,
            remote_remove_event_count: 0,
            remote_segments: Vec::new(),
        }
    }
}

/// Isolated ECS state used only for movement parity diagnostics.
#[derive(Debug, Default, Resource)]
pub(crate) struct MovementShadow {
    self_object_id: Option<String>,
    predicted: Option<MovementShadowTransform>,
    authoritative: Option<MovementShadowTransform>,
    pending_intent: Option<MovementIntent>,
    last_applied_intent: Option<MovementIntent>,
    last_command: Option<ObservedMovementCommand>,
    last_command_diagnostic: Option<MovementCommandDiagnostic>,
    pending_commands: VecDeque<ObservedMovementCommand>,
    remote_segments: HashMap<String, MotionSegment>,
    fixed_tick_count: u64,
    reset_count: u64,
    processed_event_count: u64,
    pending_event_drop_count: u64,
    decode_error_count: u64,
    last_decode_error: Option<String>,
    command_match_count: u64,
    command_mismatch_count: u64,
    command_untracked_count: u64,
    pending_command_drop_count: u64,
    last_ack: Option<MovementAckDiagnostic>,
    ack_match_count: u64,
    ack_degraded_count: u64,
    ack_mismatch_count: u64,
    ack_untracked_count: u64,
    remote_motion_event_count: u64,
    remote_remove_event_count: u64,
}

// The JS bridge queue is thread-local. Requiring this non-send marker keeps the
// fixed system on Bevy's main thread, so native tests and future threaded builds
// observe the same queue semantics as the browser runtime.
#[derive(Default)]
struct MovementShadowMainThread;

impl MovementShadow {
    #[cfg(test)]
    pub(crate) fn predicted_transform(&self) -> Option<&MovementShadowTransform> {
        self.predicted.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn authoritative_transform(&self) -> Option<&MovementShadowTransform> {
        self.authoritative.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn pending_intent(&self) -> Option<&MovementIntent> {
        self.pending_intent.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn last_applied_intent(&self) -> Option<&MovementIntent> {
        self.last_applied_intent.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn remote_motion_segment(&self, object_id: &str) -> Option<&MotionSegment> {
        self.remote_segments.get(object_id)
    }

    pub(crate) fn diagnostics_snapshot(&self) -> MovementShadowDiagnostics {
        let mut remote_segments: Vec<_> = self.remote_segments.values().cloned().collect();
        remote_segments.sort_by(|left, right| left.object_id.cmp(&right.object_id));

        MovementShadowDiagnostics {
            fixed_interval_ms: MOVEMENT_SHADOW_FIXED_INTERVAL_MS,
            fixed_tick_count: self.fixed_tick_count,
            logical_elapsed_ms: self
                .fixed_tick_count
                .saturating_mul(MOVEMENT_SHADOW_FIXED_INTERVAL_MS),
            reset_count: self.reset_count,
            processed_event_count: self.processed_event_count,
            pending_event_drop_count: self.pending_event_drop_count,
            decode_error_count: self.decode_error_count,
            last_decode_error: self.last_decode_error.clone(),
            self_object_id: self.self_object_id.clone(),
            predicted: self.predicted.clone(),
            authoritative: self.authoritative.clone(),
            pending_intent: self.pending_intent.clone(),
            last_applied_intent: self.last_applied_intent.clone(),
            last_command: self.last_command.clone(),
            last_command_diagnostic: self.last_command_diagnostic.clone(),
            command_match_count: self.command_match_count,
            command_mismatch_count: self.command_mismatch_count,
            command_untracked_count: self.command_untracked_count,
            pending_command_count: self.pending_commands.len(),
            pending_command_drop_count: self.pending_command_drop_count,
            last_ack: self.last_ack.clone(),
            ack_match_count: self.ack_match_count,
            ack_degraded_count: self.ack_degraded_count,
            ack_mismatch_count: self.ack_mismatch_count,
            ack_untracked_count: self.ack_untracked_count,
            remote_motion_event_count: self.remote_motion_event_count,
            remote_remove_event_count: self.remote_remove_event_count,
            remote_segments,
        }
    }

    fn fixed_update(&mut self, pending_json: Vec<String>, dropped_events: u64) {
        self.fixed_tick_count = self.fixed_tick_count.saturating_add(1);

        for json in pending_json {
            match serde_json::from_str::<MovementShadowEvent>(&json) {
                Ok(event) => self.apply_event(event),
                Err(error) => self.record_decode_error(error.to_string()),
            }
        }

        // A fixed step consumes at most the latest still-pending intent. If a
        // commandSent event was observed above, it consumed the latest intent
        // immediately before comparing the observed command, preserving event
        // order when command and ACK both arrive inside one 100 ms window.
        self.apply_latest_intent();
        self.pending_event_drop_count =
            self.pending_event_drop_count.saturating_add(dropped_events);
    }

    fn apply_event(&mut self, event: MovementShadowEvent) {
        match event {
            MovementShadowEvent::Clear { at_ms } => self.clear(at_ms),
            MovementShadowEvent::Reset {
                at_ms,
                object_id,
                x,
                y,
                direction,
            } => self.reset(at_ms, object_id, x, y, direction),
            MovementShadowEvent::Intent {
                at_ms,
                direction,
                mode,
                from_x,
                from_y,
                to_x,
                to_y,
                phase_count,
            } => {
                // Replacement, not a backlog: only the latest input decision is
                // eligible for the next fixed step.
                self.pending_intent = Some(MovementIntent {
                    at_ms,
                    direction,
                    mode,
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    phase_count: normalized_move_phase_count(phase_count),
                });
            }
            MovementShadowEvent::CommandSent {
                at_ms,
                direction,
                mode,
                from_x,
                from_y,
                to_x,
                to_y,
                phase_count,
            } => {
                self.apply_latest_intent();
                self.observe_command(ObservedMovementCommand {
                    at_ms,
                    direction,
                    mode,
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    phase_count: normalized_move_phase_count(phase_count),
                });
            }
            MovementShadowEvent::Authoritative {
                at_ms,
                packet,
                object_id,
                is_self,
                x,
                y,
                direction,
                ts_predicted_x,
                ts_predicted_y,
                ts_disposition,
            } => {
                if is_self {
                    self.observe_self_authoritative(
                        at_ms,
                        packet,
                        object_id,
                        x,
                        y,
                        direction,
                        ts_predicted_x,
                        ts_predicted_y,
                        ts_disposition,
                    );
                }
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
            } => self.observe_remote_motion(MotionSegment {
                at_ms,
                packet,
                object_id,
                from_x,
                from_y,
                to_x,
                to_y,
                direction,
                mode,
                phase_count: normalized_move_phase_count(phase_count),
            }),
            MovementShadowEvent::RemoteRemove {
                at_ms: _,
                object_id,
            } => {
                self.remote_segments.remove(&object_id);
                self.remote_remove_event_count = self.remote_remove_event_count.saturating_add(1);
            }
        }

        self.processed_event_count = self.processed_event_count.saturating_add(1);
    }

    fn clear(&mut self, _at_ms: f64) {
        let fixed_tick_count = self.fixed_tick_count;
        *self = Self::default();
        self.fixed_tick_count = fixed_tick_count;
    }

    fn reset(&mut self, at_ms: f64, object_id: String, x: i32, y: i32, direction: String) {
        let transform = MovementShadowTransform {
            at_ms,
            x,
            y,
            direction,
        };

        self.self_object_id = Some(object_id);
        self.predicted = Some(transform.clone());
        self.authoritative = Some(transform);
        self.pending_intent = None;
        self.last_applied_intent = None;
        self.last_command = None;
        self.last_command_diagnostic = None;
        self.pending_commands.clear();
        self.remote_segments.clear();
        self.processed_event_count = 0;
        self.pending_event_drop_count = 0;
        self.decode_error_count = 0;
        self.last_decode_error = None;
        self.command_match_count = 0;
        self.command_mismatch_count = 0;
        self.command_untracked_count = 0;
        self.pending_command_drop_count = 0;
        self.last_ack = None;
        self.ack_match_count = 0;
        self.ack_degraded_count = 0;
        self.ack_mismatch_count = 0;
        self.ack_untracked_count = 0;
        self.remote_motion_event_count = 0;
        self.remote_remove_event_count = 0;
        self.reset_count = self.reset_count.saturating_add(1);
    }

    fn apply_latest_intent(&mut self) {
        let Some(intent) = self.pending_intent.take() else {
            return;
        };

        let (predicted_x, predicted_y) = movement_destination(&intent);

        self.predicted = Some(MovementShadowTransform {
            at_ms: intent.at_ms,
            x: predicted_x,
            y: predicted_y,
            direction: intent.direction.clone(),
        });
        self.last_applied_intent = Some(intent);
    }

    fn observe_command(&mut self, command: ObservedMovementCommand) {
        let shadow_intent = self.last_applied_intent.clone();
        let comparison = match shadow_intent.as_ref() {
            Some(intent) if command_matches_intent(&command, intent) => ShadowComparison::Match,
            Some(_) => ShadowComparison::Mismatch,
            None => ShadowComparison::Untracked,
        };

        match comparison {
            ShadowComparison::Match => {
                self.command_match_count = self.command_match_count.saturating_add(1)
            }
            ShadowComparison::Degraded | ShadowComparison::Mismatch => {
                self.command_mismatch_count = self.command_mismatch_count.saturating_add(1)
            }
            ShadowComparison::Untracked => {
                self.command_untracked_count = self.command_untracked_count.saturating_add(1)
            }
        }

        self.last_command = Some(command.clone());
        self.last_command_diagnostic = Some(MovementCommandDiagnostic {
            command: command.clone(),
            shadow_intent,
            comparison,
        });
        if self.pending_commands.len() >= MAX_PENDING_COMMANDS {
            self.pending_commands.pop_front();
            self.pending_command_drop_count = self.pending_command_drop_count.saturating_add(1);
        }
        self.pending_commands.push_back(command);
    }

    #[allow(clippy::too_many_arguments)]
    fn observe_self_authoritative(
        &mut self,
        at_ms: f64,
        packet: String,
        object_id: String,
        x: i32,
        y: i32,
        direction: String,
        ts_predicted_x: Option<i32>,
        ts_predicted_y: Option<i32>,
        ts_disposition: Option<String>,
    ) {
        if self.self_object_id.is_none() {
            self.self_object_id = Some(object_id.clone());
        }

        let authoritative = MovementShadowTransform {
            at_ms,
            x,
            y,
            direction,
        };
        let command = self.pending_commands.pop_front();
        let shadow_predicted = command.as_ref().map(command_prediction);
        let comparison = command
            .as_ref()
            .map(|pending| ack_comparison(pending, &authoritative))
            .unwrap_or(ShadowComparison::Untracked);
        let direction_matches = shadow_predicted
            .as_ref()
            .map(|predicted| predicted.direction == authoritative.direction);

        let ts_prediction_matches_authoritative = ts_prediction(ts_predicted_x, ts_predicted_y)
            .map(|(ts_x, ts_y)| ts_x == x && ts_y == y);
        let shadow_prediction_matches_ts = match (
            shadow_predicted.as_ref(),
            ts_prediction(ts_predicted_x, ts_predicted_y),
        ) {
            (Some(predicted), Some((ts_x, ts_y))) => {
                Some(predicted.x == ts_x && predicted.y == ts_y)
            }
            _ => None,
        };
        let disposition_matches_ts = ts_disposition
            .as_deref()
            .and_then(|disposition| comparison_matches_ts_disposition(comparison, disposition));

        match comparison {
            ShadowComparison::Match => {
                self.ack_match_count = self.ack_match_count.saturating_add(1)
            }
            ShadowComparison::Degraded => {
                self.ack_degraded_count = self.ack_degraded_count.saturating_add(1)
            }
            ShadowComparison::Mismatch => {
                self.ack_mismatch_count = self.ack_mismatch_count.saturating_add(1)
            }
            ShadowComparison::Untracked => {
                self.ack_untracked_count = self.ack_untracked_count.saturating_add(1)
            }
        }

        self.authoritative = Some(authoritative.clone());
        self.predicted = self
            .pending_commands
            .back()
            .map(command_prediction)
            .or_else(|| Some(authoritative.clone()));
        self.last_ack = Some(MovementAckDiagnostic {
            at_ms,
            packet,
            object_id,
            command,
            authoritative,
            shadow_predicted,
            comparison,
            direction_matches,
            ts_predicted_x,
            ts_predicted_y,
            ts_disposition,
            ts_prediction_matches_authoritative,
            shadow_prediction_matches_ts,
            disposition_matches_ts,
        });
    }

    fn observe_remote_motion(&mut self, segment: MotionSegment) {
        self.remote_motion_event_count = self.remote_motion_event_count.saturating_add(1);
        if !self.remote_segments.contains_key(&segment.object_id)
            && self.remote_segments.len() >= MAX_REMOTE_SEGMENTS
        {
            let oldest = self
                .remote_segments
                .iter()
                .min_by(|left, right| left.1.at_ms.total_cmp(&right.1.at_ms))
                .map(|(object_id, _)| object_id.clone());
            if let Some(object_id) = oldest {
                self.remote_segments.remove(&object_id);
            }
        }
        self.remote_segments
            .insert(segment.object_id.clone(), segment);
    }

    fn record_decode_error(&mut self, error: String) {
        self.decode_error_count = self.decode_error_count.saturating_add(1);
        self.last_decode_error = Some(error.chars().take(MAX_DECODE_ERROR_CHARS).collect());
    }
}

fn command_matches_intent(command: &ObservedMovementCommand, intent: &MovementIntent) -> bool {
    movement_target_matches_direction(
        intent.from_x,
        intent.from_y,
        intent.to_x,
        intent.to_y,
        &intent.direction,
        &intent.mode,
    ) && movement_target_matches_direction(
        command.from_x,
        command.from_y,
        command.to_x,
        command.to_y,
        &command.direction,
        &command.mode,
    ) && command.direction == intent.direction
        && command.mode == intent.mode
        && command.from_x == intent.from_x
        && command.from_y == intent.from_y
        && command.to_x == intent.to_x
        && command.to_y == intent.to_y
}

fn movement_destination(intent: &MovementIntent) -> (i32, i32) {
    (intent.to_x, intent.to_y)
}

fn movement_target_matches_direction(
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
    direction: &str,
    mode: &str,
) -> bool {
    let dx = i64::from(to_x) - i64::from(from_x);
    let dy = i64::from(to_y) - i64::from(from_y);

    if mode == "turn" {
        return dx == 0 && dy == 0;
    }

    match direction {
        "Up" => dx == 0 && dy < 0,
        "UpRight" => dx > 0 && dy < 0 && dx == -dy,
        "Right" => dx > 0 && dy == 0,
        "DownRight" => dx > 0 && dy > 0 && dx == dy,
        "Down" => dx == 0 && dy > 0,
        "DownLeft" => dx < 0 && dy > 0 && -dx == dy,
        "Left" => dx < 0 && dy == 0,
        "UpLeft" => dx < 0 && dy < 0 && dx == dy,
        _ => false,
    }
}

fn one_tile_destination_from(from_x: i32, from_y: i32, direction: &str) -> Option<(i32, i32)> {
    let (dx, dy) = match direction {
        "Up" => (0, -1),
        "UpRight" => (1, -1),
        "Right" => (1, 0),
        "DownRight" => (1, 1),
        "Down" => (0, 1),
        "DownLeft" => (-1, 1),
        "Left" => (-1, 0),
        "UpLeft" => (-1, -1),
        _ => return None,
    };

    Some((from_x.checked_add(dx)?, from_y.checked_add(dy)?))
}

fn command_prediction(command: &ObservedMovementCommand) -> MovementShadowTransform {
    MovementShadowTransform {
        at_ms: command.at_ms,
        x: command.to_x,
        y: command.to_y,
        direction: command.direction.clone(),
    }
}

fn ack_comparison(
    command: &ObservedMovementCommand,
    authoritative: &MovementShadowTransform,
) -> ShadowComparison {
    if !movement_target_matches_direction(
        command.from_x,
        command.from_y,
        command.to_x,
        command.to_y,
        &command.direction,
        &command.mode,
    ) {
        return ShadowComparison::Mismatch;
    }

    let expected = command_prediction(command);
    if expected.x == authoritative.x
        && expected.y == authoritative.y
        && expected.direction == authoritative.direction
    {
        return ShadowComparison::Match;
    }

    if command.mode == "run" {
        if let Some((degraded_x, degraded_y)) =
            one_tile_destination_from(command.from_x, command.from_y, &command.direction)
        {
            if degraded_x == authoritative.x
                && degraded_y == authoritative.y
                && command.direction == authoritative.direction
            {
                return ShadowComparison::Degraded;
            }
        }
    }

    ShadowComparison::Mismatch
}

fn ts_prediction(x: Option<i32>, y: Option<i32>) -> Option<(i32, i32)> {
    x.zip(y)
}

fn comparison_matches_ts_disposition(
    comparison: ShadowComparison,
    disposition: &str,
) -> Option<bool> {
    match disposition.to_ascii_lowercase().as_str() {
        "confirmed" | "match" => Some(matches!(
            comparison,
            ShadowComparison::Match | ShadowComparison::Degraded
        )),
        "correction" | "mismatch" => Some(comparison == ShadowComparison::Mismatch),
        "accepted" | "untracked" => Some(comparison == ShadowComparison::Untracked),
        // none/staleEcho have no direct shadow equivalent: there was no TS move
        // decision to compare, or the ACK intentionally targets an older move.
        _ => None,
    }
}

/// Adds one JSON event to the current thread's bridge queue.
pub(crate) fn enqueue_movement_shadow_event_json(json: String) {
    PENDING_MOVEMENT_SHADOW_JSON.with(|pending| {
        let mut pending = pending.borrow_mut();
        if pending.len() >= MAX_PENDING_EVENT_JSON {
            pending.pop_front();
            PENDING_MOVEMENT_SHADOW_DROP_COUNT
                .with(|count| count.set(count.get().saturating_add(1)));
        }
        pending.push_back(json);
    });
}

/// Drains the current thread's bridge queue in arrival order.
pub(crate) fn drain_pending_movement_shadow_json() -> Vec<String> {
    PENDING_MOVEMENT_SHADOW_JSON.with(|pending| pending.borrow_mut().drain(..).collect())
}

fn take_pending_movement_shadow_drop_count() -> u64 {
    PENDING_MOVEMENT_SHADOW_DROP_COUNT.with(|count| count.replace(0))
}

/// Returns the latest diagnostics published by the fixed system.
pub(crate) fn get_movement_shadow_diagnostics() -> MovementShadowDiagnostics {
    LATEST_MOVEMENT_SHADOW_DIAGNOSTICS
        .with(|latest| latest.borrow().clone())
        .unwrap_or_default()
}

/// JSON getter suitable for a thin wasm-bindgen wrapper in `lib.rs`.
pub(crate) fn get_movement_shadow_diagnostics_json() -> String {
    serde_json::to_string(&get_movement_shadow_diagnostics()).unwrap_or_else(|_| {
        r#"{"serializationError":"movement shadow diagnostics were not finite"}"#.to_owned()
    })
}

/// The only movement shadow ECS system. It mutates the isolated diagnostic
/// resource and has no access to gameplay, transport, rendering, or collision.
fn movement_shadow_fixed_update_system(
    mut shadow: ResMut<MovementShadow>,
    _main_thread: NonSend<MovementShadowMainThread>,
) {
    shadow.fixed_update(
        drain_pending_movement_shadow_json(),
        take_pending_movement_shadow_drop_count(),
    );
    let snapshot = shadow.diagnostics_snapshot();
    LATEST_MOVEMENT_SHADOW_DIAGNOSTICS.with(|latest| {
        *latest.borrow_mut() = Some(snapshot);
    });
}

/// Optional one-line integration for the runtime app. Add this plugin after
/// Bevy's time plugin so the movement shadow owns the explicit 100 ms cadence.
pub(crate) struct MovementShadowPlugin;

impl Plugin for MovementShadowPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send_resource(MovementShadowMainThread)
            .init_resource::<MovementShadow>()
            .insert_resource(Time::<Fixed>::from_duration(MOVEMENT_SHADOW_FIXED_INTERVAL))
            .add_systems(FixedUpdate, movement_shadow_fixed_update_system);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_thread_locals() {
        let _ = drain_pending_movement_shadow_json();
        PENDING_MOVEMENT_SHADOW_DROP_COUNT.with(|count| count.set(0));
        LATEST_MOVEMENT_SHADOW_DIAGNOSTICS.with(|latest| {
            *latest.borrow_mut() = None;
        });
    }

    fn test_app() -> App {
        clean_thread_locals();
        let mut app = App::new();
        app.add_plugins(MovementShadowPlugin);
        app
    }

    fn enqueue(json: &str) {
        enqueue_movement_shadow_event_json(json.to_owned());
    }

    fn run_fixed(app: &mut App) {
        app.world_mut().run_schedule(FixedUpdate);
    }

    #[test]
    fn event_schema_is_internally_tagged_and_camel_case() {
        let event = MovementShadowEvent::CommandSent {
            at_ms: 125.0,
            direction: "Right".to_owned(),
            mode: "walk".to_owned(),
            from_x: 10,
            from_y: 20,
            to_x: 11,
            to_y: 20,
            phase_count: Some(8),
        };

        let value = serde_json::to_value(event).expect("event should serialize");
        assert_eq!(value["type"], "commandSent");
        assert_eq!(value["atMs"], 125.0);
        assert_eq!(value["fromX"], 10);
        assert_eq!(value["toY"], 20);
        assert_eq!(value["phaseCount"], 8);
        assert!(value.get("at_ms").is_none());
    }

    #[test]
    fn latest_intent_replaces_older_intents_before_the_fixed_step() {
        let mut app = test_app();
        enqueue(r#"{"type":"reset","atMs":0,"objectId":"self","x":10,"y":10,"direction":"Down"}"#);
        enqueue(
            r#"{"type":"intent","atMs":10,"direction":"Right","mode":"walk","fromX":10,"fromY":10,"toX":11,"toY":10}"#,
        );
        enqueue(
            r#"{"type":"intent","atMs":20,"direction":"Down","mode":"walk","fromX":10,"fromY":10,"toX":10,"toY":11}"#,
        );

        run_fixed(&mut app);

        let shadow = app.world().resource::<MovementShadow>();
        let predicted = shadow.predicted_transform().expect("prediction");
        assert_eq!((predicted.x, predicted.y), (10, 11));
        assert_eq!(predicted.direction, "Down");
        assert_eq!(shadow.last_applied_intent().expect("intent").at_ms, 20.0);
        assert!(shadow.pending_intent().is_none());
    }

    #[test]
    fn plugin_and_system_use_one_hundred_millisecond_fixed_ticks() {
        let mut app = test_app();
        assert_eq!(
            app.world().resource::<Time<Fixed>>().timestep(),
            Duration::from_millis(100)
        );

        run_fixed(&mut app);
        run_fixed(&mut app);

        let diagnostics = app
            .world()
            .resource::<MovementShadow>()
            .diagnostics_snapshot();
        assert_eq!(diagnostics.fixed_interval_ms, 100);
        assert_eq!(diagnostics.fixed_tick_count, 2);
        assert_eq!(diagnostics.logical_elapsed_ms, 200);
    }

    #[test]
    fn matching_ack_compares_shadow_typescript_and_authoritative_positions() {
        let mut app = test_app();
        enqueue(r#"{"type":"reset","atMs":0,"objectId":"self","x":4,"y":5,"direction":"Right"}"#);
        enqueue(
            r#"{"type":"intent","atMs":100,"direction":"Right","mode":"walk","fromX":4,"fromY":5,"toX":5,"toY":5}"#,
        );
        enqueue(
            r#"{"type":"commandSent","atMs":101,"direction":"Right","mode":"walk","fromX":4,"fromY":5,"toX":5,"toY":5}"#,
        );
        enqueue(
            r#"{"type":"authoritative","atMs":180,"packet":"UserLocation","objectId":"self","isSelf":true,"x":5,"y":5,"direction":"Right","tsPredictedX":5,"tsPredictedY":5,"tsDisposition":"confirmed"}"#,
        );

        run_fixed(&mut app);

        let diagnostics = app
            .world()
            .resource::<MovementShadow>()
            .diagnostics_snapshot();
        let ack = diagnostics.last_ack.expect("ACK diagnostic");
        assert_eq!(ack.comparison, ShadowComparison::Match);
        assert_eq!(ack.direction_matches, Some(true));
        assert_eq!(ack.ts_prediction_matches_authoritative, Some(true));
        assert_eq!(ack.shadow_prediction_matches_ts, Some(true));
        assert_eq!(ack.disposition_matches_ts, Some(true));
        assert_eq!(diagnostics.ack_match_count, 1);
        assert_eq!(diagnostics.ack_mismatch_count, 0);
        assert_eq!(diagnostics.command_match_count, 1);
    }

    #[test]
    fn ordinary_two_tile_run_uses_the_explicit_target_for_command_and_ack() {
        let mut app = test_app();
        enqueue(r#"{"type":"reset","atMs":0,"objectId":"self","x":4,"y":5,"direction":"Right"}"#);
        enqueue(
            r#"{"type":"intent","atMs":100,"direction":"Right","mode":"run","fromX":4,"fromY":5,"toX":6,"toY":5,"phaseCount":6}"#,
        );
        enqueue(
            r#"{"type":"commandSent","atMs":101,"direction":"Right","mode":"run","fromX":4,"fromY":5,"toX":6,"toY":5,"phaseCount":6}"#,
        );
        enqueue(
            r#"{"type":"authoritative","atMs":180,"packet":"UserLocation","objectId":"self","isSelf":true,"x":6,"y":5,"direction":"Right","tsPredictedX":6,"tsPredictedY":5,"tsDisposition":"confirmed"}"#,
        );

        run_fixed(&mut app);

        let diagnostics = app
            .world()
            .resource::<MovementShadow>()
            .diagnostics_snapshot();
        let ack = diagnostics.last_ack.expect("two-tile run ACK");
        assert_eq!(ack.comparison, ShadowComparison::Match);
        assert_eq!(
            ack.shadow_predicted
                .map(|predicted| (predicted.x, predicted.y)),
            Some((6, 5))
        );
        assert_eq!(diagnostics.command_match_count, 1);
        assert_eq!(diagnostics.ack_match_count, 1);
    }

    #[test]
    fn mounted_or_swift_feet_three_tile_run_uses_the_explicit_target() {
        let mut app = test_app();
        enqueue(
            r#"{"type":"reset","atMs":0,"objectId":"self","x":331,"y":270,"direction":"Right"}"#,
        );
        enqueue(
            r#"{"type":"intent","atMs":100,"direction":"Right","mode":"run","fromX":331,"fromY":270,"toX":334,"toY":270,"phaseCount":6}"#,
        );
        enqueue(
            r#"{"type":"commandSent","atMs":101,"direction":"Right","mode":"run","fromX":331,"fromY":270,"toX":334,"toY":270,"phaseCount":6}"#,
        );
        enqueue(
            r#"{"type":"authoritative","atMs":180,"packet":"UserLocation","objectId":"self","isSelf":true,"x":334,"y":270,"direction":"Right","tsPredictedX":334,"tsPredictedY":270,"tsDisposition":"confirmed"}"#,
        );

        run_fixed(&mut app);

        let diagnostics = app
            .world()
            .resource::<MovementShadow>()
            .diagnostics_snapshot();
        let command = diagnostics
            .last_command
            .as_ref()
            .expect("three-tile command");
        assert_eq!(
            (command.from_x, command.from_y, command.to_x, command.to_y),
            (331, 270, 334, 270)
        );
        assert_eq!(command.phase_count, 6);
        let ack = diagnostics.last_ack.expect("three-tile run ACK");
        assert_eq!(ack.comparison, ShadowComparison::Match);
        assert_eq!(
            ack.shadow_predicted
                .map(|predicted| (predicted.x, predicted.y)),
            Some((334, 270))
        );
        assert_eq!(diagnostics.command_match_count, 1);
        assert_eq!(diagnostics.ack_match_count, 1);
        assert_eq!(diagnostics.command_mismatch_count, 0);
        assert_eq!(diagnostics.ack_mismatch_count, 0);
    }

    #[test]
    fn duplicate_ack_is_untracked_after_fifo_command_is_consumed() {
        let mut app = test_app();
        enqueue(r#"{"type":"reset","atMs":0,"objectId":"self","x":4,"y":5,"direction":"Right"}"#);
        enqueue(
            r#"{"type":"intent","atMs":100,"direction":"Right","mode":"walk","fromX":4,"fromY":5,"toX":5,"toY":5}"#,
        );
        enqueue(
            r#"{"type":"commandSent","atMs":101,"direction":"Right","mode":"walk","fromX":4,"fromY":5,"toX":5,"toY":5}"#,
        );
        for at_ms in [180, 181] {
            enqueue(&format!(
                r#"{{"type":"authoritative","atMs":{at_ms},"packet":"UserLocation","objectId":"self","isSelf":true,"x":5,"y":5,"direction":"Right","tsPredictedX":5,"tsPredictedY":5,"tsDisposition":"confirmed"}}"#
            ));
        }

        run_fixed(&mut app);

        let diagnostics = app
            .world()
            .resource::<MovementShadow>()
            .diagnostics_snapshot();
        assert_eq!(diagnostics.ack_match_count, 1);
        assert_eq!(diagnostics.ack_untracked_count, 1);
        assert_eq!(diagnostics.pending_command_count, 0);
        assert_eq!(
            diagnostics.last_ack.expect("duplicate ACK").comparison,
            ShadowComparison::Untracked
        );
    }

    #[test]
    fn turn_ack_requires_direction_match_even_when_tile_is_unchanged() {
        let mut app = test_app();
        enqueue(r#"{"type":"reset","atMs":0,"objectId":"self","x":4,"y":5,"direction":"Right"}"#);
        enqueue(
            r#"{"type":"intent","atMs":100,"direction":"Left","mode":"turn","fromX":4,"fromY":5,"toX":4,"toY":5}"#,
        );
        enqueue(
            r#"{"type":"commandSent","atMs":101,"direction":"Left","mode":"turn","fromX":4,"fromY":5,"toX":4,"toY":5}"#,
        );
        enqueue(
            r#"{"type":"authoritative","atMs":180,"packet":"UserLocation","objectId":"self","isSelf":true,"x":4,"y":5,"direction":"Right","tsPredictedX":4,"tsPredictedY":5,"tsDisposition":"correction"}"#,
        );

        run_fixed(&mut app);

        let diagnostics = app
            .world()
            .resource::<MovementShadow>()
            .diagnostics_snapshot();
        let ack = diagnostics.last_ack.expect("turn ACK");
        assert_eq!(ack.comparison, ShadowComparison::Mismatch);
        assert_eq!(ack.direction_matches, Some(false));
        assert_eq!(ack.disposition_matches_ts, Some(true));
    }

    #[test]
    fn one_tile_run_landing_is_counted_as_confirmed_degradation() {
        let mut app = test_app();
        enqueue(
            r#"{"type":"reset","atMs":0,"objectId":"self","x":331,"y":270,"direction":"Right"}"#,
        );
        enqueue(
            r#"{"type":"intent","atMs":100,"direction":"Right","mode":"run","fromX":331,"fromY":270,"toX":334,"toY":270,"phaseCount":6}"#,
        );
        enqueue(
            r#"{"type":"commandSent","atMs":101,"direction":"Right","mode":"run","fromX":331,"fromY":270,"toX":334,"toY":270,"phaseCount":6}"#,
        );
        enqueue(
            r#"{"type":"authoritative","atMs":180,"packet":"UserLocation","objectId":"self","isSelf":true,"x":332,"y":270,"direction":"Right","tsPredictedX":334,"tsPredictedY":270,"tsDisposition":"confirmed"}"#,
        );

        run_fixed(&mut app);

        let diagnostics = app
            .world()
            .resource::<MovementShadow>()
            .diagnostics_snapshot();
        let ack = diagnostics.last_ack.expect("degraded ACK");
        assert_eq!(ack.comparison, ShadowComparison::Degraded);
        assert_eq!(
            ack.shadow_predicted
                .map(|predicted| (predicted.x, predicted.y)),
            Some((334, 270))
        );
        assert_eq!(ack.disposition_matches_ts, Some(true));
        assert_eq!(diagnostics.ack_degraded_count, 1);
        assert_eq!(diagnostics.ack_mismatch_count, 0);
    }

    #[test]
    fn target_vector_that_disagrees_with_direction_is_a_mismatch() {
        let mut app = test_app();
        enqueue(
            r#"{"type":"reset","atMs":0,"objectId":"self","x":331,"y":270,"direction":"Right"}"#,
        );
        enqueue(
            r#"{"type":"intent","atMs":100,"direction":"Right","mode":"run","fromX":331,"fromY":270,"toX":331,"toY":267,"phaseCount":6}"#,
        );
        enqueue(
            r#"{"type":"commandSent","atMs":101,"direction":"Right","mode":"run","fromX":331,"fromY":270,"toX":331,"toY":267,"phaseCount":6}"#,
        );
        enqueue(
            r#"{"type":"authoritative","atMs":180,"packet":"UserLocation","objectId":"self","isSelf":true,"x":331,"y":267,"direction":"Right","tsPredictedX":331,"tsPredictedY":267,"tsDisposition":"correction"}"#,
        );

        run_fixed(&mut app);

        let shadow = app.world().resource::<MovementShadow>();
        let diagnostics = shadow.diagnostics_snapshot();
        assert_eq!(diagnostics.command_match_count, 0);
        assert_eq!(diagnostics.command_mismatch_count, 1);
        assert_eq!(
            diagnostics
                .last_command_diagnostic
                .expect("command diagnostic")
                .comparison,
            ShadowComparison::Mismatch
        );
        let ack = diagnostics.last_ack.expect("wrong-direction ACK");
        assert_eq!(ack.comparison, ShadowComparison::Mismatch);
        assert_eq!(
            ack.shadow_predicted
                .map(|predicted| (predicted.x, predicted.y)),
            Some((331, 267))
        );
        assert_eq!(ack.direction_matches, Some(true));
        assert_eq!(diagnostics.ack_mismatch_count, 1);
    }

    #[test]
    fn mismatching_ack_preserves_diagnostic_prediction_then_reconciles_current_state() {
        let mut app = test_app();
        enqueue(r#"{"type":"reset","atMs":0,"objectId":"self","x":4,"y":5,"direction":"Right"}"#);
        enqueue(
            r#"{"type":"intent","atMs":100,"direction":"Right","mode":"walk","fromX":4,"fromY":5,"toX":5,"toY":5}"#,
        );
        enqueue(
            r#"{"type":"commandSent","atMs":101,"direction":"Right","mode":"walk","fromX":4,"fromY":5,"toX":5,"toY":5}"#,
        );
        enqueue(
            r#"{"type":"authoritative","atMs":180,"packet":"UserLocation","objectId":"self","isSelf":true,"x":4,"y":5,"direction":"Left","tsPredictedX":5,"tsPredictedY":5,"tsDisposition":"correction"}"#,
        );

        run_fixed(&mut app);

        let shadow = app.world().resource::<MovementShadow>();
        assert_eq!(
            (
                shadow.predicted_transform().unwrap().x,
                shadow.predicted_transform().unwrap().y
            ),
            (4, 5)
        );
        assert_eq!(
            (
                shadow.authoritative_transform().unwrap().x,
                shadow.authoritative_transform().unwrap().y,
            ),
            (4, 5)
        );
        let diagnostics = shadow.diagnostics_snapshot();
        let ack = diagnostics.last_ack.expect("ACK diagnostic");
        assert_eq!(ack.comparison, ShadowComparison::Mismatch);
        assert_eq!(ack.direction_matches, Some(false));
        assert_eq!(ack.ts_prediction_matches_authoritative, Some(false));
        assert_eq!(ack.shadow_prediction_matches_ts, Some(true));
        assert_eq!(ack.disposition_matches_ts, Some(true));
        assert_eq!(diagnostics.ack_mismatch_count, 1);
    }

    #[test]
    fn reset_reinitializes_session_diagnostics_and_shadow_transforms() {
        let mut app = test_app();
        enqueue(r#"{"type":"reset","atMs":0,"objectId":"old","x":1,"y":1,"direction":"Up"}"#);
        enqueue(
            r#"{"type":"remoteMotion","atMs":10,"packet":"ObjectWalk","objectId":"remote","fromX":2,"fromY":2,"toX":3,"toY":2,"direction":"Right","mode":"walk"}"#,
        );
        enqueue(
            r#"{"type":"authoritative","atMs":20,"packet":"UserLocation","objectId":"old","isSelf":true,"x":9,"y":9,"direction":"Down"}"#,
        );
        run_fixed(&mut app);

        enqueue(r#"{"type":"reset","atMs":500,"objectId":"new","x":7,"y":8,"direction":"Left"}"#);
        run_fixed(&mut app);

        let shadow = app.world().resource::<MovementShadow>();
        let diagnostics = shadow.diagnostics_snapshot();
        assert_eq!(diagnostics.self_object_id.as_deref(), Some("new"));
        assert_eq!(diagnostics.predicted, diagnostics.authoritative);
        assert_eq!(diagnostics.predicted.unwrap().x, 7);
        assert_eq!(diagnostics.processed_event_count, 1);
        assert_eq!(diagnostics.reset_count, 2);
        assert_eq!(diagnostics.ack_mismatch_count, 0);
        assert!(diagnostics.last_ack.is_none());
        assert!(diagnostics.remote_segments.is_empty());
        assert!(shadow.remote_motion_segment("remote").is_none());
    }

    #[test]
    fn remote_motion_records_latest_segment_per_object() {
        let mut app = test_app();
        enqueue(
            r#"{"type":"remoteMotion","atMs":100,"packet":"ObjectWalk","objectId":"p2","fromX":2,"fromY":3,"toX":3,"toY":3,"direction":"Right","mode":"walk"}"#,
        );
        enqueue(
            r#"{"type":"remoteMotion","atMs":200,"packet":"ObjectRun","objectId":"p2","fromX":3,"fromY":3,"toX":5,"toY":3,"direction":"Right","mode":"run"}"#,
        );
        enqueue(
            r#"{"type":"remoteMotion","atMs":150,"packet":"ObjectWalk","objectId":"p1","fromX":9,"fromY":9,"toX":9,"toY":8,"direction":"Up","mode":"walk"}"#,
        );

        run_fixed(&mut app);

        let shadow = app.world().resource::<MovementShadow>();
        let segment = shadow.remote_motion_segment("p2").expect("segment");
        assert_eq!(segment.packet, "ObjectRun");
        assert_eq!((segment.from_x, segment.to_x), (3, 5));
        assert_eq!(segment.mode, "run");

        let diagnostics = shadow.diagnostics_snapshot();
        assert_eq!(diagnostics.remote_motion_event_count, 3);
        assert_eq!(diagnostics.remote_segments.len(), 2);
        assert_eq!(diagnostics.remote_segments[0].object_id, "p1");
        assert_eq!(diagnostics.remote_segments[1].object_id, "p2");
    }

    #[test]
    fn pending_event_queue_and_remote_segment_registry_are_bounded() {
        let mut app = test_app();
        for index in 0..(MAX_PENDING_EVENT_JSON + 5) {
            enqueue(&format!(
                r#"{{"type":"remoteRemove","atMs":{index},"objectId":"discard-{index}"}}"#
            ));
        }
        run_fixed(&mut app);
        let diagnostics = app
            .world()
            .resource::<MovementShadow>()
            .diagnostics_snapshot();
        assert_eq!(diagnostics.pending_event_drop_count, 5);
        assert_eq!(
            diagnostics.processed_event_count,
            MAX_PENDING_EVENT_JSON as u64
        );

        for index in 0..MAX_REMOTE_SEGMENTS {
            enqueue(&format!(
                r#"{{"type":"remoteMotion","atMs":{index},"packet":"ObjectWalk","objectId":"remote-{index}","fromX":0,"fromY":0,"toX":1,"toY":0,"direction":"Right","mode":"walk"}}"#
            ));
        }
        run_fixed(&mut app);
        enqueue(&format!(
            r#"{{"type":"remoteMotion","atMs":9999,"packet":"ObjectRun","objectId":"remote-new","fromX":1,"fromY":0,"toX":3,"toY":0,"direction":"Right","mode":"run"}}"#
        ));
        run_fixed(&mut app);
        assert_eq!(
            app.world()
                .resource::<MovementShadow>()
                .diagnostics_snapshot()
                .remote_segments
                .len(),
            MAX_REMOTE_SEGMENTS
        );

        enqueue(r#"{"type":"remoteRemove","atMs":10000,"objectId":"remote-new"}"#);
        run_fixed(&mut app);
        let diagnostics = app
            .world()
            .resource::<MovementShadow>()
            .diagnostics_snapshot();
        assert_eq!(diagnostics.remote_segments.len(), MAX_REMOTE_SEGMENTS - 1);
        assert!(diagnostics.remote_remove_event_count > 0);
    }

    #[test]
    fn clear_starts_a_fresh_diagnostic_session_without_restarting_fixed_time() {
        let mut app = test_app();
        enqueue(r#"{"type":"reset","atMs":0,"objectId":"self","x":1,"y":2,"direction":"Down"}"#);
        enqueue(
            r#"{"type":"remoteMotion","atMs":1,"packet":"ObjectWalk","objectId":"remote","fromX":2,"fromY":2,"toX":3,"toY":2,"direction":"Right","mode":"walk"}"#,
        );
        run_fixed(&mut app);
        enqueue(r#"{"type":"clear","atMs":100}"#);
        run_fixed(&mut app);

        let diagnostics = app
            .world()
            .resource::<MovementShadow>()
            .diagnostics_snapshot();
        assert_eq!(diagnostics.fixed_tick_count, 2);
        assert_eq!(diagnostics.processed_event_count, 1);
        assert!(diagnostics.self_object_id.is_none());
        assert!(diagnostics.remote_segments.is_empty());
        assert_eq!(diagnostics.command_match_count, 0);
        assert_eq!(diagnostics.ack_match_count, 0);
    }

    #[test]
    fn malformed_json_is_counted_without_stopping_the_fixed_system() {
        let mut app = test_app();
        enqueue("{not-json");
        enqueue(r#"{"type":"reset","atMs":1,"objectId":"self","x":1,"y":2,"direction":"Down"}"#);
        enqueue("{still-not-json");

        run_fixed(&mut app);

        let diagnostics = app
            .world()
            .resource::<MovementShadow>()
            .diagnostics_snapshot();
        // Reset clears errors that preceded it; the later decode error remains.
        assert_eq!(diagnostics.decode_error_count, 1);
        assert!(diagnostics.last_decode_error.is_some());
        assert_eq!(diagnostics.processed_event_count, 1);
    }

    #[test]
    fn thread_local_getters_publish_a_serializable_snapshot() {
        let mut app = test_app();
        enqueue(r#"{"type":"reset","atMs":1,"objectId":"self","x":3,"y":4,"direction":"Down"}"#);
        run_fixed(&mut app);

        let snapshot = get_movement_shadow_diagnostics();
        assert_eq!(snapshot.fixed_tick_count, 1);
        assert_eq!(snapshot.self_object_id.as_deref(), Some("self"));

        let json = get_movement_shadow_diagnostics_json();
        let value: serde_json::Value = serde_json::from_str(&json).expect("diagnostics JSON");
        assert_eq!(value["fixedIntervalMs"], 100);
        assert_eq!(value["selfObjectId"], "self");
    }
}
