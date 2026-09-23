//! AI41/42: Crystal YinDevilNode.ProcessTarget / CompleteAttack.
//! Preserve the source's unusual two filters: FindFriendsNearby starts a cast,
//! but completion uses FindAllTargets (attackable) then IsFriendlyTarget.
//! HumanObject.IsFriendlyTarget(MonsterObject) returns true; ordinary wild
//! monsters are only attackable by a wild node under hallucination/rage.
use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct NodeAiState {
    pending_completion_ms: Option<u64>,
}

impl ZoneRuntime {
    pub(super) fn tick_node_ai(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| matches!(m.ai, 41 | 42).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            let monster = self.native_monsters[&id].clone();
            let pending = monster
                .special_ai
                .as_ref()
                .and_then(|s| s.node.as_ref())
                .and_then(|s| s.pending_completion_ms);
            if pending.is_some_and(|due| now_ms >= due) {
                self.native_monsters
                    .get_mut(&id)
                    .unwrap()
                    .special_ai
                    .as_mut()
                    .unwrap()
                    .node
                    .as_mut()
                    .unwrap()
                    .pending_completion_ms = None;
                out.extend(self.complete_node_cast(id, now_ms));
            }
            // Crystal Die leaves delayed actions alive; Despawn removes them.
            // A corpse may finish a cast but never initiate another one.
            if monster.dead
                || monster.hp <= 0
                || native_monster_control_active(&monster, now_ms)
                || now_ms < monster.next_ai_ready_at_ms
                || now_ms < monster.next_attack_ready_at_ms
                || pending.is_some_and(|due| now_ms < due)
            {
                continue;
            }
            let friends = self.native_monsters.iter().any(|(&other, m)| {
                other != id
                    && !m.dead
                    && m.hp > 0
                    && m.owner_session_id.is_none()
                    && monster.owner_session_id.is_none()
                    && now_ms >= monster.hallucination_until_ms
                    && zone_tile_distance(&monster.position, &m.position) <= 7
            });
            if !friends {
                continue;
            }
            let m = self.native_monsters.get_mut(&id).unwrap();
            m.next_ai_ready_at_ms = now_ms.saturating_add(300);
            m.next_attack_ready_at_ms = now_ms.saturating_add(m.attack_speed_ms);
            m.special_ai
                .get_or_insert_with(Default::default)
                .node
                .get_or_insert_with(Default::default)
                .pending_completion_ms = Some(now_ms.saturating_add(500));
            let packet = ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id: id,
                    location: monster.position.clone(),
                    direction: monster.direction,
                    spell: 0,
                    level: 0,
                    attack_type: 0,
                },
            };
            out.push(ZoneOutbound::ToMany {
                session_ids: self.native_monster_visible_recipients(id, &monster.position),
                packets: vec![packet],
            });
        }
        out
    }

    fn complete_node_cast(&mut self, id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
        let source = self.native_monsters[&id].clone();
        let (buff_type, stat) = if source.ai == 41 {
            (CRYSTAL_BLESSED_ARMOUR_BUFF_TYPE, CRYSTAL_STAT_MAX_AC)
        } else {
            (CRYSTAL_ULTIMATE_ENHANCER_BUFF_TYPE, CRYSTAL_STAT_MAX_DC)
        };
        let players: Vec<_> = self
            .players
            .values()
            .filter(|p| {
                source.hostile_to_player
                    && !p.dead
                    && !p.hidden
                    && !p.chat_profile.in_safe_zone
                    && zone_tile_distance(&source.position, &p.position) <= 7
                    && !source
                        .friendly_guild
                        .as_deref()
                        .zip(p.chat_profile.guild_name.as_deref())
                        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
            })
            .map(|p| {
                (
                    p.session_id.clone(),
                    p.object_id,
                    p.position.clone(),
                    p.level,
                )
            })
            .collect();
        let mut out = Vec::new();
        for (session, object_id, position, level) in players {
            let packets = self.apply_native_player_buff(
                &session,
                buff_type,
                5_000,
                vec![UserItemStat {
                    stat,
                    value: i32::from(level) / 7 + 4,
                }],
                now_ms,
            );
            out.push(ZoneOutbound::ToMany {
                session_ids: self.player_status_recipients(object_id, &position),
                packets,
            });
        }
        // Monster.IsFriendlyTarget rejects pets, while FindAllTargets rejects
        // ordinary wild allies unless the node's hallucination makes them
        // attackable. Rage has no authoritative Zone field yet (OPEN).
        if now_ms < source.hallucination_until_ms && source.owner_session_id.is_none() {
            let targets: Vec<_> = self
                .native_monsters
                .iter()
                .filter_map(|(&other, m)| {
                    (other != id
                        && !m.dead
                        && m.hp > 0
                        && m.owner_session_id.is_none()
                        && monster_visibility_is_attackable(m)
                        && zone_tile_distance(&source.position, &m.position) <= 7)
                        .then_some(other)
                })
                .collect();
            for other in targets {
                let m = self.native_monsters.get_mut(&other).unwrap();
                let buff = ClientBuff {
                    buff_type,
                    visible: true,
                    object_id: other,
                    expire_time: 5_000,
                    infinite: false,
                    paused: false,
                    stats: vec![UserItemStat {
                        stat,
                        value: i32::from(m.level) / 7 + 4,
                    }],
                    values: Vec::new(),
                };
                // Replace by type, never mutate base AC/DC or compound bonuses.
                m.buffs.insert(
                    buff_type,
                    super::super::types::ZonePlayerBuff {
                        buff: buff.clone(),
                        expires_at_ms: Some(now_ms.saturating_add(5_000)),
                        notify_owner_on_expiry: false,
                    },
                );
                let position = m.position.clone();
                let packet = ServerPacket::AddBuff { buff };
                self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
                out.push(ZoneOutbound::ToMany {
                    session_ids: self.native_monster_visible_recipients(other, &position),
                    packets: vec![packet],
                });
            }
        }
        out
    }
}
