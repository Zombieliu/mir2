//! Ordinary member, rank and leave commands. GuildObject.cs is the rule authority.
use super::*;

fn command_guild(
    store: &AccountStore,
    guild_id: &str,
    actor: &Stage5FriendIdentity,
    permission: u8,
    denied_key: &str,
) -> Result<(SharedGuildRecord, u8), String> {
    if !store
        .accounts
        .get(&actor.account_id)
        .is_some_and(|account| {
            account
                .characters
                .iter()
                .any(|character| character.index == actor.character_index)
        })
    {
        return Err("Guild character no longer exists.".into());
    }
    let guild = store
        .shared_guilds
        .get(guild_id)
        .ok_or("Guild no longer exists.")?;
    let member = guild.member(actor).ok_or("server.NotInGuild")?;
    let rank = guild
        .ranks
        .iter()
        .find(|rank| rank.index == member.rank_index)
        .ok_or("Guild rank no longer exists.")?;
    if permission != 0 && rank.options & permission == 0 {
        return Err(denied_key.into());
    }
    Ok((guild.clone(), rank.index))
}

fn contiguous_ranks(guild: &SharedGuildRecord) -> bool {
    guild
        .ranks
        .iter()
        .enumerate()
        .all(|(index, rank)| usize::from(rank.index) == index)
}

fn next_revision(guild: &SharedGuildRecord) -> Result<u64, String> {
    guild
        .revision
        .checked_add(1)
        .ok_or_else(|| "Guild revision exhausted.".into())
}

impl SimulationConfig {
    pub fn commit_shared_guild_rank_options(
        &self,
        actor: &Stage5FriendIdentity,
        rank_index: u8,
        option: i32,
        enabled: &str,
    ) -> Result<SharedGuildRecord, String> {
        let guild_id = self
            .shared_guild_for_identity(actor)?
            .ok_or("server.NotInGuild")?
            .id;
        self.commit_account_store_transaction_with_guilds(
            std::slice::from_ref(&actor.account_id),
            std::slice::from_ref(&guild_id),
            |store| {
                let (mut guild, actor_rank) =
                    command_guild(store, &guild_id, actor, 1, "server.NotAllowedChangeRank")?;
                if !(0..=7).contains(&option)
                    || !guild.ranks.iter().any(|rank| rank.index == rank_index)
                {
                    return Err("server.RankNotFound".into());
                }
                if actor_rank >= rank_index {
                    return Err("server.CannotChangeOwnRankOptions".into());
                }
                if enabled != "true" && enabled != "false" {
                    return Err("Invalid Guild rank option value.".into());
                }
                let revision = next_revision(&guild)?;
                let rank = guild
                    .ranks
                    .iter_mut()
                    .find(|rank| rank.index == rank_index)
                    .unwrap();
                let bit = 1u8 << option;
                // Crystal deliberately uses XOR for false, including repeated requests.
                if enabled == "true" {
                    rank.options |= bit;
                } else {
                    rank.options ^= bit;
                }
                guild.revision = revision;
                store.shared_guilds.insert(guild_id.clone(), guild.clone());
                Ok(guild)
            },
        )
    }

    pub fn commit_shared_guild_new_rank(
        &self,
        actor: &Stage5FriendIdentity,
    ) -> Result<SharedGuildRecord, String> {
        let guild_id = self
            .shared_guild_for_identity(actor)?
            .ok_or("server.NotInGuild")?
            .id;
        self.commit_account_store_transaction_with_guilds(
            std::slice::from_ref(&actor.account_id),
            std::slice::from_ref(&guild_id),
            |store| {
                let (mut guild, _) =
                    command_guild(store, &guild_id, actor, 1, "server.NotAllowedChangeRank")?;
                if guild.ranks.len() >= 255 {
                    return Err("server.NoMoreRankSlotsAvailable".into());
                }
                if !contiguous_ranks(&guild) {
                    return Err("Guild ranks require contiguous indexes.".into());
                }
                let revision = next_revision(&guild)?;
                let old_count = guild.ranks.len();
                let index = if old_count > 1 { old_count - 1 } else { 1 };
                if old_count > 1 {
                    let old_lowest = (old_count - 1) as u8;
                    for member in &mut guild.members {
                        if member.rank_index == old_lowest {
                            member.rank_index = old_count as u8;
                        }
                    }
                }
                guild.ranks.insert(
                    index,
                    SharedGuildRank {
                        index: index as u8,
                        name: mir2_game_data::format_localized_text(
                            mir2_game_data::LanguageCode::English,
                            "server.RankNum",
                            [index.to_string()],
                        ),
                        options: 0,
                    },
                );
                guild.ranks.last_mut().unwrap().index = old_count as u8;
                guild.revision = revision;
                store.shared_guilds.insert(guild_id.clone(), guild.clone());
                Ok(guild)
            },
        )
    }

    /// `online` comes from authenticated shared presences while the social coordinator
    /// is held; client names or a private runtime snapshot cannot supply this witness.
    pub fn commit_shared_guild_member_rank(
        &self,
        actor: &Stage5FriendIdentity,
        target_name: &str,
        rank_index: u8,
        online: &BTreeSet<(String, i32)>,
    ) -> Result<(SharedGuildRecord, Stage5FriendIdentity), String> {
        let before = self
            .shared_guild_for_identity(actor)?
            .ok_or("server.NotInGuild")?;
        let expected_target = before
            .members
            .iter()
            .find(|member| member.name == target_name)
            .ok_or("Guild member not found.")?
            .identity
            .clone();
        let mut accounts = vec![actor.account_id.clone(), expected_target.account_id.clone()];
        accounts.sort();
        accounts.dedup();
        self.commit_account_store_transaction_with_guilds(
            &accounts,
            std::slice::from_ref(&before.id),
            |store| {
                let (mut guild, actor_rank) = command_guild(
                    store,
                    &before.id,
                    actor,
                    1,
                    "server.NotAllowedChangeOtherRank",
                )?;
                if !guild.ranks.iter().any(|rank| rank.index == rank_index) {
                    return Err("server.RankNotFound".into());
                }
                let target = guild
                    .members
                    .iter()
                    .find(|member| member.name == target_name)
                    .ok_or("Guild member no longer exists.")?;
                if target.identity != expected_target {
                    return Err("Guild target changed; retry the request.".into());
                }
                let target_character = store
                    .accounts
                    .get(&expected_target.account_id)
                    .and_then(|account| {
                        account
                            .characters
                            .iter()
                            .find(|character| character.index == expected_target.character_index)
                    })
                    .ok_or("Guild target character no longer exists.")?;
                if rank_index == 0
                    && target_character.level
                        < u16::from(mir2_game_data::crystal_guild_settings().minimum_level)
                {
                    return Err(format!(
                        "server.GuildLeaderMinLevel:{}",
                        mir2_game_data::crystal_guild_settings().minimum_level
                    ));
                }
                // Preserve GuildObject.ChangeRank's exact, unusual current-rank condition.
                // DeleteMember and ChangeRankOption use different inequalities.
                if actor_rank != 0 && target.rank_index >= actor_rank {
                    return Err("server.YourRankNotAdequate".into());
                }
                if target.rank_index == 0 {
                    let leaders: Vec<_> = guild
                        .members
                        .iter()
                        .filter(|member| member.rank_index == 0)
                        .collect();
                    if leaders.len() <= 2 {
                        return Err("server.GuildNeedsTwoLeaders".into());
                    }
                    if !leaders.iter().any(|member| {
                        member.identity != expected_target
                            && online.contains(&(
                                member.identity.account_id.clone(),
                                member.identity.character_index,
                            ))
                    }) {
                        return Err("server.NeedOneLeaderOnline".into());
                    }
                }
                let revision = next_revision(&guild)?;
                // Crystal appends the moved member to the destination rank, including
                // an otherwise unchanged move to the same rank.
                let target_position = guild
                    .members
                    .iter()
                    .position(|member| member.identity == expected_target)
                    .unwrap();
                let mut moved = guild.members.remove(target_position);
                moved.rank_index = rank_index;
                guild.members.push(moved);
                guild.revision = revision;
                store.shared_guilds.insert(before.id.clone(), guild.clone());
                Ok((guild, expected_target.clone()))
            },
        )
    }

    /// Self-leave is authenticated by identity, and does not require CanKick.
    /// Packet type 1 uses CanKick even when its supplied member is the actor.
    pub fn commit_shared_guild_remove_member(
        &self,
        actor: &Stage5FriendIdentity,
        target_name: &str,
        self_leave: bool,
    ) -> Result<(Option<SharedGuildRecord>, Stage5FriendIdentity, String), String> {
        let before = self
            .shared_guild_for_identity(actor)?
            .ok_or("server.NotInGuild")?;
        let expected_target = before
            .members
            .iter()
            .find(|member| member.name == target_name)
            .ok_or("Guild member not found.")?
            .identity
            .clone();
        let mut accounts = vec![actor.account_id.clone(), expected_target.account_id.clone()];
        accounts.sort();
        accounts.dedup();
        self.commit_account_store_transaction_with_guilds(
            &accounts,
            std::slice::from_ref(&before.id),
            |store| {
                let (mut guild, actor_rank) = command_guild(
                    store,
                    &before.id,
                    actor,
                    if self_leave { 0 } else { 4 },
                    "server.CannotRemoveMembers",
                )?;
                let target = guild
                    .members
                    .iter()
                    .find(|member| member.name == target_name)
                    .ok_or("Guild member no longer exists.")?;
                if target.identity != expected_target || (self_leave && expected_target != *actor) {
                    return Err("Guild target changed; retry the request.".into());
                }
                if self_leave && !guild.active_wars.is_empty() {
                    return Err("server.CannotLeaveGuildAtWar".into());
                }
                if actor_rank != 0 && actor_rank >= target.rank_index && expected_target != *actor {
                    return Err("server.YourRankNotAdequate".into());
                }
                let disband = target.rank_index == 0 && guild.members.len() == 1;
                if target.rank_index == 0
                    && !disband
                    && guild
                        .members
                        .iter()
                        .filter(|member| member.rank_index == 0)
                        .count()
                        <= 1
                {
                    return Err("server.YouNeedLastLeaderToDisbandGuild".into());
                }
                // A deletion cannot leave an opponent pointing at a missing stable Guild.
                // Chat leave already rejects wars; reject packet self-kick at this boundary too.
                if disband && !guild.active_wars.is_empty() {
                    return Err("server.CannotLeaveGuildAtWar".into());
                }
                let removed_name = target.name.clone();
                if disband {
                    store.shared_guilds.remove(&before.id);
                    Ok((None, expected_target.clone(), removed_name))
                } else {
                    guild.revision = next_revision(&guild)?;
                    guild
                        .members
                        .retain(|member| member.identity != expected_target);
                    store.shared_guilds.insert(before.id.clone(), guild.clone());
                    Ok((Some(guild), expected_target.clone(), removed_name))
                }
            },
        )
    }
}
