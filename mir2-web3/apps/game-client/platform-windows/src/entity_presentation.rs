//! Windows-only authoritative entity animation presentation.
//!
//! Gateway packets remain authoritative for object state. This resource owns
//! only the client-side Crystal frame clock, keeping stable action state between
//! network snapshots and producing updated atlas rects only when a visual frame
//! or authoritative payload actually changes.

use std::collections::{HashMap, HashSet};

use bevy::prelude::{Res, ResMut, Resource, Time};
use mir2_bevy_runtime::entity_animation::{
    AnimationAction, AnimationEvent, AnimationWorld, Direction, EntityKind, TransitionReason,
};
use serde_json::Value;

const NATIVE_ANIMATION_WORLD_SEED: u64 = 0x4d49_5232_5749_4e44;

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

    fn sync_pending_payload(&mut self, now_ms: u64) {
        let Some(payload) = self.pending_payload.take() else {
            return;
        };
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
                crate::frame_sets::animation_catalog_for(
                    entity.kind,
                    &entity.body_library,
                    entity.mounted,
                ),
                now_ms,
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
                    now_ms,
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

    fn render_state_if_changed(&mut self, now_ms: u64, effect_visible: bool) -> Option<Value> {
        self.sync_pending_payload(now_ms);
        let payload = self.latest_payload.as_ref()?;
        let effect_visibility_changed =
            self.last_effect_visible.replace(effect_visible) != Some(effect_visible);
        let transitions = self.world.tick(now_ms).ok()?;
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
        let state = crate::atlas::build_entity_render_state_with_poses_and_effect_visibility(
            &visible_payload,
            &frames,
            effect_visible,
        )?;
        self.last_frames = frames;
        self.payload_dirty = false;
        Some(state)
    }
}

pub fn tick_native_entity_presentation(
    time: Res<Time>,
    mut presentation: ResMut<NativeEntityPresentation>,
    player_ui: Option<Res<mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState>>,
) {
    let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    let effect_visible = player_ui
        .as_deref()
        .map(|state| state.core.options.effect)
        .unwrap_or(true);
    let Some(state) = presentation.render_state_if_changed(now_ms, effect_visible) else {
        return;
    };
    if let Ok(json) = serde_json::to_string(&state) {
        let _ = mir2_bevy_runtime::native_ingest::push_native_entity_render_state(json);
    }
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
    use serde_json::json;

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
    fn monster_run_normalizes_to_supported_walk_cycle() {
        assert_eq!(
            normalize_action(EntityKind::Monster, AnimationAction::Running),
            Some(AnimationAction::Walking)
        );
        assert_eq!(
            normalize_action(EntityKind::Npc, AnimationAction::Attack1),
            None
        );
    }

    #[test]
    fn effect_option_removes_and_restores_scarecrow_post_world_layer_without_new_packet() {
        let mut presentation = NativeEntityPresentation::default();
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

        let visible = presentation
            .render_state_if_changed(0, true)
            .expect("effect enabled state");
        assert!(has_additive_layer(&visible));

        let hidden = presentation
            .render_state_if_changed(0, false)
            .expect("option transition rebuilds the same authoritative pose");
        assert!(!has_additive_layer(&hidden));
        assert!(presentation.render_state_if_changed(0, false).is_none());

        let restored = presentation
            .render_state_if_changed(0, true)
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
}
