use mir2_protocol::{MirClass, MirDirection, MirGender, Point};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    ZoneChatProfile, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterDefense, ZoneMonsterSpawn,
    ZonePlayerCombatStats, ZoneRuntime,
};

const REPLAY_CHECKPOINT_VERSION: u32 = 1;
const REPLAY_COMMITMENT_DOMAIN: &[u8] = b"obelisk.mir2.zone-replay.v1\0";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneReplayCombatStats {
    pub min_dc: i32,
    pub max_dc: i32,
    pub min_mc: i32,
    pub max_mc: i32,
    pub min_sc: i32,
    pub max_sc: i32,
    pub accuracy: i32,
    pub agility: i32,
    pub min_ac: i32,
    pub max_ac: i32,
    pub min_mac: i32,
    pub max_mac: i32,
    pub critical_rate: i32,
    pub critical_damage: i32,
    pub luck: i32,
}

impl From<ZoneReplayCombatStats> for ZonePlayerCombatStats {
    fn from(value: ZoneReplayCombatStats) -> Self {
        Self {
            min_dc: value.min_dc,
            max_dc: value.max_dc,
            min_mc: value.min_mc,
            max_mc: value.max_mc,
            min_sc: value.min_sc,
            max_sc: value.max_sc,
            accuracy: value.accuracy,
            agility: value.agility,
            min_ac: value.min_ac,
            max_ac: value.max_ac,
            min_mac: value.min_mac,
            max_mac: value.max_mac,
            critical_rate: value.critical_rate,
            critical_damage: value.critical_damage,
            luck: value.luck,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ZoneReplayCommand {
    Join {
        session_id: String,
        account_id: String,
        character_index: i32,
        object_id: u32,
        name: String,
        class: MirClass,
        gender: MirGender,
        level: u16,
        hp: i32,
        max_hp: i32,
        mp: i32,
        map_file_name: String,
        position: Point,
        direction: MirDirection,
        combat_stats: ZoneReplayCombatStats,
    },
    Leave {
        session_id: String,
    },
    Walk {
        session_id: String,
        direction: MirDirection,
        movement_sequence: u64,
    },
    Run {
        session_id: String,
        direction: MirDirection,
        movement_sequence: u64,
    },
    Turn {
        session_id: String,
        direction: MirDirection,
    },
    SpawnMonster {
        session_id: String,
        object_id: u32,
        name: String,
        name_colour_argb: i32,
        image: u16,
        ai: u8,
        level: u16,
        max_hp: i32,
        hp: i32,
        experience: u32,
        position: Point,
        direction: MirDirection,
        agility: i32,
        min_ac: i32,
        max_ac: i32,
        min_mac: i32,
        max_mac: i32,
    },
    PlayerAttackObject {
        session_id: String,
        object_id: u32,
        direction: MirDirection,
        spell: u8,
        level: u8,
        attack_type: u8,
        damage: i32,
    },
    OpenDoor {
        session_id: String,
        door_index: u8,
    },
    ConfigureHazards {
        session_id: String,
        lightning: bool,
        fire: bool,
        lightning_damage: i32,
        fire_damage: i32,
    },
    Tick,
}

impl ZoneReplayCommand {
    fn into_zone_command(self, logical_time_ms: u64) -> ZoneCommand {
        match self {
            Self::Join {
                session_id,
                account_id,
                character_index,
                object_id,
                name,
                class,
                gender,
                level,
                hp,
                max_hp,
                mp,
                map_file_name,
                position,
                direction,
                combat_stats,
            } => ZoneCommand::Join(ZoneJoin {
                session_id: session_id.into(),
                account_id,
                character_index,
                object_id,
                name,
                class,
                gender,
                level,
                hp,
                max_hp,
                mp,
                map_file_name,
                position,
                direction,
                chat_profile: ZoneChatProfile::default(),
                combat_stats: combat_stats.into(),
            }),
            Self::Leave { session_id } => ZoneCommand::Leave {
                session_id: session_id.into(),
            },
            Self::Walk {
                session_id,
                direction,
                movement_sequence,
            } => ZoneCommand::Walk {
                session_id: session_id.into(),
                direction,
                seq: movement_sequence,
                now_ms: logical_time_ms,
            },
            Self::Run {
                session_id,
                direction,
                movement_sequence,
            } => ZoneCommand::Run {
                session_id: session_id.into(),
                direction,
                seq: movement_sequence,
                now_ms: logical_time_ms,
            },
            Self::Turn {
                session_id,
                direction,
            } => ZoneCommand::Turn {
                session_id: session_id.into(),
                direction,
                now_ms: logical_time_ms,
            },
            Self::SpawnMonster {
                session_id,
                object_id,
                name,
                name_colour_argb,
                image,
                ai,
                level,
                max_hp,
                hp,
                experience,
                position,
                direction,
                agility,
                min_ac,
                max_ac,
                min_mac,
                max_mac,
            } => ZoneCommand::SpawnMonster {
                session_id: session_id.into(),
                monster: ZoneMonsterSpawn {
                    object_id,
                    name,
                    name_colour_argb,
                    image,
                    ai,
                    level,
                    max_hp,
                    hp,
                    experience,
                    friendly_guild: None,
                    position,
                    direction,
                    defense: ZoneMonsterDefense {
                        agility,
                        min_ac,
                        max_ac,
                        min_mac,
                        max_mac,
                    },
                    drops: Vec::new(),
                },
                now_ms: logical_time_ms,
            },
            Self::PlayerAttackObject {
                session_id,
                object_id,
                direction,
                spell,
                level,
                attack_type,
                damage,
            } => ZoneCommand::PlayerAttackObject {
                session_id: session_id.into(),
                object_id,
                direction,
                spell,
                level,
                attack_type,
                damage,
                now_ms: logical_time_ms,
            },
            Self::OpenDoor {
                session_id,
                door_index,
            } => ZoneCommand::OpenDoor {
                session_id: session_id.into(),
                door_index,
                now_ms: logical_time_ms,
            },
            Self::ConfigureHazards {
                session_id,
                lightning,
                fire,
                lightning_damage,
                fire_damage,
            } => ZoneCommand::ConfigureHazards {
                session_id: session_id.into(),
                lightning,
                fire,
                lightning_damage,
                fire_damage,
            },
            Self::Tick => ZoneCommand::Tick {
                now_ms: logical_time_ms,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneInput {
    pub zone_id: String,
    pub epoch: u64,
    pub sequence: u64,
    pub logical_time_ms: u64,
    pub command: ZoneReplayCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneReplayScenario {
    pub zone_key: ZoneKey,
    pub epoch: u64,
    pub inputs: Vec<ZoneInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneOutput {
    pub sequence: u64,
    pub outbound_count: usize,
    pub state_root: String,
    /// Rolling commitment to every accepted input and resulting state root.
    pub checkpoint_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneReplayReport {
    pub zone_id: String,
    pub epoch: u64,
    pub applied_inputs: usize,
    pub final_sequence: Option<u64>,
    pub tick_count: usize,
    pub outbound_count: usize,
    pub state_root: String,
    pub checkpoint_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ZoneReplayCheckpoint {
    version: u32,
    zone_key: ZoneKey,
    zone_id: String,
    epoch: u64,
    next_sequence: u64,
    last_logical_time_ms: u64,
    inputs: Vec<ZoneInput>,
    state_root: String,
    checkpoint_hash: String,
    outbound_count: usize,
    tick_count: usize,
}

pub struct ZoneReplayEngine {
    runtime: ZoneRuntime,
    zone_id: String,
    epoch: u64,
    next_sequence: u64,
    last_logical_time_ms: u64,
    inputs: Vec<ZoneInput>,
    state_root: String,
    checkpoint_hash: String,
    outbound_count: usize,
    tick_count: usize,
}

impl ZoneReplayEngine {
    pub fn new(zone_key: ZoneKey, epoch: u64) -> Result<Self, String> {
        let zone_id = zone_id_for_key(&zone_key);
        let runtime = ZoneRuntime::new(zone_key);
        let state_root = runtime.canonical_state_root()?;
        let checkpoint_hash = genesis_commitment(&zone_id, epoch, &state_root);
        Ok(Self {
            runtime,
            zone_id,
            epoch,
            next_sequence: 0,
            last_logical_time_ms: 0,
            inputs: Vec::new(),
            state_root,
            checkpoint_hash,
            outbound_count: 0,
            tick_count: 0,
        })
    }

    pub fn apply(&mut self, input: ZoneInput) -> Result<ZoneOutput, String> {
        self.validate_input(&input)?;
        let is_tick = matches!(input.command, ZoneReplayCommand::Tick);
        let zone_command = input
            .command
            .clone()
            .into_zone_command(input.logical_time_ms);
        let outbounds = self.runtime.handle(zone_command);
        let state_root = self.runtime.canonical_state_root()?;
        let checkpoint_hash = next_commitment(&self.checkpoint_hash, &input, &state_root)?;
        let output = ZoneOutput {
            sequence: input.sequence,
            outbound_count: outbounds.len(),
            state_root: state_root.clone(),
            checkpoint_hash: checkpoint_hash.clone(),
        };

        self.next_sequence = self.next_sequence.saturating_add(1);
        self.last_logical_time_ms = input.logical_time_ms;
        self.outbound_count = self.outbound_count.saturating_add(outbounds.len());
        self.tick_count = self.tick_count.saturating_add(usize::from(is_tick));
        self.inputs.push(input);
        self.state_root = state_root;
        self.checkpoint_hash = checkpoint_hash;
        Ok(output)
    }

    pub fn apply_all<I>(&mut self, inputs: I) -> Result<ZoneReplayReport, String>
    where
        I: IntoIterator<Item = ZoneInput>,
    {
        for input in inputs {
            self.apply(input)?;
        }
        Ok(self.report())
    }

    pub fn report(&self) -> ZoneReplayReport {
        ZoneReplayReport {
            zone_id: self.zone_id.clone(),
            epoch: self.epoch,
            applied_inputs: self.inputs.len(),
            final_sequence: self.next_sequence.checked_sub(1),
            tick_count: self.tick_count,
            outbound_count: self.outbound_count,
            state_root: self.state_root.clone(),
            checkpoint_hash: self.checkpoint_hash.clone(),
        }
    }

    /// Gate 5.1 intentionally uses an event-sourced checkpoint: the accepted
    /// canonical input log is persisted and replayed on restore. This proves
    /// deterministic restart semantics before a compact production snapshot is
    /// introduced in the failover gate.
    pub fn checkpoint_bytes(&self) -> Result<Vec<u8>, String> {
        let checkpoint = ZoneReplayCheckpoint {
            version: REPLAY_CHECKPOINT_VERSION,
            zone_key: self.runtime.key().clone(),
            zone_id: self.zone_id.clone(),
            epoch: self.epoch,
            next_sequence: self.next_sequence,
            last_logical_time_ms: self.last_logical_time_ms,
            inputs: self.inputs.clone(),
            state_root: self.state_root.clone(),
            checkpoint_hash: self.checkpoint_hash.clone(),
            outbound_count: self.outbound_count,
            tick_count: self.tick_count,
        };
        serde_json::to_vec(&checkpoint)
            .map_err(|error| format!("failed to serialize zone replay checkpoint: {error}"))
    }

    pub fn restore(checkpoint_bytes: &[u8]) -> Result<Self, String> {
        let checkpoint: ZoneReplayCheckpoint = serde_json::from_slice(checkpoint_bytes)
            .map_err(|error| format!("failed to decode zone replay checkpoint: {error}"))?;
        if checkpoint.version != REPLAY_CHECKPOINT_VERSION {
            return Err(format!(
                "unsupported zone replay checkpoint version {}, expected {}",
                checkpoint.version, REPLAY_CHECKPOINT_VERSION
            ));
        }
        if checkpoint.zone_id != zone_id_for_key(&checkpoint.zone_key) {
            return Err("checkpoint zone id does not match its zone key".to_string());
        }

        let mut engine = Self::new(checkpoint.zone_key, checkpoint.epoch)?;
        engine.apply_all(checkpoint.inputs)?;
        if engine.next_sequence != checkpoint.next_sequence
            || engine.last_logical_time_ms != checkpoint.last_logical_time_ms
            || engine.state_root != checkpoint.state_root
            || engine.checkpoint_hash != checkpoint.checkpoint_hash
            || engine.outbound_count != checkpoint.outbound_count
            || engine.tick_count != checkpoint.tick_count
        {
            return Err("checkpoint replay commitment mismatch".to_string());
        }
        Ok(engine)
    }

    /// Start a new fenced ownership epoch from the exact restored state. Input
    /// sequence and rolling commitment restart at zero for the new owner while
    /// the canonical state root remains unchanged.
    pub fn rebase_epoch(mut self, new_epoch: u64) -> Result<Self, String> {
        if new_epoch <= self.epoch {
            return Err(format!(
                "new zone replay epoch {new_epoch} must exceed current epoch {}",
                self.epoch
            ));
        }
        self.epoch = new_epoch;
        self.next_sequence = 0;
        self.last_logical_time_ms = 0;
        self.inputs.clear();
        self.outbound_count = 0;
        self.tick_count = 0;
        self.checkpoint_hash = genesis_commitment(&self.zone_id, new_epoch, &self.state_root);
        Ok(self)
    }

    fn validate_input(&self, input: &ZoneInput) -> Result<(), String> {
        if input.zone_id != self.zone_id {
            return Err(format!(
                "zone id mismatch: expected {}, got {}",
                self.zone_id, input.zone_id
            ));
        }
        if input.epoch != self.epoch {
            return Err(format!(
                "epoch mismatch: expected {}, got {}",
                self.epoch, input.epoch
            ));
        }
        if input.sequence != self.next_sequence {
            return Err(format!(
                "input sequence mismatch: expected {}, got {}",
                self.next_sequence, input.sequence
            ));
        }
        if input.logical_time_ms < self.last_logical_time_ms {
            return Err(format!(
                "logical time regressed: previous {}, got {}",
                self.last_logical_time_ms, input.logical_time_ms
            ));
        }
        if let ZoneReplayCommand::Join { map_file_name, .. } = &input.command {
            if map_file_name != &self.runtime.key().map_file_name {
                return Err(format!(
                    "join map mismatch: zone map {}, got {}",
                    self.runtime.key().map_file_name,
                    map_file_name
                ));
            }
        }
        Ok(())
    }
}

pub fn run_zone_replay_scenario(
    scenario: ZoneReplayScenario,
) -> Result<(ZoneReplayEngine, ZoneReplayReport), String> {
    let mut engine = ZoneReplayEngine::new(scenario.zone_key, scenario.epoch)?;
    let report = engine.apply_all(scenario.inputs)?;
    Ok((engine, report))
}

pub fn zone_id_for_key(key: &ZoneKey) -> String {
    format!(
        "{}/{}/{}/{}",
        key.shard_id, key.map_file_name, key.channel_id, key.instance_id
    )
}

/// Deterministic, non-trivial scenario used by the Gate 5.1 acceptance binary
/// and integration tests. It runs one authoritative player, one hostile native
/// monster, movement inputs, attacks, and exactly `tick_count` logical ticks.
pub fn gate5_demo_scenario(tick_count: usize) -> ZoneReplayScenario {
    let zone_key = ZoneKey::new("poc", "0", 0, "gate5");
    let zone_id = zone_id_for_key(&zone_key);
    let epoch = 1;
    let mut sequence = 0_u64;
    let mut inputs = Vec::with_capacity(tick_count.saturating_add(tick_count / 8 + 4));
    let mut push = |logical_time_ms: u64, command: ZoneReplayCommand| {
        inputs.push(ZoneInput {
            zone_id: zone_id.clone(),
            epoch,
            sequence,
            logical_time_ms,
            command,
        });
        sequence = sequence.saturating_add(1);
    };

    push(
        0,
        ZoneReplayCommand::Join {
            session_id: "gate5-player".to_string(),
            account_id: "gate5-account".to_string(),
            character_index: 0,
            object_id: 1,
            name: "Gate5".to_string(),
            class: MirClass::Warrior,
            gender: MirGender::Male,
            level: 40,
            hp: 1_000_000,
            max_hp: 1_000_000,
            mp: 10_000,
            map_file_name: "0".to_string(),
            position: Point { x: 10, y: 10 },
            direction: MirDirection::Right,
            combat_stats: ZoneReplayCombatStats {
                min_dc: 25,
                max_dc: 45,
                accuracy: 20,
                min_ac: 5,
                max_ac: 10,
                ..Default::default()
            },
        },
    );
    push(
        0,
        ZoneReplayCommand::SpawnMonster {
            session_id: "gate5-player".to_string(),
            object_id: 9_001,
            name: "Gate5 Wooma".to_string(),
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            level: 20,
            max_hp: 1_000_000,
            hp: 1_000_000,
            experience: 100,
            position: Point { x: 16, y: 10 },
            direction: MirDirection::Left,
            agility: 4,
            min_ac: 2,
            max_ac: 5,
            min_mac: 1,
            max_mac: 3,
        },
    );

    let directions = [
        MirDirection::Right,
        MirDirection::Down,
        MirDirection::Left,
        MirDirection::Up,
    ];
    let mut movement_sequence = 1_u64;
    for tick in 1..=tick_count {
        let logical_time_ms = (tick as u64).saturating_mul(600);
        if tick % 16 == 0 {
            push(
                logical_time_ms,
                ZoneReplayCommand::Walk {
                    session_id: "gate5-player".to_string(),
                    direction: directions[(tick / 16) % directions.len()],
                    movement_sequence,
                },
            );
            movement_sequence = movement_sequence.saturating_add(1);
        }
        if tick % 24 == 0 {
            push(
                logical_time_ms,
                ZoneReplayCommand::PlayerAttackObject {
                    session_id: "gate5-player".to_string(),
                    object_id: 9_001,
                    direction: MirDirection::Right,
                    spell: 0,
                    level: 0,
                    attack_type: 0,
                    damage: 30,
                },
            );
        }
        push(logical_time_ms, ZoneReplayCommand::Tick);
    }

    ZoneReplayScenario {
        zone_key,
        epoch,
        inputs,
    }
}

fn genesis_commitment(zone_id: &str, epoch: u64, state_root: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(REPLAY_COMMITMENT_DOMAIN);
    hasher.update(zone_id.as_bytes());
    hasher.update(epoch.to_le_bytes());
    hasher.update(state_root.as_bytes());
    hex_lower(&hasher.finalize())
}

fn next_commitment(previous: &str, input: &ZoneInput, state_root: &str) -> Result<String, String> {
    let input_bytes = serde_json::to_vec(input)
        .map_err(|error| format!("failed to serialize canonical zone input: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(REPLAY_COMMITMENT_DOMAIN);
    hasher.update(previous.as_bytes());
    hasher.update((input_bytes.len() as u64).to_le_bytes());
    hasher.update(input_bytes);
    hasher.update(state_root.as_bytes());
    Ok(hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
