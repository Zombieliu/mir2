use super::*;
use mir2_client_bevy::entities::EntityModel;

#[derive(Default)]
pub struct SpellTargetMemory {
    epoch: u64,
    owner: Option<String>,
    selected: Option<u32>,
    magic: Option<String>,
}
pub struct SpellAim {
    pub direction: String,
    pub target_id: u32,
    pub location: (i32, i32),
}
impl SpellTargetMemory {
    pub fn session(&mut self, epoch: u64) {
        if self.epoch != epoch {
            *self = Self {
                epoch,
                ..Default::default()
            };
        }
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
        if self.owner.as_deref() != Some(&player.object_id) {
            *self = Self {
                owner: Some(player.object_id.clone()),
                epoch: self.epoch,
                ..Default::default()
            };
        }
        let find = |id: &str| entities.entities.iter().find(|e| e.object_id == id);
        let selected = combat.and_then(|m| m.target.as_ref()).map(|t| t.object_id);
        if self.selected != selected {
            self.selected = selected;
            self.magic = selected
                .and_then(|id| find(&id.to_string()))
                .filter(|e| {
                    e.kind == EntityKind::Monster
                        && !matches!(presentation.magic_target_flags(&e.object_id).2, 6 | 64 | 70)
                })
                .map(|e| e.object_id.clone());
        }
        let hovered = if cursor_stage.is_some() {
            None
        } else if presentation.self_hovered() {
            entities
                .entities
                .iter()
                .find(|e| e.kind == EntityKind::SelfPlayer)
        } else {
            presentation.hovered_object_id().and_then(find)
        };
        let alive = |e: &&EntityModel| {
            e.kind != EntityKind::Npc && !presentation.magic_target_flags(&e.object_id).0
        };
        let target = match spell {
            "FireBall" | "GreatFireBall" | "ElectricShock" | "Poisoning" | "ThunderBolt"
            | "FlameDisruptor" | "SoulFireBall" | "TurnUndead" | "FrostCrunch" | "Vampirism"
            | "Revelation" | "Entrapment" | "Hallucination" | "DarkBody" | "FireBounce"
            | "MeteorShower" | "StraightShot" | "DoubleShot" | "ElementalShot"
            | "DelayedExplosion" | "BindingShot" | "VampireShot" | "PoisonShot" | "CrippleShot"
            | "NapalmShot" | "SummonVampire" | "SummonToad" | "SummonSnakes" => {
                let target = hovered
                    .filter(alive)
                    .or_else(|| self.magic.as_deref().and_then(find).filter(alive));
                if let Some(e) = target.filter(|e| e.kind == EntityKind::Monster) {
                    self.magic = Some(e.object_id.clone());
                }
                target
            }
            "Purification" | "Healing" | "UltimateEnhancer" | "EnergyShield" | "PetEnhancer" => {
                Some(
                    hovered
                        .filter(alive)
                        .filter(|e| {
                            e.kind != EntityKind::Monster
                                || presentation.magic_target_flags(&e.object_id).1 != 0
                        })
                        .unwrap_or(player),
                )
            }
            "Reincarnation" => hovered.filter(|e| {
                matches!(e.kind, EntityKind::Player | EntityKind::SelfPlayer)
                    && presentation.magic_target_flags(&e.object_id).0
            }),
            "Stonetrap" | "FireBang" | "MassHiding" | "FireWall" | "TrapHexagon"
            | "HealingCircle" | "CatTongue" | "PoisonCloud" | "Blizzard" | "MeteorStrike"
            | "Trap" => hovered.filter(alive),
            _ => None,
        };
        let cursor = cursor_stage
            .and_then(|p| presentation.grid_position_for_stage((p[0], p[1])))
            .or_else(|| presentation.hovered_grid_position());
        let location = target.map(|e| (e.x, e.y)).or(cursor)?;
        let direction = if spell == "FlashDash" {
            player.direction.as_deref().unwrap_or("down")
        } else {
            let facing_point = target
                .filter(|e| e.object_id != player.object_id)
                .map(|e| (e.x, e.y))
                .or(cursor);
            movement_direction_toward(facing_point, (player.x, player.y))
                .unwrap_or(player.direction.as_deref().unwrap_or("down"))
        }
        .to_owned();
        Some(SpellAim {
            direction,
            target_id: target.and_then(|e| e.object_id.parse().ok()).unwrap_or(0),
            location,
        })
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
