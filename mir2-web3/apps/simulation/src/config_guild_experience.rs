//! Source Guild.GainExp arithmetic and durable source-event journal types.
//! Integration consumes this journal in the same transaction as the complete
//! character checkpoint. A balance difference is never an XP award source.
use super::*;

pub(super) const GUILD_COMMIT_OUTCOME_UNKNOWN: &str = "GUILD_COMMIT_OUTCOME_UNKNOWN";

#[cfg(test)]
impl SimulationConfig {
    pub(crate) fn inject_guild_xp_unknown_publication_probe(&self) {
        self.inject_account_store_repository_writer_probe(Err(format!(
            "{GUILD_COMMIT_OUTCOME_UNKNOWN}: simulated lost COMMIT response"
        )));
    }
    pub(crate) fn guild_xp_publication_probe_attempts(&self) -> usize {
        self.account_store_repository_writer_probe_invocations()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildExperienceJournal {
    /// Monotonic sequence saved with the source checkpoint; never separately
    /// reserved or reset on logout. Pending events are cleared only at commit.
    #[serde(default)]
    pub next_sequence: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub event_payloads: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending: Vec<GuildExperienceEvent>,
    /// Shared kill keys must survive a new gateway service/session instance.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub applied_kill_receipts: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub kill_outcomes: BTreeMap<String, GuildExperienceKillOutcome>,
}
impl GuildExperienceJournal {
    pub fn is_empty(&self) -> bool {
        self.next_sequence == 0
            && self.pending.is_empty()
            && self.event_payloads.is_empty()
            && self.applied_kill_receipts.is_empty()
            && self.kill_outcomes.is_empty()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildExperienceEvent {
    pub sequence: u64,
    pub source_revision: u64,
    pub event_id: String,
    pub identity: Stage5FriendIdentity,
    pub guild_id: String,
    pub membership_epoch: u64,
    /// Actual GainExp amount after player multipliers, before level rollover.
    pub final_amount: u32,
    /// Set only by the trusted shared reward consumer, never by an imported save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_hash: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum GuildExperienceKillOutcome {
    Credited { guild_amount: u32 },
    SkippedDisbanded,
    NoGuild,
    NoExperience,
    LegacyNoCapture,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GuildExperienceDelta {
    pub amount: u32,
    pub leveled: bool,
}

pub(crate) fn apply_source_gain(
    guild: &mut SharedGuildRecord,
    amount: u32,
    settings: &mir2_game_data::CrystalGuildSettings,
) -> Result<GuildExperienceDelta, String> {
    if !settings.experience_rate.is_finite() || settings.experience_rate < 0.0 {
        return Err("invalid source guild experience rate".into());
    }
    // Crystal multiplies uint by float and truncates EACH event. Batching small
    // events before multiplying would manufacture XP absent from the source.
    let scaled = amount as f32 * settings.experience_rate;
    if !scaled.is_finite() || f64::from(scaled) > f64::from(u32::MAX) {
        return Err("guild experience award exhausted".into());
    }
    let gained = scaled as u32;
    apply_captured_gain(guild, gained, settings)
}

pub(crate) fn apply_captured_gain(
    guild: &mut SharedGuildRecord,
    gained: u32,
    settings: &mir2_game_data::CrystalGuildSettings,
) -> Result<GuildExperienceDelta, String> {
    let mut threshold = settings
        .experience_levels
        .get(usize::from(guild.level))
        .copied()
        .unwrap_or(0);
    if threshold <= 0 {
        return Ok(GuildExperienceDelta {
            amount: 0,
            leveled: false,
        });
    }
    if gained == 0 {
        return Ok(GuildExperienceDelta {
            amount: 0,
            leveled: false,
        });
    }
    let total = guild
        .experience
        .checked_add(u64::from(gained))
        .ok_or("guild experience exhausted")?;
    let mut remainder = total;
    let mut level = guild.level;
    let mut points = guild.spare_points;
    while threshold > 0 && remainder > threshold as u64 {
        level = level.checked_add(1).ok_or("guild level exhausted")?;
        points = points.saturating_add(settings.points_per_level);
        remainder -= threshold as u64;
        threshold = settings
            .experience_levels
            .get(usize::from(level))
            .copied()
            .unwrap_or(0);
        if threshold == 0 || level == u8::MAX {
            break;
        }
    }
    let leveled = level != guild.level;
    // Deliberate source quirk: GuildObject only subtracts from its local
    // remainder, never Info.Experience. Do not silently correct its economy.
    guild.experience = total;
    guild.level = level;
    guild.spare_points = points;
    Ok(GuildExperienceDelta {
        amount: gained,
        leveled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn guild() -> SharedGuildRecord {
        let config = SimulationConfig::default();
        let guild = guild_clock::schema_tests::fixture_guild(&config.account_store.lock().unwrap());
        guild
    }
    #[test]
    fn guild_xp_source_rounding_is_per_event_and_boundary_is_strict() {
        let settings = mir2_game_data::crystal_guild_settings();
        let mut guild = guild();
        for _ in 0..100 {
            assert_eq!(
                apply_source_gain(&mut guild, 99, settings).unwrap().amount,
                0
            );
        }
        assert_eq!(guild.experience, 0);
        guild.experience = settings.experience_levels[0] as u64 - 1;
        apply_source_gain(&mut guild, 100, settings).unwrap();
        assert_eq!(guild.level, 0);
        apply_source_gain(&mut guild, 100, settings).unwrap();
        assert_eq!(guild.level, 1);
        assert_eq!(guild.spare_points, settings.points_per_level);
        assert_eq!(guild.experience, settings.experience_levels[0] as u64 + 1);
    }
    #[test]
    fn guild_xp_keeps_original_accumulated_experience_quirk_and_terminal_level() {
        let mut settings = mir2_game_data::crystal_guild_settings().clone();
        settings.experience_rate = 1.0;
        settings.experience_levels = vec![10, 20];
        let mut guild = guild();
        guild.spare_points = 254;
        apply_source_gain(&mut guild, 11, &settings).unwrap();
        assert_eq!(
            (guild.level, guild.experience, guild.spare_points),
            (1, 11, 255)
        );
        // The stored eleven is used again, rather than the local remainder one.
        apply_source_gain(&mut guild, 10, &settings).unwrap();
        assert_eq!(
            (guild.level, guild.experience, guild.spare_points),
            (2, 21, 255)
        );
        assert_eq!(
            apply_source_gain(&mut guild, 100, &settings)
                .unwrap()
                .amount,
            0
        );
        assert_eq!(guild.experience, 21);
    }
    #[test]
    fn guild_xp_overflow_is_rejected_without_partial_progress() {
        let settings = mir2_game_data::crystal_guild_settings();
        let mut guild = guild();
        guild.experience = u64::MAX;
        let before = guild.clone();
        assert!(apply_source_gain(&mut guild, 100, settings).is_err());
        assert_eq!(guild, before);
    }
}

#[path = "config_guild_experience_commit.rs"]
mod commit;
pub(super) use commit::{settle_authorized_sources, validate_import};
#[path = "config_guild_experience_permit.rs"]
mod permit;
pub(crate) use permit::GuildExperienceCommitPermit;

#[cfg(test)]
#[path = "config_guild_experience_tests.rs"]
mod transaction_tests;

pub(crate) use commit::event_hash as source_event_hash;
