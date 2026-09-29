//! Shared Guild domain transactions. Invitation transport and NPC grants are session-owned.
use super::items::{crystal_item_template_for_item_key, ItemState};
use crate::config::{
    AccountStore, CharacterSaveRecord, SharedGuildMember, SharedGuildRank, SharedGuildRecord,
    Stage5FriendIdentity,
};
use crate::SimulationConfig;
use rand_core::{OsRng, RngCore};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct GuildCreationCommit {
    save: CharacterSaveRecord,
    inventory: Vec<ItemState>,
    belt: Vec<ItemState>,
    packets: Vec<mir2_protocol::ServerPacket>,
}
fn creation_projection(
    before: &CharacterSaveRecord,
    after: CharacterSaveRecord,
) -> Result<GuildCreationCommit, String> {
    let decode = |items: &[String]| -> Result<Vec<ItemState>, String> {
        items
            .iter()
            .map(|item| serde_json::from_str(item).map_err(|e| e.to_string()))
            .collect()
    };
    let inventory = decode(&after.inventory_items_json)?;
    let belt = decode(&after.belt_items_json)?;
    let quantities: BTreeMap<u64, u32> = inventory
        .iter()
        .chain(&belt)
        .map(|item| (super::items::item_unique_id(item), item.quantity))
        .collect();
    let mut packets = Vec::new();
    if before.gold > after.gold {
        packets.push(mir2_protocol::ServerPacket::LoseGold {
            gold: before.gold - after.gold,
        });
    }
    for item in decode(&before.inventory_items_json)?
        .into_iter()
        .chain(decode(&before.belt_items_json)?)
    {
        let unique_id = super::items::item_unique_id(&item);
        let count = item
            .quantity
            .saturating_sub(quantities.get(&unique_id).copied().unwrap_or(0));
        if count > 0 {
            packets.push(mir2_protocol::ServerPacket::DeleteItem {
                unique_id,
                count: u16::try_from(count)
                    .map_err(|_| "guild material deletion exceeds protocol quantity")?,
            });
        }
    }
    Ok(GuildCreationCommit {
        save: after,
        inventory,
        belt,
        packets,
    })
}

fn guild_for<'a>(
    store: &'a AccountStore,
    identity: &Stage5FriendIdentity,
) -> Option<&'a SharedGuildRecord> {
    store
        .shared_guilds
        .values()
        .find(|guild| guild.member(identity).is_some())
}
fn fresh_guild_id() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|e| format!("guild ID entropy unavailable: {e}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
fn actual_item_name(item: &ItemState) -> String {
    crystal_item_template_for_item_key(&item.key)
        .map(|template| template.name)
        .unwrap_or_else(|| item.name.clone())
}
fn charge_creation_costs(save: &mut CharacterSaveRecord) -> Result<(), String> {
    let settings = mir2_game_data::crystal_guild_settings();
    if save.character.level < u16::from(settings.minimum_level) {
        return Err("server.GuildCreationLevel".into());
    }
    let mut costs = BTreeMap::<String, u32>::new();
    for cost in &settings.creation_costs {
        let current = costs.entry(cost.item_name.clone()).or_default();
        *current = current
            .checked_add(cost.amount)
            .ok_or("guild creation cost overflow")?;
    }
    let gold = costs.remove("").unwrap_or(0);
    if save.gold < gold {
        return Err("server.NotEnoughGold".into());
    }
    let decode = |items: &[String]| -> Result<Vec<ItemState>, String> {
        items
            .iter()
            .map(|item| {
                serde_json::from_str(item)
                    .map_err(|e| format!("invalid guild creation inventory: {e}"))
            })
            .collect()
    };
    let mut inventory = decode(&save.inventory_items_json)?;
    let mut belt = decode(&save.belt_items_json)?;
    for item in inventory.iter().chain(&belt) {
        super::items::validate_committed_item_state_carrier(item)
            .map_err(|e| format!("invalid guild creation carrier: {e}"))?;
    }
    // Complete preflight precedes every deduction, including repeated configured costs.
    for (name, amount) in &costs {
        let owned: u64 = inventory
            .iter()
            .chain(&belt)
            .filter(|item| actual_item_name(item) == *name)
            .map(|item| u64::from(item.quantity))
            .sum();
        if owned < u64::from(*amount) {
            return Err(format!("server.GuildCreationMissingItem:{name}"));
        }
    }
    for (name, amount) in costs {
        let mut remaining = amount;
        for item in inventory
            .iter_mut()
            .chain(&mut belt)
            .filter(|item| actual_item_name(item) == name)
        {
            let consumed = remaining.min(item.quantity);
            item.quantity -= consumed;
            remaining -= consumed;
            if remaining == 0 {
                break;
            }
        }
    }
    inventory.retain(|item| item.quantity != 0);
    belt.retain(|item| item.quantity != 0);
    let encode = |items: Vec<ItemState>| -> Result<Vec<String>, String> {
        items
            .iter()
            .map(|item| serde_json::to_string(item).map_err(|e| e.to_string()))
            .collect()
    };
    save.inventory_items_json = encode(inventory)?;
    save.belt_items_json = encode(belt)?;
    save.gold -= gold;
    Ok(())
}
impl SimulationConfig {
    fn commit_shared_guild_gold(
        &self,
        identity: &Stage5FriendIdentity,
        checkpoint: &CharacterSaveRecord,
        change_type: u8,
        amount: u32,
    ) -> Result<CharacterSaveRecord, String> {
        if change_type > 1 {
            return Err("invalid guild gold operation".into());
        }
        let guild_id = self
            .shared_guild_for_identity(identity)?
            .ok_or("guild membership missing")?
            .id;
        self.commit_account_store_transaction_with_guilds(
            std::slice::from_ref(&identity.account_id),
            std::slice::from_ref(&guild_id),
            |store| {
                let saved = store
                    .accounts
                    .get(&identity.account_id)
                    .and_then(|account| account.saves.get(&identity.character_index))
                    .ok_or("guild bank character missing")?;
                if saved.revision != checkpoint.revision
                    || checkpoint.character.index != identity.character_index
                    || saved.character.name != checkpoint.character.name
                {
                    return Err("stale guild bank checkpoint".into());
                }
                let mut next = checkpoint.clone();
                super::save::merge_persisted_mail_into_character_save(&mut next, saved)?;
                next.revision = saved
                    .revision
                    .checked_add(1)
                    .ok_or("character revision exhausted")?;
                let guild = store
                    .shared_guilds
                    .get_mut(&guild_id)
                    .ok_or("guild missing")?;
                let member = guild.member(identity).ok_or("guild membership changed")?;
                if change_type == 1 && member.rank_index != 0 {
                    return Err("guild leader required".into());
                }
                if change_type == 0 {
                    next.gold = next
                        .gold
                        .checked_sub(amount)
                        .ok_or("insufficient personal gold")?;
                    guild.gold = guild.gold.checked_add(amount).ok_or("guild gold limit")?;
                } else {
                    guild.gold = guild
                        .gold
                        .checked_sub(amount)
                        .ok_or("insufficient guild gold")?;
                    next.gold = next.gold.checked_add(amount).ok_or("personal gold limit")?;
                }
                guild.revision = guild
                    .revision
                    .checked_add(1)
                    .ok_or("guild revision exhausted")?;
                let account = store.accounts.get_mut(&identity.account_id).unwrap();
                let character = account
                    .characters
                    .iter_mut()
                    .find(|character| character.index == identity.character_index)
                    .ok_or("guild bank character missing")?;
                *character = next.character.clone();
                account.saves.insert(identity.character_index, next.clone());
                Ok(next)
            },
        )
    }
    /// Internal only: the session must consume its NPC one-use grant before calling.
    /// The full server checkpoint and creation cost are committed with the new guild.
    pub(super) fn commit_shared_guild_creation(
        &self,
        identity: &Stage5FriendIdentity,
        checkpoint: &CharacterSaveRecord,
        name: &str,
    ) -> Result<GuildCreationCommit, String> {
        if !(3..=20).contains(&name.encode_utf16().count()) || name.contains('\\') {
            return Err("server.GuildNameInvalid".into());
        }
        let id = fresh_guild_id()?;
        let accounts = vec![identity.account_id.clone()];
        self.commit_account_store_transaction_with_guilds(
            &accounts,
            std::slice::from_ref(&id),
            |store| {
                if guild_for(store, identity).is_some() {
                    return Err("server.AlreadyInGuild".into());
                }
                if store.shared_guilds.contains_key(&id) {
                    return Err("guild ID collision".into());
                }
                let saved = store
                    .accounts
                    .get(&identity.account_id)
                    .and_then(|account| account.saves.get(&identity.character_index))
                    .ok_or("guild creator save missing")?;
                if saved.revision != checkpoint.revision
                    || checkpoint.character.index != identity.character_index
                    || saved.character.name != checkpoint.character.name
                {
                    return Err("stale guild creation checkpoint".into());
                }
                let mut next = checkpoint.clone();
                super::save::merge_persisted_mail_into_character_save(&mut next, saved)?;
                charge_creation_costs(&mut next)?;
                next.revision = saved
                    .revision
                    .checked_add(1)
                    .ok_or("guild creator revision exhausted")?;
                let guild = SharedGuildRecord {
                    id: id.clone(),
                    name: name.into(),
                    revision: 1,
                    level: 0,
                    experience: 0,
                    spare_points: 0,
                    gold: 0,
                    ranks: vec![SharedGuildRank {
                        index: 0,
                        name: "Leader".into(),
                        options: 255,
                    }],
                    members: vec![SharedGuildMember { membership_epoch: 1,
                        identity: identity.clone(),
                        name: next.character.name.clone(),
                        rank_index: 0,
                    }],
                    notice: Vec::new(),
                    storage: BTreeMap::new(),
                    buffs: BTreeMap::new(),
                    last_buff_tick_ms: 0,
                    experience_receipts: BTreeSet::new(), experience_receipt_payloads: Default::default(),
                };
                if store
                    .shared_guilds
                    .values()
                    .any(|other| other.name_key() == guild.name_key())
                {
                    return Err("server.GuildNameExists".into());
                }
                // All decoding and wire-size validation precedes durable publication.
                let projection = creation_projection(checkpoint, next.clone())?;
                let account = store.accounts.get_mut(&identity.account_id).unwrap();
                let character = account
                    .characters
                    .iter_mut()
                    .find(|character| character.index == identity.character_index)
                    .ok_or("guild creator missing")?;
                *character = next.character.clone();
                account.saves.insert(identity.character_index, next.clone());
                store.shared_guilds.insert(id.clone(), guild.clone());
                Ok(projection)
            },
        )
    }
    /// Only transport's validated, still-live invitation may call this method.
    pub fn commit_shared_guild_join(
        &self,
        recruiter: &Stage5FriendIdentity,
        target: &Stage5FriendIdentity,
        guild_id: &str,
    ) -> Result<SharedGuildRecord, String> {
        let mut accounts = vec![recruiter.account_id.clone(), target.account_id.clone()];
        accounts.sort();
        accounts.dedup();
        self.commit_account_store_transaction_with_guilds(&accounts, &[guild_id.into()], |store| {
            if recruiter == target || guild_for(store, target).is_some() {
                return Err("server.AlreadyInGuild".into());
            }
            let target_name = store
                .accounts
                .get(&target.account_id)
                .and_then(|account| {
                    account
                        .characters
                        .iter()
                        .find(|character| character.index == target.character_index)
                })
                .map(|character| character.name.clone())
                .ok_or("guild invite target missing")?;
            let guild = store
                .shared_guilds
                .get_mut(guild_id)
                .ok_or("guild no longer exists")?;
            let member = guild
                .member(recruiter)
                .ok_or("guild recruiter no longer a member")?;
            let rank = guild
                .ranks
                .iter()
                .find(|rank| rank.index == member.rank_index)
                .ok_or("guild rank missing")?;
            if rank.options & 2 == 0 {
                return Err("server.GuildPermissionDenied".into());
            }
            if guild.members.iter().any(|member|member.identity.character_index==target.character_index){
                return Err("ambiguous legacy public character index; account migration required".into());
            }
            let cap = *mir2_game_data::crystal_guild_settings()
                .member_caps
                .get(usize::from(guild.level))
                .ok_or("guild level has no member cap")?;
            if cap != 0 && i64::try_from(guild.members.len()).unwrap_or(i64::MAX) >= i64::from(cap)
            {
                return Err("server.GuildFull".into());
            }
            if guild.ranks.len() == 1 {
                guild.ranks.push(SharedGuildRank {
                    index: 1,
                    name: "Members".into(),
                    options: 0,
                });
            }
            let rank_index = guild.ranks.last().ok_or("guild rank missing")?.index;
            guild.members.push(SharedGuildMember { membership_epoch: guild.revision.checked_add(1).ok_or("guild membership epoch exhausted")?,
                identity: target.clone(),
                name: target_name,
                rank_index,
            });
            guild.revision = guild
                .revision
                .checked_add(1)
                .ok_or("guild revision exhausted")?;
            Ok(guild.clone())
        })
    }
}
use super::resources::{
    InventoryResource, PlayerRuntimeResource, RuntimeConfigResource, SessionResource,
};
use crate::config::Stage5GuildState;
use bevy_ecs::prelude::{Resource, World};
use mir2_protocol::{GuildMember, GuildRank, ServerPacket};

#[derive(Resource, Default)]
pub(super) struct SharedGuildSession {
    pub enabled: bool,
    create_grant: Option<Stage5FriendIdentity>,
    online: BTreeSet<(String, i32)>,
    last_announced_name: Option<String>,
    loaded_bank_guild: Option<String>,
}
fn world_identity(world: &World) -> Option<Stage5FriendIdentity> {
    let session = world.resource::<SessionResource>();
    Some(Stage5FriendIdentity {
        account_id: session.account_id.clone()?,
        character_index: session.selected_character.as_ref()?.index,
    })
}
pub(super) fn enabled(world: &World) -> bool {
    world
        .get_resource::<SharedGuildSession>()
        .is_some_and(|state| state.enabled)
}
pub(super) fn is_guild_leader(world: &World) -> bool {
    let Some(identity) = world_identity(world) else {
        return false;
    };
    guild_view(world).is_some_and(|guild| {
        guild
            .member(&identity)
            .is_some_and(|member| member.rank_index == 0)
    })
}
pub(super) fn guild_view(world: &World) -> Option<SharedGuildRecord> {
    if !enabled(world) {
        return None;
    }
    world
        .resource::<RuntimeConfigResource>()
        .config
        .shared_guild_for_identity(&world_identity(world)?)
        .ok()
        .flatten()
}
pub(super) fn project_legacy_shape(world: &World) -> Option<Stage5GuildState> {
    if !enabled(world) {
        return None;
    }
    let Some(guild) = guild_view(world) else {
        return Some(Stage5GuildState::default());
    };
    let identity = world_identity(world)?;
    let member = guild.member(&identity)?;
    let rank = guild
        .ranks
        .iter()
        .find(|rank| rank.index == member.rank_index)?;
    let mut view = Stage5GuildState::default();
    view.name = guild.name.clone();
    view.rank = rank.name.clone();
    view.members = guild
        .members
        .iter()
        .map(|member| member.name.clone())
        .collect();
    view.notice = guild.notice.clone();
    view.storage_gold = guild.gold;
    for (bit, name) in [
        (1, "CanChangeRank"),
        (2, "CanRecruit"),
        (4, "CanKick"),
        (8, "CanStoreItem"),
        (16, "CanRetrieveItem"),
        (32, "CanAlterAlliance"),
        (64, "CanChangeNotice"),
        (128, "CanActivateBuff"),
    ] {
        if rank.options & bit != 0 {
            view.permissions.push(name.into());
        }
    }
    for (slot, stored) in &guild.storage {
        if let Ok(item) = serde_json::from_str::<ItemState>(&stored.item_state_json) {
            view.storage_items.insert(*slot, item.key);
            view.storage_item_states
                .insert(*slot, stored.item_state_json.clone());
            view.storage_item_users
                .insert(*slot, stored.depositor_character_index);
        }
    }
    Some(view)
}
pub(super) fn status_packet(world: &World) -> ServerPacket {
    let guild = guild_view(world);
    let settings = mir2_game_data::crystal_guild_settings();
    let identity = world_identity(world);
    let rank = guild.as_ref().and_then(|guild| {
        guild.member(identity.as_ref()?).and_then(|member| {
            guild
                .ranks
                .iter()
                .find(|rank| rank.index == member.rank_index)
        })
    });
    ServerPacket::GuildStatus {
        guild_name: guild
            .as_ref()
            .map(|guild| guild.name.clone())
            .unwrap_or_default(),
        guild_rank_name: rank.map(|rank| rank.name.clone()).unwrap_or_default(),
        level: guild.as_ref().map(|guild| guild.level).unwrap_or(0),
        experience: guild
            .as_ref()
            .map(|guild| i64::try_from(guild.experience).unwrap_or(i64::MAX))
            .unwrap_or(0),
        max_experience: guild
            .as_ref()
            .and_then(|guild| settings.experience_levels.get(usize::from(guild.level)))
            .copied()
            .unwrap_or(0),
        gold: guild.as_ref().map(|guild| guild.gold).unwrap_or(0),
        spare_points: guild.as_ref().map(|guild| guild.spare_points).unwrap_or(0),
        member_count: guild
            .as_ref()
            .map(|guild| guild.members.len() as i32)
            .unwrap_or(0),
        max_members: guild
            .as_ref()
            .and_then(|guild| settings.member_caps.get(usize::from(guild.level)))
            .copied()
            .unwrap_or(0),
        voting: false,
        item_count: guild
            .as_ref()
            .map(|guild| guild.storage.len() as u8)
            .unwrap_or(0),
        buff_count: guild
            .as_ref()
            .map(|guild| guild.buffs.len() as u8)
            .unwrap_or(0),
        my_options: rank.map(|rank| rank.options).unwrap_or(0),
        my_rank_id: rank.map(|rank| i32::from(rank.index)).unwrap_or(-1),
    }
}
pub(super) fn ranks_packet(world: &World) -> ServerPacket {
    let config = &world.resource::<RuntimeConfigResource>().config;
    let last_access = config
        .account_store
        .lock()
        .ok()
        .map(|store| {
            store
                .accounts
                .iter()
                .flat_map(|(account_id, account)| {
                    account
                        .character_last_access_binary_datetimes
                        .iter()
                        .map(move |(index, time)| ((account_id.clone(), *index), *time))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let ranks = guild_view(world)
        .map(|guild| {
            guild
                .ranks
                .iter()
                .map(|rank| GuildRank {
                    name: rank.name.clone(),
                    options: rank.options,
                    index: i32::from(rank.index),
                    members: guild
                        .members
                        .iter()
                        .filter(|member| member.rank_index == rank.index)
                        .map(|member| GuildMember {
                            name: member.name.clone(),
                            id: member.identity.character_index,
                            last_login_binary_datetime: last_access
                                .get(&(
                                    member.identity.account_id.clone(),
                                    member.identity.character_index,
                                ))
                                .copied()
                                .unwrap_or(0),
                            has_voted: false,
                            online: world.resource::<SharedGuildSession>().online.contains(&(
                                member.identity.account_id.clone(),
                                member.identity.character_index,
                            )),
                        })
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default();
    ServerPacket::GuildMemberChange {
        name: String::new(),
        rank_index: 0,
        status: u8::MAX,
        ranks,
    }
}
impl crate::SimulationSession {
    pub fn refresh_shared_guild_authority(&self) -> Result<(), String> {
        self.app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .refresh_shared_guild_authority()
    }

    pub fn enable_shared_guild_authority(&mut self) {
        if !enabled(self.app.world()) {
            self.app.world_mut().insert_resource(SharedGuildSession {
                enabled: true,
                ..Default::default()
            });
        }
    }
    pub(super) fn clear_shared_guild_grant(&mut self) {
        if let Some(mut state) = self
            .app
            .world_mut()
            .get_resource_mut::<SharedGuildSession>()
        {
            state.create_grant = None;
            state.last_announced_name = None;
            state.loaded_bank_guild = None;
        }
    }
    pub(super) fn grant_shared_guild_creation_from_npc(&mut self) -> Vec<ServerPacket> {
        if !enabled(self.app.world()) {
            return Vec::new();
        }
        let Some(identity) = world_identity(self.app.world()) else {
            return Vec::new();
        };
        let Some(checkpoint) = self.active_character_checkpoint() else {
            return Vec::new();
        };
        if checkpoint.character.level
            < u16::from(mir2_game_data::crystal_guild_settings().minimum_level)
            || guild_view(self.app.world()).is_some()
        {
            return Vec::new();
        }
        self.app
            .world_mut()
            .resource_mut::<SharedGuildSession>()
            .create_grant = Some(identity);
        vec![ServerPacket::GuildNameRequest]
    }
    pub fn submit_shared_guild_name(&mut self, name: &str) -> Result<Vec<ServerPacket>, String> {
        let identity =
            world_identity(self.app.world()).ok_or("guild create requires active character")?;
        let grant = self
            .app
            .world_mut()
            .get_resource_mut::<SharedGuildSession>()
            .and_then(|mut state| state.create_grant.take());
        if grant.as_ref() != Some(&identity) {
            return Err("guild create requires an unused NPC authorization".into());
        }
        if name.is_empty() {
            return Ok(Vec::new());
        }
        let checkpoint = self
            .active_character_checkpoint()
            .ok_or("guild creation checkpoint missing")?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        config.refresh_shared_guild_authority()?;
        let committed = config.commit_shared_guild_creation(&identity, &checkpoint, name)?;
        let mut packets = committed.packets;
        self.app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .inventory_items = committed.inventory;
        self.app
            .world_mut()
            .resource_mut::<InventoryResource>()
            .belt_items = committed.belt;
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = committed.save.gold;
        self.app
            .world()
            .resource::<SessionResource>()
            .advance_active_save_revision(checkpoint.revision, committed.save.revision);
        packets.push(status_packet(self.app.world()));
        packets.push(ranks_packet(self.app.world()));
        Ok(packets)
    }
    pub fn change_shared_guild_gold(
        &mut self,
        change_type: u8,
        amount: u32,
    ) -> Result<Vec<ServerPacket>, String> {
        if !enabled(self.app.world())
            || !super::resources::is_in_world(self.app.world())
            || !super::combat::current_player_in_safe_zone(self.app.world())
        {
            return Err("guild storage requires an active character in a safe zone".into());
        }
        let identity = world_identity(self.app.world()).ok_or("guild bank identity missing")?;
        let checkpoint = self
            .active_character_checkpoint()
            .ok_or("guild bank checkpoint missing")?;
        let config = self
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let committed =
            config.commit_shared_guild_gold(&identity, &checkpoint, change_type, amount)?;
        self.app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = committed.gold;
        self.app
            .world()
            .resource::<SessionResource>()
            .advance_active_save_revision(checkpoint.revision, committed.revision);
        Ok(vec![
            if change_type == 0 {
                ServerPacket::LoseGold { gold: amount }
            } else {
                ServerPacket::GainedGold { gold: amount }
            },
            ServerPacket::GuildStorageGoldChange {
                change_type,
                amount,
                name: checkpoint.character.name,
            },
        ])
    }
    pub fn shared_guild_packets(&mut self, online: &BTreeSet<(String, i32)>) -> Vec<ServerPacket> {
        if !super::resources::is_in_world(self.app.world()) || !enabled(self.app.world()) {
            return Vec::new();
        }
        self.app
            .world_mut()
            .resource_mut::<SharedGuildSession>()
            .online = online.clone();
        let mut packets = vec![
            status_packet(self.app.world()),
            ranks_packet(self.app.world()),
            buffs::active_packet(self.app.world()),
        ];
        super::stats::refresh_player_stats(self.app.world_mut());
        let name = guild_view(self.app.world())
            .map(|guild| guild.name)
            .unwrap_or_default();
        let announce = self
            .app
            .world()
            .resource::<SharedGuildSession>()
            .last_announced_name
            .as_ref()
            != Some(&name);
        if announce {
            self.app
                .world_mut()
                .resource_mut::<SharedGuildSession>()
                .last_announced_name = Some(name.clone());
            if let Some(object_id) = super::components::current_player_object_id(self.app.world()) {
                packets.push(ServerPacket::ObjectGuildNameChanged {
                    object_id,
                    guild_name: name,
                });
            }
        }
        packets
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ItemContainer, SimulationSession};
    use mir2_protocol::ClientPacket;
    pub(super) fn session() -> SimulationSession {
        let mut session = SimulationSession::new(SimulationConfig::default());
        session.enable_shared_guild_authority();
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        session
            .app
            .world_mut()
            .resource_mut::<SessionResource>()
            .selected_character
            .as_mut()
            .unwrap()
            .level = 22;
        let player = super::super::components::player_entity(session.app.world()).unwrap();
        session
            .app
            .world_mut()
            .entity_mut(player)
            .get_mut::<super::super::components::CharacterBody>()
            .unwrap()
            .level = 22;
        session
            .app
            .world_mut()
            .resource_mut::<PlayerRuntimeResource>()
            .gold = 2_000_000;
        let template = mir2_game_data::crystal_item_by_name("WoomaHorn").unwrap();
        super::super::inventory::add_or_increment_item(
            session.app.world_mut(),
            ItemContainer::Bag1,
            &super::super::items::crystal_item_key_for_template(&template),
            &template.name,
            "",
            20,
            2,
            u16::from(template.weight),
        );
        session
    }
    #[test]
    fn guild_creation_requires_consumable_npc_grant_and_commits_live_level_costs_once() {
        let mut session = session();
        let before = session.active_character_checkpoint().unwrap();
        assert!(session.submit_shared_guild_name("Knights").is_err());
        assert_eq!(
            session.active_character_checkpoint().unwrap().gold,
            before.gold
        );
        assert!(matches!(
            session.grant_shared_guild_creation_from_npc().as_slice(),
            [ServerPacket::GuildNameRequest]
        ));
        let packets = session.submit_shared_guild_name("Knights").unwrap();
        assert!(packets.iter().any(|packet|matches!(packet,ServerPacket::GuildStatus{guild_name,level:0,spare_points:0,..} if guild_name=="Knights")));
        let after = session.active_character_checkpoint().unwrap();
        assert_eq!(after.gold, 1_000_000);
        let config = session
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let store = config.account_store.lock().unwrap();
        let guild = store.shared_guilds.values().next().unwrap();
        assert_eq!(guild.members[0].identity.character_index, 0);
        assert_eq!(guild.ranks[0].options, 255);
        assert_eq!(
            store.accounts["demo"].characters[0].level, 22,
            "authoritative unsaved live level is included in same transaction"
        );
        assert_eq!(store.accounts["demo"].saves[&0].gold, 1_000_000);
        drop(store);
        assert!(session.submit_shared_guild_name("Other").is_err());
        assert_eq!(
            session.active_character_checkpoint().unwrap().gold,
            1_000_000
        );
    }
    #[test]
    fn guild_creation_failure_rolls_back_wallet_and_consumes_grant() {
        let mut session = session();
        let config = session
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let before = session.active_character_checkpoint().unwrap();
        session.grant_shared_guild_creation_from_npc();
        config.inject_account_store_transaction_fault(
            crate::AccountStoreTransactionFault::BeforePersist,
        );
        assert!(session.submit_shared_guild_name("Knights").is_err());
        assert!(config
            .account_store
            .lock()
            .unwrap()
            .shared_guilds
            .is_empty());
        assert_eq!(
            session
                .active_character_checkpoint()
                .unwrap()
                .inventory_items_json,
            before.inventory_items_json
        );
        assert_eq!(
            session.active_character_checkpoint().unwrap().gold,
            before.gold
        );
        assert!(session.submit_shared_guild_name("Knights").is_err());
        session.grant_shared_guild_creation_from_npc();
        assert!(session.submit_shared_guild_name("\\bad").is_err());
        assert!(session.submit_shared_guild_name("Knights").is_err());
        session.grant_shared_guild_creation_from_npc();
        session.submit_shared_guild_name("Knights").unwrap();
    }
    #[test]
    fn guild_shared_projection_hides_legacy_bank_without_destroying_saved_legacy() {
        let mut session = session();
        let mut systems = session
            .app
            .world_mut()
            .resource_mut::<super::super::resources::Stage5SystemsResource>();
        systems.stage5_systems.guild.name = "Legacy".into();
        systems.stage5_systems.guild.rank = "GuildChief".into();
        systems.stage5_systems.guild.storage_gold = 999_999;
        drop(systems);
        assert!(session
            .world_snapshot()
            .stage5_systems
            .guild
            .name
            .is_empty());
        let checkpoint = session.active_character_checkpoint().unwrap();
        let saved: crate::config::Stage5SystemsState =
            serde_json::from_str(checkpoint.stage5_systems_json.as_deref().unwrap()).unwrap();
        assert_eq!(saved.guild.name, "Legacy");
        assert_eq!(saved.guild.storage_gold, 999_999);
        session.grant_shared_guild_creation_from_npc();
        session.submit_shared_guild_name("Knights").unwrap();
        let view = session.world_snapshot().stage5_systems.guild;
        assert_eq!(view.name, "Knights");
        assert_eq!(view.storage_gold, 0);
        let config = session
            .app
            .world()
            .resource::<RuntimeConfigResource>()
            .config
            .clone();
        let store = config.account_store.lock().unwrap();
        let saved: crate::config::Stage5SystemsState = serde_json::from_str(
            store.accounts["demo"].saves[&0]
                .stage5_systems_json
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(saved.guild.name, "Legacy");
        assert_eq!(saved.guild.storage_gold, 999_999);
    }
}

#[cfg(test)]
#[path = "shared_guild_bank_tests.rs"]
mod bank_tests;

#[cfg(test)]
#[path = "shared_guild_experience_tests.rs"]
mod experience_tests;

#[path = "shared_guild_bank.rs"]
mod bank;

#[path = "shared_guild_buffs.rs"]
pub(super) mod buffs;
