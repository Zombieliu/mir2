//! Crystal ExplosiveTrap: one pending action is one linked trap group. Each
//! retained ObjectSpell has its own Param/deadline, just like SpellObject;
//! deleting one tile detonates (and renews) its surviving siblings.
//!
//! Source: HumanObject.cs:5630, Map.cs:1900..1957 and
//! SpellObject.cs:57/99/194/411/589. This path must run before generic ground
//! expiry and before expire_zone_objects. It owns its spawn/update/removal
//! packets and authoritative object retention; the generic AoE must not run.
use super::*;

const SOURCE_TRAP_TICK_MS: u64 = 500;
const SOURCE_TRAP_DETONATION_MS: u64 = 1_000;
const SOURCE_TRAP_OWNER_RANGE: i32 = 15;

fn source_trap_points(front: &Point, direction: MirDirection) -> [Point; 3] {
    [
        front.clone(),
        offset_point(front, zone_rotated_direction(direction, 6), 1),
        offset_point(front, zone_rotated_direction(direction, 2), 1),
    ]
}

fn source_trap_retire_at(now: u64, duration: u64) -> u64 {
    // Source checks Time > ExpireTime; generic retained objects expire at >=.
    // The first representable millisecond after Source's deadline preserves
    // that strict boundary without a second, divergent expiry mechanism.
    now.saturating_add(duration).saturating_add(1)
}

impl ZoneRuntime {
    /// HumanObject counts distinct already-spawned groups, not queued casts
    /// and not the number of individual tiles. The first reserved object ID is
    /// a stable group identity even after that first tile has despawned.
    pub(super) fn can_queue_explosive_trap(
        &self,
        session: &SessionId,
        object_id: u32,
        level: u8,
        now: u64,
    ) -> bool {
        let Some(owner) = self
            .owned_pet_owner(session)
            .filter(|owner| owner.online.object_id == object_id)
        else {
            return false;
        };
        let groups = self
            .pending_native_ground_spells
            .iter()
            .filter(|action| {
                action.spell == Spell::ExplosiveTrap
                    && action.caster_session_id == *session
                    && action.caster_object_id == object_id
                    && action.owned_caster.as_ref() == Some(&owner)
                    && action.spell_packets_sent
                    && action.spell_object_ids.iter().any(|id| {
                        self.five_class_trap_tile(*id).is_some()
                            && self.objects[id]
                                .expires_at_ms
                                .is_some_and(|expiry| now < expiry)
                    })
            })
            .filter_map(|action| action.spell_object_ids.first().copied())
            .collect::<BTreeSet<_>>();
        groups.len() < usize::from(level) + 1
    }

    /// AOI/bootstrap must use this as well as geometric visibility. Undetonated
    /// traps are visible only to the caster and PlayerObject.IsFriendlyTarget;
    /// the explosion (Param=true) is public. No missing owner proof becomes a
    /// public, undetonated trap after checkpoint/rejoin.
    pub(super) fn can_see_five_class_trap(
        &self,
        viewer: &ZonePlayer,
        info: &ObjectSpellInfo,
        now: u64,
    ) -> bool {
        if info.spell != Spell::ExplosiveTrap || info.param {
            return true;
        }
        self.pending_native_ground_spells
            .iter()
            .filter(|action| {
                action.spell == Spell::ExplosiveTrap
                    && action.spell_packets_sent
                    && action.spell_object_ids.contains(&info.object_id)
            })
            .filter_map(|action| action.owned_caster.as_ref())
            .filter(|owner| self.owned_pet_owner_is_current(owner))
            .filter_map(|owner| self.players.get(&owner.online.session_id))
            .any(|caster| self.native_purification_players_are_friendly(caster, viewer, now))
    }

    pub(super) fn tick_five_class_explosive_trap(
        &mut self,
        action: &mut PendingNativeGroundSpellAction,
        occupied_spell_tiles: &mut BTreeSet<(i32, i32)>,
        now: u64,
    ) -> (bool, Vec<ZoneOutbound>) {
        if action.spell != Spell::ExplosiveTrap {
            return (false, Vec::new());
        }
        let mut out = Vec::new();
        let owner = action.owned_caster.as_ref().filter(|owner| {
            self.owned_pet_caster_proof_is_current(
                owner,
                &action.caster_session_id,
                action.caster_object_id,
            )
        });
        let Some(owner) = owner.cloned() else {
            // Node/online/life revocation removes the traps without transferring
            // former Human authority or generating placement practice.
            for id in action.spell_object_ids.clone() {
                self.remove_five_class_trap_tile(action, id, occupied_spell_tiles, now, &mut out);
            }
            return (false, out);
        };

        if !action.spell_packets_sent {
            if now < action.spell_packets_at_ms {
                return (true, out);
            }
            let Some(front) = action.locations.first().cloned() else {
                return (false, out);
            };
            if self.collision.is_blocked(&front)
                || occupied_spell_tiles.contains(&(front.x, front.y))
                || self.five_class_trap_square_occupied(&front, now)
            {
                // Map cancels the whole placement at a blocked/overlapped
                // front. Its LevelMagic call occurs only after this check.
                return (false, out);
            }
            let caster = self.players[&action.caster_session_id].clone();
            let expiry = source_trap_retire_at(
                now,
                10_000_u64.saturating_add(
                    u64::try_from(action.damage.max(0) / 2)
                        .unwrap_or_default()
                        .saturating_mul(1_000),
                ),
            );
            let mut locations = Vec::new();
            let mut ids = Vec::new();
            // The front is captured at cast; Map takes the Human's current
            // Direction at delayed placement for Left/Right.
            for (index, location) in source_trap_points(&front, caster.direction)
                .into_iter()
                .enumerate()
            {
                if index > 0
                    && (self.collision.is_blocked(&location)
                        || occupied_spell_tiles.contains(&(location.x, location.y))
                        || self.five_class_trap_square_occupied(&location, now))
                {
                    continue;
                }
                let id = action
                    .spell_object_ids
                    .get(index)
                    .copied()
                    .filter(|id| !self.object_id_in_use(*id))
                    .unwrap_or_else(|| self.unique_object_id(0));
                locations.push(location);
                ids.push(id);
            }
            action.locations = locations;
            action.spell_object_ids = ids;
            action.spell_packets_sent = true;
            action.trap_detonated = false;
            action.expires_at_ms = expiry;
            action.next_damage_at_ms = now;
            action.tick_interval_ms = SOURCE_TRAP_TICK_MS;

            let packets = action
                .spell_object_ids
                .iter()
                .zip(&action.locations)
                .map(|(&object_id, location)| ServerPacket::ObjectSpell {
                    info: ObjectSpellInfo {
                        object_id,
                        location: location.clone(),
                        spell: Spell::ExplosiveTrap,
                        // Map's new SpellObject leaves Direction at its default.
                        direction: MirDirection::Up,
                        param: false,
                    },
                })
                .collect::<Vec<_>>();
            self.apply_zone_object_packets(&packets, now);
            for packet in &packets {
                if let ServerPacket::ObjectSpell { info } = packet {
                    // The generic personal compatibility cache does not
                    // retain ObjectSpell. These real Zone-owned traps do.
                    self.object_grid.insert(info.object_id, &info.location);
                    self.objects.insert(
                        info.object_id,
                        ZoneObject {
                            object_id: info.object_id,
                            position: info.location.clone(),
                            packet: packet.clone(),
                            health: None,
                            mana: None,
                            expires_at_ms: Some(expiry),
                            buffs: BTreeMap::new(),
                        },
                    );
                    occupied_spell_tiles.insert((info.location.x, info.location.y));
                }
            }
            let recipients = self
                .ground_spell_visible_recipients(&action.caster_session_id, &action.locations)
                .into_iter()
                .filter(|session| {
                    self.players.get(session).is_some_and(|viewer| {
                        self.native_purification_players_are_friendly(&caster, viewer, now)
                    })
                })
                .collect::<Vec<_>>();
            self.broadcast_five_class_trap_packets(recipients, packets, &mut out);
            out.extend(self.five_class_practice(
                &action.caster_session_id,
                Spell::ExplosiveTrap,
                now,
            ));
        }

        // Read each deadline immediately before processing the tile. Despawning
        // an earlier sibling renews every remaining sibling, including an
        // already-detonated one (Source Despawn -> DetonateTrapNow).
        for id in action.spell_object_ids.clone() {
            let Some(info) = self.five_class_trap_tile(id).cloned() else {
                continue;
            };
            let caster_position = &self.players[&action.caster_session_id].position;
            let beyond_owner_range =
                zone_tile_distance(caster_position, &info.location) > SOURCE_TRAP_OWNER_RANGE;
            let expired = self.objects[&id]
                .expires_at_ms
                .is_some_and(|expiry| now >= expiry);
            if expired || beyond_owner_range {
                self.remove_five_class_trap_tile(action, id, occupied_spell_tiles, now, &mut out);
                for sibling in action.spell_object_ids.clone() {
                    self.detonate_five_class_trap_tile(action, sibling, now, &mut out);
                }
                continue;
            }
            if info.param || now < action.next_damage_at_ms {
                continue;
            }
            let candidate_ids = self
                .players
                .values()
                .filter(|player| player.position == info.location)
                .map(|player| player.object_id)
                .chain(
                    self.native_monsters
                        .iter()
                        .filter(|(_, monster)| monster.position == info.location)
                        .map(|(&id, _)| id),
                )
                .collect::<BTreeSet<_>>();
            let target = candidate_ids
                .into_iter()
                .filter_map(|id| self.five_class_target(id))
                .find(|target| {
                    self.five_class_target_hostile(&action.caster_session_id, target, now)
                });
            if let Some(target) = target {
                // Detonation precedes Attacked, including a lawful attack fully
                // absorbed by MAC. A second same-cell object cannot consume it.
                self.detonate_five_class_trap_tile(action, id, now, &mut out);
                let (_, hits) =
                    self.five_class_hit(&owner, &target, action.damage, EntityDefence::MAC, now);
                out.extend(hits);
            }
        }
        if now >= action.next_damage_at_ms {
            action.next_damage_at_ms = now.saturating_add(SOURCE_TRAP_TICK_MS);
        }
        let tiles = action
            .spell_object_ids
            .iter()
            .filter_map(|id| {
                let info = self.five_class_trap_tile(*id)?;
                Some((info.param, self.objects[id].expires_at_ms?))
            })
            .collect::<Vec<_>>();
        action.trap_detonated = !tiles.is_empty() && tiles.iter().all(|(param, _)| *param);
        if let Some(expiry) = tiles.iter().map(|(_, expiry)| *expiry).max() {
            action.expires_at_ms = expiry;
        }
        (!tiles.is_empty(), out)
    }

    fn five_class_trap_tile(&self, id: u32) -> Option<&ObjectSpellInfo> {
        match &self.objects.get(&id)?.packet {
            ServerPacket::ObjectSpell { info } if info.spell == Spell::ExplosiveTrap => Some(info),
            _ => None,
        }
    }

    fn five_class_trap_square_occupied(&self, point: &Point, now: u64) -> bool {
        self.objects.values().any(|object| {
            object.position == *point
                && object.expires_at_ms.is_none_or(|expiry| now < expiry)
                && matches!(&object.packet, ServerPacket::ObjectSpell { info }
                    if matches!(info.spell, Spell::FireWall | Spell::ExplosiveTrap))
        })
    }

    fn broadcast_five_class_trap_packets(
        &mut self,
        recipients: Vec<SessionId>,
        packets: Vec<ServerPacket>,
        out: &mut Vec<ZoneOutbound>,
    ) {
        if recipients.is_empty() || packets.is_empty() {
            return;
        }
        for session in &recipients {
            if let Some(player) = self.players.get_mut(session) {
                for packet in &packets {
                    if let ServerPacket::ObjectSpell { info } = packet {
                        player.visible_object_ids.insert(info.object_id);
                    }
                }
            }
        }
        out.push(ZoneOutbound::ToMany {
            session_ids: recipients,
            packets,
        });
    }

    fn detonate_five_class_trap_tile(
        &mut self,
        action: &PendingNativeGroundSpellAction,
        id: u32,
        now: u64,
        out: &mut Vec<ZoneOutbound>,
    ) {
        let Some(mut info) = self.five_class_trap_tile(id).cloned() else {
            return;
        };
        info.param = true;
        let recipients = self.ground_spell_visible_recipients(
            &action.caster_session_id,
            std::slice::from_ref(&info.location),
        );
        let packets = vec![ServerPacket::ObjectSpell { info }];
        self.apply_zone_object_packets(&packets, now);
        if let Some(object) = self.objects.get_mut(&id) {
            object.packet = packets[0].clone();
            object.expires_at_ms = Some(source_trap_retire_at(now, SOURCE_TRAP_DETONATION_MS));
        }
        self.broadcast_five_class_trap_packets(recipients, packets, out);
    }

    fn remove_five_class_trap_tile(
        &mut self,
        action: &PendingNativeGroundSpellAction,
        id: u32,
        occupied_spell_tiles: &mut BTreeSet<(i32, i32)>,
        now: u64,
        out: &mut Vec<ZoneOutbound>,
    ) {
        let Some(info) = self.five_class_trap_tile(id).cloned() else {
            return;
        };
        let recipients = self.ground_spell_visible_recipients(
            &action.caster_session_id,
            std::slice::from_ref(&info.location),
        );
        self.remove_retained_zone_object(id);
        if !self.five_class_trap_square_occupied(&info.location, now) {
            occupied_spell_tiles.remove(&(info.location.x, info.location.y));
        }
        if !recipients.is_empty() {
            out.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![ServerPacket::ObjectRemove { object_id: id }],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldEntityDisposition;
    use crate::runtime::zone::types::{ZoneChatProfile, ZoneMonsterDefense, ZonePlayerCombatStats};

    const OWNER: u32 = 101;

    fn fixture() -> (ZoneRuntime, SessionId) {
        let session = SessionId::new("explosive-owner");
        let mut zone = ZoneRuntime::new_with_collision(
            ZoneKey::for_map("five-class-explosive-unit"),
            ZoneCollision::unbounded(),
        );
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session.clone(),
            account_id: "explosive-account".into(),
            character_index: 1,
            object_id: OWNER,
            name: "TrapArcher".into(),
            class: MirClass::Archer,
            gender: mir2_protocol::MirGender::Male,
            level: 50,
            hp: 500,
            max_hp: 500,
            mp: 500,
            map_file_name: "five-class-explosive-unit".into(),
            position: Point { x: 10, y: 10 },
            direction: MirDirection::Right,
            chat_profile: ZoneChatProfile {
                attack_mode: 5,
                ..Default::default()
            },
            combat_stats: ZonePlayerCombatStats {
                min_mc: 50,
                max_mc: 50,
                accuracy: 100,
                five_class_learned_mask: (1u64 << 34) - 1,
                ..Default::default()
            },
        }));
        assert!(zone.owned_pet_owner(&session).is_some());
        (zone, session)
    }

    fn queued(
        zone: &mut ZoneRuntime,
        session: &SessionId,
        front: Point,
    ) -> PendingNativeGroundSpellAction {
        let locations = source_trap_points(&front, MirDirection::Right).to_vec();
        let spell_object_ids = (0..3).map(|_| zone.unique_object_id(0)).collect();
        PendingNativeGroundSpellAction {
            spell: Spell::ExplosiveTrap,
            skill_level: 0,
            caster_session_id: session.clone(),
            caster_object_id: OWNER,
            owned_caster: zone.owned_pet_owner(session),
            locations,
            spell_object_ids,
            direction: MirDirection::Right,
            spell_param: 0,
            damage: 21,
            spell_packets_at_ms: 500,
            spell_packets_sent: false,
            trap_detonated: false,
            next_damage_at_ms: 500,
            expires_at_ms: 10_500,
            tick_interval_ms: 500,
            journey_event: None,
        }
    }

    fn practice_count(out: &[ZoneOutbound]) -> usize {
        out.iter()
            .filter(|packet| matches!(packet, ZoneOutbound::FiveClassMagicPractice { .. }))
            .count()
    }

    fn spawn(zone: &mut ZoneRuntime, id: u32, point: Point, mac: i32) {
        assert!(
            zone.spawn_world_event_monster(
                &ZoneMonsterSpawn {
                    crystal_drop_seed: None,
                    object_id: id,
                    name: "Scarecrow".into(),
                    name_colour_argb: -1,
                    image: 5,
                    ai: 0,
                    disposition: Some(WorldEntityDisposition::Hostile),
                    level: 1,
                    hp: 500,
                    max_hp: 500,
                    experience: 0,
                    move_speed_ms: 60_000,
                    attack_speed_ms: 60_000,
                    friendly_guild: None,
                    position: point,
                    direction: MirDirection::Left,
                    defense: ZoneMonsterDefense {
                        min_mac: mac,
                        max_mac: mac,
                        ..Default::default()
                    },
                    respawn: None,
                    drops: Vec::new(),
                },
                0
            )
            .0
        );
    }

    fn observer(
        zone: &mut ZoneRuntime,
        session: &str,
        id: u32,
        point: Point,
        attack_mode: u8,
    ) -> SessionId {
        let session = SessionId::new(session);
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session.clone(),
            account_id: session.as_str().into(),
            character_index: 2,
            object_id: id,
            name: session.as_str().into(),
            class: MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 50,
            hp: 500,
            max_hp: 500,
            mp: 500,
            map_file_name: "five-class-explosive-unit".into(),
            position: point,
            direction: MirDirection::Left,
            chat_profile: ZoneChatProfile {
                attack_mode,
                ..Default::default()
            },
            combat_stats: ZonePlayerCombatStats {
                min_mac: 5,
                max_mac: 5,
                ..Default::default()
            },
        }));
        session
    }

    #[test]
    fn five_class_explosive_source_birth_is_delayed_and_practices_once_after_valid_front() {
        let (mut zone, owner) = fixture();
        let mut action = queued(&mut zone, &owner, Point { x: 11, y: 10 });
        let mut occupied = BTreeSet::new();
        let (retain, out) = zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 499);
        assert!(retain && !action.spell_packets_sent && out.is_empty());
        let (retain, out) = zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 500);
        assert!(retain && action.spell_packets_sent);
        assert_eq!(practice_count(&out), 1);
        assert_eq!(
            action.locations,
            vec![
                Point { x: 11, y: 10 },
                Point { x: 11, y: 9 },
                Point { x: 11, y: 11 }
            ]
        );
        assert_eq!(action.expires_at_ms, 20_501); // (10 + 21/2)s, strict Source >.
        for id in &action.spell_object_ids {
            assert_eq!(zone.objects[id].expires_at_ms, Some(20_501));
            assert!(!zone.five_class_trap_tile(*id).unwrap().param);
        }
        let (_, out) = zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 1_000);
        assert_eq!(practice_count(&out), 0);
    }

    #[test]
    fn five_class_explosive_front_overlap_cancels_but_side_overlap_only_skips_that_tile() {
        let (mut zone, owner) = fixture();
        let mut action = queued(&mut zone, &owner, Point { x: 11, y: 10 });
        let mut occupied = BTreeSet::from([(11, 10)]);
        let (retain, out) = zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 500);
        assert!(!retain && !action.spell_packets_sent && out.is_empty());
        occupied = BTreeSet::from([(11, 9)]);
        let (retain, out) = zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 500);
        assert!(retain);
        assert_eq!(practice_count(&out), 1);
        assert_eq!(
            action.locations,
            vec![Point { x: 11, y: 10 }, Point { x: 11, y: 11 }]
        );
        assert_eq!(action.spell_object_ids.len(), 2);
    }

    #[test]
    fn five_class_explosive_counts_live_groups_and_keeps_group_identity_after_front_retires() {
        let (mut zone, owner) = fixture();
        let mut action = queued(&mut zone, &owner, Point { x: 11, y: 10 });
        let mut occupied = BTreeSet::new();
        zone.pending_native_ground_spells.push(action.clone());
        assert!(zone.can_queue_explosive_trap(&owner, OWNER, 0, 0)); // queued != spawned.
        zone.pending_native_ground_spells.clear();
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 500);
        let front = action.spell_object_ids[0];
        zone.pending_native_ground_spells.push(action.clone());
        zone.pending_native_ground_spells.push(action.clone()); // one distinct group.
        assert!(!zone.can_queue_explosive_trap(&owner, OWNER, 0, 500));
        assert!(zone.can_queue_explosive_trap(&owner, OWNER, 1, 500));
        zone.remove_retained_zone_object(front);
        assert!(!zone.can_queue_explosive_trap(&owner, OWNER, 0, 500));
        for id in action.spell_object_ids.iter().skip(1) {
            zone.remove_retained_zone_object(*id);
        }
        assert!(zone.can_queue_explosive_trap(&owner, OWNER, 0, 500));
        assert!(!zone.can_queue_explosive_trap(&owner, OWNER + 1, 3, 500));
    }

    #[test]
    fn five_class_explosive_first_lawful_overlap_hits_mac_once_not_every_500ms() {
        let (mut zone, owner) = fixture();
        let mut action = queued(&mut zone, &owner, Point { x: 11, y: 10 });
        let mut occupied = BTreeSet::new();
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 500);
        // Two actual objects deliberately share a cell, matching the source
        // Cell.Objects loop. Only the first lawful object can consume a tile.
        spawn(&mut zone, 9_001, Point { x: 11, y: 10 }, 5);
        spawn(&mut zone, 9_002, Point { x: 11, y: 10 }, 0);
        // Ordinary spawn correctly separates blocking monsters. Exercise the
        // independent SpellObject cell-consumption rule with an explicit
        // overlapping-cell fixture, keeping all real actor Nodes and lives.
        zone.native_monsters.get_mut(&9_002).unwrap().position = Point { x: 11, y: 10 };
        let object = zone.objects.get_mut(&9_002).unwrap();
        object.position = Point { x: 11, y: 10 };
        if let ServerPacket::ObjectMonster { info } = &mut object.packet {
            info.location = object.position.clone();
        }
        zone.object_grid.moved(&9_002, &Point { x: 11, y: 10 });
        let (_, out) = zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 1_000);
        assert_eq!(zone.native_monsters[&9_001].hp, 484);
        assert_eq!(zone.native_monsters[&9_002].hp, 500);
        assert_eq!(practice_count(&out), 0);
        assert!(
            zone.five_class_trap_tile(action.spell_object_ids[0])
                .unwrap()
                .param
        );
        assert!(
            !zone
                .five_class_trap_tile(action.spell_object_ids[1])
                .unwrap()
                .param
        );
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 1_500);
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 2_000);
        assert_eq!(zone.native_monsters[&9_001].hp, 484);
        assert!(zone.objects.contains_key(&action.spell_object_ids[0]));
        // Source removal is strictly after the 1000ms animation deadline.
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 2_001);
        assert!(!zone.objects.contains_key(&action.spell_object_ids[0]));
        assert!(action.trap_detonated);
        assert!(action
            .spell_object_ids
            .iter()
            .skip(1)
            .all(|id| zone.five_class_trap_tile(*id).unwrap().param));
        assert_eq!(zone.native_monsters[&9_002].hp, 500);
    }

    #[test]
    fn five_class_explosive_linked_despawn_restarts_sibling_deadline_without_damage() {
        let (mut zone, owner) = fixture();
        let mut action = queued(&mut zone, &owner, Point { x: 11, y: 10 });
        let mut occupied = BTreeSet::new();
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 500);
        spawn(&mut zone, 9_001, Point { x: 11, y: 10 }, 0);
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 1_000);
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 2_001);
        spawn(&mut zone, 9_002, Point { x: 11, y: 9 }, 0);
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 2_500);
        assert_eq!(zone.native_monsters[&9_002].hp, 500); // linked detonation is visual only.
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 3_002);
        assert!(!zone.objects.contains_key(&action.spell_object_ids[1]));
        assert_eq!(
            zone.objects[&action.spell_object_ids[2]].expires_at_ms,
            Some(4_003)
        );
        let (retain, out) = zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 4_003);
        assert!(!retain && occupied.is_empty());
        assert!(out
            .iter()
            .any(|out| matches!(out, ZoneOutbound::ToMany { packets, .. }
            if packets.iter().any(|packet| matches!(packet, ServerPacket::ObjectRemove { .. })))));
    }

    #[test]
    fn five_class_explosive_player_hits_are_lawful_and_initial_traps_are_private() {
        let (mut zone, owner) = fixture();
        let victim = observer(
            &mut zone,
            "explosive-victim",
            202,
            Point { x: 11, y: 10 },
            5,
        );
        let mut action = queued(&mut zone, &owner, Point { x: 11, y: 10 });
        let mut occupied = BTreeSet::new();
        let (_, out) = zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 500);
        assert_eq!(zone.players[&victim].hp, 484);
        assert_eq!(practice_count(&out), 1);
        let birth = out
            .iter()
            .find_map(|out| match out {
                ZoneOutbound::ToMany {
                    session_ids,
                    packets,
                } if packets.iter().any(
                    |packet| matches!(packet, ServerPacket::ObjectSpell { info } if !info.param),
                ) =>
                {
                    Some(session_ids)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(birth, &vec![owner.clone()]);
        let linked = action.spell_object_ids[1];
        zone.pending_native_ground_spells.push(action);
        let info = zone.five_class_trap_tile(linked).unwrap();
        assert!(!zone.can_see_five_class_trap(&zone.players[&victim], info, 500));
        assert!(zone.can_see_five_class_trap(&zone.players[&owner], info, 500));
        let mut detonation = info.clone();
        detonation.param = true;
        assert!(zone.can_see_five_class_trap(&zone.players[&victim], &detonation, 500));
    }

    #[test]
    fn five_class_explosive_stale_owner_removes_all_tiles_without_ghost_damage_or_xp() {
        let (mut zone, owner) = fixture();
        let mut action = queued(&mut zone, &owner, Point { x: 11, y: 10 });
        let mut occupied = BTreeSet::new();
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 500);
        spawn(&mut zone, 9_001, Point { x: 11, y: 10 }, 0);
        zone.players.get_mut(&owner).unwrap().life_generation += 1;
        let (retain, out) = zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 1_000);
        assert!(!retain && occupied.is_empty());
        assert_eq!(zone.native_monsters[&9_001].hp, 500);
        assert_eq!(practice_count(&out), 0);
        assert!(action
            .spell_object_ids
            .iter()
            .all(|id| !zone.objects.contains_key(id)));
    }

    #[test]
    fn five_class_explosive_blocked_sides_and_current_direction_match_delayed_map_placement() {
        let (mut zone, owner) = fixture();
        let mut action = queued(&mut zone, &owner, Point { x: 11, y: 10 });
        zone.players.get_mut(&owner).unwrap().direction = MirDirection::UpRight;
        zone.collision = ZoneCollision::unbounded().with_blocked_cells([Point { x: 10, y: 9 }]);
        let mut occupied = BTreeSet::new();
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 500);
        assert_eq!(
            action.locations,
            vec![Point { x: 11, y: 10 }, Point { x: 12, y: 11 }]
        );
        assert!(action.spell_object_ids.iter().all(|id| zone
            .five_class_trap_tile(*id)
            .unwrap()
            .direction
            == MirDirection::Up));
    }

    #[test]
    fn five_class_explosive_safe_human_does_not_consume_trap_but_mac_absorption_does() {
        let (mut zone, owner) = fixture();
        let victim = observer(&mut zone, "safe-victim", 202, Point { x: 11, y: 10 }, 5);
        zone.players
            .get_mut(&victim)
            .unwrap()
            .chat_profile
            .in_safe_zone = true;
        let mut action = queued(&mut zone, &owner, Point { x: 11, y: 10 });
        let mut occupied = BTreeSet::new();
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 500);
        assert_eq!(zone.players[&victim].hp, 500);
        assert!(action
            .spell_object_ids
            .iter()
            .all(|id| !zone.five_class_trap_tile(*id).unwrap().param));
        let point = action.locations[1].clone();
        spawn(&mut zone, 9_001, point, 100);
        zone.tick_five_class_explosive_trap(&mut action, &mut occupied, 1_000);
        assert_eq!(zone.native_monsters[&9_001].hp, 500);
        assert!(
            zone.five_class_trap_tile(action.spell_object_ids[1])
                .unwrap()
                .param
        );
        assert_eq!(zone.players[&victim].hp, 500);
    }

    #[test]
    fn five_class_explosive_distinct_level_limit_and_owner_distance_retirement_are_real() {
        let (mut zone, owner) = fixture();
        let mut first = queued(&mut zone, &owner, Point { x: 11, y: 10 });
        let mut second = queued(&mut zone, &owner, Point { x: 12, y: 10 });
        let mut occupied = BTreeSet::new();
        zone.tick_five_class_explosive_trap(&mut first, &mut occupied, 500);
        zone.tick_five_class_explosive_trap(&mut second, &mut occupied, 500);
        zone.pending_native_ground_spells = vec![first.clone(), second.clone()];
        assert!(!zone.can_queue_explosive_trap(&owner, OWNER, 1, 500));
        assert!(zone.can_queue_explosive_trap(&owner, OWNER, 2, 500));
        // Source checks range even when the next 500ms damage clock is not due.
        zone.players.get_mut(&owner).unwrap().position = Point { x: 30, y: 30 };
        let (retained, out) = zone.tick_five_class_explosive_trap(&mut first, &mut occupied, 501);
        assert!(!retained);
        assert_eq!(practice_count(&out), 0);
        assert!(first
            .spell_object_ids
            .iter()
            .all(|id| !zone.objects.contains_key(id)));
        assert!(second
            .spell_object_ids
            .iter()
            .all(|id| zone.objects.contains_key(id)));
    }
}
