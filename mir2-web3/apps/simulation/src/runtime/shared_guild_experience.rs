use super::resources::{RuntimeConfigResource, SessionResource};
use crate::config::guild_experience::GuildExperienceCommitPermit;
use crate::config::{GuildExperienceEvent, GuildExperienceJournal, Stage5FriendIdentity};
use bevy_ecs::prelude::{Resource, World};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
#[path = "shared_kill_experience.rs"]
mod shared_kill;
pub use shared_kill::SharedMonsterKillCommitFailure;
#[derive(Debug, Resource)]
pub(super) struct SharedKillExperiencePermit(pub GuildExperienceCommitPermit);
pub(super) fn commit_source<T, F>(
    world: &World,
    config: &crate::SimulationConfig,
    accounts: &[String],
    transaction: F,
) -> Result<T, String>
where
    F: FnOnce(&mut crate::config::AccountStore) -> Result<T, String>,
{
    match world.get_resource::<SharedKillExperiencePermit>() {
        Some(permit) => config.commit_account_store_transaction_with_guild_experience_permit(
            accounts,
            &permit.0,
            transaction,
        ),
        None => config.commit_account_store_transaction(accounts, transaction),
    }
}
#[derive(Debug, Default, Resource)]
pub(super) struct GuildExperienceResource(
    Mutex<GuildExperienceJournal>,
    AtomicBool,
    Mutex<Option<String>>,
);
pub(super) fn snapshot(world: &World) -> GuildExperienceJournal {
    world
        .get_resource::<GuildExperienceResource>()
        .map(|resource| {
            resource
                .0
                .lock()
                .expect("guild experience journal poisoned")
                .clone()
        })
        .unwrap_or_default()
}
pub(super) fn restore(world: &mut World, journal: &GuildExperienceJournal) {
    world.insert_resource(GuildExperienceResource(
        Mutex::new(journal.clone()),
        AtomicBool::new(false),
        Mutex::new(None),
    ));
}
pub(super) fn acknowledge(world: &World, sequence: u64) {
    if let Some(resource) = world.get_resource::<GuildExperienceResource>() {
        resource
            .0
            .lock()
            .expect("guild experience journal poisoned")
            .pending
            .retain(|event| event.sequence > sequence);
    }
}
pub(super) fn record_gain(world: &mut World, amount: u32) -> Result<(), String> {
    // Shared kill envelopes have their own immutable selection/receipt path.
    // A source may only journal while its complete-checkpoint guard is active.
    if !world
        .get_resource::<GuildExperienceResource>()
        .is_some_and(|resource| resource.1.load(Ordering::Acquire))
    {
        return Ok(());
    }
    if amount == 0 {
        return Ok(());
    }
    let session = world.resource::<SessionResource>();
    let identity = Stage5FriendIdentity {
        account_id: session
            .account_id
            .clone()
            .ok_or("guild XP requires account")?,
        character_index: session
            .selected_character
            .as_ref()
            .ok_or("guild XP requires character")?
            .index,
    };
    let source_revision = session
        .active_save_revision()
        .ok_or("guild XP requires durable source revision")?;
    let (guild_id, epoch, capture_hash) =
        if let Some(permit) = world.get_resource::<SharedKillExperiencePermit>() {
            let Some(selection) = &permit.0.selection else {
                return Ok(());
            };
            let Some(guild) = &selection.guild else {
                return Ok(());
            };
            if permit.0.identity != identity || selection.final_amount != amount {
                return Err("shared kill GainExp differs from immutable selection".into());
            }
            (
                guild.guild_id.clone(),
                guild.membership_epoch,
                permit.0.selection_hash.clone(),
            )
        } else {
            let Some(guild) = super::shared_guilds::guild_view(world) else {
                return Ok(());
            };
            let member = guild
                .members
                .iter()
                .find(|member| member.identity == identity)
                .ok_or("guild XP membership missing")?;
            (guild.id.clone(), member.membership_epoch, None)
        };
    if !world.contains_resource::<GuildExperienceResource>() {
        world.insert_resource(GuildExperienceResource::default());
    }
    let mut journal = world
        .resource::<GuildExperienceResource>()
        .0
        .lock()
        .map_err(|_| "guild XP journal poisoned")?;
    let sequence = journal
        .next_sequence
        .checked_add(1)
        .ok_or("guild XP event sequence exhausted")?;
    let event_id = serde_json::to_string(&(
        "guild-xp-v1",
        &identity.account_id,
        identity.character_index,
        sequence,
    ))
    .map_err(|e| e.to_string())?;
    journal.next_sequence = sequence;
    journal.pending.push(GuildExperienceEvent {
        capture_hash,
        sequence,
        source_revision,
        event_id,
        identity,
        guild_id,
        membership_epoch: epoch,
        final_amount: amount,
    });
    Ok(())
}

use super::resources::{
    runtime_tick, set_runtime_tick, BuffResource, NpcStateResource, RuntimeQueueResource,
};
use super::session::SimulationSession;
use crate::config::CharacterSaveRecord;
use mir2_protocol::ServerPacket;
use std::collections::BTreeSet;
#[derive(Debug)]
pub(crate) struct GuildExperienceCheckpoint {
    save: CharacterSaveRecord,
    visible: BTreeSet<u32>,
    dirty_economy: BTreeSet<String>,
    buffs: BuffResource,
    npc: NpcStateResource,
    queue: RuntimeQueueResource,
    tick: u64,
    hero: super::hero_ai::hero_transient::HeroTransientCheckpoint,
}

pub(super) fn record_gain_or_reject(world: &mut World, amount: i64) -> bool {
    if !world
        .get_resource::<GuildExperienceResource>()
        .is_some_and(|resource| resource.1.load(Ordering::Acquire))
    {
        return true;
    }
    let result = u32::try_from(amount)
        .map_err(|_| "guild XP gain exceeds protocol amount".to_string())
        .and_then(|amount| record_gain(world, amount));
    if let Err(error) = result {
        if let Some(resource) = world.get_resource::<GuildExperienceResource>() {
            *resource.2.lock().expect("guild XP error state poisoned") = Some(error);
        }
        false
    } else {
        true
    }
}
fn has_uncommitted(world: &World) -> Result<bool, String> {
    let current = snapshot(world);
    if current.is_empty() {
        return Ok(false);
    }
    let session = world.resource::<SessionResource>();
    let account = session
        .account_id
        .as_deref()
        .ok_or("guild XP account missing")?;
    let index = session
        .selected_character
        .as_ref()
        .ok_or("guild XP character missing")?
        .index;
    let config = &world.resource::<RuntimeConfigResource>().config;
    let store = config
        .account_store
        .lock()
        .map_err(|_| "guild XP source store poisoned")?;
    let durable = &store
        .accounts
        .get(account)
        .and_then(|account| account.saves.get(&index))
        .ok_or("guild XP durable source missing")?
        .guild_experience_journal;
    for event in &current.pending {
        let hash = crate::config::guild_experience::source_event_hash(event)?;
        match durable.event_payloads.get(&event.event_id) {
            Some(previous) if previous != &hash => {
                return Err("guild XP source receipt payload mismatch".into())
            }
            None => return Ok(true),
            _ => {}
        }
    }
    for (key, hash) in &current.applied_kill_receipts {
        match durable.applied_kill_receipts.get(key) {
            Some(previous) if previous != hash => {
                return Err("shared kill receipt payload mismatch".into())
            }
            None => return Ok(true),
            _ => {}
        }
    }
    if let Some(resource) = world.get_resource::<GuildExperienceResource>() {
        let mut journal = resource.0.lock().map_err(|_| "guild XP journal poisoned")?;
        journal
            .pending
            .retain(|event| event.sequence > durable.next_sequence);
        journal
            .event_payloads
            .extend(durable.event_payloads.clone());
    }
    Ok(false)
}
impl SimulationSession {
    pub(crate) fn begin_guild_experience_command(
        &mut self,
        force: bool,
    ) -> Result<Option<GuildExperienceCheckpoint>, String> {
        self.app
            .world_mut()
            .init_resource::<GuildExperienceResource>();
        let world = self.app.world();
        if world
            .resource::<GuildExperienceResource>()
            .1
            .load(Ordering::Acquire)
        {
            return Ok(None);
        }
        if !force && !super::shared_guilds::enabled(world) {
            return Ok(None);
        }
        world
            .resource::<RuntimeConfigResource>()
            .config
            .ensure_account_store_writable()?;
        if !force && super::shared_guilds::guild_view(world).is_none() {
            return Ok(None);
        }
        let Some(save) = self.active_character_checkpoint() else {
            return Ok(None);
        };
        world
            .resource::<GuildExperienceResource>()
            .1
            .store(true, Ordering::Release);
        *world
            .resource::<GuildExperienceResource>()
            .2
            .lock()
            .map_err(|_| "guild XP error state poisoned")? = None;
        Ok(Some(GuildExperienceCheckpoint {
            save,
            visible: self.visible_objects.clone(),
            dirty_economy: self.dirty_economy_projection_event_ids.clone(),
            buffs: world.resource::<BuffResource>().clone(),
            npc: world.resource::<NpcStateResource>().clone(),
            queue: world.resource::<RuntimeQueueResource>().clone(),
            tick: runtime_tick(world),
            hero: super::hero_ai::hero_transient::capture(world),
        }))
    }
    pub(crate) fn finish_guild_experience_command(
        &mut self,
        before: Option<GuildExperienceCheckpoint>,
        packets: Vec<ServerPacket>,
    ) -> Result<Vec<ServerPacket>, String> {
        let Some(before) = before else {
            return Ok(packets);
        };
        self.app
            .world()
            .resource::<GuildExperienceResource>()
            .1
            .store(false, Ordering::Release);
        let source_error = self
            .app
            .world()
            .resource::<GuildExperienceResource>()
            .2
            .lock()
            .map_err(|_| "guild XP error state poisoned")?
            .take();
        let result = match source_error.map_or_else(|| has_uncommitted(self.app.world()), Err) {
            Ok(false) => return Ok(packets),
            Ok(true) => super::save::persist_active_character_save(self.app.world()),
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            let config = self
                .app
                .world()
                .resource::<RuntimeConfigResource>()
                .config
                .clone();
            // An unknown publication outcome is frozen by the repository. Do
            // not replace the live source with a guessed pre-commit checkpoint.
            if config.ensure_account_store_writable().is_err() {
                return Err(error);
            }
            let account = self
                .active_identity()
                .ok_or("guild XP rollback identity missing")?;
            let durable = config
                .account_store
                .lock()
                .map_err(|_| "guild XP rollback store poisoned")?
                .accounts
                .get(&account.account_id)
                .and_then(|account| account.saves.get(&before.save.character.index))
                .cloned()
                .ok_or("guild XP rollback source missing")?;
            if durable.revision == before.save.revision {
                self.restore_active_character_checkpoint(&before.save)?;
                // Preserve original in-memory clock anchors. Deserializing the
                // saved remaining duration alone would extend real-time buffs.
                *self.app.world_mut().resource_mut::<BuffResource>() = before.buffs;
                *self.app.world_mut().resource_mut::<NpcStateResource>() = before.npc;
                *self.app.world_mut().resource_mut::<RuntimeQueueResource>() = before.queue;
                set_runtime_tick(self.app.world_mut(), before.tick);
                super::hero_ai::hero_transient::restore(self.app.world_mut(), &before.hero)?;
                self.visible_objects = before.visible;
                self.dirty_economy_projection_event_ids = before.dirty_economy;
            } else {
                self.restore_active_character_checkpoint(&durable)?;
            }
            return Err(error);
        }
        let sequence = snapshot(self.app.world()).next_sequence;
        acknowledge(self.app.world(), sequence);
        Ok(packets)
    }
}
