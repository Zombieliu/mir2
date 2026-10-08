//! Trusted projection of the shared Request/Classic Sabuk record into its Zone.
//! No client command can create this authority or supply guild membership.
use super::*;
use crate::config::WorldEntityDisposition;
use crate::conquest::{
    default_sabuk_defenses, ConquestDefenseKind, ConquestDefenseState, ConquestPolicy,
    SharedConquestRecord,
};
use crate::runtime::zone::types::ZoneMonsterDefense;
use std::sync::OnceLock;

const ARCHER_REGEN_DELAY_MS: u64 = 10_000;

fn sabuk_source_defenses() -> &'static BTreeMap<String, ConquestDefenseState> {
    static SOURCE: OnceLock<BTreeMap<String, ConquestDefenseState>> = OnceLock::new();
    SOURCE.get_or_init(default_sabuk_defenses)
}

/// Obtained from the shared guild store, then matched to the live Zone identity.
/// An explicit `None` guild distinguishes a known unguilded player from a player
/// whose trusted membership has not yet been refreshed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneConquestMembership {
    pub session_id: SessionId,
    pub account_id: String,
    pub character_index: i32,
    pub guild_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneConquestPlayerSample {
    pub session_id: SessionId,
    pub account_id: String,
    pub character_index: i32,
    pub map_file_name: String,
    pub position: Point,
    pub hp: i32,
    pub alive: bool,
}

/// The revision and previous committed HP fence samples taken before a repair.
/// The server store must compare both before applying an authoritative HP delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneConquestDefenseSample {
    pub conquest_index: i32,
    pub record_revision: u64,
    pub key: String,
    pub hp: i32,
    pub committed_hp: i32,
    pub battle_day: Option<i64>,
    pub war_ends_ms: u64,
    pub clock_generation: u64,
    /// A trusted Zone damage hook admitted at least one real HP loss during this
    /// exact battle and clock generation. Final samples may be flushed after the
    /// cutoff; this never permits a new attack after the cutoff.
    pub damage_was_admitted_before_cutoff: bool,
}

#[derive(Debug, Clone)]
struct DefenseBinding {
    object_id: u32,
    source: ConquestDefenseState,
    cool_eye: bool,
    target: Option<(SessionId, u32)>,
    fear_until_ms: u64,
    action_until_ms: u64,
    next_regen_at_ms: u64,
    last_accepted_damage_zone_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct WarRegion {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

impl WarRegion {
    fn contains(self, point: &Point) -> bool {
        (self.min_x..=self.max_x).contains(&point.x) && (self.min_y..=self.max_y).contains(&point.y)
    }

    fn include(&mut self, x: i32, y: i32) {
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
    }
}

/// A derived server projection, deliberately absent from client serialization.
/// After restoring a Zone checkpoint the shared record is re-applied and live
/// guild membership is fetched again, rather than trusting saved authorization.
#[derive(Debug, Clone)]
pub(super) struct ZoneConquestProjection {
    index: i32,
    record_revision: u64,
    owner_guild_id: Option<String>,
    attacker_guild_id: Option<String>,
    war_active: bool,
    battle_day: Option<i64>,
    clock_generation: u64,
    war_ends_epoch_ms: u64,
    war_deadline_zone_ms: u64,
    authority_epoch_ms: u64,
    map_file_name: String,
    palace_file_name: String,
    region: WarRegion,
    memberships: BTreeMap<SessionId, ZoneConquestMembership>,
    defenses: BTreeMap<String, DefenseBinding>,
}

impl ZoneConquestProjection {
    fn war_is_on(&self, now_ms: u64) -> bool {
        self.war_active && now_ms < self.war_deadline_zone_ms
    }
}

impl ZoneRuntime {
    pub(crate) fn conquest_projection_ready(&self) -> bool {
        self.conquest.is_some()
    }
    pub(crate) fn conquest_record_revision(&self) -> Option<u64> {
        self.conquest
            .as_ref()
            .map(|projection| projection.record_revision)
    }
    pub(crate) fn conquest_membership_matches(
        &self,
        session_id: &SessionId,
        guild_id: Option<&str>,
    ) -> bool {
        let Some(player) = self.players.get(session_id) else {
            return false;
        };
        let Some(member) = self
            .conquest
            .as_ref()
            .and_then(|projection| projection.memberships.get(session_id))
        else {
            return false;
        };
        member.account_id == player.account_id
            && member.character_index == player.character_index
            && match (member.guild_id.as_deref(), guild_id) {
                (None, None) => true,
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                _ => false,
            }
    }
    /// Apply only a committed server record. Epoch time belongs to the shared
    /// scheduler; Zone time belongs to gameplay. Converting the deadline once
    /// also fences delayed attacks between scheduler ticks.
    pub(crate) fn apply_conquest_projection(
        &mut self,
        policy: &ConquestPolicy,
        record: &SharedConquestRecord,
        memberships: &[ZoneConquestMembership],
        authority_now_epoch_ms: u64,
        zone_now_ms: u64,
    ) -> Result<Vec<ZoneOutbound>, String> {
        policy.validate()?;
        record.validate()?;
        if policy.index != 1
            || record.index != policy.index
            || policy.map_file_name != "3"
            || policy.palace_file_name != "0150"
        {
            return Err("unsupported shared conquest projection".into());
        }
        if self.key.map_file_name != policy.map_file_name
            && self.key.map_file_name != policy.palace_file_name
        {
            return Ok(Vec::new());
        }
        if self.key.shard_id != "primary"
            || self.key.channel_id != 0
            || self.key.instance_id != "main"
        {
            return Err("shared conquest requires the primary map instance".into());
        }

        // Source metadata is fixed. A corrupted record must not turn an
        // arbitrary map object into an attackable siege structure.
        let defaults = default_sabuk_defenses();
        if record.defenses.len() != defaults.len() {
            return Err("incomplete Sabuk defense projection".into());
        }
        let mut templates = BTreeMap::new();
        for (key, defense) in &record.defenses {
            let Some(source) = defaults.get(key) else {
                return Err("unknown Sabuk defense slot".into());
            };
            if defense.kind != source.kind
                || defense.slot != source.slot
                || defense.template_name != source.template_name
                || defense.x != source.x
                || defense.y != source.y
                || defense.max_hp != source.max_hp
            {
                return Err("Sabuk defense metadata differs from source".into());
            }
            let template = crystal_monster_by_name(&defense.template_name)
                .ok_or("missing source Sabuk monster template")?;
            let expected_ai = match defense.kind {
                ConquestDefenseKind::Gate => 81,
                ConquestDefenseKind::Wall => 82,
                ConquestDefenseKind::Archer => 80,
            };
            if template.ai != expected_ai || template.hp != defense.max_hp {
                return Err("invalid source Sabuk monster template".into());
            }
            templates.insert(key.clone(), template);
        }

        let remaining_ms = record.war_ends_ms.saturating_sub(authority_now_epoch_ms);
        let mut deadline = zone_now_ms
            .checked_add(remaining_ms)
            .ok_or("conquest Zone deadline exhausted")?;
        if let Some(previous) = &self.conquest {
            if previous.index != record.index
                || record.revision < previous.record_revision
                || authority_now_epoch_ms < previous.authority_epoch_ms
            {
                return Err("stale shared conquest projection".into());
            }
            if record.revision == previous.record_revision
                && (record.owner_guild_id != previous.owner_guild_id
                    || record.attacker_guild_id != previous.attacker_guild_id
                    || record.war_active != previous.war_active
                    || record.battle_day != previous.battle_day
                    || record.clock_generation != previous.clock_generation
                    || record.war_ends_ms != previous.war_ends_epoch_ms
                    || previous
                        .defenses
                        .iter()
                        .any(|(key, binding)| record.defenses.get(key) != Some(&binding.source)))
            {
                return Err("conflicting shared conquest revision".into());
            }
            if record.war_active
                && previous.war_active
                && record.war_ends_ms == previous.war_ends_epoch_ms
            {
                // Repeated projections and wall-clock adjustment cannot grant
                // extra gameplay time to the same battle.
                deadline = deadline.min(previous.war_deadline_zone_ms);
            }
        }

        let mut trusted_memberships = BTreeMap::new();
        for membership in memberships {
            let Some(player) = self.players.get(&membership.session_id) else {
                continue;
            };
            if membership.account_id != player.account_id
                || membership.character_index != player.character_index
            {
                continue;
            }
            if membership.guild_id.as_ref().is_some_and(|id| {
                id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit())
            }) {
                return Err("invalid trusted conquest guild identity".into());
            }
            let mut accepted = membership.clone();
            accepted.guild_id = accepted.guild_id.map(|id| id.to_ascii_lowercase());
            if trusted_memberships
                .insert(accepted.session_id.clone(), accepted)
                .is_some()
            {
                return Err("duplicate trusted conquest membership".into());
            }
        }

        // Actual source FullMap=false, centre(641,287), Size30. Its rectangle
        // omits the main gate and southern archers. The explicit playable data
        // repair minimally expands those bounds to include defense footprints;
        // it never marks all of Mongchon as a PK-free war map.
        let mut region = WarRegion {
            min_x: 611,
            max_x: 671,
            min_y: 257,
            max_y: 317,
        };
        for (key, defense) in &record.defenses {
            region.include(defense.x, defense.y);
            if defense.kind == ConquestDefenseKind::Gate {
                for (dx, dy) in gate_ai::gate_offsets(templates[key].effect) {
                    region.include(defense.x + dx, defense.y + dy);
                }
            }
        }

        let previous = self.conquest.clone();
        // Plan every object ID before the first mutation. The ordinary Zone
        // allocator saturates its counter; preflight prevents exhausted IDs
        // from leaving a half-projected castle (or looping at u32::MAX).
        let mut defense_ids = BTreeMap::new();
        let mut planned_ids = BTreeSet::new();
        let mut candidate_id = self.next_object_id.max(1);
        if self.key.map_file_name == policy.map_file_name {
            for (key, defense) in &record.defenses {
                let previous_binding = previous.as_ref().and_then(|p| p.defenses.get(key));
                let retained_id = previous_binding
                    .map(|binding| binding.object_id)
                    .or_else(|| {
                        self.native_monsters.iter().find_map(|(id, monster)| {
                            (!planned_ids.contains(id)
                                && monster.owner_session_id.is_none()
                                && monster.name == defense.template_name
                                && monster.position.x == defense.x
                                && monster.position.y == defense.y)
                                .then_some(*id)
                        })
                    });
                let object_id = if let Some(id) = retained_id {
                    if planned_ids.contains(&id)
                        || self.players.values().any(|p| p.object_id == id)
                        || self.ground_drops.contains_key(&id)
                        || self.claimed_ground_drops.contains_key(&id)
                        || self.native_monsters.get(&id).is_some_and(|m| {
                            m.name != defense.template_name
                                || m.owner_session_id.is_some()
                                || m.position.x != defense.x
                                || m.position.y != defense.y
                        })
                    {
                        return Err("conquest defense object identity was replaced".into());
                    }
                    id
                } else {
                    while self.object_id_in_use(candidate_id) || planned_ids.contains(&candidate_id)
                    {
                        candidate_id = candidate_id
                            .checked_add(1)
                            .ok_or("conquest object IDs exhausted")?;
                    }
                    let id = candidate_id;
                    candidate_id = candidate_id
                        .checked_add(1)
                        .ok_or("conquest object IDs exhausted")?;
                    id
                };
                planned_ids.insert(object_id);
                defense_ids.insert(key.clone(), object_id);
            }
        }
        let mut projection = ZoneConquestProjection {
            index: record.index,
            record_revision: record.revision,
            owner_guild_id: record
                .owner_guild_id
                .as_ref()
                .map(|id| id.to_ascii_lowercase()),
            attacker_guild_id: record
                .attacker_guild_id
                .as_ref()
                .map(|id| id.to_ascii_lowercase()),
            war_active: record.war_active,
            battle_day: record.battle_day,
            clock_generation: record.clock_generation,
            war_ends_epoch_ms: record.war_ends_ms,
            war_deadline_zone_ms: deadline,
            authority_epoch_ms: authority_now_epoch_ms,
            map_file_name: policy.map_file_name.clone(),
            palace_file_name: policy.palace_file_name.clone(),
            region,
            memberships: trusted_memberships,
            defenses: BTreeMap::new(),
        };
        let mut outbounds = Vec::new();
        if self.key.map_file_name == policy.map_file_name {
            let mut used_ids = BTreeSet::new();
            let disposition = if projection.war_is_on(zone_now_ms) {
                WorldEntityDisposition::Hostile
            } else {
                WorldEntityDisposition::Friendly
            };
            for (key, defense) in &record.defenses {
                let previous_binding = previous.as_ref().and_then(|p| p.defenses.get(key));
                // Re-adopt checkpoint objects without trusting saved war/guild
                // authorization. Duplicate source tiles still have distinct IDs.
                let object_id = defense_ids[key];
                used_ids.insert(object_id);
                let old_monster = self.native_monsters.get(&object_id).cloned();
                let template = &templates[key];
                let hp = match (previous_binding, old_monster.as_ref()) {
                    (Some(binding), Some(monster))
                        if defense.kind == ConquestDefenseKind::Archer
                            && binding.source.hp > 0
                            && defense.hp >= binding.source.hp =>
                    {
                        // Living archer increases are already applied by Zone
                        // regen. Membership/lease refresh must neither undo
                        // that heal nor replay it over a subsequent hit/death.
                        // Only a committed dead->alive rehire revives an archer.
                        monster.hp.clamp(0, defense.max_hp)
                    }
                    (Some(binding), Some(monster)) if defense.hp <= binding.source.hp => {
                        // Lease/capture/event revisions must not undo damage
                        // that the next trusted HP sample has yet to commit.
                        monster.hp.min(defense.hp).max(0)
                    }
                    _ => defense.hp,
                };
                let spawn = ZoneMonsterSpawn {
                    crystal_drop_seed: None,
                    object_id,
                    name: defense.template_name.clone(),
                    name_colour_argb: -1,
                    image: template.image,
                    ai: template.ai,
                    disposition: Some(disposition),
                    level: template.level,
                    max_hp: defense.max_hp,
                    hp,
                    experience: 0,
                    move_speed_ms: u64::from(template.move_speed),
                    attack_speed_ms: u64::from(template.attack_speed),
                    friendly_guild: projection.owner_guild_id.clone(),
                    position: Point {
                        x: defense.x,
                        y: defense.y,
                    },
                    direction: MirDirection::Up,
                    defense: ZoneMonsterDefense::from_crystal_template(template),
                    respawn: None,
                    drops: Vec::new(),
                };
                let (_, spawned_outbounds) =
                    self.spawn_authoritative_monster_internal(&spawn, zone_now_ms, false, true);
                outbounds.extend(spawned_outbounds);
                self.native_monster_respawns.remove(&object_id);

                let monster = self
                    .native_monsters
                    .get_mut(&object_id)
                    .expect("trusted defense spawn must retain its allocated object ID");
                // Generic respawn placement avoids overlaps. Siege structures
                // instead occupy their exact configured footprint, including
                // the original duplicated archer tile; no map cells are edited.
                monster.position = spawn.position.clone();
                monster.hp = hp;
                monster.max_hp = defense.max_hp;
                monster.dead = hp == 0;
                monster.experience = 0;
                monster.drops.clear();
                monster.crystal_drop_seed = None;
                monster.owner_session_id = None;
                monster.master_object_id = 0;
                monster.owner_player_object_id = 0;
                monster.disposition = Some(disposition);
                monster.hostile_to_player = disposition == WorldEntityDisposition::Hostile;
                monster.friendly_guild = projection.owner_guild_id.clone();
                if defense.kind == ConquestDefenseKind::Gate {
                    if defense.open && hp > 0 {
                        monster.direction = MirDirection::Left;
                    } else {
                        initialize_gate(monster);
                    }
                }
                let direction = monster.direction;
                let was_dead = old_monster.as_ref().is_some_and(|m| m.dead || m.hp <= 0);
                let mut packet = native_monster_spawn_packet(&spawn, object_id);
                if let ServerPacket::ObjectMonster { info } = &mut packet {
                    info.direction = direction;
                    info.dead = hp == 0;
                }
                self.apply_zone_object_packets(std::slice::from_ref(&packet), zone_now_ms);

                let mut updates = Vec::new();
                if let Some(old) = &old_monster {
                    if old.hp != hp {
                        updates.push(ServerPacket::ObjectHealth {
                            info: ObjectHealthInfo {
                                object_id,
                                percent: native_monster_health_percent(hp, defense.max_hp),
                                expire: 0,
                            },
                        });
                    }
                    if !was_dead && hp == 0 {
                        updates.push(ServerPacket::ObjectDied {
                            info: ObjectDiedInfo {
                                object_id,
                                location: spawn.position.clone(),
                                direction,
                                kind: 0,
                            },
                        });
                    }
                    let gate_toggled = defense.kind == ConquestDefenseKind::Gate
                        && previous_binding.is_some_and(|p| p.source.open != defense.open)
                        && hp > 0;
                    if gate_toggled {
                        updates.push(ServerPacket::ObjectAttack {
                            info: ObjectAttackInfo {
                                object_id,
                                location: spawn.position.clone(),
                                direction,
                                spell: 0,
                                level: 0,
                                attack_type: u8::from(!defense.open),
                            },
                        });
                    } else if old.direction != direction {
                        updates.push(ServerPacket::ObjectTurn {
                            movement: ObjectMovement {
                                object_id,
                                position: spawn.position.clone(),
                                direction,
                            },
                        });
                    }
                }
                if !updates.is_empty() {
                    self.apply_zone_object_packets(&updates, zone_now_ms);
                    let recipients =
                        self.native_monster_visible_recipients(object_id, &spawn.position);
                    if !recipients.is_empty() {
                        outbounds.push(ZoneOutbound::ToMany {
                            session_ids: recipients,
                            packets: updates,
                        });
                    }
                }
                let mut binding = previous_binding.cloned().unwrap_or_else(|| DefenseBinding {
                    object_id,
                    source: defense.clone(),
                    cool_eye: u64::from(template.cool_eye)
                        > crate::runtime::combat::crystal_accuracy_roll(
                            zone_now_ms,
                            object_id,
                            object_id,
                            100,
                        ),
                    target: None,
                    fear_until_ms: 0,
                    action_until_ms: zone_now_ms.saturating_add(2_000),
                    next_regen_at_ms: zone_now_ms.saturating_add(
                        crate::runtime::combat::crystal_accuracy_roll(
                            zone_now_ms,
                            object_id,
                            object_id,
                            ARCHER_REGEN_DELAY_MS,
                        ),
                    ),
                    last_accepted_damage_zone_ms: None,
                });
                let changed_damage_authority = previous.as_ref().is_none_or(|p| {
                    p.battle_day != record.battle_day
                        || p.clock_generation != record.clock_generation
                        || p.war_ends_epoch_ms != record.war_ends_ms
                });
                let repaired_or_rehired = previous_binding.is_some_and(|old| {
                    (defense.kind != ConquestDefenseKind::Archer || old.source.hp == 0)
                        && defense.hp > old.source.hp
                });
                if changed_damage_authority || repaired_or_rehired {
                    binding.last_accepted_damage_zone_ms = None;
                }
                binding.object_id = object_id;
                binding.source = defense.clone();
                if was_dead && hp > 0 {
                    binding.target = None;
                    binding.fear_until_ms = 0;
                    binding.action_until_ms = zone_now_ms.saturating_add(2_000);
                    binding.next_regen_at_ms = zone_now_ms.saturating_add(ARCHER_REGEN_DELAY_MS);
                }
                projection.defenses.insert(key.clone(), binding);
            }
            // Retire pre-feature synthetic normal respawns (extra palace gate
            // and relocated archer), not any ordinary world monster.
            let obsolete = self
                .native_monsters
                .iter()
                .filter_map(|(id, monster)| {
                    (!used_ids.contains(id)
                        && monster.owner_session_id.is_none()
                        && defaults.values().any(|s| s.template_name == monster.name))
                    .then_some(*id)
                })
                .collect::<Vec<_>>();
            for object_id in obsolete {
                outbounds.extend(self.despawn_world_event_monster(object_id, zone_now_ms));
            }
        }
        self.conquest = Some(projection);
        outbounds.extend(self.diff_all_zone_object_visibility());
        Ok(outbounds)
    }

    pub(crate) fn conquest_player_samples(&self) -> Vec<ZoneConquestPlayerSample> {
        self.players
            .values()
            .map(|player| ZoneConquestPlayerSample {
                session_id: player.session_id.clone(),
                account_id: player.account_id.clone(),
                character_index: player.character_index,
                map_file_name: self.key.map_file_name.clone(),
                position: player.position.clone(),
                hp: player.hp,
                alive: !player.dead && player.hp > 0,
            })
            .collect()
    }

    pub(crate) fn conquest_defense_samples(&self) -> Vec<ZoneConquestDefenseSample> {
        let Some(projection) = &self.conquest else {
            return Vec::new();
        };
        projection
            .defenses
            .iter()
            .filter_map(|(key, binding)| {
                let monster = self.native_monsters.get(&binding.object_id)?;
                Some(ZoneConquestDefenseSample {
                    conquest_index: projection.index,
                    record_revision: projection.record_revision,
                    key: key.clone(),
                    hp: if monster.dead {
                        0
                    } else {
                        monster.hp.clamp(0, binding.source.max_hp)
                    },
                    committed_hp: binding.source.hp,
                    battle_day: projection.battle_day,
                    war_ends_ms: projection.war_ends_epoch_ms,
                    clock_generation: projection.clock_generation,
                    damage_was_admitted_before_cutoff: binding
                        .last_accepted_damage_zone_ms
                        .is_some_and(|accepted| accepted < projection.war_deadline_zone_ms),
                })
            })
            .collect()
    }

    fn conquest_membership(&self, player: &ZonePlayer) -> Option<&ZoneConquestMembership> {
        self.conquest
            .as_ref()?
            .memberships
            .get(&player.session_id)
            .filter(|m| {
                m.account_id == player.account_id && m.character_index == player.character_index
            })
    }

    fn conquest_defense_binding(&self, object_id: u32) -> Option<&DefenseBinding> {
        self.conquest
            .as_ref()?
            .defenses
            .values()
            .find(|binding| binding.object_id == object_id)
    }

    /// Checkpoints retain physical actors, never castle authorization. Until a
    /// committed projection is installed, exact source actors must remain inert
    /// rather than falling through to ordinary AI80/82 combat and movement.
    /// Matching AI alone would incorrectly quarantine unrelated town guards.
    fn unprojected_sabuk_defense_kind(&self, object_id: u32) -> Option<ConquestDefenseKind> {
        if self.conquest.is_some() || self.key != ZoneKey::for_map("3") {
            return None;
        }
        let monster = self.native_monsters.get(&object_id)?;
        if !matches!(monster.ai, 80..=82)
            || monster.owner_session_id.is_some()
            || monster.master_object_id != 0
            || monster.owner_player_object_id != 0
        {
            return None;
        }
        let source = sabuk_source_defenses().values().find(|source| {
            monster.name == source.template_name
                && monster.max_hp == source.max_hp
                && monster.position.x == source.x
                && monster.position.y == source.y
        })?;
        let expected_ai = match source.kind {
            ConquestDefenseKind::Gate => 81,
            ConquestDefenseKind::Wall => 82,
            ConquestDefenseKind::Archer => 80,
        };
        let template = crystal_monster_by_name(&source.template_name)?;
        let object = self.objects.get(&object_id)?;
        if monster.ai != expected_ai
            || template.ai != expected_ai
            || template.hp != source.max_hp
            || !matches!(
                &object.packet,
                ServerPacket::ObjectMonster { info }
                    if info.object_id == object_id
                        && info.name == source.template_name
                        && info.ai == expected_ai
                        && info.image == template.image
                        && info.location == monster.position
            )
        {
            return None;
        }
        Some(source.kind)
    }

    pub(super) fn conquest_is_defense(&self, object_id: u32) -> bool {
        self.conquest_defense_binding(object_id).is_some()
            || self.unprojected_sabuk_defense_kind(object_id).is_some()
    }

    pub(super) fn conquest_is_archer(&self, object_id: u32) -> bool {
        self.conquest_defense_binding(object_id)
            .is_some_and(|binding| binding.source.kind == ConquestDefenseKind::Archer)
            || self.unprojected_sabuk_defense_kind(object_id) == Some(ConquestDefenseKind::Archer)
    }

    /// Called by the central HP mutation only after identity, war, friendly and
    /// damage checks accept a positive loss. A clock sample cannot reconstruct
    /// this proof from HP alone, particularly in the final polling interval.
    pub(super) fn record_conquest_damage_admitted(&mut self, object_id: u32, now_ms: u64) -> bool {
        let Some(projection) = self.conquest.as_mut() else {
            return false;
        };
        if !projection.war_is_on(now_ms) || !self.native_monsters.contains_key(&object_id) {
            return false;
        }
        let Some(binding) = projection
            .defenses
            .values_mut()
            .find(|b| b.object_id == object_id)
        else {
            return false;
        };
        binding.last_accepted_damage_zone_ms = Some(now_ms);
        true
    }

    pub(super) fn conquest_gate_is_open(&self, object_id: u32) -> bool {
        self.conquest_defense_binding(object_id)
            .is_some_and(|binding| {
                binding.source.kind == ConquestDefenseKind::Gate && binding.source.open
            })
    }

    /// Original defenses can take damage only while their conquest is at war.
    /// A delayed attack rechecks the actual live guild after every capture.
    pub(super) fn conquest_defense_accepts_damage(
        &self,
        object_id: u32,
        attacker: Option<&SessionId>,
        now_ms: u64,
    ) -> bool {
        if !self.conquest_is_defense(object_id) {
            return true;
        }
        let Some(projection) = &self.conquest else {
            return false;
        };
        if !projection.war_is_on(now_ms) {
            return false;
        }
        let Some(player) = attacker.and_then(|sid| self.players.get(sid)) else {
            return false;
        };
        let Some(membership) = self.conquest_membership(player) else {
            return false;
        };
        !player.dead
            && player.hp > 0
            && !membership
                .guild_id
                .as_ref()
                .zip(projection.owner_guild_id.as_ref())
                .is_some_and(|(guild, owner)| guild == owner)
    }

    pub(super) fn conquest_player_is_in_war_region(
        &self,
        player: &ZonePlayer,
        now_ms: u64,
    ) -> bool {
        self.conquest.as_ref().is_some_and(|projection| {
            projection.war_is_on(now_ms)
                && (self.key.map_file_name == projection.palace_file_name
                    || (self.key.map_file_name == projection.map_file_name
                        && projection.region.contains(&player.position)))
        })
    }

    /// Crystal PlayerObject.Die exempts AtWar(hitter) OR the victim's WarZone.
    /// Capture this from the authoritative actors at damage time, including for
    /// deferred hits; attack mode and a later session snapshot do not prove it.
    /// A dead victim remains available so immediate settlement can query after
    /// the HP transition without losing the exemption.
    pub(crate) fn conquest_player_kill_is_lawful(
        &self,
        attacker_session: &SessionId,
        attacker_object_id: u32,
        target_object_id: u32,
        now_ms: u64,
    ) -> bool {
        let Some(attacker) = self
            .players
            .get(attacker_session)
            .filter(|p| p.object_id == attacker_object_id)
        else {
            return false;
        };
        let Some(target) = self
            .players
            .values()
            .find(|p| p.object_id == target_object_id && p.session_id != *attacker_session)
        else {
            return false;
        };
        self.npc_teleport_config.map(&self.key.map_file_name).is_some_and(|map| map.fight)
            || self.conquest_player_is_in_war_region(target, now_ms)
            || attacker
                .chat_profile
                .guild_name
                .as_deref()
                .zip(target.chat_profile.guild_name.as_deref())
                .is_some_and(|(attacker_guild, _)| {
                    target
                        .chat_profile
                        .active_guild_wars
                        .iter()
                        .any(|war| war.eq_ignore_ascii_case(attacker_guild))
                })
    }

    /// Canonical attacker/owner enemy status is deliberately local to the
    /// battlefield. Third guilds and unguilded occupants contest the palace,
    /// but acquiring presence does not enroll them as an eligible challenger.
    pub(super) fn conquest_players_are_war_enemies(
        &self,
        attacker: &ZonePlayer,
        target: &ZonePlayer,
        now_ms: u64,
    ) -> bool {
        if attacker.session_id == target.session_id
            || attacker.dead
            || target.dead
            || attacker.hp <= 0
            || target.hp <= 0
            || attacker.chat_profile.in_safe_zone
            || target.chat_profile.in_safe_zone
            || !self.conquest_player_is_in_war_region(attacker, now_ms)
            || !self.conquest_player_is_in_war_region(target, now_ms)
        {
            return false;
        }
        let Some(projection) = &self.conquest else {
            return false;
        };
        let Some((owner, challenger)) = projection
            .owner_guild_id
            .as_ref()
            .zip(projection.attacker_guild_id.as_ref())
        else {
            return false;
        };
        let Some(a) = self
            .conquest_membership(attacker)
            .and_then(|m| m.guild_id.as_ref())
        else {
            return false;
        };
        let Some(b) = self
            .conquest_membership(target)
            .and_then(|m| m.guild_id.as_ref())
        else {
            return false;
        };
        a != b && ((a == owner && b == challenger) || (a == challenger && b == owner))
    }

    /// Preserve Crystal attack modes, adding siege EnemyGuild status only in the
    /// trusted local region. In particular Peace never becomes an auto-PK mode.
    pub(super) fn conquest_player_can_attack_player(
        &self,
        attacker: &ZonePlayer,
        target: &ZonePlayer,
        now_ms: u64,
    ) -> bool {
        // Crystal PlayerObject.IsAttackTarget(HumanObject):4684. This is not
        // used by the independent MonsterObject attacker (owned pet) branch.
        if self.npc_teleport_config.map(&self.key.map_file_name).is_some_and(|map| map.no_fight) {
            return false;
        }
        zone_player_can_attack_player_at(attacker, target, now_ms,
            self.owned_pet_brown_until(target),
            self.conquest_players_are_war_enemies(attacker, target, now_ms))
    }

    /// Recheck launch and impact: ownership may change before an arrow lands.
    pub(super) fn conquest_archer_hit_allowed(
        &self,
        source_id: u32,
        session_id: &SessionId,
        target_id: u32,
        now_ms: u64,
    ) -> bool {
        if !self.conquest_is_archer(source_id) {
            return true;
        }
        let Some(projection) = &self.conquest else {
            return false;
        };
        let Some(player) = self.players.get(session_id) else {
            return false;
        };
        let Some(membership) = self.conquest_membership(player) else {
            return false;
        };
        projection.war_is_on(now_ms)
            && player.object_id == target_id
            && !player.dead
            && player.hp > 0
            && !player.chat_profile.in_safe_zone
            && !player.combat_stats.gm_never_die
            && !membership
                .guild_id
                .as_ref()
                .zip(projection.owner_guild_id.as_ref())
                .is_some_and(|(guild, owner)| guild == owner)
    }

    /// Projectile AC/Agility accuracy is checked at impact, not while finding
    /// the target. This matches TownArcher's physical ProjectileAttack path.
    pub(super) fn conquest_archer_impact_allowed(
        &self,
        source_id: u32,
        session_id: &SessionId,
        target_id: u32,
        now_ms: u64,
    ) -> bool {
        if !self.conquest_is_archer(source_id) {
            return true;
        }
        if !self.conquest_archer_hit_allowed(source_id, session_id, target_id, now_ms) {
            return false;
        }
        let Some(template) = self
            .native_monsters
            .get(&source_id)
            .and_then(|m| crystal_monster_by_name(&m.name))
        else {
            return false;
        };
        let Some(player) = self.players.get(session_id) else {
            return false;
        };
        crate::runtime::combat::crystal_accuracy_roll(
            now_ms,
            source_id,
            target_id,
            player.combat_stats.agility.max(0) as u64 + 1,
        ) <= template.accuracy.max(0) as u64
    }

    /// Castle gates/walls stay in place; route-free conquest archers inherit
    /// TownArcher's 10-cell retained range and source projectile/cadence rules.
    pub(super) fn try_tick_conquest_defense(
        &mut self,
        object_id: u32,
        now_ms: u64,
    ) -> Option<Vec<ZoneOutbound>> {
        if self.unprojected_sabuk_defense_kind(object_id).is_some() {
            return Some(Vec::new());
        }
        let binding = self.conquest_defense_binding(object_id)?.clone();
        if binding.source.kind != ConquestDefenseKind::Archer {
            return Some(Vec::new());
        }
        let monster = self.native_monsters.get(&object_id)?.clone();
        if monster.dead
            || monster.hp <= 0
            || !self.conquest.as_ref()?.war_is_on(now_ms)
            || now_ms <= binding.action_until_ms
            || now_ms <= monster.next_attack_ready_at_ms
        {
            return Some(Vec::new());
        }
        let template = crystal_monster_by_name(&monster.name)?;
        let valid = |player: &ZonePlayer| {
            self.conquest_archer_hit_allowed(
                object_id,
                &player.session_id,
                player.object_id,
                now_ms,
            )
        };
        let retained = binding.target.as_ref().and_then(|(session_id, target_id)| {
            self.players
                .get(session_id)
                .filter(|p| p.object_id == *target_id && valid(p))
        });
        let target = retained
            .or_else(|| {
                self.players
                    .values()
                    .filter(|player| {
                        valid(player)
                            && zone_tile_distance(&monster.position, &player.position)
                                <= i32::from(template.view_range)
                            && (!player.hidden
                                || (binding.cool_eye && monster.level >= player.level))
                    })
                    .min_by_key(|player| {
                        (
                            zone_tile_distance(&monster.position, &player.position),
                            player.position.y,
                            player.position.x,
                            player.object_id,
                        )
                    })
            })
            .map(|p| (p.session_id.clone(), p.object_id, p.position.clone()));
        let Some((session_id, target_id, position)) = target else {
            if let Some(b) = self
                .conquest
                .as_mut()?
                .defenses
                .values_mut()
                .find(|b| b.object_id == object_id)
            {
                b.target = None;
            }
            return Some(Vec::new());
        };
        let distance = zone_tile_distance(&monster.position, &position);
        let live_binding = self
            .conquest
            .as_mut()?
            .defenses
            .values_mut()
            .find(|b| b.object_id == object_id)?;
        live_binding.target = Some((session_id.clone(), target_id));
        if distance > 10 || now_ms >= live_binding.fear_until_ms {
            live_binding.fear_until_ms = now_ms.saturating_add(2_000);
            if distance > 10 {
                live_binding.target = None;
                return Some(self.turn_native_monster(object_id, MirDirection::Up, now_ms));
            }
            return Some(Vec::new());
        }
        live_binding.action_until_ms = now_ms.saturating_add(300);
        let direction =
            zone_direction_toward(&monster.position, &position).unwrap_or(monster.direction);
        let damage =
            zone_roll_stat_range(template.min_dc, template.max_dc, now_ms, object_id, 0x80);
        let live = self.native_monsters.get_mut(&object_id)?;
        live.direction = direction;
        live.next_ai_ready_at_ms = now_ms.saturating_add(300);
        live.next_attack_ready_at_ms = now_ms.saturating_add(live.attack_speed_ms);
        if damage > 0 {
            self.pending_native_player_hits
                .push(PendingNativePlayerHit {
                    ready_at_ms: now_ms.saturating_add(500 + distance as u64 * 50),
                    attacker_object_id: object_id,
                    attacker_ai: 80,
                    target_session_id: session_id,
                    target_object_id: target_id,
                    damage,
                    magic: false,
                });
        }
        let packet = ServerPacket::ObjectRangeAttack {
            info: ObjectRangeAttackInfo {
                object_id,
                location: monster.position,
                direction,
                target_id,
                target: position.clone(),
                attack_type: 0,
                spell: 0,
                level: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
        Some(vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_combat_recipients(object_id, target_id, &position),
            packets: vec![packet],
        }])
    }

    /// MonsterObject.ProcessRegen is independent of attack/movement control.
    /// Original archers inherit 2.2%+1 every10s; gates/walls override CanRegen.
    /// Only living archers regenerate, including outside a battle. Death requires
    /// the owner's ordinary repair/rehire action and never silently respawns.
    pub(super) fn tick_conquest_archer_regen(&mut self, now_ms: u64) -> Vec<ZoneOutbound> {
        let Some(projection) = &mut self.conquest else {
            return Vec::new();
        };
        let mut packets = Vec::new();
        for binding in projection.defenses.values_mut() {
            if binding.source.kind != ConquestDefenseKind::Archer
                || now_ms < binding.next_regen_at_ms
            {
                continue;
            }
            binding.next_regen_at_ms = now_ms.saturating_add(ARCHER_REGEN_DELAY_MS);
            let Some(monster) = self.native_monsters.get_mut(&binding.object_id) else {
                continue;
            };
            if monster.dead || monster.hp <= 0 || monster.hp >= monster.max_hp {
                continue;
            }
            let amount =
                i32::try_from(i64::from(monster.max_hp) * 22 / 1_000 + 1).unwrap_or(i32::MAX);
            monster.hp = monster.hp.saturating_add(amount).min(monster.max_hp);
            packets.push((
                monster.position.clone(),
                ServerPacket::ObjectHealth {
                    info: ObjectHealthInfo {
                        object_id: binding.object_id,
                        percent: native_monster_health_percent(monster.hp, monster.max_hp),
                        expire: 0,
                    },
                },
            ));
        }
        let mut outbounds = Vec::new();
        for (position, packet) in packets {
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now_ms);
            let object_id = match &packet {
                ServerPacket::ObjectHealth { info } => info.object_id,
                _ => unreachable!(),
            };
            let recipients = self.native_monster_visible_recipients(object_id, &position);
            if !recipients.is_empty() {
                outbounds.push(ZoneOutbound::ToMany {
                    session_ids: recipients,
                    packets: vec![packet],
                });
            }
        }
        outbounds
    }

    /// MonsterObject.PoisonStopRegen: each actual Green/Bleeding damage tick
    /// postpones regen. Ordinary melee/magic strikes do not postpone it.
    pub(super) fn conquest_delay_archer_regen(&mut self, object_id: u32, now_ms: u64) {
        if let Some(binding) = self.conquest.as_mut().and_then(|p| {
            p.defenses
                .values_mut()
                .find(|b| b.object_id == object_id && b.source.kind == ConquestDefenseKind::Archer)
        }) {
            binding.next_regen_at_ms = now_ms.saturating_add(ARCHER_REGEN_DELAY_MS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conquest::sabuk_policy;
    use crate::runtime::zone::types::{ZoneChatProfile, ZonePlayerCombatStats};
    const OWNER: &str = "11111111111111111111111111111111";
    const ATTACKER: &str = "22222222222222222222222222222222";
    const THIRD: &str = "33333333333333333333333333333333";
    const EPOCH: u64 = 1_000_000;
    const ZONE_NOW: u64 = 1_000;

    fn record(war: bool) -> SharedConquestRecord {
        let mut record = SharedConquestRecord::new(1);
        record.defenses = default_sabuk_defenses();
        record.owner_guild_id = Some(OWNER.into());
        record.attacker_guild_id = Some(ATTACKER.into());
        record.war_active = war;
        record.battle_day = war.then_some(0);
        record.war_ends_ms = EPOCH + 100_000;
        record
    }
    fn join(
        zone: &mut ZoneRuntime,
        name: &str,
        id: u32,
        position: Point,
        guild: Option<&str>,
    ) -> ZoneConquestMembership {
        let session_id = SessionId::new(name);
        zone.handle(ZoneCommand::Join(ZoneJoin {
            session_id: session_id.clone(),
            account_id: format!("account-{name}"),
            character_index: 7,
            object_id: id,
            name: name.into(),
            class: MirClass::Warrior,
            gender: mir2_protocol::MirGender::Male,
            level: 30,
            hp: 1_000,
            max_hp: 1_000,
            mp: 100,
            map_file_name: zone.key.map_file_name.clone(),
            position,
            direction: MirDirection::Up,
            chat_profile: ZoneChatProfile {
                attack_mode: 3,
                ..Default::default()
            },
            combat_stats: ZonePlayerCombatStats::default(),
        }));
        ZoneConquestMembership {
            session_id,
            account_id: format!("account-{name}"),
            character_index: 7,
            guild_id: guild.map(str::to_string),
        }
    }
    fn zone() -> ZoneRuntime {
        ZoneRuntime::new_with_collision(ZoneKey::for_map("3"), ZoneCollision::unbounded())
    }
    fn apply(
        zone: &mut ZoneRuntime,
        record: &SharedConquestRecord,
        members: &[ZoneConquestMembership],
    ) {
        zone.apply_conquest_projection(&sabuk_policy(), record, members, EPOCH, ZONE_NOW)
            .unwrap();
    }
    fn defense_id(zone: &ZoneRuntime, key: &str) -> u32 {
        zone.conquest.as_ref().unwrap().defenses[key].object_id
    }

    #[test]
    fn exact_source_slots_have_independent_lifetimes_and_no_normal_rewards_or_respawn() {
        let mut zone = zone();
        apply(&mut zone, &record(false), &[]);
        assert_eq!(zone.conquest_defense_samples().len(), 16);
        assert_eq!(zone.native_monster_count(), 16);
        let a = defense_id(&zone, "archer:10");
        let b = defense_id(&zone, "archer:12");
        assert_ne!(a, b);
        assert_eq!(zone.native_monsters[&a].position, Point { x: 671, y: 334 });
        assert_eq!(zone.native_monsters[&b].position, Point { x: 671, y: 334 });
        assert!(zone
            .native_monsters
            .values()
            .all(|m| m.experience == 0 && m.drops.is_empty()));
        assert!(zone.native_monster_respawns.is_empty());
        assert!(zone.native_monsters.values().all(|m| !m.hostile_to_player));
    }

    #[test]
    fn ordinary_gates_are_not_enabled_by_the_sabuk_projection() {
        let mut zone = zone();
        apply(&mut zone, &record(true), &[]);
        assert!(!zone.conquest_is_defense(u32::MAX));
        assert!(!zone.conquest_gate_is_open(u32::MAX));
    }

    #[test]
    fn projection_cannot_restore_pending_damage_but_new_committed_repair_can() {
        let mut zone = zone();
        let mut state = record(true);
        apply(&mut zone, &state, &[]);
        let gate = defense_id(&zone, "gate:1");
        zone.native_monsters.get_mut(&gate).unwrap().hp = 4_300;
        state.revision += 1; // An unrelated lease/event revision.
        apply(&mut zone, &state, &[]);
        assert_eq!(zone.native_monsters[&gate].hp, 4_300);
        let sample = zone
            .conquest_defense_samples()
            .into_iter()
            .find(|s| s.key == "gate:1")
            .unwrap();
        assert_eq!(
            (sample.hp, sample.committed_hp, sample.record_revision),
            (4_300, 5_000, state.revision)
        );
        state.defenses.get_mut("gate:1").unwrap().hp = 4_300;
        state.revision += 1;
        apply(&mut zone, &state, &[]);
        state.repair("gate:1", &mut 5_000).unwrap();
        apply(&mut zone, &state, &[]);
        assert_eq!(zone.native_monsters[&gate].hp, 5_000);
    }

    #[test]
    fn corrupt_or_stale_record_rejected_before_mutating_zone() {
        let mut zone = zone();
        let state = record(true);
        apply(&mut zone, &state, &[]);
        let before = zone.conquest_defense_samples();
        let mut invalid = state.clone();
        invalid.defenses.get_mut("gate:1").unwrap().x += 1;
        assert!(zone
            .apply_conquest_projection(&sabuk_policy(), &invalid, &[], EPOCH, ZONE_NOW)
            .is_err());
        let mut conflicting = state.clone();
        conflicting.defenses.get_mut("gate:1").unwrap().hp -= 1;
        assert!(zone
            .apply_conquest_projection(&sabuk_policy(), &conflicting, &[], EPOCH, ZONE_NOW)
            .is_err());
        assert!(zone
            .apply_conquest_projection(&sabuk_policy(), &state, &[], EPOCH - 1, ZONE_NOW)
            .is_err());
        assert_eq!(zone.conquest_defense_samples(), before);
    }

    #[test]
    fn gate_open_close_and_dead_state_retain_exact_dynamic_footprint() {
        let mut zone = zone();
        let mut state = record(false);
        apply(&mut zone, &state, &[]);
        let gate = defense_id(&zone, "gate:1");
        let footprint = Point { x: 672, y: 329 };
        assert!(zone.gate_blocks_tile(&footprint));
        state.set_gate_open("gate:1", true).unwrap();
        apply(&mut zone, &state, &[]);
        assert!(zone.conquest_gate_is_open(gate));
        assert_eq!(zone.native_monsters[&gate].direction, MirDirection::Left);
        assert!(!zone.gate_blocks_tile(&footprint));
        assert!(zone.can_player_movement_occupy(&Point { x: 672, y: 330 }, None));
        state.set_gate_open("gate:1", false).unwrap();
        apply(&mut zone, &state, &[]);
        assert!(zone.gate_blocks_tile(&footprint));
        state.defenses.get_mut("gate:1").unwrap().hp = 0;
        state.revision += 1;
        apply(&mut zone, &state, &[]);
        assert!(!zone.gate_blocks_tile(&footprint));
        assert!(zone.can_player_movement_occupy(&Point { x: 672, y: 330 }, None));
        state.repair("gate:1", &mut 1_000).unwrap();
        apply(&mut zone, &state, &[]);
        assert!(!zone.conquest_gate_is_open(gate));
        assert!(zone.gate_blocks_tile(&footprint));
    }

    #[test]
    fn source_war_region_includes_missing_gate_and_palace_without_global_pk() {
        let mut zone = zone();
        let a = join(
            &mut zone,
            "owner",
            100,
            Point { x: 672, y: 330 },
            Some(OWNER),
        );
        let b = join(
            &mut zone,
            "challenger",
            101,
            Point { x: 673, y: 330 },
            Some(ATTACKER),
        );
        apply(&mut zone, &record(true), &[a.clone(), b.clone()]);
        let owner = &zone.players[&a.session_id];
        let challenger = &zone.players[&b.session_id];
        assert!(zone.conquest_player_is_in_war_region(owner, ZONE_NOW));
        assert!(zone.conquest_players_are_war_enemies(owner, challenger, ZONE_NOW));
        assert!(zone.conquest_player_can_attack_player(owner, challenger, ZONE_NOW));
        zone.players
            .get_mut(&a.session_id)
            .unwrap()
            .chat_profile
            .attack_mode = 0;
        assert!(!zone.conquest_player_can_attack_player(
            &zone.players[&a.session_id],
            &zone.players[&b.session_id],
            ZONE_NOW
        ));
        zone.players.get_mut(&a.session_id).unwrap().position = Point { x: 100, y: 100 };
        assert!(!zone.conquest_players_are_war_enemies(
            &zone.players[&a.session_id],
            &zone.players[&b.session_id],
            ZONE_NOW
        ));
        let mut palace =
            ZoneRuntime::new_with_collision(ZoneKey::for_map("0150"), ZoneCollision::unbounded());
        let member = join(
            &mut palace,
            "palace",
            200,
            Point { x: 9, y: 23 },
            Some(ATTACKER),
        );
        apply(&mut palace, &record(true), &[member.clone()]);
        assert!(
            palace.conquest_player_is_in_war_region(&palace.players[&member.session_id], ZONE_NOW)
        );
        assert!(palace.conquest_defense_samples().is_empty());
    }

    #[test]
    fn defense_damage_requires_live_matching_membership_war_and_non_owner() {
        let mut zone = zone();
        let owner = join(
            &mut zone,
            "owner",
            100,
            Point { x: 600, y: 300 },
            Some(OWNER),
        );
        let attacker = join(
            &mut zone,
            "attacker",
            101,
            Point { x: 601, y: 300 },
            Some(ATTACKER),
        );
        let third = join(
            &mut zone,
            "third",
            102,
            Point { x: 602, y: 300 },
            Some(THIRD),
        );
        let unguilded = join(&mut zone, "unguilded", 103, Point { x: 603, y: 300 }, None);
        let mut state = record(true);
        apply(
            &mut zone,
            &state,
            &[
                owner.clone(),
                attacker.clone(),
                third.clone(),
                unguilded.clone(),
            ],
        );
        let gate = defense_id(&zone, "gate:1");
        assert!(!zone.conquest_defense_accepts_damage(gate, Some(&owner.session_id), ZONE_NOW));
        for m in [&attacker, &third, &unguilded] {
            assert!(zone.conquest_defense_accepts_damage(gate, Some(&m.session_id), ZONE_NOW));
        }
        assert!(!zone.conquest_defense_accepts_damage(gate, None, ZONE_NOW));
        assert!(!zone.conquest_defense_accepts_damage(
            gate,
            Some(&attacker.session_id),
            ZONE_NOW + 100_000
        ));
        let mut wrong_identity = attacker.clone();
        wrong_identity.character_index += 1;
        apply(&mut zone, &state, &[wrong_identity]);
        assert!(!zone.conquest_defense_accepts_damage(gate, Some(&attacker.session_id), ZONE_NOW));
        state.war_active = false;
        state.revision += 1;
        apply(&mut zone, &state, &[attacker.clone()]);
        assert!(!zone.conquest_defense_accepts_damage(gate, Some(&attacker.session_id), ZONE_NOW));
    }

    #[test]
    fn archer_targets_all_nonowners_and_impact_rechecks_capture_and_deadline() {
        let mut zone = zone();
        let owner = join(
            &mut zone,
            "owner",
            100,
            Point { x: 662, y: 334 },
            Some(OWNER),
        );
        let enemy = join(
            &mut zone,
            "enemy",
            101,
            Point { x: 663, y: 333 },
            Some(ATTACKER),
        );
        let mut state = record(true);
        apply(&mut zone, &state, &[owner.clone(), enemy.clone()]);
        let archer = defense_id(&zone, "archer:1");
        assert!(!zone.conquest_archer_hit_allowed(archer, &owner.session_id, 100, ZONE_NOW));
        assert!(zone.conquest_archer_hit_allowed(archer, &enemy.session_id, 101, ZONE_NOW));
        assert!(!zone.conquest_archer_hit_allowed(archer, &enemy.session_id, 999, ZONE_NOW));
        assert!(!zone.conquest_archer_hit_allowed(
            archer,
            &enemy.session_id,
            101,
            ZONE_NOW + 100_000
        ));
        assert!(zone
            .try_tick_conquest_defense(archer, ZONE_NOW + 2_001)
            .unwrap()
            .is_empty());
        let launched = zone
            .try_tick_conquest_defense(archer, ZONE_NOW + 2_301)
            .unwrap();
        assert!(!launched.is_empty());
        let hit = zone.pending_native_player_hits.last().unwrap();
        assert_eq!(hit.target_object_id, 101);
        assert_eq!(hit.ready_at_ms, ZONE_NOW + 2_301 + 550);
        state.owner_guild_id = Some(ATTACKER.into());
        state.attacker_guild_id = Some(OWNER.into());
        state.revision += 1;
        apply(&mut zone, &state, &[owner, enemy.clone()]);
        assert!(!zone.conquest_archer_hit_allowed(
            archer,
            &enemy.session_id,
            101,
            ZONE_NOW + 3_000
        ));
    }

    #[test]
    fn living_archers_regenerate_exact_source_amount_in_peace_and_dead_never_regenerate() {
        let mut zone = zone();
        let mut state = record(false);
        state.defenses.get_mut("archer:1").unwrap().hp = 9_000;
        state.defenses.get_mut("archer:2").unwrap().hp = 0;
        state.defenses.get_mut("gate:1").unwrap().hp = 4_000;
        state.defenses.get_mut("wall:1").unwrap().hp = 4_000;
        apply(&mut zone, &state, &[]);
        for binding in zone.conquest.as_mut().unwrap().defenses.values_mut() {
            binding.next_regen_at_ms = ZONE_NOW + 10_000;
        }
        zone.tick_conquest_archer_regen(ZONE_NOW + 9_999);
        assert_eq!(
            zone.native_monsters[&defense_id(&zone, "archer:1")].hp,
            9_000
        );
        zone.tick_conquest_archer_regen(ZONE_NOW + 10_000);
        assert_eq!(
            zone.native_monsters[&defense_id(&zone, "archer:1")].hp,
            9_220
        );
        assert_eq!(zone.native_monsters[&defense_id(&zone, "archer:2")].hp, 0);
        assert_eq!(zone.native_monsters[&defense_id(&zone, "gate:1")].hp, 4_000);
        assert_eq!(zone.native_monsters[&defense_id(&zone, "wall:1")].hp, 4_000);
        zone.conquest_delay_archer_regen(defense_id(&zone, "archer:1"), ZONE_NOW + 19_000);
        zone.tick_conquest_archer_regen(ZONE_NOW + 20_000);
        assert_eq!(
            zone.native_monsters[&defense_id(&zone, "archer:1")].hp,
            9_220
        );
        zone.tick_conquest_archer_regen(ZONE_NOW + 29_000);
        assert_eq!(
            zone.native_monsters[&defense_id(&zone, "archer:1")].hp,
            9_440
        );
    }

    #[test]
    fn transactional_fork_preserves_conquest_authority_and_deadline_cannot_extend() {
        let mut zone = zone();
        let state = record(true);
        apply(&mut zone, &state, &[]);
        let fork = zone.transaction_fork();
        assert_eq!(
            fork.conquest_defense_samples(),
            zone.conquest_defense_samples()
        );
        zone.apply_conquest_projection(
            &sabuk_policy(),
            &state,
            &[],
            EPOCH + 1_000,
            ZONE_NOW + 2_000,
        )
        .unwrap();
        assert_eq!(
            zone.conquest.as_ref().unwrap().war_deadline_zone_ms,
            ZONE_NOW + 100_000
        );
    }

    #[test]
    fn object_id_exhaustion_and_duplicate_membership_are_preflighted() {
        let mut zone = zone();
        zone.next_object_id = u32::MAX;
        assert!(zone
            .apply_conquest_projection(&sabuk_policy(), &record(false), &[], EPOCH, ZONE_NOW)
            .is_err());
        assert!(zone.conquest.is_none());
        assert!(zone.native_monsters.is_empty());
        let mut zone = self::zone();
        let member = join(
            &mut zone,
            "member",
            101,
            Point { x: 600, y: 300 },
            Some(ATTACKER),
        );
        assert!(zone
            .apply_conquest_projection(
                &sabuk_policy(),
                &record(true),
                &[member.clone(), member],
                EPOCH,
                ZONE_NOW
            )
            .is_err());
        assert!(zone.conquest.is_none());
        assert!(zone.native_monsters.is_empty());
    }

    #[test]
    fn archer_regen_is_not_undone_by_equal_projection_or_replayed_over_new_death() {
        let mut zone = zone();
        let mut state = record(true);
        state.defenses.get_mut("archer:1").unwrap().hp = 9_000;
        apply(&mut zone, &state, &[]);
        let id = defense_id(&zone, "archer:1");
        zone.conquest
            .as_mut()
            .unwrap()
            .defenses
            .get_mut("archer:1")
            .unwrap()
            .next_regen_at_ms = ZONE_NOW;
        zone.tick_conquest_archer_regen(ZONE_NOW);
        assert_eq!(zone.native_monsters[&id].hp, 9_220);
        apply(&mut zone, &state, &[]);
        assert_eq!(zone.native_monsters[&id].hp, 9_220);
        zone.native_monsters.get_mut(&id).unwrap().hp = 0;
        zone.native_monsters.get_mut(&id).unwrap().dead = true;
        // A sample committed before that later death cannot revive the object.
        state.defenses.get_mut("archer:1").unwrap().hp = 9_220;
        state.revision += 1;
        apply(&mut zone, &state, &[]);
        assert!(zone.native_monsters[&id].dead);
        state.defenses.get_mut("archer:1").unwrap().hp = 0;
        state.revision += 1;
        apply(&mut zone, &state, &[]);
        state.repair("archer:1", &mut 1_000).unwrap();
        apply(&mut zone, &state, &[]);
        assert_eq!(zone.native_monsters[&id].hp, 9_999);
        assert!(!zone.native_monsters[&id].dead);
    }

    #[test]
    fn restored_source_defenses_stay_inert_until_committed_authority_is_rehydrated() {
        let mut original = ZoneRuntime::new(ZoneKey::for_map("3"));
        let owner = join(
            &mut original,
            "owner",
            100,
            Point { x: 662, y: 334 },
            Some(OWNER),
        );
        apply(&mut original, &record(true), &[owner.clone()]);
        let guard = defense_id(&original, "archer:1");
        // An arrow from the checkpoint's old battle cannot authenticate its own
        // restored authorization. It is discarded until the shared record binds
        // the actual player's guild again.
        original
            .pending_native_player_hits
            .push(PendingNativePlayerHit {
                ready_at_ms: ZONE_NOW,
                attacker_object_id: guard,
                attacker_ai: 80,
                target_session_id: owner.session_id.clone(),
                target_object_id: 100,
                damage: 999,
                magic: false,
            });
        let mut restored = ZoneRuntime::restore_checkpoint(&original.checkpoint_bytes().unwrap())
            .expect("restore source world actors");
        assert!(restored.conquest.is_none());
        let before: Vec<_> = restored
            .native_monsters
            .iter()
            .map(|(id, m)| (*id, m.position.clone(), m.hp))
            .collect();
        for (id, _, _) in &before {
            assert!(restored.conquest_is_defense(*id));
            assert!(!restored.conquest_defense_accepts_damage(
                *id,
                Some(&owner.session_id),
                ZONE_NOW
            ));
            assert!(restored
                .tick_native_monster(*id, ZONE_NOW + 5_000)
                .is_empty());
        }
        assert!(restored.conquest_is_archer(guard));
        assert!(restored
            .resolve_pending_native_player_hits(ZONE_NOW + 5_000)
            .is_empty());
        assert_eq!(restored.players[&owner.session_id].hp, 1_000);
        assert!(restored
            .apply_native_monster_damage(guard, 500, Some(&owner.session_id), ZONE_NOW)
            .is_none());
        assert_eq!(
            restored
                .native_monsters
                .iter()
                .map(|(id, m)| (*id, m.position.clone(), m.hp))
                .collect::<Vec<_>>(),
            before
        );
        apply(&mut restored, &record(true), &[owner.clone()]);
        assert!(!restored.conquest_archer_hit_allowed(guard, &owner.session_id, 100, ZONE_NOW));
    }

    #[test]
    fn quarantine_matches_source_identity_without_disabling_ordinary_guards_or_pets() {
        let mut original = ZoneRuntime::new(ZoneKey::for_map("3"));
        apply(&mut original, &record(true), &[]);
        let guard = defense_id(&original, "archer:1");
        let restored =
            ZoneRuntime::restore_checkpoint(&original.checkpoint_bytes().unwrap()).unwrap();
        assert!(restored.conquest_is_archer(guard));
        for field in [
            "map", "channel", "name", "ai", "image", "position", "max_hp", "owner",
        ] {
            let mut candidate = restored.transaction_fork();
            match field {
                "map" => candidate.key = ZoneKey::for_map("0"),
                "channel" => candidate.key.channel_id = 1,
                "name" => {
                    candidate.native_monsters.get_mut(&guard).unwrap().name = "ArcherGuard".into()
                }
                "ai" => candidate.native_monsters.get_mut(&guard).unwrap().ai = 57,
                "image" => {
                    let ServerPacket::ObjectMonster { info } =
                        &mut candidate.objects.get_mut(&guard).unwrap().packet
                    else {
                        panic!("monster packet")
                    };
                    info.image += 1;
                }
                "position" => {
                    candidate
                        .native_monsters
                        .get_mut(&guard)
                        .unwrap()
                        .position
                        .x += 1
                }
                "max_hp" => candidate.native_monsters.get_mut(&guard).unwrap().max_hp -= 1,
                "owner" => {
                    candidate
                        .native_monsters
                        .get_mut(&guard)
                        .unwrap()
                        .owner_session_id = Some(SessionId::new("pet-owner"))
                }
                _ => unreachable!(),
            }
            assert!(
                !candidate.conquest_is_defense(guard),
                "ordinary {field} must retain its own AI rules"
            );
            assert!(!candidate.conquest_is_archer(guard), "ordinary {field}");
            assert!(
                candidate
                    .try_tick_conquest_defense(guard, ZONE_NOW)
                    .is_none(),
                "ordinary {field}"
            );
            assert!(
                candidate.conquest_defense_accepts_damage(guard, None, ZONE_NOW),
                "ordinary {field}"
            );
        }
    }

    #[test]
    fn siege_pk_exemption_uses_victim_region_at_damage_time_for_every_attack_mode() {
        let mut zone = zone();
        let attacker = join(
            &mut zone,
            "attacker",
            100,
            Point { x: 600, y: 335 },
            Some(ATTACKER),
        );
        let victim = join(&mut zone, "victim", 101, Point { x: 670, y: 335 }, None);
        apply(
            &mut zone,
            &record(true),
            &[attacker.clone(), victim.clone()],
        );
        for mode in 0..=5 {
            zone.players
                .get_mut(&attacker.session_id)
                .unwrap()
                .chat_profile
                .attack_mode = mode;
            assert!(
                zone.conquest_player_kill_is_lawful(&attacker.session_id, 100, 101, ZONE_NOW),
                "mode {mode}"
            );
        }
        // The damage admission helper decides whether a swing may occur. This
        // settlement lookup must still work immediately after the victim dies.
        let target = zone.players.get_mut(&victim.session_id).unwrap();
        target.hp = 0;
        target.dead = true;
        assert!(zone.conquest_player_kill_is_lawful(&attacker.session_id, 100, 101, ZONE_NOW));
        assert!(!zone.conquest_player_kill_is_lawful(
            &attacker.session_id,
            100,
            101,
            ZONE_NOW + 100_000
        ));
        zone.players.get_mut(&victim.session_id).unwrap().position = Point { x: 600, y: 334 };
        assert!(!zone.conquest_player_kill_is_lawful(&attacker.session_id, 100, 101, ZONE_NOW));
    }

    #[test]
    fn palace_pk_exemption_requires_real_actors_and_active_deadline() {
        let mut palace =
            ZoneRuntime::new_with_collision(ZoneKey::for_map("0150"), ZoneCollision::unbounded());
        let attacker = join(
            &mut palace,
            "attacker",
            100,
            Point { x: 1, y: 1 },
            Some(ATTACKER),
        );
        let victim = join(
            &mut palace,
            "third",
            101,
            Point { x: 20, y: 23 },
            Some(THIRD),
        );
        apply(&mut palace, &record(true), &[attacker.clone(), victim]);
        assert!(palace.conquest_player_kill_is_lawful(&attacker.session_id, 100, 101, ZONE_NOW));
        assert!(!palace.conquest_player_kill_is_lawful(&attacker.session_id, 999, 101, ZONE_NOW));
        assert!(!palace.conquest_player_kill_is_lawful(&attacker.session_id, 100, 999, ZONE_NOW));
        assert!(!palace.conquest_player_kill_is_lawful(&attacker.session_id, 100, 100, ZONE_NOW));
        assert!(!palace.conquest_player_kill_is_lawful(
            &SessionId::new("stale"),
            100,
            101,
            ZONE_NOW
        ));
        assert!(!palace.conquest_player_kill_is_lawful(
            &attacker.session_id,
            100,
            101,
            ZONE_NOW + 100_000
        ));
        // Naming the attacker guild in a stale personal attack mode does not
        // create a permanent war or immunity after the battle has ended.
        palace
            .players
            .get_mut(&attacker.session_id)
            .unwrap()
            .chat_profile
            .attack_mode = 3;
        assert!(!palace.conquest_player_kill_is_lawful(
            &attacker.session_id,
            100,
            101,
            ZONE_NOW + 100_000
        ));
    }

    #[test]
    fn ordinary_guild_war_pk_exemption_is_independent_of_attack_mode() {
        let mut zone =
            ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
        let attacker = join(&mut zone, "attacker", 100, Point { x: 10, y: 10 }, None);
        let victim = join(&mut zone, "victim", 101, Point { x: 11, y: 10 }, None);
        zone.players
            .get_mut(&attacker.session_id)
            .unwrap()
            .chat_profile
            .guild_name = Some("Challengers".into());
        let target = zone.players.get_mut(&victim.session_id).unwrap();
        target.chat_profile.guild_name = Some("Defenders".into());
        target.chat_profile.active_guild_wars = vec!["challengers".into()];
        target.hp = 0;
        target.dead = true;
        for mode in 0..=5 {
            zone.players
                .get_mut(&attacker.session_id)
                .unwrap()
                .chat_profile
                .attack_mode = mode;
            assert!(zone.conquest_player_kill_is_lawful(&attacker.session_id, 100, 101, ZONE_NOW));
        }
        zone.players
            .get_mut(&victim.session_id)
            .unwrap()
            .chat_profile
            .guild_name = None;
        assert!(!zone.conquest_player_kill_is_lawful(&attacker.session_id, 100, 101, ZONE_NOW));
    }

    #[test]
    fn final_interval_damage_keeps_trusted_provenance_but_cutoff_rejects_new_damage() {
        let mut zone = zone();
        let attacker = join(
            &mut zone,
            "attacker",
            100,
            Point { x: 600, y: 300 },
            Some(ATTACKER),
        );
        let mut state = record(true);
        state.clock_generation = 1;
        apply(&mut zone, &state, &[attacker.clone()]);
        let gate = defense_id(&zone, "gate:1");
        assert!(
            !zone
                .conquest_defense_samples()
                .iter()
                .find(|s| s.key == "gate:1")
                .unwrap()
                .damage_was_admitted_before_cutoff
        );
        let admitted_at = ZONE_NOW + 99_999;
        assert!(zone
            .apply_native_monster_damage(gate, 125, Some(&attacker.session_id), admitted_at)
            .is_some());
        let sample = zone
            .conquest_defense_samples()
            .into_iter()
            .find(|s| s.key == "gate:1")
            .unwrap();
        assert_eq!((sample.hp, sample.committed_hp), (4_875, 5_000));
        assert_eq!(
            (
                sample.battle_day,
                sample.war_ends_ms,
                sample.clock_generation
            ),
            (state.battle_day, state.war_ends_ms, 1)
        );
        assert!(sample.damage_was_admitted_before_cutoff);
        let at_cutoff = ZONE_NOW + 100_000;
        assert!(zone
            .apply_native_monster_damage(gate, 500, Some(&attacker.session_id), at_cutoff)
            .is_none());
        assert!(!zone.record_conquest_damage_admitted(gate, at_cutoff));
        assert_eq!(zone.native_monsters[&gate].hp, 4_875);
        // Polling after cutoff must still be able to flush the earlier accepted
        // HP change. No mutation or clock reread fabricates this capability.
        let final_sample = zone
            .conquest_defense_samples()
            .into_iter()
            .find(|s| s.key == "gate:1")
            .unwrap();
        assert!(final_sample.damage_was_admitted_before_cutoff);
    }

    #[test]
    fn pending_damage_provenance_survives_tax_revision_and_commit_ack_but_not_repair() {
        let mut zone = zone();
        let attacker = join(
            &mut zone,
            "attacker",
            100,
            Point { x: 600, y: 300 },
            Some(ATTACKER),
        );
        let mut state = record(true);
        state.clock_generation = 1;
        apply(&mut zone, &state, &[attacker.clone()]);
        let gate = defense_id(&zone, "gate:1");
        assert!(zone
            .apply_native_monster_damage(gate, 125, Some(&attacker.session_id), ZONE_NOW + 1)
            .is_some());
        state.gold += 1; // Ordinary merchant tax is an unrelated castle revision.
        state.revision += 1;
        apply(&mut zone, &state, &[attacker.clone()]);
        let pending = zone
            .conquest_defense_samples()
            .into_iter()
            .find(|s| s.key == "gate:1")
            .unwrap();
        assert_eq!(
            (pending.hp, pending.committed_hp, pending.record_revision),
            (4_875, 5_000, state.revision)
        );
        assert!(pending.damage_was_admitted_before_cutoff);
        state.defenses.get_mut("gate:1").unwrap().hp = pending.hp;
        state.revision += 1;
        apply(&mut zone, &state, &[attacker.clone()]);
        let acknowledged = zone
            .conquest_defense_samples()
            .into_iter()
            .find(|s| s.key == "gate:1")
            .unwrap();
        assert_eq!((acknowledged.hp, acknowledged.committed_hp), (4_875, 4_875));
        assert!(acknowledged.damage_was_admitted_before_cutoff);
        state.repair("gate:1", &mut 1_000).unwrap();
        apply(&mut zone, &state, &[attacker]);
        let repaired = zone
            .conquest_defense_samples()
            .into_iter()
            .find(|s| s.key == "gate:1")
            .unwrap();
        assert_eq!((repaired.hp, repaired.committed_hp), (5_000, 5_000));
        assert!(!repaired.damage_was_admitted_before_cutoff);
    }

    #[test]
    fn damage_provenance_does_not_cross_a_new_clock_generation_or_battle() {
        let mut zone = zone();
        let attacker = join(
            &mut zone,
            "attacker",
            100,
            Point { x: 600, y: 300 },
            Some(ATTACKER),
        );
        let mut state = record(true);
        state.clock_generation = 1;
        apply(&mut zone, &state, &[attacker.clone()]);
        let gate = defense_id(&zone, "gate:1");
        assert!(zone.record_conquest_damage_admitted(gate, ZONE_NOW + 1));
        state.clock_generation += 1;
        state.revision += 1;
        apply(&mut zone, &state, &[attacker.clone()]);
        assert!(
            !zone
                .conquest_defense_samples()
                .iter()
                .find(|s| s.key == "gate:1")
                .unwrap()
                .damage_was_admitted_before_cutoff
        );
        assert!(zone.record_conquest_damage_admitted(gate, ZONE_NOW + 1));
        state.battle_day = Some(1);
        state.war_ends_ms += 86_400_000;
        state.revision += 1;
        apply(&mut zone, &state, &[attacker]);
        assert!(
            !zone
                .conquest_defense_samples()
                .iter()
                .find(|s| s.key == "gate:1")
                .unwrap()
                .damage_was_admitted_before_cutoff
        );
        zone.conquest = None;
        assert!(!zone.record_conquest_damage_admitted(gate, ZONE_NOW));
        assert!(zone.conquest_defense_samples().is_empty());
    }
}
