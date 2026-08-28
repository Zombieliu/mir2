//! Windows-only authoritative entity animation presentation.
//!
//! Gateway packets remain authoritative for object state. This resource owns
//! only the client-side Crystal frame clock, keeping stable action state between
//! network snapshots and producing updated atlas rects only when a visual frame
//! or authoritative payload actually changes.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::{Query, Res, ResMut, Resource, Time, Window, With};
use bevy::window::PrimaryWindow;
use mir2_bevy_runtime::entity_animation::{
    AnimationAction, AnimationCatalog, AnimationEvent, AnimationWorld, Direction, EntityKind,
    TransitionReason,
};
use serde_json::Value;

const NATIVE_ANIMATION_WORLD_SEED: u64 = 0x4d49_5232_5749_4e44;
const CRYSTAL_MOVE_PHASE_COUNT: u8 = 6;
const CRYSTAL_MOVE_PHASE_MS: u64 = 100;
const MAX_SMOOTH_TILE_DISTANCE: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
struct NativeMotionWindow {
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    started_ms: u64,
    expires_ms: u64,
}

#[derive(Debug)]
struct ObservedEntity {
    object_id: String,
    kind: EntityKind,
    body_library: String,
    mounted: bool,
    direction: Direction,
    action: Option<(u64, AnimationAction)>,
    initially_dead: bool,
    initially_skeleton: bool,
}

#[derive(Resource, Debug)]
pub struct NativeEntityPresentation {
    world: AnimationWorld,
    latest_payload: Option<Value>,
    pending_payload: Option<Value>,
    last_applied_sequence: HashMap<String, u64>,
    last_libraries: HashMap<String, String>,
    last_frames: HashMap<String, (i64, AnimationAction)>,
    hidden_after_hide: HashSet<String>,
    last_effect_visible: Option<bool>,
    hover_cursor_stage: Option<(f32, f32)>,
    highlight_target: bool,
    hovered_object_id: Option<String>,
    self_hovered: bool,
    self_object_id: Option<String>,
    last_positions: HashMap<String, (i32, i32)>,
    motion_windows: HashMap<String, NativeMotionWindow>,
    payload_dirty: bool,
}

impl Default for NativeEntityPresentation {
    fn default() -> Self {
        Self {
            world: AnimationWorld::new(NATIVE_ANIMATION_WORLD_SEED),
            latest_payload: None,
            pending_payload: None,
            last_applied_sequence: HashMap::new(),
            last_libraries: HashMap::new(),
            last_frames: HashMap::new(),
            hidden_after_hide: HashSet::new(),
            last_effect_visible: None,
            hover_cursor_stage: None,
            highlight_target: true,
            hovered_object_id: None,
            self_hovered: false,
            self_object_id: None,
            last_positions: HashMap::new(),
            motion_windows: HashMap::new(),
            payload_dirty: false,
        }
    }
}

impl NativeEntityPresentation {
    pub fn reset_session(&mut self) {
        *self = Self::default();
    }

    pub fn replace_payload(&mut self, payload: Value) {
        self.pending_payload = Some(payload);
    }

    pub(crate) fn hovered_object_id(&self) -> Option<&str> {
        self.hovered_object_id.as_deref()
    }

    pub(crate) fn self_hovered(&self) -> bool {
        self.self_hovered
    }

    pub(crate) fn hovered_grid_position(&self) -> Option<(i32, i32)> {
        let (cursor_x, cursor_y) = self.hover_cursor_stage?;
        if !(0.0..1024.0).contains(&cursor_x) || !(0.0..768.0).contains(&cursor_y) {
            return None;
        }
        let center = self
            .latest_payload
            .as_ref()?
            .get("sceneView")?
            .get("center")?;
        let center_x = center
            .get("x")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())?;
        let center_y = center
            .get("y")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())?;
        Some((
            center_x + (cursor_x / 48.0).floor() as i32 - 10,
            center_y + (cursor_y / 32.0).floor() as i32 - 11,
        ))
    }

    pub(crate) fn has_active_motion(&self, now_ms: u64) -> bool {
        self.motion_windows
            .values()
            .any(|window| now_ms < window.expires_ms)
    }

    pub(crate) fn camera_screen_offset(&self, now_ms: u64) -> (f32, f32) {
        self.self_object_id
            .as_deref()
            .and_then(|object_id| self.motion_windows.get(object_id))
            .map(|window| native_motion_offset(window, now_ms))
            .map(|(x, y)| (-x, -y))
            .unwrap_or((0.0, 0.0))
    }

    pub(crate) fn entity_screen_offset(&self, object_id: &str, now_ms: u64) -> (f32, f32) {
        let (camera_x, camera_y) = self.camera_screen_offset(now_ms);
        let (entity_x, entity_y) = self
            .motion_windows
            .get(object_id)
            .map(|window| native_motion_offset(window, now_ms))
            .unwrap_or((0.0, 0.0));
        (entity_x + camera_x, entity_y + camera_y)
    }

    #[cfg(test)]
    pub(crate) fn set_hovered_object_id_for_test(&mut self, object_id: Option<&str>) {
        self.hovered_object_id = object_id.map(ToOwned::to_owned);
    }

    #[cfg(test)]
    pub(crate) fn set_hover_grid_context_for_test(
        &mut self,
        center: (i32, i32),
        cursor_stage: (f32, f32),
    ) {
        self.latest_payload = Some(serde_json::json!({
            "sceneView": {"center": {"x": center.0, "y": center.1}}
        }));
        self.hover_cursor_stage = Some(cursor_stage);
    }

    /// Object ids whose client-side Crystal action clock has reached the
    /// terminal corpse pose. `dead=true` and the `Die` action are deliberately
    /// excluded: Crystal projectile completion callbacks suppress their impact
    /// only when `CurrentAction == MirAction.Dead`.
    pub(crate) fn dead_action_object_ids(&self) -> HashSet<u32> {
        self.world
            .active_states()
            .filter_map(|(object_id, state)| {
                (state.pose().action == AnimationAction::Dead)
                    .then(|| object_id.parse::<u32>().ok())
                    .flatten()
            })
            .collect()
    }

    fn set_hover_presentation(
        &mut self,
        hover_cursor_stage: Option<(f32, f32)>,
        highlight_target: bool,
    ) {
        if self.hover_cursor_stage != hover_cursor_stage
            || self.highlight_target != highlight_target
        {
            self.hover_cursor_stage = hover_cursor_stage;
            self.highlight_target = highlight_target;
            self.payload_dirty = true;
        }
    }

    fn sync_pending_payload(&mut self, animation_now_ms: u64, motion_now_ms: u64) {
        self.sync_pending_payload_with_clocks(
            animation_now_ms,
            motion_now_ms,
            crate::frame_sets::animation_catalog_for,
        );
    }

    fn sync_pending_payload_with<F>(&mut self, now_ms: u64, catalog_for: F)
    where
        F: Fn(EntityKind, &str, bool) -> AnimationCatalog,
    {
        self.sync_pending_payload_with_clocks(now_ms, now_ms, catalog_for);
    }

    fn sync_pending_payload_with_clocks<F>(
        &mut self,
        animation_now_ms: u64,
        motion_now_ms: u64,
        catalog_for: F,
    ) where
        F: Fn(EntityKind, &str, bool) -> AnimationCatalog,
    {
        let Some(mut payload) = self.pending_payload.take() else {
            return;
        };
        self.attach_native_motion_windows(&mut payload, motion_now_ms);
        let observed = payload
            .get("entities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(parse_observed_entity)
            .collect::<Vec<_>>();
        let observed_ids = observed
            .iter()
            .map(|entity| entity.object_id.clone())
            .collect::<HashSet<_>>();
        self.hidden_after_hide
            .retain(|object_id| observed_ids.contains(object_id));
        for entity in &observed {
            if entity
                .action
                .is_some_and(|(_, action)| action == AnimationAction::Show)
            {
                self.hidden_after_hide.remove(&entity.object_id);
            }
        }
        let visible = observed
            .iter()
            .filter(|entity| !self.hidden_after_hide.contains(&entity.object_id))
            .map(|entity| entity.object_id.clone())
            .collect::<HashSet<_>>();

        let removed = self
            .world
            .active_states()
            .filter_map(|(object_id, state)| {
                (!visible.contains(object_id)).then_some(state.key.clone())
            })
            .collect::<Vec<_>>();
        for key in removed {
            let _ = self.world.remove(&key);
            self.last_applied_sequence.remove(&key.object_id);
            self.last_libraries.remove(&key.object_id);
            self.last_frames.remove(&key.object_id);
        }

        for entity in observed {
            if self.hidden_after_hide.contains(&entity.object_id) {
                continue;
            }
            let catalog_key = format!("{}#mounted={}", entity.body_library, entity.mounted);
            if self
                .last_libraries
                .get(&entity.object_id)
                .is_some_and(|library| library != &catalog_key)
            {
                if let Some(key) = self
                    .world
                    .active_state(&entity.object_id)
                    .map(|state| state.key.clone())
                {
                    let _ = self.world.remove(&key);
                }
                self.last_applied_sequence.remove(&entity.object_id);
                self.last_frames.remove(&entity.object_id);
            }
            let update = self.world.observe_snapshot(
                entity.object_id.clone(),
                entity.kind,
                entity.direction,
                catalog_for(entity.kind, &entity.body_library, entity.mounted),
                animation_now_ms,
            );
            let Ok(update) = update else {
                continue;
            };
            self.last_libraries
                .insert(entity.object_id.clone(), catalog_key);

            let action = entity
                .action
                .or_else(|| {
                    (update.spawned && entity.initially_skeleton)
                        .then_some((0, AnimationAction::Skeleton))
                })
                .or_else(|| {
                    (update.spawned && entity.initially_dead).then_some((0, AnimationAction::Dead))
                });
            let Some((sequence, action)) = action else {
                continue;
            };
            if self.last_applied_sequence.get(&entity.object_id) == Some(&sequence) {
                continue;
            }
            let Some(action) = normalize_action(entity.kind, action) else {
                continue;
            };
            if self
                .world
                .apply_event(
                    &update.key,
                    AnimationEvent::new(sequence, action, entity.direction),
                    animation_now_ms,
                )
                .is_ok()
            {
                self.last_applied_sequence
                    .insert(entity.object_id, sequence);
            }
        }

        self.latest_payload = Some(payload);
        self.payload_dirty = true;
    }

    fn attach_native_motion_windows(&mut self, payload: &mut Value, now_ms: u64) {
        let Some(entities) = payload.get_mut("entities").and_then(Value::as_array_mut) else {
            self.self_object_id = None;
            self.last_positions.clear();
            self.motion_windows.clear();
            return;
        };
        let mut observed_ids = HashSet::new();
        for entity in entities {
            let Some(object_id) = entity.get("objectId").and_then(value_object_id) else {
                continue;
            };
            let (Some(x), Some(y)) = (
                entity
                    .get("x")
                    .and_then(Value::as_i64)
                    .and_then(|value| i32::try_from(value).ok()),
                entity
                    .get("y")
                    .and_then(Value::as_i64)
                    .and_then(|value| i32::try_from(value).ok()),
            ) else {
                continue;
            };
            observed_ids.insert(object_id.clone());
            let is_self = entity.get("kind").and_then(Value::as_str) == Some("selfPlayer");
            if is_self {
                self.self_object_id = Some(object_id.clone());
            }

            let previous = self.last_positions.insert(object_id.clone(), (x, y));
            if let Some((from_x, from_y)) = previous.filter(|previous| *previous != (x, y)) {
                let distance = from_x.abs_diff(x).max(from_y.abs_diff(y));
                let is_movement_action = entity
                    .get("_nativeAnimationAction")
                    .and_then(Value::as_str)
                    .is_some_and(|action| matches!(action, "walking" | "running"));
                if is_movement_action && distance <= MAX_SMOOTH_TILE_DISTANCE {
                    let duration_ms = u64::from(CRYSTAL_MOVE_PHASE_COUNT) * CRYSTAL_MOVE_PHASE_MS;
                    let window = NativeMotionWindow {
                        from_x: from_x as f32,
                        from_y: from_y as f32,
                        to_x: x as f32,
                        to_y: y as f32,
                        started_ms: now_ms,
                        expires_ms: now_ms.saturating_add(duration_ms),
                    };
                    self.motion_windows.insert(object_id.clone(), window);
                    if is_self {
                        mir2_bevy_runtime::set_mir2_self_camera_motion(
                            window.from_x,
                            window.from_y,
                            window.to_x,
                            window.to_y,
                            window.started_ms as f64,
                            window.expires_ms as f64,
                        );
                    }
                } else {
                    self.motion_windows.remove(&object_id);
                }
            }

            let active_window = self
                .motion_windows
                .get(&object_id)
                .copied()
                .filter(|window| {
                    now_ms < window.expires_ms && window.to_x == x as f32 && window.to_y == y as f32
                });
            if let Some(window) = active_window {
                entity["motionFromX"] = Value::from(window.from_x);
                entity["motionFromY"] = Value::from(window.from_y);
                entity["motionToX"] = Value::from(window.to_x);
                entity["motionToY"] = Value::from(window.to_y);
                entity["motionStartedMs"] = Value::from(window.started_ms);
                entity["motionDurationMs"] =
                    Value::from(window.expires_ms.saturating_sub(window.started_ms));
            }
        }
        self.last_positions
            .retain(|object_id, _| observed_ids.contains(object_id));
        self.motion_windows.retain(|object_id, window| {
            observed_ids.contains(object_id) && now_ms < window.expires_ms
        });
        if self
            .self_object_id
            .as_ref()
            .is_some_and(|object_id| !observed_ids.contains(object_id))
        {
            self.self_object_id = None;
        }
    }

    fn render_state_if_changed(&mut self, now_ms: u64, effect_visible: bool) -> Option<Value> {
        self.render_state_if_changed_with(
            now_ms,
            effect_visible,
            crate::atlas::build_entity_render_state_with_poses_and_effect_visibility,
        )
    }

    fn render_state_if_changed_at_clocks(
        &mut self,
        animation_now_ms: u64,
        motion_now_ms: u64,
        effect_visible: bool,
    ) -> Option<Value> {
        self.render_state_if_changed_with_clocks(
            animation_now_ms,
            motion_now_ms,
            effect_visible,
            crate::atlas::build_entity_render_state_with_poses_and_effect_visibility,
        )
    }

    fn render_state_if_changed_with<F>(
        &mut self,
        now_ms: u64,
        effect_visible: bool,
        render: F,
    ) -> Option<Value>
    where
        F: FnOnce(&Value, &HashMap<String, (i64, AnimationAction)>, bool) -> Option<Value>,
    {
        self.render_state_if_changed_with_clocks(now_ms, now_ms, effect_visible, render)
    }

    fn render_state_if_changed_with_clocks<F>(
        &mut self,
        animation_now_ms: u64,
        motion_now_ms: u64,
        effect_visible: bool,
        render: F,
    ) -> Option<Value>
    where
        F: FnOnce(&Value, &HashMap<String, (i64, AnimationAction)>, bool) -> Option<Value>,
    {
        self.sync_pending_payload(animation_now_ms, motion_now_ms);
        let payload = self.latest_payload.as_ref()?;
        let effect_visibility_changed =
            self.last_effect_visible.replace(effect_visible) != Some(effect_visible);
        let transitions = self.world.tick(animation_now_ms).ok()?;
        for transition in transitions {
            if transition.reason == TransitionReason::HideCompleted
                && self
                    .last_libraries
                    .get(&transition.key.object_id)
                    .is_some_and(|library| hide_removes_rendered_entity(library))
            {
                self.hidden_after_hide
                    .insert(transition.key.object_id.clone());
            }
        }
        let frames = self
            .world
            .active_states()
            .map(|(object_id, state)| {
                let pose = state.pose();
                (
                    object_id.to_owned(),
                    (i64::from(pose.draw_frame_index), pose.action),
                )
            })
            .collect::<HashMap<_, _>>();
        if !self.payload_dirty && frames == self.last_frames && !effect_visibility_changed {
            return None;
        }

        let mut visible_payload = payload.clone();
        if let Some(object) = visible_payload.as_object_mut() {
            object.insert(
                "_nativeHighlightTarget".to_owned(),
                Value::Bool(self.highlight_target),
            );
            if let Some((x, y)) = self.hover_cursor_stage {
                object.insert(
                    "_nativeHoverCursor".to_owned(),
                    serde_json::json!({"x": x, "y": y}),
                );
            } else {
                object.remove("_nativeHoverCursor");
            }
        }
        if let Some(entities) = visible_payload
            .get_mut("entities")
            .and_then(Value::as_array_mut)
        {
            entities.retain(|entity| {
                entity
                    .get("objectId")
                    .and_then(value_object_id)
                    .is_none_or(|object_id| !self.hidden_after_hide.contains(&object_id))
            });
        }
        let Some(state) = render(&visible_payload, &frames, effect_visible) else {
            self.hovered_object_id = None;
            self.self_hovered = false;
            return None;
        };
        self.hovered_object_id = state.get("hoveredObjectId").and_then(value_object_id);
        self.self_hovered = state
            .get("selfHovered")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        self.last_frames = frames;
        self.payload_dirty = false;
        Some(state)
    }
}

pub fn tick_native_entity_presentation(
    time: Res<Time>,
    mut presentation: ResMut<NativeEntityPresentation>,
    player_ui: Option<Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>>,
    shell: Option<Res<mir2_client_bevy::native_shell::NativeShellModel>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let animation_now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    let motion_now_ms = native_motion_clock_ms();
    let effect_visible = player_ui
        .as_deref()
        .map(|state| state.core.options.effect)
        .unwrap_or(true);
    let highlight_target = player_ui
        .as_deref()
        .map(|state| state.core.options.highlight_target)
        .unwrap_or(true);
    // Hit-testing is an input contract, not a highlight preference. Always
    // publish the cursor while world input is allowed; `highlight_target`
    // controls only the extra redraw band; name visibility is handled by the
    // overlay layer and must not make monsters unclickable.
    let hover_cursor_stage = native_hover_cursor(
        windows.iter().next(),
        shell.as_deref(),
        player_ui.as_deref(),
    );
    presentation.set_hover_presentation(hover_cursor_stage, highlight_target);
    let Some(state) = presentation.render_state_if_changed_at_clocks(
        animation_now_ms,
        motion_now_ms,
        effect_visible,
    ) else {
        return;
    };
    if let Ok(json) = serde_json::to_string(&state) {
        let _ = mir2_bevy_runtime::native_ingest::push_native_entity_render_state(json);
    }
}

pub(crate) fn native_motion_clock_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn native_hover_cursor(
    window: Option<&Window>,
    shell: Option<&mir2_client_bevy::native_shell::NativeShellModel>,
    player_ui: Option<&mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>,
) -> Option<(f32, f32)> {
    use mir2_client_bevy::native_shell::NativeShellScreen;

    if shell.is_none_or(|shell| shell.screen != NativeShellScreen::InGame)
        || player_ui.is_some_and(|ui| ui.blocks_world_click())
    {
        return None;
    }
    let window = window?;
    if !window.focused {
        return None;
    }
    let cursor = window.cursor_position()?;
    let transform = mir2_client_bevy::crystal_ui::CrystalStageTransform::fit(
        window.resolution.width(),
        window.resolution.height(),
    );
    if !transform.contains_physical_point(cursor.x, cursor.y) {
        return None;
    }
    Some(transform.physical_to_logical(cursor.x, cursor.y))
}

fn parse_observed_entity(entity: &Value) -> Option<ObservedEntity> {
    let object_id = entity.get("objectId").and_then(|value| match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(value) => Some(value.clone()),
        _ => None,
    })?;
    let kind = match entity.get("kind").and_then(Value::as_str) {
        Some("selfPlayer" | "player" | "hero") => EntityKind::Player,
        Some("npc") => EntityKind::Npc,
        _ => EntityKind::Monster,
    };
    let direction = parse_direction(
        entity
            .get("direction")
            .and_then(Value::as_str)
            .unwrap_or("down"),
    );
    let action = entity
        .get("_nativeAnimationAction")
        .and_then(Value::as_str)
        .and_then(parse_action)
        .zip(
            entity
                .get("_nativeAnimationSequence")
                .and_then(Value::as_u64),
        )
        .map(|(action, sequence)| (sequence, action));
    let resolved_sprite = crate::atlas::resolved_native_sprite(
        entity,
        action
            .map(|(_, action)| action)
            .unwrap_or(AnimationAction::Standing),
    );
    let mounted = resolved_sprite.mounted();
    Some(ObservedEntity {
        object_id,
        kind,
        body_library: resolved_sprite.body_library,
        mounted,
        direction,
        action,
        initially_dead: entity.get("dead").and_then(Value::as_bool) == Some(true),
        initially_skeleton: entity.get("skeleton").and_then(Value::as_bool) == Some(true),
    })
}

fn parse_direction(direction: &str) -> Direction {
    match direction.to_ascii_lowercase().as_str() {
        "up" => Direction::Up,
        "upright" => Direction::UpRight,
        "right" => Direction::Right,
        "downright" => Direction::DownRight,
        "downleft" => Direction::DownLeft,
        "left" => Direction::Left,
        "upleft" => Direction::UpLeft,
        _ => Direction::Down,
    }
}

fn parse_action(action: &str) -> Option<AnimationAction> {
    match action {
        "standing" => Some(AnimationAction::Standing),
        "harvest" => Some(AnimationAction::Harvest),
        "show" => Some(AnimationAction::Show),
        "hide" => Some(AnimationAction::Hide),
        "walking" => Some(AnimationAction::Walking),
        "running" => Some(AnimationAction::Running),
        "attack1" => Some(AnimationAction::Attack1),
        "attack2" => Some(AnimationAction::Attack2),
        "attack3" => Some(AnimationAction::Attack3),
        "attack4" => Some(AnimationAction::Attack4),
        "attackRange1" => Some(AnimationAction::AttackRange1),
        "attackRange2" => Some(AnimationAction::AttackRange2),
        "dashAttack" => Some(AnimationAction::DashAttack),
        "spell" => Some(AnimationAction::Spell),
        "struck" => Some(AnimationAction::Struck),
        "die" => Some(AnimationAction::Die),
        "dead" => Some(AnimationAction::Dead),
        "skeleton" => Some(AnimationAction::Skeleton),
        "revive" => Some(AnimationAction::Revive),
        _ => None,
    }
}

fn value_object_id(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn native_motion_offset(window: &NativeMotionWindow, now_ms: u64) -> (f32, f32) {
    if window.expires_ms <= window.started_ms || now_ms >= window.expires_ms {
        return (0.0, 0.0);
    }
    let elapsed_ms = now_ms.saturating_sub(window.started_ms);
    let phase_index = (elapsed_ms / CRYSTAL_MOVE_PHASE_MS)
        .min(u64::from(CRYSTAL_MOVE_PHASE_COUNT.saturating_sub(1)));
    // Crystal applies the first displacement increment while frame zero is
    // drawn, then advances one of six movement phases every 100 ms.
    let progress = (phase_index + 1) as f32 / f32::from(CRYSTAL_MOVE_PHASE_COUNT);
    let remaining = (1.0 - progress).clamp(0.0, 1.0);
    (
        crystal_even_pixel((window.from_x - window.to_x) * 48.0 * remaining),
        crystal_even_pixel((window.from_y - window.to_y) * 32.0 * remaining),
    )
}

fn crystal_even_pixel(value: f32) -> f32 {
    if !value.is_finite() || value.abs() < 0.001 {
        return 0.0;
    }
    let pixel = value.trunc() as i32;
    (pixel + pixel % 2) as f32
}

fn hide_removes_rendered_entity(catalog_key: &str) -> bool {
    let library = catalog_key
        .split_once("#mounted=")
        .map_or(catalog_key, |(library, _)| library)
        .trim()
        .replace('\\', "/");
    let library = library
        .trim_matches('/')
        .strip_prefix("original-ui/")
        .unwrap_or_else(|| library.trim_matches('/'));
    library == "Monster/010"
}

fn normalize_action(kind: EntityKind, action: AnimationAction) -> Option<AnimationAction> {
    match (kind, action) {
        (EntityKind::Npc, AnimationAction::Standing | AnimationAction::Harvest) => Some(action),
        (EntityKind::Npc, _) => None,
        (EntityKind::Monster, AnimationAction::Running) => Some(AnimationAction::Walking),
        (EntityKind::Monster, AnimationAction::AttackRange1 | AnimationAction::Spell) => {
            Some(AnimationAction::Attack1)
        }
        _ => Some(action),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay_bridge::NativeGameplayAdapter;
    use crate::native_protocol::PacketEvent;
    use mir2_bevy_runtime::entity_animation::FrameDescriptor;
    use serde_json::json;
    use std::io::Read;

    fn player_payload(sequence: u64) -> Value {
        json!({
            "sceneView": {"center": {"x": 10, "y": 10}},
            "entities": [{
                "objectId": 1,
                "kind": "selfPlayer",
                "x": 10,
                "y": 10,
                "direction": "down",
                "dead": false,
                "_nativeAnimationAction": "walking",
                "_nativeAnimationSequence": sequence,
                "sprite": {
                    "bodyLibrary": "AArmour/00",
                    "directionStride": 4,
                    "frameBaseOffset": 0
                }
            }]
        })
    }

    fn rendered_path(state: &Value) -> &str {
        state["entities"][0]["layers"][0]["path"]
            .as_str()
            .expect("rendered frame path")
    }

    fn has_additive_layer(state: &Value) -> bool {
        state["entities"][0]["layers"]
            .as_array()
            .expect("entity layers")
            .iter()
            .any(|layer| layer["additive"].as_bool() == Some(true))
    }

    #[test]
    fn animation_advances_without_another_gateway_payload() {
        let mut presentation = NativeEntityPresentation::default();
        presentation.replace_payload(player_payload(1));
        let first = presentation
            .render_state_if_changed(0, true)
            .expect("initial walking frame");
        assert!(rendered_path(&first).ends_with("/56.png"));
        assert!(presentation.render_state_if_changed(99, true).is_none());
        let second = presentation
            .render_state_if_changed(100, true)
            .expect("next timed walking frame");
        assert!(rendered_path(&second).ends_with("/57.png"));
    }

    #[test]
    fn attack_range_two_parser_uses_the_crystal_player_action() {
        assert_eq!(
            parse_action("attackRange2"),
            Some(AnimationAction::AttackRange2)
        );
    }

    #[test]
    fn duplicate_action_sequence_does_not_restart_animation() {
        let mut presentation = NativeEntityPresentation::default();
        presentation.replace_payload(player_payload(7));
        let _ = presentation.render_state_if_changed(0, true);
        let second = presentation
            .render_state_if_changed(100, true)
            .expect("second frame");
        assert!(rendered_path(&second).ends_with("/57.png"));

        presentation.replace_payload(player_payload(7));
        let repeated = presentation
            .render_state_if_changed(100, true)
            .expect("authoritative payload refresh");
        assert!(rendered_path(&repeated).ends_with("/57.png"));
        let third = presentation
            .render_state_if_changed(200, true)
            .expect("continued frame");
        assert!(rendered_path(&third).ends_with("/58.png"));
    }

    #[test]
    fn native_motion_offset_matches_crystal_phase_boundaries_and_even_pixels() {
        let horizontal = NativeMotionWindow {
            from_x: 10.0,
            from_y: 10.0,
            to_x: 11.0,
            to_y: 10.0,
            started_ms: 1_000,
            expires_ms: 1_600,
        };
        assert_eq!(native_motion_offset(&horizontal, 1_000), (-40.0, 0.0));
        assert_eq!(native_motion_offset(&horizontal, 1_099), (-40.0, 0.0));
        assert_eq!(native_motion_offset(&horizontal, 1_100), (-32.0, 0.0));
        assert_eq!(native_motion_offset(&horizontal, 1_499), (-8.0, 0.0));
        assert_eq!(native_motion_offset(&horizontal, 1_500), (0.0, 0.0));
        assert_eq!(native_motion_offset(&horizontal, 1_600), (0.0, 0.0));

        let vertical = NativeMotionWindow {
            from_x: 10.0,
            from_y: 10.0,
            to_x: 10.0,
            to_y: 9.0,
            started_ms: 2_000,
            expires_ms: 2_600,
        };
        assert_eq!(native_motion_offset(&vertical, 2_000), (0.0, 26.0));
        assert_eq!(native_motion_offset(&vertical, 2_100), (0.0, 22.0));
        assert_eq!(crystal_even_pixel(-21.9), -22.0);
        assert_eq!(crystal_even_pixel(21.9), 22.0);
    }

    #[test]
    fn authoritative_move_uses_wall_clock_and_keeps_self_screen_locked() {
        let mut presentation = NativeEntityPresentation::default();
        presentation.replace_payload(player_payload(1));
        let _ = presentation
            .render_state_if_changed_with_clocks(0, 1_700_000_000_000, true, |payload, _, _| {
                Some(payload.clone())
            })
            .expect("initial authoritative payload");

        let mut moved = player_payload(2);
        moved["sceneView"]["center"]["x"] = json!(11);
        moved["entities"][0]["x"] = json!(11);
        presentation.replace_payload(moved);
        let rendered = presentation
            .render_state_if_changed_with_clocks(100, 1_700_000_000_100, true, |payload, _, _| {
                Some(payload.clone())
            })
            .expect("moved authoritative payload");

        let entity = &rendered["entities"][0];
        assert_eq!(entity["motionFromX"], json!(10.0));
        assert_eq!(entity["motionToX"], json!(11.0));
        assert_eq!(entity["motionStartedMs"], json!(1_700_000_000_100_u64));
        assert_eq!(entity["motionDurationMs"], json!(600));
        assert_eq!(
            presentation.camera_screen_offset(1_700_000_000_100),
            (40.0, 0.0)
        );
        assert_eq!(
            presentation.entity_screen_offset("1", 1_700_000_000_100),
            (0.0, 0.0),
            "self motion and camera motion must cancel"
        );
        assert_eq!(
            presentation.entity_screen_offset("remote", 1_700_000_000_100),
            (40.0, 0.0),
            "static world actors follow the moving self camera"
        );
        assert_eq!(
            presentation.camera_screen_offset(1_700_000_000_700),
            (0.0, 0.0)
        );

        let mut teleported = player_payload(3);
        teleported["sceneView"]["center"]["x"] = json!(20);
        teleported["entities"][0]["x"] = json!(20);
        presentation.replace_payload(teleported);
        let rendered = presentation
            .render_state_if_changed_with_clocks(200, 1_700_000_000_200, true, |payload, _, _| {
                Some(payload.clone())
            })
            .expect("teleport payload");
        assert!(rendered["entities"][0].get("motionFromX").is_none());
        assert!(!presentation.has_active_motion(1_700_000_000_200));
    }

    #[test]
    fn hover_cursor_and_highlight_setting_republish_without_authoritative_packet() {
        let mut presentation = NativeEntityPresentation::default();
        presentation.replace_payload(player_payload(1));
        let initial = presentation
            .render_state_if_changed_with(0, true, |payload, _, _| Some(payload.clone()))
            .expect("initial local presentation payload");
        assert_eq!(initial["_nativeHighlightTarget"], json!(true));
        assert!(initial.get("_nativeHoverCursor").is_none());

        presentation.set_hover_presentation(Some((482.25, 353.5)), true);
        let hovered = presentation
            .render_state_if_changed_with(0, true, |payload, _, _| Some(payload.clone()))
            .expect("cursor-only redraw");
        assert_eq!(hovered["_nativeHoverCursor"]["x"], json!(482.25));
        assert_eq!(hovered["_nativeHoverCursor"]["y"], json!(353.5));
        assert!(presentation
            .render_state_if_changed_with(0, true, |payload, _, _| Some(payload.clone()))
            .is_none());

        presentation.set_hover_presentation(None, true);
        let cleared = presentation
            .render_state_if_changed_with(0, true, |payload, _, _| Some(payload.clone()))
            .expect("cursor-leave redraw");
        assert!(cleared.get("_nativeHoverCursor").is_none());

        presentation.set_hover_presentation(None, false);
        let disabled = presentation
            .render_state_if_changed_with(0, true, |payload, _, _| Some(payload.clone()))
            .expect("setting-only redraw");
        assert_eq!(disabled["_nativeHighlightTarget"], json!(false));
    }

    #[test]
    fn native_hover_cursor_is_independent_of_highlight_and_uses_runtime_gates() {
        use bevy::prelude::Vec2;
        use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};

        let mut window = Window::default();
        window.resolution.set(1280.0, 720.0);
        window.focused = true;
        window.set_cursor_position(Some(Vec2::new(100.0, 360.0)));
        let mut shell = NativeShellModel::default();
        shell.screen = NativeShellScreen::InGame;
        assert_eq!(native_hover_cursor(Some(&window), Some(&shell), None), None);

        window.set_cursor_position(Some(Vec2::new(640.0, 360.0)));
        assert_eq!(
            native_hover_cursor(Some(&window), Some(&shell), None),
            Some((512.0, 384.0))
        );
        shell.screen = NativeShellScreen::Login;
        assert_eq!(native_hover_cursor(Some(&window), Some(&shell), None), None);
        shell.screen = NativeShellScreen::InGame;
        window.focused = false;
        assert_eq!(native_hover_cursor(Some(&window), Some(&shell), None), None);
    }

    #[test]
    fn rendered_hover_identity_is_published_and_cleared_fail_closed() {
        let mut presentation = NativeEntityPresentation::default();
        presentation.replace_payload(player_payload(1));
        let _ = presentation.render_state_if_changed_with(0, true, |_, _, _| {
            Some(json!({"hoveredObjectId": "2001", "selfHovered": true}))
        });
        assert_eq!(presentation.hovered_object_id(), Some("2001"));
        assert!(presentation.self_hovered());

        presentation.set_hover_presentation(Some((512.0, 384.0)), false);
        assert!(presentation
            .render_state_if_changed_with(0, true, |_, _, _| None)
            .is_none());
        assert_eq!(presentation.hovered_object_id(), None);
        assert!(!presentation.self_hovered());

        presentation.reset_session();
        assert_eq!(presentation.hovered_object_id(), None);
        assert!(!presentation.self_hovered());
    }

    #[test]
    fn monster_run_normalizes_to_supported_walk_cycle() {
        assert_eq!(
            normalize_action(EntityKind::Monster, AnimationAction::Running),
            Some(AnimationAction::Walking)
        );
        assert_eq!(
            normalize_action(EntityKind::Npc, AnimationAction::Attack1),
            None
        );
        assert_eq!(
            parse_action("dashAttack"),
            Some(AnimationAction::DashAttack)
        );
    }

    #[test]
    fn projectile_completion_gate_uses_dead_action_not_dead_flag() {
        let mut presentation = NativeEntityPresentation::default();
        presentation.replace_payload(json!({
            "sceneView": {"center": {"x": 10, "y": 10}},
            "entities": [{
                "objectId": 2005,
                "kind": "monster",
                "x": 10,
                "y": 10,
                "direction": "down",
                "dead": true,
                "_nativeAnimationAction": "die",
                "_nativeAnimationSequence": 1,
                "sprite": {
                    "bodyLibrary": "Monster/005",
                    "directionStride": 10,
                    "frameBaseOffset": 0
                }
            }]
        }));
        presentation.sync_pending_payload_with(0, |_, _, _| AnimationCatalog::crystal_monster());
        assert!(
            presentation.dead_action_object_ids().is_empty(),
            "dead=true during Die must not suppress a Crystal projectile impact"
        );
        presentation.world.tick(999).expect("advance Die action");
        assert!(presentation.dead_action_object_ids().is_empty());
        presentation
            .world
            .tick(1_000)
            .expect("complete Die into terminal Dead action");
        assert_eq!(
            presentation.dead_action_object_ids(),
            HashSet::from([2005]),
            "the shared action clock closes the impact callback only at Dead"
        );
    }

    #[test]
    fn effect_option_removes_and_restores_scarecrow_post_world_layer_without_new_packet() {
        let mut presentation = NativeEntityPresentation::default();
        let manifest = crate::atlas::scarecrow_routing_atlas_manifest_fixture();
        presentation.replace_payload(json!({
            "sceneView": {"center": {"x": 10, "y": 10}, "width": 19, "height": 15},
            "entities": [{
                "objectId": 2005,
                "kind": "monster",
                "x": 10,
                "y": 10,
                "direction": "down",
                "_nativeAnimationAction": "die",
                "_nativeAnimationSequence": 1,
                "sprite": {
                    "bodyLibrary": "Monster/005",
                    "directionStride": 10,
                    "frameBaseOffset": 0
                }
            }]
        }));
        // Keep this unit test independent from a packaged frame-set manifest.
        // Monster/005 uses the compiled Crystal monster descriptor, whose
        // source ranges are separately locked against the generated catalog.
        presentation.sync_pending_payload_with(0, |_, _, _| AnimationCatalog::crystal_monster());

        let visible = presentation
            .render_state_if_changed_with(0, true, |payload, poses, effect_visible| {
                crate::atlas::build_entity_render_state_with_manifest_for_test(
                    payload,
                    poses,
                    effect_visible,
                    &manifest,
                )
            })
            .expect("effect enabled state");
        assert!(has_additive_layer(&visible));

        let hidden = presentation
            .render_state_if_changed_with(0, false, |payload, poses, effect_visible| {
                crate::atlas::build_entity_render_state_with_manifest_for_test(
                    payload,
                    poses,
                    effect_visible,
                    &manifest,
                )
            })
            .expect("option transition rebuilds the same authoritative pose");
        assert!(!has_additive_layer(&hidden));
        assert!(presentation
            .render_state_if_changed_with(0, false, |payload, poses, effect_visible| {
                crate::atlas::build_entity_render_state_with_manifest_for_test(
                    payload,
                    poses,
                    effect_visible,
                    &manifest,
                )
            })
            .is_none());

        let restored = presentation
            .render_state_if_changed_with(0, true, |payload, poses, effect_visible| {
                crate::atlas::build_entity_render_state_with_manifest_for_test(
                    payload,
                    poses,
                    effect_visible,
                    &manifest,
                )
            })
            .expect("re-enabling effects rebuilds without another gateway packet");
        assert!(has_additive_layer(&restored));
    }

    #[test]
    fn cannibal_plant_hide_finishes_before_visual_suppression_and_show_restores_it() {
        let mut presentation = NativeEntityPresentation::default();
        let payload = |sequence: u64, action: &str| {
            json!({
                "sceneView": {"center": {"x": 10, "y": 10}},
                "entities": [{
                    "objectId": 10,
                    "kind": "monster",
                    "x": 10,
                    "y": 10,
                    "direction": "down",
                    "_nativeAnimationAction": action,
                    "_nativeAnimationSequence": sequence,
                    "sprite": {
                        "bodyLibrary": "Monster/010",
                        "directionStride": 0,
                        "frameBaseOffset": 0
                    }
                }]
            })
        };

        presentation.replace_payload(payload(1, "hide"));
        let hide_start = presentation
            .render_state_if_changed(0, true)
            .expect("hide starts before suppression");
        assert!(rendered_path(&hide_start).ends_with("/Monster/010/12.png"));

        let hidden = presentation
            .render_state_if_changed(1_600, true)
            .expect("hide completion changes visible set");
        assert!(hidden["entities"]
            .as_array()
            .expect("rendered entities")
            .is_empty());

        presentation.replace_payload(payload(2, "show"));
        let show_start = presentation
            .render_state_if_changed(1_600, true)
            .expect("ObjectShow restores the entity before animation");
        assert!(rendered_path(&show_start).ends_with("/Monster/010/4.png"));
    }

    #[test]
    fn removed_hidden_object_does_not_suppress_a_reused_object_id() {
        let mut presentation = NativeEntityPresentation::default();
        let payload = |sequence: u64, action: &str, body_library: &str| {
            json!({
                "sceneView": {"center": {"x": 10, "y": 10}},
                "entities": [{
                    "objectId": 10,
                    "kind": "monster",
                    "x": 10,
                    "y": 10,
                    "direction": "down",
                    "_nativeAnimationAction": action,
                    "_nativeAnimationSequence": sequence,
                    "sprite": {
                        "bodyLibrary": body_library,
                        "directionStride": 0,
                        "frameBaseOffset": 0
                    }
                }]
            })
        };

        presentation.replace_payload(payload(1, "hide", "Monster/010"));
        presentation
            .render_state_if_changed(0, true)
            .expect("hide starts before suppression");
        presentation
            .render_state_if_changed(1_600, true)
            .expect("hide completion suppresses the plant");

        presentation.replace_payload(json!({
            "sceneView": {"center": {"x": 10, "y": 10}},
            "entities": []
        }));
        presentation
            .render_state_if_changed(1_601, true)
            .expect("ObjectRemove changes the rendered entity set");

        presentation.replace_payload(payload(1, "standing", "Monster/003"));
        let reused = presentation
            .render_state_if_changed(1_602, true)
            .expect("a later actor may reuse the removed object id");
        assert!(rendered_path(&reused).contains("/Monster/003/"));
    }

    fn vis01_catalog(kind: EntityKind, library: &str, _: bool) -> AnimationCatalog {
        let normalized = library
            .trim()
            .trim_matches('/')
            .strip_prefix("original-ui/")
            .unwrap_or_else(|| library.trim().trim_matches('/'));
        if normalized == "Monster/010" {
            let mut catalog = AnimationCatalog::new();
            for (action, descriptor) in [
                (
                    AnimationAction::Standing,
                    FrameDescriptor::from_crystal(0, 4, -4, 500, false),
                ),
                (
                    AnimationAction::Show,
                    FrameDescriptor::from_crystal(4, 8, -8, 200, false),
                ),
                (
                    AnimationAction::Hide,
                    FrameDescriptor::from_crystal(12, 8, -8, 200, true),
                ),
            ] {
                catalog
                    .insert(action, descriptor)
                    .expect("VIS-01 CannibalPlant source descriptor");
            }
            return catalog;
        }

        let mut catalog = AnimationCatalog::crystal_default(kind);
        if normalized == "Monster/004" {
            catalog
                .insert(
                    AnimationAction::Skeleton,
                    FrameDescriptor::from_crystal(224, 1, 0, 1_000, false),
                )
                .expect("VIS-01 Deer skeleton source descriptor");
        }
        catalog
    }

    fn checkpoint_frame_paths(fixture: &Value) -> Vec<String> {
        let mut paths = fixture["timeline"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|step| step.get("checkpoint"))
            .filter_map(|checkpoint| checkpoint.get("layers"))
            .flat_map(|layers| layers.as_array().into_iter().flatten())
            .filter_map(|layer| layer.get("path").and_then(Value::as_str))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        paths.sort_unstable();
        paths.dedup();
        paths
    }

    fn rendered_layers<'a>(state: &'a Value, object_id: &str) -> &'a [Value] {
        state["entities"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|entity| entity["objectId"].as_str() == Some(object_id))
            .and_then(|entity| entity["layers"].as_array())
            .map(Vec::as_slice)
            .unwrap_or_else(|| panic!("rendered layers for object {object_id}"))
    }

    fn rendered_layer<'a>(layers: &'a [Value], key: &str) -> &'a Value {
        layers
            .iter()
            .find(|layer| layer["key"].as_str() == Some(key))
            .unwrap_or_else(|| panic!("rendered layer {key}"))
    }

    fn highlight_layer_count(state: &Value) -> usize {
        state["entities"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|entity| entity["layers"].as_array().into_iter().flatten())
            .filter(|layer| {
                layer["key"]
                    .as_str()
                    .is_some_and(|key| key.contains(":target-highlight:"))
            })
            .count()
    }

    #[test]
    fn crystal_selected_remote_player_redraws_exact_composite_at_thirty_percent() {
        let manifest = crate::atlas::routing_atlas_manifest_fixture(&[
            "/original-ui/CArmour/00/980.png",
            "/original-ui/CHair/00/980.png",
            "/original-ui/CWeapon/00/588.png",
        ]);
        let mut payload = json!({
            "sceneView": {"center": {"x": 288, "y": 616}, "width": 19, "height": 15},
            "selectedObjectId": "1001",
            "entities": [{
                "objectId": 1001,
                "kind": "player",
                "classKey": "warrior",
                "genderKey": "female",
                "x": 289,
                "y": 616,
                "direction": "Left",
                "hidden": false,
                "sprite": {
                    "bodyLibrary": "CArmour/00",
                    "hairLibrary": "CHair/00",
                    "weaponLibrary": "CWeapon/00",
                    "frameBaseOffset": 808,
                    "weaponFrameOffset": 416,
                    "directionStride": 4
                }
            }]
        });
        let poses = HashMap::from([("1001".to_owned(), (172, AnimationAction::Attack1))]);
        let state = crate::atlas::build_entity_render_state_with_manifest_for_test(
            &payload, &poses, true, &manifest,
        )
        .expect("selected remote player render state");
        let layers = rendered_layers(&state, "1001");
        assert_eq!(layers.len(), 6, "three source layers plus three redraws");

        for (role, expected_path) in [
            ("weapon-primary", "/original-ui/CWeapon/00/588.png"),
            ("body", "/original-ui/CArmour/00/980.png"),
            ("hair", "/original-ui/CHair/00/980.png"),
        ] {
            let normal = rendered_layer(layers, &format!("1001:{role}"));
            let highlight = rendered_layer(layers, &format!("1001:target-highlight:{role}"));
            assert_eq!(normal["path"], json!(expected_path));
            for field in [
                "path",
                "left",
                "top",
                "width",
                "height",
                "atlasKey",
                "atlasRectKey",
            ] {
                assert_eq!(highlight[field], normal[field], "redraw {role} {field}");
            }
            assert_eq!(highlight["opacity"], json!(0.3));
            assert_eq!(highlight["additive"], json!(false));
            assert!(highlight["z"].as_f64() > normal["z"].as_f64());
        }
        assert!(
            rendered_layer(layers, "1001:target-highlight:weapon-primary")["z"].as_f64()
                < rendered_layer(layers, "1001:target-highlight:body")["z"].as_f64()
        );
        assert!(
            rendered_layer(layers, "1001:target-highlight:body")["z"].as_f64()
                < rendered_layer(layers, "1001:target-highlight:hair")["z"].as_f64()
        );

        payload["entities"][0]["hidden"] = json!(true);
        let hidden = crate::atlas::build_entity_render_state_with_manifest_for_test(
            &payload, &poses, true, &manifest,
        )
        .expect("hidden selected remote player render state");
        let hidden_layers = rendered_layers(&hidden, "1001");
        assert_eq!(
            rendered_layer(hidden_layers, "1001:body")["opacity"],
            json!(0.5)
        );
        assert_eq!(
            rendered_layer(hidden_layers, "1001:target-highlight:body")["opacity"],
            json!(0.3),
            "Crystal DrawBlend is independent of the hidden draw opacity"
        );

        let partial_manifest = crate::atlas::routing_atlas_manifest_fixture(&[
            "/original-ui/CHair/00/980.png",
            "/original-ui/CWeapon/00/588.png",
        ]);
        let partial = crate::atlas::build_entity_render_state_with_manifest_for_test(
            &payload,
            &poses,
            true,
            &partial_manifest,
        )
        .expect("partially resolved player render state");
        assert_eq!(
            highlight_layer_count(&partial),
            3,
            "a verified standalone body keeps the complete selected composite"
        );
        let partial_layers = rendered_layers(&partial, "1001");
        assert!(rendered_layer(partial_layers, "1001:body")
            .get("atlasRectKey")
            .is_none());
        assert!(rendered_layer(partial_layers, "1001:target-highlight:body")
            .get("atlasRectKey")
            .is_none());

        payload["entities"][0]["kind"] = json!("selfPlayer");
        let self_selected = crate::atlas::build_entity_render_state_with_manifest_for_test(
            &payload, &poses, true, &manifest,
        )
        .expect("self player render state");
        assert_eq!(highlight_layer_count(&self_selected), 0);

        payload["entities"][0]["kind"] = json!("npc");
        let npc_selected = crate::atlas::build_entity_render_state_with_manifest_for_test(
            &payload, &poses, true, &manifest,
        )
        .expect("NPC render state");
        assert_eq!(highlight_layer_count(&npc_selected), 0);

        payload["entities"][0]
            .as_object_mut()
            .expect("player object")
            .remove("kind");
        let missing_kind = crate::atlas::build_entity_render_state_with_manifest_for_test(
            &payload, &poses, true, &manifest,
        )
        .expect("missing-kind render state");
        assert_eq!(highlight_layer_count(&missing_kind), 0);
    }

    #[test]
    fn crystal_selected_redraw_precedes_same_and_cross_actor_flaming_sword_effects() {
        let manifest = crate::atlas::routing_atlas_manifest_fixture(&[
            "/original-ui/CArmour/00/980.png",
            "/original-ui/CHair/00/980.png",
            "/original-ui/CWeapon/00/588.png",
        ]);
        let payload = json!({
            "sceneView": {"center": {"x": 288, "y": 616}, "width": 19, "height": 15},
            "selectedObjectId": 1001,
            "entities": [
                {
                    "objectId": 1000,
                    "kind": "selfPlayer",
                    "classKey": "warrior",
                    "genderKey": "male",
                    "x": 285,
                    "y": 614,
                    "direction": "Up",
                    "sprite": {"bodyLibrary": "CArmour/00", "frameBaseOffset": 0}
                },
                {
                    "objectId": 1001,
                    "kind": "player",
                    "classKey": "warrior",
                    "genderKey": "female",
                    "x": 292,
                    "y": 618,
                    "direction": "Left",
                    "sprite": {
                        "bodyLibrary": "CArmour/00",
                        "hairLibrary": "CHair/00",
                        "weaponLibrary": "CWeapon/00",
                        "frameBaseOffset": 808,
                        "weaponFrameOffset": 416
                    }
                }
            ]
        });
        let selected = crate::atlas::build_entity_render_state_with_manifest_for_test(
            &payload,
            &HashMap::from([("1001".to_owned(), (172, AnimationAction::Attack1))]),
            true,
            &manifest,
        )
        .expect("selected player state");
        let selected_layers = rendered_layers(&selected, "1001");
        let max_highlight_z = selected_layers
            .iter()
            .filter(|layer| {
                layer["key"]
                    .as_str()
                    .is_some_and(|key| key.contains(":target-highlight:"))
            })
            .filter_map(|layer| layer["z"].as_f64())
            .reduce(f64::max)
            .expect("selected composite z");

        let effect_z_for = |object_id: u32, x: i32, y: i32| {
            let mut effects = crate::effects::NativeEffects::default();
            effects.observe_render_payload(&payload);
            effects.observe(
                0,
                288,
                616,
                &[crate::gameplay_bridge::NativeEffectEvent {
                    sequence: 1,
                    generation: 1,
                    packet: "ObjectAttack".to_owned(),
                    payload: json!({
                        "objectId": object_id,
                        "location": {"x": x, "y": y},
                        "direction": "Up",
                        "spell": 8,
                        "level": 3,
                        "attackType": 0
                    }),
                }],
                &HashMap::from([(object_id, (x, y))]),
            );
            let state: Value = serde_json::from_str(
                &effects
                    .tick_with_visibility(0, true)
                    .expect("FlamingSword render state"),
            )
            .expect("FlamingSword JSON");
            state["effects"][0]["z"].as_f64().expect("FlamingSword z")
        };

        assert!(
            max_highlight_z < effect_z_for(1001, 292, 618),
            "same-actor DrawEffects must follow selected DrawBlend"
        );
        assert!(
            max_highlight_z < effect_z_for(1000, 285, 614),
            "even a shallower actor effect must follow the deeper selected redraw"
        );
    }

    #[test]
    fn crystal_selected_monster_redraw_stays_between_real_front_tiles_and_effects() {
        let manifest = crate::atlas::routing_atlas_manifest_fixture(&[
            "/original-ui/Monster/005/136.png",
            "/original-ui/Monster/005/164.png",
            "/original-ui/Monster/005/173.png",
            "/original-ui/Monster/005/224.png",
            "/original-ui/Monster/005/233.png",
        ]);
        let mut payload = json!({
            "sceneView": {"center": {"x": 288, "y": 616}, "width": 19, "height": 15},
            "selectedObjectId": 2005,
            "entities": [{
                "objectId": 2005,
                "kind": "monster",
                "x": 285,
                "y": 614,
                "direction": "Right",
                "hidden": false,
                "sprite": {
                    "bodyLibrary": "Monster/005",
                    "frameBaseOffset": 0,
                    "directionStride": 10
                }
            }]
        });

        let compressed = include_bytes!("../../../web/lib/generated/crystal-map-pack/0.map.gz");
        let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
        let mut bytes = Vec::new();
        decoder
            .read_to_end(&mut bytes)
            .expect("decode real 0.map.gz");
        let map = crate::map_parser::parse_type100_map(&bytes).expect("parse real 0.map");
        let front = crate::map_parser::resolve_map_tile_draws(&map)
            .into_iter()
            .find(|draw| {
                draw.x == 285 && draw.y == 614 && draw.layer == crate::map_parser::TileLayer::Front
            })
            .expect("VIS-01 source coordinate is a real 0.map front tile");
        let front_z = f64::from(crate::atlas::map_tile_draw_z_for_test(
            front.x, front.y, front.z,
        ));

        for (frame, action, expected_body, expected_effect) in [
            (
                136,
                AnimationAction::Struck,
                "/original-ui/Monster/005/136.png",
                None,
            ),
            (
                164,
                AnimationAction::Die,
                "/original-ui/Monster/005/164.png",
                Some("/original-ui/Monster/005/224.png"),
            ),
            (
                173,
                AnimationAction::Die,
                "/original-ui/Monster/005/173.png",
                Some("/original-ui/Monster/005/233.png"),
            ),
            (
                173,
                AnimationAction::Dead,
                "/original-ui/Monster/005/173.png",
                None,
            ),
        ] {
            let poses = HashMap::from([("2005".to_owned(), (frame, action))]);
            let state = crate::atlas::build_entity_render_state_with_manifest_for_test(
                &payload, &poses, true, &manifest,
            )
            .expect("selected Scarecrow phase render state");
            let layers = rendered_layers(&state, "2005");
            let normal = rendered_layer(layers, "2005:body");
            let highlight = rendered_layer(layers, "2005:target-highlight:body");
            assert_eq!(normal["path"], json!(expected_body));
            assert_eq!(highlight["path"], normal["path"]);
            assert_eq!(highlight["opacity"], json!(0.3));
            assert_eq!(highlight["additive"], json!(false));
            assert!(normal["z"].as_f64().unwrap() < front_z);
            assert!(front_z < highlight["z"].as_f64().unwrap());

            let effects = layers
                .iter()
                .filter(|layer| layer["additive"].as_bool() == Some(true))
                .collect::<Vec<_>>();
            assert_eq!(effects.len(), usize::from(expected_effect.is_some()));
            if let Some(expected_effect) = expected_effect {
                let effect = effects[0];
                assert_eq!(effect["path"], json!(expected_effect));
                assert!(highlight["z"].as_f64() < effect["z"].as_f64());
                assert!(!layers.iter().any(|layer| {
                    layer["key"]
                        .as_str()
                        .is_some_and(|key| key.contains("target-highlight:scarecrow-die-effect"))
                }));
            }
        }

        payload["entities"]
            .as_array_mut()
            .expect("entities")
            .push(json!({
                "objectId": "2006",
                "kind": "monster",
                "x": 286,
                "y": 614,
                "direction": "Right",
                "sprite": {"bodyLibrary": "Monster/005", "frameBaseOffset": 0, "directionStride": 10}
            }));
        payload["selectedObjectId"] = json!("2006");
        let switched = crate::atlas::build_entity_render_state_with_manifest_for_test(
            &payload,
            &HashMap::from([
                ("2005".to_owned(), (136, AnimationAction::Struck)),
                ("2006".to_owned(), (136, AnimationAction::Struck)),
            ]),
            true,
            &manifest,
        )
        .expect("selection switch render state");
        assert!(!rendered_layers(&switched, "2005").iter().any(|layer| {
            layer["key"]
                .as_str()
                .is_some_and(|key| key.contains(":target-highlight:"))
        }));
        assert_eq!(
            rendered_layers(&switched, "2006")
                .iter()
                .filter(|layer| {
                    layer["key"]
                        .as_str()
                        .is_some_and(|key| key.contains(":target-highlight:"))
                })
                .count(),
            1
        );

        for selected in [Value::Null, json!(2999)] {
            payload["selectedObjectId"] = selected;
            let unselected = crate::atlas::build_entity_render_state_with_manifest_for_test(
                &payload,
                &HashMap::from([
                    ("2005".to_owned(), (136, AnimationAction::Struck)),
                    ("2006".to_owned(), (136, AnimationAction::Struck)),
                ]),
                true,
                &manifest,
            )
            .expect("unselected render state");
            assert_eq!(highlight_layer_count(&unselected), 0);
        }

        payload["selectedObjectId"] = json!(2005);
        payload["entities"]
            .as_array_mut()
            .expect("entities")
            .truncate(1);
        let missing_rect = crate::atlas::build_entity_render_state_with_manifest_for_test(
            &payload,
            &HashMap::from([("2005".to_owned(), (136, AnimationAction::Struck))]),
            true,
            &crate::atlas::routing_atlas_manifest_fixture(&[]),
        )
        .expect("missing target rect state");
        assert_eq!(highlight_layer_count(&missing_rect), 0);
        assert!(
            rendered_layers(&missing_rect, "2005").is_empty(),
            "a non-player body without atlas geometry fails closed instead of inventing 48x64"
        );
    }

    fn assert_vis01_checkpoint(
        presentation: &NativeEntityPresentation,
        payload: &Value,
        state: &Value,
        checkpoint: &Value,
    ) {
        let object_id = checkpoint["objectId"]
            .as_u64()
            .expect("checkpoint objectId")
            .to_string();
        let rendered = state["entities"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|entity| entity["objectId"].as_str() == Some(object_id.as_str()));
        let visible = checkpoint["visible"].as_bool().unwrap_or(true);
        assert_eq!(rendered.is_some(), visible, "object {object_id} visibility");

        let payload_entity = payload["entities"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|entity| {
                entity["objectId"]
                    .as_u64()
                    .is_some_and(|id| id.to_string() == object_id)
            })
            .expect("checkpoint payload entity");
        if let Some(sequence) = checkpoint.get("sequence").and_then(Value::as_u64) {
            assert_eq!(
                payload_entity["_nativeAnimationSequence"].as_u64(),
                Some(sequence),
                "object {object_id} derived packet sequence"
            );
        }
        if let Some(dead) = checkpoint.get("dead").and_then(Value::as_bool) {
            assert_eq!(payload_entity["dead"].as_bool(), Some(dead));
        }
        for field in ["x", "y", "deathKind"] {
            if let Some(expected) = checkpoint.get(field).and_then(Value::as_i64) {
                assert_eq!(
                    payload_entity[field].as_i64(),
                    Some(expected),
                    "object {object_id} authoritative {field}"
                );
            }
        }
        for field in ["direction", "disposition"] {
            if let Some(expected) = checkpoint.get(field).and_then(Value::as_str) {
                assert_eq!(
                    payload_entity[field].as_str(),
                    Some(expected),
                    "object {object_id} authoritative {field}"
                );
            }
        }
        if let Some(expected) = checkpoint.get("bodyLibrary").and_then(Value::as_str) {
            assert_eq!(
                payload_entity["sprite"]["bodyLibrary"].as_str(),
                Some(expected),
                "object {object_id} authoritative body library"
            );
        }
        if let Some(action) = checkpoint.get("action").and_then(Value::as_str) {
            let actual = presentation
                .world
                .active_state(&object_id)
                .map(|state| format!("{:?}", state.pose().action).to_ascii_lowercase())
                .expect("checkpoint animation state");
            assert_eq!(actual, action, "object {object_id} presentation action");
        }

        let Some(rendered) = rendered else {
            return;
        };
        let layers = rendered["layers"].as_array().expect("rendered layers");
        let expected_layers = checkpoint["layers"].as_array().expect("checkpoint layers");
        assert_eq!(
            layers.len(),
            expected_layers.len(),
            "object {object_id} must have no extra or missing layers"
        );
        let unique_keys = layers
            .iter()
            .filter_map(|layer| layer["key"].as_str())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(
            unique_keys.len(),
            layers.len(),
            "object {object_id} layer keys must be unique"
        );
        for expected in expected_layers {
            let key = expected["key"].as_str().expect("layer key");
            let layer = layers
                .iter()
                .find(|layer| layer["key"].as_str() == Some(key))
                .unwrap_or_else(|| panic!("missing layer {key} for object {object_id}"));
            assert_eq!(layer["path"], expected["path"], "layer {key} path");
            assert!(
                layer["atlasRectKey"].as_str().is_some(),
                "layer {key} must route through the selected atlas manifest"
            );
            if let Some(additive) = expected.get("additive").and_then(Value::as_bool) {
                assert_eq!(layer["additive"].as_bool(), Some(additive));
            }
        }

        if let (Some(x), Some(y)) = (
            checkpoint.get("x").and_then(Value::as_i64),
            checkpoint.get("y").and_then(Value::as_i64),
        ) {
            assert_eq!(rendered["gridX"].as_i64(), Some(x));
            assert_eq!(rendered["gridY"].as_i64(), Some(y));
        }
        let layer_z = |suffix: &str| {
            layers
                .iter()
                .find(|layer| {
                    layer["key"]
                        .as_str()
                        .is_some_and(|key| key.ends_with(suffix))
                })
                .and_then(|layer| layer["z"].as_f64())
        };
        if object_id == "1001" {
            assert!(layer_z(":weapon-primary") < layer_z(":body"));
            assert!(layer_z(":body") < layer_z(":hair"));
        } else if object_id == "1000" {
            assert!(layer_z(":body") < layer_z(":hair"));
            assert!(layer_z(":hair") < layer_z(":weapon-primary"));
        } else if object_id == "2005" && expected_layers.len() == 2 {
            assert!(layer_z(":body") < layer_z(":scarecrow-die-effect"));
        }
    }

    #[test]
    fn vis01_bichon_actor_transcript_drives_packets_clocks_layers_and_real_front_occlusion() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/vis01-bichon-actors-v1.json"
        ))
        .expect("VIS-01 fixture JSON");
        assert_eq!(fixture["schemaVersion"], json!(1));
        assert_eq!(
            fixture["source"]["crystalRevision"],
            json!("484983404e3d6afa584e93801f8006ae3429bea9")
        );
        let fixture_text = serde_json::to_string(&fixture).expect("fixture serializes");
        assert!(
            !fixture_text.contains("_nativeAnimation"),
            "the fixture must contain server/Gateway output, never client-derived animation fields"
        );

        let frame_paths = checkpoint_frame_paths(&fixture);
        let frame_path_refs = frame_paths.iter().map(String::as_str).collect::<Vec<_>>();
        let manifest = crate::atlas::routing_atlas_manifest_fixture(&frame_path_refs);
        let mut adapter = NativeGameplayAdapter::default();
        let mut payload = fixture["worldSnapshot"].clone();
        adapter.observe_world_snapshot(&payload);
        let mut presentation = NativeEntityPresentation::default();
        let mut checked_real_occlusion = false;

        for step in fixture["timeline"].as_array().expect("VIS-01 timeline") {
            let at_ms = step["atMs"].as_u64().expect("timeline atMs");
            let event_applied = if let Some(event) = step.get("event") {
                let packet = event["packet"].as_str().expect("packet name").to_owned();
                let packet_event = PacketEvent::Other {
                    packet,
                    payload: event["payload"].clone(),
                };
                assert!(adapter.observe_packet(&packet_event));
                adapter.apply_authoritative_overlay(&mut payload);
                true
            } else {
                false
            };

            if let Some(expected) = step.get("damageCheckpoint") {
                let snapshot = adapter.snapshot(&payload);
                let damage = snapshot.damage_events.last().expect("damage event");
                assert_eq!(damage.sequence, expected["sequence"].as_u64().unwrap());
                assert_eq!(
                    u64::from(damage.object_id),
                    expected["objectId"].as_u64().unwrap()
                );
                assert_eq!(
                    i64::from(damage.damage),
                    expected["damage"].as_i64().unwrap()
                );
                assert_eq!(
                    i64::from(damage.damage_type),
                    expected["damageType"].as_i64().unwrap()
                );
            }

            let Some(checkpoint) = step.get("checkpoint") else {
                continue;
            };
            if event_applied {
                presentation.replace_payload(payload.clone());
                presentation.sync_pending_payload_with(at_ms, vis01_catalog);
            }
            let state = presentation
                .render_state_if_changed_with(at_ms, true, |payload, poses, effect_visible| {
                    crate::atlas::build_entity_render_state_with_manifest_for_test(
                        payload,
                        poses,
                        effect_visible,
                        &manifest,
                    )
                })
                .expect("VIS-01 checkpoint must produce a render-state change");
            assert_vis01_checkpoint(&presentation, &payload, &state, checkpoint);

            if !checked_real_occlusion && checkpoint["objectId"] == json!(2010) {
                let compressed =
                    include_bytes!("../../../web/lib/generated/crystal-map-pack/0.map.gz");
                let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
                let mut bytes = Vec::new();
                decoder
                    .read_to_end(&mut bytes)
                    .expect("decode real 0.map.gz");
                let map = crate::map_parser::parse_type100_map(&bytes).expect("parse real 0.map");
                let front = crate::map_parser::resolve_map_tile_draws(&map)
                    .into_iter()
                    .find(|draw| {
                        draw.x
                            == fixture["source"]["frontOcclusionCell"]["x"]
                                .as_i64()
                                .unwrap() as i32
                            && draw.y
                                == fixture["source"]["frontOcclusionCell"]["y"]
                                    .as_i64()
                                    .unwrap() as i32
                            && draw.layer == crate::map_parser::TileLayer::Front
                    })
                    .expect("fixture coordinate is an actual 0.map front cell");
                let front_z = crate::atlas::map_tile_draw_z_for_test(front.x, front.y, front.z);
                let body_z = state["entities"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|entity| entity["objectId"] == json!("2010"))
                    .and_then(|entity| entity["layers"].as_array())
                    .into_iter()
                    .flatten()
                    .find(|layer| layer["key"] == json!("2010:body"))
                    .and_then(|layer| layer["z"].as_f64())
                    .expect("CannibalPlant body z") as f32;
                assert!(
                    front_z > body_z,
                    "real front cell must occlude same-cell body"
                );
                checked_real_occlusion = true;
            }
        }

        assert!(checked_real_occlusion);
    }

    #[test]
    fn vis01_bichon_actor_transcript_routes_through_candidate_manifests_and_map_state() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/vis01-bichon-actors-v1.json"
        ))
        .expect("VIS-01 fixture JSON");
        crate::assets::require_asset_root().expect("complete Candidate asset root");
        assert!(
            crate::atlas::validate_starter_entity_atlas_pages_for_test(),
            "every Candidate entity-atlas page must exist, decode, and match manifest dimensions/hash"
        );
        let frame_set_path = crate::assets::asset_path("original-ui/frame-sets.generated.json")
            .expect("Candidate frame-set path");
        let frame_sets: Value = serde_json::from_str(
            &std::fs::read_to_string(frame_set_path).expect("read Candidate frame-set catalog"),
        )
        .expect("parse Candidate frame-set catalog");
        assert_eq!(
            frame_sets["sourceContentHash"], fixture["source"]["frameSetSourceContentHash"],
            "fixture clocks must be locked to the packaged generated frame-set source hash"
        );

        let mut adapter = NativeGameplayAdapter::default();
        let mut payload = fixture["worldSnapshot"].clone();
        adapter.observe_world_snapshot(&payload);
        let mut presentation = NativeEntityPresentation::default();
        let mut cannibal_body_bounds = None;

        for step in fixture["timeline"].as_array().expect("VIS-01 timeline") {
            let at_ms = step["atMs"].as_u64().expect("timeline atMs");
            let event_applied = if let Some(event) = step.get("event") {
                let packet = event["packet"].as_str().expect("packet name").to_owned();
                assert!(adapter.observe_packet(&PacketEvent::Other {
                    packet,
                    payload: event["payload"].clone(),
                }));
                adapter.apply_authoritative_overlay(&mut payload);
                true
            } else {
                false
            };

            let Some(checkpoint) = step.get("checkpoint") else {
                continue;
            };
            if event_applied {
                presentation.replace_payload(payload.clone());
                presentation.sync_pending_payload(at_ms, at_ms);
            }
            let state = presentation
                .render_state_if_changed(at_ms, true)
                .expect("Candidate manifest checkpoint must produce render state");
            assert_vis01_checkpoint(&presentation, &payload, &state, checkpoint);

            if cannibal_body_bounds.is_none()
                && checkpoint["objectId"] == json!(2010)
                && checkpoint["visible"] == json!(true)
            {
                let body = state["entities"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|entity| entity["objectId"] == json!("2010"))
                    .and_then(|entity| entity["layers"].as_array())
                    .into_iter()
                    .flatten()
                    .find(|layer| layer["key"] == json!("2010:body"))
                    .expect("Candidate CannibalPlant body layer");
                cannibal_body_bounds = Some((
                    body["left"].as_f64().expect("body left"),
                    body["top"].as_f64().expect("body top"),
                    body["width"].as_f64().expect("body width"),
                    body["height"].as_f64().expect("body height"),
                ));
            }
        }

        let compressed = include_bytes!("../../../web/lib/generated/crystal-map-pack/0.map.gz");
        let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
        let mut bytes = Vec::new();
        decoder
            .read_to_end(&mut bytes)
            .expect("decode real 0.map.gz");
        let map = crate::map_parser::parse_type100_map(&bytes).expect("parse real 0.map");
        let front_draw = crate::map_parser::resolve_map_tile_draws(&map)
            .into_iter()
            .find(|draw| {
                draw.x
                    == fixture["source"]["frontOcclusionCell"]["x"]
                        .as_i64()
                        .unwrap() as i32
                    && draw.y
                        == fixture["source"]["frontOcclusionCell"]["y"]
                            .as_i64()
                            .unwrap() as i32
                    && draw.layer == crate::map_parser::TileLayer::Front
            })
            .expect("real front draw");
        let map_state = crate::map_parser::build_map_render_state(
            &map,
            crate::map_parser::MapViewport::from_gateway_payload(&fixture["worldSnapshot"]),
        )
        .expect("Candidate map render state");
        let key = format!(
            "front:{}:{}",
            fixture["source"]["frontOcclusionCell"]["x"]
                .as_i64()
                .expect("front x"),
            fixture["source"]["frontOcclusionCell"]["y"]
                .as_i64()
                .expect("front y")
        );
        let atlas_tile = map_state["tiles"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|tile| tile["key"].as_str() == Some(key.as_str()));
        let standalone_tile = map_state["standaloneTiles"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|tile| {
                tile["key"].as_str().is_some_and(|tile_key| {
                    tile_key
                        == format!(
                            "standalone:{}:{}:{}:{}",
                            if front_draw.additive {
                                "additive"
                            } else {
                                "normal"
                            },
                            front_draw.x,
                            front_draw.y,
                            crate::map_parser::atlas_rect_key(
                                &front_draw.library,
                                front_draw.frame_index,
                            )
                        )
                })
            });
        let front = atlas_tile
            .or(standalone_tile)
            .expect("real front cell must survive production map binding");
        if atlas_tile.is_some() {
            let atlas_key = front["atlasKey"].as_str().expect("front atlas key");
            let rect_key = front["rectKey"].as_str().expect("front rect key");
            let atlas = map_state["atlases"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|atlas| atlas["key"].as_str() == Some(atlas_key))
                .expect("front atlas page retained");
            assert!(atlas["rects"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|rect| rect["key"].as_str() == Some(rect_key)));
        } else {
            let image_url = front["imageUrl"].as_str().expect("front standalone image");
            assert!(crate::assets::asset_path(image_url).is_some_and(|path| path.is_file()));
        }

        let front_bounds = (
            front["left"].as_f64().expect("front left"),
            front["top"].as_f64().expect("front top"),
            front["width"].as_f64().expect("front width"),
            front["height"].as_f64().expect("front height"),
        );
        let body_bounds = cannibal_body_bounds.expect("CannibalPlant body geometry");
        assert!(
            front_bounds.0 < body_bounds.0 + body_bounds.2
                && front_bounds.0 + front_bounds.2 > body_bounds.0
                && front_bounds.1 < body_bounds.1 + body_bounds.3
                && front_bounds.1 + front_bounds.3 > body_bounds.1,
            "real front tile and actor body geometry must overlap"
        );
    }
}
