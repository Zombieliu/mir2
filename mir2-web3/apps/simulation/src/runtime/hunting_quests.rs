//! Server-only quest source adapter. Preparation owns a separate World and
//! account image; the live ECS changes only after the common PG commit confirms.
use bevy_ecs::prelude::{Resource, World};
use mir2_protocol::{ChatType, ServerPacket};
use sha2::{Digest, Sha256};

use crate::config::{CharacterSaveRecord, HuntingQuestCommit, HuntingSourceError, QuestStage};

use super::components::{player_entity, CharacterBody, PlayerVitals};
use super::quests::{self, QuestState};
use super::resources::{
    runtime_tick, set_runtime_tick, BuffResource, ElementalResource, GroupResource,
    InventoryResource, MapRuntimeResource, MountResource, NpcStateResource,
    PlayerPermissionResource, PlayerRuntimeResource, QuestResource, RuntimeConfigResource,
    RuntimeQueueResource, SessionResource, SkillResource, Stage5SystemsResource,
};
use super::save::{active_session_mutating_account_id, snapshot_active_character_save};
use super::session::SimulationSession;

#[derive(Resource)]
struct RejectionPacket(ServerPacket);

#[cfg(test)]
#[derive(Resource)]
struct HuntingQuestTestFault(u8);

const UNSUPPORTED: &str =
    "This quest reward is not supported by the bound economy yet; no reward was claimed.";
const UNAVAILABLE: &str = "Quest economy settlement is unavailable; no reward was published. Please retry the original quest.";
const RECOVERING: &str = "This quest settlement requires recovery. Do not claim another reward; reconnect after recovery.";
const RELOAD_REQUIRED: &str = "This quest was settled, but the complete character checkpoint needs recovery. Automatic replay could not update this character; reconnect after recovery.";

fn message_packet(message: &'static str) -> ServerPacket {
    ServerPacket::Chat {
        message: message.into(),
        chat_type: ChatType::System,
    }
}

fn reject(world: &mut World, message: &'static str) -> bool {
    world.insert_resource(RejectionPacket(message_packet(message)));
    false
}

pub(super) fn take_rejection_packet(world: &mut World) -> Option<ServerPacket> {
    world
        .remove_resource::<RejectionPacket>()
        .map(|value| value.0)
}

struct PreparedQuest {
    shadow: SimulationSession,
    before: CharacterSaveRecord,
    planned: CharacterSaveRecord,
}

/// A stable definition digest deliberately excludes revision, transient EXP
/// modifiers, time, connection and position. Recovery must use the original
/// business slot even after a buff expires or the player reconnects.
fn source_definition(world: &World, quest_id: i32) -> Result<(String, u64), String> {
    let info = quests::effective_crystal_quest_info_by_id(world, quest_id)
        .ok_or_else(|| UNSUPPORTED.to_owned())?;
    let template = quests::effective_crystal_quest_template_by_id(world, quest_id)
        .ok_or_else(|| UNSUPPORTED.to_owned())?;
    let daily = quests::is_hunting_newcomer_daily_quest(world, quest_id);
    if (quests::quest_has_recurrence(world, quest_id) && !daily)
        || (daily && (info.quest_type != 1 || info.npc_index != 24 || info.finish_npc_index != 24))
        || (!daily && !matches!(info.quest_type, 0 | 3))
        || info.reward_gold == 0
        || info
            .rewards_fixed_item
            .iter()
            .any(|reward| reward.count > 0)
        || !info.rewards_select_item.is_empty()
        || !template.carry_items.is_empty()
        || !template.item_tasks.is_empty()
    {
        return Err(UNSUPPORTED.into());
    }
    let domain = if daily {
        "hunting-newcomer-daily-quest-definition-v1"
    } else {
        "hunting-once-quest-definition-v1"
    };
    let bytes = serde_json::to_vec(&(domain, &info, &template))
        .map_err(|_| "quest definition cannot be encoded".to_owned())?;
    let digest = format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    Ok((digest, u64::from(info.reward_gold)))
}

fn prepare(world: &World, quest_id: i32) -> Result<PreparedQuest, String> {
    prepare_with_daily_time(world, quest_id, None)
}

fn prepare_with_daily_time(
    world: &World,
    quest_id: i32,
    database_time_ms: Option<u64>,
) -> Result<PreparedQuest, String> {
    if quests::is_hunting_newcomer_daily_quest(world, quest_id) != database_time_ms.is_some() {
        return Err("daily quest preparation requires its database period capability".into());
    }
    if quests::quest_stage(world, quest_id) != Some(QuestStage::ReadyToTurnIn) {
        return Err("quest is not ready".into());
    }
    let config = &world.resource::<RuntimeConfigResource>().config;
    config.ensure_account_store_writable()?;
    let before = snapshot_active_character_save(world)
        .ok_or_else(|| "quest needs a complete active checkpoint".to_owned())?;
    let mut shadow = SimulationSession::new(config.fork_for_hunting_preparation()?);
    *shadow.app.world_mut().resource_mut::<SessionResource>() = world
        .resource::<SessionResource>()
        .fork_for_hunting_preparation();
    shadow.restore_active_character_checkpoint(&before)?;

    // Restore the real owned context after hydration. Never share revision,
    // journal mutexes, persistence authority or clock anchors with the player.
    macro_rules! owned {
        ($($ty:ty),* $(,)?) => { $(
            *shadow.app.world_mut().resource_mut::<$ty>() = world.resource::<$ty>().clone();
        )* };
    }
    owned!(
        InventoryResource,
        QuestResource,
        PlayerRuntimeResource,
        BuffResource,
        SkillResource,
        Stage5SystemsResource,
        MapRuntimeResource,
        MountResource,
        NpcStateResource,
        RuntimeQueueResource,
        ElementalResource,
        GroupResource,
        PlayerPermissionResource,
    );
    if let Some(guild) = world.get_resource::<super::shared_guilds::SharedGuildSession>() {
        shadow.app.world_mut().insert_resource(guild.clone());
    }
    // Hydration recomputes pools before the owned Guild/buff context is in
    // place. Begin with the exact live components, including damage and any
    // derived pool bonus; only a real level-up may run the usual refresh.
    let live_player = player_entity(world).ok_or("quest player entity missing")?;
    let body = *world
        .entity(live_player)
        .get::<CharacterBody>()
        .ok_or("quest player body missing")?;
    let vitals = *world
        .entity(live_player)
        .get::<PlayerVitals>()
        .ok_or("quest player vitals missing")?;
    let shadow_player = player_entity(shadow.app.world()).ok_or("prepared quest player missing")?;
    shadow
        .app
        .world_mut()
        .entity_mut(shadow_player)
        .insert((body, vitals));
    set_runtime_tick(shadow.app.world_mut(), runtime_tick(world));
    super::shared_guild_experience::restore(
        shadow.app.world_mut(),
        &super::shared_guild_experience::snapshot(world),
    );
    super::shared_guild_experience::begin_owned_hunting_preparation(shadow.app.world_mut());

    if let Some(now_ms) = database_time_ms {
        let period = quests::quest_recurrence::daily_period_at(now_ms);
        // OS refresh may already have advanced a live calendar. It cannot be
        // lowered or used as permission to select a different database slot.
        if shadow
            .app
            .world()
            .resource::<QuestResource>()
            .quests
            .iter()
            .filter(|quest| {
                quests::is_hunting_newcomer_daily_quest(shadow.app.world(), quest.quest_id)
            })
            .any(|quest| {
                quest
                    .cadence_high_watermark_period
                    .is_some_and(|value| value > period)
                    || quest
                        .cadence_last_claimed_period
                        .is_some_and(|value| value > period)
                    || (quest.cadence_last_claimed_period == Some(period)
                        && quest.quest_id == quest_id)
                    || (quest.stage == QuestStage::Completed
                        && quest.cadence_last_claimed_period.is_none())
            })
        {
            return Err("daily quest calendar is ahead of its database claim period".into());
        }
        let _ =
            quests::quest_recurrence::refresh_quest_recurrence_at(shadow.app.world_mut(), now_ms);
        if quests::quest_stage(shadow.app.world(), quest_id) != Some(QuestStage::ReadyToTurnIn) {
            return Err("daily quest eligibility changed at database reset".into());
        }
    }

    // Recheck server level/class/profile/prerequisite gates without the ordinary
    // "already accepted" gate rejecting this existing ReadyToTurnIn quest.
    let info = quests::effective_crystal_quest_info_by_id(shadow.app.world(), quest_id)
        .ok_or_else(|| "effective quest definition disappeared".to_owned())?;
    quests::set_quest_stage(shadow.app.world_mut(), quest_id, QuestStage::Available);
    let eligible = quests::can_accept_crystal_quest(shadow.app.world(), &info);
    quests::set_quest_stage(shadow.app.world_mut(), quest_id, QuestStage::ReadyToTurnIn);
    let completed = eligible
        && match database_time_ms {
            Some(now_ms) => quests::complete_quest_personal_at(
                shadow.app.world_mut(),
                quest_id,
                None,
                false,
                now_ms,
            ),
            None => quests::complete_quest_personal(shadow.app.world_mut(), quest_id, None, false),
        };
    if !completed {
        return Err("quest eligibility changed".into());
    }
    if let Some(now_ms) = database_time_ms {
        let period = quests::quest_recurrence::daily_period_at(now_ms);
        let quest = shadow
            .app
            .world()
            .resource::<QuestResource>()
            .quests
            .iter()
            .find(|quest| quest.quest_id == quest_id)
            .ok_or("prepared daily quest missing")?;
        if quest.cadence_last_claimed_period != Some(period)
            || quest.cadence_high_watermark_period != Some(period)
        {
            return Err("prepared daily completion left its database claim period".into());
        }
    }
    super::shared_guild_experience::finish_owned_hunting_preparation(shadow.app.world())?;
    let after = snapshot_active_character_save(shadow.app.world())
        .ok_or_else(|| "prepared quest checkpoint missing".to_owned())?;
    if after.gold != before.gold
        || after.inventory_items_json != before.inventory_items_json
        || after.belt_items_json != before.belt_items_json
        || after.equipment_items_json != before.equipment_items_json
    {
        return Err("unsupported quest attempted an item or legacy gold mutation".into());
    }
    // Only the supported reward fields are staged. The original timer and Zone
    // transform carriers are retained verbatim, rather than reserializing time.
    let mut planned = before.clone();
    planned.character = after.character;
    planned.hp = after.hp;
    planned.max_hp = after.max_hp;
    planned.mp = after.mp;
    planned.max_mp = after.max_mp;
    planned.experience = after.experience;
    planned.max_experience = after.max_experience;
    planned.credit = after.credit;
    planned.quest_states_json = after.quest_states_json;
    planned.guild_experience_journal = after.guild_experience_journal;
    Ok(PreparedQuest {
        shadow,
        before,
        planned,
    })
}

struct PersonalProjection {
    character: crate::CharacterRecord,
    quests: Vec<QuestState>,
    experience: i64,
    max_experience: i64,
    credit: u32,
    revision: u64,
    journal: crate::config::GuildExperienceJournal,
}

impl PersonalProjection {
    fn decode(world: &World, save: &CharacterSaveRecord) -> Result<Self, String> {
        super::save::validate_character_save_record(save)?;
        let session = world.resource::<SessionResource>();
        let current = session
            .selected_character
            .as_ref()
            .ok_or("quest player retired")?;
        if current.index != save.character.index || current.name != save.character.name {
            return Err("committed quest belongs to another character".into());
        }
        let quests = save
            .quest_states_json
            .iter()
            .map(|encoded| {
                serde_json::from_str(encoded)
                    .map_err(|_| "committed quest state is invalid".to_owned())
            })
            .collect::<Result<Vec<QuestState>, _>>()?;
        Ok(Self {
            character: save.character.clone(),
            quests,
            experience: save.experience,
            max_experience: save.max_experience,
            credit: save.credit,
            revision: save.revision,
            journal: save.guild_experience_journal.clone(),
        })
    }

    fn apply(
        self,
        world: &mut World,
        expected_revision: u64,
        fresh_before: Option<&CharacterSaveRecord>,
    ) -> Result<(), String> {
        // A recovered current checkpoint may include a later session's bag,
        // wallet or other changes. Publishing only this quest projection must
        // never bless those old live fields with that later complete revision.
        if expected_revision.checked_add(1) != Some(self.revision) {
            return Err("quest recovery requires a fresh complete character load".into());
        }
        let player = player_entity(world).ok_or("quest player entity retired")?;
        let body = world
            .entity(player)
            .get::<CharacterBody>()
            .ok_or("quest player body retired")?;
        let current = *world
            .entity(player)
            .get::<PlayerVitals>()
            .ok_or("quest player vitals retired")?;
        // A fresh synchronous claim can retain the ordinary LevelUp refill.
        // Unknown/replayed claims cannot infer the order of intervening Zone
        // damage, healing or death from two HP values, so never replay a refill.
        let refill_level_up = fresh_before.is_some_and(|before| {
            self.character.level > before.character.level
                && body.level == before.character.level
                && current.hp > 0
                && (current.hp, current.max_hp, current.mp, current.max_mp)
                    == (before.hp, before.max_hp, before.mp, before.max_mp)
        });
        if !world
            .resource::<SessionResource>()
            .advance_active_save_revision(expected_revision, self.revision)
        {
            return Err("committed quest projection revision changed".into());
        }
        {
            let mut session = world.resource_mut::<SessionResource>();
            session.selected_character = Some(self.character.clone());
            if let Some(character) = session
                .characters
                .iter_mut()
                .find(|item| item.index == self.character.index)
            {
                *character = self.character.clone();
            }
        }
        world.resource_mut::<QuestResource>().quests = self.quests;
        {
            let mut runtime = world.resource_mut::<PlayerRuntimeResource>();
            runtime.experience = self.experience;
            runtime.max_experience = self.max_experience;
            runtime.credit = self.credit;
        }
        world
            .entity_mut(player)
            .get_mut::<CharacterBody>()
            .expect("preflight body")
            .level = self.character.level;
        super::stats::refresh_player_stats(world);
        // Caps come from the current level/equipment/Guild and still-active
        // real-time buffs. A PG wait must never restore an expired saved cap.
        if refill_level_up {
            let updated = {
                let mut entity = world.entity_mut(player);
                let mut vitals = entity.get_mut::<PlayerVitals>().expect("preflight vitals");
                vitals.hp = vitals.max_hp;
                vitals.mp = vitals.max_mp;
                *vitals
            };
            world.resource_mut::<PlayerRuntimeResource>().player_vitals = updated;
        }
        super::shared_guild_experience::restore(world, &self.journal);
        Ok(())
    }
}

fn publish(
    world: &mut World,
    commit: &HuntingQuestCommit,
    expected_revision: u64,
    fresh_before: Option<&CharacterSaveRecord>,
) -> Result<(), String> {
    let projection = PersonalProjection::decode(world, &commit.checkpoint)?;
    projection.apply(
        world,
        expected_revision,
        if commit.credit.replayed {
            None
        } else {
            fresh_before
        },
    )
}

/// Called only after the ordinary request's NPC/diary authorization. This does
/// not consult today's reward definition: the config owns the original digest,
/// amount and epoch, and can release only that exact actor's source fence.
pub(super) fn recover_original_request(
    world: &mut World,
    quest_id: i32,
) -> Option<Vec<ServerPacket>> {
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    config.hunting_economy_realm.as_ref()?;
    let session = world.resource::<SessionResource>();
    let account = active_session_mutating_account_id(session)?;
    let witness = session.hunting_character_incarnation.clone()?;
    let index = session.selected_character.as_ref()?.index;
    let revision = session.active_save_revision()?;
    match config.recover_pending_hunting_quest_source(&account, &witness, index, quest_id) {
        Ok(None) => None,
        Ok(Some(commit)) => match publish(world, &commit, revision, None) {
            Ok(()) => Some(vec![
                ServerPacket::ChangeQuest {
                    quest_id,
                    task_list: vec![],
                    taken: false,
                    completed: true,
                    new: false,
                    quest_state: 2,
                    track_quest: false,
                },
                ServerPacket::CompleteQuest {
                    completed_quests: quests::completed_quest_ids(world),
                },
            ]),
            Err(detail) => {
                config.freeze_hunting_quest_publication(detail);
                Some(vec![message_packet(RELOAD_REQUIRED)])
            }
        },
        Err(_) => Some(vec![message_packet(RECOVERING)]),
    }
}

/// Shared by both FinishQuest and NPC quest targets. Some(false) is a terminal
/// refusal for this enabled source path, never permission to award old gold.
pub(super) fn complete_if_configured(
    world: &mut World,
    quest_id: i32,
    selected: Option<i32>,
) -> Option<bool> {
    let config = world.resource::<RuntimeConfigResource>().config.clone();
    if config.hunting_economy_realm.is_none() {
        return None;
    }
    world.remove_resource::<RejectionPacket>();
    if let Some(packets) = recover_original_request(world, quest_id) {
        if packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::CompleteQuest { .. }))
        {
            return Some(true);
        }
        if let Some(message) = packets
            .into_iter()
            .find(|packet| matches!(packet, ServerPacket::Chat { .. }))
        {
            world.insert_resource(RejectionPacket(message));
        }
        return Some(false);
    }
    if selected.is_some() {
        return Some(reject(world, UNSUPPORTED));
    }
    let session = world.resource::<SessionResource>();
    let Some(account) = active_session_mutating_account_id(session) else {
        return Some(reject(world, UNAVAILABLE));
    };
    let Some(incarnation) = session.hunting_character_incarnation.clone() else {
        return Some(reject(world, UNAVAILABLE));
    };
    // A midnight wait can invalidate an uncommitted daily plan. Start again
    // from a fresh DB observation and owned World at most once; a published or
    // unknown result always stays on its original slot and never enters here.
    for attempt in 0..=1 {
        let (digest, amount) = match source_definition(world, quest_id) {
            Ok(value) => value,
            Err(_) => return Some(reject(world, UNSUPPORTED)),
        };
        let daily = quests::is_hunting_newcomer_daily_quest(world, quest_id);
        let Some(before) = snapshot_active_character_save(world) else {
            return Some(reject(world, UNAVAILABLE));
        };
        let period = if daily {
            match config.read_hunting_daily_quest_period(&account, &incarnation, quest_id, &before)
            {
                Ok(period) => Some(period),
                Err(_) => return Some(reject(world, UNAVAILABLE)),
            }
        } else {
            None
        };
        let prepared = match period.as_ref() {
            Some(period) => {
                prepare_with_daily_time(world, quest_id, Some(period.database_time_ms()))
            }
            None => prepare(world, quest_id),
        };
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(_) => return Some(reject(world, UNAVAILABLE)),
        };
        let planned = prepared.planned.clone();
        let source = if let Some(period) = period.as_ref() {
            config.prepare_hunting_daily_quest_source(
                period,
                &digest,
                amount,
                prepared.before.clone(),
                |store| {
                    super::save::stage_prepared_character_save(
                        prepared.shadow.app.world(),
                        store,
                        planned,
                    )
                },
            )
        } else {
            config.prepare_hunting_quest_source(
                &account,
                &incarnation,
                quest_id,
                &digest,
                amount,
                prepared.before.clone(),
                |store| {
                    super::save::stage_prepared_character_save(
                        prepared.shadow.app.world(),
                        store,
                        planned,
                    )
                },
            )
        };
        let source = match source {
            Ok(source) => source,
            Err(_) => return Some(reject(world, UNAVAILABLE)),
        };
        #[cfg(test)]
        let committed = match world.remove_resource::<HuntingQuestTestFault>() {
            Some(fault) => config.publish_hunting_quest_source_with_test_fault(&source, fault.0),
            None => config.publish_hunting_quest_source(&source),
        };
        #[cfg(not(test))]
        let committed = config.publish_hunting_quest_source(&source);
        match committed {
            Ok(commit) => {
                return Some(
                    match publish(world, &commit, before.revision, Some(&before)) {
                        Ok(()) => true,
                        Err(detail) => {
                            config.freeze_hunting_quest_publication(detail);
                            reject(world, RELOAD_REQUIRED)
                        }
                    },
                )
            }
            Err(HuntingSourceError::PeriodChanged) if daily && attempt == 0 => continue,
            Err(HuntingSourceError::PeriodChanged | HuntingSourceError::Rejected(_)) => {
                return Some(reject(world, UNAVAILABLE));
            }
            Err(HuntingSourceError::OutcomeUnknown(_))
            | Err(HuntingSourceError::PostCommit { .. })
            | Err(HuntingSourceError::KnownCommitted { .. }) => {
                return Some(reject(world, RECOVERING));
            }
        }
    }
    Some(reject(world, UNAVAILABLE))
}

#[cfg(test)]
#[path = "hunting_quests_tests.rs"]
mod tests;
