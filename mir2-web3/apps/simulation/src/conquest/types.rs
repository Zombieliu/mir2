use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConquestPolicy {
    pub index: i32,
    pub name: String,
    pub map_file_name: String,
    pub palace_file_name: String,
    pub start_minute: u16,
    pub duration_minutes: u16,
    /// Sunday first, matching Crystal DayOfWeek.
    pub enabled_days: [bool; 7],
    /// Authoritative server calendar, independent of a player's display locale.
    pub utc_offset_minutes: i16,
    pub capture_interval_ms: u64,
}
impl ConquestPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.index <= 0
            || self.name.trim().is_empty()
            || self.name.len() > 80
            || self.map_file_name.is_empty()
            || self.palace_file_name.is_empty()
            || self.start_minute >= 1440
            || self.duration_minutes == 0
            || u32::from(self.start_minute) + u32::from(self.duration_minutes) > 1440
            || !self.enabled_days.iter().any(|day| *day)
            || !(-840..=840).contains(&self.utc_offset_minutes)
            || !(1..=60_000).contains(&self.capture_interval_ms)
        {
            return Err("invalid classic conquest policy".into());
        }
        Ok(())
    }
    pub fn window(&self, now_ms: u64) -> Result<ConquestWindow, String> {
        self.validate()?;
        let now = i64::try_from(now_ms).map_err(|_| "conquest time exhausted")?;
        let offset = i64::from(self.utc_offset_minutes) * 60_000;
        let local = now
            .checked_add(offset)
            .ok_or("conquest calendar exhausted")?;
        let day = local.div_euclid(86_400_000);
        let weekday = (day + 4).rem_euclid(7) as usize; // 1970-01-01 Thursday.
        let start = day
            .checked_mul(86_400_000)
            .and_then(|v| v.checked_add(i64::from(self.start_minute) * 60_000))
            .and_then(|v| v.checked_sub(offset))
            .ok_or("conquest window exhausted")?;
        let end = start
            .checked_add(i64::from(self.duration_minutes) * 60_000)
            .ok_or("conquest window exhausted")?;
        Ok(ConquestWindow {
            day,
            start_ms: u64::try_from(start).map_err(|_| "conquest window before epoch")?,
            end_ms: u64::try_from(end).map_err(|_| "conquest window before epoch")?,
            open: self.enabled_days[weekday] && start <= now && now < end,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConquestWindow {
    pub day: i64,
    pub start_ms: u64,
    pub end_ms: u64,
    pub open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConquestLease {
    pub owner: String,
    pub generation: u64,
    pub expires_ms: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConquestEvent {
    pub sequence: u64,
    pub at_ms: u64,
    pub kind: ConquestEventKind,
    pub guild_id: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConquestEventKind {
    Requested,
    WarStarted,
    Captured,
    WarEnded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConquestDefenseState {
    pub slot: u8,
    pub kind: ConquestDefenseKind,
    pub template_name: String,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub open: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConquestDefenseKind {
    Gate,
    Wall,
    Archer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedConquestRecord {
    pub index: i32,
    pub revision: u64,
    pub owner_guild_id: Option<String>,
    /// One eligible attacker in Request mode; after a capture, the previous
    /// incumbent becomes attacker and may recapture until the war ends.
    pub attacker_guild_id: Option<String>,
    pub war_active: bool,
    pub battle_day: Option<i64>,
    pub war_ends_ms: u64,
    pub last_observed_ms: u64,
    pub last_capture_ms: u64,
    pub clock_generation: u64,
    pub lease: Option<ConquestLease>,
    pub next_event_sequence: u64,
    pub events: Vec<ConquestEvent>,
    pub defenses: BTreeMap<String, ConquestDefenseState>,
    pub tax_rate_percent: u8,
    pub gold: u32,
}
pub fn valid_guild_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}
impl SharedConquestRecord {
    pub fn new(index: i32) -> Self {
        Self {
            index,
            revision: 1,
            owner_guild_id: None,
            attacker_guild_id: None,
            war_active: false,
            battle_day: None,
            war_ends_ms: 0,
            last_observed_ms: 0,
            last_capture_ms: 0,
            clock_generation: 0,
            lease: None,
            next_event_sequence: 1,
            events: Vec::new(),
            defenses: BTreeMap::new(),
            tax_rate_percent: 0,
            gold: 0,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.index <= 0
            || self.revision == 0
            || self.next_event_sequence == 0
            || self.clock_generation > i64::MAX as u64
            || self.events.len() > 100
            || self.tax_rate_percent > 100
            || self.last_observed_ms > i64::MAX as u64
            || self.war_ends_ms > i64::MAX as u64
            || self
                .owner_guild_id
                .iter()
                .chain(self.attacker_guild_id.iter())
                .any(|id| !valid_guild_id(id))
            || (self.war_active
                && (self.battle_day.is_none()
                    || self.war_ends_ms == 0
                    || self.attacker_guild_id.is_none()))
            || self
                .events
                .windows(2)
                .any(|pair| pair[0].sequence >= pair[1].sequence)
            || self.events.iter().any(|event| {
                event.sequence >= self.next_event_sequence
                    || event
                        .guild_id
                        .as_deref()
                        .is_some_and(|id| !valid_guild_id(id))
            })
        {
            return Err("invalid shared conquest record".into());
        }
        if let Some(lease) = &self.lease {
            if !valid_guild_id(&lease.owner)
                || lease.generation == 0
                || lease.generation != self.clock_generation
                || lease.expires_ms <= self.last_observed_ms
                || lease.expires_ms > i64::MAX as u64
            {
                return Err("invalid conquest clock lease".into());
            }
        }
        for (key, defense) in &self.defenses {
            if defense.slot == 0
                || defense.max_hp <= 0
                || defense.hp < 0
                || defense.hp > defense.max_hp
                || defense.template_name.is_empty()
                || defense.x < 0
                || defense.y < 0
                || (defense.open && defense.kind != ConquestDefenseKind::Gate)
                || key
                    != &format!(
                        "{}:{}",
                        match defense.kind {
                            ConquestDefenseKind::Gate => "gate",
                            ConquestDefenseKind::Wall => "wall",
                            ConquestDefenseKind::Archer => "archer",
                        },
                        defense.slot
                    )
            {
                return Err("invalid conquest defense state".into());
            }
        }
        Ok(())
    }
    pub(crate) fn changed(&mut self) -> Result<(), String> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("conquest revision exhausted")?;
        Ok(())
    }
    pub(crate) fn event(
        &mut self,
        at_ms: u64,
        kind: ConquestEventKind,
        guild_id: Option<String>,
    ) -> Result<(), String> {
        let sequence = self.next_event_sequence;
        self.next_event_sequence = sequence
            .checked_add(1)
            .ok_or("conquest event sequence exhausted")?;
        self.events.push(ConquestEvent {
            sequence,
            at_ms,
            kind,
            guild_id,
        });
        if self.events.len() > 100 {
            self.events.remove(0);
        }
        Ok(())
    }
}

/// Internal authoritative Zone sample, never deserialized from a client packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConquestPalacePresence {
    pub account_id: String,
    pub character_index: i32,
    pub map_file_name: String,
    pub alive: bool,
    pub guild_id: Option<String>,
}
