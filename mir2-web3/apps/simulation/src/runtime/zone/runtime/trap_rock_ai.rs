//! Crystal TrapRock.cs (AI47). Player-target trap lifecycle, not a damage turret.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TrapRockState {
    visible: bool,
    parent: Option<u32>,
    children: Vec<u32>,
    target: Option<SessionId>,
    target_id: u32,
    target_location: Point,
    check_ms: u64,
    first_attack: bool,
    rng: u64,
}

impl TrapRockState {
    fn roll(&mut self, n: u64) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E3779B97F4A7C15);
        let mut x = self.rng;
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
        (x ^ (x >> 31)) % n
    }
}

pub(in crate::runtime::zone) fn initialize_trap_rock(m: &mut ZoneNativeMonster, id: u32, now: u64) {
    if m.ai != 47 {
        return;
    }
    m.special_ai
        .get_or_insert_with(Default::default)
        .trap_rock
        .get_or_insert(TrapRockState {
            visible: false,
            parent: None,
            children: Vec::new(),
            target: None,
            target_id: 0,
            target_location: m.position.clone(),
            check_ms: now.saturating_add(2_000),
            first_attack: true,
            rng: now ^ u64::from(id),
        });
}
fn rock(m: &ZoneNativeMonster) -> Option<&TrapRockState> {
    m.special_ai.as_ref()?.trap_rock.as_ref()
}
pub(super) fn trap_rock_visible(m: &ZoneNativeMonster) -> bool {
    m.ai != 47 || rock(m).is_some_and(|s| s.visible)
}
pub(super) fn trap_rock_accepts_hp_change(m: &ZoneNativeMonster) -> bool {
    m.ai != 47 || rock(m).is_some_and(|s| s.parent.is_none())
}
pub(super) fn sync_trap_rock_packet(m: &ZoneNativeMonster, p: &mut ServerPacket) {
    if m.ai == 47 {
        if let ServerPacket::ObjectMonster { info } = p {
            info.hidden = !trap_rock_visible(m);
        }
    }
}

impl ZoneRuntime {
    /// Root calls for direct human/monster Attacked, before the HP write.
    /// Child hits disarm the parent's first-hit suicide but cannot change child HP.
    pub(super) fn trap_rock_direct_hit(&mut self, id: u32, now: u64) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?;
        if m.ai != 47 || m.dead {
            return None;
        }
        let s = rock(m)?.clone();
        if let Some(parent) = s.parent {
            if let Some(p) = self
                .native_monsters
                .get_mut(&parent)
                .and_then(|m| m.special_ai.as_mut())
                .and_then(|s| s.trap_rock.as_mut())
            {
                p.first_attack = false;
            }
            return Some(Vec::new());
        }
        s.first_attack.then(|| self.kill_trap_group(id, now))
    }

    pub(super) fn tick_trap_rocks(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| (m.ai == 47 && !m.dead).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            initialize_trap_rock(self.native_monsters.get_mut(&id).unwrap(), id, now);
            let m = self.native_monsters[&id].clone();
            let mut s = rock(&m).unwrap().clone();
            if s.parent
                .is_some_and(|parent| self.native_monsters.get(&parent).is_none_or(|m| m.dead))
            {
                out.extend(self.kill_trap_group(id, now));
                continue;
            }
            if s.target.is_none() && s.parent.is_none() {
                if let Some(t) =
                    self.nearest_native_monster_target(&m.position, m.friendly_guild.as_deref())
                {
                    s.target = Some(t.session_id);
                    s.target_id = t.object_id;
                }
            }
            let target = s
                .target
                .as_ref()
                .and_then(|key| self.players.get(key))
                .filter(|p| p.object_id == s.target_id && !p.dead && !p.chat_profile.in_safe_zone)
                .cloned();
            if s.visible && target.is_none() {
                out.extend(self.kill_trap_group(id, now));
                continue;
            }
            if now > s.check_ms {
                s.check_ms = now.saturating_add(2_000);
                if let Some(t) = target.as_ref() {
                    if s.visible && t.position != s.target_location {
                        out.extend(self.kill_trap_group(id, now));
                        continue;
                    }
                    if !s.visible {
                        let claimed = self.native_monsters.iter().any(|(&other, m)| {
                            other != id
                                && !m.dead
                                && rock(m).is_some_and(|r| {
                                    r.visible && r.parent.is_none() && r.target_id == t.object_id
                                })
                        });
                        if !claimed {
                            let corner = s.roll(4) as i8 * 2;
                            let direction = zone_rotated_direction(MirDirection::Up, corner);
                            let destination = offset_point(&t.position, direction, 1);
                            if self.can_native_monster_occupy(id, &destination) {
                                s.visible = true;
                                s.target_location = t.position.clone();
                                let live = self.native_monsters.get_mut(&id).unwrap();
                                live.position = destination.clone();
                                live.next_ai_ready_at_ms = now.saturating_add(1_000);
                                live.special_ai.as_mut().unwrap().trap_rock = Some(s.clone());
                                if let Some(o) = self.objects.get_mut(&id) {
                                    o.position = destination.clone();
                                    if let ServerPacket::ObjectMonster { info } = &mut o.packet {
                                        info.location = destination.clone();
                                        info.hidden = false;
                                    }
                                }
                                self.object_grid.moved(&id, &destination);
                                out.extend(self.diff_all_zone_object_visibility());
                                out.push(ZoneOutbound::ToMany {
                                    session_ids: self
                                        .native_monster_visible_recipients(id, &destination),
                                    packets: vec![ServerPacket::ObjectShow { object_id: id }],
                                });
                                out.extend(self.trap_paralyze(t, now));
                                for c in (0..8).step_by(2) {
                                    if c == corner {
                                        continue;
                                    }
                                    let position = offset_point(
                                        &t.position,
                                        zone_rotated_direction(MirDirection::Up, c),
                                        1,
                                    );
                                    if !self.can_native_monster_occupy(0, &position) {
                                        continue;
                                    }
                                    let child = self.unique_object_id(0);
                                    let image = match self.objects.get(&id).map(|o| &o.packet) {
                                        Some(ServerPacket::ObjectMonster { info }) => info.image,
                                        _ => 47,
                                    };
                                    let spawn = ZoneMonsterSpawn {
                                        crystal_drop_seed: None,
                                        object_id: child,
                                        name: m.name.clone(),
                                        name_colour_argb: -1,
                                        image,
                                        ai: 47,
                                        disposition: m.disposition,
                                        level: m.level,
                                        max_hp: m.max_hp,
                                        hp: m.max_hp,
                                        experience: m.experience,
                                        move_speed_ms: m.move_speed_ms,
                                        attack_speed_ms: m.attack_speed_ms,
                                        friendly_guild: m.friendly_guild.clone(),
                                        position: position.clone(),
                                        direction: m.direction,
                                        defense: m.defense.clone(),
                                        respawn: None,
                                        drops: Vec::new(),
                                    };
                                    let mut cm = ZoneNativeMonster::from_spawn(&spawn, child);
                                    cm.next_ai_ready_at_ms = now.saturating_add(1_000);
                                    cm.next_attack_ready_at_ms = m.next_attack_ready_at_ms;
                                    let mut cs = s.clone();
                                    cs.parent = Some(id);
                                    cs.children.clear();
                                    cm.special_ai.get_or_insert_with(Default::default).trap_rock =
                                        Some(cs);
                                    let mut packet = native_monster_spawn_packet(&spawn, child);
                                    sync_trap_rock_packet(&cm, &mut packet);
                                    cm.incarnation = self.allocate_monster_incarnation();
                                    self.native_monsters.insert(child, cm);
                                    self.apply_zone_object_packets(&[packet], now);
                                    out.extend(self.diff_all_zone_object_visibility());
                                    out.push(ZoneOutbound::ToMany {
                                        session_ids: self
                                            .native_monster_visible_recipients(child, &position),
                                        packets: vec![ServerPacket::ObjectShow {
                                            object_id: child,
                                        }],
                                    });
                                    s.children.push(child);
                                }
                            }
                        }
                    }
                } else {
                    s.target = None;
                    s.target_id = 0;
                }
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .trap_rock = Some(s.clone());
            let live = &self.native_monsters[&id];
            if !s.visible
                || now < live.next_ai_ready_at_ms
                || now < live.next_attack_ready_at_ms
                || native_monster_control_active(live, now)
            {
                continue;
            }
            let Some(t) = target else {
                continue;
            };
            let packet = if s.parent.is_some() {
                ServerPacket::ObjectAttack {
                    info: ObjectAttackInfo {
                        object_id: id,
                        location: live.position.clone(),
                        direction: live.direction,
                        spell: 0,
                        level: 0,
                        attack_type: 0,
                    },
                }
            } else {
                ServerPacket::ObjectRangeAttack {
                    info: ObjectRangeAttackInfo {
                        object_id: id,
                        location: live.position.clone(),
                        direction: live.direction,
                        target_id: t.object_id,
                        target: t.position.clone(),
                        attack_type: 0,
                        spell: 0,
                        level: 0,
                    },
                }
            };
            out.push(ZoneOutbound::ToMany {
                session_ids: self.native_monster_visible_recipients(id, &live.position),
                packets: vec![packet],
            });
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.next_ai_ready_at_ms = now.saturating_add(300);
            live.next_attack_ready_at_ms = now.saturating_add(live.attack_speed_ms);
            if s.parent.is_none() && s.roll(8) == 0 {
                out.extend(self.trap_paralyze(&t, now));
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .trap_rock = Some(s);
        }
        out
    }

    fn trap_paralyze(&mut self, target: &ZonePlayer, now: u64) -> Vec<ZoneOutbound> {
        let p = self.players.get_mut(&target.session_id).unwrap();
        zone_add_player_status_poison(p, CRYSTAL_POISON_PARALYSIS, 3_000, now);
        let packet = ServerPacket::ObjectPoisoned {
            object_id: p.object_id,
            poison: p.poison,
        };
        vec![ZoneOutbound::ToMany {
            session_ids: self.player_status_recipients(target.object_id, &target.position),
            packets: vec![packet],
        }]
    }
    pub(super) fn kill_trap_group(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let mut ids = vec![id];
        if let Some(s) = self.native_monsters.get(&id).and_then(rock) {
            ids.extend(s.children.iter().copied());
        }
        let mut out = Vec::new();
        for id in ids {
            let Some(m) = self.native_monsters.get_mut(&id) else {
                continue;
            };
            if m.dead {
                continue;
            }
            m.dead = true;
            m.hp = 0;
            let position = m.position.clone();
            if let Some(r) = self.native_monster_respawns.get_mut(&id) {
                r.due_at_ms = r.spawn.respawn.map(|policy| policy.due_at_ms(now));
            }
            let packet = ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: id,
                    location: position.clone(),
                    direction: m.direction,
                    kind: 0,
                },
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            if !self.native_monster_respawns.contains_key(&id) {
                if let Some(object) = self.objects.get_mut(&id) {
                    object.expires_at_ms = Some(now.saturating_add(180_000));
                }
            }
            out.push(ZoneOutbound::ToMany {
                session_ids: self.native_monster_visible_recipients(id, &position),
                packets: vec![packet],
            });
        }
        out
    }
}
