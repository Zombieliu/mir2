//! Trusted reward selection captured at death, before delayed account delivery.
//! These values are never fields of a client gameplay packet. Old checkpoints
//! have no selection and must not infer a guild from a later login.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneGuildExperienceMembership {
    pub guild_id: String,
    pub membership_epoch: u64,
}

/// A durable relationship resolved by the server before the Zone read. Display
/// names cannot identify a spouse/teacher, and a later relationship cannot
/// replace the one whose buff is being evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneExperiencePartner {
    pub identity: crate::Stage5FriendIdentity,
    pub relationship_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneExperienceRateStat {
    pub stat: u8,
    pub value: i32,
}

/// Personal buff inputs keep their source order and expiry. They are supplied
/// by the recipient's private runtime, never inferred from a peer's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneExperienceRateBuff {
    pub expires_at_ms: Option<u64>,
    pub lover_buff: bool,
    pub mentee_buff: bool,
    #[serde(default, skip_serializing_if = "experience_is_allowed")]
    pub newbie_buff: bool,
    #[serde(default, skip_serializing_if = "experience_is_allowed")]
    pub pause_in_safe_zone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_remaining_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_buff_key: Option<String>,
    #[serde(default, skip_serializing_if = "source_buff_instance_is_absent")]
    pub source_buff_instance: u64,
    pub stats: Vec<ZoneExperienceRateStat>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneExperienceRateSource {
    pub captured_at_ms: u64,
    pub base_experience_rate_percent: i32,
    pub base_lover_rate_percent: i32,
    pub base_mentee_rate_percent: i32,
    pub buffs: Vec<ZoneExperienceRateBuff>,
    /// Only the source AccountStore admission can replace Guild contributions.
    pub guild_stats: Vec<ZoneExperienceRateStat>,
}

impl ZoneExperienceRateSource {
    pub(crate) fn rates_at(&self, now_ms: u64) -> Result<([i32; 3], bool, bool), String> {
        let mut rates = [
            self.base_experience_rate_percent,
            self.base_lover_rate_percent,
            self.base_mentee_rate_percent,
        ];
        let mut lover = false;
        let mut mentee = false;
        let mut add = |entry: &ZoneExperienceRateStat| -> Result<(), String> {
            let index = match entry.stat {
                100 => 0,
                120 => 1,
                123 => 2,
                _ => return Err("invalid trusted experience stat contribution".into()),
            };
            rates[index] = rates[index].saturating_add(entry.value);
            Ok(())
        };
        for buff in &self.buffs {
            if buff.pause_in_safe_zone {
                if !matches!(buff.source_buff_key.as_deref(), Some("exp" | "drop"))
                    || buff.source_buff_instance == 0
                    || buff.expires_at_ms.is_some() == buff.paused_remaining_ms.is_some()
                {
                    return Err("invalid trusted safe-zone experience buff clock".into());
                }
            } else if buff.paused_remaining_ms.is_some()
                || buff.source_buff_key.is_some()
                || buff.source_buff_instance != 0
            {
                return Err("unexpected trusted experience buff clock identity".into());
            }
            // Reject malformed metadata even when its claimed expiry is past.
            if buff
                .stats
                .iter()
                .any(|entry| !matches!(entry.stat, 100 | 120 | 123))
            {
                return Err("invalid trusted experience buff contribution".into());
            }
            // HumanObject.RefreshBuffs excludes every paused buff's stats.
            // Keep its clock row so accepted Zone movement can resume it.
            if buff.paused_remaining_ms.is_some()
                || buff.expires_at_ms.is_some_and(|expiry| now_ms >= expiry)
            {
                continue;
            }
            lover |= buff.lover_buff;
            mentee |= buff.mentee_buff;
            for entry in &buff.stats {
                add(entry)?;
            }
        }
        for entry in &self.guild_stats {
            add(entry)?;
        }
        Ok((rates, lover, mentee))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneExperienceProfile {
    pub account_id: String,
    pub character_index: i32,
    #[serde(default, skip_serializing_if = "profile_level_is_unknown")]
    pub level: u16,
    #[serde(default, skip_serializing_if = "experience_is_allowed")]
    pub no_experience: bool,
    #[serde(
        default = "gain_experience_is_allowed",
        skip_serializing_if = "gain_experience_is_enabled"
    )]
    pub can_gain_experience: bool,
    pub guild: Option<ZoneGuildExperienceMembership>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lover_partner: Option<ZoneExperiencePartner>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mentee_partner: Option<ZoneExperiencePartner>,
    #[serde(
        default = "source_level_difference",
        skip_serializing_if = "level_difference_is_source_default"
    )]
    pub reduce_monster_level_difference: bool,
    #[serde(
        default = "source_world_rate_bits",
        skip_serializing_if = "world_rate_is_source_default"
    )]
    pub world_experience_rate_bits: u32,
    pub monster_multiplier: u32,
    pub natural_kill_multiplier: u32,
    pub lover_eligible: bool,
    pub lover_rate_percent: i32,
    pub mentee_eligible: bool,
    pub mentee_rate_percent: i32,
    pub experience_rate_percent: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_source: Option<ZoneExperienceRateSource>,
    /// Exact source f32 bits preserve the original per-event truncation rule.
    pub guild_experience_rate_bits: u32,
}

fn source_level_difference() -> bool {
    true
}
fn experience_is_allowed(no_experience: &bool) -> bool {
    !*no_experience
}
fn profile_level_is_unknown(value: &u16) -> bool {
    *value == 0
}
fn gain_experience_is_allowed() -> bool {
    true
}
fn gain_experience_is_enabled(value: &bool) -> bool {
    *value
}
fn source_buff_instance_is_absent(value: &u64) -> bool {
    *value == 0
}
fn level_difference_is_source_default(value: &bool) -> bool {
    *value
}
fn source_world_rate_bits() -> u32 {
    1.0f32.to_bits()
}
fn world_rate_is_source_default(value: &u32) -> bool {
    *value == source_world_rate_bits()
}

/// Crystal HumanObject.ReduceExp uses uint division before Math.Round. Its
/// default rounding is nearest-even; the result is at least one before the
/// world rate is applied. Invalid server metadata/rates reject the capture.
fn source_reduced_experience(
    amount: u32,
    owner_level: u16,
    monster_level: u16,
    reduce: bool,
) -> Result<u32, String> {
    if amount > i32::MAX as u32 {
        return Err("source monster experience exceeds signed WinExp range".into());
    }
    let threshold = u32::from(monster_level) + 10;
    let difference = u32::from(owner_level).saturating_sub(threshold);
    let penalty = if reduce && u32::from(owner_level) >= threshold {
        (f64::from((amount / 15).max(1)) * f64::from(difference)).round_ties_even() as i64
    } else {
        0
    };
    Ok((i64::from(amount) - penalty).max(1) as u32)
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
    pub(crate) fn refresh_captured_rate_projection(&mut self) -> Result<(), String> {
        if let Some(source) = &self.rate_source {
            let (rates, _, _) = source.rates_at(source.captured_at_ms)?;
            self.experience_rate_percent = rates[0];
            self.lover_rate_percent = rates[1];
            self.mentee_rate_percent = rates[2];
        }
        Ok(())
    }

    pub(crate) fn gain_rates_at(&self, now_ms: u64) -> Result<[i32; 3], String> {
        let (mut rates, lover_buff, mentee_buff) = match &self.rate_source {
            Some(source) => source.rates_at(now_ms)?,
            None => (
                [
                    self.experience_rate_percent,
                    self.lover_rate_percent,
                    self.mentee_rate_percent,
                ],
                true,
                true,
            ),
        };
        if !self.lover_eligible || !lover_buff {
            rates[1] = 0;
        }
        if !self.mentee_eligible || !mentee_buff {
            rates[2] = 0;
        }
        Ok(rates)
    }

    /// Original quest completion casts the f32 world-rate product to uint before
    /// calling GainExp. Invalid trusted configuration must not mint a wrapped or
    /// saturated reward. Monster WinExp separately uses the signed-int limit.
    pub(crate) fn quest_world_experience(amount: u32, rate: f32) -> Result<u32, String> {
        let product = amount as f32 * rate;
        if !rate.is_finite()
            || rate < 0.0
            || !product.is_finite()
            || product < 0.0
            || f64::from(product.trunc()) > f64::from(u32::MAX)
        {
            return Err("invalid source quest world experience rate or amount".into());
        }
        Ok(product.trunc() as u32)
    }

    pub(super) fn win_experience(
        &self,
        amount: u32,
        owner_level: u16,
        monster_level: u16,
    ) -> Result<u32, String> {
        if self.no_experience {
            return Ok(0);
        }
        let amount = source_reduced_experience(
            amount,
            owner_level,
            monster_level,
            self.reduce_monster_level_difference,
        )?;
        let rate = f32::from_bits(self.world_experience_rate_bits);
        let scaled = amount as f32 * rate;
        if !rate.is_finite() || rate < 0.0 || !scaled.is_finite() || scaled >= 2_147_483_648.0 {
            return Err("invalid source world experience rate or WinExp overflow".into());
        }
        (scaled as u32)
            .checked_mul(self.monster_multiplier)
            .and_then(|amount| amount.checked_mul(self.natural_kill_multiplier))
            .ok_or_else(|| "source owner experience multiplier overflow".into())
    }

    pub(super) fn select(
        &self,
        source_zone: String,
        monster_object_id: u32,
        killed_at_ms: u64,
        raw_share: u32,
    ) -> Result<ZoneExperienceSelection, String> {
        self.select_after_win_experience(
            source_zone,
            monster_object_id,
            killed_at_ms,
            self.win_experience(raw_share, 0, 0)?,
        )
    }

    /// Crystal WinExp applies the owner's world rates before splitting a
    /// party. Each recipient then runs its own ordered GainExp percentages.
    pub(super) fn select_after_win_experience(
        &self,
        source_zone: String,
        monster_object_id: u32,
        killed_at_ms: u64,
        amount: u32,
    ) -> Result<ZoneExperienceSelection, String> {
        if self.account_id.is_empty()
            || self
                .guild
                .as_ref()
                .is_some_and(|guild| guild.guild_id.is_empty())
        {
            return Err("reward profile has no stable identity".into());
        }
        let rates = self.gain_rates_at(killed_at_ms)?;
        let final_amount = crate::apply_crystal_experience_rates(
            if self.no_experience || !self.can_gain_experience {
                0
            } else {
                amount
            },
            rates[1],
            rates[2],
            rates[0],
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
            level: 0,
            no_experience: false,
            can_gain_experience: true,
            guild: Some(ZoneGuildExperienceMembership {
                guild_id: "guild-a".into(),
                membership_epoch: 3,
            }),
            lover_partner: None,
            mentee_partner: None,
            reduce_monster_level_difference: true,
            world_experience_rate_bits: 1.0f32.to_bits(),
            monster_multiplier: 2,
            natural_kill_multiplier: 1,
            lover_eligible: true,
            lover_rate_percent: 5,
            mentee_eligible: false,
            mentee_rate_percent: 10,
            experience_rate_percent: 20,
            rate_source: None,
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

    fn profile() -> ZoneExperienceProfile {
        ZoneExperienceProfile {
            account_id: "owner".into(),
            character_index: 7,
            level: 0,
            no_experience: false,
            can_gain_experience: true,
            guild: None,
            lover_partner: None,
            mentee_partner: None,
            reduce_monster_level_difference: true,
            world_experience_rate_bits: 1.0f32.to_bits(),
            monster_multiplier: 1,
            natural_kill_multiplier: 1,
            lover_eligible: false,
            lover_rate_percent: 0,
            mentee_eligible: false,
            mentee_rate_percent: 0,
            experience_rate_percent: 0,
            rate_source: None,
            guild_experience_rate_bits: 0.01f32.to_bits(),
        }
    }

    #[test]
    fn source_reduce_experience_uses_owner_level_integer_division_and_minimum_one() {
        let profile = profile();
        assert_eq!(
            profile.win_experience(101, 12, 9).unwrap(),
            101,
            "level difference three is below source threshold"
        );
        assert_eq!(
            profile.win_experience(101, 19, 9).unwrap(),
            101,
            "difference ten removes zero"
        );
        assert_eq!(
            profile.win_experience(101, 22, 9).unwrap(),
            83,
            "101/15 truncates to six before multiplying three"
        );
        assert_eq!(profile.win_experience(101, 29, 9).unwrap(), 41);
        assert_eq!(profile.win_experience(1, 100, 1).unwrap(), 1);
        assert_eq!(profile.win_experience(0, 1, 1).unwrap(), 1);
        let mut disabled = profile.clone();
        disabled.reduce_monster_level_difference = false;
        assert_eq!(disabled.win_experience(101, 100, 1).unwrap(), 101);
    }

    #[test]
    fn source_win_experience_world_rate_truncates_before_owner_tiers_and_party() {
        let mut profile = profile();
        profile.world_experience_rate_bits = 1.3f32.to_bits();
        profile.monster_multiplier = 3;
        assert_eq!(profile.win_experience(11, 1, 1).unwrap(), 42);
        profile.world_experience_rate_bits = 0.0f32.to_bits();
        assert_eq!(profile.win_experience(11, 1, 1).unwrap(), 0);
        // Math.Round's source mode is explicitly nearest-even. Uint division
        // makes current ReduceExp penalties integral, so no fractional penalty
        // is invented to test an impossible live monster input.
        assert_eq!(2.5f64.round_ties_even(), 2.0);
        assert_eq!(3.5f64.round_ties_even(), 4.0);
    }

    #[test]
    fn source_win_experience_rejects_nonfinite_negative_and_overflow_rates() {
        let mut profile = profile();
        for rate in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, f32::MAX] {
            profile.world_experience_rate_bits = rate.to_bits();
            assert!(profile.win_experience(11, 1, 1).is_err(), "rate {rate:?}");
        }
        profile.world_experience_rate_bits = 1.0f32.to_bits();
        assert!(profile.win_experience(u32::MAX, 1, 1).is_err());
    }

    #[test]
    fn source_default_profile_fields_remain_absent_from_legacy_canonical_bytes() {
        let mut encoded = serde_json::to_value(profile()).unwrap();
        assert!(encoded.get("lover_partner").is_none());
        assert!(encoded.get("mentee_partner").is_none());
        assert!(encoded.get("no_experience").is_none());
        assert!(encoded.get("reduce_monster_level_difference").is_none());
        assert!(encoded.get("world_experience_rate_bits").is_none());
        assert!(encoded.get("rate_source").is_none());
        let restored: ZoneExperienceProfile = serde_json::from_value(encoded.clone()).unwrap();
        assert!(restored.reduce_monster_level_difference);
        assert!(!restored.no_experience);
        assert_eq!(restored.world_experience_rate_bits, 1.0f32.to_bits());
        assert!(restored.rate_source.is_none());
        encoded["reduce_monster_level_difference"] = false.into();
        assert!(
            !serde_json::from_value::<ZoneExperienceProfile>(encoded)
                .unwrap()
                .reduce_monster_level_difference
        );
    }

    fn rate_buff(
        expiry: Option<u64>,
        lover: bool,
        mentee: bool,
        stat: u8,
        value: i32,
    ) -> ZoneExperienceRateBuff {
        ZoneExperienceRateBuff {
            expires_at_ms: expiry,
            lover_buff: lover,
            mentee_buff: mentee,
            newbie_buff: false,
            pause_in_safe_zone: false,
            paused_remaining_ms: None,
            source_buff_key: None,
            source_buff_instance: 0,
            stats: vec![ZoneExperienceRateStat { stat, value }],
        }
    }

    #[test]
    fn paused_safe_rate_keeps_its_row_without_granting_stats_and_legacy_bytes_stay_absent() {
        let legacy = rate_buff(Some(1000), false, false, 100, 30);
        let encoded = serde_json::to_value(&legacy).unwrap();
        for field in [
            "pause_in_safe_zone",
            "paused_remaining_ms",
            "source_buff_key",
            "source_buff_instance",
        ] {
            assert!(encoded.get(field).is_none());
        }
        let restored: ZoneExperienceRateBuff = serde_json::from_value(encoded).unwrap();
        assert!(!restored.pause_in_safe_zone);
        assert_eq!(restored.source_buff_instance, 0);
        let mut paused = legacy;
        paused.pause_in_safe_zone = true;
        paused.source_buff_key = Some("exp".into());
        paused.source_buff_instance = 41;
        paused.paused_remaining_ms = Some(900);
        paused.expires_at_ms = None;
        let mut source = ZoneExperienceRateSource {
            captured_at_ms: 100,
            base_experience_rate_percent: 7,
            base_lover_rate_percent: 0,
            base_mentee_rate_percent: 0,
            buffs: vec![paused],
            guild_stats: vec![ZoneExperienceRateStat {
                stat: 100,
                value: 4,
            }],
        };
        assert_eq!(source.rates_at(100).unwrap().0, [11, 0, 0]);
        assert_eq!(source.rates_at(1_000_000).unwrap().0, [11, 0, 0]);
        source.buffs[0].paused_remaining_ms = None;
        source.buffs[0].expires_at_ms = Some(6000);
        assert_eq!(source.rates_at(5999).unwrap().0, [41, 0, 0]);
        assert_eq!(source.rates_at(6000).unwrap().0, [11, 0, 0]);
        source.buffs[0].paused_remaining_ms = Some(900);
        assert!(source.rates_at(6000).is_err());
        source.buffs[0].expires_at_ms = None;
        source.buffs[0].source_buff_instance = 0;
        assert!(source.rates_at(6000).is_err());
    }

    #[test]
    fn can_gain_experience_defaults_true_and_gates_recipient_after_owner_win_exp() {
        let mut owner = profile();
        let encoded = serde_json::to_value(&owner).unwrap();
        assert!(encoded.get("can_gain_experience").is_none());
        assert!(
            serde_json::from_value::<ZoneExperienceProfile>(encoded)
                .unwrap()
                .can_gain_experience
        );
        owner.can_gain_experience = false;
        assert_eq!(owner.win_experience(1000, 20, 10).unwrap(), 1000);
        let selected = owner
            .select_after_win_experience("source-zone".into(), 700, 200, 650)
            .unwrap();
        assert_eq!(selected.final_amount, 0);
        assert_eq!(selected.guild_amount, 0);
        let restored: ZoneExperienceProfile =
            serde_json::from_value(serde_json::to_value(owner).unwrap()).unwrap();
        assert!(!restored.can_gain_experience);
    }

    #[test]
    fn source_timed_rates_expire_at_death_without_removing_base_guild_or_prior_selection() {
        let mut profile = profile();
        profile.lover_eligible = true;
        profile.mentee_eligible = true;
        profile.guild = Some(ZoneGuildExperienceMembership {
            guild_id: "original-guild".into(),
            membership_epoch: 7,
        });
        profile.rate_source = Some(ZoneExperienceRateSource {
            captured_at_ms: 100,
            base_experience_rate_percent: 7,
            base_lover_rate_percent: 1,
            base_mentee_rate_percent: 2,
            buffs: vec![
                rate_buff(None, false, false, 100, 5),
                rate_buff(Some(200), false, false, 100, 20),
                rate_buff(Some(200), true, false, 120, 4),
                rate_buff(Some(300), false, true, 123, 8),
            ],
            guild_stats: vec![ZoneExperienceRateStat {
                stat: 100,
                value: 4,
            }],
        });
        assert_eq!(profile.gain_rates_at(199).unwrap(), [36, 5, 10]);
        assert_eq!(profile.gain_rates_at(200).unwrap(), [16, 0, 10]);
        assert_eq!(profile.gain_rates_at(300).unwrap(), [16, 0, 0]);
        let before = profile
            .select_after_win_experience("source-map".into(), 700, 199, 1000)
            .unwrap();
        let at_general_and_lover_expiry = profile
            .select_after_win_experience("source-map".into(), 701, 200, 1000)
            .unwrap();
        let at_mentee_expiry = profile
            .select_after_win_experience("source-map".into(), 702, 300, 1000)
            .unwrap();
        assert_eq!((before.final_amount, before.guild_amount), (1570, 15));
        assert_eq!(
            (
                at_general_and_lover_expiry.final_amount,
                at_general_and_lover_expiry.guild_amount,
            ),
            (1276, 12)
        );
        assert_eq!(
            (at_mentee_expiry.final_amount, at_mentee_expiry.guild_amount),
            (1160, 11)
        );
        // A delayed recipient consumes these exact issued bytes, not a later
        // expiry, relationship or Guild profile recomputation.
        profile.rate_source.as_mut().unwrap().buffs.clear();
        profile.guild = None;
        let restored: ZoneExperienceSelection =
            serde_json::from_slice(&serde_json::to_vec(&before).unwrap()).unwrap();
        assert_eq!(restored, before);
        assert_eq!(restored.guild.unwrap().membership_epoch, 7);
        assert_eq!(restored.final_amount, 1570);
    }

    #[test]
    fn source_rate_order_saturates_and_invalid_expired_metadata_is_rejected() {
        let mut profile = profile();
        profile.rate_source = Some(ZoneExperienceRateSource {
            captured_at_ms: 10,
            base_experience_rate_percent: i32::MAX,
            base_lover_rate_percent: 0,
            base_mentee_rate_percent: 0,
            buffs: vec![
                rate_buff(None, false, false, 100, 1),
                rate_buff(None, false, false, 100, -1),
            ],
            guild_stats: vec![],
        });
        assert_eq!(profile.gain_rates_at(10).unwrap(), [i32::MAX - 1, 0, 0]);
        profile.rate_source.as_mut().unwrap().buffs.push(rate_buff(
            Some(5),
            false,
            false,
            255,
            100,
        ));
        assert!(profile
            .select_after_win_experience("source-map".into(), 700, 10, 1000)
            .is_err());
    }

    #[test]
    fn source_no_experience_blocks_owner_and_recipient_without_losing_selection() {
        let mut profile = profile();
        profile.no_experience = true;
        profile.guild = Some(ZoneGuildExperienceMembership {
            guild_id: "source-guild".into(),
            membership_epoch: 9,
        });
        assert_eq!(profile.win_experience(1000, 10, 10).unwrap(), 0);
        let selected = profile
            .select_after_win_experience("source-map".into(), 700, 90, 1000)
            .unwrap();
        assert_eq!((selected.final_amount, selected.guild_amount), (0, 0));
        assert_eq!(selected.guild.unwrap().membership_epoch, 9);
    }

    #[test]
    fn source_quest_world_rate_truncates_unsigned_reward_and_rejects_invalid_product() {
        assert_eq!(
            ZoneExperienceProfile::quest_world_experience(11, 1.3).unwrap(),
            14
        );
        assert_eq!(
            ZoneExperienceProfile::quest_world_experience(11, 0.0).unwrap(),
            0
        );
        assert_eq!(
            ZoneExperienceProfile::quest_world_experience(0, 1.0).unwrap(),
            0
        );
        for rate in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, f32::MAX] {
            assert!(ZoneExperienceProfile::quest_world_experience(11, rate).is_err());
        }
        assert!(ZoneExperienceProfile::quest_world_experience(u32::MAX, 1.0).is_err());
    }
}
