use super::*;
pub use mir2_client_bevy::combat_input::SpellAim;
use mir2_client_bevy::{
    combat_input::{CombatActor, SpellTargetMemory as SharedSpellTargetMemory},
    entities::EntityModel,
};
#[derive(Default)]
pub struct SpellTargetMemory {
    memory: SharedSpellTargetMemory,
    magic: Option<String>,
}
impl SpellTargetMemory {
    pub fn session(&mut self, epoch: u64) {
        self.memory.session(epoch);
        self.magic = self.memory.magic.clone();
    }
    pub fn aim(
        &mut self,
        spell: &str,
        player: &EntityModel,
        entities: &EntityModelSet,
        presentation: &NativeEntityPresentation,
        combat: Option<&CombatTargetModel>,
    ) -> Option<SpellAim> {
        self.aim_at(spell, player, entities, presentation, combat, None)
    }
    pub fn aim_at(
        &mut self,
        spell: &str,
        player: &EntityModel,
        entities: &EntityModelSet,
        presentation: &NativeEntityPresentation,
        combat: Option<&CombatTargetModel>,
        cursor_stage: Option<[f32; 2]>,
    ) -> Option<SpellAim> {
        let actor = |e: &EntityModel| {
            let (dead, master_object_id, ai) = presentation.magic_target_flags(&e.object_id);
            CombatActor {
                object_id: e.object_id.clone(),
                kind: e.kind,
                x: e.x,
                y: e.y,
                direction: e.direction.clone(),
                dead,
                master_object_id,
                ai,
            }
        };
        let normalized: Vec<_> = entities.entities.iter().map(actor).collect();
        let current = actor(player);
        let hovered = if cursor_stage.is_some() {
            None
        } else if presentation.self_hovered() {
            entities
                .entities
                .iter()
                .find(|e| e.kind == EntityKind::SelfPlayer)
                .map(|e| e.object_id.as_str())
        } else {
            presentation.hovered_object_id()
        };
        let cursor = cursor_stage
            .and_then(|p| presentation.grid_position_for_stage((p[0], p[1])))
            .or_else(|| presentation.hovered_grid_position());
        let aim = self.memory.aim(
            spell,
            &current,
            &normalized,
            combat.and_then(|m| m.target.as_ref()).map(|t| t.object_id),
            hovered,
            cursor,
        );
        self.magic = self.memory.magic.clone();
        aim
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (EntityModelSet, NativeEntityPresentation) {
        let entities = EntityModelSet {
            entities: vec![
                EntityModel {
                    object_id: "1".into(),
                    kind: EntityKind::SelfPlayer,
                    name: "Self".into(),
                    x: 100,
                    y: 100,
                    level: Some(1),
                    direction: Some("up".into()),
                },
                EntityModel {
                    object_id: "2".into(),
                    kind: EntityKind::Monster,
                    name: "Deer".into(),
                    x: 103,
                    y: 101,
                    level: Some(1),
                    direction: None,
                },
            ],
        };
        let mut p = NativeEntityPresentation::default();
        p.set_hover_grid_context_for_test((110, 120), (512., 384.));
        (entities, p)
    }
    #[test]
    fn source_target_race_dead_and_pet_master_rules_use_authoritative_fields() {
        let (mut entities, mut p) = fixture();
        let mut state = SpellTargetMemory::default();
        p.observe_packet_payload(serde_json::json!({"sceneView":{"center":{"x":110,"y":120}},"entities":[{"objectId":"2","kind":"monster","x":103,"y":101,"dead":false,"masterObjectId":1}]}),0);
        p.set_hovered_object_id_for_test(Some("2"));
        assert_eq!(
            state
                .aim("Healing", &entities.entities[0], &entities, &p, None)
                .unwrap()
                .target_id,
            2
        );
        entities.entities[1].kind = EntityKind::Player;
        p.observe_packet_payload(serde_json::json!({"sceneView":{"center":{"x":110,"y":120}},"entities":[{"objectId":"2","kind":"player","x":103,"y":101,"dead":true}]}),1);
        assert_eq!(
            state
                .aim("Reincarnation", &entities.entities[0], &entities, &p, None)
                .unwrap()
                .target_id,
            2
        );
        assert_eq!(
            state
                .aim("FireBall", &entities.entities[0], &entities, &p, None)
                .unwrap()
                .target_id,
            0
        );
        entities.entities[1].kind = EntityKind::Npc;
        assert_eq!(
            state
                .aim("FireWall", &entities.entities[0], &entities, &p, None)
                .unwrap()
                .target_id,
            0
        );
        state.session(22);
        assert!(state.magic.is_none());
    }

    #[test]
    fn ground_spell_uses_cursor_and_never_selected_combat_target() {
        let (entities, p) = fixture();
        let mut state = SpellTargetMemory::default();
        let aim = state
            .aim("FireWall", &entities.entities[0], &entities, &p, None)
            .unwrap();
        assert_eq!(aim.location, p.hovered_grid_position().unwrap());
        assert_eq!(aim.target_id, 0);
        assert_eq!(aim.direction, "downright");
    }
    #[test]
    fn target_spell_prefers_hover_and_remembers_monster_but_healing_defaults_self() {
        let (entities, mut p) = fixture();
        let mut state = SpellTargetMemory::default();
        p.set_hovered_object_id_for_test(Some("2"));
        let aim = state
            .aim("FireBall", &entities.entities[0], &entities, &p, None)
            .unwrap();
        assert_eq!(aim.target_id, 2);
        assert_eq!(aim.location, (103, 101));
        p.set_hovered_object_id_for_test(None);
        assert_eq!(
            state
                .aim("FireBall", &entities.entities[0], &entities, &p, None)
                .unwrap()
                .target_id,
            2
        );
        p.set_hovered_object_id_for_test(Some("2"));
        assert_eq!(
            state
                .aim("Healing", &entities.entities[0], &entities, &p, None)
                .unwrap()
                .target_id,
            1
        );
        assert_eq!(
            state
                .aim("Reincarnation", &entities.entities[0], &entities, &p, None)
                .unwrap()
                .target_id,
            0
        );
    }
}
