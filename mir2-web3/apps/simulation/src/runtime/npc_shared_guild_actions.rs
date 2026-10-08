//! Trusted Default-NPC membership actions, published with their source save.
use bevy_ecs::prelude::{Resource, World};
use crate::config::{AccountStore, Stage5FriendIdentity};
use super::resources::{RuntimeConfigResource, SessionResource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceGuildLeave {
    identity: Stage5FriendIdentity,
    guild_id: String,
    membership_epoch: u64,
    source_revision: u64,
    guild_ids: Vec<String>,
}
#[derive(Debug, Clone, Default, Resource)]
pub(super) struct PendingSourceGuildActions(Vec<SourceGuildLeave>);

pub(super) fn capture(world: &World) -> PendingSourceGuildActions {
    world.get_resource::<PendingSourceGuildActions>().cloned().unwrap_or_default()
}
pub(super) fn restore(world: &mut World, value: PendingSourceGuildActions) { world.insert_resource(value); }
pub(super) fn has_pending(world: &World) -> bool { world.get_resource::<PendingSourceGuildActions>().is_some_and(|value| !value.0.is_empty()) }
pub(super) fn acknowledge(world: &mut World) { world.remove_resource::<PendingSourceGuildActions>(); }
pub(super) fn guild_ids(value: &PendingSourceGuildActions) -> Vec<String> {
    value.0.iter().flat_map(|action| action.guild_ids.iter().cloned()).collect::<std::collections::BTreeSet<_>>().into_iter().collect()
}

pub(super) fn queue_source_shared_guild_leave(world: &mut World) -> Result<(), String> {
    if !super::shared_guild_experience::source_command_active(world) {
        return Err("NPC guild action requires a complete source checkpoint".into());
    }
    let session = world.resource::<SessionResource>();
    let identity = Stage5FriendIdentity {
        account_id: session.account_id.clone().ok_or("NPC guild action requires authenticated account")?,
        character_index: session.selected_character.as_ref().ok_or("NPC guild action requires character")?.index,
    };
    let config = &world.resource::<RuntimeConfigResource>().config;
    let Some(guild) = config.shared_guild_for_identity(&identity)? else { return Ok(()); };
    let member = guild.member(&identity).ok_or("NPC guild member disappeared")?;
    // GuildObject.DeleteMember(actor, actor.Name) retains its last-leader rule.
    // NPC does not call the chat CanLeaveGuildAtWar gate.
    if member.rank_index == 0 && guild.members.len() > 1
        && guild.members.iter().filter(|member| member.rank_index == 0).count() <= 1 {
        return Ok(());
    }
    let action = SourceGuildLeave {
        identity,
        guild_id: guild.id.clone(),
        membership_epoch: member.membership_epoch,
        source_revision: guild.revision,
        guild_ids: std::iter::once(guild.id.clone()).chain(guild.active_wars.keys().cloned()).collect(),
    };
    world.init_resource::<PendingSourceGuildActions>();
    let pending = &mut world.resource_mut::<PendingSourceGuildActions>().0;
    if !pending.iter().any(|previous| previous.identity == action.identity) { pending.push(action); }
    Ok(())
}

pub(super) fn apply(store: &mut AccountStore, pending: &PendingSourceGuildActions) -> Result<(), String> {
    for action in &pending.0 {
        let guild = store.shared_guilds.get(&action.guild_id).ok_or("NPC source guild disappeared")?;
        let member = guild.member(&action.identity).ok_or("NPC source guild membership changed")?;
        let next_revision=action.source_revision.checked_add(1).ok_or("NPC source guild revision exhausted")?;
        if guild.revision!=action.source_revision && guild.revision!=next_revision {
            return Err("NPC source guild revision changed".into());
        }
        if member.membership_epoch != action.membership_epoch { return Err("NPC source guild membership epoch changed".into()); }
        if member.rank_index == 0 && guild.members.len() > 1
            && guild.members.iter().filter(|member| member.rank_index == 0).count() <= 1 {
            return Err("NPC source last leader changed".into());
        }
        let disband = guild.members.len() == 1;
        if disband {
            let opponents: Vec<String> = guild.active_wars.keys().cloned().collect();
            if opponents.iter().any(|id| !action.guild_ids.contains(id)) { return Err("NPC source guild war scope changed".into()); }
            for opponent in opponents {
                if let Some(other) = store.shared_guilds.get_mut(&opponent) {
                    if other.active_wars.remove(&action.guild_id).is_some() {
                        other.revision = other.revision.checked_add(1).ok_or("NPC opponent guild revision exhausted")?;
                    }
                }
            }
            store.shared_guilds.remove(&action.guild_id);
        } else {
            let guild = store.shared_guilds.get_mut(&action.guild_id).unwrap();
            guild.members.retain(|member| member.identity != action.identity);
            // Source GainExp and the following LevelUp script are one CAS.
            // XP may already have advanced the same Guild; publish once.
            guild.revision = next_revision;
        }
    }
    Ok(())
}
