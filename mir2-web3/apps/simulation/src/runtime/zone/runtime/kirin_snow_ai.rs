//! Crystal Kirin186 and SnowWolfKing180; shared, checkpointed actions.
use super::entity_combat::{
    EntityDefence, EntityTargetPurpose, NativeEntityTarget, ZoneCombatEntityRef,
};
use super::stage_summons::StageSummonState;
use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct KirinSnowState {
    sequence: u64,
    action_ms: u64,
    hits: Vec<KirinSnowHit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    entity_hits: Vec<KirinSnowEntityHit>,
    #[serde(default)]
    entity_target: Option<ZoneCombatEntityRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    death_lives: Vec<ZoneCombatEntityRef>,
    death_ms: Option<u64>,
    death_finished: bool,
    excluded_lives: BTreeSet<u32>,
    spawned: bool,
    slaves: Vec<u32>,
    reward_owner: Option<SessionId>,
    reward_until_ms: u64,
    target: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KirinSnowEntityHit {
    source: ZoneCombatEntityRef,
    target: ZoneCombatEntityRef,
    due_ms: u64,
    damage: i32,
    defence: EntityDefence,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KirinSnowHit {
    hit: PendingNativePlayerHit,
    accuracy: Option<i32>,
}
pub(in crate::runtime::zone) fn initialize_kirin_snow(m: &mut ZoneNativeMonster) {
    if matches!(m.ai, 180 | 186) {
        m.special_ai
            .get_or_insert_with(Default::default)
            .kirin_snow
            .get_or_insert_with(Default::default);
    }
}
pub(super) fn kirin_snow_tag(m: &mut ZoneNativeMonster, owner: Option<&SessionId>, now: u64) {
    if m.ai != 180 {
        return;
    }
    initialize_kirin_snow(m);
    let Some(owner) = owner else { return };
    let s = m.special_ai.as_mut().unwrap().kirin_snow.as_mut().unwrap();
    if s.reward_owner.is_none() || now > s.reward_until_ms {
        s.reward_owner = Some(owner.clone());
    }
    if s.reward_owner.as_ref() == Some(owner) {
        s.reward_until_ms = now.saturating_add(5000);
    }
}
fn roll(s: &mut KirinSnowState, id: u32, now: u64, n: u64) -> u64 {
    s.sequence = s.sequence.wrapping_add(1);
    let mut x = now ^ u64::from(id) ^ s.sequence.wrapping_mul(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    (x ^ (x >> 31)) % n.max(1)
}
impl ZoneRuntime {
    pub(super) fn clear_kirin_snow_target_life(&mut self, target: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.kirin_snow.as_mut()) {
                s.hits.retain(|h| h.hit.target_object_id != target);
                s.entity_hits.retain(|h| h.target.object_id() != target);
                if s.entity_target
                    .as_ref()
                    .is_some_and(|t| t.object_id() == target)
                {
                    s.entity_target = None;
                }
                if s.target == Some(target) {
                    s.target = None;
                }
                if s.death_ms.is_some() && !s.death_finished {
                    s.excluded_lives.insert(target);
                }
                if s.reward_owner.as_ref().is_some_and(|owner| {
                    self.players
                        .get(owner)
                        .is_none_or(|p| p.object_id == target)
                }) {
                    s.reward_owner = None;
                }
            }
        }
    }
    pub(super) fn kirin_snow_on_death(&mut self, id: u32, now: u64) {
        let lives = self
            .players
            .keys()
            .filter_map(|session| self.native_entity_player_ref(session))
            .chain(
                self.native_monsters
                    .keys()
                    .filter_map(|id| self.native_entity_monster_ref(*id)),
            )
            .collect();
        let Some(m) = self.native_monsters.get_mut(&id) else {
            return;
        };
        if m.ai != 180 {
            return;
        }
        initialize_kirin_snow(m);
        let s = m.special_ai.as_mut().unwrap().kirin_snow.as_mut().unwrap();
        if now > s.reward_until_ms {
            s.reward_owner = None;
        }
        if s.death_ms.is_none() {
            s.death_lives = lives;
        }
        s.death_ms.get_or_insert(now.saturating_add(500));
    }
    pub(super) fn native_monster_has_pending_kirin_snow_death(&self, id: u32) -> bool {
        self.native_monsters
            .get(&id)
            .and_then(|m| m.special_ai.as_ref())
            .and_then(|s| s.kirin_snow.as_ref())
            .is_some_and(|s| s.death_ms.is_some() && !s.death_finished)
    }
    /// None permits ordinary pursuit only. Root must exclude these AIs from generic attacks.
    pub(super) fn try_tick_kirin_snow(&mut self, id: u32, now: u64) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?.clone();
        if !matches!(m.ai, 180 | 186) {
            return None;
        }
        if m.dead || !m.hostile_to_player {
            return Some(Vec::new());
        }
        initialize_kirin_snow(self.native_monsters.get_mut(&id)?);
        let mut s = self.native_monsters[&id]
            .special_ai
            .as_ref()?
            .kirin_snow
            .clone()?;
        if now <= s.action_ms || now <= m.next_attack_ready_at_ms {
            return Some(Vec::new());
        }
        let range = crystal_monster_by_name(&m.name).map_or(0, |t| i32::from(t.view_range));
        let target = self.selected_native_entity_target(id, now).or_else(|| {
            self.native_entity_targets(id, &m.position, range, EntityTargetPurpose::Search, now)
                .into_iter()
                .next()
        })?;
        self.set_native_entity_target(id, &target.reference, now);
        let source = self.native_entity_monster_ref(id)?;
        s.target = Some(target.object_id);
        s.entity_target = Some(target.reference.clone());
        let dx = (target.position.x - m.position.x).abs();
        let dy = (target.position.y - m.position.y).abs();
        let melee = if m.ai == 180 {
            dx.max(dy) <= 1
        } else {
            dx.max(dy) <= 2 && dx.max(dy) > 0 && (dx.max(dy) <= 1 || dx == dy || dx % 2 == dy % 2)
        };
        let ranged = m.ai == 186 && roll(&mut s, id, now, 5) == 0;
        if !melee && !ranged {
            self.native_monsters
                .get_mut(&id)?
                .special_ai
                .as_mut()?
                .kirin_snow = Some(s);
            return Some(self.kirin_snow_pursue(id, &m, &target.position, now));
        }
        let t = crystal_monster_by_name(&m.name)?;
        let direction = zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
        let attack_type = if ranged {
            2
        } else if m.ai == 180 {
            if roll(&mut s, id, now, 3) > 0 {
                0
            } else if m.hp.saturating_mul(100) / m.max_hp.max(1) >= 60 {
                1
            } else if m.hp.saturating_mul(100) / m.max_hp.max(1) >= 30 {
                2
            } else {
                3
            }
        } else if roll(&mut s, id, now, 5) == 0 {
            1
        } else {
            0
        };
        let mut damage = zone_roll_stat_range(
            if ranged { t.min_mc } else { t.min_dc },
            if ranged {
                t.max_mc
                    .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_MC))
            } else {
                t.max_dc
                    .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_DC))
            },
            now,
            id,
            roll(&mut s, id, now, u64::MAX),
        );
        if m.ai == 186 && !ranged && attack_type == 0 && damage > 0 {
            damage = zone_roll_stat_range(
                t.min_dc,
                t.max_dc
                    .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_DC)),
                now,
                id,
                roll(&mut s, id, now, u64::MAX),
            );
        }
        s.action_ms = now.saturating_add(if m.ai == 186 && attack_type > 0 {
            500
        } else {
            300
        });
        let live = self.native_monsters.get_mut(&id)?;
        live.direction = direction;
        live.next_attack_ready_at_ms = now.saturating_add(m.attack_speed_ms);
        live.next_ai_ready_at_ms = live.next_ai_ready_at_ms.max(s.action_ms);
        let mut out = Vec::new();
        if damage > 0 || m.ai == 180 {
            let packet = ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id: id,
                    location: m.position.clone(),
                    direction,
                    spell: 0,
                    level: 0,
                    attack_type,
                },
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            out.push(ZoneOutbound::ToMany {
                session_ids: self.native_monster_visible_recipients(id, &m.position),
                packets: vec![packet],
            });
        }
        if ranged && damage > 0 {
            // Crystal rolls MC again in IceThrust and hits these nine cells immediately.
            let damage = zone_roll_stat_range(
                t.min_mc,
                t.max_mc
                    .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_MC)),
                now,
                id,
                roll(&mut s, id, now, u64::MAX),
            );
            let mut cells = BTreeSet::new();
            for shift in [-1, 0, 1] {
                let start = offset_point(&m.position, zone_rotated_direction(direction, shift), 1);
                for row in 0..3 {
                    let p = offset_point(&start, direction, row);
                    if !self.collision.is_blocked(&p) {
                        cells.insert((p.x, p.y));
                    }
                }
            }
            let targets = self
                .native_entity_targets(id, &m.position, 4, EntityTargetPurpose::Impact, now)
                .into_iter()
                .filter(|t| cells.contains(&(t.position.x, t.position.y)))
                .collect::<Vec<_>>();
            for target in targets {
                let resist = self.native_entity_poison_resist(&target.reference).max(0) as u64;
                let (dealt, events) = self.resolve_native_entity_hit(
                    &source,
                    &target.reference,
                    damage,
                    EntityDefence::MAC,
                    false,
                    now,
                );
                out.extend(events);
                if dealt > 0 && roll(&mut s, id, now, 10) >= resist && roll(&mut s, id, now, 5) == 0
                {
                    let duration = match &target.reference {
                        ZoneCombatEntityRef::Player { .. } => 4,
                        ZoneCombatEntityRef::Monster { .. } => 5 + roll(&mut s, id, now, 5),
                    };
                    // Human.ApplyPoison performs its second resistance test in
                    // the shared API; Monster.ApplyPoison has no second roll.
                    out.extend(self.apply_native_entity_slow(
                        &source,
                        &target.reference,
                        duration,
                        1000,
                        now,
                    ));
                }
            }
        } else if damage > 0 {
            s.entity_hits.push(KirinSnowEntityHit {
                source,
                target: target.reference.clone(),
                due_ms: now.saturating_add(500),
                damage,
                defence: if m.ai == 186 && attack_type == 1 {
                    EntityDefence::AC
                } else {
                    EntityDefence::ACAgility
                },
            });
        }
        if m.ai == 180
            && damage > 0
            && m.hp.saturating_mul(100) / m.max_hp.max(1) < 70
            && !s.spawned
        {
            s.spawned = true;
            out.extend(self.spawn_snow_wolves(id, &m, &target, &mut s, now));
        }
        s.entity_hits.retain(|h| {
            self.native_entity_can_attack(&h.source, &h.target, EntityTargetPurpose::Impact, now)
        });
        s.hits.retain(|h| {
            self.players
                .get(&h.hit.target_session_id)
                .is_some_and(|p| p.object_id == h.hit.target_object_id && !p.dead)
        });
        if s.entity_target.as_ref().is_some_and(|target| {
            self.native_entity_monster_ref(id).is_none_or(|source| {
                !self.native_entity_can_attack(&source, target, EntityTargetPurpose::Impact, now)
            })
        }) {
            s.entity_target = None;
            s.target = None;
        }
        self.native_monsters
            .get_mut(&id)?
            .special_ai
            .as_mut()?
            .kirin_snow = Some(s);
        Some(out)
    }
    fn kirin_snow_pursue(
        &mut self,
        id: u32,
        m: &ZoneNativeMonster,
        target: &Point,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(direction) = zone_direction_toward(&m.position, target) else {
            return Vec::new();
        };
        let point = offset_point(&m.position, direction, 1);
        if !self.can_native_monster_occupy(id, &point) {
            return Vec::new();
        }
        let old = self.native_monster_visible_recipients(id, &m.position);
        let live = self.native_monsters.get_mut(&id).unwrap();
        live.position = point.clone();
        live.direction = direction;
        live.next_ai_ready_at_ms = now.saturating_add(m.move_speed_ms.max(300));
        let packet = ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id: id,
                position: point.clone(),
                direction,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        let mut out = self.diff_all_zone_object_visibility();
        let recipients = self
            .native_monster_visible_recipients(id, &point)
            .into_iter()
            .filter(|s| old.contains(s))
            .collect::<Vec<_>>();
        if !recipients.is_empty() {
            out.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![packet],
            });
        }
        out
    }
    fn spawn_snow_wolves(
        &mut self,
        id: u32,
        m: &ZoneNativeMonster,
        target: &NativeEntityTarget,
        s: &mut KirinSnowState,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(t) = crystal_monster_by_name("SnowWolf") else {
            return Vec::new();
        };
        let target_direction = match &target.reference {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                self.players.get(session_id).map(|p| p.direction)
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                self.native_monsters.get(object_id).map(|m| m.direction)
            }
        }
        .unwrap_or(m.direction);
        let back = offset_point(
            &target.position,
            zone_rotated_direction(target_direction, 4),
            1,
        );
        let mut out = Vec::new();
        for _ in 0..3 {
            // Shared occupancy safety replaces Crystal's stacked fallback spawn.
            let position = (0..=3)
                .flat_map(|r| (-r..=r).flat_map(move |x| (-r..=r).map(move |y| (x, y))))
                .map(|(x, y)| Point {
                    x: back.x + x,
                    y: back.y + y,
                })
                .find(|p| self.can_native_monster_occupy(0, p));
            let Some(position) = position else { break };
            let child = self.unique_object_id(0);
            let spawn = ZoneMonsterSpawn {
                crystal_drop_seed: Some(now / 300),
                object_id: child,
                name: t.name.clone(),
                name_colour_argb: -1,
                image: t.image,
                ai: t.ai,
                disposition: m.disposition,
                level: t.level,
                max_hp: t.hp.max(1),
                hp: t.hp.max(1),
                experience: t.experience,
                move_speed_ms: u64::from(t.move_speed),
                attack_speed_ms: u64::from(t.attack_speed),
                friendly_guild: m.friendly_guild.clone(),
                defense: super::super::types::ZoneMonsterDefense::from_crystal_template(&t),
                position,
                direction: m.direction,
                respawn: None,
                drops: if mir2_game_data::crystal_map_respawns_ref(&self.key.map_file_name)
                    .is_some_and(|map| map.no_drop_monster)
                {
                    Vec::new()
                } else {
                    crate::runtime::drops::zone_ground_drop_snapshots_for_monster_at_tick(
                        child,
                        &t.name,
                        now / 300,
                    )
                },
            };
            let (ok, events) = self.spawn_authoritative_monster(&spawn, now);
            out.extend(events);
            if ok {
                s.slaves.push(child);
                let c = self.native_monsters.get_mut(&child).unwrap();
                c.next_ai_ready_at_ms = now.saturating_add(2000);
                c.next_attack_ready_at_ms = now.saturating_add(2000);
                c.special_ai
                    .get_or_insert_with(Default::default)
                    .stage_summons = Some(StageSummonState {
                    stage: 0,
                    slave_object_ids: Vec::new(),
                    parent_object_id: Some(id),
                    inherited_target_object_id: Some(target.object_id),
                    target_bound: true,
                    parent_incarnation: Some(m.incarnation),
                    slave_lives: BTreeMap::new(),
                });
                self.set_native_entity_target(child, &target.reference, now);
            }
        }
        out
    }
    pub(super) fn tick_kirin_snow_delays(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self.native_monsters.keys().copied().collect();
        let mut out = Vec::new();
        for id in ids {
            let m = self.native_monsters[&id].clone();
            let Some(mut s) = m.special_ai.as_ref().and_then(|s| s.kirin_snow.clone()) else {
                continue;
            };
            let mut hits = Vec::new();
            let mut entity_hits = Vec::new();
            for h in std::mem::take(&mut s.entity_hits) {
                if now >= h.due_ms {
                    entity_hits.push(h);
                } else {
                    s.entity_hits.push(h);
                }
            }
            for h in std::mem::take(&mut s.hits) {
                if now >= h.hit.ready_at_ms {
                    hits.push(h)
                } else {
                    s.hits.push(h)
                }
            }
            if s.death_ms.is_some_and(|due| now >= due) && !s.death_finished {
                s.death_finished = true;
                let damage = crystal_monster_by_name(&m.name).map_or(0, |t| {
                    zone_roll_stat_range(
                        t.min_dc,
                        t.max_dc.saturating_add(zone_native_monster_buff_stat_total(
                            &m,
                            CRYSTAL_STAT_MAX_DC,
                        )),
                        now,
                        id,
                        roll(&mut s, id, now, u64::MAX),
                    )
                });
                if let Some(source) = self.native_entity_monster_ref(id) {
                    for target in self.native_entity_targets(
                        id,
                        &m.position,
                        1,
                        EntityTargetPurpose::Impact,
                        now,
                    ) {
                        if s.excluded_lives.contains(&target.object_id)
                            || s.death_lives.iter().any(|old| {
                                old.object_id() == target.object_id && old != &target.reference
                            })
                        {
                            continue;
                        }
                        entity_hits.push(KirinSnowEntityHit {
                            source: source.clone(),
                            target: target.reference,
                            due_ms: now,
                            damage,
                            defence: EntityDefence::MAC,
                        });
                    }
                }
                // Adoption is performed after the death blast below.
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .kirin_snow = Some(s.clone());
            for h in entity_hits {
                let (_, events) = self.resolve_native_entity_hit(
                    &h.source, &h.target, h.damage, h.defence, false, now,
                );
                out.extend(events);
            }
            for h in hits {
                if self.players.get(&h.hit.target_session_id).is_none_or(|p| {
                    p.object_id != h.hit.target_object_id
                        || m.friendly_guild
                            .as_deref()
                            .zip(p.chat_profile.guild_name.as_deref())
                            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
                }) {
                    continue;
                }
                if h.accuracy.is_some_and(|a| {
                    self.players.get(&h.hit.target_session_id).is_none_or(|p| {
                        crate::runtime::combat::crystal_accuracy_roll(
                            now,
                            id,
                            p.object_id,
                            p.combat_stats.agility.max(0) as u64 + 1,
                        ) > a.max(0) as u64
                    })
                }) {
                    continue;
                }
                out.extend(self.resolve_pending_native_player_hit(h.hit, now));
            }
            // Damage can clear every pending action/target/reward for the old
            // player life. Reload that authoritative state before adoption;
            // writing the pre-damage clone would undo those lifecycle clears.
            let Some(mut s) = self
                .native_monsters
                .get(&id)
                .and_then(|m| m.special_ai.as_ref())
                .and_then(|s| s.kirin_snow.clone())
            else {
                continue;
            };
            if s.death_finished && !s.slaves.is_empty() {
                out.extend(self.adopt_snow_wolves(id, &mut s, now));
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .kirin_snow = Some(s);
            }
        }
        out
    }
    fn adopt_snow_wolves(
        &mut self,
        id: u32,
        s: &mut KirinSnowState,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let owner = s
            .reward_owner
            .as_ref()
            .and_then(|o| self.players.get(o))
            .filter(|p| !p.dead)
            .map(|p| (p.session_id.clone(), p.object_id));
        let children = std::mem::take(&mut s.slaves);
        let mut out = Vec::new();
        let Some((session, owner)) = owner else {
            return out;
        };
        let mut count = self
            .native_monsters
            .values()
            .filter(|m| !m.dead && zone_native_summon_owner_player_object_id(m) == owner)
            .count();
        for child in children {
            if count >= 6 {
                break;
            }
            let Some(m) = self.native_monsters.get_mut(&child) else {
                continue;
            };
            if m.dead {
                continue;
            }
            m.owner_session_id = Some(session.clone());
            m.owner_player_object_id = owner;
            m.master_object_id = owner;
            m.hostile_to_player = false;
            m.disposition = Some(crate::config::WorldEntityDisposition::Friendly);
            m.visible_extra = true;

            if let Some(stage) = m.special_ai.as_mut().and_then(|s| s.stage_summons.as_mut()) {
                stage.inherited_target_object_id = None;
                stage.parent_object_id = None;
            }
            let _ = id;
            if let Some(object) = self.objects.get_mut(&child) {
                object.expires_at_ms = Some(now.saturating_add(3600000));
                if let ServerPacket::ObjectMonster { info } = &mut object.packet {
                    info.extra = true;
                    info.master_object_id = owner;
                }
            }
            if let Some(object) = self.objects.get(&child) {
                out.push(ZoneOutbound::ToMany {
                    session_ids: self.native_monster_visible_recipients(child, &object.position),
                    packets: vec![object.packet.clone()],
                });
            }
            count += 1;
        }
        out.extend(self.diff_all_zone_object_visibility());
        out
    }
    fn kirin_snow_target_min_dc(&self, target: &ZoneCombatEntityRef) -> Option<i32> {
        match target {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                self.players.get(session_id).map(|p| {
                    p.combat_stats
                        .min_dc
                        .saturating_add(zone_player_buff_stat_total(p, CRYSTAL_STAT_MIN_DC))
                })
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let m = self.native_monsters.get(object_id)?;
                Some(
                    crystal_monster_by_name(&m.name)?.min_dc.saturating_add(
                        zone_native_monster_buff_stat_total(m, CRYSTAL_STAT_MIN_DC),
                    ),
                )
            }
        }
    }
    /// Positive admitted damage reaction. Crystal tests the actual damage,
    /// then selects the last eligible target weaker than its current target.
    pub(super) fn snow_wolf_attacked(
        &mut self,
        id: u32,
        damage: i32,
        human: bool,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(m) = self
            .native_monsters
            .get(&id)
            .cloned()
            .filter(|m| m.ai == 180 && !m.dead)
        else {
            return Vec::new();
        };
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return Vec::new();
        };
        initialize_kirin_snow(self.native_monsters.get_mut(&id).unwrap());
        let mut s = self.native_monsters[&id]
            .special_ai
            .as_ref()
            .unwrap()
            .kirin_snow
            .clone()
            .unwrap();
        let own = zone_roll_stat_range(
            t.min_dc,
            t.max_dc
                .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_DC)),
            now,
            id,
            roll(&mut s, id, now, u64::MAX),
        );
        let chance = roll(&mut s, id, now, if human { 2 } else { 10 }) == 0;
        let candidates = self.native_entity_targets(
            id,
            &m.position,
            i32::from(t.view_range),
            EntityTargetPurpose::VisibleImpact,
            now,
        );
        let current = self.selected_native_entity_target(id, now);
        let mut destination = None;
        if damage > own && chance && candidates.len() >= 2 {
            if let Some(current_dc) = current
                .as_ref()
                .and_then(|t| self.kirin_snow_target_min_dc(&t.reference))
            {
                if let Some(target) = candidates
                    .iter()
                    .filter(|t| {
                        self.kirin_snow_target_min_dc(&t.reference)
                            .is_some_and(|dc| dc < current_dc)
                    })
                    .last()
                {
                    s.target = Some(target.object_id);
                    s.entity_target = Some(target.reference.clone());
                    self.set_native_entity_target(id, &target.reference, now);
                    let direction =
                        zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
                    let point =
                        offset_point(&target.position, zone_rotated_direction(direction, 4), 1);
                    if point != m.position && self.can_native_monster_occupy(id, &point) {
                        destination = Some((point, direction));
                    }
                }
            }
        }
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .kirin_snow = Some(s);
        let Some((point, direction)) = destination else {
            return Vec::new();
        };
        let old = self.native_monster_visible_recipients(id, &m.position);
        let live = self.native_monsters.get_mut(&id).unwrap();
        live.position = point.clone();
        live.direction = direction;
        let Some(object) = self.objects.get_mut(&id) else {
            return Vec::new();
        };
        object.position = point.clone();
        if let ServerPacket::ObjectMonster { info } = &mut object.packet {
            info.location = point;
            info.direction = direction;
        }
        let packet = object.packet.clone();
        let mut out = self.diff_all_zone_object_visibility();
        let recipients = self
            .native_monster_visible_recipients(id, &self.native_monsters[&id].position)
            .into_iter()
            .filter(|s| old.contains(s))
            .collect();
        out.push(ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![ServerPacket::ObjectRemove { object_id: id }, packet],
        });
        out
    }
}

#[cfg(test)]
mod life_clear_tests {
    use super::*;

    #[test]
    fn adoption_preserves_lethal_hit_lifecycle_clears_and_checkpoint() {
        let mut z = ZoneRuntime::new(ZoneKey::for_map("snow-life-clear"));
        let session = SessionId::new("victim");
        z.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session.clone(),
            account_id: "test".into(),
            character_index: 1,
            object_id: 101,
            name: "victim".into(),
            class: MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 1,
            hp: 50,
            max_hp: 50,
            mp: 0,
            map_file_name: "snow-life-clear".into(),
            position: Point { x: 21, y: 20 },
            direction: MirDirection::Left,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        let spawn = ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 180,
            name: "ArcherGuard".into(),
            name_colour_argb: -1,
            image: 139,
            ai: 180,
            disposition: Some(crate::config::WorldEntityDisposition::Hostile),
            level: 1,
            max_hp: 100,
            hp: 100,
            experience: 0,
            move_speed_ms: 300,
            attack_speed_ms: 1000,
            friendly_guild: None,
            position: Point { x: 20, y: 20 },
            direction: MirDirection::Right,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        };
        assert!(z.spawn_authoritative_monster(&spawn, 0).0);
        initialize_kirin_snow(z.native_monsters.get_mut(&180).unwrap());
        let hit = |due, damage| KirinSnowHit {
            hit: PendingNativePlayerHit {
                ready_at_ms: due,
                attacker_object_id: 180,
                attacker_ai: 180,
                target_session_id: session.clone(),
                target_object_id: 101,
                damage,
                magic: false,
            },
            accuracy: None,
        };
        let s = z
            .native_monsters
            .get_mut(&180)
            .unwrap()
            .special_ai
            .as_mut()
            .unwrap()
            .kirin_snow
            .as_mut()
            .unwrap();
        s.hits = vec![hit(100, 100), hit(200, 10)];
        s.death_finished = true;
        s.death_ms = Some(100);
        s.slaves = vec![999];
        s.reward_owner = Some(session.clone());
        s.target = Some(101);
        z.tick_kirin_snow_delays(100);
        assert!(z.players[&session].dead);
        let s = z.native_monsters[&180]
            .special_ai
            .as_ref()
            .unwrap()
            .kirin_snow
            .as_ref()
            .unwrap();
        assert!(s.hits.is_empty());
        assert_eq!(s.target, None);
        assert_eq!(s.reward_owner, None);
        let mut restored = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
        restored.handle(ZoneCommand::SyncPlayerVitals {
            session_id: session.clone(),
            hp: 50,
            max_hp: 50,
            mp: 0,
        });
        restored.tick_kirin_snow_delays(200);
        assert_eq!(restored.players[&session].hp, 50);
    }
}
