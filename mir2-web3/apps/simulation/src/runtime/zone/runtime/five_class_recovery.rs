//! Cold deserialization revokes online skill leases. Retained live map
//! transfer uses FiveClassSkillState and never calls this cleanup.
use super::*;

pub(super) fn clear_cold_player_state(player: &mut ZonePlayer, protected_bleeding: bool) {
    let hidden_lease_lost = player.buffs.contains_key(&13) || player.buffs.contains_key(&14);
    for buff in [13, 14, 15, 25] {
        player.buffs.remove(&buff);
    }
    player.fatal_sword_armed = false;
    player.element_orb_effect = 0;
    player.element_orb_level = 0;
    player.element_orb_max = 0;
    if hidden_lease_lost {
        player.hidden = player.buffs.contains_key(&2) || player.buffs.contains_key(&114);
    }
    let mask = 64 | if protected_bleeding { 0 } else { 128 };
    player.poison &= !mask;
    player.native_status_poison &= !mask;
    player
        .native_status_poison_deadlines
        .retain(|bit, _| bit & mask == 0);
    if !player.native_status_poison_deadlines.is_empty() {
        player.native_status_poison_expires_at_ms = player
            .native_status_poison_deadlines
            .values()
            .copied()
            .max();
    } else if player.native_status_poison == 0 {
        player.native_status_poison_expires_at_ms = None;
    }
}

impl ZoneRuntime {
    /// Called only after the ORIGINAL checkpoint commitment has been verified.
    pub(super) fn clear_cold_five_class_state(&mut self) {
        let mut protected_bleeding = self.cold_entity_poison_lease_targets(128);
        protected_bleeding.extend(
            self.native_periodic_player_poisons
                .iter()
                .filter(|lease| {
                    lease.mask == 128
                        && self
                            .players
                            .get(&lease.session)
                            .is_some_and(|p| p.object_id == lease.target)
                })
                .map(|lease| lease.target),
        );
        for player in self.players.values_mut() {
            clear_cold_player_state(player, protected_bleeding.contains(&player.object_id));
        }
        self.clear_cold_detached_five_class_state(&protected_bleeding);
        for (&id, monster) in &mut self.native_monsters {
            let mask = 64
                | if protected_bleeding.contains(&id) {
                    0
                } else {
                    128
                };
            monster.entity_poison &= !mask;
            if let Some(object) = self.objects.get_mut(&id) {
                if let ServerPacket::ObjectMonster { info } = &mut object.packet {
                    info.poison = native_monster_current_poison(monster);
                }
            }
        }
        // Source temporary AssassinClone is not a SavedPetInfo. With its
        // ten-second actor lease gone, retire it instead of leaving an idle
        // occupancy blocker. Other AI59 monsters and all ordinary pets survive.
        let clones: Vec<_> = self
            .native_monsters
            .iter()
            .filter(|(_, m)| {
                m.ai == 59 && m.name == "AssassinClone" && m.owner_session_id.is_some()
            })
            .map(|(&id, _)| id)
            .collect();
        for id in clones {
            self.remove_retained_zone_object(id);
        }
        // Only the trusted new Assassin lease is revoked. Original ElectricShock,
        // TrapHexagon, other monster controls, and newer replacement clocks survive.
        let mut owned_trap_leases = Vec::new();
        for (&id, monster) in &mut self.native_monsters {
            if let Some(deadline) = monster.assassin_trap_control_until_ms.take() {
                owned_trap_leases.push((id, deadline));
                if monster.control_until_ms == deadline {
                    monster.control_until_ms = 0;
                }
                if monster.next_ai_ready_at_ms == deadline {
                    monster.next_ai_ready_at_ms = 0;
                }
                if monster.next_attack_ready_at_ms == deadline {
                    monster.next_attack_ready_at_ms = 0;
                }
            }
        }
        for (id, deadline) in owned_trap_leases {
            self.clear_cold_five_class_owned_trap_shock(id, deadline);
        }
        let traps: Vec<_> = self.objects.iter().filter_map(|(&id, object)| {
            matches!(object.packet, ServerPacket::ObjectSpell { ref info } if info.spell == Spell::Trap)
                .then_some(id)
        }).collect();
        for id in traps {
            self.remove_retained_zone_object(id);
        }
        self.five_class_skills = Default::default();
        self.five_class_pvp = Default::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::zone::types::ZonePlayerBuff;

    fn fixture() -> (ZoneRuntime, SessionId) {
        let session = SessionId::new("cold-five-class");
        let mut zone = ZoneRuntime::new(ZoneKey::for_map("cold-five-class-fixture"));
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session.clone(),
            account_id: "cold-five-class".into(),
            character_index: 1,
            object_id: 101,
            name: "ColdAssassin".into(),
            class: MirClass::Assassin,
            gender: mir2_protocol::MirGender::Male,
            level: 50,
            hp: 100,
            max_hp: 100,
            mp: 100,
            map_file_name: "cold-five-class-fixture".into(),
            position: Point { x: 10, y: 10 },
            direction: MirDirection::Right,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        (zone, session)
    }
    fn buff(player: &mut ZonePlayer, kind: u8) {
        player.buffs.insert(
            kind,
            ZonePlayerBuff {
                buff: ClientBuff {
                    buff_type: kind,
                    visible: true,
                    object_id: player.object_id,
                    expire_time: 99_999_999,
                    values: Vec::new(),
                    infinite: false,
                    paused: false,
                    stats: Vec::new(),
                },
                expires_at_ms: Some(99_999_999),
                notify_owner_on_expiry: true,
            },
        );
    }

    #[test]
    fn five_class_cold_checkpoint_revokes_orphan_leases_after_integrity_check() {
        let (mut zone, session) = fixture();
        let player = zone.players.get_mut(&session).unwrap();
        for kind in [13, 14, 15, 25, 8] {
            buff(player, kind);
        }
        player.hidden = true;
        player.fatal_sword_armed = true;
        player.poison = 64 | 128 | 32;
        player.native_status_poison = player.poison;
        player.native_status_poison_deadlines =
            BTreeMap::from([(64, u64::MAX), (128, u64::MAX), (32, 90_000)]);
        player.native_status_poison_expires_at_ms = Some(u64::MAX);
        let bytes = zone.checkpoint_bytes().unwrap();
        let cold = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
        let player = &cold.players[&session];
        assert!(!player.hidden && !player.fatal_sword_armed);
        assert_eq!(player.poison, 32);
        assert_eq!(player.native_status_poison, 32);
        assert_eq!(player.native_status_poison_expires_at_ms, Some(90_000));
        assert_eq!(player.buffs.keys().copied().collect::<Vec<_>>(), vec![8]);
        let mut changed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        changed["players"][session.as_str()]["fatal_sword_armed"] = serde_json::json!(false);
        assert!(ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&changed).unwrap()).is_err());
    }

    #[test]
    fn five_class_cold_cleanup_preserves_original_hiding_and_real_monster_bleeding() {
        let (mut zone, session) = fixture();
        let player = zone.players.get_mut(&session).unwrap();
        for kind in [2, 13, 114] {
            buff(player, kind);
        }
        player.hidden = true;
        zone.apply_native_player_periodic_poison(9001, &session, 101, 128, 5, 10, 1000, 1);
        let cold = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        let player = &cold.players[&session];
        assert!(player.hidden);
        assert!(player.buffs.contains_key(&2));
        assert!(player.buffs.contains_key(&114));
        assert!(!player.buffs.contains_key(&13));
        assert_ne!(player.poison & 128, 0);
        assert_eq!(cold.native_periodic_player_poisons.len(), 1);
    }

    #[test]
    fn five_class_cold_trap_revokes_real_cast_only_lease_and_node_after_integrity_check() {
        let (mut zone, session) = fixture();
        zone.handle(ZoneCommand::SpawnMonster {
            session_id: session.clone(),
            now_ms: 0,
            monster: ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: 9001,
                name: "Scarecrow".into(),
                name_colour_argb: -1,
                image: 5,
                ai: 0,
                disposition: Some(crate::config::WorldEntityDisposition::Hostile),
                level: 1,
                max_hp: 100,
                hp: 100,
                experience: 1,
                move_speed_ms: 60_000,
                attack_speed_ms: 60_000,
                friendly_guild: None,
                position: Point { x: 11, y: 10 },
                direction: MirDirection::Left,
                defense: crate::runtime::zone::types::ZoneMonsterDefense::default(),
                respawn: None,
                drops: vec![],
            },
        });
        // A real learned skill is supplied by the authenticated personal session.
        zone.players
            .get_mut(&session)
            .unwrap()
            .combat_stats
            .five_class_learned_mask =
            super::super::super::five_class_receipts::ZoneFiveClassPracticeSpell::Trap
                .learned_bit();
        let magic = crystal_magic_by_spell("Trap").unwrap();
        let expected_mp = zone.players[&session].mp
            - i32::from(magic.base_cost)
            - 3 * i32::from(magic.level_cost);
        let launch = zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: session.clone(),
            object_id: 9001,
            spell: Spell::Trap,
            direction: MirDirection::Right,
            target: Point { x: 11, y: 10 },
            cast: true,
            level: 3,
            damage: 99_999,
            mp_cost: i32::from(magic.base_cost) + 3 * i32::from(magic.level_cost),
            cooldown_ms: u64::from(magic.delay_base)
                .saturating_sub(3 * u64::from(magic.delay_reduction)),
            now_ms: 100,
        });
        assert!(launch
            .iter()
            .flat_map(|o| match o {
                ZoneOutbound::ToSession { packets, .. }
                | ZoneOutbound::ToMany { packets, .. }
                | ZoneOutbound::ToAll { packets } => packets.as_slice(),
                _ => &[],
            })
            .any(|p| matches!(
                p,
                ServerPacket::Magic {
                    spell: Spell::Trap,
                    cast: true,
                    ..
                }
            )));
        assert_eq!(launch.iter().filter(|o| matches!(o,
            ZoneOutbound::FiveClassMagicPractice { receipt } if receipt.spell.spell() == Spell::Trap)).count(), 1);
        assert_eq!(zone.players[&session].mp, expected_mp);
        let born = zone.tick(1000);
        assert_eq!(born.iter().filter(|o| matches!(o,
            ZoneOutbound::FiveClassMagicPractice { receipt } if receipt.spell.spell() == Spell::Trap)).count(), 1);
        let monster = &zone.native_monsters[&9001];
        let deadline = monster
            .assassin_trap_control_until_ms
            .expect("real server-authored Trap marker");
        assert_eq!(monster.control_until_ms, deadline);
        assert_eq!(deadline, 61_000);
        let trap_id = zone.objects.iter().find_map(|(&id, object)| {
            matches!(object.packet, ServerPacket::ObjectSpell { ref info } if info.spell == Spell::Trap)
                .then_some(id)
        }).expect("real retained Trap Node");
        let bytes = zone.checkpoint_bytes().unwrap();
        let cold = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
        assert_eq!(
            cold.native_monsters[&9001].assassin_trap_control_until_ms,
            None
        );
        assert_eq!(cold.native_monsters[&9001].control_until_ms, 0);
        assert!(!cold.objects.contains_key(&trap_id));
        assert!(!cold
            .object_grid
            .neighbors(&Point { x: 11, y: 10 })
            .contains(&trap_id));
    }
}
