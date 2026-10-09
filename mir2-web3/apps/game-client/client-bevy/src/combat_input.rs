//! Renderer-neutral ordinary combat decisions. Requests never establish server acceptance.
use crate::{
    entities::EntityKind,
    skill_model::{SkillCastSelection, SkillModel},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillKeyBinding {
    pub function: String,
    pub group: String,
    pub description: String,
    pub key: String,
    pub alt: u8,
    pub ctrl: u8,
    pub shift: u8,
    pub tilde: u8,
}
pub fn skill_function_slot(function: &str) -> Option<u8> {
    (1..=16).find(|slot| {
        function
            == format!(
                "Bar{}Skill{}",
                if *slot <= 8 { 1 } else { 2 },
                (*slot - 1) % 8 + 1
            )
    })
}
pub fn default_skill_bindings() -> Vec<SkillKeyBinding> {
    let all: Vec<SkillKeyBinding> = serde_json::from_str(
        include_str!("crystal_ui/keyboard_defaults.json").trim_start_matches('\u{feff}'),
    )
    .expect("source controlled key descriptions");
    all.into_iter()
        .filter(|b| skill_function_slot(&b.function).is_some())
        .collect()
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CombatModifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub tilde: bool,
}
impl SkillKeyBinding {
    pub fn matches(&self, key: &str, m: CombatModifiers) -> bool {
        self.key != "None"
            && self.key == key
            && !(key == "Insert" && m.ctrl && !m.alt && !m.shift)
            && [
                (self.alt, m.alt),
                (self.ctrl, m.ctrl),
                (self.shift, m.shift),
                (self.tilde, m.tilde),
            ]
            .into_iter()
            .all(|(rule, pressed)| rule == 2 || rule == u8::from(pressed))
    }
}
pub fn matching_skill_slots(
    bindings: &[SkillKeyBinding],
    key: &str,
    modifiers: CombatModifiers,
) -> Vec<u8> {
    bindings
        .iter()
        .filter(|b| b.matches(key, modifiers))
        .filter_map(|b| skill_function_slot(&b.function))
        .collect()
}
pub fn skill_ready(
    skills: &SkillModel,
    slot: u8,
    hp: i32,
    mp: i32,
    remaining_ms: impl Fn(u32) -> u32,
) -> Option<SkillCastSelection> {
    let selection = skills.selection_for_shortcut(slot)?;
    if selection.cast_kind.as_deref() == Some("passive")
        || selection.cooldown_remaining_ticks > 0
        || remaining_ms(selection.skill_id) > 0
        || hp <= 0
        || selection
            .mp_cost
            .is_some_and(|cost| mp < i32::try_from(cost).unwrap_or(i32::MAX))
        || selection
            .spell
            .as_ref()
            .is_none_or(|spell| spell.trim().is_empty())
    {
        return None;
    }
    Some(selection)
}
pub fn next_toggle_state(can_use: Option<bool>) -> i8 {
    if can_use == Some(true) {
        0
    } else {
        1
    }
}
pub fn attack_request_interval_ms(level: u32, speed: i64) -> u64 {
    (1400_i64
        .saturating_sub(speed.saturating_mul(60))
        .saturating_sub((i64::from(level) * 14).min(370)))
    .max(550) as u64
}
pub fn direction_toward(origin: (i32, i32), target: (i32, i32)) -> Option<&'static str> {
    match (
        (i64::from(target.0) - i64::from(origin.0)).signum(),
        (i64::from(target.1) - i64::from(origin.1)).signum(),
    ) {
        (0, -1) => Some("up"),
        (1, -1) => Some("upright"),
        (1, 0) => Some("right"),
        (1, 1) => Some("downright"),
        (0, 1) => Some("down"),
        (-1, 1) => Some("downleft"),
        (-1, 0) => Some("left"),
        (-1, -1) => Some("upleft"),
        _ => None,
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CombatActor {
    pub object_id: String,
    pub kind: EntityKind,
    pub x: i32,
    pub y: i32,
    pub direction: Option<String>,
    pub dead: bool,
    pub master_object_id: u32,
    pub ai: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpellAim {
    pub direction: String,
    pub target_id: u32,
    pub location: (i32, i32),
}
#[derive(Debug, Default, Clone)]
pub struct SpellTargetMemory {
    epoch: u64,
    owner: Option<String>,
    selected: Option<u32>,
    pub magic: Option<String>,
}
impl SpellTargetMemory {
    pub fn session(&mut self, epoch: u64) {
        if self.epoch != epoch {
            *self = Self {
                epoch,
                ..Default::default()
            };
        }
    }
    pub fn aim(
        &mut self,
        spell: &str,
        player: &CombatActor,
        entities: &[CombatActor],
        selected: Option<u32>,
        hovered_id: Option<&str>,
        cursor: Option<(i32, i32)>,
    ) -> Option<SpellAim> {
        if self.owner.as_deref() != Some(&player.object_id) {
            *self = Self {
                owner: Some(player.object_id.clone()),
                epoch: self.epoch,
                ..Default::default()
            };
        }
        let find = |id: &str| entities.iter().find(|e| e.object_id == id);
        if self.selected != selected {
            self.selected = selected;
            self.magic = selected
                .and_then(|id| find(&id.to_string()))
                .filter(|e| e.kind == EntityKind::Monster && !matches!(e.ai, 6 | 64 | 70))
                .map(|e| e.object_id.clone());
        }
        let hovered = hovered_id.and_then(find);
        let alive = |e: &&CombatActor| e.kind != EntityKind::Npc && !e.dead;
        let target = match spell {
            "FireBall" | "GreatFireBall" | "ElectricShock" | "Poisoning" | "ThunderBolt"
            | "FlameDisruptor" | "SoulFireBall" | "TurnUndead" | "FrostCrunch" | "Vampirism"
            | "Revelation" | "Entrapment" | "Hallucination" | "DarkBody" | "FireBounce"
            | "MeteorShower" | "StraightShot" | "DoubleShot" | "ElementalShot"
            | "DelayedExplosion" | "BindingShot" | "VampireShot" | "PoisonShot" | "CrippleShot"
            | "NapalmShot" | "SummonVampire" | "SummonToad" | "SummonSnakes" => {
                let target = hovered
                    .filter(alive)
                    .or_else(|| self.magic.as_deref().and_then(find).filter(alive));
                if let Some(e) = target.filter(|e| e.kind == EntityKind::Monster) {
                    self.magic = Some(e.object_id.clone());
                }
                target
            }
            "Purification" | "Healing" | "UltimateEnhancer" | "EnergyShield" | "PetEnhancer" => {
                Some(
                    hovered
                        .filter(alive)
                        .filter(|e| e.kind != EntityKind::Monster || e.master_object_id != 0)
                        .unwrap_or(player),
                )
            }
            "Reincarnation" => hovered.filter(|e| {
                matches!(e.kind, EntityKind::Player | EntityKind::SelfPlayer) && e.dead
            }),
            "Stonetrap" | "FireBang" | "MassHiding" | "FireWall" | "TrapHexagon"
            | "HealingCircle" | "CatTongue" | "PoisonCloud" | "Blizzard" | "MeteorStrike"
            | "Trap" => hovered.filter(alive),
            _ => None,
        };
        let location = target.map(|e| (e.x, e.y)).or(cursor)?;
        let direction = if spell == "FlashDash" {
            player.direction.as_deref().unwrap_or("down")
        } else {
            let point = target
                .filter(|e| e.object_id != player.object_id)
                .map(|e| (e.x, e.y))
                .or(cursor);
            point
                .and_then(|p| direction_toward((player.x, player.y), p))
                .unwrap_or(player.direction.as_deref().unwrap_or("down"))
        }
        .to_owned();
        Some(SpellAim {
            direction,
            target_id: target.and_then(|e| e.object_id.parse().ok()).unwrap_or(0),
            location,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChaseDecision {
    Clear,
    Wait,
    Attack,
    Approach,
}
/// A pending movement ACK does not bar an already in-range action. The adapter
/// supplies the actual motion/pacing gate and performs existing neighbour path finding.
pub fn chase_decision(
    valid: bool,
    distance: u64,
    archer_weapon: bool,
    ranged: bool,
    request_ready: bool,
    movement_ready: bool,
    planning_in_range: bool,
) -> ChaseDecision {
    if !valid {
        ChaseDecision::Clear
    } else if distance <= if ranged { 9 } else { 1 } {
        if request_ready {
            ChaseDecision::Attack
        } else {
            ChaseDecision::Wait
        }
    } else if archer_weapon || !movement_ready || planning_in_range {
        ChaseDecision::Wait
    } else {
        ChaseDecision::Approach
    }
}
pub fn attack_neighbours(target: (i32, i32)) -> Vec<(i32, i32)> {
    let mut cells = Vec::with_capacity(8);
    for dx in -1..=1 {
        for dy in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            if let (Some(x), Some(y)) = (target.0.checked_add(dx), target.1.checked_add(dy)) {
                cells.push((x, y));
            }
        }
    }
    cells
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CombatCommand {
    Attack {
        #[serde(rename = "objectId")]
        object_id: u32,
    },
    AttackDirection {
        direction: String,
        spell: Option<u8>,
    },
    RangeAttack {
        direction: String,
        x: i32,
        y: i32,
        #[serde(rename = "targetId")]
        target_id: u32,
        #[serde(rename = "targetX")]
        target_x: i32,
        #[serde(rename = "targetY")]
        target_y: i32,
    },
    Harvest {
        direction: String,
    },
    Magic {
        #[serde(rename = "objectId")]
        object_id: u32,
        spell: String,
        direction: String,
        #[serde(rename = "targetId")]
        target_id: u32,
        x: i32,
        y: i32,
        #[serde(rename = "spellTargetLock")]
        spell_target_lock: bool,
    },
    SpellToggle {
        spell: String,
        #[serde(rename = "toggleState")]
        toggle_state: i8,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CrystalWorldClickTarget {
    pub kind: EntityKind,
    pub object_id: u32,
    pub x: i32,
    pub y: i32,
    pub dead: Option<bool>,
    pub ai: Option<u8>,
    pub harvestable: Option<bool>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CrystalWorldClickContext {
    pub in_game: bool,
    pub world_actions_blocked: bool,
    pub player_hp: Option<i32>,
    pub player_max_hp: Option<i32>,
    pub player_x: i32,
    pub player_y: i32,
    pub target: Option<CrystalWorldClickTarget>,
    pub alt: bool,
    pub shift: bool,
    pub class: Option<String>,
    pub has_class_weapon: Option<bool>,
    pub riding_mount: Option<bool>,
    pub dazed: Option<bool>,
    pub fishing: Option<bool>,
    pub target_in_range: Option<bool>,
}
pub fn resolve_world_click(context: &CrystalWorldClickContext) -> Option<CombatCommand> {
    if !context.in_game
        || context.world_actions_blocked
        || context.player_max_hp? <= 0
        || context.player_hp? <= 0
    {
        return None;
    }
    let target = context.target?;
    let direction = direction_toward((context.player_x, context.player_y), (target.x, target.y));
    if context.alt {
        if target.kind != EntityKind::Monster
            || target.object_id == 0
            || target.ai == Some(70)
            || target.ai.is_none()
            || context.riding_mount != Some(false)
        {
            return None;
        }
        return Some(CombatCommand::Harvest {
            direction: direction.unwrap_or("up").to_owned(),
        });
    }
    let direction = direction?;
    if context.shift {
        if context.dazed != Some(false)
            || target.kind != EntityKind::Monster
            || target.object_id == 0
            || target.ai == Some(70)
            || target.ai.is_none()
        {
            return None;
        }
        if context
            .class
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case("Archer"))
        {
            if context.has_class_weapon != Some(true)
                || context.riding_mount != Some(false)
                || context.target_in_range != Some(true)
            {
                return None;
            }
            return Some(CombatCommand::RangeAttack {
                direction: direction.to_owned(),
                x: context.player_x,
                y: context.player_y,
                target_id: target.object_id,
                target_x: target.x,
                target_y: target.y,
            });
        }
        return Some(CombatCommand::AttackDirection {
            direction: direction.to_owned(),
            spell: None,
        });
    }
    if target.kind != EntityKind::Monster
        || target.object_id == 0
        || target.dead != Some(false)
        || target.ai == Some(70)
        || target.ai.is_none()
        || !context
            .class
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case("Archer"))
        || context.has_class_weapon != Some(true)
        || context.riding_mount != Some(false)
        || context.fishing != Some(false)
        || context.target_in_range != Some(true)
    {
        return None;
    }
    Some(CombatCommand::RangeAttack {
        direction: direction.to_owned(),
        x: context.player_x,
        y: context.player_y,
        target_id: target.object_id,
        target_x: target.x,
        target_y: target.y,
    })
}

#[cfg(test)]
#[path = "combat_input_tests.rs"]
mod tests;

use crate::skill_model::SkillModelAuthority;
use crate::skill_page_state::{
    exact_skill, normalize_raw_skills, PortableRequestNamespace, SkillCooldownClock,
    MAX_REQUEST_RUN, MAX_SAFE_ID,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CombatIdentity {
    pub controller_run: u64,
    pub connection_generation: u64,
    pub session_generation: u64,
    pub owner_revision: u64,
    pub player_object_id: u32,
}
impl CombatIdentity {
    pub fn valid(self) -> bool {
        self.controller_run > 0
            && self.controller_run <= MAX_REQUEST_RUN
            && self.connection_generation > 0
            && self.connection_generation <= MAX_SAFE_ID
            && self.session_generation > 0
            && self.session_generation <= MAX_SAFE_ID
            && self.owner_revision <= MAX_SAFE_ID
            && self.player_object_id > 0
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CombatTiming {
    pub spell: String,
    pub sequence: u64,
    pub observed_at_ms: u64,
    pub delay_ms: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReachableNeighbour {
    pub x: i32,
    pub y: i32,
    pub cost: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CombatSnapshot {
    pub identity: CombatIdentity,
    pub revision: u64,
    pub model_revision: u64,
    pub map: String,
    pub enabled: bool,
    pub now_ms: u64,
    pub player: CombatActor,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub level: u32,
    pub attack_speed: i64,
    pub learned: Vec<serde_json::Value>,
    pub actors: Vec<CombatActor>,
    pub timing: Vec<CombatTiming>,
    pub click_targets: Vec<CrystalWorldClickTarget>,
    pub selected_object_id: Option<u32>,
    pub hovered_object_id: Option<String>,
    pub cursor: Option<(i32, i32)>,
    pub class: Option<String>,
    pub has_class_weapon: Option<bool>,
    pub riding_mount: Option<bool>,
    pub fishing: Option<bool>,
    pub dazed: Option<bool>,
    pub motion_remaining_ms: u32,
    pub movement_ready: bool,
    pub planning_position: Option<(i32, i32)>,
    pub neighbours: Vec<ReachableNeighbour>,
    pub spell_lock_key: String,
    pub pointer_spell_lock: bool,
    pub bindings: Option<Vec<SkillKeyBinding>>,
}
impl CombatSnapshot {
    pub fn validate(&self) -> bool {
        let unique = self
            .actors
            .iter()
            .map(|e| e.object_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        self.identity.valid()
            && self.revision > 0
            && self.revision <= MAX_SAFE_ID
            && self.model_revision > 0
            && self.model_revision <= MAX_SAFE_ID
            && self.now_ms <= MAX_SAFE_ID
            && !self.map.is_empty()
            && self.map.len() <= 256
            && self.player.kind == EntityKind::SelfPlayer
            && self.player.object_id == self.identity.player_object_id.to_string()
            && self.max_hp > 0
            && self.learned.len() <= 512
            && self.learned.iter().all(serde_json::Value::is_object)
            && self.actors.len() <= 512
            && unique.len() == self.actors.len()
            && self
                .actors
                .iter()
                .all(|e| e.object_id.parse::<u32>().is_ok_and(|id| id > 0))
            && self.click_targets.len() <= 512
            && self.timing.len() <= 512
            && self
                .timing
                .iter()
                .map(|t| t.spell.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len()
                == self.timing.len()
            && self.timing.iter().all(|t| {
                !t.spell.is_empty()
                    && t.spell.len() <= 128
                    && t.sequence <= MAX_SAFE_ID
                    && t.observed_at_ms <= self.now_ms
            })
            && self.neighbours.len() <= 8
            && self.spell_lock_key.len() <= 64
            && self.bindings.as_ref().is_none_or(|b| {
                b.len() <= 16
                    && b.iter().all(|b| {
                        skill_function_slot(&b.function).is_some()
                            && b.key.len() <= 64
                            && [b.alt, b.ctrl, b.shift, b.tilde].iter().all(|v| *v <= 2)
                    })
            })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum CombatAction {
    Key {
        key: String,
        modifiers: CombatModifiers,
        #[serde(default)]
        repeat: bool,
    },
    SkillKey {
        #[serde(rename = "skillKey")]
        skill_key: String,
    },
    Slot {
        slot: u8,
        cursor: Option<(i32, i32)>,
    },
    Select {
        #[serde(rename = "objectId")]
        object_id: u32,
    },
    Cancel,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CombatEdge {
    pub identity: CombatIdentity,
    pub model_revision: u64,
    pub map: String,
    pub sequence: u64,
    pub action: CombatAction,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CombatProof {
    pub identity: CombatIdentity,
    pub model_revision: u64,
    pub map: String,
    pub sequence: u64,
    pub edge_sequence: u64,
    pub movement_block_ms: u32,
    pub slot: Option<u8>,
    pub skill_id: Option<u32>,
    pub spell: Option<String>,
    pub target_id: Option<u32>,
    pub command: CombatCommand,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CombatOutput {
    Wire {
        proof: CombatProof,
    },
    Approach {
        identity: CombatIdentity,
        #[serde(rename = "modelRevision")]
        model_revision: u64,
        map: String,
        sequence: u64,
        #[serde(rename = "edgeSequence")]
        edge_sequence: u64,
        #[serde(rename = "targetId")]
        target_id: u32,
        x: i32,
        y: i32,
    },
    Clear,
}
/// Namespace, edge high-water and retired run survive presentation/owner clears.
#[derive(Default)]
pub struct CombatController {
    pub snapshot: Option<CombatSnapshot>,
    skills: SkillModel,
    clock: SkillCooldownClock,
    memory: SpellTargetMemory,
    namespace: Option<PortableRequestNamespace>,
    highest_run: u64,
    edge_high: u64,
    last_now: u64,
    identity_high: Option<CombatIdentity>,
    revision_high: u64,
    chase: Option<u32>,
    next_attack_at: u64,
    next_approach_at: u64,
    last_chase_point: Option<(i32, i32)>,
}
impl CombatController {
    pub fn clear(&mut self) {
        self.snapshot = None;
        self.skills = SkillModel::default();
        self.clock = SkillCooldownClock::default();
        self.memory = SpellTargetMemory::default();
        self.chase = None;
        self.next_attack_at = 0;
        self.next_approach_at = 0;
        self.last_chase_point = None;
    }
    pub fn ingest(&mut self, s: CombatSnapshot) -> bool {
        if !s.identity.valid() {
            return false;
        }
        if let Some(old) = self.identity_high {
            if s.identity.controller_run < old.controller_run
                || s.identity.controller_run == old.controller_run
                    && ((
                        s.identity.connection_generation,
                        s.identity.session_generation,
                        s.identity.owner_revision,
                    ) < (
                        old.connection_generation,
                        old.session_generation,
                        old.owner_revision,
                    ) || s.revision <= self.revision_high)
            {
                return false;
            }
        }
        if s.identity.controller_run < self.highest_run {
            return false;
        }
        if s.identity.controller_run > self.highest_run {
            self.clear();
            self.namespace = PortableRequestNamespace::new(s.identity.controller_run);
            self.highest_run = s.identity.controller_run;
            self.edge_high = 0;
            self.identity_high = None;
            self.revision_high = 0;
        }
        self.identity_high = Some(s.identity);
        self.revision_high = s.revision;
        if !s.validate() {
            self.clear();
            return false;
        }
        let Ok(mut skills) = normalize_raw_skills(&s.learned) else {
            self.clear();
            return false;
        };
        skills.authority = SkillModelAuthority {
            session_epoch: s.identity.session_generation,
            snapshot_serial: s.model_revision,
            player_object_id: s.identity.player_object_id,
        };
        // Legacy Web display ticks are not a clock. Actual packet starts are the source.
        for b in &mut skills.bindings {
            b.cooldown_remaining_ticks = 0;
            b.cast_sequence = 0;
            if let Some(t) = s.timing.iter().find(|t| Some(&t.spell) == b.spell.as_ref()) {
                b.cast_sequence = t.sequence;
                if let Some(delay) = t.delay_ms {
                    b.delay_ms = Some(delay);
                }
            }
        }
        if self
            .snapshot
            .as_ref()
            .is_some_and(|old| old.identity != s.identity || old.map != s.map)
        {
            self.clock = SkillCooldownClock::default();
            self.memory = SpellTargetMemory::default();
            self.chase = None;
            self.next_attack_at = 0;
            self.last_chase_point = None;
        }
        self.memory.session(s.identity.session_generation);
        self.clock
            .observe_with_starts(&skills, s.now_ms, |_, spell, sequence| {
                s.timing
                    .iter()
                    .find(|t| t.spell == spell && t.sequence == sequence)
                    .map(|t| t.observed_at_ms)
            });
        self.last_now = self.last_now.max(s.now_ms);
        self.skills = skills;
        if !s.enabled {
            self.chase = None;
        }
        self.snapshot = Some(s);
        true
    }
    pub fn key_is_skill(&self, key: &str, m: CombatModifiers) -> bool {
        let bindings = self
            .snapshot
            .as_ref()
            .and_then(|s| s.bindings.clone())
            .unwrap_or_else(default_skill_bindings);
        !matching_skill_slots(&bindings, key, m).is_empty()
    }
    pub fn edge(&mut self, e: CombatEdge, now: u64) -> (bool, Vec<CombatOutput>) {
        let handled = match &e.action {
            CombatAction::Key { key, modifiers, .. } => self.key_is_skill(key, *modifiers),
            _ => true,
        };
        if self.identity_high != Some(e.identity)
            || e.sequence == 0
            || e.sequence > MAX_SAFE_ID
            || e.sequence <= self.edge_high
        {
            return (handled, vec![]);
        }
        self.edge_high = e.sequence;
        // Burn before any eligibility, snapshot or dispatch failure; never retain action for replay.
        let Some(sequence) = self
            .namespace
            .as_mut()
            .and_then(PortableRequestNamespace::allocate)
        else {
            return (handled, vec![]);
        };
        let Some(s) = self.snapshot.clone().filter(|s| {
            s.identity == e.identity
                && s.model_revision == e.model_revision
                && s.map == e.map
                && s.enabled
        }) else {
            return (handled, vec![]);
        };
        if now < self.last_now {
            self.clear();
            return (handled, vec![]);
        }
        self.last_now = now;
        match e.action {
            CombatAction::Cancel => {
                self.chase = None;
                (handled, vec![CombatOutput::Clear])
            }
            CombatAction::Select { object_id } => {
                let next = s
                    .actors
                    .iter()
                    .find(|a| {
                        a.object_id == object_id.to_string()
                            && a.kind == EntityKind::Monster
                            && !a.dead
                            && !matches!(a.ai, 64 | 70)
                    })
                    .map(|_| object_id);
                if next != self.chase {
                    self.next_attack_at = 0;
                    self.last_chase_point = None;
                }
                self.chase = next;
                let out = self.tick(now, e.sequence);
                (handled, out)
            }
            CombatAction::Key {
                key,
                modifiers,
                repeat,
            } => {
                if repeat {
                    return (handled, vec![]);
                }
                let bindings = s.bindings.clone().unwrap_or_else(default_skill_bindings);
                let mut out = vec![];
                for (index, slot) in matching_skill_slots(&bindings, &key, modifiers)
                    .into_iter()
                    .enumerate()
                {
                    let id = if index == 0 {
                        Some(sequence)
                    } else {
                        self.namespace
                            .as_mut()
                            .and_then(PortableRequestNamespace::allocate)
                    };
                    if let Some(id) = id {
                        if let Some(proof) = self.cast(
                            &s,
                            slot,
                            None,
                            s.spell_lock_key != "None"
                                && !s.spell_lock_key.is_empty()
                                && s.spell_lock_key.eq_ignore_ascii_case(&key),
                            now,
                            id,
                            e.sequence,
                        ) {
                            out.push(CombatOutput::Wire { proof });
                        }
                    }
                }
                (handled, out)
            }
            CombatAction::Slot { slot, cursor } => {
                let out = self
                    .cast(
                        &s,
                        slot,
                        cursor,
                        s.pointer_spell_lock,
                        now,
                        sequence,
                        e.sequence,
                    )
                    .map(|proof| CombatOutput::Wire { proof })
                    .into_iter()
                    .collect();
                (handled, out)
            }
            CombatAction::SkillKey { skill_key } => {
                let rows = self
                    .skills
                    .skills
                    .iter()
                    .filter(|r| r.key.as_deref() == Some(skill_key.as_str()))
                    .collect::<Vec<_>>();
                let slot = if rows.len() == 1 {
                    self.skills
                        .binding_for(rows[0].id)
                        .hotkey
                        .and_then(|k| u8::try_from(k).ok())
                        .filter(|k| (1..=16).contains(k))
                } else {
                    None
                };
                let out = slot
                    .and_then(|slot| {
                        self.cast(
                            &s,
                            slot,
                            None,
                            s.pointer_spell_lock,
                            now,
                            sequence,
                            e.sequence,
                        )
                    })
                    .map(|proof| CombatOutput::Wire { proof })
                    .into_iter()
                    .collect();
                (handled, out)
            }
        }
    }
    fn cast(
        &mut self,
        s: &CombatSnapshot,
        slot: u8,
        cursor: Option<(i32, i32)>,
        lock: bool,
        now: u64,
        sequence: u64,
        edge_sequence: u64,
    ) -> Option<CombatProof> {
        if s.player.dead {
            return None;
        }
        let selection = skill_ready(&self.skills, slot, s.hp, s.mp, |id| {
            self.clock.remaining_ms(id, &self.skills, now)
        })?;
        let spell = selection.spell?;
        if !exact_skill(&self.skills, selection.skill_id, &spell) {
            return None;
        }
        let command = if selection.cast_kind.as_deref() == Some("toggle") {
            CombatCommand::SpellToggle {
                spell: spell.clone(),
                toggle_state: next_toggle_state(selection.can_use),
            }
        } else {
            let aim = self.memory.aim(
                &spell,
                &s.player,
                &s.actors,
                s.selected_object_id,
                if cursor.is_some() {
                    None
                } else {
                    s.hovered_object_id.as_deref()
                },
                cursor.or(s.cursor),
            )?;
            CombatCommand::Magic {
                object_id: s.identity.player_object_id,
                spell: spell.clone(),
                direction: aim.direction,
                target_id: aim.target_id,
                x: aim.location.0,
                y: aim.location.1,
                spell_target_lock: lock,
            }
        };
        let target_id = match &command {
            CombatCommand::Magic { target_id, .. } => Some(*target_id),
            _ => None,
        };
        Some(CombatProof {
            identity: s.identity,
            model_revision: s.model_revision,
            map: s.map.clone(),
            sequence,
            edge_sequence,
            movement_block_ms: 0,
            slot: Some(slot),
            skill_id: Some(selection.skill_id),
            spell: Some(spell),
            target_id,
            command,
        })
    }
    pub fn tick(&mut self, now: u64, edge_sequence: u64) -> Vec<CombatOutput> {
        if now < self.last_now || now > MAX_SAFE_ID {
            self.clear();
            return vec![];
        }
        self.last_now = now;
        let Some(s) = self
            .snapshot
            .clone()
            .filter(|s| s.enabled && s.hp > 0 && !s.player.dead)
        else {
            return vec![];
        };
        let Some(id) = self.chase else {
            return vec![];
        };
        if s.selected_object_id != Some(id) {
            self.chase = None;
            return vec![CombatOutput::Clear];
        }
        let Some(target) = s.actors.iter().find(|a| {
            a.object_id == id.to_string()
                && a.kind == EntityKind::Monster
                && !a.dead
                && !matches!(a.ai, 64 | 70)
        }) else {
            self.chase = None;
            return vec![CombatOutput::Clear];
        };
        let archer_weapon = s
            .class
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case("Archer"))
            && s.has_class_weapon == Some(true);
        let ranged = archer_weapon && s.riding_mount == Some(false) && s.fishing == Some(false);
        let distance = (i64::from(target.x) - i64::from(s.player.x))
            .unsigned_abs()
            .max((i64::from(target.y) - i64::from(s.player.y)).unsigned_abs());
        let planned = s.planning_position.is_some_and(|p| {
            (i64::from(target.x) - i64::from(p.0))
                .unsigned_abs()
                .max((i64::from(target.y) - i64::from(p.1)).unsigned_abs())
                <= if ranged { 9 } else { 1 }
        });
        match chase_decision(
            true,
            distance,
            archer_weapon,
            ranged,
            now >= self.next_attack_at && s.motion_remaining_ms == 0,
            s.movement_ready,
            planned,
        ) {
            ChaseDecision::Clear => {
                self.chase = None;
                vec![CombatOutput::Clear]
            }
            ChaseDecision::Wait => vec![],
            ChaseDecision::Attack => {
                let Some(sequence) = self
                    .namespace
                    .as_mut()
                    .and_then(PortableRequestNamespace::allocate)
                else {
                    self.clear();
                    return vec![];
                };
                self.next_attack_at =
                    now.saturating_add(attack_request_interval_ms(s.level, s.attack_speed));
                let context = CrystalWorldClickContext {
                    in_game: true,
                    world_actions_blocked: false,
                    player_hp: Some(s.hp),
                    player_max_hp: Some(s.max_hp),
                    player_x: s.player.x,
                    player_y: s.player.y,
                    target: s
                        .click_targets
                        .iter()
                        .find(|t| {
                            t.object_id == id
                                && t.x == target.x
                                && t.y == target.y
                                && t.kind == target.kind
                        })
                        .copied(),
                    alt: false,
                    shift: false,
                    class: s.class.clone(),
                    has_class_weapon: s.has_class_weapon,
                    riding_mount: s.riding_mount,
                    dazed: s.dazed,
                    fishing: s.fishing,
                    target_in_range: Some(distance <= 9),
                };
                let command = resolve_world_click(&context)
                    .unwrap_or(CombatCommand::Attack { object_id: id });
                vec![CombatOutput::Wire {
                    proof: CombatProof {
                        identity: s.identity,
                        model_revision: s.model_revision,
                        map: s.map.clone(),
                        sequence,
                        edge_sequence,
                        movement_block_ms: attack_request_interval_ms(s.level, s.attack_speed)
                            .min(u64::from(u32::MAX))
                            as u32,
                        slot: None,
                        skill_id: None,
                        spell: None,
                        target_id: Some(id),
                        command,
                    },
                }]
            }
            ChaseDecision::Approach => {
                let point = (target.x, target.y);
                if self.last_chase_point == Some(point) && now < self.next_approach_at {
                    return vec![];
                }
                let candidates = attack_neighbours(point);
                let next = s
                    .neighbours
                    .iter()
                    .filter(|n| candidates.contains(&(n.x, n.y)))
                    .min_by_key(|n| n.cost);
                let Some(next) = next else {
                    self.chase = None;
                    return vec![CombatOutput::Clear];
                };
                self.last_chase_point = Some(point);
                self.next_approach_at = now.saturating_add(160);
                let Some(sequence) = self
                    .namespace
                    .as_mut()
                    .and_then(PortableRequestNamespace::allocate)
                else {
                    self.clear();
                    return vec![];
                };
                vec![CombatOutput::Approach {
                    identity: s.identity,
                    model_revision: s.model_revision,
                    map: s.map.clone(),
                    sequence,
                    edge_sequence,
                    target_id: id,
                    x: next.x,
                    y: next.y,
                }]
            }
        }
    }
}
