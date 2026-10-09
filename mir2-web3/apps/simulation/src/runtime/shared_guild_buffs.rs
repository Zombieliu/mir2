use super::*;
use crate::config::SharedGuildBuff;
use mir2_protocol::{GuildBuff, UserItemStat};

// Domain step only. The server clock/lease driver owns when a minute is admitted;
// session ticks must never call this independently for each connected member.
pub(crate) fn advance_minutes(
    guild: &mut SharedGuildRecord,
    minutes: u64,
) -> Vec<GuildBuff> {
    advance_minutes_with_definitions(
        guild,
        minutes,
        mir2_game_data::crystal_guild_buff_definitions(),
    )
}
fn advance_minutes_with_definitions(
    guild: &mut SharedGuildRecord,
    minutes: u64,
    definitions: &[mir2_protocol::GuildBuffInfo],
) -> Vec<GuildBuff> {
    let mut expired = Vec::new();
    if minutes == 0 {
        return expired;
    }
    for buff in guild.buffs.values_mut() {
        let Some(definition) = definitions
            .iter()
            .find(|definition| definition.id == buff.id)
        else {
            continue;
        };
        if !buff.active || definition.time_limit == 0 {
            continue;
        }
        let remaining = i64::from(buff.remaining_minutes)
            .saturating_sub(i64::try_from(minutes).unwrap_or(i64::MAX));
        buff.remaining_minutes = i32::try_from(remaining.max(-1)).unwrap_or(i32::MAX);
        if buff.remaining_minutes < 0 {
            buff.active = false;
            expired.push(wire_buff(buff));
        }
    }
    expired
}

fn wire_buff(buff: &SharedGuildBuff) -> GuildBuff {
    GuildBuff {
        id: buff.id,
        active: buff.active,
        active_time_remaining: buff.remaining_minutes,
    }
}
pub(in crate::runtime) fn active_stats(world: &World) -> Vec<UserItemStat> {
    if !enabled(world) {
        return Vec::new();
    }
    let Some(identity) = world_identity(world) else {
        return Vec::new();
    };
    let config = &world.resource::<RuntimeConfigResource>().config;
    config
        .shared_guild_active_stats(&identity)
        .unwrap_or_default()
}
pub(super) fn active_packet(world: &World) -> ServerPacket {
    let buffs = guild_view(world)
        .map(|guild| guild.buffs.values().map(wire_buff).collect())
        .unwrap_or_default();
    ServerPacket::GuildBuffList {
        remove: 0,
        guild_buffs: Vec::new(),
        active_buffs: buffs,
    }
}
impl SimulationConfig {
    fn commit_shared_guild_buff(
        &self,
        identity: &Stage5FriendIdentity,
        action: u8,
        id: i32,
    ) -> Result<Vec<ServerPacket>, String> {
        if !(1..=2).contains(&action) {
            return Err("invalid guild buff action".into());
        }
        let definition = mir2_game_data::crystal_guild_buff_definitions()
            .iter()
            .find(|definition| definition.id == id)
            .ok_or("guild buff not found")?;
        self.commit_shared_guild_buff_definition(identity, action, definition)
    }
    fn commit_shared_guild_buff_definition(
        &self,
        identity: &Stage5FriendIdentity,
        action: u8,
        definition: &mir2_protocol::GuildBuffInfo,
    ) -> Result<Vec<ServerPacket>, String> {
        if !(1..=2).contains(&action) {
            return Err("invalid guild buff action".into());
        }
        let id = definition.id;
        if definition.activation_cost < 0 || definition.time_limit < 0 {
            return Err("invalid guild buff definition".into());
        }
        let guild_id = self
            .shared_guild_for_identity(identity)?
            .ok_or("guild membership missing")?
            .id;
        self.commit_account_store_transaction_with_guilds(
            &[],
            std::slice::from_ref(&guild_id),
            |store| {
                let guild = store
                    .shared_guilds
                    .get_mut(&guild_id)
                    .ok_or("guild missing")?;
                let member = guild.member(identity).ok_or("guild membership changed")?;
                if !guild
                    .ranks
                    .iter()
                    .any(|rank| rank.index == member.rank_index && rank.options & 128 != 0)
                {
                    return Err("guild buff permission denied".into());
                }
                let cost = if action == 1 {
                    if guild.buffs.contains_key(&id) {
                        return Err("guild buff already obtained".into());
                    }
                    if guild.level < definition.level_requirement {
                        return Err("guild level insufficient".into());
                    }
                    guild.spare_points = guild
                        .spare_points
                        .checked_sub(definition.points_requirement)
                        .ok_or("guild spare points insufficient")?;
                    if definition.time_limit > 0 {
                        definition.activation_cost as u32
                    } else {
                        0
                    }
                } else {
                    let existing = guild.buffs.get(&id).ok_or("guild buff not obtained")?;
                    if existing.active {
                        return Err("guild buff already active".into());
                    }
                    definition.activation_cost as u32
                };
                guild.gold = guild
                    .gold
                    .checked_sub(cost)
                    .ok_or("guild bank funds insufficient")?;
                let buff = SharedGuildBuff {
                    id,
                    active: true,
                    remaining_minutes: definition.time_limit,
                };
                guild.buffs.insert(id, buff.clone());
                guild.revision = guild
                    .revision
                    .checked_add(1)
                    .ok_or("guild revision exhausted")?;
                let mut packets = Vec::new();
                if cost > 0 {
                    packets.push(ServerPacket::GuildStorageGoldChange {
                        change_type: 2,
                        amount: cost,
                        name: String::new(),
                    });
                }
                packets.push(ServerPacket::GuildBuffList {
                    remove: 0,
                    guild_buffs: Vec::new(),
                    active_buffs: vec![wire_buff(&buff)],
                });
                Ok(packets)
            },
        )
    }
}
impl crate::SimulationSession {
    pub fn change_shared_guild_buff(
        &mut self,
        action: u8,
        id: i32,
    ) -> Result<Vec<ServerPacket>, String> {
        if !enabled(self.app.world())
            || !super::super::resources::is_in_world(self.app.world())
            || id < 0
        {
            return Err("guild buff requires active character".into());
        }
        let identity = world_identity(self.app.world()).ok_or("guild identity missing")?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        if action == 0 {
            config
                .shared_guild_for_identity(&identity)?
                .ok_or("guild membership missing")?;
            return Ok(vec![ServerPacket::GuildBuffList {
                remove: 0,
                guild_buffs: mir2_game_data::crystal_guild_buff_definitions().to_vec(),
                active_buffs: Vec::new(),
            }]);
        }
        let packets = config.commit_shared_guild_buff(&identity, action, id)?;
        super::super::stats::refresh_player_stats(self.app.world_mut());
        Ok(packets)
    }
}

#[cfg(test)]
#[path = "shared_guild_buff_tests.rs"]
mod tests;
