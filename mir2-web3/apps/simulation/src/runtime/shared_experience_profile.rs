//! Server-only reward inputs from the authenticated personal session.
//! Presence/range/life/group are deliberately evaluated later by the Zone.
use super::resources::{
    runtime_tick, BuffResource, MapRuntimeResource, RuntimeConfigResource, SessionResource,
    Stage5SystemsResource,
};
use super::session::SimulationSession;
use super::zone::{
    ZoneChatProfile, ZoneExperiencePartner, ZoneExperienceProfile, ZoneExperienceRateBuff,
    ZoneExperienceRateSource, ZoneExperienceRateStat, ZoneGuildExperienceMembership, ZoneKey,
};
use crate::{AccountStore, SimulationConfig, Stage5FriendIdentity, Stage5SystemsState};
use bevy_ecs::prelude::{Resource, World};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Resource, Debug)]
struct SharedPersonalExperienceContext {
    profile: Option<ZoneExperienceProfile>,
    zone: Option<ZoneKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TickExperienceBuffKey {
    key: String,
    expires_at_tick: u64,
    stats: Vec<(u8, i32)>,
}

#[derive(Debug, Default)]
struct ExperienceRateClockState {
    identity: Option<Stage5FriendIdentity>,
    anchors: BTreeMap<TickExperienceBuffKey, u64>,
}

/// Ephemeral own-runtime clock. A repeated projection cannot extend a finite
/// buff merely because an idle TCP character's action tick did not advance.
/// This is never a shared peer lock or part of a full character checkpoint.
#[derive(Resource, Debug, Default)]
struct SharedExperienceRateClock(Mutex<ExperienceRateClockState>);

fn experience_now_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "source experience clock predates Unix epoch".to_string())
        .and_then(|duration| {
            u64::try_from(duration.as_millis())
                .map_err(|_| "source experience clock overflows milliseconds".to_string())
        })
}

fn experience_stat(stat: u8) -> bool {
    matches!(stat, 100 | 120 | 123)
}

fn capture_personal_rate_source(
    world: &World,
    identity: &Stage5FriendIdentity,
    now_ms: u64,
) -> Result<ZoneExperienceRateSource, String> {
    let base = super::stats::player_experience_base_rates(world);
    let tick = runtime_tick(world);
    let mut detached = ExperienceRateClockState::default();
    let mut guard = world
        .get_resource::<SharedExperienceRateClock>()
        .map(|clock| {
            clock
                .0
                .lock()
                .map_err(|_| "own experience rate clock poisoned")
        })
        .transpose()?;
    let clock = guard.as_deref_mut().unwrap_or(&mut detached);
    if clock.identity.as_ref() != Some(identity) {
        clock.identity = Some(identity.clone());
        clock.anchors.clear();
    }
    let mut seen = BTreeSet::new();
    let mut buffs = Vec::new();
    for buff in &world.resource::<BuffResource>().buffs {
        let stats = buff
            .stats
            .iter()
            .filter(|entry| experience_stat(entry.stat))
            .map(|entry| ZoneExperienceRateStat {
                stat: entry.stat,
                value: entry.value,
            })
            .collect::<Vec<_>>();
        let lover_buff = buff.key == "lover";
        let mentee_buff = buff.key == "mentee";
        let pause_duration = matches!(buff.key.as_str(), "exp" | "drop")
            .then_some(buff.real_time_duration.as_ref())
            .flatten();
        let pause_in_safe_zone = pause_duration.is_some();
        if stats.is_empty() && !lover_buff && !mentee_buff && !pause_in_safe_zone {
            continue;
        }
        let paused_remaining_ms = pause_duration
            .filter(|duration| duration.is_paused())
            .map(|duration| duration.remaining_ms());
        let expires_at_ms = if paused_remaining_ms.is_some()
            || (buff.real_time_duration.is_none() && buff.expires_at_tick == u64::MAX)
        {
            None
        } else {
            let proposed = now_ms
                .checked_add(buff.remaining_ms(tick))
                .ok_or("trusted experience buff expiry overflows milliseconds")?;
            if buff.real_time_duration.is_some() {
                // The monotonic personal duration already preserves its anchor.
                Some(proposed)
            } else {
                let key = TickExperienceBuffKey {
                    key: buff.key.clone(),
                    expires_at_tick: buff.expires_at_tick,
                    stats: stats
                        .iter()
                        .map(|entry| (entry.stat, entry.value))
                        .collect(),
                };
                seen.insert(key.clone());
                let anchor = clock.anchors.entry(key).or_insert(proposed);
                *anchor = (*anchor).min(proposed);
                Some(*anchor)
            }
        };
        buffs.push(ZoneExperienceRateBuff {
            expires_at_ms,
            lover_buff,
            mentee_buff,
            newbie_buff: buff.key == "newbie",
            pause_in_safe_zone,
            paused_remaining_ms,
            source_buff_key: pause_in_safe_zone.then(|| buff.key.clone()),
            source_buff_instance: pause_duration.map_or(0, |duration| duration.instance_id()),
            stats,
        });
    }
    // Retain expired-but-still-present finite buffers to prevent resurrection;
    // removed/replaced buffers do not accumulate an unbounded history.
    clock.anchors.retain(|key, _| seen.contains(key));
    Ok(ZoneExperienceRateSource {
        captured_at_ms: now_ms,
        base_experience_rate_percent: base[0],
        base_lover_rate_percent: base[1],
        base_mentee_rate_percent: base[2],
        buffs,
        guild_stats: Vec::new(),
    })
}

fn guild_rate_stats_from_store(
    store: &AccountStore,
    actor: &Stage5FriendIdentity,
) -> Vec<ZoneExperienceRateStat> {
    let Some(guild) = store
        .shared_guilds
        .values()
        .find(|guild| guild.member(actor).is_some())
    else {
        return Vec::new();
    };
    mir2_game_data::crystal_guild_buff_definitions()
        .iter()
        .filter(|definition| {
            guild
                .buffs
                .get(&definition.id)
                .is_some_and(|buff| buff.active)
        })
        .flat_map(|definition| &definition.stats)
        .filter(|entry| experience_stat(entry.stat))
        .map(|entry| ZoneExperienceRateStat {
            stat: entry.stat,
            value: entry.value,
        })
        .collect()
}

fn newbie_guild_member(
    config: &SimulationConfig,
    store: &AccountStore,
    actor: &Stage5FriendIdentity,
) -> bool {
    mir2_game_data::crystal_guild_settings().newbie_guild_buff_enabled
        && store.shared_guilds.values().any(|guild| {
            guild.name == config.crystal_newbie_guild_name && guild.member(actor).is_some()
        })
}

fn current_map_blocks_experience(world: &World) -> bool {
    let map = world.resource::<MapRuntimeResource>();
    let config = &world.resource::<RuntimeConfigResource>().config;
    mir2_game_data::crystal_map_respawns_ref(&map.current_map.file_name)
        .is_some_and(|source| source.no_experience)
        || super::map::current_map_drop_rule(config, map).is_some_and(|rule| rule.no_experience)
}

/// PlayerObject.CompleteQuest scales the raw quest reward by Settings.ExpRate
/// before GainExp. NPC GiveExp and mentor payouts enter GainExp directly.
pub(super) fn crystal_apply_quest_experience_source(world: &World, amount: u32) -> u32 {
    if current_map_blocks_experience(world) {
        return 0;
    }
    let rate = world
        .resource::<RuntimeConfigResource>()
        .config
        .crystal_world_experience_rate;
    let Ok(amount) = ZoneExperienceProfile::quest_world_experience(amount, rate) else {
        return 0;
    };
    super::stats::crystal_apply_social_exp_rate(world, amount)
}

/// Shared-mode personal GainExp never resolves relationships from names. The
/// command's admission uses a trusted current Zone view; the own runtime then
/// checks its identity, relationship epoch, current map and actual live buff.
pub(super) fn apply_current_personal_experience_rates(world: &World, amount: u32) -> u32 {
    if current_map_blocks_experience(world)
        || !super::default_npc_events::can_gain_experience(world)
    {
        return 0;
    }
    let context = world.get_resource::<SharedPersonalExperienceContext>();
    let session = world.resource::<SessionResource>();
    let (Some(account_id), Some(character)) = (&session.account_id, &session.selected_character)
    else {
        return 0;
    };
    let identity = Stage5FriendIdentity {
        account_id: account_id.clone(),
        character_index: character.index,
    };
    let Ok(now_ms) = experience_now_ms() else {
        return 0;
    };
    let Ok(mut source) = capture_personal_rate_source(world, &identity, now_ms) else {
        return 0;
    };
    // Personal GainExp reads the current own Guild stats, as RefreshStats did.
    // Monster source capture separately rebases this vector under admission.
    let config = &world.resource::<RuntimeConfigResource>().config;
    let Ok(guild_stats) = config.shared_guild_active_stats(&identity) else {
        return 0;
    };
    source.guild_stats = guild_stats
        .into_iter()
        .filter(|entry| experience_stat(entry.stat))
        .map(|entry| ZoneExperienceRateStat {
            stat: entry.stat,
            value: entry.value,
        })
        .collect();
    if source.buffs.iter().any(|buff| buff.newbie_buff) {
        let Ok(guild) = config.shared_guild_for_identity(&identity) else {
            return 0;
        };
        let newbie = mir2_game_data::crystal_guild_settings().newbie_guild_buff_enabled
            && guild.is_some_and(|guild| guild.name == config.crystal_newbie_guild_name);
        source.buffs.retain(|buff| !buff.newbie_buff || newbie);
    }
    let Ok((rates, lover_buff, mentee_buff)) = source.rates_at(now_ms) else {
        return 0;
    };
    let profile = context
        .and_then(|context| context.profile.as_ref())
        .filter(|profile| {
            session.account_id.as_deref() == Some(profile.account_id.as_str())
                && session
                    .selected_character
                    .as_ref()
                    .is_some_and(|character| character.index == profile.character_index)
                && context
                    .and_then(|context| context.zone.as_ref())
                    .is_some_and(|zone| {
                        world
                            .resource::<MapRuntimeResource>()
                            .current_map
                            .file_name
                            .eq_ignore_ascii_case(&zone.map_file_name)
                    })
        });
    let systems = &world.resource::<Stage5SystemsResource>().stage5_systems;
    let lover = profile.is_some_and(|profile| {
        profile.lover_eligible
            && lover_buff
            && profile.lover_partner.as_ref().is_some_and(|partner| {
                systems.relationship.partner_identity.as_ref() == Some(&partner.identity)
                    && systems.relationship.authority_revision == partner.relationship_epoch
            })
    });
    let mentee = profile.is_some_and(|profile| {
        profile.mentee_eligible
            && mentee_buff
            && !systems.mentor.is_mentor
            && profile.mentee_partner.as_ref().is_some_and(|partner| {
                systems.mentor.partner_identity.as_ref() == Some(&partner.identity)
                    && systems.mentor.ledger.relationship_epoch == partner.relationship_epoch
            })
    });
    crate::apply_crystal_experience_rates(
        amount,
        if lover { rates[1] } else { 0 },
        if mentee { rates[2] } else { 0 },
        rates[0],
    )
}

fn systems_for(
    store: &AccountStore,
    identity: &Stage5FriendIdentity,
) -> Result<Stage5SystemsState, String> {
    let save = store
        .accounts
        .get(&identity.account_id)
        .and_then(|account| account.saves.get(&identity.character_index))
        .ok_or("experience profile character checkpoint missing")?;
    save.stage5_systems_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map(|systems| systems.unwrap_or_default())
        .map_err(|error| format!("invalid experience relationship authority: {error}"))
}

fn guild_projection_from_store(
    config: &SimulationConfig,
    store: &AccountStore,
    actor: &Stage5FriendIdentity,
) -> (
    Option<String>,
    Vec<String>,
    Option<ZoneGuildExperienceMembership>,
) {
    let guild = store
        .shared_guilds
        .values()
        .find(|guild| guild.member(actor).is_some());
    let membership = guild
        .filter(|guild| guild.name != config.crystal_newbie_guild_name)
        .map(|guild| ZoneGuildExperienceMembership {
            guild_id: guild.id.clone(),
            membership_epoch: guild
                .member(actor)
                .expect("membership selected")
                .membership_epoch,
        });
    let name = guild.map(|guild| guild.name.clone());
    let wars = guild
        .map(|guild| {
            guild
                .active_wars
                .keys()
                .filter_map(|enemy| {
                    store
                        .shared_guilds
                        .get(enemy)
                        .map(|guild| guild.name.clone())
                })
                .collect()
        })
        .unwrap_or_default();
    (name, wars, membership)
}

fn refresh_experience_authority_from_store(
    config: &SimulationConfig,
    store: &AccountStore,
    profile: &mut ZoneExperienceProfile,
) -> Result<(), String> {
    let identity = Stage5FriendIdentity {
        account_id: profile.account_id.clone(),
        character_index: profile.character_index,
    };
    if identity.account_id.is_empty() {
        return Err("experience profile account missing".into());
    }
    let systems = systems_for(store, &identity)?;
    if let Some(source) = &mut profile.rate_source {
        source.guild_stats = guild_rate_stats_from_store(store, &identity);
        let newbie = newbie_guild_member(config, store, &identity);
        source.buffs.retain(|buff| !buff.newbie_buff || newbie);
    }
    profile.refresh_captured_rate_projection()?;
    profile.guild = store
        .shared_guilds
        .values()
        // Crystal excludes this exact source guild name from GainExp.
        .find(|guild| {
            guild.name != config.crystal_newbie_guild_name && guild.member(&identity).is_some()
        })
        .map(|guild| ZoneGuildExperienceMembership {
            guild_id: guild.id.clone(),
            membership_epoch: guild
                .member(&identity)
                .expect("membership selected")
                .membership_epoch,
        });
    profile.lover_partner = systems
        .relationship
        .partner_identity
        .as_ref()
        .filter(|partner| *partner != &identity)
        .map(|partner| -> Result<Option<ZoneExperiencePartner>, String> {
            let peer = systems_for(store, partner)?;
            Ok(
                (peer.relationship.partner_identity.as_ref() == Some(&identity)).then(|| {
                    ZoneExperiencePartner {
                        identity: partner.clone(),
                        relationship_epoch: systems.relationship.authority_revision,
                    }
                }),
            )
        })
        .transpose()?
        .flatten();
    profile.mentee_partner = if systems.mentor.is_mentor {
        None
    } else {
        systems
            .mentor
            .partner_identity
            .as_ref()
            .filter(|partner| *partner != &identity)
            .map(|partner| -> Result<Option<ZoneExperiencePartner>, String> {
                let peer = systems_for(store, partner)?;
                Ok((peer.mentor.is_mentor
                    && peer.mentor.partner_identity.as_ref() == Some(&identity)
                    && peer.mentor.ledger.relationship_epoch
                        == systems.mentor.ledger.relationship_epoch
                    && systems.mentor.ledger.relationship_epoch > 0)
                    .then(|| ZoneExperiencePartner {
                        identity: partner.clone(),
                        relationship_epoch: systems.mentor.ledger.relationship_epoch,
                    }))
            })
            .transpose()?
            .flatten()
    };
    if profile.lover_partner.is_none() {
        profile.lover_eligible = false;
    }
    if profile.mentee_partner.is_none() {
        profile.mentee_eligible = false;
    }
    Ok(())
}

impl SimulationConfig {
    /// One frozen-state check and authority read for all currently registered
    /// recipients. Gateway code cannot inspect a frozen AccountStore directly.
    pub fn shared_zone_guild_projections_for(
        &self,
        identities: &[Stage5FriendIdentity],
    ) -> Result<
        Vec<(
            Option<String>,
            Vec<String>,
            Option<ZoneGuildExperienceMembership>,
        )>,
        String,
    > {
        self.ensure_account_store_writable()?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "guild war authority unavailable")?;
        Ok(identities
            .iter()
            .map(|actor| guild_projection_from_store(self, &store, actor))
            .collect())
    }

    /// Refresh only authority fields; another character's stats/buffs always
    /// remain those captured by its own authenticated runtime.
    pub fn refresh_zone_experience_profile_authority(
        &self,
        profile: &mut ZoneExperienceProfile,
    ) -> Result<(), String> {
        self.ensure_account_store_writable()?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "experience profile authority mutex poisoned")?;
        self.ensure_account_store_writable()?;
        refresh_experience_authority_from_store(self, &store, profile)
    }

    /// Linearize source authority and Zone death issuance. The callback may
    /// rebase existing trusted profiles and handle/tick the Zone while this
    /// authority image is locked. It must not enter any personal runtime,
    /// persist a save, or call another AccountStore API. Release the callback
    /// before delivering/settling the resulting durable award.
    ///
    /// All relationship/Guild publications hold the same AccountStore mutex;
    /// this guard orders admission before or after closure even when its
    /// generation notification has not yet been published.
    pub fn with_shared_zone_experience_admission<T>(
        &self,
        admission: impl FnOnce(
            &dyn Fn(
                &Stage5FriendIdentity,
                &mut Option<ZoneExperienceProfile>,
                &mut ZoneChatProfile,
            ) -> Result<(), String>,
        ) -> Result<T, String>,
    ) -> Result<T, String> {
        self.ensure_account_store_writable()?;
        let store = self
            .account_store
            .lock()
            .map_err(|_| "experience profile authority mutex poisoned")?;
        self.ensure_account_store_writable()?;
        let refresh = |actor: &Stage5FriendIdentity,
                       profile: &mut Option<ZoneExperienceProfile>,
                       chat: &mut ZoneChatProfile| {
            let mut next = profile.clone();
            if let Some(profile) = &mut next {
                if profile.account_id != actor.account_id
                    || profile.character_index != actor.character_index
                {
                    return Err("experience profile differs from its admitted identity".into());
                }
                refresh_experience_authority_from_store(self, &store, profile)?;
            } else {
                // An old checkpoint has no trusted personal rates. Preserve
                // that absence while validating its registered character.
                systems_for(&store, actor)?;
            }
            let (guild_name, active_guild_wars, _) =
                guild_projection_from_store(self, &store, actor);
            *profile = next;
            chat.guild_name = guild_name;
            chat.active_guild_wars = active_guild_wars;
            Ok(())
        };
        admission(&refresh)
    }
}

impl SimulationSession {
    /// Reconcile only this owner's EXP/DROP duration instances from accepted
    /// Zone movement. An old row cannot overwrite a newly stacked/replaced
    /// potion, and a different character cannot mutate this private runtime.
    pub fn sync_shared_experience_buff_clocks(
        &mut self,
        profile: Option<&ZoneExperienceProfile>,
        now_ms: u64,
    ) -> Result<(), String> {
        let active = self.active_identity();
        if let Some(profile) = profile {
            let active = active
                .as_ref()
                .ok_or("experience clock has no active character")?;
            if active.account_id != profile.account_id
                || active.character_index != profile.character_index
            {
                return Err("experience clock belongs to another character".into());
            }
            if let Some(source) = &profile.rate_source {
                source.rates_at(now_ms)?;
            }
        }
        let tick = runtime_tick(self.app.world());
        let mut buffs = self.app.world_mut().resource_mut::<BuffResource>();
        for buff in &mut buffs.buffs {
            if !matches!(buff.key.as_str(), "exp" | "drop") {
                continue;
            }
            // Legacy saves had only action ticks. Upgrade each actual finite
            // instance once, so idle source capture and safe pause use time.
            if buff.real_time_duration.is_none() && buff.expires_at_tick != u64::MAX {
                buff.real_time_duration = Some(super::buffs::RealTimeBuffDuration::new(
                    buff.remaining_ms(tick),
                ));
            }
            let Some(duration) = &mut buff.real_time_duration else {
                continue;
            };
            let Some(row) = profile
                .and_then(|profile| profile.rate_source.as_ref())
                .and_then(|source| {
                    source.buffs.iter().find(|row| {
                        row.pause_in_safe_zone
                            && row.source_buff_key.as_deref() == Some(buff.key.as_str())
                            && row.source_buff_instance == duration.instance_id()
                    })
                })
            else {
                continue;
            };
            let (remaining, paused) = match (row.expires_at_ms, row.paused_remaining_ms) {
                (Some(expiry), None) => (expiry.saturating_sub(now_ms), false),
                (None, Some(remaining)) => (remaining, true),
                _ => return Err("experience clock is neither running nor paused".into()),
            };
            duration.set_remaining_and_paused(remaining, paused);
        }
        Ok(())
    }

    pub fn initialize_shared_experience_rate_clock(&mut self) {
        if !self
            .app
            .world()
            .contains_resource::<SharedExperienceRateClock>()
        {
            self.app
                .world_mut()
                .insert_resource(SharedExperienceRateClock::default());
        }
    }

    /// Only the Gateway's trusted Zone eligibility probe supplies this context.
    /// None clears it on pre-login/teardown rather than retaining a previous
    /// character's relationship flags.
    pub fn set_shared_personal_experience_context(
        &mut self,
        profile: Option<ZoneExperienceProfile>,
        zone: Option<ZoneKey>,
    ) -> Result<(), String> {
        if profile.is_some() != zone.is_some() {
            return Err("personal experience profile lacks its trusted Zone".into());
        }
        if let Some(profile) = &profile {
            let active = self
                .active_identity()
                .ok_or("personal experience context has no active character")?;
            if active.account_id != profile.account_id
                || active.character_index != profile.character_index
            {
                return Err("personal experience context belongs to another character".into());
            }
        }
        self.initialize_shared_experience_rate_clock();
        if self.active_identity().is_none() {
            *self
                .app
                .world()
                .resource::<SharedExperienceRateClock>()
                .0
                .lock()
                .map_err(|_| "own experience rate clock poisoned")? =
                ExperienceRateClockState::default();
        }
        self.app
            .world_mut()
            .insert_resource(SharedPersonalExperienceContext { profile, zone });
        Ok(())
    }

    /// Not a packet payload: the Gateway supplies this trusted projection to
    /// the currently admitted online character before authorizing normal play.
    pub fn active_zone_experience_profile(&self) -> Result<Option<ZoneExperienceProfile>, String> {
        self.active_zone_experience_profile_at(experience_now_ms()?)
    }

    /// Deterministic server clock input for prepared simulation tests. Gateway
    /// production uses the same Unix-millisecond basis as source killed_at_ms.
    pub fn active_zone_experience_profile_at(
        &self,
        now_ms: u64,
    ) -> Result<Option<ZoneExperienceProfile>, String> {
        let Some(active) = self.active_identity() else {
            return Ok(None);
        };
        let Some(config) = self.shared_mentor_config() else {
            return Ok(None);
        };
        let identity = Stage5FriendIdentity {
            account_id: active.account_id.clone(),
            character_index: active.character_index,
        };
        let rate_source = capture_personal_rate_source(self.app.world(), &identity, now_ms)?;
        let (rates, lover_buff, mentee_buff) = rate_source.rates_at(now_ms)?;
        let level = super::leveling::player_current_level(self.app.world());
        let natural_multiplier = std::env::var("MIR2_QA_NATURAL_KILL_EXPERIENCE_MULTIPLIER")
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(1)
            .min(1_000_000);
        let mut profile = ZoneExperienceProfile {
            account_id: active.account_id,
            character_index: active.character_index,
            level,
            no_experience: current_map_blocks_experience(self.app.world()),
            can_gain_experience: super::default_npc_events::can_gain_experience(self.app.world()),
            guild: None,
            lover_partner: None,
            mentee_partner: None,
            reduce_monster_level_difference: config.crystal_exp_mob_level_difference,
            world_experience_rate_bits: config.crystal_world_experience_rate.to_bits(),
            monster_multiplier: self
                .app
                .world()
                .resource::<RuntimeConfigResource>()
                .config
                .monster_experience_multiplier(level),
            natural_kill_multiplier: natural_multiplier,
            lover_eligible: lover_buff,
            lover_rate_percent: rates[1],
            mentee_eligible: mentee_buff,
            mentee_rate_percent: rates[2],
            experience_rate_percent: rates[0],
            rate_source: Some(rate_source),
            guild_experience_rate_bits: mir2_game_data::crystal_guild_settings()
                .experience_rate
                .to_bits(),
        };
        config.refresh_zone_experience_profile_authority(&mut profile)?;
        Ok(Some(profile))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountRecord, CharacterSaveRecord};
    use mir2_protocol::{ClientPacket, MirGridType, ServerPacket, UserItemStat};

    fn session() -> SimulationSession {
        let config = SimulationConfig::default();
        let actor = Stage5FriendIdentity {
            account_id: "demo".into(),
            character_index: config.default_character.index,
        };
        let spouse = Stage5FriendIdentity {
            account_id: "source-spouse".into(),
            character_index: actor.character_index + 1,
        };
        let teacher = Stage5FriendIdentity {
            account_id: "source-teacher".into(),
            character_index: actor.character_index + 2,
        };
        let mut systems = Stage5SystemsState::default();
        systems.relationship.partner_identity = Some(spouse.clone());
        systems.relationship.authority_revision = 3;
        systems.relationship.partner_name = "Source Spouse".into();
        systems.mentor.partner_identity = Some(teacher.clone());
        systems.mentor.ledger.relationship_epoch = 7;
        systems.mentor.name = "Source Teacher".into();
        {
            let mut store = config.account_store.lock().unwrap();
            store
                .accounts
                .get_mut("demo")
                .unwrap()
                .saves
                .get_mut(&actor.character_index)
                .unwrap()
                .stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
            for (id, name, mentor_role) in [
                (&spouse, "Source Spouse", false),
                (&teacher, "Source Teacher", true),
            ] {
                let mut character = config.default_character.clone();
                character.index = id.character_index;
                character.name = name.into();
                character.level = 50;
                let mut peer = Stage5SystemsState::default();
                if mentor_role {
                    peer.mentor.partner_identity = Some(actor.clone());
                    peer.mentor.is_mentor = true;
                    peer.mentor.ledger.relationship_epoch = 7;
                } else {
                    peer.relationship.partner_identity = Some(actor.clone());
                    peer.relationship.authority_revision = 3;
                }
                let mut save = CharacterSaveRecord::new(character.clone());
                save.stage5_systems_json = Some(serde_json::to_string(&peer).unwrap());
                let mut account = AccountRecord::empty();
                account.characters.push(character);
                account.saves.insert(id.character_index, save);
                store.accounts.insert(id.account_id.clone(), account);
            }
        }
        let mut session = SimulationSession::new(config.clone());
        session.enable_shared_guild_authority();
        assert!(session
            .handle_packet(ClientPacket::Login {
                account_id: "demo".into(),
                password: "demo".into()
            })
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        session.handle_packet(ClientPacket::StartGame {
            character_index: actor.character_index,
        });
        session
            .refresh_shared_social_buffs(
                &[
                    (spouse.account_id, spouse.character_index),
                    (teacher.account_id, teacher.character_index),
                ]
                .into_iter()
                .collect(),
            )
            .unwrap();
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .push(super::super::buffs::BuffState {
                key: "exp".into(),
                name: "Prepared Exp".into(),
                description: String::new(),
                expires_at_tick: u64::MAX,
                real_time_duration: None,
                attack_bonus: 0,
                defence_bonus: 0,
                stats: vec![UserItemStat {
                    stat: 100,
                    value: 20,
                }],
            });
        session
    }

    fn normal_use_prepared_exp_potion(
        session: &mut SimulationSession,
        index: i32,
        uid: u64,
    ) -> Vec<ServerPacket> {
        let template = mir2_game_data::crystal_item_by_index(index).unwrap();
        assert_eq!(template.item_type, 13);
        assert_eq!(template.shape, 4);
        assert_eq!(template.durability, 60);
        // Only the item acquisition is prepared. The ordinary UseItem handler
        // validates and consumes this real imported item and produces its buff.
        let mut item = super::super::items::embedded_item_state_from_template(
            &template,
            crate::ItemContainer::Bag1,
            31,
        );
        item.unique_id = uid;
        session
            .app
            .world_mut()
            .resource_mut::<super::super::resources::InventoryResource>()
            .inventory_items
            .push(item);
        let packets = session.handle_packet(ClientPacket::UseItem {
            unique_id: uid,
            grid: MirGridType::Inventory,
        });
        assert!(packets.iter()
            .any(|packet| matches!(packet, ServerPacket::UseItem { unique_id, success: true, .. } if *unique_id == uid)));
        assert!(session
            .app
            .world()
            .resource::<super::super::resources::InventoryResource>()
            .inventory_items
            .iter()
            .all(|item| item.unique_id != uid));
        packets
    }

    fn prepare_outside_imported_safe_zone(session: &mut SimulationSession) {
        use mir2_protocol::{MirDirection, Point};
        let point = Point { x: 339, y: 270 };
        let world = session.app.world();
        assert_eq!(
            world.resource::<MapRuntimeResource>().current_map.file_name,
            "0"
        );
        assert!(!super::super::map::is_safe_zone_point(
            &world.resource::<RuntimeConfigResource>().config,
            world.resource::<MapRuntimeResource>(),
            &point,
        ));
        // The location is prepared through the trusted transform API. Normal
        // UseItem and later accepted Zone movement remain the real producers.
        session.force_authoritative_player_transform(point, MirDirection::Left);
    }

    #[test]
    fn source_real_exp_potion_safe_use_accepted_walk_resume_and_native_death() {
        use super::super::zone::{
            SessionId, ZoneCommand, ZoneManager, ZoneMonsterKillAward, ZoneMonsterSpawn,
            ZoneOutbound,
        };
        use mir2_protocol::{MirDirection, Point, Spell};

        fn native_award(
            manager: &mut ZoneManager,
            sid: &SessionId,
            object_id: u32,
            monster_level: u16,
            now_ms: u64,
        ) -> ZoneMonsterKillAward {
            let (position, _) = manager.player_transform(sid).unwrap();
            manager.handle(ZoneCommand::SpawnMonster {
                session_id: sid.clone(),
                now_ms,
                monster: ZoneMonsterSpawn {
                    crystal_drop_seed: None,
                    object_id,
                    name: "Scarecrow".into(),
                    name_colour_argb: -1,
                    image: 0,
                    ai: 0,
                    disposition: Some(crate::WorldEntityDisposition::Hostile),
                    level: monster_level,
                    max_hp: 1,
                    hp: 1,
                    experience: 1000,
                    move_speed_ms: 60_000,
                    attack_speed_ms: 60_000,
                    friendly_guild: None,
                    position: Point {
                        x: position.x + 1,
                        y: position.y,
                    },
                    direction: MirDirection::Left,
                    defense: Default::default(),
                    respawn: None,
                    drops: Vec::new(),
                },
            });
            let mut output = manager.handle(ZoneCommand::PlayerAttackObject {
                session_id: sid.clone(),
                object_id,
                direction: MirDirection::Right,
                spell: Spell::None as u8,
                level: 0,
                attack_type: 0,
                damage: 1,
                now_ms,
            });
            output.extend(manager.tick_all(now_ms));
            let (source, recipient, owner, award) = output
                .into_iter()
                .find_map(|out| match out {
                    ZoneOutbound::OwnedMonsterKillAward {
                        source,
                        session_id,
                        online_owner,
                        award,
                    } if award.monster_object_id == object_id => {
                        Some((source, session_id, online_owner, award))
                    }
                    _ => None,
                })
                .expect("an admitted real native death produces its source-issued selection");
            assert!(manager.issued_monster_award_is_current(&source, &recipient, &owner, &award));
            award
        }

        let mut session = session();
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .retain(|buff| buff.key != "exp");
        // Normal UseItem at the actual initial safe tile produces a paused
        // packet and buff immediately. Only item acquisition is prepared.
        assert!(session.world_snapshot().in_safe_zone);
        let used = normal_use_prepared_exp_potion(&mut session, 1301, 88021301);
        assert!(used.iter().any(|packet| matches!(packet,
            ServerPacket::AddBuff { buff } if buff.buff_type == 102 && buff.paused
        )));
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1000
        );
        prepare_outside_imported_safe_zone(&mut session);
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1300
        );
        let initial = session
            .active_zone_experience_profile_at(1000)
            .unwrap()
            .unwrap();
        let level = initial.level;
        let mut manager = ZoneManager::new();
        let sid = SessionId::new("real-exp-safe-potion");
        let join = session.active_zone_join_snapshot(sid.as_str()).unwrap();
        let class = join.class;
        manager.handle(ZoneCommand::Join(join));
        manager.handle(ZoneCommand::sync_player_combat_state(
            sid.clone(),
            class,
            false,
            false,
            false,
            false,
            false,
            false,
        ));
        assert!(manager.update_experience_profile(&sid, Some(initial.clone())));

        let mut walked = manager.handle(ZoneCommand::Walk {
            session_id: sid.clone(),
            direction: MirDirection::Left,
            seq: 1,
            now_ms: 1100,
        });
        walked.extend(manager.tick_all(1100));
        assert!(walked.iter().any(|out| matches!(out,
            ZoneOutbound::ToSession { session_id, packets } if session_id == &sid
                && packets.iter().any(|packet| matches!(packet, ServerPacket::PauseBuff { buff_type: 102, paused: true, .. }))
        )));
        let (position, direction) = manager.player_transform(&sid).unwrap();
        assert_eq!(position, Point { x: 338, y: 270 });
        assert!(manager.update_experience_profile(&sid, Some(initial)));
        let paused = manager.player_experience_profile(&sid).unwrap();
        let row = paused
            .rate_source
            .as_ref()
            .unwrap()
            .buffs
            .iter()
            .find(|row| row.source_buff_key.as_deref() == Some("exp"))
            .unwrap();
        let frozen_remaining = row.paused_remaining_ms.unwrap();
        let instance = row.source_buff_instance;
        assert_eq!(row.expires_at_ms, None);
        session.force_authoritative_player_transform(position, direction);
        session
            .sync_shared_experience_buff_clocks(Some(&paused), 1100)
            .unwrap();
        let actual = session
            .active_zone_experience_profile_at(6_000_000)
            .unwrap()
            .unwrap();
        let actual_row = actual
            .rate_source
            .as_ref()
            .unwrap()
            .buffs
            .iter()
            .find(|row| row.source_buff_key.as_deref() == Some("exp"))
            .unwrap();
        assert_eq!(actual_row.paused_remaining_ms, Some(frozen_remaining));
        assert_eq!(actual_row.source_buff_instance, instance);
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1000
        );
        assert_eq!(
            // The normal walk also owns the 600ms action cadence; the swing
            // must wait for it even though the pause transition is immediate.
            native_award(&mut manager, &sid, 8800, level, 1800)
                .experience_selection
                .unwrap()
                .final_amount,
            1000
        );

        let mut walked = manager.handle(ZoneCommand::Walk {
            session_id: sid.clone(),
            direction: MirDirection::Right,
            seq: 2,
            now_ms: 6100,
        });
        walked.extend(manager.tick_all(6100));
        assert!(walked.iter().any(|out| matches!(out,
            ZoneOutbound::ToSession { session_id, packets } if session_id == &sid
                && packets.iter().any(|packet| matches!(packet, ServerPacket::PauseBuff { buff_type: 102, paused: false, .. }))
        )));
        let (position, direction) = manager.player_transform(&sid).unwrap();
        assert_eq!(position, Point { x: 339, y: 270 });
        assert!(manager.update_experience_profile(&sid, Some(paused)));
        let resumed = manager.player_experience_profile(&sid).unwrap();
        let row = resumed
            .rate_source
            .as_ref()
            .unwrap()
            .buffs
            .iter()
            .find(|row| row.source_buff_key.as_deref() == Some("exp"))
            .unwrap();
        let expiry = row.expires_at_ms.unwrap();
        assert_eq!(expiry, 6100 + frozen_remaining);
        assert_eq!(row.paused_remaining_ms, None);
        session.force_authoritative_player_transform(position, direction);
        session
            .sync_shared_experience_buff_clocks(Some(&resumed), 6100)
            .unwrap();
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1300
        );
        assert_eq!(
            native_award(&mut manager, &sid, 8801, level, 7000)
                .experience_selection
                .unwrap()
                .final_amount,
            1300
        );
        assert_eq!(
            native_award(&mut manager, &sid, 8802, level, expiry)
                .experience_selection
                .unwrap()
                .final_amount,
            1000
        );
        session
            .sync_shared_experience_buff_clocks(
                manager.player_experience_profile(&sid).as_ref(),
                expiry,
            )
            .unwrap();
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1000
        );
    }

    #[test]
    fn source_safe_clock_sync_freezes_actual_exp_and_rejects_old_stacked_instances() {
        let mut session = session();
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .retain(|buff| buff.key != "exp");
        normal_use_prepared_exp_potion(&mut session, 1301, 88011301);
        let mut paused = session
            .active_zone_experience_profile_at(1000)
            .unwrap()
            .unwrap();
        let row = paused
            .rate_source
            .as_mut()
            .unwrap()
            .buffs
            .iter_mut()
            .find(|row| row.source_buff_key.as_deref() == Some("exp"))
            .unwrap();
        let first_instance = row.source_buff_instance;
        assert_ne!(first_instance, 0);
        row.expires_at_ms = None;
        row.paused_remaining_ms = Some(60_000);
        session
            .sync_shared_experience_buff_clocks(Some(&paused), 1100)
            .unwrap();
        let frozen = session
            .active_zone_experience_profile_at(1_000_000)
            .unwrap()
            .unwrap();
        let row = frozen
            .rate_source
            .as_ref()
            .unwrap()
            .buffs
            .iter()
            .find(|row| row.source_buff_key.as_deref() == Some("exp"))
            .unwrap();
        assert_eq!(row.paused_remaining_ms, Some(60_000));
        assert_eq!(row.expires_at_ms, None);
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1000
        );
        let mut resumed = frozen.clone();
        let row = resumed
            .rate_source
            .as_mut()
            .unwrap()
            .buffs
            .iter_mut()
            .find(|row| row.source_buff_key.as_deref() == Some("exp"))
            .unwrap();
        row.paused_remaining_ms = None;
        row.expires_at_ms = Some(1_030_000);
        session
            .sync_shared_experience_buff_clocks(Some(&resumed), 1_000_000)
            .unwrap();
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1300
        );
        normal_use_prepared_exp_potion(&mut session, 1305, 88011305);
        let duration = session
            .app
            .world()
            .resource::<BuffResource>()
            .buffs
            .iter()
            .find(|buff| buff.key == "exp")
            .unwrap()
            .real_time_duration
            .as_ref()
            .unwrap();
        let second_instance = duration.instance_id();
        let running_before = duration.remaining_ms();
        let paused_before = duration.is_paused();
        assert_ne!(first_instance, second_instance);
        session
            .sync_shared_experience_buff_clocks(Some(&paused), 1_000_000)
            .unwrap();
        let duration = session
            .app
            .world()
            .resource::<BuffResource>()
            .buffs
            .iter()
            .find(|buff| buff.key == "exp")
            .unwrap()
            .real_time_duration
            .as_ref()
            .unwrap();
        assert_eq!(duration.instance_id(), second_instance);
        assert_eq!(duration.is_paused(), paused_before);
        assert!(duration.remaining_ms() <= running_before);
        assert!(duration.remaining_ms() > running_before.saturating_sub(1000));
        let mut wrong_actor = resumed;
        wrong_actor.account_id = "different-owner".into();
        assert!(session
            .sync_shared_experience_buff_clocks(Some(&wrong_actor), 1_000_000)
            .is_err());
    }

    #[test]
    fn source_safe_clock_upgrades_only_actual_finite_legacy_exp_drop_durations() {
        let mut session = session();
        let tick = runtime_tick(session.app.world());
        {
            let mut buffs = session.app.world_mut().resource_mut::<BuffResource>();
            let exp = buffs
                .buffs
                .iter_mut()
                .find(|buff| buff.key == "exp")
                .unwrap();
            exp.expires_at_tick = tick + 60;
            assert!(exp.real_time_duration.is_none());
        }
        session
            .sync_shared_experience_buff_clocks(None, 1000)
            .unwrap();
        let profile = session
            .active_zone_experience_profile_at(1000)
            .unwrap()
            .unwrap();
        let row = profile
            .rate_source
            .as_ref()
            .unwrap()
            .buffs
            .iter()
            .find(|row| row.source_buff_key.as_deref() == Some("exp"))
            .unwrap();
        assert!(row.pause_in_safe_zone);
        assert_ne!(row.source_buff_instance, 0);
        assert!((60_000..=61_000).contains(&row.expires_at_ms.unwrap()));
        assert_eq!(row.paused_remaining_ms, None);
    }

    fn source_npc_action(session: &mut SimulationSession, line: &str) {
        let mut packets = Vec::new();
        let mut state = super::super::npc_script::CrystalNpcExecutionState::default();
        assert_eq!(
            super::super::npc_script::execute_crystal_npc_action_line(
                session.app.world_mut(),
                line,
                &mut packets,
                &mut state,
            ),
            super::super::npc_script::CrystalNpcActionControl::Continue,
        );
    }

    #[test]
    fn source_npc_can_gain_exp_action_controls_personal_and_captured_recipient_only() {
        let mut session = session();
        source_npc_action(&mut session, "CANGAINEXP False");
        assert!(!super::super::default_npc_events::can_gain_experience(
            session.app.world()
        ));
        assert!(
            !session
                .active_zone_experience_profile()
                .unwrap()
                .unwrap()
                .can_gain_experience
        );
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            0
        );
        let before = session.active_character_checkpoint().unwrap().experience;
        assert!(
            super::super::leveling::apply_experience_gain(session.app.world_mut(), 1000).is_empty()
        );
        assert_eq!(
            session.active_character_checkpoint().unwrap().experience,
            before
        );
        source_npc_action(&mut session, "CANGAINEXP TRUE");
        assert!(
            session
                .active_zone_experience_profile()
                .unwrap()
                .unwrap()
                .can_gain_experience
        );
        source_npc_action(&mut session, "CANGAINEXP");
        assert!(
            session
                .active_zone_experience_profile()
                .unwrap()
                .unwrap()
                .can_gain_experience
        );
        source_npc_action(&mut session, "CANGAINEXP 1");
        assert!(
            !session
                .active_zone_experience_profile()
                .unwrap()
                .unwrap()
                .can_gain_experience
        );
    }

    #[test]
    fn source_issued_native_selection_survives_later_npc_can_gain_exp_false() {
        use super::super::zone::{
            SessionId, ZoneCommand, ZoneManager, ZoneMonsterSpawn, ZoneOutbound,
        };
        use mir2_protocol::{MirDirection, Point, Spell};
        let mut session = session();
        prepare_outside_imported_safe_zone(&mut session);
        source_npc_action(&mut session, "CANGAINEXP True");
        let mut manager = ZoneManager::new();
        let sid = SessionId::new("source-later-npc-gate");
        let mut join = session.active_zone_join_snapshot(sid.as_str()).unwrap();
        join.map_file_name = "xp-test".into();
        join.position = Point { x: 330, y: 270 };
        let class = join.class;
        manager.handle(ZoneCommand::Join(join));
        manager.handle(ZoneCommand::sync_player_combat_state(
            sid.clone(),
            class,
            false,
            false,
            false,
            false,
            false,
            false,
        ));
        assert!(manager
            .update_experience_profile(&sid, session.active_zone_experience_profile().unwrap()));
        manager.handle(ZoneCommand::SpawnMonster {
            session_id: sid.clone(),
            now_ms: 0,
            monster: ZoneMonsterSpawn {
                crystal_drop_seed: None,
                object_id: 8700,
                name: "Scarecrow".into(),
                name_colour_argb: -1,
                image: 0,
                ai: 0,
                disposition: Some(crate::WorldEntityDisposition::Hostile),
                level: 20,
                max_hp: 1,
                hp: 1,
                experience: 1000,
                move_speed_ms: 60_000,
                attack_speed_ms: 60_000,
                friendly_guild: None,
                position: Point { x: 331, y: 270 },
                direction: MirDirection::Left,
                defense: Default::default(),
                respawn: None,
                drops: Vec::new(),
            },
        });
        manager.handle(ZoneCommand::PlayerAttackObject {
            session_id: sid.clone(),
            object_id: 8700,
            direction: MirDirection::Right,
            spell: Spell::None as u8,
            level: 0,
            attack_type: 0,
            damage: 1,
            now_ms: 100,
        });
        let (source, recipient, owner, award) = manager
            .tick_all(100)
            .into_iter()
            .find_map(|out| match out {
                ZoneOutbound::OwnedMonsterKillAward {
                    source,
                    session_id,
                    online_owner,
                    award,
                } => Some((source, session_id, online_owner, award)),
                _ => None,
            })
            .expect("real native death produces a source-issued selection");
        assert!(manager.issued_monster_award_is_current(&source, &recipient, &owner, &award));
        let expected = award.experience_selection.as_ref().unwrap().final_amount;
        assert_eq!(expected, 1200);
        source_npc_action(&mut session, "CANGAINEXP False");
        let (receipt, replay) = session
            .try_commit_shared_monster_kill_award_with_receipt("source-gate/8700/100", &award)
            .unwrap();
        assert!(receipt.committed && !replay);
        assert!(receipt.packets.iter().any(|packet| matches!(packet,ServerPacket::GainExperience { amount } if *amount==expected)),
            "a later transient flag must not suppress the already-authenticated final amount");
        assert!(!super::super::default_npc_events::can_gain_experience(
            session.app.world()
        ));
        let (receipt, replay) = session
            .try_commit_shared_monster_kill_award_with_receipt("source-gate/8700/100", &award)
            .unwrap();
        assert!(receipt.committed && replay && receipt.packets.is_empty());
    }

    #[test]
    fn source_normal_exp_potion_maps_item_luck_to_exp_rate_and_stacks_real_remaining_time() {
        let mut session = session();
        prepare_outside_imported_safe_zone(&mut session);
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .retain(|buff| buff.key != "exp");
        normal_use_prepared_exp_potion(&mut session, 1301, 88001301);
        let remaining = |session: &SimulationSession| {
            session
                .app
                .world()
                .resource::<BuffResource>()
                .buffs
                .iter()
                .find(|buff| buff.key == "exp")
                .unwrap()
                .real_time_duration
                .as_ref()
                .expect("normal EXP potion uses source elapsed time")
                .remaining_ms()
        };
        let exp = session
            .app
            .world()
            .resource::<BuffResource>()
            .buffs
            .iter()
            .find(|buff| buff.key == "exp")
            .unwrap();
        assert_eq!(
            exp.stats,
            vec![UserItemStat {
                stat: 100,
                value: 30
            }]
        );
        assert!((3_595_000..=3_600_000).contains(&remaining(&session)));
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1300
        );
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .iter_mut()
            .find(|buff| buff.key == "exp")
            .unwrap()
            .real_time_duration
            .as_mut()
            .unwrap()
            .elapse_for_test(30_000);
        let before_stack = remaining(&session);
        // Crystal Exp is StackDuration: the second potion adds its time but
        // does not replace the first potion's 30% with its own 50% value.
        normal_use_prepared_exp_potion(&mut session, 1305, 88001305);
        let after_stack = remaining(&session);
        assert!(after_stack <= before_stack + 3_600_000);
        assert!(after_stack >= before_stack + 3_595_000);
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1300
        );
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .iter_mut()
            .find(|buff| buff.key == "exp")
            .unwrap()
            .real_time_duration
            .as_mut()
            .unwrap()
            .elapse_for_test(7_200_000);
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1000
        );
        normal_use_prepared_exp_potion(&mut session, 1305, 88001306);
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1500
        );
        assert!((3_595_000..=3_600_000).contains(&remaining(&session)));
    }

    #[test]
    fn source_drop_template_branch_maps_rate_stacks_duration_and_expires_without_changing_exp() {
        let mut session = session();
        // The imported 117 database contains no Potion shape5 rows. Exercise
        // the original server template branch explicitly; this is not evidence
        // that a normal player can acquire or use a shipped DROP potion.
        assert!(!mir2_game_data::crystal_item_manifest_ref()
            .items
            .iter()
            .any(|item| item.item_type == 13 && item.shape == 5));
        let mut configured_template = mir2_game_data::crystal_item_by_index(1301).unwrap();
        configured_template.shape = 5;
        configured_template.name = "Prepared Drop Template".into();
        let tick = runtime_tick(session.app.world());
        let configured_buff =
            super::super::buffs::crystal_template_consumable_buffs(&configured_template, tick)
                .into_iter()
                .next()
                .unwrap();
        assert_eq!(
            configured_buff.stats,
            vec![UserItemStat {
                stat: 101,
                value: 30
            }]
        );
        assert!(configured_buff.real_time_duration.is_some());
        let original_luck = super::super::stats::player_stats(session.app.world()).get(15);
        super::super::buffs::apply_or_stack_duration_buff(session.app.world_mut(), configured_buff);
        let remaining = |session: &SimulationSession| {
            session
                .app
                .world()
                .resource::<BuffResource>()
                .buffs
                .iter()
                .find(|buff| buff.key == "drop")
                .unwrap()
                .real_time_duration
                .as_ref()
                .unwrap()
                .remaining_ms()
        };
        assert!((3_595_000..=3_600_000).contains(&remaining(&session)));
        assert_eq!(
            super::super::stats::player_stats(session.app.world()).get(101),
            30
        );
        assert_eq!(
            super::super::stats::player_stats(session.app.world()).get(15),
            original_luck
        );
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1200
        );
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .iter_mut()
            .find(|buff| buff.key == "drop")
            .unwrap()
            .real_time_duration
            .as_mut()
            .unwrap()
            .elapse_for_test(30_000);
        let before_stack = remaining(&session);
        configured_template
            .stats
            .iter_mut()
            .find(|stat| stat.stat == 15)
            .unwrap()
            .value = 50;
        let configured_buff =
            super::super::buffs::crystal_template_consumable_buffs(&configured_template, tick)
                .into_iter()
                .next()
                .unwrap();
        super::super::buffs::apply_or_stack_duration_buff(session.app.world_mut(), configured_buff);
        let after_stack = remaining(&session);
        assert!(after_stack <= before_stack + 3_600_000);
        assert!(after_stack >= before_stack + 3_595_000);
        assert_eq!(
            super::super::stats::player_stats(session.app.world()).get(101),
            30
        );
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .iter_mut()
            .find(|buff| buff.key == "drop")
            .unwrap()
            .real_time_duration
            .as_mut()
            .unwrap()
            .elapse_for_test(7_200_000);
        assert_eq!(
            super::super::stats::player_stats(session.app.world()).get(101),
            0
        );
        let configured_buff =
            super::super::buffs::crystal_template_consumable_buffs(&configured_template, tick)
                .into_iter()
                .next()
                .unwrap();
        super::super::buffs::apply_or_stack_duration_buff(session.app.world_mut(), configured_buff);
        assert_eq!(
            super::super::stats::player_stats(session.app.world()).get(101),
            50
        );
    }

    #[test]
    fn source_idle_tick_buff_projection_does_not_extend_expiry_and_renewal_gets_new_anchor() {
        let mut session = session();
        let tick = runtime_tick(session.app.world());
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .iter_mut()
            .find(|buff| buff.key == "exp")
            .unwrap()
            .expires_at_tick = tick + 10;
        let first = session
            .active_zone_experience_profile_at(1000)
            .unwrap()
            .unwrap();
        let expiry = |profile: &ZoneExperienceProfile| {
            profile
                .rate_source
                .as_ref()
                .unwrap()
                .buffs
                .iter()
                .find(|buff| buff.stats.iter().any(|entry| entry.stat == 100))
                .unwrap()
                .expires_at_ms
        };
        assert_eq!(expiry(&first), Some(11_000));
        let still_live = session
            .active_zone_experience_profile_at(10_999)
            .unwrap()
            .unwrap();
        assert_eq!(expiry(&still_live), Some(11_000));
        assert_eq!(still_live.experience_rate_percent, 20);
        let expired = session
            .active_zone_experience_profile_at(11_000)
            .unwrap()
            .unwrap();
        assert_eq!(expiry(&expired), Some(11_000));
        assert_eq!(expired.experience_rate_percent, 0);
        assert_eq!(expired.lover_rate_percent, 5);
        assert_eq!(expired.mentee_rate_percent, 10);
        let expired_again = session
            .active_zone_experience_profile_at(20_000)
            .unwrap()
            .unwrap();
        assert_eq!(expired_again.experience_rate_percent, 0);
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .iter_mut()
            .find(|buff| buff.key == "exp")
            .unwrap()
            .expires_at_tick = tick + 20;
        let renewed = session
            .active_zone_experience_profile_at(20_001)
            .unwrap()
            .unwrap();
        assert_eq!(expiry(&renewed), Some(40_001));
        assert_eq!(renewed.experience_rate_percent, 20);
        assert_eq!(
            session
                .app
                .world()
                .resource::<SharedExperienceRateClock>()
                .0
                .lock()
                .unwrap()
                .anchors
                .len(),
            1,
            "replaced buffers do not retain an unbounded anchor history"
        );
    }

    #[test]
    fn source_personal_gain_rechecks_expired_actual_buffs_but_keeps_equipment_rates() {
        let mut session = session();
        {
            let mut inventory = session
                .app
                .world_mut()
                .resource_mut::<super::super::resources::InventoryResource>();
            let equipment = inventory.equipment_items.first_mut().unwrap();
            equipment.added_stats.extend([
                UserItemStat {
                    stat: 100,
                    value: 7,
                },
                UserItemStat {
                    stat: 120,
                    value: 1,
                },
                UserItemStat {
                    stat: 123,
                    value: 2,
                },
            ]);
        }
        assert_eq!(
            super::super::stats::player_experience_base_rates(session.app.world()),
            [7, 1, 2]
        );
        for buff in &mut session.app.world_mut().resource_mut::<BuffResource>().buffs {
            if matches!(buff.key.as_str(), "exp" | "lover" | "mentee") {
                buff.real_time_duration =
                    Some(super::super::buffs::RealTimeBuffDuration::new(60_000));
            }
        }
        let source = session.active_zone_experience_profile().unwrap().unwrap();
        let zone = ZoneKey::for_map(
            session
                .app
                .world()
                .resource::<MapRuntimeResource>()
                .current_map
                .file_name
                .clone(),
        );
        session
            .set_shared_personal_experience_context(Some(source), Some(zone))
            .unwrap();
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1507
        );
        for buff in &mut session.app.world_mut().resource_mut::<BuffResource>().buffs {
            if let Some(duration) = &mut buff.real_time_duration {
                duration.elapse_for_test(60_000);
            }
        }
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1070,
            "stale eligible context cannot revive buffs; equipped social rates require an actual buff"
        );
        let source = session.active_zone_experience_profile().unwrap().unwrap();
        assert_eq!(
            source.gain_rates_at(experience_now_ms().unwrap()).unwrap(),
            [7, 0, 0]
        );
    }

    #[test]
    fn source_personal_gain_context_requires_current_epoch_map_and_actual_buff() {
        let mut session = session();
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1200,
            "absent context must not use social display names"
        );
        let mut source = session.active_zone_experience_profile().unwrap().unwrap();
        let zone = ZoneKey::for_map(
            session
                .app
                .world()
                .resource::<MapRuntimeResource>()
                .current_map
                .file_name
                .clone(),
        );
        assert!(source.lover_eligible && source.mentee_eligible);
        // This prepared server context tests application rechecks. Peer range,
        // life and group admission are covered by real Zone capture tests.
        source.lover_rate_percent = 9999;
        source.mentee_rate_percent = 9999;
        source.experience_rate_percent = 9999;
        session
            .set_shared_personal_experience_context(Some(source.clone()), Some(zone.clone()))
            .unwrap();
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1386,
            "apply reads current own stats rather than stale context rates"
        );
        let mut changed = source.clone();
        changed.mentee_partner.as_mut().unwrap().relationship_epoch += 1;
        session
            .set_shared_personal_experience_context(Some(changed), Some(zone.clone()))
            .unwrap();
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1260
        );
        let mut changed = source.clone();
        changed.lover_partner.as_mut().unwrap().relationship_epoch += 1;
        session
            .set_shared_personal_experience_context(Some(changed), Some(zone.clone()))
            .unwrap();
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1320
        );
        session
            .set_shared_personal_experience_context(
                Some(source.clone()),
                Some(ZoneKey::for_map("other-source-map")),
            )
            .unwrap();
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1200
        );
        session
            .set_shared_personal_experience_context(Some(source), Some(zone))
            .unwrap();
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .retain(|buff| buff.key != "mentee");
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1260
        );
        session
            .app
            .world_mut()
            .resource_mut::<BuffResource>()
            .buffs
            .retain(|buff| buff.key != "lover");
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1200
        );
    }

    #[test]
    fn normal_quest_world_rate_precedes_current_gain_exp_and_obeys_map_gate() {
        let mut session = session();
        session
            .app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .crystal_world_experience_rate = 1.3;
        assert_eq!(
            crystal_apply_quest_experience_source(session.app.world(), 11),
            16
        );
        let source = session.active_zone_experience_profile().unwrap().unwrap();
        let map = session
            .app
            .world()
            .resource::<MapRuntimeResource>()
            .current_map
            .file_name
            .clone();
        session
            .set_shared_personal_experience_context(Some(source), Some(ZoneKey::for_map(&map)))
            .unwrap();
        // 1000 * f32 1.3 = 1300, then 5%, 10%, 20%, each truncate.
        assert_eq!(
            crystal_apply_quest_experience_source(session.app.world(), 1000),
            1801
        );
        // Direct NPC/mentor GainExp has no world or monster multiplier.
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            1386
        );
        // The real manifest currently has no positive NoExperience flags.
        // Exercise a future trusted rule override; default config intentionally
        // reads the manifest and does not materialize a map_drop_rules vector.
        let rule = crate::config::MapDropRuleRecord {
            map_file_name: map,
            no_experience: true,
            no_town_teleport: false,
            no_escape: false,
            no_random: false,
            no_drug: false,
            no_reincarnation: false,
            no_throw_item: false,
            no_drop_player: false,
            no_drop_monster: false,
            no_mount: false,
            no_hero: false,
            no_intelligent_creatures: false,
            need_bridle: false,
        };
        session
            .app
            .world_mut()
            .resource_mut::<RuntimeConfigResource>()
            .config
            .map_drop_rules
            .push(rule);
        assert_eq!(
            crystal_apply_quest_experience_source(session.app.world(), 1000),
            0
        );
        assert_eq!(
            apply_current_personal_experience_rates(session.app.world(), 1000),
            0
        );
        assert!(
            session
                .active_zone_experience_profile()
                .unwrap()
                .unwrap()
                .no_experience
        );
    }

    #[test]
    fn locked_source_authority_keeps_legacy_profile_absent_and_rejects_wrong_identity() {
        let session = session();
        let config = session.shared_mentor_config().unwrap();
        let active = session.active_identity().unwrap();
        let actor = Stage5FriendIdentity {
            account_id: active.account_id,
            character_index: active.character_index,
        };
        let mut absent = None;
        let mut chat = ZoneChatProfile::default();
        config
            .with_shared_zone_experience_admission(|rebase| {
                rebase(&actor, &mut absent, &mut chat)?;
                assert!(absent.is_none(), "legacy rates must not be manufactured");
                Ok(())
            })
            .unwrap();
        let mut wrong = session.active_zone_experience_profile().unwrap();
        wrong.as_mut().unwrap().character_index += 1;
        assert!(config
            .with_shared_zone_experience_admission(|rebase| rebase(&actor, &mut wrong, &mut chat))
            .is_err());
    }

    #[test]
    fn source_no_guild_selection_still_validates_the_actual_gain_amount() {
        let mut session = session();
        let checkpoint = session.begin_guild_experience_command(true).unwrap();
        assert!(checkpoint.is_some());
        let active = session.active_identity().unwrap();
        let identity = Stage5FriendIdentity {
            account_id: active.account_id.clone(),
            character_index: active.character_index,
        };
        let selection = super::super::zone::ZoneExperienceSelection {
            source_zone: "source-zone".into(),
            monster_object_id: 7,
            killed_at_ms: 9,
            account_id: active.account_id,
            character_index: active.character_index,
            guild: None,
            final_amount: 100,
            guild_amount: 0,
        };
        let permit = crate::config::guild_experience::GuildExperienceCommitPermit::from_verified_shared_kill(identity, "source-no-guild".into(), "a".repeat(64), Some(selection)).unwrap();
        session.app.world_mut().insert_resource(
            super::super::shared_guild_experience::SharedKillExperiencePermit(permit, None),
        );
        assert!(
            super::super::shared_guild_experience::record_gain(session.app.world_mut(), 99)
                .unwrap_err()
                .contains("differs from immutable selection")
        );
        assert!(
            super::super::shared_guild_experience::record_gain(session.app.world_mut(), 0)
                .unwrap_err()
                .contains("differs from immutable selection")
        );
        assert!(
            super::super::shared_guild_experience::record_gain(session.app.world_mut(), 100)
                .is_ok()
        );
        assert!(session
            .active_character_checkpoint()
            .unwrap()
            .guild_experience_journal
            .pending
            .is_empty());
    }
}
