//! Crystal MonsterObject.ProcessRoam for unowned neutral livestock (AI1/2).
//! Other species retain their specialized AI; this is not a universal mover.
use super::*;
use crate::config::WorldEntityDisposition;

const ROAM_DELAY_MS: u64 = 1_000;
const ALONE_DELAY_MS: u64 = 3_000;
// Crystal Shared/Globals.cs: DataRange = 16; CheckAlone uses twice that range.
const ALONE_RANGE: i32 = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IdleRoamState {
    next_roam_ms: u64,
    next_alone_check_ms: u64,
    move_ready_ms: u64,
    alone: bool,
    rng_state: u64,
}

impl IdleRoamState {
    fn new(object_id: u32, incarnation: u64, now_ms: u64) -> Self {
        let mut state = Self {
            next_roam_ms: 0,
            next_alone_check_ms: 0,
            move_ready_ms: 0,
            alone: false,
            // A separate replayable stream must not consume combat/loot draws.
            rng_state: (u64::from(object_id) << 32)
                ^ incarnation.rotate_left(17)
                ^ now_ms
                ^ 0x524F_414D,
        };
        state.next_roam_ms = now_ms.saturating_add(state.roll(ROAM_DELAY_MS));
        state
    }

    fn roll(&mut self, upper: u64) -> u64 {
        loop {
            self.rng_state = self.rng_state.wrapping_add(0x9E3779B97F4A7C15);
            let mut value = self.rng_state;
            value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
            value ^= value >> 31;
            if value >= upper.wrapping_neg() % upper {
                return value % upper;
            }
        }
    }
}

pub(super) fn supports_idle_roam(monster: &ZoneNativeMonster) -> bool {
    matches!(monster.ai, 1 | 2)
        && monster.disposition == Some(WorldEntityDisposition::Neutral)
        && !monster.hostile_to_player
        && monster.owner_session_id.is_none()
        && monster.master_object_id == 0
        && monster.owner_player_object_id == 0
}

pub(super) fn idle_roam_move_ready(monster: &ZoneNativeMonster, now_ms: u64) -> bool {
    monster
        .special_ai
        .as_ref()
        .and_then(|state| state.idle_roam.as_ref())
        .is_none_or(|state| now_ms > state.move_ready_ms)
}

impl ZoneRuntime {
    /// Called only after target selection has established that there is no
    /// target. World state, retained ObjectMonster and AOI all share this writer.
    pub(super) fn tick_livestock_idle_roam(
        &mut self,
        object_id: u32,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(monster) = self.native_monsters.get(&object_id) else {
            return Vec::new();
        };
        if !supports_idle_roam(monster) || monster.dead || monster.hp <= 0 {
            return Vec::new();
        }
        if monster
            .special_ai
            .as_ref()
            .and_then(|s| s.idle_roam.as_ref())
            .is_some_and(|state| now_ms < state.next_roam_ms && now_ms < state.next_alone_check_ms)
        {
            return Vec::new();
        }
        let position = monster.position.clone();
        let direction = monster.direction;
        let move_speed_ms = monster.move_speed_ms.max(300);
        let mut state = monster
            .special_ai
            .as_ref()
            .and_then(|special| special.idle_roam.clone())
            .unwrap_or_else(|| IdleRoamState::new(object_id, monster.incarnation, now_ms));
        if now_ms >= state.next_alone_check_ms {
            state.alone = !self
                .players
                .values()
                .any(|player| zone_tile_distance(&position, &player.position) <= ALONE_RANGE);
            state.next_alone_check_ms = now_ms.saturating_add(ALONE_DELAY_MS);
        }
        let can_move = now_ms > state.move_ready_ms
            && now_ms >= monster.next_ai_ready_at_ms
            && !native_monster_control_active(monster, now_ms)
            && monster.entity_poison
                & (CRYSTAL_POISON_PARALYSIS | CRYSTAL_POISON_LR_PARALYSIS | CRYSTAL_POISON_FROZEN)
                == 0
            && (monster.entity_poison & CRYSTAL_POISON_STUN == 0
                || crystal_monster_by_name(&monster.name)
                    .is_some_and(|template| matches!(template.light, 5 | 10)))
            && monster_visibility_can_act(monster, now_ms)
            && monster_visibility_can_move(monster);
        let mut packet = None;
        // Do not catch up missed rolls after a long tick gap or empty map.
        if !state.alone && now_ms >= state.next_roam_ms && state.next_roam_ms != u64::MAX {
            state.next_roam_ms = now_ms.saturating_add(ROAM_DELAY_MS);
            if state.roll(10) == 0 {
                if state.roll(3) == 0 {
                    let directions = [
                        MirDirection::Up,
                        MirDirection::UpRight,
                        MirDirection::Right,
                        MirDirection::DownRight,
                        MirDirection::Down,
                        MirDirection::DownLeft,
                        MirDirection::Left,
                        MirDirection::UpLeft,
                    ];
                    let turned = directions[state.roll(8) as usize];
                    if can_move {
                        packet = Some(ServerPacket::ObjectTurn {
                            movement: ObjectMovement {
                                object_id,
                                position: position.clone(),
                                direction: turned,
                            },
                        });
                    }
                } else if can_move {
                    let destination = offset_point(&position, direction, 1);
                    if self.can_native_monster_occupy(object_id, &destination) {
                        state.move_ready_ms = now_ms.saturating_add(move_speed_ms);
                        packet = Some(ServerPacket::ObjectWalk {
                            movement: ObjectMovement {
                                object_id,
                                position: destination,
                                direction,
                            },
                        });
                    }
                }
            }
        }
        let monster = self.native_monsters.get_mut(&object_id).unwrap();
        // RoamTime advances independently of MoveTime. Do not schedule another
        // think delay after an idle check: an incoming hit may need to wake a
        // passive animal immediately, using the existing attack/move gates.
        monster
            .special_ai
            .get_or_insert_with(Default::default)
            .idle_roam = Some(state);
        let Some(packet) = packet else {
            return Vec::new();
        };
        let movement = match &packet {
            ServerPacket::ObjectWalk { movement } | ServerPacket::ObjectTurn { movement } => {
                movement
            }
            _ => unreachable!("roaming produces movement only"),
        };
        monster.position = movement.position.clone();
        monster.direction = movement.direction;
        if matches!(packet, ServerPacket::ObjectWalk { .. }) {
            monster.next_attack_ready_at_ms = monster
                .next_attack_ready_at_ms
                .max(now_ms.saturating_add(move_speed_ms));
        }
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        let mut out = self.diff_all_zone_object_visibility();
        let recipients = self.native_monster_visible_recipients(object_id, &movement.position);
        if !recipients.is_empty() {
            out.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![packet],
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (ZoneRuntime, u32) {
        let id = 9_001;
        let mut zone = ZoneRuntime::new_with_collision(
            ZoneKey::for_map("roam-control-fixture"),
            ZoneCollision::unbounded(),
        );
        // Real neutral animal constructed by the server spawn producer.
        let spawn = ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: id,
            name: "Hen".into(),
            name_colour_argb: -1,
            image: 1,
            ai: 1,
            disposition: Some(WorldEntityDisposition::Neutral),
            level: 1,
            max_hp: 100,
            hp: 100,
            experience: 0,
            move_speed_ms: 1_400,
            attack_speed_ms: 1_400,
            friendly_guild: None,
            defense: Default::default(),
            position: Point { x: 10, y: 10 },
            direction: MirDirection::Down,
            respawn: None,
            drops: Vec::new(),
        };
        assert!(zone.spawn_world_event_monster(&spawn, 0).0);
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: SessionId::new("roam-control-observer"),
            account_id: "roam-control-account".into(),
            character_index: 1,
            object_id: 101,
            name: "Observer".into(),
            class: MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 1,
            hp: 100,
            max_hp: 100,
            mp: 100,
            map_file_name: "roam-control-fixture".into(),
            position: Point { x: 13, y: 13 },
            direction: MirDirection::Up,
            chat_profile: Default::default(),
            combat_stats: Default::default(),
        }));
        (zone, id)
    }

    #[test]
    fn all_canmove_poison_masks_and_finite_control_block_roam_then_release() {
        for mask in [
            CRYSTAL_POISON_PARALYSIS,
            CRYSTAL_POISON_LR_PARALYSIS,
            CRYSTAL_POISON_FROZEN,
            CRYSTAL_POISON_STUN,
        ] {
            let (mut zone, id) = fixture();
            zone.native_monsters.get_mut(&id).unwrap().entity_poison = mask;
            for now in (1..=65_001).step_by(100) {
                assert!(zone.tick_livestock_idle_roam(id, now).is_empty());
                assert_eq!(zone.native_monsters[&id].position, Point { x: 10, y: 10 });
                assert_eq!(zone.native_monsters[&id].direction, MirDirection::Down);
            }
            zone.native_monsters.get_mut(&id).unwrap().entity_poison = 0;
            let mut moved = false;
            for now in (65_101..=130_001).step_by(100) {
                zone.tick_livestock_idle_roam(id, now);
                moved |= zone.native_monsters[&id].position != Point { x: 10, y: 10 };
            }
            assert!(
                moved,
                "released poison {mask} must not permanently freeze AI"
            );
        }
        let (mut zone, id) = fixture();
        let monster = zone.native_monsters.get_mut(&id).unwrap();
        monster.control_poison = CRYSTAL_POISON_FROZEN;
        monster.control_until_ms = 65_100;
        for now in (1..=65_001).step_by(100) {
            assert!(zone.tick_livestock_idle_roam(id, now).is_empty());
        }
        let mut moved = false;
        for now in (65_101..=130_001).step_by(100) {
            zone.tick_livestock_idle_roam(id, now);
            moved |= zone.native_monsters[&id].position != Point { x: 10, y: 10 };
        }
        assert!(moved);
    }

    #[test]
    fn idle_roam_never_claims_dead_owned_friendly_or_static_objects() {
        for variant in 0..7 {
            let (mut zone, id) = fixture();
            let m = zone.native_monsters.get_mut(&id).unwrap();
            match variant {
                0 => m.dead = true,
                1 => m.owner_session_id = Some(SessionId::new("roam-control-observer")),
                2 => m.master_object_id = 101,
                3 => m.owner_player_object_id = 101,
                4 => m.disposition = Some(WorldEntityDisposition::Friendly),
                5 => m.ai = 6,
                _ => m.ai = 40,
            }
            let before = zone.checkpoint_bytes().unwrap();
            for now in (1..=65_001).step_by(100) {
                assert!(zone.tick_livestock_idle_roam(id, now).is_empty());
            }
            assert_eq!(zone.checkpoint_bytes().unwrap(), before);
        }
    }

    #[test]
    fn other_living_monster_occupancy_blocks_forward_walks() {
        let (mut zone, id) = fixture();
        let animal = zone.native_monsters[&id].clone();
        for x in 9..=11 {
            for y in 9..=11 {
                if (x, y) == (10, 10) {
                    continue;
                }
                let mut blocker = animal.clone();
                blocker.position = Point { x, y };
                let blocker_id = 10_000 + (x * 3 + y) as u32;
                zone.native_monsters.insert(blocker_id, blocker);
            }
        }
        for now in (1..=65_001).step_by(100) {
            zone.tick_livestock_idle_roam(id, now);
            assert_eq!(zone.native_monsters[&id].position, Point { x: 10, y: 10 });
        }
    }

    #[test]
    fn roam_roll_clock_keeps_ticking_during_a_long_walk() {
        let (mut zone, id) = fixture();
        let mut first_walk = None;
        for now in (1..=65_001).step_by(100) {
            let before = zone.native_monsters[&id].position.clone();
            zone.tick(now);
            if zone.native_monsters[&id].position != before {
                first_walk = Some(now);
                break;
            }
        }
        let walked_at = first_walk.expect("normal seed must naturally walk");
        let at = zone.native_monsters[&id].position.clone();
        let after = walked_at + ROAM_DELAY_MS;
        assert!(!idle_roam_move_ready(&zone.native_monsters[&id], after));
        zone.tick(after);
        let roam = zone.native_monsters[&id]
            .special_ai
            .as_ref()
            .unwrap()
            .idle_roam
            .as_ref()
            .unwrap();
        assert_eq!(roam.next_roam_ms, after + ROAM_DELAY_MS);
        assert_eq!(zone.native_monsters[&id].position, at);
        assert!(!idle_roam_move_ready(
            &zone.native_monsters[&id],
            walked_at + 1_400
        ));
        assert!(idle_roam_move_ready(
            &zone.native_monsters[&id],
            walked_at + 1_401
        ));
    }

    #[test]
    fn new_attacker_does_not_shorten_the_current_roam_walk() {
        let (mut zone, id) = fixture();
        let m = zone.native_monsters.get_mut(&id).unwrap();
        m.ai = 2;
        m.name = "Deer".into();
        initialize_passive_monster(m, id, 0);
        zone.players
            .values_mut()
            .next()
            .unwrap()
            .chat_profile
            .in_safe_zone = true;
        let mut first_walk = None;
        for now in (1..=65_001).step_by(100) {
            let before = zone.native_monsters[&id].position.clone();
            zone.tick(now);
            if zone.native_monsters[&id].position != before {
                first_walk = Some(now);
                break;
            }
        }
        let walked_at = first_walk.expect("normal deer seed must naturally roam");
        let at = zone.native_monsters[&id].position.clone();
        let ready = walked_at + zone.native_monsters[&id].move_speed_ms;
        zone.players
            .values_mut()
            .next()
            .unwrap()
            .chat_profile
            .in_safe_zone = false;
        passive_monster_on_damage(
            zone.native_monsters.get_mut(&id).unwrap(),
            id,
            Some(101),
            walked_at + 1,
        );
        assert!(zone
            .try_tick_passive_monster(id, walked_at + 1)
            .unwrap()
            .is_empty());
        assert_eq!(zone.native_monsters[&id].position, at);
        assert!(zone.try_tick_passive_monster(id, ready).unwrap().is_empty());
        assert_eq!(zone.native_monsters[&id].position, at);
        zone.try_tick_passive_monster(id, ready + 1).unwrap();
        assert_ne!(zone.native_monsters[&id].position, at);
    }
}
