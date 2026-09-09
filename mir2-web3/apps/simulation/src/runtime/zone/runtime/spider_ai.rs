//! Crystal RootSpider (AI39) summons and BombSpider (AI40) delayed explosions.
//!
//! Source mapping matters: AI39 is RootSpider, AI41/42 are YinDevilNode.
//! AI41/42 friendly-buff mechanisms remain a separate pending leaf.
//! Spawn collision safety differs from Crystal: an occupied or blocked output
//! tile searches free cells within eight tiles instead of stacking creatures.
//! The imported default BombSpider name is used; server-name overrides and
//! parent/child spawn scripts remain general content-engine integration gaps.
//! Wild-to-player/owned-monster targets use typed shared combat. Hero and pet PvP
//! remain outside this leaf. Map-owned spawns retain their original producer life.
use super::entity_combat::{
    EntityDefence, EntityTargetPurpose, NativeEntityTarget, ZoneCombatEntityRef,
};
use super::*;

const BOMB_AI: u8 = 40;
const ROOT_AI: u8 = 39;
const ROOT_SLAVE_LIMIT: usize = 20;
const ROOT_ATTACK_INTERVAL_MS: u64 = 3_000;
const ROOT_SPAWN_DELAY_MS: u64 = 500;
const BOMB_LIFETIME_MS: u64 = 300_000;
const BOMB_DEATH_DELAY_MS: u64 = 500;
const BOMB_CORPSE_MS: u64 = 180_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SpiderAiState {
    pub explosion_deadline_ms: u64,
    pub explosion_at_ms: Option<u64>,
    pub corpse_expires_at_ms: Option<u64>,
    pub exploded: bool,
    pub target_object_id: Option<u32>,
    pub reward_owner_session_id: Option<SessionId>,
    pub reward_owner_until_ms: u64,
    pub rng_state: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<RootSpiderState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_object_id: Option<u32>,
    #[serde(default)]
    pub parent_ref: Option<ZoneCombatEntityRef>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RootSpiderState {
    pub slave_object_ids: Vec<u32>,
    pub pending_spawn: Option<PendingRootSpiderSpawn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingRootSpiderSpawn {
    #[serde(default)]
    pub source: Option<ZoneCombatEntityRef>,
    #[serde(default)]
    pub target: Option<ZoneCombatEntityRef>,
    pub created_at_ms: u64,
    pub due_at_ms: u64,
    pub target_object_id: u32,
    pub position: Point,
    pub direction: MirDirection,
    pub drop_roll: u64,
}

impl SpiderAiState {
    fn roll_below(&mut self, upper: u64) -> u64 {
        let threshold = upper.wrapping_neg() % upper;
        loop {
            self.rng_state = self.rng_state.wrapping_add(0x9E3779B97F4A7C15);
            let mut value = self.rng_state;
            value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
            value ^= value >> 31;
            if value >= threshold {
                return value % upper;
            }
        }
    }
}

pub(in crate::runtime::zone) fn initialize_spider_monster(
    monster: &mut ZoneNativeMonster,
    object_id: u32,
    now_ms: u64,
) {
    if !matches!(monster.ai, ROOT_AI | BOMB_AI) {
        return;
    }
    let special = monster.special_ai.get_or_insert_with(Default::default);
    if special.spider.is_some() {
        return;
    }
    let mut state = SpiderAiState {
        explosion_deadline_ms: now_ms.saturating_add(BOMB_LIFETIME_MS),
        explosion_at_ms: None,
        corpse_expires_at_ms: None,
        exploded: false,
        target_object_id: None,
        reward_owner_session_id: None,
        reward_owner_until_ms: 0,
        rng_state: now_ms ^ (u64::from(object_id) << 32) ^ 0xB04B_5A1D_EA7A,
        root: None,
        parent_object_id: None,
        parent_ref: None,
    };
    if monster.ai == ROOT_AI {
        monster.direction = match state.roll_below(3) {
            0 => MirDirection::Up,
            1 => MirDirection::UpRight,
            _ => MirDirection::Right,
        };
        state.root = Some(RootSpiderState::default());
        // MonsterObject.Spawned sets ActionTime, after construction.
        monster.next_ai_ready_at_ms = monster
            .next_ai_ready_at_ms
            .max(now_ms.saturating_add(2_000));
    }
    special.spider = Some(state);
}

pub(super) fn sync_spider_monster_packet(monster: &ZoneNativeMonster, packet: &mut ServerPacket) {
    if monster.ai == ROOT_AI {
        if let ServerPacket::ObjectMonster { info } = packet {
            info.direction = monster.direction;
        }
    }
}

/// Call for positive applied damage before the central alive -> dead decision.
/// Remember the attacking session so a damaged spider's subsequent suicide
/// can use the established authoritative reward/drop path exactly once.
pub(super) fn spider_monster_on_damage(
    monster: &mut ZoneNativeMonster,
    object_id: u32,
    owner: Option<&SessionId>,
    now_ms: u64,
) {
    if monster.ai != BOMB_AI {
        return;
    }
    initialize_spider_monster(monster, object_id, now_ms);
    let Some(owner) = owner else { return };
    let state = monster
        .special_ai
        .as_mut()
        .unwrap()
        .spider
        .as_mut()
        .unwrap();
    if state.reward_owner_session_id.is_none() || now_ms > state.reward_owner_until_ms {
        state.reward_owner_session_id = Some(owner.clone());
    }
    if state.reward_owner_session_id.as_ref() == Some(owner) {
        state.reward_owner_until_ms = now_ms.saturating_add(5_000);
    }
}

/// Invoke once at every central alive -> dead transition, including poison.
/// The guard also makes a repeated death notification incapable of scheduling
/// a second explosion. Ordinary same-ID respawn must wait for this action.
pub(super) fn spider_monster_on_death(
    monster: &mut ZoneNativeMonster,
    object_id: u32,
    now_ms: u64,
) {
    if !matches!(monster.ai, ROOT_AI | BOMB_AI) || !monster.dead {
        return;
    }
    initialize_spider_monster(monster, object_id, now_ms);
    let state = monster
        .special_ai
        .as_mut()
        .unwrap()
        .spider
        .as_mut()
        .unwrap();
    if monster.ai == ROOT_AI {
        state
            .corpse_expires_at_ms
            .get_or_insert(now_ms.saturating_add(BOMB_CORPSE_MS));
        // A Map-owned delayed Spawn survives its producer's death.
    } else if !state.exploded && state.explosion_at_ms.is_none() {
        state.explosion_at_ms = Some(now_ms.saturating_add(BOMB_DEATH_DELAY_MS));
        state.corpse_expires_at_ms = Some(now_ms.saturating_add(BOMB_CORPSE_MS));
    }
    monster.control_poison = 0;
    monster.control_until_ms = 0;
    monster.damage_poison = 0;
    monster.damage_poison_value = 0;
    monster.damage_poison_next_damage_at_ms = 0;
    monster.damage_poison_expires_at_ms = 0;
    monster.damage_poison_owner_session_id = None;
    monster.damage_poison_owner_object_id = 0;
}

impl ZoneRuntime {
    pub(super) fn native_monster_has_pending_spider_explosion(&self, object_id: u32) -> bool {
        self.native_monsters.get(&object_id).is_some_and(|m| {
            m.dead
                && m.special_ai
                    .as_ref()
                    .and_then(|s| s.spider.as_ref())
                    .is_some_and(|s| {
                        (!s.exploded && s.explosion_at_ms.is_some())
                            || s.root
                                .as_ref()
                                .is_some_and(|root| root.pending_spawn.is_some())
                    })
        })
    }

    /// Run before ordinary monster AI and respawns. AI39/40 must be excluded
    /// from generic attack dispatch: they summon / die rather than swing.
    pub(super) fn tick_spider_lifecycles(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| matches!(m.ai, ROOT_AI | BOMB_AI).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            if let Some(m) = self.native_monsters.get_mut(&id) {
                initialize_spider_monster(m, id, now_ms);
            }
            let Some(monster) = self.native_monsters.get(&id).cloned() else {
                continue;
            };
            if monster.ai == ROOT_AI {
                out.extend(self.tick_root_spider(id, now_ms));
                if monster.dead {
                    out.extend(self.finish_spider_death(id, now_ms));
                }
                continue;
            }
            if monster.dead {
                out.extend(self.finish_spider_death(id, now_ms));
                continue;
            }
            // Charmed spiders need the general pet targeting/death ownership
            // contract; keep that difference explicit rather than making a
            // player's pet explode against its own owner.
            if !monster.hostile_to_player || monster.master_object_id != 0 {
                continue;
            }
            let state = monster
                .special_ai
                .as_ref()
                .unwrap()
                .spider
                .as_ref()
                .unwrap();
            let target = self.spider_entity_target(id, &monster, now_ms);
            if let Some(target) = &target {
                self.set_native_entity_target(id, &target.reference, now_ms);
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .spider
                .as_mut()
                .unwrap()
                .target_object_id = target.as_ref().map(|t| t.object_id);
            if target.is_none()
                || target
                    .as_ref()
                    .is_some_and(|t| zone_tile_distance(&monster.position, &t.position) <= 1)
                || now_ms > state.explosion_deadline_ms
            {
                out.extend(self.kill_triggered_bomb_spider(id, now_ms));
                continue;
            }
            if now_ms < monster.next_ai_ready_at_ms
                || native_monster_control_active(&monster, now_ms)
                || !monster_visibility_can_act(&monster, now_ms)
            {
                continue;
            }
            let target = target.unwrap();
            let Some(direction) = zone_direction_toward(&monster.position, &target.position) else {
                continue;
            };
            let destination = offset_point(&monster.position, direction, 1);
            if !self.can_native_monster_occupy(id, &destination) {
                out.extend(self.turn_native_monster(id, direction, now_ms));
                continue;
            }
            let old = self.native_monster_visible_recipients(id, &monster.position);
            let m = self.native_monsters.get_mut(&id).unwrap();
            m.position = destination.clone();
            m.direction = direction;
            m.next_ai_ready_at_ms = now_ms.saturating_add(m.move_speed_ms);
            let packet = ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: id,
                    position: destination.clone(),
                    direction,
                },
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
            out.extend(self.diff_all_zone_object_visibility());
            let session_ids = self
                .native_monster_visible_recipients(id, &destination)
                .into_iter()
                .filter(|sid| old.contains(sid))
                .collect::<Vec<_>>();
            if !session_ids.is_empty() {
                out.push(ZoneOutbound::ToMany {
                    session_ids,
                    packets: vec![packet],
                });
            }
        }
        out
    }

    fn tick_root_spider(&mut self, id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
        let mut out = self.resolve_root_spider_spawn(id, now_ms);
        let Some(monster) = self.native_monsters.get(&id).cloned() else {
            return out;
        };
        let mut state = monster
            .special_ai
            .as_ref()
            .unwrap()
            .spider
            .as_ref()
            .unwrap()
            .clone();
        let parent_ref = self.native_entity_monster_ref(id);
        let root = state.root.as_mut().expect("initialized root spider");
        root.slave_object_ids.retain(|child_id| {
            self.native_monsters.get(child_id).is_some_and(|child| {
                !child.dead
                    && child.hp > 0
                    && child
                        .special_ai
                        .as_ref()
                        .and_then(|s| s.spider.as_ref())
                        .is_some_and(|s| {
                            s.parent_ref
                                .as_ref()
                                .map_or(s.parent_object_id == Some(id), |p| {
                                    Some(p) == parent_ref.as_ref()
                                })
                        })
            })
        });
        let full = root.slave_object_ids.len() + usize::from(root.pending_spawn.is_some())
            >= ROOT_SLAVE_LIMIT;
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .spider = Some(state);
        if monster.dead
            || !monster.hostile_to_player
            || monster.master_object_id != 0
            || full
            || now_ms <= monster.next_ai_ready_at_ms
            || now_ms <= monster.next_attack_ready_at_ms
            || native_monster_control_active(&monster, now_ms)
            || !monster_visibility_can_act(&monster, now_ms)
        {
            return out;
        }
        let Some(target) = self.spider_entity_target(id, &monster, now_ms) else {
            return out;
        };
        if zone_tile_distance(&monster.position, &target.position) > 16 {
            return out; // BugBagMaggot.InAttackRange uses Globals.DataRange.
        }
        if crystal_monster_by_name("BombSpider").is_none() {
            return out;
        }
        let output_direction = match monster.direction {
            MirDirection::Up => MirDirection::Down,
            MirDirection::UpRight => MirDirection::DownRight,
            MirDirection::Right => MirDirection::DownLeft,
            _ => return out,
        };
        let position = offset_point(&monster.position, output_direction, 1);
        let child_direction = match self.spider_roll(id, 8) {
            0 => MirDirection::Up,
            1 => MirDirection::UpRight,
            2 => MirDirection::Right,
            3 => MirDirection::DownRight,
            4 => MirDirection::Down,
            5 => MirDirection::DownLeft,
            6 => MirDirection::Left,
            _ => MirDirection::UpLeft,
        };
        let drop_roll = self.spider_roll(id, u64::MAX);
        self.set_native_entity_target(id, &target.reference, now_ms);
        let source = self.native_entity_monster_ref(id);
        let m = self.native_monsters.get_mut(&id).unwrap();
        m.next_ai_ready_at_ms = now_ms.saturating_add(300);
        m.next_attack_ready_at_ms = now_ms.saturating_add(ROOT_ATTACK_INTERVAL_MS);
        let state = m.special_ai.as_mut().unwrap().spider.as_mut().unwrap();
        state.target_object_id = Some(target.object_id);
        state.root.as_mut().unwrap().pending_spawn = Some(PendingRootSpiderSpawn {
            source,
            target: Some(target.reference.clone()),
            created_at_ms: now_ms,
            due_at_ms: now_ms.saturating_add(ROOT_SPAWN_DELAY_MS),
            target_object_id: target.object_id,
            position,
            direction: child_direction,
            drop_roll,
        });
        let packets = vec![ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: id,
                location: monster.position.clone(),
                direction: monster.direction,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        }];
        self.apply_zone_object_packets(&packets, now_ms);
        let session_ids = self.native_monster_visible_recipients(id, &monster.position);
        if !session_ids.is_empty() {
            out.push(ZoneOutbound::ToMany {
                session_ids,
                packets,
            });
        }
        out
    }

    fn spider_entity_target(
        &self,
        id: u32,
        monster: &ZoneNativeMonster,
        now: u64,
    ) -> Option<NativeEntityTarget> {
        if let Some(target) = self.selected_native_entity_target(id, now) {
            return Some(target);
        }
        let view = crystal_monster_by_name(&monster.name).map_or(0, |t| i32::from(t.view_range));
        let targets = self.native_entity_targets(
            id,
            &monster.position,
            view,
            EntityTargetPurpose::Search,
            now,
        );
        let remembered = monster
            .special_ai
            .as_ref()
            .and_then(|s| s.spider.as_ref())
            .and_then(|s| s.target_object_id);
        targets
            .iter()
            .find(|t| Some(t.object_id) == remembered)
            .or_else(|| targets.first())
            .cloned()
    }
    pub(super) fn clear_spider_target_life(&mut self, id: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.spider.as_mut()) {
                if s.target_object_id == Some(id) {
                    s.target_object_id = None;
                }
                if let Some(p) = s.root.as_mut().and_then(|r| r.pending_spawn.as_mut()) {
                    if p.target.as_ref().is_some_and(|t| t.object_id() == id)
                        || p.target_object_id == id
                    {
                        p.target = None;
                        p.target_object_id = 0;
                    }
                }
            }
        }
    }
    fn resolve_root_spider_spawn(&mut self, id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
        let pending = self
            .native_monsters
            .get(&id)
            .and_then(|m| m.special_ai.as_ref())
            .and_then(|s| s.spider.as_ref())
            .and_then(|s| s.root.as_ref())
            .and_then(|s| s.pending_spawn.as_ref())
            .filter(|p| now_ms >= p.due_at_ms)
            .cloned();
        let Some(pending) = pending else {
            return Vec::new();
        };
        // Consume first, including failed spawn attempts. A blocked tile or
        // missing template must not retry the same delayed action every tick.
        let parent = self.native_monsters.get_mut(&id).unwrap();
        parent
            .special_ai
            .as_mut()
            .unwrap()
            .spider
            .as_mut()
            .unwrap()
            .root
            .as_mut()
            .unwrap()
            .pending_spawn = None;
        let disposition = parent.disposition;
        let friendly_guild = parent.friendly_guild.clone();
        let source = self.native_entity_monster_ref(id);
        if pending.source.is_some() && pending.source != source {
            return Vec::new();
        }
        let target = pending.target.clone().or_else(|| {
            self.players
                .values()
                .find(|p| p.object_id == pending.target_object_id)
                .and_then(|p| self.native_entity_player_ref(&p.session_id))
        });

        let Some(template) = crystal_monster_by_name("BombSpider") else {
            return Vec::new();
        };
        let Some(position) = self.root_spider_child_position(&pending.position) else {
            return Vec::new();
        };
        let Some(child_id) = self.root_spider_child_object_id() else {
            return Vec::new();
        };
        let drops = if mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name)
            .is_some_and(|map| map.no_drop_monster)
        {
            Vec::new()
        } else {
            crate::runtime::drops::zone_ground_drop_snapshots_for_monster_at_tick(
                child_id,
                &template.name,
                pending.drop_roll ^ u64::from(child_id),
            )
        };
        let spawn = ZoneMonsterSpawn {
            object_id: child_id,
            name: template.name.clone(),
            name_colour_argb: -1,
            image: template.image,
            ai: template.ai,
            disposition,
            level: template.level,
            max_hp: template.hp.max(1),
            hp: template.hp.max(1),
            experience: template.experience,
            move_speed_ms: u64::from(template.move_speed),
            attack_speed_ms: u64::from(template.attack_speed),
            friendly_guild,
            position,
            direction: pending.direction,
            respawn: None,
            defense: super::super::types::ZoneMonsterDefense::from_crystal_template(&template),
            drops,
        };
        let (spawned, out) = self.spawn_authoritative_monster(&spawn, now_ms);
        if !spawned {
            return out;
        }
        let child = self
            .native_monsters
            .get_mut(&child_id)
            .expect("shared spider child");
        // Spawned() overrides RootSpider.Attack's earlier +1000 action time.
        child.next_ai_ready_at_ms = now_ms.saturating_add(2_000);
        child.next_attack_ready_at_ms = now_ms.saturating_add(2_000);
        let child_state = child.special_ai.as_mut().unwrap().spider.as_mut().unwrap();
        child_state.explosion_deadline_ms = pending.created_at_ms.saturating_add(BOMB_LIFETIME_MS);
        child_state.parent_object_id = Some(id);
        child_state.parent_ref = source;
        child_state.target_object_id = None;
        // SlaveList is not Master: retain a wild monster's rewards and target
        // relationship, without turning the spider into its parent's pet.
        debug_assert_eq!(child.master_object_id, 0);
        if let Some(target) = target {
            if self
                .native_entity_monster_ref(child_id)
                .is_some_and(|source| {
                    self.native_entity_can_attack(
                        &source,
                        &target,
                        EntityTargetPurpose::Impact,
                        now_ms,
                    )
                })
            {
                self.native_monsters
                    .get_mut(&child_id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .spider
                    .as_mut()
                    .unwrap()
                    .target_object_id = Some(target.object_id());
                self.set_native_entity_target(child_id, &target, now_ms);
            }
        }
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .spider
            .as_mut()
            .unwrap()
            .root
            .as_mut()
            .unwrap()
            .slave_object_ids
            .push(child_id);
        out
    }

    fn root_spider_child_object_id(&mut self) -> Option<u32> {
        let mut id = self.next_object_id.max(1);
        while self.object_id_in_use(id) {
            id = id.checked_add(1)?;
        }
        self.next_object_id = id.saturating_add(1);
        Some(id)
    }

    fn root_spider_child_position(&self, center: &Point) -> Option<Point> {
        if self.can_native_monster_occupy(0, center) {
            return Some(center.clone());
        }
        for radius in 1..=8i32 {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    let (Some(x), Some(y)) = (center.x.checked_add(dx), center.y.checked_add(dy))
                    else {
                        continue;
                    };
                    let position = Point { x, y };
                    if self.can_native_monster_occupy(0, &position) {
                        return Some(position);
                    }
                }
            }
        }
        None
    }

    fn kill_triggered_bomb_spider(&mut self, id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
        let m = &self.native_monsters[&id];
        let hp = m.hp;
        let reward_owner = m
            .special_ai
            .as_ref()
            .and_then(|s| s.spider.as_ref())
            .filter(|s| now_ms <= s.reward_owner_until_ms)
            .and_then(|s| s.reward_owner_session_id.as_ref())
            .filter(|session| {
                self.players
                    .get(*session)
                    .is_some_and(|p| !p.dead && p.hp > 0)
            })
            .cloned();
        let Some((_, _, killed, name, experience, position, direction, _, drops, boss_audit)) =
            self.apply_native_monster_damage(id, hp, reward_owner.as_ref(), now_ms)
        else {
            return Vec::new();
        };
        if !killed {
            return Vec::new();
        }
        let packets = vec![
            ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: id,
                    location: position.clone(),
                    direction,
                    kind: 0,
                },
            },
            ServerPacket::ObjectPoisoned {
                object_id: id,
                poison: 0,
            },
        ];
        self.apply_zone_object_packets(&packets, now_ms);
        let session_ids = self.native_monster_visible_recipients(id, &position);
        let mut out = Vec::new();
        if !session_ids.is_empty() {
            out.push(ZoneOutbound::ToMany {
                session_ids,
                packets,
            });
        }
        // An untagged suicide has no EXP owner in Crystal and creates no loot.
        if let Some(owner) = reward_owner {
            let player_id = self.players[&owner].object_id;
            let drops = self.spawn_native_monster_drops(&name, &position, player_id, drops, now_ms);
            out.extend(self.diff_all_zone_object_visibility());
            out.extend(self.group_monster_kill_awards(
                &owner,
                ZoneMonsterKillAward {
                    monster_object_id: id,
                    killed_at_ms: now_ms,
                    monster_name: name,
                    experience,
                    drops,
                    boss_audit,
                },
            ));
        }
        out
    }

    fn finish_spider_death(&mut self, id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
        let mut out = Vec::new();
        let monster = self.native_monsters[&id].clone();
        let state = monster
            .special_ai
            .as_ref()
            .unwrap()
            .spider
            .as_ref()
            .unwrap();
        // Also remove a poison tint after a player/DoT kill, not only suicide.
        if self.objects.get(&id).is_some_and(|object|
            matches!(&object.packet, ServerPacket::ObjectMonster { info } if info.poison != 0)) {
            let packets = vec![ServerPacket::ObjectPoisoned { object_id: id, poison: 0 }];
            self.apply_zone_object_packets(&packets, now_ms);
            let session_ids = self.native_monster_visible_recipients(id, &monster.position);
            if !session_ids.is_empty() { out.push(ZoneOutbound::ToMany { session_ids, packets }); }
        }
        if !state.exploded && state.explosion_at_ms.is_some_and(|due| now_ms >= due) {
            // Commit before producing damage; repeat ticks and restored
            // checkpoints can never execute the same delayed action twice.
            let s = self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .spider
                .as_mut()
                .unwrap();
            s.exploded = true;
            s.explosion_at_ms = None;
            out.extend(self.explode_bomb_spider(id, &monster, now_ms));
        }
        if state.corpse_expires_at_ms.is_some_and(|due| now_ms >= due) {
            self.retire_native_source_object(id);
            self.objects.remove(&id);
            self.object_grid.remove(&id);
            self.dead_object_ids.remove(&id);
            self.revived_object_ids.remove(&id);
            self.removed_object_ids.insert(id);
            if self.native_monster_respawns.contains_key(&id) {
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .spider
                    .as_mut()
                    .unwrap()
                    .corpse_expires_at_ms = None;
            } else {
                self.native_monsters.remove(&id);
            }
            let session_ids = self
                .players
                .iter_mut()
                .filter_map(|(session, p)| {
                    p.visible_object_ids.remove(&id).then_some(session.clone())
                })
                .collect::<Vec<_>>();
            if !session_ids.is_empty() {
                out.push(ZoneOutbound::ToMany {
                    session_ids,
                    packets: vec![ServerPacket::ObjectRemove { object_id: id }],
                });
            }
        }
        out
    }

    fn explode_bomb_spider(
        &mut self,
        id: u32,
        monster: &ZoneNativeMonster,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        // needSight=false: current impact positions include hidden valid targets.
        let Some(source) = self.native_entity_monster_ref(id) else {
            return Vec::new();
        };
        let targets = self.native_entity_targets(
            id,
            &monster.position,
            1,
            EntityTargetPurpose::Impact,
            now_ms,
        );
        let Some(template) = crystal_monster_by_name(&monster.name) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for target in targets {
            let damage = zone_roll_stat_range(
                template.min_dc,
                template
                    .max_dc
                    .saturating_add(zone_native_monster_buff_stat_total(
                        monster,
                        CRYSTAL_STAT_MAX_DC,
                    )),
                now_ms,
                id,
                self.spider_roll(id, u64::MAX),
            );
            if damage == 0 {
                break;
            } // source returns for zero roll, but CONTINUES for resisted hits.
            let (actual, events) = self.resolve_native_entity_hit(
                &source,
                &target.reference,
                damage,
                EntityDefence::ACAgility,
                false,
                now_ms,
            );
            out.extend(events);
            let resist = self.native_entity_poison_resist(&target.reference).max(0) as u64;
            if actual <= 0 || self.spider_roll(id, 10) < resist || self.spider_roll(id, 5) != 0 {
                continue;
            }
            let value = zone_roll_stat_range(
                template.min_sc,
                template
                    .max_sc
                    .saturating_add(zone_native_monster_buff_stat_total(
                        monster,
                        CRYSTAL_STAT_MAX_SC,
                    )),
                now_ms,
                id,
                self.spider_roll(id, u64::MAX),
            );
            if matches!(&target.reference, ZoneCombatEntityRef::Player { .. })
                && self.spider_roll(id, 10) < resist
            {
                continue;
            }
            out.extend(self.apply_native_entity_green_poison(
                &source,
                &target.reference,
                value,
                5,
                2000,
                now_ms,
            ));
        }
        out
    }

    fn spider_roll(&mut self, id: u32, upper: u64) -> u64 {
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .spider
            .as_mut()
            .unwrap()
            .roll_below(upper)
    }
}
