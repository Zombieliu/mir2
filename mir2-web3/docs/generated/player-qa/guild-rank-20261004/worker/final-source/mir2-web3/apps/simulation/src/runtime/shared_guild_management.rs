//! Bounded shared rank-name authority. Membership and other management stay separate.
use crate::config::{SharedGuildRecord, SimulationConfig, Stage5FriendIdentity};

impl SimulationConfig {
    /// The Gateway derives `identity` from its authenticated active presence.
    /// Recheck all authority inside the existing scoped durable transaction.
    pub fn commit_shared_guild_rank_name(
        &self,
        identity: &Stage5FriendIdentity,
        rank_index: u8,
        rank_name: &str,
    ) -> Result<SharedGuildRecord, String> {
        let guild_id = self
            .shared_guild_for_identity(identity)?
            .ok_or("You are not in a guild.")?
            .id;
        self.commit_account_store_transaction_with_guilds(
            std::slice::from_ref(&identity.account_id),
            std::slice::from_ref(&guild_id),
            |store| {
                if !store
                    .accounts
                    .get(&identity.account_id)
                    .is_some_and(|account| {
                        account
                            .characters
                            .iter()
                            .any(|character| character.index == identity.character_index)
                    })
                {
                    return Err("Guild character no longer exists.".into());
                }
                let guild = store
                    .shared_guilds
                    .get_mut(&guild_id)
                    .ok_or("Guild no longer exists.")?;
                let member = guild.member(identity).ok_or("You are not in this guild.")?;
                let actor_rank = guild
                    .ranks
                    .iter()
                    .find(|rank| rank.index == member.rank_index)
                    .ok_or("Guild rank no longer exists.")?;
                // Crystal CanChangeRank is bit 1; a displayed leader name is not authority.
                if actor_rank.options & 1 == 0 {
                    return Err("Your rank cannot change guild ranks.".into());
                }
                if !(3..=20).contains(&rank_name.encode_utf16().count()) || rank_name.contains('\\')
                {
                    return Err(
                        "Guild rank names must contain 3 to 20 characters and no backslash.".into(),
                    );
                }
                // Rename allows the actor's own rank; option editing has a different rule.
                if actor_rank.index > rank_index {
                    return Err("Your rank cannot rename a higher guild rank.".into());
                }
                let next_revision = guild
                    .revision
                    .checked_add(1)
                    .ok_or("Guild revision exhausted.")?;
                let target_rank = guild
                    .ranks
                    .iter_mut()
                    .find(|rank| rank.index == rank_index)
                    .ok_or("Guild rank no longer exists.")?;
                // Source sets/broadcasts on every valid request, including the same name.
                target_rank.name = rank_name.into();
                guild.revision = next_revision;
                Ok(guild.clone())
            },
        )
    }
}
