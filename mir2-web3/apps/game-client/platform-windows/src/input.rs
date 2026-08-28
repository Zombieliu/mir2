//! Native keyboard input 鈫?gateway player intents.
//!
//! Maps WASD / arrow keys to Mir2 directions and forwards them to the gateway
//! WebSocket task via the cross-thread [`gateway::GatewayCommandSender`]. This
//! is a thin presentation鈫抜ntent edge (ADR-0001): the server validates
//! movement; the client only expresses the request.

use bevy::input::ButtonInput;
use bevy::prelude::{Interaction, KeyCode, MouseButton, Query, Res, ResMut, Time, Window, With};
use mir2_client_bevy::crystal_ui::hud::{belt_slot_item, CrystalHudAction};
use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
use mir2_client_bevy::entities::{EntityKind, EntityModelSet};
use mir2_client_bevy::inventory::InventoryModel;
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use mir2_client_bevy::quest_model::{CombatTargetModel, NpcDialogModel};
use mir2_client_bevy::quest_ui::{QuestUiIntent, QuestUiIntentQueue};
use mir2_client_bevy::read_model::UiReadModel;
use mir2_client_bevy::skill_model::SkillModel;

use crate::effects::{NativeEffects, CELL_HEIGHT, CELL_WIDTH};
use crate::entity_presentation::NativeEntityPresentation;
use crate::gameplay_bridge::{GameplayEventInbox, NativeSelfMovementAck};
use crate::gateway::{GatewayCommand, GatewayCommandSender, PlayerIntent};
use crate::native_protocol::NativeOutboundCommand;

/// Mir2 direction strings the gateway `Walk`/`Run`/`Turn` commands accept.
const UP: &str = "up";
const DOWN: &str = "down";
const LEFT: &str = "left";
const RIGHT: &str = "right";

/// Bevy resource holding the gateway command sender, injected by the host.
#[derive(bevy::prelude::Resource)]
pub struct GatewayCommands {
    sender: GatewayCommandSender,
}

impl GatewayCommands {
    pub fn new(sender: impl Into<GatewayCommandSender>) -> Self {
        Self {
            sender: sender.into(),
        }
    }

    pub fn send_command(&self, command: GatewayCommand) -> bool {
        self.sender.send(command).is_ok()
    }

    fn send(&self, intent: PlayerIntent) -> bool {
        self.send_command(GatewayCommand::Player(intent))
    }

    fn send_town_revive(&self) {
        self.send_command(GatewayCommand::Wire(NativeOutboundCommand::TownRevive));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorldPointerMovementMode {
    Walk,
    Run,
}

const CRYSTAL_MOVE_PRESENTATION_MS: f64 = 600.0;
const CRYSTAL_RUN_PRIME_MS: f64 = 1_200.0;
const CRYSTAL_CORRECTION_BLOCK_MS: f64 = 400.0;
const MOVEMENT_PENDING_MAX_AGE_MS: f64 = 3_000.0;
const MOVEMENT_BLOCKED_STEP_MAX_AGE_MS: f64 = 3_000.0;
const MOVEMENT_BLOCKED_STEP_LIMIT: usize = 16;
const NPC_APPROACH_MAX_AGE_MS: f64 = 15_000.0;

#[derive(Clone, Debug, PartialEq)]
struct PendingSelfMove {
    from: (i32, i32),
    to: (i32, i32),
    direction: &'static str,
    mode: WorldPointerMovementMode,
    sent_at_ms: f64,
    visual_until_ms: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct PendingNpcInteraction {
    object_id: u32,
    started_at_ms: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct BlockedSelfMove {
    from: (i32, i32),
    direction: &'static str,
    mode: WorldPointerMovementMode,
    observed_at_ms: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MovementAckOutcome {
    Confirmed,
    Degraded,
    Correction,
    Accepted,
}

/// Crystal-style native movement controller. The shared Zone remains the only
/// gameplay authority; this resource owns one bounded pending intent plus a
/// presentation-only prediction copied into the shared runtime. ACKs either
/// confirm that visual path or clear it and expose the authoritative tile.
#[derive(bevy::prelude::Resource, Default, Debug)]
pub struct WorldPointerMovementState {
    active: Option<WorldPointerMovementMode>,
    pending: Option<PendingSelfMove>,
    self_object_id: Option<String>,
    authoritative_position: Option<(i32, i32)>,
    authoritative_direction: Option<String>,
    next_move_send_at_ms: f64,
    run_primed_until_ms: f64,
    input_blocked_until_ms: f64,
    last_packet_ack_at_ms: Option<f64>,
    pending_npc_interaction: Option<PendingNpcInteraction>,
    blocked_steps: Vec<BlockedSelfMove>,
}

impl WorldPointerMovementState {
    fn begin(&mut self, mode: WorldPointerMovementMode) {
        self.active = Some(mode);
    }

    fn stop_hold(&mut self) {
        self.active = None;
    }

    fn reset_controller(&mut self) {
        *self = Self::default();
    }

    fn cancel_npc_approach(&mut self) {
        self.pending_npc_interaction = None;
    }

    fn can_send(&self, now_ms: f64) -> bool {
        self.pending.is_none()
            && now_ms >= self.next_move_send_at_ms
            && now_ms >= self.input_blocked_until_ms
    }

    fn effective_mode(
        &self,
        requested: WorldPointerMovementMode,
        now_ms: f64,
    ) -> WorldPointerMovementMode {
        if requested == WorldPointerMovementMode::Run && now_ms < self.run_primed_until_ms {
            WorldPointerMovementMode::Run
        } else {
            WorldPointerMovementMode::Walk
        }
    }

    fn prune_blocked_steps(&mut self, now_ms: f64) {
        self.blocked_steps.retain(|step| {
            now_ms >= step.observed_at_ms
                && now_ms <= step.observed_at_ms + MOVEMENT_BLOCKED_STEP_MAX_AGE_MS
        });
    }

    fn remember_blocked_step(&mut self, pending: &PendingSelfMove, now_ms: f64) {
        self.prune_blocked_steps(now_ms);
        self.blocked_steps.retain(|step| {
            step.from != pending.from
                || step.direction != pending.direction
                || step.mode != pending.mode
        });
        self.blocked_steps.push(BlockedSelfMove {
            from: pending.from,
            direction: pending.direction,
            mode: pending.mode,
            observed_at_ms: now_ms,
        });
        if self.blocked_steps.len() > MOVEMENT_BLOCKED_STEP_LIMIT {
            self.blocked_steps
                .drain(..self.blocked_steps.len() - MOVEMENT_BLOCKED_STEP_LIMIT);
        }
    }

    fn step_was_rejected(
        &self,
        origin: (i32, i32),
        direction: &str,
        mode: WorldPointerMovementMode,
    ) -> bool {
        self.blocked_steps.iter().any(|step| {
            step.from == origin
                && step.direction.eq_ignore_ascii_case(direction)
                && (step.mode == mode
                    || (mode == WorldPointerMovementMode::Run
                        && step.mode == WorldPointerMovementMode::Walk))
        })
    }

    fn observe_identity(&mut self, object_id: &str, position: (i32, i32), direction: &str) -> bool {
        if self.self_object_id.as_deref() == Some(object_id) {
            if self.authoritative_position.is_none() {
                self.authoritative_position = Some(position);
            }
            if self.authoritative_direction.is_none() {
                self.authoritative_direction = Some(direction.to_owned());
            }
            return false;
        }
        self.pending = None;
        self.next_move_send_at_ms = 0.0;
        self.run_primed_until_ms = 0.0;
        self.input_blocked_until_ms = 0.0;
        self.pending_npc_interaction = None;
        self.self_object_id = Some(object_id.to_owned());
        self.authoritative_position = Some(position);
        self.authoritative_direction = Some(direction.to_owned());
        true
    }

    fn reconcile_ack(&mut self, ack: &NativeSelfMovementAck, now_ms: f64) -> MovementAckOutcome {
        self.authoritative_position = Some((ack.x, ack.y));
        self.authoritative_direction = Some(ack.direction.clone());
        self.last_packet_ack_at_ms = Some(now_ms);
        let Some(pending) = self.pending.take() else {
            return MovementAckOutcome::Accepted;
        };
        let outcome = if (ack.x, ack.y) == pending.to {
            MovementAckOutcome::Confirmed
        } else if pending.mode == WorldPointerMovementMode::Run
            && (ack.x, ack.y) == movement_target(pending.from, pending.direction, 1)
        {
            MovementAckOutcome::Degraded
        } else {
            MovementAckOutcome::Correction
        };
        if matches!(
            outcome,
            MovementAckOutcome::Confirmed | MovementAckOutcome::Degraded
        ) {
            self.next_move_send_at_ms = self.next_move_send_at_ms.max(pending.visual_until_ms);
            self.run_primed_until_ms = now_ms + CRYSTAL_RUN_PRIME_MS;
        } else {
            self.remember_blocked_step(&pending, now_ms);
            self.run_primed_until_ms = 0.0;
            self.input_blocked_until_ms = now_ms + CRYSTAL_CORRECTION_BLOCK_MS;
            self.next_move_send_at_ms = self.next_move_send_at_ms.max(self.input_blocked_until_ms);
        }
        outcome
    }
}

fn window_is_focused(windows: &Query<&Window>) -> bool {
    windows
        .iter()
        .next()
        .map(|window| window.focused)
        .unwrap_or(true)
}

fn gameplay_input_enabled(
    shell: Option<&NativeShellModel>,
    player_ui: Option<&NativePlayerUiState>,
    windows: &Query<&Window>,
) -> bool {
    if !window_is_focused(windows) {
        return false;
    }
    if !shell.is_some_and(|shell| shell.screen == NativeShellScreen::InGame) {
        return false;
    }
    if is_world_click_blocked(player_ui, false, false) {
        return false;
    }
    true
}

pub fn is_world_click_blocked(
    player_ui: Option<&NativePlayerUiState>,
    dialog_open: bool,
    dead: bool,
) -> bool {
    if let Some(ui) = player_ui {
        if ui.blocks_world_click() {
            return true;
        }
        if ui.captures_pointer(false, false, false) {
            return true;
        }
        if ui.blocks_world_action(dialog_open, dead) {
            return true;
        }
    } else if dialog_open || dead {
        return true;
    }
    false
}

pub fn is_pointer_captured_for_movement(
    player_ui: Option<&NativePlayerUiState>,
    is_dragging_window: bool,
    is_dragging_scrollbar: bool,
    button_pressed: bool,
) -> bool {
    if is_dragging_window || is_dragging_scrollbar || button_pressed {
        return true;
    }
    player_ui.is_some_and(|ui| {
        ui.blocks_world_click()
            || ui.captures_pointer(is_dragging_window, is_dragging_scrollbar, button_pressed)
    })
}

fn hovered_world_intent(
    hovered_object_id: Option<&str>,
    entities: &EntityModelSet,
) -> Option<QuestUiIntent> {
    let object_id = hovered_object_id?
        .parse::<u32>()
        .ok()
        .filter(|id| *id != 0)?;
    let entity = entities.entities.iter().find(|entity| {
        entity.object_id == object_id.to_string()
            && matches!(entity.kind, EntityKind::Monster | EntityKind::Npc)
    })?;
    match entity.kind {
        EntityKind::Monster => Some(QuestUiIntent::AttackTarget { object_id }),
        EntityKind::Npc => Some(QuestUiIntent::InteractNpc {
            npc_object_id: object_id,
        }),
        EntityKind::Player | EntityKind::SelfPlayer => None,
    }
}

fn pickup_tile_intent(
    hovered_grid_position: Option<(i32, i32)>,
    entities: &EntityModelSet,
) -> Option<QuestUiIntent> {
    let hovered_grid_position = hovered_grid_position?;
    entities
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::SelfPlayer)
        .filter(|entity| (entity.x, entity.y) == hovered_grid_position)
        .map(|_| QuestUiIntent::PickUpTile)
}

fn movement_direction_toward(
    hovered_grid_position: Option<(i32, i32)>,
    origin: (i32, i32),
) -> Option<&'static str> {
    let (target_x, target_y) = hovered_grid_position?;
    match (
        (target_x - origin.0).signum(),
        (target_y - origin.1).signum(),
    ) {
        (0, -1) => Some("up"),
        (1, -1) => Some("upright"),
        (1, 0) => Some("right"),
        (1, 1) => Some("downright"),
        (0, 1) => Some("down"),
        (-1, 1) => Some("downleft"),
        (-1, 0) => Some("left"),
        (-1, -1) => Some("upleft"),
        _ => None,
    }
}

fn authoritative_player(entities: &EntityModelSet) -> Option<(String, (i32, i32), String)> {
    let player = entities
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::SelfPlayer)?;
    Some((
        player.object_id.clone(),
        (player.x, player.y),
        player
            .direction
            .clone()
            .unwrap_or_else(|| "down".to_owned()),
    ))
}

fn movement_target(origin: (i32, i32), direction: &str, distance: i32) -> (i32, i32) {
    let (dx, dy) = match direction.to_ascii_lowercase().as_str() {
        "up" => (0, -1),
        "upright" => (1, -1),
        "right" => (1, 0),
        "downright" => (1, 1),
        "down" => (0, 1),
        "downleft" => (-1, 1),
        "left" => (-1, 0),
        "upleft" => (-1, -1),
        _ => (0, 0),
    };
    (origin.0 + dx * distance, origin.1 + dy * distance)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlannedPointerMove {
    direction: &'static str,
    mode: WorldPointerMovementMode,
}

fn entity_blocks_movement(
    entities: &EntityModelSet,
    presentation: Option<&NativeEntityPresentation>,
    self_object_id: &str,
    point: (i32, i32),
) -> bool {
    if let Some(blocked) = presentation
        .and_then(|presentation| presentation.tile_has_blocking_entity(self_object_id, point))
    {
        return blocked;
    }
    entities
        .entities
        .iter()
        .any(|entity| entity.object_id != self_object_id && (entity.x, entity.y) == point)
}

fn movement_step_blocked(
    movement: &WorldPointerMovementState,
    entities: &EntityModelSet,
    presentation: Option<&NativeEntityPresentation>,
    self_object_id: &str,
    map_file_name: Option<&str>,
    origin: (i32, i32),
    direction: &'static str,
    mode: WorldPointerMovementMode,
) -> bool {
    if movement.step_was_rejected(origin, direction, mode) {
        return true;
    }
    let distance = if mode == WorldPointerMovementMode::Run {
        2
    } else {
        1
    };
    (1..=distance).any(|step| {
        let point = movement_target(origin, direction, step);
        entity_blocks_movement(entities, presentation, self_object_id, point)
            || map_file_name.is_some_and(|map_file_name| {
                crate::map_parser::map_cell_blocks_movement(map_file_name, point.0, point.1)
                    == Some(true)
            })
    })
}

/// Mirror Crystal's held-pointer movement choice: try the requested direct
/// step, degrade a blocked run to a walk, then steer one direction clockwise
/// or counter-clockwise around an occupied cell. The Zone still validates the
/// chosen intent and every correction is retained briefly as a route hint.
fn plan_crystal_pointer_move(
    movement: &mut WorldPointerMovementState,
    entities: &EntityModelSet,
    presentation: Option<&NativeEntityPresentation>,
    self_object_id: &str,
    map_file_name: Option<&str>,
    origin: (i32, i32),
    direction: &'static str,
    requested_mode: WorldPointerMovementMode,
    now_ms: f64,
) -> Option<PlannedPointerMove> {
    movement.prune_blocked_steps(now_ms);
    if !movement_step_blocked(
        movement,
        entities,
        presentation,
        self_object_id,
        map_file_name,
        origin,
        direction,
        requested_mode,
    ) {
        return Some(PlannedPointerMove {
            direction,
            mode: requested_mode,
        });
    }

    if requested_mode == WorldPointerMovementMode::Run
        && !movement_step_blocked(
            movement,
            entities,
            presentation,
            self_object_id,
            map_file_name,
            origin,
            direction,
            WorldPointerMovementMode::Walk,
        )
    {
        return Some(PlannedPointerMove {
            direction,
            mode: WorldPointerMovementMode::Walk,
        });
    }

    for alternate in [
        rotate_direction(direction, 1),
        rotate_direction(direction, -1),
    ] {
        let Some(alternate) = alternate else {
            continue;
        };
        if !movement_step_blocked(
            movement,
            entities,
            presentation,
            self_object_id,
            map_file_name,
            origin,
            alternate,
            WorldPointerMovementMode::Walk,
        ) {
            return Some(PlannedPointerMove {
                direction: alternate,
                mode: WorldPointerMovementMode::Walk,
            });
        }
    }
    None
}

fn tile_distance(left: (i32, i32), right: (i32, i32)) -> i32 {
    (left.0 - right.0).abs().max((left.1 - right.1).abs())
}

fn npc_position(entities: &EntityModelSet, object_id: u32) -> Option<(i32, i32)> {
    let object_id = object_id.to_string();
    entities
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::Npc && entity.object_id == object_id)
        .map(|entity| (entity.x, entity.y))
}

fn new_move_draw_offset(cursor_stage: (f32, f32)) -> (f32, f32) {
    (
        cursor_stage.0.rem_euclid(CELL_WIDTH) - 8.0,
        cursor_stage.1.rem_euclid(CELL_HEIGHT) - 15.0,
    )
}

#[allow(clippy::too_many_arguments)]
fn send_pointer_move(
    commands: &GatewayCommands,
    presentation: &mut NativeEntityPresentation,
    movement: &mut WorldPointerMovementState,
    object_id: &str,
    origin: (i32, i32),
    direction: &'static str,
    requested_mode: WorldPointerMovementMode,
    now_ms: f64,
    animation_now_ms: u64,
) -> bool {
    let effective_mode = movement.effective_mode(requested_mode, now_ms);
    let distance = if effective_mode == WorldPointerMovementMode::Run {
        2
    } else {
        1
    };
    let pending = PendingSelfMove {
        from: origin,
        to: movement_target(origin, direction, distance),
        direction,
        mode: effective_mode,
        sent_at_ms: now_ms,
        visual_until_ms: now_ms + CRYSTAL_MOVE_PRESENTATION_MS,
    };
    let intent = match requested_mode {
        WorldPointerMovementMode::Walk => PlayerIntent::Walk {
            direction: direction.to_owned(),
        },
        WorldPointerMovementMode::Run => PlayerIntent::Run {
            direction: direction.to_owned(),
        },
    };
    if !commands.send(intent) {
        return false;
    }
    let _ = presentation.begin_local_self_motion(
        object_id,
        pending.from,
        pending.to,
        direction,
        effective_mode == WorldPointerMovementMode::Run,
        animation_now_ms,
        crate::entity_presentation::native_motion_clock_ms(),
    );
    push_movement_shadow_command(now_ms, &pending);
    movement.pending = Some(pending);
    true
}

fn crystal_direction_name(direction: &str) -> &'static str {
    match direction.to_ascii_lowercase().as_str() {
        "up" => "Up",
        "upright" => "UpRight",
        "right" => "Right",
        "downright" => "DownRight",
        "downleft" => "DownLeft",
        "left" => "Left",
        "upleft" => "UpLeft",
        _ => "Down",
    }
}

fn push_movement_shadow(value: serde_json::Value) {
    mir2_bevy_runtime::push_mir2_movement_shadow_event(value.to_string());
}

fn push_movement_shadow_reset(at_ms: f64, object_id: &str, position: (i32, i32), direction: &str) {
    push_movement_shadow(serde_json::json!({
        "type": "reset",
        "atMs": at_ms,
        "objectId": object_id,
        "x": position.0,
        "y": position.1,
        "direction": crystal_direction_name(direction),
    }));
}

fn push_movement_shadow_command(at_ms: f64, pending: &PendingSelfMove) {
    push_movement_shadow(serde_json::json!({
        "type": "commandSent",
        "atMs": at_ms,
        "direction": crystal_direction_name(pending.direction),
        "mode": match pending.mode {
            WorldPointerMovementMode::Walk => "walk",
            WorldPointerMovementMode::Run => "run",
        },
        "fromX": pending.from.0,
        "fromY": pending.from.1,
        "toX": pending.to.0,
        "toY": pending.to.1,
        "phaseCount": 6,
    }));
}

fn push_movement_shadow_authoritative(
    at_ms: f64,
    ack: &NativeSelfMovementAck,
    predicted: Option<(i32, i32)>,
    outcome: MovementAckOutcome,
) {
    let disposition = match outcome {
        MovementAckOutcome::Confirmed | MovementAckOutcome::Degraded => "confirmed",
        MovementAckOutcome::Correction => "correction",
        MovementAckOutcome::Accepted => "accepted",
    };
    push_movement_shadow(serde_json::json!({
        "type": "authoritative",
        "atMs": at_ms,
        "packet": ack.packet,
        "objectId": ack.object_id,
        "isSelf": true,
        "x": ack.x,
        "y": ack.y,
        "direction": crystal_direction_name(&ack.direction),
        "tsPredictedX": predicted.map(|point| point.0),
        "tsPredictedY": predicted.map(|point| point.1),
        "tsDisposition": disposition,
    }));
}

/// Convert Crystal world mouse input into bounded intents. Left click keeps the
/// pixel-tested combat/NPC/pickup priorities and walks while empty world stays
/// held. Right click runs while empty world stays held. The shared Zone remains
/// authoritative and may degrade the first movement from standstill to a walk.
pub fn mouse_world_interaction_system(
    mouse: Res<ButtonInput<MouseButton>>,
    shell: Option<Res<NativeShellModel>>,
    player_ui: Option<Res<NativePlayerUiState>>,
    dialog: Option<Res<NpcDialogModel>>,
    ui_read_model: Option<Res<UiReadModel>>,
    entities: Option<Res<EntityModelSet>>,
    mut presentation: Option<ResMut<NativeEntityPresentation>>,
    time: Option<Res<Time>>,
    windows: Query<&Window>,
    mut queue: Option<ResMut<QuestUiIntentQueue>>,
    commands: Option<Res<GatewayCommands>>,
    mut effects: Option<ResMut<NativeEffects>>,
    gameplay_inbox: Option<Res<GameplayEventInbox>>,
    mut movement: ResMut<WorldPointerMovementState>,
) {
    let left_pressed = mouse.just_pressed(MouseButton::Left);
    let right_pressed = mouse.just_pressed(MouseButton::Right);
    let ack_waiting = gameplay_inbox
        .as_deref()
        .is_some_and(GameplayEventInbox::has_movement_acks);
    if !left_pressed
        && !right_pressed
        && movement.active.is_none()
        && movement.pending.is_none()
        && movement.pending_npc_interaction.is_none()
        && !ack_waiting
    {
        return;
    }
    let now_ms = time
        .as_deref()
        .map(|time| time.elapsed_secs_f64() * 1_000.0)
        .unwrap_or(0.0);
    let animation_now_ms = now_ms.clamp(0.0, u64::MAX as f64) as u64;
    let (Some(shell), Some(entities), Some(presentation)) =
        (shell, entities, presentation.as_deref_mut())
    else {
        push_movement_shadow(serde_json::json!({"type": "clear", "atMs": now_ms}));
        movement.reset_controller();
        return;
    };
    let Ok(window) = windows.single() else {
        movement.stop_hold();
        return;
    };
    if shell.screen != NativeShellScreen::InGame {
        push_movement_shadow(serde_json::json!({"type": "clear", "atMs": now_ms}));
        movement.reset_controller();
        if let Some(inbox) = gameplay_inbox.as_deref() {
            inbox.clear_movement_acks();
        }
        return;
    }

    let Some((object_id, entity_position, entity_direction)) = authoritative_player(&entities)
    else {
        if movement.self_object_id.is_some() || movement.pending.is_some() {
            push_movement_shadow(serde_json::json!({"type": "clear", "atMs": now_ms}));
            movement.reset_controller();
            if let Some(inbox) = gameplay_inbox.as_deref() {
                inbox.clear_movement_acks();
            }
        } else {
            movement.stop_hold();
        }
        return;
    };
    if movement.observe_identity(&object_id, entity_position, entity_direction.as_str()) {
        push_movement_shadow_reset(
            now_ms,
            &object_id,
            entity_position,
            entity_direction.as_str(),
        );
    }

    let mut packet_ack_observed = false;
    if let Some(inbox) = gameplay_inbox.as_deref() {
        for ack in inbox.drain_movement_acks() {
            if ack.object_id != object_id {
                continue;
            }
            let predicted = movement.pending.as_ref().map(|pending| pending.to);
            let outcome = movement.reconcile_ack(&ack, now_ms);
            push_movement_shadow_authoritative(now_ms, &ack, predicted, outcome);
            if matches!(
                outcome,
                MovementAckOutcome::Degraded | MovementAckOutcome::Correction
            ) {
                presentation.cancel_local_self_prediction(
                    &object_id,
                    (ack.x, ack.y),
                    &ack.direction,
                );
            }
            packet_ack_observed = true;
        }
    }
    let snapshot_reconciles_pending = movement.pending.as_ref().is_some_and(|pending| {
        entity_position == pending.to
            || (pending.mode == WorldPointerMovementMode::Run
                && entity_position == movement_target(pending.from, pending.direction, 1))
    });
    if !packet_ack_observed
        && (movement.pending.is_none() || snapshot_reconciles_pending)
        && movement.authoritative_position != Some(entity_position)
        && movement
            .last_packet_ack_at_ms
            .is_none_or(|at_ms| now_ms >= at_ms + 250.0)
    {
        let predicted = movement.pending.as_ref().map(|pending| pending.to);
        let snapshot = NativeSelfMovementAck {
            packet: "worldSnapshot".to_owned(),
            object_id: object_id.clone(),
            x: entity_position.0,
            y: entity_position.1,
            direction: entity_direction.clone(),
        };
        let outcome = movement.reconcile_ack(&snapshot, now_ms);
        push_movement_shadow_authoritative(now_ms, &snapshot, predicted, outcome);
        if matches!(
            outcome,
            MovementAckOutcome::Degraded | MovementAckOutcome::Correction
        ) {
            presentation.cancel_local_self_prediction(
                &object_id,
                entity_position,
                &entity_direction,
            );
        }
    }

    if movement
        .pending
        .as_ref()
        .is_some_and(|pending| now_ms >= pending.sent_at_ms + MOVEMENT_PENDING_MAX_AGE_MS)
    {
        let predicted = movement.pending.as_ref().map(|pending| pending.to);
        movement.pending = None;
        movement.run_primed_until_ms = 0.0;
        movement.input_blocked_until_ms = now_ms + CRYSTAL_CORRECTION_BLOCK_MS;
        movement.next_move_send_at_ms = movement.input_blocked_until_ms;
        let position = movement.authoritative_position.unwrap_or(entity_position);
        let direction = movement
            .authoritative_direction
            .clone()
            .unwrap_or_else(|| entity_direction.clone());
        presentation.cancel_local_self_prediction(&object_id, position, &direction);
        let timeout = NativeSelfMovementAck {
            packet: "MovementTimeout".to_owned(),
            object_id: object_id.clone(),
            x: position.0,
            y: position.1,
            direction,
        };
        push_movement_shadow_authoritative(
            now_ms,
            &timeout,
            predicted,
            MovementAckOutcome::Correction,
        );
    }

    if !window.focused {
        movement.stop_hold();
        movement.cancel_npc_approach();
        return;
    }

    let dialog_open = dialog.as_deref().is_some_and(|dialog| dialog.is_open);
    let dead = ui_read_model
        .as_deref()
        .is_some_and(|model| model.player.max_hp > 0 && model.player.hp <= 0);
    if is_world_click_blocked(player_ui.as_deref(), dialog_open, dead) {
        movement.stop_hold();
        movement.cancel_npc_approach();
        return;
    }

    if right_pressed {
        movement.cancel_npc_approach();
        // Crystal reserves right-click object interactions (for example Ctrl+
        // inspect). Until those are implemented, never turn an object click
        // into movement through the actor beneath the pointer.
        if presentation.hovered_object_id().is_some() {
            movement.stop_hold();
            return;
        }
        let origin = movement.authoritative_position.unwrap_or(entity_position);
        if let (Some(target), Some(cursor_stage), Some(effects)) = (
            presentation.hovered_grid_position(),
            presentation.hover_cursor_stage(),
            effects.as_deref_mut(),
        ) {
            if target != origin {
                let _ = effects.start_new_move_destination(
                    animation_now_ms,
                    target,
                    new_move_draw_offset(cursor_stage),
                );
            }
        }
        movement.begin(WorldPointerMovementMode::Run);
    } else if left_pressed {
        if let Some(intent) = hovered_world_intent(presentation.hovered_object_id(), &entities)
            .or_else(|| pickup_tile_intent(presentation.hovered_grid_position(), &entities))
        {
            movement.stop_hold();
            if let QuestUiIntent::InteractNpc { npc_object_id } = intent {
                let origin = movement.authoritative_position.unwrap_or(entity_position);
                if npc_position(&entities, npc_object_id)
                    .is_some_and(|target| tile_distance(origin, target) > 1)
                {
                    movement.pending_npc_interaction = Some(PendingNpcInteraction {
                        object_id: npc_object_id,
                        started_at_ms: now_ms,
                    });
                } else {
                    movement.cancel_npc_approach();
                    if let Some(queue) = queue.as_deref_mut() {
                        queue.push_intent(QuestUiIntent::InteractNpc { npc_object_id });
                    }
                    return;
                }
            } else {
                movement.cancel_npc_approach();
                if let Some(queue) = queue.as_deref_mut() {
                    queue.push_intent(intent);
                }
                return;
            }
        }

        // A player or another non-interactable actor is still solid world
        // content. Do not reinterpret that pixel hit as movement through it.
        if presentation.hovered_object_id().is_some() {
            movement.stop_hold();
            if movement.pending_npc_interaction.is_none() {
                return;
            }
        } else {
            movement.cancel_npc_approach();
            movement.begin(WorldPointerMovementMode::Walk);
        }
    }

    // Match the Web client bridge: a distant NPC click becomes a bounded
    // authoritative approach, and the dialog request is emitted exactly once
    // after the player reaches an adjacent tile. Crystal's server does not
    // accept the Windows client's previous fire-and-forget request at range.
    if let Some(pending_npc) = movement.pending_npc_interaction.clone() {
        if dialog_open || now_ms >= pending_npc.started_at_ms + NPC_APPROACH_MAX_AGE_MS {
            movement.cancel_npc_approach();
            return;
        }
        let Some(target) = npc_position(&entities, pending_npc.object_id) else {
            movement.cancel_npc_approach();
            return;
        };
        let origin = movement.authoritative_position.unwrap_or(entity_position);
        let distance = tile_distance(origin, target);
        if distance <= 1 {
            movement.cancel_npc_approach();
            if let Some(queue) = queue.as_deref_mut() {
                queue.push_intent(QuestUiIntent::InteractNpc {
                    npc_object_id: pending_npc.object_id,
                });
            }
            return;
        }
        if !movement.can_send(now_ms) {
            return;
        }
        let (Some(direction), Some(commands)) = (
            movement_direction_toward(Some(target), origin),
            commands.as_deref(),
        ) else {
            return;
        };
        let requested_mode = if distance > 2 {
            WorldPointerMovementMode::Run
        } else {
            WorldPointerMovementMode::Walk
        };
        let map_file_name = presentation.current_map_file_name().map(ToOwned::to_owned);
        let Some(planned) = plan_crystal_pointer_move(
            &mut movement,
            &entities,
            Some(presentation),
            &object_id,
            map_file_name.as_deref(),
            origin,
            direction,
            requested_mode,
            now_ms,
        ) else {
            return;
        };
        let _ = send_pointer_move(
            commands,
            presentation,
            &mut movement,
            &object_id,
            origin,
            planned.direction,
            planned.mode,
            now_ms,
            animation_now_ms,
        );
        return;
    }

    let Some(mode) = movement.active else {
        return;
    };
    let held = match mode {
        WorldPointerMovementMode::Walk => mouse.pressed(MouseButton::Left),
        WorldPointerMovementMode::Run => mouse.pressed(MouseButton::Right),
    };
    if !held {
        movement.stop_hold();
        return;
    }
    if presentation.hovered_object_id().is_some() {
        movement.stop_hold();
        return;
    }

    if !movement.can_send(now_ms) {
        return;
    }
    let origin = movement.authoritative_position.unwrap_or(entity_position);
    let (Some(direction), Some(commands)) = (
        movement_direction_toward(presentation.hovered_grid_position(), origin),
        commands.as_deref(),
    ) else {
        return;
    };
    let map_file_name = presentation.current_map_file_name().map(ToOwned::to_owned);
    let Some(planned) = plan_crystal_pointer_move(
        &mut movement,
        &entities,
        Some(presentation),
        &object_id,
        map_file_name.as_deref(),
        origin,
        direction,
        mode,
        now_ms,
    ) else {
        return;
    };
    let _ = send_pointer_move(
        commands,
        presentation,
        &mut movement,
        &object_id,
        origin,
        planned.direction,
        planned.mode,
        now_ms,
        animation_now_ms,
    );
}

/// Reject a stale Bevy `Interaction::Pressed` unless it is paired with the
/// physical left-button press edge from this frame. This prevents a world key
/// (notably D) from appearing to reopen the HUD control left under the cursor
/// after an Options/modal close, while preserving real mouse clicks.
pub fn sanitize_native_hud_pointer_input(
    mouse: Res<ButtonInput<MouseButton>>,
    shell: Option<Res<NativeShellModel>>,
    mut interactions: Query<&mut Interaction, With<CrystalHudAction>>,
) {
    let accept_press = shell
        .as_deref()
        .is_some_and(|shell| shell.screen == NativeShellScreen::InGame)
        && mouse.just_pressed(MouseButton::Left);
    if accept_press {
        return;
    }
    for mut interaction in &mut interactions {
        if *interaction == Interaction::Pressed {
            *interaction = Interaction::None;
        }
    }
}

/// Forward walk intents on WASD / arrow key presses.
pub fn keyboard_walk_system(
    keys: Res<ButtonInput<KeyCode>>,
    commands: Res<GatewayCommands>,
    shell: Option<Res<NativeShellModel>>,
    player_ui: Option<Res<NativePlayerUiState>>,
    windows: Query<&Window>,
) {
    if std::env::var_os("MIR2_NATIVE_TRACE_RENDER").is_some() {
        let pressed = walk_key_map()
            .into_iter()
            .filter_map(|(code, direction)| keys.just_pressed(code).then_some(direction))
            .collect::<Vec<_>>();
        if !pressed.is_empty() {
            eprintln!(
                "[native-input] walk keys={pressed:?} screen={:?}",
                shell.as_deref().map(|model| model.screen)
            );
        }
    }
    if !gameplay_input_enabled(shell.as_deref(), player_ui.as_deref(), &windows) {
        return;
    }
    if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        return;
    }
    for (code, direction) in walk_key_map() {
        if keys.just_pressed(code) {
            commands.send(PlayerIntent::Walk {
                direction: direction.to_owned(),
            });
        }
    }
}

/// Forward run intents on WASD / arrows while Shift is held.
pub fn keyboard_run_system(
    keys: Res<ButtonInput<KeyCode>>,
    commands: Res<GatewayCommands>,
    shell: Option<Res<NativeShellModel>>,
    player_ui: Option<Res<NativePlayerUiState>>,
    windows: Query<&Window>,
) {
    if !gameplay_input_enabled(shell.as_deref(), player_ui.as_deref(), &windows) {
        return;
    }
    if !keys.pressed(KeyCode::ShiftLeft) && !keys.pressed(KeyCode::ShiftRight) {
        return;
    }
    for (code, direction) in walk_key_map() {
        if keys.just_pressed(code) {
            commands.send(PlayerIntent::Run {
                direction: direction.to_owned(),
            });
        }
    }
}

/// Forward the E-key clockwise turn as an absolute intent derived from the
/// latest authoritative self facing. Q is reserved for the shared Quest Log
/// shortcut. The gateway protocol accepts an absolute direction, not a
/// relative turn sense.
pub fn keyboard_turn_system(
    keys: Res<ButtonInput<KeyCode>>,
    commands: Res<GatewayCommands>,
    entities: Res<EntityModelSet>,
    shell: Option<Res<NativeShellModel>>,
    player_ui: Option<Res<NativePlayerUiState>>,
    windows: Query<&Window>,
) {
    if !gameplay_input_enabled(shell.as_deref(), player_ui.as_deref(), &windows) {
        return;
    }
    let Some(current) = entities
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::SelfPlayer)
        .and_then(|entity| entity.direction.as_deref())
    else {
        return;
    };

    // Q is the cross-client Quest Log shortcut. It must never also emit a
    // world turn on the frame that the native quest UI opens or closes.
    let turn_delta = if keys.just_pressed(KeyCode::KeyE) {
        Some(1)
    } else {
        None
    };
    if let Some(delta) = turn_delta.and_then(|delta| rotate_direction(current, delta)) {
        commands.send(PlayerIntent::Turn {
            direction: delta.to_owned(),
        });
    }
}

/// Send TownRevive on V only while dead with a positive max HP.
pub fn keyboard_town_revive_system(
    keys: Res<ButtonInput<KeyCode>>,
    commands: Res<GatewayCommands>,
    shell: Option<Res<NativeShellModel>>,
    ui_read_model: Option<Res<UiReadModel>>,
    player_ui: Option<Res<NativePlayerUiState>>,
    windows: Query<&Window>,
) {
    if !gameplay_input_enabled(shell.as_deref(), player_ui.as_deref(), &windows) {
        return;
    }
    if !keys.just_pressed(KeyCode::KeyV) {
        return;
    }
    let Some(ui_read_model) = ui_read_model else {
        return;
    };
    if ui_read_model.player.hp <= 0 && ui_read_model.player.max_hp > 0 {
        commands.send_town_revive();
    }
}

/// Belt 1-6 uses the corresponding belt item. F1-F8 select the learned skill
/// assigned to that server-provided hotkey, falling back only when the server
/// omitted a hotkey. This function only emits a request: the server remains
/// the authority for damage, MP, cooldown, level and range.
pub fn keyboard_skill_system(
    keys: Res<ButtonInput<KeyCode>>,
    commands: Res<GatewayCommands>,
    shell: Option<Res<NativeShellModel>>,
    entities: Res<EntityModelSet>,
    combat_target: Option<Res<CombatTargetModel>>,
    ui_read_model: Option<Res<UiReadModel>>,
    skills: Option<Res<SkillModel>>,
    inventory: Option<Res<InventoryModel>>,
    player_ui: Option<Res<NativePlayerUiState>>,
    windows: Query<&Window>,
) {
    if !gameplay_input_enabled(shell.as_deref(), player_ui.as_deref(), &windows) {
        return;
    }

    let belt_slot = if keys.just_pressed(KeyCode::Digit1) {
        Some(0)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(1)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(2)
    } else if keys.just_pressed(KeyCode::Digit4) {
        Some(3)
    } else if keys.just_pressed(KeyCode::Digit5) {
        Some(4)
    } else if keys.just_pressed(KeyCode::Digit6) {
        Some(5)
    } else {
        None
    };
    if let Some(slot) = belt_slot {
        if inventory.is_some_and(|model| belt_slot_item(model.as_ref(), slot).is_some()) {
            commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::UseItem {
                key: None,
                unique_id: None,
                slot: Some(slot),
                grid: Some("belt".to_owned()),
            }));
        }
        return;
    }
    let skill_mode = player_ui.as_deref().map(|ui| ui.core.options.skill_mode);
    let Some(skill_slot) = skill_shortcut_slot(&keys, skill_mode) else {
        return;
    };
    let Some(skills) = skills.as_deref() else {
        return;
    };
    let Some(selection) = skills.selection_for_shortcut(skill_slot) else {
        return;
    };
    if selection.cast_kind.as_deref() == Some("passive") {
        return;
    }
    if selection.cooldown_remaining_ticks > 0 {
        return;
    }
    let Some(ui) = ui_read_model.as_deref() else {
        return;
    };
    if ui.player.hp <= 0 {
        return;
    }
    if selection
        .mp_cost
        .is_some_and(|mp_cost| ui.player.mp < i32::try_from(mp_cost).unwrap_or(i32::MAX))
    {
        return;
    }
    let Some(spell) = selection.spell.filter(|spell| !spell.trim().is_empty()) else {
        return;
    };
    if selection.cast_kind.as_deref() == Some("toggle") {
        commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::SpellToggle {
            spell,
            // `canUse` is the authoritative current toggle state. Unknown is
            // not passive; the safe first request is an explicit enable.
            toggle_state: if selection.can_use == Some(true) {
                0
            } else {
                1
            },
        }));
        return;
    }
    let player = entities
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::SelfPlayer);
    let direction = player
        .and_then(|entity| entity.direction.as_deref())
        .unwrap_or("down")
        .to_owned();
    let selected_target = combat_target
        .as_deref()
        .and_then(|model| model.target.as_ref())
        .and_then(|target| {
            entities
                .entities
                .iter()
                .find(|entity| entity.object_id == target.object_id.to_string())
                .map(|entity| (target.object_id, entity.x, entity.y))
        });
    let Some(player) = player else {
        return;
    };
    let (target_id, target_x, target_y, lock) = match selection.cast_kind.as_deref() {
        Some("direction") | Some("self") => (0, player.x, player.y, false),
        Some("ground") => selected_target
            .map(|(_, x, y)| (0, x, y, false))
            .unwrap_or_else(|| {
                let (dx, dy) = direction_to_delta(&direction);
                (0, player.x + dx, player.y + dy, false)
            }),
        _ => selected_target
            .map(|(id, x, y)| (id, x, y, true))
            .unwrap_or_else(|| {
                // No selected target: express a forward tile intent. The
                // server still validates whether this spell can use it.
                let (dx, dy) = direction_to_delta(&direction);
                (0, player.x + dx, player.y + dy, false)
            }),
    };
    commands.send_command(GatewayCommand::Wire(NativeOutboundCommand::Magic {
        object_id: 0,
        spell,
        direction,
        target_id,
        x: target_x,
        y: target_y,
        spell_target_lock: lock,
    }));
}

fn skill_shortcut_slot(keys: &ButtonInput<KeyCode>, skill_mode: Option<bool>) -> Option<u8> {
    // Crystal rewrites skill bindings when SkillMode changes: false is Ctrl,
    // true is tilde. Hosts that do not yet provide shared UI state keep the
    // legacy direct-F-key bridge instead of becoming unusable.
    let modifier_active = match skill_mode {
        Some(false) => keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight),
        Some(true) => keys.pressed(KeyCode::Backquote),
        None => true,
    };
    if !modifier_active {
        return None;
    }
    [
        KeyCode::F1,
        KeyCode::F2,
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F5,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
    ]
    .into_iter()
    .enumerate()
    .find_map(|(index, key)| keys.just_pressed(key).then_some(index as u8 + 1))
}

fn direction_to_delta(direction: &str) -> (i32, i32) {
    match direction.to_ascii_lowercase().as_str() {
        "up" => (0, -1),
        "upright" => (1, -1),
        "right" => (1, 0),
        "downright" => (1, 1),
        "down" => (0, 1),
        "downleft" => (-1, 1),
        "left" => (-1, 0),
        "upleft" => (-1, -1),
        _ => (0, 0),
    }
}

fn rotate_direction(current: &str, delta: i32) -> Option<&'static str> {
    const DIRECTIONS: [&str; 8] = [
        "up",
        "upright",
        "right",
        "downright",
        "down",
        "downleft",
        "left",
        "upleft",
    ];
    let current_index = DIRECTIONS
        .iter()
        .position(|direction| direction.eq_ignore_ascii_case(current))?
        as i32;
    let next_index = (current_index + delta).rem_euclid(DIRECTIONS.len() as i32) as usize;
    Some(DIRECTIONS[next_index])
}

/// The WASD / arrow 鈫?direction mapping shared by walk and run.
fn walk_key_map() -> [(KeyCode, &'static str); 8] {
    [
        (KeyCode::KeyW, UP),
        (KeyCode::KeyS, DOWN),
        (KeyCode::KeyA, LEFT),
        (KeyCode::KeyD, RIGHT),
        (KeyCode::ArrowUp, UP),
        (KeyCode::ArrowDown, DOWN),
        (KeyCode::ArrowLeft, LEFT),
        (KeyCode::ArrowRight, RIGHT),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_client_bevy::entities::{EntityKind, EntityModel, EntityModelSet};
    use mir2_client_bevy::read_model::UiReadModel;

    fn input_app() -> (
        bevy::prelude::App,
        std::sync::mpsc::Receiver<GatewayCommand>,
    ) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut app = bevy::prelude::App::new();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(GatewayCommands::new(sender));
        app.init_resource::<WorldPointerMovementState>();
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        });
        app.insert_resource(EntityModelSet {
            entities: vec![EntityModel {
                object_id: "self".to_owned(),
                kind: EntityKind::SelfPlayer,
                name: "Self".to_owned(),
                x: 0,
                y: 0,
                level: Some(1),
                direction: Some("up".to_owned()),
            }],
        });
        (app, receiver)
    }

    fn world_entities() -> EntityModelSet {
        EntityModelSet {
            entities: vec![
                EntityModel {
                    object_id: "1000".to_owned(),
                    kind: EntityKind::SelfPlayer,
                    name: "Self".to_owned(),
                    x: 10,
                    y: 10,
                    level: Some(7),
                    direction: Some("right".to_owned()),
                },
                EntityModel {
                    object_id: "77".to_owned(),
                    kind: EntityKind::Npc,
                    name: "Teleport Gilbert".to_owned(),
                    x: 11,
                    y: 10,
                    level: None,
                    direction: Some("left".to_owned()),
                },
                EntityModel {
                    object_id: "2001".to_owned(),
                    kind: EntityKind::Monster,
                    name: "Scarecrow".to_owned(),
                    x: 10,
                    y: 11,
                    level: Some(3),
                    direction: Some("up".to_owned()),
                },
            ],
        }
    }

    fn movement_entities() -> EntityModelSet {
        let mut entities = world_entities();
        for entity in &mut entities.entities {
            if entity.kind != EntityKind::SelfPlayer {
                entity.x += 100;
                entity.y += 100;
            }
        }
        entities
    }

    #[derive(Clone, Copy, Debug)]
    enum BlockedInputContext {
        Inventory,
        Options,
        DeleteConfirm,
        ChatFocus,
        Login,
        Unfocused,
    }

    #[derive(Clone, Copy, Debug)]
    enum WorldAction {
        Walk,
        Run,
        Turn,
        Revive,
        Skill,
    }

    fn install_blocked_context(app: &mut bevy::prelude::App, context: BlockedInputContext) {
        match context {
            BlockedInputContext::Inventory
            | BlockedInputContext::Options
            | BlockedInputContext::ChatFocus => {
                let mut ui = NativePlayerUiState::default();
                match context {
                    BlockedInputContext::Inventory => ui.toggle_inventory(),
                    BlockedInputContext::Options => ui.toggle_options(),
                    BlockedInputContext::ChatFocus => ui.core.chat_focused = true,
                    BlockedInputContext::DeleteConfirm
                    | BlockedInputContext::Login
                    | BlockedInputContext::Unfocused => unreachable!(),
                }
                app.insert_resource(ui);
            }
            BlockedInputContext::DeleteConfirm => {
                app.world_mut().resource_mut::<NativeShellModel>().screen =
                    NativeShellScreen::DeleteConfirm { index: 0 };
            }
            BlockedInputContext::Login => {
                app.world_mut().resource_mut::<NativeShellModel>().screen =
                    NativeShellScreen::Login;
            }
            BlockedInputContext::Unfocused => {
                app.world_mut().spawn(Window {
                    focused: false,
                    ..Default::default()
                });
            }
        }
    }

    fn install_world_action(app: &mut bevy::prelude::App, action: WorldAction) {
        match action {
            WorldAction::Walk => app.add_systems(bevy::prelude::Update, keyboard_walk_system),
            WorldAction::Run => app.add_systems(bevy::prelude::Update, keyboard_run_system),
            WorldAction::Turn => app.add_systems(bevy::prelude::Update, keyboard_turn_system),
            WorldAction::Revive => {
                app.add_systems(bevy::prelude::Update, keyboard_town_revive_system)
            }
            WorldAction::Skill => app.add_systems(bevy::prelude::Update, keyboard_skill_system),
        };
    }

    fn press_world_action(app: &mut bevy::prelude::App, action: WorldAction) {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        match action {
            WorldAction::Walk | WorldAction::Run => {
                keys.press(KeyCode::KeyW);
                if matches!(action, WorldAction::Run) {
                    keys.press(KeyCode::ShiftLeft);
                }
            }
            WorldAction::Turn => keys.press(KeyCode::KeyE),
            WorldAction::Revive => keys.press(KeyCode::KeyV),
            WorldAction::Skill => keys.press(KeyCode::F1),
        }
    }

    fn input_app_with_ui() -> (
        bevy::prelude::App,
        std::sync::mpsc::Receiver<GatewayCommand>,
    ) {
        let (mut app, receiver) = input_app();
        app.insert_resource(UiReadModel::default());
        (app, receiver)
    }

    fn install_movement_clock_and_inbox(app: &mut bevy::prelude::App) {
        app.insert_resource(bevy::prelude::Time::<()>::default());
        let (_sender, receiver) = std::sync::mpsc::channel();
        app.insert_resource(GameplayEventInbox::new(receiver));
    }

    fn advance_movement_clock(app: &mut bevy::prelude::App, millis: u64) {
        app.world_mut()
            .resource_mut::<bevy::prelude::Time>()
            .advance_by(std::time::Duration::from_millis(millis));
    }

    fn push_test_movement_ack(app: &bevy::prelude::App, x: i32, y: i32, direction: &str) {
        app.world()
            .resource::<GameplayEventInbox>()
            .push_movement_ack(NativeSelfMovementAck {
                packet: "UserLocation".to_owned(),
                object_id: "1000".to_owned(),
                x,
                y,
                direction: direction.to_owned(),
            });
    }

    #[test]
    fn gameplay_gate_matrix_blocks_every_registered_world_action() {
        let contexts = [
            BlockedInputContext::Inventory,
            BlockedInputContext::Options,
            BlockedInputContext::DeleteConfirm,
            BlockedInputContext::ChatFocus,
            BlockedInputContext::Login,
            BlockedInputContext::Unfocused,
        ];
        let actions = [
            WorldAction::Walk,
            WorldAction::Run,
            WorldAction::Turn,
            WorldAction::Revive,
            WorldAction::Skill,
        ];

        for context in contexts {
            for action in actions {
                let (mut app, receiver) = input_app();
                app.insert_resource(UiReadModel {
                    player: mir2_client_bevy::read_model::PlayerStats {
                        hp: if matches!(action, WorldAction::Revive) {
                            0
                        } else {
                            10
                        },
                        max_hp: 20,
                        ..Default::default()
                    },
                    ..Default::default()
                });
                install_blocked_context(&mut app, context);
                install_world_action(&mut app, action);
                press_world_action(&mut app, action);

                app.update();

                assert!(
                    !receiver.try_iter().any(|_| true),
                    "{context:?} leaked {action:?}"
                );
            }
        }
    }

    #[test]
    fn rendered_hover_identity_maps_only_authoritative_npcs_and_monsters() {
        let entities = world_entities();
        assert_eq!(
            hovered_world_intent(Some("77"), &entities),
            Some(QuestUiIntent::InteractNpc { npc_object_id: 77 })
        );
        assert_eq!(
            hovered_world_intent(Some("2001"), &entities),
            Some(QuestUiIntent::AttackTarget { object_id: 2001 })
        );
        assert_eq!(hovered_world_intent(Some("1000"), &entities), None);
        assert_eq!(hovered_world_intent(Some("9999"), &entities), None);
        assert_eq!(hovered_world_intent(Some("invalid"), &entities), None);
        assert_eq!(
            pickup_tile_intent(Some((10, 10)), &entities),
            Some(QuestUiIntent::PickUpTile)
        );
        assert_eq!(pickup_tile_intent(Some((11, 10)), &entities), None);
        assert_eq!(new_move_draw_offset((576.0, 352.0)), (-8.0, -15.0));
        assert_eq!(new_move_draw_offset((600.0, 368.0)), (16.0, 1.0));
    }

    #[test]
    fn crystal_pointer_plan_degrades_run_then_steers_around_occupied_first_tile() {
        let mut entities = movement_entities();
        entities.entities.push(EntityModel {
            object_id: "blocker".to_owned(),
            kind: EntityKind::Monster,
            name: "Blocker".to_owned(),
            x: 12,
            y: 10,
            level: Some(1),
            direction: Some("left".to_owned()),
        });
        let mut movement = WorldPointerMovementState::default();

        assert_eq!(
            plan_crystal_pointer_move(
                &mut movement,
                &entities,
                None,
                "1000",
                None,
                (10, 10),
                "right",
                WorldPointerMovementMode::Run,
                0.0,
            ),
            Some(PlannedPointerMove {
                direction: "right",
                mode: WorldPointerMovementMode::Walk,
            })
        );

        entities.entities.push(EntityModel {
            object_id: "near-blocker".to_owned(),
            kind: EntityKind::Npc,
            name: "Near blocker".to_owned(),
            x: 11,
            y: 10,
            level: None,
            direction: Some("left".to_owned()),
        });
        assert_eq!(
            plan_crystal_pointer_move(
                &mut movement,
                &entities,
                None,
                "1000",
                None,
                (10, 10),
                "right",
                WorldPointerMovementMode::Run,
                1.0,
            ),
            Some(PlannedPointerMove {
                direction: "downright",
                mode: WorldPointerMovementMode::Walk,
            })
        );
    }

    #[test]
    fn real_left_click_queues_hovered_npc_or_monster_but_modal_blocks_click_through() {
        fn click_app(dialog_open: bool, hovered_object_id: &str) -> bevy::prelude::App {
            let mut app = bevy::prelude::App::new();
            app.world_mut().spawn(Window::default());
            app.insert_resource(ButtonInput::<MouseButton>::default());
            app.insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            });
            app.insert_resource(NativePlayerUiState::default());
            app.insert_resource(NpcDialogModel {
                is_open: dialog_open,
                ..Default::default()
            });
            app.insert_resource(UiReadModel::default());
            app.insert_resource(world_entities());
            let mut presentation = NativeEntityPresentation::default();
            presentation.set_hovered_object_id_for_test(Some(hovered_object_id));
            app.insert_resource(presentation);
            app.init_resource::<QuestUiIntentQueue>();
            app.init_resource::<WorldPointerMovementState>();
            app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app
        }

        let mut npc = click_app(false, "77");
        npc.update();
        assert_eq!(
            npc.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::InteractNpc { npc_object_id: 77 }]
        );

        let mut monster = click_app(false, "2001");
        monster.update();
        assert_eq!(
            monster
                .world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::AttackTarget { object_id: 2001 }]
        );

        let mut blocked = click_app(true, "2001");
        blocked.update();
        assert!(blocked
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
    }

    #[test]
    fn distant_npc_click_approaches_authoritatively_then_interacts_once() {
        let (mut app, receiver) = input_app();
        install_movement_clock_and_inbox(&mut app);
        app.world_mut().spawn(Window::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(UiReadModel::default());
        let mut entities = world_entities();
        entities
            .entities
            .iter_mut()
            .find(|entity| entity.object_id == "77")
            .expect("npc")
            .x = 15;
        app.insert_resource(entities);
        let mut presentation = NativeEntityPresentation::default();
        presentation.set_hovered_object_id_for_test(Some("77"));
        app.insert_resource(presentation);
        app.init_resource::<QuestUiIntentQueue>();
        app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);

        app.update();

        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Run { direction })) if direction == "right"
        ));
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<WorldPointerMovementState>()
                .pending_npc_interaction
                .as_ref()
                .map(|pending| pending.object_id),
            Some(77)
        );

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        app.world_mut()
            .resource_mut::<EntityModelSet>()
            .entities
            .iter_mut()
            .find(|entity| entity.kind == EntityKind::SelfPlayer)
            .expect("self")
            .x = 14;
        push_test_movement_ack(&app, 14, 10, "right");
        advance_movement_clock(&mut app, 600);
        app.update();

        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::InteractNpc { npc_object_id: 77 }]
        );
        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .pending_npc_interaction
            .is_none());

        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
    }

    #[test]
    fn npc_disappearance_cancels_pending_approach_without_fake_dialog_request() {
        let (mut app, receiver) = input_app();
        install_movement_clock_and_inbox(&mut app);
        app.world_mut().spawn(Window::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(UiReadModel::default());
        let mut entities = world_entities();
        entities
            .entities
            .iter_mut()
            .find(|entity| entity.object_id == "77")
            .expect("npc")
            .x = 15;
        app.insert_resource(entities);
        let mut presentation = NativeEntityPresentation::default();
        presentation.set_hovered_object_id_for_test(Some("77"));
        app.insert_resource(presentation);
        app.init_resource::<QuestUiIntentQueue>();
        app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(receiver.try_recv().is_ok(), "approach did not start");

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        app.world_mut()
            .resource_mut::<EntityModelSet>()
            .entities
            .retain(|entity| entity.object_id != "77");
        app.update();

        assert!(app
            .world()
            .resource::<WorldPointerMovementState>()
            .pending_npc_interaction
            .is_none());
        assert!(app
            .world_mut()
            .resource_mut::<QuestUiIntentQueue>()
            .drain_intents()
            .is_empty());
    }

    #[test]
    fn real_left_click_on_self_tile_queues_crystal_tile_pickup() {
        let mut app = bevy::prelude::App::new();
        app.world_mut().spawn(Window::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..Default::default()
        });
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(UiReadModel::default());
        app.insert_resource(world_entities());
        let mut presentation = NativeEntityPresentation::default();
        presentation.set_hovered_object_id_for_test(None);
        presentation.set_hover_grid_context_for_test((10, 10), (512.0, 368.0));
        app.insert_resource(presentation);
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<WorldPointerMovementState>();
        app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);

        app.update();

        assert_eq!(
            app.world_mut()
                .resource_mut::<QuestUiIntentQueue>()
                .drain_intents(),
            vec![QuestUiIntent::PickUpTile]
        );
    }

    #[test]
    fn real_right_click_on_empty_world_sends_authoritative_run_intent() {
        let (mut app, receiver) = input_app();
        app.world_mut().spawn(Window::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(UiReadModel::default());
        app.insert_resource(movement_entities());
        let mut presentation = NativeEntityPresentation::default();
        presentation.set_hover_grid_context_for_test((10, 10), (576.0, 352.0));
        app.insert_resource(presentation);
        app.init_resource::<QuestUiIntentQueue>();
        app.init_resource::<NativeEffects>();
        app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);

        app.update();

        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Run { direction })) if direction == "right"
        ));
        let marker = app
            .world_mut()
            .resource_mut::<NativeEffects>()
            .tick(0)
            .expect("right-click destination marker");
        assert!(marker.contains("/original-effects/Magic3/500.png"));
        {
            let state = app.world().resource::<WorldPointerMovementState>();
            let pending = state.pending.as_ref().expect("first run pending");
            assert_eq!(pending.mode, WorldPointerMovementMode::Walk);
            assert_eq!(pending.from, (10, 10));
            assert_eq!(pending.to, (11, 10));
        }
    }

    #[test]
    fn left_hold_on_empty_world_walks_only_after_authoritative_progress() {
        let (mut app, receiver) = input_app();
        install_movement_clock_and_inbox(&mut app);
        app.world_mut().spawn(Window::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(UiReadModel::default());
        app.insert_resource(movement_entities());
        let mut presentation = NativeEntityPresentation::default();
        presentation.set_hover_grid_context_for_test((10, 10), (576.0, 352.0));
        app.insert_resource(presentation);
        app.init_resource::<QuestUiIntentQueue>();
        app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);

        app.update();
        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Walk { direction })) if direction == "right"
        ));

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        app.update();
        assert!(receiver.try_recv().is_err(), "held walk flooded before ack");

        app.world_mut()
            .resource_mut::<EntityModelSet>()
            .entities
            .iter_mut()
            .find(|entity| entity.kind == EntityKind::SelfPlayer)
            .expect("self player")
            .x = 11;
        push_test_movement_ack(&app, 11, 10, "right");
        advance_movement_clock(&mut app, 600);
        app.update();
        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Walk { direction })) if direction == "right"
        ));

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(receiver.try_recv().is_err(), "released walk kept sending");
    }

    #[test]
    fn right_hold_repeats_run_only_after_authoritative_progress() {
        let (mut app, receiver) = input_app();
        install_movement_clock_and_inbox(&mut app);
        app.world_mut().spawn(Window::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(UiReadModel::default());
        app.insert_resource(movement_entities());
        let mut presentation = NativeEntityPresentation::default();
        presentation.set_hover_grid_context_for_test((10, 10), (576.0, 352.0));
        app.insert_resource(presentation);
        app.init_resource::<QuestUiIntentQueue>();
        app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);

        app.update();
        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Run { direction })) if direction == "right"
        ));

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Right);
        app.update();
        assert!(receiver.try_recv().is_err(), "held run flooded before ack");

        app.world_mut()
            .resource_mut::<EntityModelSet>()
            .entities
            .iter_mut()
            .find(|entity| entity.kind == EntityKind::SelfPlayer)
            .expect("self player")
            .x = 11;
        push_test_movement_ack(&app, 11, 10, "right");
        advance_movement_clock(&mut app, 600);
        app.update();
        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Run { direction })) if direction == "right"
        ));
        let state = app.world().resource::<WorldPointerMovementState>();
        let pending = state.pending.as_ref().expect("primed run pending");
        assert_eq!(pending.mode, WorldPointerMovementMode::Run);
        assert_eq!(pending.from, (11, 10));
        assert_eq!(pending.to, (13, 10));
    }

    #[test]
    fn authoritative_snapshot_progress_releases_pending_when_packet_ack_is_missing() {
        let (mut app, receiver) = input_app();
        install_movement_clock_and_inbox(&mut app);
        app.world_mut().spawn(Window::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(UiReadModel::default());
        app.insert_resource(movement_entities());
        let mut presentation = NativeEntityPresentation::default();
        presentation.set_hover_grid_context_for_test((10, 10), (576.0, 352.0));
        app.insert_resource(presentation);
        app.init_resource::<QuestUiIntentQueue>();
        app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);

        app.update();
        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Run { direction })) if direction == "right"
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Right);
        app.world_mut()
            .resource_mut::<EntityModelSet>()
            .entities
            .iter_mut()
            .find(|entity| entity.kind == EntityKind::SelfPlayer)
            .expect("self player")
            .x = 11;
        advance_movement_clock(&mut app, 600);

        app.update();

        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Run { direction })) if direction == "right"
        ));
        let state = app.world().resource::<WorldPointerMovementState>();
        let pending = state.pending.as_ref().expect("snapshot released next run");
        assert_eq!(pending.mode, WorldPointerMovementMode::Run);
        assert_eq!(pending.from, (11, 10));
        assert_eq!(pending.to, (13, 10));
    }

    #[test]
    fn missing_self_entity_clears_pending_movement_controller() {
        let (mut app, receiver) = input_app();
        install_movement_clock_and_inbox(&mut app);
        app.world_mut().spawn(Window::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(UiReadModel::default());
        app.insert_resource(movement_entities());
        let mut presentation = NativeEntityPresentation::default();
        presentation.set_hover_grid_context_for_test((10, 10), (576.0, 352.0));
        app.insert_resource(presentation);
        app.init_resource::<QuestUiIntentQueue>();
        app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);

        app.update();
        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Walk { direction })) if direction == "right"
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);
        app.world_mut()
            .resource_mut::<EntityModelSet>()
            .entities
            .retain(|entity| entity.kind != EntityKind::SelfPlayer);

        app.update();

        let state = app.world().resource::<WorldPointerMovementState>();
        assert!(state.pending.is_none());
        assert!(state.self_object_id.is_none());
        assert!(state.active.is_none());
    }

    #[test]
    fn unchanged_user_location_blocks_immediate_resend_then_steers_around_rejection() {
        let (mut app, receiver) = input_app();
        install_movement_clock_and_inbox(&mut app);
        app.world_mut().spawn(Window::default());
        app.insert_resource(ButtonInput::<MouseButton>::default());
        app.insert_resource(NativePlayerUiState::default());
        app.insert_resource(NpcDialogModel::default());
        app.insert_resource(UiReadModel::default());
        app.insert_resource(movement_entities());
        let mut presentation = NativeEntityPresentation::default();
        presentation.set_hover_grid_context_for_test((10, 10), (576.0, 352.0));
        app.insert_resource(presentation);
        app.init_resource::<QuestUiIntentQueue>();
        app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);

        app.update();
        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Run { direction })) if direction == "right"
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Right);
        push_test_movement_ack(&app, 10, 10, "right");
        advance_movement_clock(&mut app, 100);

        app.update();

        assert!(receiver.try_recv().is_err(), "correction resent movement");
        {
            let state = app.world().resource::<WorldPointerMovementState>();
            assert!(state.pending.is_none());
            assert_eq!(state.authoritative_position, Some((10, 10)));
            assert_eq!(state.run_primed_until_ms, 0.0);
            assert_eq!(state.input_blocked_until_ms, 500.0);
            assert!(!state.can_send(499.0));
            assert!(state.step_was_rejected((10, 10), "right", WorldPointerMovementMode::Walk));
        }

        advance_movement_clock(&mut app, 400);
        app.update();

        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Walk { direction }))
                if direction == "downright"
        ));
        let pending = app
            .world()
            .resource::<WorldPointerMovementState>()
            .pending
            .as_ref()
            .expect("Crystal alternate walk pending");
        assert_eq!(pending.from, (10, 10));
        assert_eq!(pending.to, (11, 11));
    }

    #[test]
    fn right_click_run_is_blocked_by_modal_or_hovered_actor() {
        let right_click_app = |dialog_open: bool, hovered_object_id: Option<&str>| {
            let (mut app, receiver) = input_app();
            app.world_mut().spawn(Window::default());
            app.insert_resource(ButtonInput::<MouseButton>::default());
            app.insert_resource(NativePlayerUiState::default());
            app.insert_resource(NpcDialogModel {
                is_open: dialog_open,
                ..Default::default()
            });
            app.insert_resource(UiReadModel::default());
            app.insert_resource(world_entities());
            let mut presentation = NativeEntityPresentation::default();
            presentation.set_hover_grid_context_for_test((10, 10), (528.0, 352.0));
            presentation.set_hovered_object_id_for_test(hovered_object_id);
            app.insert_resource(presentation);
            app.init_resource::<QuestUiIntentQueue>();
            app.init_resource::<WorldPointerMovementState>();
            app.add_systems(bevy::prelude::Update, mouse_world_interaction_system);
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Right);
            (app, receiver)
        };

        for (dialog_open, hovered_object_id) in [(true, None), (false, Some("2001"))] {
            let (mut app, receiver) = right_click_app(dialog_open, hovered_object_id);
            app.update();
            assert!(receiver.try_recv().is_err());
        }
    }

    #[test]
    fn closing_a_panel_restores_input_and_emits_only_once() {
        let (mut app, receiver) = input_app();
        app.insert_resource(NativePlayerUiState::default());
        app.add_systems(bevy::prelude::Update, keyboard_walk_system);

        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_inventory();
        press_world_action(&mut app, WorldAction::Walk);
        app.update();
        assert!(receiver.try_recv().is_err(), "open panel leaked walk");

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyW);
        app.update();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .close_windows();
        press_world_action(&mut app, WorldAction::Walk);
        app.update();

        let intents = receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            &intents[0],
            GatewayCommand::Player(PlayerIntent::Walk { direction }) if direction == "up"
        ));
    }

    #[test]
    fn world_and_pointer_gates_cover_modal_dialog_dead_and_capture_states() {
        let mut ui = NativePlayerUiState::default();
        let world_cases = [
            ("no modal", false, false, false),
            ("npc dialog", true, false, true),
            ("dead", false, true, true),
        ];
        for (label, dialog_open, dead, expected) in world_cases {
            assert_eq!(
                is_world_click_blocked(Some(&ui), dialog_open, dead),
                expected,
                "world gate case {label}"
            );
        }
        assert!(is_world_click_blocked(None, true, false));
        assert!(is_world_click_blocked(None, false, true));

        let pointer_cases = [
            ("no capture", None, false, false, false),
            ("inventory", Some("inventory"), false, false, false),
            ("options", Some("options"), false, false, false),
            ("chat focus", Some("chat"), false, false, false),
            ("drag window", None, true, false, false),
            ("drag scrollbar", None, false, true, false),
            ("button pressed", None, false, false, true),
        ];
        for (label, ui_capture, drag_window, drag_scrollbar, button_pressed) in pointer_cases {
            ui = NativePlayerUiState::default();
            match ui_capture {
                Some("inventory") => ui.toggle_inventory(),
                Some("options") => ui.toggle_options(),
                Some("chat") => ui.core.chat_focused = true,
                Some(other) => panic!("unknown capture fixture: {other}"),
                None => {}
            }
            let expected = ui_capture.is_some() || drag_window || drag_scrollbar || button_pressed;
            assert_eq!(
                is_pointer_captured_for_movement(
                    Some(&ui),
                    drag_window,
                    drag_scrollbar,
                    button_pressed,
                ),
                expected,
                "pointer gate case {label}"
            );
        }
    }

    #[test]
    fn shift_direction_emits_only_run() {
        let (mut app, receiver) = input_app();
        app.add_systems(
            bevy::prelude::Update,
            (keyboard_walk_system, keyboard_run_system),
        );
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ShiftLeft);
            keys.press(KeyCode::KeyW);
        }
        app.update();

        let intents = receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            &intents[0],
            GatewayCommand::Player(PlayerIntent::Run { direction }) if direction == "up"
        ));
    }

    #[test]
    fn e_rotates_current_facing_right_by_one_crystal_direction() {
        let (mut app, receiver) = input_app();
        app.add_systems(bevy::prelude::Update, keyboard_turn_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        app.update();

        let intent = receiver.try_recv().expect("turn intent");
        assert!(matches!(
            intent,
            GatewayCommand::Player(PlayerIntent::Turn { direction }) if direction == "upright"
        ));
    }

    #[test]
    fn q_is_reserved_for_quest_log_and_never_emits_a_world_turn() {
        let (mut app, receiver) = input_app();
        app.add_systems(bevy::prelude::Update, keyboard_turn_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.update();

        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn crystal_skill_mode_selects_ctrl_or_tilde_modifier() {
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::F1);
        assert_eq!(skill_shortcut_slot(&keys, Some(false)), None);
        assert_eq!(skill_shortcut_slot(&keys, Some(true)), None);
        assert_eq!(skill_shortcut_slot(&keys, None), Some(1));

        keys.press(KeyCode::ControlLeft);
        assert_eq!(skill_shortcut_slot(&keys, Some(false)), Some(1));
        assert_eq!(skill_shortcut_slot(&keys, Some(true)), None);

        keys.release(KeyCode::ControlLeft);
        keys.press(KeyCode::Backquote);
        assert_eq!(skill_shortcut_slot(&keys, Some(false)), None);
        assert_eq!(skill_shortcut_slot(&keys, Some(true)), Some(1));
    }

    #[test]
    fn session_transition_to_login_suppresses_live_d_movement_intent() {
        let (mut app, receiver) = input_app();
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::Login;
        app.insert_resource(shell);
        app.add_systems(bevy::prelude::Update, keyboard_walk_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyD);

        app.update();

        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn live_d_moves_right_ingame_without_mutating_ui_state() {
        let (mut app, receiver) = input_app();
        app.insert_resource(NativePlayerUiState::default());
        app.add_systems(bevy::prelude::Update, keyboard_walk_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyD);

        app.update();

        assert!(matches!(
            receiver.try_recv(),
            Ok(GatewayCommand::Player(PlayerIntent::Walk { direction })) if direction == "right"
        ));
        let ui = app.world().resource::<NativePlayerUiState>();
        assert!(!ui.options_open());
    }

    #[test]
    fn stale_hud_press_is_cleared_on_keyboard_frame_but_real_mouse_edge_is_preserved() {
        fn app_with_mouse(pressed: bool) -> (bevy::prelude::App, bevy::prelude::Entity) {
            let mut app = bevy::prelude::App::new();
            let mut mouse = ButtonInput::<MouseButton>::default();
            if pressed {
                mouse.press(MouseButton::Left);
            }
            app.insert_resource(mouse)
                .insert_resource(NativeShellModel {
                    screen: NativeShellScreen::InGame,
                    ..Default::default()
                })
                .add_systems(bevy::prelude::Update, sanitize_native_hud_pointer_input);
            let entity = app
                .world_mut()
                .spawn((CrystalHudAction::Option, Interaction::Pressed))
                .id();
            (app, entity)
        }

        let (mut stale, stale_button) = app_with_mouse(false);
        stale.update();
        assert_eq!(
            stale.world().get::<Interaction>(stale_button),
            Some(&Interaction::None)
        );

        let (mut real, real_button) = app_with_mouse(true);
        real.update();
        assert_eq!(
            real.world().get::<Interaction>(real_button),
            Some(&Interaction::Pressed)
        );
    }

    #[test]
    fn v_key_only_triggers_town_revive_when_dead_with_positive_max_hp() {
        let (mut app, receiver) = input_app_with_ui();
        let mut ui_read_model = app.world_mut().resource_mut::<UiReadModel>();
        ui_read_model.player.hp = 0;
        ui_read_model.player.max_hp = 100;
        drop(ui_read_model);
        app.add_systems(bevy::prelude::Update, keyboard_town_revive_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyV);

        app.update();

        let command = receiver.try_recv().expect("town revive command");
        assert!(matches!(
            command,
            GatewayCommand::Wire(NativeOutboundCommand::TownRevive)
        ));
    }

    #[test]
    fn v_key_never_triggers_town_revive_when_alive_or_unknown() {
        let (mut app, receiver) = input_app_with_ui();
        app.add_systems(bevy::prelude::Update, keyboard_town_revive_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyV);
        app.update();
        assert!(receiver.try_recv().is_err());

        let mut ui_read_model = app.world_mut().resource_mut::<UiReadModel>();
        ui_read_model.player.hp = 10;
        ui_read_model.player.max_hp = 100;
        drop(ui_read_model);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyV);
        app.update();
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn f1_selects_a_server_learned_skill_with_target_and_direction() {
        let (mut app, receiver) = input_app();
        // Insert combat target and UI so skill is allowed.
        app.insert_resource(mir2_client_bevy::quest_model::CombatTargetModel {
            target: Some(mir2_client_bevy::quest_model::CombatTarget {
                object_id: 2001,
                name: "Scarecrow".to_owned(),
                hp: 20,
                max_hp: 20,
                is_player: false,
            }),
        });
        app.world_mut()
            .resource_mut::<EntityModelSet>()
            .entities
            .push(mir2_client_bevy::entities::EntityModel {
                object_id: "2001".to_owned(),
                kind: EntityKind::Monster,
                name: "Scarecrow".to_owned(),
                x: 12,
                y: 10,
                level: Some(1),
                direction: Some("down".to_owned()),
            });
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        app.insert_resource(shell);
        app.insert_resource(UiReadModel {
            player: mir2_client_bevy::read_model::PlayerStats {
                hp: 10,
                max_hp: 20,
                ..Default::default()
            },
            ..Default::default()
        });
        app.insert_resource(SkillModel {
            skills: vec![mir2_client_bevy::skill_model::SkillEntry {
                id: 7,
                name: "FireBall".to_owned(),
                level: 2,
                key: Some("fireball".to_owned()),
                cooldown_ms: 1200,
                mp_cost: 0,
            }],
            bindings: vec![mir2_client_bevy::skill_model::SkillBinding {
                skill_id: 7,
                spell: Some("FireBall".to_owned()),
                hotkey: Some(1),
                cast_kind: Some("target".to_owned()),
                offensive: Some(true),
                ..Default::default()
            }],
        });
        app.add_systems(bevy::prelude::Update, keyboard_skill_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F1);
        app.update();
        let cmd = receiver.try_recv().expect("skill command");
        match cmd {
            GatewayCommand::Wire(NativeOutboundCommand::Magic {
                spell,
                direction,
                target_id,
                x,
                y,
                spell_target_lock,
                ..
            }) => {
                assert_eq!(spell, "FireBall");
                assert_eq!(direction, "up");
                assert_eq!(target_id, 2001);
                assert_eq!(x, 12);
                assert_eq!(y, 10);
                assert!(spell_target_lock);
            }
            other => panic!("unexpected command {other:?}"),
        }
    }

    #[test]
    fn name_only_skill_f1_does_not_emit_magic() {
        let (mut app, receiver) = input_app();
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        app.insert_resource(shell);
        app.insert_resource(UiReadModel {
            player: mir2_client_bevy::read_model::PlayerStats {
                hp: 10,
                max_hp: 20,
                mp: 30,
                max_mp: 30,
                ..Default::default()
            },
            ..Default::default()
        });
        app.insert_resource(SkillModel {
            skills: vec![mir2_client_bevy::skill_model::SkillEntry {
                id: 42,
                name: "Localized display name".to_owned(),
                level: 1,
                key: Some("display-key".to_owned()),
                cooldown_ms: 0,
                mp_cost: 0,
            }],
            bindings: vec![mir2_client_bevy::skill_model::SkillBinding {
                skill_id: 42,
                spell: None,
                hotkey: Some(1),
                cast_kind: Some("target".to_owned()),
                ..Default::default()
            }],
        });
        app.add_systems(bevy::prelude::Update, keyboard_skill_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F1);
        app.update();

        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn toggle_skill_f1_emits_the_next_authoritative_state() {
        for (can_use, expected_state) in [(None, 1), (Some(false), 1), (Some(true), 0)] {
            let (mut app, receiver) = input_app();
            let mut shell = NativeShellModel::default();
            shell.screen = NativeShellScreen::InGame;
            app.insert_resource(shell);
            app.insert_resource(UiReadModel {
                player: mir2_client_bevy::read_model::PlayerStats {
                    hp: 10,
                    max_hp: 20,
                    mp: 30,
                    max_mp: 30,
                    ..Default::default()
                },
                ..Default::default()
            });
            app.insert_resource(SkillModel {
                skills: vec![mir2_client_bevy::skill_model::SkillEntry {
                    id: 9,
                    name: "Localized sword display".to_owned(),
                    level: 1,
                    key: Some("flaming-sword-display-key".to_owned()),
                    cooldown_ms: 0,
                    mp_cost: 0,
                }],
                bindings: vec![mir2_client_bevy::skill_model::SkillBinding {
                    skill_id: 9,
                    spell: Some("FlamingSword".to_owned()),
                    hotkey: Some(1),
                    cast_kind: Some("toggle".to_owned()),
                    can_use,
                    ..Default::default()
                }],
            });
            app.add_systems(bevy::prelude::Update, keyboard_skill_system);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::F1);
            app.update();

            assert!(matches!(
                receiver.try_recv().expect("toggle command"),
                GatewayCommand::Wire(NativeOutboundCommand::SpellToggle {
                    spell,
                    toggle_state
                }) if spell == "FlamingSword" && toggle_state == expected_state
            ));
        }
    }

    #[test]
    fn passive_skill_f1_does_not_emit_magic_or_toggle() {
        let (mut app, receiver) = input_app();
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        app.insert_resource(shell);
        app.insert_resource(UiReadModel {
            player: mir2_client_bevy::read_model::PlayerStats {
                hp: 10,
                max_hp: 20,
                mp: 30,
                max_mp: 30,
                ..Default::default()
            },
            ..Default::default()
        });
        app.insert_resource(SkillModel {
            skills: vec![mir2_client_bevy::skill_model::SkillEntry {
                id: 10,
                name: "Passive display".to_owned(),
                level: 1,
                key: Some("passive-display-key".to_owned()),
                cooldown_ms: 0,
                mp_cost: 0,
            }],
            bindings: vec![mir2_client_bevy::skill_model::SkillBinding {
                skill_id: 10,
                spell: Some("PassiveSpell".to_owned()),
                hotkey: Some(1),
                cast_kind: Some("passive".to_owned()),
                ..Default::default()
            }],
        });
        app.add_systems(bevy::prelude::Update, keyboard_skill_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F1);
        app.update();
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn f2_prefers_explicit_server_hotkey_over_learned_order() {
        let (mut app, receiver) = input_app();
        app.insert_resource(UiReadModel {
            player: mir2_client_bevy::read_model::PlayerStats {
                hp: 10,
                max_hp: 20,
                mp: 30,
                max_mp: 30,
                ..Default::default()
            },
            ..Default::default()
        });
        app.insert_resource(SkillModel {
            skills: vec![
                mir2_client_bevy::skill_model::SkillEntry {
                    id: 1,
                    name: "FireBall".to_owned(),
                    level: 1,
                    key: Some("fireball".to_owned()),
                    cooldown_ms: 1000,
                    mp_cost: 1,
                },
                mir2_client_bevy::skill_model::SkillEntry {
                    id: 2,
                    name: "Lightning".to_owned(),
                    level: 1,
                    key: Some("lightning".to_owned()),
                    cooldown_ms: 1000,
                    mp_cost: 1,
                },
            ],
            bindings: vec![
                mir2_client_bevy::skill_model::SkillBinding {
                    skill_id: 1,
                    spell: Some("FireBall".to_owned()),
                    hotkey: Some(8),
                    cast_kind: Some("target".to_owned()),
                    ..Default::default()
                },
                mir2_client_bevy::skill_model::SkillBinding {
                    skill_id: 2,
                    spell: Some("Lightning".to_owned()),
                    hotkey: Some(2),
                    cast_kind: Some("target".to_owned()),
                    ..Default::default()
                },
            ],
        });
        app.add_systems(bevy::prelude::Update, keyboard_skill_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F2);
        app.update();
        match receiver.try_recv().expect("explicitly bound skill") {
            GatewayCommand::Wire(NativeOutboundCommand::Magic { spell, .. }) => {
                assert_eq!(spell, "Lightning");
            }
            other => panic!("unexpected command {other:?}"),
        }
    }

    #[test]
    fn unknown_cooldown_mp_and_unlearned_skill_never_emit_magic() {
        let cases = [
            (SkillModel::default(), 30, 30),
            (
                SkillModel {
                    skills: vec![mir2_client_bevy::skill_model::SkillEntry {
                        id: 1,
                        name: "FireBall".to_owned(),
                        level: 1,
                        key: Some("fireball".to_owned()),
                        cooldown_ms: 1000,
                        mp_cost: 9,
                    }],
                    bindings: vec![mir2_client_bevy::skill_model::SkillBinding {
                        skill_id: 1,
                        spell: Some("FireBall".to_owned()),
                        hotkey: Some(1),
                        cast_kind: Some("target".to_owned()),
                        cooldown_remaining_ticks: 2,
                        ..Default::default()
                    }],
                },
                30,
                30,
            ),
            (
                SkillModel {
                    skills: vec![mir2_client_bevy::skill_model::SkillEntry {
                        id: 1,
                        name: "FireBall".to_owned(),
                        level: 1,
                        key: Some("fireball".to_owned()),
                        cooldown_ms: 1000,
                        mp_cost: 9,
                    }],
                    bindings: vec![mir2_client_bevy::skill_model::SkillBinding {
                        skill_id: 1,
                        spell: Some("FireBall".to_owned()),
                        hotkey: Some(1),
                        cast_kind: Some("target".to_owned()),
                        mp_cost: Some(9),
                        ..Default::default()
                    }],
                },
                3,
                30,
            ),
        ];
        for (skills, mp, max_mp) in cases {
            let (mut app, receiver) = input_app();
            app.insert_resource(UiReadModel {
                player: mir2_client_bevy::read_model::PlayerStats {
                    hp: 10,
                    max_hp: 20,
                    mp,
                    max_mp,
                    ..Default::default()
                },
                ..Default::default()
            });
            app.insert_resource(skills);
            app.add_systems(bevy::prelude::Update, keyboard_skill_system);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::F1);
            app.update();
            assert!(receiver.try_recv().is_err());
        }
    }

    #[test]
    fn skill_input_suppressed_when_dead_or_not_ingame() {
        let (mut app, receiver) = input_app();
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::Login;
        app.insert_resource(shell);
        app.add_systems(bevy::prelude::Update, keyboard_skill_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F1);
        app.update();
        assert!(receiver.try_recv().is_err());
        // InGame but dead.
        let (mut app2, receiver2) = input_app();
        let mut shell2 = NativeShellModel::default();
        shell2.screen = NativeShellScreen::InGame;
        app2.insert_resource(shell2);
        app2.insert_resource(UiReadModel {
            player: mir2_client_bevy::read_model::PlayerStats {
                hp: 0,
                max_hp: 20,
                ..Default::default()
            },
            ..Default::default()
        });
        app2.add_systems(bevy::prelude::Update, keyboard_skill_system);
        app2.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F1);
        app2.update();
        assert!(receiver2.try_recv().is_err());
    }

    #[test]
    fn digit1_uses_occupied_belt_slot() {
        let (mut app, receiver) = input_app();
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        app.insert_resource(shell);
        app.insert_resource(InventoryModel {
            gold: 0,
            items: vec![mir2_client_bevy::inventory::ItemModel {
                unique_id: Some(1),
                key: "potion".to_owned(),
                name: "Small HP Potion".to_owned(),
                quantity: 2,
                slot: 0,
                container: 1,
                ..mir2_client_bevy::inventory::ItemModel::default()
            }],
            ..Default::default()
        });
        app.add_systems(bevy::prelude::Update, keyboard_skill_system);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Digit1);
        app.update();
        match receiver.try_recv().expect("belt use") {
            GatewayCommand::Wire(NativeOutboundCommand::UseItem { slot, grid, .. }) => {
                assert_eq!(slot, Some(0));
                assert_eq!(grid.as_deref(), Some("belt"));
            }
            other => panic!("{other:?}"),
        }
    }
}
