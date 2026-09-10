//! Shared player status clocks. Crystal HumanObject.ApplyPoison keeps control
//! poisons independent and does not refresh an already active control poison.
use super::*;

fn import_legacy_status_deadlines(player: &mut ZonePlayer) {
    if let Some(deadline) = player.native_status_poison_expires_at_ms {
        for shift in 0..16 {
            let bit = 1_u16 << shift;
            if player.native_status_poison & bit != 0 {
                player
                    .native_status_poison_deadlines
                    .entry(bit)
                    .or_insert(deadline);
            }
        }
    }
}

pub(super) fn zone_player_active_status(player: &ZonePlayer, now_ms: u64) -> u16 {
    player.active_status_poison(now_ms)
}

fn expire_status(player: &mut ZonePlayer, now_ms: u64) {
    import_legacy_status_deadlines(player);
    let old = player.native_status_poison;
    player
        .native_status_poison_deadlines
        .retain(|_, expiry| now_ms < *expiry);
    player.native_status_poison = player
        .native_status_poison_deadlines
        .keys()
        .fold(0, |m, b| m | *b);
    player.poison = (player.poison & !old) | player.native_status_poison;
    player.native_status_poison_expires_at_ms = player
        .native_status_poison_deadlines
        .values()
        .copied()
        .max();
}

pub(super) fn zone_add_player_status_poison(
    player: &mut ZonePlayer,
    poison: u16,
    duration_ms: u64,
    now_ms: u64,
) {
    expire_status(player, now_ms);
    if player.dead || player.hp <= 0 || duration_ms == 0 {
        return;
    }
    let deadline = now_ms.saturating_add(duration_ms);
    for shift in 0..16 {
        let bit = 1_u16 << shift;
        if poison & bit == 0 {
            continue;
        }
        // Frozen, Slow, Paralysis and LRParalysis cannot be refreshed.
        let no_refresh = bit
            & (CRYSTAL_POISON_FROZEN
                | CRYSTAL_POISON_SLOW
                | CRYSTAL_POISON_PARALYSIS
                | CRYSTAL_POISON_LR_PARALYSIS)
            != 0;
        let entry = player
            .native_status_poison_deadlines
            .entry(bit)
            .or_insert(deadline);
        if !no_refresh {
            *entry = (*entry).max(deadline);
        }
    }
    player.native_status_poison = player
        .native_status_poison_deadlines
        .keys()
        .fold(0, |m, b| m | *b);
    player.poison |= player.native_status_poison;
    player.native_status_poison_expires_at_ms = player
        .native_status_poison_deadlines
        .values()
        .copied()
        .max();
}

impl ZoneRuntime {
    pub(super) fn apply_native_player_status_poison(
        &mut self,
        session_id: &SessionId,
        poison: u16,
        duration_ms: u64,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(player) = self
            .players
            .get_mut(session_id)
            .filter(|p| !p.dead && p.hp > 0)
        else {
            return Vec::new();
        };
        let old = player.poison;
        zone_add_player_status_poison(player, poison, duration_ms, now_ms);
        if old == player.poison {
            return Vec::new();
        }
        let (object_id, position, poison) =
            (player.object_id, player.position.clone(), player.poison);
        vec![ZoneOutbound::ToMany {
            session_ids: self.player_status_recipients(object_id, &position),
            packets: vec![ServerPacket::ObjectPoisoned { object_id, poison }],
        }]
    }

    pub(super) fn expire_zone_player_status_poisons(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let mut changed = Vec::new();
        for player in self.players.values_mut() {
            let old = player.poison;
            expire_status(player, now_ms);
            if old != player.poison {
                changed.push((player.object_id, player.position.clone(), player.poison));
            }
        }
        changed
            .into_iter()
            .map(|(object_id, position, poison)| ZoneOutbound::ToMany {
                session_ids: self.player_status_recipients(object_id, &position),
                packets: vec![ServerPacket::ObjectPoisoned { object_id, poison }],
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_protocol::MirGender;
    fn fixture() -> (ZoneRuntime, SessionId) {
        let id = SessionId::new("status-player");
        let mut zone = ZoneRuntime::new(ZoneKey::for_map("status-clock-fixture"));
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: id.clone(),
            account_id: "status-account".into(),
            character_index: 0,
            object_id: 7,
            name: "Status".into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 40,
            hp: 100,
            max_hp: 100,
            mp: 100,
            map_file_name: "status-clock-fixture".into(),
            position: Point { x: 10, y: 10 },
            direction: MirDirection::Down,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        (zone, id)
    }
    #[test]
    fn independent_control_clocks_survive_checkpoint_and_do_not_extend_each_other() {
        let (mut zone, id) = fixture();
        zone.apply_native_player_status_poison(&id, CRYSTAL_POISON_SLOW, 15_000, 100);
        zone.apply_native_player_status_poison(&id, CRYSTAL_POISON_PARALYSIS, 5_000, 100);
        assert!(zone_player_status_blocks_movement(
            &zone.players[&id],
            5_099
        ));
        assert!(!zone_player_status_blocks_movement(
            &zone.players[&id],
            5_100
        ));
        let bytes = zone.checkpoint_bytes().unwrap();
        let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
        assert_eq!(restored.checkpoint_bytes().unwrap(), bytes);
        assert_eq!(restored.expire_zone_player_status_poisons(5_100).len(), 1);
        assert_eq!(restored.players[&id].poison, CRYSTAL_POISON_SLOW);
        assert!(zone_player_slowed(&restored.players[&id], 15_099));
        restored.expire_zone_player_status_poisons(15_100);
        assert_eq!(restored.players[&id].poison, 0);
        assert!(restored.players[&id]
            .native_status_poison_deadlines
            .is_empty());
    }
    #[test]
    fn active_control_cannot_be_refreshed_but_can_be_reapplied_after_expiry() {
        let (mut zone, id) = fixture();
        zone.apply_native_player_status_poison(&id, CRYSTAL_POISON_PARALYSIS, 3_000, 100);
        zone.apply_native_player_status_poison(&id, CRYSTAL_POISON_PARALYSIS, 60_000, 1_000);
        assert!(!zone_player_status_blocks_movement(
            &zone.players[&id],
            3_100
        ));
        zone.apply_native_player_status_poison(&id, CRYSTAL_POISON_PARALYSIS, 3_000, 3_100);
        assert!(zone_player_status_blocks_movement(
            &zone.players[&id],
            6_099
        ));
        assert!(!zone_player_status_blocks_movement(
            &zone.players[&id],
            6_100
        ));
    }
    #[test]
    fn legacy_single_deadline_import_preserves_existing_duration_and_other_poison_bits() {
        let (mut zone, id) = fixture();
        let p = zone.players.get_mut(&id).unwrap();
        p.native_status_poison = CRYSTAL_POISON_SLOW;
        p.native_status_poison_expires_at_ms = Some(10_000);
        p.poison = CRYSTAL_POISON_SLOW | CRYSTAL_POISON_RED;
        let old = zone.checkpoint_bytes().unwrap();
        assert!(!String::from_utf8(old.clone())
            .unwrap()
            .contains("native_status_poison_deadlines"));
        let mut zone = ZoneRuntime::restore_checkpoint(&old).unwrap();
        zone.apply_native_player_status_poison(&id, CRYSTAL_POISON_PARALYSIS, 1_000, 500);
        zone.expire_zone_player_status_poisons(1_500);
        assert_eq!(
            zone.players[&id].poison,
            CRYSTAL_POISON_SLOW | CRYSTAL_POISON_RED
        );
        zone.expire_zone_player_status_poisons(10_000);
        assert_eq!(zone.players[&id].poison, CRYSTAL_POISON_RED);
    }
    #[test]
    fn new_player_life_clears_all_status_clocks_and_aggregate_visual_bits() {
        let (mut zone, id) = fixture();
        zone.apply_native_player_status_poison(&id, CRYSTAL_POISON_PARALYSIS, 60_000, 100);
        zone.handle(ZoneCommand::SyncPlayerVitals {
            session_id: id.clone(),
            hp: 0,
            max_hp: 100,
            mp: 10,
        });
        zone.handle(ZoneCommand::SyncPlayerVitals {
            session_id: id.clone(),
            hp: 100,
            max_hp: 100,
            mp: 10,
        });
        assert_eq!(zone.players[&id].poison, 0);
        assert_eq!(zone.players[&id].native_status_poison, 0);
        assert!(zone.players[&id].native_status_poison_deadlines.is_empty());
        assert!(!zone_player_status_blocks_movement(&zone.players[&id], 200));
        let restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        assert_eq!(restored.players[&id].poison, 0);
        zone.apply_native_player_status_poison(&id, CRYSTAL_POISON_SLOW, 60_000, 300);
        zone.handle(ZoneCommand::BroadcastPackets {
            session_id: id.clone(),
            owner_local_object_id: 7,
            now_ms: 400,
            packets: vec![ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: 7,
                    location: Point { x: 10, y: 10 },
                    direction: MirDirection::Down,
                    kind: 0,
                },
            }],
        });
        assert!(zone.players[&id].native_status_poison_deadlines.is_empty());
        assert_eq!(zone.players[&id].poison, 0);
    }

    #[test]
    fn personal_clear_packet_cannot_flicker_active_shared_status_for_observer() {
        let (mut zone, id) = fixture();
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new("observer"),
            account_id: "observer".into(),
            character_index: 0,
            object_id: 8,
            name: "Observer".into(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 40,
            hp: 100,
            max_hp: 100,
            mp: 100,
            map_file_name: "status-clock-fixture".into(),
            position: Point { x: 11, y: 10 },
            direction: MirDirection::Down,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        zone.apply_native_player_status_poison(&id, CRYSTAL_POISON_SLOW, 5_000, 100);
        let out = zone.handle(ZoneCommand::BroadcastPackets {
            session_id: id.clone(),
            owner_local_object_id: 7,
            now_ms: 200,
            packets: vec![ServerPacket::ObjectPoisoned {
                object_id: 7,
                poison: 0,
            }],
        });
        assert_eq!(zone.players[&id].poison, CRYSTAL_POISON_SLOW);
        let packets = out
            .iter()
            .flat_map(|o| match o {
                ZoneOutbound::ToMany { packets, .. }
                | ZoneOutbound::ToSession { packets, .. }
                | ZoneOutbound::ToAll { packets } => packets.as_slice(),
                _ => &[],
            })
            .collect::<Vec<_>>();
        assert!(packets.iter().any(|p| matches!(
            p,
            ServerPacket::ObjectPoisoned {
                object_id: 7,
                poison: 4
            }
        )));
        assert!(!packets.iter().any(|p| matches!(
            p,
            ServerPacket::ObjectPoisoned {
                object_id: 7,
                poison: 0
            }
        )));
        assert!(zone.expire_zone_player_status_poisons(201).is_empty());
        zone.expire_zone_player_status_poisons(5_100);
        assert_eq!(zone.players[&id].poison, 0);
        zone.spawn_world_event_monster(
            &ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: 77,
                name: "Scarecrow".into(),
                name_colour_argb: -1,
                image: 5,
                ai: 0,
                disposition: Some(crate::config::WorldEntityDisposition::Hostile),
                level: 1,
                hp: 100,
                max_hp: 100,
                experience: 0,
                move_speed_ms: 60_000,
                attack_speed_ms: 60_000,
                friendly_guild: None,
                position: Point { x: 12, y: 10 },
                direction: MirDirection::Left,
                defense: Default::default(),
                respawn: None,
                drops: Vec::new(),
            },
            5_200,
        );
        zone.apply_native_player_status_poison(&id, CRYSTAL_POISON_SLOW, 5_000, 5_200);
        let shared_out = zone.handle(ZoneCommand::BroadcastSharedObjectPackets {
            session_id: id.clone(),
            local_self_object_id: Some(7),
            now_ms: 5_300,
            packets: vec![
                ServerPacket::ObjectStruck {
                    info: ObjectStruckInfo {
                        object_id: 7,
                        attacker_id: 77,
                        location: Point { x: 10, y: 10 },
                        direction: MirDirection::Down,
                    },
                },
                ServerPacket::ObjectPoisoned {
                    object_id: 7,
                    poison: CRYSTAL_POISON_RED,
                },
            ],
        });
        assert_eq!(
            zone.players[&id].poison,
            CRYSTAL_POISON_SLOW | CRYSTAL_POISON_RED
        );
        assert!(shared_out.iter().any(|out| match out {
            ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.iter().any(|p| matches!(
                p,
                ServerPacket::ObjectPoisoned {
                    object_id: 7,
                    poison: 6
                }
            )),
            _ => false,
        }));
    }
}
