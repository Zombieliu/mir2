//! Crystal TreeQueen142: map-owned roots survive their caster's death/removal.
use super::entity_combat::{EntityDefence, EntityTargetPurpose, ZoneCombatEntityRef};
use super::*;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct TreeQueenState {
    pub rng: u64,
    pub roots_at: u64,
    pub ground_at: u64,
    pub not_near: bool,
    pub target: Option<(SessionId, u32)>,
    pub hits: Vec<TreeQueenHit>,
    #[serde(default)]
    pub entity_hits: Vec<TreeQueenEntityHit>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TreeQueenHit {
    pub due: u64,
    pub target: (SessionId, u32),
    pub damage: i32,
    pub push: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TreeQueenEntityHit {
    source: ZoneCombatEntityRef,
    target: ZoneCombatEntityRef,
    due: u64,
    damage: i32,
    push: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct TreeQueenWorld {
    pub roots: Vec<TreeQueenRoot>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TreeQueenRoot {
    pub id: u32,
    pub source: u32,
    #[serde(default)]
    pub source_ref: Option<ZoneCombatEntityRef>,
    pub removed: bool,
    pub kind: u8,
    pub location: Point,
    pub center: Point,
    pub show: bool,
    pub value: i32,
    pub start: u64,
    pub expires: u64,
    pub next: u64,
    pub speed: u64,
    pub spawned: bool,
}
pub(in crate::runtime::zone) fn initialize_tree_queen(m: &mut ZoneNativeMonster, now: u64) {
    if m.ai == 142 {
        m.special_ai
            .get_or_insert_with(Default::default)
            .tree_queen
            .get_or_insert_with(|| TreeQueenState {
                roots_at: now + 5000,
                ground_at: now + 15000,
                not_near: true,
                ..Default::default()
            });
        m.direction = MirDirection::Up;
    }
}
impl ZoneRuntime {
    pub(super) fn tree_queen_root_poison_origin_alive(
        &self,
        id: u32,
        start: u64,
        expires: u64,
        now: u64,
    ) -> bool {
        // `removed` means Caster lost its Node, not removal of this spell node.
        now <= expires
            && self.tree_queen_world.as_ref().is_some_and(|w| {
                w.roots
                    .iter()
                    .any(|r| r.id == id && r.start == start && r.expires == expires && r.spawned)
            })
    }
    pub(super) fn clear_tree_queen_targets(&mut self, id: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.tree_queen.as_mut()) {
                if s.target.as_ref().is_some_and(|(_, t)| *t == id) {
                    s.target = None;
                }
                s.hits.retain(|h| h.target.1 != id);
                s.entity_hits.retain(|h| h.target.object_id() != id);
            }
        }
    }
    pub(super) fn invalidate_tree_queen_source(&mut self, id: u32) {
        if let Some(w) = self.tree_queen_world.as_mut() {
            for r in &mut w.roots {
                if r.source == id {
                    r.removed = true;
                }
            }
        }
    }
    fn tree_roll(&mut self, id: u32, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let s = self
            .native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .tree_queen
            .as_mut()
            .unwrap();
        if s.rng == 0 {
            s.rng = (u64::from(id) << 32) ^ 142;
        }
        s.rng = s.rng.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = s.rng;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        (x ^ (x >> 31)) % n
    }
    fn tree_power(&mut self, id: u32, magic: bool, nested: bool) -> i32 {
        let Some(t) = crystal_monster_by_name(&self.native_monsters[&id].name) else {
            return 0;
        };
        let monster = &self.native_monsters[&id];
        let (a, b) = if magic {
            (
                t.min_mc.saturating_add(zone_native_monster_buff_stat_total(
                    monster,
                    CRYSTAL_STAT_MIN_MC,
                )),
                t.max_mc.saturating_add(zone_native_monster_buff_stat_total(
                    monster,
                    CRYSTAL_STAT_MAX_MC,
                )),
            )
        } else {
            (
                t.min_dc.saturating_add(zone_native_monster_buff_stat_total(
                    monster,
                    CRYSTAL_STAT_MIN_DC,
                )),
                t.max_dc.saturating_add(zone_native_monster_buff_stat_total(
                    monster,
                    CRYSTAL_STAT_MAX_DC,
                )),
            )
        };
        let a = i32::from(a);
        let b = i32::from(b).max(a);
        let value = a + self.tree_roll(id, (b - a + if nested { 0 } else { 1 }) as u64) as i32;
        if nested {
            self.tree_roll(id, value.max(0) as u64) as i32
        } else {
            value
        }
    }
    fn tree_root(
        &mut self,
        id: u32,
        kind: u8,
        location: Point,
        center: Point,
        show: bool,
        value: i32,
        start: u64,
        expires: u64,
        speed: u64,
    ) {
        if location.x < 0 || location.y < 0 || self.collision.is_blocked(&location) {
            return;
        }
        let source_ref = self.native_entity_monster_ref(id);
        let oid = self.unique_object_id(0);
        self.tree_queen_world
            .get_or_insert_with(Default::default)
            .roots
            .push(TreeQueenRoot {
                id: oid,
                source: id,
                source_ref,
                removed: false,
                kind,
                location,
                center,
                show,
                value,
                start,
                expires,
                next: 0,
                speed,
                spawned: false,
            });
    }
    fn tree_emit(&self, id: u32, packets: Vec<ServerPacket>) -> Vec<ZoneOutbound> {
        let Some(m) = self.native_monsters.get(&id) else {
            return Vec::new();
        };
        vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &m.position),
            packets,
        }]
    }
    fn tree_spawn_roots(
        &mut self,
        id: u32,
        mass: bool,
        ground: bool,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let positions: Vec<_> = self
            .players
            .values()
            .map(|p| (p.object_id, p.position.clone()))
            .collect();
        if positions.is_empty() {
            return Vec::new();
        }
        let origin = self.native_monsters[&id].position.clone();
        let mut out = Vec::new();
        if !mass && !ground {
            let count = 1 + self.tree_roll(id, 4);
            let distance = 5 + self.tree_roll(id, 10) as i32;
            for (_, p) in positions {
                for _ in 0..count {
                    let mut loc = Point {
                        x: p.x + self.tree_roll(id, (distance * 2 + 1) as u64) as i32 - distance,
                        y: p.y + self.tree_roll(id, (distance * 2 + 1) as u64) as i32 - distance,
                    };
                    let hit = self.tree_roll(id, 3) == 0;
                    if hit {
                        loc = p.clone()
                    }
                    if loc.x < 0 || loc.y < 0 || self.collision.is_blocked(&loc) {
                        continue;
                    }
                    let delay = self.tree_roll(id, 2000);
                    let value = self.tree_power(id, true, true);
                    self.tree_root(
                        id,
                        0,
                        loc.clone(),
                        loc,
                        true,
                        value,
                        now + delay,
                        now + 1500 + delay,
                        2000,
                    );
                    if hit {
                        break;
                    }
                }
            }
            return out;
        }
        let centers: Vec<_> = if ground {
            positions
                .iter()
                .map(|_| {
                    (
                        0,
                        Point {
                            x: origin.x + self.tree_roll(id, 11) as i32 - 5,
                            y: origin.y + self.tree_roll(id, 11) as i32 - 5,
                        },
                    )
                })
                .collect()
        } else {
            let n = self.tree_roll(id, positions.len() as u64) as usize;
            vec![positions[n].clone()]
        };
        for (tid, center) in centers {
            if !ground {
                out.extend(self.tree_emit(
                    id,
                    vec![ServerPacket::ObjectRangeAttack {
                        info: ObjectRangeAttackInfo {
                            object_id: id,
                            location: origin.clone(),
                            direction: MirDirection::Up,
                            target_id: tid,
                            target: center.clone(),
                            attack_type: 1,
                            spell: 0,
                            level: 0,
                        },
                    }],
                ));
            }
            let r = if ground { 2 } else { 3 };
            for y in center.y - r..=center.y + r {
                for x in center.x - r..=center.x + r {
                    let loc = Point { x, y };
                    if loc == origin || x < 0 || y < 0 || self.collision.is_blocked(&loc) {
                        continue;
                    }
                    let value = self.tree_power(id, !ground, false);
                    let delay = if ground {
                        self.tree_roll(id, 4000)
                    } else {
                        500
                    };
                    self.tree_root(
                        id,
                        if ground { 2 } else { 1 },
                        loc.clone(),
                        center.clone(),
                        loc == center,
                        value,
                        now + delay,
                        now + delay + if ground { 900 } else { 1500 },
                        1000,
                    );
                }
            }
        }
        out
    }
    pub(super) fn tick_tree_queen_ai(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let mut out = self.tick_tree_roots(now);
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| (m.ai == 142).then_some(id))
            .collect();
        for id in ids {
            initialize_tree_queen(self.native_monsters.get_mut(&id).unwrap(), now);
            let hits = std::mem::take(
                &mut self
                    .native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .tree_queen
                    .as_mut()
                    .unwrap()
                    .hits,
            );
            // Old signed checkpoints retain their old player-only queue. Bind only
            // currently matching identities; new attacks always capture both lives.
            let mut pending = std::mem::take(
                &mut self
                    .native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .tree_queen
                    .as_mut()
                    .unwrap()
                    .entity_hits,
            );
            for h in hits {
                if let (Some(source), Some(target)) = (
                    self.native_entity_monster_ref(id),
                    self.native_entity_player_ref(&h.target.0),
                ) {
                    if target.object_id() == h.target.1 {
                        pending.push(TreeQueenEntityHit {
                            source,
                            target,
                            due: h.due,
                            damage: h.damage,
                            push: h.push,
                        });
                    }
                }
            }
            for h in pending {
                if now < h.due {
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .special_ai
                        .as_mut()
                        .unwrap()
                        .tree_queen
                        .as_mut()
                        .unwrap()
                        .entity_hits
                        .push(h);
                    continue;
                }
                if !self.native_entity_can_attack(
                    &h.source,
                    &h.target,
                    EntityTargetPurpose::Impact,
                    now,
                ) {
                    continue;
                }
                let origin = self.native_monsters[&id].position.clone();
                for t in self.native_entity_targets(
                    id,
                    &origin,
                    if h.push { 1 } else { 3 },
                    EntityTargetPurpose::VisibleImpact,
                    now,
                ) {
                    if h.push {
                        if let Some(dir) = zone_direction_toward(&origin, &t.position) {
                            out.extend(match &t.reference {
                                ZoneCombatEntityRef::Player { session_id, .. } => {
                                    self.great_fox_pull_player(session_id, dir, 5, now)
                                }
                                ZoneCombatEntityRef::Monster { object_id, .. } => {
                                    self.great_fox_pull_monster(*object_id, dir, 5, now)
                                }
                            });
                        }
                    } else {
                        out.extend(
                            self.resolve_native_entity_hit(
                                &h.source,
                                &t.reference,
                                h.damage,
                                EntityDefence::MACAgility,
                                false,
                                now,
                            )
                            .1,
                        );
                    }
                }
            }
            let m = self.native_monsters[&id].clone();
            if m.dead
                || !m.hostile_to_player
                || native_monster_control_active(&m, now)
                || !monster_visibility_can_act(&m, now)
                || self.players.is_empty()
            {
                continue;
            }
            let s = m.special_ai.as_ref().unwrap().tree_queen.as_ref().unwrap();
            if now > s.roots_at {
                let mass = self.tree_roll(id, 4) == 0;
                out.extend(self.tree_spawn_roots(id, mass, false, now));
                let next = 1 + self.tree_roll(id, 3);
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .tree_queen
                    .as_mut()
                    .unwrap()
                    .roots_at = now + next * 1000 * if s.not_near { 1 } else { 4 };
            }
            if now > s.ground_at {
                out.extend(self.tree_spawn_roots(id, false, true, now));
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .tree_queen
                    .as_mut()
                    .unwrap()
                    .ground_at = now + 2000 * if s.not_near { 1 } else { 4 };
            }
            let view = crystal_monster_by_name(&m.name).map_or(8, |t| i32::from(t.view_range));
            let targets =
                self.native_entity_targets(id, &m.position, view, EntityTargetPurpose::Search, now);
            let t = self
                .selected_native_entity_target(id, now)
                .or_else(|| targets.first().cloned());
            let Some(t) = t else { continue };
            self.set_native_entity_target(id, &t.reference, now);
            if now <= m.next_attack_ready_at_ms || now < m.next_ai_ready_at_ms {
                continue;
            }
            let ranged =
                m.position == t.position || zone_tile_distance(&m.position, &t.position) > 2;
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.next_ai_ready_at_ms = now + 500;
            live.next_attack_ready_at_ms = now + live.attack_speed_ms;
            live.special_ai
                .as_mut()
                .unwrap()
                .tree_queen
                .as_mut()
                .unwrap()
                .not_near = ranged;
            if ranged {
                continue;
            }
            let push = self.tree_roll(id, 2) == 0;
            out.extend(self.tree_emit(
                id,
                vec![ServerPacket::ObjectAttack {
                    info: ObjectAttackInfo {
                        object_id: id,
                        location: m.position,
                        direction: MirDirection::Up,
                        attack_type: if push { 1 } else { 0 },
                        spell: 0,
                        level: 0,
                    },
                }],
            ));
            let damage = self.tree_power(id, false, false);
            if damage > 0 {
                let Some(source) = self.native_entity_monster_ref(id) else {
                    continue;
                };
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .tree_queen
                    .as_mut()
                    .unwrap()
                    .entity_hits
                    .push(TreeQueenEntityHit {
                        source,
                        due: now + 500,
                        target: t.reference,
                        damage,
                        push,
                    });
            }
        }
        out
    }
    fn tick_tree_roots(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let Some(mut world) = self.tree_queen_world.take() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        world.roots.retain_mut(|r| {
            if now < r.start {
                return true;
            }
            if now > r.expires {
                if r.spawned && r.show {
                    let sessions = self
                        .players
                        .iter()
                        .filter_map(|(s, p)| {
                            p.visible_object_ids.contains(&r.id).then_some(s.clone())
                        })
                        .collect();
                    self.remove_retained_zone_object(r.id);
                    out.push(ZoneOutbound::ToMany {
                        session_ids: sessions,
                        packets: vec![ServerPacket::ObjectRemove { object_id: r.id }],
                    });
                }
                return false;
            }
            if !r.spawned {
                r.spawned = true;
                if r.show {
                    let spell = match r.kind {
                        0 => Spell::TreeQueenRoot,
                        1 => Spell::TreeQueenMassRoots,
                        _ => Spell::TreeQueenGroundRoots,
                    };
                    let packet = ServerPacket::ObjectSpell {
                        info: ObjectSpellInfo {
                            object_id: r.id,
                            location: r.center.clone(),
                            spell,
                            direction: MirDirection::Up,
                            param: false,
                        },
                    };
                    self.object_grid.insert(r.id, &r.center);
                    self.objects.insert(
                        r.id,
                        ZoneObject {
                            object_id: r.id,
                            position: r.center.clone(),
                            packet,
                            health: None,
                            mana: None,
                            expires_at_ms: None,
                            buffs: BTreeMap::new(),
                        },
                    );
                    out.extend(self.diff_all_zone_object_visibility());
                }
            }
            if now >= r.next {
                r.next = now + r.speed;
                let source = r
                    .source_ref
                    .clone()
                    .or_else(|| self.native_entity_monster_ref(r.source));
                if !r.removed && self.objects.contains_key(&r.source) {
                    if let Some(source) = source {
                        if self.native_entity_monster_ref(r.source).as_ref() == Some(&source) {
                            for t in self.native_entity_targets(
                                r.source,
                                &r.location,
                                0,
                                EntityTargetPurpose::Impact,
                                now,
                            ) {
                                if r.value > 0 {
                                    out.extend(
                                        self.resolve_native_entity_hit(
                                            &source,
                                            &t.reference,
                                            r.value,
                                            EntityDefence::MAC,
                                            true,
                                            now,
                                        )
                                        .1,
                                    );
                                }
                                if r.kind == 2 && self.tree_roll(r.source, 3) > 0 {
                                    out.extend(self.apply_native_tree_root_paralysis(
                                        &source,
                                        &t.reference,
                                        r.id,
                                        r.start,
                                        r.expires,
                                        now,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            true
        });
        self.tree_queen_world = Some(world);
        out
    }
}

#[cfg(test)]
mod power_tests {
    use super::*;
    use mir2_protocol::MirGender;
    fn fixture(id: u32) -> ZoneRuntime {
        let mut z = ZoneRuntime::new(ZoneKey::for_map("tree-buff"));
        z.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new("target"),
            account_id: "test".into(),
            character_index: 1,
            object_id: 101,
            name: "target".into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 50,
            hp: 10000,
            max_hp: 10000,
            mp: 0,
            map_file_name: "tree-buff".into(),
            position: Point { x: 101, y: 100 },
            direction: MirDirection::Left,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        let spawn = ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: id,
            name: "ArcherGuard".into(),
            name_colour_argb: -1,
            image: 132,
            ai: 142,
            disposition: Some(crate::config::WorldEntityDisposition::Hostile),
            level: 37,
            max_hp: 1000,
            hp: 1000,
            experience: 0,
            move_speed_ms: 500,
            attack_speed_ms: 1000,
            friendly_guild: None,
            defense: Default::default(),
            position: Point { x: 100, y: 100 },
            direction: MirDirection::Up,
            respawn: None,
            drops: Vec::new(),
        };
        assert!(z.spawn_authoritative_monster(&spawn, 0).0);
        z
    }
    #[test]
    fn tree_buff_min_max_changes_actual_bombardment_and_expiry_restores_power() {
        for id in 97000..97100 {
            let mut z = fixture(id);
            z.native_monsters.get_mut(&id).unwrap().buffs.insert(
                9,
                super::super::super::types::ZonePlayerBuff {
                    buff: ClientBuff {
                        buff_type: 9,
                        visible: true,
                        object_id: id,
                        expire_time: 1000,
                        infinite: false,
                        paused: false,
                        stats: vec![
                            UserItemStat {
                                stat: CRYSTAL_STAT_MIN_DC,
                                value: 100,
                            },
                            UserItemStat {
                                stat: CRYSTAL_STAT_MAX_DC,
                                value: 100,
                            },
                        ],
                        values: Vec::new(),
                    },
                    expires_at_ms: Some(1000),
                    notify_owner_on_expiry: false,
                },
            );
            z.tick(1);
            let hit = &z.native_monsters[&id]
                .special_ai
                .as_ref()
                .unwrap()
                .tree_queen
                .as_ref()
                .unwrap()
                .entity_hits[0];
            if hit.push {
                continue;
            }
            assert_eq!(hit.damage, 355);
            let out = z.tick(501);
            assert!(
                out.iter()
                    .any(|o| matches!(o, ZoneOutbound::PlayerDamaged { damage: 355, .. })),
                "buff must change real damage, not only packet metadata"
            );
            z.tick(1000);
            assert_eq!(z.tree_power(id, false, false), 255);
            return;
        }
        panic!("bombardment branch not exercised");
    }
    #[test]
    fn hidden_root_poison_survives_queen_retirement_but_not_its_own_node() {
        let mut z = fixture(97999);
        let source = z.native_entity_monster_ref(97999).unwrap();
        let target = z
            .native_entity_player_ref(&SessionId::new("target"))
            .unwrap();
        z.tree_root(
            97999,
            2,
            Point { x: 101, y: 100 },
            Point { x: 102, y: 100 },
            false,
            0,
            1,
            901,
            1000,
        );
        z.tick_tree_roots(1);
        let root = z.tree_queen_world.as_ref().unwrap().roots[0].id;
        assert!(
            !z.objects.contains_key(&root),
            "hidden spell cells have a real node without a client object"
        );
        z.apply_native_tree_root_paralysis(&source, &target, root, 1, 901, 1);
        assert_ne!(z.players[&SessionId::new("target")].poison & 32, 0);
        z.remove_retained_zone_object(97999);
        assert!(z.tree_queen_root_poison_origin_alive(root, 1, 901, 2));
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(z.tick(2), restored.tick(2));
        assert_ne!(
            z.players[&SessionId::new("target")].poison & 32,
            0,
            "caster retirement must not retire the root's poison source"
        );
        assert_eq!(z.tick(902), restored.tick(902));
        assert_eq!(
            z.players[&SessionId::new("target")].poison & 32,
            0,
            "root node expiry clears its admitted paralysis"
        );
    }
}
