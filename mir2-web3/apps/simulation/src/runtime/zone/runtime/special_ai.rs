//! Special monster state is advanced once by the shared Zone, independently
//! of session ticks and the ordinary movement/attack readiness gate.
use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ZoneSpecialMonsterState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<super::visibility_ai::VisibilityAiState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_summons: Option<super::stage_summons::StageSummonState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revival: Option<super::revival_ai::RevivingZombieState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactive: Option<super::reactive_ai::ReactiveAiState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sleep: Option<super::sleep_ai::SleepAiState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armadillo: Option<super::armadillo_ai::ArmadilloState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spider: Option<super::spider_ai::SpiderAiState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<super::node_ai::NodeAiState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passive: Option<super::passive_ai::PassiveAiState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trap_rock: Option<super::trap_rock_ai::TrapRockState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thunder: Option<super::thunder_ai::ThunderAiState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shinsu: Option<super::shinsu_ai::ShinsuState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub great_fox: Option<super::great_fox_ai::GreatFoxState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub town_archer: Option<super::town_archer_ai::TownArcherState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hugger: Option<super::hugger_ai::HuggerState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mud_boulder: Option<super::mud_boulder_ai::MudBoulderState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hell: Option<super::hell_ai::HellState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horned: Option<super::horned_ai::HornedState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kirin_snow: Option<super::kirin_snow_ai::KirinSnowState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horned_encounter: Option<super::horned_encounter_ai::HornedEncounterState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_queen: Option<super::tree_queen_ai::TreeQueenState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evil_mir: Option<super::evil_mir_ai::EvilMirState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yimoogi: Option<super::yimoogi_ai::YimoogiState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meow: Option<super::meow_ai::MeowState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tucson: Option<super::tucson_ai::TucsonState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pet_special: Option<super::pet_special_ai::PetSpecialState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snake_totem: Option<super::snake_totem_ai::SnakeTotemState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stone_trap: Option<super::stone_trap_ai::StoneTrapState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vampire: Option<super::vampire_ai::VampireState>,
    pub wooma: Option<WoomaState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WoomaState {
    stage: u8,
    mad_until_ms: u64,
    next_teleport_ms: u64,
    base_move_speed_ms: u64,
    base_attack_speed_ms: u64,
}

impl ZoneRuntime {
    pub(super) fn tick_special_monster_states(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| (m.ai == 11 && !m.dead && m.hp > 0).then_some(id))
            .collect();
        let mut out = Vec::new();
        for id in ids {
            out.extend(self.tick_wooma_taurus(id, now_ms));
        }
        out
    }

    fn tick_wooma_taurus(&mut self, id: u32, now_ms: u64) -> Vec<ZoneOutbound> {
        let m = self
            .native_monsters
            .get_mut(&id)
            .expect("selected living monster");
        let state = m.special_ai.get_or_insert_with(Default::default);
        let w = state.wooma.get_or_insert(WoomaState {
            stage: 7,
            mad_until_ms: 0,
            next_teleport_ms: 0,
            base_move_speed_ms: m.move_speed_ms,
            base_attack_speed_ms: m.attack_speed_ms,
        });
        // Crystal uses strict > boundaries and does not shorten an already
        // scheduled attack when the stage changes.
        if w.mad_until_ms != 0 && now_ms > w.mad_until_ms {
            w.mad_until_ms = 0;
            m.move_speed_ms = w.base_move_speed_ms;
            m.attack_speed_ms = w.base_attack_speed_ms;
        }
        let check_teleport = now_ms > w.next_teleport_ms;
        if check_teleport {
            w.next_teleport_ms = now_ms.saturating_add(10_000);
        }
        if m.max_hp >= 7 {
            let stage = (m.hp.max(0) / (m.max_hp / 7)).min(u8::MAX as i32) as u8;
            if stage < w.stage {
                w.mad_until_ms = now_ms.saturating_add(8_000);
                m.move_speed_ms = 400;
                m.attack_speed_ms = 500;
            }
            w.stage = stage;
        }
        let position = m.position.clone();
        if !check_teleport {
            return Vec::new();
        }
        let blocked = [
            MirDirection::Up,
            MirDirection::UpRight,
            MirDirection::Right,
            MirDirection::DownRight,
            MirDirection::Down,
            MirDirection::DownLeft,
            MirDirection::Left,
            MirDirection::UpLeft,
        ]
        .into_iter()
        .filter(|d| !self.can_native_monster_occupy(id, &offset_point(&position, *d, 1)))
        .count();
        if blocked < 5 {
            return Vec::new();
        }
        let Some(destination) = self.special_monster_teleport_destination(id, now_ms) else {
            return Vec::new();
        };
        self.teleport_special_monster(id, destination, now_ms, 0)
    }

    fn special_monster_teleport_destination(&self, id: u32, now_ms: u64) -> Option<Point> {
        // TeleportRandom(4,0) selects from the whole map, NOT a four-tile
        // radius. An unbounded synthetic map has no finite walkable catalogue.
        let bounds = self.collision.bounds()?;
        let current = &self.native_monsters.get(&id)?.position;
        let mut occupied: BTreeSet<_> = self.occupancy.keys().copied().collect();
        occupied.extend(self.native_monsters.iter().filter_map(|(&other, m)| {
            (other != id && !m.dead && m.hp > 0 && monster_visibility_is_visible(m))
                .then_some((m.position.x, m.position.y))
        }));
        occupied.extend(self.objects.iter().filter_map(|(&other, o)| {
            (other != id && retained_zone_object_blocks_tile(o, &o.position))
                .then_some((o.position.x, o.position.y))
        }));
        let mut random = now_ms ^ (u64::from(id) << 32) ^ 0xD1B54A32D192ED03;
        // Rejection sampling avoids a whole-map scan on ordinary open maps.
        // A deterministic exhaustive fallback still finds scarce free cells.
        let width = u64::try_from(i64::from(bounds.max_x) - i64::from(bounds.min_x) + 1).ok()?;
        let height = u64::try_from(i64::from(bounds.max_y) - i64::from(bounds.min_y) + 1).ok()?;
        let area = width.checked_mul(height).filter(|area| *area > 0)?;
        for _ in 0..64 {
            let cell = special_ai_random(&mut random) % area;
            let point = Point {
                x: (i64::from(bounds.min_x) + (cell % width) as i64) as i32,
                y: (i64::from(bounds.min_y) + (cell / width) as i64) as i32,
            };
            if point != *current
                && !self.collision.is_blocked(&point)
                && !occupied.contains(&(point.x, point.y))
            {
                return Some(point);
            }
        }
        let mut count = 0u64;
        let mut selected = None;
        for y in bounds.min_y..=bounds.max_y {
            for x in bounds.min_x..=bounds.max_x {
                let point = Point { x, y };
                if point == *current
                    || self.collision.is_blocked(&point)
                    || occupied.contains(&(x, y))
                {
                    continue;
                }
                count += 1;
                let roll = special_ai_random(&mut random);
                if roll % count == 0 {
                    selected = Some(point);
                }
            }
        }
        selected
    }

    pub(super) fn teleport_special_monster(
        &mut self,
        id: u32,
        destination: Point,
        now_ms: u64,
        effect_type: u8,
    ) -> Vec<ZoneOutbound> {
        let Some(object) = self.objects.get(&id) else {
            return Vec::new();
        };
        let ServerPacket::ObjectMonster { info } = &object.packet else {
            return Vec::new();
        };
        let health_expire = object.health.as_ref().map(|h| h.expire).unwrap_or(0);
        let mut info = info.clone();
        info.location = destination.clone();
        let old_recipients: Vec<_> = self
            .players
            .iter()
            .filter_map(|(session, p)| {
                p.visible_object_ids
                    .contains(&id)
                    .then_some(session.clone())
            })
            .collect();
        let mut out = Vec::new();
        if !old_recipients.is_empty() {
            out.push(ZoneOutbound::ToMany {
                session_ids: old_recipients,
                packets: vec![
                    ServerPacket::ObjectTeleportOut {
                        object_id: id,
                        effect_type,
                    },
                    ServerPacket::ObjectRemove { object_id: id },
                ],
            });
        }
        for p in self.players.values_mut() {
            p.visible_object_ids.remove(&id);
        }
        let m = self
            .native_monsters
            .get_mut(&id)
            .expect("teleporting monster exists");
        m.position = destination.clone();
        // No ordinary pursuit/attack in the same turn after clearing Target.
        m.next_ai_ready_at_ms = m.next_ai_ready_at_ms.max(now_ms.saturating_add(1));
        // Preserve retained health/buffs: a fresh generic spawn would discard
        // those even though teleportation is the same monster incarnation.
        let health = ObjectHealthInfo {
            object_id: id,
            percent: ((i64::from(m.hp) * 100 / i64::from(m.max_hp.max(1))).clamp(0, 100)) as u8,
            expire: health_expire,
        };
        let object = self
            .objects
            .get_mut(&id)
            .expect("retained teleporting monster");
        object.position = destination.clone();
        object.packet = ServerPacket::ObjectMonster { info };
        // Crystal's teleport-in effect precedes the refreshed health packet.
        // Keep AOI materialization from inserting a premature duplicate health.
        object.health = None;
        self.object_grid.insert(id, &destination);
        out.extend(self.diff_all_zone_object_visibility());
        self.objects
            .get_mut(&id)
            .expect("teleport retained object")
            .health = Some(health.clone());
        let recipients: Vec<_> = self
            .players
            .iter()
            .filter_map(|(session, p)| {
                p.visible_object_ids
                    .contains(&id)
                    .then_some(session.clone())
            })
            .collect();
        if !recipients.is_empty() {
            out.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![
                    ServerPacket::ObjectTeleportIn {
                        object_id: id,
                        effect_type,
                    },
                    ServerPacket::ObjectHealth { info: health },
                ],
            });
        }
        out
    }
}

fn special_ai_random(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut roll = *state;
    roll = (roll ^ (roll >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    roll = (roll ^ (roll >> 27)).wrapping_mul(0x94D049BB133111EB);
    roll ^ (roll >> 31)
}
