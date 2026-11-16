//! Crystal TownArcher.cs (AI57), distinct from conquest archer AI80.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TownArcherState {
    target: Option<(SessionId, u32)>,
    fear_until_ms: u64,
    action_until_ms: u64,
    home_direction: MirDirection,
    cool_eye: bool,
    #[serde(default)]
    arrows: Vec<TownArrow>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TownArrow {
    hit: PendingNativePlayerHit,
    accuracy: i32,
}

pub(in crate::runtime::zone) fn initialize_town_archer(
    m: &mut ZoneNativeMonster,
    id: u32,
    now: u64,
) {
    if m.ai != 57 {
        return;
    }
    let cool_eye = crystal_monster_by_name(&m.name).is_some_and(|t| {
        u64::from(t.cool_eye) > crate::runtime::combat::crystal_accuracy_roll(now, id, id, 100)
    });
    m.special_ai
        .get_or_insert_with(Default::default)
        .town_archer
        .get_or_insert(TownArcherState {
            target: None,
            fear_until_ms: 0,
            action_until_ms: now.saturating_add(2000),
            home_direction: m.direction,
            cool_eye,
            arrows: Vec::new(),
        });
}

impl ZoneRuntime {
    /// Shared player life replacement/revival must discard shots from the old life.
    pub(super) fn clear_town_archer_arrows_for_target(&mut self, target: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.town_archer.as_mut()) {
                s.arrows
                    .retain(|arrow| arrow.hit.target_object_id != target);
                if s.target.as_ref().is_some_and(|(_, id)| *id == target) {
                    s.target = None;
                }
            }
        }
    }
    /// Call before generic disposition/decoy dispatch, after shared control gates.
    /// No Route exists in shared spawn metadata: route-free town archers stand.
    pub(super) fn try_tick_town_archer(&mut self, id: u32, now: u64) -> Option<Vec<ZoneOutbound>> {
        let m = self.native_monsters.get(&id)?.clone();
        if m.ai != 57 {
            return None;
        }
        if m.dead || m.disposition.is_none() {
            return Some(Vec::new());
        }
        initialize_town_archer(self.native_monsters.get_mut(&id)?, id, now);
        let state = self.native_monsters[&id]
            .special_ai
            .as_ref()?
            .town_archer
            .as_ref()?
            .clone();
        if now <= state.action_until_ms || now <= m.next_attack_ready_at_ms {
            return Some(Vec::new());
        }
        let Some(template) = crystal_monster_by_name(&m.name) else {
            return Some(Vec::new());
        };
        let distance = |p: &Point| (p.x - m.position.x).abs().max((p.y - m.position.y).abs());
        // PK threshold belongs to FindTarget. A previously acquired red player
        // is retained after PK falls, just as source ProcessTarget/IsAttackTarget.
        let target = state
            .target
            .as_ref()
            .and_then(|(session, object_id)| {
                self.players.get(session).filter(|p| {
                    !p.dead && p.object_id == *object_id && !p.chat_profile.in_safe_zone
                })
            })
            .or_else(|| {
                self.players
                    .values()
                    .filter(|p| {
                        !p.dead
                            && !p.chat_profile.in_safe_zone
                            && p.chat_profile.pk_points >= 200
                            && distance(&p.position) <= i32::from(template.view_range)
                            && (!p.hidden || (state.cool_eye && m.level >= p.level))
                    })
                    .min_by_key(|p| {
                        (
                            distance(&p.position),
                            p.position.y,
                            p.position.x,
                            p.object_id,
                        )
                    })
            })
            .map(|p| (p.session_id.clone(), p.object_id, p.position.clone()));
        let Some((session, target_id, position)) = target else {
            self.native_monsters
                .get_mut(&id)?
                .special_ai
                .as_mut()?
                .town_archer
                .as_mut()?
                .target = None;
            return Some(Vec::new());
        };
        let dist = distance(&position);
        let s = self
            .native_monsters
            .get_mut(&id)?
            .special_ai
            .as_mut()?
            .town_archer
            .as_mut()?;
        s.target = Some((session.clone(), target_id));
        if dist > 10 || now >= s.fear_until_ms {
            s.fear_until_ms = now.saturating_add(2000);
            if dist > 10 {
                s.target = None;
                return Some(self.turn_native_monster(id, state.home_direction, now));
            }
            return Some(Vec::new());
        }
        let direction = zone_direction_toward(&m.position, &position).unwrap_or(m.direction);
        let damage = zone_roll_stat_range(
            template.min_dc,
            template
                .max_dc
                .saturating_add(zone_native_monster_buff_stat_total(&m, CRYSTAL_STAT_MAX_DC)),
            now,
            id,
            0x57,
        );
        let live = self.native_monsters.get_mut(&id)?;
        live.direction = direction;
        live.next_attack_ready_at_ms = now.saturating_add(live.attack_speed_ms);
        let s = live.special_ai.as_mut()?.town_archer.as_mut()?;
        s.action_until_ms = now.saturating_add(300);
        if damage > 0 {
            s.arrows.push(TownArrow {
                accuracy: template.accuracy,
                hit: PendingNativePlayerHit {
                    ready_at_ms: now.saturating_add(500 + dist as u64 * 50),
                    attacker_object_id: id,
                    attacker_ai: 57,
                    target_session_id: session,
                    target_object_id: target_id,
                    damage,
                    magic: false,
                },
            });
        }
        let packet = ServerPacket::ObjectRangeAttack {
            info: ObjectRangeAttackInfo {
                object_id: id,
                location: m.position.clone(),
                direction,
                target_id,
                target: position.clone(),
                attack_type: 0,
                spell: 0,
                level: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        Some(vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_combat_recipients(id, target_id, &position),
            packets: vec![packet],
        }])
    }

    /// Tick globally even when shooter is dead/controlled. Despawn removes the
    /// stored arrows; owner session/object identity and safe-zone impact gates
    /// are validated by the common authoritative damage resolver.
    pub(super) fn tick_town_archer_arrows(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let mut due = Vec::new();
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.town_archer.as_mut()) {
                let pending = std::mem::take(&mut s.arrows);
                for arrow in pending {
                    if now >= arrow.hit.ready_at_ms {
                        due.push(arrow);
                    } else {
                        s.arrows.push(arrow);
                    }
                }
            }
        }
        let mut out = Vec::new();
        for arrow in due {
            if !self
                .native_monsters
                .get(&arrow.hit.attacker_object_id)
                .is_some_and(|m| m.disposition.is_some())
            {
                continue;
            }
            let Some(p) = self.players.get(&arrow.hit.target_session_id) else {
                continue;
            };
            if crate::runtime::combat::crystal_accuracy_roll(
                now,
                arrow.hit.attacker_object_id,
                p.object_id,
                p.combat_stats.agility.max(0) as u64 + 1,
            ) > arrow.accuracy.max(0) as u64
            {
                continue;
            }
            out.extend(self.resolve_pending_native_player_hit(arrow.hit, now));
        }
        out
    }
}
