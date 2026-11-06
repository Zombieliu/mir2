//! Optional owned animation bridge without a renderer or environment clock.
//! Both pose advancement and immutable authority reads use the shared owner.

pub use mir2_client_core::entity_animation::*;

#[path = "entity_animation_state.rs"]
mod shared_bridge;

use shared_bridge::EntityAnimationOwner;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn entity_animation_abi_version() -> u32 { 1 }

#[wasm_bindgen]
pub struct EntityAnimationBridge { owner: EntityAnimationOwner }

#[wasm_bindgen]
impl EntityAnimationBridge {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self { Self { owner: EntityAnimationOwner::new() } }

    #[wasm_bindgen(js_name = resolveMir2EntityAnimationPoses)]
    pub fn resolve_mir2_entity_animation_poses(&mut self, snapshot_json: &str) -> String {
        self.owner.resolve_json(snapshot_json)
    }

    #[wasm_bindgen(js_name = getMir2EntityActionPose)]
    pub fn get_mir2_entity_action_pose(&self, query_json: &str) -> String {
        self.owner.peek_json(query_json)
    }

    #[wasm_bindgen(js_name = resetMir2EntityAnimations)]
    pub fn reset_mir2_entity_animations(&mut self) { self.owner.reset(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn snapshot(now_ms: u64, action: &str, token: Option<&str>) -> String {
        json!({"worldKey":"map:self","worldSeed":7,"nowMs":now_ms,"entities":[{
            "objectId":"self","kind":"selfPlayer","direction":"Right","action":action,
            "actionToken":token,"actionSourceKey":"session:self","actionSourceRevision":1,
            "bootstrapKnown":true,"actionKnown":true}]}).to_string()
    }
    fn query() -> String {
        json!({"version":1,"worldKey":"map:self","worldSeed":7,"objectId":"self",
            "actionSourceKey":"session:self"}).to_string()
    }
    fn known(bridge: &EntityAnimationBridge) -> Value {
        serde_json::from_str(&bridge.get_mir2_entity_action_pose(&query())).unwrap()
    }

    #[test]
    fn owned_wrapper_delegates_pose_advancement_and_peek_exactly() {
        assert_eq!(entity_animation_abi_version(), 1);
        let mut wrapper = EntityAnimationBridge::new();
        let mut owner = EntityAnimationOwner::new();
        for input in [snapshot(1000, "standing", None), snapshot(1100, "attack1", Some("attack:1")),
            snapshot(1300, "attack1", Some("attack:1"))] {
            assert_eq!(wrapper.resolve_mir2_entity_animation_poses(&input), owner.resolve_json(&input));
            assert_eq!(wrapper.get_mir2_entity_action_pose(&query()), owner.peek_json(&query()));
        }
        let pose = known(&wrapper);
        assert_eq!(pose["known"], true);
        assert_eq!(pose["action"], "attack1");
        assert_eq!(pose["direction"], "Right");
        assert_eq!(pose["lastNowMs"], 1300);
    }

    #[test]
    fn instances_keep_separate_owners_and_reset_only_the_selected_instance() {
        let mut first = EntityAnimationBridge::new();
        let mut second = EntityAnimationBridge::new();
        first.resolve_mir2_entity_animation_poses(&snapshot(1000, "standing", None));
        assert_eq!(known(&first)["known"], true);
        assert_eq!(known(&second), json!({"version":1,"known":false}));
        second.resolve_mir2_entity_animation_poses(&snapshot(1001, "standing", None));
        let second_before = known(&second);
        first.reset_mir2_entity_animations();
        assert_eq!(known(&first), json!({"version":1,"known":false}));
        assert_eq!(known(&second), second_before);
        first.resolve_mir2_entity_animation_poses(&snapshot(1002, "standing", None));
        assert!(known(&first)["bridgeEpoch"].as_u64().unwrap() > second_before["bridgeEpoch"].as_u64().unwrap());
    }

    #[test]
    fn immutable_peek_never_advances_and_invalid_resolve_withdraws_authority() {
        let mut bridge = EntityAnimationBridge::new();
        bridge.resolve_mir2_entity_animation_poses(&snapshot(1000, "standing", None));
        let before = known(&bridge);
        for _ in 0..3 { assert_eq!(known(&bridge), before); }
        assert_eq!(bridge.get_mir2_entity_action_pose("{}"), "{\"version\":1,\"known\":false}");
        assert_eq!(known(&bridge), before);
        let invalid: Value = serde_json::from_str(&bridge.resolve_mir2_entity_animation_poses("{}")).unwrap();
        assert_eq!(invalid["poses"], json!([]));
        assert!(invalid["errors"].as_array().is_some_and(|errors| !errors.is_empty()));
        assert_eq!(known(&bridge), json!({"version":1,"known":false}));
    }
}
