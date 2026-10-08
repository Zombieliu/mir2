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
pub(super) struct SharedKillExperiencePermit(
    pub GuildExperienceCommitPermit,
    pub Option<super::zone::ZoneMentorBankAttribution>,
);
pub(super) fn has_validated_selected_gain(world:&World,amount:i64)->bool {
    let Some(permit)=world.get_resource::<SharedKillExperiencePermit>() else{return false;};
    let Some(selection)=&permit.0.selection else{return false;};
    let session=world.resource::<SessionResource>();
    session.account_id.as_deref()==Some(permit.0.identity.account_id.as_str())
        && session.selected_character.as_ref().is_some_and(|character|character.index==permit.0.identity.character_index)
        && selection.account_id==permit.0.identity.account_id
        && selection.character_index==permit.0.identity.character_index
        && i64::from(selection.final_amount)==amount
}
pub(super) fn commit_source<T, F>(
    world: &World,
    config: &crate::SimulationConfig,
    accounts: &[String],
    transaction: F,
) -> Result<T, String>
where
    F: FnOnce(&mut crate::config::AccountStore) -> Result<T, String>,
{
    let actions = super::npc_shared_guild_actions::capture(world);
    let guild_ids = super::npc_shared_guild_actions::guild_ids(&actions);
    if !guild_ids.is_empty() {
        return config.commit_source_with_guild_post(accounts, &guild_ids,
            world.get_resource::<SharedKillExperiencePermit>().map(|permit| &permit.0),
            transaction, |store| super::npc_shared_guild_actions::apply(store, &actions));
    }
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
pub(super) fn source_command_active(world: &World) -> bool {
    world.get_resource::<GuildExperienceResource>().is_some_and(|resource| resource.1.load(Ordering::Acquire))
}
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
            if permit.0.identity != identity {
                return Err("shared kill GainExp identity differs from immutable selection".into());
            }
            let Some(selection) = &permit.0.selection else {
                return Ok(());
            };
            if selection.account_id != identity.account_id
                || selection.character_index != identity.character_index
                || selection.final_amount != amount
            {
                return Err("shared kill GainExp differs from immutable selection".into());
            }
            let Some(guild) = &selection.guild else {
                return Ok(());
            };
            (
                guild.guild_id.clone(),
                guild.membership_epoch,
                permit.0.selection_hash.clone(),
            )
        } else {
            // REMOVEFROMGUILD earlier in this very source command takes effect
            // before a subsequent GiveExp, even though publication is one CAS.
            if super::npc_shared_guild_actions::has_pending(world) { return Ok(()); }
            let Some(guild) = super::shared_guilds::guild_view(world) else {
                return Ok(());
            };
            if guild.name == world.resource::<super::resources::RuntimeConfigResource>()
                .config.crystal_newbie_guild_name
            {
                return Ok(());
            }
            let member = guild
                .members
                .iter()
                .find(|member| member.identity == identity)
                .ok_or("guild XP membership missing")?;
            (guild.id.clone(), member.membership_epoch, None)
        };
    if amount == 0 {
        return Ok(());
    }
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
    force_save: bool,
    save: CharacterSaveRecord,
    player_dead: bool,
    visible: BTreeSet<u32>,
    dirty_economy: BTreeSet<String>,
    buffs: BuffResource,
    npc: NpcStateResource,
    queue: RuntimeQueueResource,
    tick: u64,
    hero: super::hero_ai::hero_transient::HeroTransientCheckpoint,
    npc_guild_actions: super::npc_shared_guild_actions::PendingSourceGuildActions,
    pet_progress: super::shared_pet_progress::SharedPetProgress,
    default_transient: super::default_npc_events::DefaultNpcTransientState,
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
pub(super) fn reject_source(world: &World, error: String) {
    if let Some(resource) = world.get_resource::<GuildExperienceResource>() {
        *resource.2.lock().expect("XP source error state poisoned") = Some(error);
    }
}

fn mentor_checkpoint_required(world: &World) -> bool {
    let session = world.resource::<SessionResource>();
    let (Some(account), Some(character)) = (&session.account_id, &session.selected_character) else {
        return false;
    };
    world.resource::<RuntimeConfigResource>().config
        .shared_mentor_profile_for(&Stage5FriendIdentity {
            account_id: account.clone(), character_index: character.index,
        }).is_ok_and(|profile| profile.mentor.partner_identity.is_some())
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
    pub(crate) fn begin_default_npc_source_command(&mut self)
        -> Result<Option<GuildExperienceCheckpoint>, String> {
        let mut before = self.begin_guild_experience_command(true)?;
        if let Some(checkpoint) = before.as_mut() { checkpoint.force_save = false; }
        Ok(before)
    }
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
        let mentor_active = mentor_checkpoint_required(world);
        let guild_active = super::shared_guilds::enabled(world)
            && super::shared_guilds::guild_view(world).is_some();
        if !force && !mentor_active && !guild_active && !super::shared_pet_progress::has_live_admission(world) { return Ok(None); }
        world
            .resource::<RuntimeConfigResource>()
            .config
            .ensure_account_store_writable()?;
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
            force_save: force,
            save,
            player_dead: super::components::current_player_is_dead(world),
            visible: self.visible_objects.clone(),
            dirty_economy: self.dirty_economy_projection_event_ids.clone(),
            buffs: world.resource::<BuffResource>().clone(),
            npc: world.resource::<NpcStateResource>().clone(),
            queue: world.resource::<RuntimeQueueResource>().clone(),
            tick: runtime_tick(world),
            hero: super::hero_ai::hero_transient::capture(world),
            npc_guild_actions: super::npc_shared_guild_actions::capture(world),
            pet_progress: super::shared_pet_progress::capture(world),
            default_transient: super::default_npc_events::capture_transient(world),
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
        let mentor_before = before.save.stage5_systems_json.as_deref()
            .map(serde_json::from_str::<crate::Stage5SystemsState>).transpose()
            .map_err(|e| format!("mentor source checkpoint invalid: {e}"))?
            .unwrap_or_default().mentor;
        let mentor_now = &self.app.world().resource::<super::resources::Stage5SystemsResource>()
            .stage5_systems.mentor;
        let force_save = before.force_save || mentor_now.ledger != mentor_before.ledger
            || super::shared_pet_progress::has_earned_steps(self.app.world())
            || super::default_npc_events::capture(self.app.world()) != before.save.default_npc_events
            || super::npc_shared_guild_actions::has_pending(self.app.world())
            || ((mentor_before.partner_identity.is_some() || mentor_checkpoint_required(self.app.world()))
                && self.active_character_checkpoint().is_some_and(|save|
                    save.experience != before.save.experience || save.character.level != before.save.character.level))
            || super::quests::periodic_quests::successful_claim_since(self.app.world(),&before.save,&packets);
        let result = match source_error.map_or_else(|| has_uncommitted(self.app.world()), Err) {
            Ok(false) if !force_save => return Ok(packets),
            Ok(false) => super::save::persist_active_character_save(self.app.world()),
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
                self.app.world_mut().resource_mut::<super::resources::PlayerRuntimeResource>().player_dead = before.player_dead;
                // Preserve original in-memory clock anchors. Deserializing the
                // saved remaining duration alone would extend real-time buffs.
                *self.app.world_mut().resource_mut::<BuffResource>() = before.buffs;
                *self.app.world_mut().resource_mut::<NpcStateResource>() = before.npc;
                *self.app.world_mut().resource_mut::<RuntimeQueueResource>() = before.queue;
                set_runtime_tick(self.app.world_mut(), before.tick);
                super::hero_ai::hero_transient::restore(self.app.world_mut(), &before.hero)?;
                super::npc_shared_guild_actions::restore(self.app.world_mut(), before.npc_guild_actions);
                super::shared_pet_progress::restore(self.app.world_mut(), before.pet_progress);
                super::default_npc_events::restore_transient(self.app.world_mut(),&before.default_transient);
                self.visible_objects = before.visible;
                self.dirty_economy_projection_event_ids = before.dirty_economy;
            } else {
                self.restore_active_character_checkpoint(&durable)?;
                self.app.world_mut().resource_mut::<super::resources::PlayerRuntimeResource>().player_dead = before.player_dead;
                super::npc_shared_guild_actions::acknowledge(self.app.world_mut());
            }
            return Err(error);
        }
        let sequence = snapshot(self.app.world()).next_sequence;
        acknowledge(self.app.world(), sequence);
        super::npc_shared_guild_actions::acknowledge(self.app.world_mut());
        super::shared_pet_progress::acknowledge(self.app.world_mut());
        Ok(packets)
    }
}
