//! Crystal BoneLord/ZumaTaurus stage waves, owned once by the shared Zone.
//! Spawn positions deliberately preserve Zone collision safety: Crystal may
//! stack a wave on Front/current tile; we choose free nearby tiles instead.
use super::entity_combat::{EntityTargetPurpose, ZoneCombatEntityRef};
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StageSummonState {
    pub stage: u8,
    pub slave_object_ids: Vec<u32>,
    pub parent_object_id: Option<u32>,
    pub inherited_target_object_id: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub target_bound: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_incarnation: Option<u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slave_lives: BTreeMap<u32, u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boss_zone() -> ZoneRuntime {
        let template = crystal_monster_by_name("ZumaTaurus").unwrap();
        let mut zone = ZoneRuntime::new(ZoneKey::for_map("stage-wave-unit"));
        let spawn = ZoneMonsterSpawn {
            object_id: 17,
            name: template.name.clone(),
            name_colour_argb: -1,
            image: template.image,
            ai: 17,
            disposition: Some(crate::config::WorldEntityDisposition::Hostile),
            level: template.level,
            max_hp: 210,
            hp: 30,
            experience: template.experience,
            move_speed_ms: 1_000,
            attack_speed_ms: 1_000,
            friendly_guild: None,
            defense: Default::default(),
            position: Point { x: 10, y: 10 },
            direction: MirDirection::Down,
            respawn: None,
            drops: Vec::new(),
        };
        assert!(zone.spawn_authoritative_monster(&spawn, 0).0);
        zone
    }

    #[test]
    fn live_slave_cap_prunes_dead_children_and_death_stops_waves() {
        let mut zone = boss_zone();
        for now in 1..=6 {
            zone.native_monsters.get_mut(&17).unwrap().hp = 210;
            zone.tick_stage_summon_states(now * 2);
            zone.native_monsters.get_mut(&17).unwrap().hp = 30;
            zone.tick_stage_summon_states(now * 2 + 1);
        }
        assert_eq!(zone.native_monsters.len(), 41);
        let child_id = zone.native_monsters[&17]
            .special_ai
            .as_ref()
            .unwrap()
            .stage_summons
            .as_ref()
            .unwrap()
            .slave_object_ids[0];
        zone.native_monsters.get_mut(&child_id).unwrap().dead = true;
        zone.native_monsters.get_mut(&17).unwrap().hp = 210;
        zone.tick_stage_summon_states(20);
        zone.native_monsters.get_mut(&17).unwrap().hp = 30;
        zone.tick_stage_summon_states(21);
        let children = &zone.native_monsters[&17]
            .special_ai
            .as_ref()
            .unwrap()
            .stage_summons
            .as_ref()
            .unwrap()
            .slave_object_ids;
        assert_eq!(children.len(), 40);
        assert!(!children.contains(&child_id));
        assert_eq!(
            zone.native_monsters.len(),
            42,
            "only one replacement may spawn"
        );
        zone.native_monsters.get_mut(&17).unwrap().dead = true;
        zone.tick_stage_summon_states(30);
        assert_eq!(zone.native_monsters.len(), 42);
    }

    #[test]
    fn exhausted_id_space_returns_without_aliasing_an_existing_monster() {
        let mut zone = boss_zone();
        let occupied = zone.native_monsters[&17].clone();
        zone.native_monsters.insert(u32::MAX, occupied);
        zone.next_object_id = u32::MAX;
        assert_eq!(zone.stage_slave_object_id(), None);
        assert!(zone.native_monsters.contains_key(&u32::MAX));
    }
}

const BONE_SLAVES: [&str; 4] = ["BoneSpearman", "BoneBlademan", "BoneArcher", "BoneCaptain"];
const ZUMA_SLAVES: [&str; 7] = [
    "ZumaStatue",
    "ZumaGuardian",
    "ZumaArcher",
    "WedgeMoth",
    "ZumaArcher3",
    "ZumaStatue3",
    "ZumaGuardian3",
];

impl ZoneRuntime {
    pub(super) fn inherited_stage_summon_target(
        &self,
        source_id: u32,
        monster: &ZoneNativeMonster,
        now: u64,
    ) -> Option<NativeMonsterTarget> {
        let state = monster.special_ai.as_ref()?.stage_summons.as_ref()?;
        let id = state.inherited_target_object_id?;
        // New relationships are authorized by their captured life, never a bare ID.
        if state.target_bound {
            let target = self.selected_native_entity_target(source_id, now)?;
            if target.object_id != id
                || !matches!(target.reference, ZoneCombatEntityRef::Player { .. })
            {
                return None;
            }
        }
        let player = self.players.values().find(|p| {
            p.object_id == id
                && !p.dead
                && !p.hidden
                && !p.chat_profile.in_safe_zone
                && !monster.friendly_guild.as_deref().is_some_and(|friendly| {
                    p.chat_profile
                        .guild_name
                        .as_deref()
                        .is_some_and(|guild| guild.eq_ignore_ascii_case(friendly))
                })
                && points_visible(&monster.position, &p.position)
        })?;
        Some(NativeMonsterTarget {
            session_id: player.session_id.clone(),
            object_id: id,
            position: player.position.clone(),
        })
    }

    pub(super) fn tick_stage_summon_states(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        // Legacy checkpoints had only a player object ID. Resolve at most once;
        // subsequent death/reuse must not attach this slave to a new life.
        let legacy: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                let s = m.special_ai.as_ref()?.stage_summons.as_ref()?;
                (!s.target_bound).then_some((id, s.inherited_target_object_id))
            })
            .collect();
        for (id, target_id) in legacy {
            let reference = target_id.and_then(|target| {
                self.players
                    .values()
                    .find(|p| p.object_id == target)
                    .and_then(|p| self.native_entity_player_ref(&p.session_id))
            });
            if let Some(reference) = reference {
                self.set_native_entity_target(id, &reference, now_ms);
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .stage_summons
                .as_mut()
                .unwrap()
                .target_bound = true;
        }
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                (matches!(m.ai, 17 | 30)
                    && !m.dead
                    && m.hp > 0
                    && zone_native_monster_is_authoritatively_hostile(m))
                .then_some(id)
            })
            .collect();
        let mut out = Vec::new();
        for id in ids {
            out.extend(self.tick_stage_summon_boss(id, now_ms));
        }
        out
    }

    fn tick_stage_summon_boss(&mut self, id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
        let boss = self
            .native_monsters
            .get(&id)
            .expect("selected boss")
            .clone();
        let target = self.selected_native_entity_target(id, now_ms).or_else(|| {
            self.native_entity_targets(
                id,
                &boss.position,
                crystal_monster_by_name(&boss.name).map_or(7, |t| i32::from(t.view_range)),
                EntityTargetPurpose::Search,
                now_ms,
            )
            .into_iter()
            .next()
        });
        if let Some(target) = &target {
            self.set_native_entity_target(id, &target.reference, now_ms);
        }
        let stages = if boss.ai == 30 { 3 } else { 7 };
        let mut state = boss
            .special_ai
            .as_ref()
            .and_then(|s| s.stage_summons.clone())
            .unwrap_or(StageSummonState {
                stage: stages,
                slave_object_ids: Vec::new(),
                parent_object_id: None,
                inherited_target_object_id: None,
                target_bound: true,
                parent_incarnation: None,
                slave_lives: BTreeMap::new(),
            });
        state.slave_object_ids.retain(|child| {
            self.native_monsters.get(child).is_some_and(|m| {
                !m.dead
                    && m.hp > 0
                    && self.objects.contains_key(child)
                    && m.special_ai
                        .as_ref()
                        .and_then(|s| s.stage_summons.as_ref())
                        .is_some_and(|s| {
                            s.parent_object_id == Some(id)
                                && s.parent_incarnation
                                    .is_none_or(|life| life == boss.incarnation)
                        })
                    && state
                        .slave_lives
                        .get(child)
                        .is_none_or(|life| *life == m.incarnation)
            })
        });
        state
            .slave_lives
            .retain(|child, _| state.slave_object_ids.contains(child));
        let has_stage = boss.max_hp >= i32::from(stages);
        let stage = if has_stage {
            (boss.hp / (boss.max_hp / i32::from(stages))).min(255) as u8
        } else {
            state.stage
        };
        // BoneLord checks phases in ProcessTarget, without a CanAttack gate;
        // ZumaTaurus does so in ProcessAI even with no target, and tracks heals.
        let trigger = has_stage && stage < state.stage && (boss.ai == 17 || target.is_some());
        if boss.ai == 17 || trigger {
            state.stage = stage;
        }
        let mut out = Vec::new();
        if trigger {
            if boss.ai == 30 {
                let attack = ServerPacket::ObjectAttack {
                    info: ObjectAttackInfo {
                        object_id: id,
                        location: boss.position.clone(),
                        direction: boss.direction,
                        spell: 0,
                        level: 0,
                        attack_type: 1,
                    },
                };
                self.apply_zone_object_packets(&[attack.clone()], now_ms);
                let recipients = self.native_monster_visible_recipients(id, &boss.position);
                if !recipients.is_empty() {
                    out.push(ZoneOutbound::ToMany {
                        session_ids: recipients,
                        packets: vec![attack],
                    });
                }
                let m = self.native_monsters.get_mut(&id).expect("boss exists");
                m.next_ai_ready_at_ms = now_ms.saturating_add(300);
                m.next_attack_ready_at_ms = now_ms.saturating_add(boss.attack_speed_ms);
            }
            let names: &[&str] = if boss.ai == 30 {
                &BONE_SLAVES
            } else {
                &ZUMA_SLAVES
            };
            for slot in 0..8usize.min(40usize.saturating_sub(state.slave_object_ids.len())) {
                // Replay-stable equal-weight sampling, not Crystal's mutable
                // System.Random stream; this RNG difference remains explicit.
                let mut roll =
                    now_ms ^ (u64::from(id) << 32) ^ (u64::from(stage) << 16) ^ slot as u64;
                roll = roll.wrapping_add(0x9E3779B97F4A7C15);
                roll = (roll ^ (roll >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                roll = (roll ^ (roll >> 27)).wrapping_mul(0x94D049BB133111EB);
                roll ^= roll >> 31;
                let Some(template) =
                    crystal_monster_by_name(names[(roll % names.len() as u64) as usize])
                else {
                    continue;
                };
                let Some(position) = self.stage_slave_position(&boss.position, boss.direction)
                else {
                    break;
                };
                let Some(child_id) = self.stage_slave_object_id() else {
                    break;
                };
                let spawn = ZoneMonsterSpawn {
                    object_id: child_id,
                    name: template.name.clone(),
                    name_colour_argb: -1,
                    image: template.image,
                    ai: template.ai,
                    disposition: boss.disposition,
                    level: template.level,
                    max_hp: template.hp.max(1),
                    hp: template.hp.max(1),
                    experience: template.experience,
                    move_speed_ms: u64::from(template.move_speed),
                    attack_speed_ms: u64::from(template.attack_speed),
                    friendly_guild: boss.friendly_guild.clone(),
                    defense: super::super::types::ZoneMonsterDefense::from_crystal_template(
                        &template,
                    ),
                    position,
                    direction: boss.direction,
                    respawn: None,
                    drops: if mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name)
                        .is_some_and(|map| map.no_drop_monster)
                    {
                        Vec::new()
                    } else {
                        crate::runtime::drops::zone_ground_drop_snapshots_for_monster_at_tick(
                            child_id,
                            &template.name,
                            now_ms / 300,
                        )
                    },
                };
                let (spawned, emitted) = self.spawn_authoritative_monster(&spawn, now_ms);
                if !spawned {
                    continue;
                }
                out.extend(emitted);
                let child = self
                    .native_monsters
                    .get_mut(&child_id)
                    .expect("authoritative child spawn");
                child.next_ai_ready_at_ms = now_ms.saturating_add(2_000);
                child.next_attack_ready_at_ms = now_ms.saturating_add(2_000);
                child
                    .special_ai
                    .get_or_insert_with(Default::default)
                    .stage_summons = Some(StageSummonState {
                    stage: 0,
                    slave_object_ids: Vec::new(),
                    parent_object_id: Some(id),
                    inherited_target_object_id: target.as_ref().map(|t| t.object_id),
                    target_bound: true,
                    parent_incarnation: Some(boss.incarnation),
                    slave_lives: BTreeMap::new(),
                });
                state.slave_lives.insert(child_id, child.incarnation);
                if let Some(target) = &target {
                    self.set_native_entity_target(child_id, &target.reference, now_ms);
                }
                state.slave_object_ids.push(child_id);
            }
        }
        self.native_monsters
            .get_mut(&id)
            .expect("boss survives spawn")
            .special_ai
            .get_or_insert_with(Default::default)
            .stage_summons = Some(state);
        out
    }

    fn stage_slave_object_id(&mut self) -> Option<u32> {
        let mut id = self.next_object_id.max(1);
        while self.object_id_in_use(id) {
            id = id.checked_add(1)?;
        }
        self.next_object_id = id.saturating_add(1);
        Some(id)
    }

    fn stage_slave_position(&self, center: &Point, direction: MirDirection) -> Option<Point> {
        let front = offset_point(center, direction, 1);
        if self.can_native_monster_occupy(0, &front) {
            return Some(front);
        }
        for radius in 1..=8i32 {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    let Some(x) = center.x.checked_add(dx) else {
                        continue;
                    };
                    let Some(y) = center.y.checked_add(dy) else {
                        continue;
                    };
                    let p = Point { x, y };
                    if self.can_native_monster_occupy(0, &p) {
                        return Some(p);
                    }
                }
            }
        }
        None
    }
}
