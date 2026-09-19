//! Crystal GuildDialog right-hand Buff page model. Server packets own all buffs.
use mir2_protocol::{GuildBuff, GuildBuffInfo};

pub const VISIBLE_ROWS: usize = 8;
pub const PAGE_ORIGIN: (i32, i32) = (360, 61);
pub const ROW_ORIGIN: (i32, i32) = (4, 27);
pub const ROW_SIZE: (i32, i32) = (188, 33);
pub const ROW_STRIDE: i32 = 38;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuildBuffRequest {
    pub action: u8,
    pub id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuildBuffAuthority {
    pub level: u8,
    pub spare_points: i32,
    pub gold: i64,
    pub may_activate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildBuffError {
    InsufficientPointsAvailable,
    GuildLevelTooLow,
    GuildRankNoBuffActivation,
    BuffIsActive,
    GuildFundsInsufficient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildBuffRowStatus {
    InsufficientLevel,
    Available,
    CountingDown,
    Expired,
    Obtained,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildBuffRow {
    pub id: i32,
    pub name: String,
    /// Index in the original GuildSkill library, not an item icon.
    pub icon: i32,
    pub status: GuildBuffRowStatus,
    pub active: Option<bool>,
    pub warning_red: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildBuffDialog {
    pub catalog: Vec<GuildBuffInfo>,
    pub enabled: Vec<GuildBuff>,
    pub start_index: usize,
    requested_list: bool,
    last_request_ms: Option<u64>,
}

impl GuildBuffDialog {
    /// Reset at character/guild boundary; another guild's unlocks must not leak.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn request_list(&mut self) -> Option<GuildBuffRequest> {
        if self.requested_list {
            return None;
        }
        self.requested_list = true;
        Some(GuildBuffRequest { action: 0, id: 0 })
    }

    pub fn send_failed(&mut self, request: GuildBuffRequest) {
        if request.action == 0 {
            self.requested_list = false;
        }
    }

    /// Crystal GameScene.GuildBuffList applies nonempty catalogs and per-ID
    /// active deltas; an empty list is not a command to clear acquired buffs.
    pub fn apply_packet(&mut self, remove: u8, active: &[GuildBuff], catalog: &[GuildBuffInfo]) {
        if !catalog.is_empty() {
            self.catalog = catalog.to_vec();
        }
        for update in active {
            if let Some(index) = self.enabled.iter().position(|buff| buff.id == update.id) {
                if remove == 1 {
                    self.enabled.remove(index);
                } else {
                    self.enabled[index] = update.clone();
                }
            } else if remove != 1 {
                self.enabled.push(update.clone());
            }
        }
        self.start_index = self.start_index.min(self.max_start());
    }

    pub fn max_start(&self) -> usize {
        self.catalog.len().saturating_sub(VISIBLE_ROWS)
    }

    /// Crystal moves one row per wheel message, irrespective of wheel magnitude.
    pub fn scroll(&mut self, rows: i32) {
        self.start_index =
            (self.start_index as i64 + i64::from(rows)).clamp(0, self.max_start() as i64) as usize;
    }

    pub fn wheel(&mut self, delta: i32) {
        if delta != 0 {
            self.scroll(if delta > 0 { -1 } else { 1 });
        }
    }

    pub fn thumb_y(&self, thumb_height: i32) -> i32 {
        if self.max_start() == 0 {
            return 39;
        }
        let travel = (277 - thumb_height).max(0);
        39 + (travel as f32 / self.max_start() as f32 * self.start_index as f32) as i32
    }

    pub fn drag_thumb(&mut self, y: i32, thumb_height: i32) {
        let travel = (277 - thumb_height).max(0);
        if self.max_start() == 0 || travel == 0 {
            self.start_index = 0;
            return;
        }
        let value = (y.clamp(39, 296) - 39) as f32 / (travel as f32 / self.max_start() as f32);
        // C# Math.Round uses midpoint-to-even; clamp malformed/short catalogs.
        self.start_index = (value.round_ties_even() as usize).min(self.max_start());
    }

    pub fn rows(&self, level: u8) -> Vec<GuildBuffRow> {
        self.catalog
            .iter()
            .skip(self.start_index)
            .take(VISIBLE_ROWS)
            .map(|info| {
                let owned = self.enabled.iter().find(|buff| buff.id == info.id);
                let (status, active, offset, red) = match owned {
                    None => (
                        if info.level_requirement > level {
                            GuildBuffRowStatus::InsufficientLevel
                        } else {
                            GuildBuffRowStatus::Available
                        },
                        None,
                        2,
                        info.level_requirement > level,
                    ),
                    Some(buff) => (
                        if info.time_limit <= 0 {
                            GuildBuffRowStatus::Obtained
                        } else if buff.active {
                            GuildBuffRowStatus::CountingDown
                        } else {
                            GuildBuffRowStatus::Expired
                        },
                        Some(buff.active),
                        i32::from(buff.active),
                        false,
                    ),
                };
                GuildBuffRow {
                    id: info.id,
                    name: info.name.clone(),
                    icon: info.icon.saturating_add(offset),
                    status,
                    active,
                    warning_red: red,
                }
            })
            .collect()
    }

    pub fn request_row(
        &mut self,
        row: usize,
        authority: GuildBuffAuthority,
        now_ms: u64,
    ) -> Result<Option<GuildBuffRequest>, GuildBuffError> {
        if row >= VISIBLE_ROWS {
            return Ok(None);
        }
        let Some(info) = self.catalog.get(self.start_index.saturating_add(row)) else {
            return Ok(None);
        };
        let owned = self.enabled.iter().find(|buff| buff.id == info.id);
        // Original sequential assignments intentionally give rank denial priority,
        // then level/funds, then points/active. Preserve its single modal reason.
        let error = if !authority.may_activate {
            Some(GuildBuffError::GuildRankNoBuffActivation)
        } else if let Some(buff) = owned {
            if authority.gold < i64::from(info.activation_cost) {
                Some(GuildBuffError::GuildFundsInsufficient)
            } else if buff.active {
                Some(GuildBuffError::BuffIsActive)
            } else {
                None
            }
        } else if authority.level < info.level_requirement {
            Some(GuildBuffError::GuildLevelTooLow)
        } else if authority.spare_points < i32::from(info.points_requirement) {
            Some(GuildBuffError::InsufficientPointsAvailable)
        } else {
            None
        };
        if let Some(error) = error {
            return Err(error);
        }
        if self
            .last_request_ms
            .is_some_and(|last| now_ms < last.saturating_add(100))
        {
            return Ok(None);
        }
        self.last_request_ms = Some(now_ms);
        Ok(Some(GuildBuffRequest {
            action: if owned.is_some() { 2 } else { 1 },
            id: info.id,
        }))
    }
}

#[cfg(test)]
#[path = "guild_buff_dialog_tests.rs"]
mod tests;
