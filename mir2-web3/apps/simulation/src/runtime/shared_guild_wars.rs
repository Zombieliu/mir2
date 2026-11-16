//! Ordinary Guild wars: stable symmetric authority plus the durable Guild minute clock.
use super::*;

impl SimulationConfig {
    pub fn commit_shared_guild_war(
        &self,
        actor: &Stage5FriendIdentity,
        enemy_name: &str,
    ) -> Result<(SharedGuildRecord, SharedGuildRecord), String> {
        let own_id = self
            .shared_guild_for_identity(actor)?
            .ok_or("server.NotInGuild")?
            .id;
        let enemy_before = self
            .shared_guild_for_name(enemy_name)?
            .ok_or_else(|| format!("server.GuildNotFound:{enemy_name}"))?;
        let mut guild_ids = vec![own_id.clone(), enemy_before.id.clone()];
        guild_ids.sort();
        guild_ids.dedup();
        self.commit_account_store_transaction_with_guilds(
            std::slice::from_ref(&actor.account_id),
            &guild_ids,
            |store| {
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
                let mut own = store
                    .shared_guilds
                    .get(&own_id)
                    .ok_or("Guild no longer exists.")?
                    .clone();
                let member = own.member(actor).ok_or("server.NotInGuild")?;
                if member.rank_index != 0 {
                    return Err("Only Guild leaders can declare war.".into());
                }
                let mut enemy = store
                    .shared_guilds
                    .get(&enemy_before.id)
                    .ok_or_else(|| format!("server.GuildNotFound:{}", enemy_before.name))?
                    .clone();
                if enemy.name_key() != enemy_before.name_key() {
                    return Err("Guild target changed; retry the request.".into());
                }
                if own.id == enemy.id {
                    return Err("server.CannotWarOwnGuild".into());
                }
                if enemy.name == "NewbieGuild" {
                    return Err("server.CannotWarNewPlayersGuild".into());
                }
                if own.active_wars.contains_key(&enemy.id)
                    || enemy.active_wars.contains_key(&own.id)
                {
                    return Err("server.AlreadyAtWarWithGuild".into());
                }
                let settings = mir2_game_data::crystal_guild_settings();
                if settings.war_time < 0 {
                    return Err("Invalid Guild war duration.".into());
                }
                own.gold = own
                    .gold
                    .checked_sub(settings.war_cost)
                    .ok_or("server.GuildBankFundsInsufficient")?;
                own.revision = own
                    .revision
                    .checked_add(1)
                    .ok_or("Guild revision exhausted.")?;
                enemy.revision = enemy
                    .revision
                    .checked_add(1)
                    .ok_or("Guild revision exhausted.")?;
                own.active_wars.insert(enemy.id.clone(), settings.war_time);
                enemy.active_wars.insert(own.id.clone(), settings.war_time);
                store.shared_guilds.insert(own.id.clone(), own.clone());
                store.shared_guilds.insert(enemy.id.clone(), enemy.clone());
                Ok((own, enemy))
            },
        )
    }
}
