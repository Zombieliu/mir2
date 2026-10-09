//! Crystal PlayerObject.IsAttackTarget(MonsterObject):4709 and
//! MonsterObject.IsAttackTarget(MonsterObject):2465. Owned combat has its own
//! authority; the wild entity resolver must continue rejecting owned sources.
use super::entity_combat::{
    EntityDefence, EntityTargetPurpose, NativeEntityTarget, ZoneCombatEntityRef,
};
use super::*;
use crate::runtime::zone::online_identity::OnlineOwner;
use crate::runtime::zone::types::ZoneOwnedPetPlayerKillReceipt;
use rand_core::{OsRng, RngCore};

const LAST_HITTER_MS: u64 = 10_000;
const BROWN_MS: u64 = 60_000;
const PET_DATA_RANGE: i32 = 16;

/// Crystal CharacterInfo.PetInfo, deliberately excluding online IDs, epochs,
/// targets and queued actions. Only trusted character loading may restore it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneSavedPetSnapshot {
    #[serde(default)]
    pets: Vec<SavedPetInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedPetInfo {
    monster_index: i32,
    hp: i32,
    experience: u32,
    level: u8,
    max_pet_level: u8,
    /// Wizard logout/login keeps remaining TameTime; no wall-clock deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tame_remaining_ms: Option<u64>,
}

impl ZoneSavedPetSnapshot {
    pub fn is_empty(&self) -> bool {
        self.pets.is_empty()
    }

    pub fn validate_canonical(&self) -> Result<(), String> {
        self.validate()
    }

    /// Crystal's binary PetInfo keeps five fields; Wizard's remaining tame
    /// time belongs to the same server process's warm character cache only.
    pub fn for_durable_storage(&self) -> Self {
        let mut durable = self.clone();
        for pet in &mut durable.pets {
            pet.tame_remaining_ms = None;
        }
        durable
    }

    pub fn without_tame_time(&self) -> Self {
        self.for_durable_storage()
    }

    pub(crate) fn append_other_zone(&mut self, other: Self) {
        self.pets.extend(other.pets);
    }

    fn validate(&self) -> Result<(), String> {
        if self.pets.len() > 64 {
            return Err("saved pet count exceeds trusted snapshot limit".into());
        }
        for pet in &self.pets {
            let template =
                mir2_game_data::crystal_monster_by_index(pet.monster_index).ok_or_else(|| {
                    format!("saved pet canonical monster {} missing", pet.monster_index)
                })?;
            if pet.hp <= 0
                || pet.level > 7
                || pet.max_pet_level > 7
                || pet.hp > template.hp.max(1).saturating_add(i32::from(pet.level) * 20)
            {
                return Err("saved pet HP/level exceeds canonical source state".into());
            }
        }
        Ok(())
    }
}

/// A fresh, server-only GainExp admission. Range and typed lives are never
/// recovered from a persisted PetInfo or admitted from a client packet.
#[derive(Debug, Clone)]
pub struct ZonePetExperienceAdmission {
    owner: OwnedPetOwner,
    zone: ZoneKey,
    pets: Vec<PetExperienceEntry>,
}

/// A same-online-Node map recall. It cannot be serialized or constructed by
/// callers, and never substitutes for a canonical durable PetInfo.
#[derive(Debug, Clone)]
pub(crate) struct ZoneOnlinePetRecall {
    destination: ZoneKey,
    life: OwnedPetLife,
    monster: ZoneNativeMonster,
    object: super::super::types::ZoneObject,
    clock: Option<OwnedMonsterClock>,
    poisons: Vec<OwnedHumanPoison>,
}

impl ZoneOnlinePetRecall {
    pub(crate) fn destination(&self) -> &ZoneKey {
        &self.destination
    }
}

#[derive(Debug, Clone)]
struct PetExperienceEntry {
    life: OwnedPetLife,
    zone: ZoneKey,
    before: SavedPetInfo,
    eligible: bool,
    saveable: bool,
}

impl ZonePetExperienceAdmission {
    pub fn is_empty(&self) -> bool {
        self.pets.is_empty()
    }

    pub(crate) fn append_other_zone(&mut self, mut other: Self) -> bool {
        if self.owner != other.owner || self.zone == other.zone {
            return false;
        }
        for entry in &mut other.pets {
            entry.eligible = false;
        }
        self.pets.extend(other.pets);
        true
    }
    pub fn before_snapshot(&self) -> ZoneSavedPetSnapshot {
        ZoneSavedPetSnapshot {
            pets: self.pets.iter().map(|p| p.before.clone()).collect(),
        }
    }

    pub fn before_saved_snapshot(&self) -> ZoneSavedPetSnapshot {
        ZoneSavedPetSnapshot {
            pets: self
                .pets
                .iter()
                .filter(|p| p.saveable)
                .map(|p| p.before.clone())
                .collect(),
        }
    }

    pub fn saved_after_earned_steps(
        &self,
        amounts: &[u32],
    ) -> Result<ZoneSavedPetSnapshot, String> {
        let after = self.apply_earned_steps(amounts)?;
        Ok(ZoneSavedPetSnapshot {
            pets: after
                .pets
                .into_iter()
                .zip(&self.pets)
                .filter_map(|(pet, admission)| admission.saveable.then_some(pet))
                .collect(),
        })
    }

    /// Apply the actual, already-scaled PlayerObject.GainExp amount inside the
    /// caller's existing durable source CAS. PetExp performs one level step.
    pub fn apply_earned_experience(&self, amount: u32) -> Result<ZoneSavedPetSnapshot, String> {
        self.apply_earned_steps(&[amount])
    }

    /// Keep each real GainExp invocation separate: even a large amount only
    /// advances one pet level per invocation in MonsterObject.PetExp.
    pub fn apply_earned_steps(&self, amounts: &[u32]) -> Result<ZoneSavedPetSnapshot, String> {
        let mut after = self.before_snapshot();
        after.validate()?;
        for &amount in amounts {
            // PlayerObject.GainExp returns before visiting Pets for zero.
            if amount == 0 {
                continue;
            }
            for (pet, admission) in after.pets.iter_mut().zip(&self.pets) {
                if !admission.eligible || pet.level >= pet.max_pet_level {
                    continue;
                }
                let template = mir2_game_data::crystal_monster_by_index(pet.monster_index).unwrap();
                let earned = if source_taoist_pet_name(&template.name) {
                    amount.wrapping_mul(3)
                } else {
                    amount
                };
                pet.experience = pet.experience.wrapping_add(earned);
                let threshold = (u32::from(pet.level) + 1) * 20_000;
                if pet.experience >= threshold {
                    pet.experience -= threshold;
                    pet.level += 1;
                }
            }
        }
        Ok(after)
    }
}

fn source_taoist_pet_name(name: &str) -> bool {
    matches!(name, "BoneFamiliar" | "Shinsu" | "HolyDeva")
}

fn source_initial_max_pet_level(monster: &ZoneNativeMonster) -> u8 {
    if monster.name == "BoneFamiliar" {
        4 + monster.summon_skill_level.min(3)
    } else {
        1 + monster.summon_skill_level.min(3) * 2
    }
}

fn source_pet_colour(level: u8) -> i32 {
    match level {
        1 => 0xFF00FFFF_u32 as i32,
        2 => 0xFF7FFFD4_u32 as i32,
        3 => 0xFF20B2AA_u32 as i32,
        4 => 0xFF6A5ACD_u32 as i32,
        5 => 0xFF4682B4_u32 as i32,
        6 => 0xFF0000FF_u32 as i32,
        7 => 0xFF000080_u32 as i32,
        _ => -1,
    }
}

pub(super) fn owned_pet_dead_delay_ms(ai: u8) -> u64 {
    // Crystal MonsterObject.DeadDelay; Process removes at >= DeadTime.
    match ai {
        64 => 0,
        81 | 82 => i32::MAX as u64,
        252 => 5_000,
        _ => 180_000,
    }
}

fn owned_pet_zero(value: &u64) -> bool {
    *value == 0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct OwnedPetOwner {
    player: ZoneCombatEntityRef,
    pub(super) online: OnlineOwner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OwnedPetLife {
    owner: OwnedPetOwner,
    pet: ZoneCombatEntityRef,
    /// The causal Human authenticates this chain; it does not replace the
    /// CharmedSnake's actual immediate SnakeTotem Master.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    monster_master: Option<ZoneCombatEntityRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedPetTarget {
    life: OwnedPetLife,
    target: ZoneCombatEntityRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedPetThreatActor {
    entity: ZoneCombatEntityRef,
    online: Option<OnlineOwner>,
    zone: ZoneKey,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    player_source: Option<OwnedPetPlayerDamageSource>,
}

/// The actual positive HP mutation fixes the causal Human and optional pet.
/// Old clocks without this evidence can still drive guard AI, never PK billing.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedPetPlayerDamageSource {
    owner: OwnedPetOwner,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pet: Option<ZoneCombatEntityRef>,
}

/// Online map handoff state, never client-authored or a durable character mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OwnedPetPlayerClock {
    online: OnlineOwner,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_hitter: Option<OwnedPetThreatActor>,
    last_hit_until_ms: u64,
    brown_until_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedPetHit {
    life: OwnedPetLife,
    target: ZoneCombatEntityRef,
    target_online: Option<OnlineOwner>,
    damage: i32,
    defence: EntityDefence,
    due_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    journey_event: Option<ZoneJourneyEventReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct OwnedPetSummonTarget {
    entity: ZoneCombatEntityRef,
    online: Option<OnlineOwner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct OwnedPetHumanAuthority {
    attacker: OwnedPetOwner,
    victim: OwnedPetLife,
    pub(super) raw_damage: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedPetHumanHit {
    authority: OwnedPetHumanAuthority,
    hit: PendingNativeMonsterHit,
    defence: EntityDefence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedHumanPlayerSpell {
    caster: OwnedPetOwner,
    target: ZoneCombatEntityRef,
    target_online: OnlineOwner,
    target_position: Point,
    spell: Spell,
    level: u8,
    damage: i32,
    item_param: u8,
    due_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pet: Option<OwnedPetLife>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OwnedHumanPoison {
    caster: OwnedPetOwner,
    target: ZoneCombatEntityRef,
    target_online: OnlineOwner,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pet_owner: Option<OwnedPetOwner>,
    mask: u16,
    value: i32,
    ticks_left: u64,
    tick_speed_ms: u64,
    next_tick_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedHumanVampirism {
    caster: OwnedPetOwner,
    amount: u16,
    next_tick_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedHumanMonsterControl {
    caster: OwnedPetOwner,
    target: ZoneCombatEntityRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_owner: Option<OwnedPetOwner>,
    position: Point,
    spell: Spell,
    level: u8,
    due_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedMonsterClock {
    target: ZoneCombatEntityRef,
    shock_until_ms: u64,
    rage_until_ms: u64,
    binding_center: bool,
    tame_until_ms: u64,
    max_pet_level: u8,
    #[serde(default, skip_serializing_if = "owned_pet_u32_zero")]
    pet_experience: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    frozen_by_no_pets: bool,
    base_colour: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tame_original_disposition: Option<crate::config::WorldEntityDisposition>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    tame_original_hostile: bool,
}

fn owned_pet_u32_zero(value: &u32) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedPetWildTarget {
    source: ZoneCombatEntityRef,
    target: ZoneCombatEntityRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OwnedPetDeath {
    object_id: u32,
    position: Point,
    direction: MirDirection,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct OwnedPetCombatState {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    bindings: BTreeMap<u32, OwnedPetLife>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    targets: BTreeMap<u32, OwnedPetTarget>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    players: BTreeMap<SessionId, OwnedPetPlayerClock>,
    /// Metadata retained from a real departing Human. These are never Zone
    /// actors: no ECS entity, occupancy, AOI, actions or damage target is added.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    detached_masters: BTreeMap<SessionId, ZonePlayer>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    wild_targets: BTreeMap<u32, OwnedPetWildTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hits: Vec<OwnedPetHit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    human_hits: Vec<OwnedPetHumanHit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    player_spells: Vec<OwnedHumanPlayerSpell>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    poisons: Vec<OwnedHumanPoison>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    vampirism: Vec<OwnedHumanVampirism>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    monster_spells: Vec<OwnedHumanMonsterControl>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    monster_clocks: BTreeMap<u32, OwnedMonsterClock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    death_packets: Vec<OwnedPetDeath>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    issued_kills: BTreeMap<u64, ZoneOwnedPetPlayerKillReceipt>,
    #[serde(default, skip_serializing_if = "owned_pet_zero")]
    kill_sequence: u64,
    #[serde(default, skip_serializing_if = "owned_pet_zero")]
    last_now_ms: u64,
}

fn pet_mode_can_move(mode: u8) -> bool {
    matches!(mode, 0 | 1 | 4)
}
fn pet_mode_can_attack(mode: u8) -> bool {
    matches!(mode, 0 | 2 | 4)
}
fn pet_mode_can_search(mode: u8) -> bool {
    matches!(mode, 0 | 2)
}

impl ZoneRuntime {
    fn saved_pet_info(&self, id: u32, now: u64) -> Option<SavedPetInfo> {
        let monster = self.native_monsters.get(&id)?;
        let template = crystal_monster_by_name(&monster.name)?;
        let clock = self.owned_monster_clock(id);
        Some(SavedPetInfo {
            monster_index: template.monster_index,
            hp: monster.hp,
            experience: clock.map_or(0, |c| c.pet_experience),
            level: monster.summon_skill_level,
            // Legacy online checkpoints never recorded the originating magic
            // level. Do not invent MaxPetLevel from a later grown PetLevel.
            max_pet_level: clock.map_or(0, |c| c.max_pet_level),
            tame_remaining_ms: clock
                .filter(|c| c.tame_until_ms > 0)
                .map(|c| c.tame_until_ms.saturating_sub(now)),
        })
    }

    fn source_pet_is_saved(&self, class: MirClass, name: &str) -> bool {
        (self.npc_teleport_config.pet_save
            && (class != MirClass::Assassin || name != "AssassinClone"))
            || match class {
                MirClass::Wizard => true,
                MirClass::Taoist => source_taoist_pet_name(name),
                _ => false,
            }
    }

    pub fn capture_player_saved_pets(
        &self,
        session: &SessionId,
        now_ms: u64,
    ) -> ZoneSavedPetSnapshot {
        let Some(owner) = self.owned_pet_owner_for_live_pet(session) else {
            return ZoneSavedPetSnapshot::default();
        };
        let Some(master) = self.owned_pet_master(&owner) else {
            return ZoneSavedPetSnapshot::default();
        };
        let class = master.class;
        let mut pets: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                let life = self.owned_pet_life(id)?;
                (life.owner == owner
                    && m.master_object_id == owner.online.object_id
                    && self.source_pet_is_saved(class, &m.name))
                .then(|| self.saved_pet_info(id, now_ms).map(|p| (m.incarnation, p)))
                .flatten()
            })
            .collect();
        pets.sort_by_key(|(incarnation, _)| *incarnation);
        ZoneSavedPetSnapshot {
            pets: pets.into_iter().map(|(_, pet)| pet).collect(),
        }
    }

    pub fn capture_player_pet_experience(
        &self,
        session: &SessionId,
        now_ms: u64,
    ) -> Option<ZonePetExperienceAdmission> {
        let owner = self.owned_pet_owner_for_live_pet(session)?;
        let master = self.owned_pet_master(&owner)?;
        let position = &master.position;
        let same_map = self
            .online_presence
            .get(session)
            .is_some_and(|p| p.key == self.key);
        let mut pets: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                let life = self.owned_pet_life(id)?;
                if life.owner != owner || m.master_object_id != owner.online.object_id {
                    return None;
                }
                Some((
                    m.incarnation,
                    PetExperienceEntry {
                        life,
                        zone: self.key.clone(),
                        before: self.saved_pet_info(id, now_ms)?,
                        eligible: same_map
                            && zone_tile_distance(position, &m.position) <= PET_DATA_RANGE,
                        saveable: self.source_pet_is_saved(master.class, &m.name),
                    },
                ))
            })
            .collect();
        pets.sort_by_key(|(incarnation, _)| *incarnation);
        Some(ZonePetExperienceAdmission {
            owner,
            zone: self.key.clone(),
            pets: pets.into_iter().map(|(_, p)| p).collect(),
        })
    }

    pub fn apply_player_pet_experience(
        &mut self,
        session: &SessionId,
        admission: &ZonePetExperienceAdmission,
        amount: u32,
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        self.mirror_earned_steps(session, admission, &[amount], now_ms)
    }

    pub fn mirror_earned_steps(
        &mut self,
        session: &SessionId,
        admission: &ZonePetExperienceAdmission,
        amounts: &[u32],
        now_ms: u64,
    ) -> Vec<ZoneOutbound> {
        if &admission.owner.online.session_id != session
            || admission.zone != self.key
            || !self.owned_pet_owner_is_current(&admission.owner)
        {
            return Vec::new();
        }
        let Ok(after) = admission.apply_earned_steps(amounts) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (entry, updated) in admission.pets.iter().zip(&after.pets) {
            let id = entry.life.pet.object_id();
            if entry.zone != self.key
                || !entry.eligible
                || self.owned_pet_life(id).as_ref() != Some(&entry.life)
            {
                continue;
            }
            let Some(current) = self.saved_pet_info(id, now_ms) else {
                continue;
            };
            // Same admission cannot mirror XP twice, or overwrite a newer
            // level. HP damage during the durable CAS is intentionally kept.
            if (current.experience, current.level, current.max_pet_level)
                != (
                    entry.before.experience,
                    entry.before.level,
                    entry.before.max_pet_level,
                )
            {
                continue;
            }
            if let Some(clock) = self.owned_monster_clock_mut(id) {
                clock.pet_experience = updated.experience;
                clock.max_pet_level = updated.max_pet_level;
            }
            if updated.level != current.level {
                for level in current.level + 1..=updated.level {
                    self.native_monsters
                        .get_mut(&id)
                        .unwrap()
                        .summon_skill_level = level;
                    out.extend(self.refresh_owned_pet_source_stats(id, now_ms, false));
                }
            }
        }
        out
    }

    pub(super) fn initialize_owned_pet_growth(&mut self, id: u32, now: u64) {
        let Some(m) = self.native_monsters.get(&id) else {
            return;
        };
        let max = source_initial_max_pet_level(m);
        if let Some(clock) = self.owned_monster_clock_mut(id) {
            clock.max_pet_level = max;
        }
        let _ = self.refresh_owned_pet_source_stats(id, now, true);
    }

    fn refresh_owned_pet_source_stats(
        &mut self,
        id: u32,
        now: u64,
        fill_hp: bool,
    ) -> Vec<ZoneOutbound> {
        let Some(monster) = self.native_monsters.get(&id) else {
            return Vec::new();
        };
        let Some(template) = crystal_monster_by_name(&monster.name) else {
            return Vec::new();
        };
        let level = monster.summon_skill_level;
        let max = self.owned_monster_clock(id).map_or_else(
            || source_initial_max_pet_level(monster),
            |c| c.max_pet_level,
        );
        let m = self.native_monsters.get_mut(&id).unwrap();
        m.max_hp = template.hp.max(1).saturating_add(i32::from(level) * 20);
        m.hp = if fill_hp {
            m.max_hp
        } else {
            m.hp.min(m.max_hp)
        };
        m.defense = super::super::types::ZoneMonsterDefense::from_crystal_template(&template);
        m.defense.min_ac = m.defense.min_ac.saturating_add(i32::from(level) * 2);
        m.defense.max_ac = m.defense.max_ac.saturating_add(i32::from(level) * 2);
        m.defense.min_mac = m.defense.min_mac.saturating_add(i32::from(level) * 2);
        m.defense.max_mac = m.defense.max_mac.saturating_add(i32::from(level) * 2);
        let taoist = source_taoist_pet_name(&m.name);
        m.move_speed_ms = u64::from(template.move_speed)
            .saturating_sub(if taoist { u64::from(max) * 130 } else { 0 })
            .max(400);
        m.attack_speed_ms = u64::from(template.attack_speed)
            .saturating_sub(if taoist { u64::from(max) * 70 } else { 0 })
            .max(400);
        let health = native_monster_health_percent(m.hp, m.max_hp);
        let position = m.position.clone();
        if let Some(c) = self.owned_monster_clock_mut(id) {
            c.base_colour = source_pet_colour(level);
        }
        let mut out = self.refresh_owned_monster_control_packet(id, now);
        out.push(ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &position),
            packets: vec![ServerPacket::ObjectHealth {
                info: ObjectHealthInfo {
                    object_id: id,
                    percent: health,
                    expire: 0,
                },
            }],
        });
        out
    }

    pub fn restore_player_saved_pets(
        &mut self,
        session: &SessionId,
        snapshot: &ZoneSavedPetSnapshot,
        now_ms: u64,
    ) -> Result<Vec<ZoneOutbound>, String> {
        snapshot.validate()?;
        let Some(owner) = self.owned_pet_owner(session) else {
            return Ok(Vec::new());
        };
        // Trusted StartGame adoption only. Live XP uses the typed admission
        // above; this method cannot clone the current live owner's pets.
        if self
            .native_monsters
            .values()
            .any(|m| m.owner_session_id.as_ref() == Some(session))
        {
            return Ok(Vec::new());
        }
        let player = self.players[session].clone();
        let mut out = Vec::new();
        for pet in &snapshot.pets {
            let template = mir2_game_data::crystal_monster_by_index(pet.monster_index).unwrap();
            if !self.source_pet_is_saved(player.class, &template.name) {
                continue;
            }
            let id = self.unique_object_id(0);
            let position = self.first_available_position(player.position.clone(), None);
            let spawn = ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: id,
                name: template.name.clone(),
                name_colour_argb: source_pet_colour(pet.level),
                image: template.image,
                ai: template.ai,
                disposition: Some(crate::config::WorldEntityDisposition::Friendly),
                level: template.level,
                hp: template.hp.max(1),
                max_hp: template.hp.max(1),
                experience: 0,
                move_speed_ms: u64::from(template.move_speed),
                attack_speed_ms: u64::from(template.attack_speed),
                friendly_guild: None,
                position,
                direction: player.direction,
                defense: super::super::types::ZoneMonsterDefense::from_crystal_template(&template),
                respawn: None,
                drops: vec![],
            };
            let (spawned, _) =
                self.spawn_authoritative_monster_internal(&spawn, now_ms, false, false);
            if !spawned {
                continue;
            }
            let m = self.native_monsters.get_mut(&id).unwrap();
            m.owner_session_id = Some(session.clone());
            m.master_object_id = owner.online.object_id;
            m.owner_player_object_id = owner.online.object_id;
            m.hostile_to_player = false;
            m.summon_skill_level = pet.level;
            m.next_ai_ready_at_ms =
                now_ms.saturating_add(if template.name == "Clone" { 1001 } else { 2001 });
            m.next_attack_ready_at_ms = m.next_ai_ready_at_ms.saturating_sub(1);
            self.register_owned_pet_life(id);
            let pet_save = self.npc_teleport_config.pet_save;
            if let Some(c) = self.owned_monster_clock_mut(id) {
                c.max_pet_level = pet.max_pet_level;
                c.pet_experience = pet.experience;
                c.tame_until_ms =
                    if pet_save || player.class != MirClass::Wizard || template.name == "Clone" {
                        0
                    } else {
                        now_ms.saturating_add(pet.tame_remaining_ms.unwrap_or(0))
                    };
                if !pet_save && player.class == MirClass::Wizard && template.name != "Clone" {
                    c.tame_original_disposition = Some(if matches!(template.ai, 1 | 2 | 3) {
                        crate::config::WorldEntityDisposition::Neutral
                    } else {
                        crate::config::WorldEntityDisposition::Hostile
                    });
                    c.tame_original_hostile = !matches!(template.ai, 1 | 2 | 3);
                }
            }
            let _ = self.refresh_owned_pet_source_stats(id, now_ms, true);
            self.native_monsters.get_mut(&id).unwrap().hp = pet.hp;
            if let Some(object) = self.objects.get_mut(&id) {
                if let ServerPacket::ObjectMonster { info } = &mut object.packet {
                    info.master_object_id = owner.online.object_id;
                }
            }
            out.extend(self.diff_all_zone_object_visibility());
            out.push(ZoneOutbound::ToMany {
                session_ids: self
                    .native_monster_visible_recipients(id, &self.native_monsters[&id].position),
                packets: vec![ServerPacket::ObjectHealth {
                    info: ObjectHealthInfo {
                        object_id: id,
                        percent: native_monster_health_percent(
                            pet.hp,
                            self.native_monsters[&id].max_hp,
                        ),
                        expire: 0,
                    },
                }],
            });
        }
        out.extend(self.apply_owned_pet_map_entry_rules(session, now_ms));
        Ok(out)
    }

    pub fn apply_owned_pet_map_entry_rules(
        &mut self,
        session: &SessionId,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(owner) = self.owned_pet_owner(session) else {
            return Vec::new();
        };
        let frozen = self
            .npc_teleport_config
            .map(&self.key.map_file_name)
            .is_some_and(|m| m.no_pets);
        let ids: Vec<_> = self
            .native_monsters
            .iter()
            .filter_map(|(&id, m)| {
                (m.owner_session_id.as_ref() == Some(session)
                    && m.master_object_id == owner.online.object_id
                    && !m.dead)
                    .then_some(id)
            })
            .collect();
        let mut out = Vec::new();
        for id in ids {
            self.owned_pet_state_mut().targets.remove(&id);
            if let Some(c) = self.owned_monster_clock_mut(id) {
                c.frozen_by_no_pets = frozen;
            }
            let m = &self.native_monsters[&id];
            let packet = if frozen {
                ServerPacket::ObjectTurn {
                    movement: ObjectMovement {
                        object_id: id,
                        position: m.position.clone(),
                        direction: m.direction,
                    },
                }
            } else {
                self.objects[&id].packet.clone()
            };
            out.push(ZoneOutbound::ToMany {
                session_ids: self.native_monster_visible_recipients(id, &m.position),
                packets: vec![packet],
            });
        }
        if frozen && !out.is_empty() {
            out.push(ZoneOutbound::ToSession {
                session_id: session.clone(),
                packets: vec![ServerPacket::Chat {
                    message: "Pets are not allowed on this map.".into(),
                    chat_type: ChatType::System,
                }],
            });
        }
        self.note_owned_pet_time(now);
        out
    }

    fn owned_pet_state(&self) -> Option<&OwnedPetCombatState> {
        self.entity_combat.as_ref()?.owned_pet.as_ref()
    }

    fn owned_pet_state_mut(&mut self) -> &mut OwnedPetCombatState {
        self.entity_combat
            .get_or_insert_with(Default::default)
            .owned_pet
            .get_or_insert_with(Default::default)
    }

    pub(super) fn clear_owned_pet_owner_targets(&mut self, session: &SessionId) {
        let monsters = &self.native_monsters;
        if let Some(state) = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
        {
            state.targets.retain(|id, r| {
                &r.life.owner.online.session_id != session
                    || r.life.monster_master.is_some()
                    || monsters.get(id).is_some_and(|m| matches!(m.ai, 60 | 61))
            });
        }
    }

    pub(super) fn note_owned_pet_time(&mut self, now: u64) {
        if let Some(state) = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
        {
            state.last_now_ms = state.last_now_ms.max(now);
        }
    }

    pub(super) fn owned_pet_owner(&self, session: &SessionId) -> Option<OwnedPetOwner> {
        let player = self.players.get(session)?;
        let presence = self.online_presence.get(session)?;
        if player.dead
            || player.hp <= 0
            || presence.dead
            || presence.key != self.key
            || !presence.owner.matches_player(player)
        {
            return None;
        }
        Some(OwnedPetOwner {
            player: self.native_entity_player_ref(session)?,
            online: presence.owner.clone(),
        })
    }

    pub(super) fn owned_pet_owner_is_current(&self, owner: &OwnedPetOwner) -> bool {
        self.owned_pet_owner(&owner.online.session_id).as_ref() == Some(owner)
    }

    fn owned_pet_owner_for_live_pet(&self, session: &SessionId) -> Option<OwnedPetOwner> {
        if let Some(owner) = self.owned_pet_owner(session) {
            return Some(owner);
        }
        let master = self.owned_pet_state()?.detached_masters.get(session)?;
        let presence = self.online_presence.get(session)?;
        let owner = OwnedPetOwner {
            online: presence.owner.clone(),
            player: ZoneCombatEntityRef::Player {
                session_id: session.clone(),
                object_id: master.object_id,
                life_generation: master.life_generation,
            },
        };
        self.owned_pet_master(&owner).map(|_| owner)
    }

    fn owned_pet_master(&self, owner: &OwnedPetOwner) -> Option<ZonePlayer> {
        let presence = self.online_presence.get(&owner.online.session_id)?;
        let ZoneCombatEntityRef::Player {
            life_generation, ..
        } = &owner.player
        else {
            return None;
        };
        if presence.owner != owner.online
            || presence.dead
            || presence.life_generation != *life_generation
        {
            return None;
        }
        if let Some(player) = self.players.get(&owner.online.session_id) {
            return (self.owned_pet_owner(&owner.online.session_id).as_ref() == Some(owner))
                .then(|| player.clone());
        }
        let mut master = self
            .owned_pet_state()?
            .detached_masters
            .get(&owner.online.session_id)?
            .clone();
        if !owner.online.matches_player(&master) || master.life_generation != *life_generation {
            return None;
        }
        master.position = presence.position.clone();
        master.name = presence.name.clone();
        master.level = presence.level;
        master.chat_profile = presence.chat_profile.clone()?;
        master.dead = false;
        Some(master)
    }

    fn owned_pet_master_is_safe(&self, master: &ZonePlayer) -> bool {
        master.chat_profile.in_safe_zone
            || self
                .online_presence
                .get(&master.session_id)
                .is_some_and(|p| p.key == self.key && self.pet_position_is_safe(&master.position))
    }

    pub(crate) fn prepare_player_online_pet_transfer(&mut self, session: &SessionId) -> bool {
        let Some(owner) = self.owned_pet_owner(session) else {
            return false;
        };
        let mut metadata = self.players[session].clone();
        metadata.visible_object_ids.clear();
        metadata.movement_actions.clear();
        metadata.magic_ready_at_ms.clear();
        self.owned_pet_state_mut()
            .detached_masters
            .insert(owner.online.session_id, metadata);
        true
    }

    pub(super) fn detached_pet_survives_map_transfer(&self, id: u32) -> bool {
        let Some(binding) = self.owned_pet_state().and_then(|s| s.bindings.get(&id)) else {
            return false;
        };
        self.owned_pet_state().is_some_and(|s| {
            s.detached_masters
                .contains_key(&binding.owner.online.session_id)
        }) && self.owned_pet_master(&binding.owner).is_some()
    }

    pub(super) fn owned_pet_source_has_live_master(&self, id: u32) -> bool {
        self.owned_pet_life(id).is_some()
    }

    pub(crate) fn remove_player_detached_pets(&mut self, session: &SessionId) -> Vec<ZoneOutbound> {
        let Some(master) = self.owned_pet_state_mut().detached_masters.remove(session) else {
            return Vec::new();
        };
        self.remove_owner_generated_zone_objects(master.object_id, &master.name)
    }

    pub(crate) fn drain_online_pet_recalls(
        &mut self,
        now: u64,
    ) -> (Vec<ZoneOnlinePetRecall>, Vec<ZoneOutbound>) {
        let ids: Vec<_> = self
            .owned_pet_state()
            .map(|s| s.bindings.keys().copied().collect())
            .unwrap_or_default();
        let mut recalls = Vec::new();
        let mut out = Vec::new();
        let mut notified = BTreeSet::new();
        for id in ids {
            let Some(binding) = self
                .owned_pet_state()
                .and_then(|s| s.bindings.get(&id))
                .cloned()
            else {
                continue;
            };
            // CharmedSnake's custom AI follows its Totem, so never recall a
            // SlaveList child to the causal Human's new map.
            if binding.monster_master.is_some() {
                continue;
            }
            let Some(presence) = self
                .online_presence
                .get(&binding.owner.online.session_id)
                .cloned()
            else {
                continue;
            };
            if presence.owner != binding.owner.online {
                continue;
            }
            let Some(m) = self.native_monsters.get(&id).filter(|m| !m.dead).cloned() else {
                continue;
            };
            if presence.dead {
                self.native_monsters.get_mut(&id).unwrap().hp = 0;
                self.native_monsters.get_mut(&id).unwrap().dead = true;
                self.owned_pet_state_mut()
                    .death_packets
                    .push(OwnedPetDeath {
                        object_id: id,
                        position: m.position.clone(),
                        direction: m.direction,
                    });
                continue;
            }
            let Some(life) = self.owned_pet_life(id) else {
                continue;
            };
            let Some(master) = self.owned_pet_master(&life.owner) else {
                continue;
            };
            // These source classes override base ProcessAI and self-destruct
            // when Master is absent from their 15-tile FindObject range.
            if matches!(m.ai, 60 | 61 | 62) {
                continue;
            }
            let no_pets = self
                .npc_teleport_config
                .map(&presence.key.map_file_name)
                .is_some_and(|map| map.no_pets);
            let was_frozen = self.owned_monster_frozen(id);
            if no_pets {
                if !was_frozen {
                    self.owned_pet_state_mut().targets.remove(&id);
                    if let Some(c) = self.owned_monster_clock_mut(id) {
                        c.frozen_by_no_pets = true;
                    }
                    out.push(ZoneOutbound::ToMany {
                        session_ids: self.native_monster_visible_recipients(id, &m.position),
                        packets: vec![ServerPacket::ObjectTurn {
                            movement: ObjectMovement {
                                object_id: id,
                                position: m.position.clone(),
                                direction: m.direction,
                            },
                        }],
                    });
                    notified.insert(life.owner.online.session_id.clone());
                }
                continue;
            }
            if was_frozen {
                if let Some(c) = self.owned_monster_clock_mut(id) {
                    c.frozen_by_no_pets = false;
                }
            }
            if presence.key == self.key {
                // Source PetRecall only calls Teleport for a different map.
                // A same-map out-of-range call does not move the pet's Node.
                continue;
            }
            if !was_frozen && !pet_mode_can_move(master.chat_profile.pet_mode) {
                continue;
            }
            let Some(object) = self.objects.get(&id).cloned() else {
                continue;
            };
            let poisons = self
                .owned_pet_state()
                .map(|s| {
                    s.poisons
                        .iter()
                        .filter(|p| p.target == life.pet)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            let capsule = ZoneOnlinePetRecall {
                destination: presence.key,
                life,
                monster: m.clone(),
                object,
                clock: self.owned_monster_clock(id).cloned(),
                poisons,
            };
            out.extend(self.remove_recalled_pet_object(id, now));
            recalls.push(capsule);
        }
        for session_id in notified {
            out.push(ZoneOutbound::ToSession {
                session_id,
                packets: vec![ServerPacket::Chat {
                    message: "Pets are not allowed on this map.".into(),
                    chat_type: ChatType::System,
                }],
            });
        }
        out.extend(self.flush_owned_pet_deaths(now));
        (recalls, out)
    }

    fn remove_recalled_pet_object(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let Some(m) = self.native_monsters.get(&id) else {
            return Vec::new();
        };
        let recipients = self.native_monster_visible_recipients(id, &m.position);
        self.retire_native_source_object(id);
        self.native_monsters.remove(&id);
        self.native_monster_respawns.remove(&id);
        self.objects.remove(&id);
        self.object_grid.remove(&id);
        self.dead_object_ids.remove(&id);
        self.revived_object_ids.remove(&id);
        self.harvested_object_ids.remove(&id);
        self.removed_object_ids.insert(id);
        for player in self.players.values_mut() {
            player.visible_object_ids.remove(&id);
        }
        self.note_owned_pet_time(now);
        vec![ZoneOutbound::ToMany {
            session_ids: recipients,
            packets: vec![
                ServerPacket::ObjectTeleportOut {
                    object_id: id,
                    effect_type: 0,
                },
                ServerPacket::ObjectRemove { object_id: id },
            ],
        }]
    }

    pub(crate) fn adopt_online_pet_recall(
        &mut self,
        mut recall: ZoneOnlinePetRecall,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if recall.destination != self.key
            || recall.monster.dead
            || self
                .owned_pet_owner(&recall.life.owner.online.session_id)
                .as_ref()
                != Some(&recall.life.owner)
            || self
                .npc_teleport_config
                .map(&self.key.map_file_name)
                .is_some_and(|m| m.no_pets)
        {
            return Vec::new();
        }
        let owner = self.players[&recall.life.owner.online.session_id].clone();
        let old_id = recall.life.pet.object_id();
        let id = if self.objects.contains_key(&old_id)
            || self.players.values().any(|p| p.object_id == old_id)
        {
            self.unique_object_id(0)
        } else {
            old_id
        };
        if id != old_id {
            recall.monster.incarnation = self.allocate_monster_incarnation();
        }
        self.next_monster_incarnation = self
            .next_monster_incarnation
            .max(recall.monster.incarnation);
        // A recalled live Node also reserves its ID in the destination's
        // allocator. Otherwise logout followed by saved-pet adoption can
        // allocate this retired ID and its retained ObjectRemove tombstone
        // suppresses the fresh ObjectMonster.
        self.next_object_id = self.next_object_id.max(id.saturating_add(1));
        // MapObject.Teleport uses ValidPoint, which does not reject occupied
        // cells. PetRecall first tries Master.Back, then Master.CurrentLocation.
        let back = offset_point(&owner.position, owner.direction, -1);
        let position = if !self.collision.is_blocked(&back) {
            back
        } else {
            owner.position.clone()
        };
        if self.collision.is_blocked(&position) {
            return Vec::new();
        }
        let health = native_monster_health_percent(recall.monster.hp, recall.monster.max_hp);
        recall.monster.position = position.clone();
        recall.monster.master_object_id = owner.object_id;
        recall.monster.owner_player_object_id = owner.object_id;
        if let Some(s) = recall
            .monster
            .special_ai
            .as_mut()
            .and_then(|s| s.shinsu.as_mut())
        {
            super::shinsu_ai::clear_owned_shinsu_recall_hits(s);
        }
        recall.object.object_id = id;
        recall.object.position = position.clone();
        if let ServerPacket::ObjectMonster { info } = &mut recall.object.packet {
            info.object_id = id;
            info.location = position.clone();
            info.master_object_id = owner.object_id;
        }
        if let Some(h) = &mut recall.object.health {
            h.object_id = id;
        }
        if let Some(m) = &mut recall.object.mana {
            m.object_id = id;
        }
        self.native_monsters.insert(id, recall.monster);
        self.objects.insert(id, recall.object);
        self.object_grid.insert(id, &position);
        self.removed_object_ids.remove(&id);
        self.register_owned_pet_life(id);
        let Some(fresh) = self.owned_pet_life(id) else {
            return Vec::new();
        };
        if let Some(mut clock) = recall.clock {
            clock.target = fresh.pet.clone();
            clock.frozen_by_no_pets = false;
            self.owned_pet_state_mut().monster_clocks.insert(id, clock);
        }
        for mut poison in recall.poisons {
            poison.target = fresh.pet.clone();
            poison.pet_owner = Some(fresh.owner.clone());
            self.owned_pet_state_mut().poisons.push(poison);
        }
        let mut out = self.diff_all_zone_object_visibility();
        out.push(ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &position),
            packets: vec![
                ServerPacket::ObjectTeleportIn {
                    object_id: id,
                    effect_type: 0,
                },
                ServerPacket::ObjectHealth {
                    info: ObjectHealthInfo {
                        object_id: id,
                        percent: health,
                        expire: 0,
                    },
                },
            ],
        });
        self.note_owned_pet_time(now);
        out
    }

    fn owned_human_source_has_node(&self, owner: &OwnedPetOwner) -> bool {
        // PoisonList loses Owner only at Node removal, not at the owner's
        // death. A revive/rejoin still has a different typed life/online epoch.
        let ZoneCombatEntityRef::Player {
            session_id,
            object_id,
            life_generation,
        } = &owner.player
        else {
            return false;
        };
        self.online_presence.get(session_id).is_some_and(|p| {
            p.owner == owner.online
                && p.owner.object_id == *object_id
                && p.life_generation == *life_generation
        }) && (!self.players.contains_key(session_id)
            || self.native_entity_player_ref(session_id).as_ref() == Some(&owner.player))
    }

    fn owned_human_owner_with_node(&self, session: &SessionId) -> Option<OwnedPetOwner> {
        let player = self.players.get(session)?;
        let presence = self.online_presence.get(session)?;
        if presence.key != self.key || !presence.owner.matches_player(player) {
            return None;
        }
        Some(OwnedPetOwner {
            player: self.native_entity_player_ref(session)?,
            online: presence.owner.clone(),
        })
    }

    pub(super) fn owned_pet_caster_proof_is_current(
        &self,
        owner: &OwnedPetOwner,
        session: &SessionId,
        object_id: u32,
    ) -> bool {
        &owner.online.session_id == session
            && owner.online.object_id == object_id
            && self.owned_pet_owner_is_current(owner)
    }

    /// Called only after the real server spell has inserted its pet and Node.
    pub(super) fn register_owned_pet_life(&mut self, id: u32) {
        let Some(monster) = self.native_monsters.get(&id) else {
            return;
        };
        let Some(session) = monster.owner_session_id.as_ref() else {
            return;
        };
        let Some(owner) = self.owned_pet_owner(session) else {
            return;
        };
        if owner.online.object_id != monster.master_object_id
            || owner.online.object_id != zone_native_summon_owner_player_object_id(monster)
        {
            return;
        }
        let Some(pet) = self.native_entity_monster_ref(id) else {
            return;
        };
        self.owned_pet_state_mut().bindings.insert(
            id,
            OwnedPetLife {
                owner,
                pet,
                monster_master: None,
            },
        );
    }

    /// Only the real SnakeTotem production path may attach a newly born child.
    /// Neither a legacy checkpoint nor an unowned same-id monster mints a life.
    pub(super) fn register_owned_snake_child_life(&mut self, id: u32, master_id: u32) {
        let Some(parent) = self.owned_pet_life(master_id) else {
            return;
        };
        let Some(master) = self.native_monsters.get(&master_id) else {
            return;
        };
        let Some(child) = self.native_monsters.get(&id) else {
            return;
        };
        if parent.monster_master.is_some()
            || master.ai != 62
            || master.name != "SnakeTotem"
            || child.ai != 63
            || child.name != "CharmedSnake"
            || self.dead_object_ids.contains_key(&id)
            || self.dead_object_ids.contains_key(&master_id)
            || child
                .special_ai
                .as_ref()
                .and_then(|s| s.snake_totem.as_ref())
                .is_some_and(|s| s.retired_child)
            || child.master_object_id != master_id
            || child.owner_session_id.as_ref() != Some(&parent.owner.online.session_id)
            || zone_native_summon_owner_player_object_id(child) != parent.owner.online.object_id
        {
            return;
        }
        let Some(pet) = self.native_entity_monster_ref(id) else {
            return;
        };
        if !self.entity_ref_exists(&pet, false) {
            return;
        }
        self.owned_pet_state_mut().bindings.insert(
            id,
            OwnedPetLife {
                owner: parent.owner,
                pet,
                monster_master: Some(parent.pet),
            },
        );
    }

    fn owned_pet_life(&self, id: u32) -> Option<OwnedPetLife> {
        let binding = self.owned_pet_state()?.bindings.get(&id)?;
        let monster = self.native_monsters.get(&id)?;
        if !self.entity_ref_exists(&binding.pet, false)
            || self.owned_pet_master(&binding.owner).is_none()
            || monster.owner_session_id.as_ref() != Some(&binding.owner.online.session_id)
            || zone_native_summon_owner_player_object_id(monster) != binding.owner.online.object_id
        {
            return None;
        }
        if let Some(master) = &binding.monster_master {
            let parent = self.owned_pet_state()?.bindings.get(&master.object_id())?;
            let parent_monster = self.native_monsters.get(&master.object_id())?;
            if monster.ai != 63
                || monster.name != "CharmedSnake"
                || self.dead_object_ids.contains_key(&id)
                || self.dead_object_ids.contains_key(&master.object_id())
                || self.removed_object_ids.contains(&master.object_id())
                || monster
                    .special_ai
                    .as_ref()
                    .and_then(|s| s.snake_totem.as_ref())
                    .is_some_and(|s| s.retired_child)
                || monster.master_object_id != master.object_id()
                || parent.monster_master.is_some()
                || parent.pet != *master
                || parent.owner != binding.owner
                || parent_monster.ai != 62
                || parent_monster.name != "SnakeTotem"
                || self.owned_pet_life(master.object_id()).as_ref() != Some(parent)
                || zone_tile_distance(&monster.position, &parent_monster.position) > 15
            {
                return None;
            }
        } else if monster.master_object_id != binding.owner.online.object_id {
            return None;
        }
        Some(binding.clone())
    }

    pub(super) fn owned_pet_immediate_monster_master(
        &self,
        id: u32,
    ) -> Option<ZoneCombatEntityRef> {
        self.owned_pet_life(id)?.monster_master
    }

    fn owned_pet_immediate_master_mode(&self, life: &OwnedPetLife) -> Option<u8> {
        if life.monster_master.is_some() {
            // MonsterObject constructor: AMode=All, PMode=Both. A Human's
            // modes do not overwrite its Monster's own Master properties.
            Some(0)
        } else {
            Some(self.owned_pet_master(&life.owner)?.chat_profile.pet_mode)
        }
    }

    fn pet_position_is_safe(&self, position: &Point) -> bool {
        crystal_map_respawns_ref(&self.key.map_file_name).is_some_and(|map| {
            map.safe_zones.iter().any(|safe| {
                let size = i32::from(safe.size);
                (position.x - safe.location.x).abs() <= size
                    && (position.y - safe.location.y).abs() <= size
            })
        })
    }

    pub(super) fn owned_pet_brown_until(&self, player: &ZonePlayer) -> u64 {
        if !self.players.contains_key(&player.session_id) {
            return self
                .online_presence
                .get(&player.session_id)
                .filter(|p| p.owner.matches_player(player))
                .map_or(0, |p| p.brown_until_ms);
        }
        self.owned_pet_state()
            .and_then(|s| s.players.get(&player.session_id))
            .filter(|clock| {
                self.online_presence
                    .get(&player.session_id)
                    .is_some_and(|presence| {
                        presence.owner == clock.online && clock.online.matches_player(player)
                    })
            })
            .map_or(0, |clock| clock.brown_until_ms)
    }

    fn owned_pet_last_hitter_is(
        &self,
        player: &ZonePlayer,
        actor: &ZoneCombatEntityRef,
        now: u64,
    ) -> bool {
        let Some(clock) = self.owned_pet_master_clock(player) else {
            return false;
        };
        let Some(hitter) = clock.last_hitter.as_ref() else {
            return false;
        };
        hitter.entity == *actor
            && now <= clock.last_hit_until_ms
            && self
                .online_presence
                .get(&player.session_id)
                .is_some_and(|p| p.owner == clock.online)
            && self.owned_pet_threat_actor_is_current(hitter)
    }

    fn owned_pet_master_clock(&self, player: &ZonePlayer) -> Option<&OwnedPetPlayerClock> {
        if self.players.contains_key(&player.session_id) {
            self.owned_pet_state()?.players.get(&player.session_id)
        } else {
            self.online_presence
                .get(&player.session_id)?
                .owned_pet_clock
                .as_ref()
        }
    }

    fn owned_pet_threat_actor_is_current(&self, actor: &OwnedPetThreatActor) -> bool {
        match &actor.entity {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                actor.online.as_ref().is_some_and(|online| {
                    &online.session_id == session_id
                        && self.owned_human_source_has_node(&OwnedPetOwner {
                            player: actor.entity.clone(),
                            online: online.clone(),
                        })
                })
            }
            ZoneCombatEntityRef::Monster { .. } => {
                actor.zone == self.key && self.entity_ref_exists(&actor.entity, false)
            }
        }
    }

    fn owned_pet_has_target(&self, owner: &OwnedPetOwner, target: &ZoneCombatEntityRef) -> bool {
        self.owned_pet_state().is_some_and(|state| {
            state.targets.values().any(|r| {
                r.life.owner == *owner
                    && r.life.monster_master.is_none()
                    && r.target == *target
                    && self.owned_pet_life(r.life.pet.object_id()).as_ref() == Some(&r.life)
            })
        })
    }

    fn owned_pet_monster_targets(&self, source_id: u32, target: &ZoneCombatEntityRef) -> bool {
        self.owned_pet_state()
            .and_then(|s| s.wild_targets.get(&source_id))
            .is_some_and(|r| {
                r.target == *target
                    && self.entity_ref_exists(&r.source, false)
                    && self.entity_ref_exists(&r.target, false)
            })
            || self.entity_source_targets(source_id, target)
    }

    /// A dedicated permission check. Never widen native_entity_can_attack.
    fn owned_pet_can_attack(
        &self,
        life: &OwnedPetLife,
        target: &ZoneCombatEntityRef,
        purpose: EntityTargetPurpose,
        now: u64,
    ) -> bool {
        if !self.entity_ref_exists(target, false)
            || self.owned_pet_life(life.pet.object_id()).as_ref() != Some(life)
            || life.pet.object_id() == target.object_id()
        {
            return false;
        }
        let source = &self.native_monsters[&life.pet.object_id()];
        if let Some(master) = &life.monster_master {
            return self.owned_snake_child_can_attack(life, master, target, purpose, now);
        }
        let Some(owner) = self.owned_pet_master(&life.owner) else {
            return false;
        };
        match target {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let victim = &self.players[session_id];
                if life.owner.player == *target
                    || victim.chat_profile.is_gm
                    || victim.combat_stats.gm_never_die
                {
                    return false;
                }
                // Guard exceptions precede Master/safe/mode checks in Crystal.
                if matches!(source.ai, 6 | 58 | 113) {
                    return victim.chat_profile.pk_points >= 200;
                }
                if victim.chat_profile.in_safe_zone
                    || self.owned_pet_master_is_safe(&owner)
                    || self.pet_position_is_safe(&victim.position)
                    || self.pet_position_is_safe(&source.position)
                {
                    return false;
                }
                if purpose != EntityTargetPurpose::Impact
                    && (victim.hidden && !self.entity_cool_eye_sees(source, victim.level)
                        || now < source.hallucination_until_ms)
                {
                    return false;
                }
                if !self.owned_pet_last_hitter_is(victim, &life.owner.player, now)
                    && !self.owned_pet_last_hitter_is(&owner, target, now)
                    && !self.owned_pet_has_target(&life.owner, target)
                {
                    return false;
                }
                match owner.chat_profile.attack_mode {
                    0 => false,
                    1 => !victim
                        .chat_profile
                        .group_members
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&owner.name)),
                    2 | 5 => true,
                    3 => false,
                    4 => {
                        victim.chat_profile.pk_points >= 200
                            || now < self.owned_pet_brown_until(victim)
                    }
                    _ => true,
                }
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let victim = &self.native_monsters[object_id];
                if self.owned_monster_shocked(*object_id, now) {
                    return false;
                }
                if !monster_visibility_is_attackable(victim) || !trap_rock_visible(victim) {
                    return false;
                }
                if purpose != EntityTargetPurpose::Impact {
                    let hidden = self.objects.get(object_id).is_some_and(|o| matches!(&o.packet, ServerPacket::ObjectMonster { info } if info.hidden));
                    if hidden && !self.entity_cool_eye_sees(source, victim.level) {
                        return false;
                    }
                }
                if let Some(victim_life) = self.owned_pet_life(*object_id) {
                    if life.owner.online == victim_life.owner.online {
                        return false;
                    }
                    let Some(victim_owner) = self.owned_pet_master(&victim_life.owner) else {
                        return false;
                    };
                    if self.owned_pet_master_is_safe(&owner)
                        || self.owned_pet_master_is_safe(&victim_owner)
                    {
                        return false;
                    }
                    match owner.chat_profile.attack_mode {
                        0 => return false,
                        1 if victim_owner
                            .chat_profile
                            .group_members
                            .iter()
                            .any(|n| n.eq_ignore_ascii_case(&owner.name)) =>
                        {
                            return false;
                        }
                        // Source checks attacker.Master, and uses AND here.
                        4 if owner.chat_profile.pk_points < 200
                            || now > self.owned_pet_brown_until(&owner) =>
                        {
                            return false;
                        }
                        _ => {}
                    }
                    let victim_pets_claimed_by_owner = self.native_monsters.values().any(|pet| {
                        pet.owner_session_id.as_ref() == Some(&victim_life.owner.online.session_id)
                            && pet.master_object_id == victim_life.owner.online.object_id
                            && pet.experience_owner.as_ref().is_some_and(|claim| {
                                now <= claim.expires_at_ms
                                    && claim.online_owner.as_ref() == Some(&life.owner.online)
                            })
                    });
                    victim_pets_claimed_by_owner
                        || self.owned_pet_has_target(&life.owner, target)
                        || self.owned_pet_state().is_some_and(|s| {
                            s.targets.values().any(|r| {
                                r.life.owner == victim_life.owner
                                    && r.life.monster_master.is_none()
                                    && r.target == life.pet
                                    && self.owned_pet_life(r.life.pet.object_id()).as_ref()
                                        == Some(&r.life)
                            })
                        })
                        || self.owned_pet_last_hitter_is(&victim_owner, &life.owner.player, now)
                } else {
                    if victim.owner_session_id.is_some()
                        || !victim.hostile_to_player
                        || matches!(victim.ai, 57 | 68)
                    {
                        return false;
                    }
                    self.owned_pet_monster_targets(*object_id, &life.owner.player)
                        || self.owned_pet_monster_targets(*object_id, &life.pet)
                        || self.owned_pet_has_target(&life.owner, target)
                        || now < source.hallucination_until_ms
                }
            }
        }
    }

    fn owned_snake_child_can_attack(
        &self,
        life: &OwnedPetLife,
        master: &ZoneCombatEntityRef,
        target: &ZoneCombatEntityRef,
        purpose: EntityTargetPurpose,
        now: u64,
    ) -> bool {
        let source = &self.native_monsters[&life.pet.object_id()];
        let parent = &self.native_monsters[&master.object_id()];
        match target {
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let victim = &self.native_monsters[object_id];
                if victim.owner_session_id.is_some()
                    || victim.master_object_id != 0
                    || !victim.hostile_to_player
                    || matches!(victim.ai, 57 | 68)
                    || self.owned_monster_shocked(*object_id, now)
                    || !monster_visibility_is_attackable(victim)
                    || !trap_rock_visible(victim)
                {
                    return false;
                }
                if purpose != EntityTargetPurpose::Impact {
                    let hidden = self.objects.get(object_id).is_some_and(|o| {
                        matches!(&o.packet,
                        ServerPacket::ObjectMonster { info } if info.hidden)
                    });
                    if hidden && !self.entity_cool_eye_sees(source, victim.level) {
                        return false;
                    }
                }
                // SnakeTotem.SlaveList is not MapObject.Pets. In the ordinary
                // source path only victim.Target==attacker.Master grants this
                // threat; the causal Human's Target/Pets cannot grant it.
                self.owned_pet_monster_targets(*object_id, master)
                    || now < source.hallucination_until_ms
                    || self.owned_monster_rage_active(life.pet.object_id(), now)
            }
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let victim = &self.players[session_id];
                !victim.chat_profile.is_gm
                    && !victim.combat_stats.gm_never_die
                    && !victim.chat_profile.in_safe_zone
                    && !self.pet_position_is_safe(&victim.position)
                    && !self.pet_position_is_safe(&source.position)
                    && !self.pet_position_is_safe(&parent.position)
                    && (purpose == EntityTargetPurpose::Impact
                        || (!victim.hidden || self.entity_cool_eye_sees(source, victim.level))
                            && now >= source.hallucination_until_ms)
                    // The Totem has no ordinary attack, Pets or Human
                    // LastHitter. Do not substitute the Human threat clock.
                    && self.owned_pet_last_hitter_is(victim, master, now)
            }
        }
    }

    fn owned_pet_target_position(&self, target: &ZoneCombatEntityRef) -> Option<Point> {
        match target {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                Some(self.players.get(session_id)?.position.clone())
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                Some(self.native_monsters.get(object_id)?.position.clone())
            }
        }
    }

    pub(super) fn selected_owned_pet_target(
        &mut self,
        id: u32,
        now: u64,
    ) -> Option<NativeEntityTarget> {
        let life = self.owned_pet_life(id)?;
        let source = self.native_monsters.get(&id)?.clone();
        let mode = self.owned_pet_immediate_master_mode(&life)?;
        if matches!(mode, 1 | 3) && source.ai != 61 {
            // Custom Vampire/Toad ProcessAI does not execute the base AI's
            // PMode target-clearing block. Pausing must retain assigned Target.
            if source.ai != 60 {
                self.owned_pet_state_mut().targets.remove(&id);
            }
            return None;
        }
        if let Some(relation) = self
            .owned_pet_state()
            .and_then(|s| s.targets.get(&id))
            .cloned()
        {
            if relation.life == life
                && self.owned_pet_can_attack(
                    &life,
                    &relation.target,
                    EntityTargetPurpose::Impact,
                    now,
                )
            {
                if let Some(position) = self
                    .owned_pet_target_position(&relation.target)
                    .filter(|p| zone_tile_distance(&source.position, p) <= PET_DATA_RANGE)
                {
                    return Some(NativeEntityTarget {
                        object_id: relation.target.object_id(),
                        reference: relation.target,
                        position,
                    });
                }
            }
            self.owned_pet_state_mut().targets.remove(&id);
        }
        if !pet_mode_can_search(mode) && source.ai != 61 {
            return None;
        }
        let mut candidates: Vec<_> = self
            .native_monsters
            .keys()
            .filter_map(|other| {
                let target = self.native_entity_monster_ref(*other)?;
                let position = self.owned_pet_target_position(&target)?;
                (zone_tile_distance(&source.position, &position) <= PET_DATA_RANGE
                    && self.owned_pet_can_attack(&life, &target, EntityTargetPurpose::Search, now))
                .then_some(NativeEntityTarget {
                    object_id: *other,
                    reference: target,
                    position,
                })
            })
            .chain(self.players.keys().filter_map(|session| {
                let target = self.native_entity_player_ref(session)?;
                let position = self.owned_pet_target_position(&target)?;
                (zone_tile_distance(&source.position, &position) <= PET_DATA_RANGE
                    && self.owned_pet_can_attack(&life, &target, EntityTargetPurpose::Search, now))
                .then_some(NativeEntityTarget {
                    object_id: target.object_id(),
                    reference: target,
                    position,
                })
            }))
            .collect();
        // Crystal FindTarget ring traversal is deterministic by radius, y, x.
        candidates.sort_by_key(|t| {
            (
                zone_tile_distance(&source.position, &t.position),
                t.position.y,
                t.position.x,
                t.object_id,
            )
        });
        let mut target = candidates.into_iter().next()?;
        // FindTarget:1978 redirects an eligible player's first eligible pet.
        if let ZoneCombatEntityRef::Player { session_id, .. } = &target.reference {
            if let Some(pet) = self
                .owned_pet_state()
                .into_iter()
                .flat_map(|s| s.bindings.values())
                .find(|b| {
                    &b.owner.online.session_id == session_id
                        && self.owned_pet_can_attack(
                            &life,
                            &b.pet,
                            EntityTargetPurpose::Search,
                            now,
                        )
                })
            {
                target = NativeEntityTarget {
                    object_id: pet.pet.object_id(),
                    reference: pet.pet.clone(),
                    position: self.owned_pet_target_position(&pet.pet)?,
                };
            }
        }
        self.owned_pet_state_mut().targets.insert(
            id,
            OwnedPetTarget {
                life,
                target: target.reference.clone(),
            },
        );
        Some(target)
    }

    pub(super) fn owned_pet_can_move_now(&self, id: u32) -> bool {
        if self
            .owned_monster_clock(id)
            .is_some_and(|c| c.frozen_by_no_pets)
        {
            return false;
        }
        if self.owned_monster_shock_blocks_move(
            id,
            self.owned_pet_state().map_or(0, |s| s.last_now_ms),
        ) {
            return false;
        }
        self.owned_pet_life(id).is_some_and(|life| {
            self.owned_pet_immediate_master_mode(&life)
                .is_some_and(pet_mode_can_move)
        })
    }

    pub(super) fn owned_pet_can_launch_now(&self, id: u32) -> bool {
        if self
            .owned_monster_clock(id)
            .is_some_and(|c| c.frozen_by_no_pets)
        {
            return false;
        }
        self.owned_pet_life(id).is_some_and(|life| {
            self.owned_pet_immediate_master_mode(&life)
                .is_some_and(pet_mode_can_attack)
        })
    }

    pub(super) fn owned_pet_summon_initial_target(
        &self,
        object_id: u32,
        name: &str,
    ) -> Option<OwnedPetSummonTarget> {
        if !matches!(name, "VampireSpider" | "SpittingToad" | "SnakeTotem") {
            return None;
        }
        let entity = self
            .players
            .iter()
            .find_map(|(sid, p)| {
                (p.object_id == object_id)
                    .then(|| self.native_entity_player_ref(sid))
                    .flatten()
            })
            .or_else(|| self.native_entity_monster_ref(object_id))?;
        let online = self.owned_pet_target_online(&entity);
        Some(OwnedPetSummonTarget { entity, online })
    }

    pub(super) fn assign_owned_pet_initial_target(
        &mut self,
        id: u32,
        target: Option<OwnedPetSummonTarget>,
        now: u64,
    ) {
        let Some(life) = self.owned_pet_life(id) else {
            return;
        };
        let Some(proof) = target.filter(|target| {
            self.entity_ref_exists(&target.entity, false)
                && self.owned_pet_target_online(&target.entity) == target.online
        }) else {
            return;
        };
        let target = proof.entity;
        self.owned_pet_state_mut().targets.insert(
            id,
            OwnedPetTarget {
                life: life.clone(),
                target: target.clone(),
            },
        );
        if !self.owned_pet_can_attack(&life, &target, EntityTargetPurpose::Impact, now) {
            self.owned_pet_state_mut().targets.remove(&id);
        }
    }

    pub(super) fn owned_pet_target_online(
        &self,
        target: &ZoneCombatEntityRef,
    ) -> Option<OnlineOwner> {
        match target {
            ZoneCombatEntityRef::Player { session_id, .. } => self
                .online_presence
                .get(session_id)
                .map(|p| p.owner.clone()),
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                self.owned_pet_life(*object_id).map(|p| p.owner.online)
            }
        }
    }

    /// VampireSpider.ProcessAI omits ProcessRoam: it pursues an actual Target
    /// but never follows Master while idle. Its bite resolves immediately.
    pub(super) fn tick_owned_vampire_spider(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let Some(life) = self.owned_pet_life(id) else {
            return Vec::new();
        };
        let m = self.native_monsters[&id].clone();
        if m.ai != 60 || m.dead {
            return Vec::new();
        }
        let Some(target) = self.selected_owned_pet_target(id, now) else {
            return Vec::new();
        };
        if !self.owned_pet_can_launch_now(id)
            || now <= m.next_ai_ready_at_ms
            || now <= m.next_attack_ready_at_ms
            || super::entity_combat::native_entity_attack_blocked(&m, now)
            || !self.owned_pet_can_attack(
                &life,
                &target.reference,
                EntityTargetPurpose::VisibleImpact,
                now,
            )
        {
            return Vec::new();
        }
        let direction = zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
        if !native_monster_adjacent_to(&m.position, &target.position) {
            if !self.owned_pet_can_move_now(id)
                || !zone_native_summon_can_move(&m)
                || self.owned_monster_shock_blocks_move(id, now)
            {
                return Vec::new();
            }
            let destination = offset_point(&m.position, direction, 1);
            if !self.can_native_monster_occupy(id, &destination) {
                let out = self.turn_native_monster(id, direction, now);
                // MonsterObject.Turn does not assign ActionTime or MoveTime.
                if let Some(live) = self.native_monsters.get_mut(&id) {
                    live.next_ai_ready_at_ms = m.next_ai_ready_at_ms;
                }
                return out;
            }
            let live = self.native_monsters.get_mut(&id).unwrap();
            live.position = destination.clone();
            live.direction = direction;
            live.next_ai_ready_at_ms = now.saturating_add(m.move_speed_ms);
            let packet = ServerPacket::ObjectWalk {
                movement: ObjectMovement {
                    object_id: id,
                    position: destination.clone(),
                    direction,
                },
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            let mut out = self.diff_all_zone_object_visibility();
            out.push(ZoneOutbound::ToMany {
                session_ids: self.native_monster_visible_recipients(id, &destination),
                packets: vec![packet],
            });
            return out;
        }
        let Some(template) = crystal_monster_by_name(&m.name) else {
            return Vec::new();
        };
        let growth = i32::from(m.summon_skill_level);
        let damage = zone_roll_stat_range(
            template
                .min_dc
                .saturating_add(growth)
                .saturating_add(zone_native_monster_buff_stat_total(&m, 4)),
            template
                .max_dc
                .saturating_add(growth)
                .saturating_add(zone_native_monster_buff_stat_total(&m, 5)),
            now,
            id,
            60,
        );
        let live = self.native_monsters.get_mut(&id).unwrap();
        live.direction = direction;
        live.next_ai_ready_at_ms = now.saturating_add(300);
        live.next_attack_ready_at_ms = now.saturating_add(m.attack_speed_ms);
        if let Some(clock) = self.owned_monster_clock_mut(id) {
            clock.shock_until_ms = 0;
        }
        let packet = ServerPacket::ObjectAttack {
            info: ObjectAttackInfo {
                object_id: id,
                location: m.position.clone(),
                direction,
                spell: 0,
                level: 0,
                attack_type: 0,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        let mut out = vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &m.position),
            packets: vec![packet],
        }];
        let cell = offset_point(&m.position, direction, 1);
        let victim = if target.position == cell {
            Some(target.reference)
        } else {
            self.owned_pet_target_in_cell(id, &cell, now)
        };
        let Some(victim) = victim.filter(|_| damage > 0) else {
            return out;
        };
        let target_online = self.owned_pet_target_online(&victim);
        let damage_out = self.resolve_owned_pet_hit(
            OwnedPetHit {
                life: life.clone(),
                target: victim.clone(),
                target_online,
                damage,
                defence: EntityDefence::MACAgility,
                due_ms: now,
                journey_event: None,
            },
            now,
        );
        let actual = damage_out.iter().find_map(|event| match event {
            ZoneOutbound::PlayerDamaged { session_id, damage, .. }
                if matches!(&victim, ZoneCombatEntityRef::Player { session_id: target, .. } if target == session_id) => Some(*damage),
            ZoneOutbound::ToMany { packets, .. } | ZoneOutbound::ToSession { packets, .. } => packets.iter().find_map(|packet| match packet {
                ServerPacket::DamageIndicator { damage, damage_type: 0, object_id } if *object_id == victim.object_id() => Some(*damage),
                _ => None,
            }),
            _ => None,
        }).unwrap_or(0);
        out.extend(damage_out);
        // CounterAttack can kill the spider during Target.Attacked. Source
        // MasterVampire only runs if that same Master still exists afterward.
        if actual > 0 && self.owned_pet_life(id).as_ref() == Some(&life) {
            let amount =
                (actual as f32 * (f32::from(m.summon_skill_level) + 1.0) * 0.25) as u32 as u16;
            let vampires = &mut self.owned_pet_state_mut().vampirism;
            if let Some(v) = vampires.iter_mut().find(|v| v.caster == life.owner) {
                if v.amount == 0 {
                    v.next_tick_ms = now.saturating_add(1000);
                }
                v.amount = v.amount.wrapping_add(amount);
            } else if amount > 0 {
                vampires.push(OwnedHumanVampirism {
                    caster: life.owner,
                    amount,
                    next_tick_ms: now.saturating_add(1000),
                });
            }
            let packet = ServerPacket::ObjectEffect {
                info: ObjectEffectInfo {
                    object_id: victim.object_id(),
                    effect: CRYSTAL_SPELL_EFFECT_BLEEDING,
                    effect_type: 0,
                    delay_time: 0,
                    time: 0,
                },
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            let recipients = match &victim {
                ZoneCombatEntityRef::Player { .. } => {
                    self.player_status_recipients(victim.object_id(), &cell)
                }
                ZoneCombatEntityRef::Monster { .. } => {
                    self.native_monster_visible_recipients(victim.object_id(), &cell)
                }
            };
            out.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![packet],
            });
        }
        out
    }

    /// Actual SummonToad creates this life and its optional assigned Target.
    /// The source custom ProcessAI searches every 3s and never follows Master.
    pub(super) fn tick_owned_spitting_toad(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let Some(life) = self.owned_pet_life(id) else {
            return Vec::new();
        };
        let m = self.native_monsters[&id].clone();
        if m.ai != 61 || m.dead {
            return Vec::new();
        }
        let search_at = m
            .special_ai
            .as_ref()
            .and_then(|s| s.pet_special.as_ref())
            .map_or(0, |s| s.search_at);
        if now < search_at {
            return Vec::new();
        }
        self.native_monsters
            .get_mut(&id)
            .unwrap()
            .special_ai
            .get_or_insert_with(Default::default)
            .pet_special
            .get_or_insert_with(Default::default)
            .search_at = now.saturating_add(3000);
        let Some(target) = self.selected_owned_pet_target(id, now) else {
            return Vec::new();
        };
        let distance = zone_tile_distance(&m.position, &target.position);
        if distance == 0
            || distance > 12
            || now <= m.next_ai_ready_at_ms
            || now <= m.next_attack_ready_at_ms
            || !self.owned_pet_can_launch_now(id)
            || super::entity_combat::native_entity_attack_blocked(&m, now)
            || !self.owned_pet_can_attack(
                &life,
                &target.reference,
                EntityTargetPurpose::VisibleImpact,
                now,
            )
        {
            return Vec::new();
        }
        let Some(t) = crystal_monster_by_name(&m.name) else {
            return Vec::new();
        };
        let growth = i32::from(m.summon_skill_level);
        let damage = zone_roll_stat_range(
            t.min_dc
                .saturating_add(growth)
                .saturating_add(zone_native_monster_buff_stat_total(&m, 4)),
            t.max_dc
                .saturating_add(growth)
                .saturating_add(zone_native_monster_buff_stat_total(&m, 5)),
            now,
            id,
            61,
        );
        let direction = zone_direction_toward(&m.position, &target.position).unwrap_or(m.direction);
        let live = self.native_monsters.get_mut(&id).unwrap();
        live.direction = direction;
        live.next_ai_ready_at_ms = now.saturating_add(300);
        live.next_attack_ready_at_ms = now.saturating_add(m.attack_speed_ms).saturating_add(500);
        if damage > 0 {
            self.queue_owned_pet_hit(
                id,
                &target.reference,
                damage,
                EntityDefence::MAC,
                now.saturating_add(500).saturating_add(distance as u64 * 50),
                now,
            );
        }
        let packets = vec![
            ServerPacket::ObjectTurn {
                movement: ObjectMovement {
                    object_id: id,
                    position: m.position.clone(),
                    direction,
                },
            },
            ServerPacket::ObjectRangeAttack {
                info: ObjectRangeAttackInfo {
                    object_id: id,
                    location: m.position.clone(),
                    direction,
                    target_id: target.object_id,
                    target: target.position.clone(),
                    attack_type: 0,
                    spell: 0,
                    level: 0,
                },
            },
        ];
        self.apply_zone_object_packets(&packets, now);
        vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_action_recipients(
                &life.owner.online.session_id,
                id,
                target.object_id,
                &target.position,
            ),
            packets,
        }]
    }

    pub(super) fn launch_owned_pet_attack(
        &mut self,
        id: u32,
        target: NativeEntityTarget,
        direction: MirDirection,
        now: u64,
        ranged: bool,
    ) -> Vec<ZoneOutbound> {
        let Some(life) = self.owned_pet_life(id) else {
            return Vec::new();
        };
        let source = self.native_monsters[&id].clone();
        if !self.owned_pet_can_launch_now(id)
            || now <= source.next_attack_ready_at_ms
            || super::entity_combat::native_entity_attack_blocked(&source, now)
            || !self.owned_pet_can_attack(
                &life,
                &target.reference,
                EntityTargetPurpose::VisibleImpact,
                now,
            )
        {
            return Vec::new();
        }
        let (damage, magic) =
            zone_native_monster_player_attack_damage(&source, &target.position, now, id);
        let live = self.native_monsters.get_mut(&id).unwrap();
        live.direction = direction;
        live.next_ai_ready_at_ms = now.saturating_add(ZONE_NATIVE_MONSTER_THINK_MS);
        live.next_attack_ready_at_ms = now.saturating_add(source.attack_speed_ms);
        let delay = if ranged {
            zone_native_summon_hit_delay_ms(&source, &target.position)
        } else {
            300
        };
        self.queue_owned_pet_hit(
            id,
            &target.reference,
            damage,
            if magic || life.monster_master.is_some() && source.ai == 63 {
                EntityDefence::MAC
            } else {
                EntityDefence::ACAgility
            },
            now.saturating_add(delay),
            now,
        );
        let packet = if ranged {
            ServerPacket::ObjectRangeAttack {
                info: ObjectRangeAttackInfo {
                    object_id: id,
                    location: source.position.clone(),
                    direction,
                    target_id: target.object_id,
                    target: target.position.clone(),
                    attack_type: zone_native_monster_range_attack_type(
                        source.ai,
                        &source.position,
                        &target.position,
                    ),
                    spell: 0,
                    level: 0,
                },
            }
        } else {
            ServerPacket::ObjectAttack {
                info: ObjectAttackInfo {
                    object_id: id,
                    location: source.position.clone(),
                    direction,
                    spell: 0,
                    level: 0,
                    attack_type: 0,
                },
            }
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_action_recipients(
                &life.owner.online.session_id,
                id,
                target.object_id,
                &target.position,
            ),
            packets: vec![packet],
        }]
    }

    /// Compatibility for the internal native AI call whose target parameter is
    /// the original assigned Target. Only real wild monsters can be assigned
    /// here; player/pet PvP must already have the typed permission relationship.
    pub(super) fn assigned_owned_pet_native_target(
        &mut self,
        id: u32,
        target: NativeSummonTarget,
    ) -> Option<NativeEntityTarget> {
        let life = self.owned_pet_life(id)?;
        let reference = self
            .players
            .iter()
            .find_map(|(sid, p)| {
                (p.object_id == target.object_id)
                    .then(|| self.native_entity_player_ref(sid))
                    .flatten()
            })
            .or_else(|| self.native_entity_monster_ref(target.object_id))?;
        if self
            .native_monsters
            .get(&target.object_id)
            .is_some_and(|m| {
                m.owner_session_id.is_none()
                    && m.hostile_to_player
                    && !m.dead
                    && m.hp > 0
                    && !matches!(m.ai, 57 | 68)
            })
        {
            self.owned_pet_state_mut().targets.insert(
                id,
                OwnedPetTarget {
                    life,
                    target: reference.clone(),
                },
            );
        }
        Some(NativeEntityTarget {
            object_id: target.object_id,
            reference,
            position: target.position,
        })
    }

    pub(super) fn tick_owned_pet_idle(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let Some(life) = self.owned_pet_life(id) else {
            return Vec::new();
        };
        let source = self.native_monsters[&id].clone();
        let Some(owner) = self.owned_pet_master(&life.owner) else {
            return Vec::new();
        };
        if life.monster_master.is_none()
            && self
                .online_presence
                .get(&owner.session_id)
                .is_none_or(|p| p.key != self.key)
        {
            return Vec::new();
        }
        // CharmedSnake.ProcessRoam uses MasterTotem.Back. The causal Human's
        // current position must not turn into the child's follow destination.
        let follow = life.monster_master.as_ref().map_or_else(
            || owner.position.clone(),
            |master| {
                let parent = &self.native_monsters[&master.object_id()];
                offset_point(
                    &parent.position,
                    zone_rotated_direction(parent.direction, 4),
                    1,
                )
            },
        );
        let already_home = if life.monster_master.is_some() {
            source.position == follow
        } else {
            zone_tile_distance(&source.position, &follow) <= 2
        };
        if !self.owned_pet_can_move_now(id)
            || !zone_native_summon_can_move(&source)
            || source.ai == 18 && !shinsu_can_act(&source, now)
            || already_home
        {
            return Vec::new();
        }
        let Some(direction) = zone_direction_toward(&source.position, &follow) else {
            return Vec::new();
        };
        let destination = offset_point(&source.position, direction, 1);
        if !self.can_native_monster_occupy(id, &destination) {
            return self.turn_native_monster(id, direction, now);
        }
        let monster = self.native_monsters.get_mut(&id).unwrap();
        monster.position = destination.clone();
        monster.direction = direction;
        monster.next_ai_ready_at_ms = now.saturating_add(monster.move_speed_ms.max(300));
        monster.next_attack_ready_at_ms = monster
            .next_attack_ready_at_ms
            .max(now.saturating_add(monster.move_speed_ms));
        let packet = ServerPacket::ObjectWalk {
            movement: ObjectMovement {
                object_id: id,
                position: destination.clone(),
                direction,
            },
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        let mut out = self.diff_all_zone_object_visibility();
        out.push(ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &destination),
            packets: vec![packet],
        });
        out
    }

    pub(super) fn note_owned_pet_wild_target(
        &mut self,
        source_id: u32,
        target: &ZoneCombatEntityRef,
    ) {
        let Some(source) = self.native_entity_monster_ref(source_id) else {
            return;
        };
        if self
            .native_monsters
            .get(&source_id)
            .is_none_or(|m| m.owner_session_id.is_some())
        {
            return;
        }
        self.owned_pet_state_mut().wild_targets.insert(
            source_id,
            OwnedPetWildTarget {
                source,
                target: target.clone(),
            },
        );
    }

    /// HumanObject Focus assigns at an accepted attack, before accuracy/armour.
    pub(super) fn focus_owned_pets(&mut self, session: &SessionId, target_id: u32, now: u64) {
        let Some(owner) = self.owned_pet_owner(session) else {
            return;
        };
        if self.players[session].chat_profile.pet_mode != 4 {
            return;
        }
        let target = self
            .players
            .iter()
            .find_map(|(sid, p)| {
                (p.object_id == target_id)
                    .then(|| self.native_entity_player_ref(sid))
                    .flatten()
            })
            .or_else(|| self.native_entity_monster_ref(target_id));
        let Some(target) = target else {
            return;
        };
        if !self.entity_ref_exists(&target, false)
            || !self.owned_pet_owner_target_is_hostile(&owner, &target, now)
        {
            return;
        }
        let bindings: Vec<_> = self
            .owned_pet_state()
            .into_iter()
            .flat_map(|s| s.bindings.values())
            .filter(|b| {
                b.owner == owner
                    && b.monster_master.is_none()
                    && self.owned_pet_life(b.pet.object_id()).as_ref() == Some(*b)
            })
            .cloned()
            .collect();
        for life in bindings {
            self.owned_pet_state_mut().targets.insert(
                life.pet.object_id(),
                OwnedPetTarget {
                    life,
                    target: target.clone(),
                },
            );
        }
    }

    fn owned_pet_owner_target_is_hostile(
        &self,
        owner: &OwnedPetOwner,
        target: &ZoneCombatEntityRef,
        now: u64,
    ) -> bool {
        let attacker = &self.players[&owner.online.session_id];
        match target {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let victim = &self.players[session_id];
                if !self.conquest_player_can_attack_player(attacker, victim, now) {
                    return false;
                }
                match attacker.chat_profile.attack_mode {
                    0 => false,
                    1 => !victim
                        .chat_profile
                        .group_members
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&attacker.name)),
                    2 => !victim
                        .chat_profile
                        .guild_name
                        .as_ref()
                        .zip(attacker.chat_profile.guild_name.as_ref())
                        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b)),
                    3 => victim.chat_profile.guild_name.as_ref().is_some_and(|g| {
                        attacker
                            .chat_profile
                            .active_guild_wars
                            .iter()
                            .any(|w| w.eq_ignore_ascii_case(g))
                    }),
                    4 => {
                        victim.chat_profile.pk_points >= 200
                            || now < self.owned_pet_brown_until(victim)
                    }
                    _ => true,
                }
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let monster = &self.native_monsters[object_id];
                if monster.owner_session_id.is_none() {
                    return monster.hostile_to_player && monster_visibility_is_attackable(monster);
                }
                // MonsterObject.IsFriendlyTarget(HumanObject):2541 is deliberately
                // different from the player branch: Guild=false, EnemyGuild=true.
                let Some(victim_life) = self.owned_pet_life(*object_id) else {
                    return false;
                };
                if victim_life.owner == *owner
                    || !self.player_can_attack_owned_pet(&owner.online.session_id, *object_id, now)
                {
                    return false;
                }
                let Some(victim_owner) = self.owned_pet_master(&victim_life.owner) else {
                    return false;
                };
                match attacker.chat_profile.attack_mode {
                    1 => true,
                    2 => true,
                    4 => {
                        victim_owner.chat_profile.pk_points >= 200
                            || now < self.owned_pet_brown_until(&victim_owner)
                    }
                    _ => false,
                }
            }
        }
    }

    /// MonsterObject.IsAttackTarget(HumanObject), independently of the generic
    /// wild-monster admission gate. Focus also requires !IsFriendlyTarget.
    pub fn player_can_attack_owned_pet(&self, session: &SessionId, pet: u32, now: u64) -> bool {
        let Some(attacker_life) = self.owned_pet_owner(session) else {
            return false;
        };
        let Some(victim_life) = self.owned_pet_life(pet) else {
            return false;
        };
        let attacker = &self.players[session];
        let Some(master) = self.owned_pet_master(&victim_life.owner) else {
            return false;
        };
        let victim = &self.native_monsters[&pet];
        if !monster_visibility_is_attackable(victim) {
            return false;
        }
        if attacker.chat_profile.attack_mode == 0 {
            return false;
        }
        // Source returns for a self-owned pet before safe-zone checks.
        if attacker_life == victim_life.owner {
            return attacker.chat_profile.attack_mode == 5;
        }
        if attacker.chat_profile.in_safe_zone
            || self.pet_position_is_safe(&attacker.position)
            || self.pet_position_is_safe(&victim.position)
        {
            return false;
        }
        match attacker.chat_profile.attack_mode {
            1 => !master
                .chat_profile
                .group_members
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&attacker.name)),
            2 => !master
                .chat_profile
                .guild_name
                .as_ref()
                .zip(attacker.chat_profile.guild_name.as_ref())
                .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b)),
            3 => master
                .chat_profile
                .guild_name
                .as_ref()
                .zip(attacker.chat_profile.guild_name.as_ref())
                .is_some_and(|(_, attackers)| {
                    master
                        .chat_profile
                        .active_guild_wars
                        .iter()
                        .any(|guild| guild.eq_ignore_ascii_case(attackers))
                }),
            4 => master.chat_profile.pk_points >= 200 || now < self.owned_pet_brown_until(&master),
            _ => true,
        }
    }

    pub(super) fn enqueue_native_player_monster_hit(
        &mut self,
        hit: PendingNativeMonsterHit,
        defence: EntityDefence,
        now: u64,
    ) {
        if self
            .native_monsters
            .get(&hit.object_id)
            .is_none_or(|m| m.owner_session_id.is_none())
        {
            self.pending_native_hits.push(hit);
            return;
        }
        let Some(attacker) = self
            .owned_pet_owner(&hit.session_id)
            .filter(|owner| owner.online.object_id == hit.attacker_object_id)
        else {
            return;
        };
        if hit
            .fire_bounce
            .as_ref()
            .is_some_and(|bounce| bounce.owned_caster.as_ref() != Some(&attacker))
        {
            return;
        }
        let Some(victim) = self.owned_pet_life(hit.object_id) else {
            return;
        };
        if !self.player_can_attack_owned_pet(&hit.session_id, hit.object_id, now) {
            return;
        }
        self.owned_pet_state_mut()
            .human_hits
            .push(OwnedPetHumanHit {
                authority: OwnedPetHumanAuthority {
                    attacker,
                    victim,
                    raw_damage: hit.damage,
                },
                hit,
                defence,
            });
    }

    pub(super) fn owned_pet_human_impact_is_current(
        &self,
        authority: &OwnedPetHumanAuthority,
        hit: &PendingNativeMonsterHit,
        now: u64,
    ) -> bool {
        self.owned_pet_owner_is_current(&authority.attacker)
            && authority.attacker.online.session_id == hit.session_id
            && authority.attacker.online.object_id == hit.attacker_object_id
            && authority.victim.pet.object_id() == hit.object_id
            && self.owned_pet_life(hit.object_id).as_ref() == Some(&authority.victim)
            && self.player_can_attack_owned_pet(&hit.session_id, hit.object_id, now)
    }

    pub(super) fn tick_owned_human_pet_hits(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let pending = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
            .map(|state| std::mem::take(&mut state.human_hits))
            .unwrap_or_default();
        let mut remaining = Vec::new();
        let mut out = Vec::new();
        for mut queued in pending {
            if !self.owned_pet_human_impact_is_current(&queued.authority, &queued.hit, now) {
                continue;
            }
            if now < queued.hit.ready_at_ms {
                remaining.push(queued);
                continue;
            }
            let target = &self.native_monsters[&queued.hit.object_id];
            let attacker = &self.players[&queued.hit.session_id];
            let accuracy = attacker
                .combat_stats
                .accuracy
                .saturating_add(zone_player_buff_stat_total(attacker, 10))
                .max(0);
            let agility = target
                .defense
                .agility
                .saturating_add(zone_native_monster_buff_stat_total(target, 11))
                .max(0);
            let resist = zone_native_monster_buff_stat_total(target, 30).clamp(0, 10);
            let miss = (matches!(
                queued.defence,
                EntityDefence::MAC | EntityDefence::MACAgility
            ) && crate::runtime::combat::crystal_accuracy_roll(
                now,
                queued.hit.attacker_object_id,
                queued.hit.object_id,
                10,
            ) < resist as u64)
                || (matches!(
                    queued.defence,
                    EntityDefence::ACAgility | EntityDefence::MACAgility | EntityDefence::Agility
                ) && crate::runtime::combat::crystal_accuracy_roll(
                    now,
                    queued.hit.attacker_object_id,
                    queued.hit.object_id,
                    agility as u64 + 1,
                ) > accuracy as u64);
            queued.hit.damage = if miss {
                0
            } else {
                super::entity_combat::entity_monster_defended_damage(
                    target,
                    queued.hit.object_id,
                    queued.hit.damage,
                    queued.defence,
                    queued.hit.attacker_object_id,
                    now,
                )
            };
            if queued.hit.damage <= 0 {
                out.extend(self.entity_miss_packet(
                    queued.hit.attacker_object_id,
                    &queued.authority.victim.pet,
                ));
                continue;
            }
            out.extend(self.resolve_pending_native_monster_hit_authorized(
                queued.hit,
                now,
                Some(&queued.authority),
            ));
        }
        remaining.retain(|queued| {
            self.owned_pet_human_impact_is_current(&queued.authority, &queued.hit, now)
        });
        if let Some(state) = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
        {
            state.human_hits.extend(remaining);
        }
        out
    }

    pub(super) fn note_human_owned_pet_damage(
        &mut self,
        authority: &OwnedPetHumanAuthority,
        now: u64,
    ) {
        if authority.attacker == authority.victim.owner {
            return;
        }
        let Some(master) = self.owned_pet_master(&authority.victim.owner) else {
            return;
        };
        // MonsterObject.Attacked(HumanObject):2623 sets Brown before the later
        // redundant AtWar conditional at 2668. Preserve that actual ordering.
        if master.chat_profile.pk_points >= 200 || now <= self.owned_pet_brown_until(&master) {
            return;
        }
        let online = authority.attacker.online.clone();
        let state = self.owned_pet_state_mut();
        state.last_now_ms = state.last_now_ms.max(now);
        let clock = state
            .players
            .entry(online.session_id.clone())
            .or_insert_with(|| OwnedPetPlayerClock {
                online: online.clone(),
                last_hitter: None,
                last_hit_until_ms: 0,
                brown_until_ms: 0,
            });
        if clock.online != online {
            *clock = OwnedPetPlayerClock {
                online,
                last_hitter: None,
                last_hit_until_ms: 0,
                brown_until_ms: 0,
            };
        }
        clock.brown_until_ms = now.saturating_add(BROWN_MS);
    }

    fn owned_pet_viewer_name_colour(
        &self,
        viewer: &ZonePlayer,
        actor: &ZonePlayer,
        now: u64,
    ) -> i32 {
        // Crystal PlayerObject.GetNameColour:1849. The observer's guild decides
        // the war colour; only the actor's self colour is retained in ZonePlayer.
        if actor.chat_profile.pk_points >= 200 {
            return 0xFFFF0000_u32 as i32;
        }
        if now < self.owned_pet_brown_until(actor) {
            return 0xFF8B4513_u32 as i32;
        }
        let same_guild = actor
            .chat_profile
            .guild_name
            .as_ref()
            .zip(viewer.chat_profile.guild_name.as_ref())
            .is_some_and(|(a, v)| a.eq_ignore_ascii_case(v));
        if self.conquest_player_is_in_war_region(actor, now) {
            return if actor.chat_profile.guild_name.is_none() || same_guild {
                0xFF008000_u32 as i32
            } else {
                0xFFFFA500_u32 as i32
            };
        }
        if viewer.chat_profile.guild_name.is_some()
            && !viewer.chat_profile.active_guild_wars.is_empty()
        {
            if same_guild {
                return 0xFF0000FF_u32 as i32;
            }
            if actor.chat_profile.guild_name.as_ref().is_some_and(|guild| {
                viewer
                    .chat_profile
                    .active_guild_wars
                    .iter()
                    .any(|enemy| enemy.eq_ignore_ascii_case(guild))
            }) {
                return 0xFFFFA500_u32 as i32;
            }
        }
        if actor.chat_profile.pk_points >= 100 {
            0xFFFFFF00_u32 as i32
        } else {
            0xFFFFFFFF_u32 as i32
        }
    }

    pub(super) fn flush_owned_player_colours(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let now = now.max(self.owned_pet_state().map_or(0, |state| state.last_now_ms));
        self.note_owned_pet_time(now);
        let changed: Vec<_> = self
            .players
            .iter()
            .filter_map(|(session, actor)| {
                let clock = self.owned_pet_state()?.players.get(session)?;
                if clock.brown_until_ms == 0
                    || !self
                        .online_presence
                        .get(session)
                        .is_some_and(|presence| presence.owner == clock.online)
                {
                    return None;
                }
                let colour = self.owned_pet_viewer_name_colour(actor, actor, now);
                (actor.name_colour_argb != colour).then_some((session.clone(), colour))
            })
            .collect();
        let mut out = Vec::new();
        for (session, colour) in changed {
            self.players.get_mut(&session).unwrap().name_colour_argb = colour;
            let actor = &self.players[&session];
            out.push(ZoneOutbound::ToSession {
                session_id: session.clone(),
                packets: vec![ServerPacket::ColourChanged {
                    name_colour_argb: colour,
                }],
            });
            // HumanObject.BroadcastColourChange:2351 uses DataRange rather than
            // each viewer's colour cache. Never feed observer colours back into
            // apply_zone_object_packets, which would corrupt the actor's cache.
            for (observer_session, viewer) in &self.players {
                if observer_session == &session
                    || (viewer.position.x - actor.position.x).abs() > PET_DATA_RANGE
                    || (viewer.position.y - actor.position.y).abs() > PET_DATA_RANGE
                {
                    continue;
                }
                out.push(ZoneOutbound::ToSession {
                    session_id: observer_session.clone(),
                    packets: vec![ServerPacket::ObjectColourChanged {
                        object_id: actor.object_id,
                        name_colour_argb: self.owned_pet_viewer_name_colour(viewer, actor, now),
                    }],
                });
            }
        }
        out
    }

    pub(super) fn record_owned_pet_player_hit(
        &mut self,
        attacker_id: u32,
        victim_session: &SessionId,
        now: u64,
    ) {
        self.record_owned_pet_player_hit_kind(attacker_id, victim_session, now, true);
    }

    pub(super) fn record_owned_pet_player_hit_kind(
        &mut self,
        attacker_id: u32,
        victim_session: &SessionId,
        now: u64,
        brown_on_hit: bool,
    ) {
        let Some(victim) = self.players.get(victim_session).cloned() else {
            return;
        };
        let Some(victim_online) = self
            .online_presence
            .get(victim_session)
            .map(|p| p.owner.clone())
        else {
            return;
        };
        let (entity, online, human, player_source) = if let Some((sid, _)) = self
            .players
            .iter()
            .find(|(_, p)| p.object_id == attacker_id)
        {
            let Some(entity) = self.native_entity_player_ref(sid) else {
                return;
            };
            (
                entity,
                self.online_presence.get(sid).map(|p| p.owner.clone()),
                true,
                self.owned_human_owner_with_node(sid)
                    .map(|owner| OwnedPetPlayerDamageSource { owner, pet: None }),
            )
        } else if let Some(life) = self.owned_pet_life(attacker_id) {
            (
                life.owner.player.clone(),
                Some(life.owner.online.clone()),
                false,
                Some(OwnedPetPlayerDamageSource {
                    owner: life.owner,
                    pet: Some(life.pet),
                }),
            )
        } else {
            let Some(entity) = self.native_entity_monster_ref(attacker_id) else {
                return;
            };
            (entity, None, false, None)
        };
        let white_victim =
            victim.chat_profile.pk_points < 200 && now > self.owned_pet_brown_until(&victim);
        let at_war = online
            .as_ref()
            .is_some_and(|a| self.owned_pet_players_at_war(&a.session_id, victim_session));
        let zone = self.key.clone();
        let state = self.owned_pet_state_mut();
        state.last_now_ms = state.last_now_ms.max(now);
        let clock = state
            .players
            .entry(victim_session.clone())
            .or_insert_with(|| OwnedPetPlayerClock {
                online: victim_online.clone(),
                last_hitter: None,
                last_hit_until_ms: 0,
                brown_until_ms: 0,
            });
        if clock.online != victim_online {
            *clock = OwnedPetPlayerClock {
                online: victim_online,
                last_hitter: None,
                last_hit_until_ms: 0,
                brown_until_ms: 0,
            };
        }
        clock.last_hitter = Some(OwnedPetThreatActor {
            entity,
            online: online.clone(),
            zone,
            player_source,
        });
        clock.last_hit_until_ms = now.saturating_add(LAST_HITTER_MS);
        if brown_on_hit && human && white_victim && !at_war {
            if let Some(online) = online {
                let attacker_clock = state
                    .players
                    .entry(online.session_id.clone())
                    .or_insert_with(|| OwnedPetPlayerClock {
                        online: online.clone(),
                        last_hitter: None,
                        last_hit_until_ms: 0,
                        brown_until_ms: 0,
                    });
                if attacker_clock.online != online {
                    *attacker_clock = OwnedPetPlayerClock {
                        online,
                        last_hitter: None,
                        last_hit_until_ms: 0,
                        brown_until_ms: 0,
                    };
                }
                attacker_clock.brown_until_ms = now.saturating_add(BROWN_MS);
            }
        }
    }

    /// PlayerObject.Die reads LastHitter while the victim's Brown/life still
    /// exists. Called only once after a real living-to-dead HP transition and
    /// before life cleanup, for ordinary Human, owned pet, poison and hazards.
    pub(super) fn issue_native_player_death_receipt(
        &mut self,
        victim_session: &SessionId,
        now: u64,
    ) -> Option<ZoneOutbound> {
        let victim = self.players.get(victim_session)?;
        if !victim.dead || victim.hp != 0 {
            return None;
        }
        let clock = self.owned_pet_state()?.players.get(victim_session)?;
        if now > clock.last_hit_until_ms
            || !self
                .online_presence
                .get(victim_session)
                .is_some_and(|p| p.owner == clock.online)
        {
            return None;
        }
        let hitter = clock.last_hitter.as_ref()?;
        let source = hitter.player_source.as_ref()?;
        if hitter.entity != source.owner.player
            || hitter.online.as_ref() != Some(&source.owner.online)
            || !self.owned_pet_threat_actor_is_current(hitter)
        {
            return None;
        }
        let owner = source.owner.online.clone();
        let owner_life_generation = match source.owner.player {
            ZoneCombatEntityRef::Player {
                life_generation, ..
            } => life_generation,
            _ => return None,
        };
        let (pet_object_id, pet_incarnation) = match &source.pet {
            Some(ZoneCombatEntityRef::Monster {
                object_id,
                incarnation,
            }) => (*object_id, *incarnation),
            None => (0, 0),
            _ => return None,
        };
        let protected_by_law = self
            .npc_teleport_config
            .map(&self.key.map_file_name)
            .is_some_and(|m| m.fight)
            || self.conquest_player_is_in_war_region(victim, now)
            || self
                .players
                .get(&owner.session_id)
                .and_then(|p| p.chat_profile.guild_name.as_ref())
                .or_else(|| {
                    self.online_presence
                        .get(&owner.session_id)
                        .and_then(|p| p.guild_name.as_ref())
                })
                .zip(victim.chat_profile.guild_name.as_ref())
                .is_some_and(|(guild, _)| {
                    victim
                        .chat_profile
                        .active_guild_wars
                        .iter()
                        .any(|g| g.eq_ignore_ascii_case(guild))
                });
        let unlawful =
            !protected_by_law && victim.chat_profile.pk_points < 200 && now > clock.brown_until_ms;
        let receipt = ZoneOwnedPetPlayerKillReceipt {
            zone_key: self.key.clone(),
            sequence: self.owned_pet_state()?.kill_sequence.checked_add(1)?,
            at_ms: now,
            owner_session_id: owner.session_id.clone(),
            owner_account_id: owner.account_id.clone(),
            owner_character_index: owner.character_index,
            owner_object_id: owner.object_id,
            owner_life_generation,
            owner_online_identity: owner.encoded(),
            pet_object_id,
            pet_incarnation,
            direct_player: source.pet.is_none(),
            victim_session_id: victim_session.clone(),
            victim_object_id: victim.object_id,
            victim_life_generation: victim.life_generation,
            victim_name: victim.name.clone(),
            protected_by_law,
            unlawful,
            curse_roll: (OsRng.next_u32() & 3) as u8,
        };
        let state = self.owned_pet_state_mut();
        state.kill_sequence = receipt.sequence;
        state.issued_kills.insert(receipt.sequence, receipt.clone());
        Some(ZoneOutbound::OwnedPetPlayerKill { receipt })
    }

    pub(super) fn owned_human_spell_delay(spell: Spell, distance: i32) -> Option<u64> {
        match spell {
            Spell::Poisoning | Spell::ThunderBolt | Spell::Vampirism => Some(500),
            Spell::FireBall
            | Spell::GreatFireBall
            | Spell::FrostCrunch
            | Spell::SoulFireBall
            | Spell::FlameDisruptor
            | Spell::FireBounce
            | Spell::CatTongue => Some(500 + distance.max(0) as u64 * 50),
            _ => None,
        }
    }

    pub(super) fn queue_owned_human_player_spell(
        &mut self,
        session: &SessionId,
        target_session: &SessionId,
        spell: Spell,
        level: u8,
        damage: i32,
        item_param: u8,
        due_ms: u64,
    ) {
        let Some(caster) = self.owned_pet_owner(session) else {
            return;
        };
        let Some(target) = self.native_entity_player_ref(target_session) else {
            return;
        };
        let Some(player) = self.players.get(target_session) else {
            return;
        };
        let Some(target_online) = self
            .online_presence
            .get(target_session)
            .map(|p| p.owner.clone())
        else {
            return;
        };
        let target_position = player.position.clone();
        self.owned_pet_state_mut()
            .player_spells
            .push(OwnedHumanPlayerSpell {
                caster,
                target,
                target_online,
                target_position,
                spell,
                level,
                damage,
                item_param,
                due_ms,
                pet: None,
            });
    }

    pub(super) fn queue_owned_human_pet_poisoning(
        &mut self,
        session: &SessionId,
        object_id: u32,
        level: u8,
        damage: i32,
        item_param: u8,
        now: u64,
    ) {
        let Some(caster) = self.owned_pet_owner(session) else {
            return;
        };
        let Some(pet) = self.owned_pet_life(object_id) else {
            return;
        };
        if !self.player_can_attack_owned_pet(session, object_id, now) {
            return;
        }
        let target_position = self.native_monsters[&object_id].position.clone();
        self.owned_pet_state_mut()
            .player_spells
            .push(OwnedHumanPlayerSpell {
                caster,
                target: pet.pet.clone(),
                target_online: pet.owner.online.clone(),
                target_position,
                spell: Spell::Poisoning,
                level,
                damage,
                item_param,
                due_ms: now.saturating_add(500),
                pet: Some(pet),
            });
    }

    pub(super) fn tick_owned_human_player_spells(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let Some(state) = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
        else {
            return Vec::new();
        };
        let pending = std::mem::take(&mut state.player_spells);
        let mut out = Vec::new();
        for hit in pending {
            if !self.owned_pet_owner_is_current(&hit.caster)
                || !self.entity_ref_exists(&hit.target, false)
                || !self
                    .online_presence
                    .get(&hit.target_online.session_id)
                    .is_some_and(|p| p.owner == hit.target_online)
            {
                continue;
            }
            if now < hit.due_ms {
                self.owned_pet_state_mut().player_spells.push(hit);
                continue;
            }
            let source = self.players[&hit.caster.online.session_id].clone();
            if let Some(pet) = &hit.pet {
                if hit.spell != Spell::Poisoning
                    || self.owned_pet_life(hit.target.object_id()).as_ref() != Some(pet)
                {
                    continue;
                }
                let value = hit.damage.max(0) / 15
                    + i32::from(hit.level)
                    + 1
                    + self.owned_human_poison_attack_bonus(&source, now);
                let ticks = (hit.damage.max(0) as u64)
                    .saturating_mul(2)
                    .saturating_add((u64::from(hit.level) + 1) * 7);
                out.extend(self.apply_owned_human_poison(
                    &hit.caster,
                    &hit.target,
                    zone_taoist_poison_from_item_param(hit.item_param),
                    value,
                    ticks,
                    2000,
                    true,
                    now,
                ));
                continue;
            }
            let target = self.players[&hit.target_online.session_id].clone();
            if !self.conquest_player_can_attack_player(&source, &target, now)
                || (hit.spell != Spell::Poisoning
                    && zone_tile_distance(&target.position, &hit.target_position) > 2)
            {
                continue;
            }
            if hit.spell == Spell::Poisoning {
                let mask = zone_taoist_poison_from_item_param(hit.item_param);
                let value = hit.damage.max(0) / 15
                    + i32::from(hit.level)
                    + 1
                    + self.owned_human_poison_attack_bonus(&source, now);
                let ticks = (hit.damage.max(0) as u64)
                    .saturating_mul(2)
                    .saturating_add((u64::from(hit.level) + 1) * 7);
                out.extend(self.apply_owned_human_poison(
                    &hit.caster,
                    &hit.target,
                    mask,
                    value,
                    ticks,
                    2000,
                    true,
                    now,
                ));
            } else {
                let damage =
                    zone_player_native_incoming_damage(&target, hit.damage.max(0), true, now);
                if let Some((packets, settlements)) = self.apply_native_player_pvp_damage(
                    &target.session_id,
                    source.object_id,
                    damage,
                    1,
                    now,
                ) {
                    out.extend(settlements);
                    out.push(ZoneOutbound::ToMany {
                        session_ids: self
                            .player_status_recipients(target.object_id, &target.position),
                        packets,
                    });
                }
                if hit.spell == Spell::Vampirism && damage > 0 {
                    // HumanObject:6010 captures a ushort amount after real
                    // MAC damage. ProcessRegen:619 releases at most 10 HP,
                    // strictly after +1000 ms and then +500 ms.
                    let amount = (damage as f32 * (f32::from(hit.level) + 1.0) * 0.25) as u16;
                    let vampires = &mut self.owned_pet_state_mut().vampirism;
                    if let Some(v) = vampires.iter_mut().find(|v| v.caster == hit.caster) {
                        if v.amount == 0 {
                            v.next_tick_ms = now.saturating_add(1000);
                        }
                        v.amount = v.amount.wrapping_add(amount);
                    } else if amount > 0 {
                        vampires.push(OwnedHumanVampirism {
                            caster: hit.caster,
                            amount,
                            next_tick_ms: now.saturating_add(1000),
                        });
                    }
                }
            }
        }
        out
    }

    pub(super) fn tick_owned_human_vampirism(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let Some(state) = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
        else {
            return Vec::new();
        };
        let pending = std::mem::take(&mut state.vampirism);
        let mut out = Vec::new();
        for mut vampire in pending {
            if !self.owned_pet_owner_is_current(&vampire.caster) {
                continue;
            }
            if now > vampire.next_tick_ms {
                let amount = vampire.amount.min(10);
                vampire.amount -= amount;
                vampire.next_tick_ms = now.saturating_add(500);
                out.extend(self.apply_native_player_heal(
                    vampire.caster.online.session_id.clone(),
                    i32::from(amount),
                ));
            }
            if vampire.amount > 0 {
                self.owned_pet_state_mut().vampirism.push(vampire);
            }
        }
        out
    }

    pub(super) fn owned_human_poison_attack_bonus(&self, source: &ZonePlayer, now: u64) -> i32 {
        let power = source
            .combat_stats
            .poison_attack
            .saturating_add(zone_player_buff_stat_total(source, 23))
            .max(0);
        if power == 0 {
            0
        } else {
            zone_roll_stat_range(0, power - 1, now, source.object_id, 0xC10D)
        }
    }

    fn owned_monster_clock(&self, id: u32) -> Option<&OwnedMonsterClock> {
        let clock = self.owned_pet_state()?.monster_clocks.get(&id)?;
        (self.native_entity_monster_ref(id).as_ref() == Some(&clock.target)).then_some(clock)
    }

    fn owned_monster_clock_mut(&mut self, id: u32) -> Option<&mut OwnedMonsterClock> {
        let target = self.native_entity_monster_ref(id)?;
        let base_colour = match &self.objects.get(&id)?.packet {
            ServerPacket::ObjectMonster { info } => info.name_colour_argb,
            _ => -1,
        };
        let clocks = &mut self.owned_pet_state_mut().monster_clocks;
        if clocks.get(&id).is_some_and(|c| c.target != target) {
            clocks.remove(&id);
        }
        Some(clocks.entry(id).or_insert(OwnedMonsterClock {
            target,
            shock_until_ms: 0,
            rage_until_ms: 0,
            binding_center: false,
            tame_until_ms: 0,
            max_pet_level: 0,
            pet_experience: 0,
            frozen_by_no_pets: false,
            base_colour,
            tame_original_disposition: None,
            tame_original_hostile: false,
        }))
    }

    pub(super) fn owned_monster_rage_active(&self, id: u32, now: u64) -> bool {
        self.owned_monster_clock(id)
            .is_some_and(|c| now < c.rage_until_ms)
    }

    pub(super) fn owned_monster_frozen(&self, id: u32) -> bool {
        self.owned_monster_clock(id)
            .is_some_and(|c| c.frozen_by_no_pets)
    }

    pub(super) fn owned_monster_shocked(&self, id: u32, now: u64) -> bool {
        self.owned_monster_clock(id)
            .is_some_and(|c| now < c.shock_until_ms)
    }

    pub(super) fn owned_monster_shock_blocks_move(&self, id: u32, now: u64) -> bool {
        self.owned_monster_clock(id)
            .is_some_and(|c| c.shock_until_ms != 0 && now <= c.shock_until_ms)
    }

    fn owned_human_control_target_is_current(
        &self,
        cast: &OwnedHumanMonsterControl,
        now: u64,
    ) -> bool {
        self.owned_pet_owner_is_current(&cast.caster)
            && self.entity_ref_exists(&cast.target, false)
            && match &cast.target_owner {
                Some(owner) => self
                    .owned_pet_life(cast.target.object_id())
                    .is_some_and(|p| {
                        p.owner == *owner
                            && self.player_can_attack_owned_pet(
                                &cast.caster.online.session_id,
                                cast.target.object_id(),
                                now,
                            )
                    }),
                None => self
                    .native_monsters
                    .get(&cast.target.object_id())
                    .is_some_and(|p| {
                        p.owner_session_id.is_none() && monster_visibility_is_attackable(p)
                    }),
            }
    }

    pub(super) fn queue_owned_human_monster_control(
        &mut self,
        session: &SessionId,
        id: u32,
        spell: Spell,
        level: u8,
        now: u64,
    ) {
        let Some(caster) = self.owned_pet_owner(session) else {
            return;
        };
        let Some(target) = self.native_entity_monster_ref(id) else {
            return;
        };
        let monster = &self.native_monsters[&id];
        if spell == Spell::BindingShot
            && (monster.level > self.players[session].level.saturating_add(2)
                || self
                    .owned_monster_clock(id)
                    .is_some_and(|c| c.shock_until_ms >= now))
        {
            return;
        }
        let position = monster.position.clone();
        let target_owner = self.owned_pet_life(id).map(|p| p.owner);
        if monster.owner_session_id.is_some() && target_owner.is_none() {
            return;
        }
        let due_ms = now
            .saturating_add(500)
            .saturating_add(if spell == Spell::BindingShot {
                zone_tile_distance(&self.players[session].position, &position).max(0) as u64 * 50
            } else {
                0
            });
        self.owned_pet_state_mut()
            .monster_spells
            .push(OwnedHumanMonsterControl {
                caster,
                target,
                target_owner,
                position,
                spell,
                level: level.min(3),
                due_ms,
            });
    }

    fn refresh_owned_monster_control_packet(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let Some(clock) = self.owned_monster_clock(id).cloned() else {
            return Vec::new();
        };
        let Some(monster) = self.native_monsters.get(&id) else {
            return Vec::new();
        };
        let position = monster.position.clone();
        let colour = if now < clock.shock_until_ms {
            0xFFCD853F_u32 as i32
        } else if now < clock.rage_until_ms {
            0xFFFF0000_u32 as i32
        } else if now < monster.hallucination_until_ms {
            0xFFBA55D3_u32 as i32
        } else {
            clock.base_colour
        };
        let Some(object) = self.objects.get_mut(&id) else {
            return Vec::new();
        };
        let ServerPacket::ObjectMonster { info } = &mut object.packet else {
            return Vec::new();
        };
        let changed = info.name_colour_argb != colour;
        info.name_colour_argb = colour;
        info.shock_time = clock.shock_until_ms.saturating_sub(now) as i64;
        info.binding_shot_center = clock.binding_center;
        if !changed {
            return Vec::new();
        }
        vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &position),
            packets: vec![ServerPacket::ObjectColourChanged {
                object_id: id,
                name_colour_argb: colour,
            }],
        }]
    }

    fn shock_owned_monster(&mut self, id: u32, until: u64, now: u64) -> Vec<ZoneOutbound> {
        let Some(clock) = self.owned_monster_clock_mut(id) else {
            return Vec::new();
        };
        clock.shock_until_ms = until;
        self.clear_native_entity_target(id);
        self.owned_pet_state_mut().targets.remove(&id);
        self.refresh_owned_monster_control_packet(id, now)
    }

    pub(super) fn release_owned_monster_shock(&mut self, id: u32, now: u64) -> Vec<ZoneOutbound> {
        let Some(clock) = self.owned_monster_clock(id).cloned() else {
            return Vec::new();
        };
        if clock.shock_until_ms == 0 {
            return Vec::new();
        }
        let mut affected = vec![id];
        if clock.binding_center {
            // ReleaseBindingShot: the first shocked monster in each nearby
            // cell is woken; nested centres release their own nearby cells.
            let mut index = 0;
            while index < affected.len() {
                let centre = affected[index];
                index += 1;
                let Some(c) = self.owned_monster_clock(centre) else {
                    continue;
                };
                if !c.binding_center {
                    continue;
                }
                let centre_position = self.native_monsters[&centre].position.clone();
                for y in centre_position.y - 1..=centre_position.y + 1 {
                    for x in centre_position.x - 1..=centre_position.x + 1 {
                        if let Some((&other, _)) = self
                            .native_monsters
                            .iter()
                            .filter(|(other, m)| {
                                m.position == (Point { x, y })
                                    && !affected.contains(other)
                                    && self
                                        .owned_monster_clock(**other)
                                        .is_some_and(|c| c.shock_until_ms != 0)
                            })
                            .min_by_key(|(_, m)| m.incarnation)
                        {
                            affected.push(other);
                        }
                    }
                }
            }
        }
        let mut out = Vec::new();
        for target in affected {
            let was_center = self
                .owned_monster_clock(target)
                .is_some_and(|c| c.binding_center);
            if let Some(c) = self.owned_monster_clock_mut(target) {
                c.shock_until_ms = 0;
                c.binding_center = false;
            }
            if was_center {
                let position = self.native_monsters[&target].position.clone();
                out.push(ZoneOutbound::ToMany {
                    session_ids: self.native_monster_visible_recipients(target, &position),
                    packets: vec![ServerPacket::SetBindingShot {
                        object_id: target,
                        enabled: false,
                        value: 0,
                    }],
                });
            }
            out.extend(self.refresh_owned_monster_control_packet(target, now));
        }
        out
    }

    pub(super) fn tick_owned_human_monster_controls(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let Some(state) = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
        else {
            return Vec::new();
        };
        let pending = std::mem::take(&mut state.monster_spells);
        let mut out = Vec::new();
        for cast in pending {
            if !self.owned_human_control_target_is_current(&cast, now) {
                continue;
            }
            if now < cast.due_ms {
                self.owned_pet_state_mut().monster_spells.push(cast);
                continue;
            }
            let id = cast.target.object_id();
            let duration = (u64::from(cast.level) * 5 + 10) * 1000;
            if cast.spell == Spell::BindingShot {
                if self
                    .owned_monster_clock(id)
                    .is_some_and(|c| c.shock_until_ms >= now)
                    || zone_tile_distance(&self.native_monsters[&id].position, &cast.position) > 2
                {
                    continue;
                }
                let position = self.native_monsters[&id].position.clone();
                let caster_level = self.players[&cast.caster.online.session_id].level;
                let mut centre = None;
                let mut ids = Vec::new();
                for y in position.y - 1..=position.y + 1 {
                    for x in position.x - 1..=position.x + 1 {
                        let mut cell: Vec<_> = self
                            .native_monsters
                            .iter()
                            .filter(|(_, m)| m.position == (Point { x, y }))
                            .collect();
                        cell.sort_by_key(|(_, m)| m.incarnation);
                        ids.extend(cell.into_iter().map(|(&id, _)| id));
                    }
                }
                for target in ids {
                    let monster = &self.native_monsters[&target];
                    if monster.position == position {
                        centre = Some(target);
                    }
                    if monster.dead
                        || !monster_visibility_is_attackable(monster)
                        || monster.level > caster_level.saturating_add(2)
                        || (monster.owner_session_id.is_some()
                            && !self.player_can_attack_owned_pet(
                                &cast.caster.online.session_id,
                                target,
                                now,
                            ))
                    {
                        continue;
                    }
                    if centre.is_none() {
                        centre = Some(target);
                    }
                    out.extend(self.shock_owned_monster(target, now.saturating_add(duration), now));
                }
                if let Some(centre) = centre {
                    if let Some(c) = self.owned_monster_clock_mut(centre) {
                        c.binding_center = true;
                    }
                    self.refresh_owned_monster_control_packet(centre, now);
                    let position = self.native_monsters[&centre].position.clone();
                    out.push(ZoneOutbound::ToMany {
                        session_ids: self.native_monster_visible_recipients(centre, &position),
                        packets: vec![ServerPacket::SetBindingShot {
                            object_id: centre,
                            enabled: true,
                            value: duration as i64,
                        }],
                    });
                }
            } else if cast.spell == Spell::ElectricShock {
                if self
                    .npc_teleport_config
                    .map(&self.key.map_file_name)
                    .is_some_and(|m| m.no_pets)
                {
                    out.push(ZoneOutbound::ToSession {
                        session_id: cast.caster.online.session_id.clone(),
                        packets: vec![ServerPacket::Chat {
                            message: "You cannot summon pets on this map.".into(),
                            chat_type: mir2_protocol::ChatType::System,
                        }],
                    });
                    continue;
                }
                let mut rng = OsRng;
                if rng.next_u32() % u32::from(4 - cast.level) != 0 {
                    continue;
                }
                if cast.target_owner.as_ref() == Some(&cast.caster) || rng.next_u32() % 2 > 0 {
                    out.extend(self.shock_owned_monster(id, now.saturating_add(duration), now));
                    continue;
                }
                let monster = self.native_monsters[&id].clone();
                let Some(template) = crystal_monster_by_name(&monster.name) else {
                    continue;
                };
                let level = self.players[&cast.caster.online.session_id].level;
                if monster.level > level.saturating_add(2) || !template.can_tame {
                    continue;
                }
                let bosses = self
                    .native_monsters
                    .values()
                    .filter(|p| {
                        !p.dead
                            && p.owner_session_id.as_ref() == Some(&cast.caster.online.session_id)
                            && crystal_monster_by_name(&p.name).is_some_and(|t| t.is_boss)
                    })
                    .count();
                if template.is_boss
                    && (self.npc_teleport_config.max_boss_tames == 0
                        || bosses >= self.npc_teleport_config.max_boss_tames as usize)
                {
                    continue;
                }
                if rng.next_u32() % (u32::from(level) + 20 + u32::from(cast.level) * 5)
                    <= u32::from(monster.level) + 10
                {
                    if rng.next_u32() % 5 > 0 && monster.owner_session_id.is_none() {
                        if let Some(c) = self.owned_monster_clock_mut(id) {
                            c.rage_until_ms = now + (u64::from(rng.next_u32() % 20) + 10) * 1000;
                        }
                        self.clear_native_entity_target(id);
                        out.extend(self.refresh_owned_monster_control_packet(id, now));
                    }
                    continue;
                }
                let count = self
                    .native_monsters
                    .values()
                    .filter(|p| {
                        !p.dead
                            && p.owner_session_id.as_ref() == Some(&cast.caster.online.session_id)
                    })
                    .count();
                if count >= usize::from(cast.level) + 2 {
                    continue;
                }
                let rate = if monster.max_hp / 100 <= 2 {
                    2
                } else {
                    monster.max_hp / 100 * 2
                };
                if rng.next_u32() % rate.max(1) as u32 != 0 {
                    continue;
                }
                out.extend(self.tame_owned_monster(&cast.caster, id, cast.level, now));
            }
        }
        out
    }

    fn tame_owned_monster(
        &mut self,
        caster: &OwnedPetOwner,
        id: u32,
        level: u8,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let old_growth = self.owned_monster_clock(id).cloned();
        let old_tame = old_growth
            .as_ref()
            .filter(|c| c.tame_original_disposition.is_some());
        self.retire_entity_combat_life(id);
        self.clear_owned_pet_life(id);
        let incarnation = self.allocate_monster_incarnation();
        let pet_save = self.npc_teleport_config.pet_save;
        let monster = self.native_monsters.get_mut(&id).unwrap();
        let original_disposition = old_tame.map_or_else(
            || {
                if monster.owner_session_id.is_some() {
                    Some(if matches!(monster.ai, 1 | 2 | 3) {
                        crate::config::WorldEntityDisposition::Neutral
                    } else {
                        crate::config::WorldEntityDisposition::Hostile
                    })
                } else {
                    monster.disposition
                }
            },
            |c| c.tame_original_disposition,
        );
        let original_hostile = old_tame.map_or_else(
            || {
                if monster.owner_session_id.is_some() {
                    !matches!(monster.ai, 1 | 2 | 3)
                } else {
                    monster.hostile_to_player
                }
            },
            |c| c.tame_original_hostile,
        );
        if monster.owner_session_id.is_some() {
            monster.hp = monster.max_hp / 10;
            monster.dead = monster.hp == 0;
        }
        monster.incarnation = incarnation;
        monster.owner_session_id = Some(caster.online.session_id.clone());
        monster.master_object_id = caster.online.object_id;
        monster.owner_player_object_id = caster.online.object_id;
        monster.hostile_to_player = false;
        monster.disposition = Some(crate::config::WorldEntityDisposition::Friendly);
        // ElectricShock changes MaxPetLevel, retaining the target's PetLevel.
        // Wild monsters already start at zero; stealing a pet keeps its level.
        self.native_monster_respawns.remove(&id);
        self.register_owned_pet_life(id);
        if let Some(c) = self.owned_monster_clock_mut(id) {
            c.shock_until_ms = 0;
            c.rage_until_ms = 0;
            c.binding_center = false;
            c.max_pet_level = 1 + level * 2;
            c.pet_experience = old_growth.as_ref().map_or(0, |old| old.pet_experience);
            c.base_colour = old_growth.as_ref().map_or(-1, |old| old.base_colour);
            c.tame_original_disposition = original_disposition;
            c.tame_original_hostile = original_hostile;
            c.tame_until_ms = if pet_save {
                0
            } else {
                now.saturating_add(3_600_000)
            };
        }
        let monster = &self.native_monsters[&id];
        let position = monster.position.clone();
        let name = format!(
            "{}({})",
            monster.name, self.players[&caster.online.session_id].name
        );
        let packet = ServerPacket::ObjectName {
            object_id: id,
            name: name.clone(),
        };
        let died = monster.dead.then(|| OwnedPetDeath {
            object_id: id,
            position: position.clone(),
            direction: monster.direction,
        });
        let health = native_monster_health_percent(monster.hp, monster.max_hp);
        if let Some(o) = self.objects.get_mut(&id) {
            o.expires_at_ms = None;
            if let ServerPacket::ObjectMonster { info } = &mut o.packet {
                info.name = name;
                info.master_object_id = caster.online.object_id;
            }
        }
        let mut out = vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_visible_recipients(id, &position),
            packets: vec![
                packet,
                ServerPacket::ObjectHealth {
                    info: ObjectHealthInfo {
                        object_id: id,
                        percent: health,
                        expire: 0,
                    },
                },
            ],
        }];
        if let Some(death) = died {
            self.owned_pet_state_mut().death_packets.push(death);
        }
        out.extend(self.refresh_owned_monster_control_packet(id, now));
        out
    }

    pub(super) fn tick_owned_monster_clocks(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let ids: Vec<_> = self
            .owned_pet_state()
            .map(|s| s.monster_clocks.keys().copied().collect())
            .unwrap_or_default();
        let mut out = Vec::new();
        for id in ids {
            let Some(clock) = self.owned_monster_clock(id).cloned() else {
                if let Some(s) = self
                    .entity_combat
                    .as_mut()
                    .and_then(|s| s.owned_pet.as_mut())
                {
                    s.monster_clocks.remove(&id);
                }
                continue;
            };
            if clock.tame_until_ms != 0 && now >= clock.tame_until_ms {
                self.retire_entity_combat_life(id);
                self.clear_owned_pet_life(id);
                let incarnation = self.allocate_monster_incarnation();
                let m = self.native_monsters.get_mut(&id).unwrap();
                m.incarnation = incarnation;
                m.owner_session_id = None;
                m.master_object_id = 0;
                m.owner_player_object_id = 0;
                m.disposition = clock.tame_original_disposition;
                m.hostile_to_player = clock.tame_original_hostile;
                let position = m.position.clone();
                let name = m.name.clone();
                if let Some(o) = self.objects.get_mut(&id) {
                    if let ServerPacket::ObjectMonster { info } = &mut o.packet {
                        info.master_object_id = 0;
                        info.name = name.clone();
                    }
                }
                out.push(ZoneOutbound::ToMany {
                    session_ids: self.native_monster_visible_recipients(id, &position),
                    packets: vec![ServerPacket::ObjectName {
                        object_id: id,
                        name,
                    }],
                });
                let mut carried = clock;
                carried.target = self.native_entity_monster_ref(id).unwrap();
                carried.tame_until_ms = 0;
                carried.max_pet_level = 0;
                // RefreshNameColour uses White once a normal monster loses
                // Master; PetLevel no longer supplies its base colour.
                carried.base_colour = -1;
                self.owned_pet_state_mut()
                    .monster_clocks
                    .insert(id, carried);
                out.extend(self.refresh_owned_monster_control_packet(id, now));
                continue;
            }
            if now > clock.shock_until_ms && clock.binding_center {
                if let Some(c) = self.owned_monster_clock_mut(id) {
                    c.binding_center = false;
                }
            }
            out.extend(self.refresh_owned_monster_control_packet(id, now));
        }
        out
    }

    pub(super) fn apply_owned_human_poison(
        &mut self,
        caster: &OwnedPetOwner,
        target: &ZoneCombatEntityRef,
        mask: u16,
        mut value: i32,
        mut ticks: u64,
        tick_speed_ms: u64,
        ignore_defence: bool,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        if !self.owned_pet_owner_is_current(caster)
            || !self.entity_ref_exists(target, false)
            || !matches!(mask, CRYSTAL_POISON_GREEN | CRYSTAL_POISON_RED)
        {
            return Vec::new();
        }
        let (target_online, pet_owner, position, armour, recovery) = match target {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let victim = &self.players[session_id];
                let source = &self.players[&caster.online.session_id];
                if !self.conquest_player_can_attack_player(source, victim, now) {
                    return Vec::new();
                }
                let Some(online) = self
                    .online_presence
                    .get(session_id)
                    .map(|p| p.owner.clone())
                else {
                    return Vec::new();
                };
                let armour = zone_roll_stat_range(
                    victim
                        .combat_stats
                        .min_mac
                        .saturating_add(zone_player_buff_stat_total(victim, 2)),
                    victim
                        .combat_stats
                        .max_mac
                        .saturating_add(zone_player_buff_stat_total(victim, 3)),
                    now,
                    victim.object_id,
                    0xD07,
                );
                let recovery = victim
                    .combat_stats
                    .poison_recovery
                    .saturating_add(zone_player_buff_stat_total(victim, 34))
                    .max(0);
                (online, None, victim.position.clone(), armour, recovery)
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let Some(life) = self.owned_pet_life(*object_id) else {
                    return Vec::new();
                };
                if !self.player_can_attack_owned_pet(&caster.online.session_id, *object_id, now) {
                    return Vec::new();
                }
                let victim = &self.native_monsters[object_id];
                if victim.ai == 142 || !hell_accepts_poison(victim) {
                    return Vec::new();
                }
                let armour = zone_roll_stat_range(
                    victim
                        .defense
                        .min_mac
                        .saturating_add(zone_native_monster_buff_stat_total(victim, 2)),
                    victim
                        .defense
                        .max_mac
                        .saturating_add(zone_native_monster_buff_stat_total(victim, 3)),
                    now,
                    *object_id,
                    0xD07,
                );
                (
                    life.owner.online.clone(),
                    Some(life.owner),
                    victim.position.clone(),
                    armour,
                    0,
                )
            }
        };
        // Source ApplyPoison browns at application, independently of later
        // poison ticks. Human checks ordinary guild war / caster WarZone.
        match target {
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                if let Some(victim) = self.owned_pet_life(*object_id) {
                    self.note_human_owned_pet_damage(
                        &OwnedPetHumanAuthority {
                            attacker: caster.clone(),
                            victim,
                            raw_damage: value,
                        },
                        now,
                    );
                }
            }
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let victim = &self.players[session_id];
                let source = &self.players[&caster.online.session_id];
                let ordinary_war = source
                    .chat_profile
                    .guild_name
                    .as_ref()
                    .zip(victim.chat_profile.guild_name.as_ref())
                    .is_some_and(|(g, _)| {
                        victim
                            .chat_profile
                            .active_guild_wars
                            .iter()
                            .any(|w| w.eq_ignore_ascii_case(g))
                    });
                if victim.chat_profile.pk_points < 200
                    && now > self.owned_pet_brown_until(victim)
                    && !ordinary_war
                    && !self.conquest_player_is_in_war_region(source, now)
                {
                    let online = caster.online.clone();
                    let clock = self
                        .owned_pet_state_mut()
                        .players
                        .entry(online.session_id.clone())
                        .or_insert_with(|| OwnedPetPlayerClock {
                            online: online.clone(),
                            last_hitter: None,
                            last_hit_until_ms: 0,
                            brown_until_ms: 0,
                        });
                    clock.brown_until_ms = now.saturating_add(BROWN_MS);
                }
            }
        }
        ticks = ticks.saturating_sub(recovery as u64);
        if mask == CRYSTAL_POISON_GREEN && !ignore_defence {
            value = value.saturating_sub(armour);
        }
        if ticks == 0 || (mask == CRYSTAL_POISON_GREEN && value < 0) {
            return Vec::new();
        }
        let old = self.owned_pet_state().and_then(|s| {
            s.poisons
                .iter()
                .find(|p| p.target == *target && p.mask == mask)
        });
        if old.is_some_and(|p| {
            (mask == CRYSTAL_POISON_GREEN && p.value > value)
                || (mask != CRYSTAL_POISON_GREEN && p.ticks_left > ticks)
        }) {
            return Vec::new();
        }
        if let ZoneCombatEntityRef::Player { object_id, .. } = target {
            if self.native_periodic_player_poisons.iter().any(|p| {
                p.target == *object_id
                    && p.mask == mask
                    && ((mask == CRYSTAL_POISON_GREEN && p.value > value)
                        || (mask != CRYSTAL_POISON_GREEN
                            && p.ticks.saturating_sub(p.count) > ticks))
            }) {
                return Vec::new();
            }
            self.native_periodic_player_poisons
                .retain(|p| p.target != *object_id || p.mask != mask);
        }
        self.owned_pet_state_mut()
            .poisons
            .retain(|p| p.target != *target || p.mask != mask);
        self.owned_pet_state_mut().poisons.push(OwnedHumanPoison {
            caster: caster.clone(),
            target: target.clone(),
            target_online,
            pet_owner,
            mask,
            value: value.max(0),
            ticks_left: ticks,
            tick_speed_ms: tick_speed_ms.max(1),
            next_tick_ms: 0,
        });
        let poison = match target {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let p = self.players.get_mut(session_id).unwrap();
                p.native_status_poison |= mask;
                p.poison |= mask;
                p.native_status_poison_deadlines.insert(mask, u64::MAX - 1);
                p.native_status_poison_expires_at_ms =
                    p.native_status_poison_deadlines.values().copied().max();
                p.poison
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let m = self.native_monsters.get_mut(object_id).unwrap();
                m.entity_poison |= mask;
                if mask == CRYSTAL_POISON_GREEN {
                    m.damage_poison = 0;
                    m.damage_poison_value = 0;
                    m.damage_poison_owner_session_id = None;
                    m.damage_poison_owner_object_id = 0;
                }
                native_monster_current_poison(m)
            }
        };
        let packet = ServerPacket::ObjectPoisoned {
            object_id: target.object_id(),
            poison,
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_combat_recipients(
                caster.online.object_id,
                target.object_id(),
                &position,
            ),
            packets: vec![packet],
        }]
    }

    fn owned_human_poison_target_is_current(&self, poison: &OwnedHumanPoison) -> bool {
        self.entity_ref_exists(&poison.target, false)
            && self
                .online_presence
                .get(&poison.target_online.session_id)
                .is_some_and(|p| p.owner == poison.target_online)
            && match &poison.target {
                ZoneCombatEntityRef::Player { session_id, .. } => {
                    self.players[session_id].poison & poison.mask != 0
                }
                ZoneCombatEntityRef::Monster { object_id, .. } => {
                    self.owned_pet_life(*object_id)
                        .is_some_and(|p| Some(&p.owner) == poison.pet_owner.as_ref())
                        && self.native_monsters[object_id].entity_poison & poison.mask != 0
                }
            }
    }

    fn clear_owned_human_poison(
        &mut self,
        poison: &OwnedHumanPoison,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        // Cold restore revokes live authority but still has to remove its
        // revoked status. A newly joined epoch/life must never be cleared.
        if !self.entity_ref_exists(&poison.target, false)
            || self
                .online_presence
                .get(&poison.target_online.session_id)
                .is_some_and(|p| p.owner != poison.target_online)
        {
            return Vec::new();
        }
        let (mask, position) = match &poison.target {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let p = self.players.get_mut(session_id).unwrap();
                p.poison &= !poison.mask;
                p.native_status_poison &= !poison.mask;
                p.native_status_poison_deadlines.remove(&poison.mask);
                p.native_status_poison_expires_at_ms =
                    p.native_status_poison_deadlines.values().copied().max();
                (p.poison, p.position.clone())
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let m = self.native_monsters.get_mut(object_id).unwrap();
                m.entity_poison &= !poison.mask;
                (native_monster_current_poison(m), m.position.clone())
            }
        };
        let packet = ServerPacket::ObjectPoisoned {
            object_id: poison.target.object_id(),
            poison: mask,
        };
        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
        vec![ZoneOutbound::ToMany {
            session_ids: self.native_monster_combat_recipients(
                poison.caster.online.object_id,
                poison.target.object_id(),
                &position,
            ),
            packets: vec![packet],
        }]
    }

    pub(super) fn tick_owned_human_poisons(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let Some(state) = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
        else {
            return Vec::new();
        };
        let pending = std::mem::take(&mut state.poisons);
        let mut out = Vec::new();
        for mut poison in pending {
            if !self.owned_human_poison_target_is_current(&poison) {
                out.extend(self.clear_owned_human_poison(&poison, now));
                continue;
            }
            if !self.owned_human_source_has_node(&poison.caster) || poison.ticks_left == 0 {
                out.extend(self.clear_owned_human_poison(&poison, now));
                continue;
            }
            if now > poison.next_tick_ms {
                poison.ticks_left -= 1;
                poison.next_tick_ms = now.saturating_add(poison.tick_speed_ms);
                if poison.mask == CRYSTAL_POISON_GREEN {
                    match &poison.target {
                        ZoneCombatEntityRef::Player {
                            session_id,
                            object_id,
                            ..
                        } => {
                            self.record_owned_human_poison_last_hitter(
                                &poison.caster,
                                session_id,
                                now,
                            );
                            out.extend(self.resolve_native_player_damage_kind(
                                PendingNativePlayerHit {
                                    ready_at_ms: now,
                                    attacker_object_id: poison.caster.online.object_id,
                                    attacker_ai: 0,
                                    target_session_id: session_id.clone(),
                                    target_object_id: *object_id,
                                    damage: poison.value,
                                    magic: false,
                                },
                                now,
                                true,
                                false,
                                true,
                                false,
                            ));
                        }
                        ZoneCombatEntityRef::Monster { object_id, .. } => {
                            if let Some(result) = self.apply_native_monster_damage_internal(
                                *object_id,
                                poison.value,
                                Some(&poison.caster.online.session_id),
                                now,
                                NativeMonsterDamageCause::Indirect,
                                Some(NativeExperienceActor::PeriodicPoison {
                                    owner_object_id: poison.caster.online.object_id,
                                }),
                            ) {
                                out.extend(
                                    self.entity_monster_damage_outcome(
                                        poison.caster.online.object_id,
                                        *object_id,
                                        result,
                                        None,
                                        now,
                                    )
                                    .1,
                                );
                            }
                        }
                    }
                }
            }
            if self.owned_human_poison_target_is_current(&poison) {
                self.owned_pet_state_mut().poisons.push(poison);
            }
        }
        out
    }

    fn record_owned_human_poison_last_hitter(
        &mut self,
        caster: &OwnedPetOwner,
        target: &SessionId,
        now: u64,
    ) {
        if !self.owned_human_source_has_node(caster) {
            return;
        }
        let Some(online) = self.online_presence.get(target).map(|p| p.owner.clone()) else {
            return;
        };
        let zone = self
            .online_presence
            .get(&caster.online.session_id)
            .map_or(self.key.clone(), |p| p.key.clone());
        let actor = OwnedPetThreatActor {
            entity: caster.player.clone(),
            online: Some(caster.online.clone()),
            zone,
            player_source: Some(OwnedPetPlayerDamageSource {
                owner: caster.clone(),
                pet: None,
            }),
        };
        let clock = self
            .owned_pet_state_mut()
            .players
            .entry(target.clone())
            .or_insert(OwnedPetPlayerClock {
                online: online.clone(),
                last_hitter: None,
                last_hit_until_ms: 0,
                brown_until_ms: 0,
            });
        if clock.online != online {
            *clock = OwnedPetPlayerClock {
                online,
                last_hitter: None,
                last_hit_until_ms: 0,
                brown_until_ms: 0,
            };
        }
        clock.last_hitter = Some(actor);
        clock.last_hit_until_ms = now.saturating_add(LAST_HITTER_MS);
    }

    pub(crate) fn capture_player_owned_human_poisons(
        &self,
        session: &SessionId,
    ) -> Vec<OwnedHumanPoison> {
        self.owned_pet_state().map_or(Vec::new(), |s| {
            s.poisons.iter().filter(|p|
            matches!(&p.target,ZoneCombatEntityRef::Player{session_id,..} if session_id==session)
                && self.owned_human_poison_target_is_current(p)).cloned().collect()
        })
    }

    pub(crate) fn restore_player_owned_human_poisons(
        &mut self,
        session: &SessionId,
        poisons: Vec<OwnedHumanPoison>,
    ) {
        let Some(target) = self.native_entity_player_ref(session) else {
            return;
        };
        for mut poison in poisons {
            if poison.target != target
                || poison.pet_owner.is_some()
                || self
                    .online_presence
                    .get(session)
                    .is_none_or(|p| p.owner != poison.target_online)
                || !self.entity_ref_exists(&target, false)
            {
                continue;
            }
            poison.target = target.clone();
            let mask = poison.mask;
            let p = self.players.get_mut(session).unwrap();
            p.poison |= mask;
            p.native_status_poison |= mask;
            p.native_status_poison_deadlines.insert(mask, u64::MAX - 1);
            p.native_status_poison_expires_at_ms =
                p.native_status_poison_deadlines.values().copied().max();
            self.owned_pet_state_mut()
                .poisons
                .retain(|p| p.target != target || p.mask != mask);
            self.owned_pet_state_mut().poisons.push(poison);
        }
    }

    pub(super) fn resolve_owned_ground_player_hits(
        &mut self,
        action: &PendingNativeGroundSpellAction,
        now: u64,
    ) -> Vec<ZoneOutbound> {
        let Some(caster) = action
            .owned_caster
            .as_ref()
            .filter(|p| self.owned_pet_owner_is_current(p))
        else {
            return Vec::new();
        };
        let source = self.players[&caster.online.session_id].clone();
        let victims = self
            .players
            .values()
            .filter(|p| {
                action.locations.contains(&p.position)
                    && self.conquest_player_can_attack_player(&source, p, now)
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut out = Vec::new();
        for target in victims {
            let damage =
                zone_player_native_incoming_damage(&target, action.damage.max(0), true, now);
            if let Some((packets, settlements)) = self.apply_native_player_pvp_damage(
                &target.session_id,
                source.object_id,
                damage,
                1,
                now,
            ) {
                out.extend(settlements);
                out.push(ZoneOutbound::ToMany {
                    session_ids: self.player_status_recipients(target.object_id, &target.position),
                    packets,
                });
            }
            if action.spell == Spell::PoisonCloud {
                if let Some(reference) = self.native_entity_player_ref(&target.session_id) {
                    let value = self.owned_poison_cloud_value(&source, action.spell_param);
                    out.extend(self.apply_owned_human_poison(
                        caster,
                        &reference,
                        CRYSTAL_POISON_GREEN,
                        value,
                        12,
                        1000,
                        false,
                        now,
                    ));
                }
            }
        }
        out
    }

    pub(super) fn owned_poison_cloud_value(&self, source: &ZonePlayer, bonus: u8) -> i32 {
        source
            .combat_stats
            .min_sc
            .saturating_add(source.combat_stats.max_sc)
            .saturating_add(zone_player_buff_stat_total(source, 8))
            .saturating_add(zone_player_buff_stat_total(source, 9))
            / 2
            + i32::from(bonus)
    }

    fn owned_pet_players_at_war(
        &self,
        attacker_session: &SessionId,
        victim_session: &SessionId,
    ) -> bool {
        // PlayerObject.AtWar:10363 tests the victim's current Map.Info.Fight
        // before guild relationships. This trusted map bit never comes from a
        // client attack mode or colour.
        if self.players.contains_key(attacker_session)
            && self.players.contains_key(victim_session)
            && self
                .npc_teleport_config
                .map(&self.key.map_file_name)
                .is_some_and(|map| map.fight)
        {
            return true;
        }
        self.players
            .get(attacker_session)
            .zip(self.players.get(victim_session))
            .is_some_and(|(a, v)| {
                a.chat_profile
                    .guild_name
                    .as_ref()
                    .zip(v.chat_profile.guild_name.as_ref())
                    .is_some_and(|(ag, _)| {
                        v.chat_profile
                            .active_guild_wars
                            .iter()
                            .any(|w| w.eq_ignore_ascii_case(ag))
                    })
            })
    }

    pub(super) fn note_owned_pet_owner_monster_hit(
        &mut self,
        owner_session: &SessionId,
        target_id: u32,
        now: u64,
    ) {
        let Some(owner) = self.owned_pet_owner(owner_session) else {
            return;
        };
        let Some(target) = self.native_entity_monster_ref(target_id) else {
            return;
        };
        self.note_owned_pet_wild_target(target_id, &owner.player);
        let bindings: Vec<_> = self
            .owned_pet_state()
            .into_iter()
            .flat_map(|s| s.bindings.values())
            .filter(|b| {
                b.owner == owner
                    && b.monster_master.is_none()
                    && self.owned_pet_life(b.pet.object_id()).as_ref() == Some(*b)
            })
            .cloned()
            .collect();
        for life in bindings {
            if self
                .owned_pet_state()
                .is_some_and(|s| s.targets.contains_key(&life.pet.object_id()))
            {
                continue;
            }
            if self.owned_pet_can_attack(&life, &target, EntityTargetPurpose::Impact, now) {
                self.owned_pet_state_mut().targets.insert(
                    life.pet.object_id(),
                    OwnedPetTarget {
                        life,
                        target: target.clone(),
                    },
                );
            }
        }
    }

    pub(super) fn queue_owned_pet_hit(
        &mut self,
        pet_id: u32,
        target: &ZoneCombatEntityRef,
        damage: i32,
        defence: EntityDefence,
        due_ms: u64,
        now: u64,
    ) -> bool {
        let Some(life) = self.owned_pet_life(pet_id) else {
            return false;
        };
        if damage <= 0
            || !self.owned_pet_can_launch_now(pet_id)
            || !self.owned_pet_can_attack(&life, target, EntityTargetPurpose::VisibleImpact, now)
        {
            return false;
        }
        let target_online = match target {
            ZoneCombatEntityRef::Player { session_id, .. } => self
                .online_presence
                .get(session_id)
                .map(|p| p.owner.clone()),
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                self.owned_pet_life(*object_id).map(|l| l.owner.online)
            }
        };
        let journey_event = (self.native_monsters[&pet_id].name == "BoneFamiliar"
            && matches!(target, ZoneCombatEntityRef::Monster { .. })
            && self.players.contains_key(&life.owner.online.session_id))
        .then(|| {
            self.journey_event_draft(
                &self.players[&life.owner.online.session_id],
                ZoneJourneyEventKind::SummonSkeletonDamage,
                now,
                pet_id,
                Some(target.object_id()),
                self.owned_pet_target_position(target),
                None,
                false,
            )
        });
        self.owned_pet_state_mut().hits.push(OwnedPetHit {
            life,
            target: target.clone(),
            target_online,
            damage,
            defence,
            due_ms,
            journey_event,
        });
        true
    }

    pub(super) fn owned_pet_target_in_cell(
        &self,
        pet_id: u32,
        cell: &Point,
        now: u64,
    ) -> Option<ZoneCombatEntityRef> {
        let life = self.owned_pet_life(pet_id)?;
        self.native_monsters
            .keys()
            .filter_map(|id| self.native_entity_monster_ref(*id))
            .chain(
                self.players
                    .keys()
                    .filter_map(|sid| self.native_entity_player_ref(sid)),
            )
            .find(|target| {
                self.owned_pet_target_position(target).as_ref() == Some(cell)
                    && self.owned_pet_can_attack(
                        &life,
                        target,
                        EntityTargetPurpose::VisibleImpact,
                        now,
                    )
            })
    }

    pub(super) fn tick_owned_pet_hits(&mut self, now: u64) -> Vec<ZoneOutbound> {
        let Some(state) = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
        else {
            return Vec::new();
        };
        let pending = std::mem::take(&mut state.hits);
        let mut remaining = Vec::new();
        let mut out = Vec::new();
        for hit in pending {
            if now < hit.due_ms {
                remaining.push(hit);
                continue;
            }
            if !self.owned_pet_can_attack(&hit.life, &hit.target, EntityTargetPurpose::Impact, now)
            {
                continue;
            }
            let current_target_online = match &hit.target {
                ZoneCombatEntityRef::Player { session_id, .. } => self
                    .online_presence
                    .get(session_id)
                    .map(|p| p.owner.clone()),
                ZoneCombatEntityRef::Monster { object_id, .. } => {
                    self.owned_pet_life(*object_id).map(|l| l.owner.online)
                }
            };
            if current_target_online != hit.target_online {
                continue;
            }
            out.extend(self.resolve_owned_pet_hit(hit, now));
        }
        // A death inside resolution invalidates later hits already taken from
        // the queue; revalidation above also applies to this remaining batch.
        remaining.retain(|h| {
            self.owned_pet_life(h.life.pet.object_id()).as_ref() == Some(&h.life)
                && self.entity_ref_exists(&h.target, false)
        });
        if let Some(state) = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
        {
            state.hits.extend(remaining);
        }
        out
    }

    fn resolve_owned_pet_hit(&mut self, hit: OwnedPetHit, now: u64) -> Vec<ZoneOutbound> {
        let pet_id = hit.life.pet.object_id();
        let source = self.native_monsters[&pet_id].clone();
        if source.ai == 18 && !self.shinsu_not_expired(pet_id, now) {
            return Vec::new();
        }
        let accuracy = crystal_monster_by_name(&source.name)
            .map_or(0, |t| t.accuracy)
            .saturating_add(zone_native_monster_buff_stat_total(&source, 10))
            .max(0);
        let (agility, resist) = match &hit.target {
            ZoneCombatEntityRef::Player { session_id, .. } => {
                let p = &self.players[session_id];
                (p.combat_stats.agility, p.combat_stats.magic_resist)
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let m = &self.native_monsters[object_id];
                (
                    m.defense
                        .agility
                        .saturating_add(zone_native_monster_buff_stat_total(m, 11)),
                    zone_native_monster_buff_stat_total(m, 30),
                )
            }
        };
        let magic = matches!(hit.defence, EntityDefence::MAC | EntityDefence::MACAgility);
        let agility_defence = matches!(
            hit.defence,
            EntityDefence::ACAgility | EntityDefence::MACAgility | EntityDefence::Agility
        );
        if (magic
            && crate::runtime::combat::crystal_accuracy_roll(
                now,
                pet_id,
                hit.target.object_id(),
                10,
            ) < resist.clamp(0, 10) as u64)
            || (agility_defence
                && crate::runtime::combat::crystal_accuracy_roll(
                    now,
                    pet_id,
                    hit.target.object_id().wrapping_add(0xEA),
                    agility.max(0) as u64 + 1,
                ) > accuracy as u64)
        {
            return self.entity_miss_packet(pet_id, &hit.target);
        }
        match &hit.target {
            ZoneCombatEntityRef::Player {
                session_id,
                object_id,
                ..
            } => {
                let damage_out = self.resolve_native_player_damage(
                    PendingNativePlayerHit {
                        ready_at_ms: now,
                        attacker_object_id: pet_id,
                        attacker_ai: source.ai,
                        target_session_id: session_id.clone(),
                        target_object_id: *object_id,
                        damage: hit.damage,
                        magic,
                    },
                    now,
                    matches!(hit.defence, EntityDefence::Agility | EntityDefence::None),
                    true,
                );
                owned_pet_owner_struck_projection(damage_out, session_id, *object_id, pet_id)
            }
            ZoneCombatEntityRef::Monster { object_id, .. } => {
                let monster = self.native_monsters[object_id].clone();
                let damage = super::entity_combat::entity_monster_defended_damage(
                    &monster,
                    *object_id,
                    hit.damage,
                    hit.defence,
                    pet_id,
                    now,
                );
                if damage <= 0 {
                    return self.entity_miss_packet(pet_id, &hit.target);
                }
                let result = self.apply_native_monster_damage_internal(
                    *object_id,
                    damage,
                    Some(&hit.life.owner.online.session_id),
                    now,
                    NativeMonsterDamageCause::Direct,
                    Some(NativeExperienceActor::Object {
                        object_id: pet_id,
                        force_owner: false,
                    }),
                );
                let Some(result) = result else {
                    return Vec::new();
                };
                let (actual, mut out) = self.entity_monster_damage_outcome(
                    pet_id,
                    *object_id,
                    result,
                    Some(pet_id),
                    now,
                );
                if actual > 0 {
                    out.extend(self.release_owned_monster_shock(*object_id, now));
                    if let Some(packet) =
                        self.apply_native_charmed_snake_hit_paralysis(pet_id, *object_id, now)
                    {
                        self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
                        out.push(ZoneOutbound::ToMany {
                            session_ids: self
                                .native_monster_visible_recipients(*object_id, &monster.position),
                            packets: vec![packet],
                        });
                    }
                }
                if actual > 0 {
                    if let Some(victim_life) = self.owned_pet_life(*object_id) {
                        if self.owned_pet_can_attack(
                            &victim_life,
                            &hit.life.pet,
                            EntityTargetPurpose::Impact,
                            now,
                        ) {
                            self.owned_pet_state_mut().targets.insert(
                                *object_id,
                                OwnedPetTarget {
                                    life: victim_life,
                                    target: hit.life.pet.clone(),
                                },
                            );
                        }
                    } else {
                        self.set_native_entity_target(*object_id, &hit.life.pet, now);
                    }
                    if let Some(receipt) = hit.journey_event.as_ref().and_then(|draft| {
                        self.commit_journey_event(
                            draft,
                            now,
                            Some(actual as u32),
                            Some(monster.position),
                            None,
                            None,
                        )
                    }) {
                        out.push(ZoneOutbound::JourneyEvent { receipt });
                    }
                }
                out
            }
        }
    }

    pub(super) fn clear_owned_pet_life(&mut self, id: u32) {
        // Manager's atomic map transfer retains the spawned global Node. Only
        // actual logout/revoke invalidates an earlier causal Human hitter.
        let source_node_survives = self
            .online_presence
            .values()
            .any(|p| p.owner.object_id == id);
        let preserve_transferred_pets = self.owned_pet_state().is_some_and(|s| {
            s.detached_masters.values().any(|master| {
                master.object_id == id
                    && self
                        .online_presence
                        .get(&master.session_id)
                        .is_some_and(|p| {
                            p.owner.matches_player(master)
                                && !p.dead
                                && p.life_generation == master.life_generation
                        })
            })
        });
        let death_time = self
            .players
            .values()
            .find(|p| p.object_id == id && p.dead)
            .map(|p| {
                p.last_damaged_at_ms
                    .max(self.owned_pet_state().map_or(0, |s| s.last_now_ms))
            });
        let dead_owner = self
            .players
            .values()
            .find(|p| p.object_id == id && p.dead)
            .map(|p| p.session_id.clone());
        let pets: Vec<_> = dead_owner
            .as_ref()
            .map(|sid| {
                self.native_monsters
                    .iter()
                    .filter_map(|(&pet_id, m)| {
                        (m.owner_session_id.as_ref() == Some(sid)
                            && zone_native_summon_owner_player_object_id(m) == id
                            && !m.dead)
                            .then_some(pet_id)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut deaths = Vec::new();
        for pet_id in &pets {
            if let Some(pet) = self.native_monsters.get_mut(pet_id) {
                pet.hp = 0;
                pet.dead = true;
                deaths.push(OwnedPetDeath {
                    object_id: *pet_id,
                    position: pet.position.clone(),
                    direction: pet.direction,
                });
            }
        }
        if !deaths.is_empty() {
            self.owned_pet_state_mut();
        }
        if let Some(state) = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
        {
            let affected = |object: u32| object == id || pets.contains(&object);
            state.targets.retain(|_, r| {
                !affected(r.life.pet.object_id())
                    && !r
                        .life
                        .monster_master
                        .as_ref()
                        .is_some_and(|m| affected(m.object_id()))
                    && (!affected(r.life.owner.online.object_id) || preserve_transferred_pets)
                    && !affected(r.target.object_id())
            });
            state.hits.retain(|h| {
                !affected(h.life.pet.object_id())
                    && !h
                        .life
                        .monster_master
                        .as_ref()
                        .is_some_and(|m| affected(m.object_id()))
                    && (!affected(h.life.owner.online.object_id) || preserve_transferred_pets)
                    && !affected(h.target.object_id())
            });
            state.human_hits.retain(|h| {
                !affected(h.authority.attacker.online.object_id)
                    && !affected(h.authority.victim.pet.object_id())
                    && !h
                        .authority
                        .victim
                        .monster_master
                        .as_ref()
                        .is_some_and(|m| affected(m.object_id()))
                    && !affected(h.authority.victim.owner.online.object_id)
            });
            state.player_spells.retain(|h| {
                !affected(h.caster.online.object_id) && !affected(h.target.object_id())
            });
            state.poisons.retain(|p| !affected(p.target.object_id()));
            state.monster_spells.retain(|p| {
                !affected(p.caster.online.object_id) && !affected(p.target.object_id())
            });
            state.monster_clocks.retain(|target, _| !affected(*target));
            state
                .vampirism
                .retain(|p| !affected(p.caster.online.object_id));
            state.bindings.retain(|_, b| {
                !affected(b.pet.object_id())
                    && !b
                        .monster_master
                        .as_ref()
                        .is_some_and(|m| affected(m.object_id()))
                    && (!affected(b.owner.online.object_id) || preserve_transferred_pets)
            });
            state
                .wild_targets
                .retain(|_, r| !affected(r.source.object_id()) && !affected(r.target.object_id()));
            for clock in state.players.values_mut() {
                if (clock.online.object_id == id && !preserve_transferred_pets)
                    || clock.last_hitter.as_ref().is_some_and(|h| {
                        affected(h.entity.object_id())
                            && death_time.is_none()
                            && !source_node_survives
                    })
                {
                    clock.last_hitter = None;
                    clock.last_hit_until_ms = 0;
                }
                if clock.online.object_id == id {
                    if let Some(now) = death_time {
                        clock.brown_until_ms = now;
                    }
                }
            }
            state.death_packets.extend(deaths);
        }
    }

    pub(super) fn flush_owned_pet_deaths(&mut self, now: u64) -> Vec<ZoneOutbound> {
        self.note_owned_pet_time(now);
        let now = self
            .owned_pet_state()
            .map_or(now, |s| s.last_now_ms.max(now));
        let packets = self
            .entity_combat
            .as_mut()
            .and_then(|s| s.owned_pet.as_mut())
            .map(|s| std::mem::take(&mut s.death_packets))
            .unwrap_or_default();
        let mut out = Vec::new();
        for death in packets {
            let recipients =
                self.native_monster_visible_recipients(death.object_id, &death.position);
            self.mark_vampire_death(death.object_id, now);
            self.release_vampire_master_after_death(death.object_id);
            self.retire_native_source_object(death.object_id);
            let packet = ServerPacket::ObjectDied {
                info: ObjectDiedInfo {
                    object_id: death.object_id,
                    location: death.position,
                    direction: death.direction,
                    kind: 0,
                },
            };
            self.apply_zone_object_packets(std::slice::from_ref(&packet), now);
            let dead_delay = self
                .native_monsters
                .get(&death.object_id)
                .map(|pet| owned_pet_dead_delay_ms(pet.ai));
            if let Some(object) = self.objects.get_mut(&death.object_id) {
                object.expires_at_ms = dead_delay.map(|delay| now.saturating_add(delay));
            }
            out.push(ZoneOutbound::ToMany {
                session_ids: recipients,
                packets: vec![packet],
            });
        }
        out
    }

    pub(crate) fn player_owned_pet_clock(
        &self,
        session: &SessionId,
    ) -> Option<OwnedPetPlayerClock> {
        self.owned_pet_state()?
            .players
            .get(session)
            .filter(|clock| {
                self.online_presence
                    .get(session)
                    .is_some_and(|p| p.owner == clock.online)
            })
            .cloned()
    }

    pub(crate) fn restore_player_owned_pet_clock(
        &mut self,
        session: &SessionId,
        clock: OwnedPetPlayerClock,
    ) {
        if self
            .online_presence
            .get(session)
            .is_some_and(|p| p.owner == clock.online && &clock.online.session_id == session)
        {
            self.owned_pet_state_mut()
                .players
                .insert(session.clone(), clock);
        }
    }

    pub(crate) fn issued_owned_pet_kill_is_current(
        &self,
        receipt: &ZoneOwnedPetPlayerKillReceipt,
    ) -> bool {
        // Issued only at real Death with the typed owner/pet/victim lifetimes.
        // The authenticated ledger remains the immutable settlement proof
        // after logout or cold recovery; live damage still needs presence.
        self.key == receipt.zone_key
            && self
                .owned_pet_state()
                .and_then(|s| s.issued_kills.get(&receipt.sequence))
                == Some(receipt)
    }

    pub(crate) fn acknowledge_owned_pet_kill(&mut self, receipt: &ZoneOwnedPetPlayerKillReceipt) {
        if self.issued_owned_pet_kill_is_current(receipt) {
            self.owned_pet_state_mut()
                .issued_kills
                .remove(&receipt.sequence);
        }
    }
}

/// HumanObject.Attacked(MonsterObject) enqueues Struck to its own connection and
/// broadcasts ObjectStruck to observers. Preserve the common HP/proc resolver,
/// while avoiding a duplicate owner flinch from the observer packet.
fn owned_pet_owner_struck_projection(
    out: Vec<ZoneOutbound>,
    victim: &SessionId,
    victim_id: u32,
    pet_id: u32,
) -> Vec<ZoneOutbound> {
    let mut projected = Vec::new();
    for outbound in out {
        match outbound {
            ZoneOutbound::ToMany { mut session_ids, packets } if session_ids.contains(victim)
                && packets.iter().any(|p|matches!(p,ServerPacket::ObjectStruck {info}if info.object_id==victim_id && info.attacker_id==pet_id)) => {
                session_ids.retain(|sid|sid != victim);
                let mut owner_packets=vec![ServerPacket::Struck { info:mir2_protocol::StruckInfo {attacker_id:pet_id} }];
                owner_packets.extend(packets.iter().filter(|p|!matches!(p,ServerPacket::ObjectStruck{info}if info.object_id==victim_id && info.attacker_id==pet_id)).cloned());
                projected.push(ZoneOutbound::ToSession {session_id:victim.clone(),packets:owner_packets});
                if !session_ids.is_empty() {projected.push(ZoneOutbound::ToMany {session_ids,packets});}
            }
            other=>projected.push(other),
        }
    }
    projected
}

#[cfg(test)]
#[path = "owned_pet_combat_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "owned_pet_immediate_master_tests.rs"]
mod owned_pet_immediate_master_tests;
