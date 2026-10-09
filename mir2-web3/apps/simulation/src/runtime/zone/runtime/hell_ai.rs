//! Crystal HellKnight 97 / HellLord 98 / HellBomb 99 shared encounter state.
use super::entity_combat::{EntityDefence, EntityTargetPurpose, ZoneCombatEntityRef};
use super::*;
use crate::WorldEntityDisposition;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct HellState {
    pub stage: u8,
    pub begun: bool,
    pub raged: bool,
    pub rage_until: u64,
    pub explosion_time: u64,
    pub explosion_due: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explosion_source_ref: Option<ZoneCombatEntityRef>,
    pub parent: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_incarnation: Option<u64>,
    pub death_notified: bool,
    pub rng: u64,
    pub reward_owner: Option<SessionId>,
    pub reward_until: u64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct HellWorldState {
    pub summons: Vec<HellSummon>,
    pub stage_updates: Vec<u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stage_update_lives: BTreeMap<u32, u64>,
    pub quakes: Vec<HellQuake>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HellSummon {
    pub parent: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_incarnation: Option<u64>,
    pub name: String,
    pub position: Point,
    pub due: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HellQuake {
    pub id: u32,
    pub position: Point,
    pub variant: u8,
    pub value: i32,
    pub start: u64,
    pub expires: u64,
    pub next_tick: u64,
    pub spawned: bool,
}
pub(in crate::runtime::zone) fn initialize_hell_monster(m: &mut ZoneNativeMonster, now: u64) {
    if !matches!(m.ai, 97..=99) {
        return;
    }
    let state = m
        .special_ai
        .get_or_insert_with(Default::default)
        .hell
        .get_or_insert_with(|| HellState {
            explosion_time: now.saturating_add(10000),
            ..Default::default()
        });
    let _ = state;
    if matches!(m.ai, 98 | 99) {
        m.direction = MirDirection::Up;
    }
}
pub(super) fn hell_can_move(m: &ZoneNativeMonster) -> bool {
    !matches!(m.ai, 98 | 99)
}
pub(super) fn hell_accepts_struck(m: &ZoneNativeMonster) -> bool {
    m.ai != 99
}
pub(super) fn hell_can_regen(m: &ZoneNativeMonster) -> bool {
    !matches!(m.ai, 98 | 99)
}
pub(super) fn hell_accepts_poison(m: &ZoneNativeMonster) -> bool {
    !matches!(m.ai, 98 | 99)
}
pub(super) fn hell_accepts_attack(m: &ZoneNativeMonster) -> bool {
    m.ai != 98
        || m.special_ai
            .as_ref()
            .and_then(|s| s.hell.as_ref())
            .is_some_and(|s| s.stage >= 4)
}
pub(super) fn sync_hell_packet(m: &ZoneNativeMonster, packet: &mut ServerPacket) {
    if let ServerPacket::ObjectMonster { info } = packet {
        if m.ai == 97 {
            info.extra = true;
        }
        if m.ai == 98 {
            info.extra_byte = m
                .special_ai
                .as_ref()
                .and_then(|s| s.hell.as_ref())
                .map_or(0, |s| s.stage);
        }
        if matches!(m.ai, 98 | 99) {
            info.direction = MirDirection::Up;
        }
    }
}
pub(super) fn hell_on_damage(m: &mut ZoneNativeMonster, owner: Option<&SessionId>, now: u64) {
    if m.ai != 99 {
        return;
    }
    initialize_hell_monster(m, now);
    let Some(owner) = owner else {
        return;
    };
    let s = m.special_ai.as_mut().unwrap().hell.as_mut().unwrap();
    if s.reward_owner.is_none() || now > s.reward_until {
        s.reward_owner = Some(owner.clone());
    }
    if s.reward_owner.as_ref() == Some(owner) {
        s.reward_until = now.saturating_add(5000);
    }
}
impl ZoneRuntime {
    // Retiring a Node must not cancel the map's delayed Spawn. Its captured
    // Lord reference stays detached from any replacement with the same ID.
    pub(super) fn retire_hell_parent(&mut self, id: u32) {
        if let Some(world) = self.hell_world.as_mut() {
            world.stage_updates.retain(|parent| *parent != id);
            world.stage_update_lives.remove(&id);
        }
    }
    pub(super) fn clear_hell_targets(&mut self, id: u32) {
        let session = self
            .players
            .values()
            .find(|p| p.object_id == id)
            .map(|p| p.session_id.clone());
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.hell.as_mut()) {
                if session.is_some() && s.reward_owner == session {
                    s.reward_owner = None;
                }
            }
        }
    }
    fn hell_roll(&mut self, id: u32, upper: u64) -> u64 {
        if upper == 0 {
            return 0;
        }
        let s = self
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .hell
            .as_mut()
            .unwrap();
        if s.rng == 0 {
            s.rng = (u64::from(id) << 32) ^ 0x9849;
        }
        loop {
            s.rng = s.rng.wrapping_add(0x9E3779B97F4A7C15);
            let mut n = s.rng;
            n = (n ^ (n >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            n = (n ^ (n >> 27)).wrapping_mul(0x94D049BB133111EB);
            n ^= n >> 31;
            if n >= upper.wrapping_neg() % upper {
                return n % upper;
            }
        }
    }
    fn hell_broadcast(
        &mut self,
        id: u32,
        packets: Vec<ServerPacket>,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let pos = self.native_monsters[&id].position.clone();
        self.apply_zone_object_packets(&packets, now);
        let session_ids = self.native_monster_visible_recipients(id, &pos);
        if session_ids.is_empty() {
            Vec::new()
        } else {
            vec![ZoneOutbound::ToMany {
                session_ids,
                packets,
            }]
        }
    }
    fn hell_attack_packet(&self, id: u32) -> ServerPacket {
        let m = &self.native_monsters[&id];
        ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: id,
                location: m.position.clone(),
                direction: MirDirection::Up,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        }
    }
    pub(super) fn hell_knight_died(&mut self, id: u32, now: u64) {
        let Some(m) = self.native_monsters.get_mut(&id) else {
            return;
        };
        if m.ai != 97 || !m.dead {
            return;
        }
        let Some(s) = m.special_ai.as_mut().and_then(|s| s.hell.as_mut()) else {
            return;
        };
        if s.death_notified {
            return;
        }
        s.death_notified = true;
        let Some(parent) = s.parent else {
            return;
        };
        // Legacy checkpoints have no proof of which life owned this knight.
        // Keep the child, but never attach that unresolved reference to a Lord.
        let Some(parent_incarnation) = s.parent_incarnation else {
            return;
        };
        let Some(lord) = self.native_monsters.get_mut(&parent) else {
            return;
        };
        if lord.ai != 98 || lord.incarnation != parent_incarnation {
            return;
        }
        let state = lord.special_ai.as_mut().unwrap().hell.as_mut().unwrap();
        state.stage = state.stage.wrapping_add(1);
        state.raged = true;
        state.rage_until = now.saturating_add(120000);
        if let Some(o) = self.objects.get_mut(&parent) {
            sync_hell_packet(lord, &mut o.packet);
        }
        let world = self.hell_world.get_or_insert_with(Default::default);
        world.stage_update_lives.insert(parent, parent_incarnation);
        let updates = &mut world.stage_updates;
        if !updates.contains(&parent) {
            updates.push(parent);
        }
    }
    pub(super) fn tick_hell_ai(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| matches!(m.ai, 97..=99).then_some(id))
            .collect();
        if ids.is_empty() && self.hell_world.is_none() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for &id in &ids {
            initialize_hell_monster(self.native_monsters.get_mut(&id).unwrap(), now);
        }
        for &id in &ids {
            self.hell_knight_died(id, now);
        }
        out.extend(self.tick_hell_world(now));
        for id in ids {
            let Some(m) = self.native_monsters.get(&id).cloned() else {
                continue;
            };
            let s = m
                .special_ai
                .as_ref()
                .unwrap()
                .hell
                .as_ref()
                .unwrap()
                .clone();
            if m.ai == 99 {
                if s.explosion_due.is_some_and(|due| now >= due) {
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .hell
                        .as_mut()
                        .unwrap()
                        .explosion_due = None;
                    let source = s
                        .explosion_source_ref
                        .or_else(|| self.native_entity_monster_ref(id));
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .hell
                        .as_mut()
                        .unwrap()
                        .explosion_source_ref = None;
                    if let Some(source) = source {
                        out.extend(self.hell_explode(id, &source, now));
                    }
                }
                if !m.dead
                    && m.hp > 0
                    && now > s.explosion_time
                    && !native_monster_control_active(&m, now)
                {
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .hell
                        .as_mut()
                        .unwrap()
                        .explosion_due = Some(now.saturating_add(500));
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .hell
                        .as_mut()
                        .unwrap()
                        .explosion_source_ref = Some(ZoneCombatEntityRef::Monster {
                        object_id: id,
                        incarnation: m.incarnation,
                    });
                    out.extend(self.hell_broadcast(id, vec![self.hell_attack_packet(id)], now));
                    out.extend(self.finish_native_hell_bomb_death(id, now));
                }
            } else if m.ai == 98 {
                if self.players.is_empty() {
                    if s.stage > 0 {
                        let s = self
                            .native_monsters
                            .get_mut(&id)
                            .unwrap()
                            .special_ai
                            .as_mut()
                            .unwrap()
                            .hell
                            .as_mut()
                            .unwrap();
                        s.stage = 0;
                        s.begun = false;
                    }
                    continue;
                }
                if m.dead
                    || now <= m.next_attack_ready_at_ms
                    || now < m.next_ai_ready_at_ms
                    || native_monster_control_active(&m, now)
                {
                    continue;
                }
                out.extend(self.hell_lord_turn(id, now));
            }
        }
        out
    }
    fn finish_native_hell_bomb_death(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let m = &self.native_monsters[&id];
        let s = m.special_ai.as_ref().unwrap().hell.as_ref().unwrap();
        let owner = s
            .reward_owner
            .clone()
            .filter(|sid| now <= s.reward_until && self.players.get(sid).is_some_and(|p| !p.dead));
        let Some((_, _, killed, name, experience, position, direction, _, drops, boss_audit)) =
            self.apply_native_monster_damage(id, m.hp, owner.as_ref(), now)
        else {
            return Vec::new();
        };
        if !killed {
            return Vec::new();
        }
        let mut out = self.hell_broadcast(
            id,
            vec![ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: id,
                    location: position.clone(),
                    direction,
                    kind: 0,
                },
            }],
            now,
        );
        if let Some(owner) = owner {
            let drops = self.spawn_native_monster_drops(
                &name,
                &position,
                self.players[&owner].object_id,
                drops,
                now,
            );
            out.extend(self.diff_all_zone_object_visibility());
            out.extend(self.group_monster_kill_awards(
                &owner,
                ZoneMonsterKillAward {
                    source_receipt_key: None,
                    experience_selection: None,
                    monster_object_id: id,
                    killed_at_ms: now,
                    monster_name: name,
                    experience,
                    drops,
                    boss_audit,
                },
            ));
        }
        out
    }
    fn hell_explode(
        &mut self,
        id: u32,
        source: &ZoneCombatEntityRef,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if self.native_entity_monster_ref(id).as_ref() != Some(source) {
            return Vec::new();
        }
        let m = self.native_monsters[&id].clone();
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return Vec::new();
        };
        // CompleteDeath captures its victims now, and intentionally ignores sight.
        let targets =
            self.native_entity_targets(id, &m.position, 4, EntityTargetPurpose::Impact, now);
        let mut out = Vec::new();
        for target in targets {
            let damage =
                t.min_dc + self.hell_roll(id, (t.max_dc - t.min_dc + 1).max(1) as u64) as i32;
            if damage <= 0 {
                break;
            }
            let (damaged, hit) = self.resolve_native_entity_hit(
                source,
                &target.reference,
                damage,
                EntityDefence::AC,
                false,
                now,
            );
            out.extend(hit);
            if damaged <= 0 {
                continue;
            }
            let mask = match self
                .objects
                .get(&id)
                .and_then(|o| match &o.packet {
                    ServerPacket::ObjectMonster { info } => Some(info.image),
                    _ => None,
                })
                .unwrap_or(t.image)
            {
                903 => 8,
                904 => 1024,
                905 => 128,
                _ => continue,
            };
            let value =
                t.min_sc + self.hell_roll(id, (t.max_sc - t.min_sc + 1).max(1) as u64) as i32;
            let resist = self
                .native_entity_poison_resist(&target.reference)
                .clamp(0, 10) as u64;
            if self.hell_roll(id, 10) < resist {
                continue;
            }
            let _chance = self.hell_roll(id, 1);
            // Human ApplyPoison's second roll; native Monster admission has none.
            if matches!(&target.reference, ZoneCombatEntityRef::Player { .. })
                && self.hell_roll(id, 10) < resist
            {
                continue;
            }
            match &target.reference {
                ZoneCombatEntityRef::Player {
                    session_id,
                    object_id,
                    ..
                } => {
                    if mask == 8 && self.players[session_id].poison & mask != 0 {
                        continue;
                    }
                    out.extend(self.apply_native_player_periodic_poison(
                        id, session_id, *object_id, mask, value, 5, 2000, now,
                    ));
                }
                ZoneCombatEntityRef::Monster { .. } => {
                    out.extend(self.apply_native_entity_hell_poison(
                        source,
                        &target.reference,
                        mask,
                        value,
                        5,
                        2000,
                        now,
                    ));
                }
            }
        }
        out
    }
}

impl ZoneRuntime {
    fn hell_lord_turn(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let m = self.native_monsters[&id].clone();
        let s = m
            .special_ai
            .as_ref()
            .unwrap()
            .hell
            .as_ref()
            .unwrap()
            .clone();
        let mut out = Vec::new();
        if !s.begun || (s.raged && now > s.rage_until && s.stage < 4) {
            let state = self
                .native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .hell
                .as_mut()
                .unwrap();
            state.begun = true;
            state.raged = false;
            if s.stage < 4 {
                let front = offset_point(&m.position, MirDirection::DownLeft, 12);
                for _ in 0..50 {
                    let location = Point {
                        x: front.x + self.hell_roll(id, 20) as i32 - 10,
                        y: front.y + self.hell_roll(id, 20) as i32 - 10,
                    };
                    if self.collision.is_blocked(&location) {
                        continue;
                    }
                    self.hell_world
                        .get_or_insert_with(Default::default)
                        .summons
                        .push(HellSummon {
                            parent: id,
                            parent_incarnation: Some(m.incarnation),
                            name: format!("HellKnight{}", s.stage + 1),
                            position: location,
                            due: now.saturating_add(500),
                        });
                    break;
                }
            }
            out.extend(self.hell_broadcast(id, vec![self.hell_attack_packet(id)], now));
        }
        let raged = self.native_monsters[&id]
            .special_ai
            .as_ref()
            .unwrap()
            .hell
            .as_ref()
            .unwrap()
            .raged;
        let positions: Vec<_> = self.players.values().map(|p| p.position.clone()).collect();
        if self.hell_roll(id, 3) == 0 || raged {
            let distance = 5 + self.hell_roll(id, 15) as i32;
            for pos in &positions {
                let location = Point {
                    x: pos.x + self.hell_roll(id, (distance * 2 + 1) as u64) as i32 - distance,
                    y: pos.y + self.hell_roll(id, (distance * 2 + 1) as u64) as i32 - distance,
                };
                let name = format!("HellBomb{}", 1 + self.hell_roll(id, 3));
                out.extend(self.spawn_hell_child(
                    id,
                    Some(m.incarnation),
                    &name,
                    location,
                    now,
                    false,
                ));
            }
        }
        let count = 1 + self.hell_roll(id, if raged { 9 } else { 4 });
        let distance = 5 + self.hell_roll(id, 10) as i32;
        let template = crystal_monster_by_name(&m.name);
        for pos in positions {
            for _ in 0..count {
                let mut location = Point {
                    x: pos.x + self.hell_roll(id, (distance * 2 + 1) as u64) as i32 - distance,
                    y: pos.y + self.hell_roll(id, (distance * 2 + 1) as u64) as i32 - distance,
                };
                if self.hell_roll(id, 10) == 0 {
                    location = pos.clone();
                }
                if self.collision.is_blocked(&location) {
                    continue;
                }
                let start = now.saturating_add(self.hell_roll(id, 5000));
                let variant = self.hell_roll(id, 2) as u8;
                let bound = template.as_ref().map_or(0, |t| {
                    t.min_dc + self.hell_roll(id, (t.max_dc - t.min_dc).max(1) as u64) as i32
                });
                let value = self.hell_roll(id, bound.max(0) as u64) as i32;
                let object_id = self.unique_object_id(0);
                self.hell_world
                    .get_or_insert_with(Default::default)
                    .quakes
                    .push(HellQuake {
                        id: object_id,
                        position: location,
                        variant,
                        value,
                        start,
                        expires: start.saturating_add(2000),
                        next_tick: start,
                        spawned: false,
                    });
            }
        }
        let m = self.native_monsters.get_mut(&id).unwrap();
        m.next_ai_ready_at_ms = now.saturating_add(600);
        m.next_attack_ready_at_ms = now.saturating_add(m.attack_speed_ms);
        out
    }
    fn spawn_hell_child(
        &mut self,
        parent: u32,
        parent_incarnation: Option<u64>,
        name: &str,
        position: Point,
        now: u64,
        knight: bool,
    ) -> Vec<ZoneOutbound> {
        // Crystal Spawn also requires a free tile. Never silently relocate a
        // failed random spawn to a different point in the shared world.
        if !self.can_occupy(&position, None) {
            return Vec::new();
        }
        let Some(t) = crystal_monster_by_name(name) else {
            return Vec::new();
        };
        let id = self.unique_object_id(0);
        let spawn = ZoneMonsterSpawn {
            crystal_drop_seed: Some(now / 300),
            object_id: id,
            name: t.name.clone(),
            name_colour_argb: -1,
            image: t.image,
            ai: t.ai,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: t.level,
            max_hp: t.hp.max(1),
            hp: t.hp.max(1),
            experience: t.experience,
            move_speed_ms: u64::from(t.move_speed),
            attack_speed_ms: u64::from(t.attack_speed),
            friendly_guild: None,
            defense: super::super::types::ZoneMonsterDefense::from_crystal_template(&t),
            position,
            direction: MirDirection::Up,
            respawn: None,
            drops: if mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name)
                .is_some_and(|m| m.no_drop_monster)
            {
                Vec::new()
            } else {
                crate::runtime::drops::zone_ground_drop_snapshots_for_monster_at_tick(
                    id,
                    name,
                    now / 300,
                )
            },
        };
        let (ok, out) = self.spawn_authoritative_monster(&spawn, now);
        if ok && knight {
            let m = self.native_monsters.get_mut(&id).unwrap();
            initialize_hell_monster(m, now);
            let state = m.special_ai.as_mut().unwrap().hell.as_mut().unwrap();
            state.parent = Some(parent);
            state.parent_incarnation = parent_incarnation;
        }
        out
    }
    fn tick_hell_world(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let mut state = self.hell_world.take().unwrap_or_default();
        let mut out = Vec::new();
        for id in state.stage_updates.drain(..) {
            let life = state.stage_update_lives.remove(&id);
            if !self
                .native_monsters
                .get(&id)
                .is_some_and(|m| m.ai == 98 && life == Some(m.incarnation))
            {
                continue;
            }
            if let Some(packet) = self.objects.get(&id).map(|o| o.packet.clone()) {
                out.extend(self.hell_broadcast(id, vec![packet], now));
            }
        }
        let mut future = Vec::new();
        for summon in state.summons.drain(..) {
            if now < summon.due {
                future.push(summon);
                continue;
            }
            // This is a Map delayed action: it survives the lord's death.
            out.extend(self.spawn_hell_child(
                summon.parent,
                summon.parent_incarnation,
                &summon.name,
                summon.position,
                now,
                true,
            ));
        }
        state.summons = future;
        state.quakes.retain_mut(|q| {
            if now < q.start {
                return true;
            }
            if now > q.expires {
                if q.spawned {
                    let session_ids = self
                        .players
                        .iter()
                        .filter_map(|(sid, p)| {
                            p.visible_object_ids.contains(&q.id).then_some(sid.clone())
                        })
                        .collect();
                    self.remove_retained_zone_object(q.id);
                    out.push(ZoneOutbound::ToMany {
                        session_ids,
                        packets: vec![ServerPacket::ObjectRemove { object_id: q.id }],
                    });
                }
                return false;
            }
            if !q.spawned {
                let packet = ServerPacket::ObjectSpell {
                    info: ObjectSpellInfo {
                        object_id: q.id,
                        location: q.position.clone(),
                        spell: if q.variant == 0 {
                            Spell::MapQuake1
                        } else {
                            Spell::MapQuake2
                        },
                        direction: MirDirection::Up,
                        param: false,
                    },
                };
                self.object_grid.insert(q.id, &q.position);
                self.objects.insert(
                    q.id,
                    ZoneObject {
                        object_id: q.id,
                        position: q.position.clone(),
                        packet,
                        health: None,
                        mana: None,
                        expires_at_ms: None,
                        buffs: BTreeMap::new(),
                    },
                );
                out.extend(self.diff_all_zone_object_visibility());
                q.spawned = true;
            }
            if now >= q.next_tick {
                q.next_tick = now.saturating_add(500);
                let victims: Vec<_> = self
                    .players
                    .values()
                    .filter(|p| !p.dead && p.position == q.position)
                    .map(|p| (p.session_id.clone(), p.object_id))
                    .collect();
                for (session, target) in victims {
                    out.extend(self.resolve_native_player_environment_struck(
                        PendingNativePlayerHit {
                            ready_at_ms: now,
                            attacker_object_id: q.id,
                            attacker_ai: 98,
                            target_session_id: session,
                            target_object_id: target,
                            damage: q.value,
                            magic: true,
                        },
                        now,
                    ));
                }
                // MapQuake's Caster is null. SpellObject.ProcessSpell does not
                // call IsAttackTarget: wild monsters are hit as well as pets.
                // Resolve the current occupant's exact life on each field tick.
                let native_victims: Vec<_> = self
                    .native_monsters
                    .iter()
                    .filter(|(_, m)| !m.dead && m.hp > 0 && m.position == q.position)
                    .filter_map(|(&id, _)| self.native_entity_monster_ref(id))
                    .collect();
                for target in native_victims {
                    let (_, hit) =
                        self.resolve_native_monster_environment_struck(&target, q.value, true, now);
                    out.extend(hit);
                }
            }
            true
        });
        // New spawns may have initialized an empty world container; retain any
        // events produced through their lifecycle hooks before publishing state.
        if let Some(mut newly) = self.hell_world.take() {
            state.stage_updates.append(&mut newly.stage_updates);
            state
                .stage_update_lives
                .append(&mut newly.stage_update_lives);
            state.summons.append(&mut newly.summons);
            state.quakes.append(&mut newly.quakes);
        }
        self.hell_world = Some(state);
        out
    }
}
