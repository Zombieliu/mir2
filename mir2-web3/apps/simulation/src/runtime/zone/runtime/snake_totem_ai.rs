//! SnakeTotem62's production and life-bound hostile monster aggro.
use super::entity_combat::EntityTargetPurpose;
use super::*;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SnakeTotemState {
    pub search_at: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retired_child: bool,
}
pub(in crate::runtime::zone) fn initialize_snake_totem(
    m: &mut ZoneNativeMonster,
    id: u32,
    now: u64,
) {
    if m.ai != 62 {
        return;
    }
    m.special_ai
        .get_or_insert_with(Default::default)
        .snake_totem
        .get_or_insert_with(|| SnakeTotemState {
            search_at: now + zone_roll_stat_range(0, 2999, now, id, 62) as u64,
            retired_child: false,
        });
    m.direction = MirDirection::Up;
}
impl ZoneRuntime {
    /// Called before removal/replacement while the retiring totem still exists.
    /// Tag the current child instances, never rescan the reused parent id later.
    pub(super) fn retire_shared_snake_totem_children(&mut self, id: u32) {
        if !self.native_monsters.get(&id).is_some_and(|m| m.ai == 62) {
            return;
        }
        let children: BTreeSet<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&child, m)| (m.ai == 63 && m.master_object_id == id).then_some(child))
            .collect();
        for child in &children {
            let m = self.native_monsters.get_mut(child).unwrap();
            m.special_ai
                .get_or_insert_with(Default::default)
                .snake_totem
                .get_or_insert_with(Default::default)
                .retired_child = true;
            m.next_ai_ready_at_ms = u64::MAX;
        }
        self.pending_native_hits
            .retain(|h| !children.contains(&h.attacker_object_id));
        self.pending_native_player_hits
            .retain(|h| !children.contains(&h.attacker_object_id));
    }
    pub(super) fn expire_retired_snake_children(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let children: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                (m.ai == 63
                    && !m.dead
                    && m.special_ai
                        .as_ref()
                        .and_then(|s| s.snake_totem.as_ref())
                        .is_some_and(|s| s.retired_child))
                .then_some(id)
            })
            .collect();
        children
            .into_iter()
            .flat_map(|id| self.explode_native_charmed_snake(id, now))
            .collect()
    }
    pub(super) fn tick_shared_snake_totem(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let Some(totem) = self
            .native_monsters
            .get(&id)
            .filter(|m| m.ai == 62 && !m.dead)
            .cloned()
        else {
            return Vec::new();
        };
        let Some(owner) = totem.owner_session_id.clone() else {
            return Vec::new();
        };
        let owner_id = zone_native_summon_owner_player_object_id(&totem);
        if !self
            .players
            .get(&owner)
            .is_some_and(|p| p.object_id == owner_id && !p.dead)
        {
            return Vec::new();
        }
        if native_monster_control_active(&totem, now) || now < totem.next_ai_ready_at_ms {
            return Vec::new();
        }
        let search = totem
            .special_ai
            .as_ref()
            .and_then(|s| s.snake_totem.as_ref())
            .map_or(0, |s| s.search_at);
        if now < search {
            return Vec::new();
        }
        let live = self.native_monsters.get_mut(&id).unwrap();
        live.direction = MirDirection::Up;
        live.special_ai
            .get_or_insert_with(Default::default)
            .snake_totem
            .get_or_insert_with(Default::default)
            .search_at = now + 3000;
        // Crystal aggro runs every search even when the minion cap is full.
        if let Some(target) = self.native_entity_monster_ref(id) {
            let range = crystal_monster_by_name(&totem.name).map_or(0, |t| i32::from(t.view_range));
            let enemies: Vec<_> = self
                .native_monsters
                .iter()
                .filter_map(|(&enemy_id, enemy)| {
                    if enemy_id == id
                        || zone_tile_distance(&totem.position, &enemy.position) > range
                        || crystal_monster_by_name(&enemy.name).is_some_and(|t| t.cool_eye == 100)
                    {
                        return None;
                    }
                    let hidden = self.objects.get(&enemy_id).is_some_and(|o|
                    matches!(&o.packet, ServerPacket::ObjectMonster { info } if info.hidden));
                    let sees_hidden = totem.level >= enemy.level
                        && crystal_monster_by_name(&totem.name).is_some_and(|t| {
                            u64::from(t.cool_eye)
                                > crate::runtime::combat::crystal_accuracy_roll(
                                    0,
                                    totem.incarnation as u32,
                                    totem.incarnation as u32,
                                    100,
                                )
                        });
                    if hidden && !sees_hidden {
                        return None;
                    }
                    self.native_entity_can_attack(
                        &self.native_entity_monster_ref(enemy_id)?,
                        &target,
                        EntityTargetPurpose::Search,
                        now,
                    )
                    .then_some(enemy_id)
                })
                .collect();
            for enemy in enemies {
                if self.native_monsters.get(&enemy).is_some_and(|m| m.ai == 36) {
                    self.yimoogi_set_attacker_target(enemy, &target, now);
                } else {
                    self.set_native_entity_target(enemy, &target, now);
                }
            }
        }
        let cap = usize::from(totem.summon_skill_level) + 1;
        if self.active_native_totem_minion_count(id) >= cap {
            return Vec::new();
        }
        let Some(t) = crystal_monster_by_name("CharmedSnake") else {
            return Vec::new();
        };
        // Crystal overlaps this minion with its totem. Shared occupancy requires a
        // vacant adjacent tile; never silently place a new entity in an occupied cell.
        let Some(position) = (0..8)
            .map(|n| {
                offset_point(
                    &totem.position,
                    zone_rotated_direction(MirDirection::Up, n),
                    1,
                )
            })
            .find(|p| self.can_occupy(p, None))
        else {
            return Vec::new();
        };
        let child = self.unique_object_id(0);
        let spawn = ZoneMonsterSpawn {
            object_id: child,
            name: t.name.clone(),
            name_colour_argb: -1,
            image: t.image,
            ai: t.ai,
            disposition: Some(crate::WorldEntityDisposition::Friendly),
            level: t.level,
            max_hp: t.hp.max(1),
            hp: t.hp.max(1),
            experience: 0,
            move_speed_ms: u64::from(t.move_speed),
            attack_speed_ms: u64::from(t.attack_speed),
            friendly_guild: totem.friendly_guild.clone(),
            defense: super::super::types::ZoneMonsterDefense::from_crystal_template(&t),
            position,
            direction: MirDirection::Up,
            respawn: None,
            drops: Vec::new(),
        };
        let mut minion = ZoneNativeMonster::from_spawn(&spawn, child);
        minion.hostile_to_player = false;
        minion.owner_session_id = Some(owner);
        minion.master_object_id = id;
        minion.owner_player_object_id = owner_id;
        minion.summon_skill_level = totem.summon_skill_level;
        minion.visible_extra = true;
        minion.next_ai_ready_at_ms = now + 1000;
        minion.incarnation = self.allocate_monster_incarnation();
        self.native_monsters.insert(child, minion);
        let packet = native_summon_spawn_packet(&spawn, child, id);
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        if let Some(object) = self.objects.get_mut(&child) {
            object.expires_at_ms = Some(now + 10000 + u64::from(totem.summon_skill_level) * 2000);
        }
        self.diff_all_zone_object_visibility()
    }
}

#[cfg(test)]
mod retirement_tests {
    use super::*;
    use mir2_protocol::{MirClass, MirGender};
    #[test]
    fn owned_totem_retirement_does_not_claim_new_same_id_children() {
        let mut z = ZoneRuntime::new(ZoneKey::for_map("snake-retirement-fixture"));
        let owner = SessionId::new("owner");
        z.handle(ZoneCommand::Join(ZoneJoin {
            session_id: owner.clone(),
            account_id: "owner".into(),
            character_index: 1,
            object_id: 101,
            name: "owner".into(),
            class: MirClass::Archer,
            gender: MirGender::Male,
            level: 50,
            hp: 1000,
            max_hp: 1000,
            mp: 100,
            map_file_name: "snake-retirement-fixture".into(),
            position: Point { x: 100, y: 100 },
            direction: MirDirection::Right,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        z.handle(ZoneCommand::PlayerCastMagic {
            session_id: owner,
            object_id: 0,
            spell: Spell::SummonSnakes,
            direction: MirDirection::Right,
            target: Point { x: 104, y: 100 },
            cast: true,
            level: 2,
            damage: 0,
            mp_cost: 0,
            cooldown_ms: 1000,
            now_ms: 10,
        });
        for now in [1210, 4210, 7210] {
            z.tick(now);
        }
        let parent = *z
            .native_monsters
            .iter()
            .find(|(_, m)| m.ai == 62)
            .unwrap()
            .0;
        let old_parent = z.native_monsters[&parent].clone();
        let old_children: Vec<_> = z
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| (m.ai == 63 && m.master_object_id == parent).then_some(id))
            .collect();
        assert!(old_children.len() >= 2);
        let reused = old_children[0];
        let old_child = z.native_monsters[&reused].clone();
        z.despawn_world_event_monster(parent, 7211);
        assert!(old_children.iter().all(|id| z.native_monsters[id]
            .special_ai
            .as_ref()
            .unwrap()
            .snake_totem
            .as_ref()
            .unwrap()
            .retired_child));
        // Trusted producer fixture recreates ownership the same way as the summon
        // producer; this is a real new incarnation, not a modified checkpoint.
        let make_spawn = |id: u32, m: &ZoneNativeMonster| ZoneMonsterSpawn {
            object_id: id,
            name: m.name.clone(),
            name_colour_argb: -1,
            image: crystal_monster_by_name(&m.name).unwrap().image,
            ai: m.ai,
            disposition: Some(crate::WorldEntityDisposition::Friendly),
            level: m.level,
            max_hp: m.max_hp,
            hp: m.max_hp,
            experience: 0,
            move_speed_ms: m.move_speed_ms,
            attack_speed_ms: m.attack_speed_ms,
            friendly_guild: None,
            defense: Default::default(),
            position: m.position.clone(),
            direction: MirDirection::Up,
            respawn: None,
            drops: Vec::new(),
        };
        assert!(
            z.spawn_world_event_monster(&make_spawn(parent, &old_parent), 7211)
                .0
        );
        z.despawn_world_event_monster(reused, 7211);
        assert!(
            z.spawn_world_event_monster(&make_spawn(reused, &old_child), 7211)
                .0
        );
        for (id, previous) in [(parent, &old_parent), (reused, &old_child)] {
            let m = z.native_monsters.get_mut(&id).unwrap();
            m.owner_session_id = previous.owner_session_id.clone();
            m.master_object_id = previous.master_object_id;
            m.owner_player_object_id = previous.owner_player_object_id;
            m.summon_skill_level = previous.summon_skill_level;
            m.visible_extra = true;
            m.next_ai_ready_at_ms = 8211;
            let packet = native_summon_spawn_packet(
                &make_spawn(id, previous),
                id,
                previous.master_object_id,
            );
            z.apply_zone_object_packets(std::slice::from_ref(&packet), 7211);
        }
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(z.tick(7212), restored.tick(7212));
        assert!(!z.native_monsters[&parent].dead);
        assert!(!z.native_monsters[&reused].dead);
        assert_eq!(z.native_monsters[&reused].master_object_id, parent);
        for old in &old_children[1..] {
            assert!(!z.native_monsters.get(old).is_some_and(|m| !m.dead));
        }
        assert_eq!(
            z.checkpoint_bytes().unwrap(),
            restored.checkpoint_bytes().unwrap()
        );
    }
}
