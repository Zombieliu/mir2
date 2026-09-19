//! Crystal StoneTrap255. Struck immunity does not make Attacked invulnerable.
//! Its ProcessAI mutates newly constructed monsters, but MonsterObject.FindTarget
//! independently implements a real, qualified StoneTrap target preference.
use super::*;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct StoneTrapState {
    initialized: bool,
    die_at_ms: Option<u64>,
    corpse_at_ms: Option<u64>,
    #[serde(default)]
    incoming: Vec<StoneTrapHit>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoneTrapHit {
    source: u32,
    due: u64,
    damage: i32,
    accuracy: i32,
}
pub(in crate::runtime::zone) fn initialize_stone_trap(m: &mut ZoneNativeMonster) {
    if m.ai == 255 {
        m.special_ai
            .get_or_insert_with(Default::default)
            .stone_trap
            .get_or_insert_with(Default::default);
        m.visible_extra = true;
        // Constructor faces Up; HumanObject explicitly overrides summoned
        // stones with the owner's cast direction before Spawned.
        if m.owner_session_id.is_none() {
            m.direction = MirDirection::Up;
        }
    }
}
pub(super) fn stone_trap_allows_struck(m: &ZoneNativeMonster) -> bool {
    m.ai != 255
}
pub(super) fn sync_stone_trap_packet(
    m: &ZoneNativeMonster,
    packet: &mut ServerPacket,
    owner_name: Option<&str>,
) {
    if m.ai != 255 {
        return;
    }
    if let ServerPacket::ObjectMonster { info } = packet {
        info.image = 358;
        info.extra = true;
        info.direction = m.direction;
        info.name = if !m.dead {
            owner_name.map_or_else(|| m.name.clone(), |owner| format!("{}({})", m.name, owner))
        } else {
            m.name.clone()
        };
    }
}
impl ZoneRuntime {
    /// Called only after the existing decoy attack passes its attack-ready gate.
    /// Retained on the trap, so replacing its life discards its incoming swings.
    pub(super) fn queue_stone_trap_monster_hit(&mut self, source: u32, target: u32, now: u64) {
        if !self.stone_trap_decoy_eligible(source, target) {
            return;
        }
        let Some(m) = self.native_monsters.get(&source) else {
            return;
        };
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return;
        };
        let damage = zone_roll_stat_range(
            t.min_dc,
            t.max_dc
                .saturating_add(zone_native_monster_buff_stat_total(m, CRYSTAL_STAT_MAX_DC)),
            now,
            source,
            0x255,
        );
        if damage <= 0 {
            return;
        }
        let m = self.native_monsters.get_mut(&target).unwrap();
        initialize_stone_trap(m);
        m.special_ai
            .as_mut()
            .unwrap()
            .stone_trap
            .as_mut()
            .unwrap()
            .incoming
            .push(StoneTrapHit {
                source,
                due: now.saturating_add(500),
                damage,
                accuracy: t.accuracy,
            });
    }
    /// Root calls on attacker removal and before same-ID replacement. A corpse
    /// alone does not invalidate already launched MonsterObject actions.
    pub(super) fn invalidate_stone_trap_source(&mut self, source: u32) {
        for m in self.native_monsters.values_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.stone_trap.as_mut()) {
                s.incoming.retain(|h| h.source != source);
            }
        }
    }
    pub(super) fn tick_stone_trap_hits(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let mut due = Vec::new();
        for (&id, m) in self.native_monsters.iter_mut() {
            if let Some(s) = m.special_ai.as_mut().and_then(|s| s.stone_trap.as_mut()) {
                if m.dead {
                    s.incoming.clear();
                    continue;
                }
                for h in std::mem::take(&mut s.incoming) {
                    if now >= h.due {
                        due.push((id, h))
                    } else {
                        s.incoming.push(h)
                    }
                }
            }
        }
        let mut out = Vec::new();
        for (id, h) in due {
            let Some(source) = self.native_monsters.get(&h.source) else {
                continue;
            };
            let Some(target) = self
                .native_monsters
                .get(&id)
                .filter(|m| m.ai == 255 && !m.dead && m.owner_session_id.is_some())
                .cloned()
            else {
                continue;
            };
            let Some(owner) = target
                .owner_session_id
                .as_ref()
                .and_then(|s| self.players.get(s))
                .filter(|p| p.object_id == zone_native_summon_owner_player_object_id(&target))
            else {
                continue;
            };
            if source
                .friendly_guild
                .as_deref()
                .zip(owner.chat_profile.guild_name.as_deref())
                .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
            {
                continue;
            }
            if crate::runtime::combat::crystal_accuracy_roll(
                now,
                h.source,
                id,
                target.defense.agility.max(0) as u64 + 1,
            ) > h.accuracy.max(0) as u64
            {
                continue;
            }
            let armour = zone_roll_stat_range(
                target
                    .defense
                    .min_ac
                    .saturating_add(zone_native_monster_buff_stat_total(
                        &target,
                        CRYSTAL_STAT_MIN_AC,
                    )),
                target
                    .defense
                    .max_ac
                    .saturating_add(zone_native_monster_buff_stat_total(
                        &target,
                        CRYSTAL_STAT_MAX_AC,
                    )),
                now,
                h.source,
                0x255AC,
            );
            let damage = h.damage.saturating_sub(armour).max(0);
            if damage == 0 {
                continue;
            }
            // This is a wild monster attacking an owned object, not a player
            // earning kill credit. Never route through PendingNativeMonsterHit
            // with the trap owner's session, or award its XP/drop return values.
            let Some((damage, percent, killed, _, _, position, direction, _, _, _)) =
                self.apply_native_monster_damage(id, damage, None, now)
            else {
                continue;
            };
            let mut packets = vec![
                ServerPacket::ObjectStruck {
                    info: ObjectStruckInfo {
                        object_id: id,
                        attacker_id: h.source,
                        location: position.clone(),
                        direction,
                    },
                },
                ServerPacket::DamageIndicator {
                    damage,
                    damage_type: 0,
                    object_id: id,
                },
                ServerPacket::ObjectHealth {
                    info: ObjectHealthInfo {
                        object_id: id,
                        percent,
                        expire: 5,
                    },
                },
            ];
            if killed {
                packets.push(ServerPacket::ObjectDied {
                    info: ObjectDiedInfo {
                        object_id: id,
                        location: position.clone(),
                        direction,
                        kind: 0,
                    },
                });
            }
            self.apply_zone_object_packets(&packets, now);
            out.push(ZoneOutbound::ToMany {
                session_ids: self.native_monster_combat_recipients(h.source, id, &position),
                packets,
            });
            if killed {
                // Start the corpse clock at the actual fatal impact, not at
                // the next Zone update after the lifecycle pass already ran.
                out.extend(self.tick_stone_traps(now));
            }
        }
        out
    }
    /// Run after owned spawn expiry assignment and before the generic expiry
    /// pass. DieTime kills on strictly greater time; it does not erase the rock.
    pub(super) fn tick_stone_traps(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| (m.ai == 255).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            initialize_stone_trap(self.native_monsters.get_mut(&id).unwrap());
            let m = self.native_monsters[&id].clone();
            let mut s = m.special_ai.as_ref().unwrap().stone_trap.clone().unwrap();
            let owner = m
                .owner_session_id
                .as_ref()
                .and_then(|session| self.players.get(session))
                .filter(|p| p.object_id == zone_native_summon_owner_player_object_id(&m));
            let owner_name = owner.map(|p| p.name.clone());
            if !s.initialized {
                s.initialized = true;
                if let Some(object) = self.objects.get_mut(&id) {
                    s.die_at_ms = object.expires_at_ms.take();
                    sync_stone_trap_packet(&m, &mut object.packet, owner_name.as_deref());
                    out.push(ZoneOutbound::ToMany {
                        session_ids: self.native_monster_visible_recipients(id, &m.position),
                        packets: vec![ServerPacket::ObjectName {
                            object_id: id,
                            name: owner_name.as_ref().map_or_else(
                                || m.name.clone(),
                                |name| format!("{}({})", m.name, name),
                            ),
                        }],
                    });
                }
            }
            let should_die = !m.dead
                && m.owner_session_id.is_some()
                && (owner.is_none_or(|p| zone_tile_distance(&p.position, &m.position) > 15)
                    || s.die_at_ms.is_some_and(|due| now > due));
            if should_die {
                if let Some((_, _, killed, _, _, position, direction, _, _, _)) =
                    self.apply_native_monster_scripted_death_damage(id, None, now)
                {
                    if killed {
                        let packet = ServerPacket::ObjectDied {
                            info: ObjectDiedInfo {
                                object_id: id,
                                location: position.clone(),
                                direction,
                                kind: 0,
                            },
                        };
                        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
                        out.push(ZoneOutbound::ToMany {
                            session_ids: self.native_monster_visible_recipients(id, &position),
                            packets: vec![packet],
                        });
                    }
                }
            }
            let live = self.native_monsters[&id].clone();
            if live.dead && s.corpse_at_ms.is_none() {
                s.corpse_at_ms = Some(now.saturating_add(180000));
                if let Some(object) = self.objects.get_mut(&id) {
                    object.expires_at_ms = s.corpse_at_ms;
                    sync_stone_trap_packet(&live, &mut object.packet, None);
                }
                out.push(ZoneOutbound::ToMany {
                    session_ids: self.native_monster_visible_recipients(id, &live.position),
                    packets: vec![ServerPacket::ObjectName {
                        object_id: id,
                        name: live.name.clone(),
                    }],
                });
            }
            self.native_monsters
                .get_mut(&id)
                .unwrap()
                .special_ai
                .as_mut()
                .unwrap()
                .stone_trap = Some(s);
        }
        out
    }
    /// Tighten the existing StoneTrap-specific decoy selector; this is not a
    /// general monster targeting implementation. No unowned/friendly impostor.
    pub(super) fn stone_trap_decoy_eligible(&self, source_id: u32, trap_id: u32) -> bool {
        let Some(source) = self
            .native_monsters
            .get(&source_id)
            .filter(|m| !m.dead && m.hostile_to_player)
        else {
            return false;
        };
        let Some(trap) = self
            .native_monsters
            .get(&trap_id)
            .filter(|m| m.ai == 255 && !m.dead && m.hp > 0 && monster_visibility_is_visible(m))
        else {
            return false;
        };
        let Some(owner) = trap
            .owner_session_id
            .as_ref()
            .and_then(|id| self.players.get(id))
            .filter(|p| p.object_id == zone_native_summon_owner_player_object_id(trap))
        else {
            return false;
        };
        if source
            .friendly_guild
            .as_deref()
            .zip(owner.chat_profile.guild_name.as_deref())
            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
        {
            return false;
        }
        let Some(t) = crystal_monster_by_name(&source.name) else {
            return false;
        };
        zone_tile_distance(&source.position, &trap.position) <= i32::from(t.view_range)
    }
}
