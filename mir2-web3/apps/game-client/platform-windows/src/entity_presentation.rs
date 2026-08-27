//! Windows-only authoritative entity animation presentation.
//!
//! Gateway packets remain authoritative for object state. This resource owns
//! only the client-side Crystal frame clock, keeping stable action state between
//! network snapshots and producing updated atlas rects only when a visual frame
//! or authoritative payload actually changes.

use std::collections::{HashMap, HashSet};

use bevy::prelude::{Res, ResMut, Resource, Time};
use mir2_bevy_runtime::entity_animation::{
    AnimationAction, AnimationCatalog, AnimationEvent, AnimationWorld, Direction, EntityKind,
    TransitionReason,
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
        self.sync_pending_payload_with(now_ms, crate::frame_sets::animation_catalog_for);
    }

    fn sync_pending_payload_with<F>(&mut self, now_ms: u64, catalog_for: F)
    where
        F: Fn(EntityKind, &str, bool) -> AnimationCatalog,
    {
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
                catalog_for(entity.kind, &entity.body_library, entity.mounted),
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
        self.render_state_if_changed_with(
            now_ms,
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
        let state = render(&visible_payload, &frames, effect_visible)?;
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
            let highlight =
                rendered_layer(layers, &format!("1001:target-highlight:{role}"));
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
            0,
            "one missing actor rect suppresses the entire selected composite"
        );

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
            state["effects"][0]["z"]
                .as_f64()
                .expect("FlamingSword z")
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
                draw.x == 285
                    && draw.y == 614
                    && draw.layer == crate::map_parser::TileLayer::Front
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
        assert!(rendered_layers(&missing_rect, "2005")[0]
            .get("atlasRectKey")
            .is_none());
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
                presentation.sync_pending_payload(at_ms);
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
