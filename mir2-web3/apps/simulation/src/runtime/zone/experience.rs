//! Trusted reward selection captured at death, before delayed account delivery.
//! These values are never fields of a client gameplay packet. Old checkpoints
//! have no selection and must not infer a guild from a later login.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneGuildExperienceMembership {
    pub guild_id: String,
    pub membership_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneExperienceProfile {
    pub account_id: String,
    pub character_index: i32,
    pub guild: Option<ZoneGuildExperienceMembership>,
    pub monster_multiplier: u32,
    pub natural_kill_multiplier: u32,
    pub lover_eligible: bool,
    pub lover_rate_percent: i32,
    pub mentee_eligible: bool,
    pub mentee_rate_percent: i32,
    pub experience_rate_percent: i32,
    /// Exact source f32 bits preserve the original per-event truncation rule.
    pub guild_experience_rate_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneExperienceSelection {
    pub source_zone: String,
    pub monster_object_id: u32,
    pub killed_at_ms: u64,
    pub account_id: String,
    pub character_index: i32,
    pub guild: Option<ZoneGuildExperienceMembership>,
    pub final_amount: u32,
    pub guild_amount: u32,
}

impl ZoneExperienceProfile {
    pub(super) fn select(
        &self,
        source_zone: String,
        monster_object_id: u32,
        killed_at_ms: u64,
        raw_share: u32,
    ) -> Result<ZoneExperienceSelection, String> {
        if self.account_id.is_empty()
            || self
                .guild
                .as_ref()
                .is_some_and(|guild| guild.guild_id.is_empty())
        {
            return Err("reward profile has no stable identity".into());
        }
        let amount = raw_share
            .saturating_mul(self.monster_multiplier)
            .saturating_mul(self.natural_kill_multiplier);
        let final_amount = crate::apply_crystal_experience_rates(
            amount,
            if self.lover_eligible {
                self.lover_rate_percent
            } else {
                0
            },
            if self.mentee_eligible {
                self.mentee_rate_percent
            } else {
                0
            },
            self.experience_rate_percent,
        );
        let rate = f32::from_bits(self.guild_experience_rate_bits);
        if !rate.is_finite() || rate < 0.0 {
            return Err("invalid captured guild experience rate".into());
        }
        let guild_amount = if self.guild.is_some() {
            let amount = final_amount as f32 * rate;
            if !amount.is_finite() || amount >= 4_294_967_296.0 {
                return Err("captured guild experience amount overflow".into());
            }
            amount as u32
        } else {
            0
        };
        Ok(ZoneExperienceSelection {
            source_zone,
            monster_object_id,
            killed_at_ms,
            account_id: self.account_id.clone(),
            character_index: self.character_index,
            guild: self.guild.clone(),
            final_amount,
            guild_amount,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guild_xp_death_selection_freezes_original_membership_and_final_rates() {
        let mut profile = ZoneExperienceProfile {
            account_id: "owner".into(),
            character_index: 7,
            guild: Some(ZoneGuildExperienceMembership {
                guild_id: "guild-a".into(),
                membership_epoch: 3,
            }),
            monster_multiplier: 2,
            natural_kill_multiplier: 1,
            lover_eligible: true,
            lover_rate_percent: 5,
            mentee_eligible: false,
            mentee_rate_percent: 10,
            experience_rate_percent: 20,
            guild_experience_rate_bits: 0.01f32.to_bits(),
        };
        let selection = profile.select("zone".into(), 23, 500, 100).unwrap();
        assert_eq!(selection.final_amount, 252);
        assert_eq!(selection.guild_amount, 2);
        profile.guild.as_mut().unwrap().guild_id = "guild-b".into();
        profile.experience_rate_percent = 0;
        let replay: ZoneExperienceSelection =
            serde_json::from_slice(&serde_json::to_vec(&selection).unwrap()).unwrap();
        assert_eq!(replay, selection);
        assert_eq!(replay.guild.as_ref().unwrap().guild_id, "guild-a");
        assert_ne!(replay, profile.select("zone".into(), 23, 500, 100).unwrap());
    }
}
