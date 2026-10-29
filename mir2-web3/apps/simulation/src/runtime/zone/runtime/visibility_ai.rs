//! Shared visibility/state rules from Crystal CannibalPlant, EvilCentipede and ZumaMonster.
//! Hidden plants do not have a public GetInfo; stone Zuma are visible but immune.
use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct VisibilityAiState {
    #[serde(default)]
    pub(crate) stationary: Option<super::statue_centipede_ai::StatueCentipedeState>,
    pub(crate) hidden: bool,
    pub(crate) stoned: bool,
    next_check_ms: u64,
    action_ready_ms: u64,
    #[serde(default)]
    dig_out: Option<DigOutState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DigOutState {
    effect_due_ms: u64,
    location: Point,
    direction: MirDirection,
    emitted: bool,
}

impl VisibilityAiState {
    pub(crate) fn for_ai(ai: u8) -> Option<Self> {
        match ai {
            5 | 14 | 24 | 124 | 125 => Some(Self {
                hidden: true,
                ..Self::default()
            }),
            15 | 16 | 17 | 173 | 174 => Some(Self {
                stoned: true,
                ..Self::default()
            }),
            _ => None,
        }
    }
}

fn state(monster: &ZoneNativeMonster) -> Option<VisibilityAiState> {
    monster
        .special_ai
        .as_ref()
        .and_then(|s| s.visibility.clone())
        .or_else(|| VisibilityAiState::for_ai(monster.ai))
}

pub(in crate::runtime::zone) fn initialize_monster_visibility(monster: &mut ZoneNativeMonster) {
    if let Some(initial) = VisibilityAiState::for_ai(monster.ai) {
        monster
            .special_ai
            .get_or_insert_with(Default::default)
            .visibility
            .get_or_insert(initial);
    }
}

pub(super) fn monster_visibility_is_visible(monster: &ZoneNativeMonster) -> bool {
    state(monster).is_none_or(|s| !s.hidden) && trap_rock_visible(monster)
}

pub(super) fn monster_visibility_is_attackable(monster: &ZoneNativeMonster) -> bool {
    state(monster).is_none_or(|s| !s.hidden && !s.stoned) && trap_rock_visible(monster)
}

pub(super) fn monster_visibility_can_act(monster: &ZoneNativeMonster, now_ms: u64) -> bool {
    state(monster).is_none_or(|s| !s.hidden && !s.stoned && now_ms > s.action_ready_ms)
}

pub(super) fn monster_visibility_can_move(monster: &ZoneNativeMonster) -> bool {
    !matches!(monster.ai, 5 | 14) && monster_visibility_is_attackable(monster)
}

pub(super) fn sync_monster_visibility_packet(
    monster: &ZoneNativeMonster,
    packet: &mut ServerPacket,
) {
    if let (Some(s), ServerPacket::ObjectMonster { info }) = (state(monster), packet) {
        info.hidden = s.hidden;
        // Only Zuma own Extra here; other special families retain their own flags.
        if matches!(monster.ai, 15 | 16 | 17 | 173 | 174) {
            info.extra = s.stoned;
        }
    }
}

impl ZoneRuntime {
    pub(super) fn tick_monster_visibility_states(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                (VisibilityAiState::for_ai(m.ai).is_some()
                    && ((!m.dead && m.hp > 0)
                        || state(m).is_some_and(|s| s.dig_out.is_some_and(|d| !d.emitted))))
                .then_some(id)
            })
            .collect();
        let mut out = Vec::new();
        for id in ids {
            let Some(monster) = self.native_monsters.get(&id).cloned() else {
                continue;
            };
            let mut current = state(&monster).expect("supported visibility family");
            if matches!(monster.ai, 24 | 124 | 125) {
                // Crystal DigOutZombie.ProcessAI checks every 2000ms and only
                // ever reveals. Its decorative ground spell uses the original
                // position/direction, even if the monster later moves or dies.
                if !monster.dead && current.hidden && now_ms > current.next_check_ms {
                    current.next_check_ms = now_ms.saturating_add(2_000);
                    if self.visibility_ai_target_near(&monster, 3) {
                        current.hidden = false;
                        current.action_ready_ms = now_ms.saturating_add(2_000);
                        current.dig_out = Some(DigOutState {
                            effect_due_ms: now_ms.saturating_add(if monster.ai == 24 {
                                1_000
                            } else {
                                500
                            }),
                            location: monster.position.clone(),
                            direction: monster.direction,
                            emitted: false,
                        });
                    }
                }
                let effect = current
                    .dig_out
                    .as_mut()
                    .filter(|d| !d.emitted && now_ms > d.effect_due_ms)
                    .map(|d| {
                        d.emitted = true;
                        (d.location.clone(), d.direction)
                    });
                out.extend(self.commit_visibility_state(id, current, false, now_ms));
                if let Some((location, direction)) = effect {
                    out.extend(
                        self.spawn_shared_dig_out_effect(monster.ai, location, direction, now_ms),
                    );
                }
                continue;
            }
            if matches!(monster.ai, 15 | 16 | 17 | 173 | 174) {
                if !current.stoned
                    || now_ms <= current.action_ready_ms
                    || !self.visibility_ai_target_near(&monster, 2)
                {
                    continue;
                }
                // WakeAll is a single radius around the triggering monster, not a
                // recursively expanding chain through newly awakened neighbours.
                let wave: Vec<_> = self
                    .native_monsters
                    .iter()
                    .filter_map(|(&other, m)| {
                        (!m.dead
                            && m.hp > 0
                            && state(m).is_some_and(|s| s.stoned)
                            && zone_tile_distance(&monster.position, &m.position) <= 14)
                            .then_some(other)
                    })
                    .collect();
                for other in wave {
                    let mut awake = state(&self.native_monsters[&other]).expect("stone state");
                    awake.stoned = false;
                    awake.action_ready_ms = now_ms.saturating_add(1_000);
                    out.extend(self.commit_visibility_state(other, awake, false, now_ms));
                }
                continue;
            }
            if now_ms <= current.next_check_ms {
                continue;
            }
            current.next_check_ms = now_ms.saturating_add(2_000);
            let radius = if monster.ai == 14 && !current.hidden {
                7
            } else {
                3
            };
            let nearby = if monster.ai == 14 {
                !self
                    .native_entity_targets(
                        id,
                        &monster.position,
                        radius,
                        super::entity_combat::EntityTargetPurpose::Search,
                        now_ms,
                    )
                    .is_empty()
            } else {
                self.visibility_ai_target_near(&monster, radius)
            };
            let reset_hp = !current.hidden && !nearby;
            if current.hidden && nearby {
                current.hidden = false;
                current.action_ready_ms =
                    now_ms.saturating_add(if monster.ai == 5 { 1_000 } else { 2_000 });
            } else if reset_hp {
                current.hidden = true;
                current.next_check_ms = now_ms.saturating_add(3_000);
            }
            out.extend(self.commit_visibility_state(id, current, reset_hp, now_ms));
        }
        out
    }

    fn spawn_shared_dig_out_effect(
        &mut self,
        ai: u8,
        location: Point,
        direction: MirDirection,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let object_id = self.unique_object_id(0);
        let packet = ServerPacket::ObjectSpell {
            info: ObjectSpellInfo {
                object_id,
                location: location.clone(),
                direction,
                spell: if ai == 24 {
                    Spell::DigOutZombie
                } else {
                    Spell::DigOutArmadillo
                },
                param: false,
            },
        };
        // Decorative, nonblocking and casterless (Crystal SpellObject default
        // branch): no damage/FireWall simulation. Retain for late AOI entrants
        // and checkpoint restore; expire strictly after five minutes.
        self.object_grid.insert(object_id, &location);
        self.objects.insert(
            object_id,
            ZoneObject {
                object_id,
                position: location,
                packet,
                health: None,
                mana: None,
                expires_at_ms: Some(now_ms.saturating_add(300_001)),
                buffs: BTreeMap::new(),
            },
        );
        self.diff_all_zone_object_visibility()
    }

    fn visibility_ai_target_near(&self, monster: &ZoneNativeMonster, radius: i32) -> bool {
        // Use the shared population; a second player's proximity must wake the same object.
        self.players.values().any(|p| {
            !p.dead
                && !p.hidden
                && !p.chat_profile.in_safe_zone
                && zone_tile_distance(&monster.position, &p.position) <= radius
                && monster.hostile_to_player
                && !monster
                    .friendly_guild
                    .as_deref()
                    .zip(p.chat_profile.guild_name.as_deref())
                    .is_some_and(|(friendly, guild)| friendly.eq_ignore_ascii_case(guild))
        }) || self.native_monsters.values().any(|m| {
            !m.dead
                && m.hp > 0
                && m.hostile_to_player != monster.hostile_to_player
                && monster_visibility_is_attackable(m)
                && zone_tile_distance(&monster.position, &m.position) <= radius
        })
    }

    fn commit_visibility_state(
        &mut self,
        id: u32,
        next: VisibilityAiState,
        reset_hp: bool,
        _now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        let previous = state(&self.native_monsters[&id]).expect("supported visibility state");
        let changed = previous.hidden != next.hidden || previous.stoned != next.stoned;
        let before =
            self.native_monster_visible_recipients(id, &self.native_monsters[&id].position);
        let monster = self.native_monsters.get_mut(&id).expect("selected monster");
        if reset_hp {
            monster.hp = monster.max_hp;
        }
        monster
            .special_ai
            .get_or_insert_with(Default::default)
            .visibility = Some(next.clone());
        if let Some(object) = self.objects.get_mut(&id) {
            sync_monster_visibility_packet(monster, &mut object.packet);
            if reset_hp {
                if let Some(health) = object.health.as_mut() {
                    health.percent = 100;
                }
            }
        }
        if !changed {
            return Vec::new();
        }
        let mut out = Vec::new();
        if next.hidden && !before.is_empty() {
            out.push(ZoneOutbound::ToMany {
                session_ids: before,
                packets: vec![ServerPacket::ObjectHide { object_id: id }],
            });
        }
        // Reveal: materialize ObjectMonster before ObjectShow. Hide: remove from
        // AOI after ObjectHide so late joiners never learn a hidden plant.
        out.extend(self.diff_all_zone_object_visibility());
        if !next.hidden {
            let recipients =
                self.native_monster_visible_recipients(id, &self.native_monsters[&id].position);
            if !recipients.is_empty() {
                out.push(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets: vec![ServerPacket::ObjectShow { object_id: id }],
                });
            }
        }
        out
    }
}
